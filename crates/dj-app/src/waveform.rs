//! Serving waveform tiles to the interface.
//!
//! Tiles reach the webview through a custom URI scheme rather than through IPC.
//! An `<img src="wave://...">` is decoded by the browser off the main thread and
//! then moved by a CSS transform, which is compositor work -- exactly what
//! [ADR-0004](../../../docs/adr/0004-waveform-rendering-strategy.md) requires.
//!
//! Sending pixels over IPC instead would mean base64 on the way out and
//! `putImageData` on the way in: per-frame JavaScript drawing on the main
//! thread, which is the pattern that collapses under WebKitGTK.

use dj_core::DeckId;
use dj_render::{
    Colouring, EqPart, GridLines, GridOverlay, Theme, TileSpec, WaveformSummary, encode_png,
    render_tile_with_grid,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The URI scheme tiles are served on.
pub const SCHEME: &str = "wave";

/// Summaries for loaded tracks, plus a cache of encoded tiles.
///
/// Shared between the load path (which fills it) and the protocol handler
/// (which reads it). Locking is fine here: neither is the audio thread.
#[derive(Debug, Default)]
pub struct WaveformStore {
    summaries: Mutex<HashMap<u8, Arc<WaveformSummary>>>,
    /// Beat grid per deck, drawn into the tiles themselves. The working copy:
    /// what the analyser found, plus whatever the DJ has since edited.
    grids: Mutex<HashMap<u8, GridOverlay>>,
    /// What the analyser originally reported, kept so an edit can be undone.
    ///
    /// A grid edit you cannot reverse is one nobody tries, and a DJ who taps a
    /// tempo badly mid-set needs one button to get back to a grid that at least
    /// mostly worked.
    analysed_grids: Mutex<HashMap<u8, GridOverlay>>,
    /// Bumped whenever a deck's tiles stop being valid.
    ///
    /// **This is not belt-and-braces.** Tiles are served with a one-year
    /// `immutable` cache header, so the webview keeps its own copy keyed by
    /// URL. Clearing the cache here does nothing about that copy: without a
    /// discriminant in the URL, loading a second track on the same deck at the
    /// same zoom would redisplay the *first* track's waveform, and editing a
    /// beat grid would appear to do nothing at all.
    epochs: Mutex<HashMap<u8, u32>>,
    /// Encoded PNGs, keyed by the request that produced them. Tiles are
    /// deterministic, so a hit is always byte-identical to a re-render.
    cache: Mutex<HashMap<TileKey, Arc<Vec<u8>>>>,
    /// §116's melody line per deck: the strongest line of notes, in Hz, and
    /// how many frames of the file each point covers.
    melodies: Mutex<HashMap<u8, Arc<Melody>>>,
}

/// §116: a record's melody, as `dj_analysis::melody::pitches` reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Melody {
    /// Hz, or `None` where nothing periodic was found.
    pub hertz: Vec<Option<f32>>,
    /// Frames of the file between two points.
    pub frames_per_point: f64,
}

/// Identifies a tile exactly. Integer-keyed so it can be hashed -- floats
/// cannot, and a tile request is always at integer pixel and zoom steps anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub deck: u8,
    pub width: u32,
    pub height: u32,
    /// Frame of the tile's left edge, rounded. Tiles start on exact boundaries.
    pub start_frame: i64,
    /// Frames per pixel, scaled by 1000 so fractional zoom still keys exactly.
    pub zoom_milli: u64,
    /// Which generation of this deck's content the tile belongs to. See
    /// `WaveformStore::epochs`.
    pub epoch: u32,
    /// Which palette the tile was drawn with.
    ///
    /// Part of the key, not a render-time argument, because tiles are cached
    /// hard (`immutable`, a year). Without it, switching to the light theme
    /// would keep serving the dark tiles already in the cache and the waveform
    /// would simply not change.
    pub theme: Theme,
    /// Which of §25's grid layers the tile carries.
    ///
    /// Part of the key for the same reason the theme is: tiles are cached hard,
    /// `immutable` for a year, so a DJ turning the beat lines off would keep
    /// being served the tiles already drawn with them and nothing on screen
    /// would change.
    pub lines: GridLines,
    /// How the spectral balance is coloured — §110's light, or the three EQ
    /// bands. In the key for the same reason as the theme and the lines.
    pub colouring: Colouring,
    /// Which EQ band's part of the column the tile draws — the whole of it,
    /// or one of the three the lane lays over each other and dims by the
    /// deck's knobs. See `dj_render::EqPart`.
    pub part: EqPart,
}

/// Bound on the tile cache.
///
/// A 512x128 tile encodes to roughly 10-20 kB, so 512 tiles is a few megabytes
/// -- enough for several decks' worth of visible waveform at a few zoom levels,
/// and cheap enough not to think about.
const MAX_CACHED_TILES: usize = 512;

impl WaveformStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store the summary for a deck, replacing whatever was there.
    pub fn set_summary(&self, deck: DeckId, summary: WaveformSummary) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.insert(deck.human_number(), Arc::new(summary));
        }
        // The last record's tune is not this one's.
        if let Ok(mut melodies) = self.melodies.lock() {
            melodies.remove(&deck.human_number());
        }
        // Tiles for the previous track on this deck are now wrong.
        self.invalidate(deck);
    }

    /// Put §110's spectrum into a deck's summary, if the deck still holds the
    /// record it was measured from.
    ///
    /// The spectrum is measured on a background thread after the load, so by
    /// the time it lands the DJ may have loaded something else. `measured` is
    /// the summary it was measured against; if that is no longer the deck's,
    /// the answer belongs to a record that has gone and is dropped rather
    /// than drawn over the one that replaced it. Returns whether it landed.
    pub fn set_spectrum(
        &self,
        deck: DeckId,
        measured: &Arc<WaveformSummary>,
        coloured: WaveformSummary,
    ) -> bool {
        let landed = match self.summaries.lock() {
            Ok(mut summaries) => match summaries.get_mut(&deck.human_number()) {
                Some(current) if Arc::ptr_eq(current, measured) => {
                    *current = Arc::new(coloured);
                    true
                }
                _ => false,
            },
            Err(_) => false,
        };
        // A new generation, so tiles drawn before the colour arrived are
        // fetched again rather than served from the webview's own cache.
        if landed {
            self.invalidate(deck);
        }
        landed
    }

    /// Put §116's melody beside a deck's record, if the deck still holds the
    /// record it was read from — the same guard the spectrum has, for the
    /// same reason: a late answer for a record since replaced would draw one
    /// record's tune over another's waveform.
    ///
    /// Set *before* the spectrum on the thread that measures both, so that
    /// the new epoch the spectrum brings — which is what the lane re-asks on
    /// — finds the melody already here.
    pub fn set_melody(
        &self,
        deck: DeckId,
        measured: &Arc<WaveformSummary>,
        melody: Melody,
    ) -> bool {
        let current = self
            .summaries
            .lock()
            .ok()
            .and_then(|summaries| summaries.get(&deck.human_number()).cloned());
        if !current.is_some_and(|current| Arc::ptr_eq(&current, measured)) {
            return false;
        }
        if let Ok(mut melodies) = self.melodies.lock() {
            melodies.insert(deck.human_number(), Arc::new(melody));
        }
        true
    }

    /// §116's melody for a deck's record, once it has been read.
    #[must_use]
    pub fn melody(&self, deck: u8) -> Option<Arc<Melody>> {
        self.melodies.lock().ok()?.get(&deck).cloned()
    }

    /// Whether a deck's record is still waiting for its spectrum.
    #[must_use]
    pub fn colour_pending(&self, deck: u8) -> bool {
        self.summary(deck)
            .is_some_and(|summary| !summary.has_spectrum())
    }

    pub fn clear(&self, deck: DeckId) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.remove(&deck.human_number());
        }
        if let Ok(mut melodies) = self.melodies.lock() {
            melodies.remove(&deck.human_number());
        }
        if let Ok(mut grids) = self.grids.lock() {
            grids.remove(&deck.human_number());
        }
        if let Ok(mut grids) = self.analysed_grids.lock() {
            grids.remove(&deck.human_number());
        }
        self.invalidate(deck);
    }

    fn invalidate(&self, deck: DeckId) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.retain(|key, _| key.deck != deck.human_number());
        }
        // And move the deck on a generation, so the *webview's* cache misses
        // too. Wrapping is fine: it would take four billion track loads on one
        // deck to come back round, and the tile it collided with would have
        // been evicted long before.
        if let Ok(mut epochs) = self.epochs.lock() {
            let epoch = epochs.entry(deck.human_number()).or_insert(0);
            *epoch = epoch.wrapping_add(1);
        }
    }

    /// The current generation of a deck's content, for building tile URLs.
    #[must_use]
    pub fn epoch(&self, deck: u8) -> u32 {
        self.epochs
            .lock()
            .map(|e| e.get(&deck).copied().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Record what the analyser found: both the working grid and the copy
    /// [`Self::analysed_grid`] hands back when an edit is undone.
    pub fn set_analysed_grid(&self, deck: DeckId, overlay: Option<GridOverlay>) {
        if let Ok(mut grids) = self.analysed_grids.lock() {
            match overlay {
                Some(o) => grids.insert(deck.human_number(), o),
                None => grids.remove(&deck.human_number()),
            };
        }
        self.set_grid(deck, overlay);
    }

    /// What the analyser found, before any editing.
    #[must_use]
    pub fn analysed_grid(&self, deck: u8) -> Option<GridOverlay> {
        self.analysed_grids.lock().ok()?.get(&deck).copied()
    }

    /// Set or clear the beat grid drawn over a deck's tiles.
    ///
    /// Invalidates, because every existing tile for this deck was drawn with
    /// the old grid.
    pub fn set_grid(&self, deck: DeckId, overlay: Option<GridOverlay>) {
        let changed = match self.grids.lock() {
            Ok(mut grids) => {
                let previous = match overlay {
                    Some(o) => grids.insert(deck.human_number(), o),
                    None => grids.remove(&deck.human_number()),
                };
                previous != overlay
            }
            Err(_) => false,
        };
        // Only on a real change: re-rendering every tile because the analyser
        // reported the same grid twice would throw away the cache for nothing.
        if changed {
            self.invalidate(deck);
        }
    }

    #[must_use]
    pub fn grid(&self, deck: u8) -> Option<GridOverlay> {
        self.grids.lock().ok()?.get(&deck).copied()
    }

    #[must_use]
    pub fn summary(&self, deck: u8) -> Option<Arc<WaveformSummary>> {
        self.summaries.lock().ok()?.get(&deck).cloned()
    }

    #[must_use]
    pub fn has_summary(&self, deck: u8) -> bool {
        self.summaries
            .lock()
            .map(|s| s.contains_key(&deck))
            .unwrap_or(false)
    }

    /// Total frames in a deck's track, so the UI can size its strip.
    #[must_use]
    pub fn total_frames(&self, deck: u8) -> Option<usize> {
        Some(self.summary(deck)?.total_frames())
    }

    /// Render and encode a tile, or return a cached copy.
    ///
    /// The palette comes from the key's theme rather than from a caller
    /// argument, so it is impossible to render with one palette and cache under
    /// another.
    pub fn tile_png(&self, key: TileKey) -> Option<Arc<Vec<u8>>> {
        if let Ok(cache) = self.cache.lock()
            && let Some(hit) = cache.get(&key)
        {
            return Some(Arc::clone(hit));
        }

        let summary = self.summary(key.deck)?;
        let spec = TileSpec {
            width: key.width,
            height: key.height,
            start_frame: key.start_frame as f64,
            frames_per_pixel: key.zoom_milli as f64 / 1000.0,
        };
        // The DJ's chosen lines, applied to the deck's own grid. `None` when
        // they have turned all three off, rather than an overlay that draws
        // nothing: that way everyone who wants no grid shares one cache entry
        // instead of paying for a rendering pass that paints no pixels.
        let overlay = key
            .lines
            .any()
            .then(|| self.grid(key.deck))
            .flatten()
            .map(|grid| GridOverlay {
                lines: key.lines,
                ..grid
            });
        let palette = key.theme.palette().coloured(key.colouring).only(key.part);
        let tile = render_tile_with_grid(&summary, &spec, &palette, overlay.as_ref());
        let png = Arc::new(encode_png(&tile).ok()?);

        if let Ok(mut cache) = self.cache.lock() {
            // Crude eviction: clear when full rather than tracking recency. Tiles
            // regenerate in half a millisecond, so a rebuild after a zoom change
            // is cheaper than the bookkeeping an LRU would cost.
            if cache.len() >= MAX_CACHED_TILES {
                cache.clear();
            }
            cache.insert(key, Arc::clone(&png));
        }
        Some(png)
    }

    #[must_use]
    pub fn cached_tiles(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }
}

/// Parse a `wave://` request path into a tile key.
///
/// Shape:
/// `/tile/{deck}/{width}/{height}/{start_frame}/{zoom_milli}/{theme}/{epoch}/{lines}/{colouring}/{part}`
///
/// Deliberately strict. A malformed URL returns `None` and the handler answers
/// 400 rather than guessing, because a silently wrong tile is far harder to
/// diagnose than a missing one. The theme is required for that reason:
/// defaulting an unrecognised word to dark would make a typo in the interface
/// look like a theme that simply does not apply to the waveform.
#[must_use]
pub fn parse_tile_path(path: &str) -> Option<TileKey> {
    let mut parts = path.trim_start_matches('/').split('/');
    if parts.next()? != "tile" {
        return None;
    }

    let key = TileKey {
        deck: parts.next()?.parse().ok()?,
        width: parts.next()?.parse().ok()?,
        height: parts.next()?.parse().ok()?,
        start_frame: parts.next()?.parse().ok()?,
        zoom_milli: parts.next()?.parse().ok()?,
        theme: Theme::from_slug(parts.next()?)?,
        epoch: parts.next()?.parse().ok()?,
        lines: GridLines::from_slug(parts.next()?)?,
        colouring: Colouring::from_slug(parts.next()?)?,
        part: EqPart::from_slug(parts.next()?)?,
    };

    // Nothing may follow, and the numbers must be drawable.
    if parts.next().is_some() || key.width == 0 || key.height == 0 || key.zoom_milli == 0 {
        return None;
    }
    // A tile larger than any plausible screen is either a bug or an attempt to
    // make the app allocate hundreds of megabytes on demand.
    if key.width > 4_096 || key.height > 1_024 {
        return None;
    }
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::SampleRate;
    use std::f32::consts::PI;

    fn samples(frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let v = (2.0 * PI * 440.0 * n as f32 / 48_000.0).sin() * 0.8;
                [v, v]
            })
            .collect()
    }

    fn store_with_track() -> (WaveformStore, DeckId) {
        let store = WaveformStore::new();
        let deck = DeckId::from_human(1).unwrap();
        store.set_summary(
            deck,
            WaveformSummary::analyse(&samples(96_000), SampleRate::DEFAULT),
        );
        (store, deck)
    }

    fn key(deck: u8) -> TileKey {
        TileKey {
            deck,
            width: 256,
            height: 128,
            start_frame: 0,
            zoom_milli: 128_000,
            theme: Theme::Dark,
            epoch: 0,
            lines: GridLines::all(),
            colouring: Colouring::Light,
            part: EqPart::All,
        }
    }

    #[test]
    fn a_tile_is_served_for_a_loaded_deck() {
        let (store, _) = store_with_track();
        let png = store.tile_png(key(1)).unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn an_unloaded_deck_serves_nothing() {
        let (store, _) = store_with_track();
        assert!(store.tile_png(key(2)).is_none());
    }

    /// The claim a grid edit rests on: nudging the grid changes the pixels the
    /// interface is served. The epoch test proves the URL changes; this proves
    /// the picture behind it does, which is what the DJ actually looks at.
    #[test]
    fn editing_the_grid_changes_the_served_tile() {
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

        let (store, deck) = store_with_track();
        let overlay = |anchor: f64| {
            Some(GridOverlay {
                lines: GridLines::all(),
                grid: Beatgrid::new(
                    FramePos::new(anchor),
                    Bpm::new(128.0).unwrap(),
                    Confidence::CERTAIN,
                ),
                sample_rate: SampleRate::DEFAULT,
                phrase: None,
            })
        };

        store.set_analysed_grid(deck, overlay(0.0));
        let mut before_key = key(1);
        before_key.epoch = store.epoch(1);
        let before = store.tile_png(before_key).unwrap();

        // Half a beat later: every line lands somewhere new.
        store.set_grid(deck, overlay(11_250.0));
        let mut after_key = key(1);
        after_key.epoch = store.epoch(1);
        let after = store.tile_png(after_key).unwrap();

        assert_ne!(before, after, "the drawn grid must follow the edit");
    }

    /// Reset has to give back the analyser's grid, and the tile with it.
    #[test]
    fn the_analysers_grid_is_kept_for_reset() {
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

        let (store, deck) = store_with_track();
        let original = GridOverlay {
            lines: GridLines::all(),
            grid: Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(128.0).unwrap(),
                Confidence::new(0.3),
            ),
            sample_rate: SampleRate::DEFAULT,
            phrase: None,
        };
        store.set_analysed_grid(deck, Some(original));

        let mut edited = original;
        edited.grid.anchor = FramePos::new(5_000.0);
        store.set_grid(deck, Some(edited));
        assert_eq!(store.grid(1), Some(edited));

        assert_eq!(
            store.analysed_grid(1),
            Some(original),
            "an edit must not overwrite what there is to reset to"
        );
    }

    #[test]
    fn tiles_are_cached() {
        let (store, _) = store_with_track();
        assert_eq!(store.cached_tiles(), 0);
        let first = store.tile_png(key(1)).unwrap();
        assert_eq!(store.cached_tiles(), 1);
        let second = store.tile_png(key(1)).unwrap();
        // Same allocation, not merely equal bytes.
        assert!(Arc::ptr_eq(&first, &second));
    }

    /// Loading a new track must drop the old track's tiles, or the waveform
    /// would show the previous song.
    #[test]
    fn loading_a_new_track_invalidates_its_tiles() {
        let (store, deck) = store_with_track();
        let _ = store.tile_png(key(1)).unwrap();
        assert_eq!(store.cached_tiles(), 1);

        store.set_summary(
            deck,
            WaveformSummary::analyse(&samples(48_000), SampleRate::DEFAULT),
        );
        assert_eq!(store.cached_tiles(), 0, "stale tiles survived a load");
    }

    #[test]
    fn invalidation_is_per_deck() {
        let store = WaveformStore::new();
        for n in [1u8, 2] {
            store.set_summary(
                DeckId::from_human(n).unwrap(),
                WaveformSummary::analyse(&samples(48_000), SampleRate::DEFAULT),
            );
        }
        let _ = store.tile_png(key(1));
        let _ = store.tile_png(key(2));
        assert_eq!(store.cached_tiles(), 2);

        store.clear(DeckId::from_human(1).unwrap());
        assert_eq!(store.cached_tiles(), 1, "clearing deck 1 touched deck 2");
        assert!(store.has_summary(2));
    }

    #[test]
    fn the_cache_is_bounded() {
        let (store, _) = store_with_track();
        for i in 0..(MAX_CACHED_TILES + 20) {
            let mut k = key(1);
            k.start_frame = i as i64 * 100;
            let _ = store.tile_png(k);
        }
        assert!(
            store.cached_tiles() <= MAX_CACHED_TILES,
            "cache grew to {}",
            store.cached_tiles()
        );
    }

    #[test]
    fn a_well_formed_path_parses() {
        let key = parse_tile_path("/tile/2/512/128/48000/256000/dark/0/bdp/light/all").unwrap();
        assert_eq!(key.deck, 2);
        assert_eq!(key.width, 512);
        assert_eq!(key.height, 128);
        assert_eq!(key.start_frame, 48_000);
        assert_eq!(key.zoom_milli, 256_000);
        assert_eq!(key.theme, Theme::Dark);
    }

    #[test]
    fn the_theme_comes_from_the_path() {
        assert_eq!(
            parse_tile_path("/tile/1/512/128/0/256000/light/0/bdp/light/all")
                .unwrap()
                .theme,
            Theme::Light
        );
    }

    /// **The reason the theme is in the key.** Tiles are cached with a one-year
    /// immutable header. If the two themes shared a key, switching would keep
    /// serving whichever palette happened to be rendered first and the waveform
    /// would simply not change.
    #[test]
    fn the_two_themes_do_not_share_a_cache_entry() {
        let (store, _) = store_with_track();

        let mut light = key(1);
        light.theme = Theme::Light;

        let dark_png = store.tile_png(key(1)).unwrap();
        let light_png = store.tile_png(light).unwrap();

        assert_eq!(store.cached_tiles(), 2, "the themes collided in the cache");
        assert_ne!(
            dark_png.as_ref(),
            light_png.as_ref(),
            "both themes encoded to the same image"
        );
    }

    /// **The reason the grid layers are in the key**, which is the theme's
    /// reason exactly.
    ///
    /// Tiles are cached with a one-year immutable header. If a tile drawn with
    /// beat lines and one drawn without shared a key, a DJ turning the grid off
    /// would keep being served the tiles already drawn with it and nothing on
    /// screen would change — which is how they would conclude the setting does
    /// not work rather than that it has not reached the cache.
    #[test]
    fn a_tile_without_the_grid_is_a_different_tile() {
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

        let (store, deck) = store_with_track();
        store.set_analysed_grid(
            deck,
            Some(GridOverlay {
                lines: GridLines::all(),
                grid: Beatgrid::new(
                    FramePos::new(0.0),
                    Bpm::new(128.0).unwrap(),
                    Confidence::CERTAIN,
                ),
                sample_rate: SampleRate::DEFAULT,
                phrase: None,
            }),
        );

        let mut bare = key(1);
        bare.lines = GridLines {
            beats: false,
            downbeats: false,
            phrases: false,
        };

        let with_grid = store.tile_png(key(1)).unwrap();
        let without = store.tile_png(bare).unwrap();

        assert_eq!(store.cached_tiles(), 2, "the two collided in the cache");
        assert_ne!(
            with_grid.as_ref(),
            without.as_ref(),
            "turning every grid line off drew the same pixels"
        );

        // And the middle case: bars only. Different from both, because the
        // three layers are three separate answers rather than one switch.
        let mut bars = key(1);
        bars.lines = GridLines {
            beats: false,
            downbeats: true,
            phrases: false,
        };
        let bars_png = store.tile_png(bars).unwrap();
        assert_ne!(bars_png.as_ref(), with_grid.as_ref());
        assert_ne!(bars_png.as_ref(), without.as_ref());

        // **Every beat, and none of them emphasised.** This is the case the
        // first version of this test missed, and a mutation found it: with the
        // downbeats gate deleted the three assertions above all still passed,
        // because turning the bars off *alone* only changes the colour a line
        // is painted in — the line itself is drawn either way, by the beats
        // layer. A DJ who turns the bar emphasis off and gets emphasised bars
        // anyway has a switch that does nothing.
        let mut flat = key(1);
        flat.lines = GridLines {
            beats: true,
            downbeats: false,
            phrases: false,
        };
        let flat_png = store.tile_png(flat).unwrap();
        assert_ne!(
            flat_png.as_ref(),
            with_grid.as_ref(),
            "turning off the bar emphasis drew the emphasised bars anyway"
        );
        assert_ne!(flat_png.as_ref(), without.as_ref());
    }

    /// The slug is a round trip, because it is the half of the key a human
    /// reads when the waveform is drawing the wrong thing.
    #[test]
    fn every_combination_of_grid_lines_survives_the_url() {
        for beats in [false, true] {
            for downbeats in [false, true] {
                for phrases in [false, true] {
                    let lines = GridLines {
                        beats,
                        downbeats,
                        phrases,
                    };
                    assert_eq!(
                        GridLines::from_slug(&lines.slug()),
                        Some(lines),
                        "{:?} did not survive {:?}",
                        lines,
                        lines.slug()
                    );
                }
            }
        }
        assert_eq!(GridLines::all().slug(), "bdp");
    }

    #[test]
    fn negative_start_frames_parse() {
        // The strip extends before the track start while scrolled to the very
        // beginning, so those tiles are legitimately requested.
        assert_eq!(
            parse_tile_path("/tile/1/512/128/-2048/256000/dark/0/bdp/light/all")
                .unwrap()
                .start_frame,
            -2_048
        );
    }

    #[test]
    fn malformed_paths_are_rejected_rather_than_guessed() {
        for bad in [
            "",
            "/",
            "/nope/1/512/128/0/1000/dark/0",
            "/tile/1/512/128/0/1000/0/bdp/light/all", // no theme
            "/tile/1/512/128/0/1000/dark/bdp/light/all", // no epoch
            "/tile/1/512/128/0/1000/dark/0/light/all", // no grid
            "/tile/1/512/128/0/1000/dark/0/bdp",      // no colouring
            "/tile/1/512/128/0/1000/dark/0/bdp/light", // no part
            "/tile/1/512/128/0/1000/dark/0/bdp/light/all/extra", // too many
            "/tile/1/512/128/0/1000/dark/0/bd/light/all", // a two-letter grid
            "/tile/1/512/128/0/1000/dark/0/bpd/light/all", // out of order
            "/tile/1/512/128/0/1000/dark/0/BDP/light/all", // grids are lower-case
            "/tile/x/512/128/0/1000/dark/0/bdp/light/all", // non-numeric deck
            "/tile/1/0/128/0/1000/dark/0/bdp/light/all", // zero width
            "/tile/1/512/0/0/1000/dark/0/bdp/light/all", // zero height
            "/tile/1/512/128/0/0/dark/0/bdp/light/all", // zero zoom
            "/tile/1/512/128/0/1000/sepia/0/bdp/light/all", // not a theme
            "/tile/1/512/128/0/1000/Dark/0/bdp/light/all", // themes are lower-case
            "/tile/1/512/128/0/1000/dark/0/bdp/rainbow/all", // not a colouring
            "/tile/1/512/128/0/1000/dark/0/bdp/Light/all", // colourings are lower-case
            "/tile/1/512/128/0/1000/dark/0/bdp/light/bass", // not a part
            "/tile/1/512/128/0/1000/dark/0/bdp/light/Low", // parts are lower-case
        ] {
            assert!(
                parse_tile_path(bad).is_none(),
                "should have rejected {bad:?}"
            );
        }
    }

    /// An unbounded size in a URL is an invitation to allocate gigabytes from a
    /// single request.
    #[test]
    fn absurd_tile_sizes_are_refused() {
        assert!(parse_tile_path("/tile/1/999999/128/0/1000/dark/0/bdp/light/all").is_none());
        assert!(parse_tile_path("/tile/1/512/999999/0/1000/dark/0/bdp/light/all").is_none());
    }

    #[test]
    fn total_frames_is_reported_for_sizing_the_strip() {
        let (store, _) = store_with_track();
        assert_eq!(store.total_frames(1), Some(96_000));
        assert_eq!(store.total_frames(2), None);
    }

    /// **The reason the epoch exists.** Tiles are served immutable for a year,
    /// so the webview keeps its own copy keyed by URL. Server-side
    /// invalidation does nothing about that copy: without a discriminant that
    /// changes, loading a second track on the same deck at the same zoom
    /// redisplays the *first* track's waveform.
    #[test]
    fn loading_a_new_track_moves_the_deck_to_a_new_epoch() {
        let (store, deck) = store_with_track();
        let first = store.epoch(1);

        store.set_summary(
            deck,
            WaveformSummary::analyse(&samples(48_000), SampleRate::DEFAULT),
        );
        assert_ne!(store.epoch(1), first, "the URL would not have changed");
    }

    /// Editing the grid changes what every tile looks like, so it has to change
    /// the URLs too -- otherwise the edit would appear to do nothing.
    #[test]
    fn setting_a_grid_moves_the_epoch_and_redraws() {
        let (store, deck) = store_with_track();
        let before = store.tile_png(key(1)).unwrap();
        let epoch = store.epoch(1);

        store.set_grid(
            deck,
            Some(GridOverlay {
                lines: GridLines::all(),
                grid: dj_core::Beatgrid::new(
                    dj_core::FramePos::new(0.0),
                    dj_core::Bpm::new(128.0).unwrap(),
                    dj_core::Confidence::new(1.0),
                ),
                sample_rate: SampleRate::DEFAULT,
                phrase: None,
            }),
        );

        assert_ne!(store.epoch(1), epoch, "the grid change was not versioned");
        let after = store.tile_png(key(1)).unwrap();
        assert_ne!(
            before.as_ref(),
            after.as_ref(),
            "the grid was not drawn into the tile"
        );
    }

    /// Re-reporting the same grid must not throw the cache away. The analyser
    /// can hand back an identical result, and re-rendering every tile for it
    /// would be work for nothing.
    #[test]
    fn setting_the_same_grid_again_is_free() {
        let (store, deck) = store_with_track();
        let overlay = GridOverlay {
            lines: GridLines::all(),
            grid: dj_core::Beatgrid::new(
                dj_core::FramePos::new(0.0),
                dj_core::Bpm::new(128.0).unwrap(),
                dj_core::Confidence::new(1.0),
            ),
            sample_rate: SampleRate::DEFAULT,
            phrase: None,
        };

        store.set_grid(deck, Some(overlay));
        let _ = store.tile_png(key(1));
        let epoch = store.epoch(1);
        assert_eq!(store.cached_tiles(), 1);

        store.set_grid(deck, Some(overlay));
        assert_eq!(store.epoch(1), epoch, "an identical grid bumped the epoch");
        assert_eq!(
            store.cached_tiles(),
            1,
            "an identical grid cleared the cache"
        );
    }

    /// Ejecting must take the grid with the track, or the next one would be
    /// drawn under the previous track's beats.
    #[test]
    fn clearing_a_deck_drops_its_grid() {
        let (store, deck) = store_with_track();
        store.set_grid(
            deck,
            Some(GridOverlay {
                lines: GridLines::all(),
                grid: dj_core::Beatgrid::new(
                    dj_core::FramePos::new(0.0),
                    dj_core::Bpm::new(128.0).unwrap(),
                    dj_core::Confidence::new(1.0),
                ),
                sample_rate: SampleRate::DEFAULT,
                phrase: None,
            }),
        );
        assert!(store.grid(1).is_some());

        store.clear(deck);
        assert!(store.grid(1).is_none(), "a grid outlived its track");
    }

    /// **§110's spectrum lands on the record it was measured from, and on no
    /// other.**
    ///
    /// The load-bearing one for the background pass. The spectrum is measured
    /// on its own thread after the load, and a DJ can load another record onto
    /// the deck before it finishes. The late answer must be dropped rather
    /// than painted over the record that replaced it — which would be the
    /// first record's colours on the second record's waveform.
    #[test]
    fn a_spectrum_for_a_record_that_has_gone_is_dropped() {
        let (store, deck) = store_with_track();
        let first = store.summary(1).expect("the first record's summary");
        assert!(
            store.colour_pending(1),
            "a fresh load already had its colour"
        );

        // Another record arrives before the first one's spectrum is done.
        store.set_summary(
            deck,
            WaveformSummary::analyse(&samples(48_000), SampleRate::DEFAULT),
        );
        let second = store.summary(1).expect("the second record's summary");
        let epoch = store.epoch(1);

        let mut late = (*first).clone();
        late.measure_spectrum(&samples(96_000));
        assert!(
            !store.set_spectrum(deck, &first, late),
            "the first record's colour landed on the second"
        );
        assert!(Arc::ptr_eq(&store.summary(1).unwrap(), &second));
        assert_eq!(
            store.epoch(1),
            epoch,
            "a dropped answer still invalidated the tiles"
        );
        assert!(store.colour_pending(1));

        // The second record's own spectrum does land, and moves the epoch so
        // the webview fetches the coloured tiles.
        let mut own = (*second).clone();
        own.measure_spectrum(&samples(48_000));
        assert!(store.set_spectrum(deck, &second, own));
        assert!(!store.colour_pending(1));
        assert_ne!(store.epoch(1), epoch);
    }

    /// The colouring is part of the key, so the same tile in the two
    /// colourings is two different images and neither is served for the other.
    #[test]
    fn the_colouring_is_part_of_what_a_tile_is() {
        let (store, deck) = store_with_track();
        let measured = store.summary(1).unwrap();
        let mut coloured = (*measured).clone();
        coloured.measure_spectrum(&samples(96_000));
        assert!(store.set_spectrum(deck, &measured, coloured));
        let light = store.tile_png(TileKey {
            colouring: Colouring::Light,
            epoch: store.epoch(1),
            ..key(1)
        });
        let bands = store.tile_png(TileKey {
            colouring: Colouring::Bands,
            epoch: store.epoch(1),
            ..key(1)
        });
        assert_ne!(light.unwrap(), bands.unwrap());
    }

    /// **A tune read from one record is never drawn over the next.** The
    /// melody is read on the thread the spectrum is measured on, a second or
    /// so after the load; a DJ who loaded again in between gets nothing
    /// rather than the first record's notes, and a new load clears the old.
    #[test]
    fn a_melody_belongs_to_the_record_it_was_read_from() {
        let (store, deck) = store_with_track();
        let first = store.summary(1).unwrap();
        let tune = Melody {
            hertz: vec![Some(220.0), None, Some(330.0)],
            frames_per_point: 4_800.0,
        };
        assert!(store.set_melody(deck, &first, tune.clone()));
        assert_eq!(store.melody(1).as_deref(), Some(&tune));

        store.set_summary(
            deck,
            WaveformSummary::analyse(&samples(48_000), SampleRate::DEFAULT),
        );
        assert!(
            store.melody(1).is_none(),
            "the last record's tune outlived it"
        );
        assert!(
            !store.set_melody(deck, &first, tune),
            "a late tune landed on the wrong record"
        );
        assert!(store.melody(1).is_none());
    }

    /// **The EQ part is part of the key, and the URL carries it.** The lane
    /// asks for the low, mid and high parts of the same tile and dims each by
    /// its knob; a store that served the whole column for all three would
    /// draw every band three times over and no knob would take anything away.
    #[test]
    fn each_eq_part_is_its_own_tile() {
        let (store, deck) = store_with_track();
        let measured = store.summary(1).unwrap();
        let mut coloured = (*measured).clone();
        coloured.measure_spectrum(&samples(96_000));
        assert!(store.set_spectrum(deck, &measured, coloured));
        let epoch = store.epoch(1);
        let tile = |part: &str| {
            let path = format!("/tile/1/256/128/0/128000/dark/{epoch}/bdp/light/{part}");
            let key = parse_tile_path(&path).unwrap();
            store.tile_png(key).unwrap()
        };
        let (all, low, mid, high) = (tile("all"), tile("low"), tile("mid"), tile("high"));
        assert_ne!(all, low);
        assert_ne!(low, mid);
        assert_ne!(mid, high);
        assert_ne!(high, all);
    }
}

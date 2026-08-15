//! Rasterising waveform tiles.
//!
//! Tiles are fixed-width RGBA images covering a span of the track. The interface
//! lays them end to end and scrolls them with a CSS transform, which is
//! compositor work rather than per-frame drawing -- see
//! `docs/adr/0004-waveform-rendering-strategy.md`.
//!
//! # Why this is CPU work
//!
//! ADR-0004 said "rasterised in Rust (`wgpu`, offscreen)". The architectural
//! requirement it was protecting is *the webview never draws the waveform*, and
//! that is satisfied either way.
//!
//! In practice a tile is a column-fill: for each pixel column, look up one
//! bucket and paint a vertical run. A 512x128 tile is 65k pixel writes, it is
//! memory-bandwidth-bound, and tiles are cached per track per zoom level, so the
//! work happens once and then never again while scrolling. Against that, a GPU
//! path costs a device and queue to manage, shaders to compile, async surface
//! handling, adapter-selection failure modes on headless Linux, and roughly a
//! hundred crates of dependency.
//!
//! So this is a CPU rasteriser, and the trait boundary is the same either way:
//! [`render_tile`] takes a summary and returns pixels. If profiling ever shows
//! tile generation is a bottleneck -- it is not currently close -- a `wgpu`
//! implementation drops in behind the same signature.

use crate::summary::{Bucket, WaveformSummary};
use serde::{Deserialize, Serialize};

/// Bytes per pixel. RGBA8, which is what every image path expects.
pub const BYTES_PER_PIXEL: usize = 4;

/// Where a tile sits in the track and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TileSpec {
    pub width: u32,
    pub height: u32,
    /// Frame at the tile's left edge.
    pub start_frame: f64,
    /// Zoom: how many frames one pixel column covers.
    pub frames_per_pixel: f64,
}

impl TileSpec {
    /// Frames covered by the whole tile.
    #[must_use]
    pub fn frame_span(&self) -> f64 {
        f64::from(self.width) * self.frames_per_pixel
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * BYTES_PER_PIXEL
    }

    /// True when the spec would produce nothing drawable.
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        self.width == 0
            || self.height == 0
            || !self.frames_per_pixel.is_finite()
            || self.frames_per_pixel <= 0.0
            || !self.start_frame.is_finite()
    }
}

/// Colours for the waveform.
///
/// Spectral colouring is not decoration: it is how a DJ reads structure at a
/// glance. A bass-heavy intro and a hi-hat breakdown have completely different
/// shapes in colour and nearly identical shapes in monochrome.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub background: [u8; 4],
    /// Colour for low-dominant content.
    pub low: [u8; 4],
    pub mid: [u8; 4],
    pub high: [u8; 4],
    /// Drawn inside the peaks to show perceived loudness.
    pub rms_tint: [u8; 4],
}

impl Default for Palette {
    fn default() -> Self {
        Self::dark()
    }
}

/// Which way round the interface is.
///
/// The waveform is rasterised here rather than in the webview
/// ([ADR-0004](../../../docs/adr/0004-waveform-rendering.md)), so a theme that
/// only changed CSS would leave a dark waveform sitting on a light page. The
/// choice has to reach the rasteriser, which means it has to travel in the tile
/// URL, which means it is part of the cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    /// The single word that appears in a `wave://` URL.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    /// Parse the URL segment. Strict: an unknown word is not a theme.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    #[must_use]
    pub fn palette(self) -> Palette {
        match self {
            Theme::Dark => Palette::dark(),
            Theme::Light => Palette::light(),
        }
    }
}

impl Palette {
    /// The booth palette. Bright bands on a dark ground.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            // Transparent, so tiles composite over whatever the skin puts behind.
            background: [0, 0, 0, 0],
            low: [129, 140, 248, 255],
            mid: [94, 234, 212, 255],
            high: [251, 191, 36, 255],
            // A white veil reads as "denser" against dark bands.
            rms_tint: [255, 255, 255, 60],
        }
    }

    /// The daylight palette.
    ///
    /// Not the dark colours on a light ground — those are chosen for contrast
    /// against near-black and turn into pale washes on white, which is exactly
    /// where a waveform stops being readable. These are the same three hues
    /// several steps darker, so the low/mid/high distinction survives the
    /// inversion. The RMS veil flips to black for the same reason: a white
    /// tint inside a light-coloured band is invisible.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            background: [0, 0, 0, 0],
            low: [67, 56, 202, 255],
            mid: [15, 118, 110, 255],
            high: [180, 83, 9, 255],
            rms_tint: [0, 0, 0, 52],
        }
    }
    /// Blend the band colours by their energies.
    #[must_use]
    fn colour_for(&self, bucket: &Bucket) -> [u8; 4] {
        let total = bucket.low + bucket.mid + bucket.high;
        if total <= 1e-6 {
            return self.mid;
        }
        let (wl, wm, wh) = (bucket.low / total, bucket.mid / total, bucket.high / total);
        let mix = |a: u8, b: u8, c: u8| {
            (f32::from(a) * wl + f32::from(b) * wm + f32::from(c) * wh).round() as u8
        };
        [
            mix(self.low[0], self.mid[0], self.high[0]),
            mix(self.low[1], self.mid[1], self.high[1]),
            mix(self.low[2], self.mid[2], self.high[2]),
            mix(self.low[3], self.mid[3], self.high[3]),
        ]
    }
}

/// A rasterised tile.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub spec: TileSpec,
    /// RGBA8, row-major, top row first.
    pub pixels: Vec<u8>,
}

impl Tile {
    /// Colour at a pixel, for tests and debugging.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.spec.width || y >= self.spec.height {
            return [0, 0, 0, 0];
        }
        let offset = ((y * self.spec.width + x) as usize) * BYTES_PER_PIXEL;
        [
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ]
    }

    /// Height in pixels of the drawn waveform in a column.
    #[must_use]
    pub fn drawn_height(&self, x: u32) -> u32 {
        (0..self.spec.height)
            .filter(|&y| self.pixel(x, y)[3] > 0)
            .count() as u32
    }
}

/// Rasterise one tile from a summary.
///
/// Never fails: a degenerate spec yields an empty tile, and reading past the end
/// of the track yields transparent columns. A waveform that refuses to draw is
/// worse than one that draws nothing.
#[must_use]
pub fn render_tile(summary: &WaveformSummary, spec: &TileSpec, palette: &Palette) -> Tile {
    if spec.is_degenerate() {
        return Tile {
            spec: TileSpec {
                width: 0,
                height: 0,
                ..*spec
            },
            pixels: Vec::new(),
        };
    }

    let mut pixels = vec![0u8; spec.byte_len()];
    // Fill the background first if it is not transparent.
    if palette.background[3] > 0 {
        for chunk in pixels.chunks_exact_mut(BYTES_PER_PIXEL) {
            chunk.copy_from_slice(&palette.background);
        }
    }

    let level = summary.level_for(spec.frames_per_pixel);
    let centre = f64::from(spec.height) * 0.5;
    let half_height = centre - 1.0;

    for x in 0..spec.width {
        let frame = spec.start_frame + f64::from(x) * spec.frames_per_pixel;
        if frame < 0.0 || frame >= summary.total_frames() as f64 {
            continue;
        }

        let bucket = summary.bucket_at(level, frame);
        if bucket.is_silent() {
            // A silent column still gets a centre line, so the lane reads as
            // "track present but quiet" rather than "track missing".
            paint(
                &mut pixels,
                spec,
                x,
                centre as u32,
                palette.colour_for(&bucket),
            );
            continue;
        }

        let colour = palette.colour_for(&bucket);
        let top = (centre - f64::from(bucket.max.clamp(-1.0, 1.0)) * half_height).round();
        let bottom = (centre - f64::from(bucket.min.clamp(-1.0, 1.0)) * half_height).round();
        let (top, bottom) = (
            top.max(0.0) as u32,
            (bottom.min(f64::from(spec.height) - 1.0)) as u32,
        );

        for y in top..=bottom.max(top) {
            paint(&mut pixels, spec, x, y, colour);
        }

        // RMS body, drawn over the peaks: the visual weight tracks loudness
        // rather than the occasional transient that sets the outline.
        let rms_extent = f64::from(bucket.rms.clamp(0.0, 1.0)) * half_height;
        let rms_top = (centre - rms_extent).round().max(0.0) as u32;
        let rms_bottom = (centre + rms_extent)
            .round()
            .min(f64::from(spec.height) - 1.0) as u32;
        for y in rms_top..=rms_bottom.max(rms_top) {
            blend(&mut pixels, spec, x, y, palette.rms_tint);
        }
    }

    Tile {
        spec: *spec,
        pixels,
    }
}

fn offset_of(spec: &TileSpec, x: u32, y: u32) -> Option<usize> {
    if x >= spec.width || y >= spec.height {
        return None;
    }
    Some(((y * spec.width + x) as usize) * BYTES_PER_PIXEL)
}

fn paint(pixels: &mut [u8], spec: &TileSpec, x: u32, y: u32, colour: [u8; 4]) {
    if let Some(offset) = offset_of(spec, x, y) {
        pixels[offset..offset + BYTES_PER_PIXEL].copy_from_slice(&colour);
    }
}

/// Source-over alpha blend, for the RMS tint.
fn blend(pixels: &mut [u8], spec: &TileSpec, x: u32, y: u32, colour: [u8; 4]) {
    let Some(offset) = offset_of(spec, x, y) else {
        return;
    };
    let alpha = f32::from(colour[3]) / 255.0;
    for channel in 0..3 {
        let existing = f32::from(pixels[offset + channel]);
        let incoming = f32::from(colour[channel]);
        pixels[offset + channel] = (existing * (1.0 - alpha) + incoming * alpha).round() as u8;
    }
    pixels[offset + 3] = pixels[offset + 3].max(colour[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::SampleRate;
    use std::f32::consts::PI;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn sine(frames: usize, frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..frames)
            .flat_map(|n| {
                let v = (2.0 * PI * frequency * n as f32 / 48_000.0).sin() * amplitude;
                [v, v]
            })
            .collect()
    }

    fn spec(width: u32, height: u32, frames_per_pixel: f64) -> TileSpec {
        TileSpec {
            width,
            height,
            start_frame: 0.0,
            frames_per_pixel,
        }
    }

    /// A light theme that rendered identical pixels would be a silent no-op —
    /// the setting would appear to work and change nothing on screen.
    #[test]
    fn the_two_themes_actually_render_differently() {
        let summary = WaveformSummary::analyse(&sine(48_000, 440.0, 0.8), SR);
        let spec = spec(128, 64, 200.0);
        let dark = render_tile(&summary, &spec, &Theme::Dark.palette());
        let light = render_tile(&summary, &spec, &Theme::Light.palette());
        assert_ne!(dark.pixels, light.pixels);
    }

    /// The light bands have to be *dark enough to see on white*. Reusing the
    /// booth colours is the obvious mistake: they are chosen for contrast
    /// against near-black and become pale washes on a light ground, which is
    /// exactly where a waveform stops being readable.
    #[test]
    fn the_light_palette_is_dark_enough_to_read_on_white() {
        let light = Palette::light();
        for (name, colour) in [("low", light.low), ("mid", light.mid), ("high", light.high)] {
            // Rec. 709 luma, the standard perceptual weighting.
            let luma = 0.2126 * f32::from(colour[0])
                + 0.7152 * f32::from(colour[1])
                + 0.0722 * f32::from(colour[2]);
            assert!(
                luma < 140.0,
                "the light theme's {name} band has luma {luma}, too pale against white"
            );
        }
    }

    /// And the dark bands have to be bright enough on near-black, which is the
    /// same test pointing the other way.
    #[test]
    fn the_dark_palette_is_bright_enough_to_read_on_black() {
        let dark = Palette::dark();
        for (name, colour) in [("low", dark.low), ("mid", dark.mid), ("high", dark.high)] {
            let luma = 0.2126 * f32::from(colour[0])
                + 0.7152 * f32::from(colour[1])
                + 0.0722 * f32::from(colour[2]);
            assert!(
                luma > 100.0,
                "the dark theme's {name} band has luma {luma}, too dim against black"
            );
        }
    }

    /// The RMS veil is drawn *inside* the peaks, so it has to contrast with the
    /// band colours rather than with the page. A white tint on a light band is
    /// invisible, which would quietly remove the loudness cue.
    #[test]
    fn the_rms_veil_contrasts_with_its_own_bands() {
        assert!(
            Palette::dark().rms_tint[0] > 200,
            "the dark veil should lighten"
        );
        assert!(
            Palette::light().rms_tint[0] < 60,
            "the light veil should darken"
        );
    }

    /// The theme travels in a URL, so the round trip has to be exact -- a slug
    /// that did not parse back would silently fall through to the default and
    /// serve the wrong tiles.
    #[test]
    fn every_theme_survives_the_url_round_trip() {
        for theme in [Theme::Dark, Theme::Light] {
            assert_eq!(Theme::from_slug(theme.slug()), Some(theme));
        }
        assert_eq!(Theme::from_slug("sepia"), None);
        assert_eq!(Theme::from_slug(""), None);
        assert_eq!(Theme::from_slug("DARK"), None, "parsing is case-sensitive");
    }

    /// Both palettes composite over the skin rather than painting their own
    /// ground, which is what lets a tile sit on any background.
    #[test]
    fn neither_palette_paints_its_own_background() {
        assert_eq!(Palette::dark().background[3], 0);
        assert_eq!(Palette::light().background[3], 0);
    }

    #[test]
    fn a_tile_is_the_expected_size() {
        let summary = WaveformSummary::analyse(&sine(48_000, 440.0, 0.8), SR);
        let tile = render_tile(&summary, &spec(256, 128, 100.0), &Palette::default());
        assert_eq!(tile.pixels.len(), 256 * 128 * 4);
    }

    #[test]
    fn a_loud_track_fills_more_height_than_a_quiet_one() {
        let loud = WaveformSummary::analyse(&sine(96_000, 440.0, 0.95), SR);
        let quiet = WaveformSummary::analyse(&sine(96_000, 440.0, 0.15), SR);
        let s = spec(64, 128, 500.0);
        let palette = Palette::default();

        let loud_tile = render_tile(&loud, &s, &palette);
        let quiet_tile = render_tile(&quiet, &s, &palette);

        assert!(
            loud_tile.drawn_height(32) > quiet_tile.drawn_height(32) * 2,
            "loud {} vs quiet {}",
            loud_tile.drawn_height(32),
            quiet_tile.drawn_height(32)
        );
    }

    #[test]
    fn a_full_scale_signal_nearly_fills_the_tile() {
        let summary = WaveformSummary::analyse(&sine(96_000, 440.0, 1.0), SR);
        let tile = render_tile(&summary, &spec(64, 128, 500.0), &Palette::default());
        let drawn = tile.drawn_height(32);
        assert!(
            drawn >= 120,
            "full scale should nearly fill 128 px, drew {drawn}"
        );
    }

    #[test]
    fn silence_still_draws_a_centre_line() {
        // "Quiet" and "no track loaded" must not look the same.
        let summary = WaveformSummary::analyse(&vec![0.0; 96_000 * 2], SR);
        let tile = render_tile(&summary, &spec(64, 128, 500.0), &Palette::default());
        assert_eq!(
            tile.drawn_height(32),
            1,
            "silence should be a single centre line"
        );
    }

    #[test]
    fn past_the_end_of_the_track_is_transparent() {
        let summary = WaveformSummary::analyse(&sine(10_000, 440.0, 0.8), SR);
        let tile = render_tile(
            &summary,
            &TileSpec {
                width: 64,
                height: 128,
                start_frame: 1_000_000.0,
                frames_per_pixel: 100.0,
            },
            &Palette::default(),
        );
        assert_eq!(tile.drawn_height(32), 0, "past the end should draw nothing");
    }

    #[test]
    fn negative_start_frames_do_not_panic() {
        let summary = WaveformSummary::analyse(&sine(10_000, 440.0, 0.8), SR);
        let tile = render_tile(
            &summary,
            &TileSpec {
                width: 64,
                height: 64,
                start_frame: -5_000.0,
                frames_per_pixel: 100.0,
            },
            &Palette::default(),
        );
        assert_eq!(tile.pixels.len(), 64 * 64 * 4);
    }

    #[test]
    fn degenerate_specs_yield_an_empty_tile_rather_than_failing() {
        let summary = WaveformSummary::analyse(&sine(10_000, 440.0, 0.8), SR);
        let palette = Palette::default();
        for bad in [
            spec(0, 128, 100.0),
            spec(128, 0, 100.0),
            spec(128, 128, 0.0),
            spec(128, 128, -1.0),
            spec(128, 128, f64::NAN),
        ] {
            let tile = render_tile(&summary, &bad, &palette);
            assert!(tile.pixels.is_empty(), "expected an empty tile for {bad:?}");
        }
    }

    /// Colour is the whole reason for the band split -- a bass line and a
    /// hi-hat pattern must not render identically.
    #[test]
    fn bass_and_treble_render_in_different_colours() {
        let bass = WaveformSummary::analyse(&sine(96_000, 60.0, 0.8), SR);
        let treble = WaveformSummary::analyse(&sine(96_000, 12_000.0, 0.8), SR);
        let s = spec(64, 128, 500.0);
        let palette = Palette::default();

        let bass_pixel = render_tile(&bass, &s, &palette).pixel(40, 64);
        let treble_pixel = render_tile(&treble, &s, &palette).pixel(40, 64);

        assert_ne!(
            [bass_pixel[0], bass_pixel[1], bass_pixel[2]],
            [treble_pixel[0], treble_pixel[1], treble_pixel[2]],
            "bass and treble should not share a colour"
        );
    }

    /// Tiles are cached and reused, so the same input must always give the same
    /// bytes -- otherwise the cache would show seams between regenerated tiles.
    #[test]
    fn rendering_is_deterministic() {
        let summary = WaveformSummary::analyse(&sine(96_000, 440.0, 0.7), SR);
        let s = spec(128, 96, 300.0);
        let palette = Palette::default();
        assert_eq!(
            render_tile(&summary, &s, &palette).pixels,
            render_tile(&summary, &s, &palette).pixels
        );
    }

    /// Adjacent tiles must join without a gap or an overlap, or the seams show
    /// as the waveform scrolls.
    #[test]
    fn adjacent_tiles_are_continuous() {
        let summary = WaveformSummary::analyse(&sine(200_000, 440.0, 0.8), SR);
        let palette = Palette::default();
        let width = 64;
        let fpp = 200.0;

        let first = render_tile(
            &summary,
            &TileSpec {
                width,
                height: 128,
                start_frame: 0.0,
                frames_per_pixel: fpp,
            },
            &palette,
        );
        let second = render_tile(
            &summary,
            &TileSpec {
                width,
                height: 128,
                start_frame: first.spec.frame_span(),
                frames_per_pixel: fpp,
            },
            &palette,
        );

        // The column after the first tile's last is the second tile's first.
        let straddling = render_tile(
            &summary,
            &TileSpec {
                width: 2,
                height: 128,
                start_frame: (width as f64 - 1.0) * fpp,
                frames_per_pixel: fpp,
            },
            &palette,
        );

        assert_eq!(
            first.drawn_height(width - 1),
            straddling.drawn_height(0),
            "tile boundary shifted the last column"
        );
        assert_eq!(
            second.drawn_height(0),
            straddling.drawn_height(1),
            "tile boundary shifted the first column of the next tile"
        );
    }

    #[test]
    fn zoom_changes_which_summary_level_is_used() {
        let summary = WaveformSummary::analyse(&sine(500_000, 440.0, 0.8), SR);
        // Zoomed in: finest level. Zoomed out: something coarser.
        assert_eq!(summary.level_for(100.0), 0);
        assert!(summary.level_for(20_000.0) > 0);
    }

    #[test]
    fn a_transparent_background_leaves_untouched_pixels_clear() {
        let summary = WaveformSummary::analyse(&sine(96_000, 440.0, 0.2), SR);
        let tile = render_tile(&summary, &spec(64, 128, 500.0), &Palette::default());
        // A quiet signal leaves the top of the tile untouched.
        assert_eq!(tile.pixel(32, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn an_opaque_background_fills_the_whole_tile() {
        let summary = WaveformSummary::analyse(&sine(96_000, 440.0, 0.2), SR);
        let palette = Palette {
            background: [20, 20, 30, 255],
            ..Palette::default()
        };
        let tile = render_tile(&summary, &spec(64, 128, 500.0), &palette);
        assert_eq!(tile.pixel(32, 0), [20, 20, 30, 255]);
    }
}

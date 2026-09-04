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
use dj_core::{Beatgrid, Phrase, SampleRate};
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
    /// Beat lines. Alpha is the *full-confidence* alpha; a grid the analyser is
    /// unsure of is drawn fainter.
    pub beat: [u8; 4],
    /// Every fourth beat, so the eye can count bars without counting beats.
    pub downbeat: [u8; 4],
    /// The start of a phrase -- the 16 or 32 beat group the music is actually
    /// built from, and the only line on the waveform a DJ can safely mix on.
    ///
    /// Brighter than a downbeat because it is the one being looked for, and
    /// drawn even when the beat and bar lines are too dense to show: at
    /// overview zoom the phrase markers *are* the structure.
    pub phrase: [u8; 4],
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
            beat: [255, 255, 255, 70],
            downbeat: [255, 255, 255, 150],
            // Not a third tier of the same white -- that reads as "even more
            // emphasis" and gets lost among the downbeats at a glance, which is
            // the moment it is needed. And not amber: this line was amber for
            // as long as it has existed, which is *exactly* the high band's
            // colour, so a phrase marker vanished into any bar with hi-hats in
            // it. That is the overload §57 forbids by name, and
            // `the_phrase_line_is_not_a_colour_the_waveform_can_be` now fails
            // if it comes back. Pink is chosen because no mixture of indigo,
            // teal and amber can reach it: every blend has a green channel of
            // at least 140, and this one is 72.
            phrase: [236, 72, 153, 220],
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
            beat: [0, 0, 0, 60],
            downbeat: [0, 0, 0, 130],
            // The same rule as the dark palette's, several steps darker: no
            // blend of these three bands has a green channel under 83, and this
            // one is 24.
            phrase: [190, 24, 93, 230],
        }
    }
    /// Two colours closer than this, on the same lane, read as one.
    ///
    /// Euclidean distance in sRGB, which is crude -- it is not a perceptual
    /// metric and 60 is not a just-noticeable difference. It does not need to
    /// be: the question here is "is this marker the same colour as the
    /// waveform", asked of colours chosen a hue apart, not "can these be told
    /// apart under studio lighting". A cheap measure that answers the actual
    /// question beats a correct one nobody can read.
    pub const CONFUSABLE: f32 = 60.0;

    /// How far `colour` is from the nearest colour the *waveform itself* can be.
    ///
    /// §57: **never overload the same colour with multiple meanings.** On this
    /// lane that rule is not about two swatches, it is about a swatch and a
    /// continuum -- the waveform is every mixture of the three bands, with and
    /// without the RMS veil over it, so a marker drawn in any of those colours
    /// has no colour of its own. It may still be legible by shape; it is no
    /// longer legible by hue, and hue is what a glance uses.
    ///
    /// Sampled rather than solved. The achievable set is a filled triangle in
    /// RGB plus its lightened copy, and 1/32 of the simplex is far finer than
    /// the distance being asked about.
    #[must_use]
    pub fn distance_from_the_waveform(&self, colour: [u8; 3]) -> f32 {
        const STEPS: u32 = 32;
        let mut nearest = f32::MAX;
        for l in 0..=STEPS {
            for m in 0..=(STEPS - l) {
                let h = STEPS - l - m;
                #[allow(clippy::cast_precision_loss)]
                let band = self.colour_for(&Bucket {
                    low: l as f32,
                    mid: m as f32,
                    high: h as f32,
                    ..Bucket::default()
                });
                for over in [false, true] {
                    let drawn = if over {
                        veiled(band, self.rms_tint)
                    } else {
                        band
                    };
                    nearest = nearest.min(separation(colour, [drawn[0], drawn[1], drawn[2]]));
                }
            }
        }
        nearest
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

/// The beat grid, ready to draw over a tile.
///
/// Drawn *here*, in the same pass as the waveform, rather than as an overlay in
/// the interface. That is not an optimisation — it is the only way the two stay
/// locked together. A grid drawn in the webview and a waveform drawn in Rust
/// are two independent coordinate systems that agree only as long as nothing
/// rounds differently, and a beat marker sitting a pixel off the transient it
/// marks is worse than no marker at all. In the same pass they cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridOverlay {
    pub grid: Beatgrid,
    pub sample_rate: SampleRate,
    /// The phrase structure, when the analyser found one.
    ///
    /// Counted in beats from `grid.anchor`, so it travels with the grid it was
    /// measured against and never alone.
    pub phrase: Option<Phrase>,
}

/// Closest two grid lines may be before they stop being lines and start being a
/// wash.
///
/// Fourteen pixels, raised from six after looking at an overview strip: at
/// seven-pixel spacing the grid reads as a picket fence and hides the structure
/// underneath, which is the one thing an overview exists to show. Six was
/// arithmetically "not overlapping" and visually far too dense.
///
/// It costs the scrolling lane nothing. At 256 frames per pixel a 128 BPM beat
/// is 88 pixels — an order of magnitude clear — so the floor only bites at the
/// zoomed-out end, where individual beats are noise and the emphasised lines
/// carry the phrase structure on their own.
const MIN_LINE_SPACING_PX: f64 = 14.0;

/// Beats between emphasised lines.
const BEATS_PER_EMPHASIS: i64 = 4;

/// How faint a zero-confidence grid is drawn, as a fraction of full alpha.
///
/// Not zero. A grid the analyser doubts is still worth seeing — it is usually
/// close, and being able to see *that* it is wrong is what lets someone fix it.
/// But it must not look like a fact, so confidence scales the alpha instead of
/// gating the drawing.
const UNSURE_ALPHA: f64 = 0.3;

/// Draw beat lines over an already-rendered tile.
fn draw_grid(pixels: &mut [u8], spec: &TileSpec, overlay: &GridOverlay, palette: &Palette) {
    let beat_frames = overlay.grid.bpm.beat_frames(overlay.sample_rate);
    if !beat_frames.is_finite() || beat_frames <= 0.0 {
        return;
    }
    // The anchor needs no check of its own: `FramePos::new` clamps non-finite
    // input to zero, so a NaN cannot get this far. That matters more than it
    // looks — a NaN here would not be caught by the bounds test below, because
    // `NaN < 0.0` and `NaN >= width` are both false, and `NaN as u32` is 0. It
    // would paint a line down column zero of every tile in the track. The type
    // is what prevents that; there is a test for it in `dj-core`.

    let beat_px = beat_frames / spec.frames_per_pixel;
    let emphasis_px = beat_px * BEATS_PER_EMPHASIS as f64;

    // Too dense even for the emphasised lines: draw nothing rather than a band
    // of grey over the waveform -- unless there are phrase lines, which are
    // sixteen or thirty-two beats apart and still legible where bars are not.
    let phrase_px = overlay.phrase.map_or(0.0, |p| beat_px * f64::from(p.beats));
    if emphasis_px < MIN_LINE_SPACING_PX && phrase_px < MIN_LINE_SPACING_PX {
        return;
    }
    let draw_every_beat = beat_px >= MIN_LINE_SPACING_PX;
    // Bars have their own density test, separate from the early return above.
    // Without it, a track *with* phrases keeps the early return open -- phrase
    // lines are far enough apart to draw -- and the bar lines it was meant to
    // suppress come back with it. Which is how a 12-pixel picket fence appeared
    // on the overview the moment phrase detection started working.
    let draw_bars = emphasis_px >= MIN_LINE_SPACING_PX;

    let confidence = overlay.grid.confidence.get().clamp(0.0, 1.0);
    let strength = UNSURE_ALPHA + (1.0 - UNSURE_ALPHA) * confidence;

    let anchor = overlay.grid.anchor.get();
    let span = f64::from(spec.width) * spec.frames_per_pixel;
    // One beat of slack at each end so a line whose centre is just outside the
    // tile still contributes its pixel.
    let first = ((spec.start_frame - anchor) / beat_frames).floor() as i64 - 1;
    let last = ((spec.start_frame + span - anchor) / beat_frames).ceil() as i64 + 1;

    for index in first..=last {
        // `rem_euclid`, not `%`: the anchor is a beat somewhere in the middle of
        // the track, so indices before it are negative, and `%` would emphasise
        // the wrong ones on that side.
        let emphasised = index.rem_euclid(BEATS_PER_EMPHASIS) == 0;
        let starts_phrase = overlay.phrase.is_some_and(|p| p.starts_at(index));
        // A phrase line survives density that hides the others. At overview
        // zoom every beat and bar line is suppressed, and the phrase markers
        // are then the only structure left -- which is the zoom level where
        // knowing where the phrases are matters most.
        // Read as: this line is worth drawing if it starts a phrase, or every
        // beat is being drawn, or it is a bar line at a zoom where bars fit.
        let worth_drawing = starts_phrase || draw_every_beat || (emphasised && draw_bars);
        if !worth_drawing {
            continue;
        }

        let frame = anchor + index as f64 * beat_frames;
        let x = ((frame - spec.start_frame) / spec.frames_per_pixel).round();
        if x < 0.0 || x >= f64::from(spec.width) {
            continue;
        }
        let x = x as u32;

        let base = if starts_phrase {
            palette.phrase
        } else if emphasised {
            palette.downbeat
        } else {
            palette.beat
        };
        let colour = [
            base[0],
            base[1],
            base[2],
            (f64::from(base[3]) * strength).round().clamp(0.0, 255.0) as u8,
        ];

        for y in 0..spec.height {
            blend(pixels, spec, x, y, colour);
        }
    }
}

/// Rasterise one tile from a summary.
///
/// Never fails: a degenerate spec yields an empty tile, and reading past the end
/// of the track yields transparent columns. A waveform that refuses to draw is
/// worse than one that draws nothing.
#[must_use]
pub fn render_tile(summary: &WaveformSummary, spec: &TileSpec, palette: &Palette) -> Tile {
    render_tile_with_grid(summary, spec, palette, None)
}

/// Rasterise a tile with the beat grid drawn over it.
///
/// See [`GridOverlay`] for why the grid is drawn here rather than in the
/// interface.
#[must_use]
pub fn render_tile_with_grid(
    summary: &WaveformSummary,
    spec: &TileSpec,
    palette: &Palette,
    overlay: Option<&GridOverlay>,
) -> Tile {
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
        for chunk in pixels.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
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

    // Grid last, so beat lines sit over the waveform rather than under it.
    if let Some(overlay) = overlay {
        draw_grid(&mut pixels, spec, overlay, palette);
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

/// Straight-line distance between two colours in sRGB. See [`Palette::CONFUSABLE`].
#[must_use]
fn separation(a: [u8; 3], b: [u8; 3]) -> f32 {
    let d = |i: usize| f32::from(a[i]) - f32::from(b[i]);
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt()
}

/// `tint` composited over `base`, the way [`blend`] does it to a pixel.
///
/// Separate from `blend` because that one works on a buffer and this one has to
/// answer the same question about a colour nothing has drawn yet.
#[must_use]
fn veiled(base: [u8; 4], tint: [u8; 4]) -> [u8; 4] {
    let alpha = f32::from(tint[3]) / 255.0;
    let mix =
        |i: usize| (f32::from(base[i]) * (1.0 - alpha) + f32::from(tint[i]) * alpha).round() as u8;
    [mix(0), mix(1), mix(2), base[3].max(tint[3])]
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

    // -- the beat grid ----------------------------------------------------

    use dj_core::{Bpm, Confidence, FramePos};

    fn overlay(bpm: f64, anchor: f64, confidence: f64) -> GridOverlay {
        GridOverlay {
            grid: Beatgrid::new(
                FramePos::new(anchor),
                Bpm::new(bpm).unwrap(),
                Confidence::new(confidence),
            ),
            sample_rate: SR,
            phrase: None,
        }
    }

    /// The same, with a phrase structure hung on it.
    fn phrased(bpm: f64, anchor: f64, beats: u32, phrase_anchor: u32) -> GridOverlay {
        GridOverlay {
            phrase: Phrase::new(beats, phrase_anchor),
            ..overlay(bpm, anchor, 1.0)
        }
    }

    /// Columns where the grid painted something, at a given row.
    fn grid_columns(tile: &Tile, without: &Tile) -> Vec<u32> {
        (0..tile.spec.width)
            .filter(|&x| (0..tile.spec.height).any(|y| tile.pixel(x, y) != without.pixel(x, y)))
            .collect()
    }

    /// **A phrase marker is a different colour from a downbeat.**
    ///
    /// Not merely brighter. A third tier of the same white reads as "a bit more
    /// emphasis" at a glance, and a glance is all a phrase marker ever gets --
    /// it is looked at while a track is running and a hand is on the fader.
    #[test]
    fn a_phrase_start_is_drawn_in_its_own_colour() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 20, 440.0, 0.8), SR);
        // 120 BPM: 24 000 frames per beat, 100 frames per pixel -> 240 px per
        // beat, so a 16-beat phrase starts every 3 840 px. Column 0 is one.
        let spec = spec(1_024, 64, 100.0);
        let palette = Palette::default();
        let bars =
            render_tile_with_grid(&summary, &spec, &palette, Some(&overlay(120.0, 0.0, 1.0)));
        let phrases =
            render_tile_with_grid(&summary, &spec, &palette, Some(&phrased(120.0, 0.0, 16, 0)));

        // Column 0 is a downbeat in both, and a phrase start in only one.
        assert_ne!(
            bars.pixel(0, 32),
            phrases.pixel(0, 32),
            "the phrase start was drawn the same as an ordinary downbeat"
        );
        // Column 240 is beat 1: a plain beat in both, so it must not have moved.
        assert_eq!(
            bars.pixel(240, 32),
            phrases.pixel(240, 32),
            "adding phrases changed a line that is not a phrase start"
        );
    }

    /// **§57, checked rather than intended: a marker may not wear the
    /// waveform's colour.**
    ///
    /// The phrase line was `[251, 191, 36]` in the dark palette and `[180, 83,
    /// 9]` in the light one — which are, exactly, those palettes' *high band*.
    /// A phrase marker over a bar with hi-hats in it was therefore drawn in the
    /// colour of the thing it was drawn on, and the two tests above could not
    /// see it: both compare a phrase line to a *downbeat*, which is white, and
    /// both pass on a pure 440 Hz tone that is entirely mid.
    ///
    /// This asks the question §57 actually asks — is there *any* content that
    /// makes this marker disappear — and it asks it of both palettes, because a
    /// theme is where this kind of collision is reintroduced.
    #[test]
    fn the_phrase_line_is_not_a_colour_the_waveform_can_be() {
        for (name, palette) in [("dark", Palette::dark()), ("light", Palette::light())] {
            let phrase = [palette.phrase[0], palette.phrase[1], palette.phrase[2]];
            let distance = palette.distance_from_the_waveform(phrase);
            assert!(
                distance > Palette::CONFUSABLE,
                "the {name} palette draws phrase markers {distance:.0} from a colour the \
                 waveform itself can be, which is under the {} that reads as the same \
                 colour. §57: never overload the same colour with multiple meanings",
                Palette::CONFUSABLE
            );
        }
    }

    /// The bands are the one place on this lane where colour *is* the whole
    /// message, so they have to differ from each other by more than a shade.
    #[test]
    fn the_three_bands_are_three_colours() {
        for (name, palette) in [("dark", Palette::dark()), ("light", Palette::light())] {
            let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
            for (a, b, pair) in [
                (palette.low, palette.mid, "low and mid"),
                (palette.mid, palette.high, "mid and high"),
                (palette.low, palette.high, "low and high"),
            ] {
                let distance = separation(rgb(a), rgb(b));
                assert!(
                    distance > Palette::CONFUSABLE,
                    "the {name} palette's {pair} are {distance:.0} apart"
                );
            }
        }
    }

    /// **Phrase markers survive a zoom that hides every other line.**
    ///
    /// At overview zoom the beat and bar lines are suppressed as too dense --
    /// correctly, they would be a grey wash. The phrase markers are sixteen
    /// times further apart and still legible, and at that zoom they are the
    /// only structure on the strip. Suppressing them with the rest would empty
    /// the overview of exactly what it exists to show.
    #[test]
    fn phrase_markers_are_drawn_where_bars_are_too_dense() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 200, 440.0, 0.8), SR);
        // 8 000 frames per pixel: a beat is 3 px and a bar 12 px, under the
        // 14 px floor, so both are suppressed. A 32-beat phrase is 96 px.
        let spec = spec(1_024, 64, 8_000.0);
        let palette = Palette::default();
        let plain = render_tile(&summary, &spec, &palette);

        let bars_only =
            render_tile_with_grid(&summary, &spec, &palette, Some(&overlay(120.0, 0.0, 1.0)));
        assert!(
            grid_columns(&bars_only, &plain).is_empty(),
            "bars should be suppressed at this zoom; the test is not measuring what it claims"
        );

        let with_phrases =
            render_tile_with_grid(&summary, &spec, &palette, Some(&phrased(120.0, 0.0, 32, 0)));
        let columns = grid_columns(&with_phrases, &plain);
        assert_eq!(
            columns,
            vec![0, 96, 192, 288, 384, 480, 576, 672, 768, 864, 960],
            "phrase markers were suppressed along with the bars"
        );
    }

    /// A phrase that does not start on beat zero moves every marker with it.
    #[test]
    fn an_offset_phrase_start_moves_the_markers() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 200, 440.0, 0.8), SR);
        let spec = spec(1_024, 64, 8_000.0);
        let palette = Palette::default();
        let plain = render_tile(&summary, &spec, &palette);
        // Beat 4 of a 32-beat phrase: 4 beats is 12 px at this zoom.
        let with_phrases =
            render_tile_with_grid(&summary, &spec, &palette, Some(&phrased(120.0, 0.0, 32, 4)));
        let columns = grid_columns(&with_phrases, &plain);
        assert_eq!(columns.first(), Some(&12), "the offset was ignored");
    }

    /// **The measurement that says the grid is in the right place.** Lines must
    /// land exactly one beat apart, because a grid that is merely close is a
    /// grid that walks off the beat over the length of a track.
    #[test]
    fn beat_lines_land_one_beat_apart() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 4, 440.0, 0.8), SR);
        // 120 BPM at 48 kHz is 24 000 frames per beat; at 100 frames per pixel
        // that is a line every 240 pixels.
        let spec = spec(1_024, 64, 100.0);
        let plain = render_tile(&summary, &spec, &Palette::default());
        let gridded = render_tile_with_grid(
            &summary,
            &spec,
            &Palette::default(),
            Some(&overlay(120.0, 0.0, 1.0)),
        );

        let columns = grid_columns(&gridded, &plain);
        assert!(!columns.is_empty(), "no grid was drawn at all");
        assert_eq!(columns, vec![0, 240, 480, 720, 960], "lines are misplaced");
    }

    /// The grid has to follow the anchor, not the tile. An anchor half a beat
    /// along must move every line half a beat along.
    #[test]
    fn the_grid_follows_its_anchor() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 4, 440.0, 0.8), SR);
        let spec = spec(1_024, 64, 100.0);
        let plain = render_tile(&summary, &spec, &Palette::default());

        // Half a beat is 12 000 frames, which is 120 pixels.
        let shifted = render_tile_with_grid(
            &summary,
            &spec,
            &Palette::default(),
            Some(&overlay(120.0, 12_000.0, 1.0)),
        );
        assert_eq!(
            grid_columns(&shifted, &plain),
            vec![120, 360, 600, 840],
            "the grid did not move with its anchor"
        );
    }

    /// A tile in the middle of a track must line up with the tile before it.
    /// This is the join where an off-by-one in the index arithmetic hides, and
    /// it shows up as a visible stutter every tile boundary.
    #[test]
    fn the_grid_is_continuous_across_a_tile_boundary() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 20, 440.0, 0.8), SR);
        let palette = Palette::default();
        let width = 512u32;
        let fpp = 100.0;
        let overlay = overlay(120.0, 3_000.0, 1.0);

        // Absolute pixel positions of every line across two adjacent tiles.
        let mut lines = Vec::new();
        for tile_index in 0..2u32 {
            let start = f64::from(tile_index * width) * fpp;
            let spec = TileSpec {
                width,
                height: 64,
                start_frame: start,
                frames_per_pixel: fpp,
            };
            let plain = render_tile(&summary, &spec, &palette);
            let gridded = render_tile_with_grid(&summary, &spec, &palette, Some(&overlay));
            for x in grid_columns(&gridded, &plain) {
                lines.push(tile_index * width + x);
            }
        }

        assert!(
            lines.len() >= 4,
            "not enough lines to check spacing: {lines:?}"
        );
        for pair in lines.windows(2) {
            assert_eq!(
                pair[1] - pair[0],
                240,
                "spacing broke at a tile boundary: {lines:?}"
            );
        }
    }

    /// Negative beat indices are the case `%` gets wrong. Before the anchor the
    /// index is negative, and plain remainder emphasises the wrong lines on
    /// that side -- so the bar phase would flip halfway through a track.
    #[test]
    fn emphasis_is_consistent_on_both_sides_of_the_anchor() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 20, 440.0, 0.8), SR);
        let palette = Palette::default();
        let fpp = 400.0;
        // Anchor well into the track, so one tile sits before it and one after.
        let overlay = overlay(120.0, 48_000.0 * 5.0, 1.0);

        let strengths = |start: f64| -> Vec<u8> {
            let spec = TileSpec {
                width: 512,
                height: 64,
                start_frame: start,
                frames_per_pixel: fpp,
            };
            let plain = render_tile(&summary, &spec, &palette);
            let gridded = render_tile_with_grid(&summary, &spec, &palette, Some(&overlay));
            grid_columns(&gridded, &plain)
                .into_iter()
                // Row 0 is above the waveform body, so the pixel there is the
                // grid line alone rather than a blend with the waveform.
                .map(|x| gridded.pixel(x, 0)[3])
                .collect()
        };

        // Every fourth line is emphasised, on both sides.
        for start in [0.0, 48_000.0 * 8.0] {
            let alphas = strengths(start);
            assert!(alphas.len() >= 8, "too few lines at {start}: {alphas:?}");
            let strong = alphas.iter().filter(|a| **a > 100).count();
            let weak = alphas.len() - strong;
            assert!(
                weak >= strong * 2,
                "emphasis pattern is wrong at {start}: {alphas:?}"
            );
        }
    }

    /// Zoomed far out, individual beats would merge into a grey band that hides
    /// the waveform. Beats drop out first, then everything.
    #[test]
    fn a_grid_too_dense_to_read_is_not_drawn() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 60, 440.0, 0.8), SR);
        let palette = Palette::default();
        let grid = overlay(120.0, 0.0, 1.0);

        // 24 000 frames per beat. At 6 000 frames per pixel a beat is 4 px --
        // under the limit -- but a bar is 16 px, so bars still draw.
        let bars_only = spec(512, 64, 6_000.0);
        let bar_columns = grid_columns(
            &render_tile_with_grid(&summary, &bars_only, &palette, Some(&grid)),
            &render_tile(&summary, &bars_only, &palette),
        );
        assert!(!bar_columns.is_empty(), "bars should still be drawn");
        for pair in bar_columns.windows(2) {
            assert_eq!(pair[1] - pair[0], 16, "beats were drawn when too dense");
        }

        // At 60 000 frames per pixel even a bar is under half a pixel.
        let nothing = spec(512, 64, 60_000.0);
        assert!(
            grid_columns(
                &render_tile_with_grid(&summary, &nothing, &palette, Some(&grid)),
                &render_tile(&summary, &nothing, &palette),
            )
            .is_empty(),
            "an unreadable grid was drawn anyway"
        );
    }

    /// **A grid the analyser doubts must not look like a fact.** It is still
    /// drawn -- being able to see that it is wrong is what lets someone fix it
    /// -- but visibly fainter.
    #[test]
    fn an_unsure_grid_is_drawn_faintly() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 4, 440.0, 0.8), SR);
        let spec = spec(1_024, 64, 100.0);
        let palette = Palette::default();

        let alpha_at = |confidence: f64| {
            let tile = render_tile_with_grid(
                &summary,
                &spec,
                &palette,
                Some(&overlay(120.0, 0.0, confidence)),
            );
            tile.pixel(240, 0)[3]
        };

        let sure = alpha_at(1.0);
        let unsure = alpha_at(0.1);
        assert!(unsure > 0, "an unsure grid vanished entirely");
        assert!(
            f64::from(unsure) < f64::from(sure) * 0.6,
            "an unsure grid ({unsure}) was nearly as strong as a sure one ({sure})"
        );
    }

    /// No grid means no change. The overlay is optional and must be free when
    /// absent, not merely cheap.
    #[test]
    fn no_grid_leaves_the_tile_untouched() {
        let summary = WaveformSummary::analyse(&sine(48_000 * 2, 440.0, 0.8), SR);
        let spec = spec(512, 64, 100.0);
        let palette = Palette::default();
        assert_eq!(
            render_tile(&summary, &spec, &palette).pixels,
            render_tile_with_grid(&summary, &spec, &palette, None).pixels
        );
    }

    /// **The invariant that keeps a NaN out of the rasteriser.**
    ///
    /// This started as a test that a NaN anchor draws nothing, and a guard in
    /// `draw_grid` to make it pass. Both were wrong: `FramePos::new` clamps
    /// non-finite input to zero, so a NaN anchor is not representable and the
    /// guard was dead code implying a hazard that does not exist.
    ///
    /// The hazard would be real without that clamp — `NaN < 0.0` and
    /// `NaN >= width` are both false, so the bounds check would pass, and
    /// `NaN as u32` is 0, so a line would be painted down column zero of every
    /// tile in the track. So the invariant is worth pinning from this side,
    /// where the consequence lives.
    #[test]
    fn a_non_finite_anchor_cannot_reach_the_rasteriser() {
        let anchor = FramePos::new(f64::NAN);
        assert_eq!(anchor.get(), 0.0, "FramePos stopped sanitising its input");
        assert_eq!(FramePos::new(f64::INFINITY).get(), 0.0);

        // And the grid built from it is an ordinary grid at zero, not a
        // scattering of lines at column zero of every tile.
        let summary = WaveformSummary::analyse(&sine(48_000 * 4, 440.0, 0.8), SR);
        let spec = spec(1_024, 64, 100.0);
        let palette = Palette::default();
        let drawn = render_tile_with_grid(
            &summary,
            &spec,
            &palette,
            Some(&overlay(120.0, f64::NAN, 1.0)),
        );
        assert_eq!(
            grid_columns(&drawn, &render_tile(&summary, &spec, &palette)),
            vec![0, 240, 480, 720, 960]
        );
    }
}

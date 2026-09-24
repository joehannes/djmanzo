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

use crate::summary::{Bucket, SPECTRUM_BANDS, SPECTRUM_EDGES_HZ, WaveformSummary};
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
    /// How the spectral-balance layer is coloured.
    pub colouring: Colouring,
    /// How bright §110's spectrum is drawn, 0..1.
    ///
    /// Full on a dark ground. On a light one the same hues are drawn as ink,
    /// darker: yellow at full strength is a colour a white page swallows.
    pub light_level: f32,
    /// Which of the mixer's EQ bands this tile draws. See [`EqPart`].
    pub part: EqPart,
}

/// How the spectral-balance layer is coloured.
///
/// Both answer the same question — *which part of the spectrum is this moment*
/// — and they suit different jobs, so the DJ chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Colouring {
    /// Three bands at the mixer's isolator crossovers, averaged. What a DJ
    /// sees matches exactly what the LOW, MID and HIGH knobs act on.
    Bands,
    /// §110: the spectrum as light. Every pitch drawn in its own colour of
    /// the visible spectrum — 20 Hz the deepest red an eye sees, 20 kHz a
    /// violet at the edge of ultraviolet, octave for octave between — and the
    /// bands stacked from the centre outward, each as thick as it is loud, so
    /// a column is the colours of what is in it, never a mixture of them.
    #[default]
    Light,
}

impl Colouring {
    /// Both, in the order a picker offers them.
    pub const ALL: [Self; 2] = [Self::Light, Self::Bands];

    /// The word that appears in a `wave://` URL.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Bands => "bands",
            Self::Light => "light",
        }
    }

    /// Parse the URL segment. Strict: an unknown word is not a colouring.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|colouring| colouring.slug() == slug)
    }

    /// What a picker calls it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Bands => "Three bands, like the EQ",
            Self::Light => "The spectrum as light",
        }
    }

    /// What choosing it means, in a DJ's words.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Bands => {
                "Low, mid and high at the mixer's own crossovers, so what you see is what the EQ knobs act on."
            }
            Self::Light => {
                "Every pitch in its own colour of light, 20 Hz deep red to 20 kHz violet: the kick a red core, the voice yellow and green around it, the hats a violet edge. Each part fades with its EQ knob."
            }
        }
    }
}

/// The lowest and the highest pitch a human ear hears, in Hz: §110's two ends.
pub const HEARING_HZ: (f32, f32) = (20.0, 20_000.0);

/// The deepest red and the most violet violet an eye sees, in nanometres —
/// the owner's *red close to infrared* and *violet that might be close to
/// ultraviolet* — matched to [`HEARING_HZ`]'s two ends.
pub const VISIBLE_NM: (f32, f32) = (750.0, 380.0);

/// Where a pitch sits in hearing, by octaves: 0 at 20 Hz, 1 at 20 kHz.
#[must_use]
pub fn hearing_position(hz: f32) -> f32 {
    let (low, high) = HEARING_HZ;
    ((hz.max(1e-3) / low).ln() / (high / low).ln()).clamp(0.0, 1.0)
}

/// §110: the wavelength of light a pitch is drawn in, in nanometres.
///
/// **Octave for octave.** Hearing spans ten octaves and sight almost exactly
/// one — 750 to 380 nm is a ratio of 1.97 in the light's own frequency — so a
/// pitch's place among the ten is its light's place in the one:
/// `λ = 750 · (380 / 750)^p`, with `p` the pitch's [`hearing_position`].
/// Every octave of sound is the same step of colour, which is how an ear
/// hears octaves too, and it is one power: fast enough to call per pixel,
/// though the rasteriser reads a table built from it.
///
/// Where things land: a kick's fundamental (50–100 Hz) red, a bass line
/// orange, a voice's body yellow to green, its consonants and a snare's crack
/// cyan to blue, hats and cymbals violet.
#[must_use]
pub fn wavelength_for(hz: f32) -> f32 {
    let (red, violet) = VISIBLE_NM;
    red * (violet / red).powf(hearing_position(hz))
}

/// How dark the two ends of sight are drawn at their darkest.
///
/// The eye's sensitivity falls away towards infrared and ultraviolet, so the
/// spectrum's ends are dimmer than its middle, and that dimming is what tells
/// a 20 Hz rumble from a 60 Hz kick when both are red. Floored, because a
/// violet as dark as the physics would make it is invisible on a dark booth
/// screen, and the hats are the one thing that must never be.
const EDGE_FLOOR: f32 = 0.6;

/// A wavelength as a screen colour, sRGB-encoded, each channel 0..1.
///
/// Dan Bruton's piecewise approximation of the visible spectrum, not the CIE
/// colour-matching functions the first version of §110 used. A screen cannot
/// show a spectral colour exactly — its red primary sits near 612 nm and
/// nothing redder exists on it — and through the CIE functions every
/// wavelength past that came out the one same red. Bruton's ramps lay the
/// spectrum's hues, in order, across what the screen can show, and dim the
/// ends the way the eye does. Every colour it produces has at least one
/// channel at zero: nothing it draws is white or grey.
fn wavelength_rgb(nm: f32) -> [f32; 3] {
    let (r, g, b) = if nm < 440.0 {
        ((440.0 - nm) / 60.0, 0.0, 1.0)
    } else if nm < 490.0 {
        (0.0, (nm - 440.0) / 50.0, 1.0)
    } else if nm < 510.0 {
        (0.0, 1.0, (510.0 - nm) / 20.0)
    } else if nm < 580.0 {
        ((nm - 510.0) / 70.0, 1.0, 0.0)
    } else if nm < 645.0 {
        (1.0, (645.0 - nm) / 65.0, 0.0)
    } else {
        (1.0, 0.0, 0.0)
    };
    let edge = if nm < 420.0 {
        0.3 + 0.7 * (nm - 380.0) / 40.0
    } else if nm > 700.0 {
        0.3 + 0.7 * (780.0 - nm) / 80.0
    } else {
        1.0
    }
    .clamp(EDGE_FLOOR, 1.0);
    [r, g, b].map(|channel: f32| (channel.clamp(0.0, 1.0) * edge).powf(0.8))
}

/// Entries in the pitch-to-colour table: a step of about a twenty-fifth of an
/// octave, finer than any band edge a DJ could see.
const SPECTRUM_STEPS: usize = 256;

/// Every pitch's colour, by [`hearing_position`], built once.
fn spectrum_table() -> &'static [[u8; 3]; SPECTRUM_STEPS] {
    static TABLE: std::sync::OnceLock<[[u8; 3]; SPECTRUM_STEPS]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let (red, violet) = VISIBLE_NM;
        std::array::from_fn(|step| {
            let position = step as f32 / (SPECTRUM_STEPS - 1) as f32;
            wavelength_rgb(red * (violet / red).powf(position))
                .map(|channel| (channel * 255.0).round() as u8)
        })
    })
}

/// The colour at a place in hearing, 0 at 20 Hz and 1 at 20 kHz.
fn colour_at_position(position: f32) -> [u8; 3] {
    let step = (position.clamp(0.0, 1.0) * (SPECTRUM_STEPS - 1) as f32).round() as usize;
    spectrum_table()[step]
}

/// §110: the colour a pitch is drawn in. 20 Hz a deep red, 20 kHz a violet,
/// and every pitch between at its own place in the spectrum.
#[must_use]
pub fn frequency_colour(hz: f32) -> [u8; 3] {
    colour_at_position(hearing_position(hz))
}

/// Where each of the eight bands begins and ends in hearing, lowest first.
fn band_positions() -> &'static [(f32, f32); SPECTRUM_BANDS] {
    static POSITIONS: std::sync::OnceLock<[(f32, f32); SPECTRUM_BANDS]> =
        std::sync::OnceLock::new();
    POSITIONS.get_or_init(|| {
        std::array::from_fn(|band| {
            let low = if band == 0 {
                HEARING_HZ.0
            } else {
                SPECTRUM_EDGES_HZ[band - 1]
            };
            let high = SPECTRUM_EDGES_HZ.get(band).copied().unwrap_or(HEARING_HZ.1);
            (hearing_position(low), hearing_position(high))
        })
    })
}

/// How bright the part of a column beyond its RMS body is, against the body.
///
/// The body is where the sound's weight is; the peaks outside it are the
/// transients. The white veil that used to say so is gone with white itself,
/// so the same fact is a step of brightness: a dense, compressed record is
/// bright almost to its edge, a punchy one bright only at its heart.
const BEYOND_BODY: f32 = 0.62;

/// One column of §110's light: which band holds which share of the height.
///
/// The bands are stacked from the centre line outward, lowest nearest the
/// centre: sub-bass at the heart of the waveform in deep red, the air at its
/// outer edge in violet, each band as thick as its share of what is sounding.
/// So the colours are never mixed — there is no white, because there is no
/// moment with every frequency at once, and a moment with many is many
/// colours side by side — and what is playing is read from the order and the
/// thickness: a kick is a fat red core, a hat a violet fringe, a voice a band
/// of yellow and green between them.
#[derive(Debug, Clone, Copy)]
struct Stack {
    /// Where each band begins, as a fraction of the way from the centre to the
    /// edge; the ninth entry is 1.
    bounds: [f32; SPECTRUM_BANDS + 1],
}

/// How far below the strongest band a band can be and still take room, in
/// decibels.
///
/// **Room is given by loudness, not by amplitude.** The first version stacked
/// the raw levels, and a club record came out almost solid red: a kick's
/// amplitude is many times a hat's, so the hats, the snare and the chords —
/// everything a DJ looks ahead for — were a pixel at the edge. An ear does not
/// hear a hat twenty decibels down as a twentieth of the record, so the stack
/// does not draw it as one: each band's room is how far it stands above a
/// floor this far under the strongest.
pub const STACK_RANGE_DB: f32 = 30.0;

/// The room one band's level takes, 0..1, from its byte (255 the strongest).
fn room(level: u8) -> f32 {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|byte| {
            if byte == 0 {
                return 0.0;
            }
            let db = 20.0 * (byte as f32 / 255.0).log10();
            (1.0 + db / STACK_RANGE_DB).max(0.0)
        })
    })[usize::from(level)]
}

impl Stack {
    /// The stack for a measured spectrum, or `None` for one never measured.
    fn of(spectrum: &[u8; SPECTRUM_BANDS]) -> Option<Self> {
        let rooms = spectrum.map(room);
        let total: f32 = rooms.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut bounds = [0.0f32; SPECTRUM_BANDS + 1];
        let mut sum = 0.0;
        for (band, share) in rooms.iter().enumerate() {
            sum += share;
            bounds[band + 1] = sum / total;
        }
        bounds[SPECTRUM_BANDS] = 1.0;
        Some(Self { bounds })
    }

    /// The band at a fraction of the way out, and the place in hearing its
    /// colour is taken from.
    ///
    /// Within a band the colour runs across the band's own pitches — the
    /// inner edge of the bass band the colour of 112 Hz, its outer edge that
    /// of 266 Hz — so a column is one continuous spectrum, stretched where a
    /// band is loud and squeezed where it is quiet, never a flat block.
    fn at(&self, fraction: f32) -> (usize, f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        let band = (0..SPECTRUM_BANDS)
            .find(|&band| fraction < self.bounds[band + 1])
            .unwrap_or(SPECTRUM_BANDS - 1);
        let (from, to) = (self.bounds[band], self.bounds[band + 1]);
        let within = if to > from {
            (fraction - from) / (to - from)
        } else {
            0.5
        };
        let (low, high) = band_positions()[band];
        (band, low + (high - low) * within)
    }
}

/// Which of the mixer's three EQ bands a tile draws.
///
/// §110 asked for the waveform *interactive* and *live*. The lane draws each
/// record as three images laid over each other, one per EQ band, and dims
/// each by its knob: kill the low and the red heart of the waveform fades to
/// a ghost where it stood, while the shape stays put. The dimming is an
/// opacity the compositor applies, so turning a knob redraws nothing.
///
/// The parts split §110's eight bands at the band edges nearest the
/// isolator's own crossovers (300 Hz and 4 kHz): low is sub, kick and bass,
/// to 266 Hz; mid is low mids to presence, to 3.56 kHz; high is the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqPart {
    /// The whole column. The overview, and the three-band colouring.
    #[default]
    All,
    Low,
    Mid,
    High,
    /// No band at all: the beat grid alone, which the lane lays over its three
    /// parts. Drawn into each part, the grid would be three lines on top of
    /// each other, and a killed low would take a third of every line with it.
    Grid,
}

impl EqPart {
    /// Every part, `All` first.
    pub const ALL: [Self; 5] = [Self::All, Self::Low, Self::Mid, Self::High, Self::Grid];

    /// The word in a `wave://` URL.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Grid => "grid",
        }
    }

    /// Parse the URL segment. Strict, like the theme and the colouring.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|part| part.slug() == slug)
    }

    /// Whether one of §110's eight bands belongs to this part.
    #[must_use]
    pub const fn holds(self, band: usize) -> bool {
        match self {
            Self::All => true,
            Self::Low => band <= 2,
            Self::Mid => band >= 3 && band <= 5,
            Self::High => band >= 6,
            Self::Grid => false,
        }
    }
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
            // Warm rather than brighter white: a third tier of the same hue
            // reads as "even more emphasis" and gets lost among the downbeats
            // at a glance, which is the moment it is needed.
            phrase: [251, 191, 36, 210],
            colouring: Colouring::Light,
            light_level: 1.0,
            part: EqPart::All,
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
            phrase: [180, 83, 9, 220],
            colouring: Colouring::Light,
            // Yellow, the palest hue in the spectrum, at 60 % is about 3:1
            // against white; every other hue is darker than it.
            light_level: 0.6,
            part: EqPart::All,
        }
    }

    /// The same palette, coloured another way.
    #[must_use]
    pub const fn coloured(self, colouring: Colouring) -> Self {
        Self { colouring, ..self }
    }

    /// The same palette, drawing only one EQ band's part of the column.
    #[must_use]
    pub const fn only(self, part: EqPart) -> Self {
        Self { part, ..self }
    }

    /// §110's stack for a moment, when §110 is chosen and the moment was
    /// measured in eight bands.
    fn stack_for(&self, bucket: &Bucket) -> Option<Stack> {
        if self.colouring == Colouring::Light {
            Stack::of(&bucket.spectrum)
        } else {
            None
        }
    }

    /// A place in hearing as a pixel, at a brightness.
    fn spectral(&self, position: f32, brightness: f32) -> [u8; 4] {
        let [r, g, b] = colour_at_position(position);
        let scale = self.light_level * brightness;
        let dim = |channel: u8| (f32::from(channel) * scale).round() as u8;
        [dim(r), dim(g), dim(b), 255]
    }

    /// The colour a whole moment is drawn in, where a column is one colour:
    /// the three bands blended by their energies. A summary with no spectrum
    /// in it — every byte zero — is drawn this way under §110 too, rather
    /// than not at all.
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

/// Which of §25's three grid layers a tile draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridLines {
    /// Every beat. §25's `beats`.
    pub beats: bool,
    /// The bar line every fourth beat. §25's `downbeats`.
    pub downbeats: bool,
    /// Where a phrase begins. §25's `phrases`.
    pub phrases: bool,
}

impl Default for GridLines {
    /// All three, which is what djmanzo has always drawn.
    fn default() -> Self {
        Self::all()
    }
}

impl GridLines {
    /// Every line. The tile a DJ gets unless they have said otherwise.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            beats: true,
            downbeats: true,
            phrases: true,
        }
    }

    /// Whether any line at all would be drawn.
    ///
    /// Three `false`s is a grid overlay with nothing in it, and the tile server
    /// passes `None` instead — so a DJ who turned the whole grid off gets tiles
    /// that share a cache entry with everyone else who did, rather than a
    /// separate rendering pass that draws nothing.
    #[must_use]
    pub const fn any(self) -> bool {
        self.beats || self.downbeats || self.phrases
    }

    /// The three as a slug, for the tile URL. `bdp`, `b-p`, `--p`.
    ///
    /// Readable rather than a number, because a tile URL is the first thing
    /// anybody looks at when the waveform is drawing the wrong thing, and
    /// `5` says nothing about which two of the three are on.
    #[must_use]
    pub fn slug(self) -> String {
        let mark = |on: bool, letter: char| if on { letter } else { '-' };
        [
            mark(self.beats, 'b'),
            mark(self.downbeats, 'd'),
            mark(self.phrases, 'p'),
        ]
        .iter()
        .collect()
    }

    /// Read one back, or `None` for anything that is not three of `bdp-`.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        let mut chars = slug.chars();
        let mut read = |on: char| match chars.next()? {
            c if c == on => Some(true),
            '-' => Some(false),
            _ => None,
        };
        let lines = Self {
            beats: read('b')?,
            downbeats: read('d')?,
            phrases: read('p')?,
        };
        chars.next().is_none().then_some(lines)
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
    /// Which of §25's three grid layers this tile carries.
    ///
    /// §8 Level 1 asks djmanzo to remember a DJ's *preferred waveform display*,
    /// and §25 is the list of what that display is made of. A DJ who reads the
    /// phrases and finds every beat line a distraction can say so, and the
    /// three are separate because they answer separate questions: where the
    /// pulse is, where the bar turns over, where the music starts again.
    pub lines: GridLines,
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
    // A phrase the DJ has turned off is not a phrase for any purpose here --
    // not for the density test, not for the colour, not for the line that
    // survives an overview. Gating at the classification rather than at the
    // draw is what keeps that true: with phrases off, the line that begins one
    // is still a bar line and is drawn as one, in the bar line's own colour.
    let phrases = overlay.lines.phrases;
    let phrase_px = if phrases {
        overlay.phrase.map_or(0.0, |p| beat_px * f64::from(p.beats))
    } else {
        0.0
    };
    if emphasis_px < MIN_LINE_SPACING_PX && phrase_px < MIN_LINE_SPACING_PX {
        return;
    }
    let draw_every_beat = overlay.lines.beats && beat_px >= MIN_LINE_SPACING_PX;
    // Bars have their own density test, separate from the early return above.
    // Without it, a track *with* phrases keeps the early return open -- phrase
    // lines are far enough apart to draw -- and the bar lines it was meant to
    // suppress come back with it. Which is how a 12-pixel picket fence appeared
    // on the overview the moment phrase detection started working.
    let draw_bars = overlay.lines.downbeats && emphasis_px >= MIN_LINE_SPACING_PX;

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
        let emphasised = overlay.lines.downbeats && index.rem_euclid(BEATS_PER_EMPHASIS) == 0;
        let starts_phrase = phrases && overlay.phrase.is_some_and(|p| p.starts_at(index));
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

    // The grid's own layer draws no waveform, only what is laid over it.
    let columns = if palette.part == EqPart::Grid {
        0
    } else {
        spec.width
    };
    for x in 0..columns {
        let frame = spec.start_frame + f64::from(x) * spec.frames_per_pixel;
        if frame < 0.0 || frame >= summary.total_frames() as f64 {
            continue;
        }

        let bucket = summary.bucket_at(level, frame);
        let stack = palette.stack_for(&bucket);
        if bucket.is_silent() {
            // A silent column still gets a centre line, so the lane reads as
            // "track present but quiet" rather than "track missing". Under
            // §110 it is the colour of the lowest band sounding, and belongs
            // to that band's part, so a killed low takes its line with it.
            let line = match stack {
                Some(stack) => {
                    let (band, position) = stack.at(0.0);
                    palette
                        .part
                        .holds(band)
                        .then(|| palette.spectral(position, 1.0))
                }
                None => Some(palette.colour_for(&bucket)),
            };
            if let Some(colour) = line {
                paint(&mut pixels, spec, x, centre as u32, colour);
            }
            continue;
        }

        let top = (centre - f64::from(bucket.max.clamp(-1.0, 1.0)) * half_height).round();
        let bottom = (centre - f64::from(bucket.min.clamp(-1.0, 1.0)) * half_height).round();
        let (top, bottom) = (
            top.max(0.0) as u32,
            (bottom.min(f64::from(spec.height) - 1.0)) as u32,
        );
        let rms_extent = f64::from(bucket.rms.clamp(0.0, 1.0)) * half_height;

        if let Some(stack) = stack {
            paint_stack(
                &mut pixels,
                spec,
                palette,
                x,
                &stack,
                (top, bottom),
                rms_extent,
            );
            continue;
        }
        // One colour for the whole column cannot be split by band, so a part
        // asked for a column never measured — the moment between a record
        // loading and its spectrum landing — draws all of it. The lane asks
        // for parts only once the spectrum is there; this is what keeps a
        // waveform on screen if it ever asks sooner.
        let colour = palette.colour_for(&bucket);
        for y in top..=bottom.max(top) {
            paint(&mut pixels, spec, x, y, colour);
        }

        // RMS body, drawn over the peaks: the visual weight tracks loudness
        // rather than the occasional transient that sets the outline.
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

/// Paint one column of §110's spectrum: the [`Stack`] mirrored about the
/// centre line, filling the peaks, full brightness inside the RMS body and
/// [`BEYOND_BODY`] outside it.
///
/// Each half is scaled to its own peak, so the red heart sits on the centre
/// line and the violet on the outline on both sides even when the waveform is
/// lopsided.
fn paint_stack(
    pixels: &mut [u8],
    spec: &TileSpec,
    palette: &Palette,
    x: u32,
    stack: &Stack,
    (top, bottom): (u32, u32),
    rms_extent: f64,
) {
    let centre = f64::from(spec.height) * 0.5;
    let above = (centre - f64::from(top)).max(1.0);
    let below = (f64::from(bottom) - centre).max(1.0);
    for y in top..=bottom.max(top) {
        // Measured at the pixel's middle, so the row on the centre line is
        // the heart of the lowest band and not the gap before it.
        let offset = f64::from(y) + 0.5 - centre;
        let reach = if offset < 0.0 { above } else { below };
        let (band, position) = stack.at((offset.abs() / reach) as f32);
        if !palette.part.holds(band) {
            continue;
        }
        let brightness = if offset.abs() <= rms_extent.max(1.0) {
            1.0
        } else {
            BEYOND_BODY
        };
        paint(pixels, spec, x, y, palette.spectral(position, brightness));
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

/// Source-over alpha blend, for the RMS tint and the grid.
///
/// The full formula, weighting what is already there by its own alpha. The
/// shortcut this replaced assumed an opaque pixel underneath, and over a
/// transparent one it darkened the incoming colour by its own alpha a second
/// time: a grid line drawn where no waveform is — the whole of §110's grid
/// layer — came out a dim grey instead of the palette's colour.
fn blend(pixels: &mut [u8], spec: &TileSpec, x: u32, y: u32, colour: [u8; 4]) {
    let Some(offset) = offset_of(spec, x, y) else {
        return;
    };
    let alpha = f32::from(colour[3]) / 255.0;
    let under = f32::from(pixels[offset + 3]) / 255.0;
    let out = alpha + under * (1.0 - alpha);
    if out <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let existing = f32::from(pixels[offset + channel]) * under * (1.0 - alpha);
        let incoming = f32::from(colour[channel]) * alpha;
        pixels[offset + channel] = ((existing + incoming) / out).round() as u8;
    }
    pixels[offset + 3] = (out * 255.0).round() as u8;
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
            lines: GridLines::all(),
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
            lines: GridLines::all(),
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

#[cfg(test)]
mod light {
    //! §110, as the owner put it the second time: *the lower limit of the
    //! human ear ... drawn by a red close to infrared ... the upper limit at
    //! about 20khz ... a violet that might be close to ultraviolet ... the
    //! frequencies in between ... determined by a fitting and fast formula ...
    //! i guess white is out of the picture.*
    use super::*;

    /// Hue in degrees of an sRGB colour, 0 for red.
    fn hue(rgb: [u8; 3]) -> f32 {
        let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        if delta <= 1e-6 {
            return 0.0;
        }
        let h = if max == r {
            60.0 * ((g - b) / delta)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        h.rem_euclid(360.0)
    }

    fn rgb(pixel: [u8; 4]) -> [u8; 3] {
        [pixel[0], pixel[1], pixel[2]]
    }

    /// A sine of `hz`, or several summed, as the interleaved stereo a
    /// summary is built from, measured with its spectrum.
    fn tones(hz: &[f32], seconds: f32) -> WaveformSummary {
        let frames = (48_000.0 * seconds) as usize;
        let amplitude = 0.9 / hz.len() as f32;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let t = n as f32 / 48_000.0;
                let v: f32 = hz
                    .iter()
                    .map(|f| (2.0 * std::f32::consts::PI * f * t).sin() * amplitude)
                    .sum();
                [v, v]
            })
            .collect();
        WaveformSummary::analyse_with_spectrum(&samples, dj_core::SampleRate::DEFAULT)
    }

    /// Zoomed out far enough that one column spans several cycles of the
    /// lowest tone, so a column's peak is the tone's peak.
    const WIDE: TileSpec = TileSpec {
        width: 32,
        height: 128,
        start_frame: 0.0,
        frames_per_pixel: 4_096.0,
    };

    /// The pixels of one column, top to bottom.
    fn column(tile: &Tile, x: u32) -> Vec<[u8; 4]> {
        (0..tile.spec.height).map(|y| tile.pixel(x, y)).collect()
    }

    /// **The two ends of hearing are the two ends of sight.**
    #[test]
    fn twenty_hertz_is_the_red_end_and_twenty_kilohertz_the_violet() {
        assert!((wavelength_for(20.0) - 750.0).abs() < 0.01);
        assert!((wavelength_for(20_000.0) - 380.0).abs() < 0.01);
        // Past either end is that end, not a colour off the edge of sight.
        assert_eq!(wavelength_for(5.0), wavelength_for(20.0));
        assert_eq!(wavelength_for(30_000.0), wavelength_for(20_000.0));

        let deep = frequency_colour(20.0);
        assert!(
            deep[1] == 0 && deep[2] == 0 && deep[0] > 100,
            "20 Hz drew {deep:?}, not a red"
        );
        // Deep: darker than the red of a kick, which is how two reds differ.
        assert!(
            frequency_colour(60.0)[0] > deep[0] + 40,
            "20 Hz {deep:?} is not deeper than 60 Hz {:?}",
            frequency_colour(60.0)
        );
        let violet = frequency_colour(20_000.0);
        let h = hue(violet);
        assert!(
            violet[1] == 0 && (270.0..=310.0).contains(&h),
            "20 kHz drew {violet:?}, hue {h}"
        );
    }

    /// **Octave for octave: the middle of hearing is the middle of the
    /// spectrum**, and landmarks land where a DJ would look for them.
    #[test]
    fn a_kick_is_red_a_voice_green_and_a_hat_violet() {
        let kick = frequency_colour(60.0);
        assert!(hue(kick) < 10.0 && kick[0] == 255, "60 Hz drew {kick:?}");
        let bass = hue(frequency_colour(150.0));
        assert!(
            (10.0..=45.0).contains(&bass),
            "150 Hz is hue {bass}, not orange"
        );
        // ~630 Hz is the geometric middle of hearing.
        let voice = hue(frequency_colour(632.0));
        assert!(
            (70.0..=150.0).contains(&voice),
            "632 Hz is hue {voice}, not green"
        );
        let snare = hue(frequency_colour(3_000.0));
        assert!(
            (190.0..=250.0).contains(&snare),
            "3 kHz is hue {snare}, not blue"
        );
        let hat = hue(frequency_colour(12_000.0));
        assert!(
            (260.0..=310.0).contains(&hat),
            "12 kHz is hue {hat}, not violet"
        );
    }

    /// **Every pitch a step further along the spectrum than the one below**,
    /// all the way: no two neighbouring octaves the same colour, which is how
    /// the first version failed, and no pitch going back.
    #[test]
    fn rising_pitch_runs_the_spectrum_in_order() {
        let mut last = 0.0f32;
        let mut distinct = 0;
        for step in 0..=200 {
            let position = step as f32 / 200.0;
            let h = hue(colour_at_position(position));
            assert!(
                h + 0.5 >= last,
                "hue went back from {last} to {h} at {position}"
            );
            if h > last + 0.5 {
                distinct += 1;
            }
            last = h;
        }
        assert!(last > 270.0, "the top of hearing only reached hue {last}");
        assert!(distinct > 120, "only {distinct} distinct steps of colour");
    }

    /// **The load-bearing one: nothing is white.** Not a colour in the table
    /// and not a pixel of a column with every band sounding — the moment the
    /// first version drew pure white.
    #[test]
    fn white_is_out_of_the_picture() {
        for colour in spectrum_table() {
            assert_eq!(
                colour.iter().min(),
                Some(&0),
                "{colour:?} is a tint of white, not a colour of the spectrum"
            );
        }
        // A tone in the middle of every one of the eight bands at once.
        let everything = tones(
            &[
                30.0, 75.0, 180.0, 420.0, 1_000.0, 2_400.0, 5_600.0, 13_000.0,
            ],
            4.0,
        );
        assert!(
            everything.level(0)[400]
                .spectrum
                .iter()
                .all(|&band| band > 60),
            "not every band sounding: {:?}",
            everything.level(0)[400].spectrum
        );
        let tile = render_tile(&everything, &WIDE, &Palette::dark());
        let drawn: Vec<[u8; 4]> = column(&tile, 16).into_iter().filter(|p| p[3] > 0).collect();
        assert!(drawn.len() > 60, "only {} pixels drawn", drawn.len());
        for pixel in &drawn {
            assert!(
                pixel[..3].iter().min() == Some(&0),
                "a full spectrum drew {pixel:?}"
            );
        }
        // Instead: the spectrum itself, red to violet, in one column.
        let hues: Vec<f32> = drawn.iter().map(|p| hue(rgb(*p))).collect();
        assert!(hues.iter().any(|h| *h < 15.0), "no red in {hues:?}");
        assert!(hues.iter().any(|h| (90.0..150.0).contains(h)), "no green");
        assert!(hues.iter().any(|h| *h > 260.0), "no violet");
    }

    /// **Drawn from real tones: a sub-bass note at the heart, an air tone at
    /// the edge.** Two sines at once — 30 Hz and 14 kHz — and the column shows
    /// both, each in its own colour and in its own place, rather than the one
    /// colour between them.
    #[test]
    fn two_tones_at_once_are_two_colours_in_their_places() {
        let tile = render_tile(&tones(&[30.0, 14_000.0], 4.0), &WIDE, &Palette::dark());
        let heart = tile.pixel(16, 64);
        assert!(
            heart[0] > 120 && heart[1] == 0 && heart[2] == 0,
            "the centre of a 30 Hz + 14 kHz column drew {heart:?}, not red"
        );
        let top = (0..128).find(|&y| tile.pixel(16, y)[3] > 0).unwrap();
        let edge = rgb(tile.pixel(16, top + 1));
        let h = hue(edge);
        assert!(
            (260.0..=310.0).contains(&h),
            "the edge of a 30 Hz + 14 kHz column drew {edge:?}, hue {h}"
        );
        // And nothing between them: none of the orange, yellow and green of
        // the bands that are not sounding. (Blue next to the violet is the
        // air tone's own skirt in the band below it — a 24 dB-an-octave
        // filter still passes a seventh of a tone under an octave away — and
        // is the measurement telling the truth.)
        for pixel in column(&tile, 16).into_iter().filter(|p| p[3] > 0) {
            let h = hue(rgb(pixel));
            assert!(
                !(30.0..=200.0).contains(&h),
                "a column of two tones drew {pixel:?} (hue {h}) between them"
            );
        }
    }

    /// **A band is as thick as it is loud — as loud as an ear hears it.** A
    /// band twenty decibels under the kick takes a third of the kick's room,
    /// not a tenth: in raw amplitude the hats and chords of a club record were
    /// a pixel at the edge of a red column, and on a synthetic drop the
    /// decibel stack draws half its pixels in colours other than red against
    /// the raw stack's 38 %.
    #[test]
    fn a_band_takes_room_as_loud_as_it_sounds() {
        let mut spectrum = [0; SPECTRUM_BANDS];
        spectrum[1] = 255;
        // Twenty decibels down: a tenth of the amplitude.
        spectrum[6] = 26;
        let stack = Stack::of(&spectrum).unwrap();
        let kick = stack.bounds[2] - stack.bounds[1];
        let hats = stack.bounds[7] - stack.bounds[6];
        assert!(
            (hats / kick - 1.0 / 3.0).abs() < 0.02,
            "a band 20 dB down took {hats:.3} against {kick:.3}"
        );
        assert_eq!(stack.at(0.1).0, 1);
        assert_eq!(stack.at(0.9).0, 6);
        // Past the floor, nothing: the filters' own skirts take no room.
        spectrum[6] = 7; // about 31 dB down
        let stack = Stack::of(&spectrum).unwrap();
        assert_eq!(stack.bounds[7], stack.bounds[6], "{:?}", stack.bounds);
        assert!(Stack::of(&[0; SPECTRUM_BANDS]).is_none());
    }

    /// **The three EQ parts are the whole column, cut three ways.** Each drawn
    /// pixel of the full tile is drawn by exactly one part, in the same
    /// colour — so the lane's three layered images, all at full opacity, are
    /// the full waveform, and a knob that dims one dims only its own bands.
    #[test]
    fn the_eq_parts_partition_the_column() {
        let summary = tones(&[40.0, 500.0, 9_000.0], 4.0);
        let dark = Palette::dark();
        let whole = render_tile(&summary, &WIDE, &dark);
        let parts: Vec<Tile> = [EqPart::Low, EqPart::Mid, EqPart::High]
            .into_iter()
            .map(|part| render_tile(&summary, &WIDE, &dark.only(part)))
            .collect();
        let mut drawn_by = [0usize; 3];
        for y in 0..WIDE.height {
            for x in 0..WIDE.width {
                let full = whole.pixel(x, y);
                let owners: Vec<usize> = (0..3).filter(|&i| parts[i].pixel(x, y)[3] > 0).collect();
                if full[3] == 0 {
                    assert!(
                        owners.is_empty(),
                        "a part drew ({x},{y}) outside the waveform"
                    );
                    continue;
                }
                assert_eq!(owners.len(), 1, "({x},{y}) drawn by parts {owners:?}");
                assert_eq!(parts[owners[0]].pixel(x, y), full);
                drawn_by[owners[0]] += 1;
            }
        }
        assert!(
            drawn_by.iter().all(|&n| n > 50),
            "each of three sounding bands should hold room: {drawn_by:?}"
        );
    }

    /// **The grid has a layer of its own, and it is only the grid.** Laid
    /// over the three parts, it is drawn once, in the palette's own colour —
    /// not a dim grey from being blended onto nothing.
    #[test]
    fn the_grid_layer_is_the_grid_alone_in_its_own_colour() {
        use dj_core::{Beatgrid, Bpm, Confidence, FramePos};
        let summary = tones(&[40.0, 500.0, 9_000.0], 4.0);
        let overlay = GridOverlay {
            grid: Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(120.0).unwrap(),
                Confidence::new(1.0),
            ),
            sample_rate: dj_core::SampleRate::DEFAULT,
            lines: GridLines::all(),
            phrase: None,
        };
        let spec = TileSpec {
            width: 256,
            height: 64,
            start_frame: 0.0,
            frames_per_pixel: 512.0,
        };
        let dark = Palette::dark();
        let grid = render_tile_with_grid(&summary, &spec, &dark.only(EqPart::Grid), Some(&overlay));
        let drawn: Vec<u32> = (0..spec.width)
            .filter(|&x| grid.pixel(x, 5)[3] > 0)
            .collect();
        // 120 BPM is 24,000 frames a beat: about 47 px at 512 frames a pixel.
        assert!(
            (5..=7).contains(&drawn.len()),
            "the grid layer drew columns {drawn:?}"
        );
        for x in 0..spec.width {
            if !drawn.contains(&x) {
                assert_eq!(
                    grid.drawn_height(x),
                    0,
                    "the grid layer drew waveform at {x}"
                );
            }
        }
        let beat = grid.pixel(drawn[1], 5);
        assert_eq!(&beat[..3], &dark.beat[..3], "a beat line drew {beat:?}");
    }

    /// **The lane's EQ parts meet where these parts do.** The interface dims
    /// each part by its knob (`ui/src/eqLight.ts`) and works out how much of
    /// it the filter passes from the same two edges; a copy that drifted from
    /// these would dim one band's colours with another band's knob.
    #[test]
    fn the_interface_splits_the_parts_where_the_rasteriser_does() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/eqLight.ts"
        ))
        .expect("the interface's EQ light is where it was");
        let constant = |name: &str| -> f32 {
            let line = source
                .lines()
                .find(|line| line.starts_with(&format!("export const {name} = ")))
                .unwrap_or_else(|| panic!("{name} is not in eqLight.ts"));
            line.trim_end_matches(';')
                .rsplit(' ')
                .next()
                .unwrap()
                .replace('_', "")
                .parse()
                .unwrap()
        };
        let last = |part: EqPart| {
            (0..SPECTRUM_BANDS)
                .rev()
                .find(|&band| part.holds(band))
                .unwrap()
        };
        assert_eq!(constant("LOW_MID_HZ"), SPECTRUM_EDGES_HZ[last(EqPart::Low)]);
        assert_eq!(
            constant("MID_HIGH_HZ"),
            SPECTRUM_EDGES_HZ[last(EqPart::Mid)]
        );
    }

    /// **On a light page, ink.** Every hue dark enough to read on white —
    /// yellow, the palest, included.
    #[test]
    fn on_a_light_page_every_colour_reads() {
        let palette = Palette::light();
        for step in 0..SPECTRUM_STEPS {
            let [r, g, b, _] = palette.spectral(step as f32 / (SPECTRUM_STEPS - 1) as f32, 1.0);
            let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
            assert!(
                luma < 150.0,
                "step {step} is ({r},{g},{b}), too pale on white"
            );
        }
    }

    /// **The three bands are still there, unchanged, for a DJ who wants what
    /// the EQ knobs act on**, and a summary with no spectrum in it falls back
    /// to them rather than drawing nothing.
    #[test]
    fn bands_are_what_they_were_and_an_unmeasured_moment_falls_back() {
        let bucket = Bucket {
            min: -0.5,
            max: 0.5,
            rms: 0.3,
            low: 1.0,
            mid: 0.2,
            high: 0.1,
            spectrum: [0, 0, 0, 0, 0, 0, 0, 255],
        };
        let unmeasured = Bucket {
            spectrum: [0; SPECTRUM_BANDS],
            ..bucket
        };
        let bands = Palette::dark().coloured(Colouring::Bands);
        assert!(bands.stack_for(&bucket).is_none());
        assert!(Palette::dark().stack_for(&unmeasured).is_none());
        assert!(Palette::dark().stack_for(&bucket).is_some());
    }

    /// The URL words round-trip, and an unknown one is refused rather than
    /// defaulted — the same strictness the theme word has.
    #[test]
    fn the_colouring_and_the_part_are_spelled_the_same_both_ways() {
        for colouring in Colouring::ALL {
            assert_eq!(Colouring::from_slug(colouring.slug()), Some(colouring));
        }
        assert_eq!(Colouring::from_slug("rainbow"), None);
        for part in EqPart::ALL {
            assert_eq!(EqPart::from_slug(part.slug()), Some(part));
        }
        assert_eq!(EqPart::from_slug("Low"), None);
        assert!((0..SPECTRUM_BANDS).all(|band| !EqPart::Grid.holds(band)));
        // Every band in exactly one of the three.
        for band in 0..SPECTRUM_BANDS {
            let holders = [EqPart::Low, EqPart::Mid, EqPart::High]
                .into_iter()
                .filter(|part| part.holds(band))
                .count();
            assert_eq!(holders, 1, "band {band}");
        }
    }
}

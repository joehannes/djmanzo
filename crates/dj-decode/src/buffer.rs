//! The buffer a deck plays from.

use dj_core::SampleRate;

/// The engine works in stereo throughout; mono sources are duplicated at decode
/// time so nothing downstream branches on channel count.
pub const CHANNELS: usize = 2;

/// Separation splits a track four ways. The order is [`dj_core::Stem::ALL`] --
/// vocal, drums, bass, other -- and every index downstream means that order.
pub const STEM_COUNT: usize = 4;

/// One frame of separated audio: every stem's channels interleaved, so a deck
/// reads the whole frame as one contiguous chunk instead of chasing four
/// pointers.
pub type StemFrame = [f32; STEM_COUNT * CHANNELS];

/// The separated track a background worker fills and a deck reads.
///
/// The lock is an `RwLock` rather than a channel because separation arrives in
/// chunks while the deck may already be playing: the worker appends, the deck
/// reads what has landed so far. The audio thread only ever `try_read`s it, so
/// it never waits on the worker -- see
/// [`AudioBuffer::stem_frame_interpolated`].
pub type StemBuffer = std::sync::Arc<parking_lot::RwLock<Vec<StemFrame>>>;

/// Decoded audio, interleaved stereo `f32`, ready for the engine.
///
/// Always stereo: mono sources are duplicated at decode time so that nothing
/// downstream has to branch on channel count. Always `f32`: the engine mixes in
/// float, and converting once at load is cheaper than converting per sample.
///
/// # Memory
///
/// This holds the whole track in memory -- roughly 23 MB per minute of stereo
/// audio at 48 kHz. That is fine for the M0 walking skeleton and for a handful
/// of decks, but it does not scale to a browser preview or six long tracks. The
/// [`TrackSource`](crate::source::TrackSource) trait exists so a streaming
/// implementation can replace this without the engine noticing.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Interleaved L,R,L,R...
    samples: Vec<f32>,
    sample_rate: SampleRate,
    /// Stems, if available. 4 stems * 2 channels = 8 floats per frame.
    /// Uses RwLock so the background thread can write chunks.
    stems: StemBuffer,
}

impl AudioBuffer {
    /// Wrap already-interleaved stereo samples.
    ///
    /// A trailing partial frame is dropped rather than accepted, so
    /// `samples.len()` is always an exact multiple of the channel count and the
    /// reader can index without a bounds check on every access.
    pub fn from_interleaved(mut samples: Vec<f32>, sample_rate: SampleRate) -> Self {
        let usable = samples.len() - (samples.len() % CHANNELS);
        samples.truncate(usable);
        let frames = usable / CHANNELS;
        Self {
            samples,
            sample_rate,
            stems: std::sync::Arc::new(parking_lot::RwLock::new(Vec::with_capacity(frames))),
        }
    }

    /// An empty buffer, which every deck starts with. Avoids `Option` in the
    /// engine's hot path.
    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            sample_rate: SampleRate::DEFAULT,
            stems: std::sync::Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    /// Get a reference to the stems lock, to be populated by the background worker.
    pub fn stems_lock(&self) -> StemBuffer {
        self.stems.clone()
    }

    #[must_use]
    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    /// Number of frames (sample pairs).
    #[must_use]
    pub fn len_frames(&self) -> usize {
        self.samples.len() / CHANNELS
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.len_frames() as f64 / self.sample_rate.as_f64()
    }

    #[must_use]
    pub fn as_interleaved(&self) -> &[f32] {
        &self.samples
    }

    /// One frame, or silence past the end.
    ///
    /// Returning silence rather than `None` keeps the reader branch-light: a
    /// deck running off the end of a track just goes quiet.
    #[must_use]
    pub fn frame(&self, index: usize) -> [f32; CHANNELS] {
        let base = index * CHANNELS;
        if base + CHANNELS <= self.samples.len() {
            [self.samples[base], self.samples[base + 1]]
        } else {
            [0.0; CHANNELS]
        }
    }

    /// Linearly interpolated frame at a fractional position.
    ///
    /// The playhead is almost never on an exact frame -- any pitch other than
    /// zero puts it between two. Reading the nearest frame instead of
    /// interpolating produces audible aliasing on sustained material, which is
    /// exactly what a pitched-up track is.
    ///
    /// Linear interpolation is the M0 choice: cheap and correct enough to prove
    /// the path. A windowed-sinc resampler replaces it in M1, where pitch and
    /// keylock arrive.
    #[must_use]
    pub fn frame_interpolated(&self, position: f64) -> [f32; CHANNELS] {
        if position < 0.0 || self.samples.is_empty() {
            return [0.0; CHANNELS];
        }
        let index = position.floor();
        let fraction = (position - index) as f32;
        let index = index as usize;

        let a = self.frame(index);
        let b = self.frame(index + 1);
        [
            a[0] + (b[0] - a[0]) * fraction,
            a[1] + (b[1] - a[1]) * fraction,
        ]
    }

    /// Linearly interpolated stem frame at a fractional position.
    #[must_use]
    pub fn stem_frame_interpolated(&self, position: f64) -> Option<[[f32; CHANNELS]; STEM_COUNT]> {
        if position < 0.0 {
            return None;
        }
        let index = position.floor();
        let fraction = (position - index) as f32;
        let index = index as usize;

        // Use try_read to avoid blocking the audio thread!
        let stems = self.stems.try_read()?;

        // If we haven't processed this far yet, fallback
        if index + 1 >= stems.len() {
            return None;
        }

        let a = stems[index];
        let b = stems[index + 1];

        let mut out = [[0.0; CHANNELS]; STEM_COUNT];
        for (stem, frame) in out.iter_mut().enumerate() {
            let offset = stem * CHANNELS;
            for (channel, sample) in frame.iter_mut().enumerate() {
                let from = a[offset + channel];
                let to = b[offset + channel];
                *sample = from + (to - from) * fraction;
            }
        }
        Some(out)
    }
}

impl Default for AudioBuffer {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer(frames: &[[f32; 2]]) -> AudioBuffer {
        let samples = frames.iter().flat_map(|f| [f[0], f[1]]).collect();
        AudioBuffer::from_interleaved(samples, SampleRate::DEFAULT)
    }

    #[test]
    fn empty_buffer_is_silent_and_zero_length() {
        let b = AudioBuffer::empty();
        assert!(b.is_empty());
        assert_eq!(b.len_frames(), 0);
        assert_eq!(b.frame(0), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(0.5), [0.0, 0.0]);
    }

    #[test]
    fn partial_trailing_frame_is_discarded() {
        // Three samples cannot form two whole stereo frames; the odd one goes.
        let b = AudioBuffer::from_interleaved(vec![1.0, 2.0, 3.0], SampleRate::DEFAULT);
        assert_eq!(b.len_frames(), 1);
        assert_eq!(b.as_interleaved().len(), 2);
    }

    #[test]
    fn frames_read_back_in_order() {
        let b = buffer(&[[1.0, -1.0], [0.5, -0.5]]);
        assert_eq!(b.len_frames(), 2);
        assert_eq!(b.frame(0), [1.0, -1.0]);
        assert_eq!(b.frame(1), [0.5, -0.5]);
    }

    #[test]
    fn reading_past_the_end_yields_silence() {
        let b = buffer(&[[1.0, 1.0]]);
        assert_eq!(b.frame(1), [0.0, 0.0]);
        assert_eq!(b.frame(999), [0.0, 0.0]);
    }

    #[test]
    fn interpolation_hits_exact_frames_exactly() {
        let b = buffer(&[[0.0, 0.0], [1.0, 1.0]]);
        assert_eq!(b.frame_interpolated(0.0), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(1.0), [1.0, 1.0]);
    }

    #[test]
    fn interpolation_is_linear_between_frames() {
        let b = buffer(&[[0.0, 0.0], [1.0, -1.0]]);
        let mid = b.frame_interpolated(0.5);
        assert!((mid[0] - 0.5).abs() < 1e-6);
        assert!((mid[1] + 0.5).abs() < 1e-6);

        let quarter = b.frame_interpolated(0.25);
        assert!((quarter[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn interpolation_past_the_end_fades_into_silence() {
        // Reading between the last frame and the void should ramp down, not
        // click or wrap around.
        let b = buffer(&[[1.0, 1.0]]);
        let past = b.frame_interpolated(0.5);
        assert!((past[0] - 0.5).abs() < 1e-6);
        assert_eq!(b.frame_interpolated(5.0), [0.0, 0.0]);
    }

    #[test]
    fn negative_positions_are_silent() {
        let b = buffer(&[[1.0, 1.0]]);
        assert_eq!(b.frame_interpolated(-0.5), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(-100.0), [0.0, 0.0]);
    }

    #[test]
    fn duration_follows_frame_count_and_rate() {
        let frames = vec![[0.0f32, 0.0]; 48_000];
        let b = buffer(&frames);
        assert!((b.duration_seconds() - 1.0).abs() < 1e-9);
    }
}

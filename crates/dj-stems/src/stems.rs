//! What a separated track is, and what a separator has to promise.
//!
//! The stems themselves are [`dj_core::Stem`], not a type of our own: the
//! pad pages, the parameter registry and the deck all index by that order,
//! and a separator with its own idea of which stem is first would swap two
//! of them with nothing to say so.

use dj_core::Stem;

/// A separated track: four interleaved stereo buffers of equal length.
///
/// Interleaved, like everything else that crosses into the engine, so playback
/// reads four buffers the same way it reads one.
#[derive(Debug, Clone, PartialEq)]
pub struct Stems {
    parts: [Vec<f32>; Stem::COUNT],
    sample_rate: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StemError {
    #[error("stems have different lengths: {0:?}")]
    Ragged([usize; Stem::COUNT]),
    #[error("a stem is not interleaved stereo: {0} samples")]
    NotStereo(usize),
    #[error("sample rate is zero")]
    NoSampleRate,
}

impl Stems {
    /// Build from four interleaved stereo buffers.
    ///
    /// # Errors
    /// When they are not the same length, not stereo, or the rate is zero. All
    /// three would otherwise show up as a crash or as silence on one stem,
    /// several seconds into a set.
    pub fn new(parts: [Vec<f32>; Stem::COUNT], sample_rate: u32) -> Result<Stems, StemError> {
        if sample_rate == 0 {
            return Err(StemError::NoSampleRate);
        }
        let lengths = [
            parts[0].len(),
            parts[1].len(),
            parts[2].len(),
            parts[3].len(),
        ];
        if lengths.iter().any(|len| *len != lengths[0]) {
            return Err(StemError::Ragged(lengths));
        }
        if !lengths[0].is_multiple_of(2) {
            return Err(StemError::NotStereo(lengths[0]));
        }
        Ok(Stems { parts, sample_rate })
    }

    #[must_use]
    pub fn get(&self, stem: Stem) -> &[f32] {
        &self.parts[stem.index()]
    }

    #[must_use]
    pub fn frames(&self) -> usize {
        self.parts[0].len() / 2
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The four stems added back together.
    ///
    /// The invariant the whole crate rests on: a DJ who has not touched a stem
    /// control must hear exactly the track. See the crate note.
    #[must_use]
    pub fn mixed(&self) -> Vec<f32> {
        let mut out = vec![0.0; self.parts[0].len()];
        for part in &self.parts {
            for (sum, sample) in out.iter_mut().zip(part) {
                *sum += *sample;
            }
        }
        out
    }

    /// How far the sum strays from a reference mix, as a peak absolute error.
    ///
    /// For asserting the invariant in tests and for the honest number in a log
    /// when a separator is added.
    #[must_use]
    pub fn error_against(&self, mix: &[f32]) -> f32 {
        self.mixed()
            .iter()
            .zip(mix)
            .map(|(sum, original)| (sum - original).abs())
            .fold(0.0, f32::max)
    }

    /// Take the buffers back out, for handing to the engine.
    #[must_use]
    pub fn into_parts(self) -> [Vec<f32>; Stem::COUNT] {
        self.parts
    }
}

/// Something that can take a mix apart.
///
/// The seam a neural separator will slot into. Deliberately synchronous and
/// whole-track: a separator that streamed would have to be realtime-safe, and
/// none of them are — the pending state belongs to the application, which plays
/// the mix until the stems arrive.
/// Frames of surrounding audio a chunk needs on each side to separate as if it
/// were part of the whole track.
///
/// # Why a chunk cannot be separated alone
///
/// Separation is a windowed transform, so the first and last windows of any
/// buffer have no neighbours to overlap-add with and the reconstruction there
/// is wrong — and the median filters that decide harmonic from percussive want
/// context on both sides of every frame. Separating ten-second chunks
/// independently and butting them together therefore put a **large** glitch at
/// every seam, once every ten seconds, for the whole track.
///
/// # Where the number comes from
///
/// Measured, not guessed. `hpss::seam_tests` separates a passage with varying
/// amounts of surrounding audio and compares its interior against separating
/// the whole track:
///
/// | margin | worst deviation |
/// |-------:|----------------:|
/// | 0      | 3.77            |
/// | 1024   | 0.014           |
/// | 2048   | 0.0053          |
/// | 4096+  | 0.0053          |
///
/// It converges at 2048 and does not improve after: what is left is the median
/// filters' dependence on longer-range statistics, which is a real difference
/// rather than an edge artefact, and at half a percent of full scale it is not
/// audible. 4096 is that convergence point with a factor of two in hand, and on
/// a ten-second chunk it costs under two percent more work.
pub const SEPARATION_MARGIN: usize = 4096;

pub trait Separator: Send + Sync + std::fmt::Debug {
    /// What to call this in a log and in the interface.
    fn name(&self) -> &'static str;

    /// Take `mix` — interleaved stereo — apart.
    ///
    /// # Errors
    /// When the input is not usable. A separator must not fail for reasons of
    /// quality: a bad separation is still a separation, and refusing one leaves
    /// a DJ with nothing.
    fn separate(&self, mix: &[f32], sample_rate: u32) -> Result<Stems, StemError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(len: usize) -> [Vec<f32>; Stem::COUNT] {
        [
            vec![0.1; len],
            vec![0.2; len],
            vec![0.3; len],
            vec![0.4; len],
        ]
    }

    #[test]
    fn every_stem_round_trips_through_its_name_and_index() {
        for stem in Stem::ALL {
            assert_eq!(Stem::parse(stem.name()), Some(stem));
            assert_eq!(Stem::from_index(stem.index()), Some(stem));
        }
        assert_eq!(Stem::parse("guitar"), None);
        assert_eq!(Stem::from_index(4), None);
    }

    /// Two stems sharing an index would share a gain: dropping the vocal would
    /// take the bass with it.
    #[test]
    fn no_two_stems_share_an_index() {
        let mut seen = [false; Stem::COUNT];
        for stem in Stem::ALL {
            assert!(!seen[stem.index()], "{stem} reuses an index");
            seen[stem.index()] = true;
        }
    }

    #[test]
    fn the_stems_add_up() {
        let stems = Stems::new(parts(8), 48_000).unwrap();
        assert!(stems.mixed().iter().all(|s| (s - 1.0).abs() < 1e-6));
        assert_eq!(stems.frames(), 4);
    }

    /// Ragged stems would be silence on one part several seconds into a set.
    #[test]
    fn stems_of_different_lengths_are_refused() {
        let mut ragged = parts(8);
        ragged[2].pop();
        assert!(matches!(
            Stems::new(ragged, 48_000),
            Err(StemError::Ragged(_))
        ));
    }

    #[test]
    fn a_stem_that_is_not_stereo_is_refused() {
        assert!(matches!(
            Stems::new(parts(7), 48_000),
            Err(StemError::NotStereo(7))
        ));
    }

    #[test]
    fn a_rate_of_zero_is_refused() {
        assert_eq!(Stems::new(parts(8), 0), Err(StemError::NoSampleRate));
    }

    #[test]
    fn the_error_against_a_mix_is_the_worst_sample() {
        let stems = Stems::new(parts(4), 48_000).unwrap();
        let mix = vec![1.0, 1.0, 1.0, 0.5];
        assert!((stems.error_against(&mix) - 0.5).abs() < 1e-6);
    }
}

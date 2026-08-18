//! Working out what a track is: tempo, beat grid, key and loudness.
//!
//! Every algorithm here is written rather than borrowed, and that is a licence
//! decision rather than a preference: aubio, libKeyFinder, Essentia and BTrack
//! are all copyleft, and [ADR-0002](../../../docs/adr/0002-clean-room-permissive-licensing.md)
//! rules copyleft out of the core. The only borrowed piece is an FFT
//! (`rustfft`, MIT/Apache).
//!
//! # The rule this crate is written under
//!
//! **An analyser that is confidently wrong is worse than one that says it does
//! not know.** Silently syncing to a bad grid derails a mix at the exact moment
//! the DJ has stopped watching. So:
//!
//! - tempo carries a real [`dj_core::Confidence`], measured as the peak
//!   autocorrelation of the onset curve, and the engine refuses to auto-sync
//!   below a threshold;
//! - key carries its correlation score, so a weak result can be shown as weak;
//! - anything too short, too quiet or too irregular returns `None` rather than
//!   a number.
//!
//! # What can actually be verified
//!
//! Most of this has no ground truth to test against, which is why M2 also calls
//! for a labelled regression set. Two things can be checked outright and are:
//! loudness against the **EBU Tech 3341 conformance figure** (a 1 kHz sine at
//! −23 dBFS must read −23.0 LUFS), and tempo against synthetic click tracks at
//! known BPM. The rest is tested for properties that must hold — transposing
//! the input moves the key round the wheel by a fifth, noise is never
//! confident — rather than for numbers nobody can vouch for.

pub mod key;
pub mod loudness;
pub mod onset;
pub mod regression;
pub mod tempo;

pub use key::KeyAnalysis;
pub use loudness::{Lufs, integrated};
pub use onset::{OnsetEnvelope, detect};
pub use tempo::TempoAnalysis;

use dj_core::SampleRate;

/// Everything the analyser worked out about one track.
///
/// Every field is optional because every one of them can legitimately fail: a
/// field recording has no tempo, a drum loop has no key, and a silent file has
/// no loudness. A missing field means "could not tell", which the interface
/// should show as such rather than filling in a plausible zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Analysis {
    pub tempo: Option<TempoAnalysis>,
    pub key: Option<KeyAnalysis>,
    pub loudness: Lufs,
}

impl Analysis {
    /// Gain that would bring this track to the reference loudness.
    ///
    /// This is the auto-gain figure: the whole point of measuring loudness is
    /// that a DJ should not be riding the trim for every track.
    #[must_use]
    pub fn auto_gain_db(&self) -> f64 {
        self.loudness.gain_to(Lufs::REFERENCE)
    }

    /// Whether the beat grid is worth syncing to.
    #[must_use]
    pub fn is_sync_worthy(&self) -> bool {
        self.tempo
            .is_some_and(|t| t.grid.confidence.is_sync_worthy())
    }
}

/// Analyse a whole track. Interleaved stereo.
///
/// Worker-thread work: this reads the entire track and runs several FFT passes
/// over it. Nothing here is realtime-safe and nothing here should ever be
/// called from the audio thread.
#[must_use]
pub fn analyse(samples: &[f32], sample_rate: SampleRate) -> Analysis {
    let rate = sample_rate.get();
    let envelope = onset::detect(samples, rate);

    Analysis {
        tempo: tempo::analyse(&envelope, sample_rate),
        key: key::detect(samples, rate),
        loudness: loudness::integrated(samples, rate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;

    /// A click track carrying a C major chord: something with both a tempo and
    /// a key, which is what a real track is.
    fn musical(bpm: f64, seconds: f64) -> Vec<f32> {
        use std::f32::consts::TAU;
        let rate = SR.get();
        let mut audio = onset::tests::clicks(bpm, seconds, rate);
        let frames = audio.len() / 2;
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            // C, E, G across two octaves.
            let mut v = 0.0;
            for hz in [130.81, 164.81, 196.0, 261.63, 329.63, 392.0] {
                v += (TAU * hz * t).sin();
            }
            let v = v / 6.0 * 0.25;
            audio[n * 2] += v;
            audio[n * 2 + 1] += v;
        }
        audio
    }

    #[test]
    fn a_musical_track_yields_all_three_measurements() {
        let analysis = analyse(&musical(128.0, 30.0), SR);

        let tempo = analysis.tempo.expect("should find a tempo");
        assert!(
            (tempo.grid.bpm.get() - 128.0).abs() < 1.5,
            "measured {}",
            tempo.grid.bpm.get()
        );
        assert!(analysis.is_sync_worthy());

        let key = analysis.key.expect("should find a key");
        assert_eq!(key.key.camelot(), "8B", "detected {}", key.key.standard());

        assert!(!analysis.loudness.is_silent());
    }

    /// The point of measuring loudness: the trim is set for you.
    #[test]
    fn auto_gain_moves_a_track_towards_the_reference() {
        let analysis = analyse(&musical(128.0, 20.0), SR);
        let gain = analysis.auto_gain_db();
        let corrected = analysis.loudness.get() + gain;
        assert!(
            (corrected - Lufs::REFERENCE).abs() < 0.01,
            "auto-gain should land on the reference, got {corrected}"
        );
    }

    /// Every field can legitimately fail, and a failure must read as "could not
    /// tell" rather than as a plausible zero.
    #[test]
    fn silence_reports_nothing_rather_than_zeroes() {
        let analysis = analyse(&vec![0.0f32; SR.get() as usize * 2 * 30], SR);
        assert!(analysis.key.is_none());
        assert!(analysis.loudness.is_silent());
        assert!(!analysis.is_sync_worthy());
        // And asks for no gain, rather than infinite gain.
        assert_eq!(analysis.auto_gain_db(), 0.0);
    }

    #[test]
    fn a_short_clip_is_not_guessed_at() {
        let analysis = analyse(&musical(128.0, 3.0), SR);
        assert!(
            analysis.tempo.is_none(),
            "3 seconds is not enough for tempo"
        );
        assert!(!analysis.is_sync_worthy());
    }

    #[test]
    fn an_empty_track_does_not_panic() {
        let analysis = analyse(&[], SR);
        assert!(analysis.tempo.is_none());
        assert!(analysis.key.is_none());
        assert!(analysis.loudness.is_silent());
    }
}

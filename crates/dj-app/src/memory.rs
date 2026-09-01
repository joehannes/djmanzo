//! Finding a record from what you remember of it.
//!
//! # Three ways in, because people remember three different things
//!
//! - **The words.** A line, half of it, with the accents missing. Searched
//!   against lyrics djmanzo has fetched for the collection.
//! - **A description.** "That bachata with the piano, sounds like Aventura,
//!   I heard it at the beach bar." Handed to the assistant, which is the one
//!   part of djmanzo that can turn a sentence like that into names.
//! - **A hum.** Sung into the microphone, and read for its key and its tempo.
//!
//! They are one feature rather than three because a DJ trying to find a record
//! uses all of them at once — a bit of a line, a rough idea of the sound, and
//! a tempo their hands remember — and any one of them alone usually is not
//! enough.
//!
//! # What the hum does, and what it does not
//!
//! It does **not** identify the song. Recognising a recording from a hum needs
//! a licensed fingerprint service with a catalogue of tens of millions of
//! reference melodies; djmanzo has no such service and is not going to pretend
//! otherwise by shipping something that gets it right one time in five.
//!
//! What it does is real and useful: it runs the hum through djmanzo's **own**
//! analysis — the same tempo and key detection every track in the collection
//! has already been through — and narrows the collection to records near that
//! key and that tempo. A DJ who can hum the bassline knows more than they can
//! type, and this is the part of that knowledge a machine can actually use.

use dj_core::{MusicalKey, SampleRate};

/// The longest hum djmanzo will listen to, in seconds.
///
/// Twelve. Key detection wants a few seconds of pitch to be sure and gains
/// nothing after that, and a longer clip is a bigger thing to hand across the
/// interface boundary for no better answer.
pub const LONGEST_HUM: f32 = 12.0;

/// The shortest hum worth reading.
///
/// Two seconds. Below that there is not enough pitch for a key and not enough
/// onsets for a tempo, and an answer drawn from a cough is worse than saying
/// "sing a bit more".
pub const SHORTEST_HUM: f32 = 2.0;

/// How far either side of the hummed tempo counts as near, in BPM.
///
/// Six. People hum a little fast when they are excited about a record and a
/// little slow when they are trying to remember it, and six is about the
/// spread of the same person humming the same song twice.
pub const TEMPO_SPREAD: f64 = 6.0;

/// What djmanzo made of a hum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    /// The key, when there was enough pitch to tell. `None` is a real answer.
    pub key: Option<MusicalKey>,
    /// Beats per minute, when the hum had a rhythm to it.
    pub tempo: Option<f64>,
    /// How long was listened to.
    pub seconds: f32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HumError {
    #[error("that was too short to read — hum a couple of seconds of it")]
    TooShort,
    #[error("nothing came through the microphone")]
    Silent,
}

/// Read a hum for its key and its tempo.
///
/// `samples` are mono, at `rate`. Mono because a hum is one voice and the
/// interface has no reason to send two copies of it.
///
/// # Errors
/// When the clip is too short to read, or is silence.
pub fn listen(samples: &[f32], rate: SampleRate) -> Result<Heard, HumError> {
    #[allow(clippy::cast_precision_loss)]
    let seconds = samples.len() as f32 / rate.get() as f32;
    if seconds < SHORTEST_HUM {
        return Err(HumError::TooShort);
    }
    // A microphone that was muted, or a permission granted to a device with
    // nothing attached, sends a clip of exact zeros. That is not a quiet hum,
    // it is no hum, and key detection on it would still return something.
    if !samples.iter().any(|s| s.abs() > 1e-4) {
        return Err(HumError::Silent);
    }

    // The analyser reads interleaved stereo, so the one voice is given two
    // channels. Duplicating rather than resampling: the arithmetic downstream
    // averages the pair, and a duplicated mono channel averages to itself.
    let mut stereo = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        stereo.push(*sample);
        stereo.push(*sample);
    }

    let analysis = dj_analysis::analyse(&stereo, rate);
    Ok(Heard {
        key: analysis.key.map(|k| k.key),
        // Only a tempo the analyser is confident enough to sync to. A hum is
        // legato and its onsets are soft, so an unconfident reading here is
        // usually the analyser finding a rhythm in somebody's breathing.
        tempo: analysis
            .tempo
            .filter(|t| t.grid.confidence.is_sync_worthy())
            .map(|t| t.grid.bpm.get()),
        seconds,
    })
}

/// Whether a record's tempo is near enough to a hummed one.
///
/// Half and double count, because somebody humming a 140 BPM record usually
/// hums it at 70 — they are following the vocal, not the kick.
#[must_use]
pub fn near_tempo(hummed: f64, track: f64) -> bool {
    [0.5, 1.0, 2.0]
        .into_iter()
        .any(|factor| (track - hummed * factor).abs() <= TEMPO_SPREAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate() -> SampleRate {
        SampleRate::new(44_100).expect("rate")
    }

    /// A hum: a sine at `hz`, with a little vibrato so it is not a test tone.
    fn hum(hz: f32, seconds: f32) -> Vec<f32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = (seconds * 44_100.0) as usize;
        (0..count)
            .map(|n| {
                #[allow(clippy::cast_precision_loss)]
                let t = n as f32 / 44_100.0;
                let wobble = 1.0 + 0.01 * (t * 5.0 * std::f32::consts::TAU).sin();
                0.4 * (t * hz * wobble * std::f32::consts::TAU).sin()
            })
            .collect()
    }

    /// **A cough is not a hum.**
    #[test]
    fn something_too_short_is_refused_rather_than_read() {
        let clip = hum(220.0, 0.5);
        assert_eq!(listen(&clip, rate()), Err(HumError::TooShort));
    }

    /// **Silence is told apart from a quiet hum.**
    ///
    /// A muted microphone, or a permission granted against a device with
    /// nothing attached, sends exact zeros. Key detection on that still
    /// returns *something*, and a confident answer drawn from silence is the
    /// worst thing this could do.
    #[test]
    fn silence_is_refused() {
        let quiet = vec![0.0f32; 44_100 * 4];
        assert_eq!(listen(&quiet, rate()), Err(HumError::Silent));
        // A genuinely quiet hum is still a hum.
        let faint: Vec<f32> = hum(220.0, 4.0).iter().map(|s| s * 0.01).collect();
        assert!(listen(&faint, rate()).is_ok());
    }

    /// **A hum long enough to read comes back with how long it was.**
    #[test]
    fn a_hum_is_read_and_says_how_long_it_listened() {
        let clip = hum(220.0, 5.0);
        let heard = listen(&clip, rate()).expect("heard");
        assert!((heard.seconds - 5.0).abs() < 0.01, "{}", heard.seconds);
    }

    /// **Humming at half speed still finds the record.**
    ///
    /// The single most common way a hum is wrong: people follow the vocal, and
    /// the vocal is half the tempo of the kick.
    #[test]
    fn half_and_double_time_count_as_near() {
        assert!(near_tempo(128.0, 128.0));
        assert!(near_tempo(64.0, 128.0), "hummed at half speed");
        assert!(near_tempo(256.0, 128.0), "hummed at double speed");
        // And a little either side of each.
        assert!(near_tempo(64.0, 126.0));
        assert!(near_tempo(128.0, 133.0));
    }

    /// **A record at a genuinely different tempo is not near.**
    #[test]
    fn a_different_record_is_not_near() {
        assert!(!near_tempo(128.0, 100.0));
        assert!(!near_tempo(128.0, 140.0));
        // Just outside the spread, on the doubled reading too.
        assert!(!near_tempo(64.0, 121.0));
    }

    /// **The spread is a window, not a direction.**
    #[test]
    fn the_spread_is_even_on_both_sides() {
        let edge = TEMPO_SPREAD;
        assert!(near_tempo(120.0, 120.0 + edge));
        assert!(near_tempo(120.0, 120.0 - edge));
        assert!(!near_tempo(120.0, 120.0 + edge + 0.1));
        assert!(!near_tempo(120.0, 120.0 - edge - 0.1));
    }
}

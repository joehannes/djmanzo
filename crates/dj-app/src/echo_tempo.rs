//! Karaoke: a singer's echo timed to the record they sing over.
//!
//! A strip's echo can be set in beats rather than milliseconds
//! ([`dj_vocal::EchoSettings::beats`]) — half a beat, a dotted eighth — and
//! is then played at the tempo of the record the room is hearing: the deck
//! [`crate::context::carrying`] names, the playing one with its fader
//! highest, at its effective tempo (pitch fader included). The pump calls
//! [`tick`] with each snapshot; when that tempo moves, every strip with a
//! timed echo is sent again, and every other road a strip is sent by —
//! the screen, a preset, a singer put on — goes through
//! `commands::as_played`, which times it with [`timed`].
//!
//! Only what is played is timed. What is kept is the note length and the
//! delay as the host set it, which is what plays when nothing is playing.
//! Nothing here runs on the audio thread: the engine is handed a strip's
//! settings exactly as before, with a delay in it.

use std::sync::{Mutex, OnceLock};

use crate::AppState;

/// A move smaller than this is the pitch fader breathing, not a new tempo:
/// at 120 BPM it shifts half a beat's echo by a millisecond.
const STEADY_BPM: f32 = 0.5;

/// The tempo strips were last timed to.
fn followed() -> &'static Mutex<Option<f32>> {
    static FOLLOWED: OnceLock<Mutex<Option<f32>>> = OnceLock::new();
    FOLLOWED.get_or_init(|| Mutex::new(None))
}

/// The tempo to time an echo to, from a snapshot.
#[must_use]
pub fn tempo_of(snapshot: &crate::snapshot::Snapshot) -> Option<f32> {
    crate::context::carrying(snapshot)
        .and_then(|deck| deck.effective_bpm)
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
}

/// Whether `now` is a different tempo from `then`, for timing an echo.
#[must_use]
pub fn moved(then: Option<f32>, now: Option<f32>) -> bool {
    match (then, now) {
        (Some(a), Some(b)) => (a - b).abs() >= STEADY_BPM,
        (None, None) => false,
        _ => true,
    }
}

/// A strip's settings with its echo timed to the tempo followed now.
#[must_use]
pub fn timed(settings: dj_vocal::StripSettings) -> dj_vocal::StripSettings {
    let bpm = followed().lock().map_or(None, |held| *held);
    timed_at(settings, bpm)
}

/// A strip's settings with its echo timed to `bpm`.
#[must_use]
pub fn timed_at(settings: dj_vocal::StripSettings, bpm: Option<f32>) -> dj_vocal::StripSettings {
    dj_vocal::StripSettings {
        echo: settings.echo.map(|echo| echo.timed(bpm)),
        ..settings
    }
}

/// Follow the record playing: when its tempo moves, send every strip with a
/// timed echo again.
pub fn tick(state: &AppState, snapshot: &crate::snapshot::Snapshot) {
    let now = tempo_of(snapshot);
    {
        let Ok(mut held) = followed().lock() else {
            return;
        };
        if !moved(*held, now) {
            return;
        }
        *held = now;
    }
    for (strip, settings) in crate::commands::played_vocal_settings(state)
        .into_iter()
        .enumerate()
    {
        if settings.echo.is_some_and(|echo| echo.beats.is_some()) {
            // A strip that cannot be sent now is sent with the next move,
            // or the next time anything sends it.
            let _ = crate::commands::send_vocal_strip(state, strip, settings);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A tempo moves when it moves, not when the fader breathes**, and a
    /// record starting or stopping is a move either way.
    #[test]
    fn a_new_tempo_is_one_the_echo_should_hear() {
        assert!(!moved(Some(120.0), Some(120.3)));
        assert!(moved(Some(120.0), Some(120.5)));
        assert!(moved(Some(120.0), Some(96.0)));
        assert!(moved(None, Some(120.0)));
        assert!(moved(Some(120.0), None));
        assert!(!moved(None, None));
    }

    /// **Only the echo is timed, and only one set in beats.**
    #[test]
    fn timing_a_strip_moves_its_echo_and_nothing_else() {
        let strip = dj_vocal::StripSettings {
            echo: Some(dj_vocal::EchoSettings {
                beats: Some(0.75),
                ..dj_vocal::EchoSettings::default()
            }),
            ..dj_vocal::StripSettings::default()
        };
        let played = timed_at(strip, Some(100.0));
        assert!((played.echo.map_or(0.0, |e| e.delay_ms) - 450.0).abs() < 1e-3);
        assert_eq!(
            dj_vocal::StripSettings {
                echo: strip.echo,
                ..played
            },
            strip,
            "something besides the echo's delay changed"
        );
        let plain = dj_vocal::StripSettings::default();
        assert_eq!(timed_at(plain, Some(100.0)), plain);
    }

    /// **Through the app: a timed echo is sent at the record's tempo, and
    /// again when the tempo moves.** A strip set with half a beat goes to
    /// the engine at 300 ms once a record at 100 BPM is playing, and at
    /// 250 ms when it is at 120; a strip with an echo in milliseconds is not
    /// sent again for a tempo.
    #[test]
    fn the_engine_is_sent_the_echo_at_the_records_tempo() {
        let dir = tempfile::tempdir().expect("a folder");
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        let (producer, mut queue) = rtrb::RingBuffer::new(256);
        state.bus().reconnect(producer);
        let mut sent = move || {
            let mut sent = Vec::new();
            while let Ok(command) = queue.pop() {
                if let dj_engine::Command::VocalStrip { strip, settings } = command {
                    sent.push((strip, settings.echo.map(|e| e.delay_ms)));
                }
            }
            sent
        };
        let timed_strip = dj_vocal::StripSettings {
            echo: Some(dj_vocal::EchoSettings {
                beats: Some(0.5),
                ..dj_vocal::EchoSettings::default()
            }),
            ..dj_vocal::StripSettings::default()
        };
        let plain = dj_vocal::StripSettings {
            echo: Some(dj_vocal::EchoSettings::default()),
            ..dj_vocal::StripSettings::default()
        };
        crate::commands::set_vocal_strip(&state, 0, timed_strip).expect("strip 1");
        crate::commands::set_vocal_strip(&state, 1, plain).expect("strip 2");
        sent();

        let playing_at = |bpm: f32| {
            let registry = dj_control::ParameterRegistry::new();
            let mut snapshot = crate::snapshot::Snapshot::capture(&registry, 2);
            snapshot.decks[0].playing = true;
            snapshot.decks[0].loaded = true;
            snapshot.decks[0].volume = 1.0;
            snapshot.decks[0].effective_bpm = Some(bpm);
            snapshot
        };
        tick(&state, &playing_at(100.0));
        let at_100 = sent();
        assert_eq!(at_100.len(), 1, "only the timed strip is sent: {at_100:?}");
        assert_eq!(at_100[0].0, 0);
        assert!(
            (at_100[0].1.unwrap_or(0.0) - 300.0).abs() < 1e-3,
            "{at_100:?}"
        );

        // The fader breathing is not a new tempo; a new record is.
        tick(&state, &playing_at(100.2));
        assert!(sent().is_empty());
        tick(&state, &playing_at(120.0));
        let at_120 = sent();
        assert!(
            (at_120[0].1.unwrap_or(0.0) - 250.0).abs() < 1e-3,
            "{at_120:?}"
        );

        // And anything else that sends the strip sends it timed.
        crate::commands::set_vocal_strip(&state, 0, timed_strip).expect("strip 1");
        let again = sent();
        assert!(
            (again[0].1.unwrap_or(0.0) - 250.0).abs() < 1e-3,
            "{again:?}"
        );
        *followed().lock().expect("the tempo") = None;
    }
}

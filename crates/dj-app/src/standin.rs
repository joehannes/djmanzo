//! §106 in the running application: the stand-in turntable and controller,
//! opened from the environment.
//!
//! `dj_dvs::turntable` and `dj_hid::standin` were written for tests, where a
//! decoder or a mapping is held to what a scripted hand did. This puts them
//! where a person can look at them: a deck following a control record nobody
//! is playing, in the timecode panel, and a "controller" open in the
//! controllers panel with §53's reach read off it. A development affordance,
//! like `DJMANZO_DEMO`, and never set in a normal session.
//!
//! - `DJMANZO_STANDIN_TIMECODE=<deck>` — once an output is open, the null
//!   inputs play the stand-in deck's record (`dj_audio::null::play_into_inputs`)
//!   and that deck is put on it through `timecode::start`, the door the panel
//!   uses. Only with the null backend: on a machine with a sound card the
//!   inputs are real and nothing here can play into them.
//! - `DJMANZO_STANDIN_CONTROLLER=<mapping file stem>` — the stand-in is opened
//!   in place of a port (`ControlHub::open_standin`), and the panel offers to
//!   press every control of it.
//!
//! # What this cannot tell you
//!
//! Everything `dj_dvs::turntable` and `dj_hid::standin` say they cannot: not
//! whether a pressed record, a real cartridge or a real controller behaves
//! like the stand-in. What it shows is the path from those to the screen.

use crate::state::AppState;
use dj_dvs::turntable::{Deck, Move, Turntable};

/// The deck the stand-in turntable plays into, written 1..=4.
pub const TIMECODE_ENV: &str = "DJMANZO_STANDIN_TIMECODE";

/// The mapping the stand-in controller plays, by its file stem.
pub const CONTROLLER_ENV: &str = "DJMANZO_STANDIN_CONTROLLER";

/// How long the stand-in waits for an output before giving up. The interface
/// opens one as it starts; a minute is far longer than that ever takes.
const WAIT_FOR_OUTPUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The hand on the stand-in turntable: about twenty-five seconds of what a DJ
/// does to a record, played round and round. Playing, a brake to a stop and a
/// start again, a baby scratch, a spin back, the needle lifted and dropped
/// further in, and the pitch pushed up eight per cent.
#[must_use]
pub fn hand() -> Vec<Move> {
    vec![
        Move::Play {
            seconds: 6.0,
            speed: 1.0,
        },
        Move::Ramp {
            seconds: 0.5,
            from: 1.0,
            to: 0.0,
        },
        Move::Play {
            seconds: 1.5,
            speed: 0.0,
        },
        Move::Ramp {
            seconds: 0.4,
            from: 0.0,
            to: 1.0,
        },
        Move::Play {
            seconds: 4.0,
            speed: 1.0,
        },
        Move::Scratch {
            seconds: 2.5,
            depth: 1.5,
            rate: 2.0,
        },
        Move::Play {
            seconds: 3.0,
            speed: 1.0,
        },
        Move::Ramp {
            seconds: 0.15,
            from: 1.0,
            to: -3.0,
        },
        Move::Ramp {
            seconds: 1.2,
            from: -3.0,
            to: 0.0,
        },
        Move::Lift { seconds: 1.5 },
        Move::Drop { at: 40.0 },
        Move::Play {
            seconds: 4.0,
            speed: 1.08,
        },
        Move::Ramp {
            seconds: 0.5,
            from: 1.08,
            to: 1.0,
        },
    ]
}

/// What the stand-in deck puts out under [`hand`], interleaved stereo at
/// `sample_rate`: a working deck through a line-level preamp, by
/// `Deck::WORKING`'s textbook figures. `None` for a format that describes no
/// record.
#[must_use]
pub fn record(format: dj_dvs::TimecodeFormat, sample_rate: f64) -> Option<Vec<f32>> {
    let synth = dj_dvs::Synth::new(format, sample_rate)?;
    let (audio, _) = Turntable::new(synth, Deck::WORKING, sample_rate).perform(0.0, &hand());
    Some(audio)
}

/// The deck the environment names, if it names one.
#[must_use]
pub fn deck_of(value: &str) -> Option<u8> {
    let deck: u8 = value.trim().parse().ok()?;
    dj_core::DeckId::from_human(deck).map(|_| deck)
}

/// Open whichever stand-ins the environment asks for. Called once from setup.
pub fn open(handle: &tauri::AppHandle) {
    use tauri::Manager;
    if let Ok(stem) = std::env::var(CONTROLLER_ENV) {
        let state = handle.state::<AppState>();
        match state.control().open_standin(&stem) {
            Ok(()) => tracing::info!(%stem, "the stand-in controller is open"),
            Err(error) => tracing::warn!(%error, "the stand-in controller did not open"),
        }
    }
    let Ok(value) = std::env::var(TIMECODE_ENV) else {
        return;
    };
    let Some(deck) = deck_of(&value) else {
        tracing::warn!(%value, "{TIMECODE_ENV} names no deck");
        return;
    };
    if std::env::var(crate::NULL_BACKEND_ENV).is_err() {
        tracing::warn!(
            "{TIMECODE_ENV} needs {}: only the null inputs can play the stand-in record",
            crate::NULL_BACKEND_ENV
        );
        return;
    }
    let handle = handle.clone();
    let spawned = std::thread::Builder::new()
        .name("djmanzo-standin-timecode".into())
        .spawn(move || {
            let state = handle.state::<AppState>();
            if let Err(error) = put_on_record(&state, deck) {
                tracing::warn!(%error, "the stand-in turntable did not start");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the stand-in turntable did not start");
    }
}

/// §106: press every control of the stand-in controller, and say what it
/// reached.
///
/// # Errors
/// When no stand-in controller is open.
#[tauri::command]
pub fn controller_standin_press(
    state: tauri::State<'_, AppState>,
) -> Result<crate::control::StandInRun, String> {
    state.control().press_standin()
}

/// Wait for an output, then play the stand-in record into the null inputs at
/// its rate and put `deck` on it.
fn put_on_record(state: &AppState, deck: u8) -> Result<(), String> {
    let started = std::time::Instant::now();
    let rate = loop {
        if let Some(device) = state.active_device() {
            break device.sample_rate;
        }
        if started.elapsed() > WAIT_FOR_OUTPUT {
            return Err("no output opened to attach a deck to".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    let format = crate::timecode::pick_format(None)?;
    let samples = record(format.clone(), f64::from(rate))
        .ok_or_else(|| format!("{} describes no record", format.name))?;
    let sample_rate = dj_core::SampleRate::new(rate).ok_or("the output has no rate")?;
    dj_audio::null::play_into_inputs(samples, 2, sample_rate);
    crate::timecode::start(state, deck, None, Some(format.name.clone()), Some(false))?;
    tracing::info!(deck, format = %format.name, "deck is on the stand-in turntable");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The record the running application plays is one the decoder follows
    /// through the hand's moves.** Fed in callback-sized blocks as the
    /// engine feeds it: playing reads as playing, the brake as stopped, and
    /// the pushed pitch as eight per cent fast. A record the decoder could not
    /// read would leave the timecode panel at no signal and look exactly like
    /// a broken stand-in.
    #[test]
    fn the_stand_in_record_is_followed_through_the_hands_moves() {
        const RATE: f64 = 48_000.0;
        let format = crate::timecode::pick_format(None).expect("a format");
        let audio = record(format.clone(), RATE).expect("a record");
        let mut decoder = dj_dvs::Decoder::new(format, RATE).expect("a decoder");
        let mut readings = Vec::new();
        for block in audio.chunks(512) {
            readings.push(decoder.feed(block));
        }
        // A reading at `seconds` into the hand: blocks are 256 frames.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let at = |seconds: f64| readings[(seconds * RATE / 256.0) as usize];

        let playing = at(4.0);
        assert!((playing.speed - 1.0).abs() < 0.05, "{playing:?}");
        assert!(playing.quality > 0.5, "{playing:?}");
        let stopped = at(7.5);
        assert!(stopped.speed.abs() < 0.05, "{stopped:?}");
        // Dropped forty seconds in, then pushed up: 6 + 0.5 + 1.5 + 0.4 + 4 +
        // 2.5 + 3 + 0.15 + 1.2 + 1.5 = 20.75 s to the drop.
        let pushed = at(23.0);
        assert!((pushed.speed - 1.08).abs() < 0.05, "{pushed:?}");
    }

    #[test]
    fn the_environment_names_a_deck_or_none() {
        assert_eq!(deck_of("1"), Some(1));
        assert_eq!(deck_of(" 4 "), Some(4));
        assert_eq!(deck_of("0"), None);
        assert_eq!(deck_of("9"), None);
        assert_eq!(deck_of("left"), None);
    }
}

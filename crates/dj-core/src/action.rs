//! The action vocabulary -- every intent the application can express.
//!
//! See `docs/adr/0003-action-bus-and-parameter-registry.md`. The rule this file
//! exists to enforce: a MIDI note, a click, a script line and a network command
//! all become the *same* value here, so there is one execution path rather than
//! one per input source.
//!
//! Every action also has a text form (`deck 2 play`) in the spirit of VirtualDJ
//! script. Text is parsed at the edge into the enum below; nothing downstream
//! ever sees a string.

use crate::deck::DeckId;
use crate::hotcue::HOT_CUE_SLOTS;
use crate::time::{FramePos, Rate};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A single user intent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Deck { deck: DeckId, action: DeckAction },
    Mixer(MixerAction),
}

/// Something done to one deck.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DeckAction {
    Play,
    Pause,
    /// Play if paused, pause if playing -- what a controller's play button sends.
    PlayPause,
    /// Return to the cue point and stop. CDJ behaviour.
    Cue,
    /// Jump the playhead. Absolute, in frames.
    Seek(FramePos),
    /// Set playback speed directly. Used by jog wheels and, later, timecode.
    SetRate(Rate),
    /// Pitch fader, as a fraction: 0.0 is centre, +0.08 is +8%.
    SetPitch(f64),
    /// Channel fader, 0.0..=1.0.
    SetVolume(f32),
    /// Trim/gain in decibels.
    SetGainDb(f32),
    /// Low band of the isolator EQ, as a linear gain. 0.0 kills the band.
    SetEqLow(f32),
    /// Mid band of the isolator EQ.
    SetEqMid(f32),
    /// High band of the isolator EQ.
    SetEqHigh(f32),
    /// Filter sweep: -1.0 fully low-passed, 0.0 off, +1.0 fully high-passed.
    SetFilter(f32),
    /// Pre-fader listen: send this deck to the headphones.
    SetCue(bool),
    /// Flip the headphone cue send.
    ToggleCue,
    /// Keylock: hold the musical key while the pitch fader changes tempo.
    SetKeylock(bool),
    /// Flip keylock.
    ToggleKeylock,
    /// Transpose in semitones for harmonic mixing, independent of tempo.
    SetKeyShift(i32),
    /// Match this deck's tempo, and align its phase once, to another playing
    /// deck. Refused when either grid is too weak to trust.
    Sync,
    /// Release the tempo lock; the pitch fader is the DJ's again.
    SyncOff,
    /// Move the playhead by whole beats. Negative goes back.
    BeatJump(i32),

    /// One-button hot cue, 1-based: jump to it if set, set it here if not.
    ///
    /// The behaviour every controller's pads send, and the reason it is one
    /// verb rather than two: a pad has one message, and a DJ reaching for a pad
    /// mid-set does not want to think about which mode it is in.
    HotCue(u8),
    /// Set (or overwrite) a hot cue at the playhead.
    HotCueSet(u8),
    /// Forget a hot cue.
    HotCueClear(u8),

    /// Loop the next `n` beats from here, and start looping.
    ///
    /// Zero or negative turns looping off, so a controller encoder that can
    /// reach zero does the obvious thing instead of an error.
    LoopBeats(i32),
    /// Stop looping and carry on from where the playhead is.
    LoopOff,
    /// Halve the loop, keeping its start. Repeatable down to a fraction of a beat.
    LoopHalve,
    /// Double the loop, keeping its start.
    LoopDouble,
    /// Drop the loop's in point at the playhead. Manual looping, half one.
    LoopIn,
    /// Drop the out point and start looping. Manual looping, half two.
    LoopOut,
    /// Slide the whole loop by whole beats, keeping its length.
    LoopMove(i32),
    /// Drop the loaded track.
    Eject,
}

/// Something done to the mixer as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MixerAction {
    /// -1.0 is hard left, 0.0 centre, +1.0 hard right.
    Crossfader(f32),
    MasterGainDb(f32),
    /// Headphone blend: 0.0 is all cue, 1.0 is all master.
    CueMix(f32),
    /// Split cue: cue in one ear, master in the other.
    SplitCue(bool),
    BoothGainDb(f32),
    /// Snap beat jumps -- and later cues and loops -- to the grid.
    ///
    /// Global rather than per-deck because it is a way of *working*, not a
    /// property of a track: a DJ who wants quantised jumps wants them on
    /// whichever deck they happen to be touching.
    SetQuantize(bool),
    /// Engage or bypass the master limiter.
    ///
    /// On by default. Bypassing is for the DJ feeding an external processor
    /// that is already doing the job — two limiters in series is worse than
    /// one, and the second one has no way to know that.
    SetLimiter(bool),
}

impl Action {
    /// Parse the text form. Case-insensitive, whitespace-separated.
    ///
    /// ```
    /// # use dj_core::action::{Action, DeckAction};
    /// # use dj_core::deck::DeckId;
    /// let a = Action::parse("deck 2 play").unwrap();
    /// assert_eq!(a, Action::Deck {
    ///     deck: DeckId::from_human(2).unwrap(),
    ///     action: DeckAction::Play,
    /// });
    /// ```
    pub fn parse(input: &str) -> Result<Action, ParseError> {
        let lowered = input.trim().to_ascii_lowercase();
        let mut words = lowered.split_whitespace();

        match words.next().ok_or(ParseError::Empty)? {
            "deck" => {
                let number: u8 = words
                    .next()
                    .ok_or(ParseError::MissingDeckNumber)?
                    .parse()
                    .map_err(|_| ParseError::BadDeckNumber)?;
                let deck = DeckId::from_human(number).ok_or(ParseError::BadDeckNumber)?;
                let verb = words.next().ok_or(ParseError::MissingVerb)?;
                let action = parse_deck_verb(verb, words.next())?;
                Ok(Action::Deck { deck, action })
            }
            "crossfader" => {
                let value = parse_f32(words.next())?;
                Ok(Action::Mixer(MixerAction::Crossfader(
                    value.clamp(-1.0, 1.0),
                )))
            }
            "master" => match words.next().ok_or(ParseError::MissingVerb)? {
                "gain" => Ok(Action::Mixer(MixerAction::MasterGainDb(parse_f32(
                    words.next(),
                )?))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            "booth" => match words.next().ok_or(ParseError::MissingVerb)? {
                "gain" => Ok(Action::Mixer(MixerAction::BoothGainDb(parse_f32(
                    words.next(),
                )?))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            "quantize" => match words.next().ok_or(ParseError::MissingVerb)? {
                "on" => Ok(Action::Mixer(MixerAction::SetQuantize(true))),
                "off" => Ok(Action::Mixer(MixerAction::SetQuantize(false))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            "limiter" => match words.next().ok_or(ParseError::MissingVerb)? {
                "on" => Ok(Action::Mixer(MixerAction::SetLimiter(true))),
                "off" => Ok(Action::Mixer(MixerAction::SetLimiter(false))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            "cue" => match words.next().ok_or(ParseError::MissingVerb)? {
                "mix" => Ok(Action::Mixer(MixerAction::CueMix(
                    parse_f32(words.next())?.clamp(0.0, 1.0),
                ))),
                "split_on" => Ok(Action::Mixer(MixerAction::SplitCue(true))),
                "split_off" => Ok(Action::Mixer(MixerAction::SplitCue(false))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            other => Err(ParseError::UnknownTarget(other.to_owned())),
        }
    }
}

fn parse_deck_verb(verb: &str, argument: Option<&str>) -> Result<DeckAction, ParseError> {
    Ok(match verb {
        "play" => DeckAction::Play,
        "pause" => DeckAction::Pause,
        "play_pause" | "playpause" => DeckAction::PlayPause,
        "cue" => DeckAction::Cue,
        "eject" => DeckAction::Eject,
        "seek" => DeckAction::Seek(FramePos::new(f64::from(parse_f32(argument)?))),
        "rate" => DeckAction::SetRate(Rate::new(f64::from(parse_f32(argument)?))),
        "pitch" => DeckAction::SetPitch(f64::from(parse_f32(argument)?)),
        "volume" => DeckAction::SetVolume(parse_f32(argument)?.clamp(0.0, 1.0)),
        "gain" => DeckAction::SetGainDb(parse_f32(argument)?),
        "eq_low" => DeckAction::SetEqLow(parse_f32(argument)?.clamp(0.0, 4.0)),
        "eq_mid" => DeckAction::SetEqMid(parse_f32(argument)?.clamp(0.0, 4.0)),
        "eq_high" => DeckAction::SetEqHigh(parse_f32(argument)?.clamp(0.0, 4.0)),
        "filter" => DeckAction::SetFilter(parse_f32(argument)?.clamp(-1.0, 1.0)),
        "cue_on" => DeckAction::SetCue(true),
        "cue_off" => DeckAction::SetCue(false),
        "cue_toggle" => DeckAction::ToggleCue,
        "sync" => DeckAction::Sync,
        "sync_off" => DeckAction::SyncOff,
        "beatjump" => DeckAction::BeatJump(parse_i32(argument)?),
        "hotcue" => DeckAction::HotCue(parse_slot(argument)?),
        "hotcue_set" => DeckAction::HotCueSet(parse_slot(argument)?),
        "hotcue_clear" => DeckAction::HotCueClear(parse_slot(argument)?),
        "loop" => DeckAction::LoopBeats(parse_i32(argument)?),
        "loop_off" => DeckAction::LoopOff,
        "loop_halve" => DeckAction::LoopHalve,
        "loop_double" => DeckAction::LoopDouble,
        "loop_in" => DeckAction::LoopIn,
        "loop_out" => DeckAction::LoopOut,
        "loop_move" => DeckAction::LoopMove(parse_i32(argument)?),
        "keylock_on" => DeckAction::SetKeylock(true),
        "keylock_off" => DeckAction::SetKeylock(false),
        "keylock_toggle" => DeckAction::ToggleKeylock,
        "key" => DeckAction::SetKeyShift(parse_f32(argument)?.round() as i32),
        other => return Err(ParseError::UnknownVerb(other.to_owned())),
    })
}

/// Whole beats, for beat jump.
///
/// Parsed as an integer rather than through `parse_f32`, because half a beat is
/// not a beat jump -- it is a seek, and there is a verb for that.
fn parse_i32(word: Option<&str>) -> Result<i32, ParseError> {
    word.ok_or(ParseError::MissingArgument)?
        .parse()
        .map_err(|_| ParseError::BadArgument)
}

/// A hot cue slot, 1-based as the interface and every controller number them.
///
/// Rejected rather than clamped: slot 0 or slot 99 is a mistake somewhere
/// upstream, and silently firing slot 1 instead would hide it.
fn parse_slot(word: Option<&str>) -> Result<u8, ParseError> {
    let slot: u8 = word
        .ok_or(ParseError::MissingArgument)?
        .parse()
        .map_err(|_| ParseError::BadArgument)?;
    if (1..=HOT_CUE_SLOTS as u8).contains(&slot) {
        Ok(slot)
    } else {
        Err(ParseError::BadArgument)
    }
}

fn parse_f32(word: Option<&str>) -> Result<f32, ParseError> {
    let word = word.ok_or(ParseError::MissingArgument)?;
    let value: f32 = word.parse().map_err(|_| ParseError::BadArgument)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ParseError::BadArgument)
    }
}

/// Format a number for the text form.
///
/// Every numeric argument arrives through `parse_f32`, so an `f64` field here
/// is always a widened `f32` — and printing one of those directly gives
/// `0.03999999910593033` where the user typed `0.04`. That is technically the
/// value and practically unreadable, and it lands in the session log, in the
/// interface, and in anything the assistant echoes back.
///
/// Six decimal places is more than an `f32` carries, so nothing is lost;
/// trimming the zeros keeps whole numbers whole.
fn number(value: f64) -> String {
    if !value.is_finite() {
        // Cannot round-trip, and should never reach here: the parser rejects
        // non-finite input. Emitting `0` keeps the output parseable rather than
        // producing `NaN`, which would fail to re-parse.
        return "0".to_owned();
    }
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Deck { deck, action } => match action {
                DeckAction::Play => write!(f, "deck {deck} play"),
                DeckAction::Pause => write!(f, "deck {deck} pause"),
                DeckAction::PlayPause => write!(f, "deck {deck} play_pause"),
                DeckAction::Cue => write!(f, "deck {deck} cue"),
                DeckAction::Eject => write!(f, "deck {deck} eject"),
                DeckAction::Seek(p) => write!(f, "deck {deck} seek {}", number(p.get())),
                DeckAction::SetRate(r) => write!(f, "deck {deck} rate {}", number(r.get())),
                DeckAction::SetPitch(p) => write!(f, "deck {deck} pitch {}", number(*p)),
                DeckAction::SetVolume(v) => {
                    write!(f, "deck {deck} volume {}", number(f64::from(*v)))
                }
                DeckAction::SetGainDb(g) => write!(f, "deck {deck} gain {}", number(f64::from(*g))),
                DeckAction::SetEqLow(v) => {
                    write!(f, "deck {deck} eq_low {}", number(f64::from(*v)))
                }
                DeckAction::SetEqMid(v) => {
                    write!(f, "deck {deck} eq_mid {}", number(f64::from(*v)))
                }
                DeckAction::SetEqHigh(v) => {
                    write!(f, "deck {deck} eq_high {}", number(f64::from(*v)))
                }
                DeckAction::SetFilter(v) => {
                    write!(f, "deck {deck} filter {}", number(f64::from(*v)))
                }
                DeckAction::SetCue(true) => write!(f, "deck {deck} cue_on"),
                DeckAction::SetCue(false) => write!(f, "deck {deck} cue_off"),
                DeckAction::ToggleCue => write!(f, "deck {deck} cue_toggle"),
                DeckAction::SetKeylock(true) => write!(f, "deck {deck} keylock_on"),
                DeckAction::SetKeylock(false) => write!(f, "deck {deck} keylock_off"),
                DeckAction::ToggleKeylock => write!(f, "deck {deck} keylock_toggle"),
                DeckAction::SetKeyShift(n) => write!(f, "deck {deck} key {n}"),
                DeckAction::Sync => write!(f, "deck {deck} sync"),
                DeckAction::SyncOff => write!(f, "deck {deck} sync_off"),
                DeckAction::BeatJump(n) => write!(f, "deck {deck} beatjump {n}"),
                DeckAction::HotCue(n) => write!(f, "deck {deck} hotcue {n}"),
                DeckAction::HotCueSet(n) => write!(f, "deck {deck} hotcue_set {n}"),
                DeckAction::HotCueClear(n) => write!(f, "deck {deck} hotcue_clear {n}"),
                DeckAction::LoopBeats(n) => write!(f, "deck {deck} loop {n}"),
                DeckAction::LoopOff => write!(f, "deck {deck} loop_off"),
                DeckAction::LoopHalve => write!(f, "deck {deck} loop_halve"),
                DeckAction::LoopDouble => write!(f, "deck {deck} loop_double"),
                DeckAction::LoopIn => write!(f, "deck {deck} loop_in"),
                DeckAction::LoopOut => write!(f, "deck {deck} loop_out"),
                DeckAction::LoopMove(n) => write!(f, "deck {deck} loop_move {n}"),
            },
            Action::Mixer(MixerAction::Crossfader(v)) => {
                write!(f, "crossfader {}", number(f64::from(*v)))
            }
            Action::Mixer(MixerAction::MasterGainDb(v)) => {
                write!(f, "master gain {}", number(f64::from(*v)))
            }
            Action::Mixer(MixerAction::BoothGainDb(v)) => {
                write!(f, "booth gain {}", number(f64::from(*v)))
            }
            Action::Mixer(MixerAction::CueMix(v)) => write!(f, "cue mix {}", number(f64::from(*v))),
            Action::Mixer(MixerAction::SplitCue(true)) => write!(f, "cue split_on"),
            Action::Mixer(MixerAction::SplitCue(false)) => write!(f, "cue split_off"),
            Action::Mixer(MixerAction::SetQuantize(true)) => write!(f, "quantize on"),
            Action::Mixer(MixerAction::SetQuantize(false)) => write!(f, "quantize off"),
            Action::Mixer(MixerAction::SetLimiter(true)) => write!(f, "limiter on"),
            Action::Mixer(MixerAction::SetLimiter(false)) => write!(f, "limiter off"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("empty action")]
    Empty,
    #[error("unknown target `{0}`")]
    UnknownTarget(String),
    #[error("unknown verb `{0}`")]
    UnknownVerb(String),
    #[error("expected a deck number")]
    MissingDeckNumber,
    #[error("deck number out of range")]
    BadDeckNumber,
    #[error("expected a verb")]
    MissingVerb,
    #[error("expected an argument")]
    MissingArgument,
    #[error("argument is not a finite number")]
    BadArgument,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    #[test]
    fn parses_bare_verbs() {
        assert_eq!(
            Action::parse("deck 1 play").unwrap(),
            Action::Deck {
                deck: deck(1),
                action: DeckAction::Play
            }
        );
        assert_eq!(
            Action::parse("deck 4 cue").unwrap(),
            Action::Deck {
                deck: deck(4),
                action: DeckAction::Cue
            }
        );
    }

    #[test]
    fn parses_verbs_with_arguments() {
        assert_eq!(
            Action::parse("deck 2 volume 0.75").unwrap(),
            Action::Deck {
                deck: deck(2),
                action: DeckAction::SetVolume(0.75)
            }
        );
        assert_eq!(
            Action::parse("crossfader -1").unwrap(),
            Action::Mixer(MixerAction::Crossfader(-1.0))
        );
    }

    #[test]
    fn parsing_is_case_and_space_insensitive() {
        assert_eq!(
            Action::parse("  DECK 1   PLAY  ").unwrap(),
            Action::parse("deck 1 play").unwrap()
        );
    }

    #[test]
    fn out_of_range_values_are_clamped_not_rejected() {
        // A controller sending a slightly out-of-range value should not fail the
        // whole action; the sane response is to clamp.
        assert_eq!(
            Action::parse("deck 1 volume 5").unwrap(),
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetVolume(1.0)
            }
        );
        assert_eq!(
            Action::parse("crossfader 99").unwrap(),
            Action::Mixer(MixerAction::Crossfader(1.0))
        );
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(Action::parse(""), Err(ParseError::Empty));
        assert_eq!(Action::parse("deck"), Err(ParseError::MissingDeckNumber));
        assert_eq!(Action::parse("deck x play"), Err(ParseError::BadDeckNumber));
        assert_eq!(Action::parse("deck 0 play"), Err(ParseError::BadDeckNumber));
        assert_eq!(
            Action::parse("deck 99 play"),
            Err(ParseError::BadDeckNumber)
        );
        assert_eq!(Action::parse("deck 1"), Err(ParseError::MissingVerb));
        assert_eq!(
            Action::parse("deck 1 volume"),
            Err(ParseError::MissingArgument)
        );
        assert_eq!(
            Action::parse("deck 1 volume abc"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(
            Action::parse("deck 1 volume nan"),
            Err(ParseError::BadArgument)
        );
        assert!(matches!(
            Action::parse("teleport 1 play"),
            Err(ParseError::UnknownTarget(_))
        ));
        assert!(matches!(
            Action::parse("deck 1 levitate"),
            Err(ParseError::UnknownVerb(_))
        ));
    }

    /// Numbers arrive as `f32` and are stored widened, so printing one raw
    /// gives `0.03999999910593033` where the user typed `0.04`. That string
    /// lands in the session log, in the interface, and in anything the
    /// assistant echoes back.
    #[test]
    fn numbers_print_the_way_they_were_typed() {
        let cases = [
            ("deck 1 pitch 0.04", "deck 1 pitch 0.04"),
            ("deck 1 pitch -0.08", "deck 1 pitch -0.08"),
            ("deck 1 volume 0.8", "deck 1 volume 0.8"),
            ("deck 1 seek 48000", "deck 1 seek 48000"),
            ("deck 1 rate 1", "deck 1 rate 1"),
            ("deck 1 eq_low 0", "deck 1 eq_low 0"),
            ("crossfader -1", "crossfader -1"),
            ("cue mix 0.35", "cue mix 0.35"),
            ("master gain -3.5", "master gain -3.5"),
        ];
        for (input, expected) in cases {
            let rendered = Action::parse(input).unwrap().to_string();
            assert_eq!(rendered, expected, "`{input}` printed as `{rendered}`");
        }
    }

    /// Formatting must not cost a round trip: what is printed has to parse back
    /// to the same value, or the session log would replay differently.
    #[test]
    fn formatting_never_breaks_the_round_trip() {
        for input in [
            "deck 1 pitch 0.0833",
            "deck 1 gain -12.25",
            "deck 1 filter -0.333333",
            "deck 1 seek 1234567",
            "cue mix 0.123456",
        ] {
            let action = Action::parse(input).unwrap();
            let reparsed = Action::parse(&action.to_string()).unwrap();
            assert_eq!(reparsed, action, "`{input}` did not survive formatting");
        }
    }

    /// The text form is an API surface (scripts, OSC, WebSocket), so it has to
    /// survive a round trip or those clients will silently drift from the enum.
    ///
    /// The list is maintained by hand, so **add a case whenever you add a
    /// variant**. A missing one is silent: the action still works from the
    /// interface and fails only from a script, a mapping file or the assistant,
    /// which is the worst place to find out.
    #[test]
    fn text_form_round_trips() {
        let cases = [
            Action::Deck {
                deck: deck(1),
                action: DeckAction::Play,
            },
            Action::Deck {
                deck: deck(3),
                action: DeckAction::PlayPause,
            },
            Action::Deck {
                deck: deck(2),
                action: DeckAction::SetVolume(0.5),
            },
            Action::Deck {
                deck: deck(6),
                action: DeckAction::Seek(FramePos::new(1234.0)),
            },
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetRate(Rate::new(1.5)),
            },
            Action::Deck {
                deck: deck(2),
                action: DeckAction::SetEqLow(0.0),
            },
            Action::Deck {
                deck: deck(3),
                action: DeckAction::SetEqMid(1.5),
            },
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetEqHigh(0.25),
            },
            Action::Deck {
                deck: deck(4),
                action: DeckAction::SetFilter(-0.75),
            },
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetCue(true),
            },
            Action::Deck {
                deck: deck(2),
                action: DeckAction::ToggleCue,
            },
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetKeylock(true),
            },
            Action::Deck {
                deck: deck(2),
                action: DeckAction::SetKeylock(false),
            },
            Action::Deck {
                deck: deck(4),
                action: DeckAction::ToggleKeylock,
            },
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetKeyShift(2),
            },
            Action::Deck {
                deck: deck(2),
                action: DeckAction::SetKeyShift(-5),
            },
            Action::Mixer(MixerAction::CueMix(0.35)),
            Action::Mixer(MixerAction::SplitCue(true)),
            Action::Mixer(MixerAction::BoothGainDb(-6.0)),
            Action::Mixer(MixerAction::Crossfader(-0.25)),
            Action::Mixer(MixerAction::MasterGainDb(-3.0)),
        ];
        for action in cases {
            let text = action.to_string();
            let parsed = Action::parse(&text)
                .unwrap_or_else(|e| panic!("`{text}` failed to round trip: {e}"));
            assert_eq!(
                parsed, action,
                "`{text}` round-tripped to a different action"
            );
        }
    }
}

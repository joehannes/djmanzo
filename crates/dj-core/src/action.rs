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
    /// Drop the loaded track.
    Eject,
}

/// Something done to the mixer as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MixerAction {
    /// -1.0 is hard left, 0.0 centre, +1.0 hard right.
    Crossfader(f32),
    MasterGainDb(f32),
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
        other => return Err(ParseError::UnknownVerb(other.to_owned())),
    })
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

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Deck { deck, action } => match action {
                DeckAction::Play => write!(f, "deck {deck} play"),
                DeckAction::Pause => write!(f, "deck {deck} pause"),
                DeckAction::PlayPause => write!(f, "deck {deck} play_pause"),
                DeckAction::Cue => write!(f, "deck {deck} cue"),
                DeckAction::Eject => write!(f, "deck {deck} eject"),
                DeckAction::Seek(p) => write!(f, "deck {deck} seek {}", p.get()),
                DeckAction::SetRate(r) => write!(f, "deck {deck} rate {}", r.get()),
                DeckAction::SetPitch(p) => write!(f, "deck {deck} pitch {p}"),
                DeckAction::SetVolume(v) => write!(f, "deck {deck} volume {v}"),
                DeckAction::SetGainDb(g) => write!(f, "deck {deck} gain {g}"),
            },
            Action::Mixer(MixerAction::Crossfader(v)) => write!(f, "crossfader {v}"),
            Action::Mixer(MixerAction::MasterGainDb(v)) => write!(f, "master gain {v}"),
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

    /// The text form is an API surface (scripts, OSC, WebSocket), so it has to
    /// survive a round trip or those clients will silently drift from the enum.
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

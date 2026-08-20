//! The action vocabulary, described.
//!
//! [`action`](crate::action) defines what an action *is*. This module describes
//! what actions *exist*, in a form a machine can read: one entry per verb, with
//! its argument shape, what it does, and an example that parses.
//!
//! # Why this exists
//!
//! [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md) requires
//! that tool schemas exposed to a language model be **generated from the action
//! vocabulary**, so a model can only be told about actions that actually exist.
//! Without a machine-readable list, that schema would be hand-written somewhere
//! in the assistant, and would drift the first time a verb was added or renamed
//! — producing a model confidently emitting commands the parser rejects.
//!
//! The drift is prevented by test rather than by discipline:
//! [`every_example_parses`](#) runs every example through
//! [`Action::parse`](crate::action::Action::parse), so an entry that describes
//! something the parser does not accept fails the build.
//!
//! This is also the natural source for `--help`, for scripting documentation,
//! and for the network API's self-description. One list, several readers.

use serde::Serialize;

/// What an action is aimed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// Addressed as `deck <n> <verb>`.
    Deck,
    /// Addressed as `<prefix> <verb>` or just `<verb>`.
    Mixer,
}

/// The shape of a verb's argument.
///
/// Ranges are the *accepted* range; values outside are clamped rather than
/// rejected, which is what keeps a confused model from being able to do
/// anything worse than something mild.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArgSpec {
    /// No argument. `deck 1 play`.
    None,
    /// A number within a range.
    Number { min: f32, max: f32 },
    /// A position in frames. Clamped to the track length at execution.
    Frames,
    /// A small sub-grammar of its own: several words, described by the help
    /// line rather than by a range. Used where the alternative is multiplying
    /// one verb out into one per thing it can address — see `fx`, where that
    /// would have meant thirty-six verbs.
    Words,
}

impl ArgSpec {
    #[must_use]
    pub const fn takes_argument(self) -> bool {
        !matches!(self, ArgSpec::None)
    }
}

/// One verb, fully described.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct VerbSpec {
    pub target: Target,
    /// The word itself, as typed: `play`, `eq_low`, `keylock_toggle`.
    pub verb: &'static str,
    pub argument: ArgSpec,
    /// One line, in the imperative. Read by models and by humans.
    pub help: &'static str,
    /// A complete action string that parses. Asserted by test.
    pub example: &'static str,
}

/// Every verb the parser accepts.
#[must_use]
pub fn vocabulary() -> &'static [VerbSpec] {
    &VOCABULARY
}

/// Look one up.
#[must_use]
pub fn verb(name: &str) -> Option<&'static VerbSpec> {
    VOCABULARY.iter().find(|spec| spec.verb == name)
}

/// Render the whole vocabulary as lines a model can be shown.
///
/// Deliberately terse: this goes into a system prompt, where every token is
/// paid for on every request.
#[must_use]
pub fn as_prompt_lines() -> Vec<String> {
    VOCABULARY
        .iter()
        .map(|spec| format!("{} — {}", spec.example, spec.help))
        .collect()
}

/// Gain ranges match the isolator EQ: 0.0 is a true kill, 4.0 is +12 dB.
const EQ: ArgSpec = ArgSpec::Number { min: 0.0, max: 4.0 };

static VOCABULARY: [VerbSpec; 73] = [
    // -- transport ---------------------------------------------------------
    VerbSpec {
        target: Target::Deck,
        verb: "play",
        argument: ArgSpec::None,
        help: "start playback",
        example: "deck 1 play",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "pause",
        argument: ArgSpec::None,
        help: "stop playback, staying where you are",
        example: "deck 1 pause",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "play_pause",
        argument: ArgSpec::None,
        help: "toggle playback",
        example: "deck 1 play_pause",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "cue",
        argument: ArgSpec::None,
        help: "stop and return to the cue point",
        example: "deck 1 cue",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "eject",
        argument: ArgSpec::None,
        help: "unload the track",
        example: "deck 1 eject",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "seek",
        argument: ArgSpec::Frames,
        help: "jump the playhead to a position in frames",
        example: "deck 1 seek 48000",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "rate",
        argument: ArgSpec::Number {
            min: -4.0,
            max: 4.0,
        },
        help: "set playback speed directly; 1.0 is normal, negative is reverse",
        example: "deck 1 rate 1.0",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "pitch",
        argument: ArgSpec::Number {
            min: -1.0,
            max: 1.0,
        },
        help: "pitch fader as a fraction; 0.08 is +8%",
        example: "deck 1 pitch 0.08",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "keylock_on",
        argument: ArgSpec::None,
        help: "hold the musical key while the pitch fader changes tempo",
        example: "deck 1 keylock_on",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "keylock_off",
        argument: ArgSpec::None,
        help: "let the pitch fader move tempo and key together",
        example: "deck 1 keylock_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "keylock_toggle",
        argument: ArgSpec::None,
        help: "flip keylock",
        example: "deck 1 keylock_toggle",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "slip_on",
        argument: ArgSpec::None,
        help: "keep a shadow playhead running while a loop or a censor diverts this one",
        example: "deck 1 slip_on",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "slip_off",
        argument: ArgSpec::None,
        help: "stop shadowing, and stay where the playhead is",
        example: "deck 1 slip_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "slip_toggle",
        argument: ArgSpec::None,
        help: "flip slip mode",
        example: "deck 1 slip_toggle",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "reverse_on",
        argument: ArgSpec::None,
        help: "play backwards",
        example: "deck 1 reverse_on",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "reverse_off",
        argument: ArgSpec::None,
        help: "play forwards again",
        example: "deck 1 reverse_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "reverse_toggle",
        argument: ArgSpec::None,
        help: "flip the direction of travel",
        example: "deck 1 reverse_toggle",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "censor_on",
        argument: ArgSpec::None,
        help: "hold the censor: reverse over a word, and land back on the beat",
        example: "deck 1 censor_on",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "censor_off",
        argument: ArgSpec::None,
        help: "release the censor",
        example: "deck 1 censor_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "brake",
        argument: ArgSpec::Number {
            min: 0.0625,
            max: 32.0,
        },
        help: "cut the motor and coast to a stop over this many beats, like a turntable losing power",
        example: "deck 1 brake 2",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "brake_off",
        argument: ArgSpec::None,
        help: "put the motor back on, wherever the record got to",
        example: "deck 1 brake_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "backspin",
        argument: ArgSpec::Number {
            min: 0.0625,
            max: 32.0,
        },
        help: "throw the record backwards and let friction take it down over this many beats",
        example: "deck 1 backspin 1",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "backspin_off",
        argument: ArgSpec::None,
        help: "put the motor back on after a backspin",
        example: "deck 1 backspin_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "roll",
        argument: ArgSpec::Number {
            min: 0.0625,
            max: 32.0,
        },
        help: "hold a loop roll this many beats long, as a decimal or 1/4 — the track carries on underneath",
        example: "deck 1 roll 0.25",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "roll_off",
        argument: ArgSpec::None,
        help: "release the roll, and land where the track would have been",
        example: "deck 1 roll_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "fx",
        argument: ArgSpec::Words,
        help: "change an effect slot: `fx <1-3> <none|echo|delay|reverb|gate|crush|flanger|phaser|filter>` to load one, then `on`/`off`/`toggle`, `wet <0-1>`, `beats <0.0625-4>`, `amount <0-1>`, `pre`/`post`",
        example: "deck 1 fx 1 wet 0.5",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "key",
        argument: ArgSpec::Number {
            min: -12.0,
            max: 12.0,
        },
        help: "transpose in semitones for harmonic mixing, without changing tempo",
        example: "deck 1 key 2",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "sync",
        argument: ArgSpec::None,
        help: "match tempo and phase to the other playing deck; refused if either grid is weak",
        example: "deck 2 sync",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "sync_off",
        argument: ArgSpec::None,
        help: "release the tempo lock and give the pitch fader back",
        example: "deck 2 sync_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "beatjump",
        argument: ArgSpec::Number {
            min: -64.0,
            max: 64.0,
        },
        help: "move the playhead by whole beats; negative goes back",
        example: "deck 1 beatjump 4",
    },
    // -- hot cues and loops -------------------------------------------------
    VerbSpec {
        target: Target::Deck,
        verb: "hotcue",
        argument: ArgSpec::Number { min: 1.0, max: 8.0 },
        help: "jump to a hot cue, or set it here if the slot is empty",
        example: "deck 1 hotcue 1",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "hotcue_set",
        argument: ArgSpec::Number { min: 1.0, max: 8.0 },
        help: "set a hot cue at the playhead, replacing whatever was there",
        example: "deck 1 hotcue_set 2",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "hotcue_clear",
        argument: ArgSpec::Number { min: 1.0, max: 8.0 },
        help: "forget a hot cue",
        example: "deck 1 hotcue_clear 2",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop",
        argument: ArgSpec::Number {
            min: 0.0,
            max: 128.0,
        },
        help: "loop this many beats from here, as a decimal or 1/4; 0 turns looping off",
        example: "deck 1 loop 4",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_off",
        argument: ArgSpec::None,
        help: "stop looping and carry on",
        example: "deck 1 loop_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_halve",
        argument: ArgSpec::None,
        help: "halve the loop, keeping its start",
        example: "deck 1 loop_halve",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_double",
        argument: ArgSpec::None,
        help: "double the loop, keeping its start",
        example: "deck 1 loop_double",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_in",
        argument: ArgSpec::None,
        help: "drop the loop's in point at the playhead",
        example: "deck 1 loop_in",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_out",
        argument: ArgSpec::None,
        help: "drop the out point and start looping",
        example: "deck 1 loop_out",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_move",
        argument: ArgSpec::Number {
            min: -64.0,
            max: 64.0,
        },
        help: "slide the whole loop by whole beats, keeping its length",
        example: "deck 1 loop_move 4",
    },
    // -- channel strip -----------------------------------------------------
    VerbSpec {
        target: Target::Deck,
        verb: "volume",
        argument: ArgSpec::Number { min: 0.0, max: 1.0 },
        help: "channel fader",
        example: "deck 1 volume 0.8",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "gain",
        argument: ArgSpec::Number {
            min: -24.0,
            max: 24.0,
        },
        help: "trim, in decibels",
        example: "deck 1 gain -3",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "eq_low",
        argument: EQ,
        help: "low band; 0 is a true kill, 1 is flat",
        example: "deck 1 eq_low 0",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "eq_mid",
        argument: EQ,
        help: "mid band; 0 is a true kill, 1 is flat",
        example: "deck 1 eq_mid 1",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "eq_high",
        argument: EQ,
        help: "high band; 0 is a true kill, 1 is flat",
        example: "deck 1 eq_high 1",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "filter",
        argument: ArgSpec::Number {
            min: -1.0,
            max: 1.0,
        },
        help: "filter sweep; -1 fully low-passed, 0 off, +1 fully high-passed",
        example: "deck 1 filter -0.5",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "cue_on",
        argument: ArgSpec::None,
        help: "send this deck to the headphones",
        example: "deck 1 cue_on",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "cue_off",
        argument: ArgSpec::None,
        help: "stop sending this deck to the headphones",
        example: "deck 1 cue_off",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "cue_toggle",
        argument: ArgSpec::None,
        help: "flip the headphone cue send",
        example: "deck 1 cue_toggle",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "xfader_left",
        argument: ArgSpec::None,
        help: "put this deck on the left half of the crossfader",
        example: "deck 1 xfader_left",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "xfader_right",
        argument: ArgSpec::None,
        help: "put this deck on the right half of the crossfader",
        example: "deck 2 xfader_right",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "xfader_thru",
        argument: ArgSpec::None,
        help: "take this deck off the crossfader; it plays whatever the crossfader does",
        example: "deck 3 xfader_thru",
    },
    // -- beat grid ---------------------------------------------------------
    VerbSpec {
        target: Target::Deck,
        verb: "grid_here",
        argument: ArgSpec::None,
        help: "put a beat on the playhead, leaving the tempo alone",
        example: "deck 1 grid_here",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "grid_nudge",
        argument: ArgSpec::Number {
            min: -500.0,
            max: 500.0,
        },
        help: "slide the whole beat grid by milliseconds; negative is earlier",
        example: "deck 1 grid_nudge -10",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "grid_scale",
        argument: ArgSpec::Number {
            min: 0.25,
            max: 4.0,
        },
        help: "multiply the grid tempo, keeping the anchor; 2 and 0.5 fix an octave error",
        example: "deck 1 grid_scale 2",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "grid_bpm",
        argument: ArgSpec::Number {
            min: 20.0,
            max: 400.0,
        },
        help: "set the grid tempo outright, keeping the anchor",
        example: "deck 1 grid_bpm 128",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "grid_tap",
        argument: ArgSpec::None,
        help: "tap along with the music; two taps give a tempo and the last sets the phase",
        example: "deck 1 grid_tap",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "grid_reset",
        argument: ArgSpec::None,
        help: "throw away grid edits and go back to what the analyser found",
        example: "deck 1 grid_reset",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_save",
        argument: ArgSpec::Number { min: 1.0, max: 8.0 },
        help: "keep the loop that is playing in a numbered slot, with the track",
        example: "deck 1 loop_save 1",
    },
    VerbSpec {
        target: Target::Deck,
        verb: "loop_recall",
        argument: ArgSpec::Number { min: 1.0, max: 8.0 },
        help: "put a saved loop back and start looping it",
        example: "deck 1 loop_recall 1",
    },
    // -- mixer -------------------------------------------------------------
    VerbSpec {
        target: Target::Mixer,
        verb: "crossfader",
        argument: ArgSpec::Number {
            min: -1.0,
            max: 1.0,
        },
        help: "crossfader; -1 is the decks assigned left, +1 the decks assigned right",
        example: "crossfader 0",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "sampler",
        argument: ArgSpec::Words,
        help: "the sampler: `sampler <1-8> trigger|release|stop|clear`, `sampler <1-8> one_shot|hold|loop|stutter` to set how a pad behaves, `sampler <1-8> volume <0-1>`, `master`/`cue` to route it, `sync`/`sync_off`; and `sampler bank <1-4>`, `sampler volume <0-1>`, `sampler stop_all`",
        example: "sampler 1 volume 0.8",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "sampler record",
        argument: ArgSpec::Words,
        help: "record into a slot of the bank showing: `sampler record <1-8> master` takes what the room hears, `sampler record <1-8> deck <n>` takes that deck before its fader; then `sampler record stop` to keep it or `sampler record cancel` to bin it",
        example: "sampler record 3 deck 1",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "master fx",
        argument: ArgSpec::Words,
        help: "change a master effect slot; the same grammar as a deck's, applied to the whole mix",
        example: "master fx 1 wet 0.5",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "master gain",
        argument: ArgSpec::Number {
            min: -24.0,
            max: 24.0,
        },
        help: "master output level in decibels",
        example: "master gain 0",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "booth gain",
        argument: ArgSpec::Number {
            min: -24.0,
            max: 24.0,
        },
        help: "booth monitor level in decibels",
        example: "booth gain -6",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "cue mix",
        argument: ArgSpec::Number { min: 0.0, max: 1.0 },
        help: "headphone blend; 0 is all cue, 1 is all master",
        example: "cue mix 0.5",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "cue split_on",
        argument: ArgSpec::None,
        help: "cue in one ear, master in the other",
        example: "cue split_on",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "cue split_off",
        argument: ArgSpec::None,
        help: "return the headphones to a blend",
        example: "cue split_off",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "quantize on",
        argument: ArgSpec::None,
        help: "snap beat jumps to the grid",
        example: "quantize on",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "quantize off",
        argument: ArgSpec::None,
        help: "let beat jumps move by an exact beat from wherever you are",
        example: "quantize off",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "limiter on",
        argument: ArgSpec::None,
        help: "engage the master limiter (on by default)",
        example: "limiter on",
    },
    VerbSpec {
        target: Target::Mixer,
        verb: "limiter off",
        argument: ArgSpec::None,
        help: "bypass the master limiter, for an external processor downstream",
        example: "limiter off",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

    /// **The test this module exists for.**
    ///
    /// A schema shown to a model is generated from these entries, so an entry
    /// describing something the parser rejects would produce a model confidently
    /// emitting commands that fail. Every example must parse.
    #[test]
    fn every_example_parses() {
        for spec in vocabulary() {
            Action::parse(spec.example).unwrap_or_else(|e| {
                panic!(
                    "vocabulary entry `{}` has example `{}` which does not parse: {e}",
                    spec.verb, spec.example
                )
            });
        }
    }

    /// An example must exercise the verb it claims to describe, or the test
    /// above passes while documenting the wrong thing.
    #[test]
    fn every_example_uses_its_own_verb() {
        for spec in vocabulary() {
            assert!(
                spec.example.contains(spec.verb),
                "`{}` is described by example `{}`, which does not use it",
                spec.verb,
                spec.example
            );
        }
    }

    #[test]
    fn an_entry_that_takes_an_argument_shows_one() {
        for spec in vocabulary() {
            let words: Vec<&str> = spec.example.split_whitespace().collect();
            let has_trailing_number = words.last().is_some_and(|w| w.parse::<f32>().is_ok());
            assert_eq!(
                spec.argument.takes_argument(),
                has_trailing_number,
                "`{}` argument spec disagrees with its example `{}`",
                spec.verb,
                spec.example
            );
        }
    }

    #[test]
    fn verbs_are_unique() {
        use std::collections::HashSet;
        let names: HashSet<&str> = vocabulary().iter().map(|s| s.verb).collect();
        assert_eq!(names.len(), vocabulary().len(), "duplicate verb");
    }

    #[test]
    fn every_entry_explains_itself() {
        for spec in vocabulary() {
            assert!(!spec.help.is_empty(), "{} has no help", spec.verb);
            // A model reads this. One or two words is not a description.
            assert!(spec.help.len() > 8, "{} has a stub help line", spec.verb);
        }
    }

    /// Hand-maintained mirror of what the parser accepts.
    ///
    /// Adding a verb to `action.rs` without adding it here fails this test,
    /// which is the point: a verb missing from the vocabulary is invisible to
    /// the assistant, to `--help` and to the scripting documentation, and
    /// nothing else would notice.
    #[test]
    fn the_vocabulary_covers_every_verb_the_parser_accepts() {
        let expected = [
            "play",
            "pause",
            "play_pause",
            "cue",
            "eject",
            "seek",
            "rate",
            "pitch",
            "keylock_on",
            "keylock_off",
            "keylock_toggle",
            "key",
            "volume",
            "gain",
            "eq_low",
            "eq_mid",
            "eq_high",
            "filter",
            "cue_on",
            "cue_off",
            "cue_toggle",
            "crossfader",
            "xfader_left",
            "xfader_right",
            "xfader_thru",
            "grid_here",
            "grid_nudge",
            "grid_scale",
            "grid_bpm",
            "grid_tap",
            "grid_reset",
            "loop_save",
            "loop_recall",
            "master gain",
            "booth gain",
            "cue mix",
            "cue split_on",
            "cue split_off",
            "limiter on",
            "limiter off",
            "sync",
            "sync_off",
            "beatjump",
            "quantize on",
            "quantize off",
            "hotcue",
            "hotcue_set",
            "hotcue_clear",
            "loop",
            "loop_off",
            "loop_halve",
            "loop_double",
            "loop_in",
            "loop_out",
            "loop_move",
            "slip_on",
            "slip_off",
            "slip_toggle",
            "reverse_on",
            "reverse_off",
            "reverse_toggle",
            "censor_on",
            "censor_off",
            "brake",
            "brake_off",
            "backspin",
            "backspin_off",
            "roll",
            "roll_off",
            "fx",
            "master fx",
            "sampler",
            "sampler record",
        ];
        for name in expected {
            assert!(
                verb(name).is_some(),
                "`{name}` parses but is missing from the vocabulary"
            );
        }
        assert_eq!(
            vocabulary().len(),
            expected.len(),
            "the vocabulary has an entry the parser does not accept"
        );
    }

    #[test]
    fn prompt_lines_are_one_per_verb_and_carry_both_halves() {
        let lines = as_prompt_lines();
        assert_eq!(lines.len(), vocabulary().len());
        assert!(lines.iter().any(|l| l.starts_with("deck 1 play —")));
    }

    /// Ranges are for the model's benefit; the parser clamps regardless. This
    /// checks the documented range does not contradict the clamp.
    #[test]
    fn documented_ranges_are_the_ones_actually_enforced() {
        // An EQ value above the documented maximum comes back clamped to it.
        let action = Action::parse("deck 1 eq_low 99").unwrap();
        match action {
            Action::Deck {
                action: crate::action::DeckAction::SetEqLow(gain),
                ..
            } => assert_eq!(gain, 4.0),
            other => panic!("unexpected: {other:?}"),
        }

        let action = Action::parse("crossfader 5").unwrap();
        assert_eq!(
            action,
            Action::Mixer(crate::action::MixerAction::Crossfader(1.0))
        );
    }
}

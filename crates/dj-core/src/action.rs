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

use crate::deck::{CrossfaderAssign, DeckId};
use crate::fx::{EffectKind, FX_SLOTS, FxChange, Placement};
use crate::hotcue::HOT_CUE_SLOTS;
use crate::sampler::{SAMPLE_SLOTS, SampleChange, SampleOutput, SamplerChange, TriggerMode};
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
    /// Slip mode: keep a shadow playhead running at the natural rate while
    /// something diverts the audible one, and land there when it stops.
    SetSlip(bool),
    /// Flip slip mode.
    ToggleSlip,
    /// Play backwards, or forwards again.
    SetReverse(bool),
    /// Flip the direction of travel.
    ToggleReverse,
    /// Hold or release the censor: momentary reverse that always slips, so it
    /// hides a word and puts you back on the beat.
    SetCensor(bool),
    /// Hold a loop roll of this many beats, or release it with `None`.
    ///
    /// Momentary, like the censor, and for the same reason: a roll is a
    /// stutter you hold, and the track carries on underneath so you land back
    /// on the beat.
    ///
    /// Fractional because the roll a DJ means by the word is the sub-beat one:
    /// a quarter-beat stutter into a drop. Whole-beat rolls are real too, but a
    /// roll that could only be a whole beat would be missing the move it is
    /// named after. Clamped by the engine's own loop limits, so 1/16 of a beat
    /// is the floor here as it is for halving a loop.
    LoopRoll(Option<f32>),
    /// Change one of this deck's effect slots.
    ///
    /// One variant with a sub-grammar rather than a verb per slot per control.
    /// Three slots times seven controls would be twenty-one verbs on the deck
    /// alone, and the vocabulary is read by a model on every request.
    Fx {
        slot: u8,
        change: FxChange,
    },
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
    ///
    /// Fractional for the same reason a roll is: halving a loop already
    /// reaches a sixteenth of a beat, so the length was never an integer —
    /// only the way of asking for one was. `loop 1/4` and `loop 0.25` both
    /// work, and the pad ladder runs from a sixteenth to eight beats.
    LoopBeats(f32),
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
    /// Put this deck on one side of the crossfader, or take it off entirely.
    SetCrossfaderAssign(CrossfaderAssign),

    // -- beat grid editing --------------------------------------------------
    //
    // These are edits a *person* makes, which is why they are actions while
    // `Command::SetGrid` is not: nobody presses "set grid", but everybody who
    // has played a track the analyser misread has wanted to drag the grid onto
    // the beat. A controller encoder, a script and the assistant should all be
    // able to say so, and that is what the vocabulary is for.
    //
    // Every one of them marks the result certain. The DJ looked at the
    // waveform and said where the beat is; that outranks a correlation score,
    // and it is the whole point of editing a grid the analyser was unsure of.
    /// Put a beat exactly on the playhead, leaving the tempo alone.
    ///
    /// The one-button fix for a grid whose tempo is right and whose phase is
    /// not, which is the common failure: cue to the downbeat, press it once.
    GridAnchorHere,
    /// Slide the whole grid by milliseconds, keeping the tempo. Negative is
    /// earlier.
    GridNudge(f64),
    /// Multiply the tempo, keeping the anchor. `2` and `0.5` fix an octave
    /// error; values near 1 fine-tune a grid that drifts over a long track.
    GridScale(f64),
    /// Set the tempo outright, keeping the anchor.
    GridSetBpm(f64),
    /// Tap along with the music. Two taps give a tempo, more refine it, and the
    /// last tap sets the phase.
    GridTap,
    /// Throw the edits away and go back to what the analyser said.
    GridReset,

    // -- saved loops --------------------------------------------------------
    //
    // A saved loop belongs to the *track*, not the deck: the eight-bar section
    // you loop every time you play a record is a property of the record. So
    // these say which slot, and the host looks the region up in the library.
    /// Keep the loop that is playing in a numbered slot.
    LoopSave(u8),
    /// Put a saved loop back and start looping it.
    LoopRecall(u8),
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
    /// Change one of the master rack's effect slots.
    ///
    /// The same sub-grammar as a deck's, because it is the same rack in a
    /// different place — and a DJ who has learnt one has learnt both.
    Fx {
        slot: u8,
        change: FxChange,
    },
    /// Fire or configure one sampler pad, in the bank that is showing.
    Sample {
        slot: u8,
        change: SampleChange,
    },
    /// Change the sampler as a whole: its bank, its level, or stop everything.
    Sampler(SamplerChange),
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
                // `fx` is the one verb with a sub-grammar of its own, so it
                // takes the rest of the line rather than a single argument.
                if verb == "fx" {
                    let (slot, change) = parse_fx(&mut words)?;
                    return Ok(Action::Deck {
                        deck,
                        action: DeckAction::Fx { slot, change },
                    });
                }
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
                "fx" => {
                    let (slot, change) = parse_fx(&mut words)?;
                    Ok(Action::Mixer(MixerAction::Fx { slot, change }))
                }
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            "booth" => match words.next().ok_or(ParseError::MissingVerb)? {
                "gain" => Ok(Action::Mixer(MixerAction::BoothGainDb(parse_f32(
                    words.next(),
                )?))),
                other => Err(ParseError::UnknownVerb(other.to_owned())),
            },
            // `sampler 3 trigger` addresses a pad; `sampler bank 2` addresses
            // the sampler. Told apart by whether the next word is a slot
            // number, which is unambiguous because no verb here is a number.
            "sampler" => {
                let what = words.next().ok_or(ParseError::MissingVerb)?;
                match what.parse::<u8>() {
                    Ok(slot) => {
                        let slot = valid_sample_slot(slot)?;
                        let change = parse_sample_change(words.next(), words.next())?;
                        Ok(Action::Mixer(MixerAction::Sample { slot, change }))
                    }
                    Err(_) => Ok(Action::Mixer(MixerAction::Sampler(parse_sampler_change(
                        what,
                        words.next(),
                    )?))),
                }
            }
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

/// A sampler slot, 1-based as the pads number them.
///
/// Refused rather than clamped, like a hot cue slot: slot 0 or slot 9 is a
/// mistake upstream, and quietly firing slot 1 instead would hide a controller
/// mapped to the wrong range — while playing the wrong sample to a room.
fn valid_sample_slot(slot: u8) -> Result<u8, ParseError> {
    if slot >= 1 && usize::from(slot) <= SAMPLE_SLOTS {
        Ok(slot)
    } else {
        Err(ParseError::BadArgument)
    }
}

fn parse_sample_change(
    what: Option<&str>,
    value: Option<&str>,
) -> Result<SampleChange, ParseError> {
    Ok(match what.ok_or(ParseError::MissingArgument)? {
        "trigger" => SampleChange::Trigger,
        "release" => SampleChange::Release,
        "stop" => SampleChange::Stop,
        "master" => SampleChange::Route(SampleOutput::Master),
        "cue" => SampleChange::Route(SampleOutput::Cue),
        "sync" => SampleChange::SetSync(true),
        "sync_off" => SampleChange::SetSync(false),
        "clear" => SampleChange::Clear,
        "volume" => SampleChange::Volume(parse_f32(value)?.clamp(0.0, 1.0)),
        // Anything else must be a trigger mode, so an unknown word here is an
        // unknown *mode* -- which is the mistake the caller actually made.
        name => SampleChange::SetMode(
            TriggerMode::parse(name).ok_or_else(|| ParseError::UnknownMode(name.to_owned()))?,
        ),
    })
}

fn parse_sampler_change(what: &str, value: Option<&str>) -> Result<SamplerChange, ParseError> {
    Ok(match what {
        "bank" => SamplerChange::Bank(parse_slot(value)?),
        "volume" => SamplerChange::Volume(parse_f32(value)?.clamp(0.0, 1.0)),
        "stop_all" => SamplerChange::StopAll,
        other => return Err(ParseError::UnknownVerb(other.to_owned())),
    })
}

/// `<slot> <what> [value]` — the effect sub-grammar, shared by decks and master.
///
/// A bare effect name selects it, so `fx 1 echo` reads the way a DJ would say
/// it. Everything else is a named control, which keeps the grammar open: a new
/// control is a new word here rather than three new verbs in the vocabulary.
fn parse_fx<'a>(words: &mut impl Iterator<Item = &'a str>) -> Result<(u8, FxChange), ParseError> {
    let slot = parse_fx_slot(words.next())?;
    let what = words.next().ok_or(ParseError::MissingArgument)?;
    let change = match what {
        "on" => FxChange::SetEnabled(true),
        "off" => FxChange::SetEnabled(false),
        "toggle" => FxChange::ToggleEnabled,
        "pre" => FxChange::Place(Placement::PreFader),
        "post" => FxChange::Place(Placement::PostFader),
        "wet" => FxChange::Wet(parse_f32(words.next())?.clamp(0.0, 1.0)),
        "beats" => FxChange::Beats(parse_beats(words.next())?),
        "amount" => FxChange::Amount(parse_f32(words.next())?.clamp(0.0, 1.0)),
        // Anything else must be an effect name, so an unknown word here is an
        // unknown *effect* -- which is the error the caller actually made.
        name => FxChange::Select(
            EffectKind::parse(name).ok_or_else(|| ParseError::UnknownEffect(name.to_owned()))?,
        ),
    };
    Ok((slot, change))
}

/// An effect slot, 1-based as the interface and every controller number them.
///
/// Rejected rather than clamped, for the same reason a hot cue slot is: slot 0
/// or slot 9 is a mistake upstream, and quietly using slot 1 instead would hide
/// it.
fn parse_fx_slot(word: Option<&str>) -> Result<u8, ParseError> {
    let slot: u8 = word
        .ok_or(ParseError::MissingArgument)?
        .parse()
        .map_err(|_| ParseError::BadArgument)?;
    if slot >= 1 && usize::from(slot) <= FX_SLOTS {
        Ok(slot)
    } else {
        Err(ParseError::BadArgument)
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
        "loop" => DeckAction::LoopBeats(parse_beats(argument)?),
        "loop_off" => DeckAction::LoopOff,
        "loop_halve" => DeckAction::LoopHalve,
        "loop_double" => DeckAction::LoopDouble,
        "loop_in" => DeckAction::LoopIn,
        "loop_out" => DeckAction::LoopOut,
        "loop_move" => DeckAction::LoopMove(parse_i32(argument)?),
        "keylock_on" => DeckAction::SetKeylock(true),
        "keylock_off" => DeckAction::SetKeylock(false),
        "keylock_toggle" => DeckAction::ToggleKeylock,
        "slip_on" => DeckAction::SetSlip(true),
        "slip_off" => DeckAction::SetSlip(false),
        "slip_toggle" => DeckAction::ToggleSlip,
        "reverse_on" => DeckAction::SetReverse(true),
        "reverse_off" => DeckAction::SetReverse(false),
        "reverse_toggle" => DeckAction::ToggleReverse,
        // Momentary: a censor is held, so it needs a press and a release
        // rather than a toggle. A toggled censor would be reverse with extra
        // steps.
        "censor_on" => DeckAction::SetCensor(true),
        "censor_off" => DeckAction::SetCensor(false),
        "roll" => DeckAction::LoopRoll(Some(parse_beats(argument)?)),
        "roll_off" => DeckAction::LoopRoll(None),
        "key" => DeckAction::SetKeyShift(parse_f32(argument)?.round() as i32),
        // Three verbs rather than one verb with a word argument, matching
        // `cue_on`/`cue_off` above: a three-position switch is three buttons on
        // a controller and three buttons in the interface, and one message per
        // position is what each of them sends.
        "xfader_left" => DeckAction::SetCrossfaderAssign(CrossfaderAssign::Left),
        "xfader_right" => DeckAction::SetCrossfaderAssign(CrossfaderAssign::Right),
        "xfader_thru" => DeckAction::SetCrossfaderAssign(CrossfaderAssign::Thru),
        "grid_here" => DeckAction::GridAnchorHere,
        "grid_nudge" => DeckAction::GridNudge(f64::from(parse_f32(argument)?)),
        "grid_scale" => DeckAction::GridScale(f64::from(parse_f32(argument)?)),
        "grid_bpm" => DeckAction::GridSetBpm(f64::from(parse_f32(argument)?)),
        "grid_tap" => DeckAction::GridTap,
        "grid_reset" => DeckAction::GridReset,
        "loop_save" => DeckAction::LoopSave(parse_slot(argument)?),
        "loop_recall" => DeckAction::LoopRecall(parse_slot(argument)?),
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

/// A loop length in beats, written the way a DJ says it.
///
/// `1/4` as well as `0.25`, because a sub-beat loop has a spoken name and a
/// script or a hardware mapping written by hand will use it. `Display` emits
/// the decimal, so the canonical form still round-trips; this only widens what
/// is accepted.
fn parse_beats(word: Option<&str>) -> Result<f32, ParseError> {
    let word = word.ok_or(ParseError::MissingArgument)?;
    let value = match word.split_once('/') {
        Some((numerator, denominator)) => {
            let numerator: f32 = numerator
                .trim()
                .parse()
                .map_err(|_| ParseError::BadArgument)?;
            let denominator: f32 = denominator
                .trim()
                .parse()
                .map_err(|_| ParseError::BadArgument)?;
            if denominator == 0.0 {
                return Err(ParseError::BadArgument);
            }
            numerator / denominator
        }
        None => word.parse().map_err(|_| ParseError::BadArgument)?,
    };
    // NaN would pass every later comparison by failing it, which is the one
    // way a bad number gets past a range check without being noticed.
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ParseError::BadArgument)
    }
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
                DeckAction::SetSlip(true) => write!(f, "deck {deck} slip_on"),
                DeckAction::SetSlip(false) => write!(f, "deck {deck} slip_off"),
                DeckAction::ToggleSlip => write!(f, "deck {deck} slip_toggle"),
                DeckAction::SetReverse(true) => write!(f, "deck {deck} reverse_on"),
                DeckAction::SetReverse(false) => write!(f, "deck {deck} reverse_off"),
                DeckAction::ToggleReverse => write!(f, "deck {deck} reverse_toggle"),
                DeckAction::SetCensor(true) => write!(f, "deck {deck} censor_on"),
                DeckAction::SetCensor(false) => write!(f, "deck {deck} censor_off"),
                DeckAction::LoopRoll(Some(beats)) => write!(f, "deck {deck} roll {beats}"),
                DeckAction::LoopRoll(None) => write!(f, "deck {deck} roll_off"),
                DeckAction::SetKeyShift(n) => write!(f, "deck {deck} key {n}"),
                DeckAction::Fx { slot, change } => write!(f, "deck {deck} fx {slot} {change}"),
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
                DeckAction::SetCrossfaderAssign(a) => {
                    write!(f, "deck {deck} xfader_{}", a.slug())
                }
                DeckAction::GridAnchorHere => write!(f, "deck {deck} grid_here"),
                DeckAction::GridNudge(ms) => write!(f, "deck {deck} grid_nudge {}", number(*ms)),
                DeckAction::GridScale(x) => write!(f, "deck {deck} grid_scale {}", number(*x)),
                DeckAction::GridSetBpm(b) => write!(f, "deck {deck} grid_bpm {}", number(*b)),
                DeckAction::GridTap => write!(f, "deck {deck} grid_tap"),
                DeckAction::GridReset => write!(f, "deck {deck} grid_reset"),
                DeckAction::LoopSave(slot) => write!(f, "deck {deck} loop_save {slot}"),
                DeckAction::LoopRecall(slot) => write!(f, "deck {deck} loop_recall {slot}"),
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
            Action::Mixer(MixerAction::Fx { slot, change }) => {
                write!(f, "master fx {slot} {change}")
            }
            Action::Mixer(MixerAction::Sample { slot, change }) => {
                write!(f, "sampler {slot} {change}")
            }
            Action::Mixer(MixerAction::Sampler(change)) => write!(f, "sampler {change}"),
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
    /// Its own error rather than `UnknownVerb`, because in the effect
    /// sub-grammar an unrecognised word is an effect name and saying "unknown
    /// verb `revrb`" would point at the wrong thing.
    #[error("unknown effect `{0}`")]
    UnknownEffect(String),
    /// Likewise for the sampler's sub-grammar: an unrecognised word there is a
    /// trigger mode, and "unknown verb `sutter`" would point at the wrong
    /// thing.
    #[error("unknown trigger mode `{0}`")]
    UnknownMode(String),
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

    /// A roll is spoken as a fraction — "quarter-beat roll" — so a script or a
    /// hand-written mapping will write one. Both spellings must reach the same
    /// action, and `Display` must emit something that parses back.
    #[test]
    fn a_roll_can_be_written_as_a_fraction_or_a_decimal() {
        let quarter = Action::Deck {
            deck: deck(1),
            action: DeckAction::LoopRoll(Some(0.25)),
        };
        assert_eq!(Action::parse("deck 1 roll 1/4").unwrap(), quarter);
        assert_eq!(Action::parse("deck 1 roll 0.25").unwrap(), quarter);
        assert_eq!(Action::parse(&quarter.to_string()).unwrap(), quarter);

        assert_eq!(
            Action::parse("deck 1 roll 1/0"),
            Err(ParseError::BadArgument),
            "a zero denominator is not an infinite roll"
        );
        assert_eq!(
            Action::parse("deck 1 roll half"),
            Err(ParseError::BadArgument)
        );
    }

    /// The effect grammar is one verb with a sub-grammar, so it needs testing
    /// as a grammar rather than as a list of verbs.
    #[test]
    fn the_effect_sub_grammar_reaches_every_control() {
        let fx = |text: &str, slot: u8, change: FxChange| {
            assert_eq!(
                Action::parse(text).unwrap(),
                Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Fx { slot, change },
                },
                "parsing `{text}`"
            );
        };

        // A bare effect name selects it, the way a DJ says it.
        fx("deck 1 fx 1 echo", 1, FxChange::Select(EffectKind::Echo));
        fx("deck 1 fx 3 none", 3, FxChange::Select(EffectKind::None));
        fx("deck 1 fx 2 on", 2, FxChange::SetEnabled(true));
        fx("deck 1 fx 2 off", 2, FxChange::SetEnabled(false));
        fx("deck 1 fx 2 toggle", 2, FxChange::ToggleEnabled);
        fx("deck 1 fx 1 wet 0.25", 1, FxChange::Wet(0.25));
        fx("deck 1 fx 1 beats 1/4", 1, FxChange::Beats(0.25));
        fx("deck 1 fx 1 amount 0.8", 1, FxChange::Amount(0.8));
        fx("deck 1 fx 1 pre", 1, FxChange::Place(Placement::PreFader));
        fx("deck 1 fx 1 post", 1, FxChange::Place(Placement::PostFader));
    }

    #[test]
    fn the_master_rack_takes_the_same_grammar_as_a_deck() {
        assert_eq!(
            Action::parse("master fx 2 gate").unwrap(),
            Action::Mixer(MixerAction::Fx {
                slot: 2,
                change: FxChange::Select(EffectKind::Gate),
            })
        );
    }

    /// An unknown word in the sub-grammar is an unknown *effect*, and saying
    /// "unknown verb" would point the reader at the wrong thing entirely.
    #[test]
    fn a_misspelt_effect_says_so() {
        assert_eq!(
            Action::parse("deck 1 fx 1 revrb"),
            Err(ParseError::UnknownEffect("revrb".to_owned()))
        );
    }

    /// Slot 0 and slot 9 are mistakes upstream. Quietly using slot 1 instead
    /// would hide a controller mapped to the wrong range.
    #[test]
    fn an_out_of_range_effect_slot_is_refused_rather_than_clamped() {
        assert_eq!(
            Action::parse("deck 1 fx 0 echo"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(
            Action::parse("deck 1 fx 4 echo"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(
            Action::parse("deck 1 fx x echo"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(Action::parse("deck 1 fx"), Err(ParseError::MissingArgument));
        assert_eq!(
            Action::parse("deck 1 fx 1"),
            Err(ParseError::MissingArgument)
        );
    }

    /// Everything in the log has to be replayable, and the effect grammar is
    /// the first action whose text form is more than a verb and a number.
    #[test]
    fn every_effect_change_round_trips_through_its_text_form() {
        let changes = [
            FxChange::Select(EffectKind::Flanger),
            FxChange::SetEnabled(true),
            FxChange::SetEnabled(false),
            FxChange::ToggleEnabled,
            FxChange::Wet(0.5),
            FxChange::Beats(0.25),
            FxChange::Amount(0.75),
            FxChange::Place(Placement::PreFader),
            FxChange::Place(Placement::PostFader),
        ];
        for change in changes {
            for action in [
                Action::Deck {
                    deck: deck(2),
                    action: DeckAction::Fx { slot: 3, change },
                },
                Action::Mixer(MixerAction::Fx { slot: 3, change }),
            ] {
                let text = action.to_string();
                assert_eq!(
                    Action::parse(&text).unwrap(),
                    action,
                    "`{text}` did not survive the round trip"
                );
            }
        }
    }

    /// The sampler grammar has to tell a pad from the sampler itself, and it
    /// does that by whether the word after `sampler` is a number. Worth its own
    /// test because it is the only place in the vocabulary where a *shape*
    /// rather than a keyword picks the branch.
    #[test]
    fn the_sampler_grammar_tells_a_pad_from_the_sampler() {
        assert_eq!(
            Action::parse("sampler 3 trigger").unwrap(),
            Action::Mixer(MixerAction::Sample {
                slot: 3,
                change: SampleChange::Trigger,
            })
        );
        assert_eq!(
            Action::parse("sampler bank 2").unwrap(),
            Action::Mixer(MixerAction::Sampler(SamplerChange::Bank(2)))
        );
        assert_eq!(
            Action::parse("sampler stop_all").unwrap(),
            Action::Mixer(MixerAction::Sampler(SamplerChange::StopAll))
        );
        // `volume` exists on both sides, which is exactly the case the shape
        // test has to get right.
        assert_eq!(
            Action::parse("sampler volume 0.5").unwrap(),
            Action::Mixer(MixerAction::Sampler(SamplerChange::Volume(0.5)))
        );
        assert_eq!(
            Action::parse("sampler 4 volume 0.5").unwrap(),
            Action::Mixer(MixerAction::Sample {
                slot: 4,
                change: SampleChange::Volume(0.5),
            })
        );
    }

    #[test]
    fn every_sampler_change_reaches_its_own_action() {
        use SampleChange as C;
        let cases = [
            ("trigger", C::Trigger),
            ("release", C::Release),
            ("stop", C::Stop),
            ("clear", C::Clear),
            ("one_shot", C::SetMode(TriggerMode::OneShot)),
            ("hold", C::SetMode(TriggerMode::Hold)),
            ("loop", C::SetMode(TriggerMode::Loop)),
            ("stutter", C::SetMode(TriggerMode::Stutter)),
            ("master", C::Route(SampleOutput::Master)),
            ("cue", C::Route(SampleOutput::Cue)),
            ("sync", C::SetSync(true)),
            ("sync_off", C::SetSync(false)),
        ];
        for (word, change) in cases {
            assert_eq!(
                Action::parse(&format!("sampler 1 {word}")).unwrap(),
                Action::Mixer(MixerAction::Sample { slot: 1, change }),
                "parsing `sampler 1 {word}`"
            );
        }
    }

    /// Firing the wrong sample plays it to a room, so an out-of-range slot has
    /// to be refused rather than clamped onto slot 1.
    #[test]
    fn an_out_of_range_sampler_slot_is_refused() {
        assert_eq!(
            Action::parse("sampler 0 trigger"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(
            Action::parse("sampler 9 trigger"),
            Err(ParseError::BadArgument)
        );
        assert_eq!(Action::parse("sampler"), Err(ParseError::MissingVerb));
        assert_eq!(Action::parse("sampler 1"), Err(ParseError::MissingArgument));
        assert_eq!(
            Action::parse("sampler 1 sutter"),
            Err(ParseError::UnknownMode("sutter".to_owned()))
        );
    }

    #[test]
    fn every_sampler_action_round_trips_through_its_text_form() {
        let mut actions: Vec<Action> = vec![
            Action::Mixer(MixerAction::Sampler(SamplerChange::Bank(3))),
            Action::Mixer(MixerAction::Sampler(SamplerChange::Volume(0.25))),
            Action::Mixer(MixerAction::Sampler(SamplerChange::StopAll)),
        ];
        for change in [
            SampleChange::Trigger,
            SampleChange::Release,
            SampleChange::Stop,
            SampleChange::Clear,
            SampleChange::Volume(0.75),
            SampleChange::Route(SampleOutput::Master),
            SampleChange::Route(SampleOutput::Cue),
            SampleChange::SetSync(true),
            SampleChange::SetSync(false),
        ] {
            actions.push(Action::Mixer(MixerAction::Sample { slot: 2, change }));
        }
        for mode in TriggerMode::ALL {
            actions.push(Action::Mixer(MixerAction::Sample {
                slot: 2,
                change: SampleChange::SetMode(mode),
            }));
        }
        for action in actions {
            let text = action.to_string();
            assert_eq!(
                Action::parse(&text).unwrap(),
                action,
                "`{text}` did not survive the round trip"
            );
        }
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

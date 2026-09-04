//! Deciding what the assistant should do next, and whether it may.
//!
//! Everything else in the assistant feature answers a question in isolation:
//! [`crate::plan`] says how to mix two records, `dj_library::suggest` says which
//! record, [`dj_assistant::Posture`] says how much to do, and
//! [`dj_assistant::Takeover`] says whose hands are on the controls. This joins
//! them into one answer: **given all of that, what is the next thing to do right
//! now?**
//!
//! # It decides, it does not act
//!
//! [`next_step`] is a pure function returning a [`Step`]. Nothing here touches
//! a deck, sends a command, or knows the bus exists. That is not fastidiousness
//! -- it is what makes the hardest behaviour in the feature testable at all. A
//! DJ needs to know that Prepare will not move a fader, and the way to know it
//! is a test that asks what Prepare would do and reads the answer, rather than
//! a test that runs an engine and listens.
//!
//! # The order of the questions matters
//!
//! 1. **Is the human holding this?** If so, nothing. Checked first, before
//!    anything else is even considered, so no other rule can accidentally
//!    override it.
//! 2. **Does the posture permit it?** Prepare stages and does not act;
//!    Autopilot mixes.
//! 3. **Is there anything worth doing?** Most of the time the answer is no,
//!    and returning [`Step::Nothing`] is the common case rather than a
//!    failure.
//!
//! # It never surprises
//!
//! The step is returned with the reasoning that produced it, so an interface
//! can show what is about to happen before it happens. An assistant that acts
//! and then explains is one a DJ cannot get ahead of.

use crate::plan::{self, Incoming, Outgoing};
use dj_assistant::{Occasion, Posture, Takeover};
use dj_core::{DeckId, ParamId, TrackId, action::TransitionStyle, param::DeckParam};

/// Where the set is, as the autopilot needs it.
#[derive(Debug, Clone)]
pub struct Situation {
    pub posture: Posture,
    pub occasion: Occasion,
    /// The deck the room is hearing.
    pub live: DeckId,
    /// Where the live deck is and how it is set up.
    pub outgoing: Outgoing,
    /// A deck that is free to stage into, if there is one.
    pub idle: Option<DeckId>,
    /// What is loaded on the idle deck, and what it is.
    pub staged: Option<(TrackId, Incoming)>,
    /// The next record, from the setlist or the suggester.
    pub next: Option<TrackId>,
    /// Trim needed to bring the staged track to the live one's level, in dB.
    pub gain_offset_db: Option<f64>,
    /// The transition a human has set up, if one is being held.
    ///
    /// [`crate::automix::Setup`] rather than a third shape for the same thing.
    /// §68's whole complaint is that a transition was several separate ideas
    /// in several modules; the automix and the autopilot answering the same
    /// question from the same struct is the point of having the object at all.
    pub set_up: Option<crate::automix::Setup>,
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Put a record on a deck that is not playing.
    Stage { deck: DeckId, track: TrackId },
    /// Move a staged deck's playhead to the phrase the mix will start from.
    Cue { deck: DeckId, beat: i64 },
    /// Match the staged deck's level to the live one.
    MatchGain { deck: DeckId, db: f64 },
    /// Start the mix.
    Mix {
        from: DeckId,
        to: DeckId,
        style: TransitionStyle,
        beats: u32,
    },
    /// Nothing to do. The common case, not a failure.
    Nothing,
}

/// A step, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub step: Step,
    /// Short, for an interface that shows what is about to happen. Written to
    /// be read at a glance in a dark room, not to be complete.
    pub because: String,
}

impl Decision {
    fn nothing(because: &str) -> Self {
        Self {
            step: Step::Nothing,
            because: because.to_owned(),
        }
    }
}

/// How close to the end of the outgoing record the autopilot starts staging.
///
/// Ninety seconds. Long enough to decode a track and cue it without hurrying,
/// short enough that the choice is made against a set that has nearly caught up
/// with it -- staging ten minutes out means choosing the next record for a room
/// that no longer exists.
const STAGE_WITHIN_SECONDS: f64 = 90.0;

/// What to do next.
///
/// `takeover` is consulted before anything else. See the module docs on why the
/// order of the questions matters.
#[must_use]
pub fn next_step(situation: &Situation, takeover: &Takeover) -> Decision {
    let Situation {
        posture, occasion, ..
    } = situation;

    if *posture == Posture::Off || *posture == Posture::Watch {
        return Decision::nothing("the assistant is not acting at this level");
    }

    // A deck with no record on it has zero length, which is not the same as a
    // record that has just finished -- and treating it as one had the autopilot
    // staging into an idle deck before anything had ever been played. Checked
    // on the length rather than on `remaining`, because both are zero and only
    // one of them means "empty".
    if situation.outgoing.length <= 0.0 || situation.outgoing.length.is_nan() {
        return Decision::nothing("nothing is playing");
    }
    let remaining = remaining_seconds(&situation.outgoing);
    if !remaining.is_finite() {
        return Decision::nothing("nothing is playing");
    }

    // -- staging ----------------------------------------------------------
    //
    // Everything from Prepare upwards does this. It is the whole of what
    // Prepare is, and the part of Autopilot that happens first.
    if posture.may_stage()
        && let Some(idle) = situation.idle
    {
        {
            if situation.staged.is_none() {
                if remaining > STAGE_WITHIN_SECONDS {
                    return Decision::nothing(&format!(
                        "{:.0}s left; staging inside {STAGE_WITHIN_SECONDS:.0}s",
                        remaining
                    ));
                }
                let Some(track) = situation.next else {
                    return Decision::nothing("nothing chosen to play next");
                };
                // Loading is not a live control -- the deck is silent -- so
                // takeover of the *playing* deck does not block it. Takeover of
                // the idle deck does: a DJ who has cued something themselves
                // has chosen it.
                if !takeover.may_move(ParamId::Deck(idle, DeckParam::Position)) {
                    return Decision::nothing("you have that deck");
                }
                return Decision {
                    step: Step::Stage { deck: idle, track },
                    because: format!(
                        "{remaining:.0}s left on deck {}",
                        situation.live.human_number()
                    ),
                };
            }

            // Staged but not levelled. Gain on a silent deck is still not
            // something the room hears.
            if let Some(db) = situation.gain_offset_db
                && db.abs() > 0.5
                && takeover.may_move(ParamId::Deck(idle, DeckParam::GainDb))
            {
                return Decision {
                    step: Step::MatchGain { deck: idle, db },
                    because: format!(
                        "{db:+.1} dB to match deck {}",
                        situation.live.human_number()
                    ),
                };
            }
        }
    }

    // -- mixing -----------------------------------------------------------
    //
    // Only Autopilot, and only with somewhere to go.
    if !posture.may_mix() {
        return Decision::nothing(match posture {
            Posture::Prepare => "ready when you are",
            _ => "waiting",
        });
    }

    let (Some(idle), Some((_, incoming))) = (situation.idle, situation.staged.as_ref()) else {
        return Decision::nothing("nothing staged to mix into");
    };

    // The crossfader is the control a mix moves. If the human has it, there is
    // no mix to perform -- and this is the check that matters most, because it
    // is the one whose failure an audience hears.
    if !takeover.may_move(ParamId::Global(dj_core::param::GlobalParam::Crossfader)) {
        return Decision::nothing("you have the crossfader");
    }

    // A transition the DJ set up is the answer to this question, and the
    // planner is what djmanzo does when nobody has decided. Two things
    // deciding the same mix differently is what §68 exists to stop -- and the
    // automix already performs the held one, so an autopilot planning its own
    // would be announcing a mix other than the one about to happen.
    let held = situation
        .set_up
        .filter(|s| s.outgoing == situation.live && s.incoming == idle);

    let (start, style, beats, mine) = match held {
        // Their length, not the occasion's. A human who chose thirty-two beats
        // has chosen; §45's rule does not stop at the controls.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(set_up) => (
            set_up.start,
            set_up.style,
            set_up.beats.max(1.0) as u32,
            false,
        ),
        None => {
            let Some(planned) = plan::plan(&situation.outgoing, incoming) else {
                return Decision::nothing("no sensible place left to mix");
            };
            // The plan says where; the occasion says how long. The planner
            // works from the two records, the occasion from the room, and
            // neither knows what the other knows.
            (
                planned.start_frame,
                planned.style,
                occasion.transition_beats().min(planned.length_beats),
                true,
            )
        }
    };

    let position = situation.outgoing.position;
    let beat_frames = situation.outgoing.sample_rate.as_f64() * 60.0 / situation.outgoing.bpm;
    let until = (start - position) / beat_frames;

    if until > 1.0 {
        return Decision::nothing(&format!("mixing in {until:.0} beats"));
    }

    Decision {
        step: Step::Mix {
            from: situation.live,
            to: idle,
            style,
            beats,
        },
        because: if mine {
            format!("{style:?} over {beats} beats")
        } else {
            format!("the {style} you set up, over {beats} beats")
        },
    }
}

/// Seconds of the outgoing record still to play.
fn remaining_seconds(out: &Outgoing) -> f64 {
    let rate = out.sample_rate.as_f64();
    if rate <= 0.0 {
        return f64::NAN;
    }
    (out.length - out.position) / rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Mode, MusicalKey, Phrase, SampleRate};

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn key() -> Option<MusicalKey> {
        MusicalKey::new(8, Mode::Minor)
    }

    /// A live deck with `seconds_left` still to play.
    fn outgoing(seconds_left: f64) -> Outgoing {
        let total = 300.0 * SR.as_f64();
        Outgoing {
            position: total - seconds_left * SR.as_f64(),
            length: total,
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: key(),
            sample_rate: SR,
            grid_anchor: 0.0,
        }
    }

    fn incoming() -> Incoming {
        Incoming {
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: key(),
        }
    }

    /// A situation positioned exactly where the planner says the mix starts.
    ///
    /// Built by asking the planner and moving the playhead to its answer,
    /// rather than by guessing a number of seconds. Three tests were failing
    /// against a fixture that had sixteen seconds left and a mix point still
    /// ahead of it -- so "autopilot mixes" was being tested against a situation
    /// in which not mixing was correct.
    fn ready_to_mix(posture: Posture) -> Situation {
        let base = situation(posture);
        let planned = plan::plan(&base.outgoing, &incoming())
            .expect("the fixture must be mixable or it tests nothing");
        Situation {
            outgoing: Outgoing {
                position: planned.start_frame,
                ..base.outgoing
            },
            staged: Some((TrackId::from_bytes([9; 32]), incoming())),
            ..base
        }
    }

    /// A situation with a record ending soon and somewhere to go.
    fn situation(posture: Posture) -> Situation {
        Situation {
            posture,
            occasion: Occasion::Open,
            live: deck(1),
            outgoing: outgoing(60.0),
            idle: Some(deck(2)),
            staged: None,
            next: Some(TrackId::from_bytes([9; 32])),
            gain_offset_db: None,
            set_up: None,
        }
    }

    /// A transition set up out of deck 1 into deck 2, at a moment and a length
    /// the planner would not have chosen.
    ///
    /// Sixty-four beats where the occasion allows far fewer, and a cut where
    /// two matched records would get a blend: a fixture that agreed with the
    /// planner would pass whether or not any of this existed.
    fn set_up_at(start: f64) -> crate::automix::Setup {
        crate::automix::Setup {
            outgoing: deck(1),
            incoming: deck(2),
            start,
            beats: 64.0,
            style: TransitionStyle::Cut,
        }
    }

    /// **A transition the DJ set up is the one the autopilot performs.**
    ///
    /// The automix already runs the held transition, so an autopilot planning
    /// its own would be announcing one mix while another was about to happen —
    /// two answers to the same question, which is the whole of what §68 is
    /// complaining about. The fixture's set-up transition is a *cut over 64
    /// beats*, which is neither what the planner would choose for two matched
    /// records nor what the occasion would allow.
    #[test]
    fn it_performs_the_transition_that_was_set_up() {
        let base = ready_to_mix(Posture::Autopilot);
        let mine = next_step(&base, &Takeover::default());
        let Step::Mix { style, beats, .. } = mine.step else {
            panic!("the fixture is not at a mix point: {mine:?}");
        };
        assert_ne!(
            (style, beats),
            (TransitionStyle::Cut, 64),
            "the planner already chose what the set-up transition says, so \
             this fixture cannot tell the two apart"
        );

        let theirs = next_step(
            &Situation {
                set_up: Some(set_up_at(base.outgoing.position)),
                ..base
            },
            &Takeover::default(),
        );
        assert_eq!(
            theirs.step,
            Step::Mix {
                from: deck(1),
                to: deck(2),
                style: TransitionStyle::Cut,
                beats: 64,
            },
            "it planned its own mix over the one the DJ set up"
        );
        assert!(
            theirs.because.contains("you set up"),
            "it did not say whose mix it was performing: {}",
            theirs.because
        );
    }

    /// **A human's length is not trimmed by the occasion.**
    ///
    /// The occasion caps djmanzo's *own* transitions, which is reasoning about
    /// a room. A DJ who set up sixty-four beats has decided, and §45's rule
    /// does not stop at the controls.
    #[test]
    fn the_occasion_does_not_shorten_a_transition_a_human_set_up() {
        let base = ready_to_mix(Posture::Autopilot);
        let cautious = Situation {
            // Learning wants eight-beat mixes it can watch happen; the set-up
            // one is sixty-four.
            occasion: Occasion::Learning,
            set_up: Some(set_up_at(base.outgoing.position)),
            ..base
        };
        let Step::Mix { beats, .. } = next_step(&cautious, &Takeover::default()).step else {
            panic!("it did not mix");
        };
        assert_eq!(beats, 64, "the occasion trimmed a length a human chose");
    }

    /// A transition set up between two *other* decks is not this mix, and the
    /// autopilot goes on planning its own.
    #[test]
    fn a_transition_for_other_decks_is_not_this_one() {
        let base = ready_to_mix(Posture::Autopilot);
        let elsewhere = Situation {
            set_up: Some(crate::automix::Setup {
                incoming: deck(3),
                ..set_up_at(base.outgoing.position)
            }),
            ..base
        };
        let Step::Mix { style, beats, .. } = next_step(&elsewhere, &Takeover::default()).step
        else {
            panic!("it did not mix");
        };
        assert_ne!(
            (style, beats),
            (TransitionStyle::Cut, 64),
            "it performed a transition set up between two other decks"
        );
    }

    /// **Off and Watch do nothing at all.**
    #[test]
    fn the_quiet_postures_do_nothing() {
        for posture in [Posture::Off, Posture::Watch] {
            let decision = next_step(&situation(posture), &Takeover::new());
            assert_eq!(decision.step, Step::Nothing, "{} acted", posture.name());
        }
    }

    /// **Prepare stages, and stops there.**
    ///
    /// The whole of what Prepare is. A DJ who set this level and found the
    /// crossfader moving would never trust it again.
    #[test]
    fn prepare_stages_and_goes_no_further() {
        let mut state = situation(Posture::Prepare);
        let takeover = Takeover::new();

        // With nothing staged, it stages.
        assert_eq!(
            next_step(&state, &takeover).step,
            Step::Stage {
                deck: deck(2),
                track: TrackId::from_bytes([9; 32])
            }
        );

        // With something staged and levelled, it stops -- it does not mix.
        state.staged = Some((TrackId::from_bytes([9; 32]), incoming()));
        assert_eq!(
            next_step(&state, &takeover).step,
            Step::Nothing,
            "Prepare performed a transition"
        );
    }

    /// **Autopilot mixes, and Prepare does not, from the same situation.**
    ///
    /// The two run against identical input, so the only difference is the
    /// posture. Without this pair the postures could be labels.
    #[test]
    fn only_autopilot_mixes_from_the_same_situation() {
        let takeover = Takeover::new();

        assert!(
            matches!(
                next_step(&ready_to_mix(Posture::Autopilot), &takeover).step,
                Step::Mix { .. }
            ),
            "autopilot did not mix when everything was ready"
        );
        assert_eq!(
            next_step(&ready_to_mix(Posture::Prepare), &takeover).step,
            Step::Nothing
        );
    }

    /// **A hand on the crossfader stops the mix.**
    ///
    /// The check whose failure an audience hears. Tested against a situation
    /// that would otherwise definitely mix, so a pass means the takeover did
    /// the work and not some other condition.
    #[test]
    fn a_hand_on_the_crossfader_stops_the_mix() {
        let ready = ready_to_mix(Posture::Autopilot);
        let free = Takeover::new();
        assert!(
            matches!(next_step(&ready, &free).step, Step::Mix { .. }),
            "the fixture does not mix even with nothing held, so this test \
             would pass for the wrong reason"
        );

        let mut held = Takeover::new();
        held.touched(ParamId::Global(dj_core::param::GlobalParam::Crossfader));
        assert_eq!(
            next_step(&ready, &held).step,
            Step::Nothing,
            "the assistant moved the crossfader out of the DJ's hand"
        );
    }

    /// **A hand on the idle deck stops it being staged over.**
    ///
    /// A DJ who cued something up themselves has chosen it, and having it
    /// replaced is worse than having nothing staged at all.
    #[test]
    fn a_hand_on_the_idle_deck_stops_it_being_staged_over() {
        let state = situation(Posture::Prepare);
        let mut held = Takeover::new();
        held.touched(ParamId::Deck(deck(2), DeckParam::Position));

        assert_eq!(next_step(&state, &held).step, Step::Nothing);
    }

    /// **It does not stage ten minutes early.**
    ///
    /// Choosing the next record long in advance is choosing it for a room that
    /// will not exist by the time it plays.
    #[test]
    fn staging_waits_until_the_record_is_nearly_over() {
        let early = Situation {
            outgoing: outgoing(600.0),
            ..situation(Posture::Prepare)
        };
        assert_eq!(next_step(&early, &Takeover::new()).step, Step::Nothing);
    }

    /// **Level matching happens on the silent deck, before the mix.**
    ///
    /// Riding the trim during a transition is audible; doing it beforehand is
    /// not, and it is exactly the tedious thing worth automating.
    #[test]
    fn a_staged_deck_is_levelled_before_it_is_mixed() {
        let state = Situation {
            staged: Some((TrackId::from_bytes([9; 32]), incoming())),
            gain_offset_db: Some(-3.5),
            ..situation(Posture::Prepare)
        };
        assert_eq!(
            next_step(&state, &Takeover::new()).step,
            Step::MatchGain {
                deck: deck(2),
                db: -3.5
            }
        );
    }

    /// A trim already close enough is left alone: nudging a fader by a fifth of
    /// a decibel is noise, not assistance.
    #[test]
    fn a_level_already_close_enough_is_left_alone() {
        let state = Situation {
            staged: Some((TrackId::from_bytes([9; 32]), incoming())),
            gain_offset_db: Some(0.2),
            ..situation(Posture::Prepare)
        };
        assert_eq!(next_step(&state, &Takeover::new()).step, Step::Nothing);
    }

    /// **The occasion shortens the mix, and cannot lengthen it past the plan.**
    ///
    /// The planner works from the two records and knows how much track is
    /// left; the occasion works from the room. Where they disagree the planner
    /// wins, because it is the one that knows the mix would run off the end.
    #[test]
    fn the_occasion_can_shorten_a_mix_but_not_overrun_the_track() {
        let peak = Situation {
            occasion: Occasion::Peak,
            ..ready_to_mix(Posture::Autopilot)
        };
        let background = Situation {
            occasion: Occasion::Background,
            ..peak.clone()
        };

        let Step::Mix { beats: at_peak, .. } = next_step(&peak, &Takeover::new()).step else {
            panic!("peak did not mix");
        };
        let Step::Mix {
            beats: in_background,
            ..
        } = next_step(&background, &Takeover::new()).step
        else {
            panic!("background did not mix");
        };

        assert!(
            at_peak <= 16,
            "a peak-time mix was planned at {at_peak} beats"
        );
        // Background asks for 64, but the planner will not have offered that
        // much with sixteen seconds left.
        assert!(
            in_background <= 32,
            "the occasion overrode the planner and ran past the end of the \
             record: {in_background} beats"
        );
    }

    /// **Nothing staged means nothing to mix into**, and it says so rather
    /// than mixing into silence.
    #[test]
    fn autopilot_with_nothing_staged_does_not_mix() {
        let state = Situation {
            staged: None,
            idle: None,
            ..ready_to_mix(Posture::Autopilot)
        };
        assert_eq!(next_step(&state, &Takeover::new()).step, Step::Nothing);
    }

    /// **Every decision carries a reason**, so an interface can say what is
    /// about to happen before it happens. An assistant that acts and then
    /// explains is one a DJ cannot get ahead of.
    #[test]
    fn every_decision_says_why() {
        let cases = [
            situation(Posture::Off),
            situation(Posture::Prepare),
            Situation {
                outgoing: outgoing(600.0),
                ..situation(Posture::Prepare)
            },
            ready_to_mix(Posture::Autopilot),
        ];
        for state in cases {
            let decision = next_step(&state, &Takeover::new());
            assert!(
                !decision.because.trim().is_empty(),
                "a {} decision gave no reason",
                state.posture.name()
            );
        }
    }

    /// With no record playing there is nothing to reason about, and it does not
    /// divide by a zero sample rate on the way to saying so.
    #[test]
    fn a_dead_deck_produces_nothing() {
        let state = Situation {
            outgoing: Outgoing {
                sample_rate: SR,
                length: 0.0,
                position: 0.0,
                ..outgoing(0.0)
            },
            ..situation(Posture::Autopilot)
        };
        assert_eq!(next_step(&state, &Takeover::new()).step, Step::Nothing);
    }
}

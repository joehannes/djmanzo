//! Where to start a mix, how long to take, and which way to do it.
//!
//! The suggester in `dj_library::suggest` answers *what* to play next. This
//! answers the two questions that follow, and they are not the same question:
//! a record that is a perfect harmonic match is still a bad mix if it comes in
//! four beats before the outgoing track's phrase ends.
//!
//! Beside `automix` rather than inside it on purpose. Automix *runs* a
//! transition on a fixed style the DJ chose in advance; this *decides* one from
//! what the two tracks actually are. Keeping them apart means the planner can
//! be asked for an opinion without anything moving, which is what a DJ wants
//! when the answer is going on a screen rather than into the mix.
//!
//! # Reasons are typed, as everywhere else
//!
//! Same principle as the suggester and as
//! [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md): a plan
//! that says `Reason::LandsOnPhrase { beat: 384 }` can be checked against the
//! waveform; one that says "sounds good here" cannot.
//!
//! # What it cannot know
//!
//! **Where the outgoing track's outro actually is.** A phrase boundary near the
//! end is the best structural guess available, and it is a guess: plenty of
//! records have a vocal over the last sixteen bars, and no analysis here can
//! hear that. The plan says which boundary it chose and how much track is left,
//! so the DJ can disagree with the specific thing rather than the whole answer.
//!
//! **Whether the two records suit each other musically.** Key and tempo are
//! arithmetic; taste is not.

use dj_core::{MusicalKey, Phrase, SampleRate, action::TransitionStyle};

/// The track going out, as the planner needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outgoing {
    /// Where the playhead is now, in frames.
    pub position: f64,
    /// Total length in frames.
    pub length: f64,
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub key: Option<MusicalKey>,
    pub sample_rate: SampleRate,
    /// Frame position of a beat, from which every other beat follows.
    pub grid_anchor: f64,
}

/// The track coming in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Incoming {
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub key: Option<MusicalKey>,
}

/// Why the planner chose what it chose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// The mix starts on a phrase boundary of the outgoing track.
    LandsOnPhrase { beat: i64 },
    /// No phrase structure, so the start is a bar line instead. Weaker, and
    /// said so rather than presented as the same thing.
    LandsOnBar { beat: i64 },
    /// How much of the outgoing track is left after the mix starts.
    Remaining { beats: f64 },
    /// The tempos are close enough to ride the pitch fader.
    TemposMatch { from: f64, to: f64 },
    /// Too far apart to blend; the plan cuts instead.
    TemposClash { from: f64, to: f64 },
    /// The keys sit together.
    KeysMatch,
    /// The keys fight, so a long blend would expose it.
    KeysClash,
    /// Not enough track left for a long transition.
    Rushed { beats_left: f64 },
}

/// A proposed transition.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Beat index in the outgoing track, counted from its grid anchor, where
    /// the mix should begin.
    pub start_beat: i64,
    /// Frame position of that beat, so the interface can draw it without
    /// redoing the arithmetic and landing a pixel off.
    pub start_frame: f64,
    /// How long the transition runs, in beats.
    pub length_beats: u32,
    pub style: TransitionStyle,
    pub reasons: Vec<Reason>,
}

/// Transition lengths the planner will propose, longest first.
///
/// Whole phrases, not round numbers: a 24-beat blend ends in the middle of a
/// phrase, which is the thing this module exists to avoid. Thirty-two is a
/// comfortable DJ blend, sixteen is brisk, eight is nearly a cut.
const LENGTHS: [u32; 3] = [32, 16, 8];

/// How close two tempos must be to blend rather than cut.
///
/// Six percent, matching the deck's comfortable pitch range in
/// `dj_library::suggest` -- the same physical constraint, so the two must not
/// drift apart. Beyond it a long blend means two audibly different tempos
/// running together for eight bars.
const TEMPO_TOLERANCE: f64 = 0.06;

/// Beats of outgoing track that must remain *after* the transition ends.
///
/// Zero would mean planning a mix that finishes exactly as the file runs out,
/// which leaves no room for the transition to be late -- and it always is, by a
/// beat or two, because a human presses the button.
const TAIL_MARGIN: f64 = 8.0;

/// Plan a transition from `out` into `into`.
///
/// `None` when there is nothing sensible to propose: no grid, a track already
/// past its last usable phrase, or a length that will not fit. A planner that
/// always answers is a planner that answers wrongly near the end of a record,
/// which is exactly when it is being read.
#[must_use]
pub fn plan(out: &Outgoing, into: &Incoming) -> Option<Plan> {
    let beat_frames = beat_frames(out.bpm, out.sample_rate)?;
    let remaining_beats = (out.length - out.position) / beat_frames;
    if !remaining_beats.is_finite() || remaining_beats <= TAIL_MARGIN {
        return None;
    }

    let mut reasons = Vec::new();

    // Tempo and key first: they decide the style, and the style decides how
    // long the mix wants to be before the track's remaining length trims it.
    let tempos_match = ratio_within(out.bpm, into.bpm, TEMPO_TOLERANCE);
    reasons.push(if tempos_match {
        Reason::TemposMatch {
            from: out.bpm,
            to: into.bpm,
        }
    } else {
        Reason::TemposClash {
            from: out.bpm,
            to: into.bpm,
        }
    });

    let keys_match = match (out.key, into.key) {
        (Some(a), Some(b)) => a.is_compatible_with(b),
        // Unknown keys are not a clash. Treating them as one would make every
        // unanalysed track a cut, which is a guess dressed as a decision.
        _ => true,
    };
    if out.key.is_some() && into.key.is_some() {
        reasons.push(if keys_match {
            Reason::KeysMatch
        } else {
            Reason::KeysClash
        });
    }

    let style = choose_style(tempos_match, keys_match);

    // The longest transition that leaves the tail margin intact.
    let usable = remaining_beats - TAIL_MARGIN;
    #[allow(clippy::cast_precision_loss)]
    let length = LENGTHS
        .into_iter()
        .find(|&l| f64::from(l) <= usable)
        .unwrap_or(*LENGTHS.last().expect("LENGTHS is not empty"));
    #[allow(clippy::cast_precision_loss)]
    if f64::from(length) > usable {
        reasons.push(Reason::Rushed {
            beats_left: remaining_beats,
        });
    }

    // Where. The last boundary that still leaves room for the whole
    // transition, so the mix ends before the track does rather than being cut
    // off by it.
    let current = ((out.position - out.grid_anchor) / beat_frames).floor() as i64;
    #[allow(clippy::cast_possible_truncation)]
    let last_usable = current + (usable - f64::from(length)).max(0.0) as i64;

    let start_beat = match out.phrase {
        Some(phrase) => {
            let within = i64::from(phrase.beat_within(last_usable));
            let boundary = last_usable - within;
            // Never behind the playhead: a plan to start the mix in the past is
            // not a plan.
            if boundary < current {
                current
            } else {
                boundary
            }
        }
        None => last_usable,
    };
    reasons.push(match out.phrase {
        Some(p) if p.starts_at(start_beat) => Reason::LandsOnPhrase { beat: start_beat },
        _ => Reason::LandsOnBar { beat: start_beat },
    });

    #[allow(clippy::cast_precision_loss)]
    let start_frame = out.grid_anchor + start_beat as f64 * beat_frames;
    reasons.push(Reason::Remaining {
        beats: (out.length - start_frame) / beat_frames,
    });

    Some(Plan {
        start_beat,
        start_frame,
        length_beats: length,
        style,
        reasons,
    })
}

/// Which way to do it.
///
/// Blend is the default because it is what a DJ does by hand. The two
/// departures from it are both about not holding a problem open for eight bars:
/// mismatched tempos and clashing keys are each tolerable for a moment and
/// tiring for a phrase.
fn choose_style(tempos_match: bool, keys_match: bool) -> TransitionStyle {
    match (tempos_match, keys_match) {
        (true, true) => TransitionStyle::Blend,
        // Tempos work, keys fight: get through it quickly, and let the outgoing
        // track dissolve rather than sit against the new one.
        (true, false) => TransitionStyle::Echo,
        // Tempos do not work. Nothing overlapping will help, whatever the keys
        // do -- this is the case `Cut` exists for, and it is the honest answer.
        (false, _) => TransitionStyle::Cut,
    }
}

/// Frames per beat, or `None` if the tempo is not a tempo.
fn beat_frames(bpm: f64, rate: SampleRate) -> Option<f64> {
    let frames = rate.as_f64() * 60.0 / bpm;
    (frames.is_finite() && frames > 0.0).then_some(frames)
}

/// Whether `b` is within `tolerance` of `a`, or of half or double it.
fn ratio_within(a: f64, b: f64, tolerance: f64) -> bool {
    [1.0, 0.5, 2.0]
        .into_iter()
        .any(|factor| (b / (a * factor) - 1.0).abs() <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Mode;

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    /// Frames per beat, **derived** rather than written down.
    ///
    /// It was a literal 22 050 first -- 120 BPM at 44.1 kHz -- and
    /// `SampleRate::DEFAULT` is 48 kHz, so every fixture position was a beat
    /// index the planner did not agree with. The failure looked like an
    /// off-by-thirty bug in the planner and was a wrong constant in the test.
    fn beat() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    /// An outgoing track `beats_in` beats along, `total_beats` long.
    fn outgoing(beats_in: f64, total_beats: f64) -> Outgoing {
        Outgoing {
            position: beats_in * beat(),
            length: total_beats * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: Some(key(8, Mode::Minor)),
            sample_rate: SR,
            grid_anchor: 0.0,
        }
    }

    fn incoming(bpm: f64, k: Option<MusicalKey>) -> Incoming {
        Incoming {
            bpm,
            phrase: Phrase::new(16, 0),
            key: k,
        }
    }

    /// **The mix starts on a phrase boundary.**
    ///
    /// The whole reason the planner exists. A transition that begins four beats
    /// into a phrase brings the new record in against the middle of a musical
    /// idea, and it sounds like a mistake even when every other number is
    /// right.
    #[test]
    fn the_transition_starts_on_a_phrase_boundary() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");

        assert_eq!(
            plan.start_beat % 16,
            0,
            "the mix was planned to start on beat {}, which is {} beats into a phrase",
            plan.start_beat,
            plan.start_beat % 16
        );
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::LandsOnPhrase { .. })),
            "it landed on a phrase and did not say so: {:?}",
            plan.reasons
        );
    }

    /// **An offset phrase start moves the plan with it.**
    ///
    /// A track opening with a five-beat pickup has its phrases at 5, 21, 37. A
    /// planner that assumed phrases begin at the grid anchor would start every
    /// mix on such a record eleven beats early -- on a beat, so it would look
    /// right on the waveform, and wrong in the room.
    #[test]
    fn an_offset_phrase_start_moves_the_plan() {
        let out = Outgoing {
            phrase: Phrase::new(16, 5),
            ..outgoing(100.0, 400.0)
        };
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_eq!(
            plan.start_beat.rem_euclid(16),
            5,
            "started at beat {}, which is not a phrase start for this track",
            plan.start_beat
        );
    }

    /// **Mismatched tempos get a cut, not a blend.**
    ///
    /// Nothing overlapping helps when the two records are at audibly different
    /// speeds. `Cut` is the honest answer and the style exists for exactly
    /// this.
    #[test]
    fn tempos_too_far_apart_are_cut() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(145.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Cut);
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::TemposClash { .. })),
            "it cut without saying why: {:?}",
            plan.reasons
        );
    }

    /// Half and double time are a match, not a clash: 120 into 60 is an
    /// ordinary move and the plain ratio cannot see it.
    #[test]
    fn half_time_is_not_a_tempo_clash() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(60.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_ne!(
            plan.style,
            TransitionStyle::Cut,
            "120 into 60 was treated as unmixable"
        );
    }

    /// **Clashing keys get a shorter, dissolving transition.**
    ///
    /// Two records whose keys fight are tolerable for a moment and tiring for
    /// eight bars. Echo lets the outgoing one dissolve instead of sitting
    /// against the new one.
    #[test]
    fn clashing_keys_dissolve_rather_than_blend() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, Some(key(2, Mode::Major)))).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Echo);
        assert!(plan.reasons.contains(&Reason::KeysClash));
    }

    /// **An unknown key is not a clash.**
    ///
    /// Treating it as one would make every unanalysed track a cut, which is a
    /// guess dressed up as a decision.
    #[test]
    fn an_unknown_key_still_blends() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, None)).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Blend);
        assert!(
            !plan.reasons.contains(&Reason::KeysClash),
            "an unknown key was reported as a clash"
        );
    }

    /// **The mix finishes before the track does.**
    ///
    /// With a margin, because a human presses the button a beat or two late and
    /// a transition that was planned to end exactly at the last sample ends
    /// with silence instead.
    #[test]
    fn the_transition_ends_before_the_track_runs_out() {
        let total = 400.0;
        let out = outgoing(360.0, total);
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");

        let end_beat = plan.start_beat + i64::from(plan.length_beats);
        assert!(
            (end_beat as f64) <= total,
            "the mix was planned to end at beat {end_beat} of a {total}-beat track"
        );
    }

    /// **Near the end, the transition is shortened rather than refused.**
    ///
    /// A DJ forty beats from the end of a record still has to get out of it.
    /// Answering "no plan" there would be the planner giving up at the one
    /// moment it is being read.
    #[test]
    fn a_short_tail_gets_a_shorter_transition() {
        let roomy = plan(
            &outgoing(100.0, 400.0),
            &incoming(120.0, Some(key(8, Mode::Minor))),
        )
        .expect("a plan");
        let tight = plan(
            &outgoing(370.0, 400.0),
            &incoming(120.0, Some(key(8, Mode::Minor))),
        )
        .expect("a plan near the end");

        assert!(
            tight.length_beats < roomy.length_beats,
            "a 30-beat tail got the same {} beat transition as a 300-beat one",
            roomy.length_beats
        );
    }

    /// **The start is never behind the playhead.**
    ///
    /// A plan to begin the mix in the past is not a plan. This is the case that
    /// arises when the track is nearly over: the last usable boundary has
    /// already gone by.
    #[test]
    fn the_plan_never_starts_in_the_past() {
        for beats_in in [300.0, 350.0, 380.0, 390.0] {
            let out = outgoing(beats_in, 400.0);
            let Some(plan) = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))) else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation)]
            let current = (beats_in) as i64;
            assert!(
                plan.start_beat >= current,
                "at beat {current} it planned to start at {}",
                plan.start_beat
            );
        }
    }

    /// Past the tail margin there is nothing to propose, and it says so rather
    /// than inventing a transition that cannot happen.
    #[test]
    fn a_track_at_its_end_gets_no_plan() {
        assert!(
            plan(
                &outgoing(397.0, 400.0),
                &incoming(120.0, Some(key(8, Mode::Minor)))
            )
            .is_none(),
            "a track three beats from the end was given a transition plan"
        );
    }

    /// A tempo that is not a tempo produces no plan rather than an infinity.
    #[test]
    fn a_nonsense_tempo_gets_no_plan() {
        for bad in [0.0, -120.0, f64::NAN] {
            let out = Outgoing {
                bpm: bad,
                ..outgoing(100.0, 400.0)
            };
            assert!(
                plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).is_none(),
                "{bad} BPM produced a plan"
            );
        }
    }

    /// Without a phrase structure it still plans, and says the start is only a
    /// bar line rather than presenting it as the same thing.
    #[test]
    fn no_phrase_structure_is_reported_not_hidden() {
        let out = Outgoing {
            phrase: None,
            ..outgoing(100.0, 400.0)
        };
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::LandsOnBar { .. })),
            "a bar-line start was not distinguished from a phrase start: {:?}",
            plan.reasons
        );
    }
}

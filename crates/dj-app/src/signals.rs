//! What the DJ did, and the situation it happened in.
//!
//! [§14](../../../docs/DIRECTIVE.md) asks for useful events to be tracked
//! "from the existing Action/event architecture" and lists twenty of them.
//! [§13](../../../docs/DIRECTIVE.md) is the rule for reading them, and it is
//! the more important half:
//!
//! > Do not silently convert unusual behavior into permanent preference. […] A
//! > DJ raises BPM dramatically once because the crowd suddenly explodes. Do
//! > not conclude "User likes enormous BPM jumps". Instead: "Large jumps
//! > occasionally occur in high-energy contexts."
//!
//! The two ship together because either alone is worse than neither: signals
//! with no rule for reading them is how the wrong preference gets learned, and
//! a rule with nothing to read is a comment.
//!
//! # §13 is a type here, not a warning
//!
//! A [`Signal`] carries what was done **and** the phase the night was in, as
//! one value that cannot be taken apart. A [`Tendency`] — the only thing that
//! generalises from signals — cannot be constructed without a phase, and
//! cannot be constructed from fewer than [`ENOUGH`] of them in that same
//! phase. So *"the DJ likes large tempo moves"* is not a sentence this module
//! can produce; *"large tempo moves happen sometimes, at peak"* is.
//!
//! That is the same discipline `dj_assistant::posture::Grounds` uses for §9:
//! a rule enforced by a constructor is a rule that is still there in the
//! seventh place somebody needs it, and a runtime check is one `if` away from
//! being forgotten.
//!
//! # Derived from the log, like everything else here
//!
//! Nothing new is recorded. The action bus has timestamped every gesture since
//! M0, so a set from before this module existed has its signals in it too —
//! the same argument [`crate::mixes`] makes, for the same reason.
//!
//! # What §14 lists and the log cannot see
//!
//! *Track searched*, *previewed*, *staged*, *candidate rejected*, *candidate
//! selected* and *assistant suggestion accepted*. None of them is an action:
//! searching is a query, previewing needs a player djmanzo does not have, and
//! staging and the rail's verdicts are interface gestures that never reach the
//! bus. They are absent rather than approximated, because a signal inferred
//! from something else is exactly the "unusual behaviour" §13 is about.

use dj_control::{SessionEvent, TimedEvent};
use dj_core::{Action, DeckAction, MixerAction, SessionPhase};
use std::collections::BTreeMap;
use std::time::Duration;

/// One kind of thing a DJ does, from §14's list.
///
/// Coarser than the vocabulary on purpose. Six separate EQ verbs are six
/// spellings of one gesture, and a tendency counted per verb would need six
/// times the evidence to notice the thing a DJ would call "you ride the EQ".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Did {
    Loaded,
    Ejected,
    Cued,
    Looped,
    LoopResized,
    StemChanged,
    EqMoved,
    FilterSwept,
    FxUsed,
    Crossfaded,
    TempoMoved,
    SyncChanged,
}

impl Did {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Did::Loaded => "loaded",
            Did::Ejected => "ejected",
            Did::Cued => "cued",
            Did::Looped => "looped",
            Did::LoopResized => "loop-resized",
            Did::StemChanged => "stem-changed",
            Did::EqMoved => "eq-moved",
            Did::FilterSwept => "filter-swept",
            Did::FxUsed => "fx-used",
            Did::Crossfaded => "crossfaded",
            Did::TempoMoved => "tempo-moved",
            Did::SyncChanged => "sync-changed",
        }
    }

    /// Every gesture djmanzo watches for.
    pub const ALL: [Did; 12] = [
        Did::Loaded,
        Did::Ejected,
        Did::Cued,
        Did::Looped,
        Did::LoopResized,
        Did::StemChanged,
        Did::EqMoved,
        Did::FilterSwept,
        Did::FxUsed,
        Did::Crossfaded,
        Did::TempoMoved,
        Did::SyncChanged,
    ];

    /// Read back what [`Self::slug`] wrote.
    ///
    /// `None` for anything else. A stored gesture djmanzo no longer has is
    /// dropped rather than mapped onto a neighbour, because a profile that
    /// silently turned "filter swept" into "EQ moved" would be telling a DJ
    /// they do something they do not.
    #[must_use]
    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.slug() == slug.trim())
    }

    /// The gesture, as a DJ would name it.
    ///
    /// Written to follow "you", and deliberately in the plural-ish present:
    /// these end up in a sentence about a *tendency*, and a singular past
    /// tense would read as a claim about one moment.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Did::Loaded => "put records on",
            Did::Ejected => "clear decks",
            Did::Cued => "set cues",
            Did::Looped => "loop",
            Did::LoopResized => "resize loops as they run",
            Did::StemChanged => "play the stems",
            Did::EqMoved => "ride the EQ",
            Did::FilterSwept => "sweep the filter",
            Did::FxUsed => "reach for effects",
            Did::Crossfaded => "work the crossfader",
            Did::TempoMoved => "move the tempo by hand",
            Did::SyncChanged => "change what sync is doing",
        }
    }
}

/// One thing done, and the night it was done in.
///
/// The two are one value and there is deliberately no way to take them apart:
/// §13's whole point is that a gesture without its context is a preference
/// waiting to be learned wrongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal {
    pub did: Did,
    pub at: Duration,
    /// The phase the night was in.
    ///
    /// `None` before anything could say — which is the first six minutes of
    /// every set, by `dj_core::context`'s own design. A signal with no context
    /// is kept, because it happened, and can never become a [`Tendency`].
    pub context: Option<SessionPhase>,
}

/// Something the DJ does often enough, in one situation, to be worth saying.
///
/// **Constructible only through [`tendencies`]**, which is what makes §13
/// structural rather than advisory: the fields are readable but the type
/// cannot be assembled elsewhere, so nothing in the workspace can mint a
/// preference out of one surprising night.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tendency {
    did: Did,
    context: SessionPhase,
    seen: usize,
    share: f32,
}

impl Tendency {
    #[must_use]
    pub const fn did(&self) -> Did {
        self.did
    }
    /// The phase it was seen in. Never absent — that is the point of the type.
    #[must_use]
    pub const fn context(&self) -> SessionPhase {
        self.context
    }
    #[must_use]
    pub const fn seen(&self) -> usize {
        self.seen
    }
    /// What share of everything done in that phase this gesture was.
    #[must_use]
    pub const fn share(&self) -> f32 {
        self.share
    }

    /// The sentence, written the way §13 asks for it.
    ///
    /// "Sometimes" and "often" rather than "likes" and "prefers", and the
    /// phase is in the sentence rather than implied. The directive's own
    /// example is the test: not *"user likes enormous BPM jumps"* but *"large
    /// jumps occasionally occur in high-energy contexts"*.
    #[must_use]
    pub fn words(&self) -> String {
        let how_often = if self.share >= OFTEN {
            "often"
        } else {
            "sometimes"
        };
        format!(
            "You {how_often} {} when the night is {}. Seen {} times.",
            self.did.words(),
            crate::night::phase_words(self.context),
            self.seen
        )
    }
}

/// Where a phase sits in the arc, for keying a map by.
///
/// `SessionPhase` is deliberately not `Ord` and this does not make it so — it
/// is a lookup, used only so a map can have a key.
fn place(phase: SessionPhase) -> Option<usize> {
    SessionPhase::ALL.iter().position(|p| *p == phase)
}

/// A share at or above this is "often"; below it, "sometimes".
///
/// A fifth. Not a half: a DJ doing one thing for a fifth of every gesture in a
/// phase is doing it a lot, and a threshold that needed a majority would only
/// ever fire for the crossfader.
const OFTEN: f32 = 0.2;

/// Nothing becomes a tendency on fewer than this many occurrences.
///
/// Four, in one phase. §13's example is a thing done *once*, and the rule that
/// stops it becoming a preference has to be a number rather than a judgement.
/// Four is the smallest count that cannot be a mistake, a change of mind and a
/// correction.
pub const ENOUGH: usize = 4;

/// Read a night's log as the gestures §14 names.
///
/// `phase_at` says what the night was doing at a given moment. Supplied by the
/// caller rather than looked up here, because this module has no business
/// holding a context engine — and because a caller replaying a saved set has a
/// different answer from one watching a live one.
#[must_use]
pub fn signals(
    events: &[TimedEvent],
    phase_at: &dyn Fn(Duration) -> Option<SessionPhase>,
) -> Vec<Signal> {
    events
        .iter()
        .filter_map(|entry| {
            did(&entry.event).map(|did| Signal {
                did,
                at: entry.at,
                context: phase_at(entry.at),
            })
        })
        .collect()
}

fn did(event: &SessionEvent) -> Option<Did> {
    match event {
        SessionEvent::Load { .. } => Some(Did::Loaded),
        SessionEvent::Action(Action::Mixer(MixerAction::Crossfader(_))) => Some(Did::Crossfaded),
        SessionEvent::Action(Action::Mixer(MixerAction::StemSwap { .. })) => Some(Did::StemChanged),
        SessionEvent::Action(Action::Deck { action, .. }) => match action {
            DeckAction::Eject => Some(Did::Ejected),
            DeckAction::HotCueSet(_) | DeckAction::HotCueClear(_) => Some(Did::Cued),
            DeckAction::LoopBeats(_)
            | DeckAction::LoopPhrases(_)
            | DeckAction::LoopIn
            | DeckAction::LoopOut
            | DeckAction::LoopOff => Some(Did::Looped),
            DeckAction::LoopHalve | DeckAction::LoopDouble | DeckAction::LoopMove(_) => {
                Some(Did::LoopResized)
            }
            DeckAction::Stem { .. } => Some(Did::StemChanged),
            DeckAction::SetEqLow(_) | DeckAction::SetEqMid(_) | DeckAction::SetEqHigh(_) => {
                Some(Did::EqMoved)
            }
            DeckAction::SetFilter(_) => Some(Did::FilterSwept),
            DeckAction::Fx { .. } => Some(Did::FxUsed),
            DeckAction::SetPitch(_) | DeckAction::SetRate(_) => Some(Did::TempoMoved),
            DeckAction::Sync | DeckAction::SyncOff | DeckAction::SyncToggle => {
                Some(Did::SyncChanged)
            }
            _ => None,
        },
        SessionEvent::Action(_) => None,
    }
}

/// What the night's signals amount to, if anything.
///
/// **The only way to make a [`Tendency`]**, and every rule §13 asks for is
/// here: a signal with no context is not counted, nothing generalises on fewer
/// than [`ENOUGH`] occurrences, and the count is per *phase* rather than
/// across the night — so a thing done four times at peak says nothing about a
/// warm-up.
///
/// Sorted by how much of its phase each one was, so the strongest reads first.
#[must_use]
pub fn tendencies(signals: &[Signal]) -> Vec<Tendency> {
    // Keyed by the phase's place in the arc rather than by the phase itself,
    // because `SessionPhase` is deliberately not `Ord`: the arc is an order in
    // *time* and the energy does not follow it — coming down is later than
    // peak and quieter. An order asserted for a map's convenience is one
    // something else eventually reads as meaning something.
    let mut per_phase: BTreeMap<usize, usize> = BTreeMap::new();
    let mut counts: BTreeMap<(usize, Did), usize> = BTreeMap::new();
    for signal in signals {
        // No context, no count. This is §13's rule at the one place it can be
        // enforced: everything downstream reads `Tendency`, and a `Tendency`
        // has a phase by construction.
        let Some(phase) = signal.context.and_then(place) else {
            continue;
        };
        *per_phase.entry(phase).or_default() += 1;
        *counts.entry((phase, signal.did)).or_default() += 1;
    }

    let mut found: Vec<Tendency> = counts
        .into_iter()
        .filter(|(_, seen)| *seen >= ENOUGH)
        .filter_map(|((place, did), seen)| {
            let context = *SessionPhase::ALL.get(place)?;
            let total = per_phase.get(&place).copied().unwrap_or(seen).max(1);
            #[allow(clippy::cast_precision_loss)]
            Some(Tendency {
                did,
                context,
                seen,
                share: seen as f32 / total as f32,
            })
        })
        .collect();
    found.sort_by(|a, b| {
        b.share
            .partial_cmp(&a.share)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.did.cmp(&b.did))
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::DeckId;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a real deck")
    }

    fn at(secs: f64, action: Action) -> TimedEvent {
        TimedEvent {
            event: SessionEvent::Action(action),
            at: Duration::from_secs_f64(secs),
        }
    }

    fn pitch(secs: f64) -> TimedEvent {
        at(
            secs,
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetPitch(0.08),
            },
        )
    }

    fn eq(secs: f64) -> TimedEvent {
        at(
            secs,
            Action::Deck {
                deck: deck(1),
                action: DeckAction::SetEqLow(0.2),
            },
        )
    }

    const PEAK: fn(Duration) -> Option<SessionPhase> = |_| Some(SessionPhase::Peak);
    const UNREAD: fn(Duration) -> Option<SessionPhase> = |_| None;

    /// **§13's own example, and the sentence it refuses.**
    ///
    /// One dramatic tempo move does not become a preference. The directive
    /// spells this out: not "user likes enormous BPM jumps" but "large jumps
    /// occasionally occur in high-energy contexts".
    #[test]
    fn one_surprising_thing_never_becomes_a_preference() {
        let once = signals(&[pitch(10.0)], &PEAK);
        assert_eq!(once.len(), 1, "the signal itself is still recorded");
        assert_eq!(tendencies(&once), vec![], "one gesture became a tendency");

        // Nor does three. Four is the smallest count that cannot be a mistake,
        // a change of mind and a correction.
        let thrice = signals(&[pitch(10.0), pitch(20.0), pitch(30.0)], &PEAK);
        assert_eq!(tendencies(&thrice), vec![]);
    }

    /// **And when it does become one, it says which night it was about.**
    #[test]
    fn a_tendency_names_its_context_and_never_generalises_past_it() {
        let log: Vec<TimedEvent> = (0..6).map(|i| pitch(f64::from(i) * 10.0)).collect();
        let found = tendencies(&signals(&log, &PEAK));
        assert_eq!(found.len(), 1);

        let it = found[0];
        assert_eq!(it.context(), SessionPhase::Peak);
        assert_eq!(it.seen(), 6);
        let said = it.words();
        assert!(
            said.contains("at its peak"),
            "the sentence does not say when: {said:?}"
        );
        // The words §13 refuses.
        for forbidden in ["likes", "prefers", "always"] {
            assert!(
                !said.to_lowercase().contains(forbidden),
                "{said:?} claims a preference"
            );
        }
    }

    /// **A gesture whose context nobody could read counts towards nothing.**
    ///
    /// The first six minutes of every set, by `dj_core::context`'s own design.
    /// Those signals are kept — they happened — but they cannot generalise,
    /// because a tendency without a context is the thing §13 exists to
    /// prevent.
    #[test]
    fn signals_from_a_night_nobody_could_read_yet_teach_nothing() {
        let log: Vec<TimedEvent> = (0..20).map(|i| pitch(f64::from(i))).collect();
        let unread = signals(&log, &UNREAD);
        assert_eq!(unread.len(), 20, "the signals are still recorded");
        assert!(unread.iter().all(|s| s.context.is_none()));
        assert_eq!(tendencies(&unread), vec![]);
    }

    /// **Counted per phase, so peak says nothing about a warm-up.**
    #[test]
    fn a_tendency_at_peak_is_not_a_tendency_while_warming_up() {
        let log: Vec<TimedEvent> = (0..8).map(|i| pitch(f64::from(i) * 10.0)).collect();
        // Four at peak, four warming up: neither reaches four *and* is the
        // whole story, but both reach the threshold in their own phase.
        let split = |t: Duration| {
            Some(if t.as_secs() < 40 {
                SessionPhase::WarmUp
            } else {
                SessionPhase::Peak
            })
        };
        let found = tendencies(&signals(&log, &split));
        assert_eq!(found.len(), 2, "{found:?}");
        let phases: Vec<SessionPhase> = found.iter().map(Tendency::context).collect();
        assert!(phases.contains(&SessionPhase::WarmUp));
        assert!(phases.contains(&SessionPhase::Peak));

        // Three at peak and five warming up: only one of them generalises.
        let uneven = |t: Duration| {
            Some(if t.as_secs() < 50 {
                SessionPhase::WarmUp
            } else {
                SessionPhase::Peak
            })
        };
        let found = tendencies(&signals(&log, &uneven));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].context(), SessionPhase::WarmUp);
    }

    /// **The share is of everything done in that phase, not of the night.**
    ///
    /// "Often" has to mean something a DJ would recognise, and a gesture that
    /// was a fifth of a busy peak is a different claim from one that was a
    /// fifth of a quiet whole night.
    #[test]
    fn how_often_is_measured_against_the_phase_it_happened_in() {
        let mut log: Vec<TimedEvent> = (0..5).map(|i| pitch(f64::from(i))).collect();
        log.extend((0..5).map(|i| eq(f64::from(i) + 100.0)));
        let found = tendencies(&signals(&log, &PEAK));
        assert_eq!(found.len(), 2);
        for it in &found {
            assert!(
                (it.share() - 0.5).abs() < 0.01,
                "half of a phase read as {:.2}",
                it.share()
            );
            assert!(it.words().contains("often"), "{}", it.words());
        }
    }

    /// **Every gesture this module names is one the log can actually produce.**
    ///
    /// A `Did` nothing maps to would be a §14 signal djmanzo claims to watch
    /// for and never sees — which is worse than not listing it, because the
    /// absence would look like the DJ never doing it.
    #[test]
    fn every_gesture_named_is_one_something_can_produce() {
        let deck1 = |action| {
            SessionEvent::Action(Action::Deck {
                deck: deck(1),
                action,
            })
        };
        let produced: std::collections::BTreeSet<Did> = [
            SessionEvent::Load {
                deck: deck(1),
                track: dj_core::TrackId::from_bytes([1; 32]),
            },
            SessionEvent::Action(Action::Mixer(MixerAction::Crossfader(0.0))),
            deck1(DeckAction::Eject),
            deck1(DeckAction::HotCueSet(1)),
            deck1(DeckAction::LoopBeats(4.0)),
            deck1(DeckAction::LoopDouble),
            deck1(DeckAction::Stem {
                stem: dj_core::Stem::Vocal,
                change: dj_core::StemChange::ToggleMute,
            }),
            deck1(DeckAction::SetEqLow(0.0)),
            deck1(DeckAction::SetFilter(0.5)),
            deck1(DeckAction::Fx {
                slot: 1,
                change: dj_core::fx::FxChange::SetEnabled(true),
            }),
            deck1(DeckAction::SetPitch(0.05)),
            deck1(DeckAction::Sync),
        ]
        .iter()
        .filter_map(did)
        .collect();

        for named in [
            Did::Loaded,
            Did::Ejected,
            Did::Cued,
            Did::Looped,
            Did::LoopResized,
            Did::StemChanged,
            Did::EqMoved,
            Did::FilterSwept,
            Did::FxUsed,
            Did::Crossfaded,
            Did::TempoMoved,
            Did::SyncChanged,
        ] {
            assert!(
                produced.contains(&named),
                "{} is named but nothing on the bus produces it",
                named.slug()
            );
            assert!(!named.words().is_empty());
        }
    }

    /// Six spellings of one gesture are one gesture.
    #[test]
    fn every_eq_verb_is_the_same_thing_being_done() {
        for action in [
            DeckAction::SetEqLow(0.0),
            DeckAction::SetEqMid(0.5),
            DeckAction::SetEqHigh(1.5),
        ] {
            assert_eq!(
                did(&SessionEvent::Action(Action::Deck {
                    deck: deck(1),
                    action,
                })),
                Some(Did::EqMoved)
            );
        }
    }
}

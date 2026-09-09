//! The mixes a night contains, read back out of its own log.
//!
//! [§67](../../../docs/DIRECTIVE.md) says the session is the application's
//! central object and that it *contains transitions*. [§68](../../../docs/DIRECTIVE.md)
//! says an explicit transition object should drive, among other things,
//! practice and replay. [`crate::transition`] is that object for the mix that
//! is *about* to happen; this is the same thing for the ones that already did.
//!
//! # Derived, never recorded
//!
//! The obvious implementation is to write a transition into the session file
//! as it is performed. That is the wrong one, and `dj_control::SessionEvent`
//! already says why about loading: a variant for something the log can already
//! express gives one event two spellings, and two takes recorded through
//! different paths then diff as different sets. A mix *is* its actions — the
//! fader that moved, the bass that came out — so recording it beside them
//! would be recording it twice and inviting the two to disagree.
//!
//! Deriving it instead has a second benefit that settled the question: every
//! set djmanzo has ever recorded gains its mixes, including the ones recorded
//! before this module existed. Nothing has to have been present at the time.
//!
//! # What a handover is
//!
//! **The room stopped hearing one record and started hearing another.** One
//! rule rather than a recogniser per gesture: a crossfader sweep, two channel
//! faders crossing, and a fader against a crossfader assignment are all the
//! same event to the room, and a module with three rules for them would
//! disagree with itself the first time a DJ used two at once.
//!
//! So audibility is computed the way the engine computes it — the channel
//! fader times the crossfader gain for that deck's assignment, through
//! `dj_dsp`'s own curve — and a crossing of [`AUDIBLE`] in opposite directions
//! on two decks, close enough together, is a handover.
//!
//! # What it cannot know, and does not claim
//!
//! Whether it *worked*. The log says what was done and when, exactly, with no
//! detection involved — the same argument `dj_assistant::coach` makes for
//! reading the log rather than the audio. It cannot say whether the two
//! records suited each other or whether the room noticed, and nothing here
//! reports a score.
//!
//! It also cannot see a mix made entirely with EQ and no fader movement at
//! all — the record never becomes inaudible, so nothing crosses. That is a
//! real gap and it is a small one: a mix that never takes the outgoing record
//! out is a mix that has not finished.

use dj_control::{SessionEvent, TimedEvent};
use dj_core::action::TransitionStyle;
use dj_core::fx::{EffectKind, FxChange};
use dj_core::{Action, CrossfaderAssign, DeckAction, DeckId, MixerAction, Stem, TrackId};
use dj_dsp::{CrossfaderCurve, crossfader_gains};
use std::collections::BTreeMap;
use std::time::Duration;

/// Below this a deck is not in the room.
///
/// About -26 dB. Not zero: a channel fader left a hair off the stop, or a
/// crossfader curve that never quite reaches its end, would otherwise keep a
/// record "audible" for the rest of the night and no handover would ever be
/// seen. Not higher either — a record fading under another is still playing to
/// the room well below a quarter of its level.
const AUDIBLE: f32 = 0.05;

/// The longest gap between one record leaving and another arriving that is
/// still one handover.
///
/// Forty-five seconds, which is sixty-four beats at 85 BPM — the longest
/// transition [`crate::transition::LENGTH_RANGE`] will hold, at the slowest
/// tempo anybody dances to. Longer than that the two records were not in a mix
/// together; one ended and, some time later, another began.
const TOGETHER: Duration = Duration::from_secs(45);

/// How far before the handover a gesture still counts as part of it.
///
/// A DJ pulls the outgoing bass as the new record arrives, and sometimes a
/// moment before. Without this the commonest mix in dance music would be
/// recorded as a plain fade because its defining move landed just outside the
/// window.
const LEAD: Duration = Duration::from_secs(4);

/// One record handed over to another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handover {
    /// The deck the room stopped hearing.
    pub out: DeckId,
    /// The deck it started hearing.
    pub into: DeckId,
    /// What was on them, when the log says so.
    pub out_track: Option<TrackId>,
    pub in_track: Option<TrackId>,
    /// When the first of the two records crossed.
    pub began: Duration,
    /// When the second did.
    pub ended: Duration,
    /// What kind of mix it was, from what was done during it.
    pub style: TransitionStyle,
}

impl Handover {
    /// How long the handover took.
    #[must_use]
    pub fn took(&self) -> Duration {
        self.ended.saturating_sub(self.began)
    }

    /// How long it took in beats, given the tempo it was mixed at.
    ///
    /// Beats rather than seconds is how a DJ thinks about a transition and how
    /// §68's object states its length — but the log holds no tempo, so this is
    /// asked of the caller that has the library rather than guessed at here.
    #[must_use]
    pub fn beats(&self, bpm: f64) -> Option<f64> {
        (bpm.is_finite() && bpm > 0.0).then(|| self.took().as_secs_f64() * bpm / 60.0)
    }
}

/// A moment one deck's audibility changed.
#[derive(Debug, Clone, Copy)]
struct Crossing {
    deck: DeckId,
    at: Duration,
    /// True when the room started hearing it.
    up: bool,
    track: Option<TrackId>,
}

/// Something done during a mix that names what kind of mix it was.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Signal {
    /// The outgoing low pulled out.
    BassOut,
    /// An echo engaged.
    Echo,
    /// The vocal taken off this deck and put on another.
    VocalAway,
}

/// A low EQ at or below this has been pulled out rather than trimmed.
///
/// The same number `dj_assistant::coach` uses to recognise a bass swap, and
/// for the same reason: below a quarter the bass is gone rather than reduced.
const LOW_IS_OUT: f32 = 0.25;

/// The mixes in a night, in the order they happened.
#[must_use]
pub fn handovers(events: &[TimedEvent]) -> Vec<Handover> {
    let (crossings, signals) = walk(events);
    pair(&crossings, &signals)
}

/// One pass over the log, gathering everything the second pass needs.
///
/// State rather than pattern-matching, because audibility is a property of
/// three separate controls and a deck can become inaudible without anything
/// happening to *it*: another deck's assignment does not move the crossfader,
/// but the crossfader moves every assigned deck at once.
fn walk(events: &[TimedEvent]) -> (Vec<Crossing>, Vec<(Duration, DeckId, Signal)>) {
    let mut volume: BTreeMap<DeckId, f32> = BTreeMap::new();
    let mut assign: BTreeMap<DeckId, CrossfaderAssign> = BTreeMap::new();
    let mut track: BTreeMap<DeckId, TrackId> = BTreeMap::new();
    let mut selected: BTreeMap<(DeckId, u8), EffectKind> = BTreeMap::new();
    let mut audible: BTreeMap<DeckId, bool> = BTreeMap::new();
    let mut crossfader = 0.0_f32;

    let mut crossings = Vec::new();
    let mut signals = Vec::new();

    for entry in events {
        match entry.event {
            SessionEvent::Load { deck, track: id } => {
                track.insert(deck, id);
            }
            SessionEvent::Action(Action::Mixer(MixerAction::Crossfader(x))) => crossfader = x,
            SessionEvent::Action(Action::Mixer(MixerAction::StemSwap {
                stem: Stem::Vocal,
                from,
                to,
            })) => {
                // The vocal leaves `from` and lands on `to`. §68's VocalDrop is
                // about the record going out, so it is the source that is
                // marked.
                signals.push((entry.at, from, Signal::VocalAway));
                let _ = to;
            }
            SessionEvent::Action(Action::Deck { deck, ref action }) => match action {
                DeckAction::SetVolume(v) => {
                    volume.insert(deck, *v);
                }
                DeckAction::SetCrossfaderAssign(a) => {
                    assign.insert(deck, *a);
                }
                DeckAction::SetEqLow(level) if *level <= LOW_IS_OUT => {
                    signals.push((entry.at, deck, Signal::BassOut));
                }
                DeckAction::Fx { slot, change } => match change {
                    FxChange::Select(kind) => {
                        selected.insert((deck, *slot), *kind);
                        if *kind == EffectKind::Echo {
                            signals.push((entry.at, deck, Signal::Echo));
                        }
                    }
                    FxChange::SetEnabled(true) | FxChange::ToggleEnabled => {
                        if selected.get(&(deck, *slot)) == Some(&EffectKind::Echo) {
                            signals.push((entry.at, deck, Signal::Echo));
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            SessionEvent::Action(_) => {}
        }

        // Every deck the log has seen, re-read after every event.
        for deck in track.keys().copied().collect::<Vec<_>>() {
            let now = gain(deck, &volume, &assign, crossfader) >= AUDIBLE;
            let before = audible.insert(deck, now);
            if before != Some(now) && before.is_some() {
                crossings.push(Crossing {
                    deck,
                    at: entry.at,
                    up: now,
                    track: track.get(&deck).copied(),
                });
            }
        }
    }

    (crossings, signals)
}

/// What the room hears from one deck, on the engine's own arithmetic.
fn gain(
    deck: DeckId,
    volume: &BTreeMap<DeckId, f32>,
    assign: &BTreeMap<DeckId, CrossfaderAssign>,
    crossfader: f32,
) -> f32 {
    // The startup convention `CrossfaderAssign` documents: decks 1 and 2 are
    // left and right, anything beyond them is through. Written here because a
    // log that never mentions an assignment is a log recorded from that state.
    let placed = assign
        .get(&deck)
        .copied()
        .unwrap_or(match deck.human_number() {
            1 => CrossfaderAssign::Left,
            2 => CrossfaderAssign::Right,
            _ => CrossfaderAssign::Thru,
        });
    // The curve the mixer defaults to. All three are monotonic, so which one
    // is in force moves where the crossing sits by a fraction of the fader's
    // travel and never which side of it a deck is on.
    let (left, right) = crossfader_gains(crossfader, CrossfaderCurve::default());
    let cut = match placed {
        CrossfaderAssign::Left => left,
        CrossfaderAssign::Right => right,
        CrossfaderAssign::Thru => 1.0,
    };
    // A deck the log has never set a volume on is at unity, which is where a
    // channel fader sits when a deck is loaded.
    volume.get(&deck).copied().unwrap_or(1.0) * cut
}

/// Pair each record leaving with the one that replaced it.
fn pair(crossings: &[Crossing], signals: &[(Duration, DeckId, Signal)]) -> Vec<Handover> {
    let mut used = vec![false; crossings.len()];
    let mut found = Vec::new();

    for (i, leaving) in crossings.iter().enumerate() {
        if leaving.up || used[i] {
            continue;
        }
        // The nearest arrival on another deck, in either direction: a DJ may
        // bring the new record in first or take the old one out first, and
        // both are the same mix.
        let arrival = crossings
            .iter()
            .enumerate()
            .filter(|(j, c)| !used[*j] && c.up && c.deck != leaving.deck)
            .map(|(j, c)| (j, c, gap(c.at, leaving.at)))
            .filter(|(_, _, gap)| *gap <= TOGETHER)
            .min_by_key(|(_, _, gap)| *gap);

        let Some((j, arriving, _)) = arrival else {
            continue;
        };
        used[i] = true;
        used[j] = true;

        let began = leaving.at.min(arriving.at);
        let ended = leaving.at.max(arriving.at);
        found.push(Handover {
            out: leaving.deck,
            into: arriving.deck,
            out_track: leaving.track,
            in_track: arriving.track,
            began,
            ended,
            style: style(leaving.deck, began, ended, signals),
        });
    }

    found.sort_by_key(|h| h.began);
    found
}

fn gap(a: Duration, b: Duration) -> Duration {
    a.saturating_sub(b).max(b.saturating_sub(a))
}

/// What kind of mix this was, from what was done to the outgoing deck during
/// it.
///
/// **Length decides between a cut and a mix first**, because that is what the
/// room hears: a two-second handover is an edit whatever else was touched.
/// Within a mix, the most specific gesture names it — a vocal taken across is
/// a vocal drop even if the bass also came out, because nobody does the first
/// by accident.
fn style(
    out: DeckId,
    began: Duration,
    ended: Duration,
    signals: &[(Duration, DeckId, Signal)],
) -> TransitionStyle {
    if ended.saturating_sub(began) <= CUT_MAX {
        return TransitionStyle::Cut;
    }
    let from = began.saturating_sub(LEAD);
    let during = |what: Signal| {
        signals.iter().any(|(at, deck, signal)| {
            *deck == out && *signal == what && *at >= from && *at <= ended
        })
    };
    if during(Signal::VocalAway) {
        TransitionStyle::VocalDrop
    } else if during(Signal::Echo) {
        TransitionStyle::Echo
    } else if during(Signal::BassOut) {
        TransitionStyle::Blend
    } else {
        TransitionStyle::Fade
    }
}

/// The longest handover the room hears as an edit rather than as a mix.
///
/// `dj_assistant::coach`'s own number, imported rather than repeated: it
/// decides whether a crossfade is called a cut when the coach names it, and
/// the two must not answer differently about one mix on one screen.
const CUT_MAX: Duration = dj_assistant::coach::CUT_MAX;

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a real deck")
    }

    fn id(n: u8) -> TrackId {
        let mut bytes = [0u8; 32];
        bytes[0] = n;
        TrackId::from_bytes(bytes)
    }

    fn at(secs: f64, event: SessionEvent) -> TimedEvent {
        TimedEvent {
            event,
            at: Duration::from_secs_f64(secs),
        }
    }

    fn load(secs: f64, on: u8, track: u8) -> TimedEvent {
        at(
            secs,
            SessionEvent::Load {
                deck: deck(on),
                track: id(track),
            },
        )
    }

    fn xf(secs: f64, x: f32) -> TimedEvent {
        at(
            secs,
            SessionEvent::Action(Action::Mixer(MixerAction::Crossfader(x))),
        )
    }

    fn deck_action(secs: f64, on: u8, action: DeckAction) -> TimedEvent {
        at(
            secs,
            SessionEvent::Action(Action::Deck {
                deck: deck(on),
                action,
            }),
        )
    }

    /// A crossfader ramp from `from` to `to` over `secs`, in twenty steps —
    /// which is what a hand on a fader or the automix's own tick produces.
    fn sweep(start: f64, secs: f64, from: f32, to: f32) -> Vec<TimedEvent> {
        (0..=20)
            .map(|i| {
                let part = f64::from(i) / 20.0;
                #[allow(clippy::cast_possible_truncation)]
                let x = from + (to - from) * part as f32;
                xf(start + secs * part, x)
            })
            .collect()
    }

    /// Two records on decks 1 and 2, the second brought in with the fader.
    fn a_night(mix_seconds: f64) -> Vec<TimedEvent> {
        let mut log = vec![load(0.0, 1, 1), xf(1.0, -1.0), load(60.0, 2, 2)];
        log.extend(sweep(120.0, mix_seconds, -1.0, 1.0));
        log
    }

    /// **A record leaving as another arrives is one handover, and it names
    /// both decks.**
    ///
    /// The whole claim of the module. It is derived from the log rather than
    /// recorded, so this is also the assertion that a set recorded before any
    /// of this existed still has its mixes in it.
    #[test]
    fn a_crossfade_is_one_handover_with_both_decks_named() {
        let found = handovers(&a_night(20.0));
        assert_eq!(found.len(), 1, "{found:?}");
        let mix = &found[0];
        assert_eq!(mix.out, deck(1));
        assert_eq!(mix.into, deck(2));
        assert_eq!(mix.out_track, Some(id(1)));
        assert_eq!(mix.in_track, Some(id(2)));
        // It covers the sweep rather than a moment inside it.
        assert!(
            mix.took() >= Duration::from_secs(15),
            "a twenty-second mix was recorded as {:?}",
            mix.took()
        );
        assert!(mix.began >= Duration::from_secs(119));
    }

    /// **Two channel faders crossing is the same event.**
    ///
    /// One rule, not a recogniser per gesture. The room cannot tell which
    /// control did it, and a module with a branch for each would disagree with
    /// itself the first time somebody used both.
    #[test]
    fn channel_faders_make_the_same_handover_as_a_crossfader() {
        let mut log = vec![
            load(0.0, 1, 1),
            // Through, so the crossfader has no say and only the faders do.
            deck_action(
                0.1,
                1,
                DeckAction::SetCrossfaderAssign(CrossfaderAssign::Thru),
            ),
            load(60.0, 2, 2),
            deck_action(
                60.1,
                2,
                DeckAction::SetCrossfaderAssign(CrossfaderAssign::Thru),
            ),
            deck_action(60.2, 2, DeckAction::SetVolume(0.0)),
        ];
        for i in 0..=10 {
            let part = f64::from(i) / 10.0;
            #[allow(clippy::cast_possible_truncation)]
            let up = part as f32;
            log.push(deck_action(
                120.0 + part * 16.0,
                2,
                DeckAction::SetVolume(up),
            ));
            log.push(deck_action(
                120.0 + part * 16.0,
                1,
                DeckAction::SetVolume(1.0 - up),
            ));
        }
        let found = handovers(&log);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].out, found[0].into), (deck(1), deck(2)));
    }

    /// **The first record of the night is not a handover.**
    ///
    /// Something arriving with nothing leaving is a set starting. Reporting it
    /// as a mix would put a transition in the list that nobody performed, and
    /// the list is meant to be what the DJ did.
    #[test]
    fn a_record_arriving_alone_is_not_a_mix() {
        let log = vec![
            load(0.0, 1, 1),
            deck_action(0.1, 1, DeckAction::SetVolume(0.0)),
            deck_action(5.0, 1, DeckAction::SetVolume(1.0)),
        ];
        assert_eq!(handovers(&log), vec![]);
    }

    /// **A record that ends long after another began is not a mix with it.**
    #[test]
    fn records_too_far_apart_are_not_one_mix() {
        let mut log = vec![load(0.0, 1, 1), xf(1.0, -1.0), load(60.0, 2, 2)];
        // Deck 2 in at two minutes, deck 1 not out until five minutes later.
        log.extend(sweep(120.0, 4.0, -1.0, 0.0));
        log.push(deck_action(420.0, 1, DeckAction::SetVolume(0.0)));
        assert_eq!(handovers(&log), vec![], "two unrelated moves became a mix");
    }

    /// **The bass coming out is what makes it a blend rather than a fade**,
    /// and both are said rather than one standing for the other.
    #[test]
    fn the_style_comes_from_what_was_actually_done() {
        let plain = handovers(&a_night(20.0));
        assert_eq!(plain[0].style, TransitionStyle::Fade);

        let mut with_bass = a_night(20.0);
        with_bass.push(deck_action(121.0, 1, DeckAction::SetEqLow(0.0)));
        with_bass.sort_by_key(|e| e.at);
        assert_eq!(handovers(&with_bass)[0].style, TransitionStyle::Blend);

        let mut with_echo = a_night(20.0);
        with_echo.push(deck_action(
            121.0,
            1,
            DeckAction::Fx {
                slot: 1,
                change: FxChange::Select(EffectKind::Echo),
            },
        ));
        with_echo.sort_by_key(|e| e.at);
        assert_eq!(handovers(&with_echo)[0].style, TransitionStyle::Echo);

        let mut with_vocal = a_night(20.0);
        with_vocal.push(at(
            121.0,
            SessionEvent::Action(Action::Mixer(MixerAction::StemSwap {
                stem: Stem::Vocal,
                from: deck(1),
                to: deck(2),
            })),
        ));
        with_vocal.sort_by_key(|e| e.at);
        assert_eq!(handovers(&with_vocal)[0].style, TransitionStyle::VocalDrop);
    }

    /// **A gesture on the other deck does not name this mix.**
    ///
    /// The bass coming out of the record *arriving* is a DJ making room for
    /// it, not a blend out of the one leaving. Attributing it to the outgoing
    /// deck would call almost every mix a blend and the word would stop
    /// meaning anything.
    #[test]
    fn a_gesture_on_the_incoming_deck_does_not_name_the_mix() {
        let mut log = a_night(20.0);
        log.push(deck_action(121.0, 2, DeckAction::SetEqLow(0.0)));
        log.sort_by_key(|e| e.at);
        assert_eq!(handovers(&log)[0].style, TransitionStyle::Fade);
    }

    /// **Two seconds is an edit, whatever else was touched.**
    ///
    /// Length decides first because that is what the room hears. The same two
    /// seconds `dj_assistant::coach` uses, imported rather than repeated.
    #[test]
    fn a_short_handover_is_a_cut_even_with_the_bass_out() {
        let mut log = a_night(1.0);
        log.push(deck_action(120.5, 1, DeckAction::SetEqLow(0.0)));
        log.sort_by_key(|e| e.at);
        let found = handovers(&log);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].style, TransitionStyle::Cut);
    }

    /// Beats need a tempo, which the log does not hold — so it is asked for,
    /// and a tempo that is not a tempo gets nothing rather than infinity.
    #[test]
    fn a_length_in_beats_needs_a_tempo_and_says_so() {
        let mix = handovers(&a_night(16.0))[0];
        let beats = mix.beats(120.0).expect("a real tempo answers");
        assert!(
            (beats - 32.0).abs() < 4.0,
            "a sixteen-second mix at 120 BPM is about 32 beats, not {beats}"
        );
        assert_eq!(mix.beats(0.0), None);
        assert_eq!(mix.beats(f64::NAN), None);
    }
}

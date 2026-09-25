//! §115: a background assistant that proposes without interrupting.
//!
//! > a background AI always auto watching the DJ and proposing useful things
//! > without being intrusive
//!
//! djmanzo already proposes in places — §29's marks on the knobs, the
//! activity strip's suggestion, §74's rail — each where its subject is. What
//! nothing watched was the *whole* booth: a deck clipping on its trim, a
//! limiter working hard all night, a record running out with nothing loaded
//! after it, the peak of the night going unrecorded, two records drifting
//! apart. This watches the snapshot the interface is drawn from, on the pump
//! that draws it, and proposes **one thing at a time, in one quiet line**.
//!
//! # What keeps it from intruding
//!
//! Each is a rule, because a proposal a DJ has to argue with is worse than
//! none:
//!
//! - **A condition has to hold before it is said.** A peak on one kick is
//!   not clipping; a tempo gap while a DJ is beatmatching by ear is the
//!   beatmatch. Each kind has its own hold.
//! - **One at a time**, the urgent first — a record running out outranks a
//!   suggestion to record.
//! - **Quiet ones wait for the hands.** A proposal that is not urgent is not
//!   shown while a hand is on a platter or the crossfader is moving, nor for
//!   [`HANDS_SETTLE`] after.
//! - **Declined is declined.** *Not now* silences that kind for
//!   [`QUIET_FOR`]; *not tonight* for the rest of the night; one ignored for
//!   [`IGNORED_AFTER`] is taken as *not now*, so nothing sits there nagging.
//!   Taking one silences it too — it was done.
//! - **It never acts.** A proposal carries the action a DJ would have typed,
//!   and runs only when they press it.
//!
//! Deterministic rules rather than a language model, on purpose: they run
//! sixty times a second, need no network in a booth, and every one of them is
//! a sentence a test can hold.

use crate::snapshot::{DeckSnapshot, Snapshot};
use std::collections::{HashMap, HashSet};

/// How long a declined kind stays silent: ten minutes.
pub const QUIET_FOR: f64 = 600.0;

/// How long a proposal nobody answered stays on screen before it counts as
/// declined.
pub const IGNORED_AFTER: f64 = 40.0;

/// How long after the hands were last busy a quiet proposal may be shown.
pub const HANDS_SETTLE: f64 = 5.0;

/// A deck's peak at or above this is at the ceiling.
pub const CLIPPING: f32 = 0.98;

/// A limiter holding back this much is working, not catching.
pub const LIMITER_WORKING_DB: f32 = 6.0;

/// A record with less than this left and nothing after it is running out.
pub const RUNNING_OUT_SECONDS: f32 = 30.0;

/// Two records this far apart in tempo, as a fraction, are drifting; beyond
/// the upper bound they are different tempos on purpose.
pub const DRIFT: (f32, f32) = (0.005, 0.08);

/// A deck heard this loudly is part of what the room hears.
pub const HEARD: f32 = 0.3;

/// What is proposed.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Proposal {
    /// The rule it came from, which is what declining it silences.
    pub kind: &'static str,
    /// The sentence, in the booth's words.
    pub says: String,
    /// The button's words.
    pub offer: String,
    /// What pressing it runs: an action, or an interface operation (`ui …`).
    pub run: String,
    /// Whether it may be shown while the hands are busy.
    pub urgent: bool,
    /// The deck it is about, when it is about one.
    pub deck: Option<u8>,
    /// §118a: how much of the screen it should take. A line for all but a
    /// record running out, which grows as its seconds do not
    /// ([`crate::decide::presence`]).
    pub presence: crate::decide::Presence,
}

/// One rule's reading of the booth right now.
struct Rule {
    proposal: Proposal,
    /// How long the condition must have held, in seconds.
    hold: f64,
}

fn rule(
    kind: &'static str,
    says: String,
    offer: String,
    run: String,
    urgent: bool,
    hold: f64,
) -> Rule {
    Rule {
        proposal: Proposal {
            kind,
            says,
            offer,
            run,
            urgent,
            deck: None,
            presence: crate::decide::Presence::Line,
        },
        hold,
    }
}

/// How loud the room hears a deck: its fader, and its side of the crossfader.
fn heard(deck: &DeckSnapshot, crossfader: f32) -> f32 {
    use dj_core::CrossfaderAssign;
    let assign = match deck.crossfader_assign {
        CrossfaderAssign::Left => -1.0,
        CrossfaderAssign::Right => 1.0,
        CrossfaderAssign::Thru => 0.0,
    };
    crate::live::level(deck.volume, assign, crossfader)
}

/// Every rule whose condition holds in this snapshot, most urgent first.
fn reading(snapshot: &Snapshot) -> Vec<Rule> {
    let master = &snapshot.master;
    let mut out = Vec::new();

    // Urgent: what the room is hearing is being damaged, or is about to stop.
    if let Some(deck) = snapshot
        .decks
        .iter()
        .find(|d| d.playing && d.peak >= CLIPPING)
    {
        let to = (deck.gain_db - 3.0).max(-24.0);
        out.push(rule(
            "clipping",
            format!(
                "Deck {} is hitting the ceiling. Its trim down 3 dB?",
                deck.number
            ),
            "Trim 3 dB".to_owned(),
            format!("deck {} gain {to:.1}", deck.number),
            true,
            1.5,
        ));
    }
    if master.limiter_enabled && master.limiter_reduction_db >= LIMITER_WORKING_DB {
        let to = (master.gain_db - 3.0).max(-24.0);
        out.push(rule(
            "limiter",
            format!(
                "The limiter is holding back {:.0} dB. The master down 3 dB?",
                master.limiter_reduction_db
            ),
            "Master −3 dB".to_owned(),
            format!("master gain {to:.1}"),
            true,
            3.0,
        ));
    }
    let loudest = snapshot
        .decks
        .iter()
        .filter(|d| d.playing)
        .max_by(|a, b| heard(a, master.crossfader).total_cmp(&heard(b, master.crossfader)));
    if let Some(deck) = loudest {
        let left = deck.length_seconds - deck.position_seconds;
        let alone = snapshot
            .decks
            .iter()
            .all(|other| other.number == deck.number || !other.loaded);
        if deck.length_seconds > 0.0 && left > 0.0 && left < RUNNING_OUT_SECONDS && alone {
            let mut running_out = rule(
                "running-out",
                format!(
                    "Deck {} ends in {} and nothing else is loaded.",
                    deck.number,
                    crate::share::clock(left.round() as i64)
                ),
                "Choose the next record".to_owned(),
                "ui show next".to_owned(),
                true,
                0.0,
            );
            running_out.proposal.deck = Some(deck.number);
            running_out.proposal.presence = crate::decide::presence(left);
            out.push(running_out);
        }
    }

    // Quiet: worth saying, when the hands are free.
    let peak = snapshot
        .context
        .session
        .as_ref()
        .is_some_and(|read| read.phase == dj_core::SessionPhase::Peak);
    if peak && !master.recording.active {
        out.push(rule(
            "record",
            "It is peak time and nothing is recording. Record the set?".to_owned(),
            "Record".to_owned(),
            "record on".to_owned(),
            false,
            60.0,
        ));
    }
    let mut playing: Vec<&DeckSnapshot> = snapshot
        .decks
        .iter()
        .filter(|d| d.playing && heard(d, master.crossfader) >= HEARD)
        .collect();
    if playing.len() == 2 {
        playing.sort_by(|a, b| heard(b, master.crossfader).total_cmp(&heard(a, master.crossfader)));
        let (lead, follow) = (playing[0], playing[1]);
        if let (Some(lead_bpm), Some(follow_bpm)) = (lead.effective_bpm, follow.effective_bpm) {
            let apart = (follow_bpm - lead_bpm).abs() / lead_bpm.max(1.0);
            if !lead.synced
                && !follow.synced
                && follow.can_sync
                && apart >= DRIFT.0
                && apart <= DRIFT.1
            {
                out.push(rule(
                    "drift",
                    format!(
                        "Decks {} and {} are {:.1}% apart in tempo. Sync deck {}?",
                        lead.number,
                        follow.number,
                        apart * 100.0,
                        follow.number
                    ),
                    format!("Sync deck {}", follow.number),
                    format!("deck {} sync", follow.number),
                    false,
                    8.0,
                ));
            }
        }
    }
    out
}

/// The watcher: what has held since when, and what the DJ declined.
#[derive(Debug, Default)]
pub struct Watcher {
    since: HashMap<&'static str, f64>,
    silent_until: HashMap<&'static str, f64>,
    tonight: HashSet<&'static str>,
    shown_since: Option<(&'static str, f64)>,
    last_crossfader: Option<f32>,
    hands_at: Option<f64>,
    showing: Option<Proposal>,
}

/// How a DJ answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Pressed it: done, so silent for a while.
    Taken,
    NotNow,
    NotTonight,
}

impl Answer {
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "taken" => Some(Self::Taken),
            "not-now" => Some(Self::NotNow),
            "not-tonight" => Some(Self::NotTonight),
            _ => None,
        }
    }
}

impl Watcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read one snapshot at `now` (monotonic seconds) and decide what, if
    /// anything, is proposed.
    pub fn observe(&mut self, snapshot: &Snapshot, now: f64) -> Option<Proposal> {
        let crossfader = snapshot.master.crossfader;
        let moved = self
            .last_crossfader
            .is_some_and(|before| (before - crossfader).abs() > 0.01);
        self.last_crossfader = Some(crossfader);
        if moved || snapshot.decks.iter().any(|d| d.jog_touched) {
            self.hands_at = Some(now);
        }
        let settled = self.hands_at.is_none_or(|at| now - at >= HANDS_SETTLE);

        let rules = reading(snapshot);
        self.since
            .retain(|kind, _| rules.iter().any(|r| r.proposal.kind == *kind));
        for rule in &rules {
            self.since.entry(rule.proposal.kind).or_insert(now);
        }

        // One ignored long enough counts as declined.
        if let Some((kind, from)) = self.shown_since
            && now - from >= IGNORED_AFTER
        {
            self.silent_until.insert(kind, now + QUIET_FOR);
            self.shown_since = None;
        }

        let chosen = rules
            .into_iter()
            .filter(|r| now - self.since[r.proposal.kind] >= r.hold)
            .filter(|r| !self.tonight.contains(r.proposal.kind))
            .filter(|r| {
                self.silent_until
                    .get(r.proposal.kind)
                    .is_none_or(|until| now >= *until)
            })
            .filter(|r| r.proposal.urgent || settled)
            .min_by_key(|r| !r.proposal.urgent)
            .map(|r| r.proposal);

        match (&chosen, self.shown_since) {
            (Some(p), Some((kind, _))) if kind == p.kind => {}
            (Some(p), _) => self.shown_since = Some((p.kind, now)),
            (None, _) => self.shown_since = None,
        }
        self.showing.clone_from(&chosen);
        chosen
    }

    /// What is proposed now.
    #[must_use]
    pub fn showing(&self) -> Option<Proposal> {
        self.showing.clone()
    }

    /// The DJ's answer to the proposal of `kind`.
    pub fn answer(&mut self, kind: &str, answer: Answer, now: f64) {
        let Some(kind) = KINDS.iter().copied().find(|k| *k == kind) else {
            return;
        };
        match answer {
            Answer::Taken | Answer::NotNow => {
                self.silent_until.insert(kind, now + QUIET_FOR);
            }
            Answer::NotTonight => {
                self.tonight.insert(kind);
            }
        }
        if self.showing.as_ref().is_some_and(|p| p.kind == kind) {
            self.showing = None;
            self.shown_since = None;
        }
    }
}

/// Every kind there is, which is what an answer may name.
pub const KINDS: [&str; 5] = ["clipping", "limiter", "running-out", "record", "drift"];

#[cfg(test)]
mod tests {
    use super::*;

    /// A booth with two records loaded, from a seeded registry, nothing
    /// wrong.
    fn booth() -> Snapshot {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 2);
        for deck in &mut snapshot.decks {
            deck.loaded = true;
            deck.volume = 1.0;
            deck.length_seconds = 300.0;
            deck.position_seconds = 60.0;
            deck.peak = 0.5;
            deck.jog_touched = false;
        }
        snapshot.master.crossfader = 0.0;
        snapshot.master.limiter_reduction_db = 0.0;
        snapshot
    }

    /// Feed the same snapshot for `seconds`, at 10 Hz, from `from`.
    fn hold(
        watcher: &mut Watcher,
        snapshot: &Snapshot,
        from: f64,
        seconds: f64,
    ) -> Option<Proposal> {
        let mut last = None;
        let mut t = from;
        while t <= from + seconds {
            last = watcher.observe(snapshot, t);
            t += 0.1;
        }
        last
    }

    /// **Nothing wrong, nothing said.**
    #[test]
    fn a_quiet_booth_hears_nothing() {
        let mut watcher = Watcher::new();
        let mut snapshot = booth();
        snapshot.decks[0].playing = true;
        assert_eq!(hold(&mut watcher, &snapshot, 0.0, 120.0), None);
    }

    /// **A condition has to hold.** One peak is a kick; a second and a half
    /// at the ceiling is clipping, and the proposal is the action a DJ would
    /// type, three decibels down from where the trim is.
    #[test]
    fn clipping_is_said_once_it_holds_and_offers_the_trim() {
        let mut watcher = Watcher::new();
        let mut snapshot = booth();
        snapshot.decks[1].playing = true;
        snapshot.decks[1].peak = 1.0;
        snapshot.decks[1].gain_db = 2.0;
        assert_eq!(
            hold(&mut watcher, &snapshot, 0.0, 1.0),
            None,
            "a single peak is not clipping"
        );
        let said = hold(&mut watcher, &snapshot, 1.1, 1.0).expect("held clipping is said");
        assert_eq!(said.kind, "clipping");
        assert_eq!(said.run, "deck 2 gain -1.0");
        assert!(dj_core::Action::parse(&said.run).is_ok());
    }

    /// **One at a time, the urgent first**: a record running out with
    /// nothing after it outranks the suggestion to record the peak.
    #[test]
    fn the_urgent_one_is_said_first() {
        let mut watcher = Watcher::new();
        let mut snapshot = booth();
        snapshot.decks[0].playing = true;
        snapshot.decks[0].position_seconds = 285.0;
        snapshot.decks[1].loaded = false;
        snapshot.context.session = Some(dj_core::SessionRead {
            phase: dj_core::SessionPhase::Peak,
            ..Default::default()
        });
        // Both hold from the start; the urgent one is said, straight away.
        let said = hold(&mut watcher, &snapshot, 0.0, 1.0).unwrap();
        assert_eq!(said.kind, "running-out");
        assert_eq!(said.run, "ui show next");
        assert!(crate::uiop::UiOp::parse(&said.run).is_ok());
        assert!(said.says.contains("0:15"), "{}", said.says);
        // §118a: it names its deck, and fifteen seconds is a card -- the
        // choices come up by themselves -- where the others are a line.
        assert_eq!(said.deck, Some(1));
        assert_eq!(said.presence, crate::decide::Presence::Card);
        // A record loaded on the other deck is the answer, and then the peak
        // unrecorded is what is left to say.
        snapshot.decks[1].loaded = true;
        assert_eq!(
            hold(&mut watcher, &snapshot, 1.1, 50.0),
            None,
            "the peak said before its minute"
        );
        let said = hold(&mut watcher, &snapshot, 51.2, 10.0).unwrap();
        assert_eq!(said.kind, "record");
        assert_eq!(said.run, "record on");
    }

    /// **Quiet ones wait for the hands**: two records drifting apart is not
    /// said while a hand is on a platter — that is a DJ beatmatching — nor
    /// until the hands have settled.
    #[test]
    fn a_quiet_proposal_waits_for_the_hands() {
        let mut watcher = Watcher::new();
        let mut snapshot = booth();
        for deck in &mut snapshot.decks {
            deck.playing = true;
            deck.can_sync = true;
        }
        snapshot.decks[0].effective_bpm = Some(124.0);
        snapshot.decks[1].effective_bpm = Some(126.0);
        snapshot.decks[1].jog_touched = true;
        assert_eq!(
            hold(&mut watcher, &snapshot, 0.0, 30.0),
            None,
            "said under a hand"
        );
        snapshot.decks[1].jog_touched = false;
        assert_eq!(
            hold(&mut watcher, &snapshot, 30.1, 4.0),
            None,
            "said before the hands settled"
        );
        let said = hold(&mut watcher, &snapshot, 34.2, 2.0).expect("said once settled");
        assert_eq!(said.kind, "drift");
        assert!(said.says.contains("1.6%"), "{}", said.says);
        // The quieter of two equally loud decks is the second; either way the
        // run is a sync the parser takes.
        assert!(dj_core::Action::parse(&said.run).is_ok(), "{}", said.run);
        // Beyond the drift band it is two tempos on purpose.
        snapshot.decks[1].effective_bpm = Some(140.0);
        assert_eq!(hold(&mut watcher, &snapshot, 40.0, 20.0), None);
    }

    /// **Declined is declined**: *not now* is quiet for ten minutes, *not
    /// tonight* for the night, and one nobody answered goes away by itself.
    #[test]
    fn declining_silences_and_ignoring_declines() {
        let mut watcher = Watcher::new();
        let mut snapshot = booth();
        snapshot.decks[0].playing = true;
        snapshot.master.limiter_enabled = true;
        snapshot.master.limiter_reduction_db = 8.0;
        assert_eq!(
            hold(&mut watcher, &snapshot, 0.0, 4.0).unwrap().kind,
            "limiter"
        );

        watcher.answer("limiter", Answer::NotNow, 4.0);
        assert_eq!(watcher.showing(), None);
        assert_eq!(
            hold(&mut watcher, &snapshot, 4.1, 500.0),
            None,
            "back before ten minutes"
        );
        assert!(
            hold(&mut watcher, &snapshot, 604.2, 1.0).is_some(),
            "not back after ten"
        );

        watcher.answer("limiter", Answer::NotTonight, 605.3);
        assert_eq!(
            hold(&mut watcher, &snapshot, 605.4, 2_000.0),
            None,
            "back the same night"
        );

        let mut other = Watcher::new();
        assert!(hold(&mut other, &snapshot, 0.0, 4.0).is_some());
        assert_eq!(
            hold(&mut other, &snapshot, 4.1, IGNORED_AFTER + 1.0),
            None,
            "an unanswered proposal stayed on screen"
        );
    }

    /// Every proposal runs something djmanzo accepts: an action the parser
    /// takes, or an interface operation.
    #[test]
    fn every_proposal_runs_something_real() {
        let mut snapshot = booth();
        for deck in &mut snapshot.decks {
            deck.playing = true;
            deck.can_sync = true;
            deck.peak = 1.0;
        }
        snapshot.decks[0].effective_bpm = Some(124.0);
        snapshot.decks[1].effective_bpm = Some(125.0);
        snapshot.decks[0].position_seconds = 290.0;
        snapshot.master.limiter_enabled = true;
        snapshot.master.limiter_reduction_db = 9.0;
        snapshot.context.session = Some(dj_core::SessionRead {
            phase: dj_core::SessionPhase::Peak,
            ..Default::default()
        });
        let rules = reading(&snapshot);
        assert!(rules.len() >= 4, "{}", rules.len());
        for rule in rules {
            let p = rule.proposal;
            assert!(KINDS.contains(&p.kind), "{}", p.kind);
            if let Some(op) = p.run.strip_prefix("ui ") {
                assert!(
                    crate::uiop::UiOp::parse(&format!("ui {op}")).is_ok(),
                    "{}",
                    p.run
                );
            } else {
                assert!(dj_core::Action::parse(&p.run).is_ok(), "{}", p.run);
            }
        }
    }
}

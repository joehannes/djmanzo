//! §37: what the room did after a mix, and whether it usually does.
//!
//! [§37 of the directive](../../../docs/DIRECTIVE.md) asks for more than
//! measuring a room:
//!
//! > Correlate changes with DJ actions. […] Track A was playing. Transition to
//! > Track B occurs. Crowd motion increases 12–30 seconds later. […] Similar
//! > reaction has occurred in previous sessions. The AI can infer: "This type
//! > of transition has historically improved room response here."
//!
//! Two claims, and they need different things. **What the room did after this
//! mix** is arithmetic over readings djmanzo already has. **What it usually
//! does** is the one thing in this project that cannot be derived: the
//! readings live for twenty minutes ([`dj_assistant::room::RECENT`]) and a
//! night's log does not outlive the run that made it, so a comparison across
//! nights has to be written down. That is the same exception §81 took, for the
//! same reason, and it is written down as narrowly as possible — one row per
//! mix, six numbers, no time series.
//!
//! # Why the correlation is not a causal claim, whatever the section is called
//!
//! A floor that fills twelve seconds after a mix may be filling because of the
//! mix, because the last record ended, because a round of drinks arrived, or
//! because it was going to fill anyway. Nothing here can tell those apart, and
//! nothing here says "because". What it says is *what happened after*, over
//! enough nights that coincidence is a worse explanation than the obvious one
//! — which is the honest form of §37's sentence and is why [`Seen`] refuses to
//! generalise below [`ENOUGH_NIGHTS`].
//!
//! # Why "here" is a kind of night rather than a venue
//!
//! §37 says *here*, meaning this room. djmanzo has no venue: it has §81's
//! [`Setting`](crate::setting::Setting), which is what the DJ said the night
//! is. A club night is not a venue and saying so would be inventing a fact,
//! so the sentence this produces says "at a club night" and means it.

use dj_assistant::room::{Reading, Sense};
use std::time::{Duration, SystemTime};

/// The window §37 names, after the mix has finished.
///
/// Twelve to thirty seconds, from the section's own example. Not a guess and
/// not tunable here: a room takes a few bars to decide about a record, and
/// widening this until something correlates is how a correlation gets found in
/// noise.
pub const LAG: (Duration, Duration) = (Duration::from_secs(12), Duration::from_secs(30));

/// How much room to read as the "before".
///
/// A minute, ending where the mix begins. Long enough to be a level rather
/// than a moment, short enough that it is the same stretch of night.
pub const BEFORE: Duration = Duration::from_secs(60);

/// The smallest change worth calling a change.
///
/// Five percent of the sensor's own range. Below it the two windows are the
/// same reading twice, and a `Lift` for every mix would make the whole table
/// noise with a shape.
pub const MOVED: f32 = 0.05;

/// How many readings each window needs before it is a level.
///
/// At the interface's cadence of one every two seconds this is ten seconds of
/// looking, which the shorter of the two windows can just supply. A window
/// that cannot reach it produces no answer rather than a thin one.
pub const ENOUGH: usize = 5;

/// How many nights a claim about *usually* needs.
///
/// Three, matching [`crate::profile::ENOUGH_NIGHTS`]: the same discipline at
/// the same scale, and for the same reason — two nights that agree is a
/// coincidence with a sample size.
pub const ENOUGH_NIGHTS: usize = 3;

/// How much of the evidence has to agree before anything is said.
///
/// Two thirds, and deliberately stricter than §81's half. A profile says what
/// a DJ tends to do, which they can recognise or dismiss; this says what a
/// room tends to do, which they cannot check.
pub const AGREE: f64 = 2.0 / 3.0;

/// Which way the room went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lift {
    Rose,
    Held,
    Fell,
}

impl Lift {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rose => "rose",
            Self::Held => "held",
            Self::Fell => "fell",
        }
    }

    /// Read from two levels, with [`MOVED`] as the dead band.
    #[must_use]
    pub fn between(before: f32, after: f32) -> Self {
        if after - before > MOVED {
            Self::Rose
        } else if before - after > MOVED {
            Self::Fell
        } else {
            Self::Held
        }
    }
}

/// What one sense did across one mix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    pub sense: Sense,
    /// The minute before the mix began.
    pub before: f32,
    /// The twelve to thirty seconds after it finished.
    pub after: f32,
    pub lift: Lift,
}

/// What the room did across a mix that began and ended at these moments.
///
/// One entry per sense that both windows could answer for. Empty when nothing
/// was looking, which is the ordinary case and is not an error: §37 is a
/// bonus a camera buys, not a thing djmanzo depends on.
///
/// `readings` may be in any order and may include far more of the night than
/// the two windows; the windows are cut here so that a caller cannot cut them
/// slightly differently.
#[must_use]
pub fn read(began: SystemTime, ended: SystemTime, readings: &[Reading]) -> Vec<Response> {
    [Sense::Movement, Sense::Loudness, Sense::Light]
        .into_iter()
        .filter_map(|sense| {
            let before = level(readings, sense, began.checked_sub(BEFORE)?, began)?;
            let after = level(
                readings,
                sense,
                ended.checked_add(LAG.0)?,
                ended.checked_add(LAG.1)?,
            )?;
            Some(Response {
                sense,
                before,
                after,
                lift: Lift::between(before, after),
            })
        })
        .collect()
}

/// The middle of one sense over a window, or `None` if too little landed in it.
///
/// A median, for the reason [`dj_assistant::room::Room::lately`] gives: one
/// frame where somebody walked across the lens is an outlier, and a mean
/// carries it into the answer.
fn level(readings: &[Reading], sense: Sense, from: SystemTime, to: SystemTime) -> Option<f32> {
    let mut values: Vec<f32> = readings
        .iter()
        .filter(|reading| reading.at >= from && reading.at <= to)
        .filter_map(|reading| match sense {
            Sense::Light => reading.light,
            Sense::Movement => reading.movement,
            Sense::Loudness => reading.loudness,
        })
        .collect();
    if values.len() < ENOUGH {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(values[values.len() / 2])
}

/// One kind of mix, at one kind of night, over however many nights there were.
///
/// Private fields and no public constructor but [`seen`], which is §13's and
/// §81's discipline again: the whole value of this type is that it *cannot*
/// be built from one night, and a struct literal anywhere would be a way
/// around that.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    setting: String,
    style: String,
    sense: Sense,
    nights: usize,
    rose: usize,
    fell: usize,
}

impl Seen {
    /// How many nights this is drawn from.
    #[must_use]
    pub const fn nights(&self) -> usize {
        self.nights
    }

    #[must_use]
    pub fn setting(&self) -> &str {
        &self.setting
    }

    #[must_use]
    pub fn style(&self) -> &str {
        &self.style
    }

    #[must_use]
    pub const fn sense(&self) -> Sense {
        self.sense
    }

    /// Which way it usually went, or `None` when the nights do not agree.
    ///
    /// `None` is the common answer and the important one: a room that rose
    /// twice and fell twice has told djmanzo nothing, and the failure this
    /// guards against is reporting the majority of four as a finding.
    #[must_use]
    pub fn usually(&self) -> Option<Lift> {
        #[allow(clippy::cast_precision_loss)]
        let share = |count: usize| count as f64 / self.nights as f64;
        if share(self.rose) >= AGREE {
            Some(Lift::Rose)
        } else if share(self.fell) >= AGREE {
            Some(Lift::Fell)
        } else {
            None
        }
    }

    /// What it says, or nothing at all.
    ///
    /// Always names the evidence and always names the kind of night, because
    /// §37's own sentence — "historically […] here" — is only worth anything
    /// with both. Never says *because*: see the module note.
    #[must_use]
    pub fn words(&self) -> Option<String> {
        let lift = self.usually()?;
        let sense = match self.sense {
            Sense::Movement => "the floor",
            Sense::Loudness => "the room",
            Sense::Light => "the light",
        };
        let went = match lift {
            Lift::Rose => "picked up",
            Lift::Fell => "went quiet",
            Lift::Held => return None,
        };
        Some(format!(
            "After a {} at a {} night, {sense} has {went} — {} of {} nights.",
            self.style,
            self.setting.replace('-', " "),
            if lift == Lift::Rose {
                self.rose
            } else {
                self.fell
            },
            self.nights,
        ))
    }
}

/// One night's verdict on one kind of mix.
///
/// A night is one vote however many times it did the thing, which is §81's
/// rule — "a gesture counts once per night, not once per press" — and it is
/// what stops a night with forty blends outvoting five whole nights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Night<'a> {
    pub session_id: &'a str,
    pub setting: &'a str,
    pub style: &'a str,
    pub sense: Sense,
    pub lift: Lift,
}

/// What has usually happened, per setting, style and sense.
///
/// Takes the *rows* rather than a database, so the arithmetic is testable
/// without one and so the query cannot quietly become part of the rule.
#[must_use]
pub fn seen(nights: &[Night<'_>]) -> Vec<Seen> {
    use std::collections::BTreeMap;

    // One vote per night per (setting, style, sense). A night that both rose
    // and fell across its blends has no opinion about blends and is dropped
    // rather than counted twice or resolved by whichever came last.
    let mut votes: BTreeMap<(&str, &str, &str, &str), Option<Lift>> = BTreeMap::new();
    for night in nights {
        let key = (
            night.setting,
            night.style,
            night.sense.name(),
            night.session_id,
        );
        match votes.get(&key) {
            None => {
                votes.insert(key, Some(night.lift));
            }
            Some(Some(seen)) if *seen != night.lift => {
                votes.insert(key, None);
            }
            Some(_) => {}
        }
    }

    // Keyed by the sense's *slug* rather than the sense, so the map needs no
    // ordering on a type that has no natural one. It is turned back into a
    // `Sense` once, below, where an unknown slug is dropped rather than
    // guessed — which cannot happen, because the slug came from a `Sense`.
    let mut tallies: BTreeMap<(&str, &str, &str), (usize, usize, usize)> = BTreeMap::new();
    for ((setting, style, sense, _), vote) in &votes {
        let tally = tallies.entry((setting, style, sense)).or_insert((0, 0, 0));
        tally.0 += 1;
        match vote {
            Some(Lift::Rose) => tally.1 += 1,
            Some(Lift::Fell) => tally.2 += 1,
            _ => {}
        }
    }

    tallies
        .into_iter()
        .filter(|(_, (nights, _, _))| *nights >= ENOUGH_NIGHTS)
        .filter_map(|((setting, style, slug), (nights, rose, fell))| {
            Some(Seen {
                setting: (*setting).to_owned(),
                style: (*style).to_owned(),
                sense: [Sense::Movement, Sense::Loudness, Sense::Light]
                    .into_iter()
                    .find(|sense| sense.name() == slug)?,
                nights,
                rose,
                fell,
            })
        })
        .collect()
}

/// One mix, read and ready to be written down.
#[derive(Debug, Clone, PartialEq)]
pub struct Noted {
    /// Seconds into the set the mix began — the key it is stored under.
    pub at_seconds: i64,
    /// The style, as `dj_core::action::TransitionStyle` spells it.
    pub style: String,
    /// One entry per sense that both windows could answer for. May be empty,
    /// and usually is: nothing was looking.
    pub responses: Vec<Response>,
}

/// Every mix whose window has closed and that has not been read yet.
///
/// **Deliberately not a live subscription.** It is called with whatever the
/// caller has — the log so far, how long the set has run, the room's readings
/// — and it answers about mixes that are *finished being observable*. Running
/// it twice does nothing the second time, because `done` remembers; running it
/// after a restart does nothing either, because the readings are gone and the
/// windows come back empty. Both are the right behaviour and neither needed a
/// state machine.
///
/// `elapsed` is how long the set has been running, on the same clock the log's
/// timestamps use; `now` is that same instant as a wall clock, which is what
/// the room's readings are stamped with. Two clocks rather than one because
/// the two halves of §37 keep time differently, and converting between them
/// here is one subtraction that would otherwise be done, slightly differently,
/// by every caller.
///
/// A mix that produced nothing is still marked done. Readings only age out, so
/// a window that was empty when it closed will not fill in later, and retrying
/// it every tick for the rest of the night is work with a known answer.
pub fn note(
    handovers: &[crate::mixes::Handover],
    elapsed: Duration,
    now: SystemTime,
    readings: &[Reading],
    done: &mut std::collections::BTreeSet<i64>,
) -> Vec<Noted> {
    let mut fresh = Vec::new();
    for handover in handovers {
        // The window has to have closed. Reading a mix whose thirty seconds
        // are still running would record whatever the room happened to be
        // doing at second fourteen and never look again.
        if elapsed < handover.ended + LAG.1 {
            continue;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let at_seconds = handover.began.as_secs() as i64;
        if !done.insert(at_seconds) {
            continue;
        }
        let Some(began) = wall(now, elapsed, handover.began) else {
            continue;
        };
        let Some(ended) = wall(now, elapsed, handover.ended) else {
            continue;
        };
        fresh.push(Noted {
            at_seconds,
            style: handover.style.as_str().to_owned(),
            responses: read(began, ended, readings),
        });
    }
    fresh
}

/// A moment in the set, as a wall clock.
fn wall(now: SystemTime, elapsed: Duration, at: Duration) -> Option<SystemTime> {
    now.checked_sub(elapsed.checked_sub(at)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: u64 = 1_700_000_000;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(START + seconds)
    }

    /// Readings every two seconds from `from` to `to`, at `level`.
    fn steady(from: u64, to: u64, level: f32, sense: Sense) -> Vec<Reading> {
        (from..to)
            .step_by(2)
            .map(|n| Reading::at(at(n)).with(sense, level))
            .collect()
    }

    /// A night where the floor sat at `before`, a mix ran from 300 to 320, and
    /// the floor sat at `after` from the moment the lag window opens.
    fn across(before: f32, after: f32) -> Vec<Reading> {
        let mut readings = steady(200, 320, before, Sense::Movement);
        // 320 + 12 = 332 is where §37's window opens.
        readings.extend(steady(332, 360, after, Sense::Movement));
        readings
    }

    /// **The window is the one §37 names, and nothing outside it counts.**
    ///
    /// The load-bearing arithmetic, and it needs stating as an *exclusion*
    /// rather than as a value. The median that makes this robust to one odd
    /// frame also makes it robust to the window being a few seconds wrong:
    /// widening it to start at the mix's end moves six readings into fifteen
    /// and the median does not budge, so a test that only checked the number
    /// passed with the window in the wrong place. What separates them is
    /// whether a reading counts at all.
    ///
    /// So: a room watched only in the twelve seconds djmanzo is meant to
    /// ignore has said nothing, and so has one watched only after the thirty
    /// are up. Neither may produce a response.
    #[test]
    fn the_room_is_read_twelve_to_thirty_seconds_after_the_mix() {
        // The mix runs 300 to 320, so §37's window is 332 to 350.
        let before = steady(240, 300, 0.2, Sense::Movement);
        let only = |from: u64, to: u64| {
            let mut readings = before.clone();
            readings.extend(steady(from, to, 0.9, Sense::Movement));
            read(at(300), at(320), &readings)
        };

        assert!(
            only(320, 332).is_empty(),
            "the twelve seconds §37 skips were counted"
        );
        assert!(
            only(350, 380).is_empty(),
            "the room was read after the window had closed"
        );

        let inside = only(332, 350);
        assert_eq!(
            inside.len(),
            1,
            "the window itself found nothing: {inside:?}"
        );
        assert_eq!(inside[0].lift, Lift::Rose);
        assert!((inside[0].after - 0.9).abs() < 1e-6);
    }

    /// **A floor that fills after a mix says so; one that empties says so.**
    #[test]
    fn a_room_that_moves_after_a_mix_is_reported_as_moving() {
        let up = read(at(300), at(320), &across(0.2, 0.8));
        assert_eq!(up.len(), 1, "only movement was measured: {up:?}");
        assert_eq!(up[0].lift, Lift::Rose);
        assert!((up[0].before - 0.2).abs() < 1e-6);
        assert!((up[0].after - 0.8).abs() < 1e-6);

        let down = read(at(300), at(320), &across(0.8, 0.2));
        assert_eq!(down[0].lift, Lift::Fell);
    }

    /// A change smaller than the sensor's own noise is not a change.
    #[test]
    fn a_room_that_barely_moved_did_not_move() {
        let read = read(at(300), at(320), &across(0.50, 0.50 + MOVED / 2.0));
        assert_eq!(read[0].lift, Lift::Held);
    }

    /// **Nothing was looking, so nothing is claimed.**
    ///
    /// The ordinary case: djmanzo runs without a camera and §37 is a bonus one
    /// buys. An empty answer here has to stay empty rather than become a
    /// confident zero.
    #[test]
    fn a_mix_nobody_watched_produces_nothing() {
        assert!(read(at(300), at(320), &[]).is_empty());
        // And a window with too little in it is the same answer.
        let thin = steady(200, 300, 0.4, Sense::Movement)
            .into_iter()
            .chain(steady(332, 338, 0.9, Sense::Movement))
            .collect::<Vec<_>>();
        assert!(
            read(at(300), at(320), &thin).is_empty(),
            "three readings is not a level"
        );
    }

    fn handover(began: u64, ended: u64) -> crate::mixes::Handover {
        use dj_core::DeckId;
        crate::mixes::Handover {
            out: DeckId::from_human(1).unwrap(),
            into: DeckId::from_human(2).unwrap(),
            out_track: None,
            in_track: None,
            began: Duration::from_secs(began),
            ended: Duration::from_secs(ended),
            style: dj_core::action::TransitionStyle::Blend,
        }
    }

    /// **A mix is not read until its own window has closed.**
    ///
    /// The failure this stops is subtle and would never look like one: reading
    /// at second fourteen records whatever the room was doing then, marks the
    /// mix done, and never looks again — so every response in the table would
    /// be the first two seconds of a thirty-second window, consistently, and
    /// the aggregate over them would look like evidence.
    #[test]
    fn a_mix_is_left_alone_until_its_thirty_seconds_are_up() {
        let mixes = [handover(300, 320)];
        let readings = across(0.2, 0.8);
        let mut done = std::collections::BTreeSet::new();

        // A second before the window closes.
        let early = note(
            &mixes,
            Duration::from_secs(349),
            at(349),
            &readings,
            &mut done,
        );
        assert!(early.is_empty(), "read before the window closed: {early:?}");
        assert!(done.is_empty(), "and marked done while it was at it");

        let ready = note(
            &mixes,
            Duration::from_secs(350),
            at(350),
            &readings,
            &mut done,
        );
        assert_eq!(ready.len(), 1, "{ready:?}");
        assert_eq!(ready[0].at_seconds, 300);
        assert_eq!(ready[0].style, "blend");
        assert_eq!(ready[0].responses[0].lift, Lift::Rose);
    }

    /// **Reading twice writes once.**
    ///
    /// The recorder runs on every snapshot, sixty times a second. Without this
    /// a single mix becomes a night's worth of identical rows, and a night
    /// that counts once would be the only thing standing between that and a
    /// number that means nothing.
    #[test]
    fn the_same_mix_is_only_ever_read_once() {
        let mixes = [handover(300, 320)];
        let readings = across(0.2, 0.8);
        let mut done = std::collections::BTreeSet::new();

        for tick in 0..5 {
            let elapsed = Duration::from_secs(350 + tick);
            let fresh = note(&mixes, elapsed, at(350 + tick), &readings, &mut done);
            assert_eq!(
                fresh.len(),
                usize::from(tick == 0),
                "tick {tick} produced {fresh:?}"
            );
        }
    }

    /// A mix nobody watched is still finished with, rather than retried for
    /// the rest of the night against readings that only age further out.
    #[test]
    fn a_mix_nobody_watched_is_not_retried_forever() {
        let mixes = [handover(300, 320)];
        let mut done = std::collections::BTreeSet::new();
        let first = note(&mixes, Duration::from_secs(350), at(350), &[], &mut done);
        assert_eq!(first.len(), 1);
        assert!(first[0].responses.is_empty(), "{first:?}");
        assert!(
            note(&mixes, Duration::from_secs(400), at(400), &[], &mut done).is_empty(),
            "an unwatched mix came back for another look"
        );
    }

    fn night<'a>(id: &'a str, setting: &'a str, style: &'a str, lift: Lift) -> Night<'a> {
        Night {
            session_id: id,
            setting,
            style,
            sense: Sense::Movement,
            lift,
        }
    }

    /// **Nothing is said about "usually" until there are three nights of it.**
    ///
    /// §37's whole payoff sentence is the word *historically*. Two nights that
    /// agree is a coincidence with a sample size, and this is the same floor
    /// §81 puts under a profile.
    #[test]
    fn two_nights_that_agree_are_still_a_coincidence() {
        // Written out rather than looped over `ENOUGH_NIGHTS`, which is what
        // the first draft did — and a loop bounded by the constant it is
        // testing is a tautology: lowering the constant shortens the loop and
        // the test stays green. §81's threshold test had the same bug and was
        // rewritten the same way.
        let none: [Night<'_>; 0] = [];
        assert!(seen(&none).is_empty());
        assert!(
            seen(&[night("a", "club", "blend", Lift::Rose)]).is_empty(),
            "one night was enough to generalise from"
        );
        assert!(
            seen(&[
                night("a", "club", "blend", Lift::Rose),
                night("b", "club", "blend", Lift::Rose),
            ])
            .is_empty(),
            "two nights that agree are a coincidence with a sample size"
        );

        let three = [
            night("a", "club", "blend", Lift::Rose),
            night("b", "club", "blend", Lift::Rose),
            night("c", "club", "blend", Lift::Rose),
        ];
        let found = seen(&three);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].nights(), 3);
        assert_eq!(found[0].usually(), Some(Lift::Rose));
    }

    /// **Nights that disagree say nothing, however many of them there are.**
    ///
    /// The failure this exists to stop: reporting the majority of a coin flip
    /// as a finding. Two of four is not "usually" and neither is three of six.
    #[test]
    fn a_room_that_does_both_has_told_djmanzo_nothing() {
        let split = [
            night("a", "club", "blend", Lift::Rose),
            night("b", "club", "blend", Lift::Rose),
            night("c", "club", "blend", Lift::Fell),
            night("d", "club", "blend", Lift::Fell),
        ];
        let found = seen(&split);
        assert_eq!(found.len(), 1, "the tally is there");
        assert_eq!(found[0].usually(), None, "and it says nothing");
        assert_eq!(found[0].words(), None);
    }

    /// **A night is one vote, however many mixes it contained.**
    ///
    /// §81's rule at this scale. Without it a night with forty blends outvotes
    /// five whole nights, and the sentence "five of six nights" becomes a
    /// sentence about one night that was busy.
    #[test]
    fn a_busy_night_gets_one_vote_like_every_other() {
        let mut nights: Vec<Night<'_>> = (0..40)
            .map(|_| night("loud", "club", "blend", Lift::Rose))
            .collect();
        nights.push(night("b", "club", "blend", Lift::Fell));
        nights.push(night("c", "club", "blend", Lift::Fell));
        nights.push(night("d", "club", "blend", Lift::Fell));

        let found = seen(&nights);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].nights(), 4, "forty mixes are one night: {found:?}");
        assert_eq!(
            found[0].usually(),
            Some(Lift::Fell),
            "the busy night outvoted three others"
        );
    }

    /// **A night that did both has no opinion, and does not lend one.**
    ///
    /// Its first mix is not the night's verdict and neither is its last. The
    /// arrangement here is the one that tells them apart: with the abstention
    /// counted as a `Rose` the tally is three of four and speaks; with it
    /// abstaining the tally is two of four and does not.
    #[test]
    fn a_night_that_did_both_abstains_rather_than_voting_twice() {
        // Arranged so that "abstains" and "takes its last verdict" give
        // different answers, which the obvious arrangement does not: an
        // abstention only ever removes a vote from one side, so it changes
        // nothing unless that vote is the one that crosses AGREE. Here night
        // `a` resolving to Fell would make it three of four and speak.
        let nights = [
            night("a", "club", "blend", Lift::Rose),
            night("a", "club", "blend", Lift::Fell),
            night("b", "club", "blend", Lift::Fell),
            night("c", "club", "blend", Lift::Rose),
            night("d", "club", "blend", Lift::Fell),
        ];
        let found = seen(&nights);
        assert_eq!(found[0].nights(), 4, "four nights, one of them undecided");
        assert_eq!(
            found[0].usually(),
            None,
            "the undecided night was resolved to one of its own mixes"
        );

        // And the same four nights *without* the contradicting mix do speak,
        // so the silence above is the abstention rather than the arithmetic.
        let decided = [
            night("a", "club", "blend", Lift::Rose),
            night("b", "club", "blend", Lift::Rose),
            night("c", "club", "blend", Lift::Rose),
            night("d", "club", "blend", Lift::Fell),
        ];
        assert_eq!(seen(&decided)[0].usually(), Some(Lift::Rose));
    }

    /// Kinds of night and kinds of mix are never pooled.
    ///
    /// "Historically, here" is two conditions, and dropping either turns a
    /// statement about club blends into a statement about everything.
    #[test]
    fn a_club_blend_is_not_a_wedding_cut() {
        let nights = [
            night("a", "club", "blend", Lift::Rose),
            night("b", "club", "blend", Lift::Rose),
            night("c", "wedding", "cut", Lift::Rose),
            night("d", "wedding", "cut", Lift::Rose),
        ];
        assert!(
            seen(&nights).is_empty(),
            "two and two were pooled into four"
        );
    }

    /// The sentence names the evidence, the kind of night and the kind of mix
    /// — and never says *because*.
    #[test]
    fn the_sentence_says_what_it_is_built_from() {
        let nights = [
            night("a", "open-format", "echo", Lift::Rose),
            night("b", "open-format", "echo", Lift::Rose),
            night("c", "open-format", "echo", Lift::Rose),
        ];
        let said = seen(&nights)[0].words().expect("three nights that agree");
        assert_eq!(
            said,
            "After a echo at a open format night, the floor has picked up — 3 of 3 nights."
        );
        assert!(!said.contains("because"), "{said}");
    }
}

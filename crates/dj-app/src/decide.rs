//! §118a: a decision, when one becomes necessary under pressure.
//!
//! > whenever in a stressful situation a decision becomes necessary a super
//! > easy and quick to identify and select widget can pop up (as unobtrusive
//! > or occupying as necessary and adequate)
//!
//! The pressure a DJ meets most is a record running out with nothing after
//! it. [`crate::whisper`] already notices it; what it offered was one line
//! and a button that opened the rail — a list of twelve to read while the
//! clock runs. A decision here is **three**, one per direction the night can
//! go (up, level, down), each told apart before it is read: an arrow and a
//! colour, then a title. Three is the number a hand picks from without
//! counting.
//!
//! # As much room as the moment needs
//!
//! [`presence`] is how much of the screen it takes, from the seconds left:
//! a line in the top bar while there is time to finish what the hands are
//! doing, a card that comes up by itself once there is not, and most of the
//! screen in the last seconds, when the only thing that matters is the next
//! record. The thresholds are here rather than in the interface so the rule
//! is a sentence a test can hold.
//!
//! # Chosen by the rail's own ranking
//!
//! Each direction's record is the top of what the rail would offer going
//! that way — the same scorer, the same pack, the same phase, the same
//! profile (`commands::next_decision`). [`pick`] only keeps the three apart:
//! a record that tops two directions is offered once, in the first, and the
//! other offers its next best.

use serde::Serialize;

/// Seconds left above which the decision is a line in the top bar.
pub const CARD_BELOW: f32 = 20.0;

/// Seconds left below which the decision takes most of the screen.
pub const WHOLE_BELOW: f32 = 8.0;

/// How much of the screen a decision takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    /// One line where the top bar has room. Nothing moves.
    Line,
    /// A card over the decks, by itself, with the choices on it.
    Card,
    /// Most of the screen: the choices large enough to hit without aiming.
    Whole,
}

/// How much room a decision with `left` seconds to go should take.
#[must_use]
pub fn presence(left: f32) -> Presence {
    if left >= CARD_BELOW {
        Presence::Line
    } else if left >= WHOLE_BELOW {
        Presence::Card
    } else {
        Presence::Whole
    }
}

/// Where the next record takes the night.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Lift,
    Hold,
    Ease,
}

impl Direction {
    /// In the order a decision offers them: up, level, down.
    pub const ALL: [Direction; 3] = [Direction::Lift, Direction::Hold, Direction::Ease];

    /// The trajectory name the rail ranks by.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Direction::Lift => "lift",
            Direction::Hold => "hold",
            Direction::Ease => "ease",
        }
    }

    /// What choosing it does to the night, in the booth's words.
    #[must_use]
    pub const fn says(self) -> &'static str {
        match self {
            Direction::Lift => "Lift the floor",
            Direction::Hold => "Keep it where it is",
            Direction::Ease => "Take it down",
        }
    }
}

/// One of a decision's choices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice<T> {
    pub direction: Direction,
    pub says: &'static str,
    #[serde(flatten)]
    pub offer: T,
}

/// A change in loudness this size the other way is one a room notices.
pub const AGAINST_DB: f64 = 1.0;

/// A change this size is no longer holding the level: about the smallest
/// step a room notices, which is the rail's own unit.
pub const HOLD_DB: f64 = 3.0;

/// Whether a record `delta_db` louder than the one playing goes the way
/// `direction` says. The rail's directions are made of loudness
/// (`dj_library::suggest`), and a choice shows its change in decibels, so a
/// *Lift the floor* two decibels quieter would be a label the numbers
/// beside it contradict. Nothing known is nothing against it.
#[must_use]
pub fn goes(direction: Direction, delta_db: Option<f64>) -> bool {
    let Some(delta) = delta_db.filter(|d| d.is_finite()) else {
        return true;
    };
    match direction {
        Direction::Lift => delta >= -AGAINST_DB,
        Direction::Hold => delta.abs() < HOLD_DB,
        Direction::Ease => delta <= AGAINST_DB,
    }
}

/// One choice per direction, each the best of its own ranking that no
/// earlier direction took and that `fits` it. A direction with nothing left
/// that goes its way is left out rather than filled with a repeat or with a
/// record that goes the other way.
pub fn pick<T, K: PartialEq>(
    ranked: [(Direction, Vec<T>); 3],
    key: impl Fn(&T) -> K,
    fits: impl Fn(Direction, &T) -> bool,
) -> Vec<Choice<T>> {
    let mut taken: Vec<K> = Vec::new();
    let mut out = Vec::new();
    for (direction, list) in ranked {
        if let Some(offer) = list
            .into_iter()
            .find(|item| !taken.contains(&key(item)) && fits(direction, item))
        {
            taken.push(key(&offer));
            out.push(Choice {
                direction,
                says: direction.says(),
                offer,
            });
        }
    }
    out
}

/// How many beats a stall loops.
pub const STALL_BEATS: u32 = 8;

/// The move that buys time instead of choosing: the record running out,
/// looped where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stall {
    pub says: String,
    /// The action, as any press sends it.
    pub run: String,
}

/// Loop `deck` where it is, for [`STALL_BEATS`] -- what a DJ does with a
/// record about to end and nothing after it, while they find one.
#[must_use]
pub fn stall(deck: u8) -> Stall {
    Stall {
        says: format!("Loop deck {deck} for {STALL_BEATS} beats"),
        run: format!("deck {deck} loop {STALL_BEATS}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stall is a real action, on the deck running out.
    #[test]
    fn a_stall_loops_the_deck_running_out() {
        let bought = stall(2);
        assert_eq!(bought.run, "deck 2 loop 8");
        assert!(
            dj_core::Action::parse(&bought.run).is_ok(),
            "{}",
            bought.run
        );
    }

    /// **As much room as the moment needs, and no more.** A line while
    /// there is time, a card once there is not, most of the screen at the
    /// end -- and never back to less as the seconds fall.
    #[test]
    fn a_decision_takes_more_room_as_the_seconds_run_out() {
        assert_eq!(presence(29.0), Presence::Line);
        assert_eq!(presence(CARD_BELOW), Presence::Line);
        assert_eq!(presence(CARD_BELOW - 0.1), Presence::Card);
        assert_eq!(presence(WHOLE_BELOW), Presence::Card);
        assert_eq!(presence(WHOLE_BELOW - 0.1), Presence::Whole);
        assert_eq!(presence(0.5), Presence::Whole);
        let mut last = Presence::Line;
        let rank = |p: Presence| match p {
            Presence::Line => 0,
            Presence::Card => 1,
            Presence::Whole => 2,
        };
        for tenth in (0..=300).rev() {
            let now = presence(tenth as f32 / 10.0);
            assert!(
                rank(now) >= rank(last),
                "shrank at {}s",
                tenth as f32 / 10.0
            );
            last = now;
        }
    }

    /// **Three different records, one per direction**, each the best its
    /// own ranking offers that an earlier direction did not take.
    #[test]
    fn a_record_is_offered_once_and_the_next_best_takes_its_place() {
        let ranked = [
            (Direction::Lift, vec!["a", "b"]),
            (Direction::Hold, vec!["a", "c"]),
            (Direction::Ease, vec!["b", "c", "d"]),
        ];
        let chosen = pick(ranked, |s| *s, |_, _| true);
        let offers: Vec<(Direction, &str)> =
            chosen.iter().map(|c| (c.direction, c.offer)).collect();
        assert_eq!(
            offers,
            [
                (Direction::Lift, "a"),
                (Direction::Hold, "c"),
                (Direction::Ease, "b"),
            ]
        );
        assert_eq!(chosen[0].says, "Lift the floor");

        // A direction with nothing new is left out, not filled with a repeat.
        let thin = [
            (Direction::Lift, vec!["a"]),
            (Direction::Hold, vec!["a"]),
            (Direction::Ease, vec![]),
        ];
        let chosen = pick(thin, |s| *s, |_, _| true);
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].direction, Direction::Lift);
    }

    /// **A direction is only claimed by a record that goes that way.** The
    /// choice shows its change in decibels; *Lift the floor* on a record
    /// two decibels quieter is a label its own numbers contradict, so the
    /// next record that does lift takes the slot -- or nothing does.
    #[test]
    fn a_direction_is_only_offered_by_a_record_that_goes_that_way() {
        assert!(goes(Direction::Lift, Some(2.0)));
        assert!(goes(Direction::Lift, Some(-0.5)), "level enough");
        assert!(!goes(Direction::Lift, Some(-2.0)));
        assert!(goes(Direction::Ease, Some(-3.0)));
        assert!(!goes(Direction::Ease, Some(2.0)));
        assert!(goes(Direction::Hold, Some(-2.0)));
        assert!(!goes(Direction::Hold, Some(4.0)));
        assert!(
            goes(Direction::Lift, None),
            "nothing known is nothing against it"
        );

        // Each direction's own ranking, as (name, dB louder).
        let ranked = [
            (Direction::Lift, vec![("quieter", -2.0), ("louder", 2.5)]),
            (Direction::Hold, vec![("quieter", -2.0)]),
            (Direction::Ease, vec![("louder", 2.5)]),
        ];
        let chosen = pick(ranked, |s| s.0, |d, s| goes(d, Some(s.1)));
        let offers: Vec<(Direction, &str)> =
            chosen.iter().map(|c| (c.direction, c.offer.0)).collect();
        assert_eq!(
            offers,
            [(Direction::Lift, "louder"), (Direction::Hold, "quieter")],
            "lift skipped the quieter record; ease had only a louder one, so it is left out"
        );
    }
}

//! §118a: the assistant's guides, one per topic.
//!
//! > the integrated ai assistant shall have different guides that are
//! > visually represented via floating widgets with visual presentations of
//! > topics and representations of (multi-choice) options that are easy to
//! > identify
//!
//! A guide is a topic the DJ can ask about at any moment, drawn as a
//! floating widget of a few large choices. What a guide *offers* is the
//! work of what already answers that question — the rail for the next
//! record ([`crate::decide`]), the planner for the mix
//! ([`crate::transition`]), the prepared night for trouble
//! ([`crate::gig::tonight`]). What this module decides is **which guides
//! can be opened now, and why not the others**: a guide that opened onto
//! nothing, or onto an error, would teach a DJ not to open it again.
//!
//! The decks it names are the ones the guide works on — the deck the room
//! is hearing most, and the one the next record goes on — so the interface
//! never has to work out which deck "the playing one" is. Only the decks on
//! screen count: the engine always has four, and a record loaded on one the
//! layout does not show would be a record loaded nowhere a DJ can see.

use crate::snapshot::{DeckSnapshot, Snapshot};
use serde::Serialize;

/// What a guide is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    /// Three records to follow what is playing, one each way.
    Next,
    /// How to bring the record waiting on the other deck in.
    Mix,
    /// The plans written for tonight's troubles.
    Trouble,
}

impl Topic {
    /// In the order the guides are offered.
    pub const ALL: [Topic; 3] = [Topic::Next, Topic::Mix, Topic::Trouble];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Topic::Next => "Next record",
            Topic::Mix => "The mix",
            Topic::Trouble => "If something goes wrong",
        }
    }

    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Topic::Next => "Three records to follow what is playing: up, level, or down.",
            Topic::Mix => "How to bring the waiting record in, each way drawn as it sounds.",
            Topic::Trouble => "The plans you wrote for tonight, one press each.",
        }
    }
}

/// One guide, as the launcher offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Guide {
    pub topic: Topic,
    pub title: &'static str,
    pub about: &'static str,
    /// Why it cannot be opened now, or `None` when it can.
    pub not_now: Option<String>,
    /// The deck the room is hearing most, which the guide works from.
    pub from: Option<u8>,
    /// The deck it works towards: an empty one for the next record, the
    /// loaded and waiting one for the mix.
    pub to: Option<u8>,
}

/// The playing deck the room hears most, among the first `shown`.
fn leading(snapshot: &Snapshot, shown: u8) -> Option<&DeckSnapshot> {
    let crossfader = snapshot.master.crossfader;
    snapshot
        .decks
        .iter()
        .filter(|d| d.number <= shown && d.playing)
        .max_by(|a, b| {
            crate::whisper::heard(a, crossfader).total_cmp(&crate::whisper::heard(b, crossfader))
        })
}

/// Every guide, each open or saying why not, over the first `shown` decks.
#[must_use]
pub fn guides(snapshot: &Snapshot, live: bool, shown: u8) -> Vec<Guide> {
    let lead = leading(snapshot, shown);
    let from = lead.map(|d| d.number);
    let other = |wanted: fn(&DeckSnapshot) -> bool| {
        snapshot
            .decks
            .iter()
            .find(|d| d.number <= shown && Some(d.number) != from && wanted(d))
            .map(|d| d.number)
    };
    Topic::ALL
        .into_iter()
        .map(|topic| {
            let (not_now, to) = match topic {
                Topic::Next => match (from, other(|d| !d.loaded)) {
                    (None, _) => (Some("Nothing is playing.".to_owned()), None),
                    (Some(_), None) => (
                        Some("Every other deck has a record on it already.".to_owned()),
                        None,
                    ),
                    (Some(_), to) => (None, to),
                },
                Topic::Mix => match (from, other(|d| d.loaded && !d.playing)) {
                    (None, _) => (Some("Nothing is playing.".to_owned()), None),
                    (Some(_), None) => (
                        Some("Load the next record on another deck first.".to_owned()),
                        None,
                    ),
                    (Some(_), to) => (None, to),
                },
                Topic::Trouble if live => (None, None),
                Topic::Trouble => (
                    Some("Only while a prepared night is being played.".to_owned()),
                    None,
                ),
            };
            Guide {
                topic,
                title: topic.title(),
                about: topic.about(),
                from: if topic == Topic::Trouble { None } else { from },
                not_now,
                to,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn booth() -> Snapshot {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 2);
        for deck in &mut snapshot.decks {
            deck.loaded = false;
            deck.playing = false;
            deck.volume = 1.0;
        }
        snapshot.master.crossfader = 0.0;
        snapshot
    }

    fn open(guides: &[Guide], topic: Topic) -> &Guide {
        guides
            .iter()
            .find(|g| g.topic == topic)
            .expect("every topic is offered")
    }

    /// **A guide opens onto something, or says why not.** Nothing playing
    /// opens neither the next record nor the mix; one deck playing and the
    /// other empty opens the next record towards the empty one; a record
    /// waiting on the other deck opens the mix towards it instead.
    #[test]
    fn a_guide_opens_onto_something_or_says_why_not() {
        let mut snapshot = booth();
        let quiet = guides(&snapshot, false, 2);
        assert_eq!(quiet.len(), Topic::ALL.len());
        assert!(quiet.iter().all(|g| g.not_now.is_some()), "{quiet:#?}");

        snapshot.decks[0].loaded = true;
        snapshot.decks[0].playing = true;
        let one = guides(&snapshot, false, 2);
        let next = open(&one, Topic::Next);
        assert_eq!(
            (next.not_now.as_deref(), next.from, next.to),
            (None, Some(1), Some(2))
        );
        assert_eq!(
            open(&one, Topic::Mix).not_now.as_deref(),
            Some("Load the next record on another deck first.")
        );

        snapshot.decks[1].loaded = true;
        let waiting = guides(&snapshot, false, 2);
        let mix = open(&waiting, Topic::Mix);
        assert_eq!(
            (mix.not_now.as_deref(), mix.from, mix.to),
            (None, Some(1), Some(2))
        );
        assert!(
            open(&waiting, Topic::Next).not_now.is_some(),
            "no deck to load on"
        );
    }

    /// The deck a guide works from is the one the room hears, not the
    /// first one playing: deck 1 playing behind a crossfader hard over to
    /// deck 2 is not what the room is listening to.
    #[test]
    fn a_guide_works_from_the_deck_the_room_hears() {
        let mut snapshot = booth();
        for deck in &mut snapshot.decks {
            deck.loaded = true;
            deck.playing = true;
        }
        snapshot.decks[0].crossfader_assign = dj_core::CrossfaderAssign::Left;
        snapshot.decks[1].crossfader_assign = dj_core::CrossfaderAssign::Right;
        snapshot.master.crossfader = 1.0;
        let both = guides(&snapshot, false, 2);
        assert_eq!(open(&both, Topic::Next).from, Some(2));
    }

    /// **Only the decks on screen.** The engine always has four; with two
    /// shown and both holding a record, there is nowhere to load the next
    /// one -- not the third deck nobody can see.
    #[test]
    fn only_the_decks_on_screen_count() {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 4);
        for deck in &mut snapshot.decks {
            deck.loaded = false;
            deck.playing = false;
            deck.volume = 1.0;
        }
        snapshot.decks[0].loaded = true;
        snapshot.decks[0].playing = true;
        snapshot.decks[1].loaded = true;
        let two = guides(&snapshot, false, 2);
        assert!(open(&two, Topic::Next).not_now.is_some(), "{two:#?}");
        let four = guides(&snapshot, false, 4);
        assert_eq!(open(&four, Topic::Next).to, Some(3));
        // A deck off screen is not what the room hears either.
        snapshot.decks[0].playing = false;
        snapshot.decks[2].loaded = true;
        snapshot.decks[2].playing = true;
        assert!(
            open(&guides(&snapshot, false, 2), Topic::Next)
                .from
                .is_none()
        );
    }

    /// Trouble is tonight's: open only while a prepared night is played.
    #[test]
    fn trouble_is_open_only_while_a_night_is_played() {
        let snapshot = booth();
        assert!(
            open(&guides(&snapshot, false, 2), Topic::Trouble)
                .not_now
                .is_some()
        );
        assert_eq!(
            open(&guides(&snapshot, true, 2), Topic::Trouble).not_now,
            None
        );
    }
}

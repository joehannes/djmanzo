//! Deck identity.

use serde::{Deserialize, Serialize};

/// Upper bound on decks. VirtualDJ offers 2, 4 and 6; six is the ceiling, and
/// fixing it here lets the parameter table be a flat array sized at compile time
/// instead of a map the audio thread would have to hash into.
pub const MAX_DECKS: usize = 6;

/// A deck, numbered from 0 internally and from 1 in anything user-facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeckId(u8);

impl DeckId {
    #[must_use]
    pub const fn new(index: u8) -> Option<Self> {
        if (index as usize) < MAX_DECKS {
            Some(Self(index))
        } else {
            None
        }
    }

    /// Parse a user-facing deck number, which is 1-based.
    #[must_use]
    pub const fn from_human(number: u8) -> Option<Self> {
        if number == 0 {
            None
        } else {
            Self::new(number - 1)
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The number shown in the interface and used in scripts.
    #[must_use]
    pub const fn human_number(self) -> u8 {
        self.0 + 1
    }

    /// Every deck, for iteration at startup.
    pub fn all() -> impl Iterator<Item = DeckId> {
        (0..MAX_DECKS as u8).map(DeckId)
    }
}

/// Which side of the crossfader a deck is on.
///
/// Fixed assignment (deck 1 left, deck 2 right) is fine while the interface
/// shows two decks, because the two decks *are* the two sides. It stops being
/// fine the moment four are on screen: decks 3 and 4 would be permanently
/// through, which means the crossfader — the one control a DJ uses without
/// looking — cannot reach half the decks.
///
/// Every hardware mixer since the 1980s solves this with a three-way switch per
/// channel, and DJs already know it, so that is what this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfaderAssign {
    /// Cut by the left half of the crossfader.
    Left,
    /// Cut by the right half.
    Right,
    /// Not cut at all — the crossfader cannot silence this deck.
    ///
    /// The default, and deliberately so: a deck that appears mid-set (a third
    /// deck for a loop, a fourth for an acapella) should be audible when its
    /// channel fader is up, not silent because the crossfader happens to be
    /// parked. Decks 1 and 2 are assigned left and right at startup, which is
    /// the convention; anything beyond them starts through.
    #[default]
    Thru,
}

impl CrossfaderAssign {
    /// The word that appears in an action, a script and a controller mapping.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            CrossfaderAssign::Left => "left",
            CrossfaderAssign::Right => "right",
            CrossfaderAssign::Thru => "thru",
        }
    }

    /// How this assignment travels through the parameter table, which is `f32`
    /// and has no room for an enum.
    #[must_use]
    pub const fn as_param(self) -> f32 {
        match self {
            CrossfaderAssign::Left => -1.0,
            CrossfaderAssign::Thru => 0.0,
            CrossfaderAssign::Right => 1.0,
        }
    }

    /// Recover an assignment from its parameter value.
    ///
    /// Rounded rather than compared exactly: the value crosses a lock-free table
    /// of `f32`, and an assignment is not something to lose to a bit pattern.
    #[must_use]
    pub fn from_param(value: f32) -> Self {
        match value.partial_cmp(&0.0) {
            Some(std::cmp::Ordering::Less) => CrossfaderAssign::Left,
            Some(std::cmp::Ordering::Greater) => CrossfaderAssign::Right,
            _ => CrossfaderAssign::Thru,
        }
    }

    /// The assignment a deck starts with: 1 left, 2 right, the rest through.
    #[must_use]
    pub const fn default_for(index: usize) -> Self {
        match index {
            0 => CrossfaderAssign::Left,
            1 => CrossfaderAssign::Right,
            _ => CrossfaderAssign::Thru,
        }
    }
}

impl std::fmt::Display for DeckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.human_number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_ids_are_bounded() {
        assert!(DeckId::new(0).is_some());
        assert!(DeckId::new(MAX_DECKS as u8 - 1).is_some());
        assert!(DeckId::new(MAX_DECKS as u8).is_none());
    }

    #[test]
    fn human_numbering_is_one_based() {
        let deck = DeckId::from_human(1).unwrap();
        assert_eq!(deck.index(), 0);
        assert_eq!(deck.human_number(), 1);
        assert_eq!(deck.to_string(), "1");
        assert!(DeckId::from_human(0).is_none());
    }

    #[test]
    fn all_yields_every_deck_once() {
        let ids: Vec<_> = DeckId::all().collect();
        assert_eq!(ids.len(), MAX_DECKS);
        assert_eq!(ids[0].index(), 0);
        assert_eq!(ids[MAX_DECKS - 1].index(), MAX_DECKS - 1);
    }
}

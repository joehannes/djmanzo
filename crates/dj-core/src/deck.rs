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

//! §26's *beat jump: contextual action* — the one item on its list that is
//! not a drag.
//!
//! > # Direct manipulation on the waveform
//! >
//! > The waveform should become an interactive control surface. Allow direct
//! > interaction with semantic elements. […] **Beat jump: contextual action.**
//! > […] The DJ should be able to physically grab the thing they are thinking
//! > about.
//!
//! Every other entry on §26's list names a thing with a position — a cue, a
//! phrase marker, the ends of a transition, a loop — and says *drag it*. A
//! beat jump has no position. It is not a mark on a record; it is a move you
//! make to one, which is precisely why §26 gives it the other verb.
//!
//! # What "contextual" is doing
//!
//! Not a fixed menu with a different name. The list is built from what is
//! actually under the playhead, so it changes:
//!
//! - **A phrase jump is offered only when the record has phrase structure.**
//!   An unanalysed record has no phrases to jump by, and an entry that moved
//!   the playhead by a guess would be worse than no entry — a DJ who used it
//!   once mid-mix would stop trusting the menu.
//! - **A move that would run off the record is not offered.** Forward sixteen
//!   beats, four beats from the end, is a disabled button pretending to be a
//!   choice. Near the top and near the end the menu is shorter, and that is
//!   the honest shape.
//!
//! # Every entry is an action djmanzo already accepts
//!
//! [`crate::handle::Option_`]'s rule, reused rather than restated: *a menu
//! whose entries are not actions is a second vocabulary*. Each row carries the
//! exact text [`dj_core::Action::parse`] takes, so a jump made from this menu,
//! from a controller, from a script and from the assistant are one thing in
//! one log — and a set replays with the menu's moves in it.
//!
//! # The arithmetic is here, not in the interface
//!
//! §26's standing rule, the same one [`crate::transition::Transition::end_at`]
//! follows. The interface says *a DJ right-clicked deck 2*; this decides what
//! can be offered. A component working out for itself whether sixteen beats
//! fit would need the grid, the tempo and the length, which is three things it
//! does not own and one answer that could disagree with the deck.

use crate::handle::Option_;

/// The jump sizes offered, in beats.
///
/// Four and eight, forward and back, and nothing else. A DJ nudging a record
/// into place is thinking in bars, and a menu offering one, two, four, eight,
/// sixteen and thirty-two is §18's attention budget spent on arithmetic in the
/// middle of a mix. The bigger moves are what the phrase entry is for, and it
/// is grid-aware in a way a fixed number is not.
const SIZES: [i32; 2] = [4, 8];

/// What can be jumped from here.
///
/// `position` and `length` are frames on the record; `beat_frames` is how long
/// one beat is, and `has_phrases` whether the analyser found phrase structure.
///
/// Empty when the deck has no record, no grid, or a tempo that makes no sense.
/// Empty is a real answer and the interface draws no menu for it: a record
/// djmanzo cannot count beats in is one it cannot offer a beat jump on, and a
/// menu with nothing in it is worse than none.
#[must_use]
pub fn from_here(
    deck: u8,
    position: f64,
    length: f64,
    beat_frames: f64,
    has_phrases: bool,
    phrase_beats: u32,
) -> Vec<Option_> {
    if !beat_frames.is_finite() || beat_frames <= 0.0 || !position.is_finite() || length <= 0.0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let fits = |beats: i32| {
        let moved = position + f64::from(beats) * beat_frames;
        moved >= 0.0 && moved <= length
    };

    // Back before forward, and the larger move outside the smaller one, so the
    // menu reads as a line through the record rather than as a list of
    // numbers: -8, -4, +4, +8.
    for beats in SIZES.iter().rev() {
        if fits(-beats) {
            out.push(Option_ {
                label: format!("Back {beats} beats"),
                action: format!("deck {deck} beatjump -{beats}"),
            });
        }
    }
    for beats in SIZES {
        if fits(beats) {
            out.push(Option_ {
                label: format!("Forward {beats} beats"),
                action: format!("deck {deck} beatjump {beats}"),
            });
        }
    }

    // The phrase, last and only when there is one. It is the move a DJ cannot
    // make on most hardware, because it needs the grid rather than a count.
    if has_phrases && phrase_beats > 0 {
        let beats = i32::try_from(phrase_beats).unwrap_or(i32::MAX);
        if fits(-beats) {
            out.push(Option_ {
                label: "Back a phrase".to_owned(),
                action: format!("deck {deck} phrasejump -1"),
            });
        }
        if fits(beats) {
            out.push(Option_ {
                label: "Forward a phrase".to_owned(),
                action: format!("deck {deck} phrasejump 1"),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minute of record at 120 BPM, 48 kHz: 24 000 frames to the beat.
    const BEAT: f64 = 24_000.0;
    const LENGTH: f64 = BEAT * 240.0;

    fn middle(has_phrases: bool) -> Vec<Option_> {
        from_here(1, LENGTH / 2.0, LENGTH, BEAT, has_phrases, 16)
    }

    /// **The load-bearing one: every entry is a line djmanzo can parse.**
    ///
    /// `handle::Option_`'s rule, and the reason this module builds the strings
    /// rather than the interface: a menu whose entries are not actions is a
    /// second vocabulary, and the first entry in it that djmanzo cannot parse
    /// is a menu item that does nothing with no error anywhere.
    #[test]
    fn every_entry_is_an_action_djmanzo_accepts() {
        let offered = middle(true);
        assert!(!offered.is_empty());
        for option in &offered {
            dj_core::Action::parse(&option.action)
                .unwrap_or_else(|e| panic!("{:?} is not an action: {e}", option.action));
            assert!(!option.label.is_empty());
        }
    }

    /// **A record with no phrase structure is not offered a phrase jump.**
    ///
    /// The half that makes this *contextual* rather than a fixed menu under
    /// another name. A jump by a phrase djmanzo has not found is a jump by a
    /// guess, and one bad move mid-mix is enough for a DJ to stop opening the
    /// menu.
    #[test]
    fn no_phrase_structure_means_no_phrase_jump() {
        let without = middle(false);
        assert!(
            !without.iter().any(|o| o.label.contains("phrase")),
            "a record with no phrases was offered a phrase jump"
        );
        let with = middle(true);
        assert!(with.iter().any(|o| o.label.contains("phrase")));
    }

    /// **A move that would run off the front is not offered.**
    ///
    /// Two beats in, back four and back eight are both off the record. What is
    /// left is the forward half, which is the honest menu for that moment.
    #[test]
    fn near_the_top_only_the_forward_moves_are_offered() {
        let offered = from_here(1, BEAT * 2.0, LENGTH, BEAT, true, 16);
        assert!(
            offered.iter().all(|o| !o.label.starts_with("Back")),
            "a jump before the start of the record was offered: {offered:?}"
        );
        assert!(offered.iter().any(|o| o.label == "Forward 4 beats"));
    }

    /// **And one that would run off the end is not offered either.**
    #[test]
    fn near_the_end_only_the_backward_moves_are_offered() {
        let offered = from_here(1, LENGTH - BEAT * 2.0, LENGTH, BEAT, true, 16);
        assert!(
            offered.iter().all(|o| !o.label.starts_with("Forward")),
            "a jump past the end of the record was offered: {offered:?}"
        );
        assert!(offered.iter().any(|o| o.label == "Back 4 beats"));
    }

    /// **A deck with no grid is offered nothing**, rather than a menu of moves
    /// it cannot measure.
    #[test]
    fn no_grid_is_no_menu() {
        assert!(from_here(1, 0.0, LENGTH, 0.0, true, 16).is_empty());
        assert!(from_here(1, 0.0, LENGTH, f64::NAN, true, 16).is_empty());
        assert!(from_here(1, 0.0, 0.0, BEAT, true, 16).is_empty());
    }

    /// **The menu names the deck it was opened on.**
    ///
    /// Four decks and one menu component: an action that always said `deck 1`
    /// would move the wrong record, and would look right doing it.
    #[test]
    fn the_actions_name_the_deck_they_were_asked_about() {
        for deck in 1..=4u8 {
            for option in from_here(deck, LENGTH / 2.0, LENGTH, BEAT, true, 16) {
                assert!(
                    option.action.starts_with(&format!("deck {deck} ")),
                    "a menu for deck {deck} offered {:?}",
                    option.action
                );
            }
        }
    }

    /// **It reads as a line through the record**: back first, largest outside.
    ///
    /// Not decoration. A menu whose entries are in an arbitrary order is one a
    /// DJ has to read rather than aim at, and §18 says what mid-mix reading
    /// costs.
    #[test]
    fn the_moves_are_ordered_outward_from_where_the_playhead_is() {
        let offered = middle(true);
        let labels: Vec<&str> = offered.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Back 8 beats",
                "Back 4 beats",
                "Forward 4 beats",
                "Forward 8 beats",
                "Back a phrase",
                "Forward a phrase",
            ]
        );
    }
}

//! §74's contextual control rail: the four to eight controls that matter now.
//!
//! A deck has more controls than a rail can hold and more than a DJ needs at
//! any one moment. The directive's idea is that a small strip should carry
//! whichever of them the moment calls for — its three examples are a stem
//! transition, scratching, and readying a record — and that this is "the core
//! idea of adaptive UI".
//!
//! Two rules make that safe rather than hostile.
//!
//! **The mode follows from what the DJ just did.** A hand on the platter, a
//! stem muted, a deck stopped: every mode change here is a consequence of an
//! action they took, so the rail never rearranges itself under a hand reaching
//! for it. That is the failure `cockpit::Attention::reflow` exists to prevent,
//! and it applies to a rail's contents as much as to a panel's position.
//!
//! **Six at most, always.** §74 asks for four to eight;
//! `cockpit::Attention::performing` allows six while mixing, which is the
//! tightest of the four budgets. Sizing every mode to the tightest one means
//! the rail does not shrink when the music starts — a strip you have to
//! re-learn at the worst moment is worse than one control fewer. A test holds
//! this to the budget rather than to a number written here.
//!
//! What the rail does **not** do is latch. Every control it promotes also
//! exists as a widget on the deck, and that widget is where its state is shown;
//! a second, lit copy in the rail would be two things claiming to say whether
//! slip is on. The rail makes a control reachable, which is what "promote"
//! means.

use serde::{Deserialize, Serialize};

/// The most controls a rail may offer.
///
/// Checked against `cockpit::Attention::performing().promoted_controls` by a
/// test in `dj-app`, which is where that budget lives.
pub const MOST: usize = 6;

/// What a deck is doing, which decides what its rail offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RailMode {
    /// A hand is on the platter.
    Scratch,
    /// A stem is muted or a solo is held.
    Stems,
    /// A record is loaded and not playing.
    Preparing,
    /// Playing. The ordinary case, and the one §74 does not give a list for.
    Mixing,
}

/// One control the rail offers.
///
/// A label and a **deck-relative verb**, which is the whole of it: the rail
/// sends the same actions a pad or a key does, so a control promoted here is
/// the control itself and not a second implementation of it. See ADR-0003.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RailControl {
    pub label: &'static str,
    /// The verb and its argument, without the `deck N` in front.
    pub verb: &'static str,
}

const fn c(label: &'static str, verb: &'static str) -> RailControl {
    RailControl { label, verb }
}

impl RailMode {
    /// Every mode, in no particular order — nothing chooses between them by
    /// hand, [`RailMode::of`] does.
    pub const ALL: [RailMode; 4] = [
        RailMode::Scratch,
        RailMode::Stems,
        RailMode::Preparing,
        RailMode::Mixing,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            RailMode::Scratch => "scratch",
            RailMode::Stems => "stems",
            RailMode::Preparing => "preparing",
            RailMode::Mixing => "mixing",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }

    /// Which mode a deck in this state is in.
    ///
    /// Ordered by immediacy, not by importance. A hand on the platter is
    /// happening *now* and outranks a stem that was muted a minute ago; a
    /// muted stem outranks the fact that the deck happens to be playing. The
    /// last case is the ordinary one.
    ///
    /// An unloaded deck reads as preparing, which is what it is. Whether to
    /// draw a rail for a deck with nothing on it is the interface's question,
    /// and it answers it the same way it does for the overview.
    #[must_use]
    pub const fn of(playing: bool, jog_touched: bool, stems_in_use: bool) -> Self {
        if jog_touched {
            RailMode::Scratch
        } else if stems_in_use {
            RailMode::Stems
        } else if playing {
            RailMode::Mixing
        } else {
            RailMode::Preparing
        }
    }

    /// The controls this mode promotes, in order.
    #[must_use]
    pub const fn controls(self) -> &'static [RailControl] {
        match self {
            RailMode::Scratch => &SCRATCH,
            RailMode::Stems => &STEMS,
            RailMode::Preparing => &PREPARING,
            RailMode::Mixing => &MIXING,
        }
    }
}

// §74's list is jog, scratch mode, brake, reverse, cue. The jog is the wheel
// itself, which the deck already draws and a strip of buttons cannot be;
// "scratch mode" is the platter's behaviour, which is `jog_mode`. Slip is the
// sixth because it is what makes scratching something you can walk away from —
// the record lands where it would have been.
static SCRATCH: [RailControl; 6] = [
    c("Cue", "cue"),
    c("Vinyl", "jog_mode vinyl"),
    c("Slip", "slip_toggle"),
    c("Reverse", "reverse_toggle"),
    c("Brake", "brake 2"),
    c("Spin", "backspin 2"),
];

// §74's list is vocal, drums, bass, instrumental, stem FX, loop. Four stems and
// a loop; **stem FX has no button in the vocabulary** and is not invented here
// — a per-stem filter is a continuous control, which is a knob's gesture and
// not a switch's. It is on the deck's own stem widget.
static STEMS: [RailControl; 6] = [
    c("Vocal", "stem_mute vocal"),
    c("Drums", "stem_mute drums"),
    c("Bass", "stem_mute bass"),
    c("Other", "stem_mute other"),
    c("Loop 4", "loop 4"),
    c("Phrase", "loop_phrase 1"),
];

// §74's list is cue, loop, phrase, tags, rating, transition points. The first
// three are here; **tags, rating and transition points have no action** — they
// are a library row and a panel, and a rail entry is an action by
// construction. The three that take their places are the rest of readying a
// record: putting the grid where the music is.
static PREPARING: [RailControl; 6] = [
    c("Cue", "cue"),
    c("Loop 4", "loop 4"),
    c("Phrase", "loop_phrase 1"),
    c("Grid here", "grid_here"),
    c("Tap", "grid_tap"),
    c("Reset grid", "grid_reset"),
];

// §74 gives no list for the ordinary case, so this is a choice: the controls a
// blend needs, in the order a hand reaches for them. Nothing here is
// destructive, which matters because this is the mode a deck is in while it is
// audible.
static MIXING: [RailControl; 6] = [
    c("Sync", "sync_toggle"),
    c("Loop 4", "loop 4"),
    c("Phrase", "loop_phrase 1"),
    c("Keylock", "keylock_toggle"),
    c("Slip", "slip_toggle"),
    c("Headphones", "cue_toggle"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_name() {
        for mode in RailMode::ALL {
            assert_eq!(RailMode::parse(mode.name()), Some(mode));
        }
        assert_eq!(RailMode::parse("scratching"), None);
    }

    /// **No mode is bigger than the rail.** §74 asks for four to eight and the
    /// attention budget allows six while performing; a rail that overflowed
    /// would either scroll — which is not a rail — or drop a control without
    /// saying which.
    #[test]
    fn no_mode_offers_more_than_the_rail_can_hold() {
        for mode in RailMode::ALL {
            let controls = mode.controls();
            assert!(
                controls.len() <= MOST,
                "{} offers {} controls",
                mode.name(),
                controls.len()
            );
            assert!(
                controls.len() >= 4,
                "{} offers only {} -- §74 asks for at least four",
                mode.name(),
                controls.len()
            );
        }
    }

    /// **A hand on the platter outranks everything.** It is the one input that
    /// is happening at this instant, and a rail that answered a stem muted a
    /// minute ago while somebody is scratching would be offering the past.
    #[test]
    fn the_mode_follows_the_most_immediate_thing_the_dj_is_doing() {
        assert_eq!(
            RailMode::of(true, true, true),
            RailMode::Scratch,
            "a hand on the platter has to win"
        );
        assert_eq!(RailMode::of(true, false, true), RailMode::Stems);
        assert_eq!(RailMode::of(true, false, false), RailMode::Mixing);
        assert_eq!(RailMode::of(false, false, false), RailMode::Preparing);
    }

    /// No mode may offer the same control twice: six slots is few enough that
    /// a duplicate costs a control the DJ does not get.
    #[test]
    fn a_mode_never_offers_the_same_control_twice() {
        for mode in RailMode::ALL {
            let mut seen = std::collections::HashSet::new();
            for control in mode.controls() {
                assert!(
                    seen.insert(control.verb),
                    "{} offers {} twice",
                    mode.name(),
                    control.verb
                );
            }
        }
    }
}

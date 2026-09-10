//! The four to eight controls that matter right now — §74's contextual rail.
//!
//! Named `at_hand` rather than `rail` because djmanzo already has a rail:
//! §22's **Next** rail, three to eight candidates for the next record. Two
//! things called the same thing in one codebase is a session lost to reading
//! the wrong one, and this is what the panel is called on screen anyway.
//!
//! [§74](../../../docs/DIRECTIVE.md) asks for "a compact contextual rail"
//! holding "whichever 4–8 controls are most relevant now", gives three
//! examples — a stem transition, scratching, preparing — and closes with *this
//! is the core idea of adaptive UI*.
//!
//! # It reads the hands, not the night
//!
//! §11's context engine reads the *night*: where the set is in its arc, how
//! sure that reading is. That is the wrong clock for this. A rail answers
//! "what am I doing this second", and the answer changes when a hand lands on
//! a platter — three minutes before any engine would notice the tempo had
//! risen. So the reading here is the snapshot: a hand on the jog, a stem
//! muted, a record playing against another, a deck cued and waiting.
//!
//! Both readings exist and neither replaces the other, which is why this is a
//! separate module rather than another consumer of [`crate::night`].
//!
//! # Every control is an action the parser already accepts
//!
//! The rail proposes nothing new. Each entry carries the exact text
//! `dj_core::Action::parse` takes, so pressing one is the same event as typing
//! it, mapping a controller to it, or the assistant asking for it — one
//! execution path, as [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)
//! requires. A test parses every action of every rail, so a rail cannot offer
//! a verb djmanzo does not have.
//!
//! Unlike the palette, a control **may** carry an argument: `loop 4` and
//! `eq_low 0` are exactly the sort of thing "most relevant now" means, and the
//! palette refuses them only because it would have to invent the number.
//! Choosing the number *is* this module's job.
//!
//! # What §74 lists and this does not carry
//!
//! Stem FX, tags, rating and transition points. The first is a rack whose
//! controls are a slot, an effect, a wet amount and a beat division — four
//! numbers, not a button. The other three belong to a record rather than to a
//! deck, and a rail that edited a rating would be the browser's job done
//! somewhere a DJ cannot see which record they were changing.

use crate::snapshot::DeckSnapshot;

/// What the hands are doing.
///
/// Ordered by immediacy rather than importance: whatever is checked first wins
/// when two are true at once, and a hand already on the platter is more
/// present than a mix that has been running for eight bars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doing {
    /// A hand is on the platter.
    Scratching,
    /// The stems are being played rather than the record.
    Stems,
    /// This record is playing against another.
    Mixing,
    /// Loaded, not playing: the work before a record goes out.
    Preparing,
    /// Playing, with nothing else to say about it.
    Playing,
}

impl Doing {
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Doing::Scratching => "scratching",
            Doing::Stems => "stems",
            Doing::Mixing => "mixing",
            Doing::Preparing => "preparing",
            Doing::Playing => "playing",
        }
    }

    /// Why this set and not another, in the DJ's own terms.
    ///
    /// Said rather than left to be inferred: a rail whose contents change
    /// without explanation is a rail a DJ stops trusting, and the whole of
    /// §74 rests on being trusted enough to reach for without looking.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Doing::Scratching => "your hand is on the platter",
            Doing::Stems => "you are playing the stems",
            Doing::Mixing => "this record is playing against another",
            Doing::Preparing => "this record is cued and waiting",
            Doing::Playing => "this record is playing",
        }
    }
}

/// One control on the rail.
#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    /// What it says. A word, because the rail is scanned rather than read.
    pub label: String,
    /// The action, exactly as `dj_core::Action::parse` accepts it.
    pub action: String,
    /// True when the thing it controls is currently on.
    ///
    /// A control that latches shows its state; a momentary one is always
    /// false. Both are on the same rail because a DJ reaching for *reverse*
    /// does not first ask which kind it is.
    pub on: bool,
}

impl Control {
    fn new(label: impl Into<String>, action: impl Into<String>, on: bool) -> Self {
        Self {
            label: label.into(),
            action: action.into(),
            on,
        }
    }
}

/// What is at hand on one deck.
#[derive(Debug, Clone, PartialEq)]
pub struct AtHand {
    pub deck: u8,
    pub doing: Doing,
    pub because: &'static str,
    pub controls: Vec<Control>,
}

/// The fewest and most controls a rail may hold.
///
/// §74's own numbers. The floor matters as much as the ceiling: a rail with
/// two things on it is a gap where a DJ expected a row, and they will look at
/// it during a mix to find out why.
pub const FEWEST: usize = 4;
pub const MOST: usize = 8;

/// Two stem volumes this far apart mean one of them was pulled.
const STEM_TOUCHED: f32 = 0.05;

/// An EQ band at or below this is out rather than trimmed.
///
/// The same quarter `dj_assistant::coach` and `crate::mixes` use to recognise
/// a bass swap. Three numbers for one judgement would let the rail say the
/// bass is back while the night's own list of the mix says it came out.
const BAND_IS_OUT: f32 = 0.25;

/// What this deck's rail should hold.
///
/// `against` is whether another deck is audible, which is the one thing a
/// rail cannot see from its own deck — and the difference between *mixing*
/// and merely *playing*.
#[must_use]
pub fn at_hand(deck: &DeckSnapshot, against: bool) -> AtHand {
    let doing = doing(deck, against);
    AtHand {
        deck: deck.number,
        doing,
        because: doing.because(),
        controls: controls(deck, doing),
    }
}

/// Which deck the hands are on.
///
/// A rail is one row of controls, so something has to choose whose. §41's
/// `ui focus` exists but fades after six seconds by design — attention is a
/// moment, not an arrangement — so it cannot be what a rail follows for a
/// whole set.
///
/// The rule is the same one the states are ordered by, applied across decks
/// instead of within one: a hand on a platter, then stems being played, then
/// the record being got ready — because during a set the deck that is *not*
/// playing to the room is the one being worked on. Failing all of that, the
/// lowest-numbered deck with a record on it, so the answer is stable rather
/// than flickering between two idle decks.
///
/// `None` when no deck has a record on it.
#[must_use]
pub fn busiest(decks: &[DeckSnapshot]) -> Option<u8> {
    let loaded = || decks.iter().filter(|d| d.loaded);
    let first = |mut matching: Box<dyn Iterator<Item = &DeckSnapshot> + '_>| {
        matching.next().map(|d| d.number)
    };
    first(Box::new(loaded().filter(|d| d.jog_touched)))
        .or_else(|| first(Box::new(loaded().filter(|d| stems_in_play(d)))))
        .or_else(|| first(Box::new(loaded().filter(|d| !d.playing))))
        .or_else(|| first(Box::new(loaded())))
}

fn doing(deck: &DeckSnapshot, against: bool) -> Doing {
    if !deck.loaded {
        // Nothing on it: the controls that make sense are the ones for getting
        // a record ready, which is what preparing is.
        return Doing::Preparing;
    }
    if deck.jog_touched {
        return Doing::Scratching;
    }
    if stems_in_play(deck) {
        return Doing::Stems;
    }
    if deck.playing && against {
        return Doing::Mixing;
    }
    if deck.playing {
        Doing::Playing
    } else {
        Doing::Preparing
    }
}

/// Whether the stems are being played rather than the record.
///
/// A mute, a solo, or one stem's volume pulled away from the others. Not the
/// stem EQ or filter: those are shaping, and a DJ who trimmed a stem's highs
/// an hour ago is not "playing the stems" now — the rail would latch into that
/// state and never leave it.
///
/// **Unequal, not away from unity.** The first version asked whether any
/// volume differed from 1.0, which is true of every deck in the first second
/// after launch: the registry reads 0.0 for a stem the engine has not
/// published yet, so every deck would have opened on the stem row. Comparing
/// the four against *each other* says the same thing about a pulled stem and
/// nothing at all about four that have never been touched, whatever value they
/// are all sitting at.
fn stems_in_play(deck: &DeckSnapshot) -> bool {
    if deck.stem_soloing || deck.stem_mutes.iter().any(|muted| *muted) {
        return true;
    }
    let spread = deck
        .stem_volumes
        .iter()
        .copied()
        .fold((f32::MAX, f32::MIN), |(low, high), v| {
            (low.min(v), high.max(v))
        });
    spread.1 - spread.0 > STEM_TOUCHED
}

fn controls(deck: &DeckSnapshot, doing: Doing) -> Vec<Control> {
    let n = deck.number;
    match doing {
        // §74's list: jog, scratch mode, brake, reverse, cue. The jog is the
        // platter itself and the scratch mode is a setting rather than a move,
        // so what is left is the four things a hand reaches for *while* the
        // other hand is on the record.
        Doing::Scratching => vec![
            Control::new("cue", format!("deck {n} cue"), false),
            Control::new("reverse", format!("deck {n} reverse_toggle"), deck.reversed),
            Control::new("censor", format!("deck {n} censor_on"), false),
            Control::new("slip", format!("deck {n} slip_toggle"), deck.slip),
            Control::new("play", format!("deck {n} play_pause"), deck.playing),
        ],
        // §74's list, minus the stem FX rack. Each stem is a mute rather than
        // a fader because a rail is pressed, not swept.
        Doing::Stems => vec![
            Control::new(
                "vocal",
                format!("deck {n} stem_mute vocal"),
                deck.stem_mutes[0],
            ),
            Control::new(
                "drums",
                format!("deck {n} stem_mute drums"),
                deck.stem_mutes[1],
            ),
            Control::new(
                "bass",
                format!("deck {n} stem_mute bass"),
                deck.stem_mutes[2],
            ),
            Control::new(
                "other",
                format!("deck {n} stem_mute other"),
                deck.stem_mutes[3],
            ),
            looping(deck),
        ],
        // The moves a mix is actually made of. The bass is one entry rather
        // than two, and what it says is what pressing it will do — a row with
        // "bass out" and "bass in" side by side is a row where half the
        // buttons are always wrong.
        Doing::Mixing => vec![
            Control::new("sync", format!("deck {n} sync"), deck.synced),
            bass(deck),
            Control::new(
                "filter",
                format!(
                    "deck {n} filter {}",
                    if deck.filter.abs() > 0.01 { 0.0 } else { -0.6 }
                ),
                deck.filter.abs() > 0.01,
            ),
            Control::new("keylock", format!("deck {n} keylock_toggle"), deck.keylock),
            looping(deck),
        ],
        // §74's list, minus the three that belong to a record rather than a
        // deck — tags, rating and transition points are the browser's and the
        // pair view's, and doing them here would be doing them where a DJ
        // cannot see which record they are changing.
        Doing::Preparing => vec![
            Control::new("cue", format!("deck {n} cue"), false),
            Control::new("mark", format!("deck {n} hotcue_set 1"), false),
            looping(deck),
            Control::new("sync", format!("deck {n} sync"), deck.synced),
            Control::new("keylock", format!("deck {n} keylock_toggle"), deck.keylock),
        ],
        Doing::Playing => vec![
            Control::new("cue", format!("deck {n} cue"), false),
            looping(deck),
            Control::new("sync", format!("deck {n} sync"), deck.synced),
            bass(deck),
            Control::new("slip", format!("deck {n} slip_toggle"), deck.slip),
        ],
    }
}

/// One loop control whose label says what pressing it does.
fn looping(deck: &DeckSnapshot) -> Control {
    let n = deck.number;
    match deck.active_loop {
        Some(_) => Control::new("loop off", format!("deck {n} loop_off"), true),
        None => Control::new("loop 4", format!("deck {n} loop 4"), false),
    }
}

/// And one for the low band, for the same reason.
fn bass(deck: &DeckSnapshot) -> Control {
    let n = deck.number;
    if deck.eq_low <= BAND_IS_OUT {
        Control::new("bass in", format!("deck {n} eq_low 1"), true)
    } else {
        Control::new("bass out", format!("deck {n} eq_low 0"), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Action;

    /// A deck as the application actually starts it, with a record on it.
    ///
    /// From a *seeded* registry rather than a bare one, for the reason
    /// `crate::world`'s tests give: a bare registry reads zero for every
    /// parameter, and for the ones where zero is a legitimate value that is a
    /// confident wrong answer. An EQ at zero is a bass that has been pulled
    /// out, and this module reads exactly that.
    fn deck() -> DeckSnapshot {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 2);
        let mut view = snapshot.decks.remove(0);
        view.loaded = true;
        // The engine publishes every stem at unity and this registry has never
        // run a block, so they read zero. Set here rather than left, because a
        // fixture where all four are zero is one where "unequal" and "away
        // from unity" cannot be told apart — and telling them apart is the
        // whole of `stems_in_play`.
        view.stem_volumes = [1.0; 4];
        view
    }

    fn every_state() -> Vec<AtHand> {
        let scratching = DeckSnapshot {
            jog_touched: true,
            playing: true,
            ..deck()
        };
        let stems = DeckSnapshot {
            stem_mutes: [true, false, false, false],
            playing: true,
            ..deck()
        };
        let playing = DeckSnapshot {
            playing: true,
            ..deck()
        };
        let empty = DeckSnapshot {
            loaded: false,
            ..deck()
        };
        vec![
            at_hand(&scratching, false),
            at_hand(&stems, false),
            at_hand(&playing, true),
            at_hand(&deck(), false),
            at_hand(&playing, false),
            at_hand(&empty, false),
        ]
    }

    /// **Every control is an action djmanzo already accepts.**
    ///
    /// The rail proposes nothing new: pressing one is the same event as typing
    /// it, mapping a controller to it, or the assistant asking for it. A verb
    /// the parser rejects would be a button that does nothing, in the one row
    /// a DJ reaches for without looking.
    #[test]
    fn every_control_on_every_rail_is_a_real_action() {
        for hand in every_state() {
            for control in &hand.controls {
                assert!(
                    Action::parse(&control.action).is_ok(),
                    "{} offers {:?}, which the parser refuses",
                    hand.doing.slug(),
                    control.action
                );
            }
        }
    }

    /// **§74's own numbers, and the floor matters as much as the ceiling.**
    ///
    /// A rail with two things on it is a gap where a DJ expected a row, and
    /// they will look at it during a mix to find out why.
    #[test]
    fn every_rail_holds_between_four_and_eight_controls() {
        for hand in every_state() {
            let count = hand.controls.len();
            assert!(
                (FEWEST..=MOST).contains(&count),
                "{} has {count} controls, and §74 asks for {FEWEST} to {MOST}",
                hand.doing.slug()
            );
            // And no two of them are the same press twice.
            let mut actions: Vec<&str> = hand.controls.iter().map(|c| c.action.as_str()).collect();
            actions.sort_unstable();
            let before = actions.len();
            actions.dedup();
            assert_eq!(
                before,
                actions.len(),
                "{} repeats a control",
                hand.doing.slug()
            );
        }
    }

    /// **A hand on the platter wins over everything else.**
    ///
    /// The ordering is the whole of what makes this adaptive rather than
    /// arbitrary: a deck can be scratching *and* mixing *and* have a stem
    /// muted, and the rail has to pick one. The most immediate thing is what a
    /// hand is already touching.
    #[test]
    fn the_most_immediate_thing_decides() {
        let busy = DeckSnapshot {
            jog_touched: true,
            playing: true,
            stem_mutes: [true, false, false, false],
            ..deck()
        };
        assert_eq!(at_hand(&busy, true).doing, Doing::Scratching);

        // Take the hand off and the stems are what is left.
        let stems = DeckSnapshot {
            jog_touched: false,
            ..busy.clone()
        };
        assert_eq!(at_hand(&stems, true).doing, Doing::Stems);

        // Un-mute it and it is a mix.
        let mixing = DeckSnapshot {
            stem_mutes: [false; 4],
            ..stems.clone()
        };
        assert_eq!(at_hand(&mixing, true).doing, Doing::Mixing);

        // And with nothing else audible it is simply playing.
        assert_eq!(at_hand(&mixing, false).doing, Doing::Playing);
    }

    /// **A stem trimmed an hour ago is not "playing the stems".**
    ///
    /// Shaping is not performing. Without this the rail would latch into the
    /// stem row the first time somebody touched a stem's highs and never leave
    /// it — which is the failure mode that makes adaptive interfaces annoying
    /// rather than useful.
    #[test]
    fn shaping_a_stem_is_not_playing_the_stems() {
        let shaped = DeckSnapshot {
            playing: true,
            stem_eq: [[1.0, 1.0, 0.4]; 4],
            stem_filters: [0.5; 4],
            ..deck()
        };
        assert_eq!(at_hand(&shaped, false).doing, Doing::Playing);

        // Pulling one stem down *is* playing them.
        let played = DeckSnapshot {
            stem_volumes: [0.0, 1.0, 1.0, 1.0],
            ..shaped.clone()
        };
        assert_eq!(at_hand(&played, false).doing, Doing::Stems);

        // And four stems the engine has not published yet — all zero, which is
        // every deck in the first second after launch — are not four stems
        // somebody pulled. Without this every deck opens on the stem row.
        let fresh = DeckSnapshot {
            stem_volumes: [0.0; 4],
            ..shaped
        };
        assert_eq!(at_hand(&fresh, false).doing, Doing::Playing);
    }

    /// **A control that latches says what pressing it will do.**
    ///
    /// Not what it did. A row with "bass out" and "bass in" side by side is a
    /// row where half the buttons are always wrong, and the same is true of a
    /// loop.
    #[test]
    fn a_latched_control_offers_the_other_half() {
        let out = DeckSnapshot {
            playing: true,
            eq_low: 0.0,
            ..deck()
        };
        let control = at_hand(&out, true)
            .controls
            .into_iter()
            .find(|c| c.action.contains("eq_low"))
            .expect("the mixing rail has a bass control");
        assert_eq!(control.label, "bass in");
        assert_eq!(control.action, "deck 1 eq_low 1");
        assert!(control.on);

        let flat = DeckSnapshot { eq_low: 1.0, ..out };
        let control = at_hand(&flat, true)
            .controls
            .into_iter()
            .find(|c| c.action.contains("eq_low"))
            .expect("the mixing rail has a bass control");
        assert_eq!(control.label, "bass out");
        assert!(!control.on);
    }

    /// **The rail follows the hands, not the focus.**
    ///
    /// §41's `ui focus` fades after six seconds because attention is a moment;
    /// a rail has to keep answering for a whole set. So it follows the same
    /// order the states do, across decks instead of within one.
    #[test]
    fn the_rail_follows_whichever_deck_the_hands_are_on() {
        let playing = DeckSnapshot {
            number: 1,
            playing: true,
            ..deck()
        };
        let cued = DeckSnapshot {
            number: 2,
            playing: false,
            ..deck()
        };
        // Mid-set: one record out to the room, one being got ready. The hands
        // are on the one that is not playing.
        assert_eq!(busiest(&[playing.clone(), cued.clone()]), Some(2));

        // A hand on the playing deck's platter beats that.
        let scratching = DeckSnapshot {
            jog_touched: true,
            ..playing.clone()
        };
        assert_eq!(busiest(&[scratching, cued.clone()]), Some(1));

        // Both playing, neither touched: the lowest, so it does not flicker.
        let also_playing = DeckSnapshot {
            playing: true,
            ..cued.clone()
        };
        assert_eq!(busiest(&[playing.clone(), also_playing]), Some(1));

        // An empty deck is never the answer, and no decks at all is None.
        let empty = DeckSnapshot {
            number: 2,
            loaded: false,
            ..cued
        };
        assert_eq!(busiest(&[playing, empty.clone()]), Some(1));
        assert_eq!(busiest(&[]), None);
        assert_eq!(busiest(&[empty]), None);
    }

    /// An empty deck gets the preparing rail rather than nothing at all.
    #[test]
    fn a_deck_with_nothing_on_it_still_has_a_rail() {
        let empty = DeckSnapshot {
            loaded: false,
            ..deck()
        };
        assert_eq!(at_hand(&empty, false).doing, Doing::Preparing);
        assert!(at_hand(&empty, false).controls.len() >= FEWEST);
    }
}

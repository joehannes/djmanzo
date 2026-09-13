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

    /// The five controls this state reaches for, in the order a hand finds
    /// them.
    ///
    /// The table §74 asks for, written once. It used to be five arms of
    /// `Control::new` calls, which meant a control existed only as the label
    /// it happened to be built with -- and so `loop 4` and `loop off` were two
    /// different controls, and there was nothing for §8's *preferred controls*
    /// to name.
    #[must_use]
    pub const fn reaches(self) -> [Reach; 5] {
        match self {
            // §74's list: jog, scratch mode, brake, reverse, cue. The jog is
            // the platter itself and the scratch mode is a setting rather than
            // a move, so what is left is the four things a hand reaches for
            // *while* the other hand is on the record.
            Doing::Scratching => [
                Reach::Cue,
                Reach::Reverse,
                Reach::Censor,
                Reach::Slip,
                Reach::Play,
            ],
            // §74's list, minus the stem FX rack. Each stem is a mute rather
            // than a fader because a rail is pressed, not swept.
            Doing::Stems => [
                Reach::StemVocal,
                Reach::StemDrums,
                Reach::StemBass,
                Reach::StemOther,
                Reach::Loop,
            ],
            // The moves a mix is actually made of. The bass is one entry
            // rather than two, and what it says is what pressing it will do --
            // a row with "bass out" and "bass in" side by side is a row where
            // half the buttons are always wrong.
            Doing::Mixing => [
                Reach::Sync,
                Reach::Bass,
                Reach::Filter,
                Reach::Keylock,
                Reach::Loop,
            ],
            // §74's list, minus the three that belong to a record rather than
            // a deck -- tags, rating and transition points are the browser's
            // and the pair view's, and doing them here would be doing them
            // where a DJ cannot see which record they are changing.
            Doing::Preparing => [
                Reach::Cue,
                Reach::Mark,
                Reach::Loop,
                Reach::Sync,
                Reach::Keylock,
            ],
            Doing::Playing => [
                Reach::Cue,
                Reach::Loop,
                Reach::Sync,
                Reach::Bass,
                Reach::Slip,
            ],
        }
    }
}

/// One control a DJ can reach for.
///
/// The *identity* of a control, which is not what it currently says. `bass in`
/// and `bass out` are one control showing two faces, and until this existed
/// they were two strings built in two arms -- so §8's *preferred controls* had
/// nothing stable it could name, and neither did anything else.
///
/// Every entry the rail can hold is listed here once, and each [`Doing`] names
/// five of them. That is what stops the mixing row and the playing row from
/// spelling the same control two ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Jump to the cue point.
    Cue,
    /// Play, or stop.
    Play,
    /// Start a four-beat loop, or leave the one that is running.
    Loop,
    /// Match this record's tempo and phase to the other one.
    Sync,
    /// Pull the low band out, or put it back.
    Bass,
    /// Sweep the filter, or return it to the middle.
    Filter,
    /// Hold the pitch where it is while the tempo moves.
    Keylock,
    /// Keep the record running underneath while you play over it.
    Slip,
    /// Run the record backwards.
    Reverse,
    /// Reverse while held, and drop back where it would have been.
    Censor,
    /// Set the first hot cue where the record is now.
    Mark,
    /// Mute the vocal.
    StemVocal,
    /// Mute the drums.
    StemDrums,
    /// Mute the bass line.
    StemBass,
    /// Mute everything the other three are not.
    StemOther,
}

impl Reach {
    /// Every control the rail can hold.
    pub const ALL: [Self; 15] = [
        Self::Cue,
        Self::Play,
        Self::Loop,
        Self::Sync,
        Self::Bass,
        Self::Filter,
        Self::Keylock,
        Self::Slip,
        Self::Reverse,
        Self::Censor,
        Self::Mark,
        Self::StemVocal,
        Self::StemDrums,
        Self::StemBass,
        Self::StemOther,
    ];

    /// The slug stored in `controls.json` and sent over the wire.
    ///
    /// Not the label: a label says what pressing it will do *now* and changes
    /// under the DJ, which is exactly what a stored preference must not do.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cue => "cue",
            Self::Play => "play",
            Self::Loop => "loop",
            Self::Sync => "sync",
            Self::Bass => "bass",
            Self::Filter => "filter",
            Self::Keylock => "keylock",
            Self::Slip => "slip",
            Self::Reverse => "reverse",
            Self::Censor => "censor",
            Self::Mark => "mark",
            Self::StemVocal => "stem-vocal",
            Self::StemDrums => "stem-drums",
            Self::StemBass => "stem-bass",
            Self::StemOther => "stem-other",
        }
    }

    /// What it does, for the picker that offers it.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Cue => "Jump to the cue point",
            Self::Play => "Play, or stop",
            Self::Loop => "Loop four beats, or leave the loop",
            Self::Sync => "Match tempo and phase to the other record",
            Self::Bass => "Pull the low band out, or put it back",
            Self::Filter => "Sweep the filter, or return it to the middle",
            Self::Keylock => "Hold the pitch while the tempo moves",
            Self::Slip => "Keep the record running underneath",
            Self::Reverse => "Run the record backwards",
            Self::Censor => "Reverse while held, then drop back in place",
            Self::Mark => "Set the first hot cue where you are",
            Self::StemVocal => "Mute the vocal",
            Self::StemDrums => "Mute the drums",
            Self::StemBass => "Mute the bass line",
            Self::StemOther => "Mute everything else",
        }
    }

    /// The reach a stored slug means, or `None` for one this build lost.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reach| reach.name() == name)
    }

    /// This control, as it stands on this deck right now.
    ///
    /// The label and the `on` light are both read from the snapshot, so a
    /// control cannot say one thing and do another: `bass in` is offered
    /// exactly when pressing it would put the bass back.
    #[must_use]
    pub fn build(self, deck: &DeckSnapshot) -> Control {
        let n = deck.number;
        let c = |label: &'static str, action: String, on: bool| Control {
            reach: self,
            label: label.to_owned(),
            action,
            on,
            kept: false,
        };
        match self {
            Self::Cue => c("cue", format!("deck {n} cue"), false),
            Self::Play => c("play", format!("deck {n} play_pause"), deck.playing),
            // One loop control whose label says what pressing it does.
            Self::Loop => match deck.active_loop {
                Some(_) => c("loop off", format!("deck {n} loop_off"), true),
                None => c("loop 4", format!("deck {n} loop 4"), false),
            },
            Self::Sync => c("sync", format!("deck {n} sync"), deck.synced),
            // And one for the low band, for the same reason.
            Self::Bass => {
                if deck.eq_low <= BAND_IS_OUT {
                    c("bass in", format!("deck {n} eq_low 1"), true)
                } else {
                    c("bass out", format!("deck {n} eq_low 0"), false)
                }
            }
            Self::Filter => c(
                "filter",
                format!(
                    "deck {n} filter {}",
                    if deck.filter.abs() > 0.01 { 0.0 } else { -0.6 }
                ),
                deck.filter.abs() > 0.01,
            ),
            Self::Keylock => c("keylock", format!("deck {n} keylock_toggle"), deck.keylock),
            Self::Slip => c("slip", format!("deck {n} slip_toggle"), deck.slip),
            Self::Reverse => c("reverse", format!("deck {n} reverse_toggle"), deck.reversed),
            Self::Censor => c("censor", format!("deck {n} censor_on"), false),
            Self::Mark => c("mark", format!("deck {n} hotcue_set 1"), false),
            Self::StemVocal => c(
                "vocal",
                format!("deck {n} stem_mute vocal"),
                deck.stem_mutes[0],
            ),
            Self::StemDrums => c(
                "drums",
                format!("deck {n} stem_mute drums"),
                deck.stem_mutes[1],
            ),
            Self::StemBass => c(
                "bass",
                format!("deck {n} stem_mute bass"),
                deck.stem_mutes[2],
            ),
            Self::StemOther => c(
                "other",
                format!("deck {n} stem_mute other"),
                deck.stem_mutes[3],
            ),
        }
    }
}

/// The controls a DJ has asked to keep, as the rail will actually read them.
///
/// The same round trip §20's columns make: a slug this build does not have is
/// dropped, a repeat is collapsed, and more than a rail can hold is cut -- so
/// what is stored and what is drawn cannot drift apart. Unlike the columns
/// there is no floor. Keeping nothing is the ordinary case and means djmanzo
/// judges the whole rail, which is what §74 describes.
#[must_use]
pub fn keeping(asked: &[String]) -> Vec<Reach> {
    let mut kept: Vec<Reach> = Vec::new();
    for reach in asked.iter().filter_map(|name| Reach::from_name(name)) {
        if !kept.contains(&reach) {
            kept.push(reach);
        }
    }
    kept.truncate(MOST);
    kept
}

/// One control on the rail.
#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    /// Which control this is, whatever it happens to say at the moment.
    pub reach: Reach,
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
    /// True when it is here because the DJ asked for it rather than because
    /// djmanzo judged it relevant.
    pub kept: bool,
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
pub fn at_hand(deck: &DeckSnapshot, against: bool, kept: &[Reach]) -> AtHand {
    let doing = doing(deck, against);
    // §8's *preferred controls*, first and whatever the deck is doing. A DJ
    // who has said they want keylock within reach has said it about the whole
    // night, not about the state djmanzo happens to read -- so a kept control
    // displaces a judged one rather than waiting its turn, and a DJ who keeps
    // eight has a rail that no longer moves. That is the point of asking.
    let mut controls: Vec<Control> = Vec::new();
    for reach in kept {
        if !controls.iter().any(|c| c.reach == *reach) {
            let mut control = reach.build(deck);
            control.kept = true;
            controls.push(control);
        }
    }
    for reach in doing.reaches() {
        if !controls.iter().any(|c| c.reach == reach) {
            controls.push(reach.build(deck));
        }
    }
    controls.truncate(MOST);
    AtHand {
        deck: deck.number,
        doing,
        because: doing.because(),
        controls,
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
        every_state_keeping(&[])
    }

    fn every_state_keeping(kept: &[Reach]) -> Vec<AtHand> {
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
            at_hand(&scratching, false, kept),
            at_hand(&stems, false, kept),
            at_hand(&playing, true, kept),
            at_hand(&deck(), false, kept),
            at_hand(&playing, false, kept),
            at_hand(&empty, false, kept),
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
        assert_eq!(at_hand(&busy, true, &[]).doing, Doing::Scratching);

        // Take the hand off and the stems are what is left.
        let stems = DeckSnapshot {
            jog_touched: false,
            ..busy.clone()
        };
        assert_eq!(at_hand(&stems, true, &[]).doing, Doing::Stems);

        // Un-mute it and it is a mix.
        let mixing = DeckSnapshot {
            stem_mutes: [false; 4],
            ..stems.clone()
        };
        assert_eq!(at_hand(&mixing, true, &[]).doing, Doing::Mixing);

        // And with nothing else audible it is simply playing.
        assert_eq!(at_hand(&mixing, false, &[]).doing, Doing::Playing);
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
        assert_eq!(at_hand(&shaped, false, &[]).doing, Doing::Playing);

        // Pulling one stem down *is* playing them.
        let played = DeckSnapshot {
            stem_volumes: [0.0, 1.0, 1.0, 1.0],
            ..shaped.clone()
        };
        assert_eq!(at_hand(&played, false, &[]).doing, Doing::Stems);

        // And four stems the engine has not published yet — all zero, which is
        // every deck in the first second after launch — are not four stems
        // somebody pulled. Without this every deck opens on the stem row.
        let fresh = DeckSnapshot {
            stem_volumes: [0.0; 4],
            ..shaped
        };
        assert_eq!(at_hand(&fresh, false, &[]).doing, Doing::Playing);
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
        let control = at_hand(&out, true, &[])
            .controls
            .into_iter()
            .find(|c| c.action.contains("eq_low"))
            .expect("the mixing rail has a bass control");
        assert_eq!(control.label, "bass in");
        assert_eq!(control.action, "deck 1 eq_low 1");
        assert!(control.on);

        let flat = DeckSnapshot { eq_low: 1.0, ..out };
        let control = at_hand(&flat, true, &[])
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
        assert_eq!(at_hand(&empty, false, &[]).doing, Doing::Preparing);
        assert!(at_hand(&empty, false, &[]).controls.len() >= FEWEST);
    }

    /// **A control you keep is there whatever the deck is doing.**
    ///
    /// §8 Level 1's *preferred controls*, and the whole of it: a DJ who said
    /// they want keylock within reach said it about the night, not about the
    /// one state djmanzo happens to be reading. A rail that honoured the
    /// preference in four states out of six would be worse than one that
    /// ignored it, because the DJ would reach for it and find something else
    /// under the finger.
    #[test]
    fn a_control_you_keep_is_on_the_rail_in_every_state() {
        let kept = keeping(&["keylock".to_owned()]);
        for hand in every_state_keeping(&kept) {
            let found = hand
                .controls
                .iter()
                .find(|c| c.reach == Reach::Keylock)
                .unwrap_or_else(|| panic!("{} dropped a kept control", hand.doing.slug()));
            assert!(
                found.kept,
                "{} offers keylock without saying the DJ asked for it",
                hand.doing.slug()
            );
            assert_eq!(found.action, "deck 1 keylock_toggle");
        }
    }

    /// **A kept control is not a second copy of one already there.**
    ///
    /// Mixing already offers keylock. Keeping it must mark that entry rather
    /// than add another beside it — two identical buttons in a row reached for
    /// without looking is worse than either of them alone.
    #[test]
    fn keeping_something_the_rail_already_offers_does_not_double_it() {
        let mixing = DeckSnapshot {
            playing: true,
            ..deck()
        };
        let hand = at_hand(&mixing, true, &keeping(&["keylock".to_owned()]));
        assert_eq!(hand.doing, Doing::Mixing);
        let keylocks: Vec<&Control> = hand
            .controls
            .iter()
            .filter(|c| c.reach == Reach::Keylock)
            .collect();
        assert_eq!(keylocks.len(), 1, "{:?}", hand.controls);
        assert!(keylocks[0].kept);
    }

    /// **Kept controls displace judged ones rather than overflowing the rail.**
    ///
    /// §74's ceiling is eight and it is not negotiable: the rail is reached for
    /// without looking, and a row that grew to thirteen is a row where nothing
    /// is where it was. A DJ who keeps eight has said they want a rail that
    /// does not move, which is a thing they are allowed to want.
    #[test]
    fn keeping_more_than_the_rail_holds_still_holds_eight() {
        let asked: Vec<String> = Reach::ALL.iter().map(|r| r.name().to_owned()).collect();
        let kept = keeping(&asked);
        assert_eq!(kept.len(), MOST);
        for hand in every_state_keeping(&kept) {
            assert_eq!(hand.controls.len(), MOST, "{}", hand.doing.slug());
            assert!(
                hand.controls.iter().all(|c| c.kept),
                "{} let a judged control onto a rail the DJ filled",
                hand.doing.slug()
            );
        }
    }

    /// **A slug this build has never heard of costs the preference, not the
    /// rail.**
    ///
    /// The same rule §20's columns follow: a `controls.json` written by a later
    /// djmanzo should leave a DJ with the controls this one *does* have, rather
    /// than with an error where the rail was.
    #[test]
    fn a_control_this_build_does_not_have_is_dropped_and_the_rest_kept() {
        let asked = vec![
            "spinback".to_owned(),
            "sync".to_owned(),
            "sync".to_owned(),
            "".to_owned(),
        ];
        assert_eq!(keeping(&asked), vec![Reach::Sync]);
    }

    /// **Every control the picker can offer is one the rail can build.**
    ///
    /// The reaches are what a DJ ticks and what `controls.json` stores, so one
    /// that produced an action the parser refuses would be a stored preference
    /// for a button that does nothing.
    #[test]
    fn every_control_a_dj_can_keep_is_a_real_action() {
        for reach in Reach::ALL {
            let control = reach.build(&deck());
            assert!(
                Action::parse(&control.action).is_ok(),
                "{} offers {:?}, which the parser refuses",
                reach.name(),
                control.action
            );
            assert!(!control.label.is_empty(), "{} has no label", reach.name());
            assert!(!reach.about().is_empty());
            assert_eq!(Reach::from_name(reach.name()), Some(reach));
        }
    }

    /// **Every state's five are five different controls.**
    ///
    /// A [`Doing`] naming the same reach twice would draw four buttons where
    /// §74 asks for five, and the duplicate would be invisible: both entries
    /// build the same label.
    #[test]
    fn no_state_reaches_for_the_same_control_twice() {
        for doing in [
            Doing::Scratching,
            Doing::Stems,
            Doing::Mixing,
            Doing::Preparing,
            Doing::Playing,
        ] {
            let mut reaches = doing.reaches().to_vec();
            reaches.sort_unstable_by_key(|r| r.name());
            reaches.dedup();
            assert_eq!(reaches.len(), 5, "{} repeats a control", doing.slug());
        }
    }
}

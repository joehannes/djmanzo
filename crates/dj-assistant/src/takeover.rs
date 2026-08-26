//! Who is holding the controls.
//!
//! The single most important interaction in the whole assistant feature, and
//! the one that decides whether a DJ ever turns it on twice.
//!
//! # Touching a control takes it
//!
//! Not a button labelled "manual" — the fader, the jog, the EQ itself. If a
//! hand arrives on a control the assistant is moving, the assistant steps off
//! **that control** immediately and says nothing about it. Anything else is a
//! fight over a fader in front of an audience, and there is no version of that
//! which ends well.
//!
//! Per control, not globally. A DJ reaching for the bass on deck one has not
//! asked the assistant to stop keeping deck two in sync, and taking the whole
//! thing away would punish them for touching anything.
//!
//! # Handing back is deliberate
//!
//! The reverse is *not* symmetric, and that asymmetry is the point. Taking over
//! is instant and implicit because a hand on a fader is unambiguous. Handing
//! back is explicit, because "I have stopped touching this" is not a decision —
//! a DJ who lets go of the crossfader to pick up a drink has not asked for the
//! machine to resume.
//!
//! # It expires
//!
//! A held control is released after [`FORGET_AFTER`] of no contact, so a DJ who
//! nudged something an hour ago is not still holding it. The timeout is long
//! enough that it never fires mid-mix and short enough that it does not
//! outlive a set.

use dj_core::{DeckId, ParamId};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a control stays held after the last touch.
///
/// Ten minutes. Long enough that it cannot expire during a transition — the
/// longest transition djmanzo will plan is 64 beats, under two minutes at any
/// danceable tempo — and short enough that a nudge at the start of a set is not
/// still being honoured at the end of it.
pub const FORGET_AFTER: Duration = Duration::from_secs(600);

/// What is holding a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder {
    /// Nobody has touched it; the assistant may move it.
    Free,
    /// A hand arrived. The assistant does not touch this until it is handed
    /// back or it expires.
    Human,
}

/// Which controls the human has taken.
///
/// Not `Send`-hostile and not on the audio path: this is consulted by whatever
/// is deciding to emit an action, which is never the audio thread.
#[derive(Debug, Default)]
pub struct Takeover {
    held: HashMap<ParamId, Instant>,
    /// Set when the human has taken the whole session rather than one control.
    all: Option<Instant>,
}

impl Takeover {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A hand arrived on a control.
    ///
    /// Idempotent, and cheap enough to call on every parameter change from the
    /// interface — which is how it will be called, because the alternative is
    /// asking the interface to decide what counts as a touch and it does not
    /// know.
    pub fn touched(&mut self, param: ParamId) {
        self.touched_at(param, Instant::now());
    }

    /// The same, at a stated moment. For tests, and for replaying a log.
    pub fn touched_at(&mut self, param: ParamId, when: Instant) {
        self.held.insert(param, when);
    }

    /// The human has taken everything — the panic button.
    pub fn take_all(&mut self) {
        self.all = Some(Instant::now());
    }

    /// Hand one control back.
    pub fn release(&mut self, param: ParamId) {
        self.held.remove(&param);
    }

    /// Hand everything back.
    ///
    /// The resume gesture. One call, whatever was held and however it was
    /// taken, because a DJ resuming should not have to remember what they
    /// touched.
    pub fn release_all(&mut self) {
        self.held.clear();
        self.all = None;
    }

    /// Who is holding a control, as of now.
    #[must_use]
    pub fn holder(&self, param: ParamId) -> Holder {
        self.holder_at(param, Instant::now())
    }

    /// The same, at a stated moment.
    #[must_use]
    pub fn holder_at(&self, param: ParamId, now: Instant) -> Holder {
        if let Some(taken) = self.all
            && now.duration_since(taken) < FORGET_AFTER
        {
            return Holder::Human;
        }
        match self.held.get(&param) {
            Some(&when) if now.duration_since(when) < FORGET_AFTER => Holder::Human,
            _ => Holder::Free,
        }
    }

    /// Whether the assistant may move this control.
    #[must_use]
    pub fn may_move(&self, param: ParamId) -> bool {
        self.holder(param) == Holder::Free
    }

    /// Whether anything at all is held.
    ///
    /// What the interface reads to decide whether to show the resume control.
    /// Offering "resume" when nothing was taken is offering to undo nothing.
    #[must_use]
    pub fn anything_held(&self) -> bool {
        self.anything_held_at(Instant::now())
    }

    #[must_use]
    pub fn anything_held_at(&self, now: Instant) -> bool {
        if let Some(taken) = self.all
            && now.duration_since(taken) < FORGET_AFTER
        {
            return true;
        }
        self.held
            .values()
            .any(|&when| now.duration_since(when) < FORGET_AFTER)
    }

    /// Forget everything expired.
    ///
    /// Not required for correctness -- [`holder_at`](Self::holder_at) already
    /// ignores stale entries -- but a four-hour set touches a lot of controls
    /// and the map should not grow for the whole of it.
    pub fn sweep(&mut self, now: Instant) {
        self.held
            .retain(|_, when| now.duration_since(*when) < FORGET_AFTER);
        if let Some(taken) = self.all
            && now.duration_since(taken) >= FORGET_AFTER
        {
            self.all = None;
        }
    }

    /// Every deck with at least one control held.
    ///
    /// For an interface marking which decks the human has claimed, so a glance
    /// answers "what am I holding" without reading a list of parameter names.
    #[must_use]
    pub fn decks_held(&self) -> Vec<DeckId> {
        let now = Instant::now();
        let mut decks: Vec<DeckId> = self
            .held
            .iter()
            .filter(|(_, when)| now.duration_since(**when) < FORGET_AFTER)
            .filter_map(|(param, _)| match param {
                ParamId::Deck(deck, _) => Some(*deck),
                ParamId::Global(_) => None,
            })
            .collect();
        decks.sort_by_key(|d: &DeckId| d.human_number());
        decks.dedup();
        decks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::param::{DeckParam, GlobalParam};

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn eq_low(n: u8) -> ParamId {
        ParamId::Deck(deck(n), DeckParam::EqLow)
    }

    fn volume(n: u8) -> ParamId {
        ParamId::Deck(deck(n), DeckParam::Volume)
    }

    /// **Touching a control takes it, immediately.**
    ///
    /// No mode, no button, no confirmation. A hand on a fader is unambiguous
    /// and anything that made the DJ ask twice would be a fight over a control
    /// in front of an audience.
    #[test]
    fn touching_a_control_takes_it() {
        let mut takeover = Takeover::new();
        assert!(takeover.may_move(eq_low(1)));

        takeover.touched(eq_low(1));
        assert_eq!(takeover.holder(eq_low(1)), Holder::Human);
        assert!(!takeover.may_move(eq_low(1)));
    }

    /// **Taking one control does not take the rest.**
    ///
    /// A DJ reaching for the bass on deck one has not asked the assistant to
    /// stop keeping deck two in sync. Taking everything would punish them for
    /// touching anything, and the result is a DJ who stops touching things --
    /// which is the opposite of what the feature is for.
    #[test]
    fn taking_one_control_leaves_the_others() {
        let mut takeover = Takeover::new();
        takeover.touched(eq_low(1));

        assert!(!takeover.may_move(eq_low(1)));
        assert!(takeover.may_move(volume(1)), "the whole deck was taken");
        assert!(takeover.may_move(eq_low(2)), "the other deck was taken");
    }

    /// **The panic button takes everything.**
    ///
    /// The one case where taking the lot is right: a DJ who wants the machine
    /// off *now* should not have to touch eight controls to get it.
    #[test]
    fn taking_all_holds_every_control() {
        let mut takeover = Takeover::new();
        takeover.take_all();

        assert!(!takeover.may_move(eq_low(1)));
        assert!(!takeover.may_move(volume(4)));
        assert!(!takeover.may_move(ParamId::Global(GlobalParam::MasterGainDb)));
    }

    /// **Handing back is one gesture, whatever was taken.**
    ///
    /// A DJ resuming should not have to remember what they touched. This is
    /// the asymmetry the module exists for: taking is implicit and per
    /// control, giving back is explicit and total.
    #[test]
    fn one_gesture_hands_everything_back() {
        let mut takeover = Takeover::new();
        takeover.touched(eq_low(1));
        takeover.touched(volume(2));
        takeover.take_all();

        takeover.release_all();

        assert!(takeover.may_move(eq_low(1)));
        assert!(takeover.may_move(volume(2)));
        assert!(!takeover.anything_held());
    }

    /// **Letting go is not handing back.**
    ///
    /// "I have stopped touching this" is not a decision. A DJ who releases the
    /// crossfader to pick up a drink has not asked the machine to resume, and
    /// one that did would be genuinely dangerous.
    #[test]
    fn releasing_the_control_does_not_resume_by_itself() {
        let mut takeover = Takeover::new();
        takeover.touched(volume(1));
        // Time passes, but not enough to expire, and no explicit hand-back.
        let later = Instant::now() + Duration::from_secs(30);
        assert_eq!(
            takeover.holder_at(volume(1), later),
            Holder::Human,
            "the assistant resumed a control the DJ had not given back"
        );
    }

    /// **A control held an hour ago is not still held.**
    ///
    /// A nudge at the start of a set should not silently disable the assistant
    /// for the rest of the night.
    #[test]
    fn a_forgotten_control_expires() {
        let mut takeover = Takeover::new();
        let start = Instant::now();
        takeover.touched_at(eq_low(1), start);

        let much_later = start + FORGET_AFTER + Duration::from_secs(1);
        assert_eq!(takeover.holder_at(eq_low(1), much_later), Holder::Free);
        assert!(!takeover.anything_held_at(much_later));
    }

    /// **And it does not expire during a mix.**
    ///
    /// The longest transition djmanzo will plan is 64 beats -- under two
    /// minutes at any danceable tempo. A timeout that could fire inside one
    /// would hand a control back mid-blend, which is the worst possible moment.
    #[test]
    fn the_timeout_cannot_fire_during_a_transition() {
        // 64 beats at 60 BPM, the slowest tempo anyone would mix at.
        let longest_transition = Duration::from_secs(64);
        assert!(
            FORGET_AFTER > longest_transition * 4,
            "a held control could expire in the middle of a mix"
        );
    }

    /// One control can be handed back on its own, for a DJ who took the EQ to
    /// fix something and wants only that back.
    #[test]
    fn a_single_control_can_be_handed_back() {
        let mut takeover = Takeover::new();
        takeover.touched(eq_low(1));
        takeover.touched(volume(1));

        takeover.release(eq_low(1));
        assert!(takeover.may_move(eq_low(1)));
        assert!(!takeover.may_move(volume(1)), "the wrong control was freed");
    }

    /// **The interface can see which decks are claimed.**
    ///
    /// So a glance answers "what am I holding" rather than a list of parameter
    /// names nobody reads mid-set.
    #[test]
    fn the_decks_being_held_can_be_listed() {
        let mut takeover = Takeover::new();
        takeover.touched(eq_low(3));
        takeover.touched(volume(1));
        takeover.touched(eq_low(1));

        assert_eq!(takeover.decks_held(), vec![deck(1), deck(3)]);
    }

    /// A global control is held without claiming a deck.
    #[test]
    fn a_global_control_belongs_to_no_deck() {
        let mut takeover = Takeover::new();
        takeover.touched(ParamId::Global(GlobalParam::MasterGainDb));
        assert!(takeover.anything_held());
        assert!(takeover.decks_held().is_empty());
    }

    /// Sweeping drops what has expired, so a four-hour set does not grow a map
    /// of every control ever touched.
    #[test]
    fn sweeping_forgets_what_expired() {
        let mut takeover = Takeover::new();
        let start = Instant::now();
        takeover.touched_at(eq_low(1), start);
        takeover.touched_at(volume(2), start + FORGET_AFTER);

        takeover.sweep(start + FORGET_AFTER + Duration::from_secs(1));
        assert!(takeover.may_move(eq_low(1)));
        assert!(!takeover.may_move(volume(2)), "a live hold was swept away");
    }
}

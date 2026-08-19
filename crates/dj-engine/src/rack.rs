//! Three effect slots and the order they run in.
//!
//! One rack per deck and one on the master, so a DJ can put an echo on a deck
//! and a gate over the whole mix without either knowing about the other.
//!
//! The rack is a *chain*, not a parallel bank: slot 1 feeds slot 2 feeds
//! slot 3. That is what makes stacking mean anything — a gate after an echo
//! chops the repeats, and a gate before it feeds the echo chopped audio, and a
//! DJ can hear the difference. A parallel bank would just be three effects
//! played at once.

use dj_core::fx::{EffectKind, FX_SLOTS, FxChange, Placement};
use dj_dsp::fx::{FxContext, Slot};

/// The three slots on one deck, or on the master.
#[derive(Debug)]
pub struct Rack {
    slots: [Slot; FX_SLOTS],
}

impl Rack {
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            slots: std::array::from_fn(|_| Slot::new(sample_rate)),
        }
    }

    /// One slot, by its 1-based number as the interface and controllers use it.
    #[must_use]
    pub fn slot(&self, number: u8) -> Option<&Slot> {
        self.slots.get(usize::from(number.checked_sub(1)?))
    }

    fn slot_mut(&mut self, number: u8) -> Option<&mut Slot> {
        self.slots.get_mut(usize::from(number.checked_sub(1)?))
    }

    /// Whether anything at all is happening.
    ///
    /// Lets the caller skip the whole rack — and, on a deck, skip splitting the
    /// signal into a pre- and post-fader path at all. Three racks of nothing is
    /// the normal state of a mixer and it should cost one comparison.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.slots
            .iter()
            .all(|slot| !slot.is_enabled() || slot.kind() == EffectKind::None)
    }

    /// Apply one change from the action bus.
    ///
    /// Returns false for a slot number the rack does not have. The parser
    /// already rejects those, so this is the second line of defence rather than
    /// the first — but an engine that indexes an array from a network message
    /// wants both.
    pub fn apply(&mut self, number: u8, change: FxChange) -> bool {
        let Some(slot) = self.slot_mut(number) else {
            return false;
        };
        match change {
            FxChange::Select(kind) => slot.select(kind),
            FxChange::SetEnabled(on) => slot.set_enabled(on),
            FxChange::ToggleEnabled => {
                let on = slot.is_enabled();
                slot.set_enabled(!on);
            }
            FxChange::Wet(wet) => slot.set_wet(wet),
            FxChange::Beats(beats) => slot.set_beats(beats),
            FxChange::Amount(amount) => slot.set_amount(amount),
            FxChange::Place(placement) => slot.set_placement(placement),
        }
        true
    }

    /// Run the slots placed before the fader.
    #[inline]
    #[must_use]
    pub fn process_pre(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        self.process(left, right, ctx, Placement::PreFader)
    }

    /// Run the slots placed after it.
    #[inline]
    #[must_use]
    pub fn process_post(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        self.process(left, right, ctx, Placement::PostFader)
    }

    #[inline]
    fn process(
        &mut self,
        left: f32,
        right: f32,
        ctx: &FxContext,
        placement: Placement,
    ) -> (f32, f32) {
        let mut frame = (left, right);
        for slot in &mut self.slots {
            if slot.placement() == placement {
                frame = slot.process_frame(frame.0, frame.1, ctx);
            }
        }
        frame
    }

    /// Forget every tail. For a device change or a track eject, where carrying
    /// a second of the previous situation forward would be a glitch rather than
    /// an effect.
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn ctx() -> FxContext {
        FxContext {
            sample_rate: SR,
            beat_frames: Some(SR / 2.0),
        }
    }

    fn settle(rack: &mut Rack) {
        for _ in 0..(SR as usize) {
            let _ = rack.process_pre(0.0, 0.0, &ctx());
            let _ = rack.process_post(0.0, 0.0, &ctx());
        }
    }

    #[test]
    fn a_fresh_rack_is_idle_and_passes_audio_through() {
        let mut rack = Rack::new(SR);
        assert!(rack.is_idle());
        assert_eq!(rack.process_pre(0.5, -0.5, &ctx()), (0.5, -0.5));
        assert_eq!(rack.process_post(0.5, -0.5, &ctx()), (0.5, -0.5));
    }

    #[test]
    fn loading_an_effect_and_switching_it_on_makes_the_rack_busy() {
        let mut rack = Rack::new(SR);
        assert!(rack.apply(1, FxChange::Select(EffectKind::Crush)));
        assert!(rack.is_idle(), "loaded but not switched on");
        assert!(rack.apply(1, FxChange::SetEnabled(true)));
        assert!(!rack.is_idle());
    }

    /// A slot that is on but empty is still idle: there is nothing to run.
    #[test]
    fn an_empty_slot_that_is_switched_on_is_still_idle() {
        let mut rack = Rack::new(SR);
        rack.apply(1, FxChange::SetEnabled(true));
        assert!(rack.is_idle());
    }

    #[test]
    fn a_slot_the_rack_does_not_have_is_refused() {
        let mut rack = Rack::new(SR);
        assert!(!rack.apply(0, FxChange::ToggleEnabled));
        assert!(!rack.apply(4, FxChange::ToggleEnabled));
        assert!(rack.slot(0).is_none());
        assert!(rack.slot(4).is_none());
        assert!(rack.slot(1).is_some());
        assert!(rack.slot(FX_SLOTS as u8).is_some());
    }

    /// Placement is what decides which pass a slot runs in, and a slot must run
    /// in exactly one of them — running in both would apply it twice.
    #[test]
    fn a_slot_runs_in_its_own_pass_and_not_the_other() {
        let mut rack = Rack::new(SR);
        rack.apply(1, FxChange::Select(EffectKind::Crush));
        rack.apply(1, FxChange::SetEnabled(true));
        rack.apply(1, FxChange::Wet(1.0));
        rack.apply(1, FxChange::Amount(1.0));
        rack.apply(1, FxChange::Place(Placement::PostFader));
        settle(&mut rack);

        // Pre-fader is untouched, because the only slot is placed after it.
        assert_eq!(rack.process_pre(0.3, 0.3, &ctx()), (0.3, 0.3));
        let (post, _) = rack.process_post(0.3, 0.3, &ctx());
        assert_ne!(post, 0.3, "the post-fader pass should have crushed it");
    }

    /// The chain is what makes stacking mean anything, so the order has to be
    /// observable. Crush before an echo means the echo repeats already-coarse
    /// audio; crush after it means the sum of dry and repeats gets coarsened
    /// together. Those are different signals, and comparing the two sequences
    /// says so without anyone having to reason about which is louder.
    #[test]
    fn slots_run_in_order_so_stacking_changes_the_sound() {
        fn render(crush_slot: u8, echo_slot: u8) -> Vec<f32> {
            let mut rack = Rack::new(SR);
            rack.apply(echo_slot, FxChange::Select(EffectKind::Echo));
            rack.apply(echo_slot, FxChange::SetEnabled(true));
            rack.apply(echo_slot, FxChange::Wet(1.0));
            // A sixteenth of a beat, so several repeats fall inside the
            // window below. A longer echo would have produced no repeat at all
            // in the samples compared, and two orderings of an echo that never
            // repeats are identical — which is a test that passes for the
            // wrong reason, or in this case fails for a confusing one.
            rack.apply(echo_slot, FxChange::Beats(1.0 / 16.0));
            rack.apply(echo_slot, FxChange::Amount(0.5));

            rack.apply(crush_slot, FxChange::Select(EffectKind::Crush));
            rack.apply(crush_slot, FxChange::SetEnabled(true));
            rack.apply(crush_slot, FxChange::Wet(1.0));
            rack.apply(crush_slot, FxChange::Amount(1.0));
            settle(&mut rack);

            (0..12_000)
                .map(|n| {
                    let sample = (n as f32 * 0.01).sin();
                    rack.process_pre(sample, sample, &ctx()).0
                })
                .collect()
        }

        let crush_first = render(1, 2);
        let echo_first = render(2, 1);
        let difference: f32 = crush_first
            .iter()
            .zip(&echo_first)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            difference > 1.0,
            "swapping the order changed the output by only {difference}"
        );
    }

    #[test]
    fn resetting_forgets_every_tail() {
        let mut rack = Rack::new(SR);
        rack.apply(1, FxChange::Select(EffectKind::Echo));
        rack.apply(1, FxChange::SetEnabled(true));
        rack.apply(1, FxChange::Wet(1.0));
        rack.apply(1, FxChange::Beats(1.0));
        settle(&mut rack);
        for _ in 0..1_000 {
            let _ = rack.process_pre(1.0, 1.0, &ctx());
        }

        rack.reset();
        let mut peak = 0.0f32;
        for _ in 0..(SR as usize / 2) {
            let (left, _) = rack.process_pre(0.0, 0.0, &ctx());
            peak = peak.max(left.abs());
        }
        assert!(peak < 1e-6, "a tail survived the reset at {peak}");
    }

    #[test]
    fn toggling_flips_whatever_the_slot_was() {
        let mut rack = Rack::new(SR);
        rack.apply(2, FxChange::Select(EffectKind::Gate));
        rack.apply(2, FxChange::ToggleEnabled);
        assert!(rack.slot(2).unwrap().is_enabled());
        rack.apply(2, FxChange::ToggleEnabled);
        assert!(!rack.slot(2).unwrap().is_enabled());
    }
}

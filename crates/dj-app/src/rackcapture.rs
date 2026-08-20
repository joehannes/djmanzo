//! Turning the rack you have into a preset you can get back.
//!
//! # Why this is a capture rather than a new kind of preset
//!
//! A preset is already a list of action text ([`dj_presets::Preset`]), and the
//! effect rack is already reachable from that vocabulary — `deck 1 fx 2 wet
//! 0.4` is a sentence the bus understands. So an "FX chain preset" needs no new
//! machinery at all. What was missing is the other direction: **reading the
//! rack back out**, so a DJ who has just dialled something in can keep it
//! without transcribing six numbers by hand.
//!
//! That is all this module does. It reads the parameter registry and writes the
//! sentences that would reproduce it.
//!
//! # Why the order matters
//!
//! The effect is selected first and switched on last. Selecting an effect
//! resets the slot's buffers, so a wet level set before the selection would be
//! overwritten; and switching on before the settings have landed means a bar of
//! the *previous* settings goes to the room. Neither is visible in a test that
//! only checks the actions are all present, which is why one here checks the
//! order.

use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, GlobalParam};
use dj_core::{DeckId, EffectKind, FX_SLOTS, ParamId};

/// Which rack to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rack {
    /// One deck's three slots.
    Deck(DeckId),
    /// The master's three slots.
    Master,
}

/// Read a rack out of the registry as the action text that would restore it.
///
/// Empty when every slot is on `none`: a preset that sets three slots to
/// nothing is a preset that does nothing, and offering to save it invites a
/// library full of them.
#[must_use]
pub fn capture(registry: &ParameterRegistry, rack: Rack) -> Vec<String> {
    let mut actions = Vec::new();

    for slot in 1..=FX_SLOTS as u8 {
        let Some(slot_state) = read_slot(registry, rack, slot) else {
            continue;
        };
        let at = prefix(rack);

        let kind = EffectKind::from_index(slot_state.kind.max(0.0) as usize);
        if kind == EffectKind::None {
            // Written all the same: a preset that left slot 3 alone would
            // recall on top of whatever was already there, and "my chain" has
            // to mean the whole chain.
            actions.push(format!("{at} {slot} none"));
            continue;
        }

        actions.push(format!("{at} {slot} {}", kind.name()));
        actions.push(format!("{at} {slot} wet {}", trim(slot_state.wet)));
        if kind.is_timed() {
            // A beat length on an untimed effect is a number the parser accepts
            // and the engine ignores, which would put a lie in the file.
            actions.push(format!("{at} {slot} beats {}", trim(slot_state.beats)));
        }
        actions.push(format!("{at} {slot} amount {}", trim(slot_state.amount)));
        actions.push(format!(
            "{at} {slot} {}",
            if slot_state.post { "post" } else { "pre" }
        ));
        // Last, so nothing half-configured is ever audible.
        actions.push(format!(
            "{at} {slot} {}",
            if slot_state.enabled { "on" } else { "off" }
        ));
    }

    // A chain of three empty slots is a preset that does nothing, and offering
    // to save one invites a library full of them.
    if actions.iter().all(|line| line.ends_with(" none")) {
        return Vec::new();
    }
    actions
}

/// One slot, read out of the registry.
struct SlotState {
    kind: f32,
    enabled: bool,
    wet: f32,
    beats: f32,
    amount: f32,
    post: bool,
}

fn read_slot(registry: &ParameterRegistry, rack: Rack, slot: u8) -> Option<SlotState> {
    match rack {
        Rack::Deck(deck) => {
            let p = DeckParam::fx(slot)?;
            let get = |param| registry.get(ParamId::Deck(deck, param));
            Some(SlotState {
                kind: get(p.kind),
                enabled: get(p.enabled) >= 0.5,
                wet: get(p.wet),
                beats: get(p.beats),
                amount: get(p.amount),
                post: get(p.post) >= 0.5,
            })
        }
        Rack::Master => {
            let p = GlobalParam::fx(slot)?;
            let get = |param| registry.get(ParamId::Global(param));
            Some(SlotState {
                kind: get(p.kind),
                enabled: get(p.enabled) >= 0.5,
                wet: get(p.wet),
                beats: get(p.beats),
                amount: get(p.amount),
                post: get(p.post) >= 0.5,
            })
        }
    }
}

/// `deck {deck} fx` or `master fx` — the part every line of a rack shares.
///
/// `{deck}` rather than a number, because that is the placeholder
/// [`dj_presets::Preset`] substitutes when a preset is applied to a deck. A
/// captured chain is therefore usable on any deck, not only the one it came
/// from.
fn prefix(rack: Rack) -> &'static str {
    match rack {
        Rack::Deck(_) => "deck {deck} fx",
        Rack::Master => "master fx",
    }
}

/// A number short enough to read in a file.
///
/// Three places is finer than any control here resolves, and it keeps a
/// captured `0.4` from becoming `0.4000000059604645` — which is the same value
/// and a much worse thing to find in a preset somebody is trying to edit.
fn trim(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let text = format!("{rounded}");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{EffectKind, Placement};

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    /// How a slot is set, for the fixture below.
    #[derive(Clone, Copy)]
    struct Set {
        kind: EffectKind,
        wet: f32,
        beats: f32,
        amount: f32,
        post: bool,
        on: bool,
    }

    /// Set a deck slot up the way the engine would have.
    fn arrange(registry: &ParameterRegistry, slot: u8, s: Set) {
        let Set {
            kind,
            wet,
            beats,
            amount,
            post,
            on,
        } = s;
        let p = DeckParam::fx(slot).unwrap();
        let set = |param, value: f32| registry.set(ParamId::Deck(deck(1), param), value);
        set(p.kind, kind.index() as f32);
        set(p.wet, wet);
        set(p.beats, beats);
        set(p.amount, amount);
        set(p.post, if post { 1.0 } else { 0.0 });
        set(p.enabled, if on { 1.0 } else { 0.0 });
    }

    fn empty(registry: &ParameterRegistry, slot: u8) {
        arrange(
            registry,
            slot,
            Set {
                kind: EffectKind::None,
                wet: 0.0,
                beats: 0.0,
                amount: 0.0,
                post: false,
                on: false,
            },
        );
    }

    #[test]
    fn a_rack_of_nothing_is_not_worth_saving() {
        let registry = ParameterRegistry::new();
        for slot in 1..=FX_SLOTS as u8 {
            empty(&registry, slot);
        }
        assert!(capture(&registry, Rack::Deck(deck(1))).is_empty());
    }

    #[test]
    fn what_comes_out_is_what_the_bus_would_take_back() {
        let registry = ParameterRegistry::new();
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Echo,
                wet: 0.4,
                beats: 0.5,
                amount: 0.6,
                post: false,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        // Every line has to parse, with the placeholder filled in, or the
        // preset is a file of sentences nothing can read.
        for line in &actions {
            let text = line.replace("{deck}", "1");
            dj_core::Action::parse(&text)
                .unwrap_or_else(|e| panic!("{text:?} does not parse: {e}"));
        }
        assert!(actions.iter().any(|a| a.contains("fx 1 echo")));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 1 wet 0.4"));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 1 beats 0.5"));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 1 amount 0.6"));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 1 pre"));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 1 on"));
    }

    /// **The order is the whole thing.** Selecting an effect resets the slot's
    /// buffers, so a wet level written before the selection is thrown away; and
    /// switching on before the settings have landed sends a bar of the
    /// *previous* chain to the room. A test that only checked the lines were
    /// present would pass on either mistake.
    #[test]
    fn the_effect_is_chosen_first_and_switched_on_last() {
        let registry = ParameterRegistry::new();
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Reverb,
                wet: 0.3,
                beats: 1.0,
                amount: 0.7,
                post: true,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        let at = |needle: &str| {
            actions
                .iter()
                .position(|a| a.contains(needle))
                .unwrap_or_else(|| panic!("no line containing {needle:?}"))
        };
        assert!(at("reverb") < at("wet"), "wet was set before the effect");
        assert!(at("reverb") < at("amount"));
        assert!(
            at("wet") < at("fx 1 on"),
            "switched on before it was set up"
        );
        assert!(at("amount") < at("fx 1 on"));
        assert!(at("post") < at("fx 1 on"));
    }

    /// A beat length means nothing on an effect that is not timed. Writing one
    /// would put a number in the file that the engine ignores — a preset that
    /// says something untrue about itself.
    #[test]
    fn an_untimed_effect_carries_no_beat_length() {
        let registry = ParameterRegistry::new();
        assert!(!EffectKind::Crush.is_timed());
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Crush,
                wet: 0.5,
                beats: 2.0,
                amount: 0.5,
                post: false,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        assert!(
            !actions.iter().any(|a| a.contains("beats")),
            "a crush was given a beat length: {actions:?}"
        );
    }

    /// An empty slot is written rather than skipped, or recalling a two-effect
    /// chain would leave whatever was in slot 3 running underneath it.
    #[test]
    fn empty_slots_are_cleared_rather_than_left_alone() {
        let registry = ParameterRegistry::new();
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Filter,
                wet: 0.5,
                beats: 1.0,
                amount: 0.5,
                post: false,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 2 none"));
        assert!(actions.iter().any(|a| a == "deck {deck} fx 3 none"));
    }

    /// A captured chain is written with the placeholder, so it can be recalled
    /// onto any deck rather than only the one it came from.
    #[test]
    fn a_deck_chain_is_captured_for_any_deck() {
        let registry = ParameterRegistry::new();
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Gate,
                wet: 0.8,
                beats: 0.25,
                amount: 0.5,
                post: false,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        assert!(actions.iter().all(|a| a.starts_with("deck {deck} fx")));
        assert!(!actions.iter().any(|a| a.starts_with("deck 1")));
    }

    #[test]
    fn the_master_rack_is_captured_as_the_master() {
        let registry = ParameterRegistry::new();
        let p = GlobalParam::fx(1).unwrap();
        registry.set(ParamId::Global(p.kind), EffectKind::Delay.index() as f32);
        registry.set(ParamId::Global(p.wet), 0.25);
        registry.set(ParamId::Global(p.enabled), 1.0);

        let actions = capture(&registry, Rack::Master);
        assert!(actions.iter().all(|a| a.starts_with("master fx")));
        for line in &actions {
            dj_core::Action::parse(line).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        }
    }

    /// A number that came back out of an `f32` should not land in a file a
    /// person might open as `0.4000000059604645`.
    #[test]
    fn numbers_are_written_the_way_they_were_set() {
        let registry = ParameterRegistry::new();
        arrange(
            &registry,
            1,
            Set {
                kind: EffectKind::Echo,
                wet: 0.4,
                beats: 0.125,
                amount: 0.7,
                post: false,
                on: true,
            },
        );
        empty(&registry, 2);
        empty(&registry, 3);

        let actions = capture(&registry, Rack::Deck(deck(1)));
        assert!(
            actions.iter().any(|a| a.ends_with("wet 0.4")),
            "{actions:?}"
        );
        assert!(actions.iter().any(|a| a.ends_with("amount 0.7")));
        assert!(actions.iter().any(|a| a.ends_with("beats 0.125")));
    }

    /// Placement round-trips both ways: a post-fader slot recalled as pre-fader
    /// is an effect the fader can no longer cut, which is the whole point of
    /// the setting.
    #[test]
    fn placement_survives_the_trip() {
        for (post, expect) in [(true, "post"), (false, "pre")] {
            let registry = ParameterRegistry::new();
            arrange(
                &registry,
                1,
                Set {
                    kind: EffectKind::Echo,
                    wet: 0.5,
                    beats: 1.0,
                    amount: 0.5,
                    post,
                    on: true,
                },
            );
            empty(&registry, 2);
            empty(&registry, 3);
            let actions = capture(&registry, Rack::Deck(deck(1)));
            assert!(
                actions
                    .iter()
                    .any(|a| a == &format!("deck {{deck}} fx 1 {expect}")),
                "{actions:?}"
            );
        }
        let _ = Placement::PreFader;
    }
}

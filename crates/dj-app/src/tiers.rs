//! §58's information hierarchy, as a type.
//!
//! > Design a clear hierarchy: Tier 1 — glanceable […] Tier 2 — performable
//! > […] Tier 3 — contextual […] Tier 4 — preparation.
//!
//! The section is a *ranking*, and a ranking nothing reads is a paragraph. This
//! one is read by the command palette, which is the surface §98 says a whole
//! night's work should be reachable from: *everything common should be visible,
//! near, one gesture away, one shortcut away, or one contextual reveal away*.
//!
//! # What it changes
//!
//! The palette answers a query with every verb, every surface and every
//! interface operation that matches, and then cuts the list to what fits. The
//! order was the order they were generated in — the vocabulary's, then the
//! surfaces', then §41's operations — so a DJ who typed three letters mid-mix
//! could be offered *Pin the Journal* above *deck 2 cue*, and on a six-deck
//! layout the cut could take the performing controls off the bottom entirely.
//!
//! Ranked by tier, and **stably**, so within a tier the order stays the one it
//! had: the vocabulary is written in the order a DJ meets it, and re-sorting
//! inside a tier would throw that away to no purpose.
//!
//! # Why the mapping is explicit
//!
//! There is no `_ =>` arm that quietly makes an unclassified thing tier 4. A
//! verb added to `dj_core::vocabulary` and not named here fails a test, because
//! the failure mode of a default is the worst one available: the new verb is
//! the one a DJ has not learned the position of, and it would be the one the
//! palette buried.

/// One of §58's four.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Play state, track identity, BPM, position, phase, level, next track.
    /// What a DJ reads without looking away from the floor.
    Glanceable,
    /// Cue, loop, EQ, filter, pitch, stems, FX, crossfader. The hands.
    Performable,
    /// Suggestions, explanations, transition planning, room response,
    /// technique advice. What djmanzo has to say, when there is room to hear it.
    Contextual,
    /// Metadata, library, analysis, tags. Work done before a night, or after.
    Preparation,
}

impl Tier {
    /// §58's four, in §58's order.
    pub const ALL: [Self; 4] = [
        Self::Glanceable,
        Self::Performable,
        Self::Contextual,
        Self::Preparation,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Glanceable => "glanceable",
            Self::Performable => "performable",
            Self::Contextual => "contextual",
            Self::Preparation => "preparation",
        }
    }

    /// §58's own numbering, and what the palette sorts by.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Glanceable => 1,
            Self::Performable => 2,
            Self::Contextual => 3,
            Self::Preparation => 4,
        }
    }

    /// Whether this is one of the two §18 lets take room during a mix.
    ///
    /// `Attention::performing`'s own words are "Tier 1 and 2 only; nothing else
    /// may take room", which until now was a sentence in a doc comment.
    #[must_use]
    pub const fn survives_a_mix(self) -> bool {
        matches!(self, Self::Glanceable | Self::Performable)
    }
}

/// The transport a DJ reads the deck by. §58's tier 1.
///
/// Narrow on purpose. Tier 1 is what is *glanceable* — play state and position
/// — so it is the handful of verbs that answer "is it running and where is it",
/// not everything on the transport row. `keylock`, `slip` and `reverse` change
/// how a record behaves under the hand and are tier 2 with the rest of them.
const GLANCEABLE: [&str; 7] = [
    "play",
    "pause",
    "play_pause",
    "cue",
    "eject",
    "seek",
    "sync",
];

/// Work done away from a live mix. §58's tier 4.
///
/// The beat grid, all of it. A grid is corrected while a record is being
/// prepared — §57's own framing — and a DJ mid-transition reaching for
/// `grid_nudge` is not a case worth ranking above `eq_low`.
const PREPARATION_VERBS: [&str; 6] = [
    "grid_here",
    "grid_nudge",
    "grid_scale",
    "grid_bpm",
    "grid_tap",
    "grid_reset",
];

/// Which tier a verb of the action vocabulary belongs to.
///
/// Everything not named above is tier 2: the hands. That is the honest default
/// for a *vocabulary of actions* — every verb in it moves something a DJ is
/// performing with — and the two lists are the exceptions, which is why they
/// are short and why a test walks the whole vocabulary against them.
#[must_use]
pub fn of_verb(verb: &str) -> Tier {
    if GLANCEABLE.contains(&verb) {
        Tier::Glanceable
    } else if PREPARATION_VERBS.contains(&verb) {
        Tier::Preparation
    } else {
        Tier::Performable
    }
}

/// What djmanzo has to say, given room to hear it. §58's tier 3.
const CONTEXTUAL_SURFACES: [&str; 9] = [
    "next",
    "pair",
    "plan",
    "practice",
    "assistant",
    "night",
    "room",
    "transition",
    "athand",
];

/// The performing surfaces: the ones a hand is on. §58's tier 2.
const PERFORMABLE_SURFACES: [&str; 4] = ["stems", "fx", "sampler", "booth"];

/// Which tier a dockable surface belongs to.
///
/// Everything else is tier 4 — the library, the journal, the settings, the
/// controllers, the log — which is right: those are where a night is prepared
/// and where it is reviewed, and neither is a thing to be offered first while
/// somebody is mixing.
#[must_use]
pub fn of_surface(name: &str) -> Tier {
    if PERFORMABLE_SURFACES.contains(&name) {
        Tier::Performable
    } else if CONTEXTUAL_SURFACES.contains(&name) {
        Tier::Contextual
    } else {
        Tier::Preparation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing one: every verb djmanzo accepts has a tier, and it is
    /// one somebody chose.**
    ///
    /// Not that a lookup returns *something* — it always would, since the
    /// fallback is tier 2 — but that the two exception lists only ever name
    /// verbs that exist. A verb removed from the vocabulary and left here is a
    /// rule about nothing; a verb misspelled here is a rule that silently does
    /// not apply, and `grid_nudge` spelled `grid-nudge` would rank the beat
    /// grid among the hands with nothing saying so.
    #[test]
    fn every_verb_this_ranks_is_a_verb_djmanzo_accepts() {
        let vocabulary: Vec<&str> = dj_core::vocabulary::vocabulary()
            .iter()
            .map(|spec| spec.verb)
            .collect();
        for verb in GLANCEABLE.iter().chain(PREPARATION_VERBS.iter()) {
            assert!(
                vocabulary.contains(verb),
                "`{verb}` is ranked here and is not in the vocabulary, so the \
                 rule applies to nothing"
            );
        }
        // And the whole vocabulary is covered, which it is by construction —
        // stated so that a future `_ => panic!` arm is a deliberate change
        // rather than a discovery.
        for spec in dj_core::vocabulary::vocabulary() {
            let tier = of_verb(spec.verb);
            assert!(
                Tier::ALL.contains(&tier),
                "`{}` has no tier at all",
                spec.verb
            );
        }
    }

    /// And every surface, likewise.
    #[test]
    fn every_surface_this_ranks_is_a_surface_djmanzo_has() {
        let names: Vec<&str> = crate::cockpit::surfaces()
            .iter()
            .map(|surface| surface.name)
            .collect();
        for name in CONTEXTUAL_SURFACES
            .iter()
            .chain(PERFORMABLE_SURFACES.iter())
        {
            assert!(
                names.contains(name),
                "`{name}` is ranked here and is not a surface, so a DJ typing \
                 its name gets nothing and the rule covers nothing"
            );
        }
    }

    /// The hands outrank the paperwork, and §58's numbering is §58's.
    #[test]
    fn the_tiers_rank_the_way_the_section_numbers_them() {
        assert!(Tier::Glanceable < Tier::Performable);
        assert!(Tier::Performable < Tier::Contextual);
        assert!(Tier::Contextual < Tier::Preparation);
        for (n, tier) in Tier::ALL.into_iter().enumerate() {
            assert_eq!(u8::try_from(n + 1).unwrap(), tier.rank());
        }
    }

    /// §18's sentence, made a rule.
    ///
    /// `Attention::performing`'s doc comment has said "Tier 1 and 2 only;
    /// nothing else may take room" since it was written, and until now that was
    /// a sentence nothing could be checked against.
    #[test]
    fn a_mix_leaves_room_for_the_first_two_tiers_and_no_others() {
        assert!(Tier::Glanceable.survives_a_mix());
        assert!(Tier::Performable.survives_a_mix());
        assert!(!Tier::Contextual.survives_a_mix());
        assert!(!Tier::Preparation.survives_a_mix());
    }

    /// **The budget and this predicate are one rule, not two.**
    ///
    /// `Attention::room_for` is the same sentence carried as data so something
    /// can read it, and `survives_a_mix` is the same sentence as a question.
    /// Two spellings of one rule is how they come to disagree — which is the
    /// failure this codebase has met often enough to have a name for it — so
    /// the mixing budget is checked against the predicate here rather than
    /// left to be kept in step by whoever edits one of them.
    #[test]
    fn the_mixing_budget_leaves_room_for_exactly_what_survives_a_mix() {
        let mixing = crate::cockpit::Attention::performing().room_for;
        for tier in Tier::ALL {
            assert_eq!(
                tier <= mixing,
                tier.survives_a_mix(),
                "{} is on one side of §18's line by the budget and the other \
                 by the predicate",
                tier.name()
            );
        }
    }

    /// The obvious cases, written out, so the table can be read against §58.
    #[test]
    fn the_obvious_cases_land_where_the_section_puts_them() {
        assert_eq!(of_verb("play"), Tier::Glanceable);
        assert_eq!(of_verb("cue"), Tier::Glanceable);
        assert_eq!(of_verb("eq_low"), Tier::Performable);
        assert_eq!(of_verb("loop_in"), Tier::Performable);
        assert_eq!(of_verb("crossfader"), Tier::Performable);
        assert_eq!(of_verb("stem_mute"), Tier::Performable);
        assert_eq!(of_verb("grid_nudge"), Tier::Preparation);

        assert_eq!(of_surface("next"), Tier::Contextual);
        assert_eq!(of_surface("room"), Tier::Contextual);
        assert_eq!(of_surface("stems"), Tier::Performable);
        assert_eq!(of_surface("library"), Tier::Preparation);
        assert_eq!(of_surface("settings"), Tier::Preparation);
    }

    /// Every tier is spelled the same way stored as it is spoken.
    #[test]
    fn a_tier_is_spelled_the_same_way_stored_as_it_is_spoken() {
        for tier in Tier::ALL {
            assert_eq!(
                serde_json::to_string(&tier).expect("a tier serialises"),
                format!("\"{}\"", tier.name())
            );
        }
    }
}

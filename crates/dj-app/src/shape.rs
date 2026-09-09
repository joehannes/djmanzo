//! What a transition style does with its hands.
//!
//! A transition is two channel faders moving in opposite directions, and then
//! everything else: the bass swap that stops two kicks fighting, the echo
//! thrown over a record as it leaves, the vocal held over the incoming
//! instrumental. §68 of the directive asks the transition object to carry
//! those as `outgoingStems`, `incomingStems`, `eqPlan` and `fxPlan` — because
//! that is what makes a transition something an interface can *show* before it
//! is pressed, rather than a name that only means something once it has
//! happened.
//!
//! # One table, not two descriptions
//!
//! The obvious way to do this is to keep the automix as it was and write the
//! panel copy beside it. That gives two statements of what a blend does, in
//! different files, in different languages, and the day someone changes the
//! bass swap only one of them is right — and it is the silent one, because the
//! panel keeps confidently describing the mix djmanzo used to perform.
//!
//! So this is the one table. [`shape`] answers *what this style does*, the
//! automix performs the answer rather than deciding inline, and the pair view
//! reads the same answer to say what pressing the button will do. Adding a
//! style is a row here; nothing in [`crate::automix`] changes.
//!
//! # Deliberately not the geometry
//!
//! Where the mix starts, how long it runs and how the faders travel are
//! [`crate::plan`]'s and the automix's. What lives here is only what a style
//! does *beyond* the faders — with one exception, [`Shape::overlaps`], because
//! a cut's defining property is that there is no overlap for a stem or an EQ
//! plan to happen over, and the automix needs to ask that question of the
//! table rather than of the style.

use dj_core::action::{Stem, TransitionStyle};
use dj_core::fx::EffectKind;

/// The FX rack slot a transition uses on the outgoing deck.
///
/// Fixed rather than asked about: automix is driving, and a transition that
/// borrowed whichever effect the DJ happened to leave loaded would be a
/// different transition every time.
pub const SLOT: u8 = 1;

/// How long the outgoing effect is set to, in beats.
///
/// One bar at four-four: long enough to hear as a tail, short enough that it
/// has decayed before the incoming record is carrying the room.
pub const FX_BEATS: f32 = 4.0;

/// What happens to one deck's stems for the length of a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stems {
    /// All four left as the DJ had them.
    AsFound,
    /// One stem soloed for the length of the mix, and released at the end.
    ///
    /// Solo rather than muting the other three, so the deck is handed back
    /// with three mutes the DJ did not set.
    Solo(Stem),
}

impl Stems {
    /// The stem this plan solos, if it solos one.
    #[must_use]
    pub const fn solo(self) -> Option<Stem> {
        match self {
            Stems::AsFound => None,
            Stems::Solo(stem) => Some(stem),
        }
    }
}

/// What happens to the two decks' EQ across the transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Eq {
    /// Neither deck's EQ moves; the channel faders carry the whole mix.
    Flat,
    /// The low band is handed over: the outgoing kick comes out as the
    /// incoming one arrives, so the two are never both carrying the bottom.
    ///
    /// `done_by` is the fraction of the transition at which the handover has
    /// completed — 0.5 finishes the swap at the halfway point and leaves the
    /// second half to the faders alone.
    HandOverLows { done_by: f64 },
}

impl Eq {
    /// How far through the low-end handover the mix is at `progress`.
    ///
    /// 0 is the outgoing record holding the bottom, 1 is the incoming one.
    /// [`Eq::Flat`] is 0 throughout, which is what "nobody swapped anything"
    /// means and is why the automix can ask this unconditionally.
    #[must_use]
    pub fn swap_at(self, progress: f64) -> f64 {
        match self {
            Eq::Flat => 0.0,
            Eq::HandOverLows { done_by } => {
                if done_by <= 0.0 {
                    1.0
                } else {
                    (progress / done_by).clamp(0.0, 1.0)
                }
            }
        }
    }
}

/// What is thrown over the outgoing deck while it leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fx {
    /// Nothing. The rack is left as the DJ had it.
    AsFound,
    /// One effect enabled on the outgoing deck for the transition, and switched
    /// off again when it ends.
    Outgoing {
        slot: u8,
        kind: EffectKind,
        beats: f32,
    },
}

impl Fx {
    /// The effect this plan throws, if it throws one.
    #[must_use]
    pub const fn outgoing(self) -> Option<(u8, EffectKind, f32)> {
        match self {
            Fx::AsFound => None,
            Fx::Outgoing { slot, kind, beats } => Some((slot, kind, beats)),
        }
    }
}

/// Everything a style does beyond the two channel faders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// Whether the two records are audible at the same time at all.
    ///
    /// False only for a cut, where the outgoing deck stops on the tick the
    /// incoming one starts and there is no span for anything else to happen
    /// over.
    pub overlaps: bool,
    pub outgoing_stems: Stems,
    pub incoming_stems: Stems,
    pub eq: Eq,
    pub fx: Fx,
}

impl Shape {
    /// What this shape does, in the words a panel can print.
    ///
    /// Derived from the fields rather than written beside them, so a line
    /// cannot survive the behaviour it describes.
    #[must_use]
    pub fn words(&self) -> Vec<String> {
        let mut said = Vec::new();
        if !self.overlaps {
            said.push("no overlap: one stops, the next starts".to_owned());
            return said;
        }
        if let Some(stem) = self.outgoing_stems.solo() {
            said.push(format!("outgoing deck: {stem} only"));
        }
        if let Some(stem) = self.incoming_stems.solo() {
            said.push(format!("incoming deck: {stem} only"));
        }
        match self.eq {
            Eq::Flat => said.push("EQ untouched; the faders carry it".to_owned()),
            Eq::HandOverLows { done_by } => said.push(format!(
                "low EQ handed over by {}% through",
                (done_by * 100.0).round() as i64
            )),
        }
        if let Some((slot, kind, beats)) = self.fx.outgoing() {
            said.push(format!(
                "{} on the outgoing deck, {beats:.0} beats, slot {slot}",
                kind.name()
            ));
        }
        said
    }
}

/// What a style does. The one table.
#[must_use]
pub fn shape(style: TransitionStyle) -> Shape {
    let plain = Shape {
        overlaps: true,
        outgoing_stems: Stems::AsFound,
        incoming_stems: Stems::AsFound,
        eq: Eq::Flat,
        fx: Fx::AsFound,
    };
    let echo = Fx::Outgoing {
        slot: SLOT,
        kind: EffectKind::Echo,
        beats: FX_BEATS,
    };
    match style {
        TransitionStyle::Cut => Shape {
            overlaps: false,
            ..plain
        },
        TransitionStyle::Fade => plain,
        // The bass swap. Both kicks at once is the thing that makes an
        // automatic mix sound automatic, so the outgoing low end comes out over
        // the first half and the incoming one arrives over it.
        TransitionStyle::Blend => Shape {
            eq: Eq::HandOverLows { done_by: 0.5 },
            ..plain
        },
        // Thrown as the record leaves, so it dissolves rather than ends.
        TransitionStyle::Echo => Shape { fx: echo, ..plain },
        // Keep the outgoing vocal, cut its instruments, bring the incoming
        // record in underneath, and let the vocal echo out at the end.
        TransitionStyle::VocalDrop => Shape {
            outgoing_stems: Stems::Solo(Stem::Vocal),
            fx: echo,
            ..plain
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every style has a shape, and only a cut refuses to overlap.
    #[test]
    fn every_style_is_in_the_table() {
        for style in TransitionStyle::ALL {
            let shape = shape(style);
            assert_eq!(
                shape.overlaps,
                style != TransitionStyle::Cut,
                "{style:?} overlaps?"
            );
            assert!(!shape.words().is_empty(), "{style:?} says nothing");
        }
    }

    /// **The bass is handed over, not shared.**
    ///
    /// A blend that swaps the lows over the whole transition has both kicks at
    /// half strength through the middle, which is the muddy overlap the swap
    /// exists to prevent. Finishing by the halfway point means the second half
    /// is the incoming record's bottom alone.
    #[test]
    fn a_blend_finishes_the_bass_swap_before_the_mix_does() {
        let eq = shape(TransitionStyle::Blend).eq;
        assert_eq!(eq.swap_at(0.0), 0.0);
        assert!(eq.swap_at(0.25) > 0.0 && eq.swap_at(0.25) < 1.0);
        assert_eq!(eq.swap_at(0.5), 1.0);
        assert_eq!(eq.swap_at(1.0), 1.0);
    }

    /// A style with no EQ plan reports no swap at any point, so the automix can
    /// ask without first asking whether to ask.
    #[test]
    fn a_flat_eq_never_swaps() {
        let eq = shape(TransitionStyle::Fade).eq;
        for tenth in 0..=10 {
            assert_eq!(eq.swap_at(f64::from(tenth) / 10.0), 0.0);
        }
    }

    /// The vocal drop is the only style that touches a stem, and it holds the
    /// outgoing vocal rather than the incoming one -- the other way round is a
    /// different transition, and a worse one: the record the room is dancing to
    /// loses its instruments to a track nobody has heard yet.
    #[test]
    fn only_the_vocal_drop_solos_and_it_solos_the_outgoing_vocal() {
        for style in TransitionStyle::ALL {
            let shape = shape(style);
            assert_eq!(shape.incoming_stems, Stems::AsFound, "{style:?} incoming");
            let want = (style == TransitionStyle::VocalDrop).then_some(Stem::Vocal);
            assert_eq!(shape.outgoing_stems.solo(), want, "{style:?} outgoing");
        }
    }

    /// What the panel prints comes from the fields, so it cannot describe a
    /// blend that no longer swaps anything.
    #[test]
    fn the_words_come_from_the_plan() {
        let said = shape(TransitionStyle::VocalDrop).words().join("; ");
        assert!(said.contains("vocal only"), "{said}");
        assert!(said.contains("echo"), "{said}");
        let cut = shape(TransitionStyle::Cut).words().join("; ");
        assert!(cut.contains("no overlap"), "{cut}");
        assert!(!cut.contains("EQ"), "a cut has nothing to EQ over: {cut}");
    }
}

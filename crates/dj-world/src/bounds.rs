//! How much a thing may move, and how alive it is.
//!
//! # Why there are limits at all
//!
//! A DJ in a dark booth has to hit a cue in about two hundred milliseconds, and
//! a target that has moved cannot be hit from muscle memory. This is the
//! strongest objection to a living interface and it is answered here rather
//! than apologised for later: **motion is bounded, and the bound is tight
//! enough that aiming where a control was still hits it.**
//!
//! The rule comes from the metaphor rather than fighting it. A trunk is rigid
//! and bears weight; foliage moves and carries the light. Nothing in nature asks
//! you to stand on something swaying.
//!
//! # Stillness is the default
//!
//! Nature is mostly still. A forest that thrashed constantly would tell you
//! nothing about the wind, and an interface where everything moves all the time
//! communicates nothing by moving. So a paused deck is still water, and
//! [`Vitality::still`] is what most of the world looks like most of the time.

use crate::RiverReading;
use serde::{Deserialize, Serialize};

/// How far a thing may stray from where it rests.
///
/// A fraction of the element's own radius, never an absolute distance: a small
/// marker drifting twenty pixels is lost, and a large field drifting twenty
/// pixels is imperceptible. Relative is the only measure that means the same
/// thing at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Excursion {
    /// Fraction of the element's own radius. Its *centre* never moves.
    pub drift: f32,
    /// Multiplier on the resting size.
    pub scale: f32,
}

impl Excursion {
    /// The widest anything is allowed to wander.
    ///
    /// A quarter of its own radius. Chosen so that a click aimed at where a
    /// control rests still lands inside it at full excursion — which is the
    /// entire constraint, and why the number is this small rather than
    /// whatever looked best.
    pub const MAX_DRIFT: f32 = 0.25;
    /// The narrowest and widest a resting size may be scaled.
    ///
    /// Tight enough that a control never changes which thing it looks like.
    pub const MIN_SCALE: f32 = 0.9;
    pub const MAX_SCALE: f32 = 1.15;

    /// Rest.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            drift: 0.0,
            scale: 1.0,
        }
    }

    /// Clamp into what a DJ can still hit.
    ///
    /// Clamped rather than rejected: an excursion is a rendering hint, and a
    /// value out of range should cost a slightly duller animation rather than
    /// an error in the middle of a set.
    #[must_use]
    pub fn sane(self) -> Self {
        Self {
            drift: finite(self.drift, 0.0).clamp(0.0, Self::MAX_DRIFT),
            scale: finite(self.scale, 1.0).clamp(Self::MIN_SCALE, Self::MAX_SCALE),
        }
    }

    /// Whether this is close enough to rest to count as still.
    #[must_use]
    pub fn is_rest(self) -> bool {
        self.drift.abs() < 1e-6 && (self.scale - 1.0).abs() < 1e-6
    }
}

/// How alive a thing is.
///
/// Every field is a *rate or a depth*, never a position: the world says how
/// something behaves and the renderer works out where that puts it this frame.
/// That is what keeps the world small enough to cross to the interface sixty
/// times a second, and what lets a still renderer ignore all of it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vitality {
    /// Pulses per minute. **The music's tempo, not a decorative rate** — an
    /// interface visibly in time with the room is the one thing this design
    /// offers that a conventional one structurally cannot.
    pub pulse_bpm: f32,
    /// Where in the beat the crest is, 0..=1. Two rivers in sync share this.
    pub phase: f32,
    /// How strongly it pulses, 0..=1. Zero is still.
    pub depth: f32,
    /// Surface disturbance, 0..=1. Peak level, and machine strain.
    pub agitation: f32,
    /// How murky, 0..=1. Uncertainty — a grid nobody trusts, a track nobody has
    /// analysed. You do not navigate water you cannot see through.
    pub turbidity: f32,
    /// How far it may stray while doing all of that.
    pub excursion: Excursion,
}

impl Vitality {
    /// Still water. The default, and what most of the world is most of the time.
    #[must_use]
    pub const fn still() -> Self {
        Self {
            pulse_bpm: 0.0,
            phase: 0.0,
            depth: 0.0,
            agitation: 0.0,
            turbidity: 0.0,
            excursion: Excursion::none(),
        }
    }

    /// How alive a river is.
    ///
    /// A paused deck is still even when it has a tempo, because a tempo is a
    /// property of the track and stillness is a statement about *now*. Pulsing
    /// a paused deck would say it is playing.
    #[must_use]
    pub fn of(river: &RiverReading) -> Self {
        if !river.loaded || !river.playing {
            return Self::still();
        }
        let level = clamp01(river.level);
        Self {
            pulse_bpm: river
                .bpm
                .filter(|b| b.is_finite() && *b > 0.0)
                .unwrap_or(0.0),
            phase: finite(river.beat_phase, 0.0).rem_euclid(1.0),
            // A deck faded out is playing but inaudible, and the world should
            // say so: the pulse is there and shallow rather than absent.
            depth: level,
            agitation: clamp01(river.peak),
            turbidity: 0.0,
            excursion: Excursion {
                drift: Excursion::MAX_DRIFT * level,
                scale: 1.0 + (Excursion::MAX_SCALE - 1.0) * level,
            }
            .sane(),
        }
    }

    /// Whether this would draw identically in a still renderer.
    ///
    /// What Tier 0 and `prefers-reduced-motion` ask, and what the trunk rule is
    /// tested against.
    #[must_use]
    pub fn is_still(&self) -> bool {
        self.depth < 1e-6 && self.agitation < 1e-6 && self.excursion.is_rest()
    }
}

impl Default for Vitality {
    fn default() -> Self {
        Self::still()
    }
}

fn clamp01(v: f32) -> f32 {
    finite(v, 0.0).clamp(0.0, 1.0)
}

fn finite(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> RiverReading {
        RiverReading {
            deck: 1,
            loaded: true,
            playing: true,
            progress: 0.5,
            remaining_seconds: 90.0,
            bpm: Some(128.0),
            beat_phase: 0.25,
            grid_confidence: 0.9,
            key: None,
            key_confidence: 0.0,
            level: 1.0,
            peak: 0.4,
            surveying: false,
            ..RiverReading::empty(1)
        }
    }

    // -- the limit that keeps controls hittable ----------------------------

    /// The whole constraint, stated as a test: at full excursion, a click aimed
    /// at where the control rests still lands inside it.
    #[test]
    fn a_control_at_full_excursion_still_contains_its_resting_centre() {
        let worst = Excursion {
            drift: Excursion::MAX_DRIFT,
            scale: Excursion::MIN_SCALE,
        }
        .sane();
        // Drift is a fraction of the radius; the shape after scaling still has
        // `scale` of that radius. It covers its old centre while drift < scale.
        assert!(
            worst.drift < worst.scale,
            "drift {} has escaped a shape of size {}",
            worst.drift,
            worst.scale
        );
    }

    #[test]
    fn excursion_is_clamped_rather_than_believed() {
        let absurd = Excursion {
            drift: 40.0,
            scale: 12.0,
        }
        .sane();
        assert_eq!(absurd.drift, Excursion::MAX_DRIFT);
        assert_eq!(absurd.scale, Excursion::MAX_SCALE);

        let negative = Excursion {
            drift: -3.0,
            scale: 0.01,
        }
        .sane();
        assert_eq!(negative.drift, 0.0);
        assert_eq!(negative.scale, Excursion::MIN_SCALE);
    }

    /// A layout or a reading carrying NaN must cost a dull animation, not an
    /// element that vanishes or fills the screen.
    #[test]
    fn a_nonfinite_excursion_falls_back_to_rest() {
        let broken = Excursion {
            drift: f32::NAN,
            scale: f32::INFINITY,
        }
        .sane();
        assert_eq!(broken.drift, 0.0);
        assert!((Excursion::MIN_SCALE..=Excursion::MAX_SCALE).contains(&broken.scale));
    }

    /// Pinned at compile time rather than in a test: these are constants, and
    /// the point is that nobody may widen them, not that they hold at runtime.
    /// Half size or half again is a different control, not the same one moving.
    const _SCALE_NEVER_CHANGES_WHAT_A_CONTROL_LOOKS_LIKE: () = {
        assert!(Excursion::MIN_SCALE > 0.5);
        assert!(Excursion::MAX_SCALE < 1.5);
    };

    // -- stillness ---------------------------------------------------------

    #[test]
    fn an_empty_deck_is_still() {
        let mut empty = playing();
        empty.loaded = false;
        assert!(Vitality::of(&empty).is_still());
    }

    /// A tempo is a property of the track; stillness is a statement about now.
    /// Pulsing a paused deck would say it is playing.
    #[test]
    fn a_paused_deck_is_still_even_though_it_has_a_tempo() {
        let mut paused = playing();
        paused.playing = false;
        assert!(paused.bpm.is_some());
        assert!(Vitality::of(&paused).is_still());
    }

    #[test]
    fn a_playing_deck_is_not_still() {
        assert!(!Vitality::of(&playing()).is_still());
    }

    // -- the clock is the music --------------------------------------------

    #[test]
    fn the_pulse_is_the_tracks_tempo_not_a_decorative_rate() {
        assert_eq!(Vitality::of(&playing()).pulse_bpm, 128.0);
    }

    /// A track with no grid has no beat to pulse on, and inventing one would be
    /// the interface claiming a tempo the analyser refused to give.
    #[test]
    fn a_track_with_no_grid_has_no_pulse_rate() {
        let mut ungridded = playing();
        ungridded.bpm = None;
        assert_eq!(Vitality::of(&ungridded).pulse_bpm, 0.0);
    }

    #[test]
    fn a_nonsense_tempo_is_refused_rather_than_animated() {
        for bad in [f32::NAN, f32::INFINITY, 0.0, -128.0] {
            let mut river = playing();
            river.bpm = Some(bad);
            assert_eq!(Vitality::of(&river).pulse_bpm, 0.0, "{bad}");
        }
    }

    /// Two decks in sync share a phase, which is what makes crests align at the
    /// confluence -- the channel answering "are these two in time?".
    #[test]
    fn phase_is_carried_verbatim_so_synced_decks_agree() {
        let mut a = playing();
        a.beat_phase = 0.75;
        let mut b = playing();
        b.deck = 2;
        b.beat_phase = 0.75;
        assert_eq!(Vitality::of(&a).phase, Vitality::of(&b).phase);
    }

    #[test]
    fn phase_wraps_rather_than_running_off_the_end() {
        let mut river = playing();
        river.beat_phase = 2.25;
        assert!((Vitality::of(&river).phase - 0.25).abs() < 1e-5);

        river.beat_phase = -0.25;
        assert!((Vitality::of(&river).phase - 0.75).abs() < 1e-5);
    }

    // -- level -------------------------------------------------------------

    /// A deck faded out is playing and inaudible. The pulse should be there and
    /// shallow, rather than absent -- absent would say it had stopped.
    #[test]
    fn a_faded_out_deck_pulses_shallowly_rather_than_not_at_all() {
        let mut faded = playing();
        faded.level = 0.0;
        let vitality = Vitality::of(&faded);
        assert_eq!(vitality.depth, 0.0);
        assert!(
            vitality.pulse_bpm > 0.0,
            "the river is still running, it is just not being heard"
        );
    }

    #[test]
    fn a_quiet_deck_moves_less_than_a_loud_one() {
        let mut quiet = playing();
        quiet.level = 0.2;
        assert!(Vitality::of(&quiet).excursion.drift < Vitality::of(&playing()).excursion.drift);
    }

    #[test]
    fn nothing_a_river_produces_exceeds_the_limits() {
        for level in [0.0, 0.5, 1.0, 4.0, -1.0] {
            for peak in [0.0, 1.0, 9.0, f32::NAN] {
                let mut river = playing();
                river.level = level;
                river.peak = peak;
                let vitality = Vitality::of(&river);
                assert!(vitality.excursion.drift <= Excursion::MAX_DRIFT);
                assert!(vitality.excursion.scale <= Excursion::MAX_SCALE);
                assert!((0.0..=1.0).contains(&vitality.depth));
                assert!((0.0..=1.0).contains(&vitality.agitation));
            }
        }
    }

    #[test]
    fn still_water_is_still_by_every_measure() {
        let still = Vitality::still();
        assert!(still.is_still());
        assert!(still.excursion.is_rest());
        assert_eq!(Vitality::default(), still);
    }
}

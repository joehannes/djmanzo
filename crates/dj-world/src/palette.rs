//! Colour, with one meaning per axis.
//!
//! Colour stops carrying information the moment two things use the same
//! channel, so each axis has exactly one job and keeps it everywhere:
//!
//! | Axis | Means |
//! |---|---|
//! | **Hue** | musical key, on the Camelot wheel |
//! | **Saturation** | certainty — pale is unsure, saturated is known |
//! | **Lightness** | energy and level |
//! | **No hue at all** | structure: chrome, furniture, anything not music |
//!
//! Two consequences fall out, and both are wanted. Uncertainty looks the same
//! everywhere — a weak beat grid, an unanalysed track and a shaky key detection
//! are all pale, and a DJ learns that once rather than three times. And colour
//! belongs to music: if a thing is grey it is furniture, and if it has hue it is
//! telling you something about the sound.
//!
//! # Hue is never the only channel
//!
//! Roughly one man in twelve cannot separate some hues, so nothing may rest on
//! hue alone. Key is written as text as well; compatibility is shown by
//! behaviour — whether two waters blend or seam — rather than by colour; level
//! is width as well as lightness. The standing test is in
//! [VISUAL-LANGUAGE.md](../../../docs/VISUAL-LANGUAGE.md): **switch the display
//! to greyscale and the interface must still work.**

use dj_core::{Mode, MusicalKey};
use serde::{Deserialize, Serialize};

/// A colour as the world describes it, before any renderer turns it into
/// pixels. Hue in degrees, the rest 0..=1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tint {
    /// 0..360. Meaningless when `saturation` is zero, and set to zero there so
    /// two greys compare equal rather than differing in a field nobody can see.
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

impl Tint {
    /// Structure: no hue, no claim about music.
    #[must_use]
    pub fn structural(lightness: f32) -> Self {
        Self {
            hue: 0.0,
            saturation: 0.0,
            lightness: lightness.clamp(0.0, 1.0),
        }
    }

    /// The colour of a piece of music.
    ///
    /// `key` may be absent — an unanalysed track has no key, and the honest
    /// answer is grey rather than a hue picked at random, which would be the
    /// interface asserting something it does not know.
    #[must_use]
    pub fn musical(key: Option<MusicalKey>, certainty: f32, energy: f32) -> Self {
        let certainty = clamp01(certainty);
        let lightness = 0.25 + clamp01(energy) * 0.5;
        match key {
            Some(key) => Self {
                hue: hue_of(key),
                // Never fully saturated: a key is a measurement, and the top of
                // the range is reserved for a grid the DJ set by hand.
                saturation: certainty * 0.85,
                lightness,
            },
            None => Self {
                hue: 0.0,
                saturation: 0.0,
                lightness,
            },
        }
    }

    /// What this colour looks like with the hue channel removed.
    ///
    /// Used by the greyscale test rather than for display: two things that must
    /// be told apart may not differ in hue alone.
    #[must_use]
    pub fn greyscale(self) -> f32 {
        self.lightness
    }
}

/// Where a key sits on the hue circle.
///
/// A circle for a circle: the Camelot wheel has twelve hours and hue has 360
/// degrees, so the mapping is exact rather than decorative — and it means keys
/// that mix well are *adjacent colours*, which is the property the whole scheme
/// exists for.
///
/// The two rings are not given separate hues. `8A` and `8B` are relative minor
/// and major, they mix, and giving them different colours would say they do not.
/// The ring shows in lightness instead, minor sitting slightly darker, which is
/// also how the two actually feel.
#[must_use]
pub fn hue_of(key: MusicalKey) -> f32 {
    // Hour 1 at 0°, going round. `- 1` because the wheel is 1-based.
    (f32::from(key.hour() - 1) / 12.0) * 360.0
}

/// How much darker the minor ring sits than the major.
///
/// Small on purpose. It has to be enough to tell `8A` from `8B` at a glance and
/// not so much that it competes with level, which owns lightness.
pub const MODE_LIGHTNESS_STEP: f32 = 0.06;

/// The lightness offset for a key's ring.
#[must_use]
pub fn mode_offset(key: MusicalKey) -> f32 {
    match key.mode() {
        Mode::Minor => -MODE_LIGHTNESS_STEP,
        Mode::Major => MODE_LIGHTNESS_STEP,
    }
}

/// How two keys behave where their waters meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Confluence {
    /// Nothing to say: one side or both has no key.
    ///
    /// The default, because an empty world knows nothing about anybody's keys
    /// and the alternatives would each be an assertion.
    #[default]
    Unknown,
    /// The same key. One body of water.
    Same,
    /// Neighbouring on the wheel, or the relative major/minor. They blend.
    Blend,
    /// They do not. A seam runs down the middle of the confluence.
    Seam,
}

/// What happens where two rivers meet.
///
/// This is the channel that carries harmonic compatibility, and it is
/// deliberately *behaviour* rather than colour — see the module note on hue
/// never being the only channel.
#[must_use]
pub fn confluence(left: Option<MusicalKey>, right: Option<MusicalKey>) -> Confluence {
    let (Some(a), Some(b)) = (left, right) else {
        return Confluence::Unknown;
    };
    if a == b {
        return Confluence::Same;
    }
    if a.compatible().contains(&b) {
        Confluence::Blend
    } else {
        Confluence::Seam
    }
}

fn clamp01(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    // -- hue ---------------------------------------------------------------

    /// The property the scheme exists for: keys that mix are adjacent colours.
    #[test]
    fn neighbouring_keys_are_neighbouring_hues() {
        let eight = hue_of(key(8, Mode::Minor));
        let nine = hue_of(key(9, Mode::Minor));
        assert!(
            (nine - eight - 30.0).abs() < 1e-3,
            "one hour is thirty degrees"
        );
    }

    #[test]
    fn the_wheel_closes() {
        // Hour 12 and hour 1 are neighbours, so they must be neighbours in hue
        // too -- across the 360/0 seam.
        let twelve = hue_of(key(12, Mode::Minor));
        let one = hue_of(key(1, Mode::Minor));
        let gap = (twelve - one).abs().min(360.0 - (twelve - one).abs());
        assert!(
            (gap - 30.0).abs() < 1e-3,
            "{twelve} and {one} are not adjacent"
        );
    }

    /// Relative minor and major mix, so they must not be different colours.
    #[test]
    fn the_two_rings_share_a_hue() {
        assert!((hue_of(key(8, Mode::Minor)) - hue_of(key(8, Mode::Major))).abs() < 1e-6);
    }

    #[test]
    fn the_rings_are_still_told_apart_by_lightness() {
        assert!(mode_offset(key(8, Mode::Minor)) < mode_offset(key(8, Mode::Major)));
    }

    // -- certainty ---------------------------------------------------------

    #[test]
    fn an_uncertain_key_is_paler_than_a_certain_one() {
        let sure = Tint::musical(Some(key(8, Mode::Minor)), 1.0, 0.5);
        let unsure = Tint::musical(Some(key(8, Mode::Minor)), 0.2, 0.5);
        assert!(unsure.saturation < sure.saturation);
        assert!(
            (unsure.lightness - sure.lightness).abs() < 1e-6,
            "certainty must not leak into the lightness channel"
        );
    }

    /// The honest answer for a track nobody has analysed is grey, not a hue
    /// chosen at random -- which would be the interface asserting a key it does
    /// not know.
    #[test]
    fn no_key_is_grey_rather_than_a_guessed_hue() {
        let unknown = Tint::musical(None, 1.0, 0.5);
        assert_eq!(unknown.saturation, 0.0);
    }

    #[test]
    fn structure_carries_no_hue_at_all() {
        let chrome = Tint::structural(0.4);
        assert_eq!(chrome.saturation, 0.0);
        assert_eq!(chrome.hue, 0.0, "a grey must not carry an invisible hue");
    }

    #[test]
    fn energy_owns_lightness_and_nothing_else_does() {
        let quiet = Tint::musical(Some(key(8, Mode::Minor)), 1.0, 0.0);
        let loud = Tint::musical(Some(key(8, Mode::Minor)), 1.0, 1.0);
        assert!(loud.lightness > quiet.lightness);
        assert_eq!(
            loud.saturation, quiet.saturation,
            "level must not leak into the certainty channel"
        );
    }

    #[test]
    fn absurd_inputs_are_clamped_rather_than_producing_an_impossible_colour() {
        for bad in [f32::NAN, f32::INFINITY, -5.0, 40.0] {
            let tint = Tint::musical(Some(key(1, Mode::Major)), bad, bad);
            assert!((0.0..=1.0).contains(&tint.saturation), "{bad}");
            assert!((0.0..=1.0).contains(&tint.lightness), "{bad}");
        }
    }

    // -- the greyscale rule ------------------------------------------------

    /// The standing test from VISUAL-LANGUAGE.md. Two keys differ *only* in hue,
    /// so hue can never be the sole carrier of anything that matters -- which is
    /// why compatibility is behaviour and key is also text.
    #[test]
    fn two_keys_are_indistinguishable_in_greyscale_which_is_why_hue_is_never_alone() {
        let a = Tint::musical(Some(key(2, Mode::Minor)), 1.0, 0.5);
        let b = Tint::musical(Some(key(9, Mode::Minor)), 1.0, 0.5);
        assert_ne!(a.hue, b.hue);
        assert!(
            (a.greyscale() - b.greyscale()).abs() < 1e-6,
            "they are the same grey -- so anything resting on this difference \
             would be invisible to a colour-blind DJ"
        );
    }

    /// Certainty, by contrast, *does* survive greyscale in the renderer, because
    /// a desaturated colour is visibly flatter. This test pins the intent: the
    /// two channels are not equally safe and the design knows which is which.
    #[test]
    fn certainty_does_not_rest_on_hue() {
        let sure = Tint::musical(Some(key(2, Mode::Minor)), 1.0, 0.5);
        let unsure = Tint::musical(Some(key(2, Mode::Minor)), 0.1, 0.5);
        assert_ne!(sure.saturation, unsure.saturation);
        assert_eq!(sure.hue, unsure.hue, "certainty is not a hue shift");
    }

    // -- the confluence ----------------------------------------------------

    #[test]
    fn the_same_key_is_one_body_of_water() {
        let k = key(8, Mode::Minor);
        assert_eq!(confluence(Some(k), Some(k)), Confluence::Same);
    }

    #[test]
    fn neighbours_and_the_relative_mode_blend() {
        let eight_a = key(8, Mode::Minor);
        assert_eq!(
            confluence(Some(eight_a), Some(key(9, Mode::Minor))),
            Confluence::Blend
        );
        assert_eq!(
            confluence(Some(eight_a), Some(key(7, Mode::Minor))),
            Confluence::Blend
        );
        assert_eq!(
            confluence(Some(eight_a), Some(key(8, Mode::Major))),
            Confluence::Blend,
            "the relative major is the classic harmonic move"
        );
    }

    #[test]
    fn keys_across_the_wheel_seam() {
        assert_eq!(
            confluence(Some(key(8, Mode::Minor)), Some(key(2, Mode::Minor))),
            Confluence::Seam
        );
    }

    /// Not knowing is its own answer. Reporting a seam for a track nobody has
    /// analysed would tell a DJ their mix will clash on no evidence at all.
    #[test]
    fn an_unanalysed_side_means_unknown_not_seam() {
        let k = key(8, Mode::Minor);
        assert_eq!(confluence(Some(k), None), Confluence::Unknown);
        assert_eq!(confluence(None, Some(k)), Confluence::Unknown);
        assert_eq!(confluence(None, None), Confluence::Unknown);
    }

    /// Compatibility is symmetric, and a confluence has no favoured side.
    #[test]
    fn the_confluence_reads_the_same_from_either_bank() {
        for hour_a in 1..=12u8 {
            for hour_b in 1..=12u8 {
                for mode_a in [Mode::Minor, Mode::Major] {
                    for mode_b in [Mode::Minor, Mode::Major] {
                        let a = key(hour_a, mode_a);
                        let b = key(hour_b, mode_b);
                        assert_eq!(
                            confluence(Some(a), Some(b)),
                            confluence(Some(b), Some(a)),
                            "{} against {}",
                            a.camelot(),
                            b.camelot()
                        );
                    }
                }
            }
        }
    }
}

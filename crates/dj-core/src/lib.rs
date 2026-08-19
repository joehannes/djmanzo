//! Domain types shared by every djmanzo crate.
//!
//! This crate is the bottom of the dependency graph. It has no I/O, spawns no
//! threads, and depends on nothing but `std`, `serde` and `thiserror`. Anything
//! that needs a file handle, a device or a thread belongs one layer up.
//!
//! The types here are deliberately strict: [`music::Bpm`] refuses a tempo of
//! zero, [`time::FramePos`] refuses NaN, [`deck::DeckId`] refuses to exist
//! outside the deck count. Validating at construction means the realtime engine
//! can assume its inputs are sane, which is exactly where a defensive check is
//! most expensive and a bad value most damaging.

pub mod action;
pub mod deck;

pub mod fx;
pub mod hotcue;
pub mod music;
pub mod param;
pub mod time;
pub mod vocabulary;

pub use action::{Action, DeckAction, MixerAction};
pub use deck::{CrossfaderAssign, DeckId, MAX_DECKS};
pub use fx::{EffectKind, FX_SLOTS, FxChange, Placement};
pub use hotcue::{HOT_CUE_SLOTS, LoopLimits, LoopRegion};
pub use music::{Beatgrid, Bpm, Confidence, Mode, MusicalKey};
pub use param::{DeckParam, GlobalParam, ParamId};
pub use time::{FramePos, Rate, SampleRate};
pub use vocabulary::{ArgSpec, Target, VerbSpec, vocabulary};

pub mod track;
pub use track::{TrackId, TrackInfo};

/// Convert decibels to a linear amplitude multiplier.
///
/// `-inf`..`0` dB maps to `0.0`..`1.0`. Anything at or below [`SILENCE_DB`]
/// returns exactly zero, so a fader at its bottom is truly silent rather than
/// leaving a -60 dB residue that shows up on a VU meter.
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= SILENCE_DB {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

/// Convert a linear amplitude multiplier to decibels.
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        SILENCE_DB
    } else {
        20.0 * linear.log10()
    }
}

/// The level treated as silence. Below this, gain snaps to zero.
pub const SILENCE_DB: f32 = -96.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_gain_is_zero_db() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!(linear_to_db(1.0).abs() < 1e-6);
    }

    #[test]
    fn minus_six_db_is_about_half_amplitude() {
        assert!((db_to_linear(-6.0) - 0.501_187).abs() < 1e-5);
    }

    #[test]
    fn silence_is_exactly_zero() {
        assert_eq!(db_to_linear(SILENCE_DB), 0.0);
        assert_eq!(db_to_linear(-200.0), 0.0);
        assert_eq!(db_to_linear(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn db_round_trips() {
        for db in [-48.0, -12.0, -6.0, -3.0, 0.0, 3.0, 6.0] {
            let round_tripped = linear_to_db(db_to_linear(db));
            assert!(
                (round_tripped - db).abs() < 1e-4,
                "{db} dB round-tripped to {round_tripped}"
            );
        }
    }

    #[test]
    fn non_positive_amplitude_reports_silence() {
        assert_eq!(linear_to_db(0.0), SILENCE_DB);
        assert_eq!(linear_to_db(-1.0), SILENCE_DB);
    }
}

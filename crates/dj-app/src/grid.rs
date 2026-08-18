//! Editing a beat grid the analyser got wrong.
//!
//! # Why the edits live here and not in the engine
//!
//! The grid has two homes: the engine's copy, which sync, quantize and beat
//! jump make decisions from, and the renderer's copy, which is drawn into the
//! waveform tiles. The analyser already writes to both (see
//! `commands::load_track`), and an edit is the same shape of event — a new
//! grid, sent to both destinations.
//!
//! Doing it here rather than on the audio thread means the audio thread keeps
//! no editing state, does no arithmetic it does not need, and the two copies
//! cannot drift: there is exactly one place a grid is decided.
//!
//! # Why the edits are still actions
//!
//! [`dj_core::Command::SetGrid`] is deliberately *not* in the action
//! vocabulary — nobody presses "set grid", it is the analyser reporting a
//! finding. Editing is the opposite: it is a person saying where the beat
//! actually is, and a controller encoder, a script and the assistant should all
//! be able to say it. So the edits are actions, intercepted in `dispatch`
//! before they reach the engine, exactly as `Eject` already is.

use dj_core::{Beatgrid, Bpm, Confidence, DeckId, FramePos, SampleRate};
use std::collections::HashMap;
use std::sync::Mutex;

/// Taps kept per deck.
///
/// Eight is about four seconds of tapping at 120 BPM. Long enough to average
/// out a human's jitter, short enough that a DJ who changes their mind is not
/// fighting a long tail of old taps — and the gap rule below handles the rest.
const MAX_TAPS: usize = 8;

/// A gap longer than this ends a run of taps.
///
/// Two and a half seconds is slower than 24 BPM, which is below anything
/// playable, so a gap this long is not a tempo — it is somebody starting again.
/// Without the rule, a tap from five minutes ago would still be dragging the
/// average.
const TAP_TIMEOUT_SECONDS: f64 = 2.5;

/// The smallest gap that counts as two taps rather than one.
///
/// 400 BPM is [`Bpm::MAX`]; anything faster is a double-strike on the pad or a
/// key repeat, and averaging it in would halve the tempo.
const MIN_TAP_GAP_SECONDS: f64 = 60.0 / Bpm::MAX;

/// Put a beat exactly on `position`, leaving the tempo alone.
///
/// The anchor is just *some* beat, so moving it to the playhead re-phases the
/// whole grid — which is precisely the intent: the tempo was right and the
/// downbeat was not.
#[must_use]
pub fn anchor_here(grid: Beatgrid, position: FramePos) -> Beatgrid {
    Beatgrid {
        anchor: position,
        confidence: Confidence::CERTAIN,
        ..grid
    }
}

/// Slide the whole grid by `millis`, keeping the tempo.
#[must_use]
pub fn nudge(grid: Beatgrid, millis: f64, rate: SampleRate) -> Beatgrid {
    if !millis.is_finite() {
        return grid;
    }
    let frames = millis / 1000.0 * rate.as_f64();
    Beatgrid {
        anchor: FramePos::new(grid.anchor.get() + frames),
        confidence: Confidence::CERTAIN,
        ..grid
    }
}

/// Multiply the tempo, keeping the anchor where it is.
///
/// Returns `None` when the result would be outside [`Bpm`]'s range rather than
/// clamping: a DJ who asks for eight times the tempo has hit the wrong control,
/// and silently giving them four times would be a grid they did not ask for.
#[must_use]
pub fn scale(grid: Beatgrid, factor: f64) -> Option<Beatgrid> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    set_bpm(grid, grid.bpm.get() * factor)
}

/// Set the tempo outright, keeping the anchor.
#[must_use]
pub fn set_bpm(grid: Beatgrid, bpm: f64) -> Option<Beatgrid> {
    Some(Beatgrid {
        bpm: Bpm::new(bpm)?,
        confidence: Confidence::CERTAIN,
        ..grid
    })
}

/// Tap history, one run per deck.
///
/// Taps are recorded as *playhead* positions rather than wall-clock times.
/// That is the whole trick: a DJ taps along to what they are hearing, so the
/// position of the music at each tap is exactly the quantity a grid is made of,
/// and it stays correct when the pitch fader is moved or the deck is paused
/// mid-run.
#[derive(Debug, Default)]
pub struct TapTracker {
    runs: Mutex<HashMap<u8, Vec<FramePos>>>,
}

/// What a tap produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tap {
    /// The first tap of a run. Nothing to measure from yet.
    Started,
    /// Enough taps to state a tempo and a phase.
    Grid(Beatgrid),
    /// Recorded, but the taps do not describe a playable tempo.
    Unusable,
}

impl TapTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a tap at `position` and report what the run says so far.
    ///
    /// `beats_per_bar` is carried over from the existing grid so that tapping
    /// does not silently reset a 3/4 track to 4/4.
    pub fn tap(
        &self,
        deck: DeckId,
        position: FramePos,
        rate: SampleRate,
        beats_per_bar: u8,
    ) -> Tap {
        let Ok(mut runs) = self.runs.lock() else {
            return Tap::Unusable;
        };
        let taps = runs.entry(deck.human_number()).or_default();

        if let Some(&last) = taps.last() {
            let gap = (position.get() - last.get()) / rate.as_f64();
            // Backwards means the DJ seeked or the deck looped, which ends the
            // run for the same reason a long gap does: the interval between
            // those two taps is not a tempo.
            if !(MIN_TAP_GAP_SECONDS..=TAP_TIMEOUT_SECONDS).contains(&gap) {
                taps.clear();
            }
        }

        taps.push(position);
        if taps.len() > MAX_TAPS {
            taps.remove(0);
        }

        let Some(grid) = fit(taps, rate, beats_per_bar) else {
            return if taps.len() < 2 {
                Tap::Started
            } else {
                Tap::Unusable
            };
        };
        Tap::Grid(grid)
    }

    /// Forget a deck's taps. Called on load and on eject, so a run cannot span
    /// two tracks.
    pub fn clear(&self, deck: DeckId) {
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(&deck.human_number());
        }
    }
}

/// Turn a run of taps into a grid.
///
/// The tempo comes from the span between the first and last tap divided by the
/// number of intervals, not from averaging the individual gaps. They are the
/// same number, but the span form makes it obvious why more taps help: the
/// human error at each end is divided by a growing count.
///
/// The phase comes from the *last* tap. It is the one the DJ has had the most
/// practice at by the time it lands, and it is the one nearest the music they
/// are looking at.
fn fit(taps: &[FramePos], rate: SampleRate, beats_per_bar: u8) -> Option<Beatgrid> {
    let (&first, &last) = (taps.first()?, taps.last()?);
    let intervals = taps.len().checked_sub(1)?;
    if intervals == 0 {
        return None;
    }
    let beat_frames = (last.get() - first.get()) / intervals as f64;
    if beat_frames.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let bpm = Bpm::new(rate.as_f64() * 60.0 / beat_frames)?;
    Some(Beatgrid {
        anchor: last,
        bpm,
        beats_per_bar,
        confidence: Confidence::CERTAIN,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn grid(bpm: f64, anchor: f64) -> Beatgrid {
        Beatgrid::new(
            FramePos::new(anchor),
            Bpm::new(bpm).unwrap(),
            // Deliberately weak, so every test also proves an edit makes the
            // grid trustworthy -- which is the reason to edit one.
            Confidence::new(0.2),
        )
    }

    #[test]
    fn anchoring_moves_the_phase_and_leaves_the_tempo() {
        let edited = anchor_here(grid(128.0, 1000.0), FramePos::new(44_100.0));
        assert_eq!(edited.anchor.get(), 44_100.0);
        assert_eq!(edited.bpm.get(), 128.0);
        assert_eq!(edited.confidence, Confidence::CERTAIN);
    }

    #[test]
    fn nudging_moves_by_milliseconds_of_the_devices_own_rate() {
        // 10 ms at the default rate.
        let expected = 10.0 / 1000.0 * SR.as_f64();
        let edited = nudge(grid(128.0, 1000.0), 10.0, SR);
        assert!((edited.anchor.get() - (1000.0 + expected)).abs() < 1e-6);
        let back = nudge(edited, -10.0, SR);
        assert!((back.anchor.get() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn scaling_fixes_an_octave_error_both_ways() {
        let doubled = scale(grid(70.0, 500.0), 2.0).unwrap();
        assert!((doubled.bpm.get() - 140.0).abs() < 1e-9);
        // The anchor is what makes this usable: an octave fix must not move the
        // beat the DJ already lined up.
        assert_eq!(doubled.anchor.get(), 500.0);
        let halved = scale(doubled, 0.5).unwrap();
        assert!((halved.bpm.get() - 70.0).abs() < 1e-9);
    }

    #[test]
    fn a_scale_that_leaves_the_playable_range_is_refused_not_clamped() {
        // 128 x 8 is 1024, past `Bpm::MAX`.
        assert!(scale(grid(128.0, 0.0), 8.0).is_none());
        assert!(scale(grid(128.0, 0.0), 0.0).is_none());
        assert!(scale(grid(128.0, 0.0), -1.0).is_none());
        assert!(scale(grid(128.0, 0.0), f64::NAN).is_none());
    }

    #[test]
    fn a_nan_nudge_leaves_the_grid_alone() {
        let before = grid(128.0, 1000.0);
        assert_eq!(nudge(before, f64::NAN, SR).anchor.get(), 1000.0);
    }

    /// Tap a perfect 120 BPM and read the tempo back.
    #[test]
    fn tapping_in_time_finds_the_tempo_and_the_phase() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        let beat = SR.as_f64() * 0.5; // 120 BPM

        assert_eq!(tracker.tap(deck, FramePos::new(0.0), SR, 4), Tap::Started);
        let mut last = Tap::Started;
        for n in 1..=4 {
            last = tracker.tap(deck, FramePos::new(beat * f64::from(n)), SR, 4);
        }
        let Tap::Grid(found) = last else {
            panic!("four taps must give a grid, got {last:?}");
        };
        assert!(
            (found.bpm.get() - 120.0).abs() < 1e-6,
            "got {}",
            found.bpm.get()
        );
        // Phase from the last tap.
        assert!((found.anchor.get() - beat * 4.0).abs() < 1e-6);
        assert_eq!(found.confidence, Confidence::CERTAIN);
    }

    /// Two taps are enough. A DJ who taps twice and stops has said something.
    #[test]
    fn two_taps_are_already_a_tempo() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(0.0), SR, 4);
        let second = tracker.tap(deck, FramePos::new(SR.as_f64() * 0.5), SR, 4);
        let Tap::Grid(found) = second else {
            panic!("two taps must give a grid, got {second:?}");
        };
        assert!((found.bpm.get() - 120.0).abs() < 1e-6);
    }

    /// The reason the timeout exists: without it, a tap from the intro is still
    /// averaged into a tempo tapped at the drop.
    #[test]
    fn a_long_gap_starts_a_new_run() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(0.0), SR, 4);
        // Ten seconds later -- a different part of the track.
        let restart = tracker.tap(deck, FramePos::new(SR.as_f64() * 10.0), SR, 4);
        assert_eq!(
            restart,
            Tap::Started,
            "a gap past the timeout must start again, not report 6 BPM"
        );
    }

    /// A double-strike on a pad must not halve the tempo.
    #[test]
    fn a_bounced_tap_starts_a_new_run_rather_than_doubling_the_tempo() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(0.0), SR, 4);
        // 1 ms later: 60000 BPM, far past anything playable.
        let bounced = tracker.tap(deck, FramePos::new(SR.as_f64() * 0.001), SR, 4);
        assert_eq!(bounced, Tap::Started);
    }

    /// Seeking backwards mid-run means the interval is not a tempo.
    #[test]
    fn seeking_backwards_ends_the_run() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(SR.as_f64() * 30.0), SR, 4);
        let after_seek = tracker.tap(deck, FramePos::new(SR.as_f64() * 5.0), SR, 4);
        assert_eq!(after_seek, Tap::Started);
    }

    #[test]
    fn taps_are_kept_per_deck() {
        let tracker = TapTracker::new();
        let one = DeckId::from_human(1).unwrap();
        let two = DeckId::from_human(2).unwrap();
        tracker.tap(one, FramePos::new(0.0), SR, 4);
        assert_eq!(
            tracker.tap(two, FramePos::new(SR.as_f64() * 0.5), SR, 4),
            Tap::Started,
            "deck 2's first tap must not be measured against deck 1's"
        );
    }

    #[test]
    fn clearing_ends_the_run() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(0.0), SR, 4);
        tracker.clear(deck);
        assert_eq!(
            tracker.tap(deck, FramePos::new(SR.as_f64() * 0.5), SR, 4),
            Tap::Started,
            "a run must not span two tracks"
        );
    }

    /// Beyond `MAX_TAPS` the oldest drops off, so the tempo follows the taps
    /// rather than being anchored to one from long ago.
    #[test]
    fn the_run_keeps_only_the_recent_taps() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        let beat = SR.as_f64() * 0.5;
        for n in 0..MAX_TAPS + 4 {
            tracker.tap(deck, FramePos::new(beat * n as f64), SR, 4);
        }
        let taps = tracker.runs.lock().unwrap();
        assert_eq!(taps[&1].len(), MAX_TAPS);
    }

    #[test]
    fn tapping_keeps_the_bar_length_it_was_given() {
        let tracker = TapTracker::new();
        let deck = DeckId::from_human(1).unwrap();
        tracker.tap(deck, FramePos::new(0.0), SR, 3);
        let Tap::Grid(found) = tracker.tap(deck, FramePos::new(SR.as_f64() * 0.5), SR, 3) else {
            panic!("expected a grid");
        };
        assert_eq!(found.beats_per_bar, 3, "tapping must not reset 3/4 to 4/4");
    }
}

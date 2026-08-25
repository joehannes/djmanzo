//! Platters that report where they are, not how far they moved.
//!
//! A motorised platter -- a Rane Twelve, a Denon SC6000M, a turntable running
//! timecode -- sends an **absolute angle**, thousands of steps to the
//! revolution, and it wraps: 3599, 0, 1. An ordinary encoder sends a delta and
//! never wraps, which is why [`crate::mapping::Encoding`] cannot be used for
//! this. Reading a wrap as movement would be a full revolution of audio in one
//! frame, every time the platter passes its zero.
//!
//! # Taking the short way round, and why that is safe
//!
//! Two readings can always be read as movement in either direction: 100 to
//! 3500 is either 200 backwards or 3400 forwards. Nothing in the numbers says
//! which. What settles it is arithmetic about the hardware.
//!
//! A platter reports at a few hundred hertz. At playing speed -- 33 1/3 RPM,
//! about 0.56 revolutions a second -- a 200 Hz platter moves 0.003 of a
//! revolution between reports. Even scratched hard, ten times that speed, it
//! moves 0.03. Half a revolution is more than fifteen times further than the
//! wildest hand can take it between two reports, so the short way is not a
//! guess: the long way is physically impossible.
//!
//! That reasoning has a limit, and [`MAX_PLAUSIBLE`] is where it runs out.
//! Past that the reading is not a fast platter, it is a dropped packet or a
//! device that was unplugged and plugged back in mid-turn -- and the honest
//! answer to "how far did it move?" is that we do not know. Reporting zero
//! loses a fraction of a turn; reporting the difference lurches the record.

/// The largest movement between two reports that is taken as real, as a
/// fraction of a revolution.
///
/// A tenth of a turn is about thirty times what a platter covers between
/// reports at playing speed and three times what hard scratching covers, so
/// anything past it is a gap in the reports rather than a movement.
const MAX_PLAUSIBLE: f64 = 0.1;

/// The fewest steps a revolution can usefully be divided into.
///
/// A 7-bit control gives 128, which is nearly three degrees a step -- coarse,
/// but a real thing a cheap controller does. Below that the wrap detection
/// stops being able to tell a movement from a jump.
pub const MIN_RESOLUTION: u32 = 8;

/// A platter that reports its angle.
#[derive(Debug, Clone)]
pub struct AbsolutePlatter {
    /// Steps in one revolution, as the device's manual states it.
    resolution: u32,
    /// The last angle seen. `None` before the first report, and after a jump
    /// that could not be believed.
    last: Option<u32>,
}

impl AbsolutePlatter {
    /// A platter whose revolution is `resolution` steps.
    ///
    /// # Errors
    /// If the resolution is too coarse to tell movement from a wrap.
    pub fn new(resolution: u32) -> Result<Self, PlatterError> {
        if resolution < MIN_RESOLUTION {
            return Err(PlatterError::TooCoarse(resolution));
        }
        Ok(Self {
            resolution,
            last: None,
        })
    }

    #[must_use]
    pub const fn resolution(&self) -> u32 {
        self.resolution
    }

    /// Forget where the platter was.
    ///
    /// For a device going away or a track changing: the next report is a fresh
    /// start rather than a movement from wherever it was last time.
    pub fn reset(&mut self) {
        self.last = None;
    }

    /// How far the platter has turned since the last report, in revolutions.
    ///
    /// Positive is forwards. `0.0` for the first report -- there is nothing to
    /// measure from -- and `0.0` for a jump too large to believe, which is a
    /// gap in the reports rather than a movement.
    ///
    /// An angle past the end of a revolution is folded rather than refused: a
    /// device that counts to its resolution inclusive, or one whose manual is
    /// a step out, should turn the record rather than do nothing.
    pub fn advance(&mut self, angle: u32) -> f32 {
        let steps = self.resolution;
        let angle = angle % steps;

        let Some(last) = self.last.replace(angle) else {
            return 0.0;
        };

        // The short way round, as a signed number of steps.
        let forward = (angle + steps - last) % steps;
        let delta = if forward * 2 > steps {
            f64::from(forward) - f64::from(steps)
        } else {
            f64::from(forward)
        };

        let turns = delta / f64::from(steps);
        if turns.abs() > MAX_PLAUSIBLE {
            // Not a movement. Keep the new angle -- it is where the platter is
            // now, and the next report should measure from here -- but report
            // nothing, because the truth is that we do not know.
            return 0.0;
        }
        turns as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlatterError {
    #[error("a platter of {0} steps a revolution is too coarse to follow")]
    TooCoarse(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEPS: u32 = 3_600;

    fn platter() -> AbsolutePlatter {
        AbsolutePlatter::new(STEPS).expect("3600 steps is a normal platter")
    }

    /// There is nothing to measure the first report against, so it is where
    /// the platter is rather than how far it went.
    ///
    /// Mutation, and the reason the small angle is here: taking the first
    /// report as a movement from zero passes if the only angle tested is a
    /// large one, because the gap guard refuses it anyway. A small angle is
    /// inside what is plausible, so nothing else can catch it.
    #[test]
    fn the_first_report_is_a_starting_point_not_a_movement() {
        let mut small = platter();
        assert_eq!(small.advance(36), 0.0, "a small first angle leaked through");

        let mut far = platter();
        assert_eq!(far.advance(1_234), 0.0);

        // And a platter switched on at zero is the same case.
        let mut origin = platter();
        assert_eq!(origin.advance(0), 0.0);
    }

    #[test]
    fn turning_forwards_reports_forwards() {
        let mut platter = platter();
        platter.advance(0);
        let turns = platter.advance(36);
        assert!((turns - 0.01).abs() < 1e-6, "got {turns}");
    }

    #[test]
    fn turning_back_reports_backwards() {
        let mut platter = platter();
        platter.advance(100);
        let turns = platter.advance(64);
        assert!((turns + 0.01).abs() < 1e-6, "got {turns}");
    }

    /// **The bug this module exists to prevent.** A platter passing zero
    /// reports 3599 then 0. Read as a difference that is minus a whole
    /// revolution -- 1.8 seconds of audio, backwards, every time the record
    /// goes round.
    #[test]
    fn passing_zero_forwards_is_a_small_step_not_a_revolution() {
        let mut platter = platter();
        platter.advance(STEPS - 2);
        let turns = platter.advance(2);
        assert!(
            (turns - 4.0 / f64::from(STEPS) as f32).abs() < 1e-6,
            "crossing zero reported {turns} of a revolution"
        );
    }

    #[test]
    fn passing_zero_backwards_is_a_small_step_too() {
        let mut platter = platter();
        platter.advance(2);
        let turns = platter.advance(STEPS - 2);
        assert!(
            (turns + 4.0 / f64::from(STEPS) as f32).abs() < 1e-6,
            "crossing zero backwards reported {turns}"
        );
    }

    /// A full revolution of real turning, one report at a time, has to add up
    /// to exactly one revolution -- including the report that crosses zero.
    #[test]
    fn a_whole_revolution_adds_up_to_one() {
        let mut platter = platter();
        platter.advance(0);

        let mut total = 0.0f64;
        // Ten steps at a time, which is well inside what is plausible.
        for step in (10..=STEPS).step_by(10) {
            total += f64::from(platter.advance(step % STEPS));
        }

        assert!(
            (total - 1.0).abs() < 1e-4,
            "a revolution added up to {total}"
        );
    }

    /// Several revolutions, forwards then backwards, must come back to zero.
    /// A wrap handled wrongly in one direction only would show up here.
    #[test]
    fn turning_out_and_back_returns_to_where_it_started() {
        let mut platter = platter();
        platter.advance(0);

        let mut total = 0.0f64;
        let mut angle: i64 = 0;
        for _ in 0..3 {
            for _ in 0..360 {
                angle += 10;
                total += f64::from(platter.advance((angle.rem_euclid(3_600)) as u32));
            }
        }
        for _ in 0..3 {
            for _ in 0..360 {
                angle -= 10;
                total += f64::from(platter.advance((angle.rem_euclid(3_600)) as u32));
            }
        }

        assert!(total.abs() < 1e-4, "out and back left {total} of a turn");
    }

    /// **A gap in the reports is not a movement.** A dropped packet, or a
    /// device unplugged mid-turn, is indistinguishable from a huge jump -- and
    /// the honest answer is that we do not know how far it went. Reporting the
    /// difference would lurch the record by up to half a revolution.
    #[test]
    fn a_jump_too_large_to_believe_reports_nothing() {
        let mut platter = platter();
        platter.advance(0);
        // A fifth of a revolution between two reports: twice what hard
        // scratching covers, so this is a gap.
        assert_eq!(platter.advance(720), 0.0);
    }

    /// And the platter carries on from where it actually is, rather than
    /// measuring the next movement from the angle it never reached.
    #[test]
    fn after_a_gap_the_next_movement_is_measured_from_where_it_landed() {
        let mut platter = platter();
        platter.advance(0);
        platter.advance(720);

        let turns = platter.advance(756);
        assert!(
            (turns - 0.01).abs() < 1e-6,
            "the movement after a gap was {turns}, measured from the wrong place"
        );
    }

    /// The boundary itself: just inside is a movement, just outside is a gap.
    /// Getting this backwards would either lurch on a fast scratch or swallow
    /// a real one.
    #[test]
    fn the_boundary_between_a_movement_and_a_gap_is_where_it_says() {
        let inside = (f64::from(STEPS) * MAX_PLAUSIBLE) as u32 - 1;
        let outside = (f64::from(STEPS) * MAX_PLAUSIBLE) as u32 + 2;

        let mut believable = platter();
        believable.advance(0);
        assert!(
            believable.advance(inside) > 0.0,
            "a fast scratch was swallowed"
        );

        let mut not = platter();
        not.advance(0);
        assert_eq!(not.advance(outside), 0.0, "a gap was taken as movement");
    }

    /// Resetting is for a device going away. The next report starts again
    /// rather than measuring from an angle from before it was unplugged.
    #[test]
    fn resetting_makes_the_next_report_a_starting_point() {
        let mut platter = platter();
        platter.advance(0);
        platter.advance(36);

        platter.reset();
        assert_eq!(platter.advance(1_800), 0.0, "it measured across the reset");
    }

    /// A device that counts to its resolution inclusive, or a manual that is a
    /// step out, should still turn the record.
    #[test]
    fn an_angle_past_the_end_is_folded_rather_than_refused() {
        let mut platter = platter();
        platter.advance(0);
        // 3600 on a 3600-step platter is zero again, so this is no movement.
        assert_eq!(platter.advance(STEPS), 0.0);
        // And the one after it is one step, not 3601.
        let turns = platter.advance(STEPS + 1);
        assert!(
            (turns - 1.0 / f64::from(STEPS) as f32).abs() < 1e-6,
            "got {turns}"
        );
    }

    /// A resolution too coarse to tell a movement from a wrap is refused when
    /// the mapping is read, rather than producing nonsense all night.
    #[test]
    fn a_platter_too_coarse_to_follow_is_refused() {
        assert_eq!(
            AbsolutePlatter::new(0).unwrap_err(),
            PlatterError::TooCoarse(0)
        );
        assert_eq!(
            AbsolutePlatter::new(1).unwrap_err(),
            PlatterError::TooCoarse(1)
        );
        assert!(AbsolutePlatter::new(MIN_RESOLUTION).is_ok());
    }

    /// A 7-bit control is a coarse platter but a real one, and the wrap has to
    /// work there too -- it is only 128 steps, so zero comes round often.
    #[test]
    fn a_seven_bit_platter_still_wraps_correctly() {
        let mut platter = AbsolutePlatter::new(128).unwrap();
        platter.advance(126);
        let turns = platter.advance(2);
        assert!(
            (turns - 4.0 / 128.0).abs() < 1e-6,
            "a coarse platter crossing zero reported {turns}"
        );
    }

    /// A platter that has not moved reports nothing, however many times it
    /// says so -- some devices report continuously whether or not anything
    /// changed.
    #[test]
    fn a_platter_that_is_not_moving_reports_nothing() {
        let mut platter = platter();
        platter.advance(500);
        for _ in 0..10 {
            assert_eq!(platter.advance(500), 0.0);
        }
    }
}

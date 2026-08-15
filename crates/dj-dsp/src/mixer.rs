//! Crossfader curves.

/// How the crossfader trades one side for the other.
///
/// The curve is the single most personal setting on a mixer: a scratch DJ wants
/// the opposite side to reach full volume within millimetres of the end stop,
/// while someone doing long blends wants a gentle slope across the whole throw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossfaderCurve {
    /// Constant power. Both sides at ~0.707 in the centre, so a blend of two
    /// uncorrelated tracks holds a steady perceived level. The default for
    /// mixing.
    #[default]
    Smooth,
    /// Linear fade. Sums to unity in the centre -- correct for correlated
    /// material such as the same track on both decks.
    Linear,
    /// Sharp cut. The opposite side is at full volume almost immediately, for
    /// scratching and cutting.
    Sharp,
}

/// Gains for the left and right sides at crossfader position `position`.
///
/// `position` runs from -1.0 (hard left) through 0.0 (centre) to +1.0 (hard
/// right). Returns `(left_gain, right_gain)`, each in `0.0..=1.0`.
#[must_use]
pub fn crossfader_gains(position: f32, curve: CrossfaderCurve) -> (f32, f32) {
    let position = if position.is_finite() {
        position.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    // Remap to 0.0..=1.0, where 0 is fully left.
    let x = (position + 1.0) * 0.5;

    match curve {
        CrossfaderCurve::Linear => (1.0 - x, x),
        CrossfaderCurve::Smooth => {
            // Equal-power: sin/cos quarter turn. Both sides sit at 1/sqrt(2) in
            // the centre, which sums to unity power rather than unity amplitude.
            let angle = x * std::f32::consts::FRAC_PI_2;
            // `cos(FRAC_PI_2)` is -4.37e-8 in f32, not zero. Left unclamped that
            // is a negative gain -- an inverted-phase whisper at the end stop,
            // which is exactly the sort of thing that only shows up as a comb
            // filter when someone runs the same track on both decks.
            (angle.cos().max(0.0), angle.sin().max(0.0))
        }
        CrossfaderCurve::Sharp => {
            // Full volume for all but the last stretch of travel at each end.
            const CUT: f32 = 0.1;
            let left = ((1.0 - x) / CUT).clamp(0.0, 1.0);
            let right = (x / CUT).clamp(0.0, 1.0);
            (left, right)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-5;

    #[test]
    fn hard_left_silences_the_right_side() {
        for curve in [
            CrossfaderCurve::Linear,
            CrossfaderCurve::Smooth,
            CrossfaderCurve::Sharp,
        ] {
            let (left, right) = crossfader_gains(-1.0, curve);
            assert!((left - 1.0).abs() < EPS, "{curve:?}: left should be full");
            assert!(right.abs() < EPS, "{curve:?}: right should be silent");
        }
    }

    #[test]
    fn hard_right_silences_the_left_side() {
        for curve in [
            CrossfaderCurve::Linear,
            CrossfaderCurve::Smooth,
            CrossfaderCurve::Sharp,
        ] {
            let (left, right) = crossfader_gains(1.0, curve);
            assert!(left.abs() < EPS, "{curve:?}: left should be silent");
            assert!((right - 1.0).abs() < EPS, "{curve:?}: right should be full");
        }
    }

    #[test]
    fn centre_is_symmetric_for_every_curve() {
        for curve in [
            CrossfaderCurve::Linear,
            CrossfaderCurve::Smooth,
            CrossfaderCurve::Sharp,
        ] {
            let (left, right) = crossfader_gains(0.0, curve);
            assert!(
                (left - right).abs() < EPS,
                "{curve:?} is asymmetric at centre"
            );
        }
    }

    #[test]
    fn smooth_curve_holds_constant_power() {
        // The point of equal-power: gain_l^2 + gain_r^2 == 1 at every position.
        for step in 0..=20 {
            let position = -1.0 + (step as f32) * 0.1;
            let (left, right) = crossfader_gains(position, CrossfaderCurve::Smooth);
            let power = left * left + right * right;
            assert!(
                (power - 1.0).abs() < EPS,
                "power at {position} was {power}, expected 1.0"
            );
        }
    }

    #[test]
    fn linear_curve_sums_to_unity_amplitude() {
        for step in 0..=20 {
            let position = -1.0 + (step as f32) * 0.1;
            let (left, right) = crossfader_gains(position, CrossfaderCurve::Linear);
            assert!((left + right - 1.0).abs() < EPS);
        }
    }

    #[test]
    fn sharp_curve_opens_fully_near_the_centre() {
        // A cut curve must reach full volume well before the middle, otherwise
        // it is useless for scratching.
        let (_, right) = crossfader_gains(-0.7, CrossfaderCurve::Sharp);
        assert!(
            (right - 1.0).abs() < EPS,
            "sharp curve opened too slowly: {right}"
        );
    }

    #[test]
    fn gains_stay_in_range_across_the_whole_throw() {
        for curve in [
            CrossfaderCurve::Linear,
            CrossfaderCurve::Smooth,
            CrossfaderCurve::Sharp,
        ] {
            for step in 0..=100 {
                let position = -1.0 + (step as f32) * 0.02;
                let (left, right) = crossfader_gains(position, curve);
                assert!((0.0..=1.0).contains(&left), "{curve:?} left out of range");
                assert!((0.0..=1.0).contains(&right), "{curve:?} right out of range");
            }
        }
    }

    #[test]
    fn out_of_range_and_non_finite_input_is_handled() {
        assert_eq!(
            crossfader_gains(-5.0, CrossfaderCurve::Linear),
            crossfader_gains(-1.0, CrossfaderCurve::Linear)
        );
        let (left, right) = crossfader_gains(f32::NAN, CrossfaderCurve::Smooth);
        assert!(left.is_finite() && right.is_finite());
        assert!((left - right).abs() < EPS, "NaN should fall back to centre");
    }
}

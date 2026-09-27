//! §114: the shape of what is in a sampler slot.
//!
//! > i'd also like to see the forms and shapes of the controls and widgets to
//! > be individual and useful and resembling nature and functionality
//!
//! A pad named *kick 03* and one named *vox chop* look the same until they
//! are pressed. The sampler draws each slot's sound instead: how loud it is
//! from its start to its end, in [`POINTS`] steps, with the playhead filling
//! it as it plays. A kick is a spike that falls away, a pad a long plateau, a
//! vocal chop a few humps — read at a glance, and the loudness is the
//! sample's own rather than scaled up, so a quiet sample looks quiet.
//!
//! Measured once, where the sample arrives -- a file read on a worker, or a
//! recording the host has finished -- and never on the audio thread.

/// Each sampler slot's outline, by `(bank, slot)`.
pub type Outlines = std::sync::Mutex<std::collections::HashMap<(u8, u8), Vec<f32>>>;

/// How many steps a slot's outline has: enough for a hump to be a hump, few
/// enough to send once and draw in a row of the sampler.
pub const POINTS: usize = 48;

/// The loudest sample in each of `points` equal stretches of an interleaved
/// stereo buffer, 0..=1, to three places. Empty for an empty buffer.
#[must_use]
pub fn outline(interleaved: &[f32], points: usize) -> Vec<f32> {
    let frames = interleaved.len() / 2;
    if frames == 0 || points == 0 {
        return Vec::new();
    }
    (0..points)
        .map(|i| {
            let from = i * frames / points;
            let to = ((i + 1) * frames / points).clamp(from + 1, frames);
            let peak = interleaved[from * 2..to * 2]
                .iter()
                .fold(0.0_f32, |loudest, sample| loudest.max(sample.abs()));
            let peak = if peak.is_finite() { peak.min(1.0) } else { 0.0 };
            (peak * 1000.0).round() / 1000.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(mono: &[f32]) -> Vec<f32> {
        mono.iter().flat_map(|&s| [s, s]).collect()
    }

    /// **A kick is a spike that falls away**: loudest at the start, each step
    /// no louder than the last, and its level its own.
    #[test]
    fn a_decaying_hit_falls_away() {
        let hit: Vec<f32> = (0..24_000)
            .map(|n| {
                let t = n as f32 / 48_000.0;
                0.8 * (-t * 12.0).exp() * (std::f32::consts::TAU * 60.0 * t).sin()
            })
            .collect();
        let shape = outline(&stereo(&hit), POINTS);
        assert_eq!(shape.len(), POINTS);
        assert!(shape[0] > 0.7, "{shape:?}");
        for pair in shape.windows(2).skip(1) {
            assert!(pair[1] <= pair[0] + 0.01, "{shape:?}");
        }
        assert!(shape[POINTS - 1] < 0.02, "{shape:?}");
    }

    /// A click in the middle shows in its own stretch and nowhere else; the
    /// left and right channels both count.
    #[test]
    fn a_click_shows_where_it_is() {
        let mut buffer = vec![0.0_f32; 48 * 100 * 2];
        buffer[2 * (48 * 50) + 1] = -0.5;
        let shape = outline(&buffer, POINTS);
        let loud: Vec<usize> = (0..POINTS).filter(|&i| shape[i] > 0.0).collect();
        assert_eq!(loud, vec![24]);
        assert!((shape[24] - 0.5).abs() < 1e-6);
    }

    /// A sample shorter than the outline still fills it, silence reads as
    /// nothing, and nothing reads as nothing.
    #[test]
    fn short_silent_and_empty_samples() {
        let shape = outline(&stereo(&[0.25; 10]), POINTS);
        assert_eq!(shape.len(), POINTS);
        assert!(shape.iter().all(|&v| (v - 0.25).abs() < 1e-6));
        assert!(outline(&[0.0; 960], POINTS).iter().all(|&v| v == 0.0));
        assert!(outline(&[], POINTS).is_empty());
        assert_eq!(outline(&stereo(&[2.0, f32::NAN]), 2), vec![1.0, 0.0]);
    }
}

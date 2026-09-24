//! Changing one channel's sample rate, for a model trained at one rate.
//!
//! HT-Demucs was trained on 44.1 kHz audio and its ONNX export takes that
//! rate and no other. A track is decoded at its own rate — 48 kHz is common —
//! and handing the model 48 kHz audio as if it were 44.1 kHz shifts every
//! frequency up by 9 %, a semitone and a half, which is not what it learned
//! vocals and drums from. So the mix is brought to the model's rate and each
//! stem taken back to the track's.
//!
//! A windowed sinc (Blackman), evaluated per output sample, with the cutoff
//! below the lower of the two Nyquist frequencies: the textbook
//! band-limited interpolator, written out because the workspace has no
//! resampler and this one is thirty lines. Not fast, and it does not need to
//! be: two passes over a ten-second chunk are milliseconds against the
//! seconds the model takes over it.

/// Zero crossings of the sinc either side of the centre, at the input rate
/// when upsampling. More is a sharper cutoff and more arithmetic.
const HALF_TAPS: f64 = 24.0;

/// The cutoff as a share of the lower Nyquist frequency: a little under it,
/// so the window's transition band sits below Nyquist rather than across it.
const CUTOFF: f64 = 0.92;

/// `input`, sampled at `from`, resampled to `to`. The output is
/// `round(len × to / from)` samples long, so a chunk taken there and back
/// comes back the length it went.
#[must_use]
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || to == 0 || input.is_empty() {
        return input.to_vec();
    }
    let step = f64::from(from) / f64::from(to);
    let out_len = (input.len() as f64 / step).round() as usize;
    // Cycles per input sample. Downsampling moves the cutoff down to the new
    // Nyquist; the kernel spans the same number of the sinc's zero
    // crossings either way, which are 1 / 2fc input samples apart.
    let fc = 0.5 * CUTOFF * (1.0 / step).min(1.0);
    let half = HALF_TAPS / (2.0 * fc);
    let last = input.len() as isize - 1;

    (0..out_len)
        .map(|j| {
            let t = j as f64 * step;
            let first = (t - half).ceil() as isize;
            let end = (t + half).floor() as isize;
            let mut sum = 0.0f64;
            for k in first.max(0)..=end.min(last) {
                let x = k as f64 - t;
                sum += f64::from(input[k as usize]) * kernel(x, fc, half);
            }
            sum as f32
        })
        .collect()
}

/// The windowed sinc at `x` input samples from the centre.
fn kernel(x: f64, fc: f64, half: f64) -> f64 {
    if x.abs() >= half {
        return 0.0;
    }
    let arg = 2.0 * fc * x;
    let sinc = if arg.abs() < 1e-12 {
        1.0
    } else {
        (std::f64::consts::PI * arg).sin() / (std::f64::consts::PI * arg)
    };
    let phase = std::f64::consts::PI * x / half;
    let blackman = 0.42 + 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos();
    2.0 * fc * sinc * blackman
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f64, rate: u32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|n| {
                (2.0 * std::f64::consts::PI * freq * n as f64 / f64::from(rate)).sin() as f32 * 0.5
            })
            .collect()
    }

    /// Worst difference away from the ends, where the kernel runs off the
    /// buffer and nothing can be exact.
    fn worst(a: &[f32], b: &[f32], edge: usize) -> f32 {
        a.iter()
            .zip(b)
            .skip(edge)
            .take(a.len().min(b.len()) - 2 * edge)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max)
    }

    /// **A tone keeps its pitch and its level** from 48 kHz to 44.1 kHz and
    /// back: the thing a model trained at 44.1 kHz needs from it.
    #[test]
    fn a_tone_keeps_its_pitch_and_level() {
        for freq in [110.0, 1_000.0, 9_000.0] {
            let at_48 = tone(freq, 48_000, 48_000);
            let at_44 = resample(&at_48, 48_000, 44_100);
            assert_eq!(at_44.len(), 44_100);
            let expected = tone(freq, 44_100, 44_100);
            assert!(worst(&at_44, &expected, 200) < 0.003, "{freq} Hz down");

            let back = resample(&at_44, 44_100, 48_000);
            assert_eq!(back.len(), 48_000);
            assert!(worst(&back, &at_48, 200) < 0.003, "{freq} Hz round trip");
        }
    }

    /// Same rate is the same samples, and nothing in is nothing out.
    #[test]
    fn the_same_rate_is_left_alone() {
        let samples = tone(440.0, 44_100, 1_000);
        assert_eq!(resample(&samples, 44_100, 44_100), samples);
        assert!(resample(&[], 48_000, 44_100).is_empty());
    }
}

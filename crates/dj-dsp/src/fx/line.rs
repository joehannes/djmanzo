//! The one buffer every time-based effect needs.

use crate::CHANNELS;

/// A stereo delay line with fractional read positions.
///
/// Owned by the *slot* rather than by the effect, and lent out for the length
/// of a call. Every time-based effect wants the same thing — somewhere to keep
/// the recent past — and giving each of them its own buffer would mean the
/// memory cost of a rack scaled with the number of effects installed rather
/// than the number of slots that can run at once. Switching a slot from echo
/// to flanger then also costs nothing: the buffer is already there, and only
/// the arithmetic reading it changes.
///
/// Allocated once, at the sample rate the engine was opened with. `process`
/// only ever indexes it.
#[derive(Debug)]
pub struct DelayLine {
    buffer: Vec<f32>,
    /// Where the next frame will be written, in frames.
    write: usize,
    /// Length in frames. `buffer.len() / CHANNELS`, kept for the hot path.
    frames: usize,
}

impl DelayLine {
    /// Longest delay a slot can ask for.
    ///
    /// Two seconds is a two-beat echo at 60 BPM and a four-beat echo at 120 —
    /// past the point where an echo is an effect and into the territory of a
    /// second copy of the music. Everything longer belongs to a looper.
    pub const MAX_SECONDS: f32 = 2.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let frames = (sample_rate * Self::MAX_SECONDS).ceil().max(1.0) as usize;
        Self {
            buffer: vec![0.0; frames * CHANNELS],
            write: 0,
            frames,
        }
    }

    #[must_use]
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Forget everything. Called when a slot is switched or bypassed, so an
    /// effect turned back on does not spit out a stale second of audio.
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    /// Write one frame and advance.
    #[inline]
    pub fn push(&mut self, left: f32, right: f32) {
        let at = self.write * CHANNELS;
        self.buffer[at] = left;
        self.buffer[at + 1] = right;
        self.write = (self.write + 1) % self.frames;
    }

    /// Read `delay` frames back, interpolating between neighbours.
    ///
    /// Fractional because a beat-synced delay almost never lands on a whole
    /// number of samples, and rounding to one detunes the echo by up to half a
    /// sample per repeat — audible as a pitch drift on a long feedback tail.
    /// It also means a flanger can sweep its delay smoothly instead of
    /// stepping, which is the whole sound of a flanger.
    #[inline]
    #[must_use]
    pub fn read(&self, delay: f32) -> (f32, f32) {
        let delay = delay.clamp(1.0, self.frames as f32 - 2.0);
        let whole = delay.floor();
        let fraction = delay - whole;

        // `+ frames` before the modulo: the read position is behind the write
        // position and would otherwise go negative.
        let back = whole as usize;
        let first = (self.write + self.frames - back) % self.frames;
        let second = (first + self.frames - 1) % self.frames;

        let a = first * CHANNELS;
        let b = second * CHANNELS;
        (
            self.buffer[a] + (self.buffer[b] - self.buffer[a]) * fraction,
            self.buffer[a + 1] + (self.buffer[b + 1] - self.buffer[a + 1]) * fraction,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn a_frame_comes_back_out_where_it_was_put_in() {
        let mut line = DelayLine::new(SR);
        line.push(1.0, -1.0);
        for _ in 0..99 {
            line.push(0.0, 0.0);
        }
        let (left, right) = line.read(100.0);
        assert!((left - 1.0).abs() < 1e-6, "got {left}");
        assert!((right + 1.0).abs() < 1e-6, "got {right}");
    }

    /// The property that makes a flanger sweep instead of step.
    #[test]
    fn a_fractional_read_lands_between_its_neighbours() {
        let mut line = DelayLine::new(SR);
        line.push(0.0, 0.0);
        line.push(1.0, 1.0);
        // Half a sample between the two most recent frames.
        let (left, _) = line.read(1.5);
        assert!(
            (left - 0.5).abs() < 1e-6,
            "expected the midpoint, got {left}"
        );
    }

    /// A delay longer than the buffer must not read someone else's memory or
    /// panic. Clamping is right here: the slot already limits what it asks for,
    /// and this is the backstop.
    #[test]
    fn an_impossible_delay_is_clamped_rather_than_panicking() {
        let line = DelayLine::new(SR);
        let _ = line.read(f32::MAX);
        let _ = line.read(0.0);
        let _ = line.read(-5.0);
    }

    #[test]
    fn clearing_forgets_the_past() {
        let mut line = DelayLine::new(SR);
        line.push(1.0, 1.0);
        line.clear();
        for _ in 0..10 {
            line.push(0.0, 0.0);
        }
        let (left, right) = line.read(10.0);
        assert_eq!((left, right), (0.0, 0.0));
    }

    /// The line wraps, so writing more than its length must not grow it — the
    /// audio thread cannot allocate.
    #[test]
    fn writing_past_the_end_wraps_instead_of_growing() {
        let mut line = DelayLine::new(1_000.0);
        let capacity = line.buffer.capacity();
        for _ in 0..(line.frames() * 3) {
            line.push(0.5, 0.5);
        }
        assert_eq!(line.buffer.capacity(), capacity);
    }
}

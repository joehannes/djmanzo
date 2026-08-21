//! The boundary between "where audio comes from" and "the engine".

use crate::buffer::{AudioBuffer, CHANNELS};
use dj_core::SampleRate;

/// Something a deck can read frames from.
///
/// This trait is the seam that lets M0 hold whole tracks in memory while M1
/// swaps in a streaming reader that keeps only a window around the playhead.
/// The engine is written against the trait, so that change never reaches it.
///
/// Implementations must be realtime-safe: [`frame_at`](Self::frame_at) is called
/// from the audio callback, so it must not allocate, lock or block. A streaming
/// implementation returns silence for a region it has not yet fetched rather
/// than waiting for it.
pub trait TrackSource: Send + Sync + std::fmt::Debug {
    /// Interpolated frame at a fractional position, or silence out of range.
    fn frame_at(&self, position: f64) -> [f32; CHANNELS];

    fn len_frames(&self) -> usize;

    fn sample_rate(&self) -> SampleRate;

    fn is_empty(&self) -> bool {
        self.len_frames() == 0
    }

    /// Interpolated stem frames at a fractional position, returning 4 stereo frames
    /// (Vocal, Drums, Bass, Other). Returns `None` if the stems for this position
    /// are not yet available, allowing the engine to gracefully fall back to the original mix.
    fn stem_frame_at(&self, _position: f64) -> Option<[[f32; CHANNELS]; 4]> {
        None
    }
}

impl TrackSource for AudioBuffer {
    fn frame_at(&self, position: f64) -> [f32; CHANNELS] {
        self.frame_interpolated(position)
    }

    fn len_frames(&self) -> usize {
        AudioBuffer::len_frames(self)
    }

    fn sample_rate(&self) -> SampleRate {
        AudioBuffer::sample_rate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_buffer_satisfies_the_trait() {
        let buffer = AudioBuffer::from_interleaved(vec![1.0, -1.0, 0.0, 0.0], SampleRate::DEFAULT);
        let source: &dyn TrackSource = &buffer;
        assert_eq!(source.len_frames(), 2);
        assert_eq!(source.frame_at(0.0), [1.0, -1.0]);
        assert!(!source.is_empty());
    }

    #[test]
    fn an_empty_source_reports_empty() {
        let buffer = AudioBuffer::empty();
        let source: &dyn TrackSource = &buffer;
        assert!(source.is_empty());
        assert_eq!(source.frame_at(0.0), [0.0, 0.0]);
    }
}

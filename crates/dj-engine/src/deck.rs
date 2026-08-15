//! A single deck.

use dj_core::{FramePos, Rate, SampleRate, db_to_linear};
use dj_decode::{AudioBuffer, TrackSource};
use dj_dsp::SmoothedValue;
use std::sync::Arc;

/// One player: a source, a playhead, and the gain staging around it.
///
/// Everything here runs on the audio thread. The one rule that shapes the whole
/// type: it never allocates and never drops an `Arc`. Retiring a source is the
/// engine's job.
#[derive(Debug)]
pub struct Deck {
    source: Arc<dyn TrackSource>,
    position: FramePos,
    /// Speed the user asked for, before sample-rate conversion.
    rate: Rate,
    /// Pitch fader as a fraction; 0.0 is centre.
    pitch: f64,
    playing: bool,
    /// Where `cue` returns to.
    cue_point: FramePos,
    /// Channel fader times trim, smoothed so moves do not click.
    channel_gain: SmoothedValue,
    volume: f32,
    gain_db: f32,
    /// Crossfader contribution, smoothed for the same reason.
    crossfader_gain: SmoothedValue,
    /// Rate of the device we are feeding, for sample-rate conversion.
    device_rate: SampleRate,
}

impl Deck {
    #[must_use]
    pub fn new(device_rate: SampleRate) -> Self {
        let sr = device_rate.as_f64() as f32;
        Self {
            source: Arc::new(AudioBuffer::empty()),
            position: FramePos::ZERO,
            rate: Rate::NORMAL,
            pitch: 0.0,
            playing: false,
            cue_point: FramePos::ZERO,
            channel_gain: SmoothedValue::new(1.0, sr),
            volume: 1.0,
            gain_db: 0.0,
            crossfader_gain: SmoothedValue::new(1.0, sr),
            device_rate,
        }
    }

    /// Install a new source, returning the old one for the caller to retire.
    ///
    /// Never drops the displaced `Arc` -- see [`crate::command::Retired`].
    #[must_use]
    pub fn load(&mut self, source: Arc<dyn TrackSource>) -> Arc<dyn TrackSource> {
        let previous = std::mem::replace(&mut self.source, source);
        self.position = FramePos::ZERO;
        self.cue_point = FramePos::ZERO;
        self.playing = false;
        previous
    }

    /// Replace the source with silence, returning the old one to retire.
    #[must_use]
    pub fn eject(&mut self) -> Arc<dyn TrackSource> {
        self.load(Arc::new(AudioBuffer::empty()))
    }

    pub fn play(&mut self) {
        if !self.source.is_empty() {
            self.playing = true;
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle_play(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// CDJ-style cue: stop and return to the cue point.
    pub fn cue(&mut self) {
        self.playing = false;
        self.position = self.cue_point;
    }

    pub fn set_cue_point(&mut self, position: FramePos) {
        self.cue_point = position.clamped(self.len_frames() as f64);
    }

    pub fn seek(&mut self, position: FramePos) {
        self.position = position.clamped(self.len_frames() as f64);
    }

    pub fn set_rate(&mut self, rate: Rate) {
        self.rate = rate;
    }

    /// Pitch fader, as a fraction: `0.08` is +8%.
    pub fn set_pitch(&mut self, pitch: f64) {
        if pitch.is_finite() {
            self.pitch = pitch.clamp(-1.0, 1.0);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if volume.is_finite() {
            self.volume = volume.clamp(0.0, 1.0);
            self.update_channel_gain();
        }
    }

    pub fn set_gain_db(&mut self, db: f32) {
        if db.is_finite() {
            self.gain_db = db.clamp(-24.0, 24.0);
            self.update_channel_gain();
        }
    }

    fn update_channel_gain(&mut self) {
        self.channel_gain
            .set_target(self.volume * db_to_linear(self.gain_db));
    }

    pub fn set_crossfader_gain(&mut self, gain: f32) {
        self.crossfader_gain.set_target(gain);
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        !self.source.is_empty()
    }

    #[must_use]
    pub fn position(&self) -> FramePos {
        self.position
    }

    #[must_use]
    pub fn rate(&self) -> Rate {
        self.rate
    }

    #[must_use]
    pub fn pitch(&self) -> f64 {
        self.pitch
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    #[must_use]
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    #[must_use]
    pub fn len_frames(&self) -> usize {
        self.source.len_frames()
    }

    /// Frames of source consumed per output frame.
    ///
    /// Combines the pitch fader, any directly-set rate, and conversion between
    /// the track's sample rate and the device's. A 44.1 kHz track on a 48 kHz
    /// device must advance at 0.919 frames per output frame or it plays sharp.
    #[must_use]
    fn step_per_output_frame(&self) -> f64 {
        let ratio = self.source.sample_rate().as_f64() / self.device_rate.as_f64();
        self.rate.get() * (1.0 + self.pitch) * ratio
    }

    /// Render into `out`, adding rather than overwriting, and return the peak
    /// level this deck contributed.
    ///
    /// Realtime-safe: no allocation, no locking, no I/O.
    pub fn process(&mut self, out: &mut [f32], channels: usize) -> f32 {
        if !self.playing || self.source.is_empty() {
            // Still advance the smoothers so a fader moved while paused has
            // settled by the time playback resumes.
            self.channel_gain.set_target(self.channel_gain.target());
            return 0.0;
        }

        let step = self.step_per_output_frame();
        let len = self.len_frames() as f64;
        let mut peak = 0.0f32;
        let mut position = self.position.get();

        for frame in out.chunks_exact_mut(channels) {
            let gain = self.channel_gain.next_value() * self.crossfader_gain.next_value();

            if position < 0.0 || position >= len {
                continue;
            }

            let [left, right] = self.source.frame_at(position);
            let left = left * gain;
            let right = right * gain;

            frame[0] += left;
            if channels > 1 {
                frame[1] += right;
            }

            let magnitude = left.abs().max(right.abs());
            if magnitude > peak {
                peak = magnitude;
            }
            position += step;
        }

        // Running off either end stops the transport rather than leaving a
        // silent deck reporting itself as playing.
        if position >= len {
            self.position = FramePos::new(len);
            self.playing = false;
        } else if position < 0.0 {
            self.position = FramePos::ZERO;
            self.playing = false;
        } else {
            self.position = FramePos::new(position);
        }

        peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;

    /// A ramp source: frame `n` has value `n`, so tests can read the position
    /// straight out of the rendered audio.
    fn ramp(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    fn deck_with(frames: usize) -> Deck {
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(frames));
        deck
    }

    #[test]
    fn a_new_deck_is_empty_and_silent() {
        let mut deck = Deck::new(SR);
        assert!(!deck.is_loaded());
        assert!(!deck.is_playing());
        let mut out = vec![0.0; 16];
        assert_eq!(deck.process(&mut out, 2), 0.0);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn an_empty_deck_refuses_to_play() {
        let mut deck = Deck::new(SR);
        deck.play();
        assert!(
            !deck.is_playing(),
            "playing silence is a bug, not a feature"
        );
    }

    #[test]
    fn loading_returns_the_previous_source_for_retirement() {
        let mut deck = Deck::new(SR);
        let first = ramp(10);
        let retired = deck.load(Arc::clone(&first));
        // The empty placeholder comes back, not the new track.
        assert_eq!(retired.len_frames(), 0);

        let second = ramp(20);
        let retired = deck.load(second);
        assert_eq!(
            retired.len_frames(),
            10,
            "must hand back the displaced track"
        );
    }

    #[test]
    fn loading_resets_the_playhead() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(50.0));
        let _ = deck.load(ramp(100));
        assert_eq!(deck.position().get(), 0.0);
        assert!(!deck.is_playing());
    }

    #[test]
    fn playback_advances_one_frame_per_output_frame_at_unity() {
        let mut deck = deck_with(1000);
        deck.play();
        let mut out = vec![0.0; 32]; // 16 frames
        deck.process(&mut out, 2);
        assert!((deck.position().get() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn pitch_scales_the_advance() {
        let mut deck = deck_with(1000);
        deck.set_pitch(0.08);
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames
        deck.process(&mut out, 2);
        assert!(
            (deck.position().get() - 108.0).abs() < 1e-6,
            "+8% pitch should advance 108 frames, got {}",
            deck.position().get()
        );
    }

    /// A 44.1 kHz track on a 48 kHz device must run slower than one frame per
    /// output frame, or it plays sharp. This is the bug that makes everything
    /// sound subtly wrong and is easy to miss by ear.
    #[test]
    fn sample_rate_conversion_is_applied() {
        let source_rate = SampleRate::new(44_100).unwrap();
        let samples: Vec<f32> = (0..1000).flat_map(|n| [n as f32, n as f32]).collect();
        let mut deck = Deck::new(SampleRate::new(48_000).unwrap());
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(
            samples,
            source_rate,
        )));
        deck.play();

        let mut out = vec![0.0; 960]; // 480 output frames
        deck.process(&mut out, 2);
        let expected = 480.0 * (44_100.0 / 48_000.0);
        assert!(
            (deck.position().get() - expected).abs() < 1e-6,
            "expected {expected}, got {}",
            deck.position().get()
        );
    }

    #[test]
    fn a_paused_deck_renders_silence_and_holds_position() {
        let mut deck = deck_with(1000);
        deck.seek(FramePos::new(100.0));
        let mut out = vec![0.0; 32];
        deck.process(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(deck.position().get(), 100.0);
    }

    #[test]
    fn reaching_the_end_stops_the_transport() {
        let mut deck = deck_with(10);
        deck.play();
        let mut out = vec![0.0; 64]; // 32 frames, more than the track has
        deck.process(&mut out, 2);
        assert!(!deck.is_playing(), "deck should stop at the end");
        assert_eq!(deck.position().get(), 10.0);
    }

    #[test]
    fn running_past_the_end_does_not_read_out_of_bounds() {
        // The real safety property: no panic, no garbage, just silence.
        let mut deck = deck_with(4);
        deck.play();
        let mut out = vec![0.0; 200];
        deck.process(&mut out, 2);
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn reverse_playback_stops_at_the_start() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(10.0));
        deck.set_rate(Rate::new(-1.0));
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames of reverse
        deck.process(&mut out, 2);
        assert!(!deck.is_playing());
        assert_eq!(deck.position().get(), 0.0);
    }

    #[test]
    fn cue_returns_to_the_cue_point_and_stops() {
        let mut deck = deck_with(1000);
        deck.set_cue_point(FramePos::new(200.0));
        deck.seek(FramePos::new(500.0));
        deck.play();
        deck.cue();
        assert!(!deck.is_playing());
        assert_eq!(deck.position().get(), 200.0);
    }

    #[test]
    fn seek_is_clamped_to_the_track() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(1e9));
        assert_eq!(deck.position().get(), 100.0);
        deck.seek(FramePos::new(-50.0));
        assert_eq!(deck.position().get(), 0.0);
    }

    #[test]
    fn volume_of_zero_produces_silence() {
        let mut deck = deck_with(10_000);
        deck.set_volume(0.0);
        deck.play();
        // Long enough for the gain ramp to complete.
        let mut out = vec![0.0; 8192];
        deck.process(&mut out, 2);
        let mut out = vec![0.0; 8192];
        deck.process(&mut out, 2);
        let tail = &out[out.len() - 100..];
        assert!(
            tail.iter().all(|&s| s.abs() < 1e-4),
            "volume 0 should be silent once the ramp settles"
        );
    }

    #[test]
    fn gain_settings_are_clamped_to_sane_ranges() {
        let mut deck = Deck::new(SR);
        deck.set_volume(5.0);
        assert_eq!(deck.volume(), 1.0);
        deck.set_volume(-1.0);
        assert_eq!(deck.volume(), 0.0);
        deck.set_gain_db(100.0);
        assert_eq!(deck.gain_db(), 24.0);
    }

    #[test]
    fn non_finite_input_is_ignored() {
        let mut deck = Deck::new(SR);
        deck.set_volume(0.5);
        deck.set_volume(f32::NAN);
        assert_eq!(deck.volume(), 0.5);
        deck.set_pitch(0.1);
        deck.set_pitch(f64::NAN);
        assert!((deck.pitch() - 0.1).abs() < 1e-12);
    }

    #[test]
    fn process_adds_into_the_buffer_rather_than_overwriting() {
        let mut deck = deck_with(1000);
        deck.play();
        let mut out = vec![1.0; 32];
        deck.process(&mut out, 2);
        // Frame 0 of the ramp is 0.0, so the pre-existing 1.0 must survive.
        assert!(out[0] >= 1.0, "deck overwrote existing mix content");
    }

    #[test]
    fn peak_reflects_the_loudest_sample_rendered() {
        let mut deck = Deck::new(SR);
        let samples = vec![0.5f32; 200];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();
        let mut out = vec![0.0; 64];
        let peak = deck.process(&mut out, 2);
        assert!((peak - 0.5).abs() < 0.01, "expected ~0.5, got {peak}");
    }
}

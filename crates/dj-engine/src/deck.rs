//! A single deck.

use crate::bus::BusLayout;
use dj_core::{FramePos, Rate, SampleRate, db_to_linear};
use dj_decode::{AudioBuffer, TrackSource};
use dj_dsp::{SmoothedValue, SweepFilter, ThreeBandEq};
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
    /// Trim, smoothed. Applied before the cue send, so PFL hears the trimmed
    /// signal -- which is the point of having a trim knob at all.
    trim_gain: SmoothedValue,
    /// Channel fader, smoothed. Applied after the cue send.
    fader_gain: SmoothedValue,
    volume: f32,
    gain_db: f32,
    /// Pre-fader listen: send this deck to the headphones.
    cue_enabled: bool,
    /// Crossfader contribution, smoothed for the same reason.
    crossfader_gain: SmoothedValue,
    /// Rate of the device we are feeding, for sample-rate conversion.
    device_rate: SampleRate,
    /// Isolator EQ, one per channel. Filters carry state, so left and right
    /// cannot share an instance.
    eq: [ThreeBandEq; 2],
    filter: [SweepFilter; 2],
    eq_low: f32,
    eq_mid: f32,
    eq_high: f32,
    filter_position: f32,
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
            trim_gain: SmoothedValue::new(1.0, sr),
            fader_gain: SmoothedValue::new(1.0, sr),
            volume: 1.0,
            gain_db: 0.0,
            cue_enabled: false,
            crossfader_gain: SmoothedValue::new(1.0, sr),
            device_rate,
            eq: [ThreeBandEq::new(sr), ThreeBandEq::new(sr)],
            filter: [SweepFilter::new(sr), SweepFilter::new(sr)],
            eq_low: 1.0,
            eq_mid: 1.0,
            eq_high: 1.0,
            filter_position: 0.0,
        }
    }

    pub fn set_eq_low(&mut self, gain: f32) {
        if gain.is_finite() {
            self.eq_low = gain.clamp(0.0, 4.0);
            for eq in &mut self.eq {
                eq.set_low(self.eq_low);
            }
        }
    }

    pub fn set_eq_mid(&mut self, gain: f32) {
        if gain.is_finite() {
            self.eq_mid = gain.clamp(0.0, 4.0);
            for eq in &mut self.eq {
                eq.set_mid(self.eq_mid);
            }
        }
    }

    pub fn set_eq_high(&mut self, gain: f32) {
        if gain.is_finite() {
            self.eq_high = gain.clamp(0.0, 4.0);
            for eq in &mut self.eq {
                eq.set_high(self.eq_high);
            }
        }
    }

    pub fn set_filter(&mut self, position: f32) {
        if position.is_finite() {
            self.filter_position = position.clamp(-1.0, 1.0);
            for filter in &mut self.filter {
                filter.set_position(self.filter_position);
            }
        }
    }

    #[must_use]
    pub fn eq_low(&self) -> f32 {
        self.eq_low
    }

    #[must_use]
    pub fn eq_mid(&self) -> f32 {
        self.eq_mid
    }

    #[must_use]
    pub fn eq_high(&self) -> f32 {
        self.eq_high
    }

    #[must_use]
    pub fn filter_position(&self) -> f32 {
        self.filter_position
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
        // Filter memory belongs to the old track. Carrying it into a new one
        // would leak a fragment of the previous audio into the first samples.
        for eq in &mut self.eq {
            eq.reset();
        }
        for filter in &mut self.filter {
            filter.reset();
        }
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
            self.fader_gain.set_target(self.volume);
        }
    }

    pub fn set_gain_db(&mut self, db: f32) {
        if db.is_finite() {
            self.gain_db = db.clamp(-24.0, 24.0);
            self.trim_gain.set_target(db_to_linear(self.gain_db));
        }
    }

    /// Send this deck to the headphones.
    pub fn set_cue(&mut self, enabled: bool) {
        self.cue_enabled = enabled;
    }

    pub fn toggle_cue(&mut self) {
        self.cue_enabled = !self.cue_enabled;
    }

    #[must_use]
    pub fn is_cued(&self) -> bool {
        self.cue_enabled
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

    /// Render into the interleaved output buffer, adding rather than
    /// overwriting.
    ///
    /// Writes to two buses at different points in the gain chain:
    ///
    /// ```text
    ///   source → EQ → filter → trim ─┬─→ × fader × crossfader → MAIN
    ///                                └─→ (unmodified)          → CUE
    /// ```
    ///
    /// The cue send is taken **before** the channel fader and crossfader, which
    /// is what "pre-fader listen" means and the entire reason PFL is useful: you
    /// cue up the next track with its fader all the way down, hearing it in
    /// headphones while the audience hears nothing.
    ///
    /// Realtime-safe: no allocation, no locking, no I/O.
    pub fn process(&mut self, out: &mut [f32], layout: &BusLayout) -> DeckLevels {
        if !self.playing || self.source.is_empty() {
            return DeckLevels::default();
        }

        let step = self.step_per_output_frame();
        let len = self.len_frames() as f64;
        let mut levels = DeckLevels::default();
        let mut position = self.position.get();
        let channels = layout.channels.max(1);
        let cue_send = if self.cue_enabled { layout.cue } else { None };

        for frame in out.chunks_exact_mut(channels) {
            // Advance the smoothers every frame regardless of whether audio is
            // produced, so a fader moved during a silent stretch has settled by
            // the time sound returns.
            let trim = self.trim_gain.next_value();
            let fader = self.fader_gain.next_value() * self.crossfader_gain.next_value();

            if position < 0.0 || position >= len {
                continue;
            }

            let [left, right] = self.source.frame_at(position);

            // Tone shaping happens before the fader, as on a real mixer: the
            // channel fader must attenuate the EQ'd signal, not the other way
            // round, or riding the fader would change the tone.
            let pre_left = self.filter[0].process(self.eq[0].process(left)) * trim;
            let pre_right = self.filter[1].process(self.eq[1].process(right)) * trim;

            let main_left = pre_left * fader;
            let main_right = pre_right * fader;

            if layout.is_mono() {
                frame[layout.main.0] += (main_left + main_right) * 0.5;
            } else {
                frame[layout.main.0] += main_left;
                frame[layout.main.1] += main_right;
            }

            if let Some((cue_l, cue_r)) = cue_send {
                frame[cue_l] += pre_left;
                frame[cue_r] += pre_right;
            }

            levels.pre_fader = levels.pre_fader.max(pre_left.abs()).max(pre_right.abs());
            levels.post_fader = levels.post_fader.max(main_left.abs()).max(main_right.abs());

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

        levels
    }
}

/// Peak levels a deck produced in one block.
///
/// Both are reported because they answer different questions: pre-fader is what
/// the trim knob should be set by (and what the cue meter shows), post-fader is
/// what actually reaches the master.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DeckLevels {
    pub pre_fader: f32,
    pub post_fader: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;

    /// Plain stereo: master on 0/1, no cue. What most of these tests want.
    fn stereo() -> BusLayout {
        BusLayout::for_channels(2)
    }

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
        assert_eq!(deck.process(&mut out, &stereo()), DeckLevels::default());
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
        let _ = deck.process(&mut out, &stereo());
        assert!((deck.position().get() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn pitch_scales_the_advance() {
        let mut deck = deck_with(1000);
        deck.set_pitch(0.08);
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames
        let _ = deck.process(&mut out, &stereo());
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
        let _ = deck.process(&mut out, &stereo());
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
        let _ = deck.process(&mut out, &stereo());
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(deck.position().get(), 100.0);
    }

    #[test]
    fn reaching_the_end_stops_the_transport() {
        let mut deck = deck_with(10);
        deck.play();
        let mut out = vec![0.0; 64]; // 32 frames, more than the track has
        let _ = deck.process(&mut out, &stereo());
        assert!(!deck.is_playing(), "deck should stop at the end");
        assert_eq!(deck.position().get(), 10.0);
    }

    #[test]
    fn running_past_the_end_does_not_read_out_of_bounds() {
        // The real safety property: no panic, no garbage, just silence.
        let mut deck = deck_with(4);
        deck.play();
        let mut out = vec![0.0; 200];
        let _ = deck.process(&mut out, &stereo());
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn reverse_playback_stops_at_the_start() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(10.0));
        deck.set_rate(Rate::new(-1.0));
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames of reverse
        let _ = deck.process(&mut out, &stereo());
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
        let _ = deck.process(&mut out, &stereo());
        let mut out = vec![0.0; 8192];
        let _ = deck.process(&mut out, &stereo());
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
        let _ = deck.process(&mut out, &stereo());
        // Frame 0 of the ramp is 0.0, so the pre-existing 1.0 must survive.
        assert!(out[0] >= 1.0, "deck overwrote existing mix content");
    }

    #[test]
    fn peak_reflects_the_loudest_sample_rendered() {
        let mut deck = Deck::new(SR);
        // Long enough for the EQ's crossover filters to settle on the step: the
        // 300 Hz band alone is 160 samples per cycle, so a short window would
        // measure the transient rather than the steady state.
        let samples = vec![0.5f32; 40_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &stereo());
        let mut out = vec![0.0; 8_000];
        let peak = deck.process(&mut out, &stereo()).post_fader;

        assert!(
            (peak - 0.5).abs() < 0.02,
            "expected ~0.5 through a flat EQ, got {peak}"
        );
    }

    #[test]
    fn killing_the_eq_low_band_removes_a_bass_tone() {
        use std::f32::consts::PI;

        let frames = 48_000;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let v = (2.0 * PI * 60.0 * n as f32 / 48_000.0).sin() * 0.5;
                [v, v]
            })
            .collect();

        let mut deck = Deck::new(SR);
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.set_eq_low(0.0);
        deck.play();

        // Let the gain ramp and filters settle before measuring.
        let mut out = vec![0.0; 20_000];
        let _ = deck.process(&mut out, &stereo());
        let mut out = vec![0.0; 20_000];
        let peak = deck.process(&mut out, &stereo()).post_fader;

        assert!(
            peak < 0.02,
            "killing the low band should remove a 60 Hz tone, peak was {peak}"
        );
    }

    #[test]
    fn a_flat_eq_and_centred_filter_leave_the_signal_alone() {
        let mut deck = Deck::new(SR);
        let samples = vec![0.4f32; 40_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let mut out = vec![0.0; 16_000];
        let _ = deck.process(&mut out, &stereo());
        let mut out = vec![0.0; 8_000];
        let peak = deck.process(&mut out, &stereo()).post_fader;

        assert!(
            (peak - 0.4).abs() < 0.02,
            "default tone controls should be transparent, got {peak}"
        );
    }

    #[test]
    fn loading_clears_filter_memory() {
        // Filter state from a previous track must not bleed into the next one.
        let mut deck = Deck::new(SR);
        let loud = vec![1.0f32; 20_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(loud, SR)));
        deck.play();
        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &stereo());

        // Swap in silence; the first samples must be silent, not a filter tail.
        let silence = vec![0.0f32; 20_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(silence, SR)));
        deck.play();
        let mut out = vec![0.0; 512];
        let peak = deck.process(&mut out, &stereo()).post_fader;

        assert!(
            peak < 1e-6,
            "previous track bled through the filters: {peak}"
        );
    }
}

#[cfg(test)]
mod cue_tests {
    use super::*;
    use crate::bus::BusLayout;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn tone(frames: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(
            vec![amplitude; frames * 2],
            SR,
        ))
    }

    fn deck_playing() -> Deck {
        let mut deck = Deck::new(SR);
        let _ = deck.load(tone(200_000, 0.5));
        deck.play();
        deck
    }

    /// Render and report peak on the master and cue buses separately.
    fn render(deck: &mut Deck, layout: &BusLayout, frames: usize) -> (f32, f32) {
        let mut out = vec![0.0; frames * layout.channels];
        let _ = deck.process(&mut out, layout);

        let mut main = 0.0f32;
        let mut cue = 0.0f32;
        for frame in out.chunks_exact(layout.channels) {
            main = main.max(frame[layout.main.0].abs());
            if let Some((l, r)) = layout.cue {
                cue = cue.max(frame[l].abs()).max(frame[r].abs());
            }
        }
        (main, cue)
    }

    #[test]
    fn a_deck_is_not_cued_by_default() {
        assert!(!Deck::new(SR).is_cued());
    }

    #[test]
    fn cue_send_is_silent_until_enabled() {
        let mut deck = deck_playing();
        let layout = BusLayout::for_channels(4);
        let (main, cue) = render(&mut deck, &layout, 8_000);
        assert!(main > 0.1, "master should have audio");
        assert_eq!(cue, 0.0, "cue must be silent when PFL is off");
    }

    /// The entire point of pre-fader listen: with the channel fader down, the
    /// audience hears nothing and the DJ still hears the track.
    #[test]
    fn pre_fader_listen_survives_a_closed_fader() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_volume(0.0);
        let layout = BusLayout::for_channels(4);

        // Let the fader ramp reach zero.
        render(&mut deck, &layout, 16_000);
        let (main, cue) = render(&mut deck, &layout, 8_000);

        assert!(
            main < 0.01,
            "fader down means the room hears nothing, got {main}"
        );
        assert!(cue > 0.4, "PFL must still feed the headphones, got {cue}");
    }

    /// Likewise the crossfader: cueing the deck you are about to bring in is
    /// the normal case, and it is always crossfaded away.
    #[test]
    fn pre_fader_listen_survives_the_crossfader() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_crossfader_gain(0.0);
        let layout = BusLayout::for_channels(4);

        render(&mut deck, &layout, 16_000);
        let (main, cue) = render(&mut deck, &layout, 8_000);

        assert!(main < 0.01, "crossfaded away, got {main}");
        assert!(cue > 0.4, "PFL should ignore the crossfader, got {cue}");
    }

    /// Trim is before the cue send, so the headphone level tracks it. That is
    /// what makes trim usable for gain-staging a track before it goes out.
    #[test]
    fn trim_affects_the_cue_send() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_gain_db(-12.0);
        let layout = BusLayout::for_channels(4);

        render(&mut deck, &layout, 16_000);
        let (_, cue) = render(&mut deck, &layout, 8_000);

        // -12 dB is about a quarter amplitude: 0.5 * 0.25 = 0.125.
        assert!(cue < 0.2 && cue > 0.05, "cue should follow trim, got {cue}");
    }

    #[test]
    fn cue_is_dropped_on_a_device_with_no_spare_channels() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        let layout = BusLayout::for_channels(2);
        // Must not panic or write out of bounds on a stereo device.
        let (main, _) = render(&mut deck, &layout, 4_000);
        assert!(main > 0.1);
    }

    #[test]
    fn toggling_cue_flips_it() {
        let mut deck = Deck::new(SR);
        deck.toggle_cue();
        assert!(deck.is_cued());
        deck.toggle_cue();
        assert!(!deck.is_cued());
    }

    #[test]
    fn levels_report_both_sides_of_the_fader() {
        let mut deck = deck_playing();
        deck.set_volume(0.5);
        let layout = BusLayout::for_channels(4);

        let mut out = vec![0.0; 16_000 * 4];
        let _ = deck.process(&mut out, &layout);
        let mut out = vec![0.0; 8_000 * 4];
        let levels = deck.process(&mut out, &layout);

        assert!(
            levels.pre_fader > levels.post_fader,
            "a half-open fader should make post-fader lower: pre {} post {}",
            levels.pre_fader,
            levels.post_fader
        );
        assert!((levels.pre_fader - 0.5).abs() < 0.05);
        assert!((levels.post_fader - 0.25).abs() < 0.05);
    }

    #[test]
    fn mono_output_sums_both_channels() {
        let mut deck = Deck::new(SR);
        // Distinct L and R so summing is observable. Long enough for the EQ's
        // crossovers to settle on the step -- a short window measures their
        // transient overshoot rather than the steady state.
        let samples: Vec<f32> = (0..80_000).flat_map(|_| [0.4f32, 0.2]).collect();
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let layout = BusLayout::for_channels(1);
        let mut settle = vec![0.0; 16_000];
        let _ = deck.process(&mut settle, &layout);

        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &layout);

        let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            (peak - 0.3).abs() < 0.02,
            "mono should be the average of 0.4 and 0.2, got {peak}"
        );
    }
}

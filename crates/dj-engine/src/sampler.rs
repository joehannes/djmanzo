//! Four banks of eight samples, and the pads that fire them.
//!
//! A sample is a deck with almost everything taken away: a source, a playhead,
//! a level. It reuses [`TrackSource`] rather than inventing a second kind of
//! audio, so a sample is loaded, retired and interpolated by exactly the code
//! that loads, retires and interpolates a track — and gets the same fractional
//! reads, which is what lets a sample follow the tempo.
//!
//! Everything here runs on the audio thread. Firing a pad is setting a `bool`
//! and a `f64`; loading a sample happens on the host thread and crosses the
//! same queue a track does, with the displaced buffer handed back to be dropped
//! where dropping is allowed.

use dj_core::{SAMPLE_BANKS, SAMPLE_SLOTS, SampleChange, SampleOutput, TriggerMode};
use dj_decode::{AudioBuffer, TrackSource};
use dj_dsp::SmoothedValue;
use std::sync::Arc;

/// One pad's worth of sampler.
#[derive(Debug)]
pub struct Sample {
    source: Arc<dyn TrackSource>,
    /// Where in the sample we are, in source frames.
    position: f64,
    playing: bool,
    mode: TriggerMode,
    /// Smoothed, so a level moved while a loop is running does not click.
    gain: SmoothedValue,
    volume: f32,
    output: SampleOutput,
    /// Stretch to the master tempo when the sample's own tempo is known.
    synced: bool,
    /// The sample's own tempo, when the analyser found one. `None` is not a
    /// failure — a vocal stab has no tempo, and syncing one would be nonsense.
    bpm: Option<f64>,
}

impl Sample {
    #[must_use]
    fn new(sample_rate: f32) -> Self {
        Self {
            source: Arc::new(AudioBuffer::empty()),
            position: 0.0,
            playing: false,
            mode: TriggerMode::default(),
            gain: SmoothedValue::new(1.0, sample_rate),
            volume: 1.0,
            output: SampleOutput::default(),
            synced: false,
            bpm: None,
        }
    }

    /// Install a sample, handing back whatever was there.
    ///
    /// Returns the old source rather than dropping it: dropping an `Arc` can
    /// free a buffer, and freeing is an allocator call, which is the one thing
    /// this thread may not do.
    #[must_use]
    pub fn load(&mut self, source: Arc<dyn TrackSource>, bpm: Option<f64>) -> Arc<dyn TrackSource> {
        self.playing = false;
        self.position = 0.0;
        self.bpm = bpm;
        std::mem::replace(&mut self.source, source)
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        !self.source.is_empty()
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub fn mode(&self) -> TriggerMode {
        self.mode
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    #[must_use]
    pub fn output(&self) -> SampleOutput {
        self.output
    }

    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.synced
    }

    #[must_use]
    pub fn bpm(&self) -> Option<f64> {
        self.bpm
    }

    /// How far through, 0..=1. Zero when nothing is loaded.
    #[must_use]
    pub fn progress(&self) -> f32 {
        let len = self.source.len_frames() as f64;
        if len <= 0.0 {
            return 0.0;
        }
        (self.position / len).clamp(0.0, 1.0) as f32
    }

    fn trigger(&mut self) {
        if !self.is_loaded() {
            return;
        }
        if self.mode == TriggerMode::Loop && self.playing {
            // The one mode where a second press is a stop. A loop is a thing
            // you switch on, so the pad is a switch.
            self.playing = false;
            return;
        }
        if self.mode.retriggers() || !self.playing {
            self.position = 0.0;
        }
        self.playing = true;
    }

    fn release(&mut self) {
        if self.mode.is_momentary() {
            self.playing = false;
        }
    }

    fn stop(&mut self) {
        self.playing = false;
        self.position = 0.0;
    }

    /// Read one frame, at `step` source frames per output frame.
    #[inline]
    fn next_frame(&mut self, step: f64) -> Option<(f32, f32)> {
        let gain = self.gain.next_value();
        if !self.playing {
            return None;
        }
        let len = self.source.len_frames() as f64;
        if self.position >= len {
            if self.mode == TriggerMode::Loop {
                // Wrapped rather than reset, so a loop whose length is not a
                // whole number of output frames does not drift a sample per
                // pass — which over a long set is an audible slide out of time.
                self.position %= len.max(1.0);
            } else {
                self.playing = false;
                self.position = 0.0;
                return None;
            }
        }
        let [left, right] = self.source.frame_at(self.position);
        self.position += step;
        Some((left * gain, right * gain))
    }

    /// Source frames per output frame, given where the room is.
    ///
    /// One when the sample is not synced or has no tempo of its own. A sample
    /// with no tempo cannot be stretched to a tempo, and pretending otherwise
    /// would resample a vocal stab by whatever ratio happened to be handy.
    #[inline]
    fn step(&self, device_rate: f64, master_bpm: Option<f64>) -> f64 {
        let ratio = self.source.sample_rate().as_f64() / device_rate;
        let stretch = match (self.synced, self.bpm, master_bpm) {
            (true, Some(own), Some(master)) if own > 0.0 && master > 0.0 => master / own,
            _ => 1.0,
        };
        ratio * stretch
    }
}

/// Four banks of eight.
#[derive(Debug)]
pub struct Sampler {
    banks: [[Sample; SAMPLE_SLOTS]; SAMPLE_BANKS],
    /// Which bank the pads are showing, 0-based.
    bank: usize,
    /// The sampler's own level, after every slot's.
    gain: SmoothedValue,
    volume: f32,
    device_rate: f64,
}

impl Sampler {
    #[must_use]
    pub fn new(device_rate: f64) -> Self {
        let sr = device_rate as f32;
        Self {
            banks: std::array::from_fn(|_| std::array::from_fn(|_| Sample::new(sr))),
            bank: 0,
            gain: SmoothedValue::new(1.0, sr),
            volume: 1.0,
            device_rate,
        }
    }

    /// The bank the pads are showing, 1-based.
    #[must_use]
    pub fn bank(&self) -> u8 {
        self.bank as u8 + 1
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// One slot in the *showing* bank, 1-based.
    #[must_use]
    pub fn slot(&self, number: u8) -> Option<&Sample> {
        self.banks[self.bank].get(usize::from(number.checked_sub(1)?))
    }

    /// One slot in a named bank, both 1-based. For loading, which addresses a
    /// bank explicitly so a load cannot land somewhere else because the DJ
    /// switched banks while the file was being read.
    pub fn slot_in_mut(&mut self, bank: u8, number: u8) -> Option<&mut Sample> {
        let bank = self.banks.get_mut(usize::from(bank.checked_sub(1)?))?;
        bank.get_mut(usize::from(number.checked_sub(1)?))
    }

    fn slot_mut(&mut self, number: u8) -> Option<&mut Sample> {
        self.banks[self.bank].get_mut(usize::from(number.checked_sub(1)?))
    }

    /// Whether anything at all is sounding, in any bank.
    ///
    /// Any bank, not just the showing one: a loop keeps running when the DJ
    /// switches away from its bank, which is the point of banks.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.banks
            .iter()
            .all(|bank| bank.iter().all(|sample| !sample.playing))
    }

    pub fn set_bank(&mut self, bank: u8) {
        if let Some(index) = bank.checked_sub(1).map(usize::from)
            && index < SAMPLE_BANKS
        {
            self.bank = index;
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if volume.is_finite() {
            self.volume = volume.clamp(0.0, 1.0);
            self.gain.set_target(self.volume);
        }
    }

    /// Silence everything, everywhere. The panic button.
    pub fn stop_all(&mut self) {
        for bank in &mut self.banks {
            for sample in bank {
                sample.stop();
            }
        }
    }

    /// Apply one change to one slot of the showing bank.
    ///
    /// Returns false for a slot that does not exist. The parser already rejects
    /// those, so this is the second line of defence — but an engine that
    /// indexes an array from a network message wants both.
    pub fn apply(&mut self, number: u8, change: SampleChange) -> bool {
        let Some(sample) = self.slot_mut(number) else {
            return false;
        };
        match change {
            SampleChange::Trigger => sample.trigger(),
            SampleChange::Release => sample.release(),
            SampleChange::Stop => sample.stop(),
            SampleChange::SetMode(mode) => {
                sample.mode = mode;
                // A mode change while it sounds leaves it sounding: the DJ
                // changed how the *next* press behaves, not this one.
            }
            SampleChange::Volume(volume) => {
                if volume.is_finite() {
                    sample.volume = volume.clamp(0.0, 1.0);
                    sample.gain.set_target(sample.volume);
                }
            }
            SampleChange::Route(output) => sample.output = output,
            SampleChange::SetSync(on) => sample.synced = on,
            SampleChange::Clear => {
                sample.stop();
                sample.bpm = None;
            }
        }
        true
    }

    /// Mix every sounding sample into the buses.
    ///
    /// `master_bpm` is what a synced sample stretches to — the same borrowed
    /// tempo the master effect rack uses, for the same reason: the sampler has
    /// no tempo of its own and the room does.
    ///
    /// Frames are the outer loop and slots the inner one, which is the way
    /// round it has to be. The first version had it the other way and the
    /// sampler's own level ended up multiplying the *whole* main bus — decks
    /// included — because by then the decks had already added to it. With
    /// frames outside, the sampler's gain is read once per frame and applied
    /// only to what the sampler put there.
    ///
    /// Returns the peak it produced, for the meter.
    pub fn process(
        &mut self,
        out: &mut [f32],
        layout: &crate::bus::BusLayout,
        master_bpm: Option<f64>,
    ) -> f32 {
        let channels = layout.channels.max(1);
        let frames = out.len() / channels;

        // The smoothers still have to advance through a silence, or a level
        // moved while nothing was playing would arrive all at once on the next
        // pad hit.
        if self.is_idle() {
            for _ in 0..frames {
                let _ = self.gain.next_value();
            }
            for bank in &mut self.banks {
                for sample in bank.iter_mut() {
                    for _ in 0..frames {
                        let _ = sample.gain.next_value();
                    }
                }
            }
            return 0.0;
        }

        // Per-slot playback rate, fixed for the block: a tempo cannot change
        // inside one, and this is a division per slot rather than per frame.
        let device_rate = self.device_rate;
        let steps: [[f64; SAMPLE_SLOTS]; SAMPLE_BANKS] = std::array::from_fn(|bank| {
            std::array::from_fn(|slot| self.banks[bank][slot].step(device_rate, master_bpm))
        });

        let (main_l, main_r) = layout.main;
        let mut peak = 0.0f32;
        for (index, frame) in out.chunks_exact_mut(channels).enumerate() {
            let _ = index;
            let master = self.gain.next_value();
            for (bank_index, bank) in self.banks.iter_mut().enumerate() {
                for (slot_index, sample) in bank.iter_mut().enumerate() {
                    let Some((left, right)) = sample.next_frame(steps[bank_index][slot_index])
                    else {
                        continue;
                    };
                    match sample.output {
                        SampleOutput::Master => {
                            let (left, right) = (left * master, right * master);
                            if layout.is_mono() {
                                frame[main_l] += (left + right) * 0.5;
                            } else {
                                frame[main_l] += left;
                                frame[main_r] += right;
                            }
                            peak = peak.max(left.abs()).max(right.abs());
                        }
                        SampleOutput::Cue => {
                            // Pre the sampler's own level, like a deck's PFL is
                            // pre-fader: auditioning a sample must not depend on
                            // where the sampler fader happens to be.
                            if let Some((cue_l, cue_r)) = layout.cue {
                                frame[cue_l] += left;
                                frame[cue_r] += right;
                            }
                        }
                    }
                }
            }
        }
        peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusLayout;
    use dj_core::SampleRate;

    const SR: f64 = 48_000.0;

    /// A short ramp, so a test can read the position out of the audio.
    fn sample_of(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SampleRate::DEFAULT))
    }

    fn loaded(mode: TriggerMode) -> Sampler {
        let mut sampler = Sampler::new(SR);
        let previous = sampler
            .slot_in_mut(1, 1)
            .unwrap()
            .load(sample_of(1_000), None);
        drop(previous);
        sampler.apply(1, SampleChange::SetMode(mode));
        sampler
    }

    /// Render a block and return the peak of the main bus.
    fn render(sampler: &mut Sampler, frames: usize) -> f32 {
        let layout = BusLayout::for_channels(2);
        let mut out = vec![0.0; frames * 2];
        let _ = sampler.process(&mut out, &layout, None);
        out.iter().fold(0.0f32, |peak, s| peak.max(s.abs()))
    }

    #[test]
    fn an_empty_sampler_is_idle_and_silent() {
        let mut sampler = Sampler::new(SR);
        assert!(sampler.is_idle());
        assert_eq!(render(&mut sampler, 256), 0.0);
    }

    #[test]
    fn a_pad_with_nothing_loaded_does_nothing() {
        let mut sampler = Sampler::new(SR);
        assert!(sampler.apply(1, SampleChange::Trigger));
        assert!(sampler.is_idle(), "an empty slot cannot play");
    }

    #[test]
    fn a_slot_the_sampler_does_not_have_is_refused() {
        let mut sampler = Sampler::new(SR);
        assert!(!sampler.apply(0, SampleChange::Trigger));
        assert!(!sampler.apply(9, SampleChange::Trigger));
        assert!(sampler.slot(0).is_none());
        assert!(sampler.slot(9).is_none());
        assert!(sampler.slot(1).is_some());
    }

    /// The default mode: fire and let it finish, and a release does nothing.
    #[test]
    fn a_one_shot_plays_to_the_end_and_ignores_the_release() {
        let mut sampler = loaded(TriggerMode::OneShot);
        sampler.apply(1, SampleChange::Trigger);
        assert!(sampler.slot(1).unwrap().is_playing());

        sampler.apply(1, SampleChange::Release);
        assert!(
            sampler.slot(1).unwrap().is_playing(),
            "letting go must not stop a one-shot"
        );

        // Long enough to run off the end of a 1000-frame sample.
        let _ = render(&mut sampler, 2_000);
        assert!(
            !sampler.slot(1).unwrap().is_playing(),
            "it should have ended"
        );
    }

    #[test]
    fn a_held_sample_stops_when_it_is_let_go() {
        let mut sampler = loaded(TriggerMode::Hold);
        sampler.apply(1, SampleChange::Trigger);
        let _ = render(&mut sampler, 100);
        assert!(sampler.slot(1).unwrap().is_playing());

        sampler.apply(1, SampleChange::Release);
        assert!(!sampler.slot(1).unwrap().is_playing());
    }

    /// A loop is a switch: press to start, press again to stop.
    #[test]
    fn a_loop_runs_until_it_is_pressed_again() {
        let mut sampler = loaded(TriggerMode::Loop);
        sampler.apply(1, SampleChange::Trigger);

        // Several times the sample's length, so it must have wrapped.
        let _ = render(&mut sampler, 5_000);
        assert!(
            sampler.slot(1).unwrap().is_playing(),
            "a loop should still be running"
        );

        sampler.apply(1, SampleChange::Trigger);
        assert!(!sampler.slot(1).unwrap().is_playing());
    }

    /// The difference between hold and stutter, and the whole reason both
    /// exist: a second press while it is still sounding starts it over.
    #[test]
    fn a_stutter_restarts_on_every_press_and_a_hold_does_not() {
        for (mode, expect_restart) in [(TriggerMode::Stutter, true), (TriggerMode::Hold, false)] {
            let mut sampler = loaded(mode);
            sampler.apply(1, SampleChange::Trigger);
            let _ = render(&mut sampler, 400);
            let part_way = sampler.slot(1).unwrap().progress();
            assert!(part_way > 0.0, "{} should have advanced", mode.name());

            sampler.apply(1, SampleChange::Trigger);
            let after = sampler.slot(1).unwrap().progress();
            if expect_restart {
                assert_eq!(after, 0.0, "{} should have restarted", mode.name());
            } else {
                assert_eq!(after, part_way, "{} should have carried on", mode.name());
            }
        }
    }

    /// A sample routed to the headphones must not reach the room. This is the
    /// property the whole cue path exists for, and getting it backwards means
    /// playing an unchecked sample to the dancefloor.
    #[test]
    fn a_cued_sample_stays_out_of_the_master() {
        let layout = BusLayout::for_channels(4);
        let mut sampler = loaded(TriggerMode::Loop);
        sampler.apply(1, SampleChange::Route(SampleOutput::Cue));
        sampler.apply(1, SampleChange::Trigger);

        let mut out = vec![0.0; 256 * 4];
        let _ = sampler.process(&mut out, &layout, None);

        let (main_l, main_r) = layout.main;
        let master: f32 = out.chunks_exact(4).fold(0.0f32, |peak, f| {
            peak.max(f[main_l].abs()).max(f[main_r].abs())
        });
        assert_eq!(master, 0.0, "a cued sample leaked into the master");

        let (cue_l, _) = layout.cue.expect("four channels means a cue bus");
        let cue: f32 = out
            .chunks_exact(4)
            .fold(0.0f32, |peak, f| peak.max(f[cue_l].abs()));
        assert!(cue > 0.0, "and it should be audible in the headphones");
    }

    /// Loops keep running when the DJ switches away from their bank. That is
    /// what banks are for — a bank is a view, not a mute.
    #[test]
    fn a_loop_keeps_running_when_its_bank_is_switched_away_from() {
        let mut sampler = loaded(TriggerMode::Loop);
        sampler.apply(1, SampleChange::Trigger);
        let _ = render(&mut sampler, 200);

        sampler.set_bank(3);
        assert_eq!(sampler.bank(), 3);
        assert!(!sampler.is_idle(), "bank 1's loop should still be running");
        assert!(render(&mut sampler, 256) > 0.0, "and still audible");
    }

    /// Eight loops running and no way to stop them in one gesture is a sampler
    /// that will one day be the loudest thing in the room.
    #[test]
    fn stop_all_silences_every_bank() {
        let mut sampler = Sampler::new(SR);
        for bank in 1..=SAMPLE_BANKS as u8 {
            let previous = sampler
                .slot_in_mut(bank, 1)
                .unwrap()
                .load(sample_of(1_000), None);
            drop(previous);
        }
        for bank in 1..=SAMPLE_BANKS as u8 {
            sampler.set_bank(bank);
            sampler.apply(1, SampleChange::SetMode(TriggerMode::Loop));
            sampler.apply(1, SampleChange::Trigger);
        }
        assert!(!sampler.is_idle());

        sampler.stop_all();
        assert!(sampler.is_idle(), "the panic button left something running");
    }

    /// A sample with no tempo of its own cannot be stretched to one. Pretending
    /// otherwise would resample a vocal stab by whatever ratio was handy.
    #[test]
    fn a_sample_with_no_tempo_is_never_stretched() {
        let mut sampler = Sampler::new(SR);
        let previous = sampler
            .slot_in_mut(1, 1)
            .unwrap()
            .load(sample_of(1_000), None);
        drop(previous);
        sampler.apply(1, SampleChange::SetSync(true));

        let sample = sampler.slot(1).unwrap();
        assert_eq!(sample.step(SR, Some(174.0)), 1.0);
    }

    /// And one that does have a tempo follows the room.
    #[test]
    fn a_synced_sample_stretches_to_the_master_tempo() {
        let mut sampler = Sampler::new(SR);
        let previous = sampler
            .slot_in_mut(1, 1)
            .unwrap()
            .load(sample_of(1_000), Some(120.0));
        drop(previous);
        sampler.apply(1, SampleChange::SetSync(true));

        let sample = sampler.slot(1).unwrap();
        // Playing into a 140 BPM mix, a 120 BPM sample has to run faster.
        assert!((sample.step(SR, Some(140.0)) - 140.0 / 120.0).abs() < 1e-9);
        // And with sync off it runs at its own speed however fast the room is.
        let sample_rate_ratio = 1.0;
        assert_eq!(
            Sampler::new(SR).slot(1).unwrap().step(SR, Some(140.0)),
            sample_rate_ratio
        );
    }

    /// Loading hands the old buffer back rather than dropping it, because
    /// dropping an `Arc` can free memory and this runs where freeing is banned.
    #[test]
    fn loading_returns_the_previous_sample_rather_than_dropping_it() {
        let mut sampler = Sampler::new(SR);
        let first = sample_of(100);
        let previous = sampler.slot_in_mut(1, 1).unwrap().load(first, None);
        assert!(previous.is_empty(), "the slot started empty");

        let second = sample_of(200);
        let displaced = sampler.slot_in_mut(1, 1).unwrap().load(second, None);
        assert_eq!(displaced.len_frames(), 100, "the first one came back");
    }

    /// Loading a slot must not leave the previous sample playing over the top
    /// of the new one.
    #[test]
    fn loading_over_a_playing_slot_stops_it() {
        let mut sampler = loaded(TriggerMode::Loop);
        sampler.apply(1, SampleChange::Trigger);
        assert!(!sampler.is_idle());

        let displaced = sampler.slot_in_mut(1, 1).unwrap().load(sample_of(50), None);
        drop(displaced);
        assert!(sampler.is_idle(), "the old sample kept playing");
    }
}

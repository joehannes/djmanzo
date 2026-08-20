//! The microphone and line input.
//!
//! # Why it is a channel rather than a deck
//!
//! A microphone has no playhead, no tempo and nothing to cue. What it has is a
//! level, a switch, and the ability to make the music get out of its way. So it
//! is a mixer strip: gain, on/off, a send to the headphones, and a ducker
//! sidechained from itself.
//!
//! An aux — a phone, a second laptop, a guitar amp — is the same strip with
//! talkover switched off, which is why there is one type here and not two.
//!
//! # Where the samples come from
//!
//! The operating system delivers input on a callback of its own, which is a
//! different thread from the one that renders the master. So the host owns the
//! sending half of a lock-free ring and the engine holds this end, exactly as
//! [`crate::command::Command::RecordStream`] does in the other direction.
//!
//! That ring is also where the latency lives. A microphone routed through a
//! computer is late by the input buffer plus the output buffer plus whatever
//! sits in the ring, and no arrangement of software makes it not so. A DJ who
//! can hear themselves in the monitors will hear the delay; the honest answer
//! is a small buffer, and saying so.
//!
//! # What happens when the ring runs dry
//!
//! Silence for that frame, and a counter goes up. Not a stall and not a retry:
//! the master has a deadline, and a microphone that is briefly not there is a
//! gap in one channel rather than a reason to miss it. The count is published
//! so the interface can say the input is not keeping up, which is a real fault
//! with a real fix — a larger buffer — and invisible without it.
//!
//! # Realtime rules
//!
//! Everything is sized at construction. `next_frame` is two ring pops and
//! arithmetic. The consumer itself is never dropped here: it leaves through the
//! retirement queue like every other buffer the engine parts with.

use dj_dsp::{Ducker, PeakMeter, SmoothedValue};

/// Channels carried on the input ring. Interleaved stereo, always — a mono
/// microphone is doubled on the way in, so the engine has one shape to handle
/// rather than a branch on every frame.
pub const CHANNELS: usize = 2;

/// One frame of input, and what it does to the music.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MicFrame {
    pub left: f32,
    pub right: f32,
    /// The gain to apply to everything that is *not* the microphone.
    ///
    /// Returned rather than applied because the mic does not own the music.
    /// See [`dj_dsp::Ducker`].
    pub music_gain: f32,
}

/// The microphone / line input strip.
#[derive(Debug)]
pub struct Mic {
    /// The receiving end of the host's input ring. `None` when no input device
    /// is open, which is the normal state.
    input: Option<rtrb::Consumer<f32>>,
    gain: SmoothedValue,
    gain_db: f32,
    ducker: Ducker,
    meter: PeakMeter,
    /// Whether the channel is open. Distinct from having an input device:
    /// closing the microphone must silence it instantly and leave the device
    /// alone, because a DJ closes and opens it dozens of times a night and
    /// re-opening a sound card takes long enough to miss a cue.
    open: bool,
    /// Whether the microphone is also sent to the headphones, so a DJ can hear
    /// themselves without depending on a monitor.
    to_cue: bool,
    /// Frames the ring could not supply. See the module note.
    starved: u64,
}

impl Mic {
    /// Where the fader starts, in decibels. Unity: a mic preamp's own gain is
    /// what sets the level, and a channel that started boosted would feed back.
    pub const DEFAULT_GAIN_DB: f32 = 0.0;
    /// The range the fader covers.
    pub const MIN_GAIN_DB: f32 = -60.0;
    pub const MAX_GAIN_DB: f32 = 12.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Mic {
            input: None,
            gain: SmoothedValue::new(db_to_linear(Self::DEFAULT_GAIN_DB), sample_rate),
            gain_db: Self::DEFAULT_GAIN_DB,
            ducker: Ducker::new(sample_rate),
            meter: PeakMeter::new(sample_rate),
            open: false,
            to_cue: false,
            starved: 0,
        }
    }

    /// Install an input, handing back whatever was there.
    ///
    /// The old consumer is returned rather than dropped: dropping it releases a
    /// share of the ring, and if the host has already gone that is a `free()`
    /// on the audio thread.
    pub fn set_input(&mut self, input: Option<rtrb::Consumer<f32>>) -> Option<rtrb::Consumer<f32>> {
        // A new device means the old envelope describes a signal that no longer
        // exists, and carrying it over would duck the music against a room
        // nobody is in any more.
        self.ducker.reset();
        self.meter.reset();
        self.starved = 0;
        match input {
            Some(input) => self.input.replace(input),
            None => self.input.take(),
        }
    }

    /// Whether an input device is attached at all.
    #[must_use]
    pub fn has_input(&self) -> bool {
        self.input.is_some()
    }

    /// Open or close the channel.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        if !open {
            // The meter must fall to zero rather than freezing at the last
            // reading, or a closed microphone shows a level and looks live.
            self.meter.reset();
        }
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_gain_db(&mut self, db: f32) {
        self.gain_db = db.clamp(Self::MIN_GAIN_DB, Self::MAX_GAIN_DB);
        self.gain.set_target(db_to_linear(self.gain_db));
    }

    #[must_use]
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    pub fn set_to_cue(&mut self, to_cue: bool) {
        self.to_cue = to_cue;
    }

    #[must_use]
    pub fn to_cue(&self) -> bool {
        self.to_cue
    }

    /// The ducker, for the actions that configure it.
    pub fn ducker_mut(&mut self) -> &mut Ducker {
        &mut self.ducker
    }

    #[must_use]
    pub fn ducker(&self) -> &Ducker {
        &self.ducker
    }

    /// Peak level of the microphone after its gain, for the meter.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.meter.peak()
    }

    /// Frames the input ring could not supply since the device was opened.
    #[must_use]
    pub fn starved_frames(&self) -> u64 {
        self.starved
    }

    /// One frame of input, and the gain the music should be given.
    ///
    /// Always called, open or closed, because the ring has to be drained either
    /// way: a closed microphone whose ring filled up would deliver several
    /// seconds of stale room noise the moment it was opened again.
    pub fn next_frame(&mut self) -> MicFrame {
        let (mut left, mut right) = self.pull();

        // Applied whether or not the channel is open, so the smoothing is
        // always in step and opening the microphone does not begin with a ramp
        // from wherever the fader happened to be left.
        let gain = self.gain.next_value();
        left *= gain;
        right *= gain;

        if !self.open {
            // Closed: silent, and the ducker gets nothing to chew on. The
            // gain still eases back to unity at the release rate rather than
            // stepping, which is what `Ducker::process_frame(0.0)` does.
            return MicFrame {
                left: 0.0,
                right: 0.0,
                music_gain: self.ducker.process_frame(0.0),
            };
        }

        // The sidechain is the microphone *after* its gain, so turning the mic
        // up makes it duck more readily — which is what the control on a mixer
        // does and therefore what a DJ expects.
        let sidechain = if left.abs() > right.abs() {
            left
        } else {
            right
        };
        let music_gain = self.ducker.process_frame(sidechain);
        self.meter.process(&[left, right]);

        MicFrame {
            left,
            right,
            music_gain,
        }
    }

    /// One frame off the ring, or silence and a mark against it.
    fn pull(&mut self) -> (f32, f32) {
        let Some(input) = self.input.as_mut() else {
            return (0.0, 0.0);
        };
        // Both halves or neither: taking one sample of a stereo frame would put
        // the ring permanently out of phase, so every later frame has the left
        // channel of one and the right of the next.
        if input.slots() < CHANNELS {
            self.starved += 1;
            return (0.0, 0.0);
        }
        let (Ok(left), Ok(right)) = (input.pop(), input.pop()) else {
            self.starved += 1;
            return (0.0, 0.0);
        };
        (left, right)
    }
}

fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// A mic with a ring already holding `samples`, interleaved.
    fn wired(samples: &[f32]) -> (Mic, rtrb::Producer<f32>) {
        let (mut producer, consumer) = rtrb::RingBuffer::new(samples.len().max(64) * 2);
        for sample in samples {
            producer.push(*sample).unwrap();
        }
        let mut mic = Mic::new(RATE);
        assert!(mic.set_input(Some(consumer)).is_none());
        (mic, producer)
    }

    fn frames(mic: &mut Mic, count: usize) -> Vec<MicFrame> {
        (0..count).map(|_| mic.next_frame()).collect()
    }

    #[test]
    fn a_closed_microphone_is_silent() {
        let (mut mic, _p) = wired(&[0.5; 64]);
        let out = frames(&mut mic, 8);
        assert!(out.iter().all(|f| f.left == 0.0 && f.right == 0.0));
        assert!(out.iter().all(|f| f.music_gain == 1.0));
    }

    #[test]
    fn an_open_microphone_passes_what_it_is_given() {
        let (mut mic, _p) = wired(&[0.5, -0.25, 0.5, -0.25]);
        mic.set_open(true);
        let first = mic.next_frame();
        assert!((first.left - 0.5).abs() < 1e-3);
        assert!((first.right - (-0.25)).abs() < 1e-3);
    }

    /// **The reason `pull` takes both halves or neither.** A ring holding an
    /// odd number of samples must not leave the channels swapped from then on.
    #[test]
    fn a_half_frame_in_the_ring_does_not_swap_the_channels() {
        let (mut mic, mut producer) = wired(&[]);
        mic.set_open(true);
        // One lonely sample: not a frame.
        producer.push(1.0).unwrap();
        let starved = mic.next_frame();
        assert_eq!((starved.left, starved.right), (0.0, 0.0));
        assert_eq!(mic.starved_frames(), 1);

        // Its partner arrives; now there is a whole frame, in the right order.
        producer.push(-1.0).unwrap();
        let whole = mic.next_frame();
        assert!((whole.left - 1.0).abs() < 1e-3, "{whole:?}");
        assert!((whole.right - (-1.0)).abs() < 1e-3, "{whole:?}");
    }

    /// A microphone that is briefly not there is a gap in one channel, not a
    /// reason for the master to miss its deadline.
    #[test]
    fn an_empty_ring_is_silence_and_a_count() {
        let (mut mic, _p) = wired(&[]);
        mic.set_open(true);
        let out = frames(&mut mic, 10);
        assert!(out.iter().all(|f| f.left == 0.0));
        assert_eq!(mic.starved_frames(), 10);
    }

    #[test]
    fn no_input_device_is_silence_without_a_starve_count() {
        let mut mic = Mic::new(RATE);
        mic.set_open(true);
        let out = frames(&mut mic, 10);
        assert!(out.iter().all(|f| f.left == 0.0));
        assert_eq!(
            mic.starved_frames(),
            0,
            "no device is not the same fault as a device that cannot keep up"
        );
    }

    /// **A closed channel still drains.** Otherwise the ring fills while the
    /// microphone is off and opening it plays several seconds of stale room.
    #[test]
    fn a_closed_channel_still_drains_the_ring() {
        let (mut mic, _p) = wired(&[0.5; 32]);
        frames(&mut mic, 8);
        mic.set_open(true);
        let first = mic.next_frame();
        // Frame 9 of the ring, not frame 1 — the first eight were consumed
        // while it was closed.
        assert!((first.left - 0.5).abs() < 1e-3);
        // 32 samples is 16 frames; 8 went while closed, this was the 9th.
        assert_eq!(mic.starved_frames(), 0);
        frames(&mut mic, 7);
        assert_eq!(
            mic.starved_frames(),
            0,
            "the ring should have held 16 frames"
        );
        mic.next_frame();
        assert_eq!(mic.starved_frames(), 1, "and then run out");
    }

    /// The gain ramp is a one-pole, so it approaches its target rather than
    /// arriving at it. `SmoothedValue` snaps once it is within its own epsilon,
    /// which takes many time constants — so this runs long enough for that to
    /// happen rather than asserting a tolerance chosen to make it pass.
    #[test]
    fn gain_moves_the_level() {
        let (mut mic, _p) = wired(&vec![1.0; 65_536]);
        mic.set_open(true);
        mic.set_gain_db(-6.0);
        let out = frames(&mut mic, 32_768);
        let last = out.last().unwrap();
        assert!((last.left - db_to_linear(-6.0)).abs() < 1e-4, "{last:?}");
        assert_eq!(last.left, last.right, "a mono source stays centred");
    }

    #[test]
    fn gain_is_clamped_to_the_range_the_fader_covers() {
        let mut mic = Mic::new(RATE);
        mic.set_gain_db(400.0);
        assert_eq!(mic.gain_db(), Mic::MAX_GAIN_DB);
        mic.set_gain_db(-400.0);
        assert_eq!(mic.gain_db(), Mic::MIN_GAIN_DB);
    }

    /// Turning the microphone up must make it duck more readily, because that
    /// is what the control on a mixer does.
    #[test]
    fn the_sidechain_is_taken_after_the_gain() {
        // A signal well under the ducker's threshold on its own.
        let quiet = db_to_linear(-40.0);
        let (mut mic, _p) = wired(&vec![quiet; 8192]);
        mic.set_open(true);
        let out = frames(&mut mic, 2048);
        assert!(
            out.last().unwrap().music_gain == 1.0,
            "a quiet room should not duck the music"
        );

        let (mut loud, _p) = wired(&vec![quiet; 8192]);
        loud.set_open(true);
        loud.set_gain_db(12.0);
        // +12 dB puts -40 at -28, over the -30 dB threshold.
        let out = frames(&mut loud, 4096);
        assert!(
            out.last().unwrap().music_gain < 1.0,
            "the gain should have pushed it over the threshold"
        );
    }

    #[test]
    fn a_new_device_forgets_the_old_room() {
        let (mut mic, _p) = wired(&vec![0.8; 4096]);
        mic.set_open(true);
        frames(&mut mic, 1024);
        assert!(mic.ducker().is_ducking());
        assert!(mic.level() > 0.0);

        let (_p2, consumer) = rtrb::RingBuffer::<f32>::new(64);
        let old = mic.set_input(Some(consumer));
        assert!(old.is_some(), "the previous input must come back, not drop");
        assert!(!mic.ducker().is_ducking());
        assert_eq!(mic.level(), 0.0);
        assert_eq!(mic.starved_frames(), 0);
    }

    /// The engine must never drop a ring consumer: dropping it can be the last
    /// reference, and freeing on the audio thread is what the whole retirement
    /// queue exists to prevent.
    #[test]
    fn removing_an_input_hands_it_back() {
        let (mut mic, _p) = wired(&[]);
        assert!(mic.has_input());
        assert!(mic.set_input(None).is_some());
        assert!(!mic.has_input());
        assert!(mic.set_input(None).is_none());
    }

    /// A closed microphone showing a level looks live, which in a booth is the
    /// difference between speaking and being heard speaking.
    #[test]
    fn closing_the_channel_drops_the_meter() {
        let (mut mic, _p) = wired(&vec![0.9; 4096]);
        mic.set_open(true);
        frames(&mut mic, 512);
        assert!(mic.level() > 0.5);
        mic.set_open(false);
        assert_eq!(mic.level(), 0.0);
    }

    #[test]
    fn the_headphone_send_is_off_until_asked_for() {
        let mut mic = Mic::new(RATE);
        assert!(!mic.to_cue());
        mic.set_to_cue(true);
        assert!(mic.to_cue());
    }
}

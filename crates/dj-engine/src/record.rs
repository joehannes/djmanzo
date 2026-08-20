//! Capturing audio into a sampler slot.
//!
//! # The shape of the problem
//!
//! Recording needs somewhere to put the audio, and the audio thread may not
//! allocate one — so the buffer is allocated on the host thread, sent in, filled
//! here, and sent back out full. Between handing one back and being given the
//! next, the recorder has nowhere to write and says so rather than pretending.
//! That is the same discipline the retirement queue already uses for displaced
//! track buffers, extended from "hand this back to be dropped" to "hand this
//! back to be *used*".
//!
//! Everything in this module runs on the audio thread except [`Capture`], which
//! only travels through it.

use dj_core::{RecordSource, SampleRate};

/// A finished recording, on its way to the host thread.
///
/// Carries the `Vec` itself rather than a copy of it: moving a `Vec` across the
/// queue is three words, and the alternative — copying the samples somewhere —
/// is precisely the work that may not happen here.
#[derive(Debug)]
pub struct Capture {
    pub bank: u8,
    pub slot: u8,
    pub source: RecordSource,
    /// Interleaved stereo at the device rate. Longer than the recording: the
    /// tail past `frames` is the silence the buffer arrived with.
    pub samples: Vec<f32>,
    /// Frames actually recorded.
    pub frames: usize,
    pub sample_rate: SampleRate,
    /// What the room was running at when this was captured, when it was running
    /// at anything. Stamped so the sample can be synced later without being
    /// analysed again — a recording of a mix is at the tempo of that mix, and
    /// that is a fact worth keeping rather than rediscovering.
    pub bpm: Option<f64>,
}

/// Where a capture is in its life.
///
/// Three states rather than a pair of booleans, because "running and also
/// finished" is not a thing that can happen and a type that cannot express it
/// is one fewer thing to get wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Running,
    /// Complete, waiting for the host thread to collect it.
    Ready,
}

/// The tap and the buffer behind it.
#[derive(Debug)]
pub struct Recorder {
    /// Somewhere to write. `None` between handing a capture back and being
    /// given the next buffer, which is a real state the interface can see.
    space: Option<Vec<f32>>,
    /// Frames written so far.
    written: usize,
    state: State,
    source: RecordSource,
    bank: u8,
    slot: u8,
    sample_rate: SampleRate,
    /// Stamped at the start rather than the end: the tempo that matters is the
    /// one the recording was made at, and a DJ who changes the pitch mid-capture
    /// has already made the sample's tempo a fiction either way.
    bpm: Option<f64>,
}

impl Recorder {
    #[must_use]
    pub fn new(sample_rate: SampleRate) -> Self {
        Self {
            space: None,
            written: 0,
            state: State::Idle,
            source: RecordSource::default(),
            bank: 1,
            slot: 1,
            sample_rate,
            bpm: None,
        }
    }

    /// Install somewhere to record, handing back whatever was there.
    ///
    /// Returns the displaced buffer for the same reason [`crate::sampler::Sample::load`]
    /// returns the displaced source: dropping a `Vec` here is a `free()` on the
    /// audio thread.
    #[must_use]
    pub fn give_space(&mut self, space: Vec<f32>) -> Option<Vec<f32>> {
        // Refused while a capture is running, or the DJ would be recording into
        // one buffer and collecting another.
        if self.state == State::Running {
            return Some(space);
        }
        self.space.replace(space)
    }

    /// Whether there is a buffer to record into.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.space.is_some()
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == State::Running
    }

    /// Which tap to take, while running. `None` means take none.
    ///
    /// The engine asks this rather than being told, so a source that is not
    /// being recorded costs one comparison per block rather than anything per
    /// frame.
    #[must_use]
    pub fn tapping(&self) -> Option<RecordSource> {
        (self.state == State::Running).then_some(self.source)
    }

    #[must_use]
    pub fn frames(&self) -> usize {
        self.written
    }

    #[must_use]
    pub fn seconds(&self) -> f32 {
        (self.written as f64 / self.sample_rate.as_f64().max(1.0)) as f32
    }

    /// The slot being recorded into, 1-based, while running.
    #[must_use]
    pub fn slot(&self) -> Option<u8> {
        (self.state == State::Running).then_some(self.slot)
    }

    /// Begin. Returns whether it did.
    ///
    /// Refuses without a buffer, and refuses while another capture is running
    /// or waiting to be collected — one recorder, one recording.
    pub fn start(&mut self, bank: u8, slot: u8, source: RecordSource, bpm: Option<f64>) -> bool {
        if self.state != State::Idle || self.space.is_none() {
            return false;
        }
        self.bank = bank;
        self.slot = slot;
        self.source = source;
        self.bpm = bpm;
        self.written = 0;
        self.state = State::Running;
        true
    }

    /// Stop and keep what was captured.
    ///
    /// A capture of nothing is thrown away rather than landing an empty sample
    /// in a slot: pressing record and stop in the same gesture is a mistake, and
    /// wiping a loaded slot with silence is not a helpful response to it.
    pub fn stop(&mut self) {
        if self.state != State::Running {
            return;
        }
        self.state = if self.written > 0 {
            State::Ready
        } else {
            State::Idle
        };
    }

    /// Stop and throw it away.
    pub fn cancel(&mut self) {
        if self.state == State::Running {
            self.written = 0;
            self.state = State::Idle;
        }
    }

    /// Write one stereo frame.
    ///
    /// Stops itself when the buffer fills. A recorder that silently wrapped
    /// would hand back a sample whose beginning is its end.
    #[inline]
    pub fn write(&mut self, left: f32, right: f32) {
        if self.state != State::Running {
            return;
        }
        let Some(space) = self.space.as_mut() else {
            return;
        };
        let at = self.written * 2;
        if at + 1 >= space.len() {
            self.state = if self.written > 0 {
                State::Ready
            } else {
                State::Idle
            };
            return;
        }
        space[at] = left;
        space[at + 1] = right;
        self.written += 1;
    }

    /// Collect a finished capture, if there is one.
    ///
    /// Takes the buffer with it, which is why [`Self::is_ready`] goes false
    /// afterwards: the host has to send another before the next recording.
    #[must_use]
    pub fn take(&mut self) -> Option<Capture> {
        if self.state != State::Ready {
            return None;
        }
        let samples = self.space.take()?;
        self.state = State::Idle;
        let frames = std::mem::take(&mut self.written);
        Some(Capture {
            bank: self.bank,
            slot: self.slot,
            source: self.source,
            samples,
            frames,
            sample_rate: self.sample_rate,
            bpm: self.bpm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::DeckId;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn space(frames: usize) -> Vec<f32> {
        vec![0.0; frames * 2]
    }

    fn armed(frames: usize) -> Recorder {
        let mut recorder = Recorder::new(SR);
        assert!(recorder.give_space(space(frames)).is_none());
        recorder
    }

    #[test]
    fn a_recorder_with_nowhere_to_write_refuses_to_start() {
        let mut recorder = Recorder::new(SR);
        assert!(!recorder.is_ready());
        assert!(!recorder.start(1, 1, RecordSource::Master, None));
        assert!(!recorder.is_running());
    }

    #[test]
    fn what_goes_in_comes_out() {
        let mut recorder = armed(100);
        assert!(recorder.start(2, 5, RecordSource::Master, Some(128.0)));

        for n in 0..10 {
            recorder.write(n as f32, -(n as f32));
        }
        recorder.stop();

        let capture = recorder.take().expect("a capture was waiting");
        assert_eq!((capture.bank, capture.slot), (2, 5));
        assert_eq!(capture.frames, 10);
        assert_eq!(capture.bpm, Some(128.0));
        assert_eq!(&capture.samples[..4], &[0.0, 0.0, 1.0, -1.0]);
        // Past the recording is the silence the buffer arrived with, which is
        // why `frames` travels with it rather than being inferred from length.
        assert_eq!(capture.samples.len(), 200);
        assert_eq!(capture.samples[20], 0.0);
    }

    /// Collecting a capture takes the buffer with it, so the recorder is not
    /// ready again until the host sends another. A recorder that quietly stayed
    /// ready would record the next take over the top of the last one.
    #[test]
    fn collecting_a_capture_leaves_nowhere_to_write() {
        let mut recorder = armed(100);
        recorder.start(1, 1, RecordSource::Master, None);
        recorder.write(1.0, 1.0);
        recorder.stop();

        assert!(recorder.is_ready());
        let _ = recorder.take().unwrap();
        assert!(!recorder.is_ready());
        assert!(!recorder.start(1, 2, RecordSource::Master, None));

        assert!(recorder.give_space(space(100)).is_none());
        assert!(recorder.start(1, 2, RecordSource::Master, None));
    }

    /// A full buffer ends the take. Wrapping would hand back a sample whose
    /// beginning is its end.
    #[test]
    fn it_stops_itself_when_the_buffer_is_full() {
        let mut recorder = armed(4);
        recorder.start(1, 1, RecordSource::Master, None);
        for n in 0..20 {
            recorder.write(n as f32, n as f32);
        }
        assert!(!recorder.is_running(), "it should have stopped itself");

        let capture = recorder
            .take()
            .expect("what it caught is still worth having");
        assert_eq!(capture.frames, 4, "the whole buffer should be used");
        assert_eq!(capture.samples[0], 0.0);
        assert_eq!(capture.samples[6], 3.0, "the last frame it had room for");
        // The sixteen frames it refused are simply gone. They are not wrapped
        // over the start, which would hand back a sample whose beginning is
        // its end.
        assert_eq!(capture.samples[2], 1.0);
    }

    /// Press record and stop in the same gesture and you get nothing — not an
    /// empty sample dropped over whatever was in the slot.
    #[test]
    fn a_capture_of_nothing_is_not_a_capture() {
        let mut recorder = armed(100);
        recorder.start(1, 1, RecordSource::Master, None);
        recorder.stop();
        assert!(recorder.take().is_none());
        // And the buffer is still there to record into.
        assert!(recorder.is_ready());
    }

    #[test]
    fn cancelling_throws_the_take_away() {
        let mut recorder = armed(100);
        recorder.start(1, 1, RecordSource::Master, None);
        for _ in 0..50 {
            recorder.write(1.0, 1.0);
        }
        recorder.cancel();

        assert!(!recorder.is_running());
        assert!(recorder.take().is_none(), "a cancelled take must not land");
        assert!(recorder.is_ready());
        assert_eq!(recorder.frames(), 0);
    }

    /// One recorder, one recording. Starting a second while the first runs
    /// would silently abandon it.
    #[test]
    fn a_second_take_cannot_start_over_a_running_one() {
        let mut recorder = armed(100);
        recorder.start(1, 1, RecordSource::Master, None);
        recorder.write(1.0, 1.0);

        assert!(!recorder.start(1, 2, RecordSource::Master, None));
        assert_eq!(recorder.slot(), Some(1), "it kept recording slot 1");

        // And a buffer offered mid-take is handed straight back rather than
        // swapped in under the recording.
        let offered = recorder.give_space(space(100));
        assert!(offered.is_some(), "the buffer must come back untouched");
    }

    #[test]
    fn only_the_named_tap_is_taken() {
        let deck = DeckId::from_human(2).unwrap();
        let mut recorder = armed(100);
        assert_eq!(recorder.tapping(), None, "idle taps nothing");

        recorder.start(1, 1, RecordSource::Deck(deck), None);
        assert_eq!(recorder.tapping(), Some(RecordSource::Deck(deck)));

        recorder.stop();
        assert_eq!(recorder.tapping(), None, "a finished take taps nothing");
    }

    #[test]
    fn elapsed_time_follows_what_was_written() {
        let mut recorder = armed(SR.as_f64() as usize);
        recorder.start(1, 1, RecordSource::Master, None);
        assert_eq!(recorder.seconds(), 0.0);

        for _ in 0..(SR.as_f64() as usize / 2) {
            recorder.write(0.5, 0.5);
        }
        assert!(
            (recorder.seconds() - 0.5).abs() < 1e-3,
            "{}",
            recorder.seconds()
        );
    }

    /// Writing while nothing is running must not touch the buffer — a stray
    /// frame from a block that ended after `stop` would land in the next take.
    #[test]
    fn writes_outside_a_take_go_nowhere() {
        let mut recorder = armed(100);
        recorder.write(1.0, 1.0);
        assert_eq!(recorder.frames(), 0);

        recorder.start(1, 1, RecordSource::Master, None);
        recorder.write(0.25, 0.25);
        recorder.stop();
        recorder.write(1.0, 1.0);

        let capture = recorder.take().unwrap();
        assert_eq!(capture.frames, 1);
        assert_eq!(capture.samples[0], 0.25);
        assert_eq!(capture.samples[2], 0.0);
    }
}

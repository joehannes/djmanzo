//! §22's *audition*: a record in the headphones, without a deck and without
//! the room.
//!
//! [§22 of the directive](../../../docs/DIRECTIVE.md) lists six things a DJ may
//! do to a candidate on the next-track rail — audition, stage, load, reject,
//! pin, "more like this" — and five of them were about rows in a list. This one
//! is about audio, and the reason it stayed missing is worth stating plainly
//! because it was **wrong**: the rail's row said auditioning "needs a preview
//! player djmanzo does not have", and djmanzo had every part of one. A sampler
//! slot already takes the same [`TrackSource`] a deck takes, already plays it,
//! and can already be routed to the headphones alone. What it cannot do is
//! start anywhere but the beginning, and a preview that always opens a record
//! at 0:00 answers a question nobody asked: a DJ deciding whether to bring a
//! record in at the drop wants to hear the drop.
//!
//! # Why a voice of its own rather than a reserved sampler slot
//!
//! A sampler slot is the DJ's. Taking one for previews would take a pad away,
//! which is the contextual demotion §3 refuses, and an audition would stop
//! whatever that pad was holding. Four banks of eight is a promise about how
//! many samples fit; thirty-one is a different promise.
//!
//! # It cannot reach the room, structurally
//!
//! There is no `output` field here and no branch that sums this into the main
//! bus. [`Sample`](crate::sampler::Sample) carries a
//! [`SampleOutput`](dj_core::SampleOutput) because a sample is *for* the room
//! and the cue is the courtesy; a preview is the other way round. The whole
//! point of an audition is that the floor does not hear it, and a routing
//! switch is a switch somebody gets wrong once, in front of a crowd, at the
//! worst possible moment. A preview with no route to the master cannot make
//! that mistake, and no test has to prove it will not.
//!
//! A machine with no cue pair is therefore a machine where an audition is
//! silent rather than public — see [`Preview::process`]. That is the right
//! failure: a DJ on a single stereo output has nowhere private to listen, and
//! playing the candidate out loud would be djmanzo deciding that hearing it
//! matters more than the set does.
//!
//! # At the record's own speed
//!
//! Not stretched to the master tempo, though the machinery is right there. The
//! rail already says what the tempo difference is — `+3 BPM` is the first thing
//! on every row — so the number is answered. What an audition answers is the
//! other question, *what is this record*, and the honest playback of a record
//! is the record. Stretching it would also mean picking a key-lock policy for
//! a listen that lasts fifteen seconds, and a preview that pitched a vocal up
//! two semitones would be lying about the one thing the DJ is listening for.
//!
//! # Everything here runs on the audio thread
//!
//! Starting an audition sets a `bool` and an `f64`. The source crosses the
//! command queue as an `Arc` the way a deck's does, and the displaced one is
//! handed back to be dropped where dropping is allowed.

use dj_decode::{AudioBuffer, TrackSource};
use std::sync::Arc;

/// One record, in the headphones.
#[derive(Debug)]
pub struct Preview {
    source: Arc<dyn TrackSource>,
    /// Where in the record we are, in source frames.
    position: f64,
    playing: bool,
    /// The device's rate, so a 44.1 kHz record plays at the right speed on a
    /// 48 kHz card. Held rather than passed in because nothing else about a
    /// preview varies per block.
    device_rate: f64,
}

impl Preview {
    #[must_use]
    pub fn new(device_rate: f64) -> Self {
        Self {
            source: Arc::new(AudioBuffer::empty()),
            position: 0.0,
            playing: false,
            device_rate: if device_rate > 0.0 {
                device_rate
            } else {
                dj_core::SampleRate::DEFAULT.as_f64()
            },
        }
    }

    /// Start an audition, handing back whatever was playing before.
    ///
    /// Returns the old source rather than dropping it, for the reason
    /// [`crate::sampler::Sample::load`] gives: dropping an `Arc` can free a
    /// buffer, and freeing is an allocator call.
    ///
    /// `from_frame` is where to start, in the candidate's **own** frames —
    /// which is the whole difference between this and a sampler slot. Out of
    /// range is clamped rather than refused: a mix point past the end of a
    /// record is a question about a record that ends sooner than the planner
    /// thought, and starting at the last frame says that in the one way a DJ
    /// listening will understand, by the audition being over immediately.
    /// Silence would be indistinguishable from a broken button.
    #[must_use]
    pub fn start(&mut self, source: Arc<dyn TrackSource>, from_frame: f64) -> Arc<dyn TrackSource> {
        let len = source.len_frames() as f64;
        let previous = std::mem::replace(&mut self.source, source);
        self.position = if from_frame.is_finite() {
            from_frame.clamp(0.0, (len - 1.0).max(0.0))
        } else {
            0.0
        };
        self.playing = len > 0.0;
        previous
    }

    /// Stop listening.
    ///
    /// The source stays loaded. A DJ who stops an audition and starts it again
    /// is the ordinary case, and re-sending the whole record over the queue to
    /// answer a second press would be the host doing work the engine already
    /// has the answer to.
    pub fn stop(&mut self) {
        self.playing = false;
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// How far into the record the audition is, in source frames.
    ///
    /// Zero when nothing is playing, which is not the same as the record's
    /// first frame and does not need to be: the interface draws a position
    /// only while [`Preview::is_playing`] is true.
    #[must_use]
    pub fn position(&self) -> f64 {
        if self.playing { self.position } else { 0.0 }
    }

    /// Mix the audition into the cue pair, and nowhere else.
    ///
    /// Returns the block's peak, for a meter. Zero when nothing is playing and
    /// zero on a machine with no cue pair — see the module docs for why that
    /// is silence rather than the main bus.
    pub fn process(&mut self, out: &mut [f32], layout: &crate::bus::BusLayout) -> f32 {
        if !self.playing {
            return 0.0;
        }
        let channels = layout.channels.max(1);
        let Some((cue_l, cue_r)) = layout.cue else {
            // Nowhere private to listen. The audition still *runs* -- the
            // position advances and it ends when the record does -- because a
            // preview that silently froze would leave the interface drawing a
            // playhead that never moves, and the DJ diagnosing the wrong
            // thing.
            self.advance(out.len() / channels);
            return 0.0;
        };

        let step = self.step();
        let len = self.source.len_frames() as f64;
        let mut peak = 0.0f32;
        for frame in out.chunks_exact_mut(channels) {
            if self.position >= len {
                // One-shot, always. A preview that looped would still be
                // running in a DJ's ear an hour later, and the pad that
                // started it is eight rows up a list that has since scrolled.
                self.playing = false;
                self.position = 0.0;
                break;
            }
            let [left, right] = self.source.frame_at(self.position);
            frame[cue_l] += left;
            frame[cue_r] += right;
            peak = peak.max(left.abs()).max(right.abs());
            self.position += step;
        }
        peak
    }

    /// Run the playhead on without producing audio.
    fn advance(&mut self, frames: usize) {
        let len = self.source.len_frames() as f64;
        self.position += self.step() * frames as f64;
        if self.position >= len {
            self.playing = false;
            self.position = 0.0;
        }
    }

    /// Source frames per output frame.
    ///
    /// Rate conversion only. There is no tempo term here and the module docs
    /// say why: an audition is the record, not the mix.
    #[inline]
    fn step(&self) -> f64 {
        self.source.sample_rate().as_f64() / self.device_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::BusLayout;
    use dj_core::SampleRate;

    const SR: f64 = 48_000.0;

    /// A ramp, so a test can read the playhead out of the audio.
    fn record(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SampleRate::DEFAULT))
    }

    /// Four channels: a main pair and a cue pair, which is what a DJ's
    /// interface gives them.
    fn with_cue() -> BusLayout {
        BusLayout::for_channels(4)
    }

    #[test]
    fn nothing_loaded_is_silence() {
        let mut preview = Preview::new(SR);
        let mut out = vec![0.0f32; 4 * 8];
        assert_eq!(preview.process(&mut out, &with_cue()), 0.0);
        assert!(out.iter().all(|s| *s == 0.0));
        assert!(!preview.is_playing());
    }

    /// **The load-bearing one: an audition starts where it was asked to.**
    ///
    /// The difference between this and a sampler slot, and the reason §22's
    /// audition could not be a reserved slot. The ramp makes the answer
    /// readable: frame *n* carries the value *n*.
    #[test]
    fn an_audition_starts_at_the_frame_it_was_given() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(1_000), 400.0);
        let mut out = vec![0.0f32; 4 * 4];
        preview.process(&mut out, &with_cue());

        let layout = with_cue();
        let (cue_l, _) = layout.cue.unwrap();
        assert_eq!(out[cue_l], 400.0, "the audition began somewhere else");
        assert_eq!(out[4 + cue_l], 401.0);
    }

    /// **And it cannot reach the room.**
    ///
    /// The main pair stays at zero while the cue pair carries the record. Not
    /// a routing test so much as a statement about what is absent: there is no
    /// code here that could put a preview on the main bus, and this is what
    /// says so out loud.
    #[test]
    fn an_audition_never_reaches_the_main_bus() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(1_000), 0.0);
        let mut out = vec![0.0f32; 4 * 16];
        preview.process(&mut out, &with_cue());

        let layout = with_cue();
        let (main_l, main_r) = layout.main;
        let (cue_l, _) = layout.cue.unwrap();
        for frame in out.as_chunks::<4>().0 {
            assert_eq!(frame[main_l], 0.0, "the room heard the audition");
            assert_eq!(frame[main_r], 0.0, "the room heard the audition");
        }
        assert!(out[cue_l] != 0.0 || out[4 + cue_l] != 0.0);
    }

    /// **A machine with no cue pair auditions in silence rather than out loud.**
    #[test]
    fn with_nowhere_private_to_listen_nothing_is_played() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(1_000), 0.0);
        let stereo = BusLayout::for_channels(2);
        assert!(
            stereo.cue.is_none(),
            "this test needs a machine with no cue"
        );

        let mut out = vec![0.0f32; 2 * 16];
        assert_eq!(preview.process(&mut out, &stereo), 0.0);
        assert!(out.iter().all(|s| *s == 0.0), "the room heard the audition");
    }

    /// **It ends with the record rather than looping.**
    #[test]
    fn an_audition_stops_at_the_end_of_the_record() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(8), 0.0);
        let mut out = vec![0.0f32; 4 * 32];
        preview.process(&mut out, &with_cue());
        assert!(!preview.is_playing(), "the audition wrapped round");
        assert_eq!(preview.position(), 0.0);
    }

    /// **Stopping keeps the record**, so a second press does not need the host.
    #[test]
    fn stopping_leaves_the_record_loaded() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(1_000), 0.0);
        preview.stop();
        assert!(!preview.is_playing());

        let same = Arc::clone(&preview.source);
        let previous = preview.start(same, 10.0);
        assert!(
            Arc::ptr_eq(&previous, &preview.source),
            "a second audition of the same record should not need it sent again"
        );
        assert!(preview.is_playing());
    }

    /// **A start position past the end of the record ends the audition at once.**
    ///
    /// Rather than refusing it or wrapping to the top. A planner that put a mix
    /// point past the end of a record is describing a record that is shorter
    /// than it thought, and the DJ finds that out by the audition being over.
    #[test]
    fn a_start_past_the_end_is_clamped_rather_than_refused() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(8), 10_000.0);
        assert!(preview.is_playing(), "the button did nothing");

        let mut out = vec![0.0f32; 4 * 4];
        preview.process(&mut out, &with_cue());
        assert!(!preview.is_playing());
    }

    /// **An empty record is not an audition**, so the interface does not draw a
    /// playhead crawling through nothing.
    #[test]
    fn an_empty_record_does_not_start() {
        let mut preview = Preview::new(SR);
        let _ = preview.start(record(0), 0.0);
        assert!(!preview.is_playing());
    }

    /// **A 44.1 kHz record plays at the right speed on a 48 kHz card.**
    ///
    /// The one piece of arithmetic in here, and the one that is silently wrong
    /// if it is missing: a preview that ignored the rate would play every
    /// record from a CD rip about 9% flat, which is a semitone and a half and
    /// would have a DJ rejecting records for being out of key.
    #[test]
    fn a_record_at_another_rate_is_converted_rather_than_pitched() {
        let mut preview = Preview::new(48_000.0);
        let samples: Vec<f32> = (0..1_000).flat_map(|n| [n as f32, n as f32]).collect();
        let at_44k = Arc::new(AudioBuffer::from_interleaved(
            samples,
            SampleRate::new(44_100).unwrap(),
        ));
        let _ = preview.start(at_44k, 0.0);

        let mut out = vec![0.0f32; 4 * 2];
        preview.process(&mut out, &with_cue());
        let (cue_l, _) = with_cue().cue.unwrap();
        // 44100/48000 = 0.91875 source frames per output frame.
        assert!(
            (out[4 + cue_l] - 0.918_75).abs() < 0.01,
            "expected the second output frame to be 0.91875 source frames in, got {}",
            out[4 + cue_l]
        );
    }
}

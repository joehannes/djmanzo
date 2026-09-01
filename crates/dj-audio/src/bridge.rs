//! Two sound cards, two crystals, one mix.
//!
//! A DJ with a laptop and a cheap USB interface has no four-channel device, so
//! the master goes to one card and the headphone cue to another. That is a
//! normal setup and it is the reason this module exists.
//!
//! # The problem is that neither clock is right
//!
//! Two devices both nominally running at 48 kHz are not running at the same
//! rate. Each is timed by its own crystal, and a cheap one is comfortably
//! ±50 ppm out. Fifty parts per million is 2.4 samples every second, 180 ms
//! every hour — one card genuinely produces more audio per wall-clock second
//! than the other consumes.
//!
//! So a plain ring buffer between the two callbacks does not work. It works for
//! a few minutes, then either drains — clicks, and eventually silence — or
//! fills, adding latency until it overflows and drops a chunk. Both failures
//! arrive mid-set, after everything sounded fine during the soundcheck, which
//! is the worst possible time to discover them.
//!
//! # The fix is to resample by the amount they disagree
//!
//! The consumer reads the ring at a rate slightly other than one sample per
//! sample, chosen to hold the buffer at a constant level. If the reader is
//! running fast it is asked to read a shade slower, and the ring stops
//! draining. Measuring the fill level *is* measuring the drift: nothing needs
//! to know either card's true frequency, only that the queue is not the length
//! it should be.
//!
//! Three properties matter, and all three are the difference between this
//! working and being a novelty:
//!
//! - **The loop is slow.** Fill level is noisy — callbacks are scheduled by the
//!   operating system and arrive in bursts — and a loop fast enough to chase
//!   that noise would be modulating pitch at audio rates. Real drift changes
//!   over minutes, so the loop is given a time constant in seconds and the
//!   jitter averages out.
//! - **The correction is clamped.** Bounded to a range real hardware can
//!   actually exhibit, so a bug, a stalled thread or a device change produces a
//!   bounded, inaudible error rather than something recognisably wrong.
//! - **Interpolation is cubic.** Resampling by 1.00005 still sweeps the
//!   fractional read position slowly through the whole interval, so the
//!   interpolator's error is heard as a slow filter sweep rather than as
//!   distortion. Linear interpolation is audible that way on sustained
//!   material; four-point Hermite is not, and costs a handful of multiplies.
//!
//! # Realtime rules
//!
//! Both halves live on audio threads — different ones — so nothing here
//! allocates, locks or blocks. The ring is a lock-free SPSC queue, and every
//! buffer is sized at construction.

use crate::{AudioCallback, RenderContext};
use dj_core::SampleRate;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Channels carried across the bridge.
const CHANNELS: usize = 2;

/// What the bridge is doing, readable from any thread.
///
/// The two halves are owned by device callbacks and moved into the backend, so
/// nothing can borrow them afterwards to ask how it is going. These atomics are
/// the answer: written from the audio threads with relaxed ordering, read by
/// whoever is drawing the interface.
///
/// This is worth surfacing rather than keeping internal. A drift figure that
/// settles near zero says the two cards are well matched; one that keeps
/// climbing says a device is lying about its rate, which is a real and
/// otherwise invisible fault.
#[derive(Debug, Default)]
pub struct BridgeStats {
    /// Correction currently applied, in parts per million, scaled by 1000 so it
    /// survives an integer.
    drift_milli_ppm: AtomicI64,
    queued_frames: AtomicU64,
    target_frames: AtomicU64,
    starved_frames: AtomicU64,
    dropped_samples: AtomicU64,
}

impl BridgeStats {
    /// Measured disagreement between the two clocks, in ppm.
    #[must_use]
    pub fn drift_ppm(&self) -> f64 {
        self.drift_milli_ppm.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Frames queued between the devices. Over the sample rate, this is the
    /// extra latency the headphone path carries.
    #[must_use]
    pub fn queued_frames(&self) -> u64 {
        self.queued_frames.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn target_frames(&self) -> u64 {
        self.target_frames.load(Ordering::Relaxed)
    }

    /// Non-zero means the headphones went silent for a moment.
    #[must_use]
    pub fn starved_frames(&self) -> u64 {
        self.starved_frames.load(Ordering::Relaxed)
    }

    /// Non-zero means the secondary device stopped consuming.
    #[must_use]
    pub fn dropped_samples(&self) -> u64 {
        self.dropped_samples.load(Ordering::Relaxed)
    }

    /// True when either side has lost audio. What the interface should react
    /// to, rather than making it compare two counters itself.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.starved_frames() == 0 && self.dropped_samples() == 0
    }
}

/// How much of a callback period to hold in the queue, as a multiple of the
/// buffer size.
///
/// The queue absorbs the difference in *scheduling* between two devices that
/// have no relationship to each other: their callbacks interleave arbitrarily,
/// and one can be late while the other is early. Below about two periods that
/// jitter alone empties the queue; much above four is latency in the
/// headphones being paid for nothing.
const TARGET_PERIODS: usize = 3;

/// Headroom in the ring beyond the target, so a late callback has somewhere to
/// put its audio instead of dropping it.
const CAPACITY_PERIODS: usize = 8;

/// Widest correction the loop may apply, as a fraction.
///
/// 0.002 is 2000 ppm — twenty times the drift of genuinely bad hardware, so it
/// never limits legitimate correction, while still bounding the damage from
/// anything unexpected to a shift of about three cents. Three cents is below
/// the threshold most people can hear at all, and far below the point where a
/// headphone cue stops being usable for beatmatching.
const MAX_CORRECTION: f64 = 0.002;

/// Proportional gain of the control loop.
///
/// Applied to the fill error expressed as a fraction of the target, so a queue
/// at double its target length asks for the full correction. Deliberately
/// gentle: this is the term that decides how hard the loop pulls, and pulling
/// hard on a noisy measurement is what makes a resampler audible.
const LOOP_GAIN: f64 = 0.002;

/// Time constant of the ratio smoother, in seconds.
///
/// The loop's real defence against jitter. Drift is a property of two crystals
/// and changes over minutes; anything moving faster than this is scheduling
/// noise, and following it would put that noise onto the audio as pitch.
const SMOOTHING_SECONDS: f64 = 4.0;

/// The primary side. Lives in the callback that drives the engine.
#[derive(Debug)]
pub struct CueProducer {
    ring: rtrb::Producer<f32>,
    /// Frames the ring would not accept. Reported rather than silently ignored:
    /// a non-zero count means the secondary device has stopped consuming, which
    /// the interface should say out loud.
    dropped: u64,
    stats: Arc<BridgeStats>,
}

impl CueProducer {
    /// Push one block of interleaved stereo.
    ///
    /// Realtime-safe. Audio that does not fit is counted and discarded — the
    /// alternative is blocking the master callback, and a dropout on the PA to
    /// protect the headphones is precisely the wrong trade.
    pub fn push(&mut self, frames: &[f32]) {
        let before = self.dropped;
        for sample in frames {
            if self.ring.push(*sample).is_err() {
                self.dropped += 1;
            }
        }
        // Published only when it changes: a store per block is cheap, a store
        // per sample is not, and this number moves rarely or never.
        if self.dropped != before {
            self.stats
                .dropped_samples
                .store(self.dropped, Ordering::Relaxed);
        }
    }

    #[must_use]
    pub fn dropped_samples(&self) -> u64 {
        self.dropped
    }
}

/// The secondary side. Lives in the headphone device's callback.
#[derive(Debug)]
pub struct CueConsumer {
    ring: rtrb::Consumer<f32>,

    /// Four input frames, newest last, for the interpolator.
    history: [[f32; 4]; CHANNELS],
    /// Fractional position between `history[..][1]` and `history[..][2]`.
    phase: f64,

    /// Input frames consumed per output frame. 1.0 is no correction.
    ratio: f64,
    /// Where the loop wants `ratio` to be. `ratio` slews towards it.
    ratio_target: f64,
    /// One-pole coefficient for that slew.
    smoothing: f64,

    target_fill: usize,
    /// Output frames produced while the ring was empty.
    starved: u64,
    /// True once the queue has filled far enough to start.
    primed: bool,
    stats: Arc<BridgeStats>,
}

impl CueConsumer {
    /// Pull one block of interleaved stereo.
    ///
    /// Realtime-safe. On starvation the output is silence rather than the last
    /// sample repeated: a held sample is a buzz, and silence is both quieter
    /// and more obviously a fault.
    pub fn pull(&mut self, out: &mut [f32]) {
        self.fill(out);

        // Published *after* the block, so the depth anyone reads is the current
        // one rather than the one the control loop happened to act on at the
        // top. The two differ by a whole block, which is most of the number.
        self.stats
            .queued_frames
            .store(self.queued_frames() as u64, Ordering::Relaxed);
        self.stats
            .starved_frames
            .store(self.starved, Ordering::Relaxed);
    }

    fn fill(&mut self, out: &mut [f32]) {
        self.update_ratio(out.len() / CHANNELS);

        // Wait for the queue to reach its working depth before the first
        // sample. Starting into a nearly-empty ring guarantees an immediate
        // underrun, and the control loop would spend its first seconds
        // recovering from a problem that never had to happen.
        if !self.primed {
            if self.ring.slots() < self.target_fill * CHANNELS {
                out.fill(0.0);
                return;
            }
            self.primed = true;
        }

        for frame in out.as_chunks_mut::<CHANNELS>().0 {
            // Advance first: `phase` counts how far past the current input
            // frame we are, and each whole step shifts a new frame in.
            self.phase += self.ratio;
            while self.phase >= 1.0 {
                if !self.shift_in() {
                    // Ring empty. Report it, stop consuming phase, and let the
                    // loop's own correction refill the queue.
                    self.starved += 1;
                    self.primed = false;
                    out.fill(0.0);
                    return;
                }
                self.phase -= 1.0;
            }

            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = hermite(&self.history[channel], self.phase as f32);
            }
        }
    }

    /// Take one frame from the ring into the interpolator's history.
    fn shift_in(&mut self) -> bool {
        if self.ring.slots() < CHANNELS {
            return false;
        }
        for channel in &mut self.history {
            let Ok(sample) = self.ring.pop() else {
                return false;
            };
            channel.rotate_left(1);
            channel[3] = sample;
        }
        true
    }

    /// Steer the ratio from the queue's depth.
    fn update_ratio(&mut self, frames: usize) {
        let fill = (self.ring.slots() / CHANNELS) as f64;
        let target = self.target_fill as f64;

        // Positive error means the queue is longer than it should be, so the
        // producer's clock is the faster one and this side should read faster
        // to catch up.
        let error = (fill - target) / target;
        self.ratio_target =
            (1.0 + error * LOOP_GAIN).clamp(1.0 - MAX_CORRECTION, 1.0 + MAX_CORRECTION);

        // Slew over this block's worth of time, so the smoothing is in seconds
        // rather than in callbacks -- otherwise the loop's speed would change
        // with the buffer size.
        let alpha = 1.0 - (-(frames as f64) * self.smoothing).exp();
        self.ratio += (self.ratio_target - self.ratio) * alpha;

        // Once per block, not once per frame.
        self.stats
            .drift_milli_ppm
            .store(((self.ratio - 1.0) * 1e9) as i64, Ordering::Relaxed);
    }

    /// Current correction in parts per million, signed.
    ///
    /// This is the measured disagreement between the two cards, and it is worth
    /// showing: a figure that settles near zero says the pair is well matched,
    /// and one that keeps climbing says a device is misreporting its rate.
    #[must_use]
    pub fn drift_ppm(&self) -> f64 {
        (self.ratio - 1.0) * 1e6
    }

    /// Frames currently queued. Divided by the sample rate this is the extra
    /// latency the headphone path is carrying.
    #[must_use]
    pub fn queued_frames(&self) -> usize {
        self.ring.slots() / CHANNELS
    }

    #[must_use]
    pub fn target_frames(&self) -> usize {
        self.target_fill
    }

    /// Output frames produced while the queue was empty. Non-zero means the
    /// user heard something.
    #[must_use]
    pub fn starved_frames(&self) -> u64 {
        self.starved
    }
}

/// Four-point cubic Hermite interpolation.
///
/// `points` are consecutive samples; `t` is the position in `[0, 1)` between
/// the middle two. The Catmull-Rom form, which passes through every sample and
/// takes its slope from the neighbours — so it degrades to something smooth
/// rather than to something bright.
fn hermite(points: &[f32; 4], t: f32) -> f32 {
    let (a, b, c, d) = (points[0], points[1], points[2], points[3]);
    let c0 = b;
    let c1 = 0.5 * (c - a);
    let c2 = a - 2.5 * b + 2.0 * c - 0.5 * d;
    let c3 = 0.5 * (d - a) + 1.5 * (b - c);
    ((c3 * t + c2) * t + c1) * t + c0
}

/// Build a bridge between two device callbacks.
///
/// `buffer_frames` is the *secondary* device's block size, because that is what
/// sets how much the queue has to absorb.
#[must_use]
pub fn cue_bridge(
    sample_rate: SampleRate,
    buffer_frames: u32,
) -> (CueProducer, CueConsumer, Arc<BridgeStats>) {
    let period = (buffer_frames as usize).max(32);
    let target_fill = period * TARGET_PERIODS;
    let capacity = period * CAPACITY_PERIODS * CHANNELS;

    let (producer, consumer) = rtrb::RingBuffer::new(capacity);
    let rate = sample_rate.as_f64();
    let stats = Arc::new(BridgeStats::default());
    stats
        .target_frames
        .store(target_fill as u64, Ordering::Relaxed);

    (
        CueProducer {
            ring: producer,
            dropped: 0,
            stats: Arc::clone(&stats),
        },
        CueConsumer {
            ring: consumer,
            history: [[0.0; 4]; CHANNELS],
            phase: 0.0,
            ratio: 1.0,
            ratio_target: 1.0,
            smoothing: 1.0 / (SMOOTHING_SECONDS * rate),
            target_fill,
            starved: 0,
            primed: false,
            stats: Arc::clone(&stats),
        },
        stats,
    )
}

/// Wraps the engine so the master callback also feeds the headphone device.
///
/// The engine renders master and cue into one interleaved buffer whatever the
/// output looks like, so in a split setup it renders four channels into a
/// scratch buffer: the first pair goes to this device, the second pair goes
/// across the bridge.
#[derive(Debug)]
pub struct SplitPrimary {
    inner: Box<dyn AudioCallback>,
    producer: CueProducer,
    /// Four-channel working buffer, sized once.
    scratch: Vec<f32>,
}

/// Frames the scratch buffer is sized for.
///
/// Generous: a device asking for more than this is not a configuration anyone
/// runs, and the alternative to a fixed size is allocating on the audio thread.
const MAX_BLOCK_FRAMES: usize = 8_192;

/// Channels the engine renders in split mode: master pair, then cue pair.
const SPLIT_CHANNELS: usize = 4;

impl SplitPrimary {
    #[must_use]
    pub fn new(inner: Box<dyn AudioCallback>, producer: CueProducer) -> Self {
        Self {
            inner,
            producer,
            scratch: vec![0.0; MAX_BLOCK_FRAMES * SPLIT_CHANNELS],
        }
    }

    #[must_use]
    pub fn producer(&self) -> &CueProducer {
        &self.producer
    }
}

impl AudioCallback for SplitPrimary {
    fn render(&mut self, out: &mut [f32], ctx: &RenderContext) {
        let channels = ctx.channels.max(1);
        let frames = (out.len() / channels).min(MAX_BLOCK_FRAMES);

        let block = &mut self.scratch[..frames * SPLIT_CHANNELS];
        block.fill(0.0);
        self.inner.render(
            block,
            &RenderContext {
                frames,
                channels: SPLIT_CHANNELS,
                sample_rate: ctx.sample_rate,
            },
        );

        // Master to this device, cue to the other one. Interleaved in both
        // cases, so this is a strided copy rather than anything clever.
        for (index, source) in block.as_chunks::<SPLIT_CHANNELS>().0.iter().enumerate() {
            let target = &mut out[index * channels..];
            target[0] = source[0];
            if channels > 1 {
                target[1] = source[1];
            }
        }
        for source in block.as_chunks::<SPLIT_CHANNELS>().0 {
            self.producer.push(&source[2..4]);
        }

        // A device asking for more than the scratch holds gets silence past
        // that point rather than an allocation or a panic.
        for sample in &mut out[frames * channels..] {
            *sample = 0.0;
        }
    }
}

/// The headphone device's callback: drain the bridge and nothing else.
#[derive(Debug)]
pub struct SplitSecondary {
    consumer: CueConsumer,
}

impl SplitSecondary {
    #[must_use]
    pub fn new(consumer: CueConsumer) -> Self {
        Self { consumer }
    }

    #[must_use]
    pub fn consumer(&self) -> &CueConsumer {
        &self.consumer
    }
}

impl AudioCallback for SplitSecondary {
    fn render(&mut self, out: &mut [f32], ctx: &RenderContext) {
        if ctx.channels == CHANNELS {
            self.consumer.pull(out);
            return;
        }

        // A secondary device with a different channel count still has to work.
        // Pull a stereo frame and spread it, rather than refusing to run.
        let channels = ctx.channels.max(1);
        let mut frame = [0.0f32; CHANNELS];
        for target in out.chunks_mut(channels) {
            self.consumer.pull(&mut frame);
            for (index, sample) in target.iter_mut().enumerate() {
                *sample = frame[index.min(CHANNELS - 1)];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;
    const BLOCK: usize = 256;

    fn bridge() -> (CueProducer, CueConsumer) {
        let (producer, consumer, _stats) = cue_bridge(SR, BLOCK as u32);
        (producer, consumer)
    }

    /// Interleaved stereo ramp, so a discontinuity in the output is visible as
    /// a break in a straight line rather than having to be inferred.
    fn ramp(start: usize, frames: usize) -> Vec<f32> {
        (start..start + frames)
            .flat_map(|n| {
                let v = (n % 1000) as f32 / 1000.0;
                [v, v]
            })
            .collect()
    }

    fn sine(start: usize, frames: usize, hz: f32) -> Vec<f32> {
        use std::f32::consts::TAU;
        (start..start + frames)
            .flat_map(|n| {
                let v = (TAU * hz * n as f32 / 48_000.0).sin() * 0.5;
                [v, v]
            })
            .collect()
    }

    /// Run the bridge with the two clocks deliberately mismatched.
    ///
    /// `ppm` is how much faster the *producer* runs. Simulated the way it
    /// actually happens: the producer puts in slightly more or slightly fewer
    /// frames per block than the consumer takes out.
    fn run_mismatched(ppm: f64, seconds: f64) -> (CueConsumer, u64) {
        let (mut producer, mut consumer) = bridge();
        let blocks = (seconds * SR.as_f64() / BLOCK as f64) as usize;
        let mut produced = 0.0f64;
        let mut cursor = 0usize;
        let mut out = vec![0.0f32; BLOCK * CHANNELS];

        for _ in 0..blocks {
            // The producer's block is BLOCK frames on its own clock, which is
            // slightly more or fewer frames of the consumer's time.
            produced += BLOCK as f64 * (1.0 + ppm / 1e6);
            let want = produced as usize - cursor;
            producer.push(&ramp(cursor, want));
            cursor += want;

            consumer.pull(&mut out);
        }
        let dropped = producer.dropped_samples();
        (consumer, dropped)
    }

    /// A set long enough for uncorrected drift to actually fail.
    ///
    /// This number is load-bearing and was chosen by working out what it takes
    /// rather than by picking something that sounded long. At 50 ppm the two
    /// clocks separate by 2.4 frames a second, so a two-minute simulation
    /// accumulates 288 frames — comfortably inside a 2048-frame ring, which
    /// means a two-minute test passes with the correction *switched off* and
    /// proves nothing. Twenty minutes accumulates 2880 frames, past both the
    /// ring's capacity and its target depth, so both failure modes are really
    /// reached. Verified by setting `LOOP_GAIN` to zero and watching these
    /// fail.
    ///
    /// It is also the honest scenario: twenty minutes is a short set.
    const LONG_SET_SECONDS: f64 = 1_200.0;

    /// **The failure this module exists to prevent.** With the producer running
    /// fast, an uncorrected queue grows without bound until it overflows and
    /// drops audio.
    #[test]
    fn a_fast_producer_does_not_overflow_the_queue() {
        let (consumer, dropped) = run_mismatched(50.0, LONG_SET_SECONDS);
        assert_eq!(dropped, 0, "the queue overflowed and dropped audio");
        assert!(
            consumer.queued_frames() < consumer.target_frames() * 3,
            "queue grew to {} against a target of {}",
            consumer.queued_frames(),
            consumer.target_frames()
        );
    }

    /// The same failure the other way round: a slow producer drains the queue
    /// and the headphones click and then go silent.
    #[test]
    fn a_slow_producer_does_not_starve_the_consumer() {
        let (consumer, _) = run_mismatched(-50.0, LONG_SET_SECONDS);
        assert_eq!(
            consumer.starved_frames(),
            0,
            "the headphone bus ran dry {} times",
            consumer.starved_frames()
        );
    }

    /// Bad hardware, not merely ordinary hardware. Cheap USB interfaces can be
    /// several hundred ppm out, and the loop's range has to cover them.
    #[test]
    fn even_badly_matched_hardware_survives_a_set() {
        for ppm in [-400.0, 400.0] {
            let (consumer, dropped) = run_mismatched(ppm, LONG_SET_SECONDS);
            assert_eq!(dropped, 0, "{ppm} ppm overflowed the queue");
            assert_eq!(
                consumer.starved_frames(),
                0,
                "{ppm} ppm starved the headphone bus"
            );
        }
    }

    /// The loop has to actually *find* the drift, not merely survive it. After
    /// it settles, the correction should be close to the real mismatch.
    #[test]
    fn the_loop_converges_on_the_true_drift() {
        for ppm in [-50.0, -12.0, 30.0, 80.0] {
            let (consumer, _) = run_mismatched(ppm, 240.0);
            let measured = consumer.drift_ppm();
            assert!(
                (measured - ppm).abs() < 20.0,
                "drift of {ppm} ppm was measured as {measured} ppm"
            );
        }
    }

    /// Matched clocks must be left alone. A loop that hunts around zero when
    /// there is nothing to correct would be putting pitch modulation on a
    /// perfectly good signal.
    #[test]
    fn matched_clocks_are_barely_corrected() {
        let (consumer, _) = run_mismatched(0.0, 120.0);
        assert!(
            consumer.drift_ppm().abs() < 20.0,
            "corrected by {} ppm with nothing to correct",
            consumer.drift_ppm()
        );
    }

    /// The clamp is the safety net for everything unforeseen: a stalled
    /// device, a wildly misreported rate, a bug upstream. The error has to stay
    /// bounded and inaudible rather than becoming a chipmunk.
    #[test]
    fn the_correction_is_clamped_even_when_the_queue_is_absurd() {
        let (mut producer, mut consumer) = bridge();
        // Fill the ring completely, which is far past any real drift.
        producer.push(&ramp(0, BLOCK * CAPACITY_PERIODS));
        let mut out = vec![0.0f32; BLOCK * CHANNELS];
        for _ in 0..2_000 {
            consumer.pull(&mut out);
            producer.push(&ramp(0, BLOCK));
        }
        assert!(
            consumer.drift_ppm().abs() <= MAX_CORRECTION * 1e6 + 1.0,
            "correction ran away to {} ppm",
            consumer.drift_ppm()
        );
    }

    /// Latency is bounded and roughly what was asked for, or the headphone cue
    /// drifts further and further behind the master as the night goes on.
    #[test]
    fn the_queue_settles_near_its_target_depth() {
        let (consumer, _) = run_mismatched(30.0, 240.0);
        let queued = consumer.queued_frames() as f64;
        let target = consumer.target_frames() as f64;
        assert!(
            (queued - target).abs() < target,
            "settled at {queued} frames against a target of {target}"
        );
    }

    /// Silence in, silence out. A resampler that rings on nothing would put a
    /// tone in the headphones between tracks.
    #[test]
    fn silence_stays_silent() {
        let (mut producer, mut consumer) = bridge();
        producer.push(&vec![0.0; BLOCK * CHANNELS * 8]);
        let mut out = vec![0.0f32; BLOCK * CHANNELS];
        for _ in 0..8 {
            consumer.pull(&mut out);
        }
        assert!(
            out.iter().all(|s| s.abs() < 1e-9),
            "resampler rang on silence"
        );
    }

    /// The audio has to survive the trip. A sine through the bridge should come
    /// out as the same sine, not as something the interpolator invented.
    #[test]
    fn a_tone_passes_through_recognisably() {
        let (mut producer, mut consumer) = bridge();
        producer.push(&sine(0, BLOCK * 16, 1_000.0));

        let mut out = vec![0.0f32; BLOCK * CHANNELS];
        // Past the priming threshold and the interpolator's history.
        for _ in 0..6 {
            producer.push(&sine(0, BLOCK, 1_000.0));
            consumer.pull(&mut out);
        }

        let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.05, "a 0.5 sine came out at {peak}");
        // Both channels carry the same signal here, so they must match.
        for frame in out.as_chunks::<CHANNELS>().0 {
            assert!(
                (frame[0] - frame[1]).abs() < 1e-6,
                "the two channels diverged: {} vs {}",
                frame[0],
                frame[1]
            );
        }
    }

    /// Starting into an empty ring is the one guaranteed underrun, so the
    /// consumer waits rather than beginning and immediately failing.
    #[test]
    fn the_consumer_waits_for_the_queue_to_prime() {
        let (mut producer, mut consumer) = bridge();
        let mut out = vec![0.0f32; BLOCK * CHANNELS];

        producer.push(&ramp(0, BLOCK));
        consumer.pull(&mut out);
        assert!(out.iter().all(|s| *s == 0.0), "started before priming");
        assert_eq!(consumer.starved_frames(), 0, "waiting is not starving");

        producer.push(&ramp(BLOCK, BLOCK * TARGET_PERIODS));
        consumer.pull(&mut out);
        assert!(out.iter().any(|s| *s != 0.0), "never started");
    }

    /// A stopped secondary device must not take the master down with it. The
    /// producer counts what it could not hand over and carries on.
    #[test]
    fn a_stalled_consumer_costs_the_producer_nothing_but_a_count() {
        let (mut producer, _consumer) = bridge();
        for _ in 0..100 {
            producer.push(&ramp(0, BLOCK));
        }
        assert!(
            producer.dropped_samples() > 0,
            "a full ring should be reported"
        );
    }

    /// The stats exist so an interface can show the drift rather than leaving
    /// it invisible, so they have to agree with the consumer they describe.
    #[test]
    fn the_stats_report_what_the_consumer_measures() {
        let (mut producer, mut consumer, stats) = cue_bridge(SR, BLOCK as u32);
        let mut out = vec![0.0f32; BLOCK * CHANNELS];

        assert_eq!(stats.target_frames(), consumer.target_frames() as u64);
        assert!(stats.is_healthy(), "healthy before anything has happened");

        // A producer running fast, long enough for the loop to find it.
        let mut produced = 0.0f64;
        let mut cursor = 0usize;
        for _ in 0..4_000 {
            produced += BLOCK as f64 * (1.0 + 60.0 / 1e6);
            let want = produced as usize - cursor;
            producer.push(&ramp(cursor, want));
            cursor += want;
            consumer.pull(&mut out);
        }

        assert!(
            (stats.drift_ppm() - consumer.drift_ppm()).abs() < 0.01,
            "stats say {} ppm, consumer says {}",
            stats.drift_ppm(),
            consumer.drift_ppm()
        );
        assert_eq!(stats.queued_frames(), consumer.queued_frames() as u64);
        assert!(stats.is_healthy(), "a healthy run reported a fault");
    }

    /// A fault has to show up, or the report is worse than useless.
    #[test]
    fn the_stats_report_a_stalled_consumer() {
        let (mut producer, _consumer, stats) = cue_bridge(SR, BLOCK as u32);
        for _ in 0..100 {
            producer.push(&ramp(0, BLOCK));
        }
        assert!(stats.dropped_samples() > 0);
        assert!(!stats.is_healthy(), "losing audio counted as healthy");
    }

    #[test]
    fn hermite_passes_through_its_samples() {
        let points = [0.0, 0.25, 0.75, 1.0];
        assert!((hermite(&points, 0.0) - 0.25).abs() < 1e-6);
        assert!((hermite(&points, 1.0) - 0.75).abs() < 1e-6);
    }

    /// A straight line must interpolate to a straight line, or the resampler
    /// would add curvature to material that has none.
    #[test]
    fn hermite_is_exact_on_a_ramp() {
        let points = [0.0, 1.0, 2.0, 3.0];
        for step in 0..=10 {
            let t = step as f32 / 10.0;
            assert!(
                (hermite(&points, t) - (1.0 + t)).abs() < 1e-5,
                "bent a straight line at t={t}"
            );
        }
    }
}

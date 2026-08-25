//! The platter under the hand.
//!
//! A jog wheel is the control a DJ touches most and the one that has to feel
//! right, so what it does is worth stating precisely. It is really three
//! controls wearing one piece of plastic, and which one you get depends on the
//! mode, whether the top is being touched, and whether the deck is playing:
//!
//! | touched | playing | mode  | what happens |
//! |---------|---------|-------|--------------|
//! | yes     | yes     | vinyl | **scratch** -- the audio follows the hand |
//! | yes     | yes     | CDJ   | **bend** -- a temporary nudge to the tempo |
//! | no      | yes     | either| **bend** -- the side of the platter |
//! | either  | no      | either| **search** -- scrub through the track |
//!
//! # Position control for scratching, rate control for bending
//!
//! These are not the same kind of control and they are deliberately not
//! implemented the same way.
//!
//! A **scratch** is a position: where the record is under the needle *is*
//! where the hand has put it. Movement is applied the moment it arrives, with
//! no smoothing, because any smoothing is latency and latency is exactly what
//! makes a scratch feel like rubber rather than like vinyl.
//!
//! A **bend** is a speed: it is "run a little faster while I push". That has
//! to come from how fast the wheel is *turning*, which a single message does
//! not tell you -- so it is estimated from movement over time and smoothed.
//! The smoothing is also what makes the answer independent of how chatty the
//! controller is: a device sending two messages of half a turn and one sending
//! one message of a whole turn arrive at the same speed.

use dj_core::{JogMode, SampleRate};

/// Seconds for one turn of a 12" record at 33 1/3 RPM.
///
/// The number that makes a scratch feel like vinyl: one revolution of the
/// wheel moves one revolution's worth of music. Every jog wheel sold is built
/// to this expectation, so it is a constant rather than a setting.
const PLATTER_SECONDS: f64 = 1.8;

/// How much faster searching is than scratching.
///
/// Scrubbing a five-minute track at record speed would be nearly three
/// minutes of winding. Eight times over makes it about twenty seconds of
/// turning end to end, which is a few flicks of the wrist.
const SEARCH_MULTIPLIER: f64 = 8.0;

/// The most a bend may change the tempo, as a fraction.
///
/// A bend is a nudge to bring two records back into line, not a speed control:
/// past about a fifth a listener hears the pitch move rather than the beats
/// align. Spinning the wheel hard in CDJ mode is therefore *clamped* rather
/// than turning into a scratch, which is what a CDJ does too.
const MAX_BEND: f64 = 0.2;

/// How quickly the estimated wheel speed follows the hand.
///
/// Expressed as a time constant rather than a coefficient because the
/// coefficient depends on the sample rate. 120 ms is long enough to bridge the
/// gaps between a controller's messages -- even a slow one sends every 10 ms --
/// and short enough that letting go feels immediate.
const BEND_TIME_CONSTANT: f64 = 0.120;

/// What the wheel is asking the deck to do this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JogEffect {
    /// Nothing; the deck runs as it would have.
    Free,
    /// Multiply the deck's step by this. `1.0` is normal speed.
    Bend(f64),
    /// Move the playhead by this many source frames, and do not advance
    /// normally -- the hand is driving.
    Scrub(f64),
}

/// One deck's platter.
#[derive(Debug, Clone)]
pub struct Jog {
    mode: JogMode,
    touched: bool,
    /// Movement not yet handed to the deck, in revolutions.
    pending: f64,
    /// Smoothed wheel speed, revolutions per second.
    velocity: f64,
    /// One-pole coefficient for [`BEND_TIME_CONSTANT`] at this rate.
    smoothing: f64,
    device_rate: f64,
    source_rate: f64,
}

impl Jog {
    #[must_use]
    pub fn new(device_rate: SampleRate) -> Self {
        let mut jog = Self {
            mode: JogMode::default(),
            touched: false,
            pending: 0.0,
            velocity: 0.0,
            smoothing: 0.0,
            device_rate: device_rate.as_f64(),
            source_rate: device_rate.as_f64(),
        };
        jog.recompute_smoothing();
        jog
    }

    fn recompute_smoothing(&mut self) {
        self.smoothing = (1.0 / (BEND_TIME_CONSTANT * self.device_rate)).clamp(0.0, 1.0);
    }

    /// The rate the *device* runs at, which sets the smoothing.
    pub fn set_device_rate(&mut self, rate: SampleRate) {
        self.device_rate = rate.as_f64();
        self.recompute_smoothing();
    }

    /// The rate the loaded track runs at, which sets how many frames a turn is
    /// worth.
    pub fn set_source_rate(&mut self, rate: SampleRate) {
        self.source_rate = rate.as_f64();
    }

    #[must_use]
    pub const fn mode(&self) -> JogMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: JogMode) {
        self.mode = mode;
    }

    #[must_use]
    pub const fn is_touched(&self) -> bool {
        self.touched
    }

    /// A hand landing on, or leaving, the top of the platter.
    ///
    /// Letting go drops the pending movement. It has not been applied yet and
    /// applying it after the hand is gone would be the record lurching a
    /// moment after it was released.
    pub fn set_touched(&mut self, touched: bool) {
        if self.touched && !touched {
            self.pending = 0.0;
        }
        self.touched = touched;
    }

    /// The wheel turned by `revolutions`. Positive is forwards.
    ///
    /// Accumulated rather than acted on, so several messages arriving between
    /// two audio frames are one movement rather than several.
    pub fn turn(&mut self, revolutions: f32) {
        let revolutions = f64::from(revolutions);
        if revolutions.is_finite() {
            self.pending += revolutions;
        }
    }

    /// Whether there is movement waiting to be applied.
    ///
    /// Asked by the deck before it decides to render at all: a paused deck
    /// with a hand winding the wheel has to make sound, because searching by
    /// ear is how a DJ finds a cue point, but a paused deck that nobody is
    /// touching must stay on the cheap path.
    #[must_use]
    pub fn has_movement(&self) -> bool {
        self.pending != 0.0
    }

    /// The current bend, as a fraction: `0.0` when the wheel is still.
    ///
    /// For the interface and for the parameter registry, where a jog wheel
    /// that is doing something should be visible.
    #[must_use]
    pub fn bend(&self) -> f64 {
        (self.velocity * PLATTER_SECONDS).clamp(-MAX_BEND, MAX_BEND)
    }

    /// Forget everything. For a track change or a device going away.
    pub fn reset(&mut self) {
        self.pending = 0.0;
        self.velocity = 0.0;
        self.touched = false;
    }

    /// What the wheel wants, for one output frame.
    ///
    /// Called whether or not the wheel moved, because the speed estimate has
    /// to decay when the hand comes off -- a bend that only updated on a
    /// message would stay applied forever after the last one.
    pub fn advance(&mut self, playing: bool) -> JogEffect {
        self.advance_block(1, playing)
    }

    /// The same, for a whole block of `frames`.
    ///
    /// The engine computes its step once per render block, so this is what it
    /// actually calls; [`Jog::advance`] is the one-frame case and shares this
    /// code so the two cannot drift apart.
    ///
    /// The smoothing is the per-frame one-pole applied `frames` times, worked
    /// out in closed form rather than looped. Doing it any other way would
    /// make the feel of the wheel depend on the audio buffer size, which is a
    /// setting a DJ picks for latency and should not change how a platter
    /// behaves.
    pub fn advance_block(&mut self, frames: usize, playing: bool) -> JogEffect {
        let frames = frames.max(1);
        let n = frames as f64;

        // Speed first, from whatever arrived since the last block. Dividing by
        // the block's length in seconds is what makes this a speed rather than
        // a count, and therefore independent of how often the controller talks
        // and of how long the block is.
        let instant = self.pending * self.device_rate / n;
        let alpha = 1.0 - (1.0 - self.smoothing).powi(frames.min(i32::MAX as usize) as i32);
        self.velocity += (instant - self.velocity) * alpha;

        let scrubbing = !playing || (self.touched && self.mode == JogMode::Vinyl);
        if scrubbing {
            // Position control: apply the movement now, unsmoothed. Anything
            // else is latency, and latency is what makes a scratch feel like
            // rubber.
            let turns = std::mem::take(&mut self.pending);
            let multiplier = if playing { 1.0 } else { SEARCH_MULTIPLIER };
            return JogEffect::Scrub(turns * PLATTER_SECONDS * self.source_rate * multiplier);
        }

        // Rate control. The movement has been folded into the speed estimate,
        // so it must not also be applied as a position.
        self.pending = 0.0;
        let bend = self.bend();
        if bend == 0.0 {
            JogEffect::Free
        } else {
            JogEffect::Bend(1.0 + bend)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate::DEFAULT;

    fn jog() -> Jog {
        Jog::new(RATE)
    }

    /// Turn the wheel at a steady speed for `seconds`, one frame at a time,
    /// as a real controller and a real audio callback would interleave.
    fn turn_steadily(jog: &mut Jog, revolutions_per_second: f64, seconds: f64, playing: bool) {
        let frames = (RATE.as_f64() * seconds) as usize;
        let per_frame = revolutions_per_second / RATE.as_f64();
        for _ in 0..frames {
            jog.turn(per_frame as f32);
            jog.advance(playing);
        }
    }

    #[test]
    fn a_still_wheel_asks_for_nothing() {
        let mut jog = jog();
        assert_eq!(jog.advance(true), JogEffect::Free);
        assert_eq!(jog.advance(true), JogEffect::Free);
    }

    // -- scratching --------------------------------------------------------

    /// **The number that makes it feel like vinyl.** One turn of the wheel is
    /// one turn of a record: 1.8 seconds of music at 33 1/3 RPM. Every jog
    /// wheel sold is built to that expectation.
    #[test]
    fn one_revolution_is_one_revolution_of_a_record() {
        let mut jog = jog();
        jog.set_touched(true);
        jog.turn(1.0);

        let JogEffect::Scrub(frames) = jog.advance(true) else {
            panic!("a touched platter in vinyl mode scratches");
        };
        assert!(
            (frames - 1.8 * RATE.as_f64()).abs() < 1.0,
            "one turn moved {frames} frames, not 1.8 seconds"
        );
    }

    /// Position control, not rate control: the movement arrives on the frame
    /// it was given, with nothing held back. Smoothing here would be latency,
    /// and latency is what makes a scratch feel like rubber.
    #[test]
    fn a_scratch_is_applied_immediately_and_only_once() {
        let mut jog = jog();
        jog.set_touched(true);
        jog.turn(0.5);

        let JogEffect::Scrub(first) = jog.advance(true) else {
            panic!("expected a scrub");
        };
        assert!(first > 0.0);
        assert_eq!(
            jog.advance(true),
            JogEffect::Scrub(0.0),
            "the same movement was applied twice"
        );
    }

    #[test]
    fn turning_back_scratches_back() {
        let mut jog = jog();
        jog.set_touched(true);
        jog.turn(-0.25);
        let JogEffect::Scrub(frames) = jog.advance(true) else {
            panic!("expected a scrub");
        };
        assert!(frames < 0.0, "backwards is {frames}");
    }

    /// In CDJ mode the top of the platter is not a record. Touching it and
    /// turning bends, exactly as it would at the side.
    #[test]
    fn a_touched_platter_in_cdj_mode_bends_rather_than_scratching() {
        let mut jog = jog();
        jog.set_mode(JogMode::Cdj);
        jog.set_touched(true);
        turn_steadily(&mut jog, 0.05, 0.5, true);

        assert!(
            matches!(jog.advance(true), JogEffect::Bend(_)),
            "CDJ mode scratched"
        );
    }

    /// Movement that has not been applied yet must not arrive after the hand
    /// has gone -- that is the record lurching a moment after it was released.
    #[test]
    fn letting_go_drops_movement_that_was_never_applied() {
        let mut jog = jog();
        jog.set_touched(true);
        jog.turn(0.5);
        jog.set_touched(false);

        assert!(
            !matches!(jog.advance(true), JogEffect::Scrub(f) if f.abs() > 0.0),
            "a released platter still lurched"
        );
    }

    // -- searching ---------------------------------------------------------

    /// A paused deck searches whether or not the platter is touched: winding
    /// through a track to find a spot is what the wheel is for when nothing
    /// is playing.
    #[test]
    fn a_paused_deck_searches_without_being_touched() {
        for touched in [false, true] {
            let mut jog = jog();
            jog.set_touched(touched);
            jog.turn(0.1);
            assert!(
                matches!(jog.advance(false), JogEffect::Scrub(f) if f > 0.0),
                "a paused deck did not search (touched: {touched})"
            );
        }
    }

    /// Searching at record speed would be three minutes of winding to cross a
    /// five-minute track. Eight times over is a few flicks of the wrist.
    #[test]
    fn searching_is_faster_than_scratching() {
        let mut scratching = jog();
        scratching.set_touched(true);
        scratching.turn(1.0);
        let JogEffect::Scrub(scratched) = scratching.advance(true) else {
            panic!("expected a scrub");
        };

        let mut searching = jog();
        searching.turn(1.0);
        let JogEffect::Scrub(searched) = searching.advance(false) else {
            panic!("expected a search");
        };

        assert!(
            (searched / scratched - SEARCH_MULTIPLIER).abs() < 0.01,
            "searching is {}x scratching",
            searched / scratched
        );
    }

    // -- bending -----------------------------------------------------------

    /// **The point of a bend.** Pushing the wheel forwards has to make the
    /// deck run faster while the hand is on it, and nudging back has to slow
    /// it down.
    #[test]
    fn pushing_forwards_speeds_the_deck_up_and_back_slows_it() {
        let mut forwards = jog();
        turn_steadily(&mut forwards, 0.05, 0.5, true);
        let JogEffect::Bend(faster) = forwards.advance(true) else {
            panic!("a turning wheel on a playing deck bends");
        };
        assert!(faster > 1.0, "pushing forwards gave {faster}");

        let mut backwards = jog();
        turn_steadily(&mut backwards, -0.05, 0.5, true);
        let JogEffect::Bend(slower) = backwards.advance(true) else {
            panic!("expected a bend");
        };
        assert!(slower < 1.0, "nudging back gave {slower}");
    }

    /// **The call-rate trap.** A bend is a speed, and a speed cannot depend on
    /// how chatty the controller is. A device sending one message per frame
    /// and one sending a message every tenth frame, moving the wheel at the
    /// same speed, must produce the same bend -- otherwise the same hardware
    /// would feel different on a busier USB bus.
    #[test]
    fn a_chattier_controller_does_not_bend_further() {
        let seconds = 0.5;
        let speed = 0.05;

        let mut chatty = jog();
        turn_steadily(&mut chatty, speed, seconds, true);

        let mut sparse = jog();
        let frames = (RATE.as_f64() * seconds) as usize;
        for frame in 0..frames {
            // The same total movement, in a tenth as many messages.
            if frame % 10 == 0 {
                sparse.turn((speed * 10.0 / RATE.as_f64()) as f32);
            }
            sparse.advance(true);
        }

        let difference = (chatty.bend() - sparse.bend()).abs();
        assert!(
            difference < 0.005,
            "chatty bent {} and sparse bent {}",
            chatty.bend(),
            sparse.bend()
        );
    }

    /// A bend is a nudge, not a speed control. Past about a fifth a listener
    /// hears the pitch move rather than the beats line up -- so spinning the
    /// wheel hard in CDJ mode is clamped rather than becoming a scratch, which
    /// is what a real CDJ does too.
    #[test]
    fn a_hard_spin_is_clamped_rather_than_becoming_a_scratch() {
        let mut jog = jog();
        jog.set_mode(JogMode::Cdj);
        turn_steadily(&mut jog, 20.0, 0.5, true);

        let JogEffect::Bend(bend) = jog.advance(true) else {
            panic!("expected a bend");
        };
        assert!(
            (bend - (1.0 + MAX_BEND)).abs() < 1e-9,
            "a hard spin gave {bend}"
        );
    }

    /// **A bend has to let go.** It is applied while the hand is moving; a
    /// bend that only changed when a message arrived would stay applied
    /// forever after the last one, and the deck would never come back to its
    /// own tempo.
    #[test]
    fn a_bend_decays_when_the_hand_stops() {
        let mut jog = jog();
        turn_steadily(&mut jog, 0.05, 0.5, true);
        let pushed = jog.bend();
        assert!(pushed > 0.0);

        // Half a second of nothing.
        for _ in 0..(RATE.as_f64() * 0.5) as usize {
            jog.advance(true);
        }

        // Half a second is 4.17 of these time constants, and a one-pole leaves
        // e^-4.17 -- about 1.5% -- so the test is against what a listener could
        // hear rather than against a round number. Half a percent is roughly
        // the smallest pitch change anyone notices on a sustained note; the
        // deck is back to its own tempo as far as the room is concerned.
        assert!(
            jog.bend().abs() < 0.005,
            "the bend was still {} after half a second (from {pushed})",
            jog.bend()
        );
    }

    /// The time constant is what makes the bend bridge the gaps between a
    /// controller's messages. Measured rather than asserted from the formula:
    /// one time constant of a one-pole covers 1 - 1/e, about 63%.
    #[test]
    fn the_bend_reaches_most_of_the_way_in_its_time_constant() {
        let mut jog = jog();
        let speed = 0.05;
        turn_steadily(&mut jog, speed, BEND_TIME_CONSTANT, true);

        let settled = speed * PLATTER_SECONDS;
        let reached = jog.bend() / settled;
        assert!(
            (0.55..0.70).contains(&reached),
            "one time constant reached {reached} of the way, not about 0.63"
        );
    }

    /// A track at a different sample rate has a different number of frames in
    /// 1.8 seconds, and a scratch that ignored that would move the wrong
    /// distance through the music.
    #[test]
    fn a_scratch_follows_the_tracks_own_sample_rate() {
        let mut jog = jog();
        jog.set_source_rate(SampleRate::new(44_100).unwrap());
        jog.set_touched(true);
        jog.turn(1.0);

        let JogEffect::Scrub(frames) = jog.advance(true) else {
            panic!("expected a scrub");
        };
        assert!(
            (frames - 1.8 * 44_100.0).abs() < 1.0,
            "one turn moved {frames} frames of a 44.1 kHz track"
        );
    }

    /// Nonsense from a controller must not poison the wheel: once a NaN is in
    /// the speed estimate every later comparison is false and the platter
    /// never works again.
    #[test]
    fn nonsense_movement_is_ignored() {
        let mut jog = jog();
        jog.turn(f32::NAN);
        jog.turn(f32::INFINITY);
        jog.advance(true);
        assert!(jog.bend().is_finite(), "the wheel is poisoned");

        turn_steadily(&mut jog, 0.05, 0.2, true);
        assert!(jog.bend() > 0.0, "the wheel stopped working");
    }

    /// **The buffer size is a latency setting, not a feel setting.** A DJ
    /// picks 128 frames or 1024 for how responsive the system is; the platter
    /// must behave the same either way. Turning at one speed for one second
    /// has to reach the same bend however the frames were grouped.
    #[test]
    fn the_block_size_does_not_change_how_the_wheel_feels() {
        let speed = 0.05;
        let seconds = 0.5;

        let mut per_frame = jog();
        turn_steadily(&mut per_frame, speed, seconds, true);

        let mut per_block = jog();
        let block = 256;
        let blocks = (RATE.as_f64() * seconds / f64::from(block)) as usize;
        for _ in 0..blocks {
            per_block.turn((speed * f64::from(block) / RATE.as_f64()) as f32);
            per_block.advance_block(block as usize, true);
        }

        let difference = (per_frame.bend() - per_block.bend()).abs();
        assert!(
            difference < 0.005,
            "per-frame bent {} and per-block bent {}",
            per_frame.bend(),
            per_block.bend()
        );
    }

    /// A block still hands over the movement whole: a scratch is a position,
    /// and losing part of it would be the record slipping under the hand.
    #[test]
    fn a_block_scrubs_everything_that_arrived_during_it() {
        let mut jog = jog();
        jog.set_touched(true);
        jog.turn(0.25);
        jog.turn(0.25);

        let JogEffect::Scrub(frames) = jog.advance_block(256, true) else {
            panic!("expected a scrub");
        };
        assert!(
            (frames - 0.5 * PLATTER_SECONDS * RATE.as_f64()).abs() < 1.0,
            "half a turn moved {frames} frames"
        );
    }

    #[test]
    fn a_mode_round_trips_through_its_name() {
        for mode in [JogMode::Vinyl, JogMode::Cdj] {
            assert_eq!(JogMode::parse(mode.name()), Some(mode));
            assert_eq!(JogMode::parse(&mode.to_string()), Some(mode));
        }
        assert_eq!(JogMode::parse("turntable"), None);
    }

    #[test]
    fn resetting_forgets_the_hand_and_the_movement() {
        let mut jog = jog();
        jog.set_touched(true);
        turn_steadily(&mut jog, 0.05, 0.2, true);
        jog.reset();

        assert!(!jog.is_touched());
        assert_eq!(jog.bend(), 0.0);
        assert_eq!(jog.advance(true), JogEffect::Free);
    }
}

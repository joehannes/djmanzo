//! Talkover: the music steps back while somebody is speaking.
//!
//! A DJ picking up a microphone needs the music to drop far enough to be heard
//! over, and to come back when they stop. Doing it by hand means one hand on
//! the master while trying to talk, which is why every mixer with a mic input
//! has had this since the 1970s.
//!
//! # Why the hold is the part that matters
//!
//! The obvious implementation ducks while the microphone is above a threshold
//! and recovers when it drops below. That fails on the first sentence anybody
//! says, because speech is mostly gaps: the pause between two words is tens of
//! milliseconds of near-silence, and a ducker without a hold surges the music
//! back up into every one of them. The result is a pumping mess that is worse
//! than no ducking at all.
//!
//! So the gate stays open for a **hold** after the microphone falls quiet —
//! long enough to bridge the gaps inside a sentence, short enough that the
//! music comes back promptly at the end of one. Half a second does both.
//!
//! # Why attack is fast and release is slow
//!
//! The same asymmetry as a limiter, for a different reason. A DJ who starts
//! talking wants the music down *now*, so the first syllable is not lost — that
//! is the attack, and it is fast. Coming back is a musical event the room hears,
//! and a fast one sounds like a mistake, so release is slow.
//!
//! # Why the detector is peak rather than RMS
//!
//! Speech starts with a consonant, which is a transient. An RMS detector
//! averages it away and opens a syllable late. Peak detection with a short
//! release opens on the plosive, which is exactly when the ducking is wanted.
//!
//! # Realtime rules
//!
//! Everything is sized at construction. `process` is a handful of multiplies
//! and one comparison per frame — no allocation, no branch on anything
//! unbounded.

/// How long the gate stays open after the microphone falls quiet, in seconds.
///
/// Bridges the gaps between words without holding the music down after a
/// sentence ends. See the module note: this is the constant that decides
/// whether talkover is usable.
const HOLD_SECONDS: f32 = 0.5;

/// How fast the peak detector forgets, for a 1/e step, in seconds.
///
/// Short, so the detector tracks the envelope of speech rather than smearing a
/// shout across the next two seconds. The *hold* is what provides continuity;
/// making the detector slow as well would double up on the same job and leave
/// the music down long after anybody stopped talking.
const DETECTOR_RELEASE_SECONDS: f32 = 0.050;

/// The detector's decay is deliberately far shorter than the hold.
///
/// If it were not, the hold would be doing nothing and this module's central
/// claim would be false — the detector alone would be bridging the gaps between
/// words, and shortening the hold to nothing would not change the sound. A
/// compile-time check rather than a test, because it is a property of two
/// constants and nothing at runtime can make it true or false.
const _: () = assert!(HOLD_SECONDS > DETECTOR_RELEASE_SECONDS * 5.0);

/// Below this the ducker is at rest and stops doing arithmetic on it.
///
/// A one-pole approaches its target geometrically and never arrives, so without
/// a snap the music sits a hair below unity forever and the reduction meter
/// shows a permanent fraction of a decibel — which reads as a stuck ducker.
const GAIN_EPSILON: f32 = 1e-3;

/// The music, ducked by a sidechain.
///
/// Give it the microphone and it gives back the gain to apply to everything
/// else. It deliberately does **not** apply that gain itself: the caller knows
/// which signal is the music and which is the microphone, and a ducker that
/// ducked its own sidechain would be a gate.
#[derive(Debug)]
pub struct Ducker {
    sample_rate: f32,
    /// Sidechain level above which the music steps back, linear.
    threshold: f32,
    /// Gain applied to the music when fully ducked, linear.
    depth: f32,
    /// Per-frame coefficients for the two directions.
    attack: f32,
    release: f32,
    attack_ms: f32,
    release_ms: f32,
    detector_release: f32,
    hold_frames: u32,

    /// Frames left on the hold.
    held: u32,
    /// The sidechain's envelope.
    detector: f32,
    /// The gain currently applied to the music.
    gain: f32,
    /// Whether talkover is switched on at all.
    enabled: bool,
}

impl Ducker {
    /// Default depth, in decibels. Twelve is the number most mixers ship with:
    /// enough to speak over comfortably, not so much that the room thinks the
    /// music stopped.
    pub const DEFAULT_DEPTH_DB: f32 = -12.0;
    /// Default threshold, in decibels. Below a raised voice on a mic set to a
    /// sensible gain, above room noise and a kick drum leaking into it.
    pub const DEFAULT_THRESHOLD_DB: f32 = -30.0;
    /// Default attack, in milliseconds.
    pub const DEFAULT_ATTACK_MS: f32 = 15.0;
    /// Default release, in milliseconds.
    pub const DEFAULT_RELEASE_MS: f32 = 400.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let mut ducker = Ducker {
            sample_rate,
            threshold: db_to_linear(Self::DEFAULT_THRESHOLD_DB),
            depth: db_to_linear(Self::DEFAULT_DEPTH_DB),
            attack: 0.0,
            release: 0.0,
            attack_ms: Self::DEFAULT_ATTACK_MS,
            release_ms: Self::DEFAULT_RELEASE_MS,
            detector_release: coefficient(DETECTOR_RELEASE_SECONDS, sample_rate),
            hold_frames: (HOLD_SECONDS * sample_rate) as u32,
            held: 0,
            detector: 0.0,
            gain: 1.0,
            enabled: true,
        };
        ducker.set_attack_ms(Self::DEFAULT_ATTACK_MS);
        ducker.set_release_ms(Self::DEFAULT_RELEASE_MS);
        ducker
    }

    /// How far the music drops, in decibels. Positive values are refused as
    /// negative ones: ducking that made the music *louder* is never what was
    /// meant, and a mixer that boosted the room when the DJ spoke would be
    /// memorable for the wrong reason.
    pub fn set_depth_db(&mut self, db: f32) {
        self.depth = db_to_linear(-db.abs().min(60.0));
    }

    /// The level the sidechain has to reach, in decibels.
    pub fn set_threshold_db(&mut self, db: f32) {
        self.threshold = db_to_linear(db.clamp(-80.0, 0.0));
    }

    /// How fast the music steps back, in milliseconds.
    pub fn set_attack_ms(&mut self, ms: f32) {
        self.attack_ms = ms.clamp(1.0, 500.0);
        self.attack = coefficient(self.attack_ms / 1000.0, self.sample_rate);
    }

    /// How fast it comes back, in milliseconds.
    pub fn set_release_ms(&mut self, ms: f32) {
        self.release_ms = ms.clamp(10.0, 5000.0);
        self.release = coefficient(self.release_ms / 1000.0, self.sample_rate);
    }

    /// The times as they were set, rather than as coefficients.
    ///
    /// Kept alongside the coefficients rather than recovered from them: the
    /// conversion is a logarithm and rounds, so a panel showing 400 ms would
    /// slowly drift to 399 every time the value went out and came back.
    #[must_use]
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    #[must_use]
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }

    /// Switch talkover off without forgetting how it was set up.
    ///
    /// Switching off does not slam the gain back to unity — it releases at the
    /// normal rate, because a DJ turning talkover off mid-sentence should not
    /// produce a step in the master.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn depth_db(&self) -> f32 {
        linear_to_db(self.depth)
    }

    #[must_use]
    pub fn threshold_db(&self) -> f32 {
        linear_to_db(self.threshold)
    }

    /// The gain currently applied to the music, linear.
    #[must_use]
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// How far the music is down right now, in decibels. Zero at rest.
    #[must_use]
    pub fn reduction_db(&self) -> f32 {
        -linear_to_db(self.gain)
    }

    /// Whether the gate is open — speaking, or inside the hold after it.
    #[must_use]
    pub fn is_ducking(&self) -> bool {
        self.held > 0
    }

    /// Forget everything. For a device change, where the old envelope describes
    /// a signal that no longer exists.
    pub fn reset(&mut self) {
        self.held = 0;
        self.detector = 0.0;
        self.gain = 1.0;
    }

    /// One frame of sidechain in, the music's gain out.
    ///
    /// The caller multiplies its own music by the return value. Nothing here
    /// touches the sidechain itself.
    pub fn process_frame(&mut self, sidechain: f32) -> f32 {
        // A peak follower: rises instantly, falls at the detector's rate. See
        // the module note on why this is not RMS.
        let level = sidechain.abs();
        self.detector = if level > self.detector {
            level
        } else {
            self.detector + (level - self.detector) * self.detector_release
        };

        if self.enabled && self.detector >= self.threshold {
            self.held = self.hold_frames;
        } else {
            self.held = self.held.saturating_sub(1);
        }

        let target = if self.held > 0 { self.depth } else { 1.0 };
        // Down is the attack, up is the release. Named for what the *music*
        // does, which is the opposite of what a compressor's attack describes,
        // and the reason those two words are worth defining here.
        let coefficient = if target < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain += (target - self.gain) * coefficient;

        // Snap at the top, so at rest the ducker is genuinely at rest and the
        // meter reads zero rather than a permanent hundredth of a decibel.
        if self.held == 0 && (1.0 - self.gain).abs() < GAIN_EPSILON {
            self.gain = 1.0;
        }
        self.gain
    }
}

/// A one-pole coefficient for a given 1/e time.
fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
    if seconds <= 0.0 {
        return 1.0;
    }
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 1e-6 {
        -120.0
    } else {
        20.0 * linear.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// Run `frames` of a steady sidechain through and give back the final gain.
    fn run(ducker: &mut Ducker, sidechain: f32, frames: usize) -> f32 {
        let mut gain = 1.0;
        for _ in 0..frames {
            gain = ducker.process_frame(sidechain);
        }
        gain
    }

    fn ms(count: f32) -> usize {
        (RATE * count / 1000.0) as usize
    }

    #[test]
    fn silence_leaves_the_music_alone() {
        let mut ducker = Ducker::new(RATE);
        assert_eq!(run(&mut ducker, 0.0, ms(1000.0)), 1.0);
        assert_eq!(ducker.reduction_db(), 0.0);
        assert!(!ducker.is_ducking());
    }

    #[test]
    fn speech_ducks_the_music_to_the_depth_it_was_given() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_depth_db(-12.0);
        // Long enough for the attack to settle.
        let gain = run(&mut ducker, 0.5, ms(200.0));
        assert!(
            (gain - db_to_linear(-12.0)).abs() < 0.005,
            "gain {gain} is not -12 dB"
        );
        assert!((ducker.reduction_db() - 12.0).abs() < 0.1);
        assert!(ducker.is_ducking());
    }

    #[test]
    fn the_music_comes_back_when_the_talking_stops() {
        let mut ducker = Ducker::new(RATE);
        run(&mut ducker, 0.5, ms(200.0));
        // Hold, then release. Generous, because both are deliberately slow.
        let gain = run(&mut ducker, 0.0, ms(4000.0));
        assert_eq!(gain, 1.0, "the music never came back");
        assert!(!ducker.is_ducking());
    }

    /// **The test this module exists for.** Speech is mostly gaps; a ducker
    /// without a hold surges the music back up into every one of them, and the
    /// result pumps so badly it is worse than no ducking at all.
    ///
    /// The gap has to be longer than the detector's own decay or this passes
    /// with no hold at all. From a half-scale voice down to a −30 dB threshold
    /// the peak detector takes about 140 ms, so a shorter gap is bridged by the
    /// detector and the test proves nothing. 300 ms is past that and still
    /// inside the 500 ms hold, so only the hold can keep the music down — which
    /// is what a mutation confirmed: shortening the hold to a single frame left
    /// the 120 ms version of this test green.
    #[test]
    fn a_gap_between_two_words_does_not_let_the_music_back_up() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_depth_db(-12.0);
        run(&mut ducker, 0.5, ms(200.0));
        let ducked = ducker.gain();

        let after = run(&mut ducker, 0.0, ms(300.0));
        assert!(
            (after - ducked).abs() < 0.01,
            "the music surged from {ducked} to {after} inside a sentence"
        );
        assert!(ducker.is_ducking(), "the hold should still be open");
    }

    /// And the other half of that: the hold must actually end, or the music
    /// stays down after the DJ has finished speaking.
    ///
    /// The wait is longer than `HOLD_SECONDS` on purpose. The hold is measured
    /// from when the *detector* falls below the threshold, not from when the
    /// sound stops, and the detector takes its own release time to get there —
    /// about 140 ms from a half-scale shout down to −30 dB. So the effective
    /// hold after loud speech is a little longer than after quiet speech, which
    /// is worth knowing and is not a bug: a raised voice is usually a longer
    /// sentence.
    #[test]
    fn the_hold_ends_after_a_sentence_does() {
        let mut ducker = Ducker::new(RATE);
        run(&mut ducker, 0.5, ms(200.0));
        run(&mut ducker, 0.0, ms(800.0));
        assert!(!ducker.is_ducking(), "the hold never ended");
        assert!(
            ducker.gain() > db_to_linear(-12.0),
            "recovery never started"
        );
    }

    /// Room noise and a kick drum leaking into an open microphone must not duck
    /// the music, or the mix breathes all night with nobody speaking.
    #[test]
    fn a_signal_below_the_threshold_is_ignored() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_threshold_db(-30.0);
        // -40 dB: audible in a quiet room, well under a voice.
        assert_eq!(run(&mut ducker, db_to_linear(-40.0), ms(500.0)), 1.0);
        assert!(!ducker.is_ducking());
    }

    #[test]
    fn the_threshold_is_where_it_says_it_is() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_threshold_db(-20.0);
        assert!((ducker.threshold_db() - (-20.0)).abs() < 0.01);
        // A hair over the threshold opens it; a hair under does not.
        assert!(run(&mut ducker, db_to_linear(-19.0), ms(50.0)) < 1.0);

        let mut quiet = Ducker::new(RATE);
        quiet.set_threshold_db(-20.0);
        assert_eq!(run(&mut quiet, db_to_linear(-21.0), ms(50.0)), 1.0);
    }

    /// Attack is fast so the first syllable is not lost, release is slow
    /// because the room hears it. If those two were the same the feature would
    /// be either late or obvious.
    ///
    /// Measured rather than asserted against two separate thresholds: each side
    /// is timed to cover half the distance, and the two times compared. That
    /// way the test says what it means — *this direction is faster than that
    /// one* — instead of encoding two numbers that both happen to pass.
    #[test]
    fn attack_is_faster_than_release() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_depth_db(-12.0);
        let depth = db_to_linear(-12.0);
        let halfway_down = (1.0 + depth) / 2.0;

        let mut down_frames = 0usize;
        while ducker.process_frame(0.5) > halfway_down {
            down_frames += 1;
            assert!(down_frames < ms(5000.0), "the attack never got halfway");
        }

        // Settle fully, then let the hold expire so release can start.
        run(&mut ducker, 0.5, ms(1000.0));
        run(&mut ducker, 0.0, ms(HOLD_SECONDS * 1000.0 + 200.0));

        let mut up_frames = 0usize;
        while ducker.process_frame(0.0) < halfway_down {
            up_frames += 1;
            assert!(up_frames < ms(10_000.0), "the release never got halfway");
        }

        assert!(
            up_frames > down_frames * 4,
            "attack took {down_frames} frames and release {up_frames}; \
             release should be much slower"
        );
    }

    /// Ducking that made the music *louder* is never what was meant.
    #[test]
    fn a_positive_depth_still_ducks() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_depth_db(12.0);
        assert!((ducker.depth_db() - (-12.0)).abs() < 0.01);
        assert!(run(&mut ducker, 0.5, ms(200.0)) < 1.0);
    }

    #[test]
    fn switched_off_it_does_nothing() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_enabled(false);
        assert_eq!(run(&mut ducker, 0.9, ms(500.0)), 1.0);
        assert!(!ducker.is_ducking());
    }

    /// Switching off mid-sentence must not slam the master back to unity: that
    /// is a step in the output, which is a click.
    #[test]
    fn switching_off_releases_rather_than_jumping() {
        let mut ducker = Ducker::new(RATE);
        ducker.set_depth_db(-12.0);
        run(&mut ducker, 0.5, ms(200.0));
        let ducked = ducker.gain();

        ducker.set_enabled(false);
        let one_frame = ducker.process_frame(0.5);
        assert!(
            one_frame < ducked + 0.01,
            "gain jumped from {ducked} to {one_frame}"
        );
        // ...and it does get all the way back.
        assert_eq!(run(&mut ducker, 0.5, ms(4000.0)), 1.0);
    }

    #[test]
    fn a_reset_forgets_the_envelope() {
        let mut ducker = Ducker::new(RATE);
        run(&mut ducker, 0.5, ms(200.0));
        assert!(ducker.is_ducking());
        ducker.reset();
        assert_eq!(ducker.gain(), 1.0);
        assert!(!ducker.is_ducking());
    }

    /// A sample rate of zero is a device lying about itself, and dividing by it
    /// would produce a NaN gain — silence, or worse, on the master.
    #[test]
    fn a_nonsense_sample_rate_does_not_produce_nan() {
        let mut ducker = Ducker::new(0.0);
        let gain = run(&mut ducker, 0.5, 100);
        assert!(gain.is_finite(), "gain went to {gain}");
    }

    /// The whole point is that the ducker leaves the microphone alone: it
    /// reports a gain for somebody else's signal and never touches its own
    /// input. A ducker that ducked its sidechain would be a gate.
    #[test]
    fn it_returns_a_gain_and_never_touches_the_sidechain() {
        let mut ducker = Ducker::new(RATE);
        let sidechain = 0.7;
        let gain = ducker.process_frame(sidechain);
        assert_eq!(sidechain, 0.7);
        assert!(gain <= 1.0);
    }
}

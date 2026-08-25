//! Following a tempo and phase that somebody else owns.
//!
//! The tempo itself is [`dj_core::Bpm`], not a type of this crate's own. A
//! network master's tempo and a deck's tempo have to be the same thing or
//! syncing to one would mean converting between two ideas of what a tempo is
//! -- and this crate previously had its own unbounded `Tempo`, which is how
//! `MidiClockOut::advance` came to be able to spin forever on a tempo of
//! `f64::MAX`. `Bpm` is bounded to 20..=400, so a beat period cannot collapse.

use dj_core::Bpm;

/// How much of the phase error to take out on each observation.
///
/// Deliberately small. A network peer's phase arrives late and jittered, so a
/// correction that trusted one observation would chase the jitter; taking a
/// twelfth of the error each time converges over a bar or so and ignores a
/// single bad packet.
const PHASE_CORRECTION: f64 = 0.08;

/// The largest tempo disagreement worth acting on, as a fraction.
///
/// Six percent is wider than any two records a DJ would beatmatch by hand and
/// far outside normal clock drift, so a bigger difference means the peer is
/// playing something else -- or is reporting nonsense -- and following it
/// would drag the deck off its own tempo.
const MAX_TEMPO_ERROR: f64 = 0.06;

/// How much of an accepted tempo disagreement to absorb per observation.
const TEMPO_CORRECTION: f64 = 0.05;

/// The most the follower will ask a deck to change speed, as a fraction.
///
/// One percent is about the limit of what passes unnoticed on a sustained
/// note; beyond it a listener hears the pitch move rather than the beats line
/// up, which defeats the purpose.
const MAX_RATE_NUDGE: f64 = 0.01;

/// A bounded phase and tempo follower for a network master.
///
/// It corrects a little on each update rather than jumping a deck; the caller
/// turns [`PhaseFollower::observe`]'s return into a temporary pitch nudge.
#[derive(Debug, Clone, Copy)]
pub struct PhaseFollower {
    phase: f64,
    tempo: Bpm,
}

impl PhaseFollower {
    #[must_use]
    pub fn new(tempo: Bpm) -> Self {
        Self { phase: 0.0, tempo }
    }

    /// Move the local phase on by `seconds`.
    ///
    /// A non-finite or negative span is ignored rather than propagated: it can
    /// only come from a clock that went backwards, and letting a NaN in would
    /// poison the phase permanently.
    pub fn advance(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.phase = (self.phase + seconds / self.tempo.beat_seconds()).rem_euclid(1.0);
        }
    }

    /// Apply one peer observation and return a safe fractional rate nudge.
    ///
    /// The phase error is taken the short way round the circle, so a peer at
    /// 0.01 against a local 0.99 is two hundredths ahead rather than
    /// ninety-eight hundredths behind.
    pub fn observe(&mut self, peer_phase: f64, peer_tempo: Bpm) -> f64 {
        if !peer_phase.is_finite() {
            return 0.0;
        }
        let peer_phase = peer_phase.rem_euclid(1.0);
        let error = (peer_phase - self.phase + 0.5).rem_euclid(1.0) - 0.5;
        self.phase = (self.phase + error * PHASE_CORRECTION).rem_euclid(1.0);

        let tempo_error =
            (peer_tempo.get() / self.tempo.get() - 1.0).clamp(-MAX_TEMPO_ERROR, MAX_TEMPO_ERROR);
        // Back through `Bpm::new`, so the follower can never walk its own
        // tempo out of the range every other part of the project relies on.
        if let Some(tempo) = Bpm::new(self.tempo.get() * (1.0 + tempo_error * TEMPO_CORRECTION)) {
            self.tempo = tempo;
        }

        (tempo_error * 0.25 + error * 0.02).clamp(-MAX_RATE_NUDGE, MAX_RATE_NUDGE)
    }

    #[must_use]
    pub const fn phase(&self) -> f64 {
        self.phase
    }

    #[must_use]
    pub const fn tempo(&self) -> Bpm {
        self.tempo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bpm(value: f64) -> Bpm {
        Bpm::new(value).expect("a real tempo")
    }

    #[test]
    fn corrections_take_the_shortest_way_round() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        follower.advance(0.99 * 0.5);
        assert!(follower.observe(0.01, bpm(120.0)) > 0.0);
        assert!(follower.phase() > 0.98, "phase went the long way round");
    }

    /// **The point of the follower.** Repeated observations have to converge
    /// on the peer, or it is not following anything.
    #[test]
    fn the_follower_converges_on_the_peer() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        // A beat is half a second at 120, so an eighth of a second is a
        // quarter of a beat -- a quarter turn away from the peer at 0.5.
        follower.advance(0.125);
        let start = (0.5f64 - follower.phase()).abs();
        assert!(start > 0.2, "the test has to start out of phase, not at it");

        for _ in 0..100 {
            follower.observe(0.5, bpm(120.0));
        }
        let end = (0.5 - follower.phase()).abs();

        assert!(
            end < start * 0.1,
            "phase error only came down from {start} to {end}"
        );
    }

    /// **One bad packet is nearly free.** A peer's phase and tempo arrive over
    /// a network, so a single absurd reading is a thing that happens; it must
    /// not be audible.
    #[test]
    fn one_wild_observation_barely_moves_the_tempo() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        follower.observe(0.0, bpm(400.0));

        let moved = (follower.tempo().get() - 120.0).abs() / 120.0;
        assert!(
            moved < 0.005,
            "one nonsense packet moved the tempo by {:.3}%",
            moved * 100.0
        );
    }

    /// And the other half of the same design, stated so it is not mistaken for
    /// a bug: what is bounded is the **rate** of change, not the destination.
    /// A peer that really has changed tempo is followed there, slowly --
    /// fifty observations of a 400 BPM peer take a 120 BPM follower to about
    /// 139, roughly a third of one percent each time. A follower that refused
    /// to arrive would not be following anything.
    #[test]
    fn sustained_disagreement_is_followed_slowly() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        let mut worst_step = 0.0f64;

        for _ in 0..50 {
            let before = follower.tempo().get();
            follower.observe(0.0, bpm(400.0));
            worst_step = worst_step.max((follower.tempo().get() - before).abs() / before);
        }

        assert!(
            worst_step < 0.005,
            "a single step moved the tempo by {:.3}%",
            worst_step * 100.0
        );
        let arrived = follower.tempo().get();
        assert!(
            (135.0..145.0).contains(&arrived),
            "fifty observations should reach about 139 BPM, got {arrived}"
        );
    }

    /// The returned nudge is what becomes a pitch change, so it is what a
    /// listener would hear. Beyond a percent they hear the pitch move.
    #[test]
    fn the_rate_nudge_never_exceeds_what_a_listener_would_miss() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        for phase in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9] {
            for peer in [20.0, 60.0, 120.0, 400.0] {
                let nudge = follower.observe(phase, bpm(peer));
                assert!(
                    nudge.abs() <= MAX_RATE_NUDGE + 1e-12,
                    "nudge {nudge} at phase {phase} against {peer} BPM"
                );
            }
        }
    }

    /// A peer that reports nonsense must not poison the phase. Once a NaN is
    /// in, every later comparison is false and the follower never recovers.
    #[test]
    fn a_nonsense_peer_phase_is_ignored() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        follower.advance(0.25);
        let before = follower.phase();

        assert_eq!(follower.observe(f64::NAN, bpm(120.0)), 0.0);
        assert_eq!(follower.phase(), before, "NaN moved the phase");

        follower.observe(0.5, bpm(120.0));
        assert!(follower.phase().is_finite(), "the phase is poisoned");
    }

    /// Time only goes forwards. A clock that jumps back, or a NaN span,
    /// leaves the phase where it was rather than taking it somewhere.
    #[test]
    fn a_clock_that_went_backwards_is_ignored() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        follower.advance(0.1);
        let before = follower.phase();
        follower.advance(-1.0);
        follower.advance(f64::NAN);
        follower.advance(f64::INFINITY);
        assert_eq!(follower.phase(), before);
    }

    #[test]
    fn advancing_a_whole_beat_returns_to_the_same_phase() {
        let mut follower = PhaseFollower::new(bpm(120.0));
        follower.advance(0.5);
        assert!(follower.phase().abs() < 1e-9, "got {}", follower.phase());
    }
}

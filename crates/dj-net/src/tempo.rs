/// A finite musical tempo in beats per minute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo(f64);

impl Tempo {
    #[must_use]
    pub fn new(bpm: f64) -> Option<Self> {
        (bpm.is_finite() && bpm > 0.0).then_some(Self(bpm))
    }
    #[must_use]
    pub const fn bpm(self) -> f64 {
        self.0
    }
    #[must_use]
    pub fn beat_period_seconds(self) -> f64 {
        60.0 / self.0
    }
}

/// A bounded phase/tempo follower for a network master.
///
/// It deliberately corrects a little each update rather than jumping a deck;
/// the caller may turn `rate_adjustment` into a temporary pitch correction.
#[derive(Debug, Clone, Copy)]
pub struct PhaseFollower {
    phase: f64,
    tempo: Tempo,
}

impl PhaseFollower {
    #[must_use]
    pub fn new(tempo: Tempo) -> Self {
        Self { phase: 0.0, tempo }
    }
    pub fn advance(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.phase = (self.phase + seconds / self.tempo.beat_period_seconds()).rem_euclid(1.0);
        }
    }
    /// Applies one peer observation and returns a safe fractional rate nudge.
    pub fn observe(&mut self, peer_phase: f64, peer_tempo: Tempo) -> f64 {
        let error = (peer_phase - self.phase + 0.5).rem_euclid(1.0) - 0.5;
        self.phase = (self.phase + error * 0.08).rem_euclid(1.0);
        let tempo_error = (peer_tempo.bpm() / self.tempo.bpm() - 1.0).clamp(-0.06, 0.06);
        self.tempo = Tempo(self.tempo.bpm() * (1.0 + tempo_error * 0.05));
        (tempo_error * 0.25 + error * 0.02).clamp(-0.01, 0.01)
    }
    #[must_use]
    pub const fn phase(&self) -> f64 {
        self.phase
    }
    #[must_use]
    pub const fn tempo(&self) -> Tempo {
        self.tempo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrections_take_the_shortest_way_round() {
        let mut follower = PhaseFollower::new(Tempo::new(120.0).unwrap());
        follower.advance(0.99 * 0.5);
        assert!(follower.observe(0.01, Tempo::new(120.0).unwrap()) > 0.0);
        assert!(follower.phase() > 0.98);
    }
}

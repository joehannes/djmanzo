use crate::tempo::Tempo;
use std::time::{Duration, Instant};

/// MIDI timing clock has exactly 24 pulses per quarter note.
pub const MIDI_CLOCK_TICKS_PER_BEAT: u32 = 24;

/// Schedules MIDI clock bytes from an audio/control clock without cumulative drift.
#[derive(Debug)]
pub struct MidiClockOut {
    tempo: Tempo,
    until_next: f64,
}
impl MidiClockOut {
    #[must_use]
    pub fn new(tempo: Tempo) -> Self {
        let until_next = tempo.beat_period_seconds() / f64::from(MIDI_CLOCK_TICKS_PER_BEAT);
        Self { tempo, until_next }
    }
    pub fn set_tempo(&mut self, tempo: Tempo) {
        self.tempo = tempo;
    }
    /// Returns one `0xf8` per tick that falls in this duration.
    pub fn advance(&mut self, elapsed: Duration) -> usize {
        let tick = self.tempo.beat_period_seconds() / f64::from(MIDI_CLOCK_TICKS_PER_BEAT);
        self.until_next -= elapsed.as_secs_f64();
        let mut ticks = 0;
        while self.until_next <= 0.0 {
            ticks += 1;
            self.until_next += tick;
        }
        ticks
    }
}

/// Estimates an external MIDI clock tempo. Invalid / wildly discontinuous pulses reset it.
#[derive(Debug, Default)]
pub struct MidiClockIn {
    previous: Option<Instant>,
    intervals: Vec<Duration>,
}
impl MidiClockIn {
    pub fn tick(&mut self, now: Instant) -> Option<Tempo> {
        let previous = self.previous.replace(now)?;
        let interval = now.checked_duration_since(previous)?;
        if !(Duration::from_millis(2)..=Duration::from_secs(1)).contains(&interval) {
            self.intervals.clear();
            return None;
        }
        self.intervals.push(interval);
        if self.intervals.len() > MIDI_CLOCK_TICKS_PER_BEAT as usize {
            self.intervals.remove(0);
        }
        let average = self
            .intervals
            .iter()
            .map(Duration::as_secs_f64)
            .sum::<f64>()
            / self.intervals.len() as f64;
        Tempo::new(60.0 / (average * f64::from(MIDI_CLOCK_TICKS_PER_BEAT)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_24_ticks_per_beat() {
        let mut out = MidiClockOut::new(Tempo::new(120.0).unwrap());
        assert_eq!(out.advance(Duration::from_millis(500)), 24);
    }
    #[test]
    fn measures_a_stable_clock() {
        let mut input = MidiClockIn::default();
        let start = Instant::now();
        assert_eq!(input.tick(start), None);
        let mut tempo = None;
        for n in 1..=24 {
            tempo = input.tick(start + Duration::from_micros(n * 20_833));
        }
        assert!((tempo.unwrap().bpm() - 120.0).abs() < 0.1);
    }
}

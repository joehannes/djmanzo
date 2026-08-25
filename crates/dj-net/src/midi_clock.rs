use dj_core::Bpm;
use std::time::{Duration, Instant};

/// MIDI timing clock has exactly 24 pulses per quarter note.
pub const MIDI_CLOCK_TICKS_PER_BEAT: u32 = 24;

/// Schedules MIDI clock bytes from an audio or control clock without
/// cumulative drift.
///
/// The remainder is carried rather than reset, so a caller advancing by an
/// awkward block length -- 256 frames at 48 kHz is 5.333 ms, which is not a
/// whole number of ticks at any tempo -- gets the right number of ticks over
/// time instead of losing a fraction on every call.
#[derive(Debug)]
pub struct MidiClockOut {
    tempo: Bpm,
    until_next: f64,
}

impl MidiClockOut {
    #[must_use]
    pub fn new(tempo: Bpm) -> Self {
        Self {
            tempo,
            until_next: tick_seconds(tempo),
        }
    }

    /// Change tempo without disturbing the tick already in flight.
    ///
    /// The remaining time keeps the old tempo's length: a tick that is
    /// two-thirds elapsed stays two-thirds elapsed. Recomputing it would
    /// either emit a tick early or swallow one, and a receiver counting
    /// twenty-four to the beat would hear the bar move.
    pub fn set_tempo(&mut self, tempo: Bpm) {
        self.tempo = tempo;
    }

    /// How many `0xf8` bytes fall in this span.
    ///
    /// Computed rather than counted round a loop. The loop this replaced ran
    /// once per tick, so a long span -- or, before the tempo was bounded, an
    /// absurd one whose tick period rounded to zero -- could spin for a very
    /// long time or forever. `Bpm` is bounded now, but a caller that stalls
    /// and then advances by an hour should still return promptly.
    pub fn advance(&mut self, elapsed: Duration) -> usize {
        let tick = tick_seconds(self.tempo);
        let remaining = self.until_next - elapsed.as_secs_f64();
        if remaining > 0.0 {
            self.until_next = remaining;
            return 0;
        }

        // `remaining` is at or below zero, so at least one tick is due. The
        // rest is how many whole tick periods fit in what is left over.
        let overshoot = -remaining;
        let extra = (overshoot / tick).floor();
        let ticks = 1.0 + extra;
        self.until_next = tick - (overshoot - extra * tick);

        // Guard the conversion rather than trusting it: `as usize` on a
        // non-finite or huge float is a silently wrong number, not an error.
        if ticks.is_finite() && ticks >= 0.0 {
            ticks as usize
        } else {
            0
        }
    }
}

/// One MIDI tick, in seconds.
fn tick_seconds(tempo: Bpm) -> f64 {
    tempo.beat_seconds() / f64::from(MIDI_CLOCK_TICKS_PER_BEAT)
}

/// Estimates an external MIDI clock tempo. Invalid / wildly discontinuous pulses reset it.
#[derive(Debug, Default)]
pub struct MidiClockIn {
    previous: Option<Instant>,
    intervals: Vec<Duration>,
}
impl MidiClockIn {
    /// Fold one incoming pulse in, and estimate the sender's tempo.
    ///
    /// `None` until there are two pulses to compare, and again whenever a
    /// gap falls outside what a MIDI clock can plausibly be -- a stopped
    /// sender, or a burst after one -- because averaging across a gap
    /// would report a tempo nobody is playing.
    pub fn tick(&mut self, now: Instant) -> Option<Bpm> {
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
        Bpm::new(60.0 / (average * f64::from(MIDI_CLOCK_TICKS_PER_BEAT)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bpm(value: f64) -> Bpm {
        Bpm::new(value).expect("a real tempo")
    }

    #[test]
    fn emits_24_ticks_per_beat() {
        let mut out = MidiClockOut::new(bpm(120.0));
        assert_eq!(out.advance(Duration::from_millis(500)), 24);
    }

    /// **The claim in the type's name.** Twenty-four to the beat has to hold
    /// over many beats, at a block length that is not a whole number of
    /// ticks -- otherwise the remainder is lost on every call and a receiver
    /// drifts further behind all night.
    #[test]
    fn a_thousand_awkward_blocks_still_come_out_at_24_a_beat() {
        let mut out = MidiClockOut::new(bpm(120.0));
        // 256 frames at 48 kHz: 5.3333... ms, deliberately not a tick.
        let block = Duration::from_secs_f64(256.0 / 48_000.0);
        let blocks = 1_000;

        let ticks: usize = (0..blocks).map(|_| out.advance(block)).sum();

        let seconds = block.as_secs_f64() * f64::from(blocks);
        let expected = seconds / bpm(120.0).beat_seconds() * f64::from(MIDI_CLOCK_TICKS_PER_BEAT);
        assert!(
            (ticks as f64 - expected).abs() <= 1.0,
            "emitted {ticks} ticks where {expected:.2} were due"
        );
    }

    /// A caller that stalls and comes back must not make this spin. The loop
    /// this replaced ran once per tick, so an hour was 172,800 iterations --
    /// and on the unbounded tempo type this crate used to have, a tick period
    /// that rounded to zero never terminated at all.
    #[test]
    fn a_very_long_span_returns_promptly() {
        let mut out = MidiClockOut::new(bpm(400.0));
        let ticks = out.advance(Duration::from_secs(3_600));
        // 400 beats a minute for 60 minutes is 24,000 beats, and 24 ticks to
        // the beat makes 576,000. The loop this replaced would have gone round
        // every one of them.
        assert_eq!(ticks, 576_000);
    }

    #[test]
    fn a_span_shorter_than_a_tick_emits_nothing_but_is_remembered() {
        let mut out = MidiClockOut::new(bpm(120.0));
        let tenth = Duration::from_secs_f64(0.002);
        assert_eq!(out.advance(tenth), 0);
        assert_eq!(out.advance(tenth), 0);
        // 20.833 ms a tick, so the third of these crosses it.
        assert_eq!(out.advance(Duration::from_secs_f64(0.018)), 1);
    }

    #[test]
    fn zero_elapsed_emits_nothing() {
        let mut out = MidiClockOut::new(bpm(120.0));
        assert_eq!(out.advance(Duration::ZERO), 0);
    }

    /// Changing tempo mid-tick must not emit a spurious tick or swallow one.
    #[test]
    fn changing_tempo_keeps_the_tick_in_flight() {
        let mut out = MidiClockOut::new(bpm(120.0));
        assert_eq!(out.advance(Duration::from_secs_f64(0.010)), 0);
        out.set_tempo(bpm(174.0));
        // 10.833 ms of the old tick still to run, and the new tempo does not
        // retroactively shorten it.
        assert_eq!(out.advance(Duration::from_secs_f64(0.005)), 0);
        assert_eq!(out.advance(Duration::from_secs_f64(0.006)), 1);
    }

    #[test]
    fn measures_a_stable_clock() {
        let mut input = MidiClockIn::default();
        let start = Instant::now();
        assert_eq!(input.tick(start), None, "one pulse is not an interval");

        let mut tempo = None;
        for n in 1..=24 {
            tempo = input.tick(start + Duration::from_micros(n * 20_833));
        }
        assert!((tempo.unwrap().get() - 120.0).abs() < 0.1);
    }

    /// A sender that stops and restarts must not be averaged across the gap:
    /// the answer would be a tempo nobody played.
    #[test]
    fn a_gap_resets_the_estimate() {
        let mut input = MidiClockIn::default();
        let start = Instant::now();
        for n in 0..24 {
            input.tick(start + Duration::from_micros(n * 20_833));
        }
        // Two seconds of silence, then the sender comes back.
        assert_eq!(
            input.tick(start + Duration::from_secs(2)),
            None,
            "a gap must not be measured as an interval"
        );

        let resumed = start + Duration::from_secs(2);
        let mut tempo = None;
        for n in 1..=24 {
            tempo = input.tick(resumed + Duration::from_micros(n * 10_416));
        }
        assert!(
            (tempo.unwrap().get() - 240.0).abs() < 1.0,
            "the new tempo was contaminated by the old: {:?}",
            tempo.map(Bpm::get)
        );
    }

    /// A sender faster or slower than a tempo can be reports nothing rather
    /// than a number outside the range the rest of the project relies on.
    #[test]
    fn an_impossible_clock_reports_nothing() {
        let mut input = MidiClockIn::default();
        let start = Instant::now();
        // 2 ms a tick is 1250 BPM -- inside the interval filter, outside `Bpm`.
        input.tick(start);
        let tempo = input.tick(start + Duration::from_millis(2));
        assert_eq!(tempo, None, "1250 BPM is not a tempo");
    }
}

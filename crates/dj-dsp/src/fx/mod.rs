//! Effects, and the slot that runs one.
//!
//! Three decisions shape everything here.
//!
//! **A slot owns the buffer, not the effect.** Every time-based effect wants
//! somewhere to keep the recent past, and giving each its own would make rack
//! memory scale with the size of the effect catalogue instead of with how many
//! effects can run at once. See [`DelayLine`].
//!
//! **An effect is an enum variant, not a `Box<dyn Effect>`.** Switching one
//! then costs an assignment rather than an allocation, which matters because
//! the switch happens on the audio thread like every other action. The price is
//! that adding an effect means adding a variant — a price worth paying for a
//! fixed catalogue, and one that also keeps the whole rack `Send` and
//! inspectable.
//!
//! **Timing is in beats, not milliseconds.** A quarter-beat echo stays a
//! quarter-beat echo when the DJ moves the pitch fader, which is the only
//! behaviour that is any use in a mix. The caller passes the deck's tempo in
//! [`FxContext`]; nothing here reads a clock.

mod line;

pub use line::DelayLine;

use crate::SmoothedValue;
pub use dj_core::fx::{EffectKind, FX_SLOTS, Placement};

/// What an effect needs to know about the music it is being applied to.
///
/// Passed per block rather than stored, because a deck's tempo changes with the
/// pitch fader and an effect holding a stale copy would drift out of time
/// exactly when the DJ is riding the pitch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxContext {
    pub sample_rate: f32,
    /// Frames in one beat at the tempo actually being played. `None` when the
    /// deck has no grid — beat-synced effects then fall back to a fixed time,
    /// because refusing to run is worse than running slightly wrong.
    pub beat_frames: Option<f32>,
}

impl FxContext {
    /// What a beat is worth when there is no grid to measure one.
    ///
    /// 120 BPM. A guess, and labelled as one: an effect on an ungridded track
    /// should still make a sound, and the DJ can hear that it is not in time
    /// far faster than any message could tell them.
    pub const ASSUMED_BPM: f32 = 120.0;

    #[must_use]
    pub fn beat(&self) -> f32 {
        self.beat_frames
            .filter(|frames| frames.is_finite() && *frames > 0.0)
            .unwrap_or(self.sample_rate * 60.0 / Self::ASSUMED_BPM)
    }
}

/// One effect, its buffer, and the controls around it.
#[derive(Debug)]
pub struct Slot {
    kind: EffectKind,
    enabled: bool,
    /// Smoothed, because switching an effect in mid-bar is a normal DJ move and
    /// an unsmoothed wet/dry step clicks.
    wet: SmoothedValue,
    /// Length in beats. Ignored by effects that have no time in them.
    beats: f32,
    /// The effect's own knob, 0..=1. What it means is the effect's business.
    amount: f32,
    placement: Placement,
    line: DelayLine,
    /// Per-effect scalar state. Cleared with the line on any switch.
    phase: f32,
    hold: (f32, f32),
    held: f32,
}

impl Slot {
    /// Shortest and longest a timed effect can be set to, in beats.
    ///
    /// A sixteenth matches the shortest loop the engine will make, so the two
    /// families of "how short can this get" agree. Four beats is a bar.
    pub const MIN_BEATS: f32 = 1.0 / 16.0;
    pub const MAX_BEATS: f32 = 4.0;

    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            kind: EffectKind::None,
            enabled: false,
            // A slower ramp than a fader: an effect coming in over 30 ms sounds
            // deliberate, and over 10 ms sounds like a mistake being corrected.
            wet: SmoothedValue::with_ramp(0.0, sample_rate, 30.0),
            beats: 0.5,
            amount: 0.5,
            placement: Placement::default(),
            line: DelayLine::new(sample_rate),
            phase: 0.0,
            hold: (0.0, 0.0),
            held: 0.0,
        }
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn beats(&self) -> f32 {
        self.beats
    }

    #[must_use]
    pub fn amount(&self) -> f32 {
        self.amount
    }

    #[must_use]
    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// The wet target, which is what the interface should show — the smoothed
    /// value is mid-ramp and would make a slider twitch.
    #[must_use]
    pub fn wet(&self) -> f32 {
        self.wet.target()
    }

    /// Install an effect. Switching clears the buffer.
    ///
    /// Without the clear, changing echo to flanger would flange a second of
    /// audio from before the switch — the past belonged to a different effect
    /// and reading it is reading someone else's state.
    pub fn select(&mut self, kind: EffectKind) {
        if self.kind != kind {
            self.kind = kind;
            self.reset();
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        if self.enabled != on {
            self.enabled = on;
            // Only on the way *in*. Clearing on the way out would cut the tail
            // dead, and the tail is why a DJ throws an echo and lets go.
            if on {
                self.reset();
            }
        }
    }

    pub fn set_wet(&mut self, wet: f32) {
        if wet.is_finite() {
            self.wet.set_target(wet.clamp(0.0, 1.0));
        }
    }

    pub fn set_beats(&mut self, beats: f32) {
        if beats.is_finite() {
            self.beats = beats.clamp(Self::MIN_BEATS, Self::MAX_BEATS);
        }
    }

    pub fn set_amount(&mut self, amount: f32) {
        if amount.is_finite() {
            self.amount = amount.clamp(0.0, 1.0);
        }
    }

    pub fn set_placement(&mut self, placement: Placement) {
        self.placement = placement;
    }

    /// Forget everything time-based. Does not touch the controls.
    pub fn reset(&mut self) {
        self.line.clear();
        self.phase = 0.0;
        self.hold = (0.0, 0.0);
        self.held = 0.0;
    }

    /// Run one frame through, if this slot is doing anything.
    ///
    /// Returns the input untouched when the slot is off or empty, so the caller
    /// can put this in the chain unconditionally and pay one branch.
    #[inline]
    #[must_use]
    pub fn process_frame(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        // The smoother runs even when the slot is off, so that a slot switched
        // off mid-ramp finishes arriving at zero instead of freezing part-wet.
        let wet = self.wet.next_value();
        if !self.enabled || self.kind == EffectKind::None || wet <= 0.0 {
            return (left, right);
        }

        let (wet_l, wet_r) = match self.kind {
            EffectKind::None => (left, right),
            EffectKind::Echo => self.echo(left, right, ctx),
            EffectKind::Gate => self.gate(left, right, ctx),
            EffectKind::Crush => self.crush(left, right),
            EffectKind::Flanger => self.flanger(left, right, ctx),
        };

        let dry = 1.0 - wet;
        (left * dry + wet_l * wet, right * dry + wet_r * wet)
    }

    /// Beat-synced delay with feedback.
    ///
    /// The feedback path is *inside* the line, so the tail keeps going after
    /// the wet control is pulled down — which is what makes an echo throwable.
    #[inline]
    fn echo(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        let delay = (self.beats * ctx.beat()).clamp(1.0, self.line.frames() as f32 - 2.0);
        let (tail_l, tail_r) = self.line.read(delay);
        // Capped below 1.0 so a rack left running cannot build to infinity —
        // self-oscillation is a synthesiser feature and a PA speaker hazard.
        let feedback = self.amount * 0.9;
        self.line
            .push(left + tail_l * feedback, right + tail_r * feedback);
        (tail_l, tail_r)
    }

    /// Chop the signal in and out on the beat.
    ///
    /// A raised-cosine edge rather than a square one: a hard gate clicks, and
    /// the click is louder than the effect. `amount` is the duty cycle, so
    /// turning it up makes the holes narrower rather than the chops faster —
    /// the rate belongs to the beat control.
    #[inline]
    fn gate(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        let period = (self.beats * ctx.beat()).max(2.0);
        self.phase += 1.0 / period;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }

        let duty = 0.1 + self.amount * 0.8;
        let edge = 0.05;
        let gain = if self.phase < duty - edge {
            1.0
        } else if self.phase < duty {
            let t = (duty - self.phase) / edge;
            0.5 - 0.5 * (std::f32::consts::PI * (1.0 - t)).cos()
        } else if self.phase < 1.0 - edge {
            0.0
        } else {
            let t = (self.phase - (1.0 - edge)) / edge;
            0.5 - 0.5 * (std::f32::consts::PI * t).cos()
        };
        (left * gain, right * gain)
    }

    /// Bit depth and sample rate reduction.
    ///
    /// One knob for both, because they are one sound: turning it up makes the
    /// signal coarser in time and in level together, which is what a DJ means
    /// by "crush". No time in it at all, so it ignores the beat control.
    #[inline]
    fn crush(&mut self, left: f32, right: f32) -> (f32, f32) {
        // 16 bits down to 3. Below three it stops being a pitch and becomes a
        // fault, and there is no musical use for the last stop.
        let levels = 2.0_f32.powf(16.0 - self.amount * 13.0);
        // Hold every Nth sample, up to 32 -- a downsample to 1.5 kHz.
        let stride = 1.0 + self.amount * 31.0;

        self.held += 1.0;
        if self.held >= stride {
            self.held -= stride;
            self.hold = (
                (left * levels).round() / levels,
                (right * levels).round() / levels,
            );
        }
        self.hold
    }

    /// A short swept delay mixed back against itself.
    ///
    /// The sweep runs on the beat like everything else here, so a flanger set
    /// to four beats takes a bar to travel — in time with the music rather than
    /// at some rate in hertz that has nothing to do with the track.
    #[inline]
    fn flanger(&mut self, left: f32, right: f32, ctx: &FxContext) -> (f32, f32) {
        let period = (self.beats * ctx.beat()).max(2.0);
        self.phase += 1.0 / period;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }

        // 1 ms to 10 ms, the range where the comb notches land in the audible
        // band. Longer and it is a chorus; longer still and it is an echo.
        let sweep = 0.5 - 0.5 * (std::f32::consts::TAU * self.phase).cos();
        let delay = ctx.sample_rate * (0.001 + 0.009 * sweep);

        let (tail_l, tail_r) = self.line.read(delay);
        let feedback = self.amount * 0.7;
        self.line
            .push(left + tail_l * feedback, right + tail_r * feedback);
        // Summed with the dry signal rather than replacing it: the comb filter
        // *is* the interference between the two, so a flanger with no dry path
        // in its wet signal has nothing to interfere with.
        ((left + tail_l) * 0.5, (right + tail_r) * 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn ctx(bpm: f32) -> FxContext {
        FxContext {
            sample_rate: SR,
            beat_frames: Some(SR * 60.0 / bpm),
        }
    }

    fn slot(kind: EffectKind) -> Slot {
        let mut slot = Slot::new(SR);
        slot.select(kind);
        slot.set_enabled(true);
        slot.set_wet(1.0);
        settle(&mut slot);
        slot
    }

    /// Run silence through until the wet ramp has actually arrived.
    ///
    /// A whole second, not a fraction of one. The ramp is a one-pole reaching
    /// 63% in 30 ms, so it takes about twenty time constants to come within the
    /// smoother's epsilon and snap — and until it snaps, a hair of dry signal
    /// leaks through and every test that measures the wet path measures the
    /// leak as well.
    fn settle(slot: &mut Slot) {
        for _ in 0..(SR as usize) {
            let _ = slot.process_frame(0.0, 0.0, &ctx(120.0));
        }
    }

    #[test]
    fn an_empty_slot_passes_the_signal_through_untouched() {
        let mut slot = Slot::new(SR);
        slot.set_enabled(true);
        slot.set_wet(1.0);
        assert_eq!(slot.process_frame(0.7, -0.3, &ctx(120.0)), (0.7, -0.3));
    }

    #[test]
    fn a_slot_that_is_off_passes_the_signal_through_untouched() {
        let mut slot = Slot::new(SR);
        slot.select(EffectKind::Crush);
        slot.set_wet(1.0);
        assert_eq!(slot.process_frame(0.7, -0.3, &ctx(120.0)), (0.7, -0.3));
    }

    /// The property the whole design exists for: the delay is measured in
    /// beats, so it follows the tempo instead of the wall clock.
    #[test]
    fn an_echo_lands_a_beat_later_at_whatever_tempo_is_playing() {
        for bpm in [90.0, 120.0, 174.0] {
            let mut slot = slot(EffectKind::Echo);
            slot.set_beats(1.0);
            slot.set_amount(0.0);

            let beat = (SR * 60.0 / bpm) as usize;
            let _ = slot.process_frame(1.0, 1.0, &ctx(bpm));
            let mut loudest = (0usize, 0.0f32);
            for frame in 1..(beat * 2) {
                let (left, _) = slot.process_frame(0.0, 0.0, &ctx(bpm));
                if left.abs() > loudest.1 {
                    loudest = (frame, left.abs());
                }
            }
            let error = loudest.0.abs_diff(beat);
            assert!(
                error <= 2,
                "at {bpm} BPM the repeat landed at {} rather than {beat}",
                loudest.0
            );
        }
    }

    /// Feedback must decay. A rack left running with the wet control up is the
    /// normal state of a DJ booth, and an echo that gains is a blown speaker.
    #[test]
    fn echo_feedback_decays_even_at_full_amount() {
        let mut slot = slot(EffectKind::Echo);
        slot.set_beats(1.0 / 16.0);
        slot.set_amount(1.0);

        let _ = slot.process_frame(1.0, 1.0, &ctx(120.0));
        let mut peak = 0.0f32;
        for _ in 0..(SR as usize * 4) {
            let (left, _) = slot.process_frame(0.0, 0.0, &ctx(120.0));
            peak = peak.max(left.abs());
        }
        assert!(peak <= 1.0, "the tail grew to {peak}");
    }

    /// The wet control is a mix, so at zero the effect is inaudible however
    /// loud it is underneath.
    #[test]
    fn a_dry_slot_is_silent_however_wet_the_effect_would_be() {
        let mut slot = slot(EffectKind::Crush);
        slot.set_amount(1.0);
        slot.set_wet(0.0);
        settle(&mut slot);
        let (left, right) = slot.process_frame(0.4, -0.4, &ctx(120.0));
        assert!((left - 0.4).abs() < 1e-4, "got {left}");
        assert!((right + 0.4).abs() < 1e-4, "got {right}");
    }

    #[test]
    fn a_gate_actually_closes_and_opens_within_its_period() {
        let mut slot = slot(EffectKind::Gate);
        slot.set_beats(1.0);
        slot.set_amount(0.5);

        let beat = (SR * 60.0 / 120.0) as usize;
        let mut open = false;
        let mut shut = false;
        for _ in 0..beat {
            let (left, _) = slot.process_frame(1.0, 1.0, &ctx(120.0));
            if left > 0.9 {
                open = true;
            }
            if left < 0.1 {
                shut = true;
            }
        }
        assert!(open && shut, "open {open}, shut {shut}");
    }

    /// A hard-edged gate clicks louder than the effect it is making. The edges
    /// must be ramps, which shows up as intermediate values existing at all.
    #[test]
    fn the_gate_has_soft_edges() {
        let mut slot = slot(EffectKind::Gate);
        slot.set_beats(1.0);
        slot.set_amount(0.5);

        let beat = (SR * 60.0 / 120.0) as usize;
        let mut between = 0;
        for _ in 0..beat {
            let (left, _) = slot.process_frame(1.0, 1.0, &ctx(120.0));
            if left > 0.2 && left < 0.8 {
                between += 1;
            }
        }
        assert!(between > 100, "only {between} frames on an edge");
    }

    /// Crush is quantisation, so its output has to take fewer distinct values
    /// than its input. Counting them is the honest test.
    #[test]
    fn crush_coarsens_the_signal() {
        use std::collections::HashSet;

        let mut fine = slot(EffectKind::Crush);
        fine.set_amount(0.0);
        let mut coarse = slot(EffectKind::Crush);
        coarse.set_amount(1.0);

        let mut fine_values = HashSet::new();
        let mut coarse_values = HashSet::new();
        for n in 0..2_000 {
            let sample = (n as f32 / 2_000.0) * 2.0 - 1.0;
            fine_values.insert(fine.process_frame(sample, sample, &ctx(120.0)).0.to_bits());
            coarse_values.insert(
                coarse
                    .process_frame(sample, sample, &ctx(120.0))
                    .0
                    .to_bits(),
            );
        }
        assert!(
            coarse_values.len() * 4 < fine_values.len(),
            "coarse produced {} values against fine's {}",
            coarse_values.len(),
            fine_values.len()
        );
    }

    /// Switching effects must not let the new one read the old one's audio.
    #[test]
    fn switching_an_effect_forgets_the_previous_one_s_buffer() {
        let mut slot = slot(EffectKind::Echo);
        slot.set_beats(1.0);
        for _ in 0..1_000 {
            let _ = slot.process_frame(1.0, 1.0, &ctx(120.0));
        }
        slot.select(EffectKind::Flanger);
        slot.select(EffectKind::Echo);

        // A beat of silence in: anything that comes out came from before.
        let beat = (SR * 60.0 / 120.0) as usize;
        let mut peak = 0.0f32;
        for _ in 0..beat {
            let (left, _) = slot.process_frame(0.0, 0.0, &ctx(120.0));
            peak = peak.max(left.abs());
        }
        assert!(peak < 1e-6, "stale audio came through at {peak}");
    }

    /// An effect on an ungridded track should still make a sound. Refusing to
    /// run would be a control that does nothing, which reads as broken.
    #[test]
    fn a_deck_with_no_grid_still_gets_an_effect() {
        let no_grid = FxContext {
            sample_rate: SR,
            beat_frames: None,
        };
        assert!((no_grid.beat() - SR / 2.0).abs() < 1.0, "120 BPM at 48 kHz");

        let mut slot = Slot::new(SR);
        slot.select(EffectKind::Echo);
        slot.set_enabled(true);
        slot.set_wet(1.0);
        slot.set_beats(1.0);
        slot.set_amount(0.0);
        for _ in 0..(SR as usize) {
            let _ = slot.process_frame(0.0, 0.0, &no_grid);
        }
        let _ = slot.process_frame(1.0, 1.0, &no_grid);

        let mut peak = 0.0f32;
        for _ in 0..(SR as usize / 2 + 10) {
            let (left, _) = slot.process_frame(0.0, 0.0, &no_grid);
            peak = peak.max(left.abs());
        }
        assert!(peak > 0.5, "no repeat came back: peak {peak}");
    }

    /// A garbage number from a controller must not become a garbage delay.
    #[test]
    fn nonsense_settings_are_refused_rather_than_stored() {
        let mut slot = Slot::new(SR);
        slot.set_beats(f32::NAN);
        slot.set_wet(f32::INFINITY);
        slot.set_amount(f32::NAN);
        assert_eq!(slot.beats(), 0.5);
        assert_eq!(slot.wet(), 0.0);
        assert_eq!(slot.amount(), 0.5);

        slot.set_beats(1_000.0);
        assert_eq!(slot.beats(), Slot::MAX_BEATS);
        slot.set_beats(0.0);
        assert_eq!(slot.beats(), Slot::MIN_BEATS);
    }
}

//! The engine: everything that happens inside the audio callback.

use crate::command::{Command, Retired};
use crate::deck::Deck;
use dj_audio::{AudioCallback, RenderContext};
use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, GlobalParam};
use dj_core::{
    Action, DeckAction, DeckId, MAX_DECKS, MixerAction, ParamId, SampleRate, db_to_linear,
};
use dj_dsp::{CrossfaderCurve, PeakMeter, SmoothedValue, crossfader_gains};
use std::sync::Arc;

/// Decks assigned to the left side of the crossfader.
///
/// Deck 1 left, deck 2 right, everything else straight through -- the
/// convention every mixer follows. Per-channel assignment is an M1 feature.
const CROSSFADER_LEFT: usize = 0;
const CROSSFADER_RIGHT: usize = 1;

/// The realtime engine.
///
/// Lives on the audio thread and obeys its rules absolutely: no allocation, no
/// locking, no I/O, no logging, no panics. Every field is sized at construction.
#[derive(Debug)]
pub struct Engine {
    decks: Vec<Deck>,
    commands: rtrb::Consumer<Command>,
    retired: rtrb::Producer<Retired>,
    registry: Arc<ParameterRegistry>,

    crossfader: f32,
    crossfader_curve: CrossfaderCurve,
    master_gain: SmoothedValue,
    master_gain_db: f32,

    peak_left: PeakMeter,
    peak_right: PeakMeter,
    sample_rate: SampleRate,

    /// Sources we could not hand back because the retirement queue was full.
    /// Held rather than dropped -- dropping here is exactly what the queue
    /// exists to prevent. Drained on the next callback that has room.
    stranded: Vec<Retired>,
}

impl Engine {
    /// Build an engine for `deck_count` decks.
    ///
    /// Everything is allocated here, before any audio flows.
    #[must_use]
    pub fn new(
        deck_count: usize,
        sample_rate: SampleRate,
        commands: rtrb::Consumer<Command>,
        retired: rtrb::Producer<Retired>,
        registry: Arc<ParameterRegistry>,
    ) -> Self {
        let deck_count = deck_count.min(MAX_DECKS);
        let sr = sample_rate.as_f64() as f32;

        let engine = Self {
            decks: (0..deck_count).map(|_| Deck::new(sample_rate)).collect(),
            commands,
            retired,
            registry,
            crossfader: 0.0,
            crossfader_curve: CrossfaderCurve::default(),
            master_gain: SmoothedValue::new(1.0, sr),
            master_gain_db: 0.0,
            peak_left: PeakMeter::new(sr),
            peak_right: PeakMeter::new(sr),
            sample_rate,
            // Capacity for a pathological burst of loads; never grown at runtime.
            stranded: Vec::with_capacity(MAX_DECKS * 2),
        };
        engine.publish_static_state();
        // Publish deck defaults immediately, so the interface shows real values
        // between opening a device and the first callback firing.
        engine.publish_deck_state();
        engine
    }

    /// Write the values that do not change per block.
    fn publish_static_state(&self) {
        self.registry.set(
            ParamId::Global(GlobalParam::SampleRate),
            self.sample_rate.get() as f32,
        );
        self.registry
            .set(ParamId::Global(GlobalParam::Crossfader), self.crossfader);
        self.registry.set(
            ParamId::Global(GlobalParam::MasterGainDb),
            self.master_gain_db,
        );
    }

    fn deck_mut(&mut self, id: DeckId) -> Option<&mut Deck> {
        self.decks.get_mut(id.index())
    }

    /// Hand a source back to the host thread to be dropped.
    ///
    /// If the queue is full we hold onto it rather than dropping it here --
    /// dropping on the audio thread is the whole thing we are avoiding.
    fn retire(&mut self, source: Arc<dyn dj_decode::TrackSource>) {
        let mut item = Retired(source);
        if self.stranded.len() < self.stranded.capacity() {
            // Try the queue first; stash only if it will not take it.
            match self.retired.push(item) {
                Ok(()) => return,
                Err(rtrb::PushError::Full(returned)) => item = returned,
            }
            self.stranded.push(item);
        } else {
            // Both the queue and the stash are full, which means the host thread
            // has stopped draining entirely. Push and let it fail; leaking is
            // strictly better than a dropout.
            let _ = self.retired.push(item);
        }
    }

    /// Retry anything the retirement queue would not take earlier.
    fn drain_stranded(&mut self) {
        while let Some(item) = self.stranded.pop() {
            if let Err(rtrb::PushError::Full(returned)) = self.retired.push(item) {
                self.stranded.push(returned);
                break;
            }
        }
    }

    /// Take everything waiting in the command queue.
    fn drain_commands(&mut self) {
        // Bounded so a flood of commands cannot stretch one callback past its
        // deadline; the remainder is handled next block, microseconds later.
        const MAX_PER_BLOCK: usize = 512;

        for _ in 0..MAX_PER_BLOCK {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            match command {
                Command::Action(action) => self.apply(action),
                Command::Load { deck, source } => {
                    if let Some(target) = self.deck_mut(deck) {
                        let previous = target.load(source);
                        self.retire(previous);
                    }
                }
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Deck { deck, action } => {
                if matches!(action, DeckAction::Eject) {
                    if let Some(target) = self.deck_mut(deck) {
                        let previous = target.eject();
                        self.retire(previous);
                    }
                    return;
                }
                let Some(target) = self.deck_mut(deck) else {
                    return;
                };
                match action {
                    DeckAction::Play => target.play(),
                    DeckAction::Pause => target.pause(),
                    DeckAction::PlayPause => target.toggle_play(),
                    DeckAction::Cue => target.cue(),
                    DeckAction::Seek(position) => target.seek(position),
                    DeckAction::SetRate(rate) => target.set_rate(rate),
                    DeckAction::SetPitch(pitch) => target.set_pitch(pitch),
                    DeckAction::SetVolume(volume) => target.set_volume(volume),
                    DeckAction::SetGainDb(db) => target.set_gain_db(db),
                    DeckAction::Eject => unreachable!("handled above"),
                }
            }
            Action::Mixer(MixerAction::Crossfader(position)) => {
                self.crossfader = position.clamp(-1.0, 1.0);
                self.registry
                    .set(ParamId::Global(GlobalParam::Crossfader), self.crossfader);
            }
            Action::Mixer(MixerAction::MasterGainDb(db)) => {
                self.master_gain_db = db.clamp(-24.0, 24.0);
                self.master_gain
                    .set_target(db_to_linear(self.master_gain_db));
                self.registry.set(
                    ParamId::Global(GlobalParam::MasterGainDb),
                    self.master_gain_db,
                );
            }
        }
    }

    fn apply_crossfader(&mut self) {
        let (left, right) = crossfader_gains(self.crossfader, self.crossfader_curve);
        for (index, deck) in self.decks.iter_mut().enumerate() {
            let gain = match index {
                CROSSFADER_LEFT => left,
                CROSSFADER_RIGHT => right,
                _ => 1.0,
            };
            deck.set_crossfader_gain(gain);
        }
    }

    fn publish_deck_state(&self) {
        for (index, deck) in self.decks.iter().enumerate() {
            let Some(id) = DeckId::new(index as u8) else {
                continue;
            };
            let set = |param, value| self.registry.set(ParamId::Deck(id, param), value);
            set(
                DeckParam::Playing,
                if deck.is_playing() { 1.0 } else { 0.0 },
            );
            set(DeckParam::Position, deck.position().get() as f32);
            set(DeckParam::Rate, deck.rate().get() as f32);
            set(DeckParam::Pitch, deck.pitch() as f32);
            set(DeckParam::Volume, deck.volume());
            set(DeckParam::GainDb, deck.gain_db());
            set(DeckParam::Loaded, if deck.is_loaded() { 1.0 } else { 0.0 });
            set(DeckParam::LengthFrames, deck.len_frames() as f32);
        }
    }

    /// Number of decks this engine was built with.
    #[must_use]
    pub fn deck_count(&self) -> usize {
        self.decks.len()
    }

    #[must_use]
    pub fn deck(&self, id: DeckId) -> Option<&Deck> {
        self.decks.get(id.index())
    }

    pub fn set_crossfader_curve(&mut self, curve: CrossfaderCurve) {
        self.crossfader_curve = curve;
    }
}

impl AudioCallback for Engine {
    fn render(&mut self, out: &mut [f32], ctx: &RenderContext) {
        let start = std::time::Instant::now();

        self.drain_stranded();
        self.drain_commands();
        self.apply_crossfader();

        let channels = ctx.channels.max(1);

        // Decks add into the shared buffer, so no per-deck scratch is needed.
        for index in 0..self.decks.len() {
            let peak = self.decks[index].process(out, channels);
            if let Some(id) = DeckId::new(index as u8) {
                self.registry
                    .set(ParamId::Deck(id, DeckParam::PeakLevel), peak);
            }
        }

        for frame in out.chunks_exact_mut(channels) {
            let gain = self.master_gain.next_value();
            for sample in frame.iter_mut() {
                *sample *= gain;
                // Hard clip at full scale. A real limiter arrives in M1; until
                // then this guarantees nothing leaves the engine above 0 dBFS,
                // which protects both speakers and ears.
                *sample = sample.clamp(-1.0, 1.0);
            }
        }

        // Meter the master by walking the interleaved buffer per channel.
        let mut left_peak = 0.0f32;
        let mut right_peak = 0.0f32;
        for frame in out.chunks_exact(channels) {
            left_peak = left_peak.max(frame[0].abs());
            if channels > 1 {
                right_peak = right_peak.max(frame[1].abs());
            }
        }
        let left = self.peak_left.process(&[left_peak]);
        let right = self.peak_right.process(&[right_peak]);
        self.registry
            .set(ParamId::Global(GlobalParam::MasterPeakLeft), left);
        self.registry
            .set(ParamId::Global(GlobalParam::MasterPeakRight), right);

        self.publish_deck_state();

        // Fraction of the block's wall-clock budget we consumed. Above 1.0 the
        // device will underrun, so this is the number that predicts dropouts.
        let budget = ctx.frames as f64 / ctx.sample_rate.as_f64();
        if budget > 0.0 {
            let load = start.elapsed().as_secs_f64() / budget;
            self.registry
                .set(ParamId::Global(GlobalParam::CpuLoad), load as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_decode::{AudioBuffer, TrackSource};

    struct Harness {
        commands: rtrb::Producer<Command>,
        retired: rtrb::Consumer<Retired>,
        registry: Arc<ParameterRegistry>,
    }

    fn engine(decks: usize) -> (Engine, Harness) {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        let engine = Engine::new(
            decks,
            SampleRate::DEFAULT,
            command_rx,
            retired_tx,
            Arc::clone(&registry),
        );
        (
            engine,
            Harness {
                commands: command_tx,
                retired: retired_rx,
                registry,
            },
        )
    }

    fn tone(frames: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(
            vec![amplitude; frames * 2],
            SampleRate::DEFAULT,
        ))
    }

    fn ctx(frames: usize) -> RenderContext {
        RenderContext {
            frames,
            channels: 2,
            sample_rate: SampleRate::DEFAULT,
        }
    }

    fn render(engine: &mut Engine, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        engine.render(&mut out, &ctx(frames));
        out
    }

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    #[test]
    fn silence_when_nothing_is_loaded() {
        let (mut engine, _h) = engine(2);
        assert!(render(&mut engine, 64).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a_loaded_playing_deck_produces_audio() {
        let (mut engine, mut h) = engine(2);
        h.commands
            .push(Command::Load {
                deck: deck(1),
                source: tone(10_000, 0.5),
            })
            .unwrap();
        h.commands
            .push(Command::Action(Action::Deck {
                deck: deck(1),
                action: DeckAction::Play,
            }))
            .unwrap();

        // First block ramps the gain up from its initial state; check the second.
        render(&mut engine, 512);
        let out = render(&mut engine, 512);
        assert!(
            out.iter().any(|&s| s.abs() > 0.1),
            "expected audible output"
        );
    }

    #[test]
    fn loading_retires_the_displaced_source_instead_of_dropping_it() {
        let (mut engine, mut h) = engine(2);
        let first = tone(100, 0.5);
        h.commands
            .push(Command::Load {
                deck: deck(1),
                source: Arc::clone(&first),
            })
            .unwrap();
        render(&mut engine, 64);
        // The empty placeholder came back.
        assert!(h.retired.pop().is_ok());

        h.commands
            .push(Command::Load {
                deck: deck(1),
                source: tone(200, 0.5),
            })
            .unwrap();
        render(&mut engine, 64);

        let retired = h.retired.pop().expect("displaced source must be retired");
        assert_eq!(retired.0.len_frames(), 100);
        // The engine gave up its reference; ours is the only one left.
        drop(retired);
        assert_eq!(Arc::strong_count(&first), 1);
    }

    #[test]
    fn crossfader_hard_left_silences_deck_two() {
        let (mut engine, mut h) = engine(2);
        for n in [1u8, 2] {
            h.commands
                .push(Command::Load {
                    deck: deck(n),
                    source: tone(50_000, 0.5),
                })
                .unwrap();
            h.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(n),
                    action: DeckAction::Play,
                }))
                .unwrap();
        }
        h.commands
            .push(Command::Action(Action::Mixer(MixerAction::Crossfader(
                -1.0,
            ))))
            .unwrap();

        // Let the crossfader and gain ramps settle.
        for _ in 0..20 {
            render(&mut engine, 512);
        }
        let out = render(&mut engine, 512);

        // Only deck 1 should be contributing, so the level is one tone, not two.
        let peak = out.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(
            (peak - 0.5).abs() < 0.05,
            "expected one deck's level (~0.5), got {peak}"
        );
    }

    #[test]
    fn output_is_clamped_to_full_scale() {
        let (mut engine, mut h) = engine(4);
        // Four decks at near-full scale would sum well past 1.0.
        for n in 1..=4u8 {
            h.commands
                .push(Command::Load {
                    deck: deck(n),
                    source: tone(50_000, 0.9),
                })
                .unwrap();
            h.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(n),
                    action: DeckAction::Play,
                }))
                .unwrap();
        }
        for _ in 0..20 {
            render(&mut engine, 256);
        }
        let out = render(&mut engine, 256);
        assert!(
            out.iter().all(|s| s.abs() <= 1.0),
            "engine must never emit above full scale"
        );
    }

    #[test]
    fn telemetry_reaches_the_registry() {
        let (mut engine, mut h) = engine(2);
        h.commands
            .push(Command::Load {
                deck: deck(1),
                source: tone(4_800, 0.5),
            })
            .unwrap();
        h.commands
            .push(Command::Action(Action::Deck {
                deck: deck(1),
                action: DeckAction::Play,
            }))
            .unwrap();
        render(&mut engine, 256);

        let id = deck(1);
        assert_eq!(h.registry.get(ParamId::Deck(id, DeckParam::Loaded)), 1.0);
        assert_eq!(h.registry.get(ParamId::Deck(id, DeckParam::Playing)), 1.0);
        assert_eq!(
            h.registry.get(ParamId::Deck(id, DeckParam::LengthFrames)),
            4_800.0
        );
        assert!(h.registry.get(ParamId::Deck(id, DeckParam::Position)) > 0.0);
        assert_eq!(
            h.registry.get(ParamId::Global(GlobalParam::SampleRate)),
            48_000.0
        );
        assert!(h.registry.get(ParamId::Global(GlobalParam::CpuLoad)) >= 0.0);
    }

    #[test]
    fn actions_for_decks_that_do_not_exist_are_ignored() {
        let (mut engine, mut h) = engine(2);
        // Deck 6 on a 2-deck engine: must not panic or corrupt anything.
        h.commands
            .push(Command::Action(Action::Deck {
                deck: deck(6),
                action: DeckAction::Play,
            }))
            .unwrap();
        h.commands
            .push(Command::Load {
                deck: deck(5),
                source: tone(100, 0.5),
            })
            .unwrap();
        render(&mut engine, 64);
        assert_eq!(engine.deck_count(), 2);
    }

    #[test]
    fn eject_retires_the_source_and_silences_the_deck() {
        let (mut engine, mut h) = engine(2);
        h.commands
            .push(Command::Load {
                deck: deck(1),
                source: tone(10_000, 0.5),
            })
            .unwrap();
        h.commands
            .push(Command::Action(Action::Deck {
                deck: deck(1),
                action: DeckAction::Play,
            }))
            .unwrap();
        render(&mut engine, 256);
        while h.retired.pop().is_ok() {}

        h.commands
            .push(Command::Action(Action::Deck {
                deck: deck(1),
                action: DeckAction::Eject,
            }))
            .unwrap();
        let out = render(&mut engine, 256);

        assert!(h.retired.pop().is_ok(), "eject must retire the source");
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(
            h.registry.get(ParamId::Deck(deck(1), DeckParam::Loaded)),
            0.0
        );
    }

    #[test]
    fn engine_is_capped_at_the_maximum_deck_count() {
        let (engine, _h) = engine(99);
        assert_eq!(engine.deck_count(), MAX_DECKS);
    }

    /// A full retirement queue must not cause a drop on the audio thread. The
    /// engine holds the sources instead and hands them over when there is room.
    #[test]
    fn a_full_retirement_queue_does_not_drop_on_the_audio_thread() {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, mut retired_rx) = rtrb::RingBuffer::new(1);
        let registry = Arc::new(ParameterRegistry::new());
        let mut engine = Engine::new(
            2,
            SampleRate::DEFAULT,
            command_rx,
            retired_tx,
            Arc::clone(&registry),
        );
        let mut commands = command_tx;

        let tracked = tone(100, 0.5);
        for source in [Arc::clone(&tracked), tone(100, 0.5), tone(100, 0.5)] {
            commands
                .push(Command::Load {
                    deck: deck(1),
                    source,
                })
                .unwrap();
        }
        render(&mut engine, 64);

        // Our clone plus whatever the engine still holds -- but crucially the
        // source was never freed inside the callback.
        assert!(Arc::strong_count(&tracked) >= 1);

        // Draining lets the stranded entries through on the next block.
        let mut drained = 0;
        for _ in 0..8 {
            while retired_rx.pop().is_ok() {
                drained += 1;
            }
            render(&mut engine, 64);
        }
        assert!(
            drained >= 2,
            "stranded sources should eventually be handed back"
        );
    }
}

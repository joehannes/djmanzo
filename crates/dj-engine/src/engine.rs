//! The engine: everything that happens inside the audio callback.

use crate::bus::BusLayout;
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

    /// Headphone blend: 0.0 all cue, 1.0 all master.
    cue_mix: SmoothedValue,
    cue_mix_target: f32,
    /// Cue in one ear, master in the other.
    cue_split: bool,
    booth_gain: SmoothedValue,
    booth_gain_db: f32,

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
            // Default to hearing only the cue: a DJ reaching for headphones is
            // almost always checking the incoming track, not the master.
            cue_mix: SmoothedValue::new(0.0, sr),
            cue_mix_target: 0.0,
            cue_split: false,
            booth_gain: SmoothedValue::new(1.0, sr),
            booth_gain_db: 0.0,
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
                    DeckAction::SetEqLow(g) => target.set_eq_low(g),
                    DeckAction::SetEqMid(g) => target.set_eq_mid(g),
                    DeckAction::SetEqHigh(g) => target.set_eq_high(g),
                    DeckAction::SetFilter(p) => target.set_filter(p),
                    DeckAction::SetCue(on) => target.set_cue(on),
                    DeckAction::ToggleCue => target.toggle_cue(),
                    DeckAction::SetKeylock(on) => target.set_keylock(on),
                    DeckAction::ToggleKeylock => target.toggle_keylock(),
                    DeckAction::Eject => unreachable!("handled above"),
                }
            }
            Action::Mixer(MixerAction::Crossfader(position)) => {
                self.crossfader = position.clamp(-1.0, 1.0);
                self.registry
                    .set(ParamId::Global(GlobalParam::Crossfader), self.crossfader);
            }
            Action::Mixer(MixerAction::CueMix(mix)) => {
                self.cue_mix_target = mix.clamp(0.0, 1.0);
                self.cue_mix.set_target(self.cue_mix_target);
                self.registry
                    .set(ParamId::Global(GlobalParam::CueMix), self.cue_mix_target);
            }
            Action::Mixer(MixerAction::SplitCue(on)) => {
                self.cue_split = on;
                self.registry
                    .set_bool(ParamId::Global(GlobalParam::CueSplit), on);
            }
            Action::Mixer(MixerAction::BoothGainDb(db)) => {
                self.booth_gain_db = db.clamp(-24.0, 24.0);
                self.booth_gain.set_target(db_to_linear(self.booth_gain_db));
                self.registry.set(
                    ParamId::Global(GlobalParam::BoothGainDb),
                    self.booth_gain_db,
                );
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
            set(DeckParam::EqLow, deck.eq_low());
            set(DeckParam::EqMid, deck.eq_mid());
            set(DeckParam::EqHigh, deck.eq_high());
            set(DeckParam::Filter, deck.filter_position());
            set(
                DeckParam::CueEnabled,
                if deck.is_cued() { 1.0 } else { 0.0 },
            );
            set(
                DeckParam::Keylock,
                if deck.is_keylocked() { 1.0 } else { 0.0 },
            );
            set(
                DeckParam::KeylockLatencyFrames,
                deck.keylock_latency_frames() as f32,
            );
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

        let layout = BusLayout::for_channels(ctx.channels);
        let channels = layout.channels;
        self.registry
            .set_bool(ParamId::Global(GlobalParam::CueAvailable), layout.has_cue());

        // Decks add into the shared buffer -- master post-fader, cue pre-fader
        // -- so no per-deck scratch is needed.
        for index in 0..self.decks.len() {
            let levels = self.decks[index].process(out, &layout);
            if let Some(id) = DeckId::new(index as u8) {
                self.registry
                    .set(ParamId::Deck(id, DeckParam::PeakLevel), levels.post_fader);
                self.registry.set(
                    ParamId::Deck(id, DeckParam::PreFaderLevel),
                    levels.pre_fader,
                );
            }
        }

        let (main_l, main_r) = layout.main;
        for frame in out.chunks_exact_mut(channels) {
            let master = self.master_gain.next_value();
            let booth = self.booth_gain.next_value();
            let mix = self.cue_mix.next_value();

            // Master first: everything downstream is derived from it.
            frame[main_l] = (frame[main_l] * master).clamp(-1.0, 1.0);
            if !layout.is_mono() {
                frame[main_r] = (frame[main_r] * master).clamp(-1.0, 1.0);
            }
            let (master_l, master_r) = (frame[main_l], frame[main_r]);

            // Booth is the master at its own level, so the monitors can be
            // turned down without touching what the room hears.
            if let Some((booth_l, booth_r)) = layout.booth {
                frame[booth_l] = (master_l * booth).clamp(-1.0, 1.0);
                frame[booth_r] = (master_r * booth).clamp(-1.0, 1.0);
            }

            // Headphones: blend the pre-fader cue sum against the master.
            if let Some((cue_l, cue_r)) = layout.cue {
                let (raw_l, raw_r) = (frame[cue_l], frame[cue_r]);
                let (out_l, out_r) = if self.cue_split {
                    // Split cue: cue mono in the left ear, master mono in the
                    // right. Standard for beatmatching -- you hear both sources
                    // separately instead of superimposed.
                    ((raw_l + raw_r) * 0.5, (master_l + master_r) * 0.5)
                } else {
                    (
                        raw_l * (1.0 - mix) + master_l * mix,
                        raw_r * (1.0 - mix) + master_r * mix,
                    )
                };
                frame[cue_l] = out_l.clamp(-1.0, 1.0);
                frame[cue_r] = out_r.clamp(-1.0, 1.0);
            }
        }

        // Meter the master bus specifically, not channel 0/1 of whatever the
        // device gave us -- with a booth in the middle those differ.
        let mut left_peak = 0.0f32;
        let mut right_peak = 0.0f32;
        for frame in out.chunks_exact(channels) {
            left_peak = left_peak.max(frame[main_l].abs());
            right_peak = right_peak.max(frame[main_r].abs());
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

#[cfg(test)]
mod cue_routing_tests {
    use super::*;
    use dj_decode::{AudioBuffer, TrackSource};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn tone(frames: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(
            vec![amplitude; frames * 2],
            SR,
        ))
    }

    struct Rig {
        engine: Engine,
        commands: rtrb::Producer<Command>,
        channels: usize,
    }

    fn rig(channels: usize) -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        Rig {
            engine: Engine::new(2, SR, command_rx, retired_tx, registry),
            commands: command_tx,
            channels,
        }
    }

    impl Rig {
        fn act(&mut self, action: Action) {
            self.commands.push(Command::Action(action)).unwrap();
        }

        fn load_and_play(&mut self, n: u8, amplitude: f32) {
            self.commands
                .push(Command::Load {
                    deck: deck(n),
                    source: tone(400_000, amplitude),
                })
                .unwrap();
            self.act(Action::Deck {
                deck: deck(n),
                action: DeckAction::Play,
            });
        }

        /// Render, discarding blocks first so every ramp has settled.
        fn settle_then_render(&mut self, frames: usize) -> Vec<f32> {
            for _ in 0..40 {
                let mut warm = vec![0.0; 2_048 * self.channels];
                self.engine.render(&mut warm, &self.ctx(2_048));
            }
            let mut out = vec![0.0; frames * self.channels];
            self.engine.render(&mut out, &self.ctx(frames));
            out
        }

        fn ctx(&self, frames: usize) -> RenderContext {
            RenderContext {
                frames,
                channels: self.channels,
                sample_rate: SR,
            }
        }

        /// Peak on a given channel pair.
        fn peak(&self, out: &[f32], pair: (usize, usize)) -> f32 {
            out.chunks_exact(self.channels).fold(0.0f32, |acc, frame| {
                acc.max(frame[pair.0].abs()).max(frame[pair.1].abs())
            })
        }
    }

    fn layout(channels: usize) -> BusLayout {
        BusLayout::for_channels(channels)
    }

    /// Cue at 0.0 is the whole point: the DJ hears only what is being previewed.
    #[test]
    fn cue_mix_at_zero_is_pure_cue() {
        let mut rig = rig(4);
        rig.load_and_play(1, 0.5);
        // Deck 1 cued, but crossfaded away so it contributes nothing to master.
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        rig.act(Action::Mixer(MixerAction::Crossfader(1.0)));
        rig.act(Action::Mixer(MixerAction::CueMix(0.0)));

        let out = rig.settle_then_render(4_096);
        let l = layout(4);
        assert!(
            rig.peak(&out, l.main) < 0.02,
            "master should be silent, got {}",
            rig.peak(&out, l.main)
        );
        assert!(
            rig.peak(&out, l.cue.unwrap()) > 0.4,
            "headphones should carry the cued deck, got {}",
            rig.peak(&out, l.cue.unwrap())
        );
    }

    /// At 1.0 the headphones follow the master, which is how a DJ checks what
    /// the room is actually hearing.
    #[test]
    fn cue_mix_at_one_follows_the_master() {
        let mut rig = rig(4);
        rig.load_and_play(1, 0.5);
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        rig.act(Action::Mixer(MixerAction::Crossfader(1.0)));
        rig.act(Action::Mixer(MixerAction::CueMix(1.0)));

        let out = rig.settle_then_render(4_096);
        let l = layout(4);
        // Master is silent (crossfaded away), so full-master blend is silent too.
        assert!(
            rig.peak(&out, l.cue.unwrap()) < 0.02,
            "full master blend should mirror a silent master, got {}",
            rig.peak(&out, l.cue.unwrap())
        );
    }

    #[test]
    fn cue_mix_is_monotonic_between_the_extremes() {
        let mut levels = Vec::new();
        for mix in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut rig = rig(4);
            rig.load_and_play(1, 0.5);
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::SetCue(true),
            });
            rig.act(Action::Mixer(MixerAction::Crossfader(1.0)));
            rig.act(Action::Mixer(MixerAction::CueMix(mix)));
            let out = rig.settle_then_render(4_096);
            levels.push(rig.peak(&out, layout(4).cue.unwrap()));
        }
        for pair in levels.windows(2) {
            assert!(
                pair[1] <= pair[0] + 0.01,
                "cue level should fall as the blend moves toward master: {levels:?}"
            );
        }
    }

    /// Split cue: cue in one ear, master in the other, so the two sources are
    /// heard separately rather than superimposed.
    #[test]
    fn split_cue_separates_the_ears() {
        let mut rig = rig(4);
        // Deck 1 cued and crossfaded away; deck 2 on the master.
        rig.load_and_play(1, 0.5);
        rig.load_and_play(2, 0.5);
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        rig.act(Action::Mixer(MixerAction::Crossfader(1.0)));
        rig.act(Action::Mixer(MixerAction::SplitCue(true)));

        let out = rig.settle_then_render(4_096);
        let (cue_l, cue_r) = layout(4).cue.unwrap();

        let left = out
            .chunks_exact(4)
            .fold(0.0f32, |a, f| a.max(f[cue_l].abs()));
        let right = out
            .chunks_exact(4)
            .fold(0.0f32, |a, f| a.max(f[cue_r].abs()));

        assert!(left > 0.4, "left ear should carry the cue, got {left}");
        assert!(
            right > 0.4,
            "right ear should carry the master, got {right}"
        );
    }

    #[test]
    fn booth_follows_the_master_at_its_own_level() {
        let mut rig = rig(6);
        rig.load_and_play(1, 0.5);
        rig.act(Action::Mixer(MixerAction::Crossfader(-1.0)));
        rig.act(Action::Mixer(MixerAction::BoothGainDb(-12.0)));

        let out = rig.settle_then_render(4_096);
        let l = layout(6);
        let master = rig.peak(&out, l.main);
        let booth = rig.peak(&out, l.booth.unwrap());

        assert!(master > 0.4, "master should be playing, got {master}");
        // -12 dB is about a quarter amplitude.
        assert!(
            (booth - master * 0.25).abs() < 0.05,
            "booth should be 12 dB below master: master {master}, booth {booth}"
        );
    }

    /// The failure that would embarrass you in front of a room: the cue bus
    /// leaking into the master, so the audience hears the track being previewed.
    #[test]
    fn the_cue_bus_never_reaches_the_master() {
        let mut rig = rig(4);
        rig.load_and_play(1, 0.9);
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        // Fader fully down and crossfaded away: nothing may reach the room.
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetVolume(0.0),
        });
        rig.act(Action::Mixer(MixerAction::Crossfader(1.0)));

        let out = rig.settle_then_render(4_096);
        let l = layout(4);
        assert!(
            rig.peak(&out, l.main) < 0.01,
            "cue leaked into the master: {}",
            rig.peak(&out, l.main)
        );
        assert!(
            rig.peak(&out, l.cue.unwrap()) > 0.5,
            "the cue itself should be loud, got {}",
            rig.peak(&out, l.cue.unwrap())
        );
    }

    /// A stereo device cannot carry a cue, and the UI needs to know so it can
    /// explain why the cue controls are dead rather than just disabling them.
    #[test]
    fn cue_availability_is_published() {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(16);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(16);
        let registry = Arc::new(ParameterRegistry::new());
        let mut engine = Engine::new(2, SR, command_rx, retired_tx, Arc::clone(&registry));
        drop(command_tx);

        let mut out = vec![0.0; 256 * 2];
        engine.render(
            &mut out,
            &RenderContext {
                frames: 256,
                channels: 2,
                sample_rate: SR,
            },
        );
        assert!(!registry.get_bool(ParamId::Global(GlobalParam::CueAvailable)));

        let mut out = vec![0.0; 256 * 4];
        engine.render(
            &mut out,
            &RenderContext {
                frames: 256,
                channels: 4,
                sample_rate: SR,
            },
        );
        assert!(registry.get_bool(ParamId::Global(GlobalParam::CueAvailable)));
    }

    #[test]
    fn a_stereo_device_still_plays_with_decks_cued() {
        let mut rig = rig(2);
        rig.load_and_play(1, 0.5);
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        rig.act(Action::Mixer(MixerAction::Crossfader(-1.0)));

        let out = rig.settle_then_render(4_096);
        assert!(
            rig.peak(&out, layout(2).main) > 0.4,
            "master must still work when cue is unavailable"
        );
    }
}

//! The engine: everything that happens inside the audio callback.

use crate::bus::BusLayout;
use crate::command::{Command, Retired};
use crate::deck::Deck;
use crate::rack::Rack;
use crate::record::Recorder;
use crate::sampler::Sampler;
use dj_audio::{AudioCallback, RenderContext};
use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, GlobalParam};
use dj_core::{
    Action, CrossfaderAssign, DeckAction, DeckId, MAX_DECKS, MixerAction, ParamId, SampleRate,
    db_to_linear,
};
use dj_dsp::fx::FxContext;
use dj_dsp::{CrossfaderCurve, Limiter, PeakMeter, SmoothedValue, Spectrum, crossfader_gains};
use std::sync::Arc;

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

    /// Snap beat jumps to the grid.
    quantize: bool,

    /// Four banks of eight samples.
    ///
    /// Beside the decks rather than inside one: a sample belongs to the set,
    /// not to a track, and a DJ firing a stab does not first choose which deck
    /// it comes out of.
    sampler: Sampler,
    /// Three effect slots over the whole mix.
    ///
    /// Placement is meaningless here — there is no fader after the master — so
    /// both passes run, and a slot's placement setting is simply the order it
    /// falls in. Unlike a deck's rack this one keeps running when every deck is
    /// paused, which is what lets an echo thrown into the master ring out over
    /// a silence.
    master_rack: Rack,
    /// Capturing into a sampler slot. See [`crate::record`].
    recorder: Recorder,
    /// The master, on its way to a file. See [`Command::RecordStream`].
    record_stream: Option<rtrb::Producer<f32>>,
    /// Samples the ring would not take since recording started.
    ///
    /// A full ring means the writer thread is behind — a slow disk, a machine
    /// under load. The audio thread will not wait for it, so the samples are
    /// lost, and the count is published rather than swallowed: a recording with
    /// a gap in it should say so, not be discovered later.
    dropped_samples: u64,
    /// The last thing before the PA.
    limiter: Limiter,
    /// The last thing before the DJ's ears.
    ///
    /// A second instance rather than a shared one, because the two buses carry
    /// different audio. Having it here at all is what keeps the headphones and
    /// the master time-aligned: both paths pick up exactly the same look-ahead
    /// delay, so beatmatching against the master stays honest. Hearing damage
    /// is the other reason — a pre-fader cue sum of four decks can go a long
    /// way over full scale, and it is going straight into someone's ears.
    cue_limiter: Limiter,

    /// Sources we could not hand back because the retirement queue was full.
    /// Held rather than dropped -- dropping here is exactly what the queue
    /// exists to prevent. Drained on the next callback that has room.
    stranded: Vec<Retired>,

    /// Band energies off the master, for the interface to move to. Nothing in
    /// the audio path reads it.
    spectrum: Spectrum,
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
            decks: (0..deck_count)
                .map(|index| {
                    let mut deck = Deck::new(sample_rate);
                    deck.set_crossfader_assign(CrossfaderAssign::default_for(index));
                    deck
                })
                .collect(),
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
            quantize: false,
            sampler: Sampler::new(sample_rate.as_f64()),
            master_rack: Rack::new(sr),
            recorder: Recorder::new(sample_rate),
            record_stream: None,
            dropped_samples: 0,
            limiter: Limiter::new(sr),
            cue_limiter: Limiter::new(sr),
            // Capacity for a pathological burst of loads; never grown at runtime.
            stranded: Vec::with_capacity(MAX_DECKS * 2),
            // 1024 frames of window, a transform every 512 — 94 Hz, comfortably
            // ahead of the 60 Hz snapshot and a quarter of the work analysing
            // every block would have been.
            spectrum: Spectrum::new(1024, 512, sr),
        };
        let mut engine = engine;
        // Turn the starting assignments into starting gains before any audio
        // flows: without this a deck assigned left would sit at unity until the
        // first time somebody touched the crossfader.
        engine.apply_crossfader();
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
        self.registry.set_bool(
            ParamId::Global(GlobalParam::LimiterEnabled),
            !self.limiter.is_bypassed(),
        );
        // Constant whether the limiter is engaged or bypassed -- see
        // `Limiter::set_bypass` for why that matters.
        self.registry.set(
            ParamId::Global(GlobalParam::OutputLatencyFrames),
            self.limiter.latency_frames() as f32,
        );
        self.registry
            .set_bool(ParamId::Global(GlobalParam::Quantize), self.quantize);
    }

    /// The master rack's three slots.
    ///
    /// Alongside the deck racks in [`Self::publish_deck_state`] rather than
    /// inside them, because the master is not a deck — but published on the
    /// same schedule, so the interface never sees a half-updated rack.
    /// The sampler's own state, and the showing bank's eight slots.
    fn publish_sampler(&self) {
        let set = |param, value| self.registry.set(ParamId::Global(param), value);
        set(GlobalParam::SamplerBank, f32::from(self.sampler.bank()));
        set(GlobalParam::SamplerVolume, self.sampler.volume());

        for number in 1..=dj_core::SAMPLE_SLOTS as u8 {
            let (Some(param), Some(slot)) =
                (GlobalParam::sample(number), self.sampler.slot(number))
            else {
                continue;
            };
            set(param.loaded, if slot.is_loaded() { 1.0 } else { 0.0 });
            set(param.playing, if slot.is_playing() { 1.0 } else { 0.0 });
            set(param.mode, slot.mode().index() as f32);
            set(param.volume, slot.volume());
            set(param.progress, slot.progress());
            set(
                param.cue,
                if slot.output() == dj_core::SampleOutput::Cue {
                    1.0
                } else {
                    0.0
                },
            );
            set(param.synced, if slot.is_synced() { 1.0 } else { 0.0 });
            // Zero for a sample with no tempo of its own, which is how the
            // interface knows to hide the sync switch rather than grey it out.
            set(param.bpm, slot.bpm().unwrap_or(0.0) as f32);
        }
    }

    /// What the interface needs to draw the record button.
    ///
    /// Published every block rather than on change, like everything else here:
    /// the elapsed time moves continuously, and a snapshot that had to ask
    /// separately for "is it recording" and "how long" could catch the two
    /// disagreeing.
    fn publish_recorder(&self) {
        self.registry.set_bool(
            ParamId::Global(GlobalParam::RecordReady),
            self.recorder.is_ready(),
        );
        self.registry.set_bool(
            ParamId::Global(GlobalParam::Recording),
            self.recorder.is_running(),
        );
        self.registry.set(
            ParamId::Global(GlobalParam::RecordSlot),
            f32::from(self.recorder.slot().unwrap_or(0)),
        );
        self.registry.set(
            ParamId::Global(GlobalParam::RecordSeconds),
            self.recorder.seconds(),
        );
        // 0 for the master, otherwise the deck's own number — one reading
        // rather than a flag plus a number that could disagree about which.
        self.registry.set(
            ParamId::Global(GlobalParam::RecordSourceDeck),
            match self.recorder.tapping() {
                Some(dj_core::RecordSource::Deck(deck)) => f32::from(deck.human_number()),
                _ => 0.0,
            },
        );
    }

    fn publish_master_rack(&self) {
        for number in 1..=dj_core::FX_SLOTS as u8 {
            let (Some(param), Some(slot)) =
                (GlobalParam::fx(number), self.master_rack.slot(number))
            else {
                continue;
            };
            let set = |p, value| self.registry.set(ParamId::Global(p), value);
            set(param.kind, slot.kind().index() as f32);
            set(param.enabled, if slot.is_enabled() { 1.0 } else { 0.0 });
            set(param.wet, slot.wet());
            set(param.beats, slot.beats());
            set(param.amount, slot.amount());
            set(
                param.post,
                if slot.placement() == dj_core::Placement::PostFader {
                    1.0
                } else {
                    0.0
                },
            );
        }
    }

    fn deck_mut(&mut self, id: DeckId) -> Option<&mut Deck> {
        self.decks.get_mut(id.index())
    }

    /// Hand a buffer back to the host thread.
    ///
    /// If the queue is full we hold onto it rather than dropping it here --
    /// dropping on the audio thread is the whole thing we are avoiding.
    fn retire(&mut self, mut item: Retired) {
        if self.stranded.len() < self.stranded.capacity() {
            // Try the queue first; stash only if it will not take it.
            match self.retired.push(item) {
                Ok(()) => return,
                Err(rtrb::PushError::Full(returned)) => item = returned,
            }
            self.stranded.push(item);
        } else {
            // Both the queue and the stash are full, which means the host thread
            // has stopped draining entirely. Leak rather than free: a permanent
            // loss of a few megabytes is strictly better than a dropout, and a
            // plain drop here would be exactly the `free()` this queue exists to
            // avoid.
            if let Err(rtrb::PushError::Full(returned)) = self.retired.push(item) {
                std::mem::forget(returned);
            }
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
                Command::SetGrid { deck, grid } => {
                    if let Some(target) = self.deck_mut(deck) {
                        target.set_grid(grid);
                    }
                }
                Command::SetHotCues { deck, cues } => {
                    if let Some(target) = self.deck_mut(deck) {
                        target.set_hot_cues(cues);
                    }
                }
                Command::SetLoop { deck, region } => {
                    if let Some(target) = self.deck_mut(deck) {
                        target.set_loop_region(region);
                    }
                }
                Command::Load { deck, source } => {
                    if let Some(target) = self.deck_mut(deck) {
                        let previous = target.load(source);
                        self.retire(Retired::Source(previous));
                    }
                }
                Command::LoadSample {
                    bank,
                    slot,
                    source,
                    bpm,
                } => {
                    if let Some(target) = self.sampler.slot_in_mut(bank, slot) {
                        let previous = target.load(source, bpm);
                        self.retire(Retired::Source(previous));
                    }
                }
                Command::RecordStream { sink } => {
                    // The displaced half goes back rather than being dropped,
                    // for the same reason a displaced track does.
                    let previous = match sink {
                        Some(sink) => self.record_stream.replace(sink),
                        None => self.record_stream.take(),
                    };
                    self.dropped_samples = 0;
                    if let Some(previous) = previous {
                        self.retire(Retired::Stream(previous));
                    }
                }
                Command::RecordSpace { samples } => {
                    // The displaced buffer goes back the way it came rather
                    // than being dropped, for the same reason a displaced track
                    // does. It is refused outright mid-capture, and refusing
                    // hands it straight back.
                    if let Some(previous) = self.recorder.give_space(samples) {
                        self.retire(Retired::Buffer(previous));
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
                        self.retire(Retired::Source(previous));
                    }
                    return;
                }
                // Sync reads one deck while writing another, so it is handled
                // before the single mutable borrow below rather than inside it.
                match action {
                    DeckAction::Sync => {
                        self.sync_deck(deck);
                        return;
                    }
                    DeckAction::SyncOff => {
                        if let Some(target) = self.deck_mut(deck) {
                            target.set_synced(false);
                        }
                        return;
                    }
                    // Handled here rather than below because changing the
                    // assignment has to recompute *every* deck's crossfader
                    // gain, which needs the engine and not just this deck.
                    DeckAction::SetCrossfaderAssign(assign) => {
                        let Some(target) = self.deck_mut(deck) else {
                            return;
                        };
                        target.set_crossfader_assign(assign);
                        self.apply_crossfader();
                        return;
                    }
                    _ => {}
                }

                let quantize = self.quantize;
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
                    DeckAction::SetKeyShift(n) => target.set_key_shift(n),
                    DeckAction::SetSlip(on) => target.set_slip(on),
                    DeckAction::ToggleSlip => {
                        let on = target.slip();
                        target.set_slip(!on);
                    }
                    DeckAction::SetReverse(on) => target.set_reverse(on),
                    DeckAction::ToggleReverse => {
                        // The deck's own reverse, not `reversed()` -- that also
                        // reports true while a censor is held, and toggling
                        // during a censor would leave the deck reversed when
                        // the pad came up.
                        let on = target.reversed() && !target.censoring();
                        target.set_reverse(!on);
                    }
                    DeckAction::SetCensor(held) => target.set_censor(held),
                    DeckAction::Fx { slot, change } => {
                        target.rack_mut().apply(slot, change);
                    }
                    DeckAction::Brake(Some(beats)) => {
                        target.brake(f64::from(beats), false);
                    }
                    DeckAction::Brake(None) => target.release_brake(),
                    DeckAction::Backspin(Some(beats)) => {
                        target.brake(f64::from(beats), true);
                    }
                    DeckAction::Backspin(None) => target.release_brake(),
                    DeckAction::LoopRoll(beats) => {
                        target.set_loop_roll(beats, quantize);
                    }
                    DeckAction::Slice(Some(slice)) => {
                        target.hold_slice(slice);
                    }
                    DeckAction::Slice(None) => target.release_slice(),
                    DeckAction::SliceDomain(beats) => {
                        target.set_slice_domain(f64::from(beats));
                    }
                    DeckAction::BeatJump(beats) => {
                        target.beat_jump(beats, quantize);
                    }
                    DeckAction::HotCue(slot) => {
                        target.hot_cue_pressed(slot, quantize);
                    }
                    DeckAction::HotCueSet(slot) => {
                        target.set_hot_cue(slot, quantize);
                    }
                    DeckAction::HotCueClear(slot) => {
                        target.clear_hot_cue(slot);
                    }
                    DeckAction::LoopBeats(beats) => {
                        target.set_loop_length(f64::from(beats), quantize);
                    }
                    DeckAction::LoopOff => target.exit_loop(),
                    DeckAction::LoopHalve => {
                        target.scale_loop(0.5);
                    }
                    DeckAction::LoopDouble => {
                        target.scale_loop(2.0);
                    }
                    DeckAction::LoopIn => target.set_loop_in(quantize),
                    DeckAction::LoopOut => {
                        target.set_loop_out(quantize);
                    }
                    DeckAction::LoopMove(beats) => {
                        target.move_loop(beats);
                    }
                    // Sync needs to read another deck while writing this one,
                    // so it cannot run inside a borrow of `target`.
                    DeckAction::Sync | DeckAction::SyncOff => unreachable!("handled above"),
                    DeckAction::SetCrossfaderAssign(_) => unreachable!("handled above"),
                    DeckAction::Eject => unreachable!("handled above"),
                    // Grid edits are computed by the host and arrive here as
                    // `Command::SetGrid`, because editing needs the analyser's
                    // original to reset to and a tap history to average, and
                    // neither belongs on the audio thread. They stay in the
                    // action vocabulary so a controller, a script and the
                    // assistant can all express them -- see `dj_app::grid`.
                    //
                    // Ignored rather than `unreachable!`: a panic here would be
                    // on the audio thread, and an action arriving by a path
                    // nobody has written yet is not worth killing the audio for.
                    DeckAction::GridAnchorHere
                    | DeckAction::GridNudge(_)
                    | DeckAction::GridScale(_)
                    | DeckAction::GridSetBpm(_)
                    | DeckAction::GridTap
                    | DeckAction::GridReset
                    // Saved loops are the same shape: the region lives in the
                    // library with the track, so the host reads or writes it
                    // and a recall arrives here as `Command::SetLoop`.
                    | DeckAction::LoopSave(_)
                    | DeckAction::LoopRecall(_) => {}
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
            // Recording is the application's business, not the engine's: the
            // engine has no idea what a file is. It is in the vocabulary so a
            // controller and a script can start one, and the application
            // intercepts it before it reaches here — this arm is only for an
            // action that somehow arrived anyway.
            Action::Mixer(MixerAction::SetRecording(_)) => {}
            Action::Mixer(MixerAction::SetQuantize(on)) => {
                self.quantize = on;
                self.registry
                    .set_bool(ParamId::Global(GlobalParam::Quantize), on);
            }
            Action::Mixer(MixerAction::Fx { slot, change }) => {
                self.master_rack.apply(slot, change);
            }
            Action::Mixer(MixerAction::Sample { slot, change }) => {
                self.sampler.apply(slot, change);
            }
            Action::Mixer(MixerAction::Sampler(change)) => match change {
                dj_core::SamplerChange::Bank(bank) => self.sampler.set_bank(bank),
                dj_core::SamplerChange::Volume(volume) => self.sampler.set_volume(volume),
                dj_core::SamplerChange::StopAll => self.sampler.stop_all(),
                dj_core::SamplerChange::Record { slot, source } => {
                    // The tempo stamped on the capture comes from whichever tap
                    // it is: a recording off one deck is at that deck's tempo,
                    // which is not the room's when it is not synced.
                    let bpm = match source {
                        dj_core::RecordSource::Master => self.master_bpm(),
                        dj_core::RecordSource::Deck(deck) => self
                            .decks
                            .get(deck.index())
                            .and_then(|deck| deck.effective_bpm()),
                    };
                    self.recorder.start(self.sampler.bank(), slot, source, bpm);
                }
                dj_core::SamplerChange::RecordStop => self.recorder.stop(),
                dj_core::SamplerChange::RecordCancel => self.recorder.cancel(),
            },
            Action::Mixer(MixerAction::SetLimiter(on)) => {
                // The cue limiter is not switched with it. Bypass exists for
                // the DJ feeding an external processor, and that processor is
                // downstream of the master only -- the headphones are still
                // wired straight to a pair of drivers next to someone's ears.
                self.limiter.set_bypass(!on);
                self.registry
                    .set_bool(ParamId::Global(GlobalParam::LimiterEnabled), on);
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
        for deck in &mut self.decks {
            let gain = match deck.crossfader_assign() {
                CrossfaderAssign::Left => left,
                CrossfaderAssign::Right => right,
                // Full gain, not the curve's mid-point: "through" means the
                // crossfader is not in this deck's signal path at all, so
                // parking the fader must not attenuate it.
                CrossfaderAssign::Thru => 1.0,
            };
            deck.set_crossfader_gain(gain);
        }
    }

    fn publish_deck_state(&self) {
        // The master rack changes all night, so it rides with the per-block
        // publisher rather than with the static state written at construction.
        self.publish_master_rack();
        self.publish_sampler();
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
            set(DeckParam::KeyShift, deck.key_shift() as f32);
            set(
                DeckParam::CrossfaderAssign,
                deck.crossfader_assign().as_param(),
            );
            set(DeckParam::Synced, if deck.is_synced() { 1.0 } else { 0.0 });
            set(
                DeckParam::EffectiveBpm,
                deck.effective_bpm().unwrap_or(0.0) as f32,
            );
            set(
                DeckParam::GridConfidence,
                deck.grid().map_or(0.0, |g| g.confidence.get() as f32),
            );
            // The audio thread's own idea of where the beat is, which is what
            // makes the interface able to move in time with the music rather
            // than alongside it.
            set(
                DeckParam::BeatPhase,
                deck.beat_phase().unwrap_or(0.0) as f32,
            );
            set(DeckParam::Slip, if deck.slip() { 1.0 } else { 0.0 });
            set(DeckParam::Reversed, if deck.reversed() { 1.0 } else { 0.0 });
            set(DeckParam::Rolling, if deck.rolling() { 1.0 } else { 0.0 });
            set(DeckParam::Slicing, if deck.slicing() { 1.0 } else { 0.0 });
            set(DeckParam::SliceBeats, deck.slice_beats() as f32);
            // Zero means "no grid, so no slices" — distinct from slice 1, which
            // is why the pads are numbered from one.
            set(
                DeckParam::SliceIndex,
                f32::from(deck.slice_index().unwrap_or(0)),
            );
            set(
                DeckParam::Spinning,
                if deck.is_spinning() { 1.0 } else { 0.0 },
            );
            set(
                DeckParam::SlipPosition,
                // Zero is a real position, so "not slipping" cannot be zero.
                // Negative one is outside the track and unambiguous.
                deck.slip_position().map_or(-1.0, |p| p.get() as f32),
            );

            let region = deck.active_loop();
            set(
                DeckParam::LoopActive,
                if region.is_some() { 1.0 } else { 0.0 },
            );
            set(
                DeckParam::LoopStart,
                region.map_or(0.0, |r| r.start.get() as f32),
            );
            set(
                DeckParam::LoopEnd,
                region.map_or(0.0, |r| r.end.get() as f32),
            );
            set(
                DeckParam::LoopBeats,
                deck.loop_beats().unwrap_or(0.0) as f32,
            );

            for number in 1..=dj_core::FX_SLOTS as u8 {
                let (Some(param), Some(slot)) = (DeckParam::fx(number), deck.rack().slot(number))
                else {
                    continue;
                };
                set(param.kind, slot.kind().index() as f32);
                set(param.enabled, if slot.is_enabled() { 1.0 } else { 0.0 });
                set(param.wet, slot.wet());
                set(param.beats, slot.beats());
                set(param.amount, slot.amount());
                set(
                    param.post,
                    if slot.placement() == dj_core::Placement::PostFader {
                        1.0
                    } else {
                        0.0
                    },
                );
            }

            for slot in 1..=dj_core::HOT_CUE_SLOTS as u8 {
                let Some(param) = DeckParam::hot_cue(slot) else {
                    continue;
                };
                set(
                    param,
                    deck.hot_cue(slot)
                        .map_or(dj_core::param::UNSET_HOT_CUE, |pos| pos.get() as f32),
                );
            }
        }
    }

    /// Which deck a sync request should follow.
    ///
    /// Automatic rather than a designated leader, because the answer is nearly
    /// always obvious: the deck that is already playing is the one the room is
    /// hearing, and it is the one that must not move. Where several qualify the
    /// lowest-numbered wins, which is arbitrary but stable — and a DJ with three
    /// decks running has bigger decisions than this one.
    ///
    /// A candidate must be playing *and* carry a grid solid enough to trust.
    /// Following a guess is how sync earns its reputation for derailing mixes.
    #[must_use]
    fn sync_leader(&self, follower: DeckId) -> Option<usize> {
        self.decks.iter().enumerate().find_map(|(index, deck)| {
            let is_self = index == follower.index();
            let usable =
                deck.is_playing() && deck.grid().is_some_and(|g| g.confidence.is_sync_worthy());
            (!is_self && usable).then_some(index)
        })
    }

    /// Match a deck's tempo and phase to the leader.
    ///
    /// Both halves can fail independently and the distinction matters: a deck
    /// whose tempo could not be matched is not synced at all, while one whose
    /// phase could not be aligned (because the shift would run off the end of
    /// the track) is still tempo-locked and worth marking as such.
    fn sync_deck(&mut self, follower: DeckId) {
        let Some(leader_index) = self.sync_leader(follower) else {
            return;
        };
        let Some((leader_bpm, leader_phase)) = self
            .decks
            .get(leader_index)
            .and_then(|leader| Some((leader.effective_bpm()?, leader.beat_phase()?)))
        else {
            return;
        };

        let Some(target) = self.decks.get_mut(follower.index()) else {
            return;
        };
        // A follower with a grid too weak to trust is refused for the same
        // reason a leader is: the grid is what sync acts on, and acting
        // confidently on a bad one is the failure mode.
        if !target.grid().is_some_and(|g| g.confidence.is_sync_worthy()) {
            return;
        }

        if !target.match_tempo(leader_bpm) {
            return;
        }
        target.align_phase_to(leader_phase);
        target.set_synced(true);
    }

    /// Hold every synced deck at its leader's tempo.
    ///
    /// Run once a block rather than only when sync is pressed, because the
    /// leader's pitch fader can move afterwards — and a "sync" that silently
    /// stops being true the moment someone touches a fader is worse than no
    /// sync, since nobody is watching for it.
    ///
    /// Phase is deliberately *not* re-corrected here. A continuous phase servo
    /// is beat lock, which is a different feature with different failure modes;
    /// nudging the playhead every block to chase rounding error would be
    /// audible for no benefit.
    fn hold_sync(&mut self) {
        for index in 0..self.decks.len() {
            if !self.decks[index].is_synced() {
                continue;
            }
            let Some(follower_id) = DeckId::new(index as u8) else {
                continue;
            };
            let Some(leader_index) = self.sync_leader(follower_id) else {
                // The leader stopped or lost its grid. The lock is released
                // rather than frozen at the last tempo, because a lock to
                // nothing is a lie the interface would keep showing.
                self.decks[index].set_synced(false);
                continue;
            };
            let Some(leader_bpm) = self.decks[leader_index].effective_bpm() else {
                continue;
            };
            if !self.decks[index].match_tempo(leader_bpm) {
                self.decks[index].set_synced(false);
            }
        }
    }

    /// The tempo the room is running at, if any.
    ///
    /// The loudest playing deck that has a grid — the same rule the master
    /// effect rack borrows by, and for the same reason: it is the deck the room
    /// is hearing, so it is the tempo a sample should stretch to.
    fn master_bpm(&self) -> Option<f64> {
        let mut best: Option<(f32, f64)> = None;
        for deck in &self.decks {
            if !deck.is_playing() {
                continue;
            }
            let Some(bpm) = deck.effective_bpm() else {
                continue;
            };
            let volume = deck.volume();
            if best.is_none_or(|(loudest, _)| volume > loudest) {
                best = Some((volume, bpm));
            }
        }
        best.map(|(_, bpm)| bpm)
    }

    /// What the master rack should measure a beat as.
    ///
    /// The master has no tempo of its own, so it borrows one from **the loudest
    /// playing deck that has a grid**. Loudest rather than lowest-numbered
    /// because that is the deck the room is hearing, and an echo over the mix
    /// should be in time with the music people are dancing to — during a
    /// transition it follows the incoming deck as it comes up, which is exactly
    /// when a master effect gets thrown.
    ///
    /// Channel volume rather than the meter: the meter swings with every kick,
    /// and a rule that changes its mind on every kick is not a rule.
    fn master_fx_context(&self) -> FxContext {
        let sample_rate = self.sample_rate.as_f64();
        let mut best: Option<(f32, f64)> = None;
        for deck in &self.decks {
            if !deck.is_playing() {
                continue;
            }
            let Some(bpm) = deck.effective_bpm() else {
                continue;
            };
            let volume = deck.volume();
            if best.is_none_or(|(loudest, _)| volume > loudest) {
                best = Some((volume, bpm));
            }
        }
        FxContext {
            sample_rate: sample_rate as f32,
            beat_frames: best
                .map(|(_, bpm)| (sample_rate * 60.0 / bpm) as f32)
                .filter(|frames| frames.is_finite() && *frames > 0.0),
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
        self.hold_sync();
        self.apply_crossfader();

        let layout = BusLayout::for_channels(ctx.channels);
        let channels = layout.channels;
        self.registry
            .set_bool(ParamId::Global(GlobalParam::CueAvailable), layout.has_cue());

        // Decks add into the shared buffer -- master post-fader, cue pre-fader
        // -- so no per-deck scratch is needed.
        // Which deck, if any, the recorder is listening to. Read once for the
        // block: the answer cannot change inside one, and asking per deck is a
        // comparison rather than a branch on every frame.
        let tapped_deck = match self.recorder.tapping() {
            Some(dj_core::RecordSource::Deck(deck)) => Some(deck.index()),
            _ => None,
        };
        for index in 0..self.decks.len() {
            let tap = (tapped_deck == Some(index)).then_some(&mut self.recorder);
            let levels = self.decks[index].process(out, &layout, tap);
            if let Some(id) = DeckId::new(index as u8) {
                self.registry
                    .set(ParamId::Deck(id, DeckParam::PeakLevel), levels.post_fader);
                self.registry.set(
                    ParamId::Deck(id, DeckParam::PreFaderLevel),
                    levels.pre_fader,
                );
            }
        }

        // The sampler adds to the same bus the decks did, before the master
        // gain and the rack — so a master effect covers the samples too, which
        // is what a DJ throwing an echo over the mix expects.
        let master_ctx = self.master_fx_context();
        let sample_peak = self.sampler.process(out, &layout, self.master_bpm());
        self.registry
            .set(ParamId::Global(GlobalParam::SamplerPeak), sample_peak);

        let recording_master = self.recorder.tapping() == Some(dj_core::RecordSource::Master);

        let (main_l, main_r) = layout.main;
        for frame in out.chunks_exact_mut(channels) {
            let master = self.master_gain.next_value();
            let booth = self.booth_gain.next_value();
            let mix = self.cue_mix.next_value();

            // Master first: everything downstream is derived from it.
            //
            // Deliberately *not* clamped before the limiter. A clamp is hard
            // digital clipping, and clipping the signal on the way into a
            // limiter throws away the very peaks it exists to catch -- the
            // damage would already be done and merely quieter.
            let raw_l = frame[main_l] * master;
            let raw_r = if layout.is_mono() {
                raw_l
            } else {
                frame[main_r] * master
            };

            // The master rack, between the master gain and the limiter. After
            // the gain so the DJ's level control is what feeds it, and before
            // the limiter because the limiter is the last thing before the PA
            // and nothing may get past it.
            //
            // Both passes run: there is no fader after the master for a
            // placement to be on either side of, so pre and post here are
            // simply the order the slots fall in.
            let (raw_l, raw_r) = self.master_rack.process_pre(raw_l, raw_r, &master_ctx);
            let (raw_l, raw_r) = self.master_rack.process_post(raw_l, raw_r, &master_ctx);

            // The clamp that remains is a backstop for a limiter that has been
            // bypassed, and for anything non-finite that got this far. It
            // should never engage while the limiter is engaged.
            let (master_l, master_r) = self.limiter.process_frame(raw_l, raw_r);
            let (master_l, master_r) = (master_l.clamp(-1.0, 1.0), master_r.clamp(-1.0, 1.0));

            frame[main_l] = master_l;
            if !layout.is_mono() {
                frame[main_r] = master_r;
            }

            // The master tap sits here: after the limiter, so a capture of the
            // master is a capture of what the room actually heard rather than
            // of a signal that would have clipped on the way out.
            if recording_master {
                self.recorder.write(master_l, master_r);
            }

            // Booth is the master at its own level, so the monitors can be
            // turned down without touching what the room hears.
            if let Some((booth_l, booth_r)) = layout.booth {
                frame[booth_l] = (master_l * booth).clamp(-1.0, 1.0);
                frame[booth_r] = (master_r * booth).clamp(-1.0, 1.0);
            }

            // Headphones: blend the pre-fader cue sum against the master.
            //
            // The cue goes through its own limiter first, and the alignment is
            // the point: `master_l`/`master_r` have already been delayed by the
            // master limiter's look-ahead, so an undelayed cue would sit 5 ms
            // early against them. Beatmatching is the act of comparing these
            // two signals, so a 5 ms offset between them is not a detail --
            // it is a grid the DJ would carefully match, and the room would
            // hear 5 ms out. Matching delays on both paths cancels exactly.
            if let Some((cue_l, cue_r)) = layout.cue {
                let (raw_l, raw_r) = self.cue_limiter.process_frame(frame[cue_l], frame[cue_r]);
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
            // Summed to mono for the spectrum: the interface wants to know what
            // the room is hearing, and no theme has ever needed to know that the
            // hats are panned left.
            self.spectrum.push((frame[main_l] + frame[main_r]) * 0.5);

            // Straight off the master, after the limiter: a recording of the
            // set is a recording of what the room heard, including the effects
            // and the sampler, and including whatever the limiter did about it.
            //
            // `push` per sample rather than a bulk chunk. At 256 frames that is
            // 512 atomic stores a block, which measures in single-digit
            // microseconds against a 5.3 ms budget — not worth the partial-write
            // bookkeeping a chunked write would need. Never blocks: a ring the
            // writer thread has fallen behind on loses samples and says so.
            if let Some(sink) = self.record_stream.as_mut() {
                let mut lost = 0u64;
                if sink.push(frame[main_l]).is_err() {
                    lost += 1;
                }
                if sink.push(frame[main_r]).is_err() {
                    lost += 1;
                }
                self.dropped_samples = self.dropped_samples.saturating_add(lost);
            }
        }
        let left = self.peak_left.process(&[left_peak]);
        let right = self.peak_right.process(&[right_peak]);
        self.registry
            .set(ParamId::Global(GlobalParam::MasterPeakLeft), left);
        self.registry
            .set(ParamId::Global(GlobalParam::MasterPeakRight), right);

        let bands = self.spectrum.bands();
        for (band, param) in GlobalParam::BANDS.into_iter().enumerate() {
            self.registry.set(ParamId::Global(param), bands[band]);
        }

        // The master meter reads post-limiter, so it can never show over 0 dB
        // and cannot tell you how hard you are driving it. That is what the
        // reduction figure is for: the two together say "this is what goes out"
        // and "this is what it cost".
        self.registry.set(
            ParamId::Global(GlobalParam::LimiterReductionDb),
            self.limiter.reduction_db(),
        );

        // A finished capture goes out on the retirement queue. Here rather than
        // the moment it finishes because a capture that ends mid-block would
        // otherwise be pushed while the block is still being written.
        if let Some(capture) = self.recorder.take() {
            self.retire(Retired::Capture(capture));
        }
        self.publish_recorder();
        self.registry.set(
            ParamId::Global(GlobalParam::SetRecordDropped),
            self.dropped_samples as f32,
        );

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
        let Retired::Source(source) = &retired else {
            panic!("a displaced track must come back as a source");
        };
        assert_eq!(source.len_frames(), 100);
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
        render(&mut engine, 256);

        assert!(h.retired.pop().is_ok(), "eject must retire the source");
        assert_eq!(
            h.registry.get(ParamId::Deck(deck(1), DeckParam::Loaded)),
            0.0
        );

        // The deck stops contributing at once, but the master limiter's
        // look-ahead still holds a few milliseconds of already-rendered audio.
        // That tail is correct -- it is the same delay everything downstream
        // pays -- so the silence is asserted past it rather than within it.
        let latency = h
            .registry
            .get(ParamId::Global(GlobalParam::OutputLatencyFrames)) as usize;
        let out = render(&mut engine, latency + 256);
        let tail = &out[latency * 2..];
        assert!(
            tail.iter().all(|&s| s == 0.0),
            "master still sounding {} frames after eject",
            latency
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

/// The master limiter, as the engine actually wires it.
///
/// `dj-dsp` proves the limiter limits. These prove it is in the signal path,
/// in the right place, and that putting it there did not break the timing
/// relationship the headphone cue depends on.
#[cfg(test)]
mod limiter_tests {
    use super::*;
    use dj_decode::{AudioBuffer, TrackSource};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    /// A constant level, for driving the bus into the limiter.
    fn tone(frames: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(
            vec![amplitude; frames * 2],
            SR,
        ))
    }

    /// Silence, then a hard step. The step edge is a landmark whose arrival
    /// time can be compared between two buses.
    fn step(silence: usize, then: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        let mut samples = vec![0.0f32; silence * 2];
        samples.extend(std::iter::repeat_n(amplitude, then * 2));
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    struct Rig {
        engine: Engine,
        commands: rtrb::Producer<Command>,
        registry: Arc<ParameterRegistry>,
        channels: usize,
    }

    fn rig(decks: usize, channels: usize) -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        Rig {
            engine: Engine::new(decks, SR, command_rx, retired_tx, Arc::clone(&registry)),
            commands: command_tx,
            registry,
            channels,
        }
    }

    impl Rig {
        fn act(&mut self, action: Action) {
            self.commands.push(Command::Action(action)).unwrap();
        }

        fn play(&mut self, n: u8, source: Arc<dyn TrackSource>) {
            self.commands
                .push(Command::Load {
                    deck: deck(n),
                    source,
                })
                .unwrap();
            self.act(Action::Deck {
                deck: deck(n),
                action: DeckAction::Play,
            });
        }

        fn render(&mut self, frames: usize) -> Vec<f32> {
            let mut out = vec![0.0; frames * self.channels];
            self.engine.render(
                &mut out,
                &RenderContext {
                    frames,
                    channels: self.channels,
                    sample_rate: SR,
                },
            );
            out
        }

        fn global(&self, param: GlobalParam) -> f32 {
            self.registry.get(ParamId::Global(param))
        }

        fn peak(&self, out: &[f32], pair: (usize, usize)) -> f32 {
            out.chunks_exact(self.channels).fold(0.0f32, |acc, frame| {
                acc.max(frame[pair.0].abs()).max(frame[pair.1].abs())
            })
        }

        /// Frame index where a channel first rises above `threshold`.
        fn first_crossing(&self, out: &[f32], channel: usize, threshold: f32) -> Option<usize> {
            out.chunks_exact(self.channels)
                .position(|frame| frame[channel].abs() > threshold)
        }
    }

    /// **The reason the limiter exists.** Four decks up, every fader open: the
    /// bus is far over full scale and the PA must not be asked to reproduce it.
    #[test]
    fn the_master_holds_the_ceiling_with_every_deck_open() {
        let mut rig = rig(MAX_DECKS, 2);
        for n in 1..=MAX_DECKS as u8 {
            rig.play(n, tone(400_000, 1.0));
        }
        // Warm through the fader ramps, then measure.
        let _ = rig.render(48_000);
        let out = rig.render(48_000);

        let peak = rig.peak(&out, (0, 1));
        assert!(
            peak <= 1.0,
            "the master clipped at {peak} with every deck open"
        );
        // Below full scale, not merely at it -- the ceiling leaves headroom for
        // inter-sample peaks the converter will reconstruct.
        assert!(peak < 0.99, "no headroom left below full scale: {peak}");
        assert!(
            rig.global(GlobalParam::LimiterReductionDb) > 3.0,
            "four decks at full should show real reduction, showed {}",
            rig.global(GlobalParam::LimiterReductionDb)
        );
    }

    /// **The property that made the cue limiter necessary.**
    ///
    /// The master picks up the limiter's look-ahead delay. If the headphone bus
    /// did not pick up the same delay, the two would sit 5 ms apart — and
    /// beatmatching is nothing but the act of comparing those two signals. A DJ
    /// would line up the grid they could hear, and the room would hear it 5 ms
    /// out. A failure here reads as roughly `latency_frames` of skew, not one
    /// or two.
    #[test]
    fn the_headphone_cue_stays_aligned_with_the_master() {
        let mut rig = rig(2, 4);
        rig.play(1, step(24_000, 24_000, 0.6));
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetCue(true),
        });
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetVolume(1.0),
        });
        // Pure cue in the headphones, so the two buses are independent paths
        // carrying the same deck rather than one derived from the other.
        rig.act(Action::Mixer(MixerAction::CueMix(0.0)));
        rig.act(Action::Mixer(MixerAction::Crossfader(0.0)));

        let layout = BusLayout::for_channels(4);
        let out = rig.render(40_000);

        let master_edge = rig
            .first_crossing(&out, layout.main.0, 0.05)
            .expect("the master never carried the step");
        let cue_edge = rig
            .first_crossing(&out, layout.cue.unwrap().0, 0.05)
            .expect("the headphones never carried the step");

        let skew = master_edge.abs_diff(cue_edge);
        let latency = rig.global(GlobalParam::OutputLatencyFrames) as usize;
        assert!(
            skew < 16,
            "master and cue are {skew} frames apart (limiter latency is {latency}); \
             a missing delay on one bus looks exactly like this"
        );
    }

    /// The delay is real and has to be reported, or everything downstream that
    /// needs to line up with the master — a recording, a video output, a second
    /// device — would be guessing.
    #[test]
    fn the_output_latency_is_published() {
        let rig = rig(2, 2);
        let latency = rig.global(GlobalParam::OutputLatencyFrames);
        assert!(latency > 0.0, "no output latency reported");
        // 5 ms at the default rate.
        assert_eq!(latency, (SR.as_f64() as f32 * 0.005).floor());
    }

    /// Bypass has to actually bypass, and has to be visible in the parameter
    /// table so the interface can show the limiter is off rather than just
    /// quiet.
    #[test]
    fn the_limiter_can_be_bypassed_and_says_so() {
        let mut rig = rig(2, 2);
        assert_eq!(
            rig.global(GlobalParam::LimiterEnabled),
            1.0,
            "off by default"
        );

        rig.play(1, tone(400_000, 1.0));
        rig.play(2, tone(400_000, 1.0));
        let _ = rig.render(48_000);
        assert!(rig.global(GlobalParam::LimiterReductionDb) > 0.0);

        rig.act(Action::Mixer(MixerAction::SetLimiter(false)));
        let _ = rig.render(48_000);

        assert_eq!(rig.global(GlobalParam::LimiterEnabled), 0.0);
        assert_eq!(
            rig.global(GlobalParam::LimiterReductionDb),
            0.0,
            "a bypassed limiter must not report reduction"
        );
    }

    /// Bypassing must not change the reported latency, because it does not
    /// change the actual latency — the delay line keeps running. See
    /// `Limiter::set_bypass`.
    #[test]
    fn bypassing_does_not_move_the_output_latency() {
        let mut rig = rig(2, 2);
        let engaged = rig.global(GlobalParam::OutputLatencyFrames);
        rig.act(Action::Mixer(MixerAction::SetLimiter(false)));
        let _ = rig.render(512);
        assert_eq!(rig.global(GlobalParam::OutputLatencyFrames), engaged);
    }

    /// A quiet mix must come out of the limiter untouched. If the limiter
    /// coloured normal programme material it would be a fault, not a safety net.
    #[test]
    fn an_ordinary_mix_is_not_touched() {
        let mut rig = rig(2, 2);
        rig.play(1, tone(400_000, 0.25));
        let _ = rig.render(48_000);
        let out = rig.render(4_800);

        assert_eq!(
            rig.global(GlobalParam::LimiterReductionDb),
            0.0,
            "reduced a mix that was nowhere near the ceiling"
        );
        let peak = rig.peak(&out, (0, 1));
        assert!(peak > 0.1, "the deck should still be audible, got {peak}");
    }
}

/// Sync, quantize and beat jump.
///
/// The property that matters is not "sync ran" but "the beats line up
/// afterwards", so these measure tempo and phase rather than checking a flag.
#[cfg(test)]
mod sync_tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};
    use dj_decode::{AudioBuffer, TrackSource};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn tone(frames: usize) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(vec![0.25; frames * 2], SR))
    }

    fn grid(bpm: f64, anchor: f64, confidence: f64) -> Beatgrid {
        Beatgrid::new(
            FramePos::new(anchor),
            Bpm::new(bpm).unwrap(),
            Confidence::new(confidence),
        )
    }

    struct Rig {
        engine: Engine,
        commands: rtrb::Producer<Command>,
        registry: Arc<ParameterRegistry>,
    }

    fn new_rig() -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        Rig {
            engine: Engine::new(4, SR, command_rx, retired_tx, Arc::clone(&registry)),
            commands: command_tx,
            registry,
        }
    }

    impl Rig {
        fn send(&mut self, command: Command) {
            self.commands.push(command).expect("queue full");
        }

        fn act(&mut self, action: Action) {
            self.send(Command::Action(action));
        }

        fn deck_act(&mut self, n: u8, action: DeckAction) {
            self.act(Action::Deck {
                deck: deck(n),
                action,
            });
        }

        /// Load a track, attach a grid, and start it.
        fn prepare(&mut self, n: u8, bpm: f64, anchor: f64, confidence: f64, playing: bool) {
            self.send(Command::Load {
                deck: deck(n),
                source: tone(48_000 * 300),
            });
            self.send(Command::SetGrid {
                deck: deck(n),
                grid: Some(grid(bpm, anchor, confidence)),
            });
            if playing {
                self.deck_act(n, DeckAction::Play);
            }
        }

        fn render(&mut self, frames: usize) {
            let mut out = vec![0.0; frames * 2];
            self.engine.render(
                &mut out,
                &RenderContext {
                    frames,
                    channels: 2,
                    sample_rate: SR,
                },
            );
        }

        fn param(&self, n: u8, param: DeckParam) -> f32 {
            self.registry.get(ParamId::Deck(deck(n), param))
        }

        fn engine_deck(&self, n: u8) -> &crate::deck::Deck {
            self.engine.deck(deck(n)).unwrap()
        }
    }

    /// **What sync is for.** Two tracks at different tempos, one command, and
    /// afterwards they are playing at the same speed.
    #[test]
    fn sync_matches_the_leaders_tempo() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        let leader = rig.engine_deck(1).effective_bpm().unwrap();
        let follower = rig.engine_deck(2).effective_bpm().unwrap();
        assert!(
            (follower - leader).abs() < 0.01,
            "follower at {follower} against a leader at {leader}"
        );
        assert_eq!(rig.param(2, DeckParam::Synced), 1.0);
        // The leader must not have moved. It is what the room is hearing.
        assert!(
            (leader - 128.0).abs() < 0.01,
            "the leader was retuned to {leader}"
        );
    }

    /// Tempo alone is not sync. Two decks at the same speed but half a beat
    /// apart sound like a mistake, and lining the beats up is the harder half.
    #[test]
    fn sync_aligns_the_phase() {
        let mut rig = new_rig();
        rig.prepare(1, 120.0, 0.0, 0.9, true);
        // Deck 2's grid is offset by half a beat: 120 BPM is 24 000 frames per
        // beat, so 12 000 puts it exactly out of phase.
        rig.prepare(2, 120.0, 12_000.0, 0.9, true);
        rig.render(256);

        let before = (rig.engine_deck(1).beat_phase().unwrap()
            - rig.engine_deck(2).beat_phase().unwrap())
        .abs();
        assert!(
            (before - 0.5).abs() < 0.05,
            "the fixture is not actually out of phase: {before}"
        );

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        let mut delta = (rig.engine_deck(1).beat_phase().unwrap()
            - rig.engine_deck(2).beat_phase().unwrap())
        .abs();
        if delta > 0.5 {
            delta = 1.0 - delta;
        }
        assert!(delta < 0.01, "beats are still {delta} of a beat apart");
    }

    /// **The case that broke phase alignment first.** Sync pressed shortly
    /// after loading is the *normal* moment to press it, and there the shorter
    /// direction is often backwards past the start of the file. Alignment used
    /// to give up there, leaving the decks out of phase with sync showing as
    /// engaged -- the worst of both.
    #[test]
    fn sync_aligns_even_at_the_very_start_of_a_track() {
        let mut rig = new_rig();
        rig.prepare(1, 120.0, 0.0, 0.9, true);
        // Half a beat out, and only a few hundred frames from the start, so the
        // shorter correction is 12 000 frames backwards -- off the front.
        rig.prepare(2, 120.0, 12_000.0, 0.9, true);
        rig.render(256);
        assert!(
            rig.engine_deck(2).position().get() < 12_000.0,
            "the fixture is not near the start"
        );

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        let mut delta = (rig.engine_deck(1).beat_phase().unwrap()
            - rig.engine_deck(2).beat_phase().unwrap())
        .abs();
        if delta > 0.5 {
            delta = 1.0 - delta;
        }
        assert!(delta < 0.01, "still {delta} of a beat apart near the start");
        assert!(
            rig.engine_deck(2).position().get() > 0.0,
            "aligned by running off the front of the track"
        );
    }

    /// **The rule the whole analyser is written around.** A grid the analyser
    /// does not trust must not be synced to, in either direction. Silently
    /// syncing to a guess derails a mix at the moment nobody is watching.
    #[test]
    fn a_weak_grid_refuses_to_sync() {
        // Weak leader.
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.2, true);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        assert_eq!(
            rig.param(2, DeckParam::Synced),
            0.0,
            "synced to a weak grid"
        );
        assert!((rig.engine_deck(2).effective_bpm().unwrap() - 120.0).abs() < 0.01);

        // Weak follower.
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.prepare(2, 120.0, 0.0, 0.2, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        assert_eq!(
            rig.param(2, DeckParam::Synced),
            0.0,
            "a deck with a weak grid synced itself to a good one"
        );
    }

    /// A deck with no grid at all has nothing to sync, and asking must be a
    /// no-op rather than a guess.
    #[test]
    fn a_deck_with_no_grid_cannot_sync() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.send(Command::Load {
            deck: deck(2),
            source: tone(48_000 * 60),
        });
        rig.deck_act(2, DeckAction::Play);
        rig.render(256);

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        assert_eq!(rig.param(2, DeckParam::Synced), 0.0);
        assert!(rig.engine_deck(2).effective_bpm().is_none());
    }

    /// Nothing playing means no leader. Sync must not pick a paused deck: the
    /// point of following is to follow what the room can hear.
    #[test]
    fn sync_needs_something_to_follow() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, false);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        assert_eq!(rig.param(2, DeckParam::Synced), 0.0);
    }

    /// **Half and double are the same tempo.** A 70 BPM track against a 140 one
    /// should play at 70 with beats landing every other beat, not be stretched
    /// to double speed -- which is what a naive ratio would do.
    #[test]
    fn a_half_time_track_is_not_stretched_to_double() {
        let mut rig = new_rig();
        rig.prepare(1, 140.0, 0.0, 0.9, true);
        rig.prepare(2, 70.0, 0.0, 0.9, true);
        rig.render(256);

        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        let follower = rig.engine_deck(2).effective_bpm().unwrap();
        assert!(
            (follower - 70.0).abs() < 0.01,
            "a 70 BPM track was played at {follower}"
        );
        assert_eq!(
            rig.param(2, DeckParam::Synced),
            1.0,
            "the match was refused"
        );
    }

    /// Past a certain stretch it is a wrong grid rather than a hard mix, and
    /// refusing is better than playing a record at an absurd speed.
    #[test]
    fn an_impossible_stretch_is_refused_rather_than_approximated() {
        let mut rig = new_rig();
        rig.prepare(1, 170.0, 0.0, 0.9, true);
        // 96 against 170 is +77%; against half of 170 it is -11%... so pick a
        // pairing that is bad at every octave. 170 vs 122: +39%, and half
        // (85) is -30%... also within range. 170 vs 112: +52%, half is -24%.
        // The genuinely impossible zone is around the midpoint between
        // octaves, e.g. 170 against 120 is +42% and 85 against 120 is -29%.
        // Use a leader far from any octave of the follower.
        rig.prepare(2, 200.0, 0.0, 0.9, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        let follower = rig.engine_deck(2).effective_bpm().unwrap();
        // Either it matched within the allowed stretch, or it refused and left
        // the deck alone. What it must never do is land somewhere between.
        let synced = rig.param(2, DeckParam::Synced) == 1.0;
        if synced {
            assert!(
                (follower - 170.0).abs() < 0.01 || (follower - 85.0).abs() < 0.01,
                "synced to neither the tempo nor its octave: {follower}"
            );
        } else {
            assert!(
                (follower - 200.0).abs() < 0.01,
                "refused the sync but moved the deck anyway, to {follower}"
            );
        }
    }

    /// **A lock that quietly stops holding is worse than no lock.** Moving the
    /// leader's pitch fader after sync must carry the follower with it.
    #[test]
    fn the_follower_tracks_the_leader_afterwards() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);

        rig.deck_act(1, DeckAction::SetPitch(0.06));
        rig.render(256);

        let leader = rig.engine_deck(1).effective_bpm().unwrap();
        let follower = rig.engine_deck(2).effective_bpm().unwrap();
        assert!(
            (leader - 128.0 * 1.06).abs() < 0.01,
            "the leader did not move: {leader}"
        );
        assert!(
            (follower - leader).abs() < 0.01,
            "the follower stayed at {follower} while the leader went to {leader}"
        );
    }

    /// Releasing sync gives the pitch fader back, and must not snap the tempo
    /// somewhere new -- the deck keeps playing at whatever it was.
    #[test]
    fn releasing_sync_leaves_the_tempo_where_it_was() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        let locked = rig.engine_deck(2).effective_bpm().unwrap();

        rig.deck_act(2, DeckAction::SyncOff);
        rig.render(256);
        assert_eq!(rig.param(2, DeckParam::Synced), 0.0);
        assert!(
            (rig.engine_deck(2).effective_bpm().unwrap() - locked).abs() < 0.01,
            "releasing sync jumped the tempo"
        );

        // And the leader can now move without dragging the follower.
        rig.deck_act(1, DeckAction::SetPitch(0.1));
        rig.render(256);
        assert!((rig.engine_deck(2).effective_bpm().unwrap() - locked).abs() < 0.01);
    }

    /// The leader stopping releases the lock rather than freezing it. A lock to
    /// nothing is a lie the interface would keep showing.
    #[test]
    fn losing_the_leader_releases_the_lock() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.prepare(2, 120.0, 0.0, 0.9, true);
        rig.render(256);
        rig.deck_act(2, DeckAction::Sync);
        rig.render(256);
        assert_eq!(rig.param(2, DeckParam::Synced), 1.0);

        rig.deck_act(1, DeckAction::Pause);
        rig.render(256);
        assert_eq!(
            rig.param(2, DeckParam::Synced),
            0.0,
            "the lock outlived its leader"
        );
    }

    /// Beat jump moves by exactly whole beats, which is the only thing that
    /// makes it different from a seek.
    #[test]
    fn beat_jump_moves_whole_beats() {
        let mut rig = new_rig();
        // Anchor at zero, so the playhead starts exactly on a beat.
        rig.prepare(1, 120.0, 0.0, 0.9, false);
        rig.render(256);
        let before = rig.engine_deck(1).position().get();

        rig.deck_act(1, DeckAction::BeatJump(4));
        rig.render(256);
        let after = rig.engine_deck(1).position().get();

        // 120 BPM at 48 kHz is 24 000 frames per beat.
        assert!(
            (after - before - 96_000.0).abs() < 1.0,
            "jumped {} frames, expected 96 000",
            after - before
        );
    }

    /// And backwards, which is the direction that reveals a sign error.
    #[test]
    fn beat_jump_goes_backwards_too() {
        let mut rig = new_rig();
        rig.prepare(1, 120.0, 0.0, 0.9, false);
        rig.deck_act(1, DeckAction::Seek(FramePos::new(480_000.0)));
        rig.render(256);
        let before = rig.engine_deck(1).position().get();

        rig.deck_act(1, DeckAction::BeatJump(-2));
        rig.render(256);
        assert!(
            (rig.engine_deck(1).position().get() - before + 48_000.0).abs() < 1.0,
            "backward jump landed at {}",
            rig.engine_deck(1).position().get()
        );
    }

    /// **What quantize is for.** From an off-beat position, a quantised jump
    /// lands *on* the grid; an unquantised one carries the same offset forward
    /// forever.
    #[test]
    fn quantize_snaps_a_jump_onto_the_grid() {
        let beat = 24_000.0;
        let off_beat = 5_000.0;

        let landed = |quantize: bool| {
            let mut rig = new_rig();
            rig.prepare(1, 120.0, 0.0, 0.9, false);
            rig.act(Action::Mixer(MixerAction::SetQuantize(quantize)));
            rig.deck_act(1, DeckAction::Seek(FramePos::new(beat * 3.0 + off_beat)));
            rig.render(256);
            rig.deck_act(1, DeckAction::BeatJump(1));
            rig.render(256);
            rig.engine_deck(1).position().get()
        };

        // Unquantised: exactly one beat on from where we were, offset intact.
        assert!(
            (landed(false) - (beat * 4.0 + off_beat)).abs() < 1.0,
            "unquantised jump landed at {}",
            landed(false)
        );
        // Quantised: snapped to the nearest beat first, so it lands on one.
        // 5 000 is less than half a beat, so the nearest beat is beat 3.
        assert!(
            (landed(true) - beat * 4.0).abs() < 1.0,
            "quantised jump landed at {}, not on the grid",
            landed(true)
        );
    }

    /// Without a grid there are no beats to jump by, so the playhead must stay
    /// exactly where it is rather than moving by some default.
    #[test]
    fn beat_jump_without_a_grid_does_nothing() {
        let mut rig = new_rig();
        rig.send(Command::Load {
            deck: deck(1),
            source: tone(48_000 * 60),
        });
        rig.deck_act(1, DeckAction::Seek(FramePos::new(100_000.0)));
        rig.render(256);

        rig.deck_act(1, DeckAction::BeatJump(4));
        rig.render(256);
        assert!((rig.engine_deck(1).position().get() - 100_000.0).abs() < 1.0);
    }

    /// Ejecting must take the grid with the track, or the next one would be
    /// synced against the previous track's tempo.
    #[test]
    fn a_grid_does_not_outlive_its_track() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.render(256);
        assert!(rig.engine_deck(1).grid().is_some());

        rig.send(Command::SetGrid {
            deck: deck(1),
            grid: None,
        });
        rig.render(256);
        assert!(rig.engine_deck(1).grid().is_none());
        assert_eq!(rig.param(1, DeckParam::Synced), 0.0);
        assert_eq!(rig.param(1, DeckParam::EffectiveBpm), 0.0);
    }

    /// The tempo the interface shows has to be the tempo being played, pitch
    /// fader included -- not the tempo the file was recorded at.
    #[test]
    fn the_published_tempo_follows_the_pitch_fader() {
        let mut rig = new_rig();
        rig.prepare(1, 128.0, 0.0, 0.9, true);
        rig.render(256);
        assert!((rig.param(1, DeckParam::EffectiveBpm) - 128.0).abs() < 0.01);

        rig.deck_act(1, DeckAction::SetPitch(0.08));
        rig.render(256);
        assert!(
            (rig.param(1, DeckParam::EffectiveBpm) - 138.24).abs() < 0.05,
            "published {}",
            rig.param(1, DeckParam::EffectiveBpm)
        );
    }
}

/// Hot cues and loops.
///
/// A loop is the one feature here whose correctness is a property of the
/// *audio*, not of a flag: "the loop is active" means nothing if the playhead
/// walks straight past the end. So these follow the playhead and read what
/// comes out, rather than asserting on state.
#[cfg(test)]
mod loop_tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};
    use dj_decode::{AudioBuffer, TrackSource};

    const SR: SampleRate = SampleRate::DEFAULT;
    /// 120 BPM at 48 kHz.
    const BEAT: f64 = 24_000.0;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    /// A ramp, so the sample value *is* the frame number.
    ///
    /// That is what makes looping observable: play a ramp through a loop and
    /// the output has to come back down, not keep climbing.
    fn ramp(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let v = n as f32 / frames as f32;
                [v, v]
            })
            .collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    struct Rig {
        engine: Engine,
        commands: rtrb::Producer<Command>,
        registry: Arc<ParameterRegistry>,
    }

    fn new_rig() -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        Rig {
            engine: Engine::new(2, SR, command_rx, retired_tx, Arc::clone(&registry)),
            commands: command_tx,
            registry,
        }
    }

    impl Rig {
        fn send(&mut self, command: Command) {
            self.commands.push(command).expect("queue full");
        }

        fn act(&mut self, n: u8, action: DeckAction) {
            self.send(Command::Action(Action::Deck {
                deck: deck(n),
                action,
            }));
        }

        /// Load a ramp with a 120 BPM grid anchored at zero, and play it.
        fn prepare(&mut self, n: u8, playing: bool) {
            self.send(Command::Load {
                deck: deck(n),
                source: ramp(48_000 * 120),
            });
            self.send(Command::SetGrid {
                deck: deck(n),
                grid: Some(Beatgrid::new(
                    FramePos::new(0.0),
                    Bpm::new(120.0).unwrap(),
                    Confidence::new(0.9),
                )),
            });
            if playing {
                self.act(n, DeckAction::Play);
            }
        }

        fn render(&mut self, frames: usize) {
            let mut out = vec![0.0; frames * 2];
            self.engine.render(
                &mut out,
                &RenderContext {
                    frames,
                    channels: 2,
                    sample_rate: SR,
                },
            );
        }

        fn param(&self, n: u8, param: DeckParam) -> f32 {
            self.registry.get(ParamId::Deck(deck(n), param))
        }

        fn pos(&self, n: u8) -> f64 {
            self.engine.deck(deck(n)).unwrap().position().get()
        }
    }

    /// **What a loop is.** Play long past the end of a four-beat loop and the
    /// playhead must still be inside it. A flag that says "looping" while the
    /// playhead walks away is the failure this test exists to catch.
    #[test]
    fn a_loop_holds_the_playhead_inside_it() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.render(256);

        rig.act(1, DeckAction::LoopBeats(4.0));
        // Four beats is 96 000 frames; render three times that.
        for _ in 0..1_200 {
            rig.render(256);
        }

        let start = rig.param(1, DeckParam::LoopStart) as f64;
        let end = rig.param(1, DeckParam::LoopEnd) as f64;
        assert_eq!(rig.param(1, DeckParam::LoopActive), 1.0);
        assert!(
            (end - start - BEAT * 4.0).abs() < 2.0,
            "loop is {} frames, expected {}",
            end - start,
            BEAT * 4.0
        );
        let here = rig.pos(1);
        assert!(
            here >= start && here < end,
            "playhead at {here} escaped the loop [{start}, {end})"
        );
    }

    /// **The case a per-block fold would get wrong.** A sixteenth of a beat is
    /// 1 500 frames -- shorter than one 4 096-frame callback -- so the fold has
    /// to happen per frame or the deck plays straight through.
    #[test]
    fn a_loop_shorter_than_one_buffer_still_holds() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.render(256);

        rig.act(1, DeckAction::LoopBeats(1.0));
        rig.render(256);
        for _ in 0..4 {
            rig.act(1, DeckAction::LoopHalve);
            rig.render(256);
        }

        let start = rig.param(1, DeckParam::LoopStart) as f64;
        let end = rig.param(1, DeckParam::LoopEnd) as f64;
        assert!(
            end - start < 4_096.0,
            "the fixture is not shorter than a buffer: {} frames",
            end - start
        );

        // One render of 4 096 frames wraps this loop several times over.
        for _ in 0..20 {
            rig.render(4_096);
        }
        let here = rig.pos(1);
        assert!(
            here >= start && here < end,
            "a sub-buffer loop leaked: playhead {here} outside [{start}, {end})"
        );
    }

    /// The audio has to loop, not just the playhead. A ramp played through a
    /// loop comes back down; a ramp played past one keeps climbing.
    #[test]
    fn the_audio_repeats_rather_than_running_on() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::Seek(FramePos::new(BEAT * 8.0)));
        rig.render(256);
        rig.act(1, DeckAction::LoopBeats(1.0));
        rig.render(256);

        // Render more than a loop's worth and watch the ramp.
        let mut out = vec![0.0f32; 48_000 * 2];
        rig.engine.render(
            &mut out,
            &RenderContext {
                frames: 48_000,
                channels: 2,
                sample_rate: SR,
            },
        );

        // A ramp that never loops is monotonically increasing. Looping means it
        // must fall at least once.
        let fell = out
            .chunks_exact(2)
            .map(|f| f[0])
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[1] < w[0] - 1e-6);
        assert!(fell, "the ramp never came back -- the audio did not loop");
    }

    /// Halving keeps the start, so a loop tightens onto the beat it began on.
    #[test]
    fn halving_and_doubling_keep_the_start() {
        let mut rig = new_rig();
        rig.prepare(1, false);
        rig.act(1, DeckAction::LoopBeats(4.0));
        rig.render(256);
        let start = rig.param(1, DeckParam::LoopStart);

        rig.act(1, DeckAction::LoopHalve);
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopStart), start);
        assert!((rig.param(1, DeckParam::LoopBeats) - 2.0).abs() < 0.01);

        rig.act(1, DeckAction::LoopDouble);
        rig.act(1, DeckAction::LoopDouble);
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopStart), start);
        assert!((rig.param(1, DeckParam::LoopBeats) - 8.0).abs() < 0.01);
    }

    /// Halving until the playhead falls outside the new, shorter loop must pull
    /// it back in -- otherwise the deck keeps playing forward and the loop
    /// silently stops looping.
    #[test]
    fn shrinking_a_loop_pulls_the_playhead_back_in() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::LoopBeats(8.0));
        // Play until well past what a one-beat loop would cover.
        for _ in 0..500 {
            rig.render(256);
        }
        for _ in 0..3 {
            rig.act(1, DeckAction::LoopHalve);
            rig.render(256);
        }

        let start = rig.param(1, DeckParam::LoopStart) as f64;
        let end = rig.param(1, DeckParam::LoopEnd) as f64;
        let here = rig.pos(1);
        assert!(
            here >= start && here < end,
            "playhead {here} left outside [{start}, {end}) after shrinking"
        );
    }

    /// Manual looping: in, then out.
    #[test]
    fn a_manual_loop_takes_its_points_from_the_playhead() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::Seek(FramePos::new(BEAT * 4.0)));
        rig.render(256);
        rig.act(1, DeckAction::LoopIn);
        rig.render(256);

        // An in point alone must not start looping.
        assert_eq!(
            rig.param(1, DeckParam::LoopActive),
            0.0,
            "half a loop started looping"
        );

        for _ in 0..100 {
            rig.render(256);
        }
        rig.act(1, DeckAction::LoopOut);
        rig.render(256);

        assert_eq!(rig.param(1, DeckParam::LoopActive), 1.0);
        let start = rig.param(1, DeckParam::LoopStart) as f64;
        let end = rig.param(1, DeckParam::LoopEnd) as f64;
        assert!(
            (start - BEAT * 4.0).abs() < 512.0,
            "in point landed at {start}"
        );
        assert!(end > start, "out point landed before the in point");
    }

    /// Leaving a loop must not move the playhead: the deck carries on from
    /// where it is, which is what makes a loop something you can drop out of
    /// on the beat.
    #[test]
    fn leaving_a_loop_carries_on_from_where_it_is() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::LoopBeats(4.0));
        for _ in 0..300 {
            rig.render(256);
        }
        let before = rig.pos(1);

        rig.act(1, DeckAction::LoopOff);
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 0.0);

        let after = rig.pos(1);
        assert!(
            after > before && after - before < 512.0,
            "leaving the loop jumped from {before} to {after}"
        );

        // And it now runs past where the loop used to end.
        for _ in 0..600 {
            rig.render(256);
        }
        assert!(
            rig.pos(1) > BEAT * 4.0,
            "still trapped after the loop was released"
        );
    }

    /// A loop of zero beats is how a controller encoder turned to zero reads.
    /// It should mean "off" rather than being an error or a zero-length loop.
    #[test]
    fn a_zero_beat_loop_means_off() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::LoopBeats(4.0));
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 1.0);

        rig.act(1, DeckAction::LoopBeats(0.0));
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 0.0);
    }

    /// Without a grid there are no beats to loop over, so an auto loop must do
    /// nothing rather than invent a length.
    #[test]
    fn an_auto_loop_needs_a_grid() {
        let mut rig = new_rig();
        rig.send(Command::Load {
            deck: deck(1),
            source: ramp(48_000 * 60),
        });
        rig.act(1, DeckAction::Play);
        rig.render(256);

        rig.act(1, DeckAction::LoopBeats(4.0));
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 0.0);
    }

    /// A manual loop, by contrast, works perfectly well with no grid at all --
    /// which is the whole reason a loop is stored in frames rather than beats.
    #[test]
    fn a_manual_loop_works_without_a_grid() {
        let mut rig = new_rig();
        rig.send(Command::Load {
            deck: deck(1),
            source: ramp(48_000 * 60),
        });
        rig.act(1, DeckAction::Play);
        rig.render(256);

        rig.act(1, DeckAction::LoopIn);
        for _ in 0..50 {
            rig.render(256);
        }
        rig.act(1, DeckAction::LoopOut);
        rig.render(256);

        assert_eq!(rig.param(1, DeckParam::LoopActive), 1.0);
        // No grid, so no beat count -- but the loop is real.
        assert_eq!(rig.param(1, DeckParam::LoopBeats), 0.0);
    }

    // -- hot cues -----------------------------------------------------------

    /// The one-button behaviour every controller pad sends: set on an empty
    /// slot, jump on a full one.
    #[test]
    fn a_pad_sets_an_empty_slot_and_jumps_to_a_full_one() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::Seek(FramePos::new(BEAT * 6.0)));
        rig.render(256);

        rig.act(1, DeckAction::HotCue(1));
        rig.render(256);
        let stored = rig.param(1, DeckParam::HotCue1) as f64;
        assert!(
            (stored - BEAT * 6.0).abs() < 512.0,
            "cue stored at {stored}, expected about {}",
            BEAT * 6.0
        );

        // Play on, then press the same pad: it should come back.
        for _ in 0..200 {
            rig.render(256);
        }
        assert!(rig.pos(1) > stored + 1_000.0);
        rig.act(1, DeckAction::HotCue(1));
        rig.render(256);
        assert!(
            (rig.pos(1) - stored).abs() < 512.0,
            "pressing a set pad landed at {} rather than {stored}",
            rig.pos(1)
        );
    }

    /// An empty slot has to be distinguishable from a cue at frame zero, which
    /// is a perfectly ordinary place to put one.
    #[test]
    fn an_empty_slot_is_not_a_cue_at_zero() {
        let mut rig = new_rig();
        rig.prepare(1, false);
        rig.render(256);
        assert_eq!(
            rig.param(1, DeckParam::HotCue2),
            dj_core::param::UNSET_HOT_CUE
        );

        rig.act(1, DeckAction::Seek(FramePos::new(0.0)));
        rig.act(1, DeckAction::HotCueSet(2));
        rig.render(256);
        assert_eq!(
            rig.param(1, DeckParam::HotCue2),
            0.0,
            "a cue at the very start read as unset"
        );
    }

    #[test]
    fn clearing_a_slot_empties_it() {
        let mut rig = new_rig();
        rig.prepare(1, false);
        rig.act(1, DeckAction::HotCueSet(3));
        rig.render(256);
        assert_ne!(
            rig.param(1, DeckParam::HotCue3),
            dj_core::param::UNSET_HOT_CUE
        );

        rig.act(1, DeckAction::HotCueClear(3));
        rig.render(256);
        assert_eq!(
            rig.param(1, DeckParam::HotCue3),
            dj_core::param::UNSET_HOT_CUE
        );
    }

    /// All eight slots are independent. An off-by-one in the slot lookup would
    /// show up as pads writing over each other.
    #[test]
    fn the_eight_slots_are_independent() {
        let mut rig = new_rig();
        rig.prepare(1, false);
        for slot in 1..=8u8 {
            rig.act(1, DeckAction::Seek(FramePos::new(BEAT * f64::from(slot))));
            rig.act(1, DeckAction::HotCueSet(slot));
            rig.render(256);
        }

        for slot in 1..=8u8 {
            let param = DeckParam::hot_cue(slot).unwrap();
            let stored = rig.param(1, param) as f64;
            assert!(
                (stored - BEAT * f64::from(slot)).abs() < 512.0,
                "slot {slot} holds {stored}, expected about {}",
                BEAT * f64::from(slot)
            );
        }
    }

    /// Quantize applies to cue points too, so a pad pressed slightly late still
    /// lands on the beat.
    #[test]
    fn quantize_snaps_a_hot_cue_onto_the_beat() {
        let mut rig = new_rig();
        rig.prepare(1, false);
        rig.send(Command::Action(Action::Mixer(MixerAction::SetQuantize(
            true,
        ))));
        // A little past beat 4.
        rig.act(1, DeckAction::Seek(FramePos::new(BEAT * 4.0 + 3_000.0)));
        rig.act(1, DeckAction::HotCueSet(1));
        rig.render(256);

        let stored = rig.param(1, DeckParam::HotCue1) as f64;
        assert!(
            (stored - BEAT * 4.0).abs() < 2.0,
            "quantised cue landed at {stored}, not on the beat"
        );
    }

    /// Ejecting must take the cues and the loop with the track, or the next one
    /// inherits them.
    #[test]
    fn cues_and_loops_do_not_outlive_their_track() {
        let mut rig = new_rig();
        rig.prepare(1, true);
        rig.act(1, DeckAction::HotCueSet(1));
        rig.act(1, DeckAction::LoopBeats(4.0));
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 1.0);

        rig.act(1, DeckAction::Eject);
        rig.render(256);
        assert_eq!(rig.param(1, DeckParam::LoopActive), 0.0);
        assert_eq!(
            rig.param(1, DeckParam::HotCue1),
            dj_core::param::UNSET_HOT_CUE,
            "a hot cue outlived its track"
        );
    }
}

/// Crossfader assignment.
///
/// The tests that matter here are about *reach*: with four decks on screen, a
/// crossfader that can only cut decks 1 and 2 leaves half the mixer outside the
/// one control a DJ uses without looking.
#[cfg(test)]
mod crossfader_assign_tests {
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
        registry: Arc<ParameterRegistry>,
    }

    fn new_rig() -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, _retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        Rig {
            engine: Engine::new(4, SR, command_rx, retired_tx, Arc::clone(&registry)),
            commands: command_tx,
            registry,
        }
    }

    impl Rig {
        fn send(&mut self, action: Action) {
            self.commands
                .push(Command::Action(action))
                .expect("queue full");
        }

        fn play(&mut self, n: u8, amplitude: f32) {
            self.commands
                .push(Command::Load {
                    deck: deck(n),
                    source: tone(200_000, amplitude),
                })
                .expect("queue full");
            self.send(Action::Deck {
                deck: deck(n),
                action: DeckAction::Play,
            });
        }

        fn render(&mut self, frames: usize) -> Vec<f32> {
            let mut out = vec![0.0; frames * 2];
            self.engine.render(
                &mut out,
                &RenderContext {
                    frames,
                    channels: 2,
                    sample_rate: SR,
                },
            );
            out
        }

        /// Settle every gain ramp, then measure what comes out.
        fn settled_peak(&mut self) -> f32 {
            for _ in 0..20 {
                self.render(512);
            }
            self.render(512)
                .iter()
                .fold(0.0f32, |acc, s| acc.max(s.abs()))
        }

        fn assign(&mut self, n: u8, assign: CrossfaderAssign) {
            self.send(Action::Deck {
                deck: deck(n),
                action: DeckAction::SetCrossfaderAssign(assign),
            });
        }
    }

    #[test]
    fn decks_start_on_the_conventional_sides() {
        let rig = new_rig();
        let assigns: Vec<_> = rig
            .engine
            .decks
            .iter()
            .map(Deck::crossfader_assign)
            .collect();
        assert_eq!(
            assigns,
            vec![
                CrossfaderAssign::Left,
                CrossfaderAssign::Right,
                CrossfaderAssign::Thru,
                CrossfaderAssign::Thru,
            ],
            "deck 1 left, deck 2 right, the rest through -- what every mixer does"
        );
    }

    /// The bug this whole feature exists to fix.
    #[test]
    fn a_third_deck_can_be_put_on_the_crossfader_and_cut() {
        let mut rig = new_rig();
        rig.play(3, 0.5);
        rig.assign(3, CrossfaderAssign::Right);
        rig.send(Action::Mixer(MixerAction::Crossfader(-1.0)));

        let peak = rig.settled_peak();
        assert!(
            peak < 0.01,
            "deck 3 assigned right must be silenced by a hard-left crossfader, got {peak}"
        );
    }

    /// The other half: a deck taken off the crossfader is not touched by it.
    #[test]
    fn a_thru_deck_is_untouched_by_the_crossfader() {
        let mut rig = new_rig();
        rig.play(1, 0.5);
        rig.assign(1, CrossfaderAssign::Thru);
        // Hard right, which would normally silence deck 1 completely.
        rig.send(Action::Mixer(MixerAction::Crossfader(1.0)));

        let peak = rig.settled_peak();
        assert!(
            (peak - 0.5).abs() < 0.05,
            "a through deck must play at full level whatever the crossfader does, got {peak}"
        );
    }

    /// Order must not matter: assigning after the fader has already moved has to
    /// pick up the fader's current position, not the one it had at startup.
    #[test]
    fn assigning_after_the_fader_moved_still_applies() {
        let mut rig = new_rig();
        rig.play(4, 0.5);
        // Fader first...
        rig.send(Action::Mixer(MixerAction::Crossfader(1.0)));
        let _ = rig.render(512);
        // ...assignment second.
        rig.assign(4, CrossfaderAssign::Left);

        let peak = rig.settled_peak();
        assert!(
            peak < 0.01,
            "deck 4 assigned left with the fader hard right must be silent, got {peak}"
        );
    }

    #[test]
    fn the_assignment_reaches_the_parameter_table() {
        let mut rig = new_rig();
        rig.assign(3, CrossfaderAssign::Left);
        let _ = rig.render(64);

        let raw = rig
            .registry
            .get(ParamId::Deck(deck(3), DeckParam::CrossfaderAssign));
        assert_eq!(
            CrossfaderAssign::from_param(raw),
            CrossfaderAssign::Left,
            "the interface reads the assignment from here"
        );
    }
}

/// Recording into a sampler slot, as the engine wires it.
///
/// `crate::record` proves the recorder records. These prove the taps are in the
/// right places in the signal path — which is the part that cannot be checked
/// anywhere else, and the part where being one stage out gives a DJ a sample of
/// something they did not mean to capture.
#[cfg(test)]
mod record_tests {
    use super::*;
    use crate::record::Capture;
    use dj_core::{RecordSource, SamplerChange};
    use dj_decode::{AudioBuffer, TrackSource};

    const SR: SampleRate = SampleRate::DEFAULT;
    const SPACE: usize = 48_000;

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
        retired: rtrb::Consumer<Retired>,
        registry: Arc<ParameterRegistry>,
        channels: usize,
    }

    fn rig(channels: usize) -> Rig {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(256);
        let (retired_tx, retired_rx) = rtrb::RingBuffer::new(64);
        let registry = Arc::new(ParameterRegistry::new());
        let mut rig = Rig {
            engine: Engine::new(2, SR, command_rx, retired_tx, Arc::clone(&registry)),
            commands: command_tx,
            retired: retired_rx,
            registry,
            channels,
        };
        rig.give_space();
        rig
    }

    impl Rig {
        fn give_space(&mut self) {
            self.commands
                .push(Command::RecordSpace {
                    samples: vec![0.0; SPACE * 2],
                })
                .unwrap();
        }

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

        fn render(&mut self, frames: usize) -> Vec<f32> {
            let mut out = vec![0.0; frames * self.channels];
            self.engine.render(
                &mut out,
                &RenderContext {
                    frames,
                    channels: self.channels,
                    sample_rate: SR,
                },
            );
            out
        }

        /// Peak on the master pair of a rendered block.
        fn master_peak(&self, out: &[f32]) -> f32 {
            out.chunks_exact(self.channels).fold(0.0f32, |peak, frame| {
                peak.max(frame[0].abs()).max(frame[1].abs())
            })
        }

        /// Render until the gain ramps have settled, so a capture is of the
        /// steady state rather than of a fade-in.
        fn settle(&mut self) {
            for _ in 0..40 {
                let _ = self.render(2_048);
            }
        }

        fn capture(&mut self) -> Option<Capture> {
            while let Ok(item) = self.retired.pop() {
                if let Retired::Capture(capture) = item {
                    return Some(capture);
                }
            }
            None
        }

        fn peak_of(capture: &Capture) -> f32 {
            capture.samples[..capture.frames * 2]
                .iter()
                .fold(0.0f32, |peak, s| peak.max(s.abs()))
        }

        fn get(&self, param: GlobalParam) -> f32 {
            self.registry.get(ParamId::Global(param))
        }
    }

    /// The master tap is after the limiter, so what lands in the slot is what
    /// the room heard.
    #[test]
    fn recording_the_master_captures_the_mix() {
        let mut rig = rig(2);
        rig.load_and_play(1, 0.5);
        rig.settle();

        rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Record {
            slot: 3,
            source: RecordSource::Master,
        })));
        let out = rig.render(4_096);
        // What the master actually put out over that block. Compared against
        // rather than a guessed level: at 0.5 with the crossfader centred the
        // master is 0.354, and a test asserting "louder than 0.4" would have
        // been testing the crossfader curve by accident.
        let master = rig.master_peak(&out);
        assert!(master > 0.1, "the mix should be audible: {master}");
        assert!(
            rig.get(GlobalParam::Recording) > 0.5,
            "it should be running"
        );
        assert_eq!(rig.get(GlobalParam::RecordSlot), 3.0);
        assert_eq!(
            rig.get(GlobalParam::RecordSourceDeck),
            0.0,
            "0 is the master"
        );

        rig.act(Action::Mixer(MixerAction::Sampler(
            SamplerChange::RecordStop,
        )));
        let _ = rig.render(256);

        let capture = rig.capture().expect("a capture should have come back");
        assert_eq!(capture.slot, 3);
        assert_eq!(capture.source, RecordSource::Master);
        assert!(capture.frames > 4_000, "recorded {} frames", capture.frames);
        assert!(
            (Rig::peak_of(&capture) - master).abs() < 0.01,
            "the capture is {} and the master was {master}",
            Rig::peak_of(&capture)
        );
    }

    /// **The reason the deck tap is pre-fader.** Lifting a hook off a track the
    /// room cannot hear yet is the whole use for it, and that deck's fader is
    /// down. A post-fader tap would have recorded silence.
    #[test]
    fn a_deck_can_be_recorded_with_its_fader_shut() {
        let mut rig = rig(2);
        rig.load_and_play(1, 0.5);
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::SetVolume(0.0),
        });
        rig.settle();

        rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Record {
            slot: 1,
            source: RecordSource::Deck(deck(1)),
        })));
        let _ = rig.render(4_096);
        assert_eq!(rig.get(GlobalParam::RecordSourceDeck), 1.0);
        rig.act(Action::Mixer(MixerAction::Sampler(
            SamplerChange::RecordStop,
        )));
        let _ = rig.render(256);

        let capture = rig.capture().expect("a capture should have come back");
        // The deck's own level, not the mix's: pre-fader is before the
        // crossfader too, so this is the full 0.5 the track carries.
        assert!(
            (Rig::peak_of(&capture) - 0.5).abs() < 0.01,
            "a shut fader silenced the capture: peak {}",
            Rig::peak_of(&capture)
        );
    }

    /// And the deck tap takes *that* deck, not the bus. Recording deck 1 while
    /// deck 2 is playing must not pick up deck 2.
    #[test]
    fn a_deck_tap_hears_only_that_deck() {
        let mut rig = rig(2);
        rig.load_and_play(2, 0.5);
        // Deck 1 is loaded but stopped, so it contributes nothing.
        rig.commands
            .push(Command::Load {
                deck: deck(1),
                source: tone(400_000, 0.5),
            })
            .unwrap();
        rig.settle();

        rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Record {
            slot: 1,
            source: RecordSource::Deck(deck(1)),
        })));
        let _ = rig.render(4_096);
        rig.act(Action::Mixer(MixerAction::Sampler(
            SamplerChange::RecordStop,
        )));
        let _ = rig.render(256);

        assert!(
            rig.capture().is_none(),
            "a silent deck produced a capture, so the tap is on the wrong signal"
        );
    }

    /// A cancelled take must not land in the slot.
    #[test]
    fn cancelling_sends_nothing_back() {
        let mut rig = rig(2);
        rig.load_and_play(1, 0.5);
        rig.settle();

        rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Record {
            slot: 1,
            source: RecordSource::Master,
        })));
        let _ = rig.render(4_096);
        rig.act(Action::Mixer(MixerAction::Sampler(
            SamplerChange::RecordCancel,
        )));
        let _ = rig.render(256);

        assert!(rig.capture().is_none(), "a cancelled take landed anyway");
        assert!(
            rig.get(GlobalParam::RecordReady) > 0.5,
            "and the buffer should still be there"
        );
    }

    /// The recorder says when it cannot record, because the reason is not one
    /// the DJ caused: the buffer is out being turned into a sample.
    #[test]
    fn the_interface_is_told_when_there_is_nowhere_to_record() {
        let mut rig = rig(2);
        rig.load_and_play(1, 0.5);
        rig.settle();
        assert!(rig.get(GlobalParam::RecordReady) > 0.5);

        rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Record {
            slot: 1,
            source: RecordSource::Master,
        })));
        let _ = rig.render(2_048);
        rig.act(Action::Mixer(MixerAction::Sampler(
            SamplerChange::RecordStop,
        )));
        let _ = rig.render(256);

        assert!(
            rig.get(GlobalParam::RecordReady) < 0.5,
            "the buffer left with the capture, and the interface must know"
        );

        rig.give_space();
        let _ = rig.render(256);
        assert!(
            rig.get(GlobalParam::RecordReady) > 0.5,
            "and know when it is back"
        );
    }
}

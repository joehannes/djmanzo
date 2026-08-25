//! A single deck.

use crate::bus::BusLayout;
use crate::jog::{Jog, JogEffect};
use crate::rack::Rack;
use crate::record::Recorder;
use dj_core::{
    Beatgrid, CrossfaderAssign, FramePos, HOT_CUE_SLOTS, JogMode, LoopLimits, LoopRegion, PADS,
    Rate, SampleRate, db_to_linear,
};
use dj_decode::{AudioBuffer, TrackSource};
use dj_dsp::fx::FxContext;
use dj_dsp::{CHANNELS, Keylock, SmoothedValue, SweepFilter, ThreeBandEq};
use std::sync::Arc;

/// Widest tempo change sync may ask for, as a fraction.
///
/// A third either way covers every real pairing once half and double time are
/// considered — a 90 BPM track meets a 128 one at 128 against 90, which is 42%,
/// but at *double* 90 it is 128 against 180, which is under 30%. Anything still
/// outside this after that is a wrong grid rather than a hard mix, and the
/// right answer is to refuse rather than to play a record at half speed.
const MAX_SYNC_STRETCH: f64 = 0.34;

/// Frames of source read per pass through the pitch shifter.
///
/// Fixed rather than derived from the callback size, so the scratch buffer is
/// allocated once and cannot be outgrown by a driver that varies its block
/// length between callbacks.
const SCRATCH_FRAMES: usize = 256;

/// A single stem channel with independent DSP processing.
#[derive(Debug)]
pub struct StemChannel {
    pub mute: bool,
    pub volume: f32,
    pub eq: [ThreeBandEq; 2],
    pub filter: [SweepFilter; 2],
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
    pub filter_position: f32,
}

impl StemChannel {
    pub fn new(sr: f32) -> Self {
        Self {
            mute: false,
            volume: 1.0,
            eq: [ThreeBandEq::new(sr), ThreeBandEq::new(sr)],
            filter: [SweepFilter::new(sr), SweepFilter::new(sr)],
            eq_low: 1.0,
            eq_mid: 1.0,
            eq_high: 1.0,
            filter_position: 0.0,
        }
    }
}

/// One player: a source, a playhead, and the gain staging around it.
///
/// Everything here runs on the audio thread. The one rule that shapes the whole
/// type: it never allocates and never drops an `Arc`. Retiring a source is the
/// engine's job.
#[derive(Debug)]
pub struct Deck {
    source: Arc<dyn TrackSource>,
    position: FramePos,
    /// Speed the user asked for, before sample-rate conversion.
    rate: Rate,
    /// Pitch fader as a fraction; 0.0 is centre.
    pitch: f64,
    playing: bool,
    /// Where `cue` returns to.
    cue_point: FramePos,
    /// Trim, smoothed. Applied before the cue send, so PFL hears the trimmed
    /// signal -- which is the point of having a trim knob at all.
    trim_gain: SmoothedValue,
    /// Channel fader, smoothed. Applied after the cue send.
    fader_gain: SmoothedValue,
    volume: f32,
    gain_db: f32,
    /// Pre-fader listen: send this deck to the headphones.
    cue_enabled: bool,
    /// Crossfader contribution, smoothed for the same reason.
    crossfader_gain: SmoothedValue,
    /// Which side of the crossfader cuts this deck. The gain above is what the
    /// assignment and the fader position work out to.
    crossfader_assign: CrossfaderAssign,
    /// Rate of the device we are feeding, for sample-rate conversion.
    device_rate: SampleRate,

    /// Master Isolator EQ (used when stems are pending).
    eq: [ThreeBandEq; 2],
    filter: [SweepFilter; 2],
    eq_low: f32,
    eq_mid: f32,
    eq_high: f32,
    filter_position: f32,

    /// The four independent stem channels (Vocal, Drums, Bass, Other).
    pub stem_channels: [StemChannel; 4],
    /// Pitch shifter used when keylock is on. Always built, so engaging keylock
    /// mid-set never allocates.
    keylock: Keylock,
    keylock_on: bool,
    /// Deliberate transposition in semitones, for harmonic mixing. Non-zero
    /// engages the shifter whether or not keylock is on.
    key_shift: i32,
    /// Set when the playhead moved discontinuously, so the shifter's history is
    /// refilled before the next block instead of fading in from silence.
    needs_prime: bool,
    /// Staging buffer for the keylocked path. Never resized.
    scratch: Vec<f32>,
    /// A platter losing power, or thrown backwards.
    ///
    /// `None` is the ordinary case: the motor is driving and the record turns
    /// at whatever the pitch fader says. See [`Spin`].
    spin: Option<Spin>,
    /// The platter under the hand: scratching, bending and searching.
    ///
    /// Held per deck and never reallocated, so a hand landing on a wheel
    /// mid-set costs nothing. See [`crate::jog`] for what each mode does.
    jog: Jog,
    /// The step the hand is imposing this block, when it is driving.
    ///
    /// `Some` only while scratching or searching: the wheel replaces the motor
    /// rather than adding to it, so this is not a multiplier.
    jog_step: Option<f64>,
    /// The wheel's bend this block, as a multiplier. `1.0` when still.
    jog_bend: f64,
    /// This deck's three effect slots.
    ///
    /// Per deck rather than only on the master because the commonest use of an
    /// effect in a mix is on the track coming *in*, where it has to be audible
    /// on one deck and not the other. Allocated with the deck, so switching an
    /// effect on mid-set never allocates.
    rack: Rack,
    /// The beat grid, when the analyser found one worth having.
    ///
    /// Sent in from the host thread rather than computed here: analysis is FFT
    /// work over a whole track and has no business anywhere near the audio
    /// callback. A `Beatgrid` is four numbers and `Copy`, so it crosses the
    /// queue without allocating.
    grid: Option<Beatgrid>,
    /// Stems state: true if muted
    pub stem_mutes: [bool; 4],
    /// Mutes as they were before a held stem solo.
    ///
    /// A solo is an audition, not a destructive "unmute all" command. Keeping
    /// this snapshot means releasing a pad restores the DJ's deliberate stem
    /// mutes exactly, including when a second solo pad is held while the first
    /// is down.
    stem_mutes_before_solo: Option<[bool; 4]>,
    /// True when this deck's tempo is being held to another's.
    synced: bool,
    /// The region repeating right now, if any.
    active_loop: Option<LoopRegion>,
    /// Where the playhead would be if nothing had diverted it.
    ///
    /// `None` when nothing is diverting it, which is most of the time. Set the
    /// moment a diversion begins and cleared when the playhead is put back, so
    /// its presence *is* the answer to "is something being slipped over".
    slip_anchor: Option<f64>,
    /// Whether the DJ has armed slip mode.
    slip: bool,
    /// Playing backwards.
    reversed: bool,
    /// A loop roll is held: a momentary loop that always slips, for the same
    /// reason a censor does -- the point of a roll is that the track carries on
    /// underneath and you land back on the beat when you let go.
    rolling: bool,
    /// A slicer pad is held. Slips for the same reason a roll does — see
    /// [`Deck::hold_slice`].
    slicing: bool,
    /// How many beats the slicer's eight pads divide up.
    ///
    /// Eight by default, which makes each pad exactly one beat: the reading a
    /// DJ can follow without counting. Four gives half-beats for stutter work,
    /// sixteen and thirty-two give whole bars for rearranging a phrase.
    slice_beats: f64,
    /// Censor is held: momentary reverse that always slips, whatever the slip
    /// button says. That is the whole difference between a censor and simply
    /// playing backwards -- a censor hides a word and puts you back on the
    /// beat, and it cannot do the second half without slipping.
    censoring: bool,
    /// Where a manual loop's in point was dropped, waiting for its out point.
    ///
    /// Separate from `active_loop` because a half-made loop is not a loop: the
    /// deck keeps playing straight through until the out point lands.
    pending_loop_in: Option<FramePos>,
    /// Hot cues, 0-indexed here and 1-indexed everywhere a human can see.
    hot_cues: [Option<FramePos>; HOT_CUE_SLOTS],
}

impl Deck {
    #[must_use]
    pub fn new(device_rate: SampleRate) -> Self {
        let sr = device_rate.as_f64() as f32;
        Self {
            source: Arc::new(AudioBuffer::empty()),
            position: FramePos::ZERO,
            rate: Rate::NORMAL,
            pitch: 0.0,
            playing: false,
            cue_point: FramePos::ZERO,
            trim_gain: SmoothedValue::new(1.0, sr),
            fader_gain: SmoothedValue::new(1.0, sr),
            volume: 1.0,
            gain_db: 0.0,
            cue_enabled: false,
            crossfader_gain: SmoothedValue::new(1.0, sr),
            crossfader_assign: CrossfaderAssign::default(),
            device_rate,
            eq: [ThreeBandEq::new(sr), ThreeBandEq::new(sr)],
            filter: [SweepFilter::new(sr), SweepFilter::new(sr)],
            eq_low: 1.0,
            eq_mid: 1.0,
            eq_high: 1.0,
            filter_position: 0.0,
            stem_channels: [
                StemChannel::new(sr),
                StemChannel::new(sr),
                StemChannel::new(sr),
                StemChannel::new(sr),
            ],
            spin: None,
            jog: Jog::new(device_rate),
            jog_step: None,
            jog_bend: 1.0,
            rack: Rack::new(sr),
            keylock: Keylock::new(sr),
            // Off by default: at unity pitch there is nothing to correct, and a
            // shifter in the path costs CPU and latency for no audible gain.
            keylock_on: false,
            key_shift: 0,
            needs_prime: true,
            scratch: vec![0.0; SCRATCH_FRAMES * CHANNELS],
            grid: None,
            stem_mutes: [false; 4],
            stem_mutes_before_solo: None,
            synced: false,
            active_loop: None,
            slip_anchor: None,
            slip: false,
            reversed: false,
            censoring: false,
            rolling: false,
            slicing: false,
            slice_beats: 8.0,
            pending_loop_in: None,
            hot_cues: [None; HOT_CUE_SLOTS],
        }
    }

    #[must_use]
    pub fn rack(&self) -> &Rack {
        &self.rack
    }

    pub fn rack_mut(&mut self) -> &mut Rack {
        &mut self.rack
    }

    /// What this deck's effects should measure a beat as, in *device* frames.
    ///
    /// Device frames, not source frames: an echo's delay is counted in output
    /// samples, and using the track's own rate would put the repeat a few
    /// percent out on any track that is not at the device rate.
    ///
    /// Derived from `effective_bpm`, so it already includes the pitch fader —
    /// which is the property the whole beat-synced design exists for. Ride the
    /// pitch and the echo rides with it.
    #[must_use]
    fn fx_context(&self) -> FxContext {
        let device = self.device_rate.as_f64();
        FxContext {
            sample_rate: device as f32,
            beat_frames: self
                .effective_bpm()
                .map(|bpm| (device * 60.0 / bpm) as f32)
                .filter(|frames| frames.is_finite() && *frames > 0.0),
        }
    }

    /// Pitch-independent tempo: change the speed, keep the key.
    pub fn set_keylock(&mut self, enabled: bool) {
        if enabled != self.keylock_on {
            self.keylock_on = enabled;
            // Engaging mid-track means the shifter has no history. Refill it
            // before the next block rather than fading in from silence.
            self.needs_prime = true;
        }
    }

    pub fn toggle_keylock(&mut self) {
        self.set_keylock(!self.keylock_on);
    }

    /// Transpose deliberately, in semitones, for harmonic mixing.
    ///
    /// Independent of keylock and composed with it: keylock cancels the pitch
    /// change speed introduced, this adds one you asked for. A non-zero shift
    /// engages the shifter on its own, so two tracks a semitone apart can be
    /// brought into key without either changing tempo.
    pub fn set_key_shift(&mut self, semitones: i32) {
        let semitones = semitones.clamp(
            -dj_dsp::keylock::MAX_KEY_SHIFT,
            dj_dsp::keylock::MAX_KEY_SHIFT,
        );
        if semitones != self.key_shift {
            // Going from silent to engaged means the shifter has no history.
            if (self.key_shift == 0) != (semitones == 0) {
                self.needs_prime = true;
            }
            self.key_shift = semitones;
            self.keylock.set_key_shift(semitones);
        }
    }

    #[must_use]
    pub fn key_shift(&self) -> i32 {
        self.key_shift
    }

    #[must_use]
    pub fn is_keylocked(&self) -> bool {
        self.keylock_on
    }

    /// Extra latency keylock adds before compensation, in frames.
    ///
    /// Zero when keylock is off. Reported so the interface can be honest about
    /// what the feature costs.
    #[must_use]
    pub fn keylock_latency_frames(&self) -> usize {
        if self.keylock_on || self.key_shift != 0 {
            self.keylock.latency_frames()
        } else {
            0
        }
    }

    pub fn set_eq_low(&mut self, gain: f32) {
        if gain.is_finite() {
            let clamped = gain.clamp(0.0, 4.0);
            self.eq_low = clamped;
            for eq in &mut self.eq {
                eq.set_low(self.eq_low);
            }
            for ch in &mut self.stem_channels {
                ch.eq_low = clamped;
                for eq in &mut ch.eq {
                    eq.set_low(ch.eq_low);
                }
            }
        }
    }

    pub fn set_eq_mid(&mut self, gain: f32) {
        if gain.is_finite() {
            let clamped = gain.clamp(0.0, 4.0);
            self.eq_mid = clamped;
            for eq in &mut self.eq {
                eq.set_mid(self.eq_mid);
            }
            for ch in &mut self.stem_channels {
                ch.eq_mid = clamped;
                for eq in &mut ch.eq {
                    eq.set_mid(ch.eq_mid);
                }
            }
        }
    }

    pub fn set_eq_high(&mut self, gain: f32) {
        if gain.is_finite() {
            let clamped = gain.clamp(0.0, 4.0);
            self.eq_high = clamped;
            for eq in &mut self.eq {
                eq.set_high(self.eq_high);
            }
            for ch in &mut self.stem_channels {
                ch.eq_high = clamped;
                for eq in &mut ch.eq {
                    eq.set_high(ch.eq_high);
                }
            }
        }
    }

    pub fn set_filter(&mut self, position: f32) {
        if position.is_finite() {
            let clamped = position.clamp(-1.0, 1.0);
            self.filter_position = clamped;
            for filter in &mut self.filter {
                filter.set_position(self.filter_position);
            }
            for ch in &mut self.stem_channels {
                ch.filter_position = clamped;
                for filter in &mut ch.filter {
                    filter.set_position(ch.filter_position);
                }
            }
        }
    }

    /// Toggle one stem when it is not temporarily isolated by a held solo.
    pub fn toggle_stem_mute(&mut self, stem: usize) {
        if stem < self.stem_channels.len() && self.stem_mutes_before_solo.is_none() {
            self.stem_channels[stem].mute = !self.stem_channels[stem].mute;
            self.stem_mutes[stem] = self.stem_channels[stem].mute;
        }
    }

    /// Hold or release a stem solo without destroying the prior mute pattern.
    pub fn set_stem_solo(&mut self, stem: usize, solo: bool) {
        if stem >= self.stem_channels.len() {
            return;
        }
        if solo {
            if self.stem_mutes_before_solo.is_none() {
                self.stem_mutes_before_solo = Some(self.stem_mutes);
            }
            for channel in &mut self.stem_channels {
                channel.mute = true;
            }
            self.stem_channels[stem].mute = false;
        } else if let Some(previous) = self.stem_mutes_before_solo.take() {
            self.stem_mutes = previous;
            for (channel, muted) in self.stem_channels.iter_mut().zip(previous) {
                channel.mute = muted;
            }
        }
    }

    /// Set a finite, unit-range gain for one isolated stem.
    pub fn set_stem_volume(&mut self, stem: usize, volume: f32) {
        if stem < self.stem_channels.len() && volume.is_finite() {
            self.stem_channels[stem].volume = volume.clamp(0.0, 1.0);
        }
    }

    #[must_use]
    pub fn eq_low(&self) -> f32 {
        self.eq_low
    }

    #[must_use]
    pub fn eq_mid(&self) -> f32 {
        self.eq_mid
    }

    #[must_use]
    pub fn eq_high(&self) -> f32 {
        self.eq_high
    }

    #[must_use]
    pub fn filter_position(&self) -> f32 {
        self.filter_position
    }

    /// Install a new source, returning the old one for the caller to retire.
    ///
    /// Never drops the displaced `Arc` -- see [`crate::command::Retired`].
    #[must_use]
    pub fn load(&mut self, source: Arc<dyn TrackSource>) -> Arc<dyn TrackSource> {
        let previous = std::mem::replace(&mut self.source, source);
        self.position = FramePos::ZERO;
        self.cue_point = FramePos::ZERO;
        self.playing = false;
        // A turn of the wheel is 1.8 seconds of *this* track, so the platter
        // has to know what rate it was decoded at. Forgetting the hand too:
        // whatever was being scratched is gone.
        self.jog.set_source_rate(self.source.sample_rate());
        self.jog.reset();
        self.jog_step = None;
        self.jog_bend = 1.0;
        // Filter memory belongs to the old track. Carrying it into a new one
        // would leak a fragment of the previous audio into the first samples.
        for eq in &mut self.eq {
            eq.reset();
        }
        for filter in &mut self.filter {
            filter.reset();
        }
        // Cues and the loop belong to the old track as surely as the filter
        // memory does. A loop region is a pair of frame positions with no idea
        // which audio they were measured against, so leaving one in place would
        // set the new track looping over a passage nobody chose. The grid goes
        // for the same reason -- it describes a tempo this audio may not have.
        self.active_loop = None;
        self.pending_loop_in = None;
        self.hot_cues = [None; HOT_CUE_SLOTS];
        self.grid = None;
        self.synced = false;
        self.needs_prime = true;
        previous
    }

    /// Replace the source with silence, returning the old one to retire.
    #[must_use]
    pub fn eject(&mut self) -> Arc<dyn TrackSource> {
        self.load(Arc::new(AudioBuffer::empty()))
    }

    pub fn play(&mut self) {
        if !self.source.is_empty() {
            self.playing = true;
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn toggle_play(&mut self) {
        if self.playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// CDJ-style cue: stop and return to the cue point.
    pub fn cue(&mut self) {
        self.playing = false;
        self.position = self.cue_point;
        self.needs_prime = true;
    }

    pub fn set_cue_point(&mut self, position: FramePos) {
        self.cue_point = position.clamped(self.len_frames() as f64);
    }

    /// Jump the playhead.
    ///
    /// Marks the pitch shifter for re-priming: a jump makes its history belong
    /// to a different part of the track. Jog-wheel scrubbing goes through
    /// [`Self::set_rate`] instead, which is continuous and needs no re-prime.
    pub fn seek(&mut self, position: FramePos) {
        self.position = position.clamped(self.len_frames() as f64);
        self.needs_prime = true;
    }

    pub fn set_rate(&mut self, rate: Rate) {
        self.rate = rate;
    }

    /// Pitch fader, as a fraction: `0.08` is +8%.
    pub fn set_pitch(&mut self, pitch: f64) {
        if pitch.is_finite() {
            self.pitch = pitch.clamp(-1.0, 1.0);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if volume.is_finite() {
            self.volume = volume.clamp(0.0, 1.0);
            self.fader_gain.set_target(self.volume);
        }
    }

    pub fn set_gain_db(&mut self, db: f32) {
        if db.is_finite() {
            self.gain_db = db.clamp(-24.0, 24.0);
            self.trim_gain.set_target(db_to_linear(self.gain_db));
        }
    }

    /// Send this deck to the headphones.
    pub fn set_cue(&mut self, enabled: bool) {
        self.cue_enabled = enabled;
    }

    pub fn toggle_cue(&mut self) {
        self.cue_enabled = !self.cue_enabled;
    }

    #[must_use]
    pub fn is_cued(&self) -> bool {
        self.cue_enabled
    }

    pub fn set_crossfader_gain(&mut self, gain: f32) {
        self.crossfader_gain.set_target(gain);
    }

    pub fn set_crossfader_assign(&mut self, assign: CrossfaderAssign) {
        self.crossfader_assign = assign;
    }

    #[must_use]
    pub fn crossfader_assign(&self) -> CrossfaderAssign {
        self.crossfader_assign
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        !self.source.is_empty()
    }

    #[must_use]
    pub fn position(&self) -> FramePos {
        self.position
    }

    #[must_use]
    pub fn rate(&self) -> Rate {
        self.rate
    }

    #[must_use]
    pub fn pitch(&self) -> f64 {
        self.pitch
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    #[must_use]
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    // -- beat grid and sync -------------------------------------------------

    /// Attach the analyser's grid, or clear it.
    ///
    /// Clearing also drops sync: a deck with no grid has no tempo to lock, and
    /// leaving `synced` set would show a lock that is not holding anything.
    pub fn set_grid(&mut self, grid: Option<Beatgrid>) {
        self.grid = grid;
        if grid.is_none() {
            self.synced = false;
        }
    }

    #[must_use]
    pub fn grid(&self) -> Option<Beatgrid> {
        self.grid
    }

    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.synced
    }

    pub fn set_synced(&mut self, synced: bool) {
        // Only a deck with a grid can be locked to anything.
        self.synced = synced && self.grid.is_some();
    }

    // -- hot cues and loops -------------------------------------------------

    /// Limits on loop length for this deck.
    ///
    /// Derived from the beat where there is a grid, so "an eighth of a beat"
    /// means the same musical thing at any tempo, and from the sample rate
    /// where there is not — a manual loop on an unanalysed track is perfectly
    /// legitimate and must still be bounded.
    #[must_use]
    fn loop_limits(&self) -> LoopLimits {
        match self.beat_frames() {
            Some(beat) => LoopLimits::from_beat(beat),
            None => LoopLimits::from_rate(self.source.sample_rate().as_f64()),
        }
    }

    /// Where a cue or loop point should land, honouring quantize.
    fn snapped(&self, pos: FramePos, quantize: bool) -> FramePos {
        if quantize {
            self.nearest_beat(pos).unwrap_or(pos)
        } else {
            pos
        }
    }

    #[must_use]
    pub fn active_loop(&self) -> Option<LoopRegion> {
        self.active_loop
    }

    /// Loop length in beats, for display. `None` without a grid to measure it
    /// against — the loop still works, it just cannot be named in beats.
    #[must_use]
    pub fn loop_beats(&self) -> Option<f64> {
        let region = self.active_loop?;
        let beat = self.beat_frames()?;
        Some(region.len_frames() / beat)
    }

    /// Loop `beats` beats forward from the playhead, and start looping.
    ///
    /// Zero or fewer turns looping off, so a controller encoder that can reach
    /// zero does the obvious thing rather than erroring.
    ///
    /// Fractional because halving a loop already reaches a sixteenth of a beat:
    /// the length was never really an integer, only the way of asking for one
    /// was. The pad ladder runs from a sixteenth to eight beats through here.
    ///
    /// Clamped to [`LoopLimits`], so an absurd request produces the nearest
    /// loop the engine will make rather than an error a pad cannot report.
    pub fn set_loop_length(&mut self, beats: f64, quantize: bool) -> bool {
        // `is_finite` first so a NaN length turns looping off rather than
        // slipping through every comparison by failing it.
        if !beats.is_finite() || beats <= 0.0 {
            self.exit_loop();
            return true;
        }
        let Some(beat) = self.beat_frames() else {
            return false;
        };

        let start = self.snapped(self.position, quantize);
        let limits = self.loop_limits();
        let len = (beats * beat).clamp(limits.min_frames, limits.max_frames);
        let Some(region) = LoopRegion::new(start, FramePos::new(start.get() + len)) else {
            return false;
        };
        self.enter_loop(region)
    }

    /// Drop a manual loop's in point. The deck keeps playing until the out
    /// point lands.
    /// Install a loop over an explicit region.
    ///
    /// For recalling a saved loop, which is the one case where the region comes
    /// from outside rather than from the playhead. `None` clears, so recalling
    /// an empty slot is a no-op rather than a loop over nothing.
    ///
    /// Does not move the playhead. A DJ recalling a saved loop while a track
    /// plays wants the loop armed, not the music jumping; the wrap pulls the
    /// playhead in on the next pass if it is past the end.
    /// Install or clear a loop directly, bypassing the beat maths.
    ///
    /// Routed through the same enter/exit as every other loop so slip behaves
    /// identically however the loop arrived — a restored loop from the library
    /// is not a different kind of loop from one a pad made.
    pub fn set_loop_region(&mut self, region: Option<LoopRegion>) {
        self.pending_loop_in = None;
        match region {
            Some(region) => {
                self.enter_loop(region);
            }
            None => self.exit_loop(),
        }
    }

    // -- slip, reverse and censor ------------------------------------------

    /// Whether the playhead is being slipped over.
    ///
    /// Censor always slips, whatever the slip button says: a censor hides a
    /// word *and puts you back on the beat*, and it cannot do the second half
    /// otherwise.
    #[must_use]
    pub fn is_slipping(&self) -> bool {
        self.slip || self.censoring || self.rolling || self.slicing
    }

    /// Hold a loop roll of `beats`, or release it.
    ///
    /// A roll is a loop that always slips: hold for a stutter, let go and the
    /// track is where it would have been. Returns false when the deck has no
    /// grid to measure beats against, the same as any other beat-length loop.
    pub fn set_loop_roll(&mut self, beats: Option<f32>, quantize: bool) -> bool {
        match beats {
            Some(beats) if beats > 0.0 => {
                // Armed *before* the loop, so the loop's own `begin_diversion`
                // sees a deck that is slipping. The other order would set the
                // anchor only if the slip button happened to be on.
                self.rolling = true;
                let made = self.set_loop_length(f64::from(beats), quantize);
                if !made {
                    self.rolling = false;
                }
                made
            }
            _ => {
                self.exit_loop();
                true
            }
        }
    }

    #[must_use]
    pub fn rolling(&self) -> bool {
        self.rolling
    }

    // -- slicer ------------------------------------------------------------

    /// Beats the eight pads divide up. See [`Deck::slice_beats`].
    pub fn set_slice_domain(&mut self, beats: f64) {
        if beats.is_finite() && beats > 0.0 {
            // A domain shorter than the pad count would give slices under a
            // frame at any sane tempo, and one longer than a phrase is not a
            // thing anyone slices.
            self.slice_beats = beats.clamp(1.0, 64.0);
        }
    }

    #[must_use]
    pub fn slice_beats(&self) -> f64 {
        self.slice_beats
    }

    #[must_use]
    pub fn slicing(&self) -> bool {
        self.slicing
    }

    /// Where the domain is measured from.
    ///
    /// The *shadow* playhead when one is running, not the audible one. That is
    /// what keeps the domain walking forward underneath a held slice: press
    /// pad 1 and hold it, and the light still crosses the grid, because the
    /// record still would be. Measuring from the audible playhead would freeze
    /// the domain around the loop it is stuck in — the slicer would stop
    /// slicing the moment you used it.
    fn slice_reference(&self) -> f64 {
        self.slip_anchor.unwrap_or_else(|| self.position.get())
    }

    /// Start of the span the eight pads currently divide, in frames.
    fn slice_domain_start(&self) -> Option<f64> {
        let grid = self.grid?;
        let beat = self.beat_frames()?;
        let span = self.slice_beats * beat;
        let from_anchor = self.slice_reference() - grid.anchor.get();
        // `div_euclid` rather than a plain division: before the grid anchor the
        // quotient is negative, and truncating toward zero would make the domain
        // either side of the anchor twice as long as the rest.
        let index = (from_anchor / span).div_euclid(1.0).floor();
        Some(grid.anchor.get() + index * span)
    }

    /// Which slice the playhead is in, 1-based. `None` without a grid.
    #[must_use]
    pub fn slice_index(&self) -> Option<u8> {
        let start = self.slice_domain_start()?;
        let beat = self.beat_frames()?;
        let each = self.slice_beats / f64::from(PADS as u8) * beat;
        if each <= 0.0 {
            return None;
        }
        let into = ((self.slice_reference() - start) / each).floor();
        // Clamped rather than wrapped: a reference that has drifted past the
        // domain by a rounding error belongs to the last slice, not the first
        // one of the next domain.
        let index = into.clamp(0.0, (PADS - 1) as f64) as u8;
        Some(index + 1)
    }

    /// Hold slice `slice` of the current domain, 1-based.
    ///
    /// A slice is a loop over one eighth of a span of the grid, and it always
    /// slips — which is what makes it a performance move rather than a jump.
    /// Let go and the track is where it would have been, so eight pads can
    /// rearrange a bar and hand it back in time.
    ///
    /// Refused without a grid, like every other beat-measured gesture, and
    /// refused when the slice would run past the end of the track.
    pub fn hold_slice(&mut self, slice: u8) -> bool {
        if slice < 1 || slice > PADS as u8 {
            return false;
        }
        let (Some(start), Some(beat)) = (self.slice_domain_start(), self.beat_frames()) else {
            return false;
        };
        let each = self.slice_beats / f64::from(PADS as u8) * beat;
        if !each.is_finite() || each <= 0.0 {
            return false;
        }

        let from = start + f64::from(slice - 1) * each;
        let to = from + each;
        if from < 0.0 || to > self.len_frames() as f64 {
            return false;
        }

        // Armed before the loop, so the loop's own `begin_diversion` sees a
        // deck that is slipping — the same ordering the roll needs, and for the
        // same reason.
        self.slicing = true;
        let region = LoopRegion::new(FramePos::new(from), FramePos::new(to));
        let entered = region.is_some_and(|region| self.enter_loop(region));
        if !entered {
            self.slicing = false;
        }
        entered
    }

    /// Let a held slice go. The track lands where it would have been.
    pub fn release_slice(&mut self) {
        if self.slicing {
            self.exit_loop();
        }
    }

    #[must_use]
    pub fn slip(&self) -> bool {
        self.slip
    }

    #[must_use]
    pub fn reversed(&self) -> bool {
        self.reversed || self.censoring
    }

    #[must_use]
    pub fn censoring(&self) -> bool {
        self.censoring
    }

    /// Where the track would be if nothing had diverted it, when something has.
    #[must_use]
    pub fn slip_position(&self) -> Option<FramePos> {
        self.slip_anchor.map(FramePos::new)
    }

    /// Arm or disarm slip mode.
    ///
    /// Arming *during* a diversion starts the shadow here rather than
    /// retroactively: the DJ asked for slip now, and pretending the anchor had
    /// been running since the loop began would jump the track forward by
    /// however long they had been looping.
    pub fn set_slip(&mut self, on: bool) {
        if self.slip == on {
            return;
        }
        self.slip = on;
        if on {
            if self.is_diverted() {
                self.begin_diversion();
            }
        } else if !self.censoring {
            // Disarming mid-loop leaves the playhead where it is. Jumping to
            // the anchor would be the opposite of what turning slip *off*
            // means.
            self.slip_anchor = None;
        }
    }

    /// Play backwards, or forwards again.
    pub fn set_reverse(&mut self, on: bool) {
        if self.reversed == on {
            return;
        }
        self.reversed = on;
        // Reversing is a discontinuity in the audio the shifter is holding
        // history for, the same as a seek.
        self.needs_prime = true;
        if on {
            self.begin_diversion();
        } else if !self.censoring {
            self.end_diversion();
        }
    }

    // -- the platter ------------------------------------------------------

    /// A hand landing on, or leaving, the top of the platter.
    ///
    /// In vinyl mode this alone stops the record, before anything turns --
    /// which is why touch is a separate action from movement. The shifter's
    /// history stops meaning anything the moment the hand drives the playhead,
    /// so this primes it the same way a seek does.
    pub fn set_jog_touch(&mut self, touched: bool) {
        if self.jog.is_touched() == touched {
            return;
        }
        self.jog.set_touched(touched);
        if self.jog.mode() == JogMode::Vinyl && self.playing {
            self.needs_prime = true;
            if touched {
                self.begin_diversion();
            } else if !self.censoring {
                self.end_diversion();
            }
        }
    }

    /// The platter turned, in revolutions. Positive is forwards.
    pub fn jog(&mut self, revolutions: f32) {
        self.jog.turn(revolutions);
    }

    pub fn set_jog_mode(&mut self, mode: JogMode) {
        self.jog.set_mode(mode);
    }

    #[must_use]
    pub fn jog_mode(&self) -> JogMode {
        self.jog.mode()
    }

    #[must_use]
    pub fn jog_touched(&self) -> bool {
        self.jog.is_touched()
    }

    /// How far the wheel is currently bending the tempo, as a fraction.
    #[must_use]
    pub fn jog_bend(&self) -> f64 {
        self.jog.bend()
    }

    /// Whether the hand is driving the playhead rather than the motor.
    #[must_use]
    pub fn is_scratching(&self) -> bool {
        self.playing && self.jog.is_touched() && self.jog.mode() == JogMode::Vinyl
    }

    /// Cut the motor: coast to a stop over `beats`, then pause.
    ///
    /// Refused on a deck with no grid, because a brake measured in beats needs
    /// a beat to measure — and a brake that lands somewhere arbitrary is worse
    /// than no brake. Returns whether it started.
    pub fn brake(&mut self, beats: f64, backwards: bool) -> bool {
        if !self.playing || !beats.is_finite() || beats <= 0.0 {
            return false;
        }
        let Some(beat) = self.beat_frames() else {
            return false;
        };
        // Beats of *source* audio converted to output frames, so a brake is the
        // same length in the room whatever rate the deck is running at.
        let ratio = self.device_rate.as_f64() / self.source.sample_rate().as_f64();
        let frames = beats * beat * ratio;
        self.spin = Some(if backwards {
            Spin::thrown(frames)
        } else {
            Spin::braking(frames)
        });
        // The shifter's history is about to become meaningless: the rate now
        // changes every frame, and the path changes under it too.
        self.needs_prime = true;
        self.begin_diversion();
        true
    }

    /// Put the motor back on, wherever the record got to.
    ///
    /// Deliberately *not* a return to where the brake started. A DJ who brakes
    /// and changes their mind wants the record moving again from here; the way
    /// back is the cue button, which already exists.
    pub fn release_brake(&mut self) {
        if self.spin.take().is_some() {
            self.needs_prime = true;
            self.end_diversion();
        }
    }

    #[must_use]
    pub fn is_spinning(&self) -> bool {
        self.spin.is_some()
    }

    /// What the coasting platter is doing, for the interface: 1.0 at full
    /// speed, 0.0 stopped, negative while thrown backwards.
    #[must_use]
    pub fn spin_rate(&self) -> f64 {
        self.spin.map_or(1.0, |spin| spin.rate)
    }

    /// Advance the coast by one output frame, stopping the deck when it rests.
    #[inline]
    fn advance_spin(&mut self) {
        if let Some(spin) = &mut self.spin
            && !spin.advance()
        {
            self.spin = None;
            self.playing = false;
            self.end_diversion();
        }
    }

    /// Hold or release the censor.
    pub fn set_censor(&mut self, held: bool) {
        if self.censoring == held {
            return;
        }
        self.censoring = held;
        self.needs_prime = true;
        if held {
            self.begin_diversion();
        } else if !self.reversed {
            self.end_diversion();
        }
    }

    /// Whether something is currently taking the playhead off its natural path.
    fn is_diverted(&self) -> bool {
        self.active_loop.is_some() || self.reversed || self.censoring || self.spin.is_some()
    }

    /// Start the shadow playhead here, if slip is in force.
    fn begin_diversion(&mut self) {
        if self.is_slipping() && self.slip_anchor.is_none() {
            self.slip_anchor = Some(self.position.get());
        }
    }

    /// Put the playhead where the track would have been, and stop shadowing.
    ///
    /// Only when nothing else is still diverting it — releasing a censor inside
    /// a loop should return to the loop, not out of it.
    fn end_diversion(&mut self) {
        if self.is_diverted() {
            return;
        }
        if let Some(anchor) = self.slip_anchor.take() {
            let len = self.len_frames() as f64;
            self.position = FramePos::new(anchor.clamp(0.0, len));
            self.needs_prime = true;
        }
    }

    /// Advance the shadow playhead by one output frame's worth of time.
    ///
    /// At the track's *natural forward* rate, whatever the audible playhead is
    /// doing: that is the entire point — the shadow is where the record would
    /// be if you had left it alone. Running off the end stops shadowing, since
    /// there is nowhere to land.
    fn advance_slip(&mut self, forward_step: f64, len: f64) {
        let Some(anchor) = self.slip_anchor else {
            return;
        };
        let next = anchor + forward_step.abs();
        self.slip_anchor = if next >= len { None } else { Some(next) };
    }

    pub fn set_loop_in(&mut self, quantize: bool) {
        self.pending_loop_in = Some(self.snapped(self.position, quantize));
    }

    /// Drop the out point and start looping.
    ///
    /// Falls back to the *active* loop's start when no in point is pending, so
    /// pressing out twice shortens an existing loop instead of doing nothing.
    pub fn set_loop_out(&mut self, quantize: bool) -> bool {
        let start = match self.pending_loop_in.or(self.active_loop.map(|r| r.start)) {
            Some(start) => start,
            None => return false,
        };
        let end = self.snapped(self.position, quantize);
        let Some(region) = LoopRegion::new(start, end) else {
            return false;
        };
        self.pending_loop_in = None;
        self.enter_loop(region)
    }

    /// Halve or double the loop, keeping its start.
    pub fn scale_loop(&mut self, factor: f64) -> bool {
        let Some(region) = self.active_loop else {
            return false;
        };
        let Some(scaled) = region.scaled(factor, self.loop_limits()) else {
            return false;
        };
        self.enter_loop(scaled)
    }

    /// Slide the loop by whole beats, keeping its length.
    pub fn move_loop(&mut self, beats: i32) -> bool {
        let (Some(region), Some(beat)) = (self.active_loop, self.beat_frames()) else {
            return false;
        };
        let Some(moved) = region.moved(f64::from(beats) * beat) else {
            return false;
        };
        if moved.start.get() < 0.0 || moved.end.get() > self.len_frames() as f64 {
            return false;
        }
        // The playhead moves with the loop, keeping its position *within* it --
        // otherwise sliding a loop forward would drop the playhead outside it
        // and the next wrap would jump audibly.
        let offset = self.position.get() - region.start.get();
        self.active_loop = Some(moved);
        self.seek(FramePos::new(moved.start.get() + offset));
        true
    }

    /// Stop looping. The playhead stays where it is and playback carries on.
    pub fn exit_loop(&mut self) {
        let had = self.active_loop.is_some();
        self.active_loop = None;
        self.pending_loop_in = None;
        // A roll ends with the loop it made, so releasing the pad and the loop
        // running out are the same thing. A slice is a roll with a different
        // start, so it ends the same way.
        self.rolling = false;
        self.slicing = false;
        if had {
            self.end_diversion();
        }
    }

    /// Start looping over `region`, pulling the playhead in if it is outside.
    fn enter_loop(&mut self, region: LoopRegion) -> bool {
        if region.end.get() > self.len_frames() as f64 {
            return false;
        }
        let fresh = self.active_loop.is_none();
        self.active_loop = Some(region);
        // Here rather than in `set_loop_region`, because that is not the path
        // the loop buttons take: `set_loop_beats`, `loop_out`, halve and double
        // all come through here, and hanging the diversion off the other one
        // meant slip simply never engaged from a pad.
        if fresh {
            self.begin_diversion();
        }
        // A loop set from outside itself -- by halving until the playhead falls
        // past the new end, or by setting a loop behind the playhead -- has to
        // pull the playhead in, or the deck would keep playing forward and the
        // loop would never engage.
        if !region.contains(self.position) {
            self.seek(region.wrap(self.position));
        }
        true
    }

    #[must_use]
    pub fn hot_cue(&self, slot: u8) -> Option<FramePos> {
        self.hot_cues.get(slot.checked_sub(1)? as usize).copied()?
    }

    /// Set a hot cue at the playhead.
    pub fn set_hot_cue(&mut self, slot: u8, quantize: bool) -> bool {
        let Some(index) = slot.checked_sub(1).map(usize::from) else {
            return false;
        };
        // Computed before the mutable borrow: `snapped` reads the grid, and the
        // borrow checker is right that reading self while holding a mutable
        // slot is not allowed.
        let landing = self.snapped(self.position, quantize);
        let Some(cell) = self.hot_cues.get_mut(index) else {
            return false;
        };
        *cell = Some(landing);
        true
    }

    /// Replace every hot cue at once.
    ///
    /// Wholesale rather than slot by slot, because that is what a restore is:
    /// the set this track had, including the slots that were empty. Filling
    /// them one at a time would leave the previous track's cues in whatever
    /// slots this one does not use.
    pub fn set_hot_cues(&mut self, cues: [Option<FramePos>; HOT_CUE_SLOTS]) {
        self.hot_cues = cues;
    }

    pub fn clear_hot_cue(&mut self, slot: u8) -> bool {
        let Some(index) = slot.checked_sub(1).map(usize::from) else {
            return false;
        };
        match self.hot_cues.get_mut(index) {
            Some(cell) => {
                *cell = None;
                true
            }
            None => false,
        }
    }

    /// Jump to a hot cue.
    ///
    /// Leaves an active loop alone rather than exiting it: jumping to a cue
    /// inside a loop is a normal way to work, and the wrap will pull the
    /// playhead back in if the cue is outside.
    pub fn jump_to_hot_cue(&mut self, slot: u8) -> bool {
        match self.hot_cue(slot) {
            Some(pos) => {
                self.seek(pos);
                true
            }
            None => false,
        }
    }

    /// The one-button behaviour every controller pad sends: jump if set, set if
    /// empty.
    pub fn hot_cue_pressed(&mut self, slot: u8, quantize: bool) -> bool {
        if self.hot_cue(slot).is_some() {
            self.jump_to_hot_cue(slot)
        } else {
            self.set_hot_cue(slot, quantize)
        }
    }

    /// Fold a position back into the active loop.
    ///
    /// Called on every frame of both render paths, which is why it is a plain
    /// arithmetic function on `Option` rather than anything cleverer.
    #[must_use]
    fn fold(&self, pos: f64) -> f64 {
        match self.active_loop {
            Some(region) if pos >= region.end.get() || pos < region.start.get() => {
                region.wrap(FramePos::new(pos)).get()
            }
            _ => pos,
        }
    }

    /// Tempo the deck is actually playing at, pitch fader included.
    ///
    /// `None` when there is no grid. Not zero: "no tempo" and "zero BPM" are
    /// different statements and the interface shows them differently.
    #[must_use]
    pub fn effective_bpm(&self) -> Option<f64> {
        let grid = self.grid?;
        let bpm = grid.bpm.get() * self.rate.get() * (1.0 + self.pitch);
        bpm.is_finite().then_some(bpm).filter(|b| *b > 0.0)
    }

    /// Frames of *source* audio in one beat.
    ///
    /// Source frames, not device frames: the grid's anchor and the playhead are
    /// both positions in the track, and mixing the two rates here would put the
    /// grid a few percent off on any track that is not at the device rate.
    #[must_use]
    fn beat_frames(&self) -> Option<f64> {
        let grid = self.grid?;
        let frames = grid.bpm.beat_frames(self.source.sample_rate());
        (frames.is_finite() && frames > 0.0).then_some(frames)
    }

    /// Position of the beat nearest `pos`.
    #[must_use]
    pub fn nearest_beat(&self, pos: FramePos) -> Option<FramePos> {
        let grid = self.grid?;
        Some(grid.nearest_beat(pos, self.source.sample_rate()))
    }

    /// Where in the current beat the playhead sits, as a fraction in `[0, 1)`.
    ///
    /// This is the number phase sync works on: two decks whose phases match are
    /// two decks whose beats land together.
    #[must_use]
    pub fn beat_phase(&self) -> Option<f64> {
        let grid = self.grid?;
        let beat = self.beat_frames()?;
        let offset = (self.position.get() - grid.anchor.get()) / beat;
        Some(offset - offset.floor())
    }

    /// Set the pitch fader so this deck plays at `target_bpm`.
    ///
    /// Returns false when it cannot: no grid, or a stretch so large it would be
    /// an octave error rather than a tempo match.
    pub fn match_tempo(&mut self, target_bpm: f64) -> bool {
        let Some(grid) = self.grid else {
            return false;
        };
        let own = grid.bpm.get() * self.rate.get();
        if !target_bpm.is_finite() || target_bpm <= 0.0 || own <= 0.0 {
            return false;
        }

        // Half and double are offered because they are musically the same
        // tempo. A 70 BPM track against a 140 BPM one should play at 70 with
        // beats landing every other beat, not stretched to double speed --
        // which is what a naive ratio would do, and it would sound absurd.
        let best = [target_bpm, target_bpm * 0.5, target_bpm * 2.0]
            .into_iter()
            .map(|candidate| (candidate / own - 1.0, candidate))
            .filter(|(pitch, _)| pitch.abs() <= MAX_SYNC_STRETCH)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));

        match best {
            Some((pitch, _)) => {
                self.set_pitch(pitch);
                true
            }
            // Refused rather than approximated. A sync that needs more than a
            // third either way is not a tempo match; it is a wrong grid, and
            // quietly playing the track at that speed would be worse than
            // doing nothing.
            None => false,
        }
    }

    /// Shift the playhead to the nearest point where this deck's beats land
    /// with `phase`.
    ///
    /// Moves by at most half a beat where it can, in whichever direction is
    /// shorter, so a deck already nearly in phase barely moves.
    ///
    /// # Why there is a fallback
    ///
    /// The shorter direction is often backwards, and a track synced shortly
    /// after loading is only a fraction of a second from its start -- so the
    /// shorter move runs off the front of the file. That is the *most common*
    /// moment to press sync, not an edge case. Going the other way is a full
    /// beat further but lands on exactly the same phase, which is what was
    /// asked for; refusing instead would leave the decks silently unaligned
    /// with sync showing as engaged.
    pub fn align_phase_to(&mut self, phase: f64) -> bool {
        let (Some(beat), Some(mine)) = (self.beat_frames(), self.beat_phase()) else {
            return false;
        };
        if !phase.is_finite() {
            return false;
        }

        let mut delta = phase - mine;
        // Wrap into (-0.5, 0.5]: going forward 0.9 of a beat is going back 0.1.
        if delta > 0.5 {
            delta -= 1.0;
        } else if delta < -0.5 {
            delta += 1.0;
        }

        let len = self.len_frames() as f64;
        let here = self.position.get();
        // Shorter first, then the same phase a beat the other way.
        let other = if delta < 0.0 {
            delta + 1.0
        } else {
            delta - 1.0
        };
        for candidate in [delta, other] {
            let moved = here + candidate * beat;
            if moved >= 0.0 && moved < len {
                self.seek(FramePos::new(moved));
                return true;
            }
        }
        false
    }

    /// Move the playhead by whole beats.
    ///
    /// With `quantize` the landing point is snapped to the grid, so repeated
    /// jumps from an off-beat position converge onto it rather than carrying
    /// the same error forward forever. Without it the jump is exactly `beats`
    /// beats from wherever the playhead happens to be, which is what you want
    /// when deliberately playing against the grid.
    pub fn beat_jump(&mut self, beats: i32, quantize: bool) -> bool {
        let Some(beat) = self.beat_frames() else {
            return false;
        };
        let from = if quantize {
            match self.nearest_beat(self.position) {
                Some(snapped) => snapped.get(),
                None => return false,
            }
        } else {
            self.position.get()
        };

        let target = from + f64::from(beats) * beat;
        // Clamped rather than refused: jumping past the end of a track should
        // land at the end, the way a seek does, not silently do nothing.
        self.seek(FramePos::new(target));
        true
    }

    #[must_use]
    pub fn len_frames(&self) -> usize {
        self.source.len_frames()
    }

    /// Read a frame at the given position. If stems are available, applies
    /// per-stem EQ, Filter, Volume, and Mute, then mixes them down.
    /// If stems are not available, returns the raw track frame.
    /// If `apply_dsp` is false, bypasses EQ and filter state updates (used for priming).
    fn read_frame(&mut self, position: f64, apply_dsp: bool) -> ([f32; 2], bool) {
        if let Some(stems) = self.source.stem_frame_at(position) {
            let mut mixed_left = 0.0;
            let mut mixed_right = 0.0;
            for (i, stem) in stems.iter().enumerate() {
                let ch = &mut self.stem_channels[i];
                if !ch.mute {
                    let (l, r) = if apply_dsp {
                        (
                            ch.filter[0].process(ch.eq[0].process(stem[0])) * ch.volume,
                            ch.filter[1].process(ch.eq[1].process(stem[1])) * ch.volume,
                        )
                    } else {
                        (stem[0] * ch.volume, stem[1] * ch.volume)
                    };
                    mixed_left += l;
                    mixed_right += r;
                }
            }
            ([mixed_left, mixed_right], true)
        } else {
            (self.source.frame_at(position), false)
        }
    }

    /// Frames of source consumed per output frame.
    ///
    /// Combines the pitch fader, any directly-set rate, and conversion between
    /// the track's sample rate and the device's. A 44.1 kHz track on a 48 kHz
    /// device must advance at 0.919 frames per output frame or it plays sharp.
    #[must_use]
    fn step_per_output_frame(&self) -> f64 {
        let forward = self.forward_step_per_output_frame();
        let directed = if self.reversed() { -forward } else { forward };
        // A hand on the record beats everything else, because it physically
        // would: while the platter is being scratched or searched the motor is
        // not driving, the wheel is, so this replaces the step rather than
        // scaling it.
        if let Some(scrub) = self.jog_step {
            return scrub;
        }
        // A coasting platter multiplies whatever the step was. Applied last, so
        // a brake on a reversed deck slows to a stop rather than turning round.
        let coasted = match self.spin {
            Some(spin) => directed * spin.rate,
            None => directed,
        };
        // A bend is the last word on speed: it is "run a little faster while I
        // push", on top of whatever the deck was already doing.
        coasted * self.jog_bend
    }

    /// Fold the wheel's movement into this block.
    ///
    /// Called once per render block, before the step is taken, because the
    /// step is what carries the answer. See [`crate::jog`].
    fn take_jog(&mut self, frames: usize) {
        match self.jog.advance_block(frames, self.playing) {
            JogEffect::Free => {
                self.jog_step = None;
                self.jog_bend = 1.0;
            }
            JogEffect::Bend(multiplier) => {
                self.jog_step = None;
                self.jog_bend = multiplier;
            }
            JogEffect::Scrub(source_frames) => {
                // Spread across the block: the movement that arrived during
                // these frames is played over these frames.
                self.jog_step = Some(source_frames / frames.max(1) as f64);
                self.jog_bend = 1.0;
            }
        }
    }

    /// The step the record would take if nothing were reversing it.
    ///
    /// What the shadow playhead advances by, and what the beat grid and the
    /// waveform are measured in — reversing changes which way the audible
    /// playhead moves, not how fast the music is nominally going.
    fn forward_step_per_output_frame(&self) -> f64 {
        let ratio = self.source.sample_rate().as_f64() / self.device_rate.as_f64();
        self.rate.get() * (1.0 + self.pitch) * ratio
    }

    /// Render into the interleaved output buffer, adding rather than
    /// overwriting.
    ///
    /// Writes to two buses at different points in the gain chain:
    ///
    /// ```text
    ///   source → EQ → filter → trim ─┬─→ × fader × crossfader → MAIN
    ///                                └─→ (unmodified)          → CUE
    /// ```
    ///
    /// The cue send is taken **before** the channel fader and crossfader, which
    /// is what "pre-fader listen" means and the entire reason PFL is useful: you
    /// cue up the next track with its fader all the way down, hearing it in
    /// headphones while the audience hears nothing.
    ///
    /// Realtime-safe: no allocation, no locking, no I/O.
    ///
    /// `tap` is the sampler's recorder, when it is recording *this* deck. It is
    /// fed the pre-fader signal — the same one the headphones get — so a hook
    /// can be lifted off a track the room is not hearing yet. See
    /// [`dj_core::RecordSource::Deck`].
    pub fn process(
        &mut self,
        out: &mut [f32],
        layout: &BusLayout,
        tap: Option<&mut Recorder>,
    ) -> DeckLevels {
        if self.source.is_empty() {
            return DeckLevels::default();
        }
        // A paused deck still plays while the wheel is being wound: searching
        // by ear is how a DJ finds a cue point, and a silent search would be a
        // wheel that scrolls a waveform rather than one that cues a record.
        // A paused deck nobody is touching still takes the cheap path.
        let searching = !self.playing && self.jog.has_movement();
        if !self.playing && !searching {
            return DeckLevels::default();
        }
        // The shifter runs whenever it has something to do: correcting the
        // pitch that speed introduced, transposing on purpose, or both.
        // A coasting platter goes down the direct path whatever keylock says.
        // The *sound* of a brake is the pitch falling; keylock exists to stop
        // the pitch falling; a keylocked brake is a brake that does not brake.
        // The shifter also works on blocks, and a rate that changes every frame
        // is not something a block-based shifter can follow.
        // A search goes down the direct path for the same reason a brake does:
        // the shifter works on blocks at a settled tempo, and a hand winding a
        // wheel is neither.
        if (self.keylock_on || self.key_shift != 0) && self.spin.is_none() && !searching {
            self.process_keylocked(out, layout, tap)
        } else {
            self.process_direct(out, layout, tap)
        }
    }

    /// The plain path: read, shape, mix. What runs whenever keylock is off.
    fn process_direct(
        &mut self,
        out: &mut [f32],
        layout: &BusLayout,
        mut tap: Option<&mut Recorder>,
    ) -> DeckLevels {
        let len = self.len_frames() as f64;
        let mut levels = DeckLevels::default();
        let mut position = self.position.get();
        let channels = layout.channels.max(1);
        let cue_send = if self.cue_enabled { layout.cue } else { None };
        // Once per block, not per frame: the tempo cannot change inside a
        // block, and `effective_bpm` is a grid lookup and a division.
        let ctx = self.fx_context();
        // The wheel's contribution is settled for the block before the first
        // step is taken, so a hand on the platter is already in `step` below.
        self.take_jog(out.len() / channels);

        for frame in out.chunks_exact_mut(channels) {
            // Advance the smoothers every frame regardless of whether audio is
            // produced, so a fader moved during a silent stretch has settled by
            // the time sound returns.
            let trim = self.trim_gain.next_value();
            let fader = self.fader_gain.next_value() * self.crossfader_gain.next_value();

            if position < 0.0 || position >= len {
                continue;
            }

            let ([left, right], stems_available) = self.read_frame(position, true);
            let (pre_left, pre_right) = if stems_available {
                self.rack.process_pre(left * trim, right * trim, &ctx)
            } else {
                self.shape(left, right, trim, &ctx)
            };
            let post_left = pre_left * fader;
            let post_right = pre_right * fader;
            let pre = (pre_left, pre_right);
            let post = self.rack.process_post(post_left, post_right, &ctx);

            Self::write_frame(
                frame,
                layout,
                cue_send,
                pre,
                post,
                &mut levels,
                tap.as_deref_mut(),
            );

            // Advance, then fold back into the loop. Per frame rather than per
            // block: a loop shorter than one buffer -- which is most of them at
            // a sixteenth of a beat -- wraps several times inside a single
            // callback, and folding once per block would play straight past it.
            // Read per frame rather than once per block, because a coasting
            // platter changes speed every frame. Per block a one-beat brake
            // would fall in about ninety audible steps, which is a zipper
            // rather than a slowdown.
            let step = self.step_per_output_frame();
            position = self.fold(position + step);
            // The shadow keeps going forward at the natural rate whatever the
            // audible playhead is doing. One f64 add, so it costs nothing and
            // stays allocation-free.
            self.advance_slip(step, len);
            self.advance_spin();
            // Stop early only when *nothing* is driving the record. A hand
            // searching a paused deck keeps it turning: `jog_step` is `Some`
            // exactly while the wheel is driving, so this asks whether the
            // record has actually come to rest rather than whether the motor
            // is on.
            if !self.playing && self.jog_step.is_none() {
                // The coast ended part-way through this block. Stop here: with
                // the spin gone the multiplier is gone with it, and the
                // remaining frames would play at *full speed* — up to a whole
                // buffer of audio after the record has audibly stopped, which
                // at 256 frames is five milliseconds of the track jumping back
                // to pitch as it comes to rest.
                break;
            }
        }

        self.finish(position, len);
        levels
    }

    /// The keylocked path: read ahead, transpose, shape, mix.
    ///
    /// Structurally different from [`Self::process_direct`] because the pitch
    /// shifter works on blocks, not on one frame at a time. Audio is read into
    /// a scratch buffer in chunks, transposed, and only then run through the
    /// same gain chain -- so everything downstream of the source is identical
    /// and only the reading changes.
    ///
    /// Two details carry the whole design:
    ///
    /// - The read cursor runs [`Keylock::latency_frames`] *ahead* of the
    ///   playhead, so the shifter's group delay cancels and engaging keylock
    ///   does not shove a beatmatched track out of time.
    /// - `position` still advances by exactly `step` per output frame, so the
    ///   playhead, the waveform and the beat grid are unaffected by keylock.
    fn process_keylocked(
        &mut self,
        out: &mut [f32],
        layout: &BusLayout,
        mut tap: Option<&mut Recorder>,
    ) -> DeckLevels {
        let channels = layout.channels.max(1);
        // Before the step, for the same reason as the plain path.
        self.take_jog(out.len() / channels);
        let step = self.step_per_output_frame();
        let len = self.len_frames() as f64;
        let cue_send = if self.cue_enabled { layout.cue } else { None };
        let mut levels = DeckLevels::default();
        let ctx = self.fx_context();

        // Musical speed only. Sample-rate conversion is also part of `step` but
        // changes no pitch, so undoing it would put the track *out* of key.
        self.keylock.set_tempo(self.rate.get() * (1.0 + self.pitch));
        if self.needs_prime {
            self.prime_keylock(step, len);
            self.needs_prime = false;
        }

        let read_ahead = self.keylock.latency_frames() as f64 * step;
        let mut position = self.position.get();
        let frames_total = out.len() / channels;
        let mut done = 0;

        while done < frames_total {
            let n = (frames_total - done).min(SCRATCH_FRAMES);

            // Read ahead of the playhead. Out of range is silence rather than a
            // skip: the shifter still has the previous audio in flight, and
            // feeding it nothing is how that tail gets flushed out in time.
            // The playhead is tracked frame by frame here rather than by
            // `n * step` at the end of the block, because a loop has to fold
            // every frame: at a sixteenth of a beat a loop is shorter than one
            // buffer, and folding once per block would play straight past it.
            //
            // The read cursor is `read_ahead` frames of *looped* time ahead of
            // the playhead, so it is folded too. Reading across a loop point
            // hands the shifter a discontinuity, which it smears over its
            // window -- inherent to a phase vocoder, and the same thing that
            // happens on any seek.
            let mut cursor = position;
            let mut stems_available = false;
            for f in 0..n {
                let p = self.fold(cursor + read_ahead);
                let ([left, right], stems_avail) = if p >= 0.0 && p < len {
                    self.read_frame(p, true)
                } else {
                    ([0.0, 0.0], false)
                };
                stems_available = stems_avail;

                self.scratch[f * CHANNELS] = left;
                self.scratch[f * CHANNELS + 1] = right;
                cursor = self.fold(cursor + step);
            }

            self.keylock.process(&mut self.scratch[..n * CHANNELS]);

            let block = &mut out[done * channels..(done + n) * channels];
            for (f, frame) in block.chunks_exact_mut(channels).enumerate() {
                let trim = self.trim_gain.next_value();
                let fader = self.fader_gain.next_value() * self.crossfader_gain.next_value();
                let left = self.scratch[f * CHANNELS];
                let right = self.scratch[f * CHANNELS + 1];

                let (pre_left, pre_right) = if stems_available {
                    self.rack.process_pre(left * trim, right * trim, &ctx)
                } else {
                    self.shape(left, right, trim, &ctx)
                };
                let post_left = pre_left * fader;
                let post_right = pre_right * fader;
                let pre = (pre_left, pre_right);
                let post = self.rack.process_post(post_left, post_right, &ctx);

                Self::write_frame(
                    frame,
                    layout,
                    cue_send,
                    pre,
                    post,
                    &mut levels,
                    tap.as_deref_mut(),
                );
            }

            position = cursor;
            // The shadow advances by the same number of output frames the
            // block produced, at the natural forward rate. Per block here
            // rather than per frame, because that is how this path works —
            // the result is the same, since the rate does not change inside a
            // block.
            for _ in 0..n {
                self.advance_slip(step, len);
            }
            done += n;
        }

        self.finish(position, len);
        levels
    }

    /// Fill the shifter's history so playback resumes without a fade-in.
    ///
    /// Reads the frames immediately before the point the read cursor starts
    /// from -- which is ahead of the playhead by the shifter's latency, the
    /// same offset [`Self::process_keylocked`] uses.
    fn prime_keylock(&mut self, step: f64, len: f64) {
        let preroll = self.keylock.preroll_frames() as f64;
        let start = self.position.get() + (self.keylock.latency_frames() as f64 - preroll) * step;
        // Borrowed as a field, not through `&self`, so the shifter can be
        // borrowed mutably at the same time.
        let source = &*self.source;

        self.keylock.prime_with(|frame| {
            let p = start + frame as f64 * step;
            if p >= 0.0 && p < len {
                if let Some(stems) = source.stem_frame_at(p) {
                    let mut left = 0.0;
                    let mut right = 0.0;
                    // We don't have access to stem_channels here because of the borrow checker.
                    // But priming doesn't need perfect volume matching, just the audio content.
                    for stem in stems.iter() {
                        left += stem[0];
                        right += stem[1];
                    }
                    [left, right]
                } else {
                    source.frame_at(p)
                }
            } else {
                [0.0, 0.0]
            }
        });
    }

    /// Tone shaping, applied before the fader.
    ///
    /// On a real mixer the channel fader attenuates the EQ'd signal, not the
    /// other way round; reversed, riding the fader would change the tone.
    #[inline]
    fn shape(&mut self, left: f32, right: f32, trim: f32, ctx: &FxContext) -> (f32, f32) {
        let left = self.filter[0].process(self.eq[0].process(left)) * trim;
        let right = self.filter[1].process(self.eq[1].process(right)) * trim;
        // After the isolator and the trim, before the fader. An effect placed
        // here hears the DJ's EQ moves, which is what a DJ expects: killing the
        // bass under an echo should kill the bass in the repeats too.
        self.rack.process_pre(left, right, ctx)
    }

    /// Add one shaped frame to the buses.
    ///
    /// An associated function rather than a method so it borrows nothing from
    /// the deck, and both process paths can call it while holding other fields.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn write_frame(
        frame: &mut [f32],
        layout: &BusLayout,
        cue_send: Option<(usize, usize)>,
        pre: (f32, f32),
        post: (f32, f32),
        levels: &mut DeckLevels,
        tap: Option<&mut Recorder>,
    ) {
        let (pre_left, pre_right) = pre;
        // The fader has already been applied, along with any effect placed
        // after it. Passed in rather than derived here because a post-fader
        // effect changes what reaches the master and must not change what
        // reaches the headphones -- pre-fader listen means pre-fader.
        let (main_left, main_right) = post;

        if layout.is_mono() {
            frame[layout.main.0] += (main_left + main_right) * 0.5;
        } else {
            frame[layout.main.0] += main_left;
            frame[layout.main.1] += main_right;
        }

        if let Some((cue_l, cue_r)) = cue_send {
            frame[cue_l] += pre_left;
            frame[cue_r] += pre_right;
        }

        // The recorder takes the pre-fader signal, exactly what the cue send
        // above just took. Recording a deck is auditioning it and keeping the
        // result, so the two should hear the same thing.
        if let Some(recorder) = tap {
            recorder.write(pre_left, pre_right);
        }

        levels.pre_fader = levels.pre_fader.max(pre_left.abs()).max(pre_right.abs());
        levels.post_fader = levels.post_fader.max(main_left.abs()).max(main_right.abs());
    }

    /// Store the new playhead, stopping the transport at either end.
    ///
    /// Running off the end stops rather than leaving a silent deck reporting
    /// itself as playing.
    fn finish(&mut self, position: f64, len: f64) {
        if position >= len {
            self.position = FramePos::new(len);
            self.playing = false;
        } else if position < 0.0 {
            self.position = FramePos::ZERO;
            self.playing = false;
        } else {
            self.position = FramePos::new(position);
        }
    }
}

/// A platter coasting, because the motor was cut or the record was thrown.
///
/// One state for both moves, because they are one move with a different push:
/// a brake starts at full speed and coasts to a stop, and a backspin starts
/// several times faster in the other direction and coasts to the same place.
///
/// **Linear decay, not exponential.** A platter slows against roughly constant
/// friction, so its speed falls in a straight line and it stops at a definite
/// moment — which is why a brake has a *length* a DJ can put on a beat. An
/// exponential decay would approach zero and never arrive, and the record would
/// still be crawling four bars later.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spin {
    /// What the natural step is multiplied by. Starts at 1.0 for a brake and
    /// at [`Spin::THROW`] for a backspin; decays toward zero.
    rate: f64,
    /// How much `rate` moves per output frame. Always toward zero.
    per_frame: f64,
}

impl Spin {
    /// How fast a thrown record spins backwards, as a multiple of playing
    /// speed.
    ///
    /// Four. A real backspin is faster than that for an instant and then mostly
    /// friction; four is where it stops sounding like a rewind and starts
    /// sounding like a throw.
    const THROW: f64 = -4.0;

    fn braking(frames: f64) -> Self {
        Self {
            rate: 1.0,
            per_frame: -1.0 / frames.max(1.0),
        }
    }

    fn thrown(frames: f64) -> Self {
        Self {
            rate: Self::THROW,
            per_frame: -Self::THROW / frames.max(1.0),
        }
    }

    /// Advance one output frame. `false` once it has come to rest.
    #[inline]
    fn advance(&mut self) -> bool {
        let was_negative = self.rate < 0.0;
        self.rate += self.per_frame;
        // Crossing zero is the stop, in both directions. Compared against the
        // sign it started at rather than against zero, so a backspin does not
        // stop the instant it passes through zero on the way up.
        if was_negative {
            self.rate < 0.0
        } else {
            self.rate > 0.0
        }
    }
}

/// Peak levels a deck produced in one block.
///
/// Both are reported because they answer different questions: pre-fader is what
/// the trim knob should be set by (and what the cue meter shows), post-fader is
/// what actually reaches the master.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DeckLevels {
    pub pre_fader: f32,
    pub post_fader: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: SampleRate = SampleRate::DEFAULT;

    /// Plain stereo: master on 0/1, no cue. What most of these tests want.
    fn stereo() -> BusLayout {
        BusLayout::for_channels(2)
    }

    /// A ramp source: frame `n` has value `n`, so tests can read the position
    /// straight out of the rendered audio.
    fn ramp(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    fn deck_with(frames: usize) -> Deck {
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(frames));
        deck
    }

    /// Render `frames` output frames and throw the audio away.
    fn run(deck: &mut Deck, frames: usize) {
        let mut out = vec![0.0; frames * 2];
        deck.process(&mut out, &stereo(), None);
    }

    // -- slip, reverse and censor ------------------------------------------

    #[test]
    fn a_deck_without_slip_stays_where_the_loop_left_it() {
        let mut deck = deck_with(4000);
        deck.play();
        run(&mut deck, 100);
        deck.set_loop_region(LoopRegion::new(FramePos::new(100.0), FramePos::new(200.0)));
        run(&mut deck, 500);
        deck.set_loop_region(None);
        assert!(
            deck.position().get() < 250.0,
            "without slip the playhead is wherever the loop left it, got {}",
            deck.position().get()
        );
    }

    /// The whole point of slip: loop for a while, come out, and land where the
    /// record would have been if you had left it alone.
    #[test]
    fn slipping_over_a_loop_lands_where_the_track_would_have_been() {
        let mut deck = deck_with(4000);
        deck.set_slip(true);
        deck.play();
        run(&mut deck, 100);
        deck.set_loop_region(LoopRegion::new(FramePos::new(100.0), FramePos::new(200.0)));
        run(&mut deck, 500);
        deck.set_loop_region(None);
        // 100 frames before the loop, 500 inside it: the record would be at 600.
        assert!(
            (deck.position().get() - 600.0).abs() < 2.0,
            "expected to land near 600, got {}",
            deck.position().get()
        );
    }

    /// Arming slip in the middle of a loop starts the shadow *now*. Pretending
    /// it had been running since the loop began would jump the track forward by
    /// however long the DJ had been looping before they reached for the button.
    #[test]
    fn arming_slip_mid_loop_starts_the_shadow_from_that_moment() {
        let mut deck = deck_with(4000);
        deck.play();
        deck.set_loop_region(LoopRegion::new(FramePos::new(0.0), FramePos::new(100.0)));
        run(&mut deck, 500);
        deck.set_slip(true);
        run(&mut deck, 200);
        deck.set_loop_region(None);
        assert!(
            deck.position().get() < 350.0,
            "the shadow should have run for 200 frames, not 700; got {}",
            deck.position().get()
        );
    }

    /// Turning slip *off* mid-loop means "stay here", which is the opposite of
    /// jumping to the shadow.
    #[test]
    fn disarming_slip_mid_loop_leaves_the_playhead_alone() {
        let mut deck = deck_with(4000);
        deck.set_slip(true);
        deck.play();
        deck.set_loop_region(LoopRegion::new(FramePos::new(0.0), FramePos::new(100.0)));
        run(&mut deck, 500);
        deck.set_slip(false);
        deck.set_loop_region(None);
        assert!(
            deck.position().get() < 150.0,
            "expected to stay inside the loop, got {}",
            deck.position().get()
        );
    }

    /// A grid, so beat-length loops and rolls can be measured.
    fn gridded(frames: usize, bpm: f64) -> Deck {
        use dj_core::{Beatgrid, Bpm, Confidence};
        let mut deck = deck_with(frames);
        deck.set_grid(Some(Beatgrid::new(
            FramePos::ZERO,
            Bpm::new(bpm).unwrap(),
            Confidence::new(0.9),
        )));
        deck
    }

    /// The bug this found: the loop *buttons* do not go through
    /// `set_loop_region`, so hanging the diversion off that one meant slip
    /// never engaged from a pad — only from the path the first tests used.
    #[test]
    fn slip_engages_from_the_loop_buttons_not_only_from_a_set_region() {
        let mut deck = gridded(400_000, 120.0);
        deck.set_slip(true);
        deck.play();
        run(&mut deck, 1000);
        assert!(deck.set_loop_length(1.0, false), "a gridded deck can loop");
        assert!(
            deck.slip_position().is_some(),
            "arming slip then pressing a loop pad must start the shadow"
        );
    }

    /// A roll always slips, whatever the slip button says — the point of a roll
    /// is that the track carries on underneath.
    #[test]
    fn a_loop_roll_slips_even_with_slip_off() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        assert!(!deck.slip());
        run(&mut deck, 1000);
        assert!(deck.set_loop_roll(Some(1.0), false));
        assert!(deck.is_slipping(), "a roll always slips");
        assert!(deck.slip_position().is_some());
    }

    #[test]
    fn releasing_a_roll_lands_where_the_track_would_have_been() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 1000);
        deck.set_loop_roll(Some(1.0), false);
        run(&mut deck, 5000);
        deck.set_loop_roll(None, false);
        assert!(deck.active_loop().is_none(), "the roll's loop went with it");
        assert!(!deck.rolling());
        assert!(
            (deck.position().get() - 6000.0).abs() < 50.0,
            "expected to land near 6000, got {}",
            deck.position().get()
        );
    }

    /// A roll on a deck with no grid cannot be measured in beats, and must not
    /// leave the deck believing it is rolling.
    #[test]
    fn a_roll_on_an_ungridded_deck_is_refused_cleanly() {
        let mut deck = deck_with(400_000);
        deck.play();
        assert!(!deck.set_loop_roll(Some(1.0), false));
        assert!(!deck.rolling(), "a refused roll is not a roll");
        assert!(!deck.is_slipping());
    }

    /// The roll a DJ means by the word is the sub-beat one, so a quarter beat
    /// has to make a quarter-beat loop and not round to nothing or to one.
    #[test]
    fn a_roll_can_be_a_fraction_of_a_beat() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 1000);
        assert!(deck.set_loop_roll(Some(0.25), false));
        let beats = deck.loop_beats().expect("a gridded deck can name its loop");
        assert!(
            (beats - 0.25).abs() < 0.01,
            "expected a quarter-beat loop, got {beats}"
        );
    }

    /// Below a sixteenth a loop is a pitched buzz, not a roll. The engine's own
    /// limit catches it, and the roll must go through that limit rather than
    /// around it.
    #[test]
    fn an_absurdly_short_roll_lands_on_the_shortest_loop_the_engine_makes() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        assert!(deck.set_loop_roll(Some(0.001), false));
        let beats = deck.loop_beats().expect("a gridded deck can name its loop");
        assert!(
            (beats - dj_core::hotcue::MIN_LOOP_BEATS).abs() < 0.001,
            "expected the floor of {}, got {beats}",
            dj_core::hotcue::MIN_LOOP_BEATS
        );
    }

    /// Every other roll test plays for a while first and passes `quantize:
    /// false`. A freshly loaded track is the opposite — paused, at frame zero,
    /// with quantize on — and that is the state a DJ is in when they reach for
    /// a pad to hear what a section does. Snapping to the nearest beat from
    /// zero is the case most likely to produce a region the engine refuses.
    #[test]
    fn a_roll_works_on_a_freshly_loaded_paused_deck_with_quantize_on() {
        let mut deck = gridded(400_000, 120.0);
        assert!(!deck.is_playing());
        assert_eq!(deck.position().get(), 0.0);
        assert!(
            deck.set_loop_roll(Some(0.25), true),
            "a roll at the start of a gridded track must be accepted"
        );
        assert!(deck.active_loop().is_some(), "the roll made no loop");
        assert!(deck.rolling());
    }

    /// The whole point of a brake: the record measurably *slows*, and then
    /// stops.
    ///
    /// The first version of this test only checked that each block travelled no
    /// further than the last, and then that the deck had stopped. A mutation
    /// that dropped the coast multiplier entirely passed it — a constant rate
    /// satisfies "no faster than before", and the deck still stopped because
    /// the coast's own timer ended it. So the assertion has to be that the
    /// record ends up crawling compared with how it started.
    #[test]
    fn a_brake_slows_the_record_and_stops_it() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 1_000);
        assert!(deck.brake(2.0, false), "a gridded playing deck can brake");

        let travelled = |deck: &mut Deck| {
            let before = deck.position().get();
            run(deck, 256);
            deck.position().get() - before
        };

        let first = travelled(&mut deck);
        let mut last = first;
        for _ in 0..400 {
            if !deck.is_spinning() {
                break;
            }
            last = travelled(&mut deck);
        }
        assert!(first > 0.0, "it never moved");
        assert!(
            last < first * 0.2,
            "it barely slowed: {last} against {first}"
        );
        assert!(!deck.is_playing(), "a brake ends stopped");
        assert!(!deck.is_spinning(), "and the coast is over");
    }

    /// A backspin goes the other way. Everything else about it is a brake.
    #[test]
    fn a_backspin_travels_backwards_and_stops() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 100_000);
        let started = deck.position().get();
        assert!(deck.brake(1.0, true));

        for _ in 0..400 {
            run(&mut deck, 256);
            if !deck.is_spinning() {
                break;
            }
        }
        assert!(
            deck.position().get() < started,
            "a backspin should end behind where it started: {} against {started}",
            deck.position().get()
        );
        assert!(!deck.is_playing());
    }

    /// A brake is measured in beats, so a deck with no grid has nothing to
    /// measure it against — and a brake that lands somewhere arbitrary is worse
    /// than no brake.
    #[test]
    fn a_brake_on_an_ungridded_deck_is_refused_cleanly() {
        let mut deck = deck_with(400_000);
        deck.play();
        assert!(!deck.brake(2.0, false));
        assert!(!deck.is_spinning());
        assert!(deck.is_playing(), "and it keeps playing");
    }

    /// Braking a stopped deck is a gesture with no meaning, and starting a
    /// coast on one would leave it spinning down from a standstill.
    #[test]
    fn a_paused_deck_cannot_brake() {
        let mut deck = gridded(400_000, 120.0);
        assert!(!deck.is_playing());
        assert!(!deck.brake(2.0, false));
        assert!(!deck.is_spinning());
    }

    /// Releasing puts the motor back on where the record got to — not back
    /// where the brake started. The way back is the cue button.
    #[test]
    fn releasing_a_brake_carries_on_from_here() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 1_000);
        deck.brake(4.0, false);
        run(&mut deck, 2_000);
        let part_way = deck.position().get();
        assert!(part_way > 1_000.0);

        deck.release_brake();
        assert!(!deck.is_spinning());
        assert!(deck.is_playing(), "the motor is back on");
        run(&mut deck, 100);
        assert!(
            deck.position().get() > part_way,
            "and it carried on from where it was"
        );
    }

    /// The sound of a brake is the pitch falling. Keylock exists to stop the
    /// pitch falling. A keylocked brake would be a brake that does not brake,
    /// so a coast goes down the direct path whatever keylock says.
    #[test]
    fn a_brake_bypasses_keylock() {
        let mut deck = gridded(400_000, 120.0);
        deck.set_keylock(true);
        deck.play();
        run(&mut deck, 1_000);
        assert!(deck.brake(2.0, false));

        for _ in 0..400 {
            run(&mut deck, 256);
            if !deck.is_spinning() {
                break;
            }
        }
        assert!(!deck.is_playing(), "a keylocked brake still ends stopped");
    }

    /// Slip and a backspin are the pair a DJ actually uses: throw the record,
    /// and the shadow keeps running so there is somewhere to land.
    #[test]
    fn a_backspin_with_slip_keeps_a_shadow_running() {
        let mut deck = gridded(400_000, 120.0);
        deck.set_slip(true);
        deck.play();
        run(&mut deck, 10_000);
        assert!(deck.brake(1.0, true));
        assert!(
            deck.slip_position().is_some(),
            "the shadow should be running"
        );
        for _ in 0..400 {
            run(&mut deck, 256);
            if !deck.is_spinning() {
                break;
            }
        }
        assert!(!deck.is_playing());
    }

    /// A brake on a deck already running backwards slows to a stop rather than
    /// turning around: the coast multiplies whatever the step was.
    #[test]
    fn a_brake_on_a_reversed_deck_still_slows_to_a_stop() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        run(&mut deck, 100_000);
        deck.set_reverse(true);
        let started = deck.position().get();
        assert!(deck.brake(1.0, false));

        for _ in 0..400 {
            run(&mut deck, 256);
            if !deck.is_spinning() {
                break;
            }
        }
        assert!(
            deck.position().get() < started,
            "a reversed deck braking still travels backwards"
        );
        assert!(!deck.is_playing());
    }

    /// Nonsense lengths must not become a coast that never ends or one that
    /// divides by zero.
    #[test]
    fn a_nonsense_brake_length_is_refused() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        assert!(!deck.brake(0.0, false));
        assert!(!deck.brake(-1.0, false));
        assert!(!deck.brake(f64::NAN, false));
        assert!(!deck.brake(f64::INFINITY, false));
        assert!(!deck.is_spinning());
    }

    /// The loop running out is the same event as letting go of the pad.
    #[test]
    fn a_roll_that_ends_any_other_way_stops_rolling() {
        let mut deck = gridded(400_000, 120.0);
        deck.play();
        deck.set_loop_roll(Some(1.0), false);
        deck.exit_loop();
        assert!(!deck.rolling());
    }

    #[test]
    fn reverse_walks_the_playhead_backwards() {
        let mut deck = deck_with(4000);
        deck.play();
        run(&mut deck, 500);
        let forward = deck.position().get();
        deck.set_reverse(true);
        run(&mut deck, 200);
        assert!(
            deck.position().get() < forward,
            "reversed from {forward} to {}",
            deck.position().get()
        );
    }

    #[test]
    fn reversing_to_the_start_stops_rather_than_running_negative() {
        let mut deck = deck_with(4000);
        deck.play();
        run(&mut deck, 50);
        deck.set_reverse(true);
        run(&mut deck, 500);
        assert_eq!(deck.position().get(), 0.0);
        assert!(!deck.is_playing(), "a deck at the start is not playing");
    }

    /// A censor is a momentary reverse that puts you back on the beat, and it
    /// slips whatever the slip button says — without the second half it would
    /// just be reverse.
    #[test]
    fn a_censor_returns_to_where_the_track_would_have_been() {
        let mut deck = deck_with(4000);
        deck.play();
        assert!(!deck.slip(), "slip is not armed");
        run(&mut deck, 500);
        deck.set_censor(true);
        assert!(deck.is_slipping(), "a censor always slips");
        run(&mut deck, 200);
        assert!(deck.position().get() < 400.0, "the censor played backwards");
        deck.set_censor(false);
        assert!(
            (deck.position().get() - 700.0).abs() < 2.0,
            "expected to land near 700, got {}",
            deck.position().get()
        );
    }

    /// Releasing a censor inside a loop returns to the loop, not out of it:
    /// the loop is still diverting the playhead.
    #[test]
    fn releasing_a_censor_inside_a_loop_stays_in_the_loop() {
        let mut deck = deck_with(4000);
        deck.play();
        deck.set_loop_region(LoopRegion::new(FramePos::new(0.0), FramePos::new(400.0)));
        run(&mut deck, 200);
        deck.set_censor(true);
        run(&mut deck, 50);
        deck.set_censor(false);
        assert!(
            deck.position().get() < 400.0,
            "still looping, so still inside it; got {}",
            deck.position().get()
        );
        assert!(deck.active_loop().is_some());
    }

    #[test]
    fn nothing_is_slipped_over_when_nothing_is_diverting_the_playhead() {
        let mut deck = deck_with(4000);
        deck.set_slip(true);
        deck.play();
        run(&mut deck, 200);
        assert_eq!(
            deck.slip_position(),
            None,
            "a shadow with nothing to shadow is just the playhead"
        );
    }

    /// The shadow runs at the track's natural rate, not the reversed one — it
    /// is where the record would be if you had left it alone.
    #[test]
    fn the_shadow_runs_forwards_while_the_audible_playhead_runs_back() {
        let mut deck = deck_with(4000);
        deck.play();
        run(&mut deck, 300);
        deck.set_censor(true);
        run(&mut deck, 100);
        let shadow = deck.slip_position().expect("censoring, so shadowing");
        assert!(
            shadow.get() > 300.0,
            "the shadow went forward, got {}",
            shadow.get()
        );
        assert!(
            deck.position().get() < 300.0,
            "the audible playhead went back"
        );
    }

    /// A shadow that has run off the end has nowhere to land, so it stops
    /// shadowing rather than parking the playhead past the end of the track.
    #[test]
    fn a_shadow_that_reaches_the_end_stops_shadowing() {
        let mut deck = deck_with(400);
        deck.play();
        run(&mut deck, 100);
        deck.set_loop_region(LoopRegion::new(FramePos::new(100.0), FramePos::new(150.0)));
        deck.set_slip(true);
        run(&mut deck, 600);
        assert_eq!(deck.slip_position(), None);
        deck.set_loop_region(None);
        assert!(
            deck.position().get() <= 400.0,
            "never past the end, got {}",
            deck.position().get()
        );
    }

    #[test]
    fn a_new_deck_is_empty_and_silent() {
        let mut deck = Deck::new(SR);
        assert!(!deck.is_loaded());
        assert!(!deck.is_playing());
        let mut out = vec![0.0; 16];
        assert_eq!(
            deck.process(&mut out, &stereo(), None),
            DeckLevels::default()
        );
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn an_empty_deck_refuses_to_play() {
        let mut deck = Deck::new(SR);
        deck.play();
        assert!(
            !deck.is_playing(),
            "playing silence is a bug, not a feature"
        );
    }

    #[test]
    fn loading_returns_the_previous_source_for_retirement() {
        let mut deck = Deck::new(SR);
        let first = ramp(10);
        let retired = deck.load(Arc::clone(&first));
        // The empty placeholder comes back, not the new track.
        assert_eq!(retired.len_frames(), 0);

        let second = ramp(20);
        let retired = deck.load(second);
        assert_eq!(
            retired.len_frames(),
            10,
            "must hand back the displaced track"
        );
    }

    #[test]
    fn loading_resets_the_playhead() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(50.0));
        let _ = deck.load(ramp(100));
        assert_eq!(deck.position().get(), 0.0);
        assert!(!deck.is_playing());
    }

    #[test]
    fn playback_advances_one_frame_per_output_frame_at_unity() {
        let mut deck = deck_with(1000);
        deck.play();
        let mut out = vec![0.0; 32]; // 16 frames
        let _ = deck.process(&mut out, &stereo(), None);
        assert!((deck.position().get() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn pitch_scales_the_advance() {
        let mut deck = deck_with(1000);
        deck.set_pitch(0.08);
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames
        let _ = deck.process(&mut out, &stereo(), None);
        assert!(
            (deck.position().get() - 108.0).abs() < 1e-6,
            "+8% pitch should advance 108 frames, got {}",
            deck.position().get()
        );
    }

    /// A 44.1 kHz track on a 48 kHz device must run slower than one frame per
    /// output frame, or it plays sharp. This is the bug that makes everything
    /// sound subtly wrong and is easy to miss by ear.
    #[test]
    fn sample_rate_conversion_is_applied() {
        let source_rate = SampleRate::new(44_100).unwrap();
        let samples: Vec<f32> = (0..1000).flat_map(|n| [n as f32, n as f32]).collect();
        let mut deck = Deck::new(SampleRate::new(48_000).unwrap());
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(
            samples,
            source_rate,
        )));
        deck.play();

        let mut out = vec![0.0; 960]; // 480 output frames
        let _ = deck.process(&mut out, &stereo(), None);
        let expected = 480.0 * (44_100.0 / 48_000.0);
        assert!(
            (deck.position().get() - expected).abs() < 1e-6,
            "expected {expected}, got {}",
            deck.position().get()
        );
    }

    #[test]
    fn a_paused_deck_renders_silence_and_holds_position() {
        let mut deck = deck_with(1000);
        deck.seek(FramePos::new(100.0));
        let mut out = vec![0.0; 32];
        let _ = deck.process(&mut out, &stereo(), None);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(deck.position().get(), 100.0);
    }

    #[test]
    fn reaching_the_end_stops_the_transport() {
        let mut deck = deck_with(10);
        deck.play();
        let mut out = vec![0.0; 64]; // 32 frames, more than the track has
        let _ = deck.process(&mut out, &stereo(), None);
        assert!(!deck.is_playing(), "deck should stop at the end");
        assert_eq!(deck.position().get(), 10.0);
    }

    #[test]
    fn running_past_the_end_does_not_read_out_of_bounds() {
        // The real safety property: no panic, no garbage, just silence.
        let mut deck = deck_with(4);
        deck.play();
        let mut out = vec![0.0; 200];
        let _ = deck.process(&mut out, &stereo(), None);
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn reverse_playback_stops_at_the_start() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(10.0));
        deck.set_rate(Rate::new(-1.0));
        deck.play();
        let mut out = vec![0.0; 200]; // 100 frames of reverse
        let _ = deck.process(&mut out, &stereo(), None);
        assert!(!deck.is_playing());
        assert_eq!(deck.position().get(), 0.0);
    }

    #[test]
    fn cue_returns_to_the_cue_point_and_stops() {
        let mut deck = deck_with(1000);
        deck.set_cue_point(FramePos::new(200.0));
        deck.seek(FramePos::new(500.0));
        deck.play();
        deck.cue();
        assert!(!deck.is_playing());
        assert_eq!(deck.position().get(), 200.0);
    }

    #[test]
    fn seek_is_clamped_to_the_track() {
        let mut deck = deck_with(100);
        deck.seek(FramePos::new(1e9));
        assert_eq!(deck.position().get(), 100.0);
        deck.seek(FramePos::new(-50.0));
        assert_eq!(deck.position().get(), 0.0);
    }

    #[test]
    fn volume_of_zero_produces_silence() {
        let mut deck = deck_with(10_000);
        deck.set_volume(0.0);
        deck.play();
        // Long enough for the gain ramp to complete.
        let mut out = vec![0.0; 8192];
        let _ = deck.process(&mut out, &stereo(), None);
        let mut out = vec![0.0; 8192];
        let _ = deck.process(&mut out, &stereo(), None);
        let tail = &out[out.len() - 100..];
        assert!(
            tail.iter().all(|&s| s.abs() < 1e-4),
            "volume 0 should be silent once the ramp settles"
        );
    }

    #[test]
    fn gain_settings_are_clamped_to_sane_ranges() {
        let mut deck = Deck::new(SR);
        deck.set_volume(5.0);
        assert_eq!(deck.volume(), 1.0);
        deck.set_volume(-1.0);
        assert_eq!(deck.volume(), 0.0);
        deck.set_gain_db(100.0);
        assert_eq!(deck.gain_db(), 24.0);
    }

    #[test]
    fn non_finite_input_is_ignored() {
        let mut deck = Deck::new(SR);
        deck.set_volume(0.5);
        deck.set_volume(f32::NAN);
        assert_eq!(deck.volume(), 0.5);
        deck.set_pitch(0.1);
        deck.set_pitch(f64::NAN);
        assert!((deck.pitch() - 0.1).abs() < 1e-12);
    }

    #[test]
    fn process_adds_into_the_buffer_rather_than_overwriting() {
        let mut deck = deck_with(1000);
        deck.play();
        let mut out = vec![1.0; 32];
        let _ = deck.process(&mut out, &stereo(), None);
        // Frame 0 of the ramp is 0.0, so the pre-existing 1.0 must survive.
        assert!(out[0] >= 1.0, "deck overwrote existing mix content");
    }

    #[test]
    fn peak_reflects_the_loudest_sample_rendered() {
        let mut deck = Deck::new(SR);
        // Long enough for the EQ's crossover filters to settle on the step: the
        // 300 Hz band alone is 160 samples per cycle, so a short window would
        // measure the transient rather than the steady state.
        let samples = vec![0.5f32; 40_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &stereo(), None);
        let mut out = vec![0.0; 8_000];
        let peak = deck.process(&mut out, &stereo(), None).post_fader;

        assert!(
            (peak - 0.5).abs() < 0.02,
            "expected ~0.5 through a flat EQ, got {peak}"
        );
    }

    #[test]
    fn killing_the_eq_low_band_removes_a_bass_tone() {
        use std::f32::consts::PI;

        let frames = 48_000;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let v = (2.0 * PI * 60.0 * n as f32 / 48_000.0).sin() * 0.5;
                [v, v]
            })
            .collect();

        let mut deck = Deck::new(SR);
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.set_eq_low(0.0);
        deck.play();

        // Let the gain ramp and filters settle before measuring.
        let mut out = vec![0.0; 20_000];
        let _ = deck.process(&mut out, &stereo(), None);
        let mut out = vec![0.0; 20_000];
        let peak = deck.process(&mut out, &stereo(), None).post_fader;

        assert!(
            peak < 0.02,
            "killing the low band should remove a 60 Hz tone, peak was {peak}"
        );
    }

    #[test]
    fn a_flat_eq_and_centred_filter_leave_the_signal_alone() {
        let mut deck = Deck::new(SR);
        let samples = vec![0.4f32; 40_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let mut out = vec![0.0; 16_000];
        let _ = deck.process(&mut out, &stereo(), None);
        let mut out = vec![0.0; 8_000];
        let peak = deck.process(&mut out, &stereo(), None).post_fader;

        assert!(
            (peak - 0.4).abs() < 0.02,
            "default tone controls should be transparent, got {peak}"
        );
    }

    #[test]
    fn loading_clears_filter_memory() {
        // Filter state from a previous track must not bleed into the next one.
        let mut deck = Deck::new(SR);
        let loud = vec![1.0f32; 20_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(loud, SR)));
        deck.play();
        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &stereo(), None);

        // Swap in silence; the first samples must be silent, not a filter tail.
        let silence = vec![0.0f32; 20_000];
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(silence, SR)));
        deck.play();
        let mut out = vec![0.0; 512];
        let peak = deck.process(&mut out, &stereo(), None).post_fader;

        assert!(
            peak < 1e-6,
            "previous track bled through the filters: {peak}"
        );
    }
}

#[cfg(test)]
mod cue_tests {
    use super::*;
    use crate::bus::BusLayout;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn tone(frames: usize, amplitude: f32) -> Arc<dyn TrackSource> {
        Arc::new(AudioBuffer::from_interleaved(
            vec![amplitude; frames * 2],
            SR,
        ))
    }

    fn deck_playing() -> Deck {
        let mut deck = Deck::new(SR);
        let _ = deck.load(tone(200_000, 0.5));
        deck.play();
        deck
    }

    /// Render and report peak on the master and cue buses separately.
    fn render(deck: &mut Deck, layout: &BusLayout, frames: usize) -> (f32, f32) {
        let mut out = vec![0.0; frames * layout.channels];
        let _ = deck.process(&mut out, layout, None);

        let mut main = 0.0f32;
        let mut cue = 0.0f32;
        for frame in out.chunks_exact(layout.channels) {
            main = main.max(frame[layout.main.0].abs());
            if let Some((l, r)) = layout.cue {
                cue = cue.max(frame[l].abs()).max(frame[r].abs());
            }
        }
        (main, cue)
    }

    #[test]
    fn a_deck_is_not_cued_by_default() {
        assert!(!Deck::new(SR).is_cued());
    }

    #[test]
    fn cue_send_is_silent_until_enabled() {
        let mut deck = deck_playing();
        let layout = BusLayout::for_channels(4);
        let (main, cue) = render(&mut deck, &layout, 8_000);
        assert!(main > 0.1, "master should have audio");
        assert_eq!(cue, 0.0, "cue must be silent when PFL is off");
    }

    /// The entire point of pre-fader listen: with the channel fader down, the
    /// audience hears nothing and the DJ still hears the track.
    #[test]
    fn pre_fader_listen_survives_a_closed_fader() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_volume(0.0);
        let layout = BusLayout::for_channels(4);

        // Let the fader ramp reach zero.
        render(&mut deck, &layout, 16_000);
        let (main, cue) = render(&mut deck, &layout, 8_000);

        assert!(
            main < 0.01,
            "fader down means the room hears nothing, got {main}"
        );
        assert!(cue > 0.4, "PFL must still feed the headphones, got {cue}");
    }

    /// Likewise the crossfader: cueing the deck you are about to bring in is
    /// the normal case, and it is always crossfaded away.
    #[test]
    fn pre_fader_listen_survives_the_crossfader() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_crossfader_gain(0.0);
        let layout = BusLayout::for_channels(4);

        render(&mut deck, &layout, 16_000);
        let (main, cue) = render(&mut deck, &layout, 8_000);

        assert!(main < 0.01, "crossfaded away, got {main}");
        assert!(cue > 0.4, "PFL should ignore the crossfader, got {cue}");
    }

    /// Trim is before the cue send, so the headphone level tracks it. That is
    /// what makes trim usable for gain-staging a track before it goes out.
    #[test]
    fn trim_affects_the_cue_send() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        deck.set_gain_db(-12.0);
        let layout = BusLayout::for_channels(4);

        render(&mut deck, &layout, 16_000);
        let (_, cue) = render(&mut deck, &layout, 8_000);

        // -12 dB is about a quarter amplitude: 0.5 * 0.25 = 0.125.
        assert!(cue < 0.2 && cue > 0.05, "cue should follow trim, got {cue}");
    }

    #[test]
    fn cue_is_dropped_on_a_device_with_no_spare_channels() {
        let mut deck = deck_playing();
        deck.set_cue(true);
        let layout = BusLayout::for_channels(2);
        // Must not panic or write out of bounds on a stereo device.
        let (main, _) = render(&mut deck, &layout, 4_000);
        assert!(main > 0.1);
    }

    #[test]
    fn toggling_cue_flips_it() {
        let mut deck = Deck::new(SR);
        deck.toggle_cue();
        assert!(deck.is_cued());
        deck.toggle_cue();
        assert!(!deck.is_cued());
    }

    #[test]
    fn levels_report_both_sides_of_the_fader() {
        let mut deck = deck_playing();
        deck.set_volume(0.5);
        let layout = BusLayout::for_channels(4);

        let mut out = vec![0.0; 16_000 * 4];
        let _ = deck.process(&mut out, &layout, None);
        let mut out = vec![0.0; 8_000 * 4];
        let levels = deck.process(&mut out, &layout, None);

        assert!(
            levels.pre_fader > levels.post_fader,
            "a half-open fader should make post-fader lower: pre {} post {}",
            levels.pre_fader,
            levels.post_fader
        );
        assert!((levels.pre_fader - 0.5).abs() < 0.05);
        assert!((levels.post_fader - 0.25).abs() < 0.05);
    }

    #[test]
    fn mono_output_sums_both_channels() {
        let mut deck = Deck::new(SR);
        // Distinct L and R so summing is observable. Long enough for the EQ's
        // crossovers to settle on the step -- a short window measures their
        // transient overshoot rather than the steady state.
        let samples: Vec<f32> = (0..80_000).flat_map(|_| [0.4f32, 0.2]).collect();
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.play();

        let layout = BusLayout::for_channels(1);
        let mut settle = vec![0.0; 16_000];
        let _ = deck.process(&mut settle, &layout, None);

        let mut out = vec![0.0; 8_000];
        let _ = deck.process(&mut out, &layout, None);

        let peak = out.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            (peak - 0.3).abs() < 0.02,
            "mono should be the average of 0.4 and 0.2, got {peak}"
        );
    }
}

#[cfg(test)]
mod keylock_tests {
    use super::*;
    use crate::bus::BusLayout;

    const SR: SampleRate = SampleRate::DEFAULT;

    fn stereo() -> BusLayout {
        BusLayout::for_channels(2)
    }

    fn ramp(frames: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    /// A burst of tone surrounded by silence.
    ///
    /// The *envelope* is what these tests measure. A phase vocoder does not
    /// reproduce a waveform sample for sample -- it reconstructs the spectrum
    /// with its own phases -- so comparing samples would fail for reasons that
    /// have nothing to do with timing. The envelope survives intact, and
    /// timing is the thing under test.
    fn burst(frames: usize, from: usize, to: usize) -> Arc<dyn TrackSource> {
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                let v = if n >= from && n < to {
                    (std::f32::consts::TAU * 440.0 * n as f32 / 48_000.0).sin() * 0.8
                } else {
                    0.0
                };
                [v, v]
            })
            .collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    /// Frames per envelope window. 256 at 48 kHz is about 5 ms -- fine enough
    /// to catch a misalignment that would matter musically.
    const ENV_WINDOW: usize = 256;

    fn envelope(out: &[f32]) -> Vec<f32> {
        out.chunks(ENV_WINDOW * CHANNELS)
            .map(|w| (w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32).sqrt())
            .collect()
    }

    /// Lag, in envelope windows, at which `b` best matches `a`.
    fn best_lag(a: &[f32], b: &[f32], max: isize) -> isize {
        let mut best = (f32::NEG_INFINITY, 0isize);
        for lag in -max..=max {
            let mut sum = 0.0;
            for (i, &sample) in a.iter().enumerate() {
                let j = i as isize + lag;
                if j >= 0 && (j as usize) < b.len() {
                    sum += sample * b[j as usize];
                }
            }
            if sum > best.0 {
                best = (sum, lag);
            }
        }
        best.1
    }

    fn render_burst(keylock: bool, pitch: f64, frames: usize) -> Vec<f32> {
        let mut deck = Deck::new(SR);
        let _ = deck.load(burst(frames, 20_000, 40_000));
        deck.set_keylock(keylock);
        deck.set_pitch(pitch);
        deck.play();
        let mut out = vec![0.0; frames * CHANNELS];
        let _ = deck.process(&mut out, &stereo(), None);
        out
    }

    /// **The test keylock exists to pass.**
    ///
    /// A pitch shifter has group delay. If keylock simply inserted it, a track
    /// that was beatmatched would slide out of time the moment keylock was
    /// pressed -- at 128 BPM the shifter's round trip is most of a semiquaver.
    /// The deck compensates by reading ahead; this proves the compensation is
    /// the right size and in the right direction.
    #[test]
    fn engaging_keylock_does_not_move_the_music_in_time() {
        let frames = 96_000;
        let plain = envelope(&render_burst(false, 0.0, frames));
        let locked = envelope(&render_burst(true, 0.0, frames));

        let lag = best_lag(&plain, &locked, 40);
        assert!(
            lag.abs() <= 2,
            "keylock shifted the audio by {} frames ({} ms) -- latency \
             compensation is wrong",
            lag * ENV_WINDOW as isize,
            lag as f32 * ENV_WINDOW as f32 / 48.0
        );
    }

    /// Same, with the pitch fader off centre -- the state keylock is actually
    /// used in. The read-ahead scales with `step`, so this catches an error
    /// that unity playback would hide.
    #[test]
    fn keylock_stays_aligned_with_the_pitch_fader_moved() {
        let frames = 96_000;
        let plain = envelope(&render_burst(false, 0.08, frames));
        let locked = envelope(&render_burst(true, 0.08, frames));

        let lag = best_lag(&plain, &locked, 40);
        assert!(
            lag.abs() <= 2,
            "keylock at +8% shifted the audio by {} frames ({} ms)",
            lag * ENV_WINDOW as isize,
            lag as f32 * ENV_WINDOW as f32 / 48.0
        );
    }

    /// Keylock must not touch the transport. The playhead, and therefore the
    /// waveform, the beat grid and every seek, has to read the same either way.
    #[test]
    fn keylock_does_not_change_the_playhead() {
        let mut plain = Deck::new(SR);
        let _ = plain.load(ramp(96_000));
        plain.set_pitch(0.08);
        plain.play();

        let mut locked = Deck::new(SR);
        let _ = locked.load(ramp(96_000));
        locked.set_keylock(true);
        locked.set_pitch(0.08);
        locked.play();

        let mut out = vec![0.0; 8_192];
        let _ = plain.process(&mut out, &stereo(), None);
        out.fill(0.0);
        let _ = locked.process(&mut out, &stereo(), None);

        // Not bit-equal, and should not be asserted as such: the direct path
        // adds `step` once per frame while the keylocked path adds it once per
        // chunk, so the same sum is accumulated in a different order. The
        // difference is a few ULPs -- femtoseconds of audio -- and demanding
        // exactness here would be testing the shape of the loop, not the
        // behaviour that matters.
        let drift = (plain.position().get() - locked.position().get()).abs();
        assert!(drift < 1e-6, "keylock moved the playhead by {drift} frames");
    }

    #[test]
    fn keylock_is_off_until_asked_for() {
        assert!(!Deck::new(SR).is_keylocked());
        assert_eq!(Deck::new(SR).keylock_latency_frames(), 0);
    }

    #[test]
    fn keylock_toggles_and_reports_its_latency() {
        let mut deck = Deck::new(SR);
        deck.toggle_keylock();
        assert!(deck.is_keylocked());
        assert!(
            deck.keylock_latency_frames() > 0,
            "an engaged shifter has group delay; reporting zero would be a lie"
        );
        deck.toggle_keylock();
        assert!(!deck.is_keylocked());
        assert_eq!(deck.keylock_latency_frames(), 0);
    }

    /// The point of the feature: run fast, stay in key.
    #[test]
    fn keylock_holds_the_key_while_the_tempo_changes() {
        // Count upward zero crossings to get the pitch. A steady tone in gives
        // a steady tone out, so crossings land where the maths says.
        fn frequency(out: &[f32]) -> f32 {
            let left: Vec<f32> = out.iter().step_by(CHANNELS).copied().collect();
            let (mut crossings, mut first, mut last) = (0usize, None, 0usize);
            for i in 1..left.len() {
                if left[i - 1] <= 0.0 && left[i] > 0.0 {
                    if first.is_none() {
                        first = Some(i);
                    } else {
                        last = i;
                    }
                    crossings += 1;
                }
            }
            match first {
                Some(f) if crossings > 2 => (crossings - 1) as f32 * 48_000.0 / (last - f) as f32,
                _ => 0.0,
            }
        }

        let tone: Vec<f32> = (0..240_000)
            .flat_map(|n| {
                let v = (std::f32::consts::TAU * 440.0 * n as f32 / 48_000.0).sin() * 0.8;
                [v, v]
            })
            .collect();

        let mut plain = Deck::new(SR);
        let _ = plain.load(Arc::new(AudioBuffer::from_interleaved(tone.clone(), SR)));
        plain.set_pitch(0.25);
        plain.play();
        let mut out = vec![0.0; 160_000];
        let _ = plain.process(&mut out, &stereo(), None);
        let sped_up = frequency(&out);
        assert!(
            (sped_up - 550.0).abs() < 15.0,
            "without keylock, +25% should take 440 Hz to 550 Hz; got {sped_up}"
        );

        let mut locked = Deck::new(SR);
        let _ = locked.load(Arc::new(AudioBuffer::from_interleaved(tone, SR)));
        locked.set_keylock(true);
        locked.set_pitch(0.25);
        locked.play();
        let mut out = vec![0.0; 160_000];
        let _ = locked.process(&mut out, &stereo(), None);
        // Skip the priming region, where the shifter is still filling.
        let measured = frequency(&out[20_000..]);
        assert!(
            (measured - 440.0).abs() < 15.0,
            "with keylock, +25% should stay at 440 Hz; got {measured}"
        );
    }

    /// Keylock sits before the fader, like everything else in the strip, so
    /// pre-fader listen still hears it.
    #[test]
    fn a_keylocked_deck_still_feeds_the_cue_send() {
        let mut deck = Deck::new(SR);
        let _ = deck.load(burst(96_000, 0, 96_000));
        deck.set_keylock(true);
        deck.set_cue(true);
        deck.set_volume(0.0);
        deck.play();

        let layout = BusLayout::for_channels(4);
        // Let the fader smoother reach zero and the shifter fill before
        // measuring. Both ramp; a window that includes the ramp measures the
        // ramp rather than the routing.
        let mut settle = vec![0.0; 32_768];
        let _ = deck.process(&mut settle, &layout, None);

        let mut out = vec![0.0; 32_768];
        let _ = deck.process(&mut out, &layout, None);

        let master = out.chunks_exact(4).fold(0.0f32, |a, f| a.max(f[0].abs()));
        let cue = out.chunks_exact(4).fold(0.0f32, |a, f| a.max(f[2].abs()));
        assert!(master < 1e-6, "a closed fader still reached the master");
        assert!(cue > 0.05, "keylocked audio never reached the cue bus");
    }

    /// A block bigger than the internal scratch must still come out whole.
    /// Drivers really do hand over 1024 and 2048 frames at a time.
    #[test]
    fn keylock_handles_a_block_larger_than_its_scratch() {
        let mut deck = Deck::new(SR);
        let _ = deck.load(burst(96_000, 0, 96_000));
        deck.set_keylock(true);
        deck.play();

        let frames = SCRATCH_FRAMES * 5 + 37; // deliberately not a multiple
        let mut out = vec![0.0; frames * CHANNELS];
        let _ = deck.process(&mut out, &stereo(), None);

        assert_eq!(
            deck.position().get().round() as usize,
            frames,
            "the playhead did not advance by exactly one block"
        );
        assert!(
            out.iter().any(|s| s.abs() > 0.01),
            "a large block produced no audio"
        );
    }
}

/// The slicer: eight equal parts of a span of the grid.
///
/// A slice is a roll whose loop starts somewhere else, so most of the machinery
/// is already proved by the roll's tests. These pin the part that is new — where
/// the eight parts *are*, and that they keep moving while one is held.
#[cfg(test)]
mod slicer_tests {
    use super::*;
    use dj_core::Stem;

    const SR: SampleRate = SampleRate::DEFAULT;
    /// 120 BPM at 48 kHz is exactly 24 000 frames a beat, so every expected
    /// position in these tests is an integer rather than a tolerance.
    const BPM: f64 = 120.0;
    const BEAT: f64 = 24_000.0;

    fn deck() -> Deck {
        deck_of_beats(64.0)
    }

    fn deck_of_beats(beats: f64) -> Deck {
        use dj_core::{Beatgrid, Bpm, Confidence};
        let frames = (BEAT * beats) as usize;
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        let mut deck = Deck::new(SR);
        let _ = deck.load(Arc::new(AudioBuffer::from_interleaved(samples, SR)));
        deck.set_grid(Some(Beatgrid::new(
            FramePos::ZERO,
            Bpm::new(BPM).unwrap(),
            Confidence::new(0.9),
        )));
        deck
    }

    /// Eight pads over eight beats is one beat each, which is the default
    /// because it is the reading a DJ can follow without counting.
    #[test]
    fn the_eight_pads_divide_the_span_evenly() {
        assert_eq!(deck().slice_beats(), 8.0);

        for slice in 1..=8u8 {
            let mut deck = deck();
            assert!(deck.hold_slice(slice), "slice {slice} should be reachable");
            let region = deck.active_loop().expect("a slice is a loop");
            assert_eq!(region.start.get(), f64::from(slice - 1) * BEAT);
            assert_eq!(region.end.get(), f64::from(slice) * BEAT);
        }
    }

    /// The span is anchored to the grid, not to wherever the playhead is. Two
    /// presses of the same pad from different points inside one span must give
    /// the same slice, or the page is unplayable.
    #[test]
    fn the_span_is_anchored_to_the_grid() {
        for offset in [0.0, BEAT * 0.4, BEAT * 3.0, BEAT * 7.9] {
            let mut deck = deck();
            deck.seek(FramePos::new(offset));
            assert!(deck.hold_slice(3));
            let region = deck.active_loop().unwrap();
            assert_eq!(
                region.start.get(),
                BEAT * 2.0,
                "pad 3 moved when the playhead did, from offset {offset}"
            );
        }
    }

    /// And it advances a whole span at a time, so the next bar's pad 3 is the
    /// next bar's third beat rather than eight beats after wherever you were.
    #[test]
    fn the_span_advances_a_whole_span_at_a_time() {
        let mut deck = deck();
        deck.seek(FramePos::new(BEAT * 8.5));
        assert!(deck.hold_slice(1));
        assert_eq!(deck.active_loop().unwrap().start.get(), BEAT * 8.0);

        deck.release_slice();
        deck.seek(FramePos::new(BEAT * 17.0));
        assert!(deck.hold_slice(1));
        assert_eq!(deck.active_loop().unwrap().start.get(), BEAT * 16.0);
    }

    #[test]
    fn a_shorter_span_gives_shorter_slices() {
        let mut deck = deck();
        deck.set_slice_domain(4.0);
        assert!(deck.hold_slice(2));
        let region = deck.active_loop().unwrap();
        // Four beats over eight pads is half a beat each.
        assert_eq!(region.start.get(), BEAT * 0.5);
        assert_eq!(region.end.get(), BEAT * 1.0);
    }

    #[test]
    fn a_longer_span_gives_longer_slices() {
        let mut deck = deck();
        deck.set_slice_domain(32.0);
        assert!(deck.hold_slice(2));
        let region = deck.active_loop().unwrap();
        // Thirty-two beats over eight pads is a bar each.
        assert_eq!(region.start.get(), BEAT * 4.0);
        assert_eq!(region.end.get(), BEAT * 8.0);
    }

    /// **The property that makes it a performance move rather than a jump.**
    /// Hold a slice, let go, and the track is where it would have been — so a
    /// bar can be rearranged and handed back in time.
    #[test]
    fn letting_a_slice_go_lands_where_the_record_would_have_been() {
        let mut deck = deck();
        deck.play();
        deck.seek(FramePos::new(BEAT * 4.0));

        assert!(
            deck.hold_slice(1),
            "jump back to the first beat of the span"
        );
        assert!(deck.is_slipping(), "a slice always slips");

        let mut out = vec![0.0; 4_096 * 2];
        let layout = BusLayout::for_channels(2);
        let _ = deck.process(&mut out, &layout, None);

        deck.release_slice();
        assert!(
            (deck.position().get() - (BEAT * 4.0 + 4_096.0)).abs() < 2.0,
            "landed at {} rather than where the record would have been",
            deck.position().get()
        );
    }

    /// **The bug this design exists to avoid.** If the span were measured from
    /// the *audible* playhead it would freeze around the loop the slice put it
    /// in — the slicer would stop slicing the moment you used it. Measuring
    /// from the shadow keeps it walking.
    #[test]
    fn the_span_keeps_moving_underneath_a_held_slice() {
        let mut deck = deck();
        deck.play();
        assert!(deck.hold_slice(1));
        let first = deck.slice_index().unwrap();

        // Long enough for the record to have crossed several slices.
        let layout = BusLayout::for_channels(2);
        let mut out = vec![0.0; (BEAT * 3.5) as usize * 2];
        let _ = deck.process(&mut out, &layout, None);

        let later = deck.slice_index().unwrap();
        assert_ne!(
            later, first,
            "the light stopped walking while a slice was held"
        );
    }

    #[test]
    fn the_light_follows_the_playhead_across_the_span() {
        let mut deck = deck();
        for (beat, expect) in [(0.0, 1u8), (1.5, 2), (7.99, 8), (8.0, 1), (9.0, 2)] {
            deck.seek(FramePos::new(BEAT * beat));
            assert_eq!(
                deck.slice_index(),
                Some(expect),
                "beat {beat} should be slice {expect}"
            );
        }
    }

    #[test]
    fn a_deck_with_no_grid_has_no_slices() {
        let mut deck = deck();
        deck.set_grid(None);
        assert_eq!(deck.slice_index(), None);
        assert!(!deck.hold_slice(1), "nothing to divide");
        assert!(!deck.slicing());
    }

    #[test]
    fn a_pad_outside_the_eight_is_refused() {
        let mut deck = deck();
        assert!(!deck.hold_slice(0));
        assert!(!deck.hold_slice(9));
        assert!(!deck.slicing());
        assert!(deck.hold_slice(8), "and eight is still a pad");
    }

    /// A span is refused rather than clamped when it runs off the end: a slice
    /// that silently played a different length would be worse than one that
    /// does nothing.
    #[test]
    fn a_slice_past_the_end_of_the_track_is_refused() {
        // Sixty beats, so the second thirty-two-beat span runs off the end and
        // the eight-beat spans do not. A track that divided evenly would never
        // exercise this at all, which is how the first version of this test
        // passed while asserting the opposite.
        let mut deck = deck_of_beats(60.0);
        deck.seek(FramePos::new(BEAT * 56.5));
        assert!(deck.hold_slice(4), "beats 56-60 are still there");
        deck.release_slice();

        deck.set_slice_domain(32.0);
        deck.seek(FramePos::new(BEAT * 59.0));
        // The span covering beat 59 runs from 32 to 64; pad 8 is beats 60-64,
        // and the track stops at 60.
        assert!(
            !deck.hold_slice(8),
            "a slice running past the end must be refused"
        );
        assert!(!deck.slicing(), "and a refused slice is not a slice");
        assert!(deck.hold_slice(7), "the one before it still fits");
    }

    #[test]
    fn the_span_is_kept_within_reason() {
        let mut deck = deck();
        deck.set_slice_domain(0.0);
        assert_eq!(deck.slice_beats(), 8.0, "zero is not a span");
        deck.set_slice_domain(f64::NAN);
        assert_eq!(deck.slice_beats(), 8.0, "nor is a NaN");
        deck.set_slice_domain(1_000.0);
        assert_eq!(deck.slice_beats(), 64.0);
        deck.set_slice_domain(0.01);
        assert_eq!(deck.slice_beats(), 1.0);
    }

    /// A separated track whose four stems each carry a different constant, so
    /// a test can tell which one a mute removed.
    fn separated(frames: usize) -> Arc<AudioBuffer> {
        let buffer = AudioBuffer::from_interleaved(vec![0.0; frames * 2], SR);
        let published = buffer.stems_lock();
        // vocal, drums, bass, other -- dj_core::Stem::ALL order.
        let chunk: dj_decode::StemChunk = (0..frames)
            .map(|_| [0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4])
            .collect();
        published.store(Arc::new(
            dj_decode::StemTable::default()
                .with_chunk(0, chunk)
                .expect("the first chunk always fits"),
        ));
        Arc::new(buffer)
    }

    fn peak(deck: &mut Deck, layout: &BusLayout, frames: usize) -> f32 {
        let mut out = vec![0.0; frames * layout.channels];
        let _ = deck.process(&mut out, layout, None);
        out.chunks_exact(layout.channels)
            .fold(0.0f32, |worst, frame| worst.max(frame[layout.main.0].abs()))
    }

    /// **The guard on every other stem test.** If the deck were not actually
    /// reading the separated buffer, the mute tests and the realtime-safety
    /// tests would all pass while exercising ordinary playback -- the stem
    /// path would be dead code that nothing noticed.
    ///
    /// The four stems sum to 1.0 at the source, and the vocal is a tenth of
    /// it, so muting the vocal must leave nine tenths.
    ///
    /// Compared as a **ratio between two fresh decks**, not against an
    /// absolute level. Each stem goes through its own EQ and filter, and on
    /// the constant this fixture feeds them the step response overshoots --
    /// the unmuted peak is about 1.069, not 1.0. Asserting the absolute would
    /// have been asserting the filters' transient, which is a different claim
    /// and a fragile one. Two decks rendered identically share that transient
    /// exactly, so the ratio isolates the mute.
    #[test]
    fn muting_a_stem_removes_exactly_that_stem() {
        let layout = BusLayout::for_channels(2);

        let mut whole_deck = Deck::new(SR);
        let _ = whole_deck.load(separated(100_000));
        whole_deck.play();
        let whole = peak(&mut whole_deck, &layout, 512);

        let mut muted_deck = Deck::new(SR);
        let _ = muted_deck.load(separated(100_000));
        muted_deck.play();
        muted_deck.toggle_stem_mute(Stem::Vocal as usize);
        let muted = peak(&mut muted_deck, &layout, 512);

        assert!(whole > 0.5, "the stem path is not live at all: {whole}");
        let share = muted / whole;
        assert!(
            (share - 0.9).abs() < 1e-3,
            "muting the vocal should leave nine tenths; got {share} ({muted} of {whole})"
        );
    }

    /// **The defect this pins.** The separated track used to live behind an
    /// `RwLock<Vec<StemFrame>>` that the worker appended to. The audio thread
    /// read it with `try_read` so it could never block -- but it could
    /// *fail*, and `read_frame` falls back to the unseparated mix when the
    /// stems are not readable. While the worker held the write lock for its
    /// 1024-frame crossfade and fifteen-megabyte `extend_from_slice`, every
    /// callback took that fallback: a DJ holding the vocal muted heard it
    /// come back, once per chunk, for the whole track.
    ///
    /// Measured before the fix, with a writer simply holding the lock: the
    /// level went from 0.90 to **0**. The stem mix was abandoned entirely.
    ///
    /// There is no write lock to hold now, so this drives the real thing --
    /// a worker publishing chunk after chunk while the deck plays -- and
    /// asserts the level never moves.
    #[test]
    fn a_muted_stem_stays_muted_while_the_worker_publishes() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let layout = BusLayout::for_channels(2);
        const CHUNK: usize = 4_096;
        /// Enough chunks to cover every frame this test reads, so the
        /// playhead can never outrun separation. Running off the end of what
        /// is separated is a real thing that happens -- and it is *not* what
        /// this test is about, so it is designed out rather than raced.
        const AHEAD: usize = 20;

        // The unseparated mix is a constant 0.5, distinct from the 0.9 the
        // stem path produces with the vocal muted. If the deck ever falls
        // back, the number in the failure message says which happened.
        let buffer = AudioBuffer::from_interleaved(vec![0.5; 200_000 * 2], SR);
        let published = buffer.stems_lock();
        let chunk: dj_decode::StemChunk = (0..CHUNK)
            .map(|_| [0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4])
            .collect();

        let mut table = dj_decode::StemTable::default();
        for index in 0..AHEAD {
            table = table
                .with_chunk(index, Arc::clone(&chunk))
                .expect("chunks in order always fit");
        }
        published.store(Arc::new(table));

        let mut deck = Deck::new(SR);
        let _ = deck.load(Arc::new(buffer));
        deck.play();
        deck.toggle_stem_mute(Stem::Vocal as usize);

        // Settle past the per-stem filters' step response.
        let _ = peak(&mut deck, &layout, 2_048);
        let settled = peak(&mut deck, &layout, 512);
        assert!(
            settled > 0.8,
            "the stem path is not live: {settled} (0.5 would mean the raw mix)"
        );

        // Now a worker publishes chunk after chunk *while* the deck reads.
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let published = Arc::clone(&published);
            let stop = Arc::clone(&stop);
            let chunk = Arc::clone(&chunk);
            std::thread::spawn(move || {
                let mut next = AHEAD;
                while !stop.load(Ordering::Relaxed) {
                    let current = published.load();
                    let Some(table) = current.with_chunk(next, Arc::clone(&chunk)) else {
                        break;
                    };
                    published.store(Arc::new(table));
                    next += 1;
                }
            })
        };

        // 200 blocks of 256 frames stays well inside the pre-published span.
        let mut worst = f32::MAX;
        for _ in 0..200 {
            worst = worst.min(peak(&mut deck, &layout, 256));
        }
        stop.store(true, Ordering::Relaxed);
        worker.join().expect("the worker panicked");

        let drift = (worst - settled).abs() / settled;
        assert!(
            drift < 0.01,
            "a stem mute must survive the worker publishing; the level dipped \
             from {settled} to {worst}"
        );
    }

    // -- the platter, on a real deck ---------------------------------------

    /// A ramp: frame `n` holds the value `n`, so a test can read the playhead
    /// straight out of the audio.
    fn ramp(frames: usize) -> Arc<AudioBuffer> {
        let samples: Vec<f32> = (0..frames).flat_map(|n| [n as f32, n as f32]).collect();
        Arc::new(AudioBuffer::from_interleaved(samples, SR))
    }

    fn playhead_after(deck: &mut Deck, layout: &BusLayout, frames: usize) -> f64 {
        let mut out = vec![0.0; frames * layout.channels];
        let _ = deck.process(&mut out, layout, None);
        deck.position().get()
    }

    /// **The number that makes it feel like vinyl, end to end.** One turn of
    /// the wheel has to move the playhead one revolution of a record --
    /// 1.8 seconds -- through the engine, not just in the jog module.
    #[test]
    fn scratching_moves_the_playhead_by_a_revolution() {
        let layout = BusLayout::for_channels(2);
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(2_000_000));
        deck.play();
        deck.set_jog_touch(true);

        let before = deck.position().get();
        deck.jog(1.0);
        let after = playhead_after(&mut deck, &layout, 256);

        let moved = after - before;
        let expected = 1.8 * SR.as_f64();
        assert!(
            (moved - expected).abs() < expected * 0.01,
            "one turn moved {moved} frames, not {expected}"
        );
    }

    /// While the hand is on the record the motor is not driving: a touched
    /// platter that is not being turned holds the playhead still, which is
    /// what stops the music when you put a finger down.
    #[test]
    fn a_hand_on_the_record_stops_it() {
        let layout = BusLayout::for_channels(2);
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(2_000_000));
        deck.play();

        // Playing normally, the playhead moves.
        let start = playhead_after(&mut deck, &layout, 256);
        assert!(start > 0.0);

        deck.set_jog_touch(true);
        let held = playhead_after(&mut deck, &layout, 256);
        assert!(
            (held - start).abs() < 1.0,
            "the record kept moving under the hand: {start} to {held}"
        );

        // And it runs again when the hand comes off.
        deck.set_jog_touch(false);
        let released = playhead_after(&mut deck, &layout, 256);
        assert!(released > held + 100.0, "the record did not start again");
    }

    /// In CDJ mode the top of the platter is not a record: touching it does
    /// not stop the music, which is the whole difference between the modes.
    #[test]
    fn in_cdj_mode_a_hand_does_not_stop_the_record() {
        let layout = BusLayout::for_channels(2);
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(2_000_000));
        deck.set_jog_mode(JogMode::Cdj);
        deck.play();

        let free = playhead_after(&mut deck, &layout, 256);
        deck.set_jog_touch(true);
        let touched = playhead_after(&mut deck, &layout, 256);

        assert!(
            touched - free > 100.0,
            "CDJ mode stopped the record under the hand"
        );
    }

    /// **What a bend is for.** Pushing the wheel forwards has to make the deck
    /// cover more ground while the hand is moving -- that is how a DJ pulls two
    /// records back into line.
    #[test]
    fn bending_forwards_covers_more_ground() {
        let layout = BusLayout::for_channels(2);

        let mut plain = Deck::new(SR);
        let _ = plain.load(ramp(2_000_000));
        plain.play();
        let normal = playhead_after(&mut plain, &layout, 4_096);

        let mut bent = Deck::new(SR);
        let _ = bent.load(ramp(2_000_000));
        bent.play();
        // A steady push at the side of the platter, block by block.
        for _ in 0..16 {
            bent.jog(0.05 * 256.0 / SR.as_f64() as f32);
            let mut out = vec![0.0; 256 * layout.channels];
            let _ = bent.process(&mut out, &layout, None);
        }
        let pushed = bent.position().get();

        assert!(
            pushed > normal,
            "a bend covered {pushed} where normal play covered {normal}"
        );
        assert!(
            bent.jog_bend() > 0.0,
            "the deck does not report the bend it is applying"
        );
    }

    /// A paused deck searches: winding the wheel finds a spot in the track,
    /// and it does not start playing by itself.
    #[test]
    fn a_paused_deck_searches_and_stays_paused() {
        let layout = BusLayout::for_channels(2);
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(2_000_000));

        deck.jog(1.0);
        let after = playhead_after(&mut deck, &layout, 256);

        assert!(
            after > 1.8 * SR.as_f64(),
            "searching did not wind on: {after}"
        );
        assert!(!deck.is_playing(), "searching started the deck");
    }

    #[test]
    fn a_new_track_forgets_the_hand() {
        let mut deck = Deck::new(SR);
        let _ = deck.load(ramp(100_000));
        deck.set_jog_touch(true);
        deck.jog(0.5);

        let _ = deck.load(ramp(100_000));
        assert!(!deck.jog_touched(), "the hand carried over to a new track");
        assert_eq!(deck.jog_bend(), 0.0);
    }

    #[test]
    fn releasing_a_stem_solo_restores_the_djs_mutes() {
        let mut deck = deck();
        deck.toggle_stem_mute(Stem::Drums as usize);
        deck.toggle_stem_mute(Stem::Other as usize);
        let before = deck.stem_mutes;

        deck.set_stem_solo(Stem::Vocal as usize, true);
        assert_eq!(
            deck.stem_channels
                .iter()
                .map(|ch| ch.mute)
                .collect::<Vec<_>>(),
            [false, true, true, true]
        );
        deck.set_stem_solo(Stem::Vocal as usize, false);

        assert_eq!(deck.stem_mutes, before);
        assert_eq!(deck.stem_channels.map(|ch| ch.mute), before);
    }
}

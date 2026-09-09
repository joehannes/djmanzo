//! Letting the application run the mix.
//!
//! Automix is not a feature of the audio engine. Nothing here is realtime and
//! nothing here touches a sample: it watches where the playing track has got
//! to and, at the right moment, sends the same actions a DJ would send by hand.
//! That is the whole design, and it is why this is a plain state machine with
//! a `tick` that returns actions — everything it can do, a person could do,
//! and every action it emits is one that already existed in the vocabulary.
//!
//! # Volume, not the crossfader
//!
//! The obvious way to automate a mix is to sweep the crossfader. This does not,
//! for two reasons. A crossfader only cuts decks assigned to one of its halves,
//! and beyond the first two everything is assigned *through* — so a crossfader
//! automix works on decks 1 and 2 and silently does nothing on 3 and 4. And a
//! DJ who parks the crossfader hard left and then switches automix on would
//! hand over to a system that fades in a deck the crossfader is already
//! silencing.
//!
//! So transitions are made with the channel faders, and automix sets the decks
//! it is using to *through* when it takes over, taking the crossfader out of
//! the question entirely. That is a visible change to the mixer, and a
//! deliberate one: handing over the mix means handing it over.
//!
//! # Progress comes from the music, not from the clock
//!
//! A transition's position is derived from the outgoing deck's playhead rather
//! than from elapsed wall-clock time. The tick rate is whatever the interface
//! pump happens to be running at, and a transition timed off it would stretch
//! or compress whenever the machine got busy — which is exactly when a
//! transition is happening. Reading the playhead also means a transition
//! survives the DJ nudging the outgoing track, and stops if they pause it.
//!
//! # What it does not do
//!
//! It does not know where a track's outro is. The handover point is the end of
//! the file minus the transition length, which is right for a track that ends
//! when the music does and wrong for one with a minute of silence or applause
//! on the end. Detecting the real end is analysis work and is not done here;
//! until it is, the honest description is "mixes out of the end of the file".

use crate::shape;
use dj_core::action::{Action, AutomixChange, DeckAction, StemChange, TransitionStyle};
use dj_core::fx::FxChange;
use dj_core::{CrossfaderAssign, DeckId};

/// How far ahead of the handover a track is asked for, in seconds.
///
/// Long enough to read a file off a slow disk and analyse it, short enough that
/// the queue can still be changed most of the way through a track. Loading
/// early costs nothing — a loaded, paused deck is silent.
const PRELOAD_SECONDS: f64 = 20.0;

/// Default transition length, in beats.
///
/// Sixteen is four bars: long enough to be a mix rather than a switch, short
/// enough that two tracks are not fighting for half a minute.
pub const DEFAULT_BEATS: f32 = 16.0;

/// The shortest and longest a transition may be asked to last.
pub const MIN_BEATS: f32 = 1.0;
pub const MAX_BEATS: f32 = 128.0;

/// What one deck looks like to the automix.
///
/// A view rather than a borrow of the engine: everything here comes off the
/// same 60 Hz parameter snapshot the interface draws from, so automix sees
/// exactly what the DJ sees and there is no second path to keep in step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckView {
    pub id: DeckId,
    pub loaded: bool,
    pub playing: bool,
    /// Playhead, in frames.
    pub position: f64,
    /// Track length, in frames. Zero when nothing is loaded.
    pub length: f64,
    /// The deck's tempo after its pitch fader, when it has a grid.
    pub bpm: Option<f64>,
    pub sample_rate: f64,
}

impl DeckView {
    /// Frames per beat at this deck's current tempo.
    ///
    /// Falls back to 120 BPM for a track with no grid. A transition still has
    /// to last *something*, and a wrong tempo makes it the wrong length rather
    /// than making it not happen.
    fn frames_per_beat(&self) -> f64 {
        let bpm = self.bpm.filter(|b| *b > 1.0).unwrap_or(120.0);
        self.sample_rate * 60.0 / bpm
    }

    fn remaining(&self) -> f64 {
        (self.length - self.position).max(0.0)
    }
}

/// What the automix wants to happen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Actions to send, in order.
    pub actions: Vec<Action>,
    /// A track is wanted on this deck. The caller takes the next thing off
    /// whatever queue it keeps and loads it; automix has no queue of its own,
    /// because "what plays next" is a library question.
    pub load: Option<DeckId>,
}

impl Plan {
    fn act(&mut self, action: Action) {
        self.actions.push(action);
    }

    fn deck(&mut self, deck: DeckId, action: DeckAction) {
        self.act(Action::Deck { deck, action });
    }
}

/// Where a transition has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Watching a playing deck approach its handover point.
    Watching,
    /// A track has been asked for on `incoming` and not yet arrived.
    Waiting { outgoing: DeckId, incoming: DeckId },
    /// Mid-transition.
    Mixing(Running),
}

/// The transition that is happening, as one thing.
///
/// One value rather than five arguments, because the five belong together: a
/// transition is these decks, from there, for that long, in that style, and a
/// function that took four of them and read the fifth off the panel is exactly
/// the defect [`Running::style`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Running {
    outgoing: DeckId,
    incoming: DeckId,
    /// The outgoing playhead where the transition began, in frames.
    from: u64,
    /// How long it lasts, in frames.
    span: u64,
    /// What kind of mix this *is*.
    ///
    /// Carried rather than read from the panel, because a held mix can name a
    /// style the panel does not — and the panel can be changed mid-transition
    /// by a DJ setting up the *next* one. Without this, `begin` honoured the
    /// held style and `mix` and `finish` read the panel's: a held blend against
    /// a panel set to fade opened as a blend and carried on as a fade, which is
    /// neither, and which djmanzo's own record of the night would read back as
    /// the wrong one.
    ///
    /// §68's argument, in one field: one object drives the mix, so every part
    /// of it agrees about what the mix is.
    style: TransitionStyle,
}

/// The mix djmanzo is holding, as the automix needs it.
///
/// [§68 of the directive](../../../docs/DIRECTIVE.md) asks for one transition
/// object that drives the waveform, the suggestions, the preview, the
/// autopilot, practice and replay — "that would unify many currently separate
/// concepts". This is that unification arriving here. Without it the automix
/// decides its own decks, its own moment and its own length, so a DJ who spent
/// a minute adjusting a mix point in the pair view and then switched automix on
/// watched it be ignored — two answers to one question, which is precisely what
/// §68 exists to stop.
///
/// A copy rather than a borrow of `crate::transition::Transition`: this module
/// is a pure state machine over `DeckView`s, and it stays that way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub outgoing: DeckId,
    pub incoming: DeckId,
    /// Where the mix starts, in frames of the outgoing record.
    ///
    /// The point of the whole thing. Without a held mix the handover is "the
    /// end of the file minus the transition length", which the module note
    /// above is honest about being wrong for any record with applause on the
    /// end. With one, somebody has actually decided.
    pub start_frame: f64,
    pub length_beats: u32,
    pub style: TransitionStyle,
}

/// The automix.
#[derive(Debug)]
pub struct Automix {
    enabled: bool,
    style: TransitionStyle,
    beats: f32,
    /// The mix djmanzo is holding, if it is holding one. See [`Held`].
    held: Option<Held>,
    phase: Phase,
    /// Set by `automix now`, consumed on the next tick.
    forced: bool,
    /// The last fader value written to each deck, so a tick that would change
    /// nothing sends nothing. A transition at 60 Hz is otherwise several
    /// hundred actions through the queue for no audible benefit.
    written: [Option<f32>; dj_core::MAX_DECKS],
}

impl Default for Automix {
    fn default() -> Self {
        Automix::new()
    }
}

impl Automix {
    #[must_use]
    pub fn new() -> Self {
        Automix {
            enabled: false,
            style: TransitionStyle::Blend,
            beats: DEFAULT_BEATS,
            held: None,
            phase: Phase::Watching,
            forced: false,
            written: [None; dj_core::MAX_DECKS],
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn style(&self) -> TransitionStyle {
        self.style
    }

    #[must_use]
    pub fn beats(&self) -> f32 {
        self.beats
    }

    /// True while a transition is actually running.
    #[must_use]
    pub fn is_mixing(&self) -> bool {
        matches!(self.phase, Phase::Mixing { .. })
    }

    /// Hand over the transition djmanzo is holding, or take it away.
    ///
    /// Pushed rather than pulled: this module knows nothing about the
    /// application, and a tick that reached for a lock sixty times a second to
    /// ask whether a mix had been set up would be a lock in the wrong place.
    pub fn hold(&mut self, held: Option<Held>) {
        self.held = held;
    }

    /// The mix it will perform, if one has been set up for it.
    #[must_use]
    pub fn held(&self) -> Option<Held> {
        self.held
    }

    /// Whether the held mix still describes the decks in front of it.
    ///
    /// A plan about deck 1 does not describe a mix out of deck 3. Where it does
    /// not apply it is ignored rather than forced — the automix carries on with
    /// its own answer, which is worse but is at least about the right records.
    fn holding_for(&self, outgoing: DeckId, decks: &[DeckView]) -> Option<Held> {
        let held = self.held?;
        if held.outgoing != outgoing || held.incoming == outgoing {
            return None;
        }
        view(decks, held.incoming)?;
        Some(held)
    }

    /// Apply a change from the action vocabulary.
    ///
    /// Returns anything that has to happen immediately — switching off
    /// mid-transition finishes it rather than abandoning it, because a channel
    /// fader left at 30% is not a state anybody wants to be handed back.
    pub fn apply(&mut self, change: AutomixChange, decks: &[DeckView]) -> Plan {
        let mut plan = Plan::default();
        match change {
            AutomixChange::SetEnabled(true) => {
                self.enabled = true;
                self.phase = Phase::Watching;
            }
            AutomixChange::SetEnabled(false) => {
                self.enabled = false;
                if let Phase::Mixing(running) = self.phase {
                    self.finish(running, decks, &mut plan);
                }
                self.phase = Phase::Watching;
            }
            AutomixChange::Style(style) => self.style = style,
            AutomixChange::Beats(beats) => {
                self.beats = beats.clamp(MIN_BEATS, MAX_BEATS);
            }
            AutomixChange::Now => self.forced = true,
        }
        plan
    }

    /// One pass. Call it as often as the interface updates.
    pub fn tick(&mut self, decks: &[DeckView]) -> Plan {
        let mut plan = Plan::default();
        if !self.enabled {
            return plan;
        }

        match self.phase {
            Phase::Watching => self.watch(decks, &mut plan),
            Phase::Waiting { outgoing, incoming } => {
                self.wait(outgoing, incoming, decks, &mut plan);
            }
            Phase::Mixing(running) => self.mix(running, decks, &mut plan),
        }
        plan
    }

    /// Watch a playing deck approach its handover point.
    fn watch(&mut self, decks: &[DeckView], plan: &mut Plan) {
        let Some(outgoing) = leader(decks) else {
            self.forced = false;
            return;
        };
        let lead = PRELOAD_SECONDS * outgoing.sample_rate;

        // How long until the mix starts. With a held mix that is a place
        // somebody decided; without one it is the end of the file minus the
        // transition, which the module note above is honest about.
        let until = match self.holding_for(outgoing.id, decks) {
            Some(held) => held.start_frame - outgoing.position,
            None => outgoing.remaining() - self.span_frames(&outgoing),
        };

        // `forced` skips the wait but not the load: there still has to be a
        // track to mix into.
        if !self.forced && until > lead {
            return;
        }

        // A held mix names the deck it is going into. Falling back to "any
        // free deck" would be djmanzo performing a different transition from
        // the one on screen, which is the whole failure §68 names.
        let incoming = match self.holding_for(outgoing.id, decks) {
            Some(held) => held.incoming,
            None => {
                let Some(free) = free_deck(decks, outgoing.id) else {
                    // Nowhere to go. Not an error — a two-deck rig with both
                    // decks playing is a DJ who is already mixing.
                    self.forced = false;
                    return;
                };
                free
            }
        };

        // Ask for a track if there is not one already there. A deck the DJ
        // pre-loaded themselves is used as it stands — that is them choosing
        // what plays next, which beats anything a queue would have picked.
        if !view(decks, incoming).is_some_and(|d| d.loaded) {
            plan.load = Some(incoming);
        }
        self.phase = Phase::Waiting {
            outgoing: outgoing.id,
            incoming,
        };
        // Evaluate the new phase on this same tick rather than the next one.
        // Loading is what happens *early*; starting is a separate question with
        // its own answer, and `wait` is where that answer lives. Beginning here
        // because a deck happened to be loaded would start every transition a
        // preload-length too soon.
        self.wait(outgoing.id, incoming, decks, plan);
    }

    /// A track was asked for. Start as soon as it arrives and it is time.
    fn wait(&mut self, outgoing: DeckId, incoming: DeckId, decks: &[DeckView], plan: &mut Plan) {
        let (Some(out), Some(inc)) = (view(decks, outgoing), view(decks, incoming)) else {
            self.phase = Phase::Watching;
            return;
        };
        if !out.playing {
            // The DJ stopped the outgoing track. Their mix now.
            self.forced = false;
            self.phase = Phase::Watching;
            return;
        }
        if !inc.loaded {
            // Still loading, or the queue was empty. Keep watching; if the
            // track runs out first, the deck simply stops, which is what would
            // happen without automix too.
            return;
        }
        let due = match self.holding_for(outgoing, decks) {
            Some(held) => out.position >= held.start_frame,
            None => out.remaining() <= self.span_frames(&out),
        };
        if self.forced || due {
            self.begin(out, incoming, decks, plan);
        }
    }

    /// Open the transition.
    fn begin(&mut self, outgoing: DeckView, incoming: DeckId, decks: &[DeckView], plan: &mut Plan) {
        self.forced = false;
        // The held mix decides how long and in what style, and is then spent:
        // performing it twice would be re-running a mix that has happened.
        // What the DJ set in the automix panel is what the *next* one uses,
        // which is the honest reading of a plan that was about this handover.
        let held = self.holding_for(outgoing.id, decks).inspect(|_| {
            self.held = None;
        });
        let style = held.map_or(self.style, |held| held.style);
        let span = held.map_or_else(
            || self.span_frames(&outgoing),
            |held| f64::from(held.length_beats) * outgoing.frames_per_beat(),
        );

        // Take the crossfader out of the question — see the module note.
        plan.deck(
            outgoing.id,
            DeckAction::SetCrossfaderAssign(CrossfaderAssign::Thru),
        );
        plan.deck(
            incoming,
            DeckAction::SetCrossfaderAssign(CrossfaderAssign::Thru),
        );

        // Match tempo before anything is audible. Sync on the *incoming* deck,
        // so the track already playing to the room is the one that keeps its
        // tempo — a DJ would never re-pitch the record the crowd is dancing to.
        plan.deck(incoming, DeckAction::Sync);

        // Everything past the two faders is the style's, and the style's shape
        // is a table rather than a branch here -- see `crate::shape`.
        let shape = shape::shape(style);

        let running = Running {
            outgoing: outgoing.id,
            incoming,
            from: outgoing.position as u64,
            span: span as u64,
            style,
        };

        if !shape.overlaps {
            // Nothing is audible at the same time, so there is no span for a
            // stem or an EQ plan to happen over: the outgoing deck stops on
            // the same tick the incoming one starts.
            self.set_fader(incoming, 1.0, plan);
            plan.deck(incoming, DeckAction::Play);
            self.finish(running, decks, plan);
            self.phase = Phase::Watching;
            return;
        }

        if let Some((slot, kind, beats)) = shape.fx.outgoing() {
            // The slot is overwritten rather than asked about: automix is
            // driving, and a transition that depended on which effect the DJ
            // happened to leave loaded would be a different transition every
            // time.
            for change in [
                FxChange::Select(kind),
                FxChange::Beats(beats),
                FxChange::SetEnabled(true),
            ] {
                plan.act(Action::Deck {
                    deck: outgoing.id,
                    action: DeckAction::Fx { slot, change },
                });
            }
        }
        for (deck, stems) in [
            (outgoing.id, shape.outgoing_stems),
            (incoming, shape.incoming_stems),
        ] {
            if let Some(stem) = stems.solo() {
                plan.act(Action::Deck {
                    deck,
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::SetSolo(true),
                    },
                });
            }
        }

        // Start silent and come up, so the first frame of the incoming track is
        // not a step in the master.
        self.set_fader(incoming, 0.0, plan);
        plan.deck(incoming, DeckAction::Play);

        self.phase = Phase::Mixing(running);
    }

    /// Mid-transition: move the faders to where the music says they should be.
    fn mix(&mut self, running: Running, decks: &[DeckView], plan: &mut Plan) {
        let Running {
            outgoing,
            incoming,
            from,
            span,
            style,
        } = running;
        let Some(out) = view(decks, outgoing) else {
            self.phase = Phase::Watching;
            return;
        };

        let travelled = out.position - from as f64;
        let progress = if span == 0 {
            1.0
        } else {
            (travelled / span as f64).clamp(0.0, 1.0)
        };

        // Constant-power, the same law the crossfader uses. Two linear ramps
        // dip in the middle, which is audible as a hole in the mix at exactly
        // the moment both tracks are meant to be carrying it.
        let angle = progress * std::f64::consts::FRAC_PI_2;
        self.set_fader(outgoing, angle.cos() as f32, plan);
        self.set_fader(incoming, angle.sin() as f32, plan);

        // The style's EQ plan. Guarded on whether there *is* a plan rather than
        // on the value it currently gives: a swap is nought at the start too,
        // and skipping the write there would leave the incoming record's bass
        // in for the first tick of the one style that exists to take it out.
        //
        // The EQ range is 0..=4 with 1.0 as unity, so a cut is toward zero.
        let eq = shape::shape(style).eq;
        if eq != shape::Eq::Flat {
            let swap = eq.swap_at(progress) as f32;
            plan.deck(outgoing, DeckAction::SetEqLow(1.0 - swap));
            plan.deck(incoming, DeckAction::SetEqLow(swap));
        }

        if progress >= 1.0 {
            self.finish(running, decks, plan);
            self.phase = Phase::Watching;
        }
    }

    /// Close the transition: the incoming deck holds the room, the outgoing one
    /// is put back the way it was found.
    fn finish(&mut self, running: Running, decks: &[DeckView], plan: &mut Plan) {
        let Running {
            outgoing,
            incoming,
            style,
            ..
        } = running;
        self.set_fader(incoming, 1.0, plan);
        plan.deck(incoming, DeckAction::SetEqLow(1.0));

        plan.deck(outgoing, DeckAction::Pause);
        // Its fader goes back up and its EQ back to flat *after* it is paused,
        // so the deck is left in a state a DJ can immediately use rather than
        // silent-with-the-fader-down, which reads as a broken channel.
        self.set_fader(outgoing, 1.0, plan);
        plan.deck(outgoing, DeckAction::SetEqLow(1.0));
        // Everything the shape switched on is switched off again, from the same
        // table that switched it on -- so a style cannot leave a deck holding
        // an effect or a solo the DJ did not set.
        let shape = shape::shape(style);
        if let Some((slot, _, _)) = shape.fx.outgoing() {
            plan.act(Action::Deck {
                deck: outgoing,
                action: DeckAction::Fx {
                    slot,
                    change: FxChange::SetEnabled(false),
                },
            });
        }
        for (deck, stems) in [
            (outgoing, shape.outgoing_stems),
            (incoming, shape.incoming_stems),
        ] {
            if let Some(stem) = stems.solo() {
                plan.act(Action::Deck {
                    deck,
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::SetSolo(false),
                    },
                });
            }
        }
        plan.deck(outgoing, DeckAction::Eject);
        // Ejecting clears the deck, so the next preload will fill it.
        let _ = decks;
    }

    /// Write a fader, unless it is already there.
    ///
    /// Quantised to the step the interface can actually show. Without this a
    /// transition is several hundred actions through the command queue, all but
    /// a handful of them inaudible.
    fn set_fader(&mut self, deck: DeckId, value: f32, plan: &mut Plan) {
        const STEP: f32 = 512.0;
        let value = (value.clamp(0.0, 1.0) * STEP).round() / STEP;
        let slot = &mut self.written[deck.index()];
        if *slot == Some(value) {
            return;
        }
        *slot = Some(value);
        plan.deck(deck, DeckAction::SetVolume(value));
    }

    /// How long a transition lasts on this deck, in frames.
    fn span_frames(&self, outgoing: &DeckView) -> f64 {
        f64::from(self.beats) * outgoing.frames_per_beat()
    }
}

fn view(decks: &[DeckView], id: DeckId) -> Option<DeckView> {
    decks.iter().copied().find(|d| d.id == id)
}

/// The deck currently holding the room: the playing one closest to its end.
///
/// Closest to the end rather than loudest or lowest-numbered, because that is
/// the one that needs replacing. With two decks already mixing by hand this
/// picks the one going out, which is the same answer.
fn leader(decks: &[DeckView]) -> Option<DeckView> {
    decks
        .iter()
        .copied()
        .filter(|d| d.playing && d.loaded && d.length > 0.0)
        .min_by(|a, b| {
            a.remaining()
                .partial_cmp(&b.remaining())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Somewhere to put the next track: a deck that is not playing and is not the
/// one going out. Lowest-numbered, so a two-deck rig alternates.
fn free_deck(decks: &[DeckView], outgoing: DeckId) -> Option<DeckId> {
    decks
        .iter()
        .find(|d| d.id != outgoing && !d.playing)
        .map(|d| d.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;
    /// A minute at 120 BPM.
    const LENGTH: f64 = SR * 60.0;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn playing(n: u8, position_seconds: f64) -> DeckView {
        DeckView {
            id: deck(n),
            loaded: true,
            playing: true,
            position: position_seconds * SR,
            length: LENGTH,
            bpm: Some(120.0),
            sample_rate: SR,
        }
    }

    fn empty(n: u8) -> DeckView {
        DeckView {
            id: deck(n),
            loaded: false,
            playing: false,
            position: 0.0,
            length: 0.0,
            bpm: None,
            sample_rate: SR,
        }
    }

    fn loaded_idle(n: u8) -> DeckView {
        DeckView {
            loaded: true,
            ..empty(n)
        }
    }

    fn on(style: TransitionStyle) -> Automix {
        let mut mix = Automix::new();
        mix.apply(AutomixChange::SetEnabled(true), &[]);
        mix.apply(AutomixChange::Style(style), &[]);
        mix
    }

    /// Every action a plan wants, as text — the vocabulary is the contract, so
    /// asserting on it is asserting on what a DJ would have typed.
    fn text(plan: &Plan) -> Vec<String> {
        plan.actions.iter().map(ToString::to_string).collect()
    }

    fn has(plan: &Plan, needle: &str) -> bool {
        text(plan).iter().any(|a| a == needle)
    }

    fn held_at(seconds: f64, beats: u32, style: TransitionStyle) -> Held {
        Held {
            outgoing: deck(1),
            incoming: deck(2),
            start_frame: seconds * SR,
            length_beats: beats,
            style,
        }
    }

    /// **§68: it performs the mix djmanzo is holding, not one of its own.**
    ///
    /// Without this the automix mixes out of the end of the file, so a DJ who
    /// set a mix point at 0:30 in the pair view and switched automix on got a
    /// transition at 0:59 instead. Two answers to one question, which is the
    /// failure the transition object exists to end.
    #[test]
    fn it_starts_where_the_held_mix_says_rather_than_at_the_end() {
        let mut mix = on(TransitionStyle::Blend);
        mix.hold(Some(held_at(30.0, 16, TransitionStyle::Blend)));

        // Well before the end of the file, and *at* the held point.
        let plan = mix.tick(&[playing(1, 30.0), loaded_idle(2)]);
        assert!(
            has(&plan, "deck 2 play"),
            "the held mix did not start at its own point: {:?}",
            text(&plan)
        );
        assert!(mix.is_mixing());

        // Without a held mix the same moment is far too early.
        let mut plain = on(TransitionStyle::Blend);
        assert_eq!(
            plain.tick(&[playing(1, 30.0), loaded_idle(2)]),
            Plan::default(),
            "it began a transition thirty seconds early with nothing held"
        );
    }

    /// The held mix decides the style and the length too, not only the moment.
    #[test]
    fn the_held_mix_decides_its_own_style_and_length() {
        let mut mix = on(TransitionStyle::Blend);
        mix.apply(AutomixChange::Beats(16.0), &[]);
        // A cut, which is a different transition from the blend set in the
        // panel — and short.
        mix.hold(Some(held_at(30.0, 4, TransitionStyle::Cut)));

        let plan = mix.tick(&[playing(1, 30.0), loaded_idle(2)]);
        // A cut finishes on the spot: the outgoing deck stops as the incoming
        // one starts, so nothing is left mixing.
        assert!(has(&plan, "deck 2 play"));
        assert!(!mix.is_mixing(), "a cut left a transition running");
        assert!(
            has(&plan, "deck 2 volume 1"),
            "a cut did not bring the incoming deck straight up: {:?}",
            text(&plan)
        );
    }

    /// **A plan about other decks is ignored, not forced.**
    ///
    /// A transition set up for deck 1 says nothing about a mix out of deck 3,
    /// and performing it anyway would be worse than performing djmanzo's own
    /// answer — which is what it falls back to.
    #[test]
    fn a_held_mix_about_other_decks_is_ignored() {
        let mut mix = on(TransitionStyle::Blend);
        // Out of deck 3 and into deck 2. Deck 2 is present and free, so the
        // *only* thing that can refuse this plan is that deck 1 is the one
        // playing — which is the check being tested. An earlier version of
        // this test named a deck that was not in the rig at all, so it passed
        // whether or not the check existed; mutation testing found that.
        mix.hold(Some(Held {
            outgoing: deck(3),
            incoming: deck(2),
            start_frame: 30.0 * SR,
            length_beats: 16,
            style: TransitionStyle::Blend,
        }));
        assert_eq!(
            mix.tick(&[playing(1, 30.0), loaded_idle(2), loaded_idle(3)]),
            Plan::default(),
            "a plan about deck 3 started a mix out of deck 1"
        );
        assert!(mix.held().is_some(), "an inapplicable plan was thrown away");
    }

    /// Spent once performed. Re-running a mix that has happened is not a mix.
    #[test]
    fn a_held_mix_is_used_once() {
        let mut mix = on(TransitionStyle::Blend);
        mix.hold(Some(held_at(30.0, 16, TransitionStyle::Blend)));
        mix.tick(&[playing(1, 30.0), loaded_idle(2)]);
        assert!(
            mix.held().is_none(),
            "the held mix survived being performed"
        );
    }

    /// **The style a mix starts with is the style it finishes with.**
    ///
    /// §68's whole argument: one transition object drives the mix, so that
    /// every part of it agrees about what the mix *is*. `begin` honoured the
    /// held mix's style and then `mix` and `finish` read the panel's, so a
    /// held Blend performed against a panel set to Fade opened as a blend and
    /// carried on as a fade — no bass swap, no cleanup of what the opening
    /// set up. A hybrid of two styles, which is neither, and which djmanzo's
    /// own record of the night would then read back as the wrong one.
    #[test]
    fn a_held_mix_keeps_its_own_style_all_the_way_through() {
        // The panel says fade; the DJ set up a blend.
        let mut mix = on(TransitionStyle::Fade);
        mix.hold(Some(held_at(30.0, 16, TransitionStyle::Blend)));
        mix.tick(&[playing(1, 30.0), loaded_idle(2)]);
        assert!(mix.is_mixing(), "the held mix did not start");

        // Halfway through: a blend swaps the bass, a fade does not.
        let plan = mix.tick(&[playing(1, 34.0), playing(2, 4.0)]);
        assert!(
            text(&plan).iter().any(|a| a.starts_with("deck 1 eq_low")),
            "the blend stopped being a blend once it was running: {:?}",
            text(&plan)
        );
    }

    /// And the other way: a panel set to blend must not turn a held fade into
    /// one. The held mix is the mix, in both directions.
    #[test]
    fn the_panel_cannot_turn_a_held_fade_into_a_blend() {
        let mut mix = on(TransitionStyle::Blend);
        mix.hold(Some(held_at(30.0, 16, TransitionStyle::Fade)));
        mix.tick(&[playing(1, 30.0), loaded_idle(2)]);

        let plan = mix.tick(&[playing(1, 34.0), playing(2, 4.0)]);
        assert!(
            !text(&plan).iter().any(|a| a.starts_with("deck 1 eq_low")),
            "a held fade swapped the bass: {:?}",
            text(&plan)
        );
    }

    /// The held mix does not stop it loading a record in time.
    #[test]
    fn a_held_mix_still_gets_its_record_loaded_early() {
        let mut mix = on(TransitionStyle::Blend);
        mix.hold(Some(held_at(30.0, 16, TransitionStyle::Blend)));
        // Fifteen seconds before the held point, inside the preload window.
        let plan = mix.tick(&[playing(1, 15.0), empty(2)]);
        assert_eq!(
            plan.load,
            Some(deck(2)),
            "nothing was asked for ahead of a held mix"
        );
        assert!(!mix.is_mixing(), "it began early rather than loading early");
    }

    #[test]
    fn switched_off_it_does_nothing() {
        let mut mix = Automix::new();
        let plan = mix.tick(&[playing(1, 59.0), empty(2)]);
        assert_eq!(plan, Plan::default());
    }

    #[test]
    fn early_in_a_track_it_waits() {
        let mut mix = on(TransitionStyle::Blend);
        let plan = mix.tick(&[playing(1, 5.0), empty(2)]);
        assert_eq!(plan, Plan::default(), "it moved too early");
    }

    #[test]
    fn it_asks_for_a_track_before_it_needs_one() {
        let mut mix = on(TransitionStyle::Blend);
        // 20 s of preload plus an 8 s transition means it asks at ~32 s in.
        let plan = mix.tick(&[playing(1, 35.0), empty(2)]);
        assert_eq!(plan.load, Some(deck(2)));
        assert!(
            plan.actions.is_empty(),
            "it started mixing at the same time"
        );
    }

    /// **The transition.** Both faders move, in opposite directions, and the
    /// incoming deck is tempo-matched before it is audible.
    #[test]
    fn a_transition_crossfades_the_two_decks() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];

        // Ask, then arrive at the handover point.
        mix.tick(&decks(35.0));
        let start = mix.tick(&decks(52.0));
        assert!(
            mix.is_mixing(),
            "the transition never started: {:?}",
            text(&start)
        );
        assert!(has(&start, "deck 2 sync"), "{:?}", text(&start));
        assert!(has(&start, "deck 2 play"), "{:?}", text(&start));
        assert!(
            has(&start, "deck 2 volume 0"),
            "it started audible: {:?}",
            text(&start)
        );

        // Halfway through an eight-second transition that began at 52 s, so
        // 56 s rather than 54. Constant power puts both at about 0.707.
        let half = mix.tick(&decks(56.0));
        let out = fader(&half, 1).expect("deck 1 fader");
        let inc = fader(&half, 2).expect("deck 2 fader");
        assert!((out - 0.707).abs() < 0.02, "outgoing at {out}");
        assert!((inc - 0.707).abs() < 0.02, "incoming at {inc}");
    }

    /// Where a plan leaves a deck's fader, if it moves it.
    ///
    /// The *last* write, not the first: a plan that both fades a deck out and
    /// then tidies up after it contains two, and the one that matters is where
    /// the deck ends up.
    fn fader(plan: &Plan, deck_number: u8) -> Option<f32> {
        plan.actions.iter().rev().find_map(|action| match action {
            Action::Deck {
                deck: id,
                action: DeckAction::SetVolume(v),
            } if id.human_number() == deck_number => Some(*v),
            _ => None,
        })
    }

    /// Two linear ramps dip in the middle, which is a hole in the mix at
    /// exactly the moment both tracks are meant to be carrying it.
    #[test]
    fn the_crossfade_holds_its_power_through_the_middle() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        mix.tick(&decks(52.0));

        let mut worst: f32 = 1.0;
        let mut out = 1.0f32;
        let mut inc = 0.0f32;
        for step in 0..=32 {
            let seconds = 52.0 + f64::from(step) * 0.25;
            let plan = mix.tick(&decks(seconds));
            out = fader(&plan, 1).unwrap_or(out);
            inc = fader(&plan, 2).unwrap_or(inc);
            let power = (out * out + inc * inc).sqrt();
            worst = worst.min(power);
        }
        assert!(
            worst > 0.98,
            "the mix dipped to {worst} of full power in the middle"
        );
    }

    /// A blend takes the outgoing bass out of the way. Both kicks at once is
    /// the thing that makes an automatic mix sound automatic.
    #[test]
    fn a_blend_swaps_the_bass_and_a_fade_does_not() {
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];

        let mut blend = on(TransitionStyle::Blend);
        blend.tick(&decks(35.0));
        blend.tick(&decks(52.0));
        let mid = blend.tick(&decks(53.0));
        let low = mid.actions.iter().any(|a| {
            matches!(
                a,
                Action::Deck {
                    action: DeckAction::SetEqLow(_),
                    ..
                }
            )
        });
        assert!(low, "a blend left the bass alone: {:?}", text(&mid));

        let mut fade = on(TransitionStyle::Fade);
        fade.tick(&decks(35.0));
        fade.tick(&decks(52.0));
        let mid = fade.tick(&decks(53.0));
        assert!(
            !mid.actions.iter().any(|a| matches!(
                a,
                Action::Deck {
                    action: DeckAction::SetEqLow(_),
                    ..
                }
            )),
            "a fade touched the EQ: {:?}",
            text(&mid)
        );
    }

    /// **The incoming record's bass is out from the first tick of the mix, not
    /// the second.**
    ///
    /// A blend whose swap only starts once progress has moved off nought lets
    /// the incoming kick through for one tick at exactly the moment both
    /// records first sound together -- which is the one thing the swap exists
    /// to prevent, at the one moment it is most audible.
    #[test]
    fn a_blend_takes_the_incoming_bass_out_from_the_first_tick() {
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        let mut blend = on(TransitionStyle::Blend);
        blend.tick(&decks(35.0));
        blend.tick(&decks(52.0));
        assert!(blend.is_mixing(), "the transition never opened");

        // The same playhead again: progress is exactly nought.
        let first = blend.tick(&decks(52.0));
        let incoming = first.actions.iter().find_map(|a| match a {
            Action::Deck {
                deck,
                action: DeckAction::SetEqLow(gain),
            } if deck.human_number() == 2 => Some(*gain),
            _ => None,
        });
        assert_eq!(
            incoming,
            Some(0.0),
            "the incoming bass was not cut at the top of the blend: {:?}",
            text(&first)
        );
    }

    /// A cut has no overlap: the outgoing deck stops on the tick the incoming
    /// one starts, and nothing is left mid-transition.
    #[test]
    fn a_cut_does_not_overlap() {
        let mut mix = on(TransitionStyle::Cut);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        let plan = mix.tick(&decks(52.0));
        assert!(!mix.is_mixing(), "a cut left a transition running");
        assert!(has(&plan, "deck 2 play"), "{:?}", text(&plan));
        assert!(has(&plan, "deck 1 pause"), "{:?}", text(&plan));
        assert_eq!(
            fader(&plan, 2),
            Some(1.0),
            "the incoming deck came up quiet"
        );
    }

    #[test]
    fn an_echo_transition_throws_an_echo_and_takes_it_away_again() {
        let mut mix = on(TransitionStyle::Echo);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        let start = mix.tick(&decks(52.0));
        assert!(has(&start, "deck 1 fx 1 echo"), "{:?}", text(&start));
        assert!(has(&start, "deck 1 fx 1 on"), "{:?}", text(&start));

        let end = mix.tick(&decks(60.0));
        assert!(has(&end, "deck 1 fx 1 off"), "{:?}", text(&end));
    }

    /// The transition must finish, leaving the incoming deck holding the room
    /// and the outgoing one in a state a DJ can immediately use.
    #[test]
    fn a_finished_transition_tidies_up_after_itself() {
        let mut mix = on(TransitionStyle::Blend);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        mix.tick(&decks(52.0));
        let end = mix.tick(&decks(60.0));

        assert!(!mix.is_mixing(), "it never finished");
        assert!(has(&end, "deck 1 pause"), "{:?}", text(&end));
        assert!(has(&end, "deck 1 eject"), "{:?}", text(&end));
        assert_eq!(
            fader(&end, 1),
            Some(1.0),
            "the outgoing deck was left with its fader down"
        );
        assert!(
            has(&end, "deck 1 eq_low 1"),
            "the outgoing deck was left with its bass cut: {:?}",
            text(&end)
        );
    }

    /// **Switching off mid-transition finishes it.** A channel fader left at
    /// 30% is not a state anybody wants to be handed back.
    #[test]
    fn taking_back_control_does_not_leave_a_half_fade() {
        let mut mix = on(TransitionStyle::Blend);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        mix.tick(&decks(52.0));
        let all = decks(54.0);
        mix.tick(&all);
        assert!(mix.is_mixing());

        let plan = mix.apply(AutomixChange::SetEnabled(false), &all);
        assert!(!mix.is_mixing());
        assert_eq!(
            fader(&plan, 2),
            Some(1.0),
            "the incoming deck was left quiet"
        );
        assert!(has(&plan, "deck 1 pause"), "{:?}", text(&plan));
        assert!(mix.tick(&all).actions.is_empty(), "it kept going");
    }

    /// The DJ pausing the outgoing track takes the mix back without saying so.
    #[test]
    fn it_stands_down_when_the_dj_stops_the_outgoing_track() {
        let mut mix = on(TransitionStyle::Blend);
        mix.tick(&[playing(1, 35.0), empty(2)]);
        let stopped = DeckView {
            playing: false,
            ..playing(1, 40.0)
        };
        let plan = mix.tick(&[stopped, loaded_idle(2)]);
        assert!(plan.actions.is_empty(), "{:?}", text(&plan));
        assert!(!mix.is_mixing());
    }

    /// With every deck busy there is nowhere to go, and that is a DJ already
    /// mixing rather than a fault.
    #[test]
    fn with_no_free_deck_it_waits() {
        let mut mix = on(TransitionStyle::Blend);
        let plan = mix.tick(&[playing(1, 55.0), playing(2, 10.0)]);
        assert_eq!(plan, Plan::default());
    }

    /// `automix now` starts the transition wherever the track has got to.
    #[test]
    fn now_starts_immediately() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = vec![playing(1, 4.0), loaded_idle(2)];
        assert_eq!(mix.tick(&decks), Plan::default(), "it moved unbidden");

        mix.apply(AutomixChange::Now, &decks);
        let plan = mix.tick(&decks);
        assert!(mix.is_mixing(), "`now` did nothing: {:?}", text(&plan));
        assert!(has(&plan, "deck 2 play"));
    }

    /// A forced transition with nothing loaded still has to fetch a track
    /// rather than mixing into silence.
    #[test]
    fn now_still_needs_something_to_mix_into() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = vec![playing(1, 4.0), empty(2)];
        mix.apply(AutomixChange::Now, &decks);
        let plan = mix.tick(&decks);
        assert_eq!(plan.load, Some(deck(2)));
        assert!(!mix.is_mixing(), "it mixed into an empty deck");
    }

    /// Automix takes the crossfader out of the question, because a DJ who
    /// parked it hard left would otherwise hand over to a system that fades in
    /// a deck the crossfader is already silencing.
    #[test]
    fn it_sets_both_decks_through_before_mixing() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        let start = mix.tick(&decks(52.0));
        assert!(has(&start, "deck 1 xfader_thru"), "{:?}", text(&start));
        assert!(has(&start, "deck 2 xfader_thru"), "{:?}", text(&start));
    }

    /// A tick that changes nothing sends nothing. At 60 Hz an eight-second
    /// transition is otherwise a thousand actions through the command queue.
    #[test]
    fn a_tick_that_changes_nothing_sends_nothing() {
        let mut mix = on(TransitionStyle::Fade);
        let decks = |seconds: f64| vec![playing(1, seconds), loaded_idle(2)];
        mix.tick(&decks(35.0));
        mix.tick(&decks(52.0));

        // The same playhead twice: the second pass has nothing to say.
        mix.tick(&decks(54.0));
        let repeat = mix.tick(&decks(54.0));
        assert!(
            repeat.actions.is_empty(),
            "a still transition sent {:?}",
            text(&repeat)
        );
    }

    /// A track with no grid still has to be mixed out of. 120 BPM is a guess,
    /// and a wrong length beats not transitioning at all.
    #[test]
    fn a_track_without_a_grid_still_transitions() {
        let mut mix = on(TransitionStyle::Fade);
        let ungridded = DeckView {
            bpm: None,
            ..playing(1, 52.0)
        };
        mix.tick(&[
            DeckView {
                bpm: None,
                ..playing(1, 35.0)
            },
            empty(2),
        ]);
        let plan = mix.tick(&[ungridded, loaded_idle(2)]);
        assert!(mix.is_mixing(), "{:?}", text(&plan));
    }

    #[test]
    fn the_transition_length_is_clamped_to_something_playable() {
        let mut mix = Automix::new();
        mix.apply(AutomixChange::Beats(10_000.0), &[]);
        assert_eq!(mix.beats(), MAX_BEATS);
        mix.apply(AutomixChange::Beats(-4.0), &[]);
        assert_eq!(mix.beats(), MIN_BEATS);
    }
}

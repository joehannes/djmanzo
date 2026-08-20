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

use dj_core::action::{Action, AutomixChange, DeckAction, TransitionStyle};
use dj_core::fx::{EffectKind, FxChange};
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
    Mixing {
        outgoing: DeckId,
        incoming: DeckId,
        /// The outgoing playhead where the transition began, in frames.
        from: u64,
        /// How long it lasts, in frames.
        span: u64,
    },
}

/// The automix.
#[derive(Debug)]
pub struct Automix {
    enabled: bool,
    style: TransitionStyle,
    beats: f32,
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
                if let Phase::Mixing {
                    outgoing, incoming, ..
                } = self.phase
                {
                    self.finish(outgoing, incoming, decks, &mut plan);
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
            Phase::Mixing {
                outgoing,
                incoming,
                from,
                span,
            } => self.mix(outgoing, incoming, from, span, decks, &mut plan),
        }
        plan
    }

    /// Watch a playing deck approach its handover point.
    fn watch(&mut self, decks: &[DeckView], plan: &mut Plan) {
        let Some(outgoing) = leader(decks) else {
            self.forced = false;
            return;
        };
        let span = self.span_frames(&outgoing);
        let lead = PRELOAD_SECONDS * outgoing.sample_rate;

        // `forced` skips the wait but not the load: there still has to be a
        // track to mix into.
        if !self.forced && outgoing.remaining() > span + lead {
            return;
        }

        let Some(incoming) = free_deck(decks, outgoing.id) else {
            // Nowhere to go. Not an error — a two-deck rig with both decks
            // playing is a DJ who is already mixing.
            self.forced = false;
            return;
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
        if self.forced || out.remaining() <= self.span_frames(&out) {
            self.begin(out, incoming, decks, plan);
        }
    }

    /// Open the transition.
    fn begin(&mut self, outgoing: DeckView, incoming: DeckId, decks: &[DeckView], plan: &mut Plan) {
        self.forced = false;
        let span = self.span_frames(&outgoing);

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

        match self.style {
            TransitionStyle::Cut => {
                // No overlap at all: the outgoing deck stops on the same tick
                // the incoming one starts.
                self.set_fader(incoming, 1.0, plan);
                plan.deck(incoming, DeckAction::Play);
                self.finish(outgoing.id, incoming, decks, plan);
                self.phase = Phase::Watching;
                return;
            }
            TransitionStyle::Echo => {
                // Slot 1 of the outgoing deck's rack, thrown as it leaves. The
                // slot is overwritten rather than asked about: automix is
                // driving, and a transition that depended on which effect the
                // DJ happened to leave loaded would be a different transition
                // every time.
                plan.act(Action::Deck {
                    deck: outgoing.id,
                    action: DeckAction::Fx {
                        slot: 1,
                        change: FxChange::Select(EffectKind::Echo),
                    },
                });
                plan.act(Action::Deck {
                    deck: outgoing.id,
                    action: DeckAction::Fx {
                        slot: 1,
                        change: FxChange::Beats(4.0),
                    },
                });
                plan.act(Action::Deck {
                    deck: outgoing.id,
                    action: DeckAction::Fx {
                        slot: 1,
                        change: FxChange::SetEnabled(true),
                    },
                });
            }
            TransitionStyle::Fade | TransitionStyle::Blend => {}
        }

        // Start silent and come up, so the first frame of the incoming track is
        // not a step in the master.
        self.set_fader(incoming, 0.0, plan);
        plan.deck(incoming, DeckAction::Play);

        self.phase = Phase::Mixing {
            outgoing: outgoing.id,
            incoming,
            from: outgoing.position as u64,
            span: span as u64,
        };
    }

    /// Mid-transition: move the faders to where the music says they should be.
    fn mix(
        &mut self,
        outgoing: DeckId,
        incoming: DeckId,
        from: u64,
        span: u64,
        decks: &[DeckView],
        plan: &mut Plan,
    ) {
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

        if self.style == TransitionStyle::Blend {
            // The bass swap. Both kicks at once is the thing that makes an
            // automatic mix sound automatic, so the outgoing low end comes out
            // over the first half and the incoming one arrives over it.
            //
            // The EQ range is 0..=4 with 1.0 as unity, so a cut is toward zero.
            let swap = (progress * 2.0).clamp(0.0, 1.0) as f32;
            plan.deck(outgoing, DeckAction::SetEqLow(1.0 - swap));
            plan.deck(incoming, DeckAction::SetEqLow(swap));
        }

        if progress >= 1.0 {
            self.finish(outgoing, incoming, decks, plan);
            self.phase = Phase::Watching;
        }
    }

    /// Close the transition: the incoming deck holds the room, the outgoing one
    /// is put back the way it was found.
    fn finish(&mut self, outgoing: DeckId, incoming: DeckId, decks: &[DeckView], plan: &mut Plan) {
        self.set_fader(incoming, 1.0, plan);
        plan.deck(incoming, DeckAction::SetEqLow(1.0));

        plan.deck(outgoing, DeckAction::Pause);
        // Its fader goes back up and its EQ back to flat *after* it is paused,
        // so the deck is left in a state a DJ can immediately use rather than
        // silent-with-the-fader-down, which reads as a broken channel.
        self.set_fader(outgoing, 1.0, plan);
        plan.deck(outgoing, DeckAction::SetEqLow(1.0));
        if self.style == TransitionStyle::Echo {
            plan.act(Action::Deck {
                deck: outgoing,
                action: DeckAction::Fx {
                    slot: 1,
                    change: FxChange::SetEnabled(false),
                },
            });
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

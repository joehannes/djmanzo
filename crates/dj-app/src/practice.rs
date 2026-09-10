//! Trying a mix without playing it to the room.
//!
//! §69 asks for a practice surface where "two tracks can be explored without
//! altering the live master". The words that matter are the last four. A
//! practice lab that borrows the decks is not one — a DJ who has to stop
//! playing in order to rehearse the next thing has a booth, not a sandbox.
//!
//! # It is a render, not a second pair of decks
//!
//! djmanzo already has a way to hear a mix that is not happening: replay
//! drives a headless engine against a frame counter rather than a sound card
//! ([`crate::replay`]). It was built to hear a set back. Point it at a set
//! that was never played and it becomes a rehearsal — nothing is allocated in
//! the live engine, no fader moves, and the DJ's records keep playing to the
//! room the whole time.
//!
//! So a rehearsal here is a [`crate::session::Session`]: a list of actions
//! with timestamps, exactly the shape a real night is recorded in. It can be
//! rendered to a WAV, diffed against another, or read as text.
//!
//! # The automix writes it, so what you rehearse is what you would get
//!
//! The obvious way to build the action list is to write out the crossfade
//! here: two faders, a cosine, an EQ swap. That would be a *second*
//! implementation of the transition, and the day the automix changed, the lab
//! would keep confidently rehearsing the old one — the same failure
//! [`crate::shape`] exists to prevent, one level up.
//!
//! Instead the real [`crate::automix::Automix`] is handed the transition and
//! stepped over a simulated playhead, and the actions it emits *are* the
//! rehearsal. It is the same state machine, holding the same mix, taking the
//! same table of what a style does. A rehearsal is therefore a promise about
//! the automix and not merely about this module.
//!
//! # Why a rehearsal is cheap and a replay is not
//!
//! [`crate::replay::Window`] renders everything before the window and throws
//! it away, because the engine's state at a moment is the whole set up to it.
//! A rehearsal has no such history: it is a synthetic set that *starts* with
//! the outgoing record seeked to a few seconds before the mix. There is
//! nothing before it to be faithful to, so there is nothing to render. Hearing
//! a mix from the third hour of a real night costs three hours; rehearsing the
//! same mix costs twenty seconds.
//!
//! # What this container cannot say
//!
//! Whether any of it *sounds* right. There is no audio device here. What can
//! be shown is that the actions are the automix's own, that the file is the
//! length it claims, and that the two records are both in it.

use std::time::Duration;

use crate::automix::{Automix, DeckView, Held};
use crate::plan::Record;
use crate::session::Session;
use crate::transition::Transition;
use dj_control::{SessionEvent, TimedEvent};
use dj_core::action::{Action, AutomixChange, DeckAction};
use dj_core::{DeckId, FramePos};

/// How much of the outgoing record is heard before the mix starts.
///
/// Four bars at a danceable tempo. What a mix sounds like depends on what was
/// already playing, so a rehearsal that opened on the first fader move would
/// be asking a DJ to judge a transition out of context — which is the one
/// thing a transition cannot be judged out of.
pub const RUN_UP: Duration = Duration::from_secs(8);

/// How much of the incoming record is heard after the mix ends.
///
/// Long enough to hear the new record standing on its own, short enough that
/// the file is about the mix rather than about the next track.
pub const TAIL: Duration = Duration::from_secs(4);

/// How often the simulated playhead is offered to the automix.
///
/// The same rate the interface pumps it at, because that is the automix this
/// rehearsal is a promise about. A finer step would rehearse a transition
/// smoother than the one djmanzo performs, and a coarser one would rehearse a
/// worse one; either would be a lie in a direction.
const STEP_HZ: f64 = 60.0;

/// A mix that was never played, as something that can be heard.
#[derive(Debug, Clone, PartialEq)]
pub struct Rehearsal {
    /// The actions, timestamped — a set file like any other.
    pub session: Session,
    /// How long the whole thing runs, in seconds.
    pub seconds: f64,
    /// Where the mix itself sits inside that, in seconds from the start.
    ///
    /// Carried so an interface can mark it rather than working it out from the
    /// run-up and getting a different answer at the edges.
    ///
    /// `mix_to` is where the transition **actually finished**, not where the
    /// plan's geometry said it would. A cut has no overlap, so it finishes on
    /// the tick it starts — and taking the planned length regardless would
    /// have the panel mark a fifteen-second mix inside a twelve-second file.
    pub mix_from: f64,
    pub mix_to: f64,
    /// The highest deck number the rehearsal uses, which is how many decks the
    /// render needs. The transition's own decks are used rather than 1 and 2:
    /// a rehearsal of *this* mix is a rehearsal of the decks it names.
    pub decks: usize,
    /// Frames to keep rendering after the last action, as
    /// [`crate::replay::render`]'s `extra_frames`.
    ///
    /// **The tail is not made of events.** A replay stops at the last thing
    /// that happened, and the last thing a transition does is eject the record
    /// that went out — after which the incoming record plays on with nothing
    /// being sent to it. Rendered without this the file simply ends there,
    /// four seconds short of what [`Rehearsal::seconds`] promises, and a DJ is
    /// handed a mix that stops the instant it lands.
    pub tail_frames: u64,
}

/// The two decks, as much of them as a rehearsal has to simulate.
///
/// Only what [`DeckView`] can change: whether a record is on it, whether it is
/// running, and where its playhead is. Everything else about a record is fixed
/// and comes from the analysis.
///
/// This is not a second engine. Nothing here makes a sound — the real engine
/// does that, later, from the actions this produces. What it models is only
/// enough for the automix to be asked the right questions.
#[derive(Debug, Clone, Copy)]
struct Side {
    id: DeckId,
    loaded: bool,
    playing: bool,
    position: f64,
    length: f64,
    bpm: f64,
    /// Frames of this record per frame of output.
    ///
    /// One until the automix syncs the incoming deck, after which the record
    /// is being resampled to the outgoing tempo and its playhead moves at a
    /// different speed. It changes nothing the automix decides — it reads the
    /// outgoing playhead for all of it — and modelling it anyway costs two
    /// lines and keeps the simulated deck honest.
    rate: f64,
    sample_rate: f64,
}

impl Side {
    fn view(self) -> DeckView {
        DeckView {
            id: self.id,
            loaded: self.loaded,
            playing: self.playing,
            position: self.position,
            length: if self.loaded { self.length } else { 0.0 },
            bpm: Some(self.bpm),
            sample_rate: self.sample_rate,
        }
    }

    fn step(&mut self, frames: f64) {
        if self.playing && self.loaded {
            self.position = (self.position + frames * self.rate).min(self.length);
        }
    }
}

/// Rehearse a transition: the actions the automix would send, timestamped.
///
/// `incoming` is the record coming in, which the transition object does not
/// carry — [`crate::plan::Incoming`] is deliberately the analysis alone, with
/// no length and no playhead. A rehearsal needs the length, so the caller that
/// has the library row supplies it.
///
/// The result is a set file. Render it with [`crate::replay::render_to_wav`]
/// and [`crate::replay::Window::WHOLE`]: a rehearsal has no history to skip.
#[must_use]
pub fn rehearse(
    transition: &Transition,
    incoming: Record,
    run_up: Duration,
    tail: Duration,
) -> Rehearsal {
    let out = transition.outgoing();
    let rate = out.sample_rate.as_f64();
    let step = rate / STEP_HZ;

    // Where the recording opens: the run-up before the mix, or the top of the
    // record if the mix is closer to it than that. Clamped rather than
    // refused, because a mix eight seconds into a track is a real thing to
    // want to hear and a shorter run-up is a fair answer to it.
    let from = (transition.plan.start_frame - run_up.as_secs_f64() * rate).max(0.0);
    let run_up_frames = transition.plan.start_frame - from;

    let mut side = [
        Side {
            id: transition.outgoing_deck,
            loaded: true,
            playing: true,
            position: from,
            length: out.length,
            bpm: out.bpm,
            rate: 1.0,
            sample_rate: rate,
        },
        Side {
            id: transition.incoming_deck,
            loaded: false,
            playing: false,
            position: 0.0,
            length: incoming.length,
            bpm: incoming.bpm,
            rate: 1.0,
            sample_rate: rate,
        },
    ];

    let mut events = Vec::new();
    let at = |events: &mut Vec<TimedEvent>, frame: f64, event: SessionEvent| {
        events.push(TimedEvent {
            event,
            at: Duration::from_secs_f64((frame / rate).max(0.0)),
        });
    };

    // The record already playing, put where the run-up starts. Seek after
    // load, because loading a deck puts its playhead at the top.
    at(
        &mut events,
        0.0,
        SessionEvent::Load {
            deck: transition.outgoing_deck,
            track: transition.outgoing_track,
        },
    );
    at(
        &mut events,
        0.0,
        SessionEvent::Action(Action::Deck {
            deck: transition.outgoing_deck,
            action: DeckAction::Seek(FramePos::new(from)),
        }),
    );
    at(
        &mut events,
        0.0,
        SessionEvent::Action(Action::Deck {
            deck: transition.outgoing_deck,
            action: DeckAction::Play,
        }),
    );

    // The automix, holding this exact mix and nothing else. Its own panel
    // settings are never consulted: a held mix names its style and its length,
    // and this *is* a held mix.
    let mut automix = Automix::new();
    automix.hold(Some(Held {
        outgoing: transition.outgoing_deck,
        incoming: transition.incoming_deck,
        start_frame: transition.plan.start_frame,
        length_beats: transition.plan.length_beats,
        style: transition.plan.style,
    }));
    let views: Vec<DeckView> = side.iter().map(|s| s.view()).collect();
    let opening = automix.apply(AutomixChange::SetEnabled(true), &views);
    debug_assert!(opening.actions.is_empty(), "switching on played something");

    // Step the playhead and let the automix write the rehearsal. It stops when
    // the transition has finished, plus the tail; a run-away is bounded by the
    // outgoing record's own length, because a mix cannot outlast the record it
    // is mixing out of.
    let tail_frames = tail.as_secs_f64() * rate;
    let mut ended: Option<f64> = None;
    let mut elapsed = 0.0_f64;
    let ceiling = out.length - from + tail_frames + step;

    while elapsed <= ceiling {
        let views: Vec<DeckView> = side.iter().map(|s| s.view()).collect();
        let plan = automix.tick(&views);

        for action in &plan.actions {
            at(&mut events, elapsed, SessionEvent::Action(*action));
            apply(&mut side, *action, out.bpm);
        }
        if let Some(deck) = plan.load {
            // Automix has no queue; here there is exactly one thing it can
            // want, which is the record the transition is into.
            at(
                &mut events,
                elapsed,
                SessionEvent::Load {
                    deck,
                    track: transition.incoming_track,
                },
            );
            if let Some(s) = side.iter_mut().find(|s| s.id == deck) {
                s.loaded = true;
                s.position = 0.0;
            }
        }

        // The transition is over the tick after it stops mixing, having started
        // at all — and once it is, there is nothing left to step for. The tail
        // is rendered rather than simulated, because nothing is *sent* during
        // it; see `Rehearsal::tail_frames`.
        if !automix.is_mixing() && elapsed > run_up_frames {
            ended = Some(elapsed);
            break;
        }

        for s in &mut side {
            s.step(step);
        }
        elapsed += step;
    }

    let total = ended.unwrap_or(elapsed) + tail_frames;
    Rehearsal {
        session: Session { events },
        seconds: total / rate,
        mix_from: run_up_frames / rate,
        mix_to: ended.unwrap_or(elapsed) / rate,
        decks: side
            .iter()
            .map(|s| s.id.index() + 1)
            .max()
            .unwrap_or(2)
            .max(2),
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        tail_frames: tail_frames.max(0.0) as u64,
    }
}

/// Apply one of the automix's own actions to the simulated decks.
///
/// Only the four that move a playhead. Everything else it sends — faders, EQ,
/// stems, effects — changes what the mix *sounds* like and nothing the automix
/// asks about, so simulating them would be modelling the engine in order to
/// tell the engine what to do.
fn apply(side: &mut [Side], action: Action, leader_bpm: f64) {
    let Action::Deck { deck, action } = action else {
        return;
    };
    let Some(s) = side.iter_mut().find(|s| s.id == deck) else {
        return;
    };
    match action {
        DeckAction::Play => s.playing = true,
        DeckAction::Pause => s.playing = false,
        DeckAction::Eject => {
            s.loaded = false;
            s.playing = false;
            s.position = 0.0;
        }
        // Matching tempo resamples the record, so its playhead moves at a
        // different speed than the output does.
        DeckAction::Sync if s.bpm > 1.0 => s.rate = leader_bpm / s.bpm,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Incoming, Outgoing};
    use dj_core::action::TransitionStyle;
    use dj_core::{Phrase, SampleRate, TrackId};

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    fn beat() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a deck")
    }

    fn record() -> Record {
        Record {
            length: 400.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            sample_rate: SR,
            grid_anchor: 0.0,
        }
    }

    /// A transition out of a 400-beat record, 100 beats in, into one at the
    /// same tempo. The same shape `transition`'s own tests use.
    fn armed(style: TransitionStyle) -> Transition {
        let outgoing = Outgoing {
            position: 100.0 * beat(),
            length: 400.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: None,
            sample_rate: SR,
            grid_anchor: 0.0,
        };
        let incoming = Incoming {
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: None,
        };
        let mut transition = Transition::plan(
            (deck(1), deck(2)),
            (TrackId::from_bytes([1; 32]), TrackId::from_bytes([2; 32])),
            outgoing,
            incoming,
            0.8,
        )
        .expect("a plan");
        transition.set_style(style);
        transition
    }

    fn lines(rehearsal: &Rehearsal) -> Vec<String> {
        rehearsal
            .session
            .events
            .iter()
            .map(|entry| entry.event.to_line())
            .collect()
    }

    /// **A rehearsal opens with the record already playing.**
    ///
    /// The whole reason this is cheap rather than costing the set up to it: a
    /// synthetic mix has no history, so the outgoing record is seeked to the
    /// run-up and started, and there is nothing before that to be faithful to.
    #[test]
    fn it_starts_the_outgoing_record_where_the_run_up_begins() {
        let transition = armed(TransitionStyle::Blend);
        let rehearsal = rehearse(&transition, record(), RUN_UP, TAIL);
        let said = lines(&rehearsal);

        assert_eq!(
            said[0],
            format!("load deck 1 {}", "01".repeat(32)),
            "the outgoing record was not the first thing on"
        );
        let want = transition.plan.start_frame - RUN_UP.as_secs_f64() * SR.as_f64();
        assert_eq!(said[1], format!("deck 1 seek {want}"));
        assert_eq!(said[2], "deck 1 play");
        assert_eq!(rehearsal.session.events[2].at, Duration::ZERO);
    }

    /// **It loads the other record itself**, because the automix asks for one
    /// and here there is exactly one thing it can mean.
    #[test]
    fn it_puts_the_incoming_record_on_the_other_deck() {
        let rehearsal = rehearse(&armed(TransitionStyle::Blend), record(), RUN_UP, TAIL);
        let loads: Vec<String> = lines(&rehearsal)
            .into_iter()
            .filter(|line| line.starts_with("load deck 2 "))
            .collect();
        assert_eq!(
            loads,
            vec![format!("load deck 2 {}", "02".repeat(32))],
            "the incoming record was not loaded exactly once"
        );
    }

    /// **The mix is the automix's, not this module's.**
    ///
    /// The rehearsal exists to say what djmanzo *would do*, so the actions in
    /// it have to be the ones the automix emits — including the parts that
    /// come from `crate::shape`. A blend hands the lows over; a fade does not
    /// touch the EQ; an echo throws an effect and takes it away again. If this
    /// module ever grew its own crossfade, one of these would keep passing
    /// against a rehearsal djmanzo would not perform.
    #[test]
    fn what_it_rehearses_is_what_the_automix_performs() {
        let blend = lines(&rehearse(
            &armed(TransitionStyle::Blend),
            record(),
            RUN_UP,
            TAIL,
        ));
        assert!(
            blend.iter().any(|line| line.starts_with("deck 1 eq_low")),
            "a blend left the outgoing bass alone"
        );

        let fade = lines(&rehearse(
            &armed(TransitionStyle::Fade),
            record(),
            RUN_UP,
            TAIL,
        ));
        assert!(
            !fade.iter().any(|line| line.contains("eq_low 0")),
            "a fade cut the bass: {fade:?}"
        );

        let echo = lines(&rehearse(
            &armed(TransitionStyle::Echo),
            record(),
            RUN_UP,
            TAIL,
        ));
        assert!(
            echo.iter().any(|line| line.contains("fx 1 on")),
            "an echo threw nothing: {echo:?}"
        );
        assert!(
            echo.iter().any(|line| line.contains("fx 1 off")),
            "an echo was left running on a deck handed back"
        );

        let drop = lines(&rehearse(
            &armed(TransitionStyle::VocalDrop),
            record(),
            RUN_UP,
            TAIL,
        ));
        assert!(
            drop.iter().any(|line| line == "deck 1 stem_solo_on vocal"),
            "a vocal drop kept no vocal: {drop:?}"
        );
    }

    /// **Both faders move, and the mix ends with the room on the new record.**
    ///
    /// The one thing every style has in common. A rehearsal in which the
    /// incoming deck never comes up is a recording of one record.
    #[test]
    fn the_room_ends_up_on_the_incoming_record() {
        let rehearsal = rehearse(&armed(TransitionStyle::Blend), record(), RUN_UP, TAIL);
        let said = lines(&rehearsal);
        let last_two = said
            .iter()
            .rev()
            .find(|line| line.starts_with("deck 2 volume"))
            .expect("the incoming fader never moved");
        assert_eq!(last_two, "deck 2 volume 1");
        assert!(
            said.iter().any(|line| line == "deck 1 pause"),
            "the outgoing record was never stopped"
        );
    }

    /// **A rehearsal is about as long as it says it is.**
    ///
    /// Within a tick, because the automix is stepped at 60 Hz and the mix ends
    /// on whichever tick crosses the finish.
    #[test]
    fn it_runs_for_the_run_up_the_mix_and_the_tail() {
        let transition = armed(TransitionStyle::Blend);
        let rehearsal = rehearse(&transition, record(), RUN_UP, TAIL);

        let mix = transition.end_seconds() - transition.start_seconds();
        let want = RUN_UP.as_secs_f64() + mix + TAIL.as_secs_f64();
        assert!(
            (rehearsal.seconds - want).abs() < 2.0 / STEP_HZ,
            "{} seconds, wanted about {want}",
            rehearsal.seconds
        );
        assert!((rehearsal.mix_from - RUN_UP.as_secs_f64()).abs() < 1e-6);
        assert!(
            (rehearsal.mix_to - rehearsal.mix_from - mix).abs() < 2.0 / STEP_HZ,
            "the mix is marked {:.2}s long and is {mix:.2}s",
            rehearsal.mix_to - rehearsal.mix_from
        );
    }

    /// A mix nearer the top of a record than the run-up gets a shorter run-up
    /// rather than a refusal: it is a real thing to want to hear.
    #[test]
    fn a_mix_near_the_top_of_a_record_still_rehearses() {
        let outgoing = Outgoing {
            position: 2.0 * beat(),
            length: 400.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: None,
            sample_rate: SR,
            grid_anchor: 0.0,
        };
        let incoming = Incoming {
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: None,
        };
        let mut transition = Transition::plan(
            (deck(1), deck(2)),
            (TrackId::from_bytes([1; 32]), TrackId::from_bytes([2; 32])),
            outgoing,
            incoming,
            0.8,
        )
        .expect("a plan");
        transition.move_start(-1_000);

        let rehearsal = rehearse(&transition, record(), RUN_UP, TAIL);
        assert!(rehearsal.mix_from >= 0.0, "a negative run-up");
        assert!(
            rehearsal.session.events[1].event.to_line().ends_with(" 0") || rehearsal.mix_from > 0.0,
            "seeked somewhere before the start of the record"
        );
    }

    /// **A rehearsal is as long as it says it is, once rendered.**
    ///
    /// The number in [`Rehearsal::seconds`] and the file a DJ is handed have to
    /// be the same thing. They were not: a replay stops at the last event, the
    /// last thing a transition does is eject the outgoing record, and the four
    /// seconds of tail after that contain no events at all — so the file ended
    /// the instant the mix landed, four seconds short, and every test above
    /// still passed because none of them rendered anything.
    ///
    /// Found by playing the file. This is that defect, pinned.
    #[test]
    fn the_rendered_file_is_as_long_as_the_rehearsal_promises() {
        let transition = armed(TransitionStyle::Blend);
        let rehearsal = rehearse(&transition, record(), RUN_UP, TAIL);

        // A ramp per record, long enough for the whole thing.
        let source = |frames: usize| -> std::sync::Arc<dyn dj_decode::TrackSource> {
            #[allow(clippy::cast_precision_loss)]
            let samples: Vec<f32> = (0..frames)
                .flat_map(|n| {
                    let v = (n as f32 / frames as f32) * 0.5;
                    [v, v]
                })
                .collect();
            std::sync::Arc::new(dj_decode::AudioBuffer::from_interleaved(samples, SR))
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = record().length as usize;
        let mut resolve = move |_| Some(source(frames));

        let mut emitted = 0_u64;
        let rendered = crate::replay::render(
            &rehearsal.session,
            SR,
            rehearsal.decks,
            rehearsal.tail_frames,
            crate::replay::Window::WHOLE,
            &mut resolve,
            &mut |block: &[f32]| emitted += block.len() as u64 / 2,
        )
        .expect("the rehearsal renders");

        let seconds = rendered.emitted as f64 / SR.as_f64();
        assert!(
            (seconds - rehearsal.seconds).abs() < 0.5,
            "the file is {seconds:.1}s and the panel says {:.1}s",
            rehearsal.seconds
        );
        assert_eq!(emitted, rendered.emitted, "the sink and the count disagree");
    }

    /// **A cut's mix window is a point, not a span.**
    ///
    /// A cut has no overlap: the outgoing record stops on the tick the incoming
    /// one starts. Marking it with the planned transition length — which the
    /// panel did — put a fifteen-second mix inside a twelve-second file. Found
    /// by rendering one and reading the panel beside the file.
    #[test]
    fn a_cut_is_marked_as_the_instant_it_is() {
        let cut = rehearse(&armed(TransitionStyle::Cut), record(), RUN_UP, TAIL);
        assert!(
            cut.mix_to - cut.mix_from < 2.0 / STEP_HZ,
            "a cut was marked as lasting {:.2}s",
            cut.mix_to - cut.mix_from
        );
        assert!(
            cut.mix_to <= cut.seconds,
            "the mix ends after the file does: {:.2} in {:.2}",
            cut.mix_to,
            cut.seconds
        );

        // And a blend, which does overlap, is still marked as a span.
        let blend = rehearse(&armed(TransitionStyle::Blend), record(), RUN_UP, TAIL);
        assert!(blend.mix_to - blend.mix_from > 1.0);
        assert!(blend.mix_to <= blend.seconds);
    }
}

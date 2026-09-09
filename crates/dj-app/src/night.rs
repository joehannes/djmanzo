//! What the night has been, and therefore what it is.
//!
//! [§11 of the directive](../../../docs/DIRECTIVE.md) asks for **one** context
//! engine underneath the interface rather than a phase worked out separately
//! inside the theme, the planner, the assistant and the autopilot. This is
//! where that engine lives in the running application: the engine itself is
//! `dj_core::ContextEngine`, which has no clock and no idea what a deck is, and
//! this hands it what the set is doing and holds the one answer everybody then
//! reads.
//!
//! # Two inputs, and only two
//!
//! **What the DJ declared.** Choosing an occasion is a statement about the
//! night, and it is pushed here when it is made rather than pulled from the
//! assistant's lock on every snapshot — the pump runs sixty times a second and
//! has no business taking that lock.
//!
//! **What the set has actually done.** Taken from the snapshot the pump has
//! just built, so the reading is of exactly the frame the interface is about to
//! be shown, and nothing has to be measured twice.
//!
//! # The clock is the set's, not the wall's
//!
//! [`Night::observe`] measures elapsed time from when the application started,
//! which is close enough to one gig that it is worth having before there is a
//! real session concept — the same approximation `AppState::session_id` makes,
//! for the same reason. [`Night::observe_at`] takes the time instead, so a test
//! can run a six-hour night in a millisecond.

use dj_core::{ContextEngine, Observation, SessionPhase, SessionRead};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// The context engine, in the running application.
#[derive(Debug)]
pub struct Night {
    engine: Mutex<ContextEngine>,
    /// The phase the DJ's occasion declares, when it declares one.
    declared: Mutex<Option<SessionPhase>>,
    /// Every phase the night has been read as, and when it became that.
    ///
    /// §67 lists the *set arc* as part of the session, and this is it as it
    /// actually happened rather than as it stands now. `crate::signals` needs
    /// it and cannot work without it: attributing a whole night's gestures to
    /// whichever phase it happens to be now is exactly the mis-learning §13
    /// exists to prevent.
    ///
    /// Only changes are stored, so this is a handful of entries over a whole
    /// set rather than one per snapshot at sixty a second.
    arc: Mutex<Vec<(Duration, SessionPhase)>>,
    started: Instant,
}

impl Default for Night {
    fn default() -> Self {
        Self::new()
    }
}

impl Night {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(ContextEngine::new()),
            declared: Mutex::new(None),
            arc: Mutex::new(Vec::new()),
            started: Instant::now(),
        }
    }

    /// Record what the DJ's chosen occasion says about the night.
    ///
    /// `None` for the occasions that name no point in an arc — see
    /// `dj_assistant::Occasion::declares_phase`, which is the only place that
    /// mapping is written down.
    pub fn declare(&self, phase: Option<SessionPhase>) {
        if let Ok(mut held) = self.declared.lock() {
            *held = phase;
        }
    }

    /// What the DJ last declared.
    #[must_use]
    pub fn declared(&self) -> Option<SessionPhase> {
        self.declared.lock().ok().and_then(|held| *held)
    }

    /// Take one look at the set, from the frame about to be shown.
    pub fn observe(&self, snapshot: &crate::Snapshot) -> Option<SessionRead> {
        let hour = dj_assistant::room::hour_of(SystemTime::now()).map(u32::from);
        self.observe_at(snapshot, self.started.elapsed(), hour)
    }

    /// As [`Self::observe`], with the clock supplied.
    pub fn observe_at(
        &self,
        snapshot: &crate::Snapshot,
        elapsed: Duration,
        hour: Option<u32>,
    ) -> Option<SessionRead> {
        let observation = Observation {
            elapsed,
            hour,
            playing: playing(snapshot),
            tempo: leading_tempo(snapshot),
            audio: snapshot.context.audio,
            declared: self.declared(),
        };
        let read = self
            .engine
            .lock()
            .ok()
            .and_then(|mut engine| engine.observe(&observation));

        // Record the arc, changes only. `read` is `Some` on every observation
        // once the engine has enough, so appending unconditionally would store
        // sixty entries a second for the rest of the night.
        if let Some(seen) = &read
            && let Ok(mut arc) = self.arc.lock()
            && arc.last().map(|(_, phase)| *phase) != Some(seen.phase)
        {
            arc.push((elapsed, seen.phase));
        }
        read
    }

    /// What the night was, at a moment that has already passed.
    ///
    /// `None` before anything could say — which is the first stretch of every
    /// set by `dj_core::context`'s own design, and is the honest answer rather
    /// than the earliest phase backdated over it.
    #[must_use]
    pub fn phase_at(&self, at: Duration) -> Option<SessionPhase> {
        let arc = self.arc.lock().ok()?;
        arc.iter()
            .rev()
            .find(|(began, _)| *began <= at)
            .map(|(_, phase)| *phase)
    }

    /// The arc so far, as changes.
    #[must_use]
    pub fn arc(&self) -> Vec<(Duration, SessionPhase)> {
        self.arc.lock().map(|arc| arc.clone()).unwrap_or_default()
    }

    /// The last answer, without taking another look.
    #[must_use]
    pub fn read(&self) -> Option<SessionRead> {
        self.engine.lock().ok().and_then(|engine| engine.read())
    }

    /// The phase the music alone reads as, if it reads as anything.
    #[must_use]
    pub fn measured(&self) -> Option<SessionPhase> {
        self.engine.lock().ok().and_then(|engine| engine.measured())
    }

    /// How many readings tonight's range is built from, and how many are still
    /// wanted before the evidence may name anything.
    #[must_use]
    pub fn progress(&self) -> (usize, usize) {
        self.engine
            .lock()
            .ok()
            .map_or((0, dj_core::context::ENOUGH), |engine| {
                (engine.readings(), engine.still_needed())
            })
    }

    /// Everything worth saying about the night, in the order it matters.
    ///
    /// Sentences rather than fields, and built here rather than in the
    /// interface, for the reason `dj_assistant::room` builds its own: the words
    /// are a judgement about which fact leads, and a template assembled in
    /// Svelte from four enums produces English nobody says.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        let Some(read) = self.read() else {
            let (kept, wanted) = self.progress();
            return vec![if kept == 0 {
                "Nothing has read the night yet.".to_owned()
            } else {
                format!("Listening. {wanted} more readings before the music can name the night.")
            }];
        };
        // Every sentence is built around `phase_words`, which is written to
        // follow "tonight is". That is why the declared and measured cases are
        // phrased as they are rather than as "set to" and "reads as": those
        // shipped first and read as "set to at its peak", which is nobody's
        // English and was obvious the first time the panel was looked at.
        let phase = phase_words(read.phase);
        match (read.basis, read.drift) {
            (dj_core::Basis::Agreed, _) => vec![
                format!("Tonight is {phase}, and the music agrees."),
                read.certainty.about().to_owned(),
            ],
            (dj_core::Basis::Disputed, Some(drift)) => vec![
                format!("You have said tonight is {phase}, and {}.", drift.phrase()),
                "Nothing will be mixed unasked while the two disagree.".to_owned(),
            ],
            (dj_core::Basis::Disputed, None) => {
                vec![format!(
                    "You have said tonight is {phase}. The music reads otherwise."
                )]
            }
            (dj_core::Basis::Declared, _) => vec![
                format!("You have said tonight is {phase}."),
                "Nothing has measured it yet.".to_owned(),
            ],
            (dj_core::Basis::Measured, _) => vec![
                format!("By the music, tonight is {phase}."),
                "Nobody has said what tonight is.".to_owned(),
            ],
            (dj_core::Basis::Nothing, _) => vec!["Nothing has read the night yet.".to_owned()],
        }
    }
}

/// A phase as it appears mid-sentence.
///
/// Separate from `SessionPhase::name`, which is the stable wire spelling a
/// workspace file and a snapshot use and must never change to suit a sentence.
#[must_use]
pub const fn phase_words(phase: SessionPhase) -> &'static str {
    match phase {
        SessionPhase::WarmUp => "a warm-up",
        SessionPhase::Heat => "building",
        SessionPhase::Peak => "at its peak",
        SessionPhase::Cooldown => "coming down",
        SessionPhase::ChillOut => "winding down",
    }
}

/// How many decks the room can actually hear.
///
/// A deck that is running with its fader down is not playing to anybody, and
/// counting it would have the engine sampling silence as music.
fn playing(snapshot: &crate::Snapshot) -> u8 {
    let count = snapshot
        .decks
        .iter()
        .filter(|deck| deck.playing && deck.volume > 0.01)
        .count();
    u8::try_from(count).unwrap_or(u8::MAX)
}

/// The tempo of the loudest deck the room can hear.
///
/// The played tempo, not the record's: a deck pitched up eight percent is
/// eight percent faster, and the night is what came out of the speakers.
fn leading_tempo(snapshot: &crate::Snapshot) -> Option<f32> {
    snapshot
        .decks
        .iter()
        .filter(|deck| deck.playing && deck.volume > 0.01)
        .filter_map(|deck| {
            let bpm = deck.analysis.as_ref()?.bpm?;
            (bpm.is_finite() && bpm > 0.0 && deck.rate.is_finite())
                .then_some((deck.volume, bpm * deck.rate))
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, tempo)| tempo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Snapshot;
    use dj_control::ParameterRegistry;
    use dj_core::param::{DeckParam, GlobalParam};
    use dj_core::{Basis, DeckId, ParamId};

    /// A frame with `decks` decks playing at `loudness`.
    fn frame(decks: u8, loudness: f32) -> Snapshot {
        let registry = ParameterRegistry::new();
        registry.set(ParamId::Global(GlobalParam::SampleRate), 48_000.0);
        // The bands are what the master metering publishes; `AudioMetrics`
        // derives the loudness from them, so setting them here is the same
        // path the engine sees in the running application.
        for band in GlobalParam::BANDS {
            registry.set(ParamId::Global(band), loudness / 2.0);
        }
        for number in 1..=decks {
            let deck = DeckId::from_human(number).expect("a deck");
            registry.set(ParamId::Deck(deck, DeckParam::Playing), 1.0);
            registry.set(ParamId::Deck(deck, DeckParam::Loaded), 1.0);
            registry.set(ParamId::Deck(deck, DeckParam::Volume), 1.0);
        }
        Snapshot::capture(&registry, 2)
    }

    /// Nothing declared and nothing measured is nothing, however long it runs.
    #[test]
    fn a_night_nothing_has_read_is_not_a_warm_up() {
        let night = Night::new();
        for step in 0..200u32 {
            let read = night.observe_at(&frame(1, 0.4), Duration::from_secs(u64::from(step)), None);
            assert!(read.is_none(), "named a phase from a night with no range");
        }
        assert!(night.read().is_none());
        assert!(night.notes().iter().any(|note| note.contains("night")));
    }

    /// Choosing an occasion is a statement about the night, and it is believed.
    #[test]
    fn what_the_dj_declared_is_the_night() {
        let night = Night::new();
        night.declare(Some(SessionPhase::Peak));
        let read = night
            .observe_at(&frame(1, 0.6), Duration::from_secs(30), Some(23))
            .expect("a declared night is a night");
        assert_eq!(read.phase, SessionPhase::Peak);
        assert_eq!(read.basis, Basis::Declared);
        assert_eq!(night.declared(), Some(SessionPhase::Peak));
        assert_eq!(night.read(), Some(read));
        assert!(
            night.notes()[0].contains("at its peak"),
            "{:?}",
            night.notes()
        );
    }

    /// A fader at the bottom is not playing to anybody, and sampling it as
    /// music would have the engine reading a night nobody heard.
    #[test]
    fn a_deck_with_its_fader_down_is_not_playing_to_anybody() {
        let mut snapshot = frame(2, 0.5);
        assert_eq!(playing(&snapshot), 2);
        snapshot.decks[0].volume = 0.0;
        assert_eq!(playing(&snapshot), 1);
        snapshot.decks[1].playing = false;
        assert_eq!(playing(&snapshot), 0);
    }

    /// The tempo the room heard, not the one on the label.
    #[test]
    fn the_tempo_is_the_one_that_came_out_of_the_speakers() {
        let mut snapshot = frame(1, 0.5);
        assert_eq!(leading_tempo(&snapshot), None, "invented a tempo");
        snapshot.decks[0].analysis = Some(crate::snapshot::TrackAnalysisSnapshot {
            bpm: Some(120.0),
            bpm_confidence: Some(0.9),
            bpm_alternative: None,
            sync_worthy: true,
            key_camelot: None,
            key_standard: None,
            key_confidence: None,
            key_alternative: None,
            lufs: None,
            auto_gain_db: 0.0,
            phrase_beats: None,
            phrase_anchor: None,
            phrase_confidence: None,
        });
        snapshot.decks[0].rate = 1.08;
        let tempo = leading_tempo(&snapshot).expect("a deck with a grid has a tempo");
        assert!((tempo - 129.6).abs() < 1e-3, "read {tempo}");
    }

    /// Run a night that climbs steadily, and answer with the last read.
    fn climbing_night(night: &Night, declared: Option<SessionPhase>) -> Option<SessionRead> {
        night.declare(declared);
        let mut read = None;
        for step in 0..(dj_core::context::ENOUGH + 40) {
            #[allow(clippy::cast_precision_loss)]
            let loudness = (step as f32 / 250.0).min(1.0);
            let elapsed = dj_core::context::SAMPLE * u32::try_from(step).expect("fits");
            read = night.observe_at(&frame(1, loudness), elapsed, Some(1));
        }
        read
    }

    /// **The disagreement is said, not swallowed.**
    ///
    /// The most valuable sentence this module produces, and the one a DJ acts
    /// on: you said the night is a warm-up, the music has been getting harder
    /// for an hour, and nothing will be mixed unasked until that resolves.
    /// Asserted here rather than only in the browser, because the browser test
    /// runs against a stubbed answer and would stay green if this stopped
    /// producing one.
    #[test]
    fn a_disputed_night_says_which_way_the_music_disagrees() {
        let night = Night::new();
        let read = climbing_night(&night, Some(SessionPhase::WarmUp))
            .expect("a night with both sources reads as something");
        assert_eq!(read.basis, Basis::Disputed);
        assert_eq!(read.phase, SessionPhase::WarmUp, "overruled the DJ");

        let notes = night.notes();
        assert!(
            notes[0].contains("a warm-up") && notes[0].contains("harder"),
            "the drift is not in the sentence: {notes:?}"
        );
        assert!(
            notes.iter().any(|note| note.contains("mixed unasked")),
            "nothing said what the disagreement costs: {notes:?}"
        );
        // And what it costs, through the type that enforces it.
        assert!(
            !dj_assistant::Posture::Autopilot
                .warrant(read.certainty)
                .may_mix()
        );
    }

    /// Six minutes of a night that climbs, and the engine names it — from the
    /// evidence, with nothing declared.
    #[test]
    fn a_night_that_climbs_is_read_from_the_evidence() {
        let night = Night::new();
        let mut read = None;
        for step in 0..(dj_core::context::ENOUGH + 40) {
            #[allow(clippy::cast_precision_loss)]
            let loudness = (step as f32 / 250.0).min(1.0);
            let elapsed = dj_core::context::SAMPLE * u32::try_from(step).expect("fits");
            read = night.observe_at(&frame(1, loudness), elapsed, Some(1));
        }
        let read = read.expect("a night with a range reads as something");
        assert_eq!(read.basis, Basis::Measured);
        assert_eq!(read.environment.time_of_day, dj_core::TimeOfDay::SmallHours);
        assert!(
            night.notes()[0].contains("By the music, tonight is"),
            "{:?}",
            night.notes()
        );
        let (kept, needed) = night.progress();
        assert!(kept >= dj_core::context::ENOUGH);
        assert_eq!(needed, 0);
    }

    /// **The arc keeps what the night *was*, not only what it is.**
    ///
    /// §67 lists the set arc as part of the session, and `crate::signals`
    /// cannot work without it: attributing a whole night's gestures to
    /// whichever phase it happens to be now is exactly the mis-learning §13
    /// exists to prevent.
    #[test]
    fn the_arc_remembers_when_the_night_changed_and_refuses_to_backdate() {
        let night = Night::new();
        // Nothing has read anything yet, so nothing is claimed about any
        // moment — including the ones that have already happened.
        assert_eq!(night.arc(), vec![]);
        assert_eq!(night.phase_at(Duration::from_secs(0)), None);
        assert_eq!(night.phase_at(Duration::from_secs(600)), None);

        let mut last = Duration::ZERO;
        for step in 0..(dj_core::context::ENOUGH + 40) {
            #[allow(clippy::cast_precision_loss)]
            let loudness = (step as f32 / 250.0).min(1.0);
            last = dj_core::context::SAMPLE * u32::try_from(step).expect("fits");
            night.observe_at(&frame(1, loudness), last, Some(1));
        }

        let arc = night.arc();
        assert!(
            !arc.is_empty(),
            "a night that read as something kept nothing"
        );
        // Changes only: a phase that held for a hundred observations is one
        // entry, not a hundred.
        assert!(
            arc.len() < 10,
            "the arc stored {} entries for one climb",
            arc.len()
        );
        let mut phases = arc.iter().map(|(_, p)| *p);
        let mut previous = phases.next();
        for phase in phases {
            assert_ne!(Some(phase), previous, "the arc repeats a phase");
            previous = Some(phase);
        }

        // And the answer for a moment is the phase that was current *then*.
        let (began, phase) = arc[0];
        assert_eq!(night.phase_at(began), Some(phase));
        assert_eq!(
            night.phase_at(last),
            Some(night.read().expect("a read").phase)
        );
        // Still nothing claimed about the stretch before anything was read.
        if began > Duration::ZERO {
            assert_eq!(night.phase_at(began - Duration::from_millis(1)), None);
        }
    }
}

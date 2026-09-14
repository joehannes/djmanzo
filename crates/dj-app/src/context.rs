//! §11's `DJContext`, gathered in one pass.
//!
//! > Build an explicit internal concept: `DJContext = { sessionPhase, occasion,
//! > musicContext, hardwareContext, audienceContext, djBehaviorContext,
//! > attentionBudget, performanceHealth }`. The context engine should become
//! > the common input to [nine things]. **Do not duplicate context logic inside
//! > each component.**
//!
//! [`dj_core::ContextEngine`] is the judgement half and has shipped for a long
//! time: what the DJ declared, where the last few minutes sit in the night's
//! own range, and which way the two disagree. What had not shipped is the
//! *object* — eight fields in one place, gathered once.
//!
//! # Five of the eight were real and none of them were together
//!
//! The phase, the occasion, the attention budget, the room and the machine's
//! health were each already published, by five different things, on five
//! different schedules. A consumer wanting three of them asked three questions
//! and got three answers about three moments, which is the second-descriptions
//! failure with a clock in it. Three more — `musicContext`, `hardwareContext`
//! and `djBehaviorContext` — were not gathered at all.
//!
//! # Gathered here, judged elsewhere
//!
//! Nothing in this module decides anything. Every field is read from the thing
//! that owns it: the phase from the context engine, the budget from
//! [`crate::cockpit::Attention`], the room from `dj_assistant::room`, and the
//! three new ones from the snapshot, the control hub and the action log. That
//! is §11's own instruction — *do not duplicate context logic inside each
//! component* — applied to the gatherer as much as to the consumers: a second
//! opinion about the phase would be the exact thing the engine exists to stop.
//!
//! # Absence is a value
//!
//! A deck with nothing on it has no tempo; a night nobody has read has no
//! phase; a room nothing is watching has no reading. Each is `None` rather
//! than a zero or a default, because a confident nought is a claim and an
//! absence is not.

use dj_core::{BehaviourContext, HardwareContext, HealthContext, MusicContext, SessionPhase};

/// §11's eight fields, as one object.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DjContext {
    /// Where the night is. `None` until something has read it — the first six
    /// minutes of every set, by the context engine's own design.
    pub session_phase: Option<SessionPhase>,
    /// What the DJ set the night up to be, in the assistant's own words.
    pub occasion: String,
    pub music: MusicContext,
    pub hardware: HardwareContext,
    /// What the room is doing, in the room's own sentence.
    ///
    /// `None` when nothing is watching, which is this container always and
    /// most installations most of the time. A room with no camera and no
    /// microphone has not been read, and a neutral reading would be an
    /// invention rather than a default.
    pub audience: Option<String>,
    pub behaviour: BehaviourContext,
    /// How much the interface may ask of the DJ right now. §18.
    pub attention: crate::cockpit::Attention,
    pub health: HealthContext,
}

/// §11's eight fields, in §11's order and §11's words, beside djmanzo's own.
///
/// **One table, not two descriptions.** The directive names the fields in
/// TypeScript and djmanzo names them in Rust, and the day one of them is
/// dropped the only thing that would notice is a person re-reading §11. The
/// test below reads the serialised object and holds it to exactly this list —
/// in both directions, so a field quietly added without a §11 name fails as
/// loudly as one quietly removed.
pub const FIELDS: [(&str, &str); 8] = [
    ("sessionPhase", "session_phase"),
    ("occasion", "occasion"),
    ("musicContext", "music"),
    ("hardwareContext", "hardware"),
    ("audienceContext", "audience"),
    ("djBehaviorContext", "behaviour"),
    ("attentionBudget", "attention"),
    ("performanceHealth", "health"),
];

/// What is on the decks, from the snapshot the interface is already being sent.
///
/// The **playing** decks decide the tempo, because that is what the room is
/// hearing: a record cued up at 174 on deck 3 is not the tempo of the night,
/// and taking the first loaded deck would make it one. With several playing,
/// the loudest wins — the fader is the DJ saying which record is the mix.
#[must_use]
pub fn music(snapshot: &crate::Snapshot) -> MusicContext {
    let playing: Vec<&crate::snapshot::DeckSnapshot> =
        snapshot.decks.iter().filter(|deck| deck.playing).collect();

    let carrying = carrying(snapshot);

    let tempos: Vec<f32> = playing
        .iter()
        .filter_map(|deck| deck.effective_bpm)
        .collect();
    let spread = (tempos.len() >= 2).then(|| {
        let lowest = tempos.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = tempos.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        highest - lowest
    });

    MusicContext {
        bpm: carrying.and_then(|deck| deck.effective_bpm),
        key: carrying.and_then(|deck| deck.analysis.as_ref()?.key_camelot.clone()),
        playing: playing.len(),
        ready: snapshot
            .decks
            .iter()
            .filter(|deck| deck.loaded && !deck.playing)
            .count(),
        bpm_spread: spread,
    }
}

/// The deck the room is hearing, if any.
///
/// Playing, and the loudest of those: the fader is the DJ saying which record
/// the mix is, and halfway through a blend both are playing. Taking the first
/// loaded deck instead would make a record cued at 174 on deck 3 the answer
/// for a 124 BPM set.
///
/// One rule rather than two. [`music`] reads the tempo and the key off this,
/// and the assistant's briefing asks the rail about the same deck; two answers
/// to "which record is the night" would disagree at exactly the moment it
/// matters, which is mid-blend.
#[must_use]
pub fn carrying(snapshot: &crate::Snapshot) -> Option<&crate::snapshot::DeckSnapshot> {
    snapshot
        .decks
        .iter()
        .filter(|deck| deck.playing)
        .max_by(|a, b| {
            a.volume
                .partial_cmp(&b.volume)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// The machine and what is plugged into it.
///
/// `controller` and `midi` are two different facts and both are needed: a
/// machine with no MIDI service at all cannot be helped by plugging something
/// in, and one with a service and nothing open can.
#[must_use]
pub fn hardware(
    snapshot: &crate::Snapshot,
    control: &crate::control::ControlStatus,
) -> HardwareContext {
    HardwareContext {
        sample_rate: snapshot.master.sample_rate,
        output_latency_ms: snapshot.master.output_latency_ms,
        controller: control.open_port.clone(),
        // `unavailable` is the MIDI *service* failing, which the controllers
        // panel says in as many words. An empty device list is not that.
        midi: control.unavailable.is_none(),
        cue: snapshot.master.cue_available,
    }
}

/// Whether djmanzo is keeping up with itself.
#[must_use]
pub fn health(snapshot: &crate::Snapshot) -> HealthContext {
    HealthContext {
        cpu_load: snapshot.master.cpu_load,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        dropouts: snapshot.master.xruns.max(0.0) as u32,
        limiter_reduction_db: snapshot.master.limiter_reduction_db,
    }
}

/// How the DJ is working tonight, off the action log and the rail's own record.
///
/// **Counts and rates, never a preference.** §13's rule at the scale of a
/// single night: "forty gestures a minute" is a fact and "this DJ is frantic"
/// is a claim nothing has earned. The commonest gesture is `None` until §13's
/// own threshold clears it, because that is the one place in djmanzo allowed
/// to say a DJ does something *often*.
#[must_use]
pub fn behaviour(
    signals: &[crate::signals::Signal],
    elapsed: std::time::Duration,
    taken: u64,
    ignored: u64,
) -> BehaviourContext {
    let minutes = elapsed.as_secs_f32() / 60.0;
    #[allow(clippy::cast_precision_loss)]
    let rate = if minutes > 0.0 {
        signals.len() as f32 / minutes
    } else {
        0.0
    };

    // The strongest tendency §13 will admit to, which is already sorted by how
    // much of its phase it was. Nothing invented here: a gesture that has not
    // cleared the threshold is not this DJ's habit, and this module is not the
    // place to lower the bar.
    let commonest = crate::signals::tendencies(signals)
        .first()
        .map(|found| found.did().slug().to_owned());

    BehaviourContext {
        gestures_per_minute: rate,
        commonest,
        taken,
        ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{DeckSnapshot, Snapshot, TrackAnalysisSnapshot};

    /// A captured frame, which is the only honest way to build one of these:
    /// `Snapshot` has no `Default` on purpose — a struct of forty zeros is a
    /// machine reporting silence at nought hertz, and several of those zeros
    /// are legitimate values elsewhere.
    fn frame() -> Snapshot {
        let registry = dj_control::ParameterRegistry::new();
        registry.set(
            dj_core::ParamId::Global(dj_core::param::GlobalParam::SampleRate),
            48_000.0,
        );
        Snapshot::capture(&registry, 4)
    }

    fn deck(
        number: u8,
        playing: bool,
        loaded: bool,
        bpm: Option<f32>,
        volume: f32,
    ) -> DeckSnapshot {
        let mut view = frame().decks.remove(usize::from(number) - 1);
        view.playing = playing;
        view.loaded = loaded;
        view.effective_bpm = bpm;
        view.volume = volume;
        view
    }

    fn with(decks: Vec<DeckSnapshot>) -> Snapshot {
        let mut snapshot = frame();
        snapshot.decks = decks;
        snapshot
    }

    /// **The tempo is the one the room is hearing, not the first one loaded.**
    ///
    /// The mistake this is written against: take `decks[0].bpm` and call it
    /// the night's tempo. A record cued up at 174 on deck 3 is not the tempo
    /// of a 124 BPM set, and everything downstream — the rail, the plan, the
    /// assistant's briefing — would be reasoning about a record nobody can
    /// hear.
    #[test]
    fn the_tempo_comes_from_a_deck_that_is_actually_playing() {
        let read = music(&with(vec![
            deck(1, false, true, Some(174.0), 1.0),
            deck(2, true, true, Some(124.0), 1.0),
        ]));
        assert_eq!(read.bpm, Some(124.0));
        assert_eq!(read.playing, 1);
        assert_eq!(read.ready, 1);
        assert_eq!(read.bpm_spread, None, "one tempo is not a spread");
    }

    /// **With two playing, the louder fader is the mix.**
    ///
    /// The fader is the DJ saying which record the room is hearing, and
    /// halfway through a blend both are true — so "the loudest" is the answer
    /// that agrees with what is actually coming out.
    #[test]
    fn the_louder_of_two_playing_decks_carries_the_tempo() {
        let read = music(&with(vec![
            deck(1, true, true, Some(124.0), 0.2),
            deck(2, true, true, Some(128.0), 0.9),
        ]));
        assert_eq!(read.bpm, Some(128.0));
        assert_eq!(read.playing, 2);
        assert_eq!(read.bpm_spread, Some(4.0));
    }

    /// **An empty deck has no tempo, and that is not nought.**
    ///
    /// A zero here would say "no beats per minute", which is a claim about
    /// silence rather than about an empty deck — and everything reading it
    /// would treat the night as stopped rather than as not started.
    #[test]
    fn nothing_loaded_reads_as_absent_rather_than_as_zero() {
        let read = music(&with(vec![deck(1, false, false, None, 1.0)]));
        assert_eq!(read.bpm, None);
        assert_eq!(read.key, None);
        assert_eq!(read.playing, 0);
        assert_eq!(read.ready, 0);
    }

    /// The key comes off the same deck the tempo does, or it is absent.
    #[test]
    fn the_key_is_the_playing_records_own_or_none_at_all() {
        let mut analysed = deck(1, true, true, Some(124.0), 1.0);
        // Every field absent except the one under test. Written out rather
        // than defaulted, because `TrackAnalysisSnapshot` deliberately has no
        // `Default`: a record with a bpm of nought and a confidence of nought
        // is an analysis that ran and found silence, which is not the same
        // thing as one that has not run.
        analysed.analysis = Some(TrackAnalysisSnapshot {
            bpm: None,
            bpm_confidence: None,
            bpm_alternative: None,
            sync_worthy: false,
            key_camelot: Some("8A".to_owned()),
            key_standard: None,
            key_confidence: None,
            key_alternative: None,
            lufs: None,
            auto_gain_db: 0.0,
            phrase_beats: None,
            phrase_anchor: None,
            phrase_confidence: None,
        });
        assert_eq!(music(&with(vec![analysed])).key.as_deref(), Some("8A"));

        // Playing and unanalysed: a tempo and no key, rather than a guess.
        let read = music(&with(vec![deck(1, true, true, Some(124.0), 1.0)]));
        assert_eq!(read.bpm, Some(124.0));
        assert_eq!(read.key, None);
    }

    /// **No controller and no MIDI service are different facts.**
    ///
    /// The one a DJ can act on and the one they cannot. Plugging something in
    /// fixes the first and does nothing at all for the second, and a single
    /// "no controller" would tell them to try the thing that cannot work.
    #[test]
    fn a_machine_without_midi_is_told_apart_from_one_with_nothing_plugged_in() {
        let snapshot = frame();
        // The real thing, from a hub with nothing open — which on this machine
        // is also a hub with no MIDI service, so the second case is made by
        // clearing the reason rather than by inventing one.
        let (hub, _take) = crate::control::ControlHub::new();
        let as_found = hub.status(None);
        let nothing_plugged_in = crate::control::ControlStatus {
            unavailable: None,
            ..as_found.clone()
        };
        let no_service = crate::control::ControlStatus {
            unavailable: Some("MIDI support could not be initialized".to_owned()),
            ..as_found
        };

        let quiet = hardware(&snapshot, &nothing_plugged_in);
        assert!(
            quiet.midi,
            "a machine with a MIDI service reads as having one"
        );
        assert_eq!(quiet.controller, None);

        let dead = hardware(&snapshot, &no_service);
        assert!(!dead.midi);
        assert_eq!(dead.controller, None);
    }

    /// **A rate, never a judgement.**
    ///
    /// §13's rule at the scale of one night. "Forty gestures a minute" is a
    /// fact; "this DJ is frantic" is a claim nothing has earned, and the
    /// commonest gesture stays absent until §13's own threshold clears it —
    /// this module is not the place to lower that bar.
    #[test]
    fn a_busy_night_is_a_rate_and_a_habit_is_still_earned() {
        use dj_control::{SessionEvent, TimedEvent};
        use dj_core::{Action, DeckId, action::DeckAction};

        let press = |secs: u64| TimedEvent {
            at: std::time::Duration::from_secs(secs),
            by: dj_control::By::Hand,
            event: SessionEvent::Action(Action::Deck {
                deck: DeckId::from_human(1).expect("deck 1"),
                action: DeckAction::SetEqLow(0.5),
            }),
        };
        let log: Vec<TimedEvent> = (0..6).map(press).collect();
        let signals = crate::signals::signals(&log, &|_| Some(dj_core::SessionPhase::Peak));

        let read = behaviour(&signals, std::time::Duration::from_secs(120), 3, 7);
        assert!(
            (read.gestures_per_minute - 3.0).abs() < f32::EPSILON,
            "six gestures over two minutes is three a minute, not {}",
            read.gestures_per_minute
        );
        assert_eq!(read.taken, 3);
        assert_eq!(read.ignored, 7);
        // Six of one gesture in one phase clears §13's four, so this one is
        // earned. The next test is the other direction.
        assert_eq!(read.commonest.as_deref(), Some("eq-moved"));

        let thin = crate::signals::signals(&log[..2], &|_| Some(dj_core::SessionPhase::Peak));
        assert_eq!(
            behaviour(&thin, std::time::Duration::from_secs(120), 0, 0).commonest,
            None,
            "two presses became a habit"
        );
    }

    /// **All eight of §11's fields are on the object, and nothing else is.**
    ///
    /// The guard the whole module rests on. §11 is a list of eight and djmanzo
    /// had five of them, published by five different things — so "the context
    /// engine" was a phrase rather than an object, and the only thing that
    /// would have noticed a sixth going missing is a person re-reading the
    /// directive. Both directions, because a field added without a §11 name is
    /// the same drift from the other side.
    #[test]
    fn the_object_carries_exactly_the_eight_fields_section_eleven_names() {
        let gathered = DjContext {
            session_phase: None,
            occasion: "open".to_owned(),
            music: MusicContext::default(),
            hardware: HardwareContext::default(),
            audience: None,
            behaviour: BehaviourContext::default(),
            attention: crate::cockpit::Attention::for_context(&frame()),
            health: HealthContext::default(),
        };
        let json = serde_json::to_value(&gathered).expect("the context serialises");
        let object = json.as_object().expect("an object");

        for (directive, ours) in FIELDS {
            assert!(
                object.contains_key(ours),
                "§11's `{directive}` is named `{ours}` here and the object does not carry it"
            );
        }
        let named: std::collections::BTreeSet<&str> =
            FIELDS.iter().map(|(_, ours)| *ours).collect();
        for key in object.keys() {
            assert!(
                named.contains(key.as_str()),
                "`{key}` is on the context and is not one of §11's eight"
            );
        }
        assert_eq!(object.len(), FIELDS.len());
    }

    /// A night that has not started has a rate of nought rather than a divide.
    #[test]
    fn a_night_with_no_time_in_it_does_not_divide_by_it() {
        let read = behaviour(&[], std::time::Duration::ZERO, 0, 0);
        assert!(read.gestures_per_minute.is_finite());
        assert_eq!(read.gestures_per_minute, 0.0);
    }
}

//! The transition as a thing you can hold.
//!
//! [`crate::plan`] answers a question: given these two records and this
//! playhead, where would the mix go and how long would it run. The answer used
//! to live for exactly one call — it was drawn once in the automix panel and
//! then thrown away — which is why nothing in djmanzo could *do* anything with
//! it. §68 of the directive asks for the other thing: an explicit transition
//! object, one per mix, that the waveform, the preview, the assistant, the
//! autopilot and the replay all read instead of each re-deriving its own.
//!
//! This is that object, and the smallest honest version of it: the two decks,
//! where the mix starts and ends, how long it runs, which way, what the tempo
//! and key do across it, how well the two records go together, and why.
//!
//! # It is editable, and an edit re-scores rather than re-plans
//!
//! A planned transition a DJ cannot move is a suggestion, not an object. So
//! the start, the length and the style can all be changed — and when they are,
//! the reasons are **re-derived over the new geometry** by
//! [`plan::evaluate`]. Keeping the planner's original reasons would give an
//! interface that states a mix lands on a phrase boundary the DJ has just
//! moved it off, which is worse than saying nothing: it is a confident lie
//! about the one fact the panel exists to report.
//!
//! Once edited, [`Transition::edited`] stays true. Nothing here replans over a
//! DJ's own decision on its own; re-planning is a thing they ask for.
//!
//! # What confidence means here
//!
//! **How well the two records go together**, on the scale the Next rail and
//! Set Flow's seams already draw — it comes from `dj_library::suggest`, the
//! one scorer, handed in by the caller that has the library rows. It is a
//! property of the pair, so moving the mix a phrase later does not change it;
//! what changes is the reasons.
//!
//! Two numbers on one screen that both claim to be confidence and disagree is
//! the failure this avoids. The rail says a record is a 0.8 match; the pair
//! view must not then call the same pair 0.6 because it weighed the phrase
//! boundary in as well.
//!
//! # What it does not carry yet
//!
//! §68's optional `outgoingStems`, `eqPlan` and `fxPlan` are not here. Nothing
//! in djmanzo yet decides *which stems* or *which EQ curve* a transition
//! should use — the automix runs a style, not a per-band plan — and a field
//! that is always `None` is a promise rather than a feature.

use crate::plan::{self, Incoming, Outgoing, Plan};
use dj_core::action::TransitionStyle;
use dj_core::{DeckId, KeyRelation, TrackId};

/// The shortest and longest transition the object will hold, in beats.
///
/// Wider than the three lengths the planner proposes, because this is what a
/// DJ may ask for rather than what djmanzo suggests: four beats is a hard cut
/// on the downbeat and sixty-four is a long, patient blend, and both are
/// things people do. Outside that range it is not a transition — it is a cut
/// with extra steps at one end and a mashup at the other.
pub const LENGTH_RANGE: (u32, u32) = (4, 64);

/// One mix, as an object rather than as an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub outgoing_deck: DeckId,
    pub incoming_deck: DeckId,
    /// What is on those decks. Held so a transition can notice that the record
    /// it describes has been replaced — a plan for a track that is no longer
    /// loaded is worse than no plan, because it looks current.
    pub outgoing_track: TrackId,
    pub incoming_track: TrackId,
    /// The geometry and the reasoning, from [`plan::evaluate`].
    pub plan: Plan,
    /// How well the two records go together, 0 to 1, on the rail's scale.
    pub confidence: f64,
    /// True once a human has moved, shortened or restyled it.
    pub edited: bool,
    /// The inputs, kept so an edit can be re-scored against the same two
    /// records rather than against whatever the decks hold by then.
    ///
    /// `Outgoing::position` is the playhead as it was when the transition was
    /// armed. That is deliberate: the mix point is a decision about the record,
    /// and re-reading the playhead on every edit would let the earliest
    /// allowed start creep forward under a DJ's hands while they were adjusting
    /// it.
    outgoing: Outgoing,
    incoming: Incoming,
}

impl Transition {
    /// Plan a mix and hold the result.
    ///
    /// `None` for the same reasons the planner answers `None`: no grid, a
    /// tempo that is not a tempo, or a record already past its last usable
    /// phrase.
    #[must_use]
    pub fn plan(
        decks: (DeckId, DeckId),
        tracks: (TrackId, TrackId),
        outgoing: Outgoing,
        incoming: Incoming,
        confidence: f64,
    ) -> Option<Self> {
        let plan = plan::plan(&outgoing, &incoming)?;
        Some(Self {
            outgoing_deck: decks.0,
            incoming_deck: decks.1,
            outgoing_track: tracks.0,
            incoming_track: tracks.1,
            plan,
            confidence: confidence.clamp(0.0, 1.0),
            edited: false,
            outgoing,
            incoming,
        })
    }

    /// Where the mix starts, in seconds into the outgoing record.
    #[must_use]
    pub fn start_seconds(&self) -> f64 {
        self.plan.start_frame / self.outgoing.sample_rate.as_f64()
    }

    /// Where it finishes, in seconds into the outgoing record.
    #[must_use]
    pub fn end_seconds(&self) -> f64 {
        self.plan.end_frame / self.outgoing.sample_rate.as_f64()
    }

    #[must_use]
    pub fn key_relation(&self) -> Option<KeyRelation> {
        self.plan.key_relation
    }

    /// Move the start by `beats`, negative for earlier.
    ///
    /// Clamped at both ends, and the clamps are the two facts a DJ cannot
    /// argue with: a mix cannot start behind the playhead, and it cannot end
    /// after the record does. Clamped rather than refused, because a nudge
    /// button that sometimes does nothing without saying why is the sort of
    /// control people press twice and then distrust.
    pub fn move_start(&mut self, beats: i64) {
        let target = self.plan.start_beat.saturating_add(beats);
        self.set_geometry(target, self.plan.length_beats, self.plan.style);
    }

    /// Move the start to wherever in the record a hand pointed.
    ///
    /// Frames rather than beats because that is what a hand knows: §26 asks
    /// for the mix point to be *grabbed* on the waveform rather than typed
    /// into a panel, and a pointer lands on a place in the record, not on a
    /// beat index. Which beat that is, is djmanzo's arithmetic and stays here
    /// — an interface that worked it out would need the grid, the tempo and
    /// the record's sample rate, and would be a second opinion about where
    /// beat 275 is.
    ///
    /// Snapped to the nearest beat. A mix starts on a beat, and a drag that
    /// left it 40 ms off would be a worse answer than the one the DJ was
    /// trying to give.
    pub fn move_to_frame(&mut self, frame: f64) {
        let Some(beat) = self.beat_at(frame) else {
            return;
        };
        self.set_geometry(beat, self.plan.length_beats, self.plan.style);
    }

    /// Move the *end* to wherever a hand pointed, which is to say set the
    /// length by dragging its other edge.
    ///
    /// Clamped to [`LENGTH_RANGE`] like any other length. Dragging the end
    /// back past the start does not turn the transition inside out; it makes
    /// it the shortest thing this object will hold.
    pub fn end_at_frame(&mut self, frame: f64) {
        let Some(end) = self.beat_at(frame) else {
            return;
        };
        let beats = (end - self.plan.start_beat).max(0);
        let beats = u32::try_from(beats).unwrap_or(LENGTH_RANGE.1);
        self.set_length(beats);
    }

    /// Which beat of the outgoing record a frame falls on, nearest.
    ///
    /// `None` only for a frame that is not a number, which a pointer cannot
    /// produce but a caller could.
    fn beat_at(&self, frame: f64) -> Option<i64> {
        if !frame.is_finite() {
            return None;
        }
        let beats = (frame - self.outgoing.grid_anchor) / self.beat_frames();
        if !beats.is_finite() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        Some(beats.round() as i64)
    }

    /// Set the length in beats, clamped to [`LENGTH_RANGE`].
    pub fn set_length(&mut self, beats: u32) {
        let beats = beats.clamp(LENGTH_RANGE.0, LENGTH_RANGE.1);
        self.set_geometry(self.plan.start_beat, beats, self.plan.style);
    }

    /// Choose a different way to do it.
    ///
    /// The style is not checked against the tempos. A DJ who wants to blend
    /// two records six BPM apart is doing something the planner would not
    /// suggest and is entitled to do it; the reasons still say the tempos
    /// clash, which is the honest arrangement — report, do not veto.
    pub fn set_style(&mut self, style: TransitionStyle) {
        self.set_geometry(self.plan.start_beat, self.plan.length_beats, style);
    }

    /// Throw the edits away and ask the planner again.
    ///
    /// Only ever on request. An object that quietly replanned itself would
    /// undo a DJ's adjustment at whatever moment it next recalculated, which
    /// is the behaviour §45 exists to forbid.
    pub fn replan(&mut self) {
        if let Some(plan) = plan::plan(&self.outgoing, &self.incoming) {
            self.plan = plan;
            self.edited = false;
        }
    }

    /// True when this describes the records those decks actually hold.
    #[must_use]
    pub fn describes(&self, outgoing: Option<TrackId>, incoming: Option<TrackId>) -> bool {
        outgoing == Some(self.outgoing_track) && incoming == Some(self.incoming_track)
    }

    /// Apply a geometry, clamp it into the record, and re-derive the reasons.
    fn set_geometry(&mut self, start_beat: i64, length_beats: u32, style: TransitionStyle) {
        let start_beat = start_beat.clamp(self.earliest_beat(), self.latest_beat(length_beats));
        let Some(plan) = plan::evaluate(
            &self.outgoing,
            &self.incoming,
            start_beat,
            length_beats,
            style,
        ) else {
            // Only a nonsense tempo reaches here, and one cannot appear on a
            // transition that was planned from a real grid. Leaving the object
            // as it was beats replacing it with something incoherent.
            return;
        };
        self.plan = plan;
        self.edited = true;
    }

    /// The beat the playhead is on: the earliest a mix can begin.
    fn earliest_beat(&self) -> i64 {
        let beat_frames = self.beat_frames();
        ((self.outgoing.position - self.outgoing.grid_anchor) / beat_frames).floor() as i64
    }

    /// The last beat a transition of `length` can start on and still finish
    /// inside the record.
    fn latest_beat(&self, length: u32) -> i64 {
        let beat_frames = self.beat_frames();
        let last =
            ((self.outgoing.length - self.outgoing.grid_anchor) / beat_frames).floor() as i64;
        (last - i64::from(length)).max(self.earliest_beat())
    }

    /// Frames per beat of the outgoing record.
    ///
    /// The object cannot exist without a real tempo — [`Self::plan`] returns
    /// `None` first — so the fallback is unreachable rather than a policy. It
    /// is one beat at 120 BPM, which keeps a hypothetical division from
    /// producing an infinity that would then be clamped into a beat index.
    fn beat_frames(&self) -> f64 {
        let frames = self.outgoing.sample_rate.as_f64() * 60.0 / self.outgoing.bpm;
        if frames.is_finite() && frames > 0.0 {
            frames
        } else {
            self.outgoing.sample_rate.as_f64() / 2.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Mode, MusicalKey, Phrase, SampleRate};

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    fn beat() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).expect("a deck")
    }

    fn track(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    fn key(hour: u8, mode: Mode) -> Option<MusicalKey> {
        MusicalKey::new(hour, mode)
    }

    /// A transition out of a 400-beat record, 100 beats in, into a record at
    /// the same tempo and a neighbouring key.
    fn armed() -> Transition {
        let outgoing = Outgoing {
            position: 100.0 * beat(),
            length: 400.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: key(8, Mode::Minor),
            sample_rate: SR,
            grid_anchor: 0.0,
            breakdowns: Vec::new(),
        };
        let incoming = Incoming {
            position: 0.0,
            length: 300.0 * SR.as_f64(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: key(9, Mode::Minor),
            sample_rate: SR,
            grid_anchor: 0.0,
        };
        Transition::plan(
            (deck(1), deck(2)),
            (track(1), track(2)),
            outgoing,
            incoming,
            0.8,
        )
        .expect("the fixture must be mixable or it tests nothing")
    }

    /// **An edit re-derives the reasons.**
    ///
    /// The load-bearing test of this module. A transition moved four beats off
    /// its phrase boundary is no longer landing on a phrase, and the object
    /// must say the new true thing rather than keep the sentence the planner
    /// wrote about where it used to start. Everything §68 lists as a reader of
    /// this object — the waveform, the preview, the assistant — reads those
    /// reasons.
    #[test]
    fn moving_the_start_off_a_phrase_says_so() {
        let mut transition = armed();
        assert!(
            transition
                .plan
                .reasons
                .iter()
                .any(|r| matches!(r, plan::Reason::LandsOnPhrase { .. })),
            "the fixture did not start on a phrase, so it cannot test moving off one"
        );

        transition.move_start(4);

        assert!(
            transition
                .plan
                .reasons
                .iter()
                .any(|r| matches!(r, plan::Reason::LandsOnBar { .. })),
            "moved off the phrase boundary and still claimed to land on one: {:?}",
            transition.plan.reasons
        );
        assert!(
            transition.edited,
            "a moved transition was not marked edited"
        );
    }

    /// The frame position follows the beat, so a waveform marker moves with the
    /// number the panel prints.
    #[test]
    fn moving_the_start_moves_the_frame_with_it() {
        let mut transition = armed();
        let before = transition.plan.start_frame;
        transition.move_start(16);
        let moved = transition.plan.start_frame - before;
        assert!(
            (moved - 16.0 * beat()).abs() < 1.0,
            "sixteen beats moved the start by {moved} frames, not {}",
            16.0 * beat()
        );
    }

    /// **A mix cannot be planned into the past.**
    ///
    /// Dragging the start earlier stops at the playhead. Without the clamp the
    /// panel would happily show a mix starting thirty seconds ago, and the
    /// autopilot reading it would find the moment already gone.
    #[test]
    fn the_start_never_goes_behind_the_playhead() {
        let mut transition = armed();
        transition.move_start(-1_000);
        assert!(
            transition.plan.start_beat >= 100,
            "the mix was moved to beat {}, behind the playhead at 100",
            transition.plan.start_beat
        );
    }

    /// And it cannot be planned past the end of the record either.
    #[test]
    fn the_mix_always_ends_inside_the_record() {
        let mut transition = armed();
        transition.move_start(1_000);
        assert!(
            transition.plan.end_frame <= transition.plan.start_frame.max(400.0 * beat()),
            "the mix was planned to end past the end of the record"
        );
        assert!(
            transition.plan.end_frame <= 400.0 * beat() + 1.0,
            "the mix ends at frame {} of a {}-frame record",
            transition.plan.end_frame,
            400.0 * beat()
        );
    }

    /// Changing the length moves the end and leaves the start alone. A DJ
    /// shortening a blend means "finish sooner", not "start later".
    #[test]
    fn a_shorter_transition_keeps_its_start() {
        let mut transition = armed();
        let start = transition.plan.start_frame;
        transition.set_length(8);
        assert_eq!(transition.plan.length_beats, 8);
        assert!((transition.plan.start_frame - start).abs() < 1.0);
        assert!(
            (transition.plan.end_frame - (start + 8.0 * beat())).abs() < 1.0,
            "an eight-beat transition did not end eight beats after it started"
        );
    }

    /// Lengths outside what a transition can sensibly be are clamped, not
    /// taken. Zero beats is a cut the automix cannot run and 4 000 is longer
    /// than the record.
    #[test]
    fn absurd_lengths_are_clamped() {
        let mut transition = armed();
        transition.set_length(0);
        assert_eq!(transition.plan.length_beats, LENGTH_RANGE.0);
        transition.set_length(4_000);
        assert_eq!(transition.plan.length_beats, LENGTH_RANGE.1);
    }

    /// **A style the planner would not have chosen is still allowed.**
    ///
    /// The object reports; it does not veto. The reasons keep saying what the
    /// tempos and keys do, so a DJ blending two records that clash is told
    /// what they are taking on rather than prevented from taking it on.
    #[test]
    fn a_deliberate_style_survives_and_the_reasons_still_argue() {
        let outgoing = Outgoing {
            position: 100.0 * beat(),
            length: 400.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: key(8, Mode::Minor),
            sample_rate: SR,
            grid_anchor: 0.0,
            breakdowns: Vec::new(),
        };
        // 145 against 120 is well outside the deck's pitch range: the planner
        // cuts.
        let incoming = Incoming {
            position: 0.0,
            length: 300.0 * SR.as_f64(),
            bpm: 145.0,
            phrase: Phrase::new(16, 0),
            key: key(8, Mode::Minor),
            sample_rate: SR,
            grid_anchor: 0.0,
        };
        let mut transition = Transition::plan(
            (deck(1), deck(2)),
            (track(1), track(2)),
            outgoing,
            incoming,
            0.4,
        )
        .expect("a plan");
        assert_eq!(transition.plan.style, TransitionStyle::Cut);

        transition.set_style(TransitionStyle::Blend);

        assert_eq!(transition.plan.style, TransitionStyle::Blend);
        assert!(
            transition
                .plan
                .reasons
                .iter()
                .any(|r| matches!(r, plan::Reason::TemposClash { .. })),
            "the tempo clash stopped being reported once the DJ chose to blend"
        );
    }

    /// Replanning throws the edits away and says so.
    #[test]
    fn replanning_undoes_the_edits() {
        let mut transition = armed();
        let planned = transition.plan.clone();
        transition.move_start(4);
        transition.set_length(8);
        assert!(transition.edited);

        transition.replan();

        assert_eq!(transition.plan, planned);
        assert!(!transition.edited);
    }

    /// A transition knows when the records it describes have gone.
    #[test]
    fn a_transition_notices_a_deck_being_reloaded() {
        let transition = armed();
        assert!(transition.describes(Some(track(1)), Some(track(2))));
        assert!(!transition.describes(Some(track(1)), Some(track(9))));
        assert!(!transition.describes(None, Some(track(2))));
    }

    /// **A hand on the waveform lands on a beat.**
    ///
    /// §26's whole point: the DJ grabs the mix point rather than typing a
    /// number. What a pointer produces is a place in the record, and the beat
    /// it falls on is djmanzo's arithmetic — a drag that left the mix 40 ms
    /// off the grid would be a worse answer than the one the hand was giving.
    #[test]
    fn dragging_the_start_lands_on_the_nearest_beat() {
        let mut transition = armed();
        // Two thirds of the way through beat 200: nearer 200 than 201.
        transition.move_to_frame(200.4 * beat());
        assert_eq!(transition.plan.start_beat, 200);
        // And past the halfway point it rounds up rather than truncating,
        // which would put every drag a beat early.
        transition.move_to_frame(200.7 * beat());
        assert_eq!(transition.plan.start_beat, 201);
    }

    /// The clamps hold for a hand as they do for a button: a mix cannot be
    /// dragged into the past.
    #[test]
    fn dragging_the_start_behind_the_playhead_stops_at_it() {
        let mut transition = armed();
        transition.move_to_frame(0.0);
        assert!(
            transition.plan.start_beat >= 100,
            "a drag put the mix at beat {}, behind the playhead at 100",
            transition.plan.start_beat
        );
    }

    /// Dragging the far edge sets the length, and leaves the start alone.
    #[test]
    fn dragging_the_end_sets_the_length() {
        let mut transition = armed();
        let start = transition.plan.start_beat;
        transition.end_at_frame((start as f64 + 16.0) * beat());
        assert_eq!(transition.plan.length_beats, 16);
        assert_eq!(transition.plan.start_beat, start, "the start moved with it");
    }

    /// Dragging the end back past the start does not turn the transition
    /// inside out.
    #[test]
    fn dragging_the_end_past_the_start_clamps_rather_than_inverting() {
        let mut transition = armed();
        transition.end_at_frame(0.0);
        assert_eq!(transition.plan.length_beats, LENGTH_RANGE.0);
        assert!(transition.plan.end_frame > transition.plan.start_frame);
    }

    /// A frame that is not a number leaves the transition alone rather than
    /// moving it somewhere unrepresentable.
    #[test]
    fn a_nonsense_frame_moves_nothing() {
        let mut transition = armed();
        let before = transition.plan.clone();
        transition.move_to_frame(f64::NAN);
        transition.end_at_frame(f64::INFINITY);
        assert_eq!(transition.plan, before);
    }

    /// Confidence is the pair's, so editing where the mix happens leaves it
    /// alone. The rail and the pair view must not disagree about the same two
    /// records.
    #[test]
    fn editing_does_not_change_how_well_the_records_go_together() {
        let mut transition = armed();
        transition.move_start(8);
        transition.set_length(32);
        transition.set_style(TransitionStyle::Echo);
        assert!((transition.confidence - 0.8).abs() < f64::EPSILON);
    }
}

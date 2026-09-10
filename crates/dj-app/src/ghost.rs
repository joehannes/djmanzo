//! §27: what happens if this record comes in *here*, before anything loads.
//!
//! [§27 of the directive](../../../docs/DIRECTIVE.md) asks for a
//! non-destructive ghost overlay on a candidate, and says what it is for:
//!
//! > The DJ should understand: "If I bring this in here, this is what
//! > happens." before loading or committing.
//!
//! The load-bearing words are **before loading**. Everything djmanzo already
//! says about a mix — [`crate::transition`], the pair view, the automix — is
//! about two records that are *on decks*. This is about one record that is on
//! a deck and one that is still a row in the library, which is the moment a DJ
//! actually decides.
//!
//! # It is a ghost, not a preview
//!
//! An earlier reading of §27 had this waiting on an audio preview — a second
//! player, cueing the candidate into headphones. That is a different section's
//! job and a different piece of hardware. §27 says *display*, *overlay*,
//! *ghost*, and "make the future visible": it is asking for the mix to be
//! **drawn**, on the lane the DJ is already watching, in the colour reserved
//! for things djmanzo proposes rather than things that are so. Under
//! [ADR-0004](../../../docs/adr/0004-waveform-rendered-in-rust.md) the
//! webview does not draw a waveform, so what it is handed is positions —
//! frames on the outgoing record — and it lays elements over the tiles.
//!
//! # Nothing here decides a mix point
//!
//! The geometry is [`plan::plan`]'s, unedited. That matters more than it
//! looks: a ghost that worked out its own mix point would show a DJ one
//! transition and then perform a different one the moment they hit load. What
//! this adds is the two things the planner has no reason to know — where the
//! candidate's own first phrase lands once it is beat-matched onto that mix
//! point, and which of §27's seven questions djmanzo cannot answer at all.
//!
//! # The two it cannot answer, and why they are named rather than dropped
//!
//! §27 asks for the vocal entry and the drop. Nothing in `dj_analysis`
//! produces either, and no amount of arithmetic over a beat grid will: they
//! are questions about what the record *sounds* like. So they are carried as
//! [`Asked`] entries that answer `false`, derived from
//! [`dj_render::layer`] — the `vocal` and `drops` layers declaring themselves
//! undrawn is already the fact, and stating it twice is how the two drift
//! apart. An overlay that quietly showed five of seven marks would read as a
//! record with no vocal and no drop.

use dj_core::{KeyRelation, MusicalKey, Phrase, SampleRate};
use dj_render::layer::{self, Layer};

use crate::plan::{self, MixOut, Outgoing, Plan};

/// A record being considered, as the ghost needs it.
///
/// Deliberately not [`plan::Record`]: that type is about where a record can be
/// *left* and carries a length and no key, and this is about where one would
/// be *brought in*. Two narrow types cannot borrow each other's fields by
/// accident; one wide one invites exactly that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub key: Option<MusicalKey>,
    pub sample_rate: SampleRate,
    /// Frame position of a beat in the candidate, from which the rest follow.
    pub grid_anchor: f64,
}

/// Where the candidate's first full phrase would land.
///
/// Frames on the **outgoing** record, because that is the lane on screen. Once
/// the mix begins the two records are beat-matched, so a beat of the candidate
/// is a beat of the outgoing track and the conversion is one multiplication —
/// done here rather than in the interface, for the same reason
/// [`Plan::end_frame`] is carried rather than recomputed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    /// Frame on the outgoing record where that phrase begins.
    pub frame: f64,
    /// Beats of the candidate that play before it.
    ///
    /// Zero when the record opens on a phrase. Anything else is a pickup, and
    /// it is the number a DJ is really asking for: eight beats of lead-in
    /// means the mix has eight beats of introduction before the candidate
    /// says anything.
    pub lead_beats: f64,
    /// Whether that landing is still inside the transition.
    ///
    /// A phrase that arrives after the mix has finished is not an alignment —
    /// it is the candidate starting properly on its own, with the outgoing
    /// record already gone. Said rather than left for the reader to work out
    /// from two frame positions.
    pub within_mix: bool,
}

/// One of §27's seven, and whether djmanzo can draw it.
///
/// The directive's list as a type, for the reason `dj_render::layer` holds
/// §25's twenty: a list of seven in a document is a list that quietly stops
/// matching the code, and "five of seven" then becomes somebody's
/// recollection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// Where its first strong phrase would align.
    FirstPhrase,
    /// Where the vocal enters.
    VocalEntry,
    /// Where the drop occurs.
    Drop,
    /// Where the outgoing track becomes weak.
    OutgoingWeakens,
    /// The likely transition overlap.
    Overlap,
    /// The key relationship.
    KeyRelation,
    /// The BPM movement.
    TempoMovement,
}

/// §27's seven, in its order.
pub const ASKED: [Asked; 7] = [
    Asked::FirstPhrase,
    Asked::VocalEntry,
    Asked::Drop,
    Asked::OutgoingWeakens,
    Asked::Overlap,
    Asked::KeyRelation,
    Asked::TempoMovement,
];

impl Asked {
    /// The stable slug, so the interface can stamp it on an element.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Asked::FirstPhrase => "first-phrase",
            Asked::VocalEntry => "vocal-entry",
            Asked::Drop => "drop",
            Asked::OutgoingWeakens => "outgoing-weakens",
            Asked::Overlap => "overlap",
            Asked::KeyRelation => "key-relation",
            Asked::TempoMovement => "tempo-movement",
        }
    }

    /// What §27 asks for, in its own words.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Asked::FirstPhrase => "where its first strong phrase would align",
            Asked::VocalEntry => "where the vocal enters",
            Asked::Drop => "where the drop occurs",
            Asked::OutgoingWeakens => "where the outgoing track becomes weak",
            Asked::Overlap => "likely transition overlap",
            Asked::KeyRelation => "key relationship",
            Asked::TempoMovement => "BPM movement",
        }
    }

    /// The waveform layer this needs, when it is a mark rather than a number.
    ///
    /// Key and tempo are arithmetic printed in words; the other five are
    /// positions on a record, and a position nothing draws is a position
    /// nobody sees.
    #[must_use]
    pub const fn layer(self) -> Option<&'static str> {
        match self {
            Asked::FirstPhrase => Some("phrases"),
            Asked::VocalEntry => Some("vocal"),
            Asked::Drop => Some("drops"),
            Asked::OutgoingWeakens => Some("mix-out"),
            Asked::Overlap => Some("seam"),
            Asked::KeyRelation | Asked::TempoMovement => None,
        }
    }

    /// Whether djmanzo can answer this yet.
    ///
    /// Derived from the layer table rather than written down here. The day an
    /// analyser finds vocals and `vocal` stops being `Drawn::Nowhere`, this
    /// answers `true` by itself — and [`Ghost`] will owe a position to go with
    /// it, which is what the count test below is there to say out loud.
    #[must_use]
    pub fn answered(self) -> bool {
        self.layer()
            .is_none_or(|name| layer::layer(name).is_some_and(Layer::exists))
    }
}

/// What §27 asks for that djmanzo cannot see.
#[must_use]
pub fn unseen() -> Vec<Asked> {
    ASKED.into_iter().filter(|a| !a.answered()).collect()
}

/// The future, as far as djmanzo can honestly draw it.
#[derive(Debug, Clone, PartialEq)]
pub struct Ghost {
    /// The transition djmanzo would propose. Everything else describes it.
    pub plan: Plan,
    /// Where the candidate's first full phrase lands. `None` when the
    /// candidate has no phrase structure, which is a real answer.
    pub landing: Option<Landing>,
    /// Where the outgoing record becomes weak.
    ///
    /// [`plan::mix_out`]'s window, the same one the lane already draws — not a
    /// second opinion about the same end of the same record. Carried so the
    /// ghost answers §27 on its own, rather than depending on whether some
    /// other panel happens to be open.
    pub weakens: Option<MixOut>,
    /// How far the incoming deck's pitch must move, as a percentage.
    ///
    /// Signed the way the fader moves: negative pulls a faster record down.
    /// [`Plan::bpm_delta`] says the same thing in BPM, and both are wanted —
    /// "+3 BPM" is what the record is, "-2.3%" is what the hand does.
    pub pitch_percent: f64,
    /// §27's seven, and whether each is answered.
    pub asked: Vec<(Asked, bool)>,
}

impl Ghost {
    /// The stretch the two records would share, in outgoing frames.
    #[must_use]
    pub const fn overlap(&self) -> (f64, f64) {
        (self.plan.start_frame, self.plan.end_frame)
    }

    /// How the two keys stand, when both are known.
    #[must_use]
    pub const fn keys(&self) -> Option<KeyRelation> {
        self.plan.key_relation
    }

    /// What §27 asks for that this ghost cannot draw.
    #[must_use]
    pub fn unseen(&self) -> Vec<Asked> {
        self.asked
            .iter()
            .filter(|(_, answered)| !answered)
            .map(|(a, _)| *a)
            .collect()
    }
}

/// Draw the future of `out` meeting `candidate`.
///
/// `None` for exactly the reasons [`plan::plan`] answers `None` — no grid, a
/// tempo that is not a tempo, a record already past its last usable phrase.
/// A ghost that appeared anyway, over a mix that cannot happen, would be the
/// one thing worse than no ghost.
#[must_use]
pub fn look(out: &Outgoing, candidate: &Candidate) -> Option<Ghost> {
    let into = plan::Incoming {
        bpm: candidate.bpm,
        phrase: candidate.phrase,
        key: candidate.key,
    };
    let plan = plan::plan(out, &into)?;
    let out_beat = plan::beat_frames(out.bpm, out.sample_rate)?;

    Some(Ghost {
        landing: landing(&plan, candidate, out_beat),
        weakens: plan::mix_out(&out.record()),
        pitch_percent: pitch_percent(out.bpm, candidate.bpm),
        asked: ASKED.into_iter().map(|a| (a, a.answered())).collect(),
        plan,
    })
}

/// Where the candidate's first full phrase falls on the outgoing record.
///
/// The candidate is brought in from its beginning, so the question is which of
/// its beats is both **audible** — at or after frame zero, since a beat grid
/// extends backwards from its anchor and the beats before the file starts are
/// arithmetic rather than music — and starts a phrase.
fn landing(plan: &Plan, candidate: &Candidate, out_beat: f64) -> Option<Landing> {
    let phrase = candidate.phrase?;
    let beat = plan::beat_frames(candidate.bpm, candidate.sample_rate)?;

    #[allow(clippy::cast_possible_truncation)]
    let first_audible = (-candidate.grid_anchor / beat).ceil() as i64;
    let within = phrase.beat_within(first_audible);
    let to_go = if within == 0 {
        0
    } else {
        phrase.beats - within
    };
    let boundary = first_audible + i64::from(to_go);

    // Beats of the candidate before that phrase, counted from its first
    // sample rather than from its grid anchor: what plays is what the DJ
    // hears, and the anchor is usually not at zero.
    #[allow(clippy::cast_precision_loss)]
    let lead_beats = (candidate.grid_anchor + boundary as f64 * beat) / beat;
    if !lead_beats.is_finite() || lead_beats < 0.0 {
        return None;
    }

    let frame = plan.start_frame + lead_beats * out_beat;
    Some(Landing {
        frame,
        lead_beats,
        within_mix: frame <= plan.end_frame,
    })
}

/// How far the incoming deck's pitch must move to match `out`.
///
/// Zero when either tempo is not a tempo, rather than an infinity or a NaN
/// travelling into the interface as a percentage.
fn pitch_percent(out_bpm: f64, candidate_bpm: f64) -> f64 {
    if !(out_bpm.is_finite() && candidate_bpm.is_finite()) || candidate_bpm <= 0.0 {
        return 0.0;
    }
    let percent = (out_bpm / candidate_bpm - 1.0) * 100.0;
    if percent.is_finite() { percent } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Mode;

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    /// Frames per beat, derived rather than written down — the same trap
    /// `plan`'s tests document. A literal here is a fixture that disagrees
    /// with the planner and looks like a bug in the planner.
    fn beat() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    /// A record 64 beats in, 256 beats long, on a 16-beat phrase.
    fn outgoing() -> Outgoing {
        Outgoing {
            position: 64.0 * beat(),
            length: 256.0 * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: Some(key(8, Mode::Minor)),
            sample_rate: SR,
            grid_anchor: 0.0,
        }
    }

    /// A candidate whose grid starts `anchor_beats` into the file and whose
    /// phrase begins on beat `phrase_anchor` of the grid.
    fn candidate(bpm: f64, anchor_beats: f64, phrase_anchor: u32) -> Candidate {
        Candidate {
            bpm,
            phrase: Phrase::new(16, phrase_anchor),
            key: Some(key(8, Mode::Minor)),
            sample_rate: SR,
            grid_anchor: anchor_beats * (SR.as_f64() * 60.0 / bpm),
        }
    }

    /// **A record that opens exactly on a phrase has no lead-in.**
    ///
    /// The base case, and the one that makes every other number readable: if
    /// this is not zero, nothing else in the landing means what it says.
    #[test]
    fn a_record_that_opens_on_a_phrase_leads_with_nothing() {
        let ghost = look(&outgoing(), &candidate(BPM, 0.0, 0)).expect("a ghost");
        let landing = ghost.landing.expect("a landing");
        assert!(
            landing.lead_beats.abs() < 1e-6,
            "{} beats of lead-in on a record that opens on a phrase",
            landing.lead_beats
        );
        assert!(
            (landing.frame - ghost.plan.start_frame).abs() < 1e-6,
            "the phrase should land where the mix begins"
        );
        assert!(landing.within_mix);
    }

    /// **A four-beat pickup is four beats of lead-in, drawn four beats late.**
    ///
    /// The case §27 is really about: the DJ is told the candidate says nothing
    /// for the first four beats of the mix, and shown where it starts saying
    /// something. Both numbers, because one without the other is a mark with
    /// no explanation or an explanation with no mark.
    #[test]
    fn a_pickup_pushes_the_first_phrase_later_by_exactly_its_length() {
        // Grid anchored at the first sample; the phrase starts on beat 4 of
        // it, which is a four-beat pickup.
        let ghost = look(&outgoing(), &candidate(BPM, 0.0, 4)).expect("a ghost");
        let landing = ghost.landing.expect("a landing");
        assert!(
            (landing.lead_beats - 4.0).abs() < 1e-6,
            "expected 4 beats of pickup, got {}",
            landing.lead_beats
        );
        let expected = ghost.plan.start_frame + 4.0 * beat();
        assert!(
            (landing.frame - expected).abs() < 1e-3,
            "the phrase landed at {} rather than {expected}",
            landing.frame
        );
    }

    /// **The lead-in is counted from the first sample, not from the grid.**
    ///
    /// A grid anchored two beats into the file means two beats of music
    /// before beat zero, and a DJ hears those. Counting from the anchor would
    /// under-report every record whose analyser did not find a downbeat at
    /// the very start, which is most of them.
    #[test]
    fn the_lead_in_counts_the_music_before_the_grid_anchor() {
        let ghost = look(&outgoing(), &candidate(BPM, 2.0, 0)).expect("a ghost");
        let landing = ghost.landing.expect("a landing");
        assert!(
            (landing.lead_beats - 2.0).abs() < 1e-6,
            "expected the 2 beats before the anchor to count, got {}",
            landing.lead_beats
        );
    }

    /// **The candidate's beats, converted at the outgoing record's tempo.**
    ///
    /// The mix is beat-matched, so a beat of the candidate lasts an outgoing
    /// beat however fast the file itself is. Converting at the candidate's own
    /// tempo would put the mark in the wrong place on the only lane on screen,
    /// and it would be wrong by more the further the two tempos are apart.
    #[test]
    fn a_faster_candidate_still_lands_on_the_outgoing_grid() {
        let slow = look(&outgoing(), &candidate(BPM, 0.0, 8)).expect("a ghost");
        let fast = look(&outgoing(), &candidate(BPM * 1.05, 0.0, 8)).expect("a ghost");
        let (slow, fast) = (
            slow.landing.expect("a landing"),
            fast.landing.expect("a landing"),
        );
        assert!((slow.lead_beats - 8.0).abs() < 1e-6);
        assert!((fast.lead_beats - 8.0).abs() < 1e-6);
        assert!(
            (slow.frame - fast.frame).abs() < 1e-3,
            "eight beats of pickup is eight outgoing beats at either tempo: \
             {} against {}",
            slow.frame,
            fast.frame
        );
    }

    /// A record with no phrase structure gets no landing rather than a
    /// pretended one. Plenty of records have none.
    #[test]
    fn no_phrase_structure_is_no_landing() {
        let mut none = candidate(BPM, 0.0, 0);
        none.phrase = None;
        let ghost = look(&outgoing(), &none).expect("a ghost");
        assert_eq!(ghost.landing, None);
    }

    /// **A long pickup is said to fall outside the mix.**
    ///
    /// Thirty-two beats of introduction against a sixteen-beat blend is the
    /// candidate arriving after the outgoing record has gone, which is a
    /// different thing from an alignment and is worth saying.
    #[test]
    fn a_phrase_after_the_mix_ends_is_not_an_alignment() {
        let ghost = look(&outgoing(), &candidate(BPM, 0.0, 0)).expect("a ghost");
        // A 64-beat phrase whose boundary is 48 beats in: a very long
        // introduction, against a mix the planner will make 32 beats.
        let long = Candidate {
            phrase: Phrase::new(64, 48),
            ..candidate(BPM, 0.0, 0)
        };
        let late = look(&outgoing(), &long).expect("a ghost");
        assert!(ghost.landing.expect("a landing").within_mix);
        let landing = late.landing.expect("a landing");
        assert!(landing.lead_beats > f64::from(late.plan.length_beats));
        assert!(
            !landing.within_mix,
            "a phrase {} beats in against a {}-beat mix is not inside it",
            landing.lead_beats, late.plan.length_beats
        );
    }

    /// **The pitch move is what the hand does, and it is signed.**
    ///
    /// A faster record is pulled *down* onto the outgoing tempo. Getting the
    /// sign wrong here would put a minus sign on a fader that goes up.
    #[test]
    fn a_faster_record_is_pulled_down() {
        let up = look(&outgoing(), &candidate(BPM * 1.02, 0.0, 0)).expect("a ghost");
        let down = look(&outgoing(), &candidate(BPM / 1.02, 0.0, 0)).expect("a ghost");
        assert!(
            up.pitch_percent < 0.0,
            "a 122 BPM record onto 120 is pulled down, not up: {}",
            up.pitch_percent
        );
        assert!(down.pitch_percent > 0.0);
        assert!((up.pitch_percent + 2.0).abs() < 0.1, "{}", up.pitch_percent);
        assert!(
            up.plan.bpm_delta > 0.0,
            "the record is still the faster one"
        );
    }

    /// A tempo that is not a tempo produces no percentage rather than an
    /// infinity travelling into the interface.
    #[test]
    fn an_impossible_tempo_is_not_a_percentage() {
        assert!((pitch_percent(BPM, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((pitch_percent(BPM, f64::NAN) - 0.0).abs() < f64::EPSILON);
        assert!((pitch_percent(f64::INFINITY, BPM) - 0.0).abs() < f64::EPSILON);
    }

    /// §27 asks for seven things. If this is six, one was dropped rather than
    /// decided.
    #[test]
    fn the_directive_asks_for_seven_and_so_does_this() {
        assert_eq!(ASKED.len(), 7);
        let slugs: std::collections::BTreeSet<&str> = ASKED.into_iter().map(Asked::slug).collect();
        assert_eq!(slugs.len(), 7, "two of the seven share a slug");
        // The directive's own words, so two entries saying the same thing
        // means one of the seven was transcribed over another.
        let words: std::collections::BTreeSet<&str> = ASKED.into_iter().map(Asked::about).collect();
        assert_eq!(words.len(), 7, "two of the seven read the same");
        for asked in ASKED {
            assert!(!asked.about().is_empty(), "{} says nothing", asked.slug());
        }
    }

    /// **Five of seven, and the two that are missing are named.**
    ///
    /// The count worth quoting. It is also the alarm: when an analyser finds
    /// vocals and the `vocal` layer starts being drawn, this fails — and what
    /// it is asking for is a *frame* in [`Ghost`] to go with the layer, not a
    /// new number here.
    #[test]
    fn what_djmanzo_cannot_see_is_named_rather_than_left_out() {
        let missing: Vec<&str> = unseen().into_iter().map(Asked::slug).collect();
        assert_eq!(
            missing,
            vec!["vocal-entry", "drop"],
            "the answered set changed: give Ghost the positions to match"
        );
        let ghost = look(&outgoing(), &candidate(BPM, 0.0, 0)).expect("a ghost");
        assert_eq!(ghost.asked.len(), 7);
        assert_eq!(
            ghost
                .unseen()
                .into_iter()
                .map(Asked::slug)
                .collect::<Vec<_>>(),
            missing,
            "the ghost and the module disagree about what is missing"
        );
    }

    /// **The ghost's geometry is the planner's, to the frame.**
    ///
    /// The rule the module exists to keep: what a DJ is shown before loading
    /// is what djmanzo performs after. Two mix points would make the ghost a
    /// lie told confidently.
    #[test]
    fn the_ghost_shows_the_transition_djmanzo_would_actually_do() {
        let out = outgoing();
        let cand = candidate(BPM, 0.0, 0);
        let ghost = look(&out, &cand).expect("a ghost");
        let planned = plan::plan(
            &out,
            &plan::Incoming {
                bpm: cand.bpm,
                phrase: cand.phrase,
                key: cand.key,
            },
        )
        .expect("a plan");
        assert_eq!(ghost.plan, planned);
        assert_eq!(ghost.overlap(), (planned.start_frame, planned.end_frame));
        assert_eq!(ghost.keys(), planned.key_relation);
    }

    /// The mix-out window is the lane's, not a second opinion.
    #[test]
    fn where_the_record_becomes_weak_is_the_window_already_drawn() {
        let out = outgoing();
        let ghost = look(&out, &candidate(BPM, 0.0, 0)).expect("a ghost");
        assert_eq!(ghost.weakens, plan::mix_out(&out.record()));
        assert!(ghost.weakens.is_some(), "256 beats has room to be left in");
    }

    /// No grid, no ghost. A record the planner will not plan for is a record
    /// the overlay must not appear over.
    #[test]
    fn a_mix_that_cannot_happen_gets_no_ghost() {
        let mut nearly_over = outgoing();
        nearly_over.position = nearly_over.length - 2.0 * beat();
        assert_eq!(look(&nearly_over, &candidate(BPM, 0.0, 0)), None);
    }
}

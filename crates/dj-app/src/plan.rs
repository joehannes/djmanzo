//! Where to start a mix, how long to take, and which way to do it.
//!
//! The suggester in `dj_library::suggest` answers *what* to play next. This
//! answers the two questions that follow, and they are not the same question:
//! a record that is a perfect harmonic match is still a bad mix if it comes in
//! four beats before the outgoing track's phrase ends.
//!
//! Beside `automix` rather than inside it on purpose. Automix *runs* a
//! transition on a fixed style the DJ chose in advance; this *decides* one from
//! what the two tracks actually are. Keeping them apart means the planner can
//! be asked for an opinion without anything moving, which is what a DJ wants
//! when the answer is going on a screen rather than into the mix.
//!
//! # Reasons are typed, as everywhere else
//!
//! Same principle as the suggester and as
//! [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md): a plan
//! that says `Reason::LandsOnPhrase { beat: 384 }` can be checked against the
//! waveform; one that says "sounds good here" cannot.
//!
//! # What it cannot know
//!
//! **Where the outgoing track's outro actually is.** A phrase boundary near the
//! end is the best structural guess available, and it is a guess: plenty of
//! records have a vocal over the last sixteen bars, and no analysis here can
//! hear that. The plan says which boundary it chose and how much track is left,
//! so the DJ can disagree with the specific thing rather than the whole answer.
//!
//! **Whether the two records suit each other musically.** Key and tempo are
//! arithmetic; taste is not.

use dj_core::{KeyRelation, MusicalKey, Phrase, SampleRate, action::TransitionStyle};

/// The track going out, as the planner needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outgoing {
    /// Where the playhead is now, in frames.
    pub position: f64,
    /// Total length in frames.
    pub length: f64,
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub key: Option<MusicalKey>,
    pub sample_rate: SampleRate,
    /// Frame position of a beat, from which every other beat follows.
    pub grid_anchor: f64,
}

/// A record, as the mix-out window needs it.
///
/// Deliberately less than [`Outgoing`]: no playhead, no key. Where a record
/// can be left is a fact about its shape, and a type that cannot express
/// "where the playhead is" cannot come to depend on it by accident.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    /// Total length in frames.
    pub length: f64,
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub sample_rate: SampleRate,
    /// Frame position of a beat, from which every other beat follows.
    pub grid_anchor: f64,
}

impl Outgoing {
    /// Everything about the record, with the playhead dropped.
    #[must_use]
    pub const fn record(&self) -> Record {
        Record {
            length: self.length,
            bpm: self.bpm,
            phrase: self.phrase,
            sample_rate: self.sample_rate,
            grid_anchor: self.grid_anchor,
        }
    }
}

/// The track coming in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Incoming {
    pub bpm: f64,
    pub phrase: Option<Phrase>,
    pub key: Option<MusicalKey>,
}

/// Why the planner chose what it chose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reason {
    /// The mix starts on a phrase boundary of the outgoing track.
    LandsOnPhrase { beat: i64 },
    /// No phrase structure, so the start is a bar line instead. Weaker, and
    /// said so rather than presented as the same thing.
    LandsOnBar { beat: i64 },
    /// How much of the outgoing track is left after the mix starts.
    Remaining { beats: f64 },
    /// The tempos are close enough to ride the pitch fader.
    TemposMatch { from: f64, to: f64 },
    /// Too far apart to blend; the plan cuts instead.
    TemposClash { from: f64, to: f64 },
    /// The keys sit together.
    KeysMatch,
    /// The keys fight, so a long blend would expose it.
    KeysClash,
    /// Not enough track left *after* the transition ends for the mix to be
    /// late into, which is the margin a human pressing a button needs.
    Rushed { beats_after: f64 },
}

/// A proposed transition.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Beat index in the outgoing track, counted from its grid anchor, where
    /// the mix should begin.
    pub start_beat: i64,
    /// Frame position of that beat, so the interface can draw it without
    /// redoing the arithmetic and landing a pixel off.
    pub start_frame: f64,
    /// Frame position where the transition finishes, for the same reason.
    ///
    /// Carried rather than left to the reader: start plus length beats is one
    /// multiplication, and it is the multiplication that decides where a
    /// waveform marker lands. Two callers doing it separately is two chances
    /// to do it against the wrong tempo.
    pub end_frame: f64,
    /// How long the transition runs, in beats.
    pub length_beats: u32,
    pub style: TransitionStyle,
    /// Incoming tempo minus outgoing, in BPM. Signed, because which way the
    /// tempo moves is what a DJ's hand does on the pitch fader.
    pub bpm_delta: f64,
    /// How the two keys stand to each other, or `None` when either is
    /// unanalysed. Unknown is not a clash -- see below.
    pub key_relation: Option<KeyRelation>,
    pub reasons: Vec<Reason>,
}

/// Transition lengths the planner will propose, longest first.
///
/// Whole phrases, not round numbers: a 24-beat blend ends in the middle of a
/// phrase, which is the thing this module exists to avoid. Thirty-two is a
/// comfortable DJ blend, sixteen is brisk, eight is nearly a cut.
const LENGTHS: [u32; 3] = [32, 16, 8];

/// How close two tempos must be to blend rather than cut.
///
/// Six percent, matching the deck's comfortable pitch range in
/// `dj_library::suggest` -- the same physical constraint, so the two must not
/// drift apart. Beyond it a long blend means two audibly different tempos
/// running together for eight bars.
const TEMPO_TOLERANCE: f64 = 0.06;

/// Beats of outgoing track that must remain *after* the transition ends.
///
/// Zero would mean planning a mix that finishes exactly as the file runs out,
/// which leaves no room for the transition to be late -- and it always is, by a
/// beat or two, because a human presses the button.
const TAIL_MARGIN: f64 = 8.0;

/// Plan a transition from `out` into `into`.
///
/// `None` when there is nothing sensible to propose: no grid, a track already
/// past its last usable phrase, or a length that will not fit. A planner that
/// always answers is a planner that answers wrongly near the end of a record,
/// which is exactly when it is being read.
#[must_use]
pub fn plan(out: &Outgoing, into: &Incoming) -> Option<Plan> {
    plan_as(out, into, None)
}

/// Plan a transition, joining the records the way this DJ usually does.
///
/// `usual` is §81's learned transition style for the kind of night the DJ has
/// named — `None` until they have named one and djmanzo has seen enough of
/// them, which is most installations most of the time and is why [`plan`]
/// exists as the short form rather than as a different answer.
///
/// **It decides only where the music has not.** See `STYLE_IS_TASTE` and
/// `choose_style`: a profile picks between styles that all work on a pair whose
/// tempos and keys agree, and it never touches the cut a mismatched tempo
/// demands or the echo a key clash asks for. A preference that could overrule
/// either would be three previous evenings outvoting the two records in front
/// of the DJ.
#[must_use]
pub fn plan_as(out: &Outgoing, into: &Incoming, usual: Option<TransitionStyle>) -> Option<Plan> {
    let beat_frames = beat_frames(out.bpm, out.sample_rate)?;
    let remaining_beats = (out.length - out.position) / beat_frames;
    if !remaining_beats.is_finite() || remaining_beats <= TAIL_MARGIN {
        return None;
    }

    // Tempo and key first: they decide the style, and the style decides how
    // long the mix wants to be before the track's remaining length trims it.
    let tempos_match = ratio_within(out.bpm, into.bpm, TEMPO_TOLERANCE);
    let keys_match = relation(out, into).is_none_or(KeyRelation::mixes);
    let style = choose_style(tempos_match, keys_match, usual);

    // The longest transition that leaves the tail margin intact.
    let usable = remaining_beats - TAIL_MARGIN;
    #[allow(clippy::cast_precision_loss)]
    let length = LENGTHS
        .into_iter()
        .find(|&l| f64::from(l) <= usable)
        .unwrap_or(*LENGTHS.last().expect("LENGTHS is not empty"));

    // Where. The last boundary that still leaves room for the whole
    // transition, so the mix ends before the track does rather than being cut
    // off by it.
    let current = ((out.position - out.grid_anchor) / beat_frames).floor() as i64;
    #[allow(clippy::cast_possible_truncation)]
    let last_usable = current + (usable - f64::from(length)).max(0.0) as i64;

    let start_beat = match out.phrase {
        Some(phrase) => {
            let within = i64::from(phrase.beat_within(last_usable));
            let boundary = last_usable - within;
            // Never behind the playhead: a plan to start the mix in the past is
            // not a plan.
            if boundary < current {
                current
            } else {
                boundary
            }
        }
        None => last_usable,
    };

    evaluate(out, into, start_beat, length, style)
}

/// The stretch of a record in which a mix out of it can begin.
///
/// §25's `mix-out` layer — *where this record could be left, structurally*.
///
/// It is a property of the **record**, not of a pair and not of the playhead.
/// That is the whole reason it can be drawn on a waveform with no transition
/// planned on it, and it is why the band does not creep along under a DJ who
/// is watching it: the planner's *choice* moves as the playhead advances; the
/// record's shape does not.
///
/// # Where the two edges come from
///
/// Both are [`plan`]'s own arithmetic rather than a second opinion, which is
/// why this lives beside it. The window **opens** at the last beat where the
/// longest transition the planner will propose still leaves `TAIL_MARGIN`
/// intact, and **closes** at the last beat where the shortest one does.
/// Inside it every length djmanzo would suggest fits somewhere. Before it, a
/// DJ is leaving record on the table; after it, whatever they start is
/// [`Reason::Rushed`] — the same fact, said the same way, from the same
/// constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixOut {
    /// Beat index where the window opens, counted from the grid anchor.
    pub opens_beat: i64,
    /// Frame position of that beat.
    pub opens_frame: f64,
    /// Frame position of the last beat a transition may begin on.
    pub closes_frame: f64,
    /// True when the opening is a phrase boundary rather than merely a beat.
    ///
    /// Separate from the frames for the same reason [`Reason::LandsOnPhrase`]
    /// and [`Reason::LandsOnBar`] are separate reasons: a window that opens on
    /// real structure is a stronger claim than one that opens on arithmetic,
    /// and dressing the second as the first is the kind of confident lie this
    /// module exists to avoid.
    pub on_phrase: bool,
}

/// Where `record` could be left, structurally. `None` when there is nowhere.
///
/// `None` when the tempo is not a tempo, or when there is no room: a record
/// with fewer beats left than the shortest transition plus the tail margin has
/// nowhere to be mixed out of, and an eight-bar loop is not a record you leave.
#[must_use]
pub fn mix_out(record: &Record) -> Option<MixOut> {
    let out = record;
    let beat_frames = beat_frames(out.bpm, out.sample_rate)?;
    let last_beat = (out.length - out.grid_anchor) / beat_frames;
    if !last_beat.is_finite() {
        return None;
    }

    let longest = f64::from(LENGTHS[0]);
    let shortest = f64::from(*LENGTHS.last().expect("LENGTHS is not empty"));
    #[allow(clippy::cast_possible_truncation)]
    let closes_beat = (last_beat - TAIL_MARGIN - shortest).floor() as i64;
    if closes_beat <= 0 {
        return None;
    }
    // Clamped rather than refused: a record too short for the longest blend
    // still has somewhere to be left, and the window simply starts at its
    // beginning. Refusing here would hide the answer for exactly the records
    // where the margin is tightest.
    #[allow(clippy::cast_possible_truncation)]
    let target = (last_beat - TAIL_MARGIN - longest).max(0.0).floor() as i64;

    let (opens_beat, on_phrase) = match out.phrase {
        // Snapped back, not forward: a window that opens later than it could
        // is a window that hides usable record. Never behind the start of the
        // grid, where there is no audio to open on.
        Some(phrase) => match target - i64::from(phrase.beat_within(target)) {
            boundary if boundary >= 0 => (boundary, true),
            _ => (target, false),
        },
        None => (target, false),
    };
    if closes_beat <= opens_beat {
        return None;
    }

    #[allow(clippy::cast_precision_loss)]
    Some(MixOut {
        opens_beat,
        opens_frame: out.grid_anchor + opens_beat as f64 * beat_frames,
        closes_frame: out.grid_anchor + closes_beat as f64 * beat_frames,
        on_phrase,
    })
}

/// The stretch of a record in which a mix **into** it can begin.
///
/// §25's `mix-in` layer — *where a record could be brought in* — and the other
/// half of [`MixOut`]. Both are properties of the **record**: one says where it
/// can be left, this says where it can be joined, and neither moves with the
/// playhead or with whatever is on the other deck.
///
/// # Where the two edges come from
///
/// It **opens** at the record's first phrase boundary at or after the grid
/// anchor. Bringing a record in against the middle of its first musical idea is
/// the same mistake [`plan`] exists to avoid at the other end, and there is no
/// audio before the anchor to open on.
///
/// It **closes** at the record's **first drop**, snapped back to a phrase. A
/// mix that starts after the drop has thrown the drop away, and the drop is
/// usually the thing the whole transition was building towards. Where there is
/// no drop — a record that never has one, or one nobody has analysed — the
/// close is the longest transition the planner will propose, measured from the
/// opening: *its first eight bars*, which is arithmetic rather than structure.
/// [`MixIn::before_a_drop`] says which of the two it is, for the same reason
/// [`MixOut::on_phrase`] says whether the opening is real structure. A window
/// that dressed the arithmetic as the music would be the confident lie this
/// module is written not to tell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixIn {
    /// Beat index where the window opens, counted from the grid anchor.
    pub opens_beat: i64,
    /// Frame position of that beat.
    pub opens_frame: f64,
    /// Frame position of the last beat a mix into this record may begin on.
    pub closes_frame: f64,
    /// True when the opening is a phrase boundary rather than merely a beat.
    pub on_phrase: bool,
    /// True when the close is the record's first drop rather than arithmetic.
    pub before_a_drop: bool,
}

/// Where a mix into `record` can begin.
///
/// `first_drop` is a frame position from `dj_analysis::energy::Trajectory`, or
/// `None` where nothing has found one — see [`MixIn`] for what each answer
/// makes the window mean.
///
/// `None` for a record with no usable grid, and for one whose window would
/// close before it opened: a record whose drop is in its first phrase has no
/// stretch to be brought in over, and a band drawn backwards is worse than no
/// band.
#[must_use]
pub fn mix_in(record: &Record, first_drop: Option<f64>) -> Option<MixIn> {
    let beat_frames = beat_frames(record.bpm, record.sample_rate)?;
    let last_beat = (record.length - record.grid_anchor) / beat_frames;
    if !last_beat.is_finite() || last_beat <= 0.0 {
        return None;
    }

    // Forward, not back: the opening is where the record's structure starts,
    // and snapping back would open the window before the grid.
    let (opens_beat, on_phrase) = match record.phrase {
        Some(phrase) => {
            let within = i64::from(phrase.beat_within(0));
            if within == 0 {
                (0, true)
            } else {
                (i64::from(phrase.beats) - within, true)
            }
        }
        None => (0, false),
    };

    let longest = f64::from(LENGTHS[0]);
    #[allow(clippy::cast_precision_loss)]
    let by_arithmetic = opens_beat as f64 + longest;
    let closes_beat = match first_drop {
        // Snapped back to a phrase, so the window closes on structure rather
        // than a beat or two into it — the same rule the opening follows.
        Some(frame) => {
            let at = (frame - record.grid_anchor) / beat_frames;
            if !at.is_finite() {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            let at = at.floor() as i64;
            match record.phrase {
                Some(phrase) => at - i64::from(phrase.beat_within(at)),
                None => at,
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        None => by_arithmetic.floor() as i64,
    };

    // Never past the end of the record, and never at or before the opening.
    #[allow(clippy::cast_possible_truncation)]
    let closes_beat = closes_beat.min(last_beat.floor() as i64);
    if closes_beat <= opens_beat {
        return None;
    }

    #[allow(clippy::cast_precision_loss)]
    Some(MixIn {
        opens_beat,
        opens_frame: record.grid_anchor + opens_beat as f64 * beat_frames,
        closes_frame: record.grid_anchor + closes_beat as f64 * beat_frames,
        on_phrase,
        before_a_drop: first_drop.is_some(),
    })
}

/// Say what a *particular* transition means, rather than choosing one.
///
/// [`plan`] decides where the mix starts, how long it runs and which way to do
/// it; this describes the consequences of that choice. They are separate
/// because a transition is an object a DJ can **edit** — pushed a phrase later,
/// shortened to sixteen beats, cut instead of blended — and an edited
/// transition still has to be able to say why it is what it is.
///
/// Re-deriving the reasons over the new geometry is the only way to keep the
/// explanation true. The alternative — keeping the reasons the planner
/// produced — is an interface confidently stating that a mix lands on a phrase
/// boundary that the DJ has just moved it off.
///
/// `None` only when the outgoing tempo is not a tempo. Everything else about
/// the geometry is the caller's to decide, including choices the planner would
/// not have made: this reports, it does not veto.
#[must_use]
pub fn evaluate(
    out: &Outgoing,
    into: &Incoming,
    start_beat: i64,
    length_beats: u32,
    style: TransitionStyle,
) -> Option<Plan> {
    let beat_frames = beat_frames(out.bpm, out.sample_rate)?;
    let mut reasons = Vec::new();

    let tempos_match = ratio_within(out.bpm, into.bpm, TEMPO_TOLERANCE);
    reasons.push(if tempos_match {
        Reason::TemposMatch {
            from: out.bpm,
            to: into.bpm,
        }
    } else {
        Reason::TemposClash {
            from: out.bpm,
            to: into.bpm,
        }
    });

    let key_relation = relation(out, into);
    if let Some(relation) = key_relation {
        reasons.push(if relation.mixes() {
            Reason::KeysMatch
        } else {
            Reason::KeysClash
        });
    }

    #[allow(clippy::cast_precision_loss)]
    let start_frame = out.grid_anchor + start_beat as f64 * beat_frames;
    let end_frame = start_frame + f64::from(length_beats) * beat_frames;

    // Rushed is about what is left *after the mix ends*, not after it starts:
    // a transition that finishes four beats before the file does has nowhere
    // to be late into, which is the situation the margin exists to prevent.
    let after = (out.length - end_frame) / beat_frames;
    if after < TAIL_MARGIN {
        reasons.push(Reason::Rushed {
            beats_after: after.max(0.0),
        });
    }

    reasons.push(match out.phrase {
        Some(p) if p.starts_at(start_beat) => Reason::LandsOnPhrase { beat: start_beat },
        _ => Reason::LandsOnBar { beat: start_beat },
    });
    reasons.push(Reason::Remaining {
        beats: (out.length - start_frame) / beat_frames,
    });

    Some(Plan {
        start_beat,
        start_frame,
        end_frame,
        length_beats,
        style,
        bpm_delta: into.bpm - out.bpm,
        key_relation,
        reasons,
    })
}

/// How the two keys stand to each other, when both are known.
///
/// `None` rather than a relation meaning "unknown", because unknown is not a
/// clash: treating it as one would make every unanalysed track a cut, which is
/// a guess dressed as a decision.
fn relation(out: &Outgoing, into: &Incoming) -> Option<KeyRelation> {
    Some(out.key?.relation_to(into.key?))
}

/// Which way to do it.
///
/// Blend is the default because it is what a DJ does by hand. The two
/// departures from it are both about not holding a problem open for eight bars:
/// mismatched tempos and clashing keys are each tolerable for a moment and
/// tiring for a phrase.
fn choose_style(
    tempos_match: bool,
    keys_match: bool,
    usual: Option<TransitionStyle>,
) -> TransitionStyle {
    match (tempos_match, keys_match) {
        // §81: the one case where the music has left the choice open, and
        // therefore the only case where how this DJ actually joins records at
        // this kind of night gets to decide it. Everything that overlaps two
        // records works here; which of them to use is taste, and djmanzo has
        // watched three nights of theirs.
        (true, true) => usual
            .filter(|style| STYLE_IS_TASTE.contains(style))
            .unwrap_or(TransitionStyle::Blend),
        // Tempos work, keys fight: get through it quickly, and let the outgoing
        // track dissolve rather than sit against the new one.
        (true, false) => TransitionStyle::Echo,
        // Tempos do not work. Nothing overlapping will help, whatever the keys
        // do -- this is the case `Cut` exists for, and it is the honest answer.
        (false, _) => TransitionStyle::Cut,
    }
}

/// The styles a **profile** may choose between.
///
/// §81 learns how a DJ joins records at a kind of night, and this is the list
/// its answer is allowed to be. It is a statement about where taste ends.
///
/// [`TransitionStyle::Cut`] is in it: a DJ who cuts between records at a
/// wedding is doing something deliberate, and a planner that kept proposing a
/// long blend to somebody who never uses one would be arguing with three
/// nights of evidence. [`TransitionStyle::Fade`] and [`TransitionStyle::Blend`]
/// likewise — every one of the three works on any pair whose tempos and keys
/// agree, which is the case this list is consulted in.
///
/// [`TransitionStyle::VocalDrop`] is **not**, and that is the point of having a
/// list rather than a `!=`: a vocal drop needs a vocal to keep and stems to
/// keep it out of, and the planner sees two records' tempo, key and phrase. A
/// profile could learn it — a DJ who does it every night would have it as their
/// commonest style — and proposing it for a pair with no separation available
/// would be djmanzo promising a mix it cannot perform.
///
/// [`TransitionStyle::Echo`] is not in it either, for the opposite reason: it
/// is the *music's* answer to a key clash below, not a preference, and a DJ
/// whose commonest style is Echo has mostly been mixing records whose keys
/// fight rather than expressing a taste for echoes.
const STYLE_IS_TASTE: [TransitionStyle; 3] = [
    TransitionStyle::Cut,
    TransitionStyle::Fade,
    TransitionStyle::Blend,
];

/// Frames per beat, or `None` if the tempo is not a tempo.
pub(crate) fn beat_frames(bpm: f64, rate: SampleRate) -> Option<f64> {
    let frames = rate.as_f64() * 60.0 / bpm;
    (frames.is_finite() && frames > 0.0).then_some(frames)
}

/// Whether `b` is within `tolerance` of `a`, or of half or double it.
fn ratio_within(a: f64, b: f64, tolerance: f64) -> bool {
    [1.0, 0.5, 2.0]
        .into_iter()
        .any(|factor| (b / (a * factor) - 1.0).abs() <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Mode;

    const SR: SampleRate = SampleRate::DEFAULT;
    const BPM: f64 = 120.0;

    /// Frames per beat, **derived** rather than written down.
    ///
    /// It was a literal 22 050 first -- 120 BPM at 44.1 kHz -- and
    /// `SampleRate::DEFAULT` is 48 kHz, so every fixture position was a beat
    /// index the planner did not agree with. The failure looked like an
    /// off-by-thirty bug in the planner and was a wrong constant in the test.
    fn beat() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    /// An outgoing track `beats_in` beats along, `total_beats` long.
    fn outgoing(beats_in: f64, total_beats: f64) -> Outgoing {
        Outgoing {
            position: beats_in * beat(),
            length: total_beats * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            key: Some(key(8, Mode::Minor)),
            sample_rate: SR,
            grid_anchor: 0.0,
        }
    }

    fn incoming(bpm: f64, k: Option<MusicalKey>) -> Incoming {
        Incoming {
            bpm,
            phrase: Phrase::new(16, 0),
            key: k,
        }
    }

    /// **The mix starts on a phrase boundary.**
    ///
    /// The whole reason the planner exists. A transition that begins four beats
    /// into a phrase brings the new record in against the middle of a musical
    /// idea, and it sounds like a mistake even when every other number is
    /// right.
    #[test]
    fn the_transition_starts_on_a_phrase_boundary() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");

        assert_eq!(
            plan.start_beat % 16,
            0,
            "the mix was planned to start on beat {}, which is {} beats into a phrase",
            plan.start_beat,
            plan.start_beat % 16
        );
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::LandsOnPhrase { .. })),
            "it landed on a phrase and did not say so: {:?}",
            plan.reasons
        );
    }

    /// **An offset phrase start moves the plan with it.**
    ///
    /// A track opening with a five-beat pickup has its phrases at 5, 21, 37. A
    /// planner that assumed phrases begin at the grid anchor would start every
    /// mix on such a record eleven beats early -- on a beat, so it would look
    /// right on the waveform, and wrong in the room.
    #[test]
    fn an_offset_phrase_start_moves_the_plan() {
        let out = Outgoing {
            phrase: Phrase::new(16, 5),
            ..outgoing(100.0, 400.0)
        };
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_eq!(
            plan.start_beat.rem_euclid(16),
            5,
            "started at beat {}, which is not a phrase start for this track",
            plan.start_beat
        );
    }

    /// **Mismatched tempos get a cut, not a blend.**
    ///
    /// Nothing overlapping helps when the two records are at audibly different
    /// speeds. `Cut` is the honest answer and the style exists for exactly
    /// this.
    #[test]
    fn tempos_too_far_apart_are_cut() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(145.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Cut);
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::TemposClash { .. })),
            "it cut without saying why: {:?}",
            plan.reasons
        );
    }

    /// Half and double time are a match, not a clash: 120 into 60 is an
    /// ordinary move and the plain ratio cannot see it.
    #[test]
    fn half_time_is_not_a_tempo_clash() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(60.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert_ne!(
            plan.style,
            TransitionStyle::Cut,
            "120 into 60 was treated as unmixable"
        );
    }

    /// **Clashing keys get a shorter, dissolving transition.**
    ///
    /// Two records whose keys fight are tolerable for a moment and tiring for
    /// eight bars. Echo lets the outgoing one dissolve instead of sitting
    /// against the new one.
    #[test]
    fn clashing_keys_dissolve_rather_than_blend() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, Some(key(2, Mode::Major)))).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Echo);
        assert!(plan.reasons.contains(&Reason::KeysClash));
    }

    /// **An unknown key is not a clash.**
    ///
    /// Treating it as one would make every unanalysed track a cut, which is a
    /// guess dressed up as a decision.
    #[test]
    fn an_unknown_key_still_blends() {
        let out = outgoing(100.0, 400.0);
        let plan = plan(&out, &incoming(120.0, None)).expect("a plan");
        assert_eq!(plan.style, TransitionStyle::Blend);
        assert!(
            !plan.reasons.contains(&Reason::KeysClash),
            "an unknown key was reported as a clash"
        );
    }

    /// **The mix finishes before the track does.**
    ///
    /// With a margin, because a human presses the button a beat or two late and
    /// a transition that was planned to end exactly at the last sample ends
    /// with silence instead.
    #[test]
    fn the_transition_ends_before_the_track_runs_out() {
        let total = 400.0;
        let out = outgoing(360.0, total);
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");

        let end_beat = plan.start_beat + i64::from(plan.length_beats);
        assert!(
            (end_beat as f64) <= total,
            "the mix was planned to end at beat {end_beat} of a {total}-beat track"
        );
    }

    /// **Near the end, the transition is shortened rather than refused.**
    ///
    /// A DJ forty beats from the end of a record still has to get out of it.
    /// Answering "no plan" there would be the planner giving up at the one
    /// moment it is being read.
    #[test]
    fn a_short_tail_gets_a_shorter_transition() {
        let roomy = plan(
            &outgoing(100.0, 400.0),
            &incoming(120.0, Some(key(8, Mode::Minor))),
        )
        .expect("a plan");
        let tight = plan(
            &outgoing(370.0, 400.0),
            &incoming(120.0, Some(key(8, Mode::Minor))),
        )
        .expect("a plan near the end");

        assert!(
            tight.length_beats < roomy.length_beats,
            "a 30-beat tail got the same {} beat transition as a 300-beat one",
            roomy.length_beats
        );
    }

    /// **The start is never behind the playhead.**
    ///
    /// A plan to begin the mix in the past is not a plan. This is the case that
    /// arises when the track is nearly over: the last usable boundary has
    /// already gone by.
    #[test]
    fn the_plan_never_starts_in_the_past() {
        for beats_in in [300.0, 350.0, 380.0, 390.0] {
            let out = outgoing(beats_in, 400.0);
            let Some(plan) = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))) else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation)]
            let current = (beats_in) as i64;
            assert!(
                plan.start_beat >= current,
                "at beat {current} it planned to start at {}",
                plan.start_beat
            );
        }
    }

    /// Past the tail margin there is nothing to propose, and it says so rather
    /// than inventing a transition that cannot happen.
    /// **The load-bearing one: §81's learned style decides where the music has
    /// left the choice open, and nowhere else.**
    ///
    /// `Profile::style` — how this DJ actually joins records at the kind of
    /// night they named — was shown in a sentence and read by nothing: djmanzo
    /// could say *at weddings you mostly fade* and then propose a blend, every
    /// time, forever.
    ///
    /// Both halves are asserted because only having the first is the shape this
    /// fails in. A preference that reached the mismatched-tempo case would be
    /// three previous evenings outvoting the two records in front of the DJ.
    #[test]
    fn a_learned_style_decides_the_open_case_and_never_the_musics_own() {
        let out = outgoing(100.0, 400.0);
        let agreeing = incoming(120.0, Some(key(8, Mode::Minor)));

        // Nothing learned: the answer djmanzo has always given.
        assert_eq!(
            plan(&out, &agreeing).expect("a plan").style,
            TransitionStyle::Blend
        );
        // Learned, and it lands.
        for wanted in [
            TransitionStyle::Fade,
            TransitionStyle::Cut,
            TransitionStyle::Blend,
        ] {
            assert_eq!(
                plan_as(&out, &agreeing, Some(wanted))
                    .expect("a plan")
                    .style,
                wanted,
                "a DJ who mostly uses {wanted} was offered something else on a \
                 pair the music had no opinion about"
            );
        }

        // The keys fight: the echo is the music's answer and a preference does
        // not get a vote.
        let clashing = incoming(120.0, Some(key(2, Mode::Major)));
        assert_eq!(
            plan_as(&out, &clashing, Some(TransitionStyle::Blend))
                .expect("a plan")
                .style,
            TransitionStyle::Echo,
            "a learned blend was proposed over a key clash"
        );

        // The tempos do not work: a cut, whatever anybody usually does.
        let far = incoming(145.0, Some(key(8, Mode::Minor)));
        assert_eq!(
            plan_as(&out, &far, Some(TransitionStyle::Blend))
                .expect("a plan")
                .style,
            TransitionStyle::Cut,
            "a learned blend was proposed across a tempo the deck cannot reach"
        );
    }

    /// **A vocal drop is not a preference djmanzo may act on, and the list says
    /// so rather than a `!=` hiding it.**
    ///
    /// A DJ who does one every night would have it as their commonest style,
    /// and the planner sees two records' tempo, key and phrase — not whether
    /// there is a vocal to keep or stems to keep it out of. Proposing it would
    /// be djmanzo promising a mix it cannot perform.
    ///
    /// Echo is excluded for the opposite reason and is checked here too: it is
    /// the music's own answer to a key clash below, so a DJ whose commonest
    /// style is Echo has mostly been mixing records whose keys fight rather
    /// than expressing a taste for echoes.
    #[test]
    fn the_two_styles_that_are_not_taste_are_refused_as_preferences() {
        let out = outgoing(100.0, 400.0);
        let agreeing = incoming(120.0, Some(key(8, Mode::Minor)));

        for refused in [TransitionStyle::VocalDrop, TransitionStyle::Echo] {
            assert_eq!(
                plan_as(&out, &agreeing, Some(refused))
                    .expect("a plan")
                    .style,
                TransitionStyle::Blend,
                "{refused} was taken as a preference, and it is not one"
            );
            assert!(
                !STYLE_IS_TASTE.contains(&refused),
                "the table and the behaviour disagree about {refused}"
            );
        }
        // And the three that are taste are all there, so the list cannot
        // quietly shrink to one.
        assert_eq!(STYLE_IS_TASTE.len(), 3);
    }

    /// An outgoing-shaped record, as a `Record` rather than an `Outgoing`.
    fn record(total_beats: f64, anchor_beats: f64) -> Record {
        Record {
            length: total_beats * beat(),
            bpm: BPM,
            phrase: Phrase::new(16, 0),
            sample_rate: SR,
            grid_anchor: anchor_beats * beat(),
        }
    }

    /// **The load-bearing one: a mix into a record ends before its drop.**
    ///
    /// §25's `mix-in` layer, and the whole reason it is worth drawing. A mix
    /// started after the drop has thrown the drop away, and the drop is usually
    /// the thing the transition was building towards — so the window closes
    /// there, snapped back to a phrase, because landing a record against the
    /// middle of a musical idea is the mistake this module exists to avoid at
    /// both ends.
    #[test]
    fn the_window_to_come_in_closes_at_the_records_own_drop() {
        let rec = record(400.0, 0.0);
        // A drop 70 beats in: not on a phrase boundary, so the close must snap
        // back to beat 64 rather than sitting four beats into the fifth phrase.
        let drop = 70.0 * beat();
        let window = mix_in(&rec, Some(drop)).expect("a record with a grid has a window");

        assert!(window.before_a_drop, "the close came from arithmetic");
        assert!(window.on_phrase);
        assert!(
            (window.opens_frame - 0.0).abs() < 1.0,
            "a grid anchored on a phrase opens at its own beginning"
        );
        assert!(
            (window.closes_frame - 64.0 * beat()).abs() < 1.0,
            "the close did not snap back to a phrase: {} beats",
            window.closes_frame / beat()
        );
        assert!(
            window.closes_frame < drop,
            "the window reaches past the drop"
        );
    }

    /// **A record nobody has found a drop in gets arithmetic, and says so.**
    ///
    /// The same honesty `MixOut::on_phrase` carries. A window closed by the
    /// longest transition the planner proposes is a real answer — *its first
    /// eight bars* — and dressing it as the record's structure would be the
    /// confident lie this module is written not to tell.
    #[test]
    fn a_record_with_no_drop_gets_the_planners_own_longest_mix() {
        let rec = record(400.0, 0.0);
        let window = mix_in(&rec, None).expect("a window");

        assert!(
            !window.before_a_drop,
            "arithmetic was reported as structure"
        );
        assert!(
            (window.closes_frame - f64::from(LENGTHS[0]) * beat()).abs() < 1.0,
            "the close is not the longest transition: {} beats",
            window.closes_frame / beat()
        );
    }

    /// **The window opens on the record's first phrase, never before its
    /// grid.**
    ///
    /// Snapped *forward* rather than back, which is the opposite of
    /// [`mix_out`] and is right: there is audio after the anchor and none
    /// before it, so a window that snapped back would open on nothing.
    #[test]
    fn the_window_opens_forward_onto_the_first_whole_phrase() {
        // A grid whose phrases start on beat 4 — so the boundaries are 4, 20,
        // 36, and the first whole phrase after the anchor begins four beats in.
        let mut rec = record(400.0, 0.0);
        rec.phrase = Phrase::new(16, 4);
        let window = mix_in(&rec, None).expect("a window");

        assert!(window.on_phrase);
        assert!(
            window.opens_frame > 0.0,
            "the window opened before the record's first whole phrase"
        );
        assert!(
            (window.opens_frame - 4.0 * beat()).abs() < 1.0,
            "the window did not open on the next phrase: {} beats",
            window.opens_frame / beat()
        );
        // And it is a real boundary rather than a number that happens to fit:
        // the phrase detector agrees there is one there.
        assert_eq!(
            rec.phrase.expect("a phrase").beat_within(4),
            0,
            "beat 4 is not a phrase boundary, so this test proves nothing"
        );
    }

    /// **A record whose drop is in its first phrase has no window at all.**
    ///
    /// A band drawn backwards is worse than no band, and "you may bring this in
    /// nowhere" is a true and useful thing to say about a record that is all
    /// chorus from the first bar.
    #[test]
    fn a_record_that_drops_immediately_has_nowhere_to_be_brought_in() {
        let rec = record(400.0, 0.0);
        assert_eq!(mix_in(&rec, Some(4.0 * beat())), None);
        // And a record with no usable grid has none either.
        let mut nonsense = record(400.0, 0.0);
        nonsense.bpm = 0.0;
        assert_eq!(mix_in(&nonsense, None), None);
    }

    #[test]
    fn a_track_at_its_end_gets_no_plan() {
        assert!(
            plan(
                &outgoing(397.0, 400.0),
                &incoming(120.0, Some(key(8, Mode::Minor)))
            )
            .is_none(),
            "a track three beats from the end was given a transition plan"
        );
    }

    /// A tempo that is not a tempo produces no plan rather than an infinity.
    #[test]
    fn a_nonsense_tempo_gets_no_plan() {
        for bad in [0.0, -120.0, f64::NAN] {
            let out = Outgoing {
                bpm: bad,
                ..outgoing(100.0, 400.0)
            };
            assert!(
                plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).is_none(),
                "{bad} BPM produced a plan"
            );
        }
    }

    /// **The rushed warning counts what it says it counts.**
    ///
    /// It fires when the mix would finish with less than the tail margin
    /// behind it, so the number it carries is the room *after the mix ends* —
    /// not the room from the playhead, which is larger by the whole
    /// transition. The interface prints it beside another reason ending in
    /// "beats left", and two numbers a comma apart that look alike and mean
    /// different things is how a DJ stops reading either. Found by driving the
    /// application: the line read "only 277 beats left · 32 beats left".
    #[test]
    fn a_rushed_transition_reports_the_room_it_is_short_of() {
        let out = outgoing(100.0, 400.0);
        let into = incoming(120.0, Some(key(8, Mode::Minor)));

        // Eight beats starting at 385 of a 400-beat record ends at 393, with
        // seven beats behind it -- one short of the margin.
        let plan = evaluate(&out, &into, 385, 8, TransitionStyle::Blend).expect("a plan");
        let rushed = plan
            .reasons
            .iter()
            .find_map(|r| match r {
                Reason::Rushed { beats_after } => Some(*beats_after),
                _ => None,
            })
            .expect("a mix ending seven beats from the end did not say it was rushed");
        assert!(
            (rushed - 7.0).abs() < 0.01,
            "it reported {rushed} beats, which is not the room after the mix ends"
        );

        // And a roomy one does not claim to be rushed at all.
        let roomy = evaluate(&out, &into, 200, 32, TransitionStyle::Blend).expect("a plan");
        assert!(
            !roomy
                .reasons
                .iter()
                .any(|r| matches!(r, Reason::Rushed { .. })),
            "a mix ending 168 beats before the record does was called rushed"
        );
    }

    /// Without a phrase structure it still plans, and says the start is only a
    /// bar line rather than presenting it as the same thing.
    #[test]
    fn no_phrase_structure_is_reported_not_hidden() {
        let out = Outgoing {
            phrase: None,
            ..outgoing(100.0, 400.0)
        };
        let plan = plan(&out, &incoming(120.0, Some(key(8, Mode::Minor)))).expect("a plan");
        assert!(
            plan.reasons
                .iter()
                .any(|r| matches!(r, Reason::LandsOnBar { .. })),
            "a bar-line start was not distinguished from a phrase start: {:?}",
            plan.reasons
        );
    }

    /// **The mix-out window is about the record, not about the playhead.**
    ///
    /// The thing §25's layer promises. A band that crept forward under a DJ
    /// watching it would be drawing the planner's current *answer*, which
    /// already has a marker of its own — and would say that where a record can
    /// be left depends on how far through it you are, which is not true of any
    /// record.
    #[test]
    fn where_a_record_can_be_left_does_not_move_with_the_playhead() {
        let early = mix_out(&outgoing(4.0, 400.0).record()).expect("a window");
        let late = mix_out(&outgoing(300.0, 400.0).record()).expect("the same window");
        assert_eq!(early, late);
    }

    /// **Both edges are the planner's own arithmetic.**
    ///
    /// Derived here from `LENGTHS` and `TAIL_MARGIN` rather than written down,
    /// so a change to either moves the test and the code together. Two answers
    /// to "how late is too late" is the failure this shares constants to
    /// avoid: the band would say one thing and `Reason::Rushed` another, on
    /// the same screen, about the same mix.
    #[test]
    fn the_window_closes_where_the_shortest_mix_stops_fitting() {
        let total = 400.0;
        let window = mix_out(&outgoing(100.0, total).record()).expect("a window");

        let shortest = f64::from(*LENGTHS.last().unwrap());
        let expected_close = total - TAIL_MARGIN - shortest;
        assert!(
            (window.closes_frame / beat() - expected_close).abs() < 0.01,
            "the window closes at beat {}, not at {expected_close}",
            window.closes_frame / beat()
        );

        // A mix started on the closing beat is not rushed; one a beat later is.
        let out = outgoing(100.0, total);
        let into = incoming(120.0, Some(key(8, Mode::Minor)));
        #[allow(clippy::cast_possible_truncation)]
        let closing = expected_close as i64;
        for (beat_index, rushed) in [(closing, false), (closing + 1, true)] {
            let plan =
                evaluate(&out, &into, beat_index, 8, TransitionStyle::Blend).expect("a plan");
            assert_eq!(
                plan.reasons
                    .iter()
                    .any(|r| matches!(r, Reason::Rushed { .. })),
                rushed,
                "beat {beat_index} disagrees with the window about being rushed"
            );
        }
    }

    /// **The window opens on a phrase where there is one, and says so.**
    ///
    /// A window that opened four beats into a phrase would open somewhere no
    /// DJ starts a mix, and would then be a band whose left edge means
    /// nothing.
    #[test]
    fn the_window_opens_on_a_phrase_boundary_and_admits_when_it_does_not() {
        let window = mix_out(&outgoing(100.0, 400.0).record()).expect("a window");
        assert!(window.on_phrase, "a 16-beat phrase structure was ignored");
        assert_eq!(
            window.opens_beat % 16,
            0,
            "it opened {} beats into a phrase",
            window.opens_beat % 16
        );

        let bare = mix_out(
            &Outgoing {
                phrase: None,
                ..outgoing(100.0, 400.0)
            }
            .record(),
        )
        .expect("a window");
        assert!(
            !bare.on_phrase,
            "a record with no phrase structure claimed its window opens on one"
        );
        // And it opens later than the snapped one, because nothing pulled it
        // back to a boundary -- which is what makes the claim worth carrying.
        assert!(bare.opens_beat >= window.opens_beat);
    }

    /// **A record with nowhere left to be mixed out of has no window.**
    ///
    /// Not a window at beat zero, and not a hairline at the end: a loop or a
    /// jingle is not a record you leave, and drawing a band across all of one
    /// would tell a DJ to mix out of a sample.
    #[test]
    fn a_record_too_short_to_leave_has_no_window() {
        assert_eq!(mix_out(&outgoing(0.0, 12.0).record()), None);
        assert_eq!(
            mix_out(
                &Outgoing {
                    bpm: 0.0,
                    ..outgoing(100.0, 400.0)
                }
                .record()
            ),
            None,
            "a record with no tempo has no beats to count a window in"
        );
    }
}

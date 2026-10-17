//! §118a: the assistant's guides, one per topic.
//!
//! > the integrated ai assistant shall have different guides that are
//! > visually represented via floating widgets with visual presentations of
//! > topics and representations of (multi-choice) options that are easy to
//! > identify
//!
//! A guide is a topic the DJ can ask about at any moment, drawn as a
//! floating widget of a few large choices. What a guide *offers* is the
//! work of what already answers that question — the rail for the next
//! record ([`crate::decide`]), the planner for the mix
//! ([`crate::transition`]), the prepared night for trouble
//! ([`crate::gig::tonight`]). What this module decides is **which guides
//! can be opened now, and why not the others**: a guide that opened onto
//! nothing, or onto an error, would teach a DJ not to open it again.
//!
//! The decks it names are the ones the guide works on — the deck the room
//! is hearing most, and the one the next record goes on — so the interface
//! never has to work out which deck "the playing one" is. Only the decks on
//! screen count: the engine always has four, and a record loaded on one the
//! layout does not show would be a record loaded nowhere a DJ can see.

use crate::snapshot::{DeckSnapshot, Snapshot};
use serde::Serialize;

/// What a guide is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    /// Three records to follow what is playing, one each way.
    Next,
    /// How to bring the record waiting on the other deck in.
    Mix,
    /// Where the night stands, and the DJ's word on where it goes.
    Energy,
    /// What the room asked for and is still waiting: find it, or not tonight.
    Request,
    /// The moves the DJ is learning, and one more to try.
    Learn,
    /// The plans written for tonight's troubles.
    Trouble,
}

impl Topic {
    /// In the order the guides are offered.
    pub const ALL: [Topic; 6] = [
        Topic::Next,
        Topic::Mix,
        Topic::Energy,
        Topic::Request,
        Topic::Learn,
        Topic::Trouble,
    ];

    /// How a run names it (`guide energy`), as the interface does.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Topic::Next => "next",
            Topic::Mix => "mix",
            Topic::Energy => "energy",
            Topic::Request => "request",
            Topic::Learn => "learn",
            Topic::Trouble => "trouble",
        }
    }

    /// The topic a slug names.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|topic| topic.slug() == slug)
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Topic::Next => "Next record",
            Topic::Mix => "The mix",
            Topic::Energy => "The energy of the night",
            Topic::Request => "A request waiting",
            Topic::Learn => "Learning a move",
            Topic::Trouble => "If something goes wrong",
        }
    }

    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Topic::Next => "Three records to follow what is playing: up, level, or down.",
            Topic::Mix => "How to bring the waiting record in, each way drawn as it sounds.",
            Topic::Energy => "Where the night stands, and your word on where it goes.",
            Topic::Request => {
                "What the room asked for, most wanted first: find it, or not tonight."
            }
            Topic::Learn => {
                "The moves you are learning, each as it is taught, and one more to try."
            }
            Topic::Trouble => "The plans you wrote for tonight, one press each.",
        }
    }
}

/// One guide, as the launcher offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Guide {
    pub topic: Topic,
    pub title: &'static str,
    pub about: &'static str,
    /// Why it cannot be opened now, or `None` when it can.
    pub not_now: Option<String>,
    /// The deck the room is hearing most, which the guide works from.
    pub from: Option<u8>,
    /// The deck it works towards: an empty one for the next record, the
    /// loaded and waiting one for the mix.
    pub to: Option<u8>,
}

/// The playing deck the room hears most, among the first `shown`.
fn leading(snapshot: &Snapshot, shown: u8) -> Option<&DeckSnapshot> {
    let crossfader = snapshot.master.crossfader;
    snapshot
        .decks
        .iter()
        .filter(|d| d.number <= shown && d.playing)
        .max_by(|a, b| {
            crate::whisper::heard(a, crossfader).total_cmp(&crate::whisper::heard(b, crossfader))
        })
}

/// Every guide, each open or saying why not, over the first `shown` decks;
/// `waiting` is how many of the room's requests are still waiting.
#[must_use]
pub fn guides(snapshot: &Snapshot, live: bool, shown: u8, waiting: usize) -> Vec<Guide> {
    let lead = leading(snapshot, shown);
    let from = lead.map(|d| d.number);
    let other = |wanted: fn(&DeckSnapshot) -> bool| {
        snapshot
            .decks
            .iter()
            .find(|d| d.number <= shown && Some(d.number) != from && wanted(d))
            .map(|d| d.number)
    };
    Topic::ALL
        .into_iter()
        .map(|topic| {
            let (not_now, to) = match topic {
                Topic::Next => match (from, other(|d| !d.loaded)) {
                    (None, _) => (Some("Nothing is playing.".to_owned()), None),
                    (Some(_), None) => (
                        Some("Every other deck has a record on it already.".to_owned()),
                        None,
                    ),
                    (Some(_), to) => (None, to),
                },
                Topic::Mix => match (from, other(|d| d.loaded && !d.playing)) {
                    (None, _) => (Some("Nothing is playing.".to_owned()), None),
                    (Some(_), None) => (
                        Some("Load the next record on another deck first.".to_owned()),
                        None,
                    ),
                    (Some(_), to) => (None, to),
                },
                Topic::Energy if from.is_some() => (None, None),
                Topic::Energy => (Some("Nothing is playing.".to_owned()), None),
                Topic::Request if waiting > 0 => (None, None),
                Topic::Request => (Some("Nothing is waiting from the room.".to_owned()), None),
                // The coach's own rule and words: §58 puts technique advice in
                // the contextual tier, and mixing leaves no room for it.
                Topic::Learn => (
                    crate::commands::lesson_withheld(snapshot.attention.room_for, snapshot.quiet)
                        .map(str::to_owned),
                    None,
                ),
                Topic::Trouble if live => (None, None),
                Topic::Trouble => (
                    Some("Only while a prepared night is being played.".to_owned()),
                    None,
                ),
            };
            Guide {
                topic,
                title: topic.title(),
                about: topic.about(),
                from: if matches!(
                    topic,
                    Topic::Trouble | Topic::Request | Topic::Energy | Topic::Learn
                ) {
                    None
                } else {
                    from
                },
                not_now,
                to,
            }
        })
        .collect()
}

/// One move in the learning guide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Learning {
    pub name: &'static str,
    /// What it does, in one line.
    pub what: &'static str,
    /// When a DJ reaches for it.
    pub when: &'static str,
    /// The bridge from the world it is taught in.
    pub metaphor: &'static str,
    /// `first`, `practised` or `hard`.
    pub difficulty: &'static str,
    /// One the DJ said they are learning, rather than the coach's next.
    pub yours: bool,
    /// The style the practice lab renders this move in — the word
    /// `practice_rehearse` takes — when the automix performs it.
    pub style: Option<&'static str>,
    /// Why the lab cannot, when it cannot: the move is played on the decks.
    /// Exactly one of this and `style` is there.
    pub not_in_lab: Option<String>,
}

impl Learning {
    fn of(technique: &'static dj_assistant::technique::Technique, yours: bool) -> Self {
        Self {
            name: technique.name,
            what: technique.what,
            when: technique.when,
            metaphor: technique.metaphor,
            difficulty: match technique.difficulty {
                dj_assistant::technique::Difficulty::First => "first",
                dj_assistant::technique::Difficulty::Practised => "practised",
                dj_assistant::technique::Difficulty::Hard => "hard",
            },
            yours,
            style: technique
                .performed_as
                .map(dj_core::action::TransitionStyle::as_str),
            not_in_lab: technique
                .performed_as
                .is_none()
                .then(|| not_in_lab(technique.name)),
        }
    }
}

/// Why *Rehearse it* is not offered for a move the automix does not
/// perform: the lab renders the held mix in one of djmanzo's styles, and a
/// move that is not one of them has nothing there to render.
fn not_in_lab(name: &str) -> String {
    let mut name = name.to_owned();
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    format!(
        "{name} is not one of djmanzo's mixes, so the practice lab cannot render it. \
         Try it on a deck the room cannot hear, in your headphones."
    )
}

/// §118: the learning guide's moves — the ones the DJ is learning, in their
/// order, then the coach's next lesson: the easiest move they neither play
/// nor are learning, inside the chosen pack, that the rig allows.
#[must_use]
pub fn learning(
    answers: &crate::welcome::Answers,
    rig: dj_assistant::technique::Rig,
    pack: Option<&dj_assistant::pack::Pack>,
) -> Vec<Learning> {
    let mut moves: Vec<Learning> = answers
        .learn
        .iter()
        .filter_map(|name| dj_assistant::technique::by_name(name))
        .map(|technique| Learning::of(technique, true))
        .collect();
    let known: Vec<&str> = answers
        .moves
        .iter()
        .chain(&answers.learn)
        .map(String::as_str)
        .collect();
    if let Some(next) = dj_assistant::coach::next_lesson(&known, rig, pack) {
        moves.push(Learning::of(next, false));
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn booth() -> Snapshot {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 2);
        for deck in &mut snapshot.decks {
            deck.loaded = false;
            deck.playing = false;
            deck.volume = 1.0;
        }
        snapshot.master.crossfader = 0.0;
        snapshot
    }

    fn open(guides: &[Guide], topic: Topic) -> &Guide {
        guides
            .iter()
            .find(|g| g.topic == topic)
            .expect("every topic is offered")
    }

    /// **A guide opens onto something, or says why not.** Nothing playing
    /// opens neither the next record nor the mix; one deck playing and the
    /// other empty opens the next record towards the empty one; a record
    /// waiting on the other deck opens the mix towards it instead.
    #[test]
    fn a_guide_opens_onto_something_or_says_why_not() {
        let mut snapshot = booth();
        let quiet = guides(&snapshot, false, 2, 0);
        assert_eq!(quiet.len(), Topic::ALL.len());
        // Every guide but learning a move, which a quiet booth is the time for.
        assert!(
            quiet
                .iter()
                .all(|g| g.not_now.is_some() == (g.topic != Topic::Learn)),
            "{quiet:#?}"
        );

        snapshot.decks[0].loaded = true;
        snapshot.decks[0].playing = true;
        let one = guides(&snapshot, false, 2, 0);
        let next = open(&one, Topic::Next);
        assert_eq!(
            (next.not_now.as_deref(), next.from, next.to),
            (None, Some(1), Some(2))
        );
        assert_eq!(
            open(&one, Topic::Mix).not_now.as_deref(),
            Some("Load the next record on another deck first.")
        );

        snapshot.decks[1].loaded = true;
        let waiting = guides(&snapshot, false, 2, 0);
        let mix = open(&waiting, Topic::Mix);
        assert_eq!(
            (mix.not_now.as_deref(), mix.from, mix.to),
            (None, Some(1), Some(2))
        );
        assert!(
            open(&waiting, Topic::Next).not_now.is_some(),
            "no deck to load on"
        );
    }

    /// §118: **the request guide opens only while the room is waiting** —
    /// on a request, whatever the decks are doing, and on no deck; with
    /// nothing waiting it says so rather than opening onto an empty list.
    #[test]
    fn the_request_guide_opens_only_while_the_room_is_waiting() {
        let snapshot = booth();
        let none = guides(&snapshot, false, 2, 0);
        assert_eq!(
            open(&none, Topic::Request).not_now.as_deref(),
            Some("Nothing is waiting from the room.")
        );
        let one = guides(&snapshot, false, 2, 1);
        let request = open(&one, Topic::Request);
        assert_eq!(
            (request.not_now.as_deref(), request.from, request.to),
            (None, None, None)
        );
        assert_eq!(request.title, "A request waiting");
        assert!(
            open(&one, Topic::Next).not_now.is_some(),
            "a request opened another guide"
        );
        let mut playing = booth();
        playing.decks[0].loaded = true;
        playing.decks[0].playing = true;
        let on_a_deck = guides(&playing, false, 2, 1);
        assert_eq!(
            open(&on_a_deck, Topic::Request).from,
            None,
            "a request is about the room, not a deck"
        );
    }

    /// §118: **the energy guide opens once the night is under way** — a
    /// record playing — and works on no deck, because the night is the
    /// room's, not one record's.
    #[test]
    fn the_energy_guide_opens_once_a_record_plays() {
        let mut snapshot = booth();
        assert_eq!(
            open(&guides(&snapshot, false, 2, 0), Topic::Energy)
                .not_now
                .as_deref(),
            Some("Nothing is playing.")
        );
        snapshot.decks[0].loaded = true;
        snapshot.decks[0].playing = true;
        let playing = guides(&snapshot, false, 2, 0);
        let energy = open(&playing, Topic::Energy);
        assert_eq!(
            (energy.not_now.as_deref(), energy.from, energy.to),
            (None, None, None)
        );
        assert_eq!(energy.title, "The energy of the night");
    }

    /// §118: **learning a move waits for the room the coach waits for** —
    /// closed while mixing in the coach's own words, open between records,
    /// on no deck.
    #[test]
    fn the_learning_guide_opens_between_records() {
        // Through the frame's own path — the judgement, then the focus —
        // rather than a room set by hand, which carries no reason.
        let mut snapshot = booth();
        for deck in &mut snapshot.decks {
            deck.loaded = true;
            deck.playing = true;
        }
        let mixing = guides(&snapshot.with_session(None), false, 2, 0);
        assert_eq!(
            open(&mixing, Topic::Learn).not_now.as_deref(),
            Some("Not while you are mixing. The lesson will be here between records.")
        );
        let between = guides(&booth().with_session(None), false, 2, 0);
        let learn = open(&between, Topic::Learn);
        assert_eq!(
            (learn.not_now.as_deref(), learn.from, learn.to),
            (None, None, None)
        );
    }

    /// §118: **a lesson held back by the activity says so, not that the DJ
    /// is mixing.** The drive that found it: the Mix activity, nothing
    /// playing, and the guide said *Not while you are mixing*. An activity
    /// set for playing withholds it, rightly, and says where it has room;
    /// mixing in that activity is still mixing; Practice opens it.
    #[test]
    fn a_lesson_held_back_by_the_activity_says_so() {
        use crate::cockpit::Focus;
        let said = |snapshot: Snapshot| {
            open(&guides(&snapshot, false, 2, 0), Topic::Learn)
                .not_now
                .clone()
        };
        assert_eq!(
            said(
                booth()
                    .with_session(None)
                    .with_focus(Some(Focus::Performing))
            )
            .as_deref(),
            Some("Not in an activity set for playing. The lesson has room in Practice.")
        );
        assert_eq!(
            said(
                booth()
                    .with_session(None)
                    .with_focus(Some(Focus::Supervising))
            )
            .as_deref(),
            Some("Not while you are watching the autopilot. The lesson has room in Practice.")
        );
        let mut mixing = booth();
        for deck in &mut mixing.decks {
            deck.loaded = true;
            deck.playing = true;
        }
        assert_eq!(
            said(
                mixing
                    .with_session(None)
                    .with_focus(Some(Focus::Performing))
            )
            .as_deref(),
            Some("Not while you are mixing. The lesson will be here between records.")
        );
        assert_eq!(
            said(booth().with_session(None).with_focus(Some(Focus::Learning))),
            None
        );
    }

    /// §118: **the moves being learned, then the coach's next** — theirs in
    /// their order, a name djmanzo does not know left out, and the one more
    /// to try neither one they play nor one they are learning.
    #[test]
    fn the_moves_being_learned_come_first_then_the_coachs_next() {
        let rig = dj_assistant::technique::Rig::laptop();
        let none = crate::welcome::Answers::default();
        let first = dj_assistant::coach::next_lesson(&[], rig, None).expect("a lesson");
        let alone = learning(&none, rig, None);
        assert_eq!(alone.len(), 1);
        assert_eq!((alone[0].name, alone[0].yours), (first.name, false));

        // Learning the very move the coach would offer next: it is theirs,
        // and the one more is another.
        let second = dj_assistant::coach::next_lesson(&[first.name], rig, None).expect("a second");
        let answers = crate::welcome::Answers {
            moves: vec![first.name.to_owned()],
            learn: vec![second.name.to_owned(), "moonwalk".to_owned()],
            ..crate::welcome::Answers::default()
        };
        let list = learning(&answers, rig, None);
        assert_eq!(list[0].name, second.name);
        assert!(list[0].yours);
        assert!(!list[0].metaphor.is_empty() && !list[0].what.is_empty());
        let next = list.last().expect("the coach's next");
        assert!(!next.yours);
        assert!(
            next.name != first.name && next.name != second.name,
            "suggested {} again",
            next.name
        );
        assert_eq!(list.len(), 2, "{list:#?}");
    }

    /// §118: **a move the automix performs is rehearsed in its style, and
    /// one it does not says why** — for every move in the table, exactly one
    /// of the two, and the style in the very word `practice_rehearse` parses
    /// back to that move's own style.
    #[test]
    fn a_move_is_rehearsed_in_its_style_or_says_why_not() {
        for technique in dj_assistant::technique::catalogue() {
            let move_ = Learning::of(technique, true);
            match (move_.style, &move_.not_in_lab) {
                (Some(word), None) => assert_eq!(
                    dj_core::action::TransitionStyle::parse(word),
                    technique.performed_as,
                    "{} is offered as {word}",
                    technique.name
                ),
                (None, Some(why)) => {
                    assert!(technique.performed_as.is_none(), "{}", technique.name);
                    assert!(
                        why.to_lowercase()
                            .starts_with(&technique.name.to_lowercase()),
                        "{why}"
                    );
                }
                both => panic!("{}: {both:?}", technique.name),
            }
        }
        let echo = dj_assistant::technique::by_name("echo out").expect("in the table");
        assert_eq!(Learning::of(echo, true).style, Some("echo"));
        let swap = dj_assistant::technique::by_name("bass swap").expect("in the table");
        assert_eq!(Learning::of(swap, true).style, Some("blend"));
        let ride = dj_assistant::technique::by_name("EQ ride").expect("in the table");
        assert_eq!(
            Learning::of(ride, false).not_in_lab.as_deref(),
            Some(
                "EQ ride is not one of djmanzo's mixes, so the practice lab cannot render it. \
                 Try it on a deck the room cannot hear, in your headphones."
            )
        );
    }

    /// The deck a guide works from is the one the room hears, not the
    /// first one playing: deck 1 playing behind a crossfader hard over to
    /// deck 2 is not what the room is listening to.
    #[test]
    fn a_guide_works_from_the_deck_the_room_hears() {
        let mut snapshot = booth();
        for deck in &mut snapshot.decks {
            deck.loaded = true;
            deck.playing = true;
        }
        snapshot.decks[0].crossfader_assign = dj_core::CrossfaderAssign::Left;
        snapshot.decks[1].crossfader_assign = dj_core::CrossfaderAssign::Right;
        snapshot.master.crossfader = 1.0;
        let both = guides(&snapshot, false, 2, 0);
        assert_eq!(open(&both, Topic::Next).from, Some(2));
    }

    /// **Only the decks on screen.** The engine always has four; with two
    /// shown and both holding a record, there is nowhere to load the next
    /// one -- not the third deck nobody can see.
    #[test]
    fn only_the_decks_on_screen_count() {
        let state = crate::state::AppState::new(true);
        let mut snapshot = crate::Snapshot::capture(&state.registry(), 4);
        for deck in &mut snapshot.decks {
            deck.loaded = false;
            deck.playing = false;
            deck.volume = 1.0;
        }
        snapshot.decks[0].loaded = true;
        snapshot.decks[0].playing = true;
        snapshot.decks[1].loaded = true;
        let two = guides(&snapshot, false, 2, 0);
        assert!(open(&two, Topic::Next).not_now.is_some(), "{two:#?}");
        let four = guides(&snapshot, false, 4, 0);
        assert_eq!(open(&four, Topic::Next).to, Some(3));
        // A deck off screen is not what the room hears either.
        snapshot.decks[0].playing = false;
        snapshot.decks[2].loaded = true;
        snapshot.decks[2].playing = true;
        assert!(
            open(&guides(&snapshot, false, 2, 0), Topic::Next)
                .from
                .is_none()
        );
    }

    /// Trouble is tonight's: open only while a prepared night is played.
    #[test]
    fn trouble_is_open_only_while_a_night_is_played() {
        let snapshot = booth();
        assert!(
            open(&guides(&snapshot, false, 2, 0), Topic::Trouble)
                .not_now
                .is_some()
        );
        assert_eq!(
            open(&guides(&snapshot, true, 2, 0), Topic::Trouble).not_now,
            None
        );
    }
}

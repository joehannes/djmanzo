//! §119: the crowd's reactions, on the music they were about.
//!
//! > i want to see comments ... integrated in the view: the song shall have
//! > comments and icons in places with the timestamp of messages where
//! > people comment heavily they like the moment of the music ... AI shall
//! > help discern if its that moment of the comment, a particular song or
//! > region of the song/chorus/drop ... also: the core data shall be
//! > saveable independently ... on replay and later analysis the user can
//! > have insights into the reactions of the live crowd and the mood ...
//! > even ways the user can define as of goals
//!
//! # A reaction, read
//!
//! A [`Reaction`] is what somebody said and when, from where: the room's own
//! request page, a stream's chat, a log brought in afterwards. [`read`] says
//! what it is **about** ([`About`]) — the moment, the record ("what song is
//! this?"), a request, a hello, or nothing djmanzo can place — and which
//! way it leans ([`Lean`]). Rules rather than a model, as the whisper's
//! are: they run on every message of a busy chat with no network, and each
//! is a sentence a test can hold.
//!
//! What the rules leave as [`About::Other`] — and only that — can be put to
//! the DJ's own AI provider, a batch at a time (`dj_assistant::crowd`): the
//! words and what was playing, never who said them. Its answer is kept as a
//! [`Reading`] beside the night (`crowd/read/<session>.jsonl`), and [`place`]
//! takes it where the rules had nothing; a reaction it could not place
//! either stays *something else*, and is not asked about again. [`unread`]
//! is what is still waiting, and [`due`] says when a night's next batch is:
//! every [`BATCH`] waiting or [`BATCH_WAIT`] seconds after the oldest,
//! whichever comes first. Nothing waits for it.
//!
//! # Placed on the music
//!
//! [`place`] puts each reaction on the record the room was hearing when it
//! was said — less the stream's delay, because a chat is always behind the
//! music it is about — and, for a reaction about the moment, on the part of
//! the record it most likely meant: a drop just before it, the breakdown it
//! is inside, the voice coming in. What the analysis does not know is not
//! guessed: such a reaction is placed on the record, not on a part.
//!
//! *The record the room was hearing* is measured, not assumed. The history
//! says when each record came in — when its deck started — and a DJ starts
//! the next one in the headphones a minute before anybody hears it, then
//! blends the two for half a minute more; placed by arrival, everything
//! said in those minutes went to the record the room had not heard yet.
//! [`Hearing`] follows each deck's meter after its fader and the crossfader,
//! which is what reached the room, and notes whenever a different record
//! became the louder one and stayed so ([`Heard`], kept beside the night as
//! `crowd/room/<session>.jsonl`). [`place`] asks that first, and a night
//! kept before it existed is placed by arrival, as it always was.
//!
//! # Kept on its own
//!
//! The reactions of a night are their own file, one JSON line each
//! (`crowd/<session>.jsonl`), appended as they come: readable without
//! djmanzo, and never lost with the rest of the session.
//!
//! # Read back
//!
//! [`summary`] is the night as a DJ asks about it afterwards: how many, how
//! fast at the busiest, which way it leaned, which records and which
//! moments drew the most. [`Goal`]s are the DJ's own questions of it — a
//! share of reactions that were warm, a pace, how often people asked what
//! was playing — and [`goals_for`] offers the usual ones per kind of night.

use crate::setting::Setting;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where a reaction came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// The room's own request page.
    Room,
    YouTube,
    TikTok,
    /// Brought in from a log, or anywhere else.
    Other,
}

/// What somebody said, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    /// Unix seconds, on the wall clock.
    pub at: i64,
    pub source: Source,
    /// The name they gave, if any.
    #[serde(default)]
    pub who: String,
    pub text: String,
}

/// What a reaction is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum About {
    /// The moment in the music: "this drop 🔥".
    Moment,
    /// The record itself: "what song is this?".
    Track,
    /// Something to play.
    Request,
    /// Hello, greetings from somewhere.
    Greeting,
    /// Nothing djmanzo can place.
    Other,
}

/// Which way a reaction leans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lean {
    Up,
    Down,
    Neither,
}

const REQUEST: [&str; 6] = [
    "play ",
    "can you play",
    "could you play",
    "please play",
    "request",
    "!sr ",
];
const TRACK: [&str; 11] = [
    "what song",
    "which song",
    "song name",
    "track id",
    "track name",
    "what track",
    "whats this song",
    "what's this song",
    "who is this",
    "id?",
    "shazam",
];
const MOMENT: [&str; 16] = [
    "this drop",
    "the drop",
    "drop!",
    "this part",
    "this bit",
    "right now",
    "banger",
    "tune!",
    "goosebumps",
    "chills",
    "omg",
    "yesss",
    "lets go",
    "let's go",
    "insane",
    "go off",
];
const MOMENT_MARKS: [char; 10] = ['🔥', '🙌', '🤯', '💥', '⚡', '🚀', '😱', '🕺', '💃', '🎉'];
const GREETING: [&str; 7] = [
    "hello",
    "hi ",
    "hey ",
    "greetings",
    "good evening",
    "good night",
    "from ",
];
const UP: [&str; 14] = [
    "love",
    "great",
    "amazing",
    "banger",
    "tune",
    "fire",
    "insane",
    "vibe",
    "nice",
    "sick",
    "best",
    "goosebumps",
    "chills",
    "yes",
];
const UP_MARKS: [char; 12] = [
    '🔥', '❤', '😍', '🙌', '🎉', '💯', '👏', '🥰', '💃', '🕺', '🤯', '👍',
];
const DOWN: [&str; 9] = [
    "boring",
    "skip",
    "meh",
    "too loud",
    "too quiet",
    "bad",
    "hate",
    "turn it",
    "change",
];
const DOWN_MARKS: [char; 4] = ['👎', '😴', '🥱', '💤'];

/// What a reaction is about, and which way it leans.
#[must_use]
pub fn read(text: &str) -> (About, Lean) {
    let lower = format!("{} ", text.to_lowercase());
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    let marks = |set: &[char]| text.chars().any(|c| set.contains(&c));
    let starts = |words: &[&str]| words.iter().any(|w| lower.starts_with(w));

    let about = if starts(&REQUEST) || has(&REQUEST[1..]) {
        About::Request
    } else if has(&TRACK) {
        About::Track
    } else if has(&MOMENT) || marks(&MOMENT_MARKS) {
        About::Moment
    } else if starts(&GREETING) || lower.starts_with("hi") && lower.len() <= 4 {
        About::Greeting
    } else {
        About::Other
    };
    let down = has(&DOWN) || marks(&DOWN_MARKS);
    let up = has(&UP) || marks(&UP_MARKS);
    let lean = match (up, down) {
        (_, true) => Lean::Down,
        (true, false) => Lean::Up,
        (false, false) => Lean::Neither,
    };
    (about, lean)
}

/// A record the room heard, with the parts of it the analysis knows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Played {
    /// When it came in, unix seconds.
    pub at: i64,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    /// Where it comes back after a breakdown, in seconds into the record.
    pub drops: Vec<f64>,
    /// Where it thins out, in seconds.
    pub breakdowns: Vec<(f64, f64)>,
    /// Where the voice arrives, in seconds.
    pub vocal: Option<f64>,
    /// How long the record is, in seconds, when the collection knows.
    pub length: Option<f64>,
}

/// The part of a record a reaction was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Part {
    Drop,
    Breakdown,
    Voice,
    /// The record, with no part djmanzo can name.
    Record,
}

/// Which record the room was hearing from a moment on: the louder one, by
/// the meters, once it had stayed the louder one for [`ROOM_HOLD`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heard {
    /// Unix seconds: when it became the louder one.
    pub at: i64,
    /// `None` when nothing was loud enough to be heard.
    pub track_id: Option<String>,
}

/// One deck's meter, as [`Hearing`] reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    pub deck: u8,
    pub track: Option<dj_core::TrackId>,
    pub playing: bool,
    /// Its peak after the channel fader and the crossfader, 0..=1: what
    /// reached the room, not what the deck was playing.
    pub level: f32,
}

/// How quickly [`Hearing`] follows a meter, in seconds: long enough that a
/// kick and the gap after it read as one level, short enough that a
/// crossfader cut is followed within a beat or two.
pub const ROOM_SMOOTHING: f64 = 1.0;

/// How long a record has to stay the louder one before the room is said to
/// be hearing it, in seconds. A scratch, a spinback or a cut there and back
/// again does not move the room.
pub const ROOM_HOLD: f64 = 2.0;

/// Below this a deck is not heard: about -40 dB below full scale.
pub const ROOM_SILENT: f32 = 0.01;

/// Follows which record the room is hearing, from the meters.
///
/// Fed every frame the interface is drawn; answers only when the room has
/// moved to a different record, so what is kept is a handful of lines a
/// record rather than sixty a second.
#[derive(Debug, Default)]
pub struct Hearing {
    /// Each deck's record and its smoothed level.
    levels: std::collections::HashMap<u8, (dj_core::TrackId, f64)>,
    /// The wall clock at the last reading, for the next step.
    last: Option<f64>,
    /// The record last said to be heard; `None` is silence, which is also
    /// where a night starts, so a quiet start is not written down.
    said: Option<dj_core::TrackId>,
    /// The record leading now but not yet for long enough, and since when.
    leading: Option<(Option<dj_core::TrackId>, f64)>,
}

impl Hearing {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The record the room is hearing now, as last said; `None` in silence.
    #[must_use]
    pub fn heard(&self) -> Option<dj_core::TrackId> {
        self.said
    }

    /// Read the decks at `now` (unix seconds), and say so when the room has
    /// moved to a different record.
    pub fn observe(&mut self, now: f64, decks: &[Meter]) -> Option<Heard> {
        // A clock that went backwards or stood still for a minute is not a
        // minute of listening.
        let step = self.last.map_or(0.0, |last| (now - last).clamp(0.0, 1.0));
        self.last = Some(now);
        let follow = 1.0 - (-step / ROOM_SMOOTHING).exp();

        self.levels
            .retain(|deck, _| decks.iter().any(|d| d.deck == *deck && d.track.is_some()));
        for deck in decks {
            let Some(track) = deck.track else { continue };
            let reached = if deck.playing {
                f64::from(deck.level.clamp(0.0, 1.0))
            } else {
                0.0
            };
            let entry = self.levels.entry(deck.deck).or_insert((track, 0.0));
            // A different record on the deck starts from silence: the last
            // one's level says nothing about it.
            if entry.0 != track {
                *entry = (track, 0.0);
            }
            entry.1 += (reached - entry.1) * follow;
        }

        let loudest = self
            .levels
            .values()
            .filter(|(_, level)| *level >= f64::from(ROOM_SILENT))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(track, _)| *track);

        if loudest == self.said {
            self.leading = None;
            return None;
        }
        match self.leading {
            Some((track, since)) if track == loudest => {
                if now - since < ROOM_HOLD {
                    return None;
                }
                self.said = loudest;
                self.leading = None;
                #[allow(clippy::cast_possible_truncation)]
                Some(Heard {
                    at: since.floor() as i64,
                    track_id: loudest.map(|track| track.to_hex()),
                })
            }
            _ => {
                self.leading = Some((loudest, now));
                None
            }
        }
    }
}

/// How far back a reaction about the moment looks for the part it meant:
/// long enough for a drop to land and be typed about, short enough that it
/// is still the drop people are reacting to.
pub const MOMENT_WINDOW: f64 = 20.0;

/// A reaction, read and placed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placed {
    pub reaction: Reaction,
    pub about: About,
    pub lean: Lean,
    /// Which of the records, by its place in the list; `None` before the
    /// first came in.
    pub record: Option<usize>,
    /// How far into that record the room was, in seconds.
    pub into: f64,
    pub part: Part,
    /// Where the part was, in seconds into the record, when there is one.
    pub part_at: Option<f64>,
    /// Read by the DJ's AI provider rather than the rules: the rules left it
    /// as something else, and the provider placed it.
    pub by_model: bool,
}

/// Put every reaction on the record the room was hearing, `delay` seconds
/// before it was said, and a reaction about the moment on the part of it
/// that was most likely meant.
///
/// Which record that was is `room`'s answer where it has one — the louder
/// of two playing together, and not the one waiting in the headphones —
/// and otherwise the record that came in last.
///
/// What a reaction is about is the rules' reading ([`read`]), and where the
/// rules found nothing, the provider's, from `readings`, when it found
/// something.
#[must_use]
pub fn place(
    reactions: &[Reaction],
    played: &[Played],
    room: &[Heard],
    delay: i64,
    readings: &[Reading],
) -> Vec<Placed> {
    reactions
        .iter()
        .map(|reaction| {
            let (mut about, mut lean) = read(&reaction.text);
            let mut by_model = false;
            if about == About::Other
                && let Some(reading) = readings
                    .iter()
                    .find(|r| r.of(reaction) && r.about != About::Other)
            {
                about = reading.about;
                lean = reading.lean;
                by_model = true;
            }
            let heard = reaction.at - delay;
            let record = hearing(played, room, heard);
            let (into, part, part_at) = match record {
                Some(i) => {
                    let p = &played[i];
                    #[allow(clippy::cast_precision_loss)]
                    let into = (heard - p.at) as f64;
                    let (part, at) = if about == About::Moment {
                        part_of(p, into)
                    } else {
                        (Part::Record, None)
                    };
                    (into, part, at)
                }
                None => (0.0, Part::Record, None),
            };
            Placed {
                reaction: reaction.clone(),
                about,
                lean,
                record,
                into,
                part,
                part_at,
                by_model,
            }
        })
        .collect()
}

/// The record the room was hearing at `at`: the one [`Hearing`] last measured
/// as the louder, when it measured one and that record is in the history;
/// otherwise the one that came in last.
fn hearing(played: &[Played], room: &[Heard], at: i64) -> Option<usize> {
    let arrived = played.iter().rposition(|p| p.at <= at);
    room.iter()
        .rfind(|heard| heard.at <= at)
        .and_then(|heard| heard.track_id.as_deref())
        .and_then(|track| {
            played
                .iter()
                .rposition(|p| p.at <= at && p.track_id == track)
        })
        .or(arrived)
}

/// The part a moment `into` a record most likely meant: a drop in the
/// window before it, then a breakdown it is inside, then the voice arriving
/// in the window, then nothing more specific than the record.
fn part_of(played: &Played, into: f64) -> (Part, Option<f64>) {
    let recent = |at: f64| at <= into && into - at <= MOMENT_WINDOW;
    if let Some(&drop) = played.drops.iter().rev().find(|&&d| recent(d)) {
        return (Part::Drop, Some(drop));
    }
    if let Some(&(from, _)) = played
        .breakdowns
        .iter()
        .find(|&&(from, to)| from <= into && into <= to)
    {
        return (Part::Breakdown, Some(from));
    }
    if let Some(voice) = played.vocal.filter(|&v| recent(v)) {
        return (Part::Voice, Some(voice));
    }
    (Part::Record, None)
}

/// What was playing `into` a record, in words, for the DJ's provider to read
/// a reaction against: `"Artist – Title, 1:23 in, just after the drop"` —
/// the part as [`place`] would find it for a reaction about the moment.
#[must_use]
pub fn playing(played: &Played, into: f64) -> String {
    let name = if played.artist.is_empty() {
        played.title.clone()
    } else {
        format!("{} – {}", played.artist, played.title)
    };
    #[allow(clippy::cast_possible_truncation)]
    let seconds = into.max(0.0) as i64;
    let part = match part_of(played, into).0 {
        Part::Drop => ", just after the drop",
        Part::Breakdown => ", in the breakdown",
        Part::Voice => ", as the voice comes in",
        Part::Record => "",
    };
    format!("{name}, {}:{:02} in{part}", seconds / 60, seconds % 60)
}

/// How long a record is taken to have played when the collection does not
/// know its length and nothing after it says when it stopped.
pub const UNKNOWN_LENGTH: i64 = 180;

/// A stretch of the night in which the room heard one record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    /// Which of the records, by its place in the list.
    pub record: usize,
    /// When the room began and stopped hearing it, unix seconds.
    pub from: i64,
    pub to: i64,
    /// How far into the record the room was at `from`, in seconds: a record
    /// brought up over another is heard from partway in.
    pub into: f64,
}

/// A part of a record, at the moment the room heard it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Landmark {
    /// Unix seconds.
    pub at: i64,
    pub record: usize,
    pub part: Part,
}

/// A night on one line: what the room heard and when, the parts of it, and
/// when the reactions were about.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Timeline {
    /// Where the line begins and ends, unix seconds.
    pub from: i64,
    pub to: i64,
    pub spans: Vec<Span>,
    /// Drops, breakdowns and the voice arriving, inside the spans they were
    /// heard in — a drop the room never heard is not one of the night's.
    pub landmarks: Vec<Landmark>,
    /// When the room heard what each reaction was about — said less the
    /// stream's delay — in the order of the reactions given.
    pub heard: Vec<i64>,
    /// Reactions per minute from `from`, by when the room heard it.
    pub per_minute: Vec<usize>,
}

/// The night on one line.
///
/// What the room heard is `room`'s, where the night measured it: each change
/// begins a stretch of the record it names (the latest of that record to come
/// in by then), and silence ends one. A night kept before it was measured is
/// drawn by arrival: each record from when it came in to when the next did.
/// Either way a stretch ends no later than the record's own end, when its
/// length is known, and the last one, when nothing says otherwise, after
/// [`UNKNOWN_LENGTH`]. `None` for a night with nothing in it.
#[must_use]
pub fn timeline(
    played: &[Played],
    room: &[Heard],
    placed: &[Placed],
    delay: i64,
) -> Option<Timeline> {
    #[allow(clippy::cast_possible_truncation)]
    let ends = |record: &Played| record.length.map(|l| record.at + l.round() as i64);
    let stop = |record: &Played, from: i64, next: Option<i64>| -> i64 {
        match (next, ends(record)) {
            (Some(next), Some(end)) => next.min(end),
            (Some(next), None) => next,
            (None, Some(end)) => end,
            (None, None) => from + UNKNOWN_LENGTH,
        }
    };
    let mut spans: Vec<Span> = Vec::new();
    if room.is_empty() {
        for (i, record) in played.iter().enumerate() {
            let to = stop(record, record.at, played.get(i + 1).map(|n| n.at));
            if to > record.at {
                spans.push(Span {
                    record: i,
                    from: record.at,
                    to,
                    into: 0.0,
                });
            }
        }
    } else {
        for (k, change) in room.iter().enumerate() {
            let Some(id) = &change.track_id else {
                continue;
            };
            let Some(i) = played
                .iter()
                .rposition(|p| &p.track_id == id && p.at <= change.at)
            else {
                continue;
            };
            let record = &played[i];
            let to = stop(record, change.at, room.get(k + 1).map(|n| n.at));
            if to > change.at {
                #[allow(clippy::cast_precision_loss)]
                spans.push(Span {
                    record: i,
                    from: change.at,
                    to,
                    into: (change.at - record.at) as f64,
                });
            }
        }
    }

    let mut landmarks: Vec<Landmark> = Vec::new();
    for span in &spans {
        let record = &played[span.record];
        #[allow(clippy::cast_possible_truncation)]
        let at = |seconds: f64| record.at + seconds.round() as i64;
        let parts = record
            .drops
            .iter()
            .map(|&d| (at(d), Part::Drop))
            .chain(
                record
                    .breakdowns
                    .iter()
                    .map(|&(b, _)| (at(b), Part::Breakdown)),
            )
            .chain(record.vocal.map(|v| (at(v), Part::Voice)));
        for (when, part) in parts {
            if span.from <= when && when < span.to {
                landmarks.push(Landmark {
                    at: when,
                    record: span.record,
                    part,
                });
            }
        }
    }
    landmarks.sort_by_key(|l| l.at);

    let heard: Vec<i64> = placed.iter().map(|p| p.reaction.at - delay).collect();
    let from = spans
        .iter()
        .map(|s| s.from)
        .chain(heard.iter().copied())
        .min()?;
    let to = spans
        .iter()
        .map(|s| s.to)
        .chain(heard.iter().map(|h| h + 1))
        .max()?;
    let minutes = usize::try_from((to - from + 59) / 60).unwrap_or(0).max(1);
    let mut per_minute = vec![0; minutes];
    for h in &heard {
        if let Ok(m) = usize::try_from((h - from) / 60)
            && let Some(slot) = per_minute.get_mut(m)
        {
            *slot += 1;
        }
    }
    Some(Timeline {
        from,
        to,
        spans,
        landmarks,
        heard,
        per_minute,
    })
}

/// A record, as the night received it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reception {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub reactions: usize,
    pub up: usize,
    pub down: usize,
    /// People asking what it was: the plainest sign a record landed.
    pub asked: usize,
    /// Its moments, by part and place, most reacted first.
    pub moments: Vec<Moment>,
}

/// A moment in a record that drew reactions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Moment {
    pub part: Part,
    /// Seconds into the record: the part's own place when there is one.
    pub at: f64,
    pub count: usize,
}

/// The night, read back.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub reactions: usize,
    /// The most in any one minute.
    pub busiest_minute: usize,
    pub up: usize,
    pub down: usize,
    /// Of the reactions that leaned, the share that leaned up; `None` when
    /// none leaned.
    pub warmth: Option<f64>,
    pub moments: usize,
    pub asked: usize,
    pub requests: usize,
    pub greetings: usize,
    pub by_source: Vec<(Source, usize)>,
    /// Every record that drew anything, most reacted first.
    pub records: Vec<Reception>,
    /// Reactions per minute of the night, from its first reaction.
    pub pace: Vec<usize>,
}

/// Read the night back.
#[must_use]
pub fn summary(placed: &[Placed], played: &[Played]) -> Summary {
    let count = |about: About| placed.iter().filter(|p| p.about == about).count();
    let up = placed.iter().filter(|p| p.lean == Lean::Up).count();
    let down = placed.iter().filter(|p| p.lean == Lean::Down).count();
    #[allow(clippy::cast_precision_loss)]
    let warmth = (up + down > 0).then(|| up as f64 / (up + down) as f64);

    let mut by_source: Vec<(Source, usize)> = Vec::new();
    for p in placed {
        match by_source.iter_mut().find(|(s, _)| *s == p.reaction.source) {
            Some((_, n)) => *n += 1,
            None => by_source.push((p.reaction.source, 1)),
        }
    }

    let first = placed.iter().map(|p| p.reaction.at).min();
    let last = placed.iter().map(|p| p.reaction.at).max();
    let pace = match (first, last) {
        (Some(first), Some(last)) => {
            let minutes = usize::try_from((last - first) / 60 + 1).unwrap_or(1);
            let mut pace = vec![0; minutes];
            for p in placed {
                if let Ok(m) = usize::try_from((p.reaction.at - first) / 60)
                    && let Some(slot) = pace.get_mut(m)
                {
                    *slot += 1;
                }
            }
            pace
        }
        _ => Vec::new(),
    };

    let mut records: Vec<Reception> = played
        .iter()
        .enumerate()
        .filter_map(|(i, record)| {
            let on: Vec<&Placed> = placed.iter().filter(|p| p.record == Some(i)).collect();
            if on.is_empty() {
                return None;
            }
            let mut moments: Vec<Moment> = Vec::new();
            for p in on.iter().filter(|p| p.about == About::Moment) {
                // A moment with a part is gathered at the part; one without,
                // to the nearest ten seconds, so a burst about one bar is one
                // moment and not twelve.
                let at = p.part_at.unwrap_or_else(|| (p.into / 10.0).round() * 10.0);
                match moments
                    .iter_mut()
                    .find(|m| m.part == p.part && (m.at - at).abs() < f64::EPSILON)
                {
                    Some(m) => m.count += 1,
                    None => moments.push(Moment {
                        part: p.part,
                        at,
                        count: 1,
                    }),
                }
            }
            moments.sort_by(|a, b| b.count.cmp(&a.count).then(a.at.total_cmp(&b.at)));
            Some(Reception {
                track_id: record.track_id.clone(),
                title: record.title.clone(),
                artist: record.artist.clone(),
                reactions: on.len(),
                up: on.iter().filter(|p| p.lean == Lean::Up).count(),
                down: on.iter().filter(|p| p.lean == Lean::Down).count(),
                asked: on.iter().filter(|p| p.about == About::Track).count(),
                moments,
            })
        })
        .collect();
    records.sort_by_key(|r| std::cmp::Reverse(r.reactions));

    Summary {
        reactions: placed.len(),
        busiest_minute: pace.iter().copied().max().unwrap_or(0),
        up,
        down,
        warmth,
        moments: count(About::Moment),
        asked: count(About::Track),
        requests: count(About::Request),
        greetings: count(About::Greeting),
        by_source,
        records,
        pace,
    }
}

/// A moment in one record that past crowds reacted to, gathered over every
/// night kept: what §25's *crowd response* layer draws.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mark {
    /// Seconds into the record: the part's own place when there is one.
    pub at: f64,
    pub part: Part,
    /// How many reactions, over every night.
    pub count: usize,
    /// On how many different nights -- the number that says a moment
    /// lands, where one loud night says only that it did once.
    pub nights: usize,
}

impl Mark {
    /// What the mark says when pointed at.
    #[must_use]
    pub fn says(&self) -> String {
        let reactions = if self.count == 1 {
            "1 reaction".to_owned()
        } else {
            format!("{} reactions", self.count)
        };
        let nights = if self.nights == 1 {
            "one night".to_owned()
        } else {
            format!("{} nights", self.nights)
        };
        let part = match self.part {
            Part::Drop => " to the drop",
            Part::Breakdown => " in the breakdown",
            Part::Voice => " as the voice came in",
            Part::Record => "",
        };
        format!("{reactions}{part}, on {nights}")
    }
}

/// The moments in `track_id` that crowds reacted to, over every night kept
/// -- each night its reactions placed on its own records -- most reacted
/// first. Gathered as [`summary`] gathers one night's: at the part when
/// there is one, to the nearest ten seconds when there is not.
#[must_use]
pub fn marks(nights: &[(Vec<Placed>, Vec<Played>)], track_id: &str) -> Vec<Mark> {
    let mut out: Vec<(Mark, Vec<usize>)> = Vec::new();
    for (night, (placed, played)) in nights.iter().enumerate() {
        for p in placed.iter().filter(|p| p.about == About::Moment) {
            let Some(record) = p.record.and_then(|i| played.get(i)) else {
                continue;
            };
            if record.track_id != track_id {
                continue;
            }
            let at = p.part_at.unwrap_or_else(|| (p.into / 10.0).round() * 10.0);
            match out
                .iter_mut()
                .find(|(m, _)| m.part == p.part && (m.at - at).abs() < f64::EPSILON)
            {
                Some((m, seen)) => {
                    m.count += 1;
                    if !seen.contains(&night) {
                        seen.push(night);
                        m.nights += 1;
                    }
                }
                None => out.push((
                    Mark {
                        at,
                        part: p.part,
                        count: 1,
                        nights: 1,
                    },
                    vec![night],
                )),
            }
        }
    }
    let mut marks: Vec<Mark> = out.into_iter().map(|(m, _)| m).collect();
    marks.sort_by(|a, b| {
        b.nights
            .cmp(&a.nights)
            .then(b.count.cmp(&a.count))
            .then(a.at.total_cmp(&b.at))
    });
    marks
}

/// What a goal measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Measure {
    /// The share of leaning reactions that leaned up, 0 to 1.
    Warmth,
    /// The busiest minute's reactions.
    BusiestMinute,
    /// Reactions about the moment.
    Moments,
    /// People asking what was playing.
    Asked,
    /// Requests.
    Requests,
    /// Reactions that leaned down.
    Down,
}

impl Measure {
    pub const ALL: [Measure; 6] = [
        Measure::Warmth,
        Measure::BusiestMinute,
        Measure::Moments,
        Measure::Asked,
        Measure::Requests,
        Measure::Down,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Measure::Warmth => "Share of reactions that were warm",
            Measure::BusiestMinute => "Reactions in the busiest minute",
            Measure::Moments => "Reactions to a moment in the music",
            Measure::Asked => "Times people asked what was playing",
            Measure::Requests => "Requests",
            Measure::Down => "Reactions that were cold",
        }
    }

    /// Its value for a night; `None` when the night cannot say.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn of(self, summary: &Summary) -> Option<f64> {
        match self {
            Measure::Warmth => summary.warmth,
            Measure::BusiestMinute => Some(summary.busiest_minute as f64),
            Measure::Moments => Some(summary.moments as f64),
            Measure::Asked => Some(summary.asked as f64),
            Measure::Requests => Some(summary.requests as f64),
            Measure::Down => Some(summary.down as f64),
        }
    }
}

/// How one night met a record it shares with another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Met {
    /// Whether this night played it at all.
    pub played: bool,
    pub reactions: usize,
    pub up: usize,
    pub down: usize,
    /// Reactions to a moment in it.
    pub moments: usize,
}

/// A record two or more of the nights played.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shared {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    /// Each night's, in the order the nights were given.
    pub nights: Vec<Met>,
}

/// Nights side by side.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compared {
    /// How many reactions each night drew.
    pub reactions: Vec<usize>,
    /// Each of the goals' measures, with each night's value; `None` where a
    /// night cannot say.
    pub measures: Vec<(Measure, Vec<Option<f64>>)>,
    /// The records two or more nights played, the most played first, then
    /// the most reacted to — and a night that played one to silence says so.
    pub shared: Vec<Shared>,
}

/// Nights side by side, on the measures the goals use, and the records they
/// share with how each night met them.
#[must_use]
pub fn compare(nights: &[(&Summary, &[Played])]) -> Compared {
    let mut shared: Vec<Shared> = Vec::new();
    for (_, played) in nights {
        for record in *played {
            if shared.iter().any(|s| s.track_id == record.track_id) {
                continue;
            }
            let met: Vec<Met> = nights
                .iter()
                .map(|(other, other_played)| {
                    let played = other_played.iter().any(|p| p.track_id == record.track_id);
                    let reception = other.records.iter().find(|r| r.track_id == record.track_id);
                    Met {
                        played,
                        reactions: reception.map_or(0, |r| r.reactions),
                        up: reception.map_or(0, |r| r.up),
                        down: reception.map_or(0, |r| r.down),
                        moments: reception.map_or(0, |r| r.moments.iter().map(|m| m.count).sum()),
                    }
                })
                .collect();
            if met.iter().filter(|m| m.played).count() >= 2 {
                shared.push(Shared {
                    track_id: record.track_id.clone(),
                    title: record.title.clone(),
                    artist: record.artist.clone(),
                    nights: met,
                });
            }
        }
    }
    shared.sort_by_key(|s| {
        (
            std::cmp::Reverse(s.nights.iter().filter(|m| m.played).count()),
            std::cmp::Reverse(s.nights.iter().map(|m| m.reactions).sum::<usize>()),
        )
    });
    Compared {
        reactions: nights.iter().map(|(s, _)| s.reactions).collect(),
        measures: Measure::ALL
            .into_iter()
            .map(|m| (m, nights.iter().map(|(s, _)| m.of(s)).collect()))
            .collect(),
        shared,
    }
}

/// One of the DJ's questions of a night.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub name: String,
    pub measure: Measure,
    /// At least this, or — when `at_most` — no more than it.
    pub target: f64,
    #[serde(default)]
    pub at_most: bool,
}

/// A goal, answered for a night.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answered {
    pub goal: Goal,
    pub value: Option<f64>,
    /// `None` when the night cannot say.
    pub met: Option<bool>,
}

/// Answer the DJ's goals for a night.
#[must_use]
pub fn answer(goals: &[Goal], summary: &Summary) -> Vec<Answered> {
    goals
        .iter()
        .map(|goal| {
            let value = goal.measure.of(summary);
            let met = value.map(|v| {
                if goal.at_most {
                    v <= goal.target
                } else {
                    v >= goal.target
                }
            });
            Answered {
                goal: goal.clone(),
                value,
                met,
            }
        })
        .collect()
}

/// The usual questions for a kind of night: what a DJ playing it would want
/// to know afterwards. A starting point the DJ edits, not a verdict.
#[must_use]
pub fn goals_for(night: Option<Setting>) -> Vec<Goal> {
    let goal = |name: &str, measure, target, at_most| Goal {
        name: name.to_owned(),
        measure,
        target,
        at_most,
    };
    let warm = goal("Most of the room was with me", Measure::Warmth, 0.75, false);
    match night {
        Some(Setting::Club) => vec![
            warm,
            goal("The peak landed", Measure::BusiestMinute, 10.0, false),
            goal("They asked what was playing", Measure::Asked, 5.0, false),
        ],
        Some(Setting::Wedding) => vec![
            goal("Everyone was with me", Measure::Warmth, 0.85, false),
            goal("Few complaints", Measure::Down, 3.0, true),
            goal("Requests came in", Measure::Requests, 3.0, false),
        ],
        Some(Setting::Latin) => vec![
            warm,
            goal(
                "The floor answered the moments",
                Measure::Moments,
                10.0,
                false,
            ),
        ],
        Some(Setting::Beach) => vec![
            warm,
            goal("They asked what was playing", Measure::Asked, 3.0, false),
        ],
        Some(Setting::Practice) => vec![goal("Moments that landed", Measure::Moments, 3.0, false)],
        Some(Setting::OpenFormat) | None => vec![
            warm,
            goal("Moments that landed", Measure::Moments, 5.0, false),
        ],
    }
}

/// The folder a night's reactions are kept in.
#[must_use]
pub fn folder(config: &Path) -> PathBuf {
    config.join("crowd")
}

fn is_session(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The file a session's reactions are kept in.
///
/// # Errors
/// A session id that would not be a plain file name.
pub fn file(config: &Path, session: &str) -> Result<PathBuf, String> {
    if !is_session(session) {
        return Err(format!("{session:?} is not a session"));
    }
    Ok(folder(config).join(format!("{session}.jsonl")))
}

/// Add reactions to a session's file.
///
/// # Errors
/// The file system's own sentence.
pub fn append(config: &Path, session: &str, reactions: &[Reaction]) -> Result<(), String> {
    use std::io::Write as _;
    let path = file(config, session)?;
    std::fs::create_dir_all(folder(config)).map_err(|e| e.to_string())?;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    for reaction in reactions {
        let line = serde_json::to_string(reaction).map_err(|e| e.to_string())?;
        writeln!(out, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// Add the reactions not already kept — the same moment, the same name, the
/// same words — and say how many were new: a source that hands the same
/// message over twice, or again after a restart, keeps it once.
///
/// # Errors
/// The file system's own sentence.
pub fn append_new(config: &Path, session: &str, reactions: Vec<Reaction>) -> Result<usize, String> {
    let kept = load(config, session);
    let mut fresh: Vec<Reaction> = Vec::new();
    for reaction in reactions {
        let same =
            |k: &Reaction| k.at == reaction.at && k.who == reaction.who && k.text == reaction.text;
        if !kept.iter().any(same) && !fresh.iter().any(same) {
            fresh.push(reaction);
        }
    }
    if !fresh.is_empty() {
        append(config, session, &fresh)?;
    }
    Ok(fresh.len())
}

/// A session's reactions, oldest first. A line that is not a reaction is
/// skipped rather than losing the night for one bad line.
#[must_use]
pub fn load(config: &Path, session: &str) -> Vec<Reaction> {
    let Ok(path) = file(config, session) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<Reaction> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    out.sort_by_key(|r| r.at);
    out
}

/// The file a session's [`Heard`] lines are kept in: a folder of its own,
/// so it is never listed as a night of reactions.
///
/// # Errors
/// A session id that would not be a plain file name.
pub fn room_file(config: &Path, session: &str) -> Result<PathBuf, String> {
    if !is_session(session) {
        return Err(format!("{session:?} is not a session"));
    }
    Ok(folder(config).join("room").join(format!("{session}.jsonl")))
}

/// Note that the room moved to a different record.
///
/// # Errors
/// The file system's own sentence.
pub fn note_room(config: &Path, session: &str, heard: &Heard) -> Result<(), String> {
    use std::io::Write as _;
    let path = room_file(config, session)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let line = serde_json::to_string(heard).map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut out| writeln!(out, "{line}"))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// What the room heard through a session, oldest first; empty for a night
/// kept before it was measured. A bad line is skipped, as in [`load`].
#[must_use]
pub fn load_room(config: &Path, session: &str) -> Vec<Heard> {
    let Ok(path) = room_file(config, session) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<Heard> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    out.sort_by_key(|heard| heard.at);
    out
}

/// What the DJ's AI provider read in a reaction the rules could not place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    /// The reaction it is about, as kept: when, who, and what.
    pub at: i64,
    #[serde(default)]
    pub who: String,
    pub text: String,
    /// [`About::Other`] when the provider could not place it either.
    pub about: About,
    pub lean: Lean,
}

impl Reading {
    /// Whether this is the reading of `reaction`.
    #[must_use]
    pub fn of(&self, reaction: &Reaction) -> bool {
        self.at == reaction.at && self.who == reaction.who && self.text == reaction.text
    }
}

/// A batch is due when this many reactions are waiting to be read…
pub const BATCH: usize = 20;

/// …or the oldest has waited this many seconds, whichever comes first.
pub const BATCH_WAIT: i64 = 120;

/// After the provider failed, the batches wait this many seconds before
/// asking again: a local model that is not running is not asked every few
/// seconds all night.
pub const RETRY_AFTER: i64 = 300;

/// The reactions the rules leave as something else and the provider has not
/// been asked about, in the order given.
#[must_use]
pub fn unread<'a>(reactions: &'a [Reaction], readings: &[Reading]) -> Vec<&'a Reaction> {
    let asked: std::collections::HashSet<(i64, &str, &str)> = readings
        .iter()
        .map(|r| (r.at, r.who.as_str(), r.text.as_str()))
        .collect();
    reactions
        .iter()
        .filter(|r| {
            read(&r.text).0 == About::Other
                && !asked.contains(&(r.at, r.who.as_str(), r.text.as_str()))
        })
        .collect()
}

/// Whether a night's next batch is due at `now` (unix seconds): [`BATCH`]
/// waiting, or the oldest waiting [`BATCH_WAIT`] seconds.
#[must_use]
pub fn due(unread: &[&Reaction], now: i64) -> bool {
    unread.len() >= BATCH
        || unread
            .iter()
            .map(|r| r.at)
            .min()
            .is_some_and(|oldest| now - oldest >= BATCH_WAIT)
}

/// How asking the provider is going, for the panel and the batches.
#[derive(Debug, Default)]
pub struct Asking {
    /// A batch is out; another is not sent beside it.
    pub busy: bool,
    /// What went wrong last, in the provider's words; cleared by a batch
    /// that came back.
    pub error: Option<String>,
    /// Not before this, in unix seconds: set [`RETRY_AFTER`] ahead by a
    /// failure.
    pub retry_at: i64,
}

/// The file a session's [`Reading`]s are kept in: a folder of its own, so it
/// is never listed as a night of reactions.
///
/// # Errors
/// A session id that would not be a plain file name.
pub fn readings_file(config: &Path, session: &str) -> Result<PathBuf, String> {
    if !is_session(session) {
        return Err(format!("{session:?} is not a session"));
    }
    Ok(folder(config).join("read").join(format!("{session}.jsonl")))
}

/// Keep what the provider read.
///
/// # Errors
/// The file system's own sentence.
pub fn note_readings(config: &Path, session: &str, readings: &[Reading]) -> Result<(), String> {
    use std::io::Write as _;
    let path = readings_file(config, session)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    for reading in readings {
        let line = serde_json::to_string(reading).map_err(|e| e.to_string())?;
        writeln!(out, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// What the provider read through a session; a bad line is skipped, as in
/// [`load`].
#[must_use]
pub fn load_readings(config: &Path, session: &str) -> Vec<Reading> {
    let Ok(path) = readings_file(config, session) else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Read a chat log brought in afterwards: one message a line, as
/// `H:MM:SS name: message` (or `MM:SS`), timed from `start` — the moment
/// the stream or the recording began. A line without a time is skipped.
#[must_use]
pub fn import(text: &str, start: i64, source: Source) -> Vec<Reaction> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (clock, rest) = line.split_once(char::is_whitespace)?;
            let seconds = clock_seconds(clock.trim_end_matches(','))?;
            let rest = rest.trim();
            let (who, said) = match rest.split_once(": ") {
                Some((who, said)) if !who.is_empty() && who.len() <= 40 => {
                    (who.to_owned(), said.to_owned())
                }
                _ => (String::new(), rest.to_owned()),
            };
            (!said.trim().is_empty()).then(|| Reaction {
                at: start + seconds,
                source,
                who,
                text: said.trim().to_owned(),
            })
        })
        .collect()
}

/// `H:MM:SS` or `MM:SS` as seconds.
fn clock_seconds(clock: &str) -> Option<i64> {
    let parts: Vec<&str> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }
    let numbers: Vec<i64> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let seconds = numbers.iter().fold(0, |total, n| total * 60 + n);
    // Minutes and seconds past 59 are not a clock.
    (numbers.iter().skip(1).all(|&n| n < 60)).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(at: i64, text: &str) -> Reaction {
        Reaction {
            at,
            source: Source::YouTube,
            who: "someone".to_owned(),
            text: text.to_owned(),
        }
    }

    /// **A reaction is read for what it is about**, and for which way it
    /// leans: a request, a question about the record, the moment, a hello,
    /// or nothing -- and a warm word, or a cold one.
    #[test]
    fn a_reaction_is_read_for_what_it_is_about() {
        assert_eq!(read("play despacito please").0, About::Request);
        assert_eq!(
            read("Can you play something by Bad Bunny?").0,
            About::Request
        );
        assert_eq!(read("what song is this??").0, About::Track);
        assert_eq!(read("track ID?").0, About::Track);
        assert_eq!(read("THIS DROP 🔥🔥").0, About::Moment);
        assert_eq!(read("🙌🙌🙌").0, About::Moment);
        assert_eq!(read("hello from Lisbon").0, About::Greeting);
        assert_eq!(read("hi").0, About::Greeting);
        assert_eq!(read("the weather is nice today"), (About::Other, Lean::Up));
        assert_eq!(read("this is boring, skip").1, Lean::Down);
        assert_eq!(read("love this ❤️").1, Lean::Up);
        assert_eq!(read("ok").1, Lean::Neither);
        // A cold word outweighs a warm one: "love the set but skip this"
        // is somebody asking for it to stop.
        assert_eq!(read("love the set but skip this").1, Lean::Down);
    }

    fn night() -> Vec<Played> {
        vec![
            Played {
                at: 1_000,
                track_id: "a".to_owned(),
                title: "First".to_owned(),
                artist: "One".to_owned(),
                drops: vec![60.0],
                breakdowns: vec![(40.0, 60.0)],
                vocal: Some(20.0),
                length: None,
            },
            Played {
                at: 1_200,
                track_id: "b".to_owned(),
                title: "Second".to_owned(),
                artist: "Two".to_owned(),
                drops: Vec::new(),
                breakdowns: Vec::new(),
                vocal: None,
                length: None,
            },
        ]
    }

    /// Two records, `a` from 1000 and `b` started in the headphones at 1100
    /// with its fader down, brought up over `a` at 1150; `a` stopped at 1180.
    /// Read off the meters every tenth of a second, as the interface is
    /// drawn, and what the room was said to hear each time it moved.
    fn a_blend(cut_back_at: Option<f64>) -> (dj_core::TrackId, dj_core::TrackId, Vec<Heard>) {
        let a = dj_core::TrackId::from_bytes([1; 32]);
        let b = dj_core::TrackId::from_bytes([2; 32]);
        let mut room = Hearing::new();
        let mut heard = Vec::new();
        for tenth in 9_950..12_000 {
            let now = f64::from(tenth) / 10.0;
            let b_up = now >= 1_150.0 && cut_back_at.is_none_or(|back| now < back);
            let decks = [
                Meter {
                    deck: 1,
                    track: Some(a),
                    playing: (1_000.0..1_180.0).contains(&now),
                    level: if b_up { 0.3 } else { 0.8 },
                },
                Meter {
                    deck: 2,
                    track: (now >= 1_090.0).then_some(b),
                    playing: now >= 1_100.0,
                    level: if b_up { 0.8 } else { 0.0 },
                },
            ];
            heard.extend(room.observe(now, &decks));
        }
        (a, b, heard)
    }

    /// **The room is heard by its meters, not by what came in last.** A
    /// record waiting in the headphones is not what the room hears, however
    /// long it has been playing there; in a blend the louder one is; and
    /// the moment is stamped when it became the louder, not two seconds
    /// later when that was sure.
    #[test]
    fn the_room_is_heard_by_its_meters() {
        let (a, b, heard) = a_blend(None);
        assert_eq!(heard.len(), 2, "{heard:?}");
        assert_eq!(heard[0].track_id, Some(a.to_hex()));
        assert!((1_000..=1_001).contains(&heard[0].at), "{heard:?}");
        assert_eq!(heard[1].track_id, Some(b.to_hex()));
        // Smoothed over a second, `b` passes `a` just under a second after
        // the fader went up.
        assert!((1_150..=1_151).contains(&heard[1].at), "{heard:?}");

        // Two seconds of `b` and back to `a` does not move the room: `b`
        // leads for well under the hold, from just before 1151 until a
        // third of a second after it is cut.
        let (_, _, cut) = a_blend(Some(1_152.0));
        assert_eq!(
            cut.iter()
                .filter(|h| h.track_id == Some(b.to_hex()))
                .count(),
            0,
            "{cut:?}"
        );
    }

    /// **Placed on the record the room was hearing, measured.** `b` came in
    /// at 1100 — the history stamps when a deck started — and the room did
    /// not hear it until 1150: said at 1120, a reaction is `a`'s. Placed by
    /// arrival it was `b`'s, which is what a night kept before the room was
    /// measured still does.
    #[test]
    fn a_reaction_goes_to_the_record_the_room_heard() {
        let (a, b, room) = a_blend(None);
        let played = vec![
            Played {
                at: 1_000,
                track_id: a.to_hex(),
                title: "First".to_owned(),
                artist: "One".to_owned(),
                drops: vec![118.0],
                breakdowns: Vec::new(),
                vocal: None,
                length: None,
            },
            Played {
                at: 1_100,
                track_id: b.to_hex(),
                title: "Second".to_owned(),
                artist: "Two".to_owned(),
                drops: Vec::new(),
                breakdowns: Vec::new(),
                vocal: None,
                length: None,
            },
        ];
        let reactions = [said(1_120, "this drop 🔥"), said(1_170, "love this")];
        let placed = place(&reactions, &played, &room, 0, &[]);
        assert_eq!(placed[0].record, Some(0), "waiting in the headphones");
        assert!((placed[0].into - 120.0).abs() < 1e-9);
        assert_eq!(
            (placed[0].part, placed[0].part_at),
            (Part::Drop, Some(118.0)),
            "and on the part of the record the room heard"
        );
        assert_eq!(placed[1].record, Some(1), "brought up over it");

        let by_arrival = place(&reactions, &played, &[], 0, &[]);
        assert_eq!(by_arrival[0].record, Some(1));

        // A record the room heard that the history does not hold -- cut in
        // and out before it counted as played -- leaves it to arrival.
        let unknown = [Heard {
            at: 1_110,
            track_id: Some("c".to_owned()),
        }];
        assert_eq!(
            place(&reactions, &played, &unknown, 0, &[])[0].record,
            Some(1)
        );
    }

    /// Kept beside the night, in a folder of its own so it is never listed
    /// as a night of reactions, and read back in order past a bad line.
    #[test]
    fn what_the_room_heard_is_kept() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let later = Heard {
            at: 20,
            track_id: None,
        };
        let first = Heard {
            at: 10,
            track_id: Some("a".to_owned()),
        };
        note_room(config, "session-1", &later).expect("kept");
        note_room(config, "session-1", &first).expect("kept");
        let path = room_file(config, "session-1").expect("a file");
        let mut text = std::fs::read_to_string(&path).expect("read");
        text.push_str("not a line\n");
        std::fs::write(&path, text).expect("written");
        assert_eq!(load_room(config, "session-1"), vec![first, later]);
        assert_ne!(path.parent(), Some(folder(config).as_path()));
        assert!(load_room(config, "session-2").is_empty());
        assert!(
            note_room(
                config,
                "../x",
                &Heard {
                    at: 0,
                    track_id: None
                }
            )
            .is_err()
        );
    }

    /// **Placed on what the room was hearing**, less the stream's delay,
    /// and a reaction to the moment on the part it most likely meant: the
    /// drop just before it, the breakdown it is inside, the voice arriving
    /// -- and on the record, not on a guess, when the analysis knows none.
    #[test]
    fn a_reaction_is_placed_on_the_music_it_was_about() {
        let played = night();
        let placed = place(
            &[
                said(990, "early 🔥"),
                // 8 s of delay: said at 1070, heard at 1062 -- 62 s in, two
                // seconds after the drop at 60.
                said(1_070, "this drop 🔥"),
                // Heard at 1045: 45 s in, inside the breakdown.
                said(1_053, "omg"),
                // Heard at 1025: the voice came in at 20.
                said(1_033, "🙌"),
                // A question is about the record, whatever the part.
                said(1_070, "what song is this"),
                // The second record has no parts known.
                said(1_308, "🔥🔥"),
            ],
            &played,
            &[],
            8,
            &[],
        );
        assert_eq!((placed[0].record, placed[0].part), (None, Part::Record));
        assert_eq!(
            (placed[1].record, placed[1].part, placed[1].part_at),
            (Some(0), Part::Drop, Some(60.0))
        );
        assert!((placed[1].into - 62.0).abs() < 1e-9);
        assert_eq!(
            (placed[2].part, placed[2].part_at),
            (Part::Breakdown, Some(40.0))
        );
        assert_eq!(
            (placed[3].part, placed[3].part_at),
            (Part::Voice, Some(20.0))
        );
        assert_eq!(
            (placed[4].about, placed[4].part),
            (About::Track, Part::Record)
        );
        assert_eq!(
            (placed[5].record, placed[5].part, placed[5].part_at),
            (Some(1), Part::Record, None)
        );

        // Without the delay, the drop comment is heard at 70 s: ten after
        // the drop, still within the window.
        assert_eq!(
            place(&[said(1_070, "this drop")], &played, &[], 0, &[])[0].part,
            Part::Drop
        );
        // Far past the window, it is not the drop any more.
        assert_eq!(
            place(&[said(1_095, "this drop")], &played, &[], 0, &[])[0].part,
            Part::Record
        );
    }

    /// **The night read back**: counts, the busiest minute, which way it
    /// leaned, and the records and moments that drew the most -- a burst
    /// about one drop gathered as one moment.
    #[test]
    fn the_night_is_read_back_by_record_and_moment() {
        let played = night();
        let reactions: Vec<Reaction> = [
            (1_065, "this drop 🔥"),
            (1_066, "🔥🔥"),
            (1_068, "insane"),
            (1_070, "what song is this"),
            (1_150, "boring"),
            (1_250, "play despacito"),
        ]
        .into_iter()
        .map(|(at, text)| said(at, text))
        .collect();
        let placed = place(&reactions, &played, &[], 0, &[]);
        let night = summary(&placed, &played);
        assert_eq!(night.reactions, 6);
        assert_eq!((night.moments, night.asked, night.requests), (3, 1, 1));
        assert_eq!(night.busiest_minute, 4);
        assert_eq!(night.pace, vec![4, 1, 0, 1]);
        assert_eq!((night.up, night.down), (3, 1));
        assert!((night.warmth.unwrap() - 0.75).abs() < 1e-9);
        assert_eq!(night.records[0].title, "First");
        assert_eq!(night.records[0].reactions, 5);
        assert_eq!(night.records[0].asked, 1);
        assert_eq!(
            night.records[0].moments,
            [Moment {
                part: Part::Drop,
                at: 60.0,
                count: 3
            }],
            "three reactions to one drop are one moment"
        );
        assert_eq!(
            summary(&[], &played).warmth,
            None,
            "nothing leaned: nothing to say"
        );
    }

    /// **The DJ's goals, answered** -- at least, or at most -- and a goal
    /// the night cannot answer says so rather than failing.
    #[test]
    fn goals_are_answered_and_the_unanswerable_says_so() {
        let played = night();
        let placed = place(
            &[said(1_065, "this drop 🔥"), said(1_150, "boring")],
            &played,
            &[],
            0,
            &[],
        );
        let night = summary(&placed, &played);
        let answered = answer(&goals_for(Some(Setting::Wedding)), &night);
        assert_eq!(answered[0].goal.measure, Measure::Warmth);
        assert_eq!(answered[0].met, Some(false), "half is not 85%");
        assert_eq!(answered[1].met, Some(true), "one complaint is under three");
        let quiet = answer(&goals_for(None), &summary(&[], &played));
        assert_eq!(quiet[0].met, None, "no reactions: warmth cannot be said");
        for night in Setting::ALL {
            assert!(!goals_for(Some(night)).is_empty(), "{night:?} has no goals");
        }
    }

    /// **The record remembers its crowds.** Over every night kept, the
    /// moments in one record that people reacted to, counted by night as
    /// well as by reaction -- two nights at the drop outrank one loud night
    /// elsewhere -- and nothing from another record.
    #[test]
    fn a_record_remembers_where_its_crowds_reacted() {
        let played = night();
        let one = place(
            &[
                said(1_065, "this drop 🔥"),
                said(1_066, "🔥"),
                said(1_067, "🔥"),
                said(1_110, "omg"),
            ],
            &played,
            &[],
            0,
            &[],
        );
        let two = place(
            &[
                said(1_068, "the drop!!"),
                said(1_250, "🔥 on the second record"),
            ],
            &played,
            &[],
            0,
            &[],
        );
        let nights = vec![(one, played.clone()), (two, played.clone())];
        let first = marks(&nights, "a");
        assert_eq!(
            first[0],
            Mark {
                at: 60.0,
                part: Part::Drop,
                count: 4,
                nights: 2
            },
            "{first:#?}"
        );
        assert_eq!(first[0].says(), "4 reactions to the drop, on 2 nights");
        assert_eq!(first.len(), 2, "the drop, and one other moment");
        assert_eq!((first[1].count, first[1].nights), (1, 1));
        assert!(
            marks(&nights, "b").iter().all(|m| m.count == 1),
            "the second record's own"
        );
        assert!(marks(&nights, "nothing").is_empty());
    }

    /// Kept on its own, a line each, and read back whole even past a bad
    /// line; a session id that is not a file name is refused.
    #[test]
    fn a_nights_reactions_are_kept_on_their_own() {
        let dir = std::env::temp_dir().join(format!("djmanzo-crowd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        append(&dir, "night-1", &[said(20, "second"), said(10, "first")]).unwrap();
        let path = file(&dir, "night-1").unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("not a reaction\n");
        std::fs::write(&path, text).unwrap();
        let back = load(&dir, "night-1");
        assert_eq!(
            back.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert!(file(&dir, "../settings").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A chat log brought in afterwards is timed from when the stream began.
    #[test]
    fn a_chat_log_is_timed_from_the_start() {
        let log =
            "0:05 Ana: hello from Porto\n1:02:10 Ben: THIS DROP 🔥\nno time here\n12:99 bad clock";
        let read = import(log, 1_000, Source::YouTube);
        assert_eq!(read.len(), 2, "{read:?}");
        assert_eq!(
            (read[0].at, read[0].who.as_str(), read[0].text.as_str()),
            (1_005, "Ana", "hello from Porto")
        );
        assert_eq!(read[1].at, 1_000 + 3_730);
    }

    fn reading(of: &Reaction, about: About, lean: Lean) -> Reading {
        Reading {
            at: of.at,
            who: of.who.clone(),
            text: of.text.clone(),
            about,
            lean,
        }
    }

    /// **The provider fills what the rules left, and only that.** A
    /// reaction the rules could not place takes the provider's reading — and
    /// with it a part, when the provider says it was about the moment — and
    /// says it was the provider's; a reaction the rules read keeps the
    /// rules' reading whatever the provider said; a reading the provider
    /// could not place either leaves it as something else; and a reading is
    /// only ever of the reaction it was given — the same words from somebody
    /// else, or at another time, are not it.
    #[test]
    fn the_provider_fills_what_the_rules_left() {
        let played = night();
        // Heard at 1062, two seconds after the drop at 60.
        let bassline = said(1_070, "that bassline though");
        let question = said(1_071, "what song is this");
        let shrug = said(1_072, "hmm");
        let other_one = Reaction {
            who: "somebody else".to_owned(),
            ..bassline.clone()
        };
        let readings = [
            reading(&bassline, About::Moment, Lean::Up),
            reading(&question, About::Greeting, Lean::Down),
            reading(&shrug, About::Other, Lean::Down),
        ];
        let placed = place(
            &[bassline.clone(), question, shrug, other_one],
            &played,
            &[],
            8,
            &readings,
        );
        assert_eq!(
            (
                placed[0].about,
                placed[0].lean,
                placed[0].part,
                placed[0].by_model
            ),
            (About::Moment, Lean::Up, Part::Drop, true)
        );
        assert_eq!(
            (placed[1].about, placed[1].by_model),
            (About::Track, false),
            "the rules' reading stands"
        );
        assert_eq!(
            (placed[2].about, placed[2].lean, placed[2].by_model),
            (About::Other, Lean::Neither, false)
        );
        assert_eq!(
            (placed[3].about, placed[3].by_model),
            (About::Other, false),
            "somebody else's words are not the reading's"
        );
        // Unread before, the same night without the readings.
        assert!(
            !place(&[bassline], &played, &[], 8, &[])[0].by_model,
            "read by the provider with nothing from it"
        );
    }

    /// **A night on one line, by what the room heard.** A record brought up
    /// over another begins its stretch when the room heard it, partway in;
    /// silence ends a stretch; a stretch ends at the record's own end when
    /// its length is known; a drop the room never heard — before it was
    /// brought up — is not on the line, and one it heard is, at the clock
    /// time it was heard; reactions are on the line when what they were
    /// about was heard, less the delay, and counted by that minute.
    #[test]
    fn a_night_on_one_line_is_what_the_room_heard() {
        let mut played = night();
        // The first record's drop at 0:10 is before the room heard it.
        played[0].drops = vec![10.0, 60.0];
        played[1].length = Some(100.0);
        let room = vec![
            Heard {
                at: 1_015,
                track_id: Some("a".to_owned()),
            },
            Heard {
                at: 1_210,
                track_id: Some("b".to_owned()),
            },
            Heard {
                at: 1_400,
                track_id: None,
            },
        ];
        let reactions = [said(1_068, "this drop 🔥"), said(1_250, "🔥")];
        let placed = place(&reactions, &played, &room, 8, &[]);
        let line = timeline(&played, &room, &placed, 8).expect("a night");
        assert_eq!(
            line.spans,
            vec![
                Span {
                    record: 0,
                    from: 1_015,
                    to: 1_210,
                    into: 15.0
                },
                Span {
                    record: 1,
                    from: 1_210,
                    // Its own end, 100 s after it came in at 1200, before
                    // the silence at 1400.
                    to: 1_300,
                    into: 10.0
                },
            ]
        );
        assert_eq!(
            line.landmarks,
            vec![
                Landmark {
                    at: 1_020,
                    record: 0,
                    part: Part::Voice
                },
                Landmark {
                    at: 1_040,
                    record: 0,
                    part: Part::Breakdown
                },
                Landmark {
                    at: 1_060,
                    record: 0,
                    part: Part::Drop
                },
            ]
        );
        assert_eq!(line.heard, vec![1_060, 1_242]);
        assert_eq!((line.from, line.to), (1_015, 1_300));
        assert_eq!(line.per_minute, vec![1, 0, 0, 1, 0]);
    }

    /// **A night kept before the room was measured is drawn by arrival**,
    /// each record until the next came in, the last for
    /// [`UNKNOWN_LENGTH`] when nothing says how long; and a night with
    /// nothing in it has no line.
    #[test]
    fn a_night_without_the_room_is_drawn_by_arrival() {
        let played = night();
        let line = timeline(&played, &[], &[], 0).expect("a night");
        assert_eq!(
            line.spans
                .iter()
                .map(|s| (s.record, s.from, s.to))
                .collect::<Vec<_>>(),
            [(0, 1_000, 1_200), (1, 1_200, 1_200 + UNKNOWN_LENGTH)]
        );
        assert_eq!(line.landmarks.len(), 3, "the first record's parts");
        assert!(timeline(&[], &[], &[], 0).is_none());
    }

    /// **Nights side by side.** Each measure with each night's value, a
    /// night that cannot say saying so; the records two nights share, with
    /// how each met them — a night that played one to silence as played
    /// with nothing, one that did not play it as not played — the record
    /// more nights played first; a record only one night played is not
    /// shared.
    #[test]
    fn nights_are_compared_on_the_goals_measures_and_their_shared_records() {
        let played = night();
        let one_night = [
            said(1_068, "this drop 🔥"),
            said(1_069, "🔥"),
            said(1_250, "boring"),
        ];
        let one = place(&one_night, &played, &[], 8, &[]);
        let first = summary(&one, &played);
        // A second night: the first record again, to silence, and another.
        let mut other = night();
        other[1].track_id = "c".to_owned();
        other[1].title = "Third".to_owned();
        let second = summary(&place(&[said(1_300, "hello")], &other, &[], 8, &[]), &other);
        // A third that did not play the first record at all, but the second.
        let third_played = vec![played[1].clone()];
        let third = summary(&[], &third_played);

        let side = compare(&[
            (&first, &played[..]),
            (&second, &other[..]),
            (&third, &third_played[..]),
        ]);
        assert_eq!(side.reactions, vec![3, 1, 0]);
        let warmth = &side
            .measures
            .iter()
            .find(|(m, _)| *m == Measure::Warmth)
            .expect("warmth")
            .1;
        assert_eq!(warmth[0], Some(2.0 / 3.0));
        assert_eq!(warmth[2], None, "a night with no reactions cannot say");
        assert_eq!(side.measures.len(), Measure::ALL.len());

        assert_eq!(
            side.shared
                .iter()
                .map(|s| s.track_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"],
            "c was played on one night only"
        );
        let a = &side.shared[0];
        assert_eq!(
            a.nights,
            vec![
                Met {
                    played: true,
                    reactions: 2,
                    up: 2,
                    down: 0,
                    moments: 2
                },
                Met {
                    played: true,
                    reactions: 0,
                    up: 0,
                    down: 0,
                    moments: 0
                },
                Met {
                    played: false,
                    reactions: 0,
                    up: 0,
                    down: 0,
                    moments: 0
                },
            ]
        );
        assert_eq!(
            side.shared[1].nights[0],
            Met {
                played: true,
                reactions: 1,
                up: 0,
                down: 1,
                moments: 0
            }
        );
    }

    /// **What was playing, in words**: the record, how far in, and the
    /// part the room was in — as the provider is told it.
    #[test]
    fn what_was_playing_is_said_with_its_part() {
        let played = night();
        assert_eq!(
            playing(&played[0], 62.0),
            "One – First, 1:02 in, just after the drop"
        );
        assert_eq!(
            playing(&played[0], 45.0),
            "One – First, 0:45 in, in the breakdown"
        );
        assert_eq!(
            playing(&played[0], 25.0),
            "One – First, 0:25 in, as the voice comes in"
        );
        assert_eq!(playing(&played[1], 130.0), "Two – Second, 2:10 in");
        let untitled = Played {
            artist: String::new(),
            ..played[1].clone()
        };
        assert_eq!(playing(&untitled, -3.0), "Second, 0:00 in");
    }

    /// **What waits, and when a batch is due.** Only what the rules could
    /// not place and the provider has not been asked about waits; a batch is
    /// due at [`BATCH`] waiting, or once the oldest has waited
    /// [`BATCH_WAIT`] seconds — and never with nothing waiting.
    #[test]
    fn what_waits_and_when_a_batch_is_due() {
        let asked = said(100, "hmm");
        let night = vec![
            asked.clone(),
            said(101, "this drop 🔥"),
            said(102, "play despacito"),
            said(103, "the bassline though"),
            said(104, "ok"),
        ];
        let readings = [reading(&asked, About::Other, Lean::Neither)];
        let waiting = unread(&night, &readings);
        assert_eq!(
            waiting.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["the bassline though", "ok"]
        );
        assert!(!due(&waiting, 103 + BATCH_WAIT - 1), "too soon, too few");
        assert!(due(&waiting, 103 + BATCH_WAIT), "the oldest has waited");
        assert!(!due(&[], i64::MAX), "nothing waiting is never due");

        let busy: Vec<Reaction> = (0..BATCH)
            .map(|i| said(500 + i64::try_from(i).expect("small"), "hmm"))
            .collect();
        let waiting = unread(&busy, &[]);
        assert!(due(&waiting, 500), "a full batch goes at once");
        assert!(!due(&waiting[1..], 500));
    }

    /// **What is handed over twice is kept once** — the same moment, name
    /// and words, whether already kept or twice in one handing — and a
    /// different message from the same person at the same second is kept.
    #[test]
    fn what_is_handed_over_twice_is_kept_once() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let first = vec![said(1, "hola"), said(2, "this drop 🔥")];
        assert_eq!(append_new(config, "session-1", first.clone()), Ok(2));
        let again = vec![
            said(2, "this drop 🔥"),
            said(2, "this drop 🔥"),
            said(2, "and again"),
            said(3, "hola"),
        ];
        assert_eq!(append_new(config, "session-1", again), Ok(2));
        assert_eq!(
            load(config, "session-1")
                .iter()
                .map(|r| (r.at, r.text.as_str()))
                .collect::<Vec<_>>(),
            [
                (1, "hola"),
                (2, "this drop 🔥"),
                (2, "and again"),
                (3, "hola")
            ]
        );
        assert_eq!(append_new(config, "session-1", first), Ok(0));
    }

    /// **Kept beside the night, in a folder of its own**, so it is never
    /// listed as a night; read back past a bad line; a session that is not a
    /// file name refused.
    #[test]
    fn readings_are_kept_beside_the_night() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let one = reading(&said(1, "the bassline though"), About::Moment, Lean::Up);
        let two = reading(&said(2, "hmm"), About::Other, Lean::Neither);
        note_readings(config, "session-1", std::slice::from_ref(&one)).expect("kept");
        let path = readings_file(config, "session-1").expect("a file");
        let mut text = std::fs::read_to_string(&path).expect("read");
        text.push_str("not a reading\n");
        std::fs::write(&path, text).expect("written");
        note_readings(config, "session-1", std::slice::from_ref(&two)).expect("kept");
        assert_eq!(load_readings(config, "session-1"), vec![one, two]);
        assert_ne!(path.parent(), Some(folder(config).as_path()));
        assert!(load_readings(config, "session-2").is_empty());
        assert!(note_readings(config, "../x", &[]).is_err());
    }
}

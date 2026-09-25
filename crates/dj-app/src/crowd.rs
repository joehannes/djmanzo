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
//! is a sentence a test can hold. A model can be asked later about what the
//! rules leave as [`About::Other`]; nothing here waits for one.
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
}

/// Put every reaction on the record the room was hearing, `delay` seconds
/// before it was said, and a reaction about the moment on the part of it
/// that was most likely meant.
#[must_use]
pub fn place(reactions: &[Reaction], played: &[Played], delay: i64) -> Vec<Placed> {
    reactions
        .iter()
        .map(|reaction| {
            let (about, lean) = read(&reaction.text);
            let heard = reaction.at - delay;
            let record = played.iter().rposition(|p| p.at <= heard);
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
            }
        })
        .collect()
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
            },
            Played {
                at: 1_200,
                track_id: "b".to_owned(),
                title: "Second".to_owned(),
                artist: "Two".to_owned(),
                drops: Vec::new(),
                breakdowns: Vec::new(),
                vocal: None,
            },
        ]
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
            8,
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
            place(&[said(1_070, "this drop")], &played, 0)[0].part,
            Part::Drop
        );
        // Far past the window, it is not the drop any more.
        assert_eq!(
            place(&[said(1_095, "this drop")], &played, 0)[0].part,
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
        let placed = place(&reactions, &played, 0);
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
            0,
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
}

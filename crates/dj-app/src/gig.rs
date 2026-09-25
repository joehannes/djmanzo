//! §118: preparing for an event, step by step, and the path that leads to it.
//!
//! > i want you to work on the topic workflow: learn-to-mix/specific-genre-or-
//! > technique → practise the latter → prepare a set on a specific topic →
//! > prepare for a specific event → the live event ... make a workflow and an
//! > activity to specifically prepare a set/show for an event (topic, genre,
//! > location (eg. indoor/outdoor), music, songs, transitions, duration,
//! > fallbacks, extras, wishes, flexibility, alternatives, emergencies,
//! > specials, after bonus ...) and store every step in a decent way ... and
//! > save his preparation/work to load it for the live event
//!
//! # A gig, not a second setlist
//!
//! djmanzo already builds a set (`dj_library::setlist`, the plan panel), knows
//! §81's six kinds of night ([`crate::setting`]), has §54's set-up for each
//! ([`crate::setup`]), a catalogue of moves with the words to teach them
//! (`dj_assistant::technique`), and a practice lab ([`crate::practice`]).
//! What none of them holds is **the event**: the date and the place, who is
//! coming, what was promised, the moments that have to happen at a time, and
//! what to do when it rains. That is what this keeps — and it keeps the set by
//! *reference*, the playlist the plan panel saved, so there is one set and not
//! a copy that drifts from it.
//!
//! # Five steps, each saying what it still lacks
//!
//! [`Step`] is the preparation in the order a DJ would do it: the event, the
//! music, the running order, just in case, and the extras and after. Each one
//! says what is **missing** rather than carrying a tick box: a box a DJ ticked
//! at home says nothing about whether the rain plan was ever written, and the
//! night is where that is found out. Every change is saved as it is made
//! (the panel writes on each edit), so a preparation is never lost by closing
//! the window halfway.
//!
//! # Ideas are rules, and say where they come from
//!
//! [`ideas`] offers what a kind of night usually has — a wedding's first
//! dance, a beach's sunset, a club's handover, the genres §16's packs pair
//! with that night, the moves the pack teaches, a plan for each trouble. Each
//! is one press to take and is not offered again once taken. They are
//! written rules, not a model's guess about this particular night, and the
//! panel labels them as suggestions: a DJ who knows the couple knows better.
//!
//! # The path
//!
//! [`path`] is the owner's workflow for one event: **learn** the moves the
//! night needs, **practise** them, build the **set** on its topic,
//! **prepare** the event, and play it **live**. Each stop is done or not by
//! what is stored — moves chosen, moves rehearsed, a set linked, nothing
//! missing — never by the DJ saying so.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::setting::Setting;

/// Under a roof, in the open, or some of each. Decides whether rain is a
/// trouble worth a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sky {
    #[default]
    Indoors,
    Outdoors,
    Both,
}

impl Sky {
    pub const ALL: [Sky; 3] = [Sky::Indoors, Sky::Outdoors, Sky::Both];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Sky::Indoors => "indoors",
            Sky::Outdoors => "outdoors",
            Sky::Both => "both",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Sky::Indoors => "Under a roof",
            Sky::Outdoors => "Open air",
            Sky::Both => "Some of each",
        }
    }
}

/// How closely the night keeps to what was agreed: the owner's
/// *flexibility*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Leeway {
    /// The host has a list and means it.
    Strict,
    /// A plan, and room to move inside it.
    #[default]
    Some,
    /// Read the room; the plan is a starting point.
    Open,
}

impl Leeway {
    pub const ALL: [Leeway; 3] = [Leeway::Strict, Leeway::Some, Leeway::Open];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Leeway::Strict => "strict",
            Leeway::Some => "some",
            Leeway::Open => "open",
        }
    }

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Leeway::Strict => "As agreed",
            Leeway::Some => "Room to move",
            Leeway::Open => "Read the room",
        }
    }
}

/// What can go wrong on a night, each with a plan written before it does.
///
/// A fixed list for the reason §81's settings are fixed: a plan filed under
/// "rain" and another under "Rain!" are one plan a DJ cannot find at 23:40.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trouble {
    EmptyFloor,
    Rain,
    Power,
    Microphone,
    Late,
    Longer,
    Request,
    Gear,
}

impl Trouble {
    pub const ALL: [Trouble; 8] = [
        Trouble::EmptyFloor,
        Trouble::Rain,
        Trouble::Power,
        Trouble::Microphone,
        Trouble::Late,
        Trouble::Longer,
        Trouble::Request,
        Trouble::Gear,
    ];

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Trouble::EmptyFloor => "empty-floor",
            Trouble::Rain => "rain",
            Trouble::Power => "power",
            Trouble::Microphone => "microphone",
            Trouble::Late => "late",
            Trouble::Longer => "longer",
            Trouble::Request => "request",
            Trouble::Gear => "gear",
        }
    }

    /// The trouble, as it would be said in the booth.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Trouble::EmptyFloor => "The floor empties",
            Trouble::Rain => "It rains",
            Trouble::Power => "The power goes",
            Trouble::Microphone => "The microphone fails",
            Trouble::Late => "It runs late",
            Trouble::Longer => "They ask for more",
            Trouble::Request => "A request you will not play",
            Trouble::Gear => "Your gear fails",
        }
    }

    /// What a DJ usually has ready for it: offered as an idea, never written
    /// in on the DJ's behalf.
    #[must_use]
    pub const fn usual(self) -> &'static str {
        match self {
            Trouble::EmptyFloor => {
                "Go back to what filled it: the last record that worked, then one everybody knows."
            }
            Trouble::Rain => {
                "Where the decks go if it rains, a cover for them, and a warmer hour while people find shelter."
            }
            Trouble::Power => {
                "The first record back on, cued on deck 1; a phone with the set offline and a cable into the house system."
            }
            Trouble::Microphone => {
                "A spare cable, and every announcement written down so someone else can read it."
            }
            Trouble::Late => {
                "Which moments stay and which records go, decided now rather than at the time."
            }
            Trouble::Longer => "Half an hour more you would be glad to play, already in a crate.",
            Trouble::Request => "A kind answer, and the nearest record you would play instead.",
            Trouble::Gear => {
                "The laptop alone: the keyboard map, the set as a playlist, and the headphones on the laptop's own output."
            }
        }
    }

    /// Whether this trouble can happen at this event. Rain only reaches a
    /// night that is at least partly outside.
    #[must_use]
    pub fn applies(self, gig: &Gig) -> bool {
        self != Trouble::Rain || gig.sky != Sky::Indoors
    }
}

/// Something that happens at a time: an entrance, a first dance, the sunset,
/// the handover.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Moment {
    /// `HH:MM`, or empty until the host has said.
    #[serde(default)]
    pub at: String,
    pub what: String,
    /// The record for it, as the host named it: a title, not a library id,
    /// because a host names a song before anyone has found the file.
    #[serde(default)]
    pub record: String,
}

/// A plan for one trouble.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fallback {
    pub trouble: Trouble,
    pub plan: String,
}

/// The set built for this event: the playlist the plan panel saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linked {
    pub playlist: i64,
    pub name: String,
}

/// One event, and everything prepared for it.
///
/// Every field defaults, so a file written by an earlier djmanzo opens on a
/// later one and a field added later starts empty rather than refusing it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gig {
    /// Stable, and the file's name: set when the event is made and never
    /// changed by renaming it.
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`, or empty.
    pub date: String,
    /// When the DJ starts, `HH:MM`, or empty.
    pub starts: String,
    /// How long the DJ plays.
    pub minutes: u32,
    /// §81's kind of night.
    pub setting: Option<Setting>,
    /// Where it is.
    pub place: String,
    pub sky: Sky,
    /// Who is coming, and what they came for.
    pub crowd: String,
    /// What the set is about: the owner's *topic*.
    pub topic: String,
    /// Genre families, by `dj_core::genre` name.
    pub genres: Vec<String>,
    /// Genre families not to play.
    pub avoid: Vec<String>,
    /// Records the host asked for: the owner's *wishes* and *songs*.
    pub wishes: Vec<String>,
    /// Records not to play, whatever is asked.
    pub never: Vec<String>,
    pub leeway: Leeway,
    /// The moments with a time: the owner's *specials*.
    pub moments: Vec<Moment>,
    /// The moves this night needs, by `dj_assistant::technique` name: the
    /// owner's *transitions*.
    pub techniques: Vec<String>,
    /// Those of them the DJ has rehearsed for it.
    pub rehearsed: Vec<String>,
    pub setlist: Option<Linked>,
    /// The owner's *fallbacks*, *alternatives* and *emergencies*.
    pub fallbacks: Vec<Fallback>,
    /// The owner's *extras*.
    pub extras: String,
    /// The owner's *after bonus*: what happens once the set is over.
    pub after: String,
    /// Who to call: the host, the venue, the sound engineer.
    pub contacts: String,
    pub notes: String,
    /// When it was last saved, in seconds.
    pub updated: i64,
}

impl Gig {
    /// The plan for a trouble, when one is written.
    #[must_use]
    pub fn plan_for(&self, trouble: Trouble) -> Option<&str> {
        self.fallbacks
            .iter()
            .find(|f| f.trouble == trouble)
            .map(|f| f.plan.trim())
            .filter(|plan| !plan.is_empty())
    }
}

/// The preparation, in the order a DJ does it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    Event,
    Music,
    Order,
    Trouble,
    Extras,
}

impl Step {
    pub const ALL: [Step; 5] = [
        Step::Event,
        Step::Music,
        Step::Order,
        Step::Trouble,
        Step::Extras,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Step::Event => "The event",
            Step::Music => "The music",
            Step::Order => "Running order",
            Step::Trouble => "Just in case",
            Step::Extras => "Extras and after",
        }
    }

    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Step::Event => "When and where, how long, what kind of night, and who is coming.",
            Step::Music => {
                "What the set is about, the genres, what was asked for and what not to play."
            }
            Step::Order => "The moments at a time, the moves the night needs, and the set itself.",
            Step::Trouble => "What to do when it goes wrong, written before it does.",
            Step::Extras => "The specials, the after bonus, who to call and anything else.",
        }
    }
}

/// One step, and what it still lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepState {
    pub step: Step,
    pub title: &'static str,
    pub about: &'static str,
    /// What is not there yet, one phrase each. Empty means ready.
    pub missing: Vec<String>,
    /// Nothing here is needed for the night to go ahead.
    pub optional: bool,
}

/// Every step and what it lacks.
#[must_use]
pub fn steps(gig: &Gig) -> Vec<StepState> {
    Step::ALL
        .iter()
        .map(|&step| {
            let mut missing = Vec::new();
            let mut lacks = |empty: bool, what: &str| {
                if empty {
                    missing.push(what.to_owned());
                }
            };
            match step {
                Step::Event => {
                    lacks(gig.title.trim().is_empty(), "a name");
                    lacks(gig.date.is_empty(), "the date");
                    lacks(gig.starts.is_empty(), "when you start");
                    lacks(gig.minutes == 0, "how long you play");
                    lacks(gig.setting.is_none(), "what kind of night it is");
                    lacks(gig.place.trim().is_empty(), "where it is");
                }
                Step::Music => {
                    lacks(
                        gig.topic.trim().is_empty() && gig.genres.is_empty(),
                        "what the music is: a topic or a genre",
                    );
                }
                Step::Order => {
                    lacks(gig.setlist.is_none(), "a set built for it");
                    lacks(
                        gig.setting == Some(Setting::Wedding) && gig.moments.is_empty(),
                        "the moments (a wedding has them)",
                    );
                }
                Step::Trouble => {
                    for trouble in Trouble::ALL {
                        if trouble.applies(gig) && gig.plan_for(trouble).is_none() {
                            missing.push(format!("what to do if {}", lower_first(trouble.title())));
                        }
                    }
                }
                Step::Extras => {}
            }
            StepState {
                step,
                title: step.title(),
                about: step.about(),
                missing,
                optional: step == Step::Extras,
            }
        })
        .collect()
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// What an idea adds, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Adds {
    Genre { name: String },
    Technique { name: String },
    Moment { what: String },
    Fallback { trouble: Trouble, plan: String },
    Extra { line: String },
    After { line: String },
}

/// One suggestion, taken with one press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Idea {
    pub step: Step,
    /// What it says on the button's label.
    pub text: String,
    /// Why it is offered: the rule it comes from.
    pub because: String,
    pub adds: Adds,
}

/// The genre families §16's packs pair with a kind of night.
fn genres_for(setting: Setting) -> Vec<&'static str> {
    match setting {
        // A beach is not in any pack's list; these are the warm, unhurried
        // families the genre table has.
        Setting::Beach => vec!["afro house", "disco", "amapiano"],
        // A wedding plays what the room knows; the table's name for that.
        Setting::Wedding => vec!["pop", "disco"],
        _ => {
            let mut out = Vec::new();
            for pack in dj_assistant::pack::ALL
                .iter()
                .filter(|pack| pack.setting == Some(setting.slug()))
            {
                for family in pack.families {
                    if !out.contains(family) {
                        out.push(*family);
                    }
                }
            }
            out
        }
    }
}

/// The moves a kind of night asks for: what its pack teaches, easiest first,
/// three at most — a list of ten is a syllabus, not a preparation.
fn moves_for(setting: Setting) -> Vec<&'static str> {
    let hand = |names: &[&'static str]| names.to_vec();
    match setting {
        // Nights no pack pairs with, by what they ask of the DJ: a wedding
        // needs clean, safe exits between records that do not belong
        // together; a beach, long unhurried blends.
        Setting::Wedding => hand(&["echo out", "cut", "phrase mix"]),
        Setting::Beach => hand(&["long blend", "filter fade", "EQ ride"]),
        _ => {
            let Some(pack) = dj_assistant::pack::ALL
                .iter()
                .find(|pack| pack.setting == Some(setting.slug()))
            else {
                return Vec::new();
            };
            let mut moves: Vec<_> = dj_assistant::technique::catalogue()
                .iter()
                .filter(|t| pack.teaches_move(t))
                .collect();
            moves.sort_by_key(|t| t.difficulty);
            moves.into_iter().take(3).map(|t| t.name).collect()
        }
    }
}

/// The moments a kind of night usually has.
fn moments_for(setting: Setting) -> &'static [&'static str] {
    match setting {
        Setting::Wedding => &[
            "The couple's entrance",
            "First dance",
            "Cutting the cake",
            "Parents' dance",
            "Bouquet toss",
            "Last dance",
        ],
        Setting::Beach => &["Sunset: the record for the sun touching the water"],
        Setting::Club => &["The handover: a clean last record for whoever is next"],
        Setting::Latin => &["A slow block for the couples"],
        Setting::OpenFormat => &["The singalong, held for when the room needs it"],
        Setting::Practice => &[],
    }
}

/// How long a set runs before a break is worth planning.
pub const BREAK_AFTER_MINUTES: u32 = 180;

/// Ideas for this event, per step, leaving out whatever is already taken.
#[must_use]
pub fn ideas(gig: &Gig) -> Vec<Idea> {
    let mut out = Vec::new();
    let has = |list: &[String], name: &str| list.iter().any(|x| x.eq_ignore_ascii_case(name));

    if let Some(setting) = gig.setting {
        let night = setting.title();
        for name in genres_for(setting) {
            if !has(&gig.genres, name) && !has(&gig.avoid, name) {
                out.push(Idea {
                    step: Step::Music,
                    text: name.to_owned(),
                    because: format!("What a {night} night is usually played with."),
                    adds: Adds::Genre {
                        name: name.to_owned(),
                    },
                });
            }
        }
        for name in moves_for(setting) {
            if !has(&gig.techniques, name) {
                out.push(Idea {
                    step: Step::Order,
                    text: name.to_owned(),
                    because: format!("A move a {night} night asks for."),
                    adds: Adds::Technique {
                        name: name.to_owned(),
                    },
                });
            }
        }
        for what in moments_for(setting) {
            if !gig
                .moments
                .iter()
                .any(|m| m.what.eq_ignore_ascii_case(what))
            {
                out.push(Idea {
                    step: Step::Order,
                    text: (*what).to_owned(),
                    because: format!("A {night} night usually has it."),
                    adds: Adds::Moment {
                        what: (*what).to_owned(),
                    },
                });
            }
        }
    }

    if gig.minutes >= BREAK_AFTER_MINUTES {
        let what = "Your break: a long record, or a mix prepared for ten minutes off the decks";
        if !gig.moments.iter().any(|m| m.what == what) {
            out.push(Idea {
                step: Step::Order,
                text: what.to_owned(),
                because: format!(
                    "{} hours is a long time to stand at the decks.",
                    gig.minutes / 60
                ),
                adds: Adds::Moment {
                    what: what.to_owned(),
                },
            });
        }
    }

    for trouble in Trouble::ALL {
        if trouble.applies(gig) && gig.plan_for(trouble).is_none() {
            out.push(Idea {
                step: Step::Trouble,
                text: trouble.usual().to_owned(),
                because: format!(
                    "What DJs keep ready for when {}.",
                    lower_first(trouble.title())
                ),
                adds: Adds::Fallback {
                    trouble,
                    plan: trouble.usual().to_owned(),
                },
            });
        }
    }

    let extras: &[&str] = match gig.setting {
        Some(Setting::Wedding) => &[
            "A request card on each table, collected before dinner ends",
            "The couple's own playlist through dinner",
        ],
        Some(Setting::Club) => &["A recording of the set for the promoter"],
        _ => &["A recording of the set"],
    };
    for line in extras {
        if !gig.extras.contains(line) {
            out.push(Idea {
                step: Step::Extras,
                text: (*line).to_owned(),
                because: "Something the night could have that it did not ask for.".to_owned(),
                adds: Adds::Extra {
                    line: (*line).to_owned(),
                },
            });
        }
    }
    for line in [
        "The set list to the host the next day",
        "The recording online, with the records named",
        "A thank-you with a photo from the night",
    ] {
        if !gig.after.contains(line) {
            out.push(Idea {
                step: Step::Extras,
                text: line.to_owned(),
                because: "After the set: the bonus that gets the next booking.".to_owned(),
                adds: Adds::After {
                    line: line.to_owned(),
                },
            });
        }
    }

    out
}

/// Take an idea: add what it adds to the event.
#[must_use]
pub fn take(mut gig: Gig, adds: &Adds) -> Gig {
    let push_new = |list: &mut Vec<String>, name: &str| {
        if !list.iter().any(|x| x.eq_ignore_ascii_case(name)) {
            list.push(name.to_owned());
        }
    };
    let add_line = |text: &mut String, line: &str| {
        if !text.contains(line) {
            if !text.trim().is_empty() {
                text.push('\n');
            }
            text.push_str(line);
        }
    };
    match adds {
        Adds::Genre { name } => push_new(&mut gig.genres, name),
        Adds::Technique { name } => push_new(&mut gig.techniques, name),
        Adds::Moment { what } => {
            if !gig
                .moments
                .iter()
                .any(|m| m.what.eq_ignore_ascii_case(what))
            {
                gig.moments.push(Moment {
                    what: what.clone(),
                    ..Moment::default()
                });
            }
        }
        Adds::Fallback { trouble, plan } => {
            if let Some(slot) = gig.fallbacks.iter_mut().find(|f| f.trouble == *trouble) {
                if slot.plan.trim().is_empty() {
                    slot.plan.clone_from(plan);
                }
            } else {
                gig.fallbacks.push(Fallback {
                    trouble: *trouble,
                    plan: plan.clone(),
                });
            }
        }
        Adds::Extra { line } => add_line(&mut gig.extras, line),
        Adds::After { line } => add_line(&mut gig.after, line),
    }
    gig
}

/// Minutes past midnight for `HH:MM`.
#[must_use]
pub fn clock(text: &str) -> Option<u32> {
    let (hours, minutes) = text.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u32 = hours.parse().ok()?;
    let minutes: u32 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let [year, month, day] = parts[..] else {
        return false;
    };
    let number = |part: &str, len: usize, range: std::ops::RangeInclusive<u32>| {
        part.len() == len && part.parse::<u32>().is_ok_and(|n| range.contains(&n))
    };
    number(year, 4, 2000..=2999) && number(month, 2, 1..=12) && number(day, 2, 1..=31)
}

/// One mark on the night's running order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mark {
    pub at: String,
    /// Minutes after the DJ starts.
    pub after: u32,
    pub what: String,
    /// Falls before the DJ starts or after they finish: a moment the host
    /// put at a time the DJ is not playing.
    pub outside: bool,
}

/// The running order: the start, every moment with a time, the finish, in
/// the order they happen. A night that crosses midnight is read as one
/// night — a moment at 00:30 after a 21:00 start is three and a half hours
/// in, not twenty and a half hours before.
#[must_use]
pub fn timeline(gig: &Gig) -> Vec<Mark> {
    let Some(start) = clock(&gig.starts) else {
        return Vec::new();
    };
    let after = |at: u32| (at + 24 * 60 - start) % (24 * 60);
    let mut marks: Vec<Mark> = gig
        .moments
        .iter()
        .filter_map(|moment| {
            let at = clock(&moment.at)?;
            let after = after(at);
            Some(Mark {
                at: moment.at.clone(),
                after,
                what: moment.what.clone(),
                outside: after > gig.minutes,
            })
        })
        .collect();
    marks.push(Mark {
        at: gig.starts.clone(),
        after: 0,
        what: "You start".to_owned(),
        outside: false,
    });
    if gig.minutes > 0 {
        let end = (start + gig.minutes) % (24 * 60);
        marks.push(Mark {
            at: format!("{:02}:{:02}", end / 60, end % 60),
            after: gig.minutes,
            what: "You finish".to_owned(),
            outside: false,
        });
    }
    // Stable, so a moment at the very start stays after "You start".
    marks.sort_by_key(|mark| (mark.outside, mark.after));
    marks
}

/// Why an event was not saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Unnamed,
    Id,
    Date(String),
    Time(String),
    Length(u32),
    Genre(String),
    Technique(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Unnamed => write!(f, "an event needs a name to be found again"),
            Refused::Id => write!(f, "that is not an event djmanzo made"),
            Refused::Date(date) => write!(f, "{date:?} is not a date (year-month-day)"),
            Refused::Time(time) => write!(f, "{time:?} is not a time (hours:minutes)"),
            Refused::Length(minutes) => {
                write!(f, "{minutes} minutes is longer than a day")
            }
            Refused::Genre(name) => write!(f, "djmanzo does not know the genre {name:?}"),
            Refused::Technique(name) => write!(f, "djmanzo does not know the move {name:?}"),
        }
    }
}

/// Whether an id is one [`new_id`] could have made: lower-case letters,
/// digits and hyphens. Anything else — a slash, two dots — is refused before
/// it reaches the file system.
#[must_use]
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Hold an event to what djmanzo can keep: a name, dates and times it can
/// read, genres and moves it knows. Genre and move names are written back
/// the way djmanzo spells them, so "House" and "house" are one genre.
///
/// # Errors
/// See [`Refused`].
pub fn check(mut gig: Gig) -> Result<Gig, Refused> {
    if !is_id(&gig.id) {
        return Err(Refused::Id);
    }
    if gig.title.trim().is_empty() {
        return Err(Refused::Unnamed);
    }
    if !gig.date.is_empty() && !is_date(&gig.date) {
        return Err(Refused::Date(gig.date));
    }
    for time in std::iter::once(&gig.starts).chain(gig.moments.iter().map(|m| &m.at)) {
        if !time.is_empty() && clock(time).is_none() {
            return Err(Refused::Time(time.clone()));
        }
    }
    if gig.minutes > 24 * 60 {
        return Err(Refused::Length(gig.minutes));
    }
    for list in [&mut gig.genres, &mut gig.avoid] {
        for name in list.iter_mut() {
            let family =
                dj_core::genre::family_for(name).ok_or_else(|| Refused::Genre(name.clone()))?;
            family.name.clone_into(name);
        }
        dedup(list);
    }
    for list in [&mut gig.techniques, &mut gig.rehearsed] {
        for name in list.iter_mut() {
            let technique = dj_assistant::technique::by_name(name)
                .ok_or_else(|| Refused::Technique(name.clone()))?;
            technique.name.clone_into(name);
        }
        dedup(list);
    }
    // A move rehearsed for this night that is no longer one it needs is not
    // a rehearsal of anything; keeping it would mark Practise done for a
    // list the DJ has since changed.
    let needed = gig.techniques.clone();
    gig.rehearsed.retain(|name| needed.contains(name));
    Ok(gig)
}

fn dedup(list: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    list.retain(|name| seen.insert(name.clone()));
}

/// A fresh id for an event called `title` on `date`: its words, its date,
/// and a number when another event already has both.
#[must_use]
pub fn new_id(title: &str, date: &str, taken: &[String]) -> String {
    let mut base = crate::activity::slug_of(title)
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .take(60)
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    if base.is_empty() {
        base = "event".to_owned();
    }
    if is_date(date) {
        base = format!("{base}-{date}");
    }
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// A stop on the path to the night.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stop {
    Learn,
    Practise,
    Set,
    Prepare,
    Live,
}

/// One stop, whether it is done, and what it says about this event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathStop {
    pub stop: Stop,
    pub title: &'static str,
    pub about: String,
    pub done: bool,
}

fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The owner's workflow for this event: learn → practise → set → prepare →
/// live. Each stop is done by what is stored, never by a tick.
#[must_use]
pub fn path(gig: &Gig) -> Vec<PathStop> {
    let unrehearsed: Vec<String> = gig
        .techniques
        .iter()
        .filter(|name| !gig.rehearsed.contains(name))
        .cloned()
        .collect();
    let lacking: usize = steps(gig)
        .iter()
        .filter(|s| !s.optional)
        .map(|s| s.missing.len())
        .sum();
    let prepared = lacking == 0;
    vec![
        PathStop {
            stop: Stop::Learn,
            title: "Learn",
            about: if gig.techniques.is_empty() {
                "Choose the moves this night needs.".to_owned()
            } else {
                format!(
                    "What {} are, and when to reach for them.",
                    listed(&gig.techniques)
                )
            },
            done: !gig.techniques.is_empty(),
        },
        PathStop {
            stop: Stop::Practise,
            title: "Practise",
            about: if gig.techniques.is_empty() {
                "Nothing to rehearse until the moves are chosen.".to_owned()
            } else if unrehearsed.is_empty() {
                "Every move rehearsed.".to_owned()
            } else {
                format!("Rehearse {} in the practice lab.", listed(&unrehearsed))
            },
            done: !gig.techniques.is_empty() && unrehearsed.is_empty(),
        },
        PathStop {
            stop: Stop::Set,
            title: "Set",
            about: match &gig.setlist {
                Some(linked) => format!("\u{201c}{}\u{201d} is the set.", linked.name),
                None if gig.topic.trim().is_empty() => "Build the set for this night.".to_owned(),
                None => format!("Build a set on \u{201c}{}\u{201d}.", gig.topic.trim()),
            },
            done: gig.setlist.is_some(),
        },
        PathStop {
            stop: Stop::Prepare,
            title: "Prepare",
            about: if prepared {
                "Everything the night needs is written down.".to_owned()
            } else {
                format!(
                    "{lacking} thing{} still to prepare.",
                    if lacking == 1 { "" } else { "s" }
                )
            },
            done: prepared,
        },
        PathStop {
            stop: Stop::Live,
            title: "Live",
            about: if prepared {
                "Ready to play.".to_owned()
            } else {
                "Play it, once it is prepared.".to_owned()
            },
            done: false,
        },
    ]
}

/// A move the night needs, with the words to learn it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Move {
    pub name: String,
    pub what: &'static str,
    pub when: &'static str,
    pub metaphor: &'static str,
    pub rehearsed: bool,
}

/// A trouble as the panel lists it: only the ones this night can have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TroubleRow {
    pub trouble: Trouble,
    pub title: &'static str,
    pub usual: &'static str,
}

/// Everything the panel draws for one event, in one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub gig: Gig,
    pub steps: Vec<StepState>,
    pub ideas: Vec<Idea>,
    pub path: Vec<PathStop>,
    pub timeline: Vec<Mark>,
    pub moves: Vec<Move>,
    pub troubles: Vec<TroubleRow>,
}

/// The view of an event.
#[must_use]
pub fn view(gig: Gig) -> View {
    let moves = gig
        .techniques
        .iter()
        .filter_map(|name| {
            let t = dj_assistant::technique::by_name(name)?;
            Some(Move {
                name: t.name.to_owned(),
                what: t.what,
                when: t.when,
                metaphor: t.metaphor,
                rehearsed: gig.rehearsed.iter().any(|r| r == t.name),
            })
        })
        .collect();
    let troubles = Trouble::ALL
        .into_iter()
        .filter(|t| t.applies(&gig))
        .map(|trouble| TroubleRow {
            trouble,
            title: trouble.title(),
            usual: trouble.usual(),
        })
        .collect();
    View {
        steps: steps(&gig),
        ideas: ideas(&gig),
        path: path(&gig),
        timeline: timeline(&gig),
        moves,
        troubles,
        gig,
    }
}

/// One choice of a fixed list, as a picker offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub slug: &'static str,
    pub title: &'static str,
}

/// A move the DJ may add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveChoice {
    pub name: &'static str,
    pub what: &'static str,
}

/// The fixed lists the panel offers, read off the tables that own them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Options {
    /// §81's kinds of night, which decide most of the ideas.
    pub nights: Vec<Choice>,
    pub skies: Vec<Choice>,
    pub leeways: Vec<Choice>,
    pub moves: Vec<MoveChoice>,
    /// The genre families djmanzo knows, by the name [`check`] keeps: a
    /// genre typed that is not one of these is refused, so the panel offers
    /// these rather than a free field that fails on save.
    pub genres: Vec<&'static str>,
}

#[must_use]
pub fn options() -> Options {
    Options {
        nights: Setting::ALL
            .iter()
            .map(|n| Choice {
                slug: n.slug(),
                title: n.title(),
            })
            .collect(),
        skies: Sky::ALL
            .iter()
            .map(|s| Choice {
                slug: s.slug(),
                title: s.title(),
            })
            .collect(),
        leeways: Leeway::ALL
            .iter()
            .map(|l| Choice {
                slug: l.slug(),
                title: l.title(),
            })
            .collect(),
        moves: dj_assistant::technique::catalogue()
            .iter()
            .map(|t| MoveChoice {
                name: t.name,
                what: t.what,
            })
            .collect(),
        genres: dj_core::genre::families().iter().map(|f| f.name).collect(),
    }
}

/// An event as the list shows it: enough to find it, and how much of it is
/// still to prepare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub date: String,
    pub starts: String,
    pub place: String,
    pub setting: Option<Setting>,
    /// Things still missing from the steps the night needs; the optional
    /// step does not count.
    pub lacking: usize,
}

/// The list's line for an event.
#[must_use]
pub fn summary(gig: &Gig) -> Summary {
    Summary {
        id: gig.id.clone(),
        title: gig.title.clone(),
        date: gig.date.clone(),
        starts: gig.starts.clone(),
        place: gig.place.clone(),
        setting: gig.setting,
        lacking: steps(gig)
            .iter()
            .filter(|s| !s.optional)
            .map(|s| s.missing.len())
            .sum(),
    }
}

// -- keeping them ------------------------------------------------------------

/// `events/` beside the settings: one file per event, readable and easy to
/// copy to another machine or to a friend who is covering the night.
#[must_use]
pub fn folder(config: &Path) -> PathBuf {
    config.join("events")
}

/// Every event kept in `dir`, the soonest first and the undated last. A file
/// that does not read is left out rather than failing the list.
#[must_use]
pub fn list(dir: &Path) -> Vec<Gig> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut gigs: Vec<Gig> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str::<Gig>(&text).ok())
        .filter(|gig| is_id(&gig.id))
        .collect();
    gigs.sort_by(|a, b| {
        (a.date.is_empty(), &a.date, &a.starts, &a.id).cmp(&(
            b.date.is_empty(),
            &b.date,
            &b.starts,
            &b.id,
        ))
    });
    gigs
}

/// One event.
#[must_use]
pub fn load(dir: &Path, id: &str) -> Option<Gig> {
    if !is_id(id) {
        return None;
    }
    let text = std::fs::read_to_string(dir.join(format!("{id}.json"))).ok()?;
    serde_json::from_str(&text).ok()
}

/// Keep an event, after [`check`].
///
/// # Errors
/// The refusal, or the file system's own sentence.
pub fn save(dir: &Path, gig: Gig, now: i64) -> Result<Gig, String> {
    let mut gig = check(gig).map_err(|refused| refused.to_string())?;
    gig.updated = now;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = serde_json::to_string_pretty(&gig).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", gig.id));
    // Written beside and moved into place, so a crash mid-write leaves the
    // last good preparation rather than half of one.
    let partial = dir.join(format!(".{}.json.partial", gig.id));
    std::fs::write(&partial, text).map_err(|e| format!("{}: {e}", partial.display()))?;
    std::fs::rename(&partial, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(gig)
}

/// Forget an event.
///
/// # Errors
/// An id djmanzo did not make, or the file system's own sentence.
pub fn forget(dir: &Path, id: &str) -> Result<(), String> {
    if !is_id(id) {
        return Err(Refused::Id.to_string());
    }
    let path = dir.join(format!("{id}.json"));
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wedding() -> Gig {
        Gig {
            id: "anna-and-ben-2026-10-03".to_owned(),
            title: "Anna and Ben".to_owned(),
            date: "2026-10-03".to_owned(),
            starts: "21:00".to_owned(),
            minutes: 240,
            setting: Some(Setting::Wedding),
            place: "The old mill".to_owned(),
            sky: Sky::Both,
            ..Gig::default()
        }
    }

    fn missing(gig: &Gig, step: Step) -> Vec<String> {
        steps(gig)
            .into_iter()
            .find(|s| s.step == step)
            .unwrap()
            .missing
    }

    /// **Each step says what it lacks**, and rain is only a trouble for a
    /// night that is at least partly outside.
    #[test]
    fn each_step_says_what_it_still_lacks() {
        let empty = Gig::default();
        assert_eq!(
            missing(&empty, Step::Event),
            [
                "a name",
                "the date",
                "when you start",
                "how long you play",
                "what kind of night it is",
                "where it is"
            ]
        );
        let gig = wedding();
        assert!(missing(&gig, Step::Event).is_empty());
        assert_eq!(
            missing(&gig, Step::Music),
            ["what the music is: a topic or a genre"]
        );
        assert_eq!(
            missing(&gig, Step::Order),
            ["a set built for it", "the moments (a wedding has them)"]
        );
        let trouble = missing(&gig, Step::Trouble);
        assert!(trouble.contains(&"what to do if it rains".to_owned()));
        assert_eq!(trouble.len(), Trouble::ALL.len());

        let indoors = Gig {
            sky: Sky::Indoors,
            ..wedding()
        };
        assert!(!missing(&indoors, Step::Trouble).contains(&"what to do if it rains".to_owned()));
        assert_eq!(
            missing(&indoors, Step::Trouble).len(),
            Trouble::ALL.len() - 1
        );

        let club = Gig {
            setting: Some(Setting::Club),
            ..wedding()
        };
        assert_eq!(missing(&club, Step::Order), ["a set built for it"]);

        let planned = Gig {
            fallbacks: vec![Fallback {
                trouble: Trouble::Power,
                plan: "Deck 1 cued".to_owned(),
            }],
            ..wedding()
        };
        assert!(
            !missing(&planned, Step::Trouble).contains(&"what to do if the power goes".to_owned())
        );
        // A plan that is only spaces is not a plan.
        let blank = Gig {
            fallbacks: vec![Fallback {
                trouble: Trouble::Power,
                plan: "   ".to_owned(),
            }],
            ..wedding()
        };
        assert!(
            missing(&blank, Step::Trouble).contains(&"what to do if the power goes".to_owned())
        );
    }

    /// **Ideas fit the night, and one taken is not offered again.** A
    /// wedding is offered its first dance, a beach its sunset; a genre the
    /// DJ has said not to play is never suggested.
    #[test]
    fn ideas_fit_the_night_and_are_not_offered_twice() {
        let gig = wedding();
        let offered = ideas(&gig);
        let first_dance = offered
            .iter()
            .find(|idea| idea.text == "First dance")
            .expect("a wedding is offered its first dance");
        assert_eq!(first_dance.step, Step::Order);
        assert!(offered.iter().any(|idea| matches!(
            &idea.adds,
            Adds::Fallback {
                trouble: Trouble::Rain,
                ..
            }
        )));
        assert!(
            offered
                .iter()
                .any(|idea| idea.text.starts_with("Your break")),
            "four hours is offered a break"
        );

        let taken = take(gig.clone(), &first_dance.adds);
        assert_eq!(taken.moments.len(), 1);
        assert!(ideas(&taken).iter().all(|idea| idea.text != "First dance"));
        // Taking it twice does not add it twice.
        assert_eq!(take(taken, &first_dance.adds).moments.len(), 1);

        let beach = Gig {
            setting: Some(Setting::Beach),
            minutes: 120,
            ..wedding()
        };
        let beach_ideas = ideas(&beach);
        assert!(
            beach_ideas
                .iter()
                .any(|idea| idea.text.starts_with("Sunset"))
        );
        assert!(beach_ideas.iter().all(|idea| idea.text != "First dance"));
        assert!(
            beach_ideas
                .iter()
                .all(|idea| !idea.text.starts_with("Your break"))
        );

        let latin = Gig {
            setting: Some(Setting::Latin),
            avoid: vec!["reggaeton".to_owned()],
            ..wedding()
        };
        let latin_genres: Vec<_> = ideas(&latin)
            .into_iter()
            .filter_map(|idea| match idea.adds {
                Adds::Genre { name } => Some(name),
                _ => None,
            })
            .collect();
        assert!(latin_genres.contains(&"bachata".to_owned()));
        assert!(!latin_genres.contains(&"reggaeton".to_owned()));
    }

    /// Every genre and move an idea can add is one [`check`] accepts, for
    /// every kind of night — an idea that could not be saved would be a
    /// button that breaks the preparation.
    #[test]
    fn every_idea_can_be_kept() {
        for setting in Setting::ALL {
            let gig = Gig {
                setting: Some(setting),
                ..wedding()
            };
            let mut taken = gig.clone();
            for idea in ideas(&gig) {
                taken = take(taken, &idea.adds);
            }
            let kept = check(taken.clone()).unwrap_or_else(|why| panic!("{setting}: {why}"));
            assert!(
                !kept.techniques.is_empty() || setting == Setting::Practice,
                "{setting} suggests no moves"
            );
            assert!(missing(&kept, Step::Trouble).is_empty());
            assert!(ideas(&kept).is_empty(), "{setting}: {:?}", ideas(&kept));
        }
    }

    /// **The running order reads a night across midnight as one night**,
    /// and a moment set when the DJ is not playing is marked.
    #[test]
    fn the_running_order_crosses_midnight() {
        let gig = Gig {
            moments: vec![
                Moment {
                    at: "00:30".to_owned(),
                    what: "Last dance".to_owned(),
                    ..Moment::default()
                },
                Moment {
                    at: "21:30".to_owned(),
                    what: "First dance".to_owned(),
                    ..Moment::default()
                },
                Moment {
                    at: "20:00".to_owned(),
                    what: "Dinner".to_owned(),
                    ..Moment::default()
                },
                Moment {
                    what: "Cake, time to be told".to_owned(),
                    ..Moment::default()
                },
            ],
            ..wedding()
        };
        let marks = timeline(&gig);
        let order: Vec<(&str, u32, bool)> = marks
            .iter()
            .map(|m| (m.what.as_str(), m.after, m.outside))
            .collect();
        assert_eq!(
            order,
            [
                ("You start", 0, false),
                ("First dance", 30, false),
                ("Last dance", 210, false),
                ("You finish", 240, false),
                ("Dinner", 23 * 60, true),
            ]
        );
        assert_eq!(marks[3].at, "01:00");
    }

    /// **What is kept is what djmanzo can read back**: names are spelled
    /// djmanzo's way, dates and times are real ones, and an id cannot reach
    /// outside the events folder.
    #[test]
    fn an_event_is_held_to_what_can_be_kept() {
        let spelled = check(Gig {
            genres: vec!["House".to_owned(), "house".to_owned(), "DNB".to_owned()],
            techniques: vec!["Echo Out".to_owned()],
            rehearsed: vec!["echo out".to_owned(), "cut".to_owned()],
            ..wedding()
        })
        .unwrap();
        assert_eq!(spelled.genres, ["house", "drum and bass"]);
        assert_eq!(spelled.techniques, ["echo out"]);
        assert_eq!(
            spelled.rehearsed,
            ["echo out"],
            "a move no longer needed is not rehearsed"
        );

        let refused = |gig: Gig| check(gig).unwrap_err();
        assert_eq!(
            refused(Gig {
                title: " ".to_owned(),
                ..wedding()
            }),
            Refused::Unnamed
        );
        assert_eq!(
            refused(Gig {
                date: "3.10.2026".to_owned(),
                ..wedding()
            }),
            Refused::Date("3.10.2026".to_owned())
        );
        assert_eq!(
            refused(Gig {
                starts: "25:00".to_owned(),
                ..wedding()
            }),
            Refused::Time("25:00".to_owned())
        );
        assert!(matches!(
            refused(Gig {
                genres: vec!["polka-step".to_owned()],
                ..wedding()
            }),
            Refused::Genre(_)
        ));
        for id in ["../settings", "a/b", "", "Anna"] {
            assert_eq!(
                refused(Gig {
                    id: id.to_owned(),
                    ..wedding()
                }),
                Refused::Id,
                "{id:?}"
            );
            assert!(load(Path::new("/"), id).is_none());
        }
    }

    /// **Saved as it is made, and read back the same**; the list is soonest
    /// first, undated last; forgetting one leaves the others.
    #[test]
    fn events_are_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("djmanzo-gigs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let later = save(&dir, wedding(), 100).unwrap();
        assert_eq!(later.updated, 100);
        let sooner = save(
            &dir,
            Gig {
                id: "warm-up".to_owned(),
                title: "Warm-up".to_owned(),
                date: "2026-09-30".to_owned(),
                ..Gig::default()
            },
            101,
        )
        .unwrap();
        save(
            &dir,
            Gig {
                id: "someday".to_owned(),
                title: "Someday".to_owned(),
                ..Gig::default()
            },
            102,
        )
        .unwrap();
        let ids: Vec<String> = list(&dir).into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["warm-up", "anna-and-ben-2026-10-03", "someday"]);
        assert_eq!(load(&dir, &sooner.id), Some(sooner));
        assert_eq!(load(&dir, &later.id), Some(later.clone()));
        // Nothing left half-written beside it.
        assert!(
            std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".partial"))
        );

        forget(&dir, "warm-up").unwrap();
        forget(&dir, "warm-up").unwrap();
        let ids: Vec<String> = list(&dir).into_iter().map(|g| g.id).collect();
        assert_eq!(ids, ["anna-and-ben-2026-10-03", "someday"]);

        let taken: Vec<String> = ids.clone();
        assert_eq!(
            new_id("Anna & Ben!", "2026-10-03", &taken),
            "anna-ben-2026-10-03"
        );
        assert_eq!(
            new_id("Anna and Ben", "2026-10-03", &taken),
            "anna-and-ben-2026-10-03-2"
        );
        assert_eq!(new_id("¡¿", "", &taken), "event");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The path is done by what is stored**: moves chosen, moves
    /// rehearsed, a set linked, nothing missing.
    #[test]
    fn the_path_follows_the_preparation() {
        let done = |gig: &Gig| -> Vec<bool> { path(gig).iter().map(|s| s.done).collect() };
        let gig = wedding();
        assert_eq!(done(&gig), [false, false, false, false, false]);

        let learning = Gig {
            techniques: vec!["echo out".to_owned(), "cut".to_owned()],
            rehearsed: vec!["cut".to_owned()],
            ..wedding()
        };
        assert_eq!(done(&learning), [true, false, false, false, false]);
        assert_eq!(
            path(&learning)[1].about,
            "Rehearse echo out in the practice lab."
        );

        let mut ready = Gig {
            rehearsed: learning.techniques.clone(),
            topic: "Motown to Madonna".to_owned(),
            setlist: Some(Linked {
                playlist: 7,
                name: "Anna and Ben".to_owned(),
            }),
            moments: vec![Moment {
                at: "21:30".to_owned(),
                what: "First dance".to_owned(),
                ..Moment::default()
            }],
            ..learning
        };
        for trouble in Trouble::ALL {
            ready = take(
                ready,
                &Adds::Fallback {
                    trouble,
                    plan: trouble.usual().to_owned(),
                },
            );
        }
        assert_eq!(done(&ready), [true, true, true, true, false]);
        assert_eq!(path(&ready)[4].about, "Ready to play.");
    }

    /// The list line counts what the night still needs and not the optional
    /// extras, so an event whose extras are empty can still read "ready".
    #[test]
    fn a_summary_counts_what_the_night_still_needs() {
        let fresh = Gig {
            id: "summer-party".to_owned(),
            title: "Summer party".to_owned(),
            ..Gig::default()
        };
        let lacking: usize = steps(&fresh)
            .iter()
            .filter(|s| !s.optional)
            .map(|s| s.missing.len())
            .sum();
        assert!(lacking > 0);
        assert_eq!(summary(&fresh).lacking, lacking);
        assert_eq!(summary(&fresh).title, "Summer party");
        // The optional step never counts, however empty.
        assert!(
            steps(&fresh)
                .iter()
                .filter(|s| s.optional)
                .all(|s| s.missing.is_empty())
        );
    }

    /// The pickers offer exactly what [`check`] keeps: every night §81 lists,
    /// and genre names that come back unchanged -- so nothing chosen from a
    /// list can be refused when it is saved.
    #[test]
    fn what_the_panel_offers_is_what_an_event_may_hold() {
        let offered = options();
        assert_eq!(offered.nights.len(), Setting::ALL.len());
        let mut gig = Gig {
            id: "all-of-it".to_owned(),
            title: "All of it".to_owned(),
            genres: offered.genres.iter().map(|&g| g.to_owned()).collect(),
            techniques: offered.moves.iter().map(|m| m.name.to_owned()).collect(),
            ..Gig::default()
        };
        gig.avoid = gig.genres.clone();
        let kept = check(gig.clone()).expect("everything offered is kept");
        assert_eq!(kept.genres, gig.genres);
        assert_eq!(kept.techniques, gig.techniques);
    }
}

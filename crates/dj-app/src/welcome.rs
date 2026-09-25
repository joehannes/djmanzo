//! §118b: the welcome -- djmanzo getting to know the DJ, and setting itself up.
//!
//! > a new user is welcomed by a guide that introduces to the capabilities and
//! > metaphorical usage and navigation and features and workflow of the app.
//! > ... the welcome guide of above also gets to know the DJ ... he asks him
//! > for his favorite genres/music/artists/songs/bpm/styles/combinations/
//! > tricks/transitions/techniques ... and automatically creates activities
//! > and personalised preset-packages for him ...
//!
//! # It sets up what already exists
//!
//! djmanzo has, by now, a set-up for each of §81's kinds of night
//! ([`crate::setup`]: the waveform's layers, the pad pages, how much the
//! assistant does, whether the room may ask for records, §16's pack), §8's
//! levels ([`crate::level`]), activities of the DJ's own
//! ([`crate::activity::keep`]) and a logo in place of the wordmark
//! ([`crate::brand`]). What a new DJ lacked was anyone asking the questions
//! that decide them. So the welcome asks, [`plan`] says in sentences what
//! setting up will do -- before it is done, so nothing is changed behind the
//! DJ's back -- and the interface applies it through the same commands a DJ
//! would press.
//!
//! # What it does not do
//!
//! The owner asked for the assistant to search the internet and the DJ's
//! profiles for their details. That is not done here, and the welcome does not
//! pretend it is: scraping a profile page is unreliable, most of them forbid
//! it, and a DJ's bio written by a guess is worse than a blank one. What the
//! DJ tells it is kept, in `welcome.json`, and is theirs to change.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::setting::Setting;

/// The slowest and fastest tempo the welcome accepts as a DJ's range.
pub const BPM: (u16, u16) = (40, 220);

/// What the DJ told the welcome. Every field defaults, so a file written by an
/// earlier djmanzo opens on a later one.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Answers {
    /// The name they play under: in place of the wordmark, and on the press
    /// kit when there is one.
    pub name: String,
    /// The kinds of night they play, the one they play most first.
    pub nights: Vec<Setting>,
    /// Genre families, by `dj_core::genre` name.
    pub genres: Vec<String>,
    /// The tempo they are at home in; zero is unsaid.
    pub bpm_low: u16,
    pub bpm_high: u16,
    /// Artists and records they love, one a line, as they say them.
    pub favourites: Vec<String>,
    /// The moves they already reach for, by `dj_assistant::technique` name.
    pub moves: Vec<String>,
    /// The moves they want to learn.
    pub learn: Vec<String>,
    /// §8's level, by slug, or empty.
    pub level: String,
    /// The look they chose, by theme package id, or empty for the one their
    /// kind of night wears.
    pub theme: String,
    /// The welcome was finished, or put away: it does not open by itself
    /// again.
    pub done: bool,
}

/// Why answers were not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Genre(String),
    Move(String),
    Level(String),
    Tempo(u16, u16),
    Theme(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Genre(name) => write!(f, "djmanzo does not know the genre {name:?}"),
            Refused::Move(name) => write!(f, "djmanzo does not know the move {name:?}"),
            Refused::Level(slug) => write!(f, "{slug:?} is not one of §8's levels"),
            Refused::Theme(id) => write!(f, "{id:?} is not a theme djmanzo ships"),
            Refused::Tempo(low, high) => write!(
                f,
                "{low} to {high} BPM is not a tempo range (between {} and {}, slowest first)",
                BPM.0, BPM.1
            ),
        }
    }
}

fn dedup<T: PartialEq + Clone>(list: &mut Vec<T>) {
    let mut seen: Vec<T> = Vec::new();
    list.retain(|item| {
        if seen.contains(item) {
            false
        } else {
            seen.push(item.clone());
            true
        }
    });
}

/// Hold answers to what djmanzo can use: genres and moves it knows, spelled
/// its way; a level it has; a tempo range that is one.
///
/// # Errors
/// See [`Refused`].
pub fn check(mut answers: Answers) -> Result<Answers, Refused> {
    answers.name = answers.name.trim().to_owned();
    dedup(&mut answers.nights);
    for name in &mut answers.genres {
        let family =
            dj_core::genre::family_for(name).ok_or_else(|| Refused::Genre(name.clone()))?;
        family.name.clone_into(name);
    }
    dedup(&mut answers.genres);
    for list in [&mut answers.moves, &mut answers.learn] {
        for name in list.iter_mut() {
            let technique = dj_assistant::technique::by_name(name)
                .ok_or_else(|| Refused::Move(name.clone()))?;
            technique.name.clone_into(name);
        }
        dedup(list);
    }
    // A move they already play is not one to learn.
    let known = answers.moves.clone();
    answers.learn.retain(|name| !known.contains(name));
    answers.favourites = answers
        .favourites
        .iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    let (low, high) = (answers.bpm_low, answers.bpm_high);
    if (low, high) != (0, 0) && !(BPM.0 <= low && low <= high && high <= BPM.1) {
        return Err(Refused::Tempo(low, high));
    }
    if !answers.level.is_empty() && crate::level::Level::parse(&answers.level).is_none() {
        return Err(Refused::Level(answers.level));
    }
    if !answers.theme.is_empty() && !crate::theme::ships(&answers.theme) {
        return Err(Refused::Theme(answers.theme));
    }
    Ok(answers)
}

/// An activity the welcome will keep as the DJ's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedActivity {
    /// The name it gets on the strip.
    pub title: String,
    /// The arrangement it opens, by `cockpit::workspaces()` name.
    pub workspace: &'static str,
    pub night: Setting,
}

/// What setting up will do, said before it is done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// §54's set-up for the night they play most.
    pub setup: Option<Setting>,
    /// One activity of their own per kind of night they play.
    pub activities: Vec<PlannedActivity>,
    /// §8's level, when they chose one.
    pub level: String,
    /// The moves to practise, in the order given.
    pub practise: Vec<String>,
    /// What the set-up changes, one line each, as §54 says them: listed under
    /// the first sentence rather than folded into it.
    pub changes: Vec<String>,
    /// Every change, one sentence each, in the order it happens.
    pub says: Vec<String>,
}

/// The name an activity for a kind of night gets: the night's own title, made
/// plural where a DJ would say it that way -- "Weddings", "Clubs" -- and never
/// a name djmanzo ships, which [`crate::activity::keep`] would refuse.
fn activity_title(night: Setting) -> String {
    match night {
        Setting::Club => "Club nights".to_owned(),
        Setting::Beach => "Beach sets".to_owned(),
        Setting::Wedding => "Weddings".to_owned(),
        Setting::Latin => "Latin nights".to_owned(),
        Setting::Practice => "My practice".to_owned(),
        Setting::OpenFormat => "Open format".to_owned(),
    }
}

/// What setting up will do with these answers.
#[must_use]
pub fn plan(answers: &Answers) -> Plan {
    let mut says = Vec::new();
    let setup = answers.nights.first().copied();
    let level = crate::level::Level::parse(&answers.level);
    let mut changes = Vec::new();
    if let Some(night) = setup {
        changes = crate::setup::setup(night).changes();
        // What the DJ chose over what the setup would have done, so the list
        // is what will happen rather than what would have: the level decides
        // how far the assistant goes, and a chosen look is the one worn.
        if level.is_some() {
            changes.retain(|line| !line.starts_with("Assistant:"));
        }
        if !answers.theme.is_empty() {
            let chosen = format!("Theme: {}", crate::theme::title(&answers.theme));
            match changes.iter().position(|line| line.starts_with("Theme:")) {
                Some(at) => changes[at] = chosen,
                None => changes.insert(1.min(changes.len()), chosen),
            }
        }
        says.push(format!(
            "Set djmanzo up for {} nights, the kind you play most.",
            night.title().to_lowercase()
        ));
    } else if !answers.theme.is_empty() {
        says.push(format!(
            "Wear the {} look.",
            crate::theme::title(&answers.theme)
        ));
    }
    let activities: Vec<PlannedActivity> = answers
        .nights
        .iter()
        .map(|&night| PlannedActivity {
            title: activity_title(night),
            workspace: crate::setup::setup(night).workspace,
            night,
        })
        .collect();
    for activity in &activities {
        says.push(format!(
            "Keep \u{201c}{}\u{201d} as an activity of your own: the {} arrangement.",
            activity.title, activity.workspace
        ));
    }
    if let Some(level) = level {
        says.push(format!(
            "Let djmanzo go as far as {}: {}",
            level.title(),
            level.about()
        ));
    }
    if !answers.learn.is_empty() {
        says.push(format!(
            "Start every new event with {} to learn and rehearse.",
            crate::gig::listed(&answers.learn)
        ));
    }
    if !answers.name.is_empty() {
        says.push(format!(
            "Put \u{201c}{}\u{201d} where the djmanzo mark is.",
            answers.name
        ));
    }
    Plan {
        setup,
        activities,
        level: answers.level.clone(),
        practise: answers.learn.clone(),
        changes,
        says,
    }
}

/// A new event, started from what the DJ told the welcome: the moves they
/// are learning are the moves it needs, so its path begins at learning and
/// rehearsing them. The rest is the event's own to say.
#[must_use]
pub fn started(answers: &Answers, gig: crate::gig::Gig) -> crate::gig::Gig {
    crate::gig::Gig {
        techniques: answers.learn.clone(),
        ..gig
    }
}

/// `welcome.json` beside the settings.
#[must_use]
pub fn file(config: &Path) -> PathBuf {
    config.join("welcome.json")
}

/// The answers kept, or none when the welcome has never been opened.
#[must_use]
pub fn load(config: &Path) -> Option<Answers> {
    let text = std::fs::read_to_string(file(config)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Keep answers, after [`check`].
///
/// # Errors
/// The refusal, or the file system's own sentence.
pub fn save(config: &Path, answers: Answers) -> Result<Answers, String> {
    let answers = check(answers).map_err(|refused| refused.to_string())?;
    std::fs::create_dir_all(config).map_err(|e| format!("{}: {e}", config.display()))?;
    let text = serde_json::to_string_pretty(&answers).map_err(|e| e.to_string())?;
    let path = file(config);
    let partial = config.join(".welcome.json.partial");
    std::fs::write(&partial, text).map_err(|e| format!("{}: {e}", partial.display()))?;
    std::fs::rename(&partial, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(answers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dj() -> Answers {
        Answers {
            name: "  Johannes  ".to_owned(),
            nights: vec![Setting::Wedding, Setting::Latin, Setting::Wedding],
            genres: vec!["House".to_owned(), "house".to_owned(), "disco".to_owned()],
            bpm_low: 100,
            bpm_high: 128,
            favourites: vec!["Romeo Santos".to_owned(), "  ".to_owned()],
            moves: vec!["cut".to_owned()],
            learn: vec!["cut".to_owned(), "echo out".to_owned()],
            level: "suggest".to_owned(),
            theme: String::new(),
            done: false,
        }
    }

    /// **Answers are held to what djmanzo can use**: spelled its way,
    /// without repeats, a move already played not also one to learn, and a
    /// blank line not a favourite.
    #[test]
    fn answers_are_held_to_what_djmanzo_can_use() {
        let kept = check(dj()).expect("the answers are usable");
        assert_eq!(kept.name, "Johannes");
        assert_eq!(kept.nights, [Setting::Wedding, Setting::Latin]);
        assert_eq!(kept.genres.len(), 2, "{:?}", kept.genres);
        assert_eq!(kept.learn, ["echo out"]);
        assert_eq!(kept.favourites, ["Romeo Santos"]);

        let mut wrong = dj();
        wrong.genres = vec!["polka-step".to_owned()];
        assert!(matches!(check(wrong), Err(Refused::Genre(_))));
        let mut wrong = dj();
        wrong.bpm_low = 140;
        wrong.bpm_high = 120;
        assert!(matches!(check(wrong), Err(Refused::Tempo(140, 120))));
        let mut wrong = dj();
        wrong.level = "everything".to_owned();
        assert!(matches!(check(wrong), Err(Refused::Level(_))));
        let mut wrong = dj();
        wrong.theme = "pkg-nowhere".to_owned();
        assert!(matches!(check(wrong), Err(Refused::Theme(_))));
        // Unsaid is not wrong.
        let unsaid = Answers::default();
        assert_eq!(check(unsaid.clone()), Ok(unsaid));
    }

    /// **The plan says what will happen before it does**, and what it plans
    /// is built from what exists: the set-up for the night played most, one
    /// activity per night with that night's arrangement, and never a name
    /// djmanzo ships.
    #[test]
    fn the_plan_is_made_of_what_exists_and_says_so() {
        let answers = check(dj()).unwrap();
        let plan = plan(&answers);
        assert_eq!(plan.setup, Some(Setting::Wedding));
        assert_eq!(
            plan.activities
                .iter()
                .map(|a| (a.title.as_str(), a.workspace))
                .collect::<Vec<_>>(),
            [
                ("Weddings", crate::setup::setup(Setting::Wedding).workspace),
                (
                    "Latin nights",
                    crate::setup::setup(Setting::Latin).workspace
                ),
            ]
        );
        let shipped: Vec<String> = crate::activity::shipped()
            .into_iter()
            .map(|a| a.title)
            .collect();
        let workspaces: Vec<String> = crate::cockpit::workspaces()
            .into_iter()
            .map(|w| w.name)
            .collect();
        for night in Setting::ALL {
            let title = activity_title(night);
            assert!(!shipped.contains(&title), "{title} is a name djmanzo ships");
            let workspace = crate::setup::setup(night).workspace;
            assert!(
                workspaces.iter().any(|w| w == workspace),
                "{night:?}'s set-up names {workspace}, which is not an arrangement"
            );
        }
        // One sentence per change: the set-up, two activities, the level,
        // the moves to learn, and the name.
        assert_eq!(plan.says.len(), 6, "{:#?}", plan.says);
        assert!(plan.says[0].contains("wedding"));
        assert_eq!(
            plan.changes,
            crate::setup::setup(Setting::Wedding)
                .changes()
                .into_iter()
                .filter(|line| !line.starts_with("Assistant:"))
                .collect::<Vec<_>>(),
            "the set-up's own lines, less the one the level decides"
        );
        assert_eq!(plan.practise, ["echo out"]);
        // Nothing asked, nothing planned.
        let nothing = super::plan(&Answers::default());
        assert!(nothing.says.is_empty() && nothing.setup.is_none());
    }

    /// **What the DJ chose is what the plan lists**, not what the set-up
    /// would have done: a level decides how far the assistant goes, so the
    /// set-up's own assistant line would be a change that does not happen;
    /// and a look chosen is the one named.
    #[test]
    fn the_djs_choices_replace_the_set_ups() {
        let wedding = crate::setup::setup(Setting::Wedding).changes();
        assert!(wedding.iter().any(|l| l.starts_with("Assistant:")));

        let mut answers = check(dj()).unwrap();
        answers.level = String::new();
        assert_eq!(
            plan(&answers).changes,
            wedding,
            "no level: the set-up's own"
        );

        answers.level = "prepare".to_owned();
        answers.theme = "pkg-industrial".to_owned();
        let changes = plan(&answers).changes;
        assert!(
            !changes.iter().any(|l| l.starts_with("Assistant:")),
            "{changes:#?}"
        );
        let themes: Vec<&String> = changes.iter().filter(|l| l.starts_with("Theme:")).collect();
        assert_eq!(
            themes,
            [&format!("Theme: {}", crate::theme::title("pkg-industrial"))],
            "one theme line, and it is the chosen one"
        );
        assert_ne!(
            crate::theme::title("pkg-industrial"),
            "pkg-industrial",
            "a name, not an id"
        );

        // A look with no night to set up is still said.
        let look = Answers {
            theme: "pkg-industrial".to_owned(),
            ..Answers::default()
        };
        assert_eq!(
            plan(&look).says,
            [format!(
                "Wear the {} look.",
                crate::theme::title("pkg-industrial")
            )]
        );
    }

    /// **A new event begins where the welcome left the DJ**: the moves they
    /// are learning are the moves it needs, so its path starts at learning
    /// and rehearsing them -- which is what the plan promised.
    #[test]
    fn a_new_event_starts_with_the_moves_being_learned() {
        let answers = check(dj()).unwrap();
        let gig = crate::gig::Gig {
            title: "Anna and Ben".to_owned(),
            ..crate::gig::Gig::default()
        };
        let started = started(&answers, gig);
        assert_eq!(started.title, "Anna and Ben");
        assert_eq!(started.techniques, answers.learn);
        let path = crate::gig::path(&started);
        assert!(path[0].done, "the moves are chosen");
        assert!(path[1].about.contains("echo out"), "{}", path[1].about);
        assert!(
            plan(&answers)
                .says
                .iter()
                .any(|line| line.contains("new event") && line.contains("echo out")),
            "the plan says so"
        );
    }

    /// Kept as checked, and read back the same.
    #[test]
    fn answers_are_kept_and_read_back() {
        let dir = std::env::temp_dir().join(format!("djmanzo-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(&dir), None);
        let kept = save(&dir, dj()).expect("kept");
        assert_eq!(load(&dir), Some(kept));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

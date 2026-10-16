//! §121: dashboards that are dashboards.
//!
//! > i meant the dashboard to be a dashboard not only like the which-key
//! > shortcut widget. i meant something else. press kit desktop like folder
//! > organization, quick links to workflow like current activities as of
//! > different dashboards (prepare dashboard, live dashboard, messing
//! > around/experimental dashboard, my music dashbaord, my presskit dashboard,
//! > social dashboard) ... all these dashboards can hold widgets, info,
//! > controls as of their topics and more ... they should replace (apart from
//! > the ubuquitous main bar ...) the main view entirely and show their view
//! > while active ... so a dashboard is like a specific activity itself, but
//! > a dashboard acitvity
//!
//! §117's screen of tiles, called up with `0`, is a launcher — every place
//! djmanzo can take the DJ, on one screen — and is called that now. A
//! **board** is what was meant: a whole view for one kind of work away from
//! the decks, which takes the place of the decks and the docks while it is
//! up and gives them back, as they were, when the DJ goes back to them. The
//! bar across the top stays, with the decks in brief on it — what is playing,
//! how long is left, play and pause — so the music is never out of reach
//! while the DJ is looking at something else.
//!
//! # A board is made of what djmanzo already has
//!
//! Each board is a grid of **widgets**, and a widget is one of the cockpit's
//! surfaces ([`crate::cockpit::surfaces`]) drawn larger, or one of the few
//! that exist only on a board ([`BOARD_WIDGETS`]). The library on the music
//! board is the library, not a second one that could disagree with it — the
//! failure this codebase keeps finding is two descriptions of one thing.
//!
//! The grid is twelve columns wide; a widget says how many it spans and how
//! many rows tall it is, and the interface lays them out in order.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One widget on a board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Widget {
    /// A cockpit surface's name, or one of [`BOARD_WIDGETS`].
    pub name: &'static str,
    /// How many of the board's twelve columns it spans.
    pub cols: u8,
    /// How many rows tall it is.
    pub rows: u8,
}

/// A whole view for one kind of work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Board {
    /// The stable name, as a key, a controller line and a saved choice use it.
    pub slug: Cow<'static, str>,
    pub title: Cow<'static, str>,
    /// What it is for, in one line.
    pub about: Cow<'static, str>,
    /// A drawing from the interface's icon set.
    pub glyph: Cow<'static, str>,
    pub widgets: Vec<Widget>,
    /// §117: the DJ has arranged it by hand (`boards.json`), so *Put back
    /// djmanzo's* has something to put back.
    pub arranged: bool,
    /// §117: a board of the DJ's own (`my-boards.json`), which can be
    /// forgotten rather than put back.
    pub mine: bool,
}

/// The widgets that exist only on a board, not as a panel a dock can hold.
///
/// - `flow` — the mix: the crossfader, the master and the headphones, the
///   decks meet in. The decks themselves are in brief at the top of every
///   board.
/// - `links` — the way to the rest of the work: the activities on their
///   digits and the other boards.
/// - `folders` — the press kit laid out like a desktop: a folder for each
///   part of it, opened where it is.
pub const BOARD_WIDGETS: [&str; 3] = ["flow", "links", "folders"];

const fn w(name: &'static str, cols: u8, rows: u8) -> Widget {
    Widget { name, cols, rows }
}

/// The boards djmanzo ships, in the order the owner listed them.
#[must_use]
pub fn all() -> Vec<Board> {
    vec![
        Board {
            slug: Cow::Borrowed("prepare"),
            title: Cow::Borrowed("Prepare"),
            about: Cow::Borrowed(
                "Readying records for a night: the collection, what is set aside, the plan and the pairings.",
            ),
            glyph: Cow::Borrowed("list-ol"),
            arranged: false,
            mine: false,
            widgets: vec![
                w("library", 8, 2),
                w("prepare", 4, 1),
                w("next", 4, 1),
                w("plan", 6, 1),
                w("pair", 6, 1),
            ],
        },
        Board {
            slug: Cow::Borrowed("live"),
            title: Cow::Borrowed("Live"),
            about: Cow::Borrowed(
                "Playing: the decks in brief, what comes next, the night, the room and what it asks for.",
            ),
            glyph: Cow::Borrowed("headphones"),
            arranged: false,
            mine: false,
            widgets: vec![
                w("flow", 12, 1),
                w("next", 4, 1),
                w("night", 4, 1),
                w("requests", 4, 1),
                w("crowd", 6, 1),
                w("room", 6, 1),
            ],
        },
        Board {
            slug: Cow::Borrowed("lab"),
            title: Cow::Borrowed("Experiment"),
            about: Cow::Borrowed(
                "Messing about: the sampler, presets, practice, and the keys to try things with.",
            ),
            glyph: Cow::Borrowed("flask"),
            arranged: false,
            mine: false,
            widgets: vec![
                w("flow", 12, 1),
                w("sampler", 6, 1),
                w("presets", 6, 1),
                w("practice", 8, 1),
                w("keys", 4, 1),
            ],
        },
        Board {
            slug: Cow::Borrowed("music"),
            title: Cow::Borrowed("My music"),
            about: Cow::Borrowed(
                "The collection, the mixes you have recorded, and what you keep at hand.",
            ),
            glyph: Cow::Borrowed("compact-disc"),
            arranged: false,
            mine: false,
            widgets: vec![w("library", 8, 3), w("mixes", 4, 2), w("athand", 4, 1)],
        },
        Board {
            slug: Cow::Borrowed("kit"),
            title: Cow::Borrowed("Press kit"),
            about: Cow::Borrowed(
                "Who you are as a DJ, laid out like a desktop: photos, documents, bookings, links.",
            ),
            glyph: Cow::Borrowed("folder-open"),
            arranged: false,
            mine: false,
            widgets: vec![w("folders", 4, 2), w("kit", 8, 2), w("event", 12, 1)],
        },
        Board {
            slug: Cow::Borrowed("social"),
            title: Cow::Borrowed("Social"),
            about: Cow::Borrowed(
                "The people: the crowd's comments, their requests, your mixes and your kit to share.",
            ),
            glyph: Cow::Borrowed("cloud"),
            arranged: false,
            mine: false,
            widgets: vec![
                w("crowd", 6, 2),
                w("requests", 6, 1),
                w("mixes", 6, 1),
                w("links", 12, 1),
            ],
        },
        // Asked for with the karaoke build: a host's night, and the hour
        // before it. After the owner's six, so their keys do not move.
        Board {
            slug: Cow::Borrowed("karaoke"),
            title: Cow::Borrowed("Karaoke"),
            about: Cow::Borrowed(
                "Hosting singers: the decks in brief, who is up and what they sing, and the songbook beside them.",
            ),
            glyph: Cow::Borrowed("microphone"),
            arranged: false,
            mine: false,
            widgets: vec![w("flow", 12, 1), w("karaoke", 6, 2), w("library", 6, 2)],
        },
        Board {
            slug: Cow::Borrowed("karaoke-prep"),
            title: Cow::Borrowed("Karaoke prep"),
            about: Cow::Borrowed(
                "Before the singers arrive: the songbook to check, the singers' settings, the karaoke presets and the night.",
            ),
            glyph: Cow::Borrowed("list-ol"),
            arranged: false,
            mine: false,
            widgets: vec![
                w("library", 8, 2),
                w("karaoke", 4, 2),
                w("presets", 6, 1),
                w("event", 6, 1),
            ],
        },
    ]
}

/// §117: a widget as the DJ placed it, as `boards.json` keeps it and the
/// interface sends it — a name rather than djmanzo's own string, so
/// [`check`] decides whether it is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub name: String,
    pub cols: u8,
    pub rows: u8,
}

/// The most widgets one board holds: a board of everything is the cockpit.
pub const MOST_WIDGETS: usize = 12;

/// What a widget can be: a panel the shell draws (`leader::PANELS`) or one
/// of [`BOARD_WIDGETS`].
#[must_use]
pub fn placeable() -> Vec<&'static str> {
    crate::leader::PANELS
        .iter()
        .chain(BOARD_WIDGETS.iter())
        .copied()
        .collect()
}

/// §117: hold an arrangement to what a board can be — something the
/// interface draws, each once, two to twelve columns and one to three rows,
/// at least one widget and at most [`MOST_WIDGETS`].
///
/// # Errors
/// What is wrong with it, as a sentence.
pub fn check(widgets: &[Placed]) -> Result<Vec<Widget>, String> {
    if widgets.is_empty() {
        return Err("a board needs something on it".to_owned());
    }
    if widgets.len() > MOST_WIDGETS {
        return Err(format!("a board holds {MOST_WIDGETS} widgets at most"));
    }
    let known = placeable();
    let mut out: Vec<Widget> = Vec::with_capacity(widgets.len());
    for placed in widgets {
        let Some(&name) = known.iter().find(|name| **name == placed.name) else {
            return Err(format!("djmanzo has no widget called {:?}", placed.name));
        };
        if out.iter().any(|widget| widget.name == name) {
            return Err(format!("{name} is on the board twice"));
        }
        if !(2..=12).contains(&placed.cols) || !(1..=3).contains(&placed.rows) {
            return Err(format!(
                "{name} cannot be {} columns by {} rows",
                placed.cols, placed.rows
            ));
        }
        out.push(w(name, placed.cols, placed.rows));
    }
    Ok(out)
}

/// Where the DJ's arrangements are kept.
#[must_use]
pub fn file(config: &Path) -> PathBuf {
    config.join("boards.json")
}

/// The DJ's arrangements, by board slug; empty when there are none or the
/// file cannot be read.
#[must_use]
pub fn kept(config: &Path) -> BTreeMap<String, Vec<Placed>> {
    std::fs::read_to_string(file(config))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Keep `widgets` as the DJ's arrangement of the board `slug`, or with
/// `None` put djmanzo's back.
///
/// # Errors
/// A board djmanzo does not have, an arrangement [`check`] refuses, or the
/// file system's own sentence.
pub fn keep(config: &Path, slug: &str, widgets: Option<Vec<Placed>>) -> Result<(), String> {
    if slug.starts_with(MINE) {
        return keep_mine(config, slug, widgets);
    }
    if !all().iter().any(|board| board.slug == slug) {
        return Err(format!("djmanzo has no board called {slug:?}"));
    }
    let mut boards = kept(config);
    match widgets {
        Some(widgets) => {
            check(&widgets)?;
            boards.insert(slug.to_owned(), widgets);
        }
        None => {
            boards.remove(slug);
        }
    }
    let text = serde_json::to_string_pretty(&boards).map_err(|e| e.to_string())?;
    std::fs::write(file(config), text).map_err(|e| e.to_string())
}

/// The boards, with the DJ's arrangements laid over djmanzo's — one that no
/// longer holds (a widget since gone) is left out rather than opening with a
/// hole in it.
#[must_use]
pub fn arranged(kept: &BTreeMap<String, Vec<Placed>>, mine: &[MyBoard]) -> Vec<Board> {
    all()
        .into_iter()
        .map(
            |board| match kept.get(board.slug.as_ref()).map(|widgets| check(widgets)) {
                Some(Ok(widgets)) => Board {
                    widgets,
                    arranged: true,
                    ..board
                },
                _ => board,
            },
        )
        .chain(mine.iter().filter_map(|own| {
            Some(Board {
                slug: Cow::Owned(slug_of(&own.title)),
                title: Cow::Owned(own.title.clone()),
                about: Cow::Borrowed("A board of your own."),
                glyph: Cow::Borrowed("desktop"),
                widgets: check(&own.widgets).ok()?,
                arranged: false,
                mine: true,
            })
        }))
        .collect()
}

/// What a board of the DJ's own begins with, so its slug can never be one
/// of djmanzo's.
pub const MINE: &str = "mine-";

/// The most boards of their own a DJ keeps.
pub const MOST_MINE: usize = 8;

/// §117: a board of the DJ's own, as `my-boards.json` keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyBoard {
    pub title: String,
    pub widgets: Vec<Placed>,
}

/// A board of the DJ's own's slug: [`MINE`] and its name, in lower case,
/// its words joined by dashes.
#[must_use]
pub fn slug_of(title: &str) -> String {
    let words: Vec<String> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    format!("{MINE}{}", words.join("-"))
}

/// Where the DJ's own boards are kept.
#[must_use]
pub fn mine_file(config: &Path) -> PathBuf {
    config.join("my-boards.json")
}

/// The DJ's own boards, in the order they were made.
#[must_use]
pub fn mine(config: &Path) -> Vec<MyBoard> {
    std::fs::read_to_string(mine_file(config))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_mine(config: &Path, boards: &[MyBoard]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(boards).map_err(|e| e.to_string())?;
    std::fs::write(mine_file(config), text).map_err(|e| e.to_string())
}

/// §117: make a board of the DJ's own, named `title`, holding `widgets`;
/// its slug is [`slug_of`] the name.
///
/// # Errors
/// A name of none or more than forty characters, or one with no letters or
/// digits in it; one that is already a board of theirs; more than
/// [`MOST_MINE`]; an arrangement [`check`] refuses; or the file system.
pub fn make(config: &Path, title: &str, widgets: Vec<Placed>) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 40 {
        return Err("a board's name is one to forty characters".to_owned());
    }
    let slug = slug_of(title);
    if slug == MINE {
        return Err("a board's name needs a letter or a digit in it".to_owned());
    }
    check(&widgets)?;
    let mut boards = mine(config);
    if boards.iter().any(|own| slug_of(&own.title) == slug) {
        return Err(format!("you already have a board called {title:?}"));
    }
    if boards.len() >= MOST_MINE {
        return Err(format!("{MOST_MINE} boards of your own at most"));
    }
    boards.push(MyBoard {
        title: title.to_owned(),
        widgets,
    });
    write_mine(config, &boards)?;
    Ok(slug)
}

/// §117: forget a board of the DJ's own.
///
/// # Errors
/// No board of theirs by that slug, or the file system.
pub fn forget(config: &Path, slug: &str) -> Result<(), String> {
    let mut boards = mine(config);
    let before = boards.len();
    boards.retain(|own| slug_of(&own.title) != slug);
    if boards.len() == before {
        return Err(format!("you have no board called {slug:?}"));
    }
    write_mine(config, &boards)
}

/// An arrangement of a board of the DJ's own: kept on the board itself,
/// since there is no djmanzo's to put back.
fn keep_mine(config: &Path, slug: &str, widgets: Option<Vec<Placed>>) -> Result<(), String> {
    let Some(widgets) = widgets else {
        return Err("a board of your own has no djmanzo's to put back".to_owned());
    };
    check(&widgets)?;
    let mut boards = mine(config);
    let own = boards
        .iter_mut()
        .find(|own| slug_of(&own.title) == slug)
        .ok_or_else(|| format!("you have no board called {slug:?}"))?;
    own.widgets = widgets;
    write_mine(config, &boards)
}

/// The run that takes the DJ back to the decks from any board.
pub const DECKS: &str = "decks";

/// Whether `rest`, after `board `, is something a key may run: a board's
/// slug, or [`DECKS`].
#[must_use]
pub fn runnable(rest: &str) -> bool {
    rest == DECKS || all().iter().any(|board| board.slug == rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(name: &str, cols: u8, rows: u8) -> Placed {
        Placed {
            name: name.to_owned(),
            cols,
            rows,
        }
    }

    /// §117: **a board arranged by hand is held to what a board can be** —
    /// something the interface draws, each once, two to twelve columns by
    /// one to three rows, never empty and never more than a dozen.
    #[test]
    fn an_arrangement_is_held_to_what_a_board_can_be() {
        let fine = check(&[placed("mixes", 12, 1), placed("library", 6, 3)]).expect("fine");
        assert_eq!(fine, [w("mixes", 12, 1), w("library", 6, 3)]);
        assert!(check(&[]).is_err(), "an empty board");
        assert!(
            check(&[placed("spreadsheet", 4, 1)]).is_err(),
            "nothing draws it"
        );
        assert!(
            check(&[placed("mixes", 4, 1), placed("mixes", 6, 1)]).is_err(),
            "twice"
        );
        assert!(check(&[placed("mixes", 1, 1)]).is_err(), "one column");
        assert!(
            check(&[placed("mixes", 13, 1)]).is_err(),
            "thirteen columns"
        );
        assert!(check(&[placed("mixes", 4, 0)]).is_err(), "no rows");
        assert!(check(&[placed("mixes", 4, 4)]).is_err(), "four rows");
        assert!(
            check(&[placed("flow", 12, 1)]).is_ok(),
            "a board's own widget"
        );
        let many: Vec<Placed> = placeable()
            .into_iter()
            .take(MOST_WIDGETS + 1)
            .map(|name| placed(name, 4, 1))
            .collect();
        assert!(check(&many).is_err(), "more than a dozen");
        assert!(check(&many[..MOST_WIDGETS]).is_ok());
    }

    /// §117: **the DJ's arrangement is kept and laid over djmanzo's** — that
    /// board only, marked as arranged; put back, it is djmanzo's again; a
    /// board djmanzo does not have, or an arrangement refused, keeps nothing;
    /// and one kept that no longer holds is left out rather than drawn.
    #[test]
    fn a_board_arranged_by_hand_is_kept_and_can_be_put_back() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let mine = vec![placed("mixes", 12, 1), placed("library", 6, 3)];
        keep(config, "music", Some(mine.clone())).expect("kept");
        let boards = arranged(&kept(config), &[]);
        let music = boards.iter().find(|b| b.slug == "music").expect("music");
        assert!(music.arranged);
        assert_eq!(music.widgets, check(&mine).expect("fine"));
        let live = boards.iter().find(|b| b.slug == "live").expect("live");
        assert!(!live.arranged, "another board was touched");
        assert_eq!(live.widgets, all()[1].widgets);

        assert!(keep(config, "nowhere", Some(mine.clone())).is_err());
        assert!(keep(config, "live", Some(vec![])).is_err());
        assert!(
            !arranged(&kept(config), &[])
                .iter()
                .any(|b| b.slug == "live" && b.arranged)
        );

        keep(config, "music", None).expect("put back");
        let music = arranged(&kept(config), &[])
            .into_iter()
            .find(|b| b.slug == "music")
            .expect("music");
        assert!(!music.arranged);
        assert_eq!(music.widgets, all()[3].widgets);

        let mut stale = BTreeMap::new();
        stale.insert("music".to_owned(), vec![placed("gone", 4, 1)]);
        let music = arranged(&stale, &[])
            .into_iter()
            .find(|b| b.slug == "music")
            .expect("music");
        assert!(
            !music.arranged,
            "an arrangement that no longer holds was drawn"
        );
    }

    /// §117: **a board of the DJ's own** — made under a name from an
    /// arrangement, listed after djmanzo's with the desktop icon, its slug
    /// never one of djmanzo's; arranged on itself, never put back; a name
    /// empty, too long, wordless or taken refused, and no more than
    /// [`MOST_MINE`]; forgotten on request, and only one that is theirs.
    #[test]
    fn a_board_of_the_djs_own_is_made_arranged_and_forgotten() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let widgets = vec![placed("library", 8, 2), placed("mixes", 4, 1)];
        let slug = make(config, "  Sunday sets!  ", widgets.clone()).expect("made");
        assert_eq!(slug, "mine-sunday-sets");
        let boards = arranged(&kept(config), &mine(config));
        assert_eq!(boards.len(), all().len() + 1);
        let own = boards.last().expect("theirs, after djmanzo's");
        assert_eq!(
            (own.slug.as_ref(), own.title.as_ref()),
            (slug.as_str(), "Sunday sets!")
        );
        assert!(own.mine && !own.arranged);
        assert_eq!(own.glyph, "desktop");
        assert_eq!(own.widgets, check(&widgets).expect("fine"));

        keep(config, &slug, Some(vec![placed("crowd", 12, 1)])).expect("arranged");
        let own = arranged(&kept(config), &mine(config))
            .pop()
            .expect("theirs");
        assert_eq!(own.widgets, [w("crowd", 12, 1)]);
        assert!(keep(config, &slug, None).is_err(), "put back to what?");
        assert!(
            keep(config, &slug, Some(vec![])).is_err(),
            "an empty board of their own"
        );
        assert!(
            kept(config).is_empty(),
            "an own board's arrangement went to djmanzo's file"
        );

        assert!(make(config, "", widgets.clone()).is_err());
        assert!(make(config, &"x".repeat(41), widgets.clone()).is_err());
        assert!(
            make(config, "!!", widgets.clone()).is_err(),
            "no letter or digit"
        );
        assert!(
            make(config, "sunday SETS", widgets.clone()).is_err(),
            "the same board twice"
        );
        assert!(make(config, "Empty", vec![]).is_err());
        for n in 1..MOST_MINE {
            make(config, &format!("Board {n}"), widgets.clone()).expect("made");
        }
        assert!(
            make(config, "One more", widgets.clone()).is_err(),
            "more than eight"
        );
        assert!(all().iter().all(|board| !board.slug.starts_with(MINE)));

        assert!(
            forget(config, "music").is_err(),
            "djmanzo's is not theirs to forget"
        );
        forget(config, &slug).expect("forgotten");
        assert!(
            !arranged(&kept(config), &mine(config))
                .iter()
                .any(|b| b.slug == slug.as_str())
        );
        assert!(forget(config, &slug).is_err(), "forgotten twice");
    }

    /// **Every widget is something the interface draws**: a surface the
    /// shell draws (read out of `App.svelte`, not out of the declarations),
    /// or a board widget. A board naming anything else would open with a
    /// hole in it.
    #[test]
    fn every_widget_is_something_the_interface_draws() {
        let drawn = crate::cockpit::shell_draws();
        for board in all() {
            assert!(!board.widgets.is_empty(), "{} is empty", board.slug);
            for widget in &board.widgets {
                assert!(
                    drawn.contains(widget.name) || BOARD_WIDGETS.contains(&widget.name),
                    "{} names {}, which nothing draws",
                    board.slug,
                    widget.name
                );
                assert!(
                    (1..=12).contains(&widget.cols) && (1..=3).contains(&widget.rows),
                    "{} gives {} {}×{}",
                    board.slug,
                    widget.name,
                    widget.cols,
                    widget.rows
                );
            }
        }
    }

    /// The six the owner listed, each once and first, then the two karaoke
    /// boards asked for with the karaoke build; each with an icon the
    /// interface has, and every board widget on at least one board.
    #[test]
    fn the_six_boards_the_owner_listed_are_there() {
        let boards = all();
        let slugs: Vec<&str> = boards.iter().map(|board| board.slug.as_ref()).collect();
        assert_eq!(
            slugs,
            [
                "prepare",
                "live",
                "lab",
                "music",
                "kit",
                "social",
                "karaoke",
                "karaoke-prep"
            ]
        );
        let icons = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/icons.ts"
        ))
        .expect("the icon set");
        for board in &boards {
            assert!(
                icons.contains(&format!("\"{}\":", board.glyph)),
                "{} has no drawing called {}",
                board.slug,
                board.glyph
            );
        }
        for widget in BOARD_WIDGETS {
            assert!(
                boards
                    .iter()
                    .any(|board| board.widgets.iter().any(|w| w.name == widget)),
                "{widget} is on no board"
            );
        }
        assert!(runnable("live") && runnable(DECKS) && !runnable("dig"));
    }
}

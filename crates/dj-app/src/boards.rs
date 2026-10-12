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
    pub slug: &'static str,
    pub title: &'static str,
    /// What it is for, in one line.
    pub about: &'static str,
    /// A drawing from the interface's icon set.
    pub glyph: &'static str,
    pub widgets: Vec<Widget>,
    /// §117: the DJ has arranged it by hand (`boards.json`), so *Put back
    /// djmanzo's* has something to put back.
    pub arranged: bool,
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
            slug: "prepare",
            title: "Prepare",
            about: "Readying records for a night: the collection, what is set aside, the plan and the pairings.",
            glyph: "list-ol",
            arranged: false,
            widgets: vec![
                w("library", 8, 2),
                w("prepare", 4, 1),
                w("next", 4, 1),
                w("plan", 6, 1),
                w("pair", 6, 1),
            ],
        },
        Board {
            slug: "live",
            title: "Live",
            about: "Playing: the decks in brief, what comes next, the night, the room and what it asks for.",
            glyph: "headphones",
            arranged: false,
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
            slug: "lab",
            title: "Experiment",
            about: "Messing about: the sampler, presets, practice, and the keys to try things with.",
            glyph: "flask",
            arranged: false,
            widgets: vec![
                w("flow", 12, 1),
                w("sampler", 6, 1),
                w("presets", 6, 1),
                w("practice", 8, 1),
                w("keys", 4, 1),
            ],
        },
        Board {
            slug: "music",
            title: "My music",
            about: "The collection, the mixes you have recorded, and what you keep at hand.",
            glyph: "compact-disc",
            arranged: false,
            widgets: vec![w("library", 8, 3), w("mixes", 4, 2), w("athand", 4, 1)],
        },
        Board {
            slug: "kit",
            title: "Press kit",
            about: "Who you are as a DJ, laid out like a desktop: photos, documents, bookings, links.",
            glyph: "folder-open",
            arranged: false,
            widgets: vec![w("folders", 4, 2), w("kit", 8, 2), w("event", 12, 1)],
        },
        Board {
            slug: "social",
            title: "Social",
            about: "The people: the crowd's comments, their requests, your mixes and your kit to share.",
            glyph: "cloud",
            arranged: false,
            widgets: vec![
                w("crowd", 6, 2),
                w("requests", 6, 1),
                w("mixes", 6, 1),
                w("links", 12, 1),
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
pub fn arranged(kept: &BTreeMap<String, Vec<Placed>>) -> Vec<Board> {
    all()
        .into_iter()
        .map(
            |board| match kept.get(board.slug).map(|widgets| check(widgets)) {
                Some(Ok(widgets)) => Board {
                    widgets,
                    arranged: true,
                    ..board
                },
                _ => board,
            },
        )
        .collect()
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
        let boards = arranged(&kept(config));
        let music = boards.iter().find(|b| b.slug == "music").expect("music");
        assert!(music.arranged);
        assert_eq!(music.widgets, check(&mine).expect("fine"));
        let live = boards.iter().find(|b| b.slug == "live").expect("live");
        assert!(!live.arranged, "another board was touched");
        assert_eq!(live.widgets, all()[1].widgets);

        assert!(keep(config, "nowhere", Some(mine.clone())).is_err());
        assert!(keep(config, "live", Some(vec![])).is_err());
        assert!(
            !arranged(&kept(config))
                .iter()
                .any(|b| b.slug == "live" && b.arranged)
        );

        keep(config, "music", None).expect("put back");
        let music = arranged(&kept(config))
            .into_iter()
            .find(|b| b.slug == "music")
            .expect("music");
        assert!(!music.arranged);
        assert_eq!(music.widgets, all()[3].widgets);

        let mut stale = BTreeMap::new();
        stale.insert("music".to_owned(), vec![placed("gone", 4, 1)]);
        let music = arranged(&stale)
            .into_iter()
            .find(|b| b.slug == "music")
            .expect("music");
        assert!(
            !music.arranged,
            "an arrangement that no longer holds was drawn"
        );
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

    /// The six the owner listed, each once, each with an icon the interface
    /// has, and every board widget on at least one board.
    #[test]
    fn the_six_boards_the_owner_listed_are_there() {
        let boards = all();
        let slugs: Vec<&str> = boards.iter().map(|board| board.slug).collect();
        assert_eq!(slugs, ["prepare", "live", "lab", "music", "kit", "social"]);
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

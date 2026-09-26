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

use serde::Serialize;

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
            widgets: vec![w("library", 8, 3), w("mixes", 4, 2), w("athand", 4, 1)],
        },
        Board {
            slug: "kit",
            title: "Press kit",
            about: "Who you are as a DJ, laid out like a desktop: photos, documents, bookings, links.",
            glyph: "folder-open",
            widgets: vec![w("folders", 4, 2), w("kit", 8, 2), w("event", 12, 1)],
        },
        Board {
            slug: "social",
            title: "Social",
            about: "The people: the crowd's comments, their requests, your mixes and your kit to share.",
            glyph: "cloud",
            widgets: vec![
                w("crowd", 6, 2),
                w("requests", 6, 1),
                w("mixes", 6, 1),
                w("links", 12, 1),
            ],
        },
    ]
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

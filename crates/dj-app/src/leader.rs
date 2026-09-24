//! §117: the leader key, and the tree of mnemonics behind it.
//!
//! > i'd love something like neovim mnemonic shortcuts ... with a visual
//! > feedback guide/legend where we are after each key in the shortcut chain
//! > with all the possible breadcrumbs/outcomes
//!
//! Space opens a guide, and every key after it goes one step down a tree of
//! English words: `Space d 1 p` is **d**eck **1** **p**lay. The guide shows,
//! after each key, where the DJ is and every key that goes on from there, so
//! nothing has to be remembered before it is used — which is the whole idea of
//! which-key in Neovim and of Doom Emacs's `SPC` menus, and the reason they
//! work: a shortcut is learned by reading it at the moment it is wanted, a few
//! times, until the fingers do it without the guide. Sources in
//! docs/RESEARCH.md.
//!
//! # What the tree holds, and what it does not
//!
//! Everything djmanzo can do from a key, by name: the decks, the stems, the
//! mixer, recording, every panel, every activity, theme, workspace and preset.
//! It holds **no performance keys**. A hot cue or a kill held through a
//! breakdown has to be one physical key with nothing before it, and those stay
//! in the keyboard map (`dj-hid/mappings/keyboard-default.toml`), laid out for
//! two hands. The tree is for everything a DJ reaches for rather than plays.
//!
//! # How a leaf is run
//!
//! A leaf's `run` is a sentence of one of four kinds, each carried out by the
//! path the rest of the interface already takes:
//!
//! - `action <words>` — an action in djmanzo's vocabulary, sent to the
//!   engine exactly as the keyboard map's are;
//! - `surface <name>` — a panel opened or closed, as its button does;
//! - `switch <kind> <which>` — a theme, activity, workspace or preset, as the
//!   Ctrl+K palette switches one (§115);
//! - `ui <verb>` — something only the interface does: open the palette,
//!   search the library, go back to the last activity, record, mark.
//!
//! A test runs every leaf of the default tree through the parser, the panel
//! list and the switch list, so nothing is offered that djmanzo cannot do.
//!
//! # Which letter
//!
//! Hand-chosen where a group is fixed (`d` deck, `m` mixer, `r` record).
//! Where a group is a list — themes, workspaces, presets, panels — each entry
//! takes the first letter of its name that is still free, then the first
//! letter of a later word, then any letter of the name, in the order the list
//! is written; the guide underlines the letter inside the name, the way menu
//! accelerators have been shown on Windows and GTK for decades. The letter is
//! learned by seeing it in the word.

use serde::Serialize;
use std::collections::BTreeSet;

/// One step of the tree: a group to go into, or a leaf to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Node {
    /// The key that reaches this node from its parent, as the character it
    /// types: `d`, `1`, `,`, `?`. The one key that is not a character is
    /// written `space`.
    pub key: String,
    /// What the guide calls it.
    pub label: String,
    /// What running it does, for a leaf; `None` for a group.
    pub run: Option<String>,
    /// What going into it offers, for a group; empty for a leaf.
    pub children: Vec<Node>,
    /// Whether the DJ made it rather than djmanzo.
    pub mine: bool,
}

impl Node {
    fn leaf(key: impl Into<String>, label: impl Into<String>, run: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            run: Some(run.into()),
            children: Vec::new(),
            mine: false,
        }
    }

    fn group(key: impl Into<String>, label: impl Into<String>, children: Vec<Node>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            run: None,
            children,
            mine: false,
        }
    }

    /// Every leaf under this node, with the keys that reach it.
    #[must_use]
    pub fn leaves(&self) -> Vec<(Vec<String>, &Node)> {
        let mut out = Vec::new();
        let mut stack: Vec<(Vec<String>, &Node)> = vec![(Vec::new(), self)];
        while let Some((path, node)) = stack.pop() {
            if node.run.is_some() {
                out.push((path, node));
                continue;
            }
            for child in node.children.iter().rev() {
                let mut next = path.clone();
                next.push(child.key.clone());
                stack.push((next, child));
            }
        }
        out
    }
}

/// The panels the tree opens, most reached-for first so they keep their
/// first letters. Every name is a cockpit surface the interface draws.
pub const PANELS: [&str; 20] = [
    "library",
    "next",
    "prepare",
    "plan",
    "pair",
    "practice",
    "night",
    "athand",
    "mixes",
    "requests",
    "karaoke",
    "booth",
    "room",
    "sampler",
    "presets",
    "assistant",
    "controllers",
    "keys",
    "log",
    "settings",
];

/// The letter for one name, given the letters already taken: the first
/// letter of the first word, then of each later word, then any letter of the
/// name, then any letter at all. `None` when all twenty-six are taken.
#[must_use]
pub fn letter_for(name: &str, taken: &BTreeSet<char>) -> Option<char> {
    let words: Vec<Vec<char>> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.chars().map(|c| c.to_ascii_lowercase()).collect())
        .collect();
    let initials = words.iter().filter_map(|word| word.first().copied());
    let rest = words.iter().flat_map(|word| word.iter().skip(1).copied());
    initials
        .chain(rest)
        .chain('a'..='z')
        .find(|c| c.is_ascii_lowercase() && !taken.contains(c))
}

/// Letters for a list of names, in order, none of them in `reserved`.
fn lettered(names: &[String], reserved: &[char]) -> Vec<Option<char>> {
    let mut taken: BTreeSet<char> = reserved.iter().copied().collect();
    names
        .iter()
        .map(|name| {
            let letter = letter_for(name, &taken);
            if let Some(letter) = letter {
                taken.insert(letter);
            }
            letter
        })
        .collect()
}

/// A list of `(label, run)` as a group, each entry under its letter. Entries
/// past the twenty-sixth are left to the palette, where they are one search
/// away.
fn list(key: &str, label: &str, entries: Vec<(String, String)>) -> Node {
    let names: Vec<String> = entries.iter().map(|(label, _)| label.clone()).collect();
    let children = lettered(&names, &[])
        .into_iter()
        .zip(entries)
        .filter_map(|(letter, (label, run))| Some(Node::leaf(letter?.to_string(), label, run)))
        .collect();
    Node::group(key, label, children)
}

/// What one deck does from the tree.
fn deck(n: u8) -> Node {
    let a = |key: &str, label: &str, verb: &str| {
        Node::leaf(key, label, format!("action deck {n} {verb}"))
    };
    Node::group(
        n.to_string(),
        format!("Deck {n}"),
        vec![
            a("p", "Play / pause", "play_pause"),
            a("c", "Cue", "cue"),
            a("s", "Sync on / off", "sync_toggle"),
            a("k", "Keylock on / off", "keylock_toggle"),
            a("h", "Headphones on / off", "cue_toggle"),
            a("l", "Loop four beats", "loop 4"),
            a("o", "Loop off", "loop_off"),
            a("b", "Back four beats", "beatjump -4"),
            a("f", "Forward four beats", "beatjump 4"),
            a("g", "Grid: a beat here", "grid_here"),
            a("e", "Eject", "eject"),
        ],
    )
}

/// One deck's stems, each muted or brought back.
fn stems(n: u8) -> Node {
    let a = |key: &str, stem: &str| {
        Node::leaf(
            key,
            format!("Mute / unmute the {stem}"),
            format!(
                "action deck {n} stem_mute {}",
                if stem == "vocals" { "vocal" } else { stem }
            ),
        )
    };
    Node::group(
        n.to_string(),
        format!("Deck {n}"),
        vec![
            a("v", "vocals"),
            a("d", "drums"),
            a("b", "bass"),
            a("o", "other"),
        ],
    )
}

/// The panels, each under its letter, titled as the cockpit titles them.
fn panels() -> Node {
    let entries = PANELS
        .iter()
        .filter_map(|name| {
            let surface = crate::cockpit::surfaces()
                .iter()
                .find(|surface| surface.name == *name)?;
            Some((surface.title.to_owned(), format!("surface {name}")))
        })
        .collect();
    list("o", "Open a panel", entries)
}

/// The default tree, for `decks` decks and what the DJ has: activities (the
/// first nine on their digits, as the bare digits reach them), workspaces and
/// preset packs.
#[must_use]
pub fn tree(
    decks: u8,
    activities: &[crate::activity::Activity],
    workspaces: &[crate::cockpit::Workspace],
    packs: &[dj_presets::Pack],
) -> Node {
    let decks = decks.clamp(1, 6);
    let mut root = vec![
        Node::leaf("space", "Find anything (the palette)", "ui palette"),
        Node::leaf("?", "Every key", "surface keys"),
        Node::leaf("/", "Search the library", "ui search"),
        Node::leaf(",", "Settings", "surface settings"),
        Node::leaf("`", "Back to the last activity", "ui back"),
    ];
    for (index, activity) in activities.iter().take(9).enumerate() {
        root.push(Node::leaf(
            (index + 1).to_string(),
            activity.title.clone(),
            format!("switch activity {}", activity.slug),
        ));
    }
    root.push(Node::group("d", "Deck", (1..=decks).map(deck).collect()));
    root.push(Node::group("s", "Stems", (1..=decks).map(stems).collect()));
    root.push(Node::group(
        "m",
        "Mixer",
        vec![
            Node::leaf("c", "Crossfader to the centre", "action crossfader 0"),
            Node::leaf("a", "Crossfader to side A", "action crossfader -1"),
            Node::leaf("b", "Crossfader to side B", "action crossfader 1"),
            Node::leaf("x", "Stop every sample", "action sampler stop_all"),
        ],
    ));
    root.push(Node::group(
        "r",
        "Record",
        vec![
            Node::leaf("r", "Record the set on / off", "ui record"),
            Node::leaf("m", "Mark this moment", "ui mark"),
        ],
    ));
    root.push(panels());
    root.push(Node::group(
        "v",
        "View",
        activities
            .iter()
            .take(9)
            .enumerate()
            .map(|(index, activity)| {
                Node::leaf(
                    (index + 1).to_string(),
                    activity.title.clone(),
                    format!("switch activity {}", activity.slug),
                )
            })
            .chain([Node::leaf(
                "e",
                "Everything (leave activities)",
                "ui everything",
            )])
            .collect(),
    ));
    root.push(list(
        "t",
        "Theme",
        crate::theme::ALL
            .iter()
            .filter_map(|theme| match (theme.pack, theme.world) {
                (Some(pack), false) => {
                    Some((theme.title.to_owned(), format!("switch theme {pack}")))
                }
                _ => None,
            })
            .collect(),
    ));
    let mut seen = BTreeSet::new();
    root.push(list(
        "w",
        "Workspace",
        workspaces
            .iter()
            .filter(|workspace| seen.insert(workspace.name.clone()))
            .map(|workspace| {
                (
                    workspace.name.clone(),
                    format!("switch workspace {}", workspace.name),
                )
            })
            .collect(),
    ));
    root.push(presets(packs, decks));
    Node::group("space", "Space", root)
}

/// The preset packs: a preset for the whole mixer is a leaf, one written per
/// deck asks which deck next.
fn presets(packs: &[dj_presets::Pack], decks: u8) -> Node {
    let all: Vec<&dj_presets::Preset> = packs.iter().flat_map(|pack| &pack.presets).collect();
    let names: Vec<String> = all.iter().map(|preset| preset.name.clone()).collect();
    let children = lettered(&names, &[])
        .into_iter()
        .zip(all)
        .filter_map(|(letter, preset)| {
            let key = letter?.to_string();
            Some(if preset.per_deck {
                Node::group(
                    key,
                    preset.name.clone(),
                    (1..=decks)
                        .map(|deck| {
                            Node::leaf(
                                deck.to_string(),
                                format!("On deck {deck}"),
                                format!("switch preset {} {deck}", preset.id),
                            )
                        })
                        .collect(),
                )
            } else {
                Node::leaf(
                    key,
                    preset.name.clone(),
                    format!("switch preset {}", preset.id),
                )
            })
        })
        .collect();
    Node::group("p", "Preset", children)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_tree() -> Node {
        let activities = crate::activity::all(&[]);
        tree(
            4,
            &activities,
            &crate::cockpit::workspaces(),
            &dj_presets::builtin::packs(),
        )
    }

    /// **Every leaf does something djmanzo can do.** An action parses, a
    /// panel is one the cockpit has, a switch is one the palette offers, and
    /// a `ui` verb is one the interface carries out. A guide that offered a
    /// key which then did nothing would teach a DJ to stop trusting it.
    #[test]
    fn every_leaf_is_something_djmanzo_does() {
        let root = default_tree();
        let activities = crate::activity::all(&[]);
        let switches: Vec<String> = crate::commands::switch_runs(
            &activities,
            &crate::cockpit::workspaces(),
            &dj_presets::builtin::packs(),
            4,
        );
        let leaves = root.leaves();
        assert!(leaves.len() > 80, "{} leaves", leaves.len());
        for (path, leaf) in leaves {
            let run = leaf.run.as_deref().unwrap();
            let (kind, rest) = run.split_once(' ').unwrap_or((run, ""));
            match kind {
                "action" => {
                    assert!(
                        dj_core::action::Action::parse(rest).is_ok(),
                        "{path:?}: {rest}"
                    );
                }
                "surface" => assert!(
                    crate::cockpit::surfaces().iter().any(|s| s.name == rest),
                    "{path:?}: no panel {rest}"
                ),
                "switch" => assert!(
                    switches.iter().any(|s| s == rest),
                    "{path:?}: no switch {rest}"
                ),
                "ui" => assert!(
                    ["palette", "search", "back", "record", "mark", "everything"].contains(&rest),
                    "{path:?}: no ui verb {rest}"
                ),
                other => panic!("{path:?}: {other} is not a kind of leaf"),
            }
        }
    }

    /// **No two keys in one place, every key one character, every group
    /// with somewhere to go.** A key that meant two things would do whichever
    /// the interface found first.
    #[test]
    fn every_place_in_the_tree_is_unambiguous() {
        let root = default_tree();
        let mut stack = vec![&root];
        while let Some(node) = stack.pop() {
            if node.run.is_some() {
                continue;
            }
            assert!(!node.children.is_empty(), "{} leads nowhere", node.label);
            let mut keys = BTreeSet::new();
            for child in &node.children {
                assert!(
                    child.key == "space" || child.key.chars().count() == 1,
                    "{:?} under {}",
                    child.key,
                    node.label
                );
                assert!(
                    keys.insert(&child.key),
                    "{} twice under {}",
                    child.key,
                    node.label
                );
                stack.push(child);
            }
        }
    }

    /// **The digits under Space are the digits on their own.** Bare `1`–`9`
    /// switch activities (the owner's choice), and the guide shows the same
    /// nine under the same digits, in the same order.
    #[test]
    fn the_guide_digits_are_the_activity_keys() {
        let root = default_tree();
        let activities = crate::activity::all(&[]);
        for (index, activity) in activities.iter().take(9).enumerate() {
            let digit = (index + 1).to_string();
            let node = root
                .children
                .iter()
                .find(|child| child.key == digit)
                .unwrap();
            assert_eq!(
                node.run.as_deref(),
                Some(format!("switch activity {}", activity.slug).as_str())
            );
            assert_eq!(
                crate::activity::key_for(index).as_deref(),
                Some(format!("Digit{digit}").as_str())
            );
        }
    }

    /// A list takes each name's own first letter while it can, then a later
    /// word's, then any letter of the name — the letter is in the word, where
    /// the guide underlines it — and never one already taken.
    #[test]
    fn a_list_takes_letters_from_its_own_names() {
        let names: Vec<String> = ["Library", "Set plan", "Session log", "Settings", "Stems"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let letters: Vec<char> = lettered(&names, &[])
            .into_iter()
            .map(Option::unwrap)
            .collect();
        assert_eq!(letters, vec!['l', 's', 'e', 't', 'm']);
        for (name, letter) in names.iter().zip(&letters) {
            assert!(
                name.to_ascii_lowercase().contains(*letter),
                "{name}: {letter}"
            );
        }
        let full: BTreeSet<char> = ('a'..='z').collect();
        assert_eq!(letter_for("Anything", &full), None);
    }
}

//! §117: the dashboard, instead of toolbars.
//!
//! > minimalise the use of toolbars/space ... so i can use every space for my
//! > focused activity widget ... instead of toolbards let there be an central
//! > dashboard view with easy nav buttons for packages/presets/activities/
//! > overview ... even different dashboards per purpose/activity ... and
//! > auto-arrangeable dashboards
//!
//! The two rows of buttons above the decks — thirteen panels, the stage's
//! pickers — become one screen a DJ calls up with `0` (or from the guide, or
//! the header's button) and dismisses the same way, and the top of the window
//! keeps only what is read from across a booth: the set's REC, Mark and SAFE,
//! and the assistant's one line. The toolbars are a setting, off on a fresh
//! install and one switch away for a DJ who wants them back.
//!
//! # What is on it, and in what order
//!
//! Sections of tiles, each tile a leaf of the same kinds the leader's tree
//! runs (`crate::leader`), so a tile does exactly what the key would:
//!
//! 1. **For this activity** — when the DJ is in one, the panels its own
//!    arrangement opens, then the rest of what it is for. This is the
//!    dashboard *per purpose*: Karaoke's leads with the singers, Dig's with
//!    the collection.
//! 2. **Activities**, on their digits, in their own order: they are
//!    reached by key, and a tile that moved would teach the wrong digit.
//! 3. **Panels, presets, themes, workspaces** — each **arranged by use**:
//!    what the DJ reaches for most comes first, the rest in the order
//!    djmanzo lists them. Counted per tile in the interface settings, however
//!    the thing was reached (tile, key or palette), so the arrangement follows
//!    the DJ rather than the dashboard.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The interface's own settings: whether the toolbars are shown, and how
/// often each tile has been used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    /// The two rows of buttons above the decks. Off on a fresh install: the
    /// owner asked for the space.
    #[serde(default)]
    pub toolbars: bool,
    /// How many times each tile has been used, by its id.
    #[serde(default)]
    pub uses: BTreeMap<String, u32>,
}

impl Interface {
    /// One more use of a tile. Saturates rather than wrapping round to the
    /// bottom of the list.
    pub fn used(&mut self, id: &str) {
        let count = self.uses.entry(id.to_owned()).or_insert(0);
        *count = count.saturating_add(1);
    }
}

/// One tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tile {
    /// Stable: `activity:mix`, `surface:library`, `theme:aurora`.
    pub id: String,
    pub label: String,
    /// One line on what it is for, where there is one.
    pub about: String,
    /// What it runs, written as a leader leaf is (`switch activity mix`).
    pub run: String,
    /// The key that reaches it without the dashboard, where one does.
    pub key: Option<String>,
}

/// One section of tiles under its title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub title: String,
    pub tiles: Vec<Tile>,
}

/// The dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dashboard {
    pub sections: Vec<Section>,
}

/// Tiles in the order the DJ reaches for them: most used first, and the
/// rest — ties included — in the order they were listed. A stable sort, so
/// a list nobody has used yet is exactly djmanzo's own order.
#[must_use]
pub fn arranged(mut tiles: Vec<Tile>, uses: &BTreeMap<String, u32>) -> Vec<Tile> {
    tiles.sort_by_key(|tile| std::cmp::Reverse(uses.get(&tile.id).copied().unwrap_or(0)));
    tiles
}

fn tile(id: String, label: &str, about: &str, run: String) -> Tile {
    Tile {
        id,
        label: label.to_owned(),
        about: about.to_owned(),
        run,
        key: None,
    }
}

/// A panel's tile, titled as the cockpit titles it.
fn panel(name: &str) -> Option<Tile> {
    let surface = crate::cockpit::surface(name)?;
    Some(tile(
        format!("surface:{name}"),
        surface.title,
        surface.about,
        format!("surface {name}"),
    ))
}

/// The dashboard for what the DJ has and where they are: `current` is the
/// activity they are in, when they are in one.
#[must_use]
pub fn build(
    activities: &[crate::activity::Activity],
    current: Option<&crate::activity::Activity>,
    workspaces: &[crate::cockpit::Workspace],
    packs: &[dj_presets::Pack],
    interface: &Interface,
) -> Dashboard {
    let uses = &interface.uses;
    let mut sections = Vec::new();

    if let Some(activity) = current {
        let mut tiles: Vec<Tile> = activity
            .workspace
            .surfaces
            .iter()
            .filter_map(|placement| panel(&placement.surface))
            .collect();
        tiles.dedup_by(|a, b| a.id == b.id);
        if !tiles.is_empty() {
            sections.push(Section {
                title: format!("For {}", activity.title),
                tiles,
            });
        }
    }

    sections.push(Section {
        title: "Activities".to_owned(),
        tiles: activities
            .iter()
            .enumerate()
            .map(|(index, activity)| Tile {
                key: crate::activity::key_for(index)
                    .map(|code| code.trim_start_matches("Digit").to_owned()),
                ..tile(
                    format!("activity:{}", activity.slug),
                    &activity.title,
                    &activity.doing,
                    format!("switch activity {}", activity.slug),
                )
            })
            .chain([Tile {
                key: Some("`".to_owned()),
                ..tile(
                    "ui:everything".to_owned(),
                    "Everything",
                    "Every panel, arranged by hand — the full cockpit.",
                    "ui everything".to_owned(),
                )
            }])
            .collect(),
    });

    sections.push(Section {
        title: "Panels".to_owned(),
        tiles: arranged(
            crate::leader::PANELS
                .iter()
                .filter_map(|name| panel(name))
                .collect(),
            uses,
        ),
    });

    sections.push(Section {
        title: "Presets".to_owned(),
        tiles: arranged(
            packs
                .iter()
                .flat_map(|pack| &pack.presets)
                .filter(|preset| !preset.per_deck)
                .map(|preset| {
                    tile(
                        format!("preset:{}", preset.id),
                        &preset.name,
                        &preset.description,
                        format!("switch preset {}", preset.id),
                    )
                })
                .collect(),
            uses,
        ),
    });

    sections.push(Section {
        title: "Themes".to_owned(),
        tiles: arranged(
            crate::theme::ALL
                .iter()
                .filter_map(|theme| match (theme.pack, theme.world) {
                    (Some(pack), false) => Some(tile(
                        format!("theme:{pack}"),
                        theme.title,
                        theme.about,
                        format!("switch theme {pack}"),
                    )),
                    _ => None,
                })
                .collect(),
            uses,
        ),
    });

    let mut seen = std::collections::BTreeSet::new();
    sections.push(Section {
        title: "Workspaces".to_owned(),
        tiles: arranged(
            workspaces
                .iter()
                .filter(|workspace| seen.insert(workspace.name.clone()))
                .map(|workspace| {
                    tile(
                        format!("workspace:{}", workspace.name),
                        &workspace.name,
                        &workspace.about,
                        format!("switch workspace {}", workspace.name),
                    )
                })
                .collect(),
            uses,
        ),
    });

    Dashboard { sections }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_with(current: Option<&str>, interface: &Interface) -> Dashboard {
        let activities = crate::activity::all(&[]);
        let current = current.and_then(|slug| activities.iter().find(|a| a.slug == slug));
        build(
            &activities,
            current,
            &crate::cockpit::workspaces(),
            &dj_presets::builtin::packs(),
            interface,
        )
    }

    fn titles(dashboard: &Dashboard) -> Vec<&str> {
        dashboard
            .sections
            .iter()
            .map(|s| s.title.as_str())
            .collect()
    }

    /// **Every tile does something djmanzo does**, by the same check the
    /// leader's keys go through, and every section has something in it.
    #[test]
    fn every_tile_runs_something_djmanzo_does() {
        let activities = crate::activity::all(&[]);
        let switches = crate::commands::switch_runs(
            &activities,
            &crate::cockpit::workspaces(),
            &dj_presets::builtin::packs(),
            4,
        );
        let dashboard = build_with(Some("karaoke"), &Interface::default());
        for section in &dashboard.sections {
            assert!(!section.tiles.is_empty(), "{} is empty", section.title);
            for tile in &section.tiles {
                if let Err(why) = crate::leader::runnable(&tile.run, &switches) {
                    panic!("{}: {why}", tile.id);
                }
            }
        }
    }

    /// **A dashboard per purpose**: in an activity it leads with what that
    /// activity is for — Karaoke's with the singers, Dig's with the
    /// collection — and with none, it starts at the activities.
    #[test]
    fn each_activity_has_its_own_dashboard() {
        let none = build_with(None, &Interface::default());
        assert_eq!(titles(&none)[0], "Activities");

        let karaoke = build_with(Some("karaoke"), &Interface::default());
        assert_eq!(titles(&karaoke)[0], "For Karaoke");
        assert!(
            karaoke.sections[0]
                .tiles
                .iter()
                .any(|t| t.id == "surface:karaoke")
        );

        let dig = build_with(Some("dig"), &Interface::default());
        assert_eq!(titles(&dig)[0], "For Dig");
        assert!(
            dig.sections[0]
                .tiles
                .iter()
                .any(|t| t.id == "surface:library")
        );
        assert_ne!(karaoke.sections[0], dig.sections[0]);
    }

    /// **Arranged by use**: what the DJ reaches for most comes first, the
    /// rest in djmanzo's order. The activities keep theirs, because they are
    /// reached by digit and a tile that moved would teach the wrong one.
    #[test]
    fn panels_are_arranged_by_use_and_activities_are_not() {
        let fresh = build_with(None, &Interface::default());
        let panels = |d: &Dashboard| -> Vec<String> {
            let section = d.sections.iter().find(|s| s.title == "Panels").unwrap();
            section.tiles.iter().map(|t| t.id.clone()).collect()
        };
        assert_eq!(panels(&fresh)[0], "surface:library");

        let mut interface = Interface::default();
        for _ in 0..3 {
            interface.used("surface:booth");
        }
        interface.used("surface:keys");
        interface.used("activity:karaoke");
        let used = build_with(None, &interface);
        assert_eq!(
            &panels(&used)[..3],
            ["surface:booth", "surface:keys", "surface:library"]
        );

        let activities = &used.sections[0];
        assert_eq!(activities.tiles[0].id, "activity:dig");
        assert_eq!(activities.tiles[0].key.as_deref(), Some("1"));
    }

    /// A count that has run as far as it can stays there rather than
    /// wrapping to the bottom of the list.
    #[test]
    fn a_use_count_saturates() {
        let mut interface = Interface::default();
        interface.uses.insert("surface:booth".to_owned(), u32::MAX);
        interface.used("surface:booth");
        assert_eq!(interface.uses["surface:booth"], u32::MAX);
    }
}

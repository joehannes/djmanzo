//! §16's domain knowledge packs, as a format.
//!
//! > Make DJ knowledge extensible. At minimum define packs for: General,
//! > Beginner, Open Format, House, Techno, […] A pack can define: genre
//! > relationships, transition preferences, useful techniques, energy curves,
//! > allowed BPM relationships, half/double-time relationships, suggestion
//! > weighting, theme profile, UI density, pad defaults, preferred columns,
//! > suggested cues, typical preparation, risk tolerance, automation policy.
//! > **Do not hard-code this logic into UI components.**
//!
//! # What a pack is here, and what it is not
//!
//! djmanzo already holds most of §16's fifteen fields, in the places that own
//! them: [`crate::technique`] is the catalogue of moves, `dj_core::genre` is
//! the genre map with its felt tempos and rhythmic grammars, `dj_app::shape`
//! is what a transition style does, and §54's `dj_app::setup` is the
//! presentation half — theme, density, pad defaults, columns. A pack that
//! restated any of those would be a second copy of a table that already
//! exists, and the copy is the one that goes stale.
//!
//! So a pack **selects**. It names the genre families this kind of DJing turns
//! on and the techniques it teaches, and it points at the occasion whose
//! presentation it pairs with. Nothing here is a new fact about music; it is a
//! statement about which of djmanzo's existing facts this night is made of.
//!
//! # It is read, not merely stored
//!
//! The consumer is [`crate::coach::next_lesson`], which answers *the one thing
//! to work on next*. It used to pick the easiest technique in the whole
//! catalogue that the DJ had not shown — so a bachata DJ was eventually sent to
//! learn a transformer scratch, and a turntablist was taught the bass swap
//! they had been doing for years. Teaching inside a pack is the difference
//! between a catalogue and a curriculum.
//!
//! # Why not thirty packs
//!
//! §16 lists thirty and this ships eight. The eight are the ones whose content
//! djmanzo can actually justify from tables it already has; the rest would be
//! names with invented knowledge behind them, which is worse than an honest
//! gap — a *Karaoke / MC* pack asserting a technique list nobody checked is a
//! curriculum that teaches the wrong thing with confidence. The format is the
//! deliverable; the remaining packs are content, and content is written by
//! somebody who does that kind of night.

use crate::technique::{Difficulty, Kind};

/// One body of DJ knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pack {
    /// The slug it is stored and chosen by.
    pub id: &'static str,
    /// What a DJ would call it.
    pub title: &'static str,
    /// What kind of DJing it is, in one line.
    pub about: &'static str,
    /// The genre families it turns on, by `dj_core::genre` name.
    ///
    /// Empty means *all of them*, which is a real answer for open format and
    /// not a missing value: a DJ who plays everything has no family to centre
    /// on, and a pack that picked one for them would be wrong every other
    /// record.
    pub families: &'static [&'static str],
    /// The kinds of move this pack teaches, by `technique::Kind`.
    pub teaches: &'static [Kind],
    /// The hardest move it will send a DJ to learn.
    ///
    /// §16's *risk tolerance*, at the one place risk is actually taken in
    /// teaching: a learner sent at a flare scratch after two nights stops being
    /// a learner, and a turntablist offered the bass swap stops reading.
    pub ceiling: Difficulty,
    /// The occasion whose presentation this pairs with, by §81's slug.
    ///
    /// A name rather than a copy: §54's setups own the theme, the density, the
    /// pad pages and the columns, and a pack restating them would be the same
    /// decision in two files.
    pub setting: Option<&'static str>,
}

/// The packs that ship.
pub const ALL: [Pack; 8] = [
    Pack {
        id: "general",
        title: "General",
        about: "Everything djmanzo knows, with nothing emphasised.",
        families: &[],
        teaches: &[
            Kind::Blend,
            Kind::Exit,
            Kind::Shape,
            Kind::Perform,
            Kind::Structure,
        ],
        ceiling: Difficulty::Hard,
        setting: None,
    },
    Pack {
        id: "beginner",
        title: "Beginner",
        about: "The moves that work the first time you try them.",
        families: &[],
        // No performance moves: a scratch is not where anybody starts, and a
        // list half of which is out of reach is a list that gets ignored.
        teaches: &[Kind::Blend, Kind::Exit, Kind::Shape],
        ceiling: Difficulty::First,
        setting: Some("practice"),
    },
    Pack {
        id: "open-format",
        title: "Open Format",
        about: "Whatever the room turns out to want, and the moves that cross.",
        families: &[],
        teaches: &[Kind::Blend, Kind::Exit, Kind::Shape, Kind::Structure],
        ceiling: Difficulty::Practised,
        setting: Some("open-format"),
    },
    Pack {
        id: "house",
        title: "House",
        about: "Four to the floor, long blends, and the patience for them.",
        families: &["house", "tech house", "disco", "afro house"],
        teaches: &[Kind::Blend, Kind::Shape, Kind::Structure],
        ceiling: Difficulty::Practised,
        setting: Some("club"),
    },
    Pack {
        id: "techno",
        title: "Techno",
        about: "Long, dark and mechanical. The mix is the arrangement.",
        families: &["techno", "tech house", "hardstyle", "trance"],
        teaches: &[Kind::Blend, Kind::Shape, Kind::Structure],
        ceiling: Difficulty::Practised,
        setting: Some("club"),
    },
    Pack {
        id: "hip-hop",
        title: "Hip-Hop",
        about: "Short records, quick exits, and a hand on the platter.",
        families: &["hip hop", "trap", "r&b", "jersey club"],
        teaches: &[Kind::Exit, Kind::Perform, Kind::Structure],
        ceiling: Difficulty::Hard,
        setting: None,
    },
    Pack {
        id: "latin",
        title: "Latin",
        // Not §81's blurb for the Latin *occasion*, which `dj_app::setting`
        // owns: that describes the room, this describes what djmanzo knows
        // about it, and the two are free to be edited apart.
        about: "Records that turn over on the phrase, and the ear for where.",
        families: &[
            "bachata",
            "merengue",
            "salsa",
            "reggaeton",
            "dembow",
            "cumbia",
            "guaracha",
        ],
        // Structure above all: Latin records turn over on the phrase, and a
        // mix that lands off one is audibly wrong in a way a house mix is not.
        teaches: &[Kind::Structure, Kind::Exit, Kind::Blend],
        ceiling: Difficulty::Practised,
        setting: Some("latin"),
    },
    Pack {
        id: "scratch",
        title: "Scratch / Turntablism",
        about: "The record as an instrument. The hardest table djmanzo has.",
        families: &["hip hop", "trap", "footwork"],
        teaches: &[Kind::Perform, Kind::Exit],
        ceiling: Difficulty::Hard,
        setting: None,
    },
];

/// A pack by its slug.
///
/// `None` rather than a fallback to *General*: a stored pack nobody recognises
/// must not quietly become the one that teaches everything, because the whole
/// point of choosing one is that it teaches less.
#[must_use]
pub fn pack(id: &str) -> Option<&'static Pack> {
    ALL.iter().find(|p| p.id == id)
}

impl Pack {
    /// Whether this pack would teach that move.
    #[must_use]
    pub fn teaches_move(&self, technique: &crate::technique::Technique) -> bool {
        self.teaches.contains(&technique.kind) && technique.difficulty <= self.ceiling
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::technique;

    /// **The load-bearing one: every pack names things djmanzo actually has.**
    ///
    /// A pack is a *selection* from tables that already exist, so a name that
    /// is not in one of them selects nothing — and the failure is silent and
    /// bad: a Latin pack whose families are all misspelled behaves exactly like
    /// open format, and nothing on screen says the pack is doing nothing.
    #[test]
    fn every_pack_names_families_and_settings_that_exist() {
        let families: Vec<&str> = dj_core::genre::families().iter().map(|f| f.name).collect();
        for pack in ALL {
            for family in pack.families {
                assert!(
                    families.contains(family),
                    "{} names the family `{family}`, which dj_core::genre does not have",
                    pack.id
                );
            }
            assert!(
                !pack.teaches.is_empty(),
                "{} teaches nothing at all",
                pack.id
            );
            // Every pack must be able to teach *something*, or choosing it is
            // choosing silence from the coach.
            assert!(
                technique::catalogue().iter().any(|t| pack.teaches_move(t)),
                "{} teaches no move in the catalogue",
                pack.id
            );
        }
    }

    /// **The ids are distinct, and no pack points at a blank occasion.**
    ///
    /// That the slug is one §81 actually has is checked from `dj-app`, in
    /// `setting.rs`, against `Setting::ALL` itself — this crate is below that
    /// one and cannot see the table, and a list of six copied in here to check
    /// against would be the second description the whole design is avoiding.
    #[test]
    fn the_packs_are_distinct_and_name_an_occasion_or_none() {
        let mut ids: Vec<&str> = ALL.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len(), "two packs share an id");

        for pack in ALL {
            if let Some(setting) = pack.setting {
                assert!(
                    !setting.trim().is_empty(),
                    "{} pairs with an empty occasion, which `None` is for",
                    pack.id
                );
            }
        }
    }

    /// **A beginner is never sent at a hard move, and a turntablist is never
    /// sent at an easy one they have done for years.**
    ///
    /// The two ends of §16's *risk tolerance*, at the one place teaching
    /// actually takes a risk. Both directions, because a ceiling that only held
    /// at the top would let the scratch pack teach the bass swap forever.
    #[test]
    fn a_pack_teaches_inside_its_own_range() {
        let beginner = pack("beginner").expect("the beginner pack ships");
        let scratch = pack("scratch").expect("the scratch pack ships");

        for t in technique::catalogue() {
            if beginner.teaches_move(t) {
                assert_eq!(
                    t.difficulty,
                    technique::Difficulty::First,
                    "the beginner pack offers `{}`, which is not a first-try move",
                    t.name
                );
            }
            if scratch.teaches_move(t) {
                assert!(
                    matches!(t.kind, Kind::Perform | Kind::Exit),
                    "the scratch pack offers `{}`, which is neither a performance \
                     move nor an exit",
                    t.name
                );
            }
        }
        // And each teaches something, or the ceiling has closed the pack.
        assert!(
            technique::catalogue()
                .iter()
                .any(|t| beginner.teaches_move(t))
        );
        assert!(
            technique::catalogue()
                .iter()
                .any(|t| scratch.teaches_move(t))
        );
    }

    /// Every Svelte file in the interface, with line endings normalised.
    ///
    /// The `\r\n` is not paranoia: a scan of source text passed on every
    /// machine here and failed only on Windows CI, where git checks the
    /// repository out with CRLF.
    fn interface() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src");
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "svelte") {
                    let text = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    found.push((name, text.replace("\r\n", "\n")));
                }
            }
        }
        assert!(
            found.len() > 20,
            "the interface scan found only {} files, so it is looking in the \
             wrong place and would pass whatever the interface said",
            found.len()
        );
        found
    }

    /// **The load-bearing one: no part of the interface names a genre family.**
    ///
    /// §16 ends *"Do not hard-code this logic into UI components"*, and the
    /// genre map is the table it most tempts an interface to restate — a pack
    /// picker wanting to show what House turns on has the list right there, and
    /// writing it out is one line. Then `dj_core::genre` gains a family, the
    /// picker does not, and the interface is quietly describing a different
    /// pack from the one the coach teaches out of.
    ///
    /// The scan is for a family name as a *string literal*, which is where the
    /// copy would live. Prose is left alone on purpose: a comment explaining
    /// that bachata turns over on the phrase is documentation, and a search
    /// placeholder reading "bachata with a piano hook" is an example of what to
    /// type. Neither is a second copy of the table, and a test that failed on
    /// them would be turned off within a week.
    #[test]
    fn no_part_of_the_interface_writes_the_genre_map_out_for_itself() {
        let families: Vec<&str> = dj_core::genre::families().iter().map(|f| f.name).collect();
        for (name, source) in interface() {
            for family in &families {
                for quoted in [
                    format!("\"{family}\""),
                    format!("'{family}'"),
                    format!("`{family}`"),
                ] {
                    assert!(
                        !source.to_lowercase().contains(&quoted),
                        "{name} writes the genre family {quoted} out for itself. \
                         §16: do not hard-code this logic into UI components — \
                         read the families off a pack instead, so the interface \
                         cannot describe a map dj_core no longer has"
                    );
                }
            }
        }
    }

    /// **And the packs themselves are read, not restated.**
    ///
    /// The other half of the same rule, pointed at this table rather than at
    /// the genre map. A picker that spelled its own titles and blurbs would
    /// show eight packs whatever this file said, which is the failure mode
    /// that is hardest to see: everything renders, nothing is missing, and the
    /// list is simply not the one being taught from.
    #[test]
    fn the_picker_reads_the_packs_rather_than_spelling_them() {
        let interface = interface();
        let settings = interface
            .iter()
            .find(|(name, _)| name == "Settings.svelte")
            .map(|(_, source)| source.clone())
            .expect("the settings surface ships");
        assert!(
            settings.contains("knowledgePacks()"),
            "the settings surface no longer asks Rust for the packs, so whatever \
             it shows is its own"
        );
        // The blurbs rather than the titles: a title is one word and would
        // collide with ordinary interface text, but no file has a reason to
        // contain a whole sentence this table wrote.
        for pack in ALL {
            for (name, source) in &interface {
                assert!(
                    !source.contains(pack.about),
                    "{name} spells `{}`'s own description, which is this table's \
                     to change",
                    pack.id
                );
            }
        }
    }
}

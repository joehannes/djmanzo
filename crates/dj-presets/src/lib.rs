//! Presets: named sets of actions, applied as one.
//!
//! A preset is an ordered list of action strings and nothing more. That falls
//! out of [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md):
//! every intent is already an `Action` on one bus, so a saved configuration is
//! just several of them. Which means a preset is **data** — diffable, shareable
//! as a file, reviewable — and applying one is indistinguishable from a very
//! fast pair of hands.
//!
//! It also means nothing a preset does is special. Everything it sets, you can
//! change back with the same control, and everything it did appears in the
//! session log exactly like a hand-played action.
//!
//! # What is honest about this today
//!
//! These presets set **mixer and deck state**: EQ, filter, keylock, faders,
//! transport. That is what the action vocabulary currently expresses.
//!
//! The session phases below (`warm up`, `fiesta`, `peak`, …) therefore prepare
//! the *desk* for a phase; they do not yet steer tempo, genre or energy,
//! because doing that needs beatgrids, keys and play history — M2 and M3. The
//! names are here now so the workflow exists and the packs can grow into it,
//! rather than the concept arriving late and having to be retrofitted.
//!
//! # Layering
//!
//! Presets apply in order and later actions win, so a pack can set a baseline
//! and a preset within it override part of that. Whatever the DJ touches
//! afterwards wins over both, because it is the same bus.

pub mod builtin;

use dj_core::Action;
use serde::{Deserialize, Serialize};

/// What part of the application a preset belongs to.
///
/// Used for grouping in the interface. A closed set so the panel and the packs
/// cannot disagree about what categories exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Where you are in the night.
    Phase,
    /// How the desk is set up before a track goes out.
    Prep,
    /// A move made during a mix.
    Move,
    /// Tone shaping.
    Eq,
    /// Whole-mixer state.
    Mixer,
}

impl Category {
    #[must_use]
    pub const fn all() -> &'static [Category] {
        use Category::*;
        &[Phase, Prep, Move, Eq, Mixer]
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Category::Phase => "Session phase",
            Category::Prep => "Preparation",
            Category::Move => "Mix move",
            Category::Eq => "EQ",
            Category::Mixer => "Mixer",
        }
    }
}

/// One named set of actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// What it does and when to reach for it.
    pub description: String,
    pub category: Category,
    /// Action text, applied in order. `{deck}` is substituted when the preset
    /// is applied to a particular deck.
    pub actions: Vec<String>,
    /// True when the preset is written per-deck and needs one to be chosen.
    #[serde(default)]
    pub per_deck: bool,
}

impl Preset {
    /// The actions this preset would run, resolved for `deck`.
    ///
    /// # Errors
    /// If any action does not parse. Checked here rather than at dispatch, so a
    /// bad preset is refused as a whole rather than half-applied — leaving the
    /// desk in a state that is neither where it was nor where it was going is
    /// the worst outcome available.
    pub fn resolve(&self, deck: u8) -> Result<Vec<Action>, PresetError> {
        let mut resolved = Vec::with_capacity(self.actions.len());
        for template in &self.actions {
            let text = template.replace("{deck}", &deck.to_string());
            let action = Action::parse(&text).map_err(|error| PresetError::BadAction {
                preset: self.id.clone(),
                text,
                reason: error.to_string(),
            })?;
            resolved.push(action);
        }
        Ok(resolved)
    }

    /// Whether every action parses, for a representative deck.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.resolve(1).is_ok()
    }
}

/// A named group of presets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub presets: Vec<Preset>,
    /// False for packs shipped with the application, true for the user's own.
    #[serde(default)]
    pub user: bool,
}

impl Pack {
    #[must_use]
    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("preset `{preset}` contains `{text}`, which is not a valid action: {reason}")]
    BadAction {
        preset: String,
        text: String,
        reason: String,
    },
    #[error("no preset called `{0}`")]
    NotFound(String),
    #[error("could not read presets: {0}")]
    Io(String),
    #[error("that pack file is not valid: {0}")]
    Malformed(String),
}

/// Every pack available, built-in plus whatever the user has added.
#[derive(Debug, Default)]
pub struct PresetLibrary {
    packs: Vec<Pack>,
}

impl PresetLibrary {
    /// The packs shipped with the application.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            packs: builtin::packs(),
        }
    }

    /// Add packs from a directory of JSON files.
    ///
    /// A malformed file is logged and skipped rather than failing the load:
    /// one bad file the user was editing should not take the other nine with
    /// it, least of all at the start of a set.
    pub fn load_dir(&mut self, dir: &std::path::Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut added = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path)
                .map_err(|e| PresetError::Io(e.to_string()))
                .and_then(|text| {
                    serde_json::from_str::<Pack>(&text)
                        .map_err(|e| PresetError::Malformed(e.to_string()))
                }) {
                Ok(mut pack) => {
                    pack.user = true;
                    // Refuse a pack containing an action that does not parse,
                    // rather than discovering it when someone presses the
                    // button mid-set.
                    if let Some(bad) = pack.presets.iter().find(|p| !p.is_valid()) {
                        tracing::warn!(
                            path = %path.display(),
                            preset = %bad.id,
                            "skipping a pack with an invalid preset"
                        );
                        continue;
                    }
                    self.packs.retain(|existing| existing.id != pack.id);
                    self.packs.push(pack);
                    added += 1;
                }
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping an unreadable pack");
                }
            }
        }
        added
    }

    #[must_use]
    pub fn packs(&self) -> &[Pack] {
        &self.packs
    }

    /// Find a preset by its id, across every pack.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Preset> {
        self.packs.iter().find_map(|pack| pack.preset(id))
    }

    /// Resolve a preset into actions, ready to dispatch.
    ///
    /// # Errors
    /// If the preset does not exist, or any of its actions do not parse.
    pub fn resolve(&self, id: &str, deck: u8) -> Result<Vec<Action>, PresetError> {
        self.find(id)
            .ok_or_else(|| PresetError::NotFound(id.to_owned()))?
            .resolve(deck)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.packs.iter().map(|p| p.presets.len()).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(actions: &[&str]) -> Preset {
        Preset {
            id: "test".into(),
            name: "Test".into(),
            description: "A test".into(),
            category: Category::Move,
            actions: actions.iter().map(|s| (*s).to_owned()).collect(),
            per_deck: true,
        }
    }

    #[test]
    fn a_preset_resolves_its_deck_placeholder() {
        let actions = preset(&["deck {deck} play", "deck {deck} eq_low 0"])
            .resolve(2)
            .unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].to_string(), "deck 2 play");
        assert_eq!(actions[1].to_string(), "deck 2 eq_low 0");
    }

    /// A preset must be refused whole rather than applied half-way. Leaving the
    /// desk neither where it was nor where it was going is the worst outcome.
    #[test]
    fn one_bad_action_refuses_the_whole_preset() {
        let bad = preset(&["deck {deck} play", "deck {deck} levitate", "crossfader 0"]);
        let error = bad.resolve(1).unwrap_err();
        assert!(matches!(error, PresetError::BadAction { .. }));
        assert!(error.to_string().contains("levitate"));
        assert!(!bad.is_valid());
    }

    #[test]
    fn presets_without_a_placeholder_ignore_the_deck() {
        let actions = preset(&["crossfader 0"]).resolve(3).unwrap();
        assert_eq!(actions[0].to_string(), "crossfader 0");
    }

    #[test]
    fn the_builtin_library_is_not_empty_and_every_preset_parses() {
        let library = PresetLibrary::builtin();
        assert!(library.len() > 10, "only {} presets", library.len());
        for pack in library.packs() {
            for preset in &pack.presets {
                for deck in 1..=4u8 {
                    preset.resolve(deck).unwrap_or_else(|e| {
                        panic!("built-in preset `{}` is broken: {e}", preset.id)
                    });
                }
            }
        }
    }

    #[test]
    fn preset_ids_are_unique_across_packs() {
        use std::collections::HashSet;
        let library = PresetLibrary::builtin();
        let mut seen = HashSet::new();
        for pack in library.packs() {
            for preset in &pack.presets {
                assert!(seen.insert(preset.id.clone()), "duplicate id {}", preset.id);
            }
        }
    }

    #[test]
    fn every_builtin_preset_explains_itself() {
        for pack in PresetLibrary::builtin().packs() {
            for preset in &pack.presets {
                assert!(!preset.name.is_empty(), "{}", preset.id);
                assert!(
                    preset.description.len() > 15,
                    "`{}` has a stub description",
                    preset.id
                );
                assert!(!preset.actions.is_empty(), "{} does nothing", preset.id);
            }
        }
    }

    #[test]
    fn finding_a_preset_that_does_not_exist_says_so() {
        let library = PresetLibrary::builtin();
        assert!(matches!(
            library.resolve("nonsense", 1),
            Err(PresetError::NotFound(_))
        ));
    }

    /// Every session phase the roadmap names should exist, so the workflow is
    /// complete even while the musical steering behind it is not.
    #[test]
    fn every_session_phase_is_present() {
        let library = PresetLibrary::builtin();
        for phase in [
            "phase-warmup",
            "phase-fiesta",
            "phase-peak",
            "phase-slowdown",
            "phase-chillout",
            "phase-close",
        ] {
            assert!(library.find(phase).is_some(), "{phase} missing");
        }
    }

    // -- user packs --------------------------------------------------------

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("djmanzo-presets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_user_pack_loads_from_disk() {
        let dir = temp_dir("load");
        std::fs::write(
            dir.join("mine.json"),
            r#"{
                "id": "mine",
                "name": "Mine",
                "description": "My own",
                "presets": [{
                    "id": "mine-kill",
                    "name": "Kill it",
                    "description": "Everything out on this deck at once",
                    "category": "move",
                    "actions": ["deck {deck} eq_low 0", "deck {deck} volume 0"],
                    "per_deck": true
                }]
            }"#,
        )
        .unwrap();

        let mut library = PresetLibrary::builtin();
        assert_eq!(library.load_dir(&dir), 1);
        let preset = library.find("mine-kill").expect("user preset should load");
        assert_eq!(preset.resolve(2).unwrap()[0].to_string(), "deck 2 eq_low 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One file the user was halfway through editing must not take the others
    /// with it, least of all at the start of a set.
    #[test]
    fn a_broken_pack_is_skipped_rather_than_failing_the_load() {
        let dir = temp_dir("broken");
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        std::fs::write(
            dir.join("good.json"),
            r#"{"id":"g","name":"G","description":"d","presets":[]}"#,
        )
        .unwrap();

        let mut library = PresetLibrary::builtin();
        let before = library.packs().len();
        assert_eq!(library.load_dir(&dir), 1, "the good pack should still load");
        assert_eq!(library.packs().len(), before + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pack whose actions do not parse is refused at load, not when someone
    /// presses the button mid-set.
    #[test]
    fn a_pack_with_an_invalid_action_is_refused_at_load() {
        let dir = temp_dir("invalid");
        std::fs::write(
            dir.join("bad.json"),
            r#"{
                "id": "bad", "name": "Bad", "description": "d",
                "presets": [{
                    "id": "bad-1", "name": "Bad", "description": "does not parse",
                    "category": "move", "actions": ["deck {deck} teleport"]
                }]
            }"#,
        )
        .unwrap();

        let mut library = PresetLibrary::builtin();
        assert_eq!(library.load_dir(&dir), 0);
        assert!(library.find("bad-1").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_user_pack_replaces_a_built_in_one_with_the_same_id() {
        let dir = temp_dir("override");
        let builtin_id = &PresetLibrary::builtin().packs()[0].id.clone();
        std::fs::write(
            dir.join("o.json"),
            format!(
                r#"{{"id":"{builtin_id}","name":"Mine","description":"replaced","presets":[]}}"#
            ),
        )
        .unwrap();

        let mut library = PresetLibrary::builtin();
        let before = library.packs().len();
        library.load_dir(&dir);
        assert_eq!(library.packs().len(), before, "the pack should be replaced");
        let replaced = library
            .packs()
            .iter()
            .find(|p| &p.id == builtin_id)
            .unwrap();
        assert_eq!(replaced.name, "Mine");
        assert!(replaced.user);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let mut library = PresetLibrary::builtin();
        assert_eq!(library.load_dir(std::path::Path::new("/nowhere")), 0);
    }
}

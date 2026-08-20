//! Commands for preset packs.
//!
//! Applying a preset dispatches its actions onto the same bus everything else
//! uses, so there is nothing privileged about one: it lands in the session log
//! action by action, replays like any other, and every setting it touched can
//! be changed back with the control that owns it.
//!
//! The applied actions are returned so the interface can show exactly what
//! happened. A preset that silently changes eight things is the kind of feature
//! people stop trusting.

use crate::state::AppState;
use dj_presets::{Category, Pack};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct PresetDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: &'static str,
    pub per_deck: bool,
    /// The actions it would run, for display. Shown so nothing is hidden.
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub user: bool,
    pub presets: Vec<PresetDto>,
}

fn category_slug(category: Category) -> &'static str {
    match category {
        Category::Phase => "phase",
        Category::Prep => "prep",
        Category::Move => "move",
        Category::Eq => "eq",
        Category::Mixer => "mixer",
    }
}

fn to_dto(pack: &Pack) -> PackDto {
    PackDto {
        id: pack.id.clone(),
        name: pack.name.clone(),
        description: pack.description.clone(),
        user: pack.user,
        presets: pack
            .presets
            .iter()
            .map(|preset| PresetDto {
                id: preset.id.clone(),
                name: preset.name.clone(),
                description: preset.description.clone(),
                category: category_slug(preset.category),
                per_deck: preset.per_deck,
                actions: preset.actions.clone(),
            })
            .collect(),
    }
}

/// Every pack, built-in and the user's own.
#[tauri::command]
pub fn list_presets(state: State<'_, AppState>) -> Vec<PackDto> {
    state.presets().packs().iter().map(to_dto).collect()
}

/// Apply a preset, returning what was dispatched.
#[tauri::command]
pub fn apply_preset(
    state: State<'_, AppState>,
    id: String,
    deck: Option<u8>,
) -> Result<Vec<String>, String> {
    // Resolve everything before dispatching anything. A preset half-applied
    // leaves the desk neither where it was nor where it was going, which is
    // worse than not applying it at all.
    let actions = state
        .presets()
        .resolve(&id, deck.unwrap_or(1))
        .map_err(|e| e.to_string())?;

    let mut applied = Vec::with_capacity(actions.len());
    for action in actions {
        let text = action.to_string();
        state
            .bus()
            .dispatch(action)
            .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())?;
        applied.push(text);
    }
    Ok(applied)
}

/// Where user packs are read from, so the panel can say so.
#[tauri::command]
pub fn preset_folder(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("presets")
        .to_string_lossy()
        .to_string())
}

/// Save the rack as it stands now as a preset the DJ can get back.
///
/// The other direction from [`apply_preset`], and the half that was missing: a
/// preset is already action text, and the rack is already reachable from the
/// action vocabulary, so an effect chain needed no new kind of preset — only a
/// way to read one out. See [`crate::rackcapture`].
///
/// Writes into the user pack, which the library reads on the next refresh, so
/// nothing here has to touch the in-memory library.
#[tauri::command]
pub fn save_rack_preset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
    deck: Option<u8>,
) -> Result<String, String> {
    use tauri::Manager;

    let name = name.trim();
    if name.is_empty() {
        return Err("a preset needs a name".to_owned());
    }

    let rack = match deck {
        Some(number) => crate::rackcapture::Rack::Deck(
            dj_core::DeckId::from_human(number).ok_or("no such deck")?,
        ),
        None => crate::rackcapture::Rack::Master,
    };
    let actions = crate::rackcapture::capture(&state.registry(), rack);
    if actions.is_empty() {
        return Err("there is nothing in that rack to save".to_owned());
    }

    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("presets");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    let path = dir.join("mine.json");
    // Read, add, write. The user's own pack is one file so that a DJ can copy
    // it between machines as one file.
    let mut pack: dj_presets::Pack = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| dj_presets::Pack {
            id: "mine".to_owned(),
            name: "Mine".to_owned(),
            description: "Chains and moves you saved.".to_owned(),
            presets: Vec::new(),
            user: true,
        });

    let id = format!(
        "mine-{}",
        name.to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
    );
    let preset = dj_presets::Preset {
        id: id.clone(),
        name: name.to_owned(),
        description: match deck {
            Some(_) => "An effect chain you saved off a deck.".to_owned(),
            None => "A master effect chain you saved.".to_owned(),
        },
        category: dj_presets::Category::Move,
        actions,
        per_deck: deck.is_some(),
    };

    // Saving twice under one name replaces rather than duplicating: the second
    // save is a correction, not a second preset.
    pack.presets.retain(|existing| existing.id != id);
    pack.presets.push(preset);

    let text = serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_has_a_slug() {
        use std::collections::HashSet;
        let slugs: HashSet<&str> = Category::all().iter().map(|c| category_slug(*c)).collect();
        assert_eq!(slugs.len(), Category::all().len(), "duplicate slug");
    }

    /// The interface renders these, so a pack with no presets or a preset with
    /// no actions would draw an empty, unexplained box.
    #[test]
    fn the_builtin_packs_convert_with_everything_populated() {
        let library = dj_presets::PresetLibrary::builtin();
        for pack in library.packs() {
            let dto = to_dto(pack);
            assert!(!dto.presets.is_empty(), "{} has no presets", dto.id);
            for preset in &dto.presets {
                assert!(!preset.actions.is_empty(), "{} does nothing", preset.id);
                assert!(!preset.name.is_empty());
            }
        }
    }
}

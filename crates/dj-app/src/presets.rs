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

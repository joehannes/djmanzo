//! §69: practising on a record live, in the headphones, with the room never
//! hearing it.
//!
//! > Two tracks can be explored without altering the live master. The user
//! > can: […] experiment with stems, test FX, create loops […]
//!
//! [`crate::practice`] already rehearses a *transition* — rendered to a file
//! by the real automix and heard back. What it could not do was the rest of
//! §69's list **live**: put a record somewhere private and loop it, mute its
//! drums, throw an echo on it, while a set goes on. That is a deck's work, so
//! the engine has a fifth deck for it — never one of the four: no crossfader
//! channel, no sync, no parameter row, and processed with a layout whose only
//! pair is the headphones' (`dj_engine::Practice`). A machine with no
//! headphone pair hears nothing from it, which is the right failure.
//!
//! What is done to it is said in the deck vocabulary a controller already
//! uses — `play`, `loop 4`, `stem_mute drums`, `fx 1 echo` — parsed by the
//! same parser, so practising a move is practising *the* move. What only a
//! deck the room hears can do (sync, the crossfader's side, eject, the grid's
//! edits, saved loops) is refused by name.

use std::sync::{Arc, Mutex, OnceLock};

use dj_core::action::DeckAction;
use dj_core::param::{GlobalParam, ParamId};
use serde::Serialize;
use tauri::State;

use crate::AppState;

/// The record on the practice deck, as the panel names it.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    id: dj_core::TrackId,
    title: String,
    frames: usize,
    rate: f64,
}

fn loaded() -> &'static Mutex<Option<Loaded>> {
    static LOADED: OnceLock<Mutex<Option<Loaded>>> = OnceLock::new();
    LOADED.get_or_init(|| Mutex::new(None))
}

/// What the practice deck is doing, for the panel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PracticeLiveDto {
    /// The record on it, by id, or none.
    pub track: Option<String>,
    pub title: String,
    pub playing: bool,
    pub looping: bool,
    pub position_seconds: f64,
    pub length_seconds: f64,
    /// Its peak as the headphones get it, 0 to 1.
    pub level: f32,
    /// Whether there is anywhere private to hear it: without a headphone pair
    /// the practice deck is silent, and the panel says so.
    pub headphones: bool,
}

/// What is said to the practice deck, as a deck action — or why not.
///
/// # Errors
/// A sentence: the words are not a deck action, or they are one only a deck
/// the room hears can do.
pub fn action_of(words: &str) -> Result<DeckAction, String> {
    let words = words.trim();
    let parsed = dj_core::action::Action::parse(&format!("deck 1 {words}"))
        .map_err(|error| format!("{words:?} is not something a deck does: {error}"))?;
    let dj_core::Action::Deck { action, .. } = parsed else {
        return Err(format!("{words:?} is not something a deck does"));
    };
    match action {
        DeckAction::Sync
        | DeckAction::SyncOff
        | DeckAction::SyncToggle
        | DeckAction::SetCrossfaderAssign(_) => {
            Err("the practice deck plays on its own: nothing to sync to or cross-fade".to_owned())
        }
        DeckAction::Eject => Err("take a record off the practice deck with its own button".into()),
        DeckAction::GridAnchorHere
        | DeckAction::GridNudge(_)
        | DeckAction::GridScale(_)
        | DeckAction::GridSetBpm(_)
        | DeckAction::GridTap
        | DeckAction::GridReset
        | DeckAction::GridPhrase(_)
        | DeckAction::LoopSave(_)
        | DeckAction::LoopRecall(_) => {
            Err("the grid and saved loops are edited on a deck, where they are kept".into())
        }
        other => Ok(other),
    }
}

/// The panel's view, from the engine's registry and what was loaded.
#[must_use]
fn view(registry: &dj_control::ParameterRegistry, on: Option<&Loaded>) -> PracticeLiveDto {
    let get = |param| registry.get(ParamId::Global(param));
    let rate = on.map_or(0.0, |loaded| loaded.rate);
    let seconds = |frames: f64| if rate > 0.0 { frames / rate } else { 0.0 };
    PracticeLiveDto {
        track: on.map(|loaded| loaded.id.to_hex()),
        title: on.map(|loaded| loaded.title.clone()).unwrap_or_default(),
        playing: on.is_some() && get(GlobalParam::PracticePlaying) >= 0.5,
        looping: on.is_some() && get(GlobalParam::PracticeLooping) >= 0.5,
        position_seconds: if on.is_some() {
            seconds(f64::from(get(GlobalParam::PracticePosition)))
        } else {
            0.0
        },
        #[allow(clippy::cast_precision_loss)]
        length_seconds: on.map_or(0.0, |loaded| seconds(loaded.frames as f64)),
        level: if on.is_some() {
            get(GlobalParam::PracticeLevel)
        } else {
            0.0
        },
        headphones: get(GlobalParam::CueAvailable) >= 0.5,
    }
}

fn now(state: &AppState) -> PracticeLiveDto {
    let on = loaded().lock().ok().and_then(|held| held.clone());
    view(&state.registry(), on.as_ref())
}

fn send(state: &AppState, order: dj_engine::Practice) -> Result<(), String> {
    state
        .bus()
        .send_command(dj_engine::Command::Practice(order))
        .map_err(|_| "the engine is not accepting commands; is a device open?".to_owned())
}

/// What the practice deck is doing.
#[tauri::command]
#[must_use]
pub fn practice_live(state: State<'_, AppState>) -> PracticeLiveDto {
    now(&state)
}

/// Put a record from the collection on the practice deck, with its grid when
/// the analyser has one, and separate its stems outward from its playhead.
///
/// # Errors
/// A sentence: no such record, its file unreadable, or no engine.
#[tauri::command]
pub async fn practice_live_load(
    state: State<'_, AppState>,
    track: String,
) -> Result<PracticeLiveDto, String> {
    let id = dj_core::TrackId::from_hex(track.trim())
        .ok_or_else(|| format!("not a track id: {:?}", track.trim()))?;
    let db = crate::commands::library(&state)?;
    let found = db
        .track(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no track {} in the library", id.to_hex()))?;
    let path = found.path.clone();
    let decoded = tauri::async_runtime::spawn_blocking(move || dj_decode::decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let rate = decoded.buffer.sample_rate();
    let frames = decoded.buffer.len_frames();
    let buffer = Arc::new(decoded.buffer);
    let cached = state.analysis().cached(&id);
    let (grid, phrase) = grid_of(cached.as_deref());
    send(
        &state,
        dj_engine::Practice::Load {
            source: buffer.clone(),
            grid,
            phrase,
        },
    )?;
    if let Ok(mut held) = loaded().lock() {
        *held = Some(Loaded {
            id,
            title: title_of(&found),
            frames,
            rate: rate.as_f64(),
        });
    }

    // Stems, outward from the practice deck's own playhead.
    let registry = state.registry();
    crate::commands::separate_in_background(&state, &buffer, id, rate, move || {
        f64::from(registry.get(ParamId::Global(GlobalParam::PracticePosition)))
    });

    // A record nobody has analysed gets its grid when the analyser is done,
    // for loops in beats — and only if it is still the one on the deck.
    if cached.is_none() {
        let bus = Arc::clone(state.bus());
        let audio = Arc::clone(&buffer);
        tauri::async_runtime::spawn_blocking(move || {
            let analysis = dj_analysis::analyse(audio.as_interleaved(), rate);
            let still = loaded()
                .lock()
                .ok()
                .is_some_and(|held| held.as_ref().is_some_and(|on| on.id == id));
            if still {
                let (grid, phrase) = grid_of(Some(&analysis));
                let _ = bus.send_command(dj_engine::Command::Practice(dj_engine::Practice::Grid {
                    grid,
                    phrase,
                }));
            }
        });
    }
    Ok(now(&state))
}

/// A record as the panel names it: *artist – title*, the title alone, or the
/// file's name when its tags say neither.
fn title_of(track: &dj_library::LibraryTrack) -> String {
    let file = || {
        track
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let title = track.tags.title.clone().filter(|t| !t.is_empty());
    match (
        track.tags.artist.as_deref().filter(|a| !a.is_empty()),
        title,
    ) {
        (Some(artist), Some(title)) => format!("{artist} – {title}"),
        (None, Some(title)) => title,
        (_, None) => file(),
    }
}

/// The beat grid and phrase an analysis found, as the engine takes them.
fn grid_of(
    analysis: Option<&dj_analysis::Analysis>,
) -> (Option<dj_core::Beatgrid>, Option<dj_core::Phrase>) {
    let Some(analysis) = analysis else {
        return (None, None);
    };
    (
        analysis.tempo.as_ref().map(|tempo| tempo.grid),
        analysis
            .phrases
            .and_then(|p| dj_core::Phrase::new(p.beats, p.anchor)),
    )
}

/// Do something to the practice deck, in the deck vocabulary: `play`,
/// `loop 4`, `stem_mute drums`, `fx 1 echo`.
///
/// # Errors
/// The words are not a deck action, or one only a deck the room hears does,
/// or there is no engine.
#[tauri::command]
pub fn practice_live_do(
    state: State<'_, AppState>,
    action: String,
) -> Result<PracticeLiveDto, String> {
    let action = action_of(&action)?;
    send(&state, dj_engine::Practice::Act(action))?;
    Ok(now(&state))
}

/// Take the record off the practice deck.
///
/// # Errors
/// No engine.
#[tauri::command]
pub fn practice_live_eject(state: State<'_, AppState>) -> Result<PracticeLiveDto, String> {
    send(&state, dj_engine::Practice::Eject)?;
    if let Ok(mut held) = loaded().lock() {
        *held = None;
    }
    Ok(now(&state))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The deck's own words, and only what a private deck can do.** Play,
    /// a loop, a stem, an effect parse as they do for a deck; sync, the
    /// crossfader, eject and the grid's edits are refused with a reason.
    #[test]
    fn the_practice_deck_takes_a_decks_words_and_refuses_the_rooms() {
        assert_eq!(action_of("play"), Ok(DeckAction::Play));
        assert_eq!(action_of(" loop 4 "), Ok(DeckAction::LoopBeats(4.0)));
        assert!(matches!(
            action_of("stem_mute drums"),
            Ok(DeckAction::Stem { .. })
        ));
        assert!(matches!(action_of("fx 1 echo"), Ok(DeckAction::Fx { .. })));
        for refused in ["sync", "xfader_left", "eject", "grid_tap", "nonsense"] {
            assert!(action_of(refused).is_err(), "{refused} was taken");
        }
    }

    /// **The panel's view says nothing about a deck with nothing on it**, and
    /// turns the engine's frames into the record's seconds.
    #[test]
    fn the_view_reads_the_engine_against_the_record_loaded() {
        let registry = dj_control::ParameterRegistry::new();
        registry.set(ParamId::Global(GlobalParam::PracticePlaying), 1.0);
        registry.set(ParamId::Global(GlobalParam::PracticePosition), 96_000.0);
        registry.set(ParamId::Global(GlobalParam::CueAvailable), 1.0);
        let empty = view(&registry, None);
        assert!(!empty.playing && empty.track.is_none());
        assert!(empty.headphones);

        let on = Loaded {
            id: dj_core::TrackId::from_hex(&"ab".repeat(32)).expect("an id"),
            title: "Aventura – Obsesión".into(),
            frames: 48_000 * 240,
            rate: 48_000.0,
        };
        let seen = view(&registry, Some(&on));
        assert!(seen.playing);
        assert!((seen.position_seconds - 2.0).abs() < 1e-9);
        assert!((seen.length_seconds - 240.0).abs() < 1e-9);
        assert_eq!(seen.title, "Aventura – Obsesión");
    }
}

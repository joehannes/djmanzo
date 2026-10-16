//! §127: a playlist for each deck.
//!
//! > also enable a stylish GUI extension at the bottom of each
//! > turntable/mixer widget, that on click extends to a playlist for that
//! > mixer. ... the one configured as karaoke mixer, the other one with
//! > default jingles/playlist items that can be repeated as fillers
//!
//! # What a deck's queue is
//!
//! One of the DJ's playlists, chosen for one deck, and where that deck is
//! in it: [`DeckQueue::at`] is the record the queue last put on the deck,
//! by its place in the list. A press in the drawer under the deck loads any
//! of them; [`next`] is the one after.
//!
//! # A deck that goes round by itself
//!
//! With [`DeckQueue::advance`] on, a deck that **runs out** — it was
//! playing, it stopped, and it stopped at its end ([`ran_out`]) — is given
//! the queue's next record and started. With [`DeckQueue::repeat`] on, the
//! end of the list starts it over. That is a filler deck: jingles and
//! fillers going round while the other deck carries the singers. A deck the
//! DJ pauses mid-record has not run out, so nothing is loaded over it.
//!
//! Kept in `deck_queue.json` beside the other settings, so the filler deck
//! is still the filler deck tomorrow. Read on the snapshot pump, as break
//! music is; the record is decoded off it.

use crate::commands::{LibraryTrackDto, PlaylistEntryDto};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// One deck's queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckQueue {
    /// The playlist chosen for the deck, if any.
    pub playlist: Option<i64>,
    /// The record the queue last put on the deck, by its place in the list.
    pub at: Option<usize>,
    /// When the deck runs out, load the next and play it.
    pub advance: bool,
    /// After the last, the first again.
    pub repeat: bool,
}

/// The record after `at` in a list of `len`: the first when the queue has
/// put nothing on the deck yet, the first again after the last only with
/// `repeat`, and nothing in an empty list.
#[must_use]
pub fn next(len: usize, at: Option<usize>, repeat: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match at {
        None => Some(0),
        Some(index) if index + 1 < len => Some(index + 1),
        Some(_) if repeat => Some(0),
        Some(_) => None,
    }
}

/// What the queue needs to know of a deck, from the snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckView {
    pub loaded: bool,
    pub playing: bool,
    pub position_seconds: f32,
    pub length_seconds: f32,
}

/// How near its end a deck that stops counts as having run out, in
/// seconds: the engine stops a record at its last frame, and a frame of the
/// pump is a sixtieth of a second behind it.
pub const END_SECONDS: f32 = 0.5;

/// A deck that was playing and is not, stopped at its end — not paused by a
/// hand in the middle, and not a deck that never had anything on it.
#[must_use]
pub fn ran_out(was_playing: bool, now: &DeckView) -> bool {
    was_playing
        && !now.playing
        && now.loaded
        && now.length_seconds > 0.0
        && now.position_seconds >= now.length_seconds - END_SECONDS
}

/// Every deck's queue, and what the pump saw of each last frame.
#[derive(Debug, Default)]
pub struct Queues {
    pub decks: [DeckQueue; dj_core::MAX_DECKS],
    was_playing: [bool; dj_core::MAX_DECKS],
}

impl Queues {
    /// One frame: the decks, by index from 0, whose queue should give them
    /// their next record now.
    pub fn tick(&mut self, views: &[DeckView]) -> Vec<usize> {
        let mut due = Vec::new();
        for (index, view) in views.iter().enumerate().take(dj_core::MAX_DECKS) {
            let was = std::mem::replace(&mut self.was_playing[index], view.playing);
            let queue = &self.decks[index];
            if queue.advance && queue.playlist.is_some() && ran_out(was, view) {
                due.push(index);
            }
        }
        due
    }
}

fn queues(state: &AppState) -> MutexGuard<'static, Queues> {
    static QUEUES: OnceLock<Mutex<Queues>> = OnceLock::new();
    QUEUES
        .get_or_init(|| {
            let mut held = Queues::default();
            let kept: Vec<DeckQueue> = path(state)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            for (slot, queue) in held.decks.iter_mut().zip(kept) {
                *slot = queue;
            }
            Mutex::new(held)
        })
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn path(state: &AppState) -> Option<std::path::PathBuf> {
    Some(state.config_dir()?.join("deck_queue.json"))
}

fn keep(state: &AppState, held: &Queues) {
    let Some(path) = path(state) else { return };
    let Ok(text) = serde_json::to_string_pretty(&held.decks) else {
        return;
    };
    if let Err(error) = std::fs::write(&path, text) {
        tracing::warn!(%error, ?path, "the decks' playlists will not survive a restart");
    }
}

fn index_of(deck: u8) -> Result<usize, String> {
    dj_core::DeckId::from_human(deck)
        .map(dj_core::DeckId::index)
        .ok_or_else(|| format!("there is no deck {deck}"))
}

/// A deck's queue, as the drawer draws it.
#[derive(Debug, Clone, Serialize)]
pub struct DeckQueueDto {
    pub deck: u8,
    pub playlist: Option<i64>,
    /// The playlist's name, or `None` when none is chosen or it is gone.
    pub name: Option<String>,
    pub entries: Vec<PlaylistEntryDto>,
    pub at: Option<usize>,
    pub advance: bool,
    pub repeat: bool,
    /// The record [`next`] would give, by its place in `entries`.
    pub next: Option<usize>,
    /// Why the list could not be read, said rather than shown empty.
    pub problem: Option<String>,
}

fn dto(state: &AppState, deck: u8, queue: &DeckQueue) -> DeckQueueDto {
    let mut answer = DeckQueueDto {
        deck,
        playlist: queue.playlist,
        name: None,
        entries: Vec::new(),
        at: queue.at,
        advance: queue.advance,
        repeat: queue.repeat,
        next: None,
        problem: None,
    };
    let Some(id) = queue.playlist else {
        return answer;
    };
    match crate::commands::library(state).and_then(|db| {
        let name = db
            .playlists()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|playlist| playlist.id == id)
            .map(|playlist| playlist.name);
        let entries = db.playlist_tracks(id).map_err(|e| e.to_string())?;
        Ok((name, entries))
    }) {
        Ok((name, entries)) => {
            answer.name = name;
            answer.entries = entries
                .into_iter()
                .map(|(position, track)| PlaylistEntryDto {
                    position,
                    track: LibraryTrackDto::from(track),
                })
                .collect();
            answer.next = next(answer.entries.len(), queue.at, queue.repeat);
        }
        Err(error) => answer.problem = Some(format!("The playlist could not be read: {error}")),
    }
    answer
}

/// The playlist under deck `deck` (1-based).
#[tauri::command]
pub fn deck_queue(state: tauri::State<'_, AppState>, deck: u8) -> Result<DeckQueueDto, String> {
    let index = index_of(deck)?;
    let queue = queues(&state).decks[index].clone();
    Ok(dto(&state, deck, &queue))
}

/// Choose deck `deck`'s playlist and how it goes on. A different playlist
/// starts from its first record.
#[tauri::command]
pub fn deck_queue_set(
    state: tauri::State<'_, AppState>,
    deck: u8,
    playlist: Option<i64>,
    advance: bool,
    repeat: bool,
) -> Result<DeckQueueDto, String> {
    let index = index_of(deck)?;
    let queue = {
        let mut held = queues(&state);
        let queue = &mut held.decks[index];
        if queue.playlist != playlist {
            queue.at = None;
        }
        queue.playlist = playlist;
        queue.advance = advance;
        queue.repeat = repeat;
        let queue = queue.clone();
        keep(&state, &held);
        queue
    };
    Ok(dto(&state, deck, &queue))
}

/// Load the queue's record at `index` onto deck `deck`, and play it with
/// `play`.
#[tauri::command]
pub async fn deck_queue_load(
    state: tauri::State<'_, AppState>,
    deck: u8,
    index: usize,
    play: bool,
) -> Result<DeckQueueDto, String> {
    let deck_index = index_of(deck)?;
    let state: &AppState = &state;
    let path = entry_path(state, deck_index, index)?;
    let decoded = tauri::async_runtime::spawn_blocking(move || dj_decode::decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;
    put(state, deck_index, index, decoded, play)?;
    let queue = queues(state).decks[deck_index].clone();
    Ok(dto(state, deck, &queue))
}

fn entry_path(state: &AppState, deck_index: usize, index: usize) -> Result<String, String> {
    let playlist = queues(state).decks[deck_index]
        .playlist
        .ok_or("no playlist is chosen for this deck")?;
    let entries = crate::commands::library(state)?
        .playlist_tracks(playlist)
        .map_err(|e| e.to_string())?;
    let (_, track) = entries
        .into_iter()
        .nth(index)
        .ok_or("that record is no longer in the playlist")?;
    Ok(track.path.to_string_lossy().into_owned())
}

fn put(
    state: &AppState,
    deck_index: usize,
    index: usize,
    decoded: dj_decode::DecodedTrack,
    play: bool,
) -> Result<(), String> {
    let deck = u8::try_from(deck_index)
        .ok()
        .and_then(dj_core::DeckId::new)
        .ok_or("no such deck")?;
    crate::commands::put_on_deck(state, deck, decoded)?;
    {
        let mut held = queues(state);
        held.decks[deck_index].at = Some(index);
        keep(state, &held);
    }
    if play {
        crate::commands::perform_by(
            state,
            &format!("deck {} play", deck_index + 1),
            dj_control::By::Machine,
        )?;
    }
    Ok(())
}

/// One frame of the pump: the decks that ran out and should be given their
/// next record. Cheap — no file is read here.
pub fn tick(state: &AppState, snapshot: &crate::snapshot::Snapshot) -> Vec<usize> {
    let views: Vec<DeckView> = snapshot
        .decks
        .iter()
        .map(|deck| DeckView {
            loaded: deck.loaded,
            playing: deck.playing,
            position_seconds: deck.position_seconds,
            length_seconds: deck.length_seconds,
        })
        .collect();
    queues(state).tick(&views)
}

/// Give deck `deck_index` the queue's next record and play it — off the
/// pump, since it decodes a file. Nothing when the list has run out without
/// *Repeat*.
pub fn advance(state: &AppState, deck_index: usize) {
    let (playlist, at, repeat) = {
        let held = queues(state);
        let queue = &held.decks[deck_index];
        (queue.playlist, queue.at, queue.repeat)
    };
    let Some(playlist) = playlist else { return };
    let len = crate::commands::library(state)
        .and_then(|db| db.playlist_tracks(playlist).map_err(|e| e.to_string()))
        .map_or(0, |entries| entries.len());
    let Some(index) = next(len, at, repeat) else {
        return;
    };
    let loaded = entry_path(state, deck_index, index)
        .and_then(|path| dj_decode::decode_file(&path).map_err(|e| e.to_string()))
        .and_then(|decoded| put(state, deck_index, index, decoded, true));
    if let Err(error) = loaded {
        tracing::warn!(%error, deck = deck_index + 1, "the deck's playlist could not go on");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The record after, in order; the first again only with Repeat.**
    #[test]
    fn the_next_record_is_the_one_after_and_round_again_only_with_repeat() {
        assert_eq!(
            next(3, None, false),
            Some(0),
            "nothing taken yet: the first"
        );
        assert_eq!(next(3, Some(0), false), Some(1));
        assert_eq!(next(3, Some(1), true), Some(2));
        assert_eq!(next(3, Some(2), false), None, "the end, without repeat");
        assert_eq!(next(3, Some(2), true), Some(0), "the end, with repeat");
        assert_eq!(next(0, None, true), None, "an empty list gives nothing");
        // A list that shrank under the deck: past its end is its end.
        assert_eq!(next(2, Some(5), false), None);
        assert_eq!(next(2, Some(5), true), Some(0));
    }

    fn view(playing: bool, position: f32) -> DeckView {
        DeckView {
            loaded: true,
            playing,
            position_seconds: position,
            length_seconds: 30.0,
        }
    }

    /// **A deck runs out only when it stops at its end.** A pause in the
    /// middle is a hand on it; a deck that was never playing has nothing to
    /// run out of; a deck still playing at its last frame has not stopped.
    #[test]
    fn a_deck_runs_out_only_when_it_stops_at_its_end() {
        assert!(ran_out(true, &view(false, 29.8)));
        assert!(!ran_out(true, &view(false, 12.0)), "paused mid-record");
        assert!(!ran_out(false, &view(false, 29.8)), "was not playing");
        assert!(!ran_out(true, &view(true, 29.9)), "still playing");
        let empty = DeckView {
            loaded: false,
            ..view(false, 0.0)
        };
        assert!(!ran_out(true, &empty), "nothing on it");
    }

    /// **Only a deck asked to go on is given its next record, once.** Deck 1
    /// has a playlist and *Next when this ends*; deck 2 has the playlist but
    /// not the switch. Both run out on the same frame: deck 1 is due, deck 2
    /// is not, and the frame after asks for nothing again.
    #[test]
    fn only_a_deck_asked_to_go_on_is_given_its_next_record_once() {
        let mut queues = Queues::default();
        queues.decks[0] = DeckQueue {
            playlist: Some(7),
            advance: true,
            ..DeckQueue::default()
        };
        queues.decks[1] = DeckQueue {
            playlist: Some(7),
            ..DeckQueue::default()
        };
        assert!(
            queues
                .tick(&[view(true, 29.0), view(true, 29.0)])
                .is_empty()
        );
        assert_eq!(
            queues.tick(&[view(false, 29.9), view(false, 29.9)]),
            vec![0]
        );
        assert!(
            queues
                .tick(&[view(false, 29.9), view(false, 29.9)])
                .is_empty()
        );
        // No playlist, nothing to go on with, whatever the switch says.
        queues.decks[0].playlist = None;
        queues.tick(&[view(true, 29.0)]);
        assert!(queues.tick(&[view(false, 29.9)]).is_empty());
    }
}

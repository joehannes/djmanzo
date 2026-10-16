//! Karaoke beta B2: the room's emoji, as the singers' screen asks for them.
//!
//! The book is `dj_net::reactions` on the room's page; this is what the
//! screen and the host's switch reach it through. The screen asks every
//! second for what is newer than what it last drew, and draws it small, in a
//! corner, letting it go within seconds (`RoomEmoji.svelte`).

use std::time::SystemTime;

use dj_net::reactions::{EMOJI, Reactions};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// One reaction to draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomReactionDto {
    pub id: u64,
    pub emoji: String,
}

/// Whether the host has them on, and what is new to draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomReactionsDto {
    pub on: bool,
    pub reactions: Vec<RoomReactionDto>,
}

/// What is newer than `since`, still worth drawing at `now`, as glyphs.
#[must_use]
pub fn answer(book: &Reactions, since: u64, now: SystemTime) -> RoomReactionsDto {
    RoomReactionsDto {
        on: book.is_on(),
        reactions: book
            .since(since, now)
            .into_iter()
            .filter_map(|reaction| {
                EMOJI.get(reaction.emoji).map(|glyph| RoomReactionDto {
                    id: reaction.id,
                    emoji: (*glyph).to_owned(),
                })
            })
            .collect(),
    }
}

/// The singers' screen: what the room sent since it last asked.
#[tauri::command]
#[must_use]
pub fn room_reactions(state: State<'_, AppState>, since: Option<u64>) -> RoomReactionsDto {
    state
        .audience()
        .front()
        .reactions(|book| answer(book, since.unwrap_or(0), SystemTime::now()))
        .unwrap_or(RoomReactionsDto {
            on: false,
            reactions: Vec::new(),
        })
}

/// The host's switch: the row on the room's page, and the screen drawing it.
#[tauri::command]
#[must_use]
pub fn room_reactions_set(state: State<'_, AppState>, on: bool) -> RoomReactionsDto {
    state.audience().front().reactions(|book| book.set_on(on));
    room_reactions(state, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_net::reactions::Reactions;
    use std::time::Duration;

    #[test]
    fn the_screen_is_told_the_glyphs_and_whether_it_is_on() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut book = Reactions::new();
        assert_eq!(
            answer(&book, 0, now),
            RoomReactionsDto {
                on: false,
                reactions: Vec::new()
            }
        );
        book.set_on(true);
        book.react("a", 0, now);
        book.react("b", 2, now);
        let told = answer(&book, 0, now);
        assert!(told.on);
        assert_eq!(
            told.reactions
                .iter()
                .map(|r| r.emoji.as_str())
                .collect::<Vec<_>>(),
            ["👏", "🔥"]
        );
        // Asked again after the first, only the second is new.
        let again = answer(&book, told.reactions[0].id, now);
        assert_eq!(again.reactions.len(), 1);
        assert_eq!(again.reactions[0].emoji, "🔥");
    }
}

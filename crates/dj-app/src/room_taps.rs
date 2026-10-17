//! The room's taps, kept in the night's crowd log.
//!
//! A phone on the room's page can send one of six emoji (karaoke beta B2,
//! `dj_net::reactions`); the singers' screen draws them for three seconds
//! and the book keeps the last sixty-four. That was all: the night's crowd
//! log (§119, `crate::crowd`) — where YouTube's chat and a log brought in
//! already go, placed against what was playing — never heard them. The pump
//! now calls [`tick`], and every tap newer than the last one kept is written
//! there once, as a reaction from the room with no name, its text the glyph.
//!
//! A tap the host has switched off was never taken, so nothing here filters.
//! The book's own limits (one tap a phone every two seconds, six a second
//! for the room) are what keep the log from filling.

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use dj_net::reactions::{EMOJI, Reactions};

use crate::AppState;

/// The newest tap already kept, by the book's own increasing id.
fn kept() -> &'static Mutex<u64> {
    static KEPT: OnceLock<Mutex<u64>> = OnceLock::new();
    KEPT.get_or_init(|| Mutex::new(0))
}

/// The taps newer than `after`, as the crowd log keeps a reaction, and the
/// newest id among them (`after` when there are none).
#[must_use]
pub fn fresh(book: &Reactions, after: u64, now: SystemTime) -> (Vec<crate::crowd::Reaction>, u64) {
    let mut newest = after;
    let taps = book
        .since(after, now)
        .into_iter()
        .filter_map(|tap| {
            newest = newest.max(tap.id);
            let glyph = EMOJI.get(tap.emoji)?;
            let at = tap
                .at
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|since| i64::try_from(since.as_secs()).ok())?;
            Some(crate::crowd::Reaction {
                at,
                source: crate::crowd::Source::Room,
                who: String::new(),
                text: (*glyph).to_owned(),
            })
        })
        .collect();
    (taps, newest)
}

/// Keep what the room has tapped since the last call in tonight's crowd log.
pub fn tick(state: &AppState) {
    let Some(config) = state.config_dir() else {
        return;
    };
    let Ok(mut last) = kept().lock() else {
        return;
    };
    let after = *last;
    let Some((taps, newest)) = state
        .audience()
        .front()
        .reactions(|book| fresh(book, after, SystemTime::now()))
    else {
        return;
    };
    if taps.is_empty() {
        *last = newest;
        return;
    }
    match crate::crowd::append(&config, &state.session_id(), &taps) {
        Ok(()) => *last = newest,
        // Not marked kept: tried again with the next tick, while the book
        // still holds them.
        Err(error) => tracing::warn!(%error, "the room's taps were not kept"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every tap once, as the room's, its glyph the words**, in the night's
    /// crowd log through `AppState` — and a second tick keeps nothing again.
    #[test]
    fn the_rooms_taps_reach_the_nights_crowd_log_once() {
        let dir = tempfile::tempdir().expect("a folder");
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        let now = SystemTime::now();
        let front = state.audience().front();
        front.reactions(|book| {
            book.set_on(true);
            book.react("phone-a", 2, now);
            book.react("phone-b", 0, now);
        });

        tick(&state);
        let kept = crate::crowd::load(dir.path(), &state.session_id());
        let texts: Vec<&str> = kept.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["🔥", "👏"]);
        assert!(
            kept.iter()
                .all(|r| r.source == crate::crowd::Source::Room && r.who.is_empty())
        );

        tick(&state);
        assert_eq!(
            crate::crowd::load(dir.path(), &state.session_id()).len(),
            2,
            "a tap kept twice"
        );

        front.reactions(|book| book.react("phone-c", 5, now));
        tick(&state);
        let kept = crate::crowd::load(dir.path(), &state.session_id());
        assert_eq!(kept.last().map(|r| r.text.as_str()), Some("🙌"));
        assert_eq!(kept.len(), 3);
        *super::kept().lock().expect("the cursor") = 0;
    }
}

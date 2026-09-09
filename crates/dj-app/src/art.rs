//! Cover art, served to the webview as images.
//!
//! §20 asks for a card view of the library, "when album/artwork is valuable".
//! djmanzo could not offer one: the scan read every tag except the pictures,
//! so there was no artwork to be valuable. `dj_library::tags::artwork` is the
//! read; this is how it reaches a card.
//!
//! # Why a URI scheme rather than a command
//!
//! The same reasoning as waveform tiles ([ADR-0004](../../../docs/adr/0004-waveform-rendering-strategy.md))
//! and the logo: a card grid asks for fifty images at once, and pushing fifty
//! JPEGs through IPC as base64 costs a third more bytes, blocks the main
//! thread decoding them, and defeats the browser's own image cache. As a
//! scheme they are ordinary `<img src>` — decoded off the main thread, cached
//! by URL, and free to scroll.
//!
//! # Read once, kept
//!
//! Reading a picture means opening and probing the file, which is far too slow
//! to do while a grid scrolls. So each answer is kept — **including the
//! absence of one**, which is the more important half: a hand-organised
//! collection is mostly untagged files, and without remembering that, every
//! scroll would re-probe every one of them from disk to learn nothing.

use dj_core::TrackId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The URI scheme cover art is served on.
pub const SCHEME: &str = "art";

/// One cover, ready to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub mime: String,
    pub bytes: Arc<Vec<u8>>,
}

/// What has been read, by track.
///
/// `None` inside the map is a track that has been looked at and has no cover —
/// deliberately distinct from a track that is not in the map at all, which is
/// one nobody has asked about yet.
#[derive(Debug, Default)]
pub struct Covers {
    known: Mutex<HashMap<TrackId, Option<Cover>>>,
}

impl Covers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The cover for a track, reading the file the first time it is asked for.
    ///
    /// `find` turns an id into a path. A closure rather than a library handle
    /// so this can be tested without a database, and so the lock is never held
    /// across a database call.
    pub fn get(
        &self,
        track: TrackId,
        find: impl FnOnce(TrackId) -> Option<std::path::PathBuf>,
    ) -> Option<Cover> {
        // Two separate locks with the read in between, on purpose. Holding it
        // across the file read would make a grid of fifty cards serialise on
        // one mutex behind fifty disk reads, which is precisely the stall this
        // module exists to avoid.
        if let Ok(known) = self.known.lock()
            && let Some(answer) = known.get(&track)
        {
            return answer.clone();
        }

        let found = find(track).and_then(|path| dj_library::tags::artwork(&path));
        let cover = found.map(|art| Cover {
            mime: art.mime,
            bytes: Arc::new(art.bytes),
        });
        if let Ok(mut known) = self.known.lock() {
            known.insert(track, cover.clone());
        }
        cover
    }

    /// How many tracks have been looked at. For tests and for the log.
    #[must_use]
    pub fn known(&self) -> usize {
        self.known.lock().map(|k| k.len()).unwrap_or(0)
    }
}

/// The track a request is asking for.
///
/// The path is `/<64 hex digits>`, which is a `TrackId` and nothing else.
/// Strict rather than lenient for the same reason `TrackId::from_hex` is: a
/// short or malformed id would otherwise be zero-padded into a *valid-looking*
/// id pointing at the wrong record.
#[must_use]
pub fn parse_path(path: &str) -> Option<TrackId> {
    TrackId::from_hex(path.trim_start_matches('/').trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn id(n: u8) -> TrackId {
        let mut bytes = [0u8; 32];
        bytes[0] = n;
        TrackId::from_bytes(bytes)
    }

    #[test]
    fn a_request_names_one_track_or_nothing() {
        assert_eq!(parse_path(&format!("/{}", id(7).to_hex())), Some(id(7)));
        // Without the slash, as a bare path, and with whitespace around it.
        assert_eq!(parse_path(&id(7).to_hex()), Some(id(7)));
        assert_eq!(parse_path(&format!("/{} ", id(7).to_hex())), Some(id(7)));

        assert_eq!(parse_path("/"), None);
        assert_eq!(parse_path("/not-a-track"), None);
        // A short id is refused rather than padded into a different record.
        assert_eq!(parse_path(&format!("/{}", "ab".repeat(20))), None);
    }

    /// **A track with no cover is remembered as having none.**
    ///
    /// The half that matters. A hand-organised collection is mostly untagged
    /// files, and a cache that only remembered successes would re-probe every
    /// one of them from disk on every scroll to learn nothing.
    #[test]
    fn the_absence_of_a_cover_is_remembered_too() {
        let covers = Covers::new();
        let mut asked = 0;
        let mut find = |_| {
            asked += 1;
            Some(PathBuf::from("/nonexistent/track.flac"))
        };

        assert_eq!(covers.get(id(1), &mut find), None);
        assert_eq!(covers.get(id(1), &mut find), None);
        assert_eq!(covers.get(id(1), &mut find), None);
        assert_eq!(
            asked, 1,
            "a track with no cover was looked up {asked} times"
        );
        assert_eq!(covers.known(), 1);
    }

    /// A track the library does not have is not looked at twice either.
    #[test]
    fn a_track_the_library_has_lost_is_remembered_as_absent() {
        let covers = Covers::new();
        let mut asked = 0;
        let mut find = |_| {
            asked += 1;
            None
        };
        assert_eq!(covers.get(id(2), &mut find), None);
        assert_eq!(covers.get(id(2), &mut find), None);
        assert_eq!(asked, 1);
    }
}

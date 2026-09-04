//! Sleeves, served as images.
//!
//! §20's second view is a grid of cards, and a card without its sleeve is a
//! tile with words on it. The pictures are already in the files — DJs tag
//! their collections — so this is about getting them to the webview.
//!
//! # Why a URI scheme rather than a command
//!
//! Base64 through IPC would put a megabyte of JPEG in a JSON string per card,
//! re-encoded on every render and decoded on the interface's own thread. As a
//! URL it is an ordinary `<img>`: the browser fetches what is on screen, when
//! it is on screen, decodes it off the main thread, and keeps it. That is the
//! same trick waveform tiles and the user's logo already use, for the same
//! reason — see `crate::waveform` and `crate::brand`.
//!
//! # Read on demand, cached nowhere here
//!
//! Nothing is precomputed at scan time. Reading a tag is milliseconds and
//! touches none of the audio, while a cache of five hundred sleeves is
//! hundreds of megabytes that has to be invalidated by something. The browser
//! is already a cache, so it is allowed to be the only one.

use dj_core::TrackId;
use std::sync::Arc;
use tauri::http;

/// The URI scheme sleeves are served on.
pub const SCHEME: &str = "cover";

/// How long the webview may keep a sleeve.
///
/// A [`TrackId`] hashes the audio, not the tags, so re-tagging a record with a
/// better sleeve leaves this URL pointing at the same track — cache it for a
/// year, as tiles are, and yesterday's artwork would stay on screen until a
/// restart. An hour is long enough that scrolling a collection costs one read
/// per record and short enough that a correction shows up while the person who
/// made it is still working.
pub const MAX_AGE_SECONDS: u32 = 3_600;

/// The track a `cover://` request is asking about.
///
/// The path is one segment: the track's id in hex. Anything else is a bug in
/// the caller rather than something to guess at, and returning `None` makes it
/// a 400 the network panel shows instead of a silent wrong picture.
#[must_use]
pub fn parse_cover_path(path: &str) -> Option<TrackId> {
    let mut parts = path.trim_start_matches('/').split('/');
    let id = TrackId::from_hex(parts.next()?)?;
    parts.next().is_none().then_some(id)
}

/// The sleeve for a track, and the content type to serve it as.
///
/// `None` when the library has never heard of the track, when the file has
/// moved, when it carries no picture, or when the picture is in a format a
/// browser will not draw. All four are ordinary, and all four mean the same
/// thing to a card: fall back to lettering.
///
/// **The bytes decide the content type, not the tag.** A tag that says
/// `image/jpg` — not a media type — is common enough that trusting it would
/// show a broken-image icon on real collections.
#[must_use]
pub fn read(
    library: &crate::library::LibraryHandle,
    id: TrackId,
) -> Option<(Vec<u8>, &'static str)> {
    let path = library.get().ok()?.track(id).ok()??.path;
    let bytes = dj_library::tags::artwork(&path).ok()??;
    let mime = crate::brand::content_type(&bytes)?;
    Some((bytes, mime))
}

/// Answer one `cover://` request.
///
/// Split from the protocol handler so the shape of the answer — which status,
/// which headers, what a missing sleeve does — is testable without a webview.
#[must_use]
pub fn respond(
    library: &Arc<crate::library::LibraryHandle>,
    path: &str,
) -> http::Response<Vec<u8>> {
    let Some(id) = parse_cover_path(path) else {
        return http::Response::builder()
            .status(400)
            .body(Vec::new())
            .unwrap_or_default();
    };
    match read(library, id) {
        Some((bytes, mime)) => http::Response::builder()
            .status(200)
            .header("Content-Type", mime)
            .header(
                "Cache-Control",
                format!("private, max-age={MAX_AGE_SECONDS}"),
            )
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes)
            .unwrap_or_default(),
        None => http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// A 1x1 PNG. Only the eight-byte signature matters to the sniffer, but a
    /// real one keeps the test honest about what a browser would be handed.
    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, b'I', b'D', b'A', b'T', 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, b'I',
        b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82,
    ];

    /// A silent WAV, with `sleeve` written into its tag as a front cover when
    /// there is one.
    ///
    /// The tag declares JPEG whatever the bytes are, on purpose: that is what
    /// real taggers do, and serving what the tag *says* is the bug this module
    /// documents avoiding.
    fn wav_with_cover(path: &std::path::Path, sleeve: &[u8]) {
        use lofty::config::WriteOptions;
        use lofty::prelude::TagExt;
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&40u32.to_le_bytes());
        file.extend_from_slice(b"WAVEfmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&44_100u32.to_le_bytes());
        file.extend_from_slice(&88_200u32.to_le_bytes());
        file.extend_from_slice(&2u16.to_le_bytes());
        file.extend_from_slice(&16u16.to_le_bytes());
        file.extend_from_slice(b"data");
        file.extend_from_slice(&4u32.to_le_bytes());
        file.extend_from_slice(&[0, 0, 0, 0]);
        std::fs::write(path, file).unwrap();
        if sleeve.is_empty() {
            return;
        }
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        tag.push_picture(lofty::picture::Picture::new_unchecked(
            lofty::picture::PictureType::CoverFront,
            Some(lofty::picture::MimeType::Jpeg),
            None,
            sleeve.to_vec(),
        ));
        tag.save_to_path(path, WriteOptions::default()).unwrap();
    }

    #[test]
    fn a_track_id_is_the_whole_path() {
        assert_eq!(parse_cover_path(&format!("/{HEX}")), TrackId::from_hex(HEX));
    }

    #[test]
    fn anything_but_one_id_is_refused() {
        assert_eq!(parse_cover_path("/"), None);
        assert_eq!(parse_cover_path("/not-an-id"), None);
        // A trailing segment. Nothing generates one, so it is a caller with a
        // different idea of this URL's shape and should hear about it.
        assert_eq!(parse_cover_path(&format!("/{HEX}/front")), None);
    }

    fn library() -> Arc<crate::library::LibraryHandle> {
        Arc::new(crate::library::LibraryHandle::in_memory().unwrap())
    }

    /// Put a record in the library pointing at `path`.
    fn known(library: &crate::library::LibraryHandle, id: TrackId, path: &std::path::Path) {
        library
            .get()
            .unwrap()
            .upsert_track(&dj_library::LibraryTrack {
                id,
                path: path.to_path_buf(),
                tags: dj_library::Tags::default(),
                duration_frames: 44_100,
                sample_rate: dj_core::SampleRate::new(44_100).unwrap(),
                channels: 2,
                file_size: None,
                file_modified: None,
                added_at: 0,
                analysis: dj_library::StoredAnalysis::default(),
                stats: dj_library::PlayStats::default(),
                colour: None,
            })
            .unwrap();
    }

    /// The round trip this module exists for: a file with a sleeve in its tags
    /// comes back as an image the webview can draw, typed by its own bytes.
    #[test]
    fn a_tagged_sleeve_is_served_as_an_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track.wav");
        wav_with_cover(&path, PNG);
        let library = library();
        let id = TrackId::from_hex(HEX).unwrap();
        known(&library, id, &path);

        let response = respond(&library, &format!("/{HEX}"));
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("Content-Type").unwrap(),
            "image/png",
            "the bytes are a PNG; the tag said JPEG"
        );
        assert_eq!(response.body(), &PNG.to_vec());
    }

    /// A record nobody has ever scanned is a 404, not an error page and not a
    /// picture of something else.
    #[test]
    fn an_unknown_track_is_missing_rather_than_broken() {
        assert_eq!(respond(&library(), &format!("/{HEX}")).status(), 404);
    }

    /// Most of a real collection. The card falls back to lettering, which is
    /// not a failure and must not read as one.
    #[test]
    fn a_record_with_no_sleeve_is_missing_rather_than_broken() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track.wav");
        wav_with_cover(&path, &[]);
        let library = library();
        let id = TrackId::from_hex(HEX).unwrap();
        known(&library, id, &path);
        assert_eq!(respond(&library, &format!("/{HEX}")).status(), 404);
    }

    #[test]
    fn a_malformed_path_is_a_bad_request() {
        assert_eq!(respond(&library(), "/nonsense").status(), 400);
    }
}

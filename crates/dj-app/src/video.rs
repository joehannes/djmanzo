//! §122: a video file on a deck — its sound a record's, its picture the
//! webview's, served from here. See `docs/VIDEO.md`.
//!
//! djmanzo decodes no video. The webview's own `<video>` element draws the
//! picture, and reads it from a small server on this computer:
//! `http://127.0.0.1:<port>/<key>/<deck>/<record>`. What answers is [`Pictures`],
//! which knows the one file on each deck that has a picture, and [`serve`],
//! which reads the part of it asked for.
//!
//! # Why a server, when cover art is a URI scheme
//!
//! On Linux the webview is WebKitGTK, and it plays media through GStreamer,
//! which fetches only from the addresses it has sources for — `http`, `https`,
//! `blob`. An application's own scheme is not one of them: a `<video>` given
//! one failed to load at once, its type found supported and nothing fetched
//! (driving §122 under WebKitGTK). So the picture comes over HTTP, from
//! `127.0.0.1`, which every webview djmanzo runs in plays from.
//!
//! # Only what is on a deck
//!
//! A server that read any path it was given would be a way for anything that
//! can reach it to read the disk. This reads a file only while it is on a
//! deck, only by the deck's number, and only while the record the address
//! names is still the one there: a `<video>` left over from the record before
//! is told nothing is there, rather than played the next one's picture. It
//! listens on `127.0.0.1` only, and its addresses carry a random key, so a
//! website open in a browser on the same computer cannot even find it.
//!
//! # In ranges
//!
//! A video is hundreds of megabytes and a picture seeks, so it is read the way
//! a browser reads one from any server: in ranges, [`MOST_AT_ONCE`] at a time,
//! each answered `206 Partial Content` with where it sits in the whole.

use dj_core::TrackId;
use dj_net::web::{Handler, Incoming, Method, Reply, WebServer};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

/// The extensions of the files djmanzo treats as having a picture.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "mkv", "webm"];

/// The most read for one request: four megabytes, a few seconds of an
/// ordinary music video. The webview asks again for the rest.
pub const MOST_AT_ONCE: u64 = 4 * 1024 * 1024;

/// Whether `path` is one of [`VIDEO_EXTENSIONS`].
#[must_use]
pub fn has_picture(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
}

/// What the webview is told the file is.
#[must_use]
pub fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mov") => "video/quicktime",
        _ => "video/mp4",
    }
}

/// The picture on one deck: the file, and the record it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub path: PathBuf,
    pub record: TrackId,
}

/// The pictures on the decks, by deck number, and the server they are read
/// from once anything has asked.
#[derive(Default)]
pub struct Pictures {
    on: Mutex<HashMap<u8, Picture>>,
    server: Mutex<Option<(WebServer, String)>>,
}

impl std::fmt::Debug for Pictures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pictures").finish_non_exhaustive()
    }
}

impl Pictures {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn on(&self) -> std::sync::MutexGuard<'_, HashMap<u8, Picture>> {
        self.on
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `record`, from `path`, went onto `deck`. It has a picture if its file
    /// is a video's; a record without one takes the deck's picture away.
    pub fn loaded(&self, deck: u8, record: TrackId, path: &Path) {
        let mut on = self.on();
        if has_picture(path) {
            on.insert(
                deck,
                Picture {
                    path: path.to_path_buf(),
                    record,
                },
            );
        } else {
            on.remove(&deck);
        }
    }

    /// The deck was emptied.
    pub fn ejected(&self, deck: u8) {
        self.on().remove(&deck);
    }

    /// The picture on `deck`, if it is still `record`'s.
    #[must_use]
    pub fn of(&self, deck: u8, record: &str) -> Option<Picture> {
        self.on()
            .get(&deck)
            .filter(|p| p.record.to_hex() == record)
            .cloned()
    }

    /// Where the pictures are read from — `http://127.0.0.1:<port>/<key>` —
    /// starting the server the first time it is asked.
    ///
    /// # Errors
    /// When no port can be had, or no key made.
    pub fn address(self: &Arc<Self>) -> Result<String, String> {
        let mut server = self
            .server
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((web, key)) = server.as_ref() {
            return Ok(format!("http://{}/{key}", web.address()));
        }
        let key = crate::phone_camera::new_key()?;
        // Weak: the server is the pictures', and must not keep them alive.
        let pictures = Arc::downgrade(self);
        let handler = {
            let key = key.clone();
            move |request: &Incoming| answer(&pictures, &key, request)
        };
        let web = WebServer::start(
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            Arc::new(handler) as Arc<dyn Handler>,
        )
        .map_err(|e| e.to_string())?;
        let address = format!("http://{}/{key}", web.address());
        *server = Some((web, key));
        Ok(address)
    }

    /// Every deck with a picture, the record each is, and what kind of file:
    /// what the Video panel builds its addresses from, and names when one
    /// will not play.
    #[must_use]
    pub fn all(&self) -> Vec<(u8, String, &'static str)> {
        let mut all: Vec<(u8, String, &'static str)> = self
            .on()
            .iter()
            .map(|(deck, p)| (*deck, p.record.to_hex(), mime(&p.path)))
            .collect();
        all.sort_unstable();
        all
    }
}

/// `"/<key>/<deck>/<record>"`: the deck and the record, when the key is
/// `key`.
#[must_use]
pub fn parse_path(path: &str, key: &str) -> Option<(u8, String)> {
    let mut parts = path.trim_start_matches('/').split('/');
    if key.is_empty() || parts.next()? != key {
        return None;
    }
    let deck = parts.next()?.parse().ok()?;
    let record = parts.next()?;
    (parts.next().is_none() && record.len() == 64 && record.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| (deck, record.to_ascii_lowercase()))
}

/// The server's answer to `request`.
fn answer(pictures: &Weak<Pictures>, key: &str, request: &Incoming) -> Reply {
    let picture = (request.method == Method::Get)
        .then(|| parse_path(&request.path, key))
        .flatten()
        .and_then(|(deck, record)| pictures.upgrade()?.of(deck, &record));
    let Some(picture) = picture else {
        return Reply::not_found();
    };
    let answer = serve(&picture, request.range.as_deref());
    Reply {
        status: answer.status,
        content_type: mime(&picture.path),
        headers: answer
            .headers
            .into_iter()
            // The server says how long the body is itself.
            .filter(|(name, _)| *name != "Content-Type" && *name != "Content-Length")
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
        body: answer.body,
    }
}

/// The bytes asked for by a `Range` header, of a file `total` long: first and
/// last, inclusive, at most [`MOST_AT_ONCE`] of them. `None` for a range
/// outside the file, or one not in the one form a media element sends
/// (`bytes=a-` or `bytes=a-b`). No header at all is the whole file, from the
/// start.
#[must_use]
pub fn range(header: Option<&str>, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let (first, last) = match header {
        None => (0, total - 1),
        Some(header) => {
            let spec = header.trim().strip_prefix("bytes=")?;
            let (a, b) = spec.split_once('-')?;
            let first: u64 = a.trim().parse().ok()?;
            let last = if b.trim().is_empty() {
                total - 1
            } else {
                b.trim().parse::<u64>().ok()?.min(total - 1)
            };
            (first, last)
        }
    };
    // `last` is inside the file by now, so a `first` past it is past the end.
    (first <= last).then(|| (first, last.min(first + MOST_AT_ONCE - 1)))
}

/// An answer to the webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    fn nothing(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

/// Read what `range_header` asks for of `picture`'s file.
#[must_use]
pub fn serve(picture: &Picture, range_header: Option<&str>) -> Answer {
    let Ok(mut file) = std::fs::File::open(&picture.path) else {
        return Answer::nothing(404);
    };
    let Ok(total) = file.metadata().map(|m| m.len()) else {
        return Answer::nothing(404);
    };
    let Some((first, last)) = range(range_header, total) else {
        let mut refused = Answer::nothing(416);
        refused
            .headers
            .push(("Content-Range", format!("bytes */{total}")));
        return refused;
    };
    let length = last - first + 1;
    let mut body = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
    if file.seek(SeekFrom::Start(first)).is_err()
        || file.take(length).read_to_end(&mut body).is_err()
    {
        return Answer::nothing(500);
    }
    Answer {
        status: 206,
        headers: vec![
            ("Content-Type", mime(&picture.path).to_owned()),
            ("Accept-Ranges", "bytes".to_owned()),
            ("Content-Range", format!("bytes {first}-{last}/{total}")),
            ("Content-Length", body.len().to_string()),
            // The address names the record, and a record's file is not
            // rewritten under a running application.
            ("Cache-Control", "no-cache".to_owned()),
        ],
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(n: u8) -> TrackId {
        TrackId::from_bytes([n; 32])
    }

    #[test]
    fn a_video_is_known_by_its_extension() {
        for path in ["a.mp4", "b.M4V", "c.mov", "d.mkv", "e.WebM"] {
            assert!(has_picture(Path::new(path)), "{path}");
        }
        for path in ["a.mp3", "b.flac", "c.m4a", "d", "e.webm.txt"] {
            assert!(!has_picture(Path::new(path)), "{path}");
        }
        assert_eq!(mime(Path::new("a.WEBM")), "video/webm");
        assert_eq!(mime(Path::new("a.m4v")), "video/mp4");
    }

    /// **A picture is served only while its record is on the deck**: a
    /// record without one takes it away, an eject does, and an address that
    /// names the record before is told nothing is there.
    #[test]
    fn only_the_record_on_the_deck_is_served() {
        let pictures = Pictures::new();
        pictures.loaded(1, record(1), Path::new("/music/clip.mp4"));
        pictures.loaded(2, record(2), Path::new("/music/song.flac"));
        assert_eq!(pictures.all(), vec![(1, record(1).to_hex(), "video/mp4")]);
        assert!(pictures.of(1, &record(1).to_hex()).is_some());
        assert!(pictures.of(1, &record(9).to_hex()).is_none());
        assert!(pictures.of(2, &record(2).to_hex()).is_none());

        pictures.loaded(1, record(3), Path::new("/music/song.mp3"));
        assert!(pictures.of(1, &record(1).to_hex()).is_none());
        assert!(pictures.all().is_empty());

        pictures.loaded(1, record(4), Path::new("/music/clip.webm"));
        pictures.ejected(1);
        assert!(pictures.all().is_empty());
    }

    #[test]
    fn the_path_is_a_key_a_deck_and_a_record() {
        let hex = record(5).to_hex();
        assert_eq!(
            parse_path(&format!("/k3y/2/{hex}"), "k3y"),
            Some((2, hex.clone()))
        );
        assert_eq!(
            parse_path(&format!("/k3y/2/{}", hex.to_uppercase()), "k3y"),
            Some((2, hex.clone()))
        );
        for bad in [
            format!("/2/{hex}"),
            format!("/k3z/2/{hex}"),
            format!("/k3y/x/{hex}"),
            format!("/k3y/2/{hex}/more"),
            "/k3y/2/../../etc/passwd".to_owned(),
            format!("/k3y/2/{}", &hex[..10]),
        ] {
            assert_eq!(parse_path(&bad, "k3y"), None, "{bad}");
        }
        assert_eq!(
            parse_path(&format!("//2/{hex}"), ""),
            None,
            "no key, no picture"
        );
    }

    /// **Ranges as a media element asks for them**, each capped, and nothing
    /// outside the file.
    #[test]
    fn ranges_are_read_as_asked_and_capped() {
        let total = 10 * MOST_AT_ONCE;
        assert_eq!(range(None, total), Some((0, MOST_AT_ONCE - 1)));
        assert_eq!(range(Some("bytes=0-"), total), Some((0, MOST_AT_ONCE - 1)));
        assert_eq!(range(Some("bytes=100-199"), total), Some((100, 199)));
        assert_eq!(
            range(Some("bytes=5-"), 50),
            Some((5, 49)),
            "a small file whole"
        );
        assert_eq!(
            range(Some("bytes=40-90"), 50),
            Some((40, 49)),
            "cut at the end"
        );
        assert_eq!(range(Some("bytes=50-"), 50), None, "past the end");
        assert_eq!(range(Some("bytes=9-3"), 50), None);
        assert_eq!(range(Some("bytes=-20"), 50), None, "a suffix, never sent");
        assert_eq!(range(Some("items=0-"), 50), None);
        assert_eq!(range(Some("bytes=0-"), 0), None);
    }

    /// The bytes asked for, with where they sit in the whole.
    #[test]
    fn a_range_is_served_with_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.webm");
        let bytes: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        std::fs::write(&path, &bytes).unwrap();
        let picture = Picture {
            path,
            record: record(1),
        };
        let got = serve(&picture, Some("bytes=10-19"));
        assert_eq!(got.status, 206);
        assert_eq!(got.body, bytes[10..20]);
        let header = |name: &str| {
            got.headers
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(header("Content-Range").as_deref(), Some("bytes 10-19/1000"));
        assert_eq!(header("Content-Type").as_deref(), Some("video/webm"));
        assert_eq!(header("Content-Length").as_deref(), Some("10"));

        let whole = serve(&picture, None);
        assert_eq!(whole.body, bytes);
        assert_eq!(serve(&picture, Some("bytes=2000-")).status, 416);

        std::fs::remove_file(&picture.path).unwrap();
        assert_eq!(serve(&picture, None).status, 404);
    }

    /// **The server reads the record on the deck, in ranges, and nothing
    /// else**: over a real socket, as the webview asks.
    #[test]
    fn the_server_reads_only_the_deck_in_ranges() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.webm");
        let bytes: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        std::fs::write(&path, &bytes).unwrap();
        let other = dir.path().join("secret.webm");
        std::fs::write(&other, b"not yours").unwrap();

        let pictures = Arc::new(Pictures::new());
        pictures.loaded(1, record(1), &path);
        let address = pictures.address().unwrap();
        assert_eq!(pictures.address().unwrap(), address, "one server, kept");
        let base = address.trim_start_matches("http://");
        let (host, key) = base.split_once('/').unwrap();
        let ask = |target: &str, range: Option<&str>| -> String {
            let mut socket = std::net::TcpStream::connect(host).unwrap();
            let range = range.map(|r| format!("Range: {r}\r\n")).unwrap_or_default();
            write!(
                socket,
                "GET {target} HTTP/1.1\r\nHost: {host}\r\n{range}Connection: close\r\n\r\n"
            )
            .unwrap();
            let mut got = Vec::new();
            socket.read_to_end(&mut got).unwrap();
            String::from_utf8_lossy(&got).into_owned()
        };

        let hex = record(1).to_hex();
        let got = ask(&format!("/{key}/1/{hex}"), Some("bytes=100-109"));
        assert!(got.starts_with("HTTP/1.1 206"), "{got}");
        assert!(got.contains("Content-Range: bytes 100-109/5000"), "{got}");
        assert!(got.contains("Content-Type: video/webm"), "{got}");
        assert!(got.ends_with(&String::from_utf8_lossy(&bytes[100..110]).into_owned()));

        // The wrong key, another deck, another record, another file: nothing.
        for target in [
            format!("/nokey/1/{hex}"),
            format!("/{key}/2/{hex}"),
            format!("/{key}/1/{}", record(2).to_hex()),
            format!("/{key}/1/../secret.webm"),
        ] {
            assert!(ask(&target, None).starts_with("HTTP/1.1 404"), "{target}");
        }
        // Ejected, the record's own address is nothing too.
        pictures.ejected(1);
        assert!(ask(&format!("/{key}/1/{hex}"), None).starts_with("HTTP/1.1 404"));
    }
}

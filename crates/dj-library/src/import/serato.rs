//! Reading a Serato library.
//!
//! # Licensing
//!
//! Written from the published structural description of the format, not from
//! any existing implementation. [ADR-0002](../../../../docs/adr/0002-clean-room-permissive-licensing.md)
//! rules this out of `triseratops`, which is AGPL-3.0-or-later, so nothing here
//! is derived from it. The format below is a tagged-chunk container, and the
//! description is short enough to state in full:
//!
//! ```text
//! ┌──────────┬──────────┬───────────────────┐
//! │ tag      │ length   │ payload           │
//! │ 4 bytes  │ 4 bytes  │ `length` bytes    │
//! │ ASCII    │ u32 BE   │                   │
//! └──────────┴──────────┴───────────────────┘
//! ```
//!
//! repeated to the end of the file. A chunk whose tag begins with `o` holds
//! more chunks; everything else is a leaf. Text leaves are UTF-16 big-endian.
//!
//! # What lives where
//!
//! - `_Serato_/database V2` — every track Serato knows, as `otrk` chunks. The
//!   fields worth having are `pfil` (path), `tsng` (title), `tart` (artist),
//!   `talb` (album), `tgen` (genre), `tbpm` (tempo, as *text*), `tkey` (key).
//! - `_Serato_/Subcrates/<name>.crate` — one crate, as `otrk` chunks each
//!   holding a `ptrk` path. Nesting is spelled in the filename:
//!   `Latin%%Warm-up.crate` is "Warm-up" inside "Latin".
//!
//! Hot cues are **not** here. Serato writes them into the audio files
//! themselves, as a `GEOB` tag — so they arrive with the file rather than with
//! the library, and reading them belongs in the identify path rather than in an
//! importer. See the roadmap.

use super::{Collection, ImportPayload, ImportedPlaylist, ImportedTrack, Skipped, parse_key};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How Serato spells nesting in a crate's filename.
const CRATE_SEPARATOR: &str = "%%";

/// The extension a subcrate file has.
const CRATE_EXTENSION: &str = "crate";

/// A chunk header is a four-byte tag and a four-byte length.
const HEADER: usize = 8;

/// Refuse a chunk claiming to be larger than any real library file.
///
/// A corrupt or hostile length field would otherwise ask for a gigabyte
/// allocation from four bytes of input. Serato's own chunks are kilobytes; a
/// hundred megabytes is far past anything real and far short of dangerous.
const MAX_CHUNK: u32 = 100 * 1024 * 1024;

/// One chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub tag: [u8; 4],
    pub payload: &'a [u8],
}

impl Chunk<'_> {
    #[must_use]
    pub fn tag_str(&self) -> &str {
        std::str::from_utf8(&self.tag).unwrap_or("????")
    }

    /// Whether this chunk contains more chunks.
    #[must_use]
    pub fn is_container(&self) -> bool {
        self.tag[0] == b'o'
    }

    /// The payload as text.
    ///
    /// Serato writes UTF-16 big-endian. An odd-length payload is truncated
    /// rather than refused: half a character at the end of a filename is a
    /// corrupt file, and losing that character beats losing the track.
    #[must_use]
    pub fn text(&self) -> String {
        let units: Vec<u16> = self
            .payload
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units)
            .trim_end_matches('\0')
            .to_owned()
    }
}

/// Split a buffer into its chunks.
///
/// Stops at the first header that does not fit rather than erroring: Serato
/// files sometimes carry trailing padding, and a library that reads all but the
/// last few bytes is worth far more than one that refuses the file.
#[must_use]
pub fn chunks(data: &[u8]) -> Vec<Chunk<'_>> {
    let mut out = Vec::new();
    let mut at = 0usize;

    while at + HEADER <= data.len() {
        let tag = [data[at], data[at + 1], data[at + 2], data[at + 3]];
        let length = u32::from_be_bytes([data[at + 4], data[at + 5], data[at + 6], data[at + 7]]);
        if length > MAX_CHUNK {
            break;
        }
        let start = at + HEADER;
        let Some(end) = start.checked_add(length as usize) else {
            break;
        };
        if end > data.len() {
            break;
        }
        out.push(Chunk {
            tag,
            payload: &data[start..end],
        });
        at = end;
    }
    out
}

/// Read a whole Serato folder: the database and every subcrate.
///
/// `root` is the `_Serato_` folder itself.
pub fn read_folder(root: &Path) -> Collection {
    let mut out = Collection::default();

    // The database first, so crates can be matched against tracks it knows.
    match std::fs::read(root.join("database V2")) {
        Ok(data) => read_database(&data, &mut out),
        Err(error) => out.skipped.push(Skipped {
            what: format!("database V2 ({error})"),
            reason: "the Serato database could not be read",
        }),
    }

    read_subcrates(&root.join("Subcrates"), &mut out);
    out
}

/// Read `database V2`.
pub fn read_database(data: &[u8], out: &mut Collection) {
    for chunk in chunks(data) {
        if chunk.tag_str() != "otrk" {
            continue;
        }
        let fields: HashMap<String, Chunk<'_>> = chunks(chunk.payload)
            .into_iter()
            .map(|c| (c.tag_str().to_owned(), c))
            .collect();

        let Some(path) = fields.get("pfil").map(Chunk::text) else {
            out.skipped.push(Skipped {
                what: fields
                    .get("tsng")
                    .map_or_else(|| "a track".to_owned(), Chunk::text),
                reason: "the entry has no file location",
            });
            continue;
        };

        let text = |tag: &str| {
            fields
                .get(tag)
                .map(Chunk::text)
                .filter(|value| !value.trim().is_empty())
        };

        let mut payload = ImportPayload {
            // Serato stores the tempo as *text*, not as a number.
            bpm: text("tbpm").and_then(|b| b.parse().ok()),
            ..ImportPayload::default()
        };
        if let Some((hour, minor)) = text("tkey").as_deref().and_then(parse_key) {
            payload.key_hour = Some(hour);
            payload.key_minor = Some(minor);
        }

        out.tracks.push(ImportedTrack {
            path: normalise(&path),
            title: text("tsng"),
            artist: text("tart"),
            album: text("talb"),
            genre: text("tgen"),
            label: text("tlbl"),
            comment: text("tcom"),
            year: text("ttyr").and_then(|y| y.parse().ok()),
            payload,
            ..ImportedTrack::default()
        });
    }
}

/// Read every `.crate` in a `Subcrates` folder into a tree.
fn read_subcrates(dir: &Path, out: &mut Collection) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        // No subcrates is not a failure. A DJ can have a database and no
        // crates, and saying so would be noise.
        return;
    };

    // Sorted, so a parent is created before the children that name it and the
    // sidebar comes out in a stable order rather than the filesystem's.
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(CRATE_EXTENSION))
        })
        .collect();
    files.sort();

    // Folders are implied by the names rather than existing as files, so they
    // are created on demand and remembered by their full prefix.
    let mut folders: HashMap<String, usize> = HashMap::new();

    for file in files {
        let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(data) = std::fs::read(&file) else {
            out.skipped.push(Skipped {
                what: stem.to_owned(),
                reason: "the crate file could not be read",
            });
            continue;
        };

        let segments: Vec<&str> = stem.split(CRATE_SEPARATOR).collect();
        let (name, ancestors) = match segments.split_last() {
            Some((name, ancestors)) => (*name, ancestors),
            None => continue,
        };

        // Walk the implied folders, creating any that are new.
        let mut parent = None;
        let mut prefix = String::new();
        for segment in ancestors {
            if !prefix.is_empty() {
                prefix.push_str(CRATE_SEPARATOR);
            }
            prefix.push_str(segment);

            parent = Some(*folders.entry(prefix.clone()).or_insert_with(|| {
                let index = out.playlists.len();
                out.playlists.push(ImportedPlaylist {
                    name: (*segment).to_owned(),
                    parent,
                    is_folder: true,
                    paths: Vec::new(),
                });
                index
            }));
        }

        out.playlists.push(ImportedPlaylist {
            name: name.to_owned(),
            parent,
            is_folder: false,
            paths: crate_paths(&data),
        });
    }
}

/// The track paths in one crate file, in order.
#[must_use]
pub fn crate_paths(data: &[u8]) -> Vec<PathBuf> {
    chunks(data)
        .into_iter()
        .filter(|chunk| chunk.tag_str() == "otrk")
        .filter_map(|chunk| {
            chunks(chunk.payload)
                .into_iter()
                .find(|field| field.tag_str() == "ptrk")
                .map(|field| normalise(&field.text()))
        })
        .collect()
}

/// Serato stores paths relative to the volume the `_Serato_` folder is on.
///
/// On macOS that means `Users/dj/Music/a.flac` with no leading slash, which is
/// not a path anything can open. Restoring the slash is right far more often
/// than it is wrong, and a path that is already absolute is left alone.
fn normalise(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_end_matches('\0');
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains(':') {
        PathBuf::from(trimmed)
    } else {
        PathBuf::from(format!("/{trimmed}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a chunk, so the tests read as the format does.
    fn chunk(tag: &str, payload: &[u8]) -> Vec<u8> {
        let mut out = tag.as_bytes().to_vec();
        out.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    }

    fn field(tag: &str, text: &str) -> Vec<u8> {
        chunk(tag, &utf16(text))
    }

    #[test]
    fn chunks_are_split_by_their_headers() {
        let mut data = chunk("vrsn", &utf16("2.0/Serato ScratchLive Crate"));
        data.extend(chunk("otrk", &field("ptrk", "Users/dj/a.flac")));

        let found = chunks(&data);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].tag_str(), "vrsn");
        assert_eq!(found[0].text(), "2.0/Serato ScratchLive Crate");
        assert!(found[1].is_container());
    }

    /// A length field is four bytes of input asking for an allocation. A
    /// corrupt one must not be obeyed.
    #[test]
    fn an_impossible_length_stops_the_read_rather_than_allocating() {
        let mut data = chunk("otrk", &field("ptrk", "Users/dj/a.flac"));
        data.extend_from_slice(b"vrsn");
        data.extend_from_slice(&u32::MAX.to_be_bytes());

        let found = chunks(&data);
        assert_eq!(
            found.len(),
            1,
            "the good chunk is kept, the bad one stops it"
        );
    }

    /// Trailing bytes are common in real files and must not cost the library.
    #[test]
    fn trailing_padding_is_ignored() {
        let mut data = chunk("otrk", &field("ptrk", "Users/dj/a.flac"));
        data.extend_from_slice(&[0, 0, 0]);
        assert_eq!(chunks(&data).len(), 1);
    }

    #[test]
    fn a_chunk_claiming_more_than_the_file_holds_is_not_read() {
        let mut data = b"otrk".to_vec();
        data.extend_from_slice(&1000u32.to_be_bytes());
        data.extend_from_slice(b"short");
        assert!(chunks(&data).is_empty());
    }

    #[test]
    fn text_is_read_as_utf16_big_endian() {
        let data = field("tsng", "Bachata Rosa");
        assert_eq!(chunks(&data)[0].text(), "Bachata Rosa");
    }

    /// Half a character at the end of a filename is a corrupt file; losing the
    /// character beats losing the track.
    #[test]
    fn an_odd_length_text_payload_is_truncated_not_refused() {
        let mut payload = utf16("Bachata");
        payload.push(0);
        let data = chunk("tsng", &payload);
        assert_eq!(chunks(&data)[0].text(), "Bachata");
    }

    #[test]
    fn a_database_entry_becomes_a_track() {
        let mut entry = field("pfil", "Users/dj/Music/a.flac");
        entry.extend(field("tsng", "Bachata Rosa"));
        entry.extend(field("tart", "Juan Luis Guerra"));
        entry.extend(field("tgen", "Bachata"));
        entry.extend(field("tbpm", "128.00"));
        entry.extend(field("tkey", "Am"));
        let data = chunk("otrk", &entry);

        let mut out = Collection::default();
        read_database(&data, &mut out);

        assert_eq!(out.tracks.len(), 1);
        let track = &out.tracks[0];
        assert_eq!(track.path, PathBuf::from("/Users/dj/Music/a.flac"));
        assert_eq!(track.title.as_deref(), Some("Bachata Rosa"));
        assert_eq!(track.genre.as_deref(), Some("Bachata"));
        assert_eq!(
            track.payload.bpm,
            Some(128.0),
            "Serato writes the tempo as text"
        );
        assert_eq!(track.payload.key_hour, Some(8));
        assert_eq!(track.payload.key_minor, Some(true));
    }

    #[test]
    fn an_entry_with_no_path_is_reported() {
        let data = chunk("otrk", &field("tsng", "Ghost"));
        let mut out = Collection::default();
        read_database(&data, &mut out);

        assert!(out.tracks.is_empty());
        assert_eq!(out.skipped.len(), 1);
        assert_eq!(out.skipped[0].what, "Ghost");
    }

    #[test]
    fn crate_paths_come_out_in_order() {
        let mut data = chunk("vrsn", &utf16("1.0/Serato ScratchLive Crate"));
        data.extend(chunk("otrk", &field("ptrk", "Users/dj/b.flac")));
        data.extend(chunk("otrk", &field("ptrk", "Users/dj/a.flac")));

        assert_eq!(
            crate_paths(&data),
            vec![
                PathBuf::from("/Users/dj/b.flac"),
                PathBuf::from("/Users/dj/a.flac"),
            ],
            "a crate is a sequence the DJ chose"
        );
    }

    // -- paths -------------------------------------------------------------

    #[test]
    fn a_volume_relative_path_regains_its_leading_slash() {
        assert_eq!(
            normalise("Users/dj/Music/a.flac"),
            PathBuf::from("/Users/dj/Music/a.flac")
        );
    }

    #[test]
    fn an_absolute_or_windows_path_is_left_alone() {
        assert_eq!(normalise("/music/a.flac"), PathBuf::from("/music/a.flac"));
        assert_eq!(
            normalise("C:/Music/a.flac"),
            PathBuf::from("C:/Music/a.flac")
        );
    }

    // -- crate folders -----------------------------------------------------

    #[test]
    fn nesting_is_read_out_of_the_crate_filenames() {
        let dir = tempfile::tempdir().unwrap();
        let subcrates = dir.path().join("Subcrates");
        std::fs::create_dir_all(&subcrates).unwrap();

        let mut warmup = chunk("vrsn", &utf16("1.0/Serato ScratchLive Crate"));
        warmup.extend(chunk("otrk", &field("ptrk", "Users/dj/a.flac")));
        std::fs::write(subcrates.join("Latin%%Warm-up.crate"), &warmup).unwrap();
        std::fs::write(subcrates.join("Latin%%Peak.crate"), &warmup).unwrap();
        std::fs::write(subcrates.join("Closers.crate"), &warmup).unwrap();

        let mut out = Collection::default();
        read_subcrates(&subcrates, &mut out);

        let names: Vec<&str> = out.playlists.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Latin"),
            "the folder is implied by the names"
        );
        assert!(names.contains(&"Warm-up"));
        assert!(names.contains(&"Closers"));

        let latin = out
            .playlists
            .iter()
            .position(|p| p.name == "Latin")
            .unwrap();
        assert!(out.playlists[latin].is_folder);
        for child in ["Warm-up", "Peak"] {
            let index = out.playlists.iter().position(|p| p.name == child).unwrap();
            assert_eq!(out.playlists[index].parent, Some(latin), "{child}");
        }

        let closers = out.playlists.iter().find(|p| p.name == "Closers").unwrap();
        assert_eq!(closers.parent, None);
        assert_eq!(closers.paths.len(), 1);
    }

    /// Two crates in the same folder must share one folder, not make two.
    #[test]
    fn a_folder_is_created_once_however_many_crates_name_it() {
        let dir = tempfile::tempdir().unwrap();
        let subcrates = dir.path().join("Subcrates");
        std::fs::create_dir_all(&subcrates).unwrap();
        let data = chunk("vrsn", &utf16("1.0"));
        std::fs::write(subcrates.join("Latin%%A.crate"), &data).unwrap();
        std::fs::write(subcrates.join("Latin%%B.crate"), &data).unwrap();

        let mut out = Collection::default();
        read_subcrates(&subcrates, &mut out);

        assert_eq!(
            out.playlists.iter().filter(|p| p.name == "Latin").count(),
            1
        );
    }

    #[test]
    fn crates_nested_more_than_one_deep_work() {
        let dir = tempfile::tempdir().unwrap();
        let subcrates = dir.path().join("Subcrates");
        std::fs::create_dir_all(&subcrates).unwrap();
        std::fs::write(
            subcrates.join("Latin%%Bachata%%Slow.crate"),
            chunk("vrsn", &utf16("1.0")),
        )
        .unwrap();

        let mut out = Collection::default();
        read_subcrates(&subcrates, &mut out);

        let slow = out.playlists.iter().find(|p| p.name == "Slow").unwrap();
        let bachata_index = out
            .playlists
            .iter()
            .position(|p| p.name == "Bachata")
            .unwrap();
        assert_eq!(slow.parent, Some(bachata_index));
        assert_eq!(out.playlists[bachata_index].parent, Some(0));
        assert_eq!(out.playlists[0].name, "Latin");
    }

    #[test]
    fn a_missing_subcrates_folder_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Collection::default();
        read_subcrates(&dir.path().join("nope"), &mut out);
        assert!(out.playlists.is_empty());
        assert!(out.skipped.is_empty(), "no crates is not a failure");
    }

    #[test]
    fn a_whole_folder_reads_its_database_and_crates() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("_Serato_");
        std::fs::create_dir_all(root.join("Subcrates")).unwrap();

        let mut entry = field("pfil", "Users/dj/a.flac");
        entry.extend(field("tsng", "Bachata Rosa"));
        std::fs::write(root.join("database V2"), chunk("otrk", &entry)).unwrap();
        std::fs::write(
            root.join("Subcrates/Friday.crate"),
            chunk("otrk", &field("ptrk", "Users/dj/a.flac")),
        )
        .unwrap();

        let out = read_folder(&root);
        assert_eq!(out.tracks.len(), 1);
        assert_eq!(out.playlists.len(), 1);
        assert_eq!(out.playlists[0].name, "Friday");
        assert_eq!(
            out.playlists[0].paths,
            vec![PathBuf::from("/Users/dj/a.flac")]
        );
    }

    #[test]
    fn a_missing_database_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let out = read_folder(dir.path());
        assert!(out.tracks.is_empty());
        assert_eq!(out.skipped.len(), 1);
        assert!(out.skipped[0].what.contains("database"));
    }
}

//! Walking music folders into the library.
//!
//! # The cheap half and the slow half
//!
//! A track is identified by the hash of its decoded audio, so identifying one
//! costs a full decode — seconds per file, hours for a real collection. Doing
//! that before showing anything would mean a DJ who just installed djmanzo
//! stares at an empty browser all evening.
//!
//! So a scan does only the cheap half: walk the folders, read the tags, record
//! what is there. That takes seconds and the collection is immediately
//! browsable and searchable. Identification and analysis run afterwards, in the
//! background, promoting files out of `pending_files` into `tracks` as they
//! finish. See the schema for the table.
//!
//! Nothing in here decodes audio. That is deliberate and is the whole point.

use crate::record::Tags;
use crate::store::{Library, LibraryError};
use crate::tags;
use std::path::{Path, PathBuf};

/// Extensions worth opening. Matches what `dj-decode` can handle.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aiff", "aif", "ogg", "oga", "m4a", "aac", "opus", "wv", "alac",
];

/// How deep a scan will go.
///
/// Music collections nest — genre, artist, album, disc — but not indefinitely,
/// and a symlink loop otherwise turns a scan into a hang. Twelve is far past
/// any real collection and far short of forever.
pub const MAX_DEPTH: usize = 12;

/// What a scan did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Audio files found.
    pub found: usize,
    /// Files recorded for the first time.
    pub added: usize,
    /// Files whose size and modification time matched what was already known,
    /// so nothing was re-read.
    pub unchanged: usize,
    /// Directories that could not be read — a permissions problem, usually,
    /// or an unmounted drive. Counted rather than failing the whole scan: one
    /// unreadable folder should not cost a DJ the other nine hundred.
    pub unreadable_dirs: usize,
    /// Files whose tags could not be read. They are still recorded; the browser
    /// falls back to the filename.
    pub untaggable: usize,
}

/// One file found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub tags: Tags,
    pub file_size: Option<u64>,
    pub file_modified: Option<i64>,
}

/// Walk `root` and record every audio file in the library.
///
/// `now` is passed in rather than read from the clock so a scan is reproducible
/// in a test — the same argument the rest of the codebase takes for the same
/// reason.
pub fn scan_folder(library: &Library, root: &Path, now: i64) -> Result<ScanReport, LibraryError> {
    let mut report = ScanReport::default();
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    // An explicit stack rather than recursion: a pathological directory tree
    // should exhaust a `Vec`, which is recoverable, rather than the call stack,
    // which is not.
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            report.unreadable_dirs += 1;
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            // `file_type` on the entry rather than `metadata` on the path: it
            // does not follow symlinks, so a link pointing at its own parent is
            // seen as a link and skipped rather than walked forever.
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if !is_audio(&path) {
                continue;
            }

            report.found += 1;
            let metadata = entry.metadata().ok();
            let file_size = metadata.as_ref().map(std::fs::Metadata::len);
            let file_modified = metadata.as_ref().and_then(modified_unix_seconds);

            if library.file_is_unchanged(&path, file_size, file_modified)? {
                report.unchanged += 1;
                continue;
            }

            let tags = match tags::read(&path) {
                Ok(tags) => tags,
                Err(error) => {
                    // Not a reason to skip the file. A DJ's oldest and most
                    // played records are often the ones with broken tags, and
                    // the filename is usually enough to find them by.
                    tracing::debug!(?path, %error, "could not read tags");
                    report.untaggable += 1;
                    Tags::default()
                }
            };

            library.record_pending(
                &ScannedFile {
                    path,
                    tags,
                    file_size,
                    file_modified,
                },
                now,
            )?;
            report.added += 1;
        }
    }

    Ok(report)
}

/// Walk every folder the library is watching.
pub fn scan_all(library: &Library, now: i64) -> Result<ScanReport, LibraryError> {
    let mut total = ScanReport::default();
    for folder in library.folders()? {
        let report = scan_folder(library, &folder, now)?;
        total.found += report.found;
        total.added += report.added;
        total.unchanged += report.unchanged;
        total.unreadable_dirs += report.unreadable_dirs;
        total.untaggable += report.untaggable;
    }
    Ok(total)
}

#[must_use]
pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.as_str()))
}

/// Modification time as unix seconds.
///
/// `None` for a file whose timestamp predates 1970 or that the filesystem does
/// not report — both of which mean "cannot skip this on the next scan", which
/// is the safe answer.
fn modified_unix_seconds(metadata: &std::fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn library() -> Library {
        Library::in_memory().unwrap()
    }

    /// A tree with music in it, plus the things a real folder also contains.
    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("one.mp3"), b"not really an mp3").unwrap();
        fs::write(dir.path().join("two.FLAC"), b"not really a flac").unwrap();
        fs::write(dir.path().join("cover.jpg"), b"jpeg").unwrap();
        fs::write(dir.path().join("notes.txt"), b"text").unwrap();

        let nested = dir.path().join("latin/bachata");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("three.wav"), b"not really a wav").unwrap();
        dir
    }

    #[test]
    fn a_scan_finds_audio_and_ignores_everything_else() {
        let lib = library();
        let dir = tree();
        let report = scan_folder(&lib, dir.path(), 100).unwrap();
        assert_eq!(report.found, 3, "two at the top, one nested");
        assert_eq!(report.added, 3);
    }

    #[test]
    fn extensions_are_matched_case_insensitively() {
        assert!(is_audio(Path::new("/music/Track.FLAC")));
        assert!(is_audio(Path::new("/music/track.mp3")));
        assert!(!is_audio(Path::new("/music/cover.jpg")));
        assert!(!is_audio(Path::new("/music/no-extension")));
    }

    /// The property that makes a rescan cheap: a file that cannot have changed
    /// is not re-read.
    #[test]
    fn rescanning_skips_files_that_have_not_changed() {
        let lib = library();
        let dir = tree();
        scan_folder(&lib, dir.path(), 100).unwrap();

        let second = scan_folder(&lib, dir.path(), 200).unwrap();
        assert_eq!(second.found, 3);
        assert_eq!(second.unchanged, 3);
        assert_eq!(second.added, 0);
    }

    /// And the other half: a file that *has* changed is re-read, or an edited
    /// tag would never reach the browser.
    #[test]
    fn rescanning_re_reads_a_file_that_changed() {
        let lib = library();
        let dir = tree();
        scan_folder(&lib, dir.path(), 100).unwrap();

        fs::write(dir.path().join("one.mp3"), b"a longer file than before").unwrap();
        let second = scan_folder(&lib, dir.path(), 200).unwrap();
        assert_eq!(second.added, 1);
        assert_eq!(second.unchanged, 2);
    }

    /// A file with no readable tags is still recorded. The browser falls back
    /// to the filename, which is how most hand-organised collections work.
    #[test]
    fn a_file_with_unreadable_tags_is_still_recorded() {
        let lib = library();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("track.mp3"), b"definitely not an mp3").unwrap();

        let report = scan_folder(&lib, dir.path(), 100).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.untaggable, 1);
        assert_eq!(lib.pending_count().unwrap(), 1);
    }

    #[test]
    fn scanning_a_folder_that_does_not_exist_is_reported_not_fatal() {
        let lib = library();
        let report = scan_folder(&lib, Path::new("/definitely/not/here"), 100).unwrap();
        assert_eq!(report.unreadable_dirs, 1);
        assert_eq!(report.found, 0);
    }

    /// A symlink loop must not hang the scan. This is the failure that turns a
    /// scan into a frozen application with no error to show.
    #[test]
    #[cfg(unix)]
    fn a_symlink_loop_does_not_hang_the_scan() {
        let lib = library();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("track.mp3"), b"audio").unwrap();
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        std::os::unix::fs::symlink(dir.path(), nested.join("loop")).unwrap();

        let report = scan_folder(&lib, dir.path(), 100).unwrap();
        assert_eq!(report.found, 1, "the loop must be walked once, not forever");
    }

    #[test]
    fn scan_all_walks_every_watched_folder() {
        let lib = library();
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fs::write(first.path().join("a.mp3"), b"x").unwrap();
        fs::write(second.path().join("b.flac"), b"y").unwrap();
        lib.add_folder(first.path(), 0).unwrap();
        lib.add_folder(second.path(), 0).unwrap();

        assert_eq!(scan_all(&lib, 100).unwrap().added, 2);
    }
}

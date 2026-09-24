//! §111: the downloads folder, watched, and new music filed into the
//! collection.
//!
//! > have configurable music/download folder for that purpose and use
//! > internal AI to auto-create subfolders on buying/downloading music
//!
//! A DJ buys a record in their browser (`dj_sources::stores` sends them there)
//! and the store saves it wherever the browser saves things. djmanzo watches
//! that folder, waits until a file has stopped growing, reads its tags, and
//! moves it into the DJ's music folder under a folder chosen for it — then
//! adds it to the library, so the record bought a minute ago is in the
//! collection by the time the DJ turns back to the decks.
//!
//! # How a folder is chosen
//!
//! By what djmanzo already knows about music, not by a model guessing. The
//! genre tag is read through `dj_core::genre`'s map of the families that
//! fill floors — "Deep House" and "house" are one family, "Drum & Bass" and
//! "dnb" another — and the record goes under the family and then its artist:
//! `House/Kerri Chandler/`. A backing track or an a cappella goes to
//! `Karaoke/` or `A cappellas/` first, whatever its genre, because that is
//! what a karaoke host looks for it as. A record whose genre djmanzo does not
//! recognise keeps the tag's own spelling as its folder; one with no genre at
//! all goes to `Unsorted/`, which is honest, and a DJ sorting later knows
//! where to look.
//!
//! # What it will not do
//!
//! It does not touch a file still being written (a `.part`, a
//! `.crdownload`, or anything whose size is still changing), it never
//! overwrites — a second copy of a name is `Title (2).mp3` — and it moves only
//! audio.
//!
//! # An album in a `.zip`
//!
//! Bandcamp, Beatport and most stores deliver an album as one `.zip`. Its
//! records come out of it one by one and are filed like any other record;
//! everything else in it — the cover, a booklet, the resource forks a Mac
//! leaves in `__MACOSX/` — stays in the archive. The archive itself is then put
//! away, not deleted, in `Unpacked/` inside the downloads folder: it is what the
//! DJ paid for, and a record that could not be filed is still in it.
//!
//! An archive is a file a stranger built, so unpacking one is careful about
//! three things. **Nothing is written outside the folder it is unpacked into**:
//! a record is written under its own file name alone, and an entry whose name
//! climbs out (`../../.bashrc.mp3`) or starts at the root is refused, not
//! tidied. **Nothing unpacks without end**: a record stops at the most a
//! record can be and the album at the most an album can be ([`LIMITS`]),
//! counted in the bytes actually written, not the sizes the archive claims.
//! And **a `.zip` with no records in it is not touched** — the downloads
//! folder holds all sorts of archives that are not music, and those are none
//! of djmanzo's business.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What the DJ has chosen: which folder to watch, where to file into, and
/// whether to do it at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub watch: Option<PathBuf>,
    pub into: Option<PathBuf>,
    pub on: bool,
}

/// One record filed, for the list the DJ can look at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Filed {
    /// The file's name as it arrived.
    pub arrived: String,
    /// Where it went, relative to the music folder.
    pub to: String,
    /// Seconds since the epoch.
    pub at: i64,
    /// Why it could not be filed, when it could not. It then stays where it
    /// arrived.
    pub problem: Option<String>,
}

/// Audio a DJ plays, by extension.
const AUDIO: [&str; 10] = [
    "mp3", "flac", "wav", "aif", "aiff", "m4a", "aac", "ogg", "opus", "alac",
];

/// What browsers call a download still in progress.
const PARTIAL: [&str; 5] = ["part", "crdownload", "download", "tmp", "partial"];

/// Whether a file's name hides it: a dot file, or a name that is not text.
fn hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.starts_with('.'))
}

/// A path's extension, lower-cased; empty when it has none.
fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

/// Whether a path is a finished audio file worth looking at.
#[must_use]
pub fn is_audio(path: &Path) -> bool {
    let extension = extension(path);
    !hidden(path) && AUDIO.contains(&extension.as_str()) && !PARTIAL.contains(&extension.as_str())
}

/// Whether a path is a finished `.zip`, which is how a store delivers an
/// album. Whether it *is* an album is known only once it is opened.
#[must_use]
pub fn is_album(path: &Path) -> bool {
    !hidden(path) && extension(path) == "zip"
}

/// Watches one folder for files that have finished arriving.
///
/// A file is finished when two looks some seconds apart see it at the same,
/// non-zero size. Browsers write a download under a partial name and rename
/// it at the end, but not every store's helper does, and a file moved while
/// it is still being written is a truncated record in the collection.
#[derive(Debug, Default)]
pub struct Watcher {
    sizes: HashMap<PathBuf, u64>,
    /// Files answered once and left where they are, at the size they were
    /// left at: not answered again until they change.
    left: HashMap<PathBuf, u64>,
}

impl Watcher {
    /// Look once, and answer the files that were the same size last time.
    ///
    /// A file answered is forgotten, so a file that cannot be moved is
    /// answered once per two looks rather than on every one.
    pub fn look(&mut self, folder: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(folder) else {
            self.sizes.clear();
            self.left.clear();
            return Vec::new();
        };
        let mut now = HashMap::new();
        let mut still_left = HashMap::new();
        let mut ready = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_audio(&path) && !is_album(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() || meta.len() == 0 {
                continue;
            }
            if self.left.get(&path) == Some(&meta.len()) {
                still_left.insert(path, meta.len());
            } else if self.sizes.get(&path) == Some(&meta.len()) {
                ready.push(path);
            } else {
                now.insert(path, meta.len());
            }
        }
        self.sizes = now;
        self.left = still_left;
        ready.sort();
        ready
    }

    /// Stop answering a file until it changes. A `.zip` that holds no
    /// records is left where it is, and without this it would be opened
    /// again every few seconds for as long as it sat there.
    pub fn leave(&mut self, path: &Path) {
        if let Ok(meta) = std::fs::metadata(path) {
            self.left.insert(path.to_path_buf(), meta.len());
        }
    }
}

/// Keep a name a folder can hold: no separators, no characters a file
/// system refuses, no leading dots, and not unreasonably long.
fn folder_name(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '.' || c == ' ' || c == '-')
        .to_owned();
    trimmed.chars().take(80).collect()
}

/// Title-case a family name for a folder: `drum and bass` → `Drum And Bass`
/// would be wrong, so only the first letter of each word, and the family's
/// own spelling otherwise.
fn family_folder(name: &str) -> String {
    name.split(' ')
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + letters.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a title or file name says the record is, before its genre does.
fn kind_folder(words: &str) -> Option<&'static str> {
    let words = words.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|needle| words.contains(needle));
    if any(&[
        "karaoke",
        "backing track",
        "instrumental version",
        "minus one",
    ]) {
        Some("Karaoke")
    } else if any(&["acapella", "a cappella", "a capella", "acappella"]) {
        Some("A cappellas")
    } else {
        None
    }
}

/// Where a record belongs under the music folder, as a relative path ending
/// in its file name. See the module note for the rule.
#[must_use]
pub fn place(tags: &dj_library::Tags, file_name: &str) -> PathBuf {
    let title = tags.title.as_deref().unwrap_or_default();
    let top = kind_folder(&format!("{title} {file_name}")).map_or_else(
        || match tags
            .genre
            .as_deref()
            .map(str::trim)
            .filter(|g| !g.is_empty())
        {
            Some(genre) => dj_core::genre::family_for(genre)
                .map_or_else(|| folder_name(genre), |family| family_folder(family.name)),
            None => "Unsorted".to_owned(),
        },
        str::to_owned,
    );
    let artist = tags
        .album_artist
        .as_deref()
        .or(tags.artist.as_deref())
        .map(folder_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unknown artist".to_owned());
    let top = if top.is_empty() {
        "Unsorted".to_owned()
    } else {
        top
    };
    PathBuf::from(top).join(artist).join(file_name)
}

/// A path that does not exist yet: `Title.mp3`, else `Title (2).mp3`, and on.
#[must_use]
pub fn unused(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| path.with_file_name(format!("{stem} ({n}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

/// Move a file to `into.join(relative)`, making the folders it needs and
/// never overwriting. A rename where the two are on one disk; a copy and a
/// removal where they are not.
///
/// # Errors
/// When the folders cannot be made or the file cannot be moved. The file
/// is then where it was.
pub fn file(from: &Path, into: &Path, relative: &Path) -> std::io::Result<PathBuf> {
    let target = unused(&into.join(relative));
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, &target).is_err() {
        std::fs::copy(from, &target)?;
        std::fs::remove_file(from)?;
    }
    Ok(target)
}

/// How much an album may unpack to, counted in bytes written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// One record.
    pub record: u64,
    /// The whole album.
    pub album: u64,
}

/// A record stops at 4 GiB, the most a WAV file can hold — two hours of
/// 24-bit, 96 kHz stereo — and an album at 16 GiB, several hi-res albums'
/// worth. A store's album is a few hundred megabytes; an archive that
/// unpacks past these was built to fill a disk.
pub const LIMITS: Limits = Limits {
    record: 4 << 30,
    album: 16 << 30,
};

/// One record out of an album: where it was unpacked to, or why it was not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpacked {
    /// The record's file name, as the archive names it.
    pub name: String,
    pub to: Result<PathBuf, String>,
}

/// The last part of an entry's name, whichever separator the machine that
/// built the archive used.
fn last_part(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// A size in the unit a person reads it in.
fn gigabytes(bytes: u64) -> String {
    format!("{} GB", bytes.div_ceil(1 << 30))
}

/// Unpack an album's records into `staging`, each under its own file name,
/// and nothing else.
///
/// An entry that is not audio is skipped without being read. An audio entry
/// whose name climbs out of the archive is refused; one that unpacks past
/// `limits`, is damaged, or is compressed or encrypted in a way djmanzo does
/// not read is refused and nothing of it is left behind. Two records with
/// one file name (`CD1/01 Intro.mp3`, `CD2/01 Intro.mp3`) are both kept.
///
/// # Errors
/// When the archive cannot be opened as a `.zip` at all.
pub fn unpack(archive: &Path, staging: &Path, limits: Limits) -> std::io::Result<Vec<Unpacked>> {
    use std::io::Read as _;
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(archive)?).map_err(std::io::Error::other)?;
    let mut records = Vec::new();
    let mut left = limits.album;
    for index in 0..zip.len() {
        let Some(name) = zip.name_for_index(index).map(str::to_owned) else {
            continue;
        };
        let file_name = last_part(&name).to_owned();
        if !is_audio(Path::new(&file_name)) {
            continue;
        }
        let refuse = |why: String| Unpacked {
            name: file_name.clone(),
            to: Err(why),
        };
        let mut entry = match zip.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                records.push(refuse(format!("it could not be unpacked: {error}")));
                continue;
            }
        };
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        if entry.enclosed_name().is_none() {
            records.push(refuse(format!(
                "its name in the archive, {name:?}, points outside the album"
            )));
            continue;
        }
        let cap = limits.record.min(left);
        if let Err(error) = std::fs::create_dir_all(staging) {
            records.push(refuse(error.to_string()));
            continue;
        }
        let target = unused(&staging.join(&file_name));
        let written = std::fs::File::create(&target).and_then(|mut out| {
            std::io::copy(&mut (&mut entry).take(cap.saturating_add(1)), &mut out)
        });
        let problem = match written {
            Ok(bytes) if bytes <= cap => {
                left -= bytes;
                records.push(Unpacked {
                    name: file_name.clone(),
                    to: Ok(target),
                });
                continue;
            }
            Ok(_) if cap < limits.record => format!(
                "the album unpacks to more than {}, and this is past it",
                gigabytes(limits.album)
            ),
            Ok(_) => format!(
                "it unpacks to more than {}, more than a record can be",
                gigabytes(limits.record)
            ),
            Err(error) => format!("it could not be unpacked: {error}"),
        };
        let _ = std::fs::remove_file(&target);
        records.push(refuse(problem));
    }
    Ok(records)
}

/// One arrived record moved to its place: what to show the DJ, and the
/// folder it went into when it went.
fn put(arrived: &Path, into: &Path, shown: String, now: i64) -> (Filed, Option<PathBuf>) {
    let name = arrived
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tags = dj_library::tags::read(arrived).unwrap_or_default();
    let relative = place(&tags, &name);
    match file(arrived, into, &relative) {
        Ok(target) => (
            Filed {
                arrived: shown,
                to: target
                    .strip_prefix(into)
                    .unwrap_or(&target)
                    .to_string_lossy()
                    .into_owned(),
                at: now,
                problem: None,
            },
            target.parent().map(Path::to_path_buf),
        ),
        Err(error) => (
            Filed {
                arrived: shown,
                to: String::new(),
                at: now,
                problem: Some(error.to_string()),
            },
            None,
        ),
    }
}

/// An album taken out of its archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album {
    /// Each record, filed or not, in the archive's order.
    pub filed: Vec<Filed>,
    /// The folders records went into, each once.
    pub folders: Vec<PathBuf>,
    /// Where the archive was put away; `None` when it could not be moved
    /// and is still where it arrived.
    pub kept: Option<PathBuf>,
}

/// The folder in the downloads folder an unpacked archive is put away in.
pub const UNPACKED: &str = "Unpacked";

/// Take an album out of its archive and file each record, then put the
/// archive away in [`UNPACKED`] beside it.
///
/// `None` when the archive is not an album — it cannot be opened, or holds
/// no records — and it is then left exactly where it is.
#[must_use]
pub fn take_album(archive: &Path, into: &Path, limits: Limits, now: i64) -> Option<Album> {
    let downloads = archive.parent()?;
    let archive_name = archive.file_name()?.to_string_lossy().into_owned();
    // Beside the archive, so a record moves once, from here to its place.
    // Hidden, and a folder: the watcher looks at neither.
    let staging = downloads.join(format!(".djmanzo-unpacking-{archive_name}"));
    let _ = std::fs::remove_dir_all(&staging);
    let unpacked = unpack(archive, &staging, limits);
    let records = match unpacked {
        Ok(records) if !records.is_empty() => records,
        _ => {
            let _ = std::fs::remove_dir_all(&staging);
            return None;
        }
    };
    let mut filed = Vec::with_capacity(records.len());
    let mut folders: Vec<PathBuf> = Vec::new();
    for record in records {
        let shown = format!("{archive_name} › {}", record.name);
        match record.to {
            Ok(path) => {
                let (filing, folder) = put(&path, into, shown, now);
                if let Some(folder) = folder
                    && !folders.contains(&folder)
                {
                    folders.push(folder);
                }
                filed.push(filing);
            }
            Err(problem) => filed.push(Filed {
                arrived: shown,
                to: String::new(),
                at: now,
                problem: Some(problem),
            }),
        }
    }
    let _ = std::fs::remove_dir_all(&staging);
    let kept = file(archive, downloads, &Path::new(UNPACKED).join(&archive_name)).ok();
    Some(Album {
        filed,
        folders,
        kept,
    })
}

/// How often the downloads folder is looked at. Two looks see a finished
/// file, so a purchase is in the collection within about ten seconds of the
/// browser finishing it.
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(4);

/// How many filings the DJ can look back over.
pub const REMEMBERED: usize = 30;

/// Watch the downloads folder for as long as djmanzo runs.
///
/// A thread of its own, looking every few seconds: the work is a directory
/// listing and, now and then, a move and a scan of one folder — nothing that
/// comes near the audio thread, and nothing a notification library would
/// save enough of to be worth a dependency.
pub fn start(handle: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("downloads".into())
        .spawn(move || {
            use tauri::Manager;
            let mut watcher = Watcher::default();
            let mut watching: Option<PathBuf> = None;
            loop {
                std::thread::sleep(LOOK_EVERY);
                let state: tauri::State<'_, crate::state::AppState> = handle.state();
                let settings = state.downloads();
                let (Some(watch), Some(into), true) = (settings.watch, settings.into, settings.on)
                else {
                    watching = None;
                    continue;
                };
                // A different folder is a fresh start: sizes seen in the old
                // one say nothing about files in the new.
                if watching.as_ref() != Some(&watch) {
                    watcher = Watcher::default();
                    watching = Some(watch.clone());
                }
                for arrived in watcher.look(&watch) {
                    if !is_album(&arrived) {
                        let now = crate::library::now_seconds();
                        let shown = arrived
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let (filed, folder) = put(&arrived, &into, shown, now);
                        into_library(&state, &into, folder.as_slice(), now);
                        state.note_filed(filed);
                        continue;
                    }
                    let now = crate::library::now_seconds();
                    let Some(album) = take_album(&arrived, &into, LIMITS, now) else {
                        watcher.leave(&arrived);
                        continue;
                    };
                    into_library(&state, &into, &album.folders, now);
                    // Newest first in the list, so the last pushed is at
                    // the top: pushed backwards, the album reads in order.
                    for filed in album.filed.into_iter().rev() {
                        state.note_filed(filed);
                    }
                    if album.kept.is_none() {
                        watcher.leave(&arrived);
                    }
                }
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for the downloads folder; purchases will not be filed");
    }
}

/// Put what was just filed into the collection now, rather than at the next
/// rescan: the point of filing it is that the record bought a minute ago is
/// there when the DJ looks.
fn into_library(state: &crate::state::AppState, into: &Path, folders: &[PathBuf], now: i64) {
    if folders.is_empty() {
        return;
    }
    let Ok(db) = state.library().get() else {
        return;
    };
    let _ = db.add_folder(into, now);
    for folder in folders {
        if let Err(error) = dj_library::scan_folder(&db, folder, now) {
            tracing::warn!(%error, ?folder, "filed, but the library did not take it");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(artist: &str, title: &str, genre: Option<&str>) -> dj_library::Tags {
        dj_library::Tags {
            artist: (!artist.is_empty()).then(|| artist.to_owned()),
            title: Some(title.to_owned()),
            genre: genre.map(str::to_owned),
            ..dj_library::Tags::default()
        }
    }

    /// **The load-bearing one: a record goes under its family and its
    /// artist**, whatever spelling of the genre the tag used.
    #[test]
    fn a_record_is_filed_under_its_family_and_artist() {
        assert_eq!(
            place(
                &tags("Kerri Chandler", "Rain", Some("Deep House")),
                "rain.mp3"
            ),
            PathBuf::from("House/Kerri Chandler/rain.mp3")
        );
        assert_eq!(
            place(
                &tags("Romeo Santos", "Propuesta Indecente", Some("bachata")),
                "p.flac"
            ),
            PathBuf::from("Bachata/Romeo Santos/p.flac")
        );
        // "Drum & Bass" and "dnb" are one family, one folder.
        assert_eq!(
            place(&tags("A", "B", Some("Drum & Bass")), "x.mp3").parent(),
            place(&tags("A", "B", Some("dnb")), "x.mp3").parent()
        );
    }

    /// A backing track or an a cappella is filed as what a karaoke host looks
    /// for, before its genre.
    #[test]
    fn karaoke_and_a_cappellas_are_filed_as_such() {
        assert_eq!(
            place(
                &tags("Queen", "Bohemian Rhapsody (Karaoke Version)", Some("Rock")),
                "bohemian.mp3"
            ),
            PathBuf::from("Karaoke/Queen/bohemian.mp3")
        );
        assert_eq!(
            place(
                &tags("Someone", "Hook", Some("House")),
                "Hook (Acapella).wav"
            ),
            PathBuf::from("A cappellas/Someone/Hook (Acapella).wav")
        );
    }

    /// Nothing to go on is `Unsorted`, an unknown genre keeps its own
    /// spelling, and no tag can put a file outside the music folder.
    #[test]
    fn what_is_unknown_is_said_and_nothing_escapes() {
        assert_eq!(
            place(&tags("", "Untitled", None), "u.mp3"),
            PathBuf::from("Unsorted/Unknown artist/u.mp3")
        );
        assert_eq!(
            place(&tags("X", "Y", Some("Vaporwave Polka")), "y.mp3"),
            PathBuf::from("Vaporwave Polka/X/y.mp3")
        );
        let sneaky = place(&tags("../../etc", "t", Some("../..")), "t.mp3");
        assert!(
            sneaky
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
            "{sneaky:?}"
        );
    }

    /// **A file still arriving is left alone**: it is answered only once two
    /// looks see it at the same size, and a partial download never.
    #[test]
    fn only_a_finished_file_is_answered() {
        let dir = std::env::temp_dir().join(format!("djmanzo-downloads-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let song = dir.join("song.mp3");
        std::fs::write(&song, b"abc").unwrap();
        std::fs::write(dir.join("other.mp3.part"), b"abc").unwrap();
        std::fs::write(dir.join("other.zip.crdownload"), b"abc").unwrap();
        std::fs::write(dir.join("cover.jpg"), b"abc").unwrap();
        let album = dir.join("Album.ZIP");
        std::fs::write(&album, b"abcdef").unwrap();

        let mut watcher = Watcher::default();
        assert!(watcher.look(&dir).is_empty(), "answered on first sight");
        std::fs::write(&song, b"abcdef").unwrap();
        // The song grew between the looks; the archive did not.
        assert_eq!(
            watcher.look(&dir),
            vec![album.clone()],
            "the song answered while it grew"
        );
        assert_eq!(watcher.look(&dir), vec![song.clone()]);

        // An archive answered and still there is answered again, once per
        // two looks, as any file is ...
        std::fs::remove_file(&song).unwrap();
        assert_eq!(watcher.look(&dir), vec![album.clone()]);
        // ... but one left where it is, not until it changes.
        watcher.leave(&album);
        for _ in 0..3 {
            assert!(
                watcher.look(&dir).is_empty(),
                "a left archive answered again"
            );
        }
        std::fs::write(&album, b"abcdefgh").unwrap();
        assert!(watcher.look(&dir).is_empty());
        assert_eq!(
            watcher.look(&dir),
            vec![album.clone()],
            "changed, and never answered"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder of its own for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("djmanzo-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An archive holding these entries, deflated as a store's are.
    fn archive(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap();
    }

    /// Every file under a folder, relative to it.
    fn every_file(dir: &Path) -> Vec<String> {
        let mut found = Vec::new();
        let mut folders = vec![dir.to_path_buf()];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    found.push(
                        path.strip_prefix(dir)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        found.sort();
        found
    }

    /// **Only the records leave the archive, and nothing leaves the folder
    /// they are unpacked into.** The cover, the booklet and a Mac's resource
    /// forks stay in; two records with one name are both kept; an entry
    /// whose name climbs out is refused and written nowhere.
    #[test]
    fn only_the_records_leave_an_album() {
        let dir = scratch("unpack");
        let (zip_path, staging) = (dir.join("Album.zip"), dir.join("in").join("staging"));
        archive(
            &zip_path,
            &[
                ("Album/01 Intro.mp3", b"one"),
                ("Album/cover.jpg", b"picture"),
                ("Album/booklet.pdf", b"words"),
                ("__MACOSX/Album/._01 Intro.mp3", b"fork"),
                ("CD2/01 Intro.mp3", b"two"),
                ("Album/", b""),
                ("../../evil.mp3", b"escaped"),
                ("/etc/rooted.flac", b"rooted"),
                ("CD2\\02 Outro.flac", b"windows"),
            ],
        );
        let records = unpack(&zip_path, &staging, LIMITS).unwrap();
        let names: Vec<(&str, bool)> = records
            .iter()
            .map(|record| (record.name.as_str(), record.to.is_ok()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("01 Intro.mp3", true),
                ("01 Intro.mp3", true),
                ("evil.mp3", false),
                ("rooted.flac", false),
                ("02 Outro.flac", true),
            ]
        );
        assert!(
            records[2]
                .to
                .as_ref()
                .unwrap_err()
                .contains("outside the album"),
            "{records:?}"
        );
        assert_eq!(
            every_file(&dir),
            vec![
                "Album.zip",
                "in/staging/01 Intro (2).mp3",
                "in/staging/01 Intro.mp3",
                "in/staging/02 Outro.flac",
            ]
        );
        assert_eq!(std::fs::read(staging.join("01 Intro.mp3")).unwrap(), b"one");
        assert_eq!(
            std::fs::read(staging.join("01 Intro (2).mp3")).unwrap(),
            b"two"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **An archive does not unpack without end.** A record past the most a
    /// record can be is refused — here, a few kilobytes of silence that
    /// deflates to almost nothing, a zip bomb in miniature — and the album
    /// stops at the most an album can be, counted in bytes written. A
    /// damaged record is refused too. Nothing refused is left half-written.
    #[test]
    fn an_album_stops_at_its_limits() {
        let dir = scratch("limits");
        let (zip_path, staging) = (dir.join("Big.zip"), dir.join("staging"));
        let record = vec![7_u8; 900];
        let silence = vec![0_u8; 50_000];
        archive(
            &zip_path,
            &[
                ("a.mp3", &record),
                ("bomb.wav", &silence),
                ("b.mp3", &record),
                ("c.mp3", &record),
            ],
        );
        let limits = Limits {
            record: 1_000,
            album: 2_500,
        };
        assert!(
            std::fs::metadata(&zip_path).unwrap().len() < 2_500,
            "the bomb must be small packed for this to test anything"
        );
        let records = unpack(&zip_path, &staging, limits).unwrap();
        let outcome: Vec<(&str, Result<(), &str>)> = records
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.to.as_ref().map(|_| ()).map_err(String::as_str),
                )
            })
            .collect();
        assert_eq!(outcome[0], ("a.mp3", Ok(())));
        assert!(
            outcome[1]
                .1
                .is_err_and(|why| why.contains("more than a record can be")),
            "{outcome:?}"
        );
        assert_eq!(outcome[2], ("b.mp3", Ok(())));
        assert!(
            outcome[3]
                .1
                .is_err_and(|why| why.contains("the album unpacks to more")),
            "{outcome:?}"
        );
        assert_eq!(every_file(&staging), vec!["a.mp3", "b.mp3"]);

        // A record whose bytes do not match its checksum is refused.
        let damaged = dir.join("Damaged.zip");
        let mut body = b"MARKER-".repeat(40);
        body.extend_from_slice(b"end");
        {
            use std::io::Write as _;
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&damaged).unwrap());
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("d.mp3", stored).unwrap();
            zip.write_all(&body).unwrap();
            zip.finish().unwrap();
        }
        let mut bytes = std::fs::read(&damaged).unwrap();
        let at = bytes.windows(3).position(|w| w == b"end").unwrap();
        bytes[at] = b'E';
        std::fs::write(&damaged, bytes).unwrap();
        let damaged_staging = dir.join("damaged-staging");
        let records = unpack(&damaged, &damaged_staging, LIMITS).unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].to.is_err(), "{records:?}");
        assert!(every_file(&dir.join("damaged-staging")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **An album is filed record by record and the archive put away**, never
    /// deleted and never overwritten: the same album bought twice is two
    /// archives in `Unpacked/`. What was unpacked but not filed is gone from
    /// the downloads folder too, since it is still in the archive.
    #[test]
    fn an_album_is_filed_and_its_archive_put_away() {
        let dir = scratch("album");
        let (downloads, music) = (dir.join("downloads"), dir.join("music"));
        std::fs::create_dir_all(&downloads).unwrap();
        for round in 1..=2 {
            let zip_path = downloads.join("Night Album.zip");
            archive(
                &zip_path,
                &[
                    ("Night Album/01 First.mp3", b"first"),
                    ("Night Album/02 Second.flac", b"second"),
                    ("Night Album/cover.jpg", b"picture"),
                ],
            );
            let album = take_album(&zip_path, &music, LIMITS, 1_700_000_000).unwrap();
            let shown: Vec<(&str, &str)> = album
                .filed
                .iter()
                .map(|f| (f.arrived.as_str(), f.to.as_str()))
                .collect();
            let (first, second) = if round == 1 {
                (
                    "Unsorted/Unknown artist/01 First.mp3",
                    "Unsorted/Unknown artist/02 Second.flac",
                )
            } else {
                (
                    "Unsorted/Unknown artist/01 First (2).mp3",
                    "Unsorted/Unknown artist/02 Second (2).flac",
                )
            };
            assert_eq!(
                shown,
                vec![
                    ("Night Album.zip › 01 First.mp3", first),
                    ("Night Album.zip › 02 Second.flac", second),
                ]
            );
            assert!(album.filed.iter().all(|f| f.problem.is_none()));
            assert_eq!(album.folders, vec![music.join("Unsorted/Unknown artist")]);
            assert!(
                !zip_path.exists(),
                "the archive was left to be unpacked again"
            );
        }
        assert_eq!(
            every_file(&downloads),
            vec!["Unpacked/Night Album (2).zip", "Unpacked/Night Album.zip"]
        );
        // Nothing else, not even the empty folder it was unpacked in.
        let left: Vec<_> = std::fs::read_dir(&downloads)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from(UNPACKED)]);
        assert_eq!(
            std::fs::read(music.join("Unsorted/Unknown artist/02 Second.flac")).unwrap(),
            b"second"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A `.zip` that is not an album is none of djmanzo's business**: one
    /// holding no records, and one that is not an archive at all, are left
    /// exactly where they are, and nothing is written anywhere.
    #[test]
    fn a_zip_that_is_not_an_album_is_left_alone() {
        let dir = scratch("not-album");
        let (downloads, music) = (dir.join("downloads"), dir.join("music"));
        std::fs::create_dir_all(&downloads).unwrap();
        let papers = downloads.join("tax papers.zip");
        archive(
            &papers,
            &[("2025.pdf", b"numbers"), ("notes.txt", b"words")],
        );
        let broken = downloads.join("broken.zip");
        std::fs::write(&broken, b"this was never an archive").unwrap();
        for zip_path in [&papers, &broken] {
            assert_eq!(take_album(zip_path, &music, LIMITS, 0), None);
        }
        assert_eq!(every_file(&downloads), vec!["broken.zip", "tax papers.zip"]);
        assert!(!music.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Filing makes the folders, moves the file, and never overwrites.
    #[test]
    fn filing_moves_and_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("djmanzo-filing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (downloads, music) = (dir.join("downloads"), dir.join("music"));
        std::fs::create_dir_all(&downloads).unwrap();
        let relative = PathBuf::from("House/Artist/song.mp3");
        for body in [b"first".as_slice(), b"second".as_slice()] {
            let arrived = downloads.join("song.mp3");
            std::fs::write(&arrived, body).unwrap();
            file(&arrived, &music, &relative).unwrap();
            assert!(!arrived.exists());
        }
        assert_eq!(
            std::fs::read(music.join("House/Artist/song.mp3")).unwrap(),
            b"first"
        );
        assert_eq!(
            std::fs::read(music.join("House/Artist/song (2).mp3")).unwrap(),
            b"second"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

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
//! audio. A store's album download that arrives as a `.zip` is left where it
//! is: unpacking archives is a thing a DJ should see happen, and it is not
//! built yet.

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

/// Whether a path is a finished audio file worth looking at.
#[must_use]
pub fn is_audio(path: &Path) -> bool {
    let hidden = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.starts_with('.'));
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    !hidden && AUDIO.contains(&extension.as_str()) && !PARTIAL.contains(&extension.as_str())
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
}

impl Watcher {
    /// Look once, and answer the files that were the same size last time.
    ///
    /// A file answered is forgotten, so a file that cannot be moved is
    /// answered once per two looks rather than on every one.
    pub fn look(&mut self, folder: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(folder) else {
            self.sizes.clear();
            return Vec::new();
        };
        let mut now = HashMap::new();
        let mut ready = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_audio(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() || meta.len() == 0 {
                continue;
            }
            if self.sizes.get(&path) == Some(&meta.len()) {
                ready.push(path);
            } else {
                now.insert(path, meta.len());
            }
        }
        self.sizes = now;
        ready.sort();
        ready
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
                    let filed = take(&state, &arrived, &into);
                    state.note_filed(filed);
                }
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for the downloads folder; purchases will not be filed");
    }
}

/// File one arrived record and put it in the library.
fn take(state: &crate::state::AppState, arrived: &Path, into: &Path) -> Filed {
    let name = arrived
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let now = crate::library::now_seconds();
    let tags = dj_library::tags::read(arrived).unwrap_or_default();
    let relative = place(&tags, &name);
    match file(arrived, into, &relative) {
        Ok(target) => {
            // Into the collection now, rather than at the next rescan: the
            // point of filing it is that the record bought a minute ago is
            // there when the DJ looks.
            if let Ok(db) = state.library().get() {
                let _ = db.add_folder(into, now);
                if let Some(folder) = target.parent()
                    && let Err(error) = dj_library::scan_folder(&db, folder, now)
                {
                    tracing::warn!(%error, ?folder, "filed, but the library did not take it");
                }
            }
            Filed {
                arrived: name,
                to: target
                    .strip_prefix(into)
                    .unwrap_or(&target)
                    .to_string_lossy()
                    .into_owned(),
                at: now,
                problem: None,
            }
        }
        Err(error) => Filed {
            arrived: name,
            to: String::new(),
            at: now,
            problem: Some(error.to_string()),
        },
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
        std::fs::write(dir.join("cover.jpg"), b"abc").unwrap();

        let mut watcher = Watcher::default();
        assert!(watcher.look(&dir).is_empty(), "answered on first sight");
        std::fs::write(&song, b"abcdef").unwrap();
        assert!(watcher.look(&dir).is_empty(), "answered while it grew");
        assert_eq!(watcher.look(&dir), vec![song.clone()]);
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

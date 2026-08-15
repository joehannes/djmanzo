//! Files on this machine.
//!
//! The only source that needs nobody's permission, works with no network, and
//! cannot stop working because a company changed its terms. Every other
//! provider in this crate is, ultimately, a way of deciding what to put here.
//!
//! Deliberately simple: scan some folders, keep the list in memory, match
//! substrings. A real library — SQLite, tags, content hashing, cues, playlists
//! — is M3's job. This is the part the *sources* layer needs: something to
//! search, and something to match a Spotify or YouTube result against.

use crate::provider::{
    AudioAccess, Capabilities, Playable, ProviderId, ProviderStatus, Query, SourceError,
    SourceProvider, TrackRef,
};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Extensions worth opening. Matches what `dj-decode` can handle.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aiff", "aif", "ogg", "oga", "m4a", "aac", "opus", "wv", "alac",
];

/// How deep a scan will go.
///
/// Music collections nest — genre, artist, album — but not indefinitely, and a
/// symlink loop otherwise turns a scan into a hang.
const MAX_DEPTH: usize = 12;

/// A file we found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTrack {
    pub path: PathBuf,
    /// Guessed from the filename until M3 reads real tags.
    pub title: String,
    pub artist: String,
}

impl LocalTrack {
    /// Split `Artist - Title.mp3` into its parts.
    ///
    /// Filenames are a poor substitute for tags, and this is explicitly the
    /// stopgap until M3 reads them properly. But it is the convention almost
    /// every DJ's folder already follows, so it gets the common case right.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Split on the first " - " only: "Artist - Title - Remix" should keep
        // the remix with the title, not lose it.
        match stem.split_once(" - ") {
            Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => Self {
                path: path.to_path_buf(),
                title: title.trim().to_owned(),
                artist: artist.trim().to_owned(),
            },
            _ => Self {
                path: path.to_path_buf(),
                title: stem,
                artist: String::new(),
            },
        }
    }

    fn to_ref(&self) -> TrackRef {
        TrackRef {
            provider: ProviderId::Local,
            id: self.path.to_string_lossy().to_string(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: None,
            duration_seconds: None,
            bpm: None,
            key: None,
            genre: None,
            artwork_url: None,
            web_url: None,
            playable: true,
        }
    }

    /// Everything searchable about this track, lowercased once.
    fn haystack(&self) -> String {
        format!(
            "{} {} {}",
            self.artist.to_lowercase(),
            self.title.to_lowercase(),
            self.path.to_string_lossy().to_lowercase()
        )
    }
}

/// An in-memory index of the user's folders.
#[derive(Debug, Default)]
pub struct LocalLibrary {
    tracks: RwLock<Vec<LocalTrack>>,
    roots: RwLock<Vec<PathBuf>>,
}

impl LocalLibrary {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a folder and index everything under it.
    ///
    /// Returns how many tracks were found. Re-scanning the same root replaces
    /// its entries rather than duplicating them.
    pub fn add_root(&self, root: impl AsRef<Path>) -> usize {
        let root = root.as_ref().to_path_buf();
        let mut found = Vec::new();
        scan(&root, 0, &mut found);

        if let Ok(mut tracks) = self.tracks.write() {
            // Drop anything previously indexed from this root, so a rescan after
            // deleting files does not leave ghosts pointing at nothing.
            tracks.retain(|t| !t.path.starts_with(&root));
            tracks.extend(found.iter().cloned());
            tracks.sort_by(|a, b| a.path.cmp(&b.path));
            tracks.dedup_by(|a, b| a.path == b.path);
        }
        if let Ok(mut roots) = self.roots.write()
            && !roots.contains(&root)
        {
            roots.push(root);
        }
        found.len()
    }

    pub fn remove_root(&self, root: impl AsRef<Path>) {
        let root = root.as_ref();
        if let Ok(mut tracks) = self.tracks.write() {
            tracks.retain(|t| !t.path.starts_with(root));
        }
        if let Ok(mut roots) = self.roots.write() {
            roots.retain(|r| r != root);
        }
    }

    #[must_use]
    pub fn roots(&self) -> Vec<PathBuf> {
        self.roots.read().map(|r| r.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tracks.read().map(|t| t.len()).unwrap_or(0)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Find tracks matching every word in `text`, in any order and any field.
    ///
    /// Word-wise rather than as one string, because "bachata romeo" should find
    /// "Romeo Santos - Propuesta Indecente" even though those words never appear
    /// adjacent. That is how people actually type into a search box mid-set.
    #[must_use]
    pub fn find(&self, text: &str, limit: usize) -> Vec<LocalTrack> {
        let needles: Vec<String> = text
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();

        let Ok(tracks) = self.tracks.read() else {
            return Vec::new();
        };
        if needles.is_empty() {
            return tracks.iter().take(limit).cloned().collect();
        }

        tracks
            .iter()
            .filter(|track| {
                let haystack = track.haystack();
                needles.iter().all(|needle| haystack.contains(needle))
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Find the local file that best corresponds to a result from elsewhere.
    ///
    /// This is what makes a metadata-only provider useful: plan a set from a
    /// Spotify playlist, play it from your own files. Matching on artist and
    /// title words is crude but predictable, and predictable matters more than
    /// clever when the alternative is loading the wrong track in front of a
    /// room.
    #[must_use]
    pub fn match_track(&self, remote: &TrackRef) -> Option<LocalTrack> {
        let query = format!("{} {}", remote.artist, remote.title);
        self.find(&query, 1).into_iter().next().or_else(|| {
            // Fall back to the title alone: artist names differ across services
            // more than titles do ("Romeo Santos" vs "Romeo Santos, Drake").
            self.find(&remote.title, 1).into_iter().next()
        })
    }
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<LocalTrack>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        // An unreadable folder is not worth failing a whole scan over -- a
        // permissions problem on one directory should not lose the other nine.
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `file_type` rather than `metadata`, so a symlink is not followed --
        // which is what stops a loop turning the scan into a hang.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            scan(&path, depth + 1, out);
        } else if file_type.is_file() && is_audio(&path) {
            out.push(LocalTrack::from_path(&path));
        }
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.as_str()))
}

#[async_trait::async_trait]
impl SourceProvider for LocalLibrary {
    fn id(&self) -> ProviderId {
        ProviderId::Local
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        }
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::Ready
    }

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        Ok(self
            .find(&query.text, query.limit)
            .iter()
            .map(LocalTrack::to_ref)
            .collect())
    }

    async fn resolve(&self, track: &TrackRef) -> Result<Playable, SourceError> {
        let path = PathBuf::from(&track.id);
        if path.is_file() {
            Ok(Playable::File(path))
        } else {
            // The index can outlive the file. Saying so beats handing the
            // decoder a path that will fail less legibly.
            Err(SourceError::Io(format!(
                "{} is no longer there — rescan the folder",
                path.display()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("djmanzo-local-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"not really audio").unwrap();
    }

    #[test]
    fn a_filename_splits_into_artist_and_title() {
        let track = LocalTrack::from_path(Path::new("/m/Romeo Santos - Propuesta Indecente.mp3"));
        assert_eq!(track.artist, "Romeo Santos");
        assert_eq!(track.title, "Propuesta Indecente");
    }

    /// Only the first separator splits, or a remix credit would be thrown away.
    #[test]
    fn only_the_first_separator_splits() {
        let track = LocalTrack::from_path(Path::new("/m/Aventura - Obsesion - Bachata Remix.mp3"));
        assert_eq!(track.artist, "Aventura");
        assert_eq!(track.title, "Obsesion - Bachata Remix");
    }

    #[test]
    fn a_filename_with_no_separator_is_all_title() {
        let track = LocalTrack::from_path(Path::new("/m/untitled-mix.wav"));
        assert_eq!(track.title, "untitled-mix");
        assert!(track.artist.is_empty());
    }

    #[test]
    fn scanning_finds_audio_and_ignores_everything_else() {
        let dir = temp_dir("scan");
        touch(&dir, "A - One.mp3");
        touch(&dir, "B - Two.flac");
        touch(&dir, "cover.jpg");
        touch(&dir, "notes.txt");

        let library = LocalLibrary::new();
        assert_eq!(library.add_root(&dir), 2);
        assert_eq!(library.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scanning_recurses_into_folders() {
        let dir = temp_dir("nested");
        fs::create_dir_all(dir.join("bachata/aventura")).unwrap();
        touch(&dir, "Top - Level.mp3");
        touch(&dir.join("bachata"), "Mid - Level.mp3");
        touch(&dir.join("bachata/aventura"), "Deep - Level.mp3");

        let library = LocalLibrary::new();
        assert_eq!(library.add_root(&dir), 3);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extensions_are_matched_case_insensitively() {
        let dir = temp_dir("case");
        touch(&dir, "A - Shouty.MP3");
        touch(&dir, "B - Mixed.FlAc");

        let library = LocalLibrary::new();
        assert_eq!(library.add_root(&dir), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    /// The search behaviour that matters mid-set: words in any order, across
    /// artist and title.
    #[test]
    fn search_matches_words_in_any_order() {
        let dir = temp_dir("search");
        touch(&dir, "Romeo Santos - Propuesta Indecente.mp3");
        touch(&dir, "Juan Luis Guerra - Bachata Rosa.mp3");

        let library = LocalLibrary::new();
        library.add_root(&dir);

        assert_eq!(library.find("propuesta romeo", 10).len(), 1);
        assert_eq!(library.find("romeo propuesta", 10).len(), 1);
        assert_eq!(library.find("bachata", 10).len(), 1);
        assert_eq!(library.find("nothing here", 10).len(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_is_case_insensitive() {
        let dir = temp_dir("case2");
        touch(&dir, "Romeo Santos - Propuesta Indecente.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);
        assert_eq!(library.find("ROMEO", 10).len(), 1);
        assert_eq!(library.find("pRoPuEsTa", 10).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_search_lists_the_library() {
        let dir = temp_dir("empty-query");
        touch(&dir, "A - One.mp3");
        touch(&dir, "B - Two.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);
        assert_eq!(library.find("", 10).len(), 2);
        assert_eq!(library.find("   ", 10).len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_limit_is_respected() {
        let dir = temp_dir("limit");
        for n in 0..20 {
            touch(&dir, &format!("Artist - Track {n}.mp3"));
        }
        let library = LocalLibrary::new();
        library.add_root(&dir);
        assert_eq!(library.find("artist", 5).len(), 5);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Rescanning must not double the library, and must forget deleted files.
    #[test]
    fn rescanning_replaces_rather_than_duplicates() {
        let dir = temp_dir("rescan");
        touch(&dir, "A - One.mp3");
        touch(&dir, "B - Two.mp3");

        let library = LocalLibrary::new();
        library.add_root(&dir);
        assert_eq!(library.len(), 2);

        library.add_root(&dir);
        assert_eq!(library.len(), 2, "a rescan duplicated the library");

        fs::remove_file(dir.join("B - Two.mp3")).unwrap();
        library.add_root(&dir);
        assert_eq!(library.len(), 1, "a deleted file survived a rescan");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_a_root_removes_only_its_tracks() {
        let one = temp_dir("root-one");
        let two = temp_dir("root-two");
        touch(&one, "A - One.mp3");
        touch(&two, "B - Two.mp3");

        let library = LocalLibrary::new();
        library.add_root(&one);
        library.add_root(&two);
        assert_eq!(library.len(), 2);

        library.remove_root(&one);
        assert_eq!(library.len(), 1);
        assert_eq!(library.roots().len(), 1);
        assert_eq!(library.find("two", 10).len(), 1);
        let _ = fs::remove_dir_all(&one);
        let _ = fs::remove_dir_all(&two);
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        let library = LocalLibrary::new();
        assert_eq!(library.add_root("/nowhere/at/all"), 0);
    }

    /// The point of having a local library at all, from the sources layer's
    /// perspective: turning a Spotify result into something playable.
    #[tokio::test]
    async fn a_remote_result_matches_a_local_file() {
        let dir = temp_dir("match");
        touch(&dir, "Romeo Santos - Propuesta Indecente.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);

        let remote = TrackRef {
            provider: ProviderId::Spotify,
            id: "spotify:track:abc".into(),
            title: "Propuesta Indecente".into(),
            artist: "Romeo Santos".into(),
            album: None,
            duration_seconds: None,
            bpm: None,
            key: None,
            genre: None,
            artwork_url: None,
            web_url: None,
            playable: false,
        };
        let matched = library.match_track(&remote).expect("should have matched");
        assert!(matched.path.ends_with("Romeo Santos - Propuesta Indecente.mp3"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Services credit featured artists differently. Falling back to the title
    /// alone is what makes matching survive that.
    #[tokio::test]
    async fn matching_falls_back_to_the_title_when_the_artist_differs() {
        let dir = temp_dir("match-artist");
        touch(&dir, "Aventura - Obsesion.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);

        let remote = TrackRef {
            provider: ProviderId::Spotify,
            id: "x".into(),
            title: "Obsesion".into(),
            artist: "Aventura, Judy Santos".into(),
            album: None,
            duration_seconds: None,
            bpm: None,
            key: None,
            genre: None,
            artwork_url: None,
            web_url: None,
            playable: false,
        };
        assert!(library.match_track(&remote).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn resolving_yields_the_file() {
        let dir = temp_dir("resolve");
        touch(&dir, "A - One.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);

        let results = library.search(&Query::new("one")).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].playable);

        let playable = library.resolve(&results[0]).await.unwrap();
        assert!(matches!(playable, Playable::File(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    /// An index entry can outlive its file. Saying so beats handing the decoder
    /// a path that fails less legibly.
    #[tokio::test]
    async fn resolving_a_deleted_file_says_what_happened() {
        let dir = temp_dir("resolve-gone");
        touch(&dir, "A - One.mp3");
        let library = LocalLibrary::new();
        library.add_root(&dir);
        let results = library.search(&Query::new("one")).await.unwrap();

        fs::remove_file(dir.join("A - One.mp3")).unwrap();
        let error = library.resolve(&results[0]).await.unwrap_err();
        assert!(error.to_string().contains("rescan"));
        let _ = fs::remove_dir_all(&dir);
    }
}

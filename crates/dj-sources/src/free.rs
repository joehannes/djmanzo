//! The online sources you may actually mix.
//!
//! Two of them, and they matter out of proportion to their catalogue size: they
//! are the only entries in this crate, other than the user's own files, that
//! hand over audio a deck can play without anyone needing to negotiate
//! anything. That makes them the way to prove the whole path works — search,
//! resolve, load, play — before spending money on a service that may turn out
//! to be partner-gated.
//!
//! - **Jamendo**: Creative Commons releases from independent artists. Free API
//!   key, direct MP3 URLs, explicitly licensed for reuse.
//! - **Internet Archive**: public domain and freely-licensed recordings, no key
//!   at all. Deep in historical material — including a great deal of early
//!   Caribbean and Latin recording — with wildly variable audio quality.

use crate::http::HttpClient;
use crate::provider::{
    AudioAccess, Capabilities, Playable, ProviderId, ProviderStatus, Query, SourceError,
    SourceProvider, TrackRef,
};
use dj_secrets::{SecretKind, SecretStore};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Jamendo
// ---------------------------------------------------------------------------

const JAMENDO_NAME: &str = "Jamendo";
const JAMENDO_URL: &str = "https://api.jamendo.com/v3.0/tracks/";

#[derive(Debug)]
pub struct JamendoProvider {
    http: Arc<dyn HttpClient>,
    secrets: Arc<dyn SecretStore>,
}

impl JamendoProvider {
    #[must_use]
    pub fn new(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { http, secrets }
    }
}

fn parse_jamendo(body: &serde_json::Value) -> Vec<TrackRef> {
    let Some(results) = body.get("results").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            let audio = item.get("audio").and_then(|v| v.as_str());
            Some(TrackRef {
                provider: ProviderId::Jamendo,
                // The audio URL is carried as the id, so `resolve` needs no
                // second round trip -- search already told us everything.
                id: audio.unwrap_or(id).to_owned(),
                title: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                artist: item
                    .get("artist_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                album: item
                    .get("album_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                // Jamendo reports seconds, unlike Spotify.
                duration_seconds: item
                    .get("duration")
                    .and_then(number)
                    .map(|seconds| seconds as f32),
                // One of the few APIs that reports a usable tempo.
                bpm: item.pointer("/musicinfo/bpm").and_then(number).map(|b| b as f32),
                key: None,
                genre: item
                    .pointer("/musicinfo/tags/genres/0")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                artwork_url: item.get("image").and_then(|v| v.as_str()).map(str::to_owned),
                web_url: item
                    .get("shareurl")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                playable: audio.is_some(),
            })
        })
        .collect()
}

/// Read a number that may have arrived as a JSON number or as a string.
///
/// Both of these APIs do both, in the same response, for different fields.
fn number(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

#[async_trait::async_trait]
impl SourceProvider for JamendoProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Jamendo
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        }
    }

    fn status(&self) -> ProviderStatus {
        if self.secrets.has(SecretKind::JamendoClientId) {
            ProviderStatus::Ready
        } else {
            ProviderStatus::NeedsCredentials {
                missing: vec![SecretKind::JamendoClientId],
            }
        }
    }

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        let client_id = self
            .secrets
            .get(SecretKind::JamendoClientId)
            .map_err(|_| SourceError::MissingCredentials(JAMENDO_NAME))?;

        let url = format!(
            "{JAMENDO_URL}?client_id={}&format=json&limit={}&search={}\
             &audioformat=mp32&include=musicinfo",
            urlencoding::encode(client_id.expose()),
            query.limit.min(200),
            urlencoding::encode(&query.text),
        );
        let body = self
            .http
            .get_json(&url, &[])
            .await
            .map_err(|e| e.into_source_error(JAMENDO_NAME))?;
        Ok(parse_jamendo(&body))
    }

    async fn resolve(&self, track: &TrackRef) -> Result<Playable, SourceError> {
        if !track.playable {
            return Err(SourceError::BadResponse {
                provider: JAMENDO_NAME,
                message: "this result carried no audio URL".into(),
            });
        }
        Ok(Playable::Stream {
            url: track.id.clone(),
            // Jamendo's URLs are stable, not signed with an expiry.
            expires_in_seconds: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Internet Archive
// ---------------------------------------------------------------------------

const ARCHIVE_NAME: &str = "Internet Archive";
const ARCHIVE_SEARCH: &str = "https://archive.org/advancedsearch.php";
const ARCHIVE_METADATA: &str = "https://archive.org/metadata";
const ARCHIVE_DOWNLOAD: &str = "https://archive.org/download";

/// Formats worth playing, best first.
const ARCHIVE_FORMATS: &[&str] = &["VBR MP3", "MP3", "128Kbps MP3", "Flac", "Ogg Vorbis"];

#[derive(Debug)]
pub struct ArchiveProvider {
    http: Arc<dyn HttpClient>,
}

impl ArchiveProvider {
    #[must_use]
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http }
    }
}

fn parse_archive(body: &serde_json::Value) -> Vec<TrackRef> {
    let Some(docs) = body.pointer("/response/docs").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    docs.iter()
        .filter_map(|doc| {
            let identifier = doc.get("identifier")?.as_str()?;
            Some(TrackRef {
                provider: ProviderId::InternetArchive,
                id: identifier.to_owned(),
                title: doc
                    .get("title")
                    .and_then(text_or_first)
                    .unwrap_or_else(|| identifier.to_owned()),
                artist: doc
                    .get("creator")
                    .and_then(text_or_first)
                    .unwrap_or_default(),
                album: None,
                duration_seconds: None,
                bpm: None,
                key: None,
                genre: None,
                artwork_url: Some(format!("https://archive.org/services/img/{identifier}")),
                web_url: Some(format!("https://archive.org/details/{identifier}")),
                // Playable, but only after a metadata call finds a file --
                // an Archive item is a folder, not a track.
                playable: true,
            })
        })
        .collect()
}

/// Archive fields are sometimes a string and sometimes an array of strings,
/// depending on how many values the item happens to carry.
fn text_or_first(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_array()?.first()?.as_str().map(str::to_owned))
}

/// Choose the best audio file in an item's metadata.
fn best_file(metadata: &serde_json::Value) -> Option<String> {
    let files = metadata.get("files")?.as_array()?;
    for wanted in ARCHIVE_FORMATS {
        for file in files {
            let format = file.get("format").and_then(|v| v.as_str()).unwrap_or("");
            if format.eq_ignore_ascii_case(wanted)
                && let Some(name) = file.get("name").and_then(|v| v.as_str())
            {
                return Some(name.to_owned());
            }
        }
    }
    None
}

#[async_trait::async_trait]
impl SourceProvider for ArchiveProvider {
    fn id(&self) -> ProviderId {
        ProviderId::InternetArchive
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: false,
            audio: AudioAccess::Direct,
        }
    }

    /// Needs nothing. Which is exactly why it is the application's proof that
    /// the source path works before the user has signed up for anything.
    fn status(&self) -> ProviderStatus {
        ProviderStatus::Ready
    }

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        // Constrain to audio, or the results are mostly scanned books.
        let lucene = format!("({}) AND mediatype:(audio)", query.text);
        let url = format!(
            "{ARCHIVE_SEARCH}?q={}&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=creator\
             &rows={}&page=1&output=json",
            urlencoding::encode(&lucene),
            query.limit.min(100),
        );
        let body = self
            .http
            .get_json(&url, &[])
            .await
            .map_err(|e| e.into_source_error(ARCHIVE_NAME))?;
        Ok(parse_archive(&body))
    }

    async fn resolve(&self, track: &TrackRef) -> Result<Playable, SourceError> {
        // An Archive item is a folder of files, so which file to play is a
        // second question the search response cannot answer.
        let url = format!("{ARCHIVE_METADATA}/{}", urlencoding::encode(&track.id));
        let metadata = self
            .http
            .get_json(&url, &[])
            .await
            .map_err(|e| e.into_source_error(ARCHIVE_NAME))?;

        let name = best_file(&metadata).ok_or(SourceError::NotPlayable {
            provider: ARCHIVE_NAME,
            reason: "this item holds no audio in a format we can decode",
        })?;

        Ok(Playable::Stream {
            url: format!(
                "{ARCHIVE_DOWNLOAD}/{}/{}",
                urlencoding::encode(&track.id),
                urlencoding::encode(&name)
            ),
            expires_in_seconds: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubClient;
    use dj_secrets::{MemoryStore, Secret};
    use serde_json::json;

    fn jamendo_keyed() -> Arc<dyn SecretStore> {
        let store = MemoryStore::new();
        store
            .set(SecretKind::JamendoClientId, &Secret::new("cid"))
            .unwrap();
        Arc::new(store)
    }

    fn jamendo_body() -> serde_json::Value {
        json!({"results": [{
            "id": "1886179",
            "name": "Sunny Afternoon",
            "artist_name": "The Bluebirds",
            "album_name": "Skies",
            "duration": 214,
            "audio": "https://prod-1.storage.jamendo.com/?trackid=1886179&format=mp32",
            "image": "https://usercontent.jamendo.com/art.jpg",
            "shareurl": "https://www.jamendo.com/track/1886179",
            "musicinfo": {"bpm": "128", "tags": {"genres": ["latin"]}}
        }]})
    }

    #[tokio::test]
    async fn jamendo_search_parses_and_is_playable() {
        let http = Arc::new(StubClient::new(vec![jamendo_body()]));
        let jamendo = JamendoProvider::new(http, jamendo_keyed());
        let results = jamendo.search(&Query::new("sunny")).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Sunny Afternoon");
        assert_eq!(results[0].duration_seconds, Some(214.0));
        assert!(results[0].playable);
    }

    /// Jamendo sends the BPM as a string and the duration as a number, in the
    /// same object. Handling only one of those shapes loses the other field.
    #[test]
    fn numbers_arriving_as_strings_are_still_read() {
        let results = parse_jamendo(&jamendo_body());
        assert_eq!(results[0].bpm, Some(128.0));
        assert_eq!(results[0].genre.as_deref(), Some("latin"));
    }

    /// The whole point of Jamendo being here: it resolves without a second
    /// request, because search already carried the audio URL.
    #[tokio::test]
    async fn jamendo_resolves_without_another_round_trip() {
        let http = Arc::new(StubClient::new(vec![jamendo_body()]));
        let jamendo = JamendoProvider::new(http.clone(), jamendo_keyed());
        let results = jamendo.search(&Query::new("sunny")).await.unwrap();

        let playable = jamendo.resolve(&results[0]).await.unwrap();
        match playable {
            Playable::Stream { url, .. } => assert!(url.contains("trackid=1886179")),
            other => panic!("expected a stream, got {other:?}"),
        }
        assert_eq!(http.requested.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_jamendo_result_with_no_audio_is_not_offered_as_playable() {
        let body = json!({"results": [{"id": "9", "name": "Silent", "artist_name": "X"}]});
        let results = parse_jamendo(&body);
        assert!(!results[0].playable);

        let jamendo = JamendoProvider::new(Arc::new(StubClient::new(vec![])), jamendo_keyed());
        assert!(jamendo.resolve(&results[0]).await.is_err());
    }

    // -- Internet Archive ---------------------------------------------------

    fn archive_body() -> serde_json::Value {
        json!({"response": {"docs": [
            {"identifier": "gd1977-05-08", "title": "Cornell 1977", "creator": "Grateful Dead"},
            {"identifier": "merengue-78", "title": ["Merengue Classics"], "creator": ["Various"]}
        ]}})
    }

    #[tokio::test]
    async fn archive_search_needs_no_credentials() {
        let archive = ArchiveProvider::new(Arc::new(StubClient::new(vec![archive_body()])));
        assert!(archive.status().is_usable());

        let results = archive.search(&Query::new("merengue")).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].artist, "Grateful Dead");
    }

    /// Archive fields arrive as a bare string or as an array, in the same
    /// response, depending on how many values the item carries.
    #[test]
    fn archive_fields_are_read_whether_scalar_or_array() {
        let results = parse_archive(&archive_body());
        assert_eq!(results[1].title, "Merengue Classics");
        assert_eq!(results[1].artist, "Various");
    }

    #[tokio::test]
    async fn archive_search_is_constrained_to_audio() {
        let http = Arc::new(StubClient::new(vec![archive_body()]));
        let archive = ArchiveProvider::new(http.clone());
        archive.search(&Query::new("merengue")).await.unwrap();
        assert!(
            http.last_url().contains("mediatype"),
            "unconstrained search returns scanned books: {}",
            http.last_url()
        );
    }

    /// An Archive item is a folder, so resolving picks a file — and prefers a
    /// format the decoder can actually open.
    #[tokio::test]
    async fn archive_resolve_picks_the_best_available_format() {
        let http = Arc::new(StubClient::new(vec![
            archive_body(),
            json!({"files": [
                {"name": "notes.txt", "format": "Text"},
                {"name": "track01.flac", "format": "Flac"},
                {"name": "track01.mp3", "format": "VBR MP3"}
            ]}),
        ]));
        let archive = ArchiveProvider::new(http);
        let results = archive.search(&Query::new("x")).await.unwrap();

        match archive.resolve(&results[0]).await.unwrap() {
            Playable::Stream { url, .. } => {
                assert!(url.contains("track01.mp3"), "picked the wrong file: {url}");
                assert!(url.starts_with("https://archive.org/download/gd1977-05-08/"));
            }
            other => panic!("expected a stream, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_archive_item_with_no_audio_says_so() {
        let http = Arc::new(StubClient::new(vec![
            archive_body(),
            json!({"files": [{"name": "scan.pdf", "format": "Text PDF"}]}),
        ]));
        let archive = ArchiveProvider::new(http);
        let results = archive.search(&Query::new("x")).await.unwrap();
        let error = archive.resolve(&results[0]).await.unwrap_err();
        assert!(error.to_string().contains("no audio"));
    }

    #[test]
    fn unexpected_responses_yield_nothing() {
        for value in [json!({}), json!({"response": {}}), json!({"results": "no"})] {
            assert!(parse_jamendo(&value).is_empty());
            assert!(parse_archive(&value).is_empty());
        }
    }
}

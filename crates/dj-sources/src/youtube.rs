//! YouTube and YouTube Music.
//!
//! One API key, two providers, and a difference worth being precise about.
//!
//! **YouTube** search is genuinely useful — edits, versions and live cuts that
//! exist nowhere else. Its audio is licensed only through YouTube's own player,
//! which cannot be routed into a mixer, so a result becomes playable only by
//! being matched to a file the user already holds. djmanzo ships no downloader
//! and makes no acquisition decision on anyone's behalf.
//!
//! **YouTube Music** is the one people most often hope for, so it deserves a
//! flat answer: there is no public API, and no route by which a third-party
//! application may stream its audio into a mixer, at any subscription tier. The
//! unofficial clients in circulation impersonate the web player, which breaks
//! the terms and gets accounts closed. What does work, on the same Data API
//! key, is finding music and importing playlists — so a set can be planned from
//! a YouTube Music library and played from files the user owns. If a sanctioned
//! API appears, this is where it lands.

use crate::http::HttpClient;
use crate::provider::{
    AudioAccess, Capabilities, ProviderId, ProviderStatus, Query, SourceError, SourceProvider,
    TrackRef,
};
use dj_secrets::{SecretKind, SecretStore};
use std::sync::Arc;

const SEARCH_URL: &str = "https://www.googleapis.com/youtube/v3/search";

/// YouTube's category id for Music. Restricting to it is what makes the
/// YouTube Music provider return music rather than everything.
const MUSIC_CATEGORY: &str = "10";

/// Shared by both providers; they differ only in filtering and capabilities.
#[derive(Debug)]
pub struct YouTubeProvider {
    http: Arc<dyn HttpClient>,
    secrets: Arc<dyn SecretStore>,
    music_only: bool,
}

impl YouTubeProvider {
    #[must_use]
    pub fn youtube(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            music_only: false,
        }
    }

    #[must_use]
    pub fn music(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            music_only: true,
        }
    }

    fn name(&self) -> &'static str {
        if self.music_only {
            "YouTube Music"
        } else {
            "YouTube"
        }
    }
}

/// Pull the results out of a Data API `search.list` response.
fn parse_search(body: &serde_json::Value, provider: ProviderId) -> Vec<TrackRef> {
    let Some(items) = body.get("items").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            // A search can return channels and playlists alongside videos even
            // with `type=video`; anything without a videoId is not a track.
            let id = item.pointer("/id/videoId").and_then(|v| v.as_str())?;
            let title = item.pointer("/snippet/title").and_then(|v| v.as_str())?;

            // YouTube has no artist field. The channel is the closest thing,
            // and for music it is usually the artist or their label.
            let artist = item
                .pointer("/snippet/channelTitle")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            Some(TrackRef {
                provider,
                id: id.to_owned(),
                title: decode_entities(title),
                artist: decode_entities(artist),
                album: None,
                // `search.list` does not return duration; it needs a second
                // `videos.list` call, which doubles the quota cost for
                // information the browser does not need to show a result.
                duration_seconds: None,
                bpm: None,
                key: None,
                genre: None,
                artwork_url: item
                    .pointer("/snippet/thumbnails/high/url")
                    .or_else(|| item.pointer("/snippet/thumbnails/default/url"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                web_url: Some(format!("https://www.youtube.com/watch?v={id}")),
                // Never direct from the API. A YouTube result becomes playable
                // only by being matched to a local file.
                playable: false,
            })
        })
        .collect()
}

/// YouTube returns titles HTML-escaped: `Aventura &amp; Romeo`.
///
/// Only the five entities the API actually emits. A full HTML parser would be
/// a dependency and a much larger attack surface for no gain.
fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[async_trait::async_trait]
impl SourceProvider for YouTubeProvider {
    fn id(&self) -> ProviderId {
        if self.music_only {
            ProviderId::YouTubeMusic
        } else {
            ProviderId::YouTube
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: true,
            audio: if self.music_only {
                AudioAccess::None {
                    reason: "no public API offers YouTube Music audio to a \
                             third-party mixer, at any subscription tier",
                }
            } else {
                AudioAccess::UserSupplied {
                    note: "YouTube licenses playback only through its own \
                           player. Match a result to a file you already hold.",
                }
            },
        }
    }

    fn status(&self) -> ProviderStatus {
        if self.secrets.has(SecretKind::YouTubeApi) {
            ProviderStatus::Ready
        } else {
            ProviderStatus::NeedsCredentials {
                missing: vec![SecretKind::YouTubeApi],
            }
        }
    }

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        let key = self
            .secrets
            .get(SecretKind::YouTubeApi)
            .map_err(|_| SourceError::MissingCredentials(self.name()))?;

        let mut url = format!(
            "{SEARCH_URL}?part=snippet&type=video&q={}&maxResults={}&key={}",
            urlencoding::encode(&query.text),
            // The API's own ceiling. Asking for more is a 400.
            query.limit.min(50),
            urlencoding::encode(key.expose()),
        );
        if self.music_only {
            url.push_str(&format!("&videoCategoryId={MUSIC_CATEGORY}"));
        }

        let body = self
            .http
            .get_json(&url, &[])
            .await
            .map_err(|e| e.into_source_error(self.name()))?;
        Ok(parse_search(&body, self.id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubClient;
    use dj_secrets::{MemoryStore, Secret};
    use serde_json::json;

    fn keyed() -> Arc<dyn SecretStore> {
        let store = MemoryStore::new();
        store
            .set(SecretKind::YouTubeApi, &Secret::new("api-key"))
            .unwrap();
        Arc::new(store)
    }

    fn body() -> serde_json::Value {
        json!({"items": [
            {
                "id": {"videoId": "vid123"},
                "snippet": {
                    "title": "Aventura &amp; Romeo Santos - Obsesi&#39;on",
                    "channelTitle": "AventuraVEVO",
                    "thumbnails": {"high": {"url": "https://i.ytimg.com/hq.jpg"}}
                }
            },
            {
                "id": {"channelId": "chan456"},
                "snippet": {"title": "A channel, not a track", "channelTitle": "X"}
            }
        ]})
    }

    #[tokio::test]
    async fn a_search_response_parses_and_skips_non_videos() {
        let http = Arc::new(StubClient::new(vec![body()]));
        let youtube = YouTubeProvider::youtube(http, keyed());
        let results = youtube.search(&Query::new("obsesion")).await.unwrap();

        assert_eq!(results.len(), 1, "a channel result was treated as a track");
        assert_eq!(results[0].id, "vid123");
        assert_eq!(results[0].artist, "AventuraVEVO");
        assert_eq!(
            results[0].web_url.as_deref(),
            Some("https://www.youtube.com/watch?v=vid123")
        );
    }

    /// Titles arrive HTML-escaped, and `Aventura &amp; Romeo` in a browser list
    /// looks like a bug to everyone who sees it.
    #[test]
    fn html_entities_are_decoded() {
        let results = parse_search(&body(), ProviderId::YouTube);
        assert_eq!(results[0].title, "Aventura & Romeo Santos - Obsesi'on");
    }

    #[tokio::test]
    async fn youtube_music_restricts_to_the_music_category() {
        let http = Arc::new(StubClient::new(vec![json!({"items": []})]));
        let music = YouTubeProvider::music(http.clone(), keyed());
        music.search(&Query::new("bachata")).await.unwrap();
        assert!(
            http.last_url().contains("videoCategoryId=10"),
            "YouTube Music searched all of YouTube: {}",
            http.last_url()
        );
    }

    #[tokio::test]
    async fn plain_youtube_does_not_restrict_the_category() {
        let http = Arc::new(StubClient::new(vec![json!({"items": []})]));
        let youtube = YouTubeProvider::youtube(http.clone(), keyed());
        youtube.search(&Query::new("bachata")).await.unwrap();
        assert!(!http.last_url().contains("videoCategoryId"));
    }

    /// The flat answer, asserted: YouTube Music can never hand over audio.
    #[tokio::test]
    async fn youtube_music_cannot_supply_audio() {
        let http = Arc::new(StubClient::new(vec![body()]));
        let music = YouTubeProvider::music(http, keyed());
        assert!(!music.capabilities().audio.is_playable());

        let results = parse_search(&body(), ProviderId::YouTubeMusic);
        let error = music.resolve(&results[0]).await.unwrap_err();
        assert!(error.to_string().contains("no public API"));
    }

    /// YouTube proper is not forbidden outright — the audio just has to come
    /// from the user. That distinction has to survive in the type.
    #[test]
    fn youtube_audio_is_user_supplied_rather_than_forbidden() {
        let http = Arc::new(StubClient::new(vec![]));
        let youtube = YouTubeProvider::youtube(http, keyed());
        assert!(matches!(
            youtube.capabilities().audio,
            AudioAccess::UserSupplied { .. }
        ));
        assert!(youtube.capabilities().audio.is_playable());
    }

    #[tokio::test]
    async fn no_key_means_no_request() {
        let http = Arc::new(StubClient::new(vec![]));
        let youtube = YouTubeProvider::youtube(http.clone(), Arc::new(MemoryStore::new()));
        assert!(matches!(
            youtube.status(),
            ProviderStatus::NeedsCredentials { .. }
        ));
        assert!(youtube.search(&Query::new("x")).await.is_err());
        assert!(http.requested.lock().unwrap().is_empty());
    }

    /// The API rejects maxResults above 50 with a 400, so the clamp has to
    /// happen before the request rather than being discovered by the user.
    #[tokio::test]
    async fn the_result_count_is_clamped_to_what_the_api_accepts() {
        let http = Arc::new(StubClient::new(vec![json!({"items": []})]));
        let youtube = YouTubeProvider::youtube(http.clone(), keyed());
        youtube
            .search(&Query::new("x").with_limit(200))
            .await
            .unwrap();
        assert!(http.last_url().contains("maxResults=50"));
    }

    #[test]
    fn an_unexpected_response_yields_nothing() {
        for value in [json!({}), json!({"items": {}}), json!({"items": [{}]})] {
            assert!(parse_search(&value, ProviderId::YouTube).is_empty());
        }
    }
}

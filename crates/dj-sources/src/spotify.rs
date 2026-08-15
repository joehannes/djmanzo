//! Spotify — for deciding what to play, never for playing it.
//!
//! This provider deliberately does not implement [`SourceProvider::resolve`].
//! The default refuses, and that refusal is the feature: Spotify's developer
//! policy forbids using their content to "segue, mix, re-mix, or overlap" it
//! with other audio, which is a description of the entire application. No
//! amount of API access changes that, so there is no code path here that could
//! be extended into one by mistake.
//!
//! What it is genuinely good for: search, and matching the result against the
//! user's own files. Plan from a playlist, play from your library.

use crate::http::HttpClient;
use crate::provider::{
    AudioAccess, Capabilities, ProviderId, ProviderStatus, Query, SourceError, SourceProvider,
    TrackRef,
};
use base64::Engine;
use dj_secrets::{Secret, SecretKind, SecretStore};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const NAME: &str = "Spotify";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const SEARCH_URL: &str = "https://api.spotify.com/v1/search";

/// Refresh this far before a token actually expires.
///
/// A token that expires mid-request produces a 401 that looks like a
/// credentials problem, which sends the user to check their client secret for
/// no reason.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct CachedToken {
    value: Secret,
    expires_at: Instant,
}

#[derive(Debug)]
pub struct SpotifyProvider {
    http: Arc<dyn HttpClient>,
    secrets: Arc<dyn SecretStore>,
    token: Mutex<Option<CachedToken>>,
}

impl SpotifyProvider {
    #[must_use]
    pub fn new(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            http,
            secrets,
            token: Mutex::new(None),
        }
    }

    /// Fetch a client-credentials token, or reuse one still in date.
    async fn token(&self) -> Result<Secret, SourceError> {
        if let Ok(cached) = self.token.lock()
            && let Some(token) = cached.as_ref()
            && Instant::now() < token.expires_at
        {
            return Ok(token.value.clone());
        }

        let id = self
            .secrets
            .get(SecretKind::SpotifyClientId)
            .map_err(|_| SourceError::MissingCredentials(NAME))?;
        let secret = self
            .secrets
            .get(SecretKind::SpotifyClientSecret)
            .map_err(|_| SourceError::MissingCredentials(NAME))?;

        // Client credentials go in a Basic header, not the form body -- the
        // secret then never appears in a URL or a logged form.
        let basic = base64::engine::general_purpose::STANDARD.encode(format!(
            "{}:{}",
            id.expose(),
            secret.expose()
        ));

        let body = self
            .http
            .post_form(
                TOKEN_URL,
                &[("Authorization".into(), format!("Basic {basic}"))],
                &[("grant_type", "client_credentials")],
            )
            .await
            .map_err(|e| e.into_source_error(NAME))?;

        let value = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SourceError::BadResponse {
                provider: NAME,
                message: "no access_token in the token response".into(),
            })?;
        let lifetime = body
            .get("expires_in")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(3600);

        let token = Secret::new(value);
        if let Ok(mut cached) = self.token.lock() {
            *cached = Some(CachedToken {
                value: token.clone(),
                expires_at: Instant::now() + Duration::from_secs(lifetime)
                    - EXPIRY_MARGIN.min(Duration::from_secs(lifetime)),
            });
        }
        Ok(token)
    }
}

/// Pull the track list out of a Spotify search response.
///
/// Separate from the request so it can be tested against a captured body.
fn parse_search(body: &serde_json::Value) -> Vec<TrackRef> {
    let Some(items) = body.pointer("/tracks/items").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            let title = item.get("name")?.as_str()?;
            // Several artists is the norm on this repertoire; joining them
            // reads better than picking the first and dropping the features.
            let artist = item
                .get("artists")
                .and_then(|v| v.as_array())
                .map(|artists| {
                    artists
                        .iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            Some(TrackRef {
                provider: ProviderId::Spotify,
                id: id.to_owned(),
                title: title.to_owned(),
                artist,
                album: item
                    .pointer("/album/name")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                // Spotify reports milliseconds. Getting this wrong by 1000x is
                // the classic version of this bug.
                duration_seconds: item
                    .get("duration_ms")
                    .and_then(serde_json::Value::as_f64)
                    .map(|ms| (ms / 1000.0) as f32),
                bpm: None,
                key: None,
                genre: None,
                artwork_url: item
                    .pointer("/album/images/0/url")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                web_url: item
                    .pointer("/external_urls/spotify")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                // Never. See the module documentation.
                playable: false,
            })
        })
        .collect()
}

#[async_trait::async_trait]
impl SourceProvider for SpotifyProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Spotify
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::None {
                reason: "Spotify's developer policy forbids mixing their audio \
                         with anything else",
            },
        }
    }

    fn status(&self) -> ProviderStatus {
        let missing: Vec<SecretKind> =
            [SecretKind::SpotifyClientId, SecretKind::SpotifyClientSecret]
                .into_iter()
                .filter(|kind| !self.secrets.has(*kind))
                .collect();
        if missing.is_empty() {
            ProviderStatus::Ready
        } else {
            ProviderStatus::NeedsCredentials { missing }
        }
    }

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        let token = self.token().await?;
        let url = format!(
            "{SEARCH_URL}?q={}&type=track&limit={}",
            urlencoding::encode(&query.text),
            query.limit.min(50)
        );
        let body = self
            .http
            .get_json(
                &url,
                &[("Authorization".into(), format!("Bearer {}", token.expose()))],
            )
            .await
            .map_err(|e| e.into_source_error(NAME))?;
        Ok(parse_search(&body))
    }

    // `resolve` is deliberately not implemented. The default refuses, quoting
    // the reason from `capabilities`.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubClient;
    use crate::provider::Playable;
    use dj_secrets::MemoryStore;
    use serde_json::json;

    fn stocked_secrets() -> Arc<dyn SecretStore> {
        let store = MemoryStore::new();
        store
            .set(SecretKind::SpotifyClientId, &Secret::new("client-id"))
            .unwrap();
        store
            .set(
                SecretKind::SpotifyClientSecret,
                &Secret::new("client-secret"),
            )
            .unwrap();
        Arc::new(store)
    }

    fn search_body() -> serde_json::Value {
        json!({
            "tracks": {
                "items": [
                    {
                        "id": "abc123",
                        "name": "Propuesta Indecente",
                        "duration_ms": 235_000,
                        "artists": [{"name": "Romeo Santos"}],
                        "album": {
                            "name": "Formula, Vol. 2",
                            "images": [{"url": "https://i.example/art.jpg"}]
                        },
                        "external_urls": {"spotify": "https://open.spotify.com/track/abc123"}
                    }
                ]
            }
        })
    }

    #[tokio::test]
    async fn a_search_response_parses() {
        let http = Arc::new(StubClient::new(vec![
            json!({"access_token": "tok", "expires_in": 3600}),
            search_body(),
        ]));
        let spotify = SpotifyProvider::new(http.clone(), stocked_secrets());

        let results = spotify.search(&Query::new("propuesta")).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Propuesta Indecente");
        assert_eq!(results[0].artist, "Romeo Santos");
        assert_eq!(results[0].album.as_deref(), Some("Formula, Vol. 2"));
    }

    /// Spotify reports milliseconds. Treating them as seconds would show a
    /// four-minute track as sixty-five hours, and the bug is easy to introduce.
    #[test]
    fn durations_are_converted_from_milliseconds() {
        let results = parse_search(&search_body());
        assert_eq!(results[0].duration_seconds, Some(235.0));
    }

    #[test]
    fn several_artists_are_joined_rather_than_truncated() {
        let body = json!({"tracks": {"items": [{
            "id": "x", "name": "Obsesion",
            "artists": [{"name": "Aventura"}, {"name": "Judy Santos"}]
        }]}});
        assert_eq!(parse_search(&body)[0].artist, "Aventura, Judy Santos");
    }

    /// **The rule.** Whatever the response says, a Spotify result is never
    /// playable and resolving one is refused.
    #[tokio::test]
    async fn a_spotify_result_is_never_playable() {
        let results = parse_search(&search_body());
        assert!(!results[0].playable);

        let http = Arc::new(StubClient::new(vec![]));
        let spotify = SpotifyProvider::new(http, stocked_secrets());
        let error = spotify.resolve(&results[0]).await.unwrap_err();
        assert!(matches!(error, SourceError::NotPlayable { .. }));
        assert!(error.to_string().contains("forbids mixing"));
        // And nothing that could be handed to a deck came back.
        let _: Option<Playable> = None;
    }

    #[tokio::test]
    async fn missing_credentials_are_reported_before_any_request() {
        let http = Arc::new(StubClient::new(vec![]));
        let spotify = SpotifyProvider::new(http.clone(), Arc::new(MemoryStore::new()));

        assert!(matches!(
            spotify.status(),
            ProviderStatus::NeedsCredentials { .. }
        ));
        let error = spotify.search(&Query::new("x")).await.unwrap_err();
        assert!(matches!(error, SourceError::MissingCredentials(_)));
        assert!(
            http.requested.lock().unwrap().is_empty(),
            "asked the network despite having no key"
        );
    }

    #[tokio::test]
    async fn the_token_is_reused_rather_than_refetched() {
        let http = Arc::new(StubClient::new(vec![
            json!({"access_token": "tok", "expires_in": 3600}),
            search_body(),
            search_body(),
        ]));
        let spotify = SpotifyProvider::new(http.clone(), stocked_secrets());

        spotify.search(&Query::new("one")).await.unwrap();
        spotify.search(&Query::new("two")).await.unwrap();

        let urls = http.requested.lock().unwrap().clone();
        let token_requests = urls.iter().filter(|u| u.contains("token")).count();
        assert_eq!(token_requests, 1, "fetched a token for every search");
    }

    #[tokio::test]
    async fn the_query_is_url_encoded() {
        let http = Arc::new(StubClient::new(vec![
            json!({"access_token": "tok", "expires_in": 3600}),
            json!({"tracks": {"items": []}}),
        ]));
        let spotify = SpotifyProvider::new(http.clone(), stocked_secrets());
        spotify.search(&Query::new("romeo & juliet")).await.unwrap();
        let url = http.last_url();
        assert!(
            url.contains("romeo%20%26%20juliet"),
            "unencoded query: {url}"
        );
    }

    #[tokio::test]
    async fn a_response_in_an_unexpected_shape_yields_nothing_rather_than_panicking() {
        for body in [
            json!({}),
            json!({"tracks": {}}),
            json!({"tracks": {"items": "no"}}),
        ] {
            assert!(parse_search(&body).is_empty());
        }
    }

    #[tokio::test]
    async fn a_token_response_without_a_token_is_reported_clearly() {
        let http = Arc::new(StubClient::new(vec![json!({"error": "invalid_client"})]));
        let spotify = SpotifyProvider::new(http, stocked_secrets());
        let error = spotify.search(&Query::new("x")).await.unwrap_err();
        assert!(error.to_string().contains("access_token"));
    }
}

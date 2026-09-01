//! Every source, in one place.
//!
//! The registry is what the browser and the settings panel talk to. It owns the
//! providers, reports their status, and — the part that earns its existence —
//! **searches all of them at once and folds the results together**.
//!
//! That fold is where the design pays off. A search returns local files that
//! are ready to play, Spotify results that can only be planned with, and
//! YouTube results that need a file the user already has. Each carries its own
//! honest `playable` flag, and results from metadata-only providers are matched
//! against the local library so that "plan from Spotify, play from your own
//! files" is one action rather than a chore.

use crate::http::{HttpClient, ReqwestClient};
use crate::local::LocalLibrary;
use crate::partner::PartnerProvider;
use crate::provider::{ProviderId, ProviderStatus, Query, SourceError, SourceProvider, TrackRef};
use crate::{free, spotify, youtube};
use dj_secrets::SecretStore;
use serde::Serialize;
use std::sync::Arc;

/// One provider's search results, or the reason there were none.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderResults {
    pub provider: ProviderId,
    pub label: &'static str,
    pub tracks: Vec<TrackRef>,
    /// Set when the search did not happen or failed. Reported rather than
    /// swallowed: a browser that silently omits a source the user configured
    /// is worse than one that says "Spotify: check your client secret".
    pub error: Option<String>,
    /// True when a result was matched to a local file, so a metadata-only hit
    /// became loadable.
    pub matched_locally: usize,
}

/// Everything the settings panel needs about one provider, right now.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderState {
    pub info: &'static crate::catalog::SourceInfo,
    pub status: ProviderStatus,
}

/// The set of sources the application knows about.
#[derive(Debug)]
pub struct SourceRegistry {
    local: Arc<LocalLibrary>,
    providers: Vec<Arc<dyn SourceProvider>>,
    /// The client every provider here shares, when there is one.
    ///
    /// Kept so that anything else in the application needing to make a request
    /// — the lyrics database, which is not a source of *audio* and so is not a
    /// provider — uses the same connection pool and the same TLS setup rather
    /// than building a second one. `None` on a machine where the client could
    /// not be constructed, which is the same condition that leaves this
    /// registry local-only.
    http: Option<Arc<dyn HttpClient>>,
}

impl SourceRegistry {
    /// Build the standard set, talking to the real network.
    ///
    /// Falls back to a registry with only the local library if the HTTP client
    /// cannot be constructed — the application must still start and still play
    /// files when TLS initialisation fails on some unusual machine.
    #[must_use]
    pub fn new(secrets: Arc<dyn SecretStore>) -> Self {
        match ReqwestClient::new() {
            Ok(http) => Self::with_http(Arc::new(http), secrets),
            Err(error) => {
                tracing::warn!(%error, "no HTTP client; only local files will be searchable");
                Self::local_only()
            }
        }
    }

    /// Build with a specific HTTP client. The seam the tests use.
    #[must_use]
    pub fn with_http(http: Arc<dyn HttpClient>, secrets: Arc<dyn SecretStore>) -> Self {
        let local = Arc::new(LocalLibrary::new());
        let mut providers: Vec<Arc<dyn SourceProvider>> = vec![
            Arc::clone(&local) as Arc<dyn SourceProvider>,
            Arc::new(free::JamendoProvider::new(
                Arc::clone(&http),
                Arc::clone(&secrets),
            )),
            Arc::new(free::ArchiveProvider::new(Arc::clone(&http))),
            Arc::new(spotify::SpotifyProvider::new(
                Arc::clone(&http),
                Arc::clone(&secrets),
            )),
            Arc::new(youtube::YouTubeProvider::youtube(
                Arc::clone(&http),
                Arc::clone(&secrets),
            )),
            Arc::new(youtube::YouTubeProvider::music(
                Arc::clone(&http),
                Arc::clone(&secrets),
            )),
        ];
        for partner in PartnerProvider::all(&secrets) {
            providers.push(Arc::new(partner));
        }
        Self {
            local,
            providers,
            http: Some(http),
        }
    }

    /// The shared HTTP client, when this registry has one.
    #[must_use]
    pub fn http(&self) -> Option<Arc<dyn HttpClient>> {
        self.http.clone()
    }

    #[must_use]
    pub fn local_only() -> Self {
        let local = Arc::new(LocalLibrary::new());
        Self {
            providers: vec![Arc::clone(&local) as Arc<dyn SourceProvider>],
            local,
            http: None,
        }
    }

    #[must_use]
    pub fn local(&self) -> &Arc<LocalLibrary> {
        &self.local
    }

    #[must_use]
    pub fn provider(&self, id: ProviderId) -> Option<&Arc<dyn SourceProvider>> {
        self.providers.iter().find(|p| p.id() == id)
    }

    /// What the settings panel renders: every source, described and situated.
    ///
    /// Includes providers that are not usable, because "why can I not use
    /// Beatsource" is the question the panel exists to answer.
    #[must_use]
    pub fn states(&self) -> Vec<ProviderState> {
        crate::catalog::catalog()
            .iter()
            .map(|info| ProviderState {
                info,
                status: self
                    .provider(info.id)
                    .map_or(ProviderStatus::Disabled, |p| p.status()),
            })
            .collect()
    }

    /// Search one provider.
    pub async fn search_one(&self, id: ProviderId, query: &Query) -> ProviderResults {
        let label = id.label();
        let Some(provider) = self.provider(id) else {
            return ProviderResults {
                provider: id,
                label,
                tracks: Vec::new(),
                error: Some("this source is not enabled".into()),
                matched_locally: 0,
            };
        };

        if !provider.status().is_usable() {
            return ProviderResults {
                provider: id,
                label,
                tracks: Vec::new(),
                error: Some(describe(&provider.status())),
                matched_locally: 0,
            };
        }

        match provider.search(query).await {
            Ok(mut tracks) => {
                let matched = self.match_against_local(&mut tracks);
                ProviderResults {
                    provider: id,
                    label,
                    tracks,
                    error: None,
                    matched_locally: matched,
                }
            }
            Err(error) => ProviderResults {
                provider: id,
                label,
                tracks: Vec::new(),
                error: Some(error.to_string()),
                matched_locally: 0,
            },
        }
    }

    /// Search every usable provider.
    ///
    /// Sequential rather than concurrent, deliberately: several of these APIs
    /// rate-limit by client, and a burst of parallel requests on a venue's wifi
    /// is how a search ends up slower than doing them in order. Revisit when
    /// there is a measurement saying otherwise.
    pub async fn search_all(&self, query: &Query) -> Vec<ProviderResults> {
        let mut out = Vec::new();
        for provider in &self.providers {
            let status = provider.status();
            // Silently skip what the user has not set up. Reporting "Beatport:
            // needs a partnership" on every single search would be noise; the
            // settings panel is where that belongs.
            if !status.is_usable() {
                continue;
            }
            out.push(self.search_one(provider.id(), query).await);
        }
        out
    }

    /// Point metadata-only results at a local file where one exists.
    ///
    /// This is what turns a Spotify search into something you can actually
    /// load. Returns how many were matched.
    fn match_against_local(&self, tracks: &mut [TrackRef]) -> usize {
        let mut matched = 0;
        for track in tracks.iter_mut() {
            if track.playable || track.provider == ProviderId::Local {
                continue;
            }
            if let Some(local) = self.local.match_track(track) {
                // Keep the remote provider's richer metadata -- artwork, album,
                // the web link -- and swap in the local path so it can be
                // loaded. `provider` becomes Local because that is who will
                // resolve it.
                track.provider = ProviderId::Local;
                track.id = local.path.to_string_lossy().to_string();
                track.playable = true;
                matched += 1;
            }
        }
        matched
    }

    /// Turn a search result into something loadable.
    pub async fn resolve(
        &self,
        track: &TrackRef,
    ) -> Result<crate::provider::Playable, SourceError> {
        self.provider(track.provider)
            .ok_or(SourceError::NotPlayable {
                provider: track.provider.label(),
                reason: "this source is not enabled",
            })?
            .resolve(track)
            .await
    }
}

/// A status, in a sentence a user can act on.
fn describe(status: &ProviderStatus) -> String {
    match status {
        ProviderStatus::Ready => "ready".into(),
        ProviderStatus::NeedsCredentials { missing } => {
            let names: Vec<&str> = missing.iter().map(|k| k.label()).collect();
            format!("needs {}", names.join(" and "))
        }
        ProviderStatus::PartnerGated { reason } => (*reason).to_string(),
        ProviderStatus::Disabled => "turned off".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubClient;
    use dj_secrets::{MemoryStore, Secret, SecretKind};
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("djmanzo-registry-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn empty_secrets() -> Arc<dyn SecretStore> {
        Arc::new(MemoryStore::new())
    }

    #[test]
    fn every_catalogued_source_has_a_state() {
        let registry =
            SourceRegistry::with_http(Arc::new(StubClient::new(vec![])), empty_secrets());
        let states = registry.states();
        assert_eq!(states.len(), ProviderId::all().len());
        // And nothing reports Disabled, because all of them are constructed.
        assert!(
            !states
                .iter()
                .any(|s| matches!(s.status, ProviderStatus::Disabled)),
            "a catalogued source has no provider behind it"
        );
    }

    /// The application has to be useful before the user has signed up for
    /// anything at all.
    #[test]
    fn local_and_the_archive_work_with_no_credentials() {
        let registry =
            SourceRegistry::with_http(Arc::new(StubClient::new(vec![])), empty_secrets());
        for id in [ProviderId::Local, ProviderId::InternetArchive] {
            assert!(
                registry.provider(id).unwrap().status().is_usable(),
                "{id:?} should work out of the box"
            );
        }
        for id in [
            ProviderId::Spotify,
            ProviderId::YouTube,
            ProviderId::Jamendo,
        ] {
            assert!(matches!(
                registry.provider(id).unwrap().status(),
                ProviderStatus::NeedsCredentials { .. }
            ));
        }
    }

    #[tokio::test]
    async fn searching_all_skips_what_is_not_configured() {
        let dir = temp_dir("skip");
        fs::write(dir.join("Romeo Santos - Propuesta.mp3"), b"x").unwrap();

        let registry = SourceRegistry::with_http(
            // Only the Archive will fire, and it gets the one stubbed response.
            Arc::new(StubClient::new(vec![json!({"response": {"docs": []}})])),
            empty_secrets(),
        );
        registry.local().add_root(&dir);

        let results = registry.search_all(&Query::new("propuesta")).await;
        let searched: Vec<ProviderId> = results.iter().map(|r| r.provider).collect();
        assert!(searched.contains(&ProviderId::Local));
        assert!(!searched.contains(&ProviderId::Spotify), "{searched:?}");
        assert!(!searched.contains(&ProviderId::Beatport), "{searched:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// The fold that makes metadata-only providers worth having: a Spotify
    /// result the user owns becomes loadable.
    #[tokio::test]
    async fn a_spotify_result_becomes_playable_when_the_file_is_owned() {
        let dir = temp_dir("fold");
        fs::write(dir.join("Romeo Santos - Propuesta Indecente.mp3"), b"x").unwrap();

        let memory = MemoryStore::new();
        memory
            .set(SecretKind::SpotifyClientId, &Secret::new("id"))
            .unwrap();
        memory
            .set(SecretKind::SpotifyClientSecret, &Secret::new("secret"))
            .unwrap();

        let http = Arc::new(StubClient::new(vec![
            json!({"access_token": "tok", "expires_in": 3600}),
            json!({"tracks": {"items": [{
                "id": "abc",
                "name": "Propuesta Indecente",
                "artists": [{"name": "Romeo Santos"}]
            }]}}),
        ]));
        let registry = SourceRegistry::with_http(http, Arc::new(memory));
        registry.local().add_root(&dir);

        let results = registry
            .search_one(ProviderId::Spotify, &Query::new("propuesta"))
            .await;

        assert_eq!(results.matched_locally, 1);
        let track = &results.tracks[0];
        assert!(track.playable, "an owned track stayed unplayable");
        assert_eq!(
            track.provider,
            ProviderId::Local,
            "resolution must go local"
        );
        // The richer metadata survived the swap.
        assert_eq!(track.title, "Propuesta Indecente");

        // And it really does resolve to the file.
        assert!(registry.resolve(track).await.is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A Spotify result the user does *not* own must stay unplayable. This is
    /// the failure mode the whole design exists to prevent.
    #[tokio::test]
    async fn a_spotify_result_the_user_does_not_own_stays_unplayable() {
        let memory = MemoryStore::new();
        memory
            .set(SecretKind::SpotifyClientId, &Secret::new("id"))
            .unwrap();
        memory
            .set(SecretKind::SpotifyClientSecret, &Secret::new("secret"))
            .unwrap();

        let http = Arc::new(StubClient::new(vec![
            json!({"access_token": "tok", "expires_in": 3600}),
            json!({"tracks": {"items": [{
                "id": "abc", "name": "Nothing Owned", "artists": [{"name": "Nobody"}]
            }]}}),
        ]));
        let registry = SourceRegistry::with_http(http, Arc::new(memory));

        let results = registry
            .search_one(ProviderId::Spotify, &Query::new("nothing"))
            .await;
        assert_eq!(results.matched_locally, 0);
        assert!(!results.tracks[0].playable);
        assert_eq!(results.tracks[0].provider, ProviderId::Spotify);
        assert!(registry.resolve(&results.tracks[0]).await.is_err());
    }

    /// A configured source that fails must say so rather than vanish from the
    /// results, or the user concludes their music does not exist.
    #[tokio::test]
    async fn a_failing_provider_reports_its_error() {
        let registry = SourceRegistry::with_http(
            Arc::new(StubClient::failing("connection refused")),
            empty_secrets(),
        );
        let results = registry
            .search_one(ProviderId::InternetArchive, &Query::new("x"))
            .await;
        assert!(results.tracks.is_empty());
        assert!(
            results
                .error
                .as_deref()
                .unwrap_or("")
                .contains("connection refused"),
            "{:?}",
            results.error
        );
    }

    #[tokio::test]
    async fn asking_an_unconfigured_source_directly_explains_what_is_missing() {
        let registry =
            SourceRegistry::with_http(Arc::new(StubClient::new(vec![])), empty_secrets());
        let results = registry
            .search_one(ProviderId::Spotify, &Query::new("x"))
            .await;
        let error = results.error.unwrap_or_default();
        assert!(error.contains("client ID"), "{error}");
    }

    #[tokio::test]
    async fn asking_a_partner_source_explains_the_partnership() {
        let registry =
            SourceRegistry::with_http(Arc::new(StubClient::new(vec![])), empty_secrets());
        let results = registry
            .search_one(ProviderId::Beatsource, &Query::new("bachata"))
            .await;
        assert!(
            results.error.unwrap_or_default().contains("partnership"),
            "a partner source should explain itself"
        );
    }

    #[test]
    fn a_registry_with_no_network_still_plays_local_files() {
        let registry = SourceRegistry::local_only();
        assert!(registry.provider(ProviderId::Local).is_some());
        assert!(registry.provider(ProviderId::Spotify).is_none());
        // The settings panel still lists everything, marked unavailable.
        assert_eq!(registry.states().len(), ProviderId::all().len());
    }
}

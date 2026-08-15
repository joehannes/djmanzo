//! What a music source is, and what it is allowed to do.
//!
//! The load-bearing type here is [`AudioAccess`]. Whether a service may supply
//! audio a DJ can mix is a *licensing* fact, not a technical one, and it varies
//! per service in ways nobody can be expected to hold in their head. So it is
//! encoded once, in the type system, and the code physically cannot get it
//! wrong later: [`SourceProvider::resolve`] refuses by default, and a provider
//! that must never hand over audio simply does not override it.
//!
//! See [ADR-0006](../../../docs/adr/0006-music-sources-and-licensing.md).

use dj_secrets::SecretKind;
use serde::Serialize;
use std::path::PathBuf;

/// Every source the application knows about.
///
/// A closed set, so the settings panel, the browser and the licensing table
/// cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    /// Files on this machine. The only source that needs nobody's permission.
    Local,
    /// Discovery and planning only. Spotify's policy forbids mixing.
    Spotify,
    YouTube,
    YouTubeMusic,
    /// Creative Commons, downloadable, genuinely mixable.
    Jamendo,
    /// Public domain and CC recordings, including a lot of historical material.
    InternetArchive,
    Beatport,
    Beatsource,
    Tidal,
    SoundCloud,
}

impl ProviderId {
    #[must_use]
    pub const fn all() -> &'static [ProviderId] {
        use ProviderId::*;
        &[
            Local,
            Jamendo,
            InternetArchive,
            Spotify,
            YouTube,
            YouTubeMusic,
            Beatsource,
            Beatport,
            Tidal,
            SoundCloud,
        ]
    }

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            ProviderId::Local => "local",
            ProviderId::Spotify => "spotify",
            ProviderId::YouTube => "youtube",
            ProviderId::YouTubeMusic => "youtube_music",
            ProviderId::Jamendo => "jamendo",
            ProviderId::InternetArchive => "internet_archive",
            ProviderId::Beatport => "beatport",
            ProviderId::Beatsource => "beatsource",
            ProviderId::Tidal => "tidal",
            ProviderId::SoundCloud => "soundcloud",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ProviderId::Local => "Local library",
            ProviderId::Spotify => "Spotify",
            ProviderId::YouTube => "YouTube",
            ProviderId::YouTubeMusic => "YouTube Music",
            ProviderId::Jamendo => "Jamendo",
            ProviderId::InternetArchive => "Internet Archive",
            ProviderId::Beatport => "Beatport Streaming",
            ProviderId::Beatsource => "Beatsource Streaming",
            ProviderId::Tidal => "TIDAL",
            ProviderId::SoundCloud => "SoundCloud",
        }
    }
}

/// How, if at all, a provider can supply audio to a deck.
///
/// The variants are not a convenience. `None` is the difference between a
/// working integration and a policy violation, and it carries its reason so the
/// interface can say *why* rather than showing a disabled button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioAccess {
    /// The provider hands over something the decoder can open.
    Direct,
    /// The user must obtain the audio themselves. Off unless switched on, and
    /// the application neither ships a downloader nor makes the decision.
    UserSupplied { note: &'static str },
    /// No audio from here, ever. Search results are matched against the user's
    /// own files instead.
    None { reason: &'static str },
}

impl AudioAccess {
    /// Whether a deck may ever be fed from this provider.
    #[must_use]
    pub const fn is_playable(self) -> bool {
        !matches!(self, AudioAccess::None { .. })
    }
}

/// What a provider can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Capabilities {
    pub search: bool,
    /// Can import the user's playlists, for planning a set.
    pub playlists: bool,
    pub audio: AudioAccess,
}

/// Why a provider is or is not usable right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Usable now.
    Ready,
    /// Would work, but the user has not supplied credentials yet.
    NeedsCredentials { missing: Vec<SecretKind> },
    /// Credentials alone are not enough: the service requires a commercial
    /// agreement this project does not hold. Stated plainly rather than
    /// presented as a bug the user might fix by trying harder.
    PartnerGated { reason: &'static str },
    /// The user turned it off.
    Disabled,
}

impl ProviderStatus {
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, ProviderStatus::Ready)
    }
}

/// A track as a source describes it, before anything is downloaded or decoded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackRef {
    pub provider: ProviderId,
    /// Whatever the provider uses to identify this track. Opaque to us.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_seconds: Option<f32>,
    pub bpm: Option<f32>,
    /// Musical key, in whatever notation the provider uses.
    pub key: Option<String>,
    pub genre: Option<String>,
    /// Artwork, for the browser.
    pub artwork_url: Option<String>,
    /// Where a human can go to see or buy this. Always safe to open.
    pub web_url: Option<String>,
    /// Set when the provider can hand the audio straight over.
    pub playable: bool,
}

impl TrackRef {
    /// A short label for logs and the assistant.
    #[must_use]
    pub fn describe(&self) -> String {
        format!("{} — {}", self.artist, self.title)
    }
}

/// Something the engine can actually load.
#[derive(Debug, Clone, PartialEq)]
pub enum Playable {
    /// A file the decoder can open.
    File(PathBuf),
    /// A URL to stream. Only ever produced by a licensed provider.
    Stream {
        url: String,
        /// Streaming URLs are usually short-lived; re-resolve after this.
        expires_in_seconds: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub text: String,
    pub limit: usize,
}

impl Query {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 25,
        }
    }

    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        // A provider asked for zero results does pointless work; asked for ten
        // thousand it hammers someone's API. Both are clamped here rather than
        // trusted from a UI field.
        self.limit = limit.clamp(1, 200);
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("{provider} cannot supply playable audio: {reason}")]
    NotPlayable {
        provider: &'static str,
        reason: &'static str,
    },
    #[error("{0} needs credentials that have not been set")]
    MissingCredentials(&'static str),
    #[error("{provider} requires a partnership: {reason}")]
    PartnerGated {
        provider: &'static str,
        reason: &'static str,
    },
    #[error("network error talking to {provider}: {message}")]
    Network {
        provider: &'static str,
        message: String,
    },
    #[error("{provider} returned something unexpected: {message}")]
    BadResponse {
        provider: &'static str,
        message: String,
    },
    #[error("{0}")]
    Io(String),
}

/// A place tracks come from.
#[async_trait::async_trait]
pub trait SourceProvider: Send + Sync + std::fmt::Debug {
    fn id(&self) -> ProviderId;

    fn capabilities(&self) -> Capabilities;

    /// Whether this provider can be used right now, and if not, why.
    fn status(&self) -> ProviderStatus;

    async fn search(&self, query: &Query) -> Result<Vec<TrackRef>, SourceError>;

    /// Turn a search result into something a deck can load.
    ///
    /// **The default refuses**, and that is the point. A provider forbidden
    /// from supplying audio does not override this, so there is no path by
    /// which a future change accidentally routes Spotify into a deck — it would
    /// have to be written deliberately, in the provider, next to the comment
    /// explaining why it must not be.
    async fn resolve(&self, _track: &TrackRef) -> Result<Playable, SourceError> {
        let reason = match self.capabilities().audio {
            AudioAccess::None { reason } => reason,
            _ => "this provider has not implemented audio resolution",
        };
        Err(SourceError::NotPlayable {
            provider: self.id().label(),
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Forbidden;

    #[async_trait::async_trait]
    impl SourceProvider for Forbidden {
        fn id(&self) -> ProviderId {
            ProviderId::Spotify
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                search: true,
                playlists: true,
                audio: AudioAccess::None {
                    reason: "policy forbids mixing",
                },
            }
        }
        fn status(&self) -> ProviderStatus {
            ProviderStatus::Ready
        }
        async fn search(&self, _query: &Query) -> Result<Vec<TrackRef>, SourceError> {
            Ok(Vec::new())
        }
    }

    /// The whole design in one test: a provider that must not supply audio
    /// cannot, without someone writing code to make it.
    #[tokio::test]
    async fn a_provider_that_may_not_play_refuses_by_default() {
        let track = TrackRef {
            provider: ProviderId::Spotify,
            id: "x".into(),
            title: "t".into(),
            artist: "a".into(),
            album: None,
            duration_seconds: None,
            bpm: None,
            key: None,
            genre: None,
            artwork_url: None,
            web_url: None,
            playable: false,
        };
        let error = Forbidden.resolve(&track).await.unwrap_err();
        assert!(matches!(error, SourceError::NotPlayable { .. }));
        // And the reason travels with it, so the UI can explain rather than
        // just grey something out.
        assert!(error.to_string().contains("policy forbids mixing"));
    }

    #[test]
    fn audio_access_none_is_never_playable() {
        assert!(!AudioAccess::None { reason: "" }.is_playable());
        assert!(AudioAccess::Direct.is_playable());
        assert!(AudioAccess::UserSupplied { note: "" }.is_playable());
    }

    #[test]
    fn provider_slugs_are_unique() {
        use std::collections::HashSet;
        let slugs: HashSet<&str> = ProviderId::all().iter().map(|p| p.slug()).collect();
        assert_eq!(slugs.len(), ProviderId::all().len());
    }

    #[test]
    fn every_provider_is_listed() {
        // `all()` is what the settings panel iterates. A provider missing from
        // it is invisible in the UI while still existing in the type system.
        for id in [
            ProviderId::Local,
            ProviderId::Spotify,
            ProviderId::YouTube,
            ProviderId::YouTubeMusic,
            ProviderId::Jamendo,
            ProviderId::InternetArchive,
            ProviderId::Beatport,
            ProviderId::Beatsource,
            ProviderId::Tidal,
            ProviderId::SoundCloud,
        ] {
            assert!(ProviderId::all().contains(&id), "{id:?} missing from all()");
        }
    }

    #[test]
    fn a_query_cannot_ask_for_nothing_or_for_everything() {
        assert_eq!(Query::new("x").with_limit(0).limit, 1);
        assert_eq!(Query::new("x").with_limit(10_000).limit, 200);
    }
}

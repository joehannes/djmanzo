//! Credential storage.
//!
//! API keys go in the operating system's credential store -- Keychain on macOS,
//! Secret Service on Linux. Never in a config file, never in the session log,
//! never in a crash report.
//!
//! # Why not a config file
//!
//! A DJ application's config directory gets copied between machines, synced to
//! cloud storage, and pasted into forum posts when something breaks. An API key
//! sitting in it is a key that will eventually leak. The OS keychain is
//! encrypted at rest, access-controlled, and excluded from ordinary backups.
//!
//! # What this module refuses to do
//!
//! There is no `list_all_secrets`, no way to read a key back into the UI, and
//! [`SecretRef`] deliberately does not implement `Display`. Keys go in, and they
//! come out only at the point of an API call. The settings panel shows
//! [`Secret::hint`] -- the last four characters -- which is enough to tell two
//! keys apart and useless to anyone who reads it over your shoulder.

use serde::{Serialize, Serializer};
use std::fmt;

/// Serialised as its stable [`SecretKind::id`], never as a Rust variant name.
///
/// The id is what the settings panel sends back when the user fills a field, so
/// the wire format and the keychain entry name are deliberately the same
/// string: one thing to keep stable instead of two.
impl Serialize for SecretKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

/// Which credential a secret belongs to.
///
/// A closed set rather than free-form strings, so a typo cannot silently create
/// an orphaned keychain entry that nothing ever reads or cleans up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SecretKind {
    OpenRouter,
    Anthropic,
    OpenAi,
    GoogleAi,
    Groq,
    /// Spotify, for discovery and planning only -- never audio. See
    /// `docs/adr/0006-music-sources-and-licensing.md`.
    SpotifyClientId,
    SpotifyClientSecret,
    YouTubeApi,
    KaggleUsername,
    KaggleKey,
    /// Jamendo. Creative Commons, genuinely mixable, free key.
    JamendoClientId,
    /// The licensed DJ streaming services. Each needs a partnership before the
    /// credentials mean anything -- see `docs/SOURCES.md`.
    BeatportClientId,
    BeatportClientSecret,
    BeatsourceClientId,
    BeatsourceClientSecret,
    TidalClientId,
    TidalClientSecret,
    SoundCloudClientId,
    SoundCloudClientSecret,
}

impl SecretKind {
    /// Stable identifier used as the keychain entry name.
    ///
    /// Changing one of these orphans an existing entry, so they are treated as
    /// a storage format: append, never rename.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            SecretKind::OpenRouter => "openrouter.api_key",
            SecretKind::Anthropic => "anthropic.api_key",
            SecretKind::OpenAi => "openai.api_key",
            SecretKind::GoogleAi => "googleai.api_key",
            SecretKind::Groq => "groq.api_key",
            SecretKind::SpotifyClientId => "spotify.client_id",
            SecretKind::SpotifyClientSecret => "spotify.client_secret",
            SecretKind::YouTubeApi => "youtube.api_key",
            SecretKind::KaggleUsername => "kaggle.username",
            SecretKind::KaggleKey => "kaggle.key",
            SecretKind::JamendoClientId => "jamendo.client_id",
            SecretKind::BeatportClientId => "beatport.client_id",
            SecretKind::BeatportClientSecret => "beatport.client_secret",
            SecretKind::BeatsourceClientId => "beatsource.client_id",
            SecretKind::BeatsourceClientSecret => "beatsource.client_secret",
            SecretKind::TidalClientId => "tidal.client_id",
            SecretKind::TidalClientSecret => "tidal.client_secret",
            SecretKind::SoundCloudClientId => "soundcloud.client_id",
            SecretKind::SoundCloudClientSecret => "soundcloud.client_secret",
        }
    }

    /// Human-readable name for the settings panel.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            SecretKind::OpenRouter => "OpenRouter API key",
            SecretKind::Anthropic => "Anthropic API key",
            SecretKind::OpenAi => "OpenAI API key",
            SecretKind::GoogleAi => "Google AI Studio API key",
            SecretKind::Groq => "Groq API key",
            SecretKind::SpotifyClientId => "Spotify client ID",
            SecretKind::SpotifyClientSecret => "Spotify client secret",
            SecretKind::YouTubeApi => "YouTube Data API key",
            SecretKind::KaggleUsername => "Kaggle username",
            SecretKind::KaggleKey => "Kaggle API key",
            SecretKind::JamendoClientId => "Jamendo client ID",
            SecretKind::BeatportClientId => "Beatport client ID",
            SecretKind::BeatportClientSecret => "Beatport client secret",
            SecretKind::BeatsourceClientId => "Beatsource client ID",
            SecretKind::BeatsourceClientSecret => "Beatsource client secret",
            SecretKind::TidalClientId => "TIDAL client ID",
            SecretKind::TidalClientSecret => "TIDAL client secret",
            SecretKind::SoundCloudClientId => "SoundCloud client ID",
            SecretKind::SoundCloudClientSecret => "SoundCloud client secret",
        }
    }

    /// Where to go and get one.
    ///
    /// Shown next to the field, because "paste your API key" without a link is
    /// a dead end for anyone who has not done it before.
    #[must_use]
    pub const fn signup_url(self) -> &'static str {
        match self {
            SecretKind::OpenRouter => "https://openrouter.ai/keys",
            SecretKind::Anthropic => "https://console.anthropic.com/settings/keys",
            SecretKind::OpenAi => "https://platform.openai.com/api-keys",
            SecretKind::GoogleAi => "https://aistudio.google.com/apikey",
            SecretKind::Groq => "https://console.groq.com/keys",
            SecretKind::SpotifyClientId | SecretKind::SpotifyClientSecret => {
                "https://developer.spotify.com/dashboard"
            }
            SecretKind::YouTubeApi => "https://console.cloud.google.com/apis/credentials",
            SecretKind::KaggleUsername | SecretKind::KaggleKey => {
                "https://www.kaggle.com/settings/account"
            }
            SecretKind::JamendoClientId => "https://devportal.jamendo.com/",
            SecretKind::BeatportClientId | SecretKind::BeatportClientSecret => {
                "https://api.beatport.com/v4/docs/"
            }
            SecretKind::BeatsourceClientId | SecretKind::BeatsourceClientSecret => {
                "https://www.beatsource.com/link"
            }
            SecretKind::TidalClientId | SecretKind::TidalClientSecret => {
                "https://developer.tidal.com/"
            }
            SecretKind::SoundCloudClientId | SecretKind::SoundCloudClientSecret => {
                "https://developers.soundcloud.com/"
            }
        }
    }

    /// What the free tier actually gives you, stated honestly.
    #[must_use]
    pub const fn free_tier(self) -> &'static str {
        match self {
            SecretKind::OpenRouter => {
                "Free: a rotating set of models tagged `:free`. One key reaches \
                 hundreds of models, free and paid. The easiest place to start."
            }
            SecretKind::Anthropic | SecretKind::OpenAi => {
                "Trial credit on signup, then pay as you go."
            }
            SecretKind::GoogleAi => "Generous free tier.",
            SecretKind::Groq => {
                "Free tier, and very fast -- which matters most for voice, where \
                 latency is felt directly."
            }
            SecretKind::SpotifyClientId | SecretKind::SpotifyClientSecret => {
                "Free. Discovery and planning only: Spotify's policy forbids \
                 mixing their audio, so tracks are matched to your own files."
            }
            SecretKind::YouTubeApi => "Free quota, sufficient for search.",
            SecretKind::KaggleUsername | SecretKind::KaggleKey => {
                "Free GPU notebook hours, shared and finite. Enough to generate \
                 a track during a set, not to generate one on demand."
            }
            SecretKind::JamendoClientId => {
                "Free. Creative Commons catalogue, downloadable, and one of the \
                 few online sources you may genuinely mix without asking anyone."
            }
            SecretKind::BeatportClientId | SecretKind::BeatportClientSecret => {
                "Needs a Beatport Streaming partnership, not just an account. \
                 Electronic-focused."
            }
            SecretKind::BeatsourceClientId | SecretKind::BeatsourceClientSecret => {
                "Needs a Beatsource Streaming partnership. The open-format one \
                 -- hip-hop, Latin, dancehall, reggaeton -- so the relevant one \
                 for a Dominican set."
            }
            SecretKind::TidalClientId | SecretKind::TidalClientSecret => {
                "Developer access is open; DJ-mixing rights are a separate \
                 commercial agreement."
            }
            SecretKind::SoundCloudClientId | SecretKind::SoundCloudClientSecret => {
                "API registration has been closed to new applications for years. \
                 Go+ for DJs is a partner programme."
            }
        }
    }

    /// Every credential the application knows about.
    ///
    /// A slice rather than a fixed-size array: the length changed three times
    /// while sources were being added, and each change was a compile error in
    /// an unrelated place for no benefit.
    /// Look one up by its stable id, for values arriving from the interface.
    #[must_use]
    pub fn from_id(id: &str) -> Option<SecretKind> {
        Self::all().iter().copied().find(|kind| kind.id() == id)
    }

    #[must_use]
    pub const fn all() -> &'static [SecretKind] {
        use SecretKind::*;
        &[
            OpenRouter,
            Anthropic,
            OpenAi,
            GoogleAi,
            Groq,
            SpotifyClientId,
            SpotifyClientSecret,
            YouTubeApi,
            KaggleUsername,
            KaggleKey,
            JamendoClientId,
            BeatportClientId,
            BeatportClientSecret,
            BeatsourceClientId,
            BeatsourceClientSecret,
            TidalClientId,
            TidalClientSecret,
            SoundCloudClientId,
            SoundCloudClientSecret,
        ]
    }
}

/// A secret's value, wrapped so it cannot be logged by accident.
///
/// `Debug` prints a placeholder and there is no `Display`. Reading the real
/// value takes an explicit [`Secret::expose`], which is greppable in review.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The actual value. Call this only at the point of use.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Last four characters, for telling two keys apart in the UI.
    #[must_use]
    pub fn hint(&self) -> String {
        let visible: String = self
            .0
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if self.0.chars().count() <= 4 {
            "•".repeat(self.0.chars().count())
        } else {
            format!("••••{visible}")
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("no {0} stored")]
    NotFound(&'static str),
    #[error("keychain unavailable: {0}")]
    Backend(String),
    #[error("refusing to store an empty {0}")]
    Empty(&'static str),
}

/// Somewhere secrets can live.
///
/// A trait so tests -- and any environment with no keychain, such as a headless
/// CI runner -- can substitute an in-memory store without touching the callers.
pub trait SecretStore: Send + Sync + std::fmt::Debug {
    fn get(&self, kind: SecretKind) -> Result<Secret, SecretError>;
    fn set(&self, kind: SecretKind, secret: &Secret) -> Result<(), SecretError>;
    fn delete(&self, kind: SecretKind) -> Result<(), SecretError>;

    /// Whether a secret is present, without reading it.
    fn has(&self, kind: SecretKind) -> bool {
        self.get(kind).is_ok()
    }
}

/// The application's identity in the credential store.
const SERVICE: &str = "app.djmanzo.desktop";

/// The real store, backed by the OS keychain.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[derive(Debug, Default)]
pub struct KeychainStore;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
impl KeychainStore {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn entry(kind: SecretKind) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, kind.id()).map_err(|e| SecretError::Backend(e.to_string()))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
impl SecretStore for KeychainStore {
    fn get(&self, kind: SecretKind) -> Result<Secret, SecretError> {
        let entry = Self::entry(kind)?;
        match entry.get_password() {
            Ok(value) => Ok(Secret::new(value)),
            Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(kind.id())),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }

    fn set(&self, kind: SecretKind, secret: &Secret) -> Result<(), SecretError> {
        if secret.is_empty() {
            return Err(SecretError::Empty(kind.id()));
        }
        Self::entry(kind)?
            .set_password(secret.expose())
            .map_err(|e| SecretError::Backend(e.to_string()))
    }

    fn delete(&self, kind: SecretKind) -> Result<(), SecretError> {
        let entry = Self::entry(kind)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }
}

/// An in-memory store.
///
/// Used by tests, and as a fallback on a machine with no working keychain --
/// a headless CI runner or a minimal Linux install with no Secret Service.
/// Secrets do not survive a restart, which is the correct trade: the
/// alternative is silently writing them to disk.
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: std::sync::Mutex<std::collections::HashMap<SecretKind, Secret>>,
}

impl MemoryStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, kind: SecretKind) -> Result<Secret, SecretError> {
        self.entries
            .lock()
            .map_err(|_| SecretError::Backend("poisoned".into()))?
            .get(&kind)
            .cloned()
            .ok_or(SecretError::NotFound(kind.id()))
    }

    fn set(&self, kind: SecretKind, secret: &Secret) -> Result<(), SecretError> {
        if secret.is_empty() {
            return Err(SecretError::Empty(kind.id()));
        }
        self.entries
            .lock()
            .map_err(|_| SecretError::Backend("poisoned".into()))?
            .insert(kind, secret.clone());
        Ok(())
    }

    fn delete(&self, kind: SecretKind) -> Result<(), SecretError> {
        self.entries
            .lock()
            .map_err(|_| SecretError::Backend("poisoned".into()))?
            .remove(&kind);
        Ok(())
    }
}

/// Open the best available store.
///
/// Tries the OS keychain and falls back to memory if it is unusable, reporting
/// which one was chosen so the UI can warn that keys will not persist.
#[must_use]
pub fn open_store() -> (Box<dyn SecretStore>, bool) {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let store = KeychainStore::new();
        // Probe with a read; a missing entry is fine, a broken backend is not.
        match store.get(SecretKind::OpenRouter) {
            Ok(_) | Err(SecretError::NotFound(_)) => return (Box::new(store), true),
            Err(_) => {}
        }
    }
    (Box::new(MemoryStore::new()), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_ids_are_unique() {
        use std::collections::HashSet;
        let ids: HashSet<&str> = SecretKind::all().iter().map(|k| k.id()).collect();
        assert_eq!(ids.len(), SecretKind::all().len(), "duplicate secret id");
    }

    #[test]
    fn every_secret_has_a_signup_link_and_a_free_tier_note() {
        for kind in SecretKind::all() {
            assert!(kind.signup_url().starts_with("https://"), "{:?}", kind);
            assert!(!kind.free_tier().is_empty(), "{:?}", kind);
            assert!(!kind.label().is_empty(), "{:?}", kind);
        }
    }

    /// The whole point of the wrapper: a key must not appear in a log line, and
    /// logs are exactly where secrets leak from.
    #[test]
    fn debug_never_reveals_the_value() {
        let secret = Secret::new("sk-or-v1-abcdef0123456789");
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("abcdef"), "leaked: {rendered}");
        assert!(!rendered.contains("sk-or"), "leaked: {rendered}");
        assert_eq!(rendered, "Secret(<redacted>)");
    }

    #[test]
    fn hint_shows_only_the_tail() {
        let secret = Secret::new("sk-or-v1-abcdef0123456789");
        let hint = secret.hint();
        assert_eq!(hint, "••••6789");
        assert!(!hint.contains("abcdef"));
    }

    #[test]
    fn hint_of_a_short_value_reveals_nothing() {
        assert_eq!(Secret::new("ab").hint(), "••");
        assert_eq!(Secret::new("abcd").hint(), "••••");
    }

    #[test]
    fn hint_handles_multibyte_characters() {
        // Slicing by byte would panic here.
        let hint = Secret::new("key-áéíóú").hint();
        assert!(hint.starts_with("••••"));
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::new();
        assert!(!store.has(SecretKind::OpenRouter));

        store
            .set(SecretKind::OpenRouter, &Secret::new("test-key"))
            .unwrap();
        assert!(store.has(SecretKind::OpenRouter));
        assert_eq!(
            store.get(SecretKind::OpenRouter).unwrap().expose(),
            "test-key"
        );
    }

    #[test]
    fn secrets_do_not_collide_across_kinds() {
        let store = MemoryStore::new();
        store
            .set(SecretKind::OpenRouter, &Secret::new("one"))
            .unwrap();
        store.set(SecretKind::Groq, &Secret::new("two")).unwrap();
        assert_eq!(store.get(SecretKind::OpenRouter).unwrap().expose(), "one");
        assert_eq!(store.get(SecretKind::Groq).unwrap().expose(), "two");
    }

    #[test]
    fn deleting_is_idempotent() {
        let store = MemoryStore::new();
        store.set(SecretKind::Groq, &Secret::new("x")).unwrap();
        store.delete(SecretKind::Groq).unwrap();
        // Deleting again must not error -- the desired state is already true.
        store.delete(SecretKind::Groq).unwrap();
        assert!(!store.has(SecretKind::Groq));
    }

    /// An empty key is almost always a UI mistake -- a cleared field submitted
    /// by accident. Storing it would replace a working key with nothing.
    #[test]
    fn empty_secrets_are_refused() {
        let store = MemoryStore::new();
        assert!(matches!(
            store.set(SecretKind::Groq, &Secret::new("")),
            Err(SecretError::Empty(_))
        ));
        assert!(matches!(
            store.set(SecretKind::Groq, &Secret::new("   ")),
            Err(SecretError::Empty(_))
        ));
    }

    #[test]
    fn missing_secrets_report_which_one() {
        let store = MemoryStore::new();
        let error = store.get(SecretKind::Anthropic).unwrap_err();
        assert!(error.to_string().contains("anthropic"));
    }

    #[test]
    fn open_store_always_returns_something_usable() {
        // Even with no keychain -- a headless runner -- the app must start.
        let (store, _persistent) = open_store();
        let _ = store.has(SecretKind::OpenRouter);
    }
}

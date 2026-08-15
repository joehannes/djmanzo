//! The licensed DJ streaming services.
//!
//! Beatport, Beatsource, TIDAL and SoundCloud all run — or have run —
//! DJ-specific streaming programmes. They are the only way to legally stream a
//! commercial catalogue into a mixer, which is exactly what a working DJ wants,
//! and none of them can be reached by writing better code. Each requires a
//! commercial agreement between the service and the *application*, not just a
//! subscription held by the user.
//!
//! So these are real providers with a real, honest status:
//! [`ProviderStatus::PartnerGated`]. They appear in the settings panel, they
//! accept and store credentials, and they say plainly that credentials alone
//! will not be enough. That is better than either pretending they work or
//! leaving them out — a DJ comparing applications needs to know djmanzo is
//! ready for them, and needs not to buy a subscription expecting it to work
//! today.
//!
//! When an agreement exists, the work is to implement `search` and `resolve`
//! against the service's API. Nothing above this layer changes.

use crate::provider::{
    AudioAccess, Capabilities, Playable, ProviderId, ProviderStatus, Query, SourceError,
    SourceProvider, TrackRef,
};
use dj_secrets::{SecretKind, SecretStore};
use std::sync::Arc;

/// A licensed service djmanzo is ready for but not yet admitted to.
#[derive(Debug)]
pub struct PartnerProvider {
    id: ProviderId,
    credentials: &'static [SecretKind],
    reason: &'static str,
    secrets: Arc<dyn SecretStore>,
}

impl PartnerProvider {
    #[must_use]
    pub fn beatsource(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            id: ProviderId::Beatsource,
            credentials: &[
                SecretKind::BeatsourceClientId,
                SecretKind::BeatsourceClientSecret,
            ],
            reason: "Beatsource Streaming needs a partnership agreement with \
                     Beatport, not only a subscription",
            secrets,
        }
    }

    #[must_use]
    pub fn beatport(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            id: ProviderId::Beatport,
            credentials: &[
                SecretKind::BeatportClientId,
                SecretKind::BeatportClientSecret,
            ],
            reason: "Beatport Streaming needs a partnership agreement, not only \
                     a subscription",
            secrets,
        }
    }

    #[must_use]
    pub fn tidal(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            id: ProviderId::Tidal,
            credentials: &[SecretKind::TidalClientId, SecretKind::TidalClientSecret],
            reason: "TIDAL developer access covers metadata; mixing rights are a \
                     separate commercial agreement",
            secrets,
        }
    }

    #[must_use]
    pub fn soundcloud(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            id: ProviderId::SoundCloud,
            credentials: &[
                SecretKind::SoundCloudClientId,
                SecretKind::SoundCloudClientSecret,
            ],
            reason: "SoundCloud closed API registration to new applications in \
                     2019, and Go+ for DJs is a partner programme",
            secrets,
        }
    }

    #[must_use]
    pub fn all(secrets: &Arc<dyn SecretStore>) -> Vec<Self> {
        vec![
            Self::beatsource(Arc::clone(secrets)),
            Self::beatport(Arc::clone(secrets)),
            Self::tidal(Arc::clone(secrets)),
            Self::soundcloud(Arc::clone(secrets)),
        ]
    }

    /// Whether the user has supplied every credential this service asks for.
    ///
    /// Tracked even though it changes nothing today: when an agreement lands,
    /// a user who set their keys up in advance should simply find it working.
    #[must_use]
    pub fn credentials_present(&self) -> bool {
        self.credentials.iter().all(|kind| self.secrets.has(*kind))
    }
}

#[async_trait::async_trait]
impl SourceProvider for PartnerProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    /// Reported as `Direct` deliberately.
    ///
    /// These services *can* legally supply mixable audio — that is their whole
    /// point, and the reason they belong in a different category from Spotify.
    /// What stops them today is the agreement, which [`Self::status`] reports.
    /// Conflating the two would hide the fact that Spotify is a permanent no
    /// and this is a paperwork problem.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        }
    }

    fn status(&self) -> ProviderStatus {
        ProviderStatus::PartnerGated {
            reason: self.reason,
        }
    }

    async fn search(&self, _query: &Query) -> Result<Vec<TrackRef>, SourceError> {
        Err(SourceError::PartnerGated {
            provider: self.id.label(),
            reason: self.reason,
        })
    }

    async fn resolve(&self, _track: &TrackRef) -> Result<Playable, SourceError> {
        Err(SourceError::PartnerGated {
            provider: self.id.label(),
            reason: self.reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_secrets::{MemoryStore, Secret};

    fn store() -> Arc<dyn SecretStore> {
        Arc::new(MemoryStore::new())
    }

    #[tokio::test]
    async fn a_partner_service_explains_itself_rather_than_failing_vaguely() {
        let beatsource = PartnerProvider::beatsource(store());
        let error = beatsource.search(&Query::new("bachata")).await.unwrap_err();
        let message = error.to_string();
        assert!(message.contains("Beatsource"), "{message}");
        assert!(message.contains("partnership"), "{message}");
    }

    #[test]
    fn every_partner_service_is_gated_and_says_why() {
        for provider in PartnerProvider::all(&store()) {
            match provider.status() {
                ProviderStatus::PartnerGated { reason } => {
                    assert!(reason.len() > 30, "{:?} gave a stub reason", provider.id())
                }
                other => panic!("{:?} was not gated: {other:?}", provider.id()),
            }
        }
    }

    /// The distinction that matters: these could legally play, and Spotify
    /// could not. Flattening them into one "unavailable" state would lose the
    /// difference between a paperwork problem and a permanent no.
    #[test]
    fn partner_services_are_capable_of_audio_even_while_gated() {
        for provider in PartnerProvider::all(&store()) {
            assert!(
                provider.capabilities().audio.is_playable(),
                "{:?} should be capable of audio; it is the agreement that is missing",
                provider.id()
            );
            assert!(!provider.status().is_usable());
        }
    }

    #[test]
    fn credentials_are_accepted_and_remembered_in_advance() {
        let memory = MemoryStore::new();
        memory
            .set(SecretKind::TidalClientId, &Secret::new("id"))
            .unwrap();
        memory
            .set(SecretKind::TidalClientSecret, &Secret::new("secret"))
            .unwrap();
        let secrets: Arc<dyn SecretStore> = Arc::new(memory);

        let tidal = PartnerProvider::tidal(Arc::clone(&secrets));
        assert!(tidal.credentials_present());
        // Still gated -- credentials are necessary, not sufficient.
        assert!(!tidal.status().is_usable());

        let beatport = PartnerProvider::beatport(secrets);
        assert!(!beatport.credentials_present());
    }

    #[test]
    fn all_four_services_are_present() {
        let ids: Vec<ProviderId> = PartnerProvider::all(&store())
            .iter()
            .map(SourceProvider::id)
            .collect();
        for expected in [
            ProviderId::Beatsource,
            ProviderId::Beatport,
            ProviderId::Tidal,
            ProviderId::SoundCloud,
        ] {
            assert!(ids.contains(&expected), "{expected:?} missing");
        }
    }
}

//! What each service actually offers, stated once.
//!
//! This file exists because the honest answer to "can I stream X into a deck?"
//! is different for every service, and none of the differences are technical.
//! Rather than scatter that across ten provider implementations and a settings
//! screen, it lives here as data — and the settings screen renders it directly,
//! so what the user reads is the same thing the code obeys.
//!
//! The tone is deliberate. A DJ deciding whether to pay for a service is owed a
//! straight answer, including when the answer is "this will not work for you and
//! here is why". See [ADR-0006](../../../docs/adr/0006-music-sources-and-licensing.md).

use crate::provider::{AudioAccess, Capabilities, ProviderId};
use dj_secrets::SecretKind;
use serde::Serialize;

/// Everything the interface needs to describe one source.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub id: ProviderId,
    pub label: &'static str,
    /// One line: what this is for.
    pub summary: &'static str,
    /// The honest paragraph, including what it will not do.
    pub detail: &'static str,
    pub capabilities: Capabilities,
    /// Credentials the user must supply, in the order they should be entered.
    pub credentials: &'static [SecretKind],
    /// Where to sign up, if there is anywhere.
    pub signup_url: Option<&'static str>,
    /// True when no amount of correct configuration will make it work without
    /// a commercial agreement.
    pub partner_gated: bool,
}

const NO_CREDENTIALS: &[SecretKind] = &[];

/// The full table.
#[must_use]
pub fn catalog() -> &'static [SourceInfo] {
    &CATALOG
}

/// Look one up.
#[must_use]
pub fn info(id: ProviderId) -> &'static SourceInfo {
    // `all()` and `CATALOG` are kept in step by a test, so this cannot miss.
    CATALOG
        .iter()
        .find(|entry| entry.id == id)
        .expect("every ProviderId has a catalog entry")
}

static CATALOG: [SourceInfo; 10] = [
    SourceInfo {
        id: ProviderId::Local,
        label: "Local library",
        summary: "Your own files. Always mixable, always available.",
        detail: "Folders on this machine, indexed and searchable. No account, no \
                 network, no terms of service, and nothing that stops working \
                 when a venue's wifi does. Every other source on this list is a \
                 way of deciding what to put here.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: NO_CREDENTIALS,
        signup_url: None,
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::Jamendo,
        label: "Jamendo",
        summary: "Creative Commons catalogue. Free, downloadable, genuinely yours to mix.",
        detail: "One of the very few online sources you may mix without asking \
                 anyone. Independent artists releasing under Creative Commons; \
                 the API is free and returns direct audio URLs. Not where you \
                 will find a chart bachata, but a real catalogue and a good place \
                 to test that the whole path works before paying for anything.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: &[SecretKind::JamendoClientId],
        signup_url: Some("https://devportal.jamendo.com/"),
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::InternetArchive,
        label: "Internet Archive",
        summary: "Public domain and freely-licensed recordings. No key needed.",
        detail: "Live concert recordings, historical material, and a deep seam of \
                 public-domain music — including a lot of early Caribbean and \
                 Latin recording. Open API, no registration. Audio quality varies \
                 enormously; check before you rely on something in a set.",
        capabilities: Capabilities {
            search: true,
            playlists: false,
            audio: AudioAccess::Direct,
        },
        credentials: NO_CREDENTIALS,
        signup_url: Some("https://archive.org/details/audio"),
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::Spotify,
        label: "Spotify",
        summary: "Discovery and planning. Never audio — their policy forbids it.",
        detail: "Spotify's developer policy forbids using their content to \
                 \"segue, mix, re-mix, or overlap\" it with other audio. That \
                 sentence is a description of DJing, so this is not a limitation \
                 that a better integration could work around, and the apps that \
                 do stream Spotify hold individually negotiated licences.\n\n\
                 What it is good for: searching, importing your playlists and \
                 saved tracks, and planning a set. Results are matched against \
                 your own files, so you can plan from a Spotify playlist and play \
                 from music you own.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::None {
                reason: "Spotify's developer policy forbids mixing their audio \
                         with anything else",
            },
        },
        credentials: &[SecretKind::SpotifyClientId, SecretKind::SpotifyClientSecret],
        signup_url: Some("https://developer.spotify.com/dashboard"),
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::YouTube,
        label: "YouTube",
        summary: "Search and metadata. Audio only from files you obtain yourself.",
        detail: "The Data API gives search and metadata on a free quota, which is \
                 genuinely useful for finding an edit, a version, or something \
                 that exists nowhere else.\n\n\
                 It does not give audio a mixer can use: playback is licensed only \
                 through YouTube's own embedded player, which cannot be routed \
                 into a deck. If you have a local copy of something — your own \
                 upload, a Creative Commons release, a promo you were sent — you \
                 can point djmanzo at the file and it will be matched to the \
                 search result. djmanzo ships no downloader and makes no \
                 acquisition decisions for you.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::UserSupplied {
                note: "YouTube licenses playback only through its own player. \
                       Match a result to a file you already hold.",
            },
        },
        credentials: &[SecretKind::YouTubeApi],
        signup_url: Some("https://console.cloud.google.com/apis/credentials"),
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::YouTubeMusic,
        label: "YouTube Music",
        summary: "Search and playlist import. There is no legal streaming route for a mixer.",
        detail: "Worth stating plainly, because it is the thing people most often \
                 hope for: **YouTube Music has no public API, and no route by \
                 which a third-party application may stream its audio into a \
                 mixer** — not with Premium, not with a paid key. The unofficial \
                 clients that circulate work by impersonating the web player, \
                 which breaks the terms and gets accounts closed.\n\n\
                 What does work, using the same YouTube Data API key: finding \
                 tracks and importing your playlists, so a set can be planned from \
                 your YouTube Music library and played from files you own. If a \
                 sanctioned API ever appears, this provider is where it lands.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::None {
                reason: "no public API offers YouTube Music audio to a \
                         third-party mixer, at any subscription tier",
            },
        },
        credentials: &[SecretKind::YouTubeApi],
        signup_url: Some("https://console.cloud.google.com/apis/credentials"),
        partner_gated: false,
    },
    SourceInfo {
        id: ProviderId::Beatsource,
        label: "Beatsource Streaming",
        summary: "Licensed DJ streaming, open format. The right one for a Latin set.",
        detail: "Beatsource is the open-format arm of Beatport: hip-hop, R&B, \
                 dancehall, reggaeton, bachata, merengue — the repertoire a \
                 working party DJ actually needs, licensed specifically for DJ \
                 use.\n\n\
                 A subscription lets you stream inside partner applications. \
                 Wiring a *new* application in needs a partnership agreement with \
                 Beatport, not just an account, so the slot here is ready and the \
                 credentials will be honoured the moment that exists.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: &[
            SecretKind::BeatsourceClientId,
            SecretKind::BeatsourceClientSecret,
        ],
        signup_url: Some("https://www.beatsource.com/link"),
        partner_gated: true,
    },
    SourceInfo {
        id: ProviderId::Beatport,
        label: "Beatport Streaming",
        summary: "Licensed DJ streaming, electronic focus. Needs a partnership.",
        detail: "The same arrangement as Beatsource, aimed at electronic music: \
                 house, techno, drum and bass, and the remix pools around them. \
                 Same requirement — a partnership agreement, not just a \
                 subscription.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: &[
            SecretKind::BeatportClientId,
            SecretKind::BeatportClientSecret,
        ],
        signup_url: Some("https://api.beatport.com/v4/docs/"),
        partner_gated: true,
    },
    SourceInfo {
        id: ProviderId::Tidal,
        label: "TIDAL",
        summary: "Broad catalogue, high quality. DJ rights are a separate agreement.",
        detail: "TIDAL's developer programme is open and its catalogue is large \
                 and well mastered, which is why several DJ applications support \
                 it. The developer credentials get you metadata; the right to mix \
                 the audio is a separate commercial agreement, so this stays a \
                 partner slot until that exists.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: &[SecretKind::TidalClientId, SecretKind::TidalClientSecret],
        signup_url: Some("https://developer.tidal.com/"),
        partner_gated: true,
    },
    SourceInfo {
        id: ProviderId::SoundCloud,
        label: "SoundCloud",
        summary: "Where edits and bootlegs live. API registration has been shut for years.",
        detail: "SoundCloud is where a great deal of the interesting material is — \
                 edits, bootlegs, DJ tools, local scenes that never reach a \
                 distributor. It is also the hardest to reach: new API application \
                 registrations have been closed since 2019 with no reopening \
                 announced, and Go+ for DJs is a partner programme.\n\n\
                 If you hold an existing client ID from before the shutdown, it \
                 will be used. Otherwise this stays a slot.",
        capabilities: Capabilities {
            search: true,
            playlists: true,
            audio: AudioAccess::Direct,
        },
        credentials: &[
            SecretKind::SoundCloudClientId,
            SecretKind::SoundCloudClientSecret,
        ],
        signup_url: Some("https://developers.soundcloud.com/"),
        partner_gated: true,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_an_entry() {
        for id in ProviderId::all() {
            let entry = info(*id);
            assert_eq!(entry.id, *id);
        }
        assert_eq!(CATALOG.len(), ProviderId::all().len());
    }

    #[test]
    fn every_entry_says_something_useful() {
        for entry in catalog() {
            assert!(!entry.summary.is_empty(), "{:?}", entry.id);
            // The detail is the part a user reads before spending money. A
            // one-liner there would be worse than nothing.
            assert!(
                entry.detail.len() > 120,
                "{:?} needs a real explanation, not a stub",
                entry.id
            );
        }
    }

    /// The rule ADR-0006 exists to enforce.
    #[test]
    fn spotify_and_youtube_music_can_never_supply_audio() {
        for id in [ProviderId::Spotify, ProviderId::YouTubeMusic] {
            assert!(
                !info(id).capabilities.audio.is_playable(),
                "{id:?} must never be playable"
            );
        }
    }

    #[test]
    fn a_source_that_needs_a_key_says_where_to_get_one() {
        for entry in catalog() {
            if !entry.credentials.is_empty() {
                assert!(
                    entry.signup_url.is_some(),
                    "{:?} asks for credentials with no link to obtain them",
                    entry.id
                );
            }
        }
        // And every link is real enough to be a link.
        for entry in catalog() {
            if let Some(url) = entry.signup_url {
                assert!(url.starts_with("https://"), "{:?}: {url}", entry.id);
            }
        }
    }

    /// A partner-gated source must not pretend a key is all it takes.
    #[test]
    fn partner_gated_sources_say_so_in_their_detail() {
        for entry in catalog().iter().filter(|e| e.partner_gated) {
            let detail = entry.detail.to_lowercase();
            assert!(
                detail.contains("partner") || detail.contains("agreement"),
                "{:?} is partner-gated but does not explain that",
                entry.id
            );
        }
    }

    /// The two sources that work with no account at all, which is what makes
    /// them the right default.
    #[test]
    fn something_works_without_any_credentials() {
        let free: Vec<_> = catalog()
            .iter()
            .filter(|e| e.credentials.is_empty() && !e.partner_gated)
            .map(|e| e.id)
            .collect();
        assert!(
            free.contains(&ProviderId::Local) && free.contains(&ProviderId::InternetArchive),
            "the application must be useful before the user has signed up for anything: {free:?}"
        );
    }
}

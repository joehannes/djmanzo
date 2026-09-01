//! Where tracks come from.
//!
//! A DJ application has to answer an awkward question honestly: *which of these
//! services can I actually mix from?* The answers differ per service, none of
//! the differences are technical, and getting one wrong means either a broken
//! feature or a licensing violation.
//!
//! So the answer is encoded rather than remembered. Every provider declares its
//! [`provider::Capabilities`], the trait's `resolve` refuses by default, and
//! [`catalog`] holds one honest paragraph per service that the settings panel
//! renders verbatim — so what the user reads is the same thing the code obeys.
//!
//! In brief:
//!
//! | Source | Search | Mixable audio |
//! |---|---|---|
//! | Local files | yes | **yes** |
//! | Jamendo | yes | **yes** — Creative Commons |
//! | Internet Archive | yes | **yes** — public domain, no key needed |
//! | Spotify | yes | **never** — their policy forbids mixing |
//! | YouTube | yes | only from a file you already hold |
//! | YouTube Music | yes | **no** — no sanctioned API exists at any tier |
//! | Beatsource / Beatport / TIDAL / SoundCloud | ready | needs a partnership |
//!
//! See [ADR-0006](../../../docs/adr/0006-music-sources-and-licensing.md) and
//! `docs/SOURCES.md`.

pub mod catalog;
pub mod free;
pub mod http;
pub mod local;
pub mod lyrics;
pub mod partner;
pub mod provider;
pub mod registry;
pub mod spotify;
pub mod youtube;

pub use catalog::{SourceInfo, catalog, info};
pub use http::{HttpClient, ReqwestClient};
pub use local::{LocalLibrary, LocalTrack};
pub use lyrics::{Lyrics, LyricsError, LyricsSource};
pub use provider::{
    AudioAccess, Capabilities, Playable, ProviderId, ProviderStatus, Query, SourceError,
    SourceProvider, TrackRef,
};
pub use registry::{ProviderResults, ProviderState, SourceRegistry};

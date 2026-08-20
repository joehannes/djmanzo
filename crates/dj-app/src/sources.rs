//! Commands for finding music, and for the credentials that unlock it.
//!
//! Two responsibilities that belong together because they are one workflow: a
//! source is useless without its key, and a key is meaningless without knowing
//! what the source will then do for you. So the settings panel renders
//! [`dj_sources::catalog`] directly — the same honest paragraph the code obeys —
//! and the browser searches whatever that leaves usable.
//!
//! Secrets go in and never come back out. [`secret_status`] reports only
//! whether one is set and the last four characters, which is enough to tell two
//! keys apart and useless to anyone reading over the DJ's shoulder.

use crate::state::AppState;
use dj_secrets::{Secret, SecretKind};
use dj_sources::{Playable, ProviderId, Query, TrackRef};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{Manager, State};

/// Where fetched audio is kept, under the app's cache directory.
const CACHE_SUBDIR: &str = "stream-cache";

/// One credential field, as the settings panel draws it.
#[derive(Debug, Clone, Serialize)]
pub struct CredentialDto {
    /// Stable id; sent back verbatim when the user fills the field.
    pub id: &'static str,
    pub label: &'static str,
    pub signup_url: &'static str,
    pub free_tier: &'static str,
    pub is_set: bool,
    /// Last four characters of a stored value, or empty.
    pub hint: String,
}

/// One source, fully described.
#[derive(Debug, Clone, Serialize)]
pub struct SourceDto {
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
    pub can_search: bool,
    /// `direct`, `user_supplied` or `none`.
    pub audio: &'static str,
    /// Why audio is unavailable, when it is.
    pub audio_note: &'static str,
    pub partner_gated: bool,
    pub credentials: Vec<CredentialDto>,
    /// `ready`, `needs_credentials`, `partner_gated` or `disabled`.
    pub status: &'static str,
    /// A sentence the user can act on.
    pub status_detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResultsDto {
    pub provider: &'static str,
    pub label: &'static str,
    pub tracks: Vec<TrackRef>,
    pub error: Option<String>,
    pub matched_locally: usize,
}

/// A track handed back from the browser to be loaded.
///
/// The interface returns what search gave it rather than an opaque handle, so
/// there is no server-side result table to keep in step with what is on screen.
#[derive(Debug, Clone, Deserialize)]
pub struct TrackPayload {
    pub provider: String,
    pub id: String,
    pub title: String,
    pub artist: String,
}

fn provider_from_slug(slug: &str) -> Option<ProviderId> {
    ProviderId::all().iter().copied().find(|p| p.slug() == slug)
}

fn audio_kind(access: dj_sources::AudioAccess) -> (&'static str, &'static str) {
    match access {
        dj_sources::AudioAccess::Direct => ("direct", ""),
        dj_sources::AudioAccess::UserSupplied { note } => ("user_supplied", note),
        dj_sources::AudioAccess::None { reason } => ("none", reason),
    }
}

/// Every source, with its status and its credential fields.
#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Vec<SourceDto> {
    let secrets = state.secrets();
    state
        .sources()
        .states()
        .into_iter()
        .map(|entry| {
            let (audio, audio_note) = audio_kind(entry.info.capabilities.audio);
            let (status, status_detail) = match &entry.status {
                dj_sources::ProviderStatus::Ready => ("ready", "Ready".to_owned()),
                dj_sources::ProviderStatus::NeedsCredentials { missing } => (
                    "needs_credentials",
                    format!(
                        "Needs {}",
                        missing
                            .iter()
                            .map(|k| k.label())
                            .collect::<Vec<_>>()
                            .join(" and ")
                    ),
                ),
                dj_sources::ProviderStatus::PartnerGated { reason } => {
                    ("partner_gated", (*reason).to_owned())
                }
                dj_sources::ProviderStatus::Disabled => ("disabled", "Turned off".to_owned()),
            };

            SourceDto {
                id: entry.info.id.slug(),
                label: entry.info.label,
                summary: entry.info.summary,
                detail: entry.info.detail,
                can_search: entry.info.capabilities.search,
                audio,
                audio_note,
                partner_gated: entry.info.partner_gated,
                status,
                status_detail,
                credentials: entry
                    .info
                    .credentials
                    .iter()
                    .map(|kind| {
                        let stored = secrets.get(*kind).ok();
                        CredentialDto {
                            id: kind.id(),
                            label: kind.label(),
                            signup_url: kind.signup_url(),
                            free_tier: kind.free_tier(),
                            is_set: stored.is_some(),
                            hint: stored.map(|s| s.hint()).unwrap_or_default(),
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Store a credential. The value is never read back to the interface.
#[tauri::command]
pub fn set_secret(state: State<'_, AppState>, id: String, value: String) -> Result<(), String> {
    let kind = SecretKind::from_id(&id).ok_or_else(|| format!("unknown credential `{id}`"))?;
    state
        .secrets()
        .set(kind, &Secret::new(value))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_secret(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let kind = SecretKind::from_id(&id).ok_or_else(|| format!("unknown credential `{id}`"))?;
    state.secrets().delete(kind).map_err(|e| e.to_string())
}

/// Whether credentials survive a restart.
///
/// False on a machine with no working keychain, which the settings panel says
/// out loud rather than letting the user discover it after a reboot.
#[tauri::command]
pub fn secrets_persist(state: State<'_, AppState>) -> bool {
    state.secrets_persist()
}

// ---------------------------------------------------------------------------
// The local library
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn add_music_folder(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(format!("{} is not a folder", path.display()));
    }
    Ok(state.sources().local().add_root(path))
}

/// Where this platform keeps music, if the folder is actually there.
///
/// A first launch with an empty collection is the most common way somebody
/// sees djmanzo, and "Add a folder" asks them to go and find one before
/// anything works. Every operating system already knows where music lives —
/// `~/Music` on macOS, `$XDG_MUSIC_DIR` on Linux — so the first run can offer
/// it by name and be one click instead of a file dialog.
///
/// `None` rather than a guess when the folder does not exist: offering to scan
/// a directory that is not there would fail on the click, which is worse than
/// not offering.
#[tauri::command]
#[must_use]
pub fn default_music_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    let found = app.path().audio_dir().ok()?;
    found.is_dir().then(|| found.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn remove_music_folder(state: State<'_, AppState>, path: String) {
    state.sources().local().remove_root(PathBuf::from(path));
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryDto {
    pub folders: Vec<String>,
    pub tracks: usize,
}

#[tauri::command]
pub fn music_library(state: State<'_, AppState>) -> LibraryDto {
    let local = state.sources().local();
    LibraryDto {
        folders: local
            .roots()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        tracks: local.len(),
    }
}

// ---------------------------------------------------------------------------
// Searching
// ---------------------------------------------------------------------------

/// Search one source, or every usable one.
#[tauri::command]
pub async fn search_sources(
    state: State<'_, AppState>,
    text: String,
    provider: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchResultsDto>, String> {
    let query = Query::new(text).with_limit(limit.unwrap_or(25));
    let results = match provider {
        Some(slug) => {
            let id = provider_from_slug(&slug).ok_or_else(|| format!("unknown source `{slug}`"))?;
            vec![state.sources().search_one(id, &query).await]
        }
        None => state.sources().search_all(&query).await,
    };

    Ok(results
        .into_iter()
        .map(|r| SearchResultsDto {
            provider: r.provider.slug(),
            label: r.label,
            tracks: r.tracks,
            error: r.error,
            matched_locally: r.matched_locally,
        })
        .collect())
}

/// Turn a search result into a file on disk, fetching it if need be.
///
/// Streaming URLs are downloaded to a cache first rather than decoded from the
/// network. A DJ set is the wrong place to discover that a track stalls because
/// the venue's wifi dropped: once a track is on disk it stays playable whatever
/// the network does, and the second load is instant.
#[tauri::command]
pub async fn resolve_source_track(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    track: TrackPayload,
) -> Result<String, String> {
    let provider = provider_from_slug(&track.provider)
        .ok_or_else(|| format!("unknown source `{}`", track.provider))?;

    let reference = TrackRef {
        provider,
        id: track.id,
        title: track.title,
        artist: track.artist,
        album: None,
        duration_seconds: None,
        bpm: None,
        key: None,
        genre: None,
        artwork_url: None,
        web_url: None,
        playable: true,
    };

    match state
        .sources()
        .resolve(&reference)
        .await
        .map_err(|e| e.to_string())?
    {
        Playable::File(path) => Ok(path.to_string_lossy().to_string()),
        Playable::Stream { url, .. } => {
            let dir = app
                .path()
                .app_cache_dir()
                .map_err(|e| e.to_string())?
                .join(CACHE_SUBDIR);
            fetch_to_cache(&url, &dir).await
        }
    }
}

/// Download `url` into `dir`, or return the copy already there.
async fn fetch_to_cache(url: &str, dir: &std::path::Path) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;

    // Content-addressed by URL so the same track fetched twice is fetched once,
    // and so a cache file can never collide with an unrelated track.
    let name = format!("{:016x}", fnv1a(url.as_bytes()));
    let extension = guess_extension(url);
    let path = dir.join(format!("{name}.{extension}"));
    if path.is_file() {
        return Ok(path.to_string_lossy().to_string());
    }

    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("{} returned {}", url, response.status()));
    }
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    // Write beside the target and rename, so an interrupted download never
    // leaves a truncated file that looks cached.
    let partial = path.with_extension(format!("{extension}.partial"));
    std::fs::write(&partial, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Guess a container from the URL, defaulting to MP3.
///
/// Only used to name the cache file; the decoder probes the content rather than
/// trusting the extension, so a wrong guess costs nothing.
fn guess_extension(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    let stem = lower.split(['?', '#']).next().unwrap_or(&lower);
    for candidate in ["flac", "wav", "aiff", "ogg", "opus", "m4a", "aac", "mp3"] {
        if stem.ends_with(candidate) {
            return match candidate {
                "flac" => "flac",
                "wav" => "wav",
                "aiff" => "aiff",
                "ogg" => "ogg",
                "opus" => "opus",
                "m4a" => "m4a",
                "aac" => "aac",
                _ => "mp3",
            };
        }
    }
    "mp3"
}

/// FNV-1a. Not cryptographic and does not need to be — this names a cache file.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_slugs_round_trip() {
        for id in ProviderId::all() {
            assert_eq!(provider_from_slug(id.slug()), Some(*id));
        }
        assert_eq!(provider_from_slug("nonsense"), None);
    }

    #[test]
    fn the_cache_name_depends_on_the_whole_url() {
        // Two tracks from the same host must not collide.
        assert_ne!(
            fnv1a(b"https://x/track?id=1"),
            fnv1a(b"https://x/track?id=2")
        );
        // And the same URL must always land on the same file, or nothing is
        // ever a cache hit.
        assert_eq!(fnv1a(b"https://x/a"), fnv1a(b"https://x/a"));
    }

    #[test]
    fn extensions_are_guessed_from_the_path_not_the_query() {
        assert_eq!(guess_extension("https://x/song.flac"), "flac");
        assert_eq!(guess_extension("https://x/song.MP3"), "mp3");
        // Jamendo's URLs carry the format in the query and nothing in the path.
        assert_eq!(guess_extension("https://x/?trackid=1&format=mp32"), "mp3");
        // A query string ending in something that looks like an extension must
        // not be mistaken for one.
        assert_eq!(guess_extension("https://x/stream?next=a.flac"), "mp3");
    }

    #[test]
    fn audio_kinds_map_to_stable_strings() {
        assert_eq!(audio_kind(dj_sources::AudioAccess::Direct).0, "direct");
        assert_eq!(
            audio_kind(dj_sources::AudioAccess::None { reason: "no" }),
            ("none", "no")
        );
        assert_eq!(
            audio_kind(dj_sources::AudioAccess::UserSupplied {
                note: "bring your own"
            }),
            ("user_supplied", "bring your own")
        );
    }
}

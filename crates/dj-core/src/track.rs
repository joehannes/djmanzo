//! Track identity and metadata.

use crate::music::{Beatgrid, Bpm, MusicalKey};
use crate::time::SampleRate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Identifies a track by the content of its audio, not by where it lives.
///
/// A moved or renamed file keeps its cues, grid and stem cache; a re-encoded one
/// correctly does not. The hash is computed once at import and stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackId([u8; 32]);

impl TrackId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Lowercase hex, for cache filenames and database keys.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut s = String::with_capacity(64);
        for byte in self.0 {
            s.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            s.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        s
    }
}

/// What the library knows about a track. Analysis fields are `None` until the
/// analyser has run, and the UI is expected to show that state rather than
/// pretending a BPM of 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_frames: u64,
    pub sample_rate: SampleRate,
    pub channels: u16,
    pub bpm: Option<Bpm>,
    pub key: Option<MusicalKey>,
    pub beatgrid: Option<Beatgrid>,
    /// Integrated loudness in LUFS, for auto-gain.
    pub loudness_lufs: Option<f64>,
}

impl TrackInfo {
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.duration_frames as f64 / self.sample_rate.as_f64()
    }

    /// Best available display name. Falls back through tags to the filename,
    /// because a track with no tags still has to be findable in the browser.
    #[must_use]
    pub fn display_title(&self) -> String {
        if let Some(title) = &self.title {
            return title.clone();
        }
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    #[must_use]
    pub fn display_artist(&self) -> &str {
        self.artist.as_deref().unwrap_or("Unknown artist")
    }

    /// True once the track has everything sync and harmonic mixing need.
    #[must_use]
    pub fn is_analysed(&self) -> bool {
        self.beatgrid.is_some() && self.key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str, title: Option<&str>) -> TrackInfo {
        TrackInfo {
            id: TrackId::from_bytes([0u8; 32]),
            path: PathBuf::from(path),
            title: title.map(str::to_owned),
            artist: None,
            album: None,
            duration_frames: 48_000 * 200,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            bpm: None,
            key: None,
            beatgrid: None,
            loudness_lufs: None,
        }
    }

    #[test]
    fn hex_encoding_is_full_width() {
        let id = TrackId::from_bytes([0xab; 32]);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c == 'a' || c == 'b'));
    }

    #[test]
    fn hex_pads_low_bytes() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x0f;
        assert!(TrackId::from_bytes(bytes).to_hex().starts_with("0f"));
    }

    #[test]
    fn untagged_track_falls_back_to_filename() {
        let t = track("/music/Some Track.flac", None);
        assert_eq!(t.display_title(), "Some Track");
        assert_eq!(t.display_artist(), "Unknown artist");
    }

    #[test]
    fn tagged_track_uses_its_tag() {
        let t = track("/music/whatever.flac", Some("Real Title"));
        assert_eq!(t.display_title(), "Real Title");
    }

    #[test]
    fn duration_converts_to_seconds() {
        assert!((track("/a.wav", None).duration_seconds() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn unanalysed_track_reports_as_such() {
        assert!(!track("/a.wav", None).is_analysed());
    }
}

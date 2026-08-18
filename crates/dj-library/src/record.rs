//! What a row in the library is, in Rust.
//!
//! Deliberately not [`dj_core::TrackInfo`]. That type is what the *engine* and
//! the deck need: enough to play a track and beatmatch it. A library row is
//! what a browser needs: everything above plus the tags, the file's state on
//! disk, and the performance history — fields the audio path has no business
//! carrying through a lock-free queue.

use dj_core::{Beatgrid, Bpm, Confidence, FramePos, Mode, MusicalKey, SampleRate, TrackId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One track in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryTrack {
    pub id: TrackId,
    /// Where the file was last seen. Not the identity — see the schema.
    pub path: PathBuf,
    pub tags: Tags,
    pub duration_frames: u64,
    pub sample_rate: SampleRate,
    pub channels: u16,
    /// Size and modification time as last scanned, so a rescan can skip a file
    /// that cannot have changed without decoding it.
    pub file_size: Option<u64>,
    pub file_modified: Option<i64>,
    /// Unix seconds.
    pub added_at: i64,
    pub analysis: StoredAnalysis,
    pub stats: PlayStats,
}

/// What the file's own metadata says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    /// Record label. Read from the tag where there is one; DJs sort by it.
    pub label: Option<String>,
    pub comment: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<u32>,
}

/// The analyser's findings, as stored.
///
/// Flat rather than nesting [`Beatgrid`]: a database row is flat, and a shape
/// that matches the row is one fewer place for a partially-populated grid to
/// hide. [`Self::beatgrid`] reassembles it, and returns `None` unless *every*
/// part is present — a grid with a tempo and no anchor is not a grid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct StoredAnalysis {
    pub bpm: Option<f64>,
    /// Where the grid came from, which decides what may replace it. `None`
    /// when there is no grid. See [`GridSource`].
    pub grid_source: Option<GridSource>,
    pub grid_anchor: Option<f64>,
    pub grid_beats_per_bar: Option<u8>,
    pub grid_confidence: Option<f64>,
    pub key_hour: Option<u8>,
    pub key_mode: Option<Mode>,
    pub key_confidence: Option<f64>,
    pub loudness_lufs: Option<f64>,
}

impl StoredAnalysis {
    /// Rebuild the grid, or `None` if any part of it is missing.
    #[must_use]
    pub fn beatgrid(&self) -> Option<Beatgrid> {
        Some(Beatgrid {
            anchor: FramePos::new(self.grid_anchor?),
            bpm: Bpm::new(self.bpm?)?,
            beats_per_bar: self.grid_beats_per_bar?,
            confidence: Confidence::new(self.grid_confidence.unwrap_or(0.0)),
        })
    }

    /// Flatten a grid into the stored form.
    ///
    /// Defaults the source to the analyser, which is where all but two grids
    /// come from; [`Self::from`] on a source overrides it.
    #[must_use]
    pub fn with_beatgrid(mut self, grid: Beatgrid) -> Self {
        self.bpm = Some(grid.bpm.get());
        self.grid_anchor = Some(grid.anchor.get());
        self.grid_beats_per_bar = Some(grid.beats_per_bar);
        self.grid_confidence = Some(grid.confidence.get());
        self.grid_source.get_or_insert(GridSource::Analysis);
        self
    }

    /// Say where the grid came from.
    #[must_use]
    pub fn from_source(mut self, source: GridSource) -> Self {
        self.grid_source = Some(source);
        self
    }

    #[must_use]
    pub fn key(&self) -> Option<MusicalKey> {
        MusicalKey::new(self.key_hour?, self.key_mode?)
    }

    #[must_use]
    pub fn with_key(mut self, key: MusicalKey, confidence: f64) -> Self {
        self.key_hour = Some(key.hour());
        self.key_mode = Some(key.mode());
        self.key_confidence = Some(confidence);
        self
    }

    /// True once the track has everything sync and harmonic mixing need.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.beatgrid().is_some() && self.key().is_some()
    }
}

/// Where a beat grid came from, and therefore what may replace it.
///
/// The ordering is the authority ordering: a hand edit outranks an import,
/// an import outranks an analysis, and an analysis only fills in a blank. That
/// is derived from `Ord` rather than written out in each comparison, so there
/// is one place the rule lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GridSource {
    /// Our own analyser. The weakest claim: it is a measurement, and the two
    /// below are somebody's judgement.
    Analysis,
    /// Another application's library, which a DJ has been playing from.
    Import,
    /// Edited here, by hand. Nothing may replace this.
    Manual,
}

impl GridSource {
    #[must_use]
    pub const fn as_sql(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::Import => "import",
            Self::Manual => "manual",
        }
    }

    #[must_use]
    pub fn from_sql(word: &str) -> Option<Self> {
        match word {
            "analysis" => Some(Self::Analysis),
            "import" => Some(Self::Import),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    /// Whether a grid from `self` may replace one already recorded as `existing`.
    ///
    /// An equal source may replace itself: re-analysing is allowed to improve
    /// an analysis, and re-importing is allowed to correct an import.
    #[must_use]
    pub fn may_replace(self, existing: Option<Self>) -> bool {
        match existing {
            None => true,
            Some(existing) => self >= existing,
        }
    }
}

/// How the track has been used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayStats {
    pub play_count: i64,
    /// Unix seconds.
    pub last_played: Option<i64>,
    /// 0..=5, as every library since iTunes.
    pub rating: Option<u8>,
}

/// A hot cue as stored. The engine's copy is a bare frame; this carries the
/// label and colour a browser shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredCue {
    /// 1-based, as the interface and every controller number them.
    pub slot: u8,
    pub frame: f64,
    pub label: Option<String>,
    pub colour: Option<String>,
}

/// A saved loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredLoop {
    pub slot: u8,
    pub start_frame: f64,
    pub end_frame: f64,
    pub label: Option<String>,
}

impl LibraryTrack {
    /// Best available display name, falling through tags to the filename.
    ///
    /// A track with no tags at all still has to be findable in the browser,
    /// and a row reading "Untitled" among four hundred others is not findable.
    #[must_use]
    pub fn display_title(&self) -> String {
        if let Some(title) = self.tags.title.as_deref()
            && !title.trim().is_empty()
        {
            return title.to_owned();
        }
        self.path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    #[must_use]
    pub fn display_artist(&self) -> &str {
        self.tags
            .artist
            .as_deref()
            .filter(|a| !a.trim().is_empty())
            .unwrap_or("Unknown artist")
    }

    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.duration_frames as f64 / self.sample_rate.as_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis() -> StoredAnalysis {
        StoredAnalysis::default()
    }

    #[test]
    fn a_grid_missing_any_part_is_not_a_grid() {
        assert!(analysis().beatgrid().is_none());

        let mut partial = analysis();
        partial.bpm = Some(128.0);
        assert!(
            partial.beatgrid().is_none(),
            "a tempo with no anchor is not a grid; returning one would put every \
             beat in the wrong place"
        );

        partial.grid_anchor = Some(0.0);
        assert!(partial.beatgrid().is_none(), "still no bar length");

        partial.grid_beats_per_bar = Some(4);
        assert!(partial.beatgrid().is_some());
    }

    #[test]
    fn a_grid_survives_the_round_trip() {
        let grid = Beatgrid::new(
            FramePos::new(12_345.0),
            Bpm::new(128.5).unwrap(),
            Confidence::new(0.9),
        );
        let stored = analysis().with_beatgrid(grid);
        assert_eq!(stored.beatgrid(), Some(grid));
    }

    /// An out-of-range tempo in the file must not become a grid: the database
    /// is not a trusted source, and `Bpm::new` is where that is enforced.
    #[test]
    fn a_tempo_outside_the_playable_range_yields_no_grid() {
        let mut stored = analysis();
        stored.bpm = Some(5_000.0);
        stored.grid_anchor = Some(0.0);
        stored.grid_beats_per_bar = Some(4);
        assert!(stored.beatgrid().is_none());
    }

    #[test]
    fn a_key_survives_the_round_trip() {
        let key = MusicalKey::new(8, Mode::Minor).unwrap();
        let stored = analysis().with_key(key, 0.8);
        assert_eq!(stored.key(), Some(key));
        assert_eq!(stored.key_confidence, Some(0.8));
    }

    #[test]
    fn a_track_with_no_tags_shows_its_filename() {
        let track = track(None);
        assert_eq!(track.display_title(), "01 - Untitled Demo");
        assert_eq!(track.display_artist(), "Unknown artist");
    }

    /// A blank tag is not a title. Some rippers write empty strings, and a row
    /// of blanks in a browser is worse than a row of filenames.
    #[test]
    fn a_blank_title_falls_back_like_a_missing_one() {
        assert_eq!(track(Some("   ")).display_title(), "01 - Untitled Demo");
    }

    #[test]
    fn a_real_title_is_used() {
        assert_eq!(track(Some("Bachata Rosa")).display_title(), "Bachata Rosa");
    }

    fn track(title: Option<&str>) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([0; 32]),
            path: PathBuf::from("/music/01 - Untitled Demo.flac"),
            tags: Tags {
                title: title.map(str::to_owned),
                ..Tags::default()
            },
            duration_frames: 48_000 * 200,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: analysis(),
            stats: PlayStats::default(),
        }
    }
}

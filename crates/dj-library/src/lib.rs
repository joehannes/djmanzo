//! The track database.
//!
//! Everything a DJ builds over years that is not the audio itself: which
//! tracks exist, what the analyser found, where the cues are, which playlists
//! they belong to, and what was played when.
//!
//! # Identity is the audio, not the file
//!
//! A track is keyed by the hash of its *decoded samples*
//! ([`dj_core::TrackId`], produced in `dj-decode`). That is the decision the
//! rest of the design falls out of:
//!
//! - moving or renaming a file keeps its cues, its grid and its play history;
//! - the same recording as FLAC and as an MP3 made from that FLAC are two
//!   rows, correctly — they are different audio, and a cue placed on one is
//!   a few milliseconds out on the other;
//! - two copies of the same file in different folders are one row, correctly.
//!
//! # Nothing here runs on the audio thread
//!
//! The library is read by a browser and written by a scan. It allocates, it
//! does I/O, it takes locks. The engine never touches it: what reaches a deck
//! is a decoded buffer and a `Beatgrid`, handed over through the command
//! queue like everything else.

pub mod filter;
pub mod import;
pub mod playlist;
pub mod record;
pub mod scan;
pub mod schema;
pub mod store;
pub mod tags;

pub use playlist::{PlayRecord, Playlist, PlaylistKind};
pub use record::{
    GridSource, LibraryTrack, PlayStats, StoredAnalysis, StoredCue, StoredLoop, Tags,
};
pub use scan::{ScanReport, ScannedFile, scan_all, scan_folder};
pub use store::{Library, LibraryError};

//! Bringing an existing collection in.
//!
//! # What an importer produces
//!
//! A [`Collection`]: tracks named by path, each carrying whatever cues, loops
//! and grid the source knew, plus a playlist tree. Deliberately *not* library
//! rows — an importer reads a file format and nothing else, and every format
//! disagrees about almost everything. Turning a `Collection` into library rows
//! happens once, in [`crate::Library::import`], so there is one place where the
//! decisions about identity and staging live.
//!
//! # What every importer has to get right
//!
//! - **Times in seconds.** rekordbox stores seconds, Traktor stores seconds,
//!   Serato stores milliseconds, iTunes stores nothing. Converting at the edge
//!   means the rest of the code never has to ask.
//! - **Paths as the DJ's machine sees them.** Every format has its own idea of
//!   how to spell a path — file URLs, volume-relative fragments, Windows
//!   backslashes. Each importer un-spells its own; nothing downstream should
//!   have to know which format a path came from.
//! - **Missing is missing.** A format that does not record a key must produce
//!   `None`, not a guess. The library already distinguishes "not analysed" from
//!   "analysed and inconclusive", and an import must not collapse them.

pub mod itunes;
pub mod rekordbox;
pub mod traktor;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Everything one importer read out of one file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Collection {
    pub tracks: Vec<ImportedTrack>,
    /// Flat, with parent indices into this same list — the same shape the
    /// library stores, so nothing has to be flattened twice.
    pub playlists: Vec<ImportedPlaylist>,
    /// Entries the importer could not make sense of, with a reason. Reported
    /// rather than dropped: a DJ whose import is missing forty tracks deserves
    /// to know that rather than to count them.
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub what: String,
    pub reason: &'static str,
}

/// One track, as the source described it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportedTrack {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub comment: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<u32>,
    /// 0..=5. Every format scales its stars differently; each importer converts.
    pub rating: Option<u8>,
    pub payload: ImportPayload,
}

/// The performance data an import carries until the file is identified.
///
/// Serialised as JSON into `pending_files.import_payload` — see the schema for
/// why it is staged rather than written straight in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportPayload {
    /// Seconds from the start of the track.
    pub cues: Vec<ImportedCue>,
    pub loops: Vec<ImportedLoop>,
    pub bpm: Option<f64>,
    /// Seconds. The first beat the source knew about.
    pub grid_anchor_seconds: Option<f64>,
    /// Camelot hour and mode, when the source recorded a key we understood.
    pub key_hour: Option<u8>,
    pub key_minor: Option<bool>,
}

impl ImportPayload {
    /// Whether there is anything worth staging.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
            && self.loops.is_empty()
            && self.bpm.is_none()
            && self.key_hour.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedCue {
    /// 1-based, as every controller numbers them.
    pub slot: u8,
    pub seconds: f64,
    pub label: Option<String>,
    /// `#rrggbb`, when the source had one.
    pub colour: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedLoop {
    pub slot: u8,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub label: Option<String>,
}

/// A node in the imported tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPlaylist {
    pub name: String,
    /// Index into [`Collection::playlists`], or `None` at the top level.
    pub parent: Option<usize>,
    pub is_folder: bool,
    /// Paths, in the order the DJ had them.
    pub paths: Vec<PathBuf>,
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// Tracks the source described.
    pub tracks: usize,
    /// Of those, already in the library and updated in place.
    pub already_known: usize,
    /// Of those, queued for identification.
    pub queued: usize,
    pub playlists: usize,
    pub folders: usize,
    /// Entries the importer could not read, with reasons.
    pub skipped: Vec<String>,
}

/// Which format a file is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    RekordboxXml,
    TraktorNml,
    ItunesXml,
}

impl Format {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RekordboxXml => "rekordbox XML",
            Self::TraktorNml => "Traktor NML",
            Self::ItunesXml => "iTunes XML",
        }
    }
}

/// Guess a format from what the file actually contains.
///
/// By content rather than by extension: rekordbox and iTunes both export
/// `.xml`, and a DJ who renamed the file should still get their library. The
/// first few hundred bytes are enough — every one of these formats announces
/// itself in its root element.
#[must_use]
pub fn sniff(contents: &str) -> Option<Format> {
    let head: String = contents.chars().take(2048).collect();
    if head.contains("<DJ_PLAYLISTS") {
        Some(Format::RekordboxXml)
    } else if head.contains("<NML") {
        Some(Format::TraktorNml)
    } else if head.contains("plist") && head.contains("Tracks") {
        Some(Format::ItunesXml)
    } else {
        None
    }
}

/// Read a collection, choosing the importer by content.
pub fn read(contents: &str) -> Result<(Format, Collection), ImportError> {
    let format = sniff(contents).ok_or(ImportError::UnknownFormat)?;
    let collection = match format {
        Format::RekordboxXml => rekordbox::read(contents)?,
        Format::TraktorNml => traktor::read(contents)?,
        Format::ItunesXml => itunes::read(contents)?,
    };
    Ok((format, collection))
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("this is not a library export djmanzo recognises")]
    UnknownFormat,
    #[error("the file is not valid XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("the file is {0}, but the part that holds the tracks is missing")]
    MissingTracks(&'static str),
}

/// Turn a file URL or a plain path into a path.
///
/// Shared because every one of these formats uses `file://` URLs somewhere, and
/// each spells them slightly differently. Percent-decoding is done here rather
/// than pulled in as a dependency: the only escapes that appear in a music
/// library are the ones a filename can contain, and the rule is three
/// characters wide.
#[must_use]
pub fn decode_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    // `file://localhost/Users/...` and `file:///Users/...` both occur.
    let without_scheme = trimmed
        .strip_prefix("file://localhost")
        .or_else(|| trimmed.strip_prefix("file://"))
        .unwrap_or(trimmed);

    let mut out = String::with_capacity(without_scheme.len());
    let mut chars = without_scheme.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let hex: String = chars.clone().take(2).collect();
        match u8::from_str_radix(&hex, 16) {
            Ok(byte) => {
                out.push(byte as char);
                chars.next();
                chars.next();
            }
            // A stray `%` in a filename is a `%`, not a broken escape.
            Err(_) => out.push('%'),
        }
    }

    // A Windows path exported as `/C:/Music/...` is `C:/Music/...`.
    let cleaned = if out.len() > 2
        && out.starts_with('/')
        && out.as_bytes()[2] == b':'
        && out.as_bytes()[1].is_ascii_alphabetic()
    {
        &out[1..]
    } else {
        &out[..]
    };
    PathBuf::from(cleaned)
}

/// Turn a musical key as a *word* into the Camelot pair the library stores.
///
/// Every format writes keys differently and several write them wrongly. What
/// they agree on is the note name and some marker of minor, so that is what is
/// read: `Am`, `A min`, `Amin`, `A minor` and `8A` all reach the same place,
/// and anything else is `None` rather than a guess.
#[must_use]
pub fn parse_key(raw: &str) -> Option<(u8, bool)> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }

    // Camelot already: `8A`, `11B`.
    let upper = text.to_ascii_uppercase();
    if let Some(hour) = camelot_hour(&upper) {
        return Some(hour);
    }

    // Otherwise a note name plus an optional minor marker.
    let lower = text.to_ascii_lowercase();
    // "min", "minor" and a bare trailing "m" all mean minor. Nothing else does:
    // "Amaj" ends in 'j', so there is no major spelling to exclude.
    let minor = lower.contains("min") || lower.ends_with('m');

    let mut chars = text.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let mut note = String::from(letter);
    // One accidental, in either spelling. A trailing 'b' is always a flat
    // rather than a minor marker -- `Bb` is B flat, and B minor is `Bm`.
    match chars.next() {
        Some('#' | '♯') => note.push('#'),
        Some('b' | '♭') => note.push('b'),
        _ => {}
    }

    let semitone = semitone_of(&note)?;
    Some((camelot_hour_for(semitone, minor), minor))
}

fn camelot_hour(upper: &str) -> Option<(u8, bool)> {
    let (digits, ring) = upper.split_at(upper.len().checked_sub(1)?);
    let hour: u8 = digits.parse().ok()?;
    if !(1..=12).contains(&hour) {
        return None;
    }
    match ring {
        "A" => Some((hour, true)),
        "B" => Some((hour, false)),
        _ => None,
    }
}

/// Pitch class, 0 = C.
fn semitone_of(note: &str) -> Option<u8> {
    Some(match note {
        "C" => 0,
        "C#" | "Db" => 1,
        "D" => 2,
        "D#" | "Eb" => 3,
        "E" | "Fb" => 4,
        "F" | "E#" => 5,
        "F#" | "Gb" => 6,
        "G" => 7,
        "G#" | "Ab" => 8,
        "A" => 9,
        "A#" | "Bb" => 10,
        "B" | "Cb" => 11,
        _ => return None,
    })
}

/// The Camelot wheel, as a formula rather than a table.
///
/// The wheel is the circle of fifths: going up an hour is going up a fifth
/// (seven semitones). 8A is A minor and 8B is C major, which fixes the offset
/// for each ring.
fn camelot_hour_for(semitone: u8, minor: bool) -> u8 {
    // Position on the circle of fifths, C = 0.
    let fifths = (i32::from(semitone) * 7).rem_euclid(12);
    let base = if minor {
        // A minor is 8A, and A is nine semitones above C.
        let a_minor: i32 = (9 * 7) % 12;
        (fifths - a_minor + 8 + 12 * 12).rem_euclid(12)
    } else {
        // C major is 8B.
        (fifths + 8).rem_euclid(12)
    };
    u8::try_from(if base == 0 { 12 } else { base }).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_url_becomes_a_path() {
        assert_eq!(
            decode_path("file://localhost/Users/dj/Music/track.flac"),
            PathBuf::from("/Users/dj/Music/track.flac")
        );
        assert_eq!(
            decode_path("file:///home/dj/track.flac"),
            PathBuf::from("/home/dj/track.flac")
        );
        assert_eq!(
            decode_path("/home/dj/track.flac"),
            PathBuf::from("/home/dj/track.flac")
        );
    }

    #[test]
    fn percent_escapes_are_decoded() {
        assert_eq!(
            decode_path("file:///music/Bachata%20Rosa.flac"),
            PathBuf::from("/music/Bachata Rosa.flac")
        );
        assert_eq!(
            decode_path("file:///music/AC%2FDC/track.flac"),
            PathBuf::from("/music/AC/DC/track.flac")
        );
    }

    /// A percent sign in a filename is a percent sign.
    #[test]
    fn a_stray_percent_is_left_alone() {
        assert_eq!(
            decode_path("/music/100% Pure.flac"),
            PathBuf::from("/music/100% Pure.flac")
        );
    }

    #[test]
    fn a_windows_drive_letter_loses_its_leading_slash() {
        assert_eq!(
            decode_path("file:///C:/Music/track.flac"),
            PathBuf::from("C:/Music/track.flac")
        );
    }

    // -- keys --------------------------------------------------------------

    #[test]
    fn camelot_notation_passes_straight_through() {
        assert_eq!(parse_key("8A"), Some((8, true)));
        assert_eq!(parse_key("11B"), Some((11, false)));
        assert_eq!(parse_key("12b"), Some((12, false)));
    }

    /// The anchors of the wheel, which everything else is derived from.
    #[test]
    fn the_wheel_is_anchored_correctly() {
        assert_eq!(parse_key("Am"), Some((8, true)), "A minor is 8A");
        assert_eq!(parse_key("C"), Some((8, false)), "C major is 8B");
    }

    #[test]
    fn note_names_map_onto_the_wheel() {
        // Going up a fifth goes up an hour.
        assert_eq!(parse_key("G"), Some((9, false)));
        assert_eq!(parse_key("D"), Some((10, false)));
        assert_eq!(parse_key("F"), Some((7, false)));
        // Relative minors share the hour.
        assert_eq!(parse_key("Em"), Some((9, true)));
        assert_eq!(parse_key("Dm"), Some((7, true)));
    }

    #[test]
    fn the_ways_minor_is_spelled_all_work() {
        for spelling in ["Am", "A min", "Amin", "A minor", "AMinor"] {
            assert_eq!(parse_key(spelling), Some((8, true)), "{spelling}");
        }
    }

    #[test]
    fn accidentals_work_in_both_spellings() {
        assert_eq!(parse_key("F#"), parse_key("Gb"));
        assert_eq!(parse_key("Bb"), parse_key("A#"));
    }

    /// The pair that trips a naive reading: a trailing `b` is a flat, a
    /// trailing `m` is minor, and `Bbm` is both.
    #[test]
    fn a_flat_is_not_mistaken_for_a_minor_marker() {
        assert_eq!(parse_key("Bb"), parse_key("A#"), "B flat major");
        assert_ne!(parse_key("Bb"), parse_key("Bm"), "B flat is not B minor");
        assert_eq!(parse_key("Bm").map(|(_, minor)| minor), Some(true));
        assert_eq!(parse_key("Bb").map(|(_, minor)| minor), Some(false));
        // B flat minor: flat and minor at once.
        assert_eq!(parse_key("Bbm"), parse_key("A#m"));
        assert_eq!(parse_key("Bbm").map(|(_, minor)| minor), Some(true));
    }

    /// A key nobody wrote, or wrote as something we do not understand, must be
    /// absent rather than guessed.
    #[test]
    fn an_unreadable_key_is_absent() {
        assert_eq!(parse_key(""), None);
        assert_eq!(parse_key("   "), None);
        assert_eq!(parse_key("Hmm"), None);
        assert_eq!(parse_key("13A"), None);
        assert_eq!(parse_key("0A"), None);
    }

    // -- sniffing ----------------------------------------------------------

    #[test]
    fn formats_are_told_apart_by_content_not_extension() {
        assert_eq!(
            sniff(r#"<?xml version="1.0"?><DJ_PLAYLISTS Version="1.0.0">"#),
            Some(Format::RekordboxXml)
        );
        assert_eq!(
            sniff(r#"<?xml version="1.0"?><NML VERSION="19">"#),
            Some(Format::TraktorNml)
        );
        assert_eq!(
            sniff(r#"<plist version="1.0"><dict><key>Tracks</key>"#),
            Some(Format::ItunesXml)
        );
        assert_eq!(sniff("just some text"), None);
    }
}

//! Hot cues and loops that live inside the audio file.
//!
//! # Why this is not in an importer
//!
//! Serato does not keep cues in its library — it writes them into the track
//! itself, as an ID3 `GEOB` frame (or a Vorbis comment, or an MP4 atom,
//! depending on the container). So they are not something a DJ imports; they
//! are something a file *has*, and the moment to read them is when the file is
//! first decoded. A DJ who never exported anything, and who copies their music
//! onto a new machine, still arrives with their cues.
//!
//! # Licensing
//!
//! Written from the format's published structure. `triseratops` is
//! AGPL-3.0-or-later and is therefore out of bounds under
//! [ADR-0002](../../../../docs/adr/0002-clean-room-permissive-licensing.md);
//! nothing here is derived from it. The structure is short:
//!
//! ```text
//! payload  := 0x01 0x01, then base64 of:
//!               0x01 0x01, then entries
//! entry    := name (NUL-terminated ASCII), length (u32 BE), body
//! ```
//!
//! and the bodies this reads are:
//!
//! ```text
//! CUE   := _ index(u8) position_ms(u32 BE) _ r(u8) g(u8) b(u8) _ _ name(NUL)
//! LOOP  := _ index(u8) start_ms(u32 BE) end_ms(u32 BE) ... name(NUL)
//! ```
//!
//! Bytes written `_` are ones the format reserves and this does not read. They
//! are skipped by offset rather than by name, because guessing at a meaning we
//! do not know would be worse than admitting we do not know it.
//!
//! # What this does not do
//!
//! It reads. Nothing here writes markers back into anybody's files: a DJ's
//! music is theirs, and a library that silently rewrites the tags of every
//! track it touches is one bad release away from a disaster.

use super::{ImportedCue, ImportedLoop};
use base64::Engine;

/// The `GEOB` descriptor Serato writes its cues under.
const SERATO_MARKERS2: &str = "Serato Markers2";

/// ...and the Vorbis comment field for the same data in a FLAC or Ogg.
const SERATO_MARKERS2_VORBIS: &str = "SERATO_MARKERS_V2";

/// Refuse an entry claiming to be longer than any real marker block.
///
/// A length field is four bytes of file asking for an allocation. Serato's
/// entries are tens of bytes; a megabyte is far past anything real.
const MAX_ENTRY: u32 = 1024 * 1024;

/// What was found in a file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Markers {
    pub cues: Vec<ImportedCue>,
    pub loops: Vec<ImportedLoop>,
}

impl Markers {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cues.is_empty() && self.loops.is_empty()
    }
}

/// Read whatever markers a file carries.
///
/// Never an error. A file with no markers is the common case, an unreadable
/// one is a file we simply learn nothing extra about, and neither is worth
/// failing an import over.
///
/// # Why this opens the file per container instead of once, generically
///
/// `GEOB` is an ID3v2 frame that lofty deliberately does not fold into its
/// container-agnostic tag — it stays a raw binary frame on the concrete
/// `Id3v2Tag`. So reaching it means knowing which kind of file this is. The
/// three containers below are the ones Serato writes ID3 into; FLAC and Ogg
/// keep the same payload in a Vorbis comment, which *is* a plain string and
/// comes out of the generic tag.
#[must_use]
pub fn read_file(path: &std::path::Path) -> Markers {
    use lofty::file::AudioFile;

    let Ok(mut file) = std::fs::File::open(path) else {
        return Markers::default();
    };
    let options = lofty::config::ParseOptions::new();

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let id3 = match extension.as_str() {
        "mp3" | "mp2" | "aac" => lofty::mpeg::MpegFile::read_from(&mut file, options)
            .ok()
            .and_then(|f| f.id3v2().cloned()),
        "aif" | "aiff" | "aifc" => lofty::iff::aiff::AiffFile::read_from(&mut file, options)
            .ok()
            .and_then(|f| f.id3v2().cloned()),
        "wav" | "wave" => lofty::iff::wav::WavFile::read_from(&mut file, options)
            .ok()
            .and_then(|f| f.id3v2().cloned()),
        _ => None,
    };
    if let Some(tag) = id3 {
        let markers = from_id3(&tag);
        if !markers.is_empty() {
            return markers;
        }
    }

    // FLAC, Ogg and anything else: the same base64 as a plain tag value.
    from_vorbis(path)
}

/// The Vorbis-comment spelling of the same payload.
fn from_vorbis(path: &std::path::Path) -> Markers {
    use lofty::file::TaggedFileExt;
    use lofty::prelude::ItemKey;

    let Ok(tagged) = lofty::probe::Probe::open(path).and_then(lofty::probe::Probe::read) else {
        return Markers::default();
    };
    for tag in tagged.tags() {
        if let Some(text) = tag.get_string(&ItemKey::Unknown(SERATO_MARKERS2_VORBIS.to_owned())) {
            let markers = parse_payload(text.as_bytes());
            if !markers.is_empty() {
                return markers;
            }
        }
    }
    Markers::default()
}

/// Read markers out of an ID3v2 tag's `GEOB` frames.
#[must_use]
pub fn from_id3(tag: &lofty::id3::v2::Id3v2Tag) -> Markers {
    use lofty::id3::v2::{Frame, GeneralEncapsulatedObject};

    for frame in tag {
        let Frame::Binary(binary) = frame else {
            continue;
        };
        if binary.id().as_str() != "GEOB" {
            continue;
        }
        let Ok(object) =
            GeneralEncapsulatedObject::parse(&binary.data, lofty::id3::v2::FrameFlags::default())
        else {
            continue;
        };
        if object.descriptor.as_deref() != Some(SERATO_MARKERS2) {
            continue;
        }
        let markers = parse_payload(&object.data);
        if !markers.is_empty() {
            return markers;
        }
    }
    Markers::default()
}

/// Decode the two-byte header, the base64, and the entries inside it.
#[must_use]
pub fn parse_payload(payload: &[u8]) -> Markers {
    // Two version bytes, then base64. Some writers omit the header, so a
    // payload that does not start with it is tried as base64 directly rather
    // than refused.
    let body = if payload.len() > 2 && payload[0] == 0x01 && payload[1] == 0x01 {
        &payload[2..]
    } else {
        payload
    };

    // Serato line-wraps its base64 and pads inconsistently between versions, so
    // the whitespace goes and the padding is made optional.
    let cleaned: Vec<u8> = body
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace() && *b != 0)
        .collect();
    let engine = base64::engine::general_purpose::STANDARD_NO_PAD;
    let stripped: Vec<u8> = cleaned.iter().copied().take_while(|b| *b != b'=').collect();
    let Ok(decoded) = engine.decode(&stripped) else {
        return Markers::default();
    };

    parse_entries(&decoded)
}

/// Walk the decoded entries.
#[must_use]
pub fn parse_entries(data: &[u8]) -> Markers {
    let mut markers = Markers::default();
    // Two more version bytes inside the base64.
    let mut at = if data.len() > 2 && data[0] == 0x01 && data[1] == 0x01 {
        2
    } else {
        0
    };

    while at < data.len() {
        // A NUL-terminated name, then a length, then that many bytes.
        let Some(end) = data[at..].iter().position(|b| *b == 0) else {
            break;
        };
        let Ok(name) = std::str::from_utf8(&data[at..at + end]) else {
            break;
        };
        // The block ends with a run of padding zeroes rather than a terminator.
        if name.is_empty() {
            break;
        }
        let after_name = at + end + 1;
        if after_name + 4 > data.len() {
            break;
        }
        let length = u32::from_be_bytes([
            data[after_name],
            data[after_name + 1],
            data[after_name + 2],
            data[after_name + 3],
        ]);
        if length > MAX_ENTRY {
            break;
        }
        let start = after_name + 4;
        let Some(stop) = start.checked_add(length as usize) else {
            break;
        };
        if stop > data.len() {
            break;
        }

        match name {
            "CUE" => {
                if let Some(cue) = parse_cue(&data[start..stop]) {
                    markers.cues.push(cue);
                }
            }
            "LOOP" => {
                if let Some(region) = parse_loop(&data[start..stop]) {
                    markers.loops.push(region);
                }
            }
            // COLOR, BPMLOCK and FLIP are real entries this does not use.
            _ => {}
        }
        at = stop;
    }

    markers.cues.sort_by_key(|cue| cue.slot);
    markers.loops.sort_by_key(|region| region.slot);
    markers
}

/// `_ index position_ms _ r g b _ _ name`
fn parse_cue(body: &[u8]) -> Option<ImportedCue> {
    if body.len() < 12 {
        return None;
    }
    let index = body.get(1).copied()?;
    let position = u32::from_be_bytes([body[2], body[3], body[4], body[5]]);
    let (r, g, b) = (*body.get(7)?, *body.get(8)?, *body.get(9)?);

    Some(ImportedCue {
        // Serato numbers pads from zero; we number them from one.
        slot: index.checked_add(1)?,
        seconds: f64::from(position) / 1000.0,
        label: trailing_name(&body[12..]),
        colour: Some(format!("#{r:02x}{g:02x}{b:02x}")),
    })
}

/// `_ index start_ms end_ms ... name`
fn parse_loop(body: &[u8]) -> Option<ImportedLoop> {
    if body.len() < 21 {
        return None;
    }
    let index = body.get(1).copied()?;
    let start = u32::from_be_bytes([body[2], body[3], body[4], body[5]]);
    let end = u32::from_be_bytes([body[6], body[7], body[8], body[9]]);
    if end <= start {
        return None;
    }

    Some(ImportedLoop {
        slot: index.checked_add(1)?,
        start_seconds: f64::from(start) / 1000.0,
        end_seconds: f64::from(end) / 1000.0,
        label: trailing_name(&body[21..]),
    })
}

/// The NUL-terminated name at the end of an entry.
fn trailing_name(tail: &[u8]) -> Option<String> {
    let end = tail.iter().position(|b| *b == 0).unwrap_or(tail.len());
    let text = std::str::from_utf8(&tail[..end]).ok()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an entry, so the tests read the way the format is described.
    fn entry(name: &str, body: &[u8]) -> Vec<u8> {
        let mut out = name.as_bytes().to_vec();
        out.push(0);
        out.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
        out.extend_from_slice(body);
        out
    }

    fn cue_body(index: u8, position_ms: u32, colour: [u8; 3], name: &str) -> Vec<u8> {
        let mut out = vec![0];
        out.push(index);
        out.extend_from_slice(&position_ms.to_be_bytes());
        out.push(0);
        out.extend_from_slice(&colour);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out
    }

    fn loop_body(index: u8, start_ms: u32, end_ms: u32, name: &str) -> Vec<u8> {
        let mut out = vec![0];
        out.push(index);
        out.extend_from_slice(&start_ms.to_be_bytes());
        out.extend_from_slice(&end_ms.to_be_bytes());
        out.extend_from_slice(&[0xff; 8]);
        out.extend_from_slice(&[0, 0, 0]);
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        out
    }

    fn payload(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut inner = vec![0x01, 0x01];
        for entry in entries {
            inner.extend_from_slice(entry);
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(&inner);
        let mut out = vec![0x01, 0x01];
        out.extend_from_slice(encoded.as_bytes());
        out
    }

    #[test]
    fn a_cue_is_read_with_its_position_colour_and_name() {
        let data = payload(&[entry("CUE", &cue_body(0, 32_500, [255, 0, 0], "drop"))]);
        let markers = parse_payload(&data);

        assert_eq!(markers.cues.len(), 1);
        let cue = &markers.cues[0];
        assert_eq!(cue.slot, 1, "Serato counts pads from zero");
        assert_eq!(cue.seconds, 32.5, "positions are milliseconds");
        assert_eq!(cue.colour.as_deref(), Some("#ff0000"));
        assert_eq!(cue.label.as_deref(), Some("drop"));
    }

    #[test]
    fn a_loop_is_read_with_both_ends() {
        let data = payload(&[entry("LOOP", &loop_body(1, 64_000, 80_000, "the eight"))]);
        let markers = parse_payload(&data);

        assert_eq!(markers.loops.len(), 1);
        let region = &markers.loops[0];
        assert_eq!(region.slot, 2);
        assert_eq!(region.start_seconds, 64.0);
        assert_eq!(region.end_seconds, 80.0);
        assert_eq!(region.label.as_deref(), Some("the eight"));
    }

    #[test]
    fn cues_and_loops_come_back_in_slot_order() {
        let data = payload(&[
            entry("CUE", &cue_body(2, 3000, [0, 0, 255], "third")),
            entry("CUE", &cue_body(0, 1000, [255, 0, 0], "first")),
            entry("LOOP", &loop_body(1, 5000, 6000, "b")),
            entry("LOOP", &loop_body(0, 7000, 8000, "a")),
        ]);
        let markers = parse_payload(&data);

        assert_eq!(
            markers.cues.iter().map(|c| c.slot).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(
            markers.loops.iter().map(|l| l.slot).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    /// Entries this does not understand must be stepped over, not stumbled on.
    #[test]
    fn unknown_entries_are_skipped_without_losing_what_follows() {
        let data = payload(&[
            entry("COLOR", &[0, 1, 2, 3]),
            entry("BPMLOCK", &[1]),
            entry("CUE", &cue_body(0, 1000, [255, 0, 0], "after")),
        ]);
        let markers = parse_payload(&data);
        assert_eq!(markers.cues.len(), 1);
        assert_eq!(markers.cues[0].label.as_deref(), Some("after"));
    }

    /// Serato line-wraps its base64 and pads inconsistently between versions.
    #[test]
    fn wrapped_and_unpadded_base64_both_decode() {
        let plain = payload(&[entry("CUE", &cue_body(0, 1000, [1, 2, 3], "x"))]);

        let mut wrapped = plain.clone();
        // A newline every few characters, as Serato writes it.
        let text = String::from_utf8(wrapped.split_off(2)).unwrap();
        let mut rewrapped = vec![0x01, 0x01];
        for (index, byte) in text.bytes().enumerate() {
            if index > 0 && index % 72 == 0 {
                rewrapped.push(b'\n');
            }
            rewrapped.push(byte);
        }

        assert_eq!(parse_payload(&plain), parse_payload(&rewrapped));
        assert_eq!(parse_payload(&plain).cues.len(), 1);
    }

    #[test]
    fn a_payload_with_no_version_header_still_decodes() {
        let plain = payload(&[entry("CUE", &cue_body(0, 1000, [1, 2, 3], "x"))]);
        let headerless = &plain[2..];
        assert_eq!(parse_payload(headerless).cues.len(), 1);
    }

    // -- what must not happen ----------------------------------------------

    /// A length field is four bytes of somebody's file asking for an
    /// allocation.
    #[test]
    fn an_impossible_entry_length_stops_the_read() {
        let mut inner = vec![0x01, 0x01];
        inner.extend_from_slice(b"CUE\0");
        inner.extend_from_slice(&u32::MAX.to_be_bytes());
        let encoded = base64::engine::general_purpose::STANDARD.encode(&inner);
        let mut data = vec![0x01, 0x01];
        data.extend_from_slice(encoded.as_bytes());

        assert!(parse_payload(&data).is_empty());
    }

    #[test]
    fn a_truncated_entry_is_dropped_rather_than_read_past() {
        let mut inner = vec![0x01, 0x01];
        inner.extend_from_slice(&entry("CUE", &cue_body(0, 1000, [1, 2, 3], "x")));
        // Half of a second entry.
        inner.extend_from_slice(b"LOOP\0\0\0");
        let encoded = base64::engine::general_purpose::STANDARD.encode(&inner);
        let mut data = vec![0x01, 0x01];
        data.extend_from_slice(encoded.as_bytes());

        let markers = parse_payload(&data);
        assert_eq!(markers.cues.len(), 1, "the whole entry before it survives");
        assert!(markers.loops.is_empty());
    }

    #[test]
    fn nonsense_is_empty_rather_than_an_error() {
        assert!(parse_payload(b"").is_empty());
        assert!(parse_payload(b"\x01\x01").is_empty());
        assert!(parse_payload(b"not base64 at all !!!").is_empty());
        assert!(parse_payload(&[0xff; 64]).is_empty());
    }

    /// A loop whose end is not after its start is not a loop.
    #[test]
    fn a_reversed_loop_is_refused() {
        let data = payload(&[entry("LOOP", &loop_body(0, 8000, 4000, "backwards"))]);
        assert!(parse_payload(&data).loops.is_empty());
    }

    #[test]
    fn an_entry_with_no_name_is_still_a_cue() {
        let data = payload(&[entry("CUE", &cue_body(0, 1000, [1, 2, 3], ""))]);
        let markers = parse_payload(&data);
        assert_eq!(markers.cues.len(), 1);
        assert_eq!(markers.cues[0].label, None);
    }

    #[test]
    fn reading_a_file_that_does_not_exist_is_empty_not_an_error() {
        assert!(read_file(std::path::Path::new("/nowhere/track.mp3")).is_empty());
    }

    #[test]
    fn a_file_with_no_markers_is_empty_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track.mp3");
        std::fs::write(&path, b"not really an mp3").unwrap();
        assert!(read_file(&path).is_empty());
    }

    /// The whole path, through a real file: write a GEOB frame into a WAV the
    /// way Serato does, then read the cues back off disk.
    ///
    /// This is the test that catches a mistake in *reaching* the frame rather
    /// than in decoding it — the parsing tests above all start from bytes
    /// already in hand, and getting to those bytes is half the work.
    #[test]
    fn markers_survive_a_round_trip_through_a_real_file() {
        use lofty::TextEncoding;
        use lofty::config::WriteOptions;
        use lofty::id3::v2::{BinaryFrame, Frame, FrameId, GeneralEncapsulatedObject, Id3v2Tag};
        use lofty::prelude::TagExt;
        use std::borrow::Cow;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("track.wav");
        write_silent_wav(&path);

        let data = payload(&[
            entry("CUE", &cue_body(0, 32_500, [255, 0, 0], "drop")),
            entry("LOOP", &loop_body(0, 64_000, 80_000, "the eight")),
        ]);
        let object = GeneralEncapsulatedObject::new(
            TextEncoding::UTF8,
            Some("application/octet-stream".to_owned()),
            Some(String::new()),
            Some(SERATO_MARKERS2.to_owned()),
            data,
        );
        // GEOB has no variant of its own: lofty stores it as a binary frame
        // whose payload is the serialised object, which is exactly how the
        // reader finds it again.
        let mut tag = Id3v2Tag::default();
        tag.insert(Frame::Binary(BinaryFrame::new(
            FrameId::Valid(Cow::Borrowed("GEOB")),
            object.as_bytes(),
        )));
        tag.save_to_path(&path, WriteOptions::default()).unwrap();

        let markers = read_file(&path);
        assert_eq!(markers.cues.len(), 1, "the cue must come back off disk");
        assert_eq!(markers.cues[0].seconds, 32.5);
        assert_eq!(markers.cues[0].label.as_deref(), Some("drop"));
        assert_eq!(markers.loops.len(), 1);
        assert_eq!(markers.loops[0].end_seconds, 80.0);
    }

    /// A second of silence, so there is a real container to tag.
    fn write_silent_wav(path: &std::path::Path) {
        const RATE: u32 = 44_100;
        let frames = RATE as usize;
        let data_len = (frames * 4) as u32;

        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&2u16.to_le_bytes()); // stereo
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend(std::iter::repeat_n(0u8, data_len as usize));
        std::fs::write(path, out).unwrap();
    }
}

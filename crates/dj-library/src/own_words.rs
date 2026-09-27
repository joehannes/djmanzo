//! The words a record carries itself: in its own tags, or in a `.lrc` beside it.
//!
//! [KARAOKE.md](../../../docs/KARAOKE.md) §2 lists four places words come from,
//! in order: **embedded in the file**, **a sidecar `.lrc`**, LRCLIB, and
//! transcription. The last two need a network or a model; these two are free,
//! offline and instant, and they are what a DJ who already runs karaoke has —
//! a collection with lyrics in its tags, or the `.lrc` files the karaoke world
//! passes around. So they are read first, and before anything is asked of the
//! lyrics database.
//!
//! # Timed first, then the order
//!
//! The order is the design's, with one refinement: **the first source with
//! timed words wins**, and only when neither has timings does the first with
//! plain words. A tag holding the words without times would otherwise hide a
//! `.lrc` beside the file that has them — and a singers' screen can wipe timed
//! words in time with the record, while untimed ones can only stand still.
//!
//! # What counts as timed
//!
//! - An ID3 **`SYLT`** frame of lyrics stamped in **milliseconds**. `SYLT` may
//!   also be stamped in MPEG frames, which needs the frame rate to turn into a
//!   time; such a frame is left out rather than guessed at.
//! - A lyrics tag (`USLT`, Vorbis `LYRICS`, MP4 `©lyr`) whose text **is** LRC —
//!   taggers commonly store a whole `.lrc` there. It counts as timed when
//!   [`crate::lrc::parse`] finds a timed line in it, and as plain words when
//!   not.
//! - A sidecar `.lrc` is timed or nothing: a line without a stamp is dropped by
//!   the parser, so a sidecar with no timed line says nothing.

use crate::lrc;
use std::path::{Path, PathBuf};

/// Where a record's own words were found.
pub const IN_TAGS: &str = "tags";
/// A `.lrc` beside the file.
pub const BESIDE: &str = "sidecar";

/// The largest sidecar read, in bytes.
///
/// A megabyte. The longest real `.lrc` is a few tens of kilobytes; a file
/// larger than this beside a record is not its lyrics, and reading it whole
/// into memory to find out would be the wrong way round.
const LARGEST_SIDECAR: u64 = 1024 * 1024;

/// A record's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnWords {
    /// The words without times, a line each.
    pub plain: String,
    /// The words as LRC, when they are timed.
    pub synced: Option<String>,
    /// [`IN_TAGS`] or [`BESIDE`].
    pub source: &'static str,
}

impl OwnWords {
    fn timed(synced: String, source: &'static str) -> Option<Self> {
        let lines = lrc::parse(&synced);
        if lines.is_empty() {
            return None;
        }
        let plain = lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Some(Self {
            plain,
            synced: Some(synced),
            source,
        })
    }

    fn untimed(plain: &str, source: &'static str) -> Option<Self> {
        let plain = plain.trim();
        (!plain.is_empty()).then(|| Self {
            plain: plain.to_owned(),
            synced: None,
            source,
        })
    }
}

/// The words the record at `path` carries itself, if it carries any.
///
/// `None` for a file with neither, and for one that cannot be opened: the
/// lyrics database and transcription are what cover those, and an error here
/// would stop them being asked.
#[must_use]
pub fn read(path: &Path) -> Option<OwnWords> {
    choose([in_tags(path), beside(path)])
}

/// The first with timed words, else the first with any — see the module docs.
fn choose<const N: usize>(found: [Option<OwnWords>; N]) -> Option<OwnWords> {
    let mut first = None;
    for words in found.into_iter().flatten() {
        if words.synced.is_some() {
            return Some(words);
        }
        first.get_or_insert(words);
    }
    first
}

/// The words in the file's own tags.
fn in_tags(path: &Path) -> Option<OwnWords> {
    let from_sylt = sylt(path).and_then(|synced| OwnWords::timed(synced, IN_TAGS));
    if from_sylt.is_some() {
        return from_sylt;
    }
    let text = lyrics_tag(path)?;
    OwnWords::timed(text.clone(), IN_TAGS).or_else(|| OwnWords::untimed(&text, IN_TAGS))
}

/// `USLT`, Vorbis `LYRICS` or MP4 `©lyr` — lofty folds all three onto one key.
fn lyrics_tag(path: &Path) -> Option<String> {
    use lofty::file::TaggedFileExt;
    use lofty::prelude::ItemKey;

    let tagged = lofty::probe::Probe::open(path).ok()?.read().ok()?;
    tagged
        .tags()
        .iter()
        .find_map(|tag| tag.get_string(&ItemKey::Lyrics).map(str::to_owned))
        .filter(|text| !text.trim().is_empty())
}

/// An MP3's `SYLT` lyrics frame, written out as LRC.
fn sylt(path: &Path) -> Option<String> {
    use lofty::config::ParseOptions;
    use lofty::file::AudioFile;
    use lofty::id3::v2::{
        Frame, FrameFlags, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
    };
    use lofty::mpeg::MpegFile;

    let is_mp3 = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    if !is_mp3 {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mpeg = MpegFile::read_from(&mut file, ParseOptions::new()).ok()?;
    let tag = mpeg.id3v2()?;
    tag.into_iter().find_map(|frame| {
        if frame.id_str() != "SYLT" {
            return None;
        }
        let Frame::Binary(binary) = frame else {
            return None;
        };
        let parsed = SynchronizedTextFrame::parse(&binary.data, FrameFlags::default()).ok()?;
        (parsed.timestamp_format == TimestampFormat::MS
            && matches!(
                parsed.content_type,
                SyncTextContentType::Lyrics | SyncTextContentType::TextTranscription
            ))
        .then(|| as_lrc(&parsed.content))
        .flatten()
    })
}

/// `SYLT`'s stamped fragments as LRC.
///
/// A `SYLT` frame is a list of `(milliseconds, text)`. Taggers write it two
/// ways: a line per entry, or a syllable or word per entry with a line break
/// at the start of each fragment that begins a new line. The second is read
/// as word timings (enhanced LRC), so a screen can wipe it word by word; the
/// first as one timed line per entry.
#[must_use]
pub fn as_lrc(content: &[(u32, String)]) -> Option<String> {
    let broken = content
        .iter()
        .skip(1)
        .any(|(_, text)| text.starts_with('\n') || text.starts_with('\r'));
    let mut out = String::new();
    if broken {
        let mut line: Vec<(u32, String)> = Vec::new();
        for (at, text) in content {
            if (text.starts_with('\n') || text.starts_with('\r')) && !line.is_empty() {
                push_worded(&mut out, &line);
                line.clear();
            }
            let fragment = text.trim_matches(|c| c == '\n' || c == '\r');
            if !fragment.trim().is_empty() {
                line.push((*at, fragment.to_owned()));
            }
        }
        push_worded(&mut out, &line);
    } else {
        for (at, text) in content {
            let text = text.trim();
            if !text.is_empty() {
                out.push_str(&format!("[{}]{text}\n", stamp(*at)));
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

fn push_worded(out: &mut String, line: &[(u32, String)]) {
    let Some((first, _)) = line.first() else {
        return;
    };
    out.push_str(&format!("[{}]", stamp(*first)));
    for (at, fragment) in line {
        out.push_str(&format!("<{}>{fragment}", stamp(*at)));
    }
    out.push('\n');
}

/// Milliseconds as LRC's `mm:ss.xx`.
fn stamp(ms: u32) -> String {
    let hundredths = ms / 10;
    format!(
        "{:02}:{:02}.{:02}",
        hundredths / 6000,
        (hundredths / 100) % 60,
        hundredths % 100
    )
}

/// The `.lrc` beside the file, if there is one with a timed line in it.
fn beside(path: &Path) -> Option<OwnWords> {
    let text = sidecars(path).into_iter().find_map(|candidate| {
        let meta = std::fs::metadata(&candidate).ok()?;
        if !meta.is_file() || meta.len() > LARGEST_SIDECAR {
            return None;
        }
        let bytes = std::fs::read(&candidate).ok()?;
        // Old karaoke files are often Latin-1: read what is UTF-8 as it is
        // and let the rest through lossily rather than refusing the file.
        let text = String::from_utf8_lossy(&bytes).into_owned();
        Some(text.trim_start_matches('\u{feff}').to_owned())
    })?;
    OwnWords::timed(text, BESIDE)
}

/// Where a sidecar may be: the record's name with `.lrc`, in either case.
fn sidecars(path: &Path) -> Vec<PathBuf> {
    vec![path.with_extension("lrc"), path.with_extension("LRC")]
}

#[cfg(test)]
mod tests {
    use super::*;

    const LRC: &str = "[00:01.00]first line\n[00:03.50]second line\n";

    fn words(synced: bool, source: &'static str) -> OwnWords {
        OwnWords {
            plain: "a line".to_owned(),
            synced: synced.then(|| "[00:01.00]a line".to_owned()),
            source,
        }
    }

    /// **The first with timed words wins; plain words only when neither is
    /// timed.** Untimed words in a tag must not hide a timed `.lrc` beside
    /// the file, and timed words in the tag come before the sidecar's.
    #[test]
    fn timed_words_win_and_the_design_order_breaks_ties() {
        assert_eq!(
            choose([Some(words(false, IN_TAGS)), Some(words(true, BESIDE))]).map(|w| w.source),
            Some(BESIDE)
        );
        assert_eq!(
            choose([Some(words(true, IN_TAGS)), Some(words(true, BESIDE))]).map(|w| w.source),
            Some(IN_TAGS)
        );
        assert_eq!(
            choose([Some(words(false, IN_TAGS)), None]).map(|w| w.source),
            Some(IN_TAGS)
        );
        assert_eq!(choose::<2>([None, None]), None);
    }

    /// **A `.lrc` beside the record is read, in either case, and must be
    /// timed.** A sidecar of plain text says nothing, and one far too large
    /// to be lyrics is not read.
    #[test]
    fn a_sidecar_beside_the_record_is_read_when_it_is_timed() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("song.mp3");
        std::fs::write(&record, b"not really audio").unwrap();
        assert_eq!(read(&record), None);

        std::fs::write(dir.path().join("song.lrc"), format!("\u{feff}{LRC}")).unwrap();
        let found = read(&record).expect("the sidecar");
        assert_eq!(found.source, BESIDE);
        assert_eq!(found.plain, "first line\nsecond line");
        assert_eq!(lrc::parse(found.synced.as_deref().unwrap()).len(), 2);

        std::fs::write(dir.path().join("song.lrc"), "just words\nno times\n").unwrap();
        assert_eq!(read(&record), None, "an untimed sidecar was taken as timed");

        std::fs::remove_file(dir.path().join("song.lrc")).unwrap();
        let other = dir.path().join("other.flac");
        std::fs::write(&other, b"x").unwrap();
        std::fs::write(dir.path().join("other.LRC"), LRC).unwrap();
        assert_eq!(read(&other).map(|w| w.source), Some(BESIDE));

        std::fs::write(
            dir.path().join("other.LRC"),
            "[00:01.00]x\n".repeat(200_000),
        )
        .unwrap();
        assert_eq!(read(&other), None, "a sidecar past a megabyte was read");
    }

    /// **`SYLT` written a line per entry is a timed line each; written a
    /// fragment per entry with line breaks, it is word timings.**
    #[test]
    fn sylt_becomes_lrc_line_by_line_or_word_by_word() {
        let lines = as_lrc(&[
            (1000, "first line".to_owned()),
            (63_450, "second".to_owned()),
        ])
        .unwrap();
        assert_eq!(lines, "[00:01.00]first line\n[01:03.45]second\n");

        let worded = as_lrc(&[
            (1000, "so ".to_owned()),
            (1400, "long".to_owned()),
            (2000, "\nand ".to_owned()),
            (2300, "more".to_owned()),
        ])
        .unwrap();
        let parsed = lrc::parse(&worded);
        assert_eq!(parsed.len(), 2, "{worded}");
        assert_eq!(parsed[0].words.len(), 2, "{worded}");
        assert!((parsed[1].at - 2.0).abs() < 1e-9, "{worded}");
        assert!(parsed[1].text.starts_with("and"), "{worded}");

        assert_eq!(as_lrc(&[]), None);
    }

    /// A silent MP3 of `frames` MPEG-1 Layer III frames, for tags to sit on.
    fn silent_mp3(path: &Path) {
        // 128 kbit/s, 44.1 kHz, no padding: 417 bytes a frame.
        let mut bytes = Vec::new();
        for _ in 0..40 {
            let mut frame = vec![0u8; 417];
            frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            bytes.extend_from_slice(&frame);
        }
        std::fs::write(path, bytes).unwrap();
    }

    /// **The record's own tags: a `SYLT` frame is timed words, and a plain
    /// `USLT` is plain words — unless its text is LRC.**
    #[test]
    fn an_mp3_s_own_tags_give_its_words() {
        use lofty::TextEncoding;
        use lofty::config::WriteOptions;
        use lofty::id3::v2::{
            BinaryFrame, Frame, FrameId, Id3v2Tag, SyncTextContentType, SynchronizedTextFrame,
            TimestampFormat, UnsynchronizedTextFrame,
        };
        use lofty::prelude::TagExt;
        use std::borrow::Cow;

        let dir = tempfile::tempdir().unwrap();

        let plain = dir.path().join("plain.mp3");
        silent_mp3(&plain);
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            String::new(),
            "first line\nsecond line".to_owned(),
        )));
        tag.save_to_path(&plain, WriteOptions::default()).unwrap();
        let found = read(&plain).expect("the USLT words");
        assert_eq!(found.source, IN_TAGS);
        assert_eq!(found.synced, None);
        assert_eq!(found.plain, "first line\nsecond line");

        let lrc_in_uslt = dir.path().join("lrc.mp3");
        silent_mp3(&lrc_in_uslt);
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            String::new(),
            LRC.to_owned(),
        )));
        tag.save_to_path(&lrc_in_uslt, WriteOptions::default())
            .unwrap();
        assert!(
            read(&lrc_in_uslt)
                .expect("the LRC in USLT")
                .synced
                .is_some()
        );

        let synced = dir.path().join("sylt.mp3");
        silent_mp3(&synced);
        let sylt = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            TimestampFormat::MS,
            SyncTextContentType::Lyrics,
            None,
            vec![
                (1000, "first line".to_owned()),
                (3500, "second line".to_owned()),
            ],
        );
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::Binary(BinaryFrame::new(
            FrameId::Valid(Cow::Borrowed("SYLT")),
            sylt.as_bytes().unwrap(),
        )));
        tag.save_to_path(&synced, WriteOptions::default()).unwrap();
        let found = read(&synced).expect("the SYLT words");
        assert_eq!(found.source, IN_TAGS);
        let lines = lrc::parse(found.synced.as_deref().unwrap());
        assert_eq!(lines.len(), 2);
        assert!((lines[1].at - 3.5).abs() < 1e-9);

        // Stamped in MPEG frames, which needs the frame rate to become a
        // time: left out rather than read as milliseconds.
        let frames = dir.path().join("frames.mp3");
        silent_mp3(&frames);
        let sylt = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            TimestampFormat::MPEG,
            SyncTextContentType::Lyrics,
            None,
            vec![(38, "first line".to_owned())],
        );
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::Binary(BinaryFrame::new(
            FrameId::Valid(Cow::Borrowed("SYLT")),
            sylt.as_bytes().unwrap(),
        )));
        tag.save_to_path(&frames, WriteOptions::default()).unwrap();
        assert_eq!(
            read(&frames),
            None,
            "frame-stamped SYLT was read as milliseconds"
        );
    }
}

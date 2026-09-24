//! §108: a recorded mix's tracklist, as chapters.
//!
//! A DJ who records a set posts it — to YouTube, to Mixcloud — and both want
//! the tracklist *timed against the recording*: YouTube draws chapters from
//! lines in the description that start with a time, and Mixcloud asks for
//! each record's start. The night's history knows when each record came in,
//! on the wall clock; a recording is named for when it started (`set-<unix
//! seconds>.wav`, `crate::state::AppState::start_recording`). Subtract, and
//! the tracklist is timed against the file.
//!
//! YouTube's rules decide the shape (sources in `docs/RESEARCH.md`): the first
//! chapter is at `0:00`, every chapter is at least ten seconds long, and three
//! are needed before any are drawn. So:
//!
//! - the record already playing when recording started is the first chapter,
//!   at `0:00`, if it came in within [`LOOK_BACK`] of the start;
//! - a first record that came in within [`FIRST_WITHIN`] of the start is
//!   moved to `0:00`; one that came in later leaves `0:00` as *Start*, which
//!   is true, rather than naming a record that was not yet playing;
//! - a record that lasted less than [`SHORTEST`] before the next is not a
//!   chapter — it was a preview, not a part of the set.

use dj_library::PlayRecord;

/// How long before the recording a record may have come in and still be the
/// one playing when it started: ten minutes, longer than most records.
pub const LOOK_BACK: i64 = 600;

/// A first record this soon after the start is where the recording starts.
pub const FIRST_WITHIN: i64 = 30;

/// YouTube's shortest chapter.
pub const SHORTEST: i64 = 10;

/// YouTube draws chapters only when there are this many.
pub const YOUTUBE_FEWEST: usize = 3;

/// One chapter: seconds into the recording, and what it is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub at: i64,
    pub title: String,
}

/// When a recording djmanzo made started, from its name.
#[must_use]
pub fn recording_start(file_name: &str) -> Option<i64> {
    file_name
        .strip_prefix("set-")?
        .strip_suffix(".wav")?
        .parse()
        .ok()
}

fn title_of(play: &PlayRecord) -> String {
    if play.artist.trim().is_empty() {
        play.title.clone()
    } else {
        format!("{} - {}", play.artist, play.title)
    }
}

/// The chapters of a recording that started at `start` (unix seconds) and
/// runs `length` seconds, from the plays of the night.
#[must_use]
pub fn chapters(plays: &[PlayRecord], start: i64, length: i64) -> Vec<Chapter> {
    let mut sorted: Vec<&PlayRecord> = plays.iter().collect();
    sorted.sort_by_key(|p| p.played_at);

    let playing_at_start = sorted
        .iter()
        .rfind(|p| p.played_at < start && p.played_at >= start - LOOK_BACK);
    let mut out: Vec<Chapter> = playing_at_start
        .map(|p| Chapter {
            at: 0,
            title: title_of(p),
        })
        .into_iter()
        .collect();
    out.extend(
        sorted
            .iter()
            .filter(|p| p.played_at >= start && p.played_at < start + length)
            .map(|p| Chapter {
                at: p.played_at - start,
                title: title_of(p),
            }),
    );

    // A chapter shorter than YouTube's shortest was a preview: the record
    // that stayed is the chapter.
    let mut kept: Vec<Chapter> = Vec::with_capacity(out.len());
    for (i, chapter) in out.iter().enumerate() {
        let ends = out.get(i + 1).map_or(length, |next| next.at);
        if ends - chapter.at >= SHORTEST {
            kept.push(chapter.clone());
        }
    }

    match kept.first_mut() {
        None => {}
        Some(first) if first.at <= FIRST_WITHIN => first.at = 0,
        Some(_) => kept.insert(
            0,
            Chapter {
                at: 0,
                title: "Start".to_owned(),
            },
        ),
    }
    kept
}

/// The chapters as a description's lines: `0:00 Artist - Title`, one a line,
/// hours shown once the recording runs past one.
#[must_use]
pub fn written(chapters: &[Chapter]) -> String {
    chapters
        .iter()
        .map(|c| format!("{} {}", crate::share::clock(c.at), c.title))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A recording in the recordings folder, with its chapters for a night.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub file: String,
    pub path: std::path::PathBuf,
    pub start: i64,
    pub seconds: f64,
    pub chapters: Vec<Chapter>,
}

/// Every recording in `dir` that the night's `plays` have chapters in,
/// earliest first.
///
/// Asked of the chapters rather than decided by comparing times first: a
/// recording started after the night's last *counted* play still holds that
/// record, playing when it started — which a time comparison dropped, and
/// driving the application found.
#[must_use]
pub fn in_folder(plays: &[PlayRecord], dir: &std::path::Path) -> Vec<Found> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Found> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            let start = recording_start(&file)?;
            let seconds = crate::wav::Wav::seconds_of(entry.path())?;
            #[allow(clippy::cast_possible_truncation)]
            let chapters = chapters(plays, start, seconds.round() as i64);
            (!chapters.is_empty()).then(|| Found {
                file,
                path: entry.path(),
                start,
                seconds,
                chapters,
            })
        })
        .collect();
    found.sort_by_key(|f| f.start);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: i64 = 1_700_000_000;

    fn play(at: i64, artist: &str, title: &str) -> PlayRecord {
        PlayRecord {
            track_id: format!("{at}"),
            title: title.into(),
            artist: artist.into(),
            played_at: at,
            session_id: Some("s".into()),
        }
    }

    /// **Timed against the recording, not the wall clock**: the record
    /// playing when recording started is `0:00`, the next ones where they
    /// came in, and plays outside the recording are not in it.
    #[test]
    fn the_tracklist_is_timed_against_the_file() {
        let plays = [
            play(START - 2_000, "Too", "Early"),
            play(START - 90, "Aventura", "Obsesión"),
            play(START + 245, "Juan Luis Guerra", "Bachata Rosa"),
            play(START + 490, "Romeo Santos", "Propuesta Indecente"),
            play(START + 5_000, "After", "The Recording"),
        ];
        let got = chapters(&plays, START, 900);
        assert_eq!(
            written(&got),
            "0:00 Aventura - Obsesión\n4:05 Juan Luis Guerra - Bachata Rosa\n8:10 Romeo Santos - Propuesta Indecente"
        );
        assert!(got.len() >= YOUTUBE_FEWEST);
    }

    /// YouTube needs the first chapter at `0:00`. A record that came in a
    /// few seconds after the start is moved there; one that came in two
    /// minutes later is not, and `0:00` is the start.
    #[test]
    fn the_first_chapter_is_at_zero_and_honest() {
        let soon = chapters(
            &[play(START + 12, "A", "One"), play(START + 300, "B", "Two")],
            START,
            600,
        );
        assert_eq!(
            soon[0],
            Chapter {
                at: 0,
                title: "A - One".into()
            }
        );
        let late = chapters(&[play(START + 120, "A", "One")], START, 600);
        assert_eq!(written(&late), "0:00 Start\n2:00 A - One");
        assert!(chapters(&[], START, 600).is_empty());
    }

    /// A record that lasted less than ten seconds before the next was a
    /// preview, and a chapter YouTube would refuse.
    #[test]
    fn a_preview_is_not_a_chapter() {
        let plays = [
            play(START, "A", "Kept"),
            play(START + 200, "B", "Previewed"),
            play(START + 205, "C", "Stayed"),
        ];
        let got = written(&chapters(&plays, START, 600));
        assert_eq!(got, "0:00 A - Kept\n3:25 C - Stayed");
        // The last one too, if the recording ends within ten seconds of it.
        let ending = written(&chapters(
            &[play(START, "A", "One"), play(START + 595, "B", "Two")],
            START,
            600,
        ));
        assert_eq!(ending, "0:00 A - One");
    }

    #[test]
    fn a_long_recording_shows_hours_and_a_title_alone_has_no_dash() {
        let got = written(&chapters(
            &[
                play(START, "", "Untitled"),
                play(START + 3_725, "B", "Late"),
            ],
            START,
            4_000,
        ));
        assert_eq!(got, "0:00 Untitled\n1:02:05 B - Late");
    }

    #[test]
    fn a_recording_is_named_for_when_it_started() {
        assert_eq!(recording_start("set-1700000000.wav"), Some(START));
        assert_eq!(recording_start("mix-at-12s.wav"), None);
        assert_eq!(recording_start("set-x.wav"), None);
    }

    /// A recording as the recorder writes one, at a low rate so a
    /// seven-minute file is small.
    fn recording(dir: &std::path::Path, start: i64, seconds: usize) {
        let mut wav = crate::wav::Wav::create(dir.join(format!("set-{start}.wav")), 1_000).unwrap();
        wav.write(&vec![0_i16; 1_000 * 2 * seconds]).unwrap();
        wav.close().unwrap();
    }

    /// **A night's recordings are the ones its records play in** — the one
    /// started after the last counted play included, because that record was
    /// still playing; another night's recording, and anything not a
    /// recording, are not.
    #[test]
    fn the_nights_recordings_are_found_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        recording(dir.path(), START + 13, 45);
        recording(dir.path(), START + 50_000, 30);
        recording(dir.path(), START - 400, 420);
        // Over before the record came in: nothing of this night is in it.
        recording(dir.path(), START - 900, 60);
        std::fs::write(dir.path().join("notes.txt"), "not a recording").unwrap();
        std::fs::write(dir.path().join("set-99.wav"), "a broken file").unwrap();

        let plays = [play(START, "Aventura", "Obsesión")];
        let found = in_folder(&plays, dir.path());
        let names: Vec<&str> = found.iter().map(|f| f.file.as_str()).collect();
        // The recording made a minute before the record came in holds it
        // from 6:40; the one started after it was counted holds it from 0:00.
        assert_eq!(names, vec!["set-1699999600.wav", "set-1700000013.wav"]);
        assert_eq!(written(&found[1].chapters), "0:00 Aventura - Obsesión");
        assert!((found[1].seconds - 45.0).abs() < 1e-9);
        assert!(in_folder(&plays, &dir.path().join("missing")).is_empty());
    }

    /// The length is read from the file: rate from the header, frames from
    /// the bytes, as the recorder writes them.
    #[test]
    fn a_recording_knows_its_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("set-1.wav");
        let mut wav = crate::wav::Wav::create(&path, 48_000).unwrap();
        wav.write(&vec![0_i16; 48_000 * 2 * 3]).unwrap();
        wav.close().unwrap();
        let seconds = crate::wav::Wav::seconds_of(&path).unwrap();
        assert!((seconds - 3.0).abs() < 1e-9, "{seconds}");
        std::fs::write(
            dir.path().join("junk.wav"),
            b"not a wav at all, not at all.......................",
        )
        .unwrap();
        assert_eq!(
            crate::wav::Wav::seconds_of(dir.path().join("junk.wav")),
            None
        );
    }
}

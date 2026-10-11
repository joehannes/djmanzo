//! K8: a singer's range, a record's, and the key to sing in.
//!
//! A range is two pitches, in cents above A4 — the scale K6's readings and
//! vocal lines are both on, at their true octave. A singer's is read from
//! their strip while they slide from their lowest held note to their highest;
//! a record's from its vocal line. A record's is the 10th to the 90th
//! percentile of what is sung — not the one high note everyone reaches for at
//! the end. A singer's is the 5th to the 95th: a slide passes its two ends
//! only briefly, and they are the very notes being asked for, while a crack,
//! a growl or an odd octave slip in the reading is a handful of readings. See
//! `docs/KARAOKE.md` §11.
//!
//! # The key
//!
//! A song fits when its range, moved by a key change and by whole octaves,
//! sits inside the singer's. An octave is free — a baritone sings a soprano's
//! song an octave down, as karaoke always has — and only the key change is
//! the deck's. The change offered is the smallest that fits.

use serde::{Deserialize, Serialize};

/// How long a range take listens, in seconds.
pub const TAKE_SECONDS: f64 = 20.0;

/// The least voice a range is read from, in seconds of readings.
pub const FEWEST_SECONDS: f64 = 4.0;

/// How many readings a second a strip gives: one each time the listening
/// thread wakes ([`crate::listen::EVERY`]).
pub const READINGS_A_SECOND: f64 = 20.0;

/// The furthest a key change is offered: more than this and a record sounds
/// like something else.
pub const STRETCH: i32 = 4;

/// The furthest a key is moved at all, as a stretch.
pub const FURTHEST: i32 = 6;

/// A range of pitches, in cents above A4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub low: f32,
    pub high: f32,
}

impl Range {
    /// The range moved by `cents`.
    #[must_use]
    pub fn moved(self, cents: f32) -> Self {
        Self {
            low: self.low + cents,
            high: self.high + cents,
        }
    }
}

/// How much of each end a singer's take leaves out.
pub const SINGER_TRIM: f32 = 0.05;

/// How much of each end a record's line leaves out.
pub const RECORD_TRIM: f32 = 0.1;

/// The `trim` to `1 − trim` percentiles of `points`, or `None` for fewer
/// than `fewest` of them.
#[must_use]
pub fn of(points: impl IntoIterator<Item = f32>, fewest: usize, trim: f32) -> Option<Range> {
    let mut points: Vec<f32> = points.into_iter().filter(|p| p.is_finite()).collect();
    if points.len() < fewest.max(1) {
        return None;
    }
    points.sort_by(f32::total_cmp);
    let at = |share: f32| {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let index = ((points.len() - 1) as f32 * share).round() as usize;
        points[index]
    };
    Some(Range {
        low: at(trim),
        high: at(1.0 - trim),
    })
}

/// A singer's range from their strip's readings over a take — `None` for
/// less than [`FEWEST_SECONDS`] of voice.
#[must_use]
pub fn of_singer(readings: &[f32]) -> Option<Range> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let fewest = (FEWEST_SECONDS * READINGS_A_SECOND) as usize;
    of(readings.iter().copied(), fewest, SINGER_TRIM)
}

/// A record's range from its vocal line — `None` for a line that cannot
/// score anyone (K6's `LEAST_SUNG`), which has no range either.
#[must_use]
pub fn of_line(line: &dj_library::vocal_line::VocalLine) -> Option<Range> {
    if !line.scorable() {
        return None;
    }
    of(line.cents.iter().flatten().copied(), 1, RECORD_TRIM)
}

/// How a song is offered to a singer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "fit", content = "key", rename_all = "kebab-case")]
pub enum Offer {
    /// It fits as recorded.
    AsRecorded,
    /// It fits moved by this many semitones, no more than [`STRETCH`].
    Key(i32),
    /// It fits only moved further, up to [`FURTHEST`]: shown, not offered.
    Stretch(i32),
}

impl Offer {
    /// The key change it sets.
    #[must_use]
    pub const fn key(self) -> i32 {
        match self {
            Offer::AsRecorded => 0,
            Offer::Key(key) | Offer::Stretch(key) => key,
        }
    }
}

/// How far `song`, moved by `key` semitones and the octave that suits it
/// best, stays inside `singer`, in semitones — negative where it spills
/// over.
#[must_use]
pub fn margin(singer: Range, song: Range, key: i32) -> f32 {
    (-3..=3)
        .map(|octave: i32| {
            #[allow(clippy::cast_precision_loss)]
            let moved = song.moved((key * 100 + octave * 1200) as f32);
            (moved.low - singer.low).min(singer.high - moved.high) / 100.0
        })
        .fold(f32::NEG_INFINITY, f32::max)
}

/// The key to offer `song` to `singer` in: the smallest change that fits,
/// the one leaving more room when two are as small; `None` when none up to
/// [`FURTHEST`] does — a song wider than the singer never does.
#[must_use]
pub fn offer(singer: Range, song: Range) -> Option<Offer> {
    let mut keys: Vec<i32> = (-FURTHEST..=FURTHEST).collect();
    keys.sort_by(|a, b| {
        a.abs()
            .cmp(&b.abs())
            .then(margin(singer, song, *b).total_cmp(&margin(singer, song, *a)))
    });
    let key = keys
        .into_iter()
        .find(|key| margin(singer, song, *key) >= 0.0)?;
    Some(match key {
        0 => Offer::AsRecorded,
        key if key.abs() <= STRETCH => Offer::Key(key),
        key => Offer::Stretch(key),
    })
}

/// A singer's range being read from their strip.
#[derive(Debug, Clone)]
pub struct Take {
    /// Whose, by their name in the rotation.
    pub singer: String,
    /// The strip they are on.
    pub strip: usize,
    pub started: std::time::Instant,
    /// The voiced readings so far.
    pub readings: Vec<f32>,
    /// What it came to, once over: their range, or why not.
    pub result: Option<Result<Range, String>>,
}

impl Take {
    #[must_use]
    pub fn new(singer: &str, strip: usize) -> Self {
        Self {
            singer: singer.to_owned(),
            strip,
            started: std::time::Instant::now(),
            readings: Vec::new(),
            result: None,
        }
    }

    /// One reading of every strip, `seconds` into the take: this singer's
    /// kept while it runs, and the range read once it is over. Answers the
    /// range the moment it is read, once.
    pub fn hear(&mut self, heard: &[Option<f32>], seconds: f64) -> Option<Range> {
        if self.result.is_some() {
            return None;
        }
        if seconds < TAKE_SECONDS {
            if let Some(pitch) = heard.get(self.strip).copied().flatten() {
                self.readings.push(pitch);
            }
            return None;
        }
        let range = of_singer(&self.readings);
        self.result = Some(range.ok_or_else(|| {
            format!(
                "Not enough of {}'s voice was heard to read a range: sing from your lowest \
                 note to your highest, close to the microphone, and try again.",
                self.singer
            )
        }));
        range
    }

    /// Seconds of voice heard so far.
    #[must_use]
    pub fn voiced(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let readings = self.readings.len() as f64;
        readings / READINGS_A_SECOND
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cents above A4 of a note: A4 is 0, A3 −1200.
    fn note(semitones_from_a4: i32) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let cents = (semitones_from_a4 * 100) as f32;
        cents
    }

    /// A2 to E4: a baritone's octave and a fifth.
    fn baritone() -> Range {
        Range {
            low: note(-24),
            high: note(-5),
        }
    }

    /// **A slide's range is its ends, not its cracks.** Twenty seconds
    /// sliding F2 to G#4 and back, with a squeak an octave up and a growl at
    /// the bottom, reads within two thirds of a semitone of the 5th and 95th
    /// percentile of the slide — the squeak and the growl left out.
    #[test]
    fn a_slide_reads_as_its_range() {
        let mut readings: Vec<f32> = Vec::new();
        for n in 0..400 {
            // A triangle from A2 up to E4 and down again.
            #[allow(clippy::cast_precision_loss)]
            let phase = (n % 200) as f32 / 100.0;
            let up = if phase <= 1.0 { phase } else { 2.0 - phase };
            readings.push(note(-28) + up * (note(-1) - note(-28)));
        }
        readings.extend([note(15), note(16), note(-40)]);
        let range = of_singer(&readings).expect("a range");
        let span = note(-1) - note(-28);
        assert!(
            (range.low - (note(-28) + 0.05 * span)).abs() < 60.0,
            "{range:?}"
        );
        assert!(
            (range.high - (note(-28) + 0.95 * span)).abs() < 60.0,
            "{range:?}"
        );
    }

    /// **Four seconds of voice is the least a range is read from.**
    #[test]
    fn too_little_voice_is_no_range() {
        assert_eq!(of_singer(&[note(0); 79]), None);
        assert!(of_singer(&[note(0); 80]).is_some());
    }

    /// **A song that fits is offered as recorded; one a little high, down;
    /// one an octave and a bit away, by the bit — the octave is free.**
    #[test]
    fn the_smallest_change_that_fits_is_offered() {
        let singer = baritone();
        let fits = Range {
            low: note(-20),
            high: note(-8),
        };
        assert_eq!(offer(singer, fits), Some(Offer::AsRecorded));
        let high = Range {
            low: note(-17),
            high: note(-2),
        };
        assert_eq!(offer(singer, high), Some(Offer::Key(-3)));
        // A soprano's song, an octave and two semitones above where it fits.
        let soprano = Range {
            low: note(-3),
            high: note(9),
        };
        assert_eq!(offer(singer, soprano), Some(Offer::Key(-2)));
    }

    /// **Past four semitones a stretch, past six nothing, and a song wider
    /// than the singer never fits.**
    #[test]
    fn far_keys_are_a_stretch_and_wide_songs_never_fit() {
        let narrow = Range {
            low: note(-12),
            high: note(-7),
        };
        let five_up = Range {
            low: note(-17),
            high: note(-12),
        };
        assert_eq!(offer(narrow, five_up), Some(Offer::Stretch(5)));
        let six_away = Range {
            low: note(-18),
            high: note(-13),
        };
        assert_eq!(
            offer(narrow, six_away).map(Offer::key).map(i32::abs),
            Some(6)
        );
        let wide = Range {
            low: note(-24),
            high: note(0),
        };
        assert_eq!(offer(narrow, wide), None);
    }

    /// **An octave is free**: a song an octave away has the room it has
    /// where it is.
    #[test]
    fn an_octave_is_free() {
        let singer = baritone();
        let song = Range {
            low: note(-20),
            high: note(-8),
        };
        for octave in [-1, 1] {
            let away = song.moved(note(12 * octave));
            assert_eq!(margin(singer, away, 0), margin(singer, song, 0));
            assert_eq!(offer(singer, away), Some(Offer::AsRecorded));
        }
    }

    /// **A record's range is its sung points, and a line that cannot score
    /// has none.**
    #[test]
    fn a_records_range_is_its_line() {
        let mut cents: Vec<Option<f32>> = (0..100)
            .map(|n| Some(note(-12) + n as f32 * 12.0))
            .collect();
        cents.extend(std::iter::repeat_n(None, 50));
        let line = dj_library::vocal_line::VocalLine {
            separator: "test".to_owned(),
            rate: 20.0,
            cents,
        };
        let range = of_line(&line).expect("a range");
        assert!(range.low > note(-12) && range.high < note(0), "{range:?}");
        // A few stray points — a separator that found no single voice — is
        // a line that scores nobody, and has no range either.
        let mut stray = vec![None; 200];
        stray[10] = Some(note(-3));
        stray[90] = Some(note(-20));
        let stray = dj_library::vocal_line::VocalLine {
            separator: "test".to_owned(),
            rate: 20.0,
            cents: stray,
        };
        assert!(!stray.scorable());
        assert_eq!(of_line(&stray), None);
    }

    /// **A take keeps its own strip's voice for twenty seconds, then reads
    /// the range once — or says there was not enough voice.**
    #[test]
    fn a_take_reads_its_strip_for_twenty_seconds() {
        let mut take = Take::new("Ana", 1);
        for n in 0..300 {
            #[allow(clippy::cast_precision_loss)]
            let pitch = note(-24) + (n % 100) as f32 * 19.0;
            // Strip 0 is someone else, much higher.
            assert_eq!(
                take.hear(&[Some(note(10)), Some(pitch)], f64::from(n) * 0.05),
                None
            );
        }
        assert_eq!(take.readings.len(), 300);
        assert!((take.voiced() - 15.0).abs() < 1e-9);
        let range = take.hear(&[None, None], 20.0).expect("a range");
        assert!(range.high < note(0), "strip 0's voice got in: {range:?}");
        assert_eq!(take.hear(&[None, None], 21.0), None, "read once");

        let mut quiet = Take::new("Bo", 0);
        quiet.hear(&[Some(note(0))], 1.0);
        assert_eq!(quiet.hear(&[None], 20.0), None);
        assert!(matches!(quiet.result, Some(Err(ref why)) if why.contains("Bo")));
    }
}

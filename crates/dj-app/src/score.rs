//! K6: a singer scored against the line their song's voice sings.
//!
//! The line is the record's own — read from its separated vocal stem
//! (`dj_analysis::melody::line`) — and the singer is what the listening
//! hears on their microphone (`crate::listen`), both in cents above A4.
//! [`Score`] compares them point by point as the song plays. See
//! `docs/KARAOKE.md` §9.
//!
//! # The rules, each with its reason
//!
//! - **Octave-agnostic.** The difference is folded into one octave: a
//!   baritone singing a soprano's line an octave down sings it right.
//! - **In the key it is sung in.** The line moves by the song's key, as the
//!   deck does.
//! - **A timing tolerance.** A point is judged by the best of what was heard
//!   within [`TOLERANCE_SECONDS`] of it, so a singer a breath behind is not
//!   wrong — and settled only once the song is that far past it.
//! - **Graded.** Within [`HIT_CENTS`] is a hit, within [`NEAR_CENTS`] half,
//!   beyond, nothing.
//! - **Only where the line is sung.** A rest, or a rapped verse the line has
//!   no pitch for, is not marked at all — not singing there is not wrong.

use std::collections::VecDeque;
use std::sync::Arc;

/// How far either side of a point a singer may be and still be on it.
pub const TOLERANCE_SECONDS: f64 = 0.15;

/// Within this many cents of the line is a hit.
pub const HIT_CENTS: f32 = 50.0;

/// Within this many is half a hit: near, and audibly not right.
pub const NEAR_CENTS: f32 = 150.0;

/// A jump back in the song further than this is a seek or a restart, and
/// scoring starts again from where the song now is.
const REWOUND_SECONDS: f64 = 1.0;

/// K6: the song being scored, and every strip's singer and score through it.
#[derive(Debug, Default)]
pub struct Scoring {
    /// The up-next singer's song as last read from the rotation — the
    /// record, its key and the singer — or `None` with nobody up or scoring
    /// switched off.
    pub up_next: Option<(String, String, i32)>,
    /// When the rotation was last read: a second at most between reads, not
    /// twenty times one.
    pub checked: Option<std::time::Instant>,
    /// The song the scores are for, `(record, key)`.
    pub song: Option<(String, i32)>,
    /// Its line, when it can be scored, and the points a second.
    pub line: Option<(Arc<Vec<Option<f32>>>, f64)>,
    /// Per strip, the singer on it and their score through this song.
    pub strips: Vec<Option<(String, Score)>>,
}

impl Scoring {
    /// Every singer's score through the song, by strip.
    #[must_use]
    pub fn scores(&self) -> Vec<(u8, String, Option<f32>)> {
        self.strips
            .iter()
            .enumerate()
            .filter_map(|(strip, held)| {
                let (singer, score) = held.as_ref()?;
                #[allow(clippy::cast_possible_truncation)]
                Some((strip as u8, singer.clone(), score.percent()))
            })
            .collect()
    }

    /// The score of `singer` — from the microphone they were on and nobody
    /// else's, so a duet partner's score is never kept as theirs. `None`
    /// when they were on none, or nothing sung had passed yet.
    #[must_use]
    pub fn score_of(&self, singer: &str) -> Option<f32> {
        self.scores()
            .into_iter()
            .find(|(_, who, _)| crate::guests::same_name(who, singer))
            .and_then(|(_, _, percent)| percent)
    }

    /// Forget the song: it was sung, or the night is over.
    pub fn reset(&mut self) {
        self.song = None;
        self.line = None;
        self.strips.clear();
        self.checked = None;
    }
}

/// What one heard pitch is worth against one point of the line.
#[must_use]
pub fn grade(heard: f32, line: f32) -> f32 {
    let apart = (heard - line).rem_euclid(1200.0);
    let apart = apart.min(1200.0 - apart);
    if apart <= HIT_CENTS {
        1.0
    } else if apart <= NEAR_CENTS {
        0.5
    } else {
        0.0
    }
}

/// One singer's score through one song.
#[derive(Debug, Clone)]
pub struct Score {
    line: Arc<Vec<Option<f32>>>,
    /// Points a second.
    rate: f64,
    /// The song's key, in cents from the record's.
    key: f32,
    /// What was heard, by seconds into the song — only as far back as a
    /// point still to be settled could want.
    heard: VecDeque<(f64, Option<f32>)>,
    /// The next point of the line to settle.
    next: usize,
    /// Points where the line is sung, settled so far.
    sung: u32,
    /// What they earned.
    credit: f32,
    /// The latest song time heard.
    latest: f64,
}

impl Score {
    /// A score against `line`, `rate` points a second, the song sung
    /// `semitones` from the record's key.
    #[must_use]
    pub fn new(line: Arc<Vec<Option<f32>>>, rate: f64, semitones: i32) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let key = semitones as f32 * 100.0;
        Self {
            line,
            rate,
            key,
            heard: VecDeque::new(),
            next: 0,
            sung: 0,
            credit: 0.0,
            latest: 0.0,
        }
    }

    fn time_of(&self, point: usize) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let at = point as f64 / self.rate;
        at
    }

    /// What was heard `at` seconds into the song: a pitch in cents above A4,
    /// or none. Settles every point of the line the song is now far enough
    /// past.
    pub fn hear(&mut self, at: f64, cents: Option<f32>) {
        if at + REWOUND_SECONDS < self.latest {
            // A seek or a restart: carry on from where the song is now.
            self.heard.clear();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let from = ((at - TOLERANCE_SECONDS).max(0.0) * self.rate).ceil() as usize;
            self.next = from;
        }
        self.latest = at;
        self.heard.push_back((at, cents));
        while self.next < self.line.len() && self.time_of(self.next) + TOLERANCE_SECONDS < at {
            let when = self.time_of(self.next);
            if let Some(line) = self.line[self.next] {
                let line = line + self.key;
                self.sung += 1;
                self.credit += self
                    .heard
                    .iter()
                    // A hair over the tolerance: song times are sums of
                    // floats, and a singer exactly 150 ms behind must not
                    // fall out of it on rounding.
                    .filter(|(t, _)| (t - when).abs() <= TOLERANCE_SECONDS + 1e-9)
                    .filter_map(|(_, heard)| heard.map(|h| grade(h, line)))
                    .fold(0.0, f32::max);
            }
            self.next += 1;
        }
        let keep_from = self.time_of(self.next) - TOLERANCE_SECONDS;
        while self.heard.front().is_some_and(|(t, _)| *t < keep_from) {
            self.heard.pop_front();
        }
    }

    /// The score so far, 0 to 100 — `None` until a sung point of the line has
    /// been settled.
    #[must_use]
    pub fn percent(&self) -> Option<f32> {
        #[allow(clippy::cast_precision_loss)]
        (self.sung > 0).then(|| 100.0 * self.credit / self.sung as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 20.0;
    const HEARD: f64 = 0.05;

    /// Ten seconds of a tune, a note a second (A3 C4 E4 D4 C4, twice), each
    /// after a breath of 200 ms — longer than the tolerance, so a note is
    /// judged by what was sung on it and never by its neighbour.
    fn tune() -> Arc<Vec<Option<f32>>> {
        let notes = [-1200.0, -900.0, -500.0, -700.0, -900.0];
        Arc::new(
            (0..200)
                .map(|point| {
                    let note = notes[(point / 20) % notes.len()];
                    (point % 20 >= 4).then_some(note)
                })
                .collect(),
        )
    }

    /// A singer heard every 50 ms through the song and two seconds past its
    /// end, singing the line moved by `cents` and `late` seconds behind it.
    fn sing(semitones: i32, cents: f32, late: f64) -> f32 {
        let line = tune();
        let mut score = Score::new(Arc::clone(&line), RATE, semitones);
        for step in 0..240 {
            let at = step as f64 * HEARD;
            let sung_at = at - late;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let point = (sung_at * RATE).round() as usize;
            let heard = (sung_at >= 0.0)
                .then(|| line.get(point).copied().flatten())
                .flatten()
                .map(|c| c + cents);
            score.hear(at, heard);
        }
        score.percent().expect("a sung line was settled")
    }

    /// **On the line is a hundred; an octave down is the same song.**
    #[test]
    fn on_the_line_scores_a_hundred_and_an_octave_is_the_same() {
        assert!(sing(0, 0.0, 0.0) > 99.9);
        assert!(sing(0, -1200.0, 0.0) > 99.9);
        assert!(sing(0, 2400.0, 0.0) > 99.9);
    }

    /// **Graded: a semitone sharp throughout is half, a tritone off
    /// nothing.** Thirty cents is on the note.
    #[test]
    fn a_semitone_off_is_half_and_a_tritone_is_nothing() {
        let sharp = sing(0, 100.0, 0.0);
        assert!((sharp - 50.0).abs() < 0.1, "{sharp}");
        assert!(sing(0, 30.0, 0.0) > 99.9, "thirty cents is on the note");
        assert!(sing(0, 600.0, 0.0) < 0.1);
        assert!(sing(0, 700.0, 0.0) < 0.1, "a fifth off");
    }

    /// **In the key it is sung in.** Taken two semitones down, the song is
    /// scored against the line two semitones down.
    #[test]
    fn the_line_moves_with_the_key() {
        assert!(sing(-2, -200.0, 0.0) > 99.9);
        assert!(
            sing(-2, 0.0, 0.0) < 0.1,
            "sung as recorded with the key moved"
        );
    }

    /// **A breath behind is not wrong.** 150 ms late scores as on time; a
    /// quarter of a second late loses only the start of each note — here two
    /// points of sixteen — not the notes.
    #[test]
    fn a_singer_a_breath_behind_is_not_wrong() {
        assert!(sing(0, 0.0, 0.15) > 99.9);
        let later = sing(0, 0.0, 0.25);
        assert!((85.0..95.0).contains(&later), "{later}");
    }

    /// **Silence is nothing, and a rest is not marked.** Nobody singing
    /// scores zero; a line with no sung point scores nobody.
    #[test]
    fn silence_scores_nothing_and_a_rest_is_not_marked() {
        let mut quiet = Score::new(tune(), RATE, 0);
        for step in 0..240 {
            quiet.hear(step as f64 * HEARD, None);
        }
        assert_eq!(quiet.percent(), Some(0.0));
        let mut rests = Score::new(Arc::new(vec![None; 100]), RATE, 0);
        for step in 0..120 {
            rests.hear(step as f64 * HEARD, Some(0.0));
        }
        assert_eq!(rests.percent(), None);
    }

    /// **K6: the score kept for a singer is the one from their own
    /// microphone.** Bo singing beside Ana does not lend her his score, a
    /// name is matched as the guest book matches it, and a singer on no
    /// microphone has none.
    #[test]
    fn a_singers_score_is_their_own_microphones() {
        let line = tune();
        let mut scoring = Scoring::default();
        let sung = |cents: f32| {
            let mut score = Score::new(Arc::clone(&line), RATE, 0);
            for step in 0..240 {
                let at = step as f64 * HEARD;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let point = (at * RATE).round() as usize;
                score.hear(at, line.get(point).copied().flatten().map(|c| c + cents));
            }
            score
        };
        scoring.strips = vec![
            Some(("Bo".to_owned(), sung(0.0))),
            None,
            Some(("Ana".to_owned(), sung(100.0))),
        ];
        assert!((scoring.score_of("Ana").unwrap() - 50.0).abs() < 0.1);
        assert!((scoring.score_of("ana").unwrap() - 50.0).abs() < 0.1);
        assert!(scoring.score_of("Bo").unwrap() > 99.9);
        assert_eq!(scoring.score_of("Cy"), None);
    }

    /// **A restart starts again from where the song is**, rather than
    /// settling the points it skipped back over a second time.
    #[test]
    fn a_restart_carries_on_from_where_the_song_is() {
        let line = tune();
        let mut score = Score::new(Arc::clone(&line), RATE, 0);
        for step in 0..100 {
            let at = step as f64 * HEARD;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let point = (at * RATE).round() as usize;
            score.hear(at, line[point]);
        }
        let before = score.sung;
        for step in 0..60 {
            score.hear(step as f64 * HEARD, Some(700.0));
        }
        assert!(score.sung > before, "the restarted song was not scored");
        let percent = score.percent().unwrap();
        assert!(percent < 80.0 && percent > 40.0, "{percent}");
    }
}

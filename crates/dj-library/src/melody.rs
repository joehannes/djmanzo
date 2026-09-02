//! Pitch contours, kept beside the records they came from.
//!
//! [`dj_analysis::melody`] can tell whether a hum is in a record; this is
//! where the record's half of that comparison lives, so the answer does not
//! mean decoding the whole collection every time somebody sings something.
//!
//! # Why a byte a point
//!
//! A contour is one number every hundred milliseconds, so a five-minute record
//! is three thousand of them. Stored as `f32` that is twelve kilobytes a
//! record and a hundred and twenty megabytes for a collection of ten thousand;
//! stored as a byte it is three, and thirty.
//!
//! A byte holds quarter-semitones, which is twenty-five cents -- finer than
//! anybody hums, and far finer than it needs to be given that the search folds
//! everything into an octave anyway. [`UNVOICED`] is the one value that means
//! "nothing here" rather than a pitch, which is why the usable range is
//! ±31 semitones rather than ±32.
//!
//! # Why the search is a scan
//!
//! There is no index that helps. Two melodies are near each other under
//! dynamic time warping, which is not a metric and has no triangle inequality,
//! so the usual tricks for pruning a nearest-neighbour search do not apply.
//! What does apply is that a contour is small and the comparison is cheap: an
//! eight-second hum against a five-minute record is about a quarter of a
//! million cells, which is well under a millisecond, and a collection of ten
//! thousand records is a few seconds of one core.
//!
//! Records with no contour yet are simply not searched, and [`without_melody`]
//! is how a background sweep finds them -- the same shape the lyrics sweep
//! uses, for the same reason.

use dj_analysis::melody::{Contour, Match, RATE, find};
use dj_core::TrackId;
use rusqlite::{Connection, OptionalExtension, Result, params};

/// The byte that means "nothing periodic here".
///
/// `i8::MIN`. Chosen at the end of the range rather than in the middle of it
/// so that every other value is a pitch, and a reader that forgets to check
/// gets an absurd answer rather than a plausible one.
pub const UNVOICED: i8 = i8::MIN;

/// Steps of a semitone in one stored byte.
///
/// Four, so a quarter of a semitone. Twenty-five cents is finer than a person
/// hums and the search folds into octaves regardless; the resolution is here
/// so that a contour can also be drawn, not because the match needs it.
pub const STEPS_PER_SEMITONE: f32 = 4.0;

/// A record's contour, as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub contour: Contour,
    /// How much of the record was pitched at all, from zero to one.
    pub voiced: f32,
    /// Unix seconds, so a contour made by an older version can be spotted.
    pub made_at: i64,
}

/// A record whose melody matched, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub track: TrackId,
    /// Mean semitone error per point of the hum. Lower is better; zero is
    /// identical.
    pub cost: f32,
    /// Seconds into the record where the matching passage starts.
    pub at_seconds: f64,
}

/// Pack a contour into one byte per point.
#[must_use]
pub fn pack(contour: &Contour) -> Vec<u8> {
    contour
        .semitones
        .iter()
        .map(|value| {
            let Some(semitones) = value else {
                return UNVOICED as u8;
            };
            // `UNVOICED` is the only value that is not a pitch, so the pitches
            // stop one short of it rather than wrapping onto it.
            let steps = (semitones * STEPS_PER_SEMITONE).round();
            let clamped = steps.clamp(f32::from(UNVOICED) + 1.0, f32::from(i8::MAX));
            #[expect(
                clippy::cast_possible_truncation,
                reason = "clamped to i8's range on the line above"
            )]
            let byte = clamped as i8;
            byte as u8
        })
        .collect()
}

/// Unpack what [`pack`] wrote.
#[must_use]
pub fn unpack(bytes: &[u8]) -> Contour {
    Contour {
        semitones: bytes
            .iter()
            .map(|byte| {
                let step = *byte as i8;
                (step != UNVOICED).then(|| f32::from(step) / STEPS_PER_SEMITONE)
            })
            .collect(),
        rate: RATE,
    }
}

/// Keep a record's contour.
///
/// Replaces whatever was there: a contour is derived from the audio, so a
/// second one for the same record is a better answer rather than another
/// answer.
///
/// # Errors
/// When the database refuses the write.
pub fn remember(db: &Connection, track: &TrackId, contour: &Contour) -> Result<()> {
    db.execute(
        "INSERT INTO melodies (track_id, points, voiced, made_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(track_id) DO UPDATE SET
             points = excluded.points,
             voiced = excluded.voiced,
             made_at = excluded.made_at",
        params![track.to_hex(), pack(contour), contour.voiced(), now()],
    )?;
    Ok(())
}

/// A record's contour, if one has been made.
///
/// # Errors
/// When the database refuses the read.
pub fn stored(db: &Connection, track: &TrackId) -> Result<Option<Stored>> {
    let found = db
        .query_row(
            "SELECT points, voiced, made_at FROM melodies WHERE track_id = ?1",
            params![track.to_hex()],
            |row| {
                let points: Vec<u8> = row.get(0)?;
                Ok(Stored {
                    contour: unpack(&points),
                    voiced: row.get::<_, f64>(1)? as f32,
                    made_at: row.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(found)
}

/// Records that have no contour yet, oldest first.
///
/// What a background sweep asks for. Ordered by when the record was added so
/// that a collection being worked through is worked through in one direction
/// rather than in a different random order every time the application starts.
///
/// # Errors
/// When the database refuses the read.
pub fn without_melody(db: &Connection, most: usize) -> Result<Vec<TrackId>> {
    let mut statement = db.prepare(
        "SELECT t.id FROM tracks t
         LEFT JOIN melodies m ON m.track_id = t.id
         WHERE m.track_id IS NULL
         ORDER BY t.added_at ASC
         LIMIT ?1",
    )?;
    let rows = statement.query_map(params![most as i64], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        if let Some(id) = TrackId::from_hex(&row?) {
            out.push(id);
        }
    }
    Ok(out)
}

/// How many records have a contour, and how many there are.
///
/// # Errors
/// When the database refuses the read.
pub fn progress(db: &Connection) -> Result<(usize, usize)> {
    let have: i64 = db.query_row("SELECT COUNT(*) FROM melodies", [], |row| row.get(0))?;
    let all: i64 = db.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    Ok((have as usize, all as usize))
}

/// Which records the hum is in, best first.
///
/// A scan, for the reason in the module header. Records shorter than the hum
/// are skipped by [`find`] itself rather than here, so a query longer than
/// everything in the collection comes back empty rather than wrong.
///
/// # Errors
/// When the database refuses the read.
pub fn search(db: &Connection, hum: &Contour, most: usize) -> Result<Vec<Hit>> {
    let mut statement = db.prepare("SELECT track_id, points FROM melodies")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;

    let mut hits: Vec<Hit> = Vec::new();
    for row in rows {
        let (id, points) = row?;
        let Some(track) = TrackId::from_hex(&id) else {
            continue;
        };
        let Some(Match { cost, at_seconds }) = find(hum, &unpack(&points)) else {
            continue;
        };
        hits.push(Hit {
            track,
            cost,
            at_seconds,
        });
    }

    hits.sort_by(|a, b| a.cost.total_cmp(&b.cost));
    hits.truncate(most);
    Ok(hits)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::migrate;
    use std::f32::consts::TAU;

    const SR: u32 = 48_000;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn
    }

    fn id(seed: u8) -> TrackId {
        TrackId::from_bytes([seed; 32])
    }

    fn add(db: &Connection, track: &TrackId, added_at: i64) {
        db.execute(
            "INSERT INTO tracks
                 (id, path, title, duration_frames, sample_rate, channels, added_at)
             VALUES (?1, ?2, ?3, 0, 48000, 2, ?4)",
            params![
                track.to_hex(),
                format!("/music/{added_at}.wav"),
                format!("track {added_at}"),
                added_at
            ],
        )
        .unwrap();
    }

    fn melody(steps: &[i32], each: f32) -> Vec<f32> {
        let mut out = Vec::new();
        for step in steps {
            let hz = 220.0 * 2.0f32.powf(*step as f32 / 12.0);
            let frames = (each * SR as f32) as usize;
            out.extend((0..frames).map(|n| {
                let t = n as f32 / SR as f32;
                0.6 * (TAU * hz * t).sin() + 0.3 * (TAU * hz * 2.0 * t).sin()
            }));
        }
        out
    }

    fn sung(steps: &[i32]) -> Contour {
        dj_analysis::melody::contour(&melody(steps, 0.4), SR)
    }

    /// **A contour survives being stored and read back.**
    ///
    /// The point of the whole file: what comes out has to still match what
    /// went in, or the search is comparing a hum against a rounding error.
    #[test]
    fn a_contour_round_trips_through_a_byte_a_point() {
        let tune = sung(&[0, 2, 4, 5, 7, 5, 4, 2]);
        let back = unpack(&pack(&tune));
        assert_eq!(back.len(), tune.len());
        assert_eq!(back.rate, tune.rate);

        let worst = tune
            .semitones
            .iter()
            .zip(&back.semitones)
            .filter_map(|(before, after)| Some((before.as_ref()?, after.as_ref()?)))
            .map(|(before, after)| (before - after).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst <= 0.5 / STEPS_PER_SEMITONE,
            "a byte moved a note by {worst} semitones"
        );

        // And it still finds itself, which is the property that matters.
        let hit = find(&tune, &back).expect("a match");
        assert!(
            hit.cost < 0.1,
            "a stored contour scored {} against itself",
            hit.cost
        );
    }

    /// **Unvoiced stays unvoiced, rather than becoming a very low note.**
    #[test]
    fn silence_in_a_contour_is_not_a_pitch() {
        let quiet = Contour {
            semitones: vec![Some(0.0), None, Some(3.0), None, None],
            rate: RATE,
        };
        let back = unpack(&pack(&quiet));
        assert_eq!(back.semitones, quiet.semitones);
    }

    /// **A pitch too far from the middle is clamped, not wrapped onto the
    /// value that means silence.**
    #[test]
    fn an_absurd_pitch_does_not_become_silence() {
        let absurd = Contour {
            semitones: vec![Some(-9_000.0), Some(9_000.0)],
            rate: RATE,
        };
        let back = unpack(&pack(&absurd));
        assert!(
            back.semitones.iter().all(Option::is_some),
            "a clamp wrapped onto the unvoiced marker: {:?}",
            back.semitones
        );
    }

    /// **Storing twice replaces rather than accumulates.**
    #[test]
    fn a_second_contour_replaces_the_first() {
        let db = db();
        let track = id(1);
        add(&db, &track, 100);

        remember(&db, &track, &sung(&[0, 2, 4])).unwrap();
        let second = sung(&[0, 4, 7, 12, 7, 4]);
        remember(&db, &track, &second).unwrap();

        let (have, _) = progress(&db).unwrap();
        assert_eq!(
            have, 1,
            "the second contour was added rather than replacing"
        );
        assert_eq!(
            stored(&db, &track).unwrap().expect("stored").contour.len(),
            second.len()
        );
    }

    /// **The sweep asks about records that have none, oldest first.**
    #[test]
    fn the_sweep_finds_what_has_no_contour_yet() {
        let db = db();
        let (old, middle, new) = (id(1), id(2), id(3));
        add(&db, &old, 100);
        add(&db, &middle, 200);
        add(&db, &new, 300);
        remember(&db, &middle, &sung(&[0, 2, 4])).unwrap();

        assert_eq!(without_melody(&db, 10).unwrap(), vec![old, new]);
        assert_eq!(without_melody(&db, 1).unwrap(), vec![old]);
        assert_eq!(progress(&db).unwrap(), (1, 3));
    }

    /// **The record the hum came from comes first.**
    #[test]
    fn the_right_record_is_ranked_first() {
        let db = db();
        let phrase = [0, 2, 4, 5, 7, 5, 4, 2];
        let (right, wrong, other) = (id(1), id(2), id(3));
        for (track, at) in [(&right, 100), (&wrong, 200), (&other, 300)] {
            add(&db, track, at);
        }
        remember(&db, &right, &sung(&phrase)).unwrap();
        remember(&db, &wrong, &sung(&[0, -3, 7, -5, 11, 1, -7, 9])).unwrap();
        remember(&db, &other, &sung(&[0, 1, 0, 1, 0, 1, 0, 1])).unwrap();

        let hits = search(&db, &sung(&phrase), 5).unwrap();
        assert_eq!(hits.len(), 3, "every record with a contour is scored");
        assert_eq!(hits[0].track, right, "ranked {:?}", hits);
        assert!(
            hits[0].cost < hits[1].cost,
            "the winner did not actually score better: {:?}",
            hits
        );
    }

    /// **A hum in another key still finds its record**, which is the whole
    /// reason the search compares intervals.
    #[test]
    fn humming_it_in_another_key_still_finds_it() {
        let db = db();
        let phrase = [0, 2, 4, 5, 7, 5, 4, 2];
        let (right, wrong) = (id(1), id(2));
        add(&db, &right, 100);
        add(&db, &wrong, 200);
        remember(&db, &right, &sung(&phrase)).unwrap();
        remember(&db, &wrong, &sung(&[0, -3, 7, -5, 11, 1, -7, 9])).unwrap();

        let a_fifth_up: Vec<i32> = phrase.iter().map(|s| s + 7).collect();
        let hits = search(&db, &sung(&a_fifth_up), 5).unwrap();
        assert_eq!(hits[0].track, right, "ranked {hits:?}");
    }

    /// **Only as many as were asked for.**
    #[test]
    fn the_shortlist_is_as_long_as_it_was_asked_to_be() {
        let db = db();
        for seed in 1..=5u8 {
            let track = id(seed);
            add(&db, &track, i64::from(seed));
            remember(&db, &track, &sung(&[0, 2, 4, 5, 7, 5, 4, 2])).unwrap();
        }
        assert_eq!(
            search(&db, &sung(&[0, 2, 4, 5, 7, 5, 4, 2]), 2)
                .unwrap()
                .len(),
            2
        );
    }

    /// **Nothing stored is an empty answer, not an error.**
    #[test]
    fn an_empty_collection_answers_nothing() {
        let db = db();
        assert!(
            search(&db, &sung(&[0, 2, 4, 5, 7, 5, 4, 2]), 5)
                .unwrap()
                .is_empty()
        );
        assert_eq!(progress(&db).unwrap(), (0, 0));
    }

    /// **A contour goes with its record.**
    #[test]
    fn deleting_a_record_takes_its_contour() {
        let db = db();
        db.execute("PRAGMA foreign_keys = ON", []).unwrap();
        let track = id(1);
        add(&db, &track, 100);
        remember(&db, &track, &sung(&[0, 2, 4])).unwrap();

        db.execute("DELETE FROM tracks WHERE id = ?1", params![track.to_hex()])
            .unwrap();
        assert_eq!(progress(&db).unwrap(), (0, 0));
    }
}

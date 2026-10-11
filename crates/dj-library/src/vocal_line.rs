//! K6: the line a record's voice sings, kept beside the record.
//!
//! Read from the separated vocal stem by [`dj_analysis::melody::line`] and
//! scored against while a singer sings the record — see `docs/KARAOKE.md` §9.
//! Kept rather than read again on every load because separation is slow and
//! the line does not change: what changes it is a better separator, which is
//! why the separator that produced a line is kept with it, and a line from
//! another one is replaced rather than trusted.
//!
//! # Why two bytes a point
//!
//! Twenty points a second is six thousand for a five-minute record. Whole
//! cents in an `i16` hold every pitch the line can contain — 70 Hz is −3,180
//! cents from A4 and 700 Hz is +804 — and [`UNSUNG`] is the one value that
//! means nobody sings there.

use dj_core::TrackId;
use rusqlite::{Connection, OptionalExtension, Result, params};

/// The value that means "nobody sings here" rather than a pitch.
pub const UNSUNG: i16 = i16::MIN;

/// How much of a record must be sung for its line to score a singer.
///
/// A twentieth. A song is sung for most of its length, and a line that is
/// sung for less than this is a separator that found no single voice — a
/// record of chords, or a choir, where YIN has no one period to follow. A
/// singer scored against that would be scored against a few stray points.
pub const LEAST_SUNG: f32 = 0.05;

/// A record's sung line, as kept.
#[derive(Debug, Clone, PartialEq)]
pub struct VocalLine {
    /// Which separator's vocal stem it was read from.
    pub separator: String,
    /// Points a second.
    pub rate: f64,
    /// Per point, cents above A4, or `None` where nobody sings.
    pub cents: Vec<Option<f32>>,
}

impl VocalLine {
    /// Whether there is enough of a line to score a singer against.
    #[must_use]
    pub fn scorable(&self) -> bool {
        self.sung() >= LEAST_SUNG
    }

    /// How much of the record is sung, zero to one.
    #[must_use]
    pub fn sung(&self) -> f32 {
        if self.cents.is_empty() {
            return 0.0;
        }
        self.cents.iter().flatten().count() as f32 / self.cents.len() as f32
    }
}

fn pack(cents: &[Option<f32>]) -> Vec<u8> {
    cents
        .iter()
        .flat_map(|point| {
            let value = point.map_or(UNSUNG, |c| {
                #[allow(clippy::cast_possible_truncation)]
                let whole = c.round().clamp(f32::from(UNSUNG + 1), f32::from(i16::MAX)) as i16;
                whole
            });
            value.to_le_bytes()
        })
        .collect()
}

fn unpack(bytes: &[u8]) -> Vec<Option<f32>> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let value = i16::from_le_bytes(*pair);
            (value != UNSUNG).then(|| f32::from(value))
        })
        .collect()
}

/// Keep a record's sung line, replacing whatever was kept for it.
///
/// # Errors
/// When the database refuses the write.
pub fn remember(db: &Connection, track: &TrackId, line: &VocalLine) -> Result<()> {
    db.execute(
        "INSERT INTO vocal_lines (track_id, separator, rate, cents, sung, made_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(track_id) DO UPDATE SET
             separator = excluded.separator,
             rate = excluded.rate,
             cents = excluded.cents,
             sung = excluded.sung,
             made_at = excluded.made_at",
        params![
            track.to_hex(),
            line.separator,
            line.rate,
            pack(&line.cents),
            f64::from(line.sung()),
            now()
        ],
    )?;
    Ok(())
}

/// A record's sung line, if one has been kept.
///
/// # Errors
/// When the database refuses the read.
pub fn stored(db: &Connection, track: &TrackId) -> Result<Option<VocalLine>> {
    db.query_row(
        "SELECT separator, rate, cents FROM vocal_lines WHERE track_id = ?1",
        params![track.to_hex()],
        |row| {
            let bytes: Vec<u8> = row.get(2)?;
            Ok(VocalLine {
                separator: row.get(0)?,
                rate: row.get(1)?,
                cents: unpack(&bytes),
            })
        },
    )
    .optional()
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

    /// **A line comes back as it went in, to the cent**, rests as rests and
    /// the separator with it; a second line for the record replaces the
    /// first.
    #[test]
    fn a_line_is_kept_to_the_cent_and_replaced_by_the_next() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&mut db).unwrap();
        let track = TrackId::from_bytes([3; 32]);
        db.execute(
            "INSERT INTO tracks (id, path, duration_frames, sample_rate, channels, added_at)
             VALUES (?1, '/music/x.flac', 1, 48000, 2, 0)",
            params![track.to_hex()],
        )
        .unwrap();
        assert_eq!(stored(&db, &track).unwrap(), None);
        let line = VocalLine {
            separator: "built-in".to_owned(),
            rate: 20.0,
            cents: vec![
                Some(-1200.0),
                None,
                Some(-500.4),
                Some(804.0),
                Some(-3180.0),
            ],
        };
        remember(&db, &track, &line).unwrap();
        let back = stored(&db, &track).unwrap().unwrap();
        assert_eq!(
            back.cents,
            vec![
                Some(-1200.0),
                None,
                Some(-500.0),
                Some(804.0),
                Some(-3180.0)
            ]
        );
        assert_eq!(back.separator, "built-in");
        assert!((back.sung() - 0.8).abs() < 1e-6);

        assert!(back.scorable());
        let stray = VocalLine {
            cents: (0..100).map(|n| (n < 4).then_some(0.0)).collect(),
            ..back.clone()
        };
        assert!(
            !stray.scorable(),
            "four points in a hundred scored a singer"
        );
        let enough = VocalLine {
            cents: (0..100).map(|n| (n < 5).then_some(0.0)).collect(),
            ..back.clone()
        };
        assert!(enough.scorable());

        let better = VocalLine {
            separator: "model".to_owned(),
            cents: vec![Some(0.0)],
            ..line
        };
        remember(&db, &track, &better).unwrap();
        assert_eq!(stored(&db, &track).unwrap(), Some(better));
    }
}

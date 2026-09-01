//! Finding a record from a line you half remember.
//!
//! # Why the search is over words and not over a search engine
//!
//! The question is "which of my records contains this phrase", and a phrase is
//! a run of words in order. Ranking, stemming and fuzzy term matching all make
//! *worse* answers here: a DJ typing "no puedo dormir" wants the record with
//! that line in it, not the four records that each contain one of those words.
//! So the match is a substring of the folded text, and the ordering is by how
//! early the phrase appears — because a hook in the first verse is the line
//! somebody remembers.
//!
//! # What the folding does, and why
//!
//! A record says `"Y no puedo dormir,"`. A DJ types `no puedo dormir`. Between
//! those two are a capital, a comma, and — half the time in this catalogue — an
//! accent the phone keyboard would not produce. Folding removes all three, on
//! both sides, so the two forms meet.

use dj_core::TrackId;
use rusqlite::{Connection, Result, params};

/// What the database knows about one record's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub plain: String,
    pub synced: Option<String>,
    /// Whether the lyrics database had anything. False is a real answer.
    pub found: bool,
    pub instrumental: bool,
    pub source: String,
    pub fetched_at: i64,
}

/// One record whose words contain the phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub track: TrackId,
    /// The line the phrase was found in, as it is written on the record.
    pub line: String,
    /// Which line of the lyric it is, counting from one. How the hits are
    /// ordered: a phrase somebody remembers is usually near the top.
    pub line_number: usize,
}

/// The shortest phrase that may be searched for, in folded characters.
///
/// Four. Below that almost every record in a collection matches — "yo", "and",
/// "amor" — and a result list containing everything is the same as no result
/// list, except that it took longer and looked like it worked.
pub const SHORTEST_PHRASE: usize = 4;

/// The most records a search returns.
///
/// Fifty. A phrase that matches more than fifty records is a phrase that
/// needed to be longer, and the answer to that is to say so rather than to
/// hand over a thousand rows.
pub const MOST_HITS: usize = 50;

/// Lowercase, unaccented, letters digits and single spaces.
///
/// The same folding on both sides of the comparison, which is the whole point;
/// see the module docs.
#[must_use]
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut spaced = true;
    for c in text.chars().flat_map(char::to_lowercase).map(plain) {
        if c.is_alphanumeric() {
            out.push(c);
            spaced = false;
        } else if !spaced {
            out.push(' ');
            spaced = true;
        }
    }
    // A trailing separator became a space; a phrase never ends in one.
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// A lowercase letter without its accent, where djmanzo's music has accents.
///
/// Spanish, Portuguese, French and German — the Caribbean and European
/// catalogue this is for. Deliberately a list rather than Unicode
/// decomposition: it is short, finite, and does not add a dependency to fold
/// four hundred characters nobody will type into a search box.
fn plain(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// Remember what the lyrics database said, including that it said nothing.
///
/// # Errors
/// When the row cannot be written.
pub fn remember(
    db: &Connection,
    track: &TrackId,
    plain_text: &str,
    synced: Option<&str>,
    instrumental: bool,
    source: &str,
    at: i64,
) -> Result<()> {
    let found = !plain_text.trim().is_empty() || synced.is_some();
    db.execute(
        "INSERT INTO lyrics (track_id, plain, folded, synced, found, instrumental, source, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(track_id) DO UPDATE SET
             plain = excluded.plain, folded = excluded.folded, synced = excluded.synced,
             found = excluded.found, instrumental = excluded.instrumental,
             source = excluded.source, fetched_at = excluded.fetched_at",
        params![
            track.to_hex(),
            plain_text,
            fold(plain_text),
            synced,
            i32::from(found),
            i32::from(instrumental),
            source,
            at
        ],
    )?;
    Ok(())
}

/// What is stored for one record, if anything.
///
/// # Errors
/// When the row cannot be read.
pub fn stored(db: &Connection, track: &TrackId) -> Result<Option<Stored>> {
    let mut statement = db.prepare(
        "SELECT plain, synced, found, instrumental, source, fetched_at
         FROM lyrics WHERE track_id = ?1",
    )?;
    let mut rows = statement.query(params![track.to_hex()])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(Stored {
        plain: row.get(0)?,
        synced: row.get(1)?,
        found: row.get::<_, i32>(2)? != 0,
        instrumental: row.get::<_, i32>(3)? != 0,
        source: row.get(4)?,
        fetched_at: row.get(5)?,
    }))
}

/// Records djmanzo has never asked the lyrics database about.
///
/// Returned oldest-added first and capped, so a sweep is a series of bounded
/// pieces of work rather than one that holds the whole collection in memory.
///
/// # Errors
/// When the query fails.
pub fn without_words(db: &Connection, most: usize) -> Result<Vec<TrackId>> {
    let mut statement = db.prepare(
        "SELECT t.id FROM tracks t
         LEFT JOIN lyrics l ON l.track_id = t.id
         WHERE l.track_id IS NULL
         ORDER BY t.rowid
         LIMIT ?1",
    )?;
    let ids = statement
        .query_map(params![most as i64], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;
    // A row whose id is not 64 hex digits is a corrupt record rather than a
    // track without lyrics; skipping it keeps the sweep moving.
    Ok(ids
        .iter()
        .filter_map(|hex| TrackId::from_hex(hex))
        .collect())
}

/// How many records have words, have been asked about, and exist.
///
/// # Errors
/// When the query fails.
pub fn progress(db: &Connection) -> Result<(usize, usize, usize)> {
    let asked: i64 = db.query_row("SELECT COUNT(*) FROM lyrics", [], |row| row.get(0))?;
    let with: i64 = db.query_row("SELECT COUNT(*) FROM lyrics WHERE found = 1", [], |row| {
        row.get(0)
    })?;
    let all: i64 = db.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    Ok((
        usize::try_from(with).unwrap_or(0),
        usize::try_from(asked).unwrap_or(0),
        usize::try_from(all).unwrap_or(0),
    ))
}

/// Records whose words contain `phrase`.
///
/// Returns nothing at all for a phrase shorter than [`SHORTEST_PHRASE`], which
/// is a refusal rather than a miss — see the constant.
///
/// # Errors
/// When the query fails.
pub fn search(db: &Connection, phrase: &str) -> Result<Vec<Hit>> {
    let needle = fold(phrase);
    if needle.chars().count() < SHORTEST_PHRASE {
        return Ok(Vec::new());
    }

    // No `ESCAPE` clause and no `found = 1`, and both absences are earned:
    //
    // - **Nothing is escaped** because nothing needs to be. `fold` keeps only
    //   alphanumerics and single spaces, so `%`, `_` and `\\` cannot survive
    //   into the needle. `a_wildcard_matches_only_itself` is what holds that:
    //   if folding ever started keeping them, a search for `%%%%` would match
    //   every record and that test would fail.
    // - **Records with no words are not excluded** because they cannot match.
    //   Their folded column is empty, and an empty string does not contain a
    //   four-character phrase. A `found = 1` here would be a clause that never
    //   changes an answer.
    let mut statement = db.prepare(
        "SELECT track_id, plain FROM lyrics
         WHERE folded LIKE ?1
         LIMIT ?2",
    )?;
    let pattern = format!("%{needle}%");
    let rows = statement.query_map(params![pattern, MOST_HITS as i64], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut hits: Vec<Hit> = rows
        .filter_map(|row| {
            let (id, plain_text) = row.ok()?;
            // Which line it is has to be worked out from the unfolded text,
            // because that is the text the DJ is going to read.
            let (line_number, line) = plain_text
                .lines()
                .enumerate()
                .find(|(_, line)| fold(line).contains(&needle))?;
            Some(Hit {
                track: TrackId::from_hex(&id)?,
                line: line.trim().to_owned(),
                line_number: line_number + 1,
            })
        })
        .collect();

    // Earliest line first: a phrase somebody remembers is usually the hook,
    // and the hook is usually near the top.
    hits.sort_by_key(|hit| hit.line_number);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    fn db() -> Connection {
        let mut db = Connection::open_in_memory().expect("open");
        schema::migrate(&mut db).expect("migrate");
        db
    }

    fn with_track(db: &Connection, byte: u8) -> TrackId {
        let id = TrackId::from_bytes([byte; 32]);
        db.execute(
            "INSERT INTO tracks
             (id, path, title, artist, duration_frames, sample_rate, channels, added_at)
             VALUES (?1, ?2, ?3, 'artist', 0, 44100, 2, 0)",
            params![
                id.to_hex(),
                format!("/music/{byte}.wav"),
                format!("title {byte}")
            ],
        )
        .expect("insert");
        id
    }

    /// **A phrase typed plainly finds a line written with accents and commas.**
    ///
    /// The whole feature. Between what is on the record and what somebody
    /// types are a capital, a comma and an accent, and all three have to
    /// disappear on both sides or the search finds nothing and looks broken.
    #[test]
    fn a_half_remembered_line_finds_its_record() {
        let db = db();
        let track = with_track(&db, 1);
        remember(
            &db,
            &track,
            "Son las doce de la noche\nY no puedo dormir,\nPensándote",
            None,
            false,
            "lrclib",
            0,
        )
        .expect("remember");

        for typed in [
            "no puedo dormir",
            "NO PUEDO DORMIR",
            "no  puedo, dormir!",
            "pensandote",
        ] {
            let hits = search(&db, typed).expect("search");
            assert_eq!(hits.len(), 1, "{typed:?} found nothing");
            assert_eq!(hits[0].track, track);
        }
    }

    /// **The line it found is the line as written, with its accent back.**
    #[test]
    fn the_line_is_shown_as_the_record_has_it() {
        let db = db();
        let track = with_track(&db, 1);
        remember(&db, &track, "Uno\nPensándote siempre", None, false, "x", 0).expect("remember");
        let hits = search(&db, "pensandote").expect("search");
        assert_eq!(hits[0].line, "Pensándote siempre");
        assert_eq!(hits[0].line_number, 2);
    }

    /// **The earliest line wins, because the hook is near the top.**
    #[test]
    fn hits_are_ordered_by_where_the_line_falls() {
        let db = db();
        // The one whose line is deep in the song is stored *first*, so a
        // result that came back in insertion order would be wrong and the
        // sort is the only thing that can put it right.
        let late = with_track(&db, 3);
        let early = with_track(&db, 2);
        remember(
            &db,
            &late,
            "one\ntwo\nthree\nfour\ncorazon partido",
            None,
            false,
            "x",
            0,
        )
        .expect("b");
        remember(&db, &early, "corazon partido\nfiller", None, false, "x", 0).expect("a");
        let hits = search(&db, "corazon partido").expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].track, early);
    }

    /// **A phrase too short to mean anything is refused, not answered.**
    #[test]
    fn a_word_is_not_a_phrase() {
        let db = db();
        let track = with_track(&db, 1);
        remember(&db, &track, "amor amor amor", None, false, "x", 0).expect("remember");
        assert!(search(&db, "amo").expect("search").is_empty());
        assert!(search(&db, "  a ").expect("search").is_empty());
        // Exactly the limit is a search.
        assert_eq!(search(&db, "amor").expect("search").len(), 1);
    }

    /// **Wildcards in a phrase are inert.**
    ///
    /// This is what holds the missing `ESCAPE` clause in `search` up. The
    /// defence is the folding — `%` and `_` are not alphanumeric, so they
    /// never reach the query — and this test fails the moment that stops
    /// being true: an unescaped `%%%%` matches every record in the collection,
    /// which is the worst possible answer to a search.
    #[test]
    fn a_wildcard_matches_only_itself() {
        let db = db();
        let plain_track = with_track(&db, 4);
        let odd = with_track(&db, 5);
        remember(
            &db,
            &plain_track,
            "nothing special here",
            None,
            false,
            "x",
            0,
        )
        .expect("a");
        remember(&db, &odd, "one hundred % sure", None, false, "x", 0).expect("b");

        // A pattern that would match everything if it were not escaped.
        assert!(search(&db, "%%%%").expect("search").is_empty());
        assert!(search(&db, "____").expect("search").is_empty());
        // And the record that really has one is still findable by its words.
        assert_eq!(search(&db, "hundred").expect("search").len(), 1);
    }

    /// **A miss is remembered, so the sweep does not ask twice.**
    #[test]
    fn a_record_with_no_words_is_still_answered_for() {
        let db = db();
        let track = with_track(&db, 1);
        assert_eq!(without_words(&db, 10).expect("todo"), vec![track]);

        remember(&db, &track, "", None, true, "lrclib", 99).expect("remember");
        let held = stored(&db, &track).expect("read").expect("a row");
        assert!(!held.found);
        assert!(held.instrumental);
        assert_eq!(held.fetched_at, 99);
        assert!(
            without_words(&db, 10).expect("todo").is_empty(),
            "it would be asked about again"
        );
        // And it is never a search result.
        assert!(search(&db, "anything").expect("search").is_empty());
    }

    /// **Asking again replaces rather than duplicates.**
    #[test]
    fn a_second_answer_overwrites_the_first() {
        let db = db();
        let track = with_track(&db, 1);
        remember(&db, &track, "", None, false, "lrclib", 1).expect("first");
        remember(
            &db,
            &track,
            "the words arrived later",
            None,
            false,
            "lrclib",
            2,
        )
        .expect("second");
        let held = stored(&db, &track).expect("read").expect("a row");
        assert!(held.found);
        assert_eq!(held.plain, "the words arrived later");
        let (with, asked, all) = progress(&db).expect("progress");
        assert_eq!((with, asked, all), (1, 1, 1));
    }

    #[test]
    fn progress_counts_what_is_left() {
        let db = db();
        let one = with_track(&db, 6);
        with_track(&db, 7);
        with_track(&db, 8);
        remember(&db, &one, "words", None, false, "x", 0).expect("remember");
        assert_eq!(progress(&db).expect("progress"), (1, 1, 3));
        assert_eq!(without_words(&db, 10).expect("todo").len(), 2);
        assert_eq!(without_words(&db, 1).expect("todo").len(), 1);
    }

    #[test]
    fn folding_removes_what_a_keyboard_will_not_produce() {
        assert_eq!(fold("  Y no puedo, DORMIR!  "), "y no puedo dormir");
        assert_eq!(fold("Pensándote"), "pensandote");
        assert_eq!(fold("Niña — corazón"), "nina corazon");
        assert_eq!(fold("!!!"), "");
    }
}

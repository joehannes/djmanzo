//! The database schema, as a list of migrations.
//!
//! # Why migrations rather than one `CREATE TABLE` script
//!
//! A DJ's library is the most valuable thing in the application: thousands of
//! tracks with hand-placed cues, corrected grids and years of play history. It
//! outlives every version of the code that touches it. So the schema can only
//! ever be *added to*, in numbered steps that a new binary applies to an old
//! file, and the file records how far it has got.
//!
//! Each entry in [`MIGRATIONS`] runs exactly once, in order, inside a
//! transaction. Editing one that has already shipped is not a thing that can be
//! done -- databases in the field have already run it.

use rusqlite::{Connection, Result};

/// One numbered step. The number is the version the database is at afterwards.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub sql: &'static str,
}

/// Where the schema stands now.
#[must_use]
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

/// Bring a connection up to [`latest_version`].
///
/// Idempotent: running it on an already-current database does nothing.
pub fn migrate(conn: &mut Connection) -> Result<i64> {
    // `user_version` is a four-byte field in the SQLite header. Using it rather
    // than a table of our own means the version is readable without knowing
    // anything about our schema -- including by `sqlite3` on a DJ's laptop at
    // three in the morning.
    let mut current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    for migration in MIGRATIONS {
        if migration.version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        // `PRAGMA` does not take a bound parameter, and the value is one of our
        // own constants rather than anything a user can reach.
        tx.execute_batch(&format!("PRAGMA user_version = {}", migration.version))?;
        tx.commit()?;
        current = migration.version;
        tracing::info!(version = migration.version, "applied library migration");
    }
    Ok(current)
}

/// Every migration, oldest first.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: MIGRATION_1,
    },
    Migration {
        version: 2,
        sql: MIGRATION_2,
    },
    Migration {
        version: 3,
        sql: MIGRATION_3,
    },
    Migration {
        version: 4,
        sql: MIGRATION_4,
    },
    Migration {
        version: 5,
        sql: MIGRATION_5,
    },
    Migration {
        version: 6,
        sql: MIGRATION_6,
    },
    Migration {
        version: 7,
        sql: MIGRATION_7,
    },
    Migration {
        version: 8,
        sql: MIGRATION_8,
    },
];

/// The initial schema.
///
/// Written in full rather than grown table by table, because the shape of a
/// library is not in doubt -- it is the same shape rekordbox, Serato and
/// Traktor all landed on -- and a table that exists before anything writes to
/// it costs nothing, while a migration on a 50,000-track database at a gig is
/// a risk taken for no reason.
const MIGRATION_1: &str = r#"
-- Foreign keys are off by default in SQLite, per connection, and every
-- cascade in here depends on them. Set again in `Library::open`; stated here
-- so reading the schema does not mislead.
PRAGMA foreign_keys = ON;

-- One row per distinct piece of *audio*, keyed by the hash of the decoded
-- samples. Two copies of the same track in different folders, or the same
-- recording in FLAC and in MP3-from-that-FLAC, are one row: the cues you
-- placed apply to the music, not to the file.
CREATE TABLE tracks (
    id                 TEXT PRIMARY KEY NOT NULL,
    -- Where it was last seen. Not the identity: a track that moves keeps its
    -- cues, and a missing file is a track to find rather than a track to lose.
    path               TEXT NOT NULL,
    title              TEXT,
    artist             TEXT,
    album              TEXT,
    album_artist       TEXT,
    genre              TEXT,
    label              TEXT,
    comment            TEXT,
    year               INTEGER,
    track_number       INTEGER,
    duration_frames    INTEGER NOT NULL,
    sample_rate        INTEGER NOT NULL,
    channels           INTEGER NOT NULL,
    -- Size and mtime of the file as last scanned, so a rescan can skip files
    -- that cannot have changed without opening and decoding them.
    file_size          INTEGER,
    file_modified      INTEGER,
    added_at           INTEGER NOT NULL,

    -- Analysis. Null means "not analysed", which is different from zero and
    -- shown differently.
    bpm                REAL,
    grid_anchor        REAL,
    grid_beats_per_bar INTEGER,
    grid_confidence    REAL,
    -- Camelot hour 1..12 plus mode, which is the pair the wheel is built from.
    key_hour           INTEGER,
    key_mode           TEXT,
    key_confidence     REAL,
    loudness_lufs      REAL,

    -- Performance metadata.
    play_count         INTEGER NOT NULL DEFAULT 0,
    last_played        INTEGER,
    rating             INTEGER,
    colour             TEXT
);

CREATE INDEX tracks_path      ON tracks(path);
CREATE INDEX tracks_artist    ON tracks(artist);
CREATE INDEX tracks_bpm       ON tracks(bpm);
CREATE INDEX tracks_key       ON tracks(key_hour, key_mode);
CREATE INDEX tracks_added     ON tracks(added_at);

-- Hot cues, one row per occupied slot. Absent means empty; frame zero is a
-- perfectly ordinary cue position and cannot double as "unset".
CREATE TABLE cues (
    track_id TEXT    NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    slot     INTEGER NOT NULL,
    frame    REAL    NOT NULL,
    label    TEXT,
    colour   TEXT,
    PRIMARY KEY (track_id, slot)
);

-- Saved loops, the M2 feature that was waiting for somewhere to put them.
CREATE TABLE saved_loops (
    track_id    TEXT    NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    slot        INTEGER NOT NULL,
    start_frame REAL    NOT NULL,
    end_frame   REAL    NOT NULL,
    label       TEXT,
    PRIMARY KEY (track_id, slot)
);

-- Files seen on disk but not yet identified.
--
-- A track's primary key is the hash of its decoded audio, so a file cannot
-- become a row in `tracks` until something has decoded it -- and decoding a
-- 10,000-track collection takes hours. Without this table the only honest
-- options are to make the first scan take all night before showing anything,
-- or to key tracks on their path and lose every cue the first time somebody
-- reorganises a folder.
--
-- So a scan does the cheap half immediately: walk, read tags, record what is
-- there. The collection is browsable in seconds. Identification and analysis
-- then run in the background, one file at a time, promoting rows out of here
-- into `tracks` as they finish.
CREATE TABLE pending_files (
    path          TEXT PRIMARY KEY NOT NULL,
    title         TEXT,
    artist        TEXT,
    album         TEXT,
    album_artist  TEXT,
    genre         TEXT,
    label         TEXT,
    comment       TEXT,
    year          INTEGER,
    track_number  INTEGER,
    file_size     INTEGER,
    file_modified INTEGER,
    seen_at       INTEGER NOT NULL,
    -- Set when identification failed, so a broken file is skipped on the next
    -- pass instead of being retried forever -- and so the browser can say why
    -- rather than showing a row that never resolves.
    failed_reason TEXT
);

CREATE INDEX pending_files_pending ON pending_files(failed_reason);

-- Watched music folders. Rows here are what a rescan walks.
CREATE TABLE folders (
    path     TEXT PRIMARY KEY NOT NULL,
    added_at INTEGER NOT NULL
);

-- Playlists, crates and smart folders in one tree, because to a DJ they are
-- the same gesture: a named thing in a sidebar containing tracks or other
-- named things.
CREATE TABLE playlists (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    parent_id  INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    -- 'list' holds tracks, 'folder' holds other playlists, 'smart' holds a
    -- query evaluated at read time.
    kind       TEXT    NOT NULL CHECK (kind IN ('list', 'folder', 'smart')),
    query      TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX playlists_parent ON playlists(parent_id);

-- Order matters in a playlist -- it is a set, not a bag -- so position is part
-- of the key rather than a hint.
CREATE TABLE playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    TEXT    NOT NULL REFERENCES tracks(id)    ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);

CREATE INDEX playlist_tracks_track ON playlist_tracks(track_id);

-- What was played and when. The raw material for the session export and for
-- the assistant's memory of how a room went.
CREATE TABLE history (
    id         INTEGER PRIMARY KEY,
    track_id   TEXT    NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at  INTEGER NOT NULL,
    -- Groups a night's plays without needing a sessions table yet.
    session_id TEXT
);

CREATE INDEX history_played ON history(played_at);
CREATE INDEX history_track  ON history(track_id);

-- Instant search. An external-content FTS table over `tracks`, so the text
-- lives once and the index is rebuilt from it rather than kept in step by
-- hand -- see the triggers below.
CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title, artist, album, genre, label, comment,
    content = 'tracks',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

-- The three triggers an external-content FTS5 table needs. Without them the
-- index silently stops matching new rows, which looks exactly like "search is
-- broken" and is very hard to notice in a test that only ever inserts once.
CREATE TRIGGER tracks_fts_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts(rowid, title, artist, album, genre, label, comment)
    VALUES (new.rowid, new.title, new.artist, new.album, new.genre, new.label, new.comment);
END;

CREATE TRIGGER tracks_fts_delete AFTER DELETE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, genre, label, comment)
    VALUES ('delete', old.rowid, old.title, old.artist, old.album, old.genre, old.label, old.comment);
END;

CREATE TRIGGER tracks_fts_update AFTER UPDATE ON tracks BEGIN
    INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, genre, label, comment)
    VALUES ('delete', old.rowid, old.title, old.artist, old.album, old.genre, old.label, old.comment);
    INSERT INTO tracks_fts(rowid, title, artist, album, genre, label, comment)
    VALUES (new.rowid, new.title, new.artist, new.album, new.genre, new.label, new.comment);
END;
"#;

/// What an import brings with it.
///
/// # Why importing hangs things off the pending queue
///
/// An import from rekordbox, Traktor or Serato names tracks by *path*. Our
/// identity is the hash of the decoded audio, which nothing knows until the
/// file has been decoded — so an import cannot write a `tracks` row, a `cues`
/// row or a `playlist_tracks` row at the moment it runs.
///
/// It could decode everything first, but a real collection is thousands of
/// files and hours of CPU, and a DJ who has just clicked "import" should see
/// their crates immediately rather than tomorrow.
///
/// So an import fills `pending_files` — the same queue a folder scan fills —
/// with the cues, loops and grid it found riding along as a payload, and
/// records playlist membership by path. The background identifier already
/// decodes that queue; promotion now also applies whatever the import
/// attached. The playlist *tree* is created immediately, because it needs no
/// track ids at all, so the sidebar fills in at once and the tracks appear
/// underneath as they are identified.
const MIGRATION_2: &str = r#"
-- Cues, loops and the grid an import found, as JSON, until the file behind
-- them has been identified. JSON rather than columns or a side table because
-- nothing queries it: it is opaque from the moment it is written to the moment
-- it is applied and deleted.
ALTER TABLE pending_files ADD COLUMN import_payload TEXT;

-- Which imported playlist a not-yet-identified file belongs to.
--
-- Keyed by path, like the rest of the pending machinery. Position is kept so
-- an imported set arrives in the order the DJ built it rather than the order
-- their files happen to get decoded.
CREATE TABLE pending_playlist_entries (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    path        TEXT    NOT NULL,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);

CREATE INDEX pending_playlist_entries_path ON pending_playlist_entries(path);
"#;

/// Where a beat grid came from.
///
/// # Why "is there a grid already" was not enough
///
/// Analysis, importing and hand-editing all write a grid, and the rule for
/// whether one may replace another is not the same in each direction. Without
/// knowing the source, the only expressible rules are "always overwrite" and
/// "never overwrite", and both are wrong:
///
/// - *Never* means an import brings no grids at all, because the analyser has
///   already run on everything a scan found — which makes importing from
///   rekordbox, whose grids a DJ has been playing from for years, pointless.
/// - *Always* means the next re-analysis throws away a grid the DJ corrected
///   by hand, which is the one thing that must never happen.
///
/// With a source recorded, the rule is the obvious one: a hand edit outranks an
/// import, an import outranks an analysis, and an analysis fills in a blank.
const MIGRATION_3: &str = r#"
-- 'analysis', 'import' or 'manual'. Null means no grid.
ALTER TABLE tracks ADD COLUMN grid_source TEXT;

-- Everything with a grid already got it from the analyser: importing did not
-- exist before this migration, and a hand edit could not be told apart from
-- one. Claiming otherwise would let the first re-analysis overwrite a grid a DJ
-- had corrected, so the cautious direction is the one that loses least.
UPDATE tracks SET grid_source = 'analysis' WHERE bpm IS NOT NULL;
"#;

/// Every place a track's audio has been seen.
///
/// # The gap this fills
///
/// A track is one row per distinct piece of audio, and `tracks.path` is where
/// it was last seen. That is exactly right for playing — the cues belong to the
/// music, not to the file — but it means the second copy of a track silently
/// replaces the first's path, and the DJ can never find out they have two.
/// Duplicate detection is the one job that needs the thing identity throws
/// away.
///
/// So every path is remembered here, and `tracks.path` stays what it was: the
/// one to open. A track with more than one row is a duplicate; deleting the
/// spare file removes its row on the next scan and the track carries on.
const MIGRATION_4: &str = r#"
CREATE TABLE track_paths (
    track_id  TEXT    NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    path      TEXT    NOT NULL,
    -- Unix seconds, so the browser can say which copy is the old one.
    seen_at   INTEGER NOT NULL,
    file_size INTEGER,
    PRIMARY KEY (track_id, path)
);

CREATE INDEX track_paths_path ON track_paths(path);

-- Every track already known has been seen at exactly the place it says.
INSERT OR IGNORE INTO track_paths (track_id, path, seen_at, file_size)
SELECT id, path, added_at, file_size FROM tracks;
"#;

/// Playlists the application owns rather than the DJ.
///
/// # Why the Sidelist is a playlist at all
///
/// It needs to hold tracks in an order, survive a restart, be added to from
/// the browser and be loaded to a deck. `playlists` already does every one of
/// those, and a parallel table would be the same code twice with a different
/// name on it — differing, eventually, in some detail nobody meant.
///
/// What it must *not* do is appear in the crate tree beside the folders a DJ
/// made. So a row can be marked as belonging to the application, and the tree
/// leaves those out. A nullable column rather than a new `kind`, because
/// SQLite cannot alter a `CHECK` constraint without rebuilding the table — and
/// because "which panel owns this" is a different question from "what does it
/// contain".
const MIGRATION_5: &str = r#"
-- Null for a playlist the DJ made. Otherwise names the panel it belongs to:
-- 'sidelist' today, and whatever the Automix queue turns out to need.
ALTER TABLE playlists ADD COLUMN system TEXT;

CREATE INDEX playlists_system ON playlists(system);
"#;

const MIGRATION_6: &str = r#"
-- Phrase structure: how many beats a phrase is, and which beat starts one.
--
-- Both or neither. A length without the beat it starts on is not half an
-- answer, it is a marker in an unknown place. Null throughout for a track with
-- no phrase structure, which is a real answer and not a missing one -- plenty
-- of live and ambient records have none.
ALTER TABLE tracks ADD COLUMN phrase_beats      INTEGER;
ALTER TABLE tracks ADD COLUMN phrase_anchor     INTEGER;
ALTER TABLE tracks ADD COLUMN phrase_confidence REAL;
"#;

/// Notes taken during a set.
///
/// # Why a note belongs to a moment and not to a track
///
/// "The floor emptied here" is not a fact about a record. The same record works
/// at midnight and clears the room at two, and a note filed against the track
/// would say the wrong thing on both nights. What a DJ wants back is *the
/// night*, in order, with what they thought at the time sitting between the
/// records.
///
/// # Why what was playing is copied in rather than joined
///
/// A note is an observation made at a time, like a caption under a photograph:
/// it should not change when the library does. Joining against `history` by
/// timestamp would be fragile in exactly the cases that matter — a note written
/// forty seconds into a record, or mid-transition with two of them up, or with
/// nothing playing at all.
///
/// And a foreign key would be worse than fragile. Removing a track would
/// cascade away the note about the night it was played, which has the ownership
/// backwards: the note is the DJ's, not the record's.
///
/// # Why the body may be empty
///
/// Because the moment is the part that cannot be recovered later and the words
/// are the part that can. In a booth the useful gesture is *mark this now,
/// write it afterwards* — the same thing a field recorder's marker button does,
/// for the same reason. A row with an empty body is a marker, and it is a
/// complete row rather than a half-finished one.
///
/// # Why webcam stills and audience reactions are not in here
///
/// They were considered — ROADMAP's M9 groups them with this under one
/// timeline. But neither fits a text body, and guessing now at what a still or
/// a reaction needs would produce columns that are wrong when they arrive.
/// They get their own tables and this one keeps its name.
const MIGRATION_7: &str = r#"
CREATE TABLE notes (
    id         INTEGER PRIMARY KEY,
    -- The run of the application this belongs to; the same value `history`
    -- uses, so the two can be put on one timeline.
    session_id TEXT    NOT NULL,
    -- Unix seconds, on the same clock as history.played_at.
    at         INTEGER NOT NULL,
    -- What the DJ typed. Empty for a marker not yet written up.
    body       TEXT    NOT NULL,
    -- What was playing when the moment was marked, as it read at the time.
    -- Deliberately not a reference; see above.
    playing    TEXT    NOT NULL
);

CREATE INDEX notes_session ON notes(session_id, at);
"#;

/// Words, so a half-remembered line can find a record.
///
/// # Why the words are stored twice
///
/// Once as they are, for reading, and once folded — lowercase, unaccented,
/// punctuation gone, whitespace collapsed — for searching. A DJ types "no
/// puedo dormir" and the record says "Y no puedo dormir," with a comma and a
/// capital; a search that does not fold finds nothing and looks broken. The
/// folded copy is roughly the same size as the original, which is a few
/// kilobytes per track, and it is the difference between the feature working
/// and not.
///
/// # Why a miss is a row
///
/// `found` is false for a record the database has no words for. Without it,
/// every sweep of the collection asks about the same instrumentals and the
/// same obscure edits again, forever. A row that says "asked, nothing there"
/// costs a few bytes and saves a request a night.
///
/// # Why there is no full-text index
///
/// SQLite's FTS5 would need a feature flag on `rusqlite` and a second copy of
/// the text in its own tables. A `LIKE` over a folded column reads a few
/// megabytes for a large personal collection, which is milliseconds, and the
/// question being asked — "which of my records contains this phrase" — is one
/// a scan answers exactly. If a collection ever gets large enough for that to
/// hurt, the index goes in then, against a measurement.
const MIGRATION_8: &str = r#"
CREATE TABLE lyrics (
    track_id   TEXT    PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    -- The words as the database gave them. Empty when `found` is 0.
    plain      TEXT    NOT NULL,
    -- The same words, folded for searching. See above.
    folded     TEXT    NOT NULL,
    -- With timestamps, when the database had them. NULL otherwise.
    synced     TEXT,
    -- 1 when the database answered with words, 0 when it answered "nothing
    -- here". Both are answers; only a failure to reach it leaves no row.
    found      INTEGER NOT NULL DEFAULT 0,
    -- 1 when the database says this recording has no words at all.
    instrumental INTEGER NOT NULL DEFAULT 0,
    -- Where they came from, so a later source can be told apart.
    source     TEXT    NOT NULL,
    -- Unix seconds, so a sweep can re-ask about old misses without re-asking
    -- about recent ones.
    fetched_at INTEGER NOT NULL
);

CREATE INDEX lyrics_found ON lyrics(found);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn migrating_reaches_the_latest_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&mut conn).unwrap(), latest_version());
    }

    /// The property the whole scheme rests on: an old database opened by a new
    /// binary is brought forward, and a current one is left alone.
    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        // A second run must not re-execute a `CREATE TABLE`, which would error.
        assert_eq!(migrate(&mut conn).unwrap(), latest_version());
    }

    /// The migration path itself, which only matters once there is more than
    /// one: a database created at version 1 must reach the latest without
    /// losing what was in it.
    #[test]
    fn an_older_database_is_brought_forward_with_its_rows_intact() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Stop after the first migration, as a database written by an older
        // build would be.
        let first = MIGRATIONS[0];
        conn.execute_batch(first.sql).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {}", first.version))
            .unwrap();
        conn.execute(
            "INSERT INTO pending_files (path, seen_at) VALUES ('/music/a.flac', 1)",
            [],
        )
        .unwrap();

        assert_eq!(migrate(&mut conn).unwrap(), latest_version());
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM pending_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the row from the older schema must survive");
        // And the column the newer migration added is there.
        conn.query_row("SELECT import_payload FROM pending_files", [], |row| {
            row.get::<_, Option<String>>(0)
        })
        .unwrap();
    }

    #[test]
    fn versions_are_unique_and_ascending() {
        let mut previous = 0;
        for migration in MIGRATIONS {
            assert!(
                migration.version > previous,
                "migration {} is out of order; versions are the order they run in",
                migration.version
            );
            previous = migration.version;
        }
    }

    #[test]
    fn every_table_the_library_needs_exists() {
        let conn = migrated();
        for table in [
            "tracks",
            "cues",
            "saved_loops",
            "folders",
            "playlists",
            "playlist_tracks",
            "history",
            "pending_files",
            "pending_playlist_entries",
            "track_paths",
            "tracks_fts",
            "notes",
        ] {
            let found: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "{table} is missing from the schema");
        }
    }
}

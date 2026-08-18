//! Reading and writing the library.

use crate::playlist::{PlayRecord, Playlist, PlaylistKind};
use crate::record::{LibraryTrack, PlayStats, StoredAnalysis, StoredCue, StoredLoop, Tags};
use crate::schema;
use dj_core::{Mode, SampleRate, TrackId};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library database: {0}")]
    Sql(#[from] rusqlite::Error),
    /// The mutex guarding the connection was poisoned by a panic elsewhere.
    #[error("the library is unavailable because another operation panicked")]
    Poisoned,
    /// A row in the database does not describe a track. Reported rather than
    /// papered over: a bad row is a bug or a corrupted file, and both are worth
    /// hearing about.
    #[error("library row {id} is malformed: {reason}")]
    BadRow { id: String, reason: &'static str },
    #[error("there is no playlist {0}")]
    NoSuchPlaylist(i64),
    #[error("playlist {0} is not a folder, so nothing can go inside it")]
    NotAFolder(i64),
    /// Moving a node inside itself, or inside something below it. Refused
    /// rather than performed: the branch would still exist but nothing in the
    /// sidebar could reach it.
    #[error("that would put playlist {0} inside itself")]
    WouldOrphan(i64),
}

type Result<T> = std::result::Result<T, LibraryError>;

/// The library database.
///
/// One connection behind a mutex rather than a pool. SQLite serialises writes
/// anyway, the library is read by a browser and written by a scan — neither at
/// audio rate — and a pool would add a dependency and a failure mode to solve
/// a problem this application does not have.
#[derive(Debug)]
pub struct Library {
    conn: Mutex<Connection>,
}

impl Library {
    /// Open (or create) a library at `path`, bringing the schema up to date.
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        Self::prepare(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory library, for tests and for a first run with nowhere to
    /// write. Not a fallback that hides a problem: the caller decides.
    pub fn in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::prepare(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn prepare(conn: &mut Connection) -> Result<()> {
        // Foreign keys are per-connection and off by default; every cascade in
        // the schema depends on them being on.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // Write-ahead logging: a scan writing thousands of rows must not block
        // the browser reading them, and a power cut mid-scan must not take the
        // library with it.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // `NORMAL` rather than `FULL`: one fsync per checkpoint instead of one
        // per transaction. With WAL that is still crash-safe; it is only unsafe
        // against the machine losing power mid-write, which would cost at most
        // the last few scanned tracks.
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        schema::migrate(conn)?;
        Ok(())
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        f(&conn)
    }

    /// Insert a track, or update it if the same audio is already known.
    ///
    /// Keyed by content hash, so re-scanning the same file is idempotent and
    /// finding a second copy of a track updates the path rather than making a
    /// duplicate row.
    ///
    /// Analysis and play statistics are *not* overwritten here. A rescan reads
    /// tags off disk; it has no business throwing away a grid the DJ corrected
    /// or a play count earned over a year. Use [`Self::set_analysis`] for that.
    pub fn upsert_track(&self, track: &LibraryTrack) -> Result<()> {
        self.with(|conn| upsert_track_on(conn, track))
    }

    /// Analysis for a track that has none.
    ///
    /// Conditional, not unconditional: the caller is identification, which is
    /// the one moment fresh analysis exists and the row is usually new. If the
    /// track *has* been analysed, that result may be a grid the DJ corrected by
    /// hand -- and a second copy of the same audio turning up in another folder
    /// must not quietly replace it with the analyser's guess.
    pub fn set_analysis_if_absent(&self, id: TrackId, analysis: &StoredAnalysis) -> Result<bool> {
        self.with(|conn| set_analysis_if_absent_on(conn, id, analysis))
    }

    pub fn track(&self, id: TrackId) -> Result<Option<LibraryTrack>> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id = ?1"),
                [id.to_hex()],
                read_track,
            )
            .optional()?
            .transpose()
        })
    }

    /// Look a track up by where it lives.
    ///
    /// Used by a scan to decide whether a file has to be decoded at all: if the
    /// path is known and the size and modification time match, the audio cannot
    /// have changed, and decoding a hundred-megabyte FLAC to learn that would
    /// make a rescan take hours instead of seconds.
    pub fn track_at_path(&self, path: &Path) -> Result<Option<LibraryTrack>> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE path = ?1"),
                [path.to_string_lossy()],
                read_track,
            )
            .optional()?
            .transpose()
        })
    }

    pub fn track_count(&self) -> Result<i64> {
        self.with(|conn| Ok(conn.query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))?))
    }

    /// Replace a track's analysis.
    pub fn set_analysis(&self, id: TrackId, analysis: &StoredAnalysis) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE tracks SET
                     bpm = ?2, grid_anchor = ?3, grid_beats_per_bar = ?4,
                     grid_confidence = ?5, key_hour = ?6, key_mode = ?7,
                     key_confidence = ?8, loudness_lufs = ?9
                 WHERE id = ?1",
                params![
                    id.to_hex(),
                    analysis.bpm,
                    analysis.grid_anchor,
                    analysis.grid_beats_per_bar,
                    analysis.grid_confidence,
                    analysis.key_hour,
                    analysis.key_mode.map(mode_to_sql),
                    analysis.key_confidence,
                    analysis.loudness_lufs,
                ],
            )?;
            Ok(())
        })
    }

    /// Record a play: bump the count, stamp the time, and append to history.
    pub fn record_play(&self, id: TrackId, at: i64, session: Option<&str>) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1",
                params![id.to_hex(), at],
            )?;
            conn.execute(
                "INSERT INTO history (track_id, played_at, session_id) VALUES (?1, ?2, ?3)",
                params![id.to_hex(), at, session],
            )?;
            Ok(())
        })
    }

    // -- cues and loops ----------------------------------------------------

    /// Replace a track's hot cues wholesale.
    ///
    /// Wholesale rather than per-slot because that is how the deck holds them:
    /// eight slots, some empty. Writing the set means a cleared cue is actually
    /// cleared, which a series of upserts would quietly not do.
    pub fn set_cues(&self, id: TrackId, cues: &[StoredCue]) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM cues WHERE track_id = ?1", [id.to_hex()])?;
        for cue in cues {
            tx.execute(
                "INSERT INTO cues (track_id, slot, frame, label, colour)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id.to_hex(), cue.slot, cue.frame, cue.label, cue.colour],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cues(&self, id: TrackId) -> Result<Vec<StoredCue>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT slot, frame, label, colour FROM cues
                 WHERE track_id = ?1 ORDER BY slot",
            )?;
            let rows = stmt.query_map([id.to_hex()], |row| {
                Ok(StoredCue {
                    slot: row.get(0)?,
                    frame: row.get(1)?,
                    label: row.get(2)?,
                    colour: row.get(3)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Replace a track's saved loops. Wholesale, for the same reason as cues.
    pub fn set_loops(&self, id: TrackId, loops: &[StoredLoop]) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM saved_loops WHERE track_id = ?1", [id.to_hex()])?;
        for region in loops {
            tx.execute(
                "INSERT INTO saved_loops (track_id, slot, start_frame, end_frame, label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_hex(),
                    region.slot,
                    region.start_frame,
                    region.end_frame,
                    region.label
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn loops(&self, id: TrackId) -> Result<Vec<StoredLoop>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT slot, start_frame, end_frame, label FROM saved_loops
                 WHERE track_id = ?1 ORDER BY slot",
            )?;
            let rows = stmt.query_map([id.to_hex()], |row| {
                Ok(StoredLoop {
                    slot: row.get(0)?,
                    start_frame: row.get(1)?,
                    end_frame: row.get(2)?,
                    label: row.get(3)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    // -- watched folders ---------------------------------------------------

    pub fn add_folder(&self, path: &Path, at: i64) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO folders (path, added_at) VALUES (?1, ?2)
                 ON CONFLICT(path) DO NOTHING",
                params![path.to_string_lossy(), at],
            )?;
            Ok(())
        })
    }

    pub fn remove_folder(&self, path: &Path) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "DELETE FROM folders WHERE path = ?1",
                [path.to_string_lossy()],
            )?;
            Ok(())
        })
    }

    pub fn folders(&self) -> Result<Vec<PathBuf>> {
        self.with(|conn| {
            let mut stmt = conn.prepare("SELECT path FROM folders ORDER BY path")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .map(PathBuf::from)
                .collect())
        })
    }

    // -- pending files -----------------------------------------------------

    /// True when this file is already known and cannot have changed.
    ///
    /// Size and modification time, not a hash: the point is to answer without
    /// opening the file. It can be fooled by a file edited in place within the
    /// same second and to the same length, which is not a thing that happens to
    /// audio; the cost of being wrong is a stale tag, and the cost of *not*
    /// doing it is decoding a whole collection on every scan.
    ///
    /// A file with no size or timestamp is never unchanged, because there is
    /// nothing to compare.
    pub fn file_is_unchanged(
        &self,
        path: &Path,
        size: Option<u64>,
        modified: Option<i64>,
    ) -> Result<bool> {
        let (Some(size), Some(modified)) = (size, modified) else {
            return Ok(false);
        };
        self.with(|conn| {
            // Either table can hold the answer: a file already promoted into
            // `tracks` must not be rescanned either.
            let known: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM pending_files
                     WHERE path = ?1 AND file_size = ?2 AND file_modified = ?3
                     UNION ALL
                     SELECT 1 FROM tracks
                     WHERE path = ?1 AND file_size = ?2 AND file_modified = ?3
                     LIMIT 1",
                    params![path.to_string_lossy(), size as i64, modified],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(known.is_some())
        })
    }

    /// Record a file the scan found, before anything has identified it.
    pub fn record_pending(&self, file: &crate::scan::ScannedFile, now: i64) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO pending_files (
                     path, title, artist, album, album_artist, genre, label,
                     comment, year, track_number, file_size, file_modified, seen_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(path) DO UPDATE SET
                     title = excluded.title,
                     artist = excluded.artist,
                     album = excluded.album,
                     album_artist = excluded.album_artist,
                     genre = excluded.genre,
                     label = excluded.label,
                     comment = excluded.comment,
                     year = excluded.year,
                     track_number = excluded.track_number,
                     file_size = excluded.file_size,
                     file_modified = excluded.file_modified,
                     seen_at = excluded.seen_at,
                     -- The file changed, so whatever went wrong last time is
                     -- worth trying again.
                     failed_reason = NULL",
                params![
                    file.path.to_string_lossy(),
                    file.tags.title,
                    file.tags.artist,
                    file.tags.album,
                    file.tags.album_artist,
                    file.tags.genre,
                    file.tags.label,
                    file.tags.comment,
                    file.tags.year,
                    file.tags.track_number,
                    file.file_size.map(|s| s as i64),
                    file.file_modified,
                    now,
                ],
            )?;
            Ok(())
        })
    }

    /// How many files are waiting to be identified. What a progress bar counts.
    pub fn pending_count(&self) -> Result<i64> {
        self.with(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM pending_files WHERE failed_reason IS NULL",
                [],
                |r| r.get(0),
            )?)
        })
    }

    /// The next files to identify, oldest first.
    ///
    /// Oldest first so a scan that is interrupted and resumed makes progress
    /// through the collection rather than starting over at the same file.
    pub fn next_pending(&self, limit: usize) -> Result<Vec<crate::scan::ScannedFile>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT path, title, artist, album, album_artist, genre, label,
                        comment, year, track_number, file_size, file_modified
                 FROM pending_files
                 WHERE failed_reason IS NULL
                 ORDER BY seen_at, path
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |row| {
                Ok(crate::scan::ScannedFile {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    tags: Tags {
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        album: row.get(3)?,
                        album_artist: row.get(4)?,
                        genre: row.get(5)?,
                        label: row.get(6)?,
                        comment: row.get(7)?,
                        year: row.get(8)?,
                        track_number: row.get(9)?,
                    },
                    file_size: row.get::<_, Option<i64>>(10)?.map(|s| s.max(0) as u64),
                    file_modified: row.get(11)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// A file has been decoded: turn it into a track and stop tracking it as
    /// pending.
    ///
    /// One transaction, because a crash between the two halves would either
    /// lose the track or leave it queued forever.
    pub fn promote_pending(&self, track: &LibraryTrack) -> Result<()> {
        let path = track.path.to_string_lossy().into_owned();
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;

        upsert_track_on(&tx, track)?;
        // `upsert_track` deliberately never touches the analysis columns, so
        // that a rescan reading tags off disk cannot erase a grid the DJ
        // corrected. Identification is the exception: it is the one caller that
        // has just analysed the audio, and without this the library would fill
        // up with tracks that never get a BPM.
        set_analysis_if_absent_on(&tx, track.id, &track.analysis)?;
        tx.execute("DELETE FROM pending_files WHERE path = ?1", [path])?;

        tx.commit()?;
        Ok(())
    }

    /// Identification failed. Recorded rather than retried, so a corrupt file
    /// does not sit at the head of the queue forever, and the browser can say
    /// what went wrong instead of showing a row that never resolves.
    pub fn mark_pending_failed(&self, path: &Path, reason: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE pending_files SET failed_reason = ?2 WHERE path = ?1",
                params![path.to_string_lossy(), reason],
            )?;
            Ok(())
        })
    }

    /// Files that could not be identified, with the reason.
    pub fn failed_pending(&self) -> Result<Vec<(PathBuf, String)>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT path, failed_reason FROM pending_files
                 WHERE failed_reason IS NOT NULL ORDER BY path",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((PathBuf::from(row.get::<_, String>(0)?), row.get(1)?))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    // -- playlists ---------------------------------------------------------

    /// Make a playlist, folder or smart folder.
    ///
    /// Returns the new node's id. A parent that is not a folder is refused:
    /// putting a playlist inside a playlist is not a thing, and allowing it
    /// would make the sidebar undrawable.
    pub fn create_playlist(
        &self,
        name: &str,
        parent: Option<i64>,
        kind: PlaylistKind,
        query: Option<&str>,
        now: i64,
    ) -> Result<i64> {
        if let Some(parent) = parent {
            self.require_folder(parent)?;
        }
        self.with(|conn| {
            conn.execute(
                "INSERT INTO playlists (name, parent_id, kind, query, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, parent, kind.as_sql(), query, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    fn require_folder(&self, id: i64) -> Result<()> {
        let kind = self.with(|conn| {
            Ok(conn
                .query_row("SELECT kind FROM playlists WHERE id = ?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?)
        })?;
        match kind.as_deref().and_then(PlaylistKind::from_sql) {
            Some(kind) if kind.is_container() => Ok(()),
            Some(_) => Err(LibraryError::NotAFolder(id)),
            None => Err(LibraryError::NoSuchPlaylist(id)),
        }
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE playlists SET name = ?2 WHERE id = ?1",
                params![id, name],
            )?;
            Ok(())
        })
    }

    /// Delete a node, and everything under it.
    ///
    /// The cascade is the schema's, and it is what a DJ means by deleting a
    /// folder. Tracks are untouched: a playlist holds references, and throwing
    /// one away must never take the music with it.
    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Move a node under a different parent, or to the top level.
    ///
    /// Refuses to put a node inside itself or inside its own descendant, which
    /// would detach that whole branch from the tree and leave it unreachable in
    /// the sidebar while still occupying rows.
    pub fn move_playlist(&self, id: i64, new_parent: Option<i64>) -> Result<()> {
        if let Some(parent) = new_parent {
            if parent == id {
                return Err(LibraryError::WouldOrphan(id));
            }
            self.require_folder(parent)?;
            if self.ancestors(parent)?.contains(&id) {
                return Err(LibraryError::WouldOrphan(id));
            }
        }
        self.with(|conn| {
            conn.execute(
                "UPDATE playlists SET parent_id = ?2 WHERE id = ?1",
                params![id, new_parent],
            )?;
            Ok(())
        })
    }

    /// Every node above `id`, nearest first.
    fn ancestors(&self, id: i64) -> Result<Vec<i64>> {
        let mut seen = Vec::new();
        let mut current = Some(id);
        // Bounded by the number of nodes, so a cycle written by something else
        // cannot hang this.
        while let Some(node) = current {
            if seen.contains(&node) {
                break;
            }
            seen.push(node);
            current = self.with(|conn| {
                Ok(conn
                    .query_row(
                        "SELECT parent_id FROM playlists WHERE id = ?1",
                        [node],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()?
                    .flatten())
            })?;
        }
        Ok(seen)
    }

    /// Every node, with its track count.
    ///
    /// Flat and in one query. The sidebar builds the tree; doing it here would
    /// mean one query per level, which on a DJ's crate structure is dozens.
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id, p.name, p.parent_id, p.kind, p.query, p.created_at,
                        (SELECT count(*) FROM playlist_tracks t WHERE t.playlist_id = p.id)
                 FROM playlists p
                 ORDER BY p.name COLLATE NOCASE",
            )?;
            let rows = stmt.query_map([], |row| {
                let kind: String = row.get(3)?;
                Ok(Playlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    kind: PlaylistKind::from_sql(&kind).unwrap_or(PlaylistKind::List),
                    query: row.get(4)?,
                    created_at: row.get(5)?,
                    track_count: row.get(6)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Append a track to a playlist.
    ///
    /// Appends rather than deduplicating: a track appearing twice in a set is
    /// something DJs do on purpose, and silently refusing the second would look
    /// like the drag missed.
    pub fn add_to_playlist(&self, playlist: i64, track: TrackId) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (
                     ?1, ?2,
                     COALESCE((SELECT max(position) FROM playlist_tracks WHERE playlist_id = ?1), -1) + 1
                 )",
                params![playlist, track.to_hex()],
            )?;
            Ok(())
        })
    }

    /// Take one entry out, by its position.
    ///
    /// By position rather than by track, because the same track can be in a
    /// playlist twice and removing "the track" would be ambiguous. Leaves a gap
    /// in the numbering, which nothing depends on.
    pub fn remove_from_playlist(&self, playlist: i64, position: i64) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
                params![playlist, position],
            )?;
            Ok(())
        })
    }

    /// Rewrite a playlist's order.
    ///
    /// Takes the positions in their new order, renumbering from zero. Whole-list
    /// because a drag can move an entry anywhere and the arithmetic for "shift
    /// everything between" is where off-by-ones live; a playlist is hundreds of
    /// rows at most, inside one transaction.
    pub fn reorder_playlist(&self, playlist: i64, order: &[i64]) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;

        // Read first: the caller names positions, and after the first update
        // those positions no longer mean what they did.
        let entries: Vec<(i64, String)> = {
            let mut stmt = tx
                .prepare("SELECT position, track_id FROM playlist_tracks WHERE playlist_id = ?1")?;
            let rows = stmt.query_map([playlist], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist],
        )?;
        for (index, position) in order.iter().enumerate() {
            // A position the caller named that is not in the list is skipped
            // rather than failing the reorder: the alternative is losing the
            // whole ordering because one row moved underneath a drag.
            if let Some((_, track)) = entries.iter().find(|(p, _)| p == position) {
                tx.execute(
                    "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![playlist, track, index as i64],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// A playlist's tracks, in order, with their position.
    pub fn playlist_tracks(&self, playlist: i64) -> Result<Vec<(i64, LibraryTrack)>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT playlist_tracks.position, {TRACK_COLUMNS_QUALIFIED}
                 FROM playlist_tracks
                 JOIN tracks ON tracks.id = playlist_tracks.track_id
                 WHERE playlist_tracks.playlist_id = ?1
                 ORDER BY playlist_tracks.position"
            ))?;
            let rows = stmt.query_map([playlist], |row| {
                let position: i64 = row.get(0)?;
                // The track columns start one to the right of the position.
                Ok(read_track_from(row, 1)?.map(|track| (position, track)))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .collect()
        })
    }

    // -- history -----------------------------------------------------------

    /// What was played, most recent first.
    pub fn history(&self, limit: usize) -> Result<Vec<PlayRecord>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT history.track_id, tracks.title, tracks.artist, tracks.path,
                        history.played_at, history.session_id
                 FROM history
                 JOIN tracks ON tracks.id = history.track_id
                 ORDER BY history.played_at DESC, history.id DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |row| {
                let title: Option<String> = row.get(1)?;
                let artist: Option<String> = row.get(2)?;
                let path: String = row.get(3)?;
                Ok(PlayRecord {
                    track_id: row.get(0)?,
                    // The same fallback the browser uses, applied here so a
                    // history row and a browser row never disagree about what a
                    // track is called.
                    title: title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
                        std::path::Path::new(&path).file_stem().map_or_else(
                            || "Untitled".to_owned(),
                            |s| s.to_string_lossy().into_owned(),
                        )
                    }),
                    artist: artist
                        .filter(|a| !a.trim().is_empty())
                        .unwrap_or_else(|| "Unknown artist".to_owned()),
                    played_at: row.get(4)?,
                    session_id: row.get(5)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    // -- search ------------------------------------------------------------

    /// Free-text search across the tags.
    ///
    /// The query is turned into a prefix match on every word, which is what
    /// makes it feel instant while typing: "bach ros" finds "Bachata Rosa"
    /// after four keystrokes rather than after the whole title.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<LibraryTrack>> {
        let Some(fts) = prefix_query(query) else {
            return Ok(Vec::new());
        };
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM tracks
                 JOIN tracks_fts ON tracks_fts.rowid = tracks.rowid
                 WHERE tracks_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
                TRACK_COLUMNS_QUALIFIED
            ))?;
            let rows = stmt.query_map(params![fts, limit as i64], read_track)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .collect()
        })
    }

    /// Every track, newest first. The browser's default view.
    pub fn all_tracks(&self, limit: usize) -> Result<Vec<LibraryTrack>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {TRACK_COLUMNS} FROM tracks ORDER BY added_at DESC, rowid DESC LIMIT ?1"
            ))?;
            let rows = stmt.query_map([limit as i64], read_track)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .collect()
        })
    }
}

/// The upsert itself, against whatever connection or transaction is handed in.
///
/// Free rather than a method so [`Library::promote_pending`] can run it inside
/// its own transaction: a promotion writes three things and must not be able to
/// leave two of them behind.
///
/// Analysis and play statistics are absent from the column list on purpose. A
/// rescan reads tags off disk; it has no business throwing away a grid the DJ
/// corrected or a play count earned over a year.
fn upsert_track_on(conn: &Connection, track: &LibraryTrack) -> Result<()> {
    conn.execute(
        "INSERT INTO tracks (
             id, path, title, artist, album, album_artist, genre, label,
             comment, year, track_number, duration_frames, sample_rate,
             channels, file_size, file_modified, added_at
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
             ?14, ?15, ?16, ?17
         )
         ON CONFLICT(id) DO UPDATE SET
             path = excluded.path,
             title = excluded.title,
             artist = excluded.artist,
             album = excluded.album,
             album_artist = excluded.album_artist,
             genre = excluded.genre,
             label = excluded.label,
             comment = excluded.comment,
             year = excluded.year,
             track_number = excluded.track_number,
             duration_frames = excluded.duration_frames,
             sample_rate = excluded.sample_rate,
             channels = excluded.channels,
             file_size = excluded.file_size,
             file_modified = excluded.file_modified",
        params![
            track.id.to_hex(),
            track.path.to_string_lossy(),
            track.tags.title,
            track.tags.artist,
            track.tags.album,
            track.tags.album_artist,
            track.tags.genre,
            track.tags.label,
            track.tags.comment,
            track.tags.year,
            track.tags.track_number,
            track.duration_frames as i64,
            track.sample_rate.get(),
            track.channels,
            track.file_size.map(|s| s as i64),
            track.file_modified,
            track.added_at,
        ],
    )?;
    Ok(())
}

/// Write analysis only where there is none. Returns whether it wrote.
///
/// "None" means every analysed column is null. A partially analysed row -- a
/// tempo but no key, say -- counts as analysed and is left alone: the analyser
/// having found only half the answer is still a result, and one the DJ may have
/// since corrected.
fn set_analysis_if_absent_on(
    conn: &Connection,
    id: TrackId,
    analysis: &StoredAnalysis,
) -> Result<bool> {
    let written = conn.execute(
        "UPDATE tracks SET
             bpm = ?2, grid_anchor = ?3, grid_beats_per_bar = ?4,
             grid_confidence = ?5, key_hour = ?6, key_mode = ?7,
             key_confidence = ?8, loudness_lufs = ?9
         WHERE id = ?1
           AND bpm IS NULL
           AND grid_anchor IS NULL
           AND key_hour IS NULL
           AND loudness_lufs IS NULL",
        params![
            id.to_hex(),
            analysis.bpm,
            analysis.grid_anchor,
            analysis.grid_beats_per_bar,
            analysis.grid_confidence,
            analysis.key_hour,
            analysis.key_mode.map(mode_to_sql),
            analysis.key_confidence,
            analysis.loudness_lufs,
        ],
    )?;
    Ok(written > 0)
}

/// Turn what somebody typed into an FTS5 prefix query.
///
/// Every non-alphanumeric character is dropped rather than escaped. FTS5's
/// query language treats `"`, `*`, `:`, `^`, `-` and `(` as syntax, so a DJ
/// typing an artist called `AC/DC` or a title with a quote in it would get a
/// syntax error instead of results. Nothing in a search box is meant as an
/// operator.
fn prefix_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("{word}*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// The column list, in the order [`read_track`] reads them.
const TRACK_COLUMNS: &str = "id, path, title, artist, album, album_artist, genre, label, comment, \
     year, track_number, duration_frames, sample_rate, channels, file_size, \
     file_modified, added_at, bpm, grid_anchor, grid_beats_per_bar, \
     grid_confidence, key_hour, key_mode, key_confidence, loudness_lufs, \
     play_count, last_played, rating";

/// The same list, qualified — needed wherever the query joins another table
/// that has columns of the same name.
const TRACK_COLUMNS_QUALIFIED: &str = "tracks.id, tracks.path, tracks.title, tracks.artist, \
     tracks.album, tracks.album_artist, tracks.genre, tracks.label, tracks.comment, \
     tracks.year, tracks.track_number, tracks.duration_frames, tracks.sample_rate, \
     tracks.channels, tracks.file_size, tracks.file_modified, tracks.added_at, \
     tracks.bpm, tracks.grid_anchor, tracks.grid_beats_per_bar, \
     tracks.grid_confidence, tracks.key_hour, tracks.key_mode, tracks.key_confidence, \
     tracks.loudness_lufs, tracks.play_count, tracks.last_played, tracks.rating";

/// Read one row.
///
/// Returns a nested `Result` on purpose. The outer one is rusqlite's — the row
/// could not be read at all — and the inner is ours: the row was read and does
/// not describe a track. Collapsing them would mean reporting a corrupt hex id
/// as a database error, which sends whoever is debugging it to the wrong place.
fn read_track(row: &Row<'_>) -> rusqlite::Result<Result<LibraryTrack>> {
    read_track_from(row, 0)
}

/// The same, starting at column `base`.
///
/// The playlist join puts `position` in front of the track columns, and a
/// second copy of this function that differed only by seventeen index literals
/// would be the kind of duplication that goes wrong the next time a column is
/// added.
fn read_track_from(row: &Row<'_>, base: usize) -> rusqlite::Result<Result<LibraryTrack>> {
    let at = |offset: usize| base + offset;
    let hex: String = row.get(at(0))?;
    let path: String = row.get(at(1))?;
    let sample_rate: u32 = row.get(at(12))?;
    let key_mode: Option<String> = row.get(at(22))?;

    let Some(id) = track_id_from_hex(&hex) else {
        return Ok(Err(LibraryError::BadRow {
            id: hex,
            reason: "id is not 64 hex characters",
        }));
    };
    let Some(sample_rate) = SampleRate::new(sample_rate) else {
        return Ok(Err(LibraryError::BadRow {
            id: hex,
            reason: "sample rate is not one a deck can play",
        }));
    };

    Ok(Ok(LibraryTrack {
        id,
        path: PathBuf::from(path),
        tags: Tags {
            title: row.get(at(2))?,
            artist: row.get(at(3))?,
            album: row.get(at(4))?,
            album_artist: row.get(at(5))?,
            genre: row.get(at(6))?,
            label: row.get(at(7))?,
            comment: row.get(at(8))?,
            year: row.get(at(9))?,
            track_number: row.get(at(10))?,
        },
        duration_frames: row.get::<_, i64>(at(11))?.max(0) as u64,
        sample_rate,
        channels: row.get(at(13))?,
        file_size: row.get::<_, Option<i64>>(at(14))?.map(|s| s.max(0) as u64),
        file_modified: row.get(at(15))?,
        added_at: row.get(at(16))?,
        analysis: StoredAnalysis {
            bpm: row.get(at(17))?,
            grid_anchor: row.get(at(18))?,
            grid_beats_per_bar: row.get(at(19))?,
            grid_confidence: row.get(at(20))?,
            key_hour: row.get(at(21))?,
            key_mode: key_mode.as_deref().and_then(mode_from_sql),
            key_confidence: row.get(at(23))?,
            loudness_lufs: row.get(at(24))?,
        },
        stats: PlayStats {
            play_count: row.get(at(25))?,
            last_played: row.get(at(26))?,
            rating: row.get(at(27))?,
        },
    }))
}

fn track_id_from_hex(hex: &str) -> Option<TrackId> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(TrackId::from_bytes(bytes))
}

/// Stored as a word rather than a number so the file is readable with the
/// `sqlite3` shell, which is worth more than the two bytes it costs.
fn mode_to_sql(mode: Mode) -> &'static str {
    match mode {
        Mode::Minor => "minor",
        Mode::Major => "major",
    }
}

fn mode_from_sql(word: &str) -> Option<Mode> {
    match word {
        "minor" => Some(Mode::Minor),
        "major" => Some(Mode::Major),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::PlayStats;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos, Mode, MusicalKey};

    fn id(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    fn track(byte: u8, title: &str, artist: &str) -> LibraryTrack {
        LibraryTrack {
            id: id(byte),
            path: PathBuf::from(format!("/music/{title}.flac")),
            tags: Tags {
                title: Some(title.to_owned()),
                artist: Some(artist.to_owned()),
                ..Tags::default()
            },
            duration_frames: 48_000 * 200,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: Some(40_000_000),
            file_modified: Some(1_700_000_000),
            added_at: 1_700_000_000,
            analysis: StoredAnalysis::default(),
            stats: PlayStats::default(),
        }
    }

    fn library() -> Library {
        Library::in_memory().unwrap()
    }

    #[test]
    fn a_track_survives_the_round_trip() {
        let lib = library();
        let original = track(1, "Bachata Rosa", "Juan Luis Guerra");
        lib.upsert_track(&original).unwrap();
        assert_eq!(lib.track(id(1)).unwrap().as_ref(), Some(&original));
    }

    #[test]
    fn an_unknown_track_is_absent_rather_than_an_error() {
        assert_eq!(library().track(id(9)).unwrap(), None);
    }

    /// Re-scanning the same file must not make a second row. Identity is the
    /// audio, so the same hash is the same track however many times it is seen.
    #[test]
    fn rescanning_updates_rather_than_duplicates() {
        let lib = library();
        let mut t = track(1, "Bachata Rosa", "Juan Luis Guerra");
        lib.upsert_track(&t).unwrap();

        // The DJ moved the file and fixed the artist tag.
        t.path = PathBuf::from("/music/latin/Bachata Rosa.flac");
        t.tags.artist = Some("Juan Luis Guerra 4.40".to_owned());
        lib.upsert_track(&t).unwrap();

        assert_eq!(lib.track_count().unwrap(), 1);
        let found = lib.track(id(1)).unwrap().unwrap();
        assert_eq!(found.path, PathBuf::from("/music/latin/Bachata Rosa.flac"));
        assert_eq!(found.tags.artist.as_deref(), Some("Juan Luis Guerra 4.40"));
    }

    /// The one that would hurt most if it were wrong: a rescan reads tags off
    /// disk and must not throw away a grid the DJ corrected by hand or a play
    /// count earned over a year.
    #[test]
    fn rescanning_does_not_erase_analysis_or_play_history() {
        let lib = library();
        let t = track(1, "Bachata Rosa", "Juan Luis Guerra");
        lib.upsert_track(&t).unwrap();

        let grid = Beatgrid::new(
            FramePos::new(9_000.0),
            Bpm::new(126.0).unwrap(),
            Confidence::CERTAIN,
        );
        lib.set_analysis(id(1), &StoredAnalysis::default().with_beatgrid(grid))
            .unwrap();
        lib.record_play(id(1), 1_700_000_500, Some("friday"))
            .unwrap();

        lib.upsert_track(&t).unwrap();

        let found = lib.track(id(1)).unwrap().unwrap();
        assert_eq!(found.analysis.beatgrid(), Some(grid));
        assert_eq!(found.stats.play_count, 1);
        assert_eq!(found.stats.last_played, Some(1_700_000_500));
    }

    #[test]
    fn analysis_survives_the_round_trip() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();

        let grid = Beatgrid::new(
            FramePos::new(1_234.5),
            Bpm::new(128.5).unwrap(),
            Confidence::new(0.87),
        );
        let key = MusicalKey::new(8, Mode::Minor).unwrap();
        let stored = StoredAnalysis::default()
            .with_beatgrid(grid)
            .with_key(key, 0.7);
        lib.set_analysis(id(1), &stored).unwrap();

        let found = lib.track(id(1)).unwrap().unwrap().analysis;
        assert_eq!(found.beatgrid(), Some(grid));
        assert_eq!(found.key(), Some(key));
        assert_eq!(found.key_confidence, Some(0.7));
    }

    #[test]
    fn a_track_is_findable_by_path() {
        let lib = library();
        let t = track(1, "Bachata Rosa", "Juan Luis Guerra");
        lib.upsert_track(&t).unwrap();
        assert_eq!(
            lib.track_at_path(&t.path).unwrap().map(|f| f.id),
            Some(id(1))
        );
        assert_eq!(lib.track_at_path(Path::new("/nowhere")).unwrap(), None);
    }

    // -- cues and loops ----------------------------------------------------

    #[test]
    fn cues_survive_the_round_trip() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        let cues = vec![
            StoredCue {
                slot: 1,
                frame: 0.0,
                label: Some("intro".into()),
                colour: Some("#ff0000".into()),
            },
            StoredCue {
                slot: 4,
                frame: 480_000.0,
                label: None,
                colour: None,
            },
        ];
        lib.set_cues(id(1), &cues).unwrap();
        assert_eq!(lib.cues(id(1)).unwrap(), cues);
    }

    /// Frame zero is a real cue position, so it has to come back as one rather
    /// than being mistaken for an empty slot anywhere along the way.
    #[test]
    fn a_cue_at_the_very_start_is_stored_and_returned() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        lib.set_cues(
            id(1),
            &[StoredCue {
                slot: 1,
                frame: 0.0,
                label: None,
                colour: None,
            }],
        )
        .unwrap();
        assert_eq!(lib.cues(id(1)).unwrap().len(), 1);
    }

    /// Writing the set, not upserting each slot: a cue the DJ cleared must
    /// actually go away.
    #[test]
    fn clearing_a_cue_removes_it() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        let cue = |slot| StoredCue {
            slot,
            frame: f64::from(slot) * 1000.0,
            label: None,
            colour: None,
        };
        lib.set_cues(id(1), &[cue(1), cue(2), cue(3)]).unwrap();
        lib.set_cues(id(1), &[cue(1), cue(3)]).unwrap();

        let slots: Vec<u8> = lib.cues(id(1)).unwrap().iter().map(|c| c.slot).collect();
        assert_eq!(slots, vec![1, 3]);
    }

    #[test]
    fn saved_loops_survive_the_round_trip() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        let loops = vec![StoredLoop {
            slot: 1,
            start_frame: 96_000.0,
            end_frame: 192_000.0,
            label: Some("breakdown".into()),
        }];
        lib.set_loops(id(1), &loops).unwrap();
        assert_eq!(lib.loops(id(1)).unwrap(), loops);
    }

    // -- folders -----------------------------------------------------------

    #[test]
    fn folders_are_added_once_and_removable() {
        let lib = library();
        lib.add_folder(Path::new("/music"), 0).unwrap();
        lib.add_folder(Path::new("/music"), 5).unwrap();
        assert_eq!(lib.folders().unwrap(), vec![PathBuf::from("/music")]);

        lib.remove_folder(Path::new("/music")).unwrap();
        assert!(lib.folders().unwrap().is_empty());
    }

    // -- search ------------------------------------------------------------

    #[test]
    fn search_matches_a_partial_word_as_you_type() {
        let lib = library();
        lib.upsert_track(&track(1, "Bachata Rosa", "Juan Luis Guerra"))
            .unwrap();
        lib.upsert_track(&track(2, "Burbujas de Amor", "Juan Luis Guerra"))
            .unwrap();
        lib.upsert_track(&track(3, "Vivir Mi Vida", "Marc Anthony"))
            .unwrap();

        let found = lib.search("bach", 20).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id(1));
    }

    #[test]
    fn search_matches_across_fields_and_narrows_with_each_word() {
        let lib = library();
        lib.upsert_track(&track(1, "Bachata Rosa", "Juan Luis Guerra"))
            .unwrap();
        lib.upsert_track(&track(2, "Burbujas de Amor", "Juan Luis Guerra"))
            .unwrap();

        assert_eq!(lib.search("guerra", 20).unwrap().len(), 2);
        assert_eq!(lib.search("guerra burb", 20).unwrap().len(), 1);
    }

    /// The FTS index is external-content and kept in step by triggers. Without
    /// them an edited track silently stops matching, which looks exactly like
    /// "search is broken" and would not show up in a test that inserts once.
    #[test]
    fn search_follows_an_edited_tag() {
        let lib = library();
        let mut t = track(1, "Bachata Rosa", "Juan Luis Guerra");
        lib.upsert_track(&t).unwrap();

        t.tags.title = Some("Ojala Que Llueva Cafe".to_owned());
        lib.upsert_track(&t).unwrap();

        assert!(
            lib.search("bachata", 20).unwrap().is_empty(),
            "the old title must stop matching"
        );
        assert_eq!(lib.search("llueva", 20).unwrap().len(), 1);
    }

    /// A search box is not a query language. `AC/DC` and a stray quote have to
    /// find things, not raise a syntax error.
    #[test]
    fn punctuation_in_the_search_box_is_not_an_operator() {
        let lib = library();
        lib.upsert_track(&track(1, "Thunderstruck", "AC/DC"))
            .unwrap();

        for query in ["AC/DC", "ac dc", "\"thunder", "thunder*", "-thunder"] {
            assert_eq!(
                lib.search(query, 20).unwrap().len(),
                1,
                "{query:?} must find the track rather than error"
            );
        }
    }

    #[test]
    fn an_empty_search_finds_nothing_rather_than_everything() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        assert!(lib.search("", 20).unwrap().is_empty());
        assert!(lib.search("   ", 20).unwrap().is_empty());
        assert!(lib.search("!!!", 20).unwrap().is_empty());
    }

    #[test]
    fn the_default_view_is_newest_first() {
        let lib = library();
        let mut first = track(1, "Older", "X");
        first.added_at = 100;
        let mut second = track(2, "Newer", "X");
        second.added_at = 200;
        lib.upsert_track(&first).unwrap();
        lib.upsert_track(&second).unwrap();

        let all = lib.all_tracks(10).unwrap();
        assert_eq!(
            all.iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![id(2), id(1)]
        );
    }

    // -- pending files -----------------------------------------------------

    fn scanned(path: &str, title: &str) -> crate::scan::ScannedFile {
        crate::scan::ScannedFile {
            path: PathBuf::from(path),
            tags: Tags {
                title: Some(title.to_owned()),
                ..Tags::default()
            },
            file_size: Some(1000),
            file_modified: Some(500),
        }
    }

    #[test]
    fn a_pending_file_is_queued_and_comes_back_in_order() {
        let lib = library();
        lib.record_pending(&scanned("/music/b.mp3", "B"), 200)
            .unwrap();
        lib.record_pending(&scanned("/music/a.mp3", "A"), 100)
            .unwrap();

        assert_eq!(lib.pending_count().unwrap(), 2);
        let next = lib.next_pending(10).unwrap();
        assert_eq!(
            next.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
            vec![PathBuf::from("/music/a.mp3"), PathBuf::from("/music/b.mp3")],
            "oldest first, so an interrupted scan resumes rather than restarts"
        );
        assert_eq!(next[0].tags.title.as_deref(), Some("A"));
    }

    /// The bug this exists to catch: identification analyses the audio it has
    /// just decoded, and `upsert_track` deliberately never writes the analysis
    /// columns. Without a path that does, the library fills up with tracks that
    /// never get a BPM -- and nothing looks broken while it happens.
    #[test]
    fn promoting_stores_the_analysis_identification_produced() {
        let lib = library();
        lib.record_pending(&scanned("/music/a.mp3", "A"), 100)
            .unwrap();

        let grid = Beatgrid::new(
            FramePos::new(2_000.0),
            Bpm::new(128.0).unwrap(),
            Confidence::new(0.8),
        );
        let mut t = track(1, "A", "B");
        t.path = PathBuf::from("/music/a.mp3");
        t.analysis = StoredAnalysis {
            loudness_lufs: Some(-8.5),
            ..StoredAnalysis::default()
        }
        .with_beatgrid(grid)
        .with_key(MusicalKey::new(8, Mode::Minor).unwrap(), 0.7);

        lib.promote_pending(&t).unwrap();

        let stored = lib.track(id(1)).unwrap().unwrap().analysis;
        assert_eq!(stored.beatgrid(), Some(grid));
        assert_eq!(stored.key(), Some(MusicalKey::new(8, Mode::Minor).unwrap()));
        assert_eq!(stored.loudness_lufs, Some(-8.5));
    }

    /// ...but a second copy of the same audio turning up in another folder must
    /// not replace a grid the DJ corrected with the analyser's fresh guess.
    #[test]
    fn promoting_does_not_overwrite_an_analysis_that_is_already_there() {
        let lib = library();
        let mut t = track(1, "A", "B");
        t.path = PathBuf::from("/music/a.mp3");
        lib.upsert_track(&t).unwrap();

        let corrected = Beatgrid::new(
            FramePos::new(9_999.0),
            Bpm::new(126.0).unwrap(),
            Confidence::CERTAIN,
        );
        lib.set_analysis(id(1), &StoredAnalysis::default().with_beatgrid(corrected))
            .unwrap();

        // The same audio, found again somewhere else, freshly analysed.
        let mut second_copy = t.clone();
        second_copy.path = PathBuf::from("/music/copies/a.mp3");
        second_copy.analysis = StoredAnalysis::default().with_beatgrid(Beatgrid::new(
            FramePos::new(0.0),
            Bpm::new(63.0).unwrap(),
            Confidence::new(0.3),
        ));
        lib.record_pending(&scanned("/music/copies/a.mp3", "A"), 200)
            .unwrap();
        lib.promote_pending(&second_copy).unwrap();

        assert_eq!(
            lib.track(id(1)).unwrap().unwrap().analysis.beatgrid(),
            Some(corrected),
            "the DJ's corrected grid must survive another copy being scanned"
        );
    }

    #[test]
    fn set_analysis_if_absent_reports_whether_it_wrote() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        let fresh = StoredAnalysis {
            loudness_lufs: Some(-10.0),
            ..StoredAnalysis::default()
        };

        assert!(lib.set_analysis_if_absent(id(1), &fresh).unwrap());
        assert!(
            !lib.set_analysis_if_absent(id(1), &fresh).unwrap(),
            "the second call has something to preserve, so it must decline"
        );
    }

    #[test]
    fn promoting_turns_a_pending_file_into_a_track() {
        let lib = library();
        lib.record_pending(&scanned("/music/a.mp3", "A"), 100)
            .unwrap();

        let mut t = track(1, "A", "B");
        t.path = PathBuf::from("/music/a.mp3");
        lib.promote_pending(&t).unwrap();

        assert_eq!(lib.track_count().unwrap(), 1);
        assert_eq!(
            lib.pending_count().unwrap(),
            0,
            "a promoted file must leave the queue, or it is identified forever"
        );
    }

    /// A corrupt file must not sit at the head of the queue blocking everything
    /// behind it.
    #[test]
    fn a_file_that_cannot_be_identified_leaves_the_queue_with_a_reason() {
        let lib = library();
        lib.record_pending(&scanned("/music/broken.mp3", "X"), 100)
            .unwrap();
        lib.mark_pending_failed(Path::new("/music/broken.mp3"), "no decodable audio")
            .unwrap();

        assert_eq!(lib.pending_count().unwrap(), 0);
        assert!(lib.next_pending(10).unwrap().is_empty());
        assert_eq!(
            lib.failed_pending().unwrap(),
            vec![(
                PathBuf::from("/music/broken.mp3"),
                "no decodable audio".to_owned()
            )]
        );
    }

    /// ...but a file the DJ has since replaced deserves another go.
    #[test]
    fn rescanning_a_changed_file_clears_its_failure() {
        let lib = library();
        lib.record_pending(&scanned("/music/broken.mp3", "X"), 100)
            .unwrap();
        lib.mark_pending_failed(Path::new("/music/broken.mp3"), "no decodable audio")
            .unwrap();

        let mut replaced = scanned("/music/broken.mp3", "X");
        replaced.file_size = Some(2000);
        lib.record_pending(&replaced, 300).unwrap();

        assert_eq!(lib.pending_count().unwrap(), 1);
        assert!(lib.failed_pending().unwrap().is_empty());
    }

    #[test]
    fn a_file_with_no_size_or_timestamp_is_never_considered_unchanged() {
        let lib = library();
        lib.record_pending(&scanned("/music/a.mp3", "A"), 100)
            .unwrap();
        assert!(
            !lib.file_is_unchanged(Path::new("/music/a.mp3"), None, None)
                .unwrap(),
            "with nothing to compare, the safe answer is to re-read it"
        );
    }

    /// A promoted track must not be rescanned either, or every scan would
    /// re-queue the whole collection.
    #[test]
    fn an_already_promoted_track_counts_as_unchanged() {
        let lib = library();
        let mut t = track(1, "A", "B");
        t.path = PathBuf::from("/music/a.mp3");
        t.file_size = Some(1000);
        t.file_modified = Some(500);
        lib.upsert_track(&t).unwrap();

        assert!(
            lib.file_is_unchanged(Path::new("/music/a.mp3"), Some(1000), Some(500))
                .unwrap()
        );
        assert!(
            !lib.file_is_unchanged(Path::new("/music/a.mp3"), Some(2000), Some(500))
                .unwrap(),
            "a different size means the file changed"
        );
    }

    // -- playlists ---------------------------------------------------------

    #[test]
    fn a_playlist_holds_tracks_in_the_order_they_were_added() {
        let lib = library();
        for n in 1..=3 {
            lib.upsert_track(&track(n, &format!("T{n}"), "X")).unwrap();
        }
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();

        lib.add_to_playlist(list, id(3)).unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();
        lib.add_to_playlist(list, id(2)).unwrap();

        let tracks = lib.playlist_tracks(list).unwrap();
        assert_eq!(
            tracks.iter().map(|(_, t)| t.id).collect::<Vec<_>>(),
            vec![id(3), id(1), id(2)],
            "a playlist is a sequence the DJ chose, not a set"
        );
    }

    /// A track twice in one set is something DJs do on purpose.
    #[test]
    fn the_same_track_can_appear_twice() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();

        lib.add_to_playlist(list, id(1)).unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();
        assert_eq!(lib.playlist_tracks(list).unwrap().len(), 2);
    }

    /// ...which is why removal names a position, not a track.
    #[test]
    fn removing_takes_the_entry_named_not_every_copy() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        lib.upsert_track(&track(2, "U", "X")).unwrap();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();
        lib.add_to_playlist(list, id(2)).unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();

        let positions: Vec<i64> = lib
            .playlist_tracks(list)
            .unwrap()
            .iter()
            .map(|(p, _)| *p)
            .collect();
        lib.remove_from_playlist(list, positions[0]).unwrap();

        let left = lib.playlist_tracks(list).unwrap();
        assert_eq!(
            left.iter().map(|(_, t)| t.id).collect::<Vec<_>>(),
            vec![id(2), id(1)],
            "the other copy must stay"
        );
    }

    #[test]
    fn reordering_rewrites_the_sequence() {
        let lib = library();
        for n in 1..=3 {
            lib.upsert_track(&track(n, &format!("T{n}"), "X")).unwrap();
        }
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        for n in 1..=3 {
            lib.add_to_playlist(list, id(n)).unwrap();
        }

        let positions: Vec<i64> = lib
            .playlist_tracks(list)
            .unwrap()
            .iter()
            .map(|(p, _)| *p)
            .collect();
        // Last to first.
        let mut order = vec![positions[2]];
        order.extend_from_slice(&positions[..2]);
        lib.reorder_playlist(list, &order).unwrap();

        assert_eq!(
            lib.playlist_tracks(list)
                .unwrap()
                .iter()
                .map(|(_, t)| t.id)
                .collect::<Vec<_>>(),
            vec![id(3), id(1), id(2)]
        );
    }

    /// Deleting a playlist must never take the music with it.
    #[test]
    fn deleting_a_playlist_leaves_the_tracks_alone() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();

        lib.delete_playlist(list).unwrap();
        assert_eq!(lib.track_count().unwrap(), 1);
        assert!(lib.playlists().unwrap().is_empty());
    }

    #[test]
    fn deleting_a_folder_takes_what_is_inside_it() {
        let lib = library();
        let folder = lib
            .create_playlist("Latin", None, PlaylistKind::Folder, None, 0)
            .unwrap();
        lib.create_playlist("Bachata", Some(folder), PlaylistKind::List, None, 0)
            .unwrap();
        assert_eq!(lib.playlists().unwrap().len(), 2);

        lib.delete_playlist(folder).unwrap();
        assert!(lib.playlists().unwrap().is_empty());
    }

    #[test]
    fn only_a_folder_can_hold_other_nodes() {
        let lib = library();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        assert!(matches!(
            lib.create_playlist("Nested", Some(list), PlaylistKind::List, None, 0),
            Err(LibraryError::NotAFolder(_))
        ));
    }

    #[test]
    fn a_playlist_reports_how_many_tracks_it_holds() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();

        let found = lib.playlists().unwrap();
        assert_eq!(found[0].track_count, 1);
    }

    /// The move that would detach a branch: putting a folder inside its own
    /// child. The rows would survive and nothing in the sidebar could reach
    /// them.
    #[test]
    fn a_folder_cannot_be_moved_inside_itself_or_its_own_child() {
        let lib = library();
        let outer = lib
            .create_playlist("Latin", None, PlaylistKind::Folder, None, 0)
            .unwrap();
        let inner = lib
            .create_playlist("Bachata", Some(outer), PlaylistKind::Folder, None, 0)
            .unwrap();

        assert!(matches!(
            lib.move_playlist(outer, Some(outer)),
            Err(LibraryError::WouldOrphan(_))
        ));
        assert!(matches!(
            lib.move_playlist(outer, Some(inner)),
            Err(LibraryError::WouldOrphan(_))
        ));
        // The legal direction still works.
        lib.move_playlist(inner, None).unwrap();
    }

    #[test]
    fn renaming_a_playlist_keeps_its_contents() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Fridya", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();

        lib.rename_playlist(list, "Friday").unwrap();
        let found = lib.playlists().unwrap();
        assert_eq!(found[0].name, "Friday");
        assert_eq!(lib.playlist_tracks(list).unwrap().len(), 1);
    }

    // -- history -----------------------------------------------------------

    #[test]
    fn history_is_most_recent_first() {
        let lib = library();
        lib.upsert_track(&track(1, "First", "X")).unwrap();
        lib.upsert_track(&track(2, "Second", "X")).unwrap();

        lib.record_play(id(1), 100, Some("friday")).unwrap();
        lib.record_play(id(2), 200, Some("friday")).unwrap();

        let history = lib.history(10).unwrap();
        assert_eq!(
            history.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
            vec!["Second", "First"]
        );
        assert_eq!(history[0].session_id.as_deref(), Some("friday"));
    }

    /// A history row and a browser row must not disagree about what a track is
    /// called.
    #[test]
    fn history_falls_back_to_the_filename_the_same_way_the_browser_does() {
        let lib = library();
        let mut untagged = track(1, "ignored", "X");
        untagged.tags.title = None;
        untagged.tags.artist = None;
        untagged.path = PathBuf::from("/music/01 - Untitled Demo.flac");
        lib.upsert_track(&untagged).unwrap();
        lib.record_play(id(1), 100, None).unwrap();

        let history = lib.history(10).unwrap();
        assert_eq!(history[0].title, "01 - Untitled Demo");
        assert_eq!(history[0].artist, "Unknown artist");
    }

    #[test]
    fn playing_a_track_twice_records_both() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        lib.record_play(id(1), 100, None).unwrap();
        lib.record_play(id(1), 200, None).unwrap();

        assert_eq!(lib.history(10).unwrap().len(), 2);
        assert_eq!(lib.track(id(1)).unwrap().unwrap().stats.play_count, 2);
    }

    // -- durability --------------------------------------------------------

    /// The library outlives the process. This is the only test that proves it.
    #[test]
    fn a_library_reopens_with_everything_still_there() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");

        {
            let lib = Library::open(&path).unwrap();
            lib.upsert_track(&track(1, "Bachata Rosa", "Juan Luis Guerra"))
                .unwrap();
            lib.set_cues(
                id(1),
                &[StoredCue {
                    slot: 1,
                    frame: 4_800.0,
                    label: Some("drop".into()),
                    colour: None,
                }],
            )
            .unwrap();
        }

        let reopened = Library::open(&path).unwrap();
        assert_eq!(reopened.track_count().unwrap(), 1);
        assert_eq!(
            reopened.cues(id(1)).unwrap()[0].label.as_deref(),
            Some("drop")
        );
        assert_eq!(reopened.search("bachata", 10).unwrap().len(), 1);
    }

    /// Deleting a track must take its cues with it, which is the foreign key
    /// cascade -- and cascades only fire when `foreign_keys` is on, which is
    /// per-connection and off by default.
    #[test]
    fn deleting_a_track_takes_its_cues_and_loops_with_it() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        lib.set_cues(
            id(1),
            &[StoredCue {
                slot: 1,
                frame: 0.0,
                label: None,
                colour: None,
            }],
        )
        .unwrap();

        lib.with(|conn| {
            conn.execute("DELETE FROM tracks WHERE id = ?1", [id(1).to_hex()])?;
            Ok(())
        })
        .unwrap();

        assert!(
            lib.cues(id(1)).unwrap().is_empty(),
            "orphaned cues would reappear on the next track to reuse the id"
        );
    }
}

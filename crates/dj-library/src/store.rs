//! Reading and writing the library.

use crate::import::ImportReport;
use crate::playlist::{PlayRecord, Playlist, PlaylistKind};
use crate::record::{
    EditableField, LibraryTrack, PlayStats, StoredAnalysis, StoredCue, StoredLoop, Tags, TrackEdit,
};
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
    /// Tracks put into something that is not a plain list.
    ///
    /// A folder holds lists, not tracks. A smart folder holds a *query*, and a
    /// track added by hand would be a member the filter does not select -- it
    /// would go in and never come back out, which is worse than being refused.
    #[error("playlist {0} is not a list, so tracks cannot go in it")]
    NotAList(i64),
    /// Moving a node inside itself, or inside something below it. Refused
    /// rather than performed: the branch would still exist but nothing in the
    /// sidebar could reach it.
    #[error("that would put playlist {0} inside itself")]
    WouldOrphan(i64),
    /// A smart folder's filter could not be understood. Carries the parser's
    /// own message, which names the word it choked on.
    #[error("{0}")]
    BadFilter(String),
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
                     key_confidence = ?8, loudness_lufs = ?9, grid_source = ?10,
                     phrase_beats = ?11, phrase_anchor = ?12, phrase_confidence = ?13
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
                    analysis.grid_source.map(crate::GridSource::as_sql),
                    analysis.phrase_beats,
                    analysis.phrase_anchor,
                    analysis.phrase_confidence,
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
        self.promote_pending_with(track, &crate::import::ImportPayload::default())
    }

    /// The same, plus whatever the *file itself* was carrying.
    ///
    /// Serato writes hot cues into the track rather than into its library, so
    /// they arrive with the file and are found while it is being decoded. They
    /// are an import in every sense that matters — somebody else's judgement
    /// about this track — so they go through the same path and answer to the
    /// same rules: they fill in a grid the analyser guessed at, and they never
    /// replace cues the DJ set here.
    pub fn promote_pending_with(
        &self,
        track: &LibraryTrack,
        found: &crate::import::ImportPayload,
    ) -> Result<()> {
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
        // Anything an import staged against this path: its cues, its loops, its
        // grid, and its place in the playlists. Applied here, in the same
        // transaction, because a promotion that created the row and lost the
        // cues would be silent and unrecoverable.
        Self::apply_staged_import(&tx, track.id, &path)?;
        // What the file carried, after what an import staged: a payload written
        // deliberately into a library export is a better claim than one found
        // in a tag, and `apply_payload` leaves alone anything already there.
        if !found.is_empty() {
            apply_payload(&tx, track.id, found)?;
        }
        tx.execute("DELETE FROM pending_files WHERE path = ?1", [&path])?;

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

    /// Refuse anything that is not a plain list.
    fn require_list(&self, id: i64) -> Result<()> {
        match self.kind_of(id)? {
            Some(PlaylistKind::List) => Ok(()),
            Some(_) => Err(LibraryError::NotAList(id)),
            None => Err(LibraryError::NoSuchPlaylist(id)),
        }
    }

    fn kind_of(&self, id: i64) -> Result<Option<PlaylistKind>> {
        let kind = self.with(|conn| {
            Ok(conn
                .query_row("SELECT kind FROM playlists WHERE id = ?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?)
        })?;
        Ok(kind.as_deref().and_then(PlaylistKind::from_sql))
    }

    fn require_folder(&self, id: i64) -> Result<()> {
        match self.kind_of(id)? {
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

    /// The playlist behind a panel the application owns, made if it is not
    /// there yet.
    ///
    /// Idempotent, so the caller can simply ask for it every time rather than
    /// remembering whether it exists. See the migration that added the column
    /// for why the Sidelist is a playlist rather than a table of its own.
    pub fn system_playlist(&self, name: &str, now: i64) -> Result<i64> {
        if let Some(id) = self.with(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id FROM playlists WHERE system = ?1",
                    [name],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?)
        })? {
            return Ok(id);
        }

        self.with(|conn| {
            conn.execute(
                "INSERT INTO playlists (name, parent_id, kind, query, created_at, system)
                 VALUES (?1, NULL, 'list', NULL, ?2, ?1)",
                params![name, now],
            )?;
            Ok(conn.last_insert_rowid())
        })
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
                 WHERE p.system IS NULL
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
    /// Put a track at the end of a list.
    ///
    /// Refuses anything that is not a plain list. That guard lives here rather
    /// than only in the interface because the interface is not the only caller:
    /// an importer, the assistant and the network API all reach this, and a
    /// track filed into a smart folder is a row its own query will never
    /// return -- it goes in and never comes back out.
    pub fn add_to_playlist(&self, playlist: i64, track: TrackId) -> Result<()> {
        self.require_list(playlist)?;
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

    /// Take everything out of a playlist, keeping the playlist.
    ///
    /// Explicit rather than reordering to an empty list: that would clear it
    /// too, but by a side effect of how reordering is implemented, and a
    /// caller reading `reorder(&[])` has to know that to know what it does.
    pub fn clear_playlist(&self, playlist: i64) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
                [playlist],
            )?;
            Ok(())
        })
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

    /// Evaluate a smart folder's filter.
    ///
    /// The filter is parsed and compiled here, on every call, rather than
    /// stored compiled. Parsing a line of text is microseconds and the query it
    /// produces is what actually costs; caching the parse would be an
    /// invalidation problem in exchange for nothing measurable.
    pub fn smart_tracks(&self, query: &str, limit: usize) -> Result<Vec<LibraryTrack>> {
        let filter =
            crate::filter::parse(query).map_err(|e| LibraryError::BadFilter(e.to_string()))?;
        let compiled = filter.compile();

        self.with(|conn| {
            let sql = format!(
                "SELECT {TRACK_COLUMNS} FROM tracks WHERE {} ORDER BY artist COLLATE NOCASE, title COLLATE NOCASE LIMIT ?{}",
                compiled.sql,
                compiled.params.len() + 1
            );
            let mut stmt = conn.prepare(&sql)?;

            // Bound one at a time, in the order the compiler numbered them.
            // Nothing the user typed is in `sql` -- see `filter::compile`.
            let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            for param in &compiled.params {
                bound.push(match param {
                    crate::filter::Param::Number(n) => Box::new(*n),
                    crate::filter::Param::Text(t) => Box::new(t.clone()),
                    crate::filter::Param::Integer(i) => Box::new(*i),
                });
            }
            bound.push(Box::new(limit as i64));

            let refs: Vec<&dyn rusqlite::ToSql> = bound.iter().map(std::convert::AsRef::as_ref).collect();
            let rows = stmt.query_map(refs.as_slice(), read_track)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .collect()
        })
    }

    /// A smart folder's tracks, by its id.
    pub fn smart_playlist_tracks(&self, playlist: i64, limit: usize) -> Result<Vec<LibraryTrack>> {
        let query = self.with(|conn| {
            Ok(conn
                .query_row(
                    "SELECT query FROM playlists WHERE id = ?1 AND kind = 'smart'",
                    [playlist],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten())
        })?;
        match query {
            Some(query) => self.smart_tracks(&query, limit),
            None => Err(LibraryError::NoSuchPlaylist(playlist)),
        }
    }

    /// Change what a smart folder selects.
    ///
    /// Parsed before it is stored, so a filter that cannot be understood is
    /// reported at the moment it is typed rather than the next time the folder
    /// is opened.
    pub fn set_playlist_query(&self, id: i64, query: &str) -> Result<()> {
        crate::filter::parse(query).map_err(|e| LibraryError::BadFilter(e.to_string()))?;
        self.with(|conn| {
            conn.execute(
                "UPDATE playlists SET query = ?2 WHERE id = ?1",
                params![id, query],
            )?;
            Ok(())
        })
    }

    // -- importing ---------------------------------------------------------

    /// Bring an imported collection into the library.
    ///
    /// # What happens to a track depends on whether we know it
    ///
    /// An import names tracks by path, and our identity is the hash of the
    /// decoded audio. So:
    ///
    /// - a path already in `tracks` is **updated in place** — its tags are
    ///   refreshed and its cues, loops and grid are applied straight away,
    ///   because there is a row to hang them on;
    /// - a path we have never seen is **queued**, with the import's cues and
    ///   grid attached, for the background identifier to decode and promote.
    ///
    /// The playlist *tree* is created immediately either way, because it needs
    /// no track ids: a DJ who imports 5,000 tracks sees their crates in the
    /// sidebar at once and watches the contents fill in.
    ///
    /// One transaction. An import that half-happened would be worse than one
    /// that failed, because there is no way to tell which half.
    pub fn import(&self, collection: &crate::import::Collection, now: i64) -> Result<ImportReport> {
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;

        let mut report = ImportReport {
            tracks: collection.tracks.len(),
            skipped: collection
                .skipped
                .iter()
                .map(|s| format!("{}: {}", s.what, s.reason))
                .collect(),
            ..ImportReport::default()
        };

        for track in &collection.tracks {
            let path = track.path.to_string_lossy();
            let known: Option<String> = tx
                .query_row("SELECT id FROM tracks WHERE path = ?1", [&path], |row| {
                    row.get(0)
                })
                .optional()?;

            match known.as_deref().and_then(track_id_from_hex) {
                Some(id) => {
                    apply_import_tags(&tx, id, track)?;
                    apply_payload(&tx, id, &track.payload)?;
                    report.already_known += 1;
                }
                None => {
                    queue_import(&tx, track, now)?;
                    report.queued += 1;
                }
            }
        }

        // The tree. Created after the tracks so that a playlist entry naming a
        // path already in the library can be written straight through.
        let mut ids: Vec<i64> = Vec::with_capacity(collection.playlists.len());
        for node in &collection.playlists {
            let parent = node.parent.and_then(|index| ids.get(index).copied());
            let kind = if node.is_folder {
                PlaylistKind::Folder
            } else {
                PlaylistKind::List
            };
            tx.execute(
                "INSERT INTO playlists (name, parent_id, kind, query, created_at)
                 VALUES (?1, ?2, ?3, NULL, ?4)",
                params![node.name, parent, kind.as_sql(), now],
            )?;
            let id = tx.last_insert_rowid();
            ids.push(id);

            if node.is_folder {
                report.folders += 1;
            } else {
                report.playlists += 1;
            }

            for (position, path) in node.paths.iter().enumerate() {
                add_import_entry(&tx, id, path, position as i64)?;
            }
        }

        tx.commit()?;
        Ok(report)
    }

    /// Apply whatever an import staged for a track, and forget it.
    ///
    /// Called at promotion, when the file finally has an id. Separate from
    /// [`Self::promote_pending`] so the identifier can apply it in the same
    /// transaction that creates the row.
    fn apply_staged_import(tx: &rusqlite::Transaction<'_>, id: TrackId, path: &str) -> Result<()> {
        let payload: Option<String> = tx
            .query_row(
                "SELECT import_payload FROM pending_files WHERE path = ?1",
                [path],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        if let Some(json) = payload
            && let Ok(payload) = serde_json::from_str::<crate::import::ImportPayload>(&json)
        {
            apply_payload(tx, id, &payload)?;
        }

        // Playlist membership queued by path becomes real membership.
        tx.execute(
            "INSERT OR REPLACE INTO playlist_tracks (playlist_id, track_id, position)
             SELECT playlist_id, ?2, position FROM pending_playlist_entries WHERE path = ?1",
            params![path, id.to_hex()],
        )?;
        tx.execute(
            "DELETE FROM pending_playlist_entries WHERE path = ?1",
            [path],
        )?;
        Ok(())
    }

    // -- editing -----------------------------------------------------------

    /// Change tags on a set of tracks at once.
    ///
    /// Every field is optional and `None` means *leave it alone*, not *clear
    /// it*. A DJ setting a genre across forty tracks is not asking to wipe
    /// their artists — and the alternative, where absent means empty, is the
    /// kind of interface that eats a collection in one click.
    ///
    /// One transaction, so a batch either lands or does not.
    pub fn edit_tracks(&self, ids: &[TrackId], edit: &TrackEdit) -> Result<usize> {
        if ids.is_empty() || edit.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;
        let mut changed = 0;

        for id in ids {
            changed += tx.execute(
                "UPDATE tracks SET
                     genre  = COALESCE(?2, genre),
                     label  = COALESCE(?3, label),
                     artist = COALESCE(?4, artist),
                     album  = COALESCE(?5, album),
                     comment = COALESCE(?6, comment),
                     year   = COALESCE(?7, year),
                     rating = COALESCE(?8, rating),
                     colour = COALESCE(?9, colour)
                 WHERE id = ?1",
                params![
                    id.to_hex(),
                    edit.genre,
                    edit.label,
                    edit.artist,
                    edit.album,
                    edit.comment,
                    edit.year,
                    edit.rating,
                    edit.colour,
                ],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    /// Clear a field across a set of tracks.
    ///
    /// Separate from [`Self::edit_tracks`] on purpose: "set this to nothing" is
    /// a different intention from "leave this alone", and a single method that
    /// tried to express both would have to invent a sentinel for one of them.
    pub fn clear_field(&self, ids: &[TrackId], field: EditableField) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;
        let mut changed = 0;
        // The column name comes from an enum, never from the caller's text.
        let sql = format!("UPDATE tracks SET {} = NULL WHERE id = ?1", field.column());
        for id in ids {
            changed += tx.execute(&sql, [id.to_hex()])?;
        }
        tx.commit()?;
        Ok(changed)
    }

    // -- duplicates --------------------------------------------------------

    /// Every place a track's audio has been seen, newest first.
    pub fn paths_for(&self, id: TrackId) -> Result<Vec<(PathBuf, i64, Option<u64>)>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT path, seen_at, file_size FROM track_paths
                 WHERE track_id = ?1 ORDER BY seen_at DESC, path",
            )?;
            let rows = stmt.query_map([id.to_hex()], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get(1)?,
                    row.get::<_, Option<i64>>(2)?.map(|s| s.max(0) as u64),
                ))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Tracks whose audio exists at more than one path.
    ///
    /// The same recording in FLAC and in an MP3 made from it is *not* a
    /// duplicate here, and correctly so: they are different audio, and a cue
    /// placed on one is milliseconds out on the other. This finds byte-for-byte
    /// the same music in two places, which is the thing worth deleting.
    pub fn duplicates(&self, limit: usize) -> Result<Vec<(LibraryTrack, Vec<PathBuf>)>> {
        let ids: Vec<String> = self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT track_id FROM track_paths
                 GROUP BY track_id HAVING count(*) > 1
                 ORDER BY count(*) DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })?;

        let mut out = Vec::with_capacity(ids.len());
        for hex in ids {
            let Some(id) = track_id_from_hex(&hex) else {
                continue;
            };
            if let Some(track) = self.track(id)? {
                let paths = self.paths_for(id)?.into_iter().map(|(p, _, _)| p).collect();
                out.push((track, paths));
            }
        }
        Ok(out)
    }

    /// Forget a path, without touching the track.
    ///
    /// What "delete the duplicate" means: the DJ removes the file themselves —
    /// nothing here deletes anybody's music — and this drops the library's
    /// memory of it. If it was the path the track is opened from, the track
    /// moves to another one it has.
    pub fn forget_path(&self, id: TrackId, path: &Path) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| LibraryError::Poisoned)?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM track_paths WHERE track_id = ?1 AND path = ?2",
            params![id.to_hex(), path.to_string_lossy()],
        )?;
        // If that was the path the track plays from, move it to one that is
        // left rather than leaving a row pointing at a file nobody has.
        tx.execute(
            "UPDATE tracks SET path = COALESCE(
                 (SELECT path FROM track_paths WHERE track_id = ?1
                  ORDER BY seen_at DESC, path LIMIT 1),
                 path
             )
             WHERE id = ?1 AND path = ?2",
            params![id.to_hex(), path.to_string_lossy()],
        )?;
        tx.commit()?;
        Ok(())
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
            let rows = stmt.query_map([limit as i64], read_play)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Everything played in one session, oldest first.
    ///
    /// Oldest first, unlike [`Self::history`]: a session export is a record of
    /// how a night went, and a set list runs forwards.
    pub fn session(&self, session_id: &str) -> Result<Vec<PlayRecord>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT history.track_id, tracks.title, tracks.artist, tracks.path,
                        history.played_at, history.session_id
                 FROM history
                 JOIN tracks ON tracks.id = history.track_id
                 WHERE history.session_id = ?1
                 ORDER BY history.played_at, history.id",
            )?;
            let rows = stmt.query_map([session_id], read_play)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// The sessions there are, most recent first: id, how many tracks, and
    /// when the last one played.
    pub fn sessions(&self, limit: usize) -> Result<Vec<(String, i64, i64)>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT session_id, count(*), max(played_at)
                 FROM history
                 WHERE session_id IS NOT NULL
                 GROUP BY session_id
                 ORDER BY max(played_at) DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
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
    // After the row exists, so the foreign key always has something to point
    // at -- including on the very first sight of a track.
    remember_path(conn, track)?;
    Ok(())
}

/// Note that this track's audio exists at this path.
///
/// Called on every upsert, so a second copy found in another folder is
/// remembered rather than silently replacing the first — see the migration that
/// added the table. `tracks.path` still moves to the newest, because that is
/// the one to open; this is only about knowing the others are there.
///
/// Runs *after* the track row is written, so the foreign key always has
/// something to point at — including on the very first sight of a track, which
/// an earlier version of this got wrong: recording the path first meant a new
/// track's own path was skipped, and a rescan does not re-read an unchanged
/// file, so it would never have been recorded at all.
fn remember_path(conn: &Connection, track: &LibraryTrack) -> Result<()> {
    conn.execute(
        "INSERT INTO track_paths (track_id, path, seen_at, file_size)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(track_id, path) DO UPDATE SET
             seen_at = excluded.seen_at,
             file_size = excluded.file_size",
        params![
            track.id.to_hex(),
            track.path.to_string_lossy(),
            track.added_at,
            track.file_size.map(|s| s as i64),
        ],
    )?;
    Ok(())
}

/// Write analysis where the new result has at least as much authority as what
/// is already there. Returns whether it wrote.
///
/// The authority ordering lives in [`crate::GridSource`]: a hand edit outranks
/// an import, an import outranks an analysis, and an analysis fills in a blank.
/// Without it the only rules expressible are "always" and "never", and both are
/// wrong -- see the migration that added the column.
///
/// The key and the loudness ride along with the grid. They come from the same
/// source in every case, and splitting them would mean a track whose grid was
/// imported and whose key was analysed, which is harder to explain than it is
/// worth.
fn set_analysis_if_absent_on(
    conn: &Connection,
    id: TrackId,
    analysis: &StoredAnalysis,
) -> Result<bool> {
    let existing: Option<Option<String>> = conn
        .query_row(
            "SELECT grid_source FROM tracks WHERE id = ?1",
            [id.to_hex()],
            |row| row.get(0),
        )
        .optional()?;
    // No row at all: nothing to write to.
    let Some(existing) = existing else {
        return Ok(false);
    };
    let existing = existing.as_deref().and_then(crate::GridSource::from_sql);

    // An analysis with nothing to say must not clear a grid that is there.
    let incoming = analysis.grid_source.unwrap_or(crate::GridSource::Analysis);
    if !incoming.may_replace(existing) {
        return Ok(false);
    }

    let written = conn.execute(
        "UPDATE tracks SET
             bpm = ?2, grid_anchor = ?3, grid_beats_per_bar = ?4,
             grid_confidence = ?5,
             -- COALESCE so a source that knows the grid but not the key -- an
             -- import from software that records no key, say -- does not blank
             -- one the analyser had already found.
             key_hour = COALESCE(?6, key_hour),
             key_mode = COALESCE(?7, key_mode),
             key_confidence = COALESCE(?8, key_confidence),
             loudness_lufs = COALESCE(?9, loudness_lufs),
             grid_source = ?10,
             -- Phrases move with the grid rather than being COALESCEd beside
             -- it: they are measured in beats *from* the grid anchor, so a
             -- phrase kept across a grid change points at the wrong beat. An
             -- importer that brings a grid and no phrases clears them, and the
             -- analyser finds them again.
             phrase_beats = ?11,
             phrase_anchor = ?12,
             phrase_confidence = ?13
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
            analysis.grid_source.map(crate::GridSource::as_sql),
            analysis.phrase_beats,
            analysis.phrase_anchor,
            analysis.phrase_confidence,
        ],
    )?;
    Ok(written > 0)
}

/// Read one row of play history.
///
/// The title falls back to the filename exactly as the browser's does, so a
/// history row and a browser row never disagree about what a track is called.
fn read_play(row: &Row<'_>) -> rusqlite::Result<PlayRecord> {
    let title: Option<String> = row.get(1)?;
    let artist: Option<String> = row.get(2)?;
    let path: String = row.get(3)?;
    Ok(PlayRecord {
        track_id: row.get(0)?,
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
     play_count, last_played, rating, grid_source, colour, \
     phrase_beats, phrase_anchor, phrase_confidence";

/// The same list, qualified — needed wherever the query joins another table
/// that has columns of the same name.
const TRACK_COLUMNS_QUALIFIED: &str = "tracks.id, tracks.path, tracks.title, tracks.artist, \
     tracks.album, tracks.album_artist, tracks.genre, tracks.label, tracks.comment, \
     tracks.year, tracks.track_number, tracks.duration_frames, tracks.sample_rate, \
     tracks.channels, tracks.file_size, tracks.file_modified, tracks.added_at, \
     tracks.bpm, tracks.grid_anchor, tracks.grid_beats_per_bar, \
     tracks.grid_confidence, tracks.key_hour, tracks.key_mode, tracks.key_confidence, \
     tracks.loudness_lufs, tracks.play_count, tracks.last_played, tracks.rating, \
     tracks.grid_source, tracks.colour, \
     tracks.phrase_beats, tracks.phrase_anchor, tracks.phrase_confidence";

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
            grid_source: row
                .get::<_, Option<String>>(at(28))?
                .as_deref()
                .and_then(crate::GridSource::from_sql),
            phrase_beats: row.get(at(30))?,
            phrase_anchor: row.get(at(31))?,
            phrase_confidence: row.get(at(32))?,
        },
        stats: PlayStats {
            play_count: row.get(at(25))?,
            last_played: row.get(at(26))?,
            rating: row.get(at(27))?,
        },
        colour: row.get(at(29))?,
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

/// Refresh a known track's tags from an import.
///
/// Tags only. An import must not overwrite analysis a DJ has since corrected —
/// the same rule a rescan follows, for the same reason.
fn apply_import_tags(
    tx: &rusqlite::Transaction<'_>,
    id: TrackId,
    track: &crate::import::ImportedTrack,
) -> Result<()> {
    tx.execute(
        "UPDATE tracks SET
             title = COALESCE(?2, title),
             artist = COALESCE(?3, artist),
             album = COALESCE(?4, album),
             album_artist = COALESCE(?5, album_artist),
             genre = COALESCE(?6, genre),
             label = COALESCE(?7, label),
             comment = COALESCE(?8, comment),
             year = COALESCE(?9, year),
             track_number = COALESCE(?10, track_number),
             rating = COALESCE(?11, rating)
         WHERE id = ?1",
        params![
            id.to_hex(),
            track.title,
            track.artist,
            track.album,
            track.album_artist,
            track.genre,
            track.label,
            track.comment,
            track.year,
            track.track_number,
            track.rating,
        ],
    )?;
    Ok(())
}

/// Write an import's cues, loops and grid onto a track that now has an id.
///
/// Cues and loops are written only where the track has none. A DJ who has been
/// playing a record in djmanzo has cues on it that are theirs; an import from
/// the software they left behind must not replace them. The grid follows the
/// same rule via `set_analysis_if_absent_on`.
fn apply_payload(
    tx: &rusqlite::Transaction<'_>,
    id: TrackId,
    payload: &crate::import::ImportPayload,
) -> Result<()> {
    if payload.is_empty() {
        return Ok(());
    }

    // The frame positions depend on the track's own sample rate, which is only
    // known once it has been decoded -- so this reads it back rather than
    // assuming one. A track with no rate cannot be given cues in frames.
    let rate: Option<u32> = tx
        .query_row(
            "SELECT sample_rate FROM tracks WHERE id = ?1",
            [id.to_hex()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(rate) = rate.and_then(SampleRate::new) else {
        return Ok(());
    };
    let frames = |seconds: f64| seconds * rate.as_f64();

    let existing_cues: i64 = tx.query_row(
        "SELECT count(*) FROM cues WHERE track_id = ?1",
        [id.to_hex()],
        |row| row.get(0),
    )?;
    if existing_cues == 0 {
        for cue in &payload.cues {
            tx.execute(
                "INSERT OR REPLACE INTO cues (track_id, slot, frame, label, colour)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_hex(),
                    cue.slot,
                    frames(cue.seconds),
                    cue.label,
                    cue.colour
                ],
            )?;
        }
    }

    let existing_loops: i64 = tx.query_row(
        "SELECT count(*) FROM saved_loops WHERE track_id = ?1",
        [id.to_hex()],
        |row| row.get(0),
    )?;
    if existing_loops == 0 {
        for region in &payload.loops {
            tx.execute(
                "INSERT OR REPLACE INTO saved_loops (track_id, slot, start_frame, end_frame, label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id.to_hex(),
                    region.slot,
                    frames(region.start_seconds),
                    frames(region.end_seconds),
                    region.label
                ],
            )?;
        }
    }

    // A payload with cues but no tempo — which is what Serato's in-file
    // markers usually are — has nothing to say about the grid, and must not
    // say it anyway. Writing one would blank a tempo the analyser had found:
    // every field below is `None`, and the write is an overwrite, not a merge.
    if payload.bpm.is_none() && payload.key_hour.is_none() {
        return Ok(());
    }

    let mut analysis = StoredAnalysis {
        // The grid a DJ has been playing from in another application outranks
        // whatever our analyser guessed, and is outranked by a hand edit here.
        grid_source: payload.bpm.map(|_| crate::GridSource::Import),
        bpm: payload.bpm,
        // A grid with a tempo but no anchor is not a grid. An import that gave
        // only a BPM gets frame zero, which is what every DJ software assumes
        // when it has nothing better -- and is correctable in one click.
        grid_anchor: payload
            .bpm
            .map(|_| frames(payload.grid_anchor_seconds.unwrap_or(0.0))),
        grid_beats_per_bar: payload.bpm.map(|_| 4),
        // An imported grid is one somebody already trusted enough to play from.
        grid_confidence: payload.bpm.map(|_| 1.0),
        ..StoredAnalysis::default()
    };
    if let (Some(hour), Some(minor)) = (payload.key_hour, payload.key_minor) {
        analysis.key_hour = Some(hour);
        analysis.key_mode = Some(if minor { Mode::Minor } else { Mode::Major });
        analysis.key_confidence = Some(1.0);
    }
    set_analysis_if_absent_on(tx, id, &analysis)?;
    Ok(())
}

/// Queue a track an import named but the library has never seen.
fn queue_import(
    tx: &rusqlite::Transaction<'_>,
    track: &crate::import::ImportedTrack,
    now: i64,
) -> Result<()> {
    let payload = if track.payload.is_empty() {
        None
    } else {
        serde_json::to_string(&track.payload).ok()
    };
    tx.execute(
        "INSERT INTO pending_files (
             path, title, artist, album, album_artist, genre, label, comment,
             year, track_number, seen_at, import_payload
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(path) DO UPDATE SET
             title = COALESCE(excluded.title, title),
             artist = COALESCE(excluded.artist, artist),
             album = COALESCE(excluded.album, album),
             album_artist = COALESCE(excluded.album_artist, album_artist),
             genre = COALESCE(excluded.genre, genre),
             label = COALESCE(excluded.label, label),
             comment = COALESCE(excluded.comment, comment),
             year = COALESCE(excluded.year, year),
             track_number = COALESCE(excluded.track_number, track_number),
             import_payload = COALESCE(excluded.import_payload, import_payload),
             -- A file named by an import is worth another attempt even if a
             -- previous scan could not read it.
             failed_reason = NULL",
        params![
            track.path.to_string_lossy(),
            track.title,
            track.artist,
            track.album,
            track.album_artist,
            track.genre,
            track.label,
            track.comment,
            track.year,
            track.track_number,
            now,
            payload,
        ],
    )?;
    Ok(())
}

/// Put a path into a playlist, whether or not the track behind it is known yet.
fn add_import_entry(
    tx: &rusqlite::Transaction<'_>,
    playlist: i64,
    path: &std::path::Path,
    position: i64,
) -> Result<()> {
    let path = path.to_string_lossy();
    let known: Option<String> = tx
        .query_row("SELECT id FROM tracks WHERE path = ?1", [&path], |row| {
            row.get(0)
        })
        .optional()?;

    match known {
        Some(id) => {
            tx.execute(
                "INSERT OR REPLACE INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (?1, ?2, ?3)",
                params![playlist, id, position],
            )?;
        }
        None => {
            tx.execute(
                "INSERT OR REPLACE INTO pending_playlist_entries (playlist_id, path, position)
                 VALUES (?1, ?2, ?3)",
                params![playlist, path, position],
            )?;
        }
    }
    Ok(())
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
            colour: None,
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
    fn promoting_does_not_overwrite_a_hand_corrected_grid() {
        let lib = library();
        let mut t = track(1, "A", "B");
        t.path = PathBuf::from("/music/a.mp3");
        lib.upsert_track(&t).unwrap();

        let corrected = Beatgrid::new(
            FramePos::new(9_999.0),
            Bpm::new(126.0).unwrap(),
            Confidence::CERTAIN,
        );
        lib.set_analysis(
            id(1),
            &StoredAnalysis::default()
                .with_beatgrid(corrected)
                .from_source(crate::GridSource::Manual),
        )
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

    /// The authority ordering, exercised in every direction that matters.
    #[test]
    fn a_grid_may_only_be_replaced_by_one_with_at_least_as_much_authority() {
        use crate::GridSource;

        let grid = |bpm: f64| {
            Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(bpm).unwrap(),
                Confidence::CERTAIN,
            )
        };
        let write = |lib: &Library, bpm: f64, source: GridSource| {
            lib.set_analysis_if_absent(
                id(1),
                &StoredAnalysis::default()
                    .with_beatgrid(grid(bpm))
                    .from_source(source),
            )
            .unwrap()
        };
        let bpm_now = |lib: &Library| lib.track(id(1)).unwrap().unwrap().analysis.bpm;

        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();

        // A blank takes anything.
        assert!(write(&lib, 100.0, GridSource::Analysis));
        assert_eq!(bpm_now(&lib), Some(100.0));

        // Re-analysing may improve an analysis.
        assert!(write(&lib, 101.0, GridSource::Analysis));
        assert_eq!(bpm_now(&lib), Some(101.0));

        // An import outranks an analysis: a grid somebody has been playing from
        // beats one our analyser guessed.
        assert!(write(&lib, 128.0, GridSource::Import));
        assert_eq!(bpm_now(&lib), Some(128.0));

        // ...and an analysis does not then overwrite the import.
        assert!(!write(&lib, 64.0, GridSource::Analysis));
        assert_eq!(bpm_now(&lib), Some(128.0));

        // A hand edit outranks everything.
        assert!(write(&lib, 126.0, GridSource::Manual));
        assert_eq!(bpm_now(&lib), Some(126.0));

        // ...and nothing outranks it.
        assert!(!write(&lib, 128.0, GridSource::Import));
        assert!(!write(&lib, 64.0, GridSource::Analysis));
        assert_eq!(bpm_now(&lib), Some(126.0));
    }

    /// A source that knows the grid but not the key must not blank a key the
    /// analyser had already found.
    #[test]
    fn a_grid_without_a_key_does_not_erase_one_that_is_there() {
        use crate::GridSource;

        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        lib.set_analysis(
            id(1),
            &StoredAnalysis::default().with_key(MusicalKey::new(8, Mode::Minor).unwrap(), 0.8),
        )
        .unwrap();

        lib.set_analysis_if_absent(
            id(1),
            &StoredAnalysis::default()
                .with_beatgrid(Beatgrid::new(
                    FramePos::new(0.0),
                    Bpm::new(128.0).unwrap(),
                    Confidence::CERTAIN,
                ))
                .from_source(GridSource::Import),
        )
        .unwrap();

        let found = lib.track(id(1)).unwrap().unwrap().analysis;
        assert_eq!(found.bpm, Some(128.0));
        assert_eq!(found.key(), MusicalKey::new(8, Mode::Minor));
    }

    #[test]
    fn writing_to_a_track_that_does_not_exist_reports_that_it_did_not() {
        let lib = library();
        assert!(
            !lib.set_analysis_if_absent(id(9), &StoredAnalysis::default())
                .unwrap()
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
    fn clearing_a_playlist_keeps_the_playlist_and_the_tracks() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();

        lib.clear_playlist(list).unwrap();

        assert!(lib.playlist_tracks(list).unwrap().is_empty());
        assert_eq!(lib.playlists().unwrap().len(), 1, "the playlist stays");
        assert_eq!(lib.track_count().unwrap(), 1, "the music stays");
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

    /// A folder holds lists, not tracks. Without this guard the row went in
    /// happily and simply never appeared anywhere.
    #[test]
    fn a_folder_will_not_take_tracks() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let folder = lib
            .create_playlist("Crates", None, PlaylistKind::Folder, None, 0)
            .unwrap();
        assert!(matches!(
            lib.add_to_playlist(folder, id(1)),
            Err(LibraryError::NotAList(_))
        ));
    }

    /// The one that actually loses tracks. A smart folder's contents are a
    /// query, so a track added by hand is a member the filter does not select:
    /// it goes in and never comes back out.
    #[test]
    fn a_smart_folder_will_not_take_tracks_by_hand() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let smart = lib
            .create_playlist("Fast", None, PlaylistKind::Smart, Some("bpm > 128"), 0)
            .unwrap();
        assert!(matches!(
            lib.add_to_playlist(smart, id(1)),
            Err(LibraryError::NotAList(_))
        ));
    }

    #[test]
    fn a_plain_list_takes_tracks_as_it_always_did() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let list = lib
            .create_playlist("Set", None, PlaylistKind::List, None, 0)
            .unwrap();
        lib.add_to_playlist(list, id(1)).unwrap();
        assert_eq!(lib.playlist_tracks(list).unwrap().len(), 1);
    }

    #[test]
    fn a_playlist_that_does_not_exist_says_so_rather_than_not_a_list() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        assert!(matches!(
            lib.add_to_playlist(9999, id(1)),
            Err(LibraryError::NoSuchPlaylist(_))
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

    // -- smart folders -----------------------------------------------------

    /// Three tracks with enough analysis to filter on.
    fn analysed_library() -> Library {
        let lib = library();
        let specs = [
            (
                1u8,
                "Bachata Rosa",
                "Juan Luis Guerra",
                128.0,
                8u8,
                Mode::Minor,
                "latin",
            ),
            (
                2,
                "Vivir Mi Vida",
                "Marc Anthony",
                96.0,
                11,
                Mode::Major,
                "salsa",
            ),
            (
                3,
                "Gasolina",
                "Daddy Yankee",
                140.0,
                9,
                Mode::Minor,
                "reggaeton",
            ),
        ];
        for (byte, title, artist, bpm, hour, mode, genre) in specs {
            let mut t = track(byte, title, artist);
            t.tags.genre = Some(genre.to_owned());
            t.tags.year = Some(2000 + i32::from(byte));
            lib.upsert_track(&t).unwrap();
            lib.set_analysis(
                id(byte),
                &StoredAnalysis::default()
                    .with_beatgrid(Beatgrid::new(
                        FramePos::new(0.0),
                        Bpm::new(bpm).unwrap(),
                        Confidence::CERTAIN,
                    ))
                    .with_key(MusicalKey::new(hour, mode).unwrap(), 0.9),
            )
            .unwrap();
        }
        lib
    }

    fn titles(found: &[LibraryTrack]) -> Vec<String> {
        found.iter().map(LibraryTrack::display_title).collect()
    }

    #[test]
    fn a_smart_folder_selects_by_tempo() {
        let lib = analysed_library();
        let found = lib.smart_tracks("bpm > 120", 50).unwrap();
        assert_eq!(
            titles(&found),
            vec!["Gasolina", "Bachata Rosa"],
            "ordered by artist: Daddy Yankee before Juan Luis Guerra"
        );
    }

    #[test]
    fn a_smart_folder_combines_conditions() {
        let lib = analysed_library();
        let found = lib
            .smart_tracks("bpm > 120 and not genre contains reggaeton", 50)
            .unwrap();
        assert_eq!(titles(&found), vec!["Bachata Rosa"]);
    }

    /// The harmonic case, which is the reason to filter by key.
    #[test]
    fn a_smart_folder_finds_harmonic_neighbours() {
        let lib = analysed_library();
        // 9A is one step round the wheel from 8A, so both are compatible with
        // 8A; 11B is not.
        let found = lib.smart_tracks("key compatible 8A", 50).unwrap();
        assert_eq!(titles(&found), vec!["Gasolina", "Bachata Rosa"]);
    }

    #[test]
    fn an_exact_key_matches_only_that_key() {
        let lib = analysed_library();
        assert_eq!(
            titles(&lib.smart_tracks("key = 11B", 50).unwrap()),
            vec!["Vivir Mi Vida"]
        );
    }

    #[test]
    fn text_matching_ignores_case() {
        let lib = analysed_library();
        assert_eq!(
            titles(&lib.smart_tracks("artist contains GUERRA", 50).unwrap()),
            vec!["Bachata Rosa"]
        );
    }

    /// An unanalysed track has no BPM, and must not match a tempo condition --
    /// nor be swept in by negating one.
    #[test]
    fn an_unanalysed_track_matches_no_tempo_condition_either_way() {
        let lib = analysed_library();
        lib.upsert_track(&track(9, "Unknown", "Nobody")).unwrap();

        assert!(
            !titles(&lib.smart_tracks("bpm > 120", 50).unwrap()).contains(&"Unknown".to_owned())
        );
        assert!(
            !titles(&lib.smart_tracks("not bpm > 120", 50).unwrap())
                .contains(&"Unknown".to_owned()),
            "negating a comparison must not sweep in tracks that have no value at all"
        );
    }

    /// The whole point of compiling rather than storing SQL.
    #[test]
    fn a_filter_cannot_execute_sql() {
        let lib = analysed_library();
        let found = lib
            .smart_tracks(r#"artist = "x'); DROP TABLE tracks;--""#, 50)
            .unwrap();
        assert!(found.is_empty());
        assert_eq!(
            lib.track_count().unwrap(),
            3,
            "the tracks table must still be there"
        );
    }

    #[test]
    fn a_filter_that_cannot_be_understood_is_reported() {
        let lib = analysed_library();
        assert!(matches!(
            lib.smart_tracks("colour = red", 50),
            Err(LibraryError::BadFilter(_))
        ));
    }

    /// Stored at the moment it is typed, so a broken filter is not discovered
    /// the next time the folder is opened.
    #[test]
    fn a_smart_folders_query_is_checked_before_it_is_saved() {
        let lib = analysed_library();
        let smart = lib
            .create_playlist("Fast", None, PlaylistKind::Smart, Some("bpm > 120"), 0)
            .unwrap();

        assert!(matches!(
            lib.set_playlist_query(smart, "colour = red"),
            Err(LibraryError::BadFilter(_))
        ));
        assert_eq!(
            titles(&lib.smart_playlist_tracks(smart, 50).unwrap()),
            vec!["Gasolina", "Bachata Rosa"],
            "the refused edit must not have replaced the working filter"
        );

        lib.set_playlist_query(smart, "bpm < 100").unwrap();
        assert_eq!(
            titles(&lib.smart_playlist_tracks(smart, 50).unwrap()),
            vec!["Vivir Mi Vida"]
        );
    }

    /// A smart folder follows the collection: it is a question, not a list.
    #[test]
    fn a_smart_folder_picks_up_tracks_added_later() {
        let lib = analysed_library();
        let smart = lib
            .create_playlist("Fast", None, PlaylistKind::Smart, Some("bpm > 130"), 0)
            .unwrap();
        assert_eq!(lib.smart_playlist_tracks(smart, 50).unwrap().len(), 1);

        let mut new = track(7, "Later", "Somebody");
        new.tags.genre = Some("latin".to_owned());
        lib.upsert_track(&new).unwrap();
        lib.set_analysis(
            id(7),
            &StoredAnalysis::default().with_beatgrid(Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(150.0).unwrap(),
                Confidence::CERTAIN,
            )),
        )
        .unwrap();

        assert_eq!(lib.smart_playlist_tracks(smart, 50).unwrap().len(), 2);
    }

    #[test]
    fn asking_a_normal_playlist_for_smart_results_says_no() {
        let lib = analysed_library();
        let list = lib
            .create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();
        assert!(matches!(
            lib.smart_playlist_tracks(list, 50),
            Err(LibraryError::NoSuchPlaylist(_))
        ));
    }

    // -- system playlists --------------------------------------------------

    #[test]
    fn a_system_playlist_is_made_once_and_found_again() {
        let lib = library();
        let first = lib.system_playlist("sidelist", 0).unwrap();
        let second = lib.system_playlist("sidelist", 100).unwrap();
        assert_eq!(first, second, "asking twice must not make two");
    }

    /// The point of marking it: the Sidelist is not a crate the DJ made, and
    /// showing it in the tree beside their folders would be wrong.
    #[test]
    fn a_system_playlist_is_not_in_the_crate_tree() {
        let lib = library();
        lib.system_playlist("sidelist", 0).unwrap();
        lib.create_playlist("Friday", None, PlaylistKind::List, None, 0)
            .unwrap();

        let names: Vec<String> = lib
            .playlists()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["Friday"]);
    }

    /// ...but it is a playlist in every other way, which is the whole reason
    /// it is one.
    #[test]
    fn a_system_playlist_holds_tracks_in_order_like_any_other() {
        let lib = library();
        for n in 1..=2 {
            lib.upsert_track(&track(n, &format!("T{n}"), "X")).unwrap();
        }
        let side = lib.system_playlist("sidelist", 0).unwrap();
        lib.add_to_playlist(side, id(2)).unwrap();
        lib.add_to_playlist(side, id(1)).unwrap();

        assert_eq!(
            lib.playlist_tracks(side)
                .unwrap()
                .iter()
                .map(|(_, t)| t.id)
                .collect::<Vec<_>>(),
            vec![id(2), id(1)]
        );
    }

    /// Two panels must not share one list.
    #[test]
    fn different_system_names_are_different_lists() {
        let lib = library();
        let side = lib.system_playlist("sidelist", 0).unwrap();
        let automix = lib.system_playlist("automix", 0).unwrap();
        assert_ne!(side, automix);
    }

    // -- editing -----------------------------------------------------------

    #[test]
    fn a_batch_edit_sets_the_fields_it_names_and_no_others() {
        let lib = library();
        for n in 1..=3 {
            let mut t = track(n, &format!("T{n}"), "X");
            t.tags.album = Some("Original".to_owned());
            lib.upsert_track(&t).unwrap();
        }

        let changed = lib
            .edit_tracks(
                &[id(1), id(2)],
                &TrackEdit {
                    genre: Some("Bachata".to_owned()),
                    colour: Some("#ff0000".to_owned()),
                    rating: Some(5),
                    ..TrackEdit::default()
                },
            )
            .unwrap();
        assert_eq!(changed, 2);

        for byte in [1u8, 2] {
            let found = lib.track(id(byte)).unwrap().unwrap();
            assert_eq!(found.tags.genre.as_deref(), Some("Bachata"));
            assert_eq!(found.stats.rating, Some(5));
            assert_eq!(
                found.tags.album.as_deref(),
                Some("Original"),
                "a field the edit did not name must be left alone"
            );
        }
        assert_eq!(lib.track(id(3)).unwrap().unwrap().tags.genre, None);
    }

    /// The interface that would eat a collection in one click: `None` meaning
    /// "clear it" rather than "leave it".
    #[test]
    fn an_empty_edit_changes_nothing() {
        let lib = library();
        let mut t = track(1, "T", "X");
        t.tags.genre = Some("Bachata".to_owned());
        lib.upsert_track(&t).unwrap();

        assert_eq!(lib.edit_tracks(&[id(1)], &TrackEdit::default()).unwrap(), 0);
        assert_eq!(
            lib.track(id(1)).unwrap().unwrap().tags.genre.as_deref(),
            Some("Bachata")
        );
    }

    #[test]
    fn editing_no_tracks_changes_nothing() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        let edit = TrackEdit {
            genre: Some("Bachata".to_owned()),
            ..TrackEdit::default()
        };
        assert_eq!(lib.edit_tracks(&[], &edit).unwrap(), 0);
    }

    /// Clearing is its own verb, because "set this to nothing" is a different
    /// intention from "leave this alone".
    #[test]
    fn clearing_a_field_empties_it_across_the_selection() {
        let lib = library();
        for n in 1..=2 {
            let mut t = track(n, &format!("T{n}"), "X");
            t.tags.genre = Some("Wrong".to_owned());
            t.tags.label = Some("Keep".to_owned());
            lib.upsert_track(&t).unwrap();
        }

        lib.clear_field(&[id(1), id(2)], EditableField::Genre)
            .unwrap();

        for byte in [1u8, 2] {
            let found = lib.track(id(byte)).unwrap().unwrap();
            assert_eq!(found.tags.genre, None);
            assert_eq!(
                found.tags.label.as_deref(),
                Some("Keep"),
                "clearing one field must not touch another"
            );
        }
    }

    #[test]
    fn a_field_name_that_is_not_editable_is_refused() {
        assert_eq!(
            EditableField::from_name("genre"),
            Some(EditableField::Genre)
        );
        assert_eq!(
            EditableField::from_name("color"),
            Some(EditableField::Colour)
        );
        assert_eq!(EditableField::from_name("bpm"), None);
        assert_eq!(EditableField::from_name("id"), None);
        assert_eq!(EditableField::from_name("path; DROP TABLE tracks"), None);
    }

    // -- duplicates --------------------------------------------------------

    /// The thing identity throws away and this gets back: which files hold the
    /// same music.
    #[test]
    fn the_same_audio_in_two_folders_is_one_track_and_two_paths() {
        let lib = library();
        let mut first = track(1, "Bachata Rosa", "Juan Luis Guerra");
        first.path = PathBuf::from("/music/a.flac");
        first.added_at = 100;
        lib.upsert_track(&first).unwrap();

        let mut second = first.clone();
        second.path = PathBuf::from("/music/backup/a.flac");
        second.added_at = 200;
        lib.upsert_track(&second).unwrap();

        assert_eq!(lib.track_count().unwrap(), 1, "one piece of audio, one row");

        let duplicates = lib.duplicates(10).unwrap();
        assert_eq!(duplicates.len(), 1);
        let (found, paths) = &duplicates[0];
        assert_eq!(found.id, id(1));
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/music/a.flac")));
        assert!(paths.contains(&PathBuf::from("/music/backup/a.flac")));
    }

    /// A track's own path is recorded the first time it is seen, not only when
    /// a second copy turns up. Getting this wrong would mean a duplicate showed
    /// only one path -- the newer one -- which is the copy you want to keep.
    #[test]
    fn a_tracks_first_path_is_recorded_too() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        assert_eq!(lib.paths_for(id(1)).unwrap().len(), 1);
    }

    #[test]
    fn a_track_seen_once_is_not_a_duplicate() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        assert!(lib.duplicates(10).unwrap().is_empty());
    }

    /// Re-scanning the same file must not make it look like two.
    #[test]
    fn seeing_the_same_path_again_is_not_a_duplicate() {
        let lib = library();
        let t = track(1, "A", "B");
        lib.upsert_track(&t).unwrap();
        lib.upsert_track(&t).unwrap();
        lib.upsert_track(&t).unwrap();

        assert_eq!(lib.paths_for(id(1)).unwrap().len(), 1);
        assert!(lib.duplicates(10).unwrap().is_empty());
    }

    #[test]
    fn paths_come_back_newest_first() {
        let lib = library();
        let mut first = track(1, "A", "B");
        first.path = PathBuf::from("/music/old.flac");
        first.added_at = 100;
        lib.upsert_track(&first).unwrap();

        let mut second = first.clone();
        second.path = PathBuf::from("/music/new.flac");
        second.added_at = 200;
        lib.upsert_track(&second).unwrap();

        let paths = lib.paths_for(id(1)).unwrap();
        assert_eq!(paths[0].0, PathBuf::from("/music/new.flac"));
    }

    #[test]
    fn forgetting_a_path_leaves_the_track_and_the_other_copy() {
        let lib = library();
        let mut first = track(1, "A", "B");
        first.path = PathBuf::from("/music/keep.flac");
        first.added_at = 100;
        lib.upsert_track(&first).unwrap();
        let mut second = first.clone();
        second.path = PathBuf::from("/music/spare.flac");
        second.added_at = 200;
        lib.upsert_track(&second).unwrap();

        lib.forget_path(id(1), Path::new("/music/spare.flac"))
            .unwrap();

        assert_eq!(lib.track_count().unwrap(), 1, "the music is not deleted");
        let paths = lib.paths_for(id(1)).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, PathBuf::from("/music/keep.flac"));
    }

    /// Forgetting the path the track *plays from* must move it to one that is
    /// left, not leave it pointing at a file nobody has.
    #[test]
    fn forgetting_the_playing_path_moves_the_track_to_another() {
        let lib = library();
        let mut first = track(1, "A", "B");
        first.path = PathBuf::from("/music/keep.flac");
        first.added_at = 100;
        lib.upsert_track(&first).unwrap();
        let mut second = first.clone();
        second.path = PathBuf::from("/music/spare.flac");
        second.added_at = 200;
        lib.upsert_track(&second).unwrap();

        // The track currently plays from the newest, `spare`.
        assert_eq!(
            lib.track(id(1)).unwrap().unwrap().path,
            PathBuf::from("/music/spare.flac")
        );

        lib.forget_path(id(1), Path::new("/music/spare.flac"))
            .unwrap();
        assert_eq!(
            lib.track(id(1)).unwrap().unwrap().path,
            PathBuf::from("/music/keep.flac"),
            "the track must not be left pointing at a file that is gone"
        );
    }

    #[test]
    fn deleting_a_track_takes_its_paths_with_it() {
        let lib = library();
        lib.upsert_track(&track(1, "A", "B")).unwrap();
        lib.with(|conn| {
            conn.execute("DELETE FROM tracks WHERE id = ?1", [id(1).to_hex()])?;
            Ok(())
        })
        .unwrap();
        assert!(lib.paths_for(id(1)).unwrap().is_empty());
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

    // -- sessions ----------------------------------------------------------

    /// A set list runs forwards, unlike the history panel.
    #[test]
    fn a_session_reads_oldest_first() {
        let lib = library();
        lib.upsert_track(&track(1, "First", "X")).unwrap();
        lib.upsert_track(&track(2, "Second", "X")).unwrap();
        lib.record_play(id(1), 100, Some("friday")).unwrap();
        lib.record_play(id(2), 200, Some("friday")).unwrap();

        let set = lib.session("friday").unwrap();
        assert_eq!(
            set.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(),
            vec!["First", "Second"],
            "the history panel is newest first; a set list is not"
        );
    }

    #[test]
    fn a_session_holds_only_its_own_plays() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        lib.record_play(id(1), 100, Some("friday")).unwrap();
        lib.record_play(id(1), 200, Some("saturday")).unwrap();

        assert_eq!(lib.session("friday").unwrap().len(), 1);
        assert!(lib.session("nothing").unwrap().is_empty());
    }

    #[test]
    fn sessions_are_listed_newest_first_with_their_counts() {
        let lib = library();
        lib.upsert_track(&track(1, "T", "X")).unwrap();
        lib.record_play(id(1), 100, Some("friday")).unwrap();
        lib.record_play(id(1), 150, Some("friday")).unwrap();
        lib.record_play(id(1), 900, Some("saturday")).unwrap();
        // A play with no session must not invent one.
        lib.record_play(id(1), 950, None).unwrap();

        let sessions = lib.sessions(10).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].0, "saturday");
        assert_eq!(sessions[1], ("friday".to_owned(), 2, 150));
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

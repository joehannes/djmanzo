//! The library, and the worker that fills it.
//!
//! # Why the handle exists
//!
//! [`dj_library::Library`] is opened at a path, and the path is only knowable
//! once Tauri has resolved the application's config directory — which happens
//! in `setup`, after [`crate::state::AppState`] has been built and handed over
//! to `manage`. So the state holds a handle whose database can be swapped once,
//! rather than an `Option` every call site would have to unwrap.
//!
//! The library starts in memory. That is not a fallback hiding a failure: a
//! machine with nowhere writable should still scan, browse and play — it just
//! forgets between runs, and says so.

use dj_library::{Library, LibraryError, LibraryTrack, ScannedFile};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// The library, swappable once at startup.
#[derive(Debug)]
pub struct LibraryHandle {
    inner: RwLock<Arc<Library>>,
    /// Where it lives, or `None` while it is in memory. Shown in the interface,
    /// because "your library will not survive a restart" is not something to
    /// discover later.
    path: RwLock<Option<PathBuf>>,
}

impl LibraryHandle {
    /// A handle over an in-memory library.
    ///
    /// Falls back to a second in-memory attempt only in the sense that a
    /// failure here is fatal to the library and nothing else: the rest of the
    /// application keeps working, and every library call reports the error.
    pub fn in_memory() -> Result<Self, LibraryError> {
        Ok(Self {
            inner: RwLock::new(Arc::new(Library::in_memory()?)),
            path: RwLock::new(None),
        })
    }

    /// Point the handle at a file, keeping nothing from the in-memory one.
    ///
    /// Nothing is carried over on purpose. This runs milliseconds after
    /// startup, before any scan, so the in-memory database is empty — and
    /// copying rows between two databases is a migration path that would exist
    /// only to serve a case that cannot happen.
    pub fn open_at(&self, path: &Path) -> Result<(), LibraryError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let opened = Arc::new(Library::open(path)?);
        if let Ok(mut slot) = self.inner.write() {
            *slot = opened;
        }
        if let Ok(mut slot) = self.path.write() {
            *slot = Some(path.to_path_buf());
        }
        tracing::info!(?path, "library opened");
        Ok(())
    }

    /// The database itself.
    pub fn get(&self) -> Result<Arc<Library>, LibraryError> {
        self.inner
            .read()
            .map(|guard| Arc::clone(&guard))
            .map_err(|_| LibraryError::Poisoned)
    }

    /// Where the library is stored, or `None` if it is in memory only.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.path.read().ok().and_then(|p| p.clone())
    }
}

/// How many files the identifier decodes before checking whether it should
/// stop.
///
/// One at a time. Decoding is the expensive part and a DJ may quit, or start
/// playing, at any moment; a batch would mean waiting for the batch.
const IDENTIFY_BATCH: usize = 1;

/// How long the identifier waits when there is nothing to do.
///
/// Two seconds is short enough that a scan finishing feels immediate and long
/// enough that an idle application is not waking a thread constantly.
const IDLE_SLEEP: std::time::Duration = std::time::Duration::from_secs(2);

/// Progress, for the interface.
#[derive(Debug, Default)]
pub struct IdentifyProgress {
    /// Files identified since the worker started.
    pub done: AtomicUsize,
    /// Files that could not be identified.
    pub failed: AtomicUsize,
    /// True while a file is actually being decoded, as opposed to waiting.
    pub working: AtomicBool,
}

/// The background worker that turns scanned files into tracks.
///
/// # Why this is a thread rather than a task per file
///
/// Identification is a decode: seconds of CPU and a hundred megabytes of
/// allocation per file. Spawning one per pending file would put a whole
/// collection in flight at once and take the machine down. One worker, one file
/// at a time, is both enough — the queue is hours long either way — and the only
/// shape that leaves the machine usable while it runs.
#[derive(Debug)]
pub struct Identifier {
    stop: Arc<AtomicBool>,
    progress: Arc<IdentifyProgress>,
}

impl Identifier {
    /// Start the worker.
    ///
    /// `decode` is passed in rather than called directly so this can be tested
    /// without a real audio file: identification is a queue-management problem
    /// and a decoding problem, and only the first one is interesting here.
    pub fn start<F>(library: Arc<LibraryHandle>, now: fn() -> i64, decode: F) -> Self
    where
        F: Fn(&ScannedFile) -> Result<Identified, String> + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(IdentifyProgress::default());

        let worker_stop = Arc::clone(&stop);
        let worker_progress = Arc::clone(&progress);
        std::thread::Builder::new()
            .name("djmanzo-identify".to_owned())
            .spawn(move || {
                run(&library, &worker_stop, &worker_progress, now, decode);
            })
            .map_err(|error| tracing::warn!(%error, "could not start the library identifier"))
            .ok();

        Self { stop, progress }
    }

    #[must_use]
    pub fn progress(&self) -> Arc<IdentifyProgress> {
        Arc::clone(&self.progress)
    }

    /// Ask the worker to finish the file it is on and stop.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Identifier {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run<F>(
    library: &Arc<LibraryHandle>,
    stop: &AtomicBool,
    progress: &IdentifyProgress,
    now: fn() -> i64,
    decode: F,
) where
    F: Fn(&ScannedFile) -> Result<Identified, String>,
{
    while !stop.load(Ordering::Relaxed) {
        let Ok(db) = library.get() else {
            // The library is unavailable, which is not something waiting fixes.
            return;
        };
        let batch = match db.next_pending(IDENTIFY_BATCH) {
            Ok(batch) => batch,
            Err(error) => {
                tracing::warn!(%error, "could not read the identify queue");
                return;
            }
        };
        if batch.is_empty() {
            progress.working.store(false, Ordering::Relaxed);
            // Dropping the handle before sleeping matters: holding an `Arc` to
            // the database across the idle wait would keep a swapped-out
            // in-memory library alive for as long as the application runs.
            drop(db);
            std::thread::sleep(IDLE_SLEEP);
            continue;
        }

        progress.working.store(true, Ordering::Relaxed);
        for file in batch {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            match decode(&file) {
                Ok(Identified { mut track, found }) => {
                    track.added_at = now();
                    if let Err(error) = db.promote_pending_with(&track, &found) {
                        tracing::warn!(%error, path = ?file.path, "could not store track");
                    } else {
                        progress.done.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(reason) => {
                    tracing::debug!(path = ?file.path, %reason, "could not identify");
                    let _ = db.mark_pending_failed(&file.path, &reason);
                    progress.failed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Decode a file, identify it, and analyse it.
///
/// The real `decode` for [`Identifier::start`]. Split out so the worker's queue
/// handling can be tested without touching a codec.
///
/// # Why analysis happens here rather than later
///
/// The expensive part of analysis is having the decoded samples in memory, and
/// they already are: identifying the track required decoding it. Leaving
/// analysis until the track is loaded onto a deck would mean decoding the same
/// file twice, and — worse — it would mean a DJ who imported their collection
/// last night still has no BPM or key to sort by this evening. A library you
/// cannot sort by tempo is most of the reason to have one.
/// A file, decoded and read.
///
/// Two things rather than one, because they come from different places and
/// answer to different rules: `track` is what we measured, and `found` is what
/// somebody else already decided about this record and wrote into it.
#[derive(Debug, Clone)]
pub struct Identified {
    pub track: LibraryTrack,
    pub found: dj_library::import::ImportPayload,
}

pub fn identify_file(file: &ScannedFile) -> Result<Identified, String> {
    let decoded = dj_decode::decode_file(&file.path).map_err(|e| e.to_string())?;
    let sample_rate = decoded.buffer.sample_rate();
    let analysis = dj_analysis::analyse(decoded.buffer.as_interleaved(), sample_rate);

    // What the file itself carries. Read before the buffer is dropped so the
    // cost lands on a file that has just been decoded anyway.
    let markers = dj_library::import::markers::read_file(&file.path);
    let found = dj_library::import::ImportPayload {
        cues: markers.cues,
        loops: markers.loops,
        ..dj_library::import::ImportPayload::default()
    };

    let track = LibraryTrack {
        id: decoded.id,
        path: file.path.clone(),
        // Tags from the scan, not from the decoder. The scan used `lofty`,
        // which reads fields symphonia does not carry -- album artist, label,
        // year -- and re-reading them here would lose those.
        tags: file.tags.clone(),
        duration_frames: decoded.buffer.len_frames() as u64,
        sample_rate,
        channels: 2,
        file_size: file.file_size,
        file_modified: file.file_modified,
        // Overwritten by the worker, which owns the clock.
        added_at: 0,
        analysis: stored_analysis(&analysis),
        stats: dj_library::PlayStats::default(),
        colour: None,
    };
    Ok(Identified { track, found })
}

/// Flatten an analysis into the shape the library stores.
///
/// A missing tempo or key stays missing. The library distinguishes "not
/// analysed" from "analysed and inconclusive" only by whether the row exists at
/// all -- so a track the analyser could not read gets a row with null columns,
/// which is the honest answer and is what the browser shows as a blank rather
/// than a made-up zero.
pub fn stored_analysis(analysis: &dj_analysis::Analysis) -> dj_library::StoredAnalysis {
    let mut stored = dj_library::StoredAnalysis {
        // Silence measures as negative infinity, which is a real answer and an
        // unstorable number: it does not survive JSON on the way to the
        // interface, and a database column holding it would poison every
        // comparison it took part in. Absent is the honest encoding -- there is
        // no loudness to record.
        loudness_lufs: (!analysis.loudness.is_silent()).then(|| analysis.loudness.get()),
        ..dj_library::StoredAnalysis::default()
    };
    if let Some(tempo) = &analysis.tempo {
        stored = stored.with_beatgrid(tempo.grid);
    }
    if let Some(key) = &analysis.key {
        stored = stored.with_key(key.key, key.correlation);
    }
    stored
}

/// Unix seconds. One place, so every timestamp in the library agrees.
#[must_use]
pub fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{SampleRate, TrackId};
    use dj_library::Tags;
    use std::sync::atomic::AtomicU8;
    use std::time::{Duration, Instant};

    fn handle() -> Arc<LibraryHandle> {
        Arc::new(LibraryHandle::in_memory().unwrap())
    }

    fn scanned(path: &str) -> ScannedFile {
        ScannedFile {
            path: PathBuf::from(path),
            tags: Tags {
                title: Some(path.to_owned()),
                ..Tags::default()
            },
            file_size: Some(1),
            file_modified: Some(1),
        }
    }

    /// A track as a successful decode would produce, with the id derived from
    /// the path so different files get different ids.
    fn identified(file: &ScannedFile) -> Identified {
        Identified {
            track: library_track(file),
            found: dj_library::import::ImportPayload::default(),
        }
    }

    fn library_track(file: &ScannedFile) -> LibraryTrack {
        let mut bytes = [0u8; 32];
        for (slot, byte) in bytes.iter_mut().zip(file.path.to_string_lossy().bytes()) {
            *slot = byte;
        }
        LibraryTrack {
            id: TrackId::from_bytes(bytes),
            path: file.path.clone(),
            tags: file.tags.clone(),
            duration_frames: 48_000,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: file.file_size,
            file_modified: file.file_modified,
            added_at: 0,
            analysis: dj_library::StoredAnalysis::default(),
            stats: dj_library::PlayStats::default(),
            colour: None,
        }
    }

    /// Wait for a condition, or give up.
    ///
    /// The worker is a real thread, so a test has to wait for it. Polling with
    /// a deadline rather than sleeping a fixed time: a fixed sleep is either
    /// slow or flaky, and usually both on a loaded machine.
    fn until(deadline_ms: u64, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        condition()
    }

    #[test]
    fn a_handle_starts_in_memory_and_says_so() {
        let handle = LibraryHandle::in_memory().unwrap();
        assert_eq!(
            handle.path(),
            None,
            "the interface warns on this, so it must be honest"
        );
        assert!(handle.get().is_ok());
    }

    #[test]
    fn opening_at_a_path_creates_the_file_and_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/library.db");
        let handle = LibraryHandle::in_memory().unwrap();
        handle.open_at(&path).unwrap();

        assert_eq!(handle.path(), Some(path.clone()));
        assert!(path.exists(), "the parent directory must be created");
    }

    /// The point of the swap: what is written afterwards lands in the file.
    #[test]
    fn writes_after_the_swap_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");

        let handle = LibraryHandle::in_memory().unwrap();
        handle.open_at(&path).unwrap();
        handle
            .get()
            .unwrap()
            .record_pending(&scanned("/music/a.mp3"), 0)
            .unwrap();
        drop(handle);

        let reopened = dj_library::Library::open(&path).unwrap();
        assert_eq!(reopened.pending_count().unwrap(), 1);
    }

    #[test]
    fn the_worker_drains_the_queue() {
        let handle = handle();
        let db = handle.get().unwrap();
        for name in ["a", "b", "c"] {
            db.record_pending(&scanned(&format!("/music/{name}.mp3")), 0)
                .unwrap();
        }

        let worker = Identifier::start(Arc::clone(&handle), || 42, |file| Ok(identified(file)));
        let progress = worker.progress();

        assert!(
            until(2000, || progress.done.load(Ordering::Relaxed) == 3),
            "the worker must identify every queued file"
        );
        assert_eq!(db.pending_count().unwrap(), 0);
        assert_eq!(db.track_count().unwrap(), 3);
    }

    #[test]
    fn the_worker_stamps_the_time_it_finished() {
        let handle = handle();
        let db = handle.get().unwrap();
        db.record_pending(&scanned("/music/a.mp3"), 0).unwrap();

        let worker = Identifier::start(
            Arc::clone(&handle),
            || 1_700_000_000,
            |file| Ok(identified(file)),
        );
        let progress = worker.progress();
        assert!(until(2000, || progress.done.load(Ordering::Relaxed) == 1));

        let track = db.all_tracks(10).unwrap().pop().unwrap();
        assert_eq!(
            track.added_at, 1_700_000_000,
            "the worker owns the clock, not the decoder"
        );
    }

    /// The failure that would matter most: one corrupt file at the head of the
    /// queue stalling the whole collection behind it.
    #[test]
    fn a_file_that_cannot_be_decoded_does_not_block_the_queue() {
        let handle = handle();
        let db = handle.get().unwrap();
        db.record_pending(&scanned("/music/broken.mp3"), 0).unwrap();
        db.record_pending(&scanned("/music/fine.mp3"), 1).unwrap();

        let worker = Identifier::start(
            Arc::clone(&handle),
            || 0,
            |file| {
                if file.path.to_string_lossy().contains("broken") {
                    Err("no decodable audio".to_owned())
                } else {
                    Ok(identified(file))
                }
            },
        );
        let progress = worker.progress();

        assert!(
            until(2000, || progress.done.load(Ordering::Relaxed) == 1
                && progress.failed.load(Ordering::Relaxed) == 1),
            "the good file must be identified despite the broken one ahead of it"
        );
        assert_eq!(db.track_count().unwrap(), 1);
        assert_eq!(db.failed_pending().unwrap().len(), 1);
    }

    /// The worker must not spin on an empty queue. Without the idle wait it
    /// would run a database query as fast as the CPU allows, all night.
    #[test]
    fn an_empty_queue_does_not_spin() {
        let handle = handle();
        let calls = Arc::new(AtomicU8::new(0));
        let counted = Arc::clone(&calls);

        let _worker = Identifier::start(
            Arc::clone(&handle),
            || 0,
            move |file| {
                counted.fetch_add(1, Ordering::Relaxed);
                Ok(identified(file))
            },
        );

        std::thread::sleep(Duration::from_millis(120));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "nothing was queued, so nothing should have been decoded"
        );
    }

    /// A queue that fills after the worker has gone idle must still be drained,
    /// or a folder added a minute after startup would never be identified.
    #[test]
    fn work_queued_while_idle_is_picked_up() {
        let handle = handle();
        let worker = Identifier::start(Arc::clone(&handle), || 0, |file| Ok(identified(file)));
        let progress = worker.progress();

        // Let it reach the idle wait, then queue something.
        std::thread::sleep(Duration::from_millis(50));
        handle
            .get()
            .unwrap()
            .record_pending(&scanned("/music/late.mp3"), 0)
            .unwrap();

        assert!(
            until(6000, || progress.done.load(Ordering::Relaxed) == 1),
            "a file queued during the idle wait must still be identified"
        );
    }

    /// Silence measures as negative infinity. It must not reach the database or
    /// the interface: `serde_json` refuses non-finite floats, so a single silent
    /// track would break the whole browser payload.
    #[test]
    fn a_silent_track_stores_no_loudness_rather_than_negative_infinity() {
        let analysis = dj_analysis::Analysis {
            tempo: None,
            key: None,
            loudness: dj_analysis::Lufs::SILENCE,
        };
        let stored = stored_analysis(&analysis);
        assert_eq!(stored.loudness_lufs, None);
        assert!(serde_json::to_string(&stored).is_ok());
    }

    #[test]
    fn a_measurable_track_keeps_its_loudness() {
        let analysis = dj_analysis::Analysis {
            tempo: None,
            key: None,
            loudness: dj_analysis::Lufs::new(-9.5),
        };
        assert_eq!(stored_analysis(&analysis).loudness_lufs, Some(-9.5));
    }

    /// A track the analyser could not read gets a row with null columns, not a
    /// row of zeros. The browser shows a blank; a fabricated 0.0 BPM would be
    /// read at a glance as a real number.
    #[test]
    fn an_unreadable_track_stores_nothing_rather_than_zeros() {
        let stored = stored_analysis(&dj_analysis::Analysis {
            tempo: None,
            key: None,
            loudness: dj_analysis::Lufs::new(-12.0),
        });
        assert_eq!(stored.bpm, None);
        assert_eq!(stored.beatgrid(), None);
        assert_eq!(stored.key(), None);
        assert!(!stored.is_complete());
    }

    #[test]
    fn stopping_the_worker_ends_it() {
        let handle = handle();
        let db = handle.get().unwrap();
        for n in 0..50 {
            db.record_pending(&scanned(&format!("/music/{n}.mp3")), 0)
                .unwrap();
        }

        let worker = Identifier::start(
            Arc::clone(&handle),
            || 0,
            |file| {
                std::thread::sleep(Duration::from_millis(20));
                Ok(identified(file))
            },
        );
        let progress = worker.progress();
        assert!(until(2000, || progress.done.load(Ordering::Relaxed) >= 1));

        worker.stop();
        let at_stop = progress.done.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            progress.done.load(Ordering::Relaxed) <= at_stop + 1,
            "the worker may finish the file it is on, and no more"
        );
    }
}

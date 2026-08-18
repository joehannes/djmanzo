//! Keeping a deck's cues and grid in the library.
//!
//! # What is remembered, and how it gets there
//!
//! Cues and grid edits belong to the *track*, not the deck: load the same
//! record next week and the cue you dropped on the downbeat should still be
//! there. So both are written into the library keyed by content hash, and read
//! back when the track is loaded again.
//!
//! They arrive by two different routes, because they are two different shapes
//! of event:
//!
//! - A **grid edit** is a discrete action with a known result. It is written
//!   where it happens, in `commands::apply_grid_edit`.
//! - A **hot cue** is set by the *engine*, at a playhead the host does not know
//!   until the audio thread has published it — quantize may have moved it. So
//!   cues are noticed by watching the snapshot, which is the one place that
//!   reads engine state after the fact.
//!
//! # Why the writes are on their own thread
//!
//! The snapshot pump runs at 60 Hz. A SQLite write is usually under a
//! millisecond, but "usually" is doing a lot of work on a laptop whose disk is
//! busy, and a stalled pump is a frozen interface. The watcher compares, and
//! hands anything that changed to a writer thread through a queue.

use dj_core::{FramePos, HOT_CUE_SLOTS, TrackId};
use dj_library::{StoredCue, StoredLoop};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, TrySendError};

/// One thing to write.
#[derive(Debug, Clone)]
pub enum Write {
    Cues {
        track: TrackId,
        cues: Vec<StoredCue>,
    },
    Loops {
        track: TrackId,
        loops: Vec<StoredLoop>,
    },
    /// A track has been played. Bumps the count and appends to the history.
    Play {
        track: TrackId,
        at: i64,
        session: Option<String>,
    },
}

/// How many pending writes to hold.
///
/// Small on purpose. Each is a handful of rows and the writer keeps up easily;
/// a deep queue would only mean holding more stale state after a disk stall,
/// and the watcher re-sends anything it could not enqueue.
const QUEUE_DEPTH: usize = 32;

/// Writes the library in the background.
#[derive(Debug, Clone)]
pub struct LibraryWriter {
    tx: SyncSender<Write>,
}

impl LibraryWriter {
    /// Start the writer thread.
    pub fn start(library: Arc<crate::library::LibraryHandle>) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE_DEPTH);
        std::thread::Builder::new()
            .name("djmanzo-library-write".to_owned())
            .spawn(move || {
                // Ends when every sender is dropped, which is when the
                // application is shutting down.
                for write in rx {
                    let Ok(db) = library.get() else { return };
                    let result = match &write {
                        Write::Cues { track, cues } => db.set_cues(*track, cues),
                        Write::Loops { track, loops } => db.set_loops(*track, loops),
                        Write::Play { track, at, session } => {
                            db.record_play(*track, *at, session.as_deref())
                        }
                    };
                    if let Err(error) = result {
                        tracing::warn!(%error, "could not save deck state");
                    }
                }
            })
            .map_err(|error| tracing::warn!(%error, "could not start the library writer"))
            .ok();

        Self { tx }
    }

    /// Queue a write. Returns false when the queue is full.
    ///
    /// The caller is expected to *not* record the state as saved when this
    /// returns false, so the next tick tries again. Blocking here would stall
    /// the snapshot pump, and dropping silently would lose a cue.
    pub fn send(&self, write: Write) -> bool {
        match self.tx.try_send(write) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => false,
            // The writer thread is gone. Nothing to retry against.
            Err(TrySendError::Disconnected(_)) => true,
        }
    }
}

/// Watches decks for cue changes worth saving.
///
/// Holds the last set successfully queued per deck, so an unchanged deck costs
/// one comparison of eight optional floats per tick and nothing else.
#[derive(Debug, Default)]
pub struct CueWatcher {
    /// Keyed by deck number. The track is part of the value rather than the key
    /// so that loading a different track onto the same deck is seen as a change
    /// rather than as the old track's cues being cleared.
    saved: HashMap<u8, (TrackId, Vec<Option<f32>>)>,
}

impl CueWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Note what a deck's cues are now, queueing a write if they moved.
    ///
    /// `track` is `None` for an empty deck, which forgets the deck rather than
    /// writing: ejecting is not the same as clearing every cue, and treating it
    /// that way would wipe a track's cues every time it left a deck.
    pub fn observe(
        &mut self,
        deck: u8,
        track: Option<TrackId>,
        cues: &[Option<f32>],
        writer: &LibraryWriter,
    ) {
        let Some(track) = track else {
            self.saved.remove(&deck);
            return;
        };

        if let Some((known_track, known_cues)) = self.saved.get(&deck)
            && *known_track == track
            && known_cues == cues
        {
            return;
        }

        // A freshly loaded track is not a change to save. Its cues came *from*
        // the library moments ago, and writing them straight back would be a
        // round trip for nothing -- and, on the tick before the restore lands,
        // would write an empty set over the cues being restored.
        let first_sight = self.saved.get(&deck).is_none_or(|(id, _)| *id != track);
        if first_sight {
            self.saved.insert(deck, (track, cues.to_vec()));
            return;
        }

        if writer.send(Write::Cues {
            track,
            cues: to_stored(cues),
        }) {
            self.saved.insert(deck, (track, cues.to_vec()));
        }
    }

    /// Forget a deck, so the next observation is treated as a fresh load.
    pub fn forget(&mut self, deck: u8) {
        self.saved.remove(&deck);
    }
}

/// How long a track has to have been playing before it counts as played.
///
/// Thirty seconds, or a quarter of the track if it is shorter. A DJ auditions
/// tracks constantly — loading one, hearing four bars, loading another — and a
/// history full of those is a history nobody can read. Thirty seconds is past
/// the point where you are still deciding.
const PLAY_THRESHOLD_SECONDS: f64 = 30.0;

/// The fraction of a short track that counts instead.
const PLAY_THRESHOLD_FRACTION: f64 = 0.25;

/// Watches decks for tracks that have actually been played.
///
/// # Why position rather than elapsed time
///
/// Measuring how long the deck has been in the playing state would count a
/// track paused at the drop for five minutes, and would not count one played
/// from a cue point at double speed. The playhead is what the room heard.
#[derive(Debug, Default)]
pub struct PlayWatcher {
    /// Tracks already recorded for the current load, keyed by deck. Cleared
    /// when a different track arrives, so playing the same record twice in a
    /// night is two rows -- which is what a history is for.
    counted: HashMap<u8, TrackId>,
}

impl PlayWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Note where a deck is, returning the track if this is the moment it
    /// counts as played.
    ///
    /// Returns rather than writing, so the caller decides what a play means --
    /// the history row, the play count, and eventually the assistant's memory
    /// of the night.
    /// `playing` is separate from `track` on purpose. An empty deck forgets
    /// what it counted; a *paused* one must not, or every pause and resume
    /// would be another row in the history.
    pub fn observe(
        &mut self,
        deck: u8,
        track: Option<TrackId>,
        playing: bool,
        position_seconds: f64,
        duration_seconds: f64,
    ) -> Option<TrackId> {
        let Some(track) = track else {
            self.counted.remove(&deck);
            return None;
        };

        if self.counted.get(&deck) == Some(&track) {
            return None;
        }
        // A different track on this deck: whatever was counted no longer
        // applies, and this one has not been.
        if self.counted.contains_key(&deck) {
            self.counted.remove(&deck);
        }

        // A deck parked past the threshold with the track paused has not been
        // played to anybody.
        if !playing || !crossed_threshold(position_seconds, duration_seconds) {
            return None;
        }
        self.counted.insert(deck, track);
        Some(track)
    }

    /// Forget a deck, so the next load counts afresh.
    pub fn forget(&mut self, deck: u8) {
        self.counted.remove(&deck);
    }
}

/// Whether the playhead is far enough in to call it a play.
fn crossed_threshold(position: f64, duration: f64) -> bool {
    if !position.is_finite() || position <= 0.0 {
        return false;
    }
    let threshold = if duration.is_finite() && duration > 0.0 {
        PLAY_THRESHOLD_SECONDS.min(duration * PLAY_THRESHOLD_FRACTION)
    } else {
        // No duration yet. Fall back to the flat threshold rather than
        // counting immediately -- an unknown length is not a short track.
        PLAY_THRESHOLD_SECONDS
    };
    position >= threshold
}

/// Turn the snapshot's representation into rows.
///
/// The snapshot uses `None` for an empty slot and a frame position otherwise;
/// the library stores one row per occupied slot. Frame zero is a real cue
/// position in both, and must not become an absence in either.
#[must_use]
pub fn to_stored(cues: &[Option<f32>]) -> Vec<StoredCue> {
    cues.iter()
        .enumerate()
        .filter_map(|(index, cue)| {
            Some(StoredCue {
                slot: u8::try_from(index + 1).ok()?,
                frame: f64::from((*cue)?),
                label: None,
                colour: None,
            })
        })
        .collect()
}

/// Turn stored rows back into the array the engine takes.
///
/// A slot outside 1..=[`HOT_CUE_SLOTS`] is dropped rather than rejected: the
/// database is old data, and one nonsensical row should cost that cue, not the
/// other seven.
#[must_use]
pub fn from_stored(cues: &[StoredCue]) -> [Option<FramePos>; HOT_CUE_SLOTS] {
    let mut out = [None; HOT_CUE_SLOTS];
    for cue in cues {
        if let Some(index) = cue.slot.checked_sub(1).map(usize::from)
            && let Some(slot) = out.get_mut(index)
        {
            *slot = Some(FramePos::new(cue.frame));
        }
    }
    out
}

#[cfg(test)]
mod play_tests {
    use super::*;

    fn id(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    /// The thing this exists to prevent: a history full of four-bar auditions.
    #[test]
    fn a_track_auditioned_for_a_few_seconds_is_not_a_play() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(watcher.observe(1, Some(id(1)), true, 8.0, 300.0), None);
        assert_eq!(watcher.observe(1, Some(id(1)), true, 29.9, 300.0), None);
    }

    #[test]
    fn a_track_played_past_the_threshold_counts_once() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 30.0, 300.0),
            Some(id(1))
        );
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 60.0, 300.0),
            None,
            "one row per play, not one per tick"
        );
    }

    /// A one-minute sample or a jingle is fully played well before 30 seconds.
    #[test]
    fn a_short_track_counts_at_a_quarter_of_its_length() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 5.1, 20.0),
            Some(id(1))
        );
    }

    #[test]
    fn a_track_at_the_very_start_is_never_a_play() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(watcher.observe(1, Some(id(1)), true, 0.0, 300.0), None);
        assert_eq!(watcher.observe(1, Some(id(1)), true, -1.0, 300.0), None);
    }

    /// An unknown length is not a short track.
    #[test]
    fn a_track_with_no_duration_uses_the_flat_threshold() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(watcher.observe(1, Some(id(1)), true, 5.0, 0.0), None);
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 31.0, 0.0),
            Some(id(1))
        );
    }

    /// Playing the same record twice in a night is two rows. That is what a
    /// history is for.
    #[test]
    fn reloading_the_same_track_counts_again() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );

        // Ejected, then loaded again.
        watcher.observe(1, None, false, 0.0, 0.0);
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );
    }

    #[test]
    fn a_new_track_on_the_same_deck_counts_separately() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );
        assert_eq!(
            watcher.observe(1, Some(id(2)), true, 40.0, 300.0),
            Some(id(2))
        );
    }

    /// The bug this test exists for: a paused deck reporting no track would
    /// forget what it had counted, and every pause and resume would be another
    /// row in the history.
    #[test]
    fn pausing_and_resuming_does_not_record_a_second_play() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );
        // Paused, still loaded.
        assert_eq!(watcher.observe(1, Some(id(1)), false, 40.0, 300.0), None);
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 45.0, 300.0),
            None,
            "resuming is not a new play"
        );
    }

    /// A deck cued past the threshold and left there has not been played.
    #[test]
    fn a_paused_deck_past_the_threshold_is_not_a_play() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(watcher.observe(1, Some(id(1)), false, 120.0, 300.0), None);
    }

    #[test]
    fn decks_are_counted_independently() {
        let mut watcher = PlayWatcher::new();
        assert_eq!(
            watcher.observe(1, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );
        assert_eq!(
            watcher.observe(2, Some(id(1)), true, 40.0, 300.0),
            Some(id(1))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    fn writer() -> (LibraryWriter, Arc<crate::library::LibraryHandle>) {
        let handle = Arc::new(crate::library::LibraryHandle::in_memory().unwrap());
        (LibraryWriter::start(Arc::clone(&handle)), handle)
    }

    /// Put a track in the library.
    ///
    /// Not test scaffolding for its own sake: `cues.track_id` is a foreign key,
    /// so a cue cannot exist without the track it belongs to. That is the
    /// reason `load_track` adds what it loads to the library before the deck is
    /// playable -- without it, every cue a DJ set on a file they opened from
    /// disk would be silently discarded.
    fn with_track(handle: &crate::library::LibraryHandle, track: TrackId) {
        handle
            .get()
            .unwrap()
            .upsert_track(&dj_library::LibraryTrack {
                id: track,
                path: std::path::PathBuf::from(format!("/music/{}.flac", track.to_hex())),
                tags: dj_library::Tags::default(),
                duration_frames: 48_000,
                sample_rate: dj_core::SampleRate::DEFAULT,
                channels: 2,
                file_size: None,
                file_modified: None,
                added_at: 0,
                analysis: dj_library::StoredAnalysis::default(),
                stats: dj_library::PlayStats::default(),
                colour: None,
            })
            .unwrap();
    }

    /// Wait for the writer thread to catch up.
    fn wait_for(db: &dj_library::Library, track: TrackId, count: usize) -> Vec<StoredCue> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            let cues = db.cues(track).unwrap();
            if cues.len() == count {
                return cues;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        db.cues(track).unwrap()
    }

    #[test]
    fn an_empty_slot_is_not_a_cue_at_frame_zero() {
        let stored = to_stored(&[Some(0.0), None, Some(4800.0)]);
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].slot, 1);
        assert_eq!(stored[0].frame, 0.0);
        assert_eq!(stored[1].slot, 3);
    }

    #[test]
    fn cues_survive_the_round_trip_through_storage() {
        let original = [
            Some(0.0f32),
            None,
            Some(4800.0),
            None,
            None,
            None,
            None,
            None,
        ];
        let back = from_stored(&to_stored(&original));
        assert_eq!(back[0], Some(FramePos::new(0.0)));
        assert_eq!(back[1], None);
        assert_eq!(back[2], Some(FramePos::new(4800.0)));
    }

    /// A row the database should not contain must not take the others with it.
    #[test]
    fn a_slot_outside_the_range_is_dropped_not_fatal() {
        let cues = vec![
            StoredCue {
                slot: 0,
                frame: 1.0,
                label: None,
                colour: None,
            },
            StoredCue {
                slot: 99,
                frame: 2.0,
                label: None,
                colour: None,
            },
            StoredCue {
                slot: 2,
                frame: 4800.0,
                label: None,
                colour: None,
            },
        ];
        let back = from_stored(&cues);
        assert_eq!(back[1], Some(FramePos::new(4800.0)));
        assert_eq!(back.iter().filter(|c| c.is_some()).count(), 1);
    }

    /// The first sight of a track must not write. Its cues have just come *out*
    /// of the library, and on the tick before the restore reaches the engine
    /// they are still empty -- writing then would erase them.
    #[test]
    fn loading_a_track_does_not_write_its_cues_straight_back() {
        let (writer, handle) = writer();
        let mut watcher = CueWatcher::new();
        let db = handle.get().unwrap();

        watcher.observe(1, Some(id(1)), &[None; HOT_CUE_SLOTS], &writer);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            db.cues(id(1)).unwrap().is_empty(),
            "nothing should have been written on first sight"
        );
    }

    #[test]
    fn a_cue_set_after_loading_is_written() {
        let (writer, handle) = writer();
        with_track(&handle, id(1));
        let mut watcher = CueWatcher::new();
        let db = handle.get().unwrap();

        let mut cues = vec![None; HOT_CUE_SLOTS];
        watcher.observe(1, Some(id(1)), &cues, &writer);
        cues[0] = Some(4800.0);
        watcher.observe(1, Some(id(1)), &cues, &writer);

        let stored = wait_for(&db, id(1), 1);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].frame, 4800.0);
    }

    /// Unchanged decks must not write. At 60 Hz an idle deck would otherwise be
    /// 60 database writes a second, forever.
    /// Unchanged decks must not write. At 60 Hz an idle deck would otherwise be
    /// 60 database writes a second, forever -- and with a queue 32 deep, a
    /// hundred observations of the same state would overflow it.
    #[test]
    fn an_unchanged_deck_does_not_write_again() {
        let (writer, handle) = writer();
        with_track(&handle, id(1));
        let mut watcher = CueWatcher::new();
        let db = handle.get().unwrap();

        let mut cues = vec![None; HOT_CUE_SLOTS];
        watcher.observe(1, Some(id(1)), &cues, &writer);
        cues[0] = Some(4800.0);
        for _ in 0..100 {
            watcher.observe(1, Some(id(1)), &cues, &writer);
        }

        assert_eq!(wait_for(&db, id(1), 1).len(), 1);
    }

    /// Ejecting is not "the DJ cleared every cue".
    #[test]
    fn ejecting_a_deck_does_not_erase_the_tracks_cues() {
        let (writer, handle) = writer();
        with_track(&handle, id(1));
        let mut watcher = CueWatcher::new();
        let db = handle.get().unwrap();

        let mut cues = vec![None; HOT_CUE_SLOTS];
        watcher.observe(1, Some(id(1)), &cues, &writer);
        cues[0] = Some(4800.0);
        watcher.observe(1, Some(id(1)), &cues, &writer);
        wait_for(&db, id(1), 1);

        // Eject: no track, and the engine clears the deck's cues.
        watcher.observe(1, None, &[None; HOT_CUE_SLOTS], &writer);
        std::thread::sleep(std::time::Duration::from_millis(50));

        assert_eq!(
            db.cues(id(1)).unwrap().len(),
            1,
            "the cue belongs to the track, not to the deck it was on"
        );
    }

    /// Loading a second track onto the same deck must not write the first
    /// track's cues under the second track's id.
    #[test]
    fn swapping_tracks_on_a_deck_does_not_cross_their_cues() {
        let (writer, handle) = writer();
        with_track(&handle, id(1));
        with_track(&handle, id(2));
        let mut watcher = CueWatcher::new();
        let db = handle.get().unwrap();

        let mut cues = vec![None; HOT_CUE_SLOTS];
        watcher.observe(1, Some(id(1)), &cues, &writer);
        cues[0] = Some(4800.0);
        watcher.observe(1, Some(id(1)), &cues, &writer);
        wait_for(&db, id(1), 1);

        // A different track arrives on the same deck, still showing the old
        // cues for one tick before the engine has swapped them.
        watcher.observe(1, Some(id(2)), &cues, &writer);
        std::thread::sleep(std::time::Duration::from_millis(80));

        assert!(
            db.cues(id(2)).unwrap().is_empty(),
            "track 2 must not inherit track 1's cues"
        );
    }
}

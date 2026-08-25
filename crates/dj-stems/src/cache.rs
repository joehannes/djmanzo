//! Separated audio kept on disk, so a track is only separated once.
//!
//! Separation is slow -- seconds per chunk even on the built-in separator, far
//! more through a model -- so the second load of a track must not pay for it
//! again. The cache is a flat directory of chunk files with an LRU bound.
//!
//! # What the key has to include
//!
//! A chunk is identified by the track, the chunk index **and the separator that
//! produced it**. Leaving the separator out was a real defect: a DJ who ran the
//! built-in separator, then downloaded a model, would keep hearing the built-in
//! separations forever, because the cache answered before the model was ever
//! asked. The upgrade they had just installed would have done nothing.

use dj_core::track::TrackId;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How many stems a cached chunk must contain.
///
/// Checked on read as well as write. A file claiming a different number is
/// refused rather than trusted -- see [`StemCache::get`].
const STEMS: usize = dj_core::Stem::COUNT;

/// The fixed header: chunk length in samples, then stem count, both `u32`.
const HEADER: usize = 8;

/// An upper bound on a chunk's samples-per-stem.
///
/// Sixteen million samples is about three minutes of one stem at 96 kHz, far
/// more than any chunk a worker produces.
///
/// Measured, so as not to claim more for it than it does: on read this is
/// **belt and braces**. The length check below is the guard that actually
/// stops a corrupt header becoming an allocation, because a header claiming
/// four billion samples only gets past it if the file really is sixty-eight
/// gigabytes. Removing this bound leaves
/// `a_corrupt_header_is_refused_rather_than_allocated` green. It earns its
/// place on the *write* side, where it refuses to persist a chunk so large
/// that reading it back would be the thing that hurt.
const MAX_SAMPLES: usize = 16 * 1024 * 1024;

/// A disk-backed LRU cache for separated stems.
#[derive(Debug)]
pub struct StemCache {
    cache_dir: PathBuf,
    max_size_bytes: u64,
}

impl StemCache {
    /// Open, creating the directory if it is not there.
    ///
    /// # Errors
    /// If the directory cannot be created.
    pub fn new(cache_dir: impl AsRef<Path>, max_size_bytes: u64) -> io::Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            max_size_bytes,
        })
    }

    /// A chunk separated by `separator`, if it has been separated before.
    ///
    /// Every failure is `None` rather than an error: a cache that cannot answer
    /// is a cache miss, and the caller separates again. Nothing here may be
    /// fatal, because a half-written or corrupted file on disk is a thing that
    /// happens and must not stop a set.
    #[must_use]
    pub fn get(&self, separator: &str, track: TrackId, chunk: usize) -> Option<Vec<Vec<f32>>> {
        let path = self.chunk_path(separator, track, chunk);
        let data = fs::read(&path).ok()?;
        if data.len() < HEADER {
            return None;
        }

        let samples = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
        let stems = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;

        // The length check is what makes the header safe to act on: whatever
        // it claims, the file has to actually be that long. `checked_mul`
        // because a corrupt pair of `u32`s multiplies past `usize`, which
        // panics in a debug build and wraps in a release one -- and a wrapped
        // product could agree with a short file by accident.
        if stems != STEMS || samples == 0 || samples > MAX_SAMPLES {
            return None;
        }
        let expected = samples
            .checked_mul(stems)?
            .checked_mul(size_of::<f32>())?
            .checked_add(HEADER)?;
        if data.len() != expected {
            return None;
        }

        // Touch it so eviction sees it as recently used. Best-effort: a
        // read-only cache directory should still serve reads.
        let now = filetime::FileTime::from_system_time(SystemTime::now());
        let _ = filetime::set_file_times(&path, now, now);

        let mut out = Vec::with_capacity(stems);
        let mut offset = HEADER;
        for _ in 0..stems {
            let mut stem = Vec::with_capacity(samples);
            for _ in 0..samples {
                let bytes = data.get(offset..offset + 4)?;
                stem.push(f32::from_le_bytes(bytes.try_into().ok()?));
                offset += 4;
            }
            out.push(stem);
        }
        Some(out)
    }

    /// Keep a separated chunk.
    ///
    /// # Errors
    /// If the chunk cannot be written. Ragged or wrongly-counted stems are
    /// refused here rather than written: the reader checks lengths and would
    /// reject such a file forever, so writing one would mean separating that
    /// chunk again on every single play with nothing to show why.
    pub fn put(
        &self,
        separator: &str,
        track: TrackId,
        chunk: usize,
        stems: &[Vec<f32>],
    ) -> io::Result<()> {
        if stems.len() != STEMS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("expected {STEMS} stems, got {}", stems.len()),
            ));
        }
        let samples = stems[0].len();
        if samples == 0 || samples > MAX_SAMPLES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("a chunk of {samples} samples per stem is not cacheable"),
            ));
        }
        if stems.iter().any(|stem| stem.len() != samples) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the stems are not all the same length",
            ));
        }

        let mut data = Vec::with_capacity(HEADER + samples * STEMS * size_of::<f32>());
        data.extend_from_slice(&(samples as u32).to_le_bytes());
        data.extend_from_slice(&(STEMS as u32).to_le_bytes());
        for stem in stems {
            for sample in stem {
                data.extend_from_slice(&sample.to_le_bytes());
            }
        }

        // Write beside the target and rename into place. `fs::write` straight
        // to the final path leaves a truncated file if the process dies
        // mid-write, and a truncated file fails the reader's length check
        // forever -- a chunk that is re-separated on every play and never
        // succeeds. A rename is atomic, so the file is either absent or whole.
        let path = self.chunk_path(separator, track, chunk);
        let staging = path.with_extension("partial");
        fs::write(&staging, &data)?;
        fs::rename(&staging, &path)?;

        self.evict_if_needed()
    }

    /// Where a chunk lives.
    ///
    /// The separator's name is folded into the file name rather than a
    /// subdirectory so that eviction stays one flat scan, and sanitised
    /// because it ends up in a path: a separator called `../../etc` must not
    /// escape the cache directory.
    fn chunk_path(&self, separator: &str, track: TrackId, chunk: usize) -> PathBuf {
        let tag: String = separator
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        self.cache_dir
            .join(format!("{tag}_{}_{chunk}.stem", track.to_hex()))
    }

    /// Delete the least recently used chunks until the cache fits again.
    ///
    /// # Errors
    /// If the directory cannot be read.
    fn evict_if_needed(&self) -> io::Result<()> {
        let mut total: u64 = 0;
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total += metadata.len();
                files.push((
                    entry.path(),
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    metadata.len(),
                ));
            }
        }

        if total <= self.max_size_bytes {
            return Ok(());
        }

        // Oldest touch first. `get` bumps mtime, so this is least-recently-used
        // and not least-recently-written.
        files.sort_by(|a, b| a.1.cmp(&b.1));

        for (path, _, size) in files {
            if total <= self.max_size_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEPARATOR: &str = "built-in (harmonic/percussive)";

    fn track(byte: u8) -> TrackId {
        TrackId::from_bytes([byte; 32])
    }

    fn stems(samples: usize) -> Vec<Vec<f32>> {
        (0..STEMS)
            .map(|stem| (0..samples).map(|n| (stem * 1000 + n) as f32).collect())
            .collect()
    }

    fn cache(dir: &tempfile::TempDir) -> StemCache {
        StemCache::new(dir.path(), 10 * 1024 * 1024).expect("a fresh cache")
    }

    #[test]
    fn a_chunk_comes_back_exactly_as_it_went_in() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        let written = stems(64);

        cache.put(SEPARATOR, track(1), 0, &written).expect("write");
        let read = cache.get(SEPARATOR, track(1), 0).expect("a hit");
        assert_eq!(read, written, "the stems changed on the round trip");
    }

    #[test]
    fn a_chunk_that_was_never_separated_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cache(&dir).get(SEPARATOR, track(9), 3).is_none());
    }

    #[test]
    fn chunks_of_one_track_are_kept_apart() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache.put(SEPARATOR, track(1), 0, &stems(8)).unwrap();
        cache.put(SEPARATOR, track(1), 1, &stems(16)).unwrap();

        assert_eq!(cache.get(SEPARATOR, track(1), 0).unwrap()[0].len(), 8);
        assert_eq!(cache.get(SEPARATOR, track(1), 1).unwrap()[0].len(), 16);
    }

    #[test]
    fn tracks_are_kept_apart() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache.put(SEPARATOR, track(1), 0, &stems(8)).unwrap();
        assert!(cache.get(SEPARATOR, track(2), 0).is_none());
    }

    /// **The upgrade defect.** A DJ separates a track with the built-in
    /// separator, then downloads a model. Without the separator in the key,
    /// the cache answers with the built-in separations forever and the model
    /// they installed does nothing at all.
    #[test]
    fn a_different_separator_is_a_different_cache_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache.put(SEPARATOR, track(1), 0, &stems(8)).unwrap();

        assert!(
            cache.get("HTDemucs (ONNX)", track(1), 0).is_none(),
            "a better separator must not be answered with the older one's work"
        );
        assert!(
            cache.get(SEPARATOR, track(1), 0).is_some(),
            "and the original entry is still there"
        );
    }

    /// A separator name reaches the filesystem, so it must not be able to
    /// climb out of the cache directory.
    #[test]
    fn a_separator_name_cannot_escape_the_cache_directory() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache
            .put("../../../etc/passwd", track(1), 0, &stems(4))
            .unwrap();

        let entries: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 1, "the write went somewhere unexpected");
        let name = entries[0].to_string_lossy().into_owned();
        assert!(!name.contains(".."), "{name} still has a traversal in it");
        assert!(!name.contains('/'), "{name} still has a separator in it");
    }

    /// Ragged stems are refused rather than written. The reader checks that
    /// every stem is the same length, so a ragged file would be rejected on
    /// every read -- the chunk would be separated again on every play, and
    /// nothing would ever say why.
    #[test]
    fn ragged_stems_are_refused_rather_than_written() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        let mut ragged = stems(8);
        ragged[2].truncate(3);

        assert!(cache.put(SEPARATOR, track(1), 0, &ragged).is_err());
        assert!(
            cache.get(SEPARATOR, track(1), 0).is_none(),
            "a refused write must not leave a file behind"
        );
    }

    #[test]
    fn the_wrong_number_of_stems_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        assert!(cache.put(SEPARATOR, track(1), 0, &stems(8)[..3]).is_err());
        assert!(cache.put(SEPARATOR, track(1), 0, &[]).is_err());
    }

    #[test]
    fn an_empty_chunk_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(cache(&dir).put(SEPARATOR, track(1), 0, &stems(0)).is_err());
    }

    /// **A corrupt file must not be able to take the process down.** The
    /// header is two `u32`s read straight off disk, and a file claiming four
    /// billion samples per stem must not become a `Vec::with_capacity` of
    /// that size.
    ///
    /// Mutation, for honesty about which guard does the work: removing the
    /// `MAX_SAMPLES` bound leaves this test green, because the length check
    /// refuses the header first -- it would take a genuine sixty-eight
    /// gigabyte file to get past it. Removing the length check is what this
    /// actually pins.
    #[test]
    fn a_corrupt_header_is_refused_rather_than_allocated() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        let path = cache.chunk_path(SEPARATOR, track(1), 0);

        // samples = u32::MAX, stems = 4, and a body just long enough that a
        // naive reader would start filling a vector.
        let mut data = Vec::new();
        data.extend_from_slice(&u32::MAX.to_le_bytes());
        data.extend_from_slice(&(STEMS as u32).to_le_bytes());
        data.extend_from_slice(&[0u8; 64]);
        fs::write(&path, &data).unwrap();

        assert!(cache.get(SEPARATOR, track(1), 0).is_none());
    }

    #[test]
    fn a_truncated_file_is_a_miss_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache.put(SEPARATOR, track(1), 0, &stems(64)).unwrap();

        let path = cache.chunk_path(SEPARATOR, track(1), 0);
        let whole = fs::read(&path).unwrap();
        fs::write(&path, &whole[..whole.len() / 2]).unwrap();

        assert!(cache.get(SEPARATOR, track(1), 0).is_none());
    }

    #[test]
    fn a_file_shorter_than_the_header_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        let path = cache.chunk_path(SEPARATOR, track(1), 0);
        fs::write(&path, [1u8, 2, 3]).unwrap();
        assert!(cache.get(SEPARATOR, track(1), 0).is_none());
    }

    /// A write leaves nothing half-finished behind: the staging file is
    /// renamed, so the directory holds the chunk and nothing else.
    #[test]
    fn a_write_leaves_no_staging_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir);
        cache.put(SEPARATOR, track(1), 0, &stems(8)).unwrap();

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".partial"))
            .collect();
        assert!(leftovers.is_empty(), "left behind {leftovers:?}");
    }

    /// The bound is the point of the cache: a DJ's disk is not infinite, and
    /// a long night of separation would otherwise fill it.
    #[test]
    fn the_cache_evicts_down_to_its_bound() {
        let dir = tempfile::tempdir().unwrap();
        // One chunk of 256 samples per stem is 8 + 4*256*4 = 4104 bytes, so
        // this holds about two of them.
        let cache = StemCache::new(dir.path(), 9_000).unwrap();

        for chunk in 0..6 {
            cache.put(SEPARATOR, track(1), chunk, &stems(256)).unwrap();
            // Distinct mtimes, so "oldest" is well defined on a filesystem
            // whose timestamps are coarse.
            let path = cache.chunk_path(SEPARATOR, track(1), chunk);
            let when = filetime::FileTime::from_unix_time(1_000_000 + chunk as i64, 0);
            filetime::set_file_times(&path, when, when).unwrap();
        }
        // The last put evicts against the timestamps set by the previous ones.
        cache.put(SEPARATOR, track(1), 6, &stems(256)).unwrap();

        let total: u64 = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        assert!(total <= 9_000, "the cache grew to {total} bytes");
        assert!(
            cache.get(SEPARATOR, track(1), 0).is_none(),
            "the oldest chunk should have gone first"
        );
    }

    /// Reading is what makes a chunk recently used. Without the touch in
    /// `get`, eviction would be least-recently-*written*, and the chunk a DJ
    /// keeps replaying would be the one thrown away.
    #[test]
    fn reading_a_chunk_saves_it_from_eviction() {
        let dir = tempfile::tempdir().unwrap();
        let cache = StemCache::new(dir.path(), 9_000).unwrap();

        for chunk in 0..2 {
            cache.put(SEPARATOR, track(1), chunk, &stems(256)).unwrap();
            let path = cache.chunk_path(SEPARATOR, track(1), chunk);
            let when = filetime::FileTime::from_unix_time(1_000_000 + chunk as i64, 0);
            filetime::set_file_times(&path, when, when).unwrap();
        }

        // Chunk 0 is the oldest by write time -- but it is the one being used.
        assert!(cache.get(SEPARATOR, track(1), 0).is_some());

        cache.put(SEPARATOR, track(1), 2, &stems(256)).unwrap();

        assert!(
            cache.get(SEPARATOR, track(1), 0).is_some(),
            "the chunk that was just read was evicted anyway"
        );
        assert!(
            cache.get(SEPARATOR, track(1), 1).is_none(),
            "the genuinely least recently used chunk should have gone"
        );
    }
}

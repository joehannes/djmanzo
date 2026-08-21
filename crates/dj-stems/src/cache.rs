use dj_core::track::TrackId;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// On-disk format marker. A cache is an optimisation, never user data: an
/// incompatible version is discarded rather than guessed at.
const MAGIC: [u8; 8] = *b"DJSTEM01";
const STEMS: usize = 4;
const HEADER_BYTES: u64 = 8 + 4 + 8;

/// A disk-based LRU cache for separated stem chunks.
///
/// Entries are content-addressed by `TrackId`, so moving a file cannot leave a
/// stale separation behind. Cache reads deliberately treat malformed entries as
/// misses: the worker can recreate them and playback must never fail because a
/// previous process was interrupted while writing a cache file.
#[derive(Debug)]
pub struct StemCache {
    cache_dir: PathBuf,
    max_size_bytes: u64,
}

impl StemCache {
    pub fn new(cache_dir: impl AsRef<Path>, max_size_bytes: u64) -> io::Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            max_size_bytes,
        })
    }

    /// Retrieve a complete stem chunk, or `None` when it is absent or corrupt.
    pub fn get(&self, track_id: TrackId, chunk_index: usize) -> Option<Vec<Vec<f32>>> {
        let path = self.chunk_path(track_id, chunk_index);
        let stems = self.read_chunk(&path).ok()?;

        // Modification time is the portable LRU clock. Failure to update it is
        // harmless: the data is still good, merely a little easier to evict.
        let now = filetime::FileTime::from_system_time(SystemTime::now());
        let _ = filetime::set_file_times(&path, now, now);
        Some(stems)
    }

    /// Save four equal-length, interleaved stereo stem buffers atomically.
    pub fn put(&self, track_id: TrackId, chunk_index: usize, stems: &[Vec<f32>]) -> io::Result<()> {
        let frames = valid_stem_len(stems)?;
        let path = self.chunk_path(track_id, chunk_index);
        let temporary = path.with_extension("stem.tmp");
        let mut file = fs::File::create(&temporary)?;

        let result = (|| {
            file.write_all(&MAGIC)?;
            file.write_all(&(STEMS as u32).to_le_bytes())?;
            file.write_all(&(frames as u64).to_le_bytes())?;
            for stem in stems {
                for sample in stem {
                    file.write_all(&sample.to_le_bytes())?;
                }
            }
            file.sync_all()
        })();
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        // `rename` makes a reader see either the previous complete chunk or
        // this complete chunk, never a partially-written model result.
        fs::rename(&temporary, &path)?;
        self.evict_if_needed()
    }

    fn read_chunk(&self, path: &Path) -> io::Result<Vec<Vec<f32>>> {
        let metadata = fs::metadata(path)?;
        if metadata.len() < HEADER_BYTES || metadata.len() > self.max_size_bytes {
            return Err(invalid("invalid stem cache entry size"));
        }
        let mut file = fs::File::open(path)?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(invalid("unknown stem cache version"));
        }
        let stems = read_u32(&mut file)? as usize;
        let samples = read_u64(&mut file)? as usize;
        if stems != STEMS || samples == 0 || samples % 2 != 0 {
            return Err(invalid("invalid stem cache layout"));
        }
        let expected = HEADER_BYTES
            .checked_add((stems as u64).saturating_mul(samples as u64).saturating_mul(4))
            .ok_or_else(|| invalid("stem cache entry is too large"))?;
        if metadata.len() != expected {
            return Err(invalid("truncated stem cache entry"));
        }
        let mut result = Vec::with_capacity(STEMS);
        for _ in 0..STEMS {
            let mut stem = Vec::with_capacity(samples);
            for _ in 0..samples {
                stem.push(f32::from_le_bytes(read_array(&mut file)?));
            }
            result.push(stem);
        }
        Ok(result)
    }

    fn chunk_path(&self, track_id: TrackId, chunk_index: usize) -> PathBuf {
        self.cache_dir.join(format!("{}_{}.stem", track_id.to_hex(), chunk_index))
    }

    fn evict_if_needed(&self) -> io::Result<()> {
        let mut total_size = 0;
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() && entry.path().extension().is_some_and(|ext| ext == "stem") {
                total_size += metadata.len();
                files.push((entry.path(), metadata.modified().unwrap_or(UNIX_EPOCH), metadata.len()));
            }
        }
        files.sort_by_key(|(_, modified, _)| *modified);
        for (path, _, size) in files {
            if total_size <= self.max_size_bytes {
                break;
            }
            fs::remove_file(path)?;
            total_size = total_size.saturating_sub(size);
        }
        Ok(())
    }
}

fn valid_stem_len(stems: &[Vec<f32>]) -> io::Result<usize> {
    if stems.len() != STEMS || stems.first().is_none_or(Vec::is_empty) {
        return Err(invalid("a cache entry needs four non-empty stems"));
    }
    let samples = stems[0].len();
    if samples % 2 != 0 || stems.iter().any(|stem| stem.len() != samples) {
        return Err(invalid("stems must be equal-length interleaved stereo"));
    }
    Ok(samples)
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    Ok(u32::from_le_bytes(read_array(reader)?))
}

fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    Ok(u64::from_le_bytes(read_array(reader)?))
}

fn read_array<const N: usize>(reader: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> TrackId { TrackId::from_bytes([byte; 32]) }
    fn stems(value: f32) -> Vec<Vec<f32>> { vec![vec![value, -value]; STEMS] }

    #[test]
    fn stores_and_restores_all_four_stems() {
        let dir = tempfile::tempdir().unwrap();
        let cache = StemCache::new(dir.path(), 1_024).unwrap();
        let expected = vec![vec![0.1, -0.2, 0.3, -0.4]; STEMS];
        cache.put(id(1), 7, &expected).unwrap();
        assert_eq!(cache.get(id(1), 7), Some(expected));
    }

    #[test]
    fn corrupted_entries_are_cache_misses() {
        let dir = tempfile::tempdir().unwrap();
        let cache = StemCache::new(dir.path(), 1_024).unwrap();
        fs::write(cache.chunk_path(id(2), 0), b"not a cache file").unwrap();
        assert_eq!(cache.get(id(2), 0), None);
    }

    #[test]
    fn evicts_the_oldest_complete_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = StemCache::new(dir.path(), 100).unwrap();
        cache.put(id(3), 0, &stems(0.1)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        cache.put(id(4), 0, &stems(0.2)).unwrap();
        assert_eq!(cache.get(id(3), 0), None);
        assert_eq!(cache.get(id(4), 0), Some(stems(0.2)));
    }
}

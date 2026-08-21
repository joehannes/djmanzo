use dj_core::track::TrackId;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A simple disk-based LRU cache for separated stems.
pub struct StemCache {
    cache_dir: PathBuf,
    max_size_bytes: u64,
}

impl StemCache {
    pub fn new(cache_dir: impl AsRef<Path>, max_size_bytes: u64) -> std::io::Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            max_size_bytes,
        })
    }

    /// Retrieve a stem chunk if it exists in the cache.
    pub fn get(&self, track_id: TrackId, chunk_index: usize) -> Option<Vec<Vec<f32>>> {
        let path = self.chunk_path(track_id, chunk_index);
        if !path.exists() {
            return None;
        }

        // Update the atime/mtime to bump its position in the LRU
        let now = filetime::FileTime::from_system_time(SystemTime::now());
        let _ = filetime::set_file_times(&path, now, now);

        // TODO: Implement actual binary reading
        None
    }

    /// Save a stem chunk to the cache.
    pub fn put(&self, track_id: TrackId, chunk_index: usize, _stems: &[Vec<f32>]) -> std::io::Result<()> {
        let path = self.chunk_path(track_id, chunk_index);
        
        // TODO: Implement actual binary writing
        fs::write(&path, b"dummy")?;

        self.evict_if_needed()?;
        Ok(())
    }

    fn chunk_path(&self, track_id: TrackId, chunk_index: usize) -> PathBuf {
        self.cache_dir.join(format!("{}_{}.stem", track_id.to_hex(), chunk_index))
    }

    fn evict_if_needed(&self) -> std::io::Result<()> {
        let mut total_size = 0;
        let mut files = Vec::new();

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
                files.push((entry.path(), metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
            }
        }

        if total_size <= self.max_size_bytes {
            return Ok(());
        }

        // Sort by modified time, oldest first
        files.sort_by(|a, b| a.1.cmp(&b.1));

        for (path, _) in files {
            if total_size <= self.max_size_bytes {
                break;
            }
            if let Ok(metadata) = fs::metadata(&path) {
                if fs::remove_file(&path).is_ok() {
                    total_size = total_size.saturating_sub(metadata.len());
                }
            }
        }

        Ok(())
    }
}

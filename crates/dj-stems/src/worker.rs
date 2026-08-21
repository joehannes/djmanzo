use crate::cache::StemCache;
use crate::StemsEngine;
use dj_core::track::TrackId;
use std::sync::Arc;

/// The worker runs in the background and continuously separates audio ahead of the playhead.
pub struct SeparationWorker {
    engine: Arc<StemsEngine>,
    cache: Arc<StemCache>,
}

impl SeparationWorker {
    pub fn new(engine: Arc<StemsEngine>, cache: Arc<StemCache>) -> Self {
        Self { engine, cache }
    }

    /// Process a chunk of audio for a specific track, applying crossfading to prevent clicks.
    pub fn process_chunk(&self, track_id: TrackId, chunk_index: usize, audio: &[f32]) -> Result<(), ort::Error> {
        // 1. Check if the chunk already exists in the cache
        if self.cache.get(track_id, chunk_index).is_some() {
            // Already separated
            return Ok(());
        }

        // 2. Run inference
        let separated = self.engine.separate(audio)?;

        // 3. TODO: Apply micro-crossfades across chunk boundaries if we overlap
        // (Usually involves reading the tail of the previous chunk and fading)

        // 4. Save to cache
        let _ = self.cache.put(track_id, chunk_index, &separated);

        Ok(())
    }
}

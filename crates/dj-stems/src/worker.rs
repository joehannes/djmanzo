use crate::StemsEngine;
use crate::cache::StemCache;
use dj_core::track::TrackId;
use dj_decode::buffer::StemBuffer;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// One chunk of audio handed to the separation worker.
///
/// A named struct rather than a tuple because the two `usize`-ish fields would
/// otherwise be positional: `(id, 3, audio, target)` gives the reader nothing
/// to check a caller against.
#[derive(Debug)]
struct SeparationJob {
    track: TrackId,
    chunk: usize,
    audio: Vec<f32>,
    /// Where to write the result, if a deck is still waiting for it.
    target: Option<StemBuffer>,
}

/// The worker runs in the background and continuously separates audio ahead of the playhead.
#[derive(Debug)]
pub struct SeparationWorker {
    sender: Option<Sender<SeparationJob>>,
}

impl SeparationWorker {
    /// Create a worker with a live stems engine.
    pub fn new(engine: Arc<StemsEngine>, cache: Arc<StemCache>) -> Self {
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("dj-stems-worker".into())
            .spawn(move || {
                Self::worker_loop(receiver, Some(engine), cache);
            })
            .expect("Failed to spawn stem worker thread");

        Self {
            sender: Some(sender),
        }
    }

    /// Create a no-op worker for environments where the stems engine is
    /// unavailable (missing model file, no AVX2 support for the prebuilt ORT
    /// binary, etc.).  All `process_chunk` calls are silently dropped.
    pub fn unavailable() -> Self {
        Self { sender: None }
    }

    /// Enqueue a chunk of audio for separation.
    pub fn process_chunk(
        &self,
        track_id: TrackId,
        chunk_index: usize,
        audio: &[f32],
        target: Option<StemBuffer>,
    ) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(SeparationJob {
                track: track_id,
                chunk: chunk_index,
                audio: audio.to_vec(),
                target,
            });
        }
    }

    fn worker_loop(
        receiver: Receiver<SeparationJob>,
        engine: Option<Arc<StemsEngine>>,
        cache: Arc<StemCache>,
    ) {
        while let Ok(SeparationJob {
            track: track_id,
            chunk: chunk_index,
            audio,
            target,
        }) = receiver.recv()
        {
            // 1. Check if the chunk already exists in the cache
            let mut separated = None;
            if let Some(cached) = cache.get(track_id, chunk_index) {
                separated = Some(cached);
            }

            if separated.is_none() {
                // 2. Run inference (only if an engine is available)
                if let Some(eng) = &engine {
                    match eng.separate(&audio) {
                        Ok(seps) => {
                            // 3. Save to cache
                            let _ = cache.put(track_id, chunk_index, &seps);
                            separated = Some(seps);
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to separate chunk {} for track {:?}: {:?}",
                                chunk_index,
                                track_id,
                                e
                            );
                        }
                    }
                }
            }

            // 4. Update the in-memory buffer for real-time playback
            if let (Some(seps), Some(target_lock)) = (separated, target) {
                if seps.is_empty() {
                    continue;
                }
                let frames = seps[0].len() / 2; // CHANNELS=2

                // Do crossfade logic here? Wait, we can just push directly for now
                // and do the micro-crossfade logic here before pushing.
                let mut interleaved = Vec::with_capacity(frames);
                for i in 0..frames {
                    let mut frame = [0.0; 8];
                    for s in 0..4 {
                        frame[s * 2] = seps[s][i * 2];
                        frame[s * 2 + 1] = seps[s][i * 2 + 1];
                    }
                    interleaved.push(frame);
                }

                // Micro-crossfade the overlapping section with the existing data
                // We'll overlap by 1024 frames.
                let overlap = 1024.min(frames);
                let mut lock = target_lock.write();

                if !lock.is_empty() && lock.len() >= overlap {
                    let start_idx = lock.len() - overlap;
                    for i in 0..overlap {
                        let fade_in = i as f32 / overlap as f32;
                        let fade_out = 1.0 - fade_in;
                        for c in 0..8 {
                            lock[start_idx + i][c] =
                                lock[start_idx + i][c] * fade_out + interleaved[i][c] * fade_in;
                        }
                    }
                    // Extend the rest of the chunk
                    lock.extend_from_slice(&interleaved[overlap..]);
                } else {
                    lock.extend_from_slice(&interleaved);
                }
            }
        }
    }
}

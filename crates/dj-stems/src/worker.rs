use crate::cache::StemCache;
use crate::stems::Separator;
use dj_core::track::TrackId;
use dj_decode::buffer::{CHANNELS, StemBuffer, StemChunk};
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
    /// The rate the audio was decoded at. Carried per job rather than held on
    /// the worker because two decks can hold tracks of different rates.
    sample_rate: u32,
    /// Where to write the result, if a deck is still waiting for it.
    target: Option<StemBuffer>,
}

/// The worker runs in the background and continuously separates audio ahead of the playhead.
#[derive(Debug)]
pub struct SeparationWorker {
    sender: Option<Sender<SeparationJob>>,
}

impl SeparationWorker {
    /// Create a worker driven by `separator`.
    ///
    /// Takes the trait rather than a concrete engine so the built-in
    /// separator and a downloaded model are the same thing from here on: the
    /// only difference a DJ sees is the name and the quality.
    pub fn new(separator: Arc<dyn Separator>, cache: Arc<StemCache>) -> Self {
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("dj-stems-worker".into())
            .spawn(move || {
                Self::worker_loop(receiver, Some(separator), cache);
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
        sample_rate: u32,
        target: Option<StemBuffer>,
    ) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(SeparationJob {
                track: track_id,
                chunk: chunk_index,
                audio: audio.to_vec(),
                sample_rate,
                target,
            });
        }
    }

    fn worker_loop(
        receiver: Receiver<SeparationJob>,
        separator: Option<Arc<dyn Separator>>,
        cache: Arc<StemCache>,
    ) {
        while let Ok(SeparationJob {
            track: track_id,
            chunk: chunk_index,
            audio,
            sample_rate,
            target,
        }) = receiver.recv()
        {
            // The cache is keyed by which separator produced the audio, so a
            // model installed later is actually used rather than answered
            // with the built-in separator's older work.
            let name = separator.as_ref().map_or("none", |s| s.name());

            let mut separated = cache.get(name, track_id, chunk_index);

            if separated.is_none() {
                // 2. Separate (only if a separator is available)
                if let Some(sep) = &separator {
                    match sep
                        .separate(&audio, sample_rate)
                        .map(|stems| stems.into_parts().to_vec())
                    {
                        Ok(seps) => {
                            // 3. Save to cache
                            if let Err(error) = cache.put(name, track_id, chunk_index, &seps) {
                                // Not fatal: the chunk still plays, it is just
                                // separated again next time.
                                tracing::warn!(%error, chunk_index, "a separated chunk was not cached");
                            }
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

            // 4. Publish the chunk for realtime playback.
            //
            // Built here and handed over whole: the audio thread reads the
            // published table without waiting, so nothing it does depends on
            // how long this takes. The previous version appended into a
            // `Vec` behind a write lock, and every read that landed during
            // the append fell back to the unseparated mix -- a muted vocal
            // came back, once per chunk. See `dj_decode::StemTable`.
            if let (Some(seps), Some(target)) = (separated, target) {
                let Some(chunk) = interleave(&seps) else {
                    continue;
                };
                publish(&target, chunk_index, chunk);
            }
        }
    }
}

/// Turn four interleaved-stereo stems into one interleaved frame per sample.
///
/// `None` when the stems are missing, ragged or not stereo -- all of which
/// would otherwise show up as one stem playing at the wrong speed.
fn interleave(stems: &[Vec<f32>]) -> Option<StemChunk> {
    if stems.len() != dj_core::Stem::COUNT {
        tracing::error!(
            got = stems.len(),
            "a separator returned the wrong number of stems"
        );
        return None;
    }
    let samples = stems[0].len();
    if samples == 0 || !samples.is_multiple_of(CHANNELS) || stems.iter().any(|s| s.len() != samples)
    {
        tracing::error!("separated stems are ragged or not stereo");
        return None;
    }
    let frames = samples / CHANNELS;

    let mut chunk = Vec::with_capacity(frames);
    for frame in 0..frames {
        let mut out = [0.0; dj_core::Stem::COUNT * CHANNELS];
        for (stem, samples) in stems.iter().enumerate() {
            out[stem * CHANNELS] = samples[frame * CHANNELS];
            out[stem * CHANNELS + 1] = samples[frame * CHANNELS + 1];
        }
        chunk.push(out);
    }
    Some(chunk.into())
}

/// Swap `chunk` into the published table.
///
/// A compare-and-swap loop rather than a plain store: a second worker thread,
/// or a re-queued chunk, could publish between the load and the store, and
/// overwriting that would drop a chunk from the middle of the track. Retrying
/// against the table we actually read is what makes the append total.
///
/// A chunk the table refuses -- out of order, or after a short one -- is
/// dropped with a warning rather than forced, because forcing it would move
/// every later frame in the track.
fn publish(target: &StemBuffer, index: usize, chunk: StemChunk) {
    loop {
        let current = target.load();
        let Some(next) = current.with_chunk(index, Arc::clone(&chunk)) else {
            tracing::warn!(
                index,
                have = current.len(),
                "a separated chunk does not fit; dropped"
            );
            return;
        };
        let next = Arc::new(next);
        let swapped = target.compare_and_swap(&current, Arc::clone(&next));
        if Arc::ptr_eq(&swapped, &current) {
            return;
        }
        // Someone else published first; try again against what they left.
    }
}

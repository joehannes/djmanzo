use crate::cache::StemCache;
use crate::stems::Separator;
use dj_core::track::TrackId;
use dj_decode::buffer::{CHANNELS, StemBuffer, StemChunk};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
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

/// How many chunks may wait to be separated.
///
/// **Bounded, and the number matters.** A chunk is ten seconds of stereo
/// audio: at 48 kHz that is 960,000 floats, 3.8 MB. A six-minute track is
/// thirty-six of them, so an unbounded queue -- which is what this was -- held
/// the entire track in memory a second time while the worker chewed through
/// it, and loading four decks in quick succession queued half a gigabyte of
/// audio nobody had asked for yet.
///
/// Four chunks is about 15 MB and forty seconds of lookahead, which is far
/// more than a DJ can get ahead of. Past the bound the producer blocks, which
/// is exactly right: it is a background thread whose only job is to feed this
/// one, and having it wait costs nothing.
const QUEUED_CHUNKS: usize = 4;

/// The worker runs in the background and continuously separates audio ahead of the playhead.
#[derive(Debug)]
pub struct SeparationWorker {
    sender: Option<SyncSender<SeparationJob>>,
}

impl SeparationWorker {
    /// Create a worker driven by `separator`.
    ///
    /// Takes the trait rather than a concrete engine so the built-in
    /// separator and a downloaded model are the same thing from here on: the
    /// only difference a DJ sees is the name and the quality.
    pub fn new(separator: Arc<dyn Separator>, cache: Arc<StemCache>) -> Self {
        let (sender, receiver) = mpsc::sync_channel(QUEUED_CHUNKS);

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

    /// Enqueue a chunk of audio for separation, waiting if the worker is behind.
    ///
    /// **This blocks** once [`QUEUED_CHUNKS`] are already waiting. Callers are
    /// background threads feeding this one, so waiting is the correct
    /// behaviour and the alternative -- an unbounded queue -- costs a whole
    /// track of memory per load. Nothing on the audio thread ever calls this.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stems::{StemError, Stems};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::Receiver as StdReceiver;
    use std::time::{Duration, Instant};

    /// A separator that will not finish until it is told to.
    ///
    /// Blocking is the point: the queue's bound is only observable while the
    /// worker is behind, which is exactly the condition it exists for.
    #[derive(Debug)]
    struct HeldSeparator {
        release: Mutex<StdReceiver<()>>,
        started: Arc<AtomicUsize>,
    }

    impl Separator for HeldSeparator {
        fn name(&self) -> &'static str {
            "held"
        }

        fn separate(&self, mix: &[f32], sample_rate: u32) -> Result<Stems, StemError> {
            self.started.fetch_add(1, Ordering::SeqCst);
            // One permit per chunk; the test hands them out.
            let _ = self.release.lock().expect("not poisoned").recv();
            let silence = vec![0.0; mix.len()];
            Stems::new(
                [silence.clone(), silence.clone(), silence.clone(), silence],
                sample_rate,
            )
        }
    }

    fn wait_until(deadline: Duration, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < deadline {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    /// **The bound has to actually bind.**
    ///
    /// A chunk is ten seconds of stereo audio -- 3.8 MB at 48 kHz -- and a
    /// six-minute track is thirty-six of them. With an unbounded queue, which
    /// is what this was, loading a track put the whole thing in memory a
    /// second time; four decks in a row queued half a gigabyte.
    ///
    /// So a producer that outruns the worker must be made to wait, and this is
    /// what proves it does: with the separator held, only a handful of sends
    /// may complete however many are offered.
    #[test]
    fn a_producer_that_outruns_the_worker_is_made_to_wait() {
        let (permit, wait_for_permit) = mpsc::channel();
        let started = Arc::new(AtomicUsize::new(0));
        let separator = Arc::new(HeldSeparator {
            release: Mutex::new(wait_for_permit),
            started: Arc::clone(&started),
        });
        let dir = tempfile::tempdir().expect("a temp dir");
        let cache = Arc::new(StemCache::new(dir.path(), 10 * 1024 * 1024).expect("a fresh cache"));
        let worker = SeparationWorker::new(separator, cache);

        const OFFERED: usize = 12;
        let sent = Arc::new(AtomicUsize::new(0));
        let producer = {
            let sent = Arc::clone(&sent);
            std::thread::spawn(move || {
                let track = TrackId::from_bytes([1u8; 32]);
                for index in 0..OFFERED {
                    worker.process_chunk(track, index, &[0.0; CHANNELS * 8], 48_000, None);
                    sent.fetch_add(1, Ordering::SeqCst);
                }
                worker
            })
        };

        // Give it every chance to run away. The queue holds QUEUED_CHUNKS, the
        // worker has taken one and is stuck inside it, so one more send fits.
        let ceiling = QUEUED_CHUNKS + 2;
        assert!(
            wait_until(Duration::from_secs(2), || sent.load(Ordering::SeqCst) > 0),
            "nothing was sent at all"
        );
        std::thread::sleep(Duration::from_millis(200));
        let stalled = sent.load(Ordering::SeqCst);
        assert!(
            stalled <= ceiling,
            "the producer sent {stalled} chunks with the worker held; \
             the queue is not bounded (ceiling {ceiling}, offered {OFFERED})"
        );
        assert!(
            stalled < OFFERED,
            "every chunk was accepted, so nothing was waiting"
        );

        // Let it drain, and confirm the producer was blocked rather than lost.
        for _ in 0..OFFERED {
            let _ = permit.send(());
        }
        assert!(
            wait_until(Duration::from_secs(10), || sent.load(Ordering::SeqCst)
                == OFFERED),
            "the producer never finished: {} of {OFFERED}",
            sent.load(Ordering::SeqCst)
        );
        drop(producer.join().expect("the producer thread finished"));
    }

    /// A worker with no separator drops what it is given rather than queueing
    /// it, so a machine without stems does not accumulate audio it will never
    /// look at.
    #[test]
    fn an_unavailable_worker_never_blocks_and_never_queues() {
        let worker = SeparationWorker::unavailable();
        let track = TrackId::from_bytes([1u8; 32]);
        let start = Instant::now();
        for index in 0..1_000 {
            worker.process_chunk(track, index, &[0.0; CHANNELS * 8], 48_000, None);
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "an unavailable worker blocked its caller"
        );
    }
}

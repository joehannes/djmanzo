//! The buffer a deck plays from.

use dj_core::SampleRate;

/// The engine works in stereo throughout; mono sources are duplicated at decode
/// time so nothing downstream branches on channel count.
pub const CHANNELS: usize = 2;

/// Separation splits a track four ways. The order is [`dj_core::Stem::ALL`] --
/// vocal, drums, bass, other -- and every index downstream means that order.
pub const STEM_COUNT: usize = 4;

/// One frame of separated audio: every stem's channels interleaved, so a deck
/// reads the whole frame as one contiguous chunk instead of chasing four
/// pointers.
pub type StemFrame = [f32; STEM_COUNT * CHANNELS];

/// One separated chunk: a run of consecutive frames, immutable once published.
///
/// `Arc<[_]>` rather than `Arc<Vec<_>>` so the table holds one pointer per
/// chunk and copies none of the audio when it grows.
pub type StemChunk = std::sync::Arc<[StemFrame]>;

/// The separated part of a track that has arrived so far.
///
/// # Why a table of chunks rather than one growing `Vec`
///
/// Separation arrives in chunks while the deck is already playing, so the
/// worker publishes and the audio thread reads at the same time. That handoff
/// used to be an `RwLock<Vec<StemFrame>>` the worker appended to, and the
/// audio thread read with `try_read` so it could never block.
///
/// It could not block, but it could *fail*: while the worker held the write
/// lock -- a 1024-frame crossfade plus a fifteen-megabyte `extend_from_slice`,
/// so milliseconds -- every read returned `None` and the deck fell back to the
/// unseparated mix. A DJ holding the vocal muted heard it come back, once per
/// chunk, for the whole track. `deck.rs`'s
/// `a_muted_stem_stays_muted_while_the_worker_writes` pins that.
///
/// So the table is immutable. Publishing a chunk clones a vector of pointers
/// -- tens of them, no audio -- and swaps it in atomically. The audio thread
/// loads it and never waits, never fails, and never sees a half-written table.
#[derive(Debug, Default)]
pub struct StemTable {
    /// Sparse, because separation follows the playhead rather than the file.
    ///
    /// A DJ who loads a track and cues straight to the drop at 3:00 wants the
    /// stems *there*, not at 0:00 — so the worker is fed the chunks nearest
    /// the playhead first, and they arrive out of order with holes between
    /// them. A `Vec` that only accepted the next index could not hold that;
    /// it forced separation to walk the file from the beginning, which is
    /// where the stems are least likely to be wanted.
    ///
    /// A hole reads as "not yet", exactly as the end of a partly-separated
    /// track already does, and the deck falls back to the unseparated mix for
    /// that stretch.
    chunks: Vec<Option<StemChunk>>,
    /// Frames per chunk. Every chunk but the last has exactly this many, which
    /// is what lets a lookup be a division rather than a search.
    chunk_frames: usize,
}

impl StemTable {
    /// An empty table: nothing separated yet.
    #[must_use]
    pub fn new(chunk_frames: usize) -> Self {
        Self {
            chunks: Vec::new(),
            chunk_frames,
        }
    }

    /// How many frames have been separated, counting only what has actually
    /// arrived.
    ///
    /// Holes are not counted. This is "how much audio is available", not "how
    /// far into the track the furthest chunk sits" — the two stopped being the
    /// same number when separation stopped walking the file in order.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.iter().flatten().map(|chunk| chunk.len()).sum()
    }

    /// True when nothing has been separated yet.
    ///
    /// Written as "no chunk has arrived" rather than "the vector is empty"
    /// because that is the question being asked. The two cannot currently
    /// disagree — [`Self::with_chunk`] always stores a chunk, so no reachable
    /// table holds nothing but holes — and no test pins the difference,
    /// because none can. It is stated this way so that it stays correct if a
    /// chunk ever becomes removable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(Option::is_none)
    }

    /// The highest chunk index this table has room for, whether or not that
    /// chunk has arrived.
    #[must_use]
    pub fn chunk_span(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the chunk at `index` has been separated.
    ///
    /// Asked by the feeder so it does not queue work already done — after a
    /// seek it revisits a region it may have covered already.
    #[must_use]
    pub fn has_chunk(&self, index: usize) -> bool {
        self.chunks.get(index).is_some_and(Option::is_some)
    }

    /// The separated frame at `index`, if it has arrived.
    #[must_use]
    pub fn frame(&self, index: usize) -> Option<&StemFrame> {
        if self.chunk_frames == 0 {
            return None;
        }
        self.chunks
            .get(index / self.chunk_frames)?
            .as_ref()?
            .get(index % self.chunk_frames)
    }

    /// This table with `chunk` placed at `index`, for publishing.
    ///
    /// Takes `&self` and returns a new table rather than mutating, because the
    /// old one may be in use by the audio thread at this instant. Only the
    /// pointers are copied — a [`StemChunk`] is an `Arc<[StemFrame]>`, so the
    /// audio itself is never duplicated.
    ///
    /// **Any index is accepted**, and the gaps between become holes.
    /// Separation follows the playhead, so chunk 18 routinely arrives before
    /// chunk 3; a table that only took the next index in order forced the
    /// worker to walk the file from the beginning, which is where the stems
    /// are least likely to be wanted.
    ///
    /// Refused when the chunk is empty, when the slot is already filled, or
    /// when the result would break the one invariant a lookup depends on:
    /// **every chunk but the highest one present must be full length**, because
    /// [`Self::frame`] finds a chunk by division rather than by search. A short
    /// chunk is the last of the track; anything sitting beyond it would be
    /// found at the wrong offset.
    #[must_use]
    pub fn with_chunk(&self, index: usize, chunk: StemChunk) -> Option<Self> {
        if chunk.is_empty() || self.has_chunk(index) {
            return None;
        }
        // A longer chunk raises the stride rather than being refused: the
        // first chunk to arrive may be the short final one, and it must not
        // fix the stride for every full chunk that follows.
        let chunk_frames = self.chunk_frames.max(chunk.len());

        let mut chunks = self.chunks.clone();
        if chunks.len() <= index {
            chunks.resize(index + 1, None);
        }
        chunks[index] = Some(chunk);

        // Built first, then checked. Stating the invariant once, over the
        // result, is worth more than three guards that each half-state it and
        // have to agree with each other.
        let highest = chunks.iter().rposition(Option::is_some)?;
        let sound = chunks
            .iter()
            .enumerate()
            .filter_map(|(at, held)| held.as_ref().map(|c| (at, c)))
            .all(|(at, held)| at == highest || held.len() == chunk_frames);
        if !sound {
            return None;
        }

        Some(Self {
            chunks,
            chunk_frames,
        })
    }
}

/// The published separated track: swapped atomically, read without waiting.
pub type StemBuffer = std::sync::Arc<arc_swap::ArcSwap<StemTable>>;

/// Decoded audio, interleaved stereo `f32`, ready for the engine.
///
/// Always stereo: mono sources are duplicated at decode time so that nothing
/// downstream has to branch on channel count. Always `f32`: the engine mixes in
/// float, and converting once at load is cheaper than converting per sample.
///
/// # Memory
///
/// This holds the whole track in memory -- roughly 23 MB per minute of stereo
/// audio at 48 kHz. That is fine for the M0 walking skeleton and for a handful
/// of decks, but it does not scale to a browser preview or six long tracks. The
/// [`TrackSource`](crate::source::TrackSource) trait exists so a streaming
/// implementation can replace this without the engine noticing.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Interleaved L,R,L,R...
    samples: Vec<f32>,
    sample_rate: SampleRate,
    /// Stems, if available. 4 stems * 2 channels = 8 floats per frame.
    /// Uses RwLock so the background thread can write chunks.
    stems: StemBuffer,
}

impl AudioBuffer {
    /// Wrap already-interleaved stereo samples.
    ///
    /// A trailing partial frame is dropped rather than accepted, so
    /// `samples.len()` is always an exact multiple of the channel count and the
    /// reader can index without a bounds check on every access.
    pub fn from_interleaved(mut samples: Vec<f32>, sample_rate: SampleRate) -> Self {
        let usable = samples.len() - (samples.len() % CHANNELS);
        samples.truncate(usable);
        Self {
            samples,
            sample_rate,
            stems: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(StemTable::default())),
        }
    }

    /// An empty buffer, which every deck starts with. Avoids `Option` in the
    /// engine's hot path.
    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            sample_rate: SampleRate::DEFAULT,
            stems: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(StemTable::default())),
        }
    }

    /// Get a reference to the stems lock, to be populated by the background worker.
    pub fn stems_lock(&self) -> StemBuffer {
        self.stems.clone()
    }

    #[must_use]
    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    /// Number of frames (sample pairs).
    #[must_use]
    pub fn len_frames(&self) -> usize {
        self.samples.len() / CHANNELS
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.len_frames() as f64 / self.sample_rate.as_f64()
    }

    #[must_use]
    pub fn as_interleaved(&self) -> &[f32] {
        &self.samples
    }

    /// One frame, or silence past the end.
    ///
    /// Returning silence rather than `None` keeps the reader branch-light: a
    /// deck running off the end of a track just goes quiet.
    #[must_use]
    pub fn frame(&self, index: usize) -> [f32; CHANNELS] {
        let base = index * CHANNELS;
        if base + CHANNELS <= self.samples.len() {
            [self.samples[base], self.samples[base + 1]]
        } else {
            [0.0; CHANNELS]
        }
    }

    /// Linearly interpolated frame at a fractional position.
    ///
    /// The playhead is almost never on an exact frame -- any pitch other than
    /// zero puts it between two. Reading the nearest frame instead of
    /// interpolating produces audible aliasing on sustained material, which is
    /// exactly what a pitched-up track is.
    ///
    /// Linear interpolation is the M0 choice: cheap and correct enough to prove
    /// the path. A windowed-sinc resampler replaces it in M1, where pitch and
    /// keylock arrive.
    #[must_use]
    pub fn frame_interpolated(&self, position: f64) -> [f32; CHANNELS] {
        if position < 0.0 || self.samples.is_empty() {
            return [0.0; CHANNELS];
        }
        let index = position.floor();
        let fraction = (position - index) as f32;
        let index = index as usize;

        let a = self.frame(index);
        let b = self.frame(index + 1);
        [
            a[0] + (b[0] - a[0]) * fraction,
            a[1] + (b[1] - a[1]) * fraction,
        ]
    }

    /// Linearly interpolated stem frame at a fractional position.
    #[must_use]
    pub fn stem_frame_interpolated(&self, position: f64) -> Option<[[f32; CHANNELS]; STEM_COUNT]> {
        if position < 0.0 {
            return None;
        }
        let index = position.floor();
        let fraction = (position - index) as f32;
        let index = index as usize;

        // A wait-free load, not a lock: the audio thread must never depend on
        // what the separation worker is doing this instant. See [`StemTable`].
        let stems = self.stems.load();

        // Both frames or neither. Interpolating against a frame that has not
        // arrived would fade every stem towards silence at the edge of what is
        // separated -- a dip at the head of each chunk, once per chunk.
        let (Some(a), Some(b)) = (stems.frame(index), stems.frame(index + 1)) else {
            return None;
        };
        let (a, b) = (*a, *b);

        let mut out = [[0.0; CHANNELS]; STEM_COUNT];
        for (stem, frame) in out.iter_mut().enumerate() {
            let offset = stem * CHANNELS;
            for (channel, sample) in frame.iter_mut().enumerate() {
                let from = a[offset + channel];
                let to = b[offset + channel];
                *sample = from + (to - from) * fraction;
            }
        }
        Some(out)
    }
}

impl Default for AudioBuffer {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- the stem table ----------------------------------------------------

    fn chunk(value: f32, frames: usize) -> StemChunk {
        (0..frames)
            .map(|_| [value; STEM_COUNT * CHANNELS])
            .collect()
    }

    /// **Both frames or neither.** Interpolation needs the frame at `index`
    /// and the one after it. If the second is simply missing, treating it as
    /// zero fades every stem towards silence across the last frame of what
    /// has been separated -- a dip at the head of every chunk, once per chunk,
    /// for the whole track.
    ///
    /// Mutation: replace the `let (Some(a), Some(b)) = ... else` in
    /// `stem_frame_interpolated` with `unwrap_or([0.0; _])` for the second
    /// frame. Without this test every other test in the crate stays green.
    #[test]
    fn a_position_past_what_is_separated_is_refused_not_faded() {
        let buffer = AudioBuffer::from_interleaved(vec![0.0; 16], SampleRate::DEFAULT);
        buffer.stems_lock().store(std::sync::Arc::new(
            StemTable::default().with_chunk(0, chunk(1.0, 4)).unwrap(),
        ));

        // Inside: both frames are there, so the value is the constant.
        let inside = buffer
            .stem_frame_interpolated(2.5)
            .expect("frames 2 and 3 are both separated");
        assert!(
            (inside[0][0] - 1.0).abs() < 1e-6,
            "a constant should interpolate to itself, got {}",
            inside[0][0]
        );

        // At the edge: frame 4 has not arrived. The answer is "not yet",
        // never a half-faded frame.
        assert_eq!(
            buffer.stem_frame_interpolated(3.5),
            None,
            "interpolating into unseparated audio must be refused"
        );
        assert_eq!(
            buffer.stem_frame_interpolated(3.0),
            None,
            "same at the last frame"
        );
    }

    #[test]
    fn an_empty_table_has_nothing_in_it() {
        let table = StemTable::default();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(table.frame(0), None);
    }

    #[test]
    fn the_first_chunk_sets_the_stride_and_the_rest_follow_it() {
        let table = StemTable::default()
            .with_chunk(0, chunk(1.0, 8))
            .expect("the first chunk always fits");
        assert_eq!(table.len(), 8);

        let table = table
            .with_chunk(1, chunk(2.0, 8))
            .expect("a second full chunk");
        assert_eq!(table.len(), 16);

        assert_eq!(table.frame(7).unwrap()[0], 1.0);
        assert_eq!(table.frame(8).unwrap()[0], 2.0, "chunk boundary is off");
        assert_eq!(table.frame(15).unwrap()[0], 2.0);
        assert_eq!(table.frame(16), None, "past the end");
    }

    /// The last chunk of a track is whatever is left over.
    #[test]
    fn a_short_final_chunk_is_allowed() {
        let table = StemTable::default()
            .with_chunk(0, chunk(1.0, 8))
            .unwrap()
            .with_chunk(1, chunk(2.0, 3))
            .expect("the last chunk may be short");
        assert_eq!(table.len(), 11);
        assert_eq!(table.frame(10).unwrap()[0], 2.0);
        assert_eq!(table.frame(11), None);
    }

    /// **Out of order is placed, not appended.**
    ///
    /// This used to be `a_chunk_out_of_order_is_refused`, on the reasoning that
    /// accepting chunk 3 where chunk 1 belongs would put every later frame at
    /// the wrong time. That reasoning was right about *appending* and wrong
    /// about the feature: separation follows the playhead, so chunk 3 arriving
    /// first is the normal case, and refusing it forced the worker to walk the
    /// file from the beginning — which is where the stems are least likely to
    /// be wanted. A chunk now goes to the index it names, and the gap before it
    /// stays a hole until it is filled.
    ///
    /// The invariant that mattered is kept, and kept exactly: a lookup is a
    /// division, so a chunk still has to sit at the offset its index implies.
    #[test]
    fn a_chunk_lands_at_the_index_it_names() {
        let table = StemTable::default()
            .with_chunk(0, chunk(1.0, 8))
            .unwrap()
            .with_chunk(2, chunk(3.0, 8))
            .expect("chunk 2 should be placed, leaving a hole at 1");

        // Chunk 0 is where it was, chunk 2 is at frames 16..24, and the hole
        // between them reads as "not yet" rather than as chunk 2's audio.
        assert_eq!(table.frame(0).map(|f| f[0]), Some(1.0));
        assert_eq!(table.frame(8), None, "the hole leaked the next chunk");
        assert_eq!(table.frame(15), None, "the hole leaked the next chunk");
        assert_eq!(
            table.frame(16).map(|f| f[0]),
            Some(3.0),
            "chunk 2 must sit at the offset its index implies"
        );

        // Filling the hole afterwards is the point of allowing gaps.
        let filled = table
            .with_chunk(1, chunk(2.0, 8))
            .expect("the hole should accept its chunk");
        assert_eq!(filled.frame(8).map(|f| f[0]), Some(2.0));
        assert_eq!(filled.frame(16).map(|f| f[0]), Some(3.0), "chunk 2 moved");
    }

    /// A slot that is already filled is not overwritten.
    ///
    /// Re-separating the same audio produces the same result, so this costs
    /// nothing — but a chunk queued before a seek can arrive after one queued
    /// afterwards, and "the newest write wins" is not a rule worth having when
    /// "the first write wins" is free and cannot reorder anything.
    #[test]
    fn a_slot_already_filled_is_left_alone() {
        let table = StemTable::default().with_chunk(0, chunk(1.0, 8)).unwrap();
        assert!(
            table.with_chunk(0, chunk(9.0, 8)).is_none(),
            "already have it"
        );
        assert_eq!(table.frame(0).map(|f| f[0]), Some(1.0));
    }

    /// A hole is not separated audio, and `len` counts what has arrived.
    #[test]
    fn holes_are_not_counted_as_separated() {
        let table = StemTable::default()
            .with_chunk(3, chunk(1.0, 8))
            .expect("a first chunk may land anywhere");
        assert_eq!(table.len(), 8, "three holes were counted as audio");
        assert_eq!(table.chunk_span(), 4, "the table reaches chunk 3");
        assert!(!table.is_empty());
        assert!(table.has_chunk(3));
        assert!(!table.has_chunk(1), "a hole reported as separated");

        // A table of nothing but holes has separated nothing.
        let empty = StemTable::default();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    /// Nothing may follow a short chunk: the stride would no longer be the
    /// stride, and `frame` divides by it.
    #[test]
    fn nothing_follows_a_short_chunk() {
        let table = StemTable::default()
            .with_chunk(0, chunk(1.0, 8))
            .unwrap()
            .with_chunk(1, chunk(2.0, 3))
            .unwrap();
        assert!(table.with_chunk(2, chunk(3.0, 8)).is_none());
    }

    #[test]
    fn a_chunk_longer_than_the_stride_is_refused() {
        let table = StemTable::default().with_chunk(0, chunk(1.0, 8)).unwrap();
        assert!(table.with_chunk(1, chunk(2.0, 9)).is_none());
    }

    #[test]
    fn an_empty_chunk_is_refused() {
        assert!(StemTable::default().with_chunk(0, chunk(1.0, 0)).is_none());
    }

    /// Publishing must not disturb what a reader already holds -- that is the
    /// whole reason the table is rebuilt rather than mutated.
    #[test]
    fn the_old_table_is_untouched_by_publishing() {
        let first = StemTable::default().with_chunk(0, chunk(1.0, 4)).unwrap();
        let second = first.with_chunk(1, chunk(2.0, 4)).unwrap();
        assert_eq!(first.len(), 4, "the reader's table grew underneath it");
        assert_eq!(second.len(), 8);
    }

    fn buffer(frames: &[[f32; 2]]) -> AudioBuffer {
        let samples = frames.iter().flat_map(|f| [f[0], f[1]]).collect();
        AudioBuffer::from_interleaved(samples, SampleRate::DEFAULT)
    }

    #[test]
    fn empty_buffer_is_silent_and_zero_length() {
        let b = AudioBuffer::empty();
        assert!(b.is_empty());
        assert_eq!(b.len_frames(), 0);
        assert_eq!(b.frame(0), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(0.5), [0.0, 0.0]);
    }

    #[test]
    fn partial_trailing_frame_is_discarded() {
        // Three samples cannot form two whole stereo frames; the odd one goes.
        let b = AudioBuffer::from_interleaved(vec![1.0, 2.0, 3.0], SampleRate::DEFAULT);
        assert_eq!(b.len_frames(), 1);
        assert_eq!(b.as_interleaved().len(), 2);
    }

    #[test]
    fn frames_read_back_in_order() {
        let b = buffer(&[[1.0, -1.0], [0.5, -0.5]]);
        assert_eq!(b.len_frames(), 2);
        assert_eq!(b.frame(0), [1.0, -1.0]);
        assert_eq!(b.frame(1), [0.5, -0.5]);
    }

    #[test]
    fn reading_past_the_end_yields_silence() {
        let b = buffer(&[[1.0, 1.0]]);
        assert_eq!(b.frame(1), [0.0, 0.0]);
        assert_eq!(b.frame(999), [0.0, 0.0]);
    }

    #[test]
    fn interpolation_hits_exact_frames_exactly() {
        let b = buffer(&[[0.0, 0.0], [1.0, 1.0]]);
        assert_eq!(b.frame_interpolated(0.0), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(1.0), [1.0, 1.0]);
    }

    #[test]
    fn interpolation_is_linear_between_frames() {
        let b = buffer(&[[0.0, 0.0], [1.0, -1.0]]);
        let mid = b.frame_interpolated(0.5);
        assert!((mid[0] - 0.5).abs() < 1e-6);
        assert!((mid[1] + 0.5).abs() < 1e-6);

        let quarter = b.frame_interpolated(0.25);
        assert!((quarter[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn interpolation_past_the_end_fades_into_silence() {
        // Reading between the last frame and the void should ramp down, not
        // click or wrap around.
        let b = buffer(&[[1.0, 1.0]]);
        let past = b.frame_interpolated(0.5);
        assert!((past[0] - 0.5).abs() < 1e-6);
        assert_eq!(b.frame_interpolated(5.0), [0.0, 0.0]);
    }

    #[test]
    fn negative_positions_are_silent() {
        let b = buffer(&[[1.0, 1.0]]);
        assert_eq!(b.frame_interpolated(-0.5), [0.0, 0.0]);
        assert_eq!(b.frame_interpolated(-100.0), [0.0, 0.0]);
    }

    #[test]
    fn duration_follows_frame_count_and_rate() {
        let frames = vec![[0.0f32, 0.0]; 48_000];
        let b = buffer(&frames);
        assert!((b.duration_seconds() - 1.0).abs() < 1e-9);
    }
}

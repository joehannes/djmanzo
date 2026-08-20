//! What crosses into the audio thread.

use dj_core::{Action, Beatgrid, DeckId, FramePos, HOT_CUE_SLOTS, LoopRegion};
use dj_decode::TrackSource;
use std::sync::Arc;

/// A message for the engine.
///
/// Most are plain [`Action`]s. The exception is [`Command::Load`], which carries
/// an `Arc` -- see [`Retired`] for why that matters.
#[derive(Debug)]
pub enum Command {
    Action(Action),
    /// Put a track on a deck. The engine takes the `Arc` and returns whatever
    /// was there via the retirement queue.
    Load {
        deck: DeckId,
        source: Arc<dyn TrackSource>,
    },
    /// Put a sample in one sampler slot, addressing its bank explicitly.
    ///
    /// The bank is named rather than assumed to be the one showing, so a load
    /// cannot land in the wrong place because the DJ switched banks while the
    /// file was being read.
    ///
    /// `bpm` is the sample's own tempo when the analyser found one. `None` is
    /// not a failure — a vocal stab has no tempo — and a sample without one is
    /// never stretched, however the sync switch is set.
    LoadSample {
        bank: u8,
        slot: u8,
        source: Arc<dyn TrackSource>,
        bpm: Option<f64>,
    },
    /// Attach the analyser's beat grid to a deck, or clear it.
    ///
    /// A command rather than an [`Action`] because it is not something a DJ
    /// *does* — nobody presses "set grid". It is the analyser reporting a
    /// finding, and putting it in the action vocabulary would mean offering it
    /// to controllers, scripts and the assistant, none of which have any
    /// business inventing a beat grid.
    ///
    /// [`Beatgrid`] is four numbers and `Copy`, so this crosses the queue
    /// without an allocation and needs no retirement.
    SetGrid {
        deck: DeckId,
        grid: Option<Beatgrid>,
    },
    /// Put a track's stored hot cues back on a deck.
    ///
    /// A command rather than an [`Action`] for the same reason as
    /// [`Command::SetGrid`]: nobody presses "restore cues". It is the library
    /// handing back what this track had last time, and the action vocabulary
    /// describes what a *person* does. `HotCueSet` exists for that, and it puts
    /// the cue at the playhead, which is exactly wrong for a restore.
    ///
    /// A fixed-size array of `Option<FramePos>`, so it crosses the queue with
    /// no allocation and needs no retirement.
    SetHotCues {
        deck: DeckId,
        cues: [Option<FramePos>; HOT_CUE_SLOTS],
    },
    /// Hand the recorder somewhere to put audio.
    ///
    /// Recording needs a buffer and the audio thread may not allocate one, so
    /// the host allocates it and sends it in. The engine hands it back full
    /// through [`Retired::Capture`] and needs another before it can record
    /// again — which is why this is a command sent repeatedly rather than a
    /// thing set up once.
    ///
    /// Interleaved stereo at the device rate; its length is the ceiling on how
    /// long one capture can run.
    RecordSpace {
        samples: Vec<f32>,
    },
    /// Put a saved loop back on a deck, or clear the active one.
    ///
    /// A command for the same reason as the two above: the region comes from
    /// the library, not from a playhead, and no action in the vocabulary can
    /// express "loop over exactly these frames".
    SetLoop {
        deck: DeckId,
        region: Option<LoopRegion>,
    },
}

impl From<Action> for Command {
    fn from(action: Action) -> Self {
        Command::Action(action)
    }
}

/// Something the engine has finished with, on its way back to the host thread.
///
/// # Why this exists
///
/// When a deck loads a new track it displaces the old `Arc`. If that `Arc` were
/// simply dropped on the audio thread and it held the last reference, the drop
/// would free tens of megabytes -- `free()` on the realtime thread, which can
/// block on a lock inside the allocator and produce a dropout at exactly the
/// moment the DJ is loading the next track.
///
/// So the engine never drops a buffer. It pushes it here, and the host thread
/// drains this queue where blocking is harmless.
#[derive(Debug)]
pub enum Retired {
    /// A track buffer to be freed.
    Source(Arc<dyn TrackSource>),
    /// A recording buffer with nothing in it, to be freed.
    ///
    /// Distinct from [`Retired::Capture`] so that "free this" and "this is a
    /// recording" are two statements rather than one statement read two ways —
    /// a capture of zero frames addressed to slot zero would be neither.
    Buffer(Vec<f32>),
    /// A finished recording to be turned into a sample.
    ///
    /// The same queue as a freed buffer because it is the same problem seen
    /// from the other end: a `Vec` the audio thread must part with rather than
    /// drop. The only difference is that the host does something with this one
    /// before letting it go.
    Capture(crate::record::Capture),
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::action::DeckAction;
    use dj_decode::AudioBuffer;

    #[test]
    fn actions_convert_into_commands() {
        let action = Action::Deck {
            deck: DeckId::from_human(1).unwrap(),
            action: DeckAction::Play,
        };
        assert!(matches!(Command::from(action), Command::Action(_)));
    }

    /// A grid crossing into the audio thread must not carry an allocation with
    /// it, or attaching one would be a `malloc` in the callback's path.
    #[test]
    fn a_grid_command_owns_nothing() {
        use dj_core::{Bpm, Confidence, FramePos};
        let command = Command::SetGrid {
            deck: DeckId::from_human(1).unwrap(),
            grid: Some(Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(128.0).unwrap(),
                Confidence::new(0.9),
            )),
        };
        // `Copy` is the property that matters; if `Beatgrid` ever grew a `Vec`
        // this would stop compiling, which is the point.
        fn assert_copy<T: Copy>(_: &T) {}
        if let Command::SetGrid { grid: Some(g), .. } = &command {
            assert_copy(g);
        }
    }

    /// A capture and a freed buffer ride the same queue, because they are the
    /// same problem: something the audio thread may not drop.
    #[test]
    fn the_retirement_queue_carries_both_kinds_of_buffer() {
        let source: Arc<dyn TrackSource> = Arc::new(AudioBuffer::empty());
        assert!(matches!(Retired::Source(source), Retired::Source(_)));
        assert!(matches!(Retired::Buffer(Vec::new()), Retired::Buffer(_)));
        assert!(matches!(
            Retired::Capture(crate::record::Capture {
                bank: 1,
                slot: 1,
                source: dj_core::RecordSource::Master,
                samples: Vec::new(),
                frames: 0,
                sample_rate: dj_core::SampleRate::DEFAULT,
                bpm: None,
            }),
            Retired::Capture(_)
        ));
    }

    #[test]
    fn load_carries_a_shared_source() {
        let source: Arc<dyn TrackSource> = Arc::new(AudioBuffer::empty());
        let command = Command::Load {
            deck: DeckId::from_human(1).unwrap(),
            source: Arc::clone(&source),
        };
        assert_eq!(Arc::strong_count(&source), 2);
        drop(command);
        assert_eq!(Arc::strong_count(&source), 1);
    }
}

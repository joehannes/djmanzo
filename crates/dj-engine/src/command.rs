//! What crosses into the audio thread.

use dj_core::{Action, Beatgrid, DeckId};
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
}

impl From<Action> for Command {
    fn from(action: Action) -> Self {
        Command::Action(action)
    }
}

/// A track buffer the engine has finished with.
///
/// # Why this exists
///
/// When a deck loads a new track it displaces the old `Arc`. If that `Arc` were
/// simply dropped on the audio thread and it held the last reference, the drop
/// would free tens of megabytes -- `free()` on the realtime thread, which can
/// block on a lock inside the allocator and produce a dropout at exactly the
/// moment the DJ is loading the next track.
///
/// So the engine never drops a source. It pushes it here, and the host thread
/// drains this queue and drops them where blocking is harmless.
#[derive(Debug)]
pub struct Retired(pub Arc<dyn TrackSource>);

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

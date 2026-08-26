//! The action bus.
//!
//! One ordered path from every input source into the engine. See
//! `docs/adr/0003-action-bus-and-parameter-registry.md`.

use dj_core::{Action, DeckId, TrackId};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Something the session record has to remember.
///
/// Almost everything is an [`Action`] -- the vocabulary controllers, scripts and
/// the assistant all speak. The exception is loading, and it matters:
/// [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)
/// keeps loading out of the action vocabulary because it carries an `Arc` and
/// nothing external should be inventing one.
///
/// But **a set is not reproducible from its actions alone.** Replaying "deck 1
/// play" against an empty deck reproduces silence, perfectly deterministically.
/// So the log is wider than the vocabulary: it records what was *put on* the
/// decks as well as what was done to them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionEvent {
    Action(Action),
    /// A track went on a deck.
    ///
    /// **Loading is the only thing the log needs beyond the vocabulary.**
    /// Ejecting looked like a second one and is not: `deck 1 eject` is an
    /// ordinary action and is already recorded as one. A separate variant for
    /// it would have given the same event two spellings, and two takes recorded
    /// through different paths would then diff as different sets.
    Load {
        deck: DeckId,
        track: TrackId,
    },
}

/// An event together with when it happened.
///
/// The timestamp is what makes the log replayable: it is not decoration, it is
/// the schedule a replay runs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedEvent {
    pub event: SessionEvent,
    /// Time since the session started.
    pub at: Duration,
}

impl SessionEvent {
    /// One line of a session file.
    ///
    /// Text, and specifically *the same text an action is written in
    /// everywhere else* -- the mapping files, the assistant's output, the
    /// `Display` impl. A session file is therefore readable, hand-editable, and
    /// **diffable**, which is what makes take-diffing a `diff` rather than a
    /// feature.
    #[must_use]
    pub fn to_line(self) -> String {
        match self {
            Self::Action(action) => action.to_string(),
            Self::Load { deck, track } => {
                format!("load deck {} {}", deck.human_number(), track.to_hex())
            }
        }
    }

    /// Read back what [`to_line`](Self::to_line) wrote.
    ///
    /// # Errors
    /// When the line is not an event: an unknown verb, a deck that does not
    /// exist, or a malformed track id. Refused rather than skipped, so a
    /// corrupt session file is reported at the line that is wrong instead of
    /// replaying most of a set and silently missing a track.
    pub fn parse_line(line: &str) -> Result<Self, String> {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("load deck ") {
            let (deck, track) = rest
                .split_once(char::is_whitespace)
                .ok_or_else(|| format!("a load needs a deck and a track: {line:?}"))?;
            return Ok(Self::Load {
                deck: parse_deck(deck)?,
                track: TrackId::from_hex(track.trim())
                    .ok_or_else(|| format!("not a track id: {:?}", track.trim()))?,
            });
        }
        Action::parse(line)
            .map(Self::Action)
            .map_err(|e| format!("{e}"))
    }
}

fn parse_deck(text: &str) -> Result<DeckId, String> {
    text.trim()
        .parse::<u8>()
        .ok()
        .and_then(DeckId::from_human)
        .ok_or_else(|| format!("not a deck: {:?}", text.trim()))
}

/// Sends actions to the engine and records them.
///
/// Multiple producer threads (UI, MIDI, HID, network) share one sender, so the
/// *producer* side takes a lock. The consumer -- the audio thread -- never does:
/// it owns the ring buffer's read end outright. Locking on the producer side is
/// safe because none of those threads are realtime.
#[derive(Debug)]
pub struct ActionBus<C> {
    producer: Mutex<rtrb::Producer<C>>,
    log: Mutex<SessionLog>,
    started: Instant,
}

impl<C> ActionBus<C>
where
    C: From<Action>,
{
    /// Create a bus with a ring buffer of `capacity` commands.
    ///
    /// Returns the bus and the consumer end, which is handed to the engine.
    /// Capacity should comfortably exceed the number of actions that can arrive
    /// between two audio callbacks -- a controller sweep is a few hundred at
    /// most, so a few thousand is generous.
    #[must_use]
    pub fn new(capacity: usize) -> (Self, rtrb::Consumer<C>) {
        let (producer, consumer) = rtrb::RingBuffer::new(capacity);
        (
            Self {
                producer: Mutex::new(producer),
                log: Mutex::new(SessionLog::new()),
                started: Instant::now(),
            },
            consumer,
        )
    }

    /// Submit an action. Records it in the session log whether or not the
    /// engine queue accepts it, so the log stays a faithful record of intent.
    pub fn dispatch(&self, action: Action) -> Result<(), BusFull> {
        let at = self.started.elapsed();
        if let Ok(mut log) = self.log.lock() {
            log.record(TimedEvent {
                event: SessionEvent::Action(action),
                at,
            });
        }
        self.send_command(C::from(action))
    }

    /// Note what went on a deck.
    ///
    /// The load itself travels as a [`Command`](crate::ActionBus::send_command)
    /// carrying an `Arc`, which is why it is not an [`Action`] -- see
    /// [`SessionEvent`]. This records the *fact* of it, which is what a replay
    /// needs: the id, not the buffer.
    ///
    /// Called by whoever performs the load, rather than inferred from the
    /// command, because the bus deliberately does not know what a `C` contains.
    pub fn record_load(&self, deck: DeckId, track: TrackId) {
        let at = self.started.elapsed();
        if let Ok(mut log) = self.log.lock() {
            log.record(TimedEvent {
                event: SessionEvent::Load { deck, track },
                at,
            });
        }
    }

    /// Submit a command that is not a plain action -- a track load carrying a
    /// buffer, for instance. Not recorded here; a load's *fact* is recorded by
    /// [`record_load`](Self::record_load), and anything else that should be
    /// replayable must go through [`dispatch`](Self::dispatch).
    pub fn send_command(&self, command: C) -> Result<(), BusFull> {
        let mut producer = self.producer.lock().map_err(|_| BusFull)?;
        producer.push(command).map_err(|_| BusFull)
    }

    /// Point the bus at a new engine queue.
    ///
    /// Reopening an audio device tears down and rebuilds the whole realtime
    /// graph, including the ring buffer. The bus outlives that, so it needs to
    /// be re-aimed rather than recreated -- otherwise every producer holding a
    /// reference would be left writing into a queue nobody drains.
    pub fn reconnect(&self, producer: rtrb::Producer<C>) {
        if let Ok(mut guard) = self.producer.lock() {
            *guard = producer;
        }
    }

    /// Everything dispatched so far, in order.
    #[must_use]
    pub fn log(&self) -> Vec<TimedEvent> {
        self.log
            .lock()
            .map(|log| log.entries().to_vec())
            .unwrap_or_default()
    }

    pub fn clear_log(&self) {
        if let Ok(mut log) = self.log.lock() {
            log.clear();
        }
    }
}

/// The queue was full, or a producer panicked while holding the lock.
///
/// A full queue means the engine has stopped draining -- the audio device died,
/// or a callback is wedged. Callers should surface it, not retry in a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("action queue is full or poisoned")]
pub struct BusFull;

/// The ordered record of a session.
///
/// This is the substrate for replay, offline re-render and take-diffing. For now
/// it is an in-memory list; persisting it is M8 work, but the ordering guarantee
/// has to hold from the start or the feature is not reachable later.
#[derive(Debug, Default)]
pub struct SessionLog {
    entries: Vec<TimedEvent>,
}

impl SessionLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, entry: TimedEvent) {
        self.entries.push(entry);
    }

    #[must_use]
    pub fn entries(&self) -> &[TimedEvent] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Events falling in `[from, to)`, for replaying a section of a set.
    #[must_use]
    pub fn between(&self, from: Duration, to: Duration) -> &[TimedEvent] {
        let start = self.entries.partition_point(|e| e.at < from);
        let end = self.entries.partition_point(|e| e.at < to);
        &self.entries[start..end]
    }
}

#[cfg(test)]
mod tests {

    // -- the session file format ------------------------------------------

    /// **Every event survives a round trip through text.**
    ///
    /// The session file is the substrate for replay, re-render and take
    /// diffing, and all three are worthless if a line means something
    /// different on the way back in. Written as a round trip rather than as
    /// expected strings so it cannot pass by agreeing with itself about a
    /// format that is wrong.
    #[test]
    fn every_event_survives_the_round_trip() {
        let deck = DeckId::from_human(2).unwrap();
        let track = TrackId::from_bytes([0xab; 32]);
        let events = [
            SessionEvent::Action(Action::Deck {
                deck,
                action: DeckAction::Play,
            }),
            SessionEvent::Action(Action::Deck {
                deck,
                action: DeckAction::BeatJump(-4),
            }),
            SessionEvent::Action(Action::Deck {
                deck,
                action: DeckAction::PhraseJump(1),
            }),
            SessionEvent::Action(Action::Deck {
                deck,
                action: DeckAction::LoopPhrases(0.5),
            }),
            SessionEvent::Load { deck, track },
        ];

        for event in events {
            let line = event.to_line();
            let back = SessionEvent::parse_line(&line)
                .unwrap_or_else(|e| panic!("{line:?} did not parse back: {e}"));
            assert_eq!(back, event, "{line:?} came back as something else");
        }
    }

    /// **A load line carries the track, not the file path.**
    ///
    /// Paths move; ids do not. A session recorded on one machine and replayed
    /// after the music folder was reorganised should still find its records,
    /// and a path would not.
    #[test]
    fn a_load_line_carries_the_track_id() {
        let event = SessionEvent::Load {
            deck: DeckId::from_human(1).unwrap(),
            track: TrackId::from_bytes([0x01; 32]),
        };
        let line = event.to_line();
        assert!(
            line.contains(&"01".repeat(32)),
            "the id is not in the line: {line:?}"
        );
    }

    /// **A corrupt line is refused, not skipped.**
    ///
    /// Skipping would replay most of a set and silently miss a track, which
    /// looks like a bug in the engine rather than a damaged file.
    #[test]
    fn a_corrupt_line_is_refused() {
        for bad in [
            "load deck 1 nothex",
            "load deck 99 aa",
            "load deck 1",
            "deck 1 fly",
        ] {
            assert!(
                SessionEvent::parse_line(bad).is_err(),
                "{bad:?} was accepted as an event"
            );
        }
    }

    /// A load must not be mistaken for the `load` verb of anything else, and a
    /// track id must be exactly the right length.
    #[test]
    fn a_short_track_id_is_not_padded_into_a_valid_one() {
        let short = format!("load deck 1 {}", "ab".repeat(20));
        assert!(SessionEvent::parse_line(&short).is_err());
    }
    use super::*;
    use dj_core::action::DeckAction;
    use dj_core::deck::DeckId;

    /// Minimal command type standing in for the engine's real one.
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum TestCommand {
        Action(Action),
    }

    impl From<Action> for TestCommand {
        fn from(action: Action) -> Self {
            TestCommand::Action(action)
        }
    }

    fn play(deck: u8) -> Action {
        Action::Deck {
            deck: DeckId::from_human(deck).unwrap(),
            action: DeckAction::Play,
        }
    }

    #[test]
    fn dispatched_actions_reach_the_consumer() {
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(16);
        bus.dispatch(play(1)).unwrap();
        assert_eq!(consumer.pop().unwrap(), TestCommand::Action(play(1)));
    }

    #[test]
    fn order_is_preserved() {
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(16);
        for deck in 1..=4 {
            bus.dispatch(play(deck)).unwrap();
        }
        for deck in 1..=4 {
            assert_eq!(consumer.pop().unwrap(), TestCommand::Action(play(deck)));
        }
    }

    #[test]
    fn a_full_queue_reports_rather_than_blocking() {
        let (bus, _consumer) = ActionBus::<TestCommand>::new(2);
        bus.dispatch(play(1)).unwrap();
        bus.dispatch(play(2)).unwrap();
        assert_eq!(bus.dispatch(play(3)), Err(BusFull));
    }

    #[test]
    fn the_log_records_everything_dispatched() {
        let (bus, _consumer) = ActionBus::<TestCommand>::new(16);
        bus.dispatch(play(1)).unwrap();
        bus.dispatch(play(2)).unwrap();
        let log = bus.log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].event, SessionEvent::Action(play(1)));
        assert_eq!(log[1].event, SessionEvent::Action(play(2)));
    }

    /// Intent is recorded even when the engine cannot accept it, so a replay of
    /// a session that hit a full queue still shows what the DJ actually did.
    #[test]
    fn the_log_records_actions_the_queue_rejected() {
        let (bus, _consumer) = ActionBus::<TestCommand>::new(1);
        bus.dispatch(play(1)).unwrap();
        assert!(bus.dispatch(play(2)).is_err());
        assert_eq!(bus.log().len(), 2);
    }

    #[test]
    fn log_timestamps_are_monotonic() {
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(64);
        for deck in 1..=6 {
            bus.dispatch(play(deck)).unwrap();
            let _ = consumer.pop();
        }
        let log = bus.log();
        for pair in log.windows(2) {
            assert!(pair[0].at <= pair[1].at, "timestamps went backwards");
        }
    }

    #[test]
    fn many_threads_can_dispatch_at_once() {
        use std::sync::Arc;
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(1024);
        let bus = Arc::new(bus);

        let handles: Vec<_> = (1..=4u8)
            .map(|deck| {
                let bus = Arc::clone(&bus);
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        let _ = bus.dispatch(play(deck));
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let mut received = 0;
        while consumer.pop().is_ok() {
            received += 1;
        }
        assert_eq!(received, 200);
        assert_eq!(bus.log().len(), 200);
    }

    #[test]
    fn commands_bypass_the_log() {
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(16);
        bus.send_command(TestCommand::Action(play(1))).unwrap();
        assert!(consumer.pop().is_ok());
        assert!(bus.log().is_empty(), "raw commands must not enter the log");
    }

    #[test]
    fn session_log_slices_by_time() {
        let mut log = SessionLog::new();
        for ms in [0u64, 10, 20, 30, 40] {
            log.record(TimedEvent {
                event: SessionEvent::Action(play(1)),
                at: Duration::from_millis(ms),
            });
        }
        assert_eq!(
            log.between(Duration::from_millis(10), Duration::from_millis(30))
                .len(),
            2
        );
        assert_eq!(
            log.between(Duration::ZERO, Duration::from_millis(100))
                .len(),
            5
        );
        assert!(
            log.between(Duration::from_millis(100), Duration::from_millis(200))
                .is_empty()
        );
    }

    #[test]
    fn clearing_the_log_leaves_the_queue_alone() {
        let (bus, mut consumer) = ActionBus::<TestCommand>::new(16);
        bus.dispatch(play(1)).unwrap();
        bus.clear_log();
        assert!(bus.log().is_empty());
        assert!(consumer.pop().is_ok(), "queued command should survive");
    }
}

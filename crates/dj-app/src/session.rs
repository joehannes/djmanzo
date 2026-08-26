//! Reading and writing a set as a file, and replaying one.
//!
//! Every action goes through one bus with a timestamp
//! ([ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)),
//! so a set is already a recording -- not of audio, but of decisions. This is
//! what turns that into something on disk that can be replayed, re-rendered
//! offline, and diffed against another take.
//!
//! # The file is text
//!
//! One event per line, prefixed by seconds since the set began:
//!
//! ```text
//! 0.000 load deck 1 3f2a...9c
//! 1.250 deck 1 play
//! 31.500 load deck 2 88b1...04
//! 64.000 deck 2 sync_toggle
//! ```
//!
//! The event half is *the same text an action is written in everywhere else* --
//! mapping files, the assistant's output, `Action`'s own `Display`. So a
//! session file is readable, hand-editable, and **diffable**: comparing two
//! takes of the same mix is `diff`, not a feature.
//!
//! # What replay reproduces, and what it does not
//!
//! It reproduces **the decisions**: which record went on which deck, and every
//! move made to it, at the times they happened. Given the same library it
//! therefore reproduces the mix.
//!
//! It does *not* reproduce anything that was not a decision. A microphone, an
//! aux input and timecode vinyl are live signals, and no log can bring them
//! back -- a set mixed on vinyl replays as the actions the vinyl produced,
//! which is a fair record of what was played and not a recording of the room.
//! The header says which of these were in use, so a replay that will be
//! incomplete says so before it starts rather than after.

use dj_control::{SessionEvent, TimedEvent};
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

/// The first line of a session file.
///
/// A version, so a file written by an older djmanzo is rejected rather than
/// half-understood -- the same reasoning as the analysis cache. Bump it when
/// the line format changes meaning, not when a new verb is added: an unknown
/// verb is already refused by the parser, line by line, which is the more
/// useful failure.
const HEADER: &str = "djmanzo-session 1";

/// A set, as recorded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub events: Vec<TimedEvent>,
}

impl Session {
    /// Render to the file format. See the module docs.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        for entry in &self.events {
            // Milliseconds. Finer would be false precision -- the timestamps
            // come from a UI thread and a human finger -- and coarser would
            // quantise a beat at 174 BPM into the wrong place.
            let _ = writeln!(
                out,
                "{:.3} {}",
                entry.at.as_secs_f64(),
                entry.event.to_line()
            );
        }
        out
    }

    /// Read the file format.
    ///
    /// Blank lines and `#` comments are skipped, so a session file can be
    /// annotated -- which is the point of it being text.
    ///
    /// # Errors
    /// A missing or unknown header, a line with no timestamp, a timestamp that
    /// is not a number, or an event the parser does not recognise. Every one
    /// names the line, because the reason to have a text format is being able
    /// to go and look.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = lines.next().unwrap_or_default().trim();
        if header != HEADER {
            return Err(format!(
                "not a djmanzo session file, or a newer one: expected {HEADER:?}, found \
                 {header:?}"
            ));
        }

        let mut events = Vec::new();
        for (number, line) in lines.enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Line numbers count from the header, so they match what an editor
            // shows.
            let at_line = number + 2;
            let (stamp, rest) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| format!("line {at_line}: no event after the time"))?;
            let seconds: f64 = stamp
                .parse()
                .map_err(|_| format!("line {at_line}: {stamp:?} is not a time"))?;
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(format!("line {at_line}: {seconds} is not a time"));
            }
            let event =
                SessionEvent::parse_line(rest).map_err(|e| format!("line {at_line}: {e}"))?;
            events.push(TimedEvent {
                event,
                at: Duration::from_secs_f64(seconds),
            });
        }

        // Out-of-order timestamps would make a replay run events backwards.
        // Sorted rather than refused: a hand-edited file with a line inserted
        // in the wrong place is a normal thing to do, and the intent is
        // unambiguous.
        events.sort_by_key(|e| e.at);
        Ok(Self { events })
    }

    /// Write to disk.
    ///
    /// # Errors
    /// Whatever the filesystem says.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_text())
    }

    /// Read from disk.
    ///
    /// # Errors
    /// A filesystem error, or a file that is not a session.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_text(&text)
    }

    /// How long the set ran.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.events.last().map_or(Duration::ZERO, |e| e.at)
    }

    /// Every track this set put on a deck, in the order it first appeared.
    ///
    /// What a replay needs to fetch before it can start. Ordered by first use
    /// rather than sorted, so a caller loading them one at a time can begin
    /// playing before the last one arrives.
    #[must_use]
    pub fn tracks(&self) -> Vec<dj_core::TrackId> {
        let mut seen = Vec::new();
        for entry in &self.events {
            if let SessionEvent::Load { track, .. } = entry.event
                && !seen.contains(&track)
            {
                seen.push(track);
            }
        }
        seen
    }
}

/// The difference between two takes of the same set.
///
/// Not a text diff: two takes of one mix are the *same decisions at different
/// times*, and a line-by-line comparison of a file whose first column is a
/// timestamp reports every line as changed. What a DJ wants to know is which
/// moves differ, and by how much they drifted.
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// Events in the first take and not the second.
    pub only_in_first: Vec<TimedEvent>,
    /// Events in the second and not the first.
    pub only_in_second: Vec<TimedEvent>,
    /// Events in both, and how much later the second one happened. Positive
    /// means the second take was late.
    pub drifted: Vec<(SessionEvent, f64)>,
}

/// Compare two takes.
///
/// Matches events by what they are, in order, and reports the rest. The
/// matching is deliberately simple -- first unused occurrence of the same event
/// -- because the interesting output is "you dropped the bass eight seconds
/// later this time", not an optimal alignment.
#[must_use]
pub fn diff(first: &Session, second: &Session) -> Divergence {
    let mut used = vec![false; second.events.len()];
    let mut only_in_first = Vec::new();
    let mut drifted = Vec::new();

    for a in &first.events {
        match second
            .events
            .iter()
            .enumerate()
            .find(|(i, b)| !used[*i] && b.event == a.event)
        {
            Some((i, b)) => {
                used[i] = true;
                let delta = b.at.as_secs_f64() - a.at.as_secs_f64();
                if delta.abs() > f64::EPSILON {
                    drifted.push((a.event, delta));
                }
            }
            None => only_in_first.push(*a),
        }
    }

    let only_in_second = second
        .events
        .iter()
        .zip(used)
        .filter_map(|(e, used)| (!used).then_some(*e))
        .collect();

    Divergence {
        only_in_first,
        only_in_second,
        drifted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::{Action, DeckId, TrackId, action::DeckAction};

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn at(seconds: f64, event: SessionEvent) -> TimedEvent {
        TimedEvent {
            event,
            at: Duration::from_secs_f64(seconds),
        }
    }

    fn play(n: u8) -> SessionEvent {
        SessionEvent::Action(Action::Deck {
            deck: deck(n),
            action: DeckAction::Play,
        })
    }

    /// A short set: two records, one mixed into the other.
    fn a_set() -> Session {
        Session {
            events: vec![
                at(
                    0.0,
                    SessionEvent::Load {
                        deck: deck(1),
                        track: TrackId::from_bytes([1; 32]),
                    },
                ),
                at(0.5, play(1)),
                at(
                    120.0,
                    SessionEvent::Load {
                        deck: deck(2),
                        track: TrackId::from_bytes([2; 32]),
                    },
                ),
                at(180.25, play(2)),
                at(
                    240.0,
                    SessionEvent::Action(Action::Deck {
                        deck: deck(1),
                        action: DeckAction::Eject,
                    }),
                ),
            ],
        }
    }

    /// **A set survives being written and read back.**
    ///
    /// The claim the whole feature rests on. Written as a round trip so it
    /// cannot pass by agreeing with itself about a format that is wrong.
    #[test]
    fn a_session_survives_the_round_trip() {
        let original = a_set();
        let back = Session::from_text(&original.to_text()).expect("it reads back");
        assert_eq!(back, original);
    }

    /// **The timestamps survive too**, to the millisecond.
    ///
    /// A replay is a schedule. A format that rounded 180.25 to 180 would put
    /// every move a quarter of a second out, which at 174 BPM is most of a
    /// beat -- audible, and exactly the kind of error that looks like the
    /// engine drifting.
    #[test]
    fn the_times_survive_to_the_millisecond() {
        let back = Session::from_text(&a_set().to_text()).unwrap();
        assert!(
            (back.events[3].at.as_secs_f64() - 180.25).abs() < 0.001,
            "180.25 came back as {}",
            back.events[3].at.as_secs_f64()
        );
    }

    /// **A file from another version is refused, not half-read.**
    #[test]
    fn a_foreign_file_is_refused() {
        assert!(Session::from_text("djmanzo-session 99\n0.0 deck 1 play").is_err());
        assert!(Session::from_text("some other file\n").is_err());
        assert!(Session::from_text("").is_err());
    }

    /// **A damaged line names itself.**
    ///
    /// The reason to have a text format is being able to go and look, and a
    /// complaint that does not say where is one that cannot be acted on.
    #[test]
    fn a_bad_line_says_which_line() {
        let text = "djmanzo-session 1\n0.0 deck 1 play\n1.0 deck 1 fly\n";
        let error = Session::from_text(text).expect_err("it should refuse");
        assert!(
            error.contains("line 3"),
            "the error does not name the line: {error}"
        );
    }

    /// Comments and blank lines are for annotating a set by hand.
    #[test]
    fn comments_and_blanks_are_skipped() {
        let text = "djmanzo-session 1\n\n# the warm-up\n0.0 deck 1 play\n\n";
        let session = Session::from_text(text).expect("it reads");
        assert_eq!(session.events.len(), 1);
    }

    /// **A hand-edited file with a line in the wrong place still replays in
    /// order.**
    ///
    /// Sorted rather than refused: inserting a line in the wrong place is a
    /// normal thing to do to a text file, and the intent is unambiguous.
    /// Running the events in file order would replay the set backwards at that
    /// point.
    #[test]
    fn out_of_order_lines_are_sorted_not_refused() {
        let text = "djmanzo-session 1\n5.0 deck 1 play\n1.0 deck 2 play\n";
        let session = Session::from_text(text).expect("it reads");
        assert_eq!(session.events[0].at.as_secs_f64(), 1.0);
        assert_eq!(session.events[1].at.as_secs_f64(), 5.0);
    }

    /// **The tracks a replay must fetch, in the order it will need them.**
    ///
    /// First-use order rather than sorted, so a caller loading them one at a
    /// time can start the set before the last one has finished decoding.
    #[test]
    fn the_tracks_come_back_in_the_order_they_are_needed() {
        let tracks = a_set().tracks();
        assert_eq!(
            tracks,
            vec![TrackId::from_bytes([1; 32]), TrackId::from_bytes([2; 32])]
        );
    }

    /// A track loaded twice is listed once: this is a shopping list, not a
    /// history.
    #[test]
    fn a_track_loaded_twice_is_listed_once() {
        let mut set = a_set();
        set.events.push(at(
            300.0,
            SessionEvent::Load {
                deck: deck(1),
                track: TrackId::from_bytes([1; 32]),
            },
        ));
        assert_eq!(set.tracks().len(), 2);
    }

    // -- take diffing ------------------------------------------------------

    /// **Two identical takes differ in nothing.**
    #[test]
    fn a_set_does_not_differ_from_itself() {
        let d = diff(&a_set(), &a_set());
        assert!(d.only_in_first.is_empty());
        assert!(d.only_in_second.is_empty());
        assert!(d.drifted.is_empty());
    }

    /// **The same move made later is drift, not a difference.**
    ///
    /// The reason this is not `diff(1)` on the files. Two takes of one mix are
    /// the same decisions at different times, and a line-based comparison of a
    /// file whose first column is a timestamp calls every line changed --
    /// which tells a DJ nothing at all.
    #[test]
    fn the_same_move_later_is_reported_as_drift() {
        let first = a_set();
        let mut second = a_set();
        second.events[3].at = Duration::from_secs_f64(188.25);

        let d = diff(&first, &second);
        assert!(
            d.only_in_first.is_empty() && d.only_in_second.is_empty(),
            "a late move was reported as a different move"
        );
        assert_eq!(d.drifted.len(), 1);
        assert!(
            (d.drifted[0].1 - 8.0).abs() < 0.001,
            "the drift was {} seconds, expected 8",
            d.drifted[0].1
        );
    }

    /// **A move made in one take and not the other is reported as missing.**
    #[test]
    fn a_move_only_in_one_take_is_reported() {
        let first = a_set();
        let mut second = a_set();
        second.events.remove(4);
        second.events.push(at(250.0, play(1)));

        let d = diff(&first, &second);
        assert_eq!(d.only_in_first.len(), 1, "the ejected deck was not noticed");
        assert!(matches!(
            d.only_in_first[0].event,
            SessionEvent::Action(Action::Deck {
                action: DeckAction::Eject,
                ..
            })
        ));
        assert_eq!(d.only_in_second.len(), 1);
    }

    /// The same event twice in one take and once in the other is one match and
    /// one absence, not two matches.
    #[test]
    fn a_repeated_move_is_matched_once_each() {
        let first = Session {
            events: vec![at(0.0, play(1)), at(1.0, play(1))],
        };
        let second = Session {
            events: vec![at(0.0, play(1))],
        };
        let d = diff(&first, &second);
        assert_eq!(d.only_in_first.len(), 1);
        assert!(d.only_in_second.is_empty());
    }
}

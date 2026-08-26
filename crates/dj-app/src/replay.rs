//! Playing a set file back through the engine, faster than real time.
//!
//! [`crate::session`] turns a set into a file. This turns the file back into
//! audio: the engine is driven headless against a frame counter rather than a
//! sound card, every event delivered at the frame its timestamp names, and the
//! output collected instead of played.
//!
//! # Why this is worth having
//!
//! **Practice.** A set can be replayed to hear a transition again without
//! setting the records up by hand.
//!
//! **Re-rendering.** A gig mixed on a laptop through a cheap interface can be
//! rendered afterwards at full quality, from the same decisions, with nothing
//! dropped -- because nothing here runs to a deadline. An underrun in a booth
//! is a hole in the recording; here there is no clock to miss.
//!
//! **Comparison.** Two takes of the same mix produce two files, and
//! [`crate::session::diff`] already says how the decisions differed.
//!
//! # Determinism, and what it rests on
//!
//! The same file and the same records produce byte-identical audio, every time.
//! That is not a happy accident of floating-point maths -- it is a property of
//! the engine having no wall clock in its signal path. Everything that moves
//! does so per frame: envelopes, filters, the crossfader, the beat clock. Feed
//! the same frames and the same commands in the same order and the same samples
//! come out.
//!
//! Two things break it, and both are refused rather than silently approximated:
//!
//! - **A live input.** Microphone, aux and timecode vinyl are signals from the
//!   room, and no log can bring them back. A set that used them replays as the
//!   *actions* they produced, which is a fair record of what was played and not
//!   a recording of the night.
//! - **A missing record.** If the library no longer holds a track the set
//!   loaded, the replay stops and says which one, rather than rendering a
//!   silent deck and producing a file that is quietly wrong.
//!
//! # Events land on the frame, not the block
//!
//! A block boundary at 48 kHz is up to ten milliseconds wide, which is a fifth
//! of a beat at 174 BPM. Rendering in fixed blocks and firing whatever is due
//! at each edge would quantise every move in the set to that grid -- still
//! deterministic, and audibly not what was played. So a block is split at each
//! event's frame: render up to it, deliver, carry on.

use crate::session::Session;
use dj_audio::{AudioCallback, RenderContext};
use dj_control::SessionEvent;
use dj_core::{SampleRate, TrackId};
use dj_engine::{Command, Engine, Retired};
use std::sync::Arc;

/// Frames rendered between checks when nothing is due.
///
/// A working size, not a latency: nothing here runs to a deadline, so this only
/// trades call overhead against how finely the loop is interrupted. Blocks are
/// split at events regardless, so it does not affect *when* anything happens.
const BLOCK: usize = 1_024;

/// What a replay produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendered {
    pub frames: u64,
    /// Events delivered. Fewer than the session holds means it was cut short.
    pub events: usize,
}

/// Why a replay could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The set loaded a track the resolver could not produce.
    MissingTrack(TrackId),
    /// The engine's queue would not take a command. Cannot happen with the
    /// queue this module creates, which is drained every block -- carried
    /// because silently dropping a command would make the output wrong in a way
    /// nothing downstream could detect.
    QueueFull,
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTrack(id) => write!(
                f,
                "the set loads a track the library no longer has: {}",
                id.to_hex()
            ),
            Self::QueueFull => write!(f, "the engine stopped accepting commands"),
        }
    }
}

/// Render a set, handing each block of output to `sink`.
///
/// `tracks` resolves the ids the set loaded. It is a closure rather than a
/// library handle so this can be tested without a database, and so a caller can
/// decode lazily -- a two-hour set may reference more audio than fits in memory
/// at once.
///
/// `extra_frames` keeps rendering after the last event, which is what lets the
/// final record finish rather than the file stopping on the last thing the DJ
/// touched.
///
/// # Errors
/// [`ReplayError::MissingTrack`] when the set references audio the resolver
/// cannot produce.
pub fn render(
    session: &Session,
    rate: SampleRate,
    deck_count: usize,
    extra_frames: u64,
    tracks: &mut dyn FnMut(TrackId) -> Option<Arc<dyn dj_decode::TrackSource>>,
    sink: &mut dyn FnMut(&[f32]),
) -> Result<Rendered, ReplayError> {
    // Capacity for every event at once: the queue is drained every block, so
    // this is headroom rather than a limit, and a replay that dropped a command
    // would produce audio that is wrong with nothing to show for it.
    let capacity = session.events.len().max(64) + 16;
    let (mut producer, commands) = rtrb::RingBuffer::<Command>::new(capacity);
    let (retired, mut retirement) = rtrb::RingBuffer::<Retired>::new(capacity);
    let registry = Arc::new(dj_control::ParameterRegistry::new());
    let mut engine = Engine::new(deck_count, rate, commands, retired, Arc::clone(&registry));

    // Timestamps become frame numbers once, up front. Doing it per event inside
    // the loop would recompute the same division a thousand times and invite
    // the two conversions to disagree.
    let schedule: Vec<(u64, SessionEvent)> = session
        .events
        .iter()
        .map(|entry| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let frame = (entry.at.as_secs_f64() * rate.as_f64()).round() as u64;
            (frame, entry.event)
        })
        .collect();

    let last = schedule.last().map_or(0, |(f, _)| *f);
    let total = last + extra_frames;

    let mut buffer = vec![0.0f32; BLOCK * 2];
    let mut frame = 0u64;
    let mut next = 0usize;
    let mut delivered = 0usize;

    loop {
        // Everything due at or before this frame, before any of it is
        // rendered.
        while let Some((at, event)) = schedule.get(next).copied() {
            if at > frame {
                break;
            }
            let command = match event {
                SessionEvent::Action(action) => Command::Action(action),
                SessionEvent::Load { deck, track } => {
                    let source = tracks(track).ok_or(ReplayError::MissingTrack(track))?;
                    Command::Load { deck, source }
                }
            };
            producer.push(command).map_err(|_| ReplayError::QueueFull)?;
            next += 1;
            delivered += 1;
        }

        // Checked *after* delivering, not before. A `while frame < total` loop
        // exits the moment the counter reaches the last event's frame and
        // drops that event -- and a set's last event is usually the pause or
        // eject that stops the final record, so every re-render would have
        // ended with a track still playing. Found by counting the events that
        // arrived.
        if frame >= total {
            break;
        }

        // Render up to the next event, never past it. See the module docs on
        // why a fixed block would be wrong.
        let until = schedule
            .get(next)
            .map_or(total, |(at, _)| (*at).min(total))
            .max(frame + 1);
        #[allow(clippy::cast_possible_truncation)]
        let frames = ((until - frame).min(BLOCK as u64)) as usize;

        let out = &mut buffer[..frames * 2];
        out.fill(0.0);
        engine.render(
            out,
            &RenderContext {
                frames,
                channels: 2,
                sample_rate: rate,
            },
        );
        sink(out);
        frame += frames as u64;

        // Retired buffers are dropped here rather than accumulating. In the
        // application this happens on a housekeeping thread; a replay has no
        // other thread, and a two-hour set would otherwise hold every track it
        // ever loaded.
        while retirement.pop().is_ok() {}
    }

    Ok(Rendered {
        frames: frame,
        events: delivered,
    })
}

/// Render a set straight to a 16-bit WAV.
///
/// # Errors
/// A replay error, or whatever the filesystem says.
pub fn render_to_wav(
    session: &Session,
    rate: SampleRate,
    deck_count: usize,
    extra_frames: u64,
    tracks: &mut dyn FnMut(TrackId) -> Option<Arc<dyn dj_decode::TrackSource>>,
    path: &std::path::Path,
) -> Result<Rendered, String> {
    let mut wav = crate::wav::Wav::create(path, rate.get()).map_err(|e| e.to_string())?;
    let mut scratch: Vec<i16> = Vec::with_capacity(BLOCK * 2);
    let mut failure: Option<String> = None;

    let rendered = render(
        session,
        rate,
        deck_count,
        extra_frames,
        tracks,
        &mut |block| {
            if failure.is_some() {
                return;
            }
            scratch.clear();
            scratch.extend(block.iter().map(|s| {
                // Clamped before scaling: the master limiter should have kept this
                // in range, and if it did not, wrapping would turn a loud passage
                // into white noise rather than a loud passage.
                #[allow(clippy::cast_possible_truncation)]
                {
                    (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
                }
            }));
            if let Err(e) = wav.write(&scratch) {
                failure = Some(e.to_string());
            }
        },
    )
    .map_err(|e| e.to_string())?;

    if let Some(e) = failure {
        return Err(e);
    }
    wav.close().map_err(|e| e.to_string())?;
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use dj_control::TimedEvent;
    use dj_core::{Action, DeckId, action::DeckAction};
    use std::time::Duration;

    const RATE: SampleRate = SampleRate::DEFAULT;

    fn deck(n: u8) -> DeckId {
        DeckId::from_human(n).unwrap()
    }

    fn at(seconds: f64, event: SessionEvent) -> TimedEvent {
        TimedEvent {
            event,
            at: Duration::from_secs_f64(seconds),
        }
    }

    /// A ramp, so any sample tells you which frame it came from -- silence and
    /// a sine both hide an off-by-one.
    fn source(frames: usize) -> Arc<dyn dj_decode::TrackSource> {
        let samples: Vec<f32> = (0..frames)
            .flat_map(|n| {
                #[allow(clippy::cast_precision_loss)]
                let v = (n as f32 / frames as f32) * 0.5;
                [v, v]
            })
            .collect();
        Arc::new(dj_decode::AudioBuffer::from_interleaved(samples, RATE))
    }

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
                at(
                    0.1,
                    SessionEvent::Action(Action::Deck {
                        deck: deck(1),
                        action: DeckAction::Play,
                    }),
                ),
                at(
                    0.5,
                    SessionEvent::Action(Action::Deck {
                        deck: deck(1),
                        action: DeckAction::Pause,
                    }),
                ),
            ],
        }
    }

    fn resolver() -> impl FnMut(TrackId) -> Option<Arc<dyn dj_decode::TrackSource>> {
        move |_| Some(source(RATE.get() as usize * 4))
    }

    fn run(session: &Session) -> Vec<f32> {
        let mut out = Vec::new();
        let mut tracks = resolver();
        render(session, RATE, 2, RATE.get().into(), &mut tracks, &mut |b| {
            out.extend_from_slice(b);
        })
        .expect("it renders");
        out
    }

    /// **The same set renders to byte-identical audio, twice.**
    ///
    /// The claim the whole feature rests on. Not a happy accident of
    /// floating-point maths: the engine has no wall clock in its signal path,
    /// so the same frames and the same commands in the same order produce the
    /// same samples.
    #[test]
    fn the_same_set_renders_identically_twice() {
        let session = a_set();
        let first = run(&session);
        let second = run(&session);
        assert_eq!(
            first.len(),
            second.len(),
            "the two takes are different lengths"
        );
        assert_eq!(
            first, second,
            "the same set rendered to different audio, so the replay is not deterministic"
        );
    }

    /// **Something actually came out.**
    ///
    /// The determinism test above passes trivially if both runs are silence,
    /// which is exactly what a replay that never delivered its loads would
    /// produce. This is the test that stops that.
    #[test]
    fn a_replayed_set_is_not_silence() {
        let audio = run(&a_set());
        assert!(
            audio.iter().any(|s| s.abs() > 1e-6),
            "the replay produced {} samples of silence -- nothing was loaded or nothing played",
            audio.len()
        );
    }

    /// **The set stops where the DJ stopped it.**
    ///
    /// A pause at 0.5s must be audible as a change in the output, otherwise the
    /// events after the first are not reaching the engine.
    #[test]
    fn a_pause_partway_through_is_obeyed() {
        let audio = run(&a_set());
        let pause_frame = (0.5 * RATE.as_f64()) as usize;
        let after: f32 = audio
            .iter()
            .skip(pause_frame * 2 + RATE.get() as usize)
            .map(|s| s.abs())
            .sum();
        assert!(
            after < 1e-3,
            "the deck was still making sound long after the pause: {after}"
        );
    }

    /// **A missing record stops the replay and names it.**
    ///
    /// Rather than rendering a silent deck and producing a file that is quietly
    /// wrong -- which nothing downstream could detect.
    #[test]
    fn a_missing_track_is_refused_by_name() {
        let session = a_set();
        let mut none = |_| None;
        let error = render(&session, RATE, 2, 0, &mut none, &mut |_| {})
            .expect_err("a missing track should stop the replay");
        assert_eq!(
            error,
            ReplayError::MissingTrack(TrackId::from_bytes([1; 32]))
        );
        assert!(
            error.to_string().contains(&"01".repeat(32)),
            "the error does not name the track: {error}"
        );
    }

    /// **Every event is delivered**, not just the ones that happened to fall on
    /// a block boundary.
    #[test]
    fn every_event_reaches_the_engine() {
        let session = a_set();
        let mut tracks = resolver();
        let rendered = render(&session, RATE, 2, 0, &mut tracks, &mut |_| {}).expect("renders");
        assert_eq!(rendered.events, session.events.len());
    }

    /// **A block is split at an event rather than rounded to its edge.**
    ///
    /// At 48 kHz a 1024-frame block is 21 ms, a third of a beat at 174 BPM.
    /// Firing whatever is due at each edge would quantise every move in the set
    /// to that grid -- deterministic, and audibly not what was played. The
    /// evidence is that an event at a frame which is *not* a multiple of the
    /// block size still lands there.
    #[test]
    fn an_event_lands_on_its_frame_not_the_block_edge() {
        // 0.007 seconds is frame 336 at 48 kHz: inside the first block.
        let session = Session {
            events: vec![
                at(
                    0.0,
                    SessionEvent::Load {
                        deck: deck(1),
                        track: TrackId::from_bytes([1; 32]),
                    },
                ),
                at(
                    0.007,
                    SessionEvent::Action(Action::Deck {
                        deck: deck(1),
                        action: DeckAction::Play,
                    }),
                ),
            ],
        };
        let mut lengths = Vec::new();
        let mut tracks = resolver();
        render(&session, RATE, 2, 2_000, &mut tracks, &mut |b| {
            lengths.push(b.len() / 2);
        })
        .expect("renders");

        assert_eq!(
            lengths.first(),
            Some(&336),
            "the first block ran to {:?} instead of stopping at the event's frame",
            lengths.first()
        );
    }

    /// An empty set renders nothing and does not hang.
    #[test]
    fn an_empty_set_renders_nothing() {
        let mut tracks = resolver();
        let rendered = render(&Session::default(), RATE, 2, 0, &mut tracks, &mut |_| {
            panic!("an empty set produced audio")
        })
        .expect("renders");
        assert_eq!(rendered.frames, 0);
        assert_eq!(rendered.events, 0);
    }

    /// The tail keeps rendering after the last event, so the final record
    /// finishes rather than the file stopping on the last thing touched.
    #[test]
    fn the_tail_is_rendered_after_the_last_event() {
        let mut tracks = resolver();
        let rendered =
            render(&a_set(), RATE, 2, 24_000, &mut tracks, &mut |_| {}).expect("renders");
        let last_event = (0.5 * RATE.as_f64()) as u64;
        assert!(
            rendered.frames >= last_event + 24_000,
            "rendered {} frames, expected at least {}",
            rendered.frames,
            last_event + 24_000
        );
    }
}

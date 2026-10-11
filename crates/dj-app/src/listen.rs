//! K6: listening to the singers — the pitch on each microphone, read off the
//! audio thread.
//!
//! The audio thread does no pitch reading. Each strip of the singers' rack
//! hands its microphone, decimated to 16 kHz, into a ring of its own
//! (`dj_vocal::Vocals::listen`); the host gives the reading ends to [`ears`]
//! when it opens the rack, and a thread here drains them every
//! [`EVERY`], halves it to the 8 kHz pitch is read at, reads the last moment
//! of each with the same YIN the reference line was read with
//! (`dj_analysis::melody::live_pitch`), and publishes it
//! per strip as `VocalParam::Pitch` and `VocalParam::Sung`, where the
//! snapshot and the scoring read it. See `docs/KARAOKE.md` §9.
//!
//! A2: what arrived on each strip since the last listening is handed on as
//! it came, at 16 kHz — whisper's rate — for a spoken command to be taken
//! from the DJ's strip (`docs/ASSISTANT.md` §4).

use dj_analysis::melody::{LIVE_WINDOW, live_pitch};
use dj_core::param::{MAX_VOCAL_STRIPS, ParamId, VocalParam};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// How often the singers are listened to: twenty times a second, the rate the
/// reference line is read at, so one reading meets one point of it.
pub const EVERY: Duration = Duration::from_millis(50);

/// How much of each strip's voice is kept between readings, in samples at
/// the tap's rate — twice the pitch reading's window, at twice its rate.
const KEEP: usize = LIVE_WINDOW * 4;

/// What one listening heard.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Heard {
    /// Each strip's pitch at its last moment, in cents above A4 — `None`
    /// where nobody is singing.
    pub pitches: Vec<Option<f32>>,
    /// What each strip's ring carried since the last listening, at the tap's
    /// rate.
    pub fresh: Vec<Vec<f32>>,
}

/// A voice at the tap's rate halved to the pitch reading's: each pair of
/// samples averaged, the last pair ending on the last sample, so the moment
/// read is the latest.
fn halved(voice: &[f32]) -> Vec<f32> {
    voice[voice.len() % 2..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[a, b]| (a + b) * 0.5)
        .collect()
}

/// The reading ends of the singers' listening rings — one per strip, in
/// order — while a rack is open.
#[derive(Debug, Default)]
pub struct Ears {
    rings: Mutex<Vec<rtrb::Consumer<f32>>>,
}

impl Ears {
    /// The rings of a rack just opened, or none when it closes. The old ones
    /// are let go here, off the audio thread.
    pub fn hear(&self, rings: Vec<rtrb::Consumer<f32>>) {
        if let Ok(mut held) = self.rings.lock() {
            *held = rings;
        }
    }
}

/// The application's ears: one audio host, one rack, one set of rings.
pub fn ears() -> &'static Ears {
    static EARS: OnceLock<Ears> = OnceLock::new();
    EARS.get_or_init(Ears::default)
}

/// One listening: every ring drained into its strip's recent voice, and the
/// pitch of each strip's last moment — `None` where nobody is singing — with
/// what each ring carried since the last.
pub fn listen(rings: &mut [rtrb::Consumer<f32>], recent: &mut Vec<Vec<f32>>) -> Heard {
    recent.resize_with(rings.len(), Vec::new);
    let (pitches, fresh) = rings
        .iter_mut()
        .zip(recent.iter_mut())
        .map(|(ring, voice)| {
            let mut fresh = Vec::new();
            while let Ok(sample) = ring.pop() {
                fresh.push(sample);
            }
            voice.extend_from_slice(&fresh);
            if voice.len() > KEEP {
                voice.drain(..voice.len() - KEEP);
            }
            (live_pitch(&halved(voice)), fresh)
        })
        .unzip();
    Heard { pitches, fresh }
}

/// Listen for as long as the application runs, publishing each strip's pitch
/// into `registry` and handing every listening to `on_heard` — the scoring,
/// a range take, a spoken command. A strip with no ring reads as not
/// singing.
pub fn spawn(
    registry: std::sync::Arc<dj_control::ParameterRegistry>,
    on_heard: impl Fn(&Heard) + Send + 'static,
) {
    let spawned = std::thread::Builder::new()
        .name("dj-listen".into())
        .spawn(move || {
            let mut recent = Vec::new();
            loop {
                std::thread::sleep(EVERY);
                let heard = match ears().rings.lock() {
                    Ok(mut rings) => listen(&mut rings, &mut recent),
                    Err(_) => Heard::default(),
                };
                if heard.pitches.is_empty() {
                    recent.clear();
                } else {
                    on_heard(&heard);
                }
                for strip in 0..MAX_VOCAL_STRIPS {
                    #[allow(clippy::cast_possible_truncation)]
                    let at = |param| ParamId::Vocal(strip as u8, param);
                    let pitch = heard.pitches.get(strip).copied().flatten();
                    registry.set(
                        at(VocalParam::Sung),
                        if pitch.is_some() { 1.0 } else { 0.0 },
                    );
                    if let Some(cents) = pitch {
                        registry.set(at(VocalParam::Pitch), cents);
                    }
                }
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the singers are not listened to: nothing will be scored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Each strip is read on its own, and only the last moment of it.** An
    /// A3 on strip 0 reads −1200 cents and silence on strip 1 reads as
    /// nobody singing; when strip 0's singer moves to E4, the next reading
    /// says −500 — what was sung a second ago is not averaged in.
    #[test]
    fn each_strip_is_read_on_its_own_and_only_its_last_moment() {
        let (mut a, ear_a) = rtrb::RingBuffer::new(32_000);
        let (_b, ear_b) = rtrb::RingBuffer::new(32_000);
        let mut rings = vec![ear_a, ear_b];
        let mut recent = Vec::new();
        let mut sing = |hz: f32, seconds: f32, from: &mut u32| {
            for _ in 0..(seconds * 16_000.0) as u32 {
                let t = *from as f32 / 16_000.0;
                let _ = a.push(0.4 * (std::f32::consts::TAU * hz * t).sin());
                *from += 1;
            }
        };
        let mut n = 0;
        sing(220.0, 0.5, &mut n);
        let heard = listen(&mut rings, &mut recent);
        assert_eq!(heard.fresh[0].len(), 8_000, "half a second, as it came");
        assert!(heard.fresh[1].is_empty());
        let first = heard.pitches;
        assert_eq!(first.len(), 2);
        assert!(
            first[0].is_some_and(|c| (c + 1200.0).abs() < 10.0),
            "{first:?}"
        );
        assert_eq!(first[1], None);
        sing(329.63, 0.05, &mut n);
        let next = listen(&mut rings, &mut recent);
        assert_eq!(next.fresh[0].len(), 800, "only what came since");
        let next = next.pitches;
        assert!(
            next[0].is_some_and(|c| (c + 500.0).abs() < 10.0),
            "{next:?}"
        );
        assert!(recent[0].len() <= KEEP);
    }
}

//! A2: a spoken command, taken from the DJ's own microphone while
//! push-to-talk is held (`docs/ASSISTANT.md` §4).
//!
//! The microphone is one of the strips K3 already has open — not a second
//! stream on a device it holds — and its voice arrives through K6's tap, at
//! [`RATE`], on the listening thread. While the take runs the strip is held
//! out of the room: its sends to the PA and the singers' monitor go to
//! nothing and its talkover is off, so "deck two, play" is not heard through
//! the speakers and does not duck the music. Let go, the strip is put back
//! as it was, and the take is what is transcribed.

use dj_vocal::StripSettings;
use serde::Serialize;

/// The rate a take is at: the tap's, whisper's.
pub const RATE: u32 = dj_vocal::LISTEN_RATE;

/// The longest a take runs, in seconds: a command, not a speech.
pub const MOST_SECONDS: f64 = 8.0;

/// The least voice a take must hold to be worth transcribing, in seconds.
pub const LEAST_VOICE_SECONDS: f64 = 0.3;

/// Louder than this, a moment of a take is voice, in RMS over a hundredth of
/// a second — about −46 dBFS, below speech close to a microphone and above
/// a quiet booth's hiss.
const VOICE_FLOOR: f32 = 0.005;

/// A strip's settings held out of the room for a take: nothing to the PA or
/// the singers' monitor, and no talkover — the rest, the DJ's headphones
/// included, as it was.
#[must_use]
pub fn held_out(settings: StripSettings) -> StripSettings {
    StripSettings {
        to_main: 0.0,
        to_monitor: 0.0,
        talkover: false,
        ..settings
    }
}

/// A take while push-to-talk is held.
#[derive(Debug, Clone)]
pub struct Take {
    /// The strip it is taken from.
    pub strip: usize,
    /// When the key went down.
    pub started: std::time::Instant,
    /// The voice so far, at [`RATE`].
    pub samples: Vec<f32>,
}

impl Take {
    #[must_use]
    pub fn new(strip: usize) -> Self {
        Self {
            strip,
            started: std::time::Instant::now(),
            samples: Vec::new(),
        }
    }

    /// Whether the take has run its course though the key is still down:
    /// [`MOST_SECONDS`] of voice kept, or that long and a second gone by with
    /// the strip carrying nothing. A key whose release was lost — the window
    /// losing focus with a finger on it — must not leave a microphone out of
    /// the PA for the rest of the night.
    #[must_use]
    pub fn run_out(&self) -> bool {
        self.seconds() >= MOST_SECONDS || self.started.elapsed().as_secs_f64() >= MOST_SECONDS + 1.0
    }

    /// What one listening carried, every strip's: this strip's kept, up to
    /// [`MOST_SECONDS`].
    pub fn hear(&mut self, fresh: &[Vec<f32>]) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let most = (MOST_SECONDS * f64::from(RATE)) as usize;
        if let Some(voice) = fresh.get(self.strip) {
            let room = most.saturating_sub(self.samples.len());
            self.samples
                .extend_from_slice(&voice[..voice.len().min(room)]);
        }
    }

    /// Seconds taken.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let samples = self.samples.len() as f64;
        samples / f64::from(RATE)
    }

    /// Seconds of it that are voice: hundredths of a second louder than
    /// about −46 dBFS.
    #[must_use]
    pub fn voiced(&self) -> f64 {
        let hundredth = (RATE / 100) as usize;
        let loud = self
            .samples
            .chunks(hundredth)
            .filter(|moment| {
                #[allow(clippy::cast_precision_loss)]
                let power = moment.iter().map(|s| s * s).sum::<f32>() / moment.len() as f32;
                power.sqrt() > VOICE_FLOOR
            })
            .count();
        #[allow(clippy::cast_precision_loss)]
        let seconds = loud as f64 / 100.0;
        seconds
    }
}

/// What the last take came to.
#[derive(Debug, Clone, PartialEq)]
pub struct Ended {
    pub seconds: f64,
    pub voiced: f64,
    /// Why it will not be transcribed, when it will not.
    pub problem: Option<String>,
    /// The voice, for the transcription.
    pub samples: Vec<f32>,
}

impl Ended {
    /// What `take` came to: transcribed only with enough voice in it.
    #[must_use]
    pub fn of(take: Take) -> Self {
        let voiced = take.voiced();
        Self {
            seconds: take.seconds(),
            voiced,
            problem: (voiced < LEAST_VOICE_SECONDS).then(|| {
                format!(
                    "Nothing was heard on Mic {}: hold the key while speaking, close to the microphone.",
                    take.strip + 1
                )
            }),
            samples: take.samples,
        }
    }
}

/// What a take was heard to say, and what was done about it — shown in that
/// order, so a DJ sees "pon el dos" before "deck 2 play" and can tell a
/// mishearing from a wrong answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Said {
    /// The words, as whisper.cpp wrote them.
    pub heard: String,
    /// The language it heard, when it says.
    pub language: Option<String>,
    /// Actions done, in order, as the bus wrote them.
    pub actions: Vec<String>,
    /// Actions understood and refused, each with why.
    pub undelivered: Vec<String>,
    /// What djmanzo says back.
    pub reply: String,
    /// `local` when understood without a model, `model` when the DJ's AI
    /// provider answered, `none` when nothing did.
    pub source: &'static str,
    /// Why nothing was done, when nothing was.
    pub problem: Option<String>,
    /// Loading the model and listening, in seconds.
    pub seconds: Option<f64>,
}

impl Said {
    /// Nothing done, and why.
    #[must_use]
    pub fn refused(heard: &str, why: impl Into<String>) -> Self {
        Self {
            heard: heard.to_owned(),
            language: None,
            actions: Vec::new(),
            undelivered: Vec::new(),
            reply: String::new(),
            source: "none",
            problem: Some(why.into()),
            seconds: None,
        }
    }
}

/// The words of a transcription as a command is matched on: what whisper
/// adds that nobody said — `[BLANK_AUDIO]`, `(music)`, `*laughs*` — taken
/// out, and the punctuation it writes ("Deck two, play.") made spaces, since
/// the matcher reads whole words.
#[must_use]
pub fn spoken(heard: &str) -> String {
    let mut words = String::with_capacity(heard.len());
    let mut aside: Option<char> = None;
    for c in heard.chars() {
        match (aside, c) {
            (None, '[') => aside = Some(']'),
            (None, '(') => aside = Some(')'),
            (None, '*') => aside = Some('*'),
            (Some(close), c) if c == close => {
                aside = None;
                words.push(' ');
            }
            (Some(_), _) => {}
            (None, c) if c.is_alphanumeric() || c == '\'' || c == '-' => words.push(c),
            (None, _) => words.push(' '),
        }
    }
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The DJ's voice commands: a take while one is held, what the last came
/// to, and what it was heard to say. Which strip they come from is kept on
/// disk (`voice.json`).
#[derive(Debug, Clone, Default)]
pub struct Voice {
    pub take: Option<Take>,
    pub last: Option<Ended>,
    /// Whether the last take is being turned into words.
    pub hearing: bool,
    pub said: Option<Said>,
    /// Where a take goes to be heard: the words' worker, once it runs.
    pub to_words: Option<std::sync::mpsc::Sender<Vec<f32>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64, level: f32) -> Vec<f32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (seconds * f64::from(RATE)) as usize;
        (0..n)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / RATE as f32;
                level * (std::f32::consts::TAU * 200.0 * t).sin()
            })
            .collect()
    }

    /// **Held out of the room, and only that**: no PA, no singers' monitor,
    /// no talkover — the DJ's headphones and the chain as they were.
    #[test]
    fn a_held_strip_is_out_of_the_room_and_nothing_else() {
        let found = StripSettings {
            to_main: 0.8,
            to_cue: 0.6,
            to_monitor: 0.5,
            talkover: true,
            gain_db: 3.0,
            open: true,
            pan: -0.4,
            ..StripSettings::default()
        };
        let held = held_out(found);
        assert_eq!(
            (held.to_main, held.to_monitor, held.talkover),
            (0.0, 0.0, false)
        );
        assert_eq!(held.to_cue, 0.6);
        assert_eq!(held.gain_db, 3.0);
        assert_eq!(
            (held.open, held.pan, held.compressor, held.reverb, held.gate),
            (
                found.open,
                found.pan,
                found.compressor,
                found.reverb,
                found.gate
            ),
            "the voice's own chain was changed"
        );
    }

    /// **A take keeps its own strip's voice, eight seconds at most.**
    #[test]
    fn a_take_keeps_its_strip_for_eight_seconds_at_most() {
        let mut take = Take::new(1);
        take.hear(&[tone(1.0, 0.9), tone(1.0, 0.2)]);
        assert!((take.seconds() - 1.0).abs() < 1e-9);
        assert!(!take.run_out());
        assert!(
            take.samples.iter().all(|s| s.abs() <= 0.2 + 1e-6),
            "strip 0's voice got in"
        );
        for _ in 0..10 {
            take.hear(&[Vec::new(), tone(1.0, 0.2)]);
        }
        assert!((take.seconds() - MOST_SECONDS).abs() < 1e-9);
        assert!(
            take.run_out(),
            "eight seconds is a take's end, key down or not"
        );
        take.hear(&[]);
        assert!(
            (take.seconds() - MOST_SECONDS).abs() < 1e-9,
            "a strip that is gone is nothing"
        );
    }

    /// **A lost key-up does not keep a microphone out of the PA**: a take
    /// whose strip carries nothing still runs out.
    #[test]
    fn a_take_runs_out_without_its_key_up() {
        let mut take = Take::new(3);
        take.hear(&[tone(0.5, 0.2)]);
        assert!(!take.run_out());
        take.started = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs_f64(MOST_SECONDS + 1.5))
            .expect("the clock has run that long");
        assert!(take.run_out());
    }

    /// **What whisper adds is not a command, and its punctuation is not part
    /// of one.**
    #[test]
    fn whispers_asides_and_punctuation_are_not_words() {
        assert_eq!(spoken(" Deck two, play."), "Deck two play");
        assert_eq!(spoken("¡Pon el dos!"), "Pon el dos");
        assert_eq!(spoken("[BLANK_AUDIO]"), "");
        assert_eq!(
            spoken("(music) kill the bass *laughs* on deck 1"),
            "kill the bass on deck 1"
        );
        assert_eq!(spoken("¿Qué?  Don't  stop"), "Qué Don't stop");
    }

    /// **Silence is not a command**: a take of hiss is not transcribed, and
    /// says why; a take with voice in it is.
    #[test]
    fn silence_is_not_a_command() {
        let mut quiet = Take::new(0);
        quiet.hear(&[tone(2.0, 0.001)]);
        let ended = Ended::of(quiet);
        assert!(ended.voiced < LEAST_VOICE_SECONDS);
        assert!(
            ended
                .problem
                .as_deref()
                .is_some_and(|why| why.contains("Mic 1"))
        );

        let mut spoken = Take::new(0);
        let mut voice = tone(0.5, 0.001);
        voice.extend(tone(1.0, 0.3));
        spoken.hear(&[voice]);
        let ended = Ended::of(spoken);
        assert!((ended.voiced - 1.0).abs() < 0.02, "{}", ended.voiced);
        assert_eq!(ended.problem, None);
    }
}

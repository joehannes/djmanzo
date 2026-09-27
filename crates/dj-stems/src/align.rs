//! §122: English words placed in the audio letter by letter — wav2vec 2.0
//! on the ONNX Runtime djmanzo already carries for its stems.
//!
//! > i'd like whisper.cpp + a better model than base only ... + the English
//! > aligner as well please.
//!
//! Whisper says where a word was heard to within about a quarter of a
//! second. An aligner does better when the words are known: a model that
//! hears **letters**, twenty milliseconds at a time, is asked for the most
//! likely path through exactly those letters in order (CTC forced
//! alignment, as WhisperX does it), and each word's first and last letter
//! are its start and end. The model is `facebook/wav2vec2-base-960h`
//! (Apache-2.0) — the one WhisperX uses for English — in the 8-bit ONNX
//! export published beside it; it knows the twenty-six letters and the
//! apostrophe, so it places English and nothing else.
//!
//! Aligned a line at a time, on the line's own stretch of audio: the model's
//! attention grows with the square of what it hears, and a whole song at
//! once would need gigabytes.

use crate::availability::{self, Unavailable};
use ort::session::{Session, builder::GraphOptimizationLevel};
use std::path::Path;
use std::sync::Mutex;

/// What the model hears: mono at sixteen kilohertz.
pub const RATE: u32 = 16_000;

/// Samples per frame of its output: its convolutions' strides multiplied,
/// 5·2·2·2·2·2·2 — twenty milliseconds.
pub const HOP: usize = 320;

/// The "no letter here" class.
const BLANK: usize = 0;

/// The word separator.
const SPACE: usize = 4;

/// The letters the model knows, as its vocabulary numbers them.
const LETTERS: [(char, usize); 27] = [
    ('E', 5),
    ('T', 6),
    ('A', 7),
    ('O', 8),
    ('N', 9),
    ('I', 10),
    ('H', 11),
    ('S', 12),
    ('R', 13),
    ('D', 14),
    ('L', 15),
    ('U', 16),
    ('M', 17),
    ('W', 18),
    ('C', 19),
    ('F', 20),
    ('G', 21),
    ('Y', 22),
    ('P', 23),
    ('B', 24),
    ('V', 25),
    ('K', 26),
    ('\'', 27),
    ('X', 28),
    ('J', 29),
    ('Q', 30),
    ('Z', 31),
];

fn letter(c: char) -> Option<usize> {
    let upper = c.to_ascii_uppercase();
    LETTERS
        .iter()
        .find(|(known, _)| *known == upper)
        .map(|(_, id)| *id)
}

/// The words as the model's letters, a separator between each, and where
/// each word's letters are among them. A word with no letter the model
/// knows — a number, a word in another script — has no range.
#[must_use]
pub fn tokens(words: &[&str]) -> (Vec<usize>, Vec<Option<(usize, usize)>>) {
    let mut out = Vec::new();
    let mut ranges = Vec::with_capacity(words.len());
    for word in words {
        let letters: Vec<usize> = word.chars().filter_map(letter).collect();
        if letters.is_empty() {
            ranges.push(None);
            continue;
        }
        if !out.is_empty() {
            out.push(SPACE);
        }
        let start = out.len();
        out.extend(letters);
        ranges.push(Some((start, out.len())));
    }
    (out, ranges)
}

/// The most likely path through `tokens`, in order, over `frames` frames of
/// `classes` log-probabilities each: for every token, the frame it was
/// first heard at. `None` when there are more tokens than frames.
///
/// Between tokens a frame may be heard as nothing (the blank) or as the
/// token still sounding — a sung vowel is long.
#[must_use]
pub fn force_align(
    emission: &[f32],
    frames: usize,
    classes: usize,
    tokens: &[usize],
) -> Option<Vec<usize>> {
    let n = tokens.len();
    if n == 0 || frames < n || emission.len() < frames * classes {
        return None;
    }
    let at = |t: usize, c: usize| f64::from(emission[t * classes + c]);
    let width = n + 1;
    let mut score = vec![f64::NEG_INFINITY; (frames + 1) * width];
    // Whether state j at frame t+1 was reached by moving on from j-1.
    let mut moved = vec![false; frames * width];
    score[0] = 0.0;
    for t in 0..frames {
        for j in 0..=n {
            let here = score[t * width + j];
            let mut stay = here + at(t, BLANK);
            if j > 0 {
                stay = stay.max(here + at(t, tokens[j - 1]));
            }
            let advance = if j > 0 {
                score[t * width + j - 1] + at(t, tokens[j - 1])
            } else {
                f64::NEG_INFINITY
            };
            let (best, came) = if advance > stay {
                (advance, true)
            } else {
                (stay, false)
            };
            score[(t + 1) * width + j] = best;
            moved[t * width + j] = came;
        }
    }
    if !score[frames * width + n].is_finite() {
        return None;
    }
    let mut first = vec![0usize; n];
    let mut j = n;
    for t in (0..frames).rev() {
        if j == 0 {
            break;
        }
        if moved[t * width + j] {
            first[j - 1] = t;
            j -= 1;
        }
    }
    (j == 0).then_some(first)
}

/// Each word's start and end frame, from where its tokens were first heard:
/// it starts at its first letter and ends where the separator after it — or
/// the audio — begins.
#[must_use]
pub fn word_frames(
    first: &[usize],
    ranges: &[Option<(usize, usize)>],
    frames: usize,
) -> Vec<Option<(usize, usize)>> {
    ranges
        .iter()
        .map(|range| {
            let (start, end) = (*range)?;
            let from = *first.get(start)?;
            let to = first.get(end).copied().unwrap_or(frames).max(from + 1);
            Some((from, to))
        })
        .collect()
}

/// Zero mean and unit variance, as the model was trained on.
fn normalise(audio: &[f32]) -> Vec<f32> {
    let n = audio.len().max(1) as f64;
    let mean = audio.iter().map(|&x| f64::from(x)).sum::<f64>() / n;
    let variance = audio
        .iter()
        .map(|&x| (f64::from(x) - mean).powi(2))
        .sum::<f64>()
        / n;
    let scale = 1.0 / (variance + 1e-7).sqrt();
    audio
        .iter()
        .map(|&x| ((f64::from(x) - mean) * scale) as f32)
        .collect()
}

/// Log-probabilities from raw scores, a frame at a time.
fn log_softmax(values: &mut [f32], classes: usize) {
    for frame in values.chunks_mut(classes) {
        let top = frame.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let sum: f32 = frame.iter().map(|&v| (v - top).exp()).sum();
        let log_sum = sum.ln() + top;
        for v in frame.iter_mut() {
            *v -= log_sum;
        }
    }
}

/// The English aligner, loaded.
pub struct Aligner {
    session: Mutex<Session>,
    input: String,
    output: String,
}

impl Aligner {
    /// Load the model at `model_path`, or say why it cannot be — the same
    /// checks, in the same order, as the stems' engine: the runtime first,
    /// then the model.
    ///
    /// # Errors
    /// No ONNX Runtime, no model, or a model that would not load.
    pub fn new(model_path: &Path) -> Result<Self, Unavailable> {
        let library = availability::runtime_library();
        availability::probe_named_runtime(&library)?;
        availability::probe_model(model_path)?;
        let environment = ort::init_from(&library).map_err(|error| Unavailable::Runtime {
            library: library.clone(),
            reason: error.to_string(),
        })?;
        let _ = environment.with_name("djmanzo-align").commit();
        let build = || -> Result<Session, ort::Error> {
            Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(4)?
                .commit_from_file(model_path)
        };
        let session = build().map_err(|error| Unavailable::Session {
            reason: error.to_string(),
        })?;
        let input = session
            .inputs()
            .first()
            .map(|outlet| outlet.name().to_owned())
            .ok_or_else(|| Unavailable::Unsuited {
                reason: "the aligner declares no input".to_owned(),
            })?;
        let output = session
            .outputs()
            .first()
            .map(|outlet| outlet.name().to_owned())
            .ok_or_else(|| Unavailable::Unsuited {
                reason: "the aligner declares no output".to_owned(),
            })?;
        Ok(Self {
            session: Mutex::new(session),
            input,
            output,
        })
    }

    /// The model's log-probabilities for `audio`, a row of classes per
    /// twenty-millisecond frame, with how many frames and classes.
    fn emissions(&self, audio: &[f32]) -> Result<(Vec<f32>, usize, usize), String> {
        let samples = audio.len();
        let tensor = ort::value::Tensor::from_array(([1, samples], normalise(audio)))
            .map_err(|error| error.to_string())?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| "the aligner is wedged".to_owned())?;
        let outputs = session
            .run(ort::inputs![self.input.as_str() => tensor])
            .map_err(|error| error.to_string())?;
        let (shape, values) = outputs[self.output.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|error| error.to_string())?;
        let (frames, classes) = match shape[..] {
            [1, frames, classes] => (
                usize::try_from(frames).map_err(|_| "negative frames".to_owned())?,
                usize::try_from(classes).map_err(|_| "negative classes".to_owned())?,
            ),
            _ => return Err(format!("the aligner gave back {:?}", &shape[..])),
        };
        let mut values = values.to_vec();
        log_softmax(&mut values, classes);
        Ok((values, frames, classes))
    }

    /// Place `words`, sung in that order, in `audio` — sixteen-kilohertz
    /// mono, one line's stretch. Each word's start and end in seconds from
    /// the start of `audio`, or `None` for one with no letters the model
    /// knows, or when the line could not be placed at all.
    ///
    /// # Errors
    /// What the runtime said.
    pub fn align(&self, audio: &[f32], words: &[&str]) -> Result<Vec<Option<(f64, f64)>>, String> {
        let (letters, ranges) = tokens(words);
        if letters.is_empty() || audio.len() < HOP * 2 {
            return Ok(vec![None; words.len()]);
        }
        let (emission, frames, classes) = self.emissions(audio)?;
        let Some(first) = force_align(&emission, frames, classes, &letters) else {
            return Ok(vec![None; words.len()]);
        };
        let seconds = |frame: usize| (frame * HOP) as f64 / f64::from(RATE);
        Ok(word_frames(&first, &ranges, frames)
            .into_iter()
            .map(|span| span.map(|(from, to)| (seconds(from), seconds(to))))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_become_the_models_letters_with_a_space_between() {
        let (letters, ranges) = tokens(&["Hi,", "42", "you'll"]);
        // H I | Y O U ' L L
        assert_eq!(letters, vec![11, 10, SPACE, 22, 8, 16, 27, 15, 15]);
        assert_eq!(ranges, vec![Some((0, 2)), None, Some((3, 9))]);
    }

    /// **The load-bearing one.** An emission that says, frame by frame, "H"
    /// then nothing, then "I", then a long nothing, then the space and "A"
    /// is aligned to exactly those frames — a sung vowel held over several
    /// frames stays one letter.
    #[test]
    fn the_path_follows_where_each_letter_was_heard() {
        let classes = 32;
        // Frames: 0 H, 1 blank, 2 I, 3 I (held), 4 blank, 5 blank, 6 |, 7 A, 8 A, 9 blank.
        let heard = [11, BLANK, 10, 10, BLANK, BLANK, SPACE, 7, 7, BLANK];
        let mut emission = vec![-10.0f32; heard.len() * classes];
        for (t, &c) in heard.iter().enumerate() {
            emission[t * classes + c] = -0.01;
        }
        let (letters, ranges) = tokens(&["hi", "a"]);
        let first = force_align(&emission, heard.len(), classes, &letters).expect("aligned");
        assert_eq!(first, vec![0, 2, 6, 7]);
        let words = word_frames(&first, &ranges, heard.len());
        assert_eq!(words, vec![Some((0, 6)), Some((7, 10))]);
    }

    /// A long sung vowel: the model hears the letter on and on and silence
    /// nowhere in it, then a rest, then the next word. The letter is held to
    /// its end. Were a held letter only allowed to stretch over silence, the
    /// cheapest path would rush through the next word inside the vowel —
    /// where silence is least likely — and keep its silences for the rest.
    #[test]
    fn a_held_vowel_is_one_letter_to_its_end() {
        let classes = 32;
        let frames = 11;
        let mut emission = vec![-10.0f32; frames * classes];
        let mut set = |t: usize, c: usize, v: f32| emission[t * classes + c] = v;
        // 0 H; 1..=4 I held (the next word's letters faint in it).
        set(0, 11, -0.01);
        for t in 1..=4 {
            set(t, 10, -0.01);
            set(t, SPACE, -3.0);
            set(t, 7, -3.0);
        }
        // 5..=8 a rest.
        for t in 5..=8 {
            set(t, BLANK, -0.01);
        }
        // 9 the space, 10 A, silence likely around them too.
        set(9, SPACE, -0.01);
        set(9, BLANK, -0.5);
        set(10, 7, -0.01);
        set(10, BLANK, -0.5);
        let (letters, ranges) = tokens(&["hi", "a"]);
        let first = force_align(&emission, frames, classes, &letters).expect("aligned");
        assert_eq!(first, vec![0, 1, 9, 10], "{first:?}");
        assert_eq!(
            word_frames(&first, &ranges, frames),
            vec![Some((0, 9)), Some((10, 11))]
        );
    }

    #[test]
    fn more_letters_than_frames_is_no_path() {
        let emission = vec![0.0f32; 2 * 32];
        assert_eq!(force_align(&emission, 2, 32, &[11, 10, 4]), None);
        assert_eq!(force_align(&emission, 2, 32, &[]), None);
    }

    #[test]
    fn the_audio_is_normalised_and_scores_are_log_probabilities() {
        let normal = normalise(&[1.0, 3.0, 1.0, 3.0]);
        let mean: f32 = normal.iter().sum::<f32>() / 4.0;
        assert!(mean.abs() < 1e-6);
        assert!((normal[0] + 1.0).abs() < 1e-3 && (normal[1] - 1.0).abs() < 1e-3);
        let mut scores = vec![1.0f32, 2.0, 3.0, 0.0, 0.0, 0.0];
        log_softmax(&mut scores, 3);
        for frame in scores.chunks(3) {
            let total: f32 = frame.iter().map(|v| v.exp()).sum();
            assert!((total - 1.0).abs() < 1e-5);
        }
    }

    /// **The model itself, on real speech.** Ignored: it needs the model and
    /// ONNX Runtime.
    ///
    /// ```text
    /// ORT_DYLIB_PATH=<libonnxruntime> DJMANZO_ALIGNER=<model_quantized.onnx> \
    ///   DJMANZO_ALIGNER_WAV=<16 kHz mono WAV> DJMANZO_ALIGNER_WORDS="the words" \
    ///   cargo test -p dj-stems --lib aligns_real -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs the aligner model and ONNX Runtime"]
    fn aligns_real_speech() {
        let (Ok(model), Ok(wav), Ok(words)) = (
            std::env::var("DJMANZO_ALIGNER"),
            std::env::var("DJMANZO_ALIGNER_WAV"),
            std::env::var("DJMANZO_ALIGNER_WORDS"),
        ) else {
            return;
        };
        let bytes = std::fs::read(wav).expect("read");
        let audio: Vec<f32> = bytes[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
            .collect();
        let aligner = Aligner::new(Path::new(&model)).expect("loaded");
        let words: Vec<&str> = words.split_whitespace().collect();
        let started = std::time::Instant::now();
        let placed = aligner.align(&audio, &words).expect("aligned");
        for (word, at) in words.iter().zip(&placed) {
            eprintln!("{word}: {at:?}");
        }
        eprintln!("{:.2} s", started.elapsed().as_secs_f64());
    }
}

//! whisper.cpp, in a process of its own when it has to be.
//!
//! Speech recognition — §122's words in time and A2's spoken commands — is
//! whisper.cpp, a C++ library that needs macOS 10.15. The karaoke build starts
//! on 10.13 and so cannot link it, and a friend's Mac is not always the 2011
//! laptop it was built for. This crate is the half that can be built apart:
//! the call into whisper.cpp ([`transcribe`]) and a helper program,
//! `djmanzo-listen`, that serves it over a pipe ([`serve`]). The application
//! calls it in-process where it is compiled in, and runs the helper where it
//! is not — which is how one karaoke download listens on a Mac that can and
//! says why on one that cannot.
//!
//! # What comes back
//!
//! whisper.cpp's own answer — pieces, tokens and their times — and nothing
//! more ([`Raw`]). Turning tokens into sung words, dropping Whisper's loops
//! and placing the words already known stay in `dj_app::whispercpp`, so the
//! answer in-process and the answer from the helper go through the same code
//! and cannot come to differ.
//!
//! # The helper's protocol
//!
//! One JSON [`Request`] on a line of standard input; the audio beside it in a
//! file of little-endian `f32`s, because a song at sixteen kilohertz is
//! eleven megabytes and a pipe is not where that belongs. Then [`Line`]s on
//! standard output, one per line: progress as it comes, and one answer —
//! [`Line::Heard`] or [`Line::Refused`]. `--probe` answers [`Line::Ready`] or
//! [`Line::Refused`] without loading anything: it is how the application asks
//! whether this machine can run the helper at all, by running it.
//!
//! The helper leaves when its standard input closes, so an application that
//! quits mid-song does not leave whisper.cpp working for nobody.
//!
//! # A process of its own is also a safety
//!
//! A crash in whisper.cpp — a model file cut short, a processor it misjudged
//! — is a failed transcription when it happens in the helper, and the decks
//! keep playing. In-process it would take the night down with it.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Whether whisper.cpp is compiled into this build.
pub const BUILT_IN: bool = cfg!(feature = "whisper");

/// The helper's file name, beside the application's own executable.
pub const HELPER: &str = if cfg!(windows) {
    "djmanzo-listen.exe"
} else {
    "djmanzo-listen"
};

/// One token as Whisper gave it: its text, whether it is one of Whisper's own
/// markers rather than a word, and when it was heard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub text: String,
    pub special: bool,
    /// Where Whisper's attention placed it, in seconds.
    pub at: f64,
}

/// One stretch Whisper heard: its times and its tokens. A spoken command is
/// one piece with its text and no tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub tokens: Vec<Token>,
}

/// whisper.cpp's answer, before djmanzo makes words of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Raw {
    /// The language asked for, or the one Whisper heard.
    pub language: String,
    pub pieces: Vec<Piece>,
    /// Loading the model, then listening, in seconds.
    pub load_seconds: f64,
    pub listen_seconds: f64,
}

/// What is being listened for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// A record's words, each with its time.
    Words,
    /// A spoken command: seconds long, one piece, no word times.
    Command,
}

/// One thing to listen to, apart from the audio itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ask {
    pub mode: Mode,
    /// The model file.
    pub model: PathBuf,
    /// The model's id — `tiny`, `base`, `small`, `medium`, anything else the
    /// large one — which chooses the alignment heads for word times.
    pub id: String,
    /// The model's name, for sentences.
    pub name: String,
    /// A two-letter code, or none for Whisper to tell. Commands always tell.
    pub language: Option<String>,
    /// The words already known, which steer Whisper's spelling.
    pub prompt: Option<String>,
    pub threads: usize,
}

/// What the helper reads: the ask, and where its audio is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub ask: Ask,
    /// Sixteen-kilohertz mono, little-endian `f32`s.
    pub audio: PathBuf,
}

/// One line the helper writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Line {
    /// `--probe`: this machine can run it.
    Ready,
    /// Percent listened.
    Progress(i32),
    Heard(Raw),
    /// Why not, in a sentence.
    Refused(String),
}

/// Whether this processor can run the whisper.cpp djmanzo builds.
///
/// `.cargo/config.toml` pins whisper.cpp to AVX2, FMA and F16C on x86-64,
/// rather than to whatever the build machine had: every Intel and AMD
/// processor since about 2013 has them, and a 2011 MacBook's does not.
///
/// # Errors
/// An x86-64 processor without them.
pub fn processor_can_run() -> Result<(), String> {
    #[cfg(target_arch = "x86_64")]
    {
        if !(std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
            && std::arch::is_x86_feature_detected!("f16c"))
        {
            return Err(
                "this processor lacks AVX2, which djmanzo's speech recognition needs (every Intel and AMD processor since about 2013 has it)"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// The audio file's bytes as samples.
///
/// # Errors
/// A length that is not whole samples.
pub fn samples_of(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "the audio is {} bytes, which is not whole samples",
            bytes.len()
        ));
    }
    let (samples, _) = bytes.as_chunks::<4>();
    Ok(samples
        .iter()
        .map(|four| f32::from_le_bytes(*four))
        .collect())
}

/// Samples as the audio file's bytes.
#[must_use]
pub fn bytes_of(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

/// A line of the protocol, as it is written.
fn write_line(output: &Mutex<dyn Write + Send>, line: &Line) {
    let Ok(text) = serde_json::to_string(line) else {
        return;
    };
    if let Ok(mut output) = output.lock() {
        // A parent that has gone cannot be told; the stdin watcher ends the
        // process for that.
        let _ = writeln!(output, "{text}");
        let _ = output.flush();
    }
}

/// How the helper listens: [`transcribe`] in the program, something
/// simpler in a test.
pub type Engine = fn(&Ask, &[f32], Box<dyn FnMut(i32)>) -> Result<Raw, String>;

/// The helper's whole conversation: `--probe`, or one request answered.
///
/// `input` is read for the request's line only; the program watches the
/// rest for its close. Returns the process's exit code: 0 whenever an
/// answer was written, including a refusal, because a refusal is an answer.
pub fn serve(
    probe: bool,
    input: &mut dyn BufRead,
    output: Arc<Mutex<dyn Write + Send>>,
    engine: Engine,
) -> i32 {
    if let Err(why) = processor_can_run() {
        write_line(&output, &Line::Refused(why));
        return 0;
    }
    if probe {
        write_line(&output, &Line::Ready);
        return 0;
    }
    let mut first = String::new();
    if input.read_line(&mut first).is_err() || first.trim().is_empty() {
        write_line(
            &output,
            &Line::Refused("nothing was asked of the listening helper".to_owned()),
        );
        return 2;
    }
    let answer = serde_json::from_str::<Request>(first.trim())
        .map_err(|e| format!("the listening helper could not read what was asked: {e}"))
        .and_then(|request| {
            let bytes = std::fs::read(&request.audio)
                .map_err(|e| format!("{}: {e}", request.audio.display()))?;
            let audio = samples_of(&bytes)?;
            let told = Arc::clone(&output);
            let progress = Box::new(move |percent| write_line(&told, &Line::Progress(percent)));
            engine(&request.ask, &audio, progress)
        });
    match answer {
        Ok(raw) => write_line(&output, &Line::Heard(raw)),
        Err(why) => write_line(&output, &Line::Refused(why)),
    }
    0
}

/// The model file as whisper.cpp opens it.
#[cfg(feature = "whisper")]
fn path_text(path: &std::path::Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{} is not a path whisper.cpp can open", path.display()))
}

/// The record's words with their times, or a spoken command's words, from
/// `audio` — sixteen-kilohertz mono — with the model the ask names.
///
/// Words: token timestamps through the model's alignment heads, the language
/// given or told, the known words as the prompt, and `progress` told the
/// percentage. A command: Whisper's own thirty-second window, one segment, no
/// context carried from one take to the next, the language always told (see
/// `dj_app::whispercpp::command` for what that was measured against).
///
/// # Errors
/// A processor that cannot run it, a model that would not load, or a run
/// that failed.
#[cfg(feature = "whisper")]
pub fn transcribe(ask: &Ask, audio: &[f32], progress: Box<dyn FnMut(i32)>) -> Result<Raw, String> {
    use whisper_rs::{
        DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
        WhisperContextParameters,
    };
    processor_can_run()?;
    whisper_rs::install_logging_hooks();
    let began = std::time::Instant::now();
    let context = match ask.mode {
        Mode::Words => WhisperContextParameters {
            dtw_parameters: DtwParameters {
                mode: DtwMode::ModelPreset {
                    model_preset: match ask.id.as_str() {
                        "tiny" => DtwModelPreset::Tiny,
                        "base" => DtwModelPreset::Base,
                        "small" => DtwModelPreset::Small,
                        "medium" => DtwModelPreset::Medium,
                        _ => DtwModelPreset::LargeV3Turbo,
                    },
                },
                ..DtwParameters::default()
            },
            ..WhisperContextParameters::default()
        },
        Mode::Command => WhisperContextParameters::default(),
    };
    let whisper = WhisperContext::new_with_params(path_text(&ask.model)?, context)
        .map_err(|e| format!("{} would not load: {e}", ask.name))?;
    let mut state = whisper
        .create_state()
        .map_err(|e| format!("{} would not start: {e}", ask.name))?;
    let load_seconds = began.elapsed().as_secs_f64();

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(i32::try_from(ask.threads.max(1)).unwrap_or(4));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    if let Some(prompt) = ask.prompt.as_deref().filter(|p| !p.trim().is_empty()) {
        params.set_initial_prompt(prompt);
    }
    match ask.mode {
        Mode::Words => {
            params.set_language(Some(ask.language.as_deref().unwrap_or("auto")));
            params.set_token_timestamps(true);
            params.set_progress_callback_safe(progress);
        }
        Mode::Command => {
            params.set_language(Some("auto"));
            params.set_single_segment(true);
            params.set_no_context(true);
            drop(progress);
        }
    }
    let began = std::time::Instant::now();
    state
        .full(params, audio)
        .map_err(|e| format!("{} could not listen: {e}", ask.name))?;
    let listen_seconds = began.elapsed().as_secs_f64();

    let told = || {
        whisper_rs::get_lang_str(state.full_lang_id_from_state())
            .unwrap_or("en")
            .to_owned()
    };
    let pieces = match ask.mode {
        Mode::Words => {
            let end_of_text = whisper.token_eot();
            state
                .as_iter()
                .map(|segment| Piece {
                    start: segment.start_timestamp() as f64 / 100.0,
                    end: segment.end_timestamp() as f64 / 100.0,
                    text: String::new(),
                    tokens: (0..segment.n_tokens())
                        .filter_map(|index| segment.get_token(index))
                        .map(|token| Token {
                            text: token
                                .to_str_lossy()
                                .map(|text| text.into_owned())
                                .unwrap_or_default(),
                            special: token.token_id() >= end_of_text,
                            at: token.token_data().t_dtw as f64 / 100.0,
                        })
                        .collect(),
                })
                .collect()
        }
        Mode::Command => vec![Piece {
            start: 0.0,
            end: 0.0,
            text: state
                .as_iter()
                .map(|segment| {
                    segment
                        .to_str_lossy()
                        .map(|text| text.into_owned())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_owned(),
            tokens: Vec::new(),
        }],
    };
    let language = match ask.mode {
        Mode::Words => ask.language.clone().unwrap_or_else(told),
        Mode::Command => told(),
    };
    Ok(Raw {
        language,
        pieces,
        load_seconds,
        listen_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask() -> Ask {
        Ask {
            mode: Mode::Words,
            model: PathBuf::from("/models/ggml-base.bin"),
            id: "base".to_owned(),
            name: "Base".to_owned(),
            language: Some("es".to_owned()),
            prompt: None,
            threads: 4,
        }
    }

    fn heard() -> Raw {
        Raw {
            language: "es".to_owned(),
            pieces: vec![Piece {
                start: 1.5,
                end: 3.0,
                text: String::new(),
                tokens: vec![Token {
                    text: " Vivir".to_owned(),
                    special: false,
                    at: 1.6,
                }],
            }],
            load_seconds: 0.2,
            listen_seconds: 1.1,
        }
    }

    /// A conversation, as the helper has it with the application.
    fn converse(probe: bool, request: &str, engine: Engine) -> (i32, Vec<Line>) {
        let written: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::new(Mutex::new(Shared(Arc::clone(&written))));
        let code = serve(probe, &mut request.as_bytes(), output, engine);
        let text = String::from_utf8(written.lock().expect("written").clone()).expect("text");
        let lines = text
            .lines()
            .map(|line| serde_json::from_str(line).expect("a line of the protocol"))
            .collect();
        (code, lines)
    }

    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("written").extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn listens(ask: &Ask, audio: &[f32], mut progress: Box<dyn FnMut(i32)>) -> Result<Raw, String> {
        assert_eq!(ask.id, "base");
        assert_eq!(audio, [0.25, -0.5, 1.0]);
        progress(50);
        Ok(heard())
    }

    fn refuses(_: &Ask, _: &[f32], _: Box<dyn FnMut(i32)>) -> Result<Raw, String> {
        Err("Base would not load: cut short".to_owned())
    }

    /// **The helper answers what the application asked, over the pipe** —
    /// the request read whole, the audio from its file sample for sample,
    /// progress as it comes, then whisper's answer as it gave it; a refusal
    /// is an answer too, written as one.
    #[test]
    fn the_helper_answers_a_request_over_the_pipe() {
        if processor_can_run().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("dj-whisper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let audio = dir.join("take.f32");
        std::fs::write(&audio, bytes_of(&[0.25, -0.5, 1.0])).expect("written");
        let request = serde_json::to_string(&Request { ask: ask(), audio }).expect("json");

        let (code, lines) = converse(false, &format!("{request}\n"), listens);
        assert_eq!(code, 0);
        assert_eq!(lines, vec![Line::Progress(50), Line::Heard(heard())]);

        let (code, lines) = converse(false, &format!("{request}\n"), refuses);
        assert_eq!(code, 0);
        assert_eq!(
            lines,
            vec![Line::Refused("Base would not load: cut short".to_owned())]
        );

        let (code, lines) = converse(false, "", listens);
        assert_eq!(code, 2);
        assert!(matches!(lines.as_slice(), [Line::Refused(_)]), "{lines:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// **A probe loads nothing and says whether this machine can.**
    #[test]
    fn a_probe_says_ready_or_why_not() {
        let (code, lines) = converse(true, "", refuses);
        assert_eq!(code, 0);
        match processor_can_run() {
            Ok(()) => assert_eq!(lines, vec![Line::Ready]),
            Err(why) => assert_eq!(lines, vec![Line::Refused(why)]),
        }
    }

    /// The audio crosses as little-endian samples, and a file that is not
    /// whole samples is refused rather than read short.
    #[test]
    fn audio_crosses_sample_for_sample() {
        let samples = [0.0, 1.0, -1.0, 0.123_456_7, f32::MIN_POSITIVE];
        assert_eq!(samples_of(&bytes_of(&samples)), Ok(samples.to_vec()));
        assert!(samples_of(&[0, 0, 128]).is_err());
    }

    /// The protocol's lines as they are written, so the application and an
    /// older helper agree on them.
    #[test]
    fn the_lines_are_written_as_the_protocol_says() {
        assert_eq!(
            serde_json::to_string(&Line::Ready).expect("json"),
            r#""ready""#
        );
        assert_eq!(
            serde_json::to_string(&Line::Progress(37)).expect("json"),
            r#"{"progress":37}"#
        );
        assert_eq!(
            serde_json::to_string(&Line::Refused("no".to_owned())).expect("json"),
            r#"{"refused":"no"}"#
        );
    }
}

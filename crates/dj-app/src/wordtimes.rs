//! §122: when each word is sung, found by WhisperX.
//!
//! > I prefer x-whisper, since it also tells the timestamps for the
//! > words/lyrics ... that's necessary for karaoke ... if x-whipser works on
//! > all platforms somehow, do use it.
//!
//! It does: WhisperX (BSD-2-Clause) runs on the CPU on Linux, macOS — Apple
//! Silicon included — and Windows. It is Python and PyTorch, which no
//! package djmanzo ships can carry, so djmanzo installs it **on first use**,
//! into a private environment of its own, with `uv` (MIT OR Apache-2.0):
//! one small program that fetches a Python as well when the machine has
//! none. Nothing is installed system-wide and nothing else on the machine is
//! touched; deleting the tools folder undoes it.
//!
//! # Two jobs, and the cheap one first
//!
//! WhisperX does two things: it **transcribes** (faster-whisper) and it
//! **aligns** — a wav2vec2 model placing each known word in the audio, which
//! is where word times come from. When the words are already known — a
//! record's tags, a sidecar `.lrc`, LRCLIB — only the second is needed, and it
//! is the cheap one: the words are given, so nothing has to be recognised.
//! Only a record whose words nobody has is transcribed first, with the small
//! `base` model, because the owner's budget is fifteen seconds for a three-
//! to five-minute song on a medium laptop and the larger models do not fit it
//! on a CPU.
//!
//! **Every run is timed**, stage by stage, and says whether it met that
//! budget ([`Report::within_budget`]). This module cannot promise it: the
//! time depends on the machine, and the one this was written on could not
//! download a model to measure with.
//!
//! # The helper, and what crosses between
//!
//! WhisperX is driven by a short Python helper of djmanzo's own
//! ([`HELPER`]), written into the tools folder and run as a child process on
//! a worker thread — never near the audio thread. A [`Job`] goes in on its
//! standard input as JSON; an [`Answer`] comes back on its standard output;
//! anything it says on standard error is progress, and the last of it is the
//! reason when it fails. The audio is handed over already decoded, as a
//! sixteen-kilohertz mono WAV, so WhisperX never needs FFmpeg — which it
//! otherwise calls to read files, and which a Mac or a Windows machine does
//! not have.
//!
//! What comes back becomes **enhanced LRC** ([`to_lrc`]) — a line time and a
//! time before every word — which the library keeps as the record's timed
//! words and the singers' screen already wipes word by word
//! (`dj_library::lrc`).

use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The owner's budget: longer than this for a three- to five-minute song and
/// "it's not an option".
pub const BUDGET_SECONDS: f64 = 15.0;

/// What is installed, exactly.
pub const WHISPERX: &str = "whisperx==3.8.6";

/// The `uv` fetched when the machine has none.
pub const UV_VERSION: &str = "0.12.19";

/// The Python the private environment is made with.
pub const PYTHON: &str = "3.12";

/// The transcription model, for a record whose words nobody has.
pub const MODEL: &str = "base";

/// What WhisperX is handed: mono, at the rate its models were trained on.
pub const SAMPLE_RATE: u32 = 16_000;

/// Aligners chosen over WhisperX's own, language by language, because of
/// their licences.
///
/// WhisperX's defaults for French, German, Spanish and Italian are torchaudio's
/// VoxPopuli models, published under **CC BY-NC 4.0** — non-commercial — and a
/// DJ timing words for a paid night is commercial use. These are Apache-2.0,
/// each per its model card. English's default (wav2vec 2.0, MIT) and every
/// other language's are WhisperX's own; see `docs/RESEARCH.md`.
pub const ALIGNERS: [(&str, &str); 4] = [
    ("de", "jonatasgrosman/wav2vec2-large-xlsr-53-german"),
    ("it", "jonatasgrosman/wav2vec2-large-xlsr-53-italian"),
    ("es", "facebook/wav2vec2-large-xlsr-53-spanish"),
    ("fr", "facebook/wav2vec2-large-xlsr-53-french"),
];

/// The helper djmanzo runs WhisperX through.
pub const HELPER: &str = include_str!("wordtimes_helper.py");

/// Longer than any run is allowed before it is called stuck: a first run
/// also downloads the models.
pub const TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A stretch of the record whose words are known, and roughly when it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Known {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// What the helper is asked.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Job {
    /// A sixteen-kilohertz mono WAV of the record.
    pub audio: PathBuf,
    /// The words' language, as a two-letter code, when it is known.
    pub language: Option<String>,
    /// The words, where they are known: then only alignment runs.
    pub lines: Vec<Known>,
    pub model: String,
    pub threads: usize,
    /// [`ALIGNERS`], as language code to model.
    pub aligners: std::collections::BTreeMap<String, String>,
}

/// [`ALIGNERS`] as the job carries them.
#[must_use]
pub fn aligners() -> std::collections::BTreeMap<String, String> {
    ALIGNERS
        .iter()
        .map(|(language, model)| ((*language).to_owned(), (*model).to_owned()))
        .collect()
}

/// One word, and when it is sung. A word the aligner could not place — a
/// number, a symbol — has no times.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimedWord {
    pub word: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
    #[serde(default)]
    pub score: Option<f64>,
}

/// One line of the answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Segment {
    pub start: Option<f64>,
    pub end: Option<f64>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub words: Vec<TimedWord>,
}

/// What the helper answers.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Answer {
    /// `align` when the words were given, `transcribe` when they were found.
    pub mode: String,
    pub language: String,
    pub segments: Vec<Segment>,
    /// How long each stage took, in seconds, as the helper measured it.
    #[serde(default)]
    pub stages: Vec<(String, f64)>,
}

/// How a run went, for the DJ.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub mode: String,
    pub language: String,
    /// Words with a time.
    pub words: usize,
    /// Each stage and its seconds: preparing the audio here, then the
    /// helper's own.
    pub stages: Vec<(String, f64)>,
    /// From pressing the button to the words being kept.
    pub seconds: f64,
    /// The record's length, for the budget's sake.
    pub record_seconds: f64,
    pub within_budget: bool,
    /// `vocals` when WhisperX was handed the separated vocal stem, `mix`
    /// when separation had not finished and it heard the whole record.
    pub heard: String,
}

/// The separated vocals of a record `frames` long, as interleaved stereo —
/// only when **every** frame has been separated. A record half separated is
/// handed over as the mix instead: words placed against vocals that fall
/// silent halfway would stop being placed halfway.
#[must_use]
pub fn vocals_of(table: &dj_decode::StemTable, frames: usize) -> Option<Vec<f32>> {
    const VOCALS: usize = 0;
    let channels = dj_decode::CHANNELS;
    if frames == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(frames * channels);
    for index in 0..frames {
        let frame = table.frame(index)?;
        out.extend_from_slice(&frame[VOCALS * channels..(VOCALS + 1) * channels]);
    }
    Some(out)
}

/// The stretches whose words are known, from what the library holds.
///
/// Timed lines run each to the next, the last to the end of the record. Plain
/// words with no times are one stretch over the whole record: the aligner
/// then places every word itself.
#[must_use]
pub fn known_lines(synced: Option<&str>, plain: &str, record_seconds: f64) -> Vec<Known> {
    let timed: Vec<dj_library::lrc::Line> = synced
        .map(dj_library::lrc::parse)
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !line.text.trim().is_empty())
        .collect();
    if !timed.is_empty() {
        let mut out = Vec::with_capacity(timed.len());
        for (i, line) in timed.iter().enumerate() {
            let end = timed
                .get(i + 1)
                .map_or(record_seconds, |next| next.at)
                .max(line.at);
            out.push(Known {
                start: line.at,
                end,
                text: line.text.trim().to_owned(),
            });
        }
        return out;
    }
    let text: Vec<&str> = plain
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if text.is_empty() {
        return Vec::new();
    }
    vec![Known {
        start: 0.0,
        end: record_seconds,
        text: text.join(" "),
    }]
}

/// `mm:ss.xx`, as LRC writes a time.
#[must_use]
pub fn stamp(seconds: f64) -> String {
    let hundredths = (seconds.max(0.0) * 100.0).round() as u64;
    format!(
        "{:02}:{:02}.{:02}",
        hundredths / 6000,
        (hundredths / 100) % 60,
        hundredths % 100
    )
}

/// The answer as enhanced LRC: each line at its first word, a time before
/// every word. A word the aligner could not place takes the time the one
/// before it ended, so it is wiped with its neighbour rather than guessed at;
/// a line with no placed word at all is left out rather than timed wrong.
#[must_use]
pub fn to_lrc(answer: &Answer) -> String {
    let mut out = String::from("[re:djmanzo, words timed by WhisperX]\n");
    for segment in &answer.segments {
        let mut last = segment.start;
        let mut words = Vec::new();
        for word in &segment.words {
            let text = word.word.trim();
            if text.is_empty() {
                continue;
            }
            let at = word.start.or(last);
            if let Some(at) = at {
                words.push((at, text));
            }
            last = word.end.or(word.start).or(last);
        }
        let Some(&(first, _)) = words.first() else {
            continue;
        };
        out.push('[');
        out.push_str(&stamp(first));
        out.push(']');
        for (i, (at, text)) in words.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push('<');
            out.push_str(&stamp(*at));
            out.push('>');
            out.push_str(text);
        }
        out.push('\n');
    }
    out
}

/// The plain words of an answer, a line a segment.
#[must_use]
pub fn plain_of(answer: &Answer) -> String {
    answer
        .segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many words an answer placed.
#[must_use]
pub fn timed_words(answer: &Answer) -> usize {
    answer
        .segments
        .iter()
        .flat_map(|segment| &segment.words)
        .filter(|word| word.start.is_some())
        .count()
}

/// Run the helper with `python`: the job on its standard input, the answer
/// on its standard output.
///
/// # Errors
/// A helper that would not start, ran past `timeout`, failed — with the last
/// thing it said — or answered something that is not an [`Answer`].
pub fn run(python: &Path, helper: &Path, job: &Job, timeout: Duration) -> Result<Answer, String> {
    let mut child = Command::new(python)
        .arg(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{} would not start: {e}", python.display()))?;
    let input = serde_json::to_vec(job).map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input).map_err(|e| e.to_string())?;
    }
    // Read both pipes on their own threads, so a helper that fills one while
    // this waits on the other cannot wedge them both.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "WhisperX was still running after {} minutes and was stopped",
                timeout.as_secs() / 60
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    if !status.success() {
        let said = err
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("it said nothing");
        return Err(format!("WhisperX failed: {said}"));
    }
    let last = out
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "WhisperX answered nothing".to_owned())?;
    serde_json::from_str(last).map_err(|e| format!("WhisperX's answer could not be read: {e}"))
}

/// The record as WhisperX wants it: mono, sixteen kilohertz, sixteen-bit,
/// written to `to`. Answers its length in seconds.
///
/// # Errors
/// The file system's own sentence.
pub fn write_audio(
    interleaved: &[f32],
    channels: usize,
    rate: u32,
    to: &Path,
) -> Result<f64, String> {
    let channels = channels.max(1);
    let mono: Vec<f32> = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    let seconds = mono.len() as f64 / f64::from(rate.max(1));
    let resampled = to_sixteen_kilohertz(&mono, rate);
    let mut bytes = Vec::with_capacity(44 + resampled.len() * 2);
    let data =
        u32::try_from(resampled.len() * 2).map_err(|_| "the record is too long".to_owned())?;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for sample in resampled {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(to, bytes).map_err(|e| format!("{}: {e}", to.display()))?;
    Ok(seconds)
}

/// `input`, at `from` hertz, brought to [`SAMPLE_RATE`] for WhisperX.
///
/// Not `dj_stems::resample`, which computes its kernel sample by sample for
/// the stems' sake and took sixteen seconds over a four-minute record —
/// more than the owner's whole budget before WhisperX had started. Speech
/// recognition needs a clean low-pass and the right times, not a mastering
/// resampler: the kernel here is worked out once, for [`PHASES`] fractional
/// positions, and each output sample is one row of it against the input.
/// Rounding the position to the nearest of 512 phases moves a sample by at
/// most a thousandth of a sample, far below anything an aligner resolves.
#[must_use]
pub fn to_sixteen_kilohertz(input: &[f32], from: u32) -> Vec<f32> {
    if from == SAMPLE_RATE || from == 0 || input.is_empty() {
        return input.to_vec();
    }
    // Input samples per output sample.
    let step = f64::from(from) / f64::from(SAMPLE_RATE);
    // The cut-off, in cycles per input sample: just under the new Nyquist
    // when coming down, just under the old one when going up.
    let cutoff = 0.5 * 0.92 / step.max(1.0);
    // Eight zero crossings of the sinc each side.
    let half = (8.0 / (2.0 * cutoff)).ceil() as isize;
    let taps = (2 * half) as usize;
    let mut table = vec![0.0f32; (PHASES + 1) * taps];
    for phase in 0..=PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let row = &mut table[phase * taps..(phase + 1) * taps];
        let mut sum = 0.0;
        for (slot, weight) in row.iter_mut().enumerate() {
            // The distance from the output instant to this input sample.
            let x = (slot as isize - half + 1) as f64 - fraction;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                let arg = std::f64::consts::PI * 2.0 * cutoff * x;
                arg.sin() / arg
            };
            let edge = x / (half as f64 + 1.0);
            let window = if edge.abs() >= 1.0 {
                0.0
            } else {
                0.5 * (1.0 + (std::f64::consts::PI * edge).cos())
            };
            let value = sinc * window;
            *weight = value as f32;
            sum += value;
        }
        // Unity gain at every phase, so a steady level stays steady.
        if sum.abs() > 1e-12 {
            for weight in row.iter_mut() {
                *weight = (f64::from(*weight) / sum) as f32;
            }
        }
    }
    let out_len = (input.len() as f64 / step).round() as usize;
    let last = input.len() as isize - 1;
    let mut out = Vec::with_capacity(out_len);
    for n in 0..out_len {
        let at = n as f64 * step;
        let base = at.floor();
        let phase = ((at - base) * PHASES as f64).round() as usize;
        let row = &table[phase * taps..(phase + 1) * taps];
        let first = base as isize - half + 1;
        let mut acc = 0.0f32;
        if first >= 0 && first + taps as isize - 1 <= last {
            let start = first as usize;
            for (weight, sample) in row.iter().zip(&input[start..start + taps]) {
                acc += weight * sample;
            }
        } else {
            for (slot, weight) in row.iter().enumerate() {
                let index = first + slot as isize;
                if (0..=last).contains(&index) {
                    acc += weight * input[index as usize];
                }
            }
        }
        out.push(acc);
    }
    out
}

/// How many fractional positions [`to_sixteen_kilohertz`] works its kernel
/// out for.
pub const PHASES: usize = 512;

/// The operating systems djmanzo is built for, as far as installing goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
    Windows,
}

impl Os {
    #[must_use]
    pub const fn here() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// Where WhisperX and what runs it live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub root: PathBuf,
    pub os: Os,
}

impl Tools {
    /// The private environment.
    #[must_use]
    pub fn venv(&self) -> PathBuf {
        self.root.join("whisperx")
    }

    /// Its Python.
    #[must_use]
    pub fn python(&self) -> PathBuf {
        match self.os {
            Os::Windows => self.venv().join("Scripts").join("python.exe"),
            _ => self.venv().join("bin").join("python"),
        }
    }

    /// The helper, as written out of [`HELPER`].
    #[must_use]
    pub fn helper(&self) -> PathBuf {
        self.root.join("wordtimes_helper.py")
    }

    /// The `uv` djmanzo fetched, if it did.
    #[must_use]
    pub fn own_uv(&self) -> PathBuf {
        self.root.join("uv").join(match self.os {
            Os::Windows => "uv.exe",
            _ => "uv",
        })
    }

    /// Written last, once everything installed: its presence is the one
    /// answer to "is WhisperX here", so a half-finished install is not one.
    #[must_use]
    pub fn marker(&self) -> PathBuf {
        self.root.join("whisperx.installed")
    }

    #[must_use]
    pub fn installed(&self) -> bool {
        std::fs::read_to_string(self.marker()).is_ok_and(|text| text.trim() == WHISPERX)
            && self.python().exists()
    }
}

/// The archive `uv` is published in for this machine, if it is published.
#[must_use]
pub fn uv_archive(os: Os, arch: &str) -> Option<String> {
    let target = match (os, arch) {
        (Os::Linux, "x86_64") => "x86_64-unknown-linux-gnu",
        (Os::Linux, "aarch64") => "aarch64-unknown-linux-gnu",
        (Os::MacOs, "aarch64") => "aarch64-apple-darwin",
        (Os::MacOs, "x86_64") => "x86_64-apple-darwin",
        (Os::Windows, "x86_64") => "x86_64-pc-windows-msvc",
        (Os::Windows, "aarch64") => "aarch64-pc-windows-msvc",
        _ => return None,
    };
    Some(match os {
        Os::Windows => format!("uv-{target}.zip"),
        _ => format!("uv-{target}.tar.gz"),
    })
}

/// Where a release of `uv` is published.
#[must_use]
pub fn uv_url(archive: &str) -> String {
    format!("https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/{archive}")
}

/// Whether `bytes` are what a published `.sha256` file says they are. The
/// file is `<hex>  <name>`; only the hex is read.
#[must_use]
pub fn checksum_matches(bytes: &[u8], published: &str) -> bool {
    let Some(expected) = published.split_whitespace().next() else {
        return false;
    };
    let actual = sha2::Sha256::digest(bytes);
    let hex: String = actual.iter().map(|b| format!("{b:02x}")).collect();
    expected.eq_ignore_ascii_case(&hex)
}

/// The commands that install WhisperX with `uv`: a private environment with
/// its own Python — fetched by `uv` when the machine has no suitable one —
/// and WhisperX in it, with PyTorch's CPU build (`--torch-backend cpu`: on
/// Linux PyPI's default PyTorch brings two gigabytes of graphics-card
/// libraries a CPU-only run never loads).
#[must_use]
pub fn install_steps(uv: &Path, tools: &Tools) -> Vec<Vec<String>> {
    let uv = uv.display().to_string();
    let venv = tools.venv().display().to_string();
    let python = tools.python().display().to_string();
    vec![
        vec![
            uv.clone(),
            "venv".into(),
            "--python".into(),
            PYTHON.into(),
            "--seed".into(),
            venv,
        ],
        vec![
            uv,
            "pip".into(),
            "install".into(),
            "--python".into(),
            python,
            "--torch-backend".into(),
            "cpu".into(),
            WHISPERX.into(),
        ],
    ]
}

/// Where an install has got to, for the interface to read.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Progress {
    pub installing: bool,
    /// What it is doing now, in words.
    pub step: Option<String>,
    /// Why the last install or run failed.
    pub error: Option<String>,
    /// How the last run went.
    pub last: Option<Report>,
}

/// Install WhisperX into `tools`: `uv` fetched and checked if djmanzo has
/// none, then [`install_steps`], then the marker. `say` is told each step.
///
/// # Errors
/// A download that failed or did not match its published checksum, an
/// archive with no `uv` in it, or a step that failed — with what it said.
pub async fn install(
    tools: &Tools,
    http: &reqwest::Client,
    say: impl Fn(&str),
) -> Result<(), String> {
    std::fs::create_dir_all(&tools.root).map_err(|e| format!("{}: {e}", tools.root.display()))?;
    std::fs::write(tools.helper(), HELPER).map_err(|e| e.to_string())?;
    let uv = tools.own_uv();
    if !uv.exists() {
        let archive = uv_archive(tools.os, std::env::consts::ARCH).ok_or_else(|| {
            format!(
                "uv is not published for this machine ({})",
                std::env::consts::ARCH
            )
        })?;
        say(&format!(
            "Fetching uv {UV_VERSION}, which installs WhisperX"
        ));
        let url = uv_url(&archive);
        let fetch = |url: String| async move {
            let response =
                http.get(&url).send().await.map_err(|e| {
                    format!("could not download {url} — is this machine online? ({e})")
                })?;
            if !response.status().is_success() {
                return Err(format!("could not download {url}: {}", response.status()));
            }
            response
                .bytes()
                .await
                .map_err(|e| format!("could not download {url}: {e}"))
        };
        let bytes = fetch(url.clone()).await?;
        let published = fetch(format!("{url}.sha256")).await?;
        if !checksum_matches(&bytes, &String::from_utf8_lossy(&published)) {
            return Err(format!(
                "{archive} does not match its published checksum; not used"
            ));
        }
        let unpack = tools.root.join("uv-unpack");
        let _ = std::fs::remove_dir_all(&unpack);
        std::fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;
        let saved = unpack.join(&archive);
        std::fs::write(&saved, &bytes).map_err(|e| e.to_string())?;
        // `tar` reads both .tar.gz and .zip where uv is published: it is
        // bsdtar on macOS and on Windows 10 and later.
        let status = Command::new("tar")
            .arg("-xf")
            .arg(&saved)
            .arg("-C")
            .arg(&unpack)
            .status()
            .map_err(|e| format!("tar would not start: {e}"))?;
        if !status.success() {
            return Err(format!("{archive} could not be unpacked"));
        }
        let name = uv
            .file_name()
            .map(std::ffi::OsStr::to_owned)
            .unwrap_or_default();
        let found =
            find_file(&unpack, &name).ok_or_else(|| format!("{archive} has no uv in it"))?;
        std::fs::create_dir_all(uv.parent().unwrap_or(&tools.root)).map_err(|e| e.to_string())?;
        std::fs::rename(&found, &uv)
            .or_else(|_| std::fs::copy(&found, &uv).map(|_| ()))
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&uv, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&unpack);
    }
    let words = [
        "Getting Python and making a private environment for WhisperX",
        "Installing WhisperX and PyTorch's CPU build (about two gigabytes, once)",
    ];
    for (step, what) in install_steps(&uv, tools).into_iter().zip(words) {
        say(what);
        let output = Command::new(&step[0])
            .args(&step[1..])
            .output()
            .map_err(|e| format!("{} would not start: {e}", step[0]))?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stderr);
            let last = said
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            return Err(format!("{what} failed: {last}"));
        }
    }
    std::fs::write(tools.marker(), WHISPERX).map_err(|e| e.to_string())?;
    Ok(())
}

/// A working folder for one run, gone when the run is.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("djmanzo-words-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn find_file(dir: &Path, name: &std::ffi::OsStr) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name() == Some(name) {
            return Some(path);
        }
    }
    None
}

/// Time the words of the record at `path`: decode it, hand it over with the
/// words already known, and time every stage. On a worker thread.
///
/// # Errors
/// WhisperX not installed, a record that would not decode, or what [`run`]
/// says.
pub fn time_words(
    tools: &Tools,
    path: &Path,
    known: impl FnOnce(f64) -> Vec<Known>,
    language: Option<String>,
    stems: Option<&dj_decode::StemBuffer>,
) -> Result<(Answer, Report), String> {
    if !tools.installed() {
        return Err("WhisperX is not installed yet".to_owned());
    }
    let began = Instant::now();
    let decoded = dj_decode::decode_file(path).map_err(|e| e.to_string())?;
    let buffer = decoded.buffer;
    let work = Scratch::new()?;
    let audio = work.0.join("record.wav");
    let vocals = stems.and_then(|stems| vocals_of(&stems.load(), buffer.len_frames()));
    let heard = if vocals.is_some() { "vocals" } else { "mix" };
    let record_seconds = write_audio(
        vocals.as_deref().unwrap_or_else(|| buffer.as_interleaved()),
        dj_decode::CHANNELS,
        buffer.sample_rate().get(),
        &audio,
    )?;
    let lines = known(record_seconds);
    let prepared = began.elapsed().as_secs_f64();
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let job = Job {
        audio,
        language: language.filter(|code| !code.trim().is_empty()),
        lines,
        model: MODEL.to_owned(),
        threads,
        aligners: aligners(),
    };
    let answer = run(&tools.python(), &tools.helper(), &job, TIMEOUT)?;
    let seconds = began.elapsed().as_secs_f64();
    let mut stages = vec![("prepare".to_owned(), prepared)];
    stages.extend(answer.stages.iter().cloned());
    let report = Report {
        mode: answer.mode.clone(),
        language: answer.language.clone(),
        words: timed_words(&answer),
        stages,
        seconds,
        record_seconds,
        within_budget: seconds <= BUDGET_SECONDS,
        heard: heard.to_owned(),
    };
    Ok((answer, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(segments: Vec<Segment>) -> Answer {
        Answer {
            mode: "align".into(),
            language: "es".into(),
            segments,
            stages: vec![("align".into(), 3.2)],
        }
    }

    fn word(word: &str, start: Option<f64>, end: Option<f64>) -> TimedWord {
        TimedWord {
            word: word.into(),
            start,
            end,
            score: Some(0.9),
        }
    }

    /// **The load-bearing one.** What WhisperX answers becomes LRC the
    /// singers' screen reads back word by word, at the times WhisperX gave —
    /// through `dj_library::lrc::parse`, the reader the screen really uses,
    /// not a second one written for the test.
    #[test]
    fn whisperx_words_come_back_through_the_screens_own_reader() {
        let answer = answer(vec![
            Segment {
                start: Some(12.0),
                end: Some(14.0),
                text: "Bachata en Fukuoka".into(),
                words: vec![
                    word("Bachata", Some(12.04), Some(12.6)),
                    word("en", Some(12.7), Some(12.9)),
                    word("Fukuoka", Some(13.1), Some(13.9)),
                ],
            },
            Segment {
                start: Some(75.5),
                end: Some(77.0),
                text: "tengo 2 amores".into(),
                // "2" is not a sound the aligner can place.
                words: vec![
                    word("tengo", Some(75.5), Some(75.9)),
                    word("2", None, None),
                    word("amores", Some(76.3), Some(76.9)),
                ],
            },
        ]);
        let lrc = to_lrc(&answer);
        let lines = dj_library::lrc::parse(&lrc);
        assert_eq!(lines.len(), 2, "{lrc}");
        assert!((lines[0].at - 12.04).abs() < 0.006);
        assert_eq!(lines[0].text, "Bachata en Fukuoka");
        let times: Vec<f64> = lines[0].words.iter().map(|w| w.at).collect();
        assert_eq!(times.len(), 3);
        for (got, want) in times.iter().zip([12.04, 12.7, 13.1]) {
            assert!((got - want).abs() < 0.006, "{got} for {want}");
        }
        assert_eq!(lines[1].words[1].text, "2");
        assert!(
            (lines[1].words[1].at - 75.9).abs() < 0.006,
            "an unplaced word goes with the end of the one before it"
        );
        assert!((lines[1].at - 75.5).abs() < 0.006);
        assert_eq!(timed_words(&answer), 5);
        assert_eq!(plain_of(&answer), "Bachata en Fukuoka\ntengo 2 amores");
    }

    #[test]
    fn a_line_with_no_word_placed_is_left_out_not_guessed() {
        let answer = answer(vec![Segment {
            start: None,
            end: None,
            text: "123".into(),
            words: vec![word("123", None, None)],
        }]);
        assert!(dj_library::lrc::parse(&to_lrc(&answer)).is_empty());
    }

    /// The languages whose WhisperX default is non-commercial are each given
    /// another aligner, and none of the replacements is a VoxPopuli model.
    #[test]
    fn no_non_commercial_aligner_is_chosen() {
        let chosen = aligners();
        for language in ["fr", "de", "es", "it"] {
            let model = chosen
                .get(language)
                .unwrap_or_else(|| panic!("{language} has no aligner"));
            assert!(
                !model.to_lowercase().contains("voxpopuli"),
                "{language}: {model}"
            );
            assert!(
                model.contains('/'),
                "{language}: {model} is a Hugging Face model"
            );
        }
    }

    #[test]
    fn a_time_is_written_as_lrc_writes_it() {
        assert_eq!(stamp(0.0), "00:00.00");
        assert_eq!(stamp(62.5), "01:02.50");
        assert_eq!(stamp(599.999), "10:00.00");
        assert_eq!(stamp(-3.0), "00:00.00");
    }

    /// Timed lines run to the next; plain words are one stretch; nothing is
    /// nothing, and then the record is transcribed.
    #[test]
    fn the_words_already_known_are_handed_over_with_their_stretches() {
        let synced = "[ar:Juan Luis Guerra]\n[00:12.00]Bachata en Fukuoka\n[00:20.00]\n[00:30.50]tengo dos amores\n";
        let known = known_lines(Some(synced), "", 240.0);
        assert_eq!(
            known,
            vec![
                Known {
                    start: 12.0,
                    end: 30.5,
                    text: "Bachata en Fukuoka".into()
                },
                Known {
                    start: 30.5,
                    end: 240.0,
                    text: "tengo dos amores".into()
                },
            ]
        );
        let plain = known_lines(None, "Bachata en Fukuoka\n\n  tengo dos amores  \n", 240.0);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].text, "Bachata en Fukuoka tengo dos amores");
        assert_eq!((plain[0].start, plain[0].end), (0.0, 240.0));
        assert!(known_lines(None, " \n", 240.0).is_empty());
    }

    #[test]
    fn the_install_is_a_private_environment_with_the_cpu_build() {
        let tools = Tools {
            root: PathBuf::from("/tools"),
            os: Os::Linux,
        };
        let steps = install_steps(Path::new("/tools/uv/uv"), &tools);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0][..4], ["/tools/uv/uv", "venv", "--python", PYTHON]);
        assert_eq!(steps[0].last().map(String::as_str), Some("/tools/whisperx"));
        assert!(steps[1].windows(2).any(|w| w == ["--torch-backend", "cpu"]));
        assert!(
            steps[1]
                .windows(2)
                .any(|w| w == ["--python", "/tools/whisperx/bin/python"])
        );
        assert_eq!(steps[1].last().map(String::as_str), Some(WHISPERX));
        let windows = Tools {
            root: PathBuf::from("C:/tools"),
            os: Os::Windows,
        };
        assert!(windows.python().ends_with("Scripts/python.exe"));
        assert!(windows.own_uv().ends_with("uv/uv.exe"));
    }

    #[test]
    fn uv_is_fetched_for_every_platform_djmanzo_ships_and_checked() {
        assert_eq!(
            uv_archive(Os::Linux, "x86_64").as_deref(),
            Some("uv-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert_eq!(
            uv_archive(Os::MacOs, "aarch64").as_deref(),
            Some("uv-aarch64-apple-darwin.tar.gz")
        );
        assert_eq!(
            uv_archive(Os::Windows, "x86_64").as_deref(),
            Some("uv-x86_64-pc-windows-msvc.zip")
        );
        assert_eq!(uv_archive(Os::Linux, "riscv64"), None);
        assert!(uv_url("uv-x.tar.gz").contains(&format!("/download/{UV_VERSION}/uv-x.tar.gz")));
        // SHA-256 of "abc", as published: hex, two spaces, the name.
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  uv.tar.gz";
        assert!(checksum_matches(b"abc", abc));
        assert!(checksum_matches(b"abc", &abc.to_uppercase()));
        assert!(!checksum_matches(b"abd", abc));
        assert!(!checksum_matches(b"abc", ""));
    }

    #[test]
    fn a_half_finished_install_is_not_an_install() {
        let dir = tempfile::tempdir().expect("a folder");
        let tools = Tools {
            root: dir.path().to_path_buf(),
            os: Os::here(),
        };
        assert!(!tools.installed());
        std::fs::create_dir_all(tools.python().parent().expect("a parent")).expect("made");
        std::fs::write(tools.python(), b"").expect("written");
        assert!(!tools.installed(), "no marker yet");
        std::fs::write(tools.marker(), "whisperx==0.0.1").expect("written");
        assert!(!tools.installed(), "another version's marker");
        std::fs::write(tools.marker(), WHISPERX).expect("written");
        assert!(tools.installed());
    }

    /// The vocals are handed over only once all of them are separated, and
    /// they are the vocals — the first stem — not another.
    #[test]
    fn the_vocals_are_used_only_when_the_whole_record_is_separated() {
        use dj_decode::{StemFrame, StemTable};
        let frame = |i: usize| -> StemFrame {
            let mut f = [0.0; dj_decode::STEM_COUNT * dj_decode::CHANNELS];
            for (slot, value) in f.iter_mut().enumerate() {
                // Vocals left/right carry the frame number; every other stem
                // carries a mark that must not appear.
                *value = if slot < 2 {
                    i as f32 + slot as f32 * 0.5
                } else {
                    -9.0
                };
            }
            f
        };
        let chunk = |from: usize| -> dj_decode::StemChunk { (from..from + 4).map(frame).collect() };
        let table = StemTable::new(4)
            .with_chunk(0, chunk(0))
            .expect("first chunk");
        assert_eq!(
            vocals_of(&table, 8),
            None,
            "half separated is not separated"
        );
        let table = table.with_chunk(1, chunk(4)).expect("second chunk");
        let vocals = vocals_of(&table, 8).expect("all separated");
        assert_eq!(vocals.len(), 16);
        assert_eq!(&vocals[..4], &[0.0, 0.5, 1.0, 1.5]);
        assert_eq!(&vocals[14..], &[7.0, 7.5]);
        assert_eq!(vocals_of(&table, 0), None);
    }

    /// The resampler keeps what speech is made of and drops what would fold
    /// back into it: a 1 kHz tone comes through at its level and frequency
    /// from both common rates; a 12 kHz one, above the new Nyquist, does not.
    #[test]
    fn the_resampler_keeps_speech_and_drops_what_would_alias() {
        for from in [44_100u32, 48_000] {
            let tone = |hz: f64| -> Vec<f32> {
                (0..from as usize)
                    .map(|i| {
                        (2.0 * std::f64::consts::PI * hz * i as f64 / f64::from(from)).sin() as f32
                            * 0.5
                    })
                    .collect()
            };
            let out = to_sixteen_kilohertz(&tone(1_000.0), from);
            assert_eq!(out.len(), 16_000, "one second at {from}");
            let middle = &out[2_000..14_000];
            let peak = middle.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                (peak - 0.5).abs() < 0.02,
                "{from}: 1 kHz came through at {peak}"
            );
            let crossings = middle
                .windows(2)
                .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
                .count();
            assert!(
                (749..=751).contains(&crossings),
                "{from}: {crossings} cycles in 0.75 s"
            );
            // Sample by sample, at the right instants: the kernel is centred
            // on each output time, so nothing is delayed or smeared.
            let worst = (2_000..14_000)
                .map(|n| {
                    let exact =
                        (2.0 * std::f64::consts::PI * 1_000.0 * n as f64 / 16_000.0).sin() * 0.5;
                    (f64::from(out[n]) - exact).abs()
                })
                .fold(0.0, f64::max);
            assert!(worst < 0.01, "{from}: {worst} from the exact tone");
            let high = to_sixteen_kilohertz(&tone(12_000.0), from);
            let leak = high[2_000..14_000]
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(leak < 0.01, "{from}: 12 kHz folded back at {leak}");
        }
        assert_eq!(
            to_sixteen_kilohertz(&[0.1, 0.2], SAMPLE_RATE),
            vec![0.1, 0.2]
        );
    }

    /// Four minutes are brought down in well under the owner's budget. The
    /// bound is loose on purpose, for a busy machine: the resampler this
    /// replaced took sixteen seconds here.
    #[test]
    fn four_minutes_are_resampled_quickly() {
        let mono: Vec<f32> = (0..48_000 * 240).map(|i| (i as f32 * 0.01).sin()).collect();
        let began = Instant::now();
        let out = to_sixteen_kilohertz(&mono, 48_000);
        let took = began.elapsed();
        assert_eq!(out.len(), 16_000 * 240);
        assert!(took < Duration::from_secs(4), "{took:?}");
    }

    /// The audio handed over: mono, sixteen kilohertz, as long as the record.
    #[test]
    fn the_record_is_handed_over_mono_at_sixteen_kilohertz() {
        let dir = tempfile::tempdir().expect("a folder");
        let path = dir.path().join("in.wav");
        let stereo: Vec<f32> = (0..48_000 * 2)
            .map(|i| if i % 2 == 0 { 0.5 } else { 0.1 })
            .collect();
        let seconds = write_audio(&stereo, 2, 48_000, &path).expect("written");
        assert!((seconds - 1.0).abs() < 1e-9);
        let bytes = std::fs::read(&path).expect("read");
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "mono");
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            SAMPLE_RATE
        );
        assert_eq!(
            bytes.len(),
            44 + 16_000 * 2,
            "a second at sixteen kilohertz"
        );
        // The middle of it is the two channels' mean.
        let at = 44 + 8_000 * 2;
        let middle = i16::from_le_bytes([bytes[at], bytes[at + 1]]);
        assert!((f32::from(middle) / f32::from(i16::MAX) - 0.3).abs() < 0.01);
    }

    /// The protocol, with a stand-in for WhisperX: the job goes in on standard
    /// input, the answer comes out on standard output, and a failure says
    /// what the helper last said.
    #[cfg(unix)]
    #[test]
    fn the_helper_is_asked_on_its_input_and_answers_on_its_output() {
        let dir = tempfile::tempdir().expect("a folder");
        let helper = dir.path().join("helper.py");
        // The stand-in echoes the language it was asked about, so the test
        // knows the job crossed.
        std::fs::write(
            &helper,
            r#"import json, sys
job = json.load(sys.stdin)
if job["language"] == "xx":
    print("no aligner for xx", file=sys.stderr)
    sys.exit(3)
print("loading", file=sys.stderr)
print(json.dumps({"mode": "align", "language": job["aligners"][job["language"]], "segments": [{"start": 1.0, "end": 2.0, "text": job["lines"][0]["text"], "words": [{"word": "hola", "start": 1.0, "end": 1.5}]}], "stages": [["align", 0.1]]}))
"#,
        )
        .expect("written");
        let Ok(python) = which("python3") else {
            eprintln!("no python3 here; skipped");
            return;
        };
        let job = Job {
            audio: dir.path().join("in.wav"),
            language: Some("es".into()),
            lines: vec![Known {
                start: 1.0,
                end: 2.0,
                text: "hola".into(),
            }],
            model: MODEL.into(),
            threads: 2,
            aligners: aligners(),
        };
        // The real helper is at least Python that compiles.
        let real = dir.path().join("wordtimes_helper.py");
        std::fs::write(&real, HELPER).expect("written");
        let compiled = Command::new(&python)
            .args(["-m", "py_compile"])
            .arg(&real)
            .status()
            .expect("python ran");
        assert!(compiled.success(), "the helper does not compile");

        let answer = run(&python, &helper, &job, Duration::from_secs(20)).expect("answered");
        assert_eq!(
            answer.language, "facebook/wav2vec2-large-xlsr-53-spanish",
            "the aligners crossed with the job"
        );
        assert_eq!(answer.segments[0].text, "hola");
        assert_eq!(answer.stages, vec![("align".to_owned(), 0.1)]);

        let refused = run(
            &python,
            &helper,
            &Job {
                language: Some("xx".into()),
                ..job
            },
            Duration::from_secs(20),
        )
        .expect_err("refused");
        assert!(refused.contains("no aligner for xx"), "{refused}");
    }

    /// **The whole run, WhisperX stood in for.** A record is decoded and
    /// handed over at sixteen kilohertz mono with the words already known;
    /// what comes back is timed against the budget, stage by stage. The
    /// stand-in checks the audio it was given, so a record handed over at the
    /// wrong rate, or without its words, fails here.
    #[cfg(unix)]
    #[test]
    fn a_record_is_timed_through_an_installed_environment() {
        let Ok(python3) = which("python3") else {
            eprintln!("no python3 here; skipped");
            return;
        };
        let dir = tempfile::tempdir().expect("a folder");
        let tools = Tools {
            root: dir.path().join("tools"),
            os: Os::Linux,
        };
        let bin = tools.python();
        std::fs::create_dir_all(bin.parent().expect("a parent")).expect("made");
        std::fs::write(
            &bin,
            format!("#!/bin/sh\nexec {} \"$@\"\n", python3.display()),
        )
        .expect("written");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
                .expect("made runnable");
        }
        std::fs::write(
            tools.helper(),
            r#"import array, json, sys, wave
job = json.load(sys.stdin)
with wave.open(job["audio"]) as w:
    assert (w.getframerate(), w.getnchannels()) == (16000, 1), "not 16 kHz mono"
    seconds = w.getnframes() / 16000
    samples = array.array("h", w.readframes(w.getnframes()))
peak = max(abs(s) for s in samples) / 32767
assert job["lines"], "the known words were not handed over"
line = job["lines"][0]
words = [{"word": w, "start": line["start"] + i * 0.3, "end": line["start"] + i * 0.3 + 0.2} for i, w in enumerate(line["text"].split())]
print(json.dumps({"mode": "align", "language": job["language"], "segments": [{"start": line["start"], "end": line["end"], "text": line["text"], "words": words}], "stages": [["start", 0.01], ["align", round(seconds, 2)], ["peak", round(peak, 2)]]}))
"#,
        )
        .expect("written");
        std::fs::write(tools.marker(), WHISPERX).expect("written");

        // A two-second stereo record at 44.1 kHz, as a file to decode.
        let record = dir.path().join("record.wav");
        let stereo: Vec<f32> = (0..44_100 * 2 * 2)
            .map(|i| ((i / 2) as f32 * 0.01).sin() * 0.4)
            .collect();
        write_stereo(&stereo, 44_100, &record);

        let (answer, report) = time_words(
            &tools,
            &record,
            |seconds| known_lines(Some("[00:00.50]hola que tal"), "", seconds),
            Some("es".into()),
            None,
        )
        .expect("timed");
        assert_eq!(answer.segments[0].words.len(), 3);
        assert_eq!(report.words, 3);
        assert_eq!(report.language, "es");
        assert!(
            (report.record_seconds - 2.0).abs() < 0.01,
            "{}",
            report.record_seconds
        );
        assert_eq!(report.stages[0].0, "prepare");
        assert_eq!(
            report.stages[2],
            ("align".to_owned(), 2.0),
            "the helper was handed the whole record"
        );
        assert!(report.within_budget);
        assert!(report.seconds < BUDGET_SECONDS);
        assert_eq!(report.heard, "mix", "nothing was separated");
        assert!(
            report.stages[3].1 > 0.3,
            "the mix is loud: {:?}",
            report.stages
        );

        // Separated in full, silent vocals under a loud mix: what crosses is
        // the vocals.
        let deck = dj_decode::AudioBuffer::from_interleaved(
            vec![0.0; 44_100 * 2 * 2],
            dj_core::SampleRate::new(44_100).expect("a rate"),
        );
        let mut quiet = [0.3; dj_decode::STEM_COUNT * dj_decode::CHANNELS];
        quiet[0] = 0.0;
        quiet[1] = 0.0;
        let half: dj_decode::StemChunk = vec![quiet; 44_100].into();
        let table = dj_decode::StemTable::new(44_100)
            .with_chunk(0, half.clone())
            .and_then(|t| t.with_chunk(1, half))
            .expect("two chunks");
        let stems = deck.stems_lock();
        stems.store(std::sync::Arc::new(table));
        let (_, report) = time_words(
            &tools,
            &record,
            |seconds| known_lines(None, "hola que tal", seconds),
            Some("es".into()),
            Some(&stems),
        )
        .expect("timed");
        assert_eq!(report.heard, "vocals");
        assert!(
            report.stages[3].1 < 0.01,
            "the vocals were handed over, not the mix: {:?}",
            report.stages
        );

        // Not installed: said, not attempted.
        std::fs::remove_file(tools.marker()).expect("removed");
        let refused = time_words(&tools, &record, |_| Vec::new(), None, None).expect_err("refused");
        assert!(refused.contains("not installed"), "{refused}");
    }

    /// A 16-bit stereo WAV, for a record to decode.
    #[cfg(unix)]
    fn write_stereo(interleaved: &[f32], rate: u32, to: &Path) {
        let data = (interleaved.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        for sample in interleaved {
            bytes.extend_from_slice(&((sample * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(to, bytes).expect("written");
    }

    #[cfg(unix)]
    fn which(name: &str) -> Result<PathBuf, ()> {
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .find(|candidate| candidate.is_file())
            })
            .ok_or(())
    }
}

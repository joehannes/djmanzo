//! §122: when each word is sung, found by whisper.cpp — inside djmanzo.
//!
//! > i want a max download of < 100 MB for all of install x-whisper, also i
//! > want that to get bundled with the installer ...
//! >
//! > i'd like whisper.cpp + a better model than base only, one that doesn't
//! > exceed 1.5 min of analysis on this laptop approx. + the English aligner
//! > as well please. if the model download is huge, i want the thing to be
//! > downloadable from the installer (and if not ... downloadable and
//! > auto-installable from within the app/GUI on demand)
//!
//! WhisperX needed Python and PyTorch — 2.6 GB — so it could never ship with
//! djmanzo. whisper.cpp (MIT) runs the same Whisper models from C++, and is
//! compiled into djmanzo itself through `whisper-rs` (Unlicense): about two
//! megabytes of program and nothing to install. What it needs besides is a
//! model, one file, downloaded once on the DJ's choice from [`MODELS`] and
//! checked against the SHA-256 its host publishes.
//!
//! # Which model
//!
//! Measured on one machine — four cores of a 2.1 GHz Xeon, a 4½-minute
//! record, the whole mix — and kept as [`Model::seconds_per_minute`]:
//! `small` took 73 s, inside the owner's minute and a half, with most words
//! right, so it is the one recommended. `tiny` and `base` are quicker and
//! mishear more; `medium` and `large-v3-turbo` hear best and take minutes. A
//! DJ's own machine is not that one, so each run's own time is kept and used
//! for the next estimate ([`Rates`]).
//!
//! # Word times
//!
//! Whisper's own attention says where each token was heard (whisper.cpp's
//! token-level timestamps, by dynamic time warping). Against WhisperX's
//! aligner on the same record that time sits near the **end** of a word, a
//! median 0.27 s after its start for `small`, so a word is started [`LEAD`]
//! earlier. What remains is about a quarter of a second either way — close
//! enough to wipe a line by, not to the syllable; the English aligner
//! refines English.
//!
//! # Words already known
//!
//! A record's own lyrics, a sidecar `.lrc` or LRCLIB's are what the singers
//! read — not what Whisper happened to hear. They are handed to Whisper as
//! its prompt, which steers its spelling, and afterwards matched word by
//! word onto what it heard ([`place_known`]); a known word it did not hear
//! is spread between the neighbours it did.
//!
//! # The processor
//!
//! whisper.cpp is compiled for a baseline every x86-64 processor sold since
//! 2013 has — AVX2, FMA, F16C (`.cargo/config.toml`) — and not for the
//! machine that built the package, which would crash on any processor
//! lacking the builder's instructions. One without them is told so
//! ([`cpu_can_run`]) rather than crashed.

use crate::wordtimes::{Known, Segment, TimedWord};
use serde::Serialize;
use sha2::Digest;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One Whisper model djmanzo offers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    /// Its file, as whisper.cpp's model repository names it.
    pub file: &'static str,
    pub bytes: u64,
    /// As the repository publishes it.
    pub sha256: &'static str,
    /// Seconds of work for each minute of record, measured on the machine
    /// named in the module's documentation.
    pub seconds_per_minute: f64,
    /// Whether that figure was measured, or only estimated from its
    /// neighbours.
    pub measured: bool,
    /// How often its words come out right, in a DJ's words.
    pub reliability: &'static str,
    pub recommended: bool,
}

/// whisper.cpp's model repository, at one fixed commit, so a file can never
/// change under its checksum.
pub const WHISPER_MODELS: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1";

/// The models, lightest first. Quantised to five bits, which whisper.cpp
/// reads directly: a third of the full files' size, and on a CPU no slower.
pub const MODELS: [Model; 5] = [
    Model {
        id: "tiny",
        name: "Tiny",
        file: "ggml-tiny-q5_1.bin",
        bytes: 32_152_673,
        sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
        seconds_per_minute: 2.7,
        measured: true,
        reliability: "a quick first look: mishears often",
        recommended: false,
    },
    Model {
        id: "base",
        name: "Base",
        file: "ggml-base-q5_1.bin",
        bytes: 59_707_625,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        seconds_per_minute: 5.7,
        measured: true,
        reliability: "most words on a clear vocal; stumbles over a busy mix",
        recommended: false,
    },
    Model {
        id: "small",
        name: "Small",
        file: "ggml-small-q5_1.bin",
        bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        seconds_per_minute: 16.2,
        measured: true,
        reliability: "most words right, even over the mix — works for most songs",
        recommended: true,
    },
    Model {
        id: "medium",
        name: "Medium",
        file: "ggml-medium-q5_0.bin",
        bytes: 539_212_467,
        sha256: "19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f",
        seconds_per_minute: 48.0,
        measured: false,
        reliability: "hears hard vocals and other languages better; slow",
        recommended: false,
    },
    Model {
        id: "large-v3-turbo",
        name: "Large (turbo)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        bytes: 574_041_195,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        seconds_per_minute: 78.3,
        measured: true,
        reliability: "hears best, but invents lines over long instrumentals in the mix; slowest",
        recommended: false,
    },
];

/// The model called `id`.
#[must_use]
pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|model| model.id == id)
}

/// The one recommended.
#[must_use]
pub fn recommended() -> &'static Model {
    MODELS
        .iter()
        .find(|model| model.recommended)
        .unwrap_or(&MODELS[0])
}

/// Where `model` is published.
#[must_use]
pub fn url(model: &Model) -> String {
    format!("{WHISPER_MODELS}/{}", model.file)
}

/// How much earlier than Whisper's own token time a word starts. See the
/// module's documentation.
pub const LEAD: f64 = 0.25;

/// Where the models live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Models {
    pub root: PathBuf,
}

impl Models {
    #[must_use]
    pub fn path(&self, model: &Model) -> PathBuf {
        self.root.join(model.file)
    }

    /// Whether `model` is downloaded whole. Its checksum was checked when it
    /// arrived; its size is checked here, which is cheap and catches a file
    /// cut short by hand.
    #[must_use]
    pub fn installed(&self, model: &Model) -> bool {
        std::fs::metadata(self.path(model)).is_ok_and(|meta| meta.len() == model.bytes)
    }

    fn chosen_file(&self) -> PathBuf {
        self.root.join("chosen")
    }

    /// The model the DJ chose, if it is still here; else the most capable
    /// one that is — the recommended one first.
    #[must_use]
    pub fn chosen(&self) -> Option<&'static Model> {
        let named = std::fs::read_to_string(self.chosen_file())
            .ok()
            .and_then(|id| model(id.trim()));
        named
            .filter(|model| self.installed(model))
            .or_else(|| Some(recommended()).filter(|model| self.installed(model)))
            .or_else(|| MODELS.iter().rev().find(|model| self.installed(model)))
    }

    /// Remember `model` as the one to run.
    ///
    /// # Errors
    /// A model not downloaded, or the file system's own sentence.
    pub fn choose(&self, model: &Model) -> Result<(), String> {
        if !self.installed(model) {
            return Err(format!("{} is not downloaded yet", model.name));
        }
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        std::fs::write(self.chosen_file(), model.id).map_err(|e| e.to_string())
    }

    /// Delete `model`, to give its space back.
    ///
    /// # Errors
    /// The file system's own sentence.
    pub fn remove(&self, model: &Model) -> Result<(), String> {
        match std::fs::remove_file(self.path(model)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn rates_file(&self) -> PathBuf {
        self.root.join("rates.json")
    }

    /// What runs on this machine took, model by model.
    #[must_use]
    pub fn rates(&self) -> Rates {
        std::fs::read_to_string(self.rates_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Keep what a run of `model` took, for the next estimate.
    pub fn remember_rate(&self, model: &Model, seconds: f64, record_seconds: f64) {
        if record_seconds < 10.0 || seconds <= 0.0 {
            return;
        }
        let mut rates = self.rates();
        rates
            .0
            .insert(model.id.to_owned(), seconds / (record_seconds / 60.0));
        if let Ok(text) = serde_json::to_string(&rates) {
            let _ = std::fs::create_dir_all(&self.root);
            let _ = std::fs::write(self.rates_file(), text);
        }
    }
}

/// Seconds of work per minute of record, as measured on this machine, by
/// model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct Rates(pub std::collections::BTreeMap<String, f64>);

/// How long `model` would take over `record_seconds` on this machine, and
/// whether that was measured here.
///
/// A model run here before is its own measure. Otherwise the reference
/// figure is scaled by how this machine compared on any model it has run —
/// the models' costs keep their proportions from one processor to the next
/// far better than their absolute times do — and with nothing run yet, by
/// how many cores it has against the reference's four.
#[must_use]
pub fn estimate(model: &Model, record_seconds: f64, rates: &Rates, cores: usize) -> (f64, bool) {
    let minutes = record_seconds / 60.0;
    if let Some(rate) = rates.0.get(model.id) {
        return (rate * minutes, true);
    }
    let factor = rates
        .0
        .iter()
        .find_map(|(id, rate)| self::model(id).map(|other| rate / other.seconds_per_minute))
        .unwrap_or_else(|| (4.0 / cores.max(1) as f64).clamp(0.5, 2.0));
    (model.seconds_per_minute * factor * minutes, false)
}

/// Whether this processor can run the whisper.cpp djmanzo was built with.
///
/// # Errors
/// An x86-64 processor without AVX2, FMA and F16C, which the build assumes.
pub fn cpu_can_run() -> Result<(), String> {
    #[cfg(target_arch = "x86_64")]
    {
        if !(std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
            && std::arch::is_x86_feature_detected!("f16c"))
        {
            return Err(
                "this processor lacks AVX2, which djmanzo's word timing needs (every Intel and AMD processor since about 2013 has it)"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// Download `model` into `models`, reporting bytes as they arrive, and check
/// it against its published SHA-256 before it is kept. Written beside its
/// place and moved in whole, so a download cut short is never taken for a
/// model.
///
/// # Errors
/// A download that failed, came out the wrong size, or did not match.
pub async fn download(
    http: &reqwest::Client,
    model: &Model,
    models: &Models,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    let from = url(model);
    std::fs::create_dir_all(&models.root).map_err(|e| format!("{}: {e}", models.root.display()))?;
    let part = models.root.join(format!("{}.part", model.file));
    let result = async {
        let mut response =
            http.get(&from).send().await.map_err(|e| {
                format!("could not download {from} — is this machine online? ({e})")
            })?;
        if !response.status().is_success() {
            return Err(format!("could not download {from}: {}", response.status()));
        }
        let mut file =
            std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut hasher = sha2::Sha256::new();
        let mut done = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("the download of {} broke off: {e}", model.name))?
        {
            hasher.update(&chunk);
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            done += chunk.len() as u64;
            progress(done, model.bytes);
        }
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        if done != model.bytes {
            return Err(format!(
                "{} arrived at {done} bytes, not {}; not used",
                model.name, model.bytes
            ));
        }
        let hex: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if hex != model.sha256 {
            return Err(format!(
                "{} does not match its published checksum; not used",
                model.name
            ));
        }
        std::fs::rename(&part, models.path(model)).map_err(|e| e.to_string())
    }
    .await;
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// This machine's download speed in bytes a second, from the first two
/// megabytes of the smallest model — enough to say "about a minute" rather
/// than a size alone.
pub async fn download_speed(http: &reqwest::Client) -> Option<f64> {
    const PROBE: u64 = 2 * 1024 * 1024;
    let started = std::time::Instant::now();
    let mut response = http
        .get(url(&MODELS[0]))
        .header(reqwest::header::RANGE, format!("bytes=0-{}", PROBE - 1))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut got = 0u64;
    while let Ok(Some(chunk)) = response.chunk().await {
        got += chunk.len() as u64;
        if got >= PROBE {
            break;
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    (got > 0 && seconds > 0.0).then(|| got as f64 / seconds)
}

/// One token as Whisper gave it: its text, whether it is one of Whisper's
/// own markers rather than a word, and when it was heard.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub special: bool,
    /// Where Whisper's attention placed it, in seconds.
    pub at: f64,
}

/// Whether `word` is Whisper saying what it heard rather than a sung word:
/// `[Music]`, `(applause)`, `♪`.
fn is_annotation(word: &str) -> bool {
    let word = word.trim();
    (word.starts_with('[') && word.ends_with(']'))
        || (word.starts_with('(') && word.ends_with(')'))
        || !word.chars().any(char::is_alphanumeric)
}

/// The words of one of Whisper's segments, from its tokens.
///
/// A token starting with a space starts a word; the others — pieces of a
/// long word, punctuation — join the word before. Whisper's own markers and
/// its notes on what it heard (`[Music]`) are left out. Each word starts
/// [`LEAD`] before its first token's time and ends at its last's, never
/// before the word ahead of it nor before the segment.
#[must_use]
pub fn words_of(tokens: &[Token], segment_start: f64) -> Vec<TimedWord> {
    let mut pieces: Vec<(String, f64, f64)> = Vec::new();
    for token in tokens.iter().filter(|token| !token.special) {
        let starts = token.text.starts_with(' ') || pieces.is_empty();
        let joins_brackets = pieces
            .last()
            .is_some_and(|(text, _, _)| text.starts_with('[') && !text.ends_with(']'));
        if starts && !joins_brackets {
            pieces.push((token.text.trim().to_owned(), token.at, token.at));
        } else if let Some(last) = pieces.last_mut() {
            last.0.push_str(token.text.trim_start());
            last.2 = token.at;
        }
    }
    let mut words: Vec<TimedWord> = Vec::new();
    let mut floor = segment_start;
    for (text, first, last) in pieces {
        if text.is_empty() || is_annotation(&text) {
            continue;
        }
        let start = (first - LEAD).max(floor);
        let end = last.max(start + 0.05);
        floor = start + 0.01;
        words.push(TimedWord {
            word: text,
            start: Some(start),
            end: Some(end),
            score: None,
        });
    }
    words
}

/// Whisper's loops left out: over a long instrumental it can fall into
/// saying the same short line again and again. A line said a third time
/// among the last six kept is one of those, and dropped; a chorus sung
/// twice is kept.
#[must_use]
pub fn drop_loops(segments: Vec<Segment>) -> Vec<Segment> {
    let mut kept: Vec<Segment> = Vec::new();
    for segment in segments {
        let said = normal(&segment.text);
        if said.is_empty() {
            continue;
        }
        let again = kept
            .iter()
            .rev()
            .take(6)
            .filter(|earlier| normal(&earlier.text) == said)
            .count();
        if again < 2 {
            kept.push(segment);
        }
    }
    kept
}

/// A word or line as compared: lower case, letters and digits only.
fn normal(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How alike two words are, for matching a known word to a heard one: 3 the
/// same, 1 nearly (one letter in three different, or one the start of the
/// other), 0 not at all.
fn alike(known: &str, heard: &str) -> u32 {
    if known.is_empty() || heard.is_empty() {
        return 0;
    }
    if known == heard {
        return 3;
    }
    let (a, b): (Vec<char>, Vec<char>) = (known.chars().collect(), heard.chars().collect());
    let shorter = a.len().min(b.len());
    if shorter >= 3 && (a.starts_with(&b) || b.starts_with(&a)) {
        return 1;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = if ca == cb {
                previous
            } else {
                1 + previous.min(row[j]).min(row[j + 1])
            };
            previous = here;
        }
    }
    let distance = row[b.len()];
    u32::from(distance * 3 <= a.len().max(b.len()))
}

/// The known lines, each word placed at the time Whisper heard it.
///
/// The known words are matched in order onto the heard ones — the most
/// alike pairing that keeps both in order — and where the lines carry their
/// own times, a word is only matched to one heard within three seconds of
/// its line, so a chorus is not placed at another chorus. A known word left
/// unmatched is spread, by its length, between the matched words on either
/// side; a timed line with nothing matched at all keeps its own time. The
/// text is always the known one.
#[must_use]
pub fn place_known(known: &[Known], heard: &[Segment]) -> Vec<Segment> {
    const NEAR_LINE: f64 = 3.0;
    let heard: Vec<(String, f64, f64)> = heard
        .iter()
        .flat_map(|segment| &segment.words)
        .filter_map(|word| Some((normal(&word.word), word.start?, word.end?)))
        .filter(|(text, _, _)| !text.is_empty())
        .collect();
    // Every known word: its line, its text as written, its compared form.
    let words: Vec<(usize, &str, String)> = known
        .iter()
        .enumerate()
        .flat_map(|(line, known)| {
            known
                .text
                .split_whitespace()
                .map(move |word| (line, word, normal(word)))
        })
        .collect();
    let (n, m) = (words.len(), heard.len());
    // The most alike ordered pairing (a weighted longest common subsequence).
    let mut score = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    let allowed = |i: usize, j: usize| {
        let line = &known[words[i].0];
        !line.timed || (heard[j].1 >= line.start - NEAR_LINE && heard[j].1 <= line.end + NEAR_LINE)
    };
    for i in 1..=n {
        for j in 1..=m {
            let mut best = score[at(i - 1, j)].max(score[at(i, j - 1)]);
            if allowed(i - 1, j - 1) {
                let pair = alike(&words[i - 1].2, &heard[j - 1].0);
                if pair > 0 {
                    best = best.max(score[at(i - 1, j - 1)] + pair);
                }
            }
            score[at(i, j)] = best;
        }
    }
    let mut matched: Vec<Option<(f64, f64)>> = vec![None; n];
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let pair = if allowed(i - 1, j - 1) {
            alike(&words[i - 1].2, &heard[j - 1].0)
        } else {
            0
        };
        if pair > 0 && score[at(i, j)] == score[at(i - 1, j - 1)] + pair {
            matched[i - 1] = Some((heard[j - 1].1, heard[j - 1].2));
            i -= 1;
            j -= 1;
        } else if score[at(i, j)] == score[at(i - 1, j)] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    // A timed line with no word matched keeps its own time: its words are
    // spread over it, a quarter-second a letter at most.
    for (line_index, line) in known.iter().enumerate() {
        if !line.timed {
            continue;
        }
        let in_line: Vec<usize> = (0..n).filter(|&k| words[k].0 == line_index).collect();
        if in_line.is_empty() || in_line.iter().any(|&k| matched[k].is_some()) {
            continue;
        }
        let letters: usize = in_line.iter().map(|&k| words[k].1.len().max(1)).sum();
        let span = (line.end - line.start).clamp(0.3, letters as f64 * 0.25);
        let mut t = line.start;
        for &k in &in_line {
            let length = span * words[k].1.len().max(1) as f64 / letters as f64;
            matched[k] = Some((t, t + length));
            t += length;
        }
    }
    // The rest spread between their matched neighbours.
    let mut k = 0;
    while k < n {
        if matched[k].is_some() {
            k += 1;
            continue;
        }
        let gap_start = k;
        while k < n && matched[k].is_none() {
            k += 1;
        }
        let before = gap_start
            .checked_sub(1)
            .and_then(|p| matched[p])
            .map(|(_, end)| end);
        let after = matched.get(k).copied().flatten().map(|(start, _)| start);
        let line = &known[words[gap_start].0];
        let (from, to) = match (before, after) {
            (Some(from), Some(to)) => (from, to.max(from)),
            (Some(from), None) => (from, from + 0.4 * (k - gap_start) as f64),
            (None, Some(to)) => ((to - 0.4 * (k - gap_start) as f64).max(0.0), to),
            (None, None) => (line.start, line.start + 0.4 * (k - gap_start) as f64),
        };
        let letters: usize = (gap_start..k).map(|g| words[g].1.len().max(1)).sum();
        let mut t = from;
        for g in gap_start..k {
            let length = (to - from) * words[g].1.len().max(1) as f64 / letters.max(1) as f64;
            matched[g] = Some((t, t + length));
            t += length;
        }
    }
    known
        .iter()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let placed: Vec<TimedWord> = (0..n)
                .filter(|&k| words[k].0 == line_index)
                .map(|k| TimedWord {
                    word: words[k].1.to_owned(),
                    start: matched[k].map(|(start, _)| start),
                    end: matched[k].map(|(_, end)| end),
                    score: None,
                })
                .collect();
            let start = placed.first().and_then(|word| word.start)?;
            let end = placed.last().and_then(|word| word.end);
            Some(Segment {
                start: Some(start),
                end,
                text: line.text.clone(),
                words: placed,
            })
        })
        .collect()
}

/// What Whisper heard in a record.
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    pub language: String,
    pub segments: Vec<Segment>,
    /// Loading the model, then listening, in seconds.
    pub load_seconds: f64,
    pub listen_seconds: f64,
}

/// Listen to `audio` — sixteen-kilohertz mono — with the model at `path`.
///
/// `language` a two-letter code, or `None` for Whisper to tell; `prompt`
/// the words already known, which steer its spelling. `progress` is told
/// the percentage done. On a worker thread: this takes seconds to minutes.
///
/// # Errors
/// A processor that cannot run it, a model that would not load, or a run
/// that failed.
pub fn listen(
    path: &Path,
    model: &Model,
    audio: &[f32],
    language: Option<&str>,
    prompt: Option<&str>,
    threads: usize,
    progress: impl FnMut(i32) + 'static,
) -> Result<Heard, String> {
    use whisper_rs::{
        DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
        WhisperContextParameters,
    };
    cpu_can_run()?;
    whisper_rs::install_logging_hooks();
    let began = std::time::Instant::now();
    let preset = match model.id {
        "tiny" => DtwModelPreset::Tiny,
        "base" => DtwModelPreset::Base,
        "small" => DtwModelPreset::Small,
        "medium" => DtwModelPreset::Medium,
        _ => DtwModelPreset::LargeV3Turbo,
    };
    let context = WhisperContextParameters {
        dtw_parameters: DtwParameters {
            mode: DtwMode::ModelPreset {
                model_preset: preset,
            },
            ..DtwParameters::default()
        },
        ..WhisperContextParameters::default()
    };
    let path_text = path
        .to_str()
        .ok_or_else(|| format!("{} is not a path whisper.cpp can open", path.display()))?;
    let whisper = WhisperContext::new_with_params(path_text, context)
        .map_err(|e| format!("{} would not load: {e}", model.name))?;
    let mut state = whisper
        .create_state()
        .map_err(|e| format!("{} would not start: {e}", model.name))?;
    let load_seconds = began.elapsed().as_secs_f64();

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(i32::try_from(threads.max(1)).unwrap_or(4));
    params.set_language(Some(language.unwrap_or("auto")));
    params.set_token_timestamps(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_print_special(false);
    if let Some(prompt) = prompt.filter(|prompt| !prompt.trim().is_empty()) {
        params.set_initial_prompt(prompt);
    }
    params.set_progress_callback_safe(progress);
    let began = std::time::Instant::now();
    state
        .full(params, audio)
        .map_err(|e| format!("{} could not listen: {e}", model.name))?;
    let listen_seconds = began.elapsed().as_secs_f64();

    let end_of_text = whisper.token_eot();
    let mut segments = Vec::new();
    for segment in state.as_iter() {
        let start = segment.start_timestamp() as f64 / 100.0;
        let end = segment.end_timestamp() as f64 / 100.0;
        let tokens: Vec<Token> = (0..segment.n_tokens())
            .filter_map(|index| segment.get_token(index))
            .map(|token| Token {
                text: token
                    .to_str_lossy()
                    .map(|text| text.into_owned())
                    .unwrap_or_default(),
                special: token.token_id() >= end_of_text,
                at: token.token_data().t_dtw as f64 / 100.0,
            })
            .collect();
        let words = words_of(&tokens, start);
        if words.is_empty() {
            continue;
        }
        let text = words
            .iter()
            .map(|word| word.word.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        segments.push(Segment {
            start: Some(start),
            end: Some(end),
            text,
            words,
        });
    }
    let language = language.map_or_else(
        || {
            whisper_rs::get_lang_str(state.full_lang_id_from_state())
                .unwrap_or("en")
                .to_owned()
        },
        str::to_owned,
    );
    Ok(Heard {
        language,
        segments: drop_loops(segments),
        load_seconds,
        listen_seconds,
    })
}

/// The known words as Whisper's prompt: the start of them, which is as much
/// as it reads (its prompt is at most half its context, about 220 tokens).
#[must_use]
pub fn prompt_of(known: &[Known]) -> Option<String> {
    const MOST: usize = 600;
    let mut prompt = String::new();
    for line in known {
        if prompt.len() + line.text.len() + 1 > MOST {
            break;
        }
        if !prompt.is_empty() {
            prompt.push(' ');
        }
        prompt.push_str(line.text.trim());
    }
    (!prompt.is_empty()).then_some(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, at: f64) -> Token {
        Token {
            text: text.to_owned(),
            special: false,
            at,
        }
    }

    fn word(text: &str, start: f64, end: f64) -> TimedWord {
        TimedWord {
            word: text.to_owned(),
            start: Some(start),
            end: Some(end),
            score: None,
        }
    }

    fn segment(words: Vec<TimedWord>) -> Segment {
        Segment {
            start: words.first().and_then(|w| w.start),
            end: words.last().and_then(|w| w.end),
            text: words
                .iter()
                .map(|w| w.word.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
        }
    }

    #[test]
    fn every_model_is_pinned_and_checked() {
        for model in &MODELS {
            assert_eq!(model.sha256.len(), 64, "{}", model.id);
            assert!(model.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(url(model).contains("/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/"));
            assert!(url(model).ends_with(model.file));
            assert!(model.bytes > 1_000_000);
        }
        assert_eq!(recommended().id, "small");
        assert_eq!(MODELS.iter().filter(|m| m.recommended).count(), 1);
        // Lightest first, so a list reads from quick to thorough.
        assert!(MODELS.windows(2).all(|w| w[0].bytes < w[1].bytes));
        assert!(
            MODELS
                .windows(2)
                .all(|w| w[0].seconds_per_minute < w[1].seconds_per_minute)
        );
    }

    /// **The load-bearing one for times.** Words are built from tokens —
    /// a leading space starts one, the rest join — Whisper's markers and its
    /// `[Music]` are left out, and each word starts [`LEAD`] before its
    /// first token, never before the word ahead of it.
    #[test]
    fn words_are_built_from_tokens_and_started_early() {
        let tokens = vec![
            Token {
                text: "[_BEG_]".into(),
                special: true,
                at: 22.0,
            },
            token(" When", 22.88),
            token(" I", 23.42),
            token(" wo", 24.0),
            token("ke", 24.1),
            token(" [", 24.2),
            token("Music", 24.3),
            token("]", 24.4),
            token(" morning", 25.78),
            token(",", 26.24),
        ];
        let words = words_of(&tokens, 22.0);
        let texts: Vec<&str> = words.iter().map(|w| w.word.as_str()).collect();
        assert_eq!(texts, ["When", "I", "woke", "morning,"]);
        assert!((words[0].start.unwrap() - (22.88 - LEAD)).abs() < 1e-9);
        assert!(
            (words[2].end.unwrap() - 24.1).abs() < 1e-9,
            "a word ends at its last piece"
        );
        assert!((words[3].end.unwrap() - 26.24).abs() < 1e-9);
        // Never before the segment, nor before the word ahead.
        let crowded = words_of(&[token(" a", 10.1), token(" b", 10.1)], 10.0);
        assert!(crowded[0].start.unwrap() >= 10.0);
        assert!(crowded[1].start.unwrap() > crowded[0].start.unwrap());
    }

    #[test]
    fn a_line_said_over_and_over_is_a_loop_and_a_chorus_is_not() {
        let line = |text: &str, at: f64| segment(vec![word(text, at, at + 0.8)]);
        let kept = drop_loops(vec![
            line("And the dark clouds", 86.0),
            line("Witheringly", 87.0),
            line("And the dark clouds", 88.0),
            line("Witheringly", 89.0),
            line("And the dark clouds", 90.0),
            line("Witheringly", 91.0),
            line("And the dark clouds", 92.0),
        ]);
        assert_eq!(kept.len(), 4, "each said twice, then dropped");
        let chorus = drop_loops(vec![
            line("Hold me close", 10.0),
            line("Never let go", 12.0),
            line("Hold me close", 14.0),
            line("Never let go", 16.0),
        ]);
        assert_eq!(chorus.len(), 4);
    }

    /// **The load-bearing one for known words.** The singers read the known
    /// words; each takes the time of the heard word it matches, a misheard
    /// one too, and one Whisper missed is spread between its neighbours.
    #[test]
    fn known_words_take_the_times_they_were_heard_at() {
        let heard = vec![segment(vec![
            word("When", 22.6, 22.9),
            word("I", 23.2, 23.4),
            word("woke", 23.8, 24.1),
            word("on", 24.5, 24.8),
            word("the", 24.7, 24.9),
            word("morning", 25.5, 25.8),
            word("there", 27.0, 27.3),
            word("was", 27.2, 27.4),
            word("a", 27.4, 27.6),
            word("darkness", 27.9, 28.1),
            word("in", 29.2, 29.4),
            word("your", 29.4, 29.6),
            word("skin", 29.9, 30.3),
        ])];
        let known = vec![
            Known {
                start: 0.0,
                end: 270.0,
                text: "When I woke up in the morning".into(),
                timed: false,
            },
            Known {
                start: 0.0,
                end: 270.0,
                text: "There was a dullness in your skin".into(),
                timed: false,
            },
        ];
        let placed = place_known(&known, &heard);
        assert_eq!(placed.len(), 2, "the known lines, as lines");
        assert_eq!(placed[0].text, "When I woke up in the morning");
        let at = |line: usize, w: usize| placed[line].words[w].start.unwrap();
        assert_eq!(placed[0].words[2].word, "woke");
        assert!((at(0, 2) - 23.8).abs() < 1e-9, "woke, as heard");
        // "up" was not heard: it falls between "woke" and what follows.
        assert!(at(0, 3) >= 24.1 && at(0, 3) <= 24.7, "{}", at(0, 3));
        assert!((at(0, 6) - 25.5).abs() < 1e-9, "morning, as heard");
        assert!((at(1, 0) - 27.0).abs() < 1e-9, "There, case aside");
        // "dullness" was heard as "darkness": not alike, so spread.
        assert!(at(1, 3) > 27.4 && at(1, 3) < 29.2, "{}", at(1, 3));
        assert!((at(1, 6) - 29.9).abs() < 1e-9);
        // In order, every one.
        let starts: Vec<f64> = placed
            .iter()
            .flat_map(|l| l.words.iter().map(|w| w.start.unwrap()))
            .collect();
        assert!(starts.windows(2).all(|w| w[0] <= w[1]), "{starts:?}");
    }

    /// With timed lines, a word is matched only near its own line — the
    /// second chorus is placed at the second chorus.
    #[test]
    fn a_chorus_is_placed_at_its_own_time() {
        let heard = vec![
            segment(vec![word("hold", 10.0, 10.3), word("me", 10.4, 10.6)]),
            segment(vec![word("hold", 60.0, 60.3), word("me", 60.4, 60.6)]),
        ];
        // The first chorus, which the ordered matching alone would place at
        // the second — the last pair it meets.
        let known = vec![Known {
            start: 9.0,
            end: 12.0,
            text: "hold me".into(),
            timed: true,
        }];
        let placed = place_known(&known, &heard);
        assert!((placed[0].words[0].start.unwrap() - 10.0).abs() < 1e-9);
        let second = vec![Known {
            start: 59.0,
            end: 62.0,
            text: "hold me".into(),
            timed: true,
        }];
        let placed = place_known(&second, &heard);
        assert!((placed[0].words[0].start.unwrap() - 60.0).abs() < 1e-9);
        // A timed line nothing matched keeps its own time.
        let lonely = vec![Known {
            start: 100.0,
            end: 104.0,
            text: "la la".into(),
            timed: true,
        }];
        let placed = place_known(&lonely, &heard);
        assert!((placed[0].words[0].start.unwrap() - 100.0).abs() < 1e-9);
        assert!(placed[0].words[1].end.unwrap() <= 104.0);
    }

    #[test]
    fn alike_words_are_near_misses_and_others_are_not() {
        assert_eq!(alike("there", "there"), 3);
        assert_eq!(alike("paler", "pale"), 1);
        assert_eq!(alike("grey", "gray"), 1);
        assert_eq!(alike("dullness", "darkness"), 0);
        assert_eq!(alike("a", "i"), 0);
    }

    #[test]
    fn an_estimate_learns_from_this_machine() {
        let small = model("small").expect("small");
        let base = model("base").expect("base");
        let four_minutes = 240.0;
        let (guess, measured) = estimate(small, four_minutes, &Rates::default(), 4);
        assert!(!measured);
        assert!((guess - small.seconds_per_minute * 4.0).abs() < 1e-9);
        let (slower, _) = estimate(small, four_minutes, &Rates::default(), 2);
        assert!(slower > guess, "fewer cores, longer");
        // This machine ran base at half the reference's speed: small is
        // expected at half speed too.
        let mut rates = Rates::default();
        rates.0.insert("base".into(), base.seconds_per_minute * 2.0);
        let (scaled, measured) = estimate(small, four_minutes, &rates, 4);
        assert!(!measured);
        assert!((scaled - small.seconds_per_minute * 2.0 * 4.0).abs() < 1e-6);
        rates.0.insert("small".into(), 20.0);
        assert_eq!(estimate(small, four_minutes, &rates, 4), (80.0, true));
    }

    #[test]
    fn the_chosen_model_is_one_that_is_here() {
        let dir = tempfile::tempdir().expect("a folder");
        let models = Models {
            root: dir.path().to_path_buf(),
        };
        assert_eq!(models.chosen(), None);
        let tiny = model("tiny").expect("tiny");
        assert!(models.choose(tiny).is_err(), "not downloaded");
        // A file of the right size stands in for a download.
        let file = std::fs::File::create(models.path(tiny)).expect("made");
        file.set_len(tiny.bytes).expect("sized");
        assert_eq!(models.chosen().map(|m| m.id), Some("tiny"));
        models.choose(tiny).expect("chosen");
        assert_eq!(models.chosen().map(|m| m.id), Some("tiny"));
        models.remove(tiny).expect("removed");
        assert_eq!(models.chosen(), None);
        models.remember_rate(tiny, 12.0, 240.0);
        assert_eq!(models.rates().0.get("tiny"), Some(&3.0));
    }

    #[test]
    fn the_prompt_is_the_start_of_the_known_words() {
        let known: Vec<Known> = (0..100)
            .map(|i| Known {
                start: 0.0,
                end: 0.0,
                text: format!("line number {i} of the song"),
                timed: false,
            })
            .collect();
        let prompt = prompt_of(&known).expect("a prompt");
        assert!(prompt.starts_with("line number 0 of the song line number 1"));
        assert!(prompt.len() <= 600);
        assert_eq!(prompt_of(&[]), None);
    }

    /// **whisper.cpp itself, on a real record.** Ignored: it needs a model
    /// and a song, which CI has neither of.
    ///
    /// ```text
    /// DJMANZO_WHISPER_MODEL=<ggml file> DJMANZO_WHISPER_RECORD=<16 kHz mono WAV> \
    ///   cargo test -p dj-app --lib whisper_listens -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a whisper.cpp model and a real record"]
    fn whisper_listens_to_a_real_record() {
        let (Ok(path), Ok(record)) = (
            std::env::var("DJMANZO_WHISPER_MODEL"),
            std::env::var("DJMANZO_WHISPER_RECORD"),
        ) else {
            return;
        };
        let bytes = std::fs::read(&record).expect("read");
        let audio: Vec<f32> = bytes[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
            .collect();
        let id = MODELS
            .iter()
            .find(|m| path.ends_with(m.file))
            .expect("one of djmanzo's models");
        let heard =
            listen(Path::new(&path), id, &audio, Some("en"), None, 4, |_| {}).expect("listened");
        for segment in &heard.segments {
            eprintln!("{:?} {}", segment.start, segment.text);
        }
        eprintln!(
            "load {:.2} s, listen {:.2} s, {} segments",
            heard.load_seconds,
            heard.listen_seconds,
            heard.segments.len()
        );
    }
}

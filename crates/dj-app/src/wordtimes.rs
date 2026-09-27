//! §122: when each word is sung — the words, their times, and the LRC the
//! singers' screen reads.
//!
//! > I prefer x-whisper, since it also tells the timestamps for the
//! > words/lyrics ... that's necessary for karaoke
//!
//! The listening is whisper.cpp's, compiled into djmanzo (`crate::whispercpp`),
//! since the owner asked for word timing that ships with djmanzo rather than
//! WhisperX's two and a half gigabytes of Python. This module is what
//! surrounds it: the record decoded and brought to sixteen kilohertz mono —
//! the separated vocals when separation has finished, the mix otherwise —
//! the words already known handed over, the answer turned into **enhanced
//! LRC** ([`to_lrc`]) — a line time and a time before every word — which the
//! library keeps as the record's timed words and the singers' screen wipes
//! word by word (`dj_library::lrc`).
//!
//! **Every run is timed**, stage by stage, against the owner's minute and a
//! half for a song ([`BUDGET_SECONDS`]), and says whether it met it; what it
//! took is kept, so the next estimate is this machine's own.

use crate::whispercpp::{Model, Models};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

/// The owner's limit for analysing a song: "one that doesn't exceed 1.5 min
/// of analysis on this laptop approx."
pub const BUDGET_SECONDS: f64 = 90.0;

/// What Whisper listens to: mono, at the rate its models were trained on.
pub const SAMPLE_RATE: u32 = 16_000;

/// A line of the record whose words are known, and when it is sung if that
/// is known too.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Known {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Whether `start` and `end` are the line's own times, from timed
    /// lyrics, rather than the whole record.
    #[serde(default)]
    pub timed: bool,
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

/// The words of a record, placed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Answer {
    /// `align` when the words were known and placed, `transcribe` when they
    /// were found.
    pub mode: String,
    pub language: String,
    pub segments: Vec<Segment>,
    /// How long each stage took, in seconds.
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
    /// Each stage and its seconds: preparing the audio, loading the model,
    /// listening, placing the known words.
    pub stages: Vec<(String, f64)>,
    /// From pressing the button to the words being kept.
    pub seconds: f64,
    /// The record's length, for the budget's sake.
    pub record_seconds: f64,
    pub within_budget: bool,
    /// `vocals` when Whisper heard the separated vocal stem, `mix` when
    /// separation had not finished and it heard the whole record.
    pub heard: String,
    /// The model it listened with.
    pub model: String,
}

/// Where word timing has got to, for the interface to read.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Progress {
    /// The model being downloaded, by its id.
    pub downloading: Option<String>,
    pub downloaded: u64,
    pub download_total: u64,
    /// The deck being listened to, and how much of it is done, in percent.
    pub listening: Option<u8>,
    pub percent: i32,
    /// Why the last download or run failed.
    pub error: Option<String>,
    /// How the last run went.
    pub last: Option<Report>,
    /// This machine's download speed in bytes a second, once measured.
    pub speed: Option<f64>,
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

/// The lines whose words are known, from what the library holds.
///
/// Timed lines run each to the next, the last to the end of the record.
/// Plain lines, with no times, each span the whole record: where they are
/// sung is left to what Whisper heard.
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
                timed: true,
            });
        }
        return out;
    }
    plain
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| Known {
            start: 0.0,
            end: record_seconds,
            text: line.to_owned(),
            timed: false,
        })
        .collect()
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
    let mut out = String::from("[re:djmanzo, words timed by whisper.cpp]\n");
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

/// `interleaved` audio of `channels` at `rate`, as Whisper hears it: mono,
/// at sixteen kilohertz. With the record's length in seconds.
#[must_use]
pub fn sixteen_kilohertz_mono(interleaved: &[f32], channels: usize, rate: u32) -> (Vec<f32>, f64) {
    let channels = channels.max(1);
    let mono: Vec<f32> = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    let seconds = mono.len() as f64 / f64::from(rate.max(1));
    (to_sixteen_kilohertz(&mono, rate), seconds)
}

/// `input`, at `from` hertz, brought to [`SAMPLE_RATE`] for Whisper.
///
/// Not `dj_stems::resample`, which computes its kernel sample by sample for
/// the stems' sake and took sixteen seconds over a four-minute record —
/// more than the owner's first budget, fifteen seconds, before anything had
/// listened. Speech
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

/// Time the words of the record at `path` with `model`: decode it, listen,
/// place the words already known, and time every stage. `progress` is told
/// the percentage listened. On a worker thread.
///
/// # Errors
/// The model not downloaded, a record that would not decode, or what
/// listening says.
pub fn time_words(
    models: &Models,
    model: &Model,
    path: &Path,
    known: impl FnOnce(f64) -> Vec<Known>,
    language: Option<String>,
    stems: Option<&dj_decode::StemBuffer>,
    progress: impl FnMut(i32) + 'static,
) -> Result<(Answer, Report), String> {
    if !models.installed(model) {
        return Err(format!("{} is not downloaded yet", model.name));
    }
    crate::whispercpp::cpu_can_run()?;
    let began = Instant::now();
    let decoded = dj_decode::decode_file(path).map_err(|e| e.to_string())?;
    let buffer = decoded.buffer;
    let vocals = stems.and_then(|stems| vocals_of(&stems.load(), buffer.len_frames()));
    let heard_from = if vocals.is_some() { "vocals" } else { "mix" };
    let (audio, record_seconds) = sixteen_kilohertz_mono(
        vocals.as_deref().unwrap_or_else(|| buffer.as_interleaved()),
        dj_decode::CHANNELS,
        buffer.sample_rate().get(),
    );
    drop(buffer);
    let lines = known(record_seconds);
    let prepared = began.elapsed().as_secs_f64();
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let language = language
        .map(|code| code.trim().to_lowercase())
        .filter(|code| !code.is_empty());
    let heard = crate::whispercpp::listen(
        &models.path(model),
        model,
        &audio,
        language.as_deref(),
        crate::whispercpp::prompt_of(&lines).as_deref(),
        threads,
        progress,
    )?;
    let mut stages = vec![
        ("prepare".to_owned(), prepared),
        ("load".to_owned(), heard.load_seconds),
        ("listen".to_owned(), heard.listen_seconds),
    ];
    let (mode, segments) = if lines.is_empty() {
        ("transcribe", heard.segments)
    } else {
        let placing = Instant::now();
        let placed = crate::whispercpp::place_known(&lines, &heard.segments);
        stages.push(("place".to_owned(), placing.elapsed().as_secs_f64()));
        ("align", placed)
    };
    // English, with the aligner downloaded: its times, to the letter, where
    // it places the words. Without ONNX Runtime — the Intel Mac build has
    // none — Whisper's times stand.
    let segments = if heard.language == "en" && models.aligner_installed() {
        match dj_stems::align::Aligner::new(&models.aligner_path()) {
            Ok(aligner) => {
                let aligning = Instant::now();
                let rate = f64::from(SAMPLE_RATE);
                let refined =
                    crate::whispercpp::refine(segments, record_seconds, |from, to, words| {
                        let a = ((from * rate) as usize).min(audio.len());
                        let b = ((to * rate) as usize).clamp(a, audio.len());
                        aligner.align(&audio[a..b], words).ok()
                    });
                stages.push(("align".to_owned(), aligning.elapsed().as_secs_f64()));
                refined
            }
            Err(why) => {
                tracing::warn!(%why, "the English aligner is not available; Whisper's times stand");
                segments
            }
        }
    } else {
        segments
    };
    let answer = Answer {
        mode: mode.to_owned(),
        language: heard.language,
        segments,
        stages: stages.clone(),
    };
    let seconds = began.elapsed().as_secs_f64();
    models.remember_rate(model, seconds, record_seconds);
    let report = Report {
        mode: answer.mode.clone(),
        language: answer.language.clone(),
        words: timed_words(&answer),
        stages,
        seconds,
        record_seconds,
        within_budget: seconds <= BUDGET_SECONDS,
        heard: heard_from.to_owned(),
        model: model.id.to_owned(),
    };
    Ok((answer, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

    /// **The load-bearing one.** What Whisper answers becomes LRC the
    /// singers' screen reads back word by word, at the times it gave —
    /// through `dj_library::lrc::parse`, the reader the screen really uses,
    /// not a second one written for the test.
    #[test]
    fn the_words_come_back_through_the_screens_own_reader() {
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

    #[test]
    fn a_time_is_written_as_lrc_writes_it() {
        assert_eq!(stamp(0.0), "00:00.00");
        assert_eq!(stamp(62.5), "01:02.50");
        assert_eq!(stamp(599.999), "10:00.00");
        assert_eq!(stamp(-3.0), "00:00.00");
    }

    /// Timed lines run to the next; plain lines each span the record, to be
    /// placed where they were heard; nothing is nothing, and then the record
    /// is transcribed.
    #[test]
    fn the_words_already_known_are_handed_over_line_by_line() {
        let synced = "[ar:Juan Luis Guerra]\n[00:12.00]Bachata en Fukuoka\n[00:20.00]\n[00:30.50]tengo dos amores\n";
        let known = known_lines(Some(synced), "", 240.0);
        assert_eq!(
            known,
            vec![
                Known {
                    start: 12.0,
                    end: 30.5,
                    text: "Bachata en Fukuoka".into(),
                    timed: true,
                },
                Known {
                    start: 30.5,
                    end: 240.0,
                    text: "tengo dos amores".into(),
                    timed: true,
                },
            ]
        );
        let plain = known_lines(None, "Bachata en Fukuoka\n\n  tengo dos amores  \n", 240.0);
        assert_eq!(plain.len(), 2, "a line each");
        assert_eq!(plain[1].text, "tengo dos amores");
        assert!(!plain[0].timed);
        assert_eq!((plain[0].start, plain[0].end), (0.0, 240.0));
        assert!(known_lines(None, " \n", 240.0).is_empty());
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
    /// **The whole run on a real record**: decoded, heard by the chosen
    /// model, and — for English, with the aligner downloaded — placed by the
    /// aligner. Ignored: it needs models, ONNX Runtime and a song.
    ///
    /// ```text
    /// ORT_DYLIB_PATH=<libonnxruntime> DJMANZO_MODELS=<folder with a model \
    ///   and wav2vec2-base-960h-q8.onnx> DJMANZO_RECORD=<song> \
    ///   cargo test -p dj-app --lib whole_run -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs models, ONNX Runtime and a real record"]
    fn the_whole_run_on_a_real_record() {
        let (Ok(root), Ok(record)) = (
            std::env::var("DJMANZO_MODELS"),
            std::env::var("DJMANZO_RECORD"),
        ) else {
            return;
        };
        let models = Models {
            root: std::path::PathBuf::from(root),
        };
        let model = models.chosen().expect("a model in the folder");
        let (answer, report) = time_words(
            &models,
            model,
            Path::new(&record),
            |_| Vec::new(),
            Some("en".into()),
            None,
            |_| {},
        )
        .expect("timed");
        for segment in &answer.segments {
            let words: Vec<String> = segment
                .words
                .iter()
                .map(|w| format!("{}@{:.2}", w.word, w.start.unwrap_or(-1.0)))
                .collect();
            println!("WORDS {}", words.join(" "));
        }
        eprintln!("{}", serde_json::to_string(&report).expect("a report"));
    }

    #[test]
    fn the_record_is_handed_over_mono_at_sixteen_kilohertz() {
        let stereo: Vec<f32> = (0..48_000 * 2)
            .map(|i| if i % 2 == 0 { 0.5 } else { 0.1 })
            .collect();
        let (mono, seconds) = sixteen_kilohertz_mono(&stereo, 2, 48_000);
        assert!((seconds - 1.0).abs() < 1e-9);
        assert_eq!(mono.len(), 16_000, "a second at sixteen kilohertz");
        // The middle of it is the two channels' mean.
        assert!((mono[8_000] - 0.3).abs() < 0.01, "{}", mono[8_000]);
    }
}

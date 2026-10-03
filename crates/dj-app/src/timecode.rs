//! Commands for timecode vinyl: putting a deck on a control record.
//!
//! The engine has been able to follow a control record since [`dj_dvs`] landed,
//! and until now nothing could ask it to. This is the door.
//!
//! # What the interface has to be honest about
//!
//! Three things, and each is a field below rather than a paragraph in a manual:
//!
//! - **Which record.** djmanzo ships its own timecode and no vendor's, because
//!   the published parameters for the vendor records could not be confirmed and
//!   one of them is provably not maximal — see [`dj_dvs::TimecodeFormat`].
//!   [`write_timecode_signal`] exists so that is not a dead end: a DJ can render
//!   djmanzo's own signal to a WAV, burn it or play it off a phone, and control
//!   djmanzo from any turntable or CD deck.
//! - **Whether it is reading.** [`TimecodeDeckDto::quality`] is negative when
//!   the deck is not on vinyl at all, and zero when it is connected and hearing
//!   nothing. A dusty record, a dead cartridge and the wrong input picked all
//!   look identical from the outside, and a DJ whose deck will not move needs
//!   them told apart.
//! - **Whether it has been proven.** It has not. Everything here is verified
//!   against a signal djmanzo generates, which pins the encoding and proves
//!   nothing about a pressing nobody here has heard. [`TimecodeStatusDto`]
//!   carries that sentence so the panel cannot forget to say it.

use crate::state::{AppState, TimecodeSetup};
use dj_core::{DeckId, ParamId, param::DeckParam};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

/// A control record, as the picker draws it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimecodeFormatDto {
    pub name: String,
    pub carrier_hz: f64,
    pub bits: u32,
    /// How long the record runs before a position could be mistaken for
    /// another. Shown because it is the one number that decides whether a
    /// format suits a set: a format good for four minutes is no use under a
    /// twelve-minute edit.
    pub unambiguous_seconds: f64,
    /// Whether the numbers describe a record that could work at all. False
    /// entries are shown rather than hidden, because a DJ who typed a tap value
    /// in needs to see it was rejected.
    pub usable: bool,
}

impl From<&dj_dvs::TimecodeFormat> for TimecodeFormatDto {
    fn from(format: &dj_dvs::TimecodeFormat) -> Self {
        TimecodeFormatDto {
            name: format.name.clone(),
            carrier_hz: format.carrier_hz,
            bits: format.bits,
            unambiguous_seconds: format.unambiguous_seconds(),
            usable: format.is_usable(),
        }
    }
}

/// One deck's relationship with a turntable.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimecodeDeckDto {
    /// 1-based, as it is printed on the hardware.
    pub deck: u8,
    /// Whether this deck is following a record.
    pub running: bool,
    /// Which record, when one is attached.
    pub format: Option<String>,
    /// The input it arrives on.
    pub device: Option<String>,
    /// True when the needle's place on the record is the playhead's place in
    /// the track.
    pub absolute: bool,
    /// How much of what is arriving looks like timecode, 0.0..=1.0 — and
    /// **negative when the deck is not on vinyl at all**. Zero means connected
    /// and hearing nothing, which is a different problem with a different fix.
    pub quality: f32,
    /// The speed the record is asking for, 1.0 being normal play and negative
    /// being backwards. Shown beside the quality because a plausible speed with
    /// a poor quality is a needle that is about to lose its place.
    pub speed: f32,
}

/// Everything the timecode panel draws.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimecodeStatusDto {
    pub decks: Vec<TimecodeDeckDto>,
    pub formats: Vec<TimecodeFormatDto>,
    /// Whether an output is open. Nothing can be attached before one is: the
    /// engine only exists once a device does.
    pub engine_running: bool,
    /// The compatibility caveat, in the words the panel should print.
    pub caveat: &'static str,
}

/// Quality for a deck that is not on a control record at all.
///
/// Negative, and deliberately not zero: zero is a deck that *is* on a record
/// and hearing nothing, which is a dead cartridge or the wrong input picked,
/// and the two send a DJ to different places. Matches what the engine publishes
/// for the same state.
const NOT_ON_VINYL: f32 = -1.0;

/// What no test in this repository can establish.
const CAVEAT: &str = "djmanzo's timecode decoder is verified against signals djmanzo generates, \
not against a pressed vendor record. Serato and Traktor discs are not offered because their \
published parameters could not be confirmed. Use djmanzo's own signal — you can write it to a \
file below — or add a format once you have confirmed one on a real turntable.";

/// The formats djmanzo knows about.
#[tauri::command]
#[must_use]
pub fn timecode_formats() -> Vec<TimecodeFormatDto> {
    dj_dvs::TimecodeFormat::bundled()
        .iter()
        .map(TimecodeFormatDto::from)
        .collect()
}

/// Which decks are on vinyl, and how well it is going.
///
/// Asked on a timer while the calibration panel is open, and once when it is
/// not: quality and speed move at audio rate, and the rest changes only when a
/// DJ presses something.
#[tauri::command]
#[must_use]
pub fn timecode_status(state: State<'_, AppState>) -> TimecodeStatusDto {
    status_of(&state)
}

/// What [`timecode_status`] reports, minus Tauri's `State` wrapper — which is
/// the one thing in that function a unit test cannot build.
fn status_of(state: &AppState) -> TimecodeStatusDto {
    let registry = state.registry();
    let decks = state
        .timecode_all()
        .into_iter()
        .enumerate()
        .filter_map(|(index, setup)| {
            let id = DeckId::new(u8::try_from(index).ok()?)?;
            // The engine publishes the "not on vinyl" sentinel itself, but only
            // while there *is* an engine. Before a device is open the registry
            // still holds its initial zero, which means "connected, hearing
            // nothing" -- so a panel trusting the registry alone would tell a
            // DJ who has not plugged anything in that their cartridge is dead.
            // What this process knows for certain is whether it opened an
            // input, so that is what decides.
            let (quality, speed) = if setup.is_some() {
                (
                    registry.get(ParamId::Deck(id, DeckParam::TimecodeQuality)),
                    registry.get(ParamId::Deck(id, DeckParam::TimecodeSpeed)),
                )
            } else {
                (NOT_ON_VINYL, 0.0)
            };
            Some(TimecodeDeckDto {
                deck: id.human_number(),
                running: setup.is_some(),
                format: setup.as_ref().map(|s| s.format.name.clone()),
                device: setup.as_ref().map(|s| s.device.clone()),
                absolute: setup.as_ref().is_some_and(|s| s.absolute),
                quality,
                speed,
            })
        })
        .collect();
    TimecodeStatusDto {
        decks,
        formats: timecode_formats(),
        engine_running: state.active_device().is_some(),
        caveat: CAVEAT,
    }
}

/// Put a deck on a control record.
///
/// `format` names one of [`timecode_formats`]; omitted, the first bundled
/// format is used, which is the one most likely to be right for a DJ who has
/// not thought about carrier frequencies.
///
/// `absolute` decides what the record means. In absolute mode the needle's
/// place on the record is the playhead's place in the track, so dropping the
/// needle two minutes in starts the track two minutes in. In relative mode only
/// the *movement* is followed: lifting and re-dropping changes nothing, which is
/// what most DJs want most of the time and why it is the default.
///
/// # Errors
/// When there is no such deck, no such format, no output open yet, or the input
/// device will not open.
#[tauri::command]
pub fn start_timecode(
    state: State<'_, AppState>,
    deck: u8,
    device_id: Option<String>,
    format: Option<String>,
    absolute: Option<bool>,
) -> Result<TimecodeStatusDto, String> {
    let id = DeckId::from_human(deck).ok_or_else(|| format!("no deck {deck}"))?;
    let chosen = pick_format(format.as_deref())?;
    let absolute = absolute.unwrap_or(false);

    let config = state
        .host()
        .open_timecode(
            id,
            device_id.clone().map(dj_audio::DeviceId::new),
            chosen.clone(),
            absolute,
        )
        .map_err(|e| e.to_string())?;

    // Only after the host has actually opened something, so a failed open
    // leaves the panel saying the deck is on its own transport, which it is.
    state.set_timecode(
        id,
        Some(TimecodeSetup {
            format: chosen,
            device: config.device_name,
            absolute,
        }),
    );
    Ok(status_of(&state))
}

/// Take a deck off vinyl and give it its transport back.
///
/// # Errors
/// When there is no such deck, or the host cannot be reached.
#[tauri::command]
pub fn stop_timecode(state: State<'_, AppState>, deck: u8) -> Result<TimecodeStatusDto, String> {
    let id = DeckId::from_human(deck).ok_or_else(|| format!("no deck {deck}"))?;
    state.host().close_timecode(id).map_err(|e| e.to_string())?;
    state.set_timecode(id, None);
    Ok(status_of(&state))
}

/// Find a named format among the bundled ones, or the first if none is named.
fn pick_format(name: Option<&str>) -> Result<dj_dvs::TimecodeFormat, String> {
    let bundled = dj_dvs::TimecodeFormat::bundled();
    match name {
        Some(wanted) => bundled
            .into_iter()
            .find(|f| f.name == wanted)
            .ok_or_else(|| format!("djmanzo does not know a control record called {wanted}")),
        None => bundled
            .into_iter()
            .next()
            .ok_or_else(|| "djmanzo ships no control records".to_owned()),
    }
}

/// How long a record to write, given what was asked for.
///
/// Its own function because it is a **decision**, and the write around it is
/// I/O. Testing a decision through the I/O meant rendering the record's whole
/// period to disk to find out whether a number had been clamped -- 180 MB of
/// WAV per assertion, which filled the disk on the machine this was written on
/// and would do the same to a constrained continuous-integration runner.
///
/// # Errors
/// When the requested length is not a length: zero, negative, or not a number.
fn usable_length(format: &dj_dvs::TimecodeFormat, requested: Option<f64>) -> Result<f64, String> {
    let seconds = requested.unwrap_or_else(|| format.unambiguous_seconds());
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err("a control record needs a length".to_owned());
    }
    // Past this the sequence repeats and two places on the record share a
    // position, which is exactly the failure `is_usable` exists to prevent.
    // Capped rather than refused: a DJ asking for an hour wants as much as they
    // can have, not an error.
    Ok(seconds.min(format.unambiguous_seconds()))
}

/// How loud the written signal is, as a fraction of full scale.
///
/// Not 1.0. A control record played into a phono stage and back out arrives
/// with its peaks a little taller than they left — RIAA equalisation is not
/// flat and neither is a cartridge — and a signal already at full scale clips
/// on the way in, which reads as a dirty record rather than as a loud one.
const WRITE_LEVEL: f32 = 0.8;

/// How much signal is rendered per write. Whole frames, so the two channels
/// never fall out of step.
const WRITE_CHUNK_FRAMES: usize = 8192;

/// What [`write_timecode_signal`] produced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenSignalDto {
    pub path: String,
    pub seconds: f64,
    pub sample_rate: u32,
    pub format: String,
}

/// Write djmanzo's control signal to a WAV file.
///
/// This is the answer to "djmanzo ships no Serato format": a DJ does not need
/// one. Render this, burn it to a CD or put it on a phone or a USB stick, and
/// any turntable, CD deck or media player becomes a controller — the same
/// trick, without buying a record.
///
/// Written on the calling thread rather than in a worker because it is a few
/// seconds of arithmetic for a signal a DJ makes once, and a progress bar for
/// it would be more machinery than the job.
///
/// # Errors
/// When the format is unknown or unusable, the length is not a sensible one, or
/// the file cannot be written.
#[tauri::command]
pub fn write_timecode_signal(
    path: String,
    format: Option<String>,
    seconds: Option<f64>,
    sample_rate: Option<u32>,
) -> Result<WrittenSignalDto, String> {
    let chosen = pick_format(format.as_deref())?;
    if !chosen.is_usable() {
        return Err(format!(
            "{} does not describe a control record that could work",
            chosen.name
        ));
    }
    let rate = sample_rate.unwrap_or(44_100);
    if !(8_000..=192_000).contains(&rate) {
        return Err(format!(
            "{rate} Hz is not a sample rate to write a record at"
        ));
    }
    let seconds = usable_length(&chosen, seconds)?;

    let synth = dj_dvs::Synth::new(chosen.clone(), f64::from(rate))
        .ok_or_else(|| format!("{} cannot be rendered at {rate} Hz", chosen.name))?;

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let total_frames = (seconds * f64::from(rate)) as usize;
    let path = PathBuf::from(path);
    let mut wav = crate::wav::Wav::create(&path, rate).map_err(|e| e.to_string())?;

    let mut float = vec![0.0f32; WRITE_CHUNK_FRAMES * 2];
    let mut pcm = vec![0i16; WRITE_CHUNK_FRAMES * 2];
    let mut done = 0usize;
    while done < total_frames {
        let frames = WRITE_CHUNK_FRAMES.min(total_frames - done);
        let samples = frames * 2;
        // The bit position *is* the cycle count, so where a chunk starts in the
        // sequence is where it starts in time. Rendering each chunk from its
        // own offset rather than from zero is what makes the seams join.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let from_bit = (done as f64 * chosen.carrier_hz / f64::from(rate)) as u32;
        synth.render_into(&mut float[..samples], from_bit, 1.0);
        for (out, sample) in pcm[..samples].iter_mut().zip(&float[..samples]) {
            let scaled = (sample * WRITE_LEVEL).clamp(-1.0, 1.0);
            #[allow(clippy::cast_possible_truncation)]
            {
                *out = (scaled * f32::from(i16::MAX)) as i16;
            }
        }
        wav.write(&pcm[..samples]).map_err(|e| e.to_string())?;
        done += frames;
    }
    let written = wav.close().map_err(|e| e.to_string())?;

    Ok(WrittenSignalDto {
        path: written.display().to_string(),
        #[allow(clippy::cast_precision_loss)]
        seconds: total_frames as f64 / f64::from(rate),
        sample_rate: rate,
        format: chosen.name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the tests that open null capture streams.
    ///
    /// [`dj_audio::null::live_input_streams`] counts per process, so two tests
    /// with a capture open at once see each other's. Declared before the
    /// `AppState` in each test that takes it, so the host -- and with it the
    /// streams -- is dropped before the lock is released.
    fn input_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A file that is deleted when the test ends, however it ends.
    struct TempWav(PathBuf);

    impl TempWav {
        fn new(name: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "djmanzo-timecode-{name}-{}.wav",
                std::process::id()
            ));
            TempWav(path)
        }
    }

    impl Drop for TempWav {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Read a 16-bit stereo WAV back into interleaved floats.
    ///
    /// Deliberately not `dj-decode`: the point of this test is whether the
    /// *bytes on disk* carry the signal, and going back out through the same
    /// project's decoder would let a shared misunderstanding cancel itself out.
    fn read_wav(path: &std::path::Path) -> (u32, Vec<f32>) {
        let bytes = std::fs::read(path).expect("the file was written");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let channels = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
        assert_eq!(channels, 2, "a control record has to be stereo");
        let data = &bytes[44..];
        let samples = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / f32::from(i16::MAX))
            .collect();
        (rate, samples)
    }

    #[test]
    fn the_default_format_is_the_first_bundled_one() {
        let chosen = pick_format(None).expect("a format");
        assert_eq!(chosen, dj_dvs::TimecodeFormat::bundled()[0]);
    }

    #[test]
    fn a_format_djmanzo_does_not_know_is_refused_by_name() {
        let error = pick_format(Some("Serato CV02")).expect_err("no such format");
        assert!(
            error.contains("Serato CV02"),
            "the refusal did not name what was asked for: {error}"
        );
    }

    #[test]
    fn every_offered_format_is_one_that_could_work() {
        for format in timecode_formats() {
            assert!(
                format.usable,
                "{} is offered in the picker but its numbers cannot work",
                format.name
            );
            assert!(
                format.unambiguous_seconds > 60.0,
                "{} repeats itself after {} seconds, which is shorter than a track",
                format.name,
                format.unambiguous_seconds
            );
        }
    }

    /// **The generated signal is one djmanzo can actually read.**
    ///
    /// This is the test that decides whether "write your own control record"
    /// is a feature or a sentence in a manual. It goes all the way out to
    /// 16-bit PCM on disk and back — so the write level, the clamp, the integer
    /// conversion and the chunk seams are all in the path, not just the synth.
    #[test]
    fn a_written_signal_decodes_back_at_normal_speed() {
        let temp = TempWav::new("roundtrip");
        let written =
            write_timecode_signal(temp.0.display().to_string(), None, Some(2.0), Some(44_100))
                .expect("the signal was written");
        assert_eq!(written.sample_rate, 44_100);
        assert!((written.seconds - 2.0).abs() < 0.01);

        let (rate, samples) = read_wav(&temp.0);
        assert_eq!(rate, 44_100);
        assert_eq!(samples.len(), 44_100 * 2 * 2, "two seconds of stereo");

        let format = pick_format(None).unwrap();
        let mut decoder = dj_dvs::Decoder::new(format, f64::from(rate)).expect("a decoder");
        // Fed in blocks, as a sound card would deliver it.
        let mut reading = decoder.feed(&samples[..2048]);
        for block in samples[2048..].chunks(2048) {
            reading = decoder.feed(block);
        }
        assert!(
            (reading.speed - 1.0).abs() < 0.05,
            "a signal written for normal play read back at {}",
            reading.speed
        );
        assert!(
            reading.quality > 0.5,
            "djmanzo's own signal read back at quality {}",
            reading.quality
        );
    }

    /// **The seams join.** Each chunk is rendered from its own place in the
    /// sequence; rendering every chunk from bit zero instead would restart the
    /// record every 8192 frames, which a decoder reads as the needle jumping.
    #[test]
    fn a_signal_longer_than_one_chunk_keeps_its_place() {
        let temp = TempWav::new("seams");
        // Well past WRITE_CHUNK_FRAMES, so there are seams to get wrong.
        write_timecode_signal(temp.0.display().to_string(), None, Some(1.5), Some(44_100))
            .expect("written");
        let (rate, samples) = read_wav(&temp.0);

        let format = pick_format(None).unwrap();
        let mut decoder = dj_dvs::Decoder::new(format, f64::from(rate)).expect("a decoder");
        // Warm up past the first seam, then read positions either side of the
        // next one.
        for block in samples[..WRITE_CHUNK_FRAMES * 2].chunks(2048) {
            decoder.feed(block);
        }
        let mut positions = Vec::new();
        for block in samples[WRITE_CHUNK_FRAMES * 2..].chunks(2048) {
            if let Some(seconds) = decoder.feed(block).position {
                positions.push(seconds);
            }
        }
        assert!(
            positions.len() >= 2,
            "the decoder never found its place after the first seam"
        );
        for pair in positions.windows(2) {
            let step = pair[1] - pair[0];
            assert!(
                (0.0..0.2).contains(&step),
                "the position jumped by {step}s across a chunk boundary, so the seams do not join"
            );
        }
    }

    /// **A length past the record's own period is capped, not refused.**
    ///
    /// Past that point the shift register repeats and two places on the record
    /// share a position, which is the silent failure the whole format check
    /// exists to prevent. A DJ asking for an hour wants as much as they can
    /// have, not an error.
    #[test]
    fn a_length_past_the_records_own_period_is_capped_not_refused() {
        let format = pick_format(None).unwrap();
        let period = format.unambiguous_seconds();
        assert_eq!(
            usable_length(&format, Some(period * 4.0)).expect("a long request is answered"),
            period
        );
        // And a length within the period is left alone rather than clamped to
        // something tidy.
        assert_eq!(usable_length(&format, Some(30.0)).unwrap(), 30.0);
        // Asking for nothing in particular gives the whole record.
        assert_eq!(usable_length(&format, None).unwrap(), period);
    }

    #[test]
    fn a_nonsense_length_is_refused() {
        let format = pick_format(None).unwrap();
        assert!(usable_length(&format, Some(0.0)).is_err());
        assert!(usable_length(&format, Some(-5.0)).is_err());
        assert!(usable_length(&format, Some(f64::NAN)).is_err());
        assert!(usable_length(&format, Some(f64::INFINITY)).is_err());
    }

    #[test]
    fn a_fresh_app_reports_no_deck_on_vinyl() {
        let state = AppState::new(true);
        let status = status_of(&state);
        assert_eq!(status.decks.len(), dj_core::MAX_DECKS);
        for deck in &status.decks {
            assert!(!deck.running, "deck {} started on vinyl", deck.deck);
            assert!(
                deck.quality < 0.0,
                "deck {} reported quality {} with nothing connected, which the panel draws as \
                 a dead cartridge",
                deck.deck,
                deck.quality
            );
        }
    }

    /// **A device change closes the inputs it opened.**
    ///
    /// The bug: opening an output builds a fresh engine, and every input --
    /// the microphone, and every deck on a control record -- is half of a ring
    /// whose other half belonged to the engine being dropped. Left open, the
    /// device callback keeps running and keeps writing into a ring nobody
    /// drains. The microphone went silently dead on a device change and held a
    /// sound card open for nobody; no test could see it, because until the null
    /// backend could capture, no input path could be reached without hardware.
    ///
    /// Counts streams rather than checking state, because the state is
    /// bookkeeping and the stream is the thing that was leaking.
    #[test]
    fn changing_the_output_device_closes_every_input_it_opened() {
        let _guard = input_lock();
        let state = AppState::new(true);
        crate::commands::open_device_for(&state, None, None, None).expect("the null device opens");
        assert_eq!(dj_audio::null::live_input_streams(), 0);

        let id = DeckId::from_human(1).unwrap();
        state
            .host()
            .open_timecode(id, None, pick_format(None).unwrap(), false)
            .expect("the null backend can capture");
        state.host().open_mic(None).expect("so can the microphone");
        state
            .host()
            .open_vocals(None, Vec::new())
            .expect("and the singers' microphones");
        assert_eq!(
            dj_audio::null::live_input_streams(),
            3,
            "a control record, a microphone and the singers' input are three open captures"
        );

        crate::commands::open_device_for(&state, None, None, None).expect("reconnect");
        assert_eq!(
            dj_audio::null::live_input_streams(),
            0,
            "the old engine went away and its inputs kept running into rings nobody drains"
        );
    }

    /// K3: **the singers' microphones open as wide as the input, with the
    /// host's settings, and close again.** Through the real host thread and
    /// the null backend's two-channel input: the engine ends up holding a strip
    /// per channel, a strip's settings reach `vocal.json` and survive being
    /// read back, and closing takes the rack away and the capture with it.
    #[test]
    fn the_singers_microphones_open_as_wide_as_the_input_and_close_again() {
        let _guard = input_lock();
        let dir = tempfile::tempdir().expect("a folder");
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        crate::commands::open_device_for(&state, None, None, None).expect("the null device opens");
        let inputs = |state: &AppState| {
            state.registry().get(dj_core::ParamId::Global(
                dj_core::param::GlobalParam::VocalInputs,
            ))
        };
        let settle = |state: &AppState, want: f32| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                if (inputs(state) - want).abs() < f32::EPSILON {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            false
        };

        let mc = dj_vocal::StripSettings {
            open: true,
            talkover: true,
            reverb: None,
            ..dj_vocal::StripSettings::default()
        };
        crate::commands::set_vocal_strip(&state, 1, mc).expect("kept and sent");
        assert_eq!(state.read_vocal_settings()[1], mc, "kept in vocal.json");
        assert!(crate::commands::set_vocal_strip(&state, 16, mc).is_err());

        let opened = state
            .host()
            .open_vocals(None, state.read_vocal_settings())
            .expect("the null input opens");
        assert_eq!(opened.channels, 2);
        assert!(settle(&state, 2.0), "the engine never held two strips");
        assert_eq!(dj_audio::null::live_input_streams(), 1);

        state.host().close_vocals().expect("closes");
        assert!(settle(&state, 0.0), "the rack stayed in the engine");
        assert_eq!(dj_audio::null::live_input_streams(), 0);
    }

    /// K3: **what a strip is set to is held to the limits before it is kept
    /// or played.** The screen draws its controls over `dj_vocal::LIMITS`,
    /// but the host does not trust it to: a makeup gain of +200 dB, a ratio
    /// under 1 and a fader that is not a number come back as the most there
    /// is, no compression and the fader's default — in `vocal.json`, and in
    /// what the screen is told. A `vocal.json` edited by hand is held the same
    /// way when it is read. And the screen is given the limits and every
    /// stage's starting point, by name, from the same table.
    #[test]
    fn a_strips_settings_are_held_to_the_limits_before_they_are_kept() {
        let dir = tempfile::tempdir().expect("a folder");
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        let wild = dj_vocal::StripSettings {
            open: true,
            gain_db: f32::NAN,
            compressor: Some(dj_vocal::CompressorSettings {
                threshold_db: -20.0,
                ratio: 0.2,
                makeup_db: 200.0,
            }),
            ..dj_vocal::StripSettings::default()
        };
        crate::commands::set_vocal_strip(&state, 0, wild).expect("kept and sent");
        let kept = state.read_vocal_settings()[0];
        let compressor = kept.compressor.expect("still on");
        assert!(
            (compressor.makeup_db - 24.0).abs() < f32::EPSILON,
            "{compressor:?}"
        );
        assert!(
            (compressor.ratio - 1.0).abs() < f32::EPSILON,
            "{compressor:?}"
        );
        assert!(kept.gain_db.abs() < f32::EPSILON, "{}", kept.gain_db);
        assert!(kept.open, "the switch was not the limits' business");

        let mut edited: serde_json::Value =
            serde_json::to_value(vec![dj_vocal::StripSettings::default()]).expect("serializes");
        edited[0]["reverb"]["seconds"] = serde_json::json!(600.0);
        edited[0]["to_main"] = serde_json::json!(-3.0);
        std::fs::write(
            dir.path().join("vocal.json"),
            serde_json::to_string(&edited).expect("writes"),
        )
        .expect("on disk");
        let read = state.read_vocal_settings()[0];
        assert!(
            (read.reverb.expect("on").seconds - dj_vocal::LONGEST_REVERB_SECONDS).abs()
                < f32::EPSILON
        );
        assert!(read.to_main.abs() < f32::EPSILON);

        let dto = crate::commands::vocals_for(2, 0, 0, Vec::new(), Vec::new(), 0.0);
        assert_eq!(dto.limits.len(), dj_vocal::LIMITS.len());
        assert_eq!(dto.limits["compressor.makeup_db"], [0.0, 24.0]);
        assert_eq!(dto.every_stage, dj_vocal::StripSettings::every_stage());
    }

    /// K3: **a singer's chain is laid over the strip they are put on, and
    /// kept where the singer is.** What the engine is sent is read off the
    /// command queue itself. Ana, in tonight's rotation only, goes on Mic 1
    /// through the rig's own chain; the host turns her compressor up and the
    /// fader down — the fader goes to the rig, the compressor only to what is
    /// played for her. *Keep for this singer* puts her chain on her place in
    /// the rotation; off the microphone, the rig plays again as it was; on
    /// Mic 2 she is heard through her chain over Mic 2's row. Ben, who agreed
    /// to be kept, has his MC chain kept in the guest book instead, and
    /// putting Ana where he is takes him off. *New night* takes everybody
    /// off and Ana's chain with her place — Ben's stays, until he is no
    /// longer to be kept.
    #[test]
    fn a_singers_chain_is_laid_over_the_strip_and_kept_where_the_singer_is() {
        use crate::commands::{keep_for_singer, put_on_mic, set_vocal_strip, vocals_dto};
        use crate::karaoke::Kept;
        let dir = tempfile::tempdir().expect("a folder");
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        let (producer, mut queue) = rtrb::RingBuffer::new(256);
        state.bus().reconnect(producer);
        let mut sent = move || {
            let mut sent = Vec::new();
            while let Ok(command) = queue.pop() {
                if let dj_engine::Command::VocalStrip { strip, settings } = command {
                    sent.push((strip, settings));
                }
            }
            sent
        };

        // The rig: Mic 1 open, to one side; Mic 2 a closed singer's strip.
        let rig = dj_vocal::StripSettings {
            open: true,
            pan: 0.3,
            ..dj_vocal::StripSettings::default()
        };
        set_vocal_strip(&state, 0, rig).expect("kept and sent");
        assert_eq!(sent(), vec![(0, rig)]);
        let mut rotation = state.karaoke();
        assert!(rotation.ask("Ana", "Dancing Queen", None, None, None));
        assert!(rotation.ask("Ben", "Toxic", None, None, None));
        state.set_karaoke(&rotation);
        let mut journal = state.guests();
        let ben = crate::guests::Guest {
            name: "Ben".to_owned(),
            consent: crate::guests::Consent {
                keep: true,
                ..crate::guests::Consent::default()
            },
            ..crate::guests::Guest::default()
        };
        let ben_id = journal.save(ben, 1).expect("written down").id;
        state.set_guests(&journal, &[]).expect("kept");

        // Ana on Mic 1, with nothing kept for her: the rig's own chain.
        put_on_mic(&state, 0, Some("Ana")).expect("on");
        assert_eq!(sent(), vec![(0, rig)]);
        let dto = vocals_dto(&state);
        let on = dto.on[0].clone().expect("Ana is on Mic 1");
        assert_eq!((on.singer.as_str(), on.kept), ("Ana", None));

        // Her compressor up and the fader down.
        let hers = dj_vocal::StripSettings {
            gain_db: -6.0,
            compressor: Some(dj_vocal::CompressorSettings {
                ratio: 8.0,
                ..dj_vocal::CompressorSettings::default()
            }),
            reverb: None,
            ..rig
        };
        set_vocal_strip(&state, 0, hers).expect("played");
        assert_eq!(sent(), vec![(0, hers)]);
        let rig = dj_vocal::StripSettings {
            gain_db: -6.0,
            ..rig
        };
        assert_eq!(
            state.read_vocal_settings()[0],
            rig,
            "the fader is the rig's, the compressor is not"
        );
        assert_eq!(vocals_dto(&state).strips[0], hers);
        // Put on the strip she is already on: what was changed for her stays.
        put_on_mic(&state, 0, Some("Ana")).expect("on");
        assert_eq!(sent(), Vec::new());
        assert_eq!(vocals_dto(&state).strips[0], hers);

        // Kept for her: on her place tonight, she is not in the guest book.
        keep_for_singer(&state, 0).expect("kept");
        assert_eq!(state.karaoke().microphone("Ana"), Some(hers.chain()));
        assert_eq!(state.guests().microphone("Ana"), None);
        assert_eq!(
            vocals_dto(&state).on[0].clone().expect("on").kept,
            Some(Kept::Tonight)
        );

        // Off Mic 1: the rig, as it was.
        put_on_mic(&state, 0, None).expect("off");
        assert_eq!(sent(), vec![(0, rig)]);
        assert_eq!(vocals_dto(&state).strips[0], rig);
        assert!(keep_for_singer(&state, 0).is_err(), "nobody to keep it for");

        // On Mic 2 — named as the host typed it: her chain over Mic 2's row.
        put_on_mic(&state, 1, Some(" ana ")).expect("on");
        let mic2 = dj_vocal::StripSettings::default().with_chain(&hers.chain());
        assert_eq!(sent(), vec![(1, mic2)]);
        assert_eq!(
            vocals_dto(&state).on[1].clone().expect("on").kept,
            Some(Kept::Tonight)
        );

        // Ben on Mic 1 as an MC, kept: in the guest book, not the rotation.
        put_on_mic(&state, 0, Some("Ben")).expect("on");
        assert_eq!(sent(), vec![(0, rig)]);
        let mc = dj_vocal::Preset::Mc.applied_to(&rig);
        set_vocal_strip(&state, 0, mc).expect("played");
        assert_eq!(sent(), vec![(0, mc)]);
        keep_for_singer(&state, 0).expect("kept");
        assert_eq!(state.guests().microphone("Ben"), Some(mc.chain()));
        assert_eq!(state.karaoke().microphone("Ben"), None);
        assert_eq!(
            state.read_vocal_settings()[0],
            rig,
            "the rig is not an MC's"
        );

        // Ana where Ben is: she leaves Mic 2, he leaves Mic 1.
        put_on_mic(&state, 0, Some("Ana")).expect("on");
        let mut moved = sent();
        moved.sort_by_key(|(strip, _)| *strip);
        assert_eq!(
            moved,
            vec![
                (0, rig.with_chain(&hers.chain())),
                (1, dj_vocal::StripSettings::default())
            ]
        );
        let dto = vocals_dto(&state);
        assert_eq!(dto.on[0].clone().expect("on").singer, "Ana");
        assert!(dto.on[1].is_none());

        // A new night: everybody off, Ana's chain gone with her place, Ben's
        // kept with him.
        crate::commands::new_night(&state);
        assert_eq!(sent(), vec![(0, rig)]);
        assert!(vocals_dto(&state).on.iter().all(Option::is_none));
        put_on_mic(&state, 0, Some("Ana")).expect("on");
        assert_eq!(sent(), vec![(0, rig)], "nothing kept for Ana now");
        put_on_mic(&state, 0, Some("Ben")).expect("on");
        assert_eq!(sent(), vec![(0, mc)]);
        assert_eq!(
            vocals_dto(&state).on[0].clone().expect("on").kept,
            Some(Kept::GuestBook)
        );

        // Ben no longer to be kept: his chain goes from his record.
        let mut journal = state.guests();
        let mut ben = journal.get(&ben_id).expect("kept").clone();
        ben.consent.keep = false;
        journal.save(ben, 2).expect("saved");
        state.set_guests(&journal, &[]).expect("kept");
        assert_eq!(state.guests().microphone("Ben"), None);
        assert!(
            state
                .guests()
                .get(&ben_id)
                .expect("still tonight's")
                .microphone
                .is_none()
        );
    }

    /// K3: **a singers' input pulled out mid-song is brought back by itself,
    /// and the rack stays.** Through the real host thread and the null
    /// backend's interface pulled out and plugged back: the host notices the
    /// ring running dry, lets the silent stream go and keeps trying; the
    /// engine holds its two strips all the while; plugged back, a new stream
    /// opens and the ring runs again.
    #[test]
    fn a_lost_singers_input_is_tried_again_until_it_answers() {
        let _guard = input_lock();
        /// Plugs the interface back in however the test ends.
        struct PlugBack;
        impl Drop for PlugBack {
            fn drop(&mut self) {
                dj_audio::null::unplug_inputs(false);
            }
        }
        let _plug = PlugBack;
        let state = AppState::new(true);
        crate::commands::open_device_for(&state, None, None, None).expect("the null device opens");
        let global = |p| state.registry().get(dj_core::ParamId::Global(p));
        use dj_core::param::GlobalParam::{VocalInputs, VocalStarvedFrames};
        let within = |seconds: u64, done: &dyn Fn() -> bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
            while std::time::Instant::now() < deadline {
                if done() {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            false
        };

        state
            .host()
            .open_vocals(None, Vec::new())
            .expect("the null input opens");
        assert!(within(5, &|| (global(VocalInputs) - 2.0).abs() < f32::EPSILON));
        assert_eq!(dj_audio::null::live_input_streams(), 1);

        // Pulled out: the silent stream is let go, and a new one will not open.
        dj_audio::null::unplug_inputs(true);
        assert!(
            within(5, &|| dj_audio::null::live_input_streams() == 0),
            "the host never noticed the input had gone"
        );
        let dry = global(VocalStarvedFrames);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            global(VocalStarvedFrames) > dry,
            "the ring is not running dry"
        );
        assert!(
            (global(VocalInputs) - 2.0).abs() < f32::EPSILON,
            "the rack went with the input"
        );

        // Plugged back: tried again within two seconds, and the ring runs.
        dj_audio::null::unplug_inputs(false);
        assert!(
            within(5, &|| dj_audio::null::live_input_streams() == 1),
            "the input was never tried again"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
        let before = global(VocalStarvedFrames);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let climbed = global(VocalStarvedFrames) - before;
        assert!(
            climbed < 4_800.0,
            "the ring still runs dry ({climbed} frames)"
        );
        assert!((global(VocalInputs) - 2.0).abs() < f32::EPSILON);

        state.host().close_vocals().expect("closes");
        assert!(within(5, &|| dj_audio::null::live_input_streams() == 0));
    }

    /// **A control record actually attaches, end to end.**
    ///
    /// Command to host to device to state to the panel's own words. The engine
    /// side is proven in `dj-engine`'s `rt_safety`; what this covers is the
    /// door -- which, until now, `dj-dvs` did not have.
    #[test]
    fn a_deck_can_be_put_on_a_control_record_and_taken_off_again() {
        let _guard = input_lock();
        let state = AppState::new(true);
        crate::commands::open_device_for(&state, None, None, None).expect("the null device opens");

        let id = DeckId::from_human(2).unwrap();
        let format = pick_format(None).unwrap();
        let config = state
            .host()
            .open_timecode(id, None, format.clone(), true)
            .expect("the null backend can capture");
        state.set_timecode(
            id,
            Some(TimecodeSetup {
                format: format.clone(),
                device: config.device_name,
                absolute: true,
            }),
        );

        let status = status_of(&state);
        let deck = &status.decks[id.index()];
        assert!(deck.running, "deck 2 did not come up on vinyl");
        assert!(
            deck.absolute,
            "absolute mode was asked for and not recorded"
        );
        assert_eq!(deck.format.as_deref(), Some(format.name.as_str()));
        assert!(
            deck.device.is_some(),
            "the panel cannot tell a DJ which input the record is arriving on"
        );

        state.host().close_timecode(id).expect("comes off again");
        state.set_timecode(id, None);
        let after = status_of(&state);
        assert!(!after.decks[id.index()].running);
        assert!(
            after.decks[id.index()].quality < 0.0,
            "a deck taken off vinyl reported {}, which the panel draws as a dead cartridge",
            after.decks[id.index()].quality
        );
    }

    /// **A device change forgets the control records**, because the host has
    /// closed their inputs along with the engine they fed.
    ///
    /// Without this the panel keeps saying deck 1 is on vinyl while the deck
    /// sits on its own transport — the exact shape of bug that made the
    /// microphone go quiet on a device change and told nobody.
    #[test]
    fn opening_a_device_forgets_what_was_on_vinyl() {
        let state = AppState::new(true);
        let id = DeckId::from_human(1).unwrap();
        state.set_timecode(
            id,
            Some(TimecodeSetup {
                format: pick_format(None).unwrap(),
                device: "somewhere".to_owned(),
                absolute: true,
            }),
        );
        assert!(state.timecode(id).is_some());

        crate::commands::open_device_for(&state, None, None, None).expect("the null device opens");
        assert!(
            state.timecode(id).is_none(),
            "the panel would still claim deck 1 follows a record whose input was just closed"
        );
    }
}

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
    let seconds = seconds.unwrap_or_else(|| chosen.unambiguous_seconds());
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err("a control record needs a length".to_owned());
    }
    // Past this the sequence repeats and two places on the record share a
    // position, which is exactly the failure `is_usable` exists to prevent.
    // Capped rather than refused: a DJ asking for an hour wants as much as they
    // can have, not an error.
    let seconds = seconds.min(chosen.unambiguous_seconds());

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
            .chunks_exact(2)
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

    #[test]
    fn a_length_past_the_records_own_period_is_capped_not_refused() {
        let temp = TempWav::new("capped");
        let format = pick_format(None).unwrap();
        let written = write_timecode_signal(
            temp.0.display().to_string(),
            None,
            Some(format.unambiguous_seconds() * 4.0),
            Some(44_100),
        )
        .expect("a long request is answered with what the record can carry");
        assert!(
            written.seconds <= format.unambiguous_seconds() + 0.01,
            "wrote {}s of a record that repeats after {}s",
            written.seconds,
            format.unambiguous_seconds()
        );
    }

    #[test]
    fn a_nonsense_length_is_refused() {
        let temp = TempWav::new("nonsense");
        assert!(
            write_timecode_signal(temp.0.display().to_string(), None, Some(0.0), None).is_err()
        );
        assert!(
            write_timecode_signal(temp.0.display().to_string(), None, Some(f64::NAN), None)
                .is_err()
        );
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
        assert_eq!(
            dj_audio::null::live_input_streams(),
            2,
            "a control record and a microphone are two open captures"
        );

        crate::commands::open_device_for(&state, None, None, None).expect("reconnect");
        assert_eq!(
            dj_audio::null::live_input_streams(),
            0,
            "the old engine went away and its inputs kept running into rings nobody drains"
        );
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

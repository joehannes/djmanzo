//! Tauri commands -- the UI's entire surface.
//!
//! Note how narrow this is. The UI cannot reach the engine, cannot hold a deck,
//! cannot mutate state. It opens a device, loads a file, and sends action
//! strings. Everything else it learns from snapshots.
//!
//! [`dispatch`] takes text (`"deck 1 play"`) rather than a structured payload on
//! purpose: it is the same surface a script or the network API will use, so the
//! parser gets exercised constantly instead of rotting in a corner.

use crate::state::AppState;
use dj_core::{Action, DeckId};
use dj_decode::decode_file;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

/// A device, as the UI sees it.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceDto {
    pub id: String,
    pub name: String,
    pub channels: u16,
    pub sample_rate: u32,
    pub is_default: bool,
    pub supports_split_output: bool,
}

/// The device that is actually open.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveDeviceDto {
    pub name: String,
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub channels: u16,
    pub latency_ms: f64,
    /// The second device carrying the headphone cue, when there is one.
    pub cue: Option<CueDeviceDto>,
    /// Why a requested headphone device was not used. The master still runs;
    /// cueing falls back to the main device if it has the channels for it.
    pub cue_error: Option<String>,
}

/// The headphone device in a two-card setup.
#[derive(Debug, Clone, Serialize)]
pub struct CueDeviceDto {
    pub name: String,
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub latency_ms: f64,
}

/// A track that has just been loaded.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedTrackDto {
    pub deck: u8,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub id: String,
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Result<Vec<DeviceDto>, String> {
    let devices = state.host().list_devices().map_err(|e| e.to_string())?;
    Ok(devices
        .into_iter()
        .map(|d| DeviceDto {
            id: d.id.as_str().to_owned(),
            name: d.name,
            channels: d.max_output_channels,
            sample_rate: d.default_sample_rate.get(),
            is_default: d.is_default,
            supports_split_output: d.max_output_channels >= 4,
        })
        .collect())
}

#[tauri::command]
pub fn open_device(
    state: State<'_, AppState>,
    device_id: Option<String>,
    cue_device_id: Option<String>,
    buffer_frames: Option<u32>,
) -> Result<ActiveDeviceDto, String> {
    let device = device_id.map(dj_audio::DeviceId::new);
    let cue_device = cue_device_id.map(dj_audio::DeviceId::new);
    let frames = buffer_frames.unwrap_or(dj_audio::StreamConfig::DEFAULT_BUFFER_FRAMES);
    let outcome = state
        .host()
        .open(device, cue_device, frames)
        .map_err(|e| e.to_string())?;

    state.set_bridge(outcome.bridge.clone());
    let master = outcome.master;

    let dto = ActiveDeviceDto {
        latency_ms: master.latency_ms(),
        name: master.device_name,
        sample_rate: master.sample_rate.get(),
        buffer_frames: master.buffer_frames,
        channels: master.channels,
        cue: outcome.cue.map(|cue| CueDeviceDto {
            latency_ms: cue.latency_ms(),
            name: cue.device_name,
            sample_rate: cue.sample_rate.get(),
            buffer_frames: cue.buffer_frames,
        }),
        cue_error: outcome.cue_error,
    };
    // Kept, so an interface that did not make this call can still find out. See
    // `AppState::set_active_device`.
    state.set_active_device(Some(dto.clone()));
    Ok(dto)
}

/// What is open, if anything.
///
/// Asked once on startup and again whenever the interface finds itself with a
/// running engine it did not start — which happens whenever something other
/// than the Connect button opens a device. Not part of the 60 Hz snapshot: a
/// device changes on connect and never in between.
#[tauri::command]
#[must_use]
pub fn active_device(state: State<'_, AppState>) -> Option<ActiveDeviceDto> {
    state.active_device()
}

#[tauri::command]
pub fn start_audio(state: State<'_, AppState>) -> Result<(), String> {
    state.host().play().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_audio(state: State<'_, AppState>) -> Result<(), String> {
    state.host().pause().map_err(|e| e.to_string())
}

/// Decode a file and put it on a deck.
///
/// Decoding is slow -- minutes of audio, plus a content hash -- so it runs on a
/// blocking worker rather than on the UI thread or, catastrophically, the audio
/// thread. Only the finished `Arc` crosses into the engine.
#[tauri::command]
pub async fn load_track(
    state: State<'_, AppState>,
    deck: u8,
    path: String,
) -> Result<LoadedTrackDto, String> {
    let deck_id = dj_core::DeckId::from_human(deck).ok_or_else(|| format!("no deck {deck}"))?;

    let decoded = tauri::async_runtime::spawn_blocking(move || decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let dto = LoadedTrackDto {
        deck,
        title: decoded.display_title(),
        artist: decoded.artist.clone(),
        album: decoded.album.clone(),
        duration_seconds: decoded.buffer.duration_seconds(),
        sample_rate: decoded.buffer.sample_rate().get(),
        id: decoded.id.to_hex(),
    };

    // Summarise for the waveform on the same worker that decoded. Doing it here
    // rather than lazily means the strip is drawable the moment the track
    // appears, instead of popping in a second later.
    let summary = dj_render::WaveformSummary::analyse(
        decoded.buffer.as_interleaved(),
        decoded.buffer.sample_rate(),
    );
    state.waveforms().set_summary(deck_id, summary);

    // Drop the previous track's numbers *before* the new audio starts playing,
    // so the header never shows one track's BPM against another's waveform.
    state.analysis().clear_deck(deck_id);
    state.set_deck_track(
        deck_id,
        crate::state::LoadedTrackInfo {
            title: dto.title.clone(),
            artist: dto.artist.clone(),
            id: decoded.id,
        },
    );

    let track_id = decoded.id;
    let sample_rate = decoded.buffer.sample_rate();

    // Into the library *before* the deck is playable.
    //
    // Two reasons for the ordering. A cue row has a foreign key to its track,
    // so without this every cue a DJ set on a file they opened from disk would
    // be silently discarded. And a DJ who loads a file expects to find it in
    // their collection afterwards -- a track you played is part of your library
    // whether or not you ever pointed a scan at the folder it lives in.
    remember_track(&state, &decoded, sample_rate);

    let buffer = Arc::new(decoded.buffer);
    // One allocation, two owners: the engine plays it, the analyser reads it.
    // Cloning the samples instead would double a hundred megabytes for no reason.
    let source: Arc<dyn dj_decode::TrackSource> = buffer.clone();
    state
        .bus()
        .send_command(dj_engine::Command::Load {
            deck: deck_id,
            source,
        })
        .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())?;

    // What this track had last time it was played: cues in the slots they were
    // in, and the grid as it was left -- corrected by hand, if it was. Sent
    // after the load so the engine applies them to the new track rather than to
    // whatever was on the deck a moment ago.
    restore_deck_state(&state, deck_id, track_id, sample_rate);

    // Analysis runs *after* the track is playable, on its own worker.
    //
    // The ordering is the whole point: tempo, key and loudness take FFT passes
    // over the entire file, and making the load wait for them would mean a DJ
    // reaching for the next track and getting a frozen window. The deck is
    // usable immediately; the numbers arrive a moment later.
    let store = Arc::clone(state.analysis());
    let bus = Arc::clone(state.bus());
    let waveforms = Arc::clone(state.waveforms());
    let tracks = state.deck_tracks();
    let library = state.library();
    tauri::async_runtime::spawn_blocking(move || {
        let analysis = crate::analysis::analyse_or_cached(
            &store,
            deck_id,
            track_id,
            buffer.as_interleaved(),
            sample_rate,
        );

        // Analysis takes seconds on a long file, and a DJ can load two tracks
        // onto one deck in that time. Without this check the first track's beat
        // grid, and its auto-gain, would land on the second -- a wrong grid
        // under a mix, arriving from nowhere a few seconds after the load.
        let still_loaded = tracks
            .lock()
            .map(|map| {
                map.get(&deck_id.human_number())
                    .is_some_and(|t| t.id == track_id)
            })
            .unwrap_or(false);
        if !still_loaded {
            return;
        }

        // The grid goes to the waveform store, which draws it into the tiles
        // themselves. Drawing it in the interface instead would be two
        // coordinate systems agreeing only by luck -- see `dj_render::GridOverlay`.
        waveforms.set_analysed_grid(
            deck_id,
            analysis.tempo.as_ref().map(|tempo| dj_render::GridOverlay {
                grid: tempo.grid,
                sample_rate,
            }),
        );

        // And into the library, so a track loaded straight from disk gets a BPM
        // and a key the browser can sort by without waiting for a scan to reach
        // it. `if_absent`, so this cannot overwrite a grid the DJ corrected
        // last time they played it -- which `restore_deck_state` has already
        // put back on the deck.
        if let Ok(db) = library.get()
            && let Err(error) =
                db.set_analysis_if_absent(track_id, &crate::library::stored_analysis(&analysis))
        {
            tracing::warn!(%error, "could not store the analysis");
        }

        // And to the engine, which needs it for sync, quantize and beat jump.
        // Two destinations for one finding rather than one shared home,
        // because the engine's copy has to cross a lock-free queue into the
        // audio thread and the renderer's cannot.
        let _ = bus.send_command(dj_engine::Command::SetGrid {
            deck: deck_id,
            grid: analysis.tempo.as_ref().map(|tempo| tempo.grid),
        });

        // Auto-gain goes through the action bus rather than straight to the
        // engine, so it lands in the session log like any other trim change and
        // the DJ can see -- and undo -- what was done on their behalf.
        if let Some(action) = crate::analysis::auto_gain_action(deck_id, &analysis)
            && let Ok(parsed) = dj_core::Action::parse(&action)
        {
            let _ = bus.send_command(dj_engine::Command::Action(parsed));
        }
    });

    Ok(dto)
}

/// Send an action, in its text form.
///
/// ```text
/// dispatch("deck 1 play")
/// dispatch("deck 2 volume 0.8")
/// dispatch("crossfader -0.5")
/// ```
#[tauri::command]
pub fn dispatch(state: State<'_, AppState>, action: String) -> Result<(), String> {
    let parsed = Action::parse(&action).map_err(|e| format!("{action:?}: {e}"))?;

    // Eject is the one action with consequences outside the engine: the deck's
    // name and its analysis live here, not there, and leaving them behind would
    // show a track that is no longer loaded.
    if let Action::Deck {
        deck,
        action: dj_core::DeckAction::Eject,
    } = parsed
    {
        state.clear_deck_track(deck);
        state.analysis().clear_deck(deck);
        state.taps().clear(deck);
    }

    // Saved loops live in the library with the track, so the host is the only
    // place that can look one up or store one. Intercepted for the same reason
    // as the grid edits below.
    if let Action::Deck { deck, action } = parsed {
        match action {
            dj_core::DeckAction::LoopSave(slot) => {
                save_loop(&state, deck, slot)?;
                let _ = state.bus().dispatch(parsed);
                return Ok(());
            }
            dj_core::DeckAction::LoopRecall(slot) => {
                recall_loop(&state, deck, slot)?;
                let _ = state.bus().dispatch(parsed);
                return Ok(());
            }
            _ => {}
        }
    }

    // Grid edits are the other kind: they need the analyser's original to undo
    // to and a tap history to average, neither of which belongs on the audio
    // thread. Computed here and sent on as `SetGrid`, which is the same path
    // the analyser's own result takes.
    if let Action::Deck { deck, action } = parsed
        && let Some(edit) = grid_edit(action)
    {
        apply_grid_edit(&state, deck, edit)?;
        // Dispatched *after* the edit has succeeded, so it lands in the session
        // log like every other action -- a grid the DJ moved mid-set is exactly
        // the kind of thing worth being able to look back at. The engine
        // ignores the action itself; it has already had the result as
        // `SetGrid`. A refused edit is not logged, because it did not happen.
        let _ = state.bus().dispatch(parsed);
        return Ok(());
    }

    state
        .bus()
        .dispatch(parsed)
        .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())
}

/// The grid edits, separated from the actions the engine handles itself.
#[derive(Debug, Clone, Copy)]
enum GridEdit {
    AnchorHere,
    Nudge(f64),
    Scale(f64),
    SetBpm(f64),
    Tap,
    Reset,
}

fn grid_edit(action: dj_core::DeckAction) -> Option<GridEdit> {
    use dj_core::DeckAction as A;
    Some(match action {
        A::GridAnchorHere => GridEdit::AnchorHere,
        A::GridNudge(ms) => GridEdit::Nudge(ms),
        A::GridScale(x) => GridEdit::Scale(x),
        A::GridSetBpm(b) => GridEdit::SetBpm(b),
        A::GridTap => GridEdit::Tap,
        A::GridReset => GridEdit::Reset,
        _ => return None,
    })
}

fn apply_grid_edit(state: &AppState, deck: DeckId, edit: GridEdit) -> Result<(), String> {
    use crate::grid;

    let waveforms = state.waveforms();
    let registry = state.registry();

    // Reset is the one edit that does not need an existing grid to work from --
    // and it is also how a deck whose grid was cleared gets the analyser's back.
    if let GridEdit::Reset = edit {
        let original = waveforms.analysed_grid(deck.human_number());
        waveforms.set_grid(deck, original);
        save_grid(state, deck, original.map(|o| o.grid));
        return publish_grid(state, deck, original.map(|o| o.grid));
    }

    // The playhead, read live rather than from the last snapshot: a tap is
    // timed against the music, and a snapshot can be up to 16 ms stale.
    //
    // The registry is `f32`, so past about 16.7M frames -- six minutes at
    // 48 kHz -- consecutive frames stop being distinguishable, and at the end
    // of a ten-minute track the granularity is two frames. That is 0.04 ms on
    // an anchor and 0.008% on a tapped tempo: below the width of a drawn line
    // and far below a human's tapping jitter, so it is stated rather than
    // engineered around.
    let position = dj_core::FramePos::new(f64::from(registry.get(dj_core::ParamId::Deck(
        deck,
        dj_core::param::DeckParam::Position,
    ))));

    let existing = waveforms.grid(deck.human_number());
    // The sample rate of the *track*, which is what the grid is measured in.
    // Falls back to the device's, which is what an unloaded deck reports.
    let rate = existing
        .map(|o| o.sample_rate)
        .or_else(|| {
            let hz = registry.get(dj_core::ParamId::Global(
                dj_core::param::GlobalParam::SampleRate,
            ));
            dj_core::SampleRate::new(hz as u32)
        })
        .ok_or("no sample rate yet; open a device first")?;

    let edited =
        match edit {
            GridEdit::Tap => {
                let bars = existing.map_or(4, |o| o.grid.beats_per_bar);
                match state.taps().tap(deck, position, rate, bars) {
                    grid::Tap::Grid(g) => g,
                    // A first tap is not a failure -- it is half of the gesture.
                    grid::Tap::Started => return Ok(()),
                    grid::Tap::Unusable => {
                        return Err("those taps are not a playable tempo".to_owned());
                    }
                }
            }
            _ => {
                let current = existing
                    .map(|o| o.grid)
                    .ok_or("no beat grid on this deck yet; wait for analysis or tap one in")?;
                match edit {
                    GridEdit::AnchorHere => grid::anchor_here(current, position),
                    GridEdit::Nudge(ms) => grid::nudge(current, ms, rate),
                    GridEdit::Scale(x) => grid::scale(current, x)
                        .ok_or("that would leave the playable tempo range")?,
                    GridEdit::SetBpm(b) => grid::set_bpm(current, b)
                        .ok_or("that tempo is outside the playable range")?,
                    GridEdit::Tap | GridEdit::Reset => unreachable!("handled above"),
                }
            }
        };

    waveforms.set_grid(
        deck,
        Some(dj_render::GridOverlay {
            grid: edited,
            sample_rate: rate,
        }),
    );
    save_grid(state, deck, Some(edited));
    publish_grid(state, deck, Some(edited))
}

/// Keep a grid edit, so the correction is still there next time this track is
/// played.
///
/// Unconditional, unlike the write identification does: this grid is what the
/// DJ said, and it outranks whatever the analyser had found. Reset writes the
/// analyser's original back, which is exactly right -- undoing an edit should
/// be as durable as making one.
///
/// Synchronous rather than through the writer thread, because this is already
/// off the interface thread and a DJ who edits a grid and quits immediately
/// should not lose it to a queue that never drained.
fn save_grid(state: &AppState, deck: DeckId, grid: Option<dj_core::Beatgrid>) {
    let Some(track) = state.deck_track_id(deck) else {
        return;
    };
    let Ok(db) = state.library().get() else {
        return;
    };

    let stored = match db.track(track) {
        Ok(Some(found)) => found.analysis,
        // No row means the track is not in the library, which the load path
        // should have seen to. Nothing to attach the grid to.
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, "could not read the track to save its grid");
            return;
        }
    };

    let updated = match grid {
        // Marked as the DJ's own, which is what stops an import — or a
        // re-analysis — replacing it later. See `dj_library::GridSource`.
        Some(grid) => stored
            .with_beatgrid(grid)
            .from_source(dj_library::GridSource::Manual),
        // A cleared grid, which only `grid_reset` on an unanalysed track can
        // produce. Blank the four columns rather than leaving a stale tempo.
        None => dj_library::StoredAnalysis {
            bpm: None,
            grid_anchor: None,
            grid_beats_per_bar: None,
            grid_confidence: None,
            grid_source: None,
            ..stored
        },
    };
    if let Err(error) = db.set_analysis(track, &updated) {
        tracing::warn!(%error, "could not save the grid edit");
    }
}

/// Send a grid to the engine, which needs it for sync, quantize and beat jump.
fn publish_grid(
    state: &AppState,
    deck: DeckId,
    grid: Option<dj_core::Beatgrid>,
) -> Result<(), String> {
    state
        .bus()
        .send_command(dj_engine::Command::SetGrid { deck, grid })
        .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())
}

/// Read the current state directly.
///
/// The snapshot stream only emits on change (plus a slow heartbeat), so a UI
/// that has just mounted needs one synchronous read to paint itself rather than
/// waiting for the engine to do something.
#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> crate::Snapshot {
    let bridge = state.bridge();
    let tracks = state.deck_tracks();
    crate::Snapshot::capture_all(
        &state.registry(),
        state.deck_count(),
        bridge.as_deref(),
        Some(state.analysis()),
        Some(&tracks),
    )
}

/// What the interface needs to size a deck's waveform strip.
#[derive(Debug, Clone, Serialize)]
pub struct WaveformInfo {
    pub deck: u8,
    pub ready: bool,
    pub total_frames: u64,
    /// Generation of this deck's tiles. Goes into every tile URL so the
    /// webview's own cache misses when the content changes -- see
    /// `WaveformStore::epochs`.
    pub epoch: u32,
}

#[tauri::command]
pub fn waveform_info(state: State<'_, AppState>, deck: u8) -> WaveformInfo {
    WaveformInfo {
        deck,
        ready: state.waveforms().has_summary(deck),
        total_frames: state.waveforms().total_frames(deck).unwrap_or(0) as u64,
        epoch: state.waveforms().epoch(deck),
    }
}

/// Report a frame-timing measurement from the webview.
///
/// The webview is the only place that can measure its own compositing, so the
/// numbers come back out this way to be logged where they can be read.
#[tauri::command]
pub fn report_bench(label: String, fps: f64, p50_ms: f64, p95_ms: f64, worst_ms: f64) {
    tracing::info!(
        target: "bench",
        "{label}: {fps:.1} fps | p50 {p50_ms:.2} ms | p95 {p95_ms:.2} ms | worst {worst_ms:.2} ms"
    );
    println!(
        "BENCH {label}: {fps:.1} fps | p50 {p50_ms:.2} ms | p95 {p95_ms:.2} ms | worst {worst_ms:.2} ms"
    );
}

/// The session so far, as replayable text.
///
/// This is the action log from `ADR-0003` made visible. In M0 it exists to prove
/// the log is real and ordered; M8 turns it into replay and offline re-render.
#[tauri::command]
pub fn session_log(state: State<'_, AppState>) -> Vec<String> {
    state
        .bus()
        .log()
        .into_iter()
        .map(|entry| format!("{:>8.3}  {}", entry.at.as_secs_f64(), entry.action))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command layer's real contract is that it speaks action text. Verify
    /// the parse-and-dispatch path against the bus directly, since spinning up
    /// Tauri's `State` in a unit test is not worth the machinery.
    #[test]
    fn action_text_from_the_ui_parses() {
        for text in [
            "deck 1 play",
            "deck 2 pause",
            "deck 1 play_pause",
            "deck 3 cue",
            "deck 1 volume 0.8",
            "deck 2 pitch 0.04",
            "deck 1 gain -3",
            "deck 4 eject",
            "crossfader -0.5",
            "master gain -6",
        ] {
            assert!(
                Action::parse(text).is_ok(),
                "the UI sends {text:?}, which must parse"
            );
        }
    }

    #[test]
    fn malformed_action_text_is_reported_not_ignored() {
        assert!(Action::parse("deck 9 play").is_err());
        assert!(Action::parse("deck 1 explode").is_err());
        assert!(Action::parse("").is_err());
    }

    #[test]
    fn session_log_entries_are_formatted_for_reading() {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();
        state
            .bus()
            .dispatch(Action::parse("deck 1 play").unwrap())
            .unwrap();

        let log = state.bus().log();
        assert_eq!(log.len(), 1);
        let rendered = format!("{:>8.3}  {}", log[0].at.as_secs_f64(), log[0].action);
        assert!(rendered.contains("deck 1 play"), "got {rendered:?}");
    }
}

/// The wiring between a grid-edit action and the two places a grid lives.
///
/// [`crate::grid`] tests the arithmetic. These test that the edit reaches the
/// renderer *and* the engine, that it survives a round trip, and that the
/// failures are reported rather than swallowed -- which is the part that would
/// silently break.
#[cfg(test)]
mod grid_edit_tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos, SampleRate};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn deck() -> DeckId {
        DeckId::from_human(1).unwrap()
    }

    /// An app with a device open and a weak grid on deck 1, which is the state
    /// a DJ is in when they reach for these controls.
    fn app_with_grid(bpm: f64, anchor: f64) -> AppState {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();
        state.waveforms().set_analysed_grid(
            deck(),
            Some(dj_render::GridOverlay {
                grid: Beatgrid::new(
                    FramePos::new(anchor),
                    Bpm::new(bpm).unwrap(),
                    Confidence::new(0.2),
                ),
                sample_rate: SR,
            }),
        );
        state
    }

    fn current(state: &AppState) -> Beatgrid {
        state
            .waveforms()
            .grid(deck().human_number())
            .expect("deck 1 has a grid")
            .grid
    }

    /// What `dispatch` does, minus Tauri's `State` wrapper -- which is the one
    /// thing in that function a unit test cannot build.
    fn dispatch_for_test(state: &AppState, text: &str) -> Result<(), String> {
        let parsed = Action::parse(text).map_err(|e| format!("{text:?}: {e}"))?;
        let Action::Deck { deck, action } = parsed else {
            panic!("{text} is not a deck action");
        };
        let edit = grid_edit(action).expect("not a grid edit");
        apply_grid_edit(state, deck, edit)?;
        let _ = state.bus().dispatch(parsed);
        Ok(())
    }

    fn edit(state: &AppState, text: &str) -> Result<(), String> {
        let Action::Deck { deck, action } = Action::parse(text).unwrap() else {
            panic!("{text} is not a deck action");
        };
        let edit = grid_edit(action).expect("{text} must be a grid edit");
        apply_grid_edit(state, deck, edit)
    }

    #[test]
    fn every_grid_verb_the_interface_sends_parses_and_is_recognised_as_an_edit() {
        for text in [
            "deck 1 grid_here",
            "deck 1 grid_nudge -10",
            "deck 1 grid_nudge 10",
            "deck 1 grid_scale 0.5",
            "deck 1 grid_scale 2",
            "deck 1 grid_bpm 128",
            "deck 1 grid_tap",
            "deck 1 grid_reset",
        ] {
            let Action::Deck { action, .. } = Action::parse(text).unwrap() else {
                panic!("{text} is not a deck action");
            };
            assert!(
                grid_edit(action).is_some(),
                "{text} parses but is not routed as a grid edit, so it would go \
                 to the engine and be silently ignored"
            );
        }
    }

    /// Ordinary actions must *not* be diverted.
    #[test]
    fn actions_that_are_not_grid_edits_go_to_the_engine() {
        for text in ["deck 1 play", "deck 1 beatjump 4", "deck 1 loop 4"] {
            let Action::Deck { action, .. } = Action::parse(text).unwrap() else {
                panic!("{text} is not a deck action");
            };
            assert!(grid_edit(action).is_none(), "{text} must reach the engine");
        }
    }

    /// Grid edits reach the session log. They are diverted before the engine,
    /// so it would be easy for them to skip the log with it -- and a grid moved
    /// mid-set is exactly what a DJ wants to find when reading back the night.
    #[test]
    fn a_successful_edit_is_recorded_in_the_session_log() {
        let state = app_with_grid(128.0, 10_000.0);
        state.bus().clear_log();
        dispatch_for_test(&state, "deck 1 grid_nudge 10").unwrap();
        let log = state.bus().log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].action.to_string(), "deck 1 grid_nudge 10");
    }

    /// The other half: an edit that was refused must not appear to have
    /// happened.
    #[test]
    fn a_refused_edit_is_not_recorded() {
        let state = app_with_grid(128.0, 0.0);
        state.bus().clear_log();
        assert!(dispatch_for_test(&state, "deck 1 grid_scale 4").is_err());
        assert!(state.bus().log().is_empty());
    }

    #[test]
    fn nudging_moves_the_renderers_grid() {
        let state = app_with_grid(128.0, 10_000.0);
        edit(&state, "deck 1 grid_nudge 10").unwrap();
        let expected = 10_000.0 + 10.0 / 1000.0 * SR.as_f64();
        assert!((current(&state).anchor.get() - expected).abs() < 1e-6);
    }

    /// The point of editing: a grid the analyser doubted becomes one sync will
    /// accept.
    #[test]
    fn an_edit_makes_the_grid_trustworthy() {
        let state = app_with_grid(128.0, 10_000.0);
        assert!(!current(&state).confidence.is_sync_worthy());
        edit(&state, "deck 1 grid_here").unwrap();
        assert!(current(&state).confidence.is_sync_worthy());
    }

    #[test]
    fn reset_goes_back_to_what_the_analyser_found() {
        let state = app_with_grid(128.0, 10_000.0);
        edit(&state, "deck 1 grid_scale 2").unwrap();
        edit(&state, "deck 1 grid_nudge 50").unwrap();
        assert!((current(&state).bpm.get() - 256.0).abs() < 1e-9);

        edit(&state, "deck 1 grid_reset").unwrap();
        let back = current(&state);
        assert!((back.bpm.get() - 128.0).abs() < 1e-9);
        assert!((back.anchor.get() - 10_000.0).abs() < 1e-9);
        assert_eq!(
            back.confidence,
            Confidence::new(0.2),
            "reset must restore the analyser's doubt too, not just its numbers"
        );
    }

    /// Editing has to move the tile generation on, or the webview keeps showing
    /// the old grid from its own cache and the edit appears to do nothing.
    #[test]
    fn editing_invalidates_the_tiles() {
        let state = app_with_grid(128.0, 10_000.0);
        let before = state.waveforms().epoch(deck().human_number());
        edit(&state, "deck 1 grid_nudge 5").unwrap();
        assert_ne!(
            state.waveforms().epoch(deck().human_number()),
            before,
            "the webview caches tiles for a year; without a new epoch the edit is invisible"
        );
    }

    #[test]
    fn an_out_of_range_tempo_is_reported_rather_than_clamped() {
        let state = app_with_grid(128.0, 0.0);
        assert!(edit(&state, "deck 1 grid_scale 4").is_err());
        assert!(
            (current(&state).bpm.get() - 128.0).abs() < 1e-9,
            "a refused edit must leave the grid alone"
        );
    }

    /// A deck the analyser could not read has no grid to modify -- but it can
    /// still be tapped in, which is the whole reason tap exists.
    #[test]
    fn editing_a_deck_with_no_grid_says_so_but_tapping_still_works() {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();

        assert!(edit(&state, "deck 1 grid_nudge 10").is_err());
        // First tap: accepted, nothing to report yet.
        edit(&state, "deck 1 grid_tap").unwrap();
    }

    /// Tapping twice at the same playhead position is not a tempo. The deck is
    /// paused in this test, so both taps land on frame 0.
    #[test]
    fn tapping_a_paused_deck_never_invents_a_tempo() {
        let state = app_with_grid(128.0, 10_000.0);
        edit(&state, "deck 1 grid_tap").unwrap();
        edit(&state, "deck 1 grid_tap").unwrap();
        assert!(
            (current(&state).bpm.get() - 128.0).abs() < 1e-9,
            "two taps at the same position must not change the grid"
        );
    }
}

// -- the library -----------------------------------------------------------

/// One track as the browser shows it.
///
/// Flat and pre-formatted. The interface should not be doing arithmetic on
/// frames or looking up Camelot letters — it renders a table, and a table of
/// four hundred rows re-deriving the same values on every keystroke is how a
/// browser stops feeling instant.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryTrackDto {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub duration_seconds: f64,
    pub bpm: Option<f64>,
    /// Camelot notation, which is what a DJ mixes by.
    pub key: Option<String>,
    pub loudness_lufs: Option<f64>,
    /// True once the track has everything sync and harmonic mixing need.
    pub analysed: bool,
    pub play_count: i64,
    /// 0..=5, when the DJ has rated it.
    pub rating: Option<u8>,
    /// `#rrggbb`, when the DJ has coloured it.
    pub colour: Option<String>,
}

impl From<dj_library::LibraryTrack> for LibraryTrackDto {
    fn from(track: dj_library::LibraryTrack) -> Self {
        Self {
            id: track.id.to_hex(),
            path: track.path.to_string_lossy().into_owned(),
            title: track.display_title(),
            artist: track.display_artist().to_owned(),
            album: track.tags.album.clone(),
            genre: track.tags.genre.clone(),
            year: track.tags.year,
            duration_seconds: track.duration_seconds(),
            bpm: track.analysis.bpm,
            key: track.analysis.key().map(|k| k.camelot()),
            loudness_lufs: track.analysis.loudness_lufs,
            analysed: track.analysis.is_complete(),
            play_count: track.stats.play_count,
            rating: track.stats.rating,
            colour: track.colour.clone(),
        }
    }
}

/// How the collection is doing.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryStatusDto {
    pub tracks: i64,
    /// Files scanned but not yet identified.
    pub pending: i64,
    /// Files that could not be identified, with the reason.
    pub failed: Vec<FailedFileDto>,
    pub folders: Vec<String>,
    /// Identified since the application started.
    pub identified: usize,
    /// True while a file is actually being decoded.
    pub working: bool,
    /// Where the database lives, or `None` when it is in memory only — which
    /// means everything here is lost on restart, and the interface says so.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedFileDto {
    pub path: String,
    pub reason: String,
}

fn library(state: &AppState) -> Result<Arc<dj_library::Library>, String> {
    state.library().get().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_status(state: State<'_, AppState>) -> Result<LibraryStatusDto, String> {
    let db = library(&state)?;
    let progress = state.identify_progress();
    Ok(LibraryStatusDto {
        tracks: db.track_count().map_err(|e| e.to_string())?,
        pending: db.pending_count().map_err(|e| e.to_string())?,
        failed: db
            .failed_pending()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(path, reason)| FailedFileDto {
                path: path.to_string_lossy().into_owned(),
                reason,
            })
            .collect(),
        folders: db
            .folders()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        identified: progress
            .as_ref()
            .map_or(0, |p| p.done.load(std::sync::atomic::Ordering::Relaxed)),
        working: progress
            .as_ref()
            .is_some_and(|p| p.working.load(std::sync::atomic::Ordering::Relaxed)),
        path: state
            .library()
            .path()
            .map(|p| p.to_string_lossy().into_owned()),
    })
}

/// Watch a folder and walk it now.
///
/// Synchronous, because walking is the cheap half: tags and directory entries,
/// no decoding. A large collection takes seconds, not the hours identification
/// takes — see `dj_library::scan`.
#[tauri::command]
pub async fn library_add_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<LibraryScanDto, String> {
    let db = library(&state)?;
    let now = crate::library::now_seconds();
    let folder = PathBuf::from(&path);

    db.add_folder(&folder, now).map_err(|e| e.to_string())?;
    let report =
        tauri::async_runtime::spawn_blocking(move || dj_library::scan_folder(&db, &folder, now))
            .await
            .map_err(|e| format!("scan task failed: {e}"))?
            .map_err(|e| e.to_string())?;

    Ok(report.into())
}

#[tauri::command]
pub fn library_remove_folder(state: State<'_, AppState>, path: String) -> Result<(), String> {
    library(&state)?
        .remove_folder(Path::new(&path))
        .map_err(|e| e.to_string())
}

/// Re-walk every watched folder.
#[tauri::command]
pub async fn library_rescan(state: State<'_, AppState>) -> Result<LibraryScanDto, String> {
    let db = library(&state)?;
    let now = crate::library::now_seconds();
    let report = tauri::async_runtime::spawn_blocking(move || dj_library::scan_all(&db, now))
        .await
        .map_err(|e| format!("scan task failed: {e}"))?
        .map_err(|e| e.to_string())?;
    Ok(report.into())
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryScanDto {
    pub found: usize,
    pub added: usize,
    pub unchanged: usize,
    pub unreadable_dirs: usize,
    pub untaggable: usize,
}

impl From<dj_library::ScanReport> for LibraryScanDto {
    fn from(report: dj_library::ScanReport) -> Self {
        Self {
            found: report.found,
            added: report.added,
            unchanged: report.unchanged,
            unreadable_dirs: report.unreadable_dirs,
            untaggable: report.untaggable,
        }
    }
}

/// How many rows the browser asks for at once.
///
/// A DJ scrolling a 50,000-track collection does not read 50,000 rows, and
/// serialising them all through IPC on every keystroke is what makes a browser
/// feel slow. Five hundred is more than a screen holds at any zoom.
const BROWSE_LIMIT: usize = 500;

/// Search the library, or list it when the query is empty.
#[tauri::command]
pub fn library_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<LibraryTrackDto>, String> {
    let db = library(&state)?;
    let found = if query.trim().is_empty() {
        db.all_tracks(BROWSE_LIMIT)
    } else {
        db.search(&query, BROWSE_LIMIT)
    }
    .map_err(|e| e.to_string())?;
    Ok(found.into_iter().map(LibraryTrackDto::from).collect())
}

/// Add a freshly decoded track to the library.
///
/// Tags come from the decoder here rather than from `lofty`, because this is
/// the load path and the file has already been opened once. A scan reads richer
/// tags; if this track is later scanned, the upsert fills in the rest.
fn remember_track(state: &AppState, decoded: &dj_decode::DecodedTrack, rate: dj_core::SampleRate) {
    let Ok(db) = state.library().get() else {
        return;
    };
    let track = dj_library::LibraryTrack {
        id: decoded.id,
        path: decoded.path.clone(),
        tags: dj_library::Tags {
            title: decoded.title.clone(),
            artist: decoded.artist.clone(),
            album: decoded.album.clone(),
            ..dj_library::Tags::default()
        },
        duration_frames: decoded.buffer.len_frames() as u64,
        sample_rate: rate,
        channels: 2,
        file_size: std::fs::metadata(&decoded.path).ok().map(|m| m.len()),
        file_modified: None,
        added_at: crate::library::now_seconds(),
        analysis: dj_library::StoredAnalysis::default(),
        stats: dj_library::PlayStats::default(),
        colour: None,
    };
    if let Err(error) = db.upsert_track(&track) {
        // Not fatal. The deck still plays; the DJ just will not find this track
        // in the browser, and cues set on it will not be kept.
        tracing::warn!(%error, path = ?decoded.path, "could not add the track to the library");
    }
}

/// Put a track's stored cues and grid back on the deck it has just been loaded
/// onto.
fn restore_deck_state(
    state: &AppState,
    deck: DeckId,
    track: dj_core::TrackId,
    rate: dj_core::SampleRate,
) {
    let Ok(db) = state.library().get() else {
        return;
    };

    // The watcher must not treat the restored cues as a change the DJ made --
    // and must not treat the deck's *current* cues, which still belong to the
    // previous track, as this one's. Forgetting the deck makes the next
    // observation a first sight.
    state.cue_watcher_forget(deck.human_number());

    match db.cues(track) {
        Ok(cues) if !cues.is_empty() => {
            let _ = state.bus().send_command(dj_engine::Command::SetHotCues {
                deck,
                cues: crate::persist::from_stored(&cues),
            });
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "could not read stored cues"),
    }

    // A stored grid wins over the analyser's, which has not run yet anyway --
    // and which, when it does, will not overwrite this: `analyse_or_cached`
    // only publishes a grid for a deck that has none stored.
    match db.track(track) {
        Ok(Some(found)) => {
            if let Some(grid) = found.analysis.beatgrid() {
                state.waveforms().set_analysed_grid(
                    deck,
                    Some(dj_render::GridOverlay {
                        grid,
                        sample_rate: rate,
                    }),
                );
                let _ = state.bus().send_command(dj_engine::Command::SetGrid {
                    deck,
                    grid: Some(grid),
                });
            }
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "could not read the stored grid"),
    }
}

/// Keep the loop that is playing, with the track.
///
/// Reads the region from the registry rather than from the action, because the
/// engine is what decided it: `loop_in`/`loop_out` snap to the grid when
/// quantize is on, and the loop the DJ can hear is the snapped one.
fn save_loop(state: &AppState, deck: DeckId, slot: u8) -> Result<(), String> {
    use dj_core::param::DeckParam;

    let registry = state.registry();
    let get = |param| registry.get(dj_core::ParamId::Deck(deck, param));
    if get(DeckParam::LoopActive) < 0.5 {
        return Err("there is no loop playing to save".to_owned());
    }

    let track = state
        .deck_track_id(deck)
        .ok_or("no track on that deck to save a loop with")?;
    let db = state.library().get().map_err(|e| e.to_string())?;

    let mut loops = db.loops(track).map_err(|e| e.to_string())?;
    loops.retain(|region| region.slot != slot);
    loops.push(dj_library::StoredLoop {
        slot,
        start_frame: f64::from(get(DeckParam::LoopStart)),
        end_frame: f64::from(get(DeckParam::LoopEnd)),
        label: None,
    });
    loops.sort_by_key(|region| region.slot);

    db.set_loops(track, &loops).map_err(|e| e.to_string())
}

/// Put a saved loop back on the deck.
fn recall_loop(state: &AppState, deck: DeckId, slot: u8) -> Result<(), String> {
    let track = state
        .deck_track_id(deck)
        .ok_or("no track on that deck to recall a loop for")?;
    let db = state.library().get().map_err(|e| e.to_string())?;

    let stored = db
        .loops(track)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|region| region.slot == slot)
        .ok_or_else(|| format!("nothing saved in loop slot {slot}"))?;

    // `LoopRegion::new` rejects a reversed or empty span. A row like that means
    // the database is wrong, and looping over nothing would be worse than
    // saying so.
    let region = dj_core::LoopRegion::new(
        dj_core::FramePos::new(stored.start_frame),
        dj_core::FramePos::new(stored.end_frame),
    )
    .ok_or_else(|| format!("saved loop {slot} is not a region"))?;

    state
        .bus()
        .send_command(dj_engine::Command::SetLoop {
            deck,
            region: Some(region),
        })
        .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())
}

/// Cues, grids and loops surviving a track leaving a deck and coming back.
///
/// The unit tests cover the pieces. These cover the claim a DJ actually cares
/// about: what you set on a record is still there next time you play it.
#[cfg(test)]
mod persistence_tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos, SampleRate};

    const SR: SampleRate = SampleRate::DEFAULT;

    fn deck() -> DeckId {
        DeckId::from_human(1).unwrap()
    }

    fn id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    /// An app with a device open and a track "on" deck 1: in the library, and
    /// recorded as loaded, which is what the persistence paths key off.
    fn app_with_track() -> AppState {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();

        let db = state.library().get().unwrap();
        db.upsert_track(&dj_library::LibraryTrack {
            id: id(1),
            path: std::path::PathBuf::from("/music/a.flac"),
            tags: dj_library::Tags::default(),
            duration_frames: 48_000 * 200,
            sample_rate: SR,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: dj_library::StoredAnalysis::default(),
            stats: dj_library::PlayStats::default(),
            colour: None,
        })
        .unwrap();

        state.set_deck_track(
            deck(),
            crate::state::LoadedTrackInfo {
                title: "A".to_owned(),
                artist: None,
                id: id(1),
            },
        );
        state
    }

    fn grid_on_deck(state: &AppState, grid: Beatgrid) {
        state.waveforms().set_analysed_grid(
            deck(),
            Some(dj_render::GridOverlay {
                grid,
                sample_rate: SR,
            }),
        );
    }

    fn edit(state: &AppState, text: &str) -> Result<(), String> {
        let Action::Deck { deck, action } = Action::parse(text).unwrap() else {
            panic!("{text} is not a deck action");
        };
        let edit = grid_edit(action).expect("not a grid edit");
        apply_grid_edit(state, deck, edit)
    }

    #[test]
    fn a_grid_edit_is_kept_with_the_track() {
        let state = app_with_track();
        grid_on_deck(
            &state,
            Beatgrid::new(
                FramePos::new(1_000.0),
                Bpm::new(128.0).unwrap(),
                Confidence::new(0.2),
            ),
        );

        edit(&state, "deck 1 grid_nudge 10").unwrap();

        let stored = state
            .library()
            .get()
            .unwrap()
            .track(id(1))
            .unwrap()
            .unwrap()
            .analysis;
        let expected = 1_000.0 + 10.0 / 1000.0 * SR.as_f64();
        assert!((stored.grid_anchor.unwrap() - expected).abs() < 1e-6);
        assert_eq!(stored.grid_confidence, Some(1.0), "an edit is certain");
    }

    /// Undoing an edit has to be as durable as making one, or a DJ who resets a
    /// grid finds their bad edit back tomorrow.
    #[test]
    fn resetting_a_grid_is_kept_too() {
        let state = app_with_track();
        let original = Beatgrid::new(
            FramePos::new(1_000.0),
            Bpm::new(128.0).unwrap(),
            Confidence::new(0.2),
        );
        grid_on_deck(&state, original);

        edit(&state, "deck 1 grid_scale 2").unwrap();
        edit(&state, "deck 1 grid_reset").unwrap();

        let stored = state
            .library()
            .get()
            .unwrap()
            .track(id(1))
            .unwrap()
            .unwrap()
            .analysis;
        assert_eq!(stored.beatgrid(), Some(original));
    }

    /// A grid edit on a deck holding a track the library has never seen must
    /// not fail the edit -- the deck still plays, there is simply nowhere to
    /// keep the correction.
    #[test]
    fn editing_a_grid_for_an_unknown_track_still_edits() {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();
        state.set_deck_track(
            deck(),
            crate::state::LoadedTrackInfo {
                title: "A".to_owned(),
                artist: None,
                id: id(9),
            },
        );
        grid_on_deck(
            &state,
            Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(128.0).unwrap(),
                Confidence::new(0.2),
            ),
        );

        edit(&state, "deck 1 grid_nudge 5").unwrap();
        assert!(state.waveforms().grid(1).is_some());
    }

    // -- saved loops -------------------------------------------------------

    fn set_loop(state: &AppState, start: f32, end: f32) {
        use dj_core::param::DeckParam;
        let registry = state.registry();
        let set = |param, value| registry.set(dj_core::ParamId::Deck(deck(), param), value);
        set(DeckParam::LoopActive, 1.0);
        set(DeckParam::LoopStart, start);
        set(DeckParam::LoopEnd, end);
    }

    #[test]
    fn a_saved_loop_survives_the_round_trip() {
        let state = app_with_track();
        set_loop(&state, 96_000.0, 192_000.0);

        save_loop(&state, deck(), 1).unwrap();
        let stored = state.library().get().unwrap().loops(id(1)).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].slot, 1);
        assert_eq!(stored[0].start_frame, 96_000.0);
        assert_eq!(stored[0].end_frame, 192_000.0);

        // And it comes back.
        recall_loop(&state, deck(), 1).unwrap();
    }

    #[test]
    fn saving_over_a_slot_replaces_it_rather_than_adding() {
        let state = app_with_track();
        set_loop(&state, 96_000.0, 192_000.0);
        save_loop(&state, deck(), 1).unwrap();

        set_loop(&state, 480_000.0, 576_000.0);
        save_loop(&state, deck(), 1).unwrap();

        let stored = state.library().get().unwrap().loops(id(1)).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].start_frame, 480_000.0);
    }

    #[test]
    fn several_slots_are_kept_separately_and_in_order() {
        let state = app_with_track();
        set_loop(&state, 480_000.0, 576_000.0);
        save_loop(&state, deck(), 3).unwrap();
        set_loop(&state, 96_000.0, 192_000.0);
        save_loop(&state, deck(), 1).unwrap();

        let stored = state.library().get().unwrap().loops(id(1)).unwrap();
        assert_eq!(
            stored.iter().map(|region| region.slot).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn saving_with_no_loop_playing_says_so() {
        let state = app_with_track();
        assert!(save_loop(&state, deck(), 1).is_err());
    }

    #[test]
    fn recalling_an_empty_slot_says_so_rather_than_looping_over_nothing() {
        let state = app_with_track();
        let error = recall_loop(&state, deck(), 4).unwrap_err();
        assert!(error.contains('4'), "the message should name the slot");
    }

    /// A row the database should not contain must be reported, not looped over.
    #[test]
    fn a_reversed_saved_loop_is_refused() {
        let state = app_with_track();
        state
            .library()
            .get()
            .unwrap()
            .set_loops(
                id(1),
                &[dj_library::StoredLoop {
                    slot: 1,
                    start_frame: 192_000.0,
                    end_frame: 96_000.0,
                    label: None,
                }],
            )
            .unwrap();

        assert!(recall_loop(&state, deck(), 1).is_err());
    }

    #[test]
    fn every_saved_loop_verb_the_interface_sends_parses() {
        for text in ["deck 1 loop_save 1", "deck 2 loop_recall 8"] {
            assert!(Action::parse(text).is_ok(), "{text} must parse");
        }
        // Slot 0 and slot 9 are mistakes upstream, not requests.
        assert!(Action::parse("deck 1 loop_save 0").is_err());
        assert!(Action::parse("deck 1 loop_recall 9").is_err());
    }
}

// -- playlists and history -------------------------------------------------

/// A node in the sidebar.
#[derive(Debug, Clone, Serialize)]
pub struct PlaylistDto {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    /// "list", "folder" or "smart".
    pub kind: String,
    pub track_count: i64,
    /// The filter, for a smart folder. `None` for the other kinds.
    pub query: Option<String>,
}

impl From<dj_library::Playlist> for PlaylistDto {
    fn from(node: dj_library::Playlist) -> Self {
        Self {
            id: node.id,
            name: node.name,
            parent_id: node.parent_id,
            kind: node.kind.as_sql().to_owned(),
            track_count: node.track_count,
            query: node.query,
        }
    }
}

/// A track in a playlist, with the position that identifies it.
///
/// The position is carried because the same track can be in a playlist twice,
/// and "remove this one" has to name which.
#[derive(Debug, Clone, Serialize)]
pub struct PlaylistEntryDto {
    pub position: i64,
    #[serde(flatten)]
    pub track: LibraryTrackDto,
}

#[tauri::command]
pub fn list_playlists(state: State<'_, AppState>) -> Result<Vec<PlaylistDto>, String> {
    Ok(library(&state)?
        .playlists()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(PlaylistDto::from)
        .collect())
}

/// `kind` is "list", "folder" or "smart".
#[tauri::command]
pub fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    parent: Option<i64>,
    kind: String,
    query: Option<String>,
) -> Result<i64, String> {
    let kind = dj_library::PlaylistKind::from_sql(&kind)
        .ok_or_else(|| format!("{kind:?} is not a kind of playlist"))?;
    // A smart folder with no filter would show the whole collection, which is
    // what "All tracks" is for. Give it one that says so until it is edited.
    let query = match (kind, query.as_deref()) {
        (dj_library::PlaylistKind::Smart, None | Some("")) => Some("bpm > 0"),
        (dj_library::PlaylistKind::Smart, Some(q)) => Some(q),
        _ => None,
    };
    library(&state)?
        .create_playlist(
            name.trim(),
            parent,
            kind,
            query,
            crate::library::now_seconds(),
        )
        .map_err(|e| e.to_string())
}

/// Change what a smart folder selects.
///
/// The filter is parsed before it is stored, so a mistake is reported while the
/// DJ is looking at the box rather than the next time they open the folder.
#[tauri::command]
pub fn set_playlist_query(
    state: State<'_, AppState>,
    id: i64,
    query: String,
) -> Result<(), String> {
    library(&state)?
        .set_playlist_query(id, query.trim())
        .map_err(|e| e.to_string())
}

/// Check a filter without storing it, so the editor can say what is wrong as
/// it is typed.
#[tauri::command]
pub fn check_filter(query: String) -> Result<(), String> {
    dj_library::filter::parse(query.trim())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The tracks a smart folder currently selects.
#[tauri::command]
pub fn smart_playlist_tracks(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<LibraryTrackDto>, String> {
    Ok(library(&state)?
        .smart_playlist_tracks(id, BROWSE_LIMIT)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(LibraryTrackDto::from)
        .collect())
}

#[tauri::command]
pub fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> Result<(), String> {
    library(&state)?
        .rename_playlist(id, name.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    library(&state)?
        .delete_playlist(id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_playlist(
    state: State<'_, AppState>,
    id: i64,
    parent: Option<i64>,
) -> Result<(), String> {
    library(&state)?
        .move_playlist(id, parent)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn playlist_tracks(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<PlaylistEntryDto>, String> {
    Ok(library(&state)?
        .playlist_tracks(id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(position, track)| PlaylistEntryDto {
            position,
            track: LibraryTrackDto::from(track),
        })
        .collect())
}

/// Put a track in a playlist, by its content hash as the browser reports it.
#[tauri::command]
pub fn add_to_playlist(
    state: State<'_, AppState>,
    playlist: i64,
    track: String,
) -> Result<(), String> {
    let id = parse_track_id(&track)?;
    library(&state)?
        .add_to_playlist(playlist, id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist: i64,
    position: i64,
) -> Result<(), String> {
    library(&state)?
        .remove_from_playlist(playlist, position)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_playlist(
    state: State<'_, AppState>,
    playlist: i64,
    order: Vec<i64>,
) -> Result<(), String> {
    library(&state)?
        .reorder_playlist(playlist, &order)
        .map_err(|e| e.to_string())
}

/// One play, as the history panel shows it.
#[derive(Debug, Clone, Serialize)]
pub struct PlayRecordDto {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub played_at: i64,
    pub session_id: Option<String>,
}

/// How much history to hand over at once. A long night is a few hundred
/// tracks; anything past this is scrolling nobody does.
const HISTORY_LIMIT: usize = 500;

#[tauri::command]
pub fn play_history(state: State<'_, AppState>) -> Result<Vec<PlayRecordDto>, String> {
    Ok(library(&state)?
        .history(HISTORY_LIMIT)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|record| PlayRecordDto {
            track_id: record.track_id,
            title: record.title,
            artist: record.artist,
            played_at: record.played_at,
            session_id: record.session_id,
        })
        .collect())
}

/// Turn the hex the interface carries back into a track id.
///
/// Validated rather than trusted: the value came out of a DTO and back through
/// IPC, and a malformed one should be a message rather than a row keyed on
/// nonsense.
fn parse_track_id(hex: &str) -> Result<dj_core::TrackId, String> {
    if hex.len() != 64 {
        return Err(format!("{hex:?} is not a track id"));
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = hex
            .get(index * 2..index * 2 + 2)
            .ok_or_else(|| format!("{hex:?} is not a track id"))?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| format!("{hex:?} is not a track id"))?;
    }
    Ok(dj_core::TrackId::from_bytes(bytes))
}

#[cfg(test)]
mod playlist_command_tests {
    use super::*;

    #[test]
    fn a_track_id_survives_the_trip_through_the_interface() {
        let id = dj_core::TrackId::from_bytes([0xab; 32]);
        assert_eq!(parse_track_id(&id.to_hex()).unwrap(), id);
    }

    /// The value came back through IPC. A malformed one should be a message,
    /// not a playlist row keyed on nonsense that never matches a track again.
    #[test]
    fn a_malformed_track_id_is_refused() {
        assert!(parse_track_id("").is_err());
        assert!(parse_track_id("abc").is_err());
        assert!(parse_track_id(&"z".repeat(64)).is_err());
        // Right length, wrong alphabet in the middle.
        let mut bad = "a".repeat(64);
        bad.replace_range(30..32, "zz");
        assert!(parse_track_id(&bad).is_err());
    }

    #[test]
    fn uppercase_hex_is_accepted() {
        let id = dj_core::TrackId::from_bytes([0xab; 32]);
        assert_eq!(parse_track_id(&id.to_hex().to_uppercase()).unwrap(), id);
    }
}

// -- importing -------------------------------------------------------------

/// What an import did, for the interface.
#[derive(Debug, Clone, Serialize)]
pub struct ImportResultDto {
    /// "rekordbox XML", "Traktor NML" or "iTunes XML".
    pub format: String,
    pub tracks: usize,
    /// Of those, already in the collection and updated in place.
    pub already_known: usize,
    /// Of those, queued for identification.
    pub queued: usize,
    pub playlists: usize,
    pub folders: usize,
    pub skipped: Vec<String>,
}

/// Import a library export.
///
/// The format is chosen by what is at the path rather than by its extension:
/// rekordbox and iTunes both write `.xml`, a DJ who renamed theirs should still
/// get their collection, and Serato is a folder rather than a file at all.
///
/// Reading and applying both run on a blocking worker. A rekordbox export of a
/// real collection is megabytes of XML and thousands of rows, which is nothing
/// next to decoding but is far too much for the interface thread.
#[tauri::command]
pub async fn import_library(
    state: State<'_, AppState>,
    path: String,
) -> Result<ImportResultDto, String> {
    let db = library(&state)?;
    let now = crate::library::now_seconds();

    tauri::async_runtime::spawn_blocking(move || {
        // A path rather than a file: Serato has no export file, only a
        // `_Serato_` folder, and the DJ should not have to know which kind of
        // thing they are choosing.
        let (format, collection) = dj_library::import::read_path(std::path::Path::new(&path))
            .map_err(|e| format!("{path}: {e}"))?;
        let report = db.import(&collection, now).map_err(|e| e.to_string())?;

        Ok(ImportResultDto {
            format: format.label().to_owned(),
            tracks: report.tracks,
            already_known: report.already_known,
            queued: report.queued,
            playlists: report.playlists,
            folders: report.folders,
            skipped: report.skipped,
        })
    })
    .await
    .map_err(|e| format!("import task failed: {e}"))?
}

// -- editing, duplicates and session export --------------------------------

/// What a batch edit is setting.
///
/// One struct rather than ten arguments: they are one intention, they arrive
/// together from one form, and a signature that long is one where the caller
/// eventually passes `genre` where `label` goes.
///
/// Every field is optional and absent means *leave it alone*. Clearing is
/// [`clear_track_field`], which says so.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEditDto {
    pub genre: Option<String>,
    pub label: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub comment: Option<String>,
    pub year: Option<i32>,
    /// 0..=5.
    pub rating: Option<u8>,
    /// `#rrggbb`.
    pub colour: Option<String>,
}

/// Set fields across a selection. An absent field is left alone, not cleared.
#[tauri::command]
pub fn edit_tracks(
    state: State<'_, AppState>,
    tracks: Vec<String>,
    edit: TrackEditDto,
) -> Result<usize, String> {
    let ids = parse_track_ids(&tracks)?;
    let clean = |value: Option<String>| {
        value
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let edit = dj_library::TrackEdit {
        genre: clean(edit.genre),
        label: clean(edit.label),
        artist: clean(edit.artist),
        album: clean(edit.album),
        comment: clean(edit.comment),
        year: edit.year,
        // Refused rather than clamped: a rating outside the scale is a bug
        // upstream, and silently making it five would hide it.
        rating: match edit.rating {
            Some(value) if value > 5 => return Err(format!("{value} is not a rating")),
            other => other,
        },
        colour: clean(edit.colour),
    };
    library(&state)?
        .edit_tracks(&ids, &edit)
        .map_err(|e| e.to_string())
}

/// Empty a field across a selection.
#[tauri::command]
pub fn clear_track_field(
    state: State<'_, AppState>,
    tracks: Vec<String>,
    field: String,
) -> Result<usize, String> {
    let ids = parse_track_ids(&tracks)?;
    let field = dj_library::EditableField::from_name(&field)
        .ok_or_else(|| format!("{field:?} is not a field that can be cleared"))?;
    library(&state)?
        .clear_field(&ids, field)
        .map_err(|e| e.to_string())
}

/// One track whose audio is in more than one place.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateDto {
    #[serde(flatten)]
    pub track: LibraryTrackDto,
    /// Every path holding this audio, newest first.
    pub paths: Vec<String>,
}

/// How many duplicate groups to hand over. More than this and the answer is
/// "your collection needs a tidy", not a longer list.
const DUPLICATE_LIMIT: usize = 200;

#[tauri::command]
pub fn find_duplicates(state: State<'_, AppState>) -> Result<Vec<DuplicateDto>, String> {
    Ok(library(&state)?
        .duplicates(DUPLICATE_LIMIT)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(track, paths)| DuplicateDto {
            track: LibraryTrackDto::from(track),
            paths: paths
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        })
        .collect())
}

/// Forget one of a track's paths.
///
/// The library's memory of a file, not the file. Nothing here deletes anybody's
/// music — the DJ removes the copy they do not want, and this stops the library
/// listing it.
#[tauri::command]
pub fn forget_track_path(
    state: State<'_, AppState>,
    track: String,
    path: String,
) -> Result<(), String> {
    let id = parse_track_id(&track)?;
    library(&state)?
        .forget_path(id, std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDto {
    pub id: String,
    pub tracks: i64,
    /// Unix seconds of the last play.
    pub ended_at: i64,
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionDto>, String> {
    Ok(library(&state)?
        .sessions(50)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(id, tracks, ended_at)| SessionDto {
            id,
            tracks,
            ended_at,
        })
        .collect())
}

/// Write a session out as a set list.
///
/// Plain text with the time, artist and title — the format a promoter or a
/// royalty return actually asks for, and one a DJ can read without a tool.
#[tauri::command]
pub fn export_session(
    state: State<'_, AppState>,
    session: String,
    path: String,
) -> Result<usize, String> {
    let plays = library(&state)?
        .session(&session)
        .map_err(|e| e.to_string())?;
    if plays.is_empty() {
        return Err(format!("there is nothing recorded for {session}"));
    }

    let mut out = format!("{session}\n\n");
    // Times relative to the first track: what somebody reading a set list
    // wants is how far into the night it was, not the wall clock of a machine
    // in another time zone.
    let start = plays.first().map_or(0, |play| play.played_at);
    for play in &plays {
        let elapsed = (play.played_at - start).max(0);
        out.push_str(&format!(
            "{:02}:{:02}  {} — {}\n",
            elapsed / 3600,
            (elapsed % 3600) / 60,
            play.artist,
            play.title
        ));
    }

    std::fs::write(&path, out).map_err(|e| format!("could not write {path}: {e}"))?;
    Ok(plays.len())
}

/// Several ids at once, refusing the whole batch if any is malformed.
///
/// The whole batch, because a partial edit is worse than none: a DJ who
/// selected forty tracks and had thirty-nine change has no way to tell which.
fn parse_track_ids(hexes: &[String]) -> Result<Vec<dj_core::TrackId>, String> {
    hexes.iter().map(|hex| parse_track_id(hex)).collect()
}

#[cfg(test)]
mod editing_command_tests {
    use super::*;

    /// A partial edit is worse than none: a DJ who selected forty tracks and
    /// had thirty-nine change has no way to tell which.
    #[test]
    fn one_malformed_id_refuses_the_whole_batch() {
        let good = dj_core::TrackId::from_bytes([1; 32]).to_hex();
        assert!(parse_track_ids(&[good.clone(), good.clone()]).is_ok());
        assert!(parse_track_ids(&[good, "nonsense".to_owned()]).is_err());
    }

    #[test]
    fn an_empty_selection_parses_to_an_empty_batch() {
        assert_eq!(parse_track_ids(&[]).unwrap().len(), 0);
    }
}

// -- SideView --------------------------------------------------------------

/// The name the Sidelist's playlist is filed under.
///
/// A constant rather than a literal at each call site, because it is the key
/// that finds the list again after a restart and a typo in one of three places
/// would quietly make a second one.
const SIDELIST: &str = "sidelist";

/// The Sidelist: what you have pulled aside for later.
///
/// A real playlist behind the scenes — see the migration that added system
/// playlists — so it keeps its order, survives a restart, and loads to a deck
/// by the same path as any crate.
#[tauri::command]
pub fn sidelist(state: State<'_, AppState>) -> Result<Vec<PlaylistEntryDto>, String> {
    let db = library(&state)?;
    let id = db
        .system_playlist(SIDELIST, crate::library::now_seconds())
        .map_err(|e| e.to_string())?;
    Ok(db
        .playlist_tracks(id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(position, track)| PlaylistEntryDto {
            position,
            track: LibraryTrackDto::from(track),
        })
        .collect())
}

#[tauri::command]
pub fn sidelist_add(state: State<'_, AppState>, track: String) -> Result<(), String> {
    let db = library(&state)?;
    let id = db
        .system_playlist(SIDELIST, crate::library::now_seconds())
        .map_err(|e| e.to_string())?;
    db.add_to_playlist(id, parse_track_id(&track)?)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sidelist_remove(state: State<'_, AppState>, position: i64) -> Result<(), String> {
    let db = library(&state)?;
    let id = db
        .system_playlist(SIDELIST, crate::library::now_seconds())
        .map_err(|e| e.to_string())?;
    db.remove_from_playlist(id, position)
        .map_err(|e| e.to_string())
}

/// Empty the Sidelist.
///
/// What a DJ does at the end of a night. Removes the entries, not the tracks —
/// the same rule as deleting a playlist.
#[tauri::command]
pub fn sidelist_clear(state: State<'_, AppState>) -> Result<(), String> {
    let db = library(&state)?;
    let id = db
        .system_playlist(SIDELIST, crate::library::now_seconds())
        .map_err(|e| e.to_string())?;
    db.clear_playlist(id).map_err(|e| e.to_string())
}

// -- the world -------------------------------------------------------------

/// What the living interface should draw, right now.
///
/// A pull rather than a push: the interface already receives the snapshot at
/// 60 Hz, and emitting a second stream alongside it would double the traffic to
/// say the same thing twice. The renderer asks when it has a frame to draw --
/// which, at Tier 0 or with the window hidden, is never.
///
/// See [ADR-0009](../../../docs/adr/0009-the-living-interface.md).
#[tauri::command]
pub fn world(state: State<'_, AppState>) -> dj_world::World {
    // Read once here rather than inside the world builder: the collection lives
    // behind a database handle that may not be open, and a query per frame
    // would be the wrong shape entirely.
    let highland = highland_of(&state);
    crate::world::of(&get_snapshot(state), highland)
}

/// How much of the collection is still under mist.
///
/// Best effort: a library that is not open yet, or a query that fails, means
/// there is nothing to say about the highland — not that the interface should
/// stop drawing. Nothing here is worth an error in a booth.
fn highland_of(state: &AppState) -> dj_world::HighlandReading {
    use std::sync::atomic::Ordering::Relaxed;
    let progress = state.identify_progress();
    let surveyed = progress.as_ref().map_or(0, |p| p.done.load(Relaxed));
    let dry = progress.as_ref().map_or(0, |p| p.failed.load(Relaxed));
    let unsurveyed = state
        .library()
        .get()
        .ok()
        .and_then(|db| db.pending_count().ok())
        .unwrap_or(0);
    dj_world::HighlandReading {
        unsurveyed: u32::try_from(unsurveyed).unwrap_or(u32::MAX),
        surveyed: u32::try_from(surveyed).unwrap_or(u32::MAX),
        dry: u32::try_from(dry).unwrap_or(u32::MAX),
    }
}

/// The folder a development run should load from, if any.
///
/// **Asked for** rather than pushed. The first version emitted an event three
/// seconds after startup and raced the webview: on a cold dev server the
/// listener was not registered yet, the event went nowhere, and the run looked
/// like the demo had silently failed. A command has no such race — the
/// interface asks once it exists.
#[tauri::command]
pub fn demo_folder() -> Option<String> {
    std::env::var(crate::DEMO_ENV)
        .ok()
        .filter(|s| !s.is_empty())
}

/// Whether the watershed was showing last time.
#[tauri::command]
pub fn watershed(state: State<'_, AppState>) -> bool {
    state.watershed()
}

/// Remember whether the watershed is showing.
#[tauri::command]
pub fn set_watershed(state: State<'_, AppState>, showing: bool) {
    state.set_watershed(showing);
}

// -- layouts ---------------------------------------------------------------

/// Every layout available: the four that ship, then the DJ's own.
///
/// The DJ's come second so one of theirs named "Pro" sits beside the built-in
/// rather than replacing it — a layout somebody wrote should never make a
/// shipped one unreachable.
#[tauri::command]
pub fn list_layouts(state: State<'_, AppState>) -> Vec<crate::layout::Layout> {
    let mut layouts = crate::layout::builtin();
    if let Some(dir) = state.layout_dir() {
        layouts.extend(crate::layout::load_dir(&dir));
    }
    layouts
}

/// Where a DJ puts their own layout files.
///
/// Returned so the interface can say where, and open it. A path is not a
/// secret, and telling somebody the folder is the difference between a feature
/// they can use and one they cannot find.
#[tauri::command]
pub fn layout_folder(state: State<'_, AppState>) -> Option<String> {
    state
        .layout_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
}

/// The layout the DJ last chose, resolved against the layouts that exist now.
///
/// Resolved here rather than in the interface because the name may no longer
/// name anything: a DJ can delete the layout file they were using, and the
/// honest answer then is "none", not a layout built out of defaults wearing
/// their name.
#[tauri::command]
pub fn chosen_layout(state: State<'_, AppState>) -> Option<crate::layout::Layout> {
    let name = state.chosen_layout()?;
    list_layouts(state).into_iter().find(|l| l.name == name)
}

/// Remember which layout the DJ picked.
///
/// The name only. A layout is a file the DJ owns and may edit; storing a copy
/// would mean their edits stopped taking effect for reasons nothing on screen
/// could explain.
#[tauri::command]
pub fn choose_layout(state: State<'_, AppState>, name: String) {
    state.set_chosen_layout(&name);
}

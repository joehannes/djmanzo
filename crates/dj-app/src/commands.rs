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

/// The input device feeding the microphone strip.
///
/// Its own type rather than [`ActiveDeviceDto`]: that one carries a headphone
/// cue and a reason the cue failed, and an input has neither. Reusing it would
/// mean two fields that are always null and a reader who has to know that.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicDeviceDto {
    pub name: String,
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub channels: u16,
    /// One-way latency of the input alone. The DJ hears themselves this much
    /// late *plus* the output's own latency — worth showing, because a
    /// microphone through a computer is late and no amount of software makes
    /// it not so.
    pub latency_ms: f64,
}

impl From<&dj_audio::ActiveConfig> for MicDeviceDto {
    fn from(config: &dj_audio::ActiveConfig) -> Self {
        MicDeviceDto {
            name: config.device_name.clone(),
            sample_rate: config.sample_rate.get(),
            buffer_frames: config.buffer_frames,
            channels: config.channels,
            latency_ms: config.latency_ms(),
        }
    }
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

/// A panel that can be given a window of its own.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelDto {
    /// The name used in the window label and in the URL.
    pub id: String,
    pub title: String,
    pub detached: bool,
}

/// Which panels can be detached, and which are.
#[tauri::command]
pub fn list_panels(state: State<'_, AppState>) -> Vec<PanelDto> {
    let detached = state.detached();
    crate::monitors::Panel::ALL
        .into_iter()
        .map(|panel| PanelDto {
            id: panel.slug().to_owned(),
            title: panel.title().to_owned(),
            detached: detached.contains(panel),
        })
        .collect()
}

/// Give a panel a window of its own.
///
/// The window is opened and nothing else: where it goes is the desktop's
/// business. See `crate::monitors` for why djmanzo never asks how many screens
/// there are.
///
/// # Errors
/// When the panel name is not one of the seven, or the window will not open.
#[tauri::command]
pub fn detach_panel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    panel: String,
) -> Result<(), String> {
    use tauri::Manager;
    let panel = crate::monitors::Panel::parse(&panel)
        .ok_or_else(|| format!("no panel called {panel:?}"))?;
    let label = panel.label();

    // Already open: bring it forward rather than opening a second one. A DJ
    // pressing the button twice has lost the window behind something, and
    // another identical window is not what they wanted.
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }

    let (width, height) = panel.size();
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App(format!("index.html?panel={}", panel.slug()).into()),
    )
    .title(panel.title())
    .inner_size(width, height)
    .min_inner_size(320.0, 240.0)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;
    // A detached room surface is a camera in a second window, and the second
    // window's webview needs the same answer the first one got.
    crate::senses::permit(&window);

    state.detach_panel(panel);
    Ok(())
}

/// Bring a panel back into the main window.
#[tauri::command]
pub fn attach_panel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    panel: String,
) -> Result<(), String> {
    use tauri::Manager;
    let panel = crate::monitors::Panel::parse(&panel)
        .ok_or_else(|| format!("no panel called {panel:?}"))?;
    if let Some(window) = app.get_webview_window(&panel.label()) {
        let _ = window.close();
    }
    state.attach_panel(panel);
    Ok(())
}

/// A CLAP plugin found on disk, before anything is loaded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFileDto {
    pub path: String,
    pub name: String,
}

/// One of a loaded plugin's own controls.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginParamDto {
    pub id: u32,
    pub name: String,
    pub module: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub value: f64,
    pub stepped: bool,
    pub read_only: bool,
}

/// What is on the master's plugin insert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateDto {
    pub loaded: bool,
    pub name: String,
    pub vendor: String,
    pub path: String,
    pub params: Vec<PluginParamDto>,
}

/// Every CLAP plugin in the standard search paths.
///
/// Scanning reads directory names and nothing else — no plugin code runs until
/// one is actually loaded.
#[tauri::command]
pub fn list_plugins() -> Vec<PluginFileDto> {
    dj_clap::scan()
        .into_iter()
        .map(|found| PluginFileDto {
            path: found.path.display().to_string(),
            name: found.name,
        })
        .collect()
}

/// Which separator is running, and what a better one would need.
///
/// The interface asks once at startup. `available` is about whether the stem
/// controls do anything at all; `reason` is about why the *better* separator
/// is not the one doing it, which is a different question and a different
/// sentence.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemsStatusDto {
    /// True when something is separating -- the built-in separator counts.
    pub available: bool,
    /// What is doing it, to show beside the controls. `None` when nothing is.
    pub backend: Option<String>,
    /// Why a downloaded model is not being used. `None` when one is.
    pub reason: Option<String>,
    /// A model is still being loaded behind the built-in separator: ask
    /// again shortly.
    pub loading: bool,
}

/// Which separator is running on this machine.
#[tauri::command]
pub fn stems_status(state: State<'_, AppState>) -> StemsStatusDto {
    let backend = state.stems_backend();
    StemsStatusDto {
        available: backend.is_some(),
        backend: backend.map(str::to_owned),
        reason: state.stems_reason(),
        loading: state.stems_loading(),
    }
}

/// Whether a deck can be sent out in parts, and which one is.
///
/// `channels` rather than a bare boolean because the panel has to say *why*
/// the control is unavailable, and "your interface has 2 outputs, this needs
/// 8" is a sentence a DJ can act on where a greyed-out switch is not.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StemOutDto {
    /// The deck going out in parts, as the DJ numbers it, or `None`.
    pub deck: Option<u8>,
    /// How many decks are going out on pairs of their own, or `None`.
    ///
    /// In the same shape as `deck` rather than a panel of its own, because the
    /// two arrangements are exclusive: they want the same sockets, and an
    /// interface that could show both switched on would be describing an
    /// engine state that cannot exist.
    pub decks: Option<usize>,
    /// The most decks this device could carry a pair for. Zero with nothing
    /// open.
    pub deck_capacity: usize,
    /// Outputs on the open device. `None` when no device is open.
    pub channels: Option<u16>,
    /// How many outputs this needs. Constant, but sent so the interface does
    /// not restate a number the engine owns.
    pub required: u16,
    /// True when the open device is wide enough. False with no device open:
    /// there is nothing to be wide enough yet.
    pub supported: bool,
}

/// Which deck is being sent out in parts, and whether the device allows it.
#[tauri::command]
pub fn stem_out(state: State<'_, AppState>) -> StemOutDto {
    stem_out_view(&state)
}

/// What `stem_out` reports, minus Tauri's `State` wrapper — which is the one
/// thing in that function a unit test cannot build.
fn stem_out_view(state: &AppState) -> StemOutDto {
    let channels = state.active_device().map(|device| device.channels);
    StemOutDto {
        deck: state.stem_out().map(dj_core::DeckId::human_number),
        decks: state.deck_out(),
        deck_capacity: channels.map_or(0, |open| usize::from(open) / 2),
        channels,
        required: REQUIRED_STEM_OUT_CHANNELS,
        supported: channels.is_some_and(|open| open >= REQUIRED_STEM_OUT_CHANNELS),
    }
}

/// Send every deck out on a pair of its own, or stop.
///
/// `decks` is how many, so a four-deck set on an eight-output interface can
/// send all four while a two-deck set on the same interface leaves four
/// sockets free. `None`, or zero, puts the mix back.
///
/// Accepted even where the device is too narrow, for the reason
/// [`set_stem_out`] is: the interface arrives after the plan does.
///
/// # Errors
/// When `decks` is more decks than djmanzo has.
#[tauri::command]
pub fn set_deck_out(
    state: State<'_, AppState>,
    decks: Option<usize>,
) -> Result<StemOutDto, String> {
    if let Some(count) = decks
        && count > crate::state::DECK_COUNT
    {
        return Err(format!(
            "djmanzo has {} decks, not {count}",
            crate::state::DECK_COUNT
        ));
    }
    state.set_deck_out(decks);
    Ok(stem_out_view(&state))
}

/// Four stems, two channels each.
const REQUIRED_STEM_OUT_CHANNELS: u16 = dj_engine::STEM_OUT_CHANNELS as u16;

/// Send one deck out in parts, or stop.
///
/// Accepted even on a device too narrow for it: the engine refuses to build
/// the arrangement and keeps mixing normally, and the choice takes effect if a
/// wider device is opened later. Refusing here instead would mean a DJ who
/// sets this up before plugging in the interface finds it silently forgotten.
///
/// # Errors
/// When `deck` names a deck that does not exist.
#[tauri::command]
pub fn set_stem_out(state: State<'_, AppState>, deck: Option<u8>) -> Result<StemOutDto, String> {
    let id = match deck {
        Some(number) => {
            Some(dj_core::DeckId::from_human(number).ok_or_else(|| format!("no deck {number}"))?)
        }
        None => None,
    };
    state.set_stem_out(id);
    Ok(stem_out_view(&state))
}

/// Start syncing tempo with other djmanzo instances on the network.
///
/// `listen` is where this instance listens; `send_to` is where announcements
/// go — a broadcast address for a LAN, or one peer's address for a direct link
/// between two machines. Both default to loopback so that trying it out on one
/// machine works before anything is plugged in.
///
/// # Errors
/// When either address cannot be parsed, or the listening one cannot be bound.
#[tauri::command]
pub fn start_peer_sync(
    state: State<'_, AppState>,
    listen: Option<String>,
    send_to: Option<String>,
) -> Result<crate::peersync::PeerStatus, String> {
    let listen: std::net::SocketAddr = listen
        .unwrap_or_else(|| "127.0.0.1:7655".to_owned())
        .parse()
        .map_err(|e| format!("that is not an address to listen on: {e}"))?;
    let send_to: std::net::SocketAddr = send_to
        .unwrap_or_else(|| "127.0.0.1:7655".to_owned())
        .parse()
        .map_err(|e| format!("that is not an address to announce to: {e}"))?;
    state.peers().start(listen, send_to, state.registry())
}

/// Stop syncing with the network.
#[tauri::command]
pub fn stop_peer_sync(state: State<'_, AppState>) -> crate::peersync::PeerStatus {
    state.peers().stop();
    state.peers().status()
}

/// Who is on the network, and what tempo they have settled on.
#[tauri::command]
pub fn peer_status(state: State<'_, AppState>) -> crate::peersync::PeerStatus {
    state.peers().status()
}

/// What is on the insert right now.
#[tauri::command]
pub fn plugin_state(state: State<'_, AppState>) -> PluginStateDto {
    let view = state.plugin().view();
    PluginStateDto {
        loaded: view.loaded,
        name: view.name,
        vendor: view.vendor,
        path: view.path,
        params: view
            .params
            .into_iter()
            .map(|param| PluginParamDto {
                id: param.id,
                name: param.name,
                module: param.module,
                min: param.min,
                max: param.max,
                default: param.default,
                value: param.value,
                stepped: param.stepped,
                read_only: param.read_only,
            })
            .collect(),
    }
}

/// Put a plugin on the master.
///
/// Loading runs third-party code in this process — there is no way to host
/// plugins that is not that. It happens on the plugin's own thread, and only
/// once it has activated successfully does the engine hear about it.
///
/// # Errors
/// When no device is open (the plugin has to be activated for a real sample
/// rate and block size), when the file is not a plugin, or when the plugin
/// refuses the configuration.
#[tauri::command]
pub fn load_plugin(
    state: State<'_, AppState>,
    path: String,
    plugin_id: Option<String>,
) -> Result<PluginStateDto, String> {
    let active = state
        .active_device()
        .ok_or_else(|| "open an audio device first".to_owned())?;

    let processor = state
        .plugin()
        .load(
            std::path::Path::new(&path),
            plugin_id.as_deref(),
            f64::from(active.sample_rate),
            active.buffer_frames,
        )
        .map_err(|e| e.to_string())?;

    if state
        .bus()
        .send_command(dj_engine::Command::ClapInsert {
            processor: Some(Box::new(processor)),
        })
        .is_err()
    {
        // The processor went with the failed send, so the instance now has one
        // it will never get back. Letting go of it is the only way not to leak
        // the instance as well.
        state.plugin().unload();
        return Err("engine is not accepting commands; is a device open?".to_owned());
    }
    Ok(plugin_state(state))
}

/// Take the plugin off the master.
///
/// Two steps, and this is only the first: the engine is told to release the
/// processor, and it arrives back on the plugin thread some blocks later to be
/// deactivated. Between those moments the plugin is loaded, silent and on its
/// way out.
#[tauri::command]
pub fn clear_plugin(state: State<'_, AppState>) -> Result<(), String> {
    state
        .bus()
        .send_command(dj_engine::Command::ClapInsert { processor: None })
        .map_err(|_| "engine is not accepting commands".to_owned())?;
    state.plugin().unload();
    Ok(())
}

/// Devices that can capture. Empty is a normal answer — plenty of laptops in a
/// booth have nothing plugged in.
#[tauri::command]
pub fn list_inputs(state: State<'_, AppState>) -> Result<Vec<DeviceDto>, String> {
    let devices = state.host().list_inputs().map_err(|e| e.to_string())?;
    Ok(devices
        .into_iter()
        .map(|d| DeviceDto {
            id: d.id.as_str().to_owned(),
            name: d.name,
            channels: d.max_output_channels,
            sample_rate: d.default_sample_rate.get(),
            is_default: d.is_default,
            // Meaningless for an input; there is no headphone bus to split.
            supports_split_output: false,
        })
        .collect())
}

/// Attach an input device to the microphone strip.
///
/// This is the cable, not the switch. `mic on` opens the channel, and it is a
/// separate action because opening a sound card takes long enough to miss a
/// cue: a DJ plugs in once and toggles the channel all evening.
#[tauri::command]
pub fn open_mic(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> Result<MicDeviceDto, String> {
    let device = device_id.map(dj_audio::DeviceId::new);
    let config = state.host().open_mic(device).map_err(|e| e.to_string())?;
    Ok(MicDeviceDto::from(&config))
}

#[tauri::command]
pub fn close_mic(state: State<'_, AppState>) -> Result<(), String> {
    state.host().close_mic().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_device(
    state: State<'_, AppState>,
    device_id: Option<String>,
    cue_device_id: Option<String>,
    buffer_frames: Option<u32>,
) -> Result<ActiveDeviceDto, String> {
    open_device_for(&state, device_id, cue_device_id, buffer_frames)
}

/// What `open_device` does, minus Tauri's `State` wrapper — which is the one
/// thing in that function a unit test cannot build.
///
/// Extracted rather than duplicated because the interesting part of opening a
/// device is not the open: it is everything that has to be said again
/// afterwards to an engine that did not exist a moment ago.
pub(crate) fn open_device_for(
    state: &AppState,
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
    // The engine behind this device is brand new and knows nothing about the
    // controller that is already plugged in, nor about the deck the DJ asked
    // to be sent out in parts. Tell it both.
    state.apply_controller_routing();
    state.apply_stem_out();
    state.apply_deck_out();
    // Not re-applied, unlike the routing above: the host closed every input
    // along with the old engine, and re-opening a turntable's input without
    // being asked would start a deck moving while the DJ is still choosing a
    // sound card. Forgetting is the honest state -- the panel then shows
    // nothing on vinyl, which is true.
    state.clear_timecode();
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

/// Which chunk to separate next, given where the playhead is.
///
/// Nearest to `here` wins, and at equal distance the one *ahead* wins: the
/// playhead moves one way, so audio it is about to reach is worth more than
/// audio it has just passed. `None` when everything is already separated.
///
/// A free function rather than a closure inside the feeder because it is the
/// only part of look-ahead separation with a decision in it, and a decision
/// buried in a spawned thread inside a Tauri command is a decision nothing can
/// test.
#[must_use]
fn next_chunk_to_separate(
    total: usize,
    here: usize,
    separated: impl Fn(usize) -> bool,
) -> Option<usize> {
    (0..total)
        .filter(|index| !separated(*index))
        .min_by_key(|index| (index.abs_diff(here), u8::from(*index < here)))
}

/// Queue every chunk of a track for separation **once**, nearest the
/// playhead first, asking where the playhead is before each choice.
///
/// Remembers what it has queued as well as what has come back. The first
/// version looped once per chunk and asked only the published table, which
/// fills when the worker *finishes* a chunk: while the worker was busy, the
/// chunk nearest the playhead was chosen again on every pass until the
/// bounded queue filled, the duplicates were dropped on arrival ("a
/// separated chunk does not fit"), and the passes ran out with most of the
/// track never queued at all. The built-in separator is quick enough that it
/// mostly got away with it; HT-Demucs takes seconds a chunk, and a record got
/// stems for its first minute or so. Both went unseen because the warning
/// was logged under a crate the log did not show.
fn feed_chunks(
    total: usize,
    mut here: impl FnMut() -> usize,
    separated: impl Fn(usize) -> bool,
    mut queue: impl FnMut(usize),
) {
    let mut queued = vec![false; total];
    while let Some(next) =
        next_chunk_to_separate(total, here(), |index| queued[index] || separated(index))
    {
        queued[next] = true;
        queue(next);
    }
}

/// Decode a file and put it on a deck.
///
/// Decoding is slow -- minutes of audio, plus a content hash -- so it runs on a
/// blocking worker rather than on the UI thread or, catastrophically, the audio
/// thread. Only the finished `Arc` crosses into the engine.
/// Put a file in a sampler slot.
///
/// Decoded on a worker like a track, and for the same reason: reading a file is
/// I/O and the audio thread may not do any. The bank is named rather than
/// assumed, so a load cannot land in the wrong place because the DJ switched
/// banks while the file was being read.
///
/// The tempo comes from the analyser when it can find one. `None` is not a
/// failure — a vocal stab has no tempo — and a sample without one is never
/// stretched, however the sync switch is set.
#[tauri::command]
pub async fn load_sample(
    state: State<'_, AppState>,
    bank: u8,
    slot: u8,
    path: String,
) -> Result<LoadedSampleDto, String> {
    if bank == 0 || usize::from(bank) > dj_core::SAMPLE_BANKS {
        return Err(format!("no sampler bank {bank}"));
    }
    if slot == 0 || usize::from(slot) > dj_core::SAMPLE_SLOTS {
        return Err(format!("no sampler slot {slot}"));
    }

    let decoded = tauri::async_runtime::spawn_blocking(move || decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let dto = LoadedSampleDto {
        bank,
        slot,
        name: decoded.display_title(),
        duration_seconds: decoded.buffer.duration_seconds(),
    };

    state.set_sample_name(bank, slot, dto.name.clone());
    let source: std::sync::Arc<dyn dj_decode::TrackSource> = std::sync::Arc::new(decoded.buffer);
    state
        .bus()
        .send_command(dj_engine::Command::LoadSample {
            bank,
            slot,
            source,
            // Analysis of a sample is its own slice: a two-second stab has too
            // few beats for the tempo detector to be honest about, and a wrong
            // tempo is worse than none because sync would then stretch it.
            bpm: None,
        })
        .map_err(|_| "the engine queue is full".to_owned())?;

    Ok(dto)
}

/// `<deck> <track-id>` — §87's load, from wherever it came.
///
/// Decoded on the calling thread. That is `djmanzo-control` for a controller,
/// which is djmanzo's own thread and exists to keep actions in the order they
/// were played — so a load blocking it for the length of a file read blocks
/// the next action on that controller, which is the ordering the thread is for.
/// It is never the audio thread and never the interface's.
///
/// # Errors
/// A sentence naming which half is wrong: the deck, the id, the library row or
/// the file. A controller button bound to a record that has been moved off the
/// disk should say which record, not "load failed".
fn load_by_id(state: &AppState, rest: &str) -> Result<LoadedTrackDto, String> {
    let (deck, track) = rest
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("a load needs a deck and a track id: {rest:?}"))?;
    let deck_id = deck
        .trim()
        .parse::<u8>()
        .ok()
        .and_then(dj_core::DeckId::from_human)
        .ok_or_else(|| format!("not a deck: {:?}", deck.trim()))?;
    let id = dj_core::TrackId::from_hex(track.trim())
        .ok_or_else(|| format!("not a track id: {:?}", track.trim()))?;

    let db = library(state)?;
    let found = db
        .track(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no track {} in the library", id.to_hex()))?;
    let decoded = decode_file(&found.path).map_err(|e| e.to_string())?;
    put_on_deck(state, deck_id, decoded)
}

/// A sample, as the interface names it after a load.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedSampleDto {
    pub bank: u8,
    pub slot: u8,
    pub name: String,
    pub duration_seconds: f64,
}

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

    put_on_deck(&state, deck_id, decoded)
}

/// Everything a load does once the file is decoded.
///
/// Split from [`load_track`] because decoding is the part that has to happen
/// off the caller's thread, and the automix arrives here from the snapshot
/// pump rather than from a Tauri command — it has an `&AppState` and no
/// `State`, and no business being `async`.
pub fn put_on_deck(
    state: &AppState,
    deck_id: dj_core::DeckId,
    decoded: dj_decode::DecodedTrack,
) -> Result<LoadedTrackDto, String> {
    let deck = deck_id.human_number();

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
    // What §110's spectrum will be measured against, once the buffer is shared.
    let measured = state.waveforms().summary(deck);

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

    // §43. Here rather than in `load_track`, because this is djmanzo's one load
    // funnel: a record dragged in, picked from a crate, sent by a controller,
    // asked for by the assistant or brought by the automix all arrive at this
    // line. A hook on the browser's own path would count a DJ who works from
    // their crates as ignoring everything while a DJ using the same rail through
    // a controller registered nothing at all.
    if let Ok(mut fatigue) = state.fatigue().lock() {
        fatigue.landed(track_id);
    }

    // Into the library *before* the deck is playable.
    //
    // Two reasons for the ordering. A cue row has a foreign key to its track,
    // so without this every cue a DJ set on a file they opened from disk would
    // be silently discarded. And a DJ who loads a file expects to find it in
    // their collection afterwards -- a track you played is part of your library
    // whether or not you ever pointed a scan at the folder it lives in.
    remember_track(state, &decoded, sample_rate);

    let buffer = Arc::new(decoded.buffer);
    // §110's spectrum, off the load path. It costs more than the rest of the
    // summary put together — most of a second for five minutes of audio — and
    // a DJ loading the next record mid-mix should not wait for a colour. The
    // waveform is drawn at once in the three bands and takes its light when
    // this lands; if another record has been loaded by then, the store drops
    // the answer instead of painting it over the wrong one.
    if let Some(measured) = measured {
        let store = Arc::clone(state.waveforms());
        let audio = Arc::clone(&buffer);
        let spawned = std::thread::Builder::new()
            .name("spectrum".into())
            .spawn(move || {
                // §116's melody first: the spectrum's new epoch is what the
                // lane asks again on, and the melody has to be there when it
                // does. About as long again as the spectrum.
                let rate = audio.sample_rate().as_f64();
                let mono = dj_analysis::melody::mono(audio.as_interleaved(), 2);
                let hertz = dj_analysis::melody::pitches(&mono, audio.sample_rate().get());
                store.set_melody(
                    deck_id,
                    &measured,
                    crate::waveform::Melody {
                        hertz,
                        frames_per_point: rate / dj_analysis::melody::RATE,
                    },
                );
                // §116's rhythm: the banded onset curve, stepped against
                // whatever grid the deck has when the lane asks.
                store.set_onsets(
                    deck_id,
                    &measured,
                    dj_analysis::onset::detect_bands(
                        audio.as_interleaved(),
                        audio.sample_rate().get(),
                    ),
                );
                let mut coloured = (*measured).clone();
                coloured.measure_spectrum(audio.as_interleaved());
                store.set_spectrum(deck_id, &measured, coloured);
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "no thread for the spectrum; this record stays in three bands");
        }
    }
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

    // Note it in the session record. The load itself travelled as a command
    // carrying an `Arc`, which is why it is not an action -- but a set is not
    // reproducible from its actions alone, so the *fact* of the load is
    // recorded here. See `dj_control::SessionEvent`.
    state.bus().record_load(deck_id, track_id);

    // What this track had last time it was played: cues in the slots they were
    // in, and the grid as it was left -- corrected by hand, if it was. Sent
    // after the load so the engine applies them to the new track rather than to
    // whatever was on the deck a moment ago.
    restore_deck_state(state, deck_id, track_id, sample_rate);

    // Queue stem separation in the background.
    //
    // The buffer is shared, not copied. This used to `to_vec()` the whole
    // decoded track to have something to slice in the thread -- 138 MB for a
    // six-minute track -- directly beneath the comment above explaining why
    // the analyser gets an `Arc` instead. The worker copies each chunk it
    // takes, which it has to: that one crosses a channel.
    let stems_worker = state.stems_worker();
    let audio = Arc::clone(&buffer);
    let lock_clone = buffer.stems_lock();
    let registry = state.registry();
    std::thread::spawn(move || {
        // Ten seconds of stereo audio per chunk, at the rate this track was
        // actually decoded at -- the previous constant 44_100 made a chunk
        // 9.2 seconds long on a 48 kHz track, and the built-in separator
        // needs the real rate to place its band edges anyway.
        let chunk_size = sample_rate.get() as usize * dj_decode::CHANNELS * 10;
        let interleaved = audio.as_interleaved();
        let chunk_frames = chunk_size / dj_decode::CHANNELS;
        let total_frames = interleaved.len() / dj_decode::CHANNELS;
        let chunk_count = total_frames.div_ceil(chunk_frames.max(1));

        // **Separate outward from the playhead, not forward from the file.**
        //
        // Walking 0..n is the wrong order for the one thing a DJ actually
        // does with a fresh track: load it and cue straight to the drop. The
        // worker would be twenty seconds in while the playhead sat at three
        // minutes, and the stem pads would quietly do nothing there for as
        // long as it took to grind through everything in between.
        //
        // So the next chunk to separate is chosen each time round, by distance
        // from wherever the playhead is *now* -- which also means a seek
        // mid-separation redirects the work rather than being ignored.
        // Slightly ahead is preferred to slightly behind at equal distance,
        // because the playhead is moving one way.
        let here = || {
            let position = f64::from(registry.get(dj_core::param::ParamId::Deck(
                deck_id,
                dj_core::param::DeckParam::Position,
            )));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            if chunk_frames == 0 {
                0
            } else {
                (position.max(0.0) / chunk_frames as f64) as usize
            }
        };
        feed_chunks(
            chunk_count,
            here,
            |index| lock_clone.load().has_chunk(index),
            |next| {
                // `process_chunk` blocks once the worker is a few chunks
                // behind, so this thread spends most of its life asleep rather
                // than racing ahead building a queue. That is the point of the
                // bound -- and it is also what makes re-choosing worthwhile,
                // because by the time it wakes the playhead has usually moved.
                // Separated with the audio either side of it, and trimmed back
                // to the chunk afterwards.
                //
                // A chunk separated alone is wrong at both edges -- the
                // windows there have no neighbours to overlap-add with -- so
                // butting them together put a glitch at every seam, once every
                // ten seconds, for the whole track. See
                // `dj_stems::stems::SEPARATION_MARGIN`.
                let body_start = next * chunk_frames;
                let body_end = (body_start + chunk_frames).min(total_frames);
                let lead = body_start.min(dj_stems::stems::SEPARATION_MARGIN);
                let from = body_start - lead;
                let to = (body_end + dj_stems::stems::SEPARATION_MARGIN).min(total_frames);

                stems_worker.process_chunk(
                    track_id,
                    next,
                    &interleaved[from * dj_decode::CHANNELS..to * dj_decode::CHANNELS],
                    lead..lead + (body_end - body_start),
                    sample_rate.get(),
                    Some(lock_clone.clone()),
                );
            },
        );
    });

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
                lines: dj_render::GridLines::all(),
                grid: tempo.grid,
                sample_rate,
                phrase: analysis
                    .phrases
                    .and_then(|p| dj_core::Phrase::new(p.beats, p.anchor)),
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

        // And to the engine, which needs it for sync, quantize, beat jump and
        // phrase jump. Two destinations for one finding rather than one shared
        // home, because the engine's copy has to cross a lock-free queue into
        // the audio thread and the renderer's cannot.
        let _ = bus.send_command(dj_engine::Command::SetGrid {
            deck: deck_id,
            grid: analysis.tempo.as_ref().map(|tempo| tempo.grid),
            phrase: analysis
                .phrases
                .and_then(|p| dj_core::Phrase::new(p.beats, p.anchor)),
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
    perform(&state, &action)
}

/// The body of [`dispatch`], reachable without a Tauri `State`.
///
/// Split out because a controller does not arrive through a command: MIDI is
/// delivered on a thread of the operating system's, and the actions a mapping
/// produces there have to take exactly the same path as the ones a button in
/// the interface produces — including the interceptions below, which are the
/// difference between `record on` starting a recording and `record on` doing
/// nothing at all.
///
/// # Errors
/// When the text is not in the vocabulary, or the engine is not accepting
/// commands because no device is open.
pub fn perform(state: &AppState, action: &str) -> Result<(), String> {
    perform_by(state, action, dj_control::By::Hand)
}

/// The same, saying whose doing it is.
///
/// §67's *AI interventions* and *manual interventions*. Everything djmanzo does
/// on its own — the autopilot's tick, the automix's plan, an accepted
/// transaction — comes through here with [`dj_control::By::Machine`], and
/// everything else keeps [`perform`] and is a person's. **It is still the same
/// path**: the automix does not get a private channel to the engine, and the
/// origin changes what the log says about an action rather than how it
/// travels.
pub fn perform_by(state: &AppState, action: &str, by: dj_control::By) -> Result<(), String> {
    // §87's missing origins, before anything else, because a load is not an
    // `Action` and `Action::parse` would refuse it.
    //
    // §87 lists seven places a load can come from and requires the resulting
    // state to be identical. Five of them already arrived at `put_on_deck` --
    // the browser, a drop, the Next rail, the assistant's staging, the automix.
    // **A controller and the line protocol could not load at all**, because
    // loading is deliberately outside the action vocabulary
    // ([ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md):
    // it carries an `Arc`, and nothing external should be inventing one).
    //
    // The vocabulary for it already existed, in the one place a load has always
    // had to be written down: `dj_control::SessionEvent::to_line` writes
    // `load deck 1 <track-id>` into every session file, and `parse_line` reads
    // it back. This is the same line, live. So a set replayed from its log and
    // a set driven from a controller speak one language, and there is no second
    // spelling to keep in step.
    //
    // **By id, never by path.** A track id names a record in the DJ's own
    // library and the path comes from the row; accepting a path here would hand
    // anything that can reach the bus -- including a socket -- a way to make
    // djmanzo read an arbitrary file. That is a different feature with a
    // different conversation attached to it, and §87 does not ask for it.
    if let Some(rest) = action.trim().strip_prefix("load deck ") {
        return load_by_id(state, rest).map(|_| ());
    }

    // §22's audition, on the same terms and for the same reasons: it carries a
    // decoded record, so it is a line rather than an `Action`, and it takes an
    // id rather than a path.
    if let Some(rest) = action.trim().strip_prefix("audition")
        && (rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        return audition_by_id(state, rest);
    }

    let parsed = Action::parse(action).map_err(|e| format!("{action:?}: {e}"))?;
    perform_action_by(state, parsed, by)
}

/// The body of [`perform`], for a caller that has an [`Action`] already.
///
/// Split out for the network, which parses before it reaches djmanzo: a socket
/// used to dispatch straight at the bus, and so skipped every interception
/// below. `deck 1 eject` ejected in the engine and left the interface showing a
/// record that was no longer on the deck; `record on` was forwarded to an engine
/// that cannot open a file, and answered "accepted" having started nothing. §87
/// is the section that names it — the network was the one origin not going
/// through the application's own entry point, so it was the one whose resulting
/// state differed.
///
/// # Errors
/// As [`perform`], minus the parse.
pub fn perform_action(state: &AppState, parsed: Action) -> Result<(), String> {
    perform_action_by(state, parsed, dj_control::By::Hand)
}

/// The same, saying whose doing it is. See [`perform_by`].
///
/// # Errors
/// As [`perform_action`].
pub fn perform_action_by(
    state: &AppState,
    parsed: Action,
    by: dj_control::By,
) -> Result<(), String> {
    // A hand arrived on a control. Recorded before the action is carried out,
    // so an autopilot tick that lands between the two still sees the takeover
    // -- the wrong order here would let the assistant move a fader in the
    // moment between a DJ grabbing it and the engine hearing about it.
    //
    // **Only a hand.** §87's takeover is "a person touched this, so leave it
    // alone", and until the log could say whose an action was, this fired for
    // *every* action — including djmanzo's own. The automix moving a fader
    // marked that fader as held by the DJ for ten minutes, and `next_step`
    // refuses a control the takeover says is held, so the machine was handing
    // itself the controls it had just used. A whole section of §87 was being
    // triggered by the thing it exists to defer to.
    if by == dj_control::By::Hand {
        state.note_human_touch(&parsed);
    }

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

    // Recording is handled here and never forwarded: the engine cannot open a
    // file, so the action's *meaning* lives in the application. It is in the
    // vocabulary all the same, because a DJ starts a recording from whatever is
    // nearest — a controller button, a line at the top of a script, the
    // assistant — and ADR-0003 says all of those speak one language.
    if let Action::Mixer(dj_core::MixerAction::SetRecording(on)) = parsed {
        if on {
            let rate = state
                .registry()
                .get(dj_core::ParamId::Global(
                    dj_core::param::GlobalParam::SampleRate,
                ))
                .max(1.0) as u32;
            state.start_recording(rate)?;
        } else {
            state.stop_recording();
        }
        return Ok(());
    }

    // The emergency. Expanded here rather than in the engine because what
    // "safe" means is a decision about a performance, not about audio: see
    // `make_safe` for what it does and, more importantly, what it refuses to.
    if let Action::Mixer(dj_core::MixerAction::Safe) = parsed {
        return make_safe(state);
    }

    // A plugin parameter goes to the engine like everything else — but the
    // cached list the interface draws from lives here, and a slider that
    // snapped back on the next snapshot would be unusable.
    if let Action::Mixer(dj_core::MixerAction::Clap(dj_core::action::ClapChange::Param {
        id,
        value,
    })) = parsed
    {
        state.plugin().note_param(id, f64::from(value));
    }

    // Clearing the plugin is the application's business for the same reason
    // recording is: the engine can bypass it, but only the thread that owns the
    // instance can let go of it, and the processor has to come home first.
    if let Action::Mixer(dj_core::MixerAction::Clap(dj_core::action::ClapChange::Clear)) = parsed {
        let _ = state
            .bus()
            .send_command(dj_engine::Command::ClapInsert { processor: None });
        state.plugin().unload();
        return Ok(());
    }

    // Automix is handled here and never forwarded. The engine renders audio;
    // deciding *when* one track should give way to the next is a question about
    // playheads and a queue, and both of those live on this side. It is in the
    // vocabulary for the ADR-0003 reason: a DJ hands the mix over from whatever
    // is nearest, and a controller button must be able to do it.
    if let Action::Mixer(dj_core::MixerAction::Automix(change)) = parsed {
        let decks = automix_view(state);
        let plan = {
            let mut mix = state
                .automix()
                .lock()
                .map_err(|_| "automix is unavailable".to_owned())?;
            let plan = mix.apply(change, &decks);
            // Published from inside the lock, so a reader never sees the state
            // from before a change it has already been told about.
            publish_automix(state, &mix);
            plan
        };
        run_automix_plan(state, plan);
        return Ok(());
    }

    // Clearing a sampler slot is the same shape of thing: the name lives here,
    // and a slot emptied in the engine that kept its label here would show a
    // sample that is no longer loaded.
    if let Action::Mixer(dj_core::MixerAction::Sample {
        slot,
        change: dj_core::SampleChange::Clear,
    }) = parsed
    {
        let bank = state
            .registry()
            .get(dj_core::ParamId::Global(
                dj_core::param::GlobalParam::SamplerBank,
            ))
            .max(1.0) as u8;
        state.clear_sample_name(bank, slot);
    }

    // Saved loops live in the library with the track, so the host is the only
    // place that can look one up or store one. Intercepted for the same reason
    // as the grid edits below.
    if let Action::Deck { deck, action } = parsed {
        match action {
            dj_core::DeckAction::LoopSave(slot) => {
                save_loop(state, deck, slot)?;
                let _ = state.bus().dispatch_by(parsed, by);
                return Ok(());
            }
            dj_core::DeckAction::LoopRecall(slot) => {
                recall_loop(state, deck, slot)?;
                let _ = state.bus().dispatch_by(parsed, by);
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
        apply_grid_edit(state, deck, edit)?;
        // Dispatched *after* the edit has succeeded, so it lands in the session
        // log like every other action -- a grid the DJ moved mid-set is exactly
        // the kind of thing worth being able to look back at. The engine
        // ignores the action itself; it has already had the result as
        // `SetGrid`. A refused edit is not logged, because it did not happen.
        let _ = state.bus().dispatch_by(parsed, by);
        return Ok(());
    }

    state
        .bus()
        .dispatch_by(parsed, by)
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
    /// §75's phrase handle: move the boundary to the beat nearest this frame.
    ///
    /// The one edit here that leaves the *grid* alone. Every other one moves
    /// the beats, which is why they all clear the phrase — it counts from the
    /// old anchor and would point at the wrong beat afterwards. This moves the
    /// phrase itself, so clearing it would be the edit undoing itself.
    Phrase(dj_core::FramePos),
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
        A::GridPhrase(at) => GridEdit::Phrase(at),
        _ => return None,
    })
}

fn apply_grid_edit(state: &AppState, deck: DeckId, edit: GridEdit) -> Result<(), String> {
    use crate::grid;

    let waveforms = state.waveforms();
    let registry = state.registry();

    // §75's phrase handle. Handled before everything below because it is not a
    // grid edit at all: the beats stay exactly where they are and only the
    // boundary moves, so none of the tempo arithmetic applies and the phrase
    // must survive rather than be cleared.
    if let GridEdit::Phrase(at) = edit {
        let overlay = waveforms
            .grid(deck.human_number())
            .ok_or("no beat grid on this deck yet; wait for analysis or tap one in")?;
        let phrase = waveforms
            .grid(deck.human_number())
            .and_then(|o| o.phrase)
            .or_else(|| analysed_phrase(state, deck))
            .ok_or("this record has no phrase structure to move")?;
        let moved = grid::phrase_at(overlay.grid, phrase, at, overlay.sample_rate);
        waveforms.set_grid(
            deck,
            Some(dj_render::GridOverlay {
                lines: dj_render::GridLines::all(),
                phrase: Some(moved),
                ..overlay
            }),
        );
        save_phrase(state, deck, moved);
        return publish_grid(state, deck, Some(overlay.grid), Some(moved));
    }

    // Reset is the one edit that does not need an existing grid to work from --
    // and it is also how a deck whose grid was cleared gets the analyser's back.
    if let GridEdit::Reset = edit {
        let original = waveforms.analysed_grid(deck.human_number());
        waveforms.set_grid(deck, original);
        save_grid(state, deck, original.map(|o| o.grid));
        // Reset means "give me back what the analyser found", so the phrase
        // comes back with the grid it was measured against.
        return publish_grid(
            state,
            deck,
            original.map(|o| o.grid),
            analysed_phrase(state, deck),
        );
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
                    GridEdit::Tap | GridEdit::Reset | GridEdit::Phrase(_) => {
                        unreachable!("handled above")
                    }
                }
            }
        };

    waveforms.set_grid(
        deck,
        Some(dj_render::GridOverlay {
            lines: dj_render::GridLines::all(),
            grid: edited,
            sample_rate: rate,
            // Measured against the old anchor, so it no longer describes this
            // grid. Cleared here for the same reason it is cleared on the deck.
            phrase: None,
        }),
    );
    save_grid(state, deck, Some(edited));
    // No phrase: it counts beats from the *old* anchor, so against an edited
    // grid every marker would point at the wrong beat. Cleared rather than
    // recomputed here -- recomputing needs the audio, which is the background
    // analyser's job, not a keypress's.
    publish_grid(state, deck, Some(edited), None)
}

/// The phrase structure the analyser found for whatever is on this deck.
///
/// Read back from the library rather than kept in a second place: the deck
/// already knows its track, and a cached copy beside the grid is one more thing
/// that can disagree with the database after an edit.
fn analysed_phrase(state: &AppState, deck: DeckId) -> Option<dj_core::Phrase> {
    let track = {
        let tracks = state.deck_tracks();
        let map = tracks.lock().ok()?;
        map.get(&deck.human_number())?.id
    };
    let stored = state.library().get().ok()?.track(track).ok()??;
    dj_core::Phrase::new(
        stored.analysis.phrase_beats?,
        stored.analysis.phrase_anchor?,
    )
}

/// Keep a phrase edit, so the correction is still there next time.
///
/// Beside [`save_grid`] rather than inside it, because the two write different
/// columns and a phrase edit must not touch `grid_source`: marking the *grid*
/// as the DJ's own because they moved a phrase boundary would stop a later
/// re-analysis from improving a grid nobody had corrected.
fn save_phrase(state: &AppState, deck: DeckId, phrase: dj_core::Phrase) {
    let Some(track) = state.deck_track_id(deck) else {
        return;
    };
    let Ok(db) = state.library().get() else {
        return;
    };
    let stored = match db.track(track) {
        Ok(Some(found)) => found.analysis,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, "could not read the track to save its phrase");
            return;
        }
    };
    if let Err(error) = db.set_analysis(track, &stored.with_phrase(phrase)) {
        tracing::warn!(%error, "the phrase edit will not survive a restart");
    }
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

/// Send a grid and its phrase structure to the engine, which needs them for
/// sync, quantize, beat jump and phrase jump.
///
/// Both together: a phrase counts beats from the grid's anchor, so a phrase
/// sent beside a different grid points at the wrong beat. Editing a grid
/// therefore passes `None` for the phrase -- the old measurement no longer
/// describes the new grid, and the analyser will find it again.
fn publish_grid(
    state: &AppState,
    deck: DeckId,
    grid: Option<dj_core::Beatgrid>,
    phrase: Option<dj_core::Phrase>,
) -> Result<(), String> {
    // The one place every grid edit passes through, which is why the nudge is
    // here rather than at each of the four callers. The mix windows are beats
    // counted from this grid, so moving it moves them -- and `grid_confidence`
    // cannot carry that on its own: the *first* hand edit takes it to certain
    // and every edit after that leaves it there.
    state.marks_changed(deck);
    state
        .bus()
        .send_command(dj_engine::Command::SetGrid { deck, grid, phrase })
        .map_err(|_| "engine is not accepting commands; is a device open?".to_owned())
}

/// Read the current state directly.
///
/// The snapshot stream only emits on change (plus a slow heartbeat), so a UI
/// that has just mounted needs one synchronous read to paint itself rather than
/// waiting for the engine to do something.
#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> crate::Snapshot {
    snapshot_now(&state)
}

/// The same frame, for callers holding an `&AppState` rather than a `State`.
///
/// Extracted so §43's suggestion cap can read the attention budget the
/// interface is already drawing against. Deriving the budget a second way in
/// the rail would be exactly the thing §11 exists to stop: one context engine
/// underneath, not a copy of the judgement in each consumer.
pub fn snapshot_now(state: &AppState) -> crate::Snapshot {
    let bridge = state.bridge();
    let tracks = state.deck_tracks();
    let samples = state.sample_names();
    let recording = state.recording_state();
    crate::Snapshot::capture_all(
        &state.registry(),
        state.deck_count(),
        bridge.as_deref(),
        Some(state.analysis()),
        crate::snapshot::Names {
            decks: Some(&tracks),
            samples: Some(&samples),
        },
        Some(&recording),
    )
    // The engine's last answer rather than a fresh one: a panel asking for a
    // frame outside the pump must see what the interface is already showing,
    // not a second opinion computed from a slightly different moment.
    .with_session(state.night().read())
    // And §77's focus, for the same reason: a frame read outside the pump that
    // disagreed with the pump's about how quiet the interface should be would
    // be a panel painting itself one way and then being corrected on the next
    // tick.
    .with_focus(state.focus().lock().ok().and_then(|held| *held))
    // And §25's saved-loop generations, on the same terms: a panel that has
    // just mounted reads this frame synchronously, and one that came back
    // zeroed would re-ask for every deck's loops on the pump's next tick.
    .with_marks(Some(&state.marks()))
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
    /// Where this record could be left, from `plan::mix_out`.
    ///
    /// Here rather than on the snapshot because it is a property of the
    /// *record*: it changes when a deck loads or an analysis lands and at no
    /// other time, and this is the call the waveform already makes on exactly
    /// those two events. Sixty times a second for a number that changes twice
    /// a track would be the snapshot pump carrying furniture.
    pub mix_out: Option<MixOutInfo>,
    /// Where a mix into this record could begin, from `plan::mix_in`.
    ///
    /// The other half of `mix_out`, here for the same reasons and read from the
    /// same grid. A DJ reading two lanes sees where the outgoing record can be
    /// left and where the incoming one can be joined, which between them is the
    /// whole of what §25 means by a *likely* mix.
    pub mix_in: Option<MixInInfo>,
    /// §25's `saved-loops` layer: the loops this record has kept, in slot
    /// order.
    ///
    /// From the **library**, unlike the two bands above, and the difference is
    /// deliberate for once rather than an inconsistency. A mix window is beats
    /// counted from a grid, so it has to come from the grid the beat lines are
    /// drawn from or it sits a fraction of a beat off them. A saved loop is two
    /// frame positions in a file: editing the grid moves the lines and does not
    /// move the loop, which is exactly what a DJ who saved one expects.
    ///
    /// Empty for a deck with nothing on it, a record nobody has saved a loop
    /// in, and a build with no library — all three draw nothing, which is the
    /// honest answer to each.
    pub saved_loops: Vec<SavedLoopInfo>,
    /// §75's trajectory, and the breakdowns and drops in it.
    ///
    /// Here for the same reason `mix_out` is: it is a property of the record,
    /// it changes when a deck loads or an analysis lands, and this is the call
    /// the waveform already makes on exactly those two events. Sixty times a
    /// second for a curve that changes twice a track would be the snapshot
    /// pump carrying furniture.
    ///
    /// Empty for a deck with nothing on it, one still being analysed, and a
    /// record with no grid to count phrases against. The overview draws
    /// nothing for all three, which is the honest answer to each.
    pub trajectory: dj_analysis::energy::Trajectory,
    /// §116: what changes in this record, in order — a current coming in or
    /// going, a stretch that builds or settles. Read from the trajectory above
    /// rather than stored with it, so an analysis cached before this existed
    /// answers too.
    pub changes: Vec<dj_analysis::energy::Change>,
    /// Whether §110's spectrum is still being measured for this record.
    ///
    /// The waveform is drawn in the three bands until it lands, and the lane
    /// asks again while this is true: nothing else the lane watches changes
    /// when the colour arrives, so without it a record whose analysis came
    /// from the cache would stay in three bands until the next load.
    pub colour_pending: bool,
}

/// One of §25's `saved-loops`, as the waveform draws it.
#[derive(Debug, Clone, Serialize)]
pub struct SavedLoopInfo {
    /// Which slot recalls it — the number the DJ presses, drawn on the band.
    pub slot: u8,
    pub start_frame: f64,
    pub end_frame: f64,
    /// What the DJ called it, when they called it anything.
    pub label: Option<String>,
}

/// §25's `mix-in` layer, as the waveform draws it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MixInInfo {
    pub opens_frame: f64,
    pub closes_frame: f64,
    /// Whether the opening is a phrase boundary or merely a beat.
    pub on_phrase: bool,
    /// Whether the close is the record's own first drop or the planner's
    /// longest transition measured from the opening. The band says which,
    /// because they are not the same promise — see `plan::MixIn`.
    pub before_a_drop: bool,
}

/// §25's `mix-out` layer, as the waveform draws it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MixOutInfo {
    pub opens_frame: f64,
    pub closes_frame: f64,
    /// Whether the opening is a phrase boundary or merely a beat. The band
    /// says which, because they are not the same promise.
    pub on_phrase: bool,
}

#[tauri::command]
pub fn waveform_info(state: State<'_, AppState>, deck: u8) -> WaveformInfo {
    // From the analysis rather than from the waveform store, unlike
    // `mix_out` below, and the difference is deliberate: a mix-out band has
    // to line up with the beat lines beside it, and a breakdown is where the
    // music thins out. Editing the grid moves the lines and does not move the
    // breakdown.
    let trajectory = state
        .analysis()
        .for_deck(deck)
        .map(|found| found.trajectory.clone())
        .unwrap_or_default();
    WaveformInfo {
        deck,
        ready: state.waveforms().has_summary(deck),
        total_frames: state.waveforms().total_frames(deck).unwrap_or(0) as u64,
        epoch: state.waveforms().epoch(deck),
        colour_pending: state.waveforms().colour_pending(deck),
        mix_out: mix_out_of(&state, deck),
        mix_in: mix_in_of(&state, deck),
        saved_loops: saved_loops_of(&state, deck),
        changes: trajectory.changes(),
        trajectory,
    }
}

/// §116's melody line, as the lane draws it.
#[derive(Debug, Clone, Serialize)]
pub struct MelodyLine {
    /// Frames of the file between two points.
    pub frames_per_point: f64,
    /// Each point: its pitch in Hz and which of `colours` it is drawn in, or
    /// `null` where nothing periodic was found.
    pub points: Vec<Option<(f32, u8)>>,
    /// §110's spectrum as the theme draws it, lowest pitch first. The colour
    /// of a note is the colour its pitch has in the waveform under it — a
    /// table rather than a formula, so the formula stays in one place.
    pub colours: Vec<[u8; 3]>,
    /// The lowest and highest pitch the line is read in, Hz: the lane's floor
    /// and ceiling for it.
    pub range: (f32, f32),
}

/// §116: the strongest line of notes in a deck's record, where it is known.
///
/// Asked for on the waveform's epoch rather than carried on `waveform_info`:
/// three thousand points a record is not furniture to send each time the lane
/// checks whether its colour has landed.
#[tauri::command]
pub fn melody_line(state: State<'_, AppState>, deck: u8, theme: String) -> Option<MelodyLine> {
    let melody = state.waveforms().melody(deck)?;
    let palette = dj_render::Theme::from_slug(&theme)
        .unwrap_or(dj_render::Theme::Dark)
        .palette();
    Some(MelodyLine {
        frames_per_point: melody.frames_per_point,
        points: melody
            .hertz
            .iter()
            .map(|hz| hz.map(|hz| ((hz * 10.0).round() / 10.0, dj_render::spectrum_step(hz))))
            .collect(),
        colours: palette.spectrum_steps(),
        range: (
            dj_analysis::melody::LOWEST_HZ,
            dj_analysis::melody::HIGHEST_HZ,
        ),
    })
}

/// §116: a deck's rhythm on its grid, for the lane's drum-machine strip.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RhythmLine {
    /// The frame of the first step.
    pub first_frame: f64,
    pub frames_per_step: f64,
    /// Per step, the kick, snare and hat, 0 to 255.
    pub steps: Vec<[u8; 3]>,
}

/// §116: the rhythm of a deck's record, stepped against the grid the lane's
/// beat lines are drawn from — so a grid corrected by hand moves the steps
/// with the lines. `None` before the record has been read, or without a grid.
#[tauri::command]
#[must_use]
pub fn rhythm_line(state: State<'_, AppState>, deck: u8) -> Option<RhythmLine> {
    let store = state.waveforms();
    let onsets = store.onsets(deck)?;
    let overlay = store.grid(deck)?;
    let summary = store.summary(deck)?;
    let frames_per_beat = overlay.grid.bpm.beat_frames(overlay.sample_rate);
    #[allow(clippy::cast_precision_loss)]
    let steps = dj_analysis::rhythm::steps(
        &onsets,
        overlay.grid.anchor.get(),
        frames_per_beat,
        summary.total_frames() as f64,
    );
    Some(RhythmLine {
        first_frame: steps.first_frame,
        frames_per_step: steps.frames_per_step,
        steps: steps.strength,
    })
}

/// Where this deck's record could be left, when djmanzo knows enough to say.
///
/// **From the grid the tiles are drawn from**, not from the library's stored
/// analysis. Two reasons, and the second is why this was rewritten after
/// looking at the running application:
///
/// 1. The band has to line up with the beat lines beside it. Those are
///    rasterised from `WaveformStore`'s overlay, so anything drawn against a
///    different grid would sit a fraction of a beat off the lines it claims to
///    be a beat of — and a hand-edited grid would move the lines and leave the
///    band behind.
/// 2. A deck can have a grid the library row does not. The demo run showed
///    exactly that: both decks reading 123.7 BPM at full confidence, the
///    library rows still un-analysed, and no band on either lane. The first
///    version asked the library and drew nothing, and every browser test
///    passed because the harness answered the window itself.
///
/// `None` covers a deck with nothing on it, one still being analysed, and a
/// record too short to leave. The waveform draws no band for any of them,
/// which is the honest answer to all three.
/// The loops this deck's record has kept.
///
/// §25 has listed `saved-loops` since the table existed and nothing drew them,
/// which made saving one a thing a DJ could do and never see: eight slots, a
/// recall button per slot, and no way to know where any of them were without
/// pressing one.
///
/// Empty rather than `None` for every failure — no deck, no library, a
/// database that will not answer — because the three are the same thing to a
/// waveform and an `Option<Vec<_>>` would be two ways of drawing nothing.
///
/// They arrive in slot order, which is pad order, because the store selects
/// them that way.
fn saved_loops_of(state: &AppState, deck: u8) -> Vec<SavedLoopInfo> {
    let Some(id) = dj_core::DeckId::from_human(deck).and_then(|d| state.deck_track_id(d)) else {
        return Vec::new();
    };
    let Ok(db) = state.library().get() else {
        return Vec::new();
    };
    // Slot order is the store's — `Library::loops` selects `ORDER BY slot` —
    // and re-sorting here would be a second description of an ordering that
    // already has an owner. Mutation testing found exactly that: the sort was
    // written here first, and deleting it changed nothing because the query had
    // been doing the work all along. The test below holds the contract from
    // this side, so dropping the `ORDER BY` fails here rather than silently
    // putting the wrong number on a band.
    db.loops(id)
        .unwrap_or_default()
        .into_iter()
        .map(|region| SavedLoopInfo {
            slot: region.slot,
            start_frame: region.start_frame,
            end_frame: region.end_frame,
            label: region.label,
        })
        .collect()
}

/// Where a mix into this deck's record could begin.
///
/// Read from the deck's own grid for the two reasons `mix_out_of` gives at
/// length, and from the analysis for the drop — which is the same split
/// `waveform_info` already makes between the band and the trajectory: a band
/// has to line up with the beat lines beside it, and a drop is where the music
/// comes back. Editing the grid moves the lines and does not move the drop.
fn mix_in_of(state: &AppState, deck: u8) -> Option<MixInInfo> {
    let overlay = state.waveforms().grid(deck)?;
    let length = state.waveforms().total_frames(deck)?;
    // The record's own first drop, when something has found one. `None` here
    // is not an error: it is a record nobody has analysed, or one that never
    // drops, and `plan::mix_in` answers differently and says so.
    let first_drop = state
        .analysis()
        .for_deck(deck)
        .and_then(|found| found.trajectory.drops.first().copied());
    #[allow(clippy::cast_precision_loss)]
    let window = crate::plan::mix_in(
        &crate::plan::Record {
            length: length as f64,
            bpm: overlay.grid.bpm.get(),
            phrase: overlay.phrase,
            sample_rate: overlay.sample_rate,
            grid_anchor: overlay.grid.anchor.get(),
        },
        first_drop,
    )?;
    Some(MixInInfo {
        opens_frame: window.opens_frame,
        closes_frame: window.closes_frame,
        on_phrase: window.on_phrase,
        before_a_drop: window.before_a_drop,
    })
}

fn mix_out_of(state: &AppState, deck: u8) -> Option<MixOutInfo> {
    let overlay = state.waveforms().grid(deck)?;
    let length = state.waveforms().total_frames(deck)?;
    #[allow(clippy::cast_precision_loss)]
    let window = crate::plan::mix_out(&crate::plan::Record {
        length: length as f64,
        bpm: overlay.grid.bpm.get(),
        phrase: overlay.phrase,
        sample_rate: overlay.sample_rate,
        grid_anchor: overlay.grid.anchor.get(),
    })?;
    Some(MixOutInfo {
        opens_frame: window.opens_frame,
        closes_frame: window.closes_frame,
        on_phrase: window.on_phrase,
    })
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

/// One mix the night contains, as the interface shows it.
#[derive(Debug, Clone, Serialize)]
pub struct MixDto {
    /// Seconds into the set.
    pub at: f64,
    pub took_seconds: f64,
    /// How long it ran in beats, when the outgoing record's tempo is known.
    /// Absent rather than zero: a mix whose record has left the library has an
    /// unknown length, and nought beats is a different claim.
    pub beats: Option<f64>,
    pub out_deck: u8,
    pub in_deck: u8,
    /// What was on them. `None` where the record is no longer in the library,
    /// which is the one thing that can make a night's own list incomplete.
    pub out_title: Option<String>,
    pub in_title: Option<String>,
    pub style: String,
    /// How many times this exact pair has been kept — §24's confidence
    /// weight. Zero for most, which is what the button is for.
    pub kept: u32,
}

/// One control on §74's contextual rail.
#[derive(Debug, Clone, Serialize)]
pub struct AtHandControlDto {
    /// Which control this is, whatever it currently says — the slug §8's
    /// *preferred controls* are stored under.
    pub reach: String,
    pub label: String,
    /// The action, exactly as the parser accepts it — so pressing it is the
    /// same event as typing it or mapping a controller to it.
    pub action: String,
    pub on: bool,
    /// True when it is here because the DJ kept it rather than because djmanzo
    /// judged it relevant.
    pub kept: bool,
}

/// What is at hand on one deck.
#[derive(Debug, Clone, Serialize)]
pub struct AtHandDto {
    pub deck: u8,
    pub doing: String,
    /// Why this set and not another, in the DJ's own terms.
    pub because: String,
    pub controls: Vec<AtHandControlDto>,
}

/// The four to eight controls that matter on `deck` right now.
///
/// §74's contextual rail. The judgement is `crate::at_hand`'s, over the same
/// snapshot the interface is already drawing, so the rail cannot be showing a
/// deck the rest of the application is not.
///
/// Whether another record is audible is the one thing a deck cannot see from
/// itself, and it is the difference between *mixing* and merely *playing* — so
/// it is worked out here, where every deck is in reach, rather than guessed at
/// per deck.
#[tauri::command]
pub fn at_hand(state: State<'_, AppState>, deck: Option<u8>) -> Result<AtHandDto, String> {
    let snapshot = crate::Snapshot::capture(&state.registry(), state.deck_count());
    // No deck asked for means "wherever the hands are", which is the rail's
    // ordinary use: §41's `ui focus` fades after six seconds by design, so it
    // cannot be what a rail follows for a whole set.
    let deck = deck
        .or_else(|| crate::at_hand::busiest(&snapshot.decks))
        .unwrap_or(1);
    let view = snapshot
        .decks
        .iter()
        .find(|d| d.number == deck)
        .ok_or_else(|| format!("no deck {deck}"))?;
    let against = snapshot
        .decks
        .iter()
        .any(|other| other.number != deck && other.playing && other.pre_fader_level > 0.0);

    // §8 Level 1's *preferred controls*. Read here rather than inside
    // `at_hand` because that module judges a deck and nothing else: a
    // preferences file is the host's business, and a rail that read one would
    // be a rail that could not be tested without a config directory.
    let kept = crate::at_hand::keeping(&state.kept_controls());
    let hand = crate::at_hand::at_hand(view, against, &kept);
    Ok(AtHandDto {
        deck: hand.deck,
        doing: hand.doing.slug().to_owned(),
        because: hand.because.to_owned(),
        controls: hand
            .controls
            .into_iter()
            .map(|c| AtHandControlDto {
                reach: c.reach.name().to_owned(),
                label: c.label,
                action: c.action,
                on: c.on,
                kept: c.kept,
            })
            .collect(),
    })
}

/// One thing djmanzo has noticed you do, and where.
#[derive(Debug, Clone, Serialize)]
pub struct TendencyDto {
    /// The whole sentence, written in Rust — §13's rule is about the *words*
    /// as much as the counting, so the interface is not given the parts to
    /// assemble a claim out of.
    pub says: String,
    pub gesture: String,
    pub phase: String,
    pub seen: usize,
}

/// What tonight's gestures amount to, if anything.
///
/// §14's signals read through §13's rule. Nothing here is a preference: a
/// gesture becomes a tendency only after four occurrences **in one phase of
/// the night**, and the sentence it produces names that phase. "You sometimes
/// sweep the filter when the night is at its peak" is a thing djmanzo saw;
/// "you like filter sweeps" is a thing it would be inventing.
///
/// Empty is the ordinary answer for most of a set, and it is an answer: for
/// the first stretch nothing has read the night yet, so nothing can generalise
/// — which is `crate::night::phase_at` refusing to backdate a phase over the
/// part of the evening it could not see.
#[tauri::command]
pub fn learned_tendencies(state: State<'_, AppState>) -> Vec<TendencyDto> {
    let night = state.night();
    let log = state.bus().log();
    let signals = crate::signals::signals(&log, &|at| night.phase_at(at));
    crate::signals::tendencies(&signals)
        .into_iter()
        .map(|t| TendencyDto {
            says: t.words(),
            gesture: t.did().slug().to_owned(),
            phase: t.context().name().to_owned(),
            seen: t.seen(),
        })
        .collect()
}

/// Tonight, and what kind of night it is. §81.
#[derive(Debug, Clone, Serialize)]
pub struct SettingDto {
    /// A `Setting` slug, or `null` when the DJ has not said yet.
    pub setting: Option<String>,
    /// What has been read off tonight's log so far. Every one may be absent,
    /// and an absence means "nothing to say yet" rather than "nothing".
    pub density: Option<String>,
    pub style: Option<String>,
    pub posture: Option<String>,
    pub techniques: Vec<String>,
}

/// One conditional profile, as the interface draws it.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDto {
    pub setting: String,
    pub title: String,
    /// How many nights it rests on. Drawn, because three nights and thirty are
    /// not the same claim and a profile that hides the difference is asking to
    /// be over-trusted.
    pub nights: usize,
    pub density: Option<String>,
    pub style: Option<String>,
    pub automation: Option<String>,
    pub techniques: Vec<String>,
    /// Genre and its share of the plays, commonest first.
    pub genres: Vec<(String, f64)>,
    /// The sentence, written in Rust — see `crate::profile::Profile::words`.
    pub says: String,
}

/// Say what kind of night this is, and keep what has been read off it.
///
/// §81. Both halves in one call because they happen together: the moment a DJ
/// names the setting, everything already read off tonight's log belongs to it.
///
/// Called again as the night goes, with `setting` absent — the four figures
/// come off the action log and the log does not outlive the run that made it,
/// so they are written while they exist. An absent setting never overwrites a
/// named one; see `Library::note_night`.
///
/// `density` comes from the interface because the interface is the only thing
/// that knows it: the band is chosen from the window's own height. That is not
/// a second copy of anything Rust holds — it is the only copy.
///
/// # Errors
/// A setting djmanzo does not know, or whatever the database says.
#[tauri::command]
pub fn night_setting(
    state: State<'_, AppState>,
    setting: Option<String>,
    density: Option<String>,
) -> Result<SettingDto, String> {
    let setting = match setting.as_deref() {
        Some(word) => {
            Some(crate::setting::Setting::parse(word).ok_or_else(|| format!("no {word} setting"))?)
        }
        None => None,
    };

    // Everything read off tonight's log, now, while there is a log.
    let log = state.bus().log();
    let night = state.night();
    let signals = crate::signals::signals(&log, &|at| night.phase_at(at));
    let techniques: Vec<String> = crate::signals::tendencies(&signals)
        .into_iter()
        .map(|t| t.did().slug().to_owned())
        .collect();

    // The commonest way tonight's records were joined. `None` until there has
    // been a handover: a night with one record in it has no transition style,
    // and a confident answer there would be an invention.
    let mut styles: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for handover in crate::mixes::handovers(&log) {
        *styles.entry(handover.style.as_str()).or_default() += 1;
    }
    let style = styles
        .into_iter()
        .max_by_key(|(name, n)| (*n, *name))
        .map(|(name, _)| name.to_owned());

    let posture = state
        .conduct()
        .lock()
        .ok()
        .map(|conduct| conduct.posture.name().to_owned());

    let joined = techniques.join(",");
    let db = library(&state)?;
    db.note_night(
        &state.session_id(),
        setting.map(|s| s.slug()),
        dj_library::NightRead {
            density: density.as_deref(),
            style: style.as_deref(),
            posture: posture.as_deref(),
            techniques: (!joined.is_empty()).then_some(joined.as_str()),
        },
    )
    .map_err(|e| e.to_string())?;

    let stored = db
        .night(&state.session_id())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "the night was not written".to_owned())?;
    Ok(SettingDto {
        setting: Some(stored.setting),
        density: stored.density,
        style: stored.style,
        posture: stored.posture,
        techniques: stored
            .techniques
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
    })
}

/// Tonight's row, without writing anything.
///
/// `setting: null` when the DJ has not said what kind of night this is, which
/// is the state every night opens in and the one the panel exists to end.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn night_now(state: State<'_, AppState>) -> Result<SettingDto, String> {
    let db = library(&state)?;
    let stored = db.night(&state.session_id()).map_err(|e| e.to_string())?;
    Ok(match stored {
        Some(night) => SettingDto {
            setting: Some(night.setting),
            density: night.density,
            style: night.style,
            posture: night.posture,
            techniques: night
                .techniques
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        },
        None => SettingDto {
            setting: None,
            density: None,
            style: None,
            posture: None,
            techniques: Vec::new(),
        },
    })
}

/// One of §81's six kinds of night, as the panel offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NightSettingDto {
    /// The slug it is stored and spoken as.
    pub slug: String,
    /// What a DJ would call it.
    pub title: String,
    /// What kind of evening it is, in one line.
    pub about: String,
}

/// §81's six, read off the table that owns them.
///
/// The panel used to spell all eighteen of these strings itself. That is the
/// second-description failure this codebase keeps meeting: `Setting` is what
/// every other part of djmanzo files a night under, and a seventh occasion
/// added there would have left the panel offering six — with no error anywhere,
/// because a list that is merely short looks exactly like a list that is right.
#[tauri::command]
#[must_use]
pub fn night_settings() -> Vec<NightSettingDto> {
    crate::setting::Setting::ALL
        .iter()
        .map(|setting| NightSettingDto {
            slug: setting.slug().to_owned(),
            title: setting.title().to_owned(),
            about: setting.about().to_owned(),
        })
        .collect()
}

/// §81's conditional profiles: how this DJ plays, per kind of night.
///
/// Only the settings there is enough evidence for — see
/// `crate::profile::ENOUGH_NIGHTS`. A setting under the threshold is absent
/// rather than empty, because an interface draws an empty profile as one that
/// knows nothing about you, which is a different and much weaker claim than
/// one that does not exist yet.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn learned_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileDto>, String> {
    let db = library(&state)?;
    let mut nights = Vec::new();
    for setting in crate::setting::Setting::ALL {
        nights.extend(db.nights_in(setting.slug()).map_err(|e| e.to_string())?);
    }
    // One query per setting, and only for the settings a profile was built
    // for: asking for every genre table up front would be six queries to
    // answer a question about, usually, one.
    let genres =
        |setting: crate::setting::Setting| db.genres_in(setting.slug()).unwrap_or_default();
    Ok(
        crate::profile::profiles(&nights, &genres, crate::profile::now())
            .into_iter()
            .map(|p| ProfileDto {
                setting: p.setting().slug().to_owned(),
                title: p.setting().title().to_owned(),
                nights: p.nights(),
                density: p.density().map(ToOwned::to_owned),
                style: p.style().map(|s| s.as_str().to_owned()),
                automation: p.automation().map(|a| a.name().to_owned()),
                techniques: p.techniques().iter().map(|d| d.slug().to_owned()).collect(),
                genres: p.genres().to_vec(),
                says: p.words(),
            })
            .collect(),
    )
}

/// One of §80's four learnable traits, as the panel offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LearnedDto {
    /// The slug the verdict is stored under.
    pub slug: String,
    /// The claim, in §80's own register: a preference, not an observation.
    ///
    /// Empty for a trait djmanzo cannot claim — either because it has not seen
    /// enough, or because it cannot see the thing at all.
    pub says: String,
    /// The evidence, in the DJ's words. Empty where there is no claim.
    pub because: String,
    /// Why djmanzo cannot claim this at all. Empty where it could.
    pub why_not: String,
    /// `offered`, `accepted` or `rejected`.
    pub verdict: String,
}

/// What djmanzo believes about how this DJ plays, and what they said about it.
///
/// §80 asks for learned behaviour expressed as **editable preferences** — shown
/// as a learned preference, rejectable and modifiable. §13's tendencies and
/// §81's profiles already learn; what neither did is let a DJ disagree, and a
/// learned claim a DJ cannot reject is one djmanzo may go on acting on after
/// being told it is wrong.
///
/// All four of §80's rows come back, including the one that cannot be claimed
/// and the ones there is not enough evidence for yet: a list of what djmanzo
/// happens to believe today would read as the whole of §80, and a trait that is
/// merely quiet looks exactly like one that does not exist.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn learned_persona(state: State<'_, AppState>) -> Result<Vec<LearnedDto>, String> {
    let db = library(&state)?;
    let mut nights = Vec::new();
    for setting in crate::setting::Setting::ALL {
        nights.extend(db.nights_in(setting.slug()).map_err(|e| e.to_string())?);
    }
    let genres =
        |setting: crate::setting::Setting| db.genres_in(setting.slug()).unwrap_or_default();
    let profiles = crate::profile::profiles(&nights, &genres, crate::profile::now());
    // Tonight's own actions, for the one trait §81's profiles cannot carry:
    // which stem a DJ actually reaches for. `DeckAction::Stem` has always
    // carried it, and §14's gestures collapse all four into one on purpose —
    // that rule is right for §14 and wrong here.
    let actions: Vec<dj_core::Action> = state
        .bus()
        .log()
        .into_iter()
        .filter_map(|entry| match entry.event {
            dj_control::SessionEvent::Action(action) => Some(action),
            // Neither is an action. A load is how a record got here, and an
            // audition is a record the DJ listened to and did not load.
            dj_control::SessionEvent::Load { .. } | dj_control::SessionEvent::Auditioned { .. } => {
                None
            }
        })
        .collect();
    let claims = crate::persona::learned(&profiles, &actions);
    let verdicts = state.persona_verdicts();

    Ok(crate::persona::Trait::ALL
        .iter()
        .map(|which| {
            let verdict = verdicts
                .iter()
                .find(|(slug, _)| slug == which.slug())
                .map_or(crate::persona::Verdict::Offered, |(_, said)| {
                    crate::persona::Verdict::parse(said)
                });
            // A rejected claim is not raised again. The rule is
            // `persona::offering`, where it is tested, rather than an `if` here
            // — this command is the thin half.
            let claim = crate::persona::offering(&claims, *which, verdict);
            LearnedDto {
                slug: which.slug().to_owned(),
                says: claim.map(|c| c.says.clone()).unwrap_or_default(),
                because: claim.map(|c| c.because.clone()).unwrap_or_default(),
                why_not: which.why_not().to_owned(),
                verdict: verdict.slug().to_owned(),
            }
        })
        .collect())
}

/// Agree with a learned claim, or refuse it.
///
/// `accepted` lets djmanzo act on it; `rejected` stops it acting **and** stops
/// it raising the claim again. Either half alone is a rejection that did not
/// take: a DJ asked the same thing every week has not been heard, and one whose
/// "no" is recorded and then ignored has been lied to.
///
/// # Errors
/// A trait or a verdict djmanzo does not have.
#[tauri::command]
pub fn answer_persona(
    state: State<'_, AppState>,
    slug: String,
    verdict: String,
) -> Result<Vec<LearnedDto>, String> {
    let which = crate::persona::Trait::parse(&slug)
        .ok_or_else(|| format!("{slug:?} is not one of §80's traits"))?;
    // Parsed rather than stored raw, so a word djmanzo does not know cannot be
    // written to the file and read back later as something it does know.
    let said = match verdict.as_str() {
        "accepted" => crate::persona::Verdict::Accepted,
        "rejected" => crate::persona::Verdict::Rejected,
        "offered" => crate::persona::Verdict::Offered,
        other => return Err(format!("{other:?} is not an answer djmanzo understands")),
    };
    state.set_persona_verdict(which.slug(), said.slug());
    learned_persona(state)
}

/// Keep one of tonight's mixes: §24's "Save this transition".
///
/// The only thing written to `kept_pairs`. Every mix a night contained is
/// already derivable from the action log — `crate::mixes` does it — so
/// recording those too would be a second copy that eventually disagrees with
/// the log it came from. What cannot be derived is that the DJ thought one was
/// worth keeping.
///
/// Identified by when it happened rather than by an index, because the list is
/// newest-first in the interface and re-derived on every read: an index would
/// name a different mix the moment another one finished.
///
/// # Errors
/// When no mix began at that moment, when either record has left the library,
/// or whatever the database says. Named rather than swallowed: a *Keep* that
/// silently kept nothing is worse than one that failed, because a DJ finds out
/// weeks later when the pair never comes back.
#[tauri::command]
pub fn keep_mix(state: State<'_, AppState>, at: f64) -> Result<u32, String> {
    let db = library(&state)?;
    let found = crate::mixes::handovers(&state.bus().log())
        .into_iter()
        // The same tolerance the interface's own rounding needs: `at` makes a
        // round trip through a float in JSON and comes back a hair off.
        .find(|mix| (mix.began.as_secs_f64() - at).abs() < 0.01)
        .ok_or_else(|| format!("no mix began at {at:.0}s"))?;

    let (Some(from), Some(into)) = (found.out_track, found.in_track) else {
        return Err("that mix has a record djmanzo cannot name".to_owned());
    };
    // Beats need the outgoing record's tempo, which the log does not hold.
    let beats = db
        .track(from)
        .ok()
        .flatten()
        .and_then(|t| t.analysis.bpm)
        .and_then(|bpm| found.beats(bpm));

    // §24's "works only with an 8-beat loop". The loop is read off the same log
    // the mix itself was read off — see `mixes::Handover::loop_beats` — so a
    // set recorded long before this column existed still answers it.
    db.keep_pair(
        from,
        into,
        Some(found.style.as_str()),
        beats,
        found.loop_beats.map(f64::from),
    )
    .map_err(|e| e.to_string())
}

/// The mixes tonight, read back out of the action log.
///
/// §67 says the session contains transitions and §68 says the transition
/// object should drive practice and replay. `crate::mixes` derives them rather
/// than recording them — see that module for why — so this works on a set
/// recorded long before any of it existed.
///
/// The tempo and the titles come from the library here rather than in
/// `mixes`, which has no business knowing what a library is: the log holds
/// track ids and durations, and turning those into "42 beats" and a name is
/// the job of the layer that can look them up.
#[tauri::command]
pub fn session_mixes(state: State<'_, AppState>) -> Vec<MixDto> {
    let db = library(&state).ok();
    let named = |id: Option<dj_core::TrackId>| -> (Option<String>, Option<f64>) {
        let Some(track) = id.and_then(|id| db.as_ref()?.track(id).ok().flatten()) else {
            return (None, None);
        };
        (Some(track.display_title()), track.analysis.bpm)
    };

    crate::mixes::handovers(&state.bus().log())
        .into_iter()
        .map(|mix| {
            let (out_title, bpm) = named(mix.out_track);
            let (in_title, _) = named(mix.in_track);
            MixDto {
                at: mix.began.as_secs_f64(),
                took_seconds: mix.took().as_secs_f64(),
                beats: bpm.and_then(|bpm| mix.beats(bpm)),
                out_deck: mix.out.human_number(),
                in_deck: mix.into.human_number(),
                out_title,
                in_title,
                style: mix.style.as_str().to_owned(),
                kept: match (mix.out_track, mix.in_track, db.as_ref()) {
                    (Some(from), Some(into), Some(db)) => db.pair_kept(from, into).unwrap_or(0),
                    _ => 0,
                },
            }
        })
        .collect()
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
        .map(|entry| format!("{:>8.3}  {}", entry.at.as_secs_f64(), entry.event.to_line()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §22's estimate and §27's ghost are the same plan, and the phrase they
    /// are drawn as is written once.
    mod transition_estimate {
        use super::super::{estimate_transition, transition_words};
        use dj_core::{Mode, MusicalKey, Phrase, SampleRate};

        const SR: SampleRate = SampleRate::DEFAULT;
        const BPM: f64 = 120.0;

        fn beat() -> f64 {
            SR.as_f64() * 60.0 / BPM
        }

        fn outgoing() -> crate::plan::Outgoing {
            crate::plan::Outgoing {
                position: 64.0 * beat(),
                length: 256.0 * beat(),
                bpm: BPM,
                phrase: Phrase::new(16, 0),
                key: MusicalKey::new(8, Mode::Minor),
                sample_rate: SR,
                grid_anchor: 0.0,
            }
        }

        fn candidate(analysed: bool) -> dj_library::LibraryTrack {
            dj_library::LibraryTrack {
                id: dj_core::TrackId::from_bytes([7u8; 32]),
                path: std::path::PathBuf::from("/music/a.flac"),
                tags: dj_library::Tags::default(),
                duration_frames: 300 * 48_000,
                sample_rate: SR,
                channels: 2,
                file_size: None,
                file_modified: None,
                added_at: 0,
                analysis: if analysed {
                    dj_library::StoredAnalysis {
                        bpm: Some(BPM),
                        grid_anchor: Some(0.0),
                        grid_beats_per_bar: Some(4),
                        grid_confidence: Some(0.9),
                        phrase_beats: Some(16),
                        phrase_anchor: Some(0),
                        phrase_confidence: Some(0.9),
                        ..dj_library::StoredAnalysis::default()
                    }
                } else {
                    dj_library::StoredAnalysis::default()
                },
                stats: dj_library::PlayStats::default(),
                colour: None,
            }
        }

        /// **The rail shows the mix djmanzo would actually perform.**
        ///
        /// §22 asks for the estimated transition type, and the only version of
        /// that worth having is the planner's own. A rail estimating for
        /// itself would put one style beside a record and perform another the
        /// moment it was loaded.
        #[test]
        fn the_rail_shows_the_planners_own_answer() {
            let out = outgoing();
            let track = candidate(true);
            let estimate = estimate_transition(&out, &track, None).expect("a plan");

            let ghost = crate::ghost::look(
                &out,
                &crate::ghost::Candidate {
                    bpm: BPM,
                    phrase: Phrase::new(16, 0),
                    key: None,
                    sample_rate: SR,
                    grid_anchor: 0.0,
                    drops: Vec::new(),
                    voice_enters: None,
                },
                None,
            )
            .expect("a ghost");

            assert_eq!(estimate.style, ghost.plan.style.as_str());
            assert_eq!(estimate.length_beats, ghost.plan.length_beats);
            assert!(
                (estimate.at_seconds - ghost.plan.start_frame / SR.as_f64()).abs() < 1e-9,
                "the rail and the ghost disagree about where the mix starts"
            );
        }

        /// **An unanalysed record gets no line rather than a borrowed tempo.**
        ///
        /// The planner will happily plan against any tempo it is handed, so
        /// falling back to the outgoing record's would put a confident
        /// "32-beat blend" beside a record nobody has analysed — which reads
        /// exactly like one that has been.
        #[test]
        fn a_record_with_no_grid_gets_no_estimate() {
            assert!(estimate_transition(&outgoing(), &candidate(false), None).is_none());
        }

        /// A record already past its last usable phrase gets none either, for
        /// the same reason `plan::plan` answers `None` there.
        #[test]
        fn a_record_with_no_room_left_gets_no_estimate() {
            let mut nearly_over = outgoing();
            nearly_over.position = nearly_over.length - 2.0 * beat();
            assert!(estimate_transition(&nearly_over, &candidate(true), None).is_none());
        }

        /// The phrase is written once, and it is the one a DJ reads.
        #[test]
        fn the_phrase_says_the_length_the_style_and_the_time() {
            assert_eq!(
                transition_words(32, "blend", 129.0),
                "32-beat blend at 2:09"
            );
            // Negative seconds cannot happen and must not print as `0:-9`.
            assert_eq!(transition_words(8, "cut", -1.0), "8-beat cut at 0:00");
        }
    }

    /// §12: the rail consulting tonight's profile.
    mod ranked_by_profile {
        use super::super::with_profile;

        fn suggestion(byte: u8, score: f64) -> dj_library::suggest::Suggestion {
            dj_library::suggest::Suggestion {
                track: dj_core::TrackId::from_bytes([byte; 32]),
                score,
                reasons: Vec::new(),
            }
        }

        fn profile(plays: &[(&str, u32)]) -> crate::profile::Profile {
            let counted: Vec<(String, u32)> = plays
                .iter()
                .map(|(name, n)| ((*name).to_owned(), *n))
                .collect();
            let nights: Vec<dj_library::Night> = (0..crate::profile::ENOUGH_NIGHTS)
                .map(|n| dj_library::Night {
                    session_id: format!("w{n}"),
                    setting: "wedding".to_owned(),
                    began_at: 0,
                    density: None,
                    style: None,
                    posture: None,
                    techniques: None,
                })
                .collect();
            crate::profile::profiles(&nights, &|_| counted.clone(), 0)
                .into_iter()
                .next()
                .expect("enough nights")
        }

        /// **A lifted score moves the row.**
        ///
        /// The decision this function exists for. A rail that added to a
        /// score without re-sorting would show a higher number further down
        /// the list, which reads as a bug in the ranking rather than as the
        /// profile doing its job.
        #[test]
        fn a_record_the_profile_prefers_moves_up_the_rail() {
            // Two records the scorer put a hair apart, and a wedding that is
            // almost all bachata.
            let ranked = vec![(suggestion(1, 5.0), None), (suggestion(2, 4.9), None)];
            let genre = |id: dj_core::TrackId| {
                (id == dj_core::TrackId::from_bytes([2u8; 32])).then(|| "Bachata".to_owned())
            };
            let out = with_profile(
                ranked,
                Some(&profile(&[("Bachata", 90), ("Merengue", 10)])),
                &genre,
            );

            assert_eq!(
                out[0].0.track,
                dj_core::TrackId::from_bytes([2u8; 32]),
                "the profile's preference did not reach the top of the rail"
            );
            assert!(out[0].1.is_some(), "it moved without saying why");
            assert_eq!(
                out[1].1, None,
                "a record it said nothing about got a reason"
            );
        }

        /// **A profile with no genres moves nothing, so it claims nothing.**
        ///
        /// Found in the running application: a wedding profile built from
        /// nights whose plays carry no genre is a real profile — it says how
        /// this DJ mixes at weddings — and it tilts nothing, because the tilt
        /// is entirely a genre leaning. The rail said "Ranked for tonight"
        /// over a ranking it had not touched.
        #[test]
        fn a_profile_that_cannot_tilt_does_not_reorder_anything() {
            let ranked = vec![(suggestion(1, 5.0), None), (suggestion(2, 4.9), None)];
            let no_genres = profile(&[]);
            assert!(no_genres.genres().is_empty());
            let out = with_profile(ranked, Some(&no_genres), &|_| Some("Bachata".to_owned()));
            assert_eq!(out[0].0.track, dj_core::TrackId::from_bytes([1u8; 32]));
            assert_eq!(out[0].0.score, 5.0, "a score moved with nothing to move it");
            assert!(out.iter().all(|(_, because)| because.is_none()));
        }

        /// **With no profile, nothing moves and nothing is claimed.**
        ///
        /// §81's settings are told, never inferred, so a night the DJ has not
        /// named ranks exactly as it did before any of this existed.
        #[test]
        fn a_night_nobody_has_named_ranks_as_it_always_did() {
            let ranked = vec![(suggestion(1, 5.0), None), (suggestion(2, 4.9), None)];
            let out = with_profile(ranked, None, &|_| Some("Bachata".to_owned()));
            assert_eq!(out[0].0.track, dj_core::TrackId::from_bytes([1u8; 32]));
            assert_eq!(
                out[0].0.score, 5.0,
                "a score moved with no profile to move it"
            );
            assert!(out.iter().all(|(_, because)| because.is_none()));
        }

        /// **It cannot lift a record over one that actually mixes.**
        ///
        /// The bound, seen from the rail rather than from the arithmetic. A
        /// key clash is minus two and a half; three quarters of a point
        /// cannot cross that, so the worst a profile can do is reorder
        /// records that would all work.
        #[test]
        fn it_cannot_promote_a_record_that_does_not_mix() {
            // **Ten genres, not two.** With two, an even split is a half and
            // the raw tilt cannot exceed one whatever the shares are — so a
            // two-genre fixture passes this with the bound removed and proves
            // nothing. Across ten, a genre with nine tenths of the plays is
            // nine times an even split, and log2(9) is over three: enough to
            // cross the gap below if the bound were not there.
            let mut plays = vec![("Bachata", 900u32)];
            for other in [
                "Merengue", "Salsa", "Cumbia", "Son", "Vals", "Tango", "Bolero", "Danzon",
                "Guaracha",
            ] {
                plays.push((other, 11));
            }
            let ranked = vec![(suggestion(1, 5.0), None), (suggestion(2, 2.0), None)];
            let genre = |id: dj_core::TrackId| {
                (id == dj_core::TrackId::from_bytes([2u8; 32])).then(|| "Bachata".to_owned())
            };
            let out = with_profile(ranked, Some(&profile(&plays)), &genre);
            assert_eq!(
                out[0].0.track,
                dj_core::TrackId::from_bytes([1u8; 32]),
                "a profile promoted a record three points behind"
            );
        }
    }

    mod waveform_layers {
        /// Every Svelte file that draws part of the waveform, with line endings
        /// normalised.
        ///
        /// The `\r\n` is not paranoia: a scan of source text for a closing brace
        /// at column zero passed on every machine here and failed only on
        /// Windows CI, where git checks the repository out with CRLF.
        fn strips() -> String {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src");
            ["Waveform.svelte", "Overview.svelte"]
                .iter()
                .map(|name| {
                    let path = root.join(name);
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()))
                        .replace("\r\n", "\n")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        /// **The load-bearing one: every layer the picker lets a DJ turn off is
        /// one the waveform can actually stop drawing.**
        ///
        /// The house pattern, pointed at the join this feature lives across.
        /// The two halves are drawn by two different things — the grid is
        /// rasterised in Rust and travels in the tile URL, the rest are
        /// elements in the interface — so a layer can be offered in the picker
        /// and gated in neither, and the box would tick and nothing would
        /// happen. That is the worst failure available here: a DJ would
        /// conclude the whole picker is decorative.
        #[test]
        fn every_layer_a_dj_can_turn_off_is_one_the_strip_can_hide() {
            let source = strips();
            // The three the tile carries. They reach the renderer through the
            // URL rather than through an `{#if}`, so what proves them is the
            // slug being built from the chosen set.
            let in_the_tile = ["beats", "downbeats", "phrases"];
            assert!(
                source.contains("gridSlug(remembers.layers)"),
                "the tile URL no longer carries the chosen grid layers, so the \
                 three grid boxes would tick and change nothing"
            );

            for layer in dj_render::layers().iter().filter(|l| l.choosable()) {
                if in_the_tile.contains(&layer.name) {
                    continue;
                }
                let gate = format!("showing(\"{}\")", layer.name);
                assert!(
                    source.contains(&gate),
                    "`{}` can be turned off in the picker and nothing in the \
                     strip consults it, so the box ticks and the layer stays",
                    layer.name
                );
            }
        }

        /// **And the three the tile draws are exactly the three the slug has
        /// letters for.**
        ///
        /// `gridSlug` spells `bdp` and `GridLines` parses it. A fourth grid
        /// layer added to §25's table and left out of the slug would be a
        /// preference stored, sent, and silently dropped at the URL.
        #[test]
        fn the_grid_slug_covers_every_grid_layer_the_tile_draws() {
            let api = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/api.ts");
            let source = std::fs::read_to_string(&api)
                .unwrap_or_else(|e| panic!("could not read {}: {e}", api.display()))
                .replace("\r\n", "\n");
            for layer in ["beats", "downbeats", "phrases"] {
                assert!(
                    source.contains(&format!("on(\"{layer}\"")),
                    "`{layer}` is a grid layer and the tile URL has no letter \
                     for it"
                );
            }
        }
    }

    mod command_palette {
        use super::super::{PALETTE_LIMIT, PaletteEntryDto, matches, offered};

        /// §115's switches as a fresh install has them: the shipped
        /// activities, the shipped workspaces and the built-in preset packs.
        fn shipped_switches(decks: u8) -> Vec<PaletteEntryDto> {
            super::super::switches(
                &crate::activity::shipped(),
                &crate::cockpit::workspaces(),
                &dj_presets::builtin::packs(),
                decks,
            )
        }

        fn switched(query: &str, room_for: crate::tiers::Tier) -> Vec<PaletteEntryDto> {
            super::super::offered_with(query, 2, room_for, &shipped_switches(2)).entries
        }

        /// **Every preset is reachable by its name from the one gesture** —
        /// a theme, an activity, a workspace and a preset pack each found by
        /// typing a few letters of what the DJ calls it.
        #[test]
        fn every_kind_of_preset_is_one_search_away() {
            let open = crate::tiers::Tier::Preparation;
            let first = |query: &str| {
                switched(query, open)
                    .into_iter()
                    .find(|entry| entry.kind == "switch")
                    .map(|entry| (entry.label, entry.run))
            };
            assert_eq!(
                first("theme aurora"),
                Some((
                    "Theme \u{b7} Aurora".to_owned(),
                    "theme pkg-aurora".to_owned()
                ))
            );
            let (label, run) = first("activity dig").expect("the dig activity");
            assert_eq!(run, "activity dig", "{label}");
            let (_, run) = first("workspace club").expect("a club workspace");
            assert!(run.starts_with("workspace "), "{run}");
            let (label, run) = first("preset").expect("a preset");
            assert!(run.starts_with("preset "), "{label}: {run}");
        }

        /// **And only what a picker has**: each switch names a theme that
        /// ships (and is not a world, whose picker opens it), an activity, a
        /// workspace, or a preset the library resolves for that deck.
        #[test]
        fn every_switch_names_something_djmanzo_has() {
            let library = dj_presets::PresetLibrary::builtin();
            let workspaces: Vec<String> = crate::cockpit::workspaces()
                .into_iter()
                .map(|w| w.name)
                .collect();
            let slugs: Vec<String> = crate::activity::shipped()
                .into_iter()
                .map(|a| a.slug)
                .collect();
            let switches = shipped_switches(4);
            for kind in ["theme", "activity", "workspace", "preset"] {
                assert!(
                    switches
                        .iter()
                        .any(|e| e.run.starts_with(&format!("{kind} "))),
                    "no {kind} is offered"
                );
            }
            for entry in &switches {
                assert_eq!(entry.kind, "switch");
                let (kind, which) = entry.run.split_once(' ').unwrap();
                match kind {
                    "theme" => assert!(
                        crate::theme::ALL
                            .iter()
                            .any(|t| t.pack == Some(which) && !t.world),
                        "{which} is not a theme that ships"
                    ),
                    "activity" => assert!(slugs.iter().any(|s| s == which), "{which}"),
                    "workspace" => assert!(workspaces.iter().any(|w| w == which), "{which}"),
                    "preset" => {
                        let (id, deck) = which.split_once(' ').unwrap_or((which, "1"));
                        let deck: u8 = deck.parse().unwrap();
                        assert!(library.resolve(id, deck).is_ok(), "{which}");
                    }
                    other => panic!("a switch of kind {other}"),
                }
            }
        }

        /// Mid-mix the arrangement kinds are paperwork and stay out of the
        /// list, unless they are what was asked for by name.
        #[test]
        fn mid_mix_a_theme_answers_only_when_asked_for() {
            // The tier itself, because a one-letter query fills its twelve
            // with deck verbs before any theme is reached, and would pass
            // whatever tier a theme had.
            for entry in shipped_switches(2) {
                let arrangement =
                    entry.run.starts_with("theme ") || entry.run.starts_with("workspace ");
                assert!(
                    !super::super::tier_of(&entry).survives_a_mix(),
                    "{} survives a mix",
                    entry.label
                );
                if arrangement {
                    assert_eq!(
                        entry.tier,
                        crate::tiers::Tier::Preparation.name(),
                        "{}",
                        entry.label
                    );
                }
            }
            let mixing = crate::cockpit::Attention::performing().room_for;
            assert!(
                !switched("s", mixing)
                    .iter()
                    .any(|e| e.run.starts_with("theme ")),
                "a theme was offered mid-mix to a query that did not ask for one"
            );
            assert!(
                switched("theme signal", mixing)
                    .iter()
                    .any(|e| e.run == "theme pkg-signal"),
                "a theme asked for by name mid-mix was refused"
            );
        }

        /// The palette with the whole hierarchy in reach.
        ///
        /// Every test below is about the ranking and the cut rather than about
        /// §18's budget, so they ask for the budget that removes nothing —
        /// which is the one the interface has whenever nobody is mixing, and
        /// therefore the one these were all written against.
        fn palette(query: String, decks: u8) -> Vec<PaletteEntryDto> {
            offered(&query, decks, crate::tiers::Tier::Preparation).entries
        }

        /// **The palette can only offer what djmanzo actually has.**
        ///
        /// The whole point of generating it from `dj_core::vocabulary` and
        /// `cockpit::surfaces()` rather than writing a list: every entry that
        /// says it sends an action must send one the parser accepts. A
        /// hand-written palette drifts the first time a verb is renamed, and
        /// the failure is a DJ pressing something mid-set and getting an
        /// error.
        #[test]
        fn every_action_it_offers_parses() {
            for entry in palette(String::new(), 4) {
                if entry.kind != "action" {
                    continue;
                }
                assert!(
                    dj_core::Action::parse(&entry.run).is_ok(),
                    "the palette offers {:?} ({}), which the parser refuses",
                    entry.run,
                    entry.label,
                );
            }
        }

        /// **And every surface it offers is a surface that exists.**
        #[test]
        fn every_surface_it_offers_is_real() {
            let known: Vec<&str> = crate::cockpit::surfaces().iter().map(|s| s.name).collect();
            for entry in palette("show".to_owned(), 2) {
                if entry.kind != "surface" {
                    continue;
                }
                assert!(
                    known.contains(&entry.run.as_str()),
                    "the palette offers a surface called {:?}, which is not one",
                    entry.run,
                );
            }
        }

        /// **What you typed comes first, when it is a real action.**
        ///
        /// This is the tier that makes the palette the semantic interface §51
        /// asks for rather than a menu: the verbs that take an argument -- a
        /// loop length, a key shift, a pitch -- can only be reached by typing
        /// them, because a list of buttons would have to invent the number.
        #[test]
        fn a_typed_action_is_the_first_answer() {
            let out = palette("deck 2 loop 8".to_owned(), 2);
            assert!(!out.is_empty(), "typing a real action offered nothing");
            assert_eq!(out[0].run, "deck 2 loop 8");
            assert_eq!(out[0].kind, "action");
        }

        /// **Nonsense offers nothing rather than something wrong.**
        #[test]
        fn a_query_that_is_not_an_action_is_not_offered_as_one() {
            let out = palette("deck 9 explode".to_owned(), 2);
            assert!(
                !out.iter().any(|e| e.run == "deck 9 explode"),
                "the palette offered to run something the parser refuses",
            );
        }

        /// **Only the decks in use.**
        ///
        /// A two-deck rig should not be offered deck 5. The count is the rig's,
        /// not the maximum the engine supports.
        #[test]
        fn it_offers_the_decks_the_dj_actually_has() {
            let two = palette("play".to_owned(), 2);
            assert!(
                two.iter().any(|e| e.run == "deck 2 play"),
                "deck 2 was not offered to a two-deck rig",
            );
            assert!(
                !two.iter().any(|e| e.run.starts_with("deck 3")),
                "a two-deck rig was offered a third deck",
            );
        }

        /// **A palette is read, not scrolled.**
        /// **The load-bearing one for §58: the hands come before the
        /// paperwork, and the cut takes the paperwork.**
        ///
        /// A palette is read, not scrolled, so twelve entries is the whole of
        /// it — and before this the twelve were whichever twelve the passes
        /// happened to generate first. A DJ who typed three letters mid-mix
        /// could be offered *Pin the Journal* above *deck 2 cue*, and on a
        /// six-deck layout the cut could take the performing controls off the
        /// bottom entirely.
        ///
        /// Asserted on the ranks rather than on particular rows, because the
        /// claim is the ordering and a test naming two entries would pass a
        /// palette that happened to put those two in order and everything else
        /// backwards.
        #[test]
        fn the_palette_offers_the_hands_before_the_paperwork() {
            use crate::tiers::Tier;
            let rank = |entry: &super::super::PaletteEntryDto| {
                Tier::ALL
                    .into_iter()
                    .find(|tier| tier.name() == entry.tier)
                    .unwrap_or_else(|| panic!("`{}` has no tier", entry.label))
                    .rank()
            };

            // **Queries chosen because they break without the sort.**
            //
            // Most do not, and that is worth writing down: the passes below
            // generate verbs before surfaces before §41's operations, which is
            // *already* roughly §58's order, so a test on a convenient query
            // passes against an unsorted palette. These two interleave —
            // `ss` puts a surface, then a pin, then a surface, then three
            // channel-strip verbs — and the first version of this test used `e`
            // and survived deleting the sort entirely.
            for query in ["ss", "sa", "st", "rs"] {
                let offered = palette(query.to_owned(), 4);
                assert!(offered.len() > 3, "{query:?} matches too little to rank");
                let ranks: Vec<u8> = offered.iter().map(rank).collect();
                assert!(
                    ranks.windows(2).all(|pair| pair[0] <= pair[1]),
                    "{query:?} is not ordered by §58's hierarchy: {:?}",
                    offered
                        .iter()
                        .map(|e| (e.label.as_str(), e.tier))
                        .collect::<Vec<_>>()
                );
            }

            // And the cut takes the far end rather than whatever was last.
            let crowded = palette("s".to_owned(), 6);
            assert_eq!(crowded.len(), PALETTE_LIMIT);
            assert!(
                crowded.iter().map(rank).all(|rank| rank <= 2),
                "the cut kept preparation entries while dropping performing \
                 ones, which is the failure §58 exists to rank away: {:?}",
                crowded
                    .iter()
                    .map(|e| (e.label.as_str(), e.tier))
                    .collect::<Vec<_>>()
            );
        }

        /// The operations are placed where §58 puts them, checked directly.
        ///
        /// An ordering test cannot see this: a pin mis-ranked as *performable*
        /// still sorts before the surfaces it is generated after, so the list
        /// stays in order and the rank is wrong. Written out, so the table can
        /// be read against the section.
        #[test]
        fn an_arrangement_gesture_is_paperwork_and_a_deck_gesture_is_not() {
            let tier_of = |needle: &str, label_starts: &str| {
                palette(needle.to_owned(), 2)
                    .into_iter()
                    .find(|entry| entry.label.starts_with(label_starts))
                    .unwrap_or_else(|| panic!("the palette no longer offers {label_starts:?}"))
                    .tier
            };
            assert_eq!(
                tier_of("pin library", "Pin "),
                crate::tiers::Tier::Preparation.name(),
                "pinning a panel ranks as something done with a record"
            );
            assert_eq!(
                tier_of("focus deck 1", "Focus deck"),
                crate::tiers::Tier::Performable.name(),
                "marking a deck ranks as paperwork"
            );
            assert_eq!(
                tier_of("deck 1 play", "Deck 1"),
                crate::tiers::Tier::Glanceable.name()
            );
        }

        /// Every entry carries a tier, and it is one of §58's four.
        #[test]
        fn every_entry_the_palette_offers_is_placed_in_the_hierarchy() {
            for entry in palette(String::new(), 6)
                .into_iter()
                .chain(palette("s".to_owned(), 2))
                .chain(palette("deck 1 play".to_owned(), 2))
            {
                assert!(
                    crate::tiers::Tier::ALL
                        .iter()
                        .any(|tier| tier.name() == entry.tier),
                    "`{}` is offered with tier {:?}, which is not one of §58's",
                    entry.label,
                    entry.tier
                );
            }
        }

        #[test]
        fn it_stops_at_a_readable_number() {
            assert!(palette(String::new(), 6).len() <= PALETTE_LIMIT);
        }

        /// **The load-bearing one: mid-mix the palette offers the hands and
        /// nothing else, and says so.**
        ///
        /// §18's `Attention::performing` has said *"Tier 1 and 2 only; nothing
        /// else may take room"* since it was written. §58 gave the palette a
        /// ranking; this is the budget actually spending it. The order matters
        /// as much as the rule: cutting to twelve first and dropping the
        /// paperwork afterwards would leave a DJ mid-mix with four rows,
        /// because eight of their twelve were tags and settings.
        #[test]
        fn mid_mix_the_palette_offers_only_what_a_hand_needs() {
            // `sh` and not a letter picked for looking plausible. Every
            // single-character query already fills its twelve with tiers 1 and
            // 2, because the vocabulary is mostly deck verbs — so a test
            // written against one of those is green whether the rule is there
            // or not. `sh` matches *Show <surface>* for every surface djmanzo
            // has, which is eight rows of paperwork in the visible twelve.
            // Before trusting a cut, find the input that is wrong without it.
            let mixing = crate::cockpit::Attention::performing().room_for;
            let quiet = offered("sh", 6, mixing);
            assert!(!quiet.entries.is_empty(), "the palette went dark mid-mix");
            for entry in &quiet.entries {
                assert!(
                    super::tier_of(entry).survives_a_mix(),
                    "`{}` is {} and was offered during a mix",
                    entry.label,
                    entry.tier
                );
            }
            let loud = offered("sh", 6, crate::tiers::Tier::Preparation);
            assert!(
                loud.entries
                    .iter()
                    .filter(|entry| !super::tier_of(entry).survives_a_mix())
                    .count()
                    >= 4,
                "the query no longer fills its twelve with paperwork, so this \
                 proves nothing: find one that does"
            );
            assert!(
                quiet.entries.len() < loud.entries.len(),
                "the mixing list is no shorter than the full one"
            );
            // And the cut is not a silent one.
            assert!(
                quiet.because.contains("type a name"),
                "a list that halved itself said nothing about why: {:?}",
                quiet.because
            );
            assert!(
                loud.because.is_empty(),
                "the full list claimed it had been shortened"
            );
        }

        /// **A list that did not actually change says nothing.**
        ///
        /// Almost every query already fills its twelve with tiers 1 and 2,
        /// because the vocabulary is mostly deck verbs — so the budget removes
        /// something from the *match set* and nothing from what the DJ would
        /// have seen. Announcing a cut there would be djmanzo describing a
        /// decision it did not make, over a list identical to the full one.
        ///
        /// This is the assertion the first version of these tests was missing:
        /// the note was computed over every match rather than over the visible
        /// twelve, and the whole suite stayed green.
        #[test]
        fn a_list_the_budget_did_not_shorten_claims_nothing() {
            let mixing = crate::cockpit::Attention::performing().room_for;
            let quiet = offered("s", 6, mixing);
            let loud = offered("s", 6, crate::tiers::Tier::Preparation);
            assert_eq!(
                quiet.entries.len(),
                loud.entries.len(),
                "pick a query whose visible twelve are already all hands"
            );
            assert!(
                super::ranked("s", 6, &[])
                    .iter()
                    .any(|entry| !super::tier_of(entry).survives_a_mix()),
                "`s` matches no paperwork at all, so this proves nothing"
            );
            assert!(
                quiet.because.is_empty(),
                "djmanzo announced a cut it did not make: {:?}",
                quiet.because
            );
        }

        /// **A query that only matches paperwork still answers it.**
        ///
        /// §18 governs what djmanzo *offers*, never what a DJ asks for by name.
        /// A palette that refused to find Settings during a mix would be
        /// obeying §18 and breaking §98 — *everything one shortcut away* — and
        /// the DJ would conclude djmanzo had lost the panel rather than that it
        /// was being tactful.
        #[test]
        fn asking_for_paperwork_by_name_still_finds_it_mid_mix() {
            let mixing = crate::cockpit::Attention::performing().room_for;
            let asked = offered("settings", 2, mixing);
            assert!(
                asked
                    .entries
                    .iter()
                    .any(|entry| entry.run == "settings" && entry.kind == "surface"),
                "Settings is unreachable mid-mix: {:?}",
                asked
                    .entries
                    .iter()
                    .map(|e| e.label.as_str())
                    .collect::<Vec<_>>()
            );
            assert!(
                asked.because.is_empty(),
                "nothing was cut, so nothing should be explained"
            );

            // `jo` is the whole of the fallback in three rows: everything it
            // matches is the Journal, which is tier 4, so the cut would empty
            // the list — and an empty palette is djmanzo saying it does not
            // have a thing it plainly has.
            let journal = offered("jo", 2, mixing);
            assert!(
                journal
                    .entries
                    .iter()
                    .any(|entry| entry.run.contains("journal")),
                "the Journal vanished mid-mix: {:?}",
                journal
                    .entries
                    .iter()
                    .map(|e| e.label.as_str())
                    .collect::<Vec<_>>()
            );
            assert!(journal.because.is_empty());
        }

        /// **A typed action outranks the budget, whatever tier its verb is.**
        ///
        /// `grid_nudge` is tier 4 — the beat grid is work done before a night.
        /// A DJ who has typed it in full during a mix has not asked to be
        /// protected from it.
        #[test]
        fn a_line_typed_in_full_survives_the_budget() {
            let mixing = crate::cockpit::Attention::performing().room_for;
            let typed = "deck 1 grid_nudge 0.01";
            assert_eq!(
                crate::tiers::of_verb("grid_nudge"),
                crate::tiers::Tier::Preparation
            );
            assert!(
                offered(typed, 2, mixing)
                    .entries
                    .iter()
                    .any(|entry| entry.run == typed),
                "the palette refused a line the DJ typed in full"
            );
        }

        /// **`d2p` finds `Deck 2 · play`.**
        ///
        /// The gesture a palette exists for. A substring test would refuse it,
        /// which is why the matcher is a subsequence.
        #[test]
        fn initials_find_the_thing() {
            assert!(matches("d2p", "Deck 2 \u{b7} play"));
            assert!(matches("dck", "Deck 2 \u{b7} play"));
            assert!(!matches("d3p", "Deck 2 \u{b7} play"));
            // In order, not merely present.
            assert!(!matches("pd2", "Deck 2 \u{b7} play"));
        }

        /// **An empty query opens on something, not on nothing.**
        ///
        /// A palette that showed an empty list until you typed would make the
        /// first press useless, and the first press is the one made in a hurry.
        #[test]
        fn it_opens_with_suggestions() {
            let out = palette(String::new(), 2);
            assert!(!out.is_empty(), "the palette opened empty");
            assert!(
                out.iter().any(|e| e.run.starts_with("deck 1")),
                "an empty query did not offer the first deck's transport",
            );
        }
    }

    mod rail {
        use super::super::{bpm_delta, summarise_reasons};
        use dj_core::{Mode, MusicalKey};
        use dj_library::suggest::Reason;

        fn key(hour: u8, mode: Mode) -> MusicalKey {
            MusicalKey::new(hour, mode).unwrap()
        }

        /// A record with just enough filled in to be judged.
        fn fixture(
            byte: u8,
            bpm: Option<f64>,
            k: Option<MusicalKey>,
            lufs: Option<f64>,
        ) -> dj_library::LibraryTrack {
            dj_library::LibraryTrack {
                id: dj_core::TrackId::from_bytes([byte; 32]),
                path: std::path::PathBuf::from(format!("/music/{byte}.wav")),
                tags: dj_library::Tags::default(),
                duration_frames: 44_100 * 300,
                sample_rate: dj_core::SampleRate::DEFAULT,
                channels: 2,
                file_size: None,
                file_modified: None,
                added_at: 0,
                analysis: dj_library::StoredAnalysis {
                    bpm,
                    key_hour: k.map(MusicalKey::hour),
                    key_mode: k.map(MusicalKey::mode),
                    loudness_lufs: lufs,
                    phrase_beats: Some(16),
                    phrase_anchor: Some(0),
                    ..dj_library::StoredAnalysis::default()
                },
                stats: dj_library::PlayStats::default(),
                colour: None,
            }
        }

        /// **The rail's line is deltas, in the directive's own shape.**
        ///
        /// §22 gives the example verbatim: `+3 BPM · 8A→9A · energy +1`. A
        /// suggester that answered `131 BPM · harmonic (9A) · -6 dB` is
        /// answering a different question -- one that needs the DJ to remember
        /// what is playing before any of it means anything.
        #[test]
        fn the_line_says_what_changes() {
            let line = summarise_reasons(&[
                Reason::TempoFits {
                    from: 128.0,
                    to: 131.0,
                },
                Reason::Harmonic {
                    from: key(8, Mode::Minor),
                    to: key(9, Mode::Minor),
                },
                Reason::Loudness { delta_db: 1.4 },
                Reason::PhraseKnown { beats: 16 },
            ]);
            assert_eq!(line, "+3 BPM \u{b7} 8A\u{2192}9A \u{b7} +1 dB");
        }

        /// **A phrase structure that was found is not worth a word.**
        ///
        /// Eight rows of "16-beat phrases" is eight repetitions of "nothing to
        /// worry about". Its absence is the risk, so that is what gets said.
        #[test]
        fn the_common_case_is_silent_and_the_risk_is_not() {
            assert!(
                !summarise_reasons(&[Reason::PhraseKnown { beats: 32 }]).contains("phrase"),
                "a known phrase structure took up room on the line",
            );
            assert_eq!(summarise_reasons(&[Reason::PhraseUnknown]), "no phrase");
        }

        /// **The worst feature is on the line, not hidden behind the score.**
        ///
        /// A suggestion that concealed its key clash would be one a DJ learns
        /// not to trust after being caught by it once.
        #[test]
        fn a_clash_says_so() {
            let line = summarise_reasons(&[
                Reason::KeyClash {
                    from: key(8, Mode::Minor),
                    to: key(3, Mode::Major),
                },
                Reason::TempoFar {
                    from: 128.0,
                    to: 174.0,
                },
            ]);
            assert_eq!(line, "+46 BPM stretch \u{b7} 8A\u{2192}3B clash");
        }

        /// **Half and double time are named, not turned into a huge delta.**
        ///
        /// `+70 BPM` for a 140 over a 70 describes the arithmetic and not the
        /// move, which is an ordinary one.
        #[test]
        fn the_octave_is_named() {
            assert_eq!(
                summarise_reasons(&[Reason::TempoHalfOrDouble {
                    from: 70.0,
                    to: 140.0
                }]),
                "double-time",
            );
            assert_eq!(
                summarise_reasons(&[Reason::TempoHalfOrDouble {
                    from: 140.0,
                    to: 70.0
                }]),
                "half-time",
            );
        }

        /// **The same tempo still says something.**
        ///
        /// Dropping a zero delta would leave a gap that reads as a missing
        /// value, when it is in fact the strongest thing a tempo can report.
        #[test]
        fn no_change_is_still_an_answer() {
            assert_eq!(bpm_delta(128.0, 128.0, ""), "+0 BPM");
            // The one that reached a screenshot: two records a fraction of a
            // beat apart rendered `-0 BPM`, which reads as a fault.
            assert_eq!(bpm_delta(120.4, 120.0, ""), "+0 BPM");
            assert_eq!(bpm_delta(120.0, 120.4, ""), "+0 BPM");
        }

        /// **With nothing playing, an absolute is the only honest answer.**
        ///
        /// A delta against no tempo is a delta against nothing.
        #[test]
        fn with_no_tempo_to_compare_against_the_value_is_given() {
            assert_eq!(bpm_delta(0.0, 128.0, ""), "128 BPM");
            assert_eq!(bpm_delta(f64::NAN, 128.0, ""), "128 BPM");
        }

        /// **A seam is described by the same words a candidate is.**
        ///
        /// The plan's links and the rail's rows are the same judgement about
        /// the same pair of records, so they must not be two rankings that can
        /// disagree. This pins that they share the summariser.
        #[test]
        fn a_seam_reads_like_a_candidate() {
            use super::super::link_between;
            let a = fixture(1, Some(128.0), Some(key(8, Mode::Minor)), None);
            let b = fixture(2, Some(131.0), Some(key(9, Mode::Minor)), None);
            let link = link_between(&a, &b);
            assert_eq!(link.summary, "+3 BPM \u{b7} 8A\u{2192}9A");
            assert!(
                !link.risky,
                "a harmonic move inside the pitch range is not a risk"
            );
            assert!(
                link.confidence > 0.8,
                "confidence was {:.2}",
                link.confidence
            );
        }

        /// **A key clash is a risk, and so is a tempo the deck cannot reach.**
        ///
        /// Marked rather than avoided. A set with no difficult seams never went
        /// anywhere; what the DJ must not do is meet one for the first time at
        /// 01:40.
        #[test]
        fn the_difficult_seams_are_marked() {
            use super::super::link_between;
            let playing = fixture(1, Some(128.0), Some(key(8, Mode::Minor)), None);
            let clash = fixture(2, Some(128.0), Some(key(3, Mode::Major)), None);
            let far = fixture(3, Some(174.0), Some(key(8, Mode::Minor)), None);

            assert!(
                link_between(&playing, &clash).risky,
                "a key clash was not marked"
            );
            assert!(
                link_between(&playing, &far).risky,
                "an unreachable tempo was not marked"
            );
        }

        /// **A change of rhythmic grammar is a risk the scorer cannot see.**
        ///
        /// Dembow into four-on-the-floor is a cut however well the tempos
        /// match, and the scorer ranks candidates rather than deciding whether
        /// a blend is possible. `dj_core::genre` is what knows.
        #[test]
        fn crossing_a_grammar_is_a_risk_even_when_everything_else_agrees() {
            use super::super::link_between;
            let mut a = fixture(1, Some(128.0), Some(key(8, Mode::Minor)), None);
            let mut b = fixture(2, Some(128.0), Some(key(8, Mode::Minor)), None);
            a.tags.genre = Some("dembow".to_owned());
            b.tags.genre = Some("techno".to_owned());

            let link = link_between(&a, &b);
            assert!(
                link.risky,
                "same key, same tempo, different grammar -- and it was not marked: {}",
                link.summary,
            );
        }

        /// **The first record has no seam before it.**
        #[test]
        fn a_plan_starts_without_a_link() {
            use super::super::slots_with_links;
            use dj_library::setlist::Slot;
            let pool = vec![
                fixture(1, Some(128.0), Some(key(8, Mode::Minor)), None),
                fixture(2, Some(131.0), Some(key(9, Mode::Minor)), None),
            ];
            let slots: Vec<Slot> = pool
                .iter()
                .enumerate()
                .map(|(i, t)| Slot {
                    track: t.id,
                    #[allow(clippy::cast_precision_loss)]
                    through: i as f32,
                    trajectory: dj_core::Trajectory::Hold,
                    reasons: Vec::new(),
                })
                .collect();

            let out = slots_with_links(&slots, &pool);
            assert_eq!(out.len(), 2);
            assert!(out[0].link.is_none(), "the opening record was given a seam");
            assert!(out[1].link.is_some(), "the second record was given none");
        }

        /// **A genre change is announced; staying put is named.**
        #[test]
        fn the_line_reports_where_the_genre_goes() {
            assert_eq!(
                summarise_reasons(&[Reason::SameFamily("bachata")]),
                "bachata"
            );
            assert_eq!(
                summarise_reasons(&[Reason::OtherFamily {
                    from: "merengue",
                    to: "bachata"
                }]),
                "\u{2192}bachata",
            );
        }
    }

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

    /// **djmanzo's own moves are not a hand on the control.**
    ///
    /// §87's takeover is "a person touched this, so leave it alone", and it
    /// fired for every action that went through `perform` — which is the same
    /// path the automix and the autopilot use on purpose, because everything
    /// the machine can do a person could have done. The consequence was that
    /// djmanzo marked its own fader move as the DJ's, `Takeover::may_move`
    /// then refused that control for ten minutes, and `autopilot::next_step`
    /// declines a held one. The machine was handing itself the controls it had
    /// just used, and the assistant's panel said "you have deck 1" with nobody
    /// touching anything.
    ///
    /// Both directions, because a fix that stopped noticing *either* would be
    /// worse than the bug: a DJ's hand must still take the control instantly.
    #[test]
    fn the_machines_own_actions_do_not_read_as_a_hand_on_the_control() {
        use dj_core::param::DeckParam;

        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();
        let volume = dj_core::ParamId::Deck(
            dj_core::DeckId::from_human(1).expect("deck 1"),
            DeckParam::Volume,
        );
        let free = |state: &AppState| {
            state
                .conduct()
                .lock()
                .expect("not poisoned")
                .takeover
                .may_move(volume)
        };

        assert!(free(&state), "nothing is held before anything happens");

        perform_by(&state, "deck 1 volume 0.5", dj_control::By::Machine).unwrap();
        assert!(
            free(&state),
            "djmanzo moved a fader and then treated it as taken by the DJ"
        );

        perform_by(&state, "deck 1 volume 0.7", dj_control::By::Hand).unwrap();
        assert!(
            !free(&state),
            "a hand on the fader did not take it, which is the half §87 is for"
        );
    }

    /// **The automix's own mix is filed as the machine's.**
    ///
    /// The one path where getting this wrong is worst: automix moves faders
    /// continuously for the length of a transition, so an automix blend filed
    /// under the DJ's hand would both make a set they never touched read as
    /// one they performed *and* hand every fader it used back to a DJ who is
    /// not there — §87's takeover triggered by the thing it exists to defer
    /// to, once a second, for the whole mix.
    #[test]
    fn an_automix_blend_is_the_machines_doing_and_takes_nothing_from_the_dj() {
        use dj_core::param::DeckParam;

        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();

        let deck = dj_core::DeckId::from_human(1).expect("deck 1");
        run_automix_plan(
            &state,
            crate::automix::Plan {
                actions: vec![Action::Deck {
                    deck,
                    action: dj_core::DeckAction::SetVolume(0.4),
                }],
                load: None,
            },
        );

        let log = state.bus().log();
        assert_eq!(log.len(), 1, "{log:?}");
        assert_eq!(
            log[0].by,
            dj_control::By::Machine,
            "the automix's own fader move is filed under the DJ"
        );
        assert!(
            state
                .conduct()
                .lock()
                .expect("not poisoned")
                .takeover
                .may_move(dj_core::ParamId::Deck(deck, DeckParam::Volume)),
            "the automix took the fader it was using away from itself"
        );
    }

    /// **And the log says which of the two it was.**
    ///
    /// §67 lists *AI interventions* and *manual interventions* as two of the
    /// fourteen things a session contains, and one log of undifferentiated
    /// actions cannot answer either: "what did djmanzo do tonight" and "what
    /// did I do" were the same question.
    #[test]
    fn the_log_records_whose_hand_each_action_came_from() {
        let state = AppState::new(true);
        state.host().open(None, None, 128).unwrap();

        perform(&state, "deck 1 play").unwrap();
        perform_by(&state, "deck 2 play", dj_control::By::Machine).unwrap();

        let log = state.bus().log();
        assert_eq!(log.len(), 2, "{log:?}");
        assert_eq!(log[0].by, dj_control::By::Hand);
        assert_eq!(log[1].by, dj_control::By::Machine);

        // And it survives the round trip through a session file, which is what
        // makes "what did djmanzo do" answerable about a set from last month.
        let text = crate::session::Session {
            events: log.clone(),
        }
        .to_text();
        let read = crate::session::Session::from_text(&text).expect("it parses");
        // The events and the origins, not the timestamps: a session file writes
        // milliseconds, which is deliberate (see `Session::to_text`) and is not
        // what this test is about.
        assert_eq!(
            read.events
                .iter()
                .map(|e| (e.event, e.by))
                .collect::<Vec<_>>(),
            log.iter().map(|e| (e.event, e.by)).collect::<Vec<_>>(),
        );
        // A file written before any of this existed reads as all a person's,
        // which is true of it: nothing in it was the machine's, because the
        // machine could not be told apart when it was written.
        let old = "djmanzo-session 1\n0.000 deck 1 play\n";
        let older = crate::session::Session::from_text(old).expect("an old take parses");
        assert_eq!(older.events[0].by, dj_control::By::Hand);
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
        let rendered = format!(
            "{:>8.3}  {}",
            log[0].at.as_secs_f64(),
            log[0].event.to_line()
        );
        assert!(rendered.contains("deck 1 play"), "got {rendered:?}");
    }
}

/// The order separation happens in, which is the whole of look-ahead.
#[cfg(test)]
mod safe_tests {
    use super::safe_actions;

    /// **The property that matters is what is missing.**
    ///
    /// An emergency control that silences the floor is worse than the
    /// emergency it was pressed for. So `safe` never stops a record, never
    /// moves a channel fader, and never touches the crossfader — deciding what
    /// the room hears is what a hand is for, and this clears everything that
    /// got between the hand and the sound.
    ///
    /// Asserted over the text of every action it produces rather than over a
    /// handful of them, so a verb added to the expansion later has to pass the
    /// same rule.
    #[test]
    fn nothing_in_safe_can_silence_the_floor() {
        let lines = safe_actions(4);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(
                !line.contains("crossfader") && !line.contains("xfader"),
                "safe moves the crossfader: {line}"
            );
            assert!(
                !line.contains(" volume "),
                "safe moves a channel fader: {line}"
            );
            for stopper in [
                "pause", "play", "cue", "eject", "brake", "backspin", "reverse",
            ] {
                assert!(
                    !line.split_whitespace().any(|word| word == stopper),
                    "safe touches the transport: {line}"
                );
            }
        }
    }

    /// And what it must do, on every deck the rig has.
    #[test]
    fn safe_flattens_every_deck_and_clears_every_rack() {
        let decks = 4;
        let lines = safe_actions(decks);
        for number in 1..=decks {
            for band in ["eq_low", "eq_mid", "eq_high"] {
                assert!(
                    lines.contains(&format!("deck {number} {band} 1")),
                    "deck {number}'s {band} is not flattened"
                );
            }
            assert!(lines.contains(&format!("deck {number} filter 0")));
            for slot in 1..=dj_core::FX_SLOTS {
                assert!(lines.contains(&format!("deck {number} fx {slot} off")));
            }
        }
        for slot in 1..=dj_core::FX_SLOTS {
            assert!(lines.contains(&format!("master fx {slot} off")));
        }
        assert!(lines.contains(&"master gain 0".to_owned()));
        assert!(lines.contains(&"limiter on".to_owned()));
    }

    /// Every line of it is a real action, so the whole emergency logs and
    /// replays like anything else rather than through a path of its own.
    #[test]
    fn every_line_of_safe_is_an_action_the_parser_accepts() {
        for line in safe_actions(2) {
            assert!(
                dj_core::Action::parse(&line).is_ok(),
                "safe would dispatch {line:?}, which is not in the vocabulary"
            );
        }
        // And `safe` itself is, so it is on a controller and in a script.
        assert!(dj_core::Action::parse("safe").is_ok());
    }
}

#[cfg(test)]
mod separation_order_tests {
    use super::{feed_chunks, next_chunk_to_separate};

    /// **Every chunk is queued once**, however slow the worker, nearest the
    /// playhead first — and a seek redirects what is queued next.
    ///
    /// The worker here never finishes anything, which is what HT-Demucs
    /// looks like from the feeder for the seconds each chunk takes: the
    /// published table stays empty. The first feeder queued chunk 18 again
    /// and again in that state and never reached most of the track.
    #[test]
    fn a_slow_worker_still_gets_every_chunk_once() {
        let mut queued = Vec::new();
        feed_chunks(40, || 18, |_| false, |next| queued.push(next));
        assert_eq!(queued.len(), 40, "{queued:?}");
        let mut sorted = queued.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 40, "a chunk was queued twice: {queued:?}");
        assert_eq!(&queued[..3], [18, 19, 17]);

        // A seek after five chunks: the next is chosen from the new place.
        let mut asked = 0;
        let mut queued = Vec::new();
        feed_chunks(
            40,
            || {
                asked += 1;
                if asked > 5 { 30 } else { 0 }
            },
            |_| false,
            |next| queued.push(next),
        );
        assert_eq!(&queued[..6], [0, 1, 2, 3, 4, 30]);

        // What has already come back is not queued again.
        let mut queued = Vec::new();
        feed_chunks(10, || 0, |index| index % 2 == 0, |next| queued.push(next));
        assert_eq!(queued, [1, 3, 5, 7, 9]);
    }

    /// The bug this covers: separation used to walk the file from chunk 0. A
    /// DJ who loads a track and cues straight to the drop at 3:00 -- which is
    /// what a DJ does with a fresh track -- had the worker twenty seconds in
    /// while the playhead sat two and a half minutes away, and the stem pads
    /// quietly did nothing for as long as it took to grind through the gap.
    #[test]
    fn separation_starts_where_the_playhead_is() {
        let none = |_| false;
        assert_eq!(next_chunk_to_separate(40, 18, none), Some(18));
        assert_eq!(next_chunk_to_separate(40, 0, none), Some(0));
        assert_eq!(
            next_chunk_to_separate(40, 39, none),
            Some(39),
            "the last chunk is reachable"
        );
    }

    /// It works outward from there, and at equal distance the chunk ahead wins
    /// -- the playhead moves one way, so audio it is about to reach is worth
    /// more than audio it has just passed.
    #[test]
    fn it_works_outward_preferring_ahead() {
        let done = [18usize];
        let separated = |index: usize| done.contains(&index);
        assert_eq!(
            next_chunk_to_separate(40, 18, separated),
            Some(19),
            "19 and 17 are both one away; ahead should win"
        );

        let done = [17usize, 18, 19];
        let separated = |index: usize| done.contains(&index);
        assert_eq!(next_chunk_to_separate(40, 18, separated), Some(20));
    }

    /// A seek redirects the work rather than being ignored, because the
    /// playhead is read again on every round.
    #[test]
    fn a_seek_redirects_the_next_chunk() {
        let done = [0usize, 1, 2];
        let separated = |index: usize| done.contains(&index);
        assert_eq!(next_chunk_to_separate(40, 1, separated), Some(3));
        // Same table, playhead jumped to the far end.
        assert_eq!(
            next_chunk_to_separate(40, 30, separated),
            Some(30),
            "a seek should not keep grinding through the beginning"
        );
    }

    /// Work already done is never queued twice. After a seek the feeder
    /// revisits a region it may already have covered, and re-separating ten
    /// seconds of audio for nothing is ten seconds another chunk did not get.
    #[test]
    fn separated_chunks_are_not_queued_again() {
        let separated = |index: usize| index != 7;
        assert_eq!(next_chunk_to_separate(40, 0, separated), Some(7));
        assert_eq!(next_chunk_to_separate(40, 39, separated), Some(7));
        assert_eq!(
            next_chunk_to_separate(40, 0, |_| true),
            None,
            "a fully separated track has nothing left to do"
        );
    }

    /// A playhead past the end -- which a finished track reports -- must still
    /// name a real chunk rather than running off the end or wrapping.
    #[test]
    fn a_playhead_past_the_end_still_picks_the_last_chunk() {
        assert_eq!(next_chunk_to_separate(40, 9_000, |_| false), Some(39));
        assert_eq!(next_chunk_to_separate(0, 5, |_| false), None, "empty track");
    }
}

/// Whether a deck can be sent out in parts is a fact about the open device,
/// and the interface has to be able to say which.
#[cfg(test)]
mod stem_out_tests {
    use super::*;

    fn wide_device() -> String {
        "null-wide".to_owned()
    }

    /// Opening a device builds a fresh engine, so the app has to tell it again
    /// — the same trap `apply_controller_routing` exists for. Without this the
    /// panel would keep claiming the stems were going out while the new engine
    /// had never heard of them.
    #[test]
    fn the_choice_survives_opening_another_device() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        state.set_stem_out(DeckId::from_human(2));

        // A different interface, as changing soundcards mid-set would be.
        open_device_for(&state, None, None, Some(128)).unwrap();

        assert_eq!(
            state.stem_out(),
            DeckId::from_human(2),
            "the deck being sent out in parts was forgotten on a device change"
        );
    }

    mod adaptation_level {
        use super::*;

        /// A state with somewhere to write, since the level is a file.
        ///
        /// The `TempDir` is returned rather than dropped: dropping it removes
        /// the directory, and a state pointed at a directory that no longer
        /// exists silently keeps nothing — which would make every assertion
        /// here pass for the wrong reason.
        fn state_with_a_config_dir() -> (AppState, tempfile::TempDir) {
            let dir = tempfile::tempdir().unwrap();
            let state = AppState::new(true);
            state.set_config_dir(dir.path().to_path_buf());
            (state, dir)
        }

        /// **The load-bearing one: one press reaches the posture and all six
        /// locks.**
        ///
        /// §8's whole point is that this is one decision with a range. A DJ who
        /// wanted "stage things for me but do not move my screen" had to find
        /// the posture in one panel and six locks in another, know which of the
        /// six mattered, and get both right. If setting a level reached only
        /// one of the two, the control would look like it worked and do half
        /// its job — the worst available failure, because the half that did not
        /// happen is the half a DJ was worried about.
        #[test]
        fn one_press_sets_the_posture_and_the_locks_together() {
            let (state, _dir) = state_with_a_config_dir();

            let said = set_adaptation_level_of(&state, "prepare").unwrap();
            assert_eq!(said.level, "prepare");
            assert!(
                said.departures.is_empty(),
                "a level djmanzo just set reads as drifted from: {:?}",
                said.departures
            );
            assert_eq!(
                state.conduct().lock().unwrap().posture,
                dj_assistant::Posture::Prepare
            );
            let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
            assert_eq!(
                stored.locked.len(),
                crate::cockpit::Lock::ALL.len(),
                "Prepare left the interface free to move itself"
            );

            // And the top of the axis gives every freedom back, so a level is
            // a move in both directions rather than a one-way ratchet.
            set_adaptation_level_of(&state, "autopilot").unwrap();
            assert_eq!(
                state.conduct().lock().unwrap().posture,
                dj_assistant::Posture::Autopilot
            );
            let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
            assert!(
                stored.locked.is_empty(),
                "Autopilot kept the locks Prepare put on"
            );
        }

        /// **A control the DJ moved afterwards is reported, not undone.**
        ///
        /// The contract that makes a starting point honest, and the same one
        /// §7's arrangements and §54's setups state. An axis that owned six
        /// switches would be the axis arguing with the switches, and §79's
        /// panel would be a row of controls that silently sprang back.
        #[test]
        fn moving_a_control_afterwards_is_said_rather_than_reversed() {
            let (state, _dir) = state_with_a_config_dir();
            set_adaptation_level_of(&state, "prepare").unwrap();

            // The DJ unlocks the theme by hand, as §79's panel lets them.
            let mut stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
            stored
                .locked
                .retain(|lock| *lock != crate::cockpit::Lock::Theme);
            state.set_workspace(&crate::cockpit::resolve(&stored).workspace);

            let said = standing_of(&state).unwrap();
            assert_eq!(
                said.level, "prepare",
                "the level was reset by a lock change"
            );
            assert_eq!(said.departures, ["the theme still follows the night"]);
            assert!(
                !state
                    .workspace()
                    .unwrap()
                    .locked
                    .contains(&crate::cockpit::Lock::Theme),
                "djmanzo put the lock back on rather than saying it was off"
            );
        }

        /// **Never chosen is not Static.**
        ///
        /// djmanzo's shipped behaviour is not Level 0 — it suggests, it fits
        /// the density to the window, it adapts the theme — so a fresh install
        /// reporting Static would be describing itself wrongly, and every
        /// departure it then listed would be djmanzo telling a DJ they had
        /// drifted from a decision they never made.
        #[test]
        fn a_dj_who_never_chose_is_not_reported_at_the_bottom_of_the_axis() {
            let (state, _dir) = state_with_a_config_dir();
            let said = standing_of(&state).unwrap();
            assert_eq!(said.level, "");
            assert!(said.departures.is_empty());
        }

        /// **A level djmanzo does not have is refused rather than guessed.**
        ///
        /// Both directions of wrong are dangerous here: falling back to Static
        /// would quietly turn everything off, and falling back to the top would
        /// hand a DJ the autopilot. Neither is a thing to do on a typo.
        #[test]
        fn an_unknown_level_is_refused() {
            let (state, _dir) = state_with_a_config_dir();
            assert!(set_adaptation_level_of(&state, "level-3").is_err());
            assert!(set_adaptation_level_of(&state, "").is_err());
            assert!(set_adaptation_level_of(&state, "Prepare").is_err());
            assert_eq!(
                standing_of(&state).unwrap().level,
                "",
                "a refused level was stored anyway"
            );
        }
    }

    /// The panel has to distinguish "your interface is too narrow" from "this
    /// is off", because only one of them is something the DJ can act on.
    #[test]
    fn a_narrow_device_reports_unsupported_rather_than_off() {
        let state = AppState::new(true);
        // The default null device has four outputs, which is not eight.
        open_device_for(&state, None, None, Some(128)).unwrap();
        set_stem_out_for_test(&state, Some(1)).unwrap();

        let view = stem_out_view(&state);
        assert_eq!(view.deck, Some(1), "the choice was refused, not remembered");
        assert!(!view.supported, "four channels reported as enough");
        assert_eq!(view.channels, Some(4));
        assert_eq!(view.required, 8);
    }

    /// And on a wide enough one it says so.
    #[test]
    fn a_wide_device_reports_supported() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        set_stem_out_for_test(&state, Some(1)).unwrap();

        let view = stem_out_view(&state);
        assert!(
            view.supported,
            "eight channels reported as too few: {:?}",
            view.channels
        );
        assert_eq!(view.channels, Some(8));
    }

    /// With nothing open there is no device to be too narrow, and claiming
    /// support for a device that does not exist would put the control in front
    /// of a DJ who has not plugged anything in yet.
    #[test]
    fn no_device_is_not_a_supported_device() {
        let state = AppState::new(true);
        let view = stem_out_view(&state);
        assert_eq!(view.channels, None);
        assert!(!view.supported);
    }

    #[test]
    fn a_deck_that_does_not_exist_is_refused() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        assert!(set_stem_out_for_test(&state, Some(0)).is_err(), "deck 0");
        assert!(set_stem_out_for_test(&state, Some(99)).is_err(), "deck 99");
        assert_eq!(state.stem_out(), None, "a refused deck was stored anyway");
    }

    /// The two arrangements want the same sockets, so the panel must never be
    /// able to show both switched on. Enforced in the application as well as
    /// in the engine: the interface reads this, not the audio thread.
    #[test]
    fn choosing_one_arrangement_puts_the_other_away() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();

        state.set_deck_out(Some(4));
        assert_eq!(stem_out_view(&state).decks, Some(4));

        state.set_stem_out(DeckId::from_human(1));
        let view = stem_out_view(&state);
        assert_eq!(view.deck, Some(1));
        assert_eq!(
            view.decks, None,
            "the panel would show per-deck outputs and stem out both on"
        );

        state.set_deck_out(Some(2));
        let view = stem_out_view(&state);
        assert_eq!(view.decks, Some(2));
        assert_eq!(view.deck, None, "stem out was left on beneath per-deck out");
    }

    /// How many pairs the open device could carry, which is what the panel
    /// offers. Four sockets is two decks, not four.
    #[test]
    fn the_device_decides_how_many_decks_can_go_out() {
        let state = AppState::new(true);
        assert_eq!(
            stem_out_view(&state).deck_capacity,
            0,
            "capacity with no device open"
        );

        open_device_for(&state, None, None, Some(128)).unwrap();
        assert_eq!(stem_out_view(&state).deck_capacity, 2, "four outputs");

        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        assert_eq!(stem_out_view(&state).deck_capacity, 4, "eight outputs");
    }

    /// Zero decks is not an arrangement, it is off — and it must read as off
    /// rather than as "per-deck outputs, of nothing".
    #[test]
    fn zero_decks_is_off() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        state.set_deck_out(Some(0));
        assert_eq!(state.deck_out(), None);
    }

    /// Opening a device builds a fresh engine, and it has never heard of this
    /// either.
    #[test]
    fn the_per_deck_choice_survives_opening_another_device() {
        let state = AppState::new(true);
        open_device_for(&state, Some(wide_device()), None, Some(128)).unwrap();
        state.set_deck_out(Some(4));

        open_device_for(&state, None, None, Some(128)).unwrap();

        assert_eq!(
            state.deck_out(),
            Some(4),
            "per-deck outputs were forgotten on a device change"
        );
    }

    /// What `set_stem_out` does, minus Tauri's `State` wrapper.
    fn set_stem_out_for_test(state: &AppState, deck: Option<u8>) -> Result<(), String> {
        let id = match deck {
            Some(number) => {
                Some(DeckId::from_human(number).ok_or_else(|| format!("no deck {number}"))?)
            }
            None => None,
        };
        state.set_stem_out(id);
        Ok(())
    }
}

/// The wiring between a grid-edit action and the two places a grid lives.
///
/// [`crate::grid`] tests the arithmetic. These test that the edit reaches the
/// renderer *and* the engine, that it survives a round trip, and that the
/// failures are reported rather than swallowed -- which is the part that would
/// silently break.
/// The mix-out window, from the deck's own grid.
///
/// Its own module because it needs a *summary* on the deck as well as a grid —
/// the length comes from the same place the lane's width does — and because
/// the thing worth proving here is the wiring rather than the arithmetic.
/// `plan::mix_out` is tested against the planner's constants in `plan`; this
/// asks whether a deck that has a record on it answers at all.
///
/// It exists because the first version of `mix_out_of` read the library's
/// stored analysis, which a freshly loaded deck does not have. Every browser
/// test passed — the harness answers `waveform_info` itself — and the running
/// application drew no band on any lane.
/// §24's kept pairs, folded into a ranking.
///
/// The seam between two things that are each tested elsewhere: the store
/// counts keeps, the scorer weighs them, and this is where they meet. Worth
/// its own tests because a join that reads the right rows and then forgets to
/// re-sort looks exactly like one that works, until a DJ notices the second
/// row scoring higher than the first.
#[cfg(test)]
mod kept_pair_tests {
    use super::*;
    use dj_library::suggest::{Reason, Suggestion};

    fn id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    fn scored(byte: u8, score: f64) -> Suggestion {
        Suggestion {
            track: id(byte),
            score,
            reasons: vec![Reason::PhraseUnknown],
        }
    }

    /// **A pair the DJ kept climbs, and the list is re-sorted around it.**
    #[test]
    fn a_kept_pair_moves_up_the_rail_rather_than_only_scoring_higher() {
        let ranked = vec![scored(1, 5.0), scored(2, 4.0), scored(3, 3.0)];
        let kept = std::collections::HashMap::from([(id(3), (3u32, None))]);

        let out = with_kept(ranked, &kept);
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(3), id(1), id(2)],
            "a kept pair scored higher but stayed where it was"
        );
        assert!(out[0].reasons.contains(&Reason::KeptBefore {
            times: 3,
            on_loop: None
        }));
        // And the ones nobody kept are untouched, reasons included.
        assert_eq!(out[1].reasons, vec![Reason::PhraseUnknown]);
    }

    /// **The loop the pair was kept on reaches the rail, and is said.**
    ///
    /// §24's example is "A into C works only with an 8-beat loop", and a rail
    /// that carried the keep but dropped the loop would offer the pair without
    /// the one instruction that makes it work. The store remembers it and the
    /// scorer carries it; this is the join that has to pass it along.
    #[test]
    fn the_loop_a_pair_was_kept_on_reaches_the_reason_the_dj_reads() {
        let ranked = vec![scored(1, 5.0), scored(3, 3.0)];
        let kept = std::collections::HashMap::from([(id(3), (1u32, Some(8.0)))]);

        let out = with_kept(ranked, &kept);
        let reason = out
            .iter()
            .flat_map(|s| &s.reasons)
            .find(|r| matches!(r, Reason::KeptBefore { .. }))
            .expect("the kept pair lost its reason");
        assert_eq!(
            *reason,
            Reason::KeptBefore {
                times: 1,
                on_loop: Some(8.0)
            }
        );
        assert_eq!(
            describe_reason(reason),
            "you kept this mix, on an 8-beat loop"
        );
        assert_eq!(
            summarise_reasons(std::slice::from_ref(reason)),
            "kept \u{00b7} 8-beat loop"
        );
    }

    /// **A pair kept on no loop says nothing about loops.**
    ///
    /// The absence is the common case, and a rail that wrote "on a 0-beat
    /// loop" beside every other suggestion would be noise with a number in it.
    #[test]
    fn a_pair_kept_on_no_loop_is_described_exactly_as_it_was_before() {
        let reason = Reason::KeptBefore {
            times: 2,
            on_loop: None,
        };
        assert_eq!(describe_reason(&reason), "you kept this mix 2 times");
        assert_eq!(summarise_reasons(&[reason]), "kept \u{00d7}2");
    }

    /// The deck renders a half-beat loop as `1/2`, and so does this.
    #[test]
    fn a_loop_shorter_than_a_beat_is_spelled_the_way_the_deck_spells_it() {
        assert_eq!(beat_count(0.5), "1/2-beat");
        assert_eq!(beat_count(0.25), "1/4-beat");
        assert_eq!(beat_count(8.0), "8-beat");
        assert_eq!(beat_count(32.0), "32-beat");
        // The article follows the number that is actually said.
        assert_eq!(article_for("8-beat"), "an");
        assert_eq!(article_for("4-beat"), "a");
        assert_eq!(article_for("16-beat"), "a");
        assert_eq!(article_for("1/2-beat"), "a");
    }

    /// **Nothing kept changes nothing**, which is most rails.
    #[test]
    fn a_rail_with_no_kept_pairs_is_exactly_the_ranking_it_was() {
        let ranked = vec![scored(1, 5.0), scored(2, 4.0)];
        let out = with_kept(ranked.clone(), &std::collections::HashMap::new());
        assert_eq!(out, ranked);
    }

    /// The tie-break is the suggester's own, so two equal candidates cannot
    /// come out of here in a different order from the one that produced them.
    #[test]
    fn equal_candidates_keep_the_order_the_suggester_gave_them() {
        let ranked = vec![scored(9, 4.0), scored(2, 4.0)];
        let out = with_kept(ranked, &std::collections::HashMap::new());
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(9)],
            "the tie-break differs from the suggester's"
        );
    }
}

/// §16's knowledge pack, where it meets the rail.
///
/// The seam between two things that are each tested elsewhere: `dj-assistant`
/// owns the packs and `dj-library` owns what a pack is worth. This is the join,
/// and until it existed `Pack::families` was parsed, stored, sent to the
/// interface and read by nothing — a table describing music that changed no
/// decision djmanzo made. Worth its own tests for the same reason the kept-pair
/// join is: a fold that scores correctly and forgets to re-sort looks exactly
/// like one that works.
#[cfg(test)]
mod chosen_pack_tests {
    use super::*;
    use dj_library::suggest::{Reason, Suggestion};

    fn id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    fn scored(byte: u8, score: f64, reasons: Vec<Reason>) -> Suggestion {
        Suggestion {
            track: id(byte),
            score,
            reasons,
        }
    }

    /// **The load-bearing one: choosing a pack changes the order of the rail.**
    ///
    /// Not "the score went up" — the score going up while the row stays put is
    /// the defect this join is written to avoid, and it is invisible until a
    /// DJ notices the second row scoring higher than the first.
    ///
    /// The families are read off the shipped pack rather than written here, so
    /// a pack whose families were renamed or misspelled fails this test instead
    /// of quietly behaving like open format.
    #[test]
    fn choosing_a_pack_moves_its_own_music_up_the_rail() {
        let latin = dj_assistant::pack::pack("latin").expect("the latin pack ships");
        let mine = *latin
            .families
            .first()
            .expect("the latin pack names families");

        let ranked = vec![
            scored(1, 5.0, vec![Reason::PhraseUnknown]),
            scored(2, 4.9, vec![Reason::PhraseUnknown]),
        ];
        let families = std::collections::HashMap::from([(id(2), mine)]);

        let out = with_pack(ranked, Some(latin), &|t| families.get(&t).copied());
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(1)],
            "the pack's own music scored higher but stayed where it was"
        );
        assert!(out[0].reasons.contains(&Reason::InPack(mine)));
        // And the record that is not in it is untouched, reasons included.
        assert_eq!(out[1].reasons, vec![Reason::PhraseUnknown]);
    }

    /// **A pack that does not pair across half time drops the pairing back to
    /// where an unrelated tempo sits, and the rail re-sorts around that.**
    ///
    /// §16's *half/double-time relationships*, at the place a DJ sees them.
    /// `house` is the pack that says no, and it says so in the table rather
    /// than here.
    #[test]
    fn a_house_night_stops_seeing_half_time_pairings_at_the_top() {
        let house = dj_assistant::pack::pack("house").expect("the house pack ships");
        assert!(
            !house.half_time,
            "the fixture is wrong: house was meant to be the pack that says no"
        );

        let halved = scored(
            1,
            5.0,
            vec![Reason::TempoHalfOrDouble {
                from: 128.0,
                to: 64.0,
            }],
        );
        let plain = scored(2, 4.5, vec![Reason::PhraseUnknown]);

        let out = with_pack(vec![halved, plain], Some(house), &|_| None);
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(1)],
            "a house night still had the half-time pairing at the top"
        );
        assert!(out[1].reasons.contains(&Reason::HalfTimeUnusual));

        // The same rail on a pack that *does* cross keeps it where it was.
        let latin = dj_assistant::pack::pack("latin").expect("the latin pack ships");
        assert!(latin.half_time, "the fixture is wrong: latin crosses");
        let kept = with_pack(
            vec![
                scored(
                    1,
                    5.0,
                    vec![Reason::TempoHalfOrDouble {
                        from: 128.0,
                        to: 64.0,
                    }],
                ),
                scored(2, 4.5, vec![Reason::PhraseUnknown]),
            ],
            Some(latin),
            &|_| None,
        );
        assert_eq!(
            kept.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(1), id(2)]
        );
    }

    /// **No pack chosen is djmanzo's own ranking, untouched.**
    ///
    /// Most rails. A fold that shifted every candidate when nobody had said
    /// anything would put a chip on every row saying nothing.
    #[test]
    fn a_rail_with_no_pack_chosen_is_exactly_the_ranking_it_was() {
        let ranked = vec![
            scored(1, 5.0, vec![Reason::PhraseUnknown]),
            scored(2, 4.0, vec![Reason::PhraseUnknown]),
        ];
        assert_eq!(with_pack(ranked.clone(), None, &|_| Some("house")), ranked);
    }

    /// **Open format names no families, and that is an answer rather than a
    /// gap.**
    ///
    /// A DJ who plays everything has no family to centre on. The pack must
    /// therefore credit nothing — not credit everything, which would be the
    /// same ranking with a meaningless chip on every row.
    #[test]
    fn open_format_credits_nothing_because_it_centres_on_nothing() {
        let open = dj_assistant::pack::pack("open-format").expect("the open-format pack ships");
        assert!(
            open.families.is_empty(),
            "the fixture is wrong: open format was meant to name no families"
        );

        let ranked = vec![
            scored(1, 5.0, vec![Reason::PhraseUnknown]),
            scored(2, 4.0, vec![Reason::PhraseUnknown]),
        ];
        let out = with_pack(ranked.clone(), Some(open), &|_| Some("house"));
        assert_eq!(out, ranked);
    }

    /// The tie-break is the suggester's own, so two equal candidates cannot
    /// come out of here in a different order from the one that produced them.
    #[test]
    fn equal_candidates_keep_the_order_the_suggester_gave_them() {
        let house = dj_assistant::pack::pack("house").expect("the house pack ships");
        let ranked = vec![
            scored(9, 4.0, vec![Reason::PhraseUnknown]),
            scored(2, 4.0, vec![Reason::PhraseUnknown]),
        ];
        let out = with_pack(ranked, Some(house), &|_| None);
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(9)],
            "the tie-break differs from the suggester's"
        );
    }

    /// **The load-bearing bound: the four tilts together still cannot overrule
    /// the mixing.**
    ///
    /// Four things are added on top of a suggestion's score — taste learned
    /// from what the DJ plays, §81's profile for the night they named, §16's
    /// pack they chose, and §17's reading of what phase the night is in — and
    /// each is bounded on its own in its own file, in prose, in four places.
    /// What none of those says is what happens when all four point the same way
    /// at once, which is the ordinary case: a Latin DJ at a Latin night with
    /// the Latin pack chosen, an hour into the come-down, gets every one of
    /// them.
    ///
    /// **This is the test to extend when a fifth is added**, and it will fail
    /// rather than let the rail start promoting key clashes.
    ///
    /// The rule they are all quoted against is the scoring scale itself: a
    /// same-key match is worth three and a key clash minus two and a half, so
    /// the gap between the best key relation and the worst is what a tilt must
    /// never be able to cross. They break ties. They do not overrule the
    /// mixing — and the pack is the one that made that a question worth
    /// asking, because before it there were two of them and now there are
    /// three.
    ///
    /// Read from the constants rather than restated, so raising any of the
    /// three past what the other two leave room for fails here rather than in
    /// a rail that has quietly started promoting key clashes.
    #[test]
    fn every_tilt_at_once_still_cannot_cross_a_key_relation() {
        use dj_core::{Mode, MusicalKey};
        use dj_library::suggest::Reason;

        let key = |hour| MusicalKey::new(hour, Mode::Minor).expect("a real key");
        let best = Reason::SameKey(key(8)).weight();
        let worst = Reason::KeyClash {
            from: key(8),
            to: key(2),
        }
        .weight();
        assert!(
            best > worst,
            "the scale this is quoted against has changed: {best} against {worst}"
        );

        let tilts = dj_library::learned::Learned::MOST_IT_MAY_MOVE
            + crate::profile::MOST_IT_MAY_MOVE
            + Reason::InPack("bachata").weight()
            + Reason::PhaseAsks("a key that settles").weight();
        assert!(
            tilts < best - worst,
            "taste, the night, the pack and the phase together move a record by \
             {tilts}, which crosses the {} between a key match and a key clash: \
             one of the four has outgrown the scale they are all quoted from",
            best - worst
        );
    }

    /// **Both new reasons reach the words the DJ actually reads.**
    ///
    /// A reason that scores and is never rendered is half a feature: the rail
    /// would reorder itself for a cause nothing on screen names, which reads as
    /// the ranking being wrong.
    #[test]
    fn the_pack_says_why_in_the_chip_and_on_the_summary_line() {
        assert_eq!(
            describe_reason(&Reason::InPack("bachata")),
            "your pack (bachata)"
        );
        assert_eq!(
            describe_reason(&Reason::HalfTimeUnusual),
            "half/double is unusual here"
        );

        // And on the line, the half-time verdict sits beside the tempo it
        // qualifies rather than off at the end.
        assert_eq!(
            summarise_reasons(&[
                Reason::InPack("bachata"),
                Reason::HalfTimeUnusual,
                Reason::TempoHalfOrDouble {
                    from: 128.0,
                    to: 64.0,
                },
            ]),
            "half-time \u{b7} unusual here \u{b7} your pack"
        );
    }
}

/// §17's phase, where it meets the rail.
///
/// The seam between `crate::asks`, which owns what each phase asks for, and the
/// ranking, which owns what a record is worth. Worth its own tests for the
/// reason the other two folds have them: a fold that scores correctly and
/// forgets to re-sort looks exactly like one that works.
#[cfg(test)]
mod phase_tests {
    use super::*;
    use crate::asks::{Prefer, asks};
    use dj_core::{Mode, MusicalKey, SessionPhase};
    use dj_library::suggest::{Reason, Suggestion};

    fn id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    fn key(hour: u8) -> MusicalKey {
        MusicalKey::new(hour, Mode::Minor).expect("a real key")
    }

    fn scored(byte: u8, score: f64, reasons: Vec<Reason>) -> Suggestion {
        Suggestion {
            track: id(byte),
            score,
            reasons,
        }
    }

    /// **The load-bearing one: a phase that asks for something moves the rail.**
    ///
    /// §17's *harmonic resolution*, at the phase that names it. Not "the score
    /// went up" — a lifted score that leaves the row where it was shows a
    /// higher number further down the list, which reads as the ranking being
    /// broken rather than as the feature.
    ///
    /// The phase is read from the table rather than named here, so a release
    /// that stopped asking for resolution fails this instead of quietly
    /// behaving like every other hour of the night.
    #[test]
    fn a_release_moves_a_settling_key_up_the_rail() {
        let release = asks(Some(SessionPhase::Cooldown));
        assert_eq!(
            release.prefer,
            Prefer::Resolution,
            "the fixture is wrong: release was meant to ask for resolution"
        );

        let ranked = vec![
            scored(1, 5.0, vec![Reason::PhraseUnknown]),
            scored(2, 4.9, vec![Reason::SameKey(key(8))]),
        ];
        let out = with_phase(ranked, release, &|_| 0);
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(1)],
            "the settling key scored higher but stayed where it was"
        );
        assert!(
            out[0]
                .reasons
                .contains(&Reason::PhaseAsks("a key that settles"))
        );
        // And the row it did not credit is untouched, reasons included.
        assert_eq!(out[1].reasons, vec![Reason::PhraseUnknown]);
    }

    /// **A harmonic key settles too; a clash does not.**
    ///
    /// Read off the scorer's own answer rather than recomputed, which is the
    /// point: a rail with its own opinion of whether two keys agree would
    /// eventually disagree with the chip sitting next to it on the same row.
    #[test]
    fn resolution_means_the_keys_the_scorer_already_said_agree() {
        let release = asks(Some(SessionPhase::Cooldown));
        let out = with_phase(
            vec![
                scored(
                    1,
                    5.0,
                    vec![Reason::Harmonic {
                        from: key(8),
                        to: key(9),
                    }],
                ),
                scored(
                    2,
                    5.0,
                    vec![Reason::KeyClash {
                        from: key(8),
                        to: key(2),
                    }],
                ),
            ],
            release,
            &|_| 0,
        );
        let credited: Vec<_> = out
            .iter()
            .filter(|s| s.reasons.iter().any(|r| matches!(r, Reason::PhaseAsks(_))))
            .map(|s| s.track)
            .collect();
        assert_eq!(credited, vec![id(1)], "a key clash was called a resolution");
    }

    /// **A close prefers a record this DJ has actually played.**
    ///
    /// §17's *known anchors*. Played at all rather than played often: the
    /// question is whether this room has heard this DJ play it, and a threshold
    /// would be djmanzo deciding how many times counts as known.
    #[test]
    fn a_close_moves_a_record_the_dj_plays_up_the_rail() {
        let closing = asks(Some(SessionPhase::ChillOut));
        assert_eq!(closing.prefer, Prefer::Anchors);

        let plays = std::collections::HashMap::from([(id(2), 1i64)]);
        let out = with_phase(
            vec![
                scored(1, 5.0, vec![Reason::PhraseUnknown]),
                scored(2, 4.9, vec![Reason::PhraseUnknown]),
            ],
            closing,
            &|t| plays.get(&t).copied().unwrap_or(0),
        );
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(1)]
        );
        assert!(
            out[0]
                .reasons
                .contains(&Reason::PhaseAsks("a record you play"))
        );
    }

    /// **Four of the six phases ask the ranking for nothing, and change
    /// nothing.**
    ///
    /// Most of a night. A fold that shifted every candidate when the phase had
    /// nothing to say would put a chip on every row of every rail saying
    /// nothing — and the direction, which is what those phases *do* ask for, is
    /// already in the ranking because it is what the scorer was asked for.
    #[test]
    fn a_phase_with_nothing_to_ask_leaves_the_ranking_exactly_as_it_was() {
        let ranked = vec![
            scored(1, 5.0, vec![Reason::SameKey(key(8))]),
            scored(2, 4.0, vec![Reason::PhraseUnknown]),
        ];
        for phase in [
            None,
            Some(SessionPhase::WarmUp),
            Some(SessionPhase::Heat),
            Some(SessionPhase::Peak),
        ] {
            assert_eq!(
                with_phase(ranked.clone(), asks(phase), &|_| 3),
                ranked,
                "{phase:?} moved a rail it had nothing to say about"
            );
        }
    }

    /// The tie-break is the suggester's own, so two equal candidates cannot
    /// come out of here in a different order from the one that produced them.
    #[test]
    fn equal_candidates_keep_the_order_the_suggester_gave_them() {
        let out = with_phase(
            vec![
                scored(9, 4.0, vec![Reason::PhraseUnknown]),
                scored(2, 4.0, vec![Reason::PhraseUnknown]),
            ],
            asks(Some(SessionPhase::ChillOut)),
            &|_| 0,
        );
        assert_eq!(
            out.iter().map(|s| s.track).collect::<Vec<_>>(),
            vec![id(2), id(9)],
            "the tie-break differs from the suggester's"
        );
    }

    /// **The phase's ask reaches the words the DJ reads, and sits last.**
    ///
    /// Last among the reasons that carry weight, because it is the weakest of
    /// the four and the only one the DJ did not say: what they play, what they
    /// chose and what they kept all outrank a reading djmanzo made on its own.
    #[test]
    fn the_night_says_what_it_asked_for_and_says_it_last() {
        assert_eq!(
            describe_reason(&Reason::PhaseAsks("a record you play")),
            "the night asks for a record you play"
        );
        assert_eq!(
            summarise_reasons(&[
                Reason::PhaseAsks("a key that settles"),
                Reason::InPack("bachata"),
                Reason::SameKey(key(8)),
            ]),
            "8A \u{b7} your pack \u{b7} a key that settles"
        );
    }

    /// **A trajectory survives the round trip it actually makes.**
    ///
    /// The rail sends a word and Rust reads it back, and until §17 there were
    /// two spellings of those three words in two crates with nothing holding
    /// them together. An unrecognised one holds rather than being refused,
    /// because a rail asked for a direction nobody has heard of should still
    /// rank.
    #[test]
    fn the_three_directions_survive_being_written_down() {
        for trajectory in dj_core::Trajectory::ALL {
            assert_eq!(
                dj_core::Trajectory::from_name(trajectory.name()),
                Some(trajectory)
            );
        }
        assert_eq!(dj_core::Trajectory::from_name("sideways"), None);
    }
}

/// §11's *technique recommendations*, reading the one context engine.
///
/// The consumer §11 names that read nothing: the coach narrowed its curriculum
/// by §16's pack and by what the DJ had done in the last two minutes, and had
/// no idea whether the DJ was mid-blend or standing between records.
#[cfg(test)]
mod lesson_moment_tests {
    use super::*;
    use crate::tiers::Tier;

    /// **The load-bearing one: a lesson waits for a moment to be heard in, and
    /// a correction does not.**
    ///
    /// §58 puts technique advice in the contextual tier by name and §18's
    /// mixing budget stops at the second, so the rule is two existing tables
    /// meeting rather than a new opinion. Both budgets are read from
    /// `cockpit::Attention` rather than named here, so a change to either is a
    /// change to this.
    #[test]
    fn a_lesson_waits_for_a_moment_to_be_heard_in() {
        assert!(
            !lesson_withheld(crate::cockpit::Attention::performing().room_for)
                .expect("a mix is not the moment to be taught a new move")
                .is_empty()
        );
        assert_eq!(
            lesson_withheld(crate::cockpit::Attention::preparing().room_for),
            None,
            "between records is exactly when a lesson belongs"
        );
        assert_eq!(
            lesson_withheld(crate::cockpit::Attention::learning().room_for),
            None,
            "the practising budget withheld a lesson, which is what it is for"
        );
    }

    /// **The boundary is §58's own, not a number written here.**
    ///
    /// Contextual is where technique advice lives, so that tier and everything
    /// roomier than it may teach, and the two tighter ones may not. Written
    /// against the whole of `Tier::ALL` so a fifth tier cannot be added on
    /// either side of the line without somebody deciding which side.
    #[test]
    fn the_line_falls_exactly_where_section_fifty_eight_puts_technique_advice() {
        for tier in Tier::ALL {
            assert_eq!(
                lesson_withheld(tier).is_none(),
                tier >= Tier::Contextual,
                "{tier:?} disagrees with where §58 puts technique advice"
            );
        }
    }

    /// **The load-bearing join: exactly one of the two, always.**
    ///
    /// Between the pure decision above and the browser test on the panel sat a
    /// line of command code, and dropping the gate there passed every test in
    /// the repository. It was mutated out deliberately and nothing failed —
    /// which is what "tested at both ends and not in the middle" looks like.
    ///
    /// The invariant is about the **pair**: a panel handed both would say two
    /// things at once, and a panel handed neither with no reason tells a
    /// learner they have finished the curriculum.
    #[test]
    fn a_lesson_or_a_reason_and_never_both() {
        // The default djmanzo is built for, from the table that owns it rather
        // than four booleans written here: a rig spelled out in a test is a
        // second description of a shape `technique::Rig` already has.
        let rig = dj_assistant::technique::Rig::laptop();
        let shown: [&str; 0] = [];

        let (next, withheld) = lesson_now(
            crate::cockpit::Attention::performing().room_for,
            &shown,
            rig,
            None,
        );
        assert!(
            next.is_none(),
            "a lesson was offered in the middle of a mix"
        );
        assert!(withheld.is_some(), "and nothing said why");

        let (next, withheld) = lesson_now(
            crate::cockpit::Attention::preparing().room_for,
            &shown,
            rig,
            None,
        );
        assert!(
            next.is_some(),
            "a DJ who has shown nothing, between records, was taught nothing"
        );
        assert!(
            withheld.is_none(),
            "a lesson was offered and a reason for not offering one, at once"
        );

        // And every tier, so the pair can never both be empty without a reason
        // or both be full.
        for tier in crate::tiers::Tier::ALL {
            let (next, withheld) = lesson_now(tier, &shown, rig, None);
            assert!(
                next.is_some() != withheld.is_some(),
                "{tier:?} produced both a lesson and a reason, or neither"
            );
        }
    }

    /// **Withheld and finished are different answers.**
    ///
    /// The sentence exists so a panel can draw them apart. A learner told
    /// nothing at the moment they start a mix would read it as having finished
    /// the curriculum, which is the one wrong thing a coach can say.
    #[test]
    fn the_reason_is_about_the_minute_rather_than_about_the_dj() {
        let said = lesson_withheld(crate::cockpit::Attention::performing().room_for)
            .expect("mixing withholds");
        assert!(
            said.contains("mixing"),
            "the reason does not say what is actually stopping it: {said}"
        );
        assert!(
            !said.contains('*') && !said.contains('#'),
            "the reason is written in markup"
        );
    }
}

#[cfg(test)]
mod mix_out_tests {
    use super::*;
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos, SampleRate};
    use dj_render::WaveformSummary;

    /// Small on purpose: 8 kHz and 120 BPM puts a hundred beats in 400 000
    /// frames, which is a 3 MB fixture rather than a 40 MB one. The window's
    /// arithmetic does not care, and a test that allocates a real record's
    /// worth of silence is a test people start skipping.
    const SR: SampleRate = SampleRate::new(8_000).unwrap();
    const BPM: f64 = 120.0;
    const BEATS: usize = 100;

    fn deck() -> DeckId {
        DeckId::from_human(1).unwrap()
    }

    fn beat_frames() -> f64 {
        SR.as_f64() * 60.0 / BPM
    }

    /// A deck with a record on it: a summary, so it has a length, and a grid,
    /// so it has beats. Both are what the rasteriser draws from.
    fn deck_with_a_record(phrase: Option<dj_core::Phrase>) -> AppState {
        let state = AppState::new(true);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (beat_frames() * BEATS as f64) as usize;
        state
            .waveforms()
            .set_summary(deck(), WaveformSummary::analyse(&vec![0.0; frames * 2], SR));
        state.waveforms().set_grid(
            deck(),
            Some(dj_render::GridOverlay {
                lines: dj_render::GridLines::all(),
                grid: Beatgrid::new(
                    FramePos::new(0.0),
                    Bpm::new(BPM).unwrap(),
                    Confidence::CERTAIN,
                ),
                sample_rate: SR,
                phrase,
            }),
        );
        state
    }

    /// **A deck with a record on it says where the record can be left.**
    ///
    /// The whole of what went wrong: the answer came back `None` for a deck
    /// that plainly had a grid, because it was asked of the wrong place.
    #[test]
    fn a_deck_with_a_grid_answers_a_window() {
        let state = deck_with_a_record(dj_core::Phrase::new(16, 0));
        let window = mix_out_of(&state, 1).expect("a deck with a grid has a window");

        // Both edges in beats, against the planner's own arithmetic.
        let opens = window.opens_frame / beat_frames();
        let closes = window.closes_frame / beat_frames();
        assert!(
            (closes - 84.0).abs() < 0.01,
            "closed at beat {closes} rather than at 100 - 8 - 8"
        );
        assert_eq!(
            opens % 16.0,
            0.0,
            "the window opened {opens} beats in, which is not a phrase boundary"
        );
        assert!(window.on_phrase);
        assert!(opens < closes);
    }

    /// And an empty deck says nothing rather than guessing at a length.
    #[test]
    fn a_deck_with_nothing_on_it_has_no_window() {
        let state = AppState::new(true);
        assert!(mix_out_of(&state, 1).is_none(), "an empty deck answered");

        // A grid but no record: a length of zero is not a record to leave.
        state.waveforms().set_grid(
            deck(),
            Some(dj_render::GridOverlay {
                lines: dj_render::GridLines::all(),
                grid: Beatgrid::new(
                    FramePos::new(0.0),
                    Bpm::new(BPM).unwrap(),
                    Confidence::CERTAIN,
                ),
                sample_rate: SR,
                phrase: None,
            }),
        );
        assert!(
            mix_out_of(&state, 1).is_none(),
            "a deck with no record answered"
        );
    }
}

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
                lines: dj_render::GridLines::all(),
                grid: Beatgrid::new(
                    FramePos::new(anchor),
                    Bpm::new(bpm).unwrap(),
                    Confidence::new(0.2),
                ),
                sample_rate: SR,
                phrase: None,
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
        assert_eq!(log[0].event.to_line(), "deck 1 grid_nudge 10");
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
/// §20's *vocal availability*, as the table reads it.
///
/// Two aspects of one reading rather than two fields: the **share** is what
/// was measured and is stored, and **strong** is the judgement made about it
/// at read time by the side that owns the line —
/// `dj_analysis::presence::STRONG`.
///
/// Storing the share and deciding here is the point. That constant is the one
/// number in the measurement that is a stated guess rather than a reading, so
/// baking its verdict into a library row would put today's guess in a DJ's
/// database permanently; deciding on the way out means re-tuning it re-reads
/// every column instead of re-analysing every record.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VocalReading {
    /// The strongest window's vocal share, 0..=1.
    pub share: f64,
    /// Whether that is enough to say there is a lead in the record.
    pub strong: bool,
}

impl VocalReading {
    #[must_use]
    pub fn from_share(share: f64) -> Self {
        Self {
            share,
            strong: share >= f64::from(dj_analysis::presence::STRONG),
        }
    }
}

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
    /// §20's energy, 0..=1. Not the loudness above — see `dj_analysis::energy`.
    pub energy: Option<f64>,
    /// §20's *vocal availability*. `None` means nobody has measured, which the
    /// table draws as a blank and never as "no vocal".
    pub vocal: Option<VocalReading>,
    /// True once the track has everything sync and harmonic mixing need.
    pub analysed: bool,
    pub play_count: i64,
    /// 0..=5, when the DJ has rated it.
    pub rating: Option<u8>,
    /// `#rrggbb`, when the DJ has coloured it.
    pub colour: Option<String>,
    /// Unix seconds, when the DJ has played it.
    ///
    /// §20 asks for a *last played* column and the library row has carried this
    /// since M1; it was read off the disk and thrown away here.
    pub last_played: Option<i64>,
    /// How many beats a phrase runs for, when the structure is clear enough.
    ///
    /// §20's *phrase structure*. `None` is a real answer — a record with no
    /// phrase structure is a thing that exists — so it is blank rather than
    /// guessed at.
    pub phrase_beats: Option<u32>,
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
            energy: track.analysis.energy,
            vocal: track.analysis.vocal.map(VocalReading::from_share),
            analysed: track.analysis.is_complete(),
            play_count: track.stats.play_count,
            rating: track.stats.rating,
            colour: track.colour.clone(),
            last_played: track.stats.last_played,
            phrase_beats: track.analysis.phrase_beats,
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

pub(crate) fn library(state: &AppState) -> Result<Arc<dj_library::Library>, String> {
    state.library().get().map_err(|e| e.to_string())
}

/// How often the assistant looks at the set and decides whether to act.
///
/// Half a second. The decisions it makes are on the scale of a record ending,
/// so faster buys nothing; slower would mean a mix point could pass between two
/// looks. The tick does no work at all when the posture is Off, Watch or
/// Suggest, which is where most sessions will leave it.
const TICK: std::time::Duration = std::time::Duration::from_millis(500);

/// Start the assistant's own loop.
///
/// It calls exactly the same `decide` and `perform_step` a manual press does,
/// so what the assistant does on its own and what it does when asked cannot
/// drift apart. All the gating -- posture, takeover, whether there is anything
/// worth doing -- lives in `autopilot::next_step`; this loop is only obedience.
///
/// Not on the audio thread and not on the interface's: it decodes files and
/// takes a lock, and belongs on neither.
pub fn start_assistant_tick(handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);

            // Fetched each tick rather than held: the state is owned by Tauri,
            // and borrowing it for the life of the thread would be borrowing it
            // for the life of the application.
            // `try_state` rather than `state`: the latter panics if called
            // before the state is managed, and a background thread racing
            // start-up is exactly the case that would hit it.
            use tauri::Manager as _;
            let Some(state) = handle.try_state::<AppState>() else {
                continue;
            };
            let state: &AppState = &state;

            let decision = {
                let conduct = state.conduct();
                let Ok(guard) = conduct.lock() else { continue };
                // Cheapest possible early exit: at the quiet postures there is
                // nothing to compute, and computing it would read the library
                // twice a second for an answer that is always "nothing".
                if !guard.posture.may_stage() {
                    continue;
                }
                decide(state, &guard)
            };

            if matches!(decision.step, crate::autopilot::Step::Nothing) {
                continue;
            }
            if let Err(error) = perform_step(state, &decision.step) {
                // Logged rather than retried. A step that failed once will
                // usually fail again immediately, and a loop that retried twice
                // a second would fill the log and change nothing.
                tracing::warn!(%error, "the assistant could not carry out its step");
            }
        }
    });
}

/// Hand the assistant a set to work through.
///
/// Built by `setlist_build`; this is what makes the autopilot able to answer
/// "what next" with something other than nothing. Replacing a set resets the
/// position, because a new set is a new night.
#[tauri::command]
pub fn assistant_set_setlist(
    state: State<'_, AppState>,
    tracks: Vec<String>,
) -> Result<usize, String> {
    let ids: Vec<dj_core::TrackId> = tracks
        .iter()
        .filter_map(|hex| dj_core::TrackId::from_hex(hex))
        .collect();
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.setlist = ids;
    guard.played = 0;
    Ok(guard.setlist.len())
}

/// Do the one thing the assistant would do next, once.
///
/// Explicit rather than only automatic, for two reasons. A DJ at Suggest can
/// press it to accept a suggestion without changing posture -- which is the
/// commonest thing they will want and would otherwise mean turning the
/// assistant up and down again. And the automatic tick calls exactly this, so
/// what a press does and what the tick does cannot drift apart.
///
/// Returns what was done, in words, or `None` if there was nothing to do.
#[tauri::command]
pub fn assistant_step(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let conduct = state.conduct();
    let decision = {
        let guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
        decide(&state, &guard)
    };
    perform_step(&state, &decision.step)
}

/// Carry out one step.
///
/// Separated from the deciding so that the gating lives in exactly one place
/// (`autopilot::next_step`) and this function is only obedience. A second
/// posture check here would be a second thing to keep in step with the first.
fn perform_step(state: &AppState, step: &crate::autopilot::Step) -> Result<Option<String>, String> {
    use crate::autopilot::Step;
    // Every action a step sends is djmanzo's: a step is either the autopilot
    // acting on its own or a transaction the DJ accepted, and in both cases the
    // hand on the control is the machine's. §67's two kinds of intervention.
    let perform = |state: &AppState, text: &str| perform_by(state, text, dj_control::By::Machine);
    match step {
        Step::Nothing => Ok(None),
        Step::Stage { deck, track } => {
            let db = library(state)?;
            let found = db
                .track(*track)
                .map_err(|e| e.to_string())?
                .ok_or("the set names a track the library no longer has")?;
            // Decoded on this thread: the tick runs off the interface and off
            // the audio thread, so blocking here costs nobody anything, and
            // routing it through `put_on_deck` means a staged track gets the
            // same cues, grid and analysis a hand-loaded one does.
            let decoded = decode_file(&found.path).map_err(|e| e.to_string())?;
            put_on_deck(state, *deck, decoded)?;
            // Advance the set only now, when the record has actually reached a
            // deck. A track chosen and then ejected was never played.
            if let Ok(mut guard) = state.conduct().lock()
                && guard.setlist.get(guard.played) == Some(track)
            {
                guard.played += 1;
            }
            Ok(Some(format!("loaded deck {}", deck.human_number())))
        }
        Step::Cue { deck, at } => {
            // Resolved here rather than where the step was built, because the
            // grid it is resolved against arrives with the analyser seconds
            // after the load — and a step built before the load could not have
            // known it. `deck N seek <frame>` is the action that then goes on
            // the bus, so what is logged and replayed is a seek like any other.
            let frame = cue_frame(state, *deck, *at)?;
            perform(
                state,
                &format!("deck {} seek {frame:.0}", deck.human_number()),
            )?;
            Ok(Some(format!("cued deck {}", deck.human_number())))
        }
        Step::Sync { deck } => {
            perform(state, &format!("deck {} sync", deck.human_number()))?;
            Ok(Some(format!("synced deck {}", deck.human_number())))
        }
        Step::MatchGain { deck, db } => {
            perform(state, &format!("deck {} gain {db:.2}", deck.human_number()))?;
            Ok(Some(format!(
                "trimmed deck {} by {db:+.1} dB",
                deck.human_number()
            )))
        }
        Step::Mix {
            from,
            to,
            beats,
            style,
        } => {
            // Through the automix, which already knows how to run a transition
            // of a given style and length. Re-implementing it here would be a
            // second transition engine to keep in agreement with the first.
            //
            // §68: where djmanzo is already *holding* a mix for these two
            // decks, that one is performed and this says nothing about style
            // or length. A DJ who set a mix up in the pair view and then let
            // the assistant run it should get the mix they set up — the whole
            // point of there being one transition object is that there is one
            // answer to "what happens next".
            let holding = state
                .transition()
                .is_some_and(|held| held.outgoing_deck == *from && held.incoming_deck == *to);
            if holding {
                perform(state, "automix now")?;
                return Ok(Some("performing the mix you set up".to_owned()));
            }
            perform(state, &format!("automix style {}", style.as_str()))?;
            perform(state, &format!("automix beats {beats}"))?;
            perform(state, "automix now")?;
            Ok(Some(format!("mixing over {beats} beats")))
        }
    }
}

/// The emergency, expanded.
///
/// [§47](../../../docs/DIRECTIVE.md) asks for a control a DJ can hit without
/// thinking when something has gone wrong, and says the semantics need care.
/// These are the semantics, and the second list is the important one.
///
/// **What it does.** Takes every control back from the assistant and throws
/// away anything it had staged. Clears every effect on every deck and on the
/// master. Puts all three EQ bands and the filter back to neutral. Restores
/// master gain to unity and re-engages the limiter.
///
/// **What it deliberately does not do.** It does not stop a record, move a
/// channel fader, or move the crossfader. Every one of those changes what the
/// room is hearing *immediately*, and the failure mode of an emergency control
/// that silences the floor is far worse than the emergency: a DJ who hits SAFE
/// because an effect ran away has a problem, and a DJ who hits SAFE and gets
/// silence has a disaster. What a hand is for is deciding what the room hears;
/// this clears everything that got between the hand and the sound, and stops
/// there.
///
/// Every part of it is an ordinary action on the ordinary bus, so the whole
/// thing appears in the session log as what it was and replays exactly.
fn make_safe(state: &AppState) -> Result<(), String> {
    // The assistant first: clearing an effect while something is still allowed
    // to put one back is not an emergency stop, it is a race.
    if let Ok(mut guard) = state.conduct().lock() {
        guard.takeover.take_all();
    }
    state.clear_staged();

    let mut failures = Vec::new();
    for line in safe_actions(state.deck_count()) {
        if let Err(error) = perform(state, &line) {
            failures.push(format!("{line}: {error}"));
        }
    }

    // Reported rather than swallowed, and only after everything else has been
    // tried: an emergency that stopped at its first failure would leave the
    // rest of the rack running.
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("safe, except: {}", failures.join("; ")))
    }
}

/// Exactly what `safe` expands into, in order.
///
/// A list rather than a loop of side effects so that the property that matters
/// — what is *not* in it — can be asserted without an engine, an audio device
/// or a running set. See `make_safe` for the reasoning behind the omissions;
/// `nothing_in_safe_can_silence_the_floor` is what holds them to it.
#[must_use]
pub fn safe_actions(decks: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for number in 1..=decks {
        for slot in 1..=dj_core::FX_SLOTS {
            lines.push(format!("deck {number} fx {slot} off"));
        }
        for band in ["eq_low", "eq_mid", "eq_high"] {
            lines.push(format!("deck {number} {band} 1"));
        }
        lines.push(format!("deck {number} filter 0"));
    }
    for slot in 1..=dj_core::FX_SLOTS {
        lines.push(format!("master fx {slot} off"));
    }
    lines.push("master gain 0".to_owned());
    lines.push("limiter on".to_owned());
    lines
}

/// Where a [`crate::autopilot::CueTo`] actually is on a deck, in frames.
///
/// Needs three things the deck only has once it has been analysed: a grid to
/// count beats from, a sample rate to turn beats into frames, and a phrase
/// length to know how long a phrase is. Missing any of them is an error rather
/// than a guess — seeking a record to a place nobody worked out is worse than
/// leaving it where the DJ put it, because it happens silently and the DJ finds
/// out when they bring the fader up.
fn cue_frame(
    state: &AppState,
    deck: dj_core::DeckId,
    at: crate::autopilot::CueTo,
) -> Result<f64, String> {
    let overlay = state.waveforms().grid(deck.human_number()).ok_or_else(|| {
        format!(
            "deck {} has no beat grid to cue against",
            deck.human_number()
        )
    })?;
    let beat_frames = overlay.sample_rate.as_f64() * 60.0 / overlay.grid.bpm.get();
    if !(beat_frames.is_finite() && beat_frames > 0.0) {
        return Err(format!(
            "deck {}'s grid has no usable tempo",
            deck.human_number()
        ));
    }
    match at {
        crate::autopilot::CueTo::PhraseStart => {
            let phrase = overlay.phrase.ok_or_else(|| {
                format!(
                    "deck {} has no phrase structure to cue to",
                    deck.human_number()
                )
            })?;
            // The first phrase boundary at or after the start of the record.
            // Counted from the grid anchor, which is *some* beat rather than
            // necessarily the first — so this walks forward from wherever the
            // anchor happens to be until it is inside the track.
            let anchor = overlay.grid.anchor.get();
            let length = f64::from(phrase.beats) * beat_frames;
            let first = anchor + f64::from(phrase.anchor) * beat_frames;
            let behind = ((0.0 - first) / length).ceil().max(0.0);
            Ok((first + behind * length).max(0.0))
        }
    }
}

/// How the assistant is conducting itself, for the panel.
#[derive(Debug, Clone, Serialize)]
pub struct ConductDto {
    pub posture: String,
    pub occasion: String,
    /// Deck numbers with at least one control the human has taken.
    pub decks_held: Vec<u8>,
    /// Whether anything at all is held, so the panel knows to offer resume.
    /// Offering it when nothing was taken is offering to undo nothing.
    pub anything_held: bool,
    /// What the assistant would do next, and why. Present at every posture,
    /// including the ones that will not act -- seeing what it *would* do is how
    /// a DJ decides whether to let it.
    pub next_step: String,
    pub because: String,
    /// Whether a mistake right now is expensive.
    ///
    /// What the interface reads to decide how hard the destructive controls
    /// should be to hit. Sent rather than derived in the interface, so the
    /// occasion table has one home and cannot disagree with itself.
    pub mistakes_are_costly: bool,
    /// How much explanation to offer, 0..=2.
    pub verbosity: u8,
}

/// A pack: both dials under one name.
#[derive(Debug, Clone, Serialize)]
pub struct PackDto {
    pub name: String,
    pub posture: String,
    pub occasion: String,
    pub summary: String,
}

/// The packs on offer.
#[tauri::command]
#[must_use]
pub fn assistant_packs() -> Vec<PackDto> {
    dj_assistant::packs()
        .iter()
        .map(|p| PackDto {
            name: p.name.to_owned(),
            posture: p.posture.name().to_owned(),
            occasion: p.occasion.name().to_owned(),
            summary: p.summary.to_owned(),
        })
        .collect()
}

/// How the assistant is conducting itself, and what it would do next.
#[tauri::command]
pub fn assistant_conduct(state: State<'_, AppState>) -> Result<ConductDto, String> {
    let conduct = state.conduct();
    let guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    let decision = decide(&state, &guard);
    Ok(ConductDto {
        posture: guard.posture.name().to_owned(),
        occasion: guard.occasion.name().to_owned(),
        decks_held: guard
            .takeover
            .decks_held()
            .iter()
            .map(|d| d.human_number())
            .collect(),
        anything_held: guard.takeover.anything_held(),
        next_step: describe_step(&decision.step),
        because: decision.because,
        mistakes_are_costly: guard.occasion.mistakes_are_costly(),
        verbosity: guard.occasion.verbosity(),
    })
}

// -- the journal ---------------------------------------------------------
//
// See the `notes` migration in `dj-library` for why a note belongs to a moment
// rather than to a track.

#[derive(Debug, Clone, Serialize)]
pub struct NoteDtoJournal {
    pub id: i64,
    pub session_id: String,
    /// Unix seconds, the same clock as a play.
    pub at: i64,
    pub body: String,
    pub playing: String,
    /// Marked but not yet written up.
    pub bare: bool,
}

impl From<dj_library::Note> for NoteDtoJournal {
    fn from(note: dj_library::Note) -> Self {
        Self {
            bare: note.is_bare(),
            id: note.id,
            session_id: note.session_id,
            at: note.at,
            body: note.body,
            playing: note.playing,
        }
    }
}

/// What is on the decks, as one line, for a note to carry.
///
/// Every *playing* deck, in deck order, because a note taken mid-transition is
/// about both records and picking one would be picking wrong half the time.
/// Falls back to whatever is loaded when nothing is playing: a DJ marking the
/// moment before they bring something in still means that record.
fn now_playing(state: &AppState) -> String {
    let registry = state.registry();
    let tracks = state.deck_tracks();
    let Ok(tracks) = tracks.lock() else {
        return String::new();
    };

    // The same naming the request book is matched against, so what the room
    // reads and what a played request is ticked off by cannot drift apart.
    let describe = |deck: u8| tracks.get(&deck).map(crate::track_name);

    let decks: Vec<u8> = (1..=state.deck_count())
        .filter_map(|n| u8::try_from(n).ok())
        .collect();

    let playing: Vec<String> = decks
        .iter()
        .filter(|n| {
            dj_core::DeckId::from_human(**n).is_some_and(|d| {
                registry.get(dj_core::ParamId::Deck(
                    d,
                    dj_core::param::DeckParam::Playing,
                )) > 0.5
            })
        })
        .filter_map(|n| describe(*n))
        .collect();

    if playing.is_empty() {
        decks
            .iter()
            .filter_map(|n| describe(*n))
            .collect::<Vec<_>>()
    } else {
        playing
    }
    .join(" / ")
}

/// Mark this moment.
///
/// The body may be empty, and usually is. The moment is the part that cannot
/// be recovered afterwards; the words are the part that can, so the gesture in
/// a booth is mark now and write it up later. Returns the id so the interface
/// can put a cursor in it without re-reading the night.
#[tauri::command]
pub fn note_add(state: State<'_, AppState>, body: String) -> Result<i64, String> {
    let playing = now_playing(&state);
    library(&state)?
        .add_note(
            &state.session_id(),
            crate::library::now_seconds(),
            body.trim(),
            &playing,
        )
        .map_err(|e| e.to_string())
}

/// Write up a note that was marked earlier.
#[tauri::command]
pub fn note_write(state: State<'_, AppState>, id: i64, body: String) -> Result<(), String> {
    library(&state)?
        .write_note(id, body.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    library(&state)?.delete_note(id).map_err(|e| e.to_string())
}

/// One night's notes, oldest first.
///
/// Pass no session to read tonight's, which is what the panel wants while the
/// set is running.
#[tauri::command]
pub fn notes(
    state: State<'_, AppState>,
    session: Option<String>,
) -> Result<Vec<NoteDtoJournal>, String> {
    let session = session.unwrap_or_else(|| state.session_id());
    Ok(library(&state)?
        .notes(&session)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(NoteDtoJournal::from)
        .collect())
}

/// Which nights have notes, and how many.
#[tauri::command]
pub fn note_counts(state: State<'_, AppState>) -> Result<Vec<(String, i64)>, String> {
    library(&state)?.note_counts().map_err(|e| e.to_string())
}

/// The session tonight's notes belong to.
///
/// The interface needs it to tell "this night" from the ones in the history
/// list, and it is not otherwise reachable from the front end.
#[tauri::command]
pub fn current_session(state: State<'_, AppState>) -> String {
    state.session_id()
}

// -- the coach ----------------------------------------------------------
//
// See `dj_assistant::coach` for why this reads the action log rather than the
// audio: every action is already timestamped on one bus, so what the DJ did is
// known exactly rather than inferred.

/// How far back the coach looks.
///
/// Two minutes. Long enough to contain a whole transition at any danceable
/// tempo — the longest djmanzo will plan is 64 beats, under two minutes — and
/// short enough that a DJ is told about the mix they just did rather than one
/// from earlier in the night.
const COACH_WINDOW: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Debug, Clone, Serialize)]
pub struct ObservedDto {
    pub technique: String,
    /// What it does, in one line.
    pub what: String,
    /// The bridge from the world. See ASSISTANT.md §12.
    pub metaphor: String,
    /// Seconds into the session.
    pub at: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteDto {
    pub what: String,
    pub why: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoachDto {
    /// What the coach recognised, oldest first — reading it back is watching
    /// the mix again.
    pub observed: Vec<ObservedDto>,
    /// At most one thing to say. §12's rule: a learner handed three
    /// corrections applies none of them.
    pub note: Option<NoteDto>,
    /// The next thing worth practising, if there is one.
    pub next: Option<String>,
    /// Why that one — the same metaphor the lesson is taught in.
    pub next_metaphor: Option<String>,
    /// Why there is no lesson right now, when the reason is the moment rather
    /// than the catalogue.
    ///
    /// Two different empties that must not look alike. *Nothing left to teach*
    /// is an answer about the DJ; *not now* is an answer about the minute they
    /// are in, and a panel that drew both as a blank would tell a learner they
    /// had finished the curriculum every time they started a mix.
    pub next_withheld: Option<String>,
}

/// What the rig can actually do right now.
///
/// Read rather than configured. A DJ should not have to tell djmanzo whether
/// they have a controller plugged in, and a setting they forgot to change is
/// how a laptop DJ ends up being taught scratches.
fn rig(state: &AppState) -> dj_assistant::technique::Rig {
    let registry = state.registry();
    // Any loaded deck with a confident grid means the structural techniques
    // are real. `> 0.0` rather than a threshold: the analyser reports its own
    // confidence and a grid it is unsure of is still a grid to jump around.
    let analysis = (1..=state.deck_count())
        .filter_map(|n| dj_core::DeckId::from_human(u8::try_from(n).ok()?))
        .any(|d| {
            registry.get(dj_core::ParamId::Deck(d, dj_core::param::DeckParam::Loaded)) > 0.5
                && registry.get(dj_core::ParamId::Deck(
                    d,
                    dj_core::param::DeckParam::GridConfidence,
                )) > 0.0
        });

    // A controller that is *open*, not one that exists on disk or is merely
    // plugged in. A DJ whose device is connected but whose mapping was never
    // opened has, as far as their hands are concerned, a laptop.
    let controller = state.control().status(None).open_port.is_some();

    dj_assistant::technique::Rig {
        platter: controller,
        crossfader: controller,
        stems: state.stems_backend().is_some(),
        analysis,
    }
}

/// What the coach makes of the last couple of minutes.
///
/// Says nothing rather than something vague: an empty result is the honest
/// answer when nothing recognisable happened, and is far better than a
/// generated remark that makes the DJ stop reading.
/// The tail of the session log, as the coach wants it.
///
/// Separated from the command so it can be tested: the command needs a live
/// `AppState` and this is the part with a decision in it.
///
/// Measured from the *last* event rather than from now. A DJ who mixed and
/// then stood still for five minutes should still be told what they did —
/// wall-clock silence is not a reason to forget the mix.
#[must_use]
fn recent_moments(
    log: &[dj_control::TimedEvent],
    window: std::time::Duration,
) -> Vec<dj_assistant::coach::Moment> {
    let latest = log.last().map(|e| e.at).unwrap_or_default();
    let since = latest.saturating_sub(window);
    log.iter()
        .filter(|e| e.at >= since)
        .filter_map(|e| match &e.event {
            dj_control::SessionEvent::Action(action) => {
                Some(dj_assistant::coach::Moment::new(e.at, *action))
            }
            // A load is not a technique. It is how a record got here. Nor is
            // an audition: listening to a candidate in headphones is a
            // decision about what to play, not a thing a coach has an opinion
            // on.
            dj_control::SessionEvent::Load { .. } | dj_control::SessionEvent::Auditioned { .. } => {
                None
            }
        })
        .collect()
}

#[tauri::command]
pub fn coach_report(state: State<'_, AppState>) -> Result<CoachDto, String> {
    let log = state.bus().log();
    let latest = log.last().map(|e| e.at).unwrap_or_default();
    let moments = recent_moments(&log, COACH_WINDOW);

    let mut observed: Vec<_> = dj_assistant::coach::observe(&moments)
        .into_iter()
        .map(|o| ObservedDto {
            technique: o.technique.name.to_string(),
            what: o.technique.what.to_string(),
            metaphor: o.technique.metaphor.to_string(),
            at: o.at.as_secs_f64(),
        })
        .collect();

    // The shape of the crossfade is a technique too, and the only one that
    // cannot be seen in a single action.
    if let Some(shape) = dj_assistant::coach::crossfade_shape(&moments) {
        observed.push(ObservedDto {
            technique: shape.name.to_string(),
            what: shape.what.to_string(),
            metaphor: shape.metaphor.to_string(),
            at: latest.as_secs_f64(),
        });
    }

    let note = coach_note(&state).map(|n| NoteDto {
        what: n.what,
        why: n.why,
        fix: n.fix,
    });

    let shown: Vec<&str> = observed.iter().map(|o| o.technique.as_str()).collect();
    // §16: taught inside the chosen pack. Without one the answer is the
    // easiest unshown move in the whole catalogue, which is how a bachata DJ
    // was eventually sent to learn a transformer scratch.
    let chosen = state.chosen_pack();
    let pack = chosen.as_deref().and_then(dj_assistant::pack::pack);

    // §11's *technique recommendations*, reading the one context engine —
    // §18's budget, which `cockpit::Attention::for_context` derives from the
    // night and from what is audible, rather than the coach forming its own
    // opinion about whether now is a good moment.
    //
    // §58 names technique advice in the **contextual** tier in as many words,
    // and §18's mixing budget leaves room for the first two tiers and no
    // others. So this is not a new rule: it is two tables that already existed
    // being asked the question they were written to answer.
    //
    // The *note* is deliberately not gated. A correction about the mix the DJ
    // is in the middle of is the one thing a coach is for — "both lows are up"
    // is worth saying at exactly the moment a lesson is not.
    let room = snapshot_now(&state).attention.room_for;
    let (next, withheld) = lesson_now(room, &shown, rig(&state), pack);

    Ok(CoachDto {
        observed,
        note,
        next: next.map(|t| t.name.to_string()),
        next_metaphor: next.map(|t| t.metaphor.to_string()),
        next_withheld: withheld.map(str::to_owned),
    })
}

/// Whether there is room for a lesson right now, and the words if there is not.
///
/// §58 names *technique advice* in the contextual tier, in as many words, and
/// §18's mixing budget leaves room for the first two tiers and no others. So
/// this is not a new rule — it is two tables that already existed being asked
/// the question they were written to answer, at the one place that had never
/// asked it.
///
/// Separate from the command so it can be tested: the command needs a live
/// `AppState`, a snapshot and an action log, and this is the decision.
fn lesson_withheld(room: crate::tiers::Tier) -> Option<&'static str> {
    if room >= crate::tiers::Tier::Contextual {
        return None;
    }
    Some("Not while you are mixing. The lesson will be here between records.")
}

/// The lesson to offer, or the reason there is none — never both, never
/// neither-with-no-explanation.
///
/// Separate from the command so the **join** can be tested, which is the part
/// that was not: `lesson_withheld` is a pure decision and the panel is a
/// browser test, and between them sat a line of command code where the gate
/// could be dropped without a single test noticing. It was, deliberately, and
/// nothing failed.
///
/// The pair is returned together because the one invariant worth holding is
/// about the pair: a panel handed both would say two things at once, and a
/// panel handed neither, with no reason, tells a learner they have finished the
/// curriculum.
fn lesson_now(
    room: crate::tiers::Tier,
    shown: &[&str],
    rig: dj_assistant::technique::Rig,
    pack: Option<&dj_assistant::pack::Pack>,
) -> (
    Option<&'static dj_assistant::technique::Technique>,
    Option<&'static str>,
) {
    match lesson_withheld(room) {
        Some(because) => (None, Some(because)),
        None => (dj_assistant::coach::next_lesson(shown, rig, pack), None),
    }
}

/// The one thing worth saying about the mix as it stands.
///
/// Two lows up is checked before a phrase that is off, because it is the one
/// the DJ cannot hear in headphones — it sounds fine there and wrong in the
/// room, so it is the correction that most needs a machine to make it.
fn coach_note(state: &AppState) -> Option<dj_assistant::coach::Note> {
    let registry = state.registry();
    let read = |deck: dj_core::DeckId, p| registry.get(dj_core::ParamId::Deck(deck, p));

    let playing: Vec<dj_core::DeckId> = (1..=state.deck_count())
        .filter_map(|n| dj_core::DeckId::from_human(u8::try_from(n).ok()?))
        .filter(|d| read(*d, dj_core::param::DeckParam::Playing) > 0.5)
        .collect();

    if let [a, b] = playing[..]
        && let Some(note) = dj_assistant::coach::critique_lows(
            read(a, dj_core::param::DeckParam::EqLow),
            read(b, dj_core::param::DeckParam::EqLow),
        )
    {
        return Some(note);
    }

    None
}

/// Set how much the assistant does.
#[tauri::command]
pub fn assistant_set_posture(state: State<'_, AppState>, posture: String) -> Result<(), String> {
    let wanted = dj_assistant::Posture::parse(&posture)
        .ok_or_else(|| format!("{posture:?} is not a posture"))?;
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.posture = wanted;
    Ok(())
}

/// Set what the night is.
#[tauri::command]
pub fn assistant_set_occasion(state: State<'_, AppState>, occasion: String) -> Result<(), String> {
    let wanted = dj_assistant::Occasion::parse(&occasion)
        .ok_or_else(|| format!("{occasion:?} is not an occasion"))?;
    state.set_occasion(wanted)
}

/// Choose a pack, setting both dials at once.
#[tauri::command]
pub fn assistant_apply_pack(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let pack = dj_assistant::packs()
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| format!("no pack called {name:?}"))?;
    {
        let conduct = state.conduct();
        let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
        guard.posture = pack.posture;
    }
    state.set_occasion(pack.occasion)
}

// -- the staged transaction -------------------------------------------------
//
// §44. See `crate::staged` for why a bundle rather than five decisions, and why
// accepting is not a second way of doing things.

/// Prepare the next transition, without doing any of it.
///
/// Replaces whatever was staged: a plan is about the record that is playing,
/// and asking again means asking about now.
#[tauri::command]
pub fn staged_prepare(state: State<'_, AppState>) -> Result<Option<crate::staged::Staged>, String> {
    let conduct = state.conduct();
    let guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    let situation = read_situation(&state, &guard);
    let decision = crate::autopilot::next_step(&situation, &guard.takeover, &guard.authority);
    // The planner is asked directly rather than through the decision, because
    // the decision is one step and this is the whole shape of the mix.
    let planned = situation
        .staged
        .as_ref()
        .and_then(|(_, incoming)| crate::plan::plan(&situation.outgoing, incoming));
    let live_track = state.deck_track_id(situation.live);
    let staged = crate::staged::build(
        &situation,
        planned.as_ref(),
        &guard.authority,
        guard.posture,
        &decision.because,
        live_track,
    );
    drop(guard);
    state.set_staged(staged.clone());
    Ok(staged)
}

/// What is staged, or nothing.
///
/// Answers `None` and clears the plan when the record it was about has left the
/// deck — a plan drawn against a set that no longer exists is worse than no
/// plan, because it looks current.
#[tauri::command]
pub fn staged_current(state: State<'_, AppState>) -> Option<crate::staged::Staged> {
    let staged = state.staged()?;
    let live = dj_core::DeckId::from_human(staged.live_deck)?;
    if staged.still_current(live, state.deck_track_id(live)) {
        Some(staged)
    } else {
        state.clear_staged();
        None
    }
}

/// Turn one move on or off. **Modify.**
#[tauri::command]
pub fn staged_choose(
    state: State<'_, AppState>,
    index: usize,
    chosen: bool,
) -> Result<Option<crate::staged::Staged>, String> {
    state.choose_staged(index, chosen)?;
    Ok(state.staged())
}

/// Throw it away. **Reject.**
#[tauri::command]
pub fn staged_reject(state: State<'_, AppState>) {
    state.clear_staged();
}

/// Carry it out. **Accept.**
///
/// Every chosen move goes through `perform_step`, the same function the
/// automatic tick uses, so what a press does and what the tick does cannot
/// drift apart. The plan is cleared whatever happens: a transaction that had
/// been half carried out is not one to offer again.
#[tauri::command]
pub fn staged_accept(state: State<'_, AppState>) -> Result<crate::staged::Outcome, String> {
    let staged = state.staged().ok_or("nothing is staged")?;
    let mut done = Vec::new();
    let mut stopped = None;
    for (index, step) in staged.chosen() {
        match perform_step(&state, step) {
            Ok(Some(what)) => done.push(what),
            Ok(None) => {}
            Err(because) => {
                stopped = Some(crate::staged::Stopped {
                    at: index,
                    about: staged.moves[index].about.clone(),
                    because,
                });
                break;
            }
        }
    }
    state.clear_staged();
    Ok(crate::staged::Outcome { done, stopped })
}

// -- the override matrix ----------------------------------------------------

/// One row of §72's matrix, for the panel that shows it.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorityRow {
    pub capability: String,
    pub title: String,
    /// Whether the room hears this the moment it happens.
    pub audible: bool,
    /// One entry per posture, in `Posture::ALL` order.
    pub allowances: Vec<String>,
    /// Which of those the DJ has changed from djmanzo's answer.
    pub changed: Vec<bool>,
}

/// The matrix as it currently stands.
#[tauri::command]
pub fn authority_matrix(state: State<'_, AppState>) -> Result<Vec<AuthorityRow>, String> {
    let conduct = state.conduct();
    let guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    Ok(dj_assistant::Capability::ALL
        .into_iter()
        .map(|capability| AuthorityRow {
            capability: capability.name().to_owned(),
            title: capability.title().to_owned(),
            audible: capability.is_audible(),
            allowances: dj_assistant::Posture::ALL
                .into_iter()
                .map(|posture| {
                    guard
                        .authority
                        .allows(capability, posture)
                        .name()
                        .to_owned()
                })
                .collect(),
            changed: dj_assistant::Posture::ALL
                .into_iter()
                .map(|posture| {
                    guard.authority.allows(capability, posture)
                        != dj_assistant::Authority::default_for(capability, posture)
                })
                .collect(),
        })
        .collect())
}

/// Change one cell of the matrix.
#[tauri::command]
pub fn authority_set(
    state: State<'_, AppState>,
    capability: String,
    posture: String,
    allowance: String,
) -> Result<(), String> {
    let capability = dj_assistant::Capability::parse(&capability)
        .ok_or_else(|| format!("{capability:?} is not something the matrix covers"))?;
    let posture = dj_assistant::Posture::parse(&posture)
        .ok_or_else(|| format!("{posture:?} is not a posture"))?;
    let allowance = dj_assistant::Allowance::parse(&allowance)
        .ok_or_else(|| format!("{allowance:?} is not no, limited or yes"))?;
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.authority.set(capability, posture, allowance)
}

/// Put every cell back to djmanzo's answer.
#[tauri::command]
pub fn authority_reset(state: State<'_, AppState>) -> Result<(), String> {
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.authority.reset();
    Ok(())
}

/// Take everything out of the assistant's hands, now.
///
/// The panic gesture. Touching one control already takes that one; this is for
/// a DJ who wants the machine off without hunting for eight of them.
#[tauri::command]
pub fn assistant_take_over(state: State<'_, AppState>) -> Result<(), String> {
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.takeover.take_all();
    Ok(())
}

/// Hand everything back.
///
/// One gesture, whatever was taken and however. A DJ resuming should not have
/// to remember what they touched.
#[tauri::command]
pub fn assistant_hand_back(state: State<'_, AppState>) -> Result<(), String> {
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.takeover.release_all();
    Ok(())
}

/// What the assistant would do next, given everything.
///
/// Shared by the panel and (later) by the tick that acts on it, so what is
/// shown and what is done cannot drift apart.
fn decide(state: &AppState, conduct: &crate::state::Conduct) -> crate::autopilot::Decision {
    let situation = read_situation(state, conduct);
    crate::autopilot::next_step(&situation, &conduct.takeover, &conduct.authority)
}

/// Assemble what the autopilot needs from the live application.
fn read_situation(
    state: &AppState,
    conduct: &crate::state::Conduct,
) -> crate::autopilot::Situation {
    let registry = state.registry();
    let read = |deck: dj_core::DeckId, p| f64::from(registry.get(dj_core::ParamId::Deck(deck, p)));

    // The live deck is the loaded one furthest through its track. With one
    // deck playing that is simply it; with two mid-transition it is the one
    // going out, which is the one the plan is about.
    let decks: Vec<dj_core::DeckId> = (1..=state.deck_count())
        .filter_map(|n| dj_core::DeckId::from_human(u8::try_from(n).ok()?))
        .collect();
    let live = decks
        .iter()
        .copied()
        .filter(|d| read(*d, dj_core::param::DeckParam::Loaded) > 0.5)
        .max_by(|a, b| {
            read(*a, dj_core::param::DeckParam::Position)
                .partial_cmp(&read(*b, dj_core::param::DeckParam::Position))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .or_else(|| decks.first().copied())
        // A deck must exist for the rest to mean anything; deck 1 always does.
        .unwrap_or_else(|| dj_core::DeckId::from_human(1).expect("deck 1 exists"));

    let idle = decks
        .iter()
        .copied()
        .find(|d| *d != live && read(*d, dj_core::param::DeckParam::Loaded) <= 0.5);

    let grid = state.waveforms().grid(live.human_number());
    let rate = grid.map_or(dj_core::SampleRate::DEFAULT, |g| g.sample_rate);

    let outgoing = crate::plan::Outgoing {
        position: read(live, dj_core::param::DeckParam::Position),
        length: read(live, dj_core::param::DeckParam::LengthFrames),
        bpm: grid.map_or(120.0, |g| g.grid.bpm.get()),
        phrase: grid.and_then(|g| g.phrase),
        key: None,
        sample_rate: rate,
        grid_anchor: grid.map_or(0.0, |g| g.grid.anchor.get()),
    };

    // What is on the idle deck, if anything, and what it is. Read from the
    // library rather than from the snapshot so the incoming record is described
    // by the same numbers the outgoing one is -- comparing like with like
    // matters more than saving a lookup.
    let staged = idle
        .and_then(|deck| {
            let tracks = state.deck_tracks();
            let map = tracks.lock().ok()?;
            map.get(&deck.human_number()).map(|t| t.id)
        })
        .and_then(|id| {
            let db = state.library().get().ok()?;
            let track = db.track(id).ok()??;
            Some((
                id,
                crate::plan::Incoming {
                    bpm: track.analysis.bpm.unwrap_or(outgoing.bpm),
                    phrase: dj_core::Phrase::new(
                        track.analysis.phrase_beats?,
                        track.analysis.phrase_anchor?,
                    ),
                    key: track.analysis.key(),
                },
            ))
        });

    // The next record from the set, skipping anything already on a deck.
    let next = conduct
        .setlist
        .iter()
        .skip(conduct.played)
        .find(|id| Some(**id) != staged.as_ref().map(|(id, _)| *id))
        .copied();

    // How much trim would match the staged record to the playing one. Both
    // loudnesses or nothing: half of a comparison is not a comparison.
    let gain_offset_db = staged.as_ref().and_then(|(id, _)| {
        let db = state.library().get().ok()?;
        let live_track = {
            let tracks = state.deck_tracks();
            let map = tracks.lock().ok()?;
            map.get(&live.human_number()).map(|t| t.id)
        }?;
        let a = db.track(live_track).ok()??.analysis.loudness_lufs?;
        let b = db.track(*id).ok()??.analysis.loudness_lufs?;
        Some(a - b)
    });

    crate::autopilot::Situation {
        posture: conduct.posture,
        occasion: conduct.occasion,
        // §81, so the mix the machine performs is the one the rail and the
        // ghost drew.
        //
        // Only when there is something staged, which is the same guard
        // `gain_offset_db` above uses and for the same reason: the style is
        // consulted in the one branch that plans a mix, and that branch needs a
        // staged record to plan into. This assembly runs on the assistant's
        // tick, twice a second, and `usual_style` is a night lookup and a fold
        // over every night of that setting — worth paying when a mix is
        // actually being planned and worth nothing at all when it is not.
        usual: staged.as_ref().and_then(|_| usual_style(state)),
        // What the context engine has made of the night, or `Fair` where it has
        // not made anything of it yet -- the assistant is not held back for the
        // six minutes the engine needs before it can speak.
        certainty: state
            .night()
            .read()
            .map_or(dj_core::Certainty::Fair, |read| read.certainty),
        live,
        outgoing,
        idle,
        staged,
        next,
        gain_offset_db,
    }
}

/// One line naming what a step is, for the panel.
fn describe_step(step: &crate::autopilot::Step) -> String {
    use crate::autopilot::Step;
    match step {
        Step::Nothing => "nothing".to_owned(),
        Step::Stage { deck, .. } => format!("load deck {}", deck.human_number()),
        Step::Cue { deck, .. } => format!("cue deck {} to the phrase", deck.human_number()),
        Step::Sync { deck } => format!("sync deck {}", deck.human_number()),
        Step::MatchGain { deck, db } => {
            format!("trim deck {} by {db:+.1} dB", deck.human_number())
        }
        Step::Mix {
            from, to, beats, ..
        } => format!(
            "mix deck {} into deck {} over {beats} beats",
            from.human_number(),
            to.human_number()
        ),
    }
}

/// How one record joins the one before it, or `None` for the first.
///
/// Built from the same scorer the rail uses, so a seam inside a plan and a
/// candidate in the rail are judged by one set of weights and described in one
/// vocabulary. Two rankings that disagree about the same pair of records is the
/// bug this avoids.
fn link_between(before: &dj_library::LibraryTrack, after: &dj_library::LibraryTrack) -> LinkDto {
    use dj_core::{Blendability, genre};
    use dj_library::suggest::{Playing, Reason, Trajectory, score};

    // `Hold` rather than the arc's own trajectory: this describes the seam, not
    // whether it takes the room where the arc wanted. A loudness jump is worth
    // the same warning whichever direction the set was going in.
    let scored = score(&Playing::of(before), Trajectory::Hold, after);

    // A grammar change is not in the scorer, because the scorer ranks
    // candidates and the assembler is what places a cut deliberately. It is a
    // risk here for the same reason it is a rule there: dembow into
    // four-on-the-floor is a cut however well the tempos match.
    let family = |t: &dj_library::LibraryTrack| t.tags.genre.as_deref().and_then(genre::family_for);
    let cut = match (family(before), family(after)) {
        (Some(a), Some(b)) => a.blends_with(b) == Blendability::Cut,
        _ => false,
    };

    let risky = cut
        || scored.reasons.iter().any(|r| {
            matches!(
                r,
                Reason::KeyClash { .. } | Reason::TempoFar { .. } | Reason::Unanalysed
            )
        });

    LinkDto {
        summary: summarise_reasons(&scored.reasons),
        confidence: scored.confidence(),
        risky,
    }
}

/// Turn a plan into its interface shape, filling in the seam between each pair.
///
/// One pass to resolve the records, then a second to describe the joins. The
/// link belongs between *adjacent slots in the output*, and the first pass can
/// drop a slot whose record has left the library, so the two cannot be one
/// pass without occasionally describing a seam that is not in the plan.
fn slots_with_links(
    slots: &[dj_library::setlist::Slot],
    pool: &[dj_library::LibraryTrack],
) -> Vec<SetlistSlotDto> {
    let resolved: Vec<_> = slots
        .iter()
        .filter_map(|slot| {
            let track = pool.iter().find(|t| t.id == slot.track)?;
            Some((track, slot))
        })
        .collect();

    resolved
        .iter()
        .enumerate()
        .map(|(index, (track, slot))| SetlistSlotDto {
            track: LibraryTrackDto::from((*track).clone()),
            through: slot.through,
            trajectory: trajectory_name(slot.trajectory).to_owned(),
            reasons: slot.reasons.iter().map(describe_reason).collect(),
            link: index
                .checked_sub(1)
                .map(|before| link_between(resolved[before].0, track)),
        })
        .collect()
}

/// One track in an assembled set, with the reasoning that placed it.
#[derive(Debug, Clone, Serialize)]
pub struct SetlistSlotDto {
    pub track: LibraryTrackDto,
    /// Where in the set it falls, 0..=1.
    pub through: f32,
    /// `lift`, `hold` or `ease` -- what the arc wanted at this point.
    pub trajectory: String,
    pub reasons: Vec<String>,
    /// The join **from the record before this one**, empty for the first.
    ///
    /// A plan is a list of tracks and a set is a list of *transitions*; the
    /// two are not the same list. What a DJ reading a plan wants to know is
    /// where it is going to be difficult, and that is a property of the seam
    /// rather than of either record. §20's Set Flow calls these the transition
    /// links and the risk markers, and this is both of them.
    pub link: Option<LinkDto>,
}

/// How one record joins the one before it.
#[derive(Debug, Clone, Serialize)]
pub struct LinkDto {
    /// The deltas across the seam, on one line: `+3 BPM \u{b7} 8A\u{2192}9A`.
    pub summary: String,
    /// How well the two go together, 0 to 1. The same scale the rail draws.
    pub confidence: f64,
    /// True when this seam needs a decision rather than a blend: a key clash,
    /// a tempo outside the deck's range, or a change of rhythmic grammar.
    ///
    /// Marked rather than avoided. A set with no difficult seams is a set that
    /// never went anywhere, and the assembler is allowed to place one where it
    /// judges the room can take it -- what it must not do is let the DJ meet it
    /// for the first time at 01:40.
    pub risky: bool,
}

/// Build a whole set before playing any of it.
///
/// The suggester answers "what next"; this answers "what is the whole night".
/// It asks the suggester repeatedly, each answer becoming the next question,
/// and shapes the result with an **arc** -- without one every step is locally
/// optimal and the set is an hour at a single energy.
///
/// `arc` is `rising`, `journey`, `flat` or `descent`. `favours` and `avoids`
/// are genre names or aliases: favours tilt the ranking, avoids are honoured
/// strictly. An avoided genre is not a preference to be balanced.
#[tauri::command]
pub fn setlist_build(
    state: State<'_, AppState>,
    arc: String,
    minutes: f64,
    favours: Vec<String>,
    avoids: Vec<String>,
) -> Result<Vec<SetlistSlotDto>, String> {
    use dj_library::setlist::{Arc as SetArc, Taste, assemble};

    let db = library(&state)?;
    let arc = match arc.as_str() {
        "rising" => SetArc::Rising,
        "flat" => SetArc::Flat,
        "descent" => SetArc::Descent,
        _ => SetArc::Journey,
    };
    let taste = Taste { favours, avoids };

    // The whole analysed library is the pool. Ranking is cheap arithmetic per
    // track; reading the rows is what costs, so the limit is generous and the
    // shaping happens afterwards.
    let pool = db.all_tracks(5_000).map_err(|e| e.to_string())?;

    Ok(slots_with_links(
        &assemble(&pool, arc, &taste, minutes, None),
        &pool,
    ))
}

/// One slot of a plan, as the interface holds it.
///
/// The plan lives in the interface and is handed back for each change, rather
/// than being kept here. A plan being edited is not application state — it is
/// a draft, and a draft the backend remembered would be one more thing to get
/// out of step with what is on screen.
#[derive(Debug, Clone, Deserialize)]
pub struct PlanSlotIn {
    pub track: String,
    pub through: f32,
    /// `lift`, `hold` or `ease`.
    pub trajectory: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SteeredDto {
    pub plan: Vec<SetlistSlotDto>,
    /// One line saying what just happened to the set.
    pub summary: String,
    /// How many upcoming slots actually changed. Zero is a real answer:
    /// "nothing needed to change" is different from "done".
    pub changed: usize,
}

fn trajectory_from(name: &str) -> dj_core::Trajectory {
    match name {
        "lift" => dj_core::Trajectory::Lift,
        "ease" => dj_core::Trajectory::Ease,
        _ => dj_core::Trajectory::Hold,
    }
}

fn trajectory_name(t: dj_core::Trajectory) -> &'static str {
    match t {
        dj_core::Trajectory::Lift => "lift",
        dj_core::Trajectory::Ease => "ease",
        dj_core::Trajectory::Hold => "hold",
    }
}

/// Adjust a plan without throwing it away.
///
/// The difference from rebuilding: a DJ who says "take it up from here" has
/// not asked for a different night. Everything already played stays, the next
/// record stays — it may be cued, staged or have a hand on its fader — and the
/// rest is rechosen.
///
/// `instruction` is `lift`, `ease`, `hold`, `favour`, `avoid`, `next`, `later`
/// or `drop`. `argument` is a genre name for favour and avoid, and a track id
/// for the last three.
#[tauri::command]
pub fn setlist_steer(
    state: State<'_, AppState>,
    plan: Vec<PlanSlotIn>,
    played: usize,
    instruction: String,
    argument: Option<String>,
) -> Result<SteeredDto, String> {
    use dj_library::steer::{Steer, steer};

    let db = library(&state)?;
    let pool = db.all_tracks(5_000).map_err(|e| e.to_string())?;

    let slots: Vec<dj_library::setlist::Slot> = plan
        .iter()
        .map(|s| {
            Ok(dj_library::setlist::Slot {
                track: parse_track_id(&s.track)?,
                through: s.through,
                trajectory: trajectory_from(&s.trajectory),
                // The reasons belong to the choice that was made, and steering
                // makes new choices. Carrying the old ones through would put a
                // stale explanation under a replaced record.
                reasons: Vec::new(),
            })
        })
        .collect::<Result<_, String>>()?;

    let needs_genre = || {
        argument
            .clone()
            .filter(|a| !a.trim().is_empty())
            .ok_or_else(|| format!("{instruction} needs a genre"))
    };
    let needs_track = || {
        argument
            .as_deref()
            .ok_or_else(|| format!("{instruction} needs a track"))
            .and_then(parse_track_id)
    };

    let wanted = match instruction.as_str() {
        "lift" => Steer::Lift,
        "ease" => Steer::Ease,
        "hold" => Steer::Hold,
        "favour" => Steer::Favour(needs_genre()?),
        "avoid" => Steer::Avoid(needs_genre()?),
        "next" => Steer::Next(needs_track()?),
        "later" => Steer::Later(needs_track()?),
        "drop" => Steer::Drop(needs_track()?),
        other => return Err(format!("{other:?} is not a way to steer a set")),
    };

    let out = steer(&slots, played, &wanted, &pool);
    Ok(SteeredDto {
        plan: slots_with_links(&out.plan, &pool),
        summary: out.summary,
        changed: out.changed,
    })
}

/// Turn a plan into a playlist that outlives the panel.
///
/// One call rather than a create followed by twenty adds: a plan half-written
/// because the twelfth call failed is worse than one not written at all, and
/// the interface has no way to finish the job from there.
#[tauri::command]
pub fn setlist_save(
    state: State<'_, AppState>,
    name: String,
    tracks: Vec<String>,
) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a set needs a name to be found again".into());
    }
    let ids = parse_track_ids(&tracks)?;
    let db = library(&state)?;
    let playlist = db
        .create_playlist(
            name,
            None,
            dj_library::PlaylistKind::List,
            None,
            crate::library::now_seconds(),
        )
        .map_err(|e| e.to_string())?;
    for id in ids {
        db.add_to_playlist(playlist, id)
            .map_err(|e| e.to_string())?;
    }
    Ok(playlist)
}

/// The genre families djmanzo knows, for an interface offering them.
#[derive(Debug, Clone, Serialize)]
pub struct GenreFamilyDto {
    pub name: String,
    pub region: String,
    /// The tempo a body moves at, which is not always the written one.
    pub felt_bpm: (f32, f32),
    pub grammar: String,
}

/// Every genre family, so the taste picker offers real names rather than a
/// free-text box nobody can spell into.
#[tauri::command]
#[must_use]
pub fn genre_families() -> Vec<GenreFamilyDto> {
    dj_core::genre::families()
        .iter()
        .map(|f| GenreFamilyDto {
            name: f.name.to_owned(),
            region: format!("{:?}", f.region),
            felt_bpm: f.felt_bpm(),
            grammar: format!("{:?}", f.grammar),
        })
        .collect()
}

/// What a saved set contains, without reading the whole thing back.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummaryDto {
    pub path: String,
    pub events: usize,
    pub seconds: f64,
    /// Distinct tracks that went on a deck.
    pub tracks: usize,
}

/// Write the set so far to a file.
///
/// The file is text, one event per line, in the same words an action is written
/// in everywhere else -- so it can be read, annotated and diffed. See
/// `crate::session`.
#[tauri::command]
pub fn session_save(state: State<'_, AppState>, path: String) -> Result<SessionSummaryDto, String> {
    let session = crate::session::Session {
        events: state.bus().log(),
    };
    let path = std::path::PathBuf::from(path);
    session.write(&path).map_err(|e| e.to_string())?;
    Ok(SessionSummaryDto {
        path: path.to_string_lossy().into_owned(),
        events: session.events.len(),
        seconds: session.duration().as_secs_f64(),
        tracks: session.tracks().len(),
    })
}

/// Read a saved set and say what is in it.
///
/// Deliberately does *not* replay it. Opening a file and having a set start
/// playing would be the worst possible behaviour in a booth; the DJ looks
/// first.
#[tauri::command]
pub fn session_open(path: String) -> Result<SessionSummaryDto, String> {
    let path = std::path::PathBuf::from(path);
    let session = crate::session::Session::read(&path)?;
    Ok(SessionSummaryDto {
        path: path.to_string_lossy().into_owned(),
        events: session.events.len(),
        seconds: session.duration().as_secs_f64(),
        tracks: session.tracks().len(),
    })
}

/// Decode the records a set loaded, once each.
///
/// Kept rather than re-decoded per load: a set that brings a record back for a
/// second play should not pay for it twice, and a DJ's crate is small enough
/// that holding it is cheaper than the disk.
fn decoder(
    db: Arc<dj_library::Library>,
) -> impl FnMut(dj_core::TrackId) -> Option<Arc<dyn dj_decode::TrackSource>> {
    let mut decoded: std::collections::HashMap<dj_core::TrackId, Arc<dyn dj_decode::TrackSource>> =
        std::collections::HashMap::new();
    move |id: dj_core::TrackId| -> Option<Arc<dyn dj_decode::TrackSource>> {
        if let Some(found) = decoded.get(&id) {
            return Some(Arc::clone(found));
        }
        let track = db.track(id).ok().flatten()?;
        let loaded = dj_decode::decode_file(&track.path).ok()?;
        let source: Arc<dyn dj_decode::TrackSource> = Arc::new(loaded.buffer);
        decoded.insert(id, Arc::clone(&source));
        Some(source)
    }
}

/// How much run-up one mix is rendered with.
///
/// Eight seconds. A transition is not a thing you can judge from its own
/// duration alone — what it sounds like depends on what was already playing —
/// and eight seconds is about four bars at a danceable tempo, which is enough
/// to hear where the outgoing record was before anything moved.
const MIX_LEAD_IN: f64 = 8.0;

/// And how long after it lands.
///
/// Four seconds: long enough to hear the incoming record standing on its own,
/// short enough that the file is about the mix rather than about the next
/// track.
const MIX_TAIL: f64 = 4.0;

/// Render one of tonight's mixes back to a WAV, in context.
///
/// §68's object driving replay. `crate::mixes` says when each handover
/// happened and how long it took; this hands those two numbers to
/// `replay::Window` and renders that stretch of the set the DJ is playing.
///
/// **It is not a seek, and this is where the cost is.** The engine's state at
/// any moment is the whole set up to it, so everything before the mix is
/// rendered and thrown away — a mix from the third hour means rendering three
/// hours. Replay runs to no deadline and is far faster than real time, but it
/// is not free, and a caller should say so rather than let a DJ think a button
/// is broken.
///
/// The file lands in the recordings folder beside the settings, named for
/// where in the set it came from. Not the music folder, for the same reason
/// recordings are not: the browser would find it and offer it as a track.
#[tauri::command]
pub fn session_render_mix(
    state: State<'_, AppState>,
    at: f64,
    took_seconds: f64,
) -> Result<String, String> {
    let db = library(&state)?;
    let rate = dj_core::SampleRate::DEFAULT;
    let session = crate::session::Session {
        events: state.bus().log(),
    };
    if session.events.is_empty() {
        return Err("nothing has happened yet tonight".to_owned());
    }

    let from = (at - MIX_LEAD_IN).max(0.0);
    let to = at + took_seconds.max(0.0) + MIX_TAIL;
    let dir = state
        .recordings_dir()
        .ok_or_else(|| "no settings folder to write into yet".to_owned())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Named for where in the set it came from, so two mixes from one night do
    // not overwrite each other and the name says which is which.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let path = dir.join(format!("mix-at-{}s.wav", at.max(0.0).round() as u64));

    let mut resolve = decoder(db);
    let rendered = crate::replay::render_to_wav(
        &session,
        rate,
        state.deck_count(),
        0,
        crate::replay::Window::seconds(from, to, rate),
        &mut resolve,
        &path,
    )?;

    // What came out, not what it cost: reporting the run-up would tell a DJ
    // their twenty-second mix is three hours long.
    Ok(format!(
        "{:.0}s → {}",
        rendered.emitted as f64 / rate.as_f64(),
        path.display()
    ))
}

/// One rehearsal, as the practice surface draws it.
#[derive(Debug, Clone, Serialize)]
pub struct RehearsalDto {
    /// The style that was rehearsed, as the grammar spells it.
    pub style: String,
    /// Where the file landed.
    pub path: String,
    /// How long it runs.
    pub seconds: f64,
    /// Where the mix itself sits inside it, so a player can mark it.
    pub mix_from: f64,
    pub mix_to: f64,
    /// How many actions the automix sent. A number a DJ can compare between
    /// styles: a cut is a handful, a blend is a thousand fader writes.
    pub actions: usize,
    /// What the style does beyond the faders, from the same table the automix
    /// performs -- so the file and the description cannot disagree.
    pub shape: ShapeDto,
}

/// Rehearse the held transition, and hear it.
///
/// §69's practice surface: "two tracks can be explored **without altering the
/// live master**." Nothing here touches the live engine. The automix is run
/// offline against a simulated playhead, its actions become a set file that
/// was never played, and `replay` renders that file headless -- see
/// [`crate::practice`]. The DJ's records keep playing to the room throughout.
///
/// `style` rehearses an alternative **without restyling the held mix**, which
/// is the same promise one level up: trying a vocal drop in the lab must not
/// change the mix the automix is about to perform. The transition is cloned
/// and the clone is restyled; djmanzo goes on holding what it held.
///
/// Unlike [`session_render_mix`] this is cheap. A rehearsal is synthetic, so
/// it has no history to be faithful to: it renders the run-up, the mix and the
/// tail, and nothing else, whatever hour of the night it is.
#[tauri::command]
pub fn practice_rehearse(
    state: State<'_, AppState>,
    style: Option<String>,
) -> Result<RehearsalDto, String> {
    let Some(mut transition) = state.transition() else {
        return Err("set a transition up in the pair view first".to_owned());
    };
    if let Some(word) = style.as_deref() {
        let style = dj_core::action::TransitionStyle::parse(word)
            .ok_or_else(|| format!("no {word} style"))?;
        // On the clone. `state.transition()` handed back a copy, and nothing
        // here writes it back.
        transition.set_style(style);
    }

    let db = library(&state)?;
    let track = db
        .track(transition.incoming_track)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "the record coming in has left the library".to_owned())?;
    let grid = track
        .analysis
        .beatgrid()
        .ok_or_else(|| "the record coming in has no beatgrid to rehearse against".to_owned())?;
    #[allow(clippy::cast_precision_loss)]
    let incoming = crate::plan::Record {
        length: track.duration_frames as f64,
        bpm: grid.bpm.get(),
        phrase: phrase_of(&track),
        sample_rate: track.sample_rate,
        grid_anchor: grid.anchor.get(),
    };

    let rehearsal = crate::practice::rehearse(
        &transition,
        incoming,
        crate::practice::RUN_UP,
        crate::practice::TAIL,
    );

    let dir = state
        .recordings_dir()
        .ok_or_else(|| "no settings folder to write into yet".to_owned())?;
    let dir = dir.join("practice");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Named for the pair and the style, so rehearsing the same mix four ways
    // leaves four files a DJ can play against each other -- which is §69's
    // "hear alternative transitions" as files rather than as a promise.
    let name = format!(
        "{}-{}-{}.wav",
        short(transition.outgoing_track),
        short(transition.incoming_track),
        transition.plan.style.as_str().replace(' ', "-")
    );
    let path = dir.join(name);

    let mut resolve = decoder(db);
    crate::replay::render_to_wav(
        &rehearsal.session,
        transition.outgoing().sample_rate,
        rehearsal.decks,
        // The tail. Nothing is *sent* during it, so a render that stopped at
        // the last event would end the file the instant the mix landed.
        rehearsal.tail_frames,
        crate::replay::Window::WHOLE,
        &mut resolve,
        &path,
    )?;

    Ok(RehearsalDto {
        style: transition.plan.style.as_str().to_owned(),
        path: path.display().to_string(),
        seconds: rehearsal.seconds,
        mix_from: rehearsal.mix_from,
        mix_to: rehearsal.mix_to,
        actions: rehearsal.session.events.len(),
        shape: describe_shape(&transition.shape()),
    })
}

/// The first eight characters of a track id, for a filename.
///
/// Enough to tell two records apart in a folder and short enough that the name
/// stays readable. Not the title: a title has slashes and colons in it.
fn short(track: dj_core::TrackId) -> String {
    track.to_hex().chars().take(8).collect()
}

/// Re-render a saved set to a WAV file.
///
/// Faster than real time, and with nothing dropped: a replay runs to no
/// deadline, so an underrun cannot put a hole in it the way one can in a live
/// recording. The same file and the same records produce byte-identical audio
/// every time -- see `crate::replay`.
///
/// Every track the set loaded is decoded from the library. A set that
/// references a record the library no longer has is refused by name rather than
/// rendered with a silent deck, which would be quietly wrong.
#[tauri::command]
pub fn session_render(
    state: State<'_, AppState>,
    session_path: String,
    out_path: String,
    tail_seconds: f64,
) -> Result<String, String> {
    let db = library(&state)?;
    let session = crate::session::Session::read(std::path::Path::new(&session_path))?;
    let rate = dj_core::SampleRate::DEFAULT;
    let mut resolve = decoder(db);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tail = (tail_seconds.max(0.0) * rate.as_f64()) as u64;
    let rendered = crate::replay::render_to_wav(
        &session,
        rate,
        state.deck_count(),
        tail,
        crate::replay::Window::WHOLE,
        &mut resolve,
        std::path::Path::new(&out_path),
    )?;

    Ok(format!(
        "{:.0}s from {} events → {out_path}",
        rendered.frames as f64 / rate.as_f64(),
        rendered.events
    ))
}

/// One difference between two takes of a set.
#[derive(Debug, Clone, Serialize)]
pub struct DivergenceLineDto {
    /// `only_in_first`, `only_in_second` or `drift`.
    pub kind: String,
    pub event: String,
    /// Seconds. For a drift, how much later the second take was.
    pub seconds: f64,
}

/// Compare two takes of the same set.
///
/// Not a text diff: two takes are the same decisions at different times, and a
/// line comparison of a file whose first column is a timestamp calls every line
/// changed. This reports which moves differ and how far they drifted.
#[tauri::command]
pub fn session_diff(first: String, second: String) -> Result<Vec<DivergenceLineDto>, String> {
    let a = crate::session::Session::read(std::path::Path::new(&first))?;
    let b = crate::session::Session::read(std::path::Path::new(&second))?;
    let d = crate::session::diff(&a, &b);

    let mut out = Vec::new();
    for entry in d.only_in_first {
        out.push(DivergenceLineDto {
            kind: "only_in_first".to_owned(),
            event: entry.event.to_line(),
            seconds: entry.at.as_secs_f64(),
        });
    }
    for entry in d.only_in_second {
        out.push(DivergenceLineDto {
            kind: "only_in_second".to_owned(),
            event: entry.event.to_line(),
            seconds: entry.at.as_secs_f64(),
        });
    }
    for (event, delta) in d.drifted {
        out.push(DivergenceLineDto {
            kind: "drift".to_owned(),
            event: event.to_line(),
            seconds: delta,
        });
    }
    Ok(out)
}

/// One record of a pair, as the pair view draws it.
///
/// The library row plus the two things a DJ comparing two records asks for
/// that a row does not carry: what its phrase structure is, and what it is
/// *for*. Both come from the same database read, so asking for a pair is one
/// query rather than three.
#[derive(Debug, Clone, Serialize)]
pub struct PairSideDto {
    /// 1-based deck number, as the interface counts them.
    pub deck: u8,
    pub track: LibraryTrackDto,
    /// Phrase length in beats, when the analyser found a structure. `None` is
    /// a real answer -- plenty of records have none -- and the difference
    /// matters here, because a mix planned onto a bar line is a weaker
    /// proposition than one planned onto a phrase.
    pub phrase_beats: Option<u32>,
    /// Standard notation for the key, beside the Camelot the row already
    /// carries. A DJ reading two records side by side wants both: Camelot to
    /// do the arithmetic, `Am` to know what it sounds like.
    pub key_standard: Option<String>,
    /// What the record is for, as function slugs.
    pub functions: Vec<String>,
}

/// What one style does beyond the two channel faders.
///
/// The interface shape of [`crate::shape::Shape`] -- §68's `outgoingStems`,
/// `incomingStems`, `eqPlan` and `fxPlan`. `does` is the same thing in the
/// words a panel can print, derived from the fields beside it rather than
/// written separately, so a tooltip cannot describe a mix djmanzo no longer
/// performs.
#[derive(Debug, Clone, Serialize)]
pub struct ShapeDto {
    /// Whether the two records are ever audible at the same time.
    pub overlaps: bool,
    /// The stem soloed on the outgoing deck for the length of the mix, if any.
    pub outgoing_stem: Option<String>,
    pub incoming_stem: Option<String>,
    /// The fraction of the transition by which the low-EQ handover has
    /// finished. `None` when the style does not touch the EQ at all, which is
    /// not the same as a handover that takes the whole mix.
    pub eq_done_by: Option<f64>,
    pub fx: Option<ShapeFxDto>,
    pub does: Vec<String>,
}

/// The effect a style throws over the outgoing deck.
#[derive(Debug, Clone, Serialize)]
pub struct ShapeFxDto {
    pub effect: String,
    pub beats: f32,
    pub slot: u8,
}

/// One transition style, and what it does.
///
/// Served rather than written on the interface side: the automix performs
/// [`crate::shape`]'s table, so the buttons that offer a style read the same
/// table instead of a second description of it that nothing keeps true. It is
/// also how the interface learns a style exists -- `vocal drop` was in the
/// vocabulary and performed by the automix while no panel offered it, because
/// the list of styles was hand-written twice.
#[derive(Debug, Clone, Serialize)]
pub struct StyleDto {
    /// Exactly as the action grammar spells it, so a button's label is also
    /// the word `automix style <name>` takes.
    pub name: String,
    pub shape: ShapeDto,
}

/// Turn a shape into what the interface draws.
fn describe_shape(shape: &crate::shape::Shape) -> ShapeDto {
    ShapeDto {
        overlaps: shape.overlaps,
        outgoing_stem: shape.outgoing_stems.solo().map(|s| s.name().to_owned()),
        incoming_stem: shape.incoming_stems.solo().map(|s| s.name().to_owned()),
        eq_done_by: match shape.eq {
            crate::shape::Eq::Flat => None,
            crate::shape::Eq::HandOverLows { done_by } => Some(done_by),
        },
        fx: shape.fx.outgoing().map(|(slot, kind, beats)| ShapeFxDto {
            effect: kind.name().to_owned(),
            beats,
            slot,
        }),
        does: shape.words(),
    }
}

/// Every transition style, in the vocabulary's own order.
///
/// A read of a table; it holds nothing and moves nothing.
#[tauri::command]
#[must_use]
pub fn transition_styles() -> Vec<StyleDto> {
    dj_core::action::TransitionStyle::ALL
        .into_iter()
        .map(|style| StyleDto {
            name: style.as_str().to_owned(),
            shape: describe_shape(&crate::shape::shape(style)),
        })
        .collect()
}

/// A transition, flattened for the interface.
///
/// The interface shape of [`crate::transition::Transition`], which is §68's
/// transition object. Everything here is either the object's own or looked up
/// from the two records it names; nothing is re-derived on this side, so a
/// panel cannot disagree with the object about where the mix starts.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionDto {
    pub outgoing: PairSideDto,
    pub incoming: PairSideDto,
    /// Beat index in the outgoing track where the mix should begin.
    pub start_beat: i64,
    /// Seconds into the outgoing track, for a display that speaks in time.
    pub start_seconds: f64,
    /// Where it finishes, in the same terms.
    pub end_seconds: f64,
    /// The same two points in frames, which is what the waveform is drawn in.
    ///
    /// Carried rather than converted on the other side: seconds times a sample
    /// rate the interface would have to infer from a deck's length is two
    /// roundings and a division by zero waiting for an empty deck.
    pub start_frame: f64,
    pub end_frame: f64,
    pub length_beats: u32,
    /// The style's own name, as the automix panel already spells it.
    pub style: String,
    /// Incoming tempo minus outgoing, signed.
    pub bpm_delta: f64,
    /// `same key`, `neighbour`, `relative major/minor`, `tritone` or
    /// `distant`. `None` when either record is unanalysed, which is not the
    /// same as a clash.
    pub key_relation: Option<String>,
    /// How well the two records go together, 0 to 1 -- the same number the
    /// Next rail draws, from the same scorer.
    pub confidence: f64,
    /// True once a human has moved, shortened or restyled it.
    pub edited: bool,
    /// True when this is the transition djmanzo is holding, rather than one it
    /// was just asked about.
    pub armed: bool,
    pub reasons: Vec<String>,
    /// What this style does beyond the faders, so the pair view can say what
    /// pressing the button will do before it is pressed.
    pub shape: ShapeDto,
}

/// Everything the planner needs about one deck, read from the live registry.
///
/// The live playhead, not the last snapshot: a plan is about where the track
/// is *now*, and a snapshot can be up to 16 ms stale -- which at 174 BPM is
/// most of a beat.
fn outgoing_of(
    state: &AppState,
    deck: dj_core::DeckId,
    track: &dj_library::LibraryTrack,
) -> Option<crate::plan::Outgoing> {
    let grid = track.analysis.beatgrid()?;
    let registry = state.registry();
    let read = |p| f64::from(registry.get(dj_core::ParamId::Deck(deck, p)));
    Some(crate::plan::Outgoing {
        position: read(dj_core::param::DeckParam::Position),
        length: read(dj_core::param::DeckParam::LengthFrames),
        bpm: grid.bpm.get(),
        phrase: phrase_of(track),
        key: track.analysis.key(),
        sample_rate: track.sample_rate,
        grid_anchor: grid.anchor.get(),
    })
}

/// Plan the mix between two decks, without holding the result.
///
/// The confidence comes from `dj_library::suggest` -- the scorer the Next rail
/// and Set Flow's seams already use -- rather than from anything here. Two
/// numbers on one screen that both claim to say how well two records go
/// together, and disagree, is the failure that avoids.
fn transition_between(
    state: &AppState,
    from: dj_core::DeckId,
    to: dj_core::DeckId,
) -> Result<Option<crate::transition::Transition>, String> {
    use dj_library::suggest::{Playing, Trajectory, score};

    let db = library(state)?;
    let Some(out_track) = current_track(state, from).and_then(|id| db.track(id).ok().flatten())
    else {
        return Ok(None);
    };
    let Some(in_track) = current_track(state, to).and_then(|id| db.track(id).ok().flatten()) else {
        return Ok(None);
    };
    let Some(outgoing) = outgoing_of(state, from, &out_track) else {
        return Ok(None);
    };
    let incoming = crate::plan::Incoming {
        bpm: in_track.analysis.bpm.unwrap_or(outgoing.bpm),
        phrase: phrase_of(&in_track),
        key: in_track.analysis.key(),
    };
    let confidence = score(&Playing::of(&out_track), Trajectory::Hold, &in_track).confidence();

    Ok(crate::transition::Transition::plan_as(
        (from, to),
        (out_track.id, in_track.id),
        outgoing,
        incoming,
        confidence,
        // §81, so the mix a DJ arms is the one the rail and the ghost showed
        // them. Held by the transition afterwards, so a replan reproduces
        // this answer rather than whatever the profile has become since.
        usual_style(state),
    ))
}

/// Turn a transition into what the interface draws.
///
/// `None` when either record has left the library since it was planned, which
/// is rare and is still not a reason to draw half a pair.
fn describe_transition(
    state: &AppState,
    transition: &crate::transition::Transition,
    armed: bool,
) -> Result<Option<TransitionDto>, String> {
    let db = library(state)?;
    let side = |deck: dj_core::DeckId, id: dj_core::TrackId| -> Option<PairSideDto> {
        let track = db.track(id).ok().flatten()?;
        Some(PairSideDto {
            deck: deck.human_number(),
            phrase_beats: track.analysis.phrase_beats,
            key_standard: track.analysis.key().map(|k| k.standard().to_owned()),
            functions: db
                .functions_for(id)
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.slug().to_owned())
                .collect(),
            track: LibraryTrackDto::from(track),
        })
    };
    let (Some(outgoing), Some(incoming)) = (
        side(transition.outgoing_deck, transition.outgoing_track),
        side(transition.incoming_deck, transition.incoming_track),
    ) else {
        return Ok(None);
    };

    Ok(Some(TransitionDto {
        outgoing,
        incoming,
        start_beat: transition.plan.start_beat,
        start_seconds: transition.start_seconds(),
        end_seconds: transition.end_seconds(),
        start_frame: transition.plan.start_frame,
        end_frame: transition.plan.end_frame,
        length_beats: transition.plan.length_beats,
        style: transition.plan.style.as_str().to_owned(),
        bpm_delta: transition.plan.bpm_delta,
        key_relation: transition.key_relation().map(|r| r.as_str().to_owned()),
        confidence: transition.confidence,
        edited: transition.edited,
        armed,
        reasons: transition
            .plan
            .reasons
            .iter()
            .map(describe_plan_reason)
            .collect(),
        shape: describe_shape(&transition.shape()),
    }))
}

/// Plan the mix out of `from_deck` and into `to_deck`, without holding it.
///
/// An opinion, not an instruction: asking for it moves nothing and remembers
/// nothing. [`transition_arm`] is the one that holds the answer.
///
/// `None` -- an empty result -- when there is nothing sensible to propose:
/// either deck empty, no grid, or the outgoing track already past its last
/// usable phrase. A planner that always answers is one that answers wrongly at
/// the end of a record, which is exactly when it is read.
#[tauri::command]
pub fn plan_transition(
    state: State<'_, AppState>,
    from_deck: u8,
    to_deck: u8,
) -> Result<Option<TransitionDto>, String> {
    let from = dj_core::DeckId::from_human(from_deck).ok_or("no such deck")?;
    let to = dj_core::DeckId::from_human(to_deck).ok_or("no such deck")?;
    match transition_between(&state, from, to)? {
        Some(transition) => describe_transition(&state, &transition, false),
        None => Ok(None),
    }
}

/// Plan the mix between two decks and **hold** it.
///
/// The difference from [`plan_transition`] is the whole of §68: an answer that
/// is held can be adjusted, drawn by something other than the panel that asked
/// for it, and still be there when that panel is closed and reopened.
///
/// Arming replaces whatever was held. One mix at a time -- two set-up
/// transitions would be two answers to "what happens next", and the interface
/// drawing them would have to choose.
#[tauri::command]
pub fn transition_arm(
    state: State<'_, AppState>,
    from_deck: u8,
    to_deck: u8,
) -> Result<Option<TransitionDto>, String> {
    let from = dj_core::DeckId::from_human(from_deck).ok_or("no such deck")?;
    let to = dj_core::DeckId::from_human(to_deck).ok_or("no such deck")?;
    let Some(transition) = transition_between(&state, from, to)? else {
        return Ok(None);
    };
    state.arm_transition(transition.clone());
    describe_transition(&state, &transition, true)
}

/// The transition djmanzo is holding, if it still describes what is loaded.
///
/// A held transition whose records have been replaced is **dropped rather than
/// drawn**. It is the one failure mode of holding a plan at all: a panel
/// showing a confident mix point for a record that left the deck four minutes
/// ago looks exactly like a current answer.
#[tauri::command]
pub fn transition_current(state: State<'_, AppState>) -> Result<Option<TransitionDto>, String> {
    let Some(transition) = state.transition() else {
        return Ok(None);
    };
    let loaded = |deck| current_track(&state, deck);
    if !transition.describes(
        loaded(transition.outgoing_deck),
        loaded(transition.incoming_deck),
    ) {
        state.clear_transition();
        return Ok(None);
    }
    describe_transition(&state, &transition, true)
}

/// Adjust the held transition: move it, shorten it, or change how it is done.
///
/// Every argument is optional and they compose, so one press that both
/// shortens and restyles is one call and one answer. `None` for all three is a
/// read, which is what the interface does after a deck moves.
///
/// The reasons come back **re-derived over the new geometry** — see
/// [`crate::transition`]. A transition moved off its phrase boundary says so.
#[tauri::command]
pub fn transition_adjust(
    state: State<'_, AppState>,
    move_beats: Option<i64>,
    length_beats: Option<u32>,
    style: Option<String>,
    // §26's *transition end*: where the DJ dropped the closing handle, in
    // frames on the outgoing record. A position rather than a length, because
    // that is what a waveform knows -- `Transition::end_at` turns it into
    // beats, where the tempo and the start already are.
    end_frame: Option<f64>,
) -> Result<Option<TransitionDto>, String> {
    let style = match style.as_deref() {
        Some(word) => Some(
            dj_core::action::TransitionStyle::parse(word)
                .ok_or_else(|| format!("no {word} style"))?,
        ),
        None => None,
    };
    let adjusted = state.edit_transition(|transition| {
        if let Some(beats) = move_beats {
            transition.move_start(beats);
        }
        if let Some(beats) = length_beats {
            transition.set_length(beats);
        }
        // After the two above, because a drag says where the end should be
        // *now*: a press that both moved the start and dropped the end would
        // otherwise land the end relative to a start it had already left.
        if let Some(frame) = end_frame {
            transition.end_at(frame);
        }
        if let Some(style) = style {
            transition.set_style(style);
        }
        transition.clone()
    });
    match adjusted {
        Some(transition) => describe_transition(&state, &transition, true),
        None => Ok(None),
    }
}

/// Throw the adjustments away and ask the planner again.
///
/// Only ever on request. A transition that replanned itself would undo a DJ's
/// adjustment at whatever moment it next recalculated.
#[tauri::command]
pub fn transition_replan(state: State<'_, AppState>) -> Result<Option<TransitionDto>, String> {
    let replanned = state.edit_transition(|transition| {
        transition.replan();
        transition.clone()
    });
    match replanned {
        Some(transition) => describe_transition(&state, &transition, true),
        None => Ok(None),
    }
}

/// Stop holding it.
#[tauri::command]
pub fn transition_clear(state: State<'_, AppState>) {
    state.clear_transition();
}

/// A track's phrase structure, when it has one stored.
fn phrase_of(track: &dj_library::LibraryTrack) -> Option<dj_core::Phrase> {
    dj_core::Phrase::new(track.analysis.phrase_beats?, track.analysis.phrase_anchor?)
}

/// §22: the mix into one candidate, from §27's own planner.
///
/// `None` for exactly the reasons the ghost answers `None`, and one more that
/// is the candidate's rather than the pair's: a record with no beat grid
/// cannot be planned into, and falling back to the outgoing tempo — which
/// `transition_between` does for a record that is *loaded*, where the deck has
/// one either way — would put a confident line beside a record nobody has
/// analysed.
fn estimate_transition(
    out: &crate::plan::Outgoing,
    candidate: &dj_library::LibraryTrack,
    usual: Option<dj_core::action::TransitionStyle>,
) -> Option<TransitionEstimateDto> {
    let grid = candidate.analysis.beatgrid()?;
    let ghost = crate::ghost::look(
        out,
        &crate::ghost::Candidate {
            bpm: grid.bpm.get(),
            phrase: phrase_of(candidate),
            key: candidate.analysis.key(),
            sample_rate: candidate.sample_rate,
            grid_anchor: grid.anchor.get(),
            // The rail draws a style and a length, not a drop. Left empty
            // rather than looked up: this runs once per candidate per refresh,
            // and reading a cache for a number nothing here draws would be
            // work for nobody. The vocal entry is absent on the same terms.
            drops: Vec::new(),
            voice_enters: None,
        },
        usual,
    )?;
    let at_seconds = ghost.plan.start_frame / out.sample_rate.as_f64();
    Some(TransitionEstimateDto {
        style: ghost.plan.style.as_str().to_owned(),
        length_beats: ghost.plan.length_beats,
        at_seconds,
        says: transition_words(
            ghost.plan.length_beats,
            ghost.plan.style.as_str(),
            at_seconds,
        ),
    })
}

/// A transition in one phrase: `32-beat blend at 2:09`.
///
/// One spelling, in Rust, because it is now drawn in two places — §27's ghost
/// panel and §22's rail — and two spellings of the same mix is the failure
/// `dj_app::shape` exists to prevent, at the scale of a sentence.
fn transition_words(length_beats: u32, style: &str, at_seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let at = crate::share::clock(at_seconds.max(0.0) as i64);
    format!("{length_beats}-beat {style} at {at}")
}

/// Render one planner reason for the interface. Terse, like the suggester's.
fn describe_plan_reason(reason: &crate::plan::Reason) -> String {
    use crate::plan::Reason;
    match reason {
        Reason::LandsOnPhrase { beat } => format!("phrase start (beat {beat})"),
        Reason::LandsOnBar { beat } => format!("bar line (beat {beat}) — no phrase structure"),
        Reason::Remaining { beats } => format!("{beats:.0} beats left"),
        Reason::TemposMatch { from, to } => format!("{from:.0} into {to:.0} BPM"),
        Reason::TemposClash { from, to } => format!("{from:.0} against {to:.0} BPM — too far"),
        Reason::KeysMatch => "keys sit together".to_owned(),
        Reason::KeysClash => "keys fight — keep it short".to_owned(),
        Reason::Rushed { beats_after } => {
            format!("only {beats_after:.0} beats after it ends")
        }
    }
}

/// One suggested next track, with the reasoning that produced it.
///
/// The reasons arrive as short strings rather than the typed `Reason` values,
/// because the interface renders them as chips and nothing on that side wants
/// to re-derive a sentence from an enum. The typing that matters happens in
/// `dj_library::suggest`, where the ranking can be argued with; this is the
/// last mile.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestionDto {
    pub track: LibraryTrackDto,
    pub score: f64,
    /// Human-readable, in the order the scorer produced them. Each is one
    /// `Reason`. The one-line `summary` below reorders them for reading; this
    /// keeps the scorer's order so the two can be compared.
    pub reasons: Vec<String>,
    /// The same reasons as one line of deltas: `+3 BPM · 8A→9A · +1 dB`.
    ///
    /// The rail is a rail -- eight candidates in a column narrow enough to sit
    /// beside the decks -- and a wrapped pile of chips per row is not something
    /// that can be read at a glance mid-transition. Deltas rather than
    /// absolutes, because what a DJ needs to know is what *changes*: 128 BPM
    /// means nothing without remembering what is playing, and `+3` means it
    /// immediately.
    pub summary: String,
    /// How much of the achievable score this got, 0 to 1. See
    /// `dj_library::suggest::Suggestion::confidence`.
    pub confidence: f64,
    /// §22's *estimated transition type*: what the mix into this record would
    /// be, if it were brought in.
    ///
    /// `None` when there is nothing honest to say — an empty deck, an
    /// unanalysed record on either side, or a track already too near its end
    /// for any transition the planner proposes to fit.
    pub transition: Option<TransitionEstimateDto>,
}

/// §22: what the mix into one candidate would be.
///
/// **The planner's own answer, not a second one.** It is `dj_app::ghost` —
/// the same call §27's overlay is drawn from — so the line in the rail, the
/// ghost band on the record and the mix djmanzo performs are one plan seen
/// three times. A rail that estimated for itself would be a fourth.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionEstimateDto {
    /// The style's own name, as the automix panel spells it.
    pub style: String,
    pub length_beats: u32,
    /// Where it would begin, in seconds into the outgoing record.
    pub at_seconds: f64,
    /// The same thing in one phrase — `32-beat blend at 2:09` — worded in
    /// Rust so the rail and the ghost panel cannot say it differently.
    pub says: String,
}

/// How many candidates a rail should carry.
///
/// Two sections meet here and they are not the same rule, so they are not
/// applied the same way.
///
/// **§43's fatigue thins it.** That is a signal the DJ generated themselves —
/// twenty records in a row that djmanzo did not suggest — so acting on it is
/// the assistant taking the hint, which is the section's whole instruction.
///
/// **§18's budget only silences it, and only in an emergency.** The budget is a
/// cap on what may be *put in front of* a DJ, and this panel is one they opened
/// and are looking at: a rail that dropped from eight rows to one the moment a
/// second deck became audible would look broken, and nothing on screen would say
/// why. That is the failure §17 avoided by gating on `reflow` instead of
/// second-guessing an open panel. The one case where the budget must win is
/// `Attention::emergency` — a recording that has failed, a headphone card that
/// has stopped — where §18's own words are that the DJ needs the controls, not
/// the advice, and a rail re-ranking at them is exactly the advice.
fn rail_size(asked: usize, budget: u8, fatigue: &dj_assistant::Fatigue) -> usize {
    if budget == 0 {
        return 0;
    }
    fatigue.allowance(asked.clamp(1, 100))
}

/// What to play after whatever is on `deck`.
///
/// `trajectory` is `lift`, `hold` or `ease`; anything else is treated as
/// `hold`, which is the default a set spends most of its time in and the safe
/// answer for a typo.
#[tauri::command]
pub fn suggest_next(
    state: State<'_, AppState>,
    deck: u8,
    trajectory: String,
    limit: usize,
) -> Result<Vec<SuggestionDto>, String> {
    use dj_library::suggest::{Playing, Trajectory};

    let db = library(&state)?;
    let deck_id = dj_core::DeckId::from_human(deck).ok_or("no such deck")?;

    // What is playing, read from the library rather than the snapshot: the
    // snapshot carries the analysis for display, but the library row is the
    // same numbers the candidates are being scored against, and comparing like
    // with like matters more than saving a query.
    let now = current_track(&state, deck_id)
        .and_then(|id| db.track(id).ok().flatten())
        .map_or_else(Playing::nothing, |t| Playing::of(&t));

    // Holding is what an unrecognised name gets here, and it is said here
    // rather than in `from_name`: a rail asked for a direction it has never
    // heard of should still rank, and holding is the one answer that adds
    // nothing of its own.
    let trajectory = Trajectory::from_name(&trajectory).unwrap_or(Trajectory::Hold);

    // A generous pool, then ranked and cut. Ranking is cheap arithmetic per
    // track; reading the rows is the part that costs, so the limit is applied
    // after scoring rather than before -- cutting first would rank an arbitrary
    // slice of the library.
    let pool = db.all_tracks(5_000).map_err(|e| e.to_string())?;
    let playing_now = current_track(&state, deck_id);

    // §24. What the DJ has kept going into, after this record — read once
    // rather than per candidate, because it is one indexed query for the whole
    // rail and five thousand of them would be five thousand.
    //
    // Applied after scoring rather than inside it: the scorer is a pure
    // function over two records and its whole test suite rests on that. This
    // is the layer that has a database.
    let kept: std::collections::HashMap<dj_core::TrackId, (u32, Option<f64>)> = playing_now
        .and_then(|from| db.kept_after(from).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|pair| (pair.into, (pair.kept, pair.loop_beats)))
        .collect();

    let ranked = with_kept(
        dj_library::suggest::rank(&now, trajectory, &pool)
            .into_iter()
            // Never suggest what is already on the deck.
            .filter(|s| Some(s.track) != playing_now)
            .collect(),
        &kept,
    );

    // The genre tag off each pool row, once. Two folds want it — §16's pack
    // asks which family it is, §12's profile asks what the DJ plays — and
    // reading `tags.genre` twice over five thousand rows to answer two
    // questions about the same string would be the cheaper-looking mistake.
    let genres: std::collections::HashMap<dj_core::TrackId, String> = pool
        .iter()
        .filter_map(|t| Some((t.id, t.tags.genre.clone()?)))
        .collect();

    // §16. The knowledge pack the DJ chose, which until now reached the coach
    // and nothing else: `families` was parsed, stored, sent to the interface
    // and acted on by no part of djmanzo. A pack is a narrowing the DJ *said* —
    // pressing *Latin* is a statement about what tonight is made of — and the
    // rail is where a statement about what tonight is made of has somewhere to
    // go.
    let chosen = state.chosen_pack();
    let ranked = with_pack(
        ranked,
        chosen.as_deref().and_then(dj_assistant::pack::pack),
        &|id| {
            genres
                .get(&id)
                .and_then(|tag| dj_core::genre::family_for(tag))
                .map(|family| family.name)
        },
    );

    // §17. What the phase of the night asks of the ranking, which until now
    // reached the cockpit and stopped there: `cockpit::priorities` opened
    // panels, and §17's *harmonic resolution* and *known anchors* — the two
    // entries in its six lists that are about what djmanzo should **offer** —
    // were named in a doc comment as belonging to the planner and belonged to
    // nobody.
    //
    // The play counts come off the pool rows that are already here rather than
    // from a second query, and only when the phase is one that asks: reading
    // five thousand counts to answer a question nobody put is the cheap-looking
    // mistake.
    let asks = crate::asks::asks(state.night().read().map(|read| read.phase));
    let plays: std::collections::HashMap<dj_core::TrackId, i64> =
        if matches!(asks.prefer, crate::asks::Prefer::Anchors) {
            pool.iter().map(|t| (t.id, t.stats.play_count)).collect()
        } else {
            std::collections::HashMap::new()
        };
    let ranked = with_phase(ranked, asks, &|id| plays.get(&id).copied().unwrap_or(0));

    // §12's other half: tonight's profile, when the DJ has named the night.
    // Read once for the whole rail — it is two queries and a fold, and five
    // thousand candidates would be ten thousand queries.
    let profile = tonight_profile(&state, &db);
    let ranked = with_profile(
        ranked.into_iter().map(|s| (s, None)).collect(),
        profile.as_ref(),
        &|id| genres.get(&id).cloned(),
    );

    // §22's estimated transition type. Read once for the whole rail rather
    // than per candidate — it is the *outgoing* half, which every row shares —
    // and the plan itself is arithmetic over two records, so a dozen of them
    // costs less than the query that found the candidates.
    let outgoing = playing_now
        .and_then(|id| db.track(id).ok().flatten())
        .and_then(|track| outgoing_of(&state, deck_id, &track));

    let budget = snapshot_now(&state).attention.suggestions;
    let held = state.fatigue();
    let want = held
        .lock()
        .map_or(limit, |fatigue| rail_size(limit, budget, &fatigue));
    let rail: Vec<_> = ranked.into_iter().take(want).collect();

    // What was put in front of the DJ, so that a record landing later can be
    // told apart from one they found themselves. Recorded before the rows are
    // built, because what matters is the set of records offered rather than
    // whether every one of them had a library row to draw.
    if let Ok(mut fatigue) = held.lock() {
        fatigue.offering(&rail.iter().map(|(s, _)| s.track).collect::<Vec<_>>());
    }

    // §81's learned transition style, off the profile already read above rather
    // than asked for again: `usual_style` is a query and a fold, and this rail
    // draws an estimate per row.
    let usual = profile.as_ref().and_then(crate::profile::Profile::style);

    Ok(rail
        .into_iter()
        .filter_map(|(s, because)| {
            let track = pool.iter().find(|t| t.id == s.track)?;
            Some(SuggestionDto {
                transition: outgoing
                    .as_ref()
                    .and_then(|out| estimate_transition(out, track, usual)),
                track: LibraryTrackDto::from(track.clone()),
                score: s.score,
                // The profile's reason goes with the scorer's rather than
                // beside them: it moved the same number, so it belongs in the
                // same list, and the rail already shows that list on hover.
                reasons: s
                    .reasons
                    .iter()
                    .map(describe_reason)
                    .chain(because)
                    .collect(),
                summary: summarise_reasons(&s.reasons),
                confidence: s.confidence(),
            })
        })
        .collect())
}

/// §12: the profile the rail is ranking by tonight, if any.
///
/// **Said out loud, because it changes the answer.** A ranking quietly
/// conditioned on what a DJ usually plays at weddings is a ranking they
/// cannot argue with — they would have to notice the order was different from
/// what the deltas imply and work out why. So the rail is handed the profile
/// itself: the setting, the nights behind it, and the sentence djmanzo writes
/// about it.
///
/// `None` until the DJ has named the night and there are enough nights of it.
///
/// **And `None` when the profile cannot move the ranking**, which is the case
/// the running application turned up: a profile whose nights have no genred
/// plays behind them is a real profile — it can still say how this DJ mixes
/// at weddings — and it tilts nothing, because the tilt is entirely a genre
/// leaning. The rail asks "what is the ranking conditioned on", and a profile
/// that conditions nothing is not an answer to that question; a line saying
/// "ranked for tonight" over an untouched ranking is a claim djmanzo cannot
/// support. §81's own panel still shows the profile, which is where a profile
/// that says nothing about genres belongs.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn profile_tonight(state: State<'_, AppState>) -> Result<Option<ProfileDto>, String> {
    let db = library(&state)?;
    Ok(tonight_profile(&state, &db)
        .filter(|p| !p.genres().is_empty())
        .map(|p| ProfileDto {
            setting: p.setting().slug().to_owned(),
            title: p.setting().title().to_owned(),
            nights: p.nights(),
            density: p.density().map(ToOwned::to_owned),
            style: p.style().map(|s| s.as_str().to_owned()),
            automation: p.automation().map(|a| a.name().to_owned()),
            techniques: p.techniques().iter().map(|d| d.slug().to_owned()).collect(),
            genres: p.genres().to_vec(),
            says: p.words(),
        }))
}

/// One thing tonight's profile would change, as the interface draws it.
#[derive(Debug, Clone, Serialize)]
pub struct FitDto {
    /// What it would be changed to, in the spelling the interface applies:
    /// a density band's slug, or a posture's name.
    pub to: String,
    /// The same thing as a DJ says it, for the sentence on the button.
    pub name: String,
    /// `its-own` or `if-asked`. See `crate::profile::Doing`.
    pub doing: String,
    /// The evidence, written in Rust.
    pub because: String,
}

/// What tonight's profile would fit, and what it is deliberately withholding.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FitsDto {
    pub density: Option<FitDto>,
    pub posture: Option<FitDto>,
    /// Where the profile had an answer and djmanzo is not offering it, and why.
    pub withheld: Vec<String>,
}

/// §81's other two: what this kind of night's profile would fit.
///
/// §81 lists five things a conditional profile may differ in — density,
/// technique preferences, genre weights, transition style, automation
/// tolerance — and until this only the genre weights reached anything, through
/// §12's rail. The density and the automation tolerance were learned, shown,
/// and acted on by nothing.
///
/// `crate::profile::fits` holds the rules and the reasons for them; this is the
/// layer with a database and a running assistant, and it does three things the
/// pure function cannot: it finds tonight's profile, it reads the posture the
/// assistant is actually on, and it resolves §79's locks off the stored
/// workspace.
///
/// **The density in force is the interface's own**, passed in rather than
/// looked up, for the same reason `night_setting` takes it: the band is chosen
/// from the window's height and that side is the only thing that knows it.
/// Absent when the shell has not settled on one yet, which reads as *not this
/// band* rather than as any particular band.
///
/// Empty rather than an error when there is no profile: most nights, for most
/// DJs, for a long time.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn night_fits(state: State<'_, AppState>, density: Option<String>) -> Result<FitsDto, String> {
    let db = library(&state)?;
    let Some(profile) = tonight_profile(&state, &db) else {
        return Ok(FitsDto::default());
    };
    let posture = state
        .conduct()
        .lock()
        .map_err(|_| "the conduct lock is poisoned".to_owned())?
        .posture;
    let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
    let permits = crate::cockpit::resolve(&stored).permits;
    let now = density.as_deref().and_then(crate::cockpit::Density::named);

    let fits = crate::profile::fits(&profile, now, posture, permits);
    Ok(FitsDto {
        density: fits.density.map(|fit| FitDto {
            // The slug serde writes, which is the spelling the shell matches a
            // band by — see `a_density_is_spelled_the_same_way_stored_as_it_is_spoken`.
            to: fit.to.name().to_lowercase().replace(' ', "-"),
            name: fit.to.name().to_owned(),
            doing: fit.doing.slug().to_owned(),
            because: fit.because,
        }),
        posture: fits.posture.map(|fit| FitDto {
            to: fit.to.name().to_owned(),
            name: fit.to.name().to_owned(),
            doing: fit.doing.slug().to_owned(),
            because: fit.because,
        }),
        withheld: fits.withheld,
    })
}

/// Tonight's profile, when the DJ has said what kind of night it is.
///
/// `None` until they have — §81's settings are **told, never inferred**, and
/// a rail that guessed the setting in order to rank by it would be ranking by
/// a guess. `None` too until there are enough nights of that setting for a
/// profile to exist at all, which `profile::profiles` decides.
pub(crate) fn tonight_profile(
    state: &AppState,
    db: &dj_library::Library,
) -> Option<crate::profile::Profile> {
    let setting = crate::setting::Setting::parse(&db.night(&state.session_id()).ok()??.setting)?;
    let nights = db.nights_in(setting.slug()).ok()?;
    let genres = |s: crate::setting::Setting| db.genres_in(s.slug()).unwrap_or_default();
    crate::profile::profiles(&nights, &genres, crate::profile::now())
        .into_iter()
        .find(|p| p.setting() == setting)
}

/// §81's learned transition style for the kind of night the DJ has named.
///
/// The one of §81's five that was *shown and consulted by nothing*: djmanzo
/// could tell a DJ "at weddings you mostly fade" and then propose a blend,
/// every time, for three years.
///
/// `None` in three different situations and they are all the same answer —
/// there is no library, the night has not been named, or djmanzo has not seen
/// enough nights of it to say. `profile::ENOUGH_NIGHTS` and the half-agree rule
/// own the last of those; this just asks.
///
/// One query and a fold per call, rather than a cached copy: the profile
/// changes when a night ends, this is asked once per planned transition, and a
/// second copy of an answer that has an owner is the thing this codebase keeps
/// having to take back out.
pub(crate) fn usual_style(state: &AppState) -> Option<dj_core::action::TransitionStyle> {
    let db = library(state).ok()?;
    tonight_profile(state, &db).and_then(|profile| profile.style())
}

/// Fold §81's profile into a ranking, and re-sort.
///
/// **The other half of §12.** Profiles were learned and read by nothing,
/// which made them a thing djmanzo could say about a DJ rather than a thing
/// it did for one. This is the doing, and it is deliberately the smallest
/// version of it: a bounded tilt on records whose genre this kind of night
/// actually contains, with the reason carried on the row.
///
/// Separate from the command and from the profile so all three can be tested:
/// the command needs a database, the profile owns the arithmetic, and this is
/// the part with the re-sort in it — which is the decision. A rail that lifted
/// a score without moving the row would show a higher number further down the
/// list, and that reads as a bug in the ranking rather than as the feature.
fn with_profile(
    ranked: Vec<(dj_library::suggest::Suggestion, Option<String>)>,
    profile: Option<&crate::profile::Profile>,
    genre_of: &dyn Fn(dj_core::TrackId) -> Option<String>,
) -> Vec<(dj_library::suggest::Suggestion, Option<String>)> {
    let Some(profile) = profile else {
        return ranked;
    };
    let mut out: Vec<_> = ranked
        .into_iter()
        .map(|(mut s, _)| {
            let genre = genre_of(s.track);
            s.score += profile.tilt_for(genre.as_deref());
            let because = profile.because(genre.as_deref());
            (s, because)
        })
        .collect();
    // The same tie-break the ranking used, so a re-sort cannot reorder two
    // candidates the profile said nothing about.
    out.sort_by(|a, b| {
        b.0.score
            .partial_cmp(&a.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.track.cmp(&b.0.track))
    });
    out
}

/// Fold §16's chosen knowledge pack into a ranking, and re-sort.
///
/// Separate from the command for the reason the other two folds are: the
/// command needs an `AppState` and a database, and this is the part with the
/// decision in it. The decision is the re-sort — a rail that lifted a score
/// without moving the row would show a higher number further down the list,
/// which reads as a bug in the ranking rather than as the feature.
///
/// `None` for the pack returns the ranking untouched, which is what *no pack
/// chosen* has to mean: djmanzo's own ranking, with nobody having said what
/// tonight is made of.
///
/// The family lookup is passed in rather than taken from the pool, because
/// `dj_core::genre::family_for` normalises and walks a table per call and the
/// caller already holds the genre tags — this way a pack that names no
/// families still costs one `contains` against an empty slice per row rather
/// than five thousand table walks.
fn with_pack(
    ranked: Vec<dj_library::suggest::Suggestion>,
    pack: Option<&dj_assistant::pack::Pack>,
    family_of: &dyn Fn(dj_core::TrackId) -> Option<&'static str>,
) -> Vec<dj_library::suggest::Suggestion> {
    let Some(pack) = pack else {
        return ranked;
    };
    let mut out: Vec<_> = ranked
        .into_iter()
        .map(|s| {
            // Empty `families` is *open format* and a real answer: no record is
            // in the pack's music because the pack has not named any, so none
            // is credited and the ranking is djmanzo's own. The half/double
            // rule still applies, because open format has one.
            let in_family = family_of(s.track).filter(|name| pack.families.contains(name));
            dj_library::suggest::also_in_pack(s, in_family, pack.half_time)
        })
        .collect();
    // The same tie-break `suggest::rank` uses, so a re-sort here cannot put
    // two equal candidates in a different order from the one that produced
    // them.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track.cmp(&b.track))
    });
    out
}

/// Fold §17's phase into a ranking, and re-sort.
///
/// The third of the three folds the rail applies over the scorer's answer, and
/// the last, because it is the weakest: taste and §81's profile are what this
/// DJ does, §16's pack is what they chose, §24's keeps are what they did with
/// these two records — and this is a reading djmanzo made about the night on
/// its own. When the four disagree the ordering above should win, and applying
/// this last is not what makes that true (the folds are additive and each
/// re-sorts on the same tie-break, so the order of application cannot change
/// the result) — the weight is. It is here because reading the file top to
/// bottom should say which is which.
///
/// `Prefer::Nothing` returns the ranking untouched, which is four of §17's six
/// phases: most of the night the direction is the whole of what the phase has
/// to say, and the direction is already in the ranking because it is what the
/// scorer was asked for.
fn with_phase(
    ranked: Vec<dj_library::suggest::Suggestion>,
    asks: crate::asks::Asks,
    played: &dyn Fn(dj_core::TrackId) -> i64,
) -> Vec<dj_library::suggest::Suggestion> {
    use dj_library::suggest::Reason;

    let Some(words) = asks.prefer.words() else {
        return ranked;
    };
    let mut out: Vec<_> = ranked
        .into_iter()
        .map(|s| {
            let asked = match asks.prefer {
                crate::asks::Prefer::Nothing => false,
                // §17's *harmonic resolution*, read off the scorer's own answer
                // rather than recomputed: a rail with its own opinion of
                // whether two keys agree would eventually disagree with the
                // chip sitting next to it on the same row.
                crate::asks::Prefer::Resolution => s
                    .reasons
                    .iter()
                    .any(|r| matches!(r, Reason::SameKey(_) | Reason::Harmonic { .. })),
                // §17's *known anchors*. Played at all, not played often: the
                // question is whether this room has heard this DJ play it, and
                // a threshold would be djmanzo deciding how many times counts.
                crate::asks::Prefer::Anchors => played(s.track) > 0,
            };
            dj_library::suggest::also_asked(s, asked.then_some(words))
        })
        .collect();
    // The same tie-break `suggest::rank` uses, so a re-sort here cannot put
    // two equal candidates in a different order from the one that produced
    // them.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track.cmp(&b.track))
    });
    out
}

/// Fold §24's kept pairs into a ranking, and re-sort.
///
/// Separate from the command so it can be tested: the command needs a live
/// `AppState` and a database, and this is the part with a decision in it.
///
/// Re-sorting is the decision. A rail that lifted a candidate's score without
/// moving it would show a higher number further down the list, which reads as
/// a bug in the ranking rather than as the feature it is.
fn with_kept(
    ranked: Vec<dj_library::suggest::Suggestion>,
    kept: &std::collections::HashMap<dj_core::TrackId, (u32, Option<f64>)>,
) -> Vec<dj_library::suggest::Suggestion> {
    let mut out: Vec<_> = ranked
        .into_iter()
        .map(|s| {
            let (times, on_loop) = kept.get(&s.track).copied().unwrap_or((0, None));
            dj_library::suggest::also_kept_before(s, times, on_loop)
        })
        .collect();
    // The same tie-break `suggest::rank` uses, so a re-sort here cannot put
    // two equal candidates in a different order from the one that produced
    // them.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.track.cmp(&b.track))
    });
    out
}

/// Records like a given one, tilted by what this DJ actually plays.
///
/// The difference from [`suggest_next`] is the seed and the tilt. That answers
/// "what next" from a deck; this answers "more like this" from any track in
/// the browser, which is the question a DJ asks when they have found something
/// that works and want three more of it.
///
/// Taste is added to the score, never multiplied, and is bounded well below
/// the gap between a key clash and a match — see [`dj_library::learned`]. It
/// reorders records that would all work; it cannot promote one that would not.
#[tauri::command]
pub fn similar_to(
    state: State<'_, AppState>,
    track: String,
    limit: usize,
    deck: Option<u8>,
) -> Result<Vec<SuggestionDto>, String> {
    use dj_library::suggest::{Playing, Trajectory};

    let db = library(&state)?;
    let seed_id = parse_track_id(&track)?;
    let seed = db
        .track(seed_id)
        .map_err(|e| e.to_string())?
        .ok_or("that track is not in the library")?;

    let now = Playing::of(&seed);

    let pool = db.all_tracks(5_000).map_err(|e| e.to_string())?;
    // Failing to learn is not failing to suggest: an empty taste tilts by
    // nothing, which is the same answer as a DJ with no history.
    let taste = db
        .learn_taste(crate::library::now_seconds())
        .unwrap_or_default();

    let mut ranked: Vec<_> = dj_library::suggest::rank(&now, Trajectory::Hold, &pool)
        .into_iter()
        // "More like this" that includes this is not a suggestion.
        .filter(|s| s.track != seed_id)
        .filter_map(|s| {
            let track = pool.iter().find(|t| t.id == s.track)?;
            Some((s, track))
        })
        .map(|(s, track)| {
            let tilted = s.score + taste.tilt_for(track);
            (tilted, s, track)
        })
        .collect();

    let outgoing = deck.and_then(dj_core::DeckId::from_human).and_then(|id| {
        let playing = current_track(&state, id)?;
        let track = db.track(playing).ok().flatten()?;
        outgoing_of(&state, id, &track)
    });

    // Re-sorted, because the tilt has moved things. Ties break on id so the
    // same library gives the same answer every time -- a "more like this" that
    // shuffled on each press would be impossible to trust.
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.track.cmp(&b.1.track))
    });

    // §81's learned style, read once for the whole answer rather than per row.
    let usual = usual_style(&state);

    Ok(ranked
        .into_iter()
        .take(limit.clamp(1, 100))
        .map(|(score, s, track)| SuggestionDto {
            // §22's estimate, and it is about the *deck* rather than the seed.
            // "Like this record" is a different question from "what happens if
            // it comes in here", and the rail draws both at once — so the
            // ranking answers the first and this answers the second, from
            // whatever is actually playing. `None` when the caller named no
            // deck, which is honest: without one there is nothing to mix out
            // of and a line here would be about nothing.
            transition: outgoing
                .as_ref()
                .and_then(|out| estimate_transition(out, track, usual)),
            track: LibraryTrackDto::from(track.clone()),
            score,
            reasons: s.reasons.iter().map(describe_reason).collect(),
            summary: summarise_reasons(&s.reasons),
            // The score here is "like the seed", not "follows what is playing",
            // so the ranking's own confidence is the one that means something.
            confidence: s.confidence(),
        })
        .collect())
}

/// What the history says this DJ reaches for.
#[derive(Debug, Clone, Serialize)]
pub struct TasteDto {
    /// Families played more often than owning them would predict, strongest
    /// first. Empty until there is enough history to mean anything.
    pub favourites: Vec<String>,
    /// How many plays this was drawn from.
    pub plays: usize,
    /// Whether there is enough history to act on.
    pub confident: bool,
}

/// What djmanzo has worked out about this DJ's taste.
///
/// Surfaced rather than kept hidden, because it steers suggestions and a DJ
/// should be able to see — and disagree with — what it thinks of them.
#[tauri::command]
pub fn learned_taste(state: State<'_, AppState>) -> Result<TasteDto, String> {
    let learned = library(&state)?
        .learn_taste(crate::library::now_seconds())
        .map_err(|e| e.to_string())?;
    Ok(TasteDto {
        favourites: learned.favourites(6),
        plays: learned.plays,
        confident: learned.is_confident(),
    })
}

/// What the interface should be wearing. §31.
#[derive(Debug, Clone, Serialize)]
pub struct MoodDto {
    /// The theme package's id, as `ui/src/controls/themes/packages.ts` spells
    /// it.
    pub theme: String,
    /// How long the change should take, in milliseconds. Zero when nothing is
    /// changing — which is most ticks, and is the point of §31.
    pub over_ms: u64,
    /// True when the DJ has pinned it and djmanzo has stopped deciding.
    pub locked: bool,
}

/// §31: offer the theme a reading of the night, and hear what to wear.
///
/// **The answer is almost always "the same thing".** §31's warning — "never
/// allow the interface to flicker from color to color every time the track
/// changes" — is the feature, and it lives in `dj_app::mood`: a four-minute
/// minimum, a forty-second settling time, and a lock that beats both. Calling
/// this on a tick is safe and expected; it changes its mind a handful of times
/// a night.
///
/// The reading is **the setting the DJ named** (§81) and the phase djmanzo has
/// read from the music. Venue ambience is told rather than sensed because
/// there is no light sensor here — see `crate::mood`.
///
/// # Errors
/// Whatever the database says, when tonight's setting is read back.
#[tauri::command]
pub fn theme_now(state: State<'_, AppState>) -> Result<MoodDto, String> {
    // The setting the DJ named. Without one there is nothing to adapt *to*,
    // and open format is the honest default: whatever the room turns out to
    // want. It is also what §81 writes when nobody has said.
    let setting = library(&state)
        .ok()
        .and_then(|db| db.night(&state.session_id()).ok().flatten())
        .and_then(|night| crate::setting::Setting::parse(&night.setting))
        .unwrap_or(crate::setting::Setting::OpenFormat);

    // The phase from the music. Before djmanzo can read one, a warm-up is the
    // right assumption: a set that has just started *is* warming up, and it is
    // also the most subdued answer, which is the safe direction to be wrong in.
    let phase = state
        .night()
        .read()
        .map_or(dj_core::SessionPhase::WarmUp, |read| read.phase);

    let at = state.night().elapsed();
    let want = crate::mood::wanted(setting, phase);
    let mood = state
        .weather()
        .lock()
        .map_err(|_| "the theme weather is poisoned".to_owned())?
        .consider(want, at);

    Ok(MoodDto {
        theme: mood.theme.to_owned(),
        over_ms: u64::try_from(mood.over.as_millis()).unwrap_or(u64::MAX),
        locked: mood.locked,
    })
}

/// Pin the theme, or let djmanzo decide again. §31's manual lock.
///
/// Locking does not change what is worn — it stops it changing. A lock that
/// also snapped the theme somewhere would be a second decision hiding inside a
/// refusal to decide.
///
/// # Errors
/// When the lock is poisoned.
#[tauri::command]
pub fn theme_lock(state: State<'_, AppState>, locked: bool) -> Result<(), String> {
    state
        .weather()
        .lock()
        .map_err(|_| "the theme weather is poisoned".to_owned())?
        .lock(locked);
    Ok(())
}

/// The DJ chose a theme. It takes effect now, and restarts the minimum.
///
/// **An id nothing ships is refused rather than worn.** `applyPackagePalette`
/// falls back to the organic palette for an id it does not have, so a typo used
/// to be worn silently as a different theme's colours — nothing throws, and the
/// picker simply appears not to work. §32's table is what makes refusing
/// possible: before it there was no list in Rust to check against.
///
/// The answer is the theme now worn, so the interface shows what was kept
/// rather than what was asked for.
///
/// # Errors
/// When the lock is poisoned, or the id is not one djmanzo ships.
#[tauri::command]
pub fn theme_chosen(state: State<'_, AppState>, theme: String) -> Result<String, String> {
    let known = crate::theme::for_pack(&theme)
        .ok_or_else(|| format!("djmanzo does not ship a theme called `{theme}`"))?;
    // The table's own `&'static str` rather than the argument. It was leaked
    // here before, once per manual choice, because `Weather` holds a static —
    // and there is no need now that the id has to be one of a fixed list.
    let worn = known.pack.unwrap_or_default();
    let at = state.night().elapsed();
    state
        .weather()
        .lock()
        .map_err(|_| "the theme weather is poisoned".to_owned())?
        .choose(worn, at);
    Ok(worn.to_owned())
}

/// One of §32's themes, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ThemeDto {
    /// What §32 calls it.
    pub title: String,
    /// What kind of room or evening it is for.
    pub about: String,
    /// The package id, or empty for a theme that does not ship.
    pub pack: String,
    /// Why it does not ship. Empty for the ones that do.
    pub why_not: String,
    /// Whether §32 named it, or djmanzo ships it anyway.
    pub asked: bool,
    /// Whether choosing it opens the watershed.
    pub world: bool,
}

/// §32's themes, including the ones djmanzo does not have.
///
/// The rows with no package are on the list saying why, the same posture §8's
/// `remembered` takes: a list of the six that ship would read as the whole of
/// §32, and a theme that is simply absent looks exactly like one nobody asked
/// for. It is also what closed §8's own waveform row — the gap became obvious
/// once it was somewhere a person could see it.
#[tauri::command]
#[must_use]
pub fn themes() -> Vec<ThemeDto> {
    crate::theme::ALL
        .iter()
        .map(|theme| ThemeDto {
            title: theme.title.to_owned(),
            about: theme.about.to_owned(),
            pack: theme.pack.unwrap_or_default().to_owned(),
            why_not: theme.why_not.to_owned(),
            asked: theme.asked,
            world: theme.world,
        })
        .collect()
}

/// One control's gestures, for the interface. §29.
#[derive(Debug, Clone, Serialize)]
pub struct HandleDto {
    /// The verb, so the interface can ask for the control it is drawing.
    pub control: String,
    /// What a double-click sends.
    pub reset: String,
    /// How much finer a shift-drag is than a drag.
    pub fine: f64,
    /// §29's level three: `[label, action]` per entry.
    pub options: Vec<(String, String)>,
}

/// §29: what a control's gestures do.
///
/// Asked rather than written on the interface side, for the reason every call
/// site demonstrated: each `SvgKnob` passed its own `ondblclick` naming its own
/// idea of where the control resets to, and nothing made a fourth one agree.
/// A control's unity point is a fact about the parameter.
///
/// Every answer is action text — exactly what `Action::parse` takes — so a
/// drag, a double-click, a menu entry and a MIDI CC end up as the same action.
/// That is §29's last bullet ("MIDI = same underlying parameter") and ADR-0003.
///
/// # Errors
/// A deck djmanzo does not have.
#[tauri::command]
pub fn control_handles(deck: u8) -> Result<Vec<HandleDto>, String> {
    let deck = dj_core::DeckId::from_human(deck).ok_or("no such deck")?;
    Ok(crate::handle::Control::ALL
        .into_iter()
        .map(|control| {
            let h = crate::handle::handle(deck, control);
            HandleDto {
                control: control.verb().to_owned(),
                reset: h.reset,
                fine: h.fine,
                options: h.options.into_iter().map(|o| (o.label, o.action)).collect(),
            }
        })
        .collect())
}

/// What the assistant's plan would do to one control. §29's AI hover.
#[derive(Debug, Clone, Serialize)]
pub struct SuggestedDto {
    /// The verb, so the interface can match it to the knob it is drawing.
    pub control: String,
    /// The value the plan takes it to at its furthest, or null for a gesture
    /// with no position — sync is a switch.
    pub to: Option<f64>,
    /// The action that gets there, exactly as the parser takes it.
    pub action: String,
    /// What the plan does, and why, in Rust's words.
    pub because: String,
}

/// §29's *AI hover = suggestion*, for one deck.
///
/// The last of §29's seven gestures. Its row said there was nothing for a
/// hover to read, because the assistant stages whole moves rather than single
/// parameter values — and that stopped being true when §68's transition object
/// gained a style and `crate::shape` became the one table saying what a style
/// does beyond the two channel faders, in the values the automix sends.
///
/// **It answers about a mix djmanzo has actually planned**, never in general:
/// the held transition, and only while it still describes what is on the
/// decks. A general opinion about where an EQ band should be is not a thing
/// any software has, and a hover that offered one would be the most confident
/// invention in the interface.
///
/// Empty for a deck the plan does not name, and empty whenever nothing is
/// armed — which is most of the time, and is why this is a separate question
/// from `control_handles`: the gestures are a fixed table fetched once, and
/// this changes with the mix.
///
/// # Errors
/// A deck djmanzo does not have.
#[tauri::command]
pub fn control_suggestions(
    state: State<'_, AppState>,
    deck: u8,
) -> Result<Vec<SuggestedDto>, String> {
    let asked = dj_core::DeckId::from_human(deck).ok_or("no such deck")?;
    let Some(transition) = state.transition() else {
        return Ok(Vec::new());
    };
    // The same staleness rule `transition_current` states, for the same
    // reason: a confident suggestion about a record that left the deck four
    // minutes ago looks exactly like a current one.
    let loaded = |deck| current_track(&state, deck);
    if !transition.describes(
        loaded(transition.outgoing_deck),
        loaded(transition.incoming_deck),
    ) {
        return Ok(Vec::new());
    }

    let side = if asked == transition.outgoing_deck {
        crate::handle::Side::Leaving
    } else if asked == transition.incoming_deck {
        crate::handle::Side::Arriving
    } else {
        return Ok(Vec::new());
    };

    Ok(crate::handle::suggested(
        asked,
        side,
        transition.plan.style,
        transition.plan.length_beats,
    )
    .into_iter()
    .map(|found| SuggestedDto {
        control: found.control.verb().to_owned(),
        to: found.to,
        action: found.action,
        because: found.because,
    })
    .collect())
}

/// One record through §76's lens.
///
/// Every field may be absent, and an absence is drawn as one: a lens that
/// filled its blanks with zero would rank an unanalysed record below a merely
/// bad one, and look like a judgement while doing it.
#[derive(Debug, Clone, Serialize)]
pub struct LensRowDto {
    /// The track id, as hex, so the browser can join this to the row it is
    /// already drawing rather than being handed the record twice.
    pub track: String,
    pub likely_next: Option<f64>,
    pub affinity: Option<f64>,
    pub phase_fit: Option<f64>,
    /// Slug and words per risk, so the interface can style it and say it.
    pub risks: Vec<(String, String)>,
    pub novelty: f64,
    pub familiarity: f64,
    pub functions: Vec<String>,
}

/// §76's AI lens: djmanzo's opinion beside the records the browser is showing.
///
/// **It adds; it never replaces.** The lens takes the ids of rows the browser
/// already has and answers about those — it does not query, filter or order
/// the library, so turning it off leaves the standard view exactly as it was,
/// because the lens was never inside it. That is §76's closing line, expressed
/// as the shape of the command rather than as a promise.
///
/// `deck` is what the lens is *relative to*: "likely next" and "transition
/// risk" are about following the record playing there. A deck with nothing on
/// it leaves both empty rather than ranking the collection against silence.
///
/// # Errors
/// Whatever the database says. An id the library does not hold is skipped, not
/// fatal: a browser row can be a moment stale.
#[tauri::command]
pub fn library_lens(
    state: State<'_, AppState>,
    tracks: Vec<String>,
    deck: u8,
) -> Result<Vec<LensRowDto>, String> {
    let db = library(&state)?;

    // What is playing, from the library row rather than the snapshot, for the
    // reason `suggest_next` gives: the candidates are scored against the same
    // numbers.
    let playing_id = dj_core::DeckId::from_human(deck).and_then(|id| current_track(&state, id));
    let playing = playing_id
        .and_then(|id| db.track(id).ok().flatten())
        .map(|t| dj_library::suggest::Playing::of(&t));

    // Read once for the whole page, not once per row: both of these are a
    // query, and fifty rows would be a hundred of them.
    let taste = db.learn_taste(crate::library::now_seconds()).ok();
    let phase = state.night().read().map(|read| read.phase);

    let now = crate::lens::Now {
        playing: playing.as_ref(),
        playing_id,
        trajectory: dj_core::Trajectory::Hold,
        phase,
        now: crate::library::now_seconds(),
    };

    let mut out = Vec::with_capacity(tracks.len());
    for hex in &tracks {
        let Some(id) = dj_core::TrackId::from_hex(hex) else {
            continue;
        };
        let Some(track) = db.track(id).ok().flatten() else {
            continue;
        };
        let functions = db.functions_for(id).unwrap_or_default();
        let seen = crate::lens::look(&track, &functions, taste.as_ref(), now);
        out.push(LensRowDto {
            track: hex.clone(),
            likely_next: seen.likely_next,
            affinity: seen.affinity,
            phase_fit: seen.phase_fit,
            risks: seen
                .risks
                .iter()
                .map(|r| (r.slug().to_owned(), r.words().to_owned()))
                .collect(),
            novelty: seen.novelty,
            familiarity: seen.familiarity,
            functions: seen.functions.iter().map(|f| f.slug().to_owned()).collect(),
        });
    }
    Ok(out)
}

/// One of §27's seven questions, and whether djmanzo can answer it.
#[derive(Debug, Clone, Serialize)]
pub struct GhostAskedDto {
    pub slug: String,
    /// §27's own words, so the panel cannot quietly reword what was asked.
    pub about: String,
    pub answered: bool,
}

/// Where the candidate's first full phrase would land, on the outgoing record.
#[derive(Debug, Clone, Serialize)]
pub struct GhostLandingDto {
    /// Frames on the outgoing record — the lane on screen.
    pub frame: f64,
    /// Beats of the candidate before that phrase. Zero is no pickup.
    pub lead_beats: f64,
    /// False when the phrase arrives after the mix has already finished.
    pub within_mix: bool,
}

/// §27's ghost: what happens if this record comes in here.
///
/// Frames throughout, for the reason [`TransitionDto`] gives: the waveform is
/// drawn in frames, and converting seconds back through a sample rate the
/// interface would have to infer is two roundings and a division by zero
/// waiting for an empty deck.
#[derive(Debug, Clone, Serialize)]
pub struct GhostDto {
    /// The candidate, as hex, so the caller can join this to the row it is
    /// already drawing.
    pub track: String,
    /// The deck the ghost is drawn over.
    pub deck: u8,
    /// The stretch the two records would share.
    pub start_frame: f64,
    pub end_frame: f64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub length_beats: u32,
    pub style: String,
    /// Incoming tempo minus outgoing, signed.
    pub bpm_delta: f64,
    /// What the pitch fader on the incoming deck does, as a percentage.
    pub pitch_percent: f64,
    pub key_relation: Option<String>,
    pub landing: Option<GhostLandingDto>,
    /// Where the outgoing record becomes weak, in frames. The same window the
    /// lane already draws, not a second opinion about it.
    pub weakens_from: Option<f64>,
    pub weakens_to: Option<f64>,
    /// §27's *drop*: where the candidate's first one would land, in frames on
    /// the outgoing record.
    ///
    /// `None` for a candidate djmanzo has not analysed, one with no grid, and
    /// one that never thins out -- all three are records with no drop to
    /// promise, and the ghost says nothing for any of them rather than
    /// drawing a mark somewhere plausible.
    pub drop_frame: Option<f64>,
    /// §27's *where the vocal enters*: where the candidate's voice would
    /// arrive, in frames on the outgoing record.
    ///
    /// `None` for an instrumental, for a candidate nobody has analysed, and
    /// for one whose voice is already in before the mix point -- three records
    /// with no entry to promise, and the ghost draws nothing for all three.
    pub vocal_entry_frame: Option<f64>,
    pub reasons: Vec<String>,
    /// The mix in one phrase — `32-beat blend at 2:09`. Worded here rather
    /// than in the panel, because §22's rail draws the same phrase and two
    /// spellings of one mix is two answers.
    pub says: String,
    /// §27's seven, in its order, each saying whether it is answered.
    pub asked: Vec<GhostAskedDto>,
}

/// §27: draw the future of `deck` meeting `track`, without loading anything.
///
/// **Nothing moves.** No deck is touched, no transition is armed, nothing is
/// written down — which is the whole of §27's "non-destructive", and the
/// reason a DJ can ask this of eight candidates in a row while a record plays.
///
/// The geometry is the planner's, so what is drawn here is the mix djmanzo
/// would actually perform if the record were loaded. A ghost that worked one
/// out for itself would be showing a DJ a transition and then doing another.
///
/// `None` when there is nothing honest to draw: an empty deck, an unanalysed
/// record on either side, or a track already too near its end for any
/// transition the planner proposes to fit.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn ghost_preview(
    state: State<'_, AppState>,
    deck: u8,
    track: String,
) -> Result<Option<GhostDto>, String> {
    let Some(deck_id) = dj_core::DeckId::from_human(deck) else {
        return Ok(None);
    };
    let Some(id) = dj_core::TrackId::from_hex(&track) else {
        return Ok(None);
    };
    let db = library(&state)?;
    let Some(out_track) = current_track(&state, deck_id).and_then(|id| db.track(id).ok().flatten())
    else {
        return Ok(None);
    };
    let Some(candidate_track) = db.track(id).ok().flatten() else {
        return Ok(None);
    };
    let Some(outgoing) = outgoing_of(&state, deck_id, &out_track) else {
        return Ok(None);
    };
    // The candidate's own grid, or nothing. Falling back to the outgoing
    // track's tempo -- which `transition_between` does for a *loaded* record,
    // where the deck has one either way -- would draw a confident ghost of a
    // record nobody has analysed.
    let Some(grid) = candidate_track.analysis.beatgrid() else {
        return Ok(None);
    };
    // Read once: the drop and the vocal entry are two positions on one
    // trajectory, and asking the cache twice for one record is two lookups
    // that could answer differently across a re-analysis landing between them.
    let cached = state.analysis().cached(&candidate_track.id);
    let candidate = crate::ghost::Candidate {
        bpm: grid.bpm.get(),
        phrase: phrase_of(&candidate_track),
        key: candidate_track.analysis.key(),
        sample_rate: candidate_track.sample_rate,
        grid_anchor: grid.anchor.get(),
        // §27's *drop*, from the analysis cache rather than from the library
        // row: the row stores §20's single energy number and the trajectory is
        // a curve. A candidate djmanzo has never analysed has none, and the
        // ghost then says nothing about a drop -- which is the honest answer
        // and the one §27's own list is written to allow.
        drops: cached
            .as_ref()
            .map(|found| found.trajectory.drops.clone())
            .unwrap_or_default(),
        // §27's *where the vocal enters*, from the same cached trajectory and
        // for the same reason: the library row stores one energy number and
        // this is a position on a curve.
        voice_enters: cached
            .as_ref()
            .and_then(|found| found.trajectory.voice_enters),
    };

    let Some(ghost) = crate::ghost::look(&outgoing, &candidate, usual_style(&state)) else {
        return Ok(None);
    };
    let rate = outgoing.sample_rate.as_f64();
    Ok(Some(GhostDto {
        track,
        deck,
        start_frame: ghost.plan.start_frame,
        end_frame: ghost.plan.end_frame,
        start_seconds: ghost.plan.start_frame / rate,
        end_seconds: ghost.plan.end_frame / rate,
        length_beats: ghost.plan.length_beats,
        style: ghost.plan.style.as_str().to_owned(),
        bpm_delta: ghost.plan.bpm_delta,
        pitch_percent: ghost.pitch_percent,
        key_relation: ghost.keys().map(|r| r.as_str().to_owned()),
        says: transition_words(
            ghost.plan.length_beats,
            ghost.plan.style.as_str(),
            ghost.plan.start_frame / rate,
        ),
        landing: ghost.landing.map(|l| GhostLandingDto {
            frame: l.frame,
            lead_beats: l.lead_beats,
            within_mix: l.within_mix,
        }),
        weakens_from: ghost.weakens.map(|w| w.opens_frame),
        weakens_to: ghost.weakens.map(|w| w.closes_frame),
        drop_frame: ghost.drop,
        vocal_entry_frame: ghost.vocal_entry,
        reasons: ghost
            .plan
            .reasons
            .iter()
            .map(describe_plan_reason)
            .collect(),
        asked: ghost
            .asked
            .iter()
            .map(|(asked, answered)| GhostAskedDto {
                slug: asked.slug().to_owned(),
                about: asked.about().to_owned(),
                answered: *answered,
            })
            .collect(),
    }))
}

/// §37: what the room has usually done after one kind of mix.
#[derive(Debug, Clone, Serialize)]
pub struct SeenDto {
    /// A `dj_app::setting::Setting` slug — what "here" means in §37.
    pub setting: String,
    /// A transition style's name.
    pub style: String,
    /// `light`, `movement` or `loudness`.
    pub sense: String,
    /// How many nights this is drawn from.
    pub nights: usize,
    /// `rose`, `fell`, or `null` when the nights do not agree — which is the
    /// common answer and the important one.
    pub usually: Option<String>,
    /// The sentence, worded in Rust, or `null` when there is nothing to say.
    pub says: Option<String>,
}

/// §37: what has happened after this kind of mix on previous nights.
///
/// **The one thing djmanzo writes down about a room.** Everything else it
/// says is derived from the action log; a comparison across nights cannot be,
/// because the readings live twenty minutes and the log does not outlive the
/// run. See `dj_app::response`.
///
/// Never a causal claim, whatever §37 is called. A floor that fills twelve
/// seconds after a mix may be filling because of it or in spite of it, and
/// nothing here can tell those apart — so the sentence says what happened
/// after, over enough nights that coincidence is the worse explanation, and
/// never says *because*.
///
/// Empty is the ordinary answer: djmanzo runs without a camera, and without
/// one nothing is ever recorded.
///
/// # Errors
/// Whatever the database says.
#[tauri::command]
pub fn room_history(state: State<'_, AppState>) -> Result<Vec<SeenDto>, String> {
    let db = library(&state)?;
    let stored = db.responses().map_err(|e| e.to_string())?;

    let nights: Vec<crate::response::Night<'_>> = stored
        .iter()
        .filter_map(|row| {
            Some(crate::response::Night {
                session_id: &row.session_id,
                setting: &row.setting,
                style: &row.style,
                sense: [
                    dj_assistant::room::Sense::Movement,
                    dj_assistant::room::Sense::Loudness,
                    dj_assistant::room::Sense::Light,
                ]
                .into_iter()
                .find(|sense| sense.name() == row.sense)?,
                lift: crate::response::Lift::between(row.before, row.after),
            })
        })
        .collect();

    Ok(crate::response::seen(&nights)
        .into_iter()
        .map(|seen| SeenDto {
            setting: seen.setting().to_owned(),
            style: seen.style().to_owned(),
            sense: seen.sense().name().to_owned(),
            nights: seen.nights(),
            usually: seen.usually().map(|lift| lift.name().to_owned()),
            says: seen.words(),
        })
        .collect())
}

/// Which track is on a deck, if any.
pub(crate) fn current_track(state: &AppState, deck: dj_core::DeckId) -> Option<dj_core::TrackId> {
    let tracks = state.deck_tracks();
    let map = tracks.lock().ok()?;
    map.get(&deck.human_number()).map(|t| t.id)
}

/// A loop length, said the way the deck says it.
///
/// `Deck.svelte` renders an active loop as `8` or `1/2`; this is the same
/// spelling with the unit attached, so the reason beside a suggestion and the
/// readout on the deck agree about what an eight-beat loop is called. A DJ who
/// reads "on an 8-beat loop" and then looks at the deck should see `8`.
fn beat_count(beats: f64) -> String {
    if beats >= 1.0 {
        let rounded = (beats * 100.0).round() / 100.0;
        // `8-beat`, not `8.00-beat`: every loop a deck button sets is whole,
        // and the fractional form is for the ones a DJ halved into place.
        if (rounded - rounded.round()).abs() < f64::EPSILON {
            format!("{}-beat", rounded.round())
        } else {
            format!("{rounded}-beat")
        }
    } else if beats > 0.0 {
        format!("1/{}-beat", (1.0 / beats).round())
    } else {
        // Not reachable from the engine, which refuses a loop of no length,
        // but a database column is a database column and a row saying zero
        // should read as nonsense rather than divide.
        "0-beat".to_owned()
    }
}

/// `a` or `an`, for a loop length that is about to be spoken.
///
/// Worth the three lines: §24's own words are "works only with an 8-beat
/// loop", and "a 8-beat loop" in the place that quotes it would read as a
/// program that does not speak the language its user does.
const fn article_for(count: &str) -> &'static str {
    match count.as_bytes() {
        [b'8', ..] | [b'1', b'1', ..] | [b'1', b'8', ..] => "an",
        _ => "a",
    }
}

/// Render one reason for the interface.
///
/// Deliberately terse: these are chips beside a table row, not prose. A DJ
/// scanning the list wants "same key" and "+3 dB", not a sentence.
fn describe_reason(reason: &dj_library::suggest::Reason) -> String {
    use dj_library::suggest::Reason;
    match reason {
        Reason::SameKey(k) => format!("same key ({})", k.camelot()),
        Reason::Harmonic { to, .. } => format!("harmonic ({})", to.camelot()),
        Reason::KeyClash { to, .. } => format!("key clash ({})", to.camelot()),
        Reason::TempoFits { to, .. } => format!("{to:.0} BPM fits"),
        Reason::TempoHalfOrDouble { to, .. } => format!("{to:.0} BPM at half/double"),
        Reason::TempoFar { to, .. } => format!("{to:.0} BPM is a stretch"),
        Reason::Loudness { delta_db } => {
            if delta_db.abs() < 0.5 {
                "same level".to_owned()
            } else {
                format!("{delta_db:+.0} dB")
            }
        }
        Reason::PhraseKnown { beats } => format!("{beats}-beat phrases"),
        Reason::PhraseUnknown => "no phrase structure".to_owned(),
        Reason::SameFamily(name) => format!("same family ({name})"),
        Reason::OtherFamily { from, to } => format!("{from} to {to}"),
        // §16, in the DJ's own terms: they chose the pack, so the chip says
        // *yours* rather than naming it. Which pack is on screen already, in
        // the picker they chose it in; what is worth saying beside the row is
        // that this record is the music they said tonight was made of.
        Reason::InPack(family) => format!("your pack ({family})"),
        // §17, in the one place a phase reading is worth reading: beside the
        // record it moved. "The night" rather than the phase's name, because
        // the name is already in the mission bar and what is new here is that
        // it changed the answer.
        Reason::PhaseAsks(words) => format!("the night asks for {words}"),
        Reason::HalfTimeUnusual => "half/double is unusual here".to_owned(),
        // §24's answer to "why do I keep seeing these two together?", in the
        // place a DJ asks it: beside the suggestion itself.
        Reason::KeptBefore { times, on_loop } => {
            let kept = if *times == 1 {
                "you kept this mix".to_owned()
            } else {
                format!("you kept this mix {times} times")
            };
            // §24's example in full: "A into C works only with an 8-beat
            // loop". The count says the DJ meant it; the loop says how to do
            // it again, which is the half that is actually actionable when the
            // record is eight bars from the end.
            match on_loop {
                Some(beats) => {
                    let count = beat_count(*beats);
                    format!("{kept}, on {} {count} loop", article_for(&count))
                }
                None => kept,
            }
        }
        Reason::Unanalysed => "not analysed yet".to_owned(),
    }
}

/// The reasons as one line of deltas, strongest first.
///
/// # Why deltas and not the values
///
/// `128 BPM fits` requires remembering what is playing before it means
/// anything; `+3 BPM` means it on sight. The directive's own example is
/// `+3 BPM · 8A→9A · energy +1`, and that is the shape: what changes, not what
/// is.
///
/// # What is left out, and why
///
/// A phrase structure that *was* found is not mentioned. Phrase lengths in
/// practice are 8, 16 and 32, each dividing the next, so two records that both
/// have one will align -- saying so on every row of every rail would be eight
/// repetitions of "nothing to worry about". The absence is worth a word, so
/// `no phrase` is said.
///
/// Loudness is reported in decibels rather than as an energy number. The
/// analyser measures integrated LUFS; a 1-to-10 energy scale would be a
/// invented unit dressed as a measurement. See the module docs of
/// `dj_library::suggest`.
pub(crate) fn summarise_reasons(reasons: &[dj_library::suggest::Reason]) -> String {
    use dj_library::suggest::Reason;

    /// Where each reason sits on the line, regardless of the order the scorer
    /// happened to produce them in.
    ///
    /// Tempo first because it is the first gate -- a record the deck cannot
    /// reach is not a candidate whatever its key -- then the key, then the
    /// level, then where the genre goes, then whatever is missing. The scorer
    /// pushes key before tempo because that is the order it computes in, which
    /// is an implementation detail and not something a DJ should read.
    const fn place(reason: &Reason) -> u8 {
        match reason {
            // First of all, when it is there. "You kept this mix" is the
            // strongest thing djmanzo can say about a pair and it is the DJ's
            // own word — it should not read fourth, after a loudness delta.
            Reason::KeptBefore { .. } => 0,
            Reason::TempoFits { .. }
            | Reason::TempoHalfOrDouble { .. }
            | Reason::TempoFar { .. } => 1,
            // Immediately after the tempo it qualifies, and on a rank of its
            // own rather than sharing the tempo's: "half-time" and the pack's
            // opinion of half-time are one statement, and a sort that left
            // their order to whichever was pushed first would print the verdict
            // before the thing it is a verdict on whenever a caller assembled
            // the reasons in a different order.
            Reason::HalfTimeUnusual => 2,
            Reason::SameKey(_) | Reason::Harmonic { .. } | Reason::KeyClash { .. } => 3,
            Reason::Loudness { .. } => 4,
            Reason::SameFamily(_) | Reason::OtherFamily { .. } => 5,
            // After the family, for the same reason: the family is the fact and
            // the pack is what the DJ makes of it.
            Reason::InPack(_) => 6,
            // And last of the reasons that carry weight, because it is the
            // weakest of the four and the only one the DJ did not say.
            Reason::PhaseAsks(_) => 7,
            Reason::PhraseKnown { .. } | Reason::PhraseUnknown | Reason::Unanalysed => 8,
        }
    }

    let mut ordered: Vec<&Reason> = reasons.iter().collect();
    ordered.sort_by_key(|r| place(r));

    ordered
        .into_iter()
        .filter_map(|reason| match reason {
            // On the summary line too, and first: it is the one thing on it a
            // DJ said rather than djmanzo worked out.
            Reason::KeptBefore { times, on_loop } => {
                let kept = if *times == 1 {
                    "kept".to_owned()
                } else {
                    format!("kept \u{00d7}{times}")
                };
                Some(match on_loop {
                    // The chip is beside a table row rather than in a
                    // sentence, so the loop is said the way a deck says it.
                    Some(beats) => format!("{kept} \u{00b7} {} loop", beat_count(*beats)),
                    None => kept,
                })
            }
            Reason::SameKey(k) => Some(k.camelot()),
            Reason::Harmonic { from, to } | Reason::KeyClash { from, to } => {
                let arrow = format!("{}\u{2192}{}", from.camelot(), to.camelot());
                Some(if matches!(reason, Reason::KeyClash { .. }) {
                    format!("{arrow} clash")
                } else {
                    arrow
                })
            }
            Reason::TempoFits { from, to } => Some(bpm_delta(*from, *to, "")),
            Reason::TempoHalfOrDouble { from, to } => Some(
                if *to > *from {
                    "double-time"
                } else {
                    "half-time"
                }
                .to_owned(),
            ),
            Reason::TempoFar { from, to } => Some(bpm_delta(*from, *to, " stretch")),
            Reason::Loudness { delta_db } => Some(if delta_db.abs() < 0.5 {
                "level".to_owned()
            } else {
                format!("{delta_db:+.0} dB")
            }),
            // Present is the common case and nearly free; absent is the risk.
            Reason::PhraseKnown { .. } => None,
            Reason::PhraseUnknown => Some("no phrase".to_owned()),
            Reason::SameFamily(name) => Some((*name).to_owned()),
            Reason::OtherFamily { to, .. } => Some(format!("\u{2192}{to}")),
            // The family is already on this line, as `bachata` or `\u{2192}bachata`,
            // whenever both records name one. Repeating it here would be the
            // same word twice; what the pack adds is that it is *tonight's*.
            Reason::InPack(_) => Some("your pack".to_owned()),
            // The words themselves here: they are already short, they are
            // §17's, and "the night" without them would say that something
            // about the phase mattered without saying what.
            Reason::PhaseAsks(words) => Some((*words).to_owned()),
            Reason::HalfTimeUnusual => Some("unusual here".to_owned()),
            Reason::Unanalysed => Some("not analysed".to_owned()),
        })
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// `+3 BPM`, or `128 BPM` when there is nothing to compare against.
///
/// A zero delta still reads as `+0 BPM` rather than being dropped: "the same
/// tempo" is the strongest thing a tempo can say, and silence would look like
/// a missing value.
fn bpm_delta(from: f64, to: f64, suffix: &str) -> String {
    if !from.is_finite() || from <= 0.0 {
        return format!("{to:.0} BPM{suffix}");
    }
    // Rounded first, and a rounded zero normalised to a positive one.
    // `{:+.0}` of -0.4 is `-0`, which appeared on a real seam between two 120
    // BPM records and reads as a fault rather than as "no change".
    let delta = to - from;
    let rounded = if delta.abs() < 0.5 { 0.0 } else { delta };
    format!("{rounded:+.0} BPM{suffix}")
}

// -- the command palette ----------------------------------------------------

/// One thing the palette can do.
///
/// # Why this is assembled in Rust
///
/// §51 asks for a command surface on `Ctrl/Cmd + K`, and closes by saying it
/// "can also become the semantic interface exposed to voice/AI". That sentence
/// decides the design: the palette must not be a hand-written list of pretty
/// labels, because a hand-written list is a second vocabulary that drifts from
/// the real one -- exactly what `dj_core::vocabulary` exists to prevent for the
/// assistant (ADR-0005).
///
/// So every entry is generated from something that already exists: a verb the
/// parser accepts, or a surface `cockpit::surfaces()` publishes. The palette
/// cannot offer a command djmanzo does not have, and a verb added to the
/// vocabulary appears in it without anyone remembering to add it.
#[derive(Debug, Clone, Serialize)]
pub struct PaletteEntryDto {
    /// What the DJ reads: `Deck 1 · play`, `Show Prepare`.
    pub label: String,
    /// One line, in the imperative, from the vocabulary's own help.
    pub about: String,
    /// `action`, `surface` or `ui` -- how the interface should carry it out.
    pub kind: &'static str,
    /// The action text, the surface name, or the interface operation.
    pub run: String,
    /// §58's information hierarchy: `glanceable`, `performable`, `contextual`
    /// or `preparation`. What the list is ordered by.
    pub tier: &'static str,
}

/// How many entries one query may return.
///
/// A palette is read, not scrolled: past a dozen the list stops being a list
/// and becomes a search result, and the DJ is better served by typing another
/// letter. The cap is applied after ranking, so the twelve are the best twelve.
const PALETTE_LIMIT: usize = 12;

/// What the palette should offer for `query`, best first.
///
/// # Ranking, and why it is here rather than in the interface
///
/// **What you typed comes first, if it is a real action.** `deck 2 loop 8`
/// parses, so the top entry runs it verbatim. This is what turns the palette
/// into the semantic interface §51 asks for -- the whole vocabulary is reachable
/// by typing it, including every verb that takes an argument, which a list of
/// buttons could never offer without inventing numbers. It stays at the top
/// whatever it is: a DJ who typed a thing is not asking to be ranked.
///
/// **Everything else is ordered by [§58's information
/// hierarchy](crate::tiers)** -- glanceable, then performable, then contextual,
/// then preparation -- and the cut to `PALETTE_LIMIT` happens after. Before
/// this the order was the order the passes below happened to generate in, so a
/// DJ who typed three letters mid-mix could be offered *Pin the Journal* above
/// *deck 2 cue*, and on a six-deck layout the cut could take the performing
/// controls off the bottom entirely. §58 is a ranking, and a ranking nothing
/// reads is a paragraph.
///
/// The sort is **stable**, so within a tier the order stays the one the passes
/// gave it: the vocabulary is written grouped by what the verbs do, so an empty
/// query opens on transport rather than on whatever sorts first alphabetically.
///
/// Matching is a subsequence test rather than a substring one, because that is
/// what a palette user expects: `d2p` finds `Deck 2 · play`.
/// What the palette is offering, and why it is offering that much.
#[derive(Debug, Clone, Serialize)]
pub struct PaletteDto {
    pub entries: Vec<PaletteEntryDto>,
    /// Why the list is shorter than usual, or empty when it is not.
    ///
    /// Said rather than left to be noticed, for the reason §74's rail says
    /// which deck it is following: a list that silently halved is a list a DJ
    /// stops trusting, and the sentence also carries the way out of it.
    pub because: String,
}

/// §117: the tree behind the leader key, for this many decks and what the DJ
/// has. See [`crate::leader`].
#[tauri::command]
#[must_use]
pub fn leader_tree(state: State<'_, AppState>, decks: u8) -> crate::leader::Node {
    let (tree, switches) = leader_parts(&state, decks);
    crate::leader::with_mine(tree, &state.leader_mine(), &switches)
}

/// The tree djmanzo ships for this state, and the switches a key may name.
fn leader_parts(state: &AppState, decks: u8) -> (crate::leader::Node, Vec<String>) {
    let activities = crate::activity::all(&state.activities().mine);
    let mut workspaces = state.my_workspaces();
    workspaces.extend(crate::cockpit::workspaces());
    let packs = state.presets().packs();
    (
        crate::leader::tree(decks, &activities, &workspaces, packs),
        switch_runs(&activities, &workspaces, packs, decks),
    )
}

/// §117: the dashboard for what the DJ has and the activity they are in
/// (`current`, empty for none). See [`crate::dashboard`].
#[tauri::command]
#[must_use]
pub fn dashboard(state: State<'_, AppState>, current: String) -> crate::dashboard::Dashboard {
    let activities = crate::activity::all(&state.activities().mine);
    let mut workspaces = state.my_workspaces();
    workspaces.extend(crate::cockpit::workspaces());
    let current = activities.iter().find(|activity| activity.slug == current);
    crate::dashboard::build(
        &activities,
        current,
        &workspaces,
        state.presets().packs(),
        &state.interface(),
    )
}

/// §117: the interface's own settings.
#[tauri::command]
#[must_use]
pub fn interface_settings(state: State<'_, AppState>) -> crate::dashboard::Interface {
    state.interface()
}

/// §117: show the toolbars, or keep the space for the activity.
#[tauri::command]
#[must_use]
pub fn set_toolbars(state: State<'_, AppState>, on: bool) -> crate::dashboard::Interface {
    let mut interface = state.interface();
    interface.toolbars = on;
    state.set_interface(&interface);
    interface
}

/// §117: count one use of a dashboard tile, however it was reached, so the
/// dashboard can arrange itself by what the DJ reaches for.
#[tauri::command]
pub fn used_tile(state: State<'_, AppState>, id: String) {
    let mut interface = state.interface();
    interface.used(&id);
    state.set_interface(&interface);
}

/// §117: the DJ's own keys under Space.
#[tauri::command]
#[must_use]
pub fn leader_mine(state: State<'_, AppState>) -> Vec<crate::leader::Mine> {
    state.leader_mine()
}

/// §117: keep one of the DJ's own keys under Space, checked against the tree
/// it joins; one already on the same keys is replaced.
///
/// # Errors
/// When the keys would hide one of djmanzo's groups or hang under a leaf, or
/// what it runs is not something djmanzo does — said as a sentence.
#[tauri::command]
pub fn keep_mnemonic(
    state: State<'_, AppState>,
    keys: Vec<String>,
    label: String,
    run: String,
    decks: u8,
) -> Result<Vec<crate::leader::Mine>, String> {
    let one = crate::leader::Mine { keys, label, run };
    let (tree, switches) = leader_parts(&state, decks);
    crate::leader::check(&tree, &one, &switches).map_err(|refused| refused.to_string())?;
    let kept = crate::leader::keep(&state.leader_mine(), one);
    state.set_leader_mine(&kept);
    Ok(kept)
}

/// §117: forget the DJ's own key on these keys.
#[tauri::command]
#[must_use]
pub fn forget_mnemonic(state: State<'_, AppState>, keys: Vec<String>) -> Vec<crate::leader::Mine> {
    let kept = crate::leader::forget(&state.leader_mine(), &keys);
    state.set_leader_mine(&kept);
    kept
}

/// What the palette should offer for `query`, under §18's budget.
///
/// The judgement is read here rather than passed in, like every other reading
/// of the night: one judgement, made in one place. What the interface owns is
/// the drawing.
#[tauri::command]
#[must_use]
pub fn palette(state: State<'_, AppState>, query: String, decks: u8) -> PaletteDto {
    let snapshot = crate::Snapshot::capture(&state.registry(), state.deck_count());
    let activities = crate::activity::all(&state.activities().mine);
    let mut workspaces = state.my_workspaces();
    workspaces.extend(crate::cockpit::workspaces());
    offered_with(
        &query,
        decks,
        crate::cockpit::Attention::for_context(&snapshot).room_for,
        &switches(&activities, &workspaces, state.presets().packs(), decks),
    )
}

/// §115's *preset-packages and easy switching everywhere*: every preset a DJ
/// switches between, reachable from the one gesture that reaches everything
/// else — themes, activities, workspaces and the preset packs, each by its
/// own name.
///
/// Each is an entry of kind `switch`, whose `run` is `<kind> <which>`; the
/// interface carries it out through the path its own picker takes, so a theme
/// chosen here is declared to djmanzo exactly as one chosen in the switcher.
/// Built from the tables the pickers read, so nothing can be offered that a
/// picker does not have.
///
/// Tiered by when they are reached for: an activity or a preset is a move
/// made during a night (§109's *switch activities during a live DJ task*),
/// and is contextual; a theme or a workspace is arrangement, and is paperwork
/// that §18's budget leaves out mid-mix unless it is asked for by name.
#[must_use]
fn switches(
    activities: &[crate::activity::Activity],
    workspaces: &[crate::cockpit::Workspace],
    packs: &[dj_presets::Pack],
    decks: u8,
) -> Vec<PaletteEntryDto> {
    use crate::tiers::Tier;
    let entry = |label: String, about: &str, run: String, tier: Tier| PaletteEntryDto {
        label,
        about: about.to_owned(),
        kind: "switch",
        run,
        tier: tier.name(),
    };
    let mut out = Vec::new();
    for activity in activities {
        out.push(entry(
            format!("Activity \u{b7} {}", activity.title),
            &activity.doing,
            format!("activity {}", activity.slug),
            Tier::Contextual,
        ));
    }
    for pack in packs {
        for preset in &pack.presets {
            let decks_for: Vec<Option<u8>> = if preset.per_deck {
                (1..=decks.clamp(1, 6)).map(Some).collect()
            } else {
                vec![None]
            };
            for deck in decks_for {
                let (label, run) = match deck {
                    Some(deck) => (
                        format!("Preset \u{b7} {} \u{b7} deck {deck}", preset.name),
                        format!("preset {} {deck}", preset.id),
                    ),
                    None => (
                        format!("Preset \u{b7} {}", preset.name),
                        format!("preset {}", preset.id),
                    ),
                };
                out.push(entry(label, &preset.description, run, Tier::Contextual));
            }
        }
    }
    for theme in crate::theme::ALL {
        // A world opens a surface of its own; choosing one is the theme
        // picker's job, where the world is said to open.
        let (Some(pack), false) = (theme.pack, theme.world) else {
            continue;
        };
        out.push(entry(
            format!("Theme \u{b7} {}", theme.title),
            theme.about,
            format!("theme {pack}"),
            Tier::Preparation,
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for workspace in workspaces {
        // The DJ's own first, and a name only once: a saved "Club" is the one
        // they mean.
        if !seen.insert(workspace.name.clone()) {
            continue;
        }
        out.push(entry(
            format!("Workspace \u{b7} {}", workspace.name),
            &workspace.about,
            format!("workspace {}", workspace.name),
            Tier::Preparation,
        ));
    }
    out
}

/// The `run` of every switch the palette offers, which a key under Space may
/// name (§117).
pub(crate) fn switch_runs(
    activities: &[crate::activity::Activity],
    workspaces: &[crate::cockpit::Workspace],
    packs: &[dj_presets::Pack],
    decks: u8,
) -> Vec<String> {
    switches(activities, workspaces, packs, decks)
        .into_iter()
        .map(|entry| entry.run)
        .collect()
}

/// What survives §18's *"nothing else may take room"*.
///
/// §58 ranks the list and this cuts it. The first version of this said the
/// order mattered -- that cutting to twelve before dropping the paperwork
/// would leave a DJ with four rows -- and a mutation pass showed that it does
/// not: the tiers *are* the sort key, so "everything at or above this tier" is
/// a prefix of the sorted list and the two operations commute. It is written
/// in the order the rule is stated in, not because the answer depends on it.
///
/// What does depend on an order is the **note**. It is computed over the
/// twelve the DJ would otherwise have seen rather than over every match,
/// because on most queries the twelve nearest the hands are already the twelve
/// that fit: announcing a cut there would be djmanzo describing a decision it
/// did not make, over a list identical to the full one.
///
/// **A query that only matches paperwork still answers.** An interface that
/// refused to find Settings during a mix would be obeying §18 and breaking
/// §98, which says a whole night's work is reachable from here. The budget
/// governs what djmanzo *offers*, never what a DJ asks for by name — so when
/// the cut would leave nothing, there was nothing being offered in the first
/// place and the full list stands.
#[cfg(test)]
#[must_use]
fn offered(query: &str, decks: u8, room_for: crate::tiers::Tier) -> PaletteDto {
    offered_with(query, decks, room_for, &[])
}

/// [`offered`], with the switches a DJ has — see [`switches`].
#[must_use]
fn offered_with(
    query: &str,
    decks: u8,
    room_for: crate::tiers::Tier,
    switches: &[PaletteEntryDto],
) -> PaletteDto {
    let all = ranked(query, decks, switches);
    let within: Vec<PaletteEntryDto> = all
        .iter()
        .filter(|entry| tier_of(entry) <= room_for)
        .cloned()
        .collect();
    // Whether anything the DJ would otherwise have *seen* was removed, rather
    // than whether anything was filtered: the list is cut to twelve either way,
    // and on most queries the twelve nearest the hands are already the twelve
    // that fit. Saying "this was shortened" over a list that is identical to
    // the full one would be djmanzo describing a decision it did not make.
    let dropped = all
        .iter()
        .take(PALETTE_LIMIT)
        .any(|entry| tier_of(entry) > room_for);
    let quietened = dropped && !within.is_empty();
    let mut entries = if within.is_empty() { all } else { within };
    entries.truncate(PALETTE_LIMIT);
    PaletteDto {
        entries,
        because: if quietened {
            "Only what your hands need right now — type a name to reach anything else.".to_owned()
        } else {
            String::new()
        },
    }
}

/// The tier an entry carries, back as a tier.
///
/// The DTO holds §58's slug because that is what the interface draws with; this
/// is the one place it is read back, and an unknown one ranks last rather than
/// panicking — the same posture every other lookup here takes towards a name it
/// does not have.
fn tier_of(entry: &PaletteEntryDto) -> crate::tiers::Tier {
    crate::tiers::Tier::ALL
        .into_iter()
        .find(|tier| tier.name() == entry.tier)
        .unwrap_or(crate::tiers::Tier::Preparation)
}

/// Every entry matching `query`, ranked by §58 and not yet cut.
///
/// Separate from the cut so the budget can be applied between the two.
#[must_use]
fn ranked(query: &str, decks: u8, switches: &[PaletteEntryDto]) -> Vec<PaletteEntryDto> {
    use dj_core::vocabulary::{Target, vocabulary};

    let needle = query.trim();
    let mut out = Vec::new();

    // The query itself, when djmanzo can perform it. Kept at the top by the
    // sort below rather than by being pushed first.
    //
    // `SessionEvent::parse_line` rather than `Action::parse`, because a load is
    // not an action and the palette would otherwise refuse a line `perform`
    // accepts — a DJ typing `load deck 1 <id>` would be told the vocabulary does
    // not have it, and then find that it does. That parser is the session log's,
    // which makes the three readers of this language one language: what a set
    // file records, what the palette offers, and what the bus performs.
    if !needle.is_empty() && dj_control::SessionEvent::parse_line(needle).is_ok() {
        out.push(PaletteEntryDto {
            label: format!("Run: {needle}"),
            about: "The vocabulary accepts this exactly as typed.".to_owned(),
            kind: "action",
            run: needle.to_owned(),
            // Whatever it is, it is what the DJ typed, and the sort below keeps
            // it at the top regardless.
            tier: crate::tiers::Tier::Glanceable.name(),
        });
    }

    let decks = decks.clamp(1, 6);
    for spec in vocabulary() {
        // A verb needing an argument cannot be offered as a press: the palette
        // would have to invent the number. Typing it is how those are reached.
        if spec.argument.takes_argument() {
            continue;
        }
        match spec.target {
            Target::Deck => {
                for deck in 1..=decks {
                    let run = format!("deck {deck} {}", spec.verb);
                    let label = format!("Deck {deck} \u{b7} {}", spec.verb);
                    if matches(needle, &label) || matches(needle, &run) {
                        out.push(PaletteEntryDto {
                            label,
                            about: spec.help.to_owned(),
                            kind: "action",
                            run,
                            tier: crate::tiers::of_verb(spec.verb).name(),
                        });
                    }
                }
            }
            Target::Mixer => {
                let label = spec.verb.to_owned();
                if matches(needle, &label) || matches(needle, spec.example) {
                    out.push(PaletteEntryDto {
                        label,
                        about: spec.help.to_owned(),
                        kind: "action",
                        run: spec.example.to_owned(),
                        tier: crate::tiers::of_verb(spec.verb).name(),
                    });
                }
            }
        }
    }

    // The surfaces, by the words a DJ would reach for them with.
    for surface in crate::cockpit::surfaces() {
        let label = format!("Show {}", surface.title);
        if matches(needle, &label) || matches(needle, surface.name) {
            out.push(PaletteEntryDto {
                label,
                about: surface.about.to_owned(),
                kind: "surface",
                run: surface.name.to_owned(),
                tier: crate::tiers::of_surface(surface.name).name(),
            });
        }
    }

    // The rest of §41's interface vocabulary.
    //
    // Here because the palette is what §51 calls "the semantic interface", and
    // an operation only the assistant could reach would be a control a DJ has
    // no way to press. Pinning especially: it is the per-surface half of
    // freezing a layout, and until now there was no gesture for it anywhere.
    for surface in crate::cockpit::surfaces() {
        for (verb, word) in [("pin", "Pin"), ("unpin", "Unpin")] {
            let label = format!("{word} {}", surface.title);
            let run = format!("ui {verb} {}", surface.name);
            if matches(needle, &label) || matches(needle, &run) {
                out.push(PaletteEntryDto {
                    label,
                    about: if verb == "pin" {
                        "Hold it where it is, out of adaptation's reach.".to_owned()
                    } else {
                        "Let it be moved again.".to_owned()
                    },
                    kind: "ui",
                    run,
                    // Pinning a surface is arrangement, whatever the surface is
                    // for: it is a thing done to a layout rather than with a
                    // record. §58's tier 4.
                    tier: crate::tiers::Tier::Preparation.name(),
                });
            }
        }
    }
    for deck in 1..=decks {
        let label = format!("Focus deck {deck}");
        let run = format!("ui focus {deck}");
        if matches(needle, &label) || matches(needle, &run) {
            out.push(PaletteEntryDto {
                label,
                about: "Mark this deck for a moment.".to_owned(),
                kind: "ui",
                run,
                // Focus is a gesture made with a hand on a deck, not paperwork.
                tier: crate::tiers::Tier::Performable.name(),
            });
        }
    }

    // §115: the presets, by their own names.
    out.extend(
        switches
            .iter()
            .filter(|entry| matches(needle, &entry.label))
            .cloned(),
    );

    // §58, applied. Stable, so within a tier the passes' own order survives —
    // and the typed query keeps the top because it is pushed first and nothing
    // outranks `Glanceable`.
    out.sort_by_key(|entry| tier_of(entry).rank());
    out
}

/// Whether `haystack` contains every character of `needle`, in order.
///
/// A subsequence rather than a substring, because `d2p` should find
/// `Deck 2 · play` -- that is the gesture a palette exists for, and a substring
/// test would refuse it. Case-insensitive over ASCII; the labels are the
/// vocabulary's own verbs and the cockpit's own titles, which are ASCII.
fn matches(needle: &str, haystack: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut wanted = needle.chars().filter(|c| !c.is_whitespace()).peekable();
    for c in haystack.chars() {
        match wanted.peek() {
            None => return true,
            Some(next) if next.eq_ignore_ascii_case(&c) => {
                wanted.next();
            }
            Some(_) => {}
        }
    }
    wanted.peek().is_none()
}

// -- what a record is for ---------------------------------------------------

/// One function, with the words the interface shows beside it.
///
/// The label and the sentence come from Rust rather than being typed into the
/// interface, because the assistant and the network API need the same words. A
/// vocabulary explained in one place and re-explained in another is one that
/// drifts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FunctionDto {
    pub slug: &'static str,
    pub label: &'static str,
    pub about: &'static str,
    /// How many tracks carry it. Zero is reported, not omitted -- a picker
    /// that hides what you have never used never suggests using it.
    pub count: usize,
}

/// Every function a record can be for, with how many carry each.
#[tauri::command]
pub fn track_functions(state: State<'_, AppState>) -> Result<Vec<FunctionDto>, String> {
    let db = library(&state)?;
    Ok(db
        .function_counts()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(function, count)| FunctionDto {
            slug: function.slug(),
            label: function.label(),
            about: function.about(),
            count,
        })
        .collect())
}

/// What an audition is doing, as the rail needs to draw it.
#[derive(Debug, Clone, Serialize)]
pub struct AuditionDto {
    /// The candidate being listened to, or empty when the audition stopped.
    pub track: String,
    /// Where it started, in the candidate's own frames.
    pub from_frame: f64,
    /// The same in seconds, because a rail row says "from 1:47" and not
    /// "from 5064192". Converted here rather than in the interface for the
    /// reason every other seconds field in this file is: the sample rate is
    /// the record's and the interface does not have it.
    pub from_seconds: f64,
    /// `drop`, `vocal-entry` or `top`. See [`crate::audition::Reason`].
    pub because: String,
    /// What to tell the DJ: *from the drop*, *from the vocal*, *from the top*.
    pub says: String,
}

/// §22's *audition*: play a candidate into the headphones, without loading it.
///
/// `track` empty stops whatever is playing. Anything else is a track id, and
/// the record is decoded and handed to [`dj_engine::preview`], which mixes it
/// into the cue pair and has no route to the room.
///
/// Where it starts is `crate::audition`'s decision rather than the
/// interface's, and the answer comes back on [`AuditionDto`] so the rail can
/// say *from the drop* instead of leaving a DJ to wonder why one candidate
/// opened ninety seconds in.
///
/// **This does not load, stage, or write anything down.** §22 lists audition
/// and load as two of six separate things a DJ may do to a candidate, and a
/// preview that quietly counted as a play would put every record a DJ listened
/// to into their history and into §12's taste.
///
/// # Errors
/// A sentence naming which half is wrong -- the id, the library row or the
/// file -- for the reason `load_by_id` gives.
#[tauri::command]
pub async fn audition(
    state: State<'_, AppState>,
    track: String,
) -> Result<Option<AuditionDto>, String> {
    if track.trim().is_empty() {
        stop_audition(&state)?;
        return Ok(None);
    }

    let id = dj_core::TrackId::from_hex(track.trim())
        .ok_or_else(|| format!("not a track id: {:?}", track.trim()))?;
    let db = library(&state)?;
    let found = db
        .track(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no track {} in the library", id.to_hex()))?;

    // Off the caller's thread, like `load_track`: this is the interface's
    // thread and a file read on it is a frozen window.
    let path = found.path.clone();
    let decoded = tauri::async_runtime::spawn_blocking(move || decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    Ok(Some(begin_audition(&state, id, decoded)))
}

/// Stop whatever is being auditioned.
///
/// Silent about an engine queue that is full, unlike its neighbours, because
/// the one caller that cannot report an error is the one that matters: a DJ
/// stopping a preview because the room needs their attention.
fn stop_audition(state: &AppState) -> Result<(), String> {
    state
        .bus()
        .send_command(dj_engine::Command::Preview {
            source: None,
            from_frame: 0.0,
        })
        .map_err(|_| "the engine queue is full".to_owned())
}

/// Where the audition starts, and the send. Shared by both ways in.
///
/// The decision is here rather than at either caller, for the reason the
/// waveform's key is one function: two copies would be two answers to *where
/// does this record become interesting*, and a controller and a click would
/// audition the same record from two different places.
fn begin_audition(
    state: &AppState,
    id: dj_core::TrackId,
    decoded: dj_decode::DecodedTrack,
) -> AuditionDto {
    let start = crate::audition::start_of(
        state
            .analysis()
            .cached(&id)
            .as_ref()
            .map(|found| &found.trajectory),
        decoded.buffer.len_frames(),
    );
    let rate = decoded.buffer.sample_rate().as_f64();

    // A full queue costs the audition and nothing else. Every other caller
    // here treats it as an error because the thing that failed is the thing
    // the DJ asked for; this one has already done the expensive part, and a
    // preview that does not start is a button that did nothing rather than a
    // set in trouble.
    let _ = state.bus().send_command(dj_engine::Command::Preview {
        source: Some(std::sync::Arc::new(decoded.buffer)),
        from_frame: start.frame,
    });

    // §14's *previewed*, which was absent from `signals` until there was a
    // player to make it real. Recorded after the send and unconditionally:
    // what the signal is about is the DJ *deciding to listen to this record*,
    // and a queue that was briefly full is djmanzo's problem rather than a
    // fact about their evening.
    state.bus().record_audition(id);

    AuditionDto {
        track: id.to_hex(),
        from_frame: start.frame,
        from_seconds: start.frame / rate,
        because: start.reason.slug().to_owned(),
        says: start.reason.says().to_owned(),
    }
}

/// `<track-id>` or `stop` — §22's audition, from a controller or a script.
///
/// Outside the action vocabulary for exactly the reason a load is
/// ([ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)):
/// it carries an `Arc` of a decoded record, and nothing external should be
/// inventing one. **By id, never by path**, for the reason
/// [`load_by_id`] gives: a path here would hand anything that can reach the
/// bus a way to make djmanzo read an arbitrary file.
///
/// Decoded on the calling thread, which is `djmanzo-control` for a controller
/// and never the audio thread -- the same trade `load_by_id` makes and for the
/// same reason.
///
/// # Errors
/// A sentence naming which half is wrong: the id, the library row or the file.
fn audition_by_id(state: &AppState, rest: &str) -> Result<(), String> {
    let rest = rest.trim();
    if rest.is_empty() || rest.eq_ignore_ascii_case("stop") {
        return stop_audition(state);
    }
    let id = dj_core::TrackId::from_hex(rest).ok_or_else(|| format!("not a track id: {rest:?}"))?;
    let db = library(state)?;
    let found = db
        .track(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no track {} in the library", id.to_hex()))?;
    let decoded = decode_file(&found.path).map_err(|e| e.to_string())?;
    let _ = begin_audition(state, id, decoded);
    Ok(())
}

/// What one track is for.
#[tauri::command]
pub fn functions_of(state: State<'_, AppState>, track: String) -> Result<Vec<String>, String> {
    let Some(id) = dj_core::TrackId::from_hex(&track) else {
        return Ok(Vec::new());
    };
    let db = library(&state)?;
    Ok(db
        .functions_for(id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| f.slug().to_owned())
        .collect())
}

/// Set what some tracks are for, replacing whatever was there.
///
/// The whole answer, not a change to it: the picker shows every function with
/// the ones in force lit, so what it hands back is the state. An unknown slug
/// is dropped rather than refused -- the same rule the widget registry follows
/// -- so an interface from a later build cannot make this fail.
#[tauri::command]
pub fn set_track_functions(
    state: State<'_, AppState>,
    tracks: Vec<String>,
    functions: Vec<String>,
) -> Result<usize, String> {
    let ids: Vec<dj_core::TrackId> = tracks
        .iter()
        .filter_map(|hex| dj_core::TrackId::from_hex(hex))
        .collect();
    let chosen: Vec<dj_library::functions::Function> = functions
        .iter()
        .filter_map(|slug| dj_library::functions::Function::from_slug(slug))
        .collect();
    let db = library(&state)?;
    db.set_functions(&ids, &chosen).map_err(|e| e.to_string())
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
            // Read once and used for both destinations, so the tiles and the
            // engine cannot end up with different ideas of where a phrase
            // starts.
            let stored_phrase = match (found.analysis.phrase_beats, found.analysis.phrase_anchor) {
                (Some(beats), Some(anchor)) => dj_core::Phrase::new(beats, anchor),
                _ => None,
            };
            if let Some(grid) = found.analysis.beatgrid() {
                state.waveforms().set_analysed_grid(
                    deck,
                    Some(dj_render::GridOverlay {
                        lines: dj_render::GridLines::all(),
                        grid,
                        sample_rate: rate,
                        phrase: stored_phrase,
                    }),
                );
                let _ = state.bus().send_command(dj_engine::Command::SetGrid {
                    deck,
                    grid: Some(grid),
                    phrase: stored_phrase,
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

    db.set_loops(track, &loops).map_err(|e| e.to_string())?;
    // The lane draws saved loops from the library and asks for them on a load
    // and on an analysis landing -- neither of which is this. Without the
    // nudge, a loop kept mid-set appeared the next time the record was loaded.
    state.marks_changed(deck);
    Ok(())
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

    /// An app with a track "on" deck 1: in the library, and recorded as loaded,
    /// which is what the persistence paths key off.
    ///
    /// **The device is deliberately not opened.** It used to be, and the loop
    /// tests below raced it: `Engine` publishes `LoopActive`, `LoopStart` and
    /// `LoopEnd` into the registry from `deck.active_loop()` on every block, and
    /// a deck with no decoded track has no loop -- so a tick landing between
    /// `set_loop`'s write and `save_loop`'s read replaced the test's figures
    /// with zeros. `save_loop` checks `LoopActive` before it reads `LoopStart`,
    /// so the tick had a window in which the save went ahead and stored a loop
    /// starting at 0. That is exactly what it did, once, on the Windows runner,
    /// after passing on every platform for months.
    ///
    /// Nothing here needs a device: these are persistence paths, and what they
    /// touch is the registry and the library. What the engine does with a real
    /// loop is `dj-engine`'s to test, and is tested there. The rule in HANDOFF
    /// is the same one that fixed three other tests: do not hand-write a
    /// parameter the engine owns while the engine is running.
    fn app_with_track() -> AppState {
        let state = AppState::new(true);

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

    /// **§25's saved loops reach the waveform, in the order the pads are in.**
    ///
    /// The half a browser cannot prove: a stub that delivered them sorted would
    /// only prove the stub was sorted. Slot order is pad order, and the number
    /// drawn on a band is the number a DJ presses — one drawn against the wrong
    /// loop sends them to the wrong pad, which is worse than not drawing it.
    ///
    /// They are written out of order on purpose. The ordering is the store's —
    /// `Library::loops` selects `ORDER BY slot` — and this holds that contract
    /// from the side that depends on it, so dropping the clause fails here
    /// rather than silently putting the wrong number on a band.
    ///
    /// Worth recording how this test got its shape: `saved_loops_of` sorted the
    /// rows itself at first, and deleting that sort changed nothing, because
    /// the query had been doing the work the whole time. The sort came out.
    #[test]
    fn the_saved_loops_reach_the_waveform_in_pad_order() {
        let state = app_with_track();
        let db = state.library().get().unwrap();
        db.set_loops(
            id(1),
            &[
                dj_library::StoredLoop {
                    slot: 4,
                    start_frame: 800_000.0,
                    end_frame: 900_000.0,
                    label: None,
                },
                dj_library::StoredLoop {
                    slot: 2,
                    start_frame: 100_000.0,
                    end_frame: 200_000.0,
                    label: Some("the break".to_owned()),
                },
            ],
        )
        .unwrap();

        let kept = saved_loops_of(&state, 1);
        assert_eq!(
            kept.iter().map(|l| l.slot).collect::<Vec<_>>(),
            vec![2, 4],
            "the loops did not arrive in pad order"
        );
        // And each band keeps its own frames and its own label, which is the
        // failure a sort can introduce: two loops that swapped their spans
        // would still be in the right order.
        assert!((kept[0].start_frame - 100_000.0).abs() < f64::EPSILON);
        assert_eq!(kept[0].label.as_deref(), Some("the break"));
        assert!((kept[1].start_frame - 800_000.0).abs() < f64::EPSILON);
        assert_eq!(kept[1].label, None);
    }

    /// **A deck with nothing on it has no loops, and neither has a record
    /// nobody saved one in.**
    ///
    /// Empty rather than an error for both, because they draw the same thing —
    /// and a waveform that had to tell them apart would be a waveform with a
    /// failure state for the commonest case there is.
    #[test]
    fn a_record_with_no_saved_loops_draws_nothing_rather_than_failing() {
        let state = app_with_track();
        assert!(saved_loops_of(&state, 1).is_empty());

        // And a deck with nothing on it answers with nothing even while
        // another deck has loops — which is the version of this that fails
        // when the lookup falls back to whatever track it can find. The first
        // draft asserted only against an application where no deck had any,
        // and a fallback to deck 1 passed it.
        state
            .library()
            .get()
            .unwrap()
            .set_loops(
                id(1),
                &[dj_library::StoredLoop {
                    slot: 1,
                    start_frame: 10.0,
                    end_frame: 20.0,
                    label: None,
                }],
            )
            .unwrap();
        assert_eq!(saved_loops_of(&state, 1).len(), 1);
        assert!(
            saved_loops_of(&state, 4).is_empty(),
            "an empty deck answered with another deck's loops"
        );
    }

    fn grid_on_deck(state: &AppState, grid: Beatgrid) {
        state.waveforms().set_analysed_grid(
            deck(),
            Some(dj_render::GridOverlay {
                lines: dj_render::GridLines::all(),
                grid,
                sample_rate: SR,
                phrase: None,
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

    /// **A loop kept mid-set reaches the lane without reloading the record.**
    ///
    /// §25's `saved-loops` layer reads the library, and the waveform asks for
    /// it on a load and on an analysis landing. Saving a loop is neither, so
    /// the band appeared the next time that record went on a deck -- recorded
    /// as a limitation of the layer when it shipped. The deck's mark
    /// generation is the third thing for the waveform to watch.
    ///
    /// Per deck rather than global, because a frame that changed for every
    /// deck whenever any one of them was written would have the other lanes
    /// re-asking for rows nobody touched.
    #[test]
    fn saving_a_loop_moves_only_that_decks_mark_generation() {
        let state = app_with_track();
        let marks = state.marks();
        let other = dj_core::DeckId::from_human(2).unwrap();
        let before = (marks.generation(deck()), marks.generation(other));

        set_loop(&state, 96_000.0, 192_000.0);
        save_loop(&state, deck(), 1).unwrap();

        assert_eq!(
            marks.generation(deck()),
            before.0 + 1,
            "the lane has nothing telling it to ask again"
        );
        assert_eq!(
            marks.generation(other),
            before.1,
            "a loop saved on one deck moved another deck's generation"
        );

        // And it reaches the frame the interface is actually sent.
        let snapshot = snapshot_now(&state);
        assert_eq!(snapshot.decks[deck().index()].marks, before.0 + 1);
        assert_eq!(snapshot.decks[other.index()].marks, before.1);
    }

    /// A save that cannot happen must not claim the lane should look again.
    #[test]
    fn a_refused_save_does_not_move_the_generation() {
        let state = app_with_track();
        let marks = state.marks();
        let before = marks.generation(deck());

        // Nothing is looping, which `save_loop` refuses.
        assert!(save_loop(&state, deck(), 1).is_err());
        assert_eq!(marks.generation(deck()), before);
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
///
/// The formatting lives in [`crate::share`], which is the same text the
/// WhatsApp handoff sends. Two formatters for one tracklist would drift, and
/// the drift would show up as a promoter and a group chat being given
/// different accounts of the same night.
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

    let entries = crate::share::entries(&plays);
    let out = format!("{}\n", crate::share::as_file(&entries, &session));
    std::fs::write(&path, out).map_err(|e| format!("could not write {path}: {e}"))?;
    Ok(plays.len())
}

// -- sharing a set -------------------------------------------------------
//
// See `crate::share` for why the destination is decided in Rust and not
// named by the interface.

/// A set, ready to send, and what had to be left out to make it fit.
#[derive(Debug, Clone, Serialize)]
pub struct ShareDto {
    /// The message itself, exactly as it will arrive.
    pub message: String,
    /// How many records did not fit in the link. Zero for a set that fits.
    pub dropped: usize,
    /// How many there were altogether.
    pub total: usize,
}

/// A channel by its word, or the reason there is none. `None` is WhatsApp,
/// which is what every share was before there was a choice.
fn share_channel(channel: Option<&str>) -> Result<crate::share::Channel, String> {
    match channel {
        None => Ok(crate::share::Channel::WhatsApp),
        Some(slug) => crate::share::Channel::by_slug(slug)
            .ok_or_else(|| format!("djmanzo does not share to `{slug}`")),
    }
}

/// Read one session and build the message for it.
fn share_message(
    state: &State<'_, AppState>,
    session: &str,
    heading: &str,
    channel: crate::share::Channel,
) -> Result<ShareDto, String> {
    let plays = library(state)?
        .session(session)
        .map_err(|e| e.to_string())?;
    if plays.is_empty() {
        return Err(format!("there is nothing recorded for {session}"));
    }
    let entries = crate::share::entries(&plays);
    let style = crate::share::Style {
        heading: heading.to_string(),
        timestamps: true,
        limit_for_url: true,
    };
    let (message, dropped) = crate::share::message_for(&entries, &style, channel);
    Ok(ShareDto {
        message,
        dropped,
        total: entries.len(),
    })
}

/// Every external address djmanzo will open on the DJ's behalf.
///
/// Assembled from the two catalogs rather than written out, so a provider
/// added later is reachable without anybody remembering to update a list
/// here — a list that silently fell behind would show up as a "Get one"
/// link that does nothing, which is indistinguishable from a broken app.
fn known_links() -> Vec<&'static str> {
    let mut links: Vec<&'static str> = dj_assistant::catalog()
        .iter()
        .filter_map(|provider| provider.signup_url)
        .collect();
    for source in dj_sources::catalog() {
        links.extend(source.credentials.iter().map(|kind| kind.signup_url()));
    }
    links
}

/// Open one of djmanzo's own links in the DJ's browser.
///
/// A webview cannot reach a browser on its own: `target="_blank"` inside a
/// Tauri window opens nothing at all on Linux, which is how the "Get one →"
/// links next to every credential field came to be decorative.
///
/// The URL is checked against the catalogs rather than trusted. The interface
/// already has it — it came from a DTO this process filled in — so passing it
/// back is convenient, but "the webview handed me a URL" is not a reason to
/// ask the operating system to open it. Membership makes the difference
/// between a fixed link and a general-purpose way to launch anything.
#[tauri::command]
pub fn open_signup_link(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;

    if !known_links().contains(&url.as_str()) {
        return Err(format!("{url} is not one of djmanzo's links"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("could not open the link: {e}"))
}

/// §111: one store, as a "find it to buy" link.
#[derive(Debug, Clone, Serialize)]
pub struct StoreLinkDto {
    pub slug: String,
    pub name: String,
    pub sells: String,
    /// Whether the link searches for the song, or only reaches the store.
    pub searches: bool,
    pub karaoke: bool,
}

/// §111: where a song could be bought, and what buying it covers.
#[derive(Debug, Clone, Serialize)]
pub struct StoreLinksDto {
    pub stores: Vec<StoreLinkDto>,
    /// The one sentence always shown beside them. See
    /// `dj_sources::stores::WHAT_BUYING_COVERS`.
    pub covers: String,
}

/// §111: the stores a song could be bought from — karaoke's first when a
/// karaoke host is asking.
#[tauri::command]
#[must_use]
pub fn store_links(karaoke: bool) -> StoreLinksDto {
    let mut stores: Vec<&dj_sources::stores::Store> = dj_sources::stores::STORES.iter().collect();
    if karaoke {
        stores.sort_by_key(|store| !store.karaoke);
    }
    StoreLinksDto {
        stores: stores
            .into_iter()
            .map(|store| StoreLinkDto {
                slug: store.slug.to_owned(),
                name: store.name.to_owned(),
                sells: store.sells.to_owned(),
                searches: store.searches(),
                karaoke: store.karaoke,
            })
            .collect(),
        covers: dj_sources::stores::WHAT_BUYING_COVERS.to_owned(),
    }
}

/// §111: open a store's search for a song in the DJ's browser.
///
/// The address is built here from the store's own table and the song's
/// words, encoded — the webview names a store and a song, never a URL, for
/// the reason `open_signup_link` gives.
#[tauri::command]
pub fn open_store(
    app: tauri::AppHandle,
    store: String,
    artist: String,
    title: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;

    let found = dj_sources::stores::Store::by_slug(&store)
        .ok_or_else(|| format!("no store called {store:?}"))?;
    app.opener()
        .open_url(found.url_for(&artist, &title), None::<&str>)
        .map_err(|e| format!("could not open {}: {e}", found.name))
}

/// §111: the downloads folder, as the settings show it.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadsDto {
    pub watch: Option<String>,
    pub into: Option<String>,
    pub on: bool,
    /// What was filed lately, newest first.
    pub filed: Vec<crate::downloads::Filed>,
}

fn downloads_dto(state: &AppState) -> DownloadsDto {
    let settings = state.downloads();
    DownloadsDto {
        watch: settings.watch.map(|p| p.to_string_lossy().into_owned()),
        into: settings.into.map(|p| p.to_string_lossy().into_owned()),
        on: settings.on,
        filed: state.filed(),
    }
}

/// §115: the DJ's answer to a proposal — `taken`, `not-now` or
/// `not-tonight`. Taking one runs nothing here: the interface ran the
/// proposal's own action, the path any press takes; this only stops it being
/// proposed again straight away.
///
/// # Errors
/// When the answer is not one of the three.
#[tauri::command]
pub fn whisper_answer(
    state: State<'_, AppState>,
    kind: String,
    answer: String,
) -> Result<(), String> {
    let answer = crate::whisper::Answer::parse(&answer)
        .ok_or_else(|| format!("{answer:?} is not an answer to a proposal"))?;
    let now = std::time::Instant::now()
        .duration_since(*crate::START)
        .as_secs_f64();
    if let Ok(mut watcher) = state.whisper().lock() {
        watcher.answer(&kind, answer, now);
    }
    Ok(())
}

/// §108: what going live is doing — the overlay's address, the file, and
/// the words the stream is being told.
#[tauri::command]
pub fn live_status(state: State<'_, AppState>) -> crate::live::Status {
    let mut status = state.live_status();
    // The switch as stored, not as the thread last saw it half a second ago,
    // so the switch a DJ just pressed does not flick back.
    status.on = state.live().on;
    status
}

/// §108: tell the stream what is playing, or stop.
#[tauri::command]
pub fn set_live(state: State<'_, AppState>, on: bool) -> crate::live::Status {
    state.set_live(&crate::live::Settings { on });
    live_status(state)
}

/// §111: the downloads folder and what it filed.
#[tauri::command]
pub fn downloads(state: State<'_, AppState>) -> DownloadsDto {
    downloads_dto(&state)
}

/// §111: choose the folder to watch, the music folder to file into, and
/// whether to.
///
/// # Errors
/// When filing is switched on without both folders, when a folder does not
/// exist, or when the two are the same folder — watching the music folder
/// and filing into it would sort every loose file a DJ keeps there, which is
/// not what they asked for.
#[tauri::command]
pub fn set_downloads(
    state: State<'_, AppState>,
    watch: Option<String>,
    into: Option<String>,
    on: bool,
) -> Result<DownloadsDto, String> {
    let folder = |path: Option<String>| -> Result<Option<PathBuf>, String> {
        match path.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()) {
            None => Ok(None),
            Some(path) => {
                let path = PathBuf::from(path);
                if path.is_dir() {
                    Ok(Some(path))
                } else {
                    Err(format!("{} is not a folder", path.display()))
                }
            }
        }
    };
    let settings = crate::downloads::Settings {
        watch: folder(watch)?,
        into: folder(into)?,
        on,
    };
    if on && (settings.watch.is_none() || settings.into.is_none()) {
        return Err(
            "choose the folder to watch and the music folder before switching it on".to_owned(),
        );
    }
    if settings.watch.is_some() && settings.watch == settings.into {
        return Err("the downloads folder and the music folder have to be two folders".to_owned());
    }
    state.set_downloads(&settings);
    Ok(downloads_dto(&state))
}

/// Show the DJ what will be sent, before anything opens.
///
/// A preview rather than a straight-to-send button, because the message is
/// about to leave djmanzo entirely. It is also where the DJ finds out that a
/// four-hour set does not fit in a link, at a point where they can still
/// choose the file instead.
#[tauri::command]
pub fn share_preview(
    state: State<'_, AppState>,
    session: String,
    heading: String,
    channel: Option<String>,
) -> Result<ShareDto, String> {
    share_message(
        &state,
        &session,
        &heading,
        share_channel(channel.as_deref())?,
    )
}

/// §108: a recording of the night, and its tracklist timed against it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordingChaptersDto {
    /// The file's name, as the recordings folder shows it.
    pub file: String,
    pub path: String,
    /// Unix seconds, when it started.
    pub started_at: i64,
    pub seconds: f64,
    /// The chapters as a description's lines, `0:00 Artist - Title`.
    pub chapters: String,
    pub count: usize,
    /// Whether YouTube will draw them: it needs three or more.
    pub youtube: bool,
}

/// §108: every recording made during a night, each with the night's
/// tracklist timed against it, for a YouTube description or a Mixcloud
/// upload. See [`crate::chapters`].
///
/// # Errors
/// When the library cannot be read.
#[tauri::command]
pub fn recording_chapters(
    state: State<'_, AppState>,
    session: String,
) -> Result<Vec<RecordingChaptersDto>, String> {
    let plays = library(&state)?
        .session(&session)
        .map_err(|e| e.to_string())?;
    if plays.is_empty() {
        return Ok(Vec::new());
    }
    let Some(dir) = state.recordings_dir() else {
        return Ok(Vec::new());
    };
    Ok(crate::chapters::in_folder(&plays, &dir)
        .into_iter()
        .map(|found| RecordingChaptersDto {
            file: found.file,
            path: found.path.display().to_string(),
            started_at: found.start,
            seconds: found.seconds,
            chapters: crate::chapters::written(&found.chapters),
            count: found.chapters.len(),
            youtube: found.chapters.len() >= crate::chapters::YOUTUBE_FEWEST,
        })
        .collect())
}

/// One channel a set can be handed to.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ShareChannelDto {
    pub slug: &'static str,
    pub name: &'static str,
    /// The network's post limit, where it has one, for the panel to say.
    pub limit: Option<usize>,
}

/// §108: the channels a set can be handed to, in the order they are offered.
#[tauri::command]
pub fn share_channels() -> Vec<ShareChannelDto> {
    crate::share::Channel::ALL
        .iter()
        .map(|channel| ShareChannelDto {
            slug: channel.slug(),
            name: channel.name(),
            limit: channel.post_limit(),
        })
        .collect()
}

/// §108: open a network's composer with the set already written into it.
///
/// Posts nothing, as WhatsApp's never sent anything: the composer opens with
/// the words in it, and the DJ reads, edits and posts. The address is built
/// here from the channel's own table; the interface names a channel, never a
/// URL.
#[tauri::command]
pub fn share_to(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session: String,
    heading: String,
    channel: String,
) -> Result<ShareDto, String> {
    use tauri_plugin_opener::OpenerExt as _;

    let channel = share_channel(Some(&channel))?;
    let share = share_message(&state, &session, &heading, channel)?;
    app.opener()
        .open_url(channel.compose_url(&share.message), None::<&str>)
        .map_err(|e| format!("could not hand the set to {}: {e}", channel.name()))?;
    Ok(share)
}

/// Open WhatsApp with the set already written into the message box.
///
/// Sends nothing. It opens a compose window with no recipient chosen, and the
/// DJ picks who and presses send — see [`crate::share`] for why that division
/// is deliberate rather than a limitation.
#[tauri::command]
pub fn share_to_whatsapp(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session: String,
    heading: String,
) -> Result<ShareDto, String> {
    use tauri_plugin_opener::OpenerExt as _;

    let share = share_message(&state, &session, &heading, crate::share::Channel::WhatsApp)?;
    let url = crate::share::Channel::WhatsApp.compose_url(&share.message);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("could not hand the set to WhatsApp: {e}"))?;
    Ok(share)
}

/// Several ids at once, refusing the whole batch if any is malformed.
///
/// The whole batch, because a partial edit is worse than none: a DJ who
/// selected forty tracks and had thirty-nine change has no way to tell which.
fn parse_track_ids(hexes: &[String]) -> Result<Vec<dj_core::TrackId>, String> {
    hexes.iter().map(|hex| parse_track_id(hex)).collect()
}

#[cfg(test)]
mod coach_tests {
    use super::{COACH_WINDOW, recent_moments};
    use dj_control::{SessionEvent, TimedEvent};
    use dj_core::{Action, DeckAction, DeckId, TrackId};
    use std::time::Duration;

    fn action(secs: u64, action: Action) -> TimedEvent {
        TimedEvent::hand(Duration::from_secs(secs), SessionEvent::Action(action))
    }

    fn backspin(deck: u8) -> Action {
        Action::Deck {
            deck: DeckId::from_human(deck).expect("valid deck"),
            action: DeckAction::Backspin(None),
        }
    }

    /// **A load is not a technique.**
    ///
    /// It is how a record got there. Letting it through would put an entry in
    /// the coach's list that names nothing the DJ did with their hands.
    #[test]
    fn putting_a_record_on_is_not_something_to_name() {
        let log = vec![
            TimedEvent::hand(
                Duration::from_secs(1),
                SessionEvent::Load {
                    deck: DeckId::from_human(1).expect("valid deck"),
                    track: TrackId::from_hex(&"a".repeat(64)).expect("valid id"),
                },
            ),
            action(2, backspin(1)),
        ];
        let moments = recent_moments(&log, COACH_WINDOW);
        assert_eq!(moments.len(), 1);
        assert_eq!(moments[0].action, backspin(1));
    }

    /// **The window is measured from the last event, not from now.**
    ///
    /// A DJ who mixed and then stood still should still be told what they
    /// did. Measuring from wall-clock now would erase the mix precisely
    /// because they stopped to look at the panel.
    #[test]
    fn a_pause_does_not_erase_what_came_before_it() {
        let log = vec![action(1_000, backspin(1)), action(1_030, backspin(2))];
        let moments = recent_moments(&log, COACH_WINDOW);
        assert_eq!(moments.len(), 2, "a set that started late lost its history");
    }

    /// **Earlier in the night is not this mix.**
    #[test]
    fn events_older_than_the_window_are_left_out() {
        let log = vec![
            action(0, backspin(1)),
            action(1_000, backspin(2)),
            action(1_030, backspin(2)),
        ];
        let moments = recent_moments(&log, COACH_WINDOW);
        assert_eq!(moments.len(), 2, "an hour-old move was reported as recent");
    }

    #[test]
    fn an_empty_log_is_an_empty_window() {
        assert!(recent_moments(&[], COACH_WINDOW).is_empty());
    }
}

#[cfg(test)]
mod link_tests {
    use super::known_links;

    /// **The links djmanzo shows are the links djmanzo will open.**
    ///
    /// If these fall out of step, a "Get one" button next to a credential
    /// field refuses to open — and to the DJ that is a broken app, not a
    /// security decision.
    #[test]
    fn every_signup_link_shown_is_one_we_will_open() {
        let known = known_links();
        for provider in dj_assistant::catalog() {
            if let Some(url) = provider.signup_url {
                assert!(
                    known.contains(&url),
                    "{:?} is shown but refused",
                    provider.id
                );
            }
        }
        for source in dj_sources::catalog() {
            for kind in source.credentials {
                let url = kind.signup_url();
                assert!(known.contains(&url), "{url} is shown but refused");
            }
        }
    }

    /// **And nothing else is.**
    ///
    /// The whole point of checking membership rather than trusting the
    /// webview. Without this the command is a general-purpose way to ask the
    /// operating system to open anything at all.
    #[test]
    fn an_address_we_never_showed_is_not_in_the_list() {
        let known = known_links();
        for url in [
            "https://example.com/",
            "file:///etc/passwd",
            "https://openrouter.ai/keys/../../elsewhere",
            "javascript:alert(1)",
        ] {
            assert!(!known.contains(&url), "{url} should not be openable");
        }
    }

    /// **Every link is one djmanzo could actually reach.**
    ///
    /// A `file:` or `javascript:` entry creeping into a catalog would pass the
    /// membership check by definition, so the catalogs themselves are held to
    /// the rule rather than only the lookup.
    #[test]
    fn every_known_link_is_https() {
        for url in known_links() {
            assert!(url.starts_with("https://"), "{url} is not https");
        }
    }
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
/// The decks as the automix sees them, read from the same parameter registry
/// the interface draws from.
pub fn automix_view(state: &AppState) -> Vec<crate::automix::DeckView> {
    use dj_core::param::{DeckParam, GlobalParam};
    let registry = state.registry();
    let sample_rate = f64::from(
        registry
            .get(dj_core::ParamId::Global(GlobalParam::SampleRate))
            .max(1.0),
    );
    (0..state.deck_count())
        .filter_map(|index| dj_core::DeckId::new(index as u8))
        .map(|id| {
            let get = |p| registry.get(dj_core::ParamId::Deck(id, p));
            let bpm = f64::from(get(DeckParam::EffectiveBpm));
            crate::automix::DeckView {
                id,
                loaded: get(DeckParam::Loaded) >= 0.5,
                playing: get(DeckParam::Playing) >= 0.5,
                position: f64::from(get(DeckParam::Position)),
                length: f64::from(get(DeckParam::LengthFrames)),
                bpm: (bpm > 1.0).then_some(bpm),
                sample_rate,
            }
        })
        .collect()
}

/// Put the automix's own state where the interface can read it.
///
/// Through the parameter registry rather than a command of its own, so it
/// arrives on the same 60 Hz snapshot as everything else. One path to keep in
/// step instead of two.
pub fn publish_automix(state: &AppState, mix: &crate::automix::Automix) {
    use dj_core::param::GlobalParam;
    let registry = state.registry();
    let set = |param, value: f32| registry.set(dj_core::ParamId::Global(param), value);
    set(
        GlobalParam::AutomixEnabled,
        if mix.is_enabled() { 1.0 } else { 0.0 },
    );
    set(
        GlobalParam::AutomixMixing,
        if mix.is_mixing() { 1.0 } else { 0.0 },
    );
    set(GlobalParam::AutomixBeats, mix.beats());
    set(GlobalParam::AutomixStyle, mix.style().index() as f32);
    set(
        GlobalParam::AutomixHolding,
        if mix.held().is_some() { 1.0 } else { 0.0 },
    );
}

/// Send what the automix asked for.
///
/// Actions go back through `perform`, so they take exactly the same path as a
/// button press — including the interceptions above. Automix does not get a
/// private channel to the engine, and that is the point: everything it can do,
/// a person could have done.
pub fn run_automix_plan(state: &AppState, plan: crate::automix::Plan) {
    for action in &plan.actions {
        let text = action.to_string();
        // The machine's, every one: this is djmanzo running the mix. §67 asks
        // for AI and manual interventions as two different things, and an
        // automix blend filed under the DJ's hand would make a set they never
        // touched read as one they performed.
        if let Err(error) = perform_by(state, &text, dj_control::By::Machine) {
            tracing::warn!(%error, %text, "automix action refused");
        }
    }
    if let Some(deck) = plan.load {
        load_next_from_sidelist(state, deck);
    }
}

/// Take the top of the Sidelist and put it on `deck`.
///
/// The Sidelist rather than a queue of automix's own, because a DJ already has
/// somewhere they put what plays next, and a second list they had to remember
/// to fill would be a second list they forgot to fill. Taking the entry off as
/// it loads means the list is also a record of what is left.
fn load_next_from_sidelist(state: &AppState, deck: dj_core::DeckId) {
    let Ok(db) = library(state) else { return };
    let Ok(id) = db.system_playlist(SIDELIST, crate::library::now_seconds()) else {
        return;
    };
    let Ok(entries) = db.playlist_tracks(id) else {
        return;
    };
    let Some((position, track)) = entries.into_iter().next() else {
        // Nothing queued. Not an error and not worth a warning on every tick —
        // the interface shows an empty Sidelist, which is the whole message.
        return;
    };

    // Decoding reads and expands a whole file. This is called from the snapshot
    // pump, so doing it here would freeze the interface for as long as the
    // track takes to read — which is exactly why the load is asked for twenty
    // seconds ahead of when it is needed.
    let path = track.path.clone();
    match decode_file(&path) {
        Ok(decoded) => {
            if let Err(error) = put_on_deck(state, deck, decoded) {
                tracing::warn!(%error, "automix could not put the next track on a deck");
                return;
            }
        }
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "automix could not read the next track");
            // Left in the list on purpose: a track that will not load is
            // something the DJ needs to see, and silently dropping it means the
            // queue empties itself when a drive is unplugged.
            return;
        }
    }
    if let Err(error) = db.remove_from_playlist(id, position) {
        tracing::warn!(%error, "automix loaded a track but could not take it off the Sidelist");
    }
}

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
/// Every pad page and what is on it.
///
/// Sent to the interface rather than restated there, because the same table is
/// what a controller's pads will map onto in M4 and the two must not drift. The
/// action strings are pre-rendered against a deck number so the interface can
/// dispatch them without knowing the grammar — a pad is a label, an action and
/// a reason to light up.
#[must_use]
#[tauri::command]
pub fn pad_pages(deck: u8) -> Vec<PadPageDto> {
    let Some(id) = DeckId::from_human(deck) else {
        return Vec::new();
    };
    dj_core::PadPage::ALL
        .into_iter()
        .map(|page| PadPageDto {
            name: page.name().to_owned(),
            needs_grid: page.needs_grid(),
            pads: page
                .pads()
                .into_iter()
                .map(|pad| PadDto {
                    label: label_of(pad.label),
                    press: pad.press.map(|action| render(id, action)),
                    release: pad.release.map(|action| render(id, action)),
                    clear: pad.clear.map(|action| render(id, action)),
                    lit: pad.lit,
                })
                .collect(),
        })
        .collect()
}

/// A pad's action as a string the interface can dispatch without knowing the
/// grammar.
///
/// A deck action is addressed to this deck; a mixer action already addresses
/// the whole mixer and needs no deck number — the sampler is shared, and
/// writing `deck 2 sampler 1 trigger` would suggest otherwise.
fn render(deck: DeckId, action: dj_core::PadAction) -> String {
    match action {
        dj_core::PadAction::Deck(action) => dj_core::Action::Deck { deck, action }.to_string(),
        dj_core::PadAction::Mixer(action) => dj_core::Action::Mixer(action).to_string(),
    }
}

/// A pad's face, in words.
///
/// Rendered here rather than in the interface so a beat length is spelt the one
/// way — "1/4", the way a DJ says it and the way the loop controls already
/// write it.
fn label_of(label: dj_core::PadLabel) -> String {
    use dj_core::PadLabel;
    match label {
        PadLabel::Blank => String::new(),
        PadLabel::Number(n) => n.to_string(),
        PadLabel::Beats(beats) if beats >= 1.0 => format!("{}", (beats * 100.0).round() / 100.0),
        PadLabel::Beats(beats) if beats > 0.0 => format!("1/{}", (1.0 / beats).round()),
        PadLabel::Beats(_) => "0".to_owned(),
        PadLabel::FxSlot(n) => format!("FX{n}"),
        PadLabel::FxPlace(n) => format!("{n} post"),
        PadLabel::StemMute(stem) => format!("{} mute", stem.name()),
        PadLabel::StemSolo(stem) => format!("{} solo", stem.name()),
    }
}

/// One page, as the interface draws it.
#[derive(Debug, Clone, Serialize)]
pub struct PadPageDto {
    pub name: String,
    /// True when every pad on it is measured in beats, so the page is worth
    /// hiding on a track with no grid rather than showing eight dead buttons.
    pub needs_grid: bool,
    pub pads: Vec<PadDto>,
}

/// One pad, with its actions already written out.
#[derive(Debug, Clone, Serialize)]
pub struct PadDto {
    pub label: String,
    /// `null` for a pad this page leaves blank.
    pub press: Option<String>,
    /// Present only on a momentary pad.
    pub release: Option<String>,
    /// The secondary gesture — right-click on screen, shift on hardware.
    pub clear: Option<String>,
    pub lit: dj_core::Lit,
}

#[tauri::command]
pub fn list_layouts(state: State<'_, AppState>) -> Vec<crate::layout::Layout> {
    let mut layouts = crate::layout::builtin();
    if let Some(dir) = state.layout_dir() {
        layouts.extend(crate::layout::load_dir(&dir));
        // Tree-format files appear in the same picker, summarised down to what
        // the picker shows. Choosing one stores its name, and `layout_tree`
        // then finds the tree itself rather than this summary of it.
        layouts.extend(
            crate::widgets::load_dir(&dir)
                .iter()
                .map(crate::widgets::as_layout),
        );
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

/// Every widget djmanzo can draw, with its slots, its settings and their
/// ranges.
///
/// The vocabulary itself, so a layout editor -- or the assistant composing a
/// layout -- can be written against what exists rather than against a list
/// somebody typed twice.
#[tauri::command]
#[must_use]
pub fn widget_catalog() -> &'static [crate::widgets::Widget] {
    crate::widgets::catalog()
}

/// The slots and design tokens a layout may name.
#[derive(Debug, Clone, Serialize)]
pub struct VocabularyDto {
    pub slots: &'static [&'static str],
    /// Each token with the shape its value must take, so an editor can offer a
    /// colour picker for a colour and refuse a colour for a length.
    pub tokens: Vec<(&'static str, crate::widgets::TokenShape)>,
}

/// What a layout is allowed to say.
#[tauri::command]
#[must_use]
pub fn layout_vocabulary() -> VocabularyDto {
    VocabularyDto {
        slots: crate::widgets::SLOTS,
        tokens: crate::widgets::TOKENS.to_vec(),
    }
}

/// The chosen layout as a resolved widget tree.
///
/// The flat layout is upconverted rather than replaced, which is the migration
/// [ADR-0008](../../../docs/adr/0008-one-widget-vocabulary.md) asks for: an
/// existing layout file and the choice beside it become a tree on load, and
/// nobody's file breaks.
///
/// Anything a layout got wrong is in `notes` rather than in an error, so the
/// interface can show what did not load without refusing to open.
#[tauri::command]
#[must_use]
pub fn layout_tree(state: State<'_, AppState>) -> crate::widgets::Resolved {
    let chosen = state.chosen_layout();

    // A tree-format file wins over a flat one of the same name: it is the
    // newer thing the DJ wrote, and it can say strictly more.
    if let (Some(name), Some(dir)) = (chosen.as_deref(), state.layout_dir())
        && let Some(tree) = crate::widgets::load_dir(&dir)
            .into_iter()
            .find(|tree| tree.name == name)
    {
        return crate::widgets::resolve(&tree);
    }

    // `Layout::default()` rather than the first shipped preset, and the
    // difference is not cosmetic.
    //
    // This used to answer with `builtin().first()` -- "Starter", which hides
    // the pads, the loops, the effect rack, the beat jump, the filter and
    // keylock. So a DJ who had never opened the layout picker was being handed
    // a stripped deck by a command that had been asked no question. It went
    // unnoticed for as long as the interface only read the tokens out of this
    // answer and drew the deck from its own markup; the moment the deck
    // rendered from the tree, half of it disappeared.
    //
    // Nothing chosen means the application has not been told otherwise, which
    // is the full deck -- the same posture `chosen_layout` above already takes
    // when it refuses to invent a name for a layout that is not there.
    let layout = chosen_layout(state).unwrap_or_default();
    crate::widgets::resolve(&crate::widgets::from_layout(&layout))
}

// -- the cockpit ------------------------------------------------------------

/// Every surface the cockpit can place, with where it may go and what it costs.
///
/// The counterpart of `widget_catalog` for panels rather than for the things
/// inside a deck, and the same reasoning applies: a dock manager -- or the
/// assistant proposing an arrangement -- should be written against what exists
/// rather than against a list somebody typed twice.
#[tauri::command]
#[must_use]
pub fn cockpit_surfaces() -> &'static [crate::cockpit::Surface] {
    crate::cockpit::surfaces()
}

/// The window heights each density band starts at, and what each one scales to.
///
/// Handed over once at start-up rather than asked on every resize: Rust owns
/// the policy, the browser owns the pixels, and a command round trip per drag
/// of a window edge would be latency in exchange for nothing.
#[tauri::command]
#[must_use]
pub fn density_bands() -> Vec<(u16, &'static str, f32)> {
    crate::cockpit::BANDS
        .iter()
        .map(|(least, density)| (*least, density.name(), density.scale()))
        .collect()
}

/// §17: what the night's phase asks to have on screen.
///
/// Surfaces to *open*, never an arrangement to impose — what the DJ already
/// has stays, because their own choices outrank a phase reading and §17 says
/// the DJ must always be able to override it.
///
/// The phase is read here rather than passed in, for the reason every other
/// reading of it is: one judgement, made in one place. The *timing* is the
/// interface's, and it is not free to choose it — §18's `Attention::reflow` is
/// false during a mix, always, and nothing may move while somebody is reaching
/// for it.
#[tauri::command]
#[must_use]
pub fn phase_priorities(state: State<'_, AppState>) -> Vec<String> {
    crate::cockpit::priorities(state.night().read().map(|read| read.phase))
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

/// The arrangements that ship, for the picker.
#[tauri::command]
#[must_use]
pub fn cockpit_workspaces() -> Vec<crate::cockpit::Workspace> {
    crate::cockpit::workspaces()
}

/// How the cockpit is arranged, checked against what can actually be drawn.
///
/// Resolved rather than returned raw, so the interface never has to decide
/// whether a stored placement is legal -- the same division `layout_tree`
/// already draws between Rust owning the vocabulary and the interface owning
/// the pixels.
#[tauri::command]
#[must_use]
pub fn cockpit_workspace(state: State<'_, AppState>) -> crate::cockpit::Resolved {
    let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
    crate::cockpit::resolve(&stored)
}

/// Remember how the cockpit is arranged, and hand back what was kept.
///
/// The round trip matters: a placement the resolver corrected -- a surface
/// opened below the width it needs, a dock it cannot go in -- comes back
/// corrected, so what is stored and what is drawn are the same thing. Storing
/// the raw request and drawing the resolved one is how the two drift.
#[tauri::command]
#[must_use]
pub fn set_cockpit_workspace(
    state: State<'_, AppState>,
    workspace: crate::cockpit::Workspace,
) -> crate::cockpit::Resolved {
    let resolved = crate::cockpit::resolve(&workspace);
    state.set_workspace(&resolved.workspace);
    resolved
}

/// §43: how much the assistant is offering, and why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppetiteDto {
    /// `full`, `half`, `quarter` or `least`.
    pub appetite: String,
    /// Records played in a row that djmanzo did not suggest.
    pub ignored_in_a_row: u32,
    /// Rails put in front of the DJ this session.
    pub offers: u64,
    /// Of those, the ones they played from.
    pub taken: u64,
    /// The sentence. Why it is as loud as it is, and how to change it.
    pub says: String,
}

/// What the assistant is offering, and why.
///
/// **Said out loud on purpose.** §43's instruction is *do not spam*, and the
/// obvious implementation of it — go quiet and say nothing — reads as a broken
/// feature: a DJ whose rail has thinned has no way to tell whether djmanzo has
/// given up on them, crashed, or run out of library. This is the sentence that
/// says which, and says that playing one suggested record undoes it.
#[tauri::command]
#[must_use]
pub fn assistant_appetite(state: State<'_, AppState>) -> AppetiteDto {
    let held = state.fatigue();
    let Ok(fatigue) = held.lock() else {
        return AppetiteDto {
            appetite: dj_assistant::Appetite::Full.name().to_owned(),
            ignored_in_a_row: 0,
            offers: 0,
            taken: 0,
            says: String::new(),
        };
    };
    AppetiteDto {
        appetite: fatigue.appetite().name().to_owned(),
        ignored_in_a_row: fatigue.ignored_in_a_row(),
        offers: fatigue.offers(),
        taken: fatigue.taken(),
        says: fatigue.says(),
    }
}

/// One of §20's columns, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColumnDto {
    /// The slug stored and sent back.
    pub slug: String,
    /// The word at the top of the column.
    pub heading: String,
    /// What it means, for the picker and for a hover.
    pub about: String,
}

/// One of §30's semantic roles, and which others it must be told apart from.
///
/// `SemanticRoleDto` rather than `RoleDto`: the mapping editor already has a
/// `RoleDto` and it means a different thing entirely -- what a *control* does
/// when a hand moves it. Two unrelated ideas called Role is the collision this
/// file would otherwise ship.
#[derive(Debug, Clone, Serialize)]
pub struct SemanticRoleDto {
    /// The custom property it is published as, without the `--`.
    pub token: String,
    /// What it means. §30's own sentence.
    pub about: String,
    /// The tokens of every role this one has to look different from.
    ///
    /// Sorted, so the golden file does not churn on an enum reordering.
    pub must_differ_from: Vec<String>,
}

/// §30's roles, with the pairs a theme may not collapse.
///
/// Listed by Rust for the reason §20's columns are: the pairs are a judgement
/// about what sits beside what, made once in `cockpit::Role`, and a second
/// copy in the interface is a second answer. The interface does not *call*
/// this — it reads the golden file blessed from it — which is the point: a
/// pair added here fails a browser test until the theme that has to honour it
/// is checked.
#[tauri::command]
#[must_use]
pub fn semantic_roles() -> Vec<SemanticRoleDto> {
    crate::cockpit::Role::ALL
        .iter()
        .map(|role| {
            let mut differ: Vec<String> = crate::cockpit::Role::ALL
                .iter()
                .filter(|other| role.must_differ_from(**other))
                .map(|other| other.token().to_owned())
                .collect();
            differ.sort_unstable();
            SemanticRoleDto {
                token: role.token().to_owned(),
                about: role.about().to_owned(),
                must_differ_from: differ,
            }
        })
        .collect()
}

/// Every column §20's performance table can carry.
///
/// Listed by Rust, with its heading and its sentence, so a column added there
/// appears in the picker without anybody editing the browser — and so the word
/// a DJ reads in the picker is the word at the top of the column.
#[tauri::command]
#[must_use]
pub fn library_columns() -> Vec<ColumnDto> {
    crate::columns::Column::ALL
        .iter()
        .map(|column| ColumnDto {
            slug: column.name().to_owned(),
            heading: column.heading().to_owned(),
            about: column.about().to_owned(),
        })
        .collect()
}

/// The columns the DJ has chosen, in their order.
#[tauri::command]
#[must_use]
pub fn chosen_columns(state: State<'_, AppState>) -> Vec<String> {
    crate::columns::choose(&state.chosen_columns())
        .into_iter()
        .map(|column| column.name().to_owned())
        .collect()
}

/// Choose the columns, and take back what will actually be drawn.
///
/// The round trip is the point, the same as the workspace's: a column this
/// build does not have is dropped and the title is put back, so what is stored
/// and what is drawn cannot drift apart.
#[tauri::command]
#[must_use]
pub fn set_chosen_columns(state: State<'_, AppState>, columns: Vec<String>) -> Vec<String> {
    let chosen: Vec<String> = crate::columns::choose(&columns)
        .into_iter()
        .map(|column| column.name().to_owned())
        .collect();
    state.set_chosen_columns(&chosen);
    chosen
}

/// How the performance table is sorted.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SortDto {
    /// The column slug, as `library_columns` spells it.
    pub column: String,
    /// Smallest first.
    pub ascending: bool,
}

/// How the DJ last sorted the library.
///
/// §8 Level 1's *sorting*. The browser used to start at artist, A to Z, every
/// time it was mounted — including every time a panel closed and reopened —
/// so a DJ who put the table in BPM order to find the slow records lost it the
/// moment they looked at anything else.
#[tauri::command]
#[must_use]
pub fn library_sort(state: State<'_, AppState>) -> SortDto {
    let sort = state.library_sort().unwrap_or_default();
    SortDto {
        column: sort.by.name().to_owned(),
        ascending: sort.ascending,
    }
}

/// Remember how the library is sorted, and take back what will be used.
///
/// The round trip the columns and the workspace both make: a column this build
/// does not have falls back to the default rather than being stored and
/// silently ignored.
#[tauri::command]
#[must_use]
pub fn set_library_sort(state: State<'_, AppState>, column: String, ascending: bool) -> SortDto {
    let sort = crate::columns::Sort::of(&column, ascending);
    state.set_library_sort(sort);
    SortDto {
        column: sort.by.name().to_owned(),
        ascending: sort.ascending,
    }
}

/// The pad pages the DJ has starred.
///
/// §8 Level 1's *favorite pad pages*, and the word is `favorite` rather than
/// `last used` on purpose. The pad zone deliberately did not remember the page
/// it was left on, and the reason was a good one: a DJ who left the pads on
/// the roll page an hour ago does not want to come back to a deck whose cues
/// are hidden. A favourite is not that — it is a deliberate choice about how
/// this DJ plays, which is exactly what §8 Level 1 is a list of.
#[tauri::command]
#[must_use]
pub fn favourite_pad_pages(state: State<'_, AppState>) -> Vec<String> {
    keeping_pages(&state.favourite_pad_pages())
}

/// Star the pad pages, and take back what will actually be used.
#[tauri::command]
#[must_use]
pub fn set_favourite_pad_pages(state: State<'_, AppState>, pages: Vec<String>) -> Vec<String> {
    let kept = keeping_pages(&pages);
    state.set_favourite_pad_pages(&kept);
    kept
}

/// Drop pages this build does not have, and collapse repeats.
///
/// The same round trip as the columns'. A page name is stored text and a later
/// djmanzo may have pages this one has never heard of.
fn keeping_pages(asked: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for page in asked
        .iter()
        .filter_map(|name| dj_core::PadPage::parse(name))
    {
        let name = page.name().to_owned();
        if !kept.contains(&name) {
            kept.push(name);
        }
    }
    kept
}

/// One control §74's rail can hold, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReachDto {
    /// The slug stored and sent back.
    pub slug: String,
    /// What it does, in a sentence.
    pub about: String,
}

/// Every control the rail can hold.
#[tauri::command]
#[must_use]
pub fn rail_controls() -> Vec<ReachDto> {
    crate::at_hand::Reach::ALL
        .iter()
        .map(|reach| ReachDto {
            slug: reach.name().to_owned(),
            about: reach.about().to_owned(),
        })
        .collect()
}

/// The controls the DJ keeps within reach whatever the deck is doing.
///
/// §8 Level 1's *preferred controls*. Empty is the ordinary case and means
/// djmanzo judges the whole rail, which is what §74 describes.
#[tauri::command]
#[must_use]
pub fn kept_controls(state: State<'_, AppState>) -> Vec<String> {
    crate::at_hand::keeping(&state.kept_controls())
        .into_iter()
        .map(|reach| reach.name().to_owned())
        .collect()
}

/// Keep these controls within reach, and take back what the rail will use.
#[tauri::command]
#[must_use]
pub fn set_kept_controls(state: State<'_, AppState>, controls: Vec<String>) -> Vec<String> {
    let kept: Vec<String> = crate::at_hand::keeping(&controls)
        .into_iter()
        .map(|reach| reach.name().to_owned())
        .collect();
    state.set_kept_controls(&kept);
    kept
}

/// One of §25's layers, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LayerChoiceDto {
    /// The slug stored and sent back — the same one the interface already puts
    /// on screen as `data-layer`. `name` rather than `slug`, unlike the other
    /// pickers here, because this table shipped with that spelling and a rename
    /// would be churn in every test that reads a layer off the page.
    pub name: String,
    /// What a DJ would call it.
    pub title: String,
    /// What it encodes, in one line.
    pub about: String,
    /// What its colour means. §57's rule is over this.
    pub role: dj_render::Role,
    /// Which half of the renderer draws it, or that nothing does yet.
    pub drawn: dj_render::Drawn,
    /// False for the two that *are* the waveform, and for the eight nobody has
    /// built. The box is shown either way and says which.
    pub choosable: bool,
    /// Why it cannot be turned off, for the rows that cannot. Empty otherwise.
    pub why_not: String,
}

/// §25's twenty, in the shape the interface reads them.
///
/// A free function as well as a command, because the browser harness's golden
/// file is blessed from it: the stub then answers the shape djmanzo really
/// sends, which is the whole point of having the golden file at all.
#[must_use]
pub fn layer_choices() -> Vec<LayerChoiceDto> {
    dj_render::layers()
        .iter()
        .map(|layer| LayerChoiceDto {
            name: layer.name.to_owned(),
            title: layer.title.to_owned(),
            about: layer.about.to_owned(),
            role: layer.role,
            drawn: layer.drawn,
            choosable: layer.choosable(),
            why_not: if layer.choosable() {
                String::new()
            } else if layer.exists() {
                "This is the waveform itself".to_owned()
            } else {
                "djmanzo cannot draw this yet".to_owned()
            },
        })
        .collect()
}

/// The waveform layers the DJ has chosen, in §25's order.
///
/// §8 Level 1's *preferred waveform display*, which was the one row of that
/// list djmanzo did not keep — because until §25's inventory could be chosen
/// from there was no preference to keep.
#[tauri::command]
#[must_use]
pub fn chosen_layers(state: State<'_, AppState>) -> Vec<String> {
    dj_render::choosing(&state.waveform_layers())
        .into_iter()
        .map(|layer| layer.name.to_owned())
        .collect()
}

/// Choose the layers, and take back what will actually be drawn.
///
/// The round trip every other picker here makes: a layer this build does not
/// have is dropped, the two that are the waveform itself go back, and an empty
/// ask is the whole instrumentation rather than an empty strip.
#[tauri::command]
#[must_use]
pub fn set_chosen_layers(state: State<'_, AppState>, layers: Vec<String>) -> Vec<String> {
    let chosen: Vec<String> = dj_render::choosing(&layers)
        .into_iter()
        .map(|layer| layer.name.to_owned())
        .collect();
    state.set_waveform_layers(&chosen);
    chosen
}

/// §107: the rotation as the Singers surface draws it.
#[derive(Debug, Clone, Serialize)]
pub struct RotationDto {
    /// In calling order.
    pub singers: Vec<crate::karaoke::Singer>,
    /// Who is up next, when anybody is.
    pub up_next: Option<String>,
    /// The last few songs sung, newest first: what the host glances at to
    /// answer "did I already sing tonight?".
    pub lately: Vec<crate::karaoke::Sung>,
}

fn rotation_dto(rotation: &crate::karaoke::Rotation) -> RotationDto {
    RotationDto {
        singers: rotation.singers.clone(),
        up_next: rotation.up_next().map(|(singer, _)| singer.name.clone()),
        lately: rotation.history.iter().rev().take(12).cloned().collect(),
    }
}

/// Read the rotation, change it, keep it, and answer with what it now is.
fn with_rotation(
    state: &AppState,
    change: impl FnOnce(&mut crate::karaoke::Rotation) -> bool,
) -> RotationDto {
    let mut rotation = state.karaoke();
    if change(&mut rotation) {
        state.set_karaoke(&rotation);
    }
    rotation_dto(&rotation)
}

/// §107: the singer rotation.
#[tauri::command]
#[must_use]
pub fn karaoke_rotation(state: State<'_, AppState>) -> RotationDto {
    rotation_dto(&state.karaoke())
}

/// §107: somebody asks for a song. With no key, the key they sang it in last
/// time.
///
/// # Errors
/// An empty name or title.
#[tauri::command]
pub fn karaoke_ask(
    state: State<'_, AppState>,
    singer: String,
    title: String,
    track: Option<String>,
    path: Option<String>,
    key: Option<i32>,
) -> Result<RotationDto, String> {
    let mut taken = false;
    let dto = with_rotation(&state, |rotation| {
        taken = rotation.ask(&singer, &title, track, path, key);
        taken
    });
    if taken {
        Ok(dto)
    } else {
        Err("a request needs a singer's name and a song".to_owned())
    }
}

/// §107: the singer up next has sung; they go to the back.
#[tauri::command]
#[must_use]
pub fn karaoke_sang(state: State<'_, AppState>) -> RotationDto {
    with_rotation(&state, |rotation| rotation.sang().is_some())
}

/// §107: called and not there — to the bottom, songs kept.
#[tauri::command]
#[must_use]
pub fn karaoke_not_here(state: State<'_, AppState>, singer: String) -> RotationDto {
    with_rotation(&state, |rotation| rotation.not_here(&singer))
}

/// §107: the host moves somebody to a place in the calling order.
#[tauri::command]
#[must_use]
pub fn karaoke_move(state: State<'_, AppState>, singer: String, to: usize) -> RotationDto {
    with_rotation(&state, |rotation| rotation.move_to(&singer, to))
}

/// §107: somebody leaves; their history, and so their keys, stay.
#[tauri::command]
#[must_use]
pub fn karaoke_leave(state: State<'_, AppState>, singer: String) -> RotationDto {
    with_rotation(&state, |rotation| rotation.leave(&singer))
}

/// §107: the key of a singer's next song, in semitones.
#[tauri::command]
#[must_use]
pub fn karaoke_key(state: State<'_, AppState>, singer: String, key: i32) -> RotationDto {
    with_rotation(&state, |rotation| rotation.set_key(&singer, key))
}

/// §107: a new night — everybody off the list, the history kept.
#[tauri::command]
#[must_use]
pub fn karaoke_clear(state: State<'_, AppState>) -> RotationDto {
    with_rotation(&state, |rotation| {
        rotation.clear();
        true
    })
}

/// §107: the words for the record on a deck, for the singers' screen.
///
/// Read from what the library already stored — the lyrics sweep fetches them
/// from LRCLIB beforehand — so the screen never waits on the network while a
/// singer stands at the microphone.
#[tauri::command]
pub fn singer_lyrics(state: State<'_, AppState>, deck: u8) -> crate::karaoke::SingerLyrics {
    let stored = DeckId::from_human(deck)
        .and_then(|deck| state.deck_track_id(deck))
        .and_then(|track| state.library().get().ok()?.words_for(track).ok()?);
    crate::karaoke::lyrics_for(stored)
}

/// §109: one activity as the strip draws it.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityDto {
    pub slug: String,
    pub title: String,
    pub doing: String,
    pub icon: String,
    pub shipped: bool,
    /// The key that reaches it — `F1` to `F9` — when it has one.
    pub key: Option<String>,
    pub workspace: crate::cockpit::Workspace,
}

/// §109: the strip, and the mode it is in.
#[derive(Debug, Clone, Serialize)]
pub struct ActivitiesDto {
    pub activities: Vec<ActivityDto>,
    /// The key that returns to the previous activity, as `KeyboardEvent.code`
    /// spells it.
    pub back: String,
    pub on: bool,
    pub current: String,
    pub previous: String,
}

/// The strip for a given kept state. Public so the browser fixture can be
/// generated from it rather than typed out.
#[must_use]
pub fn activities_dto(kept: &crate::activity::Kept) -> ActivitiesDto {
    ActivitiesDto {
        activities: crate::activity::all(&kept.mine)
            .into_iter()
            .enumerate()
            .map(|(index, activity)| ActivityDto {
                key: crate::activity::key_for(index),
                slug: activity.slug,
                title: activity.title,
                doing: activity.doing,
                icon: activity.icon,
                shipped: activity.shipped,
                workspace: activity.workspace,
            })
            .collect(),
        back: crate::activity::BACK_KEY.to_owned(),
        on: kept.on,
        current: kept.current.clone(),
        previous: kept.previous.clone(),
    }
}

/// §109: every activity — djmanzo's, then the DJ's own — and the mode.
#[tauri::command]
#[must_use]
pub fn activities(state: State<'_, AppState>) -> ActivitiesDto {
    activities_dto(&state.activities())
}

/// §109: turn activity mode on or off, or move to another activity.
///
/// The interface applies the activity's workspace itself, through the path
/// every arrangement takes; this keeps where the DJ is, so a restart comes
/// back to it. An activity nobody has is refused rather than kept, so the
/// file never names a tab the strip cannot draw.
///
/// # Errors
/// When `current` names no activity.
#[tauri::command]
pub fn set_activity_mode(
    state: State<'_, AppState>,
    on: bool,
    current: String,
) -> Result<ActivitiesDto, String> {
    let mut kept = state.activities();
    let known = crate::activity::all(&kept.mine);
    if !current.is_empty() && !known.iter().any(|activity| activity.slug == current) {
        return Err(format!("there is no activity called {current:?}"));
    }
    if current != kept.current {
        kept.previous = std::mem::replace(&mut kept.current, current);
    }
    kept.on = on;
    state.set_activities(&kept);
    Ok(activities_dto(&kept))
}

/// §109: keep the arrangement on screen as an activity of the DJ's own.
///
/// # Errors
/// An empty name, or one djmanzo ships.
#[tauri::command]
pub fn keep_activity(
    state: State<'_, AppState>,
    title: String,
    workspace: crate::cockpit::Workspace,
) -> Result<ActivitiesDto, String> {
    let mut kept = state.activities();
    kept.mine = crate::activity::keep(&kept.mine, &title, workspace)
        .map_err(|refused| refused.to_string())?;
    state.set_activities(&kept);
    Ok(activities_dto(&kept))
}

/// §109: forget one of the DJ's own. Leaving it while it is on screen drops
/// the strip back to no current activity rather than to a tab that is gone.
#[tauri::command]
#[must_use]
pub fn forget_activity(state: State<'_, AppState>, slug: String) -> ActivitiesDto {
    let mut kept = state.activities();
    kept.mine = crate::activity::forget(&kept.mine, &slug);
    if kept.current == slug {
        kept.current.clear();
    }
    if kept.previous == slug {
        kept.previous.clear();
    }
    state.set_activities(&kept);
    activities_dto(&kept)
}

/// §118: where events are kept, `events/` beside the settings.
fn events_dir(state: &AppState) -> Result<std::path::PathBuf, String> {
    state
        .config_dir()
        .map(|dir| crate::gig::folder(&dir))
        .ok_or_else(|| "no settings folder to keep events in yet".to_owned())
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// §118: every event kept, the soonest first.
///
/// # Errors
/// When djmanzo has no settings folder yet.
#[tauri::command]
pub fn list_events(state: State<'_, AppState>) -> Result<Vec<crate::gig::Summary>, String> {
    let dir = events_dir(&state)?;
    Ok(crate::gig::list(&dir)
        .iter()
        .map(crate::gig::summary)
        .collect())
}

/// §118: one event, with its steps, ideas, path and running order.
///
/// # Errors
/// An event that is not kept.
#[tauri::command]
pub fn event_view(state: State<'_, AppState>, id: String) -> Result<crate::gig::View, String> {
    let dir = events_dir(&state)?;
    crate::gig::load(&dir, &id)
        .map(crate::gig::view)
        .ok_or_else(|| format!("there is no event {id:?}"))
}

/// §118: start preparing an event. Kept at once, so a preparation begun is
/// never lost by closing the panel before the first edit.
///
/// # Errors
/// An empty name, a date djmanzo cannot read, or the file system's refusal.
#[tauri::command]
pub fn new_event(
    state: State<'_, AppState>,
    title: String,
    date: String,
) -> Result<crate::gig::View, String> {
    let dir = events_dir(&state)?;
    let taken: Vec<String> = crate::gig::list(&dir).into_iter().map(|g| g.id).collect();
    let gig = crate::gig::Gig {
        id: crate::gig::new_id(&title, &date, &taken),
        title: title.trim().to_owned(),
        date,
        ..crate::gig::Gig::default()
    };
    crate::gig::save(&dir, gig, now_seconds()).map(crate::gig::view)
}

/// §118: keep an edit. The panel writes on every change, so this is called
/// often and answers with the whole view: what a change leaves missing is
/// Rust's to say, not the panel's to guess.
///
/// # Errors
/// See [`crate::gig::Refused`], or the file system's refusal.
#[tauri::command]
pub fn save_event(
    state: State<'_, AppState>,
    gig: crate::gig::Gig,
) -> Result<crate::gig::View, String> {
    let dir = events_dir(&state)?;
    crate::gig::save(&dir, gig, now_seconds()).map(crate::gig::view)
}

/// §118: take one of the ideas offered for an event.
///
/// # Errors
/// An event that is not kept, or the file system's refusal.
#[tauri::command]
pub fn take_event_idea(
    state: State<'_, AppState>,
    id: String,
    adds: crate::gig::Adds,
) -> Result<crate::gig::View, String> {
    let dir = events_dir(&state)?;
    let gig = crate::gig::load(&dir, &id).ok_or_else(|| format!("there is no event {id:?}"))?;
    crate::gig::save(&dir, crate::gig::take(gig, &adds), now_seconds()).map(crate::gig::view)
}

/// §118: forget an event.
///
/// # Errors
/// An id djmanzo did not make, or the file system's refusal.
#[tauri::command]
pub fn forget_event(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<crate::gig::Summary>, String> {
    let dir = events_dir(&state)?;
    crate::gig::forget(&dir, &id)?;
    Ok(crate::gig::list(&dir)
        .iter()
        .map(crate::gig::summary)
        .collect())
}

/// §118: the night being played, at the DJ's own date and time.
///
/// The interface passes the date and the minutes past midnight because the
/// DJ's clock is the one the host agreed times on, and only the interface
/// knows its time zone.
///
/// # Errors
/// An event that is not kept.
#[tauri::command]
pub fn event_tonight(
    state: State<'_, AppState>,
    id: String,
    today: String,
    now: u32,
) -> Result<crate::gig::Tonight, String> {
    let dir = events_dir(&state)?;
    let gig = crate::gig::load(&dir, &id).ok_or_else(|| format!("there is no event {id:?}"))?;
    Ok(crate::gig::tonight(&gig, &today, now.min(24 * 60 - 1)))
}

/// §118: the event being played, if one is -- kept so a restart mid-night
/// comes back to it.
///
/// # Errors
/// When djmanzo has no settings folder yet.
#[tauri::command]
pub fn live_event(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(crate::gig::live(&events_dir(&state)?))
}

/// §118: play an event, or end the night with `None`.
///
/// # Errors
/// An event that is not kept, or the file system's refusal.
#[tauri::command]
pub fn set_live_event(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<Option<String>, String> {
    let dir = events_dir(&state)?;
    crate::gig::set_live(&dir, id.as_deref())?;
    Ok(crate::gig::live(&dir))
}

/// §118: the fixed lists the event panel offers.
#[tauri::command]
#[must_use]
pub fn event_options() -> crate::gig::Options {
    crate::gig::options()
}

/// §109 and §115: the activity the moment seems to call for, and why — or
/// nothing. A suggestion for the strip to mark, never a switch.
#[tauri::command]
#[must_use]
pub fn activity_suggestion(state: State<'_, AppState>) -> Option<crate::activity::Suggestion> {
    let snapshot = get_snapshot(state.clone());
    crate::activity::suggest(&snapshot, state.audience().waiting().len())
}

/// One way of colouring the waveform's spectral balance, as the picker offers
/// it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColouringDto {
    /// The word in the tile URL and the settings file.
    pub slug: String,
    pub title: String,
    pub about: String,
}

/// Both ways of colouring the spectral balance, in the order a picker offers
/// them. From `dj_render::Colouring`, so a picker cannot offer one the
/// renderer does not draw.
#[tauri::command]
#[must_use]
pub fn waveform_colourings() -> Vec<ColouringDto> {
    dj_render::Colouring::ALL
        .into_iter()
        .map(|colouring| ColouringDto {
            slug: colouring.slug().to_owned(),
            title: colouring.title().to_owned(),
            about: colouring.about().to_owned(),
        })
        .collect()
}

/// How the DJ has chosen to colour the spectral balance. §110's light unless
/// they chose otherwise.
#[tauri::command]
#[must_use]
pub fn waveform_colouring(state: State<'_, AppState>) -> String {
    state.waveform_colouring().slug().to_owned()
}

/// Choose how the spectral balance is coloured, and keep it.
///
/// # Errors
/// When the word is not a colouring djmanzo draws.
#[tauri::command]
pub fn set_waveform_colouring(
    state: State<'_, AppState>,
    colouring: String,
) -> Result<String, String> {
    let chosen = dj_render::Colouring::from_slug(&colouring)
        .ok_or_else(|| format!("djmanzo does not colour the waveform {colouring:?}"))?;
    state.set_waveform_colouring(chosen);
    Ok(chosen.slug().to_owned())
}

/// One of §16's knowledge packs, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KnowledgePackDto {
    pub id: String,
    pub title: String,
    pub about: String,
    /// The genre families it turns on. Empty means all of them.
    pub families: Vec<String>,
    /// The hardest move it will send a DJ to learn.
    pub ceiling: String,
    /// The occasion whose presentation it pairs with, or empty.
    pub setting: String,
    /// How many of the catalogue's moves it would teach — the number that says
    /// what choosing it costs, which is the thing a picker should show.
    pub teaches: usize,
}

/// §16's packs, and which one is chosen.
#[tauri::command]
#[must_use]
pub fn knowledge_packs() -> Vec<KnowledgePackDto> {
    dj_assistant::pack::ALL
        .iter()
        .map(|pack| KnowledgePackDto {
            id: pack.id.to_owned(),
            title: pack.title.to_owned(),
            about: pack.about.to_owned(),
            families: pack.families.iter().map(|f| (*f).to_owned()).collect(),
            ceiling: format!("{:?}", pack.ceiling).to_lowercase(),
            setting: pack.setting.unwrap_or_default().to_owned(),
            teaches: dj_assistant::technique::catalogue()
                .iter()
                .filter(|t| pack.teaches_move(t))
                .count(),
        })
        .collect()
}

/// The pack the DJ has chosen, or empty for all of djmanzo's knowledge.
#[tauri::command]
#[must_use]
pub fn chosen_pack(state: State<'_, AppState>) -> String {
    state
        .chosen_pack()
        .filter(|id| dj_assistant::pack::pack(id).is_some())
        .unwrap_or_default()
}

/// Choose a pack, and take back what will actually be used.
///
/// An empty string means "all of it", which is a real choice rather than a
/// cleared setting: a DJ who plays everything wants the whole catalogue, and
/// the coach teaching inside no pack is what that means.
#[tauri::command]
#[must_use]
pub fn set_chosen_pack(state: State<'_, AppState>, pack: String) -> String {
    // A slug this build does not have is dropped rather than stored, the same
    // round trip every other picker here makes.
    let kept = dj_assistant::pack::pack(&pack)
        .map(|p| p.id.to_owned())
        .unwrap_or_default();
    state.set_chosen_pack(&kept);
    kept
}

/// Where this deck's phrase boundaries fall, as two numbers.
///
/// §75 asks for phrase boundaries to be exposed and, where it maps to a genuine
/// action, to be draggable. The drawing is the interface's — it knows the
/// window, the zoom and the pixels — and the arithmetic is Rust's, because the
/// grid anchor is not on the snapshot and putting it there would be a field
/// every consumer of the snapshot pays for so that one overlay can multiply.
///
/// Two numbers is the whole answer: the first boundary at or after frame zero,
/// and the distance between them. Every other boundary is `first + n * spacing`,
/// which the interface can do per frame without asking again.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PhraseGridDto {
    /// The first phrase boundary at or after the start of the record.
    pub first_frame: f64,
    /// Frames between boundaries — the phrase length in beats, in frames.
    pub spacing_frames: f64,
}

/// One entry on a contextual menu: what a DJ reads, and what djmanzo does.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoveDto {
    pub label: String,
    /// Exactly the text `Action::parse` takes, so the interface dispatches it
    /// rather than mapping it to a command of its own.
    pub action: String,
}

/// §26's *beat jump: contextual action* — what can be jumped from here.
///
/// The one item on §26's list that is not a drag, because a beat jump has no
/// position to grab: it is a move made to a record rather than a mark on one.
///
/// Empty is a real answer and the interface draws no menu for it: a deck with
/// no record, or one djmanzo cannot count beats in, has no jump to offer. See
/// [`crate::jumps`] for why the list changes with where the playhead is.
#[tauri::command]
#[must_use]
pub fn waveform_moves(state: State<'_, AppState>, deck: u8) -> Vec<MoveDto> {
    let Some(id) = DeckId::from_human(deck) else {
        return Vec::new();
    };
    let Some(overlay) = state.waveforms().grid(deck) else {
        return Vec::new();
    };
    let registry = state.registry();
    let get = |p| registry.get(dj_core::ParamId::Deck(id, p));
    let position = f64::from(get(dj_core::param::DeckParam::Position));
    let length = f64::from(get(dj_core::param::DeckParam::LengthFrames));

    let phrase = overlay.phrase.or_else(|| analysed_phrase(&state, id));
    crate::jumps::from_here(
        deck,
        position,
        length,
        overlay.grid.bpm.beat_frames(overlay.sample_rate),
        phrase.is_some(),
        phrase.map_or(0, |p| p.beats),
    )
    .into_iter()
    .map(|option| MoveDto {
        label: option.label,
        action: option.action,
    })
    .collect()
}

/// Where the phrase boundaries are, or `None` when this record has no phrase
/// structure — which is a real answer rather than a gap.
#[tauri::command]
#[must_use]
pub fn phrase_grid(state: State<'_, AppState>, deck: u8) -> Option<PhraseGridDto> {
    let id = DeckId::from_human(deck)?;
    let overlay = state.waveforms().grid(deck)?;
    let phrase = overlay.phrase.or_else(|| analysed_phrase(&state, id))?;
    let beat_frames = overlay.grid.bpm.beat_frames(overlay.sample_rate);
    if !beat_frames.is_finite() || beat_frames <= 0.0 {
        return None;
    }
    let spacing_frames = beat_frames * f64::from(phrase.beats);
    // The boundary the *phrase* anchor names, brought back to the first one at
    // or after zero. A grid anchored mid-record puts its first boundary a long
    // way negative otherwise, and the interface would draw from there.
    let anchored = overlay.grid.anchor.get() + beat_frames * f64::from(phrase.anchor);
    let first_frame = anchored - spacing_frames * (anchored / spacing_frames).floor();
    Some(PhraseGridDto {
        first_frame,
        spacing_frames,
    })
}

/// §53: what the controller now open puts under the DJ's hands.
///
/// `None` when nothing is open, which is a different answer from "a controller
/// that reaches nothing" and has to stay so: the interface adapts to a
/// controller's *gaps*, and a laptop-only DJ has no gaps to fill — they are
/// already the case the interface is designed around.
#[tauri::command]
#[must_use]
pub fn controller_hands(state: State<'_, AppState>) -> Option<dj_hid::hands::Hands> {
    let control = state.control();
    let open = control.status(None).open_mapping?;
    control
        .mappings()
        .into_iter()
        .find(|mapping| mapping.name == open)
        .map(|mapping| mapping.hands)
}

/// §53's other direction: what djmanzo is lighting on the open controller.
///
/// The interface adapting to a controller is half of §53. The other half is
/// the controller showing what the interface knows, and until this it could
/// not: a mapping's `[[feedback]]` blocks were parsed, every parameter name in
/// them resolved, and no byte ever left the machine.
///
/// Both halves of the answer, because a dark board has three different causes
/// with three different answers — a mapping that declares no lights, a device
/// with no MIDI output, and an output another application already holds — and
/// "nothing lit" is the same picture for all of them.
#[tauri::command]
#[must_use]
pub fn controller_lights(state: State<'_, AppState>) -> crate::control::LightsDto {
    state.control().lights()
}

/// One of §54's functional presets, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetupDto {
    /// §81's slug for the kind of night.
    pub slug: String,
    /// What a DJ would call it.
    pub title: String,
    /// What kind of night it is, in one line.
    pub about: String,
    /// The arrangement it opens, by the name the workspace picker shows.
    pub workspace: String,
    /// The theme package id, or empty for "leave the theme alone".
    pub theme: String,
    /// Everything it would change, in the DJ's own words — shown *before* it is
    /// applied as well as after, because a preset that silently changes six
    /// systems is the kind of feature people stop trusting.
    pub changes: Vec<String>,
}

fn setup_dto(setup: crate::setup::Setup) -> SetupDto {
    SetupDto {
        slug: setup.setting.slug().to_owned(),
        title: setup.setting.title().to_owned(),
        about: setup.setting.about().to_owned(),
        workspace: setup.workspace.to_owned(),
        theme: setup.theme.to_owned(),
        changes: setup.changes(),
    }
}

/// §54's presets, one per kind of night.
///
/// Starting points rather than identities: every control one of these touches
/// stays where a DJ can change it, which is §7's rule about the workspaces
/// these name and applies to the rest of the systems too.
#[tauri::command]
#[must_use]
pub fn setups() -> Vec<SetupDto> {
    crate::setup::ALL.into_iter().map(setup_dto).collect()
}

/// What one setup actually did.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetupApplied {
    /// The arrangement the interface should open. Rust does not apply this
    /// itself: the cockpit is resolved against the window, which only the
    /// interface has measured.
    pub workspace: String,
    /// The theme to wear, or empty to leave it.
    pub theme: String,
    /// Every change, including the ones already made here.
    pub changes: Vec<String>,
}

/// Set the night up.
///
/// Rust does what Rust owns — §25's layers, the pad pages, the assistant's
/// posture, whether the room may ask for records — and hands back the two the
/// interface owns. That split is not arbitrary: a workspace is resolved against
/// a measured window and a theme is a stylesheet, and both would be Rust
/// guessing at pixels.
///
/// Refuses rather than falls back on a slug it does not know, for the reason
/// `Setting::parse` gives: the whole point of §81 is that the profiles are kept
/// apart, and a wedding quietly becoming a club is worse than an error.
#[tauri::command]
pub fn apply_setup(state: State<'_, AppState>, setting: String) -> Result<SetupApplied, String> {
    let which = crate::setting::Setting::parse(&setting)
        .ok_or_else(|| format!("{setting:?} is not a kind of night djmanzo knows"))?;
    let setup = crate::setup::setup(which);

    let layers: Vec<String> = setup.layers.iter().map(|l| (*l).to_owned()).collect();
    state.set_waveform_layers(
        &dj_render::choosing(&layers)
            .into_iter()
            .map(|layer| layer.name.to_owned())
            .collect::<Vec<_>>(),
    );
    let pages: Vec<String> = setup.pages.iter().map(|p| (*p).to_owned()).collect();
    state.set_favourite_pad_pages(&keeping_pages(&pages));

    if let Ok(mut guard) = state.conduct().lock() {
        guard.posture = setup.posture;
    }
    state.audience().front().set_open(setup.requests);
    // §16's pack, for the occasions where exactly one names them. Derived from
    // the pack table rather than written into the setup, so a pack added for a
    // new kind of night is wired in by existing — and the presets that leave it
    // alone leave the whole catalogue on, which is what a DJ who has chosen no
    // pack already has.
    if let Some(pack) = setup.pack() {
        state.set_chosen_pack(pack.id);
    }

    Ok(SetupApplied {
        workspace: setup.workspace.to_owned(),
        theme: setup.theme.to_owned(),
        changes: setup.changes(),
    })
}

/// One of §8 Level 1's nine, and whether djmanzo actually keeps it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RememberedDto {
    pub slug: String,
    /// What it is, in the DJ's words.
    pub about: String,
    /// What losing it would cost.
    pub forgotten: String,
    /// Whether djmanzo keeps it at all.
    pub kept: bool,
    /// Why not, for the rows it does not. Empty for the rows it does.
    pub why_not: String,
}

/// What djmanzo remembers about you, and what it does not.
///
/// §8 Level 1's own list, read off `crate::remembered` rather than written out
/// in the interface, so a row cannot claim something no file backs — a test
/// checks every claim against `state.rs`. The row djmanzo does *not* keep is
/// on the list saying so: a list of eight would read as the whole of §8, and a
/// preference that is silently forgotten looks exactly like one never set.
#[tauri::command]
#[must_use]
pub fn remembered() -> Vec<RememberedDto> {
    crate::remembered::Remembered::ALL
        .iter()
        .map(|entry| RememberedDto {
            slug: entry.name().to_owned(),
            about: entry.about().to_owned(),
            forgotten: entry.forgotten().to_owned(),
            kept: entry.kept(),
            why_not: entry.why_not().to_owned(),
        })
        .collect()
}

/// The arrangements the DJ has saved under names of their own.
///
/// Separate from [`cockpit_workspaces`], which is what djmanzo ships. The picker
/// shows both and says which is which: a DJ looking for the layout they built
/// for their Saturday residency should not have to pick it out of twenty-three
/// they have never opened.
#[tauri::command]
#[must_use]
pub fn my_workspaces(state: State<'_, AppState>) -> Vec<crate::cockpit::Workspace> {
    state.my_workspaces()
}

/// Save the arrangement on screen under a name.
///
/// §7 calls the shipped arrangements *starting points, not rigid identities*,
/// and that was only half true: a DJ could pick one and move what they liked,
/// and the edit survived a restart — but it could not be **named**, so a DJ with
/// a wedding layout and a club layout had one of them and a memory of the other.
/// §103's *Modularity* criterion is the same gap in other words.
///
/// The workspace is resolved first, so what is kept is what can actually be
/// drawn rather than what was asked for — the same round trip
/// [`set_cockpit_workspace`] makes, and for the same reason.
///
/// # Errors
/// When the name is empty or is one djmanzo ships. See `cockpit::keep`.
#[tauri::command]
pub fn keep_workspace(
    state: State<'_, AppState>,
    name: String,
    workspace: crate::cockpit::Workspace,
) -> Result<Vec<crate::cockpit::Workspace>, String> {
    let resolved = crate::cockpit::resolve(&workspace);
    let kept = crate::cockpit::keep(
        &state.my_workspaces(),
        &crate::cockpit::workspaces(),
        &resolved.workspace,
        &name,
    )?;
    state.set_my_workspaces(&kept);

    // And the cockpit is now *in* that arrangement, which is the half driving
    // it found missing. Without this the picker read the new name until the
    // next restart and then went back to the shipped one it was saved from —
    // the DJ's own arrangement was in the list, and the application did not
    // think it was wearing it. Naming what is on screen means what is on screen
    // is now called that.
    if let Some(saved) = kept
        .iter()
        .find(|held| held.name.trim().eq_ignore_ascii_case(name.trim()))
    {
        state.set_workspace(saved);
    }
    Ok(kept)
}

/// Take one of the DJ's own arrangements out of the collection.
#[tauri::command]
#[must_use]
pub fn forget_workspace(
    state: State<'_, AppState>,
    name: String,
) -> Vec<crate::cockpit::Workspace> {
    let kept = crate::cockpit::forget(&state.my_workspaces(), &name);
    state.set_my_workspaces(&kept);
    kept
}

/// One of §48's seven costs, and what djmanzo does about it under load.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpendDto {
    /// §48's own words for it.
    pub what: String,
    /// What a DJ would notice going.
    pub about: String,
    /// `audio`, `control` or `visual`.
    pub band: String,
    /// Whether djmanzo still pays for this at the tier asked about.
    pub paid: bool,
    /// Why it is never given up, or why djmanzo does not give it up. Empty for
    /// the rows that simply are given up.
    pub why_not: String,
}

/// What djmanzo gives up on a machine at this tier, and what it never gives up.
///
/// §48's priority is `AUDIO > CONTROL > VISUAL EFFECTS`, and the whole of this
/// command is making that sayable: the frame rate has been measured for a long
/// time and what consulted it was the theme pipeline and nothing else. An
/// unknown tier is read as the healthiest, because a machine djmanzo cannot
/// place is not one to start taking things away from.
#[tauri::command]
#[must_use]
pub fn under_load(tier: String) -> Vec<SpendDto> {
    let tier = crate::thrift::Tier::parse(&tier).unwrap_or(crate::thrift::Tier::Ultra);
    crate::thrift::Spend::ALL
        .iter()
        .map(|spend| SpendDto {
            what: spend.what.to_owned(),
            about: spend.about.to_owned(),
            band: spend.band.name().to_owned(),
            paid: spend.paid_at(tier),
            why_not: spend.why_not.to_owned(),
        })
        .collect()
}

/// How often the room may be read at this tier, in milliseconds.
///
/// §48's *reduce audience polling frequency*. One number rather than a rule the
/// panel works out for itself: a second copy of "two seconds, or eight when
/// struggling" is how the two come to disagree, and the priority decision
/// belongs beside the rest of §48's table.
#[tauri::command]
#[must_use]
pub fn room_poll_ms(tier: String) -> u32 {
    crate::thrift::room_poll_ms(
        crate::thrift::Tier::parse(&tier).unwrap_or(crate::thrift::Tier::Ultra),
    )
}

/// One of §8's seven adaptation levels, as the picker offers it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LevelDto {
    /// §8's own number, which is what a DJ will call it.
    pub number: u8,
    /// The slug it is stored and chosen by.
    pub slug: String,
    /// §8's own name for it.
    pub title: String,
    /// What choosing it means, in the DJ's words.
    pub about: String,
    /// The §10 posture it sets.
    pub posture: String,
    /// Whether preferences survive a restart at this level.
    pub remembers: bool,
    /// Whether the interface may change itself at this level.
    pub adapts: bool,
}

/// Where djmanzo stands on §8's axis, and what no longer matches it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StandingDto {
    /// The level last set, or empty if a DJ never has.
    ///
    /// Empty rather than a default: "never chosen" and "chose Static" are
    /// different, and a fresh install reporting Level 0 would be describing
    /// itself wrongly — djmanzo's shipped behaviour is not Static.
    pub level: String,
    /// What about the current state departs from that level, in the DJ's words.
    ///
    /// Empty when nothing does. A level is a starting point and a DJ may move
    /// any control it set, so djmanzo's job afterwards is to say what changed
    /// rather than to spring it back.
    pub departures: Vec<String>,
    /// §79's locks now in force, by slug.
    ///
    /// Handed back because setting a level writes them, and the interface holds
    /// the workspace: without this the six checkboxes below the axis would go
    /// on showing what they showed a second ago, which is the state djmanzo was
    /// in before the press. Read off the workspace rather than re-derived from
    /// the level, so a DJ who has since unlocked one sees that.
    pub locked: Vec<String>,
}

/// One of §40's twenty-six, for the panel.
#[derive(Debug, Clone, Serialize)]
pub struct SightDto {
    /// §40's own word for it.
    pub name: String,
    pub about: String,
    /// Whether the model is told this.
    pub told: bool,
    /// Where it comes from, or why it does not come at all. Never empty.
    pub source: String,
}

/// §40's list, and which half of it the assistant actually sees.
///
/// Answered by Rust rather than typed into the panel because the *unseen* half
/// is the part a DJ needs and the part nobody would keep true by hand. A panel
/// listing what the assistant knows is a reassurance; a panel listing what it
/// could not see is the thing that makes an answer arguable.
#[tauri::command]
#[must_use]
pub fn assistant_sight() -> Vec<SightDto> {
    crate::sight::ALL
        .iter()
        .map(|item| SightDto {
            name: item.name.to_owned(),
            about: item.about.to_owned(),
            told: item.carrier.told(),
            source: match item.carrier {
                crate::sight::Carrier::Deck { .. } => "each deck".to_owned(),
                crate::sight::Carrier::Master { .. } => "the mixer".to_owned(),
                crate::sight::Carrier::Context { .. } => "the night".to_owned(),
                crate::sight::Carrier::Beside { held } => match held {
                    crate::sight::Held::Posture | crate::sight::Held::Occasion => {
                        "what you set".to_owned()
                    }
                    crate::sight::Held::History => "tonight so far".to_owned(),
                    crate::sight::Held::Recent => "what you just did".to_owned(),
                    crate::sight::Held::Hardware => "what is plugged in".to_owned(),
                    crate::sight::Held::Focus => "what is on screen".to_owned(),
                    crate::sight::Held::Staged => "what it has prepared".to_owned(),
                    crate::sight::Held::Candidates => "the next-track rail".to_owned(),
                    crate::sight::Held::Plan => "your set plan".to_owned(),
                    crate::sight::Held::Profile => "your nights like this one".to_owned(),
                    crate::sight::Held::Transition => "the armed mix".to_owned(),
                },
                crate::sight::Carrier::Unseen { because } => because.to_owned(),
            },
        })
        .collect()
}

/// §8's seven, listed by Rust.
#[tauri::command]
#[must_use]
pub fn adaptation_levels() -> Vec<LevelDto> {
    crate::level::Level::ALL
        .iter()
        .map(|level| LevelDto {
            number: level.number(),
            slug: level.slug().to_owned(),
            title: level.title().to_owned(),
            about: level.about().to_owned(),
            posture: level.posture().name().to_owned(),
            remembers: level.remembers(),
            adapts: level.permits() == crate::cockpit::Permits::everything(),
        })
        .collect()
}

/// Where djmanzo stands, and what has drifted from it.
///
/// # Errors
/// When the conduct lock is poisoned.
#[tauri::command]
pub fn standing(state: State<'_, AppState>) -> Result<StandingDto, String> {
    standing_of(&state)
}

/// Where djmanzo stands, off a plain reference.
///
/// Split from the command so a test can reach it. Tauri's `State` cannot be
/// built in a unit test, and the rule this file already follows is that the
/// command is the thin half: `open_device_for` and `set_stem_out_for_test` are
/// the same split.
///
/// # Errors
/// When the conduct lock is poisoned.
pub fn standing_of(state: &AppState) -> Result<StandingDto, String> {
    let stored = state.adaptation_level().unwrap_or_default();
    let Some(level) = crate::level::Level::parse(&stored) else {
        // A level nobody recognises, or none set. Either way there is nothing
        // to be a departure *from*, and inventing one would be djmanzo telling
        // a DJ they had drifted from a decision they never made.
        return Ok(StandingDto {
            level: String::new(),
            departures: Vec::new(),
            locked: locks_now(state),
        });
    };
    let posture = state
        .conduct()
        .lock()
        .map_err(|_| "the conduct lock is poisoned".to_owned())?
        .posture;
    let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
    let permits = crate::cockpit::resolve(&stored).permits;
    Ok(StandingDto {
        level: level.slug().to_owned(),
        departures: level
            .departures(posture, permits)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        locked: locks_now(state),
    })
}

/// §79's locks currently on the workspace, by slug.
fn locks_now(state: &AppState) -> Vec<String> {
    state
        .workspace()
        .unwrap_or_else(crate::cockpit::opening)
        .locked
        .iter()
        .map(|lock| lock.name().to_owned())
        .collect()
}

/// Set §8's level, which sets the posture and §79's locks together.
///
/// **The one control §8 asks for.** Everything it touches stays where a DJ can
/// change it afterwards — the same contract §7's arrangements and §54's setups
/// have — and `standing` is what says so when they do. A single axis that
/// *owned* six switches would be the axis arguing with the switches, and §79's
/// panel would be a row of controls that silently sprang back.
///
/// # Errors
/// A slug djmanzo does not have, or a poisoned lock.
#[tauri::command]
pub fn set_adaptation_level(
    state: State<'_, AppState>,
    level: String,
) -> Result<StandingDto, String> {
    set_adaptation_level_of(&state, &level)
}

/// Set §8's level, off a plain reference. See [`standing_of`] for the split.
///
/// # Errors
/// A slug djmanzo does not have, or a poisoned lock.
pub fn set_adaptation_level_of(state: &AppState, level: &str) -> Result<StandingDto, String> {
    let chosen = crate::level::Level::parse(level)
        .ok_or_else(|| format!("{level:?} is not one of §8's levels"))?;

    state
        .conduct()
        .lock()
        .map_err(|_| "the conduct lock is poisoned".to_owned())?
        .posture = chosen.posture();

    // §79's six, from §78's four. The locks are the workspace's, so this is one
    // write to the workspace rather than six — and `Lock::stops` is what maps
    // a freedom back to the locks that take it away, so a seventh lock added
    // there is honoured here without anybody remembering to come back.
    let mut workspace = state.workspace().unwrap_or_else(crate::cockpit::opening);
    let permits = chosen.permits();
    workspace.locked = crate::cockpit::Lock::ALL
        .iter()
        .copied()
        .filter(|lock| !permits.allows(lock.stops()))
        .collect();
    state.set_workspace(&crate::cockpit::resolve(&workspace).workspace);

    state.set_adaptation_level(chosen.slug());
    standing_of(state)
}

/// One of §79's locks, and what a DJ is told it takes away.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LockDto {
    /// The slug stored in the workspace.
    pub slug: String,
    /// The sentence beside the switch.
    pub about: String,
}

/// The six things a DJ may lock, in §79's own order.
///
/// Listed by Rust rather than typed into the panel, for the reason every other
/// table in this file is: a seventh lock added here should appear in Settings
/// without anybody remembering to add it, and a lock whose sentence is written
/// in two places is a lock that ends up meaning two things.
#[tauri::command]
#[must_use]
pub fn cockpit_locks() -> Vec<LockDto> {
    crate::cockpit::Lock::ALL
        .iter()
        .map(|lock| LockDto {
            slug: lock.name().to_owned(),
            about: lock.about().to_owned(),
        })
        .collect()
}

/// The waveform's semantic layers — §25's twenty, and which of them exist.
///
/// Handed to the interface so that what is drawn, what is *named* and what can
/// be *turned off* come from one table. A browser test checks the other
/// direction: everything on screen carries a `data-layer` that is in this list,
/// so a layer drawn without being declared fails rather than quietly becoming a
/// twenty-first.
#[tauri::command]
#[must_use]
pub fn waveform_layers() -> Vec<LayerChoiceDto> {
    layer_choices()
}

// -- the typed UI vocabulary -------------------------------------------------
//
// §41. See `crate::uiop` for why this is a second closed vocabulary rather than
// more verbs on the action bus, and why density is deliberately not in it.

/// Every interface operation this build accepts, as lines a model can be shown.
///
/// Generated from the surfaces that exist, so it cannot offer a panel djmanzo
/// does not have — the same guarantee `dj_core::vocabulary` gives the action
/// bus, one layer up.
#[tauri::command]
#[must_use]
pub fn ui_vocabulary(state: State<'_, AppState>) -> Vec<String> {
    crate::uiop::as_prompt_lines(u8::try_from(state.deck_count()).unwrap_or(4))
}

/// Carry out one interface operation.
///
/// The DJ's own path — a palette entry, a button — and so ungated: a person
/// asking for a panel is not something to check a matrix about. The assistant's
/// path is [`ui_request`], which asks §72 first.
#[tauri::command]
pub fn ui_do(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    op: String,
) -> Result<crate::uiop::Applied, String> {
    let op = crate::uiop::UiOp::parse(&op).map_err(|error| error.to_string())?;
    Ok(carry_out_ui(&app, &state, &op))
}

/// Carry out one interface operation **on the assistant's behalf**.
///
/// Asks §72's matrix first, because rearranging a DJ's screen mid-set is
/// exactly the kind of thing they may want the machine to stay out of. It is
/// its own capability row (`adapt_layout`) rather than folded into another,
/// which is what lets "suggest records but never touch my layout" be a setting
/// rather than a feature request.
///
/// # Errors
/// When the operation is not in the vocabulary, or the posture refuses it.
pub fn ui_request(
    app: &tauri::AppHandle,
    state: &AppState,
    op: &crate::uiop::UiOp,
) -> Result<crate::uiop::Applied, String> {
    // Read once and used twice: the check and the apply see the same
    // arrangement, so a workspace written between them cannot let an operation
    // through that the DJ had just locked out.
    let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
    assistant_may_rearrange(state, &stored)?;
    Ok(carry_out(app, state, op, stored))
}

/// Whether the assistant may move anything at all right now.
///
/// Two questions with two different answers, and neither substitutes for the
/// other:
///
/// - **§72's matrix.** What the machine is allowed to do at this posture. Its
///   own capability row, `adapt_layout`, which is what lets "suggest records but
///   never touch my layout" be a setting rather than a feature request.
/// - **§78 and §79's locks.** What the *DJ* has pinned for the night. This beats
///   every posture including autopilot: §78 calls it the professional safety
///   valve, and a valve with an override is not one.
///
/// The lock has to be answered **here** rather than in the interface. This path
/// applies the workspace in Rust, stores it, and *then* tells the window — so a
/// permit consulted in `App.svelte` would be consulted after the arrangement had
/// already changed and been written to disk.
///
/// # Errors
/// A sentence saying which of the two refused, because a DJ whose assistant has
/// gone quiet needs to know whether to change the posture or clear a lock.
fn assistant_may_rearrange(
    state: &AppState,
    workspace: &crate::cockpit::Workspace,
) -> Result<(), String> {
    let posture = state
        .conduct()
        .lock()
        .map(|guard| guard.posture)
        .unwrap_or_default();
    let allowed = state
        .conduct()
        .lock()
        .map(|guard| {
            guard
                .authority
                .allows(dj_assistant::Capability::AdaptLayout, posture)
        })
        .unwrap_or_default();
    if !allowed.permits() {
        return Err(format!(
            "the assistant may not rearrange the interface at {}",
            posture.name()
        ));
    }
    if !workspace.permits().rearrange {
        return Err("the layout is locked -- unlock it in Settings".to_owned());
    }
    Ok(())
}

/// Apply, store and announce. The one place an operation actually lands.
fn carry_out_ui(
    app: &tauri::AppHandle,
    state: &AppState,
    op: &crate::uiop::UiOp,
) -> crate::uiop::Applied {
    let stored = state.workspace().unwrap_or_else(crate::cockpit::opening);
    carry_out(app, state, op, stored)
}

/// As [`carry_out_ui`], against an arrangement the caller has already read.
fn carry_out(
    app: &tauri::AppHandle,
    state: &AppState,
    op: &crate::uiop::UiOp,
    stored: crate::cockpit::Workspace,
) -> crate::uiop::Applied {
    let applied = crate::uiop::apply(op, &stored);
    state.set_workspace(&applied.workspace.workspace);
    // Announced rather than returned only, because the interesting caller is
    // the assistant: a panel that opened because the machine asked for it has
    // to appear without the DJ having pressed anything.
    use tauri::Emitter as _;
    let _ = app.emit("cockpit", &applied);
    applied
}

// ---------------------------------------------------------------- controllers

/// What is plugged in and what is listening to it.
#[tauri::command]
#[must_use]
pub fn control_status(state: State<'_, AppState>) -> crate::control::ControlStatus {
    // The device's channel count, so a controller asking for outputs the
    // device does not have is reported as not applied rather than as in force.
    let channels = state
        .active_device()
        .map(|device| usize::from(device.channels));
    state.control().status(channels)
}

/// Every mapping that can be opened.
#[tauri::command]
#[must_use]
pub fn control_mappings(state: State<'_, AppState>) -> Vec<crate::control::MappingDto> {
    state.control().mappings()
}

// -- the mapping editor ---------------------------------------------------

/// What a control should do, as the interface describes it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleDto {
    Latching {
        press: String,
    },
    Momentary {
        press: String,
        release: String,
    },
    Continuous {
        action: String,
        min: Option<f32>,
        max: Option<f32>,
    },
    Encoder {
        up: String,
        down: String,
        encoding: String,
    },
}

impl From<RoleDto> for dj_hid::editor::Role {
    fn from(dto: RoleDto) -> Self {
        use dj_hid::editor::Role;
        match dto {
            RoleDto::Latching { press } => Role::Latching { press },
            RoleDto::Momentary { press, release } => Role::Momentary { press, release },
            RoleDto::Continuous { action, min, max } => Role::Continuous { action, min, max },
            RoleDto::Encoder { up, down, encoding } => Role::Encoder {
                up,
                down,
                // An unknown convention falls back to the common one rather
                // than refusing: the encoding is a hint about hardware, and a
                // mapping that would not save over a typo here is worse than
                // one whose encoder turns the wrong way and can be corrected.
                encoding: match encoding.as_str() {
                    "offset" => dj_hid::mapping::Encoding::Offset,
                    "absolute" => dj_hid::mapping::Encoding::Absolute,
                    _ => dj_hid::mapping::Encoding::Signed,
                },
            },
        }
    }
}

/// One control in a draft, for the interface to list.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftBindingDto {
    pub on: String,
    /// What it does, in the action grammar, for showing back to the DJ.
    pub does: String,
}

/// The draft as the interface sees it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftDto {
    pub name: String,
    pub device: String,
    pub bindings: Vec<DraftBindingDto>,
    /// Whether the port is describing controls rather than acting on them.
    pub learning: bool,
    /// The last control touched while learning.
    pub learned: Option<String>,
}

fn draft_dto(state: &AppState) -> DraftDto {
    let draft = state.mapping_draft();
    DraftDto {
        name: draft.name.clone(),
        device: draft.device.clone(),
        bindings: draft
            .bindings()
            .iter()
            .map(|b| DraftBindingDto {
                on: b.on.clone(),
                does: [&b.press, &b.moved, &b.turn_up]
                    .into_iter()
                    .flatten()
                    .next()
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
        learning: state.control().is_learning(),
        learned: state.control().learned(),
    }
}

/// Start describing controls instead of acting on them.
#[tauri::command]
pub fn mapping_learn(state: State<'_, AppState>, on: bool) -> DraftDto {
    if on {
        state.control().start_learning();
    } else {
        state.control().stop_learning();
    }
    draft_dto(&state)
}

/// The draft as it stands, including whatever control was last touched.
#[tauri::command]
#[must_use]
pub fn mapping_draft(state: State<'_, AppState>) -> DraftDto {
    draft_dto(&state)
}

/// Name the mapping being built.
#[tauri::command]
pub fn mapping_rename(state: State<'_, AppState>, name: String, device: String) -> DraftDto {
    state.edit_mapping_draft(|draft| {
        draft.name = name;
        draft.device = device;
    });
    draft_dto(&state)
}

/// Give a control a job.
///
/// # Errors
/// If the control or the action would not parse -- reported now, while the DJ
/// is still looking at the control they pressed.
#[tauri::command]
pub fn mapping_bind(
    state: State<'_, AppState>,
    on: String,
    role: RoleDto,
) -> Result<DraftDto, String> {
    let role: dj_hid::editor::Role = role.into();
    state
        .edit_mapping_draft(|draft| draft.bind(&on, &role))
        .ok_or_else(|| "the mapping editor is unavailable".to_owned())?
        .map_err(|e| e.to_string())?;
    state.control().forget_learned();
    Ok(draft_dto(&state))
}

/// Take a control's job away.
#[tauri::command]
pub fn mapping_unbind(state: State<'_, AppState>, on: String) -> DraftDto {
    state.edit_mapping_draft(|draft| draft.unbind(&on));
    draft_dto(&state)
}

/// Start again from nothing, or from a mapping that already exists.
#[tauri::command]
pub fn mapping_draft_from(state: State<'_, AppState>, name: Option<String>) -> DraftDto {
    state.start_mapping_draft(name.as_deref());
    draft_dto(&state)
}

/// Write the draft into the user's mappings directory.
///
/// # Errors
/// If there is nowhere to write, or the draft would not reload.
#[tauri::command]
pub fn mapping_save(state: State<'_, AppState>) -> Result<String, String> {
    let dir = state
        .mappings_dir()
        .ok_or_else(|| "there is nowhere to save mappings yet".to_owned())?;
    let draft = state.mapping_draft();
    let path = state.control().save_mapping(&dir, &draft)?;
    Ok(path.display().to_string())
}

/// The keyboard, as a shortcut sheet.
///
/// The interface asks for this once and does the lookup itself, rather than
/// sending every key press to the backend to be translated. A key press has to
/// feel instant, and a round trip through the bridge for a key that turns out
/// not to be bound is a round trip for nothing.
#[tauri::command]
#[must_use]
pub fn keyboard_keys(state: State<'_, AppState>) -> Vec<crate::control::KeyDto> {
    state.control().keys()
}

/// Turn the keyboard on or off.
#[tauri::command]
pub fn set_keyboard_enabled(state: State<'_, AppState>, on: bool) {
    state.control().set_keyboard(on);
}

/// Open a MIDI input with a mapping. `mapping` unset means "whichever fits".
///
/// # Errors
/// When no mapping matches, or the port cannot be opened.
#[tauri::command]
pub fn open_controller(
    state: State<'_, AppState>,
    port: String,
    mapping: Option<String>,
) -> Result<(), String> {
    state.control().open(&port, mapping.as_deref())?;
    // A controller with its own soundcard usually states where its sockets go.
    // Applied here rather than left in Settings because a DJ plugging in
    // mid-set has no time to find it, and the failure it prevents -- the room
    // hearing the cue -- is the loudest kind.
    state.apply_controller_routing();
    Ok(())
}

/// Open a HID device with a named mapping.
///
/// The mapping is required, unlike the MIDI path. A HID mapping states byte
/// offsets into a report, and applying one device's offsets to another's
/// packets would not fail -- it would bind the crossfader to a button.
///
/// # Errors
/// When no such mapping exists, when it is a MIDI mapping, or when the device
/// cannot be opened.
#[tauri::command]
pub fn open_hid_controller(
    state: State<'_, AppState>,
    device: String,
    mapping: String,
) -> Result<(), String> {
    state.control().open_hid(&device, &mapping)?;
    state.apply_controller_routing();
    Ok(())
}

/// Close whatever HID device is open.
#[tauri::command]
pub fn close_hid_controller(state: State<'_, AppState>) {
    state.control().close_hid();
    state.apply_controller_routing();
}

/// Every MIDI output the machine can see, and why it can see none.
#[tauri::command]
#[must_use]
pub fn midi_outputs() -> MidiOutputsDto {
    match dj_hid::out::outputs() {
        Ok(ports) => MidiOutputsDto {
            ports,
            unavailable: None,
        },
        Err(e) => MidiOutputsDto {
            ports: Vec::new(),
            unavailable: Some(e.to_string()),
        },
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MidiOutputsDto {
    pub ports: Vec<String>,
    /// Said out loud, because "no outputs" and "no MIDI on this machine" are
    /// different problems and only one is fixed by plugging something in.
    pub unavailable: Option<String>,
}

/// Whether djmanzo is sending MIDI clock, and where.
#[tauri::command]
#[must_use]
pub fn clock_status(state: State<'_, AppState>) -> crate::clock::ClockStatus {
    state.clock().status(state.clock_follow())
}

/// Become the MIDI clock master on `port`.
///
/// # Errors
/// When MIDI is unavailable, no output matches the name, or it refuses.
#[tauri::command]
pub fn start_clock(
    state: State<'_, AppState>,
    port: String,
) -> Result<crate::clock::ClockStatus, String> {
    state.clock().start(&port, state.registry())
}

/// Stop sending clock. The follower is told, rather than left running.
#[tauri::command]
pub fn stop_clock(state: State<'_, AppState>) -> crate::clock::ClockStatus {
    state.clock().stop();
    state.clock().status(state.clock_follow())
}

/// Follow the MIDI clock arriving on `port`.
///
/// While one is being followed it outranks every deck as the sync leader.
///
/// # Errors
/// When MIDI is unavailable, no input matches, or the port refuses.
#[tauri::command]
pub fn follow_clock(
    state: State<'_, AppState>,
    port: String,
) -> Result<crate::clock::ClockStatus, String> {
    state.clock_follow().start(&port, Arc::clone(state.bus()))?;
    Ok(state.clock().status(state.clock_follow()))
}

/// Stop following, and hand the lead back to the decks.
#[tauri::command]
pub fn unfollow_clock(state: State<'_, AppState>) -> crate::clock::ClockStatus {
    state.clock_follow().stop(Some(state.bus()));
    state.clock().status(state.clock_follow())
}

/// What the network control server is doing.
#[tauri::command]
#[must_use]
pub fn remote_status(state: State<'_, AppState>) -> crate::remote::RemoteStatus {
    state.remote().status()
}

/// Open a control port so something else can drive djmanzo.
///
/// `address` is a socket address — `127.0.0.1:7654` for this machine only,
/// `0.0.0.0:7654` to face the network. A token is **required** for the second,
/// and refusing it is `dj_net`'s job rather than this one's, so it cannot be
/// forgotten by a caller.
///
/// # Errors
/// When the address cannot be parsed or bound, or when it faces the network
/// with no token.
#[tauri::command]
pub fn start_remote(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    address: String,
    token: Option<String>,
) -> Result<crate::remote::RemoteStatus, String> {
    let parsed: std::net::SocketAddr = address
        .parse()
        .map_err(|_| format!("{address:?} is not an address and port, like 127.0.0.1:7654"))?;
    state.remote().start(
        parsed,
        token.filter(|t| !t.is_empty()),
        Arc::clone(state.bus()),
        state.registry(),
        crate::remote::booth(app),
    )
}

/// Open an OSC port, so TouchOSC or QLab can drive djmanzo.
///
/// Loopback only. UDP has no handshake, so a token cannot be offered once and
/// remembered — there is nothing to authenticate with, which is why a port
/// facing the network is refused rather than protected badly.
///
/// # Errors
/// When the address cannot be parsed or bound, or is not loopback.
#[tauri::command]
pub fn start_osc(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    address: String,
) -> Result<crate::remote::RemoteStatus, String> {
    let parsed: std::net::SocketAddr = address
        .parse()
        .map_err(|_| format!("{address:?} is not an address and port, like 127.0.0.1:9000"))?;
    state.remote().start_osc(
        parsed,
        Arc::clone(state.bus()),
        state.registry(),
        crate::remote::booth(app),
    )
}

/// Close the OSC port.
#[tauri::command]
pub fn stop_osc(state: State<'_, AppState>) -> crate::remote::RemoteStatus {
    state.remote().stop_osc();
    state.remote().status()
}

/// Close the control port.
#[tauri::command]
pub fn stop_remote(state: State<'_, AppState>) -> crate::remote::RemoteStatus {
    state.remote().stop();
    state.remote().status()
}

/// Close whatever controller is open.
#[tauri::command]
pub fn close_controller(state: State<'_, AppState>) {
    state.control().close();
    // Back to guessing from the channel count, which is right for the built-in
    // output the DJ has just fallen back to.
    state.apply_controller_routing();
}

// -- The room's own page ---------------------------------------------------

/// Open the page the room can reach, and start answering to the local name.
///
/// A port of 0 asks the operating system for a free one, which is useful for a
/// test and useless for a sticker; the interface passes the real default.
#[tauri::command]
pub fn audience_start(
    state: State<'_, AppState>,
    port: Option<u16>,
) -> Result<crate::audience::AudienceStatus, String> {
    state
        .audience()
        .start(port.unwrap_or(dj_net::sticker::DEFAULT_PORT))
}

/// Close the port. What was asked for is kept.
#[tauri::command]
pub fn audience_stop(state: State<'_, AppState>) -> crate::audience::AudienceStatus {
    state.audience().stop();
    state.audience().status()
}

#[tauri::command]
pub fn audience_status(state: State<'_, AppState>) -> crate::audience::AudienceStatus {
    let audience = state.audience();
    // What the page shows as playing, from the poll the interface makes
    // anyway. Blank rather than stale when nothing is loaded.
    let playing = now_playing(&state);
    audience
        .front()
        .set_playing((!playing.is_empty()).then_some(playing));
    audience.status()
}

/// Stop taking requests without taking the page away.
#[tauri::command]
pub fn audience_open(state: State<'_, AppState>, open: bool) -> crate::audience::AudienceStatus {
    state.audience().front().set_open(open);
    state.audience().status()
}

/// The heading, the language, and whether the room is told what is playing.
#[tauri::command]
pub fn audience_settings(
    state: State<'_, AppState>,
    heading: Option<String>,
    language: Option<String>,
    show_playing: Option<bool>,
) -> crate::audience::AudienceStatus {
    let audience = state.audience();
    let front = audience.front();
    if let Some(heading) = heading {
        front.set_heading(&heading);
    }
    if let Some(language) = language {
        front.set_language(&language);
    }
    if let Some(show) = show_playing {
        front.set_show_playing(show);
    }
    audience.status()
}

/// The languages the page is written in.
#[tauri::command]
pub fn audience_languages() -> Vec<(String, String)> {
    crate::audience::Audience::languages()
}

/// Everything still waiting, most-wanted first.
#[tauri::command]
pub fn audience_waiting(state: State<'_, AppState>) -> Vec<crate::audience::AskDto> {
    state.audience().waiting()
}

/// Everything, settled and not, in the order it was asked.
#[tauri::command]
pub fn audience_all(state: State<'_, AppState>) -> Vec<crate::audience::AskDto> {
    state.audience().everything()
}

/// Say what became of a request — `played`, `passed`, or back to `waiting`.
#[tauri::command]
pub fn audience_settle(state: State<'_, AppState>, id: u64, standing: String) -> bool {
    state.audience().settle(id, &standing)
}

/// Write a printable sheet of stickers, and open it.
///
/// A file the DJ chose the place for, rather than a window djmanzo opens.
/// `window.open` from inside the webview is inert here -- it returns something
/// and no window appears, which is the same silent failure `target="_blank"`
/// had -- so the sheet is written where it was asked for and handed to the
/// operating system, which knows how to print an HTML page.
///
/// Opening is best-effort and reported separately: a machine with no browser
/// still has the file, and a sentence saying where it is beats a button that
/// looks like it did nothing.
///
/// # Errors
/// When there is no such way in, or the file cannot be written.
#[tauri::command]
pub fn audience_sheet(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    kind: String,
    copies: Option<usize>,
    path: String,
) -> Result<bool, String> {
    use tauri_plugin_opener::OpenerExt as _;

    let html = state.audience().sheet(&kind, copies.unwrap_or(12))?;
    std::fs::write(&path, html).map_err(|e| format!("could not write {path}: {e}"))?;
    // A path djmanzo just wrote itself, so there is nothing to check it
    // against -- unlike `open_signup_link`, where the URL comes from a catalog.
    Ok(app.opener().open_path(&path, None::<&str>).is_ok())
}

// -- what the room is doing ------------------------------------------------
//
// See `dj_assistant::room` for why every reading is relative to tonight and
// why nothing here ever names a mood.

/// What the sensors have made of the room.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoomDto {
    /// Whether a reading arrived recently enough to call this live.
    pub watching: bool,
    /// How many readings are in the near window.
    pub recent: usize,
    /// Whether there is enough to say anything at all.
    pub enough: bool,
    /// Everything worth saying, most important first. Empty is the normal
    /// state of a room carrying on.
    pub notes: Vec<String>,
    /// Where the room disagrees with the night the DJ set up.
    pub disagreement: Option<String>,
    /// The hour, from the clock rather than from a sensor.
    pub hour: Option<u8>,
    /// The middle of the near window, for a meter rather than a sentence.
    pub light: Option<f32>,
    pub movement: Option<f32>,
    pub loudness: Option<f32>,
    /// §35's baseline: where the room is now against each reach it can be
    /// compared with. One entry per sense that has been measured enough.
    pub baseline: Vec<BaselineDto>,
    /// The phase the comparison against similar phases was made with, or
    /// `null` when the night has not read yet. Said rather than implied — a
    /// panel showing phase comparisons with nothing naming the phase is a
    /// panel making a claim it cannot support.
    pub phase: Option<String>,
}

/// One sense, placed against every reach §35 asks for.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BaselineDto {
    /// `light`, `movement` or `loudness`.
    pub sense: String,
    /// The middle of the last three minutes: §35's *current room activity*.
    pub now: f32,
    /// Each reach and where the room sits against it.
    pub against: Vec<BaselineAgainstDto>,
    /// The sentences this baseline is worth, already worded in Rust.
    pub notes: Vec<String>,
}

/// Where the room sits against one reach.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BaselineAgainstDto {
    /// `recent`, `tonight` or `phase`.
    pub horizon: String,
    /// What it is being compared with, in words — "than it has been tonight".
    pub than: String,
    /// `lowest`, `lower`, `usual`, `higher` or `highest`.
    pub against: String,
    /// Whether this reach has anything to say. The usual is not news.
    pub notable: bool,
}

/// How recently a reading has to have arrived for the panel to say "watching".
///
/// Ten seconds, against a cadence of one every two: a browser tab that has
/// been backgrounded stops sending, and a panel still claiming to watch the
/// room is the panel lying about the one thing it is for.
const STILL_WATCHING: std::time::Duration = std::time::Duration::from_secs(10);

/// Take one reading of the room.
///
/// Sent by whatever is looking — today djmanzo's own window, which is a secure
/// context and so may open a camera and a microphone. Every field is optional
/// because a source may have one permission and not the other.
#[tauri::command]
pub fn room_saw(
    state: State<'_, AppState>,
    light: Option<f32>,
    movement: Option<f32>,
    loudness: Option<f32>,
) -> Result<(), String> {
    use dj_assistant::room::{Reading, Sense};

    let mut reading = Reading::at(std::time::SystemTime::now());
    for (sense, value) in [
        (Sense::Light, light),
        (Sense::Movement, movement),
        (Sense::Loudness, loudness),
    ] {
        if let Some(value) = value {
            reading = reading.with(sense, value);
        }
    }

    // The phase **as it is now**, stored with the reading. §35's "activity at
    // similar session phases" is a question about the past, and looking the
    // phase up when somebody asks would file the whole night under whatever it
    // had become by then.
    let phase = state.night().read().map(|read| read.phase);

    state
        .room()
        .lock()
        .map_err(|_| "the room's readings are poisoned")?
        .saw(reading, phase);
    Ok(())
}

/// §5's Mission Bar: everything a DJ glances at, in one answer.
///
/// One command rather than the interface assembling six. The judgement — what
/// each reading says, and whether it is worth a colour — is `dj_app::mission`'s
/// and is tested there; this only gathers the inputs, which is the one part
/// that needs a handle on the application.
#[tauri::command]
pub fn mission_bar(state: State<'_, AppState>) -> Vec<crate::mission::Item> {
    use crate::mission::{Device, Reading, Recording, RoomRead};

    let read = state.night().read();
    let (occasion, posture) = state.conduct().lock().map_or_else(
        |_| Default::default(),
        |guard| (guard.occasion, guard.posture),
    );

    // The snapshot the interface is already drawing, not a second capture:
    // a HUD showing a load measured a frame apart from the meters beside it is
    // two truths about one moment.
    let snapshot = get_snapshot(state.clone());
    let master = &snapshot.master;

    // The tempo only when exactly one deck is playing. With two in a mix there
    // is no single "current tempo", and picking one of them would put a number
    // on the bar that is right half the time and says nothing about which half.
    let mut playing = snapshot
        .decks
        .iter()
        .filter(|deck| deck.playing)
        .filter_map(|deck| deck.effective_bpm);
    let tempo = match (playing.next(), playing.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    };

    let room = state
        .room()
        .lock()
        .ok()
        .and_then(|room| room.glance())
        .map(|glance| RoomRead {
            mark: glance.way.mark().to_owned(),
            way: glance.way.name().to_owned(),
            agreeing: glance.agreeing,
            of: glance.of,
            says: glance.because,
        });

    crate::mission::bar(&Reading {
        phase: read.map(|read| read.phase.name().replace('_', " ")),
        phase_certainty: read.map(|read| read.certainty.name().to_owned()),
        occasion: occasion.name().to_owned(),
        posture: posture.name().to_owned(),
        posture_about: posture.about().to_owned(),
        room,
        cpu: master.cpu_load,
        xruns: master.xruns,
        limiter_db: master.limiter_reduction_db,
        recording: master.recording.active.then_some(Recording {
            seconds: master.recording.seconds,
            dropped: master.recording.dropped,
            failed: master.recording.failed,
        }),
        tempo,
        elapsed: Some(state.night().elapsed().as_secs_f64()),
        device: (master.sample_rate > 0.0).then(|| Device {
            name: state
                .active_device()
                .map_or_else(|| "the sound card".to_owned(), |device| device.name),
            sample_rate: master.sample_rate,
            latency_ms: master.output_latency_ms,
        }),
    })
}

/// §11's `DJContext`, gathered in one pass.
///
/// **One question, one moment.** The five fields that were already real were
/// published by five different things on five different schedules, so a
/// consumer wanting three of them asked three questions and got three answers
/// about three different instants. §11's own instruction is *do not duplicate
/// context logic inside each component*, and a component assembling the
/// context out of three polls is the same failure with a clock in it.
///
/// Nothing here decides anything: every field comes from the thing that owns
/// it, and `crate::context` says which. The three that had no gatherer at all
/// — `musicContext`, `hardwareContext`, `djBehaviorContext` — have one now.
///
/// # Errors
/// When the assistant's or the rail's state is poisoned. The room being
/// unreadable is not an error: nothing is watching in most installations, and
/// an absent reading is the honest answer rather than a failure.
#[tauri::command]
pub fn dj_context(state: State<'_, AppState>) -> Result<crate::context::DjContext, String> {
    let snapshot = snapshot_now(&state);

    let occasion = state
        .conduct()
        .lock()
        .map_err(|_| "the conduct lock is poisoned".to_owned())?
        .occasion
        .name()
        .to_owned();

    // §14's gestures over tonight's log, which is the only window there is:
    // the log does not outlive the run that made it.
    let night = state.night();
    let log = state.bus().log();
    let signals = crate::signals::signals(&log, &|at| night.phase_at(at));

    let (taken, ignored) = state
        .fatigue()
        .lock()
        .map_or((0, 0), |fatigue| (fatigue.taken(), fatigue.ignored()));

    // A room nothing is watching has not been read. `Glance` is already `None`
    // in that case, so this needs no second rule about it.
    let audience = state
        .room()
        .lock()
        .ok()
        .and_then(|room| room.glance())
        .map(|glance| glance.because);

    Ok(crate::context::DjContext {
        session_phase: snapshot.context.session.map(|read| read.phase),
        occasion,
        music: crate::context::music(&snapshot),
        hardware: crate::context::hardware(&snapshot, &state.control().status(None)),
        audience,
        behaviour: crate::context::behaviour(&signals, night.elapsed(), taken, ignored),
        attention: snapshot.attention,
        health: crate::context::health(&snapshot, state.worker_load()),
    })
}

/// What the room has been doing, and whether it matches the night.
#[tauri::command]
pub fn room_read(state: State<'_, AppState>) -> Result<RoomDto, String> {
    use dj_assistant::room::{Horizon, Sense, hour_of};

    let room = state.room();
    let room = room
        .lock()
        .map_err(|_| "the room's readings are poisoned")?;

    let occasion = state
        .conduct()
        .lock()
        .map(|guard| guard.occasion)
        .unwrap_or_default();

    let phase = state.night().read().map(|read| read.phase);
    let baseline = [Sense::Movement, Sense::Loudness, Sense::Light]
        .into_iter()
        .filter_map(|sense| {
            let read = room.baseline(sense, phase)?;
            Some(BaselineDto {
                sense: sense.name().to_owned(),
                now: read.now,
                notes: read.notes(),
                against: read
                    .against
                    .iter()
                    .map(|(horizon, against)| BaselineAgainstDto {
                        horizon: match horizon {
                            Horizon::Recent => "recent",
                            Horizon::Tonight => "tonight",
                            Horizon::LikePhase(_) => "phase",
                        }
                        .to_owned(),
                        than: horizon.than(),
                        against: against.name().to_owned(),
                        notable: against.is_notable(),
                    })
                    .collect(),
            })
        })
        .collect();

    Ok(RoomDto {
        // Derived from the readings themselves rather than from a flag the
        // interface sets: a window that closed without saying so cannot leave
        // this stuck on.
        watching: room.last_seen().is_some_and(|at| {
            std::time::SystemTime::now()
                .duration_since(at)
                .is_ok_and(|since| since < STILL_WATCHING)
        }),
        recent: room.recent(),
        enough: room.has_looked_enough(),
        notes: room.notes(phase),
        disagreement: room.disagrees_with(occasion),
        hour: hour_of(std::time::SystemTime::now()),
        light: room.lately(Sense::Light),
        movement: room.lately(Sense::Movement),
        loudness: room.lately(Sense::Loudness),
        baseline,
        phase: phase.map(|p| p.name().to_owned()),
    })
}

// -- what the night is -----------------------------------------------------
//
// See `crate::night` and `dj_core::context`: the phase is a judgement made in
// one place, and this is the view of it.

/// What djmanzo has made of the night.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NightDto {
    /// The phase, by its stable name, or `None` when nothing has read one.
    pub phase: Option<String>,
    /// The phase as it appears mid-sentence, for a heading.
    pub words: Option<String>,
    /// How hard the night is going, 0..=1. See `dj_core::SessionRead::energy`.
    pub energy: Option<f32>,
    /// How much to believe it: `unsure`, `fair` or `sure`.
    pub certainty: Option<String>,
    /// One line saying what that certainty means.
    pub certainty_about: Option<String>,
    /// What produced the phase: `declared`, `measured`, `agreed`, `disputed`.
    pub basis: Option<String>,
    /// Which way the evidence pulls, when the two disagree.
    pub drift: Option<String>,
    /// Roughly when it is, from the clock.
    pub time_of_day: Option<String>,
    /// What the DJ's occasion says the night is, when it says anything.
    pub declared: Option<String>,
    /// What the music alone reads as, which is not always the same thing.
    ///
    /// Carried separately from `phase` so the interface can *mark* a
    /// disagreement rather than only describe it. `phase` is the DJ's word
    /// wherever they have given one; this is what djmanzo would have said.
    pub measured: Option<String>,
    /// How many readings tonight's range is built from.
    pub readings: usize,
    /// How many more before the music alone may name a phase.
    pub still_needed: usize,
    /// Everything worth saying, most important first. Never empty.
    pub notes: Vec<String>,
    /// What the assistant is allowed to do, given the posture and the
    /// certainty. See `dj_assistant::Warrant` — this is §9's matrix as it
    /// actually stands right now.
    pub warrant: String,
}

/// What djmanzo has made of the night, for the surface that shows it.
#[tauri::command]
pub fn night_read(state: State<'_, AppState>) -> Result<NightDto, String> {
    let night = state.night();
    let read = night.read();
    let (readings, still_needed) = night.progress();
    let posture = state
        .conduct()
        .lock()
        .map(|guard| guard.posture)
        .unwrap_or_default();
    let warrant = read.map_or_else(
        || posture.unweighed(),
        |read| posture.warrant(read.certainty),
    );
    Ok(NightDto {
        phase: read.map(|read| read.phase.name().to_owned()),
        words: read.map(|read| crate::night::phase_words(read.phase).to_owned()),
        energy: read.map(|read| read.energy),
        certainty: read.map(|read| read.certainty.name().to_owned()),
        certainty_about: read.map(|read| read.certainty.about().to_owned()),
        basis: read.map(|read| read.basis.name().to_owned()),
        drift: read
            .and_then(|read| read.drift)
            .map(|d| d.name().to_owned()),
        time_of_day: read.map(|read| read.environment.time_of_day.name().to_owned()),
        declared: night.declared().map(|phase| phase.name().to_owned()),
        measured: night.measured().map(|phase| phase.name().to_owned()),
        readings,
        still_needed,
        notes: night.notes(),
        warrant: warrant.name().to_owned(),
    })
}

/// §17's ask, for the rail that starts from it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AsksDto {
    /// `lift`, `hold` or `ease` — the direction the rail starts from.
    pub trajectory: String,
    /// §17's own words for this phase's ask, for the line beside the rail.
    pub words: String,
    /// What the phase asks the ranking to prefer beyond that, when it asks
    /// anything, in the words that appear on the row it credits.
    pub prefer: Option<String>,
}

/// What the phase of the night asks of the ranking.
///
/// §17: *the system may infer phase, but the DJ must always be able to override
/// it*. This is the inferring half and it is deliberately only that — a
/// starting point, handed to the rail, which follows it until the DJ presses a
/// direction and never again after that.
///
/// Separate from [`night_read`] rather than three more fields on `NightDto`,
/// because the two are asked for at different moments by different surfaces:
/// the Night panel asks when it is open, and the rail asks when it ranks. A
/// rail pulling a thirty-field reading of the night to learn one word would be
/// paying for the panel's question.
#[tauri::command]
pub fn phase_asks(state: State<'_, AppState>) -> AsksDto {
    let asks = crate::asks::asks(state.night().read().map(|read| read.phase));
    AsksDto {
        trajectory: asks.trajectory.name().to_owned(),
        words: asks.words.to_owned(),
        prefer: asks.prefer.words().map(str::to_owned),
    }
}

/// Forget the night's readings.
///
/// Every reading is judged against the rest of the night, so moving the camera
/// to a different corner makes the whole night's distribution a comparison
/// with somewhere else. This is how a DJ says "start again from here".
#[tauri::command]
pub fn room_forget(state: State<'_, AppState>) -> Result<(), String> {
    let mut room = state
        .room()
        .lock()
        .map_err(|_| "the room's readings are poisoned")?;
    *room = dj_assistant::room::Room::new();
    Ok(())
}

// -- finding a record from what you remember --------------------------------
//
// See `crate::memory` for why the hum narrows rather than identifies, and
// `dj_library::lyrics` for why the words are folded on both sides.

/// One record whose words contain the phrase.
#[derive(Debug, Clone, Serialize)]
pub struct WordHitDto {
    pub track: LibraryTrackDto,
    /// The line it was found in, as the record has it.
    pub line: String,
    pub line_number: usize,
}

/// Records whose words contain `phrase`.
#[tauri::command]
pub fn words_search(state: State<'_, AppState>, phrase: String) -> Result<Vec<WordHitDto>, String> {
    let found = library(&state)?
        .tracks_with_words(&phrase)
        .map_err(|e| e.to_string())?;
    Ok(found
        .into_iter()
        .map(|(track, hit)| WordHitDto {
            track: LibraryTrackDto::from(track),
            line: hit.line,
            line_number: hit.line_number,
        })
        .collect())
}

/// How much of the collection has been asked about.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WordsProgressDto {
    /// Records with words stored.
    pub with_words: usize,
    /// Records asked about at all, including the ones with nothing to find.
    pub asked: usize,
    pub tracks: usize,
}

#[tauri::command]
pub fn words_progress(state: State<'_, AppState>) -> Result<WordsProgressDto, String> {
    let (with_words, asked, tracks) = library(&state)?
        .words_progress()
        .map_err(|e| e.to_string())?;
    Ok(WordsProgressDto {
        with_words,
        asked,
        tracks,
    })
}

/// How many records one sweep asks about.
///
/// Twenty-five. A sweep is a series of bounded pieces of work rather than one
/// long one, so the interface can show it moving, the DJ can stop it between
/// batches, and a collection of ten thousand does not become a single request
/// that either finishes or fails.
const SWEEP: usize = 25;

/// What one sweep did.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SweepDto {
    pub asked: usize,
    pub found: usize,
    /// Records still never asked about, after this batch.
    pub left: usize,
    /// True when the network refused, so the interface stops rather than
    /// grinding through the whole collection recording failures.
    pub gave_up: bool,
}

/// Fetch words for records that have none, a batch at a time.
///
/// # Errors
/// When the library cannot be read.
#[tauri::command]
pub async fn words_fetch(state: State<'_, AppState>) -> Result<SweepDto, String> {
    use dj_sources::lyrics::{LyricsError, LyricsSource};

    let library = library(&state)?;
    let todo = library.without_words(SWEEP).map_err(|e| e.to_string())?;
    let http = state
        .sources()
        .http()
        .ok_or_else(|| "this machine has no HTTP client, so nothing can be fetched".to_owned())?;
    let source = LyricsSource::new(http);
    let now = crate::library::now_seconds();

    let mut asked = 0;
    let mut found = 0;
    let mut gave_up = false;

    for track in todo {
        let Some(artist) = track
            .tags
            .artist
            .as_deref()
            .filter(|a| !a.trim().is_empty())
        else {
            // Nothing to look up with. Recorded as asked so the sweep moves
            // past it -- an untagged file is a job for the tag editor, not
            // something to retry against a database keyed by artist.
            record_words(&library, &track, "", None, false, "untagged", now);
            asked += 1;
            continue;
        };
        let Some(title) = track.tags.title.as_deref().filter(|t| !t.trim().is_empty()) else {
            record_words(&library, &track, "", None, false, "untagged", now);
            asked += 1;
            continue;
        };

        let seconds = seconds_of(&track);
        match source
            .words_for(artist, title, track.tags.album.as_deref(), seconds)
            .await
        {
            Ok(words) => {
                record_words(
                    &library,
                    &track,
                    &words.plain,
                    words.synced.as_deref(),
                    words.instrumental,
                    "lrclib",
                    now,
                );
                asked += 1;
                if !words.is_empty() {
                    found += 1;
                }
            }
            // A miss is an answer worth keeping, so the next sweep moves on.
            Err(LyricsError::NotFound) => {
                record_words(&library, &track, "", None, false, "lrclib", now);
                asked += 1;
            }
            // A failure is not. Nothing is written, and the sweep stops:
            // grinding through ten thousand records against a dead network
            // just fills the log.
            Err(why) => {
                tracing::warn!(%why, "the lyrics database could not be reached");
                gave_up = true;
                break;
            }
        }
    }

    let left = library
        .without_words(usize::MAX)
        .map(|rest| rest.len())
        .unwrap_or(0);
    Ok(SweepDto {
        asked,
        found,
        left,
        gave_up,
    })
}

fn record_words(
    library: &Arc<dj_library::Library>,
    track: &dj_library::LibraryTrack,
    plain: &str,
    synced: Option<&str>,
    instrumental: bool,
    source: &str,
    at: i64,
) {
    if let Err(why) = library.remember_words(track.id, plain, synced, instrumental, source, at) {
        tracing::warn!(%why, "could not store lyrics");
    }
}

/// A track's length in whole seconds, which is what the lyrics database
/// matches recordings on.
fn seconds_of(track: &dj_library::LibraryTrack) -> u32 {
    let frames = track.duration_frames;
    let rate = u64::from(track.sample_rate.get());
    u32::try_from(frames / rate.max(1)).unwrap_or(0)
}

/// One record the assistant thinks a description might be.
#[derive(Debug, Clone, Serialize)]
pub struct GuessDto {
    pub artist: String,
    pub title: String,
    pub why: Option<String>,
    /// The matching record in the collection, when there is one. This is the
    /// difference between a name and something the DJ can play.
    pub owned: Option<LibraryTrackDto>,
}

/// Ask the assistant which record a description might be.
///
/// Each guess is then looked for in the collection, because a name the DJ
/// already owns is an answer and a name they do not is a shopping list.
///
/// # Errors
/// When no assistant is configured, the budget is spent, or the model fails.
#[tauri::command]
pub async fn guess_from_description(
    state: State<'_, AppState>,
    description: String,
) -> Result<Vec<GuessDto>, String> {
    use dj_assistant::Assistant;

    let selection = state
        .assistant_selection()
        .ok_or_else(|| "no assistant provider is available".to_owned())?;
    let assistant = Assistant::new(
        selection.provider,
        selection.model,
        Arc::clone(state.budget()),
    )
    .with_pricing(selection.input_price, selection.output_price);

    let guesses = assistant
        .guess_song(&description)
        .await
        .map_err(|e| e.to_string())?;

    let library = library(&state)?;
    Ok(guesses
        .into_iter()
        .map(|guess| {
            let owned = library
                .search(&format!("{} {}", guess.artist, guess.title), 1)
                .ok()
                .and_then(|mut found| found.pop())
                .map(LibraryTrackDto::from);
            GuessDto {
                artist: guess.artist,
                title: guess.title,
                why: guess.why,
                owned,
            }
        })
        .collect())
}

/// A record whose melody matched the hum, and where in it.
#[derive(Debug, Clone, Serialize)]
pub struct MelodyHitDto {
    pub track: LibraryTrackDto,
    /// Mean semitone error per point of the hum. Lower is better.
    pub cost: f32,
    /// Seconds into the record where the matching passage starts.
    pub at_seconds: f64,
}

/// What djmanzo made of a hum, and what it narrows the collection to.
#[derive(Debug, Clone, Serialize)]
pub struct HummedDto {
    /// The key, when there was enough pitch to tell.
    pub key: Option<String>,
    pub tempo: Option<f64>,
    pub seconds: f32,
    /// Records near that key and tempo, most recently added first.
    pub near: Vec<LibraryTrackDto>,
    /// Records whose **melody** matches, best first.
    ///
    /// Separate from `near` because they answer different questions and fail
    /// differently: `near` is every record it could be, and this is the ones
    /// that sound like the tune. A collection with no contours yet has an
    /// empty list here and a full one there, which is exactly what should be
    /// shown.
    pub melody: Vec<MelodyHitDto>,
    /// How much of the hum had a pitch in it at all, zero to one.
    ///
    /// Reported so the interface can say "that was mostly breath" rather than
    /// showing an empty shortlist and letting the DJ guess why.
    pub voiced: f32,
}

/// Read a hum and narrow the collection with it.
///
/// `samples` are mono at `rate`. **This does not identify the song** — see
/// `crate::memory` for why, and say so in the interface.
///
/// # Errors
/// When the clip is too short or silent, or the library cannot be read.
#[tauri::command]
pub fn hum(state: State<'_, AppState>, samples: Vec<f32>, rate: u32) -> Result<HummedDto, String> {
    let rate = dj_core::SampleRate::new(rate).ok_or_else(|| format!("{rate} Hz is not a rate"))?;
    let heard = crate::memory::listen(&samples, rate).map_err(|e| e.to_string())?;

    let near = match heard.tempo {
        Some(tempo) => library(&state)
            .and_then(|library| library.search("", 5_000).map_err(|e| e.to_string()))
            .map(|tracks| {
                tracks
                    .into_iter()
                    .filter(|track| {
                        // Tempo narrows; key only narrows when both are known,
                        // so a collection that has not been analysed for key
                        // is filtered by what is actually known about it.
                        let tempo_fits = track
                            .analysis
                            .bpm
                            .is_some_and(|known| crate::memory::near_tempo(tempo, known));
                        let key_fits = match (heard.key, track.analysis.key()) {
                            (Some(hummed), Some(known)) => hummed == known,
                            _ => true,
                        };
                        tempo_fits && key_fits
                    })
                    .take(50)
                    .map(LibraryTrackDto::from)
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    };

    // The melody search is separate from the narrowing above and can be empty
    // while it is full: a collection whose contours have not been made yet
    // knows the key and the tempo of every record and the tune of none.
    let shape = dj_analysis::melody::contour(&samples, rate.get());
    let melody = match library(&state) {
        Ok(library) => {
            let hits = library
                .search_melody(&shape, MELODY_SHORTLIST)
                .unwrap_or_default();
            let mut out = Vec::with_capacity(hits.len());
            for hit in hits {
                // A record whose row went away between the search and here is
                // skipped rather than shown as a blank line.
                if let Ok(Some(track)) = library.track(hit.track) {
                    out.push(MelodyHitDto {
                        track: LibraryTrackDto::from(track),
                        cost: hit.cost,
                        at_seconds: hit.at_seconds,
                    });
                }
            }
            out
        }
        Err(_) => Vec::new(),
    };

    Ok(HummedDto {
        key: heard.key.map(dj_core::MusicalKey::camelot),
        tempo: heard.tempo,
        seconds: heard.seconds,
        near,
        melody,
        voiced: shape.voiced(),
    })
}

/// How many melody matches a hum comes back with.
///
/// Ten. Long enough that the right record is in it when the search is working
/// and short enough to read at a glance; a longer list is not a better answer,
/// it is the same answer with more noise after it.
const MELODY_SHORTLIST: usize = 10;

/// Make pitch contours for records that have none, a batch at a time.
///
/// The same shape as the lyrics sweep, and for the same reason: this is
/// expensive once per record and free forever after, so it is a job the
/// interface can start, watch and stop rather than something that happens at
/// an unpredictable moment.
///
/// **Decodes the whole file.** That is the cost, it is why the batch is small,
/// and it is why a record that will not decode is recorded as attempted rather
/// than retried on every sweep -- with an empty contour, which matches nothing
/// and is skipped by the search.
///
/// # Errors
/// When the library cannot be read.
#[tauri::command]
pub async fn melody_sweep(state: State<'_, AppState>) -> Result<SweepDto, String> {
    let library = library(&state)?;
    let todo = library
        .without_melody(MELODY_SWEEP)
        .map_err(|e| e.to_string())?;

    let mut asked = 0;
    let mut found = 0;

    for track in todo {
        asked += 1;
        let contour = match dj_decode::decode_file(&track.path) {
            Ok(decoded) => {
                // Folded to mono first. `as_interleaved` is stereo, and a
                // contour taken straight off it is twice as long as the
                // record, which puts every reported timestamp at half the
                // truth without failing anything -- see `melody::mono`.
                let shape = dj_analysis::melody::contour(
                    &dj_analysis::melody::mono(
                        decoded.buffer.as_interleaved(),
                        dj_decode::CHANNELS,
                    ),
                    decoded.buffer.sample_rate().get(),
                );
                if shape.voiced() > 0.0 {
                    found += 1;
                }
                shape
            }
            Err(problem) => {
                // A file that will not decode is a file that will not decode
                // next time either. An empty contour matches nothing and stops
                // the sweep offering it again for the rest of the night.
                tracing::debug!(%problem, path = %track.path.display(), "no contour: could not decode");
                dj_analysis::melody::Contour {
                    semitones: Vec::new(),
                    rate: dj_analysis::melody::RATE,
                }
            }
        };
        library
            .remember_melody(&track.id, &contour)
            .map_err(|e| e.to_string())?;
    }

    let (have, all) = library.melody_progress().map_err(|e| e.to_string())?;
    Ok(SweepDto {
        asked,
        found,
        left: all.saturating_sub(have),
        gave_up: false,
    })
}

/// How many records have a pitch contour, and how many there are.
///
/// Its own command rather than a field on the hum result, because the panel
/// needs the number *before* anybody hums: a contour index that is empty makes
/// the melody search return nothing, and a DJ finding that out by humming
/// first has already wasted the one thing this feature asks of them.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MelodyProgressDto {
    /// Records with a contour stored, including the ones that would not decode.
    pub with_melody: usize,
    pub tracks: usize,
}

/// How much of the collection can be searched by tune.
///
/// # Errors
/// When the library cannot be read.
#[tauri::command]
pub fn melody_progress(state: State<'_, AppState>) -> Result<MelodyProgressDto, String> {
    let (with_melody, tracks) = library(&state)?
        .melody_progress()
        .map_err(|e| e.to_string())?;
    Ok(MelodyProgressDto {
        with_melody,
        tracks,
    })
}

/// How many records one press of the sweep works through.
///
/// Twenty. Each one is a full decode plus a pitch pass, so this is seconds of
/// work rather than milliseconds, and a batch that returns is one the
/// interface can report on and the DJ can stop.
const MELODY_SWEEP: usize = 20;

#[cfg(test)]
mod lock_tests {
    use super::*;

    /// **The load-bearing one for §78: a lock the assistant cannot walk around.**
    ///
    /// `ui_request` applies the arrangement in Rust, stores it, and only then
    /// announces it to the window — so a permit consulted in `App.svelte` is
    /// consulted *after* the DJ's screen has already moved and the change has
    /// been written to disk. The interface honours the same locks for its own
    /// automatic paths; this is the one that has to be here.
    ///
    /// Asserted at **autopilot**, which is the posture where §72's matrix has
    /// nothing left to refuse. §78 calls Freeze the professional safety valve,
    /// and a valve every posture can open is not one.
    #[test]
    fn a_locked_layout_refuses_the_assistant_at_the_posture_that_allows_everything() {
        let state = AppState::new(true);
        if let Ok(mut guard) = state.conduct().lock() {
            guard.posture = dj_assistant::Posture::Autopilot;
        }

        let mut workspace = crate::cockpit::opening();
        assert_eq!(
            assistant_may_rearrange(&state, &workspace),
            Ok(()),
            "the assistant is refused with nothing locked and autopilot on"
        );

        workspace.freeze(true);
        let refused = assistant_may_rearrange(&state, &workspace)
            .expect_err("a frozen layout let the assistant move a panel anyway");
        assert!(
            refused.contains("locked"),
            "the refusal does not say a lock caused it, so a DJ whose assistant \
             has gone quiet cannot tell whether to change the posture or clear \
             a lock: {refused}"
        );
    }

    /// Locking something else leaves the assistant alone.
    ///
    /// §79's six are separate, and a "lock the theme" that silently stopped the
    /// assistant rearranging would be a freeze under a narrower name — which a
    /// DJ would find out mid-set.
    #[test]
    fn a_lock_on_something_else_does_not_quiet_the_assistant() {
        let state = AppState::new(true);
        let mut workspace = crate::cockpit::opening();
        workspace.locked = vec![crate::cockpit::Lock::Theme, crate::cockpit::Lock::Density];
        assert_eq!(assistant_may_rearrange(&state, &workspace), Ok(()));
    }
}

#[cfg(test)]
mod rail_tests {
    use super::*;
    use dj_assistant::{Appetite, Fatigue};

    fn id(byte: u8) -> dj_core::TrackId {
        dj_core::TrackId::from_bytes([byte; 32])
    }

    /// A DJ who has been ignoring the rail gets a shorter one.
    ///
    /// The arithmetic is `Appetite::out_of` and is tested in `dj-assistant`;
    /// this is the seam — that the rail actually asks, and asks with the number
    /// the DJ requested rather than one of its own.
    #[test]
    fn the_rail_thins_as_the_dj_ignores_it() {
        let mut fatigue = Fatigue::new();
        assert_eq!(rail_size(8, 5, &fatigue), 8, "a fresh night is not thinned");

        for _ in 0..dj_assistant::fatigue::IGNORED_BEFORE_QUIETER {
            fatigue.offering(&[id(1)]);
            fatigue.landed(id(99));
        }
        assert_eq!(fatigue.appetite(), Appetite::Half);
        assert_eq!(rail_size(8, 5, &fatigue), 4);
    }

    /// **§18's emergency silences it, and nothing else does.**
    ///
    /// The distinction this function exists to draw. A mix is a small budget and
    /// the rail keeps its length, because it is a panel the DJ opened and one
    /// that shrank under them would look broken with nothing saying why. A
    /// failed recording is a budget of none and the rail goes: §18's own words
    /// are that the DJ needs the controls, not the advice.
    #[test]
    fn a_mix_leaves_the_rail_alone_and_an_emergency_empties_it() {
        let fatigue = Fatigue::new();
        let mixing = crate::cockpit::Attention::performing().suggestions;
        assert_eq!(
            rail_size(8, mixing, &fatigue),
            8,
            "the rail the DJ opened collapsed the moment a second deck became \
             audible, and nothing on screen says why"
        );

        let emergency = crate::cockpit::Attention::emergency().suggestions;
        assert_eq!(emergency, 0, "§18's emergency is no longer silent");
        assert_eq!(
            rail_size(8, emergency, &fatigue),
            0,
            "a recording has failed and djmanzo is still re-ranking records at \
             the DJ trying to fix it"
        );
    }

    /// What the caller asked for is still a ceiling, and still sane.
    #[test]
    fn the_rail_never_exceeds_what_was_asked_for_or_what_is_reasonable() {
        let fatigue = Fatigue::new();
        assert_eq!(rail_size(3, 5, &fatigue), 3);
        assert_eq!(rail_size(0, 5, &fatigue), 1, "a rail of none is not a rail");
        assert_eq!(rail_size(10_000, 5, &fatigue), 100);
    }
}

#[cfg(test)]
mod one_source_of_truth {
    /// **§87's second sentence, as a rule: *one source of state truth*.**
    ///
    /// `load_origins.rs` proves two origins agree today. This is what stops the
    /// third from disagreeing, and it is the half that matters more, because
    /// the way this breaks is not a redesign — it is somebody adding an origin
    /// and setting the deck's name themselves, which is two lines and looks
    /// obviously correct. The deck would then have a title and no waveform, no
    /// library row, and nothing in the session log: a set that replays as
    /// silence, found weeks later.
    ///
    /// Two facts are guarded, and they are the two a load leaves outside the
    /// engine: **what is on the deck**, and **that it was**. Everything else a
    /// load does — the summary, the library row, the restored cues — follows
    /// from being inside `put_on_deck` at all.
    /// The bus, the session log and the palette spell a load the same way.
    ///
    /// Three readers of one language. `SessionEvent::to_line` writes it into
    /// every set file, `parse_line` reads it back and is what the palette offers
    /// from, and `perform` carries it out — so a line copied out of a session
    /// file into a controller mapping is a line that works, and a DJ who typed
    /// one into the palette is not told the vocabulary lacks a verb it has.
    #[test]
    fn a_load_is_spelled_the_same_way_in_the_log_as_it_is_on_the_bus() {
        let deck = dj_core::DeckId::from_human(2).unwrap();
        let track = dj_core::TrackId::from_bytes([0xab; 32]);
        let written = dj_control::SessionEvent::Load { deck, track }.to_line();

        assert!(
            written.starts_with("load deck "),
            "the log writes a load as {written:?} and the bus reads `load deck `"
        );
        assert_eq!(
            dj_control::SessionEvent::parse_line(&written),
            Ok(dj_control::SessionEvent::Load { deck, track }),
            "the log cannot read back what it wrote"
        );
        // And the palette offers it, which is the reader that used to disagree.
        assert!(
            super::offered(&written, 4, crate::tiers::Tier::Preparation)
                .entries
                .iter()
                .any(|entry| entry.run == written),
            "the palette refuses a line the bus performs, so a DJ typing it is \
             told the vocabulary does not have it and then finds that it does"
        );
    }

    #[test]
    fn only_the_load_funnel_says_what_is_on_a_deck() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        // Line endings normalised before anything looks for one.
        //
        // This is a test that reads Rust source, and CI checks the repository
        // out on Windows, where git hands it over with CRLF. The search below is
        // for a closing brace at column zero -- `"\n}\n"` -- which is simply not
        // in a file whose lines end `\r\n`, so the test failed there and only
        // there, with a message about `put_on_deck` having moved when it had
        // not. Every other house-pattern test in this workspace happens to
        // search for `"\n}"`, which *is* a substring of `"\r\n}"`, which is why
        // this is the first one to hit it.
        let funnel = std::fs::read_to_string(root.join("commands.rs"))
            .expect("this file is beside the others")
            .replace("\r\n", "\n");

        // `put_on_deck`'s body: from its signature to the next line that closes
        // a top-level item. Crude and sufficient — a function that stopped
        // being top-level would fail this loudly rather than quietly widening
        // the exemption.
        let start = funnel
            .find("pub fn put_on_deck(")
            .expect("the load funnel is no longer called that");
        let end = start
            + funnel[start..]
                .find("\n}\n")
                .expect("`put_on_deck` no longer ends at column zero");
        let inside = start..end;

        // Where the tests begin. Everything after it builds state directly,
        // which is what a unit test is for.
        let tests = funnel.find("\n#[cfg(test)]").unwrap_or(funnel.len());

        for (verb, what) in [
            (".set_deck_track(", "says what record is on a deck"),
            (".record_load(", "tells the session log a record was loaded"),
        ] {
            let mut outside = Vec::new();
            for (offset, _) in funnel.match_indices(verb) {
                if inside.contains(&offset) || offset > tests {
                    continue;
                }
                outside.push(funnel[..offset].lines().count() + 1);
            }
            assert!(
                outside.is_empty(),
                "`{verb}` -- which {what} -- is called outside `put_on_deck` at \
                 line(s) {outside:?}. §87 asks for one source of state truth, and \
                 a load that sets the deck's name without going through the \
                 funnel gets a title with no waveform, no library row and \
                 nothing in the session log."
            );
        }

        // And nowhere else in the crate either.
        for entry in std::fs::read_dir(&root).expect("the crate's sources") {
            let path = entry.expect("a readable entry").path();
            if path.extension().is_none_or(|kind| kind != "rs")
                || path.file_name().is_some_and(|name| name == "commands.rs")
            {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .expect("a readable source")
                .replace("\r\n", "\n");
            let production = source
                .split("#[cfg(test)]")
                .next()
                .expect("split always yields one");
            assert!(
                !production.contains(".set_deck_track("),
                "{} sets the deck's name outside the load funnel",
                path.display()
            );
        }
    }
}

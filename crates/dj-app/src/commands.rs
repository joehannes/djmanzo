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
/// When the panel name is not one of the six, or the window will not open.
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
    tauri::WebviewWindowBuilder::new(
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
}

/// Which separator is running on this machine.
#[tauri::command]
pub fn stems_status(state: State<'_, AppState>) -> StemsStatusDto {
    let backend = state.stems_backend();
    StemsStatusDto {
        available: backend.is_some(),
        backend: backend.map(str::to_owned),
        reason: state.stems_reason(),
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
    remember_track(state, &decoded, sample_rate);

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
        for _ in 0..chunk_count {
            let position = f64::from(registry.get(dj_core::param::ParamId::Deck(
                deck_id,
                dj_core::param::DeckParam::Position,
            )));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let here = if chunk_frames == 0 {
                0
            } else {
                (position.max(0.0) / chunk_frames as f64) as usize
            };

            let table = lock_clone.load();
            let Some(next) =
                next_chunk_to_separate(chunk_count, here, |index| table.has_chunk(index))
            else {
                break;
            };
            drop(table);

            // `process_chunk` blocks once the worker is a few chunks behind,
            // so this thread spends most of its life asleep rather than racing
            // ahead building a queue. That is the point of the bound -- and it
            // is also what makes re-choosing worthwhile, because by the time it
            // wakes the playhead has usually moved.
            // Separated with the audio either side of it, and trimmed back to
            // the chunk afterwards.
            //
            // A chunk separated alone is wrong at both edges -- the windows
            // there have no neighbours to overlap-add with -- so butting them
            // together put a glitch at every seam, once every ten seconds, for
            // the whole track. See `dj_stems::stems::SEPARATION_MARGIN`.
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
        }
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
    let parsed = Action::parse(action).map_err(|e| format!("{action:?}: {e}"))?;

    // A hand arrived on a control. Recorded before the action is carried out,
    // so an autopilot tick that lands between the two still sees the takeover
    // -- the wrong order here would let the assistant move a fader in the
    // moment between a DJ grabbing it and the engine hearing about it.
    state.note_human_touch(&parsed);

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
                let _ = state.bus().dispatch(parsed);
                return Ok(());
            }
            dj_core::DeckAction::LoopRecall(slot) => {
                recall_loop(state, deck, slot)?;
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
        apply_grid_edit(state, deck, edit)?;
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
                    GridEdit::Tap | GridEdit::Reset => unreachable!("handled above"),
                }
            }
        };

    waveforms.set_grid(
        deck,
        Some(dj_render::GridOverlay {
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
        crate::snapshot::Live {
            recording: Some(&recording),
            night: Some(state.night()),
        },
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
        .map(|entry| format!("{:>8.3}  {}", entry.at.as_secs_f64(), entry.event.to_line()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod command_palette {
        use super::super::{PALETTE_LIMIT, matches, palette};

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
        #[test]
        fn it_stops_at_a_readable_number() {
            assert!(palette(String::new(), 6).len() <= PALETTE_LIMIT);
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
mod separation_order_tests {
    use super::next_chunk_to_separate;

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

    /// **A deck with a record cued on it is somewhere to mix into.**
    ///
    /// `idle` used to mean *empty*, and `staged` is what is on the idle deck —
    /// so the two could never both be populated and the autopilot's mixing
    /// branch was unreachable in the running application. It staged a record,
    /// the deck became loaded, and it then said "nothing staged to mix into"
    /// for the rest of the night.
    ///
    /// The unit tests upstairs did not catch it because their fixture has an
    /// idle deck *with* a record on it — a situation `read_situation` could
    /// not produce. This asserts the thing the fixture assumes, against the
    /// function that has to produce it.
    #[test]
    fn a_loaded_but_stopped_deck_is_where_the_autopilot_mixes_to() {
        let state = app_with_grid(128.0, 0.0);
        let registry = state.registry();
        let set = |deck: u8, param, value: f32| {
            let id = dj_core::DeckId::from_human(deck).expect("a deck");
            registry.set(dj_core::ParamId::Deck(id, param), value);
        };
        // Deck 1 playing, deck 2 holding a cued record.
        set(1, dj_core::param::DeckParam::Loaded, 1.0);
        set(1, dj_core::param::DeckParam::Playing, 1.0);
        set(1, dj_core::param::DeckParam::Position, 100_000.0);
        set(1, dj_core::param::DeckParam::LengthFrames, 1_000_000.0);
        set(2, dj_core::param::DeckParam::Loaded, 1.0);
        set(2, dj_core::param::DeckParam::Playing, 0.0);

        let situation = read_situation(&state, &crate::state::Conduct::default());

        assert_eq!(
            situation.live,
            dj_core::DeckId::from_human(1).expect("deck 1"),
            "the playing deck is the one the room is hearing"
        );
        assert_eq!(
            situation.idle,
            dj_core::DeckId::from_human(2),
            "a stopped deck with a record on it was not offered as somewhere \
             to mix into, so the autopilot can never mix"
        );
    }

    /// **A record with no phrase structure is still a record.**
    ///
    /// `Incoming::phrase` is an `Option` because plenty of records have none —
    /// the analyser says so, and the planner answers "bar line, no phrase
    /// structure" rather than refusing. `read_situation` used to build it with
    /// `?` on the phrase fields, so such a record was not staged *at all*: the
    /// autopilot reported "nothing chosen to play next" about a track cued on
    /// the deck in front of it.
    ///
    /// Asserted through `phrase_of`, which is the one place that rule lives.
    #[test]
    fn a_record_with_no_phrase_structure_still_has_a_reading() {
        let track = dj_library::LibraryTrack {
            id: dj_core::TrackId::from_bytes([7; 32]),
            path: std::path::PathBuf::from("/music/live-set.flac"),
            tags: dj_library::Tags::default(),
            duration_frames: 48_000 * 200,
            sample_rate: SR,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            // Analysed, tempo and all -- with no phrase structure, which is a
            // real answer for a live recording or an ambient record.
            analysis: dj_library::StoredAnalysis {
                bpm: Some(124.0),
                phrase_beats: None,
                phrase_anchor: None,
                ..dj_library::StoredAnalysis::default()
            },
            stats: dj_library::PlayStats::default(),
            colour: None,
        };

        assert!(
            phrase_of(&track).is_none(),
            "the fixture has a phrase, so it cannot test a record without one"
        );
        assert!(
            crate::plan::plan(
                &crate::plan::Outgoing {
                    position: 0.0,
                    length: 400.0 * 22_050.0,
                    bpm: 124.0,
                    phrase: None,
                    key: None,
                    sample_rate: SR,
                    grid_anchor: 0.0,
                },
                &crate::plan::Incoming {
                    bpm: 124.0,
                    phrase: phrase_of(&track),
                    key: None,
                },
            )
            .is_some(),
            "the planner refused a record with no phrase structure, which it \
             is supposed to handle by saying so"
        );
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
        Step::Cue { deck, beat } => {
            perform(
                state,
                &format!("deck {} seek_beat {beat}", deck.human_number()),
            )?;
            Ok(Some(format!("cued deck {}", deck.human_number())))
        }
        Step::MatchGain { deck, db } => {
            perform(state, &format!("deck {} gain {db:.2}", deck.human_number()))?;
            Ok(Some(format!(
                "trimmed deck {} by {db:+.1} dB",
                deck.human_number()
            )))
        }
        Step::Mix { beats, style, .. } => {
            // Through the automix, which already knows how to run a transition
            // of a given style and length. Re-implementing it here would be a
            // second transition engine to keep in agreement with the first.
            perform(state, &format!("automix style {}", style.as_str()))?;
            perform(state, &format!("automix beats {beats}"))?;
            perform(state, "automix now")?;
            Ok(Some(format!("mixing over {beats} beats")))
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
            // A load is not a technique. It is how a record got here.
            dj_control::SessionEvent::Load { .. } => None,
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
    let next = dj_assistant::coach::next_lesson(&shown, rig(&state));

    Ok(CoachDto {
        observed,
        note,
        next: next.map(|t| t.name.to_string()),
        next_metaphor: next.map(|t| t.metaphor.to_string()),
    })
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
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.occasion = wanted;
    Ok(())
}

/// Choose a pack, setting both dials at once.
#[tauri::command]
pub fn assistant_apply_pack(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let pack = dj_assistant::packs()
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
        .ok_or_else(|| format!("no pack called {name:?}"))?;
    let conduct = state.conduct();
    let mut guard = conduct.lock().map_err(|_| "assistant state is poisoned")?;
    guard.posture = pack.posture;
    guard.occasion = pack.occasion;
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
    crate::autopilot::next_step(&situation, &conduct.takeover)
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

    // A deck that is **not playing** -- not one that is empty.
    //
    // This read `Loaded <= 0.5` and so could only ever name an empty deck,
    // which made `staged` below -- what is *on* the idle deck -- permanently
    // `None`, and the autopilot's whole mixing branch unreachable: it staged a
    // record, the deck became loaded, `idle` went to `None`, and it said
    // "nothing staged to mix into" for the rest of the night. The unit tests
    // never caught it because their fixture has an idle deck *with* a record
    // on it, which is the situation this function could not produce.
    //
    // Not playing is also the automix's own definition of somewhere to go
    // (`automix::free_deck`), which is the answer both of them want: a deck
    // holding a cued record is exactly what a mix needs.
    let idle = decks
        .iter()
        .copied()
        .find(|d| *d != live && read(*d, dj_core::param::DeckParam::Playing) <= 0.5);

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
                    // `phrase_of`, which answers `None` for a record with no
                    // phrase structure. This used to be two `?`s on the phrase
                    // fields *inside* the closure, so a record the analyser
                    // found no phrases in did not merely lack a phrase -- it
                    // was not staged at all, and the autopilot said "nothing
                    // chosen to play next" about a record sitting cued on the
                    // deck. `Incoming::phrase` is an `Option` precisely
                    // because plenty of records have none, and the planner
                    // already says so rather than refusing.
                    phrase: phrase_of(&track),
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
        live,
        outgoing,
        idle,
        staged,
        next,
        // The same held transition the automix is following, read the same
        // way. One question, one answer -- see `automix_setup`.
        set_up: automix_setup(state),
        gain_offset_db,
    }
}

/// One line naming what a step is, for the panel.
fn describe_step(step: &crate::autopilot::Step) -> String {
    use crate::autopilot::Step;
    match step {
        Step::Nothing => "nothing".to_owned(),
        Step::Stage { deck, .. } => format!("load deck {}", deck.human_number()),
        Step::Cue { deck, beat } => format!("cue deck {} to beat {beat}", deck.human_number()),
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

    // Decoded once each and kept, rather than re-decoded per load: a set that
    // brings a record back for a second play should not pay for it twice, and
    // a DJ's crate is small enough that holding it is cheaper than the disk.
    let mut decoded: std::collections::HashMap<dj_core::TrackId, Arc<dyn dj_decode::TrackSource>> =
        std::collections::HashMap::new();
    let mut resolve = |id: dj_core::TrackId| -> Option<Arc<dyn dj_decode::TrackSource>> {
        if let Some(found) = decoded.get(&id) {
            return Some(Arc::clone(found));
        }
        let track = db.track(id).ok().flatten()?;
        let loaded = dj_decode::decode_file(&track.path).ok()?;
        let source: Arc<dyn dj_decode::TrackSource> = Arc::new(loaded.buffer);
        decoded.insert(id, Arc::clone(&source));
        Some(source)
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tail = (tail_seconds.max(0.0) * rate.as_f64()) as u64;
    let rendered = crate::replay::render_to_wav(
        &session,
        rate,
        state.deck_count(),
        tail,
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

/// What the night is doing, and why.
///
/// `None` from [`session_read`] while the night has no shape yet — which is a
/// real answer and not an error. See [`crate::context`].
#[derive(Debug, Clone, Serialize)]
pub struct NightDto {
    /// `warm_up`, `heat`, `peak`, `cooldown` or `chill_out`.
    pub phase: String,
    /// 0..=1, the energy of the recent stretch.
    pub energy: f32,
    /// 0..=1. How much evidence is behind the reading, not how strongly it is
    /// held. A DJ reading "peak" off three records should be told it is three.
    pub confidence: f64,
    /// How many measured records it is drawn from.
    pub records: usize,
    /// Short phrases, as the planner's and the suggester's are.
    pub because: Vec<String>,
}

/// What the context engine makes of the night.
///
/// Asked for rather than pushed: the reading changes once a record, and the
/// panel that draws it is not always open. The *result* rides the snapshot —
/// `context.session` — because the living interface morphs to it on every
/// frame; this is the same reading with its reasoning attached, for the panel
/// that has to explain it.
#[tauri::command]
pub fn session_read(state: State<'_, AppState>) -> Option<NightDto> {
    let reading = state.read_night()?;
    Some(NightDto {
        phase: reading.read.phase.name().to_owned(),
        energy: reading.read.energy,
        confidence: reading.confidence,
        records: reading.records,
        because: reading.because.iter().map(describe_night_reason).collect(),
    })
}

/// Render one reason for the interface. Terse, like the planner's.
///
/// Energies as percentages rather than as `0.42`: the number is a judgement on
/// a scale nobody has seen, and a percentage at least says which way is up.
fn describe_night_reason(reason: &crate::context::Because) -> String {
    use crate::context::Because;
    let pct = |value: f64| format!("{:.0}%", value * 100.0);
    match reason {
        Because::Rising { from, to } => format!("rising · {} → {}", pct(*from), pct(*to)),
        Because::Falling { from, to } => format!("easing · {} → {}", pct(*from), pct(*to)),
        Because::Holding { at } => format!("holding at {}", pct(*at)),
        Because::NearThePeak { energy } => {
            format!("at the night's own ceiling ({})", pct(*energy))
        }
        Because::TempoRising { from, to } => format!("tempo {from:.0} → {to:.0} BPM"),
        Because::TempoFalling { from, to } => format!("tempo {from:.0} → {to:.0} BPM"),
    }
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

    Ok(crate::transition::Transition::plan(
        (from, to),
        (out_track.id, in_track.id),
        outgoing,
        incoming,
        confidence,
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

/// Move the held transition to a place in the record.
///
/// Frames, because this is what a hand on the waveform produces: §26 asks for
/// the mix point to be grabbed rather than typed, and a pointer lands on a
/// place in a record rather than on a beat index. Which beat that is stays
/// djmanzo's arithmetic — see [`crate::transition::Transition::move_to_frame`].
///
/// `which` is `start` or `end`. One command rather than two because the two
/// differ by a single line and the interface calls them from the same gesture;
/// an unknown value is refused rather than guessed at.
#[tauri::command]
pub fn transition_drag(
    state: State<'_, AppState>,
    which: String,
    frame: f64,
) -> Result<Option<TransitionDto>, String> {
    let dragged = state.edit_transition(|transition| {
        match which.as_str() {
            "start" => transition.move_to_frame(frame),
            "end" => transition.end_at_frame(frame),
            _ => return None,
        }
        Some(transition.clone())
    });
    match dragged.flatten() {
        Some(transition) => describe_transition(&state, &transition, true),
        None if which != "start" && which != "end" => Err(format!("no {which} to drag")),
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

    let trajectory = match trajectory.as_str() {
        "lift" => Trajectory::Lift,
        "ease" => Trajectory::Ease,
        _ => Trajectory::Hold,
    };

    // A generous pool, then ranked and cut. Ranking is cheap arithmetic per
    // track; reading the rows is the part that costs, so the limit is applied
    // after scoring rather than before -- cutting first would rank an arbitrary
    // slice of the library.
    let pool = db.all_tracks(5_000).map_err(|e| e.to_string())?;
    let playing_now = current_track(&state, deck_id);

    Ok(dj_library::suggest::rank(&now, trajectory, &pool)
        .into_iter()
        // Never suggest what is already on the deck.
        .filter(|s| Some(s.track) != playing_now)
        .take(limit.clamp(1, 100))
        .filter_map(|s| {
            let track = pool.iter().find(|t| t.id == s.track)?;
            Some(SuggestionDto {
                track: LibraryTrackDto::from(track.clone()),
                score: s.score,
                reasons: s.reasons.iter().map(describe_reason).collect(),
                summary: summarise_reasons(&s.reasons),
                confidence: s.confidence(),
            })
        })
        .collect())
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

    // Re-sorted, because the tilt has moved things. Ties break on id so the
    // same library gives the same answer every time -- a "more like this" that
    // shuffled on each press would be impossible to trust.
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.track.cmp(&b.1.track))
    });

    Ok(ranked
        .into_iter()
        .take(limit.clamp(1, 100))
        .map(|(score, s, track)| SuggestionDto {
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

/// Which track is on a deck, if any.
fn current_track(state: &AppState, deck: dj_core::DeckId) -> Option<dj_core::TrackId> {
    let tracks = state.deck_tracks();
    let map = tracks.lock().ok()?;
    map.get(&deck.human_number()).map(|t| t.id)
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
fn summarise_reasons(reasons: &[dj_library::suggest::Reason]) -> String {
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
            Reason::TempoFits { .. }
            | Reason::TempoHalfOrDouble { .. }
            | Reason::TempoFar { .. } => 0,
            Reason::SameKey(_) | Reason::Harmonic { .. } | Reason::KeyClash { .. } => 1,
            Reason::Loudness { .. } => 2,
            Reason::SameFamily(_) | Reason::OtherFamily { .. } => 3,
            Reason::PhraseKnown { .. } | Reason::PhraseUnknown | Reason::Unanalysed => 4,
        }
    }

    let mut ordered: Vec<&Reason> = reasons.iter().collect();
    ordered.sort_by_key(|r| place(r));

    ordered
        .into_iter()
        .filter_map(|reason| match reason {
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
    /// `action` or `surface` -- how the interface should carry it out.
    pub kind: &'static str,
    /// The action text, or the surface name.
    pub run: String,
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
/// Three tiers, and the first is the one that makes this more than a menu:
///
/// 1. **What you typed, if it is a real action.** `deck 2 loop 8` parses, so
///    the top entry runs it verbatim. This is what turns the palette into the
///    semantic interface §51 asks for -- the whole 82-verb vocabulary is
///    reachable by typing it, including every verb that takes an argument,
///    which a list of buttons could never offer without inventing numbers.
/// 2. **Verbs that need no argument**, one per deck in use. `play`, `cue`,
///    `sync`, `eject` -- the things a palette is actually reached for.
/// 3. **Surfaces**, so "show prepare" and "show plan" work as words.
///
/// Matching is a subsequence test rather than a substring one, because that is
/// what a palette user expects: `d2p` finds `Deck 2 · play`. Within a tier the
/// order is the vocabulary's own, which is grouped by what the verbs do, so an
/// empty query opens on transport rather than on whatever sorts first
/// alphabetically.
#[tauri::command]
#[must_use]
pub fn palette(query: String, decks: u8) -> Vec<PaletteEntryDto> {
    use dj_core::vocabulary::{Target, vocabulary};

    let needle = query.trim();
    let mut out = Vec::new();

    // Tier 1: the query itself, when the parser accepts it.
    if !needle.is_empty() && dj_core::Action::parse(needle).is_ok() {
        out.push(PaletteEntryDto {
            label: format!("Run: {needle}"),
            about: "The vocabulary accepts this exactly as typed.".to_owned(),
            kind: "action",
            run: needle.to_owned(),
        });
    }

    let decks = decks.clamp(1, 6);
    for spec in vocabulary() {
        // A verb needing an argument cannot be offered as a press: the palette
        // would have to invent the number. Tier 1 is how those are reached.
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
                    });
                }
            }
        }
    }

    // Tier 3: the surfaces, by the words a DJ would reach for them with.
    for surface in crate::cockpit::surfaces() {
        let label = format!("Show {}", surface.title);
        if matches(needle, &label) || matches(needle, surface.name) {
            out.push(PaletteEntryDto {
                label,
                about: surface.about.to_owned(),
                kind: "surface",
                run: surface.name.to_owned(),
            });
        }
    }

    out.truncate(PALETTE_LIMIT);
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

/// Read one session and build the message for it.
fn share_message(
    state: &State<'_, AppState>,
    session: &str,
    heading: &str,
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
    let (message, dropped) = crate::share::message_and_dropped(&entries, &style);
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
) -> Result<ShareDto, String> {
    share_message(&state, &session, &heading)
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

    let share = share_message(&state, &session, &heading)?;
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
        TimedEvent {
            event: SessionEvent::Action(action),
            at: Duration::from_secs(secs),
        }
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
            TimedEvent {
                event: SessionEvent::Load {
                    deck: DeckId::from_human(1).expect("valid deck"),
                    track: TrackId::from_hex(&"a".repeat(64)).expect("valid id"),
                },
                at: Duration::from_secs(1),
            },
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
    // The device's rate, used only for a deck with nothing on it. A deck's own
    // frames are counted at the *record's* rate -- see `DeckParam::SourceRate`
    // -- and a transition measured against the wrong one is the wrong length
    // by the ratio between them: eight percent, or most of a beat over four
    // bars.
    let device_rate = f64::from(
        registry
            .get(dj_core::ParamId::Global(GlobalParam::SampleRate))
            .max(1.0),
    );
    (0..state.deck_count())
        .filter_map(|index| dj_core::DeckId::new(index as u8))
        .map(|id| {
            let get = |p| registry.get(dj_core::ParamId::Deck(id, p));
            let bpm = f64::from(get(DeckParam::EffectiveBpm));
            let source_rate = f64::from(get(DeckParam::SourceRate));
            crate::automix::DeckView {
                id,
                loaded: get(DeckParam::Loaded) >= 0.5,
                playing: get(DeckParam::Playing) >= 0.5,
                position: f64::from(get(DeckParam::Position)),
                length: f64::from(get(DeckParam::LengthFrames)),
                bpm: (bpm > 1.0).then_some(bpm),
                sample_rate: if source_rate > 0.0 {
                    source_rate
                } else {
                    device_rate
                },
            }
        })
        .collect()
}

/// The transition djmanzo is holding, as the automix needs it.
///
/// `None` unless one is held *and* it still describes the records on those
/// decks — the same check [`transition_current`] makes, for the same reason
/// and with more at stake: a stale plan drawn on a panel is misleading, and a
/// stale plan performed is a mix into the wrong record.
///
/// Read on every tick rather than pushed when it changes. The transition is
/// edited from the interface and dropped when a deck is reloaded, and a
/// pushed copy would be one more thing that can be out of date at the moment
/// it is acted on.
#[must_use]
pub fn automix_setup(state: &AppState) -> Option<crate::automix::Setup> {
    let held = state.transition()?;
    if !held.describes(
        current_track(state, held.outgoing_deck),
        current_track(state, held.incoming_deck),
    ) {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let beats = held.plan.length_beats as f32;
    Some(crate::automix::Setup {
        outgoing: held.outgoing_deck,
        incoming: held.incoming_deck,
        start: held.plan.start_frame,
        beats,
        style: held.plan.style,
    })
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
        if let Err(error) = perform(state, &text) {
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
    state: State<'_, AppState>,
    address: String,
) -> Result<crate::remote::RemoteStatus, String> {
    let parsed: std::net::SocketAddr = address
        .parse()
        .map_err(|_| format!("{address:?} is not an address and port, like 127.0.0.1:9000"))?;
    state
        .remote()
        .start_osc(parsed, Arc::clone(state.bus()), state.registry())
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

    state
        .room()
        .lock()
        .map_err(|_| "the room's readings are poisoned")?
        .saw(reading);
    Ok(())
}

/// What the room has been doing, and whether it matches the night.
#[tauri::command]
pub fn room_read(state: State<'_, AppState>) -> Result<RoomDto, String> {
    use dj_assistant::room::{Sense, hour_of};

    let room = state.room();
    let room = room
        .lock()
        .map_err(|_| "the room's readings are poisoned")?;

    let occasion = state
        .conduct()
        .lock()
        .map(|guard| guard.occasion)
        .unwrap_or_default();

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
        notes: room.notes(),
        disagreement: room.disagrees_with(occasion),
        hour: hour_of(std::time::SystemTime::now()),
        light: room.lately(Sense::Light),
        movement: room.lately(Sense::Movement),
        loudness: room.lately(Sense::Loudness),
    })
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

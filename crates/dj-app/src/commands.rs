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
        Some(&recording),
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

/// A proposed transition, flattened for the interface.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionPlanDto {
    /// Beat index in the outgoing track where the mix should begin.
    pub start_beat: i64,
    /// Seconds into the outgoing track, for a display that speaks in time.
    pub start_seconds: f64,
    pub length_beats: u32,
    /// The style's own name, as the automix panel already spells it.
    pub style: String,
    pub reasons: Vec<String>,
}

/// Plan the mix out of `from_deck` and into `to_deck`.
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
) -> Result<Option<TransitionPlanDto>, String> {
    let db = library(&state)?;
    let from = dj_core::DeckId::from_human(from_deck).ok_or("no such deck")?;
    let to = dj_core::DeckId::from_human(to_deck).ok_or("no such deck")?;

    let Some(out_track) = current_track(&state, from).and_then(|id| db.track(id).ok().flatten())
    else {
        return Ok(None);
    };
    let Some(in_track) = current_track(&state, to).and_then(|id| db.track(id).ok().flatten())
    else {
        return Ok(None);
    };
    let Some(grid) = out_track.analysis.beatgrid() else {
        return Ok(None);
    };

    // The live playhead, not the last snapshot: a plan is about where the track
    // is *now*, and a snapshot can be up to 16 ms stale -- which at 174 BPM is
    // most of a beat.
    let registry = state.registry();
    let read = |p| f64::from(registry.get(dj_core::ParamId::Deck(from, p)));
    let position = read(dj_core::param::DeckParam::Position);
    let length = read(dj_core::param::DeckParam::LengthFrames);

    let outgoing = crate::plan::Outgoing {
        position,
        length,
        bpm: grid.bpm.get(),
        phrase: phrase_of(&out_track),
        key: out_track.analysis.key(),
        sample_rate: out_track.sample_rate,
        grid_anchor: grid.anchor.get(),
    };
    let incoming = crate::plan::Incoming {
        bpm: in_track.analysis.bpm.unwrap_or(grid.bpm.get()),
        phrase: phrase_of(&in_track),
        key: in_track.analysis.key(),
    };

    Ok(
        crate::plan::plan(&outgoing, &incoming).map(|p| TransitionPlanDto {
            start_beat: p.start_beat,
            start_seconds: p.start_frame / out_track.sample_rate.as_f64(),
            length_beats: p.length_beats,
            style: p.style.as_str().to_owned(),
            reasons: p.reasons.iter().map(describe_plan_reason).collect(),
        }),
    )
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
        Reason::Rushed { beats_left } => format!("only {beats_left:.0} beats left"),
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
    /// Human-readable, ordered strongest first. Each is one `Reason`.
    pub reasons: Vec<String>,
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
        .map_or(
            Playing {
                key: None,
                bpm: None,
                lufs: None,
                phrase_beats: None,
            },
            |t| Playing {
                key: t.analysis.key(),
                bpm: t.analysis.bpm,
                lufs: t.analysis.loudness_lufs,
                phrase_beats: t.analysis.phrase_beats,
            },
        );

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
            })
        })
        .collect())
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
        Reason::Unanalysed => "not analysed yet".to_owned(),
    }
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

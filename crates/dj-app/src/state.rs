//! Application state, shared with every Tauri command.

use crate::host::AudioHost;
use crate::waveform::WaveformStore;
use dj_assistant::{Budget, LlmProvider, ProviderId};
use dj_control::{ActionBus, ParameterRegistry};
use dj_engine::Command;
use dj_presets::PresetLibrary;
use dj_secrets::SecretStore;
use dj_sources::SourceRegistry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// What the assistant is pointed at.
///
/// Defaults to a local model, because that is the only option that works before
/// the user has signed up for anything — and the only one that keeps working
/// when the venue's wifi does not.
#[derive(Debug, Clone)]
struct AssistantChoice {
    provider: ProviderId,
    model: String,
    /// Per-million-token pricing for the chosen model, when the provider
    /// reported it. `None` means spend cannot be counted, not that it is free.
    pricing: (Option<f64>, Option<f64>),
}

impl Default for AssistantChoice {
    fn default() -> Self {
        Self {
            provider: ProviderId::Local,
            // Ollama's most common small model. Wrong for some setups, and
            // changeable in one click; better than an empty field.
            model: "llama3.2".to_owned(),
            pricing: (None, None),
        }
    }
}

/// Everything needed to ask the assistant one question.
#[derive(Debug, Clone)]
pub struct AssistantSelection {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    /// USD per million tokens, when the provider reported it.
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
}

/// Decks the engine runs. Kept in step with `host::DECK_COUNT`.
pub const DECK_COUNT: usize = 4;

/// Everything a command handler needs.
///
/// Deliberately small: a bus to send intent into, a registry to read state from,
/// and a handle to the thread that owns the audio device. There is no direct
/// reference to the engine, because nothing outside the audio thread is allowed
/// to touch it.
#[derive(Debug)]
pub struct AppState {
    bus: Arc<ActionBus<Command>>,
    registry: Arc<ParameterRegistry>,
    /// Tap-tempo runs in progress. Lives here rather than on the audio thread
    /// because a run is host state -- see `crate::grid`.
    taps: crate::grid::TapTracker,
    /// Where the DJ's own layout files live. `None` until `setup` resolves it,
    /// and then only the built-in layouts are available.
    layout_dir: Mutex<Option<std::path::PathBuf>>,
    /// The configuration directory itself, which is the layout directory's
    /// parent. Held rather than derived: walking back up out of a
    /// subdirectory to find where you started is the sort of implicit
    /// coupling that breaks silently the first time either path moves.
    config_dir: Mutex<Option<std::path::PathBuf>>,
    host: AudioHost,
    waveforms: Arc<WaveformStore>,
    /// API keys, in the OS keychain. Values go in and never come back out --
    /// see `dj_secrets`.
    secrets: Arc<dyn SecretStore>,
    /// False when no keychain was available and secrets are in memory only.
    /// Surfaced rather than hidden: a user whose keys silently vanish on
    /// restart deserves to have been told at the point of typing them.
    secrets_persist: bool,
    sources: Arc<SourceRegistry>,
    llm_providers: Vec<Arc<dyn LlmProvider>>,
    assistant: Mutex<AssistantChoice>,
    budget: Arc<Budget>,
    presets: PresetLibrary,
    /// What is in each sampler slot, by `(bank, slot)`.
    ///
    /// The engine holds audio and nothing else, the same as it does for a
    /// deck — so the application remembers the name. Here rather than in the
    /// interface for the reason the device taught: a panel that only knows the
    /// loads it made itself shows nothing for a sample a script, a preset or
    /// the assistant put there.
    sample_names: Arc<Mutex<HashMap<(u8, u8), String>>>,
    /// The device that is open, as the interface describes it.
    ///
    /// Held here rather than only in the interface because opening a device is
    /// not something only the Connect button does: a preset, a script, the
    /// assistant or a restored session can all do it, and an interface that
    /// learns about the device only from its own button call shows "no device"
    /// while audio is playing. Not in the 60 Hz snapshot either — a device
    /// changes on connect and never in between, and its name is a `String`.
    active_device: Arc<Mutex<Option<crate::commands::ActiveDeviceDto>>>,
    /// Live figures from the two-device bridge, present only while a split
    /// output is open. Replaced on every device change, so it never reports
    /// numbers from a stream that has already been closed.
    bridge: Arc<Mutex<Option<Arc<dj_audio::BridgeStats>>>>,
    analysis: Arc<crate::analysis::AnalysisStore>,
    /// The track database. In memory until `setup` can say where it lives --
    /// see `crate::library::LibraryHandle`.
    library: Arc<crate::library::LibraryHandle>,
    /// The worker turning scanned files into tracks. `None` when the library
    /// could not be opened at all, which is the one case where there is nothing
    /// for it to do.
    identifier: Mutex<Option<crate::library::Identifier>>,
    /// Writes deck state to the library, off the thread that noticed it.
    library_writer: crate::persist::LibraryWriter,
    /// Last saved cue set per deck. Shared with the snapshot pump, which is
    /// where cue changes are noticed -- see `crate::persist`.
    cue_watcher: Arc<Mutex<crate::persist::CueWatcher>>,
    /// Which tracks have been played far enough to count. Also on the snapshot
    /// pump -- see `crate::persist::PlayWatcher`.
    play_watcher: Arc<Mutex<crate::persist::PlayWatcher>>,
    /// Groups tonight's plays. One per run of the application, which is close
    /// enough to one per gig that it is worth having before there is a real
    /// session concept.
    session_id: String,
    /// Title and artist per deck.
    ///
    /// The engine knows nothing about metadata -- it has samples and a
    /// playhead -- so the name of what is playing has to be remembered here.
    /// Deliberately *not* held in the deck component: a track can arrive from
    /// the browser, the assistant, a preset or a controller as easily as from
    /// the deck's own Load button, and component-local state shows "no track"
    /// for every one of those.
    deck_tracks: Arc<Mutex<HashMap<u8, LoadedTrackInfo>>>,
}

/// What is loaded on a deck, as far as the interface is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTrackInfo {
    pub title: String,
    pub artist: Option<String>,
    /// Content hash of the audio actually on the deck.
    ///
    /// Held so a worker can tell whether the track it was analysing is still
    /// the one loaded. Analysis takes seconds; a DJ can easily load two tracks
    /// onto the same deck in that time, and without this check the first
    /// track's beat grid would be applied to the second.
    pub id: dj_core::TrackId,
}

impl AppState {
    #[must_use]
    pub fn new(use_null_backend: bool) -> Self {
        // The consumer created here is discarded: the real queue is built when a
        // device is opened, and the bus is re-aimed at it. This placeholder just
        // means `dispatch` before any device is open fails cleanly instead of
        // panicking.
        // Built before anything else that might want it. An in-memory library
        // that cannot even be created is not fatal: every library call reports
        // the error, and the decks still play.
        let library = Arc::new(
            crate::library::LibraryHandle::in_memory()
                .unwrap_or_else(|error| panic!("could not create an in-memory library: {error}")),
        );

        let (bus, _placeholder) = ActionBus::<Command>::new(64);
        let bus = Arc::new(bus);
        let registry = Arc::new(ParameterRegistry::new());
        seed_defaults(&registry);
        let host = AudioHost::start(Arc::clone(&bus), Arc::clone(&registry), use_null_backend);

        let (store, secrets_persist) = dj_secrets::open_store();
        let secrets: Arc<dyn SecretStore> = Arc::from(store);
        if !secrets_persist {
            tracing::warn!("no keychain available; API keys will not survive a restart");
        }
        let sources = Arc::new(SourceRegistry::new(Arc::clone(&secrets)));

        // One HTTP client for every provider. Falls back to a stub that fails
        // cleanly rather than refusing to start: the application must still
        // play files on a machine where TLS initialisation goes wrong.
        let llm_providers = match dj_assistant::ReqwestJson::new() {
            Ok(http) => dj_assistant::all_providers(Arc::new(http), Arc::clone(&secrets)),
            Err(error) => {
                tracing::warn!(%error, "no HTTP client; the assistant will be unavailable");
                Vec::new()
            }
        };

        Self {
            bus,
            registry,
            taps: crate::grid::TapTracker::new(),
            layout_dir: Mutex::new(None),
            config_dir: Mutex::new(None),
            host,
            waveforms: Arc::new(WaveformStore::new()),
            secrets,
            secrets_persist,
            sources,
            llm_providers,
            assistant: Mutex::new(AssistantChoice::default()),
            budget: Arc::new(Budget::default()),
            presets: PresetLibrary::builtin(),
            sample_names: Arc::new(Mutex::new(HashMap::new())),
            active_device: Arc::new(Mutex::new(None)),
            bridge: Arc::new(Mutex::new(None)),
            analysis: Arc::new(crate::analysis::AnalysisStore::new()),
            library_writer: crate::persist::LibraryWriter::start(Arc::clone(&library)),
            cue_watcher: Arc::new(Mutex::new(crate::persist::CueWatcher::new())),
            play_watcher: Arc::new(Mutex::new(crate::persist::PlayWatcher::new())),
            session_id: format!("session-{}", crate::library::now_seconds()),
            library,
            identifier: Mutex::new(None),
            deck_tracks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The track database.
    #[must_use]
    pub fn library(&self) -> Arc<crate::library::LibraryHandle> {
        Arc::clone(&self.library)
    }

    /// Open the library at its real home and start identifying what a scan has
    /// found. Called once, from Tauri's `setup`.
    pub fn open_library(&self, path: &std::path::Path) {
        if let Err(error) = self.library.open_at(path) {
            tracing::warn!(%error, ?path, "library stays in memory; it will not survive a restart");
        }
        let worker = crate::library::Identifier::start(
            Arc::clone(&self.library),
            crate::library::now_seconds,
            crate::library::identify_file,
        );
        if let Ok(mut slot) = self.identifier.lock() {
            *slot = Some(worker);
        }
    }

    #[must_use]
    pub fn library_writer(&self) -> crate::persist::LibraryWriter {
        self.library_writer.clone()
    }

    #[must_use]
    pub fn play_watcher(&self) -> Arc<Mutex<crate::persist::PlayWatcher>> {
        Arc::clone(&self.play_watcher)
    }

    /// Groups this run's plays in the history.
    #[must_use]
    pub fn session_id(&self) -> String {
        self.session_id.clone()
    }

    #[must_use]
    pub fn cue_watcher(&self) -> Arc<Mutex<crate::persist::CueWatcher>> {
        Arc::clone(&self.cue_watcher)
    }

    /// Forget a deck's saved cue state, so the next observation counts as a
    /// fresh load rather than as the DJ having changed something.
    pub fn cue_watcher_forget(&self, deck: u8) {
        if let Ok(mut watcher) = self.cue_watcher.lock() {
            watcher.forget(deck);
        }
        // A new track on the deck is a new play to count, even if it is the
        // same record the DJ played an hour ago.
        if let Ok(mut watcher) = self.play_watcher.lock() {
            watcher.forget(deck);
        }
    }

    /// Where the DJ's own layout files live, once Tauri can say.
    #[must_use]
    pub fn layout_dir(&self) -> Option<std::path::PathBuf> {
        self.layout_dir.lock().ok()?.clone()
    }

    /// Point the application at a configuration directory, making the layout
    /// folder inside it. Called once, from `setup`, for the same reason the
    /// library is.
    pub fn set_config_dir(&self, dir: std::path::PathBuf) {
        if let Ok(mut slot) = self.config_dir.lock() {
            *slot = Some(dir.clone());
        }
        let layouts = dir.join("layouts");
        if let Err(error) = std::fs::create_dir_all(&layouts) {
            tracing::warn!(%error, ?layouts, "no layout directory; only the built-in layouts");
            return;
        }
        if let Ok(mut slot) = self.layout_dir.lock() {
            *slot = Some(layouts);
        }
    }

    /// The file the chosen layout's name is written to.
    ///
    /// A name rather than a copy of the layout: a DJ who edits their layout
    /// file wants the edit to take, and storing the whole thing would mean
    /// their next start-up quietly used the version from whenever they last
    /// picked it.
    fn chosen_layout_path(&self) -> Option<std::path::PathBuf> {
        Some(self.config_dir.lock().ok()?.clone()?.join("layout.txt"))
    }

    /// Whether the DJ had the watershed showing.
    ///
    /// Stored beside the layout choice and for the same reason: somebody who
    /// set the interface up the way they wanted should not have to do it again
    /// before every set.
    #[must_use]
    pub fn watershed(&self) -> bool {
        self.config_dir
            .lock()
            .ok()
            .and_then(|d| d.clone())
            .is_some_and(|dir| dir.join("watershed").exists())
    }

    /// Remember whether the watershed was showing.
    pub fn set_watershed(&self, showing: bool) {
        let Some(dir) = self.config_dir.lock().ok().and_then(|d| d.clone()) else {
            return;
        };
        let marker = dir.join("watershed");
        // Presence is the whole state, so there is nothing to parse and nothing
        // that can be corrupt -- a file that exists means yes.
        let result = if showing {
            std::fs::write(&marker, "")
        } else {
            match std::fs::remove_file(&marker) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        };
        if let Err(error) = result {
            tracing::warn!(%error, ?marker, "the watershed choice will not survive a restart");
        }
    }

    /// Which layout the DJ last chose, if any.
    #[must_use]
    pub fn chosen_layout(&self) -> Option<String> {
        let name = std::fs::read_to_string(self.chosen_layout_path()?).ok()?;
        let name = name.trim().to_owned();
        (!name.is_empty()).then_some(name)
    }

    /// Remember the chosen layout across restarts.
    ///
    /// Failing to write is logged, not returned: a DJ who picked a layout got
    /// the layout, and an error dialog about a preference file is noise in the
    /// middle of the one thing they were doing.
    pub fn set_chosen_layout(&self, name: &str) {
        let Some(path) = self.chosen_layout_path() else {
            return;
        };
        if let Err(error) = std::fs::write(&path, name) {
            tracing::warn!(%error, ?path, "the chosen layout will not survive a restart");
        }
    }

    /// How the background identifier is getting on.
    #[must_use]
    pub fn identify_progress(&self) -> Option<Arc<crate::library::IdentifyProgress>> {
        self.identifier
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(crate::library::Identifier::progress))
    }

    #[must_use]
    pub fn presets(&self) -> &PresetLibrary {
        &self.presets
    }

    /// Add the user's own packs, once Tauri can tell us where they live.
    pub fn load_user_presets(&mut self, dir: &std::path::Path) -> usize {
        self.presets.load_dir(dir)
    }

    #[must_use]
    pub fn llm_providers(&self) -> &[Arc<dyn LlmProvider>] {
        &self.llm_providers
    }

    #[must_use]
    pub fn llm_provider(&self, id: ProviderId) -> Option<Arc<dyn LlmProvider>> {
        self.llm_providers
            .iter()
            .find(|p| p.id() == id)
            .map(Arc::clone)
    }

    #[must_use]
    pub fn budget(&self) -> &Arc<Budget> {
        &self.budget
    }

    /// Point the assistant at a provider and model.
    pub fn set_assistant(&self, provider: ProviderId, model: String) {
        if let Ok(mut choice) = self.assistant.lock() {
            choice.provider = provider;
            choice.model = model;
            // Pricing belongs to the model, so a change invalidates it until
            // the next model list says otherwise. Unknown beats stale.
            choice.pricing = (None, None);
        }
    }

    /// Record what the chosen model costs, per million tokens.
    pub fn set_assistant_pricing(&self, input: Option<f64>, output: Option<f64>) {
        if let Ok(mut choice) = self.assistant.lock() {
            choice.pricing = (input, output);
        }
    }

    /// The provider, model and pricing to use for the next question.
    ///
    /// Falls back to whatever provider exists if the chosen one is missing, so
    /// a stale selection cannot leave the assistant permanently broken.
    /// Returns `None` only when no provider could be constructed at all.
    #[must_use]
    pub fn assistant_selection(&self) -> Option<AssistantSelection> {
        let choice = self.assistant.lock().map(|c| c.clone()).unwrap_or_default();
        let provider = self
            .llm_provider(choice.provider)
            .or_else(|| self.llm_providers.first().map(Arc::clone))?;
        Some(AssistantSelection {
            provider,
            model: choice.model,
            input_price: choice.pricing.0,
            output_price: choice.pricing.1,
        })
    }

    #[must_use]
    pub fn assistant_state(&self) -> crate::assistant::AssistantStateDto {
        let choice = self.assistant.lock().map(|c| c.clone()).unwrap_or_default();
        crate::assistant::AssistantStateDto {
            provider: choice.provider.slug(),
            model: choice.model,
            spent_usd: self.budget.spent_usd(),
            cap_usd: self.budget.cap_usd(),
            unpriced_calls: self.budget.unpriced_calls(),
        }
    }

    #[must_use]
    pub fn secrets(&self) -> &Arc<dyn SecretStore> {
        &self.secrets
    }

    #[must_use]
    pub fn secrets_persist(&self) -> bool {
        self.secrets_persist
    }

    #[must_use]
    pub fn sources(&self) -> &Arc<SourceRegistry> {
        &self.sources
    }

    #[must_use]
    pub fn waveforms(&self) -> &Arc<WaveformStore> {
        &self.waveforms
    }

    #[must_use]
    pub fn bus(&self) -> &Arc<ActionBus<Command>> {
        &self.bus
    }

    #[must_use]
    pub fn taps(&self) -> &crate::grid::TapTracker {
        &self.taps
    }

    pub fn registry(&self) -> Arc<ParameterRegistry> {
        Arc::clone(&self.registry)
    }

    #[must_use]
    pub fn host(&self) -> &AudioHost {
        &self.host
    }

    pub fn set_sample_name(&self, bank: u8, slot: u8, name: String) {
        if let Ok(mut map) = self.sample_names.lock() {
            map.insert((bank, slot), name);
        }
    }

    pub fn clear_sample_name(&self, bank: u8, slot: u8) {
        if let Ok(mut map) = self.sample_names.lock() {
            map.remove(&(bank, slot));
        }
    }

    /// The whole map, shared, for the snapshot pump.
    #[must_use]
    pub fn sample_names(&self) -> Arc<Mutex<HashMap<(u8, u8), String>>> {
        Arc::clone(&self.sample_names)
    }

    /// What is in one slot, by name. `None` for an empty one.
    #[must_use]
    pub fn sample_name_of(&self, bank: u8, slot: u8) -> Option<String> {
        self.sample_names.lock().ok()?.get(&(bank, slot)).cloned()
    }

    /// Remember what was opened, so anything can ask later.
    pub fn set_active_device(&self, device: Option<crate::commands::ActiveDeviceDto>) {
        if let Ok(mut slot) = self.active_device.lock() {
            *slot = device;
        }
    }

    #[must_use]
    pub fn active_device(&self) -> Option<crate::commands::ActiveDeviceDto> {
        self.active_device.lock().ok()?.clone()
    }

    /// Record the bridge for the stream that was just opened, or clear it when
    /// the new stream is a single-device one.
    pub fn set_bridge(&self, bridge: Option<Arc<dj_audio::BridgeStats>>) {
        if let Ok(mut slot) = self.bridge.lock() {
            *slot = bridge;
        }
    }

    #[must_use]
    pub fn bridge(&self) -> Option<Arc<dj_audio::BridgeStats>> {
        self.bridge.lock().ok()?.clone()
    }

    /// A live handle for the snapshot pump.
    ///
    /// Shared rather than copied, because the bridge is replaced every time a
    /// device is opened and a pump holding a stale copy would keep reporting
    /// drift for a stream that has already been closed.
    #[must_use]
    pub fn bridge_handle(&self) -> Arc<Mutex<Option<Arc<dj_audio::BridgeStats>>>> {
        Arc::clone(&self.bridge)
    }

    #[must_use]
    pub fn analysis(&self) -> &Arc<crate::analysis::AnalysisStore> {
        &self.analysis
    }

    pub fn set_deck_track(&self, deck: dj_core::DeckId, info: LoadedTrackInfo) {
        if let Ok(mut map) = self.deck_tracks.lock() {
            map.insert(deck.human_number(), info);
        }
    }

    /// The content hash of what is on a deck, if anything.
    #[must_use]
    pub fn deck_track_id(&self, deck: dj_core::DeckId) -> Option<dj_core::TrackId> {
        self.deck_tracks
            .lock()
            .ok()?
            .get(&deck.human_number())
            .map(|loaded| loaded.id)
    }

    pub fn clear_deck_track(&self, deck: dj_core::DeckId) {
        if let Ok(mut map) = self.deck_tracks.lock() {
            map.remove(&deck.human_number());
        }
    }

    #[must_use]
    pub fn deck_tracks(&self) -> Arc<Mutex<HashMap<u8, LoadedTrackInfo>>> {
        Arc::clone(&self.deck_tracks)
    }

    /// Whether `id` is still the track on `deck`.
    ///
    /// The guard against a slow analysis landing on a track that has already
    /// been replaced.
    #[must_use]
    pub fn deck_still_holds(&self, deck: dj_core::DeckId, id: dj_core::TrackId) -> bool {
        self.deck_tracks
            .lock()
            .map(|map| map.get(&deck.human_number()).is_some_and(|t| t.id == id))
            .unwrap_or(false)
    }

    #[must_use]
    pub fn deck_count(&self) -> usize {
        DECK_COUNT
    }
}

/// Put the application's resting values into the registry.
///
/// A freshly-constructed registry is all zeros, and no engine exists until a
/// device is opened -- so without this the interface would open showing every
/// fader at zero and every deck at rate 0.0, which is not what the application
/// will actually do once it starts. Seeding is not cosmetic: the UI reflects
/// the registry, so the registry has to be honest before audio exists.
fn seed_defaults(registry: &ParameterRegistry) {
    use dj_core::param::DeckParam;
    use dj_core::{CrossfaderAssign, DeckId, ParamId};

    for id in DeckId::all() {
        registry.set(ParamId::Deck(id, DeckParam::Volume), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::Rate), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::GainDb), 0.0);
        registry.set(ParamId::Deck(id, DeckParam::EqLow), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::EqMid), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::EqHigh), 1.0);
        // Zero is a *legitimate* value for this one -- it means "through" --
        // so an unseeded registry does not read as unset, it reads as a wrong
        // answer: the switch would show every deck off the crossfader when the
        // engine will in fact put deck 1 left and deck 2 right.
        registry.set(
            ParamId::Deck(id, DeckParam::CrossfaderAssign),
            CrossfaderAssign::default_for(id.index()).as_param(),
        );
        // The same trap, and the second time this project has fallen into it:
        // frame zero is a real playhead position, so an unseeded zero here
        // does not read as "nothing is being slipped over" -- it reads as
        // "the track will land at the very start", on every deck, before
        // anything has happened.
        registry.set(ParamId::Deck(id, DeckParam::SlipPosition), NOT_SLIPPING);
    }
}

/// What [`DeckParam::SlipPosition`] reads when nothing is being slipped over.
///
/// Outside the track and unambiguous, because zero is a real position. The
/// engine publishes it and the snapshot reads it, so the two cannot disagree
/// about what "none" looks like.
pub const NOT_SLIPPING: f32 = -1.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this covers: the interface kept its own note of what was open,
    /// set only by its Connect button, and showed "no device" over playing
    /// audio whenever anything else opened one. The engine's account has to be
    /// available to ask for, or the interface has no way to correct itself.
    #[test]
    fn what_is_open_can_be_asked_for_by_anyone_not_only_by_whoever_opened_it() {
        let state = AppState::new(true);
        assert!(state.active_device().is_none(), "nothing open yet");

        state.set_active_device(Some(crate::commands::ActiveDeviceDto {
            name: "Null output (no hardware)".to_owned(),
            sample_rate: 48_000,
            buffer_frames: 256,
            channels: 4,
            latency_ms: 5.333_333,
            cue: None,
            cue_error: None,
        }));
        let open = state.active_device().expect("a device is open");
        assert_eq!(open.sample_rate, 48_000);
        assert_eq!(open.name, "Null output (no hardware)");

        // Closing has to be sayable too, or the interface would show a device
        // that has gone.
        state.set_active_device(None);
        assert!(state.active_device().is_none());
    }

    // -- the chosen layout -------------------------------------------------

    #[test]
    fn the_chosen_layout_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();

        let first = AppState::new(true);
        first.set_config_dir(dir.path().to_path_buf());
        assert_eq!(first.chosen_layout(), None, "nothing chosen yet");
        first.set_chosen_layout("Performance");

        // A second application, reading the same directory, as a restart would.
        let second = AppState::new(true);
        second.set_config_dir(dir.path().to_path_buf());
        assert_eq!(second.chosen_layout().as_deref(), Some("Performance"));
    }

    #[test]
    fn a_layout_directory_is_made_beside_the_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        assert_eq!(state.layout_dir(), Some(dir.path().join("layouts")));
        assert!(dir.path().join("layouts").is_dir());
    }

    /// Before `setup` has run there is nowhere to write, and asking must not
    /// panic or invent a path — the interface simply draws its own defaults.
    #[test]
    fn choosing_a_layout_before_there_is_anywhere_to_put_it_is_harmless() {
        let state = AppState::new(true);
        state.set_chosen_layout("Pro");
        assert_eq!(state.chosen_layout(), None);
    }

    /// An empty file is "no choice", not a layout named "".
    #[test]
    fn an_empty_choice_file_reads_as_no_choice() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("layout.txt"), "  \n ").unwrap();
        let state = AppState::new(true);
        state.set_config_dir(dir.path().to_path_buf());
        assert_eq!(state.chosen_layout(), None);
    }

    #[test]
    fn state_starts_with_sensible_defaults() {
        let state = AppState::new(true);
        let snapshot = crate::Snapshot::capture(&state.registry, state.deck_count());
        assert_eq!(snapshot.decks.len(), DECK_COUNT);
        assert!(!snapshot.decks[0].loaded);
        assert!(!snapshot.decks[0].playing);
        // Faders must not read zero before a device is open.
        assert_eq!(snapshot.decks[0].volume, 1.0);
        assert_eq!(snapshot.decks[0].rate, 1.0);
    }

    /// The one default where zero is a real value rather than an absence, so a
    /// missing seed shows as a confident wrong answer instead of an empty one.
    #[test]
    fn the_crossfader_switch_reads_right_before_a_device_is_open() {
        use dj_core::CrossfaderAssign;

        let state = AppState::new(true);
        let snapshot = crate::Snapshot::capture(&state.registry, state.deck_count());
        assert_eq!(snapshot.decks[0].crossfader_assign, CrossfaderAssign::Left);
        assert_eq!(snapshot.decks[1].crossfader_assign, CrossfaderAssign::Right);
        for deck in &snapshot.decks[2..] {
            assert_eq!(deck.crossfader_assign, CrossfaderAssign::Thru);
        }
    }

    #[test]
    fn the_host_starts_with_the_null_backend() {
        let state = AppState::new(true);
        let devices = state.host().list_devices().unwrap();
        assert_eq!(
            devices.len(),
            2,
            "null backend exposes a master and a headphone device"
        );
    }
}

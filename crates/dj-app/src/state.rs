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

/// Decks the engine runs.
///
/// Six, which is [`dj_core::MAX_DECKS`] — the engine builds them all and the
/// interface shows two, four or six. An idle deck costs a few hundred bytes and
/// a branch per block that returns immediately, so there is no reason to build
/// fewer than the maximum and every reason not to make the number a setting
/// that can disagree with the interface.
pub const DECK_COUNT: usize = dj_core::MAX_DECKS;

/// The separation model the application looks for in its data directory.
///
/// Not bundled: the model is tens of megabytes and its licence is separate
/// from ours, so it is a download rather than part of the package. A machine
/// without it gets a mixer with the stem controls off and a sentence saying so.
const STEMS_MODEL_FILE: &str = "htdemucs.onnx";

/// How much disk separated audio may occupy before the oldest is evicted.
///
/// Separation is slow enough that re-doing it mid-set would be felt, so the
/// cache is generous: two gigabytes is a few hours of material and a rounding
/// error next to a music library.
const STEMS_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Everything a command handler needs.
///
/// Deliberately small: a bus to send intent into, a registry to read state from,
/// and a handle to the thread that owns the audio device. There is no direct
/// reference to the engine, because nothing outside the audio thread is allowed
/// to touch it.
#[derive(Debug)]
pub struct AppState {
    bus: Arc<ActionBus<Command>>,
    /// How much the assistant is doing, what the night is, and which controls
    /// the human has taken.
    ///
    /// Distinct from `assistant`, which is the *provider* choice -- which model
    /// answers a question. This is what the assistant is allowed to do with the
    /// answer, which is a different decision entirely.
    ///
    /// One lock rather than three: the three are read together on every
    /// autopilot tick and changed together when a pack is chosen, and separate
    /// locks would let a tick see a new posture against an old occasion.
    conduct: Arc<Mutex<Conduct>>,
    registry: Arc<ParameterRegistry>,
    /// The network control server. Off until a DJ switches it on; see
    /// `crate::remote` for why that is not a preference.
    remote: Arc<crate::remote::Remote>,
    /// The room's own page, running or not. See `crate::audience`.
    audience: Arc<crate::audience::Audience>,
    /// What the room has been doing, when anything is watching it.
    room: Arc<Mutex<dj_assistant::room::Room>>,
    /// Tempo sync with other djmanzo instances. Off until a DJ switches it
    /// on; see `crate::peersync`.
    peers: Arc<crate::peersync::Peers>,
    /// MIDI clock out, when djmanzo is the clock master.
    clock: Arc<crate::clock::MidiClock>,
    /// MIDI clock in, when something else is.
    clock_follow: Arc<crate::clock::ClockFollow>,
    /// Tap-tempo runs in progress. Lives here rather than on the audio thread
    /// because a run is host state -- see `crate::grid`.
    taps: crate::grid::TapTracker,
    /// Where the DJ's own layout files live. `None` until `setup` resolves it,
    /// and then only the built-in layouts are available.
    layout_dir: Mutex<Option<std::path::PathBuf>>,
    /// The mapping being built in the editor.
    ///
    /// One at a time, and held here rather than in the interface so that a
    /// half-finished mapping survives closing the panel -- a DJ mapping a
    /// controller looks things up mid-way through.
    mapping_draft: Mutex<dj_hid::editor::Draft>,
    /// The configuration directory itself, which is the layout directory's
    /// parent. Held rather than derived: walking back up out of a
    /// subdirectory to find where you started is the sort of implicit
    /// coupling that breaks silently the first time either path moves.
    config_dir: Mutex<Option<std::path::PathBuf>>,
    /// The set being recorded, if one is. See [`crate::setrec`].
    recording: Mutex<Option<crate::setrec::Recording>>,
    /// Read by the snapshot pump sixty times a second, so it is held outside
    /// the lock above rather than reached for through it.
    recording_state: Arc<crate::setrec::RecordingState>,
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
    /// Controllers and the keyboard. See [`crate::control`].
    control: Arc<crate::control::ControlHub>,
    /// Which panels are on screens of their own. See [`crate::monitors`].
    detached: Arc<Mutex<crate::monitors::Detached>>,
    /// The master's plugin insert. See [`crate::plugins`].
    ///
    /// A handle rather than the thing itself: a CLAP plugin instance is `!Send`
    /// by design, so it lives on a thread of its own and this is how the rest
    /// of the application talks to it.
    plugin: crate::plugins::PluginHandle,
    /// The automix, when the DJ has handed the mix over.
    ///
    /// A mutex rather than atomics because it is a state machine, and it is
    /// only ever touched from the snapshot pump and from `perform` — sixty
    /// times a second and on a button press. Nothing realtime is behind it.
    automix: Arc<Mutex<crate::automix::Automix>>,
    /// The other end of the controller queue, until `setup` can start the
    /// thread that drains it. Held rather than dropped: a receiver dropped
    /// here would make every MIDI message a send into a closed channel.
    control_inbox: Mutex<Option<std::sync::mpsc::Receiver<String>>>,
    /// Background worker for separating tracks into stems.
    ///
    /// Behind a mutex because it is replaced, not mutated: the application
    /// starts before Tauri can tell it where its data directory is, so the
    /// worker begins life unavailable and is swapped for a real one once
    /// [`AppState::open_stems`] has somewhere to look for a model.
    stems_worker: Mutex<Arc<dj_stems::worker::SeparationWorker>>,
    /// Why separation is or is not available, in a sentence the interface can
    /// show. `None` once a model is loaded and running.
    stems_reason: Mutex<Option<String>>,
    /// Which separator is running, for the interface to name. `None` when
    /// none is.
    stems_backend: Mutex<Option<&'static str>>,
    /// How many decks are going out on pairs of their own, if any.
    ///
    /// Remembered for the same reason `stem_out` is: a fresh device means a
    /// fresh engine that has never heard of it.
    deck_out: Mutex<Option<usize>>,
    /// The deck being sent out in parts, if one is. See [`AppState::set_stem_out`].
    ///
    /// Remembered here for the reason the controller routing is: opening an
    /// audio device builds a **fresh engine**, and a selection sent to the
    /// previous one is simply gone. Without this, plugging in a different
    /// interface would silently drop the DJ back to a normal mix while the
    /// panel still claimed the stems were going out.
    stem_out: Mutex<Option<dj_core::DeckId>>,
    /// What each deck's control record is, for the decks on one.
    ///
    /// The host owns the input stream; this owns the words for it. Held here
    /// rather than asked of the host because what the interface needs to say --
    /// which record, and whether the deck is in absolute mode -- is not
    /// recoverable from an open device.
    timecode: Mutex<[Option<TimecodeSetup>; dj_core::MAX_DECKS]>,
}

/// One deck's control-record setup, as the interface needs to describe it.
#[derive(Debug, Clone, PartialEq)]
pub struct TimecodeSetup {
    /// The record djmanzo is reading.
    pub format: dj_dvs::TimecodeFormat,
    /// The input the turntable is arriving on.
    pub device: String,
    /// True for absolute mode, where where the needle sits on the record is
    /// where the playhead sits in the track. False for relative, where only
    /// the movement is followed and a lift-and-drop changes nothing.
    pub absolute: bool,
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

/// How the assistant conducts itself: how much it does, what the night is, and
/// what the human has taken out of its hands.
///
/// Deliberately small and `Clone`-free: it is read under a lock on every tick
/// and the temptation to hang more on it should meet friction.
#[derive(Debug)]
pub struct Conduct {
    pub posture: dj_assistant::Posture,
    pub occasion: dj_assistant::Occasion,
    pub takeover: dj_assistant::Takeover,
    /// The set the assistant is working through, if one was built.
    ///
    /// Track ids rather than the full slots: the assistant needs to know what
    /// comes next, and the reasoning that placed each one has already been
    /// read by whoever accepted the set. Keeping the whole thing here would be
    /// a second copy of the library's opinion that could drift from it.
    pub setlist: Vec<dj_core::TrackId>,
    /// How far through `setlist` the night has got.
    ///
    /// Advanced when a record from the list actually reaches a deck, not when
    /// one is chosen — a staged track that the DJ ejects was never played, and
    /// counting it would silently skip a record.
    pub played: usize,
}

impl Default for Conduct {
    fn default() -> Self {
        Self {
            // Suggest, not Off. A DJ who has never opened the panel should get
            // the thing that is useful and cannot surprise them -- and one who
            // wants silence has an Off to choose, whereas one who never
            // discovers the feature never chooses anything.
            posture: dj_assistant::Posture::Suggest,
            occasion: dj_assistant::Occasion::Open,
            takeover: dj_assistant::Takeover::new(),
            setlist: Vec::new(),
            played: 0,
        }
    }
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
        // The names live here, so the host is handed a way to write one rather
        // than a handle on the map -- the same reason the sampler's own loads
        // record their names here and not in the engine.
        let sample_names: Arc<Mutex<HashMap<(u8, u8), String>>> =
            Arc::new(Mutex::new(HashMap::new()));
        // Started before the host, because the host's retirement drain has to
        // be able to hand plugin processors back to it.
        let plugin = crate::plugins::PluginHandle::start();
        let host = {
            let names = Arc::clone(&sample_names);
            let insert = plugin.clone();
            AudioHost::start(
                Arc::clone(&bus),
                Arc::clone(&registry),
                use_null_backend,
                Box::new(move |bank, slot, name| {
                    if let Ok(mut map) = names.lock() {
                        map.insert((bank, slot), name);
                    }
                }),
                // Deactivation happens on the thread that owns the instance,
                // which is neither this one nor the audio one. All that happens
                // here is a send.
                Box::new(move |processor| insert.retire(processor)),
            )
        };

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

        let (mut control, control_inbox) = crate::control::ControlHub::new();
        // So a mapping's script can ask what the engine is doing. Set here
        // rather than taken by `new`, because the hub is built before the
        // registry exists.
        control.set_registry(Arc::clone(&registry));

        // Separation starts unavailable and is attached later, once `setup`
        // knows where the application's data lives. Nothing here may panic:
        // a DJ with no separation model still has a mixer, and refusing to
        // start would take the whole set down over a feature they were not
        // using. Same reasoning as the HTTP client above.
        let stems_worker = Mutex::new(Arc::new(dj_stems::worker::SeparationWorker::unavailable()));
        let stems_backend = Mutex::new(None);
        let stems_reason = Mutex::new(Some("separation has not been set up yet".to_owned()));

        Self {
            bus,
            conduct: Arc::new(Mutex::new(Conduct::default())),
            registry,
            remote: Arc::new(crate::remote::Remote::default()),
            audience: Arc::new(crate::audience::Audience::default()),
            room: Arc::new(Mutex::new(dj_assistant::room::Room::new())),
            peers: Arc::new(crate::peersync::Peers::default()),
            clock: Arc::new(crate::clock::MidiClock::default()),
            clock_follow: Arc::new(crate::clock::ClockFollow::default()),
            taps: crate::grid::TapTracker::new(),
            layout_dir: Mutex::new(None),
            mapping_draft: Mutex::new(dj_hid::editor::Draft::new("My mapping", String::new())),
            config_dir: Mutex::new(None),
            recording: Mutex::new(None),
            recording_state: Arc::new(crate::setrec::RecordingState::default()),
            host,
            waveforms: Arc::new(WaveformStore::new()),
            secrets,
            secrets_persist,
            sources,
            llm_providers,
            assistant: Mutex::new(AssistantChoice::default()),
            budget: Arc::new(Budget::default()),
            presets: PresetLibrary::builtin(),
            sample_names,
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
            control: Arc::new(control),
            detached: Arc::new(Mutex::new(crate::monitors::Detached::default())),
            plugin,
            automix: Arc::new(Mutex::new(crate::automix::Automix::new())),
            control_inbox: Mutex::new(Some(control_inbox)),
            stems_worker,
            stems_backend,
            stem_out: Mutex::new(None),
            deck_out: Mutex::new(None),
            timecode: Mutex::new([const { None }; dj_core::MAX_DECKS]),
            stems_reason,
        }
    }

    /// The track database.
    #[must_use]
    pub fn library(&self) -> Arc<crate::library::LibraryHandle> {
        Arc::clone(&self.library)
    }

    /// Background stem separator.
    ///
    /// Always returns a worker. When separation is unavailable that worker
    /// drops the chunks it is given, so callers never have to branch: the
    /// difference between "separating" and "not separating" belongs in the
    /// interface, next to [`AppState::stems_reason`], not scattered through
    /// every call site.
    #[must_use]
    pub fn stems_worker(&self) -> Arc<dj_stems::worker::SeparationWorker> {
        self.stems_worker
            .lock()
            .map(|worker| Arc::clone(&worker))
            .unwrap_or_else(|_| Arc::new(dj_stems::worker::SeparationWorker::unavailable()))
    }

    /// Why the higher-quality separator is not in use, or `None` when it is.
    ///
    /// Not the same question as "are stems available": the built-in separator
    /// always is, so this reads as an explanation of what is missing rather
    /// than as a failure.
    #[must_use]
    pub fn stems_reason(&self) -> Option<String> {
        self.stems_reason
            .lock()
            .ok()
            .and_then(|reason| reason.clone())
    }

    /// Which separator is running, if any.
    #[must_use]
    pub fn stems_backend(&self) -> Option<&'static str> {
        self.stems_backend.lock().ok().and_then(|name| *name)
    }

    /// Start separation, preferring a downloaded model and falling back to the
    /// built-in separator.
    ///
    /// # Why there is a fallback at all
    ///
    /// The model is a download, so on a fresh install there is none -- and
    /// "stems, once you have found and installed a 60 MB file" is not a
    /// feature a DJ can use on the night. The built-in separator is
    /// arithmetic: no model, no runtime, nothing to fetch, and it works on
    /// every machine. It is not as good as HTDemucs and the interface says so,
    /// but a usable acapella now beats a better one after a restart.
    ///
    /// Every failure is recorded as a sentence rather than raised: this runs
    /// during startup, and there is no useful way to fail it.
    /// `DJMANZO_STEMS_MODEL` overrides where the model is looked for.
    pub fn open_stems(&self, dir: &std::path::Path) {
        let model = match std::env::var_os("DJMANZO_STEMS_MODEL") {
            Some(path) => std::path::PathBuf::from(path),
            None => dir.join("models").join(STEMS_MODEL_FILE),
        };

        let (separator, reason): (Arc<dyn dj_stems::stems::Separator>, Option<String>) =
            match dj_stems::StemsEngine::new(&model) {
                Ok(engine) => (Arc::new(engine), None),
                Err(unavailable) => {
                    tracing::info!(%unavailable, "using the built-in separator");
                    (
                        Arc::new(dj_stems::hpss::Hpss),
                        Some(unavailable.to_string()),
                    )
                }
            };
        let name = dj_stems::stems::Separator::name(separator.as_ref());

        let cache_dir = dir.join("stems-cache");
        let cache = match dj_stems::cache::StemCache::new(&cache_dir, STEMS_CACHE_BYTES) {
            Ok(cache) => Arc::new(cache),
            Err(error) => {
                tracing::warn!(%error, ?cache_dir, "no stem cache; separation stays off");
                self.set_stems_reason(Some(format!(
                    "the stem cache at {} could not be opened: {error}",
                    cache_dir.display()
                )));
                return;
            }
        };

        let worker = Arc::new(dj_stems::worker::SeparationWorker::new(separator, cache));
        if let Ok(mut slot) = self.stems_worker.lock() {
            *slot = worker;
        }
        if let Ok(mut slot) = self.stems_backend.lock() {
            *slot = Some(name);
        }
        self.set_stems_reason(reason);
        tracing::info!(separator = name, "stem separation is ready");
    }

    fn set_stems_reason(&self, reason: Option<String>) {
        if let Ok(mut slot) = self.stems_reason.lock() {
            *slot = reason;
        }
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

    /// Where user mappings live: beside the presets and layouts, because they
    /// are things a DJ made rather than things the system may reclaim.
    #[must_use]
    pub fn mappings_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.config_dir.lock().ok()?.clone()?.join("mappings"))
    }

    /// A copy of the mapping being edited.
    #[must_use]
    pub fn mapping_draft(&self) -> dj_hid::editor::Draft {
        self.mapping_draft
            .lock()
            .map(|draft| draft.clone())
            .unwrap_or_else(|_| dj_hid::editor::Draft::new("My mapping", String::new()))
    }

    /// Change the draft, returning whatever the change returned.
    ///
    /// `None` only if the lock is poisoned, which means a previous edit
    /// panicked. Reported rather than papered over with a default: a mapping
    /// editor that silently discarded a binding would be worse than one that
    /// says it is broken.
    pub fn edit_mapping_draft<T>(
        &self,
        change: impl FnOnce(&mut dj_hid::editor::Draft) -> T,
    ) -> Option<T> {
        self.mapping_draft
            .lock()
            .ok()
            .map(|mut draft| change(&mut draft))
    }

    /// Begin a new draft, optionally from a mapping that already exists.
    ///
    /// Starting from an existing mapping is the common case: most controllers
    /// are nearly one of the bundled ones, and a DJ wants to move four pads
    /// rather than name sixty from scratch.
    pub fn start_mapping_draft(&self, from: Option<&str>) {
        let fresh = match from {
            Some(name) => self
                .control()
                .mapping_named(name)
                .map(|mapping| dj_hid::editor::Draft::from_mapping(&mapping)),
            None => None,
        }
        .unwrap_or_else(|| dj_hid::editor::Draft::new("My mapping", String::new()));

        if let Ok(mut draft) = self.mapping_draft.lock() {
            *draft = fresh;
        }
    }

    /// Where the DJ's own layout files live, once Tauri can say.
    #[must_use]
    pub fn layout_dir(&self) -> Option<std::path::PathBuf> {
        self.layout_dir.lock().ok()?.clone()
    }

    // -- recording the set -------------------------------------------------

    /// Where recordings go: `recordings/` beside the settings.
    ///
    /// Not the system's music folder, deliberately. A DJ's music folder is
    /// their *library*, and dropping hour-long captures of a set into it means
    /// the browser finds them and offers them as tracks to play.
    #[must_use]
    pub fn recordings_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.config_dir.lock().ok()?.clone()?.join("recordings"))
    }

    /// Whether a recording is running.
    #[must_use]
    pub fn is_recording(&self) -> bool {
        self.recording.lock().is_ok_and(|slot| slot.is_some())
    }

    /// Start recording the master at `sample_rate`.
    ///
    /// Returns the file it opened. Refuses rather than restarting if one is
    /// already running: a second press of a record button means "I meant it",
    /// not "throw the last twenty minutes away".
    pub fn start_recording(&self, sample_rate: u32) -> Result<std::path::PathBuf, String> {
        let mut slot = self
            .recording
            .lock()
            .map_err(|_| "the recorder is wedged".to_owned())?;
        if let Some(running) = slot.as_ref() {
            return Ok(running.path().to_path_buf());
        }

        let dir = self
            .recordings_dir()
            .ok_or_else(|| "no settings folder to record into yet".to_owned())?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

        // Named for when it started, in a form that sorts chronologically and
        // survives every filesystem this runs on.
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("set-{started}.wav"));

        let (sink, samples) =
            rtrb::RingBuffer::<f32>::new(crate::setrec::Recording::ring_capacity(sample_rate));
        let recording = crate::setrec::Recording::start(
            &path,
            sample_rate,
            samples,
            Arc::clone(&self.recording_state),
        )
        .map_err(|e| e.to_string())?;

        // The engine only starts writing once it has the ring, so the file
        // cannot be opened and then never fed.
        self.bus()
            .send_command(dj_engine::Command::RecordStream { sink: Some(sink) })
            .map_err(|_| "the engine is not listening".to_owned())?;

        let path = recording.path().to_path_buf();
        *slot = Some(recording);
        Ok(path)
    }

    /// Stop, and hand back the finished file.
    pub fn stop_recording(&self) -> Option<std::path::PathBuf> {
        let recording = self.recording.lock().ok()?.take()?;
        // The engine first: it stops handing samples over, and the ring's other
        // half comes back through the retirement queue rather than being
        // dropped on the audio thread.
        let _ = self
            .bus()
            .send_command(dj_engine::Command::RecordStream { sink: None });
        Some(recording.finish())
    }

    /// The live counters, for the snapshot pump.
    #[must_use]
    pub fn recording_state(&self) -> Arc<crate::setrec::RecordingState> {
        Arc::clone(&self.recording_state)
    }

    /// Where the running recording is going, for the interface to name.
    #[must_use]
    pub fn recording_path(&self) -> Option<String> {
        let slot = self.recording.lock().ok()?;
        let recording = slot.as_ref()?;
        Some(
            recording
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| recording.path().display().to_string()),
        )
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

    /// Controllers and the keyboard.
    #[must_use]
    pub fn control(&self) -> &Arc<crate::control::ControlHub> {
        &self.control
    }

    /// Take the controller queue's receiving end, once.
    ///
    /// Returns `None` on any call after the first, which is what a second
    /// drain thread would be: two threads racing for the same actions, in an
    /// order neither controls.
    pub fn take_control_inbox(&self) -> Option<std::sync::mpsc::Receiver<String>> {
        self.control_inbox.lock().unwrap().take()
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

    /// The network control server, running or not.
    #[must_use]
    pub fn remote(&self) -> &Arc<crate::remote::Remote> {
        &self.remote
    }

    /// The page the room sees, running or not.
    #[must_use]
    pub fn audience(&self) -> &Arc<crate::audience::Audience> {
        &self.audience
    }

    /// What the sensors have made of the room tonight.
    #[must_use]
    pub fn room(&self) -> &Arc<Mutex<dj_assistant::room::Room>> {
        &self.room
    }

    /// The MIDI clock, sending or not.
    #[must_use]
    pub fn clock(&self) -> &Arc<crate::clock::MidiClock> {
        &self.clock
    }

    /// Tempo sync with other djmanzo instances.
    #[must_use]
    pub fn peers(&self) -> &Arc<crate::peersync::Peers> {
        &self.peers
    }

    /// The MIDI clock djmanzo is following, if any.
    #[must_use]
    pub fn clock_follow(&self) -> &Arc<crate::clock::ClockFollow> {
        &self.clock_follow
    }

    #[must_use]
    pub fn bus(&self) -> &Arc<ActionBus<Command>> {
        &self.bus
    }

    /// Send one deck out in parts, or stop.
    ///
    /// Refused by the engine on a device with fewer than eight outputs, which
    /// is a fact about the device rather than about what the DJ wants — so the
    /// choice is remembered either way and takes effect if a wider device is
    /// opened later.
    pub fn set_stem_out(&self, deck: Option<dj_core::DeckId>) {
        *self.stem_out.lock().unwrap_or_else(|e| e.into_inner()) = deck;
        // They want the same sockets, so choosing one puts the other away.
        if deck.is_some() {
            *self.deck_out.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        self.apply_stem_out();
        self.apply_deck_out();
    }

    /// Send every deck out on a pair of its own, or stop.
    ///
    /// Mutually exclusive with stem out, which wants the same sockets — and
    /// the exclusion is applied here as well as in the engine, so the panel
    /// never shows both switched on while the audio can only be one of them.
    pub fn set_deck_out(&self, decks: Option<usize>) {
        let decks = decks.filter(|count| *count > 0);
        *self.deck_out.lock().unwrap_or_else(|e| e.into_inner()) = decks;
        if decks.is_some() {
            *self.stem_out.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        self.apply_stem_out();
        self.apply_deck_out();
    }

    /// How many decks are going out on pairs of their own, if any.
    #[must_use]
    pub fn deck_out(&self) -> Option<usize> {
        *self.deck_out.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Put the remembered per-deck arrangement on the engine.
    ///
    /// Called after every audio device open, for the reason
    /// [`AppState::apply_stem_out`] is.
    pub fn apply_deck_out(&self) {
        let decks = self.deck_out();
        let _ = self.bus().send_command(Command::SetDeckOut { decks });
    }

    /// What is on one deck's timecode input, if anything.
    #[must_use]
    pub fn timecode(&self, deck: dj_core::DeckId) -> Option<TimecodeSetup> {
        self.timecode.lock().unwrap_or_else(|e| e.into_inner())[deck.index()].clone()
    }

    /// Every deck's timecode setup, in deck order.
    #[must_use]
    pub fn timecode_all(&self) -> Vec<Option<TimecodeSetup>> {
        self.timecode
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .to_vec()
    }

    /// Record that a deck is on a control record, or is no longer on one.
    ///
    /// Bookkeeping only: the input is opened and closed by the host, and this
    /// is called after that has succeeded. Calling it without opening anything
    /// would leave the panel claiming a deck follows a record that nothing is
    /// feeding.
    pub fn set_timecode(&self, deck: dj_core::DeckId, setup: Option<TimecodeSetup>) {
        self.timecode.lock().unwrap_or_else(|e| e.into_inner())[deck.index()] = setup;
    }

    /// Forget every deck's control record.
    ///
    /// Called after a device change, because opening an output builds a fresh
    /// engine and the host closes every input along with the old one. Unlike
    /// the stem routing, this is *not* re-applied afterwards: re-opening an
    /// input device without being asked would start a turntable driving a deck
    /// at a moment the DJ was setting up their sound card, and a deck that
    /// starts moving on its own is worse than one that has to be switched back
    /// on.
    pub fn clear_timecode(&self) {
        *self.timecode.lock().unwrap_or_else(|e| e.into_inner()) =
            [const { None }; dj_core::MAX_DECKS];
    }

    /// Which deck is being sent out in parts, if any.
    #[must_use]
    pub fn stem_out(&self) -> Option<dj_core::DeckId> {
        *self.stem_out.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Put the remembered stem-out selection on the engine.
    ///
    /// Called after every audio device open, alongside
    /// [`AppState::apply_controller_routing`] and for the same reason: the
    /// engine behind a freshly opened device has never heard of it.
    ///
    /// Silent on failure, because there is no engine to tell until a device is
    /// open and the next open calls this again.
    pub fn apply_stem_out(&self) {
        let deck = self.stem_out();
        let _ = self.bus().send_command(Command::SetStemOut { deck });
    }

    /// Put the open controller's output arrangement on the engine.
    ///
    /// Called both when a controller is opened or closed and after every audio
    /// device open, because opening a device builds a **fresh engine** with
    /// fresh queues -- so a routing sent to the previous one is simply gone.
    /// The hub is the thing that remembers, and this is how the engine is told
    /// again.
    ///
    /// Failure is silent on purpose: there is no engine to route to until a
    /// device is open, and the next device open calls this again.
    pub fn apply_controller_routing(&self) {
        let routing = self.control().routing();
        let _ = self.bus().send_command(Command::SetRouting { routing });
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

    /// Which panels are on screens of their own.
    #[must_use]
    pub fn detached(&self) -> crate::monitors::Detached {
        self.detached.lock().map(|d| d.clone()).unwrap_or_default()
    }

    pub fn detach_panel(&self, panel: crate::monitors::Panel) {
        if let Ok(mut detached) = self.detached.lock() {
            detached.add(panel);
        }
    }

    pub fn attach_panel(&self, panel: crate::monitors::Panel) {
        if let Ok(mut detached) = self.detached.lock() {
            detached.remove(panel);
        }
    }

    /// The master's plugin insert. See [`crate::plugins`].
    #[must_use]
    pub fn plugin(&self) -> &crate::plugins::PluginHandle {
        &self.plugin
    }

    /// The automix. See [`crate::automix`].
    #[must_use]
    pub fn automix(&self) -> &Arc<Mutex<crate::automix::Automix>> {
        &self.automix
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

    /// How the assistant is conducting itself, and what the human holds.
    ///
    /// Handed out as the `Arc` rather than copied, because the takeover inside
    /// it is mutated from wherever a human action arrives and read from the
    /// autopilot tick -- two different threads, one truth.
    #[must_use]
    pub fn conduct(&self) -> Arc<Mutex<Conduct>> {
        Arc::clone(&self.conduct)
    }

    /// Note that a human moved a control.
    ///
    /// Called for every action that arrives from a person -- the interface, a
    /// controller, the network -- and not for the assistant's own. That
    /// asymmetry is the whole mechanism: see `dj_assistant::takeover`.
    pub fn note_human_touch(&self, action: &dj_core::Action) {
        let Some(param) = action.touches() else {
            return;
        };
        if let Ok(mut conduct) = self.conduct.lock() {
            conduct.takeover.touched(param);
        }
    }

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

    // -- stem separation ---------------------------------------------------

    /// The bug this covers: `AppState::new` used to build the separation
    /// engine with `.expect("Failed to load Stems Engine")` against a path
    /// relative to the working directory. Every machine without that file --
    /// which is every machine, since the model is not bundled -- could not
    /// start the application at all.
    #[test]
    fn a_machine_with_no_separation_model_still_gets_a_mixer() {
        let state = AppState::new(true);
        // Nothing has been opened yet, so nothing is separating.
        assert!(state.stems_backend().is_none());
        // The worker is still there to be called, so nothing downstream has
        // to check first.
        let worker = state.stems_worker();
        worker.process_chunk(
            dj_core::track::TrackId::from_bytes([7u8; 32]),
            0,
            &[0.0; 64],
            0..32,
            48_000,
            None,
        );
    }

    /// A fresh install has no model, and must still be able to drop a vocal.
    /// Before the fallback existed this left separation switched off and the
    /// stem pads inert -- a feature that only worked after finding and
    /// installing a 60 MB file, which is not a feature a DJ can use on the
    /// night.
    #[test]
    fn with_no_model_the_built_in_separator_takes_over() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(true);
        state.open_stems(dir.path());

        let backend = state
            .stems_backend()
            .expect("something must be separating, even with no model");
        assert!(
            backend.contains("built-in"),
            "expected the fallback, got {backend}"
        );

        // And it says why the better one is not running, naming the runtime
        // or the model so the reader knows what to go and get.
        let reason = state.stems_reason().expect("the model is absent");
        assert!(
            reason.contains("ONNX Runtime") || reason.contains(STEMS_MODEL_FILE),
            "the reason should name the runtime or the model, got: {reason}"
        );
    }

    /// Whatever happens, the worker handle is never absent: callers push
    /// chunks at it unconditionally.
    #[test]
    fn the_worker_is_always_there_to_be_called() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(true);
        state.open_stems(dir.path());

        let track = dj_core::track::TrackId::from_bytes([9u8; 32]);
        let worker = state.stems_worker();
        for chunk in 0..4 {
            worker.process_chunk(track, chunk, &[0.25; 128], 0..64, 48_000, None);
        }
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
            3,
            "null backend exposes a master, a headphone device and a wide one"
        );
    }
}

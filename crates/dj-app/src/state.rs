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
    /// Live figures from the two-device bridge, present only while a split
    /// output is open. Replaced on every device change, so it never reports
    /// numbers from a stream that has already been closed.
    bridge: Arc<Mutex<Option<Arc<dj_audio::BridgeStats>>>>,
    analysis: Arc<crate::analysis::AnalysisStore>,
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
            host,
            waveforms: Arc::new(WaveformStore::new()),
            secrets,
            secrets_persist,
            sources,
            llm_providers,
            assistant: Mutex::new(AssistantChoice::default()),
            budget: Arc::new(Budget::default()),
            presets: PresetLibrary::builtin(),
            bridge: Arc::new(Mutex::new(None)),
            analysis: Arc::new(crate::analysis::AnalysisStore::new()),
            deck_tracks: Arc::new(Mutex::new(HashMap::new())),
        }
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
    pub fn registry(&self) -> Arc<ParameterRegistry> {
        Arc::clone(&self.registry)
    }

    #[must_use]
    pub fn host(&self) -> &AudioHost {
        &self.host
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
    use dj_core::{DeckId, ParamId};

    for id in DeckId::all() {
        registry.set(ParamId::Deck(id, DeckParam::Volume), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::Rate), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::GainDb), 0.0);
        registry.set(ParamId::Deck(id, DeckParam::EqLow), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::EqMid), 1.0);
        registry.set(ParamId::Deck(id, DeckParam::EqHigh), 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

//! The AI layer.
//!
//! The whole crate exists under one constraint, from
//! [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md):
//!
//! > **The assistant is a client, not a component. It can only emit action text
//! > onto the existing bus.**
//!
//! Everything here is arranged to make that true rather than merely intended.
//! The system prompt is *generated* from [`dj_core::vocabulary`], so a model can
//! only be told about actions that exist. Whatever it replies is run through
//! [`dj_core::Action::parse`], and what does not parse is rejected at the edge
//! and reported. A hallucination is a parse error — it cannot invent a deck,
//! exceed a range, or reach anywhere near the audio thread.
//!
//! # The cheap path first
//!
//! Most of what is said to an assistant mid-set is not interesting language:
//! "play deck two", "kill the bass", "para el uno". [`intent`] matches those
//! locally — no network, no key, no cost, an answer in microseconds, and it
//! keeps working when the venue's wifi does not. The model is kept for language
//! that genuinely needs understanding.
//!
//! # Providers
//!
//! Six, behind one trait. Four speak the OpenAI chat shape and share an
//! implementation ([`openai_compat`]); Anthropic and Google have their own
//! ([`native`]). A local model needs no key at all, which is the right default
//! for anyone who would rather not send their track list to a third party.

pub mod assistant;
pub mod authority;
pub mod budget;
pub mod catalog;
pub mod coach;
pub mod http;
pub mod intent;
pub mod native;
pub mod openai_compat;
pub mod posture;
pub mod provider;
pub mod room;
pub mod takeover;
pub mod technique;

pub use assistant::{Assistant, Plan, Source, extract_actions, system_prompt};
pub use authority::{Allowance, Capability};
pub use budget::Budget;
pub use catalog::{ProviderInfo, catalog, info};
pub use coach::{Footing, Moment, Note, Observed};
pub use http::{HttpJson, ReqwestJson};
pub use native::{AnthropicProvider, GoogleProvider};
pub use openai_compat::OpenAiCompatProvider;
pub use posture::{Occasion, Pack, Posture, packs};
pub use provider::{
    AssistantError, Completion, LlmProvider, Model, ProviderId, ProviderStatus, Role, Turn, Usage,
};
pub use takeover::{Holder, Takeover};
pub use technique::{Difficulty, Kind, Needs, Rig, Technique, catalogue};

use dj_secrets::SecretStore;
use std::sync::Arc;

/// Build every provider, ready for the settings panel.
///
/// All of them, including ones with no key: the panel shows what is available
/// and what it would take to use it, which is how a user finds out that a local
/// model needs nothing at all.
#[must_use]
pub fn all_providers(
    http: Arc<dyn HttpJson>,
    secrets: Arc<dyn SecretStore>,
) -> Vec<Arc<dyn LlmProvider>> {
    vec![
        Arc::new(OpenAiCompatProvider::openrouter(
            Arc::clone(&http),
            Arc::clone(&secrets),
        )),
        Arc::new(OpenAiCompatProvider::local(
            Arc::clone(&http),
            Arc::clone(&secrets),
        )),
        Arc::new(OpenAiCompatProvider::groq(
            Arc::clone(&http),
            Arc::clone(&secrets),
        )),
        Arc::new(AnthropicProvider::new(
            Arc::clone(&http),
            Arc::clone(&secrets),
        )),
        Arc::new(OpenAiCompatProvider::openai(
            Arc::clone(&http),
            Arc::clone(&secrets),
        )),
        Arc::new(GoogleProvider::new(http, secrets)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubHttp;
    use dj_secrets::MemoryStore;

    #[test]
    fn every_provider_is_constructed() {
        let providers = all_providers(
            Arc::new(StubHttp::new(vec![])),
            Arc::new(MemoryStore::new()),
        );
        assert_eq!(providers.len(), ProviderId::all().len());

        let ids: Vec<ProviderId> = providers.iter().map(|p| p.id()).collect();
        for expected in ProviderId::all() {
            assert!(ids.contains(expected), "{expected:?} missing");
        }
    }

    /// With no keys at all, the local provider is still ready. The assistant
    /// has to be usable before the user has signed up for anything.
    #[test]
    fn something_works_with_no_keys() {
        let providers = all_providers(
            Arc::new(StubHttp::new(vec![])),
            Arc::new(MemoryStore::new()),
        );
        let ready: Vec<ProviderId> = providers
            .iter()
            .filter(|p| p.status().is_usable())
            .map(|p| p.id())
            .collect();
        assert_eq!(ready, [ProviderId::Local]);
    }
}

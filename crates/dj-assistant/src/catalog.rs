//! What each provider is for, and what it costs.
//!
//! Same shape and same reasoning as `dj-sources`' catalog: the settings panel
//! renders this text directly, so what the user reads is what the code does.
//! Anyone choosing where to spend money on tokens is owed a straight answer,
//! including "start with this one, it is free".

use crate::provider::ProviderId;
use dj_secrets::SecretKind;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: ProviderId,
    pub label: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
    pub credential: Option<SecretKind>,
    pub signup_url: Option<&'static str>,
    /// Shown first. There should be exactly one, and it should be the one a
    /// newcomer should actually pick.
    pub recommended: bool,
}

#[must_use]
pub fn catalog() -> &'static [ProviderInfo] {
    &CATALOG
}

#[must_use]
pub fn info(id: ProviderId) -> &'static ProviderInfo {
    CATALOG
        .iter()
        .find(|entry| entry.id == id)
        .expect("every ProviderId has a catalog entry")
}

static CATALOG: [ProviderInfo; 6] = [
    ProviderInfo {
        id: ProviderId::OpenRouter,
        label: "OpenRouter",
        summary: "One key, hundreds of models, free and paid side by side.",
        detail: "The easiest place to start. A single key reaches models from \
                 every major lab plus a rotating set tagged `:free` that cost \
                 nothing at all — enough to use the assistant properly before \
                 deciding whether it is worth paying for. Prices are reported \
                 per model, so the spend cap works accurately here.",
        credential: Some(SecretKind::OpenRouter),
        signup_url: Some("https://openrouter.ai/keys"),
        recommended: true,
    },
    ProviderInfo {
        id: ProviderId::Local,
        label: "Local model",
        summary: "Free, private, and works with no internet at all.",
        detail: "Point djmanzo at Ollama or anything else speaking the OpenAI \
                 shape on localhost:11434. No key, no account, no cost, and — \
                 the part that matters in a booth — no dependency on the \
                 venue's wifi. Your track list never leaves the machine.\n\n\
                 A small local model handles the DJ vocabulary well, because \
                 the task is narrow: turn a sentence into commands from a fixed \
                 list. It will not plan a set as well as a frontier model.",
        credential: None,
        signup_url: Some("https://ollama.com/"),
        recommended: false,
    },
    ProviderInfo {
        id: ProviderId::Groq,
        label: "Groq",
        summary: "Free tier, and very fast — which is what voice needs.",
        detail: "Latency is felt directly when you are speaking to something. \
                 Groq is the fastest of these by a wide margin, which makes it \
                 the right choice once voice control lands. Free tier with rate \
                 limits rather than a bill.",
        credential: Some(SecretKind::Groq),
        signup_url: Some("https://console.groq.com/keys"),
        recommended: false,
    },
    ProviderInfo {
        id: ProviderId::Anthropic,
        label: "Anthropic",
        summary: "Strong at instruction-following. Trial credit, then pay as you go.",
        detail: "Good at staying inside a fixed command vocabulary, which is \
                 exactly what this application asks of a model. No free tier \
                 beyond signup credit, and the model list carries no pricing, \
                 so the session cost shows as unknown rather than being \
                 guessed at.",
        credential: Some(SecretKind::Anthropic),
        signup_url: Some("https://console.anthropic.com/settings/keys"),
        recommended: false,
    },
    ProviderInfo {
        id: ProviderId::OpenAi,
        label: "OpenAI",
        summary: "Trial credit, then pay as you go.",
        detail: "Works well and is the most widely documented if you want to \
                 experiment. Like Anthropic, the model list carries no pricing, \
                 so spend is reported as unknown rather than estimated.",
        credential: Some(SecretKind::OpenAi),
        signup_url: Some("https://platform.openai.com/api-keys"),
        recommended: false,
    },
    ProviderInfo {
        id: ProviderId::Google,
        label: "Google AI Studio",
        summary: "Generous free tier.",
        detail: "The most usable free tier of the hosted options, with large \
                 context windows. Worth having as a second key even if \
                 OpenRouter is the default, so there is somewhere to fall back \
                 to when a provider has a bad night.",
        credential: Some(SecretKind::GoogleAi),
        signup_url: Some("https://aistudio.google.com/apikey"),
        recommended: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_an_entry() {
        for id in ProviderId::all() {
            assert_eq!(info(*id).id, *id);
        }
        assert_eq!(CATALOG.len(), ProviderId::all().len());
    }

    #[test]
    fn every_entry_explains_itself_properly() {
        for entry in catalog() {
            assert!(!entry.summary.is_empty(), "{:?}", entry.id);
            assert!(
                entry.detail.len() > 120,
                "{:?} needs a real explanation, not a stub",
                entry.id
            );
        }
    }

    /// Exactly one recommendation, or the advice is not advice.
    #[test]
    fn there_is_one_recommended_starting_point() {
        let recommended: Vec<_> = catalog().iter().filter(|e| e.recommended).collect();
        assert_eq!(recommended.len(), 1);
        assert_eq!(recommended[0].id, ProviderId::OpenRouter);
    }

    /// The local option must not ask for a key — it is the answer for anyone
    /// who would rather not send their track list anywhere.
    #[test]
    fn the_local_option_needs_no_credential() {
        assert!(info(ProviderId::Local).credential.is_none());
    }

    #[test]
    fn a_provider_that_needs_a_key_says_where_to_get_one() {
        for entry in catalog() {
            if entry.credential.is_some() {
                let url = entry.signup_url.expect("no link to obtain a key");
                assert!(url.starts_with("https://"), "{:?}: {url}", entry.id);
            }
        }
    }

    /// The catalog's credential must be the one the provider actually reads,
    /// or the settings panel offers a field that unlocks nothing.
    #[test]
    fn catalog_credentials_match_the_provider_definitions() {
        for entry in catalog() {
            assert_eq!(
                entry.credential,
                entry.id.secret(),
                "{:?} disagrees about which key it needs",
                entry.id
            );
        }
    }
}

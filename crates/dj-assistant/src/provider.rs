//! What a language-model provider is.
//!
//! The contract is deliberately narrow — text in, text out, plus a model list —
//! because [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md)
//! makes that sufficient. The assistant's entire write surface is action text on
//! the existing bus, so a provider never needs a handle on anything; it needs to
//! answer a question in words. That is also what makes a local model and a
//! frontier model interchangeable here.

use dj_secrets::SecretKind;
use serde::Serialize;

/// Every provider the application knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    /// One key, hundreds of models, free and paid side by side.
    OpenRouter,
    Anthropic,
    OpenAi,
    Google,
    /// Very fast, which is what matters for voice.
    Groq,
    /// Ollama or anything else speaking the OpenAI shape on localhost. No key,
    /// no network, no track list leaving the machine.
    Local,
}

impl ProviderId {
    #[must_use]
    pub const fn all() -> &'static [ProviderId] {
        use ProviderId::*;
        &[OpenRouter, Local, Anthropic, OpenAi, Google, Groq]
    }

    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            ProviderId::OpenRouter => "openrouter",
            ProviderId::Anthropic => "anthropic",
            ProviderId::OpenAi => "openai",
            ProviderId::Google => "google",
            ProviderId::Groq => "groq",
            ProviderId::Local => "local",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ProviderId::OpenRouter => "OpenRouter",
            ProviderId::Anthropic => "Anthropic",
            ProviderId::OpenAi => "OpenAI",
            ProviderId::Google => "Google AI Studio",
            ProviderId::Groq => "Groq",
            ProviderId::Local => "Local model",
        }
    }

    /// The credential this provider needs, if any.
    #[must_use]
    pub const fn secret(self) -> Option<SecretKind> {
        match self {
            ProviderId::OpenRouter => Some(SecretKind::OpenRouter),
            ProviderId::Anthropic => Some(SecretKind::Anthropic),
            ProviderId::OpenAi => Some(SecretKind::OpenAi),
            ProviderId::Google => Some(SecretKind::GoogleAi),
            ProviderId::Groq => Some(SecretKind::Groq),
            // The whole point of the local provider.
            ProviderId::Local => None,
        }
    }
}

/// A model, as the picker shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    /// True when the provider charges nothing for it.
    ///
    /// Read from the provider's own pricing where it reports one, rather than
    /// from a list baked into a release — a model that becomes paid overnight
    /// should not keep being offered as free.
    pub free: bool,
    /// Context window in tokens, where reported.
    pub context: Option<u32>,
    /// USD per million input tokens, where reported.
    pub input_price: Option<f64>,
    /// USD per million output tokens, where reported.
    pub output_price: Option<f64>,
}

impl Model {
    /// Cost of one exchange, in USD, when pricing is known.
    #[must_use]
    pub fn cost(&self, prompt_tokens: u32, completion_tokens: u32) -> Option<f64> {
        let input = self.input_price?;
        let output = self.output_price?;
        Some(
            (f64::from(prompt_tokens) * input + f64::from(completion_tokens) * output)
                / 1_000_000.0,
        )
    }
}

/// One turn of a conversation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Turn {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Turn {
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// What a request cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    /// Filled in when the model's pricing is known. `None` is honest about not
    /// knowing rather than reporting zero, which would quietly under-count a
    /// session's spend.
    pub cost_usd: Option<f64>,
}

/// A model's reply.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Completion {
    pub text: String,
    pub usage: Usage,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProviderStatus {
    Ready,
    NeedsKey {
        secret: &'static str,
    },
    /// A local model that is not answering. Distinguished from a missing key
    /// because the fix is completely different: start Ollama, not paste a key.
    NotRunning {
        hint: &'static str,
    },
}

impl ProviderStatus {
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, ProviderStatus::Ready)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssistantError {
    #[error("{0} needs an API key")]
    MissingKey(&'static str),
    #[error("could not reach {provider}: {message}")]
    Network {
        provider: &'static str,
        message: String,
    },
    #[error("{provider} returned something unexpected: {message}")]
    BadResponse {
        provider: &'static str,
        message: String,
    },
    #[error("this session's spend cap of ${cap:.2} has been reached (${spent:.2} used)")]
    BudgetExhausted { cap: f64, spent: f64 },
    #[error("no provider is configured — add a key, or run a local model")]
    NoProvider,
}

/// Somewhere a question can be asked.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    fn id(&self) -> ProviderId;

    fn status(&self) -> ProviderStatus;

    /// The provider's live model list.
    ///
    /// Fetched rather than baked into a release: models appear, disappear and
    /// change price constantly, and a stale hard-coded list is a support burden
    /// that grows on its own.
    async fn models(&self) -> Result<Vec<Model>, AssistantError>;

    async fn complete(&self, model: &str, turns: &[Turn]) -> Result<Completion, AssistantError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_slugs_are_unique_and_round_trip() {
        use std::collections::HashSet;
        let slugs: HashSet<&str> = ProviderId::all().iter().map(|p| p.slug()).collect();
        assert_eq!(slugs.len(), ProviderId::all().len());
    }

    /// The local provider must never ask for a key. It is the answer for anyone
    /// who would rather not send their track list to a third party.
    #[test]
    fn the_local_provider_needs_no_key() {
        assert!(ProviderId::Local.secret().is_none());
        for id in ProviderId::all()
            .iter()
            .filter(|p| **p != ProviderId::Local)
        {
            assert!(id.secret().is_some(), "{id:?} should need a key");
        }
    }

    #[test]
    fn cost_is_computed_per_million_tokens() {
        let model = Model {
            id: "m".into(),
            name: "M".into(),
            free: false,
            context: None,
            input_price: Some(3.0),
            output_price: Some(15.0),
        };
        // 1M in at $3 plus 1M out at $15.
        let cost = model.cost(1_000_000, 1_000_000).unwrap();
        assert!((cost - 18.0).abs() < 1e-9, "got {cost}");
    }

    /// Unknown pricing reports `None`, not zero. Reporting zero would quietly
    /// under-count a session and make the spend cap useless.
    #[test]
    fn unknown_pricing_is_unknown_rather_than_free() {
        let model = Model {
            id: "m".into(),
            name: "M".into(),
            free: false,
            context: None,
            input_price: None,
            output_price: None,
        };
        assert_eq!(model.cost(1000, 1000), None);
    }
}

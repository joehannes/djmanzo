//! OpenRouter, OpenAI, Groq and any local server, in one implementation.
//!
//! Four of the six providers speak the same chat-completions shape, so they get
//! one implementation differing only in base URL, auth header and how the model
//! list reports pricing. Writing four near-identical files instead would mean
//! four places to fix the next time a response field moves.
//!
//! The pricing differences are the interesting part, and are handled per
//! provider rather than assumed:
//!
//! - **OpenRouter** reports per-token prices as strings, and a price of `"0"`
//!   is what makes a model genuinely free.
//! - **OpenAI and Groq** do not report pricing in their model lists at all, so
//!   cost comes back `None` — which the budget counts as *unknown*, not free.
//! - **A local model** costs nothing, which is a fact rather than a guess.

use crate::http::HttpJson;
use crate::provider::{
    AssistantError, Completion, LlmProvider, Model, ProviderId, ProviderStatus, Role, Turn, Usage,
};
use dj_secrets::SecretStore;
use serde_json::{Value, json};
use std::sync::Arc;

/// Where a local model is expected to be listening.
///
/// Ollama's OpenAI-compatible endpoint. Nothing stops another server being put
/// here; it just has to speak the same shape.
pub const LOCAL_BASE: &str = "http://localhost:11434/v1";

#[derive(Debug)]
pub struct OpenAiCompatProvider {
    id: ProviderId,
    base: String,
    http: Arc<dyn HttpJson>,
    secrets: Arc<dyn SecretStore>,
}

impl OpenAiCompatProvider {
    #[must_use]
    pub fn openrouter(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self::new(
            ProviderId::OpenRouter,
            "https://openrouter.ai/api/v1",
            http,
            secrets,
        )
    }

    #[must_use]
    pub fn openai(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self::new(
            ProviderId::OpenAi,
            "https://api.openai.com/v1",
            http,
            secrets,
        )
    }

    #[must_use]
    pub fn groq(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self::new(
            ProviderId::Groq,
            "https://api.groq.com/openai/v1",
            http,
            secrets,
        )
    }

    #[must_use]
    pub fn local(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self::new(ProviderId::Local, LOCAL_BASE, http, secrets)
    }

    fn new(
        id: ProviderId,
        base: &str,
        http: Arc<dyn HttpJson>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            id,
            base: base.to_owned(),
            http,
            secrets,
        }
    }

    fn headers(&self) -> Result<Vec<(String, String)>, AssistantError> {
        let Some(kind) = self.id.secret() else {
            return Ok(Vec::new());
        };
        let key = self
            .secrets
            .get(kind)
            .map_err(|_| AssistantError::MissingKey(self.id.label()))?;
        let mut headers = vec![(
            "Authorization".to_owned(),
            format!("Bearer {}", key.expose()),
        )];
        if self.id == ProviderId::OpenRouter {
            // OpenRouter attributes usage to an app when these are present, and
            // ranks it in their directory. Cheap courtesy, and it makes our
            // traffic identifiable if it ever misbehaves.
            headers.push((
                "HTTP-Referer".to_owned(),
                "https://github.com/joehannes/djmanzo".to_owned(),
            ));
            headers.push(("X-Title".to_owned(), "djmanzo".to_owned()));
        }
        Ok(headers)
    }
}

/// Read a number that may have arrived as a JSON number or as a string.
///
/// OpenRouter sends prices as strings; everyone else sends numbers.
fn number(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// Parse an OpenAI-shaped `/models` response.
pub fn parse_models(body: &Value, id: ProviderId) -> Vec<Model> {
    let Some(items) = body.get("data").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let model_id = item.get("id")?.as_str()?;
            let input_price =
                number(item.pointer("/pricing/prompt")).map(|per_token| per_token * 1_000_000.0);
            let output_price = number(item.pointer("/pricing/completion"))
                .map(|per_token| per_token * 1_000_000.0);

            let free = match id {
                // A local model costs nothing. That is a fact, not a guess.
                ProviderId::Local => true,
                // Everyone else: free only when the provider says the price is
                // zero. Absent pricing is unknown, and unknown is not free.
                _ => input_price == Some(0.0) && output_price == Some(0.0),
            };

            Some(Model {
                id: model_id.to_owned(),
                name: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(model_id)
                    .to_owned(),
                free,
                context: item
                    .get("context_length")
                    .or_else(|| item.get("context_window"))
                    .and_then(Value::as_u64)
                    .map(|c| c as u32),
                input_price,
                output_price,
            })
        })
        .collect()
}

/// Parse an OpenAI-shaped chat completion.
pub fn parse_completion(
    body: &Value,
    provider: &'static str,
) -> Result<Completion, AssistantError> {
    let text = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AssistantError::BadResponse {
            provider,
            message: "no message content in the response".into(),
        })?;

    Ok(Completion {
        text: text.to_owned(),
        usage: Usage {
            prompt_tokens: body
                .pointer("/usage/prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: body
                .pointer("/usage/completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            // Filled in by the caller, which knows the model's pricing.
            cost_usd: None,
        },
        model: body
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned(),
    })
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn status(&self) -> ProviderStatus {
        match self.id.secret() {
            None => ProviderStatus::Ready,
            Some(kind) if self.secrets.has(kind) => ProviderStatus::Ready,
            Some(kind) => ProviderStatus::NeedsKey { secret: kind.id() },
        }
    }

    async fn models(&self) -> Result<Vec<Model>, AssistantError> {
        let headers = self.headers()?;
        let body = self
            .http
            .get(&format!("{}/models", self.base), &headers)
            .await
            .map_err(|e| {
                // A local server that is not running is a different problem
                // from a bad key, and saying so saves a lot of confusion.
                if self.id == ProviderId::Local {
                    AssistantError::Network {
                        provider: self.id.label(),
                        message: format!("{e} — is Ollama running on {LOCAL_BASE}?"),
                    }
                } else {
                    e.into_assistant_error(self.id.label())
                }
            })?;
        Ok(parse_models(&body, self.id))
    }

    async fn complete(&self, model: &str, turns: &[Turn]) -> Result<Completion, AssistantError> {
        let headers = self.headers()?;
        let messages: Vec<Value> = turns
            .iter()
            .map(|turn| json!({"role": role_name(turn.role), "content": turn.content}))
            .collect();

        let body = json!({
            "model": model,
            "messages": messages,
            // Low but not zero. Action text should be deterministic; a little
            // slack stops a model getting stuck repeating one wrong answer.
            "temperature": 0.2,
        });

        let response = self
            .http
            .post(&format!("{}/chat/completions", self.base), &headers, &body)
            .await
            .map_err(|e| e.into_assistant_error(self.id.label()))?;
        parse_completion(&response, self.id.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubHttp;
    use dj_secrets::{MemoryStore, Secret, SecretKind};
    use serde_json::json;

    fn keyed() -> Arc<dyn SecretStore> {
        let store = MemoryStore::new();
        store
            .set(SecretKind::OpenRouter, &Secret::new("k"))
            .unwrap();
        Arc::new(store)
    }

    fn model_list() -> Value {
        json!({"data": [
            {
                "id": "meta-llama/llama-3.1-8b-instruct:free",
                "name": "Llama 3.1 8B (free)",
                "context_length": 131072,
                "pricing": {"prompt": "0", "completion": "0"}
            },
            {
                "id": "anthropic/claude-sonnet-4",
                "name": "Claude Sonnet 4",
                "context_length": 200000,
                "pricing": {"prompt": "0.000003", "completion": "0.000015"}
            }
        ]})
    }

    #[tokio::test]
    async fn a_model_list_parses_with_free_and_paid_distinguished() {
        let http = Arc::new(StubHttp::new(vec![model_list()]));
        let provider = OpenAiCompatProvider::openrouter(http, keyed());
        let models = provider.models().await.unwrap();

        assert_eq!(models.len(), 2);
        assert!(models[0].free, "a zero-priced model should read as free");
        assert!(!models[1].free);
        assert_eq!(models[0].context, Some(131_072));
    }

    /// OpenRouter reports per-token prices as strings; the picker shows per
    /// million. Getting this wrong by 1e6 makes every model look free.
    #[test]
    fn prices_are_converted_from_per_token_to_per_million() {
        let models = parse_models(&model_list(), ProviderId::OpenRouter);
        assert_eq!(models[1].input_price, Some(3.0));
        assert_eq!(models[1].output_price, Some(15.0));
    }

    /// Providers that report no pricing must not have their models read as
    /// free, or the spend cap silently stops working.
    #[test]
    fn absent_pricing_does_not_mean_free() {
        let body = json!({"data": [{"id": "gpt-4o", "object": "model"}]});
        let models = parse_models(&body, ProviderId::OpenAi);
        assert!(!models[0].free, "unpriced must not read as free");
        assert_eq!(models[0].input_price, None);
    }

    /// A local model genuinely is free, and should say so.
    #[test]
    fn local_models_are_free() {
        let body = json!({"data": [{"id": "llama3.2"}]});
        let models = parse_models(&body, ProviderId::Local);
        assert!(models[0].free);
    }

    #[tokio::test]
    async fn a_completion_parses_with_its_usage() {
        let http = Arc::new(StubHttp::new(vec![json!({
            "model": "test-model",
            "choices": [{"message": {"role": "assistant", "content": "deck 1 play"}}],
            "usage": {"prompt_tokens": 120, "completion_tokens": 4}
        })]));
        let provider = OpenAiCompatProvider::openrouter(http.clone(), keyed());
        let completion = provider
            .complete("test-model", &[Turn::user("play deck one")])
            .await
            .unwrap();

        assert_eq!(completion.text, "deck 1 play");
        assert_eq!(completion.usage.prompt_tokens, 120);
        assert_eq!(completion.usage.completion_tokens, 4);

        // And the request carried the turn through unchanged.
        let body = http.last_body().unwrap();
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "play deck one");
    }

    #[tokio::test]
    async fn a_missing_key_is_reported_before_any_request() {
        let http = Arc::new(StubHttp::new(vec![]));
        let provider = OpenAiCompatProvider::openrouter(http.clone(), Arc::new(MemoryStore::new()));
        assert!(matches!(provider.status(), ProviderStatus::NeedsKey { .. }));
        assert!(provider.models().await.is_err());
        assert_eq!(http.call_count(), 0, "called out despite having no key");
    }

    /// The local provider must work with no key at all — that is its purpose.
    #[tokio::test]
    async fn the_local_provider_is_ready_without_a_key() {
        let http = Arc::new(StubHttp::new(vec![json!({"data": [{"id": "llama3.2"}]})]));
        let provider = OpenAiCompatProvider::local(http, Arc::new(MemoryStore::new()));
        assert!(provider.status().is_usable());
        assert_eq!(provider.models().await.unwrap().len(), 1);
    }

    /// "Ollama is not running" and "your key is wrong" need completely
    /// different fixes, so they must not produce the same message.
    #[tokio::test]
    async fn a_local_server_that_is_down_says_so_specifically() {
        let http = Arc::new(StubHttp::failing("connection refused"));
        let provider = OpenAiCompatProvider::local(http, Arc::new(MemoryStore::new()));
        let error = provider.models().await.unwrap_err().to_string();
        assert!(error.contains("Ollama"), "{error}");
    }

    #[test]
    fn a_response_missing_its_content_is_an_error_not_an_empty_reply() {
        let error = parse_completion(&json!({"choices": []}), "Test").unwrap_err();
        assert!(matches!(error, AssistantError::BadResponse { .. }));
    }

    #[test]
    fn an_unexpected_model_list_yields_nothing_rather_than_panicking() {
        for body in [json!({}), json!({"data": {}}), json!({"data": [{}]})] {
            assert!(parse_models(&body, ProviderId::OpenAi).is_empty());
        }
    }
}

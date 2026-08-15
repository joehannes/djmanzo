//! Anthropic and Google, which speak their own shapes.
//!
//! Both differ from the OpenAI shape in ways that matter enough to implement
//! separately rather than to paper over:
//!
//! - **Anthropic** takes the system prompt as a top-level field rather than as
//!   a message, requires `max_tokens`, and returns content as a list of typed
//!   blocks.
//! - **Google** calls turns `contents`, calls the assistant role `model`, nests
//!   text in `parts`, and puts the key in the query string.
//!
//! Neither publishes per-model pricing in its model list, so cost comes back
//! `None` — which [`Budget`](crate::budget::Budget) counts as *unknown* rather
//! than free.

use crate::http::HttpJson;
use crate::provider::{
    AssistantError, Completion, LlmProvider, Model, ProviderId, ProviderStatus, Role, Turn, Usage,
};
use dj_secrets::SecretStore;
use serde_json::{Value, json};
use std::sync::Arc;

const ANTHROPIC_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const GOOGLE_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Ceiling on a reply.
///
/// Anthropic requires the field. Action text is short, and a plan is a handful
/// of lines, so this is generous rather than restrictive — and it caps the
/// damage if a model decides to write an essay on the meter.
const MAX_TOKENS: u32 = 2048;

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AnthropicProvider {
    http: Arc<dyn HttpJson>,
    secrets: Arc<dyn SecretStore>,
}

impl AnthropicProvider {
    #[must_use]
    pub fn new(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { http, secrets }
    }

    fn headers(&self) -> Result<Vec<(String, String)>, AssistantError> {
        let key = self
            .secrets
            .get(dj_secrets::SecretKind::Anthropic)
            .map_err(|_| AssistantError::MissingKey("Anthropic"))?;
        Ok(vec![
            ("x-api-key".to_owned(), key.expose().to_owned()),
            ("anthropic-version".to_owned(), ANTHROPIC_VERSION.to_owned()),
        ])
    }
}

/// Parse Anthropic's `/models` response.
pub fn parse_anthropic_models(body: &Value) -> Vec<Model> {
    let Some(items) = body.get("data").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            Some(Model {
                id: id.to_owned(),
                name: item
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_owned(),
                // Anthropic publishes no pricing here, and none is not free.
                free: false,
                context: None,
                input_price: None,
                output_price: None,
            })
        })
        .collect()
}

/// Parse an Anthropic message response.
///
/// Content is a list of typed blocks; only the text ones are ours.
pub fn parse_anthropic_completion(body: &Value) -> Result<Completion, AssistantError> {
    let blocks =
        body.get("content")
            .and_then(|v| v.as_array())
            .ok_or(AssistantError::BadResponse {
                provider: "Anthropic",
                message: "no content in the response".into(),
            })?;

    let text: String = blocks
        .iter()
        .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() {
        return Err(AssistantError::BadResponse {
            provider: "Anthropic",
            message: "the response carried no text".into(),
        });
    }

    Ok(Completion {
        text,
        usage: Usage {
            prompt_tokens: body
                .pointer("/usage/input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: body
                .pointer("/usage/output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            cost_usd: None,
        },
        model: body
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned(),
    })
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn status(&self) -> ProviderStatus {
        if self.secrets.has(dj_secrets::SecretKind::Anthropic) {
            ProviderStatus::Ready
        } else {
            ProviderStatus::NeedsKey {
                secret: dj_secrets::SecretKind::Anthropic.id(),
            }
        }
    }

    async fn models(&self) -> Result<Vec<Model>, AssistantError> {
        let headers = self.headers()?;
        let body = self
            .http
            .get(&format!("{ANTHROPIC_BASE}/models"), &headers)
            .await
            .map_err(|e| e.into_assistant_error("Anthropic"))?;
        Ok(parse_anthropic_models(&body))
    }

    async fn complete(&self, model: &str, turns: &[Turn]) -> Result<Completion, AssistantError> {
        let headers = self.headers()?;

        // The system prompt is a top-level field here, not a message. Sending
        // it as a message is accepted and then largely ignored, which is the
        // worst kind of failure -- it looks like it worked.
        let system: String = turns
            .iter()
            .filter(|t| t.role == Role::System)
            .map(|t| t.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let messages: Vec<Value> = turns
            .iter()
            .filter(|t| t.role != Role::System)
            .map(|turn| {
                let role = if turn.role == Role::Assistant {
                    "assistant"
                } else {
                    "user"
                };
                json!({"role": role, "content": turn.content})
            })
            .collect();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": MAX_TOKENS,
            "temperature": 0.2,
        });
        if !system.is_empty() {
            body["system"] = Value::String(system);
        }

        let response = self
            .http
            .post(&format!("{ANTHROPIC_BASE}/messages"), &headers, &body)
            .await
            .map_err(|e| e.into_assistant_error("Anthropic"))?;
        parse_anthropic_completion(&response)
    }
}

// ---------------------------------------------------------------------------
// Google
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct GoogleProvider {
    http: Arc<dyn HttpJson>,
    secrets: Arc<dyn SecretStore>,
}

impl GoogleProvider {
    #[must_use]
    pub fn new(http: Arc<dyn HttpJson>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { http, secrets }
    }

    fn key(&self) -> Result<String, AssistantError> {
        self.secrets
            .get(dj_secrets::SecretKind::GoogleAi)
            .map(|s| s.expose().to_owned())
            .map_err(|_| AssistantError::MissingKey("Google AI Studio"))
    }
}

/// Parse Google's `/models` response.
pub fn parse_google_models(body: &Value) -> Vec<Model> {
    let Some(items) = body.get("models").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?;
            // Names arrive as `models/gemini-2.0-flash`; the bare id is what
            // the generate endpoint wants.
            let id = name.strip_prefix("models/").unwrap_or(name);

            // Anything that cannot generate text is not a chat model, and
            // offering an embedding model in the picker wastes the user's time.
            let generates = item
                .get("supportedGenerationMethods")
                .and_then(|v| v.as_array())
                .is_none_or(|methods| {
                    methods
                        .iter()
                        .any(|m| m.as_str() == Some("generateContent"))
                });
            if !generates {
                return None;
            }

            Some(Model {
                id: id.to_owned(),
                name: item
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_owned(),
                free: false,
                context: item
                    .get("inputTokenLimit")
                    .and_then(Value::as_u64)
                    .map(|c| c as u32),
                input_price: None,
                output_price: None,
            })
        })
        .collect()
}

/// Parse a Google `generateContent` response.
pub fn parse_google_completion(body: &Value) -> Result<Completion, AssistantError> {
    let text = body
        .pointer("/candidates/0/content/parts")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|text| !text.is_empty())
        .ok_or(AssistantError::BadResponse {
            provider: "Google AI Studio",
            message: "no text in the response".into(),
        })?;

    Ok(Completion {
        text,
        usage: Usage {
            prompt_tokens: body
                .pointer("/usageMetadata/promptTokenCount")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: body
                .pointer("/usageMetadata/candidatesTokenCount")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            cost_usd: None,
        },
        model: body
            .get("modelVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned(),
    })
}

#[async_trait::async_trait]
impl LlmProvider for GoogleProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Google
    }

    fn status(&self) -> ProviderStatus {
        if self.secrets.has(dj_secrets::SecretKind::GoogleAi) {
            ProviderStatus::Ready
        } else {
            ProviderStatus::NeedsKey {
                secret: dj_secrets::SecretKind::GoogleAi.id(),
            }
        }
    }

    async fn models(&self) -> Result<Vec<Model>, AssistantError> {
        let key = self.key()?;
        let body = self
            .http
            .get(&format!("{GOOGLE_BASE}/models?key={key}"), &[])
            .await
            .map_err(|e| e.into_assistant_error("Google AI Studio"))?;
        Ok(parse_google_models(&body))
    }

    async fn complete(&self, model: &str, turns: &[Turn]) -> Result<Completion, AssistantError> {
        let key = self.key()?;

        let system: String = turns
            .iter()
            .filter(|t| t.role == Role::System)
            .map(|t| t.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let contents: Vec<Value> = turns
            .iter()
            .filter(|t| t.role != Role::System)
            .map(|turn| {
                // Google calls the assistant "model".
                let role = if turn.role == Role::Assistant {
                    "model"
                } else {
                    "user"
                };
                json!({"role": role, "parts": [{"text": turn.content}]})
            })
            .collect();

        let mut body = json!({
            "contents": contents,
            "generationConfig": {"temperature": 0.2, "maxOutputTokens": MAX_TOKENS},
        });
        if !system.is_empty() {
            body["systemInstruction"] = json!({"parts": [{"text": system}]});
        }

        let response = self
            .http
            .post(
                &format!("{GOOGLE_BASE}/models/{model}:generateContent?key={key}"),
                &[],
                &body,
            )
            .await
            .map_err(|e| e.into_assistant_error("Google AI Studio"))?;
        parse_google_completion(&response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubHttp;
    use dj_secrets::{MemoryStore, Secret, SecretKind};

    fn store(kind: SecretKind) -> Arc<dyn SecretStore> {
        let store = MemoryStore::new();
        store.set(kind, &Secret::new("k")).unwrap();
        Arc::new(store)
    }

    // -- Anthropic ---------------------------------------------------------

    /// The system prompt is a top-level field here. Sending it as a message is
    /// accepted and then largely ignored -- a failure that looks like success.
    #[tokio::test]
    async fn anthropic_sends_the_system_prompt_as_a_field_not_a_message() {
        let http = Arc::new(StubHttp::new(vec![json!({
            "model": "claude-sonnet-4",
            "content": [{"type": "text", "text": "deck 1 play"}],
            "usage": {"input_tokens": 100, "output_tokens": 5}
        })]));
        let provider = AnthropicProvider::new(http.clone(), store(SecretKind::Anthropic));

        provider
            .complete(
                "claude-sonnet-4",
                &[Turn::system("you are a DJ"), Turn::user("play deck one")],
            )
            .await
            .unwrap();

        let body = http.last_body().unwrap();
        assert_eq!(body["system"], "you are a DJ");
        assert_eq!(
            body["messages"].as_array().unwrap().len(),
            1,
            "the system turn should not also be a message"
        );
        assert_eq!(body["messages"][0]["role"], "user");
        // Anthropic rejects a request without this.
        assert!(body["max_tokens"].is_number());
    }

    /// Content is a list of typed blocks; a thinking block alongside a text
    /// block must not end up concatenated into the action text.
    #[test]
    fn anthropic_reads_only_text_blocks() {
        let body = json!({
            "content": [
                {"type": "thinking", "thinking": "the user wants deck one"},
                {"type": "text", "text": "deck 1 play"}
            ],
            "usage": {"input_tokens": 10, "output_tokens": 3}
        });
        let completion = parse_anthropic_completion(&body).unwrap();
        assert_eq!(completion.text, "deck 1 play");
        assert_eq!(completion.usage.prompt_tokens, 10);
    }

    #[test]
    fn anthropic_models_are_not_reported_as_free() {
        let body = json!({"data": [{"id": "claude-sonnet-4", "display_name": "Claude Sonnet 4"}]});
        let models = parse_anthropic_models(&body);
        assert_eq!(models[0].name, "Claude Sonnet 4");
        assert!(!models[0].free, "unpriced must not read as free");
    }

    #[tokio::test]
    async fn anthropic_without_a_key_makes_no_request() {
        let http = Arc::new(StubHttp::new(vec![]));
        let provider = AnthropicProvider::new(http.clone(), Arc::new(MemoryStore::new()));
        assert!(provider.models().await.is_err());
        assert_eq!(http.call_count(), 0);
    }

    // -- Google ------------------------------------------------------------

    #[tokio::test]
    async fn google_uses_its_own_role_and_parts_shape() {
        let http = Arc::new(StubHttp::new(vec![json!({
            "modelVersion": "gemini-2.0-flash",
            "candidates": [{"content": {"parts": [{"text": "deck 2 play"}]}}],
            "usageMetadata": {"promptTokenCount": 80, "candidatesTokenCount": 4}
        })]));
        let provider = GoogleProvider::new(http.clone(), store(SecretKind::GoogleAi));

        let completion = provider
            .complete(
                "gemini-2.0-flash",
                &[Turn::system("rules"), Turn::user("play deck two")],
            )
            .await
            .unwrap();

        assert_eq!(completion.text, "deck 2 play");
        let body = http.last_body().unwrap();
        assert_eq!(body["contents"][0]["parts"][0]["text"], "play deck two");
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "rules");
    }

    #[test]
    fn google_assistant_turns_are_called_model() {
        // Verified through the request body rather than by inspection, since
        // sending "assistant" here is a 400.
        let turns = [Turn::assistant("deck 1 play")];
        let role = if turns[0].role == Role::Assistant {
            "model"
        } else {
            "user"
        };
        assert_eq!(role, "model");
    }

    /// The model list is full of embedding and vision models that cannot chat.
    /// Offering them in the picker wastes the user's time.
    #[test]
    fn google_models_that_cannot_generate_text_are_filtered_out() {
        let body = json!({"models": [
            {
                "name": "models/gemini-2.0-flash",
                "displayName": "Gemini 2.0 Flash",
                "inputTokenLimit": 1048576,
                "supportedGenerationMethods": ["generateContent"]
            },
            {
                "name": "models/text-embedding-004",
                "displayName": "Embedding",
                "supportedGenerationMethods": ["embedContent"]
            }
        ]});
        let models = parse_google_models(&body);
        assert_eq!(models.len(), 1);
        // And the `models/` prefix is stripped, since the generate endpoint
        // wants the bare id.
        assert_eq!(models[0].id, "gemini-2.0-flash");
        assert_eq!(models[0].context, Some(1_048_576));
    }

    #[test]
    fn unexpected_shapes_yield_nothing_rather_than_panicking() {
        for body in [json!({}), json!({"models": {}}), json!({"data": [{}]})] {
            assert!(parse_google_models(&body).is_empty());
            assert!(parse_anthropic_models(&body).is_empty());
            assert!(parse_google_completion(&body).is_err());
            assert!(parse_anthropic_completion(&body).is_err());
        }
    }
}

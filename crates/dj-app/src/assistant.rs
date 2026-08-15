//! Commands for the assistant.
//!
//! The whole surface is deliberately thin, and the important line is in
//! [`ask`]: whatever the assistant decides becomes action *text*, which is then
//! put on the bus through exactly the same path a button click uses. There is
//! no privileged channel — see
//! [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md).
//!
//! A consequence worth naming: an assistant-issued action lands in the session
//! log alongside hand-played ones, so an assisted set replays and re-renders
//! identically to any other, and you can read back exactly what it did.

use crate::state::AppState;
use dj_assistant::{Assistant, ProviderId, ProviderStatus};
use dj_core::Action;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// One provider, as the settings panel draws it.
#[derive(Debug, Clone, Serialize)]
pub struct LlmProviderDto {
    pub id: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
    pub recommended: bool,
    /// `ready`, `needs_key` or `not_running`.
    pub status: &'static str,
    pub status_detail: String,
    /// The credential's stable id, when it needs one.
    pub credential: Option<&'static str>,
    pub credential_label: Option<&'static str>,
    pub signup_url: Option<&'static str>,
    pub free_tier: Option<&'static str>,
    pub is_set: bool,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantStateDto {
    pub provider: &'static str,
    pub model: String,
    pub spent_usd: f64,
    pub cap_usd: f64,
    /// Calls the provider never priced. A session reading `$0.00` after fifty
    /// calls is reporting ignorance, not thrift, and the panel says which.
    pub unpriced_calls: u64,
}

/// What the assistant did, and what it cost.
#[derive(Debug, Clone, Serialize)]
pub struct AnswerDto {
    pub reply: String,
    /// Actions that were dispatched, in order.
    pub actions: Vec<String>,
    /// Model output that was not valid action text. Shown rather than hidden:
    /// a model repeatedly emitting something plausible-but-wrong is a prompt
    /// problem, and concealing the evidence makes it unfixable.
    pub rejected: Vec<String>,
    /// `local` when no model was needed at all.
    pub source: &'static str,
    pub cost_usd: Option<f64>,
    /// Actions that parsed but the engine refused, usually because no audio
    /// device is open.
    pub undelivered: Vec<String>,
}

fn provider_from_slug(slug: &str) -> Option<ProviderId> {
    ProviderId::all().iter().copied().find(|p| p.slug() == slug)
}

/// Every provider, with its status and its key field.
#[tauri::command]
pub fn list_llm_providers(state: State<'_, AppState>) -> Vec<LlmProviderDto> {
    let secrets = state.secrets();
    state
        .llm_providers()
        .iter()
        .map(|provider| {
            let info = dj_assistant::info(provider.id());
            let (status, status_detail) = match provider.status() {
                ProviderStatus::Ready => ("ready", "Ready".to_owned()),
                ProviderStatus::NeedsKey { .. } => (
                    "needs_key",
                    format!(
                        "Needs {}",
                        info.credential.map(|c| c.label()).unwrap_or("a key")
                    ),
                ),
                ProviderStatus::NotRunning { hint } => ("not_running", hint.to_owned()),
            };
            let stored = info.credential.and_then(|kind| secrets.get(kind).ok());

            LlmProviderDto {
                id: provider.id().slug(),
                label: info.label,
                summary: info.summary,
                detail: info.detail,
                recommended: info.recommended,
                status,
                status_detail,
                credential: info.credential.map(|c| c.id()),
                credential_label: info.credential.map(|c| c.label()),
                signup_url: info.signup_url,
                free_tier: info.credential.map(|c| c.free_tier()),
                is_set: stored.is_some(),
                hint: stored.map(|s| s.hint()).unwrap_or_default(),
            }
        })
        .collect()
}

/// A provider's live model list.
#[tauri::command]
pub async fn list_llm_models(
    state: State<'_, AppState>,
    provider: String,
) -> Result<Vec<dj_assistant::Model>, String> {
    let id =
        provider_from_slug(&provider).ok_or_else(|| format!("unknown provider `{provider}`"))?;
    let provider = state
        .llm_provider(id)
        .ok_or_else(|| "that provider is not available".to_owned())?;
    provider.models().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn assistant_state(state: State<'_, AppState>) -> AssistantStateDto {
    state.assistant_state()
}

#[tauri::command]
pub fn set_assistant_model(
    state: State<'_, AppState>,
    provider: String,
    model: String,
) -> Result<AssistantStateDto, String> {
    let id =
        provider_from_slug(&provider).ok_or_else(|| format!("unknown provider `{provider}`"))?;
    state.set_assistant(id, model);
    Ok(state.assistant_state())
}

#[tauri::command]
pub fn set_spend_cap(state: State<'_, AppState>, usd: f64) -> AssistantStateDto {
    state.budget().set_cap(usd);
    state.assistant_state()
}

/// Start the spend total over. Called when a set begins.
#[tauri::command]
pub fn reset_spend(state: State<'_, AppState>) -> AssistantStateDto {
    state.budget().reset();
    state.assistant_state()
}

/// Ask the assistant to do something, and do it.
///
/// The two halves are deliberately separate: interpretation produces validated
/// action *text*, and dispatch puts that text on the bus through the same door
/// the interface uses. Nothing here can reach the engine directly.
#[tauri::command]
pub async fn ask(state: State<'_, AppState>, text: String) -> Result<AnswerDto, String> {
    let selection = state
        .assistant_selection()
        .ok_or_else(|| "no assistant provider is available".to_owned())?;
    let assistant = Assistant::new(
        selection.provider,
        selection.model,
        Arc::clone(state.budget()),
    )
    .with_pricing(selection.input_price, selection.output_price);

    let plan = assistant
        .interpret(&text)
        .await
        .map_err(|e| e.to_string())?;

    // Dispatch what survived validation. An action that the engine will not
    // take -- usually because no device is open -- is reported rather than
    // silently dropped, so "nothing happened" always has a reason attached.
    let mut undelivered = Vec::new();
    for text in &plan.actions {
        match Action::parse(text) {
            Ok(action) => {
                if state.bus().dispatch(action).is_err() {
                    undelivered.push(text.clone());
                }
            }
            // Unreachable: `interpret` only returns text that parsed. Belt and
            // braces, because this is the boundary that matters.
            Err(error) => {
                tracing::error!(%text, %error, "assistant emitted unparseable text past validation");
                undelivered.push(text.clone());
            }
        }
    }

    let reply = if undelivered.is_empty() {
        plan.reply
    } else {
        format!(
            "{} (the engine did not take {} of them — is a device open?)",
            plan.reply,
            undelivered.len()
        )
    };

    Ok(AnswerDto {
        reply,
        actions: plan.actions,
        rejected: plan.rejected,
        source: match plan.source {
            dj_assistant::Source::Local => "local",
            dj_assistant::Source::Model => "model",
        },
        cost_usd: plan.cost_usd,
        undelivered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_slugs_round_trip() {
        for id in ProviderId::all() {
            assert_eq!(provider_from_slug(id.slug()), Some(*id));
        }
        assert_eq!(provider_from_slug("nonsense"), None);
    }
}

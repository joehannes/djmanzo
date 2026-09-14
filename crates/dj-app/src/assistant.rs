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
/// What the model is told about the night, as lines.
///
/// A snapshot and two strings from the conduct state. Taken under the same lock
/// the panel reads, and falling back to the neutral words rather than failing
/// the whole question: an assistant that refused to answer because the posture
/// was momentarily unreadable would be worse than one answering without it.
fn context_lines(state: &AppState) -> Vec<String> {
    let snapshot = crate::commands::snapshot_now(state);
    let Ok(value) = serde_json::to_value(&snapshot) else {
        return Vec::new();
    };
    let (posture, occasion) = state.conduct().lock().map_or_else(
        |_| (String::new(), String::new()),
        |conduct| {
            (
                conduct.posture.name().to_owned(),
                conduct.occasion.name().to_owned(),
            )
        },
    );

    // Tonight, from the same log the Mixes panel reads. Derived rather than
    // recorded beside the log, so a night that started before any of this
    // existed has a history too -- and so this and that panel cannot disagree
    // about what was played.
    let log = state.bus().log();
    let history: Vec<String> = crate::mixes::handovers(&log)
        .iter()
        .map(|mix| {
            format!(
                "deck {} into deck {} ({})",
                mix.out.human_number(),
                mix.into.human_number(),
                mix.style.as_str()
            )
        })
        .collect();

    // What the DJ just did, in the action text a mapping or a script would
    // use. The same words everywhere: a model reading a line it could emit
    // back is the whole of ADR-0003's argument for one vocabulary.
    let mut recent: Vec<String> = log
        .iter()
        .rev()
        .take(12)
        .map(|entry| entry.event.to_line())
        .collect();
    recent.reverse();

    let hardware = hands_line(state);

    // §41 already lets the assistant ask for an arrangement; §40 is the other
    // half -- knowing which one is on screen before asking for another.
    let focus = state.workspace().map_or_else(String::new, |workspace| {
        format!("{} ({})", workspace.name, workspace.about)
    });

    // §44's transaction, in the words the panel puts in front of the DJ. An
    // assistant asked "should I bring it in" that cannot see it has already
    // prepared the record is answering a different question from the one on
    // screen.
    let staged: Vec<String> = state.staged().map_or_else(Vec::new, |plan| {
        plan.moves
            .iter()
            .map(|step| {
                format!(
                    "{}{}",
                    step.about,
                    if step.chosen { "" } else { " (turned off)" }
                )
            })
            .collect()
    });

    // §21's planned set, and **how far through it**: the next record in a plan
    // is the useful half, and a bare list leaves the model to guess which one
    // that is.
    let plan = planned_lines(state);

    // §68's armed mix. Only while it still describes what is on the decks —
    // the rule `transition_current` states, because a confident mix point for
    // a record that left four minutes ago looks exactly like a current one.
    let transition = armed_line(state);

    // §22's rail and §81's profile, both of which need the library. A briefing
    // without them is the answer djmanzo already has, withheld from the thing
    // being asked the same question.
    let (candidates, profile) = match crate::commands::library(state) {
        Ok(db) => (candidate_lines(state, &db), profile_line(state, &db)),
        Err(_) => (Vec::new(), String::new()),
    };

    crate::sight::brief(
        &value,
        &crate::sight::Beside {
            posture,
            occasion,
            history,
            recent,
            hardware,
            focus,
            staged,
            candidates,
            plan,
            profile,
            transition,
        },
    )
}

/// The planned set, in order, with the one that is next marked.
fn planned_lines(state: &AppState) -> Vec<String> {
    let held = state.conduct();
    let Ok(conduct) = held.lock() else {
        return Vec::new();
    };
    conduct
        .setlist
        .iter()
        .enumerate()
        .map(|(at, track)| {
            let hex = track.to_hex();
            // Short, because a briefing is read by something paying by the
            // token and a full id says nothing a prefix does not.
            let short = &hex[..hex.len().min(8)];
            if at == conduct.played {
                format!("{short} (next)")
            } else {
                short.to_owned()
            }
        })
        .collect()
}

/// The armed mix as a shape and a length, or nothing.
fn armed_line(state: &AppState) -> String {
    let Some(transition) = state.transition() else {
        return String::new();
    };
    let loaded = |deck| crate::commands::current_track(state, deck);
    if !transition.describes(
        loaded(transition.outgoing_deck),
        loaded(transition.incoming_deck),
    ) {
        return String::new();
    }
    format!(
        "deck {} into deck {}, {} over {} beats at {:.0}s; {}",
        transition.outgoing_deck.human_number(),
        transition.incoming_deck.human_number(),
        transition.plan.style.as_str(),
        transition.plan.length_beats,
        transition.start_seconds(),
        transition.shape().words().join("; ")
    )
}

/// What the rail would offer next, best first, with the reasons.
///
/// The deck the room is hearing, because that is what the rail follows. A few
/// rather than the whole ranking: the top of a rail is what a DJ reads and a
/// briefing carrying fifty is the library in a prompt, which is what the
/// library itself is deliberately kept out of this for.
fn candidate_lines(state: &AppState, db: &dj_library::Library) -> Vec<String> {
    const OFFERED: usize = 5;

    // The same deck `context::carrying` calls the night's, so the rail the
    // model is shown and the tempo it is told are about one record.
    let snapshot = crate::commands::snapshot_now(state);
    let Some(playing) = crate::context::carrying(&snapshot)
        .and_then(|deck| dj_core::DeckId::from_human(deck.number))
    else {
        return Vec::new();
    };
    let Some(now) =
        crate::commands::current_track(state, playing).and_then(|id| db.track(id).ok().flatten())
    else {
        return Vec::new();
    };
    let Ok(pool) = db.all_tracks(5_000) else {
        return Vec::new();
    };

    let here = dj_library::suggest::Playing::of(&now);
    dj_library::suggest::rank(&here, dj_library::suggest::Trajectory::Hold, &pool)
        .into_iter()
        .filter(|found| found.track != now.id)
        .take(OFFERED)
        .filter_map(|found| {
            let track = pool.iter().find(|t| t.id == found.track)?;
            let name = track
                .tags
                .title
                .clone()
                .unwrap_or_else(|| track.id.to_hex()[..8].to_owned());
            Some(format!(
                "{name} ({})",
                crate::commands::summarise_reasons(&found.reasons)
            ))
        })
        .collect()
}

/// §81's profile, as the one sentence djmanzo writes about it.
///
/// The sentence rather than the numbers: it names the setting and the evidence
/// by construction, so the model is handed a claim djmanzo would make rather
/// than weights it could assemble a stronger one out of.
fn profile_line(state: &AppState, db: &dj_library::Library) -> String {
    crate::commands::tonight_profile(state, db).map_or_else(String::new, |p| p.words())
}

/// §53's controller profile, as one line.
///
/// The counts rather than the mapping: what matters to an answer is whether
/// there is a jog to nudge and whether the stems can be reached by hand, and a
/// list of every binding would be the mapping file in a prompt.
fn hands_line(state: &AppState) -> String {
    let control = state.control();
    let Some(open) = control.status(None).open_mapping else {
        return String::new();
    };
    let Some(mapping) = control
        .mappings()
        .into_iter()
        .find(|mapping| mapping.name == open)
    else {
        return String::new();
    };
    let hands = mapping.hands;
    format!(
        "{}: {} decks, {} jogs, {} pads, {} knobs, {}",
        mapping.name,
        hands.decks,
        hands.jogs,
        hands.pads,
        hands.knobs,
        if hands.stems {
            "reaches the stems"
        } else {
            "no stem controls"
        }
    )
}

/// The two halves are deliberately separate: interpretation produces validated
/// action *text*, and dispatch puts that text on the bus through the same door
/// the interface uses. Nothing here can reach the engine directly.
#[tauri::command]
pub async fn ask(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<AnswerDto, String> {
    let selection = state
        .assistant_selection()
        .ok_or_else(|| "no assistant provider is available".to_owned())?;
    let decks = u8::try_from(state.deck_count()).unwrap_or(4);
    let assistant = Assistant::new(
        selection.provider,
        selection.model,
        Arc::clone(state.budget()),
    )
    .with_pricing(selection.input_price, selection.output_price)
    // §41: the interface's own vocabulary, generated from the surfaces that
    // exist. Injected rather than known by `dj_assistant`, which has never
    // heard of a panel.
    .with_commands(crate::uiop::as_prompt_lines(decks))
    // §40: and what is actually happening. Built from the same snapshot the
    // interface is looking at, so what the model is told and what the DJ can
    // see cannot disagree. `sight::ALL` says which of §40's twenty-six this
    // carries and which it does not, and the assistant panel draws both halves
    // -- a DJ deciding whether to trust an answer needs to know what the thing
    // answering could not see.
    .with_context(context_lines(&state));

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

    // §41. A line that was not an action gets a second look here, because this
    // is the layer that knew what it injected. Anything that is not one of
    // *these* either stays rejected — an assistant that guessed at the
    // difference would be exactly the failure ADR-0005 exists to prevent.
    let mut rejected = Vec::new();
    let mut arranged = Vec::new();
    for line in plan.rejected {
        match crate::uiop::UiOp::parse(&line) {
            Ok(op) => match crate::commands::ui_request(&app, &state, &op) {
                Ok(applied) => arranged.push(applied.what),
                // Refused by §72's matrix. Reported rather than swallowed: a
                // DJ who has told the assistant to leave their screen alone
                // should see that it tried, not silence.
                Err(why) => rejected.push(format!("{line} — {why}")),
            },
            Err(_) => rejected.push(line),
        }
    }

    let reply = if undelivered.is_empty() {
        if arranged.is_empty() {
            plan.reply
        } else {
            format!("{} ({})", plan.reply, arranged.join(", "))
        }
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
        rejected,
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

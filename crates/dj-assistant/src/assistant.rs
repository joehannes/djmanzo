//! Turning language into actions, and refusing everything else.
//!
//! This is where [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md)
//! is enforced rather than merely intended. Whatever a model says, the only
//! thing that leaves this module is action text that
//! [`Action::parse`](dj_core::Action::parse) accepted. A hallucination is a
//! parse error; it cannot invent a deck, exceed a range, or reach past the
//! queue, because there is no path by which it could.
//!
//! The order of attempts matters and is deliberate:
//!
//! 1. **The local matcher** ([`crate::intent`]) — free, instant, offline, and
//!    handles most of what is actually said mid-set.
//! 2. **The model** — only for language that genuinely needs understanding,
//!    and only if the session's budget still allows it.
//!
//! A model is the expensive, slow, fallible option, so it is the second one.

use crate::budget::Budget;
use crate::intent;
use crate::provider::{AssistantError, LlmProvider, Turn};
use dj_core::Action;
use serde::Serialize;
use std::sync::Arc;

/// Where an answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Understood without a model. Free and instant.
    Local,
    Model,
}

/// What the assistant decided to do.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    /// Validated action text, in order. Every entry parsed.
    pub actions: Vec<String>,
    /// What to show the user.
    pub reply: String,
    pub source: Source,
    /// Lines the model produced that were **not** valid actions.
    ///
    /// Surfaced rather than silently dropped: a model that keeps emitting
    /// something plausible-but-wrong is a prompt problem, and hiding the
    /// evidence makes it unfixable.
    pub rejected: Vec<String>,
    /// Cost of this exchange, when the provider reported enough to know.
    pub cost_usd: Option<f64>,
}

impl Plan {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// The system prompt, built from the vocabulary that actually exists.
///
/// Generated rather than hand-written, per ADR-0005: a hand-written list would
/// drift the first time a verb was added, producing a model confidently
/// emitting commands the parser rejects.
#[must_use]
pub fn system_prompt() -> String {
    let mut prompt = String::from(
        "You control a DJ application. Reply with ONLY action commands, one per \
         line, chosen from the list below. No explanation, no code fences, no \
         commentary. If a request cannot be expressed as these commands, reply \
         with the single word: UNSUPPORTED\n\n\
         Commands:\n",
    );
    for line in dj_core::vocabulary::as_prompt_lines() {
        prompt.push_str("  ");
        prompt.push_str(&line);
        prompt.push('\n');
    }
    prompt.push_str(
        "\nDeck numbers are 1 to 4. Values outside a command's range are \
         clamped, so prefer sensible ones.",
    );
    prompt
}

/// What a model says when it cannot help. Recognised so it is reported as
/// "I cannot do that" rather than as four rejected lines of apology.
const UNSUPPORTED: &str = "UNSUPPORTED";

/// Pull valid actions out of whatever a model said.
///
/// Tolerant of the shapes models actually produce — code fences, numbered
/// lists, trailing commentary — because rejecting a good answer over a stray
/// backtick would be its own kind of bug. Tolerant about *framing*, strict
/// about *content*: every surviving line still has to parse.
#[must_use]
pub fn extract_actions(text: &str) -> (Vec<String>, Vec<String>) {
    let mut actions = Vec::new();
    let mut rejected = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        // Strip list markers a model adds unbidden: "1. ", "- ", "* ".
        let line = line
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(['.', ')', '-', '*', ' '])
            .trim();
        if line.is_empty() || line.eq_ignore_ascii_case(UNSUPPORTED) {
            continue;
        }

        match Action::parse(line) {
            // Round-trip through `Display` rather than keeping the model's
            // text. What executes is then exactly what the type says, with no
            // room for a spelling the parser tolerated to mean something else.
            Ok(action) => actions.push(action.to_string()),
            Err(_) => rejected.push(line.to_owned()),
        }
    }
    (actions, rejected)
}

/// The assistant.
#[derive(Debug)]
pub struct Assistant {
    provider: Arc<dyn LlmProvider>,
    model: String,
    budget: Arc<Budget>,
    /// Per-token pricing for the chosen model, when known.
    pricing: Option<(f64, f64)>,
}

impl Assistant {
    #[must_use]
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        model: impl Into<String>,
        budget: Arc<Budget>,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            budget,
            pricing: None,
        }
    }

    /// Tell the assistant what the chosen model costs, per million tokens.
    #[must_use]
    pub fn with_pricing(mut self, input: Option<f64>, output: Option<f64>) -> Self {
        self.pricing = input.zip(output);
        self
    }

    #[must_use]
    pub fn budget(&self) -> &Arc<Budget> {
        &self.budget
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Understand `text`, and return what should happen.
    ///
    /// Tries the local matcher first. Only reaches the model when that fails,
    /// and only if the budget allows.
    pub async fn interpret(&self, text: &str) -> Result<Plan, AssistantError> {
        if let Some(matched) = intent::match_local(text) {
            return Ok(Plan {
                actions: matched.actions,
                reply: matched.reply,
                source: Source::Local,
                rejected: Vec::new(),
                cost_usd: Some(0.0),
            });
        }

        // Checked before the call, not after. A cap that reports an overspend
        // is not a cap.
        if !self.budget.allows_another() {
            return Err(AssistantError::BudgetExhausted {
                cap: self.budget.cap_usd(),
                spent: self.budget.spent_usd(),
            });
        }

        let turns = [Turn::system(system_prompt()), Turn::user(text)];
        let completion = self.provider.complete(&self.model, &turns).await?;

        let cost = self.pricing.map(|(input, output)| {
            (f64::from(completion.usage.prompt_tokens) * input
                + f64::from(completion.usage.completion_tokens) * output)
                / 1_000_000.0
        });
        self.budget.record(cost);

        let (actions, rejected) = extract_actions(&completion.text);
        let reply = if actions.is_empty() {
            if completion.text.to_uppercase().contains(UNSUPPORTED) {
                "I cannot do that with the controls I have.".to_owned()
            } else {
                "I did not understand that as something I can do.".to_owned()
            }
        } else if actions.len() == 1 {
            actions[0].clone()
        } else {
            format!("{} actions.", actions.len())
        };

        Ok(Plan {
            actions,
            reply,
            source: Source::Model,
            rejected,
            cost_usd: cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Completion, Model, ProviderId, ProviderStatus, Usage};

    /// A provider that says whatever the test tells it to.
    #[derive(Debug)]
    struct Scripted {
        reply: String,
        tokens: (u32, u32),
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Scripted {
        fn new(reply: &str) -> Self {
            Self {
                reply: reply.to_owned(),
                tokens: (100, 10),
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for Scripted {
        fn id(&self) -> ProviderId {
            ProviderId::Local
        }
        fn status(&self) -> ProviderStatus {
            ProviderStatus::Ready
        }
        async fn models(&self) -> Result<Vec<Model>, AssistantError> {
            Ok(Vec::new())
        }
        async fn complete(&self, _: &str, _: &[Turn]) -> Result<Completion, AssistantError> {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Completion {
                text: self.reply.clone(),
                usage: Usage {
                    prompt_tokens: self.tokens.0,
                    completion_tokens: self.tokens.1,
                    cost_usd: None,
                },
                model: "scripted".into(),
            })
        }
    }

    fn assistant(reply: &str) -> (Assistant, Arc<Scripted>) {
        let provider = Arc::new(Scripted::new(reply));
        let assistant = Assistant::new(
            Arc::clone(&provider) as Arc<dyn LlmProvider>,
            "scripted",
            Arc::new(Budget::new(1.0)),
        );
        (assistant, provider)
    }

    // -- the rule ----------------------------------------------------------

    /// **The test ADR-0005 exists for.** Whatever a model says, only valid
    /// action text gets out.
    #[tokio::test]
    async fn nothing_that_fails_to_parse_ever_escapes() {
        let (assistant, _) = assistant(
            "deck 1 play\n\
             deck 9 explode\n\
             rm -rf /\n\
             deck 2 volume 0.5\n\
             SELECT * FROM tracks",
        );
        let plan = assistant.interpret("do the thing").await.unwrap();

        assert_eq!(plan.actions, ["deck 1 play", "deck 2 volume 0.5"]);
        assert_eq!(plan.rejected.len(), 3, "{:?}", plan.rejected);
        for action in &plan.actions {
            assert!(Action::parse(action).is_ok());
        }
    }

    /// Rejections are reported, not swallowed. A model that keeps emitting
    /// something plausible-but-wrong is a prompt problem, and hiding the
    /// evidence makes it unfixable.
    #[tokio::test]
    async fn rejected_lines_are_surfaced() {
        let (assistant, _) = assistant("deck 1 levitate");
        let plan = assistant.interpret("levitate").await.unwrap();
        assert!(plan.actions.is_empty());
        assert_eq!(plan.rejected, ["deck 1 levitate"]);
    }

    /// A deck number outside the range must not become a valid action.
    ///
    /// Tested through `extract_actions` rather than through `interpret`,
    /// because the local matcher would catch the sentence first — which is
    /// correct behaviour, and would mean this never exercised the model path.
    #[test]
    fn an_out_of_range_deck_is_rejected_not_clamped() {
        let (actions, rejected) = extract_actions("deck 99 play");
        assert!(actions.is_empty(), "{actions:?}");
        assert_eq!(rejected, ["deck 99 play"]);
    }

    /// Values inside a verb's range *are* clamped rather than rejected, which
    /// is what keeps a confused model's worst case mild rather than fatal.
    #[test]
    fn an_out_of_range_value_is_clamped_rather_than_rejected() {
        let (actions, rejected) = extract_actions("deck 1 volume 99");
        assert_eq!(actions, ["deck 1 volume 1"]);
        assert!(rejected.is_empty());
    }

    // -- the local matcher comes first --------------------------------------

    /// The point of the matcher: a common command must not cost a round trip.
    #[tokio::test]
    async fn a_command_the_matcher_knows_never_reaches_the_model() {
        let (assistant, provider) = assistant("should not be used");
        let plan = assistant.interpret("play deck 2").await.unwrap();

        assert_eq!(plan.actions, ["deck 2 play"]);
        assert_eq!(plan.source, Source::Local);
        assert_eq!(plan.cost_usd, Some(0.0));
        assert_eq!(provider.calls(), 0, "called a model for `play deck 2`");
    }

    #[tokio::test]
    async fn language_the_matcher_cannot_handle_goes_to_the_model() {
        let (assistant, provider) = assistant("deck 1 pitch 0.04");
        let plan = assistant
            .interpret("nudge the first deck up a touch")
            .await
            .unwrap();
        assert_eq!(plan.source, Source::Model);
        assert_eq!(plan.actions, ["deck 1 pitch 0.04"]);
        assert_eq!(provider.calls(), 1);
    }

    // -- tolerant framing, strict content ----------------------------------

    #[test]
    fn code_fences_and_list_markers_are_stripped() {
        let (actions, rejected) = extract_actions(
            "```\n\
             1. deck 1 play\n\
             - deck 2 cue_on\n\
             * crossfader 0\n\
             ```",
        );
        assert_eq!(actions, ["deck 1 play", "deck 2 cue_on", "crossfader 0"]);
        assert!(rejected.is_empty());
    }

    /// Output is round-tripped through `Display`, so what executes is exactly
    /// what the type means rather than the model's spelling of it.
    #[test]
    fn actions_are_normalised_rather_than_passed_through() {
        let (actions, _) = extract_actions("DECK 1 PLAY");
        assert_eq!(actions, ["deck 1 play"]);
    }

    #[tokio::test]
    async fn a_model_saying_it_cannot_help_is_reported_plainly() {
        let (assistant, _) = assistant("UNSUPPORTED");
        let plan = assistant.interpret("make me a sandwich").await.unwrap();
        assert!(plan.actions.is_empty());
        assert!(plan.rejected.is_empty(), "UNSUPPORTED is not a rejection");
        assert!(plan.reply.contains("cannot"), "{}", plan.reply);
    }

    // -- budget ------------------------------------------------------------

    /// A cap that reports an overspend is not a cap. It must refuse *before*
    /// the request.
    #[tokio::test]
    async fn an_exhausted_budget_refuses_before_calling_out() {
        let provider = Arc::new(Scripted::new("deck 1 play"));
        let budget = Arc::new(Budget::new(0.01));
        budget.record(Some(0.02));

        let assistant = Assistant::new(
            Arc::clone(&provider) as Arc<dyn LlmProvider>,
            "scripted",
            budget,
        );
        let error = assistant.interpret("something novel").await.unwrap_err();

        assert!(matches!(error, AssistantError::BudgetExhausted { .. }));
        assert_eq!(provider.calls(), 0, "spent money past the cap");
    }

    /// The matcher still works when the budget is gone — those commands cost
    /// nothing, and a DJ mid-set should not lose "play" to an accounting limit.
    #[tokio::test]
    async fn local_commands_still_work_with_no_budget_left() {
        let provider = Arc::new(Scripted::new("x"));
        let budget = Arc::new(Budget::new(0.01));
        budget.record(Some(1.0));

        let assistant = Assistant::new(provider as Arc<dyn LlmProvider>, "m", budget);
        let plan = assistant.interpret("play deck 1").await.unwrap();
        assert_eq!(plan.actions, ["deck 1 play"]);
    }

    #[tokio::test]
    async fn spend_accumulates_from_reported_usage() {
        let provider = Arc::new(Scripted::new("deck 1 play"));
        let budget = Arc::new(Budget::new(10.0));
        let assistant = Assistant::new(provider as Arc<dyn LlmProvider>, "m", Arc::clone(&budget))
            // $3/M in, $15/M out; 100 in and 10 out.
            .with_pricing(Some(3.0), Some(15.0));

        let plan = assistant.interpret("something novel").await.unwrap();
        let expected = (100.0 * 3.0 + 10.0 * 15.0) / 1_000_000.0;
        assert!((plan.cost_usd.unwrap() - expected).abs() < 1e-12);
        assert!((budget.spent_usd() - expected).abs() < 1e-9);
    }

    #[tokio::test]
    async fn an_unpriced_model_is_counted_as_unknown() {
        let (assistant, _) = assistant("deck 1 play");
        let plan = assistant.interpret("something novel").await.unwrap();
        assert_eq!(plan.cost_usd, None);
        assert_eq!(assistant.budget().unpriced_calls(), 1);
    }

    // -- the prompt --------------------------------------------------------

    /// The prompt is generated from the vocabulary, so a verb added to
    /// `dj-core` reaches the model with no assistant-side work.
    #[test]
    fn the_prompt_lists_every_verb_that_exists() {
        let prompt = system_prompt();
        for spec in dj_core::vocabulary::vocabulary() {
            assert!(
                prompt.contains(spec.example),
                "`{}` is missing from the prompt",
                spec.verb
            );
        }
        assert!(prompt.contains("keylock_on"), "keylock reached the prompt");
    }

    #[test]
    fn the_prompt_forbids_commentary() {
        let prompt = system_prompt().to_lowercase();
        assert!(prompt.contains("only action commands"));
        assert!(prompt.contains("no explanation"));
    }
}

//! Understanding the common commands without a model.
//!
//! Most of what a DJ says to an assistant mid-set is not interesting language.
//! It is "play deck two", "kill the bass", "para el uno". Sending those to a
//! model costs money, costs a round trip, and can fail because the wifi in the
//! venue is what it is.
//!
//! So they are matched here: no network, no key, no cost, and an answer in
//! microseconds. The model is kept for language that genuinely needs
//! understanding.
//!
//! # Spanish
//!
//! Not an afterthought. djmanzo is built for a Dominican DJ, and the commands
//! shouted over a merengue at half past one are not going to be in English.
//! Both languages are matched by the same rules.
//!
//! # What this deliberately does not do
//!
//! No fuzzy matching, no stemming, no scoring. A near-miss returns `None` and
//! falls through to the model, which is the correct outcome: guessing wrong
//! about "drop the bass" in front of a room is far worse than taking a round
//! trip to be sure.

use dj_core::Action;

/// Everything the matcher understood, as action text.
///
/// Text rather than `Action` values because
/// [ADR-0005](../../../docs/adr/0005-assistant-speaks-only-actions.md) sends
/// everything through the same door: the matcher's output is validated by
/// `Action::parse` exactly like a model's, so a bug here cannot produce
/// something a model could not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub actions: Vec<String>,
    /// What to say back, if anything.
    pub reply: String,
}

/// Try to understand `text` without a model.
///
/// Returns `None` when nothing matched confidently, which means "ask the model".
#[must_use]
pub fn match_local(text: &str) -> Option<Match> {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    let words: Vec<&str> = lower.split_whitespace().collect();

    let deck = deck_number(&words);

    // Order matters: the most specific patterns first, so "kill the bass on
    // deck 2" is not caught by a bare "deck 2" rule.
    for rule in RULES {
        if !rule.phrases.iter().any(|p| contains_phrase(&lower, p)) {
            continue;
        }
        let deck = deck.unwrap_or(rule.default_deck);
        let actions: Vec<String> = rule
            .actions
            .iter()
            .map(|template| template.replace("{deck}", &deck.to_string()))
            .collect();

        // Never emit something the parser would reject. A rule with a typo in
        // its template should fail here, not in front of an audience.
        if actions.iter().any(|a| Action::parse(a).is_err()) {
            tracing::error!(?actions, "an intent rule produced unparseable action text");
            return None;
        }

        return Some(Match {
            actions,
            reply: rule.reply.replace("{deck}", &deck.to_string()),
        });
    }
    None
}

/// Match a phrase, requiring whole words.
///
/// Substring matching would let "unplayable" trigger "play". Padding both sides
/// with spaces and comparing against a padded haystack is the cheapest correct
/// way to require word boundaries without pulling in a regex engine.
fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    let padded = format!(" {haystack} ");
    padded.contains(&format!(" {phrase} "))
}

/// Find a deck number anywhere in the sentence.
///
/// Handles digits and words, in both languages, and the shorthands a DJ
/// actually uses ("the left deck", "el izquierdo").
fn deck_number(words: &[&str]) -> Option<u8> {
    for (index, word) in words.iter().enumerate() {
        let number = match *word {
            "1" | "one" | "uno" | "left" | "izquierda" | "izquierdo" => Some(1),
            "2" | "two" | "dos" | "right" | "derecha" | "derecho" => Some(2),
            "3" | "three" | "tres" => Some(3),
            "4" | "four" | "cuatro" => Some(4),
            _ => None,
        };
        if let Some(number) = number {
            // A bare number only counts as a deck when something nearby says so
            // -- otherwise "eq_low 2" or "give me 2 minutes" becomes a deck.
            let is_digit = word.len() == 1 && word.chars().all(|c| c.is_ascii_digit());
            if !is_digit || near_deck_word(words, index) {
                return Some(number);
            }
        }
    }
    None
}

fn near_deck_word(words: &[&str], index: usize) -> bool {
    let start = index.saturating_sub(2);
    let end = (index + 1).min(words.len());
    words[start..end]
        .iter()
        .any(|w| matches!(*w, "deck" | "player" | "channel" | "canal" | "plato"))
}

struct Rule {
    /// Any one of these, matched on whole words.
    phrases: &'static [&'static str],
    /// Action templates; `{deck}` is substituted.
    actions: &'static [&'static str],
    reply: &'static str,
    /// Used when the sentence names no deck.
    default_deck: u8,
}

static RULES: &[Rule] = &[
    // -- EQ, before transport, because "drop the bass" contains no verb the
    // transport rules would catch but "bring back the bass on deck 2" does.
    Rule {
        phrases: &[
            "kill the bass",
            "kill bass",
            "drop the bass",
            "no bass",
            "cut the bass",
            "quita los graves",
            "sin graves",
            "corta el bajo",
        ],
        actions: &["deck {deck} eq_low 0"],
        reply: "Bass out on deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &[
            "bring back the bass",
            "bass back",
            "restore the bass",
            "devuelve los graves",
            "regresa el bajo",
        ],
        actions: &["deck {deck} eq_low 1"],
        reply: "Bass back on deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &["kill the highs", "no highs", "quita los agudos"],
        actions: &["deck {deck} eq_high 0"],
        reply: "Highs out on deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &["kill the mids", "no mids", "quita los medios"],
        actions: &["deck {deck} eq_mid 0"],
        reply: "Mids out on deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &["flat eq", "reset the eq", "eq plano", "reinicia el eq"],
        actions: &[
            "deck {deck} eq_low 1",
            "deck {deck} eq_mid 1",
            "deck {deck} eq_high 1",
        ],
        reply: "EQ flat on deck {deck}.",
        default_deck: 1,
    },
    // -- crossfader
    Rule {
        phrases: &[
            "go to deck 1",
            "cut to deck 1",
            "crossfade left",
            "all the way left",
            "pasa al uno",
        ],
        actions: &["crossfader -1"],
        reply: "Crossfader hard left.",
        default_deck: 1,
    },
    Rule {
        phrases: &[
            "go to deck 2",
            "cut to deck 2",
            "crossfade right",
            "all the way right",
            "pasa al dos",
        ],
        actions: &["crossfader 1"],
        reply: "Crossfader hard right.",
        default_deck: 2,
    },
    Rule {
        phrases: &[
            "centre the crossfader",
            "center the crossfader",
            "crossfader middle",
            "centra el crossfader",
        ],
        actions: &["crossfader 0"],
        reply: "Crossfader centred.",
        default_deck: 1,
    },
    // -- headphones
    Rule {
        phrases: &[
            "cue deck",
            "in my headphones",
            "let me hear",
            "en los audifonos",
            "en mis audifonos",
        ],
        actions: &["deck {deck} cue_on"],
        reply: "Deck {deck} in the headphones.",
        default_deck: 2,
    },
    // -- keylock
    Rule {
        phrases: &["keylock on", "lock the key", "bloquea el tono"],
        actions: &["deck {deck} keylock_on"],
        reply: "Keylock on for deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &["keylock off", "unlock the key", "quita el bloqueo"],
        actions: &["deck {deck} keylock_off"],
        reply: "Keylock off for deck {deck}.",
        default_deck: 1,
    },
    // -- transport, last, because its phrases are the shortest and would
    // otherwise swallow the more specific rules above.
    Rule {
        phrases: &["play", "start", "go", "pon", "dale", "arranca"],
        actions: &["deck {deck} play"],
        reply: "Playing deck {deck}.",
        default_deck: 1,
    },
    Rule {
        phrases: &["pause", "stop", "hold", "para", "detente", "pausa"],
        actions: &["deck {deck} pause"],
        reply: "Deck {deck} paused.",
        default_deck: 1,
    },
    Rule {
        phrases: &["cue", "back to the cue", "al cue"],
        actions: &["deck {deck} cue"],
        reply: "Deck {deck} back at the cue point.",
        default_deck: 1,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(text: &str) -> Vec<String> {
        match_local(text)
            .unwrap_or_else(|| panic!("`{text}` should have matched"))
            .actions
    }

    #[test]
    fn plain_transport_commands_match() {
        assert_eq!(actions("play deck 1"), ["deck 1 play"]);
        assert_eq!(actions("deck 2 play"), ["deck 2 play"]);
        assert_eq!(actions("stop deck 2"), ["deck 2 pause"]);
        assert_eq!(actions("cue deck 1"), ["deck 1 cue_on"]);
    }

    #[test]
    fn deck_numbers_are_read_as_words_too() {
        assert_eq!(actions("play deck two"), ["deck 2 play"]);
        assert_eq!(actions("play deck three"), ["deck 3 play"]);
    }

    /// The commands actually shouted at one in the morning.
    #[test]
    fn spanish_works_as_well_as_english() {
        assert_eq!(actions("pon el deck 2"), ["deck 2 play"]);
        assert_eq!(actions("para el deck 1"), ["deck 1 pause"]);
        assert_eq!(actions("quita los graves del deck 2"), ["deck 2 eq_low 0"]);
        assert_eq!(actions("bloquea el tono deck 1"), ["deck 1 keylock_on"]);
    }

    #[test]
    fn left_and_right_name_decks() {
        assert_eq!(actions("play the left deck"), ["deck 1 play"]);
        assert_eq!(
            actions("kill the bass on the right deck"),
            ["deck 2 eq_low 0"]
        );
    }

    /// A specific phrase must win over a general one that also matches. "Bring
    /// back the bass on deck 2" contains no transport verb, but "drop the bass"
    /// would be caught by a naive ordering.
    #[test]
    fn specific_phrases_beat_general_ones() {
        assert_eq!(actions("kill the bass on deck 2"), ["deck 2 eq_low 0"]);
        assert_eq!(
            actions("bring back the bass on deck 2"),
            ["deck 2 eq_low 1"]
        );
    }

    #[test]
    fn one_phrase_can_produce_several_actions() {
        assert_eq!(
            actions("flat eq on deck 2"),
            ["deck 2 eq_low 1", "deck 2 eq_mid 1", "deck 2 eq_high 1"]
        );
    }

    #[test]
    fn the_crossfader_understands_both_ends() {
        assert_eq!(actions("go to deck 2"), ["crossfader 1"]);
        assert_eq!(actions("go to deck 1"), ["crossfader -1"]);
        assert_eq!(actions("centre the crossfader"), ["crossfader 0"]);
    }

    /// **Everything this emits must parse.** The matcher's output goes through
    /// the same door as a model's, so a typo in a rule would be a runtime
    /// failure in front of a room rather than a compile error.
    #[test]
    fn every_rule_emits_parseable_action_text() {
        for rule in RULES {
            for deck in 1..=4u8 {
                for template in rule.actions {
                    let text = template.replace("{deck}", &deck.to_string());
                    dj_core::Action::parse(&text).unwrap_or_else(|e| {
                        panic!(
                            "rule `{}` produced `{text}`, which does not parse: {e}",
                            rule.phrases[0]
                        )
                    });
                }
            }
        }
    }

    #[test]
    fn every_rule_has_a_reply_and_a_phrase() {
        for rule in RULES {
            assert!(!rule.phrases.is_empty());
            assert!(!rule.reply.is_empty());
            assert!(!rule.actions.is_empty());
        }
    }

    /// A near-miss must fall through to the model rather than being guessed at.
    /// Guessing wrong in front of a room is much worse than a round trip.
    #[test]
    fn anything_unclear_falls_through_to_the_model() {
        for text in [
            "",
            "   ",
            "what bpm is this",
            "find me something like this but slower",
            "plan the next half hour",
            "who produced this track",
        ] {
            assert!(
                match_local(text).is_none(),
                "`{text}` should have gone to the model"
            );
        }
    }

    /// Substring matching would let "unplayable" or "display" trigger play.
    #[test]
    fn phrases_match_whole_words_only() {
        assert!(match_local("the display is broken").is_none());
        assert!(match_local("this track is unplayable").is_none());
    }

    /// A bare number is only a deck when something nearby says it is, or
    /// "set eq_low 2" and "give me 2 minutes" would move decks.
    #[test]
    fn a_bare_number_is_not_a_deck() {
        // No deck word: falls back to the rule's default rather than deck 2.
        assert_eq!(actions("play"), ["deck 1 play"]);
        assert_eq!(actions("play channel 2"), ["deck 2 play"]);
    }

    #[test]
    fn a_reply_names_the_deck_it_acted_on() {
        let matched = match_local("kill the bass on deck 2").unwrap();
        assert!(matched.reply.contains('2'), "{}", matched.reply);
    }
}

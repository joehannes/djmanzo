//! [§40](../../../docs/DIRECTIVE.md): what the assistant is told, and what it is not.
//!
//! # The assistant was blind
//!
//! `commands::ask` handed the model a system prompt, the action vocabulary and
//! the DJ's sentence. Nothing else. Not what was playing, not the tempo, not
//! the key, not where either record was — so "bring the next one in on the
//! phrase" was answered by something that could not see a phrase, and the only
//! reason that ever looked like it worked is that most of what a DJ asks for is
//! an action with the answer already in the sentence.
//!
//! §40 lists twenty-six things the AI context should include. This is that
//! list, kept in §40's own order and words, with what carries each — and,
//! where nothing does, why not.
//!
//! # Why the unseen half is in the table rather than left out
//!
//! A briefing that quietly omits the library reads, to anyone looking at it,
//! exactly like a briefing whose model has the whole library and did not use
//! it. The DJ is the one who has to decide whether to trust an answer, and
//! "it could not see your history" is the single most useful thing to know
//! about an answer that ignored it. So the panel draws both halves from this
//! table, and a reason is required for every absence.
//!
//! # Pointers rather than a second hand-written copy of the snapshot
//!
//! Each gathered item names a JSON pointer into the snapshot djmanzo already
//! sends the interface sixty times a second, and
//! [`tests::every_gathered_item_points_at_a_field_the_snapshot_really_has`]
//! resolves every one of them against a real capture. A table claiming the
//! assistant can see the cue points through a field that does not exist is the
//! failure this whole module is about, one level up.

use serde::Serialize;

/// Where one of §40's items comes from, or why it does not come at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum Carrier {
    /// A field on each deck, as a JSON pointer relative to one deck.
    Deck { pointer: &'static str },
    /// A field of the master section, relative to `/master`.
    Master { pointer: &'static str },
    /// A field of the session context, relative to `/context`.
    Context { pointer: &'static str },
    /// Held beside the assistant rather than on the snapshot: the posture the
    /// DJ set and the occasion they declared.
    Conduct,
    /// Not told, and why not. Never an empty string -- see the module note.
    Unseen { because: &'static str },
}

impl Carrier {
    /// Whether the model is told this.
    #[must_use]
    pub const fn told(self) -> bool {
        !matches!(self, Self::Unseen { .. })
    }
}

/// One of the twenty-six things §40 says the AI context should include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Item {
    /// §40's own word for it, so the list can be checked against the directive.
    pub name: &'static str,
    /// What a DJ would understand it to mean, in one line.
    pub about: &'static str,
    /// What the number is in, where that is not obvious. Empty for the rest.
    ///
    /// Carried because the briefing puts a position in seconds beside a cue in
    /// frames, and a reader given both as bare numbers -- model or person --
    /// will take the second for the first. Stating the unit costs two tokens.
    pub unit: &'static str,
    pub carrier: Carrier,
}

const fn deck(name: &'static str, about: &'static str, pointer: &'static str) -> Item {
    Item {
        name,
        about,
        unit: "",
        carrier: Carrier::Deck { pointer },
    }
}

const fn measured(
    name: &'static str,
    about: &'static str,
    pointer: &'static str,
    unit: &'static str,
) -> Item {
    Item {
        name,
        about,
        unit,
        carrier: Carrier::Deck { pointer },
    }
}

const fn unseen(name: &'static str, about: &'static str, because: &'static str) -> Item {
    Item {
        name,
        about,
        unit: "",
        carrier: Carrier::Unseen { because },
    }
}

/// §40's list, in §40's order.
///
/// The order is not decoration: the panel draws them in it, and a reader
/// checking this against the directive should be able to go down both at once.
pub const ALL: &[Item] = &[
    deck("current tracks", "What is on each deck.", "/title"),
    measured(
        "play positions",
        "How far through each record is.",
        "/position_seconds",
        "seconds",
    ),
    deck(
        "BPM",
        "The tempo each deck is actually running at, pitch included.",
        "/effective_bpm",
    ),
    deck(
        "beat grids",
        "Where the beat is, and how sure the analysis is of it.",
        "/beat_phase",
    ),
    deck(
        "key",
        "The camelot key of each record.",
        "/analysis/key_camelot",
    ),
    Item {
        name: "energy",
        about: "How loud the mix is, against tonight's own range.",
        unit: "",
        carrier: Carrier::Context {
            pointer: "/audio/loudness",
        },
    },
    deck(
        "phrase structure",
        "How long a phrase is and where one starts.",
        "/analysis/phrase_beats",
    ),
    deck(
        "stems",
        "Which of the four parts of each record are muted.",
        "/stem_mutes",
    ),
    unseen(
        "current transitions",
        "The mix in progress, as a thing with a shape and a length.",
        "a transition happens over time and the briefing is one instant. What it \
         does carry is the crossfader and what each deck is doing, which is the \
         evidence a transition is under way rather than the transition itself",
    ),
    measured(
        "cue points",
        "The hot cues set on each deck.",
        "/hot_cues",
        "frames",
    ),
    deck(
        "loop state",
        "The loop in force on each deck, if any.",
        "/active_loop",
    ),
    deck("FX", "The effects running on each deck.", "/fx"),
    Item {
        name: "mixer state",
        about: "Where the crossfader is.",
        unit: "",
        carrier: Carrier::Master {
            pointer: "/crossfader",
        },
    },
    unseen(
        "library",
        "The whole collection.",
        "a collection is thousands of records and a briefing carrying it would be \
         the database in a prompt. The assistant searches it instead, which is \
         what a search is for",
    ),
    unseen(
        "prepared tracks",
        "What is staged and on its way to a deck.",
        "staged records are the browser's and the snapshot carries what the engine \
         has. Gathering them is a second read the briefing does not yet make",
    ),
    unseen(
        "next candidates",
        "What the rail would offer next.",
        "the rail is computed from the library on demand and is not on the snapshot",
    ),
    unseen(
        "history",
        "What has already been played tonight.",
        "tonight's log is read back by a command rather than carried on the snapshot",
    ),
    unseen(
        "session plan",
        "The shape of the night, as a sequence.",
        "the plan is the planner's and is not on the snapshot",
    ),
    unseen(
        "user preferences",
        "What this DJ has been learnt to like.",
        "the profile is §12's and is read where suggestions are ranked, not here",
    ),
    Item {
        name: "current venue/occasion",
        about: "What the DJ declared the night to be.",
        unit: "",
        carrier: Carrier::Conduct,
    },
    unseen(
        "audience context",
        "What the room is doing.",
        "the room reading lives with the panel that opens the camera, and stops \
         when it closes -- so a briefing carrying it would go on claiming a room \
         nothing is looking at",
    ),
    unseen(
        "hardware",
        "What is plugged in and what it can reach.",
        "§53's controller profile is a command away rather than on the snapshot",
    ),
    Item {
        name: "assistant posture",
        about: "How much the DJ has allowed it to do.",
        unit: "",
        carrier: Carrier::Conduct,
    },
    Item {
        name: "session phase",
        about: "Where the night is in its arc, and how sure djmanzo is.",
        unit: "",
        carrier: Carrier::Context {
            pointer: "/session",
        },
    },
    unseen(
        "recent actions",
        "What the DJ has just done.",
        "the session log is a command away, and the last few gestures are a \
         different question from the state they left behind",
    ),
    unseen(
        "current GUI focus",
        "Which arrangement is on screen and what it is for.",
        "the cockpit's workspace is a command away rather than on the snapshot",
    ),
];

/// What the assistant is told, as lines.
///
/// Built from the snapshot the interface is already being sent, so what the
/// model is told and what the DJ is looking at cannot disagree -- which is the
/// whole reason the briefing is derived rather than assembled a second time
/// from the registry.
///
/// Only loaded decks appear. A line saying deck 3 holds nothing is a line the
/// model has to read past, and §18's argument about a DJ's attention applies to
/// a context window too.
///
/// A *loaded* deck's empty fields do appear, as `none`. "There is no loop" and
/// "nobody told me about the loop" are different answers to "get out of the
/// loop", and only one of them is true.
#[must_use]
pub fn brief(snapshot: &serde_json::Value, posture: &str, occasion: &str) -> Vec<String> {
    let mut lines = Vec::new();

    let decks = snapshot
        .get("decks")
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    for deck in decks {
        if deck.get("loaded").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        let number = deck
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let mut parts = Vec::new();
        for item in ALL {
            let Carrier::Deck { pointer } = item.carrier else {
                continue;
            };
            if let Some(value) = deck.pointer(pointer) {
                parts.push(format!("{}{} {}", item.name, unit(item), render(value)));
            }
        }
        if !parts.is_empty() {
            lines.push(format!("deck {number}: {}", parts.join(", ")));
        }
    }

    for item in ALL {
        let value = match item.carrier {
            Carrier::Master { pointer } => snapshot.pointer(&format!("/master{pointer}")),
            Carrier::Context { pointer } => snapshot.pointer(&format!("/context{pointer}")),
            _ => None,
        };
        if let Some(value) = value {
            lines.push(format!("{}{}: {}", item.name, unit(item), render(value)));
        }
    }

    lines.push(format!("assistant posture: {posture}"));
    lines.push(format!("current venue/occasion: {occasion}"));
    lines
}

/// An item's unit, ready to follow its name.
fn unit(item: &Item) -> String {
    if item.unit.is_empty() {
        String::new()
    } else {
        format!(" in {}", item.unit)
    }
}

/// A value as one short piece of text.
///
/// Floats are cut to two decimals: a tempo with six of them is six tokens
/// spent saying nothing, and nothing the model does with it needs the rest.
fn render(value: &serde_json::Value) -> String {
    match value {
        // Absence, said out loud. See `brief`.
        serde_json::Value::Null => "none".to_owned(),
        // An array with holes in it. Eight cue slots of which one is set is
        // seven `null`s the reader has to count past; what is being asked is
        // "where are the cues", and the empty slots are not an answer to it.
        serde_json::Value::Array(items) if items.iter().any(serde_json::Value::is_null) => {
            let set: Vec<&serde_json::Value> =
                items.iter().filter(|item| !item.is_null()).collect();
            if set.is_empty() {
                "none".to_owned()
            } else {
                render(&serde_json::Value::Array(
                    set.into_iter().cloned().collect(),
                ))
            }
        }
        // A run of slots. djmanzo's slot shapes -- effects especially -- carry
        // a `kind` and a handful of settings, and the settings of an empty slot
        // are a dozen tokens saying nothing. The kinds are the answer to "what
        // is running", which is what §40 asks for.
        serde_json::Value::Array(items)
            if !items.is_empty()
                && items
                    .iter()
                    .all(|item| item.get("kind").is_some_and(serde_json::Value::is_string)) =>
        {
            items
                .iter()
                .filter_map(|item| item.get("kind").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("/")
        }
        serde_json::Value::Number(n) => n.as_f64().map_or_else(
            || n.to_string(),
            |f| {
                if (f - f.round()).abs() < 1e-9 {
                    format!("{}", f.round())
                } else {
                    format!("{f:.2}")
                }
            },
        ),
        serde_json::Value::String(s) => s.clone(),
        other => {
            let text = other.to_string();
            // A briefing is not a place for a hundred-character array. The cap
            // is generous enough for four stems and eight cue slots and mean
            // enough that a field that grows unexpectedly cannot flood it.
            if text.chars().count() > 120 {
                text.chars().take(117).chain("...".chars()).collect()
            } else {
                text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real capture of the running application.
    ///
    /// The browser's fixture rather than a registry built here, and the
    /// difference matters: a fresh registry has nothing loaded, so every
    /// `/analysis/...` pointer would resolve to nothing and this test would
    /// pass by never looking. `e2e_fixture::the_browser_fixture_has_the_shape_
    /// the_application_sends` holds that file to the current `Snapshot` type,
    /// so a pointer that resolves here is a pointer into the real thing.
    fn captured() -> serde_json::Value {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/e2e/snapshot.json");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the captured snapshot at {path}: {e}"));
        serde_json::from_str(&text).expect("the capture is JSON")
    }

    /// **The load-bearing one: a gathered item names a field that exists.**
    ///
    /// The whole table is a claim about what the assistant can see, and the
    /// claim is made in strings. A pointer with a typo in it, or one naming a
    /// field that has since been renamed, produces a table that says the
    /// assistant sees the key and a briefing with no key in it -- and the DJ,
    /// reading the panel, would have no way to tell. That is the same defect
    /// one level up from the one this module exists to fix.
    #[test]
    fn every_gathered_item_points_at_a_field_the_snapshot_really_has() {
        let snapshot = captured();
        let decks = snapshot["decks"]
            .as_array()
            .expect("the capture has decks")
            .iter()
            .filter(|deck| deck["loaded"].as_bool().unwrap_or(false))
            .collect::<Vec<_>>();
        assert!(
            !decks.is_empty(),
            "the captured snapshot has no loaded deck, so every deck pointer below \
             would be checked against a record that is not there"
        );

        let mut checked = 0;
        for item in ALL {
            let found = match item.carrier {
                Carrier::Deck { pointer } => {
                    decks.iter().any(|deck| deck.pointer(pointer).is_some())
                }
                Carrier::Master { pointer } => {
                    snapshot.pointer(&format!("/master{pointer}")).is_some()
                }
                Carrier::Context { pointer } => {
                    snapshot.pointer(&format!("/context{pointer}")).is_some()
                }
                // Not on the snapshot by definition.
                Carrier::Conduct | Carrier::Unseen { .. } => continue,
            };
            assert!(
                found,
                "§40's `{}` says it is carried by a snapshot field that is not there. \
                 Either the field was renamed and this table now promises the assistant \
                 something it cannot see, or the pointer has a typo in it",
                item.name
            );
            checked += 1;
        }
        assert!(
            checked >= 10,
            "only {checked} items name a snapshot field, so this checked almost nothing"
        );
    }

    /// The list is §40's, in §40's order and §40's words.
    ///
    /// Written out here rather than derived, because this is the one place the
    /// directive's own text has to be copied: a table that quietly dropped
    /// *phrase structure* would make the panel's "and here is what it cannot
    /// see" list wrong by omission, which is worse than saying nothing.
    #[test]
    fn the_list_is_the_twenty_six_things_40_asks_for() {
        let names: Vec<&str> = ALL.iter().map(|item| item.name).collect();
        assert_eq!(
            names,
            vec![
                "current tracks",
                "play positions",
                "BPM",
                "beat grids",
                "key",
                "energy",
                "phrase structure",
                "stems",
                "current transitions",
                "cue points",
                "loop state",
                "FX",
                "mixer state",
                "library",
                "prepared tracks",
                "next candidates",
                "history",
                "session plan",
                "user preferences",
                "current venue/occasion",
                "audience context",
                "hardware",
                "assistant posture",
                "session phase",
                "recent actions",
                "current GUI focus",
            ]
        );
    }

    /// Nothing is absent without a reason a DJ could act on.
    #[test]
    fn nothing_is_absent_without_a_reason() {
        for item in ALL {
            let Carrier::Unseen { because } = item.carrier else {
                continue;
            };
            assert!(
                because.len() > 30,
                "§40's `{}` is not gathered and says `{because}`, which is a label \
                 rather than a reason. The panel shows this to the DJ deciding \
                 whether to trust an answer",
                item.name
            );
            assert!(
                !item.about.is_empty(),
                "§40's `{}` has no description",
                item.name
            );
        }
        assert!(
            ALL.iter().any(|item| item.carrier.told()),
            "nothing at all is gathered"
        );
        assert!(
            ALL.iter().any(|item| !item.carrier.told()),
            "every one of §40's twenty-six is claimed to be gathered, which has not \
             been true of any version of this application"
        );
    }

    /// **Every item the table calls gathered really appears in the briefing.**
    ///
    /// The table and the briefing are two descriptions of one thing, and this
    /// is the join between them. Without it the panel could go on telling a DJ
    /// the assistant knows the key while the lines handed to the model carry
    /// no key at all -- a table read by nobody, which is the defect class this
    /// repository has recorded most often.
    #[test]
    fn every_item_the_table_calls_gathered_reaches_the_briefing() {
        let lines = brief(&captured(), "suggest", "club");
        let text = lines.join("\n");
        for item in ALL {
            if !item.carrier.told() {
                assert!(
                    !text.contains(item.name),
                    "§40's `{}` is in the briefing and the table says it is not gathered",
                    item.name
                );
                continue;
            }
            assert!(
                text.contains(item.name),
                "the table says the assistant is told §40's `{}` and the briefing has \
                 no line for it:\n{text}",
                item.name
            );
        }
    }

    /// A deck with nothing on it is not described.
    #[test]
    fn an_empty_deck_costs_the_briefing_nothing() {
        let snapshot = serde_json::json!({
            "decks": [
                { "number": 1, "loaded": true, "title": "Something", "position_seconds": 12.5 },
                { "number": 2, "loaded": false, "title": null },
            ],
            "master": { "crossfader": 0.5 },
            "context": { "audio": { "loudness": 0.4 }, "session": null },
        });
        let lines = brief(&snapshot, "watch", "wedding");
        let text = lines.join("\n");

        assert!(text.contains("deck 1: current tracks Something"), "{text}");
        assert!(
            !text.contains("deck 2"),
            "an empty deck was described: {text}"
        );
        // Absence is said rather than left out -- see `brief`.
        assert!(text.contains("session phase: none"), "{text}");
        assert!(text.contains("assistant posture: watch"), "{text}");
        assert!(text.contains("current venue/occasion: wedding"), "{text}");
    }

    /// Numbers arrive short.
    #[test]
    fn a_tempo_is_two_decimals_and_a_whole_number_has_none() {
        let snapshot = serde_json::json!({
            "decks": [{ "number": 1, "loaded": true, "effective_bpm": 128.499_998, "beat_phase": 0.0 }],
            "master": {},
            "context": {},
        });
        let text = brief(&snapshot, "off", "club").join("\n");
        assert!(text.contains("BPM 128.50"), "{text}");
        assert!(text.contains("beat grids 0"), "{text}");
    }
}

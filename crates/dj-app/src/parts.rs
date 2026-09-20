//! §67's fourteen, as a table: what a session contains and where each part is.
//!
//! > # The session is a loop, not a screen
//! >
//! > The application's central conceptual object should become: **Session**. A
//! > session contains: timeline, tracks, transitions, room state, DJ state,
//! > audience response, workspace, learned context, notes, requests, AI
//! > interventions, manual interventions, phase, set arc. **The GUI is simply a
//! > live window into that session.**
//!
//! # Why this is a table and not a struct
//!
//! The obvious reading of §67 is a `Session` struct with fourteen fields, and
//! it is the wrong one. Thirteen of the fourteen already have an owner that
//! decides them — the workspace is [`crate::cockpit`]'s, the requests are
//! [`crate::audience`]'s, what was learned is [`crate::profile`]'s — and a
//! second copy gathered into one object is the failure this codebase has now
//! found six times from the other side: two descriptions of one fact, free to
//! disagree, with nothing saying which is right.
//!
//! # Writing it down was the work, and it was wrong twice
//!
//! Three of the fourteen rows named the wrong module or a command that does
//! not exist, and every one of them read as an answer until a test resolved
//! it. *Room state* pointed at [`crate::audience`], which is the room's
//! **requests** — the page a stranger types into — and not the room's state at
//! all; two things called audience is exactly how a row points somewhere
//! plausible and wrong. *Notes* pointed at [`crate::memory`], which is
//! *finding a record from what you remember of it*: a search, not a notebook,
//! sharing a word and nothing else. And three commands were named from
//! recollection and did not exist under those names.
//!
//! That is the argument for the table rather than a paragraph. A paragraph
//! claiming djmanzo has all fourteen would have been just as wrong and nothing
//! would ever have said so.
//!
//! §11 met the same problem and answered it the same way. `DJContext` is one
//! object **gathered in one pass** rather than a second store, and what makes
//! it real is `context::FIELDS` — a table held against the serialised shape by
//! a test. This is that, pointed at §67.
//!
//! # What the table is actually for
//!
//! §67's last line is the claim worth testing: *the GUI is simply a live
//! window into that session*. So every part names **how it reaches the
//! interface**, and the test resolves that:
//!
//! - [`Reach::Snapshot`] is the live window — a JSON pointer into the sixty-a-
//!   second snapshot, checked against a real capture the way
//!   [`crate::sight`]'s pointers are.
//! - [`Reach::Asked`] is a window the interface has to open: a command, called
//!   when a panel needs the answer. A transition plan is not on the snapshot
//!   and should not be — it changes twice a record, and the snapshot pump
//!   carrying it sixty times a second would be the furniture §90 keeps taking
//!   off it.
//! - [`Reach::Nowhere`] is a part a DJ cannot see, and it carries the sentence
//!   saying why. A row left off the list would make the list read as complete.
//!
//! Writing the two apart is the point. Both are windows; only one is *live*,
//! and §67's sentence is about the live one.

/// One of §67's fourteen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Timeline,
    Tracks,
    Transitions,
    RoomState,
    DjState,
    AudienceResponse,
    Workspace,
    LearnedContext,
    Notes,
    Requests,
    AiInterventions,
    ManualInterventions,
    Phase,
    SetArc,
}

/// How a part of the session reaches the DJ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// On the snapshot, sixty times a second. §67's *live window*.
    ///
    /// The pointer is relative to the whole snapshot, so it begins with the
    /// top-level key — `/context/session`, `/master/automix`. A deck pointer
    /// begins `/decks/0` and is checked against a loaded deck.
    Snapshot(&'static str),
    /// Asked for by name when a panel needs it.
    ///
    /// The command, as it is spelled in `commands.rs`. Still a window into the
    /// session; just not one that refreshes on its own.
    Asked(&'static str),
    /// Nowhere a DJ can see it. [`Part::why_not`] says why.
    Nowhere,
}

impl Part {
    /// §67's fourteen, in §67's order.
    pub const ALL: [Self; 14] = [
        Self::Timeline,
        Self::Tracks,
        Self::Transitions,
        Self::RoomState,
        Self::DjState,
        Self::AudienceResponse,
        Self::Workspace,
        Self::LearnedContext,
        Self::Notes,
        Self::Requests,
        Self::AiInterventions,
        Self::ManualInterventions,
        Self::Phase,
        Self::SetArc,
    ];

    /// §67's own word for it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Timeline => "timeline",
            Self::Tracks => "tracks",
            Self::Transitions => "transitions",
            Self::RoomState => "room state",
            Self::DjState => "DJ state",
            Self::AudienceResponse => "audience response",
            Self::Workspace => "workspace",
            Self::LearnedContext => "learned context",
            Self::Notes => "notes",
            Self::Requests => "requests",
            Self::AiInterventions => "AI interventions",
            Self::ManualInterventions => "manual interventions",
            Self::Phase => "phase",
            Self::SetArc => "set arc",
        }
    }

    /// The module that owns it.
    ///
    /// A path rather than a sentence, so the test can check it exists. A
    /// `crate::` path names a module of this crate; anything else names a
    /// crate djmanzo depends on.
    #[must_use]
    pub const fn home(self) -> &'static str {
        match self {
            // The action log, timestamped since M0, and the file it is written
            // to. A set *is* its timeline here rather than having one.
            Self::Timeline => "crate::session",
            Self::Tracks => "crate::library",
            Self::Transitions => "crate::transition",
            // `dj_assistant::room` rather than `crate::audience`, which is a
            // near-miss worth naming: `audience.rs` is the room's *requests*
            // -- the page strangers type into -- and not the room's state.
            // Two things called audience is how this row would have pointed at
            // the wrong module and still read as an answer.
            Self::RoomState => "dj_assistant::room",
            // §11's `djBehaviorContext`: counts and rates about tonight, never
            // a judgement. Distinct from `learned context`, which is every
            // other night.
            Self::DjState => "crate::context",
            // §37: what the room did after a mix, kept as a finding rather
            // than a sensor feed.
            Self::AudienceResponse => "crate::response",
            Self::Workspace => "crate::cockpit",
            Self::LearnedContext => "crate::profile",
            // The library, not `crate::memory`. `memory.rs` is *finding a
            // record from what you remember of it* -- a search, not a
            // notebook -- and the two share a word and nothing else.
            Self::Notes => "dj_library",
            // `audience.rs` holds the decision to open a port and the shapes
            // the interface reads; `dj_net::front` holds the book and the page
            // a stranger types into.
            Self::Requests => "crate::audience",
            // Both interventions are one table with a column, not two tables.
            // `By` is what tells them apart, and a session file carries it per
            // event -- so "what did djmanzo do tonight" and "what did I do"
            // are one query with two answers rather than two logs that can
            // disagree about the same fader move.
            Self::AiInterventions | Self::ManualInterventions => "dj_control::bus",
            Self::Phase => "dj_core::context",
            Self::SetArc => "crate::asks",
        }
    }

    /// How it reaches the interface.
    #[must_use]
    pub const fn reach(self) -> Reach {
        match self {
            // The log is long and a DJ reads it when they ask to. Putting a
            // whole night's events on a frame built sixty times a second is
            // the furniture §90 exists to keep off it.
            Self::Timeline => Reach::Asked("session_log"),
            Self::Tracks => Reach::Snapshot("/decks/0/title"),
            // Not on the snapshot, deliberately: a plan changes twice a record
            // and the pump runs sixty times a second.
            Self::Transitions => Reach::Asked("plan_transition"),
            Self::RoomState => Reach::Asked("room_read"),
            Self::DjState => Reach::Asked("dj_context"),
            Self::AudienceResponse => Reach::Asked("room_history"),
            Self::Workspace => Reach::Asked("cockpit_workspace"),
            Self::LearnedContext => Reach::Asked("profile_tonight"),
            Self::Notes => Reach::Asked("notes"),
            Self::Requests => Reach::Asked("audience_waiting"),
            // §67's own contribution to this list, and the reason `By` exists:
            // until it did, a crossfader move from a finger, a controller, the
            // autopilot and an accepted transaction arrived looking identical.
            Self::AiInterventions | Self::ManualInterventions => Reach::Asked("session_log"),
            // The live one. §11's engine puts the night's phase, its certainty
            // and what disagrees with it on every frame.
            Self::Phase => Reach::Snapshot("/context/session"),
            Self::SetArc => Reach::Nowhere,
        }
    }

    /// Why a DJ cannot see this part, when they cannot.
    ///
    /// Empty for everything that reaches them. The two halves have to agree
    /// and a test says so, for [`crate::remembered`]'s reason: a "not yet"
    /// beside something that works is djmanzo lying about itself in the one
    /// place it claims to be honest.
    #[must_use]
    pub const fn why_not(self) -> &'static str {
        match self {
            Self::SetArc => {
                "A set arc is a statement about a stretch of time and a phase is a reading of \
                 one minute. djmanzo reads the minute honestly and refuses to extrapolate it \
                 into a four-hour journey, because doing so would be deciding the shape of a \
                 night from its first record. §17's phases are what it does instead."
            }
            _ => "",
        }
    }

    /// Whether a DJ can see this part at all.
    #[must_use]
    pub const fn seen(self) -> bool {
        !matches!(self.reach(), Reach::Nowhere)
    }
}

/// The parts of a session a DJ cannot see, in §67's order.
#[must_use]
pub fn unseen() -> Vec<Part> {
    Part::ALL.into_iter().filter(|p| !p.seen()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(file: &str) -> Option<&'static str> {
        match file {
            "commands.rs" => Some(include_str!("commands.rs")),
            "lib.rs" => Some(include_str!("lib.rs")),
            _ => None,
        }
    }

    fn captured() -> serde_json::Value {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/snapshot.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{}: {error}\n\nRecapture it from the running application; see \
                 `tests/e2e_fixture.rs`.",
                path.display()
            )
        });
        serde_json::from_str(&text).expect("the captured snapshot is JSON")
    }

    /// **Every module this table names is one that exists.**
    ///
    /// The house pattern, and the reason it is worth repeating here: §67's
    /// list is fourteen claims about where things live, made in strings. A row
    /// naming a module that has been renamed or absorbed is a row that reads
    /// as an answer and is not one — the same failure `Remembered::kept_in`
    /// catches one section over.
    #[test]
    fn every_part_names_a_module_that_exists() {
        let lib = source("lib.rs").expect("lib.rs is in the tree");
        for part in Part::ALL {
            let home = part.home();
            if let Some(module) = home.strip_prefix("crate::") {
                assert!(
                    lib.contains(&format!("pub mod {module};"))
                        || lib.contains(&format!("mod {module};")),
                    "{} says it lives in `{home}`, and lib.rs declares no such module",
                    part.name()
                );
            } else {
                let krate = home.split("::").next().unwrap_or(home);
                assert!(
                    lib.contains(krate) || source("commands.rs").unwrap().contains(krate),
                    "{} says it lives in `{home}`, and nothing in this crate mentions {krate}",
                    part.name()
                );
            }
        }
    }

    /// **The load-bearing one: every command this table names is one djmanzo
    /// actually offers.**
    ///
    /// §67's claim is that the interface is a window into the session, and for
    /// eleven of the fourteen the window is a command. A row naming a command
    /// that does not exist is a part of the session nothing can look at, with
    /// a table saying otherwise.
    #[test]
    fn every_part_that_is_asked_for_names_a_command_that_exists() {
        let commands = source("commands.rs").expect("commands.rs is in the tree");
        let mut checked = 0;
        for part in Part::ALL {
            let Reach::Asked(command) = part.reach() else {
                continue;
            };
            assert!(
                commands.contains(&format!("pub fn {command}(")),
                "{} is asked for with `{command}`, and commands.rs has no such command",
                part.name()
            );
            checked += 1;
        }
        assert!(
            checked >= 8,
            "only {checked} parts claimed a command, so this test checked almost nothing"
        );
    }

    /// **And every live one points at a field the snapshot really has.**
    ///
    /// `crate::sight`'s test, applied to §67's own sentence. A pointer with a
    /// typo in it would have this table claiming the GUI is a live window onto
    /// something that is not on the frame.
    #[test]
    fn every_live_part_points_at_a_field_the_snapshot_really_has() {
        let snapshot = captured();
        let mut checked = 0;
        for part in Part::ALL {
            let Reach::Snapshot(pointer) = part.reach() else {
                continue;
            };
            assert!(
                snapshot.pointer(pointer).is_some(),
                "{} claims the snapshot carries `{pointer}`, and the captured frame has no \
                 such field",
                part.name()
            );
            checked += 1;
        }
        assert!(
            checked >= 2,
            "only {checked} parts claimed to be on the snapshot, so this test checked almost \
             nothing -- and §67's whole sentence is about the live window"
        );
    }

    /// **What a DJ cannot see says so, and says why.**
    ///
    /// The two halves have to agree, for `crate::remembered`'s reason.
    #[test]
    fn the_parts_nobody_can_see_are_the_parts_that_explain_themselves() {
        for part in Part::ALL {
            assert_eq!(
                part.seen(),
                part.why_not().is_empty(),
                "{} is inconsistent about whether a DJ can see it",
                part.name()
            );
        }
    }

    /// **Fourteen rows, and fourteen different ones.**
    ///
    /// §67 lists fourteen. A fifteenth that is really one of them under
    /// another name would make the list look more complete than it is, and a
    /// missing one would make it look less.
    #[test]
    fn the_table_is_sixty_sevens_own_list() {
        assert_eq!(Part::ALL.len(), 14);
        let mut names: Vec<&str> = Part::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "two parts share a name");

        for part in Part::ALL {
            assert!(!part.name().is_empty());
            assert!(!part.home().is_empty());
        }
    }

    /// **The one djmanzo refuses is the one §11 already refused.**
    ///
    /// Not a coincidence worth leaving implicit: §11's row and this one are
    /// the same refusal seen from two sections, and if one of them ever ships
    /// a set arc while the other still says it cannot, a DJ reading either
    /// would be told something untrue.
    #[test]
    fn the_set_arc_is_the_only_part_with_no_window() {
        assert_eq!(unseen(), vec![Part::SetArc]);
        assert!(
            Part::SetArc.why_not().contains("phase"),
            "the refusal should say what djmanzo does instead"
        );
    }
}

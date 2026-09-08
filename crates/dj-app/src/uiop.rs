//! What the assistant may ask of the interface, as a closed vocabulary.
//!
//! [§41 of the directive](../../../docs/DIRECTIVE.md) asks for the assistant to
//! be able to request *semantic* GUI operations — show Prepare, expand the
//! next-track rail, focus deck 2, pin the room panel — and is explicit about
//! the thing it must never be able to do: emit JavaScript or mutate the DOM.
//! `docs/GUI-OVERHAUL.md` §14 puts it in one line: this vocabulary "is the
//! GUI's equivalent of the action bus and must be as closed".
//!
//! # Generated, not written down
//!
//! Every operation that names a surface is checked against
//! [`crate::cockpit::surfaces`], so an operation naming a panel djmanzo does
//! not have cannot be parsed, let alone applied. The prompt the model is given
//! is generated from the same table. That is the whole of ADR-0005's argument,
//! applied one layer up: a hand-written list would drift the first time a
//! surface was added, and produce a model confidently asking for things that
//! do not exist.
//!
//! # Why this is not on the action bus
//!
//! An action is something the *engine* does, and the engine has no idea what a
//! panel is. Putting `ui show prepare` into `dj_core::Action` would push
//! knowledge of the cockpit into the crate at the bottom of the dependency
//! graph, and a session replay would then depend on the interface it was
//! recorded against. These are a second closed vocabulary, kept as strict as
//! the first, and deliberately not mixed with it.
//!
//! # Density is not here, on purpose
//!
//! "Switch to compact mode" is on §41's list and is the one item left out. The
//! interface picks its density from the window it is in — measured bands, see
//! `cockpit::BANDS` — and an assistant overriding that would be adaptation
//! fighting adaptation, with the DJ unable to tell which of the two had last
//! word. Resizing the window is how density changes.

use serde::Serialize;

/// One thing the assistant may ask the interface to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum UiOp {
    /// Open a surface, or leave it open if it already is.
    Show { surface: String },
    /// Close one.
    Hide { surface: String },
    /// Hold a surface where it is, out of adaptation's reach.
    Pin { surface: String },
    /// Let it be moved again.
    Unpin { surface: String },
    /// Bring one deck to the DJ's attention.
    ///
    /// The one operation that changes nothing about the arrangement, and so
    /// the one that is not stored: attention is a moment, not a layout.
    Focus { deck: u8 },
}

impl std::fmt::Display for UiOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiOp::Show { surface } => write!(f, "ui show {surface}"),
            UiOp::Hide { surface } => write!(f, "ui hide {surface}"),
            UiOp::Pin { surface } => write!(f, "ui pin {surface}"),
            UiOp::Unpin { surface } => write!(f, "ui unpin {surface}"),
            UiOp::Focus { deck } => write!(f, "ui focus {deck}"),
        }
    }
}

/// Why a line was not an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiOpError {
    /// Not addressed to the interface at all.
    NotUi,
    /// Addressed to it, but not a verb it has.
    UnknownVerb(String),
    /// A verb with nothing after it.
    MissingArgument(&'static str),
    /// A surface djmanzo does not have.
    NoSuchSurface(String),
    /// A deck this rig does not have.
    NoSuchDeck(String),
}

impl std::fmt::Display for UiOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiOpError::NotUi => write!(f, "not an interface operation"),
            UiOpError::UnknownVerb(verb) => write!(f, "the interface has no {verb:?}"),
            UiOpError::MissingArgument(what) => write!(f, "expected a {what}"),
            UiOpError::NoSuchSurface(name) => write!(f, "there is no {name:?} surface"),
            UiOpError::NoSuchDeck(name) => write!(f, "{name:?} is not a deck"),
        }
    }
}

impl UiOp {
    /// Parse the text form: `ui show prepare`, `ui focus 2`.
    ///
    /// Strict about content and tolerant about nothing else. A surface name
    /// that is not in [`crate::cockpit::surfaces`] is refused here rather than
    /// applied and quietly ignored later, which is what makes the vocabulary
    /// closed rather than merely documented.
    ///
    /// # Errors
    /// See [`UiOpError`].
    pub fn parse(input: &str) -> Result<Self, UiOpError> {
        let lowered = input.trim().to_ascii_lowercase();
        let mut words = lowered.split_whitespace();
        if words.next() != Some("ui") {
            return Err(UiOpError::NotUi);
        }
        let verb = words.next().ok_or(UiOpError::MissingArgument("verb"))?;
        if verb == "focus" {
            let number = words.next().ok_or(UiOpError::MissingArgument("deck"))?;
            let deck: u8 = number
                .parse()
                .map_err(|_| UiOpError::NoSuchDeck(number.to_owned()))?;
            dj_core::DeckId::from_human(deck)
                .ok_or_else(|| UiOpError::NoSuchDeck(number.to_owned()))?;
            return Ok(UiOp::Focus { deck });
        }
        let name = words.next().ok_or(UiOpError::MissingArgument("surface"))?;
        let surface = crate::cockpit::surface(name)
            .ok_or_else(|| UiOpError::NoSuchSurface(name.to_owned()))?;
        let surface = surface.name.to_owned();
        match verb {
            "show" => Ok(UiOp::Show { surface }),
            "hide" => Ok(UiOp::Hide { surface }),
            "pin" => Ok(UiOp::Pin { surface }),
            "unpin" => Ok(UiOp::Unpin { surface }),
            other => Err(UiOpError::UnknownVerb(other.to_owned())),
        }
    }

    /// The surface this is about, if it is about one.
    #[must_use]
    pub fn surface(&self) -> Option<&str> {
        match self {
            UiOp::Show { surface }
            | UiOp::Hide { surface }
            | UiOp::Pin { surface }
            | UiOp::Unpin { surface } => Some(surface),
            UiOp::Focus { .. } => None,
        }
    }
}

/// Every operation this build actually accepts, as lines a model can be shown.
///
/// Generated from the surfaces that exist. Deliberately terse: this goes into a
/// system prompt beside the action vocabulary, where every token is paid for on
/// every request, so the surfaces are listed once rather than four times.
#[must_use]
pub fn as_prompt_lines(decks: u8) -> Vec<String> {
    let names: Vec<&str> = crate::cockpit::surfaces()
        .iter()
        .map(|surface| surface.name)
        .collect();
    vec![
        format!(
            "ui show <panel> — open a panel. Panels: {}",
            names.join(", ")
        ),
        "ui hide <panel> — close one".to_owned(),
        "ui pin <panel> — hold a panel where it is".to_owned(),
        "ui unpin <panel> — let it be moved again".to_owned(),
        format!("ui focus <deck> — bring a deck to attention, 1 to {decks}"),
    ]
}

/// What an operation did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Applied {
    /// The arrangement afterwards, resolved the way `set_cockpit_workspace`
    /// resolves it, so what is stored and what is drawn are the same thing.
    pub workspace: crate::cockpit::Resolved,
    /// A deck the interface should bring to attention. Not stored.
    pub focus: Option<u8>,
    /// What was done, in one line, for a DJ who wants to know why a panel
    /// opened on its own.
    pub what: String,
}

/// Carry one operation out against a stored arrangement.
///
/// Pure: it takes the workspace and answers with the next one, so the whole
/// vocabulary can be exercised without a window. Where the surface should go
/// when it is opened is [`crate::cockpit::Surface::prefer`]'s business, not a
/// number chosen here — a rule beats a list of special cases.
#[must_use]
pub fn apply(op: &UiOp, workspace: &crate::cockpit::Workspace) -> Applied {
    let mut next = workspace.clone();
    let mut focus = None;
    let what = match op {
        UiOp::Focus { deck } => {
            focus = Some(*deck);
            format!("focused deck {deck}")
        }
        UiOp::Show { surface } => {
            if next.surfaces.iter().any(|p| &p.surface == surface) {
                format!("{surface} was already open")
            } else {
                next.surfaces.push(crate::cockpit::Placement {
                    surface: surface.clone(),
                    dock: home_for(surface),
                    order: next.surfaces.len() as i32,
                    size: None,
                    collapsed: false,
                    pinned: false,
                });
                format!("opened {surface}")
            }
        }
        UiOp::Hide { surface } => {
            let before = next.surfaces.len();
            next.surfaces.retain(|p| &p.surface != surface);
            if next.surfaces.len() == before {
                format!("{surface} was not open")
            } else {
                format!("closed {surface}")
            }
        }
        UiOp::Pin { surface } | UiOp::Unpin { surface } => {
            let pinned = matches!(op, UiOp::Pin { .. });
            let mut found = false;
            for placement in &mut next.surfaces {
                if &placement.surface == surface {
                    placement.pinned = pinned;
                    found = true;
                }
            }
            match (found, pinned) {
                (false, _) => format!("{surface} is not open"),
                (true, true) => format!("pinned {surface}"),
                (true, false) => format!("unpinned {surface}"),
            }
        }
    };
    Applied {
        workspace: crate::cockpit::resolve(&next),
        focus,
        what,
    }
}

/// Which dock a surface opens into when nothing has said otherwise.
///
/// The surface's own answer. It used to be a rule about the shape of the panel
/// — wider than tall goes along the bottom — which was wrong twice over: it
/// disagreed with the table the interface actually uses, and it happily put
/// `next` along the bottom, which is a dock that surface is not allowed in, so
/// the resolver dropped the placement and "show the rail" opened nothing.
fn home_for(name: &str) -> crate::cockpit::Dock {
    crate::cockpit::surface(name).map_or(crate::cockpit::Dock::Right, |surface| surface.home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cockpit;

    fn empty() -> cockpit::Workspace {
        cockpit::opening()
    }

    /// **The vocabulary is closed.** A panel djmanzo does not have cannot be
    /// asked for, and the refusal happens at the parse rather than being
    /// applied and silently doing nothing.
    #[test]
    fn a_surface_that_does_not_exist_cannot_be_asked_for() {
        assert_eq!(
            UiOp::parse("ui show chatbox"),
            Err(UiOpError::NoSuchSurface("chatbox".to_owned()))
        );
        assert_eq!(
            UiOp::parse("ui explode library"),
            Err(UiOpError::UnknownVerb("explode".to_owned()))
        );
        assert_eq!(UiOp::parse("deck 1 play"), Err(UiOpError::NotUi));
        assert_eq!(
            UiOp::parse("ui show"),
            Err(UiOpError::MissingArgument("surface"))
        );
    }

    /// Every surface djmanzo ships can be asked for by name, and round-trips.
    #[test]
    fn every_surface_can_be_named_and_round_trips() {
        for surface in cockpit::surfaces() {
            let op = UiOp::parse(&format!("ui show {}", surface.name))
                .expect("a surface djmanzo has is one it accepts");
            assert_eq!(op.surface(), Some(surface.name));
            assert_eq!(UiOp::parse(&op.to_string()), Ok(op));
        }
    }

    /// A deck the rig does not have is refused, like a surface it does not have.
    #[test]
    fn a_deck_that_does_not_exist_cannot_be_focused() {
        assert_eq!(UiOp::parse("ui focus 2"), Ok(UiOp::Focus { deck: 2 }));
        assert!(matches!(
            UiOp::parse("ui focus 99"),
            Err(UiOpError::NoSuchDeck(_))
        ));
        assert!(matches!(
            UiOp::parse("ui focus left"),
            Err(UiOpError::NoSuchDeck(_))
        ));
        assert_eq!(
            UiOp::parse("ui focus"),
            Err(UiOpError::MissingArgument("deck"))
        );
    }

    /// Showing opens it; showing again leaves it alone rather than opening a
    /// second copy.
    #[test]
    fn showing_twice_opens_one_panel() {
        let first = apply(
            &UiOp::Show {
                surface: "library".to_owned(),
            },
            &empty(),
        );
        assert_eq!(
            first
                .workspace
                .workspace
                .surfaces
                .iter()
                .filter(|p| p.surface == "library")
                .count(),
            1
        );
        assert!(first.what.contains("opened"));

        let again = apply(
            &UiOp::Show {
                surface: "library".to_owned(),
            },
            &first.workspace.workspace,
        );
        assert_eq!(
            again
                .workspace
                .workspace
                .surfaces
                .iter()
                .filter(|p| p.surface == "library")
                .count(),
            1
        );
        assert!(again.what.contains("already"));
    }

    /// Hiding closes it, and hiding what is not open says so rather than
    /// pretending it did something.
    #[test]
    fn hiding_closes_it_and_says_when_there_was_nothing_to_close() {
        let open = apply(
            &UiOp::Show {
                surface: "room".to_owned(),
            },
            &empty(),
        );
        let shut = apply(
            &UiOp::Hide {
                surface: "room".to_owned(),
            },
            &open.workspace.workspace,
        );
        assert!(shut.workspace.workspace.surfaces.is_empty());
        assert!(shut.what.contains("closed"));

        let again = apply(
            &UiOp::Hide {
                surface: "room".to_owned(),
            },
            &shut.workspace.workspace,
        );
        assert!(again.what.contains("not open"));
    }

    /// Pinning is the per-surface half of "freeze layout", so it has to reach
    /// the placement rather than a flag beside it.
    #[test]
    fn pinning_holds_the_surface_where_it_is() {
        let open = apply(
            &UiOp::Show {
                surface: "next".to_owned(),
            },
            &empty(),
        );
        let pinned = apply(
            &UiOp::Pin {
                surface: "next".to_owned(),
            },
            &open.workspace.workspace,
        );
        assert!(
            pinned
                .workspace
                .workspace
                .surfaces
                .iter()
                .any(|p| p.surface == "next" && p.pinned)
        );
        let loose = apply(
            &UiOp::Unpin {
                surface: "next".to_owned(),
            },
            &pinned.workspace.workspace,
        );
        assert!(loose.workspace.workspace.surfaces.iter().all(|p| !p.pinned));
        // Pinning something that is not open changes nothing and says so.
        let nothing = apply(
            &UiOp::Pin {
                surface: "log".to_owned(),
            },
            &empty(),
        );
        assert!(nothing.what.contains("not open"));
    }

    /// Focus changes nothing about the arrangement. Attention is a moment.
    #[test]
    fn focusing_a_deck_stores_nothing() {
        let before = empty();
        let applied = apply(&UiOp::Focus { deck: 2 }, &before);
        assert_eq!(applied.focus, Some(2));
        assert_eq!(applied.workspace.workspace.surfaces, before.surfaces);
    }

    /// The prompt names only panels that exist, and every one of them.
    #[test]
    fn the_prompt_is_generated_from_the_surfaces_that_exist() {
        let lines = as_prompt_lines(4);
        let listed = lines.join(" ");
        for surface in cockpit::surfaces() {
            assert!(
                listed.contains(surface.name),
                "{} is not offered to the model",
                surface.name
            );
        }
        // And every verb the parser accepts is described.
        for verb in ["show", "hide", "pin", "unpin", "focus"] {
            assert!(
                listed.contains(&format!("ui {verb}")),
                "{verb} is undocumented"
            );
        }
    }

    /// **Every surface opens somewhere it is allowed to be.**
    ///
    /// The bug this is the guard for: `next` may only go beside the decks, and
    /// a home of "bottom" made the resolver drop the placement — so asking the
    /// assistant to show the rail opened nothing at all, silently, with a note
    /// nobody was reading.
    #[test]
    fn every_surface_opens_in_a_dock_it_is_allowed_in() {
        for surface in cockpit::surfaces() {
            let home = home_for(surface.name);
            assert_eq!(home, surface.home);
            assert!(
                surface.docks.contains(&home),
                "{} opens in a dock it cannot be placed in",
                surface.name
            );
            // And it survives the resolver, which is what actually decides.
            let applied = apply(
                &UiOp::Show {
                    surface: surface.name.to_owned(),
                },
                &empty(),
            );
            assert!(
                applied
                    .workspace
                    .workspace
                    .surfaces
                    .iter()
                    .any(|p| p.surface == surface.name),
                "showing {} placed nothing: {:?}",
                surface.name,
                applied.workspace.notes
            );
        }
    }
}

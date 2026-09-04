//! The widget vocabulary: what an interface can be made of.
//!
//! # Why this exists
//!
//! [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md) gave
//! every *behaviour* one named vocabulary, and the payoff was that the
//! assistant, controllers, scripts and the network API all drive the
//! application through the same path with nothing privileged. Nothing
//! equivalent existed for what is *on screen*.
//!
//! What existed instead was [`crate::layout::Layout`]: a flat struct of
//! booleans, one per feature, each matched by a conditional in the interface.
//! It works for the four presets that ship and it cannot do the things a layout
//! file is for -- it cannot move a component, reorder one, put two of something
//! on screen, or name anything the binary does not already know. A file format
//! that can only say what the binary already says is decoration.
//!
//! [ADR-0008](../../../docs/adr/0008-one-widget-vocabulary.md) is the decision
//! to fix that the same way ADR-0003 did: **a layout is a tree of addressed
//! widget instances placed into named slots, not a struct of booleans.**
//!
//! # Three rules, and which one is a security boundary
//!
//! 1. **Slots, never pixel coordinates.** A widget is placed in a named
//!    container and ordered within it. A layout therefore survives a window
//!    resize, a density change, a different deck count, and the interface being
//!    redesigned underneath it. [ADR-0009](../../../docs/adr/0009-the-living-interface.md)
//!    refines this rather than breaking it: a layout file never contains
//!    coordinates, and the running world always has them, because they are
//!    simulated rather than authored.
//! 2. **Restyling is a bounded token set, not CSS.** This is the boundary. A
//!    layout is a thing one DJ sends another, and arbitrary CSS is a
//!    code-execution surface wearing a costume -- `url()` fetches, `@import`
//!    pulls, and selectors reach places the author did not mean. [`token`]
//!    therefore validates by *shape whitelist* rather than by forbidding known
//!    tricks: a colour is hex digits, a length is a number and a unit from a
//!    closed set, and anything else is refused. A blacklist of dangerous
//!    spellings is a list somebody eventually gets past; a whitelist of three
//!    shapes is not.
//! 3. **An unknown name is skipped with a note, never fatal.** A layout written
//!    for a newer djmanzo opens on an older one, minus the parts it cannot
//!    draw. A DJ opening their laptop before a set gets an interface, not a
//!    dialog.
//!
//! # Where this lives
//!
//! In `dj-app`, as ADR-0008 says, and the ADR's own reasoning is a standing
//! argument for moving it later: the network API, controller mappings and the
//! assistant are all named as eventual consumers, and none of them can depend
//! on `dj-app`, because `dj-app` depends on them. Today none of them enumerate
//! widgets, so the move would be speculative. When the first one does, this
//! module is self-contained data with no dependencies and moving it is a file
//! rename.
//!
//! # Names are a compatibility surface
//!
//! A widget name leaks into every DJ's saved layout file the day the feature
//! ships. Renaming one afterwards costs an upconversion path, exactly as an
//! action name does. They are chosen once, carefully.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::layout::Layout;

/// What one of a widget's settings holds, with its default and its bounds.
///
/// The same discipline the parameter registry applies to observable values: a
/// range is declared once, here, and a layout that exceeds it is clamped rather
/// than refused -- a layout is a preference, and a DJ whose file says
/// `rows: 40` wants more pads, not an error message.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropKind {
    Flag {
        default: bool,
    },
    /// A whole number, clamped to `least..=most`.
    Count {
        default: i64,
        least: i64,
        most: i64,
    },
    /// A real number, clamped to `least..=most`.
    Amount {
        default: f64,
        least: f64,
        most: f64,
    },
    /// One of a closed set. Anything else falls back to `default`, because a
    /// misspelt choice has no sensible nearest value the way a number does.
    Choice {
        default: &'static str,
        options: &'static [&'static str],
    },
}

/// One setting a widget accepts.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prop {
    pub name: &'static str,
    /// One line, shown wherever a layout is edited.
    pub about: &'static str,
    pub kind: PropKind,
}

/// A component that can appear on screen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Widget {
    /// The stable dotted name, in the same spirit as an action name.
    pub name: &'static str,
    pub about: &'static str,
    /// The slots this may be placed in. Empty means "nowhere by name", which
    /// is how a widget is retired without breaking the files that mention it.
    pub slots: &'static [&'static str],
    /// The slots this offers to its own children.
    pub offers: &'static [&'static str],
    pub props: &'static [Prop],
    /// What it reads from the engine snapshot, so a widget that is not on
    /// screen can be *proved* not to be paid for rather than assumed not to be.
    pub needs: &'static [&'static str],
}

/// The slots the shell itself offers.
///
/// Named here rather than inferred, because a slot with no widget in it is
/// still a place a layout may refer to, and a typo in a slot name should be
/// reportable.
pub const SLOTS: &[&str] = &[
    "stage",
    "deck",
    "mixer",
    "browser",
    "shell.top",
    "shell.bottom",
    "panel",
    "window.2",
    "window.3",
];

const NO_PROPS: &[Prop] = &[];

/// Every widget djmanzo can draw.
///
/// One table, deliberately. The alternative -- each module registering itself
/// -- reads better and makes the full vocabulary impossible to see in one
/// place, which is the thing a compatibility surface most needs.
#[must_use]
pub fn catalog() -> &'static [Widget] {
    const DECK_SLOT: &[&str] = &["deck"];
    const STAGE: &[&str] = &["stage"];
    const MIXER: &[&str] = &["mixer"];
    const BROWSER: &[&str] = &["browser"];
    const PANEL: &[&str] = &["panel", "window.2", "window.3"];
    const SHELL: &[&str] = &["shell.top", "shell.bottom"];

    &[
        // -- the deck, and the things inside one ---------------------------
        Widget {
            name: "deck",
            about: "One deck. Everything inside it is placed in its `deck` slot.",
            slots: STAGE,
            offers: DECK_SLOT,
            props: &[Prop {
                name: "number",
                about: "Which deck, counting from one.",
                kind: PropKind::Count {
                    default: 1,
                    least: 1,
                    most: 6,
                },
            }],
            needs: &["decks"],
        },
        Widget {
            name: "deck.waveform",
            about: "The scrolling waveform lane.",
            slots: DECK_SLOT,
            offers: &[],
            props: &[Prop {
                name: "height",
                about: "Lane height in pixels.",
                kind: PropKind::Count {
                    default: 96,
                    least: 48,
                    most: 320,
                },
            }],
            needs: &["decks.position", "decks.track"],
        },
        Widget {
            name: "deck.overview",
            about: "The whole track at a glance, under the lane.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.position", "decks.track"],
        },
        Widget {
            name: "deck.transport",
            about: "Cue, play, sync and eject.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.playing"],
        },
        Widget {
            name: "deck.rail",
            about: "§74's contextual rail: the controls this deck's moment calls for.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            // The playhead is not among them: the rail follows what the DJ is
            // doing -- a hand on the platter, a muted stem, a stopped deck --
            // and none of that moves sixty times a second.
            needs: &["decks.playing"],
        },
        Widget {
            name: "deck.pads",
            about: "The pad zone, with its pages.",
            slots: DECK_SLOT,
            offers: &[],
            props: &[
                Prop {
                    name: "rows",
                    about: "Pad rows.",
                    kind: PropKind::Count {
                        default: 2,
                        least: 1,
                        most: 4,
                    },
                },
                Prop {
                    name: "page",
                    about: "The page shown when the deck opens.",
                    kind: PropKind::Choice {
                        default: "cues",
                        options: &["cues", "loops", "roll", "slicer", "saved", "sampler", "fx"],
                    },
                },
            ],
            needs: &["decks.cues", "decks.loops"],
        },
        Widget {
            name: "deck.loops",
            about: "Loop in, out, length and exit.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.loop"],
        },
        Widget {
            name: "deck.beat_jump",
            about: "Jump by a number of beats.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.position"],
        },
        Widget {
            name: "deck.eq",
            about: "Three-band EQ.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.eq"],
        },
        Widget {
            name: "deck.filter",
            about: "The sweep filter.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.filter"],
        },
        Widget {
            name: "deck.pitch",
            about: "Tempo fader and its range.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.tempo"],
        },
        Widget {
            name: "deck.keylock",
            about: "Hold the key when the tempo moves.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.keylock"],
        },
        Widget {
            name: "deck.grid",
            about: "Beat-grid editing -- shift, scale and tap.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.grid"],
        },
        Widget {
            name: "deck.meter",
            about: "Channel level.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.level"],
        },
        // The four the vocabulary was missing.
        //
        // Not an oversight to be tidied away quietly: the deck draws all four,
        // so a layout tree that omits them cannot describe the deck djmanzo
        // has -- which is exactly the thing W3 needs it to do. They are added
        // here rather than special-cased in the renderer, because a control
        // the tree cannot name is a control a skin cannot move.
        Widget {
            name: "deck.jog",
            about: "The jog wheel -- position, and a nudge a mouse can reach.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.position"],
        },
        Widget {
            name: "deck.volume",
            about: "The channel fader.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.volume"],
        },
        Widget {
            name: "deck.cue",
            about: "Send this deck to the headphones.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.cue"],
        },
        Widget {
            name: "deck.xfader",
            about: "Which side of the crossfader this deck answers to.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.xfader"],
        },
        Widget {
            name: "deck.progress",
            about: "How far through the track the playhead is, as a bar.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.position"],
        },
        Widget {
            name: "deck.times",
            about: "Elapsed and remaining.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.position"],
        },
        Widget {
            name: "deck.stems",
            about: "The four stems, with their mutes and volumes.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.stems"],
        },
        Widget {
            name: "deck.fx",
            about: "The three effect slots for this deck.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.fx"],
        },
        Widget {
            name: "deck.perform",
            about: "Slip, reverse, censor, brake and backspin.",
            slots: DECK_SLOT,
            offers: &[],
            props: NO_PROPS,
            needs: &["decks.playing"],
        },
        // -- the mixer -----------------------------------------------------
        Widget {
            name: "mixer.crossfader",
            about: "The crossfader and its curve.",
            slots: MIXER,
            offers: &[],
            props: NO_PROPS,
            needs: &["mixer.crossfader"],
        },
        Widget {
            name: "mixer.master",
            about: "Master gain and its meter.",
            slots: MIXER,
            offers: &[],
            props: NO_PROPS,
            needs: &["mixer.master"],
        },
        Widget {
            name: "mixer.cue",
            about: "Headphone cue mix and volume.",
            slots: MIXER,
            offers: &[],
            props: NO_PROPS,
            needs: &["mixer.cue"],
        },
        Widget {
            name: "mixer.limiter",
            about: "What the master limiter is holding back.",
            slots: MIXER,
            offers: &[],
            props: NO_PROPS,
            needs: &["mixer.limiter"],
        },
        // -- the browser ---------------------------------------------------
        Widget {
            name: "browser.crates",
            about: "Playlists, crates and smart folders.",
            slots: BROWSER,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "browser.tracks",
            about: "The track list.",
            slots: BROWSER,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "browser.sideview",
            about: "The sidelist beside the browser.",
            slots: BROWSER,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "browser.search",
            about: "The search box.",
            slots: BROWSER,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        // -- the shell -----------------------------------------------------
        Widget {
            name: "shell.topbar",
            about: "The bar across the top.",
            slots: SHELL,
            offers: &[],
            props: NO_PROPS,
            needs: &["clock"],
        },
        Widget {
            name: "shell.status",
            about: "Sample rate, latency and load.",
            slots: SHELL,
            offers: &[],
            props: NO_PROPS,
            needs: &["health"],
        },
        Widget {
            name: "shell.log",
            about: "What djmanzo has been doing.",
            slots: SHELL,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        // -- panels --------------------------------------------------------
        Widget {
            name: "panel.assistant",
            about: "The assistant.",
            slots: PANEL,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "panel.presets",
            about: "Preset packs.",
            slots: PANEL,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "panel.settings",
            about: "Settings, keys and sources.",
            slots: PANEL,
            offers: &[],
            props: NO_PROPS,
            needs: &[],
        },
        Widget {
            name: "panel.sampler",
            about: "The sampler banks.",
            slots: PANEL,
            offers: &[],
            props: NO_PROPS,
            needs: &["sampler"],
        },
        Widget {
            name: "panel.world",
            about: "The living view of the mix.",
            slots: PANEL,
            offers: &[],
            props: NO_PROPS,
            needs: &["world"],
        },
    ]
}

/// Look one up by name.
#[must_use]
pub fn widget(name: &str) -> Option<&'static Widget> {
    catalog().iter().find(|known| known.name == name)
}

/// The design tokens a layout may set, and the shape each one's value must
/// have.
///
/// A closed list, because this is the restyling boundary from ADR-0008's rule
/// 2. The `--audio-*` and `--stem-*` custom properties the interface also uses
/// are deliberately absent: they are driven by the running audio, and a layout
/// setting them would be a layout lying about the mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TokenShape {
    /// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
    Colour,
    /// A number and a unit from a closed set.
    Length,
    /// A bare number, clamped to the given range.
    Scale,
}

/// Every token, with the shape its value must take.
pub const TOKENS: &[(&str, TokenShape)] = &[
    ("bg", TokenShape::Colour),
    ("panel", TokenShape::Colour),
    ("panel-raised", TokenShape::Colour),
    ("panel-hover", TokenShape::Colour),
    ("chip", TokenShape::Colour),
    ("text", TokenShape::Colour),
    ("text-dim", TokenShape::Colour),
    ("muted", TokenShape::Colour),
    ("accent", TokenShape::Colour),
    ("accent-2", TokenShape::Colour),
    ("accent-warm", TokenShape::Colour),
    ("accent-soft", TokenShape::Colour),
    ("on-accent", TokenShape::Colour),
    ("border", TokenShape::Colour),
    ("border-strong", TokenShape::Colour),
    ("edge", TokenShape::Colour),
    ("line", TokenShape::Colour),
    ("scrim", TokenShape::Colour),
    ("warn", TokenShape::Colour),
    ("danger", TokenShape::Colour),
    ("radius", TokenShape::Length),
    ("radius-s", TokenShape::Length),
    ("density", TokenShape::Scale),
];

/// The units a length token may carry.
///
/// Relative units and pixels only. No `vw`, `vh` or `%`: a radius that depends
/// on the viewport changes when a panel is detached to another monitor, which
/// is a layout that looks different for reasons the DJ did not author.
const UNITS: &[&str] = &["px", "rem", "em"];

/// The range a [`TokenShape::Scale`] token is clamped to.
const SCALE: (f64, f64) = (0.8, 1.4);

/// Check one token value, returning the value to use.
///
/// **Whitelist, not blacklist.** The refusal that matters is not `url(` or
/// `@import` -- those are the two everybody thinks of, and a list of the ones
/// everybody thinks of is a list somebody gets past. What is accepted here is
/// three shapes: hex digits after a hash, a number and a known unit, or a bare
/// number. Everything else, including every spelling of a fetch, a comment, a
/// selector escape or a nested declaration, falls off the end.
///
/// Returns `None` when the name is not a token or the value is not the shape
/// that token takes.
#[must_use]
pub fn token(name: &str, value: &str) -> Option<String> {
    let shape = TOKENS
        .iter()
        .find_map(|(known, shape)| (*known == name).then_some(*shape))?;
    let value = value.trim();

    match shape {
        TokenShape::Colour => {
            let digits = value.strip_prefix('#')?;
            let ok = matches!(digits.len(), 3 | 4 | 6 | 8)
                && digits.bytes().all(|byte| byte.is_ascii_hexdigit());
            ok.then(|| format!("#{}", digits.to_ascii_lowercase()))
        }
        TokenShape::Length => {
            let unit = UNITS.iter().find(|unit| value.ends_with(**unit))?;
            let number: f64 = value[..value.len() - unit.len()].trim_end().parse().ok()?;
            // A negative or absurd radius is a layout mistake rather than an
            // attack, and clamping keeps it a layout mistake.
            (number.is_finite()).then(|| format!("{}{unit}", number.clamp(0.0, 64.0)))
        }
        TokenShape::Scale => {
            let number: f64 = value.parse().ok()?;
            number
                .is_finite()
                .then(|| number.clamp(SCALE.0, SCALE.1).to_string())
        }
    }
}

/// A widget instance in a layout file, with whatever it was given.
///
/// Props are `serde_json::Value` at this stage because the file is not trusted
/// yet: it is [`resolve`] that turns them into values inside declared ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub widget: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, serde_json::Value>,
    /// Children, by the slot this widget offers them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, Vec<Placement>>,
}

impl Placement {
    /// A placement of `widget` with nothing set.
    #[must_use]
    pub fn of(widget: &str) -> Self {
        Self {
            widget: widget.to_owned(),
            props: BTreeMap::new(),
            children: BTreeMap::new(),
        }
    }

    /// The same, with one prop set.
    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<serde_json::Value>) -> Self {
        self.props.insert(name.to_owned(), value.into());
        self
    }

    /// The same, with children in one of its offered slots.
    #[must_use]
    pub fn holding(mut self, slot: &str, children: Vec<Self>) -> Self {
        self.children.insert(slot.to_owned(), children);
        self
    }
}

/// A layout, as written in a file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Tree {
    pub name: String,
    pub about: String,
    pub tokens: BTreeMap<String, String>,
    pub slots: BTreeMap<String, Vec<Placement>>,
}

/// A widget instance that has been checked: the name exists, the slot allows
/// it, and every prop is present and inside its declared range.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placed {
    pub widget: String,
    pub props: BTreeMap<String, serde_json::Value>,
    pub children: BTreeMap<String, Vec<Placed>>,
}

/// A layout the interface can render without checking anything itself.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Resolved {
    pub name: String,
    pub about: String,
    pub tokens: BTreeMap<String, String>,
    pub slots: BTreeMap<String, Vec<Placed>>,
    /// What was dropped and why.
    ///
    /// Carried rather than logged: a DJ whose layout half-loaded needs to be
    /// able to see which half, and a note the interface can show is worth more
    /// than a line in a file nobody opens.
    pub notes: Vec<String>,
}

/// How deep a layout's nesting may go.
///
/// Four. `stage > deck > ...` is two, and the depth exists so that a file --
/// which may have been written by anything -- cannot cost unbounded recursion.
const DEEPEST: usize = 4;

/// Check a layout against the registry.
///
/// Nothing here fails. Every problem becomes a note and the rest of the layout
/// loads, which is ADR-0008's third rule: a DJ opening their laptop before a
/// set gets an interface, not a dialog.
#[must_use]
pub fn resolve(tree: &Tree) -> Resolved {
    let mut notes = Vec::new();

    let mut tokens = BTreeMap::new();
    for (name, value) in &tree.tokens {
        match token(name, value) {
            Some(checked) => {
                tokens.insert(name.clone(), checked);
            }
            None => notes.push(format!(
                "token `{name}` was not set: `{value}` is not a value that token takes"
            )),
        }
    }

    let mut slots: BTreeMap<String, Vec<Placed>> = BTreeMap::new();
    for (slot, placements) in &tree.slots {
        if !SLOTS.contains(&slot.as_str()) {
            notes.push(format!(
                "no slot called `{slot}`, so its widgets were skipped"
            ));
            continue;
        }
        let placed = place(placements, slot, 0, &mut notes);
        if !placed.is_empty() {
            slots.insert(slot.clone(), placed);
        }
    }

    Resolved {
        name: if tree.name.trim().is_empty() {
            "Custom".to_owned()
        } else {
            tree.name.clone()
        },
        about: tree.about.clone(),
        tokens,
        slots,
        notes,
    }
}

fn place(
    placements: &[Placement],
    slot: &str,
    depth: usize,
    notes: &mut Vec<String>,
) -> Vec<Placed> {
    if depth >= DEEPEST {
        notes.push(format!(
            "`{slot}` is nested deeper than {DEEPEST} levels, so what is below it was skipped"
        ));
        return Vec::new();
    }

    let mut out = Vec::new();
    for placement in placements {
        let Some(known) = widget(&placement.widget) else {
            notes.push(format!(
                "no widget called `{}`, so it was skipped -- a layout written for a newer \
                 djmanzo opens on this one without it",
                placement.widget
            ));
            continue;
        };
        if !known.slots.contains(&slot) {
            notes.push(format!(
                "`{}` cannot go in `{slot}`, so it was skipped",
                known.name
            ));
            continue;
        }

        let mut props = BTreeMap::new();
        for prop in known.props {
            props.insert(
                prop.name.to_owned(),
                settle(prop, placement.props.get(prop.name), known.name, notes),
            );
        }
        for name in placement.props.keys() {
            if !known.props.iter().any(|prop| prop.name == name) {
                notes.push(format!("`{}` has no setting `{name}`", known.name));
            }
        }

        let mut children = BTreeMap::new();
        for (child_slot, kids) in &placement.children {
            if !known.offers.contains(&child_slot.as_str()) {
                notes.push(format!(
                    "`{}` does not offer a `{child_slot}` slot, so what was in it was skipped",
                    known.name
                ));
                continue;
            }
            let placed = place(kids, child_slot, depth + 1, notes);
            if !placed.is_empty() {
                children.insert(child_slot.clone(), placed);
            }
        }

        out.push(Placed {
            widget: known.name.to_owned(),
            props,
            children,
        });
    }
    out
}

/// One prop's value: what was asked for, brought inside what is allowed.
fn settle(
    prop: &Prop,
    given: Option<&serde_json::Value>,
    widget: &str,
    notes: &mut Vec<String>,
) -> serde_json::Value {
    let mut off = |asked: &str, used: String| {
        notes.push(format!(
            "`{widget}` setting `{}`: {asked} is outside what it accepts, so {used} was used",
            prop.name
        ));
    };

    match (&prop.kind, given) {
        (PropKind::Flag { default }, None) => (*default).into(),
        (PropKind::Flag { default }, Some(value)) => value.as_bool().unwrap_or(*default).into(),

        (PropKind::Count { default, .. }, None) => (*default).into(),
        (
            PropKind::Count {
                default,
                least,
                most,
            },
            Some(value),
        ) => match value.as_i64() {
            Some(number) => {
                let held = number.clamp(*least, *most);
                if held != number {
                    off(&number.to_string(), held.to_string());
                }
                held.into()
            }
            None => {
                off(&value.to_string(), default.to_string());
                (*default).into()
            }
        },

        (PropKind::Amount { default, .. }, None) => (*default).into(),
        (
            PropKind::Amount {
                default,
                least,
                most,
            },
            Some(value),
        ) => match value.as_f64() {
            Some(number) if number.is_finite() => {
                let held = number.clamp(*least, *most);
                if (held - number).abs() > f64::EPSILON {
                    off(&number.to_string(), held.to_string());
                }
                held.into()
            }
            _ => {
                off(&value.to_string(), default.to_string());
                (*default).into()
            }
        },

        (PropKind::Choice { default, .. }, None) => (*default).into(),
        (PropKind::Choice { default, options }, Some(value)) => match value.as_str() {
            Some(word) if options.contains(&word) => word.into(),
            _ => {
                off(&value.to_string(), (*default).to_string());
                (*default).into()
            }
        },
    }
}

/// Read the flat [`Layout`] as a tree.
///
/// The upconversion ADR-0008 asks for. Nobody's file breaks: an existing layout
/// and the `layout.txt` choice beside it become a tree on load, and the flat
/// form can be dropped a release later once nothing is writing it.
#[must_use]
pub fn from_layout(layout: &Layout) -> Tree {
    let layout = layout.clone().sane();

    // The order is the order the deck draws in, and that is not incidental.
    //
    // The first version of this listed the zones in the order the flat
    // `Layout` happens to declare its fields, which put the transport above
    // the pads and the EQ below the effects. Nothing noticed while the tree
    // was only inspected; the moment the deck renders *from* it, that ordering
    // is what a DJ sees -- and rearranging a deck under someone as a side
    // effect of a format migration is exactly the thing this redesign is not
    // allowed to do. An upconversion has one job: produce the interface that
    // already exists.
    let mut inside =
        vec![Placement::of("deck.waveform").with("height", i64::from(layout.waveform_height))];
    if layout.overview {
        inside.push(Placement::of("deck.overview"));
    }
    inside.push(Placement::of("deck.progress"));
    inside.push(Placement::of("deck.stems"));
    inside.push(Placement::of("deck.times"));
    // §74's rail, above the pads: both are contextual blocks of controls, and
    // the rail is the smaller and the more urgent of the two. Unconditional
    // rather than behind a flag in the flat format, because the flat format
    // predates it and a layout saved before this existed should still get it --
    // a DJ who has never heard of the rail is exactly who it is for.
    inside.push(Placement::of("deck.rail"));
    if layout.pads {
        inside.push(Placement::of("deck.pads"));
    }
    if layout.beat_jump {
        inside.push(Placement::of("deck.beat_jump"));
    }
    if layout.loops {
        inside.push(Placement::of("deck.loops"));
    }
    if layout.fx {
        inside.push(Placement::of("deck.fx"));
    }
    inside.push(Placement::of("deck.grid"));
    inside.push(Placement::of("deck.transport"));
    // Slip, reverse and censor travel with the loops: slip is what makes a
    // loop something you can leave. The flat form never had a flag of their
    // own and took the loop one, and that is preserved rather than corrected,
    // because correcting it here would change what an existing layout draws.
    if layout.loops {
        inside.push(Placement::of("deck.perform"));
    }
    inside.push(Placement::of("deck.jog"));
    if layout.eq {
        inside.push(Placement::of("deck.eq"));
    }
    if layout.filter {
        inside.push(Placement::of("deck.filter"));
    }
    inside.push(Placement::of("deck.volume"));
    inside.push(Placement::of("deck.pitch"));
    if layout.keylock {
        inside.push(Placement::of("deck.keylock"));
    }
    inside.push(Placement::of("deck.cue"));
    inside.push(Placement::of("deck.xfader"));
    inside.push(Placement::of("deck.meter"));

    let stage = (1..=i64::from(layout.decks))
        .map(|number| {
            Placement::of("deck")
                .with("number", number)
                .holding("deck", inside.clone())
        })
        .collect();

    let mut slots = BTreeMap::new();
    slots.insert("stage".to_owned(), stage);
    slots.insert(
        "mixer".to_owned(),
        vec![
            Placement::of("mixer.crossfader"),
            Placement::of("mixer.cue"),
            Placement::of("mixer.master"),
        ],
    );
    if layout.browser {
        slots.insert(
            "browser".to_owned(),
            vec![
                Placement::of("browser.search"),
                Placement::of("browser.crates"),
                Placement::of("browser.tracks"),
                Placement::of("browser.sideview"),
            ],
        );
    }

    let mut tokens = BTreeMap::new();
    // The one styling value the flat form carried.
    if (layout.density - 1.0).abs() > f32::EPSILON {
        tokens.insert("density".to_owned(), layout.density.to_string());
    }

    Tree {
        name: layout.name,
        about: layout.description,
        tokens,
        slots,
    }
}

/// Read the tree-format layouts out of a directory.
///
/// The same directory the flat layouts live in, and told apart by content
/// rather than by extension or a version field: a file with a `slots` object is
/// a tree, and anything else is left for [`crate::layout::load_dir`]. Sniffing
/// is the right call exactly once -- at the boundary between two formats where
/// one is being retired -- and it means a DJ does not have to learn which
/// suffix to use during the release where both work.
///
/// A file that is neither is skipped with a warning, never fatal.
#[must_use]
pub fn load_dir(dir: &std::path::Path) -> Vec<Tree> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str::<Tree>(&text) {
            // A flat layout also parses as a `Tree` -- every field has a
            // default -- so the empty `slots` is what separates them, and it is
            // also the only thing that makes a tree worth having.
            Ok(tree) if !tree.slots.is_empty() => out.push(tree),
            Ok(_) => {}
            Err(error) => tracing::warn!(?path, %error, "skipping a malformed layout"),
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A flat summary of a tree, for the picker that still speaks the old shape.
///
/// **Lossy on purpose.** The picker needs a name, a line of description and a
/// deck count; it does not need the tree, and pretending a tree fits in the
/// flat struct is the mistake ADR-0008 exists to undo. This goes away when the
/// picker becomes tree-native; until then it is one function in one place
/// rather than a conversion scattered through the interface.
#[must_use]
pub fn as_layout(tree: &Tree) -> Layout {
    let decks = tree
        .slots
        .get("stage")
        .map_or(0, |placed| {
            placed.iter().filter(|p| p.widget == "deck").count()
        })
        .try_into()
        .unwrap_or(u8::MAX);

    Layout {
        name: tree.name.clone(),
        description: tree.about.clone(),
        decks,
        ..Layout::default()
    }
    .sane()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- the vocabulary itself ---------------------------------------------

    /// Names are a compatibility surface, so a duplicate is a file that means
    /// two things depending on which entry is found first.
    #[test]
    fn every_widget_has_its_own_name() {
        let mut seen = std::collections::BTreeSet::new();
        for widget in catalog() {
            assert!(
                seen.insert(widget.name),
                "two widgets are called `{}`",
                widget.name
            );
        }
    }

    /// A widget nothing can place is a widget that cannot appear, which is a
    /// typo rather than a decision -- retiring one is done by emptying `slots`
    /// deliberately, and no shipped widget does that yet.
    #[test]
    fn every_widget_names_slots_that_exist() {
        for widget in catalog() {
            assert!(
                !widget.slots.is_empty(),
                "`{}` can be placed nowhere",
                widget.name
            );
            for slot in widget.slots {
                let offered_by_a_parent = catalog().iter().any(|other| other.offers.contains(slot));
                assert!(
                    SLOTS.contains(slot) || offered_by_a_parent,
                    "`{}` wants slot `{slot}`, which nothing offers",
                    widget.name
                );
            }
        }
    }

    #[test]
    fn every_prop_has_a_default_inside_its_own_range() {
        for widget in catalog() {
            for prop in widget.props {
                match &prop.kind {
                    PropKind::Flag { .. } => {}
                    PropKind::Count {
                        default,
                        least,
                        most,
                    } => assert!(
                        least <= default && default <= most,
                        "`{}` setting `{}` defaults outside its own range",
                        widget.name,
                        prop.name
                    ),
                    PropKind::Amount {
                        default,
                        least,
                        most,
                    } => assert!(
                        least <= default && default <= most,
                        "`{}` setting `{}` defaults outside its own range",
                        widget.name,
                        prop.name
                    ),
                    PropKind::Choice { default, options } => assert!(
                        options.contains(default),
                        "`{}` setting `{}` defaults to something it does not offer",
                        widget.name,
                        prop.name
                    ),
                }
            }
        }
    }

    // -- the restyling boundary --------------------------------------------

    /// **The security test.**
    ///
    /// A layout is a thing one DJ sends another, so the question is not whether
    /// these particular spellings are refused -- it is whether the check is a
    /// whitelist. Each of these is a different way of reaching outside a
    /// stylesheet, and they are all refused by the same three shapes rather
    /// than by three separate guards.
    #[test]
    fn a_token_value_that_is_not_a_colour_or_a_length_is_refused() {
        for attempt in [
            "url(http://example.invalid/pixel.png)",
            "url('data:text/html,<script>')",
            "#22d3aa; background-image: url(http://example.invalid)",
            "@import url(http://example.invalid)",
            "red",                   // a keyword is not a shape we accept
            "var(--something-else)", // indirection
            "#22d3aa/*comment*/",
            "expression(alert(1))",
            "\\75 rl(http://example.invalid)", // CSS escape for `url(`
            "#22d3aa}body{display:none",
            "",
            // The ones that are the *right length* and the wrong alphabet.
            // Without the hex check these pass the length test and carry a
            // payload: `0;url(xy` is exactly eight characters after the hash.
            "#0;url(xy",
            "#zzz",
            "#22d3aZ",
            "#a\"b",
            "#</styl",
        ] {
            assert!(
                token("accent", attempt).is_none(),
                "`{attempt}` was accepted as a colour"
            );
        }

        for attempt in ["4px; position: fixed", "4", "4vw", "100%", "-4px_"] {
            assert!(
                token("radius", attempt).is_none(),
                "`{attempt}` was accepted as a length"
            );
        }
    }

    #[test]
    fn a_token_value_that_is_the_right_shape_is_kept() {
        assert_eq!(token("accent", "#22D3AA").as_deref(), Some("#22d3aa"));
        assert_eq!(token("accent", " #abc ").as_deref(), Some("#abc"));
        assert_eq!(token("accent", "#22d3aa80").as_deref(), Some("#22d3aa80"));
        assert_eq!(token("radius", "4px").as_deref(), Some("4px"));
        assert_eq!(token("radius", "0.5rem").as_deref(), Some("0.5rem"));
    }

    /// A length is clamped rather than refused, because an absurd radius is a
    /// mistake and a mistake should still open.
    #[test]
    fn an_absurd_length_is_brought_back_rather_than_dropped() {
        assert_eq!(token("radius", "9000px").as_deref(), Some("64px"));
        assert_eq!(token("density", "40").as_deref(), Some("1.4"));
        assert_eq!(token("density", "0").as_deref(), Some("0.8"));
    }

    /// The audio-driven properties are not a skin's to set: a layout that could
    /// pin `--audio-energy` would be a layout lying about the mix.
    #[test]
    fn the_audio_driven_properties_are_not_tokens() {
        for name in ["audio-energy", "audio-hue", "stem-color", "audio-bass"] {
            assert!(token(name, "#22d3aa").is_none(), "`{name}` was settable");
        }
    }

    // -- resolution --------------------------------------------------------

    fn tree_with(slot: &str, placements: Vec<Placement>) -> Tree {
        let mut slots = BTreeMap::new();
        slots.insert(slot.to_owned(), placements);
        Tree {
            name: "Test".to_owned(),
            slots,
            ..Tree::default()
        }
    }

    #[test]
    fn an_unknown_widget_is_skipped_with_a_note_rather_than_refusing_the_layout() {
        let tree = tree_with(
            "mixer",
            vec![
                Placement::of("mixer.crossfader"),
                Placement::of("mixer.hologram"),
                Placement::of("mixer.master"),
            ],
        );
        let out = resolve(&tree);

        let names: Vec<_> = out.slots["mixer"].iter().map(|p| &p.widget).collect();
        assert_eq!(names, ["mixer.crossfader", "mixer.master"]);
        assert_eq!(out.notes.len(), 1);
        assert!(out.notes[0].contains("mixer.hologram"));
    }

    #[test]
    fn a_widget_in_a_slot_that_does_not_take_it_is_skipped() {
        let out = resolve(&tree_with("mixer", vec![Placement::of("deck.waveform")]));
        assert!(out.slots.is_empty());
        assert!(out.notes[0].contains("cannot go in `mixer`"));
    }

    #[test]
    fn an_unknown_slot_is_skipped() {
        let out = resolve(&tree_with("ceiling", vec![Placement::of("mixer.master")]));
        assert!(out.slots.is_empty());
        assert!(out.notes[0].contains("no slot called `ceiling`"));
    }

    #[test]
    fn a_missing_prop_takes_its_default() {
        let out = resolve(&tree_with("deck", vec![Placement::of("deck.waveform")]));
        assert_eq!(out.slots["deck"][0].props["height"], 96);
    }

    #[test]
    fn a_prop_outside_its_range_is_clamped_and_said_so() {
        let out = resolve(&tree_with(
            "deck",
            vec![Placement::of("deck.waveform").with("height", 9000)],
        ));
        assert_eq!(out.slots["deck"][0].props["height"], 320);
        assert!(out.notes[0].contains("320"));
    }

    #[test]
    fn a_prop_of_the_wrong_type_falls_back_to_its_default() {
        let out = resolve(&tree_with(
            "deck",
            vec![Placement::of("deck.waveform").with("height", "tall")],
        ));
        assert_eq!(out.slots["deck"][0].props["height"], 96);
        assert!(!out.notes.is_empty());
    }

    #[test]
    fn a_choice_that_is_not_offered_falls_back() {
        let out = resolve(&tree_with(
            "deck",
            vec![Placement::of("deck.pads").with("page", "hologram")],
        ));
        assert_eq!(out.slots["deck"][0].props["page"], "cues");
    }

    #[test]
    fn children_go_in_a_slot_their_parent_offers() {
        let out = resolve(&tree_with(
            "stage",
            vec![
                Placement::of("deck")
                    .with("number", 2)
                    .holding("deck", vec![Placement::of("deck.eq")])
                    .holding("mixer", vec![Placement::of("mixer.master")]),
            ],
        ));
        let deck = &out.slots["stage"][0];
        assert_eq!(deck.props["number"], 2);
        assert_eq!(deck.children["deck"][0].widget, "deck.eq");
        assert!(!deck.children.contains_key("mixer"));
        assert!(out.notes.iter().any(|note| note.contains("does not offer")));
    }

    /// **Why [`DEEPEST`] is a backstop rather than the guard that does the
    /// work.**
    ///
    /// Writing a deeply nested file does not reach the depth check at all: a
    /// `deck` inside a `deck` is refused by the *slot* rule first, because
    /// `deck` may only be placed in `stage`. The recursion is bounded by the
    /// catalog's shape, not by counting.
    ///
    /// So the thing worth testing is that shape. Containment here is a graph --
    /// `A` can hold `B` when `A` offers a slot `B` may sit in -- and what makes
    /// the recursion safe is that the graph is acyclic and short. A future
    /// widget that offered a slot it could itself occupy would make it neither,
    /// and this test is what would notice. [`DEEPEST`] stays as the backstop for
    /// the case where somebody adds that widget and only reads the failure
    /// afterwards.
    #[test]
    fn no_widget_can_contain_itself_however_deeply() {
        fn holds(parent: &Widget) -> Vec<&'static Widget> {
            catalog()
                .iter()
                .filter(|child| child.slots.iter().any(|slot| parent.offers.contains(slot)))
                .collect()
        }

        // Longest containment chain from each widget, refusing to walk a cycle.
        fn depth(widget: &Widget, seen: &mut Vec<&'static str>) -> usize {
            assert!(
                !seen.contains(&widget.name),
                "`{}` can contain itself, through {seen:?} -- the layout tree is \
                 no longer bounded by the catalog's shape",
                widget.name
            );
            seen.push(widget.name);
            let deepest = holds(widget)
                .into_iter()
                .map(|child| depth(child, seen))
                .max()
                .unwrap_or(0);
            seen.pop();
            deepest + 1
        }

        for widget in catalog() {
            let reach = depth(widget, &mut Vec::new());
            assert!(
                reach <= DEEPEST,
                "`{}` can nest {reach} deep, past the {DEEPEST} the resolver walks",
                widget.name
            );
        }
    }

    /// The backstop itself, exercised where it can be: a slot that is offered
    /// and a chain long enough to trip it would be caught here the moment the
    /// catalog grew one.
    #[test]
    fn the_depth_backstop_is_at_least_deep_enough_for_the_catalog() {
        // `stage > deck > deck.waveform` is the deepest thing that ships.
        let out = resolve(&tree_with(
            "stage",
            vec![Placement::of("deck").holding("deck", vec![Placement::of("deck.waveform")])],
        ));
        assert!(out.notes.is_empty(), "{:?}", out.notes);
        assert_eq!(
            out.slots["stage"][0].children["deck"][0].widget,
            "deck.waveform"
        );
    }

    #[test]
    fn a_layout_with_no_name_gets_one() {
        let out = resolve(&Tree {
            name: "   ".to_owned(),
            ..Tree::default()
        });
        assert_eq!(out.name, "Custom");
    }

    // -- upconversion ------------------------------------------------------

    #[test]
    fn the_flat_layout_becomes_a_tree_that_resolves_cleanly() {
        for layout in crate::layout::builtin() {
            let name = layout.name.clone();
            let decks = usize::from(layout.clone().sane().decks);
            let out = resolve(&from_layout(&layout));

            assert!(
                out.notes.is_empty(),
                "`{name}` upconverted to a tree with problems: {:?}",
                out.notes
            );
            assert_eq!(out.slots["stage"].len(), decks, "`{name}` deck count");
            assert_eq!(out.name, name);
        }
    }

    /// The flags the flat form had are the presence of a widget in the tree,
    /// which is the whole point of the change.
    #[test]
    fn a_flag_that_was_off_leaves_its_widget_out() {
        let bare = Layout {
            pads: false,
            eq: false,
            fx: false,
            ..Layout::default()
        };
        let out = resolve(&from_layout(&bare));
        let inside: Vec<_> = out.slots["stage"][0].children["deck"]
            .iter()
            .map(|placed| placed.widget.as_str())
            .collect();

        assert!(!inside.contains(&"deck.pads"));
        assert!(!inside.contains(&"deck.eq"));
        assert!(!inside.contains(&"deck.fx"));
        assert!(inside.contains(&"deck.waveform"));
    }

    /// The upconversion produces the deck djmanzo already draws, in order.
    ///
    /// Once `Deck.svelte` renders *from* this list, the list **is** the deck:
    /// a widget missing here is a control that disappears, and two swapped
    /// here are two controls that trade places under a DJ mid-set. The first
    /// version of `from_layout` had both problems -- no stems, no grid, no
    /// jog, no channel fader, no cue, no crossfader assignment and no meter,
    /// and the transport above the pads -- and nothing caught it, because
    /// nothing was rendering from it yet.
    ///
    /// Written out in full rather than checked for a few members. A golden
    /// order is the only form of this test that fails when something moves,
    /// and moving is the failure that matters.
    #[test]
    fn the_default_layout_upconverts_to_the_deck_the_interface_draws() {
        let out = resolve(&from_layout(&Layout::default()));
        let inside: Vec<_> = out.slots["stage"][0].children["deck"]
            .iter()
            .map(|placed| placed.widget.as_str())
            .collect();

        assert_eq!(
            inside,
            [
                "deck.waveform",
                "deck.overview",
                "deck.progress",
                "deck.stems",
                "deck.times",
                "deck.rail",
                "deck.pads",
                "deck.beat_jump",
                "deck.loops",
                "deck.fx",
                "deck.grid",
                "deck.transport",
                "deck.perform",
                "deck.jog",
                "deck.eq",
                "deck.filter",
                "deck.volume",
                "deck.pitch",
                "deck.keylock",
                "deck.cue",
                "deck.xfader",
                "deck.meter",
            ],
            "the order a deck is drawn in has changed. If that was deliberate, \
             change `Deck.svelte` to match and say so in the commit; if it was \
             not, this is a control moving underneath a DJ."
        );
    }

    /// The first shipped preset is not what "nothing chosen" means.
    ///
    /// `layout_tree` used to answer with `builtin().first()` when the DJ had
    /// never picked a layout, and that preset is "Starter" -- no pads, no
    /// loops, no effect rack, no beat jump, no filter, no keylock. It went
    /// unnoticed for as long as the interface read only the tokens out of that
    /// answer and drew the deck from its own markup. The moment the deck
    /// rendered from the tree, six controls vanished from a screenshot.
    ///
    /// The assertion is not that Starter is wrong -- it is a good preset, and
    /// it is *supposed* to be a reduction. It is that a reduction is a choice,
    /// and a command answering a question nobody asked must not make it.
    #[test]
    fn the_first_shipped_preset_is_a_reduction_and_so_cannot_be_the_default_answer() {
        let starter = crate::layout::builtin()
            .into_iter()
            .next()
            .expect("djmanzo ships presets");
        let unconfigured = resolve(&from_layout(&Layout::default()));
        let chosen = resolve(&from_layout(&starter));

        let names = |out: &Resolved| -> Vec<String> {
            out.slots["stage"][0].children["deck"]
                .iter()
                .map(|placed| placed.widget.clone())
                .collect()
        };
        let full = names(&unconfigured);
        let reduced = names(&chosen);

        for widget in ["deck.pads", "deck.loops", "deck.fx", "deck.beat_jump"] {
            assert!(
                full.contains(&widget.to_owned()),
                "an unconfigured djmanzo must draw `{widget}`"
            );
            assert!(
                !reduced.contains(&widget.to_owned()),
                "`{}` was expected to leave `{widget}` out -- if it no longer does, \
                 this test is asserting the wrong preset rather than passing",
                starter.name
            );
        }
    }

    #[test]
    fn the_waveform_height_survives_the_upconversion() {
        let tall = Layout {
            waveform_height: 200,
            ..Layout::default()
        };
        let out = resolve(&from_layout(&tall));
        assert_eq!(
            out.slots["stage"][0].children["deck"][0].props["height"],
            200
        );
    }

    /// Density was the one styling value the flat form carried, and it is a
    /// token now rather than a field.
    #[test]
    fn density_becomes_a_token() {
        let dense = Layout {
            density: 1.2,
            ..Layout::default()
        };
        let out = resolve(&from_layout(&dense));
        assert_eq!(out.tokens.get("density").map(String::as_str), Some("1.2"));

        let plain = resolve(&from_layout(&Layout::default()));
        assert!(!plain.tokens.contains_key("density"));
    }

    // -- reading a directory of files --------------------------------------

    fn write(dir: &std::path::Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    /// The two formats share a directory during the release where both work,
    /// and they are told apart by whether there are slots in the file.
    #[test]
    fn a_tree_file_is_read_and_a_flat_one_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "booth.json",
            r#"{ "name": "Booth", "about": "Two decks, nothing else.",
                 "slots": { "stage": [ { "widget": "deck", "props": { "number": 1 } } ] } }"#,
        );
        // A flat layout parses as a `Tree` too -- every field has a default --
        // so this is the case the emptiness check exists for.
        write(
            dir.path(),
            "old.json",
            r#"{ "name": "Old", "decks": 4, "pads": false }"#,
        );
        write(dir.path(), "notes.txt", "not a layout");
        write(dir.path(), "broken.json", "{ this is not json");

        let trees = load_dir(dir.path());
        assert_eq!(trees.len(), 1, "{trees:?}");
        assert_eq!(trees[0].name, "Booth");

        // And the flat one is still readable by the reader that owns it.
        let flat = crate::layout::load_dir(dir.path());
        assert!(flat.iter().any(|layout| layout.name == "Old"));
    }

    #[test]
    fn a_directory_that_is_not_there_is_no_layouts_rather_than_a_panic() {
        assert!(load_dir(std::path::Path::new("/no/such/place")).is_empty());
    }

    /// The picker still speaks the flat shape, so a tree has to summarise into
    /// it -- and the deck count is the one field that must not be invented.
    #[test]
    fn a_tree_summarises_into_the_shape_the_picker_reads() {
        let tree = from_layout(&Layout {
            name: "Four".to_owned(),
            decks: 4,
            ..Layout::default()
        });
        let summary = as_layout(&tree);
        assert_eq!(summary.name, "Four");
        assert_eq!(summary.decks, 4);

        // A tree with no stage at all still has to produce something drawable.
        let empty = as_layout(&Tree::default());
        assert_eq!(empty.decks, 2);
    }

    /// The whole point: a file can name something this djmanzo has never heard
    /// of, and still open.
    #[test]
    fn a_file_from_a_newer_djmanzo_opens_without_the_parts_it_cannot_draw() {
        let tree: Tree = serde_json::from_str(
            r##"{
                 "name": "From the future",
                 "tokens": { "accent": "#ff00aa", "hologram": "#000" },
                 "slots": {
                   "mixer": [
                     { "widget": "mixer.crossfader" },
                     { "widget": "mixer.holodeck" }
                   ]
                 }
               }"##,
        )
        .unwrap();
        let out = resolve(&tree);

        assert_eq!(out.slots["mixer"].len(), 1);
        assert_eq!(out.tokens["accent"], "#ff00aa");
        assert!(!out.tokens.contains_key("hologram"));
        assert_eq!(out.notes.len(), 2, "{:?}", out.notes);
    }

    // -- the file format ---------------------------------------------------

    #[test]
    fn a_layout_file_round_trips() {
        let tree = from_layout(&crate::layout::builtin()[0]);
        let text = serde_json::to_string(&tree).unwrap();
        let back: Tree = serde_json::from_str(&text).unwrap();
        assert_eq!(back, tree);
    }

    /// A file naming only what it changes is the point of the format.
    #[test]
    fn a_file_may_name_only_what_it_changes() {
        let tree: Tree = serde_json::from_str(
            r#"{ "name": "Booth", "slots": { "mixer": [ { "widget": "mixer.crossfader" } ] } }"#,
        )
        .unwrap();
        let out = resolve(&tree);
        assert_eq!(out.name, "Booth");
        assert_eq!(out.slots["mixer"][0].widget, "mixer.crossfader");
        assert!(out.notes.is_empty());
    }
}

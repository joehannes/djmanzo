//! What the waveform draws, named — the inventory [§25 of the
//! directive](../../../docs/DIRECTIVE.md) asks for.
//!
//! §25 lists twenty semantic layers and says the point of them: the waveform
//! "should become **instrumentation**", answering *what is happening, what is
//! about to happen, what could I do, what will happen if I do it*. It also says
//! the thing that shapes this module — **do not abandon the Rust renderer** —
//! so this is an architecture over both halves rather than a plan to move
//! everything into the webview.
//!
//! # Why the list is here rather than in a document
//!
//! A list of twenty layers in a markdown file is a list that quietly stops
//! matching the code. Here it is checked: every layer names where it is drawn
//! and whether it exists yet, the interface is handed the same table it draws
//! from, and a browser test asserts that everything actually on screen is in
//! it. "Five of twenty" is then a fact rather than somebody's recollection.
//!
//! # §57, as a constraint rather than a warning
//!
//! [§57](../../../docs/DIRECTIVE.md) says colour must be semantic and, more
//! sharply, that the same colour must never carry two meanings. That is a
//! property of a *set* of layers, so it cannot be enforced one layer at a time
//! — which is exactly why they need to be in one table. Every drawn layer
//! declares its [`Role`], and a test refuses two drawn layers with the same
//! one. Layers that are not drawn yet declare `Role::Unassigned`, which is
//! honest and is excluded from the check: reserving a colour for something
//! nobody has built is how a palette runs out for no reason.
//!
//! # Where "uncertainty" is drawn, and where it is not
//!
//! The rasteriser has always faded the beat grid by its own confidence — that
//! is [`Beatgrid`](dj_core::Beatgrid) being drawn at the weight it deserves,
//! and it belongs to the `beats` layer rather than being a layer of its own.
//! What it cannot do is be *read*: at overview zoom the grid is suppressed
//! entirely for density, so a faint grid and an absent one look identical, and
//! neither says whether the analyser was guessing. The `confidence` layer is
//! the part that says so, which is why it is an overlay while the fade it
//! talks about is in the tile.

use serde::Serialize;

/// What a layer's colour means.
///
/// Deliberately about *meaning* rather than about hue: "the record's own
/// sound", "where the grid is", "something a human placed". The stylesheet and
/// the rasteriser choose the colours; this says what two of them may not both
/// mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The record's own sound: amplitude and where its energy sits.
    Sound,
    /// The pulse — beats, bars, phrases. Where the music is counted from.
    Grid,
    /// Somewhere a human put a mark and can return to.
    Placed,
    /// A stretch of the record that repeats.
    Looping,
    /// A mix: where it starts, where it ends, what it covers.
    Seam,
    /// How much of the record is left.
    Runway,
    /// Somewhere djmanzo suggests, as distinct from somewhere it is so.
    ///
    /// The distinction §57 is really about. "A mix could go here" and "the mix
    /// goes here" drawn in one colour is an interface that cannot be trusted
    /// at a glance, which is the only way a waveform is ever read.
    Proposed,
    /// What is an estimate rather than a measurement.
    ///
    /// Shared, eventually and deliberately: vocal and stem presence will each
    /// arrive with a confidence of their own, and they are the same kind of
    /// claim about a different thing.
    Uncertain,
    /// Nothing yet. The layer is named but not drawn.
    Unassigned,
}

impl Role {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Role::Sound => "sound",
            Role::Grid => "grid",
            Role::Placed => "placed",
            Role::Looping => "looping",
            Role::Seam => "seam",
            Role::Runway => "runway",
            Role::Proposed => "proposed",
            Role::Uncertain => "uncertain",
            Role::Unassigned => "unassigned",
        }
    }
}

/// Which half of the renderer draws a layer.
///
/// Both are real and neither is a fallback. Anything that has to be computed
/// per audio bucket belongs in the tile — that is what the Rust rasteriser is
/// for and why ADR-0004 exists. Anything that is a handful of positions is an
/// element over the top, where it can take a pointer and be dragged (§26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Drawn {
    /// Rasterised into the tile, in Rust.
    Tile,
    /// An element over the tiles, in the interface.
    Overlay,
    /// Not drawn anywhere yet.
    Nowhere,
}

/// One of §25's twenty layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Layer {
    /// The stable slug. Appears in the interface as `data-layer`, so a browser
    /// test can check what is on screen against this table.
    pub name: &'static str,
    /// What a DJ would call it.
    pub title: &'static str,
    /// What it encodes, in one line.
    pub about: &'static str,
    pub role: Role,
    pub drawn: Drawn,
}

impl Layer {
    /// Whether anything draws this yet.
    #[must_use]
    pub const fn exists(&self) -> bool {
        !matches!(self.drawn, Drawn::Nowhere)
    }
}

/// §25's twenty, in its order, with what each one currently is.
///
/// The order is the directive's, not a z-order: what sits on top of what is a
/// question for whichever half draws it, and answering it here would be this
/// table deciding something it cannot see.
#[must_use]
pub fn layers() -> &'static [Layer] {
    &LAYERS
}

/// Look one up by its slug.
#[must_use]
pub fn layer(name: &str) -> Option<&'static Layer> {
    LAYERS.iter().find(|layer| layer.name == name)
}

static LAYERS: [Layer; 20] = [
    Layer {
        name: "amplitude",
        title: "Amplitude",
        about: "How loud the record is, moment to moment.",
        role: Role::Sound,
        drawn: Drawn::Tile,
    },
    Layer {
        name: "spectral",
        title: "Spectral balance",
        about: "Which part of the spectrum each moment belongs to.",
        role: Role::Sound,
        drawn: Drawn::Tile,
    },
    Layer {
        name: "beats",
        title: "Beat grid",
        about: "Every beat, fainter where the grid is a guess.",
        role: Role::Grid,
        drawn: Drawn::Tile,
    },
    Layer {
        name: "phrases",
        title: "Phrase structure",
        about: "The 16 or 32 beat groups the music is actually built from.",
        role: Role::Grid,
        drawn: Drawn::Tile,
    },
    Layer {
        name: "downbeats",
        title: "Downbeats",
        about: "Every fourth beat, so bars can be counted without counting beats.",
        role: Role::Grid,
        drawn: Drawn::Tile,
    },
    Layer {
        name: "cues",
        title: "Cue markers",
        about: "Where you put a hot cue.",
        role: Role::Placed,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "loop",
        title: "Loop region",
        about: "The stretch repeating right now.",
        role: Role::Looping,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "saved-loops",
        title: "Saved loops",
        about: "Loops kept for later, drawn where they would fire.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "vocal",
        title: "Vocal presence",
        about: "Where somebody is singing.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "stems",
        title: "Stem presence",
        about: "Which of the four currents is carrying the record here.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "seam",
        title: "Transition",
        about: "Where the mix starts and ends, and what it covers.",
        role: Role::Seam,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "mix-out",
        title: "Likely mix-out",
        about: "The stretch in which a mix out of this record can begin.",
        role: Role::Proposed,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "mix-in",
        title: "Likely mix-in",
        about: "Where a record could be brought in.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "breakdowns",
        title: "Breakdowns",
        about: "Where the record thins out.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "drops",
        title: "Drops",
        about: "Where it comes back.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "energy",
        title: "Energy trajectory",
        about: "Where the record is going, over its whole length.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "suggestion",
        title: "AI recommendation",
        about: "What djmanzo would do, drawn as a ghost rather than as a fact.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "crowd",
        title: "Crowd response",
        about: "What the room did last time this played.",
        role: Role::Unassigned,
        drawn: Drawn::Nowhere,
    },
    Layer {
        name: "confidence",
        title: "Uncertainty",
        about: "That the beat grid under all of this is an estimate.",
        role: Role::Uncertain,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "runway",
        title: "Runway",
        about: "How much record is left before it ends.",
        role: Role::Runway,
        drawn: Drawn::Overlay,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// §25 names twenty. If the directive is ever read again and this is
    /// nineteen, something was dropped rather than decided.
    #[test]
    fn the_directive_lists_twenty_and_so_does_this() {
        assert_eq!(layers().len(), 20);
    }

    /// A slug is what the interface stamps on an element and what a test looks
    /// for, so two of them would make one layer invisible to the check.
    #[test]
    fn every_layer_has_its_own_name_and_says_what_it_is() {
        let names: BTreeSet<&str> = layers().iter().map(|layer| layer.name).collect();
        assert_eq!(names.len(), layers().len(), "two layers share a name");
        for entry in layers() {
            assert!(!entry.title.is_empty(), "{} has no title", entry.name);
            assert!(
                entry.about.len() > 12,
                "{} has a stub description",
                entry.name
            );
            assert_eq!(layer(entry.name), Some(entry));
        }
        assert_eq!(layer("nothing-like-this"), None);
    }

    /// **§57: never overload the same colour with two meanings.**
    ///
    /// A property of the whole set, which is why the set has to exist. Two
    /// layers sharing a role would be two things a DJ has to tell apart by
    /// position alone — and the waveform is the one surface where everything
    /// is at a position that means something else already.
    ///
    /// Grouped roles are allowed and are the point: the three grid layers are
    /// one meaning drawn at three weights, which is texture rather than a
    /// second colour. What is refused is two *unrelated* layers on one role.
    #[test]
    fn no_two_unrelated_layers_share_a_colour_meaning() {
        for layer in layers().iter().filter(|l| l.exists()) {
            assert_ne!(
                layer.role,
                Role::Unassigned,
                "{} is drawn but claims no meaning for its colour",
                layer.name
            );
        }
        // The roles that are deliberately shared, and by exactly which layers.
        // Written out so that adding a layer to one of them is a decision
        // somebody makes here rather than something that happens.
        let mut grouped: std::collections::BTreeMap<&str, Vec<&str>> =
            std::collections::BTreeMap::new();
        for layer in layers().iter().filter(|l| l.exists()) {
            grouped
                .entry(layer.role.name())
                .or_default()
                .push(layer.name);
        }
        assert_eq!(
            grouped.get("sound").map(Vec::len),
            Some(2),
            "the record's own sound"
        );
        assert_eq!(grouped.get("grid").map(Vec::len), Some(3), "the pulse");
        for role in [
            "placed",
            "looping",
            "seam",
            "runway",
            "proposed",
            "uncertain",
        ] {
            assert_eq!(
                grouped.get(role).map(Vec::len),
                Some(1),
                "{role} is carrying more than one meaning"
            );
        }
    }

    /// Nothing that is not drawn reserves a colour, and nothing drawn lacks a
    /// place to be drawn.
    #[test]
    fn an_unbuilt_layer_reserves_nothing() {
        for layer in layers() {
            match layer.drawn {
                Drawn::Nowhere => assert_eq!(
                    layer.role,
                    Role::Unassigned,
                    "{} reserves a colour for something nobody has built",
                    layer.name
                ),
                _ => assert!(layer.exists()),
            }
        }
    }

    /// The count worth quoting, so "five of twenty" cannot drift.
    #[test]
    fn the_built_count_is_a_fact_rather_than_a_recollection() {
        let built: Vec<&str> = layers()
            .iter()
            .filter(|l| l.exists())
            .map(|l| l.name)
            .collect();
        assert_eq!(
            built,
            vec![
                "amplitude",
                "spectral",
                "beats",
                "phrases",
                "downbeats",
                "cues",
                "loop",
                "seam",
                "mix-out",
                "confidence",
                "runway",
            ],
            "the built layers changed; say so in the docs as well as here"
        );
    }
}

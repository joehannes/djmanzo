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
    ///
    /// Shared by the loop that is running and the loops kept for later, which
    /// is the tightest case for sharing in this table: they are one kind of
    /// thing in two states, and §30's rule is that a *state* is painted with
    /// the role it means rather than with a colour of its own. The armed loop
    /// is solid and the kept ones are quiet; a second hue for "the same span,
    /// not firing" would say they were different kinds of thing.
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
    /// Shared by the grid's own uncertainty and §25's `vocal`: how much to
    /// trust what is drawn beside it, and how much of this moment is a lead.
    /// Both are djmanzo saying *I think*.
    ///
    /// This doc used to predict that `stems` would join them -- *vocal and
    /// stem presence will each arrive with a confidence of their own* -- and
    /// building it showed the prediction was wrong. Vocal presence is one
    /// number and can be drawn as strength; stem presence is a question about
    /// *which of four*, and a drawing that cannot say which answers nothing.
    /// It has [`Role::Stems`] instead, and the four colours the interface
    /// already gives the stems.
    Uncertain,
    /// Which of the four currents is carrying the record.
    ///
    /// Its own role rather than a share of [`Role::Uncertain`] because it is
    /// not one colour: §30's four stem roles already have to be told apart
    /// from one another, a DJ already reads them on the stem controls, and
    /// naming the current on the waveform in the colour of the fader that
    /// mutes it is the association worth making.
    Stems,
    /// Where the record goes, and the two places on it worth marking.
    ///
    /// Shared by three layers on the same argument `Proposed` is shared by
    /// two: the trajectory, the breakdowns in it and the drops out of them are
    /// one reading drawn three ways, from one pass over one curve. A DJ
    /// glancing at an overview is asking "what shape is this record", and
    /// three colours for three parts of one answer would be §30's
    /// neon-everything failure rather than §57's distinction.
    Shape,
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
            Role::Stems => "stems",
            Role::Shape => "shape",
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

    /// Whether a DJ may turn this one off.
    ///
    /// §8 Level 1 asks djmanzo to remember a *preferred waveform display*, and
    /// this table is what that display is made of — so the picker is over these
    /// rows. Two of them are not preferences:
    ///
    /// - **Amplitude and spectral balance are the waveform itself.** A picker
    ///   offering to remove them is a picker offering an empty strip, which is
    ///   the same judgement §20's table makes about its title column: the box
    ///   is shown, disabled, and says why, rather than silently re-ticking
    ///   itself.
    /// - **A layer nobody has built cannot be turned off.** Offering it would
    ///   be a control over nothing, and a DJ who ticked it and saw no change
    ///   would learn the wrong thing about the ones that do work.
    #[must_use]
    pub const fn choosable(&self) -> bool {
        self.exists() && !matches!(self.role, Role::Sound)
    }
}

/// Turn what a DJ asked for into what the waveform will actually draw.
///
/// The same round trip §20's columns make, and for the same reasons: a
/// preferences file written by a later djmanzo may name a layer this build has
/// never heard of, and a DJ opening an older one should get the layers it
/// *does* have rather than a blank strip. Unknown names are dropped, repeats
/// collapse, and the two that are the waveform itself go back in whether they
/// were asked for or not.
///
/// An empty ask is **everything**, not nothing: somebody who has unticked every
/// box wants the instrumentation back, and a waveform that had to be rebuilt
/// layer by layer after one stray click would be a worse surface than one with
/// no picker at all.
#[must_use]
pub fn choosing(asked: &[String]) -> Vec<&'static Layer> {
    if asked.is_empty() {
        return LAYERS.iter().filter(|layer| layer.exists()).collect();
    }
    let mut chosen: Vec<&'static Layer> = LAYERS
        .iter()
        .filter(|layer| !layer.choosable() && layer.exists())
        .collect();
    for layer in asked
        .iter()
        .filter_map(|name| layer(name))
        .filter(|layer| layer.choosable())
    {
        if !chosen.iter().any(|held| held.name == layer.name) {
            chosen.push(layer);
        }
    }
    // Back into §25's own order, which is the order the picker offers them in
    // and the order this table is written in. A set is not a sequence and the
    // one a DJ stored is whatever order they happened to tick.
    chosen.sort_by_key(|layer| {
        LAYERS
            .iter()
            .position(|row| row.name == layer.name)
            .unwrap_or(usize::MAX)
    });
    chosen
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
        role: Role::Looping,
        drawn: Drawn::Overlay,
    },
    // An estimate about the music, and a franker one than most: the detector
    // measures a centred, sustained signal in the voice range, which is where
    // a singer sits and is also where a centred synth lead sits. `Uncertain`
    // is the colour for exactly that -- see `dj_analysis::voice`.
    Layer {
        name: "vocal",
        title: "Vocal presence",
        about: "Where a lead is centred in the voice range -- usually a singer.",
        role: Role::Uncertain,
        drawn: Drawn::Overlay,
    },
    // The same pass that answers `vocal` answers this, and answers it as a
    // partition: the four shares of a moment add to the whole of it, so this
    // is one reading of the record rather than four that could disagree --
    // see `dj_analysis::presence`.
    Layer {
        name: "stems",
        title: "Stem presence",
        about: "Which of the four currents is carrying the record here.",
        role: Role::Stems,
        drawn: Drawn::Overlay,
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
        role: Role::Proposed,
        drawn: Drawn::Overlay,
    },
    // An estimate about the music, which is what `Uncertain` is for: the
    // detector reads where the low band falls away and a record with a
    // half-time section is a record it can be wrong about.
    Layer {
        name: "breakdowns",
        title: "Breakdowns",
        about: "Where the record thins out.",
        role: Role::Shape,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "drops",
        title: "Drops",
        about: "Where it comes back.",
        role: Role::Shape,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "energy",
        title: "Energy trajectory",
        about: "Where the record is going, over its whole length.",
        role: Role::Shape,
        drawn: Drawn::Overlay,
    },
    Layer {
        name: "suggestion",
        title: "AI recommendation",
        about: "What djmanzo would do, drawn as a ghost rather than as a fact.",
        role: Role::Proposed,
        drawn: Drawn::Overlay,
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

    /// **The load-bearing one: a chosen set is a set the waveform can draw.**
    ///
    /// Three rules, and each is about a waveform that is still readable
    /// afterwards. A slug this build does not have is dropped rather than
    /// refused; the two layers that *are* the waveform go back whether they
    /// were asked for or not; and unticking everything gives the whole
    /// instrumentation back rather than an empty strip.
    #[test]
    fn choosing_gives_back_something_that_can_be_drawn() {
        let names = |chosen: Vec<&'static Layer>| -> Vec<&'static str> {
            chosen.into_iter().map(|layer| layer.name).collect()
        };

        // Nothing asked for is everything djmanzo has.
        let all = names(choosing(&[]));
        assert_eq!(
            all.len(),
            LAYERS.iter().filter(|l| l.exists()).count(),
            "an empty ask should be the whole instrumentation: {all:?}"
        );

        // One layer asked for still carries the record itself.
        let one = names(choosing(&["cues".to_owned()]));
        assert!(one.contains(&"amplitude"), "{one:?}");
        assert!(one.contains(&"spectral"), "{one:?}");
        assert!(one.contains(&"cues"), "{one:?}");
        assert!(!one.contains(&"beats"), "{one:?}");

        // A slug from another build, a repeat, and one nobody has drawn yet.
        let messy = names(choosing(&[
            "hologram".to_owned(),
            "cues".to_owned(),
            "cues".to_owned(),
            "crowd".to_owned(),
        ]));
        assert_eq!(messy, one, "unknown, repeated and unbuilt should all drop");

        // And §25's order, not the order they were ticked in.
        let backwards = names(choosing(&["runway".to_owned(), "beats".to_owned()]));
        assert_eq!(
            backwards
                .iter()
                .position(|n| *n == "beats")
                .expect("beats survived"),
            backwards
                .iter()
                .position(|n| *n == "runway")
                .map(|at| at - 1)
                .expect("runway survived"),
            "the picker offers §25's order and the chooser answers in it: {backwards:?}"
        );
    }

    /// **Only what exists, and not the record itself, can be turned off.**
    ///
    /// A control over a layer nobody has built is a control over nothing, and a
    /// DJ who ticked it and saw no change would learn the wrong thing about the
    /// ones that do work.
    #[test]
    fn the_choosable_layers_are_the_ones_choosing_is_about() {
        for layer in &LAYERS {
            if !layer.exists() {
                assert!(!layer.choosable(), "{} is not drawn anywhere", layer.name);
            }
            if layer.role == Role::Sound {
                assert!(
                    !layer.choosable(),
                    "{} is the waveform itself and cannot be a preference",
                    layer.name
                );
            }
        }
        assert!(
            LAYERS.iter().filter(|l| l.choosable()).count() >= 7,
            "the picker would be too short to be worth having"
        );
    }
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
        // `looping` carries the loop that is running and the ones kept for
        // later. One kind of thing in two states, and §30 says a state is
        // painted with the role it means: the armed one is solid, the kept
        // ones are quiet, and a second hue would say they were different kinds
        // of thing rather than the same thing not firing.
        assert_eq!(
            grouped.get("looping"),
            Some(&vec!["loop", "saved-loops"]),
            "the looping colour is for a stretch that repeats, and only that"
        );
        // `proposed` carries both mix windows and §27's ghost. All three are
        // djmanzo saying *could*, about the same mix, at three scales -- where
        // a record can be left, where one can be joined, and what happens if
        // this one comes in there. One colour for all three is the distinction
        // §57 asks for holding, not losing: what must never share it is
        // anything that is so.
        //
        // `mix-in` joining them is the clearest case there is rather than a
        // stretch. It is the same sentence as `mix-out` read from the other
        // end, and a DJ looking at two lanes is reading one question -- can
        // these two meet -- with an answer drawn on each. Two colours for the
        // two ends would say they were different kinds of claim.
        assert_eq!(
            grouped.get("proposed"),
            Some(&vec!["mix-out", "mix-in", "suggestion"]),
            "the proposed colour is for what djmanzo suggests, and only that"
        );
        // §75's three, and the same argument as `proposed` above: the
        // trajectory, the breakdowns in it and the drops out of them are one
        // reading of one curve drawn three ways. Three colours for three parts
        // of one answer to "what shape is this record" would be §30's
        // neon-everything failure rather than §57's distinction.
        assert_eq!(
            grouped.get("shape"),
            Some(&vec!["breakdowns", "drops", "energy"]),
            "the shape colour is for where the record goes, and only that"
        );
        // `uncertain` is the one the enum's own doc reserved for this: what is
        // an estimate rather than a measurement. The beat grid's uncertainty
        // and where a voice is are the same kind of claim about different
        // things -- djmanzo saying *I think* -- and a DJ reading either is
        // asking how much to trust what is drawn beside it. What must never
        // share it is anything djmanzo knows.
        assert_eq!(
            grouped.get("uncertain"),
            Some(&vec!["vocal", "confidence"]),
            "the uncertain colour is for what djmanzo is estimating, and only that"
        );
        for role in ["placed", "seam", "runway", "stems"] {
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
                "saved-loops",
                "vocal",
                "stems",
                "seam",
                "mix-out",
                "mix-in",
                "breakdowns",
                "drops",
                "energy",
                "suggestion",
                "confidence",
                "runway",
            ],
            "the built layers changed; say so in the docs as well as here"
        );
    }
}

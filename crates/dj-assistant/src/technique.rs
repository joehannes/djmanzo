//! What a DJ actually does with their hands, as data.
//!
//! The companion to [`dj_core::genre`]: that table says what two records are,
//! this one says what can be done with them. Together they answer the only
//! question that matters at the end of a record — *what now* — with something
//! more useful than a list of effects.
//!
//! # Why this is a table and not a manual
//!
//! Because it is consulted, not read. [`for_situation`] takes what is true
//! right now — how the two records blend, what the DJ has in front of them,
//! how far in the night is — and returns the handful of techniques that apply.
//! A manual would contain the same sentences and never be opened in a booth.
//!
//! # Why every technique carries a metaphor
//!
//! The interface already speaks a language: a phrase is a breath, energy is a
//! watershed, the library is a highland. Teaching in that same language costs
//! nothing and is remembered, where "16-bar structural boundary" is read once
//! and gone. See ASSISTANT.md §12.
//!
//! # Where this is wrong
//!
//! At the edges, deliberately. Difficulty is one working DJ's judgement, not a
//! syllabus; `Needs` says what a technique is *impossible* without rather than
//! what it is nicest with. Both are written down so they can be argued with,
//! which is the only way a table like this improves.

use dj_core::genre::Blendability;

/// What kind of move this is.
///
/// Grouped by what the hand is doing rather than by difficulty, because a DJ
/// reaching for something reaches by intent: "get out of this record", "put
/// these two together", "make something happen".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Getting two records playing together.
    Blend,
    /// Leaving one record for another, quickly.
    Exit,
    /// Shaping what is already playing.
    Shape,
    /// Playing the record as an instrument.
    Perform,
    /// Working with the arrangement -- phrases, loops, cues.
    Structure,
}

/// What a technique cannot be done without.
///
/// Only hard requirements. Almost everything here is *nicer* with a
/// controller; the ones marked as needing one cannot be done at all without,
/// and that distinction is the whole point of the field — a laptop DJ shown a
/// list half of which they cannot perform learns to ignore the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// A keyboard and a trackpad. Most of this table.
    Nothing,
    /// A jog wheel or a platter under the hand.
    Platter,
    /// A crossfader that can be cut, not dragged.
    Crossfader,
    /// Separated stems for the playing track.
    Stems,
    /// Two records whose structure is known -- a grid and phrases.
    Analysis,
}

/// Roughly how long before it is reliable in front of people.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty {
    /// Works the first time it is tried.
    First,
    /// A few nights of practice.
    Practised,
    /// Months, and it still goes wrong.
    Hard,
}

/// One move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Technique {
    pub name: &'static str,
    pub kind: Kind,
    pub needs: Needs,
    pub difficulty: Difficulty,
    /// What it does, in one line.
    pub what: &'static str,
    /// When a DJ reaches for it.
    pub when: &'static str,
    /// The bridge from the world, for teaching. See the module docs.
    pub metaphor: &'static str,
}

impl Technique {
    /// Whether this move is possible for two records that blend like this.
    ///
    /// The asymmetry is the useful part. A long blend is only available when
    /// the grammars agree; a cut is available always, which is exactly why it
    /// is the move that saves a set when nothing else will.
    #[must_use]
    pub fn works_when(&self, blend: Blendability) -> bool {
        match self.kind {
            // Holding two records together needs them to agree.
            Kind::Blend => blend == Blendability::Easy,
            // Everything else is done to one record, so what the other is
            // does not enter into it.
            Kind::Exit | Kind::Shape | Kind::Perform | Kind::Structure => true,
        }
    }
}

/// What the DJ has in front of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rig {
    pub platter: bool,
    pub crossfader: bool,
    pub stems: bool,
    pub analysis: bool,
}

impl Rig {
    /// A laptop and nothing else, with tracks analysed.
    ///
    /// The default djmanzo is built for. Note `analysis: true` — djmanzo
    /// analyses everything it loads, so the structural techniques are
    /// available to a DJ with no hardware at all, which is not true of most
    /// software.
    #[must_use]
    pub fn laptop() -> Self {
        Self {
            platter: false,
            crossfader: false,
            stems: false,
            analysis: true,
        }
    }

    /// A two-channel controller with a crossfader and jogs.
    #[must_use]
    pub fn controller() -> Self {
        Self {
            platter: true,
            crossfader: true,
            stems: false,
            analysis: true,
        }
    }

    /// Whether this rig can do a technique at all.
    #[must_use]
    pub fn allows(&self, needs: Needs) -> bool {
        match needs {
            Needs::Nothing => true,
            Needs::Platter => self.platter,
            Needs::Crossfader => self.crossfader,
            Needs::Stems => self.stems,
            Needs::Analysis => self.analysis,
        }
    }
}

/// The techniques that apply right now.
///
/// Filtered rather than ranked. A DJ with sixteen bars left does not want a
/// scored list; they want the three or four things that are possible, and to
/// pick one themselves.
#[must_use]
pub fn for_situation(blend: Blendability, rig: Rig) -> Vec<&'static Technique> {
    catalogue()
        .iter()
        .filter(|t| rig.allows(t.needs) && t.works_when(blend))
        .collect()
}

/// Look one up by name, case-insensitively.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static Technique> {
    catalogue()
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
}

/// The whole table.
#[must_use]
pub fn catalogue() -> &'static [Technique] {
    CATALOGUE
}

static CATALOGUE: &[Technique] = &[
    // -- getting two records together -----------------------------------
    Technique {
        name: "beatmatch by ear",
        kind: Kind::Blend,
        needs: Needs::Nothing,
        difficulty: Difficulty::Hard,
        what: "matching two tempos by listening and nudging, with no sync",
        when: "always worth being able to do; essential the night the analysis is wrong",
        metaphor: "two people walking together fall into step without counting; \
                   you are listening for the moment they stop fighting",
    },
    Technique {
        name: "sync",
        kind: Kind::Blend,
        needs: Needs::Analysis,
        difficulty: Difficulty::First,
        what: "the deck holds the tempo for you so your hands are free",
        when: "any time the hands are better spent on EQ, effects or the crowd",
        metaphor: "a river locked to its bed — it still has to be steered, \
                   it just will not wander off",
    },
    Technique {
        name: "long blend",
        kind: Kind::Blend,
        needs: Needs::Nothing,
        difficulty: Difficulty::Practised,
        what: "both records audible together for thirty seconds or more",
        when: "two records that agree, and a room that does not need surprising",
        metaphor: "two streams meeting — for a while there is one wider river \
                   and you cannot say where either began",
    },
    Technique {
        name: "bass swap",
        kind: Kind::Blend,
        needs: Needs::Nothing,
        difficulty: Difficulty::Practised,
        what: "drop the outgoing low as you raise the incoming, so only one bass plays",
        when: "any blend longer than a few bars; two basses at once is mud, not power",
        metaphor: "a riverbed only has room for one channel; \
                   the water has to leave one before it fills the other",
    },
    Technique {
        name: "phrase mix",
        kind: Kind::Blend,
        needs: Needs::Analysis,
        difficulty: Difficulty::Practised,
        what: "start the incoming record on a phrase boundary, not just a beat",
        when: "almost always — it is the difference between a mix and a collision",
        metaphor: "a breath. Come in mid-breath and the room hears you \
                   interrupt; come in on the next one and nobody notices",
    },
    Technique {
        name: "harmonic mix",
        kind: Kind::Blend,
        needs: Needs::Analysis,
        difficulty: Difficulty::Practised,
        what: "choosing the next record by key so the two do not fight",
        when: "long blends, and anything with a held vocal or pad",
        metaphor: "two streams meeting at a shallow angle join; \
                   meeting head-on they throw up spray",
    },
    Technique {
        name: "acapella over instrumental",
        kind: Kind::Blend,
        needs: Needs::Nothing,
        difficulty: Difficulty::Practised,
        what: "a vocal from one record over the music of another",
        when: "a moment nobody has heard before; the payoff for knowing your keys",
        metaphor: "a bird over a landscape — it belongs to neither and \
                   makes you look at both",
    },
    Technique {
        name: "double drop",
        kind: Kind::Blend,
        needs: Needs::Analysis,
        difficulty: Difficulty::Hard,
        what: "both records arriving at their drop on the same beat",
        when: "peak time, once, when it will land — not twice in a night",
        metaphor: "two waterfalls meeting at the bottom. \
                   Thrilling when it works and a mess when it does not",
    },
    // -- leaving a record ------------------------------------------------
    Technique {
        name: "cut",
        kind: Kind::Exit,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "one record stops, the next starts, on the beat",
        when: "records that will not blend, and any time the night needs a jolt",
        metaphor: "stepping over a wall instead of walking round it",
    },
    Technique {
        name: "echo out",
        kind: Kind::Exit,
        needs: Needs::Nothing,
        difficulty: Difficulty::Practised,
        what: "throw a delay on the outgoing record and pull the fader under it",
        when: "leaving a record that has no outro, without dead air",
        metaphor: "a shout in a valley — the sound leaves before the voice does",
    },
    Technique {
        name: "filter fade",
        kind: Kind::Exit,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "sweep the outgoing record's low end away rather than its volume",
        when: "when a volume fade sounds like something breaking",
        metaphor: "mist closing over a hill. It is still there; you just \
                   stop being able to make it out",
    },
    Technique {
        name: "brake",
        kind: Kind::Exit,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "the record slows to a stop as if the power went",
        when: "an announcement, a genre change, the end of a section",
        metaphor: "a wheel running down. Everyone knows what it means \
                   before they know what they heard",
    },
    Technique {
        name: "backspin",
        kind: Kind::Exit,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "the record is thrown backwards and the next one lands",
        when: "high energy, and a record that has said what it came to say",
        metaphor: "a wave pulling back off the sand before the next one breaks",
    },
    // -- shaping what is playing -----------------------------------------
    Technique {
        name: "EQ ride",
        kind: Kind::Shape,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "moving the three bands through a mix rather than setting them once",
        when: "every blend. It is the technique the others are built on",
        metaphor: "weather over a landscape — the same ground, lit differently",
    },
    Technique {
        name: "filter sweep",
        kind: Kind::Shape,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "opening or closing the whole record's tone over several bars",
        when: "building tension into a drop, or hiding a rough seam",
        metaphor: "a valley narrowing. Nothing is added; there is just \
                   less and less room until it opens out",
    },
    Technique {
        name: "stem swap",
        kind: Kind::Shape,
        needs: Needs::Stems,
        difficulty: Difficulty::Practised,
        what: "the drums of one record under the vocal of another, live",
        when: "when two records nearly work and one element is what is wrong",
        metaphor: "moving a tree rather than the hill it stands on",
    },
    Technique {
        name: "tone play",
        kind: Kind::Shape,
        needs: Needs::Analysis,
        difficulty: Difficulty::Hard,
        what: "pitching a cue or a stab to make a melody out of one sound",
        when: "an instrumental stretch that needs something on top",
        metaphor: "the same stone thrown into different depths of water",
    },
    // -- performing -------------------------------------------------------
    Technique {
        name: "baby scratch",
        kind: Kind::Perform,
        needs: Needs::Platter,
        difficulty: Difficulty::Practised,
        what: "the record pushed and pulled with the fader open",
        when: "the first scratch anybody learns, and still used every night",
        metaphor: "a hand in a stream, back and forth against the current",
    },
    Technique {
        name: "chirp",
        kind: Kind::Perform,
        needs: Needs::Crossfader,
        difficulty: Difficulty::Hard,
        what: "the fader closes at each turn, so the sound is clipped at both ends",
        when: "over a break, once the baby scratch is automatic",
        metaphor: "a bird call — it starts and stops sharply enough \
                   that the silence is part of it",
    },
    Technique {
        name: "transformer",
        kind: Kind::Perform,
        needs: Needs::Crossfader,
        difficulty: Difficulty::Hard,
        what: "the record runs and the fader chops it into pieces",
        when: "over a loop or a held note",
        metaphor: "sunlight through moving leaves",
    },
    Technique {
        name: "beat juggle",
        kind: Kind::Perform,
        needs: Needs::Analysis,
        difficulty: Difficulty::Hard,
        what: "two copies of one record cut against each other into a new pattern",
        when: "a break everybody knows, made into something they do not",
        metaphor: "a stone skipped so it touches the water in a rhythm \
                   the water did not have",
    },
    Technique {
        name: "loop roll",
        kind: Kind::Perform,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "a short loop held over the record, which carries on underneath",
        when: "the last bar before a drop; a stutter that costs nothing",
        metaphor: "an eddy at the edge of a river. The river does not stop",
    },
    Technique {
        name: "censor",
        kind: Kind::Perform,
        needs: Needs::Nothing,
        difficulty: Difficulty::First,
        what: "the record plays backwards, silently, and returns in time",
        when: "a word the room should not hear, with the beat unbroken",
        metaphor: "a held breath. Nothing was added and nothing was lost",
    },
    // -- structure ---------------------------------------------------------
    Technique {
        name: "loop the intro",
        kind: Kind::Structure,
        needs: Needs::Analysis,
        difficulty: Difficulty::First,
        what: "hold a short intro open until there is time to mix into it",
        when: "a record with eight bars of intro and a mix that needs thirty-two",
        metaphor: "damming a stream for as long as you need it to be deeper",
    },
    Technique {
        name: "hot cue juggle",
        kind: Kind::Structure,
        needs: Needs::Analysis,
        difficulty: Difficulty::Practised,
        what: "jumping between marked points to rearrange the record as it plays",
        when: "a long record with a short attention span in the room",
        metaphor: "walking a path you already know, taking the turns \
                   in a different order",
    },
    Technique {
        name: "slip trick",
        kind: Kind::Structure,
        needs: Needs::Nothing,
        difficulty: Difficulty::Practised,
        what: "loop, scratch or reverse while the record keeps running underneath",
        when: "anything that should sound like it never interrupted the track",
        metaphor: "the current under ice. The surface does what you like; \
                   underneath it never stopped",
    },
    Technique {
        name: "phrase jump",
        kind: Kind::Structure,
        needs: Needs::Analysis,
        difficulty: Difficulty::First,
        what: "moving a whole breath forward or back, landing in time",
        when: "a record that is running long, or an intro worth skipping",
        metaphor: "turning two pages instead of one, and still \
                   being on a paragraph",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **Nothing in the table is unreachable on the rig djmanzo assumes.**
    ///
    /// A laptop DJ shown a list of moves, half of which their setup cannot
    /// perform, learns to stop reading the list. So the laptop rig has to
    /// leave a usable set of techniques, not a token one.
    #[test]
    fn a_laptop_alone_can_still_do_most_of_this() {
        let available = for_situation(Blendability::Easy, Rig::laptop());
        assert!(
            available.len() * 2 > catalogue().len(),
            "only {} of {} techniques survive a laptop-only rig",
            available.len(),
            catalogue().len()
        );
    }

    /// **A cut is always available.**
    ///
    /// Which is exactly why it is the move that saves a set: when two records
    /// share no grammar and no tempo, everything else is off the table and
    /// this is not.
    #[test]
    fn something_is_always_possible_however_badly_two_records_agree() {
        for rig in [Rig::laptop(), Rig::controller(), Rig::default()] {
            let available = for_situation(Blendability::Cut, rig);
            assert!(
                available.iter().any(|t| t.name == "cut"),
                "no way out of a record on {rig:?}"
            );
        }
    }

    /// **Blends are withheld when the records will not blend.**
    ///
    /// The filter has to actually filter. Offering a long blend across a
    /// dembow/four-on-the-floor seam is worse than offering nothing: it is
    /// advice that produces a train wreck.
    #[test]
    fn holding_two_records_together_needs_them_to_agree() {
        let easy = for_situation(Blendability::Easy, Rig::controller());
        let cut = for_situation(Blendability::Cut, Rig::controller());
        assert!(easy.iter().any(|t| t.name == "long blend"));
        assert!(
            !cut.iter().any(|t| t.kind == Kind::Blend),
            "a blend was offered for records that cannot be blended"
        );
        // And the filter is not simply emptying the list.
        assert!(!cut.is_empty());
    }

    /// **A rig without a crossfader is not offered crossfader tricks.**
    #[test]
    fn a_rig_without_the_hardware_is_not_offered_the_move() {
        let laptop = for_situation(Blendability::Easy, Rig::laptop());
        for name in ["chirp", "transformer", "baby scratch"] {
            assert!(
                !laptop.iter().any(|t| t.name == name),
                "{name} was offered to a DJ with no platter or crossfader"
            );
        }
        let controller = for_situation(Blendability::Easy, Rig::controller());
        assert!(controller.iter().any(|t| t.name == "baby scratch"));
    }

    /// **A DJ whose tracks are not analysed yet still has moves.**
    ///
    /// Not a hypothetical: a fresh import is analysing in the background, and
    /// the grid for the record about to be loaded may simply not exist. Every
    /// structural technique is off the table there — and this is precisely
    /// why "beatmatch by ear" is in it. A table that assumed a grid would go
    /// blank at the one moment the DJ has to fall back on their ears.
    #[test]
    fn no_grid_yet_leaves_the_moves_that_never_needed_one() {
        let unanalysed = Rig {
            analysis: false,
            ..Rig::laptop()
        };
        let available = for_situation(Blendability::Easy, unanalysed);

        for needs_a_grid in ["sync", "phrase mix", "phrase jump", "loop the intro"] {
            assert!(
                !available.iter().any(|t| t.name == needs_a_grid),
                "{needs_a_grid} was offered without a beat grid"
            );
        }
        for still_works in ["beatmatch by ear", "bass swap", "cut", "EQ ride"] {
            assert!(
                available.iter().any(|t| t.name == still_works),
                "{still_works} needs no grid but was withheld"
            );
        }
    }

    /// **Every entry teaches, and every entry is findable.**
    ///
    /// The metaphor is not decoration -- it is what §12 says the teaching is
    /// made of, so an entry without one is an entry the learning module
    /// cannot use.
    #[test]
    fn every_technique_is_complete_and_uniquely_named() {
        let mut seen = std::collections::BTreeSet::new();
        for t in catalogue() {
            assert!(!t.metaphor.is_empty(), "{} has no metaphor", t.name);
            assert!(!t.what.is_empty(), "{} does not say what it does", t.name);
            assert!(!t.when.is_empty(), "{} does not say when", t.name);
            assert!(seen.insert(t.name), "{} appears twice", t.name);
            assert_eq!(
                by_name(t.name),
                Some(t),
                "{} cannot be looked up by its own name",
                t.name
            );
        }
    }

    #[test]
    fn a_name_is_matched_however_it_was_typed() {
        assert_eq!(by_name("  Bass Swap ").map(|t| t.name), Some("bass swap"));
        assert_eq!(by_name("nonsense"), None);
    }
}

//! What a record is *for*, which is not what it is.
//!
//! Genre says a record is bachata. It does not say whether it opens a room,
//! lifts one that is already moving, or is the thing you reach for when the
//! floor has emptied and you need it back inside ninety seconds. Those are the
//! questions a DJ actually asks of their collection, and no metadata field
//! anywhere answers them.
//!
//! # Why a closed vocabulary
//!
//! Free text would be easier to build and worse to use. The value of a
//! function tag is that it means the same thing on every record and can
//! therefore be *searched, counted and reasoned about* -- a smart folder for
//! "openers I have not played in three months" only works if "opener" is one
//! spelling. Free text gives you `opener`, `Opener`, `open`, `warmup` and
//! `warm-up` in the same collection within a month, which is five columns and
//! no answers.
//!
//! Ten of them, and the list is short on purpose. A vocabulary a DJ cannot
//! hold in their head is one they will not use consistently, and inconsistent
//! tags are worse than none: they look like data.
//!
//! # Why these ten
//!
//! Six are about *where in a night* a record belongs, and they are the shape
//! of every set: something opens, something builds, something peaks, something
//! rescues a floor that emptied, something everybody sings, something closes.
//! Two are about *how a record behaves in a mix* rather than where it sits --
//! a transition tool may be nobody's favourite record and still be the one
//! that gets you from 98 to 124 BPM. And two are about *risk*, which is the
//! axis a DJ actually navigates when the room is not what they expected.

use serde::{Deserialize, Serialize};

/// What a record is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Function {
    /// Sets the room going from nothing.
    Opener,
    /// Lifts a floor that is already moving.
    Builder,
    /// The top of an arc, spent rather than saved.
    Peak,
    /// Brings a floor back when it has emptied.
    FloorReset,
    /// Everybody knows the words.
    Singalong,
    /// Gets you from one tempo, key or genre to another. Not a favourite; a
    /// bridge.
    TransitionTool,
    /// Ends a night on purpose.
    Closer,
    /// Works in almost any room. The record you play when you have misread
    /// one.
    Safe,
    /// Works brilliantly or not at all, and you can tell within sixteen bars.
    Risky,
    /// For when something has gone wrong -- a dead floor, a lost file, a
    /// deck that will not load.
    Emergency,
}

impl Function {
    /// Every function, in the order a night uses them.
    ///
    /// Not alphabetical, deliberately: a picker in this order is a picker a DJ
    /// can read as a shape rather than as a list, and the six that describe
    /// where in a night a record sits come before the two about mixing and the
    /// two about risk.
    pub const ALL: [Function; 10] = [
        Function::Opener,
        Function::Builder,
        Function::Peak,
        Function::FloorReset,
        Function::Singalong,
        Function::Closer,
        Function::TransitionTool,
        Function::Safe,
        Function::Risky,
        Function::Emergency,
    ];

    /// The stored spelling. This is a compatibility surface the moment a DJ
    /// tags a record, so it is written out rather than derived from the
    /// variant name.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Function::Opener => "opener",
            Function::Builder => "builder",
            Function::Peak => "peak",
            Function::FloorReset => "floor-reset",
            Function::Singalong => "singalong",
            Function::Closer => "closer",
            Function::TransitionTool => "transition-tool",
            Function::Safe => "safe",
            Function::Risky => "risky",
            Function::Emergency => "emergency",
        }
    }

    /// What the interface calls it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Function::Opener => "Opener",
            Function::Builder => "Builder",
            Function::Peak => "Peak",
            Function::FloorReset => "Floor reset",
            Function::Singalong => "Singalong",
            Function::Closer => "Closer",
            Function::TransitionTool => "Transition tool",
            Function::Safe => "Safe",
            Function::Risky => "Risky",
            Function::Emergency => "Emergency",
        }
    }

    /// One line saying what it is for, shown where the DJ chooses.
    ///
    /// Carried in the code rather than in the interface because the assistant
    /// and the network API need the same words: a vocabulary explained in one
    /// place and re-explained in another is a vocabulary that drifts.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Function::Opener => "Sets the room going from nothing.",
            Function::Builder => "Lifts a floor that is already moving.",
            Function::Peak => "The top of an arc -- spent, not saved.",
            Function::FloorReset => "Brings a floor back when it has emptied.",
            Function::Singalong => "Everybody knows the words.",
            Function::Closer => "Ends a night on purpose.",
            Function::TransitionTool => {
                "Gets you from one tempo, key or genre to another. A bridge, not a favourite."
            }
            Function::Safe => "Works in almost any room.",
            Function::Risky => "Brilliant or not at all, and you know within sixteen bars.",
            Function::Emergency => "For when something has gone wrong.",
        }
    }

    /// Read a stored spelling back.
    ///
    /// `None` for anything this build does not know, which is deliberate: a
    /// database may outlive a rename, and a label from a newer djmanzo should
    /// be ignored rather than stop the library opening. The same rule the
    /// widget registry follows for an unknown widget.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.slug() == slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stored spellings are a compatibility surface: they are in every
    /// DJ's database the moment they tag a record.
    #[test]
    fn every_function_round_trips_through_its_stored_spelling() {
        for function in Function::ALL {
            assert_eq!(
                Function::from_slug(function.slug()),
                Some(function),
                "`{}` did not read back",
                function.slug()
            );
        }
    }

    #[test]
    fn the_vocabulary_has_no_duplicates_and_no_gaps() {
        let mut slugs: Vec<_> = Function::ALL.iter().map(|f| f.slug()).collect();
        slugs.sort_unstable();
        let count = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), count, "two functions share a stored spelling");

        for function in Function::ALL {
            assert!(!function.label().is_empty());
            assert!(
                function.about().ends_with('.'),
                "`{}` explains itself without finishing the sentence",
                function.slug()
            );
        }
    }

    /// A label this build does not know is skipped, not fatal.
    #[test]
    fn an_unknown_spelling_is_none_rather_than_a_panic() {
        assert_eq!(Function::from_slug("peak-adjacent"), None);
        assert_eq!(Function::from_slug("Opener"), None);
        assert_eq!(Function::from_slug(""), None);
    }

    /// JSON uses the same spelling the database does.
    ///
    /// Two vocabularies for one concept is how a value written by the
    /// interface stops matching a value written by the importer.
    #[test]
    fn the_json_spelling_is_the_stored_spelling() {
        for function in Function::ALL {
            let json = serde_json::to_string(&function).expect("a function serialises");
            assert_eq!(json, format!("\"{}\"", function.slug()));
        }
    }
}

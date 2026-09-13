//! §20's performance table: which columns a DJ can put on it.
//!
//! > Compact, information-dense. […] Allow instant custom column
//! > configuration.
//!
//! The browser has always drawn a fixed six — title, artist, album, BPM, key,
//! time — which is a spreadsheet, and §20's first instruction is *design a
//! DJ-native library surface, not merely a spreadsheet*. The gap was never the
//! data: a library row already carries the year, the loudness, the phrase
//! length, the rating, the play count and when it was last played, and every
//! one of them was being read from disk and thrown away.
//!
//! # What is here and what is not
//!
//! §20 lists twenty columns. Fourteen of them djmanzo can answer from a library
//! row, and those are the ones here. The other six are absent rather than
//! offered empty, which is this project's standing rule about claims it cannot
//! support:
//!
//! - **Camelot** is not a second column. djmanzo's `key` *is* Camelot — it is
//!   what a DJ mixes by — and a second column repeating it in another notation
//!   would be the same fact twice.
//! - **Colour** is not a column either, and that is a judgement rather than a
//!   gap: it is drawn as a stripe down the edge of the title, because a DJ
//!   colours tracks to find them at a glance and a column of swatches is not a
//!   glance, it is a column to read.
//! - **Vocal availability** and **stem availability** need analysis nobody has
//!   written. The separator can split any record on demand; whether a record
//!   *has* vocals is a different question and djmanzo has never asked it.
//! - **Transition suitability**, **AI confidence** and **function tag** are
//!   §76's lens, which is already a column set of its own with its own switch.
//!   Offering them here as well would give the same judgement two homes.
//! - **Request count** belongs to a night rather than to a record: §A6 counts
//!   requests for the set that is running, and a lifetime total across every
//!   night a record was requested at is a number with no question behind it.
//!
//! # Why the list is a type
//!
//! Because the header and the cells are two descriptions of one thing, and this
//! codebase has the scar from every other place that was true. A test reads
//! `Library.svelte` and fails when a column Rust offers is one the table cannot
//! draw — the same guard §7's presets needed after four of them opened surfaces
//! the shell silently dropped.

use serde::{Deserialize, Serialize};

/// One column of §20's performance table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Column {
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Bpm,
    Key,
    Duration,
    Loudness,
    Phrases,
    Rating,
    Plays,
    LastPlayed,
    Analysed,
}

impl Column {
    /// Every column there is, in the order the picker offers them.
    pub const ALL: [Self; 14] = [
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Genre,
        Self::Year,
        Self::Bpm,
        Self::Key,
        Self::Duration,
        Self::Loudness,
        Self::Phrases,
        Self::Rating,
        Self::Plays,
        Self::LastPlayed,
        Self::Analysed,
    ];

    /// What the browser has always shown, and therefore what a DJ who has never
    /// opened the picker keeps.
    ///
    /// A new column that appeared on its own would rearrange a table somebody
    /// reads at a glance in the dark, which is §18's whole posture about
    /// moving things under a working DJ.
    pub const SHIPPED: [Self; 6] = [
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Bpm,
        Self::Key,
        Self::Duration,
    ];

    /// The slug stored and sent.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Genre => "genre",
            Self::Year => "year",
            Self::Bpm => "bpm",
            Self::Key => "key",
            Self::Duration => "duration",
            Self::Loudness => "loudness",
            Self::Phrases => "phrases",
            Self::Rating => "rating",
            Self::Plays => "plays",
            Self::LastPlayed => "last-played",
            Self::Analysed => "analysed",
        }
    }

    /// The word at the top of the column.
    ///
    /// Short on purpose: §20 asks for *compact, information-dense*, and a
    /// heading wider than its cells is a column that costs a DJ room for
    /// nothing.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Genre => "Genre",
            Self::Year => "Year",
            Self::Bpm => "BPM",
            Self::Key => "Key",
            Self::Duration => "Time",
            Self::Loudness => "Loud",
            Self::Phrases => "Phrase",
            Self::Rating => "Rating",
            Self::Plays => "Plays",
            Self::LastPlayed => "Played",
            Self::Analysed => "Ready",
        }
    }

    /// What the column means, for the picker and for a hover.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Title => "The record's name. Always shown.",
            Self::Artist => "Who made it.",
            Self::Album => "The release it came from.",
            Self::Genre => "The tag on the file, as its own library wrote it.",
            Self::Year => "When it came out.",
            Self::Bpm => "Tempo, from the analyser. Blank until it has run.",
            Self::Key => "Camelot — the notation you mix by.",
            Self::Duration => "How long it runs.",
            // Not called "energy", deliberately. §20 asks for energy and what
            // djmanzo measures is integrated loudness, which is a fact; energy
            // is a judgement about a record that nothing here has made.
            Self::Loudness => {
                "Integrated loudness in LUFS — how loud it is mastered, not how hard it hits."
            }
            Self::Phrases => {
                "How many beats a phrase runs for, when the structure is clear enough to say."
            }
            Self::Rating => "Your own, out of five.",
            Self::Plays => "How many times you have played it.",
            Self::LastPlayed => "When you last played it.",
            Self::Analysed => "Whether it has everything sync and harmonic mixing need.",
        }
    }

    /// Read one back from its slug.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|column| column.name() == name)
    }
}

/// Turn what a DJ asked for into what the table will draw.
///
/// Three rules, and each is about a table that is still usable afterwards:
///
/// - **An unknown slug is dropped, not refused.** A preferences file written by
///   a later build may name a column this one has never heard of, and a DJ
///   opening an older djmanzo should get the columns it *does* have rather than
///   an error and a default table.
/// - **A column named twice is drawn once.**
/// - **Title is not optional.** A table without it is a list of BPMs. It goes
///   back at the front rather than being refused, because the DJ's intent —
///   "these other columns" — is clear and honouring it while quietly keeping the
///   one that makes the table readable is better than a dialog.
///
/// An empty ask is the shipped six rather than nothing: somebody who has
/// unticked everything wants the table back, not a blank surface.
#[must_use]
pub fn choose(asked: &[String]) -> Vec<Column> {
    let mut out: Vec<Column> = Vec::new();
    for name in asked {
        if let Some(column) = Column::from_name(name.trim())
            && !out.contains(&column)
        {
            out.push(column);
        }
    }
    if out.is_empty() {
        return Column::SHIPPED.to_vec();
    }
    if !out.contains(&Column::Title) {
        out.insert(0, Column::Title);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing one: a table always has a title, whatever was asked
    /// for.**
    ///
    /// The one column that is not a preference. A DJ who unticks it — by
    /// accident, or by asking for "just BPM and key" — is looking at a list of
    /// numbers with no records attached, and the only way back is a picker they
    /// can no longer tell the rows of.
    #[test]
    fn the_title_survives_every_way_of_asking_for_a_table_without_it() {
        let chosen = choose(&["bpm".to_owned(), "key".to_owned()]);
        assert_eq!(
            chosen.first(),
            Some(&Column::Title),
            "a table of BPMs with no records attached"
        );
        assert_eq!(chosen.len(), 3);

        // And an empty ask is the table back, not a blank surface.
        assert_eq!(choose(&[]), Column::SHIPPED.to_vec());
        assert_eq!(
            choose(&["not-a-column".to_owned()]),
            Column::SHIPPED.to_vec(),
            "a preferences file naming only columns this build lacks left the \
             browser with no table at all"
        );
    }

    /// A slug this build has never heard of is dropped rather than refused.
    ///
    /// A file written by a later djmanzo has to open in an earlier one: an
    /// error here would mean a DJ who opened a newer build once lost their
    /// column layout in the older one rather than losing the one column.
    #[test]
    fn a_column_this_build_does_not_have_is_skipped_and_the_rest_are_kept() {
        let chosen = choose(&[
            "title".to_owned(),
            "vocal-availability".to_owned(),
            "bpm".to_owned(),
        ]);
        assert_eq!(chosen, vec![Column::Title, Column::Bpm]);
    }

    /// Asked for twice is drawn once, in the place it was first asked for.
    #[test]
    fn a_column_named_twice_is_one_column() {
        let chosen = choose(&["bpm".to_owned(), "title".to_owned(), "bpm".to_owned()]);
        assert_eq!(chosen, vec![Column::Bpm, Column::Title]);
    }

    /// The order a DJ gives is the order they get.
    #[test]
    fn the_order_asked_for_is_the_order_drawn() {
        let chosen = choose(&[
            "title".to_owned(),
            "key".to_owned(),
            "bpm".to_owned(),
            "artist".to_owned(),
        ]);
        assert_eq!(
            chosen,
            vec![Column::Title, Column::Key, Column::Bpm, Column::Artist],
            "the picker sorted the columns into its own order, so a DJ who put \
             key beside title gets it wherever djmanzo prefers"
        );
    }

    /// Every column is spelled the same way stored as it is spoken, and every
    /// one has something to say for itself.
    #[test]
    fn every_column_has_a_slug_a_heading_and_a_sentence() {
        let mut seen = std::collections::BTreeSet::new();
        for column in Column::ALL {
            assert!(seen.insert(column.name()), "two columns share a slug");
            assert_eq!(
                serde_json::to_string(&column).expect("a column serialises"),
                format!("\"{}\"", column.name())
            );
            assert_eq!(Column::from_name(column.name()), Some(column));
            assert!(!column.heading().is_empty());
            assert!(
                column.about().len() > 10,
                "`{}` has nothing to tell a DJ it is",
                column.name()
            );
            assert!(
                column.heading().len() <= 7,
                "`{}`'s heading is wider than the cells under it, which costs a \
                 DJ room for nothing — §20 asks for compact",
                column.name()
            );
        }
    }

    /// What ships is a subset of what exists, and it is what the table already
    /// drew.
    ///
    /// A column appearing on its own would rearrange a table a DJ reads at a
    /// glance in the dark, which is the thing §18 is about.
    #[test]
    fn the_shipped_table_is_the_one_the_browser_already_had() {
        for column in Column::SHIPPED {
            assert!(Column::ALL.contains(&column));
        }
        assert!(Column::SHIPPED.contains(&Column::Title));
        assert!(
            Column::SHIPPED.len() < Column::ALL.len(),
            "everything is on by default, so there is nothing for the picker to \
             add and §20's density instruction is inverted"
        );
    }

    /// **The shell can draw every column Rust offers.**
    ///
    /// The trap §7's presets fell into: four of them opened surfaces the shell
    /// silently dropped, and nothing said so for weeks. A column offered in the
    /// picker and unknown to the table is an empty cell a DJ ticks on purpose.
    #[test]
    fn every_column_the_picker_offers_is_one_the_table_draws() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/Library.svelte");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the browser at {path}: {e}"))
            .replace("\r\n", "\n");

        for column in Column::ALL {
            let drawn = format!("case \"{}\":", column.name());
            assert!(
                source.contains(&drawn),
                "the picker offers `{}` and the table has no cell for it, so a \
                 DJ who ticks it gets a column of blanks",
                column.name()
            );
        }
    }
}

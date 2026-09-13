//! §8 Level 1, as a table: the nine things djmanzo remembers about a DJ.
//!
//! > **Level 1 — Remember.** The software remembers: workspace, density,
//! > library columns, sorting, panel positions, preferred decks, favorite pad
//! > pages, preferred waveform display, preferred controls.
//!
//! Nine things, and the only honest way to know how many of them djmanzo
//! actually keeps is to write them down and make a test read the answer. Five
//! were already kept when this was written, three were not, and one cannot be
//! until §25 ships waveform styles — and none of that was visible anywhere,
//! because a preference that is silently forgotten looks exactly like one that
//! was never set.
//!
//! # It names the file, and a test checks the file exists
//!
//! [`Remembered::kept_in`] is not a comment. A test reads `state.rs` and fails
//! when an entry claims a file nothing writes, which is the failure this
//! codebase keeps meeting from the other side: three fields have now been
//! found stored and read by nobody, and a serialised field round-trips
//! perfectly whether or not anything consults it.
//!
//! # `None` is a claim too
//!
//! An entry djmanzo does not keep says so, here and on screen, rather than
//! being left off the list. A DJ who sets their waveform up and finds it back
//! at the default tomorrow has learned something about djmanzo that this table
//! could have told them in advance — and leaving the row out would make the
//! list read as complete.

/// One of §8 Level 1's nine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remembered {
    /// The workspace: which arrangement the cockpit is in.
    Workspace,
    /// How tightly it is packed.
    Density,
    /// Which columns the performance table carries.
    Columns,
    /// Which column the rows are ordered by, and which way.
    Sorting,
    /// Where the panels sit, and which of them are pinned open.
    Panels,
    /// How many decks are on screen.
    Decks,
    /// Which pad pages a DJ actually plays from.
    PadPages,
    /// How the waveform is drawn.
    WaveformDisplay,
    /// Which controls stay on §74's rail whatever the deck is doing.
    Controls,
}

impl Remembered {
    /// §8's nine, in §8's order.
    pub const ALL: [Self; 9] = [
        Self::Workspace,
        Self::Density,
        Self::Columns,
        Self::Sorting,
        Self::Panels,
        Self::Decks,
        Self::PadPages,
        Self::WaveformDisplay,
        Self::Controls,
    ];

    /// The slug, for a test and for the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Density => "density",
            Self::Columns => "columns",
            Self::Sorting => "sorting",
            Self::Panels => "panels",
            Self::Decks => "decks",
            Self::PadPages => "pad-pages",
            Self::WaveformDisplay => "waveform-display",
            Self::Controls => "controls",
        }
    }

    /// What it is, in the words a DJ would use.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Workspace => "The arrangement you last worked in",
            Self::Density => "How tightly the interface is packed",
            Self::Columns => "Which columns the browser carries",
            Self::Sorting => "Which column the browser is ordered by, and which way",
            Self::Panels => "Where each panel sits, and which are pinned open",
            Self::Decks => "How many decks are on screen",
            Self::PadPages => "The pad pages you play from",
            Self::WaveformDisplay => "How the waveform is drawn",
            Self::Controls => "The controls you keep within reach",
        }
    }

    /// What a DJ would notice if djmanzo forgot it.
    ///
    /// The reason each row is worth a file, written in terms of the ten
    /// minutes before a set rather than in terms of the code. A preference
    /// that costs nothing to lose does not belong on this list.
    #[must_use]
    pub const fn forgotten(self) -> &'static str {
        match self {
            Self::Workspace => "You would set the room up again every night",
            Self::Density => "A laptop screen would open at a club's spacing",
            Self::Columns => "The six columns djmanzo ships, not the ones you read",
            Self::Sorting => "Back to artist, A to Z, every time you open the browser",
            Self::Panels => "Every panel you pinned would be closed again",
            Self::Decks => "Two decks, however many you play on",
            Self::PadPages => "Cues first, even if you never touch them",
            Self::WaveformDisplay => "Whatever djmanzo draws by default",
            Self::Controls => "Only what djmanzo judges you need this second",
        }
    }

    /// The file it is kept in, or `None` for one djmanzo does not yet keep.
    ///
    /// Four of them name `workspace.json` because the workspace *is* the
    /// arrangement: density, panel positions and deck count are three fields
    /// of one shape a DJ drags the application into, and splitting them into
    /// separate files would let a restart restore three quarters of a layout.
    #[must_use]
    pub const fn kept_in(self) -> Option<&'static str> {
        match self {
            Self::Workspace | Self::Density | Self::Panels | Self::Decks => Some("workspace.json"),
            Self::Columns => Some("columns.json"),
            Self::Sorting => Some("sort.json"),
            Self::PadPages => Some("pad-pages.json"),
            Self::Controls => Some("controls.json"),
            // §25 has not shipped waveform styles, so there is no preference to
            // keep -- not a file nobody wrote, a setting that does not exist.
            // The row stays on the list saying exactly that, because a list of
            // eight would read as the whole of §8.
            Self::WaveformDisplay => None,
        }
    }

    /// Whether djmanzo keeps it at all.
    #[must_use]
    pub const fn kept(self) -> bool {
        self.kept_in().is_some()
    }

    /// Why not, for the one that is not kept.
    ///
    /// Empty for every row djmanzo does keep. A sentence rather than a silence
    /// because "not yet, and here is what has to happen first" is a different
    /// thing from "no", and a DJ reading the list deserves to know which.
    #[must_use]
    pub const fn why_not(self) -> &'static str {
        match self {
            Self::WaveformDisplay => {
                "djmanzo draws one waveform style, so there is nothing to choose yet"
            }
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file `state.rs` is read from, with line endings normalised.
    ///
    /// The `\r\n` is not paranoia: a test reading source text for `"\n}\n"`
    /// passed on every machine here and failed only on Windows CI, and the
    /// whole of a house-pattern test is that it fails in the same place
    /// everywhere.
    fn state_source() -> String {
        include_str!("state.rs").replace("\r\n", "\n")
    }

    /// **Every file this table names is one the application writes.**
    ///
    /// The house pattern, pointed at the failure this project keeps finding
    /// from the other side. A row saying "kept in `sort.json`" when nothing
    /// opens `sort.json` is worse than no row: it is a promise on screen, in a
    /// list whose whole job is to tell a DJ what survives a restart.
    #[test]
    fn everything_this_table_claims_to_keep_has_somewhere_to_keep_it() {
        let state = state_source();
        for entry in Remembered::ALL {
            let Some(file) = entry.kept_in() else {
                continue;
            };
            let joined = format!(".join(\"{file}\")");
            assert!(
                state.contains(&joined),
                "{} says it is kept in {file}, but state.rs never opens it",
                entry.name()
            );
        }
    }

    /// **What is not kept says so, and says why.**
    ///
    /// The two halves have to agree: a row with nowhere to keep it must carry
    /// the sentence explaining that, and a row that *is* kept must not — a
    /// "not yet" beside a setting that works would be djmanzo lying about
    /// itself in the one place it claims to be honest.
    #[test]
    fn the_rows_djmanzo_does_not_keep_are_the_rows_that_explain_themselves() {
        for entry in Remembered::ALL {
            assert_eq!(
                entry.kept(),
                entry.why_not().is_empty(),
                "{} is inconsistent about whether djmanzo keeps it",
                entry.name()
            );
        }
    }

    /// **Nine rows, and nine different ones.**
    ///
    /// §8 lists nine. A tenth that is really one of the nine under another
    /// name would make the list read as more than djmanzo does.
    #[test]
    fn the_table_is_section_eights_nine() {
        let mut names: Vec<&str> = Remembered::ALL.iter().map(|e| e.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 9);
        for entry in Remembered::ALL {
            assert!(
                !entry.about().is_empty(),
                "{} has nothing to say",
                entry.name()
            );
            assert!(
                !entry.forgotten().is_empty(),
                "{} does not say what losing it costs",
                entry.name()
            );
        }
    }
}

//! §32's theme packs, as a table.
//!
//! > Create a proper theme-pack architecture. Initial themes: Studio Neutral,
//! > Pro Dark, Daylight, High Contrast, Minimal, Club, Festival, Beach Sunset,
//! > Caribbean, Latin, Wedding, Lounge, Scratch, Stem Lab,
//! > Cyber / Experimental, Watershed Living. **The existing watershed metaphor
//! > should become a theme/world pack, not the only possible identity.** It is
//! > a good visual language. It must not constrain DJs who do not want
//! > metaphors.
//!
//! # The architecture was there; the table was not
//!
//! `ui/src/controls/themes/` has been a real pack architecture for a long
//! time: a package is a palette, a geometry generator, a list of behaviours and
//! a list of effects, run through one pipeline. What was missing is anything
//! that knows **which themes there are supposed to be**.
//!
//! Rust names theme ids in three places — §31's [`crate::mood`] table, §7's
//! workspaces and §54's setups — and each had its own test that read
//! `packages.ts` and grepped for `id: "…"`. Three copies of one guard, all
//! pointing the same way: *does this id exist?* None of them could answer the
//! question §32 actually asks, which is *does every theme §32 named exist, and
//! if not, which*. Six of sixteen shipped and nothing anywhere said so.
//!
//! # A row that does not ship says why
//!
//! The same posture as §8's `crate::remembered`, and for the same reason: a
//! list of six would read as the whole of §32. The ten absent rows are on the
//! list with a sentence each, and the picker shows them — which is what made
//! §8's waveform row close itself, once the gap was somewhere a person could
//! see it rather than only in a document.
//!
//! # The mapping is conservative on purpose
//!
//! Six of djmanzo's packages answer to one of §32's names, and they are the six
//! where the name really is the same thing: *Booth* is "playing in the dark,
//! highest contrast, least motion", which is Pro Dark. *Industrial Techno* is
//! **not** filed under Club — it is a harder, narrower room than §32's general
//! club theme, and filing it there would let §32's Club row read as done while
//! a DJ who asked for a club theme got hard music's. It is one of two rows here
//! that §32 did not ask for and djmanzo ships anyway.

/// One of §32's themes, or one djmanzo ships that §32 did not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// What §32 calls it, or what djmanzo calls it for the two extras.
    pub title: &'static str,
    /// What kind of room or evening it is for, in one line.
    pub about: &'static str,
    /// The package id the interface ships for it, exactly as
    /// `ui/src/controls/themes/packages.ts` spells it, or `None`.
    ///
    /// An id rather than a description: the interface owns the pixels and Rust
    /// owns the choice, the same split the density bands already use.
    pub pack: Option<&'static str>,
    /// Why it does not ship. Empty for the rows that do.
    pub why_not: &'static str,
    /// Whether §32 named it.
    ///
    /// False for the two djmanzo ships anyway. Kept as a field rather than as
    /// two lists, so "how many of §32's did we do" is a count over one table
    /// rather than an act of memory.
    pub asked: bool,
    /// Whether wearing it opens the watershed.
    ///
    /// §32's second paragraph is the whole of this field: the metaphor *is* a
    /// good visual language and it *must not* be the only identity, so it is
    /// one theme among sixteen rather than a mode bolted beside them. It is a
    /// starting point and not a cage — choosing it opens the watershed, and
    /// the toggle in the status strip still closes it, which is the same
    /// contract §54's presets have.
    pub world: bool,
}

/// §32's sixteen, and the two djmanzo ships that §32 did not name.
pub const ALL: [Theme; 22] = [
    Theme {
        title: "Studio Neutral",
        about: "Long evenings at a desk. Easy on the eyes for hours at a time.",
        pack: Some("pkg-studio"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Pro Dark",
        about: "Playing in the dark. Highest contrast, least motion, nothing to miss.",
        pack: Some("pkg-booth"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Daylight",
        about: "Preparing by a window, or an afternoon set outdoors.",
        pack: Some("pkg-daylight"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "High Contrast",
        about: "Everything pushed to the ends of the range.",
        pack: None,
        // Plain prose: this string is shown in the picker, not rendered as
        // documentation, so emphasis markers would arrive on screen as
        // literal asterisks. Found by looking at it.
        why_not: "Not a palette but an override. The stylesheet raises contrast \
                  on every theme under the operating system's own \
                  high-contrast setting, which is \
                  stronger than a seventeenth palette a DJ has to find: it \
                  works on the theme they already chose rather than replacing \
                  it.",
        asked: true,
        world: false,
    },
    Theme {
        title: "Minimal",
        about: "Nothing on screen that is not load-bearing.",
        pack: None,
        why_not: "Minimal is a density, not a palette, and djmanzo already has \
                  four of those — the interface fits its own spacing to the \
                  window. A theme by this name would be a second control for \
                  something §5 already does, and the two would disagree.",
        asked: true,
        world: false,
    },
    Theme {
        title: "Club",
        about: "A dark room with people in it.",
        pack: Some("pkg-club"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Festival",
        about: "Daylight, a big stage, and a screen nobody can shade.",
        pack: Some("pkg-festival"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Beach Sunset",
        about: "Golden hour on a terrace, when the room is neither light nor dark.",
        pack: Some("pkg-sunset"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Caribbean",
        about: "Warm, bright, and unhurried.",
        pack: Some("pkg-caribbean"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Latin",
        about: "The one §32 names after a genre rather than a room.",
        pack: Some("pkg-latin"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Wedding",
        about: "A room that is not there for the DJ.",
        pack: Some("pkg-wedding"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Lounge",
        about: "Low light, low tempo, and nobody dancing yet.",
        pack: Some("pkg-lounge"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Scratch",
        about: "Hands on the platters, eyes on two records.",
        pack: Some("pkg-scratch"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Stem Lab",
        about: "Four parts of one record, told apart at a glance.",
        pack: Some("pkg-stemlab"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Cyber / Experimental",
        about: "Peak time. Everything moves; needs the machine to have it to spare.",
        pack: Some("pkg-cyber"),
        why_not: "",
        asked: true,
        world: false,
    },
    Theme {
        title: "Watershed Living",
        about: "The mix as moving water: decks as rivers, the crossfader as a confluence.",
        pack: Some("pkg-watershed"),
        why_not: "",
        asked: true,
        world: true,
    },
    // -- The two §32 did not name --------------------------------------------
    Theme {
        title: "Organic Base",
        about: "The default. Calm, green, and readable in most rooms.",
        pack: Some("pkg-organic"),
        why_not: "",
        asked: false,
        world: false,
    },
    Theme {
        title: "Industrial Techno",
        about: "A dark room, hard music, and a screen that should look like the music.",
        pack: Some("pkg-industrial"),
        why_not: "",
        asked: false,
        world: false,
    },
    // §33's, not §32's, and the third row here djmanzo ships that §32 did not
    // name. Every other theme is designed for a room; this one is designed for
    // an eye, and its four role colours are chosen by measurement rather than
    // by mood -- see `cockpit::tests::at_least_one_palette_holds_up_for_a_
    // colour_blind_dj`.
    Theme {
        title: "Colour-blind safe",
        about: "Every colour djmanzo uses to mean something, chosen to stay apart for a \
                colour-blind DJ.",
        pack: Some("pkg-legible"),
        why_not: "",
        asked: false,
        world: false,
    },
    // §113's three: themes designed from what a colour *does*. §32 did not
    // name them, so `asked` is false here too; §113 is where they come from.
    Theme {
        title: "Spectrum",
        about: "When the waveform's colours should be the only colours: a neutral room, and \
                every EQ knob in the colour of the band it cuts.",
        pack: Some("pkg-spectrum"),
        why_not: "",
        asked: false,
        world: false,
    },
    Theme {
        title: "Signal",
        about: "A colour code read from across the booth: green is going, cyan is chosen, \
                amber is about to, red is stop.",
        pack: Some("pkg-signal"),
        why_not: "",
        asked: false,
        world: false,
    },
    Theme {
        title: "Aurora",
        about: "Late-night and lounge sets where the screen may glow: a night sky from teal to \
                violet, the same two colours at the ends of every gradient.",
        pack: Some("pkg-aurora"),
        why_not: "",
        asked: false,
        world: false,
    },
];

/// The row a package id belongs to, or `None` for an id nothing ships.
#[must_use]
pub fn for_pack(id: &str) -> Option<&'static Theme> {
    ALL.iter().find(|t| t.pack == Some(id))
}

/// Whether this package id is one the interface can actually wear.
#[must_use]
pub fn ships(id: &str) -> bool {
    for_pack(id).is_some()
}

/// Whether wearing this package opens the watershed.
///
/// False for an id nothing ships, which is the safe direction: a stored theme
/// djmanzo no longer has must not turn a whole visual language on.
#[must_use]
pub fn wears_the_world(id: &str) -> bool {
    for_pack(id).is_some_and(|t| t.world)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every package id the interface ships, read off `packages.ts`.
    ///
    /// Read rather than copied. A list of ids maintained twice is a list that
    /// will disagree, and this particular disagreement is **silent**: a theme
    /// djmanzo asks for and no package answers to falls back to the organic
    /// palette, so picking it shows another theme's colours and nothing
    /// throws.
    fn shipped() -> Vec<String> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/themes/packages.ts"
        );
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the theme packages at {path}: {e}"))
            .replace("\r\n", "\n");

        let ids: Vec<String> = source
            .lines()
            .filter_map(|line| {
                let rest = line.trim().strip_prefix("id: \"")?;
                let end = rest.find('"')?;
                Some(rest[..end].to_owned())
            })
            .collect();
        assert!(
            !ids.is_empty(),
            "`id: \"...\"` is no longer how the packages are written, so this \
             guard has silently stopped guarding"
        );
        ids
    }

    /// **The load-bearing one: this table and the interface agree, both ways.**
    ///
    /// Both ways is the new half. The three guards this replaces each asked
    /// only *does this id exist*, so a package added to the interface and named
    /// by nothing here was invisible — and an id here that the interface
    /// dropped would have been caught only if some workspace happened to name
    /// it. §32 is a list of what there is *supposed* to be, and a list that can
    /// only detect one kind of disagreement is half a list.
    #[test]
    fn every_theme_that_claims_to_ship_ships_and_nothing_ships_unnamed() {
        let shipped = shipped();
        for theme in ALL {
            let Some(pack) = theme.pack else {
                assert!(
                    !theme.why_not.trim().is_empty(),
                    "{} does not ship and does not say why, so the picker would \
                     show a blank where the reason goes",
                    theme.title
                );
                continue;
            };
            assert!(
                theme.why_not.is_empty(),
                "{} ships and still carries a reason it does not",
                theme.title
            );
            assert!(
                shipped.iter().any(|id| id == pack),
                "{} claims the package `{pack}`, which packages.ts does not have \
                 — choosing it would silently show the organic palette",
                theme.title
            );
        }

        for id in &shipped {
            assert!(
                for_pack(id).is_some(),
                "packages.ts ships `{id}` and no row here names it, so it is a \
                 theme §32 cannot account for and the picker cannot explain"
            );
        }
    }

    /// **Every theme Rust asks for by name is one that ships.**
    ///
    /// The guard the three greps were for, now asked once. §31's mood table,
    /// §7's workspaces and §54's setups all name theme ids, and a theme djmanzo
    /// asks for that no package answers to looks exactly like a theme that
    /// decided not to change.
    #[test]
    fn everything_rust_asks_to_wear_is_something_it_can_wear() {
        for setting in crate::setting::Setting::ALL {
            for phase in dj_core::SessionPhase::ALL {
                let theme = crate::mood::wanted(setting, phase);
                assert!(
                    ships(theme),
                    "{setting:?} at {phase:?} wants `{theme}`, which no package ships"
                );
            }
        }
        for workspace in crate::cockpit::workspaces() {
            if workspace.theme.is_empty() {
                continue;
            }
            assert!(
                ships(&workspace.theme),
                "the `{}` arrangement asks for `{}`, which no package ships",
                workspace.name,
                workspace.theme
            );
        }
        for setup in crate::setup::ALL {
            if setup.theme.is_empty() {
                continue;
            }
            assert!(
                ships(setup.theme),
                "the `{}` setup asks for `{}`, which no package ships",
                setup.setting.slug(),
                setup.theme
            );
        }
    }

    /// **§32's sixteen are all here, spelled as §32 spells them.**
    ///
    /// Written out so that dropping one is a decision somebody makes here
    /// rather than something that happens quietly when a row is edited.
    #[test]
    fn the_directive_names_sixteen_and_all_sixteen_are_on_the_list() {
        let asked: std::collections::BTreeSet<&str> =
            ALL.iter().filter(|t| t.asked).map(|t| t.title).collect();
        let directive = [
            "Studio Neutral",
            "Pro Dark",
            "Daylight",
            "High Contrast",
            "Minimal",
            "Club",
            "Festival",
            "Beach Sunset",
            "Caribbean",
            "Latin",
            "Wedding",
            "Lounge",
            "Scratch",
            "Stem Lab",
            "Cyber / Experimental",
            "Watershed Living",
        ];
        for name in directive {
            assert!(
                asked.contains(name),
                "§32 asks for `{name}` and no row names it"
            );
        }
        assert_eq!(
            asked.len(),
            directive.len(),
            "a row is marked as §32's and is not one of §32's sixteen"
        );

        // And the extras are honest about not being asked for by §32. Six, and
        // each for a stated reason: two are rooms §32's list happens not to
        // name, the third is §33's -- a palette chosen by measurement rather
        // than for a room at all -- and the last three are §113's, designed
        // from what a colour does.
        let extras: Vec<&str> = ALL.iter().filter(|t| !t.asked).map(|t| t.title).collect();
        assert_eq!(
            extras,
            [
                "Organic Base",
                "Industrial Techno",
                "Colour-blind safe",
                "Spectrum",
                "Signal",
                "Aurora"
            ]
        );
    }

    /// **Exactly one theme is the metaphor.**
    ///
    /// §32's second paragraph, as an assertion. Two would mean the watershed had
    /// quietly become a mode again — something that comes on under several
    /// identities rather than being one of them — and zero would mean it is
    /// still bolted beside the themes rather than among them.
    #[test]
    fn the_watershed_is_one_identity_among_them_and_only_one() {
        let worlds: Vec<&str> = ALL.iter().filter(|t| t.world).map(|t| t.title).collect();
        assert_eq!(worlds, ["Watershed Living"]);
        assert!(wears_the_world("pkg-watershed"));
        assert!(!wears_the_world("pkg-organic"));
        // An id nothing ships must not turn a visual language on.
        assert!(!wears_the_world("pkg-nothing-like-this"));
    }

    /// **Nothing here is written in the notation the rest of this file uses.**
    ///
    /// Every string in this table is shown to a DJ, and the picker renders it
    /// as text. A `*word*` written out of habit — this codebase's own doc
    /// comments are full of them, three lines above each of these rows —
    /// arrives on screen as literal asterisks, and a backtick as a backtick.
    /// One of each shipped and was found by looking at the running
    /// application, which is where it had to be found: no type-check and no
    /// browser test reads copy.
    #[test]
    fn nothing_a_dj_reads_is_written_in_markup() {
        for theme in ALL {
            for (what, text) in [("about", theme.about), ("why_not", theme.why_not)] {
                for mark in ['*', '`', '_', '#'] {
                    assert!(
                        !text.contains(mark),
                        "{}'s {what} contains `{mark}`, which the picker draws as \
                         itself: `{text}`",
                        theme.title
                    );
                }
            }
        }
    }

    /// The titles and the package ids are both distinct: two rows sharing
    /// either would make the picker a coin toss.
    #[test]
    fn no_two_rows_look_alike() {
        let titles: std::collections::BTreeSet<_> = ALL.iter().map(|t| t.title).collect();
        let packs: std::collections::BTreeSet<_> = ALL.iter().filter_map(|t| t.pack).collect();
        assert_eq!(titles.len(), ALL.len());
        assert_eq!(packs.len(), ALL.iter().filter(|t| t.pack.is_some()).count());
    }
}

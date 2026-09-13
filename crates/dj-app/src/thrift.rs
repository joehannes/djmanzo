//! §48's laptop mode, as a table of what djmanzo gives up and in what order.
//!
//! > The app must detect limited machines. Adaptive GUI should degrade
//! > gracefully: reduce animation, reduce visual layers, reduce audience
//! > polling frequency, reduce expensive previews, reduce theme effects, reduce
//! > CPU-heavy visual analysis, preserve audio first. The interface must
//! > explicitly prioritize: **AUDIO > CONTROL > VISUAL EFFECTS.** Never the
//! > reverse.
//!
//! # The detection was there; the priority was not
//!
//! `ui/src/performance.svelte.ts` has measured the interface's own frame rate
//! for a long time and steps down a tier on a bad second, up a tier after ten
//! good ones. What consulted that tier was the theme pipeline and nothing else
//! — two of §48's seven bullets, both of them in the cheapest band.
//!
//! So the machine was detected and almost nothing was given up, and the
//! sentence §48 ends on had no expression anywhere. That sentence is not a
//! preference; it is the one thing a DJ has to be able to trust about a
//! degrading interface, because the failure it forbids is the one that ends a
//! set: a laptop under load that keeps its glow and drops its audio.
//!
//! # Why a table rather than seven `if` statements
//!
//! Seven scattered checks cannot be asked *what order do we give things up in*,
//! which is the only question §48 actually poses. This table can, and three
//! tests hold it to the answer: nothing in the audio band is ever given up,
//! nothing in the control band is given up while anything visual is still being
//! paid for, and a thing given up at one tier stays given up below it.
//!
//! # A row djmanzo does not do yet says so
//!
//! The same posture as §8's [`crate::remembered`] and §32's [`crate::theme`].
//! Two of §48's seven are deliberate refusals rather than gaps, and both are
//! collisions with other sections that had to be decided somewhere: see
//! [`Spend::Layers`] and [`Spend::Previews`].

/// Which of §48's three bands a cost belongs to.
///
/// The whole of *AUDIO > CONTROL > VISUAL EFFECTS*, as a type. Ordered so that
/// the derived comparison **is** the priority: `Band::Audio < Band::Control <
/// Band::Visual`, and a cheaper band is one djmanzo gives up first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    /// The sound itself. Never given up, at any tier, for any reason.
    Audio,
    /// Reaching a control and knowing what it says. Given up only if the
    /// alternative is not reaching it at all — which, so far, it never is.
    Control,
    /// How the interface looks while it does the other two.
    Visual,
}

impl Band {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Control => "control",
            Self::Visual => "visual",
        }
    }
}

/// How hard the machine is working, as the interface's governor decides it.
///
/// The same three the interface already has, named here so Rust can talk about
/// them. `Ultra` is a machine with room to spare; `Eco` is one that has been
/// dropping frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Dropping frames. Everything that can go, goes.
    Eco,
    /// Keeping up, with nothing spare.
    Balanced,
    /// Room to spare.
    Ultra,
}

impl Tier {
    /// Hardest-pressed first, which is the order the table is read in.
    pub const ALL: [Self; 3] = [Self::Eco, Self::Balanced, Self::Ultra];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Eco => "Eco",
            Self::Balanced => "Balanced",
            Self::Ultra => "Ultra",
        }
    }

    /// A tier by the name the interface spells it with.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.name() == name)
    }
}

/// One of §48's seven, and what djmanzo does about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spend {
    /// §48's own words for it.
    pub what: &'static str,
    /// What a DJ would notice going, in their words.
    pub about: &'static str,
    /// Which of §48's three bands it is in.
    pub band: Band,
    /// The hardest tier at which djmanzo still pays for this.
    ///
    /// `Some(Balanced)` means it is given up at Eco and kept at Balanced and
    /// Ultra. `None` means it is never given up, which is the whole of the
    /// audio band and is a claim rather than an omission.
    pub kept_to: Option<Tier>,
    /// Why it is never given up, or why djmanzo does not give it up yet.
    ///
    /// Empty for the rows that simply are given up. Two of §48's seven carry a
    /// reason that is a decision rather than a gap.
    pub why_not: &'static str,
}

impl Spend {
    /// §48's seven, in §48's order.
    pub const ALL: [Self; 7] = [
        Self {
            what: "animation",
            about: "Controls stop pulsing and shimmering with the music.",
            band: Band::Visual,
            kept_to: Some(Tier::Balanced),
            why_not: "",
        },
        Self {
            what: "visual layers",
            about: "The waveform draws fewer things at once.",
            band: Band::Visual,
            // Not given up, and this is a decision. §25's layers are a *choice*
            // — §8 Level 1 keeps it, and §79's waveform lock exists so a DJ can
            // say the interface may not change it. Dropping one under load
            // would be djmanzo overriding a deliberate choice to save a frame,
            // which is §79's own complaint. What goes instead at Eco is the
            // *answering*: the strip stops reacting to the audio, and every
            // layer a DJ asked for is still drawn.
            kept_to: None,
            why_not: "The layers on the waveform are yours. §79 lets you lock \
                      the interface out of changing them, so djmanzo drops the \
                      answering rather than the layers: the strip stops \
                      reacting to the audio and still draws everything you \
                      asked for.",
        },
        Self {
            what: "audience polling frequency",
            about: "The room is read less often — every eight seconds instead of two.",
            band: Band::Visual,
            kept_to: Some(Tier::Balanced),
            why_not: "",
        },
        Self {
            what: "expensive previews",
            about: "Artwork and waveform overviews are drawn on demand.",
            band: Band::Visual,
            // Not given up, because djmanzo has none of the expensive kind.
            // The overview is a tile the renderer already caches by key, and
            // artwork is whatever the file carried. There is nothing here to
            // stop doing, and a row claiming a saving nobody makes is worse
            // than a row saying so.
            kept_to: None,
            why_not: "djmanzo has no expensive previews to stop drawing. The \
                      overview is a cached tile and the artwork is whatever the \
                      file already carried.",
        },
        Self {
            what: "theme effects",
            about: "Glow, chromatic aberration and blend modes come off.",
            band: Band::Visual,
            kept_to: Some(Tier::Balanced),
            why_not: "",
        },
        Self {
            what: "CPU-heavy visual analysis",
            about: "The camera's optical flow stops being computed every tick.",
            band: Band::Visual,
            kept_to: Some(Tier::Balanced),
            why_not: "",
        },
        Self {
            what: "the audio",
            about: "Never. The sound is what the other six are spent on.",
            band: Band::Audio,
            // The one row §48 writes as an instruction rather than a saving,
            // and the reason the table exists. It is also structurally true
            // here rather than merely intended: the audio thread never
            // allocates, locks or does I/O (ADR-0001), and nothing the
            // interface decides can reach it.
            kept_to: None,
            why_not: "The audio thread never allocates, locks or does I/O, and \
                      nothing the interface decides can reach it. There is no \
                      tier at which djmanzo spends the sound on the screen.",
        },
    ];

    /// Whether djmanzo still pays for this at that tier.
    #[must_use]
    pub fn paid_at(&self, tier: Tier) -> bool {
        self.kept_to.is_none_or(|kept| tier >= kept)
    }
}

/// What djmanzo gives up at this tier, in §48's order.
///
/// Empty at `Ultra`, which is the honest answer for a machine with room to
/// spare and not a missing feature.
#[must_use]
pub fn given_up_at(tier: Tier) -> Vec<&'static Spend> {
    Spend::ALL.iter().filter(|s| !s.paid_at(tier)).collect()
}

/// How often the room may be read at this tier, in milliseconds.
///
/// §48's *reduce audience polling frequency*, as the one number the interface
/// needs. It is here rather than in the component because the decision is a
/// priority decision and the table above is where those live — and because a
/// second copy of "two seconds, or eight when struggling" is how the two come
/// to disagree.
///
/// The near window `dj_app::room` keeps is three minutes, so eight seconds
/// still fills it with twenty-two readings. That is fewer than the ninety a
/// healthy machine takes and it is enough for a median to mean something,
/// which is the test of whether this saving costs a feature or only sharpness.
#[must_use]
pub const fn room_poll_ms(tier: Tier) -> u32 {
    match tier {
        Tier::Eco => 8000,
        Tier::Balanced | Tier::Ultra => 2000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The load-bearing one: the audio is never given up, at any tier.**
    ///
    /// §48's last line is *AUDIO > CONTROL > VISUAL EFFECTS. Never the
    /// reverse*, and this is the half of it that has to hold absolutely. The
    /// failure it forbids is the one that ends a set — a laptop under load that
    /// keeps its glow and drops its sound — and it is exactly the failure a
    /// well-meaning future edit produces, because the audio band is the only
    /// one where giving something up would buy a lot.
    #[test]
    fn nothing_in_the_audio_band_is_ever_given_up() {
        for tier in Tier::ALL {
            for spend in Spend::ALL {
                if spend.band == Band::Audio {
                    assert!(
                        spend.paid_at(tier),
                        "`{}` is given up at {}, and §48 says never the reverse",
                        spend.what,
                        tier.name()
                    );
                }
            }
        }
        // And the band exists in the table at all. A priority list with nothing
        // in its top band is a list that has quietly become two bands.
        assert!(
            Spend::ALL.iter().any(|s| s.band == Band::Audio),
            "nothing in this table is in the audio band, so §48's first word \
             is not being said anywhere"
        );
    }

    /// **And nothing is given up out of order.**
    ///
    /// The other half of §48's sentence. At every tier, the set of things
    /// djmanzo has stopped paying for must be a prefix of the priority: if
    /// anything in the control band has gone, everything visual must have gone
    /// first. Checked at every tier rather than only at Eco, because the wrong
    /// order is easiest to introduce in the middle.
    #[test]
    fn the_cheapest_band_is_always_the_one_that_goes_first() {
        for tier in Tier::ALL {
            let gone = given_up_at(tier);
            let Some(cheapest_kept) = Spend::ALL
                .iter()
                .filter(|s| s.paid_at(tier))
                .map(|s| s.band)
                .max()
            else {
                continue;
            };
            for spend in gone {
                assert!(
                    spend.band >= cheapest_kept,
                    "at {} djmanzo gives up `{}` ({}) while still paying for \
                     something in the {} band — §48: never the reverse",
                    tier.name(),
                    spend.what,
                    spend.band.name(),
                    cheapest_kept.name()
                );
            }
        }
    }

    /// **A saving made on a struggling machine is not un-made on a worse one.**
    ///
    /// Monotone down the tiers. A row given up at Balanced and paid for again
    /// at Eco would make the governor's step down *cost* work, which is the
    /// opposite of what stepping down is for — and it is invisible, because
    /// each tier on its own looks reasonable.
    #[test]
    fn a_harder_pressed_machine_never_pays_for_more() {
        for spend in Spend::ALL {
            for pair in Tier::ALL.windows(2) {
                let (harder, easier) = (pair[0], pair[1]);
                assert!(
                    !(spend.paid_at(harder) && !spend.paid_at(easier)),
                    "`{}` is paid for at {} and not at {}",
                    spend.what,
                    harder.name(),
                    easier.name()
                );
            }
        }
    }

    /// **Every row either goes or says why it does not.**
    ///
    /// §8's posture, and §48 needs it more than most: three of these seven are
    /// not savings djmanzo makes, and a table that simply left them out would
    /// read as §48 being done. One of the three is a claim (the audio), and two
    /// are decisions taken against other sections — which is exactly the kind
    /// of thing that has to be written down where the next person will look.
    #[test]
    fn a_row_that_is_never_given_up_says_why() {
        for spend in Spend::ALL {
            if spend.kept_to.is_none() {
                assert!(
                    !spend.why_not.trim().is_empty(),
                    "`{}` is never given up and does not say why",
                    spend.what
                );
            } else {
                assert!(
                    spend.why_not.is_empty(),
                    "`{}` is given up and still carries a reason it is not",
                    spend.what
                );
            }
            for mark in ['*', '`', '_', '#'] {
                assert!(
                    !spend.about.contains(mark) && !spend.why_not.contains(mark),
                    "`{}` is written in markup, which the panel draws as itself",
                    spend.what
                );
            }
        }
    }

    /// **A struggling machine reads the room less often, and still enough.**
    ///
    /// §48 names audience polling explicitly. The saving has to be real — Eco
    /// must actually be slower — and it has to leave §39's near window with
    /// enough readings for a median to mean something, or this is a feature
    /// removed rather than a cost reduced.
    #[test]
    fn the_room_is_read_less_often_under_load_and_still_often_enough() {
        assert!(room_poll_ms(Tier::Eco) > room_poll_ms(Tier::Balanced));
        assert_eq!(room_poll_ms(Tier::Balanced), room_poll_ms(Tier::Ultra));

        // §39's near window is three minutes.
        let near_ms = 3 * 60 * 1000;
        let readings = near_ms / room_poll_ms(Tier::Eco);
        assert!(
            readings >= 20,
            "at Eco the near window gets {readings} readings, which is too few \
             for a median to mean anything — this would be removing the room \
             sense rather than slowing it"
        );
    }

    /// **The interface actually consults the tier when it reads the room.**
    ///
    /// The house pattern, pointed at the one saving this feature set wires. A
    /// number in this table that nothing reads is the failure this codebase
    /// keeps meeting: it would pass every test above and change nothing on a
    /// struggling laptop.
    #[test]
    fn the_room_panel_reads_this_table_rather_than_its_own_number() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/RoomSense.svelte");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read {path}: {e}"))
            .replace("\r\n", "\n");
        // The *call*, with the governor's own tier in it — not merely the
        // identifier. The first version of this test looked for `roomPollMs`
        // anywhere in the file and a mutation replacing the call with a
        // hard-coded `Promise.resolve(2000)` survived it, because the import
        // and the doc comment still spelled the name. A guard that a dead call
        // site satisfies is not a guard.
        assert!(
            source.contains("roomPollMs(tier)"),
            "RoomSense no longer asks djmanzo how often to look, so §48's \
             audience-polling saving is a number nothing reads"
        );
        assert!(
            source.contains("performance.resolved"),
            "RoomSense no longer follows the governor, so the number it asks \
             for cannot change when the machine starts struggling"
        );
        assert!(
            !source.contains("const EVERY_MS = 2000"),
            "RoomSense has its own two seconds again, which is the second \
             description this table exists to prevent"
        );
    }
}

//! Editions: djmanzo sold at different levels, from one codebase.
//!
//! The owner's word (`docs/DIRECTIVE.md` §130): an *Auto-Pilot*, a
//! *Hobbyist*, an *Apprentice*, a *Pro DJ*, a *Karaoke*, the *Full* version,
//! and a *Custom* one built from the Hobbyist foundation plus the modules a
//! DJ picks. Each edition is a separate download (`docs/EDITIONS.md`).
//!
//! # A foundation and modules
//!
//! Every edition has the **foundation**: two decks, the mixer, the library of
//! the DJ's own files, cues, loops and pads, the built-in effects, the
//! sampler, recording, the keyboard, a controller through its shipped
//! mapping, waveforms and themes. That is the Hobbyist edition and is not a
//! [`Module`]: nothing can be sold without it.
//!
//! Everything else is a [`Module`], and an edition is no more than the
//! modules it carries ([`Edition::modules`]). So a new edition is a new row,
//! not a new code path, and "is this here?" is one question with one answer
//! everywhere it is asked — [`Modules::has`].
//!
//! # What is not here
//!
//! Prices, and which edition this build is. Prices are a business decision
//! that changes faster than code (`docs/EDITIONS.md`); the build's edition is
//! chosen where the application is built, one layer up.

use serde::{Deserialize, Serialize};

/// A part of djmanzo that an edition carries or leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Module {
    /// Decks three and four.
    FourDecks,
    /// Separating a record into vocals, drums, bass and the rest, live.
    Stems,
    /// The application running the mix: automix, the autopilot's choices, a
    /// playlist styled and filled for it, and the DJ taking over at any time.
    Autopilot,
    /// The assistant: the rail of what to play next, the whisper, the guides,
    /// the knowledge packs and the DJ talking to it.
    Assistant,
    /// Teaching: lessons, the coach, the practice lab and practice deck, the
    /// quiz, and the road from first mix to first gig.
    Teacher,
    /// Karaoke: the singers' screen, the queue, vocals out, words in time,
    /// the room's sign-up page, the guest book and the microphones.
    Karaoke,
    /// Reading the room: the camera, the microphone's sense of the floor, the
    /// crowd's reactions and what the room did across a mix.
    Crowd,
    /// Controllers beyond a shipped mapping: the mapping editor, HID devices,
    /// motorised platters and an interface that reads what is plugged in.
    Controllers,
    /// Timecode vinyl.
    Dvs,
    /// Video, visuals and the visualiser.
    Video,
    /// Going live: broadcasting, OBS and sharing a set.
    Streaming,
    /// Hosting audio plugins.
    Plugins,
    /// Online music sources: searching, streaming and buying beyond the
    /// DJ's own files.
    Sources,
    /// The working DJ's preparation: events kept step by step, the night's
    /// plan and the press kit.
    Events,
    /// Hearing a set back and rendering it again offline.
    Replay,
}

impl Module {
    /// Every module, in the order a price list shows them.
    pub const ALL: [Self; 15] = [
        Self::FourDecks,
        Self::Stems,
        Self::Autopilot,
        Self::Assistant,
        Self::Teacher,
        Self::Karaoke,
        Self::Crowd,
        Self::Controllers,
        Self::Dvs,
        Self::Video,
        Self::Streaming,
        Self::Plugins,
        Self::Sources,
        Self::Events,
        Self::Replay,
    ];

    /// Its name in settings, build scripts and licence files.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::FourDecks => "four_decks",
            Self::Stems => "stems",
            Self::Autopilot => "autopilot",
            Self::Assistant => "assistant",
            Self::Teacher => "teacher",
            Self::Karaoke => "karaoke",
            Self::Crowd => "crowd",
            Self::Controllers => "controllers",
            Self::Dvs => "dvs",
            Self::Video => "video",
            Self::Streaming => "streaming",
            Self::Plugins => "plugins",
            Self::Sources => "sources",
            Self::Events => "events",
            Self::Replay => "replay",
        }
    }

    /// Its name where a DJ reads it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::FourDecks => "Four decks",
            Self::Stems => "Stems",
            Self::Autopilot => "Auto-pilot",
            Self::Assistant => "Assistant",
            Self::Teacher => "Teacher",
            Self::Karaoke => "Karaoke",
            Self::Crowd => "Reading the room",
            Self::Controllers => "Controllers",
            Self::Dvs => "Timecode vinyl",
            Self::Video => "Video and visuals",
            Self::Streaming => "Going live",
            Self::Plugins => "Plugins",
            Self::Sources => "Online sources",
            Self::Events => "Events",
            Self::Replay => "Replay",
        }
    }

    /// The module a slug names.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.slug() == slug.trim())
    }

    const fn bit(self) -> u32 {
        1 << self as u32
    }
}

/// A set of modules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modules(u32);

impl Modules {
    /// The foundation alone.
    pub const NONE: Self = Self(0);

    /// The modules listed.
    #[must_use]
    pub const fn of(modules: &[Module]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < modules.len() {
            bits |= modules[i].bit();
            i += 1;
        }
        Self(bits)
    }

    /// Every module.
    #[must_use]
    pub const fn all() -> Self {
        Self::of(&Module::ALL)
    }

    /// Whether this set carries `module`.
    #[must_use]
    pub const fn has(self, module: Module) -> bool {
        self.0 & module.bit() != 0
    }

    /// This set with `module` added.
    #[must_use]
    pub const fn with(self, module: Module) -> Self {
        Self(self.0 | module.bit())
    }

    /// Whether every module of `self` is also in `other`.
    #[must_use]
    pub const fn within(self, other: Self) -> bool {
        self.0 & !other.0 == 0
    }

    /// The modules in this set, in [`Module::ALL`]'s order.
    pub fn iter(self) -> impl Iterator<Item = Module> {
        Module::ALL.into_iter().filter(move |m| self.has(*m))
    }

    /// The modules not in this set: what an edition can show as within
    /// reach, never as broken.
    #[must_use]
    pub const fn missing(self) -> Self {
        Self(Self::all().0 & !self.0)
    }

    /// The set a list of slugs names, comma- or space-separated, as a build
    /// script or a licence file writes it.
    ///
    /// # Errors
    /// The first word that names no module.
    pub fn parse(list: &str) -> Result<Self, String> {
        list.split([',', ' '])
            .filter(|word| !word.trim().is_empty())
            .try_fold(Self::NONE, |set, word| {
                Module::from_slug(word)
                    .map(|m| set.with(m))
                    .ok_or_else(|| format!("{:?} is not a djmanzo module", word.trim()))
            })
    }
}

/// An edition of djmanzo, as it is sold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    /// Two decks the application mixes, styled and filled as the DJ asks,
    /// and taken over by hand whenever they like.
    AutoPilot,
    /// Everything a DJ needs to mix, with a look at what more there is.
    Hobbyist,
    /// The Hobbyist's, with a teacher and an assistant: how to DJ, the music
    /// behind it, the crowd, and the road to the first gigs.
    Apprentice,
    /// The Hobbyist's, with karaoke.
    Karaoke,
    /// Everything for the working DJ, without karaoke and the teacher.
    ProDj,
    /// Everything.
    Full,
    /// The Hobbyist's, with the modules the DJ chose
    /// ([`Edition::modules`] gives the foundation alone; the choice is the
    /// licence's).
    Custom,
}

const AUTO_PILOT: Modules = Modules::of(&[Module::Autopilot]);
const APPRENTICE: Modules = Modules::of(&[Module::Teacher, Module::Assistant]);
const KARAOKE: Modules = Modules::of(&[Module::Karaoke]);
const PRO_DJ: Modules = Modules::of(&[
    Module::FourDecks,
    Module::Stems,
    Module::Autopilot,
    Module::Assistant,
    Module::Crowd,
    Module::Controllers,
    Module::Dvs,
    Module::Video,
    Module::Streaming,
    Module::Plugins,
    Module::Sources,
    Module::Events,
    Module::Replay,
]);

impl Edition {
    /// Every edition, cheapest first as the price list shows them.
    pub const ALL: [Self; 7] = [
        Self::AutoPilot,
        Self::Hobbyist,
        Self::Apprentice,
        Self::Karaoke,
        Self::ProDj,
        Self::Full,
        Self::Custom,
    ];

    /// The modules this edition carries on top of the foundation.
    #[must_use]
    pub const fn modules(self) -> Modules {
        match self {
            Self::AutoPilot => AUTO_PILOT,
            Self::Hobbyist | Self::Custom => Modules::NONE,
            Self::Apprentice => APPRENTICE,
            Self::Karaoke => KARAOKE,
            Self::ProDj => PRO_DJ,
            Self::Full => Modules::all(),
        }
    }

    /// Its name in build scripts and settings.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::AutoPilot => "autopilot",
            Self::Hobbyist => "hobbyist",
            Self::Apprentice => "apprentice",
            Self::Karaoke => "karaoke",
            Self::ProDj => "pro",
            Self::Full => "full",
            Self::Custom => "custom",
        }
    }

    /// The edition a slug names.
    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.slug() == slug.trim())
    }

    /// What the application calls itself in this edition.
    #[must_use]
    pub const fn product_name(self) -> &'static str {
        match self {
            Self::AutoPilot => "djmanzo Auto-Pilot",
            Self::Hobbyist => "djmanzo Hobbyist",
            Self::Apprentice => "djmanzo Apprentice",
            Self::Karaoke => "djmanzo Karaoke",
            Self::ProDj => "djmanzo Pro DJ",
            Self::Full => "djmanzo",
            Self::Custom => "djmanzo Custom",
        }
    }

    /// The cheapest edition, other than Custom, that carries `module`: where
    /// a look at something out of reach points.
    #[must_use]
    pub fn cheapest_with(module: Module) -> Self {
        Self::ALL
            .into_iter()
            .filter(|e| *e != Self::Custom)
            .find(|e| e.modules().has(module))
            .unwrap_or(Self::Full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every module has one name and is found by it**, and the bits do not
    /// collide: a set of all fifteen has fifteen.
    #[test]
    fn modules_are_named_once_and_fit_the_set() {
        for module in Module::ALL {
            assert_eq!(Module::from_slug(module.slug()), Some(module));
            assert!(!module.title().is_empty());
            assert_eq!(
                Modules::of(&[module]).iter().collect::<Vec<_>>(),
                vec![module],
                "{module:?} shares its bit"
            );
        }
        assert_eq!(Modules::all().iter().count(), Module::ALL.len());
        assert!(Module::ALL.len() <= 32, "a module past bit 31 does not fit");
    }

    /// **The owner's editions, as asked.** The Hobbyist foundation is in every
    /// one; the Apprentice adds a teacher and an assistant; Karaoke adds
    /// karaoke and nothing else; Pro DJ is everything but karaoke and the
    /// teacher; Full is everything; Auto-Pilot is the foundation's two decks
    /// with the autopilot.
    #[test]
    fn the_editions_carry_what_the_owner_asked() {
        for edition in Edition::ALL {
            assert!(
                Edition::Hobbyist.modules().within(edition.modules()),
                "{edition:?}"
            );
            assert!(edition.modules().within(Edition::Full.modules()));
        }
        assert_eq!(
            Edition::Apprentice.modules().iter().collect::<Vec<_>>(),
            vec![Module::Assistant, Module::Teacher]
        );
        assert_eq!(
            Edition::Karaoke.modules().iter().collect::<Vec<_>>(),
            vec![Module::Karaoke]
        );
        assert_eq!(
            Edition::AutoPilot.modules().iter().collect::<Vec<_>>(),
            vec![Module::Autopilot]
        );
        assert!(!Edition::AutoPilot.modules().has(Module::FourDecks));
        assert_eq!(
            Edition::ProDj
                .modules()
                .missing()
                .iter()
                .collect::<Vec<_>>(),
            vec![Module::Teacher, Module::Karaoke]
        );
        assert_eq!(Edition::Full.modules().missing(), Modules::NONE);
        // Custom starts at the foundation: what it adds is the licence's.
        assert_eq!(Edition::Custom.modules(), Modules::NONE);
    }

    /// **A look at what is out of reach points at the cheapest edition that
    /// has it**, never at Custom.
    #[test]
    fn what_is_out_of_reach_points_at_the_cheapest_edition_with_it() {
        assert_eq!(
            Edition::cheapest_with(Module::Autopilot),
            Edition::AutoPilot
        );
        assert_eq!(Edition::cheapest_with(Module::Teacher), Edition::Apprentice);
        assert_eq!(Edition::cheapest_with(Module::Karaoke), Edition::Karaoke);
        assert_eq!(Edition::cheapest_with(Module::Stems), Edition::ProDj);
        for module in Module::ALL {
            assert_ne!(Edition::cheapest_with(module), Edition::Custom);
            assert!(Edition::cheapest_with(module).modules().has(module));
        }
    }

    /// **A Custom choice is read from its list, and a wrong word is named.**
    #[test]
    fn a_custom_choice_is_read_from_its_list() {
        let chosen = Modules::parse("karaoke, stems four_decks").expect("three modules");
        assert_eq!(
            chosen.iter().collect::<Vec<_>>(),
            vec![Module::FourDecks, Module::Stems, Module::Karaoke]
        );
        assert_eq!(Modules::parse("  "), Ok(Modules::NONE));
        assert_eq!(
            Modules::parse("stems, lasers"),
            Err("\"lasers\" is not a djmanzo module".into())
        );
        for edition in Edition::ALL {
            assert_eq!(Edition::from_slug(edition.slug()), Some(edition));
        }
    }
}

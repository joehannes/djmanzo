//! The design-system core the adaptive cockpit is assembled from.
//!
//! # What this is, and what it deliberately is not
//!
//! `docs/GUI-OVERHAUL.md` is the audit that produced this module. Its central
//! finding was one line of `App.svelte`:
//!
//! ```text
//! let panel = $state<"none" | "browse" | "assistant" | ... >("none")
//! ```
//!
//! One surface at a time, out of eight, with everything else nested inside
//! those eight -- three levels deep in two places. A DJ cannot watch the room
//! and browse for the next record at once, because `RoomSense` lives at
//! `App > Assistant > Conduct > RoomSense` and the browser is a sibling panel.
//!
//! This module is the *vocabulary* for fixing that. It is types and tables. It
//! renders nothing, it changes nothing on screen, and that is the point: the
//! schemas have to exist and be agreed before a single component is rewritten,
//! or the rewrite invents its own private idea of what a surface is.
//!
//! # Why it lives beside [`crate::widgets`] rather than replacing it
//!
//! A **surface** is a widget with placement metadata. The registry in
//! [`crate::widgets`] already validates widget trees, already resolves a
//! layout against a catalog, and already skips what it does not know while
//! *counting* what it skipped. Building a second registry for surfaces would
//! be exactly the mistake [ADR-0008](../../../docs/adr/0008-one-widget-vocabulary.md)
//! exists to prevent -- two vocabularies for one idea, drifting apart at the
//! first disagreement.
//!
//! So this module reuses that machinery rather than duplicating it:
//! [`Token::shape`] extends the shape whitelist the layout loader already
//! enforces, and a surface placement is checked the same way a widget
//! placement is.
//!
//! # The rule everything here serves
//!
//! > The system may change **presentation priority**. It may never change
//! > **semantic control identity**.
//!
//! A Play button stays a Play button; Deck 1 stays Deck 1. Adaptation is
//! allowed to promote, compress, dock and collapse. It is not allowed to
//! rename, reassign, or move a control a DJ aims at without looking.

use serde::{Deserialize, Serialize};

use crate::widgets::TokenShape;

// -- semantic colour roles --------------------------------------------------

/// What a colour *means*, as opposed to what it looks like.
///
/// The 23 tokens that ship are named for appearance -- `accent`,
/// `panel-raised`, `text-dim`. That is enough to recolour an interface and not
/// enough to say anything with colour, because nothing in the name tells a
/// theme author that this particular accent is the one carrying "the incoming
/// deck" and must therefore stay distinguishable from the outgoing one.
///
/// These roles are the missing half. A theme maps each role to a colour; the
/// interface asks for the role. `--incoming` is then guaranteed to differ from
/// `--outgoing` in every theme, because a theme that collapses them fails a
/// test rather than merely looking odd.
///
/// **Colour is never the sole carrier.** `docs/VISUAL-LANGUAGE.md` already
/// requires a redundant channel -- shape, position, pattern, opacity, label --
/// because roughly one man in twelve cannot use hue at all. Naming a role does
/// not exempt it from that; it makes the requirement checkable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// The deck a mix is coming *from*.
    Outgoing,
    /// The deck a mix is going *to*. Must stay distinguishable from
    /// [`Role::Outgoing`] in every theme; a transition is the one moment where
    /// confusing the two is expensive.
    Incoming,
    /// A control the DJ has selected.
    Selected,
    /// A control that is doing something right now.
    Active,
    /// Something the assistant is responsible for -- a staged action, a ghost
    /// marker, a suggestion. Distinct from [`Role::Active`] on purpose: "the
    /// machine did this" and "this is on" are different facts.
    Assistant,
    /// Anything the system is *not sure about*. The most important role here,
    /// and the one most often missing from interfaces that show confidence as
    /// a number: uncertainty needs a look of its own, or a low-confidence
    /// suggestion is presented exactly like a high-confidence one.
    Uncertain,
    /// Something the room did, rather than something the DJ or the assistant
    /// did.
    Audience,
    /// It worked.
    Success,
    /// It will probably be a problem.
    Warning,
    /// It is a problem now.
    Danger,
    StemVocal,
    StemDrums,
    StemBass,
    StemOther,
}

impl Role {
    /// Every role, in the order they are declared.
    pub const ALL: &'static [Role] = &[
        Role::Outgoing,
        Role::Incoming,
        Role::Selected,
        Role::Active,
        Role::Assistant,
        Role::Uncertain,
        Role::Audience,
        Role::Success,
        Role::Warning,
        Role::Danger,
        Role::StemVocal,
        Role::StemDrums,
        Role::StemBass,
        Role::StemOther,
    ];

    /// The custom property this role is published as, without the `--`.
    ///
    /// Kebab-case, matching the tokens already in the stylesheets, so a theme
    /// author reading `docs/VISUAL-LANGUAGE.md` sees one naming convention and
    /// not two.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Role::Outgoing => "outgoing",
            Role::Incoming => "incoming",
            Role::Selected => "selected",
            Role::Active => "active",
            Role::Assistant => "assistant",
            Role::Uncertain => "uncertain",
            Role::Audience => "audience",
            Role::Success => "success",
            Role::Warning => "warn",
            Role::Danger => "danger",
            Role::StemVocal => "stem-vocal",
            Role::StemDrums => "stem-drums",
            Role::StemBass => "stem-bass",
            Role::StemOther => "stem-other",
        }
    }

    /// One line, for a theme editor and for the documentation.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Role::Outgoing => "The deck the mix is coming from.",
            Role::Incoming => "The deck the mix is going to.",
            Role::Selected => "What the DJ has selected.",
            Role::Active => "What is doing something right now.",
            Role::Assistant => "What the assistant is responsible for.",
            Role::Uncertain => "What the system is not sure about.",
            Role::Audience => "What the room did.",
            Role::Success => "It worked.",
            Role::Warning => "It will probably be a problem.",
            Role::Danger => "It is a problem now.",
            Role::StemVocal => "The vocal stem.",
            Role::StemDrums => "The drum stem.",
            Role::StemBass => "The bass stem.",
            Role::StemOther => "Everything that is not vocal, drums or bass.",
        }
    }

    /// Whether two roles are ever shown side by side in a way that requires
    /// them to be told apart.
    ///
    /// Not every pair needs to differ -- `success` and `stem-bass` never appear
    /// in the same decision -- and demanding that all fourteen be mutually
    /// distinguishable would force a theme into fourteen arbitrary hues, which
    /// is the neon-everything failure `docs/VISUAL-LANGUAGE.md` warns against.
    /// These are the pairs where confusion actually costs something.
    #[must_use]
    pub fn must_differ_from(self, other: Role) -> bool {
        const GROUPS: &[&[Role]] = &[
            // The transition. Confusing these is confusing which record is
            // leaving, mid-mix.
            &[Role::Outgoing, Role::Incoming],
            // The four stems, which are read together as a set of four.
            &[
                Role::StemVocal,
                Role::StemDrums,
                Role::StemBass,
                Role::StemOther,
            ],
            // Who did it, and how sure anybody is.
            &[Role::Assistant, Role::Uncertain, Role::Audience],
            // The severity ladder.
            &[Role::Success, Role::Warning, Role::Danger],
            // State, which sits next to itself constantly.
            &[Role::Selected, Role::Active],
        ];
        self != other
            && GROUPS
                .iter()
                .any(|group| group.contains(&self) && group.contains(&other))
    }
}

// -- density ----------------------------------------------------------------

/// How much the interface tries to fit on a screen.
///
/// One scale, not forty font sizes. The existing `--density` custom property is
/// already a single multiplier over a root font size that every other
/// measurement is in `em` of, which is the right mechanism; this names the
/// points along it so a workspace can say "Pro Dense" rather than "1.15".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    /// Learning, or a projector at the back of a room.
    Relaxed,
    Standard,
    Compact,
    /// A professional who knows where everything is.
    ProDense,
    /// Everything, at the cost of comfort. For a big screen and a DJ who asked.
    UltraDense,
}

impl Density {
    pub const ALL: &'static [Density] = &[
        Density::Relaxed,
        Density::Standard,
        Density::Compact,
        Density::ProDense,
        Density::UltraDense,
    ];

    /// The multiplier this density puts on the root font size.
    ///
    /// The range is the one the flat `Layout` already clamps to (0.8..=1.4), so
    /// a density profile cannot ask for something the interface has never been
    /// drawn at. Monotonically decreasing: denser means smaller.
    /// The densest band that still fits a window this tall, in CSS pixels.
    ///
    /// **The numbers come from a measurement, not from taste.** At Standard, a
    /// deck column with two records loaded is 807 px; the top bar takes 138 and
    /// the master strip 110, so the whole first screen is about 1,075. A window
    /// shorter than that cannot show it, and the only honest options are to
    /// scale it down or to let a performing control fall off the bottom -- and
    /// djmanzo has let a performing control fall off the bottom three times.
    ///
    /// Bands rather than a continuous ratio, because a layout that resizes by a
    /// few pixels on every window drag is a layout a DJ cannot learn. Each step
    /// is a place the interface settles.
    ///
    /// This is only what djmanzo picks when nobody has said otherwise. A
    /// density named by a layout or a workspace wins: the interface adapts to
    /// the DJ, which means it stops adapting the moment the DJ decides.
    #[must_use]
    pub fn fitting(height: u16) -> Self {
        BANDS
            .iter()
            .find(|(least, _)| height >= *least)
            .map_or(Density::UltraDense, |(_, density)| *density)
    }

    #[must_use]
    pub const fn scale(self) -> f32 {
        match self {
            Density::Relaxed => 1.15,
            Density::Standard => 1.0,
            Density::Compact => 0.92,
            Density::ProDense => 0.86,
            Density::UltraDense => 0.8,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Density::Relaxed => "Relaxed",
            Density::Standard => "Standard",
            Density::Compact => "Compact",
            Density::ProDense => "Pro Dense",
            Density::UltraDense => "Ultra Dense",
        }
    }
}

// -- motion -----------------------------------------------------------------

/// How much the interface is allowed to move.
///
/// [ADR-0009](../../../docs/adr/0009-the-living-interface.md) states the rule
/// this enumerates: *stillness is the default; motion is information*. A level
/// here is a ceiling on how much motion may be spent, never an instruction to
/// spend it.
///
/// `prefers-reduced-motion` pins this at [`Motion::None`] and the DJ cannot be
/// talked out of it by a workspace preset -- an accessibility preference is not
/// a default to be overridden by a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Motion {
    /// Nothing moves. Every state still readable from form, position and
    /// colour -- ADR-0009's tier 0, which is a hard requirement rather than a
    /// courtesy.
    None,
    /// State changes only: something appearing, arriving, or going wrong.
    Low,
    Normal,
    /// Beat-locked pulse and flow, for a machine that can afford it.
    High,
}

impl Motion {
    pub const ALL: &'static [Motion] = &[Motion::None, Motion::Low, Motion::Normal, Motion::High];

    /// What a level is for, in one line.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Motion::None => "Nothing moves; every state is readable from a still frame.",
            Motion::Low => "Only arrivals, departures and warnings move.",
            Motion::Normal => "Transitions and continuous readings move.",
            Motion::High => "Motion locked to the audio clock.",
        }
    }
}

// -- attention --------------------------------------------------------------

/// How much the interface may ask of the DJ right now.
///
/// The idea `docs/GUI-OVERHAUL.md` §18 asks for, made explicit so it can be
/// enforced rather than hoped for. During a transition a DJ has moments; a
/// suggestion that arrives then is not help.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Attention {
    /// Controls the context rail may promote at once.
    pub promoted_controls: u8,
    /// Suggestions that may be on screen at once.
    pub suggestions: u8,
    /// Transient notices that may be on screen at once.
    pub notices: u8,
    /// Whether surfaces may be rearranged without the DJ asking.
    ///
    /// False during a mix, always. Moving a panel while somebody is reaching
    /// for it is the failure that makes adaptive interfaces feel hostile.
    pub reflow: bool,
    pub motion: Motion,
}

impl Attention {
    /// Mixing. The DJ has moments, not minutes.
    #[must_use]
    pub const fn performing() -> Self {
        Self {
            promoted_controls: 6,
            suggestions: 1,
            notices: 1,
            reflow: false,
            motion: Motion::Low,
        }
    }

    /// Between records, or before the night starts.
    #[must_use]
    pub const fn preparing() -> Self {
        Self {
            promoted_controls: 8,
            suggestions: 5,
            notices: 3,
            reflow: true,
            motion: Motion::Normal,
        }
    }

    /// Practising, where an explanation is the point.
    #[must_use]
    pub const fn learning() -> Self {
        Self {
            promoted_controls: 8,
            suggestions: 3,
            notices: 3,
            reflow: true,
            motion: Motion::Normal,
        }
    }

    /// Something is wrong and the DJ needs the controls, not the advice.
    #[must_use]
    pub const fn emergency() -> Self {
        Self {
            promoted_controls: 4,
            suggestions: 0,
            notices: 1,
            reflow: false,
            motion: Motion::None,
        }
    }

    /// The budget for the frame about to be shown.
    ///
    /// The one place this is decided. [§11](../../../docs/DIRECTIVE.md) asks
    /// for a context engine that is the *common* input to the adaptive
    /// interface rather than a rule copied into every panel, and an attention
    /// budget each surface worked out for itself would be four rules that
    /// disagree at exactly the moment they matter.
    ///
    /// The order is the order of severity, and it is not negotiable:
    ///
    /// 1. **Something is broken.** The recording has failed, or the headphone
    ///    card has stopped taking audio. Nothing may reflow and nothing may
    ///    suggest; the DJ needs the controls.
    /// 2. **Two records are audible.** A mix is happening. §18's rule that the
    ///    interface may not move while somebody is reaching for it is exactly
    ///    this case.
    /// 3. **The night is at its peak**, and something more than a guess says
    ///    so. A DJ at peak time has moments between records too — but only
    ///    where the read is worth acting on, which is what
    ///    `dj_core::Certainty` is for.
    /// 4. Otherwise there is room to think.
    #[must_use]
    pub fn for_context(snapshot: &crate::Snapshot) -> Self {
        if failing(snapshot) {
            return Self::emergency();
        }
        if audible(snapshot) >= 2 {
            return Self::performing();
        }
        let peaking = snapshot.context.session.is_some_and(|read| {
            read.phase == dj_core::SessionPhase::Peak && read.certainty >= dj_core::Certainty::Fair
        });
        if peaking {
            return Self::performing();
        }
        Self::preparing()
    }
}

/// Whether something is wrong enough to want the advice to stop.
///
/// Both of these are failures the DJ can act on and neither is a counter that
/// only goes up: an xrun count is a fact about the whole night, and an
/// interface that went into emergency at the first one and stayed there would
/// have said nothing useful about the second.
fn failing(snapshot: &crate::Snapshot) -> bool {
    snapshot.master.recording.failed
        || snapshot
            .master
            .split_output
            .as_ref()
            .is_some_and(|split| !split.healthy)
}

/// How many decks the room can hear.
fn audible(snapshot: &crate::Snapshot) -> usize {
    snapshot
        .decks
        .iter()
        .filter(|deck| deck.playing && deck.volume > 0.01)
        .count()
}

// -- surfaces ---------------------------------------------------------------

/// What kind of work a surface is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Performance,
    Library,
    Planning,
    Assistant,
    Utility,
}

/// The window heights each density band starts at, tallest first.
///
/// **Every number is derived, not chosen.** A deck column with two records
/// loaded was measured at each density, and the band starts at the window
/// height where that deck plus the chrome around it -- a 101 px top bar and the
/// 110 px master strip, both pinned -- actually fits:
///
/// | density | deck | needs |
/// |---|---|---|
/// | Relaxed 1.15 | 1088 px | 1500 |
/// | Standard 1.00 | 807 | 1130 |
/// | Compact 0.92 | 758 | 1060 |
/// | Pro Dense 0.86 | 721 | 1020 |
/// | Ultra Dense 0.80 | 685 | 956 |
///
/// The Relaxed and Standard floors moved up once already -- 1,330 to 1,460 and
/// 1,050 to 1,090 -- when the deck's channel strip was pinned. The deck was
/// measured whole before, and a pinned foot changes the sum: the scrolling body
/// competes with a foot that will not shrink, so a band needs more window than
/// the deck's own height suggests.
///
/// **Then every floor moved again, by exactly 40.** Adding one destination to
/// the top bar -- the set plan -- pushed that row onto one more wrapped line,
/// and the top bar is pinned, so the forty pixels came straight out of every
/// stage at every window height. It is worth saying plainly what that means:
/// *the number in this table is a fact about the top bar as much as about the
/// deck*, and the top bar grows every time a surface is added. Do not guess the
/// correction. `density.spec.ts` sweeps window heights and reports, for each,
/// how much the deck needs and how much it has; the floor is the height where
/// that shortfall reaches zero, and it takes one run to read it off.
///
/// The alternative -- capping the destinations to one scrolling line so the top
/// bar's height stops depending on how many surfaces exist -- was considered and
/// not taken. That row wraps *by design*: it breaks at a group boundary so the
/// seven panels hold one line, and the products this competes with are the ones
/// whose standing complaint is menus you cannot find. A table that needs
/// re-measuring occasionally is a smaller cost than a destination behind a
/// scroll.
///
/// The first version of this table was guessed round numbers, and the guesses
/// were wrong in both directions: a 1,200 px window was given Relaxed, whose
/// deck needs 1,330, and a 900 px window was given Pro Dense, which needs 980.
/// Both clipped the deck's channel strip -- the exact failure the bands exist
/// to prevent -- and a screenshot of the running application is what showed it.
///
/// **Below about 956 px nothing fits**, and the last band is the floor rather
/// than a solution: at djmanzo's own default 800 the deck's channel strip is
/// still in the part of the stage that scrolls. That is recorded as a failing
/// test rather than papered over here.
///
/// Published rather than kept private, so the interface can apply the rule
/// without asking on every resize -- one call at start-up, then arithmetic.
/// Rust still owns the policy; the browser owns the pixels it is measured in.
pub const BANDS: &[(u16, Density)] = &[
    (1500, Density::Relaxed),
    (1130, Density::Standard),
    (1060, Density::Compact),
    (1020, Density::ProDense),
    (0, Density::UltraDense),
];

/// Where a surface can be put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dock {
    Left,
    Right,
    Bottom,
    /// Over the performance zone, dismissed by the next thing the DJ does.
    Overlay,
    /// Its own window, on another screen.
    Detached,
}

/// A panel the cockpit can show.
///
/// Deliberately the same shape as [`crate::widgets::Widget`]: a name, what it
/// is, where it may go, and what it costs. The two are checked by the same
/// rules and a surface is placed by the same resolver.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Surface {
    /// The stable name. A compatibility surface the moment a workspace file
    /// mentions it, exactly like a widget name or an action name.
    pub name: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    pub category: Category,
    /// Smallest size at which this is still usable, in CSS pixels. Below it the
    /// surface collapses rather than clipping -- clipping is the bug class this
    /// project has shipped twice.
    pub least: (u16, u16),
    /// The size to open it at.
    pub prefer: (u16, u16),
    /// Higher wins when two surfaces want the same room.
    pub priority: u8,
    /// True when this must never be closed or displaced by adaptation.
    pub performance_critical: bool,
    pub detachable: bool,
    pub stackable: bool,
    pub collapsible: bool,
    /// True when the context engine may open this on its own.
    pub contextual: bool,
    /// Where it opens when nothing has said otherwise.
    ///
    /// Here rather than in the interface because two places deciding where a
    /// panel lands is two places that eventually disagree — and one of them
    /// would be `uiop`, opening a surface somewhere the DJ's own button never
    /// puts it. Always one of [`Self::docks`]; asserted by test.
    pub home: Dock,
    /// Docks this may be placed in.
    pub docks: &'static [Dock],
}

const ANY_DOCK: &[Dock] = &[
    Dock::Left,
    Dock::Right,
    Dock::Bottom,
    Dock::Overlay,
    Dock::Detached,
];
const SIDE_OR_BOTTOM: &[Dock] = &[Dock::Left, Dock::Right, Dock::Bottom, Dock::Detached];
const SIDE: &[Dock] = &[Dock::Left, Dock::Right, Dock::Detached];

/// Every surface the cockpit can show.
///
/// Sixteen of these exist as components today; the rest are views over data the
/// engine already produces. The audit's point stands here in table form: this
/// is not a list of things to build, it is a list of things that already work
/// and cannot currently be looked at together.
#[must_use]
pub fn surfaces() -> &'static [Surface] {
    &[
        Surface {
            name: "library",
            title: "Library",
            about: "The collection, searchable and sortable.",
            category: Category::Library,
            least: (420, 200),
            prefer: (900, 380),
            priority: 60,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "prepare",
            title: "Prepare",
            about: "Records on their way to a deck, before they are on one.",
            category: Category::Library,
            least: (260, 160),
            prefer: (360, 380),
            priority: 65,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "next",
            title: "Next",
            about: "What could come next, and why.",
            category: Category::Assistant,
            least: (260, 90),
            prefer: (360, 220),
            priority: 80,
            performance_critical: false,
            detachable: false,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "plan",
            title: "Set plan",
            about: "The shape of the night, as a sequence.",
            category: Category::Planning,
            least: (420, 200),
            prefer: (760, 320),
            priority: 40,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "pair",
            title: "Pair",
            about: "Two records side by side, and the seam between them.",
            category: Category::Planning,
            least: (420, 240),
            prefer: (820, 360),
            priority: 45,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "room",
            title: "Room",
            about: "What the floor is doing, against its own earlier state.",
            category: Category::Assistant,
            least: (200, 80),
            prefer: (300, 200),
            priority: 75,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: ANY_DOCK,
        },
        Surface {
            name: "night",
            title: "The night",
            about: "Where the set is in its arc, and what says so.",
            category: Category::Assistant,
            least: (220, 96),
            prefer: (320, 200),
            priority: 70,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: ANY_DOCK,
        },
        Surface {
            name: "requests",
            title: "Requests",
            about: "What the room has asked for.",
            category: Category::Assistant,
            least: (240, 120),
            prefer: (340, 300),
            priority: 55,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "assistant",
            title: "Assistant",
            about: "The second DJ, when spoken to directly.",
            category: Category::Assistant,
            least: (320, 200),
            prefer: (420, 420),
            priority: 45,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "stems",
            title: "Stems",
            about: "The four parts of a record, as performance controls.",
            category: Category::Performance,
            least: (280, 120),
            prefer: (380, 220),
            priority: 85,
            performance_critical: true,
            detachable: false,
            stackable: false,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "fx",
            title: "Effects",
            about: "The rack, its slots and their timing.",
            category: Category::Performance,
            least: (280, 100),
            prefer: (380, 180),
            priority: 82,
            performance_critical: true,
            detachable: false,
            stackable: false,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "sampler",
            title: "Sampler",
            about: "Banks, pads and what is loaded on them.",
            category: Category::Performance,
            least: (300, 160),
            prefer: (420, 280),
            priority: 70,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "practice",
            title: "Practice",
            // §69's word is "sandbox", and the whole of it is the second half
            // of that sentence: *without altering the live master*. What ships
            // is the part that can be done honestly offline -- hearing the
            // mix, and hearing the alternatives -- so this is a list of
            // renders rather than the full laboratory the first draft of this
            // entry was sized for.
            about: "Hear a transition before you play it, without touching the decks.",
            category: Category::Planning,
            least: (360, 200),
            prefer: (620, 360),
            priority: 30,
            performance_critical: false,
            detachable: true,
            // Beside the pair view, which is the same two records asked a
            // different question, so it stacks and collapses the way that one
            // does rather than demanding a dock to itself.
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "transition",
            title: "Transition",
            about: "One transition, examined: where, how long, and what happens.",
            category: Category::Planning,
            least: (420, 240),
            prefer: (760, 400),
            priority: 50,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "journal",
            title: "Journal",
            about: "Notes attached to a moment.",
            category: Category::Planning,
            least: (240, 140),
            prefer: (340, 320),
            priority: 25,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            // Not "rail": §22's Next rail is already the rail here, and two
            // things with one name is a session lost to reading the wrong one.
            name: "athand",
            title: "At hand",
            about: "The four to eight controls that matter on the focused deck right now.",
            category: Category::Performance,
            // Small on purpose: §74 calls it *compact*, and a rail that needs
            // a third of the screen is a panel.
            least: (200, 90),
            prefer: (300, 140),
            // High, because it is only worth having where it can be reached
            // without looking — which means it must not be the surface that
            // gets collapsed when room runs short.
            priority: 85,
            performance_critical: true,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: true,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "mixes",
            title: "Tonight's mixes",
            // Deliberately not History's question. That one answers *which
            // records* were played and persists across nights; this answers
            // how tonight's were joined, which is a fact about the set rather
            // than about the collection -- §67's "a session contains
            // transitions", read back out of the session's own log.
            about: "How tonight's records were joined, and what kind of mix each was.",
            category: Category::Planning,
            least: (260, 140),
            prefer: (380, 320),
            priority: 30,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "history",
            title: "History",
            about: "What has been played tonight.",
            category: Category::Library,
            least: (240, 140),
            prefer: (340, 320),
            priority: 35,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "memory",
            title: "From memory",
            about: "Find a record from a line, a description or a hum.",
            category: Category::Library,
            least: (320, 200),
            prefer: (420, 420),
            priority: 20,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "controllers",
            title: "Controllers",
            about: "What is plugged in and what is listening to it.",
            category: Category::Utility,
            least: (320, 200),
            prefer: (460, 400),
            priority: 15,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        // The three the audit's list did not have, added when the dock manager
        // was built and the shell's panels were counted against it. A panel
        // the model cannot name is a panel the dock manager cannot place, and
        // Phase 2's gate is that no feature becomes unreachable.
        Surface {
            name: "booth",
            title: "Booth",
            about: "The microphone, the automix, a plugin insert and the master effects -- the things set up once a night rather than reached for during a mix.",
            category: Category::Utility,
            least: (420, 200),
            prefer: (900, 320),
            priority: 30,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "presets",
            title: "Presets",
            about: "Effect chains and mix settings, saved and recalled.",
            category: Category::Performance,
            least: (320, 200),
            prefer: (520, 420),
            priority: 35,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Right,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "keys",
            title: "Keys",
            about: "The keyboard shortcuts, and whether they are listening.",
            category: Category::Utility,
            least: (320, 240),
            prefer: (480, 460),
            priority: 5,
            performance_critical: false,
            detachable: false,
            stackable: false,
            collapsible: false,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
        Surface {
            name: "log",
            title: "Session log",
            about: "Every action, in order, with its timestamp -- the thing that makes a set replayable.",
            category: Category::Utility,
            least: (420, 160),
            prefer: (900, 300),
            priority: 5,
            performance_critical: false,
            detachable: true,
            stackable: true,
            collapsible: true,
            contextual: false,
            home: Dock::Bottom,
            docks: SIDE_OR_BOTTOM,
        },
        Surface {
            name: "settings",
            title: "Settings",
            about: "Everything that is a preference rather than a performance.",
            category: Category::Utility,
            least: (420, 300),
            prefer: (620, 560),
            priority: 10,
            performance_critical: false,
            detachable: true,
            stackable: false,
            collapsible: false,
            contextual: false,
            home: Dock::Right,
            docks: SIDE,
        },
    ]
}

/// Look one up by name.
#[must_use]
pub fn surface(name: &str) -> Option<&'static Surface> {
    surfaces().iter().find(|known| known.name == name)
}

/// A surface, placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub surface: String,
    pub dock: Dock,
    /// Order within the dock, low first.
    #[serde(default)]
    pub order: i32,
    /// Size along the dock's axis, in CSS pixels. `None` takes the surface's
    /// own preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u16>,
    #[serde(default)]
    pub collapsed: bool,
    /// Pinned surfaces are never moved, resized or closed by adaptation. This
    /// is the per-surface half of "freeze layout".
    #[serde(default)]
    pub pinned: bool,
}

// -- workspaces -------------------------------------------------------------

/// What the DJ is doing, which decides what gets promoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Focus {
    /// Mixing. Tier 1 and 2 only; nothing else may take room.
    Performing,
    /// Choosing and readying records.
    Preparing,
    /// Planning the shape of a night.
    Planning,
    /// Practising, where explanation is the point.
    Learning,
    /// Watching an assisted mix, ready to take it back.
    Supervising,
}

impl Focus {
    #[must_use]
    pub const fn attention(self) -> Attention {
        match self {
            Focus::Performing | Focus::Supervising => Attention::performing(),
            Focus::Preparing | Focus::Planning => Attention::preparing(),
            Focus::Learning => Attention::learning(),
        }
    }
}

/// A saved arrangement of the cockpit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub surfaces: Vec<Placement>,
    pub density: Density,
    pub focus: Focus,
    /// The theme's name. The theme itself is a pack, not part of the workspace.
    #[serde(default)]
    pub theme: String,
    /// Decks on screen.
    pub decks: u8,
    /// When true, adaptation may not move anything -- the professional safety
    /// valve. The context engine still runs and still suggests; it simply may
    /// not rearrange.
    #[serde(default)]
    pub frozen: bool,
}

/// The arrangements that ship.
///
/// [§7](../../../docs/DIRECTIVE.md) names twenty-four and says the thing that
/// matters more than the list: make them **starting points, not rigid
/// identities**, and keep every one editable. So applying one is a
/// rearrangement like any other — the DJ moves what they like afterwards and
/// `set_cockpit_workspace` keeps *that*, under their own name if they give it
/// one. Nothing here is a mode djmanzo puts itself into.
///
/// **This was three for a long time**, on the argument that a DJ arrives with
/// a way of working rather than a vocabulary of workspace names. The argument
/// is a good one and the directive overrules it: §7 lists the twenty-four by
/// name, and where an implementation and that file disagree, that file wins.
/// The three that were here are still here, and "Perform" is still first
/// because [`opening`] takes the first.
///
/// **The first one is empty on purpose.** Performing means the decks and
/// nothing else; a surface that opens itself while somebody is mixing is the
/// failure mode this whole redesign exists to avoid.
///
/// # The one of §7's twenty-four that is not here
///
/// **VJ / Visual Performance.** djmanzo has no visual surface — no video
/// output, no visualiser, nothing to place — so a workspace by that name would
/// arrange the same panels as any other and lie about it in the picker. It is
/// absent rather than approximated, and a test below says so by name.
#[must_use]
pub fn workspaces() -> Vec<Workspace> {
    vec![
        Workspace {
            name: "Perform".to_owned(),
            about: "The decks and nothing else.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Beginner".to_owned(),
            about: "Everything named, nothing hidden, and the coach talking.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "practice".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "athand".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Relaxed,
            focus: Focus::Learning,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Classic DJ".to_owned(),
            about: "Two decks and a mixer, the way it has always been.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Pro Performance".to_owned(),
            about: "Two decks, tight, with what is next and the booth to hand.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "next".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "booth".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::ProDense,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "4 Deck".to_owned(),
            about: "Four decks, close together.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 4,
            frozen: false,
        },
        Workspace {
            name: "6 Deck".to_owned(),
            about: "Every deck djmanzo has, at the cost of comfort.".to_owned(),
            surfaces: Vec::new(),
            density: Density::UltraDense,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 6,
            frozen: false,
        },
        Workspace {
            name: "Club".to_owned(),
            about: "Four decks, the night's arc, and what is under your hands.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "night".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "athand".to_owned(),
                    dock: Dock::Right,
                    order: 1,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "pkg-booth".to_owned(),
            decks: 4,
            frozen: false,
        },
        Workspace {
            name: "Mobile DJ".to_owned(),
            about: "The collection and the booth, for a room that talks back.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "booth".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Wedding / Event".to_owned(),
            about: "The collection, the requests inside it, and what kind of night it is."
                .to_owned(),
            surfaces: vec![
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "night".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Preparing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Open Format".to_owned(),
            about: "Four decks and the whole collection, for a night that goes anywhere."
                .to_owned(),
            surfaces: vec![
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "next".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Compact,
            focus: Focus::Preparing,
            theme: "".to_owned(),
            decks: 4,
            frozen: false,
        },
        Workspace {
            name: "Latin / Caribbean".to_owned(),
            about:
                "What comes next and where the night is, for a set that crosses genres on purpose."
                    .to_owned(),
            surfaces: vec![
                Placement {
                    surface: "next".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "night".to_owned(),
                    dock: Dock::Right,
                    order: 1,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "pkg-sunset".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Scratch / Turntablism".to_owned(),
            about: "Two decks and the controllers, with the hands in charge.".to_owned(),
            surfaces: vec![Placement {
                surface: "controllers".to_owned(),
                dock: Dock::Right,
                order: 0,
                size: None,
                collapsed: false,
                pinned: false,
            }],
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Stem Performance".to_owned(),
            about: "The decks alone, so the stem controls and the rack on them have room."
                .to_owned(),
            surfaces: Vec::new(),
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Mashup / Remix".to_owned(),
            about: "Four decks and the sampler, with the stem controls on each deck.".to_owned(),
            surfaces: vec![Placement {
                surface: "sampler".to_owned(),
                dock: Dock::Bottom,
                order: 0,
                size: None,
                collapsed: false,
                pinned: false,
            }],
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 4,
            frozen: false,
        },
        Workspace {
            name: "Preparation".to_owned(),
            about: "The library beside the decks, with what is coming next.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "prepare".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Compact,
            focus: Focus::Preparing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Set Planning".to_owned(),
            about: "The shape of the night, and the records to build it from.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "plan".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Bottom,
                    order: 1,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Planning,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Practice / Learning".to_owned(),
            about: "A mix rehearsed offline, with the pair it is between.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "practice".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "pair".to_owned(),
                    dock: Dock::Bottom,
                    order: 1,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Learning,
            theme: "pkg-studio".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Autopilot".to_owned(),
            about: "The booth and the assistant, while it drives and you watch.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "booth".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "assistant".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Supervising,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Read the room".to_owned(),
            about: "The library and the assistant at the same time -- the thing the old \
                     shell could not do."
                .to_owned(),
            surfaces: vec![
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "assistant".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Compact,
            focus: Focus::Preparing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Minimal".to_owned(),
            about: "The decks, given room to breathe.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Relaxed,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "High Contrast".to_owned(),
            about: "The decks, in the theme built for a dark booth and a bright screen.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "pkg-booth".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Laptop Compact".to_owned(),
            about: "Everything that fits on a small screen, and nothing that does not.".to_owned(),
            surfaces: Vec::new(),
            density: Density::UltraDense,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Controller Focus".to_owned(),
            about: "The decks and what is mapped to the hardware in front of them.".to_owned(),
            surfaces: vec![Placement {
                surface: "controllers".to_owned(),
                dock: Dock::Right,
                order: 0,
                size: None,
                collapsed: false,
                pinned: false,
            }],
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "CDJ / External Mixer".to_owned(),
            about: "The decks and the booth, for a mixer djmanzo is not.".to_owned(),
            surfaces: vec![Placement {
                surface: "booth".to_owned(),
                dock: Dock::Bottom,
                order: 0,
                size: None,
                collapsed: false,
                pinned: false,
            }],
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
        Workspace {
            name: "Karaoke / MC".to_owned(),
            about: "The microphone in the booth and the collection the requests are in.".to_owned(),
            surfaces: vec![
                Placement {
                    surface: "booth".to_owned(),
                    dock: Dock::Bottom,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                Placement {
                    surface: "library".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
            ],
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            frozen: false,
        },
    ]
}

/// The one djmanzo opens with when nothing has been stored.
#[must_use]
pub fn opening() -> Workspace {
    workspaces()
        .into_iter()
        .next()
        .expect("djmanzo ships workspaces")
}

/// What was wrong with a workspace, and what was done about it.
///
/// The same posture as the widget resolver: nothing here is fatal, because a
/// DJ opening their laptop before a set needs an interface rather than a
/// dialog. Every correction is reported so the half that did not load can be
/// seen rather than inferred.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolved {
    pub workspace: Workspace,
    pub notes: Vec<String>,
}

/// Bring a workspace into what the cockpit can actually draw.
///
/// Clamped and skipped rather than refused -- a workspace is a preference, and
/// a DJ whose file names a surface this build does not have wants the rest of
/// their layout, not an error.
#[must_use]
pub fn resolve(workspace: &Workspace) -> Resolved {
    let mut notes = Vec::new();
    let mut out = workspace.clone();

    if out.name.trim().is_empty() {
        out.name = "Custom".to_owned();
    }

    // Two, four or six -- the counts the deck grid has a shape for, matching
    // `Layout::sane` so the two systems cannot disagree about what is drawable.
    out.decks = match out.decks {
        0..=3 => 2,
        4..=5 => 4,
        _ => 6,
    };

    let mut kept: Vec<Placement> = Vec::new();
    for placement in &out.surfaces {
        let Some(known) = surface(&placement.surface) else {
            notes.push(format!(
                "no surface called `{}`, so it was skipped -- a workspace from a \
                 newer djmanzo opens on this one without it",
                placement.surface
            ));
            continue;
        };
        if !known.docks.contains(&placement.dock) {
            notes.push(format!(
                "`{}` cannot go in the {:?} dock, so it was skipped",
                known.name, placement.dock
            ));
            continue;
        }
        if kept.iter().any(|other| other.surface == placement.surface) {
            notes.push(format!(
                "`{}` was placed twice; the first placement was kept",
                known.name
            ));
            continue;
        }

        let mut placement = placement.clone();
        if let Some(size) = placement.size {
            let least = match placement.dock {
                Dock::Bottom => known.least.1,
                _ => known.least.0,
            };
            if size < least {
                notes.push(format!(
                    "`{}` was given {size}px, below the {least}px it needs to be \
                     usable, so it was opened at {least}px instead",
                    known.name
                ));
                placement.size = Some(least);
            }
        }
        if placement.collapsed && !known.collapsible {
            notes.push(format!("`{}` cannot be collapsed", known.name));
            placement.collapsed = false;
        }
        kept.push(placement);
    }

    kept.sort_by_key(|placement| placement.order);
    out.surfaces = kept;

    Resolved {
        workspace: out,
        notes,
    }
}

// -- tokens -----------------------------------------------------------------

/// The semantic tokens, with the shape each value must take.
///
/// Appended to [`crate::widgets::TOKENS`] rather than replacing it: the 23 that
/// ship are in every stylesheet and every DJ's saved layout, and renaming them
/// would break both for no gain. These are the *meanings* the appearance tokens
/// never had.
#[must_use]
pub fn semantic_tokens() -> Vec<(&'static str, TokenShape)> {
    Role::ALL
        .iter()
        .map(|role| (role.token(), TokenShape::Colour))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame with `decks` decks audible.
    fn frame(decks: u8) -> crate::Snapshot {
        let registry = dj_control::ParameterRegistry::new();
        registry.set(
            dj_core::ParamId::Global(dj_core::param::GlobalParam::SampleRate),
            48_000.0,
        );
        for number in 1..=decks {
            let deck = dj_core::DeckId::from_human(number).expect("a deck");
            registry.set(
                dj_core::ParamId::Deck(deck, dj_core::param::DeckParam::Playing),
                1.0,
            );
            registry.set(
                dj_core::ParamId::Deck(deck, dj_core::param::DeckParam::Volume),
                1.0,
            );
        }
        crate::Snapshot::capture(&registry, 2).with_session(None)
    }

    /// One budget, decided in one place, in order of severity.
    ///
    /// §18's rule is the second row and it is the one that matters: while two
    /// records are audible the interface may not move, because somebody is
    /// reaching for it.
    #[test]
    fn the_attention_budget_follows_the_context() {
        assert_eq!(frame(0).attention, Attention::preparing());
        assert!(frame(0).attention.reflow);

        let mixing = frame(2);
        assert_eq!(mixing.attention, Attention::performing());
        assert!(!mixing.attention.reflow, "the interface may reflow mid-mix");

        let mut broken = frame(0);
        broken.master.recording.failed = true;
        let broken = broken.with_session(None);
        assert_eq!(broken.attention, Attention::emergency());
        assert_eq!(broken.attention.suggestions, 0);
    }

    /// Peak time earns a performing budget — but only on a read worth acting
    /// on, which is the other half of §9.
    #[test]
    fn peak_time_only_narrows_the_budget_when_the_read_is_worth_it() {
        let peak = |certainty| {
            Some(dj_core::SessionRead {
                phase: dj_core::SessionPhase::Peak,
                energy: 0.9,
                environment: dj_core::EnvironmentContext::default(),
                certainty,
                basis: dj_core::Basis::Agreed,
                drift: None,
            })
        };
        assert_eq!(
            frame(0)
                .with_session(peak(dj_core::Certainty::Sure))
                .attention,
            Attention::performing()
        );
        assert_eq!(
            frame(0)
                .with_session(peak(dj_core::Certainty::Unsure))
                .attention,
            Attention::preparing(),
            "narrowed the interface on a read nothing agreed with"
        );
    }

    /// **Where a surface opens has to be somewhere it may be.**
    ///
    /// The resolver drops a placement in a dock the surface does not allow,
    /// with a note — so a home that broke this rule would make "open the rail"
    /// open nothing at all, quietly. It was a table in `App.svelte` before §41
    /// needed a second copy of it.
    #[test]
    fn every_surface_opens_somewhere_it_is_allowed_to_be() {
        for surface in surfaces() {
            assert!(
                surface.docks.contains(&surface.home),
                "{} opens in {:?}, which is not one of its docks",
                surface.name,
                surface.home
            );
            // And never into the overlay or another screen by default: both
            // are deliberate choices a DJ makes, not places things land.
            assert!(
                !matches!(surface.home, Dock::Overlay | Dock::Detached),
                "{} opens detached or over the decks by default",
                surface.name
            );
        }
    }

    /// A surface djmanzo can place has to be one the browser knows the name
    /// of, and the contextual ones are the ones the engine may open.
    #[test]
    fn the_night_is_a_surface_the_context_engine_may_open() {
        let night = surface("night").expect("the night is a surface");
        assert!(night.contextual);
        assert!(!night.performance_critical);
        assert_eq!(night.category, Category::Assistant);
    }

    /// The bands have to be usable as a lookup: ordered, and total.
    #[test]
    fn the_density_bands_descend_and_cover_every_window() {
        let mut previous = u16::MAX;
        for (least, _) in BANDS {
            assert!(*least < previous, "the bands are not in descending order");
            previous = *least;
        }
        assert_eq!(
            BANDS.last().map(|(least, _)| *least),
            Some(0),
            "the last band must start at zero, or a short window matches nothing"
        );
    }

    /// The rule djmanzo actually applies, at the sizes it actually opens at.
    ///
    /// The boundaries are a measurement, not a round number picked for looking
    /// tidy: at Standard a deck column with two records loaded is 807 px, the
    /// top bar takes 138 and the master strip 110, so the first screen is about
    /// 1,075. A window shorter than that has to be denser or something falls
    /// off the bottom, which has shipped three times.
    ///
    /// **djmanzo's own default window gets the densest band there is**, and
    /// even that is not enough to bring the master strip back on screen -- 800
    /// would need about a 0.70 scale and the floor is 0.80. It fits the deck's
    /// own controls and no more. That is recorded here rather than papered
    /// over: the rest is a layout decision, not a scaling one.
    #[test]
    fn a_short_window_gets_a_denser_interface() {
        assert_eq!(Density::fitting(800), Density::UltraDense);
        assert_eq!(Density::fitting(900), Density::UltraDense);
        assert_eq!(Density::fitting(1000), Density::UltraDense);
        assert_eq!(Density::fitting(1020), Density::ProDense);
        assert_eq!(Density::fitting(1060), Density::Compact);
        assert_eq!(Density::fitting(1130), Density::Standard);
        assert_eq!(Density::fitting(1200), Density::Standard);
        assert_eq!(Density::fitting(1440), Density::Standard);
        assert_eq!(Density::fitting(1500), Density::Relaxed);

        // The band a window gets must be one whose deck actually fits in it.
        // Both of these were wrong in the guessed first version of the table:
        // 1,200 got Relaxed, whose deck is 1,088 px against about 990 of room,
        // and 900 got Pro Dense, which needs 980. A later pass moved Relaxed
        // again, to 1,460, when pinning the channel strip changed the sum.
        assert_ne!(Density::fitting(1200), Density::Relaxed);
        assert_ne!(Density::fitting(1400), Density::Relaxed);
        assert_ne!(Density::fitting(900), Density::ProDense);
        // And the one the browser sweep caught when the top bar grew a line:
        // 1,100 was Standard, whose deck then needed 524 px of a stage that
        // had 494.
        assert_ne!(Density::fitting(1100), Density::Standard);
        // Below every band's floor, and not a panic.
        assert_eq!(Density::fitting(0), Density::UltraDense);
    }

    /// **The browser harness and this table say the same thing.**
    ///
    /// `ui/e2e/shell.ts` answers `density_bands` with its own copy, because a
    /// Playwright stub cannot call into Rust. A copy is a second source of
    /// truth, and this one has now drifted once: the floors moved here and the
    /// harness went on measuring the application as it used to be, so the
    /// sweep that exists to catch a clipped deck reported a clipped deck that
    /// had already been fixed.
    ///
    /// Rust reads the file rather than the other way round because this is
    /// where the policy lives. If the parse below stops matching the harness's
    /// formatting the test fails loudly, which is the right failure -- a guard
    /// that silently stops guarding is worse than none.
    #[test]
    fn the_harness_and_rust_agree_about_the_bands() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/e2e/shell.ts");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the browser harness at {path}: {e}"));

        // The outer array closes on a line of its own; every inner one closes
        // mid-line. Splitting on the first `],` would stop after one row.
        let table = source
            .split_once("density_bands: [")
            .and_then(|(_, rest)| rest.split_once("\n  ],"))
            .map(|(inside, _)| inside)
            .expect(
                "`density_bands: [` ... `\n  ],` is no longer how the harness writes the table",
            );

        let floors: Vec<u16> = table
            .lines()
            .filter_map(|line| {
                let start = line.find('[')? + 1;
                let end = line[start..].find(',')? + start;
                line[start..end].trim().parse().ok()
            })
            .collect();

        let ours: Vec<u16> = BANDS.iter().map(|(least, _)| *least).collect();
        assert_eq!(
            floors, ours,
            "the browser harness answers with band floors this table does not \
             hold. It measures the interface against what it is told the bands \
             are, so a stale copy is a sweep measuring an application that does \
             not exist -- update `density_bands` in ui/e2e/shell.ts",
        );
    }

    /// A density is spelled the same way stored as it is spoken.
    ///
    /// The shell applies a preset's density by matching the slug serde writes
    /// into the workspace (`pro-dense`) against the name `density_bands`
    /// answers with (`Pro Dense`), lowercased with its spaces hyphenated. That
    /// is one table read two ways rather than two tables — but only while the
    /// two spellings agree, and nothing would report it if they stopped: a
    /// preset whose density did not match any band would quietly open at
    /// whatever density the window happened to fit.
    #[test]
    fn a_density_is_spelled_the_same_way_stored_as_it_is_spoken() {
        for density in Density::ALL {
            let stored = serde_json::to_string(density).expect("a density serialises");
            assert_eq!(
                stored.trim_matches('"'),
                density.name().to_lowercase().replace(' ', "-"),
                "`{}` is stored as {stored}, which the shell cannot match to a band",
                density.name()
            );
        }
    }

    // -- roles -------------------------------------------------------------

    #[test]
    fn every_role_has_its_own_token_name() {
        let mut seen = std::collections::BTreeSet::new();
        for role in Role::ALL {
            assert!(
                seen.insert(role.token()),
                "two roles are published as `{}`",
                role.token()
            );
        }
        assert_eq!(seen.len(), Role::ALL.len());
    }

    /// A role that reuses an appearance token's name would silently take it
    /// over: the stylesheets already set `--warn` and `--danger`, and a
    /// semantic role publishing the same property is the same property.
    ///
    /// That is *deliberate* for `warn` and `danger` -- they were already
    /// semantic and simply lacked a name in Rust -- and would be a collision
    /// for anything else.
    #[test]
    fn only_the_two_already_semantic_names_are_shared_with_the_appearance_tokens() {
        let appearance: std::collections::BTreeSet<&str> = crate::widgets::TOKENS
            .iter()
            .map(|(name, _)| *name)
            .collect();
        let shared: Vec<&str> = Role::ALL
            .iter()
            .map(|role| role.token())
            .filter(|name| appearance.contains(name))
            .collect();
        assert_eq!(
            shared,
            ["warn", "danger"],
            "a semantic role is taking over an appearance token"
        );
    }

    /// The pairs that must be told apart are told apart symmetrically, and
    /// nothing is required to differ from itself.
    #[test]
    fn the_pairs_that_must_differ_are_symmetric() {
        for a in Role::ALL {
            assert!(!a.must_differ_from(*a), "{a:?} must differ from itself");
            for b in Role::ALL {
                assert_eq!(
                    a.must_differ_from(*b),
                    b.must_differ_from(*a),
                    "{a:?} and {b:?} disagree about whether they must differ"
                );
            }
        }
    }

    /// The transition pair is the one that costs something mid-mix, so it is
    /// asserted by name rather than left to the group table.
    #[test]
    fn incoming_and_outgoing_must_differ() {
        assert!(Role::Incoming.must_differ_from(Role::Outgoing));
        assert!(Role::StemVocal.must_differ_from(Role::StemBass));
        // And a pair that never shares a decision is not constrained.
        assert!(!Role::Success.must_differ_from(Role::StemBass));
    }

    // -- density and motion ------------------------------------------------

    #[test]
    fn density_gets_denser_and_stays_inside_what_the_layout_can_draw() {
        let scales: Vec<f32> = Density::ALL.iter().map(|d| d.scale()).collect();
        for pair in scales.windows(2) {
            assert!(pair[0] > pair[1], "density is not monotonic: {scales:?}");
        }
        for (density, scale) in Density::ALL.iter().zip(&scales) {
            assert!(
                (0.8..=1.4).contains(scale),
                "{} is outside the range the layout clamps to",
                density.name()
            );
        }
    }

    #[test]
    fn motion_is_ordered_from_still_to_moving() {
        assert!(Motion::None < Motion::Low);
        assert!(Motion::Low < Motion::Normal);
        assert!(Motion::Normal < Motion::High);
    }

    // -- attention ---------------------------------------------------------

    /// The rule from `docs/GUI-OVERHAUL.md` §18, as a test: mixing must never
    /// be noisier than preparing, and nothing may rearrange while mixing.
    #[test]
    fn performing_asks_less_of_the_dj_than_preparing() {
        let mixing = Attention::performing();
        let preparing = Attention::preparing();

        assert!(mixing.suggestions < preparing.suggestions);
        assert!(mixing.notices <= preparing.notices);
        assert!(mixing.promoted_controls <= preparing.promoted_controls);
        assert!(mixing.motion <= preparing.motion);
        assert!(
            !mixing.reflow,
            "surfaces may not be rearranged during a mix -- moving a panel while \
             somebody is reaching for it is the whole failure mode"
        );
    }

    #[test]
    fn an_emergency_shows_no_suggestions_and_never_moves() {
        let panic = Attention::emergency();
        assert_eq!(panic.suggestions, 0);
        assert!(!panic.reflow);
        assert_eq!(panic.motion, Motion::None);
    }

    #[test]
    fn supervising_an_assisted_mix_is_as_quiet_as_mixing() {
        assert_eq!(Focus::Supervising.attention(), Attention::performing());
    }

    // -- workspace presets (§7) --------------------------------------------

    /// **A preset cannot place a surface djmanzo does not have.**
    ///
    /// The load-bearing check. A workspace is data, and a typo in it is a
    /// panel that silently does not open — the DJ picks "Stem Performance",
    /// sees two decks, and concludes the preset does nothing. Resolving drops
    /// unknown placements on purpose (a workspace saved by a newer djmanzo
    /// must still open), which is exactly why the shipped ones have to be
    /// checked here instead.
    #[test]
    fn every_preset_places_only_surfaces_that_exist() {
        let known: std::collections::BTreeSet<&str> = surfaces().iter().map(|s| s.name).collect();
        for workspace in workspaces() {
            for placement in &workspace.surfaces {
                assert!(
                    known.contains(placement.surface.as_str()),
                    "{} places `{}`, which is not a surface",
                    workspace.name,
                    placement.surface
                );
            }
        }
    }

    /// Every preset says what it is for, under a name of its own, with a
    /// number of decks djmanzo can actually draw.
    #[test]
    fn every_preset_has_its_own_name_and_says_what_it_is() {
        let mut seen = std::collections::BTreeSet::new();
        for workspace in workspaces() {
            assert!(
                seen.insert(workspace.name.clone()),
                "two presets are called {}",
                workspace.name
            );
            assert!(
                workspace.about.len() > 12,
                "{} has a stub description",
                workspace.name
            );
            assert!(
                workspace.decks >= 1 && (workspace.decks as usize) <= dj_core::MAX_DECKS,
                "{} asks for {} decks",
                workspace.name,
                workspace.decks
            );
        }
    }

    /// A preset that names a theme names one that ships.
    ///
    /// The same failure the theme test in `dj_app::mood` guards: a package id
    /// nothing can wear looks exactly like a workspace that decided not to
    /// change the theme.
    #[test]
    fn a_preset_that_names_a_theme_names_one_that_exists() {
        let packages = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../ui/src/controls/themes/packages.ts"),
        )
        .expect("the theme packages are beside the interface");
        for workspace in workspaces() {
            if workspace.theme.is_empty() {
                continue;
            }
            assert!(
                packages.contains(&format!("id: \"{}\"", workspace.theme)),
                "{} asks for the theme `{}`, which does not ship",
                workspace.name,
                workspace.theme
            );
        }
    }

    /// **§7's twenty-four, and the one djmanzo cannot express.**
    ///
    /// Written out so that dropping one is a decision somebody makes here
    /// rather than something that happens. VJ / Visual Performance is absent
    /// because djmanzo has no visual surface at all: a workspace by that name
    /// would arrange the same panels as any other and lie about it in the
    /// picker.
    #[test]
    fn the_directive_names_twenty_four_and_twenty_three_of_them_ship() {
        let names: std::collections::BTreeSet<String> =
            workspaces().into_iter().map(|w| w.name).collect();
        let asked = [
            "Beginner",
            "Classic DJ",
            "Pro Performance",
            "4 Deck",
            "6 Deck",
            "Club",
            "Mobile DJ",
            "Wedding / Event",
            "Open Format",
            "Latin / Caribbean",
            "Scratch / Turntablism",
            "Stem Performance",
            "Mashup / Remix",
            "Preparation",
            "Set Planning",
            "Practice / Learning",
            "Autopilot",
            "Minimal",
            "High Contrast",
            "Laptop Compact",
            "Controller Focus",
            "CDJ / External Mixer",
            "Karaoke / MC",
        ];
        assert_eq!(asked.len(), 23, "§7 lists 24; one of them is not shippable");
        for name in asked {
            assert!(names.contains(name), "§7 asks for {name} and it is missing");
        }
        assert!(
            !names
                .iter()
                .any(|n| n.contains("VJ") || n.contains("Visual")),
            "a visual workspace appeared without a visual surface to put in it"
        );
    }

    /// **Every preset places only surfaces the shell can actually draw.**
    ///
    /// The load-bearing one, and stronger than the check above it: the surface
    /// registry lists twenty, the shell draws seventeen, and the other three
    /// are components nested inside those (the room sensor inside the
    /// assistant, the stem controls on the deck). A placement naming one of
    /// them passes `resolve` — it is a real surface — and is then filtered out
    /// of `placements` in `App.svelte` without a word. The DJ picks "Stem
    /// Performance", sees two decks, and concludes the picker does nothing.
    ///
    /// Four shipped presets did exactly that, "Read the room" among them, and
    /// nothing said so until there was a picker to press. So the list of what
    /// the shell draws is read out of the shell itself: a surface promoted to
    /// top level widens what presets may place, and one demoted breaks this
    /// test rather than a night.
    #[test]
    fn every_preset_places_only_surfaces_the_shell_draws() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/App.svelte");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the shell at {path}: {e}"));
        let table = source
            .split_once("const DRAWN = [")
            .and_then(|(_, rest)| rest.split_once("] as const;"))
            .map(|(inside, _)| inside)
            .expect("`const DRAWN = [` ... `] as const;` is no longer how the shell lists them");

        let drawn: std::collections::BTreeSet<&str> = table
            .lines()
            .filter_map(|line| line.trim().strip_prefix('"'))
            .filter_map(|rest| rest.split_once('"'))
            .map(|(name, _)| name)
            .collect();
        assert!(
            drawn.len() > 5,
            "read {} surfaces out of the shell, which is not how many it draws",
            drawn.len()
        );

        for workspace in workspaces() {
            for placement in &workspace.surfaces {
                assert!(
                    drawn.contains(placement.surface.as_str()),
                    "{} places `{}`, which the shell never draws -- the preset \
                     would open looking like it had done nothing",
                    workspace.name,
                    placement.surface
                );
            }
        }
    }

    /// The harness answers the picker with presets this table holds.
    ///
    /// Four of the twenty-three are typed out in `ui/e2e/shell.ts`, and the
    /// browser test presses them and asserts what happens — four decks, a
    /// density, a theme, a panel. All of that is a measurement of a stub
    /// unless the stub says what Rust says, which is the lesson
    /// `the_harness_and_rust_agree_about_the_bands` records: that table went
    /// stale twice.
    #[test]
    fn the_harness_and_rust_agree_about_the_presets() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/e2e/shell.ts");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the browser harness at {path}: {e}"));
        let table = source
            .split_once("cockpit_workspaces: [")
            .and_then(|(_, rest)| rest.split_once("\n  ],"))
            .map(|(inside, _)| inside)
            .expect("`cockpit_workspaces: [` ... `\n  ],` is no longer how the harness writes it");

        let mut checked = 0;
        for workspace in workspaces() {
            let Some(at) = table.find(&format!("name: \"{}\",", workspace.name)) else {
                continue;
            };
            checked += 1;
            let entry = &table[at..];
            let entry = entry.split_once("\n    },").map_or(entry, |(head, _)| head);

            for (what, wanted) in [
                ("about", format!("about: \"{}\",", workspace.about)),
                ("decks", format!("decks: {},", workspace.decks)),
                (
                    "density",
                    format!(
                        "density: \"{}\",",
                        serde_json::to_string(&workspace.density)
                            .expect("a density serialises")
                            .trim_matches('"')
                    ),
                ),
                ("theme", format!("theme: \"{}\",", workspace.theme)),
            ] {
                assert!(
                    entry.contains(&wanted),
                    "the harness gives `{}` a different {what} from this table -- \
                     it wants `{wanted}`. A browser test pressing a preset the \
                     application does not ship proves nothing about the picker",
                    workspace.name,
                );
            }

            for placement in &workspace.surfaces {
                assert!(
                    entry.contains(&format!("surface: \"{}\"", placement.surface)),
                    "the harness gives `{}` without its `{}` panel",
                    workspace.name,
                    placement.surface,
                );
            }
        }
        assert_eq!(
            checked, 4,
            "the harness used to carry four of these presets and now carries \
             {checked} the names match -- a renamed preset silently stopped \
             being checked"
        );
    }

    /// The one djmanzo opens with is still the empty one.
    #[test]
    fn the_opening_arrangement_is_still_the_decks_and_nothing_else() {
        let opening = opening();
        assert_eq!(opening.name, "Perform");
        assert!(opening.surfaces.is_empty());
    }

    // -- surfaces ----------------------------------------------------------

    #[test]
    fn every_surface_has_its_own_name() {
        let mut seen = std::collections::BTreeSet::new();
        for surface in surfaces() {
            assert!(
                seen.insert(surface.name),
                "two surfaces are `{}`",
                surface.name
            );
        }
    }

    #[test]
    fn every_surface_can_be_placed_somewhere_and_prefers_at_least_its_minimum() {
        for surface in surfaces() {
            assert!(
                !surface.docks.is_empty(),
                "`{}` can be placed nowhere",
                surface.name
            );
            assert!(
                surface.prefer.0 >= surface.least.0 && surface.prefer.1 >= surface.least.1,
                "`{}` prefers to open smaller than it can be used at",
                surface.name
            );
        }
    }

    /// A surface the context engine may open on its own has to be one it may
    /// also close -- otherwise adaptation can fill the screen and never
    /// recover.
    #[test]
    fn anything_adaptation_may_open_it_may_also_collapse() {
        for surface in surfaces() {
            if surface.contextual {
                assert!(
                    surface.collapsible,
                    "`{}` can be opened by adaptation and never collapsed",
                    surface.name
                );
            }
        }
    }

    // -- resolution --------------------------------------------------------

    fn workspace_with(placements: Vec<Placement>) -> Workspace {
        Workspace {
            name: "Test".to_owned(),
            about: String::new(),
            surfaces: placements,
            density: Density::Standard,
            focus: Focus::Performing,
            theme: String::new(),
            decks: 2,
            frozen: false,
        }
    }

    fn place(name: &str, dock: Dock) -> Placement {
        Placement {
            surface: name.to_owned(),
            dock,
            order: 0,
            size: None,
            collapsed: false,
            pinned: false,
        }
    }

    /// **The whole point of the overhaul, as one assertion.** The panel model
    /// this replaces could show one of these at a time; a workspace can hold
    /// all four.
    #[test]
    fn the_room_and_the_library_can_be_open_at_the_same_time() {
        let out = resolve(&workspace_with(vec![
            place("library", Dock::Bottom),
            place("room", Dock::Right),
            place("next", Dock::Right),
            place("prepare", Dock::Left),
        ]));
        assert!(out.notes.is_empty(), "{:?}", out.notes);
        assert_eq!(out.workspace.surfaces.len(), 4);
    }

    #[test]
    fn an_unknown_surface_is_skipped_with_a_note_rather_than_refusing_the_workspace() {
        let out = resolve(&workspace_with(vec![
            place("library", Dock::Bottom),
            place("holodeck", Dock::Right),
        ]));
        assert_eq!(out.workspace.surfaces.len(), 1);
        assert_eq!(out.notes.len(), 1);
        assert!(out.notes[0].contains("holodeck"));
    }

    #[test]
    fn a_surface_in_a_dock_it_cannot_use_is_skipped() {
        // `next` is a side rail; it has no business overlaying the decks.
        let out = resolve(&workspace_with(vec![place("next", Dock::Overlay)]));
        assert!(out.workspace.surfaces.is_empty());
        assert!(out.notes[0].contains("cannot go in"));
    }

    #[test]
    fn a_surface_placed_twice_keeps_its_first_placement() {
        let mut second = place("room", Dock::Left);
        second.order = 5;
        let out = resolve(&workspace_with(vec![place("room", Dock::Right), second]));
        assert_eq!(out.workspace.surfaces.len(), 1);
        assert_eq!(out.workspace.surfaces[0].dock, Dock::Right);
        assert!(out.notes[0].contains("placed twice"));
    }

    /// Below its minimum a surface is opened at the minimum, not clipped.
    /// Clipping is the bug this project has shipped twice.
    #[test]
    fn a_surface_asked_for_less_than_it_needs_is_opened_at_what_it_needs() {
        let mut tiny = place("library", Dock::Bottom);
        tiny.size = Some(20);
        let out = resolve(&workspace_with(vec![tiny]));
        assert_eq!(out.workspace.surfaces[0].size, Some(200));
        assert!(out.notes[0].contains("usable"));
    }

    #[test]
    fn placements_come_back_in_order() {
        let mut a = place("room", Dock::Right);
        a.order = 9;
        let mut b = place("next", Dock::Right);
        b.order = 1;
        let out = resolve(&workspace_with(vec![a, b]));
        let names: Vec<&str> = out
            .workspace
            .surfaces
            .iter()
            .map(|p| p.surface.as_str())
            .collect();
        assert_eq!(names, ["next", "room"]);
    }

    #[test]
    fn a_deck_count_the_grid_has_no_shape_for_is_rounded_down() {
        for (asked, drawn) in [(0, 2), (1, 2), (3, 2), (4, 4), (5, 4), (6, 6), (9, 6)] {
            let mut workspace = workspace_with(vec![]);
            workspace.decks = asked;
            assert_eq!(resolve(&workspace).workspace.decks, drawn, "{asked} decks");
        }
    }

    #[test]
    fn a_workspace_with_no_name_gets_one() {
        let mut workspace = workspace_with(vec![]);
        workspace.name = "  ".to_owned();
        assert_eq!(resolve(&workspace).workspace.name, "Custom");
    }

    #[test]
    fn a_workspace_round_trips_through_json() {
        let workspace = workspace_with(vec![place("library", Dock::Bottom)]);
        let text = serde_json::to_string(&workspace).unwrap();
        let back: Workspace = serde_json::from_str(&text).unwrap();
        assert_eq!(back, workspace);
    }

    // -- tokens ------------------------------------------------------------

    /// Every semantic token has to pass the same shape whitelist the layout
    /// loader already enforces, or a theme could set one to something the
    /// appearance tokens are forbidden.
    #[test]
    fn semantic_tokens_are_checked_by_the_same_rules_as_the_others() {
        for (name, shape) in semantic_tokens() {
            assert_eq!(shape, TokenShape::Colour, "`{name}` is not a colour");
        }
        assert_eq!(semantic_tokens().len(), Role::ALL.len());
    }
}

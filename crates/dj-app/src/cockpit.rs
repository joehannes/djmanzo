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

// -- what a phase wants on screen --------------------------------------------

/// The surfaces §17 says to prioritise, by the phase the night is in.
///
/// > Build a session-phase model. […] The system may infer phase, but the DJ
/// > must always be able to override it.
///
/// `None` is §17's **Setup** — the stretch before anything has read as a
/// phase, which `dj_core::context` produces honestly at the start of every
/// night rather than backdating the earliest phase over it. So the six the
/// section names map onto djmanzo's five plus that unread state:
///
/// | §17 | here |
/// |---|---|
/// | Setup | `None` |
/// | Warm-up | [`SessionPhase::WarmUp`] |
/// | Build | [`SessionPhase::Heat`] |
/// | Peak | [`SessionPhase::Peak`] |
/// | Release | [`SessionPhase::Cooldown`] |
/// | Closing | [`SessionPhase::ChillOut`] |
///
/// # What a priority is, and what it is not
///
/// A surface to **open**, not an arrangement to impose. Everything already on
/// screen stays: the DJ's own choices outrank a phase reading, which is what
/// "the DJ must always be able to override it" means when the overriding has
/// to work without a dialog.
///
/// Several of §17's priorities are not surfaces at all. *Gradual energy*,
/// *longer transitions* and *harmonic resolution* are how djmanzo should
/// **suggest**, not what it should show, and they belong to the planner rather
/// than to the cockpit; *stems* and *FX* are drawn on the deck itself and have
/// nowhere to be promoted to. Those are named in the tests rather than turned
/// into panels that do not exist.
///
/// # Peak is the short one on purpose
///
/// §17's peak list ends with *minimal UI clutter*, which is an instruction
/// about the other five words in it. A phase that opened five panels at the
/// busiest moment of a night would be obeying the list and breaking the
/// sentence, so peak promotes the one thing a DJ cannot get from the decks —
/// what the floor is doing — and nothing else. A test asserts it stays the
/// shortest list here.
#[must_use]
pub fn priorities(phase: Option<dj_core::SessionPhase>) -> &'static [&'static str] {
    match phase {
        // Setup: device health, library, preparation, playlist, room setup.
        None => &["settings", "library", "prepare", "plan", "room"],
        // Warm-up: next-track candidates, and a quiet screen around them.
        Some(dj_core::SessionPhase::WarmUp) => &["next"],
        // Build: compatible candidates, transitions, phrase structure.
        Some(dj_core::SessionPhase::Heat) => &["next", "pair"],
        // Peak: room response, and minimal UI clutter.
        Some(dj_core::SessionPhase::Peak) => &["room"],
        // Release: longer blends and crowd cooling.
        Some(dj_core::SessionPhase::Cooldown) => &["next", "room"],
        // Closing: requests and history live in the browser and the mixes
        // panel; the end-of-set state and the recording are the booth's.
        Some(dj_core::SessionPhase::ChillOut) => &["library", "mixes", "booth"],
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
    /// The deepest tier of §58's hierarchy that may take room **unasked**.
    ///
    /// [`Attention::performing`] has said *"Tier 1 and 2 only; nothing else may
    /// take room"* since it was written, and for just as long that was a
    /// sentence in a doc comment: the budget carried four numbers and a flag,
    /// and nothing anywhere could be checked against the rule. This is the
    /// rule, as data.
    ///
    /// **Unasked** is the whole of it. It governs what djmanzo *offers* — the
    /// surfaces a phase opens by itself, the rows a palette puts in front of
    /// somebody who has typed three letters. It never governs what a DJ asks
    /// for by name: an interface that refused to find Settings mid-mix would be
    /// obeying §18 and breaking §98, which says a whole night's work is
    /// reachable from the palette.
    pub room_for: crate::tiers::Tier,
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
            // §18's own sentence: Tier 1 and 2 only. A record is playing against
            // another and the hands are the only thing that matters.
            room_for: crate::tiers::Tier::Performable,
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
            // Everything, because this is when the paperwork is the work.
            room_for: crate::tiers::Tier::Preparation,
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
            // Everything, for the same reason: an explanation is the point, and
            // §58's contextual tier is where explanations live.
            room_for: crate::tiers::Tier::Preparation,
            motion: Motion::Normal,
        }
    }

    /// The quieter of two budgets, field by field.
    ///
    /// §77's mechanism. The section asks djmanzo to recognise *exploration* and
    /// *performance* as two mental modes and to move between them gracefully;
    /// `Focus` has modelled them since the cockpit was written, stored on the
    /// workspace, and — like §78's `frozen` before it — **nothing read it**.
    ///
    /// What reads it now is this, and the rule it enforces is one-way: a DJ's
    /// chosen focus may make the interface **quieter and never louder**.
    ///
    /// The asymmetry is the whole design. Choosing a performing workspace is a
    /// DJ saying "keep out of my way tonight", and that should hold between
    /// records as well as during them — the machine has no standing to decide
    /// they have relaxed. Choosing a learning workspace is not the same kind of
    /// statement: a DJ practising with two decks audible is still mixing, the
    /// room can still hear it, and an interface that got chattier because the
    /// workspace said *Learning* would be talking over a live mix on the
    /// strength of a menu choice made an hour earlier.
    ///
    /// So the derived budget is the ceiling and the chosen one can only lower
    /// it. `min` on every count, `and` on the one flag, the lower of the two
    /// motion levels.
    #[must_use]
    pub fn quieter_of(self, other: Self) -> Self {
        Self {
            promoted_controls: self.promoted_controls.min(other.promoted_controls),
            suggestions: self.suggestions.min(other.suggestions),
            notices: self.notices.min(other.notices),
            reflow: self.reflow && other.reflow,
            // `min` is the shallower tier, because §58 numbers the hands first:
            // the quieter of "everything may take room" and "Tier 1 and 2 only"
            // is the second, which is the one-way rule this whole method is.
            room_for: self.room_for.min(other.room_for),
            motion: self.motion.min(other.motion),
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
            // The controls, not the paperwork -- even though the thing that fixes
            // a failed recording is in Settings. Promoting it would be djmanzo
            // rearranging the screen around somebody who is already dealing with
            // something going wrong; a DJ who wants Settings types its name, and
            // the palette always answers a name.
            room_for: crate::tiers::Tier::Performable,
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

// -- the lock ---------------------------------------------------------------

/// One of the six things [§79](../../../docs/DIRECTIVE.md) lets a DJ lock.
///
/// The vocabulary is the directive's, verbatim, because that file wins where an
/// implementation disagrees with it. Three of the six stop the same behaviour in
/// today's djmanzo — the workspace, the panel arrangement and the assistant's
/// layout behaviour are one thing with three names, since the only thing that
/// rearranges panels *is* the assistant reading the night — and [`Lock::stops`]
/// says so rather than pretending they are three switches.
///
/// A lock is a DJ's decision, so it is stored with the workspace. What it is
/// **not** is a mode djmanzo enters: nothing here stops the context engine
/// reading the room, planning a transition or offering a record. §79's own last
/// sentence is the rule — *the AI can still use the underlying state without
/// rearranging presentation* — and every one of these takes away a change to
/// the **presentation** and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lock {
    /// Which arrangement is in force.
    Workspace,
    /// Which panels are open, and where they are docked.
    Arrangement,
    /// How much is fitted onto the screen.
    Density,
    /// Which palette is worn.
    Theme,
    /// How the waveform and the controls answer the audio.
    Waveform,
    /// Whether the assistant may move anything.
    Assistant,
}

impl Lock {
    /// Every lock there is. §79's list, in §79's order.
    pub const ALL: [Self; 6] = [
        Self::Workspace,
        Self::Arrangement,
        Self::Density,
        Self::Theme,
        Self::Waveform,
        Self::Assistant,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Arrangement => "arrangement",
            Self::Density => "density",
            Self::Theme => "theme",
            Self::Waveform => "waveform",
            Self::Assistant => "assistant",
        }
    }

    /// What a DJ is told this takes away.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Workspace => "The arrangement stays the one you chose.",
            Self::Arrangement => "Nothing opens, closes or moves unless you do it.",
            Self::Density => "The interface stops resizing itself to the window.",
            Self::Theme => "The palette stays the one you are wearing.",
            Self::Waveform => "The waveform and the controls stop answering the audio.",
            Self::Assistant => "The assistant keeps working, and stops moving anything.",
        }
    }

    /// Which of [§78](../../../docs/DIRECTIVE.md)'s freedoms this takes away.
    ///
    /// The table, in one place. §78 lists four things Freeze stops and §79 lists
    /// six things a DJ may lock, and this is how the two lists meet — so a lock
    /// that stops nothing is a test failure rather than a switch that looks like
    /// it works.
    #[must_use]
    pub const fn stops(self) -> Freedom {
        match self {
            Self::Workspace | Self::Arrangement | Self::Assistant => Freedom::Rearrange,
            Self::Density => Freedom::Resize,
            Self::Theme => Freedom::Retheme,
            Self::Waveform => Freedom::Restyle,
        }
    }
}

/// One of [§78](../../../docs/DIRECTIVE.md)'s four: something djmanzo may do to
/// its own presentation without being asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freedom {
    /// Open, close or move a panel. §17's phase promotion, and the browser that
    /// opens itself with a layout that asks for one.
    Rearrange,
    /// Fit the density to the window. §18's bands.
    Resize,
    /// Change the palette. §31's adaptation.
    Retheme,
    /// Answer the audio in the interface's own colours and motion. §75.
    Restyle,
}

/// What djmanzo may still change about itself.
///
/// §78's four bullets as booleans, derived from the workspace's locks and never
/// stored beside them: two descriptions of one decision is how they come to
/// disagree, and this file has the scar. Published on [`Resolved`] so the
/// interface asks once, at the moment the answer can change, rather than
/// deciding for itself what a lock ought to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Permits {
    /// A panel may open, close or move without the DJ doing it.
    pub rearrange: bool,
    /// The density may follow the window.
    pub resize: bool,
    /// The palette may follow the night.
    pub retheme: bool,
    /// The interface may answer the audio.
    pub restyle: bool,
}

impl Permits {
    /// Nothing is locked, which is what a fresh install gets.
    #[must_use]
    pub const fn everything() -> Self {
        Self {
            rearrange: true,
            resize: true,
            retheme: true,
            restyle: true,
        }
    }

    /// Whether this freedom is still permitted.
    #[must_use]
    pub const fn allows(self, freedom: Freedom) -> bool {
        match freedom {
            Freedom::Rearrange => self.rearrange,
            Freedom::Resize => self.resize,
            Freedom::Retheme => self.retheme,
            Freedom::Restyle => self.restyle,
        }
    }

    fn deny(&mut self, freedom: Freedom) {
        match freedom {
            Freedom::Rearrange => self.rearrange = false,
            Freedom::Resize => self.resize = false,
            Freedom::Retheme => self.retheme = false,
            Freedom::Restyle => self.restyle = false,
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
    /// The deck's own composition, by the name `layout::builtin()` gives it.
    ///
    /// Empty means "leave the deck alone", which is a real answer and the
    /// default: most of §7's arrangements are about *which panels are open*,
    /// and changing what a deck is made of under a DJ who only asked for the
    /// browser would be the surprise §78 forbids.
    ///
    /// [§5B](../../../docs/DIRECTIVE.md) is why this exists. It asks that an
    /// arrangement change *the deck itself* — "club mode: large central stacked
    /// waveforms, compact decks" — and until this field a preset changed which
    /// panels were open, the deck count, the density and the theme, and left
    /// the deck's own composition at whatever the DJ last chose. A name rather
    /// than a copy, exactly like `theme`: a DJ who has edited a layout wants
    /// their edit, and a stored copy would hand them the version from whenever
    /// this table was written.
    #[serde(default)]
    pub layout: String,
    /// What the DJ has decided djmanzo may not change by itself.
    ///
    /// This was `frozen: bool` and nothing read it. A single flag could not
    /// express §79 at all -- that section asks for six separate locks, and a DJ
    /// who wants the theme to stop moving has not asked for the browser to stop
    /// opening itself. §78's Freeze is still one gesture: it is all six.
    ///
    /// Old workspace files carrying `frozen` deserialize cleanly and lose it,
    /// which costs nothing, because nothing honoured it.
    #[serde(default)]
    pub locked: Vec<Lock>,
}

impl Workspace {
    /// Whether this particular lock is on.
    #[must_use]
    pub fn locked(&self, lock: Lock) -> bool {
        self.locked.contains(&lock)
    }

    /// §78's Freeze: every lock, on.
    ///
    /// Derived rather than stored for the reason [`Permits`] gives. A separate
    /// `frozen` flag beside the list is a second description of the same
    /// decision, and the two would disagree the first time a lock was added.
    #[must_use]
    pub fn frozen(&self) -> bool {
        Lock::ALL.iter().all(|lock| self.locked(*lock))
    }

    /// Turn every lock on, or every lock off.
    pub fn freeze(&mut self, frozen: bool) {
        self.locked = if frozen {
            Lock::ALL.to_vec()
        } else {
            Vec::new()
        };
    }

    /// What djmanzo may still do to itself under these locks.
    #[must_use]
    pub fn permits(&self) -> Permits {
        let mut permits = Permits::everything();
        for lock in &self.locked {
            permits.deny(lock.stops());
        }
        permits
    }
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
            layout: String::new(),
            locked: Vec::new(),
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
            // §5B's beginner case and `Starter`'s own description are the same
            // sentence: two decks, big waveforms, everything you need and nothing
            // else. An arrangement called Beginner that left a Pro deck composition
            // in place would be named after something it did not do.
            layout: "Starter".to_owned(),
            locked: Vec::new(),
        },
        Workspace {
            name: "Classic DJ".to_owned(),
            about: "Two decks and a mixer, the way it has always been.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            layout: String::new(),
            locked: Vec::new(),
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
            // §5B's four-deck performance: readable decks with everything on screen.
            // `Pro` is that composition by name and by description.
            layout: "Pro".to_owned(),
            locked: Vec::new(),
        },
        Workspace {
            name: "4 Deck".to_owned(),
            about: "Four decks, close together.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Compact,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 4,
            layout: String::new(),
            locked: Vec::new(),
        },
        Workspace {
            name: "6 Deck".to_owned(),
            about: "Every deck djmanzo has, at the cost of comfort.".to_owned(),
            surfaces: Vec::new(),
            density: Density::UltraDense,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 6,
            layout: String::new(),
            locked: Vec::new(),
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
            // §5B's club mode: large central stacked waveforms, compact decks.
            // `Starter` is the composition with the tallest waveform and the least
            // paperwork on the deck, which is what "compact decks" means here — the
            // deck count stays this arrangement's four.
            layout: "Starter".to_owned(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            // §5B's scratch mode: jog surfaces and turntable-oriented controls
            // expand. The `Scratch` composition is that by name — a platter two and
            // a half times the usual size, a tall lane, and the racks the hands are
            // not on taken off the screen they want.
            layout: "Scratch".to_owned(),
            locked: Vec::new(),
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
            // §5B's stem performance: a large stem-aware waveform and stem controls
            // that become first-class. The module folds by default because it is the
            // biggest block on a deck; this is the arrangement where it is the point
            // rather than a readout.
            layout: "Stem Performance".to_owned(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
        },
        // §5B's autopilot supervisory mode: "performance display becomes
        // simplified and emphasizes: current, next, transition, room response,
        // automation state, emergency takeover". Every one of those is a thing
        // to *watch*, which is why this arrangement is built out of readings
        // rather than controls -- and why the booth went. That panel is the
        // microphone, the plugin insert and the master rack: its own `about`
        // says "set up once a night rather than reached for during a mix",
        // which is the opposite of what a supervisor has open.
        Workspace {
            name: "Autopilot".to_owned(),
            about: "What is playing, what is next and how the room is taking it, while it drives and you watch."
                .to_owned(),
            surfaces: vec![
                // Automation state and the emergency takeover, both. `Conduct`
                // inside this panel carries "I'll take it" -- one press that
                // pulls every control back -- above everything else in it, and
                // it is also where what the assistant is about to do is said.
                Placement {
                    surface: "assistant".to_owned(),
                    dock: Dock::Right,
                    order: 0,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                // Next.
                Placement {
                    surface: "next".to_owned(),
                    dock: Dock::Right,
                    order: 1,
                    size: None,
                    collapsed: false,
                    pinned: false,
                },
                // Room response.
                Placement {
                    surface: "room".to_owned(),
                    dock: Dock::Bottom,
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
            // Current, simplified. `Starter` is the reduction djmanzo already
            // ships -- no pads, no loops, no effect rack, no beat jump, no
            // filter, no keylock, and the tallest waveform of the six -- which
            // is a watched deck exactly: the record legible, the cue and the
            // transport there for a takeover, and nothing to perform with.
            //
            // A seventh preset spelling the same reduction under a supervisory
            // name would be two names for one layout, which
            // `the_presets_trade_complexity_for_space` exists to prevent. A
            // composition describes a deck, not the DJ in front of it.
            layout: "Starter".to_owned(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
        },
        Workspace {
            name: "Minimal".to_owned(),
            about: "The decks, given room to breathe.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Relaxed,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            layout: String::new(),
            locked: Vec::new(),
        },
        Workspace {
            name: "High Contrast".to_owned(),
            about: "The decks, in the theme built for a dark booth and a bright screen.".to_owned(),
            surfaces: Vec::new(),
            density: Density::Standard,
            focus: Focus::Performing,
            theme: "pkg-booth".to_owned(),
            decks: 2,
            layout: String::new(),
            locked: Vec::new(),
        },
        Workspace {
            name: "Laptop Compact".to_owned(),
            about: "Everything that fits on a small screen, and nothing that does not.".to_owned(),
            surfaces: Vec::new(),
            density: Density::UltraDense,
            focus: Focus::Performing,
            theme: "".to_owned(),
            decks: 2,
            // §5B's laptop compact mode: dense controls optimised for limited screen
            // height. `Performance` is the densest composition djmanzo ships, and it
            // is the one §48's laptop mode is about the machine behind.
            layout: "Performance".to_owned(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
            layout: String::new(),
            locked: Vec::new(),
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
    /// What djmanzo may still change about itself, under this workspace's locks.
    ///
    /// Sent with the workspace rather than asked for separately, because the two
    /// can only change together: a lock is a field of the workspace, and every
    /// path that can alter one goes through [`resolve`]. Derived here so the
    /// interface never has to know what a lock means -- §78's four freedoms are
    /// a judgement, and a judgement written in `App.svelte` is a judgement
    /// nobody can test.
    pub permits: Permits,
}

// -- the DJ's own arrangements ----------------------------------------------

/// Put an arrangement into the DJ's own collection, under a name they chose.
///
/// §7's promise is that the shipped arrangements are *starting points, not rigid
/// identities* — a DJ picks one, moves what they like, and that is the point. It
/// was only half kept: the edit survived a restart, because the cockpit stores
/// whatever shape it was last dragged into, but it could not be **named**, so a
/// DJ with a wedding layout and a club layout had one of them and a memory of
/// the other. §103's *Modularity* criterion is the same gap said another way: *a
/// user can construct a personal workflow*.
///
/// Pure, and taking both lists rather than reading them, so every rule here is
/// testable without a config directory — which a test process does not have.
///
/// # Errors
/// With the sentence a DJ reads. Two refusals, and both are about the picker
/// being readable:
///
/// - **A name is required.** An arrangement called "" is a blank row in a menu.
/// - **A shipped name may not be taken.** Two rows reading "Club" is a DJ
///   choosing one of them and finding out afterwards which; there are
///   twenty-three other words. Compared without case or surrounding space,
///   because "club" and "Club " are the same word to everyone except a string
///   comparison.
///
/// Saving over one of the DJ's **own** names is not a refusal — that is what
/// saving is, and asking them to confirm it is a dialog in the middle of setting
/// up.
pub fn keep(
    kept: &[Workspace],
    shipped: &[Workspace],
    workspace: &Workspace,
    name: &str,
) -> Result<Vec<Workspace>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("an arrangement needs a name to be found by".to_owned());
    }
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    if let Some(clash) = shipped.iter().find(|s| same(&s.name, name)) {
        return Err(format!(
            "djmanzo already ships an arrangement called {:?} — pick another name",
            clash.name
        ));
    }

    let mut out: Vec<Workspace> = kept
        .iter()
        .filter(|held| !same(&held.name, name))
        .cloned()
        .collect();
    let mut saved = workspace.clone();
    saved.name = name.to_owned();
    out.push(saved);
    // Sorted, so the picker does not reorder itself every time a DJ saves. Case
    // insensitively, because a menu that puts "club" after "Wedding" is a menu
    // whose order nobody can predict.
    out.sort_by_key(|held| held.name.to_lowercase());
    Ok(out)
}

/// Take one of the DJ's own arrangements out of the collection.
///
/// Silent about a name that is not there: forgetting something twice is the
/// same as forgetting it once, and an error for it would only ever be seen by
/// somebody who pressed a button that had already worked.
#[must_use]
pub fn forget(kept: &[Workspace], name: &str) -> Vec<Workspace> {
    kept.iter()
        .filter(|held| !held.name.trim().eq_ignore_ascii_case(name.trim()))
        .cloned()
        .collect()
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

    // A lock named twice is one lock, and the order is the section's rather
    // than whatever order a DJ happened to tick them in: the list goes into a
    // preferences file and a file that reorders itself on every save is a file
    // nobody can diff.
    out.locked.sort_unstable();
    out.locked.dedup();

    let permits = out.permits();
    Resolved {
        workspace: out,
        notes,
        permits,
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
            .unwrap_or_else(|e| panic!("could not read the browser harness at {path}: {e}"))
            .replace("\r\n", "\n");

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

    /// **Every role has a colour, and the pairs that must differ do.**
    ///
    /// §30's whole instruction in one test: *color must communicate meaning;
    /// do not produce a neon application where everything is colorful and
    /// nothing is semantically distinct.* Both halves are here, because either
    /// alone is satisfiable by something useless -- fourteen roles all pointing
    /// at the accent communicates nothing, and fourteen arbitrary hues is the
    /// neon failure.
    ///
    /// The values are read out of the stylesheet rather than held in Rust,
    /// because a colour is the interface's to own and this is the rule's. Each
    /// role is an *alias* onto a token the palettes already define, so all
    /// seven packages get all fourteen and nothing has to keep a hundred and
    /// sixty-eight numbers true. Two roles outside a must-differ pair are free
    /// to share one: `active`, `incoming` and `success` are all the accent, and
    /// a control being on is never mistaken for a record arriving.
    /// **And they have to differ to the eye, not only to the parser.**
    ///
    /// `every_role_has_a_colour_and_the_pairs_that_must_differ_do` checks that
    /// two roles in a must-differ group point at *different tokens*. That is a
    /// necessary condition and not a sufficient one, and the gap between the
    /// two is where a real defect lived: in the organic palette's light
    /// variant `--accent` was `#0f7b57` and `--accent-2` was `#057a5f`, two
    /// tokens with different names and the same green. So *selected* and
    /// *active* were one colour, *the assistant did it* and *the room did it*
    /// were one colour, and the vocal stem and the other stem were one colour
    /// — in a theme that had passed every test in this file for months.
    ///
    /// This resolves each role to its token, each token to the hex every
    /// shipped palette gives it, and measures. CIE76 ΔE in Lab rather than a
    /// WCAG ratio, because a ratio is about *legibility of text* and answers
    /// the wrong question here: pure red and pure blue have almost the same
    /// luminance and nobody confuses them.
    ///
    /// Twenty is the floor. It is roughly "obviously a different colour in a
    /// glance at a small swatch", it is comfortably cleared by the palettes
    /// that were right already — Daylight 26, Industrial 28, Cyber 40 — and
    /// the five that failed it were all wrong in a way anyone would report as
    /// a bug on sight.
    #[test]
    fn the_pairs_that_must_differ_differ_to_the_eye_in_every_palette() {
        /// Perceptual distance in CIE Lab, CIE76.
        fn delta_e(a: &str, b: &str) -> f64 {
            fn lab(hex: &str) -> [f64; 3] {
                let hex = hex.trim_start_matches('#');
                let channel = |at: usize| -> f64 {
                    let raw = u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
                    let c = f64::from(raw) / 255.0;
                    if c <= 0.04045 {
                        c / 12.92
                    } else {
                        ((c + 0.055) / 1.055).powf(2.4)
                    }
                };
                let (r, g, b) = (channel(0), channel(2), channel(4));
                // sRGB D65 -> XYZ, then XYZ -> Lab.
                let x = (r * 0.4124 + g * 0.3576 + b * 0.1805) / 0.950_47;
                let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
                let z = (r * 0.0193 + g * 0.1192 + b * 0.9505) / 1.088_83;
                let f = |t: f64| {
                    if t > 0.008_856 {
                        t.cbrt()
                    } else {
                        7.787 * t + 16.0 / 116.0
                    }
                };
                let (fx, fy, fz) = (f(x), f(y), f(z));
                [
                    116.0f64.mul_add(fy, -16.0),
                    500.0 * (fx - fy),
                    200.0 * (fy - fz),
                ]
            }
            let (one, two) = (lab(a), lab(b));
            (0..3)
                .map(|i| (one[i] - two[i]).powi(2))
                .sum::<f64>()
                .sqrt()
        }

        /// Obviously a different colour at a glance. See the note above.
        const FLOOR: f64 = 20.0;

        let sheet =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/app.css"))
                .expect("the stylesheet is in the tree")
                .replace("\r\n", "\n");
        let root = sheet
            .split_once(":root {")
            .and_then(|(_, rest)| rest.split_once("\n}"))
            .map(|(inside, _)| inside)
            .expect("`:root {` ... `\n}` is no longer how the stylesheet opens");

        // A role's token, and the palette token it is derived from. A role
        // defined as a literal colour is the same in every palette and so
        // cannot collide differently in one of them.
        let derived = |role: &Role| -> Option<String> {
            let want = format!("--{}:", role.token());
            let value = root.lines().find_map(|line| {
                let line = line.trim();
                Some(line.strip_prefix(&want)?.trim().trim_end_matches(';'))
            })?;
            let inner = value.strip_prefix("var(")?.strip_suffix(')')?;
            Some(inner.split(',').next()?.trim().to_owned())
        };

        let colours = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/controls/themes/colors.ts"
        ))
        .expect("the palettes are in the tree")
        .replace("\r\n", "\n");

        // Every palette, by id and variant, as token -> hex.
        let mut palettes: Vec<(String, std::collections::BTreeMap<String, String>)> = Vec::new();
        let mut id = String::new();
        let mut variant = String::new();
        let mut current = std::collections::BTreeMap::new();
        for line in colours.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix('"')
                && let Some((name, _)) = rest.split_once('"')
                && name.starts_with("pkg-")
                && line.ends_with('{')
            {
                // Flush first. Without this the variant that was being read
                // lands under the *next* palette's id, and the failure message
                // then names a theme that is not the broken one -- which is
                // how this was found.
                if !current.is_empty() {
                    palettes.push((format!("{id} {variant}"), std::mem::take(&mut current)));
                }
                id = name.to_owned();
            } else if (line == "dark: {" || line == "light: {") && !id.is_empty() {
                if !current.is_empty() {
                    palettes.push((format!("{id} {variant}"), std::mem::take(&mut current)));
                }
                variant = line.trim_end_matches(": {").to_owned();
            } else if let Some(rest) = line.strip_prefix("\"--")
                && let Some((token, tail)) = rest.split_once("\":")
                && let Some(hex) = tail.trim().trim_end_matches(',').trim().strip_prefix('"')
            {
                current.insert(format!("--{token}"), hex.trim_end_matches('"').to_owned());
            }
        }
        if !current.is_empty() {
            palettes.push((format!("{id} {variant}"), current));
        }
        assert!(
            palettes.len() >= 16,
            "read {} palettes out of `colors.ts`, which is fewer than ship -- the \
             parser above has stopped matching how they are written",
            palettes.len()
        );

        let mut checked = 0;
        for (name, palette) in &palettes {
            for one in Role::ALL {
                for two in Role::ALL {
                    if !one.must_differ_from(*two) || one.token() >= two.token() {
                        continue;
                    }
                    let (Some(a), Some(b)) = (derived(one), derived(two)) else {
                        continue;
                    };
                    let (Some(hex_a), Some(hex_b)) = (palette.get(&a), palette.get(&b)) else {
                        continue;
                    };
                    // Hex with an alpha channel would measure as its opaque
                    // form here; none of the role tokens carries one.
                    if hex_a.len() < 7 || hex_b.len() < 7 {
                        continue;
                    }
                    let distance = delta_e(hex_a, hex_b);
                    assert!(
                        distance >= FLOOR,
                        "in `{name}`, {} ({hex_a}) and {} ({hex_b}) are {distance:.1} apart, \
                         and a DJ has to tell them apart at a glance. They point at \
                         different tokens -- `{a}` and `{b}` -- which is why the test above \
                         passes; they are the same colour, which is what matters",
                        one.about(),
                        two.about(),
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            checked >= 100,
            "only {checked} pairs were measured, so this checked almost nothing"
        );
    }

    #[test]
    fn every_role_has_a_colour_and_the_pairs_that_must_differ_do() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/app.css");
        let sheet = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the stylesheet at {path}: {e}"))
            .replace("\r\n", "\n");

        // The first `:root` block is where the derived tokens live; a later
        // block redefines the palette for the light theme and must not be read
        // as a second answer for the same role.
        let root = sheet
            .split_once(":root {")
            .and_then(|(_, rest)| rest.split_once("\n}"))
            .map(|(inside, _)| inside)
            .expect("`:root {` ... `\n}` is no longer how the stylesheet opens");

        let value_of = |token: &str| -> Option<String> {
            root.lines().find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix(&format!("--{token}:"))?;
                Some(rest.trim().trim_end_matches(';').to_owned())
            })
        };

        let mut colours = std::collections::BTreeMap::new();
        for role in Role::ALL {
            let value = value_of(role.token()).unwrap_or_else(|| {
                panic!(
                    "`--{}` ({}) has no value, so anything drawn in it inherits \
                     instead -- §30 asks every role to map to an actual colour",
                    role.token(),
                    role.about()
                )
            });
            assert!(
                !value.is_empty(),
                "`--{}` is declared with nothing after the colon",
                role.token()
            );
            colours.insert(*role, value);
        }

        // And each is actually asked for. Fourteen definitions nothing reads
        // would be §30 satisfied on paper and not at all on screen, which is
        // the state this section was in: the roles existed as types with tests
        // and the stylesheet went on using the appearance tokens.
        let components: Vec<std::path::PathBuf> = std::fs::read_dir(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src"),
        )
        .expect("the interface's sources are beside the crates")
        .filter_map(|entry| Some(entry.ok()?.path()))
        .chain(
            std::fs::read_dir(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/controls"),
            )
            .expect("the shared controls are beside them")
            .filter_map(|entry| Some(entry.ok()?.path())),
        )
        .filter(|path| path.extension().is_some_and(|kind| kind == "svelte"))
        .collect();
        assert!(
            components.len() > 20,
            "read {} components, which is not how many the interface has",
            components.len()
        );
        let drawn: String = components
            .iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .map(|source| source.replace("\r\n", "\n"))
            .collect();
        for role in Role::ALL {
            assert!(
                drawn.contains(&format!("var(--{})", role.token())),
                "nothing asks for `--{}` ({}), so the role is a definition the \
                 interface never draws",
                role.token(),
                role.about()
            );
        }

        for a in Role::ALL {
            for b in Role::ALL {
                if !a.must_differ_from(*b) {
                    continue;
                }
                assert_ne!(
                    colours[a],
                    colours[b],
                    "{:?} and {:?} are both `{}`, and they are a pair a DJ has \
                     to tell apart -- {} against {}",
                    a,
                    b,
                    colours[a],
                    a.about(),
                    b.about()
                );
            }
        }
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

    // **A preset that names a theme names one that ships** is checked in
    // [`crate::theme`], where §32's table lives, rather than by grepping the
    // interface's source from here.

    /// **The load-bearing one for §5B: an arrangement that names a deck
    /// composition names one that ships.**
    ///
    /// §5B asks that an arrangement change *the deck itself* — "club mode:
    /// large central stacked waveforms, compact decks" — and until `layout`
    /// existed a preset changed which panels were open, the deck count, the
    /// density and the theme, and left the deck's own composition at whatever
    /// the DJ last chose. A name that nothing answers to fails in the worst
    /// place: five of six things take and the sixth silently does not, which is
    /// exactly what §54's own rule is about.
    #[test]
    fn an_arrangement_that_names_a_deck_composition_names_one_that_ships() {
        let shipped: Vec<String> = crate::layout::builtin()
            .into_iter()
            .map(|l| l.name)
            .collect();
        for workspace in workspaces() {
            if workspace.layout.is_empty() {
                continue;
            }
            assert!(
                shipped.contains(&workspace.layout),
                "the `{}` arrangement asks for the `{}` deck composition, which \
                 `layout::builtin()` does not have",
                workspace.name,
                workspace.layout
            );
        }
    }

    /// **And most of them name none, which is the right default.**
    ///
    /// Empty means *leave the deck alone*. Most of §7's arrangements are about
    /// which panels are open, and changing what a deck is made of under a DJ
    /// who only asked for the browser would be the surprise §78 forbids. A
    /// table where every row named a composition would make every arrangement
    /// a deck change, which is not what §5B asks for and is not what a DJ
    /// pressing *Preparation* expects.
    #[test]
    fn an_arrangement_only_changes_the_deck_when_that_is_what_it_is_for() {
        let named = workspaces()
            .into_iter()
            .filter(|w| !w.layout.is_empty())
            .count();
        let all = workspaces().len();
        assert!(
            named > 0,
            "no arrangement changes the deck's composition, so §5B's ask is \
             still a field nothing uses"
        );
        assert!(
            named * 2 < all,
            "{named} of {all} arrangements change the deck's composition — at \
             that point the field is not the exception §5B describes, and a DJ \
             pressing any preset would find their deck rebuilt"
        );
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
    /// **§5B's supervisory display shows every emphasis §5B names.**
    ///
    /// > Autopilot supervisory mode. Performance display becomes simplified and
    /// > emphasizes: current, next, transition, room response, automation
    /// > state, emergency takeover.
    ///
    /// Six nouns, and an arrangement is the only place they can all be true at
    /// once. Which panel carries which is a judgement and it is written here,
    /// where it can be disagreed with, rather than left implicit in a list of
    /// three surface names that reads as arbitrary: `assistant` carries three
    /// of the six because `Conduct` inside it is where the takeover press, the
    /// authority and the step about to be taken all live.
    ///
    /// *Simplified* is asserted against the default deck rather than against
    /// another preset: "fewer controls than the other one" is satisfied by
    /// giving the other one more.
    #[test]
    fn the_autopilot_arrangement_shows_what_5b_asks_a_supervisor_to_watch() {
        let autopilot = workspaces()
            .into_iter()
            .find(|workspace| workspace.name == "Autopilot")
            .expect("§7's autopilot arrangement ships");
        let docked: std::collections::BTreeSet<&str> = autopilot
            .surfaces
            .iter()
            .map(|placement| placement.surface.as_str())
            .collect();

        // "the deck" is not a surface: it is the performance zone itself, and
        // what §5B asks of it is the `simplified` below.
        for (emphasis, carried_by) in [
            ("current", "the deck"),
            ("next", "next"),
            ("transition", "assistant"),
            ("room response", "room"),
            ("automation state", "assistant"),
            ("emergency takeover", "assistant"),
        ] {
            if carried_by == "the deck" {
                assert!(
                    !autopilot.layout.is_empty(),
                    "§5B asks the supervisory display to say what is playing and the                      arrangement leaves the deck at whatever the DJ last chose"
                );
                continue;
            }
            assert!(
                docked.contains(carried_by),
                "§5B asks a supervisor to watch {emphasis}, which djmanzo carries in                  the `{carried_by}` panel, and this arrangement does not open it"
            );
        }

        let named = crate::layout::builtin()
            .into_iter()
            .find(|layout| layout.name == autopilot.layout)
            .unwrap_or_else(|| panic!("`{}` is a composition that ships", autopilot.layout));
        let ordinary = crate::layout::Layout::default();

        for (what, simplified) in [
            ("pads", !named.pads),
            ("loops", !named.loops),
            ("an effect rack", !named.fx),
            ("beat jump", !named.beat_jump),
        ] {
            assert!(
                simplified,
                "§5B asks the supervisory display to be *simplified* and `{}` still                  draws {what}. A deck to perform on is not a deck to watch",
                autopilot.layout
            );
        }
        assert!(
            named.waveform_height > ordinary.waveform_height,
            "the record being watched is smaller on the watching deck ({} px) than on              an ordinary one ({} px); simplified means less to read, not less legible",
            named.waveform_height,
            ordinary.waveform_height
        );
    }

    #[test]
    fn every_preset_places_only_surfaces_the_shell_draws() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/App.svelte");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the shell at {path}: {e}"))
            .replace("\r\n", "\n");
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
            .unwrap_or_else(|e| panic!("could not read the browser harness at {path}: {e}"))
            .replace("\r\n", "\n");
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
                // §5B: the field that rebuilds the deck. A fixture saying
                // `layout: ""` where this table names a composition would let a
                // browser test watch a shell that was never asked to do
                // anything and call the silence a pass.
                ("layout", format!("layout: \"{}\",", workspace.layout)),
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
            checked, 5,
            "the harness used to carry five of these presets and now carries \
             {checked} the names match -- a renamed preset silently stopped \
             being checked"
        );
    }

    // -- §17's phase priorities --------------------------------------------

    /// **Every phase promotes only surfaces the shell draws.**
    ///
    /// The load-bearing one, and the same trap §7's presets fell into: a name
    /// that `resolve` accepts and `App.svelte` then filters out is a phase
    /// change that appears to do nothing. Read out of the shell, so promoting
    /// a surface widens what a phase may ask for and demoting one breaks a
    /// test rather than a night.
    #[test]
    fn every_phase_promotes_only_surfaces_the_shell_draws() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/App.svelte");
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("could not read the shell at {path}: {e}"))
            .replace("\r\n", "\n");
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
            "read {} surfaces out of the shell",
            drawn.len()
        );

        let known: std::collections::BTreeSet<&str> = surfaces().iter().map(|s| s.name).collect();
        for phase in std::iter::once(None).chain(dj_core::SessionPhase::ALL.map(Some)) {
            for name in priorities(phase) {
                assert!(
                    known.contains(name),
                    "{phase:?} promotes `{name}`, which is not a surface"
                );
                assert!(
                    drawn.contains(name),
                    "{phase:?} promotes `{name}`, which the shell never draws -- \
                     the phase would arrive looking like it had done nothing"
                );
            }
        }
    }

    /// **Peak stays the shortest list.**
    ///
    /// §17's peak priorities end with *minimal UI clutter*, which is an
    /// instruction about the five words before it. Opening five panels at the
    /// busiest moment of a night would obey the list and break the sentence.
    #[test]
    fn peak_promotes_less_than_any_other_phase() {
        let at_peak = priorities(Some(dj_core::SessionPhase::Peak)).len();
        assert!(at_peak > 0, "peak promotes nothing at all");
        for phase in std::iter::once(None).chain(dj_core::SessionPhase::ALL.map(Some)) {
            if phase == Some(dj_core::SessionPhase::Peak) {
                continue;
            }
            assert!(
                priorities(phase).len() >= at_peak,
                "{phase:?} promotes fewer surfaces than peak does, so peak is no \
                 longer the quiet one"
            );
        }
        assert!(
            at_peak <= 2,
            "peak promotes {at_peak} surfaces, which is not minimal clutter"
        );
    }

    /// §17 names six phases; djmanzo has five plus the stretch before anything
    /// has read, and every one of the six answers something.
    #[test]
    fn all_six_of_the_directives_phases_have_priorities() {
        let mut seen = 0;
        for phase in std::iter::once(None).chain(dj_core::SessionPhase::ALL.map(Some)) {
            assert!(
                !priorities(phase).is_empty(),
                "{phase:?} promotes nothing, so that phase does nothing"
            );
            seen += 1;
        }
        assert_eq!(
            seen, 6,
            "§17 names Setup, Warm-up, Build, Peak, Release and Closing; this \
             table answers {seen} of them"
        );
    }

    /// A phase never promotes the same surface twice.
    #[test]
    fn a_phase_does_not_promote_one_surface_twice() {
        for phase in std::iter::once(None).chain(dj_core::SessionPhase::ALL.map(Some)) {
            let mut seen = std::collections::BTreeSet::new();
            for name in priorities(phase) {
                assert!(seen.insert(name), "{phase:?} promotes `{name}` twice");
            }
        }
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
            layout: String::new(),
            locked: Vec::new(),
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

    // -- §78 and §79 --------------------------------------------------------

    fn unlocked() -> Workspace {
        workspaces().into_iter().next().expect("something ships")
    }

    /// **The load-bearing one: a lock that takes nothing away is a switch that
    /// lies.**
    ///
    /// Which is what this section was: `frozen` was stored, serialised, round
    /// tripped, and read by nothing at all. §78 calls Freeze *the professional
    /// safety valve*, and a safety valve that does not open is worse than an
    /// absent one, because a DJ who believes the layout is pinned stops watching
    /// it.
    ///
    /// Both directions, because either alone passes wrongly. A lock must take
    /// something away, and it must take away only its own thing -- a "lock the
    /// theme" that also stopped the browser opening would be a Freeze wearing a
    /// narrower name, and §79's whole point is that the six are separate.
    #[test]
    fn every_lock_takes_exactly_one_freedom_away() {
        for lock in Lock::ALL {
            let mut workspace = unlocked();
            workspace.locked = vec![lock];
            let permits = workspace.permits();
            let gone = lock.stops();

            assert!(
                !permits.allows(gone),
                "`{}` is stored, serialised and read by nothing -- which is the \
                 state §78 was in",
                lock.name()
            );
            for freedom in [
                Freedom::Rearrange,
                Freedom::Resize,
                Freedom::Retheme,
                Freedom::Restyle,
            ] {
                if freedom == gone {
                    continue;
                }
                assert!(
                    permits.allows(freedom),
                    "`{}` took away {freedom:?} as well as {gone:?}, so it is a \
                     freeze under a narrower name",
                    lock.name()
                );
            }
        }
    }

    /// Each lock stops the thing its name promises.
    ///
    /// Written out rather than derived, which is the point: the test above
    /// compares `permits()` against `stops()` and so cannot see the two agreeing
    /// on the wrong answer. This one is the table a reader can check against
    /// §78 and §79 by eye, and it is the only place the pairing is asserted
    /// twice on purpose.
    #[test]
    fn each_lock_stops_the_thing_its_name_promises() {
        use Freedom::{Rearrange, Resize, Restyle, Retheme};
        assert_eq!(Lock::Workspace.stops(), Rearrange);
        assert_eq!(Lock::Arrangement.stops(), Rearrange);
        assert_eq!(Lock::Assistant.stops(), Rearrange);
        assert_eq!(Lock::Density.stops(), Resize);
        assert_eq!(Lock::Theme.stops(), Retheme);
        assert_eq!(Lock::Waveform.stops(), Restyle);
    }

    /// Nothing locked is everything permitted, which is what a fresh install is.
    #[test]
    fn a_workspace_nobody_has_locked_stops_nothing() {
        let permits = unlocked().permits();
        assert_eq!(permits, Permits::everything());
        assert!(!unlocked().frozen());
    }

    /// §78's Freeze is §79's six, and is derived rather than stored.
    #[test]
    fn freezing_is_every_lock_and_thawing_is_none() {
        let mut workspace = unlocked();
        workspace.freeze(true);
        assert!(workspace.frozen());
        assert_eq!(workspace.locked.len(), Lock::ALL.len());
        for lock in Lock::ALL {
            assert!(
                workspace.locked(lock),
                "`{}` survived a freeze",
                lock.name()
            );
        }
        let permits = workspace.permits();
        assert_eq!(
            permits,
            Permits {
                rearrange: false,
                resize: false,
                retheme: false,
                restyle: false,
            },
            "§78 lists four things Freeze stops and this stops fewer"
        );

        workspace.freeze(false);
        assert!(!workspace.frozen());
        assert!(workspace.locked.is_empty());
    }

    /// Five of six is not frozen.
    ///
    /// Worth its own test because the tempting implementation of `frozen` is a
    /// separate boolean somebody sets beside the list, and that one answers
    /// `true` here.
    #[test]
    fn a_workspace_missing_one_lock_is_not_frozen() {
        for missing in Lock::ALL {
            let mut workspace = unlocked();
            workspace.locked = Lock::ALL
                .iter()
                .copied()
                .filter(|l| *l != missing)
                .collect();
            assert!(
                !workspace.frozen(),
                "a workspace that still lets djmanzo change the {} reads as frozen",
                missing.name()
            );
        }
    }

    /// Every lock is spelled the same way stored as it is spoken.
    #[test]
    fn a_lock_is_spelled_the_same_way_stored_as_it_is_spoken() {
        for lock in Lock::ALL {
            let stored = serde_json::to_string(&lock).expect("a lock serialises");
            assert_eq!(stored, format!("\"{}\"", lock.name()));
            assert!(
                !lock.about().is_empty(),
                "`{}` has nothing to tell a DJ it does",
                lock.name()
            );
        }
    }

    /// The resolver hands the permits back with the workspace, and tidies the
    /// list on the way.
    #[test]
    fn the_resolver_answers_with_the_permits_and_one_of_each_lock() {
        let mut workspace = unlocked();
        workspace.locked = vec![Lock::Theme, Lock::Theme, Lock::Density];
        let resolved = resolve(&workspace);
        assert_eq!(resolved.workspace.locked, vec![Lock::Density, Lock::Theme]);
        assert!(!resolved.permits.retheme);
        assert!(!resolved.permits.resize);
        assert!(
            resolved.permits.rearrange,
            "locking the theme stopped the panels moving as well"
        );
        assert_eq!(resolved.permits, resolved.workspace.permits());
    }

    // -- §7 and §103's modularity ------------------------------------------

    fn named(name: &str) -> Workspace {
        let mut workspace = unlocked();
        workspace.name = name.to_owned();
        workspace
    }

    /// **The load-bearing one: saving under a name a DJ already used replaces
    /// it, and saving under a name djmanzo ships does not.**
    ///
    /// Both, because each alone is a plausible implementation and each alone is
    /// wrong. A save that always appended would give a DJ four rows called
    /// "Wedding" after four evenings of adjusting one; a save that refused a
    /// name already theirs would make "save" mean "save once" and put a delete
    /// in front of every edit.
    ///
    /// And the shipped names are not theirs to take. Two rows reading "Club" is
    /// a DJ choosing one of them and finding out afterwards which, in a menu
    /// they reach for when the night changes.
    #[test]
    fn saving_replaces_your_own_name_and_refuses_one_djmanzo_ships() {
        let shipped = workspaces();
        let mine = keep(&[], &shipped, &named("x"), "Wedding").expect("a free name");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].name, "Wedding");

        let mut moved = named("x");
        moved.decks = 4;
        let after = keep(&mine, &shipped, &moved, "Wedding").expect("saving again");
        assert_eq!(
            after.len(),
            1,
            "a second save under the same name made a second arrangement, so \
             four evenings of adjusting one leaves four rows called Wedding"
        );
        assert_eq!(after[0].decks, 4, "the save kept the old arrangement");

        let taken = shipped[1].name.clone();
        let refused =
            keep(&mine, &shipped, &named("x"), &taken).expect_err("a shipped name was taken");
        assert!(
            refused.contains(&taken),
            "the refusal does not say which name is taken: {refused}"
        );
    }

    /// The comparison is on the word, not on the bytes.
    ///
    /// "club", "Club" and "Club " are one name to everybody except a string
    /// comparison, and a DJ who typed the second is not asking for a second row.
    #[test]
    fn a_name_is_the_same_name_whatever_its_case_and_spacing() {
        let shipped = workspaces();
        let mine = keep(&[], &shipped, &named("x"), "Back Room").expect("a free name");
        let again = keep(&mine, &shipped, &named("x"), "  back room  ").expect("the same name");
        assert_eq!(again.len(), 1);
        assert_eq!(
            again[0].name, "back room",
            "the name a DJ typed last is the one they get"
        );
        assert!(
            keep(
                &mine,
                &shipped,
                &named("x"),
                &shipped[1].name.to_lowercase()
            )
            .is_err()
        );
    }

    /// A name is required, and whitespace is not one.
    #[test]
    fn an_arrangement_with_no_name_is_refused_rather_than_stored_blank() {
        let shipped = workspaces();
        for nothing in ["", "   ", "\t"] {
            assert!(
                keep(&[], &shipped, &named("x"), nothing).is_err(),
                "{nothing:?} was accepted as a name, so the picker has a blank row"
            );
        }
    }

    /// The picker's order does not depend on the order things were saved in.
    #[test]
    fn the_collection_is_sorted_so_the_picker_settles() {
        let shipped = workspaces();
        let mut mine = Vec::new();
        for name in ["zulu", "Alpha", "mike"] {
            mine = keep(&mine, &shipped, &named("x"), name).expect("a free name");
        }
        let order: Vec<&str> = mine.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(
            order,
            vec!["Alpha", "mike", "zulu"],
            "a menu that puts `mike` after `zulu` is a menu whose order nobody \
             can predict"
        );
    }

    /// Forgetting takes one out, and forgetting twice is not an error.
    #[test]
    fn forgetting_removes_one_and_says_nothing_the_second_time() {
        let shipped = workspaces();
        let mine = keep(&[], &shipped, &named("x"), "Wedding").expect("a free name");
        let mine = keep(&mine, &shipped, &named("x"), "Club Night").expect("a free name");
        assert_eq!(mine.len(), 2);

        let after = forget(&mine, " wedding ");
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "Club Night");
        assert_eq!(
            forget(&after, "Wedding").len(),
            1,
            "forgetting something already gone removed something else"
        );
    }

    /// What is saved is the arrangement, not the name it was saved from.
    #[test]
    fn saving_keeps_the_arrangement_and_takes_the_new_name() {
        let shipped = workspaces();
        let mut edited = shipped
            .iter()
            .find(|w| !w.surfaces.is_empty())
            .cloned()
            .expect("something ships with a panel in it");
        edited.decks = 4;
        let was = edited.surfaces.clone();

        let mine = keep(&[], &shipped, &edited, "Mine").expect("a free name");
        assert_eq!(mine[0].name, "Mine", "the preset's name came with it");
        assert_eq!(mine[0].surfaces, was, "the panels did not come with it");
        assert_eq!(mine[0].decks, 4);
    }

    // -- §77's two modes ---------------------------------------------------

    /// **The load-bearing one: the DJ's focus quietens the interface and never
    /// makes it louder.**
    ///
    /// The asymmetry is the whole of §77 as djmanzo can honour it. A DJ who
    /// chose a performing workspace has said "keep out of my way tonight", and
    /// that has to hold *between* records too — the machine has no standing to
    /// decide they have relaxed. A DJ who chose a learning workspace has said
    /// something much weaker, and if two decks are audible the room can hear
    /// them: an interface that got chattier on the strength of a menu choice
    /// made an hour earlier would be talking over a live mix.
    ///
    /// Both directions, because each alone passes wrongly — a `max` passes the
    /// first half and inverts the second, and ignoring the focus entirely
    /// passes the second half and leaves §77 where it was.
    #[test]
    fn a_chosen_focus_may_quieten_the_interface_and_never_loosen_it() {
        let mixing = Attention::performing();
        let between = Attention::preparing();

        // Chose performing, machine says there is room to think: stay quiet.
        let kept = between.quieter_of(Focus::Performing.attention());
        assert_eq!(
            kept, mixing,
            "a DJ who asked to be left alone got the chattier budget the moment \
             a record ended"
        );

        // Chose learning, machine says a mix is running: stay quiet anyway.
        let still_quiet = mixing.quieter_of(Focus::Learning.attention());
        assert_eq!(
            still_quiet, mixing,
            "a workspace chosen an hour ago made the interface talk over a live \
             mix"
        );
    }

    /// It is quieter on every axis, not on whichever one happens to differ.
    #[test]
    fn quieter_means_quieter_in_every_way_at_once() {
        let loud = Attention {
            promoted_controls: 8,
            suggestions: 5,
            notices: 3,
            reflow: true,
            room_for: crate::tiers::Tier::Preparation,
            motion: Motion::High,
        };
        let quiet = Attention {
            promoted_controls: 4,
            suggestions: 0,
            notices: 1,
            reflow: false,
            room_for: crate::tiers::Tier::Glanceable,
            motion: Motion::None,
        };
        assert_eq!(loud.quieter_of(quiet), quiet);
        assert_eq!(
            quiet.quieter_of(loud),
            quiet,
            "the order changed the answer"
        );

        // And a mixed pair takes the quieter half of each.
        let mixed = Attention {
            promoted_controls: 8,
            suggestions: 0,
            notices: 3,
            reflow: false,
            room_for: crate::tiers::Tier::Preparation,
            motion: Motion::High,
        };
        let other = Attention {
            promoted_controls: 4,
            suggestions: 5,
            notices: 1,
            reflow: true,
            room_for: crate::tiers::Tier::Performable,
            motion: Motion::Low,
        };
        assert_eq!(
            mixed.quieter_of(other),
            Attention {
                promoted_controls: 4,
                suggestions: 0,
                notices: 1,
                reflow: false,
                room_for: crate::tiers::Tier::Performable,
                motion: Motion::Low,
            }
        );
    }

    /// Every focus a workspace can name leaves the emergency budget alone.
    ///
    /// §18's emergency is already the quietest thing djmanzo has, and a focus
    /// that could raise it would let a menu choice put advice in front of a DJ
    /// whose recording has just failed.
    #[test]
    fn no_focus_can_talk_over_an_emergency() {
        let emergency = Attention::emergency();
        for focus in [
            Focus::Performing,
            Focus::Preparing,
            Focus::Planning,
            Focus::Learning,
            Focus::Supervising,
        ] {
            assert_eq!(
                emergency.quieter_of(focus.attention()),
                emergency,
                "`{focus:?}` made an emergency louder"
            );
        }
    }

    /// Nothing that ships arrives locked.
    ///
    /// A workspace is a starting point (§7), and one that opened with the layout
    /// pinned would be a preset a DJ cannot edit without first finding out why.
    #[test]
    fn no_shipped_workspace_arrives_locked() {
        for workspace in workspaces() {
            assert!(
                workspace.locked.is_empty(),
                "`{}` ships with a lock on it",
                workspace.name
            );
        }
    }
}

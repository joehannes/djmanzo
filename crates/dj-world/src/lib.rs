//! The interface's world model.
//!
//! Per [ADR-0009](../../../docs/adr/0009-the-living-interface.md), djmanzo's
//! interface is a watershed — decks as rivers, the crossfader as a confluence,
//! the master as an estuary — and this crate is the world itself, knowing
//! nothing about canvases, WebGL, the DOM or Tauri.
//!
//! # Why the world is small
//!
//! The renderer draws hundreds of particles; the world describes none of them. A
//! river says *how fast it flows, where its crest is, how wide it runs, how
//! clear it is and what colour it carries* — a handful of numbers — and the
//! renderer expands that into whatever it draws. So the world crosses to the
//! interface sixty times a second for almost nothing, and swapping the renderer
//! changes no part of this crate.
//!
//! That is the same relationship [ADR-0003](../../../docs/adr/0003-action-bus-and-parameter-registry.md)
//! sets up for behaviour: the renderer is a client of the world exactly as the
//! interface is a client of the action bus.
//!
//! # Everything here already exists
//!
//! Every input this crate takes is something djmanzo already publishes in its
//! 60 Hz snapshot. Nothing new is measured. That is the main practical argument
//! that the living interface is buildable rather than aspirational.
//!
//! # What this crate does *not* do
//!
//! It does not lay anything out. Where a river runs on screen is the renderer's
//! business, bounded by [`bounds`]; the world says what the river *is*.

pub mod bounds;
pub mod palette;

pub use bounds::{Excursion, Vitality};
pub use palette::{Confluence, Tint, confluence, hue_of};

use dj_core::MusicalKey;
use serde::{Deserialize, Serialize};

/// The shape family a thing belongs to.
///
/// Four, and deliberately few: a vocabulary a DJ can learn in a moment is worth
/// more than one that can express everything. Anything that does not fit is a
/// sign the metaphor is being stretched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Form {
    /// Moving water: a deck, the master, anything carrying sound along.
    Flow,
    /// Water turning back on itself: a loop.
    Eddy,
    /// A fixed point you can return to: a cue, a grid anchor.
    Marker,
    /// A region with a property rather than a boundary: level, weather, mist.
    Field,
}

/// Whether a thing bears weight.
///
/// The rule that keeps the interface usable, and it comes from the metaphor
/// rather than fighting it: **a trunk is rigid and bears weight; foliage moves
/// and carries the light.** Nothing in nature asks you to stand on something
/// swaying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bearing {
    /// A DJ clicks, drags or aims at this. It does not move, and it is a real
    /// focusable element in the document with a name a screen reader can read.
    Trunk,
    /// This reports state. It may grow, drift and pulse within [`bounds`], and
    /// it is drawn into the world rather than being an element.
    Foliage,
}

/// One thing in the world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// The widget name from [ADR-0008](../../../docs/adr/0008-one-widget-vocabulary.md),
    /// e.g. `deck.river`. This is what ties the world to the vocabulary the
    /// controllers, network API and assistant already speak.
    pub name: String,
    /// Which one, when a name has several: almost always the deck number.
    pub index: u8,
    /// Which one *within* that, when a name has several per index: the cue
    /// slot, the EQ band. Zero when the name has only one per deck.
    ///
    /// Two levels rather than a flattened number because the two are asked
    /// separately — "deck 2's cues" and "cue 3" are different questions, and a
    /// single index would make one of them arithmetic.
    #[serde(default)]
    pub slot: u8,
    pub form: Form,
    pub bearing: Bearing,
    pub tint: Tint,
    pub vitality: Vitality,
    /// How far along its river this sits, 0..=1. A cue's position, a playhead,
    /// the confluence point.
    pub along: f32,
    /// How much of the channel it occupies, 0..=1. Level, loop length, width.
    pub extent: f32,
    /// What this stands for in words and numbers, so a still frame is legible
    /// and a screen reader has something to say.
    ///
    /// Present on every entity, not only the interactive ones: rule 4 of
    /// ADR-0009 is that nature carries the gestalt and digits carry the
    /// precision, and an entity with no reading is one the DJ cannot check.
    pub reading: String,
}

/// What currently owns the peripheral channel, if anything.
///
/// Exactly one at a time, and the highest-ranked. Peripheral attention is close
/// to a single channel: three things claiming it at once means none of them
/// arrive, and an interface this alive becomes one a DJ learns to ignore --
/// which is worse than a still one.
///
/// Everything *not* holding the channel still shows its state, as static form.
/// Losing the alarm is losing the motion, not losing the information.
///
/// The order of the variants is the ranking, and `Ord` is derived from it, so
/// the rule and the code cannot drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Alarm {
    /// The audience is hearing it right now.
    Dropouts,
    /// A playing deck about to end with nothing else playing. The only
    /// unrecoverable one: silence in the room is not something you fix after.
    RunningOut { deck: u8 },
    /// The mix is being damaged while it plays.
    Limiting,
    /// Expected, and handled -- something else is already playing.
    EndingSoon { deck: u8 },
}

/// How two rivers stand relative to each other in time.
///
/// The single most valuable thing the confluence can say, because the three
/// states are **three different actions**: locked is nothing to do, an offset is
/// a nudge, and a slide is the pitch fader. A DJ reading "out of sync" learns
/// only that something is wrong; reading which of these it is tells them which
/// control to reach for.
///
/// Not a number, deliberately. The signed offset is carried inside `Offset`
/// because it says which way to nudge, but the *category* is what the interface
/// draws — nobody nudges by 0.13 of a beat, they nudge until the crests meet.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Beating {
    /// Nothing to compare: one side or both has no grid, or nothing is playing.
    ///
    /// The default, because an empty world has no two rivers to compare and
    /// every other variant would be an assertion about decks that are not there.
    #[default]
    Unknown,
    /// Same tempo, crests together. Nothing to do.
    Locked,
    /// Same tempo, crests apart. A nudge fixes it, and the sign says which way:
    /// positive means the right bank is ahead.
    Offset { beats: f32 },
    /// The tempos differ, so the offset is *changing*. A nudge would not hold;
    /// this is the pitch fader's problem. Positive means the right bank is
    /// faster.
    Sliding { bpm_difference: f32 },
}

/// Beats of phase difference inside which two rivers count as together.
///
/// An eighth of a beat is 59 ms at 128 BPM — about where a listener stops
/// hearing "slightly early" and starts hearing two separate events. The same
/// figure the beat-tracking regression harness uses for phase, and for the same
/// reason.
pub const LOCKED_WITHIN_BEATS: f32 = 0.125;

/// Tempo difference inside which two rivers count as the same tempo.
///
/// Sync locks tempo exactly, so anything above this is a DJ riding the pitch
/// fader or two decks that were never synced. Tight, because a tenth of a BPM
/// over four bars is already a visible slide.
pub const SAME_TEMPO_WITHIN_BPM: f32 = 0.05;

/// Seconds of track left before the end is worth a peripheral claim.
///
/// Thirty seconds is about a phrase and a half at club tempo: long enough to
/// choose a record and get it cued, short enough that it is genuinely now.
pub const ENDING_SOON_SECONDS: f32 = 30.0;

/// Everything on screen, once.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct World {
    pub entities: Vec<Entity>,
    /// What happens where the two crossfader sides meet.
    pub confluence: Confluence,
    /// How hard the machine is working, 0..=1. Drawn as weather.
    pub strain: f32,
    /// The one thing allowed to move in the corner of a DJ's eye.
    pub alarm: Option<Alarm>,
    /// How much of the collection is still under mist, 0..=1. Zero when there
    /// is nothing left to survey, which is the normal state.
    pub unsurveyed: f32,
    /// How the two banks stand relative to each other in time.
    pub beating: Beating,
}

/// What the world needs to know about one deck.
///
/// Its own type rather than the application's snapshot, so this crate stays
/// below `dj-app` in the dependency order and can be tested without one.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverReading {
    pub deck: u8,
    pub loaded: bool,
    pub playing: bool,
    /// 0..=1 through the track.
    pub progress: f32,
    /// Seconds left. Drawn as distance to the mouth.
    pub remaining_seconds: f32,
    /// Tempo actually playing, pitch included. `None` when there is no grid,
    /// which is different from zero and is drawn differently.
    pub bpm: Option<f32>,
    /// Where in the bar the beat is, 0..=1. The crest.
    pub beat_phase: f32,
    /// How far the grid is trusted, 0..=1. Drawn as clarity of the water.
    pub grid_confidence: f32,
    pub key: Option<MusicalKey>,
    pub key_confidence: f32,
    /// Fader level, 0..=1.
    pub level: f32,
    /// Instantaneous peak, 0..=1. Drawn as surface agitation.
    pub peak: f32,
    /// True while the analyser has not finished. Drawn as mist.
    pub surveying: bool,
    /// The three strata of the water column, low to high. 0.0 is a killed
    /// band -- drought at that stratum -- and 1.0 is unity.
    pub eq: [f32; 3],
    /// -1.0 full low-pass through 0.0 off to 1.0 full high-pass.
    pub filter: f32,
    /// The loop repeating right now, as fractions of the track, with its
    /// length in beats for the reading.
    pub loop_region: Option<LoopRegion>,
    /// Hot cue positions as fractions of the track, slot 1 first. `None` for an
    /// empty slot, which is not the same as a cue at the very start.
    pub cues: Vec<Option<f32>>,
}

/// Water circulating instead of passing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopRegion {
    /// Where the eddy starts, 0..=1 through the track.
    pub start: f32,
    /// How much of the track it covers, 0..=1.
    pub length: f32,
    /// Its length in beats, when the deck has a grid to measure it against.
    pub beats: Option<f32>,
}

/// What the world needs to know about the collection.
///
/// The highland is where the rivers come from: tracks not yet flowing. Its one
/// live fact is how much of it has been *surveyed* — the background identifier
/// decoding files one at a time — which is drawn as mist retreating rather than
/// as a progress bar, because a DJ wants to know whether their collection is
/// usable yet, not what percentage a worker is at.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HighlandReading {
    /// Files waiting to be identified.
    pub unsurveyed: u32,
    /// Tracks identified so far in this run.
    pub surveyed: u32,
    /// Files that could not be identified, and will not be retried.
    pub dry: u32,
}

/// What the world needs to know about the room, rather than any one deck.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RoomReading {
    /// How hard the machine is working, 0..=1.
    pub strain: f32,
    /// True when the audience has heard a dropout.
    pub dropouts: bool,
    /// How hard the master limiter is working, in positive decibels.
    pub limiting_db: f32,
    /// Crossfader, -1.0 hard left to 1.0 hard right.
    pub crossfader: f32,
    /// What the estuary carries, 0..=1.
    pub master_level: f32,
}

impl RiverReading {
    /// A deck with nothing on it.
    #[must_use]
    pub fn empty(deck: u8) -> Self {
        Self {
            deck,
            loaded: false,
            playing: false,
            progress: 0.0,
            remaining_seconds: 0.0,
            bpm: None,
            beat_phase: 0.0,
            grid_confidence: 0.0,
            key: None,
            key_confidence: 0.0,
            level: 0.0,
            peak: 0.0,
            surveying: false,
            eq: [1.0, 1.0, 1.0],
            filter: 0.0,
            loop_region: None,
            cues: Vec::new(),
        }
    }
}

/// Build the world.
///
/// `left` and `right` name which rivers meet at the confluence — the two decks
/// the crossfader actually cuts between. Passed in rather than assumed to be 1
/// and 2, because with four decks the assignment is a choice the DJ makes.
#[must_use]
pub fn build(
    rivers: &[RiverReading],
    left: u8,
    right: u8,
    room: RoomReading,
    highland: HighlandReading,
) -> World {
    let mut entities = Vec::with_capacity(rivers.len() * 2 + 1);

    for river in rivers {
        entities.push(river_entity(river));
        // The mouth exists only when there is something flowing toward it.
        // Drawing an end for a deck with no track would be the interface
        // announcing an ending that is not coming.
        if river.loaded {
            entities.push(mouth_entity(river));
            entities.extend(strata_entities(river));
            if let Some(shear) = filter_entity(river) {
                entities.push(shear);
            }
            if let Some(eddy) = eddy_entity(river) {
                entities.push(eddy);
            }
            entities.extend(stone_entities(river));
        }
    }

    let key_of = |deck: u8| {
        rivers
            .iter()
            .find(|r| r.deck == deck && r.loaded)
            .and_then(|r| r.key)
    };

    let bank = |deck: u8| rivers.iter().find(|r| r.deck == deck && r.loaded);
    entities.push(confluence_entity(rivers, left, right, room));

    World {
        entities,
        confluence: confluence(key_of(left), key_of(right)),
        strain: room.strain.clamp(0.0, 1.0),
        alarm: alarm(rivers, room),
        beating: beating(bank(left), bank(right)),
        unsurveyed: {
            // Of what this run knows about, not of the whole library: a DJ who
            // adds forty files to a collection of four thousand is waiting on
            // forty, and a figure computed against the four thousand would say
            // "1%" and mean nothing.
            let total = highland.unsurveyed + highland.surveyed + highland.dry;
            if total == 0 {
                0.0
            } else {
                highland.unsurveyed as f32 / total as f32
            }
        },
    }
}

/// Where the two rivers meet.
///
/// `along` is the crossfader, mapped from -1..1 to 0..1, because that is
/// literally what the control does in the world: it says *where* the merge
/// happens and therefore which side dominates downstream.
fn confluence_entity(rivers: &[RiverReading], left: u8, right: u8, room: RoomReading) -> Entity {
    let of = |deck: u8| rivers.iter().find(|r| r.deck == deck && r.loaded);
    let (a, b) = (of(left), of(right));

    // The estuary's colour is the mix's, so it is the two banks' hues weighted
    // by how much of each is getting through. A confluence carrying one river
    // is that river's colour; carrying neither is grey, which is the honest
    // answer for a mixer with nothing on it.
    let position = ((room.crossfader.clamp(-1.0, 1.0)) + 1.0) / 2.0;
    let tint = match (a.and_then(|r| r.key), b.and_then(|r| r.key)) {
        (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
            let dominant = if position < 0.5 { a } else { b };
            dominant
                .map(|r| Tint::musical(r.key, r.key_confidence, room.master_level))
                .unwrap_or_else(|| Tint::structural(0.3))
        }
        (None, None) => Tint::structural(0.3),
    };

    Entity {
        name: "mixer.confluence".to_owned(),
        index: 0,
        slot: 0,
        form: Form::Flow,
        bearing: Bearing::Foliage,
        tint,
        vitality: Vitality {
            // The confluence pulses on whichever bank is dominant, so a DJ
            // watching only the estuary still sees the beat they are playing.
            ..match if position < 0.5 { a } else { b } {
                Some(river) => Vitality::of(river),
                None => Vitality::still(),
            }
        },
        along: position,
        // Constriction: how much of the channel the limiter has taken away.
        // Six decibels of reduction is the mix being visibly squeezed.
        extent: (1.0 - (room.limiting_db.max(0.0) / 6.0)).clamp(0.0, 1.0),
        reading: describe_confluence(room),
    }
}

fn describe_confluence(room: RoomReading) -> String {
    let mut parts = Vec::new();
    let position = room.crossfader.clamp(-1.0, 1.0);
    parts.push(if position <= -0.98 {
        "hard left".to_owned()
    } else if position >= 0.98 {
        "hard right".to_owned()
    } else if position.abs() < 0.02 {
        "centre".to_owned()
    } else {
        format!("{:+.0}%", position * 100.0)
    });
    if room.limiting_db > 0.1 {
        // Said in decibels, because the constriction is the gestalt and this is
        // the precision -- see VISUAL-LANGUAGE.md §7.
        parts.push(format!("limiting {:.1} dB", room.limiting_db));
    }
    parts.join(" · ")
}

/// How the two banks stand relative to each other in time.
///
/// The order of the checks is the order of the questions a DJ asks: is there
/// anything to compare, are the tempos the same, and only then are the crests
/// together. Checking phase first would report a meaningless offset for two
/// decks at different tempos, where the offset is not a fact but a moment.
fn beating(left: Option<&RiverReading>, right: Option<&RiverReading>) -> Beating {
    let (Some(a), Some(b)) = (left, right) else {
        return Beating::Unknown;
    };
    if !a.playing || !b.playing {
        return Beating::Unknown;
    }
    let (Some(bpm_a), Some(bpm_b)) = (a.bpm, b.bpm) else {
        return Beating::Unknown;
    };
    if !bpm_a.is_finite() || !bpm_b.is_finite() || bpm_a <= 0.0 || bpm_b <= 0.0 {
        return Beating::Unknown;
    }

    let difference = bpm_b - bpm_a;
    if difference.abs() > SAME_TEMPO_WITHIN_BPM {
        return Beating::Sliding {
            bpm_difference: difference,
        };
    }

    // Signed, and wrapped to the nearer half: a crest 0.9 of a beat ahead is
    // 0.1 behind, and telling a DJ to nudge forward nine tenths of a beat when
    // a tenth back would do is the interface being unhelpful on a technicality.
    let raw = (b.beat_phase - a.beat_phase).rem_euclid(1.0);
    let offset = if raw > 0.5 { raw - 1.0 } else { raw };

    if offset.abs() <= LOCKED_WITHIN_BEATS {
        Beating::Locked
    } else {
        Beating::Offset { beats: offset }
    }
}

/// The three strata of the water column.
///
/// A real river has them and so does an isolator EQ, which is why this is the
/// EQ's shape rather than three knobs in a row: low is the deep current, mid is
/// the body, high is the surface light. A DJ swapping lows on a transition sees
/// the deep current pass from one river to the other, which is precisely what
/// they are doing.
fn strata_entities(river: &RiverReading) -> Vec<Entity> {
    const NAMES: [&str; 3] = ["low", "mid", "high"];
    river
        .eq
        .iter()
        .enumerate()
        .map(|(band, &gain)| {
            let gain = if gain.is_finite() { gain.max(0.0) } else { 1.0 };
            // A kill is drought at one stratum: total, and a visible
            // discontinuity. It is not a gentle turn of a knob and must not
            // look like one, so the threshold is the engine's own kill point.
            let killed = gain < 0.001;
            Entity {
                name: "deck.stratum".to_owned(),
                index: river.deck,
                // Low at the bottom, high at the top, which is where they are.
                slot: band as u8,
                form: Form::Field,
                bearing: Bearing::Foliage,
                tint: Tint::musical(river.key, river.key_confidence, river.level),
                // A stratum does not pulse on its own; it is part of the river,
                // and two things pulsing at the same tempo beside each other
                // reads as two tempos.
                vitality: Vitality::still(),
                along: band as f32 / 2.0,
                // Unity is 1.0 and the band goes to +12 dB, so the scale runs
                // past full: a boosted stratum genuinely stands higher.
                extent: (gain / 2.0).clamp(0.0, 1.0),
                reading: if killed {
                    format!("{} killed", NAMES[band])
                } else {
                    format!("{} {:.2}", NAMES[band], gain)
                },
            }
        })
        .collect()
}

/// The channel narrowed from one side.
///
/// `along` is where the cut sits in the column — below the middle the surface
/// is being sheared away (low-pass), above it the depth is (high-pass) — and
/// `extent` is how much has gone. `None` when the filter is off, because a
/// filter at noon is not narrowing anything and an entity saying "no cut" is a
/// thing on screen that means nothing.
fn filter_entity(river: &RiverReading) -> Option<Entity> {
    let filter = if river.filter.is_finite() {
        river.filter.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    // The same dead zone the interface uses for "off", so the world and the
    // readout agree about when a filter is doing nothing.
    if filter.abs() <= 0.02 {
        return None;
    }
    Some(Entity {
        name: "deck.shear".to_owned(),
        index: river.deck,
        slot: 0,
        form: Form::Field,
        bearing: Bearing::Foliage,
        tint: Tint::structural(0.55),
        vitality: Vitality::still(),
        along: (filter + 1.0) / 2.0,
        extent: filter.abs(),
        reading: if filter < 0.0 {
            format!("low-pass {:.0}%", -filter * 100.0)
        } else {
            format!("high-pass {:.0}%", filter * 100.0)
        },
    })
}

/// Water circulating instead of passing.
///
/// The clearest case in the whole system: an eddy is *literally* what a loop is,
/// and a DJ who has never seen this interface will recognise a whirl in the
/// water without reading a word.
fn eddy_entity(river: &RiverReading) -> Option<Entity> {
    let region = river.loop_region?;
    Some(Entity {
        name: "deck.eddy".to_owned(),
        index: river.deck,
        slot: 0,
        form: Form::Eddy,
        bearing: Bearing::Foliage,
        tint: Tint::musical(river.key, river.key_confidence, river.level),
        // An eddy turns at the track's tempo, which is what makes a halved loop
        // visibly turn twice as fast for the same water.
        vitality: Vitality::of(river),
        along: region.start.clamp(0.0, 1.0),
        extent: region.length.clamp(0.0, 1.0),
        reading: match region.beats {
            Some(beats) if beats >= 1.0 => format!("loop {}", format_beats(beats)),
            Some(beats) => format!("loop {}", format_beats(beats)),
            // A loop set by hand on a track with no grid has a real length in
            // seconds and no length in beats, and saying "loop" alone is more
            // honest than inventing a beat count.
            None => "loop".to_owned(),
        },
    })
}

/// "4" for whole loops, "1/4" for halved ones, which is how DJs say them.
fn format_beats(beats: f32) -> String {
    if beats >= 1.0 {
        format!("{}", (beats * 100.0).round() / 100.0)
    } else if beats > 0.0 {
        format!("1/{}", (1.0 / beats).round())
    } else {
        "0".to_owned()
    }
}

/// Stones in the river: fixed, named landmarks a DJ can see from upstream.
fn stone_entities(river: &RiverReading) -> Vec<Entity> {
    river
        .cues
        .iter()
        .enumerate()
        .filter_map(|(slot, position)| {
            let at = (*position)?;
            if !at.is_finite() {
                return None;
            }
            Some(Entity {
                name: "deck.stone".to_owned(),
                index: river.deck,
                // 1-based, as the pads are labelled. A DJ counting cues counts
                // from one, and an off-by-one here would show in a tooltip.
                slot: slot as u8 + 1,
                form: Form::Marker,
                bearing: Bearing::Foliage,
                // Structural: a cue is a place, not a sound, and giving it the
                // key's hue would put two meanings on one channel.
                tint: Tint::structural(0.75),
                vitality: Vitality::still(),
                along: at.clamp(0.0, 1.0),
                extent: 0.0,
                reading: format!("cue {}", slot + 1),
            })
        })
        .collect()
}

/// Gain reduction past which the limiter is doing real damage rather than
/// catching the odd peak.
///
/// Three decibels. Below that a limiter is doing its job quietly; above it the
/// mix is being reshaped, and a DJ who cannot hear it over a loud room should
/// be able to see it.
const LIMITING_DB: f32 = 3.0;

/// Which single claim owns the peripheral channel.
///
/// The ranking lives in [`Alarm`]'s variant order, so this only has to decide
/// which claims are *active* and take the strongest.
fn alarm(rivers: &[RiverReading], room: RoomReading) -> Option<Alarm> {
    let mut claims = Vec::new();

    if room.dropouts {
        claims.push(Alarm::Dropouts);
    }
    if room.limiting_db > LIMITING_DB {
        claims.push(Alarm::Limiting);
    }

    for river in rivers.iter().filter(|r| r.loaded && r.playing) {
        if river.remaining_seconds > ENDING_SOON_SECONDS {
            continue;
        }
        // Whether anything else is actually carrying the room decides which of
        // these two this is, and they are not the same event: one is a track
        // ending, the other is the music stopping.
        let covered = rivers.iter().any(|other| {
            other.deck != river.deck
                && other.loaded
                && other.playing
                && other.level > 0.01
                && other.remaining_seconds > ENDING_SOON_SECONDS
        });
        claims.push(if covered {
            Alarm::EndingSoon { deck: river.deck }
        } else {
            Alarm::RunningOut { deck: river.deck }
        });
    }

    claims.into_iter().min()
}

fn river_entity(river: &RiverReading) -> Entity {
    // Clarity is the grid's confidence, and it is why the water is drawn murky
    // rather than a tooltip explaining why Sync is disabled: you do not navigate
    // water you cannot see through. An unloaded deck is not murky, it is absent.
    let clarity = if river.loaded {
        river.grid_confidence.clamp(0.0, 1.0)
    } else {
        0.0
    };

    Entity {
        name: "deck.river".to_owned(),
        index: river.deck,
        slot: 0,
        form: Form::Flow,
        bearing: Bearing::Foliage,
        tint: Tint::musical(river.key, river.key_confidence, river.level),
        vitality: Vitality::of(river),
        along: river.progress.clamp(0.0, 1.0),
        extent: river.level.clamp(0.0, 1.0),
        reading: describe(river),
        // Clarity travels in the vitality's turbidity, below.
    }
    .with_turbidity(1.0 - clarity)
}

/// The end of the track, drawn as the distance to it.
///
/// Its own entity rather than a property of the river, because it is the one
/// thing a DJ wants visible in peripheral vision while looking at something
/// else — and because "how long have I got" is a question in
/// VISUAL-LANGUAGE.md's table with nothing else answering it.
fn mouth_entity(river: &RiverReading) -> Entity {
    // Beyond two minutes the end is not yet news; inside thirty seconds it is
    // the only news. A DJ's attention curve, not a linear one.
    const HORIZON: f32 = 120.0;
    let nearness = if river.remaining_seconds <= 0.0 {
        1.0
    } else {
        (1.0 - (river.remaining_seconds / HORIZON)).clamp(0.0, 1.0)
    };

    Entity {
        name: "deck.mouth".to_owned(),
        index: river.deck,
        slot: 0,
        form: Form::Marker,
        bearing: Bearing::Foliage,
        // Structural rather than musical: the end of a track is a fact about
        // time, not about the music, and giving it the key's hue would put two
        // meanings on one channel.
        tint: Tint::structural(0.3 + nearness * 0.5),
        vitality: Vitality::still(),
        along: 1.0,
        extent: nearness,
        reading: format!("{} left", clock(river.remaining_seconds)),
    }
}

impl Entity {
    fn with_turbidity(mut self, turbidity: f32) -> Self {
        self.vitality.turbidity = turbidity.clamp(0.0, 1.0);
        self
    }
}

/// What an entity says in words, for a still frame and for a screen reader.
fn describe(river: &RiverReading) -> String {
    if !river.loaded {
        return "empty".to_owned();
    }
    let mut parts = Vec::new();
    match river.bpm {
        Some(bpm) => parts.push(format!("{bpm:.1} BPM")),
        // Said rather than left blank: a missing grid is information, and a
        // blank cell reads as a bug.
        None => parts.push("no grid".to_owned()),
    }
    if let Some(key) = river.key {
        parts.push(key.camelot());
    }
    parts.push(clock(river.remaining_seconds));
    if river.surveying {
        parts.push("analysing".to_owned());
    }
    parts.join(" · ")
}

/// `m:ss`, never a bare number of seconds.
fn clock(seconds: f32) -> String {
    let total = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::Mode;

    fn key(hour: u8, mode: Mode) -> MusicalKey {
        MusicalKey::new(hour, mode).unwrap()
    }

    fn playing(deck: u8) -> RiverReading {
        RiverReading {
            deck,
            loaded: true,
            playing: true,
            progress: 0.25,
            remaining_seconds: 180.0,
            bpm: Some(128.0),
            beat_phase: 0.0,
            grid_confidence: 0.9,
            key: Some(key(8, Mode::Minor)),
            key_confidence: 0.8,
            level: 0.8,
            peak: 0.5,
            surveying: false,
            ..RiverReading::empty(deck)
        }
    }

    /// A collection with nothing left to survey, which is the normal state.
    fn quiet() -> HighlandReading {
        HighlandReading::default()
    }

    /// A quiet room at a given strain: nothing dropping out, nothing limiting.
    fn calm(strain: f32) -> RoomReading {
        RoomReading {
            strain,
            dropouts: false,
            limiting_db: 0.0,
            crossfader: 0.0,
            master_level: 0.8,
        }
    }

    fn find<'a>(world: &'a World, name: &str, index: u8) -> Option<&'a Entity> {
        world
            .entities
            .iter()
            .find(|e| e.name == name && e.index == index)
    }

    // -- what exists -------------------------------------------------------

    #[test]
    fn every_deck_gets_a_river() {
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.0), quiet());
        assert!(find(&world, "deck.river", 1).is_some());
        assert!(find(&world, "deck.river", 2).is_some());
    }

    /// A deck with no track has no ending coming, and drawing one would
    /// announce something that is not going to happen.
    #[test]
    fn an_empty_deck_has_a_river_but_no_mouth() {
        let world = build(&[RiverReading::empty(1)], 1, 2, calm(0.0), quiet());
        assert!(find(&world, "deck.river", 1).is_some());
        assert!(find(&world, "deck.mouth", 1).is_none());
    }

    // -- the rules ---------------------------------------------------------

    /// ADR-0009 rule 4. Every entity, not only the interactive ones.
    #[test]
    fn every_entity_can_say_what_it_is() {
        let world = build(
            &[playing(1), RiverReading::empty(2)],
            1,
            2,
            calm(0.3),
            quiet(),
        );
        assert!(!world.entities.is_empty());
        for entity in &world.entities {
            assert!(
                !entity.reading.trim().is_empty(),
                "{} {} has nothing to say in a still frame",
                entity.name,
                entity.index
            );
        }
    }

    /// ADR-0009 rule 1. Anything drawn as foliage must not be something a DJ
    /// aims at, and nothing that moves may be trunk.
    #[test]
    fn nothing_that_moves_bears_weight() {
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.0), quiet());
        for entity in &world.entities {
            if entity.bearing == Bearing::Trunk {
                assert!(
                    entity.vitality.is_still(),
                    "{} bears weight and moves",
                    entity.name
                );
            }
        }
    }

    // -- clarity -----------------------------------------------------------

    #[test]
    fn a_weak_grid_makes_the_water_murky() {
        let mut weak = playing(1);
        weak.grid_confidence = 0.1;
        let world = build(&[weak, playing(2)], 1, 2, calm(0.0), quiet());
        let murky = find(&world, "deck.river", 1).unwrap();
        let clear = find(&world, "deck.river", 2).unwrap();
        assert!(murky.vitality.turbidity > clear.vitality.turbidity);
    }

    #[test]
    fn a_hand_certain_grid_is_clear_water() {
        let mut certain = playing(1);
        certain.grid_confidence = 1.0;
        let world = build(&[certain], 1, 2, calm(0.0), quiet());
        assert!(find(&world, "deck.river", 1).unwrap().vitality.turbidity < 1e-6);
    }

    // -- the mouth ---------------------------------------------------------

    /// The question "how long have I got" answered in peripheral vision: the
    /// end must get *more* visible as it approaches, not merely change.
    #[test]
    fn the_mouth_grows_as_the_track_ends() {
        let far = {
            let mut r = playing(1);
            r.remaining_seconds = 200.0;
            r
        };
        let near = {
            let mut r = playing(1);
            r.remaining_seconds = 10.0;
            r
        };
        let a = build(&[far], 1, 2, calm(0.0), quiet());
        let b = build(&[near], 1, 2, calm(0.0), quiet());
        assert!(
            find(&b, "deck.mouth", 1).unwrap().extent > find(&a, "deck.mouth", 1).unwrap().extent
        );
    }

    #[test]
    fn a_track_beyond_the_horizon_is_not_yet_news() {
        let mut plenty = playing(1);
        plenty.remaining_seconds = 600.0;
        let world = build(&[plenty], 1, 2, calm(0.0), quiet());
        assert_eq!(find(&world, "deck.mouth", 1).unwrap().extent, 0.0);
    }

    /// The end of a track is a fact about time, not about the music. Giving it
    /// the key's hue would put two meanings on one channel.
    #[test]
    fn the_mouth_carries_no_musical_colour() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        assert_eq!(find(&world, "deck.mouth", 1).unwrap().tint.saturation, 0.0);
    }

    // -- strata, shear, eddies and stones -----------------------------------

    fn all<'a>(world: &'a World, name: &str) -> Vec<&'a Entity> {
        world.entities.iter().filter(|e| e.name == name).collect()
    }

    fn slot<'a>(world: &'a World, name: &str, deck: u8, slot: u8) -> Option<&'a Entity> {
        world
            .entities
            .iter()
            .find(|e| e.name == name && e.index == deck && e.slot == slot)
    }

    #[test]
    fn a_loaded_deck_has_three_strata_low_to_high() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        let strata = all(&world, "deck.stratum");
        assert_eq!(strata.len(), 3);
        assert!(
            slot(&world, "deck.stratum", 1, 0)
                .unwrap()
                .reading
                .contains("low")
        );
        assert!(
            slot(&world, "deck.stratum", 1, 1)
                .unwrap()
                .reading
                .contains("mid")
        );
        assert!(
            slot(&world, "deck.stratum", 1, 2)
                .unwrap()
                .reading
                .contains("high")
        );
    }

    #[test]
    fn an_empty_deck_has_no_strata() {
        let world = build(&[RiverReading::empty(1)], 1, 2, calm(0.0), quiet());
        assert!(
            all(&world, "deck.stratum").is_empty(),
            "no river, no water column"
        );
    }

    /// A kill is drought at one stratum: total, and a visible discontinuity. It
    /// is not a gentle turn of a knob and must not read as one.
    #[test]
    fn a_killed_band_is_drought_and_says_so() {
        let mut killed = playing(1);
        killed.eq = [0.0, 1.0, 1.0];
        let world = build(&[killed], 1, 2, calm(0.0), quiet());
        let low = slot(&world, "deck.stratum", 1, 0).unwrap();
        assert_eq!(low.extent, 0.0);
        assert!(low.reading.contains("killed"), "{}", low.reading);
    }

    /// The bands go to +12 dB, so a boosted stratum genuinely stands higher
    /// than unity rather than saturating at it.
    #[test]
    fn a_boosted_band_stands_higher_than_unity() {
        let mut boosted = playing(1);
        boosted.eq = [2.0, 1.0, 1.0];
        let world = build(&[boosted], 1, 2, calm(0.0), quiet());
        assert!(
            slot(&world, "deck.stratum", 1, 0).unwrap().extent
                > slot(&world, "deck.stratum", 1, 1).unwrap().extent
        );
    }

    /// Two things pulsing at the same tempo beside each other reads as two
    /// tempos. The strata belong to the river and do not pulse on their own.
    #[test]
    fn the_strata_do_not_pulse_separately_from_their_river() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        for stratum in all(&world, "deck.stratum") {
            assert!(stratum.vitality.is_still(), "slot {}", stratum.slot);
        }
    }

    /// A filter at noon is not narrowing anything, and an entity saying "no
    /// cut" is a thing on screen that means nothing.
    #[test]
    fn a_filter_at_noon_draws_nothing() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        assert!(all(&world, "deck.shear").is_empty());
    }

    #[test]
    fn a_filter_shears_from_the_side_it_cuts() {
        let sheared = |filter: f32| {
            let mut river = playing(1);
            river.filter = filter;
            let world = build(&[river], 1, 2, calm(0.0), quiet());
            all(&world, "deck.shear").first().copied().cloned()
        };

        let low_pass = sheared(-0.8).expect("a low-pass shears");
        assert!(low_pass.along < 0.5, "a low-pass cuts from the top");
        assert!(
            low_pass.reading.contains("low-pass"),
            "{}",
            low_pass.reading
        );

        let high_pass = sheared(0.8).expect("a high-pass shears");
        assert!(high_pass.along > 0.5, "a high-pass cuts from the bottom");
        assert!(
            (low_pass.extent - high_pass.extent).abs() < 1e-6,
            "same amount cut"
        );
    }

    #[test]
    fn a_deck_with_no_loop_has_no_eddy() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        assert!(all(&world, "deck.eddy").is_empty());
    }

    #[test]
    fn a_loop_is_an_eddy_where_the_loop_is() {
        let mut looping = playing(1);
        looping.loop_region = Some(LoopRegion {
            start: 0.4,
            length: 0.05,
            beats: Some(4.0),
        });
        let world = build(&[looping], 1, 2, calm(0.0), quiet());
        let eddy = all(&world, "deck.eddy")[0];
        assert!((eddy.along - 0.4).abs() < 1e-6);
        assert!((eddy.extent - 0.05).abs() < 1e-6);
        assert_eq!(eddy.reading, "loop 4");
    }

    /// A halved loop is half a beat and matches none of the auto-loop buttons.
    /// DJs say "1/4", not "0.25".
    #[test]
    fn a_halved_loop_reads_as_a_fraction() {
        let mut looping = playing(1);
        looping.loop_region = Some(LoopRegion {
            start: 0.4,
            length: 0.006,
            beats: Some(0.25),
        });
        let world = build(&[looping], 1, 2, calm(0.0), quiet());
        assert_eq!(all(&world, "deck.eddy")[0].reading, "loop 1/4");
    }

    /// A loop set by hand on an ungridded track has a real length in seconds
    /// and none in beats. Saying "loop" is more honest than inventing a count.
    #[test]
    fn a_loop_with_no_grid_does_not_invent_a_beat_count() {
        let mut looping = playing(1);
        looping.bpm = None;
        looping.loop_region = Some(LoopRegion {
            start: 0.4,
            length: 0.05,
            beats: None,
        });
        let world = build(&[looping], 1, 2, calm(0.0), quiet());
        assert_eq!(all(&world, "deck.eddy")[0].reading, "loop");
    }

    /// An empty slot is not a cue at the very start of the track.
    #[test]
    fn empty_cue_slots_are_not_stones_at_zero() {
        let mut cued = playing(1);
        cued.cues = vec![Some(0.1), None, Some(0.6), None];
        let world = build(&[cued], 1, 2, calm(0.0), quiet());
        let stones = all(&world, "deck.stone");
        assert_eq!(stones.len(), 2);
        assert!(stones.iter().all(|s| s.along > 0.0));
    }

    /// The pads are labelled from one, and an off-by-one here shows in a
    /// tooltip.
    #[test]
    fn stones_are_numbered_the_way_the_pads_are() {
        let mut cued = playing(1);
        cued.cues = vec![Some(0.1), Some(0.2), Some(0.3)];
        let world = build(&[cued], 1, 2, calm(0.0), quiet());
        assert_eq!(slot(&world, "deck.stone", 1, 1).unwrap().reading, "cue 1");
        assert_eq!(slot(&world, "deck.stone", 1, 3).unwrap().reading, "cue 3");
        assert!(slot(&world, "deck.stone", 1, 0).is_none(), "no cue zero");
    }

    /// A cue is a place, not a sound. Giving it the key's hue would put two
    /// meanings on one channel.
    #[test]
    fn a_stone_carries_no_musical_colour() {
        let mut cued = playing(1);
        cued.cues = vec![Some(0.5)];
        let world = build(&[cued], 1, 2, calm(0.0), quiet());
        assert_eq!(all(&world, "deck.stone")[0].tint.saturation, 0.0);
    }

    #[test]
    fn nonsense_positions_are_dropped_rather_than_drawn_off_the_river() {
        let mut broken = playing(1);
        broken.cues = vec![Some(f32::NAN), Some(0.5), Some(f32::INFINITY)];
        broken.eq = [f32::NAN, -3.0, 1.0];
        broken.filter = f32::NAN;
        let world = build(&[broken], 1, 2, calm(0.0), quiet());
        assert_eq!(all(&world, "deck.stone").len(), 1, "only the real one");
        for stratum in all(&world, "deck.stratum") {
            assert!(
                (0.0..=1.0).contains(&stratum.extent),
                "slot {}",
                stratum.slot
            );
        }
        assert!(
            all(&world, "deck.shear").is_empty(),
            "NaN is not a filter position"
        );
    }

    /// Two decks' cues must not collide: the same slot on different decks is
    /// two different stones, which is what the two-level index is for.
    #[test]
    fn two_decks_cues_are_told_apart() {
        let mut one = playing(1);
        one.cues = vec![Some(0.1)];
        let mut two = playing(2);
        two.cues = vec![Some(0.9)];
        let world = build(&[one, two], 1, 2, calm(0.0), quiet());
        assert!((slot(&world, "deck.stone", 1, 1).unwrap().along - 0.1).abs() < 1e-6);
        assert!((slot(&world, "deck.stone", 2, 1).unwrap().along - 0.9).abs() < 1e-6);
    }

    // -- the confluence ----------------------------------------------------

    #[test]
    fn the_confluence_reads_the_two_decks_the_crossfader_cuts() {
        let mut a = playing(1);
        a.key = Some(key(8, Mode::Minor));
        let mut b = playing(2);
        b.key = Some(key(2, Mode::Minor));
        let mut c = playing(3);
        c.key = Some(key(8, Mode::Minor));

        assert_eq!(
            build(&[a.clone(), b, c.clone()], 1, 2, calm(0.0), quiet()).confluence,
            Confluence::Seam
        );
        assert_eq!(
            build(&[a, c], 1, 3, calm(0.0), quiet()).confluence,
            Confluence::Same
        );
    }

    /// An unloaded deck has no key to clash with, and reporting a seam would
    /// tell a DJ their mix will fight on no evidence at all.
    #[test]
    fn an_empty_side_of_the_confluence_is_unknown() {
        let world = build(
            &[playing(1), RiverReading::empty(2)],
            1,
            2,
            calm(0.0),
            quiet(),
        );
        assert_eq!(world.confluence, Confluence::Unknown);
    }

    // -- readings ----------------------------------------------------------

    #[test]
    fn a_river_reads_out_its_numbers() {
        let world = build(&[playing(1)], 1, 2, calm(0.0), quiet());
        let reading = &find(&world, "deck.river", 1).unwrap().reading;
        assert!(reading.contains("128.0 BPM"), "{reading}");
        assert!(reading.contains("8A"), "{reading}");
        assert!(reading.contains("3:00"), "{reading}");
    }

    /// A missing grid is information. A blank reads as a bug.
    #[test]
    fn a_track_with_no_grid_says_so() {
        let mut ungridded = playing(1);
        ungridded.bpm = None;
        let world = build(&[ungridded], 1, 2, calm(0.0), quiet());
        assert!(
            find(&world, "deck.river", 1)
                .unwrap()
                .reading
                .contains("no grid")
        );
    }

    #[test]
    fn a_track_still_being_analysed_says_so() {
        let mut surveying = playing(1);
        surveying.surveying = true;
        let world = build(&[surveying], 1, 2, calm(0.0), quiet());
        assert!(
            find(&world, "deck.river", 1)
                .unwrap()
                .reading
                .contains("analysing")
        );
    }

    #[test]
    fn time_is_always_a_clock_never_a_bare_number_of_seconds() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(9.4), "0:09");
        assert_eq!(clock(65.0), "1:05");
        assert_eq!(
            clock(-3.0),
            "0:00",
            "a negative remainder is zero, not -1:-3"
        );
    }

    // -- the confluence ----------------------------------------------------

    /// Two rivers at the same tempo with their crests together. The state a DJ
    /// is trying to reach, and the one that needs no action.
    #[test]
    fn matched_decks_read_as_locked() {
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.0), quiet());
        assert_eq!(world.beating, Beating::Locked);
    }

    /// Same tempo, crests apart. A nudge fixes it, and the sign says which way.
    #[test]
    fn a_phase_difference_at_the_same_tempo_is_an_offset() {
        let mut behind = playing(2);
        behind.beat_phase = 0.3;
        let world = build(&[playing(1), behind], 1, 2, calm(0.0), quiet());
        match world.beating {
            Beating::Offset { beats } => assert!((beats - 0.3).abs() < 1e-5, "{beats}"),
            other => panic!("expected an offset, got {other:?}"),
        }
    }

    /// A crest eight tenths of a beat ahead is two tenths behind. Telling a DJ
    /// to nudge forward 0.8 when 0.2 back would do is unhelpful on a
    /// technicality.
    #[test]
    fn an_offset_is_reported_the_short_way_round() {
        let mut nearly_round = playing(2);
        nearly_round.beat_phase = 0.8;
        let world = build(&[playing(1), nearly_round], 1, 2, calm(0.0), quiet());
        match world.beating {
            Beating::Offset { beats } => {
                assert!(beats < 0.0, "the short way is backwards, got {beats}");
                assert!((beats + 0.2).abs() < 1e-5, "{beats}");
            }
            other => panic!("expected an offset, got {other:?}"),
        }
    }

    /// The tolerance and the wrap interact, and the near-miss is the case that
    /// gets it wrong: a tenth of a beat *the long way round* is still a tenth,
    /// and a tenth is inside the locked window.
    #[test]
    fn a_crest_just_the_wrong_side_of_the_downbeat_is_still_locked() {
        let mut a_hair_early = playing(2);
        a_hair_early.beat_phase = 0.95;
        assert_eq!(
            build(&[playing(1), a_hair_early], 1, 2, calm(0.0), quiet()).beating,
            Beating::Locked,
            "0.95 is 0.05 behind, not 0.95 ahead"
        );
    }

    /// The distinction that decides which control a DJ reaches for. Different
    /// tempos mean the offset is changing, so a nudge would not hold.
    #[test]
    fn different_tempos_are_a_slide_not_an_offset() {
        let mut faster = playing(2);
        faster.bpm = Some(130.0);
        // Crests happen to be together *right now*, which is exactly the trap:
        // a phase-only reading would call this locked, and a bar later it is not.
        let world = build(&[playing(1), faster], 1, 2, calm(0.0), quiet());
        match world.beating {
            Beating::Sliding { bpm_difference } => {
                assert!((bpm_difference - 2.0).abs() < 1e-5, "{bpm_difference}");
            }
            other => panic!("expected a slide, got {other:?}"),
        }
    }

    #[test]
    fn a_hair_of_tempo_difference_still_counts_as_the_same_tempo() {
        let mut hair = playing(2);
        hair.bpm = Some(128.0 + SAME_TEMPO_WITHIN_BPM / 2.0);
        assert_eq!(
            build(&[playing(1), hair], 1, 2, calm(0.0), quiet()).beating,
            Beating::Locked
        );
    }

    #[test]
    fn nothing_to_compare_reads_as_unknown() {
        // Only one deck.
        assert_eq!(
            build(&[playing(1)], 1, 2, calm(0.0), quiet()).beating,
            Beating::Unknown
        );

        // One paused: a stopped deck has no crests arriving.
        let mut paused = playing(2);
        paused.playing = false;
        assert_eq!(
            build(&[playing(1), paused], 1, 2, calm(0.0), quiet()).beating,
            Beating::Unknown
        );

        // One ungridded: no beat to compare against.
        let mut ungridded = playing(2);
        ungridded.bpm = None;
        assert_eq!(
            build(&[playing(1), ungridded], 1, 2, calm(0.0), quiet()).beating,
            Beating::Unknown
        );
    }

    #[test]
    fn a_nonsense_tempo_does_not_produce_a_slide() {
        for bad in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
            let mut broken = playing(2);
            broken.bpm = Some(bad);
            assert_eq!(
                build(&[playing(1), broken], 1, 2, calm(0.0), quiet()).beating,
                Beating::Unknown,
                "{bad}"
            );
        }
    }

    #[test]
    fn the_confluence_sits_where_the_crossfader_puts_it() {
        let at = |crossfader: f32| {
            let room = RoomReading {
                crossfader,
                ..calm(0.0)
            };
            find(
                &build(&[playing(1), playing(2)], 1, 2, room, quiet()),
                "mixer.confluence",
                0,
            )
            .unwrap()
            .along
        };
        assert!((at(-1.0) - 0.0).abs() < 1e-5, "hard left");
        assert!((at(0.0) - 0.5).abs() < 1e-5, "centre");
        assert!((at(1.0) - 1.0).abs() < 1e-5, "hard right");
    }

    /// The estuary's banks are fixed; the water is squeezed through them. More
    /// limiting is more constriction, which is a DJ *seeing* the mix crushed
    /// rather than reading a gain-reduction number.
    #[test]
    fn limiting_constricts_the_confluence() {
        let squeezed = |db: f32| {
            let room = RoomReading {
                limiting_db: db,
                ..calm(0.0)
            };
            find(
                &build(&[playing(1), playing(2)], 1, 2, room, quiet()),
                "mixer.confluence",
                0,
            )
            .unwrap()
            .extent
        };
        assert_eq!(squeezed(0.0), 1.0, "nothing to squeeze");
        assert!(squeezed(3.0) < squeezed(0.0));
        assert!(squeezed(9.0) < squeezed(3.0));
        assert!(
            squeezed(9.0) >= 0.0,
            "never negative, however hard it works"
        );
    }

    #[test]
    fn the_confluence_says_where_it_is_and_whether_it_is_limiting() {
        let room = RoomReading {
            crossfader: -1.0,
            limiting_db: 4.2,
            ..calm(0.0)
        };
        let world = build(&[playing(1), playing(2)], 1, 2, room, quiet());
        let reading = &find(&world, "mixer.confluence", 0).unwrap().reading;
        assert!(reading.contains("hard left"), "{reading}");
        assert!(reading.contains("4.2 dB"), "{reading}");
    }

    /// A mixer with nothing on it has no musical colour to show, and inventing
    /// one would be the interface asserting a key nobody has played.
    #[test]
    fn an_empty_confluence_carries_no_hue() {
        let world = build(&[RiverReading::empty(1)], 1, 2, calm(0.0), quiet());
        assert_eq!(
            find(&world, "mixer.confluence", 0).unwrap().tint.saturation,
            0.0
        );
    }

    // -- the highland ------------------------------------------------------

    #[test]
    fn a_surveyed_collection_has_no_mist() {
        assert_eq!(
            build(&[playing(1)], 1, 2, calm(0.0), quiet()).unsurveyed,
            0.0
        );
    }

    /// The figure is of what this run knows about, not of the whole library: a
    /// DJ who adds forty files to a collection of four thousand is waiting on
    /// forty, and a percentage of the four thousand would mean nothing.
    #[test]
    fn the_mist_is_measured_against_this_run_not_the_library() {
        let halfway = HighlandReading {
            unsurveyed: 20,
            surveyed: 20,
            dry: 0,
        };
        let world = build(&[playing(1)], 1, 2, calm(0.0), halfway);
        assert!((world.unsurveyed - 0.5).abs() < 1e-6);
    }

    /// A file that could not be identified is surveyed — the answer was "no".
    /// Counting it as outstanding would leave the mist never clearing.
    #[test]
    fn a_file_that_failed_is_still_surveyed() {
        let done = HighlandReading {
            unsurveyed: 0,
            surveyed: 8,
            dry: 2,
        };
        assert_eq!(build(&[], 1, 2, calm(0.0), done).unsurveyed, 0.0);
    }

    #[test]
    fn an_empty_highland_does_not_divide_by_nothing() {
        assert_eq!(build(&[], 1, 2, calm(0.0), quiet()).unsurveyed, 0.0);
    }

    // -- the alarm channel -------------------------------------------------

    fn ending(deck: u8, seconds: f32) -> RiverReading {
        let mut r = playing(deck);
        r.remaining_seconds = seconds;
        r
    }

    #[test]
    fn a_calm_room_has_no_alarm() {
        assert_eq!(
            build(&[playing(1), playing(2)], 1, 2, calm(0.0), quiet()).alarm,
            None
        );
    }

    /// The whole point of the ranking: exactly one claim gets the channel, and
    /// it is the strongest. Everything else still shows as static form.
    #[test]
    fn only_one_claim_owns_the_channel_and_it_is_the_strongest() {
        let room = RoomReading {
            strain: 1.0,
            dropouts: true,
            limiting_db: 9.0,
            ..calm(0.0)
        };
        // Dropouts, limiting and a deck running out, all at once.
        let world = build(&[ending(1, 5.0)], 1, 2, room, quiet());
        assert_eq!(
            world.alarm,
            Some(Alarm::Dropouts),
            "the audience hearing it now outranks everything"
        );
    }

    #[test]
    fn the_ranking_is_the_variant_order() {
        assert!(Alarm::Dropouts < Alarm::RunningOut { deck: 1 });
        assert!(Alarm::RunningOut { deck: 1 } < Alarm::Limiting);
        assert!(Alarm::Limiting < Alarm::EndingSoon { deck: 1 });
    }

    /// The distinction that matters most in this whole module. A track ending
    /// while something else carries the room is expected and handled; a track
    /// ending with nothing else playing is the room going silent, and those are
    /// not the same event.
    #[test]
    fn a_track_ending_alone_outranks_one_ending_while_another_plays() {
        let alone = build(&[ending(1, 8.0)], 1, 2, calm(0.0), quiet()).alarm;
        assert_eq!(alone, Some(Alarm::RunningOut { deck: 1 }));

        let covered = build(&[ending(1, 8.0), playing(2)], 1, 2, calm(0.0), quiet()).alarm;
        assert_eq!(covered, Some(Alarm::EndingSoon { deck: 1 }));
    }

    /// A deck that is playing but faded out is not carrying the room, so it
    /// does not make the ending deck somebody else's problem.
    #[test]
    fn a_silent_deck_does_not_count_as_covering_the_room() {
        let mut faded = playing(2);
        faded.level = 0.0;
        assert_eq!(
            build(&[ending(1, 8.0), faded], 1, 2, calm(0.0), quiet()).alarm,
            Some(Alarm::RunningOut { deck: 1 })
        );
    }

    /// Nor does a deck that is itself about to end.
    #[test]
    fn a_deck_that_is_also_ending_does_not_cover_the_room() {
        assert_eq!(
            build(&[ending(1, 8.0), ending(2, 6.0)], 1, 2, calm(0.0), quiet()).alarm,
            Some(Alarm::RunningOut { deck: 1 }),
            "two decks both ending is the room going quiet, not a handover"
        );
    }

    #[test]
    fn a_paused_deck_near_its_end_is_not_running_out() {
        let mut paused = ending(1, 3.0);
        paused.playing = false;
        assert_eq!(build(&[paused], 1, 2, calm(0.0), quiet()).alarm, None);
    }

    #[test]
    fn a_track_with_plenty_left_makes_no_claim() {
        assert_eq!(
            build(
                &[ending(1, ENDING_SOON_SECONDS + 1.0)],
                1,
                2,
                calm(0.0),
                quiet()
            )
            .alarm,
            None
        );
    }

    /// A limiter catching the odd peak is doing its job. One reshaping the mix
    /// is something a DJ who cannot hear it over a loud room should see.
    #[test]
    fn a_gently_working_limiter_does_not_take_the_channel() {
        let gentle = RoomReading {
            strain: 0.2,
            limiting_db: 1.0,
            ..calm(0.0)
        };
        assert_eq!(build(&[playing(1)], 1, 2, gentle, quiet()).alarm, None);

        let hard = RoomReading {
            limiting_db: 8.0,
            ..gentle
        };
        assert_eq!(
            build(&[playing(1)], 1, 2, hard, quiet()).alarm,
            Some(Alarm::Limiting)
        );
    }

    // -- robustness --------------------------------------------------------

    #[test]
    fn strain_is_clamped_rather_than_believed() {
        assert_eq!(build(&[], 1, 2, calm(4.0), quiet()).strain, 1.0);
        assert_eq!(build(&[], 1, 2, calm(-1.0), quiet()).strain, 0.0);
    }

    /// A mixer exists whether or not anything is on it — unlike a mouth, which
    /// belongs to a track. So an empty world has the confluence and nothing
    /// else, which is also what the mixer panel shows.
    #[test]
    fn a_world_with_no_decks_has_only_the_mixer() {
        let world = build(&[], 1, 2, calm(0.0), quiet());
        assert_eq!(world.entities.len(), 1);
        assert_eq!(world.entities[0].name, "mixer.confluence");
        assert_eq!(world.confluence, Confluence::Unknown);
        assert_eq!(world.beating, Beating::Unknown);
    }

    /// The world crosses to the interface sixty times a second, so it has to
    /// survive the trip unchanged.
    #[test]
    fn a_world_survives_the_round_trip_through_json() {
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.4), quiet());
        let text = serde_json::to_string(&world).unwrap();
        let back: World = serde_json::from_str(&text).unwrap();
        assert_eq!(back, world);
    }
}

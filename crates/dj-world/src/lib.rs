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
    /// Which one, when a name has several: the deck number, the cue slot.
    pub index: u8,
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
        }
    }
}

/// Build the world.
///
/// `left` and `right` name which rivers meet at the confluence — the two decks
/// the crossfader actually cuts between. Passed in rather than assumed to be 1
/// and 2, because with four decks the assignment is a choice the DJ makes.
#[must_use]
pub fn build(rivers: &[RiverReading], left: u8, right: u8, room: RoomReading) -> World {
    let mut entities = Vec::with_capacity(rivers.len() * 2 + 1);

    for river in rivers {
        entities.push(river_entity(river));
        // The mouth exists only when there is something flowing toward it.
        // Drawing an end for a deck with no track would be the interface
        // announcing an ending that is not coming.
        if river.loaded {
            entities.push(mouth_entity(river));
        }
    }

    let key_of = |deck: u8| {
        rivers
            .iter()
            .find(|r| r.deck == deck && r.loaded)
            .and_then(|r| r.key)
    };

    World {
        entities,
        confluence: confluence(key_of(left), key_of(right)),
        strain: room.strain.clamp(0.0, 1.0),
        alarm: alarm(rivers, room),
    }
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
        }
    }

    /// A quiet room at a given strain: nothing dropping out, nothing limiting.
    fn calm(strain: f32) -> RoomReading {
        RoomReading {
            strain,
            dropouts: false,
            limiting_db: 0.0,
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
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.0));
        assert!(find(&world, "deck.river", 1).is_some());
        assert!(find(&world, "deck.river", 2).is_some());
    }

    /// A deck with no track has no ending coming, and drawing one would
    /// announce something that is not going to happen.
    #[test]
    fn an_empty_deck_has_a_river_but_no_mouth() {
        let world = build(&[RiverReading::empty(1)], 1, 2, calm(0.0));
        assert!(find(&world, "deck.river", 1).is_some());
        assert!(find(&world, "deck.mouth", 1).is_none());
    }

    // -- the rules ---------------------------------------------------------

    /// ADR-0009 rule 4. Every entity, not only the interactive ones.
    #[test]
    fn every_entity_can_say_what_it_is() {
        let world = build(&[playing(1), RiverReading::empty(2)], 1, 2, calm(0.3));
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
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.0));
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
        let world = build(&[weak, playing(2)], 1, 2, calm(0.0));
        let murky = find(&world, "deck.river", 1).unwrap();
        let clear = find(&world, "deck.river", 2).unwrap();
        assert!(murky.vitality.turbidity > clear.vitality.turbidity);
    }

    #[test]
    fn a_hand_certain_grid_is_clear_water() {
        let mut certain = playing(1);
        certain.grid_confidence = 1.0;
        let world = build(&[certain], 1, 2, calm(0.0));
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
        let a = build(&[far], 1, 2, calm(0.0));
        let b = build(&[near], 1, 2, calm(0.0));
        assert!(
            find(&b, "deck.mouth", 1).unwrap().extent > find(&a, "deck.mouth", 1).unwrap().extent
        );
    }

    #[test]
    fn a_track_beyond_the_horizon_is_not_yet_news() {
        let mut plenty = playing(1);
        plenty.remaining_seconds = 600.0;
        let world = build(&[plenty], 1, 2, calm(0.0));
        assert_eq!(find(&world, "deck.mouth", 1).unwrap().extent, 0.0);
    }

    /// The end of a track is a fact about time, not about the music. Giving it
    /// the key's hue would put two meanings on one channel.
    #[test]
    fn the_mouth_carries_no_musical_colour() {
        let world = build(&[playing(1)], 1, 2, calm(0.0));
        assert_eq!(find(&world, "deck.mouth", 1).unwrap().tint.saturation, 0.0);
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
            build(&[a.clone(), b, c.clone()], 1, 2, calm(0.0)).confluence,
            Confluence::Seam
        );
        assert_eq!(build(&[a, c], 1, 3, calm(0.0)).confluence, Confluence::Same);
    }

    /// An unloaded deck has no key to clash with, and reporting a seam would
    /// tell a DJ their mix will fight on no evidence at all.
    #[test]
    fn an_empty_side_of_the_confluence_is_unknown() {
        let world = build(&[playing(1), RiverReading::empty(2)], 1, 2, calm(0.0));
        assert_eq!(world.confluence, Confluence::Unknown);
    }

    // -- readings ----------------------------------------------------------

    #[test]
    fn a_river_reads_out_its_numbers() {
        let world = build(&[playing(1)], 1, 2, calm(0.0));
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
        let world = build(&[ungridded], 1, 2, calm(0.0));
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
        let world = build(&[surveying], 1, 2, calm(0.0));
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

    // -- the alarm channel -------------------------------------------------

    fn ending(deck: u8, seconds: f32) -> RiverReading {
        let mut r = playing(deck);
        r.remaining_seconds = seconds;
        r
    }

    #[test]
    fn a_calm_room_has_no_alarm() {
        assert_eq!(
            build(&[playing(1), playing(2)], 1, 2, calm(0.0)).alarm,
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
        };
        // Dropouts, limiting and a deck running out, all at once.
        let world = build(&[ending(1, 5.0)], 1, 2, room);
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
        let alone = build(&[ending(1, 8.0)], 1, 2, calm(0.0)).alarm;
        assert_eq!(alone, Some(Alarm::RunningOut { deck: 1 }));

        let covered = build(&[ending(1, 8.0), playing(2)], 1, 2, calm(0.0)).alarm;
        assert_eq!(covered, Some(Alarm::EndingSoon { deck: 1 }));
    }

    /// A deck that is playing but faded out is not carrying the room, so it
    /// does not make the ending deck somebody else's problem.
    #[test]
    fn a_silent_deck_does_not_count_as_covering_the_room() {
        let mut faded = playing(2);
        faded.level = 0.0;
        assert_eq!(
            build(&[ending(1, 8.0), faded], 1, 2, calm(0.0)).alarm,
            Some(Alarm::RunningOut { deck: 1 })
        );
    }

    /// Nor does a deck that is itself about to end.
    #[test]
    fn a_deck_that_is_also_ending_does_not_cover_the_room() {
        assert_eq!(
            build(&[ending(1, 8.0), ending(2, 6.0)], 1, 2, calm(0.0)).alarm,
            Some(Alarm::RunningOut { deck: 1 }),
            "two decks both ending is the room going quiet, not a handover"
        );
    }

    #[test]
    fn a_paused_deck_near_its_end_is_not_running_out() {
        let mut paused = ending(1, 3.0);
        paused.playing = false;
        assert_eq!(build(&[paused], 1, 2, calm(0.0)).alarm, None);
    }

    #[test]
    fn a_track_with_plenty_left_makes_no_claim() {
        assert_eq!(
            build(&[ending(1, ENDING_SOON_SECONDS + 1.0)], 1, 2, calm(0.0)).alarm,
            None
        );
    }

    /// A limiter catching the odd peak is doing its job. One reshaping the mix
    /// is something a DJ who cannot hear it over a loud room should see.
    #[test]
    fn a_gently_working_limiter_does_not_take_the_channel() {
        let gentle = RoomReading {
            strain: 0.2,
            dropouts: false,
            limiting_db: 1.0,
        };
        assert_eq!(build(&[playing(1)], 1, 2, gentle).alarm, None);

        let hard = RoomReading {
            limiting_db: 8.0,
            ..gentle
        };
        assert_eq!(
            build(&[playing(1)], 1, 2, hard).alarm,
            Some(Alarm::Limiting)
        );
    }

    // -- robustness --------------------------------------------------------

    #[test]
    fn strain_is_clamped_rather_than_believed() {
        assert_eq!(build(&[], 1, 2, calm(4.0)).strain, 1.0);
        assert_eq!(build(&[], 1, 2, calm(-1.0)).strain, 0.0);
    }

    #[test]
    fn a_world_with_no_decks_is_empty_rather_than_a_panic() {
        let world = build(&[], 1, 2, calm(0.0));
        assert!(world.entities.is_empty());
        assert_eq!(world.confluence, Confluence::Unknown);
    }

    /// The world crosses to the interface sixty times a second, so it has to
    /// survive the trip unchanged.
    #[test]
    fn a_world_survives_the_round_trip_through_json() {
        let world = build(&[playing(1), playing(2)], 1, 2, calm(0.4));
        let text = serde_json::to_string(&world).unwrap();
        let back: World = serde_json::from_str(&text).unwrap();
        assert_eq!(back, world);
    }
}

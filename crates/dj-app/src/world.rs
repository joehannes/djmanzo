//! Turning a snapshot into a world.
//!
//! The mapping between what the engine reports and what
//! [`dj_world`] draws — the one place the application decides that a deck is a
//! river, that grid confidence is how clear the water runs, and that the
//! crossfader's two sides are the banks of a confluence.
//!
//! It lives here rather than in `dj-world` for the same reason `dj-world` has
//! its own input types at all: the world model sits below the application in
//! the dependency order, so it cannot know about [`crate::Snapshot`]. This
//! module is the adapter, and it is deliberately nothing else — no rules, no
//! judgement, just the translation.
//!
//! # Why the world is computed here and not in the interface
//!
//! It would be easy to let the webview build its own world from the snapshot it
//! already receives. That would put the rules in TypeScript, where the network
//! API, the controllers and the assistant cannot reach them, and
//! [ADR-0009](../../../docs/adr/0009-the-living-interface.md) is explicit that
//! the world is not the renderer's to define. Computing it in Rust costs one
//! small serialisation per frame and keeps one answer to what is on screen.

use crate::Snapshot;
use dj_core::{Mode, MusicalKey};
use dj_world::{HighlandReading, LoopRegion, RiverReading, RoomReading, World};

/// Build the world the interface should draw.
///
/// `highland` is passed in rather than read here because the collection lives
/// behind a database handle that may not be open, and the world is built sixty
/// times a second — a query per frame would be the wrong shape entirely.
#[must_use]
pub fn of(snapshot: &Snapshot, highland: HighlandReading) -> World {
    let rivers: Vec<RiverReading> = snapshot.decks.iter().map(river).collect();
    let (left, right) = banks(snapshot);
    dj_world::build(&rivers, left, right, room(snapshot), highland)
}

fn river(deck: &crate::snapshot::DeckSnapshot) -> RiverReading {
    let analysis = deck.analysis.as_ref();
    RiverReading {
        deck: deck.number,
        loaded: deck.loaded,
        playing: deck.playing,
        progress: if deck.length_frames > 0.0 {
            deck.position_frames / deck.length_frames
        } else {
            0.0
        },
        remaining_seconds: (deck.length_seconds - deck.position_seconds).max(0.0),
        // The tempo actually playing, pitch fader included. Zero from the
        // registry means "no grid", which is a different thing from 0 BPM.
        bpm: deck.effective_bpm.filter(|b| *b > 0.0),
        beat_phase: deck.beat_phase,
        grid_confidence: deck.grid_confidence,
        key: analysis
            .and_then(|a| a.key_camelot.as_deref())
            .and_then(camelot),
        key_confidence: analysis.and_then(|a| a.key_confidence).unwrap_or(0.0),
        // The channel fader, per VISUAL-LANGUAGE.md §2: "how much water is in
        // the channel". Not the meter -- the meter is an instantaneous reading
        // that swings with every kick, and a river whose width flickered with
        // the signal would say the fader was being ridden. The signal has its
        // own channel: `peak`, drawn as surface agitation.
        level: deck.volume.clamp(0.0, 1.0),
        peak: deck.peak.clamp(0.0, 1.0),
        // The analyser has not finished while the deck is loaded and has
        // nothing to report. Drawn as mist over an unsurveyed stretch.
        surveying: deck.loaded && analysis.is_none(),
        eq: [deck.eq_low, deck.eq_mid, deck.eq_high],
        filter: deck.filter,
        loop_region: deck.active_loop.as_ref().and_then(|region| {
            // Everything along a river is a fraction of the track, so a loop
            // has to be measured the same way -- and a track with no length
            // yet has nowhere to put one.
            (deck.length_frames > 0.0).then(|| LoopRegion {
                start: region.start_frames / deck.length_frames,
                length: (region.end_frames - region.start_frames) / deck.length_frames,
                beats: region.beats,
            })
        }),
        cues: deck
            .hot_cues
            .iter()
            .map(|cue| {
                cue.and_then(|frames| {
                    (deck.length_frames > 0.0).then_some(frames / deck.length_frames)
                })
            })
            .collect(),
    }
}

/// Which two decks the crossfader actually cuts between.
///
/// Read from the assignments rather than assumed to be 1 and 2: with four decks
/// which pair meets at the confluence is a choice the DJ makes, and drawing the
/// wrong pair's harmonic compatibility would be worse than drawing none.
fn banks(snapshot: &Snapshot) -> (u8, u8) {
    use dj_core::CrossfaderAssign;
    let side = |want: CrossfaderAssign| {
        snapshot
            .decks
            .iter()
            .find(|d| d.loaded && d.crossfader_assign == want)
            .map(|d| d.number)
    };
    (
        side(CrossfaderAssign::Left).unwrap_or(1),
        side(CrossfaderAssign::Right).unwrap_or(2),
    )
}

/// What the world needs to know about the room rather than any one deck.
///
/// Strain keeps xruns and CPU load apart deliberately. A machine at 80% is
/// working hard and coping; a single dropout is the DJ's audience hearing a
/// click, and averaging the two into one calm number would hide the one that
/// matters.
fn room(snapshot: &Snapshot) -> RoomReading {
    let load = snapshot.master.cpu_load.clamp(0.0, 1.0);
    let dropouts = snapshot.master.xruns > 0.0;
    RoomReading {
        strain: if dropouts { 1.0 } else { load },
        dropouts,
        limiting_db: snapshot.master.limiter_reduction_db.max(0.0),
        crossfader: snapshot.master.crossfader.clamp(-1.0, 1.0),
        // The estuary's level, from whichever meter is carrying more. Peak
        // rather than an average of the two: a mix loud in one channel only is
        // still a loud mix, and averaging would draw it as half as much water.
        master_level: snapshot
            .master
            .peak_left
            .max(snapshot.master.peak_right)
            .clamp(0.0, 1.0),
    }
}

/// Parse Camelot notation back into a key.
///
/// The snapshot carries the key as the string the interface displays, so this
/// reads it back rather than the analysis being threaded through a second time.
/// Anything unrecognised is `None` — an unparseable key is not a key, and
/// guessing one would put a confident hue on a track nobody has analysed.
fn camelot(text: &str) -> Option<MusicalKey> {
    let text = text.trim();
    // By character, not by byte. Splitting at `len() - 1` panics the moment the
    // last character is multibyte, and a key string arrives from a tag somebody
    // else wrote -- so `8Å` is a thing that will eventually turn up.
    let mut chars = text.chars();
    let mode = match chars.next_back()? {
        'A' | 'a' => Mode::Minor,
        'B' | 'b' => Mode::Major,
        _ => return None,
    };
    MusicalKey::new(chars.as_str().parse().ok()?, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::CrossfaderAssign;
    use dj_world::Confluence;

    fn deck(number: u8) -> crate::snapshot::DeckSnapshot {
        let mut snapshot = crate::Snapshot::capture(
            &dj_control::ParameterRegistry::new(),
            crate::state::DECK_COUNT,
        );
        snapshot.decks.remove((number - 1) as usize)
    }

    fn loaded(number: u8) -> crate::snapshot::DeckSnapshot {
        let mut d = deck(number);
        d.loaded = true;
        d.playing = true;
        d.length_frames = 1000.0;
        d.position_frames = 250.0;
        d.length_seconds = 200.0;
        d.position_seconds = 50.0;
        d.pre_fader_level = 0.8;
        d.grid_confidence = 0.9;
        d.beat_phase = 0.25;
        d
    }

    /// The world with nothing left to survey, which is the normal state and
    /// keeps these tests about the mapping rather than about the library.
    fn of_calm(snapshot: &Snapshot) -> dj_world::World {
        of(snapshot, HighlandReading::default())
    }

    fn with(decks: Vec<crate::snapshot::DeckSnapshot>) -> Snapshot {
        let mut snapshot = crate::Snapshot::capture(
            &dj_control::ParameterRegistry::new(),
            crate::state::DECK_COUNT,
        );
        snapshot.decks = decks;
        snapshot
    }

    // -- camelot -----------------------------------------------------------

    #[test]
    fn camelot_notation_parses_back_to_a_key() {
        assert_eq!(camelot("8A"), MusicalKey::new(8, Mode::Minor));
        assert_eq!(camelot("12B"), MusicalKey::new(12, Mode::Major));
        assert_eq!(camelot(" 1a "), MusicalKey::new(1, Mode::Minor));
    }

    /// An unparseable key is not a key. Guessing one would put a confident hue
    /// on a track nobody has analysed, which is the exact failure the colour
    /// scheme is built to avoid.
    #[test]
    fn anything_unrecognised_is_no_key_rather_than_a_guess() {
        for bad in ["", "A", "13A", "0A", "8C", "eight-A", "8"] {
            assert_eq!(camelot(bad), None, "{bad:?}");
        }
    }

    /// A single byte would panic a naive `split_at` on a multibyte boundary.
    #[test]
    fn a_multibyte_key_does_not_panic() {
        assert_eq!(camelot("8Å"), None);
        assert_eq!(camelot("♪"), None);
    }

    // -- the mapping -------------------------------------------------------

    #[test]
    fn an_empty_deck_makes_an_empty_river() {
        let world = of_calm(&with(vec![deck(1)]));
        let river = &world.entities[0];
        assert_eq!(river.name, "deck.river");
        assert_eq!(river.reading, "empty");
    }

    #[test]
    fn progress_comes_from_the_playhead() {
        let world = of_calm(&with(vec![loaded(1)]));
        let river = world
            .entities
            .iter()
            .find(|e| e.name == "deck.river")
            .unwrap();
        assert!((river.along - 0.25).abs() < 1e-5);
    }

    /// A track of zero length is not a division by zero, and not a track that
    /// is somehow at its end.
    #[test]
    fn a_zero_length_track_is_at_the_start_rather_than_infinity() {
        let mut empty_length = loaded(1);
        empty_length.length_frames = 0.0;
        let world = of_calm(&with(vec![empty_length]));
        let river = world
            .entities
            .iter()
            .find(|e| e.name == "deck.river")
            .unwrap();
        assert_eq!(river.along, 0.0);
    }

    /// The width is the fader, not the meter. A meter reading swings with
    /// every kick, and a river that flickered with the signal would read as
    /// somebody riding the fader. The signal has its own channel: agitation.
    #[test]
    fn the_river_is_as_wide_as_the_fader_not_as_loud_as_the_moment() {
        let mut quiet_moment = loaded(1);
        quiet_moment.volume = 1.0;
        quiet_moment.pre_fader_level = 0.02;
        let world = of_calm(&with(vec![quiet_moment]));
        let river = world
            .entities
            .iter()
            .find(|e| e.name == "deck.river")
            .unwrap();
        assert_eq!(
            river.extent, 1.0,
            "the fader is open, so the channel is full"
        );

        let mut closed = loaded(1);
        closed.volume = 0.0;
        let world = of_calm(&with(vec![closed]));
        let river = world
            .entities
            .iter()
            .find(|e| e.name == "deck.river")
            .unwrap();
        assert_eq!(river.extent, 0.0, "a closed fader is a dry channel");
    }

    #[test]
    fn a_loaded_deck_with_no_analysis_yet_is_being_surveyed() {
        let world = of_calm(&with(vec![loaded(1)]));
        let river = world
            .entities
            .iter()
            .find(|e| e.name == "deck.river")
            .unwrap();
        assert!(river.reading.contains("analysing"), "{}", river.reading);
    }

    // -- the banks ---------------------------------------------------------

    /// With four decks, which pair meets at the confluence is the DJ's choice.
    /// Drawing the wrong pair's compatibility is worse than drawing none.
    #[test]
    fn the_confluence_follows_the_crossfader_assignments() {
        let mut one = loaded(1);
        one.crossfader_assign = CrossfaderAssign::Thru;
        let mut three = loaded(3);
        three.crossfader_assign = CrossfaderAssign::Left;
        let mut four = loaded(4);
        four.crossfader_assign = CrossfaderAssign::Right;

        let snapshot = with(vec![one, loaded(2), three, four]);
        assert_eq!(banks(&snapshot), (3, 4));
    }

    /// Before anything is loaded there is nothing to read, and the pair a DJ
    /// would expect is the sensible answer rather than a panic.
    #[test]
    fn with_nothing_loaded_the_banks_are_the_obvious_pair() {
        assert_eq!(banks(&with(vec![deck(1), deck(2)])), (1, 2));
    }

    #[test]
    fn two_decks_in_the_same_key_make_one_body_of_water() {
        let mut a = loaded(1);
        a.crossfader_assign = CrossfaderAssign::Left;
        let mut b = loaded(2);
        b.crossfader_assign = CrossfaderAssign::Right;
        for deck in [&mut a, &mut b] {
            deck.analysis = Some(crate::snapshot::TrackAnalysisSnapshot {
                bpm: Some(128.0),
                bpm_confidence: Some(0.9),
                bpm_alternative: None,
                sync_worthy: true,
                key_camelot: Some("8A".to_owned()),
                key_standard: Some("Am".to_owned()),
                key_confidence: Some(0.8),
                key_alternative: None,
                lufs: Some(-9.0),
                auto_gain_db: 0.0,
            });
        }
        assert_eq!(of_calm(&with(vec![a, b])).confluence, Confluence::Same);
    }

    // -- strain ------------------------------------------------------------

    /// A machine at 80% is working hard and coping. One dropout is the audience
    /// hearing a click. Averaging them would hide the one that matters.
    #[test]
    fn a_single_dropout_outweighs_a_busy_but_coping_machine() {
        let mut busy = with(vec![loaded(1)]);
        busy.master.cpu_load = 0.8;
        busy.master.xruns = 0.0;

        let mut dropping = with(vec![loaded(1)]);
        dropping.master.cpu_load = 0.05;
        dropping.master.xruns = 1.0;

        assert!(room(&dropping).strain > room(&busy).strain);
        assert_eq!(room(&dropping).strain, 1.0);
        assert!(room(&dropping).dropouts);
    }

    #[test]
    fn an_idle_machine_has_no_weather() {
        let mut calm = with(vec![loaded(1)]);
        calm.master.cpu_load = 0.0;
        calm.master.xruns = 0.0;
        assert_eq!(room(&calm).strain, 0.0);
        assert!(!room(&calm).dropouts);
    }
}

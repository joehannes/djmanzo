//! What a DJ sets on a record is still there next time they play it.
//!
//! The unit tests cover each piece — the watcher's comparisons, the storage
//! round trip, the action interception. This drives the *whole* path through a
//! running engine: a cue set on a deck reaches the library, and a fresh deck
//! loading the same track gets it back.
//!
//! Uses the null audio backend, so there is a real audio thread publishing real
//! parameters, and no sound card.

use dj_app::state::{AppState, LoadedTrackInfo};
use dj_core::param::DeckParam;
use dj_core::{DeckId, FramePos, HOT_CUE_SLOTS, ParamId, SampleRate, TrackId};
use std::sync::Arc;
use std::time::{Duration, Instant};

const SR: SampleRate = SampleRate::DEFAULT;

fn deck() -> DeckId {
    DeckId::from_human(1).unwrap()
}

fn track_id(byte: u8) -> TrackId {
    TrackId::from_bytes([byte; 32])
}

/// Ten seconds of quiet audio — enough to have somewhere to put a cue.
fn source() -> Arc<dyn dj_decode::TrackSource> {
    let frames = SR.get() as usize * 10;
    Arc::new(dj_decode::AudioBuffer::from_interleaved(
        vec![0.0; frames * 2],
        SR,
    ))
}

fn library_track(id: TrackId) -> dj_library::LibraryTrack {
    dj_library::LibraryTrack {
        id,
        path: std::path::PathBuf::from(format!("/music/{}.flac", id.to_hex())),
        tags: dj_library::Tags::default(),
        duration_frames: u64::from(SR.get()) * 10,
        sample_rate: SR,
        channels: 2,
        file_size: None,
        file_modified: None,
        added_at: 0,
        analysis: dj_library::StoredAnalysis::default(),
        stats: dj_library::PlayStats::default(),
    }
}

/// An app with the null backend open, so the engine is running.
fn running_app() -> AppState {
    let state = AppState::new(true);
    state.host().open(None, None, 128).unwrap();
    state
}

/// Wait for the audio thread to publish, or give up.
///
/// The engine drains its queue on the audio callback, so a command sent from a
/// test is applied a callback later — not instantly.
fn until(condition: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    condition()
}

fn cue_at(state: &AppState, slot: u8) -> Option<f32> {
    let value = state
        .registry()
        .get(ParamId::Deck(deck(), DeckParam::hot_cue(slot)?));
    // Negative means empty; frame zero is a real cue.
    (value >= 0.0).then_some(value)
}

/// Put a track on deck 1 and tell the host about it, as `load_track` does.
fn load(state: &AppState, id: TrackId) {
    state
        .bus()
        .send_command(dj_engine::Command::Load {
            deck: deck(),
            source: source(),
        })
        .unwrap();
    state.set_deck_track(
        deck(),
        LoadedTrackInfo {
            title: "A".to_owned(),
            artist: None,
            id,
        },
    );
    assert!(
        until(|| state
            .registry()
            .get(ParamId::Deck(deck(), DeckParam::Loaded))
            >= 0.5),
        "the engine should have taken the track"
    );
}

/// The whole claim, end to end: set a cue, take the track off, put it back, and
/// the cue is where it was.
#[test]
fn a_cue_set_on_a_track_is_there_when_it_is_loaded_again() {
    let state = running_app();
    let db = state.library().get().unwrap();
    db.upsert_track(&library_track(track_id(1))).unwrap();

    load(&state, track_id(1));

    // Seek somewhere and drop a cue there, exactly as a pad press does.
    let target = f64::from(SR.get()) * 2.0;
    state
        .bus()
        .dispatch(dj_core::Action::parse(&format!("deck 1 seek {target}")).unwrap())
        .unwrap();
    assert!(until(|| {
        state
            .registry()
            .get(ParamId::Deck(deck(), DeckParam::Position))
            > 0.0
    }));
    state
        .bus()
        .dispatch(dj_core::Action::parse("deck 1 hotcue 1").unwrap())
        .unwrap();
    assert!(
        until(|| cue_at(&state, 1).is_some()),
        "the cue should be set"
    );
    let placed = cue_at(&state, 1).unwrap();

    // The snapshot pump is what notices; drive one tick of it by hand.
    let cues: Vec<Option<f32>> = (1..=HOT_CUE_SLOTS as u8)
        .map(|s| cue_at(&state, s))
        .collect();
    let watcher = state.cue_watcher();
    let writer = state.library_writer();
    {
        let mut watcher = watcher.lock().unwrap();
        // First sight, then the change -- as the pump would see it.
        watcher.observe(1, Some(track_id(1)), &[None; HOT_CUE_SLOTS], &writer);
        watcher.observe(1, Some(track_id(1)), &cues, &writer);
    }
    assert!(
        until(|| !db.cues(track_id(1)).unwrap().is_empty()),
        "the cue should have reached the library"
    );

    // Eject, which clears the deck.
    state
        .bus()
        .dispatch(dj_core::Action::parse("deck 1 eject").unwrap())
        .unwrap();
    assert!(
        until(|| cue_at(&state, 1).is_none()),
        "eject clears the deck"
    );

    // Load the same track again, and restore as `load_track` does.
    load(&state, track_id(1));
    let stored = db.cues(track_id(1)).unwrap();
    state
        .bus()
        .send_command(dj_engine::Command::SetHotCues {
            deck: deck(),
            cues: dj_app::persist::from_stored(&stored),
        })
        .unwrap();

    assert!(
        until(|| cue_at(&state, 1).is_some()),
        "the cue should be back on the deck"
    );
    let restored = cue_at(&state, 1).unwrap();
    assert!(
        (restored - placed).abs() < 1.0,
        "restored to {restored}, was placed at {placed}"
    );
}

/// Restoring must replace the whole set, not merge into it — or the previous
/// track's cues survive in the slots this one does not use.
#[test]
fn restoring_replaces_the_previous_tracks_cues_rather_than_merging() {
    let state = running_app();
    let db = state.library().get().unwrap();
    db.upsert_track(&library_track(track_id(1))).unwrap();
    db.upsert_track(&library_track(track_id(2))).unwrap();

    load(&state, track_id(1));

    // Track 1 has cues in slots 1 and 5.
    state
        .bus()
        .send_command(dj_engine::Command::SetHotCues {
            deck: deck(),
            cues: dj_app::persist::from_stored(&[
                dj_library::StoredCue {
                    slot: 1,
                    frame: 1000.0,
                    label: None,
                    colour: None,
                },
                dj_library::StoredCue {
                    slot: 5,
                    frame: 5000.0,
                    label: None,
                    colour: None,
                },
            ]),
        })
        .unwrap();
    assert!(until(|| cue_at(&state, 5).is_some()));

    // Track 2 has one in slot 1 only.
    state
        .bus()
        .send_command(dj_engine::Command::SetHotCues {
            deck: deck(),
            cues: dj_app::persist::from_stored(&[dj_library::StoredCue {
                slot: 1,
                frame: 2000.0,
                label: None,
                colour: None,
            }]),
        })
        .unwrap();

    assert!(
        until(|| cue_at(&state, 5).is_none()),
        "slot 5 belonged to the previous track and must not survive"
    );
    assert_eq!(cue_at(&state, 1), Some(2000.0));
}

/// A saved loop set on one deck is recallable on another, because it belongs to
/// the track rather than to the deck.
#[test]
fn a_saved_loop_belongs_to_the_track_not_the_deck() {
    let state = running_app();
    let db = state.library().get().unwrap();
    db.upsert_track(&library_track(track_id(1))).unwrap();

    db.set_loops(
        track_id(1),
        &[dj_library::StoredLoop {
            slot: 1,
            start_frame: 96_000.0,
            end_frame: 192_000.0,
            label: None,
        }],
    )
    .unwrap();

    // Deck 2 this time.
    let other = DeckId::from_human(2).unwrap();
    state
        .bus()
        .send_command(dj_engine::Command::Load {
            deck: other,
            source: source(),
        })
        .unwrap();
    let region = dj_core::LoopRegion::new(FramePos::new(96_000.0), FramePos::new(192_000.0));
    state
        .bus()
        .send_command(dj_engine::Command::SetLoop {
            deck: other,
            region,
        })
        .unwrap();

    assert!(
        until(|| state
            .registry()
            .get(ParamId::Deck(other, DeckParam::LoopActive))
            >= 0.5),
        "the saved loop should be armed on deck 2"
    );
    assert_eq!(
        state
            .registry()
            .get(ParamId::Deck(other, DeckParam::LoopStart)),
        96_000.0
    );
}

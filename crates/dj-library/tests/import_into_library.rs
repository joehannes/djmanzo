//! What happens when an import meets the identity model.
//!
//! An import names tracks by path; the library keys them by the hash of their
//! decoded audio. Everything interesting is in that gap: what happens to a
//! track already known, what happens to one that is not, and whether a cue
//! survives the wait in between.

use dj_core::{SampleRate, TrackId};
use dj_library::import::{
    Collection, ImportPayload, ImportedCue, ImportedLoop, ImportedPlaylist, ImportedTrack,
};
use dj_library::{Library, LibraryTrack, PlayStats, StoredAnalysis, StoredCue, Tags};
use std::path::PathBuf;

const SR: SampleRate = SampleRate::DEFAULT;

fn id(byte: u8) -> TrackId {
    TrackId::from_bytes([byte; 32])
}

fn known_track(byte: u8, path: &str) -> LibraryTrack {
    LibraryTrack {
        id: id(byte),
        path: PathBuf::from(path),
        tags: Tags::default(),
        duration_frames: u64::from(SR.get()) * 200,
        sample_rate: SR,
        channels: 2,
        file_size: None,
        file_modified: None,
        added_at: 0,
        analysis: StoredAnalysis::default(),
        stats: PlayStats::default(),
        colour: None,
    }
}

fn imported(path: &str) -> ImportedTrack {
    ImportedTrack {
        path: PathBuf::from(path),
        title: Some("Bachata Rosa".to_owned()),
        artist: Some("Juan Luis Guerra".to_owned()),
        genre: Some("Bachata".to_owned()),
        rating: Some(4),
        payload: ImportPayload {
            cues: vec![ImportedCue {
                slot: 1,
                seconds: 32.5,
                label: Some("drop".to_owned()),
                colour: Some("#ff0000".to_owned()),
            }],
            loops: vec![ImportedLoop {
                slot: 1,
                start_seconds: 64.0,
                end_seconds: 80.0,
                label: None,
            }],
            bpm: Some(128.0),
            grid_anchor_seconds: Some(0.025),
            key_hour: Some(8),
            key_minor: Some(true),
        },
        ..ImportedTrack::default()
    }
}

fn collection(tracks: Vec<ImportedTrack>, playlists: Vec<ImportedPlaylist>) -> Collection {
    Collection {
        tracks,
        playlists,
        skipped: Vec::new(),
    }
}

/// A track the library already has: everything applies at once, because there
/// is a row to hang it on.
#[test]
fn importing_a_known_track_applies_its_cues_and_grid_immediately() {
    let lib = Library::in_memory().unwrap();
    lib.upsert_track(&known_track(1, "/music/a.flac")).unwrap();

    let report = lib
        .import(
            &collection(vec![imported("/music/a.flac")], Vec::new()),
            100,
        )
        .unwrap();
    assert_eq!(report.already_known, 1);
    assert_eq!(report.queued, 0);

    let cues = lib.cues(id(1)).unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(
        cues[0].frame,
        32.5 * SR.as_f64(),
        "seconds are converted using the track's own sample rate"
    );
    assert_eq!(cues[0].label.as_deref(), Some("drop"));

    let loops = lib.loops(id(1)).unwrap();
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].end_frame, 80.0 * SR.as_f64());

    let found = lib.track(id(1)).unwrap().unwrap();
    assert_eq!(found.analysis.bpm, Some(128.0));
    assert_eq!(found.analysis.key_hour, Some(8));
    assert_eq!(found.tags.genre.as_deref(), Some("Bachata"));
    assert_eq!(found.stats.rating, Some(4));
}

/// A track nobody has decoded: queued, with its cues waiting.
#[test]
fn importing_an_unknown_track_queues_it_with_its_cues() {
    let lib = Library::in_memory().unwrap();
    let report = lib
        .import(
            &collection(vec![imported("/music/a.flac")], Vec::new()),
            100,
        )
        .unwrap();

    assert_eq!(report.queued, 1);
    assert_eq!(report.already_known, 0);
    assert_eq!(lib.track_count().unwrap(), 0);
    assert_eq!(lib.pending_count().unwrap(), 1);

    // The tags came along, so the browser has something to show meanwhile.
    let pending = lib.next_pending(10).unwrap();
    assert_eq!(pending[0].tags.artist.as_deref(), Some("Juan Luis Guerra"));
}

/// ...and the cues arrive the moment the file is identified. This is the whole
/// point of staging them.
#[test]
fn a_queued_imports_cues_are_applied_when_the_track_is_identified() {
    let lib = Library::in_memory().unwrap();
    lib.import(
        &collection(vec![imported("/music/a.flac")], Vec::new()),
        100,
    )
    .unwrap();

    // What the background identifier does once it has decoded the file.
    lib.promote_pending(&known_track(1, "/music/a.flac"))
        .unwrap();

    let cues = lib.cues(id(1)).unwrap();
    assert_eq!(cues.len(), 1, "the imported cue must survive the wait");
    assert_eq!(cues[0].frame, 32.5 * SR.as_f64());
    assert_eq!(lib.loops(id(1)).unwrap().len(), 1);

    let found = lib.track(id(1)).unwrap().unwrap();
    assert_eq!(found.analysis.bpm, Some(128.0));
    assert_eq!(
        found.analysis.grid_confidence,
        Some(1.0),
        "an imported grid is one somebody already played from"
    );
}

/// The rule that matters most: an import must not overwrite work the DJ has
/// done here.
#[test]
fn an_import_does_not_replace_cues_the_dj_already_set() {
    let lib = Library::in_memory().unwrap();
    lib.upsert_track(&known_track(1, "/music/a.flac")).unwrap();
    lib.set_cues(
        id(1),
        &[StoredCue {
            slot: 1,
            frame: 1000.0,
            label: Some("mine".to_owned()),
            colour: None,
        }],
    )
    .unwrap();

    lib.import(
        &collection(vec![imported("/music/a.flac")], Vec::new()),
        100,
    )
    .unwrap();

    let cues = lib.cues(id(1)).unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(
        cues[0].label.as_deref(),
        Some("mine"),
        "the DJ's own cue outranks the imported one"
    );
}

/// The playlist tree appears at once, because it needs no track ids.
#[test]
fn the_playlist_tree_is_created_even_when_no_track_is_known_yet() {
    let lib = Library::in_memory().unwrap();
    let tree = vec![
        ImportedPlaylist {
            name: "Latin".to_owned(),
            parent: None,
            is_folder: true,
            paths: Vec::new(),
        },
        ImportedPlaylist {
            name: "Warm-up".to_owned(),
            parent: Some(0),
            is_folder: false,
            paths: vec![PathBuf::from("/music/a.flac")],
        },
    ];

    let report = lib
        .import(&collection(vec![imported("/music/a.flac")], tree), 100)
        .unwrap();
    assert_eq!(report.folders, 1);
    assert_eq!(report.playlists, 1);

    let nodes = lib.playlists().unwrap();
    assert_eq!(nodes.len(), 2);
    let warmup = nodes.iter().find(|n| n.name == "Warm-up").unwrap();
    let latin = nodes.iter().find(|n| n.name == "Latin").unwrap();
    assert_eq!(warmup.parent_id, Some(latin.id));
    assert_eq!(
        warmup.track_count, 0,
        "empty for now: the track behind it has not been identified"
    );
}

/// ...and fills in as the tracks are identified.
#[test]
fn playlist_membership_lands_when_the_track_is_identified() {
    let lib = Library::in_memory().unwrap();
    let tree = vec![ImportedPlaylist {
        name: "Warm-up".to_owned(),
        parent: None,
        is_folder: false,
        paths: vec![
            PathBuf::from("/music/b.flac"),
            PathBuf::from("/music/a.flac"),
        ],
    }];
    lib.import(
        &collection(
            vec![imported("/music/a.flac"), imported("/music/b.flac")],
            tree,
        ),
        100,
    )
    .unwrap();

    let list = lib.playlists().unwrap()[0].id;
    assert_eq!(lib.playlist_tracks(list).unwrap().len(), 0);

    lib.promote_pending(&known_track(1, "/music/a.flac"))
        .unwrap();
    lib.promote_pending(&known_track(2, "/music/b.flac"))
        .unwrap();

    let tracks = lib.playlist_tracks(list).unwrap();
    assert_eq!(
        tracks.iter().map(|(_, t)| t.id).collect::<Vec<_>>(),
        vec![id(2), id(1)],
        "the DJ's order survives, whatever order the files were decoded in"
    );
}

/// A track that is already known when the import runs goes straight into the
/// playlist, without a detour through the queue.
#[test]
fn a_known_track_joins_an_imported_playlist_immediately() {
    let lib = Library::in_memory().unwrap();
    lib.upsert_track(&known_track(1, "/music/a.flac")).unwrap();

    let tree = vec![ImportedPlaylist {
        name: "Warm-up".to_owned(),
        parent: None,
        is_folder: false,
        paths: vec![PathBuf::from("/music/a.flac")],
    }];
    lib.import(&collection(vec![imported("/music/a.flac")], tree), 100)
        .unwrap();

    let list = lib.playlists().unwrap()[0].id;
    assert_eq!(lib.playlist_tracks(list).unwrap().len(), 1);
}

/// Importing the same file twice must not make two rows or two queue entries.
#[test]
fn importing_twice_is_idempotent() {
    let lib = Library::in_memory().unwrap();
    let import = collection(vec![imported("/music/a.flac")], Vec::new());
    lib.import(&import, 100).unwrap();
    lib.import(&import, 200).unwrap();

    assert_eq!(lib.pending_count().unwrap(), 1);
}

/// A file the scan could not read is worth another attempt once an import
/// names it: the DJ has just told us it is a track they play.
#[test]
fn importing_a_file_that_previously_failed_requeues_it() {
    let lib = Library::in_memory().unwrap();
    lib.record_pending(
        &dj_library::ScannedFile {
            path: PathBuf::from("/music/a.flac"),
            tags: Tags::default(),
            file_size: Some(1),
            file_modified: Some(1),
        },
        50,
    )
    .unwrap();
    lib.mark_pending_failed(std::path::Path::new("/music/a.flac"), "no decodable audio")
        .unwrap();
    assert_eq!(lib.pending_count().unwrap(), 0);

    lib.import(
        &collection(vec![imported("/music/a.flac")], Vec::new()),
        100,
    )
    .unwrap();
    assert_eq!(lib.pending_count().unwrap(), 1);
    assert!(lib.failed_pending().unwrap().is_empty());
}

/// The bug a screenshot found: Serato's in-file markers are usually cues with
/// no tempo, and applying one blanked the grid the analyser had already found.
#[test]
fn a_payload_with_cues_but_no_tempo_leaves_the_grid_alone() {
    let lib = Library::in_memory().unwrap();
    let mut track = known_track(1, "/music/a.flac");
    track.analysis = StoredAnalysis {
        bpm: Some(128.0),
        grid_anchor: Some(0.0),
        grid_beats_per_bar: Some(4),
        grid_confidence: Some(0.9),
        grid_source: Some(dj_library::GridSource::Analysis),
        ..StoredAnalysis::default()
    };
    lib.upsert_track(&track).unwrap();
    lib.set_analysis(track.id, &track.analysis).unwrap();

    // Cues, and nothing about the tempo — what a `GEOB` tag normally holds.
    let cues_only = ImportPayload {
        cues: vec![ImportedCue {
            slot: 1,
            seconds: 4.0,
            label: None,
            colour: None,
        }],
        ..ImportPayload::default()
    };
    lib.import(
        &collection(
            vec![ImportedTrack {
                path: PathBuf::from("/music/a.flac"),
                payload: cues_only,
                ..ImportedTrack::default()
            }],
            Vec::new(),
        ),
        100,
    )
    .unwrap();

    let found = lib.track(id(1)).unwrap().unwrap();
    assert_eq!(
        found.analysis.bpm,
        Some(128.0),
        "cues must not cost the track its tempo"
    );
    assert_eq!(
        lib.cues(id(1)).unwrap().len(),
        1,
        "...and the cues still land"
    );
}

#[test]
fn the_report_says_what_was_skipped() {
    let lib = Library::in_memory().unwrap();
    let mut import = collection(Vec::new(), Vec::new());
    import.skipped.push(dj_library::import::Skipped {
        what: "Ghost Track".to_owned(),
        reason: "the entry has no file location",
    });

    let report = lib.import(&import, 100).unwrap();
    assert_eq!(report.skipped.len(), 1);
    assert!(report.skipped[0].contains("Ghost Track"));
}

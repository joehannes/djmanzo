//! Reading each format, from samples shaped like the real thing.
//!
//! The samples are hand-written rather than captured from the applications:
//! shipping somebody's exported library would ship their music collection, and
//! the point of a fixture is that every value in it is there because a test
//! needs it. Each one exercises the parts of its format that differ from the
//! others — that is where importers go wrong.

use dj_library::import::{self, Format};
use std::path::PathBuf;

const REKORDBOX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.7.4"/>
  <COLLECTION Entries="3">
    <TRACK TrackID="1" Name="Bachata Rosa" Artist="Juan Luis Guerra"
           Album="Bachata Rosa" Genre="Bachata" Label="Karen" Year="1990"
           TrackNumber="3" Rating="204" AverageBpm="128.00" Tonality="Am"
           Location="file://localhost/music/latin/Bachata%20Rosa.flac">
      <TEMPO Inizio="0.025" Bpm="127.98" Metro="4/4" Battito="1"/>
      <POSITION_MARK Name="intro" Type="0" Start="0.5" Num="0" Red="255" Green="0" Blue="0"/>
      <POSITION_MARK Name="drop" Type="0" Start="32.5" Num="1"/>
      <POSITION_MARK Name="the eight" Type="4" Start="64.0" End="80.0" Num="2"/>
      <POSITION_MARK Name="memory" Type="0" Start="96.0" Num="-1"/>
    </TRACK>
    <TRACK TrackID="2" Name="Gasolina" Artist="Daddy Yankee" AverageBpm="94.00"
           Tonality="8A" Location="file:///music/Gasolina.mp3"/>
    <TRACK TrackID="3" Name="No Location"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="1">
      <NODE Type="0" Name="Latin" Count="1">
        <NODE Type="1" Name="Warm-up" Entries="2">
          <TRACK Key="2"/>
          <TRACK Key="1"/>
        </NODE>
      </NODE>
      <NODE Type="1" Name="Closers" Entries="1">
        <TRACK Key="99"/>
      </NODE>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>"#;

const TRAKTOR: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<NML VERSION="19">
  <HEAD COMPANY="www.native-instruments.com" PROGRAM="Traktor"/>
  <COLLECTION ENTRIES="2">
    <ENTRY MODIFIED_DATE="2024/1/1" TITLE="Bachata Rosa" ARTIST="Juan Luis Guerra">
      <LOCATION DIR="/:music/:latin/:" FILE="Bachata Rosa.flac" VOLUME="Macintosh HD"/>
      <ALBUM TITLE="Bachata Rosa" TRACK="3"/>
      <INFO GENRE="Bachata" LABEL="Karen" COMMENT="a note" RANKING="204" RELEASE_DATE="1990/1/1"/>
      <TEMPO BPM="128.000000" BPM_QUALITY="100.000000"/>
      <MUSICAL_KEY VALUE="21"/>
      <CUE_V2 NAME="grid" DISPL_ORDER="0" TYPE="4" START="25.0" LEN="0.0" HOTCUE="-1"/>
      <CUE_V2 NAME="drop" DISPL_ORDER="0" TYPE="0" START="32500.0" LEN="0.0" HOTCUE="0"/>
      <CUE_V2 NAME="the eight" DISPL_ORDER="0" TYPE="5" START="64000.0" LEN="16000.0" HOTCUE="1"/>
    </ENTRY>
    <ENTRY TITLE="No File"><LOCATION DIR="/:music/:" VOLUME="HD"/></ENTRY>
  </COLLECTION>
  <PLAYLISTS>
    <NODE TYPE="FOLDER" NAME="$ROOT">
      <SUBNODES COUNT="1">
        <NODE TYPE="FOLDER" NAME="Latin">
          <SUBNODES COUNT="1">
            <NODE TYPE="PLAYLIST" NAME="Warm-up">
              <PLAYLIST ENTRIES="1" TYPE="LIST">
                <ENTRY><PRIMARYKEY TYPE="TRACK" KEY="Macintosh HD/:music/:latin/:Bachata Rosa.flac"/></ENTRY>
              </PLAYLIST>
            </NODE>
          </SUBNODES>
        </NODE>
      </SUBNODES>
    </NODE>
  </PLAYLISTS>
</NML>"#;

const ITUNES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>Application Version</key><string>12.12</string>
  <key>Tracks</key>
  <dict>
    <key>1234</key>
    <dict>
      <key>Track ID</key><integer>1234</integer>
      <key>Name</key><string>Bachata Rosa</string>
      <key>Artist</key><string>Juan Luis Guerra</string>
      <key>Album</key><string>Bachata Rosa</string>
      <key>Genre</key><string>Bachata</string>
      <key>Year</key><integer>1990</integer>
      <key>Track Number</key><integer>3</integer>
      <key>Rating</key><integer>80</integer>
      <key>Location</key><string>file://localhost/music/latin/Bachata%20Rosa.flac</string>
    </dict>
    <key>5678</key>
    <dict>
      <key>Track ID</key><integer>5678</integer>
      <key>Name</key><string>Cloud Only</string>
    </dict>
  </dict>
  <key>Playlists</key>
  <array>
    <dict>
      <key>Name</key><string>Musik</string>
      <key>Distinguished Kind</key><integer>1</integer>
      <key>Master</key><true/>
      <key>Playlist Persistent ID</key><string>AAAA</string>
    </dict>
    <dict>
      <key>Name</key><string>Latin</string>
      <key>Playlist Persistent ID</key><string>BBBB</string>
      <key>Folder</key><true/>
    </dict>
    <dict>
      <key>Name</key><string>Warm-up</string>
      <key>Playlist Persistent ID</key><string>CCCC</string>
      <key>Parent Persistent ID</key><string>BBBB</string>
      <key>Playlist Items</key>
      <array>
        <dict><key>Track ID</key><integer>1234</integer></dict>
      </array>
    </dict>
  </array>
</dict>
</plist>"#;

// -- rekordbox --------------------------------------------------------------

#[test]
fn rekordbox_is_recognised_and_read() {
    let (format, collection) = import::read(REKORDBOX).unwrap();
    assert_eq!(format, Format::RekordboxXml);
    assert_eq!(
        collection.tracks.len(),
        2,
        "the entry with no location is skipped"
    );

    let track = &collection.tracks[0];
    assert_eq!(
        track.path,
        PathBuf::from("/music/latin/Bachata Rosa.flac"),
        "the file URL and its percent escapes are decoded"
    );
    assert_eq!(track.title.as_deref(), Some("Bachata Rosa"));
    assert_eq!(track.label.as_deref(), Some("Karen"));
    assert_eq!(track.year, Some(1990));
    assert_eq!(track.rating, Some(4), "204 out of 255 is four stars");
}

#[test]
fn rekordbox_skips_and_says_why() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    assert!(
        collection
            .skipped
            .iter()
            .any(|s| s.what == "No Location" && s.reason.contains("location")),
        "a track with no file must be reported, not silently dropped"
    );
}

#[test]
fn rekordbox_reads_the_grid_from_the_first_tempo_marker() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    let payload = &collection.tracks[0].payload;
    assert_eq!(payload.grid_anchor_seconds, Some(0.025));
    assert_eq!(
        payload.bpm,
        Some(127.98),
        "the marker's own tempo is more precise than the average"
    );
    assert_eq!(payload.key_hour, Some(8));
    assert_eq!(payload.key_minor, Some(true));
}

/// A mark with an end is a loop, whatever rekordbox calls it.
#[test]
fn rekordbox_tells_cues_and_loops_apart_by_whether_they_have_an_end() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    let payload = &collection.tracks[0].payload;

    assert_eq!(payload.loops.len(), 1);
    assert_eq!(payload.loops[0].start_seconds, 64.0);
    assert_eq!(payload.loops[0].end_seconds, 80.0);
    assert_eq!(payload.loops[0].label.as_deref(), Some("the eight"));

    // intro, drop and the memory cue.
    assert_eq!(payload.cues.len(), 3);
}

/// rekordbox counts hot cues from zero and we count from one.
#[test]
fn rekordbox_cue_numbers_become_one_based_slots() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    let cues = &collection.tracks[0].payload.cues;
    assert_eq!(cues[0].slot, 1, "Num=0 is slot 1");
    assert_eq!(cues[1].slot, 2);
    assert_eq!(cues[0].colour.as_deref(), Some("#ff0000"));
}

/// A memory cue has no pad of its own but is still something the DJ set.
#[test]
fn rekordbox_keeps_the_memory_cue_rather_than_dropping_it() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    let cues = &collection.tracks[0].payload.cues;
    assert!(
        cues.iter().any(|c| c.seconds == 96.0),
        "the memory cue must survive: {cues:?}"
    );
}

#[test]
fn rekordbox_playlists_nest_and_keep_their_order() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    let names: Vec<&str> = collection
        .playlists
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Latin", "Warm-up", "Closers"],
        "rekordbox's own ROOT node is not a folder the DJ made"
    );

    let warmup = &collection.playlists[1];
    assert_eq!(warmup.parent, Some(0));
    assert!(!warmup.is_folder);
    assert_eq!(
        warmup.paths,
        vec![
            PathBuf::from("/music/Gasolina.mp3"),
            PathBuf::from("/music/latin/Bachata Rosa.flac"),
        ],
        "the DJ's order, not the collection's"
    );
    assert!(collection.playlists[0].is_folder);
}

#[test]
fn a_playlist_entry_naming_a_missing_track_is_reported() {
    let (_, collection) = import::read(REKORDBOX).unwrap();
    assert!(
        collection.skipped.iter().any(|s| s.what.contains("99")),
        "a dangling playlist reference must be reported"
    );
}

// -- Traktor ----------------------------------------------------------------

#[test]
fn traktor_is_recognised_and_its_split_paths_rejoined() {
    let (format, collection) = import::read(TRAKTOR).unwrap();
    assert_eq!(format, Format::TraktorNml);
    assert_eq!(
        collection.tracks.len(),
        1,
        "the entry with no FILE is skipped"
    );
    assert_eq!(
        collection.tracks[0].path,
        PathBuf::from("/music/latin/Bachata Rosa.flac"),
        "DIR and FILE are joined and the /: separator undone"
    );
}

/// The one that would silently ruin every cue: Traktor is in milliseconds.
#[test]
fn traktor_cue_positions_are_converted_from_milliseconds() {
    let (_, collection) = import::read(TRAKTOR).unwrap();
    let payload = &collection.tracks[0].payload;
    assert_eq!(payload.cues.len(), 1);
    assert_eq!(payload.cues[0].seconds, 32.5, "32500 ms is 32.5 s");
    assert_eq!(payload.loops.len(), 1);
    assert_eq!(payload.loops[0].start_seconds, 64.0);
    assert_eq!(
        payload.loops[0].end_seconds, 80.0,
        "LEN is a duration, not an end"
    );
}

#[test]
fn traktor_grid_marker_becomes_the_anchor() {
    let (_, collection) = import::read(TRAKTOR).unwrap();
    let payload = &collection.tracks[0].payload;
    assert_eq!(payload.grid_anchor_seconds, Some(0.025));
    assert_eq!(payload.bpm, Some(128.0));
    assert!(
        !payload.cues.iter().any(|c| c.seconds == 0.025),
        "the grid marker is a grid marker, not a hot cue"
    );
}

#[test]
fn traktor_numeric_keys_become_camelot() {
    let (_, collection) = import::read(TRAKTOR).unwrap();
    let payload = &collection.tracks[0].payload;
    assert_eq!(payload.key_minor, Some(true));
    assert!(payload.key_hour.is_some());
}

#[test]
fn traktor_reads_tags_out_of_the_info_element() {
    let (_, collection) = import::read(TRAKTOR).unwrap();
    let track = &collection.tracks[0];
    assert_eq!(track.genre.as_deref(), Some("Bachata"));
    assert_eq!(track.label.as_deref(), Some("Karen"));
    assert_eq!(track.year, Some(1990));
    assert_eq!(track.rating, Some(4));
    assert_eq!(track.album.as_deref(), Some("Bachata Rosa"));
    assert_eq!(track.track_number, Some(3));
}

#[test]
fn traktor_playlists_nest_and_lose_the_volume_name() {
    let (_, collection) = import::read(TRAKTOR).unwrap();
    let names: Vec<&str> = collection
        .playlists
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, vec!["Latin", "Warm-up"], "$ROOT is not a folder");

    assert_eq!(
        collection.playlists[1].paths,
        vec![PathBuf::from("/music/latin/Bachata Rosa.flac")],
        "the volume name belongs to the exporting machine, not to the path"
    );
}

// -- iTunes -----------------------------------------------------------------

#[test]
fn itunes_is_recognised_and_read() {
    let (format, collection) = import::read(ITUNES).unwrap();
    assert_eq!(format, Format::ItunesXml);
    assert_eq!(
        collection.tracks.len(),
        1,
        "the cloud-only entry is skipped"
    );

    let track = &collection.tracks[0];
    assert_eq!(track.path, PathBuf::from("/music/latin/Bachata Rosa.flac"));
    assert_eq!(track.artist.as_deref(), Some("Juan Luis Guerra"));
    assert_eq!(track.year, Some(1990));
    assert_eq!(track.rating, Some(4), "80 out of 100 is four stars");
}

/// iTunes has no cues, loops or grid, and saying so is the honest result.
#[test]
fn itunes_brings_no_performance_data() {
    let (_, collection) = import::read(ITUNES).unwrap();
    assert!(collection.tracks[0].payload.is_empty());
}

#[test]
fn itunes_folders_are_reparented_from_persistent_ids() {
    let (_, collection) = import::read(ITUNES).unwrap();
    let names: Vec<&str> = collection
        .playlists
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Latin", "Warm-up"],
        "iTunes' own playlists are recognised by their marker keys, not by an \
         English name -- the fixture calls the library \"Musik\" on purpose"
    );
    assert!(collection.playlists[0].is_folder);
    assert_eq!(collection.playlists[1].parent, Some(0));
    assert_eq!(
        collection.playlists[1].paths,
        vec![PathBuf::from("/music/latin/Bachata Rosa.flac")]
    );
}

// -- shared -----------------------------------------------------------------

#[test]
fn something_that_is_not_a_library_export_is_refused() {
    assert!(import::read("<html><body>nope</body></html>").is_err());
    assert!(import::read("not xml at all").is_err());
}

/// Malformed XML must be an error, not a panic and not half a collection.
#[test]
fn truncated_xml_is_an_error() {
    let truncated = &REKORDBOX[..REKORDBOX.len() / 2];
    assert!(import::read(truncated).is_err());
}

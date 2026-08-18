//! Reading a rekordbox XML export.
//!
//! The format is Pioneer's documented interchange file — the one produced by
//! *File → Export Collection in xml format* — not the device database. It is
//! the format a DJ can actually produce on demand, and the one that carries
//! cues, loops and the beat grid in a form anybody can read.
//!
//! ```xml
//! <DJ_PLAYLISTS Version="1.0.0">
//!   <COLLECTION Entries="2">
//!     <TRACK TrackID="1" Name="Bachata Rosa" Artist="Juan Luis Guerra"
//!            AverageBpm="128.00" Tonality="Am" Location="file://localhost/music/a.flac">
//!       <TEMPO Inizio="0.025" Bpm="128.00" Metro="4/4" Battito="1"/>
//!       <POSITION_MARK Name="drop" Type="0" Start="32.5" Num="1"/>
//!     </TRACK>
//!   </COLLECTION>
//!   <PLAYLISTS>
//!     <NODE Type="0" Name="ROOT" Count="1">
//!       <NODE Type="1" Name="Friday" Entries="1">
//!         <TRACK Key="1"/>
//!       </NODE>
//!     </NODE>
//!   </PLAYLISTS>
//! </DJ_PLAYLISTS>
//! ```
//!
//! Two details the format gets wrong from our point of view, both handled here:
//! a `POSITION_MARK` with `Num="-1"` is the memory cue rather than a hot cue,
//! and a mark with an `End` is a loop rather than a point.

use super::{
    Collection, ImportError, ImportPayload, ImportedCue, ImportedLoop, ImportedPlaylist,
    ImportedTrack, Skipped, decode_path, parse_key,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// rekordbox numbers hot cues from zero; a `Num` below zero is not a hot cue at
/// all but the "memory cue" a CDJ starts from.
const MEMORY_CUE: i32 = -1;

pub fn read(contents: &str) -> Result<Collection, ImportError> {
    let doc = roxmltree::Document::parse(contents)?;
    let root = doc.root_element();

    let collection_node = root
        .children()
        .find(|n| n.has_tag_name("COLLECTION"))
        .ok_or(ImportError::MissingTracks("rekordbox XML"))?;

    let mut out = Collection::default();
    // rekordbox's playlists reference tracks by `TrackID`, so the map has to
    // exist before the playlists are read.
    let mut by_id: HashMap<String, PathBuf> = HashMap::new();

    for node in collection_node
        .children()
        .filter(|n| n.has_tag_name("TRACK"))
    {
        let Some(location) = node.attribute("Location") else {
            out.skipped.push(Skipped {
                what: node.attribute("Name").unwrap_or("a track").to_owned(),
                reason: "the entry has no file location",
            });
            continue;
        };
        let path = decode_path(location);
        if let Some(id) = node.attribute("TrackID") {
            by_id.insert(id.to_owned(), path.clone());
        }

        out.tracks.push(ImportedTrack {
            path,
            title: text(node.attribute("Name")),
            artist: text(node.attribute("Artist")),
            album: text(node.attribute("Album")),
            album_artist: text(node.attribute("AlbumArtist")),
            genre: text(node.attribute("Genre")),
            label: text(node.attribute("Label")),
            comment: text(node.attribute("Comments")),
            year: node.attribute("Year").and_then(|y| y.parse().ok()),
            track_number: node.attribute("TrackNumber").and_then(|n| n.parse().ok()),
            // rekordbox stores 0, 51, 102, 153, 204, 255 for nought to five
            // stars. Dividing by 51 is the documented conversion.
            rating: node
                .attribute("Rating")
                .and_then(|r| r.parse::<u32>().ok())
                .map(|r| u8::try_from(r / 51).unwrap_or(0).min(5)),
            payload: payload(node),
        });
    }

    read_playlists(root, &by_id, &mut out);
    Ok(out)
}

fn payload(node: roxmltree::Node<'_, '_>) -> ImportPayload {
    let mut payload = ImportPayload {
        bpm: node.attribute("AverageBpm").and_then(|b| b.parse().ok()),
        ..ImportPayload::default()
    };
    if let Some((hour, minor)) = node.attribute("Tonality").and_then(parse_key) {
        payload.key_hour = Some(hour);
        payload.key_minor = Some(minor);
    }

    for child in node.children() {
        if child.has_tag_name("TEMPO") {
            // The first `TEMPO` is the grid's anchor. Later ones describe tempo
            // changes, which a constant-tempo grid cannot represent -- the
            // track keeps the first, which is the one a DJ beatmatches from.
            if payload.grid_anchor_seconds.is_none() {
                payload.grid_anchor_seconds =
                    child.attribute("Inizio").and_then(|i| i.parse().ok());
                // A per-marker BPM is more precise than the average.
                if let Some(bpm) = child.attribute("Bpm").and_then(|b| b.parse().ok()) {
                    payload.bpm = Some(bpm);
                }
            }
        } else if child.has_tag_name("POSITION_MARK") {
            let Some(start) = child.attribute("Start").and_then(|s| s.parse::<f64>().ok()) else {
                continue;
            };
            let num: i32 = child
                .attribute("Num")
                .and_then(|n| n.parse().ok())
                .unwrap_or(MEMORY_CUE);
            let label = text(child.attribute("Name"));

            match child.attribute("End").and_then(|e| e.parse::<f64>().ok()) {
                // A mark with an end is a loop, whatever rekordbox calls it.
                Some(end) if end > start => payload.loops.push(ImportedLoop {
                    slot: slot_for(num, payload.loops.len()),
                    start_seconds: start,
                    end_seconds: end,
                    label,
                }),
                _ => payload.cues.push(ImportedCue {
                    slot: slot_for(num, payload.cues.len()),
                    seconds: start,
                    label,
                    colour: colour(child),
                }),
            }
        }
    }
    payload
}

/// rekordbox counts hot cues from zero; we count from one.
///
/// A memory cue has no slot of its own, so it takes the next free one rather
/// than being dropped: a DJ who set one meant something by it, and losing it
/// silently is worse than putting it somewhere they can see.
fn slot_for(num: i32, already: usize) -> u8 {
    if num >= 0 {
        u8::try_from(num + 1).unwrap_or(1)
    } else {
        u8::try_from(already + 1).unwrap_or(1)
    }
}

fn colour(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let component = |name: &str| node.attribute(name).and_then(|v| v.parse::<u8>().ok());
    let (r, g, b) = (component("Red")?, component("Green")?, component("Blue")?);
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}

fn read_playlists(
    root: roxmltree::Node<'_, '_>,
    by_id: &HashMap<String, PathBuf>,
    out: &mut Collection,
) {
    let Some(playlists) = root.children().find(|n| n.has_tag_name("PLAYLISTS")) else {
        return;
    };
    // The outermost NODE is rekordbox's own "ROOT", which is not a folder the
    // DJ made and should not appear in the sidebar as one.
    for node in playlists.children().filter(|n| n.has_tag_name("NODE")) {
        for child in node.children().filter(|n| n.has_tag_name("NODE")) {
            walk(child, None, by_id, out);
        }
    }
}

/// `Type="0"` is a folder, `Type="1"` is a playlist.
fn walk(
    node: roxmltree::Node<'_, '_>,
    parent: Option<usize>,
    by_id: &HashMap<String, PathBuf>,
    out: &mut Collection,
) {
    let is_folder = node.attribute("Type") == Some("0");
    let index = out.playlists.len();
    out.playlists.push(ImportedPlaylist {
        name: node.attribute("Name").unwrap_or("Untitled").to_owned(),
        parent,
        is_folder,
        paths: Vec::new(),
    });

    for child in node.children() {
        if child.has_tag_name("NODE") {
            walk(child, Some(index), by_id, out);
        } else if child.has_tag_name("TRACK") {
            // Inside a playlist, `Key` is the `TrackID` from the collection.
            if let Some(path) = child.attribute("Key").and_then(|k| by_id.get(k)) {
                out.playlists[index].paths.push(path.clone());
            } else if let Some(key) = child.attribute("Key") {
                out.skipped.push(Skipped {
                    what: format!("playlist entry {key}"),
                    reason: "it names a track that is not in the collection",
                });
            }
        }
    }
}

/// An attribute that is present but empty is absent.
fn text(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

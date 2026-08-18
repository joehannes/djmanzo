//! Reading a Traktor NML collection.
//!
//! ```xml
//! <NML VERSION="19">
//!   <COLLECTION ENTRIES="1">
//!     <ENTRY TITLE="Bachata Rosa" ARTIST="Juan Luis Guerra">
//!       <LOCATION DIR="/:music/:latin/:" FILE="a.flac" VOLUME="Macintosh HD"/>
//!       <TEMPO BPM="128.000000" BPM_QUALITY="100.0"/>
//!       <MUSICAL_KEY VALUE="21"/>
//!       <CUE_V2 NAME="drop" TYPE="0" START="32500.0" LEN="0.0" HOTCUE="0"/>
//!     </ENTRY>
//!   </COLLECTION>
//!   <PLAYLISTS>...</PLAYLISTS>
//! </NML>
//! ```
//!
//! Three things Traktor does that nothing else does, all handled here: paths
//! are split across `DIR`/`FILE` with `/:` as the separator, cue positions are
//! in **milliseconds**, and the key is an integer 0..23 rather than a name.

use super::{
    Collection, ImportError, ImportPayload, ImportedCue, ImportedLoop, ImportedPlaylist,
    ImportedTrack, Skipped, decode_path,
};
use std::path::PathBuf;

pub fn read(contents: &str) -> Result<Collection, ImportError> {
    let doc = roxmltree::Document::parse(contents)?;
    let root = doc.root_element();

    let collection = root
        .children()
        .find(|n| n.has_tag_name("COLLECTION"))
        .ok_or(ImportError::MissingTracks("Traktor NML"))?;

    let mut out = Collection::default();
    for node in collection.children().filter(|n| n.has_tag_name("ENTRY")) {
        let Some(path) = location(node) else {
            out.skipped.push(Skipped {
                what: node.attribute("TITLE").unwrap_or("an entry").to_owned(),
                reason: "the entry has no usable file location",
            });
            continue;
        };

        let info = node.children().find(|n| n.has_tag_name("INFO"));
        let attr = |name: &str| info.as_ref().and_then(|n| text(n.attribute(name)));

        out.tracks.push(ImportedTrack {
            path,
            title: text(node.attribute("TITLE")),
            artist: text(node.attribute("ARTIST")),
            album: node
                .children()
                .find(|n| n.has_tag_name("ALBUM"))
                .and_then(|n| text(n.attribute("TITLE"))),
            album_artist: None,
            genre: attr("GENRE"),
            label: attr("LABEL"),
            comment: attr("COMMENT"),
            year: attr("RELEASE_DATE").and_then(|d| d.get(..4)?.parse().ok()),
            track_number: node
                .children()
                .find(|n| n.has_tag_name("ALBUM"))
                .and_then(|n| n.attribute("TRACK")?.parse().ok()),
            // Traktor stores 0..255 in `RANKING`, in steps of 51.
            rating: info
                .as_ref()
                .and_then(|n| n.attribute("RANKING")?.parse::<u32>().ok())
                .map(|r| u8::try_from(r / 51).unwrap_or(0).min(5)),
            payload: payload(node),
        });
    }

    read_playlists(root, &mut out);
    Ok(out)
}

/// Traktor splits a path into a volume, a directory and a filename, and spells
/// the directory separator `/:`.
fn location(node: roxmltree::Node<'_, '_>) -> Option<PathBuf> {
    let location = node.children().find(|n| n.has_tag_name("LOCATION"))?;
    let dir = location.attribute("DIR").unwrap_or("");
    let file = location.attribute("FILE")?;
    if file.trim().is_empty() {
        return None;
    }
    // `/:music/:latin/:` becomes `/music/latin/`.
    let joined = format!("{}{}", dir.replace("/:", "/"), file);
    Some(decode_path(&joined))
}

fn payload(node: roxmltree::Node<'_, '_>) -> ImportPayload {
    let mut payload = ImportPayload {
        bpm: node
            .children()
            .find(|n| n.has_tag_name("TEMPO"))
            .and_then(|n| n.attribute("BPM")?.parse().ok()),
        ..ImportPayload::default()
    };

    if let Some((hour, minor)) = node
        .children()
        .find(|n| n.has_tag_name("MUSICAL_KEY"))
        .and_then(|n| n.attribute("VALUE")?.parse::<u8>().ok())
        .and_then(key_from_traktor)
    {
        payload.key_hour = Some(hour);
        payload.key_minor = Some(minor);
    }

    for cue in node.children().filter(|n| n.has_tag_name("CUE_V2")) {
        // Milliseconds, unlike every other format here.
        let Some(start_ms) = cue.attribute("START").and_then(|s| s.parse::<f64>().ok()) else {
            continue;
        };
        let start = start_ms / 1000.0;
        let length = cue
            .attribute("LEN")
            .and_then(|l| l.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1000.0;
        let label = text(cue.attribute("NAME"));
        // `HOTCUE="-1"` is a marker without a pad: the grid marker, the load
        // point, or a plain memory cue.
        let hotcue: i32 = cue
            .attribute("HOTCUE")
            .and_then(|h| h.parse().ok())
            .unwrap_or(-1);

        // `TYPE="4"` is Traktor's grid marker, which is where the first beat
        // is -- exactly what a beat grid's anchor means.
        if cue.attribute("TYPE") == Some("4") {
            payload.grid_anchor_seconds.get_or_insert(start);
            continue;
        }

        if length > 0.0 {
            payload.loops.push(ImportedLoop {
                slot: slot_for(hotcue, payload.loops.len()),
                start_seconds: start,
                end_seconds: start + length,
                label,
            });
        } else {
            payload.cues.push(ImportedCue {
                slot: slot_for(hotcue, payload.cues.len()),
                seconds: start,
                label,
                colour: None,
            });
        }
    }
    payload
}

fn slot_for(hotcue: i32, already: usize) -> u8 {
    if hotcue >= 0 {
        u8::try_from(hotcue + 1).unwrap_or(1)
    } else {
        u8::try_from(already + 1).unwrap_or(1)
    }
}

/// Traktor's key is an integer: 0..=11 major from C, 12..=23 minor from A.
///
/// Written out as the table it is rather than derived, because the two rings
/// start from different notes and a formula for that is harder to check than
/// twenty-four values.
fn key_from_traktor(value: u8) -> Option<(u8, bool)> {
    // Camelot hour for each of the twelve major keys, C first.
    const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
    // ...and for the twelve minor keys, A first.
    const MINOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];

    match value {
        0..=11 => Some((MAJOR[usize::from(value)], false)),
        12..=23 => Some((MINOR[usize::from(value - 12)], true)),
        _ => None,
    }
}

fn read_playlists(root: roxmltree::Node<'_, '_>, out: &mut Collection) {
    let Some(playlists) = root.children().find(|n| n.has_tag_name("PLAYLISTS")) else {
        return;
    };
    // Traktor wraps everything in a `$ROOT` node that is not a folder the DJ
    // made.
    for node in playlists.children().filter(|n| n.has_tag_name("NODE")) {
        for child in subnodes(node) {
            walk(child, None, out);
        }
    }
}

/// A `NODE` of type FOLDER holds a `SUBNODES` element; one of type PLAYLIST
/// holds a `PLAYLIST`.
fn subnodes<'a, 'i>(node: roxmltree::Node<'a, 'i>) -> Vec<roxmltree::Node<'a, 'i>> {
    node.children()
        .filter(|n| n.has_tag_name("SUBNODES"))
        .flat_map(|s| s.children().filter(roxmltree::Node::is_element))
        .filter(|n| n.has_tag_name("NODE"))
        .collect()
}

fn walk(node: roxmltree::Node<'_, '_>, parent: Option<usize>, out: &mut Collection) {
    let is_folder = node.attribute("TYPE") == Some("FOLDER");
    let index = out.playlists.len();
    out.playlists.push(ImportedPlaylist {
        name: node.attribute("NAME").unwrap_or("Untitled").to_owned(),
        parent,
        is_folder,
        paths: Vec::new(),
    });

    for child in subnodes(node) {
        walk(child, Some(index), out);
    }

    for playlist in node.children().filter(|n| n.has_tag_name("PLAYLIST")) {
        for entry in playlist.children().filter(|n| n.has_tag_name("ENTRY")) {
            // Inside a playlist the location is a single `KEY` attribute, in
            // the same `/:` spelling as the collection's `DIR`.
            if let Some(key) = entry
                .children()
                .find(|n| n.has_tag_name("PRIMARYKEY"))
                .and_then(|n| n.attribute("KEY"))
            {
                let path = decode_path(&key.replace("/:", "/"));
                // The volume name is the leading segment and is not part of the
                // path on the machine doing the importing.
                out.playlists[index].paths.push(strip_volume(&path));
            }
        }
    }
}

/// Traktor prefixes playlist keys with the volume name: `Macintosh HD/music/…`.
///
/// Dropped, because it is the name of a disk on the machine that exported the
/// file and means nothing on the machine reading it. What is left starts with
/// the first real directory, which is what a path lookup needs.
fn strip_volume(path: &std::path::Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.find('/') {
        // Already absolute: nothing to strip.
        Some(0) => path.to_path_buf(),
        Some(index) => PathBuf::from(format!("/{}", &text[index + 1..])),
        None => path.to_path_buf(),
    }
}

fn text(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

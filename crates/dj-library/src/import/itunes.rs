//! Reading an iTunes / Music library XML.
//!
//! ```xml
//! <plist version="1.0"><dict>
//!   <key>Tracks</key><dict>
//!     <key>1234</key><dict>
//!       <key>Track ID</key><integer>1234</integer>
//!       <key>Name</key><string>Bachata Rosa</string>
//!       <key>Location</key><string>file://localhost/music/a.flac</string>
//!     </dict>
//!   </dict>
//!   <key>Playlists</key><array>...</array>
//! </dict></plist>
//! ```
//!
//! iTunes knows nothing about cues, loops or beat grids — it is a music player,
//! not a DJ application. So this importer brings tags, ratings and playlists
//! and nothing else, which is the honest result rather than a disappointing
//! one: everything it *does* know is worth having, and the analyser fills in
//! the rest.
//!
//! # Reading a plist without a plist parser
//!
//! A plist `<dict>` is a flat sequence of alternating `<key>` and value
//! elements rather than nested pairs, which is why this walks children two at a
//! time instead of using a generic XML-to-struct mapping. That is the whole
//! trick, and it is small enough not to warrant a dependency.

use super::{Collection, ImportError, ImportedPlaylist, ImportedTrack, Skipped, decode_path};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn read(contents: &str) -> Result<Collection, ImportError> {
    let doc = roxmltree::Document::parse(contents)?;
    let root_dict = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("dict"))
        .ok_or(ImportError::MissingTracks("iTunes XML"))?;

    let mut out = Collection::default();
    let mut by_id: HashMap<String, PathBuf> = HashMap::new();

    if let Some(tracks) = value_for(root_dict, "Tracks") {
        for (id, entry) in pairs(tracks) {
            let Some(location) = string_for(entry, "Location") else {
                out.skipped.push(Skipped {
                    what: string_for(entry, "Name").unwrap_or_else(|| format!("track {id}")),
                    reason: "the entry has no file location",
                });
                continue;
            };
            let path = decode_path(&location);
            by_id.insert(id, path.clone());

            out.tracks.push(ImportedTrack {
                path,
                title: string_for(entry, "Name"),
                artist: string_for(entry, "Artist"),
                album: string_for(entry, "Album"),
                album_artist: string_for(entry, "Album Artist"),
                genre: string_for(entry, "Genre"),
                label: None,
                comment: string_for(entry, "Comments"),
                year: integer_for(entry, "Year").and_then(|y| i32::try_from(y).ok()),
                track_number: integer_for(entry, "Track Number")
                    .and_then(|n| u32::try_from(n).ok()),
                // iTunes rates out of 100, in steps of twenty.
                rating: integer_for(entry, "Rating")
                    .map(|r| u8::try_from(r / 20).unwrap_or(0).min(5)),
                // Nothing to carry: iTunes has no cues, loops or grid.
                payload: super::ImportPayload::default(),
            });
        }
    } else {
        return Err(ImportError::MissingTracks("iTunes XML"));
    }

    if let Some(playlists) = value_for(root_dict, "Playlists") {
        read_playlists(playlists, &by_id, &mut out);
    }
    Ok(out)
}

fn read_playlists(
    array: roxmltree::Node<'_, '_>,
    by_id: &HashMap<String, PathBuf>,
    out: &mut Collection,
) {
    // iTunes' folders are flat: each playlist carries its parent's persistent
    // id rather than being nested. Two passes -- create, then reparent.
    let mut index_by_persistent: HashMap<String, usize> = HashMap::new();
    let mut parents: Vec<Option<String>> = Vec::new();

    for entry in array.children().filter(|n| n.has_tag_name("dict")) {
        // iTunes' own playlists — Library, Music, Movies, Downloaded — are
        // noise in a DJ's sidebar.
        //
        // Recognised by the keys iTunes marks them with rather than by name:
        // the names are localised, so a list of English ones would import
        // "Musik" and "Películas" as crates on most of the machines that have
        // them. `Distinguished Kind` marks a built-in and `Master` marks the
        // library itself; a playlist the DJ made has neither.
        if value_for(entry, "Distinguished Kind").is_some()
            || bool_for(entry, "Master").unwrap_or(false)
        {
            continue;
        }

        let index = out.playlists.len();
        if let Some(id) = string_for(entry, "Playlist Persistent ID") {
            index_by_persistent.insert(id, index);
        }
        parents.push(string_for(entry, "Parent Persistent ID"));

        let paths = value_for(entry, "Playlist Items")
            .map(|items| {
                items
                    .children()
                    .filter(|n| n.has_tag_name("dict"))
                    .filter_map(|item| {
                        let id = integer_for(item, "Track ID")?;
                        by_id.get(&id.to_string()).cloned()
                    })
                    .collect()
            })
            .unwrap_or_default();

        out.playlists.push(ImportedPlaylist {
            name: string_for(entry, "Name").unwrap_or_else(|| "Untitled".to_owned()),
            parent: None,
            // A folder in iTunes holds no items of its own.
            is_folder: bool_for(entry, "Folder").unwrap_or(false),
            paths,
        });
    }

    for (index, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent.as_ref().and_then(|p| index_by_persistent.get(p)) {
            out.playlists[index].parent = Some(*parent);
        }
    }
}

/// The children of a `<dict>`, paired as key and value.
fn pairs<'a, 'i>(dict: roxmltree::Node<'a, 'i>) -> Vec<(String, roxmltree::Node<'a, 'i>)> {
    let elements: Vec<_> = dict
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    elements
        .chunks(2)
        .filter_map(|pair| {
            let [key, value] = pair else { return None };
            key.has_tag_name("key")
                .then(|| (key.text().unwrap_or_default().to_owned(), *value))
        })
        .collect()
}

fn value_for<'a, 'i>(dict: roxmltree::Node<'a, 'i>, key: &str) -> Option<roxmltree::Node<'a, 'i>> {
    pairs(dict)
        .into_iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn string_for(dict: roxmltree::Node<'_, '_>, key: &str) -> Option<String> {
    let text = value_for(dict, key)?.text()?.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn integer_for(dict: roxmltree::Node<'_, '_>, key: &str) -> Option<i64> {
    value_for(dict, key)?.text()?.trim().parse().ok()
}

/// A plist boolean is an empty `<true/>` or `<false/>` element.
fn bool_for(dict: roxmltree::Node<'_, '_>, key: &str) -> Option<bool> {
    let node = value_for(dict, key)?;
    match node.tag_name().name() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

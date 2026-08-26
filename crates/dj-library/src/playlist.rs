//! Playlists, crates and folders.
//!
//! # One tree, three kinds
//!
//! To a DJ these are the same gesture: a named thing in a sidebar holding
//! either tracks or other named things. rekordbox calls them playlists and
//! folders, Serato calls them crates and subcrates, and the difference is
//! nothing but which one is allowed to contain the other. So they are one
//! table with a `kind`, and the sidebar renders a tree.
//!
//! # Positions are ordered, not contiguous
//!
//! A playlist is a sequence: the same track can appear twice, and the order is
//! the DJ's. Position is part of the primary key so two tracks cannot claim the
//! same slot — but nothing requires the numbers to have no gaps. Appending is
//! then one insert rather than a renumber, and only an explicit reorder pays
//! for rewriting the list.

use serde::{Deserialize, Serialize};

/// What a node in the tree is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistKind {
    /// Holds tracks.
    List,
    /// Holds other nodes.
    Folder,
    /// Holds a query, evaluated when it is opened.
    Smart,
}

impl PlaylistKind {
    #[must_use]
    pub const fn as_sql(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Folder => "folder",
            Self::Smart => "smart",
        }
    }

    #[must_use]
    pub fn from_sql(word: &str) -> Option<Self> {
        match word {
            "list" => Some(Self::List),
            "folder" => Some(Self::Folder),
            "smart" => Some(Self::Smart),
            _ => None,
        }
    }

    /// Whether this kind can contain other nodes.
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(self, Self::Folder)
    }
}

/// One node, as stored.
///
/// Flat, with a parent id, rather than a nested structure. The tree is built
/// where it is drawn: a recursive type would have to be rebuilt on every change
/// and would make "move this node" a rewrite rather than one update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub kind: PlaylistKind,
    /// The filter, for a smart folder. `None` for the other kinds.
    pub query: Option<String>,
    /// Unix seconds.
    pub created_at: i64,
    /// How many tracks it holds. Zero for a folder, which holds nodes.
    pub track_count: i64,
}

/// One play, as recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayRecord {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    /// Unix seconds.
    pub played_at: i64,
    pub session_id: Option<String>,
}

/// One note taken during a set.
///
/// See the `notes` migration for why it belongs to a moment rather than to a
/// track, and why what was playing is copied in rather than joined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub session_id: String,
    /// Unix seconds, on the same clock as [`PlayRecord::played_at`].
    pub at: i64,
    /// What the DJ typed. Empty for a moment marked and not yet written up.
    pub body: String,
    /// What was playing when the moment was marked, as it read at the time.
    pub playing: String,
}

impl Note {
    /// Whether this is a marker with nothing written on it yet.
    ///
    /// A complete row, not a half-finished one: in a booth the useful gesture
    /// is mark now, write afterwards, and the moment is the part that cannot
    /// be recovered later.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        self.body.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_survive_the_round_trip_through_sql() {
        for kind in [
            PlaylistKind::List,
            PlaylistKind::Folder,
            PlaylistKind::Smart,
        ] {
            assert_eq!(PlaylistKind::from_sql(kind.as_sql()), Some(kind));
        }
    }

    /// The schema has a CHECK constraint on this column. A value that is not
    /// one of the three would be rejected by SQLite rather than stored, so
    /// reading an unknown one back means the database was written by something
    /// else -- worth `None` rather than a guess.
    #[test]
    fn an_unknown_kind_is_not_guessed_at() {
        assert_eq!(PlaylistKind::from_sql("crate"), None);
        assert_eq!(PlaylistKind::from_sql(""), None);
    }

    #[test]
    fn only_folders_contain_other_nodes() {
        assert!(PlaylistKind::Folder.is_container());
        assert!(!PlaylistKind::List.is_container());
        assert!(!PlaylistKind::Smart.is_container());
    }
}

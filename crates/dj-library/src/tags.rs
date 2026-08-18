//! Reading metadata out of an audio file.
//!
//! Thin on purpose: `lofty` already knows every container `symphonia` decodes,
//! and the only judgement worth making here is what to do when a tag is present
//! but useless — which is the common case in a real collection.

use crate::record::Tags;
use lofty::file::TaggedFileExt;
use lofty::prelude::{Accessor, ItemKey};
use lofty::probe::Probe;
use std::path::Path;

/// Read the tags from a file, or [`Tags::default`] if it has none.
///
/// Never an error. A file with no tags is not a problem — it is most of a
/// hand-organised collection — and the browser falls back to the filename.
/// A file that cannot be *opened* is a different matter and is reported.
pub fn read(path: &Path) -> Result<Tags, lofty::error::LoftyError> {
    let tagged = Probe::open(path)?.read()?;
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Ok(Tags::default());
    };

    Ok(Tags {
        title: clean(tag.title().as_deref()),
        artist: clean(tag.artist().as_deref()),
        album: clean(tag.album().as_deref()),
        album_artist: clean(tag.get_string(&ItemKey::AlbumArtist)),
        genre: clean(tag.genre().as_deref()),
        // DJs sort by label, and it is the one field taggers disagree about:
        // ID3 calls it TPUB, Vorbis usually LABEL, MP4 has no standard atom.
        // `lofty` normalises all three onto `Label`.
        label: clean(tag.get_string(&ItemKey::Label)),
        comment: clean(tag.comment().as_deref()),
        year: tag.year().and_then(|y| i32::try_from(y).ok()),
        track_number: tag.track(),
    })
}

/// Blank and whitespace-only tags become `None`.
///
/// Rippers and DVS software write empty strings routinely. A browser full of
/// blank cells is worse than one that falls back to the filename, and `None`
/// is what makes that fallback fire.
fn clean(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_tags_become_absent() {
        assert_eq!(clean(Some("   ")), None);
        assert_eq!(clean(Some("")), None);
        assert_eq!(clean(None), None);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(clean(Some("  Bachata Rosa  ")), Some("Bachata Rosa".into()));
    }

    #[test]
    fn a_file_that_cannot_be_opened_is_reported_not_swallowed() {
        assert!(read(Path::new("/nonexistent/track.flac")).is_err());
    }
}

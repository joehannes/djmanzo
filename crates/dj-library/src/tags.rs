//! Reading metadata out of an audio file.
//!
//! Thin on purpose: `lofty` already knows every container `symphonia` decodes,
//! and the only judgement worth making here is what to do when a tag is present
//! but useless — which is the common case in a real collection.

use crate::record::Tags;
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
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

/// Refuse a picture larger than this.
///
/// The same limit, for the same reason, as the user's own logo: a card grid
/// decodes every cover it shows, and one 40 MB scan among five hundred records
/// makes scrolling the collection stutter for a reason nobody could guess. A
/// record whose sleeve is over the limit falls back to its lettering.
pub const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;

/// The sleeve embedded in a file, if it has one.
///
/// Bytes, and only bytes. The tag also *declares* a content type and it is
/// discarded here on purpose: taggers write `image/jpg`, which is not a media
/// type, and empty strings, and the truth is in the first three bytes anyway.
/// Whoever serves this sniffs them.
///
/// **Front cover first.** A well-tagged album carries several pictures — the
/// back, the disc label, a band photo — and `pictures()` returns them in
/// whatever order the tagger wrote them. Taking the first would show the disc
/// label where the sleeve belongs on some records and the sleeve on others,
/// which reads as a bug in the browser rather than a fact about the file. Any
/// picture is still better than none, so a file with only a band photo shows
/// the band photo.
///
/// # Errors
/// If the file cannot be opened or parsed. A file with no picture is not an
/// error -- it is most of a collection.
pub fn artwork(path: &Path) -> Result<Option<Vec<u8>>, lofty::error::LoftyError> {
    let tagged = Probe::open(path)?.read()?;
    // Every tag, not just the primary one: an MP3 ripped once and re-tagged
    // later routinely has the sleeve in its ID3v2 tag and the text in APEv2.
    let pictures = tagged.tags().iter().flat_map(lofty::tag::Tag::pictures);
    let mut fallback = None;
    for picture in pictures {
        if picture.data().len() > MAX_COVER_BYTES {
            continue;
        }
        if picture.pic_type() == PictureType::CoverFront {
            return Ok(Some(picture.data().to_vec()));
        }
        fallback = fallback.or_else(|| Some(picture.data().to_vec()));
    }
    Ok(fallback)
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
    use std::path::PathBuf;

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

    /// A playable WAV with nothing in it, as somewhere to hang a tag.
    ///
    /// Written by hand rather than checked in: a binary fixture in the
    /// repository is a thing nobody can read a diff of, and this is 44 bytes
    /// of header plus one silent frame.
    fn silent_wav(path: &Path) {
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&40u32.to_le_bytes()); // everything after this
        file.extend_from_slice(b"WAVEfmt ");
        file.extend_from_slice(&16u32.to_le_bytes()); // PCM header length
        file.extend_from_slice(&1u16.to_le_bytes()); // PCM
        file.extend_from_slice(&1u16.to_le_bytes()); // mono
        file.extend_from_slice(&44_100u32.to_le_bytes());
        file.extend_from_slice(&88_200u32.to_le_bytes()); // bytes per second
        file.extend_from_slice(&2u16.to_le_bytes()); // block align
        file.extend_from_slice(&16u16.to_le_bytes()); // bits
        file.extend_from_slice(b"data");
        file.extend_from_slice(&4u32.to_le_bytes());
        file.extend_from_slice(&[0, 0, 0, 0]);
        std::fs::write(path, file).unwrap();
    }

    fn picture(kind: PictureType, bytes: &[u8]) -> lofty::picture::Picture {
        lofty::picture::Picture::new_unchecked(
            kind,
            Some(lofty::picture::MimeType::Jpeg),
            None,
            bytes.to_vec(),
        )
    }

    /// Write `pictures` into a fresh file and hand back its path.
    fn tagged_with(dir: &tempfile::TempDir, pictures: Vec<lofty::picture::Picture>) -> PathBuf {
        use lofty::config::WriteOptions;
        use lofty::prelude::TagExt;
        let path = dir.path().join("track.wav");
        silent_wav(&path);
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::Id3v2);
        for picture in pictures {
            tag.push_picture(picture);
        }
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        path
    }

    #[test]
    fn a_file_with_no_pictures_has_no_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_with(&dir, Vec::new());
        assert_eq!(artwork(&path).unwrap(), None);
    }

    /// The one that matters. A tagger writes the pictures in its own order, so
    /// a browser that took the first would show the disc label for some albums
    /// and the sleeve for others.
    #[test]
    fn the_front_cover_wins_whatever_order_it_was_written_in() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_with(
            &dir,
            vec![
                picture(PictureType::Media, b"the disc label"),
                picture(PictureType::CoverFront, b"the sleeve"),
            ],
        );
        let cover = artwork(&path).unwrap().expect("a picture was written");
        assert_eq!(cover, b"the sleeve");
    }

    /// Plenty of records carry a band photo and no sleeve. Showing it beats
    /// showing nothing, which is the whole reason there is a fallback.
    #[test]
    fn any_picture_is_used_when_there_is_no_front_cover() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_with(&dir, vec![picture(PictureType::Band, b"the band")]);
        let cover = artwork(&path).unwrap().expect("a picture was written");
        assert_eq!(cover, b"the band");
    }

    #[test]
    fn a_file_that_cannot_be_opened_has_no_artwork_either_and_says_so() {
        assert!(artwork(Path::new("/nonexistent/track.flac")).is_err());
    }
}

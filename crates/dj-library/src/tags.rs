//! Reading metadata out of an audio file.
//!
//! Thin on purpose: `lofty` already knows every container `symphonia` decodes,
//! and the only judgement worth making here is what to do when a tag is present
//! but useless — which is the common case in a real collection.

use crate::record::Tags;
use lofty::file::TaggedFileExt;
use lofty::picture::{Picture, PictureType};
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

/// A cover image carried inside a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    /// The MIME type as the tag declares it, for the `Content-Type` header.
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// The cover a file carries, if it carries one.
///
/// §20 asks for a card view "when album/artwork is valuable", which is a view
/// djmanzo could not offer at all: the scan read every tag *except* the
/// pictures. This is the missing read.
///
/// `None` for a file with no pictures, a file that cannot be opened, and a
/// picture whose type is not an image — all three mean the same thing to a
/// card, which is that it draws its fallback. An unopenable file is *not*
/// reported here, unlike [`read`]: the browser has already listed the track
/// from a successful scan, and a second error about the same file at the
/// moment a card scrolls into view is noise a DJ can do nothing with.
#[must_use]
pub fn artwork(path: &Path) -> Option<Artwork> {
    let tagged = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = choose(tag.pictures())?;
    Some(Artwork {
        // Written by whoever tagged the file, so it is checked rather than
        // trusted: this ends up in a `Content-Type` header, and a tag that
        // says `text/html` would be a file the webview is invited to run.
        mime: match picture.mime_type().map(ToString::to_string).as_deref() {
            Some(
                mime @ ("image/png" | "image/jpeg" | "image/gif" | "image/bmp" | "image/tiff"
                | "image/webp"),
            ) => mime.to_owned(),
            // A picture with no declared type is usually a JPEG, and every
            // browser sniffs it correctly. `application/octet-stream` is the
            // honest label and it makes the image not render at all.
            _ => "image/jpeg".to_owned(),
        },
        bytes: picture.data().to_vec(),
    })
}

/// Which of a file's pictures is the cover.
///
/// The front cover if there is one, and otherwise the first picture that is
/// not obviously something else. A record with a band photo and a publisher
/// logo and no cover should draw the band photo, not the logo — but only
/// because there is nothing better, which is why the order is written down
/// rather than left to whatever the tagger happened to store first.
fn choose(pictures: &[Picture]) -> Option<&Picture> {
    pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| {
            pictures.iter().find(|p| {
                !matches!(
                    p.pic_type(),
                    PictureType::Icon
                        | PictureType::OtherIcon
                        | PictureType::BandLogo
                        | PictureType::PublisherLogo
                )
            })
        })
        .or_else(|| pictures.first())
        .filter(|p| !p.data().is_empty())
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

    fn picture(kind: PictureType, bytes: &[u8]) -> Picture {
        Picture::new_unchecked(
            kind,
            Some(lofty::picture::MimeType::Jpeg),
            None,
            bytes.to_vec(),
        )
    }

    /// **The front cover wins, whatever order the tagger stored things in.**
    ///
    /// A card drawing the publisher's logo instead of the sleeve is a card
    /// that looks broken, and which picture comes first in the tag is a
    /// property of whichever program last wrote it.
    #[test]
    fn the_front_cover_is_preferred_however_the_file_stores_it() {
        // The band photo comes *first* and is a picture the fallback would
        // happily take. That is the point: with a sleeve earlier in the list
        // than anything the fallback rejects, this test passes whether or not
        // the front-cover rule exists — which is exactly what the first
        // version of it did, and a mutation removing the rule survived it.
        let pictures = [
            picture(PictureType::Artist, b"band photo"),
            picture(PictureType::CoverFront, b"sleeve"),
            picture(PictureType::CoverBack, b"back"),
        ];
        assert_eq!(choose(&pictures).map(Picture::data), Some(&b"sleeve"[..]));
    }

    /// **Without a front cover it takes the best of the rest, not the first.**
    #[test]
    fn a_logo_is_the_last_resort_rather_than_the_first_answer() {
        let pictures = [
            picture(PictureType::BandLogo, b"logo"),
            picture(PictureType::Artist, b"band photo"),
        ];
        assert_eq!(
            choose(&pictures).map(Picture::data),
            Some(&b"band photo"[..]),
            "a logo was chosen over a photograph"
        );

        // And a logo *is* chosen when it is all there is: something is better
        // than the fallback, which says nothing about the record at all.
        let only = [picture(PictureType::BandLogo, b"logo")];
        assert_eq!(choose(&only).map(Picture::data), Some(&b"logo"[..]));
    }

    /// An empty picture is not a picture. Taggers write them.
    #[test]
    fn a_picture_with_no_bytes_is_no_picture() {
        assert_eq!(choose(&[]), None);
        assert_eq!(choose(&[picture(PictureType::CoverFront, b"")]), None);
    }

    /// A file with no pictures answers nothing rather than erroring, and so
    /// does one that is not there: a card draws its fallback for both.
    #[test]
    fn no_artwork_is_absence_rather_than_an_error() {
        assert_eq!(artwork(Path::new("/nonexistent/track.flac")), None);
    }
}

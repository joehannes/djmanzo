//! K4: a guest's photo, drawn again before anybody sees it.
//!
//! A phone's photo carries more than its picture: where it was taken, when,
//! with which phone, and often a thumbnail of what it was before it was
//! cropped. None of that belongs on a screen in a bar. So a photo is never
//! kept as it arrived: it is decoded — JPEG or PNG and nothing else, within
//! limits a hostile file cannot push past — turned the way the phone held it
//! (phones say that in the metadata this throws away, so it is applied
//! first, or every portrait would lie on its side), drawn at no more than
//! [`LONG_SIDE`] pixels on its long side, and written out as a new JPEG with
//! nothing in it but the picture (docs/KARAOKE.md §7).

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

/// The most a photo may be as sent: a phone's camera at full size.
pub const MOST_PHOTO: usize = 8 * 1024 * 1024;

/// The longest side of a photo as kept, in pixels: a screen's worth.
pub const LONG_SIDE: u32 = 1280;

/// The widest or tallest picture decoded at all. A phone's camera is under
/// 9000; a file claiming more is not a photo.
pub const MOST_SIDE: u32 = 12_000;

/// Why a photo was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Not a JPEG or PNG, or not one that decodes.
    NotAPicture,
    /// Past [`MOST_PHOTO`] or [`MOST_SIDE`].
    TooLarge,
}

/// Decode `sent`, turn it as the phone held it, draw it at most
/// [`LONG_SIDE`] on its long side, and write it as a new JPEG.
///
/// # Errors
/// When `sent` is not a JPEG or PNG that decodes within the limits.
pub fn redraw(sent: &[u8]) -> Result<Vec<u8>, Refused> {
    if sent.len() > MOST_PHOTO {
        return Err(Refused::TooLarge);
    }
    let format = image::guess_format(sent).map_err(|_| Refused::NotAPicture)?;
    if !matches!(format, ImageFormat::Jpeg | ImageFormat::Png) {
        return Err(Refused::NotAPicture);
    }
    let mut reader = ImageReader::with_format(Cursor::new(sent), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MOST_SIDE);
    limits.max_image_height = Some(MOST_SIDE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(refused)?;
    let (width, height) = decoder.dimensions();
    if width > MOST_SIDE || height > MOST_SIDE {
        return Err(Refused::TooLarge);
    }
    let orientation = decoder.orientation().map_err(|_| Refused::NotAPicture)?;
    let mut picture = DynamicImage::from_decoder(decoder).map_err(refused)?;
    picture.apply_orientation(orientation);
    if picture.width().max(picture.height()) > LONG_SIDE {
        picture = picture.resize(LONG_SIDE, LONG_SIDE, image::imageops::FilterType::Triangle);
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&picture.to_rgb8())
        .map_err(|_| Refused::NotAPicture)?;
    Ok(out)
}

/// A decoder's refusal in our terms: past a limit, or not a picture.
fn refused(error: image::ImageError) -> Refused {
    match error {
        image::ImageError::Limits(_) => Refused::TooLarge,
        _ => Refused::NotAPicture,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, ImageEncoder, RgbImage};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let picture =
            RgbImage::from_fn(width, height, |x, _| image::Rgb([(x % 256) as u8, 40, 90]));
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(
                picture.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        out
    }

    /// A JPEG `width` by `height` whose EXIF says the phone was turned a
    /// quarter clockwise (orientation 6) and carries a marker that must not
    /// survive.
    fn turned_jpeg(width: u32, height: u32) -> Vec<u8> {
        let picture = RgbImage::from_fn(width, height, |x, y| image::Rgb([x as u8, y as u8, 0]));
        let mut plain = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut plain, 90)
            .encode_image(&picture)
            .unwrap();
        // TIFF, little-endian: one IFD entry, Orientation (0x0112), SHORT,
        // count 1, value 6; then a maker note the redraw must drop.
        let mut tiff = b"II*\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&[1, 0]);
        tiff.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
        tiff.extend_from_slice(&[0, 0, 0, 0]);
        tiff.extend_from_slice(b"GPS-WHERE-IT-WAS-TAKEN");
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        let length = u16::try_from(app1.len() + 2).unwrap().to_be_bytes();
        // SOI, then APP1, then the rest of the JPEG after its own SOI.
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1, length[0], length[1]];
        out.extend_from_slice(&app1);
        out.extend_from_slice(&plain[2..]);
        out
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// **The load-bearing one: what a phone's photo carried besides the
    /// picture does not survive, and the picture stands as it was held.**
    #[test]
    fn a_photo_is_redrawn_turned_as_held_and_carrying_nothing_else() {
        let sent = turned_jpeg(400, 200);
        assert!(contains(&sent, b"GPS-WHERE-IT-WAS-TAKEN"));
        let kept = redraw(&sent).expect("a photo");
        assert!(
            !contains(&kept, b"GPS-WHERE-IT-WAS-TAKEN"),
            "the metadata survived"
        );
        assert!(!contains(&kept, b"Exif"), "an EXIF block survived");
        let back = image::load_from_memory(&kept).unwrap();
        assert_eq!(
            back.dimensions(),
            (200, 400),
            "turned as the phone was held"
        );
    }

    #[test]
    fn a_large_photo_is_drawn_at_a_screens_size_and_a_small_one_as_it_is() {
        let big = image::load_from_memory(&redraw(&png(2560, 1000)).unwrap()).unwrap();
        assert_eq!(big.dimensions(), (LONG_SIDE, 500));
        let small = image::load_from_memory(&redraw(&png(300, 200)).unwrap()).unwrap();
        assert_eq!(small.dimensions(), (300, 200));
    }

    #[test]
    fn what_is_not_a_photo_is_refused() {
        assert_eq!(
            redraw(b"<script>alert(1)</script>"),
            Err(Refused::NotAPicture)
        );
        assert_eq!(redraw(b"GIF89a\x01\0\x01\0"), Err(Refused::NotAPicture));
        // A PNG cut short.
        assert_eq!(redraw(&png(64, 64)[..60]), Err(Refused::NotAPicture));
        assert_eq!(redraw(&vec![0xFF; MOST_PHOTO + 1]), Err(Refused::TooLarge));
    }

    /// **A file that claims to be enormous is refused before it is decoded.**
    #[test]
    fn a_picture_claiming_too_many_pixels_is_refused() {
        let mut liar = png(8, 8);
        // IHDR's width, at bytes 16..20, set past the limit, and its CRC
        // (over the chunk's type and data, bytes 12..29) made right again, so
        // what refuses it is the size and not a damaged file.
        liar[16..20].copy_from_slice(&(MOST_SIDE + 1).to_be_bytes());
        let crc = crc32(&liar[12..29]);
        liar[29..33].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(redraw(&liar), Err(Refused::TooLarge));
    }

    /// PNG's CRC-32 (reflected, polynomial 0xEDB88320).
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    (crc >> 1) ^ 0xEDB8_8320
                };
            }
        }
        !crc
    }
}

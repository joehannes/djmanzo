//! K1: what the singers' screen shows behind the words.
//!
//! > 1. Album art embedded in the file.
//! > 2. Cover Art Archive via MusicBrainz — free, open, no API key.
//! > 3. A generated abstract background derived from the track's own
//! >    spectrum, so there is never a blank screen.
//! > (docs/KARAOKE.md, §3)
//!
//! In that order. The record's own cover is read by [`crate::art`] as the
//! library's cards read it. A record tagged with the MusicBrainz release it
//! came from (`dj_library::tags::release_id`) has its cover asked of the
//! Cover Art Archive once, and the answer — the image, or that there is
//! none — is kept on disk, so the archive is asked at most once a release
//! and a night without a network still has its covers.
//!
//! Beneath either, always, the record's own colours: the §110 colour of
//! each of its eight bands (`dj_render::tile::frequency_colour`), weighted
//! by how much of the record sits in it. A bass-heavy record glows red and
//! orange, a bright one blue and violet. It is also the whole background
//! when there is no cover at all, and what shows while a cover loads or if
//! it fails to — the words never wait on any of it.

use crate::art::Cover;
use dj_render::summary::{SPECTRUM_BANDS, SPECTRUM_EDGES_HZ, WaveformSummary};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One colour of the record and how much of it there is, 0..1.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shade {
    /// As CSS takes it: `rgb(r g b)`.
    pub colour: String,
    pub weight: f32,
}

/// The singers' screen's background, as the interface draws it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Backdrop {
    /// `record` for art the file carries, `archive` for the Cover Art
    /// Archive's, `sound` for the record's colours alone.
    pub source: &'static str,
    /// The record, for `art://` to serve its cover by.
    pub track: String,
    /// The record's colours, strongest first.
    pub shades: Vec<Shade>,
    /// The record's colours are still being measured — its spectrum lands
    /// off the load path, seconds after the record does — so the screen asks
    /// again in a moment rather than keeping black for the whole song.
    pub pending: bool,
}

/// How many of the record's colours are drawn.
pub const SHADES: usize = 4;

/// The middle of each of the eight bands, in Hz, by octaves.
fn band_centres() -> [f32; SPECTRUM_BANDS] {
    let mut edges = vec![20.0_f32];
    edges.extend(SPECTRUM_EDGES_HZ);
    edges.push(20_000.0);
    let mut centres = [0.0; SPECTRUM_BANDS];
    for (i, centre) in centres.iter_mut().enumerate() {
        *centre = (edges[i] * edges[i + 1]).sqrt();
    }
    centres
}

/// How much of the record sits in each of the eight bands, on average,
/// from the summary's coarsest level — a few hundred columns, the whole
/// record. `None` for a summary measured before the eight bands were.
#[must_use]
pub fn mean_spectrum(summary: &WaveformSummary) -> Option<[f32; SPECTRUM_BANDS]> {
    let level = summary.level_count().checked_sub(1)?;
    let mut sum = [0.0f32; SPECTRUM_BANDS];
    let mut counted = 0usize;
    for bucket in summary.level(level) {
        if bucket.spectrum.iter().all(|&b| b == 0) {
            continue;
        }
        for (total, &band) in sum.iter_mut().zip(&bucket.spectrum) {
            *total += f32::from(band);
        }
        counted += 1;
    }
    (counted > 0).then(|| sum.map(|total| total / counted as f32))
}

/// Whether a record's colours are still to come: no summary yet, or one
/// whose spectrum has not landed. A record that is silent throughout never
/// has one, so whoever asks again gives up in the end.
#[must_use]
pub fn pending(summary: Option<&WaveformSummary>) -> bool {
    summary.is_none_or(|summary| !summary.has_spectrum())
}

/// The record's strongest colours: each band's §110 colour, weighted by its
/// share of the strongest band, the [`SHADES`] strongest kept.
#[must_use]
pub fn shades(spectrum: &[f32; SPECTRUM_BANDS]) -> Vec<Shade> {
    let top = spectrum.iter().copied().fold(0.0f32, f32::max);
    if top <= 0.0 {
        return Vec::new();
    }
    let centres = band_centres();
    let mut bands: Vec<(usize, f32)> = spectrum
        .iter()
        .enumerate()
        .map(|(i, &level)| (i, level / top))
        .collect();
    bands.sort_by(|a, b| b.1.total_cmp(&a.1));
    bands
        .into_iter()
        .take(SHADES)
        .map(|(band, weight)| {
            let [r, g, b] = dj_render::tile::frequency_colour(centres[band]);
            Shade {
                colour: format!("rgb({r} {g} {b})"),
                weight,
            }
        })
        .collect()
}

/// Where the Cover Art Archive keeps a release's front cover, at a size a
/// screen across a room needs and no more.
#[must_use]
pub fn archive_url(release: &str) -> String {
    format!("https://coverartarchive.org/release/{release}/front-500")
}

/// What the archive said about a release, as kept on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    /// Never asked.
    Unknown,
    /// Asked: it has no cover.
    Absent,
    Found(Cover),
}

/// The archive's answer for `release`, if it was kept in `dir`: the cover as
/// `<release>.jpg` or `.png`, or `<release>.none` for a release without one.
#[must_use]
pub fn kept(dir: &Path, release: &str) -> Kept {
    for (extension, mime) in [("jpg", "image/jpeg"), ("png", "image/png")] {
        if let Ok(bytes) = std::fs::read(dir.join(format!("{release}.{extension}"))) {
            return Kept::Found(Cover {
                mime: mime.to_owned(),
                bytes: Arc::new(bytes),
            });
        }
    }
    if dir.join(format!("{release}.none")).exists() {
        return Kept::Absent;
    }
    Kept::Unknown
}

/// Keep the archive's answer for `release` in `dir`.
///
/// # Errors
/// The file system's own sentence.
pub fn keep(dir: &Path, release: &str, answer: Option<&Cover>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = match answer {
        Some(cover) => {
            let extension = if cover.mime == "image/png" {
                "png"
            } else {
                "jpg"
            };
            let path = dir.join(format!("{release}.{extension}"));
            std::fs::write(&path, cover.bytes.as_slice()).map_err(|e| e.to_string())?;
            path
        }
        None => {
            let path = dir.join(format!("{release}.none"));
            std::fs::write(&path, b"").map_err(|e| e.to_string())?;
            path
        }
    };
    Ok(path)
}

/// The largest cover taken from the archive; its `front-500` images are a
/// fraction of this.
pub const MOST_BYTES: usize = 4 * 1024 * 1024;

/// What an answer from the archive is: a cover when it is a JPEG or PNG of a
/// sane size, none when the archive has none (404), and an error — not kept,
/// so asked again another night — for anything else.
///
/// # Errors
/// A status other than success or 404, or a body that is not an image.
pub fn answer(status: u16, mime: Option<&str>, bytes: Vec<u8>) -> Result<Option<Cover>, String> {
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(format!("the Cover Art Archive answered {status}"));
    }
    let mime = match mime.map(|m| m.split(';').next().unwrap_or("").trim()) {
        Some(m @ ("image/jpeg" | "image/png")) => m.to_owned(),
        other => {
            return Err(format!(
                "the Cover Art Archive sent {other:?}, not an image"
            ));
        }
    };
    if bytes.is_empty() || bytes.len() > MOST_BYTES {
        return Err(format!("a cover of {} bytes was not used", bytes.len()));
    }
    Ok(Some(Cover {
        mime,
        bytes: Arc::new(bytes),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The record's own colours**: the strongest bands first, each in the
    /// §110 colour of its middle pitch, weighted by its share of the
    /// strongest — a bass-heavy record's first colour is the bass's.
    #[test]
    fn a_record_is_drawn_in_its_own_colours() {
        let bassy = [40.0, 200.0, 160.0, 80.0, 60.0, 30.0, 20.0, 10.0];
        let drawn = shades(&bassy);
        assert_eq!(drawn.len(), SHADES);
        let centres = band_centres();
        let [r, g, b] = dj_render::tile::frequency_colour(centres[1]);
        assert_eq!(
            drawn[0].colour,
            format!("rgb({r} {g} {b})"),
            "the kick's band first"
        );
        assert!((drawn[0].weight - 1.0).abs() < 1e-6);
        assert!(
            (drawn[1].weight - 0.8).abs() < 1e-6,
            "the bass, at its share"
        );
        assert!(drawn.windows(2).all(|w| w[0].weight >= w[1].weight));
        // A bright record's first colour is not the bassy record's.
        let bright = [10.0, 20.0, 30.0, 60.0, 80.0, 160.0, 200.0, 40.0];
        assert_ne!(shades(&bright)[0].colour, drawn[0].colour);
        assert!(
            shades(&[0.0; SPECTRUM_BANDS]).is_empty(),
            "silence has no colours"
        );
        // The bands' middles climb, from sub-bass to air.
        assert!(centres.windows(2).all(|w| w[0] < w[1]));
        assert!(centres[0] > 20.0 && centres[7] < 20_000.0);
    }

    /// **The colours come after the record does.** A deck's summary is built
    /// as the record loads and its spectrum measured afterwards, off that
    /// path; until then there is nothing to draw and the screen is told to
    /// ask again, and once it lands the record's strongest colour is the
    /// band its sound is in.
    #[test]
    fn a_records_colours_are_pending_until_its_spectrum_lands() {
        assert!(pending(None), "nothing loaded yet");
        let tone: Vec<f32> = (0..96_000)
            .flat_map(|n| {
                let v = (std::f32::consts::TAU * 2_000.0 * n as f32 / 48_000.0).sin() * 0.8;
                [v, v]
            })
            .collect();
        let mut summary = WaveformSummary::analyse(&tone, dj_core::SampleRate::DEFAULT);
        assert!(pending(Some(&summary)), "loaded, not yet measured");
        assert_eq!(mean_spectrum(&summary), None);
        summary.measure_spectrum(&tone);
        assert!(!pending(Some(&summary)));
        let drawn = shades(&mean_spectrum(&summary).expect("measured"));
        // 2 kHz sits between 1.5 and 3.56 kHz: the sixth band.
        let [r, g, b] = dj_render::tile::frequency_colour(band_centres()[5]);
        assert_eq!(drawn[0].colour, format!("rgb({r} {g} {b})"));
    }

    #[test]
    fn the_archive_is_asked_once_a_release_and_its_answer_kept() {
        let dir = tempfile::tempdir().expect("a folder");
        let release = "f1f6a6bf-7c3d-4a1e-9a44-2d5b0c1e8f00";
        assert_eq!(kept(dir.path(), release), Kept::Unknown);
        keep(dir.path(), release, None).expect("kept");
        assert_eq!(
            kept(dir.path(), release),
            Kept::Absent,
            "no cover is an answer too"
        );
        let cover = Cover {
            mime: "image/png".into(),
            bytes: Arc::new(vec![1, 2, 3]),
        };
        let other = "00000000-0000-4000-8000-000000000000";
        keep(dir.path(), other, Some(&cover)).expect("kept");
        assert_eq!(kept(dir.path(), other), Kept::Found(cover));
        assert!(archive_url(release).ends_with(&format!("/release/{release}/front-500")));
    }

    #[test]
    fn only_an_image_from_the_archive_is_a_cover() {
        let jpeg = answer(200, Some("image/jpeg"), vec![0xFF, 0xD8]).expect("a cover");
        assert_eq!(jpeg.map(|c| c.mime), Some("image/jpeg".to_owned()));
        assert_eq!(answer(404, None, Vec::new()), Ok(None), "no cover");
        assert!(answer(503, None, Vec::new()).is_err(), "asked again later");
        assert!(answer(200, Some("text/html"), vec![1]).is_err());
        assert!(answer(200, Some("image/png"), Vec::new()).is_err());
        assert!(answer(200, Some("image/png"), vec![0; MOST_BYTES + 1]).is_err());
    }
}

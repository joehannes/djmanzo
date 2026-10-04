//! §122: a video on a deck — through the one load funnel every load takes.
//!
//! What makes a file a video to djmanzo is what it is called; what plays is
//! its sound. So the fixture is a WAV named as a QuickTime movie: the decoder
//! reads its sound by what is in it, and the funnel sees a file with a
//! picture. What is held: the picture is served while its record is on the
//! deck, and not once a record without one replaces it or the deck is
//! ejected.

use dj_app::state::AppState;
use dj_core::DeckId;
use std::path::Path;

/// A short stereo WAV at `path`, its tone `pitch` so two of them are two
/// different records.
fn a_wav(path: &Path, pitch: f32) {
    let rate = 48_000u32;
    let frames = 4_800usize;
    let data = frames * 2 * 2;
    let mut wav = Vec::with_capacity(44 + data);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&u32::try_from(36 + data).unwrap().to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&u32::try_from(data).unwrap().to_le_bytes());
    for frame in 0..frames {
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let sample = ((frame as f32 * pitch).sin() * 8000.0) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, wav).expect("the fixture is written");
}

fn load(state: &AppState, path: &Path) -> String {
    let decoded = dj_decode::decode_file(path).expect("the fixture decodes");
    let id = decoded.id.to_hex();
    dj_app::commands::put_on_deck(state, DeckId::from_human(1).unwrap(), decoded)
        .expect("the load");
    id
}

/// **A video's picture is served while its record is on the deck, and no
/// longer**: a record without a picture takes it away, and so does an eject.
#[test]
fn a_videos_picture_lasts_as_long_as_its_record_on_the_deck() {
    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("clip.mov");
    let song = dir.path().join("song.wav");
    a_wav(&clip, 0.05);
    a_wav(&song, 0.07);

    let state = AppState::new(true);
    state.host().open(None, None, 128).unwrap();

    let record = load(&state, &clip);
    assert_eq!(
        state.pictures().all(),
        vec![(1, record.clone(), "video/quicktime")]
    );
    assert!(state.pictures().of(1, &record).is_some());

    load(&state, &song);
    assert!(state.pictures().all().is_empty(), "a song took the deck");
    assert!(state.pictures().of(1, &record).is_none());

    load(&state, &clip);
    assert_eq!(state.pictures().all().len(), 1);
    // Through the application, as the eject button and a controller reach it.
    dj_app::commands::perform_action(&state, dj_core::Action::parse("deck 1 eject").unwrap())
        .expect("the eject");
    assert!(state.pictures().all().is_empty(), "ejected");
}

//! §87: a record loaded from anywhere looks the same afterwards.
//!
//! > If track loading originates from: browser, assistant, preset, controller,
//! > keyboard, network, drag & drop — the resulting UI must look identical.
//! > One source of state truth.
//!
//! The section is a claim about *convergence*, and the only way to hold it is a
//! test that actually drives the different paths and compares what is left
//! behind. A reading of the code proves nothing here: every one of these
//! origins was written at a different time, and the way this breaks is that
//! somebody adds an eighth and sets the deck's name themselves because it is
//! two lines and obviously correct.
//!
//! # What "identical" means
//!
//! Everything a load leaves behind that the interface then draws from:
//!
//! - **The deck's name**, which is what the header shows.
//! - **The waveform summary**, which is what the lane is rasterised from. Its
//!   absence is a deck with a title and an empty lane, which is the shape of
//!   defect this is most likely to catch.
//! - **The library row**, because a record you played is in your collection
//!   whether or not you ever scanned the folder.
//! - **The session log**, because a set is not reproducible from its actions
//!   alone and a load that reached the deck without reaching the log is a set
//!   that replays as silence.
//!
//! What is deliberately *not* compared is anything with a clock in it. Two
//! loads happen at two times, and a test that demanded the same timestamp would
//! be a test of nothing.
//!
//! # The origins this can drive
//!
//! `put_on_deck` — where the browser, a drop, the Next rail, the SideView and
//! §22's rail all arrive, through the `load_track` command — and
//! `perform("load deck N <id>")`, which is the controller, the command palette,
//! a script and the line protocol. The assistant's own staging calls
//! `put_on_deck` directly and is covered by the first.
//!
//! Uses the null audio backend, so there is a real engine accepting the load.

use dj_app::state::AppState;
use dj_core::{DeckId, SampleRate, TrackId};

const SR: SampleRate = SampleRate::DEFAULT;

fn deck() -> DeckId {
    DeckId::from_human(1).unwrap()
}

/// Two seconds of a quiet tone, written where `decode_file` can read it.
///
/// A real file rather than an `AudioBuffer`, because half the point of §87 is
/// that a load *decodes* — the id, the tags and the summary all come out of
/// that, and a test that skipped it would compare two things neither origin
/// produces.
fn a_wav_on_disk(dir: &std::path::Path) -> std::path::PathBuf {
    let frames = SR.get() as usize * 2;
    let channels = 2u16;
    let bytes_per_sample = 2u16;
    let data_len = frames * channels as usize * bytes_per_sample as usize;

    let mut wav = Vec::with_capacity(44 + data_len);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&u32::try_from(36 + data_len).unwrap().to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&SR.get().to_le_bytes());
    let byte_rate = SR.get() * u32::from(channels) * u32::from(bytes_per_sample);
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&(channels * bytes_per_sample).to_le_bytes());
    wav.extend_from_slice(&(bytes_per_sample * 8).to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&u32::try_from(data_len).unwrap().to_le_bytes());
    // Not silence: a summary of nothing is a summary that cannot tell a deck
    // that loaded from one that did not.
    for frame in 0..frames {
        #[allow(clippy::cast_possible_truncation)]
        let sample = ((frame as f32 * 0.05).sin() * 8000.0) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    let path = dir.join("origin.wav");
    std::fs::write(&path, wav).expect("the fixture is written");
    path
}

/// Everything a load leaves behind, as a comparable value.
#[derive(Debug, PartialEq)]
struct AfterALoad {
    title: String,
    artist: Option<String>,
    track: TrackId,
    /// The lane has something to draw. A summary's *shape* rather than its
    /// bytes: two decodes of one file give the same peaks, and asserting on
    /// them would be asserting about the decoder here.
    summarised: bool,
    in_the_library: bool,
    /// The session log's account of it: `load deck 1 <id>`.
    logged: Vec<String>,
}

fn look(state: &AppState) -> AfterALoad {
    let tracks = state.deck_tracks();
    let held = tracks.lock().expect("nothing poisons this");
    let info = held
        .get(&deck().human_number())
        .expect("a deck with a record on it");
    let db = state.library().get().expect("a library");
    AfterALoad {
        title: info.title.clone(),
        artist: info.artist.clone(),
        track: info.id,
        summarised: state.waveforms().summary(deck().human_number()).is_some(),
        in_the_library: db.track(info.id).ok().flatten().is_some(),
        logged: state
            .bus()
            .log()
            .into_iter()
            .map(|timed| timed.event.to_line())
            .collect(),
    }
}

fn running_app() -> AppState {
    let state = AppState::new(true);
    state.host().open(None, None, 128).unwrap();
    state
}

/// **The load-bearing one: two origins, one resulting state.**
///
/// The browser's path — `put_on_deck`, which every panel in the interface
/// reaches through `load_track` — against the bus's, which is a controller
/// button, a line in a script, the command palette and the network protocol.
/// They were written years apart and this is the first thing that has ever
/// compared them.
#[test]
fn a_record_loaded_from_the_bus_leaves_the_same_state_as_one_loaded_from_the_browser() {
    let dir = std::env::temp_dir().join(format!("djmanzo-origins-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a place for the fixture");
    let path = a_wav_on_disk(&dir);

    // The browser, a drop, the Next rail, the SideView: all of them arrive here.
    let browser = running_app();
    let decoded = dj_decode::decode_file(&path).expect("the fixture decodes");
    let id = decoded.id;
    dj_app::commands::put_on_deck(&browser, deck(), decoded).expect("the browser's load");
    let from_browser = look(&browser);

    // A controller, the palette, a script, the line protocol.
    let bus = running_app();
    // The row a controller names its record by. Written first, because the bus
    // loads by id and an id is only a record if the library has it — which is
    // exactly the state a DJ is in, since anything they can see to bind a
    // button to is already in their collection.
    let decoded = dj_decode::decode_file(&path).expect("the fixture decodes");
    dj_app::commands::put_on_deck(&bus, DeckId::from_human(2).unwrap(), decoded)
        .expect("the record reaches the library");
    let before = bus.bus().log().len();
    dj_app::commands::perform(&bus, &format!("load deck 1 {}", id.to_hex()))
        .expect("the bus's load");
    let mut from_bus = look(&bus);
    // The log carries both loads on this one; compare only what the second did.
    from_bus.logged = from_bus.logged.split_off(before);

    assert_eq!(
        from_browser.logged,
        vec![format!("load deck 1 {}", id.to_hex())],
        "the browser's load did not reach the session log, so a set replayed \
         from it plays silence"
    );
    assert_eq!(
        from_browser, from_bus,
        "the same record loaded from two of §87's origins left two different \
         states -- which is the section's whole subject: one source of state \
         truth"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A load that names a record the library does not have says which record.
///
/// The failure a controller button actually hits: a track moved off the disk,
/// or a mapping written against somebody else's collection. "Load failed" sends
/// a DJ to the wrong place.
#[test]
fn a_load_naming_a_record_that_is_not_there_says_which_record() {
    let state = running_app();
    let id = TrackId::from_bytes([9; 32]);
    let refused = dj_app::commands::perform(&state, &format!("load deck 1 {}", id.to_hex()))
        .expect_err("a record that is not in the library was loaded anyway");
    assert!(
        refused.contains(&id.to_hex()),
        "the refusal does not name the record: {refused}"
    );
}

/// The verb refuses what it cannot make sense of, rather than half-doing it.
#[test]
fn a_malformed_load_is_refused_at_the_half_that_is_wrong() {
    let state = running_app();
    let refused = |line: &str| {
        dj_app::commands::perform(&state, line).expect_err(&format!("{line:?} was accepted"))
    };
    // `load deck` with nothing after it is not the verb at all -- the prefix
    // carries the space -- so it falls through to the action vocabulary and is
    // refused there. Refused either way, which is what matters; the sentence is
    // the vocabulary's rather than this verb's.
    assert!(refused("load deck").contains("load deck"));
    assert!(refused("load deck 1").contains("deck and a track"));
    assert!(refused("load deck 99 aabb").contains("not a deck"));
    assert!(refused("load deck 1 not-a-hex-id").contains("not a track id"));
}

/// §87 by path rather than by id is deliberately not in the vocabulary.
///
/// A track id names a record in the DJ's own library and the path comes from
/// the row. Accepting a path would hand anything that can reach the bus — a
/// controller mapping somebody else wrote, a socket — a way to make djmanzo
/// read an arbitrary file off the disk. §87 asks for consistency between
/// origins, not for a new capability.
#[test]
fn a_load_will_not_take_a_path() {
    let state = running_app();
    assert!(dj_app::commands::perform(&state, "load deck 1 /etc/passwd").is_err());
    assert!(dj_app::commands::perform(&state, "load deck 1 ../../secret.wav").is_err());
}

/// **§87 for the verbs, not only for the loads: an eject means the same thing
/// whichever origin sent it.**
///
/// The defect this is the fix for. A socket used to dispatch straight at the
/// engine's bus, which is *not* where an eject finishes: the deck's name and
/// its analysis live in the application, and `commands::perform_action` is what
/// clears them. So `deck 1 eject` over the network ejected the audio and left
/// the interface showing a record that was no longer on the deck — the exact
/// shape §87 forbids, from the one origin that was not going through djmanzo's
/// own entry point.
///
/// Both roads are driven here, because the assertion only means something as a
/// pair: the application's entry point clears the deck, and the bare bus does
/// not. The second half is the defect, kept as a test so that routing the
/// network back at the bus one day fails loudly instead of quietly.
#[test]
fn an_eject_through_the_application_clears_the_deck_and_through_the_bus_alone_does_not() {
    let name = |state: &AppState| {
        let tracks = state.deck_tracks();
        let held = tracks.lock().expect("nothing poisons this");
        held.get(&deck().human_number())
            .map(|info| info.title.clone())
    };
    let loaded = |state: &AppState| {
        state.set_deck_track(
            deck(),
            dj_app::state::LoadedTrackInfo {
                title: "A Record".to_owned(),
                artist: None,
                id: TrackId::from_bytes([3; 32]),
            },
        );
    };
    let eject = dj_core::Action::parse("deck 1 eject").expect("the vocabulary has eject");

    // What a socket does now, and what the browser has always done.
    let through_the_app = running_app();
    loaded(&through_the_app);
    dj_app::commands::perform_action(&through_the_app, eject).expect("the eject");
    assert_eq!(
        name(&through_the_app),
        None,
        "an eject through the application left the deck's name behind, so the \
         header shows a record that is not loaded"
    );

    // What a socket used to do.
    let past_the_app = running_app();
    loaded(&past_the_app);
    past_the_app
        .bus()
        .dispatch(eject)
        .expect("the bus takes it");
    assert_eq!(
        name(&past_the_app),
        Some("A Record".to_owned()),
        "the bus alone now clears the deck's name -- if that is deliberate, the \
         reason `perform_action` exists has changed and §87's network path \
         should be reconsidered with it"
    );
}

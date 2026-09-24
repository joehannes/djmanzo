//! §108: going live — the record on the stream.
//!
//! [`dj_net::overlay`] serves the words to a Browser source; this decides
//! them, writes them to a file for a Text source, and switches both on and
//! off. Off until the DJ switches it on, like everything else that speaks
//! outside djmanzo.
//!
//! # Which record
//!
//! Not every playing deck, which is what the request page and a note say (a
//! note taken mid-blend is about both records). A stream's line names one:
//! **the record the room hears most** — each playing deck's channel fader
//! times what the crossfader lets through on its side — and it changes hands
//! only when another is clearly louder ([`TAKES_OVER`]), so a blend does not
//! flick the name back and forth while two records ride level. A cue in the
//! headphones is not heard by the room and never names the stream.
//!
//! # The file
//!
//! Written whole into a neighbour and renamed over, so a Text source never
//! reads half a title. It is written as soon as the switch goes on — OBS does
//! not notice a file that appears after it started reading — and emptied when
//! it goes off, so the stream stops naming a record the DJ has stopped
//! announcing.

use dj_dsp::{CrossfaderCurve, crossfader_gains};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Whether the stream is told. Off on a fresh install.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub on: bool,
}

/// The file a Text source reads, in djmanzo's own folder.
pub const FILE_NAME: &str = "now-playing.txt";

/// How often the words are decided. Twice a second: a stream is a few
/// seconds behind the room anyway, and OBS reads its file once a second.
pub const LOOK_EVERY: Duration = Duration::from_millis(500);

/// Below this a deck is not heard, whatever its transport says: a channel
/// pulled all the way down, or cut by the crossfader.
pub const SILENT: f32 = 0.05;

/// How much louder another deck must be to take the name: half as loud
/// again, about 3.5 dB. Two records riding level mid-blend keep the name
/// where it was.
pub const TAKES_OVER: f32 = 1.5;

/// One deck, as far as the room hearing it goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    pub deck: u8,
    pub playing: bool,
    /// Channel fader times the crossfader's gain on its side, 0..=1.
    pub level: f32,
}

/// How loud a deck is in the room: its channel fader, times what the
/// crossfader passes on its side (`assign` as `DeckParam::CrossfaderAssign`
/// carries it: negative left, positive right, zero through).
#[must_use]
pub fn level(volume: f32, assign: f32, crossfader: f32) -> f32 {
    let volume = if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (left, right) = crossfader_gains(crossfader, CrossfaderCurve::default());
    let side = match dj_core::deck::CrossfaderAssign::from_param(assign) {
        dj_core::deck::CrossfaderAssign::Left => left,
        dj_core::deck::CrossfaderAssign::Right => right,
        dj_core::deck::CrossfaderAssign::Thru => 1.0,
    };
    volume * side
}

/// **The deck the stream names**, given the one it names now.
///
/// The loudest deck the room can hear; but the one already named keeps the
/// name until another is [`TAKES_OVER`] times louder, or it stops being heard.
#[must_use]
pub fn lead(decks: &[Heard], current: Option<u8>) -> Option<u8> {
    let heard = |d: &&Heard| d.playing && d.level > SILENT;
    let loudest = decks
        .iter()
        .filter(heard)
        .max_by(|a, b| a.level.total_cmp(&b.level))?;
    let Some(named) = current.and_then(|n| decks.iter().filter(heard).find(|d| d.deck == n)) else {
        return Some(loudest.deck);
    };
    if loudest.level > named.level * TAKES_OVER {
        Some(loudest.deck)
    } else {
        Some(named.deck)
    }
}

/// Write `text` whole, so a reader never sees half of it.
///
/// # Errors
/// When the folder cannot be written.
pub fn write_whole(path: &Path, text: &str) -> std::io::Result<()> {
    let partial = path.with_extension("txt.part");
    std::fs::write(&partial, text)?;
    std::fs::rename(&partial, path)
}

/// What the Settings block shows.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Status {
    pub on: bool,
    /// The address for a Browser source, while it is being served.
    pub overlay: Option<String>,
    /// The file for a Text source.
    pub file: Option<String>,
    /// What the stream is being told right now. Empty for nothing.
    pub saying: String,
    /// Why part of it is not working, in words.
    pub problem: Option<String>,
}

/// The words for a deck: its record, as the request book names records.
fn words_for(state: &crate::state::AppState, deck: Option<u8>) -> String {
    let Some(deck) = deck else {
        return String::new();
    };
    state
        .deck_tracks()
        .lock()
        .ok()
        .and_then(|tracks| tracks.get(&deck).map(crate::track_name))
        .unwrap_or_default()
}

/// Every deck as the room hears it, from the parameter table.
fn heard(state: &crate::state::AppState) -> Vec<Heard> {
    use dj_core::param::{DeckParam, GlobalParam};
    use dj_core::{DeckId, ParamId};
    let registry = state.registry();
    let crossfader = registry.get(ParamId::Global(GlobalParam::Crossfader));
    (1..=state.deck_count())
        .filter_map(|n| u8::try_from(n).ok())
        .filter_map(|n| DeckId::from_human(n).map(|d| (n, d)))
        .map(|(n, d)| Heard {
            deck: n,
            playing: registry.get(ParamId::Deck(d, DeckParam::Playing)) > 0.5,
            level: level(
                registry.get(ParamId::Deck(d, DeckParam::Volume)),
                registry.get(ParamId::Deck(d, DeckParam::CrossfaderAssign)),
                crossfader,
            ),
        })
        .collect()
}

/// Where the file goes.
fn file_in(state: &crate::state::AppState) -> Option<PathBuf> {
    state.config_dir().map(|dir| dir.join(FILE_NAME))
}

/// Decide, write and serve, twice a second, for as long as djmanzo runs.
pub fn start(handle: tauri::AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("live".into())
        .spawn(move || {
            use tauri::Manager;
            let now = state_now(&handle);
            let mut server: Option<dj_net::web::WebServer> = None;
            let mut named: Option<u8> = None;
            let mut written: Option<String> = None;
            loop {
                let state: tauri::State<'_, crate::state::AppState> = handle.state();
                let file = file_in(&state);
                if !state.live().on {
                    if server.take().is_some() || written.is_some() {
                        // Said once on the way out: the stream stops naming
                        // a record the DJ has stopped announcing.
                        if let Some(file) = &file {
                            let _ = write_whole(file, "");
                        }
                        now.set("");
                        written = None;
                        named = None;
                    }
                    state.set_live_status(Status {
                        on: false,
                        file: file.map(|f| f.to_string_lossy().into_owned()),
                        ..Status::default()
                    });
                    std::thread::sleep(LOOK_EVERY);
                    continue;
                }

                let mut problem = None;
                if server.is_none() {
                    let address = std::net::SocketAddr::from((
                        std::net::Ipv4Addr::LOCALHOST,
                        dj_net::overlay::DEFAULT_PORT,
                    ));
                    let overlay = dj_net::overlay::Overlay::new(std::sync::Arc::clone(&now));
                    match dj_net::web::WebServer::start(address, std::sync::Arc::new(overlay)) {
                        Ok(started) => server = Some(started),
                        Err(error) => {
                            problem = Some(format!(
                                "The overlay page could not start: {error}. The file still works."
                            ))
                        }
                    }
                }

                named = lead(&heard(&state), named);
                let saying = words_for(&state, named);
                now.set(&saying);
                if written.as_deref() != Some(saying.as_str()) {
                    match &file {
                        Some(file) => match write_whole(file, &saying) {
                            Ok(()) => written = Some(saying.clone()),
                            Err(error) => {
                                problem = Some(format!(
                                    "{} could not be written: {error}",
                                    file.display()
                                ));
                            }
                        },
                        None => {
                            problem = Some(
                                "djmanzo has no folder of its own to write the file in yet."
                                    .to_owned(),
                            )
                        }
                    }
                }
                state.set_live_status(Status {
                    on: true,
                    overlay: server.as_ref().map(|s| format!("http://{}/", s.address())),
                    file: file.map(|f| f.to_string_lossy().into_owned()),
                    saying,
                    problem,
                });
                std::thread::sleep(LOOK_EVERY);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for going live; the stream will not be told what is playing");
    }
}

/// The shared words, held by the application so every server sees one.
fn state_now(handle: &tauri::AppHandle) -> std::sync::Arc<dj_net::overlay::NowPlaying> {
    use tauri::Manager;
    let state: tauri::State<'_, crate::state::AppState> = handle.state();
    state.live_now()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(deck: u8, playing: bool, level: f32) -> Heard {
        Heard {
            deck,
            playing,
            level,
        }
    }

    /// **The stream names the record the room hears most.**
    #[test]
    fn the_loudest_heard_deck_is_named() {
        let decks = [deck(1, true, 0.3), deck(2, true, 0.9)];
        assert_eq!(lead(&decks, None), Some(2));
        // A deck cued in the headphones, loaded and paused, or pulled down is
        // not heard by the room.
        let decks = [deck(1, true, 0.8), deck(2, false, 1.0), deck(3, true, 0.01)];
        assert_eq!(lead(&decks, None), Some(1));
        assert_eq!(lead(&[deck(1, false, 1.0)], Some(1)), None);
        assert_eq!(lead(&[], None), None);
    }

    /// **A blend does not flick the name back and forth.** The named record
    /// keeps it while the two ride near level, and gives it up once the new
    /// one is clearly louder — or once it stops.
    #[test]
    fn the_name_changes_hands_once_per_blend() {
        let mut named = lead(&[deck(1, true, 1.0), deck(2, true, 0.0)], None);
        assert_eq!(named, Some(1));
        // The incoming record comes up; the crossfader moves across, with the
        // wobble of a hand riding the faders — which is what would flick the
        // name back and forth where the two cross, without the margin.
        let mut changes = 0;
        for step in 0..=40 {
            let x = step as f32 / 40.0;
            let wobble = if step % 2 == 0 { 0.06 } else { -0.06 };
            let decks = [
                deck(1, true, (1.0 - x + wobble).max(0.0)),
                deck(2, true, (x - wobble).max(0.0)),
            ];
            let next = lead(&decks, named);
            if next != named {
                changes += 1;
            }
            named = next;
        }
        assert_eq!(changes, 1, "the name flicked during the blend");
        assert_eq!(named, Some(2));
        // Level, the old one keeps it; a little louder is not enough.
        assert_eq!(
            lead(&[deck(1, true, 0.6), deck(2, true, 0.8)], Some(1)),
            Some(1)
        );
        assert_eq!(
            lead(&[deck(1, true, 0.5), deck(2, true, 0.8)], Some(1)),
            Some(2)
        );
        // The named one stops: the other takes it at once.
        assert_eq!(
            lead(&[deck(1, false, 1.0), deck(2, true, 0.2)], Some(1)),
            Some(2)
        );
    }

    /// The room hears a deck through its fader and its side of the
    /// crossfader; a deck through the crossfader hears only its fader.
    #[test]
    fn a_deck_is_as_loud_as_its_fader_and_its_side() {
        // Crossfader hard left: the right side is cut.
        assert!((level(1.0, -1.0, -1.0) - 1.0).abs() < 1e-6);
        assert!(level(1.0, 1.0, -1.0) < 1e-6);
        // Through ignores the crossfader.
        assert!((level(0.7, 0.0, -1.0) - 0.7).abs() < 1e-6);
        // In the middle, both sides at the constant-power 71 %.
        assert!((level(1.0, 1.0, 0.0) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
        assert_eq!(level(f32::NAN, 0.0, 0.0), 0.0);
    }

    /// The file is replaced whole, and holds exactly the words.
    #[test]
    fn the_file_is_written_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        write_whole(&path, "Aventura - Obsesión").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Aventura - Obsesión"
        );
        write_whole(&path, "").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(left.len(), 1, "a partial file was left behind");
    }
}

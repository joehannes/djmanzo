//! Karaoke beta B5: a guest's song, from Suno's window to WhatsApp.
//!
//! Suno has no public API and its terms forbid robots, so djmanzo does what
//! an autofill does and no more (the owner's choice, 16 October): it fills
//! Suno's Custom form in djmanzo's own Suno window with the words and the
//! style ([`fill_script`]), and the DJ checks them and presses Create. Once
//! the DJ opens the finished song there, its address is the song's link
//! ([`song_link`]): djmanzo reads only the window's address, never the page.
//! The link then goes to the guest — only one who agreed to be contacted —
//! in WhatsApp Web in a window of djmanzo's own with the message written
//! ([`crate::guests::whatsapp_web`]); the DJ presses send. Neither window is
//! named by a capability, so neither page has any way into djmanzo.

use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{Manager, State};

use crate::state::AppState;

/// djmanzo's own Suno window — the same one *Open Suno* opens.
pub const SUNO: &str = "suno";

/// djmanzo's own WhatsApp Web window.
pub const WHATSAPP: &str = "whatsapp";

/// What runs in Suno's page to fill its form; see the file for what it does
/// and does not do.
const FILL: &str = include_str!("suno_fill.js");

/// How long a fill waits for Suno's form: long enough for a DJ to sign in.
pub const WAITS: Duration = Duration::from_secs(15 * 60);

/// A fill waiting for Suno's form: Suno shows its sign-in page first to a
/// DJ who is not signed in, and the page that held the words is gone.
#[derive(Debug, Clone)]
pub struct Pending {
    pub script: String,
    pub until: Instant,
}

impl Pending {
    /// Whether a page that finished loading at `address` at `now` is given
    /// the fill: a page on suno.com, while it waits.
    #[must_use]
    pub fn due(&self, address: &str, now: Instant) -> bool {
        let on_suno = address
            .strip_prefix("https://")
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .is_some_and(|host| {
                host.eq_ignore_ascii_case("suno.com") || host.eq_ignore_ascii_case("www.suno.com")
            });
        on_suno && now < self.until
    }
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Whether the Suno window open now gives each page it loads a waiting fill:
/// one opened here does; one *Open Suno* opened does not.
static WAITING_WINDOW: AtomicBool = AtomicBool::new(false);

/// The script that fills Suno's form with `lyrics`, `style` and `title`:
/// the data as JSON, so nothing in the words can end the script early, then
/// the page script (`suno_fill.js`). `overwrite` is a press of the DJ's; a
/// fill waiting for the form leaves a box the DJ has written in alone. `waits` says the window will
/// fill the form by itself once Create opens, so the page can say so.
#[must_use]
pub fn fill_script(lyrics: &str, style: &str, title: &str, overwrite: bool, waits: bool) -> String {
    let data = serde_json::json!({
        "lyrics": lyrics, "style": style, "title": title, "overwrite": overwrite, "waits": waits
    });
    format!("window.__djmanzoSuno = {data};\n{FILL}")
}

/// The song's link, when `address` is a song on Suno: `/song/<id>` or the
/// short `/s/<id>`, on suno.com, without what follows a `?` or `#`. Anything
/// else — Create, the library, a sign-in page, another site — is not one.
#[must_use]
pub fn song_link(address: &str) -> Option<String> {
    let rest = address
        .strip_prefix("https://")
        .or_else(|| address.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let host = host.to_ascii_lowercase();
    if host != "suno.com" && host != "www.suno.com" {
        return None;
    }
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let (kind, id) = path.trim_end_matches('/').split_once('/')?;
    let well_formed = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    (matches!(kind, "song" | "s") && well_formed).then(|| format!("https://suno.com/{kind}/{id}"))
}

/// What the Singers panel needs to fill Suno's form for a guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Filling {
    /// The language of the words put in.
    pub language: String,
    /// Whether Suno's window fills the form by itself once the DJ has signed
    /// in and Create opens; if not, the DJ presses *Fill* again then.
    pub waits: bool,
}

/// The words in `language` from the guest's song, else the first set, and
/// the style — or why there is nothing to fill with.
///
/// # Errors
/// A guest with no song written for them yet.
pub fn words_for(
    guest: &crate::guests::Guest,
    language: &str,
) -> Result<(String, String, String), String> {
    let draft = guest
        .sang
        .last()
        .and_then(|sang| sang.song.as_ref())
        .ok_or_else(|| format!("No words have been written for {} yet", guest.name))?;
    let version = draft
        .versions
        .iter()
        .find(|version| version.language == language)
        .or_else(|| draft.versions.first())
        .ok_or_else(|| format!("No words have been written for {} yet", guest.name))?;
    Ok((
        version.lyrics.clone(),
        draft.style.clone(),
        version.language.clone(),
    ))
}

/// Fill Suno's form in djmanzo's Suno window with a guest's words and style,
/// opening the window on Create if it is not open. The DJ presses Create.
///
/// # Errors
/// A guest not in the journal or with no words yet, or a window that will
/// not open.
#[tauri::command]
pub fn suno_fill(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    language: String,
) -> Result<Filling, String> {
    let journal = state.guests();
    let guest = journal
        .get(&id)
        .ok_or_else(|| crate::guests::Refusal::Unknown.to_string())?;
    let (lyrics, style, language) = words_for(guest, &language)?;
    let title = format!("A song for {}", guest.name.trim());
    // Kept for the next pages Suno loads, too: its sign-in, then Create.
    *PENDING.lock().map_err(|_| "Suno's window is busy")? = Some(Pending {
        script: fill_script(&lyrics, &style, &title, false, true),
        until: Instant::now() + WAITS,
    });
    if let Some(window) = app.get_webview_window(SUNO) {
        let _ = window.show();
        let _ = window.set_focus();
        window
            .eval(fill_script(&lyrics, &style, &title, true, waits_now()))
            .map_err(|e| format!("Suno's window would not take the words: {e}"))?;
        return Ok(Filling {
            language,
            waits: waits_now(),
        });
    }
    let url = "https://suno.com/create"
        .parse()
        .map_err(|e| format!("Suno's address: {e}"))?;
    let window = tauri::WebviewWindowBuilder::new(&app, SUNO, tauri::WebviewUrl::External(url))
        .title("Suno — check the words and the style, then press Create")
        .inner_size(1200.0, 860.0)
        .min_inner_size(480.0, 360.0)
        .resizable(true)
        .on_page_load(|window, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }
            let Ok(mut held) = PENDING.lock() else { return };
            let now = Instant::now();
            match held.as_ref() {
                Some(pending) if pending.due(payload.url().as_str(), now) => {
                    let _ = window.eval(pending.script.clone());
                }
                Some(pending) if now >= pending.until => *held = None,
                _ => {}
            }
        })
        .build()
        .map_err(|e| format!("Suno's window would not open: {e}"))?;
    WAITING_WINDOW.store(true, Ordering::SeqCst);
    window.on_window_event(|event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            WAITING_WINDOW.store(false, Ordering::SeqCst);
        }
    });
    Ok(Filling {
        language,
        waits: true,
    })
}

fn waits_now() -> bool {
    WAITING_WINDOW.load(Ordering::SeqCst)
}

/// The song's link, read from the address of djmanzo's Suno window when it
/// shows a song; none while it shows anything else, or is not open.
#[tauri::command]
#[must_use]
pub fn suno_link(app: tauri::AppHandle) -> Option<String> {
    let window = app.get_webview_window(SUNO)?;
    song_link(window.url().ok()?.as_str())
}

/// Open WhatsApp Web in djmanzo's own window, at the chat with a guest and
/// the message with their song's `link` written, for the DJ to read and
/// send. Answers the message, to show what was written.
///
/// # Errors
/// A guest not in the journal, who has not agreed to be contacted, or whose
/// number WhatsApp cannot dial; or a window that will not open.
#[tauri::command]
pub fn guests_whatsapp_here(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    link: String,
) -> Result<String, String> {
    let journal = state.guests();
    let guest = journal
        .get(&id)
        .ok_or_else(|| crate::guests::Refusal::Unknown.to_string())?;
    let text = crate::guests::message(guest, &link);
    let url: tauri::Url = crate::guests::whatsapp_web(guest, &text)?
        .parse()
        .map_err(|e| format!("WhatsApp's address: {e}"))?;
    if let Some(window) = app.get_webview_window(WHATSAPP) {
        let _ = window.show();
        let _ = window.set_focus();
        window
            .navigate(url)
            .map_err(|e| format!("WhatsApp's window would not open the chat: {e}"))?;
        return Ok(text);
    }
    tauri::WebviewWindowBuilder::new(&app, WHATSAPP, tauri::WebviewUrl::External(url))
        .title("WhatsApp — read the message, then press send")
        .inner_size(1000.0, 800.0)
        .min_inner_size(420.0, 360.0)
        .resizable(true)
        .build()
        .map_err(|e| format!("WhatsApp's window would not open: {e}"))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A song's address is its link; nothing else on Suno, or elsewhere,
    /// is.**
    #[test]
    fn only_a_song_on_suno_is_a_link() {
        assert_eq!(
            song_link("https://suno.com/song/1a2b-3c4d?sh=x#top").as_deref(),
            Some("https://suno.com/song/1a2b-3c4d")
        );
        assert_eq!(
            song_link("https://www.SUNO.com/s/AbC123/").as_deref(),
            Some("https://suno.com/s/AbC123")
        );
        for not in [
            "https://suno.com/create",
            "https://suno.com/me",
            "https://suno.com/song/",
            "https://suno.com/song/a/b",
            "https://suno.com/song/<script>",
            "https://suno.com.evil.example/song/abc",
            "https://evil.example/https://suno.com/song/abc",
            "file:///song/abc",
            "",
        ] {
            assert_eq!(song_link(not), None, "{not}");
        }
    }

    /// **The words go into the script as data**: quotes, a closing tag and a
    /// line break in them do not end it or run.
    #[test]
    fn the_words_go_in_as_data() {
        let script = fill_script(
            "She said \"go\"\n</script><b>",
            "bachata, 120 bpm",
            "A song for Ana",
            false,
            true,
        );
        let (data, rest) = script.split_once(";\n").expect("data first");
        let json = data
            .strip_prefix("window.__djmanzoSuno = ")
            .expect("assigned");
        let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
        assert_eq!(parsed["lyrics"], "She said \"go\"\n</script><b>");
        assert_eq!(parsed["style"], "bachata, 120 bpm");
        assert_eq!(parsed["overwrite"], false);
        assert_eq!(parsed["waits"], true);
        assert_eq!(rest, FILL);
        assert!(
            !FILL.contains("Create\").click") && !FILL.contains(".submit("),
            "never presses Create"
        );
    }

    /// **A fill waits for Suno's form on suno.com's pages, and not for
    /// ever** — through the sign-in a DJ who is not signed in meets first.
    #[test]
    fn a_fill_waits_for_the_form_on_suno_and_not_for_ever() {
        let now = Instant::now();
        let pending = Pending {
            script: String::new(),
            until: now + WAITS,
        };
        assert!(pending.due("https://suno.com/create", now));
        assert!(pending.due("https://www.suno.com/sign-in?next=/create", now));
        assert!(
            !pending.due("https://accounts.google.com/o/oauth2", now),
            "not another site"
        );
        assert!(!pending.due("https://suno.com.evil.example/create", now));
        assert!(
            !pending.due("https://suno.com/create", now + WAITS),
            "not for ever"
        );
    }

    #[test]
    fn the_words_are_in_the_language_asked_else_the_first() {
        let mut ana = crate::guests::Guest {
            name: "Ana".into(),
            sang: vec![crate::guests::Performance::default()],
            ..Default::default()
        };
        assert!(words_for(&ana, "Spanish").is_err(), "nothing written yet");
        ana.sang[0].song = Some(dj_assistant::SongDraft {
            style: "bachata".into(),
            versions: vec![
                dj_assistant::Version {
                    language: "Spanish".into(),
                    lyrics: "Rosa".into(),
                },
                dj_assistant::Version {
                    language: "Catalan".into(),
                    lyrics: "Rosa, en català".into(),
                },
            ],
        });
        assert_eq!(
            words_for(&ana, "Catalan").unwrap(),
            ("Rosa, en català".into(), "bachata".into(), "Catalan".into())
        );
        assert_eq!(words_for(&ana, "Basque").unwrap().2, "Spanish");
    }
}

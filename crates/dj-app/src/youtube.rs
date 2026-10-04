//! §119: YouTube's live chat, read into the crowd as it arrives.
//!
//! The owner's decision (27 September): YouTube through a verified djmanzo
//! Google app. The DJ signs in with Google in their own browser — the
//! loopback flow Google prescribes for desktop apps, with PKCE — and djmanzo
//! reads the live chat of the DJ's own broadcast: `liveBroadcasts.list` for
//! the one that is live, then `liveChatMessages.list` at the interval
//! YouTube's own answer asks for. Each message becomes a reaction of the
//! night ([`crate::crowd::Reaction`], `youtube`), placed on the music like
//! every other.
//!
//! **What djmanzo holds of the owner's** is a client id (and, for a Desktop
//! app client, its secret, which Google says such an app cannot keep and so
//! is not one), as a setting in `youtube.json`. Until one is set, nothing
//! here does anything and the settings say what to register. What djmanzo
//! holds of the DJ's is the refresh token Google gives back, kept in the
//! system keychain ([`dj_secrets::SecretKind::YouTubeSignIn`]); the access
//! token lives in memory for the hour Google grants it.
//!
//! The scope is `youtube.readonly`: djmanzo reads, and cannot post, delete
//! or moderate anything. One thread does the reading, never near the audio,
//! and talks to Google through [`Google`], so what it does with each of
//! Google's answers is tested without Google.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use dj_secrets::{Secret, SecretKind, SecretStore};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Read only: the live chat, the broadcasts. Nothing that writes.
pub const SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";

/// Where djmanzo reaches Google.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    /// The YouTube Data API's root, without a trailing slash.
    pub api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth: "https://accounts.google.com/o/oauth2/v2/auth".to_owned(),
            token: "https://oauth2.googleapis.com/token".to_owned(),
            api: "https://www.googleapis.com/youtube/v3".to_owned(),
        }
    }
}

/// Points every address at a stand-in Google while developing: the sign-in
/// at `<base>/auth`, the token at `<base>/token`, the API under `<base>/api`.
pub const STAND_IN_ENV: &str = "DJMANZO_GOOGLE";

impl Endpoints {
    /// Google's, or a stand-in's when [`STAND_IN_ENV`] names one.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(STAND_IN_ENV) {
            Ok(base) if !base.trim().is_empty() => {
                let base = base.trim().trim_end_matches('/');
                Self {
                    auth: format!("{base}/auth"),
                    token: format!("{base}/token"),
                    api: format!("{base}/api"),
                }
            }
            _ => Self::default(),
        }
    }
}

/// The DJ's settings, kept in `youtube.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Read the chat while a broadcast is live.
    pub on: bool,
    /// The Google Cloud OAuth client — a Desktop app — djmanzo signs in as.
    pub client_id: String,
    /// That client's secret, when Google's token address asks for one. Not
    /// a secret in Google's own words: a desktop app cannot keep one.
    pub client_secret: String,
}

impl Settings {
    fn ready(&self) -> bool {
        !self.client_id.trim().is_empty()
    }
}

// -- signing in -----------------------------------------------------------

/// PKCE's pair: the verifier djmanzo keeps, and the challenge Google is
/// shown first (RFC 7636, `S256`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// The pair, from 32 random bytes: a 43-character verifier and the
/// unpadded base64url of its SHA-256.
#[must_use]
pub fn pkce(random: &[u8; 32]) -> Pkce {
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Pkce {
        verifier,
        challenge,
    }
}

/// Thirty-two bytes from the operating system's own source.
///
/// # Errors
/// When the system will not give any.
pub fn random() -> Result<[u8; 32], String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no randomness from the system: {e}"))?;
    Ok(bytes)
}

/// Where the DJ's browser is sent to sign in.
#[must_use]
pub fn sign_in_address(
    endpoints: &Endpoints,
    client_id: &str,
    redirect: &str,
    challenge: &str,
    state: &str,
) -> String {
    let query = [
        ("client_id", client_id.trim()),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        // Asked every time, so a second sign-in still brings a refresh token.
        ("prompt", "consent"),
    ]
    .iter()
    .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
    .collect::<Vec<_>>()
    .join("&");
    format!("{}?{query}", endpoints.auth)
}

/// What the browser brought back to djmanzo's loopback address — the first
/// line of its request — read for the code, if the state is the one djmanzo
/// sent. `Ok(None)` for a request that is not the sign-in's at all (a
/// browser asking for an icon).
///
/// # Errors
/// The DJ declining, Google refusing, or a state that is not djmanzo's.
pub fn callback(request_line: &str, state: &str) -> Result<Option<String>, String> {
    let target = request_line.split_whitespace().nth(1).unwrap_or("");
    let Some(query) = target.strip_prefix("/?") else {
        return Ok(None);
    };
    let field = |name: &str| {
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name)
                .then(|| urlencoding::decode(v).map_or_else(|_| v.to_owned(), |v| v.into_owned()))
        })
    };
    if field("state").as_deref() != Some(state) {
        return Err("that answer was not to the sign-in djmanzo started".to_owned());
    }
    if let Some(error) = field("error") {
        return Err(if error == "access_denied" {
            "the sign-in was declined".to_owned()
        } else {
            format!("Google said {error}")
        });
    }
    field("code")
        .filter(|c| !c.is_empty())
        .map(Some)
        .ok_or_else(|| "Google sent no code".to_owned())
}

/// The form that trades the code for tokens.
#[must_use]
pub fn code_form(
    settings: &Settings,
    code: &str,
    verifier: &str,
    redirect: &str,
) -> Vec<(&'static str, String)> {
    let mut form = vec![
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("code_verifier", verifier.to_owned()),
        ("redirect_uri", redirect.to_owned()),
        ("client_id", settings.client_id.trim().to_owned()),
    ];
    if !settings.client_secret.trim().is_empty() {
        form.push(("client_secret", settings.client_secret.trim().to_owned()));
    }
    form
}

/// The form that trades the refresh token for an hour's access.
#[must_use]
pub fn refresh_form(settings: &Settings, refresh: &str) -> Vec<(&'static str, String)> {
    let mut form = vec![
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", refresh.to_owned()),
        ("client_id", settings.client_id.trim().to_owned()),
    ];
    if !settings.client_secret.trim().is_empty() {
        form.push(("client_secret", settings.client_secret.trim().to_owned()));
    }
    form
}

/// What Google's token address gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access: String,
    /// Seconds the access token is good for.
    pub expires_in: u64,
    /// Given on signing in; not on a refresh.
    pub refresh: Option<String>,
}

/// Why the token address refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The sign-in is no longer good — revoked, or expired: sign in again.
    SignInAgain(String),
    /// Anything else, in Google's words.
    Other(String),
}

/// Read the token address's answer.
///
/// # Errors
/// Google's refusal, `invalid_grant` told apart because its remedy is the
/// DJ's: signing in again.
pub fn tokens(answer: &Value) -> Result<Tokens, Refused> {
    if let Some(error) = answer.get("error").and_then(Value::as_str) {
        let said = answer
            .get("error_description")
            .and_then(Value::as_str)
            .map_or_else(|| error.to_owned(), |d| format!("{error}: {d}"));
        return Err(if error == "invalid_grant" {
            Refused::SignInAgain(said)
        } else {
            Refused::Other(said)
        });
    }
    let access = answer
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| Refused::Other("Google sent no access token".to_owned()))?;
    Ok(Tokens {
        access: access.to_owned(),
        expires_in: answer
            .get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(3_600),
        refresh: answer
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

// -- reading --------------------------------------------------------------

/// The DJ's broadcast that is live, and its chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broadcast {
    pub title: String,
    pub chat: String,
}

/// The first live broadcast with a chat, from `liveBroadcasts.list`.
#[must_use]
pub fn active(answer: &Value) -> Option<Broadcast> {
    answer.get("items")?.as_array()?.iter().find_map(|item| {
        let snippet = item.get("snippet")?;
        let chat = snippet.get("liveChatId")?.as_str()?.to_owned();
        let title = snippet
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        (!chat.is_empty()).then_some(Broadcast { title, chat })
    })
}

/// One message of the chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    /// Unix seconds.
    pub at: i64,
    pub who: String,
    pub text: String,
}

/// One answer of `liveChatMessages.list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub messages: Vec<Message>,
    pub next: Option<String>,
    /// How long YouTube asks to be left before the next.
    pub wait: Duration,
    /// The chat has ended.
    pub ended: bool,
}

/// What people said, read out of a page: text, Super Chats and Super
/// Stickers, each with the words YouTube shows; a chat that ended says so;
/// everything else — a new member, a poll, a ban, a deleted message — is not
/// somebody reacting to the music, and is left.
#[must_use]
pub fn page(answer: &Value) -> Page {
    let mut messages = Vec::new();
    let mut ended = false;
    for item in answer
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let snippet = item.get("snippet");
        let kind = snippet
            .and_then(|s| s.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        match kind {
            "chatEndedEvent" => ended = true,
            "textMessageEvent" | "superChatEvent" | "superStickerEvent" => {
                let text = snippet
                    .and_then(|s| s.get("displayMessage"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned();
                let at = snippet
                    .and_then(|s| s.get("publishedAt"))
                    .and_then(Value::as_str)
                    .and_then(unix_seconds);
                let (Some(at), false) = (at, text.is_empty()) else {
                    continue;
                };
                messages.push(Message {
                    id: item
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    at,
                    who: item
                        .get("authorDetails")
                        .and_then(|a| a.get("displayName"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    text,
                });
            }
            _ => {}
        }
    }
    Page {
        messages,
        next: answer
            .get("nextPageToken")
            .and_then(Value::as_str)
            .map(str::to_owned),
        wait: Duration::from_millis(
            answer
                .get("pollingIntervalMillis")
                .and_then(Value::as_u64)
                .unwrap_or(5_000),
        ),
        ended,
    }
}

/// The reason in a YouTube API error, as its `errors[].reason` says it:
/// `quotaExceeded`, `liveChatEnded`, `liveChatNotFound`, ….
#[must_use]
pub fn reason(answer: &Value) -> String {
    answer
        .pointer("/error/errors/0/reason")
        .and_then(Value::as_str)
        .or_else(|| answer.pointer("/error/message").and_then(Value::as_str))
        .unwrap_or("")
        .to_owned()
}

/// RFC 3339 — `2026-10-04T21:43:08.215Z`, or with an offset — as unix
/// seconds. `None` for anything else.
#[must_use]
pub fn unix_seconds(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let mut ymd = date.splitn(3, '-').map(str::parse::<i64>);
    let (y, m, d) = (ymd.next()?.ok()?, ymd.next()?.ok()?, ymd.next()?.ok()?);
    let (clock, offset) = if let Some(clock) = rest.strip_suffix('Z') {
        (clock, 0)
    } else {
        let at = rest.rfind(['+', '-'])?;
        let (clock, zone) = rest.split_at(at);
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        let (h, m) = zone[1..].split_once(':')?;
        (
            clock,
            sign * (h.parse::<i64>().ok()? * 3_600 + m.parse::<i64>().ok()? * 60),
        )
    };
    let mut hms = clock.splitn(3, ':');
    let h = hms.next()?.parse::<i64>().ok()?;
    let mi = hms.next()?.parse::<i64>().ok()?;
    let s = hms.next()?.split('.').next()?.parse::<i64>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3_600 + mi * 60 + s - offset)
}

/// Google, as the reading thread needs it. [`Http`] is the real one; the
/// tests' is scripted.
pub trait Google: Send + Sync {
    /// Post a form to the token address; Google's answer, refusals included.
    ///
    /// # Errors
    /// Google not reached.
    fn token(&self, form: &[(&'static str, String)]) -> Result<Value, String>;
    /// Get `path` under the API's root with the access token: the status and
    /// the answer.
    ///
    /// # Errors
    /// Google not reached.
    fn get(&self, path: &str, access: &str) -> Result<(u16, Value), String>;
}

/// Google over HTTPS.
#[derive(Debug, Clone)]
pub struct Http {
    pub endpoints: Endpoints,
    client: reqwest::Client,
}

impl Http {
    #[must_use]
    pub fn new(endpoints: Endpoints) -> Self {
        Self {
            endpoints,
            client: reqwest::Client::new(),
        }
    }

    /// Post a form to the token address, from async code.
    ///
    /// # Errors
    /// Google not reached, or an answer that is not JSON.
    pub async fn token_async(&self, form: &[(&'static str, String)]) -> Result<Value, String> {
        self.client
            .post(&self.endpoints.token)
            .form(form)
            .send()
            .await
            .map_err(|e| format!("Google not reached: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("Google's answer was not readable: {e}"))
    }
}

impl Google for Http {
    fn token(&self, form: &[(&'static str, String)]) -> Result<Value, String> {
        tauri::async_runtime::block_on(self.token_async(form))
    }

    fn get(&self, path: &str, access: &str) -> Result<(u16, Value), String> {
        tauri::async_runtime::block_on(async {
            let answer = self
                .client
                .get(format!("{}{path}", self.endpoints.api))
                .bearer_auth(access)
                .send()
                .await
                .map_err(|e| format!("YouTube not reached: {e}"))?;
            let status = answer.status().as_u16();
            let body = answer.json::<Value>().await.unwrap_or(Value::Null);
            Ok((status, body))
        })
    }
}

/// Where the chat's messages go: the night's reactions, less any already
/// kept. Returns how many were new.
pub type Sink = Arc<dyn Fn(Vec<crate::crowd::Reaction>) -> usize + Send + Sync>;

/// How long the reading waits, between what.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Switched off, or not signed in: how often it looks again.
    pub idle: Duration,
    /// No broadcast live: how long until it asks again.
    pub look: Duration,
    /// Google not reached, or refusing: how long until it tries again.
    pub retry: Duration,
    /// The app's daily quota spent: how long it rests.
    pub quota: Duration,
    /// The shortest wait between pages, whatever YouTube says. Each page
    /// costs a unit of the app's daily quota, and a message is placed by the
    /// time YouTube stamped on it, not by when it was fetched — so waiting
    /// longer costs the placing nothing and spares the quota.
    pub least: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(2),
            look: Duration::from_secs(30),
            retry: Duration::from_secs(10),
            quota: Duration::from_secs(600),
            least: Duration::from_secs(10),
        }
    }
}

/// What the reading is doing, for the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub on: bool,
    /// A client id is set.
    pub client: bool,
    pub signed_in: bool,
    /// The broadcast whose chat is being read.
    pub reading: Option<String>,
    /// Messages brought into the crowd since djmanzo started.
    pub read: usize,
    /// What it is waiting for, or what went wrong, in words.
    pub note: Option<String>,
}

struct Shared {
    settings: Mutex<Settings>,
    status: Mutex<Status>,
    dir: Mutex<Option<PathBuf>>,
    secrets: Mutex<Option<Arc<dyn SecretStore>>>,
    /// Woken when the DJ changes something, so the thread does not sleep
    /// through it.
    poke: (Mutex<u64>, Condvar),
}

/// The reading of YouTube's live chat.
#[derive(Clone)]
pub struct Link {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("status", &self.status())
            .finish()
    }
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

impl Link {
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                settings: Mutex::new(Settings::default()),
                status: Mutex::new(Status::default()),
                dir: Mutex::new(None),
                secrets: Mutex::new(None),
                poke: (Mutex::new(0), Condvar::new()),
            }),
        }
    }

    /// Read the kept settings from `dir`, and start the thread.
    pub fn start(
        &self,
        dir: Option<&Path>,
        secrets: Option<Arc<dyn SecretStore>>,
        google: Arc<dyn Google>,
        sink: Sink,
        timing: Timing,
    ) {
        if let Some(dir) = dir {
            let kept = std::fs::read_to_string(dir.join("youtube.json"))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
            if let Ok(mut settings) = self.shared.settings.lock() {
                *settings = kept;
            }
            if let Ok(mut slot) = self.shared.dir.lock() {
                *slot = Some(dir.to_path_buf());
            }
        }
        if let Ok(mut slot) = self.shared.secrets.lock() {
            *slot = secrets;
        }
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new()
            .name("dj-youtube-chat".into())
            .spawn(move || run(&shared, google.as_ref(), &sink, timing));
        if let Err(error) = spawned {
            tracing::warn!(%error, "YouTube's live chat will not be read");
        }
    }

    #[must_use]
    pub fn settings(&self) -> Settings {
        self.shared
            .settings
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Keep the DJ's settings, and wake the reading.
    pub fn set(&self, next: Settings) {
        if let Ok(dir) = self.shared.dir.lock()
            && let Some(dir) = dir.as_ref()
            && let Ok(text) = serde_json::to_string_pretty(&next)
            && let Err(error) = std::fs::write(dir.join("youtube.json"), text)
        {
            tracing::warn!(%error, "youtube.json was not written");
        }
        if let Ok(mut settings) = self.shared.settings.lock() {
            *settings = next;
        }
        self.poke();
    }

    #[must_use]
    pub fn status(&self) -> Status {
        let mut status = self
            .shared
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let settings = self.settings();
        status.on = settings.on;
        status.client = settings.ready();
        status.signed_in = self.shared.refresh().is_some();
        status
    }

    /// Keep what signing in gave, and wake the reading.
    ///
    /// # Errors
    /// No keychain to keep it in.
    pub fn signed_in(&self, refresh: &str) -> Result<(), String> {
        let store = self
            .shared
            .secrets
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .ok_or_else(|| "no keychain to keep the sign-in in".to_owned())?;
        store
            .set(SecretKind::YouTubeSignIn, &Secret::new(refresh.to_owned()))
            .map_err(|e| e.to_string())?;
        self.poke();
        Ok(())
    }

    /// Forget the sign-in.
    pub fn sign_out(&self) {
        self.shared.forget();
        self.poke();
    }

    fn poke(&self) {
        let (lock, wake) = &self.shared.poke;
        if let Ok(mut n) = lock.lock() {
            *n += 1;
        }
        wake.notify_all();
    }
}

impl Shared {
    fn refresh(&self) -> Option<String> {
        let store = self.secrets.lock().ok()?.clone()?;
        store
            .get(SecretKind::YouTubeSignIn)
            .ok()
            .map(|s| s.expose().to_owned())
            .filter(|s| !s.is_empty())
    }

    fn forget(&self) {
        if let Some(store) = self.secrets.lock().ok().and_then(|s| s.clone())
            && let Err(error) = store.delete(SecretKind::YouTubeSignIn)
        {
            tracing::warn!(%error, "the YouTube sign-in was not forgotten");
        }
    }

    fn note(&self, reading: Option<String>, note: Option<String>) {
        if let Ok(mut status) = self.status.lock() {
            status.reading = reading;
            status.note = note;
        }
    }

    /// Sleep for `wait`, or until the DJ changes something.
    fn rest(&self, wait: Duration) {
        let (lock, wake) = &self.poke;
        let Ok(seen) = lock.lock() else {
            std::thread::sleep(wait);
            return;
        };
        let was = *seen;
        let _ = wake.wait_timeout_while(seen, wait, |n| *n == was);
    }
}

fn run(shared: &Shared, google: &dyn Google, sink: &Sink, timing: Timing) {
    // The access token, until when, and the sign-in it came from: a new
    // sign-in is used at once rather than after the old hour runs out.
    let mut access: Option<(String, Instant)> = None;
    let mut access_from = String::new();
    let mut chat: Option<Broadcast> = None;
    let mut next_page: Option<String> = None;
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        let settings = shared
            .settings
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let refresh = shared.refresh();
        let Some(refresh) = refresh.filter(|_| settings.on && settings.ready()) else {
            access = None;
            chat = None;
            next_page = None;
            if let Ok(mut status) = shared.status.lock() {
                status.reading = None;
                // Why the sign-in went stays said until the DJ signs in
                // again or switches the reading off.
                if !settings.on {
                    status.note = None;
                }
            }
            shared.rest(timing.idle);
            continue;
        };

        // An hour's access, from the sign-in.
        if access_from != refresh {
            access = None;
            chat = None;
            next_page = None;
            access_from.clone_from(&refresh);
        }
        if access
            .as_ref()
            .is_none_or(|(_, until)| Instant::now() >= *until)
        {
            let answer = google.token(&refresh_form(&settings, &refresh));
            match answer.map_err(Refused::Other).and_then(|a| tokens(&a)) {
                Ok(tokens) => {
                    let good = Duration::from_secs(tokens.expires_in.saturating_sub(60).max(60));
                    access = Some((tokens.access, Instant::now() + good));
                }
                Err(Refused::SignInAgain(said)) => {
                    shared.forget();
                    shared.note(
                        None,
                        Some(format!(
                            "Google no longer accepts djmanzo's sign-in ({said}) — sign in again."
                        )),
                    );
                    continue;
                }
                Err(Refused::Other(said)) => {
                    shared.note(None, Some(said));
                    shared.rest(timing.retry);
                    continue;
                }
            }
        }
        let Some((token, _)) = access.clone() else {
            continue;
        };

        // The broadcast that is live.
        let Some(live) = chat.clone() else {
            match google.get(
                "/liveBroadcasts?part=snippet&broadcastStatus=active&broadcastType=all",
                &token,
            ) {
                Ok((200, answer)) => {
                    if let Some(found) = active(&answer) {
                        shared.note(Some(found.title.clone()), None);
                        chat = Some(found);
                        next_page = None;
                    } else {
                        shared.note(
                            None,
                            Some("No live broadcast on your channel right now — djmanzo looks again every half minute.".to_owned()),
                        );
                        shared.rest(timing.look);
                    }
                }
                Ok((401, _)) => access = None,
                Ok((_, answer)) => refused(shared, &answer, timing),
                Err(said) => {
                    shared.note(None, Some(said));
                    shared.rest(timing.retry);
                }
            }
            continue;
        };

        // The chat, a page at a time, at YouTube's pace.
        let path = format!(
            "/liveChat/messages?liveChatId={}&part=snippet,authorDetails&maxResults=2000{}",
            urlencoding::encode(&live.chat),
            next_page
                .as_deref()
                .map(|t| format!("&pageToken={}", urlencoding::encode(t)))
                .unwrap_or_default()
        );
        match google.get(&path, &token) {
            Ok((200, answer)) => {
                let got = page(&answer);
                let fresh: Vec<crate::crowd::Reaction> = got
                    .messages
                    .into_iter()
                    .filter(|m| m.id.is_empty() || seen.insert(m.id.clone()))
                    .map(|m| crate::crowd::Reaction {
                        at: m.at,
                        source: crate::crowd::Source::YouTube,
                        who: m.who,
                        text: m.text,
                    })
                    .collect();
                if !fresh.is_empty() {
                    let new = sink(fresh);
                    if let Ok(mut status) = shared.status.lock() {
                        status.read += new;
                    }
                }
                next_page = got.next;
                if got.ended {
                    chat = None;
                    shared.note(
                        None,
                        Some(format!("The chat of “{}” has ended.", live.title)),
                    );
                    shared.rest(timing.look);
                } else {
                    shared.note(Some(live.title.clone()), None);
                    shared.rest(got.wait.max(timing.least));
                }
            }
            Ok((401, _)) => access = None,
            Ok((403 | 404, answer))
                if matches!(
                    reason(&answer).as_str(),
                    "liveChatEnded" | "liveChatNotFound" | "liveChatDisabled"
                ) =>
            {
                chat = None;
                shared.note(None, Some(format!("The chat of “{}” is over.", live.title)));
                shared.rest(timing.look);
            }
            Ok((_, answer)) => refused(shared, &answer, timing),
            Err(said) => {
                shared.note(Some(live.title.clone()), Some(said));
                shared.rest(timing.retry);
            }
        }
    }
}

/// YouTube said no: the quota rests longest, anything else is retried.
fn refused(shared: &Shared, answer: &Value, timing: Timing) {
    let why = reason(answer);
    if why == "quotaExceeded" || why == "rateLimitExceeded" {
        shared.note(
            None,
            Some(
                "YouTube's daily quota for this app is spent — djmanzo tries again in ten minutes."
                    .to_owned(),
            ),
        );
        shared.rest(timing.quota);
    } else {
        shared.note(
            None,
            Some(if why.is_empty() {
                "YouTube refused, without saying why.".to_owned()
            } else {
                format!("YouTube refused: {why}")
            }),
        );
        shared.rest(timing.retry);
    }
}

/// Wait for the browser to come back to djmanzo's loopback address, at most
/// `within`, answer it with a page that says what happened, and give the
/// code. A request that is not the sign-in's (an icon) is answered and the
/// wait goes on.
///
/// # Errors
/// The time running out, the DJ declining, or Google refusing.
pub fn await_callback(
    listener: &std::net::TcpListener,
    state: &str,
    within: Duration,
) -> Result<String, String> {
    use std::io::{BufRead as _, BufReader, Write as _};
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let until = Instant::now() + within;
    loop {
        if Instant::now() >= until {
            return Err("the sign-in was not finished in time".to_owned());
        }
        let (stream, _) = match listener.accept() {
            Ok(accepted) => accepted,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        stream.set_nonblocking(false).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        let mut first = String::new();
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut first).is_err() {
            continue;
        }
        // The rest of the request, unread, is dropped with the connection.
        let outcome = callback(&first, state);
        let (status, said) = match &outcome {
            Ok(Some(_)) => (
                "200 OK",
                "djmanzo is signed in to YouTube. You can close this tab.",
            ),
            Ok(None) => ("404 Not Found", ""),
            Err(why) => ("400 Bad Request", why.as_str()),
        };
        let body = format!(
            "<!doctype html><meta charset=utf-8><title>djmanzo</title><p style=\"font:16px sans-serif\">{}</p>",
            said.replace('&', "&amp;").replace('<', "&lt;")
        );
        let mut out = &stream;
        let _ = write!(
            out,
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        match outcome {
            Ok(Some(code)) => return Ok(code),
            Ok(None) => {}
            Err(why) => return Err(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_secrets::MemoryStore;
    use serde_json::json;
    use std::collections::VecDeque;

    /// **PKCE is RFC 7636's**: the specification's own example verifier
    /// gives its example challenge.
    #[test]
    fn the_challenge_is_the_rfcs() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        let pair = pkce(&[7; 32]);
        assert_eq!(pair.verifier.len(), 43);
        assert_eq!(
            pair.challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(pair.verifier.as_bytes()))
        );
        assert_ne!(random().expect("random"), random().expect("random"));
    }

    /// **The sign-in asks for reading only**, with PKCE, the state, and the
    /// loopback address, each encoded.
    #[test]
    fn the_sign_in_asks_for_reading_only() {
        let address = sign_in_address(
            &Endpoints::default(),
            " 123.apps.googleusercontent.com ",
            "http://127.0.0.1:5000",
            "abc",
            "s t",
        );
        assert!(address.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        for part in [
            "client_id=123.apps.googleusercontent.com",
            "redirect_uri=http%3A%2F%2F127.0.0.1%3A5000",
            "response_type=code",
            "scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fyoutube.readonly",
            "code_challenge=abc",
            "code_challenge_method=S256",
            "state=s%20t",
            "access_type=offline",
        ] {
            assert!(address.contains(part), "{part} missing from {address}");
        }
    }

    /// **What the browser brings back is read strictly**: the code when the
    /// state is djmanzo's; a declined sign-in, Google's other refusals and
    /// a stranger's state each refused; an icon request is not the sign-in.
    #[test]
    fn the_browser_brings_back_a_code_or_a_reason() {
        assert_eq!(
            callback("GET /?state=xyz&code=4%2F0Ab&scope=x HTTP/1.1", "xyz"),
            Ok(Some("4/0Ab".to_owned()))
        );
        assert_eq!(
            callback("GET /?error=access_denied&state=xyz HTTP/1.1", "xyz"),
            Err("the sign-in was declined".to_owned())
        );
        assert!(callback("GET /?error=invalid_scope&state=xyz HTTP/1.1", "xyz").is_err());
        assert!(
            callback("GET /?state=other&code=1 HTTP/1.1", "xyz").is_err(),
            "a code with somebody else's state"
        );
        assert_eq!(callback("GET /favicon.ico HTTP/1.1", "xyz"), Ok(None));
    }

    /// **The forms**: the code with its verifier and the loopback address;
    /// the refresh; the client's secret sent only when there is one.
    #[test]
    fn the_forms_carry_the_verifier_and_the_secret_only_when_set() {
        let mut settings = Settings {
            on: true,
            client_id: "id".to_owned(),
            client_secret: String::new(),
        };
        let form = code_form(&settings, "c", "v", "http://127.0.0.1:1");
        assert!(form.contains(&("code_verifier", "v".to_owned())));
        assert!(form.contains(&("grant_type", "authorization_code".to_owned())));
        assert!(!form.iter().any(|(k, _)| *k == "client_secret"));
        settings.client_secret = "GOCSPX-1".to_owned();
        let form = refresh_form(&settings, "r");
        assert!(form.contains(&("refresh_token", "r".to_owned())));
        assert!(form.contains(&("client_secret", "GOCSPX-1".to_owned())));
    }

    /// **Google's token answers**: tokens read; `invalid_grant` told apart
    /// from any other refusal, because its remedy is signing in again.
    #[test]
    fn a_lapsed_sign_in_is_told_apart() {
        assert_eq!(
            tokens(&json!({"access_token": "a", "expires_in": 3599, "refresh_token": "r"})),
            Ok(Tokens {
                access: "a".to_owned(),
                expires_in: 3599,
                refresh: Some("r".to_owned())
            })
        );
        assert!(matches!(
            tokens(
                &json!({"error": "invalid_grant", "error_description": "Token has been expired or revoked."})
            ),
            Err(Refused::SignInAgain(_))
        ));
        assert!(matches!(
            tokens(&json!({"error": "invalid_client"})),
            Err(Refused::Other(_))
        ));
    }

    /// **A page of chat is what people said**: text, Super Chats and
    /// stickers kept with who and when; a new member, a deleted message and
    /// an empty one left; the end of the chat noticed; YouTube's interval
    /// and next page kept.
    #[test]
    fn a_page_is_what_people_said() {
        let answer = json!({
            "pollingIntervalMillis": 6000,
            "nextPageToken": "GO",
            "items": [
                {"id": "1", "snippet": {"type": "textMessageEvent", "publishedAt": "2026-10-04T21:43:08.215Z", "displayMessage": "this drop 🔥"}, "authorDetails": {"displayName": "Ana"}},
                {"id": "2", "snippet": {"type": "superChatEvent", "publishedAt": "2026-10-04T21:43:10+00:00", "displayMessage": "$5.00 from Ben: play Despacito"}, "authorDetails": {"displayName": "Ben"}},
                {"id": "3", "snippet": {"type": "newSponsorEvent", "publishedAt": "2026-10-04T21:43:11Z", "displayMessage": "Cleo is a new member"}},
                {"id": "4", "snippet": {"type": "tombstone", "publishedAt": "2026-10-04T21:43:12Z"}},
                {"id": "5", "snippet": {"type": "textMessageEvent", "publishedAt": "2026-10-04T21:43:13Z", "displayMessage": "  "}},
                {"id": "6", "snippet": {"type": "chatEndedEvent", "publishedAt": "2026-10-04T23:00:00Z"}}
            ]
        });
        let got = page(&answer);
        assert_eq!(
            got.messages,
            vec![
                Message {
                    id: "1".to_owned(),
                    at: 1_791_150_188,
                    who: "Ana".to_owned(),
                    text: "this drop 🔥".to_owned()
                },
                Message {
                    id: "2".to_owned(),
                    at: 1_791_150_190,
                    who: "Ben".to_owned(),
                    text: "$5.00 from Ben: play Despacito".to_owned()
                },
            ]
        );
        assert_eq!(got.next.as_deref(), Some("GO"));
        assert_eq!(got.wait, Duration::from_secs(6));
        assert!(got.ended);
    }

    /// **RFC 3339 as YouTube writes it**, offsets and fractions included,
    /// and anything else refused.
    #[test]
    fn youtubes_times_are_read() {
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            unix_seconds("2026-10-04T21:43:08.215Z"),
            Some(1_791_150_188)
        );
        assert_eq!(
            unix_seconds("2026-10-04T23:43:08+02:00"),
            Some(1_791_150_188)
        );
        assert_eq!(
            unix_seconds("2026-10-04T16:43:08.5-05:00"),
            Some(1_791_150_188)
        );
        assert_eq!(unix_seconds("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(unix_seconds("yesterday"), None);
        assert_eq!(unix_seconds("2026-13-04T21:43:08Z"), None);
    }

    /// **The live broadcast is the one with a chat.**
    #[test]
    fn the_live_broadcast_is_the_one_with_a_chat() {
        assert_eq!(
            active(&json!({"items": [
                {"snippet": {"title": "Rehearsal"}},
                {"snippet": {"title": "Saturday night", "liveChatId": "Cg0KC"}}
            ]})),
            Some(Broadcast {
                title: "Saturday night".to_owned(),
                chat: "Cg0KC".to_owned()
            })
        );
        assert_eq!(active(&json!({"items": []})), None);
    }

    /// A Google whose answers are written in advance, and which keeps what
    /// it was asked.
    #[derive(Default)]
    struct Scripted {
        tokens: Mutex<VecDeque<Value>>,
        pages: Mutex<VecDeque<(u16, Value)>>,
        asked: Mutex<Vec<String>>,
    }

    impl Google for Scripted {
        fn token(&self, form: &[(&'static str, String)]) -> Result<Value, String> {
            self.asked
                .lock()
                .expect("asked")
                .push(format!("token {}", form[0].1));
            Ok(self
                .tokens
                .lock()
                .expect("tokens")
                .pop_front()
                .unwrap_or_else(|| json!({"access_token": "a", "expires_in": 3600})))
        }

        fn get(&self, path: &str, access: &str) -> Result<(u16, Value), String> {
            self.asked
                .lock()
                .expect("asked")
                .push(format!("{access} {path}"));
            Ok(self
                .pages
                .lock()
                .expect("pages")
                .pop_front()
                .unwrap_or((200, json!({"items": [], "pollingIntervalMillis": 1}))))
        }
    }

    fn quick() -> Timing {
        Timing {
            idle: Duration::from_millis(5),
            look: Duration::from_millis(5),
            retry: Duration::from_millis(5),
            quota: Duration::from_millis(5),
            least: Duration::from_millis(1),
        }
    }

    fn text(id: &str, at: &str, who: &str, said: &str) -> Value {
        json!({"id": id, "snippet": {"type": "textMessageEvent", "publishedAt": at, "displayMessage": said}, "authorDetails": {"displayName": who}})
    }

    fn until(what: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// **Signed in and switched on, the chat of the live broadcast is read
    /// into the crowd**, page after page and each message once — the same
    /// message on a second page is not a second reaction; nothing is asked
    /// of Google while switched off or signed out; and a sign-in Google no
    /// longer accepts is forgotten, so the settings ask for a new one.
    #[test]
    fn the_live_chat_is_read_into_the_crowd() {
        let google = Arc::new(Scripted::default());
        google.pages.lock().expect("pages").extend([
            (
                200,
                json!({"items": [{"snippet": {"title": "Saturday night", "liveChatId": "C1"}}]}),
            ),
            (
                200,
                json!({"nextPageToken": "P2", "pollingIntervalMillis": 1, "items": [
                    text("1", "2026-10-04T21:43:08Z", "Ana", "this drop 🔥"),
                    text("2", "2026-10-04T21:43:09Z", "Ben", "hola")
                ]}),
            ),
            (
                200,
                json!({"nextPageToken": "P3", "pollingIntervalMillis": 1, "items": [
                    text("2", "2026-10-04T21:43:09Z", "Ben", "hola"),
                    text("3", "2026-10-04T21:43:20Z", "Cleo", "what song is this")
                ]}),
            ),
        ]);
        let kept: Arc<Mutex<Vec<crate::crowd::Reaction>>> = Arc::default();
        let sink: Sink = {
            let kept = Arc::clone(&kept);
            Arc::new(move |reactions| {
                let n = reactions.len();
                kept.lock().expect("kept").extend(reactions);
                n
            })
        };
        let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let dir = tempfile::tempdir().expect("a folder");
        let link = Link::new();
        link.start(
            Some(dir.path()),
            Some(Arc::clone(&secrets)),
            Arc::clone(&google) as Arc<dyn Google>,
            sink,
            quick(),
        );
        std::thread::sleep(Duration::from_millis(40));
        assert!(
            google.asked.lock().expect("asked").is_empty(),
            "Google was asked while switched off and signed out"
        );
        link.set(Settings {
            on: true,
            client_id: "id".to_owned(),
            client_secret: String::new(),
        });
        std::thread::sleep(Duration::from_millis(40));
        assert!(
            google.asked.lock().expect("asked").is_empty(),
            "asked before signing in"
        );

        link.signed_in("refresh-1").expect("kept");
        assert!(
            until(|| kept.lock().expect("kept").len() >= 3),
            "the chat was not read"
        );
        let said: Vec<(String, String)> = kept
            .lock()
            .expect("kept")
            .iter()
            .map(|r| (r.who.clone(), r.text.clone()))
            .collect();
        assert_eq!(
            said,
            [
                ("Ana".to_owned(), "this drop 🔥".to_owned()),
                ("Ben".to_owned(), "hola".to_owned()),
                ("Cleo".to_owned(), "what song is this".to_owned()),
            ]
        );
        assert!(
            kept.lock()
                .expect("kept")
                .iter()
                .all(|r| r.source == crate::crowd::Source::YouTube)
        );
        let asked = google.asked.lock().expect("asked").clone();
        assert_eq!(asked[0], "token refresh_token");
        assert!(asked[1].ends_with("broadcastStatus=active&broadcastType=all"));
        assert!(asked[2].contains("liveChatId=C1") && !asked[2].contains("pageToken"));
        assert!(asked[3].ends_with("&pageToken=P2"), "{}", asked[3]);
        assert!(until(|| link.status().read == 3));
        assert_eq!(link.status().reading.as_deref(), Some("Saturday night"));
        assert!(
            std::fs::read_to_string(dir.path().join("youtube.json"))
                .expect("kept")
                .contains("\"client_id\": \"id\"")
        );

        // Google no longer accepts the sign-in: it is forgotten, and said.
        google
            .tokens
            .lock()
            .expect("tokens")
            .push_back(json!({"error": "invalid_grant"}));
        link.sign_out();
        link.signed_in("refresh-2").expect("kept");
        assert!(
            until(|| !link.status().signed_in),
            "a lapsed sign-in was kept"
        );
        assert!(
            link.status()
                .note
                .is_some_and(|n| n.contains("sign in again"))
        );
    }

    /// **No broadcast live is said, and asked again later; the quota spent
    /// is said and rested; a chat that ends sends djmanzo looking for the
    /// next broadcast.**
    #[test]
    fn what_youtube_says_is_said() {
        let google = Arc::new(Scripted::default());
        google.pages.lock().expect("pages").extend([
            (200, json!({"items": []})),
            (
                403,
                json!({"error": {"errors": [{"reason": "quotaExceeded"}]}}),
            ),
        ]);
        let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let link = Link::new();
        let slow = Timing {
            look: Duration::from_millis(300),
            quota: Duration::from_millis(300),
            ..quick()
        };
        link.start(
            None,
            Some(secrets),
            Arc::clone(&google) as Arc<dyn Google>,
            Arc::new(|r: Vec<crate::crowd::Reaction>| r.len()),
            slow,
        );
        link.set(Settings {
            on: true,
            client_id: "id".to_owned(),
            client_secret: String::new(),
        });
        link.signed_in("r").expect("kept");
        assert!(until(|| link
            .status()
            .note
            .is_some_and(|n| n.contains("No live broadcast"))));
        assert!(until(|| link
            .status()
            .note
            .is_some_and(|n| n.contains("quota"))));
        let asked = google.asked.lock().expect("asked").len();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            google.asked.lock().expect("asked").len(),
            asked,
            "asked again while the quota rests"
        );

        // The chat ends: djmanzo looks for the next broadcast.
        let google = Arc::new(Scripted::default());
        google.pages.lock().expect("pages").extend([
            (
                200,
                json!({"items": [{"snippet": {"title": "Saturday night", "liveChatId": "C1"}}]}),
            ),
            (
                403,
                json!({"error": {"errors": [{"reason": "liveChatEnded"}]}}),
            ),
        ]);
        let link = Link::new();
        link.start(
            None,
            Some(Arc::new(MemoryStore::default())),
            Arc::clone(&google) as Arc<dyn Google>,
            Arc::new(|r: Vec<crate::crowd::Reaction>| r.len()),
            quick(),
        );
        link.set(Settings {
            on: true,
            client_id: "id".to_owned(),
            client_secret: String::new(),
        });
        link.signed_in("r").expect("kept");
        assert!(until(|| google
            .asked
            .lock()
            .expect("asked")
            .iter()
            .filter(|a| a.contains("liveBroadcasts"))
            .count()
            >= 2));
    }

    /// **The loopback address answers the browser** and gives the code; an
    /// icon request first does not end the wait; a declined sign-in is said
    /// on the page and to djmanzo.
    #[test]
    fn the_loopback_address_answers_the_browser() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let browser = std::thread::spawn(move || {
            let ask = |line: &str| {
                let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
                write!(s, "{line}\r\nHost: x\r\n\r\n").expect("written");
                let mut page = String::new();
                let _ = s.read_to_string(&mut page);
                page
            };
            (
                ask("GET /favicon.ico HTTP/1.1"),
                ask("GET /?state=ours&code=C0DE HTTP/1.1"),
            )
        });
        let code = await_callback(&listener, "ours", Duration::from_secs(5));
        let (icon, page) = browser.join().expect("the browser");
        assert_eq!(code, Ok("C0DE".to_owned()));
        assert!(icon.starts_with("HTTP/1.1 404"));
        assert!(page.starts_with("HTTP/1.1 200") && page.contains("signed in to YouTube"));

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let browser = std::thread::spawn(move || {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
            write!(s, "GET /?error=access_denied&state=ours HTTP/1.1\r\n\r\n").expect("written");
            let mut page = String::new();
            let _ = s.read_to_string(&mut page);
            page
        });
        assert_eq!(
            await_callback(&listener, "ours", Duration::from_secs(5)),
            Err("the sign-in was declined".to_owned())
        );
        assert!(browser.join().expect("the browser").contains("declined"));
        assert!(
            await_callback(
                &std::net::TcpListener::bind("127.0.0.1:0").expect("a port"),
                "ours",
                Duration::from_millis(50)
            )
            .is_err(),
            "the wait has an end"
        );
    }
}

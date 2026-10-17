//! The DJ's account on djmanzo's website, and the licence that comes with it.
//!
//! The owner asked for the website's sign-in and licences to reach the
//! application. A DJ signs in *on the website* — with Google, GitHub or
//! whichever account they bought with — and djmanzo never sees that
//! password. What djmanzo does is the OAuth device flow (RFC 8628): it asks
//! the site for a pair of codes, shows the short one, opens the browser at
//! the site's `/link` page, and polls until the DJ has said yes there. The
//! token it then receives is good for nothing but the site's small app API,
//! is kept in the system keychain
//! ([`dj_secrets::SecretKind::DjmanzoAccount`]), and can be revoked from
//! either end.
//!
//! # Licences are verified here, not trusted
//!
//! The site signs every licence and every activation with Ed25519; djmanzo
//! carries the public key ([`PUBLIC_KEYS`]) and checks the signature itself.
//! An *activation* is one licence on one machine for thirty days, renewed
//! whenever djmanzo is online — so a night in a basement with no signal is
//! a night the licence still works, and a cancelled subscription keeps only
//! what was paid for. The format is pinned on both sides by one test vector
//! (`website/test/vector.test.ts` and `tests::the_website_s_tokens_verify`).
//!
//! # Nothing is locked yet
//!
//! djmanzo is in beta, and [`ENFORCED`] is `false`: every feature stays open
//! whatever the licence says, and the account panel says what the licence
//! *covers* rather than switching anything off. Turning enforcement on is a
//! product decision for the owner, not something a sign-in should do by
//! surprise to a DJ mid-set — and when it is turned on, [`Standing::covers`]
//! is the one place that answers.
//!
//! Network calls run on their own thread or task, never near the audio.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use dj_secrets::{Secret, SecretKind, SecretStore};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where the website lives. `DJMANZO_SITE` points it elsewhere while
/// developing (`wrangler dev` serves on `http://127.0.0.1:8787`).
pub const DEFAULT_SITE: &str = "https://djmanzo.johannes-neugschwentner.workers.dev";
pub const SITE_ENV: &str = "DJMANZO_SITE";

/// The site's licence-signing keys, newest first, as base64url raw Ed25519
/// public keys (`GET /api/licence/public-key` says which one is live). A
/// list, so a key can be rotated without stranding licences signed by the
/// one before it.
pub const PUBLIC_KEYS: &[&str] = &["v_49WdbmMIr-dpxWg_bHppIJdOZqBJzGYnyrvUwzqoQ"];

/// Beta: no feature is switched off by the licence. See the module docs.
pub const ENFORCED: bool = false;

/// The major version this build is. A perpetual licence covers its own major
/// version and every update within it.
pub const MAJOR: u32 = 1;

const FILE: &str = "account.json";

/// One of the four editions the site sells.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Edition {
    Hobbyist,
    Karaoke,
    Pro,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    Monthly,
    Yearly,
    Perpetual,
}

/// The parts of djmanzo an edition is about. Coarse on purpose: the site's
/// catalogue (`website/shared/catalog.ts`) lists features one by one for
/// buyers; the application only ever needs to ask which half a thing is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Area {
    /// Decks, mixer, effects, library, the keyboard and MIDI, the assistant.
    Core,
    /// Six decks, stems, timecode vinyl, HID, recording, streams, commercial use.
    Pro,
    /// The rotation, the singers' screen, the voice knob, microphones per singer.
    Karaoke,
}

impl Edition {
    #[must_use]
    pub const fn covers(self, area: Area) -> bool {
        match area {
            Area::Core => true,
            Area::Pro => matches!(self, Self::Pro | Self::Full),
            Area::Karaoke => matches!(self, Self::Karaoke | Self::Full),
        }
    }

    /// Which licence to use when a DJ holds several: the one covering most.
    const fn rank(self) -> u8 {
        match self {
            Self::Hobbyist => 0,
            Self::Karaoke | Self::Pro => 1,
            Self::Full => 2,
        }
    }
}

/// What a licence token says, once its signature has been checked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LicenceClaims {
    pub lid: String,
    pub uid: String,
    pub name: String,
    pub email: Option<String>,
    pub edition: Edition,
    pub plan: Plan,
    pub major: u32,
    pub seats: u32,
    /// Seconds since the epoch.
    pub iat: i64,
    /// Seconds; `None` for a perpetual licence.
    pub exp: Option<i64>,
    #[serde(default)]
    pub demo: bool,
}

/// What an activation token says: one licence, one machine, until `exp`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActivationClaims {
    pub lid: String,
    pub machine: String,
    pub edition: Edition,
    pub plan: Plan,
    pub major: u32,
    pub iat: i64,
    pub exp: i64,
    #[serde(default)]
    pub demo: bool,
}

/// Check `token`'s signature against any of `keys` and return its claims.
///
/// `None` for anything that is not exactly `<prefix>.<payload>.<signature>`
/// signed by one of the keys: a token is either the site's or nothing.
#[must_use]
pub fn verify<T: DeserializeOwned>(token: &str, prefix: &str, keys: &[&str]) -> Option<T> {
    let mut parts = token.trim().split('.');
    let (Some(p), Some(payload), Some(sig), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if p != prefix {
        return None;
    }
    let signature = URL_SAFE_NO_PAD.decode(sig).ok()?;
    let signed = format!("{p}.{payload}");
    let ok = keys.iter().any(|k| {
        URL_SAFE_NO_PAD.decode(k).is_ok_and(|raw| {
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, raw)
                .verify(signed.as_bytes(), &signature)
                .is_ok()
        })
    });
    if !ok {
        return None;
    }
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()
}

/// Where this machine stands, worked out from what was verified.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Standing {
    /// No account on this machine.
    Unlicensed,
    /// A licence in force on this machine.
    Licensed {
        edition: Edition,
        plan: Plan,
        /// When the licence itself ends, in seconds; `None` for perpetual.
        until: Option<i64>,
        /// When djmanzo must next reach the site, in seconds.
        offline_until: i64,
        demo: bool,
    },
    /// There was a licence, and it no longer stands here. Says why.
    Lapsed { reason: String },
}

impl Standing {
    /// Whether this standing covers `area`. Everything, while [`ENFORCED`] is off.
    #[must_use]
    pub const fn covers(&self, area: Area) -> bool {
        if !ENFORCED {
            return true;
        }
        match self {
            Self::Licensed { edition, .. } => edition.covers(area),
            Self::Unlicensed | Self::Lapsed { .. } => false,
        }
    }
}

/// The standing an activation gives `machine` at `now` (seconds).
#[must_use]
pub fn standing(activation: Option<&ActivationClaims>, machine: &str, now: i64) -> Standing {
    let Some(a) = activation else {
        return Standing::Unlicensed;
    };
    if a.machine != machine {
        return Standing::Lapsed {
            reason: "That activation belongs to another computer.".to_owned(),
        };
    }
    if a.plan == Plan::Perpetual && a.major < MAJOR {
        return Standing::Lapsed {
            reason: format!(
                "The licence is for version {}; this is version {MAJOR}.",
                a.major
            ),
        };
    }
    if a.exp <= now {
        return Standing::Lapsed {
            reason: "djmanzo has not reached the website for thirty days — connect once to renew."
                .to_owned(),
        };
    }
    Standing::Licensed {
        edition: a.edition,
        plan: a.plan,
        until: None,
        offline_until: a.exp,
        demo: a.demo,
    }
}

/// Of several verified licences, the one to activate: in force, covering the
/// most, and — between equals — a perpetual one over a subscription, then
/// the one that lasts longest.
#[must_use]
pub fn best(licences: &[LicenceClaims], now: i64) -> Option<&LicenceClaims> {
    licences
        .iter()
        .filter(|l| l.exp.is_none_or(|e| e > now))
        .filter(|l| l.plan != Plan::Perpetual || l.major >= MAJOR)
        .max_by_key(|l| {
            (
                l.edition.rank(),
                u8::from(!l.demo),
                l.exp.is_none(),
                l.exp.unwrap_or(i64::MAX),
            )
        })
}

/// A name for this computer the DJ will recognise on the website's list of
/// machines. Never anything that identifies the hardware.
#[must_use]
pub fn machine_name() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty());
    let os = if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "computer"
    };
    host.map_or_else(|| format!("djmanzo on a {os}"), |h| format!("{h} ({os})"))
}

/// What djmanzo keeps on disk: nothing secret. Signed tokens are not secrets
/// — they are only good on this machine and say only what the site said.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stored {
    /// A random id made once, so the site can count this computer as one seat.
    pub machine: String,
    pub name: Option<String>,
    pub email: Option<String>,
    /// Every licence the account holds, as signed tokens.
    pub licences: Vec<String>,
    /// This machine's activation, as a signed token.
    pub activation: Option<String>,
    /// When the site was last reached, seconds.
    pub refreshed: Option<i64>,
}

impl Stored {
    #[must_use]
    pub fn read(dir: &Path) -> Self {
        let mut s: Self = std::fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if s.machine.len() < 16 {
            let mut bytes = [0_u8; 24];
            if getrandom::fill(&mut bytes).is_ok() {
                s.machine = URL_SAFE_NO_PAD.encode(bytes);
            }
        }
        s
    }

    /// # Errors
    /// The settings folder could not be written.
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(FILE), text).map_err(|e| e.to_string())
    }
}

/// A licence as the account panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Shown {
    pub edition: Edition,
    pub plan: Plan,
    pub until: Option<i64>,
    pub demo: bool,
    /// The one this machine is activated with.
    pub here: bool,
}

/// A link waiting for the DJ to say yes in the browser.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pending {
    pub code: String,
    pub address: String,
}

/// Everything the account panel needs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub site: String,
    pub signed_in: bool,
    pub name: Option<String>,
    pub email: Option<String>,
    pub standing: Standing,
    pub licences: Vec<Shown>,
    pub pending: Option<Pending>,
    pub enforced: bool,
    pub machine_name: String,
    /// What it is doing, or what went wrong, in words.
    pub note: Option<String>,
}

#[derive(Debug)]
struct Inner {
    stored: Stored,
    pending: Option<Pending>,
    note: Option<String>,
}

/// The account, shared between the commands and the threads that talk to
/// the site. Cheap to clone.
#[derive(Clone, Debug)]
pub struct Account {
    inner: Arc<Mutex<Inner>>,
    dir: PathBuf,
    site: String,
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Account {
    #[must_use]
    pub fn new(dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        let stored = Stored::read(&dir);
        if let Err(error) = stored.write(&dir) {
            tracing::warn!(%error, "the account file was not written");
        }
        let site = std::env::var(SITE_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_SITE.to_owned());
        Self {
            inner: Arc::new(Mutex::new(Inner {
                stored,
                pending: None,
                note: None,
            })),
            dir,
            site: site.trim_end_matches('/').to_owned(),
            secrets,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
        }
    }

    fn token(&self) -> Option<String> {
        self.secrets
            .get(SecretKind::DjmanzoAccount)
            .ok()
            .map(|s| s.expose().to_owned())
            .filter(|s| !s.is_empty())
    }

    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> Option<R> {
        self.inner.lock().ok().map(|mut i| f(&mut i))
    }

    fn save(&self) {
        if let Some(Err(error)) = self.with(|i| i.stored.write(&self.dir)) {
            tracing::warn!(%error, "the account file was not written");
        }
    }

    pub(crate) fn note(&self, note: Option<String>) {
        self.with(|i| i.note = note);
    }

    #[must_use]
    pub fn status(&self) -> Status {
        let signed_in = self.token().is_some();
        self.with(|i| {
            let activation: Option<ActivationClaims> = i
                .stored
                .activation
                .as_deref()
                .and_then(|t| verify(t, "DJA1", PUBLIC_KEYS));
            let licences: Vec<LicenceClaims> = i
                .stored
                .licences
                .iter()
                .filter_map(|t| verify(t, "DJM1", PUBLIC_KEYS))
                .collect();
            let mut standing = standing(activation.as_ref(), &i.stored.machine, now());
            if let Standing::Licensed { until, .. } = &mut standing {
                *until = licences
                    .iter()
                    .find(|l| activation.as_ref().is_some_and(|a| a.lid == l.lid))
                    .and_then(|l| l.exp);
            }
            Status {
                site: self.site.clone(),
                signed_in,
                name: i.stored.name.clone(),
                email: i.stored.email.clone(),
                standing,
                licences: licences
                    .iter()
                    .map(|l| Shown {
                        edition: l.edition,
                        plan: l.plan,
                        until: l.exp,
                        demo: l.demo,
                        here: activation.as_ref().is_some_and(|a| a.lid == l.lid),
                    })
                    .collect(),
                pending: i.pending.clone(),
                enforced: ENFORCED,
                machine_name: machine_name(),
                note: i.note.clone(),
            }
        })
        .unwrap_or_else(|| Status {
            site: self.site.clone(),
            signed_in,
            name: None,
            email: None,
            standing: Standing::Unlicensed,
            licences: Vec::new(),
            pending: None,
            enforced: ENFORCED,
            machine_name: machine_name(),
            note: Some("The account could not be read.".to_owned()),
        })
    }

    /// Ask the site for a pair of codes and wait — on a thread of its own —
    /// for the DJ to say yes in the browser. Returns the short code and the
    /// address at once, so the panel can show them while the thread polls.
    ///
    /// # Errors
    /// The site could not be reached.
    pub async fn link_start(&self) -> Result<Pending, String> {
        let answer: serde_json::Value = self
            .http
            .post(format!("{}/api/device/start", self.site))
            .json(&serde_json::json!({ "machine": machine_name() }))
            .send()
            .await
            .map_err(|e| format!("the website could not be reached: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let field = |k: &str| {
            answer
                .get(k)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let (Some(device), Some(code), Some(address)) = (
            field("device_code"),
            field("user_code"),
            field("verification_uri_complete"),
        ) else {
            return Err("the website answered something djmanzo does not understand".to_owned());
        };
        let interval = answer
            .get("interval")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(5)
            .clamp(2, 30);
        let expires = answer
            .get("expires_in")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(900)
            .min(1800);
        let pending = Pending { code, address };
        self.with(|i| {
            i.pending = Some(pending.clone());
            i.note = Some("Waiting for you to say yes in the browser.".to_owned());
        });
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("dj-account-link".into())
            .spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(expires);
                while std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_secs(interval));
                    match tauri::async_runtime::block_on(me.poll(&device)) {
                        Poll::Waiting => {}
                        Poll::Done(result) => {
                            me.with(|i| i.pending = None);
                            match result {
                                Ok(()) => {
                                    me.note(Some("Linked.".to_owned()));
                                    if let Err(error) = tauri::async_runtime::block_on(me.refresh())
                                    {
                                        me.note(Some(error));
                                    }
                                }
                                Err(error) => me.note(Some(error)),
                            }
                            return;
                        }
                    }
                }
                me.with(|i| {
                    i.pending = None;
                    i.note = Some("The code expired before it was used. Try again.".to_owned());
                });
            });
        if let Err(error) = spawned {
            return Err(error.to_string());
        }
        Ok(pending)
    }

    async fn poll(&self, device: &str) -> Poll {
        let Ok(response) = self
            .http
            .post(format!("{}/api/device/token", self.site))
            .json(&serde_json::json!({ "device_code": device }))
            .send()
            .await
        else {
            return Poll::Waiting; // A dropped connection is not a refusal; try again next time round.
        };
        let Ok(answer) = response.json::<serde_json::Value>().await else {
            return Poll::Waiting;
        };
        if let Some(token) = answer
            .get("access_token")
            .and_then(serde_json::Value::as_str)
        {
            return Poll::Done(
                self.secrets
                    .set(SecretKind::DjmanzoAccount, &Secret::new(token.to_owned()))
                    .map_err(|e| format!("the sign-in could not be kept in the keychain: {e}")),
            );
        }
        match answer.get("error").and_then(serde_json::Value::as_str) {
            Some("authorization_pending") | None => Poll::Waiting,
            Some("access_denied") => {
                Poll::Done(Err("The link was refused in the browser.".to_owned()))
            }
            Some("expired_token") => Poll::Done(Err(
                "The code expired before it was used. Try again.".to_owned(),
            )),
            Some(other) => Poll::Done(Err(other.to_owned())),
        }
    }

    /// Fetch the account's licences, keep those whose signatures hold, and
    /// activate the best one on this machine.
    ///
    /// # Errors
    /// Not signed in, the site unreachable, or every seat taken.
    pub async fn refresh(&self) -> Result<(), String> {
        let token = self.token().ok_or_else(|| "Not signed in.".to_owned())?;
        let me: serde_json::Value = self
            .http
            .get(format!("{}/api/app/me", self.site))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("the website could not be reached: {e}"))?
            .error_for_status()
            .map_err(|e| format!("the website refused: {e}"))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let tokens: Vec<String> = me
            .get("licences")
            .and_then(serde_json::Value::as_array)
            .map(|ls| {
                ls.iter()
                    .filter_map(|l| {
                        l.get("token")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let verified: Vec<LicenceClaims> = tokens
            .iter()
            .filter_map(|t| verify(t, "DJM1", PUBLIC_KEYS))
            .collect();
        let user = me.get("user");
        let text = |k: &str| {
            user.and_then(|u| u.get(k))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let machine = self.with(|i| {
            i.stored.licences = tokens.clone();
            i.stored.name = text("name");
            i.stored.email = text("email");
            i.stored.refreshed = Some(now());
            i.stored.machine.clone()
        });
        self.save();
        let (Some(machine), Some(chosen)) = (machine, best(&verified, now())) else {
            self.note(Some(if verified.is_empty() {
                "Signed in. No licence on this account yet — djmanzo stays fully open in the beta."
                    .to_owned()
            } else {
                "Every licence on this account has ended.".to_owned()
            }));
            return Ok(());
        };
        let response = self
            .http
            .post(format!("{}/api/app/activate", self.site))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "licence": chosen.lid, "machine": machine, "name": machine_name() }))
            .send()
            .await
            .map_err(|e| format!("the website could not be reached: {e}"))?;
        let ok = response.status().is_success();
        let answer: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        if !ok {
            let said = answer
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("the activation was refused");
            self.note(Some(said.to_owned()));
            return Err(said.to_owned());
        }
        let activation = answer
            .get("activation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if activation
            .as_deref()
            .and_then(|a| verify::<ActivationClaims>(a, "DJA1", PUBLIC_KEYS))
            .is_none()
        {
            return Err(
                "the website's activation did not verify — djmanzo kept what it had".to_owned(),
            );
        }
        self.with(|i| i.stored.activation = activation);
        self.save();
        self.note(None);
        Ok(())
    }

    /// Sign this machine out: tell the site to forget the token, free the
    /// seat, and forget everything here.
    pub async fn sign_out(&self) {
        if let Some(token) = self.token() {
            let (lid, machine) = self
                .with(|i| {
                    (
                        i.stored
                            .activation
                            .as_deref()
                            .and_then(|t| verify::<ActivationClaims>(t, "DJA1", PUBLIC_KEYS))
                            .map(|a| a.lid),
                        i.stored.machine.clone(),
                    )
                })
                .unwrap_or_default();
            if let Some(lid) = lid {
                let _ = self
                    .http
                    .post(format!("{}/api/app/deactivate", self.site))
                    .bearer_auth(&token)
                    .json(&serde_json::json!({ "licence": lid, "machine": machine }))
                    .send()
                    .await;
            }
            let _ = self
                .http
                .post(format!("{}/api/app/signout", self.site))
                .bearer_auth(&token)
                .send()
                .await;
        }
        if let Err(error) = self.secrets.delete(SecretKind::DjmanzoAccount) {
            tracing::warn!(%error, "the account sign-in was not forgotten");
        }
        self.with(|i| {
            let machine = i.stored.machine.clone();
            i.stored = Stored {
                machine,
                ..Stored::default()
            };
            i.pending = None;
            i.note = None;
        });
        self.save();
    }

    /// Renew once a day while djmanzo runs, on a thread of its own, so the
    /// thirty days of grace keep rolling forward whenever there is a connection.
    pub fn keep_fresh(&self) {
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("dj-account-renew".into())
            .spawn(move || {
                loop {
                    if me.token().is_some()
                        && let Err(error) = tauri::async_runtime::block_on(me.refresh())
                    {
                        tracing::info!(%error, "the licence was not renewed this time");
                    }
                    std::thread::sleep(Duration::from_secs(24 * 3600));
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "the licence renewal thread did not start");
        }
    }
}

enum Poll {
    Waiting,
    Done(Result<(), String>),
}

/// The account: who is signed in, the licences, and where this machine stands.
#[tauri::command]
#[must_use]
pub fn account_status(account: tauri::State<'_, Account>) -> Status {
    account.status()
}

/// Start linking this machine: returns the code to show and opens the
/// browser at the site's `/link` page.
///
/// # Errors
/// The site could not be reached.
#[tauri::command]
pub async fn account_link(
    app: tauri::AppHandle,
    account: tauri::State<'_, Account>,
) -> Result<Status, String> {
    use tauri_plugin_opener::OpenerExt as _;
    let pending = account.link_start().await?;
    if let Err(error) = app.opener().open_url(pending.address, None::<&str>) {
        // The code is issued and the poll is running; the panel shows the
        // address, so a machine with no browser to open is not a failure.
        tracing::info!(%error, "the browser did not open for the account link");
        account.note(Some(
            "The browser did not open by itself — open the address shown.".to_owned(),
        ));
    }

    Ok(account.status())
}

/// Check in with the site now.
///
/// # Errors
/// See [`Account::refresh`].
#[tauri::command]
pub async fn account_refresh(account: tauri::State<'_, Account>) -> Result<Status, String> {
    account.refresh().await?;
    Ok(account.status())
}

#[tauri::command]
pub async fn account_sign_out(account: tauri::State<'_, Account>) -> Result<Status, String> {
    account.sign_out().await;
    Ok(account.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Signed by the website's own code with a test key; pinned identically in
    // `website/test/vector.test.ts`.
    const TEST_PUBLIC: &str = "b1LFMiWzX6DnI2eOjRPiFCEsOwlR0sSX3KdCefRzV9I";
    const LICENCE: &str = "DJM1.eyJ2IjoxLCJsaWQiOiJsX3Rlc3QiLCJ1aWQiOiJ1X3Rlc3QiLCJuYW1lIjoiREogVmVjdG9yIiwiZW1haWwiOm51bGwsImVkaXRpb24iOiJrYXJhb2tlIiwicGxhbiI6InllYXJseSIsIm1ham9yIjoxLCJzZWF0cyI6MywiaWF0IjoxODAwMDAwMDAwLCJleHAiOjE4MzE1MzYwMDB9.qSjSSkfYf1QUDrY4hbOwSNDNxbdpIxnPxLnkWxBjo62mdVBvPKmS7zYuOqQok5Pc-pXK4DOJplF2ayschu_pCQ";
    const ACTIVATION: &str = "DJA1.eyJ2IjoxLCJsaWQiOiJsX3Rlc3QiLCJtYWNoaW5lIjoibWFjaGluZS12ZWN0b3ItMDEyMzQ1Njc4OSIsImVkaXRpb24iOiJrYXJhb2tlIiwicGxhbiI6InllYXJseSIsIm1ham9yIjoxLCJpYXQiOjE4MDAwMDAwMDAsImV4cCI6MTgwMjU5MjAwMH0.FPcpbeH_UqICfat_MLesZHvvlsDOLoPJNZSxC3uSfL2fd74bHMQfpKhAXgp-8MFpAlqFtpSAd9HUFm3s1s6-CA";

    #[test]
    fn the_website_s_tokens_verify() {
        let l: LicenceClaims =
            verify(LICENCE, "DJM1", &[TEST_PUBLIC]).expect("the website's licence verifies");
        assert_eq!(l.edition, Edition::Karaoke);
        assert_eq!(l.plan, Plan::Yearly);
        assert_eq!(l.exp, Some(1_831_536_000));
        assert_eq!(l.seats, 3);
        let a: ActivationClaims =
            verify(ACTIVATION, "DJA1", &[TEST_PUBLIC]).expect("the website's activation verifies");
        assert_eq!(a.machine, "machine-vector-0123456789");
        assert_eq!(a.exp - a.iat, 30 * 86_400);
    }

    #[test]
    fn a_token_that_was_changed_does_not_verify() {
        // One character of the payload changed: "karaoke" becomes "karaokf" in base64.
        let mut forged = LICENCE.to_owned();
        let at = forged
            .find("rYXJhb2tl")
            .expect("the edition is in the payload")
            + 3;
        forged.replace_range(at..=at, "Y");
        assert!(verify::<serde_json::Value>(&forged, "DJM1", &[TEST_PUBLIC]).is_none());
        // The right token against the wrong key, and the right key with the wrong prefix.
        assert!(verify::<LicenceClaims>(LICENCE, "DJM1", PUBLIC_KEYS).is_none());
        assert!(verify::<LicenceClaims>(LICENCE, "DJA1", &[TEST_PUBLIC]).is_none());
        assert!(verify::<LicenceClaims>("DJM1.e30", "DJM1", &[TEST_PUBLIC]).is_none());
    }

    #[test]
    fn an_activation_stands_until_it_ends_and_only_on_its_machine() {
        let a: ActivationClaims = verify(ACTIVATION, "DJA1", &[TEST_PUBLIC]).expect("verifies");
        let machine = "machine-vector-0123456789";
        assert!(matches!(
            standing(Some(&a), machine, a.exp - 1),
            Standing::Licensed {
                edition: Edition::Karaoke,
                ..
            }
        ));
        assert!(matches!(
            standing(Some(&a), machine, a.exp),
            Standing::Lapsed { .. }
        ));
        assert!(matches!(
            standing(Some(&a), "another-machine-entirely", a.iat),
            Standing::Lapsed { .. }
        ));
        assert_eq!(standing(None, machine, 0), Standing::Unlicensed);
    }

    #[test]
    fn the_best_licence_covers_most_and_is_in_force() {
        let base = LicenceClaims {
            lid: "a".into(),
            uid: "u".into(),
            name: "n".into(),
            email: None,
            edition: Edition::Hobbyist,
            plan: Plan::Perpetual,
            major: 1,
            seats: 2,
            iat: 0,
            exp: None,
            demo: false,
        };
        let full_ended = LicenceClaims {
            lid: "b".into(),
            edition: Edition::Full,
            plan: Plan::Monthly,
            exp: Some(50),
            ..base.clone()
        };
        let pro = LicenceClaims {
            lid: "c".into(),
            edition: Edition::Pro,
            plan: Plan::Yearly,
            exp: Some(500),
            ..base.clone()
        };
        let demo_full = LicenceClaims {
            lid: "d".into(),
            edition: Edition::Full,
            demo: true,
            ..base.clone()
        };
        let all = [base.clone(), full_ended, pro.clone()];
        assert_eq!(best(&all, 100).map(|l| l.lid.as_str()), Some("c"));
        // An old major version's perpetual licence does not cover this one.
        assert!(
            best(
                &[LicenceClaims {
                    major: 0,
                    ..base.clone()
                }],
                100
            )
            .is_none()
        );
        // A demo Full still outranks a real Hobbyist on coverage; the panel says "demo".
        assert_eq!(
            best(&[base, demo_full], 100).map(|l| l.lid.as_str()),
            Some("d")
        );
    }

    #[test]
    fn editions_cover_their_halves_and_nothing_is_locked_in_the_beta() {
        assert!(Edition::Karaoke.covers(Area::Karaoke) && !Edition::Karaoke.covers(Area::Pro));
        assert!(Edition::Pro.covers(Area::Pro) && !Edition::Pro.covers(Area::Karaoke));
        assert!(Edition::Full.covers(Area::Pro) && Edition::Full.covers(Area::Karaoke));
        assert!(Edition::Hobbyist.covers(Area::Core) && !Edition::Hobbyist.covers(Area::Pro));
        // Turning enforcement on is the owner's decision (see the module docs);
        // until then an unlicensed machine is covered for everything.
        assert!(Standing::Unlicensed.covers(Area::Pro));
    }

    #[test]
    fn the_stored_file_keeps_one_machine_id() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let first = Stored::read(dir.path());
        assert!(first.machine.len() >= 16);
        first.write(dir.path()).expect("written");
        assert_eq!(Stored::read(dir.path()).machine, first.machine);
    }
}

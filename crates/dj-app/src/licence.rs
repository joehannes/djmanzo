//! §131: the licence — which edition this djmanzo is, signed by the store.
//!
//! Every DJ downloads the same application ([ADR-0011](../../../docs/adr/0011-licence-by-sign-in.md));
//! what they may use is the [`Modules`] their licence carries. The licence is
//! fetched by signing in (Google, Apple, GitHub, Microsoft or Discord) from
//! the store's Worker, which reads the DJ's record and signs a short statement
//! of it. This module is the application's half: reading that statement,
//! checking its signature offline, and saying what it means today.
//!
//! # The shape
//!
//! One line of text, kept as [`FILE`] beside the other settings:
//!
//! ```text
//! djmanzo-licence-1.<payload, base64url>.<Ed25519 signature over the payload's bytes, base64url>
//! ```
//!
//! The payload is a JSON [`Payload`]. The signature is checked against
//! [`store_key`] before a word of the payload is believed, the same order
//! `dj_app::packs` checks a pack in.
//!
//! # What it never does
//!
//! - **Take something away mid-set.** A licence that gives more applies at
//!   once ([`Standing::adopt`]); one that gives less waits for the next start.
//! - **Need a connection to play.** A paid edition past its `renew_by` keeps
//!   working and asks the DJ to sign in when they can ([`Standing::renew_due`]).
//!   Only a trial ends by the calendar, because that is what a trial is.
//! - **Hold anything the DJ made.** The collection, cues and settings are not
//!   this module's business in any state.
//!
//! Nothing here decides what djmanzo does without a licence: that is the
//! owner's decision (`docs/EDITIONS.md`, *The 7-day trial*), and
//! [`Standing::modules`] says `None` rather than guess.

use base64::Engine as _;
use dj_core::edition::{Edition, Modules};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The licence's file name in the config directory.
pub const FILE: &str = "licence.txt";

/// What every licence line starts with: the format, so a later one can be
/// told apart rather than misread.
pub const PREFIX: &str = "djmanzo-licence-1.";

/// The longest licence line read. A real one is a few hundred bytes.
pub const MOST_BYTES: usize = 8 * 1024;

/// The key licences are signed with in development, Ed25519, base64.
///
/// Its private half is in the licence service's tests
/// (`services/licence/test/dev-key.json`), so anyone can sign with it: a
/// licence it signed is believed only in a debug build
/// ([`Standing::development`]). A release is built with the store's own key in
/// `DJMANZO_LICENCE_KEY`.
pub const DEV_KEY: &str = "twAB0cH0SC2jfnUNk8epDhSWQAXvQhymLQcLAkwr9r4=";

/// The key licences are checked against: the store's, when the build was
/// given one, else [`DEV_KEY`].
#[must_use]
pub fn store_key() -> &'static str {
    option_env!("DJMANZO_LICENCE_KEY").unwrap_or(DEV_KEY)
}

/// What the store signs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    /// The format of this payload.
    pub v: u32,
    /// Who it is for: the sign-in provider and that provider's identifier
    /// for them, `google:1093…`. Never an e-mail address.
    pub account: String,
    /// The edition's slug ([`Edition::slug`]).
    pub edition: String,
    /// For Custom only: the modules bought, as [`Modules::parse`] reads them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modules: Option<String>,
    /// A trial rather than a purchase.
    #[serde(default)]
    pub trial: bool,
    /// When a trial ends, in seconds since 1970. Absent for a purchase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends: Option<u64>,
    /// When it was signed.
    pub issued: u64,
    /// When djmanzo should fetch a fresh one: upgrades, refunds and a trial
    /// turned into a purchase arrive that way. Past it a purchase still works.
    pub renew_by: u64,
}

/// A licence whose signature has been checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    pub payload: Payload,
    pub edition: Edition,
    /// What it unlocks on top of the foundation.
    pub modules: Modules,
    /// Signed with [`DEV_KEY`].
    pub development: bool,
}

/// Read and check one licence line against `key` (base64 Ed25519).
///
/// # Errors
/// Why it is not a licence djmanzo believes, in a sentence for the DJ.
pub fn read(line: &str, key: &str) -> Result<Licence, String> {
    let line = line.trim();
    if line.len() > MOST_BYTES {
        return Err("that is far too long to be a licence".to_owned());
    }
    let rest = line
        .strip_prefix(PREFIX)
        .ok_or_else(|| "that is not a djmanzo licence".to_owned())?;
    let (payload_text, signature_text) = rest
        .split_once('.')
        .ok_or_else(|| "the licence is cut short".to_owned())?;
    let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let payload_bytes = url
        .decode(payload_text)
        .map_err(|_| "the licence is damaged".to_owned())?;
    let signature = url
        .decode(signature_text)
        .map_err(|_| "the licence's signature is damaged".to_owned())?;
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key)
        .map_err(|_| "djmanzo's own store key is damaged".to_owned())?;

    // The signature, before anything the payload says is believed.
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &key_bytes)
        .verify(&payload_bytes, &signature)
        .map_err(|_| {
            "the licence's signature does not match: it was changed, or it is not from djmanzo's store"
                .to_owned()
        })?;

    let payload: Payload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "the licence is signed but djmanzo cannot read it".to_owned())?;
    if payload.v != 1 {
        return Err(format!(
            "the licence is format {}, newer than this djmanzo reads: update djmanzo",
            payload.v
        ));
    }
    let edition = Edition::from_slug(&payload.edition).ok_or_else(|| {
        format!(
            "the licence names an edition this djmanzo does not know, {:?}",
            payload.edition
        )
    })?;
    let modules = match (edition, payload.modules.as_deref()) {
        (Edition::Custom, Some(list)) => Modules::parse(list)?,
        (Edition::Custom, None) => Modules::NONE,
        (_, Some(_)) => {
            return Err("the licence lists modules for an edition that is not Custom".to_owned());
        }
        (other, None) => other.modules(),
    };
    if payload.trial && payload.ends.is_none() {
        return Err("the licence is a trial with no end".to_owned());
    }
    Ok(Licence {
        payload,
        edition,
        modules,
        development: key == DEV_KEY,
    })
}

/// Where this djmanzo stands, worked out once at start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// No licence on this machine: the DJ has not signed in.
    SignedOut,
    /// A licence that is not believed, and why. The file is left where it is.
    Refused(String),
    /// A purchase, or a trial still running.
    Licensed(Licence),
    /// A trial that has ended.
    TrialOver(Licence),
}

/// Seconds in a day.
const DAY: u64 = 24 * 60 * 60;

impl Standing {
    /// From a licence line (or none) at `now`, seconds since 1970, checked
    /// against `key`. `debug` is whether this is a debug build, the only kind
    /// that believes [`DEV_KEY`] when it is not also the store's key.
    #[must_use]
    pub fn from_line(line: Option<&str>, key: &str, now: u64, debug: bool) -> Self {
        let Some(line) = line.filter(|l| !l.trim().is_empty()) else {
            return Self::SignedOut;
        };
        match read(line, key) {
            Err(why) => Self::Refused(why),
            Ok(licence) if licence.development && !debug => Self::Refused(
                "the licence is a development licence, which only a development build believes"
                    .to_owned(),
            ),
            Ok(licence)
                if licence.payload.trial && licence.payload.ends.is_some_and(|end| now >= end) =>
            {
                Self::TrialOver(licence)
            }
            Ok(licence) => Self::Licensed(licence),
        }
    }

    /// From [`FILE`] in `dir`.
    #[must_use]
    pub fn from_dir(dir: &Path, now: u64) -> Self {
        let line = read_file(dir);
        Self::from_line(line.as_deref(), store_key(), now, cfg!(debug_assertions))
    }

    /// What it unlocks on top of the foundation, or `None` where no licence
    /// is in force and what follows is the owner's decision.
    #[must_use]
    pub fn modules(&self) -> Option<Modules> {
        match self {
            Self::Licensed(licence) => Some(licence.modules),
            Self::SignedOut | Self::Refused(_) | Self::TrialOver(_) => None,
        }
    }

    /// Whole days left of a running trial, counting a part day as a day: the
    /// number the status strip says.
    #[must_use]
    pub fn trial_days_left(&self, now: u64) -> Option<u64> {
        match self {
            Self::Licensed(licence) if licence.payload.trial => {
                let end = licence.payload.ends?;
                Some(end.saturating_sub(now).div_ceil(DAY))
            }
            _ => None,
        }
    }

    /// Whether a fresh licence is due: said quietly, and never a reason to
    /// stop working.
    #[must_use]
    pub fn renew_due(&self, now: u64) -> bool {
        matches!(self, Self::Licensed(licence) if now >= licence.payload.renew_by)
    }

    /// Whether it rests on [`DEV_KEY`].
    #[must_use]
    pub fn development(&self) -> bool {
        matches!(self, Self::Licensed(l) | Self::TrialOver(l) if l.development)
    }

    /// What to wear now that `next` has arrived, while this is worn.
    ///
    /// **More applies at once, less waits.** A sign-in, a purchase or an
    /// upgrade takes effect the moment it arrives; a refund, a lapsed trial or
    /// a smaller edition only at the next start, so nothing a DJ is using
    /// disappears in the middle of a set. Returns what to wear now and
    /// whether `next` is left for the next start.
    #[must_use]
    pub fn adopt(&self, next: Self) -> (Self, bool) {
        let worn = self.modules();
        let gains = match (worn, next.modules()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(now), Some(then)) => now.within(then),
        };
        if gains {
            (next, false)
        } else {
            (self.clone(), true)
        }
    }
}

/// The licence line kept in `dir`, if there is one.
#[must_use]
pub fn read_file(dir: &Path) -> Option<String> {
    // Too long or not text is still *a* line, so it is refused with a reason
    // rather than mistaken for no licence at all.
    let bytes = std::fs::read(dir.join(FILE)).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Keep `line` as the licence in `dir`, once it reads as one against `key`.
///
/// # Errors
/// It is not a licence djmanzo believes, or the folder cannot be written.
pub fn install(dir: &Path, line: &str, key: &str) -> Result<Licence, String> {
    let licence = read(line, key)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot keep the licence: {e}"))?;
    let path = dir.join(FILE);
    let temporary = dir.join(format!("{FILE}.new"));
    std::fs::write(&temporary, format!("{}\n", line.trim()))
        .and_then(|()| std::fs::rename(&temporary, &path))
        .map_err(|e| format!("cannot keep the licence: {e}"))?;
    Ok(licence)
}

/// Forget the licence: signing out. The file is removed; nothing else is.
///
/// # Errors
/// The file is there and cannot be removed.
pub fn sign_out(dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_file(dir.join(RENEW_FILE));
    match std::fs::remove_file(dir.join(FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot sign out: {e}")),
    }
}

/// The sign-in providers the licence service knows, as its routes name them.
pub const PROVIDERS: [&str; 5] = ["google", "apple", "github", "microsoft", "discord"];

/// The environment variable naming the licence service, at run time or when
/// djmanzo is built.
pub const SERVICE_ENV: &str = "DJMANZO_LICENCE_SERVICE";

/// Where the licence service is: `https://…`, no trailing slash. `None`
/// until the owner has deployed it and a build names it.
#[must_use]
pub fn service() -> Option<String> {
    service_from(
        std::env::var(SERVICE_ENV)
            .ok()
            .or_else(|| option_env!("DJMANZO_LICENCE_SERVICE").map(str::to_owned)),
    )
}

/// [`service`]'s rule, given the setting: trimmed, and only ever HTTPS (or
/// this machine, for testing) — a licence fetched over plain HTTP could be
/// swapped on the way, and a signature would not save the renewal token.
#[must_use]
pub fn service_from(setting: Option<String>) -> Option<String> {
    setting
        .map(|s| s.trim().trim_end_matches('/').to_owned())
        .filter(|s| s.starts_with("https://") || s.starts_with("http://127.0.0.1"))
}

/// The page that starts a sign-in, opened in the DJ's own browser.
#[must_use]
pub fn login_address(
    service: &str,
    provider: &str,
    state: &str,
    challenge: &str,
    port: u16,
) -> String {
    format!(
        "{service}/login/{provider}?state={}&code_challenge={}&port={port}",
        urlencoding::encode(state),
        urlencoding::encode(challenge),
    )
}

/// What the service answers to a sign-in or a renewal.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Answer {
    pub licence: Option<String>,
    pub refused: Option<String>,
    pub renew: Option<String>,
    pub account: Option<String>,
}

/// The renewal token's file, beside the licence. Readable by the DJ alone
/// where the system has such a thing.
pub const RENEW_FILE: &str = "licence-renew.txt";

/// Keep the renewal token the service gave.
///
/// # Errors
/// The folder cannot be written.
pub fn keep_renewal(dir: &Path, token: &str) -> Result<(), String> {
    let path = dir.join(RENEW_FILE);
    std::fs::write(&path, token.trim()).map_err(|e| format!("cannot keep the sign-in: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// The renewal token, if the DJ has signed in on this machine.
#[must_use]
pub fn renewal(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(RENEW_FILE))
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// Seconds since 1970, now.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The standing this run wears, and a licence that arrived and waits for the
/// next start ([`Standing::adopt`]).
#[derive(Debug)]
pub struct Held {
    worn: std::sync::Mutex<(Standing, bool)>,
}

impl Default for Held {
    fn default() -> Self {
        Self {
            worn: std::sync::Mutex::new((Standing::SignedOut, false)),
        }
    }
}

impl Held {
    /// Read the licence in `dir`: once, at start.
    pub fn load(&self, dir: &Path) {
        let standing = Standing::from_dir(dir, now());
        if let Standing::Refused(why) = &standing {
            tracing::warn!(%why, "the licence on this machine is not believed");
        }
        if let Ok(mut worn) = self.worn.lock() {
            *worn = (standing, false);
        }
    }

    /// What is worn now.
    #[must_use]
    pub fn standing(&self) -> Standing {
        self.worn
            .lock()
            .map(|w| w.0.clone())
            .unwrap_or(Standing::SignedOut)
    }

    /// What may be used now: the licence's modules, or
    /// [`crate::gate::UNLICENSED`] where no licence is in force.
    #[must_use]
    ///
    /// Asked for every action on the bus, so it reads in place rather than
    /// cloning the standing: no allocation, one uncontended lock.
    pub fn in_force(&self) -> Modules {
        self.worn
            .lock()
            .ok()
            .and_then(|worn| worn.0.modules())
            .unwrap_or(crate::gate::UNLICENSED)
    }

    /// What the interface is told.
    #[must_use]
    pub fn status(&self) -> Status {
        let (standing, waiting) = self
            .worn
            .lock()
            .map(|w| w.clone())
            .unwrap_or((Standing::SignedOut, false));
        Status::of(&standing, now(), waiting)
    }

    /// Keep `line` in `dir` and wear it now if it gives more, else from the
    /// next start.
    ///
    /// # Errors
    /// It is not a licence djmanzo believes, or cannot be kept.
    pub fn install(&self, dir: &Path, line: &str) -> Result<Status, String> {
        install(dir, line, store_key())?;
        let next = Standing::from_dir(dir, now());
        if let Standing::Refused(why) = next {
            return Err(why);
        }
        let mut worn = self
            .worn
            .lock()
            .map_err(|_| "the licence is busy".to_owned())?;
        let (wear, waiting) = worn.0.adopt(next);
        *worn = (wear, waiting);
        Ok(Status::of(&worn.0, now(), worn.1))
    }

    /// Sign out: the file goes; what is worn stays until the next start.
    ///
    /// # Errors
    /// The file cannot be removed.
    pub fn sign_out(&self, dir: &Path) -> Result<Status, String> {
        sign_out(dir)?;
        let mut worn = self
            .worn
            .lock()
            .map_err(|_| "the licence is busy".to_owned())?;
        let (wear, waiting) = worn.0.adopt(Standing::SignedOut);
        *worn = (wear, waiting);
        Ok(Status::of(&worn.0, now(), worn.1))
    }
}

/// What the interface is told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    /// `signed_out`, `refused`, `licensed`, `trial` or `trial_over`.
    pub state: &'static str,
    /// The edition's slug, where there is a licence.
    pub edition: Option<&'static str>,
    /// What djmanzo is called in it.
    pub product: Option<&'static str>,
    /// The modules in force, by slug.
    pub modules: Vec<&'static str>,
    pub account: Option<String>,
    pub trial_days_left: Option<u64>,
    pub renew_due: bool,
    pub development: bool,
    /// Why a licence is refused.
    pub reason: Option<String>,
    /// A licence that arrived and waits for the next start.
    pub waiting: bool,
    /// What the licence in force does not carry, each with the cheapest
    /// edition that does: what the interface's quiet cards say. Empty where
    /// no licence is in force, because nothing is refused then.
    pub missing: Vec<Missing>,
}

/// A module the licence in force does not carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub module: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    /// The cheapest edition that has it, as the shop names it.
    pub edition: &'static str,
}

impl Status {
    /// `standing` as the interface is told it at `now`.
    #[must_use]
    pub fn of(standing: &Standing, now: u64, waiting: bool) -> Self {
        let licence = match standing {
            Standing::Licensed(l) | Standing::TrialOver(l) => Some(l),
            Standing::SignedOut | Standing::Refused(_) => None,
        };
        let state = match standing {
            Standing::SignedOut => "signed_out",
            Standing::Refused(_) => "refused",
            Standing::Licensed(l) if l.payload.trial => "trial",
            Standing::Licensed(_) => "licensed",
            Standing::TrialOver(_) => "trial_over",
        };
        Self {
            state,
            edition: licence.map(|l| l.edition.slug()),
            product: licence.map(|l| l.edition.product_name()),
            modules: standing
                .modules()
                .map(|m| m.iter().map(dj_core::edition::Module::slug).collect())
                .unwrap_or_default(),
            account: licence.map(|l| l.payload.account.clone()),
            trial_days_left: standing.trial_days_left(now),
            renew_due: standing.renew_due(now),
            development: standing.development(),
            reason: match standing {
                Standing::Refused(why) => Some(why.clone()),
                _ => None,
            },
            waiting,
            missing: standing
                .modules()
                .map(|m| {
                    m.missing()
                        .iter()
                        .map(|module| Missing {
                            module: module.slug(),
                            title: module.title(),
                            about: module.about(),
                            edition: Edition::cheapest_with(module).product_name(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_core::edition::Module;

    /// The development key pair, PKCS#8, base64: the private half of
    /// [`DEV_KEY`], the same bytes as `services/licence/test/dev-key.json`.
    const DEV_PKCS8: &str = "MC4CAQAwBQYDK2VwBCIEIPRHk1/8fhaH7aZj2Lowc3Gwf6zAJy9NYB5g0P2A5mio";

    const NOW: u64 = 1_792_000_000;

    /// A key pair from PKCS#8 as Node and WebCrypto write it (version 1, no
    /// public key inside), which `from_pkcs8` alone would refuse.
    fn pair(pkcs8_b64: &str) -> ring::signature::Ed25519KeyPair {
        let pkcs8 = base64::engine::general_purpose::STANDARD
            .decode(pkcs8_b64)
            .unwrap();
        ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&pkcs8).unwrap()
    }

    fn public(pkcs8_b64: &str) -> String {
        use ring::signature::KeyPair as _;
        base64::engine::general_purpose::STANDARD.encode(pair(pkcs8_b64).public_key().as_ref())
    }

    fn sign(payload: &Payload, pkcs8_b64: &str) -> String {
        let pair = pair(pkcs8_b64);
        let bytes = serde_json::to_vec(payload).unwrap();
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        format!(
            "{PREFIX}{}.{}",
            url.encode(&bytes),
            url.encode(pair.sign(&bytes).as_ref())
        )
    }

    fn payload(edition: &str) -> Payload {
        Payload {
            v: 1,
            account: "google:1093".to_owned(),
            edition: edition.to_owned(),
            modules: None,
            trial: false,
            ends: None,
            issued: NOW - DAY,
            renew_by: NOW + 29 * DAY,
        }
    }

    fn other_key() -> (String, String) {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(pkcs8.as_ref());
        let key = public(&encoded);
        (encoded, key)
    }

    /// **The development pair is a pair**: the private half the service's
    /// tests sign with is the one [`DEV_KEY`] checks. If either is changed
    /// alone, every development licence stops reading.
    #[test]
    fn the_development_key_is_the_public_half_of_the_tests_private_key() {
        assert_eq!(public(DEV_PKCS8), DEV_KEY);
    }

    /// **A purchase reads as its edition and that edition's modules.**
    #[test]
    fn a_signed_purchase_unlocks_its_edition() {
        let line = sign(&payload("karaoke"), DEV_PKCS8);
        let standing = Standing::from_line(Some(&line), DEV_KEY, NOW, true);
        let Standing::Licensed(licence) = &standing else {
            panic!("{standing:?}")
        };
        assert_eq!(licence.edition, Edition::Karaoke);
        assert_eq!(standing.modules(), Some(Modules::of(&[Module::Karaoke])));
        assert!(standing.development());
    }

    /// **A licence signed by the service itself reads here.** The line below
    /// was made by `services/licence`'s own signing code (its test
    /// `writes the shared test vector`), with the development key: the two
    /// halves of the system agree on the format byte for byte.
    #[test]
    fn a_licence_from_the_service_reads() {
        const FROM_SERVICE: &str = include_str!("../../../services/licence/test/vector.txt");
        let standing = Standing::from_line(Some(FROM_SERVICE), DEV_KEY, 1_792_000_000, true);
        let Standing::Licensed(licence) = &standing else {
            panic!("{standing:?}")
        };
        assert_eq!(licence.edition, Edition::Custom);
        assert_eq!(
            licence.modules,
            Modules::of(&[Module::Stems, Module::Karaoke])
        );
        assert_eq!(licence.payload.account, "github:583231");
    }

    /// **The service sells the editions djmanzo has.** Its table
    /// (`services/licence/src/editions.json`) is what it signs from; an
    /// edition or a module added here and not there would sign licences this
    /// djmanzo reads as something else.
    #[test]
    fn the_services_editions_are_djmanzos() {
        #[derive(Deserialize)]
        struct Table {
            modules: Vec<String>,
            editions: std::collections::BTreeMap<String, Vec<String>>,
        }
        let table: Table =
            serde_json::from_str(include_str!("../../../services/licence/src/editions.json"))
                .unwrap();
        let ours: Vec<&str> = Module::ALL.iter().map(|m| m.slug()).collect();
        assert_eq!(table.modules, ours, "the same modules, in djmanzo's order");
        assert_eq!(table.editions.len(), Edition::ALL.len());
        for edition in Edition::ALL {
            let theirs = &table.editions[edition.slug()];
            let mine: Vec<&str> = edition.modules().iter().map(Module::slug).collect();
            let mut theirs_sorted: Vec<&str> = theirs.iter().map(String::as_str).collect();
            let mut mine_sorted = mine.clone();
            theirs_sorted.sort_unstable();
            mine_sorted.sort_unstable();
            assert_eq!(theirs_sorted, mine_sorted, "{}", edition.slug());
        }
    }

    /// **One changed byte and it is refused**, with a sentence that says so —
    /// the payload is never read when the signature does not hold.
    #[test]
    fn a_changed_licence_is_refused() {
        let line = sign(&payload("hobbyist"), DEV_PKCS8);
        let (head, sig) = line.rsplit_once('.').unwrap();
        let url = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let body = head.strip_prefix(PREFIX).unwrap();
        let mut bytes = url.decode(body).unwrap();
        let text = String::from_utf8(bytes.clone())
            .unwrap()
            .replace("hobbyist", "full\"   ");
        bytes = text.into_bytes();
        let forged = format!("{PREFIX}{}.{sig}", url.encode(&bytes));
        let standing = Standing::from_line(Some(&forged), DEV_KEY, NOW, true);
        assert!(
            matches!(&standing, Standing::Refused(why) if why.contains("signature")),
            "{standing:?}"
        );
        assert_eq!(standing.modules(), None);
    }

    /// **Another key's licence is refused**: the store's key is the only one.
    #[test]
    fn a_licence_signed_with_another_key_is_refused() {
        let (pkcs8, _) = other_key();
        let line = sign(&payload("full"), &pkcs8);
        assert!(matches!(
            Standing::from_line(Some(&line), DEV_KEY, NOW, true),
            Standing::Refused(_)
        ));
    }

    /// **A release does not believe the development key**, whose private half
    /// is public; it does believe its own store key.
    #[test]
    fn a_release_refuses_a_development_licence_and_believes_its_own() {
        let line = sign(&payload("full"), DEV_PKCS8);
        assert!(matches!(
            Standing::from_line(Some(&line), DEV_KEY, NOW, false),
            Standing::Refused(why) if why.contains("development")
        ));
        let (pkcs8, public) = other_key();
        let line = sign(&payload("full"), &pkcs8);
        let standing = Standing::from_line(Some(&line), &public, NOW, false);
        assert_eq!(standing.modules(), Some(Modules::all()));
        assert!(!standing.development());
    }

    /// **A trial runs for its days and then is over**, counted in whole days
    /// the way the status strip says them.
    #[test]
    fn a_trial_counts_down_and_ends() {
        let mut trial = payload("full");
        trial.trial = true;
        trial.ends = Some(NOW + 7 * DAY);
        let line = sign(&trial, DEV_PKCS8);
        let running = Standing::from_line(Some(&line), DEV_KEY, NOW, true);
        assert_eq!(running.trial_days_left(NOW), Some(7));
        assert_eq!(running.trial_days_left(NOW + 6 * DAY + 1), Some(1));
        assert_eq!(Status::of(&running, NOW, false).state, "trial");
        let over = Standing::from_line(Some(&line), DEV_KEY, NOW + 7 * DAY, true);
        assert!(matches!(over, Standing::TrialOver(_)));
        assert_eq!(over.modules(), None);
        assert_eq!(Status::of(&over, NOW + 7 * DAY, false).state, "trial_over");
    }

    /// **A purchase past its renewal still works.** Playing never needs a
    /// connection; a DJ a month off the grid is asked to sign in when they
    /// can, and keeps everything.
    #[test]
    fn a_purchase_past_its_renewal_keeps_working() {
        let line = sign(&payload("pro"), DEV_PKCS8);
        let later = NOW + 400 * DAY;
        let standing = Standing::from_line(Some(&line), DEV_KEY, later, true);
        assert_eq!(standing.modules(), Some(Edition::ProDj.modules()));
        assert!(standing.renew_due(later));
        assert!(!standing.renew_due(NOW));
    }

    /// **Custom carries exactly what was bought**, and no other edition may
    /// name modules of its own.
    #[test]
    fn custom_carries_its_modules_and_only_custom_may() {
        let mut custom = payload("custom");
        custom.modules = Some("stems,dvs".to_owned());
        let standing = Standing::from_line(Some(&sign(&custom, DEV_PKCS8)), DEV_KEY, NOW, true);
        assert_eq!(
            standing.modules(),
            Some(Modules::of(&[Module::Stems, Module::Dvs]))
        );

        let mut greedy = payload("hobbyist");
        greedy.modules = Some("stems".to_owned());
        assert!(matches!(
            Standing::from_line(Some(&sign(&greedy, DEV_PKCS8)), DEV_KEY, NOW, true),
            Standing::Refused(_)
        ));
    }

    /// **More applies at once; less waits for the next start.** A Karaoke DJ
    /// who upgrades to Full gets it now; a Full licence replaced by Karaoke
    /// (a refund, say) leaves Full in place until djmanzo starts again.
    #[test]
    fn more_applies_at_once_and_less_waits() {
        let karaoke = Standing::from_line(
            Some(&sign(&payload("karaoke"), DEV_PKCS8)),
            DEV_KEY,
            NOW,
            true,
        );
        let full =
            Standing::from_line(Some(&sign(&payload("full"), DEV_PKCS8)), DEV_KEY, NOW, true);

        let (worn, waiting) = karaoke.adopt(full.clone());
        assert_eq!(worn, full);
        assert!(!waiting);

        let (worn, waiting) = full.adopt(karaoke.clone());
        assert_eq!(worn, full);
        assert!(waiting);

        let (worn, waiting) = full.adopt(Standing::SignedOut);
        assert_eq!(worn, full);
        assert!(waiting);

        let (worn, waiting) = Standing::SignedOut.adopt(karaoke.clone());
        assert_eq!(worn, karaoke);
        assert!(!waiting);
    }

    /// **The file round-trips, and signing out removes only it.**
    #[test]
    fn install_and_sign_out() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Standing::from_line(read_file(dir.path()).as_deref(), DEV_KEY, NOW, true),
            Standing::SignedOut
        );
        assert!(install(dir.path(), "not a licence", DEV_KEY).is_err());
        assert!(
            !dir.path().join(FILE).exists(),
            "a bad licence is never written"
        );

        let line = sign(&payload("apprentice"), DEV_PKCS8);
        install(dir.path(), &line, DEV_KEY).unwrap();
        std::fs::write(dir.path().join("library.db"), b"kept").unwrap();
        let standing = Standing::from_line(read_file(dir.path()).as_deref(), DEV_KEY, NOW, true);
        assert_eq!(standing.modules(), Some(Edition::Apprentice.modules()));

        sign_out(dir.path()).unwrap();
        sign_out(dir.path()).unwrap();
        assert!(!dir.path().join(FILE).exists());
        assert!(dir.path().join("library.db").exists());
    }

    /// **Held wears a sign-in at once and a sign-out from the next start**,
    /// and a line that is not a licence changes nothing.
    #[test]
    fn held_follows_the_rule() {
        let dir = tempfile::tempdir().unwrap();
        let held = Held::default();
        held.load(dir.path());
        assert_eq!(held.status().state, "signed_out");
        assert!(held.install(dir.path(), "nonsense").is_err());
        assert_eq!(held.status().state, "signed_out");

        let status = held
            .install(dir.path(), &sign(&payload("karaoke"), DEV_PKCS8))
            .unwrap();
        assert_eq!(
            (status.state, status.edition, status.waiting),
            ("licensed", Some("karaoke"), false)
        );

        let status = held.sign_out(dir.path()).unwrap();
        assert_eq!(
            (status.state, status.waiting),
            ("licensed", true),
            "still karaoke until the next start"
        );
        held.load(dir.path());
        assert_eq!(held.status().state, "signed_out");
    }

    /// **The sign-in page carries the state, the challenge and the port**,
    /// encoded, to the provider's route; and the service is only ever HTTPS
    /// (or this machine, for testing).
    #[test]
    fn the_sign_in_address_and_the_service() {
        let address = login_address("https://licence.example", "github", "s+t", "c/d", 49152);
        assert_eq!(
            address,
            "https://licence.example/login/github?state=s%2Bt&code_challenge=c%2Fd&port=49152"
        );
        let set = |s: &str| service_from(Some(s.to_owned()));
        assert_eq!(
            set("http://licence.example/"),
            None,
            "plain HTTP to anywhere but this machine is refused"
        );
        assert_eq!(
            set(" https://licence.example/ ").as_deref(),
            Some("https://licence.example")
        );
        assert_eq!(
            set("http://127.0.0.1:8787").as_deref(),
            Some("http://127.0.0.1:8787")
        );
        assert_eq!(service_from(None), None);
    }

    /// **Signing out forgets the renewal token too**, so a machine signed out
    /// cannot fetch a licence in the background.
    #[test]
    fn signing_out_forgets_the_renewal() {
        let dir = tempfile::tempdir().unwrap();
        keep_renewal(dir.path(), "tok\n").unwrap();
        assert_eq!(renewal(dir.path()).as_deref(), Some("tok"));
        sign_out(dir.path()).unwrap();
        assert_eq!(renewal(dir.path()), None);
    }

    /// **The interface is told the state, the edition and its modules.**
    #[test]
    fn the_status_says_what_is_in_force() {
        let standing = Standing::from_line(
            Some(&sign(&payload("autopilot"), DEV_PKCS8)),
            DEV_KEY,
            NOW,
            true,
        );
        let status = Status::of(&standing, NOW, false);
        assert_eq!(status.state, "licensed");
        assert_eq!(status.edition, Some("autopilot"));
        assert_eq!(status.product, Some("djmanzo Auto-Pilot"));
        assert_eq!(status.modules, vec!["autopilot"]);
        assert_eq!(status.account.as_deref(), Some("google:1093"));

        assert_eq!(status.missing.len(), 14);
        let teacher = status
            .missing
            .iter()
            .find(|m| m.module == "teacher")
            .unwrap();
        assert_eq!(
            (teacher.title, teacher.edition),
            ("Teacher", "djmanzo Apprentice")
        );

        let full =
            Standing::from_line(Some(&sign(&payload("full"), DEV_PKCS8)), DEV_KEY, NOW, true);
        assert!(Status::of(&full, NOW, false).missing.is_empty());

        let refused = Status::of(&Standing::Refused("no".to_owned()), NOW, false);
        assert!(
            refused.missing.is_empty(),
            "nothing is refused without a licence in force"
        );
        assert_eq!(refused.state, "refused");
        assert_eq!(refused.reason.as_deref(), Some("no"));
        assert!(refused.modules.is_empty());
    }
}

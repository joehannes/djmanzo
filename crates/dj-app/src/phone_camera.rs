//! §119: a phone as a camera, from djmanzo's own page — the switch, the
//! keys and the certificate authority kept between runs, and what the Camera
//! panel says.
//!
//! The page, the introduction and the HTTPS are `dj_net::phone`'s,
//! `dj_net::secure`'s and `dj_net::authority`'s; this decides when they run
//! and remembers what must not change from one night to the next:
//!
//! - **the port and OBS's key**, because OBS's browser source keeps the
//!   address it was given;
//! - **the phone's key**, so a phone that opened the page last week opens it
//!   again, until the DJ asks for a new one;
//! - **the authority**, because a phone that installed it should not have to
//!   again. Its key is written readable by the DJ only.
//!
//! Everything is in `phone_camera.json` and `phone_camera/` beside the other
//! settings.

use dj_net::authority::Authority;
use dj_net::phone::{Keys, PhoneNow, Serving, ViewerNow};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SETTINGS: &str = "phone_camera.json";
const FOLDER: &str = "phone_camera";
const AUTHORITY_CERT: &str = "authority.der";
const AUTHORITY_KEY: &str = "authority.key";

/// What is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub on: bool,
    pub port: u16,
    pub phone_key: String,
    pub viewer_key: String,
    /// The authority's name, which its certificate carries and a phone lists.
    pub authority: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on: false,
            port: dj_net::phone::DEFAULT_PORT,
            phone_key: String::new(),
            viewer_key: String::new(),
            authority: String::new(),
        }
    }
}

/// What the Camera panel shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Status {
    pub on: bool,
    /// The address for the phone, with its key: what the QR code says.
    pub phone_url: Option<String>,
    /// The QR code for it, as SVG.
    pub qr: Option<String>,
    /// The address for OBS's browser source.
    pub obs_url: Option<String>,
    /// The authority's fingerprint, to check against what a phone shows.
    pub fingerprint: Option<String>,
    pub phone: PhoneNow,
    pub viewer: ViewerNow,
    /// What went wrong, or what is missing, in words.
    pub note: Option<String>,
}

/// A new key: 16 random bytes, as 22 characters a URL carries unchanged.
///
/// # Errors
/// No randomness from the system.
pub fn new_key() -> Result<String, String> {
    use base64::Engine as _;
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no randomness from the system: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

#[derive(Default)]
struct Inner {
    dir: Option<PathBuf>,
    settings: Settings,
    authority: Option<Arc<Authority>>,
    serving: Option<Serving>,
    note: Option<String>,
}

/// The phone camera, switched on or off.
#[derive(Default)]
pub struct PhoneCamera {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for PhoneCamera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneCamera").finish_non_exhaustive()
    }
}

impl PhoneCamera {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Read what was kept in `dir`, and serve if it was left on.
    pub fn start(&self, dir: &Path) {
        let mut inner = self.inner();
        inner.dir = Some(dir.to_path_buf());
        inner.settings = std::fs::read_to_string(dir.join(SETTINGS))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if inner.settings.on {
            serve(&mut inner);
        }
    }

    /// Switch it on or off, and keep the choice.
    pub fn set(&self, on: bool) -> Status {
        let mut inner = self.inner();
        inner.settings.on = on;
        if on {
            serve(&mut inner);
        } else {
            inner.serving = None;
            inner.note = None;
        }
        keep(&inner);
        status_of(&inner)
    }

    /// A new key for the phone's address: the old address stops working,
    /// and whatever was being sent with it stops. OBS's address stays.
    pub fn new_phone_link(&self) -> Status {
        let mut inner = self.inner();
        match new_key() {
            Ok(key) => {
                inner.settings.phone_key.clone_from(&key);
                if let Some(serving) = &inner.serving {
                    serving.camera.replace_phone_key(key);
                }
                keep(&inner);
            }
            Err(why) => inner.note = Some(why),
        }
        status_of(&inner)
    }

    #[must_use]
    pub fn status(&self) -> Status {
        status_of(&self.inner())
    }

    /// The port it is serving on, if it is.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.inner().serving.as_ref().map(|s| s.address().port())
    }

    /// The authority, once made: for a test to trust it as a phone would.
    #[must_use]
    pub fn authority(&self) -> Option<Arc<Authority>> {
        self.inner().authority.clone()
    }
}

/// Start serving with what is kept, making whatever is missing.
fn serve(inner: &mut Inner) {
    if inner.serving.is_some() {
        return;
    }
    inner.note = None;
    let made = (|| -> Result<(), String> {
        if inner.settings.phone_key.is_empty() {
            inner.settings.phone_key = new_key()?;
        }
        if inner.settings.viewer_key.is_empty() {
            inner.settings.viewer_key = new_key()?;
        }
        if inner.authority.is_none() {
            inner.authority = Some(Arc::new(authority(inner)?));
        }
        Ok(())
    })();
    if let Err(why) = made {
        inner.note = Some(why);
        return;
    }
    let Some(authority) = inner.authority.clone() else {
        return;
    };
    let keys = Keys {
        phone: inner.settings.phone_key.clone(),
        viewer: inner.settings.viewer_key.clone(),
    };
    let at = |port| SocketAddr::from(([0, 0, 0, 0], port));
    // The kept port first, so OBS's address still works; another if that one
    // is taken, kept from then on.
    let served = dj_net::phone::serve(
        at(inner.settings.port),
        Arc::clone(&authority),
        keys.clone(),
    )
    .or_else(|first| {
        tracing::warn!(%first, "the phone camera's port is taken; another is used");
        dj_net::phone::serve(at(0), authority, keys)
    });
    match served {
        Ok(serving) => {
            inner.settings.port = serving.address().port();
            inner.serving = Some(serving);
        }
        Err(why) => inner.note = Some(why.to_string()),
    }
    keep(inner);
}

/// The authority kept in the folder, or a new one kept there.
fn authority(inner: &mut Inner) -> Result<Authority, String> {
    let folder = inner.dir.as_ref().map(|d| d.join(FOLDER));
    if let Some(folder) = &folder
        && !inner.settings.authority.is_empty()
        && let (Ok(der), Ok(key)) = (
            std::fs::read(folder.join(AUTHORITY_CERT)),
            std::fs::read(folder.join(AUTHORITY_KEY)),
        )
    {
        match Authority::restore(&inner.settings.authority, der, &key) {
            Ok(kept) => return Ok(kept),
            Err(why) => tracing::warn!(%why, "the phone camera's authority is made again"),
        }
    }
    // Named so that a phone listing it says whose it is, and which: a second
    // computer's is another.
    let name = format!("djmanzo phone camera {}", &new_key()?[..6]);
    let made = Authority::new(&name).map_err(|e| e.to_string())?;
    if let Some(folder) = &folder {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        std::fs::write(folder.join(AUTHORITY_CERT), made.certificate())
            .map_err(|e| e.to_string())?;
        write_private(&folder.join(AUTHORITY_KEY), &made.key()).map_err(|e| e.to_string())?;
    }
    inner.settings.authority = name;
    Ok(made)
}

/// Written readable by the DJ only, where the system has such a thing.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

fn keep(inner: &Inner) {
    if let Some(dir) = &inner.dir
        && let Ok(text) = serde_json::to_string_pretty(&inner.settings)
        && let Err(error) = std::fs::write(dir.join(SETTINGS), text)
    {
        tracing::warn!(%error, "phone_camera.json was not written");
    }
}

/// `host` as it goes in a URL.
fn host(address: IpAddr) -> String {
    match address {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    }
}

fn status_of(inner: &Inner) -> Status {
    let seen = inner.serving.as_ref().map(Serving::seen);
    let mut status = Status {
        on: inner.settings.on,
        phone_url: None,
        qr: None,
        obs_url: None,
        fingerprint: inner.authority.as_ref().map(|a| a.fingerprint()),
        phone: seen.map_or(PhoneNow::Away, |s| s.phone),
        viewer: seen.map_or(ViewerNow::Away, |s| s.viewer),
        note: inner.note.clone(),
    };
    let Some(serving) = &inner.serving else {
        return status;
    };
    let port = serving.address().port();
    status.obs_url = Some(format!(
        "http://127.0.0.1:{port}/v/{}/",
        inner.settings.viewer_key
    ));
    // The address a phone in the room can reach: tonight's, on tonight's
    // network, so it is asked each time.
    match dj_net::sticker::lan_address() {
        Some(address) => {
            let url = format!(
                "https://{}:{port}/p/{}/",
                host(address),
                inner.settings.phone_key
            );
            status.qr = dj_net::sticker::qr_svg(&url).ok();
            status.phone_url = Some(url);
        }
        None => {
            status.note.get_or_insert_with(|| {
                "This computer is on no network a phone could reach it by.".to_owned()
            });
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read as _, Write as _};
    use std::net::TcpStream;

    fn plain(port: u16, request: &str) -> String {
        let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
        socket.write_all(request.as_bytes()).unwrap();
        let mut got = String::new();
        let _ = socket.read_to_string(&mut got);
        got
    }

    /// **Switched on, it serves, with what it made kept**; switched off, the
    /// port closes; and a later run comes back with the same keys, port and
    /// authority, so OBS's address and an installed certificate still work.
    #[test]
    fn what_is_made_is_kept_between_runs() {
        let dir = tempfile::tempdir().unwrap();
        let camera = PhoneCamera::new();
        camera.start(dir.path());
        assert!(!camera.status().on);
        assert_eq!(camera.port(), None);

        // Port 0 kept, as a DJ whose 8443 was taken.
        let kept = Settings {
            port: 0,
            ..Settings::default()
        };
        std::fs::write(
            dir.path().join(SETTINGS),
            serde_json::to_string(&kept).unwrap(),
        )
        .unwrap();
        camera.start(dir.path());
        let status = camera.set(true);
        assert!(status.on);
        assert_eq!(status.note, None);
        let port = camera.port().expect("serving");
        assert_ne!(port, 0);
        let obs = status.obs_url.clone().unwrap();
        assert!(obs.starts_with(&format!("http://127.0.0.1:{port}/v/")));
        // OBS's page answers at the address given.
        let path = obs.trim_start_matches(&format!("http://127.0.0.1:{port}"));
        let page = plain(port, &format!("GET {path} HTTP/1.1\r\n\r\n"));
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("<video"));

        let settings: Settings =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(SETTINGS)).unwrap())
                .unwrap();
        assert!(settings.on);
        assert_eq!(settings.port, port);
        assert_eq!(settings.phone_key.len(), 22);
        assert_eq!(settings.viewer_key.len(), 22);
        assert_ne!(settings.phone_key, settings.viewer_key);
        assert!(settings.authority.starts_with("djmanzo phone camera "));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.path().join(FOLDER).join(AUTHORITY_KEY))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "the authority's key is readable by others");
        }
        let fingerprint = status.fingerprint.clone().unwrap();

        // Off: the port closes.
        camera.set(false);
        assert_eq!(camera.port(), None);
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());

        // The next run, left on: the same everything.
        camera.set(true);
        drop(camera);
        let next = PhoneCamera::new();
        next.start(dir.path());
        let again = next.status();
        assert!(again.on);
        assert_eq!(next.port(), Some(port));
        assert_eq!(again.obs_url.as_deref(), Some(obs.as_str()));
        assert_eq!(again.fingerprint.as_deref(), Some(fingerprint.as_str()));
    }

    /// **A new phone link changes the phone's address and not OBS's**, and
    /// is kept.
    #[test]
    fn a_new_phone_link_leaves_obs_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS),
            serde_json::to_string(&Settings {
                port: 0,
                ..Settings::default()
            })
            .unwrap(),
        )
        .unwrap();
        let camera = PhoneCamera::new();
        camera.start(dir.path());
        let kept = || -> Settings {
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(SETTINGS)).unwrap())
                .unwrap()
        };
        let before = camera.set(true);
        let old_key = kept().phone_key;
        let after = camera.new_phone_link();
        let new_key = kept().phone_key;
        assert_ne!(old_key, new_key);
        assert_eq!(new_key.len(), 22);
        assert_eq!(before.obs_url, after.obs_url);
        assert!(after.obs_url.is_some());
        // Where this computer is on a network, the code says the new key.
        if let Some(url) = &after.phone_url {
            assert!(url.ends_with(&format!("/p/{new_key}/")));
            assert!(after.qr.as_deref().is_some_and(|q| q.starts_with("<svg")));
        }
    }

    /// A kept authority whose key is not its own is made again rather than
    /// served with: what it signed would chain to nothing a phone installed.
    #[test]
    fn a_broken_authority_is_made_again() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS),
            serde_json::to_string(&Settings {
                port: 0,
                ..Settings::default()
            })
            .unwrap(),
        )
        .unwrap();
        let camera = PhoneCamera::new();
        camera.start(dir.path());
        let first = camera.set(true).fingerprint.unwrap();
        camera.set(false);
        drop(camera);
        std::fs::write(dir.path().join(FOLDER).join(AUTHORITY_KEY), b"not a key").unwrap();
        let next = PhoneCamera::new();
        next.start(dir.path());
        let second = next.set(true);
        assert_eq!(second.note, None);
        assert_ne!(second.fingerprint.unwrap(), first);
    }
}

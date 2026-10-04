//! §119: djmanzo drives OBS — the link.
//!
//! [`dj_net::obs`] speaks obs-websocket; this keeps the conversation going.
//! One thread owns the connection: it connects when the DJ switches the link
//! on, says what OBS is doing (streaming or not, which scene is on air,
//! which scenes there are), carries out what the DJ asks — start or stop the
//! stream, put a scene on air — and, when OBS goes away, tries again every
//! few seconds by itself, the way the singers' inputs do. Off until the DJ
//! switches it on, like everything here that speaks outside djmanzo.
//!
//! The thread talks to OBS through [`Studio`], so what it does with a
//! connection — and without one — is tested without OBS.

use dj_net::obs::{Event, Obs, ObsError, Scenes};
use dj_secrets::{Secret, SecretKind, SecretStore};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the DJ reaches OBS. Kept in `obs.json` — all but the password, which
/// is kept where every other key djmanzo holds is: the system's keychain
/// ([`dj_secrets::SecretKind::ObsPassword`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub on: bool,
    pub host: String,
    pub port: u16,
    #[serde(skip)]
    pub password: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on: false,
            host: "localhost".to_owned(),
            port: dj_net::obs::DEFAULT_PORT,
            password: String::new(),
        }
    }
}

/// What the link is doing, for the screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    /// The DJ switched the link on.
    pub on: bool,
    /// OBS is answering.
    pub connected: bool,
    /// Why not, or what OBS last refused — in words a DJ can act on.
    pub error: Option<String>,
    pub streaming: bool,
    pub scenes: Scenes,
}

/// Something the DJ asks of OBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    Stream(bool),
    Scene(String),
}

/// What the thread is told: a DJ's ask, waiting for the answer, or to look
/// again at the settings.
enum Order {
    Ask(Ask, SyncSender<Result<(), String>>),
    Reconnect,
}

/// The few things djmanzo does with OBS — [`Obs`] in the application, a
/// stand-in in the tests.
pub trait Studio: Send {
    /// # Errors
    /// OBS gone, or refusing.
    fn streaming(&mut self) -> Result<bool, ObsError>;
    /// # Errors
    /// As [`Studio::streaming`].
    fn scenes(&mut self) -> Result<Scenes, ObsError>;
    /// # Errors
    /// As [`Studio::streaming`].
    fn put_on_air(&mut self, scene: &str) -> Result<(), ObsError>;
    /// # Errors
    /// As [`Studio::streaming`].
    fn stream(&mut self, on: bool) -> Result<(), ObsError>;
    /// # Errors
    /// As [`Studio::streaming`].
    fn events(&mut self, wait: Duration) -> Result<Vec<Event>, ObsError>;
}

impl Studio for Obs {
    fn streaming(&mut self) -> Result<bool, ObsError> {
        Obs::streaming(self)
    }
    fn scenes(&mut self) -> Result<Scenes, ObsError> {
        Obs::scenes(self)
    }
    fn put_on_air(&mut self, scene: &str) -> Result<(), ObsError> {
        Obs::put_on_air(self, scene)
    }
    fn stream(&mut self, on: bool) -> Result<(), ObsError> {
        if on {
            self.start_stream()
        } else {
            self.stop_stream()
        }
    }
    fn events(&mut self, wait: Duration) -> Result<Vec<Event>, ObsError> {
        Obs::events(self, wait)
    }
}

/// How a connection is made: [`connect`] in the application.
pub type Connector = Box<dyn Fn(&Settings) -> Result<Box<dyn Studio>, ObsError> + Send>;

/// The application's connector: obs-websocket at the DJ's address.
pub fn connect() -> Connector {
    Box::new(|settings: &Settings| {
        Obs::connect(&settings.host, settings.port, &settings.password)
            .map(|obs| Box::new(obs) as Box<dyn Studio>)
    })
}

/// How often the thread does things, apart in the tests.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Between attempts to reach an OBS that is not answering.
    pub retry: Duration,
    /// Between looks at what OBS is doing, events aside.
    pub look: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            retry: Duration::from_secs(3),
            look: Duration::from_secs(2),
        }
    }
}

/// How long a DJ's ask waits for OBS before saying so.
const ANSWER_WITHIN: Duration = Duration::from_secs(8);

struct Shared {
    settings: Mutex<Settings>,
    status: Mutex<Status>,
}

/// The link to OBS: settings, what it is doing, and the thread that does it.
pub struct Link {
    shared: Arc<Shared>,
    path: Mutex<Option<PathBuf>>,
    secrets: Mutex<Option<Arc<dyn SecretStore>>>,
    orders: Mutex<Option<Sender<Order>>>,
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl Link {
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                settings: Mutex::new(Settings::default()),
                status: Mutex::new(Status::default()),
            }),
            path: Mutex::new(None),
            secrets: Mutex::new(None),
            orders: Mutex::new(None),
        }
    }

    /// Read the settings kept in `dir` and the password kept in `secrets`,
    /// and start the thread. Once; a second call does nothing.
    pub fn start(
        &self,
        dir: Option<&Path>,
        secrets: Option<Arc<dyn SecretStore>>,
        connector: Connector,
        timing: Timing,
    ) {
        let Ok(mut orders) = self.orders.lock() else {
            return;
        };
        if orders.is_some() {
            return;
        }
        let mut kept = Settings::default();
        if let Some(dir) = dir {
            let path = dir.join("obs.json");
            if let Some(read) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
            {
                kept = read;
            }
            if let Ok(mut slot) = self.path.lock() {
                *slot = Some(path);
            }
        }
        if let Some(store) = &secrets
            && let Ok(password) = store.get(SecretKind::ObsPassword)
        {
            kept.password = password.expose().to_owned();
        }
        if let Ok(mut settings) = self.shared.settings.lock() {
            *settings = kept;
        }
        if let Ok(mut slot) = self.secrets.lock() {
            *slot = secrets;
        }
        let (send, receive) = mpsc::channel();
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new()
            .name("obs".into())
            .spawn(move || run(&shared, &receive, &connector, timing));
        if spawned.is_ok() {
            *orders = Some(send);
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

    /// Keep new settings and have the thread connect again with them.
    pub fn set(&self, next: Settings) {
        let before = self.settings();
        if let Ok(mut settings) = self.shared.settings.lock() {
            *settings = next.clone();
        }
        if next.password != before.password
            && let Some(store) = self.secrets.lock().ok().and_then(|s| s.clone())
        {
            let kept = if next.password.is_empty() {
                store.delete(SecretKind::ObsPassword)
            } else {
                store.set(SecretKind::ObsPassword, &Secret::new(next.password.clone()))
            };
            if let Err(error) = kept {
                tracing::warn!(%error, "the OBS password will not survive a restart");
            }
        }
        if let Some(path) = self.path.lock().ok().and_then(|p| p.clone()) {
            match serde_json::to_string_pretty(&next) {
                Ok(text) => {
                    if let Err(error) = std::fs::write(&path, text) {
                        tracing::warn!(%error, ?path, "the OBS link will not survive a restart");
                    }
                }
                Err(error) => tracing::warn!(%error, "the OBS settings could not be written"),
            }
        }
        self.order(Order::Reconnect);
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.shared
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Ask OBS for something, and wait for its answer.
    ///
    /// # Errors
    /// The link off or not connected, OBS refusing (its own words), or no
    /// answer in time.
    pub fn ask(&self, ask: Ask) -> Result<(), String> {
        let (reply, answer) = mpsc::sync_channel(1);
        if !self.order(Order::Ask(ask, reply)) {
            return Err("the link to OBS is not running".to_owned());
        }
        answer
            .recv_timeout(ANSWER_WITHIN)
            .unwrap_or_else(|_| Err(ObsError::Timeout.to_string()))
    }

    fn order(&self, order: Order) -> bool {
        self.orders
            .lock()
            .ok()
            .and_then(|o| o.as_ref().map(|send| send.send(order).is_ok()))
            .unwrap_or(false)
    }
}

/// Whether an error means OBS is gone rather than that it said no.
fn gone(error: &ObsError) -> bool {
    matches!(
        error,
        ObsError::Unreachable(_) | ObsError::Closed | ObsError::Timeout | ObsError::Protocol(_)
    )
}

fn run(shared: &Shared, orders: &Receiver<Order>, connector: &Connector, timing: Timing) {
    let mut studio: Option<Box<dyn Studio>> = None;
    let mut tried: Option<Instant> = None;
    let mut looked: Option<Instant> = None;
    loop {
        let settings = shared
            .settings
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        if settings.on {
            if studio.is_none() && tried.is_none_or(|at| at.elapsed() >= timing.retry) {
                tried = Some(Instant::now());
                match connector(&settings) {
                    Ok(connected) => {
                        studio = Some(connected);
                        looked = None;
                        update(shared, |s| s.error = None);
                    }
                    Err(error) => update(shared, |s| {
                        s.connected = false;
                        s.error = Some(error.to_string());
                    }),
                }
            }
        } else {
            studio = None;
            tried = None;
        }
        update(shared, |s| {
            s.on = settings.on;
            s.connected = studio.is_some();
            if !settings.on {
                s.error = None;
                s.streaming = false;
                s.scenes = Scenes::default();
            }
        });

        let wait = if studio.is_some() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(200)
        };
        match orders.recv_timeout(wait) {
            Ok(Order::Reconnect) => {
                studio = None;
                tried = None;
            }
            Ok(Order::Ask(ask, reply)) => {
                let answer = match studio.as_mut() {
                    None if !settings.on => Err("the link to OBS is off".to_owned()),
                    None => Err(shared
                        .status
                        .lock()
                        .ok()
                        .and_then(|s| s.error.clone())
                        .unwrap_or_else(|| "OBS is not connected".to_owned())),
                    Some(obs) => {
                        let done = match &ask {
                            Ask::Stream(on) => obs.stream(*on),
                            Ask::Scene(scene) => obs.put_on_air(scene),
                        };
                        match done {
                            Ok(()) => {
                                looked = None;
                                Ok(())
                            }
                            Err(error) => {
                                if gone(&error) {
                                    studio = None;
                                }
                                Err(error.to_string())
                            }
                        }
                    }
                };
                let _ = reply.send(answer);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let Some(obs) = studio.as_mut() else { continue };
        let heard = if looked.is_none_or(|at| at.elapsed() >= timing.look) {
            looked = Some(Instant::now());
            obs.streaming().and_then(|streaming| {
                let scenes = obs.scenes()?;
                update(shared, |s| {
                    s.streaming = streaming;
                    s.scenes = scenes;
                });
                Ok(())
            })
        } else {
            obs.events(Duration::from_millis(1)).map(|events| {
                update(shared, |s| {
                    for event in events {
                        match event {
                            Event::Scene(scene) => s.scenes.current = scene,
                            Event::Streaming(on) => s.streaming = on,
                        }
                    }
                });
            })
        };
        if let Err(error) = heard {
            if gone(&error) {
                studio = None;
            }
            update(shared, |s| s.error = Some(error.to_string()));
        }
    }
}

fn update(shared: &Shared, change: impl FnOnce(&mut Status)) {
    if let Ok(mut status) = shared.status.lock() {
        change(&mut status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An OBS in memory: two scenes and a stream, and a switch to make it
    /// vanish the way a closed OBS does.
    #[derive(Clone)]
    struct Fake {
        state: Arc<Mutex<FakeState>>,
    }

    struct FakeState {
        streaming: bool,
        on_air: String,
        gone: bool,
        events: Vec<Event>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeState {
                    streaming: false,
                    on_air: "Decks".into(),
                    gone: false,
                    events: Vec::new(),
                })),
            }
        }
        fn with<T>(
            &self,
            f: impl FnOnce(&mut FakeState) -> Result<T, ObsError>,
        ) -> Result<T, ObsError> {
            let mut state = self.state.lock().unwrap();
            if state.gone {
                return Err(ObsError::Closed);
            }
            f(&mut state)
        }
    }

    impl Studio for Fake {
        fn streaming(&mut self) -> Result<bool, ObsError> {
            self.with(|s| Ok(s.streaming))
        }
        fn scenes(&mut self) -> Result<Scenes, ObsError> {
            self.with(|s| {
                Ok(Scenes {
                    current: s.on_air.clone(),
                    all: vec!["Decks".into(), "Crowd".into()],
                })
            })
        }
        fn put_on_air(&mut self, scene: &str) -> Result<(), ObsError> {
            self.with(|s| {
                if scene != "Decks" && scene != "Crowd" {
                    return Err(ObsError::Request {
                        request: "SetCurrentProgramScene".into(),
                        comment: format!("No source was found by the name of `{scene}`."),
                    });
                }
                s.on_air = scene.to_owned();
                Ok(())
            })
        }
        fn stream(&mut self, on: bool) -> Result<(), ObsError> {
            self.with(|s| {
                s.streaming = on;
                Ok(())
            })
        }
        fn events(&mut self, _wait: Duration) -> Result<Vec<Event>, ObsError> {
            self.with(|s| Ok(std::mem::take(&mut s.events)))
        }
    }

    const QUICK: Timing = Timing {
        retry: Duration::from_millis(100),
        look: Duration::from_millis(50),
    };

    fn until(what: &str, mut ok: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ok() {
            assert!(Instant::now() < deadline, "never: {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("djmanzo-obs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// **Off until switched on, then connected, and saying what OBS does**;
    /// the DJ's asks carried out, and OBS's own words when it says no.
    #[test]
    fn the_link_connects_when_on_and_carries_out_the_djs_asks() {
        let obs = Fake::new();
        let connects = Arc::new(AtomicUsize::new(0));
        let link = Link::new();
        let (made, count) = (obs.clone(), Arc::clone(&connects));
        link.start(
            None,
            None,
            Box::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(made.clone()) as Box<dyn Studio>)
            }),
            QUICK,
        );
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(
            connects.load(Ordering::SeqCst),
            0,
            "off: never reached for OBS"
        );
        assert!(!link.status().connected);
        assert_eq!(
            link.ask(Ask::Stream(true)),
            Err("the link to OBS is off".to_owned())
        );

        link.set(Settings {
            on: true,
            ..Settings::default()
        });
        until("connected and looking", || {
            let s = link.status();
            s.connected && s.scenes.current == "Decks"
        });
        assert_eq!(link.status().scenes.all, ["Decks", "Crowd"]);

        link.ask(Ask::Stream(true)).unwrap();
        assert!(obs.state.lock().unwrap().streaming);
        until("the stream on the screen", || link.status().streaming);
        link.ask(Ask::Scene("Crowd".into())).unwrap();
        until("the scene on the screen", || {
            link.status().scenes.current == "Crowd"
        });
        let refused = link.ask(Ask::Scene("Nowhere".into())).unwrap_err();
        assert!(refused.contains("Nowhere"), "{refused}");
        assert!(link.status().connected, "a no from OBS is not OBS gone");

        // A scene put on air by hand in OBS reaches the screen by its event.
        {
            let mut state = obs.state.lock().unwrap();
            state.on_air = "Decks".into();
            state.events.push(Event::Scene("Decks".into()));
        }
        until("the hand's scene", || {
            link.status().scenes.current == "Decks"
        });

        link.set(Settings::default());
        until("off again", || {
            !link.status().on && !link.status().connected
        });
        assert_eq!(
            link.status().scenes,
            Scenes::default(),
            "nothing claimed while off"
        );
    }

    /// **OBS closed is OBS gone, and the link finds it again by itself** when
    /// it comes back — a DJ who restarts OBS mid-night does not have to come
    /// back to djmanzo to reconnect it.
    #[test]
    fn the_link_finds_obs_again_after_it_went_away() {
        let obs = Fake::new();
        let up = Arc::new(Mutex::new(true));
        let link = Link::new();
        let (made, is_up) = (obs.clone(), Arc::clone(&up));
        link.start(
            None,
            None,
            Box::new(move |settings| {
                if *is_up.lock().unwrap() {
                    made.state.lock().unwrap().gone = false;
                    Ok(Box::new(made.clone()) as Box<dyn Studio>)
                } else {
                    Err(ObsError::Unreachable(format!(
                        "{}:{}",
                        settings.host, settings.port
                    )))
                }
            }),
            QUICK,
        );
        link.set(Settings {
            on: true,
            ..Settings::default()
        });
        until("connected", || link.status().connected);

        *up.lock().unwrap() = false;
        obs.state.lock().unwrap().gone = true;
        until("gone", || !link.status().connected);
        let said = link.status().error.unwrap_or_default();
        assert!(
            said.contains("not answering") || said.contains("closed"),
            "{said}"
        );
        let asked = link.ask(Ask::Stream(true)).unwrap_err();
        assert!(!asked.is_empty());

        *up.lock().unwrap() = true;
        until("back by itself", || link.status().connected);
        assert_eq!(link.status().error, None, "the old complaint taken down");
    }

    /// **The settings survive a restart — the password in the keychain, as
    /// every other key djmanzo holds, and never in the file.**
    #[test]
    fn the_settings_are_kept_and_the_password_in_the_keychain() {
        let dir = dir("kept");
        let keychain: Arc<dyn SecretStore> = Arc::new(dj_secrets::MemoryStore::new());
        let link = Link::new();
        link.start(
            Some(&dir),
            Some(Arc::clone(&keychain)),
            Box::new(|_| Err(ObsError::Closed)),
            QUICK,
        );
        let mine = Settings {
            on: true,
            host: "127.0.0.1".into(),
            port: 4456,
            password: "hunter2".into(),
        };
        link.set(mine.clone());
        let file = std::fs::read_to_string(dir.join("obs.json")).unwrap();
        assert!(
            !file.contains("hunter2"),
            "the password written to disk: {file}"
        );
        assert_eq!(
            keychain.get(SecretKind::ObsPassword).unwrap().expose(),
            "hunter2"
        );

        let again = Link::new();
        again.start(
            Some(&dir),
            Some(Arc::clone(&keychain)),
            Box::new(|_| Err(ObsError::Closed)),
            QUICK,
        );
        assert_eq!(again.settings(), mine);

        // Cleared is cleared, in the keychain too.
        again.set(Settings {
            password: String::new(),
            ..mine
        });
        assert!(!keychain.has(SecretKind::ObsPassword));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

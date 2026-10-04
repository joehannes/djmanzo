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
    /// Which scene goes on air on which moment.
    pub moments: MomentScenes,
}

/// A moment in the night that can put a scene on air.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Moment {
    /// The drop of the record the room is hearing.
    Drop,
    /// The next singer called up in the karaoke rotation.
    Singer,
    /// The night ended — the event being played put away.
    End,
}

/// The scenes the DJ named for the moments, by OBS's names; an empty one is
/// a moment that changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MomentScenes {
    /// The scene to come back to once a moment's has held, or empty to stay.
    pub base: String,
    pub drop: String,
    pub singer: String,
    /// Not followed by the base: the night is over.
    pub end: String,
    /// How long a moment's scene holds before the base comes back, seconds.
    pub hold: u32,
}

impl Default for MomentScenes {
    fn default() -> Self {
        Self {
            base: String::new(),
            drop: String::new(),
            singer: String::new(),
            end: String::new(),
            hold: 8,
        }
    }
}

impl MomentScenes {
    fn scene(&self, moment: Moment) -> &str {
        match moment {
            Moment::Drop => &self.drop,
            Moment::Singer => &self.singer,
            Moment::End => &self.end,
        }
    }

    /// Whether `moment` has a scene to put on air.
    #[must_use]
    pub fn wants(&self, moment: Moment) -> bool {
        !self.scene(moment).trim().is_empty()
    }
}

/// How long djmanzo leaves the scenes alone after one was put on air by a
/// hand — in OBS, or with djmanzo's own scene buttons. A DJ who chose a
/// scene meant it; a drop a moment later does not argue.
pub const HANDS_OFF: Duration = Duration::from_secs(30);

/// Which scene the moments put on air, and when the base comes back.
///
/// Pure: told the moments, the time, and every scene OBS says went on air,
/// it answers with the scene to put on air, if any.
#[derive(Debug, Default)]
pub struct Director {
    /// When the base comes back.
    returning_at: Option<Instant>,
    /// Until when a hand's choice is left alone.
    hands_off_until: Option<Instant>,
    /// Scenes djmanzo put on air whose news has not come back from OBS yet,
    /// so its own switch is not taken for a hand's.
    expecting: Vec<String>,
}

impl Director {
    /// `moment` happened at `now`: the scene to put on air.
    pub fn moment(
        &mut self,
        moment: Moment,
        now: Instant,
        scenes: &MomentScenes,
    ) -> Option<String> {
        if self.hands_off_until.is_some_and(|until| now < until) {
            return None;
        }
        let scene = scenes.scene(moment).trim();
        if scene.is_empty() {
            return None;
        }
        self.returning_at = (moment != Moment::End && !scenes.base.trim().is_empty())
            .then(|| now + Duration::from_secs(u64::from(scenes.hold)));
        self.expect(scene);
        Some(scene.to_owned())
    }

    /// The base scene, once a moment's has held long enough.
    pub fn tick(&mut self, now: Instant, scenes: &MomentScenes) -> Option<String> {
        let due = self.returning_at.is_some_and(|at| now >= at);
        if !due {
            return None;
        }
        self.returning_at = None;
        let base = scenes.base.trim();
        if base.is_empty() {
            return None;
        }
        self.expect(base);
        Some(base.to_owned())
    }

    /// OBS says `scene` went on air: djmanzo's own doing, or a hand's.
    pub fn on_air(&mut self, scene: &str, now: Instant) {
        if let Some(ours) = self.expecting.iter().position(|s| s == scene) {
            self.expecting.remove(ours);
        } else {
            self.hand(now);
        }
    }

    /// A hand chose a scene: leave the scenes alone for a while.
    pub fn hand(&mut self, now: Instant) {
        self.hands_off_until = Some(now + HANDS_OFF);
        self.returning_at = None;
    }

    fn expect(&mut self, scene: &str) {
        self.expecting.push(scene.to_owned());
        // A switch whose news never came back is not waited for forever.
        if self.expecting.len() > 4 {
            self.expecting.remove(0);
        }
    }
}

/// Whether the record the room hears just reached one of its drops.
///
/// Told the deck the room hears, where it is and whether it plays, at every
/// look; a drop counts when the position moves forward across it in one
/// step of the music — not when a DJ jumps over it, nor on a deck that just
/// came to be the one heard.
#[derive(Debug, Default)]
pub struct DropWatch {
    last: Option<(u8, f64)>,
}

impl DropWatch {
    /// `deck` heard at `position` (the analysis's frames, `rate` a second),
    /// with its `drops`: true when one was just crossed.
    pub fn crossed(&mut self, heard: Option<(u8, f64, bool)>, drops: &[f64], rate: f64) -> bool {
        let Some((deck, position, playing)) = heard else {
            self.last = None;
            return false;
        };
        let before = self.last.replace((deck, position));
        let Some((was_deck, was)) = before else {
            return false;
        };
        playing
            && was_deck == deck
            && position > was
            && position - was <= rate
            && drops.iter().any(|&drop| was < drop && drop <= position)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on: false,
            host: "localhost".to_owned(),
            port: dj_net::obs::DEFAULT_PORT,
            password: String::new(),
            moments: MomentScenes::default(),
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
    Moment(Moment),
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
    drops: Mutex<DropWatch>,
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
            drops: Mutex::new(DropWatch::default()),
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

    /// A moment happened: the thread puts its scene on air, if the DJ named
    /// one and no hand chose a scene a moment ago. Not waited for.
    pub fn moment(&self, moment: Moment) {
        let settings = self.settings();
        if settings.on && settings.moments.wants(moment) {
            self.order(Order::Moment(moment));
        }
    }

    /// Whether the link would do anything with `moment` — so the pump does
    /// not read the analysis sixty times a second for nobody.
    #[must_use]
    pub fn wants(&self, moment: Moment) -> bool {
        let settings = self.settings();
        settings.on && settings.moments.wants(moment)
    }

    /// The pump's look at the record the room hears; puts the drop scene on
    /// air when it just crossed a drop.
    pub fn watch_drops(&self, heard: Option<(u8, f64, bool)>, drops: &[f64], rate: f64) {
        let crossed = self
            .drops
            .lock()
            .map(|mut watch| watch.crossed(heard, drops, rate))
            .unwrap_or(false);
        if crossed {
            self.moment(Moment::Drop);
        }
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
    let mut director = Director::default();
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
            Ok(Order::Moment(moment)) => {
                if let Some(obs) = studio.as_mut()
                    && let Some(scene) = director.moment(moment, Instant::now(), &settings.moments)
                    && let Err(error) = obs.put_on_air(&scene)
                {
                    if gone(&error) {
                        studio = None;
                    }
                    update(shared, |s| s.error = Some(error.to_string()));
                }
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
                            Ask::Scene(scene) => {
                                // The DJ's own choice, from djmanzo's buttons:
                                // a hand, as one in OBS is.
                                director.hand(Instant::now());
                                obs.put_on_air(scene)
                            }
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
        if let Some(base) = director.tick(Instant::now(), &settings.moments)
            && let Err(error) = obs.put_on_air(&base)
        {
            update(shared, |s| s.error = Some(error.to_string()));
        }
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
                            Event::Scene(scene) => {
                                director.on_air(&scene, Instant::now());
                                s.scenes.current = scene;
                            }
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

    fn named() -> MomentScenes {
        MomentScenes {
            base: "Decks".into(),
            drop: "Crowd".into(),
            singer: "Singer".into(),
            end: "Outro".into(),
            hold: 8,
        }
    }

    /// **A moment puts its scene on air, and the base comes back once it
    /// has held** — except at the night's end, which is not followed by
    /// anything; a moment with no scene named changes nothing.
    #[test]
    fn a_moment_puts_its_scene_on_air_and_the_base_comes_back() {
        let scenes = named();
        let start = Instant::now();
        let mut director = Director::default();
        assert_eq!(
            director.moment(Moment::Drop, start, &scenes).as_deref(),
            Some("Crowd")
        );
        director.on_air("Crowd", start);
        assert_eq!(
            director.tick(start + Duration::from_secs(7), &scenes),
            None,
            "still holding"
        );
        assert_eq!(
            director
                .tick(start + Duration::from_secs(8), &scenes)
                .as_deref(),
            Some("Decks")
        );
        director.on_air("Decks", start + Duration::from_secs(8));
        assert_eq!(
            director.tick(start + Duration::from_secs(20), &scenes),
            None,
            "once"
        );

        let later = start + Duration::from_secs(30);
        assert_eq!(
            director.moment(Moment::Singer, later, &scenes).as_deref(),
            Some("Singer")
        );
        director.on_air("Singer", later);
        assert_eq!(
            director.moment(Moment::End, later, &scenes).as_deref(),
            Some("Outro")
        );
        director.on_air("Outro", later);
        assert_eq!(
            director.tick(later + Duration::from_secs(60), &scenes),
            None,
            "the night is over"
        );

        let nothing = MomentScenes {
            drop: String::new(),
            ..named()
        };
        assert_eq!(
            Director::default().moment(Moment::Drop, later, &nothing),
            None
        );
        assert!(!nothing.wants(Moment::Drop));
        assert!(nothing.wants(Moment::Singer));
    }

    /// **A hand's scene is left alone.** One put on air in OBS — djmanzo's
    /// own switches told apart by the news coming back — or with djmanzo's
    /// own buttons stops the moments for a while, and cancels a base that
    /// was about to come back over it.
    #[test]
    fn a_hands_scene_is_left_alone() {
        let scenes = named();
        let start = Instant::now();
        let mut director = Director::default();
        assert!(director.moment(Moment::Drop, start, &scenes).is_some());
        director.on_air("Crowd", start);
        // The DJ puts the singer's camera up by hand in OBS.
        let hand = start + Duration::from_secs(2);
        director.on_air("Singer", hand);
        assert_eq!(
            director.tick(start + Duration::from_secs(9), &scenes),
            None,
            "not over the hand"
        );
        assert_eq!(
            director.moment(Moment::Drop, hand + Duration::from_secs(29), &scenes),
            None
        );
        assert_eq!(
            director
                .moment(Moment::Drop, hand + HANDS_OFF, &scenes)
                .as_deref(),
            Some("Crowd"),
            "and back to work after a while"
        );
    }

    /// **A drop counts when the music crosses it** — once, forward, on the
    /// deck the room hears, playing — not when the DJ jumps over it, not
    /// paused, not on the step where the room moved to another deck.
    #[test]
    fn a_drop_counts_when_the_music_crosses_it() {
        let rate = 44_100.0;
        let drops = [441_000.0]; // ten seconds in
        let mut watch = DropWatch::default();
        assert!(
            !watch.crossed(Some((1, 440_000.0, true)), &drops, rate),
            "a first look"
        );
        assert!(watch.crossed(Some((1, 441_500.0, true)), &drops, rate));
        assert!(
            !watch.crossed(Some((1, 442_000.0, true)), &drops, rate),
            "once"
        );

        let mut jumped = DropWatch::default();
        jumped.crossed(Some((1, 300_000.0, true)), &drops, rate);
        assert!(
            !jumped.crossed(Some((1, 600_000.0, true)), &drops, rate),
            "jumped over"
        );

        let mut paused = DropWatch::default();
        paused.crossed(Some((1, 440_000.0, false)), &drops, rate);
        assert!(!paused.crossed(Some((1, 441_500.0, false)), &drops, rate));

        let mut moved = DropWatch::default();
        moved.crossed(Some((2, 440_000.0, true)), &drops, rate);
        assert!(
            !moved.crossed(Some((1, 441_500.0, true)), &drops, rate),
            "another deck"
        );

        let mut silence = DropWatch::default();
        silence.crossed(Some((1, 440_000.0, true)), &drops, rate);
        silence.crossed(None, &drops, rate);
        assert!(
            !silence.crossed(Some((1, 441_500.0, true)), &drops, rate),
            "after silence"
        );
    }

    /// **Through the link: a moment reaches OBS, and the base follows** —
    /// and nothing at all while the link is off or the moment has no scene.
    #[test]
    fn the_link_puts_a_moments_scene_on_air() {
        let obs = Fake::new();
        let link = Link::new();
        let made = obs.clone();
        link.start(
            None,
            None,
            Box::new(move |_| Ok(Box::new(made.clone()) as Box<dyn Studio>)),
            QUICK,
        );
        let scenes = MomentScenes {
            base: "Decks".into(),
            drop: "Crowd".into(),
            singer: String::new(),
            end: String::new(),
            hold: 1,
        };
        link.moment(Moment::Drop);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(obs.state.lock().unwrap().on_air, "Decks", "off: nothing");

        link.set(Settings {
            on: true,
            moments: scenes,
            ..Settings::default()
        });
        until("connected", || link.status().connected);
        assert!(link.wants(Moment::Drop));
        assert!(!link.wants(Moment::Singer));
        link.moment(Moment::Singer);
        link.moment(Moment::Drop);
        until("the drop's scene", || {
            obs.state.lock().unwrap().on_air == "Crowd"
        });
        // OBS's news of djmanzo's own switch is not a hand.
        obs.state
            .lock()
            .unwrap()
            .events
            .push(Event::Scene("Crowd".into()));
        until("the base back", || {
            obs.state.lock().unwrap().on_air == "Decks"
        });
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
            moments: MomentScenes {
                drop: "Crowd".into(),
                ..MomentScenes::default()
            },
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

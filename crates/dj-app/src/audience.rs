//! The room's requests, as the application switches them on and off.
//!
//! [`dj_net::front`] holds the book, the page and the handler; this holds the
//! decision to open a port to a room full of strangers, and the shapes the
//! interface reads.
//!
//! Two rules that live here rather than there:
//!
//! - **Off unless asked.** Opening a port on a club's wifi is not a default,
//!   and nobody reads a changelog before a set.
//! - **Bound to every interface, on purpose.** A phone in the room cannot
//!   reach loopback, so this server has to face the network to be worth
//!   having. What makes that safe is [`dj_net::front::Doorman`], which can
//!   add to a book and do nothing else.

use dj_net::front::{Doorman, Front};
use dj_net::page::Words;
use dj_net::room::Standing;
use dj_net::sticker::{self, WayIn};
use dj_net::web::{Handler, WebServer};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// One request, as the interface shows it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AskDto {
    pub id: u64,
    pub text: String,
    pub voices: u32,
    /// Seconds since the epoch, like every other time djmanzo hands out.
    pub first_asked: u64,
    pub last_asked: u64,
    pub standing: String,
}

/// A way in, and its caveat.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WayInDto {
    pub kind: String,
    pub url: String,
    pub caveat: String,
    /// The QR code as an inline SVG, ready to drop into the panel.
    ///
    /// Drawn in Rust rather than in the webview because the same square goes
    /// onto the printed sheet, and two implementations of one code is one of
    /// them being subtly wrong in a way nobody notices until a phone in a bar
    /// will not read it.
    pub qr: Option<String>,
}

impl WayInDto {
    fn of(way_in: &WayIn) -> Self {
        Self {
            kind: kind_name(way_in.kind).to_owned(),
            url: way_in.url.clone(),
            caveat: way_in.kind.caveat().to_owned(),
            qr: sticker::qr_svg(&way_in.url).ok(),
        }
    }
}

fn kind_name(kind: sticker::Kind) -> &'static str {
    match kind {
        sticker::Kind::Name => "name",
        sticker::Kind::Lan => "lan",
    }
}

/// What the panel shows and sets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudienceStatus {
    pub running: bool,
    /// Whether requests are being taken. A closed door still shows the list.
    pub open: bool,
    pub port: u16,
    pub heading: String,
    pub language: String,
    pub show_playing: bool,
    /// Every way in, most portable first, so the order is the recommendation.
    pub ways_in: Vec<WayInDto>,
    pub announcing: bool,
    /// Why the local name is not being answered for, when it is not. A guest
    /// network blocking multicast is the common answer.
    pub announce_error: Option<String>,
    pub error: Option<String>,
    pub waiting: usize,
}

/// Owns the running server, if there is one.
#[derive(Debug, Default)]
pub struct Audience {
    front: Arc<Front>,
    server: Mutex<Option<WebServer>>,
    announcer: Mutex<Option<dj_net::announce::Announcer>>,
    announce_error: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
}

impl Audience {
    /// The book and its settings, whether or not a port is open.
    #[must_use]
    pub fn front(&self) -> &Arc<Front> {
        &self.front
    }

    /// Open the port, and start answering to the local name.
    ///
    /// # Errors
    /// When the port cannot be bound — usually another copy of djmanzo, or a
    /// port already spoken for.
    pub fn start(&self, port: u16) -> Result<AudienceStatus, String> {
        // Stopped first, so restarting on the same port does not fail to bind
        // against the copy of itself that is still listening.
        self.stop();

        let handler = Arc::new(Doorman::new(Arc::clone(&self.front))) as Arc<dyn Handler>;
        match WebServer::start(SocketAddr::from(([0, 0, 0, 0], port)), handler) {
            Ok(server) => {
                let port = server.address().port();
                *self.server.lock().unwrap() = Some(server);
                *self.error.lock().unwrap() = None;
                self.announce(port);
                Ok(self.status())
            }
            Err(why) => {
                let why = why.to_string();
                *self.error.lock().unwrap() = Some(why.clone());
                Err(why)
            }
        }
    }

    /// Start answering to the local name, or record why not.
    ///
    /// Never fatal. The server is already listening and the plain address
    /// already works; a network that blocks multicast costs the printable
    /// sticker, not the night.
    fn announce(&self, port: u16) {
        let Some(address) = sticker::lan_address() else {
            *self.announce_error.lock().unwrap() =
                Some("this machine has no address on a local network".to_owned());
            return;
        };
        match dj_net::announce::Announcer::start(address, port) {
            Ok(announcer) => {
                *self.announcer.lock().unwrap() = Some(announcer);
                *self.announce_error.lock().unwrap() = None;
            }
            Err(why) => {
                *self.announcer.lock().unwrap() = None;
                *self.announce_error.lock().unwrap() = Some(why.to_string());
            }
        }
    }

    /// Close the port. Stopping nothing is not an error.
    ///
    /// The book is kept: a DJ who closes the door still wants to read what was
    /// asked for while it was open.
    pub fn stop(&self) {
        *self.announcer.lock().unwrap() = None;
        *self.announce_error.lock().unwrap() = None;
        // Dropping the server joins its accept thread and closes the port.
        *self.server.lock().unwrap() = None;
    }

    #[must_use]
    pub fn status(&self) -> AudienceStatus {
        let server = self.server.lock().unwrap();
        let port = server.as_ref().map_or(0, |s| s.address().port());
        let running = server.is_some();
        drop(server);

        AudienceStatus {
            running,
            open: self.front.is_open(),
            port,
            heading: self.front.heading(),
            language: self.front.language(),
            show_playing: self.front.shows_playing(),
            ways_in: self.ways_in(running, port),
            announcing: self.announcer.lock().unwrap().is_some(),
            announce_error: self.announce_error.lock().unwrap().clone(),
            error: self.error.lock().unwrap().clone(),
            waiting: self.front.waiting_count(),
        }
    }

    fn ways_in(&self, running: bool, port: u16) -> Vec<WayInDto> {
        if !running {
            return Vec::new();
        }
        let mut ways = Vec::new();
        // The portable one first: it is the one that can be printed, and the
        // order the panel shows is the order of the recommendation. Only
        // offered when the name is actually being answered for -- a printed
        // address nothing responds to is worse than no offer at all.
        if self.announcer.lock().unwrap().is_some() {
            ways.push(WayInDto::of(&WayIn::name(port)));
        }
        if let Some(address) = sticker::lan_address() {
            ways.push(WayInDto::of(&WayIn::lan(address, port)));
        }
        ways
    }

    /// Everything still waiting, most-wanted first.
    #[must_use]
    pub fn waiting(&self) -> Vec<AskDto> {
        self.front.waiting().iter().map(dto).collect()
    }

    /// Everything, settled and not, in the order it was asked.
    #[must_use]
    pub fn everything(&self) -> Vec<AskDto> {
        self.front.everything().iter().map(dto).collect()
    }

    /// Say what became of one. `false` when there is no such ask.
    pub fn settle(&self, id: u64, standing: &str) -> bool {
        self.front.settle(id, standing_of(standing))
    }

    /// A printable sheet of stickers for one way in.
    ///
    /// # Errors
    /// When there is no such way in, or its URL cannot become a QR code.
    pub fn sheet(&self, kind: &str, copies: usize) -> Result<String, String> {
        let status = self.status();
        let found = status
            .ways_in
            .iter()
            .find(|w| w.kind == kind)
            .ok_or_else(|| format!("there is no {kind} address to print"))?;
        let way_in = WayIn {
            kind: if found.kind == "name" {
                sticker::Kind::Name
            } else {
                sticker::Kind::Lan
            },
            url: found.url.clone(),
        };
        let heading = self.front.heading();
        sticker::sheet(&sticker::Sticker {
            heading: &heading,
            // The page's own heading, in the room's language, so what is
            // printed and what is served say the same thing.
            call: self.front.words().title,
            way_in: &way_in,
            copies,
        })
    }

    /// The languages the page is written in, for the picker.
    #[must_use]
    pub fn languages() -> Vec<(String, String)> {
        Words::all()
            .into_iter()
            .map(|w| (w.tag.to_owned(), w.name.to_owned()))
            .collect()
    }
}

fn standing_of(name: &str) -> Standing {
    match name {
        "played" => Standing::Played,
        "passed" => Standing::Passed,
        _ => Standing::Waiting,
    }
}

fn dto(ask: &dj_net::room::Ask) -> AskDto {
    AskDto {
        id: ask.id,
        text: ask.text.clone(),
        voices: ask.voices,
        first_asked: seconds(ask.first_asked),
        last_asked: seconds(ask.last_asked),
        standing: match ask.standing {
            Standing::Waiting => "waiting",
            Standing::Played => "played",
            Standing::Passed => "passed",
        }
        .to_owned(),
    }
}

fn seconds(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Nothing is listening until somebody says so.**
    #[test]
    fn it_starts_off() {
        let audience = Audience::default();
        let status = audience.status();
        assert!(!status.running);
        assert!(status.ways_in.is_empty(), "a way in to nothing");
        assert_eq!(status.port, 0);
    }

    /// **Turning it on opens a port and offers a way to reach it.**
    ///
    /// Port 0 so the test does not fight the machine for 7331. What is checked
    /// is that the port is real, that the address offered is not the port-zero
    /// one, and that turning it off puts it back.
    #[test]
    fn starting_gives_a_real_port_and_a_way_in() {
        let audience = Audience::default();
        let status = audience.start(0).expect("start");
        assert!(status.running);
        assert_ne!(status.port, 0, "port 0 was reported as the answer");
        assert!(
            status
                .ways_in
                .iter()
                .all(|w| w.url.contains(&status.port.to_string())),
            "a way in that does not carry the port: {:?}",
            status.ways_in
        );
        // Whatever ways exist carry a QR, because a URL nobody can scan is a
        // URL nobody will type either.
        assert!(status.ways_in.iter().all(|w| w.qr.is_some()));

        let stopped = audience.stop_and_report();
        assert!(!stopped.running);
        assert!(stopped.ways_in.is_empty());
    }

    /// **Settling by name is the vocabulary the interface actually sends.**
    #[test]
    fn a_standing_arrives_as_a_word() {
        assert_eq!(standing_of("played"), Standing::Played);
        assert_eq!(standing_of("passed"), Standing::Passed);
        assert_eq!(standing_of("waiting"), Standing::Waiting);
        // Anything else is not an error the room should see -- it is the
        // interface sending something new, and waiting is the safe reading.
        assert_eq!(standing_of("nonsense"), Standing::Waiting);
    }

    /// **Every language offered by name can be chosen by tag.**
    #[test]
    fn the_language_picker_offers_what_the_page_speaks() {
        let offered = Audience::languages();
        assert!(offered.len() >= 2, "{offered:?}");
        let audience = Audience::default();
        for (tag, name) in offered {
            assert!(!name.trim().is_empty(), "{tag} has no name to show");
            audience.front().set_language(&tag);
            assert_eq!(audience.status().language, tag);
        }
    }

    /// **Printing asks for a way in that exists.**
    #[test]
    fn there_is_nothing_to_print_before_anything_runs() {
        let audience = Audience::default();
        assert!(audience.sheet("name", 12).is_err());
    }

    impl Audience {
        fn stop_and_report(&self) -> AudienceStatus {
            self.stop();
            self.status()
        }
    }
}

//! The three parts joined: the book, the page, and the socket.
//!
//! [`crate::room`] knows what the room asked for, [`crate::page`] knows how to
//! draw it, and [`crate::web`] knows how bytes become a request. This is the
//! only place that knows all three, and it is deliberately small — a match on
//! two paths and a redirect.
//!
//! It lives here rather than in the application for one reason: **this is the
//! path a stranger's phone actually takes**, and a path that can only be
//! exercised by starting a desktop application is a path nobody exercises.
//! Here it is a socket and a book, and a test can drive it end to end.
//!
//! # What a handler installed from here can do
//!
//! Add to a book. That is the whole list. [`Doorman`] holds an [`Arc<Front>`]
//! and nothing else — no action bus, no parameter registry, no deck. A request
//! from the room cannot reach the audio path because there is no path from
//! here to it, rather than because something checks.

use crate::page::{self, Line, Said, View, Words};
use crate::room::{Ask, RequestBook, Standing};
use crate::web::{Handler, Incoming, Method, Reply};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// The book, and everything the page says around it.
#[derive(Debug)]
pub struct Front {
    book: Mutex<RequestBook>,
    open: AtomicBool,
    heading: Mutex<String>,
    language: Mutex<String>,
    playing: Mutex<Option<String>>,
    show_playing: AtomicBool,
}

impl Default for Front {
    fn default() -> Self {
        Self {
            book: Mutex::new(RequestBook::new()),
            open: AtomicBool::new(true),
            heading: Mutex::new("Tonight".to_owned()),
            language: Mutex::new(Words::english().tag.to_owned()),
            playing: Mutex::new(None),
            show_playing: AtomicBool::new(true),
        }
    }
}

impl Front {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take requests, or stop taking them without taking the page away.
    ///
    /// Two different sentences to the room. A closed door still shows what was
    /// asked for, and the last hour of a night is exactly when a DJ wants to
    /// stop being asked without the page going dark.
    pub fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    pub fn set_heading(&self, heading: &str) {
        if let Ok(mut held) = self.heading.lock() {
            *held = heading.trim().to_owned();
        }
    }

    #[must_use]
    pub fn heading(&self) -> String {
        self.heading.lock().map(|h| h.clone()).unwrap_or_default()
    }

    /// Set the room's language by tag.
    ///
    /// Through [`Words::by_tag`] so an unknown tag becomes English once, here,
    /// rather than on every page render.
    pub fn set_language(&self, tag: &str) {
        if let Ok(mut held) = self.language.lock() {
            *held = Words::by_tag(tag).tag.to_owned();
        }
    }

    #[must_use]
    pub fn language(&self) -> String {
        self.language.lock().map(|l| l.clone()).unwrap_or_default()
    }

    #[must_use]
    pub fn words(&self) -> Words {
        Words::by_tag(&self.language())
    }

    /// Whether the room is told what is on the decks.
    ///
    /// A real trade rather than a setting nobody touches: the track name stops
    /// people asking for the record that is already playing, and it also lets
    /// anyone at the bar write down the set as it happens.
    pub fn set_show_playing(&self, show: bool) {
        self.show_playing.store(show, Ordering::Relaxed);
    }

    #[must_use]
    pub fn shows_playing(&self) -> bool {
        self.show_playing.load(Ordering::Relaxed)
    }

    pub fn set_playing(&self, playing: Option<String>) {
        if let Ok(mut held) = self.playing.lock() {
            *held = playing;
        }
    }

    #[must_use]
    fn playing(&self) -> Option<String> {
        if !self.shows_playing() {
            return None;
        }
        self.playing.lock().ok()?.clone()
    }

    /// Read the book. `None` when it is poisoned, which is a thread that
    /// panicked while holding it and not something to panic about again.
    pub fn read<T>(&self, look: impl FnOnce(&RequestBook) -> T) -> Option<T> {
        self.book.lock().ok().map(|book| look(&book))
    }

    /// Everything still waiting, most-wanted first.
    #[must_use]
    pub fn waiting(&self) -> Vec<Ask> {
        self.read(|book| book.waiting().into_iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Everything, settled and not, in the order it was asked.
    #[must_use]
    pub fn everything(&self) -> Vec<Ask> {
        self.read(|book| book.all().to_vec()).unwrap_or_default()
    }

    #[must_use]
    pub fn waiting_count(&self) -> usize {
        self.read(|book| book.waiting().len()).unwrap_or(0)
    }

    /// Say what became of one. `false` when there is no such ask.
    pub fn settle(&self, id: u64, standing: Standing) -> bool {
        self.book
            .lock()
            .map(|mut book| book.settle(id, standing))
            .unwrap_or(false)
    }

    /// Mark the request a track answers, if the room asked for it.
    ///
    /// Called when a track starts, so a DJ who happened to play what the room
    /// wanted does not also have to notice and tick it off.
    pub fn played(&self, text: &str) -> Option<u64> {
        let mut book = self.book.lock().ok()?;
        let id = book.matching(text)?.id;
        book.settle(id, Standing::Played);
        Some(id)
    }
}

/// The only thing a served page can do.
#[derive(Debug)]
pub struct Doorman {
    front: Arc<Front>,
}

impl Doorman {
    #[must_use]
    pub fn new(front: Arc<Front>) -> Self {
        Self { front }
    }

    /// Which phone this is, as far as the limits are concerned.
    ///
    /// The address without its port. A venue that puts every phone behind one
    /// address turns the per-phone limit into a room-wide one; that is a real
    /// limitation of counting this way, and the alternative — a cookie — is
    /// gone the moment somebody opens the page in a private tab.
    fn phone(request: &Incoming) -> String {
        request
            .from
            .map_or_else(|| "unknown".to_owned(), |address| address.to_string())
    }

    fn show(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let heading = self.front.heading();
        let playing = self.front.playing();
        let lines = self
            .front
            .read(|book| Line::from_asks(&book.waiting(), &phone, page::ROWS))
            .unwrap_or_default();

        Reply::html(page::request_page(&View {
            words: self.front.words(),
            heading: &heading,
            playing: playing.as_deref(),
            lines,
            said: request.param("said").and_then(Said::by_tag),
            open: self.front.is_open(),
        }))
    }

    fn take(&self, request: &Incoming) -> Reply {
        if !self.front.is_open() {
            // No message. The page it lands on already says requests are shut,
            // and a refusal on top of that is telling somebody off for using a
            // form that should not have been on their screen.
            return Reply::see_other("/");
        }

        let phone = Self::phone(request);
        let now = SystemTime::now();
        let Ok(mut book) = self.front.book.lock() else {
            return Reply::see_other("/");
        };
        let outcome = match request
            .field("second")
            .and_then(|id| id.parse::<u64>().ok())
        {
            Some(id) => book.second(id, &phone, now),
            None => book.ask(&phone, &request.field("song").unwrap_or_default(), now),
        };
        drop(book);

        // A redirect rather than a page, so a phone refreshing to check
        // whether the song went through does not send it a second time --
        // which is a duplicate the DJ then has to read past.
        Reply::see_other(format!("/?said={}", Said::of(outcome).tag()))
    }
}

impl Handler for Doorman {
    fn handle(&self, request: &Incoming) -> Reply {
        match (request.method, request.path.as_str()) {
            (Method::Get, "/") => self.show(request),
            (Method::Post, "/request") => self.take(request),
            _ => Reply::not_found(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::WebServer;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    fn serving() -> (WebServer, Arc<Front>) {
        let front = Arc::new(Front::new());
        let handler = Arc::new(Doorman::new(Arc::clone(&front))) as Arc<dyn Handler>;
        let server =
            WebServer::start(SocketAddr::from(([127, 0, 0, 1], 0)), handler).expect("start");
        (server, front)
    }

    fn talk(address: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(address).expect("connect");
        stream.write_all(request.as_bytes()).expect("write");
        stream.flush().expect("flush");
        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        got
    }

    fn get(address: SocketAddr, path: &str) -> String {
        talk(
            address,
            &format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"),
        )
    }

    fn post(address: SocketAddr, body: &str) -> String {
        talk(
            address,
            &format!(
                "POST /request HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    /// **A phone types a song and the DJ can read it.**
    ///
    /// The whole feature in one test, over a real socket: the form posts, the
    /// answer is a redirect so a refresh cannot double it, the page that
    /// follows says it landed, and the book the panel reads has it.
    #[test]
    fn a_song_typed_on_a_phone_reaches_the_dj() {
        let (server, front) = serving();
        let sent = post(server.address(), "song=Obsesi%C3%B3n+de+Aventura");
        assert!(sent.starts_with("HTTP/1.1 303"), "{sent}");
        assert!(
            sent.to_lowercase().contains("location: /?said=ok"),
            "{sent}"
        );

        let page = get(server.address(), "/?said=ok");
        assert!(page.contains(Words::english().said_added), "{page}");
        assert!(page.contains("Obsesión de Aventura"), "{page}");

        let waiting = front.waiting();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].text, "Obsesión de Aventura");
        assert_eq!(waiting[0].voices, 1);
    }

    /// **The same phone asking twice is told so, and counted once.**
    #[test]
    fn a_second_ask_from_one_phone_does_not_double_the_tally() {
        let (server, front) = serving();
        post(server.address(), "song=Suavemente");
        let again = post(server.address(), "song=suavemente");
        assert!(
            again.to_lowercase().contains("location: /?said=yours"),
            "{again}"
        );
        assert_eq!(front.waiting()[0].voices, 1);
    }

    /// **A closed door takes the form away and refuses what gets through.**
    ///
    /// The form is gone from the page, but a phone with the page already open
    /// can still post — so the refusal is on the server too, and not only in
    /// what the page offers.
    #[test]
    fn a_closed_front_refuses_a_post_it_never_offered() {
        let (server, front) = serving();
        front.set_open(false);

        let page = get(server.address(), "/");
        assert!(!page.contains("<form"), "{page}");
        assert!(page.contains(Words::english().closed), "{page}");

        let sent = post(server.address(), "song=Suavemente");
        assert!(sent.starts_with("HTTP/1.1 303"), "{sent}");
        assert!(!sent.contains("said="), "it took a request anyway: {sent}");
        assert!(front.waiting().is_empty(), "it took a request anyway");
    }

    /// **The room's language is the page's language.**
    #[test]
    fn the_page_is_served_in_the_rooms_language() {
        let (server, front) = serving();
        front.set_language("es");
        let page = get(server.address(), "/");
        assert!(page.contains("lang=\"es\""), "{page}");
        assert!(page.contains(Words::spanish().field_label), "{page}");
    }

    /// **What is playing is shown, or withheld, as the DJ chose.**
    #[test]
    fn the_room_is_told_what_is_playing_only_when_that_is_wanted() {
        let (server, front) = serving();
        front.set_playing(Some("Aventura - Obsesión".to_owned()));
        assert!(get(server.address(), "/").contains("Aventura - Obsesión"));

        front.set_show_playing(false);
        let hidden = get(server.address(), "/");
        assert!(!hidden.contains("Aventura"), "the set leaked: {hidden}");
    }

    /// **A track starting to play ticks off the request that wanted it.**
    #[test]
    fn playing_the_song_settles_the_ask() {
        let (server, front) = serving();
        post(server.address(), "song=Obsesi%C3%B3n");
        assert_eq!(front.waiting_count(), 1);

        let settled = front.played("Aventura - Obsesión (Album Version)");
        assert!(settled.is_some());
        assert_eq!(front.waiting_count(), 0);
        assert_eq!(front.everything().len(), 1, "the night's record was lost");
    }

    /// **Nothing served here can reach anything but the book.**
    ///
    /// Not a behaviour so much as the shape of the thing, checked because the
    /// shape is the security: a path this server does not know is a 404, not a
    /// guess. If a later handler grows a route to something with a deck behind
    /// it, this is where that shows up.
    ///
    /// Both verbs, because they are two different mistakes. A `GET` that fell
    /// through to the page would leak the list from an address nobody meant to
    /// serve; a `POST` that fell through to [`Doorman::take`] would make every
    /// path on the server a request box, and the first thing that finds that
    /// is a crawler filling the book with URLs.
    #[test]
    fn there_is_nothing_else_to_reach() {
        let (server, front) = serving();
        // `/request` is the form's own target, so it is a 404 to a `GET` and
        // not to a `POST`; it is checked on its own below.
        for path in ["/deck/1/play", "/api/state", "/../../etc/passwd"] {
            let got = get(server.address(), path);
            assert!(
                got.starts_with("HTTP/1.1 404"),
                "GET {path} answered something: {got}"
            );

            let posted = talk(
                server.address(),
                &format!(
                    "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 14\r\nConnection: close\r\n\r\nsong=Suavemente",
                ),
            );
            assert!(
                posted.starts_with("HTTP/1.1 404"),
                "POST {path} answered something: {posted}"
            );
        }
        // The form's own path, asked for rather than submitted to. Nothing to
        // see: the list lives on `/`, and a second address that also serves it
        // is a second address to have to think about.
        assert!(
            get(server.address(), "/request").starts_with("HTTP/1.1 404"),
            "the form's target answered a GET"
        );
        assert!(
            front.everything().is_empty(),
            "a path that is not the form put something in the book"
        );
    }
}

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

use crate::page::{self, Line, PhotoSaid, Said, Sang, SingView, View, Words};
use crate::quiz::{Answered, LETTERS, Named, QuizBook};
use crate::quiz_page::{PhoneQuestion, QuizSaid, QuizView};
use crate::room::{Ask, RequestBook, Standing};
use crate::stage::{Posed, Proposal, Signed, Stage, Voice};
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
    /// K4: the second book — singers putting their own names down.
    stage: Mutex<Stage>,
    /// K7: the third — answers to the quiz.
    quiz: Mutex<QuizBook>,
    /// Karaoke beta B2: emoji from the room, for the singers' screen.
    reactions: Mutex<crate::reactions::Reactions>,
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
            stage: Mutex::new(Stage::new()),
            quiz: Mutex::new(QuizBook::new()),
            reactions: Mutex::new(crate::reactions::Reactions::new()),
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

    /// K4: read or change the singers' book. `None` when it is poisoned.
    pub fn stage<T>(&self, change: impl FnOnce(&mut Stage) -> T) -> Option<T> {
        self.stage.lock().ok().map(|mut stage| change(&mut stage))
    }

    /// Karaoke beta B2: read or change the room's emoji. `None` when poisoned.
    pub fn reactions<T>(
        &self,
        change: impl FnOnce(&mut crate::reactions::Reactions) -> T,
    ) -> Option<T> {
        self.reactions.lock().ok().map(|mut book| change(&mut book))
    }

    /// K7: read or change the quiz's book. `None` when it is poisoned.
    pub fn quiz<T>(&self, change: impl FnOnce(&mut QuizBook) -> T) -> Option<T> {
        self.quiz.lock().ok().map(|mut quiz| change(&mut quiz))
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
            sing: self.sing_view(request, &phone),
            quiz: self.front.quiz(|quiz| quiz.is_on()).unwrap_or(false),
            react: self
                .front
                .reactions(|book| book.is_on())
                .unwrap_or(false)
                .then(|| crate::reactions::Row {
                    reacted: request
                        .param("reacted")
                        .and_then(crate::reactions::Reacted::by_tag),
                }),
        }))
    }

    /// K7: the quiz as this phone plays it.
    fn quiz_show(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let heading = self.front.heading();
        let tag = self.front.language();
        let now = SystemTime::now();
        let view = self.front.quiz(|quiz| QuizView {
            words: crate::quiz_page::QuizWords::by_tag(&tag),
            tag: &tag,
            heading: &heading,
            on: quiz.is_on(),
            nickname: quiz.nickname(&phone).map(str::to_owned),
            points: quiz.points_of(&phone),
            question: quiz.question().map(|asked| PhoneQuestion {
                id: asked.id,
                answers: asked.answers.clone(),
                prompt: asked.prompt.clone(),
                number: asked.number,
                of: asked.of,
                open: asked.open_at(now),
                chosen: asked.chosen_by(&phone),
                right: asked.right(),
            }),
            // Between questions: while one is open, the board would be
            // read instead of the question.
            leaders: if quiz.question().is_none_or(|asked| asked.right().is_some()) {
                quiz.leaders()
            } else {
                Vec::new()
            },
            said: request.param("said").and_then(QuizSaid::by_tag),
        });
        match view {
            Some(view) => Reply::html(crate::quiz_page::quiz_page(&view)),
            None => Reply::see_other("/"),
        }
    }

    /// K7: a nickname for the night.
    fn quiz_name(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let nickname = request.field("nickname").unwrap_or_default();
        let named = self
            .front
            .quiz(|quiz| quiz.name(&phone, &nickname))
            .unwrap_or(Named::Empty);
        Reply::see_other(format!("/quiz?said={}", QuizSaid::Named(named).tag()))
    }

    /// K7: an answer, by the question it was given to and its letter.
    fn quiz_answer(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let id = request
            .field("question")
            .and_then(|id| id.trim().parse::<u64>().ok());
        let letter = request
            .field("letter")
            .and_then(|letter| LETTERS.iter().position(|l| *l == letter.trim()));
        let answered = match (id, letter) {
            (Some(id), Some(letter)) => self
                .front
                .quiz(|quiz| quiz.answer(&phone, id, letter, SystemTime::now()))
                .unwrap_or(Answered::Closed),
            _ => Answered::Closed,
        };
        Reply::see_other(format!("/quiz?said={}", QuizSaid::Answered(answered).tag()))
    }

    /// K4: the *Sing* part for this phone — shown while the host takes names,
    /// and to a phone with a singer down tonight, so it can still see its
    /// place after the host stops taking more.
    fn sing_view(&self, request: &Incoming, phone: &str) -> Option<SingView> {
        self.front
            .stage(|stage| {
                let rules = stage.rules();
                let name = stage.name_of(phone).map(str::to_owned);
                if !rules.open && name.is_none() {
                    return None;
                }
                let place = stage.place(phone);
                let query = request
                    .param("q")
                    .map(str::trim)
                    .filter(|q| !q.is_empty())
                    .map(str::to_owned);
                Some(SingView {
                    open: rules.open,
                    more: stage.may_add(phone),
                    name,
                    place,
                    songbook: rules.songbook,
                    typed_name: request.param("name").unwrap_or_default().to_owned(),
                    matches: query
                        .as_deref()
                        .map(|q| stage.search(q))
                        .unwrap_or_default(),
                    query,
                    said: request.param("sang").and_then(Sang::by_tag),
                    key: rules.key,
                    chosen_key: request.param("key").and_then(|k| k.parse().ok()),
                    guide: rules.guide,
                    chosen_voice: request.param("voice").and_then(Voice::by_tag),
                    unsure: request.param("unsure").is_some(),
                    photos: rules.photos,
                    photo: stage.photo_of_phone(phone).map(|photo| photo.approved),
                })
            })
            .flatten()
    }

    /// K4: put a name down to sing.
    fn sing(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let name = request.field("name").unwrap_or_default();
        let token = request.field("song");
        let written = request.field("written");
        let proposal = Proposal {
            key: request.field("key").and_then(|k| k.trim().parse().ok()),
            voice: request.field("voice").as_deref().and_then(Voice::by_tag),
        };
        let unsure = request.field("unsure").is_some();
        let signed = self
            .front
            .stage(|stage| {
                let signed = stage.sign_with(
                    &phone,
                    &name,
                    token.as_deref(),
                    written.as_deref(),
                    proposal,
                    SystemTime::now(),
                );
                if signed == Signed::Added && unsure {
                    stage.unsure_of_last(&phone);
                }
                signed
            })
            .unwrap_or(Signed::Closed);
        // The name goes back with a refusal, so it need not be typed again.
        let again = if signed == Signed::Added {
            String::new()
        } else {
            format!("&name={}", urlencoding::encode(name.trim()))
        };
        Reply::see_other(format!("/?sang={}{again}#sing", Sang::Signed(signed).tag()))
    }

    /// K4: a photo of this phone's singer, drawn again before it is kept
    /// ([`crate::photo::redraw`]): the bytes as sent never reach the book.
    fn photo(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let said = match request.part("photo").map(crate::photo::redraw) {
            None | Some(Err(crate::photo::Refused::NotAPicture)) => PhotoSaid::NotAPicture,
            Some(Err(crate::photo::Refused::TooLarge)) => PhotoSaid::TooLarge,
            Some(Ok(jpeg)) => match self.front.stage(|stage| stage.photo(&phone, jpeg)) {
                Some(Posed::Added) => PhotoSaid::Sent,
                Some(Posed::Full) => PhotoSaid::TooLarge,
                _ => PhotoSaid::Closed,
            },
        };
        Reply::see_other(format!("/?sang={}#sing", Sang::Photo(said).tag()))
    }

    /// K4: the singer up next says they are ready.
    fn ready(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let ready = self
            .front
            .stage(|stage| stage.ready(&phone))
            .unwrap_or(false);
        Reply::see_other(if ready { "/?sang=ready#sing" } else { "/#sing" })
    }

    /// Karaoke beta B2: a tap on one of the room's emoji.
    fn react(&self, request: &Incoming) -> Reply {
        let phone = Self::phone(request);
        let emoji = request
            .field("emoji")
            .and_then(|index| index.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        let reacted = self
            .front
            .reactions(|book| book.react(&phone, emoji, SystemTime::now()))
            .unwrap_or(crate::reactions::Reacted::Off);
        Reply::see_other(format!("/?reacted={}", reacted.tag()))
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
    /// K4: a photo is the one thing larger than a form, and only at `/photo`.
    fn most_body(&self, path: &str) -> usize {
        if path == "/photo" {
            crate::photo::MOST_PHOTO + crate::web::MOST_BODY
        } else {
            crate::web::MOST_BODY
        }
    }

    fn handle(&self, request: &Incoming) -> Reply {
        match (request.method, request.path.as_str()) {
            (Method::Get, "/") => self.show(request),
            (Method::Post, "/request") => self.take(request),
            (Method::Post, "/react") => self.react(request),
            (Method::Post, "/sing") => self.sing(request),
            (Method::Post, "/ready") => self.ready(request),
            (Method::Post, "/photo") => self.photo(request),
            (Method::Get, "/quiz") => self.quiz_show(request),
            (Method::Post, "/quiz/name") => self.quiz_name(request),
            (Method::Post, "/quiz/answer") => self.quiz_answer(request),
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

    /// Karaoke beta B2: **a tap on the room's page reaches the screen's book
    /// only while the host has them on**, over a real socket — the row is not
    /// on the page while off, a tap then is refused, and once on the row is
    /// there and a tap is taken, a second at once told to wait.
    #[test]
    fn a_tap_from_the_room_reaches_the_book_only_when_on() {
        let (server, front) = serving();
        assert!(!get(server.address(), "/").contains("action=\"/react\""));
        let off = post_to(server.address(), "/react", "emoji=0");
        assert!(off.contains("Location: /?reacted=off"), "{off}");

        front.reactions(|book| book.set_on(true));
        assert!(get(server.address(), "/").contains("action=\"/react\""));
        let sent = post_to(server.address(), "/react", "emoji=2");
        assert!(sent.contains("Location: /?reacted=sent"), "{sent}");
        let soon = post_to(server.address(), "/react", "emoji=2");
        assert!(soon.contains("Location: /?reacted=soon"), "{soon}");
        let taken = front
            .reactions(|book| book.since(0, std::time::SystemTime::now()))
            .expect("the book");
        assert_eq!(taken.iter().map(|r| r.emoji).collect::<Vec<_>>(), [2]);
        assert!(get(server.address(), "/?reacted=soon").contains("a couple of seconds"));
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

    fn post_to(address: SocketAddr, path: &str, body: &str) -> String {
        talk(
            address,
            &format!(
                "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    fn singing(front: &Front) {
        front.stage(|stage| {
            stage.set_rules(crate::stage::Rules {
                open: true,
                ..crate::stage::Rules::default()
            });
            stage.set_songbook(vec![
                crate::stage::BookSong {
                    track: "00ff".into(),
                    title: "Obsesión".into(),
                    artist: "Aventura".into(),
                },
                crate::stage::BookSong {
                    track: "00aa".into(),
                    title: "Suavemente".into(),
                    artist: "Elvis Crespo".into(),
                },
            ]);
        });
    }

    /// The token a page offers for its first match, read out of the page as
    /// a phone's browser would submit it.
    fn first_token(page: &str) -> String {
        let at = page
            .find("name=\"song\" value=\"")
            .expect("a song to choose")
            + 19;
        page[at..].split('"').next().unwrap().to_owned()
    }

    /// **Nothing about singing is on the page until the host opens it.**
    #[test]
    fn singing_is_not_offered_until_the_host_opens_it() {
        let (server, front) = serving();
        let page = get(server.address(), "/");
        assert!(!page.contains("id=\"sing\""), "{page}");
        let sent = post_to(server.address(), "/sing", "name=Ana&written=Suavemente");
        assert!(
            sent.to_lowercase().contains("location: /?sang=shut"),
            "{sent}"
        );
        assert!(front.stage(|s| s.take().0).unwrap().is_empty());
    }

    /// **"I'm not sure of this one", from the phone**: the box is on the
    /// form, carried from a search to the song chosen from it, and the
    /// sign-up says so — and one sent without it does not. Not a proposal:
    /// kept whatever the host lets phones propose.
    #[test]
    fn a_guest_can_say_they_are_not_sure_of_their_song() {
        let (server, front) = serving();
        singing(&front);
        let plain = get(server.address(), "/?name=Ana&q=obsesion");
        assert!(plain.contains("name=\"unsure\""), "{plain}");
        assert!(plain.contains("not sure of this one"), "{plain}");
        assert!(!plain.contains("value=\"1\" checked"), "{plain}");
        assert!(
            !plain.contains("<input type=\"hidden\" name=\"unsure\""),
            "carried without being asked: {plain}"
        );

        let found = get(server.address(), "/?name=Ana&q=obsesion&unsure=1");
        assert!(
            found.contains("name=\"unsure\" value=\"1\" checked"),
            "{found}"
        );
        assert!(
            found.contains("<input type=\"hidden\" name=\"unsure\" value=\"1\">"),
            "the song chosen from the search did not carry it: {found}"
        );
        let token = first_token(&found);
        post_to(
            server.address(),
            "/sing",
            &format!("name=Ana&song={}&unsure=1", urlencoding::encode(&token)),
        );
        let (signed, _) = front.stage(|s| s.take()).unwrap();
        assert_eq!(signed.len(), 1);
        assert!(signed[0].unsure, "the phone's word was lost");
        assert!(signed[0].proposal.is_empty(), "it is not a proposal");
    }

    /// **A guest finds their song, puts their name down, and the host's
    /// rotation can take it** — over a real socket, as a phone does: the
    /// search answers with forms, the chosen one posts its token, the answer
    /// is a redirect, and the page after it says where they stand.
    #[test]
    fn a_guest_puts_their_name_down_from_the_page() {
        let (server, front) = serving();
        singing(&front);
        let found = get(server.address(), "/?name=Ana&q=obsesion");
        assert!(found.contains("Obsesión"), "{found}");
        assert!(!found.contains("Suavemente"), "only what matched");
        assert!(
            !found.contains("00ff"),
            "the record's own name never reaches a phone"
        );
        let token = first_token(&found);

        let sent = post_to(
            server.address(),
            "/sing",
            &format!("name=Ana&song={}", urlencoding::encode(&token)),
        );
        assert!(sent.starts_with("HTTP/1.1 303"), "{sent}");
        assert!(
            sent.to_lowercase().contains("location: /?sang=on#sing"),
            "{sent}"
        );

        let after = get(server.address(), "/?sang=on");
        assert!(after.contains("You're on the list."), "{after}");
        assert!(after.contains("You&#39;re down as Ana."), "{after}");
        assert!(after.contains("The DJ is adding you."), "{after}");

        let (signed, _) = front.stage(|s| s.take()).unwrap();
        assert_eq!(signed.len(), 1);
        assert_eq!(signed[0].name, "Ana");
        assert!(!signed[0].unsure, "not sure, without being said");
        assert_eq!(
            signed[0].song,
            crate::stage::Chosen::Book(crate::stage::BookSong {
                track: "00ff".into(),
                title: "Obsesión".into(),
                artist: "Aventura".into(),
            })
        );
    }

    /// **A name typed as a script comes back as text**, on the page of the
    /// phone that typed it and in the search it is carried through.
    #[test]
    fn a_singers_name_cannot_become_markup() {
        let (server, front) = serving();
        singing(&front);
        let script = "<script>alert(1)</script>";
        let found = get(
            server.address(),
            &format!("/?name={}&q=suave", urlencoding::encode(script)),
        );
        assert!(!found.contains(script), "{found}");
        let token = first_token(&found);
        post_to(
            server.address(),
            "/sing",
            &format!(
                "name={}&song={}",
                urlencoding::encode(script),
                urlencoding::encode(&token)
            ),
        );
        let page = get(server.address(), "/");
        assert!(!page.contains("<script>"), "{page}");
        assert!(page.contains("&lt;script&gt;"), "{page}");
    }

    /// **The singer up next says they are ready, from their own phone.**
    #[test]
    fn the_singer_called_presses_ready() {
        let (server, front) = serving();
        singing(&front);
        let token = first_token(&get(server.address(), "/?name=Ana&q=suave"));
        post_to(
            server.address(),
            "/sing",
            &format!("name=Ana&song={}", urlencoding::encode(&token)),
        );
        front.stage(|s| {
            s.take();
            s.set_order(vec![("Bob".into(), 1), ("Ana".into(), 1)], vec![]);
        });
        let waiting = get(server.address(), "/");
        assert!(waiting.contains("1 singer before you."), "{waiting}");
        assert!(!waiting.contains("action=\"/ready\""), "not up yet");

        front.stage(|s| s.set_order(vec![("Ana".into(), 1)], vec![]));
        let up = get(server.address(), "/");
        assert!(up.contains("You're up next."), "{up}");
        assert!(up.contains("action=\"/ready\""), "{up}");
        let pressed = post_to(server.address(), "/ready", "");
        assert!(
            pressed.to_lowercase().contains("location: /?sang=ready"),
            "{pressed}"
        );
        assert_eq!(front.stage(|s| s.take().1).unwrap(), vec!["Ana".to_owned()]);
        let after = get(server.address(), "/?sang=ready");
        assert!(after.contains("The DJ knows you're ready."), "{after}");
    }

    /// **A key and the record's singer are asked for only where the host
    /// allows it**, carried from the search to the song chosen, and arrive
    /// in the book as a proposal.
    #[test]
    fn a_guest_asks_for_a_key_and_a_guide_where_the_host_allows_it() {
        let (server, front) = serving();
        singing(&front);
        let plain = get(server.address(), "/?name=Ana&q=obsesion");
        assert!(!plain.contains("name=\"key\""), "not allowed, not offered");
        assert!(!plain.contains("name=\"voice\""));

        front.stage(|s| {
            s.set_rules(crate::stage::Rules {
                open: true,
                key: true,
                guide: true,
                ..crate::stage::Rules::default()
            });
        });
        let found = get(server.address(), "/?name=Ana&q=obsesion&key=-2&voice=guide");
        assert!(
            found.contains("<option value=\"-2\" selected>2 semitones lower</option>"),
            "{found}"
        );
        assert!(
            found.contains("<option value=\"guide\" selected>Quietly, as a guide</option>"),
            "{found}"
        );
        assert!(
            found.contains("The DJ sees what you ask for and decides."),
            "{found}"
        );
        assert!(
            found.contains("name=\"key\" value=\"-2\""),
            "carried to the song: {found}"
        );
        let token = first_token(&found);
        post_to(
            server.address(),
            "/sing",
            &format!(
                "name=Ana&song={}&key=-2&voice=guide",
                urlencoding::encode(&token)
            ),
        );
        let (signed, _) = front.stage(|s| s.take()).unwrap();
        assert_eq!(
            signed[0].proposal,
            crate::stage::Proposal {
                key: Some(-2),
                voice: Some(crate::stage::Voice::Guide),
            }
        );
    }

    fn multipart(address: SocketAddr, file: &[u8]) -> String {
        let mut body = b"--K4\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"me.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
        body.extend_from_slice(file);
        body.extend_from_slice(b"\r\n--K4--\r\n");
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        use std::io::{Read, Write};
        stream
            .write_all(
                format!(
                    "POST /photo HTTP/1.1\r\nHost: x\r\nContent-Type: multipart/form-data; boundary=K4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .expect("head");
        stream.write_all(&body).expect("body");
        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        got
    }

    /// **A guest's photo reaches the host drawn again, never as sent**, over
    /// a real socket and a form with a file in it — far past the size of a
    /// form; and only from a phone with a singer down, where the host takes
    /// photos, and only if it is a picture.
    #[test]
    fn a_guests_photo_reaches_the_host_drawn_again() {
        let (server, front) = serving();
        singing(&front);
        // Noise, which a PNG cannot squeeze: a photo-sized body.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let picture = image::RgbImage::from_fn(1600, 900, |_, _| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let [r, g, b, ..] = seed.to_le_bytes();
            image::Rgb([r, g, b])
        });
        let mut png = Vec::new();
        image::ImageEncoder::write_image(
            image::codecs::png::PngEncoder::new(&mut png),
            picture.as_raw(),
            1600,
            900,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
        assert!(png.len() > crate::web::MOST_BODY, "larger than any form");

        let shut = multipart(server.address(), &png);
        assert!(
            shut.to_lowercase().contains("location: /?sang=nophotos"),
            "{shut}"
        );

        let token = first_token(&get(server.address(), "/?name=Ana&q=suave"));
        post_to(
            server.address(),
            "/sing",
            &format!("name=Ana&song={}", urlencoding::encode(&token)),
        );
        front.stage(|s| {
            s.set_rules(crate::stage::Rules {
                open: true,
                photos: true,
                ..crate::stage::Rules::default()
            });
        });
        let page = get(server.address(), "/");
        assert!(page.contains("enctype=\"multipart/form-data\""), "{page}");
        assert!(
            page.contains("if the DJ approves it"),
            "said before the button: {page}"
        );

        let not = multipart(server.address(), b"<script>alert(1)</script>");
        assert!(
            not.to_lowercase().contains("location: /?sang=notphoto"),
            "{not}"
        );
        let sent = multipart(server.address(), &png);
        assert!(
            sent.to_lowercase().contains("location: /?sang=photo#sing"),
            "{sent}"
        );

        let kept = front.stage(|s| s.photos().to_vec()).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "Ana");
        assert!(!kept[0].approved);
        assert_eq!(
            &kept[0].jpeg[..2],
            &[0xFF, 0xD8],
            "a JPEG drawn again, not the PNG sent"
        );
        let size = image::load_from_memory(&kept[0].jpeg).unwrap();
        assert_eq!((size.width(), size.height()), (1280, 720));
        assert!(get(server.address(), "/").contains("Your photo is waiting for the DJ."));
    }

    /// **A refusal keeps the name**, so it need not be typed again.
    #[test]
    fn a_refused_sign_up_says_why_and_keeps_the_name() {
        let (server, front) = serving();
        singing(&front);
        let sent = post_to(server.address(), "/sing", "name=Ana&song=ff-9");
        let location = sent.to_lowercase();
        assert!(
            location.contains("location: /?sang=changed&name=ana#sing"),
            "{sent}"
        );
        let page = get(server.address(), "/?sang=changed&name=Ana");
        assert!(
            page.contains("The songbook changed. Search again."),
            "{page}"
        );
        assert!(page.contains("value=\"Ana\""), "{page}");
    }

    /// **K7: a phone plays a question over a real socket** — the room's
    /// page points to the quiz while a round runs; a nickname first; four
    /// buttons; one answer; the answer shown and the points with it. A
    /// second phone, by the book, answers wrong.
    #[test]
    fn a_phone_plays_a_quiz_question() {
        let (server, front) = serving();
        assert!(
            !get(server.address(), "/").contains("href=\"/quiz\""),
            "no round, no link"
        );
        let answers =
            ["Obsesión — Aventura", "Suavemente — Elvis Crespo", "C", "D"].map(str::to_owned);
        let id = front
            .quiz(|quiz| quiz.ask(answers, 1, (1, 5), None, SystemTime::now()))
            .unwrap();
        let room = get(server.address(), "/");
        assert!(room.contains("href=\"/quiz\""));
        assert!(room.contains(".play{"), "the invitation unstyled");

        let first = get(server.address(), "/quiz");
        assert!(first.contains("action=\"/quiz/name\""), "{first}");
        assert!(!first.contains("/quiz/answer"), "answers before a nickname");
        let named = post_to(server.address(), "/quiz/name", "nickname=Ana");
        assert!(named.contains("/quiz?said=in"), "{named}");

        let page = get(server.address(), "/quiz");
        assert_eq!(page.matches("action=\"/quiz/answer\"").count(), 4, "{page}");
        assert!(page.contains("Suavemente — Elvis Crespo"));
        let answer = format!("question={id}&letter=B");
        assert!(post_to(server.address(), "/quiz/answer", &answer).contains("said=got"));
        assert!(post_to(server.address(), "/quiz/answer", &answer).contains("said=again"));
        assert!(get(server.address(), "/quiz").contains("You answered B."));

        front.quiz(|quiz| {
            quiz.name("phone-b", "Bo");
            quiz.answer("phone-b", id, 0, SystemTime::now())
        });
        front.quiz(|quiz| quiz.reveal(SystemTime::now()));
        let shown = get(server.address(), "/quiz");
        assert!(shown.contains("It was B:"), "{shown}");
        assert!(shown.contains("You were right!"));
        assert!(shown.contains("<span class=\"what\">Ana</span>"), "{shown}");
        assert!(!shown.contains(">Bo<"), "nothing scored, not on the board");
        let late = post_to(
            server.address(),
            "/quiz/answer",
            &format!("question={id}&letter=A"),
        );
        assert!(late.contains("said=late"), "{late}");

        front.quiz(|quiz| quiz.end_round(SystemTime::now()));
        assert!(!get(server.address(), "/").contains("href=\"/quiz\""));
        assert!(get(server.address(), "/quiz").contains("No quiz right now."));
    }
}

//! §119: a phone as a camera, from djmanzo's own page.
//!
//! The phone opens a page djmanzo serves on the local network
//! (`https://192.168.1.20:8443/p/<key>/`, from a QR code), lends it the
//! camera, and sends the picture straight to a second page OBS shows as a
//! browser source (`http://127.0.0.1:8443/v/<key>/`). The two are joined by
//! WebRTC, phone to OBS. djmanzo carries only their introduction
//! ([`Introduction`]): the phone's offer to OBS, OBS's answer back. Not a
//! frame of the picture passes through it, and nothing leaves the network.
//!
//! # Who can do what
//!
//! - **The phone's side** is reached in TLS only ([`crate::secure`]), and
//!   only with the phone's key in the path. The QR code is the one place it
//!   is shown, and the DJ can replace it.
//! - **OBS's side** answers this computer only, and only with its own key,
//!   so a new phone link does not break the browser source. With a key on
//!   that side too, no other website open on this computer can post an
//!   answer that would send the phone's picture somewhere else.
//! - A wrong key is told nothing exists, not that the key was wrong.
//!
//! # The introduction
//!
//! The phone offers and OBS answers, each in one piece once its network
//! candidates are gathered. On one network they are all there within a
//! second, and one request each way is a protocol a test can hold. Each side
//! waits for the other with a held request ([`HOLD`]). A new offer replaces
//! the last, so a phone that reloads, or a second phone, takes over, and OBS
//! follows it.

use crate::web::{Handler, Incoming, Method, Reply};
use serde::Serialize;
use std::net::IpAddr;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The port asked for first.
///
/// The usual alternative port for HTTPS, so the address reads as what it is.
pub const DEFAULT_PORT: u16 = 8443;

/// How long a request waiting for the other side is held before it is
/// answered with nothing, and asked again.
pub const HOLD: Duration = Duration::from_secs(20);

/// How long since a page last asked anything before it counts as gone. Longer
/// than [`HOLD`], so a page waiting quietly is still there.
pub const SEEN_FOR: Duration = Duration::from_secs(45);

const PHONE_PAGE: &str = include_str!("phone/phone.html");
const VIEW_PAGE: &str = include_str!("phone/view.html");

/// The two keys, each the one path segment after `/p/` or `/v/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    pub phone: String,
    pub viewer: String,
}

/// What the phone's page is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhoneNow {
    /// Not open, or not heard from in [`SEEN_FOR`].
    Away,
    /// Open, and not sending.
    Open,
    /// Sending its picture to OBS.
    Sending,
}

/// What OBS's page is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerNow {
    Away,
    /// Open, waiting for a phone.
    Open,
    /// Showing a phone's picture.
    Showing,
}

/// What each side is doing, for djmanzo to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Seen {
    pub phone: PhoneNow,
    pub viewer: ViewerNow,
}

/// What a held request came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Waited<T> {
    Got(T),
    /// Nothing within the hold: ask again.
    Nothing,
    /// The offer it was for has been replaced.
    Gone,
}

/// What OBS is told next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Answer this offer.
    Show { id: u64, sdp: String },
    /// Let the picture go: the phone stopped, or the link was replaced.
    Blank { id: u64 },
}

/// The phone's offer and OBS's answer, and who has been heard from.
#[derive(Debug)]
pub struct Introduction {
    talk: Mutex<Talk>,
    changed: Condvar,
    hold: Duration,
}

#[derive(Debug, Default)]
struct Talk {
    /// The number the next offer gets. From 1, so OBS can ask for anything
    /// after 0.
    last: u64,
    offer: Option<(u64, String)>,
    answer: Option<(u64, String)>,
    /// The number of the last stop: OBS showing anything from before it lets
    /// the picture go.
    stopped: u64,
    phone_seen: Option<Instant>,
    viewer_seen: Option<Instant>,
    sending: bool,
    showing: bool,
}

impl Introduction {
    /// With requests held for `hold` at most.
    #[must_use]
    pub fn new(hold: Duration) -> Self {
        Self {
            talk: Mutex::new(Talk::default()),
            changed: Condvar::new(),
            hold,
        }
    }

    fn talk(&self) -> std::sync::MutexGuard<'_, Talk> {
        // A thread that panicked while holding this left plain data behind,
        // and the next phone should still be introduced.
        self.talk
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The phone's offer, replacing any earlier one. Returns its number.
    pub fn offer(&self, sdp: String) -> u64 {
        let mut talk = self.talk();
        talk.last += 1;
        let id = talk.last;
        talk.offer = Some((id, sdp));
        talk.answer = None;
        talk.sending = false;
        talk.phone_seen = Some(Instant::now());
        drop(talk);
        self.changed.notify_all();
        id
    }

    /// OBS's answer to offer `id`.
    ///
    /// # Errors
    /// [`Waited::Gone`] when that offer has been replaced, or already
    /// answered — the first page to answer is the one the phone talks to.
    pub fn answer(&self, id: u64, sdp: String) -> Result<(), Waited<()>> {
        let mut talk = self.talk();
        talk.viewer_seen = Some(Instant::now());
        if talk.offer.as_ref().map(|(n, _)| *n) != Some(id) || talk.answer.is_some() {
            return Err(Waited::Gone);
        }
        talk.answer = Some((id, sdp));
        drop(talk);
        self.changed.notify_all();
        Ok(())
    }

    /// For the phone: wait for the answer to offer `id`.
    pub fn answer_to(&self, id: u64) -> Waited<String> {
        let until = Instant::now() + self.hold;
        let mut talk = self.talk();
        loop {
            talk.phone_seen = Some(Instant::now());
            if talk.offer.as_ref().map(|(n, _)| *n) != Some(id) {
                return Waited::Gone;
            }
            if let Some((answered, sdp)) = &talk.answer
                && *answered == id
            {
                return Waited::Got(sdp.clone());
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Waited::Nothing;
            }
            talk = self
                .changed
                .wait_timeout(talk, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// For OBS: wait for anything newer than `after` — an offer to answer,
    /// or a stop.
    pub fn offer_after(&self, after: u64) -> Waited<Next> {
        let until = Instant::now() + self.hold;
        let mut talk = self.talk();
        loop {
            talk.viewer_seen = Some(Instant::now());
            if let Some((id, sdp)) = &talk.offer
                && *id > after
            {
                return Waited::Got(Next::Show {
                    id: *id,
                    sdp: sdp.clone(),
                });
            }
            if talk.stopped > after {
                return Waited::Got(Next::Blank { id: talk.stopped });
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Waited::Nothing;
            }
            talk = self
                .changed
                .wait_timeout(talk, left)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Stop whatever is being shown: OBS lets the picture go, and the phone
    /// that was sending is told its offer is gone. For a link replaced, so
    /// that whoever had the old one is cut off at once.
    pub fn stop(&self) {
        let mut talk = self.talk();
        Self::stop_in(&mut talk);
        drop(talk);
        self.changed.notify_all();
    }

    fn stop_in(talk: &mut Talk) {
        talk.last += 1;
        talk.stopped = talk.last;
        talk.offer = None;
        talk.answer = None;
        talk.sending = false;
    }

    /// A phone opened the page. Only that: a second phone opening it does not
    /// stop the first one sending.
    pub fn phone_opened(&self) {
        self.talk().phone_seen = Some(Instant::now());
    }

    /// The phone that made offer `id` says whether it is sending. One that
    /// stops takes its offer with it, so OBS lets its picture go and does
    /// not answer a page that has gone. Said about any other offer, it
    /// changes nothing: a second phone stopping does not stop the first.
    pub fn phone_says(&self, id: u64, sending: bool) {
        let mut talk = self.talk();
        talk.phone_seen = Some(Instant::now());
        if talk.offer.as_ref().map(|(n, _)| *n) != Some(id) {
            return;
        }
        if sending {
            talk.sending = true;
        } else {
            Self::stop_in(&mut talk);
            drop(talk);
            self.changed.notify_all();
        }
    }

    /// OBS says whether it is showing a picture.
    pub fn viewer_says(&self, showing: bool) {
        let mut talk = self.talk();
        talk.viewer_seen = Some(Instant::now());
        talk.showing = showing;
    }

    /// What each side is doing now.
    #[must_use]
    pub fn seen(&self) -> Seen {
        let talk = self.talk();
        let recent = |at: Option<Instant>| at.is_some_and(|at| at.elapsed() < SEEN_FOR);
        Seen {
            phone: match (recent(talk.phone_seen), talk.sending) {
                (false, _) => PhoneNow::Away,
                (true, false) => PhoneNow::Open,
                (true, true) => PhoneNow::Sending,
            },
            viewer: match (recent(talk.viewer_seen), talk.showing) {
                (false, _) => ViewerNow::Away,
                (true, false) => ViewerNow::Open,
                (true, true) => ViewerNow::Showing,
            },
        }
    }
}

/// Whether `given` is `key`, taking as long for a near miss as for a far one.
fn same_key(given: &str, key: &str) -> bool {
    given.len() == key.len()
        && given
            .bytes()
            .zip(key.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// What is after `/<side>/<key>` in `path`, when the key is the right one:
/// `""` for the page itself, `"offer"` and so on for the rest.
fn under<'a>(path: &'a str, side: &str, key: &str) -> Option<&'a str> {
    let rest = path
        .strip_prefix('/')?
        .strip_prefix(side)?
        .strip_prefix('/')?;
    let (given, after) = rest.split_once('/').unwrap_or((rest, ""));
    (!key.is_empty() && same_key(given, key)).then_some(after)
}

fn sdp_of(request: &Incoming) -> Option<String> {
    let sdp = String::from_utf8(request.body.clone()).ok()?;
    // An SDP starts with its version line; anything else is not one.
    sdp.starts_with("v=0").then_some(sdp)
}

fn id_of(request: &Incoming, name: &str) -> Option<u64> {
    request.param(name)?.parse().ok()
}

fn waited_reply(waited: Waited<String>) -> Reply {
    match waited {
        Waited::Got(sdp) => Reply {
            status: 200,
            content_type: "application/sdp",
            headers: Vec::new(),
            body: sdp.into_bytes(),
        },
        Waited::Nothing => Reply::text(204, ""),
        Waited::Gone => Reply::text(410, "replaced"),
    }
}

/// The phone camera's two sides, for [`crate::secure::SecureServer`].
#[derive(Debug)]
pub struct PhoneCamera {
    pub introduction: Arc<Introduction>,
    keys: Mutex<Keys>,
    authority: Vec<u8>,
    fingerprint: String,
}

impl PhoneCamera {
    /// With `keys`, and the authority's certificate (`authority`, DER) and
    /// `fingerprint` for the phone page to offer and show.
    #[must_use]
    pub fn new(keys: Keys, authority: Vec<u8>, fingerprint: String, hold: Duration) -> Self {
        Self {
            introduction: Arc::new(Introduction::new(hold)),
            keys: Mutex::new(keys),
            authority,
            fingerprint,
        }
    }

    fn keys(&self) -> Keys {
        self.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// A new key for the phone's address. The old one is nothing from now
    /// on, and whatever it was sending is stopped.
    pub fn replace_phone_key(&self, key: String) {
        self.keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .phone = key;
        self.introduction.stop();
    }

    /// What arrives in TLS: the phone.
    #[must_use]
    pub fn phone(&self, request: &Incoming) -> Reply {
        let Some(after) = under(&request.path, "p", &self.keys().phone) else {
            return Reply::not_found();
        };
        let intro = &self.introduction;
        match (request.method, after) {
            (Method::Get, "") => {
                intro.phone_opened();
                Reply::html(PHONE_PAGE.replace("{{FINGERPRINT}}", &self.fingerprint))
            }
            (Method::Post, "offer") => match sdp_of(request) {
                Some(sdp) => Reply::json(format!("{{\"id\":{}}}", intro.offer(sdp))),
                None => Reply::text(400, "not an offer"),
            },
            (Method::Get, "answer") => match id_of(request, "id") {
                Some(id) => waited_reply(intro.answer_to(id)),
                None => Reply::text(400, "which offer"),
            },
            (Method::Post, "state") => {
                let sending = request.param("now") == Some("sending");
                if let Some(id) = id_of(request, "id") {
                    intro.phone_says(id, sending);
                } else {
                    intro.phone_opened();
                }
                Reply::text(200, "noted")
            }
            (Method::Get, "djmanzo.crt") => {
                let mut reply = Reply {
                    status: 200,
                    content_type: "application/x-x509-ca-cert",
                    headers: Vec::new(),
                    body: self.authority.clone(),
                };
                reply.headers.push((
                    "Content-Disposition".to_owned(),
                    "attachment; filename=\"djmanzo.crt\"".to_owned(),
                ));
                reply
            }
            _ => Reply::not_found(),
        }
    }

    /// What arrives in plain HTTP: OBS, on this computer. From anywhere
    /// else, a page saying to use the code.
    #[must_use]
    pub fn viewer(&self, request: &Incoming) -> Reply {
        if !request.from.is_some_and(local) {
            return Reply::html(USE_THE_CODE);
        }
        let Some(after) = under(&request.path, "v", &self.keys().viewer) else {
            return Reply::not_found();
        };
        let intro = &self.introduction;
        match (request.method, after) {
            (Method::Get, "") => {
                intro.viewer_says(false);
                Reply::html(VIEW_PAGE)
            }
            (Method::Get, "offer") => match intro.offer_after(id_of(request, "after").unwrap_or(0))
            {
                Waited::Got(Next::Show { id, sdp }) => {
                    Reply::json(serde_json::json!({ "id": id, "sdp": sdp }).to_string())
                }
                Waited::Got(Next::Blank { id }) => {
                    Reply::json(serde_json::json!({ "id": id, "sdp": null }).to_string())
                }
                Waited::Nothing => Reply::text(204, ""),
                Waited::Gone => Reply::text(410, "replaced"),
            },
            (Method::Post, "answer") => match (id_of(request, "id"), sdp_of(request)) {
                (Some(id), Some(sdp)) => match intro.answer(id, sdp) {
                    Ok(()) => Reply::text(200, "answered"),
                    Err(_) => Reply::text(410, "replaced"),
                },
                _ => Reply::text(400, "not an answer"),
            },
            (Method::Post, "state") => {
                intro.viewer_says(request.param("now") == Some("showing"));
                Reply::text(200, "noted")
            }
            _ => Reply::not_found(),
        }
    }

    /// Both sides as handlers.
    #[must_use]
    pub fn handlers(self: &Arc<Self>) -> (Arc<dyn Handler>, Arc<dyn Handler>) {
        let phone = Arc::clone(self);
        let viewer = Arc::clone(self);
        (
            Arc::new(move |r: &Incoming| phone.phone(r)),
            Arc::new(move |r: &Incoming| viewer.viewer(r)),
        )
    }
}

/// The phone camera, serving.
///
/// Dropping it closes the port.
#[derive(Debug)]
pub struct Serving {
    server: crate::secure::SecureServer,
    pub camera: Arc<PhoneCamera>,
}

impl Serving {
    #[must_use]
    pub fn address(&self) -> std::net::SocketAddr {
        self.server.address()
    }

    #[must_use]
    pub fn seen(&self) -> Seen {
        self.camera.introduction.seen()
    }
}

/// Serve the phone camera on `address` with `keys`, its certificates from
/// `authority`.
///
/// A certificate is made the first time a phone reaches each of this
/// computer's addresses, and kept for the run: one handshake is enough to
/// pay for a signature.
///
/// # Errors
/// When the address cannot be bound.
pub fn serve(
    address: std::net::SocketAddr,
    authority: Arc<crate::authority::Authority>,
    keys: Keys,
) -> Result<Serving, crate::secure::SecureError> {
    let camera = Arc::new(PhoneCamera::new(
        keys,
        authority.certificate().to_vec(),
        authority.fingerprint(),
        HOLD,
    ));
    let made: Mutex<std::collections::HashMap<IpAddr, Arc<rustls::ServerConfig>>> =
        Mutex::default();
    let certify = move |local: IpAddr| {
        let mut made = made
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(config) = made.get(&local) {
            return Some(Arc::clone(config));
        }
        // A computer has a handful of addresses. More than this is someone
        // reaching it by ways that are not addresses of its own.
        if made.len() >= 16 {
            made.clear();
        }
        match authority
            .issue(local)
            .and_then(crate::authority::Leaf::server_config)
        {
            Ok(config) => {
                made.insert(local, Arc::clone(&config));
                Some(config)
            }
            Err(why) => {
                tracing::warn!(%why, %local, "no certificate for the phone camera");
                None
            }
        }
    };
    let (phone, viewer) = camera.handlers();
    let server = crate::secure::SecureServer::start(address, Arc::new(certify), phone, viewer)?;
    Ok(Serving { server, camera })
}

/// This computer: what OBS's side answers.
fn local(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

/// What plain HTTP from the network is told. Nothing about either key.
const USE_THE_CODE: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>djmanzo</title></head><body style=\"font:16px system-ui,sans-serif;margin:2rem;\
max-width:30rem\"><h1 style=\"font-size:1.3rem\">djmanzo's phone camera</h1>\
<p>Open it with the code djmanzo shows. Its address starts with <b>https://</b>.</p>\
</body></html>";

#[cfg(test)]
mod tests {
    use super::*;

    const OFFER: &str = "v=0\r\no=- 1 2 IN IP4 127.0.0.1\r\n";
    const ANSWER: &str = "v=0\r\no=- 3 4 IN IP4 127.0.0.1\r\n";

    fn camera(hold: Duration) -> Arc<PhoneCamera> {
        Arc::new(PhoneCamera::new(
            Keys {
                phone: "phonekey".to_owned(),
                viewer: "viewkey".to_owned(),
            },
            b"the authority".to_vec(),
            "AB:CD".to_owned(),
            hold,
        ))
    }

    fn ask(method: Method, target: &str, body: &str, from: &str) -> Incoming {
        let (path, query) = crate::web::split_query(target);
        Incoming {
            method,
            path,
            query,
            body: body.as_bytes().to_vec(),
            from: Some(from.parse().unwrap()),
            range: None,
        }
    }

    const PHONE: &str = "192.168.1.30";
    const HERE: &str = "127.0.0.1";

    /// **The phone's offer reaches OBS, and OBS's answer the phone**, each
    /// through a held request that is let go the moment the other side
    /// speaks.
    #[test]
    fn an_offer_and_its_answer_cross() {
        let camera = camera(Duration::from_secs(5));
        let obs = {
            let camera = Arc::clone(&camera);
            std::thread::spawn(move || {
                let got = camera.viewer(&ask(Method::Get, "/v/viewkey/offer?after=0", "", HERE));
                assert_eq!(got.status, 200);
                let offered: serde_json::Value = serde_json::from_slice(&got.body).unwrap();
                assert_eq!(offered["sdp"], OFFER);
                let id = offered["id"].as_u64().unwrap();
                let answered = camera.viewer(&ask(
                    Method::Post,
                    &format!("/v/viewkey/answer?id={id}"),
                    ANSWER,
                    HERE,
                ));
                assert_eq!(answered.status, 200);
            })
        };
        // OBS is already waiting when the phone offers.
        std::thread::sleep(Duration::from_millis(100));
        let offered = camera.phone(&ask(Method::Post, "/p/phonekey/offer", OFFER, PHONE));
        assert_eq!(offered.status, 200);
        let id: serde_json::Value = serde_json::from_slice(&offered.body).unwrap();
        let started = Instant::now();
        let answer = camera.phone(&ask(
            Method::Get,
            &format!("/p/phonekey/answer?id={}", id["id"]),
            "",
            PHONE,
        ));
        obs.join().unwrap();
        assert_eq!(answer.status, 200);
        assert_eq!(answer.content_type, "application/sdp");
        assert_eq!(String::from_utf8(answer.body).unwrap(), ANSWER);
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "held to the end"
        );
    }

    /// **A new offer replaces the old**: the phone waiting on the old one is
    /// told it is gone, OBS asking for anything newer gets the new one, and
    /// an answer to the old one is refused. The first answer to an offer is
    /// the one kept.
    #[test]
    fn a_new_offer_takes_over() {
        let camera = camera(Duration::from_millis(300));
        let intro = &camera.introduction;
        let first = intro.offer(OFFER.to_owned());
        let second = intro.offer("v=0\r\nsecond".to_owned());
        assert_eq!(intro.answer_to(first), Waited::Gone);
        assert_eq!(
            intro.offer_after(first),
            Waited::Got(Next::Show {
                id: second,
                sdp: "v=0\r\nsecond".to_owned()
            })
        );
        assert_eq!(intro.offer_after(second), Waited::Nothing);
        assert!(intro.answer(first, ANSWER.to_owned()).is_err());
        assert!(intro.answer(second, ANSWER.to_owned()).is_ok());
        assert!(intro.answer(second, "v=0\r\nlate".to_owned()).is_err());
        assert_eq!(intro.answer_to(second), Waited::Got(ANSWER.to_owned()));

        // A phone that reloads after being answered: its new offer waits for
        // an answer of its own, and gets one.
        let third = intro.offer("v=0\r\nthird".to_owned());
        assert_eq!(intro.answer_to(third), Waited::Nothing);
        assert!(
            intro
                .answer(third, "v=0\r\nanswer three".to_owned())
                .is_ok()
        );
        assert_eq!(
            intro.answer_to(third),
            Waited::Got("v=0\r\nanswer three".to_owned())
        );
    }

    /// Nothing within the hold is an empty answer, to be asked again — not
    /// an error, and not a wait past the hold.
    #[test]
    fn a_held_request_lets_go_at_the_hold() {
        let camera = camera(Duration::from_millis(200));
        let id = camera.introduction.offer(OFFER.to_owned());
        let started = Instant::now();
        let got = camera.phone(&ask(
            Method::Get,
            &format!("/p/phonekey/answer?id={id}"),
            "",
            PHONE,
        ));
        assert_eq!(got.status, 204);
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// **Without the right key there is nothing here**, on either side, and
    /// OBS's side answers this computer only — from the network, even with
    /// its key, it says to use the code and nothing more.
    #[test]
    fn keys_and_places_are_kept() {
        let camera = camera(Duration::from_millis(50));
        for wrong in [
            "/p/phonekez/",
            "/p/phonekeyy/",
            "/p//",
            "/p/",
            "/p",
            "/v/phonekey/",
            "/",
        ] {
            assert_eq!(
                camera.phone(&ask(Method::Get, wrong, "", PHONE)).status,
                404,
                "{wrong}"
            );
        }
        for wrong in ["/v/viewkez/", "/v/phonekey/", "/p/viewkey/", "/"] {
            assert_eq!(
                camera.viewer(&ask(Method::Get, wrong, "", HERE)).status,
                404,
                "{wrong}"
            );
        }
        // OBS's page, with its key, asked for from a phone.
        let from_room = camera.viewer(&ask(Method::Get, "/v/viewkey/", "", PHONE));
        let page = String::from_utf8(from_room.body).unwrap();
        assert!(page.contains("Open it with the code"));
        assert!(!page.contains("viewkey") && !page.contains("phonekey"));
        // An answer posted from the room is not taken.
        let id = camera.introduction.offer(OFFER.to_owned());
        let posted = camera.viewer(&ask(
            Method::Post,
            &format!("/v/viewkey/answer?id={id}"),
            ANSWER,
            PHONE,
        ));
        assert_eq!(posted.status, 200, "the use-the-code page");
        assert_eq!(camera.introduction.answer_to(id), Waited::Nothing);
        // The pages themselves.
        let phone_page = camera.phone(&ask(Method::Get, "/p/phonekey/", "", PHONE));
        assert!(
            String::from_utf8(phone_page.body)
                .unwrap()
                .contains("AB:CD")
        );
        let view_page = camera.viewer(&ask(Method::Get, "/v/viewkey", "", "::1"));
        assert_eq!(view_page.status, 200);
        assert!(
            String::from_utf8(view_page.body)
                .unwrap()
                .contains("<video")
        );
    }

    /// What is not an SDP is not passed on to the other side.
    #[test]
    fn only_an_sdp_is_passed_on() {
        let camera = camera(Duration::from_millis(50));
        let got = camera.phone(&ask(Method::Post, "/p/phonekey/offer", "<script>", PHONE));
        assert_eq!(got.status, 400);
        assert_eq!(camera.introduction.offer_after(0), Waited::Nothing);
    }

    /// **What each side is doing is said as each says it**, and a phone that
    /// stops takes its offer with it.
    #[test]
    fn each_side_is_seen() {
        let camera = camera(Duration::from_millis(50));
        let intro = &camera.introduction;
        assert_eq!(
            intro.seen(),
            Seen {
                phone: PhoneNow::Away,
                viewer: ViewerNow::Away
            }
        );
        assert_eq!(
            camera
                .viewer(&ask(Method::Get, "/v/viewkey/", "", HERE))
                .status,
            200
        );
        assert_eq!(
            camera
                .phone(&ask(Method::Get, "/p/phonekey/", "", PHONE))
                .status,
            200
        );
        assert_eq!(
            intro.seen(),
            Seen {
                phone: PhoneNow::Open,
                viewer: ViewerNow::Open
            }
        );
        let id = intro.offer(OFFER.to_owned());
        let said = |now: &str| format!("/p/phonekey/state?now={now}&id={id}");
        assert_eq!(
            camera
                .phone(&ask(Method::Post, &said("sending"), "", PHONE))
                .status,
            200
        );
        assert_eq!(
            camera
                .viewer(&ask(Method::Post, "/v/viewkey/state?now=showing", "", HERE))
                .status,
            200
        );
        assert_eq!(
            intro.seen(),
            Seen {
                phone: PhoneNow::Sending,
                viewer: ViewerNow::Showing
            }
        );
        assert_eq!(
            camera
                .phone(&ask(Method::Post, &said("stopped"), "", PHONE))
                .status,
            200
        );
        assert_eq!(intro.seen().phone, PhoneNow::Open);
        assert_eq!(intro.answer_to(id), Waited::Gone);
        // OBS, which had answered `id`, is told to let the picture go.
        let Waited::Got(Next::Blank { id: stop }) = intro.offer_after(id) else {
            panic!("OBS was not told the phone stopped");
        };
        assert_eq!(intro.offer_after(stop), Waited::Nothing);
    }

    /// **A phone stopping stops only its own offer**: a second phone that
    /// opens the page and stops changes nothing for the one sending.
    #[test]
    fn a_second_phone_stopping_does_not_stop_the_first() {
        let camera = camera(Duration::from_millis(50));
        let intro = &camera.introduction;
        let first = intro.offer(OFFER.to_owned());
        assert_eq!(
            camera
                .phone(&ask(
                    Method::Post,
                    &format!("/p/phonekey/state?now=sending&id={first}"),
                    "",
                    PHONE,
                ))
                .status,
            200
        );
        // The second phone, with no offer of its own, or a stale one.
        assert_eq!(
            camera
                .phone(&ask(
                    Method::Post,
                    "/p/phonekey/state?now=stopped",
                    "",
                    "192.168.1.31",
                ))
                .status,
            200
        );
        assert_eq!(
            camera
                .phone(&ask(
                    Method::Post,
                    &format!("/p/phonekey/state?now=stopped&id={}", first + 7),
                    "",
                    "192.168.1.31",
                ))
                .status,
            200
        );
        assert_eq!(intro.seen().phone, PhoneNow::Sending);
        assert_eq!(
            intro.offer_after(0),
            Waited::Got(Next::Show {
                id: first,
                sdp: OFFER.to_owned()
            })
        );
    }

    /// **A new phone link cuts off the old one at once**: its address is
    /// nothing from then on, the phone waiting on it is told its offer is
    /// gone, and OBS lets go of the picture it was showing.
    #[test]
    fn a_new_phone_link_cuts_off_the_old() {
        let camera = camera(Duration::from_millis(50));
        let intro = &camera.introduction;
        let id = intro.offer(OFFER.to_owned());
        intro.answer(id, ANSWER.to_owned()).unwrap();
        camera.replace_phone_key("newkey".to_owned());
        assert_eq!(
            camera
                .phone(&ask(Method::Get, "/p/phonekey/", "", PHONE))
                .status,
            404
        );
        assert_eq!(
            camera
                .phone(&ask(Method::Get, "/p/newkey/", "", PHONE))
                .status,
            200
        );
        assert_eq!(intro.answer_to(id), Waited::Gone);
        assert!(matches!(
            intro.offer_after(id),
            Waited::Got(Next::Blank { .. })
        ));
        // OBS's address is untouched.
        assert_eq!(
            camera
                .viewer(&ask(Method::Get, "/v/viewkey/", "", HERE))
                .status,
            200
        );
    }

    /// The authority is offered as a download a phone installs.
    #[test]
    fn the_authority_is_offered_to_the_phone() {
        let camera = camera(Duration::from_millis(50));
        let got = camera.phone(&ask(Method::Get, "/p/phonekey/djmanzo.crt", "", PHONE));
        assert_eq!(got.status, 200);
        assert_eq!(got.content_type, "application/x-x509-ca-cert");
        assert_eq!(got.body, b"the authority");
    }
}

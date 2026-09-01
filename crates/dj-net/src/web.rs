//! An HTTP server for the pages djmanzo shows to a room.
//!
//! # Why this is not the control server
//!
//! [`crate::server`] serves the DJ their own tools over a line protocol, and
//! is safe because it is off, bound to loopback, and behind a token the moment
//! it is not. None of that is available here. A phone cannot reach loopback,
//! so this **has** to face the network to be worth having, and the people
//! using it are strangers by design — you cannot print a secret on a sticker
//! and have it stay secret.
//!
//! So the safety is of a different kind: **this server is safe because of what
//! its handlers can do, not because of who can reach it.** A page served here
//! may take a song request or a sensor reading. It may not touch a deck. That
//! is a property of the handler the application installs, and the reason the
//! two servers are separate types rather than one with a flag.
//!
//! # Why synchronous, and a thread per connection
//!
//! It matches [`crate::server`], which already works this way, and it keeps an
//! async runtime out of a crate that has none. The load is a room of phones
//! submitting a form now and then — tens of requests a night, not thousands a
//! second.
//!
//! # Why a parser rather than reading the socket directly
//!
//! Because request parsing is the whole attack surface and this socket faces
//! strangers. Header handling, chunked bodies and request smuggling are
//! well-trodden ways to get it wrong; `tiny_http` has trodden them. The one
//! thing left to us is the size limit, which is below.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The largest body this will read.
///
/// Sixty-four kilobytes. A song request is a few hundred bytes and a sensor
/// reading less; this is generous for anything the served pages submit, and
/// small enough that a room of phones cannot exhaust memory between them by
/// each holding one open.
pub const MOST_BODY: usize = 64 * 1024;

/// What a request is asking to do.
///
/// Only the two verbs the served pages use. Everything else is refused rather
/// than guessed at — a `PUT` to this server is a mistake or a probe, and
/// neither deserves a handler call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// A request, in the terms a handler needs.
///
/// Deliberately djmanzo's own type rather than the server library's: a handler
/// written against this keeps working if the library underneath is ever
/// swapped, and the application does not grow a dependency on it.
#[derive(Debug, Clone)]
pub struct Incoming {
    pub method: Method,
    /// The path, without query. Percent-decoded.
    pub path: String,
    /// Query parameters, percent-decoded.
    pub query: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Who sent it — the address only, never the port.
    ///
    /// The port is different on every connection, so an address *with* one
    /// makes every request look like a phone that has never been seen before,
    /// which quietly turns any per-phone limit into no limit at all.
    pub from: Option<std::net::IpAddr>,
}

impl Incoming {
    /// The first value for `name`, if it was given.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The body read as a submitted `<form>`.
    ///
    /// The same encoding as a query string, which is why it is the same
    /// parser: a form and a query differ in where they travel, not in how
    /// they are written. A body that is not text at all reads as no fields
    /// rather than as an error — a handler asking for `song` wants to know
    /// there is no song, and cannot do anything more useful with the reason.
    #[must_use]
    pub fn form(&self) -> Vec<(String, String)> {
        std::str::from_utf8(&self.body)
            .map(pairs)
            .unwrap_or_default()
    }

    /// The first submitted value for `name`.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<String> {
        self.form()
            .into_iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }
}

/// What to send back.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    /// Anything beyond the content type. Usually empty.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    #[must_use]
    pub fn html(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    #[must_use]
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            content_type: "application/json; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    #[must_use]
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: body.into().into_bytes(),
        }
    }

    #[must_use]
    pub fn not_found() -> Self {
        Self::text(404, "not here")
    }

    /// Send the phone somewhere else to look at the result.
    ///
    /// What a form submission answers with, so that the address bar ends up
    /// on a page that can be reloaded. Without it a refresh re-posts the
    /// form, and somebody checking whether their song went through sends it
    /// again — which is a duplicate the room then has to look at.
    #[must_use]
    pub fn see_other(location: impl Into<String>) -> Self {
        let mut reply = Self::text(303, "over there");
        reply.headers.push(("Location".to_owned(), location.into()));
        reply
    }
}

/// What the application does with a request.
///
/// One method, so installing a handler is writing a closure or a small struct
/// rather than implementing an interface. `Send + Sync` because it is shared
/// across connection threads.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, request: &Incoming) -> Reply;
}

impl<F> Handler for F
where
    F: Fn(&Incoming) -> Reply + Send + Sync + 'static,
{
    fn handle(&self, request: &Incoming) -> Reply {
        self(request)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("could not listen on {0}: {1}")]
    Listen(SocketAddr, String),
    #[error("the web server could not start its thread: {0}")]
    Thread(String),
}

/// A running web server. Dropping it stops accepting and closes the port.
#[derive(Debug)]
pub struct WebServer {
    stop: Arc<AtomicBool>,
    accepting: Option<std::thread::JoinHandle<()>>,
    address: SocketAddr,
}

impl WebServer {
    /// Serve `handler` on `address`.
    ///
    /// Port 0 asks the operating system for a free one; [`Self::address`] then
    /// says which, which is how the interface learns what to put in a QR code.
    ///
    /// # Errors
    /// When the address cannot be bound, or the accept thread cannot start.
    pub fn start(address: SocketAddr, handler: Arc<dyn Handler>) -> Result<Self, WebError> {
        let server = tiny_http::Server::http(address)
            .map_err(|e| WebError::Listen(address, e.to_string()))?;
        let address = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| WebError::Listen(address, "not an IP socket".to_owned()))?;

        let stop = Arc::new(AtomicBool::new(false));
        let accepting = std::thread::Builder::new()
            .name("djmanzo-web".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || accept_loop(&server, &handler, &stop)
            })
            .map_err(|e| WebError::Thread(e.to_string()))?;

        Ok(Self {
            stop,
            accepting: Some(accepting),
            address,
        })
    }

    /// Where it actually ended up, which is what port 0 is for.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.accepting.take() {
            let _ = thread.join();
        }
    }
}

/// How long the accept loop waits before checking whether it should stop.
///
/// A blocking accept would hold the thread until the next request, which on a
/// quiet night is "until the application is killed" -- and the DJ turning the
/// server off would then not see the port close.
const CHECK_STOP_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

fn accept_loop(server: &tiny_http::Server, handler: &Arc<dyn Handler>, stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        match server.recv_timeout(CHECK_STOP_EVERY) {
            Ok(Some(request)) => {
                let handler = Arc::clone(handler);
                // A thread per request rather than a pool. The load is a room
                // of phones submitting a form now and then; a pool would be
                // machinery for a problem this does not have.
                let spawned = std::thread::Builder::new()
                    .name("djmanzo-web-request".into())
                    .spawn(move || serve(request, &handler));
                if let Err(why) = spawned {
                    tracing::warn!(%why, "could not spawn a thread for a request");
                }
            }
            Ok(None) => {}
            Err(why) => {
                tracing::warn!(%why, "the web server stopped accepting");
                break;
            }
        }
    }
}

fn serve(mut request: tiny_http::Request, handler: &Arc<dyn Handler>) {
    let Some(parsed) = read(&mut request) else {
        // Refused before the handler sees it: an unsupported verb, or a body
        // past the limit. Neither is worth an application's attention.
        let _ = request.respond(reply_of(&Reply::text(
            413,
            "too much, or not a verb I take",
        )));
        return;
    };

    let reply = handler.handle(&parsed);
    if let Err(why) = request.respond(reply_of(&reply)) {
        // The phone walked away mid-answer. Common in a room and not a fault.
        tracing::debug!(%why, "could not finish a reply");
    }
}

/// Turn a library request into ours, or refuse it.
fn read(request: &mut tiny_http::Request) -> Option<Incoming> {
    let method = match request.method() {
        tiny_http::Method::Get => Method::Get,
        tiny_http::Method::Post => Method::Post,
        _ => return None,
    };

    let raw = request.url().to_owned();
    let (path, query) = split_query(&raw);

    let mut body = Vec::new();
    if method == Method::Post {
        // A declared length past the cap is refused without reading it: the
        // point of a limit is not to read the thing first.
        if request.body_length().unwrap_or(0) > MOST_BODY {
            return None;
        }
        use std::io::Read as _;
        // Capped again while reading, because the declared length and the
        // actual body are two different claims.
        let mut limited = request.as_reader().take(MOST_BODY as u64 + 1);
        if limited.read_to_end(&mut body).is_err() || body.len() > MOST_BODY {
            return None;
        }
    }

    Some(Incoming {
        method,
        path,
        query,
        body,
        from: request.remote_addr().map(SocketAddr::ip),
    })
}

/// Split `"/a/b?x=1&y=2"` into its path and its decoded parameters.
fn split_query(raw: &str) -> (String, Vec<(String, String)>) {
    let (path, rest) = raw.split_once('?').unwrap_or((raw, ""));
    (decode(path), pairs(rest))
}

/// `"x=1&y=2"` into its decoded pairs. A query string and a submitted form
/// are written the same way, so they are read the same way.
fn pairs(rest: &str) -> Vec<(String, String)> {
    rest.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    // `+` is a space in a query string, and percent-decoding alone does not
    // know that. A request for "Un Verano" arrives as "Un+Verano".
    urlencoding::decode(&s.replace('+', " "))
        .map(std::borrow::Cow::into_owned)
        .unwrap_or_else(|_| s.to_owned())
}

fn reply_of(reply: &Reply) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], reply.content_type.as_bytes())
        .expect("a content type built from a literal is a valid header");
    let mut response = tiny_http::Response::from_data(reply.body.clone())
        .with_status_code(reply.status)
        .with_header(header);
    for (name, value) in &reply.headers {
        // A header a handler built out of text that came from a phone could
        // be anything at all, and a header that would split the response is
        // dropped rather than sent.
        match tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            Ok(extra) => response.add_header(extra),
            Err(()) => tracing::warn!(%name, "dropped a header a handler could not have meant"),
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    /// Send a raw request and return the whole response.
    ///
    /// Deliberately a real socket rather than calling the handler directly:
    /// what is being tested is that a browser's bytes become a handler call
    /// and a handler's answer becomes bytes, and a direct call tests neither.
    fn talk(address: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(address).expect("connect");
        stream.write_all(request.as_bytes()).expect("write");
        stream.flush().expect("flush");
        let mut got = String::new();
        // The server closes the connection after answering, so read-to-end
        // terminates.
        let _ = stream.read_to_string(&mut got);
        got
    }

    fn serving(handler: impl Handler) -> WebServer {
        WebServer::start(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            Arc::new(handler) as Arc<dyn Handler>,
        )
        .expect("start")
    }

    /// **A browser's bytes reach the handler and its answer comes back.**
    #[test]
    fn a_get_is_served() {
        let server = serving(|_: &Incoming| Reply::html("<p>hola</p>"));
        let got = talk(
            server.address(),
            "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(got.starts_with("HTTP/1.1 200"), "{got}");
        assert!(got.contains("<p>hola</p>"), "{got}");
        assert!(got.contains("text/html"), "{got}");
    }

    /// **Query parameters arrive decoded, spaces and accents included.**
    ///
    /// This is a song request box: "Un Verano en Nueva York" is exactly what
    /// somebody will type, and it arrives as `Un+Verano+en+Nueva+York`.
    #[test]
    fn a_query_is_decoded_including_plus_and_accents() {
        let server = serving(|r: &Incoming| {
            Reply::text(200, r.param("song").unwrap_or("nothing").to_owned())
        });
        let got = talk(
            server.address(),
            "GET /?song=Obsesi%C3%B3n+de+Aventura HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(got.contains("Obsesión de Aventura"), "{got}");
    }

    /// **A body reaches the handler.**
    #[test]
    fn a_post_body_is_delivered() {
        let server = serving(|r: &Incoming| {
            Reply::json(format!(
                "{{\"method\":\"{:?}\",\"got\":{}}}",
                r.method,
                r.body.len()
            ))
        });
        let got = talk(
            server.address(),
            "POST /request HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhola!",
        );
        assert!(got.contains("\"method\":\"Post\""), "{got}");
        assert!(got.contains("\"got\":5"), "{got}");
    }

    /// **A body past the cap is refused, and the handler never sees it.**
    ///
    /// The limit exists because this port faces a room. A handler that had to
    /// remember to check would eventually be written by somebody who forgot.
    #[test]
    fn an_enormous_body_is_refused_before_the_handler() {
        use std::sync::atomic::AtomicUsize;
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let server = serving(|_: &Incoming| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Reply::text(200, "should not happen")
        });
        let got = talk(
            server.address(),
            &format!(
                "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MOST_BODY + 1
            ),
        );
        assert!(got.starts_with("HTTP/1.1 413"), "{got}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 0, "the handler was called");
    }

    /// **A body that is promised and never sent does not hold a thread.**
    ///
    /// The declared-length check earns its place here and nowhere else. The
    /// cap while reading already makes an oversized body *safe*; this makes an
    /// oversized body *fast to refuse*, before waiting on bytes that may never
    /// come. Without it the connection sits in a read until the client gives
    /// up, and a room of phones on bad wifi is exactly where that happens by
    /// accident rather than by malice.
    #[test]
    fn a_promised_body_that_never_arrives_is_refused_without_waiting() {
        let server = serving(|_: &Incoming| Reply::text(200, "should not happen"));
        let mut stream = TcpStream::connect(server.address()).expect("connect");
        // Long enough that a hang is unmistakable, short enough not to stall
        // the suite if this regresses.
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        write!(
            stream,
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MOST_BODY + 1
        )
        .expect("write headers");
        stream.flush().expect("flush");
        // and then nothing at all.

        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        assert!(
            got.starts_with("HTTP/1.1 413"),
            "expected an immediate refusal, got {got:?}"
        );
    }

    /// **A verb the pages never use is refused rather than guessed at.**
    #[test]
    fn an_unsupported_verb_never_reaches_the_handler() {
        use std::sync::atomic::AtomicUsize;
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let server = serving(|_: &Incoming| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Reply::text(200, "should not happen")
        });
        let got = talk(
            server.address(),
            "DELETE /everything HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(got.starts_with("HTTP/1.1 413"), "{got}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 0, "the handler was called");
    }

    /// **Port 0 finds a port, and says which one.**
    ///
    /// How the interface learns what to put in a QR code.
    #[test]
    fn the_chosen_port_is_reported() {
        let server = serving(|_: &Incoming| Reply::not_found());
        assert_ne!(server.address().port(), 0);
    }

    /// **Dropping it stops answering, before the drop returns.**
    ///
    /// A DJ switching the server off has to actually stop serving the room,
    /// and has to have stopped by the time the switch springs back — a server
    /// that answers one more request after being turned off is a server the
    /// DJ has been told a lie about.
    ///
    /// Asked this way rather than by re-binding the port: the port is an
    /// ephemeral one, so a re-bind can fail because another test in this
    /// process took it in between, which says nothing about this server. What
    /// nothing else can do is answer with this handler's words.
    #[test]
    fn dropping_the_server_stops_it_answering() {
        const ONLY_THIS_SERVER_SAYS: &str = "the-dropped-server-answered";
        let address = {
            let server = serving(|_: &Incoming| Reply::text(200, ONLY_THIS_SERVER_SAYS));
            // Alive first, so a silent failure to start cannot pass as a
            // successful stop.
            let alive = talk(
                server.address(),
                "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
            );
            assert!(alive.contains(ONLY_THIS_SERVER_SAYS), "{alive}");
            server.address()
        };

        // Refused is the expected answer; anything else on this port is now
        // somebody else's server, and either way it is not ours.
        let after = match TcpStream::connect(address) {
            Ok(_) => talk(
                address,
                "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
            ),
            Err(_) => String::new(),
        };
        assert!(
            !after.contains(ONLY_THIS_SERVER_SAYS),
            "it answered after being dropped: {after}"
        );
    }

    /// **A chunked body past the cap is refused, and the handler never sees it.**
    ///
    /// A chunked request declares no length, so the cheap declared-length
    /// refusal above cannot fire and does not: this is the case the cap while
    /// reading exists for. Anybody may send one — it is one header — so an
    /// oversized body has a way in that never mentions its size.
    #[test]
    fn a_chunked_body_past_the_cap_is_refused() {
        use std::sync::atomic::AtomicUsize;
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let server = serving(|_: &Incoming| {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Reply::text(200, "should not happen")
        });
        let mut stream = TcpStream::connect(server.address()).expect("connect");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("timeout");
        stream
            .write_all(
                b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .expect("write headers");
        write!(stream, "{:x}\r\n", MOST_BODY + 1).expect("write chunk size");
        stream
            .write_all(&vec![b'x'; MOST_BODY + 1])
            .expect("write chunk");
        // The tail may not land: by now the server has read its fill, answered
        // and hung up, which is the point.
        let _ = stream.write_all(b"\r\n0\r\n\r\n");

        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        assert!(got.starts_with("HTTP/1.1 413"), "{got}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 0, "the handler was called");
    }

    /// **A body that never ends is refused rather than read forever.**
    ///
    /// Chunked encoding lets a client keep sending until it says stop, and a
    /// client that never says stop is one `Transfer-Encoding` header away.
    /// Refusing after reading would mean holding every byte first, and a
    /// connection nobody can close is a denial of service on the room's own
    /// request page.
    ///
    /// **The proof is that the sending stops, not that the refusal arrives.**
    /// This used to assert that a 413 could be read while the client was still
    /// writing, and that assertion is not portable: on Windows a close from
    /// the peer while this side is still sending is an RST, and an RST
    /// discards whatever the receive buffer was holding -- including the
    /// refusal. It passed on Linux and macOS and failed on Windows about half
    /// the time, which is the worst kind of test. `tiny_http` owns the socket
    /// after `respond`, so a lingering close -- what Apache and nginx do for
    /// exactly this reason -- is not ours to add.
    ///
    /// What is portable, and is the property that matters, is that the writes
    /// eventually fail. A server that read forever would keep accepting: the
    /// loop below would run to its bound with every write succeeding. That the
    /// 413 does arrive when it can is covered by
    /// [`a_chunked_body_past_the_cap_is_refused`], which stops writing before
    /// it reads and so never provokes the reset.
    #[test]
    fn an_endless_body_is_refused_rather_than_read_forever() {
        let server = serving(|_: &Incoming| Reply::text(200, "should not happen"));
        let mut stream = TcpStream::connect(server.address()).expect("connect");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("write timeout");
        stream
            .write_all(
                b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .expect("write headers");

        // Sixty-four times the cap, in chunks, and never a terminating one.
        // Far past any socket buffer, so a write failure means the far end
        // really has gone rather than that the kernel is still swallowing.
        let chunk = format!("{:x}\r\n{}\r\n", 4096, "x".repeat(4096));
        let mut sent = 0usize;
        let mut hung_up = false;
        for _ in 0..(MOST_BODY / 4096 * 64) {
            if stream.write_all(chunk.as_bytes()).is_err() {
                hung_up = true;
                break;
            }
            sent += chunk.len();
        }
        assert!(
            hung_up,
            "the server was still taking body after {sent} bytes, against a cap of {MOST_BODY}"
        );

        // Whatever survived the close has to be the refusal, never a 200: the
        // handler must not have run. Empty is allowed, and on Windows likely.
        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        assert!(
            got.is_empty() || got.starts_with("HTTP/1.1 413"),
            "the server answered something other than a refusal: {got:?}"
        );
    }

    /// **A submitted form reaches the handler as fields.**
    #[test]
    fn a_form_body_is_read_as_fields() {
        let server = serving(|r: &Incoming| {
            Reply::text(200, r.field("song").unwrap_or_else(|| "nothing".to_owned()))
        });
        let body = "song=Obsesi%C3%B3n+de+Aventura&second=";
        let got = talk(
            server.address(),
            &format!(
                "POST /request HTTP/1.1\r\nHost: x\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        );
        assert!(got.contains("Obsesión de Aventura"), "{got}");
    }

    /// **A body that is not text at all is no fields, not a crash.**
    #[test]
    fn a_body_that_is_not_text_reads_as_no_fields() {
        let server = serving(|r: &Incoming| Reply::text(200, format!("{}", r.form().len())));
        let mut stream = TcpStream::connect(server.address()).expect("connect");
        stream
            .write_all(
                b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\nConnection: close\r\n\r\n",
            )
            .expect("write");
        stream.write_all(&[0xff, 0xfe, 0x00]).expect("write body");
        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        assert!(got.starts_with("HTTP/1.1 200"), "{got}");
        assert!(got.ends_with('0'), "{got}");
    }

    /// **A handler knows which phone it is talking to, and not which socket.**
    ///
    /// The per-phone limits in the request book are counted against this. An
    /// address carrying its port would make every request a first request.
    #[test]
    fn the_senders_address_arrives_without_its_port() {
        let server = serving(|r: &Incoming| Reply::text(200, format!("{:?}", r.from)));
        let got = talk(
            server.address(),
            "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(got.contains("127.0.0.1"), "{got}");
        // Two connections from this test have two different source ports; if
        // one leaked through, the two would not agree.
        let again = talk(
            server.address(),
            "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        let body = |whole: &str| whole.rsplit("\r\n\r\n").next().unwrap_or("").to_owned();
        assert_eq!(
            body(&got),
            body(&again),
            "the port leaked into the identity"
        );
    }

    /// **A redirect after a form carries where to go.**
    #[test]
    fn a_redirect_says_where() {
        let server = serving(|_: &Incoming| Reply::see_other("/?said=ok"));
        let got = talk(
            server.address(),
            "GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        );
        assert!(got.starts_with("HTTP/1.1 303"), "{got}");
        assert!(got.to_lowercase().contains("location: /?said=ok"), "{got}");
    }

    #[test]
    fn a_path_and_its_query_are_split() {
        let (path, query) = split_query("/request?song=Bachata&by=Aventura");
        assert_eq!(path, "/request");
        assert_eq!(
            query,
            vec![
                ("song".to_owned(), "Bachata".to_owned()),
                ("by".to_owned(), "Aventura".to_owned())
            ]
        );
        assert_eq!(split_query("/plain").1, Vec::new());
    }
}

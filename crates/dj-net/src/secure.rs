//! §119: one port that answers a phone over HTTPS and this computer over plain
//! HTTP.
//!
//! The phone camera page has two readers. A phone's browser lends a page its
//! camera only over HTTPS, so it is answered in TLS with a certificate from
//! [`crate::authority`]. OBS reads the other page on the same computer, where
//! `http://127.0.0.1` is already a secure origin and a certificate of
//! djmanzo's would only be something for OBS to refuse. One port serves both:
//! **the first byte of a connection says which it is** — a TLS handshake
//! starts with 22, and no HTTP request does.
//!
//! # Why not the room's server
//!
//! [`crate::web`] reads requests with `tiny_http`, whose TLS pins a 2021
//! `rustls` and an unmaintained `ring` (see RESEARCH.md). This takes the
//! current `rustls` already in djmanzo for its own HTTPS requests, and the
//! request parser `httparse`, already here under `hyper` and `tungstenite`.
//! It answers in the same [`Incoming`] and [`Reply`], so a handler written for
//! one is a handler for the other.
//!
//! # What it will not do
//!
//! What a socket facing a venue's wifi must not: **one request per
//! connection**, [`MOST_HEAD`] of headers, [`MOST_BODY`] of body, no chunked
//! bodies, [`MOST_AT_ONCE`] connections at once, and [`TO_ASK`] to send a
//! request — so a stranger holding connections open costs a few threads for
//! a few seconds, and nothing more.

use crate::web::{Handler, Incoming, MOST_BODY, Method, Reply, split_query};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The most a request's line and headers may take.
pub const MOST_HEAD: usize = 16 * 1024;

/// The most connections served at once. A phone and OBS hold one each while
/// they wait for each other; the rest is room for a reload.
pub const MOST_AT_ONCE: usize = 32;

/// How long a connection has to send its whole request.
pub const TO_ASK: Duration = Duration::from_secs(10);

/// How long an answer may take to write.
const TO_ANSWER: Duration = Duration::from_secs(10);

const ACCEPT_POLL: Duration = Duration::from_millis(200);

/// The first byte of every TLS connection: a handshake record.
const HANDSHAKE: u8 = 0x16;

/// The TLS configuration for a connection that reached `local` — the address
/// the phone used, read from the socket, which is the one its certificate
/// must name. `None` when there is none to give, and the connection is closed.
pub type Certify = dyn Fn(IpAddr) -> Option<Arc<rustls::ServerConfig>> + Send + Sync;

#[derive(Debug, thiserror::Error)]
pub enum SecureError {
    #[error("could not listen on {0}: {1}")]
    Listen(SocketAddr, String),
    #[error("could not start its thread: {0}")]
    Thread(String),
}

/// A running server. Dropping it stops accepting and closes the port.
#[derive(Debug)]
pub struct SecureServer {
    stop: Arc<AtomicBool>,
    accepting: Option<std::thread::JoinHandle<()>>,
    address: SocketAddr,
}

struct Serving {
    certify: Arc<Certify>,
    secure: Arc<dyn Handler>,
    plain: Arc<dyn Handler>,
    open: AtomicUsize,
}

impl SecureServer {
    /// Serve on `address`: `secure` answers what arrives in TLS, `plain` the
    /// rest. Port 0 asks the system for one; [`Self::address`] says which.
    ///
    /// # Errors
    /// When the address cannot be bound or the thread cannot start.
    pub fn start(
        address: SocketAddr,
        certify: Arc<Certify>,
        secure: Arc<dyn Handler>,
        plain: Arc<dyn Handler>,
    ) -> Result<Self, SecureError> {
        let listener =
            TcpListener::bind(address).map_err(|e| SecureError::Listen(address, e.to_string()))?;
        let bound = listener
            .local_addr()
            .map_err(|e| SecureError::Listen(address, e.to_string()))?;
        // Non-blocking so the loop notices `stop`; a blocking accept would
        // hold the port open until the next phone came along.
        listener
            .set_nonblocking(true)
            .map_err(|e| SecureError::Listen(address, e.to_string()))?;
        let serving = Arc::new(Serving {
            certify,
            secure,
            plain,
            open: AtomicUsize::new(0),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let accepting = std::thread::Builder::new()
            .name("djmanzo-secure".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || accept_loop(&listener, &serving, &stop)
            })
            .map_err(|e| SecureError::Thread(e.to_string()))?;
        Ok(Self {
            stop,
            accepting: Some(accepting),
            address: bound,
        })
    }

    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

impl Drop for SecureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.accepting.take() {
            let _ = thread.join();
        }
    }
}

/// Counts a connection in, and out again however its thread ends.
struct Held<'a>(&'a AtomicUsize);

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept_loop(listener: &TcpListener, serving: &Arc<Serving>, stop: &AtomicBool) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if serving.open.fetch_add(1, Ordering::SeqCst) >= MOST_AT_ONCE {
                    serving.open.fetch_sub(1, Ordering::SeqCst);
                    // Closed unanswered: answering would be work done for
                    // whoever is holding the other thirty-two.
                    continue;
                }
                let this = Arc::clone(serving);
                let spawned = std::thread::Builder::new()
                    .name("djmanzo-secure-conn".into())
                    .spawn(move || {
                        let _held = Held(&this.open);
                        if let Err(why) = serve(stream, peer, &this) {
                            tracing::debug!(%why, %peer, "a connection ended early");
                        }
                    });
                if spawned.is_err() {
                    // The thread never started, so nothing will count it out.
                    serving.open.fetch_sub(1, Ordering::SeqCst);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL);
            }
            Err(why) => {
                tracing::warn!(%why, "the phone camera's server stopped accepting");
                return;
            }
        }
    }
}

fn serve(stream: TcpStream, peer: SocketAddr, serving: &Serving) -> std::io::Result<()> {
    // Accepted sockets inherit non-blocking on some systems and not others.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(TO_ASK))?;
    stream.set_write_timeout(Some(TO_ANSWER))?;
    stream.set_nodelay(true)?;
    let mut first = [0u8; 1];
    if stream.peek(&mut first)? == 0 {
        return Ok(());
    }
    let deadline = Instant::now() + TO_ASK;
    if first[0] == HANDSHAKE {
        let Some(config) = (serving.certify)(stream.local_addr()?.ip()) else {
            return Ok(());
        };
        let connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
        let mut tls = rustls::StreamOwned::new(connection, stream);
        answer(&mut tls, peer, deadline, serving.secure.as_ref())?;
        tls.conn.send_close_notify();
        tls.flush()
    } else {
        let mut plain = stream;
        answer(&mut plain, peer, deadline, serving.plain.as_ref())
    }
}

fn answer(
    stream: &mut (impl Read + Write),
    peer: SocketAddr,
    deadline: Instant,
    handler: &dyn Handler,
) -> std::io::Result<()> {
    let reply = match read_request(stream, peer, deadline)? {
        Ok(request) => handler.handle(&request),
        Err(refused) => refused,
    };
    write_reply(stream, &reply)
}

/// Read one request, or the reply that refuses it.
///
/// `Err` is the socket failing or the deadline passing, when there is no one
/// left to answer; `Ok(Err(..))` is a request refused with a reason.
fn read_request(
    stream: &mut impl Read,
    peer: SocketAddr,
    deadline: Instant,
) -> std::io::Result<Result<Incoming, Reply>> {
    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let (head, method, target, length, range) = loop {
        if Instant::now() > deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buffer.extend_from_slice(&chunk[..read]);
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&buffer) {
            Ok(httparse::Status::Complete(head)) => {
                let method = match request.method {
                    Some("GET") => Method::Get,
                    Some("POST") => Method::Post,
                    _ => return Ok(Err(Reply::text(405, "not a verb I take"))),
                };
                let mut length = 0usize;
                let mut range = None;
                for header in request.headers.iter() {
                    if header.name.eq_ignore_ascii_case("range") {
                        range = std::str::from_utf8(header.value).ok().map(str::to_owned);
                    }
                    if header.name.eq_ignore_ascii_case("transfer-encoding") {
                        // Chunked bodies are where request smuggling lives,
                        // and nothing this serves sends one.
                        return Ok(Err(Reply::text(411, "say how long it is")));
                    }
                    if header.name.eq_ignore_ascii_case("content-length") {
                        let Some(n) = std::str::from_utf8(header.value)
                            .ok()
                            .and_then(|v| v.trim().parse::<usize>().ok())
                        else {
                            return Ok(Err(Reply::text(400, "not a length")));
                        };
                        length = n;
                    }
                }
                if length > MOST_BODY {
                    return Ok(Err(Reply::text(413, "too much")));
                }
                let target = request.path.unwrap_or("/").to_owned();
                break (head, method, target, length, range);
            }
            Ok(httparse::Status::Partial) if buffer.len() < MOST_HEAD => {}
            Ok(httparse::Status::Partial) | Err(httparse::Error::TooManyHeaders) => {
                return Ok(Err(Reply::text(431, "too much")));
            }
            Err(_) => return Ok(Err(Reply::text(400, "not a request"))),
        }
    };

    let mut body = buffer.split_off(head);
    body.truncate(length);
    while body.len() < length {
        if Instant::now() > deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let wanted = (length - body.len()).min(read);
        body.extend_from_slice(&chunk[..wanted]);
    }

    let (path, query) = split_query(&target);
    Ok(Ok(Incoming {
        method,
        path,
        query,
        body,
        from: Some(peer.ip()),
        range,
    }))
}

fn write_reply(stream: &mut impl Write, reply: &Reply) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Connection: close\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n",
        reply.status,
        reason(reply.status),
        reply.content_type,
        reply.body.len()
    );
    for (name, value) in &reply.headers {
        // A header that would end the head early, or split it, is a handler's
        // mistake and is dropped rather than sent.
        let clean = |s: &str| !s.is_empty() && !s.contains(['\r', '\n']);
        if clean(name) && !value.contains(['\r', '\n']) && !name.contains(':') {
            head.push_str(&format!("{name}: {value}\r\n"));
        } else {
            tracing::warn!(%name, "dropped a header a handler could not have meant");
        }
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&reply.body)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::Authority;
    use rustls::pki_types::{CertificateDer, ServerName};

    fn provider() -> Arc<rustls::crypto::CryptoProvider> {
        Arc::new(rustls::crypto::ring::default_provider())
    }

    fn config_for(authority: &Authority, address: IpAddr) -> Arc<rustls::ServerConfig> {
        authority.issue(address).unwrap().server_config().unwrap()
    }

    /// A server whose TLS side says "secure" and plain side says "plain",
    /// each with the path and body it was given.
    fn serving(authority: &Arc<Authority>) -> SecureServer {
        let authority = Arc::clone(authority);
        let echo = |side: &'static str| {
            Arc::new(move |r: &Incoming| {
                Reply::text(
                    200,
                    format!(
                        "{side} {:?} {} {}",
                        r.method,
                        r.path,
                        String::from_utf8_lossy(&r.body)
                    ),
                )
            }) as Arc<dyn Handler>
        };
        SecureServer::start(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            Arc::new(move |local| Some(config_for(&authority, local))),
            echo("secure"),
            echo("plain"),
        )
        .unwrap()
    }

    /// A client that trusts only `authority`, as a phone that installed it.
    fn client(authority: &Authority) -> Arc<rustls::ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from(authority.certificate().to_vec()))
            .unwrap();
        Arc::new(
            rustls::ClientConfig::builder_with_provider(provider())
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )
    }

    fn over_tls(
        address: SocketAddr,
        config: Arc<rustls::ClientConfig>,
        request: &str,
    ) -> std::io::Result<String> {
        let name = ServerName::IpAddress(address.ip().into());
        let connection = rustls::ClientConnection::new(config, name).unwrap();
        let socket = TcpStream::connect(address)?;
        let mut tls = rustls::StreamOwned::new(connection, socket);
        tls.write_all(request.as_bytes())?;
        let mut got = String::new();
        tls.read_to_string(&mut got)?;
        Ok(got)
    }

    fn over_plain(address: SocketAddr, request: &[u8]) -> String {
        let mut socket = TcpStream::connect(address).unwrap();
        socket.write_all(request).unwrap();
        let mut got = String::new();
        let _ = socket.read_to_string(&mut got);
        got
    }

    /// **A phone that installed djmanzo's authority is answered in TLS by
    /// the secure side; this computer in plain HTTP by the other, on the same
    /// port.** The certificate names the address the phone reached, chains to
    /// the authority, and is accepted by a verifier that trusts nothing else.
    #[test]
    fn one_port_answers_tls_and_plain_each_its_own_way() {
        let authority = Arc::new(Authority::new("djmanzo test").unwrap());
        let server = serving(&authority);
        let address = server.address();

        let got = over_tls(
            address,
            client(&authority),
            "POST /p/k/offer?x=1 HTTP/1.1\r\nHost: a\r\nContent-Length: 5\r\n\r\nhello",
        )
        .expect("the authority's certificate is accepted");
        assert!(got.starts_with("HTTP/1.1 200 OK\r\n"), "{got}");
        assert!(got.contains("Connection: close\r\n"));
        assert!(got.contains("Cache-Control: no-store\r\n"));
        assert!(got.ends_with("secure Post /p/k/offer hello"), "{got}");

        let got = over_plain(address, b"GET /v/k HTTP/1.1\r\nHost: a\r\n\r\n");
        assert!(got.ends_with("plain Get /v/k "), "{got}");
    }

    /// **The authority vouches for private addresses and nothing else.** A
    /// certificate it signed for a public address is refused by a verifier
    /// that trusts the authority — which is what makes its key, kept in a
    /// file, worth nothing to anyone who took it.
    #[test]
    fn the_authority_cannot_vouch_for_a_public_address() {
        let authority = Arc::new(Authority::new("djmanzo test").unwrap());
        let public: IpAddr = "8.8.8.8".parse().unwrap();
        let held = Arc::clone(&authority);
        // Certified for the public address whatever the socket says, and
        // asked for by that name.
        let server = SecureServer::start(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            Arc::new(move |_| Some(config_for(&held, public))),
            Arc::new(|_: &Incoming| Reply::text(200, "secure")) as Arc<dyn Handler>,
            Arc::new(|_: &Incoming| Reply::text(200, "plain")) as Arc<dyn Handler>,
        )
        .unwrap();
        let name = ServerName::IpAddress(public.into());
        let connection = rustls::ClientConnection::new(client(&authority), name).unwrap();
        let socket = TcpStream::connect(server.address()).unwrap();
        let mut tls = rustls::StreamOwned::new(connection, socket);
        let sent = tls.write_all(b"GET / HTTP/1.1\r\n\r\n");
        let mut got = String::new();
        let read = tls.read_to_string(&mut got);
        let refused = sent.is_err() || read.is_err();
        assert!(refused, "a public address was vouched for: {got}");
        let why = format!("{:?} {:?}", sent.err(), read.err());
        assert!(why.contains("NameConstraint"), "{why}");
    }

    /// A phone that has not installed the authority — no roots at all —
    /// is refused by a verifier, which is the warning a phone's browser
    /// shows and lets the DJ tap past.
    #[test]
    fn a_phone_without_the_authority_is_warned() {
        let authority = Arc::new(Authority::new("djmanzo test").unwrap());
        let server = serving(&authority);
        let stranger = Authority::new("someone else").unwrap();
        assert!(
            over_tls(
                server.address(),
                client(&stranger),
                "GET / HTTP/1.1\r\n\r\n"
            )
            .is_err()
        );
    }

    /// **What a socket facing a room must refuse, it refuses**: a verb it does
    /// not take, a chunked body, a body or a head past the limit, a length
    /// that is not a number.
    #[test]
    fn requests_out_of_bounds_are_refused() {
        let authority = Arc::new(Authority::new("djmanzo test").unwrap());
        let server = serving(&authority);
        let a = server.address();
        let status = |got: String| got.lines().next().unwrap_or_default().to_owned();

        assert_eq!(
            status(over_plain(a, b"PUT / HTTP/1.1\r\n\r\n")),
            "HTTP/1.1 405 Method Not Allowed"
        );
        assert_eq!(
            status(over_plain(
                a,
                b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n"
            )),
            "HTTP/1.1 411 Length Required"
        );
        let too_long = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MOST_BODY + 1
        );
        assert_eq!(
            status(over_plain(a, too_long.as_bytes())),
            "HTTP/1.1 413 Content Too Large"
        );
        let big_head = format!("GET / HTTP/1.1\r\nX-A: {}\r\n\r\n", "a".repeat(MOST_HEAD));
        assert_eq!(
            status(over_plain(a, big_head.as_bytes())),
            "HTTP/1.1 431 Request Header Fields Too Large"
        );
        assert_eq!(
            status(over_plain(
                a,
                b"POST / HTTP/1.1\r\nContent-Length: lots\r\n\r\n"
            )),
            "HTTP/1.1 400 Bad Request"
        );
        // A body is cut at its length, not at whatever else was sent.
        let got = over_plain(a, b"POST /x HTTP/1.1\r\nContent-Length: 2\r\n\r\nhello");
        assert!(got.ends_with("plain Post /x he"), "{got}");
    }

    /// A header a handler built with a line break in it is dropped, so a
    /// reply cannot be split into two.
    #[test]
    fn a_header_that_would_split_the_reply_is_dropped() {
        let mut reply = Reply::text(200, "ok");
        reply.headers.push(("X-Fine".to_owned(), "yes".to_owned()));
        reply
            .headers
            .push(("X-Bad".to_owned(), "a\r\nSet-Cookie: b".to_owned()));
        let mut out = Vec::new();
        write_reply(&mut out, &reply).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("X-Fine: yes\r\n"));
        assert!(!out.contains("Set-Cookie"));
    }

    /// **No more than [`MOST_AT_ONCE`] connections are held**: one past it is
    /// closed unanswered, and a place frees once one of them goes.
    #[test]
    fn connections_past_the_limit_are_closed() {
        let authority = Arc::new(Authority::new("djmanzo test").unwrap());
        let server = serving(&authority);
        let a = server.address();
        // Each sends half a request and waits.
        let held: Vec<TcpStream> = (0..MOST_AT_ONCE)
            .map(|_| {
                let mut s = TcpStream::connect(a).unwrap();
                s.write_all(b"GET / HTTP/1.1\r\n").unwrap();
                s
            })
            .collect();
        // Give the accept loop time to take every one of them.
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
            let mut probe = TcpStream::connect(a).unwrap();
            probe
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let _ = probe.write_all(b"GET /late HTTP/1.1\r\n\r\n");
            let mut got = String::new();
            let _ = probe.read_to_string(&mut got);
            if got.is_empty() {
                break;
            }
        }
        let mut late = TcpStream::connect(a).unwrap();
        late.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let _ = late.write_all(b"GET /late HTTP/1.1\r\n\r\n");
        let mut got = String::new();
        let _ = late.read_to_string(&mut got);
        assert!(got.is_empty(), "answered past the limit: {got}");

        drop(held);
        let until = Instant::now() + Duration::from_secs(5);
        let mut answered = String::new();
        while answered.is_empty() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
            answered = over_plain(a, b"GET /after HTTP/1.1\r\n\r\n");
        }
        assert!(answered.ends_with("plain Get /after "), "{answered}");
    }
}

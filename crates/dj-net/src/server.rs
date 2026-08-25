//! A local control server: newline-delimited JSON over TCP.
//!
//! # Why this exists
//!
//! [`ControlService`] parses untrusted text into the same [`dj_core::Action`]
//! every local input sends. That is the whole M7 boundary, and until this
//! existed nothing could reach it: the crate compiled, its tests passed, and no
//! part of djmanzo depended on it. A tested boundary with no door is a very
//! thorough no-op.
//!
//! # Why a line protocol rather than WebSocket
//!
//! Because it needs no dependency at all. A WebSocket adapter is still the
//! right thing for a browser client and the roadmap still says so -- but a
//! WebSocket is a framing layer over exactly this, and shipping the framing
//! first would have meant shipping a library before shipping the capability.
//! One JSON object per line, `\n`-terminated, both directions.
//!
//! # What it will not do
//!
//! **It is off unless switched on, and bound to loopback unless told
//! otherwise.** A DJ laptop on a club's wifi is not a place to open a port by
//! default.
//!
//! **A token is required the moment the bind address is not loopback.** On
//! `127.0.0.1` any process on the machine can already drive the decks and a
//! token buys little; the moment the socket faces a room, it is the only thing
//! between a set and anybody on the same network. That rule is enforced here,
//! at [`ControlServer::start`], rather than left to whoever writes the
//! settings panel.

use crate::control::{ControlRequest, ControlResponse, ControlService, ErrorCode};
use dj_core::Action;
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// How long a connection may sit silent before it is dropped.
///
/// Also how long the accept loop waits before noticing it has been asked to
/// stop, since a blocking `accept` cannot be interrupted portably.
const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const ACCEPT_POLL: std::time::Duration = std::time::Duration::from_millis(200);

/// How many requests a connection may make in a second, sustained.
///
/// A DJ's hands produce a few actions a second at most; a Stream Deck page
/// turn is a handful at once. Sixty leaves room for a fader being swept by a
/// script -- which is the one legitimate reason to send a lot -- while a
/// runaway loop hits the wall immediately.
const REQUESTS_PER_SECOND: f64 = 60.0;

/// How far a client may run ahead of that rate in one go.
///
/// A burst is normal: a scene change fires a dozen actions at once and should
/// not be throttled for it. Sustained flooding is not.
const BURST: f64 = 120.0;

/// The longest line a client may send.
///
/// An action is a short string and a frame that is not one is either a mistake
/// or an attempt to make djmanzo allocate. Bounded so neither costs anything.
const MAX_LINE: usize = 8 * 1024;

/// A token bucket: `rate` requests a second, `burst` in hand.
///
/// Kept here rather than reached for from a crate because it is nine lines and
/// a dependency for nine lines is a dependency to keep updated forever.
///
/// **The bus is bounded, so a flood cannot reach the audio thread either way**
/// -- it would be refused with `queue_full`. This is about the rest of the
/// process: parsing and answering a hundred thousand frames a second is work
/// djmanzo does instead of drawing waveforms.
#[derive(Debug)]
struct Bucket {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: std::time::Instant,
}

impl Bucket {
    fn new(rate: f64, burst: f64) -> Self {
        Self {
            tokens: burst,
            rate,
            burst,
            last: std::time::Instant::now(),
        }
    }

    /// Take one token, or say there was none.
    fn take(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("could not listen on {0}: {1}")]
    Listen(SocketAddr, String),
    #[error(
        "{0} is not a loopback address, so a token is required: anything on the \
         network could otherwise drive the decks"
    )]
    TokenRequired(IpAddr),
    #[error("the control server could not start its thread: {0}")]
    Thread(String),
}

/// A running control server. Dropping it stops accepting and closes the port.
#[derive(Debug)]
pub struct ControlServer {
    stop: Arc<AtomicBool>,
    accepting: Option<std::thread::JoinHandle<()>>,
    address: SocketAddr,
}

impl ControlServer {
    /// Listen on `address`, applying what arrives through `service`.
    ///
    /// `token`, when set, must be the first line a client sends. See the
    /// module documentation for when it stops being optional.
    ///
    /// # Errors
    /// When the address cannot be bound, or when it is not loopback and no
    /// token was given.
    pub fn start<C>(
        address: SocketAddr,
        token: Option<String>,
        service: Arc<ControlService<C>>,
    ) -> Result<Self, ServerError>
    where
        C: From<Action> + Send + 'static,
    {
        if !address.ip().is_loopback() && token.as_deref().unwrap_or("").is_empty() {
            return Err(ServerError::TokenRequired(address.ip()));
        }

        let listener =
            TcpListener::bind(address).map_err(|e| ServerError::Listen(address, e.to_string()))?;
        // Non-blocking so the accept loop can notice `stop`. A blocking accept
        // would hold the thread until the next client, which on a quiet port
        // is "until the application is killed".
        listener
            .set_nonblocking(true)
            .map_err(|e| ServerError::Listen(address, e.to_string()))?;
        let address = listener
            .local_addr()
            .map_err(|e| ServerError::Listen(address, e.to_string()))?;

        let stop = Arc::new(AtomicBool::new(false));
        let accepting = std::thread::Builder::new()
            .name("djmanzo-control-net".into())
            .spawn({
                let stop = Arc::clone(&stop);
                move || accept_loop(&listener, &service, token.as_deref(), &stop)
            })
            .map_err(|e| ServerError::Thread(e.to_string()))?;

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

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(accepting) = self.accepting.take() {
            let _ = accepting.join();
        }
    }
}

fn accept_loop<C>(
    listener: &TcpListener,
    service: &Arc<ControlService<C>>,
    token: Option<&str>,
    stop: &AtomicBool,
) where
    C: From<Action> + Send + 'static,
{
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _peer)) => {
                let service = Arc::clone(service);
                let token = token.map(str::to_owned);
                // A thread per connection. There will be one or two of these --
                // a lighting desk, a phone -- not a thousand, and a thread that
                // blocks on a read costs a stack.
                let spawned = std::thread::Builder::new()
                    .name("djmanzo-control-conn".into())
                    .spawn(move || {
                        let _ = serve(&stream, &service, token.as_deref());
                    });
                if spawned.is_err() {
                    // Out of threads. Refusing this client is the only option,
                    // and it is better than taking the server down with it.
                    continue;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(ACCEPT_POLL);
            }
            Err(_) => return,
        }
    }
}

/// Read frames until the client goes away.
fn serve<C>(
    stream: &TcpStream,
    service: &ControlService<C>,
    token: Option<&str>,
) -> std::io::Result<()>
where
    C: From<Action>,
{
    stream.set_read_timeout(Some(IDLE_TIMEOUT))?;
    stream.set_nodelay(true)?;
    let mut out = stream.try_clone()?;
    // Blocking again on this side: the connection thread has nothing else to
    // do, and the read timeout is what ends it.
    stream.set_nonblocking(false)?;
    let lines = BufReader::new(stream).take(u64::MAX).lines();

    let mut authorised = token.is_none();
    let mut budget = Bucket::new(REQUESTS_PER_SECOND, BURST);
    for line in lines {
        let line = line?;
        if line.len() > MAX_LINE {
            write_line(&mut out, &refused(ErrorCode::BadRequest, "frame too long"))?;
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }

        if !authorised {
            // The first frame has to be the greeting, and a wrong token ends
            // the connection rather than inviting another guess.
            match serde_json::from_str::<ControlRequest>(&line) {
                Ok(ControlRequest::Hello { token: offered })
                    if token.is_some_and(|wanted| constant_time_eq(wanted, &offered)) =>
                {
                    authorised = true;
                    write_line(&mut out, &ControlResponse::Accepted)?;
                    continue;
                }
                _ => {
                    write_line(
                        &mut out,
                        &refused(ErrorCode::Unauthorised, "send `hello` with the token first"),
                    )?;
                    return Ok(());
                }
            }
        }

        // Checked after the greeting, so a client that has not authenticated
        // cannot spend somebody else's budget, and before the work, because
        // the work is the thing being limited.
        if !budget.take(std::time::Instant::now()) {
            write_line(
                &mut out,
                &refused(ErrorCode::TooFast, "slow down and try again"),
            )?;
            continue;
        }

        write_line(&mut out, &service.handle_json(&line))?;
    }
    Ok(())
}

fn refused(code: ErrorCode, message: &str) -> ControlResponse {
    ControlResponse::Error {
        code,
        message: message.to_owned(),
    }
}

fn write_line(out: &mut impl Write, response: &ControlResponse) -> std::io::Result<()> {
    let mut line = serde_json::to_string(response).unwrap_or_else(|_| {
        // Serialising our own response type cannot fail; if it somehow does,
        // saying so is better than dropping the client without a word.
        r#"{"type":"error","code":"bad_request","message":"unserialisable"}"#.to_owned()
    });
    line.push('\n');
    out.write_all(line.as_bytes())?;
    out.flush()
}

/// Compare two secrets without leaking their length difference through timing.
///
/// Not because a token this is protecting is worth a timing attack, but
/// because writing `a == b` for a secret is the habit that eventually is.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;
    use dj_control::{ActionBus, ParameterRegistry};
    use std::io::BufWriter;
    use std::net::Ipv4Addr;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Command {
        Action(Action),
    }
    impl From<Action> for Command {
        fn from(value: Action) -> Self {
            Self::Action(value)
        }
    }

    struct Rig {
        server: ControlServer,
        engine: rtrb::Consumer<Command>,
    }

    fn rig(token: Option<&str>, ip: Ipv4Addr) -> Result<Rig, ServerError> {
        let (bus, engine) = ActionBus::<Command>::new(64);
        let service = Arc::new(ControlService::new(
            Arc::new(bus),
            Arc::new(ParameterRegistry::new()),
        ));
        // Port 0: the operating system picks a free one, so tests never
        // collide with each other or with something already running.
        let server =
            ControlServer::start(SocketAddr::from((ip, 0)), token.map(str::to_owned), service)?;
        Ok(Rig { server, engine })
    }

    /// A client, as anything driving djmanzo over the network would be.
    struct Client {
        lines: std::io::Lines<BufReader<TcpStream>>,
        out: BufWriter<TcpStream>,
    }

    impl Client {
        fn connect(server: &ControlServer) -> Self {
            let stream = TcpStream::connect(server.address()).expect("the port is open");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .expect("a read timeout");
            Self {
                lines: BufReader::new(stream.try_clone().expect("clone")).lines(),
                out: BufWriter::new(stream),
            }
        }

        fn send(&mut self, line: &str) {
            self.out.write_all(line.as_bytes()).expect("write");
            self.out.write_all(b"\n").expect("newline");
            self.out.flush().expect("flush");
        }

        fn reply(&mut self) -> Option<ControlResponse> {
            let line = self.lines.next()?.ok()?;
            serde_json::from_str(&line).ok()
        }
    }

    /// **The point of the whole crate.** Text arrives from somewhere that is
    /// not this process and comes out as the same action a pad sends.
    #[test]
    fn an_action_sent_over_the_socket_reaches_the_engine() {
        let mut rig = rig(None, Ipv4Addr::LOCALHOST).expect("loopback needs no token");
        let mut client = Client::connect(&rig.server);

        client.send(r#"{"type":"action","action":"deck 1 play"}"#);
        assert_eq!(client.reply(), Some(ControlResponse::Accepted));

        let command = wait_for(|| rig.engine.pop().ok());
        assert_eq!(
            command,
            Some(Command::Action(Action::Deck {
                deck: dj_core::DeckId::from_human(1).unwrap(),
                action: dj_core::action::DeckAction::Play,
            })),
            "the action never reached the engine"
        );
    }

    /// The other half of the boundary: state comes back by name, so a client
    /// never has to scrape the interface.
    #[test]
    fn parameters_come_back_by_name() {
        let rig = rig(None, Ipv4Addr::LOCALHOST).expect("loopback");
        let mut client = Client::connect(&rig.server);

        client.send(r#"{"type":"parameters"}"#);
        match client.reply() {
            Some(ControlResponse::Parameters { values }) => {
                assert!(values.len() > 100, "only {} parameters", values.len());
                assert!(
                    values.iter().any(|p| p.name == "deck.1.playing"),
                    "a well-known parameter is missing"
                );
            }
            other => panic!("expected parameters, got {other:?}"),
        }
    }

    /// Untrusted text is *parsed*, not trusted. Nonsense is refused with a
    /// code a client can branch on, and the connection stays up -- a typo
    /// should not cost a lighting desk its session.
    #[test]
    fn nonsense_is_refused_by_code_and_the_connection_survives() {
        let mut rig = rig(None, Ipv4Addr::LOCALHOST).expect("loopback");
        let mut client = Client::connect(&rig.server);

        client.send("this is not json");
        assert!(matches!(
            client.reply(),
            Some(ControlResponse::Error {
                code: ErrorCode::BadRequest,
                ..
            })
        ));

        client.send(r#"{"type":"action","action":"deck 1 levitate"}"#);
        assert!(matches!(
            client.reply(),
            Some(ControlResponse::Error {
                code: ErrorCode::BadAction,
                ..
            })
        ));

        // Still usable, which is the half that matters.
        client.send(r#"{"type":"action","action":"deck 1 play"}"#);
        assert_eq!(client.reply(), Some(ControlResponse::Accepted));
        assert!(wait_for(|| rig.engine.pop().ok()).is_some());
    }

    /// **The rule the module exists to enforce.** A socket facing a room
    /// without a key is a stranger's hand on the crossfader.
    #[test]
    fn a_non_loopback_address_will_not_start_without_a_token() {
        let refused = rig(None, Ipv4Addr::UNSPECIFIED).err();
        assert!(
            matches!(refused, Some(ServerError::TokenRequired(_))),
            "a public bind started with no token: {refused:?}"
        );
        // An empty token is not a token.
        assert!(matches!(
            rig(Some(""), Ipv4Addr::UNSPECIFIED).err(),
            Some(ServerError::TokenRequired(_))
        ));
        // With one, it is allowed -- binding 0.0.0.0 is the DJ's decision.
        assert!(rig(Some("a-real-token"), Ipv4Addr::UNSPECIFIED).is_ok());
    }

    /// When a token is set, nothing happens until it is offered -- and a wrong
    /// one ends the connection rather than inviting another guess.
    #[test]
    fn a_token_gates_every_action_before_the_greeting() {
        let mut rig = rig(Some("open-sesame"), Ipv4Addr::LOCALHOST).expect("loopback");

        let mut impatient = Client::connect(&rig.server);
        impatient.send(r#"{"type":"action","action":"deck 1 play"}"#);
        assert!(
            matches!(
                impatient.reply(),
                Some(ControlResponse::Error {
                    code: ErrorCode::Unauthorised,
                    ..
                })
            ),
            "an action was taken before the token"
        );
        assert!(
            rig.engine.pop().is_err(),
            "an un-greeted action reached the engine"
        );

        let mut wrong = Client::connect(&rig.server);
        wrong.send(r#"{"type":"hello","token":"guess"}"#);
        assert!(matches!(
            wrong.reply(),
            Some(ControlResponse::Error {
                code: ErrorCode::Unauthorised,
                ..
            })
        ));

        let mut right = Client::connect(&rig.server);
        right.send(r#"{"type":"hello","token":"open-sesame"}"#);
        assert_eq!(right.reply(), Some(ControlResponse::Accepted));
        right.send(r#"{"type":"action","action":"deck 1 play"}"#);
        assert_eq!(right.reply(), Some(ControlResponse::Accepted));
        assert!(wait_for(|| rig.engine.pop().ok()).is_some());
    }

    /// Dropping the server frees the port, so a DJ can switch it off and on
    /// again without restarting the application.
    #[test]
    fn stopping_the_server_gives_the_port_back() {
        let rig = rig(None, Ipv4Addr::LOCALHOST).expect("loopback");
        let address = rig.server.address();
        assert!(TcpStream::connect(address).is_ok());

        drop(rig);
        // The listener is closed by the drop, so this must fail rather than
        // hang. A short retry covers the moment the OS takes to reap it.
        let gone = wait_for(|| TcpStream::connect(address).err().map(|_| ()));
        assert!(gone.is_some(), "the port was still open after a stop");
    }

    /// **A runaway script must hit a wall.** The bus is bounded so a flood
    /// cannot reach the audio thread either way; this is about the rest of the
    /// process not spending its evening parsing frames instead of drawing
    /// waveforms.
    #[test]
    fn a_client_that_floods_is_told_to_slow_down() {
        let rig = rig(None, Ipv4Addr::LOCALHOST).expect("loopback");
        let mut client = Client::connect(&rig.server);

        let mut accepted = 0usize;
        let mut refused = 0usize;
        // Comfortably past the burst, sent as fast as a socket allows.
        for _ in 0..(BURST as usize + 200) {
            client.send(r#"{"type":"parameters"}"#);
            match client.reply() {
                Some(ControlResponse::Error {
                    code: ErrorCode::TooFast,
                    ..
                }) => refused += 1,
                Some(_) => accepted += 1,
                None => break,
            }
        }
        assert!(refused > 0, "{accepted} accepted and nothing refused");
        // The burst is honoured -- a scene change firing a dozen actions at
        // once is normal and must not be throttled.
        assert!(
            accepted >= 100,
            "only {accepted} got through; the burst is too tight to use"
        );

        // And the connection is still there: this is a client to slow down,
        // not one to throw out.
        std::thread::sleep(std::time::Duration::from_millis(300));
        client.send(r#"{"type":"parameters"}"#);
        assert!(
            matches!(client.reply(), Some(ControlResponse::Parameters { .. })),
            "the connection was dropped instead of throttled"
        );
    }

    /// The bucket refills, or a client is throttled for the rest of the night
    /// after one busy moment.
    #[test]
    fn the_budget_comes_back_with_time() {
        let mut bucket = Bucket::new(60.0, 10.0);
        let start = std::time::Instant::now();
        for _ in 0..10 {
            assert!(bucket.take(start), "the burst was not honoured");
        }
        assert!(!bucket.take(start), "it never ran out");

        // A second later, sixty tokens' worth of time has passed, capped at
        // the burst.
        let later = start + std::time::Duration::from_secs(1);
        for _ in 0..10 {
            assert!(bucket.take(later), "the bucket did not refill");
        }
        assert!(!bucket.take(later), "it refilled past its burst");
    }

    /// Poll until a thing happens, so a test is not a race in disguise.
    fn wait_for<T>(mut ready: impl FnMut() -> Option<T>) -> Option<T> {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(5) {
            if let Some(value) = ready() {
                return Some(value);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        ready()
    }
}

//! OSC: the protocol TouchOSC, Lemur and QLab already speak.
//!
//! # Why this and not another JSON socket
//!
//! Because a DJ with an iPad running TouchOSC has a control surface already,
//! and it sends OSC. Nothing here is a new capability -- an OSC packet becomes
//! the same [`dj_core::Action`] a pad sends, through the same bounded bus. It
//! is a second door onto the room the line protocol already opens.
//!
//! # The address is the action
//!
//! djmanzo does not invent an address space. The action grammar *is* the
//! address space, with the spaces written as slashes:
//!
//! ```text
//!   deck 1 play          ->  /deck/1/play
//!   deck 1 volume 0.4    ->  /deck/1/volume        , f 0.4
//!   crossfader -1        ->  /crossfader           , f -1.0
//! ```
//!
//! A float argument becomes the action's last word. That is the whole
//! translation, and it is what makes a TouchOSC layout readable next to a
//! controller mapping: `/deck/1/volume` is `deck 1 volume`, and nothing has to
//! be looked up.
//!
//! # What is deliberately not here
//!
//! **No bundles, no pattern matching, no queries.** Bundles exist to make
//! several messages take effect together, which matters for a lighting cue and
//! not for a fader. Pattern matching (`/deck/*/play`) would let one packet
//! start every deck, which is a way to end a set rather than to run one. Both
//! are refusable additions later; neither is missed now.
//!
//! Replies are not sent. OSC over UDP has no connection to reply on, and a
//! surface that needed state should read it over the line protocol, which can
//! answer.

use dj_core::Action;

/// What an OSC packet could not be turned into.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OscError {
    #[error("the packet is not an OSC message")]
    NotAMessage,
    #[error("the address {0:?} is not an action djmanzo has")]
    BadAddress(String),
    #[error("the packet is malformed at byte {0}")]
    Malformed(usize),
    #[error("bundles are not accepted; send the messages separately")]
    Bundle,
}

/// A parsed OSC message: an address and however many floats came with it.
///
/// Only floats and ints are kept. A surface that sends a string where a number
/// belongs is a surface misconfigured, and inventing a number for it would
/// hide that.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub address: String,
    pub arguments: Vec<f32>,
}

/// Everything in OSC is padded to a multiple of four bytes.
const ALIGN: usize = 4;

fn padded(len: usize) -> usize {
    len.div_ceil(ALIGN) * ALIGN
}

/// Read one OSC string: NUL-terminated, then padded to four bytes.
fn read_string(bytes: &[u8], at: usize) -> Result<(String, usize), OscError> {
    let rest = bytes.get(at..).ok_or(OscError::Malformed(at))?;
    let end = rest
        .iter()
        .position(|b| *b == 0)
        .ok_or(OscError::Malformed(at))?;
    let text = std::str::from_utf8(&rest[..end])
        .map_err(|_| OscError::Malformed(at))?
        .to_owned();
    // The NUL is part of the string's length before padding.
    Ok((text, at + padded(end + 1)))
}

/// Parse one OSC packet.
///
/// # Errors
/// When it is a bundle, when it is not an OSC message at all, or when it is
/// truncated.
pub fn parse(packet: &[u8]) -> Result<Message, OscError> {
    if packet.starts_with(b"#bundle") {
        return Err(OscError::Bundle);
    }
    if !packet.starts_with(b"/") {
        return Err(OscError::NotAMessage);
    }

    let (address, at) = read_string(packet, 0)?;

    // The type tag string is optional in the wild, though the specification
    // asks for it. A message with no tags is an address on its own, which is
    // exactly what `/deck/1/play` is.
    let Ok((tags, mut at)) = read_string(packet, at) else {
        return Ok(Message {
            address,
            arguments: Vec::new(),
        });
    };
    if !tags.starts_with(',') {
        return Ok(Message {
            address,
            arguments: Vec::new(),
        });
    }

    let mut arguments = Vec::new();
    for tag in tags.chars().skip(1) {
        match tag {
            'f' | 'i' => {
                let word: [u8; 4] = packet
                    .get(at..at + 4)
                    .ok_or(OscError::Malformed(at))?
                    .try_into()
                    .map_err(|_| OscError::Malformed(at))?;
                // Both are big-endian, which OSC calls network byte order.
                #[allow(clippy::cast_precision_loss)]
                let value = if tag == 'f' {
                    f32::from_be_bytes(word)
                } else {
                    i32::from_be_bytes(word) as f32
                };
                arguments.push(value);
                at += 4;
            }
            // A string argument is skipped rather than refused: a surface may
            // label a message, and a label is not a reason to drop the action.
            's' => at = read_string(packet, at)?.1,
            // `T`/`F`/`N`/`I` carry no bytes.
            'T' => arguments.push(1.0),
            'F' => arguments.push(0.0),
            'N' | 'I' => {}
            // Anything else has a length this does not know, so the rest of
            // the packet can no longer be located.
            _ => break,
        }
    }

    Ok(Message { address, arguments })
}

/// Turn an OSC message into the action it names.
///
/// `/deck/1/volume` with `0.4` is `deck 1 volume 0.4`. The address is the
/// action with slashes for spaces, so nothing has to be looked up.
///
/// # Errors
/// When the address does not name an action djmanzo has.
pub fn to_action(message: &Message) -> Result<Action, OscError> {
    let mut text = message.address.trim_matches('/').replace('/', " ");
    // One argument becomes the action's last word. More than one is a surface
    // sending a pair where djmanzo wants a number, and the first is the one it
    // means -- an XY pad's other axis belongs to a different action.
    if let Some(first) = message.arguments.first() {
        // Trimmed rather than left at `0.4000000059604645`, which is what a
        // 32-bit float prints as and not what a mapping file would write.
        text.push(' ');
        text.push_str(&trim(*first));
    }
    Action::parse(&text).map_err(|_| OscError::BadAddress(message.address.clone()))
}

/// A float as a mapping file would write it.
fn trim(value: f32) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// An OSC listener. Dropping it closes the port.
///
/// UDP, because that is what OSC is and what every surface sends. It means
/// there is nobody to refuse: a packet either becomes an action or is dropped,
/// and a mistyped address is only visible in the log. That is the protocol's
/// bargain, not a shortcut — the line protocol is there for a client that
/// wants an answer.
#[derive(Debug)]
pub struct OscServer {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    address: std::net::SocketAddr,
}

impl OscServer {
    /// Listen on `address` and dispatch what arrives.
    ///
    /// **Loopback or a token — the same rule the line protocol keeps**, and
    /// for a stronger reason: UDP has no handshake, so a token cannot be
    /// offered once and remembered. There is nothing to authenticate *with*,
    /// which is why an OSC port that faces the network is refused outright
    /// rather than protected badly.
    ///
    /// # Errors
    /// When the address cannot be bound, or when it is not loopback.
    pub fn start<C>(
        address: std::net::SocketAddr,
        service: std::sync::Arc<crate::ControlService<C>>,
    ) -> Result<Self, crate::ServerError>
    where
        C: From<Action> + Send + 'static,
    {
        if !address.ip().is_loopback() {
            return Err(crate::ServerError::TokenRequired(address.ip()));
        }
        let socket = std::net::UdpSocket::bind(address)
            .map_err(|e| crate::ServerError::Listen(address, e.to_string()))?;
        // So the loop can notice it has been asked to stop.
        socket
            .set_read_timeout(Some(std::time::Duration::from_millis(200)))
            .map_err(|e| crate::ServerError::Listen(address, e.to_string()))?;
        let address = socket
            .local_addr()
            .map_err(|e| crate::ServerError::Listen(address, e.to_string()))?;

        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("djmanzo-osc".into())
            .spawn({
                let stop = std::sync::Arc::clone(&stop);
                move || {
                    // The largest packet OSC is likely to send. A surface
                    // sending more than this is sending a bundle, which is
                    // refused anyway.
                    let mut buffer = [0u8; 2048];
                    while !stop.load(std::sync::atomic::Ordering::Acquire) {
                        let Ok((read, _from)) = socket.recv_from(&mut buffer) else {
                            continue; // the timeout, or a packet that vanished
                        };
                        let Ok(message) = parse(&buffer[..read]) else {
                            continue;
                        };
                        match to_action(&message) {
                            Ok(action) => {
                                let _ = service.dispatch(action);
                            }
                            Err(error) => {
                                // Said once per packet at a rate a surface
                                // sets, which is why it is a debug line and
                                // not a warning.
                                tracing::debug!(%error, "an OSC packet was dropped");
                            }
                        }
                    }
                }
            })
            .map_err(|e| crate::ServerError::Thread(e.to_string()))?;

        Ok(Self {
            stop,
            thread: Some(thread),
            address,
        })
    }

    /// Where it ended up, which is what port 0 is for.
    #[must_use]
    pub fn address(&self) -> std::net::SocketAddr {
        self.address
    }
}

impl Drop for OscServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a packet the way a surface would.
    fn packet(address: &str, tags: &str, args: &[f32]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut push = |text: &str| {
            out.extend_from_slice(text.as_bytes());
            out.push(0);
            while out.len() % ALIGN != 0 {
                out.push(0);
            }
        };
        push(address);
        if !tags.is_empty() {
            push(tags);
        }
        for arg in args {
            out.extend_from_slice(&arg.to_be_bytes());
        }
        out
    }

    /// **The whole design.** The address is the action, so a TouchOSC layout
    /// reads like a controller mapping.
    #[test]
    fn the_address_is_the_action() {
        let message = parse(&packet("/deck/1/play", "", &[])).expect("a message");
        assert_eq!(message.address, "/deck/1/play");
        assert_eq!(
            to_action(&message).expect("an action"),
            Action::parse("deck 1 play").unwrap()
        );
    }

    /// A float becomes the action's last word, which is how a fader works.
    #[test]
    fn a_float_becomes_the_actions_last_word() {
        let message = parse(&packet("/deck/1/volume", ",f", &[0.4])).expect("a message");
        assert_eq!(message.arguments, vec![0.4]);
        assert_eq!(
            to_action(&message).expect("an action"),
            Action::parse("deck 1 volume 0.4").unwrap()
        );
    }

    /// A negative float, because the crossfader runs -1 to 1 and a sign lost
    /// in formatting would send it the wrong way.
    #[test]
    fn a_negative_argument_keeps_its_sign() {
        let message = parse(&packet("/crossfader", ",f", &[-1.0])).expect("a message");
        assert_eq!(
            to_action(&message).expect("an action"),
            Action::parse("crossfader -1").unwrap()
        );
    }

    /// 32-bit floats do not print the way a person writes them. `0.4` is
    /// `0.4000000059604645` as an `f32` widened to `f64`, and an action text
    /// carrying that is not one a mapping file would ever contain.
    #[test]
    fn a_float_is_written_the_way_a_mapping_file_would() {
        assert_eq!(trim(0.4), "0.4");
        assert_eq!(trim(1.0), "1");
        assert_eq!(trim(-1.0), "-1");
        assert_eq!(trim(0.0), "0");
        assert_eq!(trim(0.123_456_79), "0.123457");
    }

    /// Ints are floats as far as the action grammar is concerned -- a surface
    /// with an integer fader is still a fader.
    #[test]
    fn an_int_argument_is_read_as_a_number() {
        let mut bytes = packet("/deck/1/volume", ",i", &[]);
        bytes.extend_from_slice(&1i32.to_be_bytes());
        let message = parse(&bytes).expect("a message");
        assert_eq!(message.arguments, vec![1.0]);
    }

    /// An address djmanzo does not have is refused by name, so a mistyped
    /// TouchOSC layout says which control is wrong.
    #[test]
    fn an_address_that_is_not_an_action_says_which() {
        let message = parse(&packet("/deck/1/levitate", "", &[])).expect("a message");
        assert_eq!(
            to_action(&message),
            Err(OscError::BadAddress("/deck/1/levitate".to_owned()))
        );
    }

    /// **Bundles are refused rather than half-read.** A bundle exists to make
    /// several messages take effect together, and quietly applying the first
    /// one would be a scene change that half happened.
    #[test]
    fn a_bundle_is_refused_rather_than_partly_applied() {
        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend_from_slice(&[0; 8]); // timetag
        bundle.extend_from_slice(&packet("/deck/1/play", "", &[]));
        assert_eq!(parse(&bundle), Err(OscError::Bundle));
    }

    /// Rubbish on a UDP port is normal -- anything can send to it. Truncated
    /// and non-OSC packets are refused rather than panicking.
    #[test]
    fn rubbish_on_the_port_is_refused_rather_than_fatal() {
        assert_eq!(parse(b""), Err(OscError::NotAMessage));
        assert_eq!(parse(b"hello"), Err(OscError::NotAMessage));
        // An address with no terminator.
        assert!(matches!(parse(b"/deck/1"), Err(OscError::Malformed(_))));
        // Tags promising a float that is not there.
        let mut short = packet("/deck/1/volume", ",f", &[]);
        short.truncate(short.len().saturating_sub(0));
        assert!(matches!(parse(&short), Err(OscError::Malformed(_))));
    }

    /// A surface labelling its message must not lose the action.
    #[test]
    fn a_string_argument_is_stepped_over() {
        let mut bytes = packet("/deck/1/volume", ",sf", &[]);
        bytes.extend_from_slice(b"fader\0\0\0");
        bytes.extend_from_slice(&0.5f32.to_be_bytes());
        let message = parse(&bytes).expect("a message");
        assert_eq!(message.arguments, vec![0.5]);
    }

    /// **The point of the adapter.** A packet from something that is not this
    /// process becomes an action in the engine.
    #[test]
    fn a_packet_from_outside_reaches_the_engine() {
        use dj_control::{ActionBus, ParameterRegistry};
        use std::sync::Arc;

        #[derive(Debug, Clone, Copy, PartialEq)]
        enum Command {
            Action(Action),
        }
        impl From<Action> for Command {
            fn from(value: Action) -> Self {
                Self::Action(value)
            }
        }

        let (bus, mut engine) = ActionBus::<Command>::new(64);
        let service = Arc::new(crate::ControlService::new(
            Arc::new(bus),
            Arc::new(ParameterRegistry::new()),
        ));
        let server = OscServer::start(
            std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 0)),
            service,
        )
        .expect("loopback binds");

        let client = std::net::UdpSocket::bind("127.0.0.1:0").expect("a client socket");
        client
            .send_to(&packet("/deck/1/volume", ",f", &[0.4]), server.address())
            .expect("send");

        let start = std::time::Instant::now();
        let got = loop {
            if let Ok(command) = engine.pop() {
                break Some(command);
            }
            if start.elapsed() > std::time::Duration::from_secs(5) {
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(
            got,
            Some(Command::Action(Action::parse("deck 1 volume 0.4").unwrap())),
            "the packet never became an action"
        );
    }

    /// **UDP has no handshake, so a token cannot be offered and remembered.**
    /// There is nothing to authenticate with, which is why an OSC port facing
    /// the network is refused outright rather than protected badly.
    #[test]
    fn an_osc_port_will_not_face_the_network_at_all() {
        use dj_control::{ActionBus, ParameterRegistry};
        use std::sync::Arc;

        #[derive(Debug)]
        enum Command {
            #[allow(dead_code)]
            Action(Action),
        }
        impl From<Action> for Command {
            fn from(value: Action) -> Self {
                Self::Action(value)
            }
        }

        let (bus, _engine) = ActionBus::<Command>::new(8);
        let service = Arc::new(crate::ControlService::new(
            Arc::new(bus),
            Arc::new(ParameterRegistry::new()),
        ));
        let refused = OscServer::start(
            std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0)),
            service,
        )
        .err();
        assert!(
            matches!(refused, Some(crate::ServerError::TokenRequired(_))),
            "a public OSC bind was allowed: {refused:?}"
        );
    }

    /// Every action in the grammar has to be reachable as an address, or the
    /// adapter is a subset with no way to tell which part.
    #[test]
    fn a_representative_action_from_each_family_round_trips() {
        for text in [
            "deck 1 play",
            "deck 2 cue",
            "deck 1 sync",
            "crossfader 0.5",
            "deck 1 eq_low 0.25",
            "deck 1 hotcue 3",
            "deck 1 stem_mute_on vocal",
        ] {
            let mut words: Vec<&str> = text.split(' ').collect();
            // A trailing number becomes the float argument, as a surface sends it.
            let argument = words
                .last()
                .and_then(|w| w.parse::<f32>().ok())
                .inspect(|_| {
                    words.pop();
                });
            let address = format!("/{}", words.join("/"));
            let message = Message {
                address: address.clone(),
                arguments: argument.into_iter().collect(),
            };
            assert!(
                to_action(&message).is_ok(),
                "{address} did not become an action, but {text} is one"
            );
        }
    }
}

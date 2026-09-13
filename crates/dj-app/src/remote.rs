//! The network control server, as the application switches it on and off.
//!
//! `dj-net` holds the protocol and the socket; this holds the decision to open
//! one. Three rules, and they are the reason this is a module rather than two
//! lines in `commands.rs`:
//!
//! - **Off unless asked.** A DJ laptop on a club's wifi is not a place to open
//!   a port by default, and nobody reads a changelog before a set.
//! - **Loopback unless told otherwise.** The common case -- a script, a
//!   Stream Deck plugin, a page on the same machine -- needs nothing more.
//! - **A token past loopback.** Enforced in `dj_net::ControlServer::start`, so
//!   it cannot be forgotten here.

use dj_engine::Command;
use dj_net::{Carry, ControlServer, ControlService};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// Where an action from a socket goes: djmanzo's own entry point.
///
/// **Not the bus.** `commands::perform_action` is where several verbs stop
/// meaning what the engine thinks they mean — `eject` clears the deck's name
/// and its analysis, which live in the application; `record on` opens a file,
/// which the engine cannot do at all. Dispatching straight at the bus, which is
/// what this did, ejected in the engine and left the interface showing a record
/// that was no longer on the deck, and answered "accepted" to a recording it had
/// not started.
///
/// §87 names it: the resulting state must be identical whichever origin a
/// command came from, and the network was the one origin not going through the
/// application. It goes through it now, which also means a socket obeys §72's
/// takeover and every other interception that path carries, for free.
#[derive(Debug)]
struct Booth(tauri::AppHandle);

/// Somewhere for a socket to put an action: this application.
#[must_use]
pub fn booth(app: tauri::AppHandle) -> Arc<dyn Carry> {
    Arc::new(Booth(app))
}

/// A carrier for tests that are about the socket rather than about the action.
///
/// Accepts and drops. These tests ask whether a port opens, closes and rebinds;
/// what an action then does is `commands`' business and is tested there.
#[cfg(test)]
#[derive(Debug)]
struct Nowhere;

#[cfg(test)]
impl Carry for Nowhere {
    fn carry(&self, _: dj_core::Action) -> Result<(), dj_net::ControlError> {
        Ok(())
    }
}

impl Carry for Booth {
    fn carry(&self, action: dj_core::Action) -> Result<(), dj_net::ControlError> {
        use tauri::Manager as _;
        let state = self.0.state::<crate::state::AppState>();
        crate::commands::perform_action(&state, action).map_err(dj_net::ControlError::Refused)
    }

    /// The whole line, because djmanzo's vocabulary is wider than `Action`'s.
    ///
    /// `load deck 1 <track-id>` is in every session file and `Action::parse`
    /// has never accepted it — a load carries an `Arc`, so it is a command
    /// rather than an action. Parsing before handing over therefore refused, at
    /// the door, the one verb §87 exists to make available from every origin.
    ///
    /// **Found by opening the port and sending the line**, after both sides of
    /// this seam were green: every test on either side was asking about
    /// actions, so none of them could see a verb that is not one.
    fn carry_line(&self, line: &str) -> Result<(), dj_net::ControlError> {
        use tauri::Manager as _;
        let state = self.0.state::<crate::state::AppState>();
        crate::commands::perform(&state, line).map_err(dj_net::ControlError::Refused)
    }
}

/// What the interface shows and sets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteStatus {
    pub running: bool,
    /// Where the OSC listener is, when one is open. Loopback only — UDP has
    /// no handshake, so there is nothing to authenticate with.
    pub osc: Option<String>,
    /// Where it is listening, once it is. Includes the port the operating
    /// system chose when the request was for port 0.
    pub address: Option<String>,
    /// Whether a token is required. The token itself is never sent back.
    pub token_set: bool,
    /// Why the last attempt to start failed, if it did.
    pub error: Option<String>,
}

/// Owns the running server, if there is one.
#[derive(Debug, Default)]
pub struct Remote {
    server: Mutex<Option<ControlServer>>,
    osc: Mutex<Option<dj_net::OscServer>>,
    error: Mutex<Option<String>>,
    token_set: std::sync::atomic::AtomicBool,
}

impl Remote {
    /// Start listening on `address`, replacing anything already running.
    ///
    /// # Errors
    /// When the address cannot be bound, or when it is not loopback and no
    /// token was given.
    pub fn start(
        &self,
        address: SocketAddr,
        token: Option<String>,
        bus: Arc<dj_control::ActionBus<Command>>,
        registry: Arc<dj_control::ParameterRegistry>,
        // Where actions go. Required rather than optional, because the way this
        // breaks is a caller forgetting it and silently reopening the hole §87
        // found -- a socket that reaches the engine without reaching the
        // application.
        carrier: Arc<dyn Carry>,
    ) -> Result<RemoteStatus, String> {
        // Stopped first, so restarting on the same port does not fail to bind
        // against the copy of itself that is still listening.
        self.stop();

        let token_set = token.as_deref().is_some_and(|t| !t.is_empty());
        let service = Arc::new(ControlService::new(bus, registry).carried_by(carrier));
        match ControlServer::start(address, token, service) {
            Ok(server) => {
                let status = RemoteStatus {
                    running: true,
                    address: Some(server.address().to_string()),
                    token_set,
                    error: None,
                    osc: self.osc_address(),
                };
                *self.server.lock().unwrap() = Some(server);
                *self.error.lock().unwrap() = None;
                self.token_set
                    .store(token_set, std::sync::atomic::Ordering::Release);
                Ok(status)
            }
            Err(why) => {
                let why = why.to_string();
                *self.error.lock().unwrap() = Some(why.clone());
                Err(why)
            }
        }
    }

    /// Stop listening. Stopping nothing is not an error.
    pub fn stop(&self) {
        // Dropping the server joins its accept thread and closes the port.
        *self.server.lock().unwrap() = None;
    }

    #[must_use]
    pub fn status(&self) -> RemoteStatus {
        let server = self.server.lock().unwrap();
        RemoteStatus {
            running: server.is_some(),
            address: server.as_ref().map(|s| s.address().to_string()),
            token_set: self.token_set.load(std::sync::atomic::Ordering::Acquire),
            error: self.error.lock().unwrap().clone(),
            osc: self.osc_address(),
        }
    }

    fn osc_address(&self) -> Option<String> {
        self.osc
            .lock()
            .ok()?
            .as_ref()
            .map(|s| s.address().to_string())
    }

    /// Open an OSC port, replacing anything already listening.
    ///
    /// Loopback only, and that is not a default: UDP has no handshake, so a
    /// token cannot be offered once and remembered. There is nothing to
    /// authenticate *with*, which is why a port facing the network is refused
    /// outright rather than protected badly.
    ///
    /// # Errors
    /// When the address cannot be bound, or is not loopback.
    pub fn start_osc(
        &self,
        address: SocketAddr,
        bus: Arc<dj_control::ActionBus<Command>>,
        registry: Arc<dj_control::ParameterRegistry>,
        // Where actions go. Required rather than optional, because the way this
        // breaks is a caller forgetting it and silently reopening the hole §87
        // found -- a socket that reaches the engine without reaching the
        // application.
        carrier: Arc<dyn Carry>,
    ) -> Result<RemoteStatus, String> {
        *self.osc.lock().unwrap() = None;
        let service = Arc::new(ControlService::new(bus, registry).carried_by(carrier));
        match dj_net::OscServer::start(address, service) {
            Ok(server) => {
                *self.osc.lock().unwrap() = Some(server);
                *self.error.lock().unwrap() = None;
                Ok(self.status())
            }
            Err(why) => {
                let why = why.to_string();
                *self.error.lock().unwrap() = Some(why.clone());
                Err(why)
            }
        }
    }

    /// Close the OSC port.
    pub fn stop_osc(&self) {
        *self.osc.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn parts() -> (
        Arc<dj_control::ActionBus<Command>>,
        Arc<dj_control::ParameterRegistry>,
        rtrb::Consumer<Command>,
    ) {
        let (bus, engine) = dj_control::ActionBus::<Command>::new(64);
        (
            Arc::new(bus),
            Arc::new(dj_control::ParameterRegistry::new()),
            engine,
        )
    }

    #[test]
    fn nothing_is_listening_until_it_is_asked_to() {
        let remote = Remote::default();
        let status = remote.status();
        assert!(!status.running, "the port was open before anybody asked");
        assert_eq!(status.address, None);
        assert_eq!(
            status.osc, None,
            "an OSC port was open before anybody asked"
        );
    }

    #[test]
    fn starting_and_stopping_is_reflected_in_the_status() {
        let remote = Remote::default();
        let (bus, registry, _engine) = parts();

        let started = remote
            .start(
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                None,
                bus,
                registry,
                Arc::new(Nowhere),
            )
            .expect("loopback needs no token");
        assert!(started.running);
        let address = started.address.clone().expect("an address");
        assert!(address.starts_with("127.0.0.1:"), "bound {address}");
        assert!(!started.token_set);
        assert_eq!(remote.status().address, started.address);

        remote.stop();
        assert!(!remote.status().running);
    }

    /// Restarting must not fail to bind against the copy of itself that was
    /// still listening -- which is what happens if the old one is dropped
    /// after the new one is created rather than before.
    #[test]
    fn restarting_on_the_same_port_works() {
        let remote = Remote::default();
        let (bus, registry, _engine) = parts();

        let first = remote
            .start(
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                None,
                Arc::clone(&bus),
                Arc::clone(&registry),
                Arc::new(Nowhere),
            )
            .expect("first start");
        let address: SocketAddr = first.address.unwrap().parse().expect("an address");

        let again = remote.start(address, None, bus, registry, Arc::new(Nowhere));
        assert!(
            again.is_ok(),
            "restarting on the same port failed: {again:?}"
        );
        assert_eq!(
            again.unwrap().address.as_deref(),
            Some(&*address.to_string())
        );
    }

    /// OSC is a second door onto the same room, opened and closed on its own.
    #[test]
    fn the_osc_port_opens_and_closes_independently_of_the_line_protocol() {
        let remote = Remote::default();
        let (bus, registry, _engine) = parts();

        let started = remote
            .start_osc(
                SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
                Arc::clone(&bus),
                Arc::clone(&registry),
                Arc::new(Nowhere),
            )
            .expect("loopback binds");
        let osc = started.osc.clone().expect("an OSC address");
        assert!(osc.starts_with("127.0.0.1:"), "bound {osc}");
        assert!(
            !started.running,
            "opening OSC also claimed the line protocol was running"
        );

        remote.stop_osc();
        assert_eq!(remote.status().osc, None);
    }

    /// UDP cannot carry a token, so a public OSC bind is refused rather than
    /// protected badly.
    #[test]
    fn an_osc_port_facing_the_network_is_refused() {
        let remote = Remote::default();
        let (bus, registry, _engine) = parts();
        let why = remote
            .start_osc(
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
                bus,
                registry,
                Arc::new(Nowhere),
            )
            .expect_err("a public OSC bind should be refused");
        assert!(why.contains("token") || why.contains("loopback"), "{why}");
        assert_eq!(remote.status().osc, None, "it opened anyway");
    }

    /// The refusal reaches the interface as a message rather than as silence,
    /// and nothing is left listening.
    #[test]
    fn a_public_bind_without_a_token_is_refused_and_said_out_loud() {
        let remote = Remote::default();
        let (bus, registry, _engine) = parts();

        let why = remote
            .start(
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
                None,
                bus,
                registry,
                Arc::new(Nowhere),
            )
            .expect_err("a public bind with no token should be refused");
        assert!(why.contains("token"), "unhelpful message: {why}");

        let status = remote.status();
        assert!(!status.running, "it started anyway");
        assert_eq!(status.error.as_deref(), Some(&*why));
    }
}

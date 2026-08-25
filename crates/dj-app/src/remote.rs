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
use dj_net::{ControlServer, ControlService};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// What the interface shows and sets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteStatus {
    pub running: bool,
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
    ) -> Result<RemoteStatus, String> {
        // Stopped first, so restarting on the same port does not fail to bind
        // against the copy of itself that is still listening.
        self.stop();

        let token_set = token.as_deref().is_some_and(|t| !t.is_empty());
        let service = Arc::new(ControlService::new(bus, registry));
        match ControlServer::start(address, token, service) {
            Ok(server) => {
                let status = RemoteStatus {
                    running: true,
                    address: Some(server.address().to_string()),
                    token_set,
                    error: None,
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
        }
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
            )
            .expect("first start");
        let address: SocketAddr = first.address.unwrap().parse().expect("an address");

        let again = remote.start(address, None, bus, registry);
        assert!(
            again.is_ok(),
            "restarting on the same port failed: {again:?}"
        );
        assert_eq!(
            again.unwrap().address.as_deref(),
            Some(&*address.to_string())
        );
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
            )
            .expect_err("a public bind with no token should be refused");
        assert!(why.contains("token"), "unhelpful message: {why}");

        let status = remote.status();
        assert!(!status.running, "it started anyway");
        assert_eq!(status.error.as_deref(), Some(&*why));
    }
}

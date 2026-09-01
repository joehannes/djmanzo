//! Answering to a name on the local network, so a sticker can be printed
//! before anybody knows what the venue's router will hand out.
//!
//! # What this actually does
//!
//! It runs multicast DNS: it joins the local group, and when a phone on the
//! same network asks "who is `djmanzo.local`?", it answers with this machine's
//! address. That is the whole trick, and it is what makes
//! `http://djmanzo.local:7331/` a URL that can be printed once and used in
//! every venue.
//!
//! It also registers as an `_http._tcp` service, which costs nothing extra and
//! makes djmanzo visible to anything that browses for local web services.
//!
//! # What it cannot do
//!
//! Make every phone resolve the name. Apple devices have done `.local` since
//! Bonjour; Android added it in 12 and still misses it on some builds and in
//! some browsers. This is why [`crate::sticker`] offers the plain address too
//! and prints the caveat rather than hiding it: the honest statement is "most
//! phones", and a DJ printing two hundred stickers deserves to be told which
//! word that is.
//!
//! # Why failure here is not failure
//!
//! Guest networks block multicast, and some do it silently. So the announcer
//! failing to start stops nothing: the server is already listening, the plain
//! address already works, and the QR on screen already carries it. The failure
//! is reported and the night continues.

use std::net::IpAddr;

/// The service type djmanzo registers under. A web page, over TCP.
const SERVICE: &str = "_http._tcp.local.";

/// The instance name, which is what a service browser lists.
const INSTANCE: &str = "djmanzo";

#[derive(Debug, thiserror::Error)]
pub enum AnnounceError {
    #[error("could not start the local-name responder: {0}")]
    Start(String),
    #[error("could not answer to {0}: {1}")]
    Register(String, String),
}

/// Answers to [`crate::sticker::LOCAL_NAME`] for as long as it is alive.
///
/// Dropping it withdraws the name. The daemon runs its own thread; nothing
/// here is on any path the audio thread touches.
pub struct Announcer {
    daemon: mdns_sd::ServiceDaemon,
    name: String,
}

impl std::fmt::Debug for Announcer {
    /// The daemon is not `Debug`, and what a reader wants here is the name
    /// anyway.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Announcer")
            .field("name", &self.name)
            .finish()
    }
}

impl Announcer {
    /// Start answering for `port` at this machine's `address`.
    ///
    /// # Errors
    /// When the responder cannot start — typically a network that blocks
    /// multicast, which is a thing to report and not a thing to stop for.
    pub fn start(address: IpAddr, port: u16) -> Result<Self, AnnounceError> {
        let daemon =
            mdns_sd::ServiceDaemon::new().map_err(|why| AnnounceError::Start(why.to_string()))?;

        // DNS names end in a dot; the one in a URL does not. Both spellings
        // are correct in their own place, which is exactly how they get
        // confused, so the conversion happens here and nowhere else.
        let host = format!("{}.", crate::sticker::LOCAL_NAME);
        let service = mdns_sd::ServiceInfo::new(
            SERVICE, INSTANCE, &host, address, port,
            // A phone reading a QR code needs no properties, and anything put
            // here is broadcast to the whole network all night.
            None,
        )
        .map_err(|why| AnnounceError::Register(host.clone(), why.to_string()))?;

        daemon
            .register(service)
            .map_err(|why| AnnounceError::Register(host.clone(), why.to_string()))?;

        Ok(Self {
            daemon,
            name: crate::sticker::LOCAL_NAME.to_owned(),
        })
    }

    /// The name being answered for, without the trailing dot.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Announcer {
    fn drop(&mut self) {
        // Withdraws the name and stops the daemon's thread. The receiver it
        // hands back reports when that finished; nothing here needs to wait,
        // and waiting on a network that is already refusing multicast is a way
        // to hang a shutdown.
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Starting and stopping the responder leaves nothing behind.**
    ///
    /// Deliberately tolerant of not starting at all: a container without
    /// multicast is a network this is expected to fail on, and the point of
    /// the test is that failing is a `Result` rather than a panic or a thread
    /// that outlives the drop.
    #[test]
    fn the_responder_starts_and_stops_or_says_why() {
        let address: IpAddr = "127.0.0.1".parse().expect("address");
        match Announcer::start(address, crate::sticker::DEFAULT_PORT) {
            Ok(announcer) => {
                assert_eq!(announcer.name(), "djmanzo.local");
                drop(announcer);
            }
            Err(why) => {
                // Reported, not swallowed -- the interface shows this sentence.
                assert!(!why.to_string().is_empty());
            }
        }
    }
}

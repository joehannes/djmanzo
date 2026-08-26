//! Tempo sync between djmanzo instances, as the application switches it on.
//!
//! `dj-net::peer` holds the protocol and the socket; this holds the decision to
//! open one, and the thread that keeps it fed. The same shape as
//! [`crate::remote`] and [`crate::clock`], for the same reasons: off unless
//! asked, and the thread is a plain one rather than the audio callback, because
//! announcing is I/O and a callback firing in 5.3 ms lumps would jitter it.
//!
//! # What it syncs to
//!
//! Other djmanzo instances. **Not Ableton Link** — see `dj_net::peer` for why
//! that is a separate item rather than an omission.

use dj_control::ParameterRegistry;
use dj_core::{Bpm, GlobalParam, ParamId};
use dj_net::peer::{LocalTempo, PeerSync};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often a peer says where it is.
///
/// Ten times a second. Often enough that a peer joining is noticed within a
/// beat, rare enough that a room full of them is not a broadcast storm — and
/// well inside `dj_net::peer`'s two-second timeout, so a single lost datagram
/// never looks like a peer leaving.
const ANNOUNCE_INTERVAL: Duration = Duration::from_millis(100);

/// What the interface shows and sets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerStatus {
    pub running: bool,
    /// Where this peer is listening, once it is.
    pub address: Option<String>,
    /// Where announcements are sent — a broadcast address, or one peer.
    pub send_to: Option<String>,
    /// How many other instances are on the network.
    pub peers: usize,
    /// The tempo the peers have settled on, when one of them is playing.
    pub peer_bpm: Option<f64>,
    /// Why the last attempt to start failed, if it did.
    pub error: Option<String>,
}

/// Owns the running peer thread, if there is one.
#[derive(Debug, Default)]
pub struct Peers {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    error: Mutex<Option<String>>,
    address: Mutex<Option<String>>,
    send_to: Mutex<Option<String>>,
    /// Live figures, written by the peer thread and read by the interface.
    ///
    /// Atomics behind an `Arc` rather than a mutex, for the reason the
    /// parameter registry uses them: a reader must never wait on a writer to
    /// draw a frame. Shared with the thread rather than reached for through
    /// `&self`, so the thread borrows nothing from this struct.
    live: Arc<Live>,
}

/// What the peer thread publishes for the interface.
#[derive(Debug, Default)]
struct Live {
    peers: AtomicUsize,
    /// The peers' tempo in thousandths of a BPM, or zero for "nobody playing".
    ///
    /// An integer because there is no atomic `f64`, and thousandths because a
    /// tempo is never shown to more than two decimal places.
    bpm: AtomicU64,
}

impl Peers {
    /// Start announcing and following.
    ///
    /// # Errors
    /// When the address cannot be bound.
    pub fn start(
        &self,
        listen: SocketAddr,
        send_to: SocketAddr,
        registry: Arc<ParameterRegistry>,
    ) -> Result<PeerStatus, String> {
        // Stopped first, so restarting on the same port does not fail to bind
        // against the copy of itself that is still listening.
        self.stop();

        // A fresh name each time it starts. Two instances on one machine must
        // not share one, or each would take the other's announcements for its
        // own and ignore them.
        let id = format!(
            "djmanzo-{:016x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64)
                ^ std::process::id() as u64
        );

        let sync = PeerSync::bind(listen, send_to, id).map_err(|e| {
            let message = e.to_string();
            *self.error.lock().unwrap_or_else(|p| p.into_inner()) = Some(message.clone());
            message
        })?;
        let bound = sync
            .address()
            .map_or_else(|_| listen.to_string(), |a| a.to_string());

        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("djmanzo-peers".into())
            .spawn({
                let stop = Arc::clone(&stop);
                let live = Arc::clone(&self.live);
                move || pump(sync, &registry, &stop, &live)
            })
            .map_err(|e| e.to_string())?;

        *self.stop.lock().unwrap_or_else(|p| p.into_inner()) = Some(stop);
        *self.thread.lock().unwrap_or_else(|p| p.into_inner()) = Some(thread);
        *self.address.lock().unwrap_or_else(|p| p.into_inner()) = Some(bound);
        *self.send_to.lock().unwrap_or_else(|p| p.into_inner()) = Some(send_to.to_string());
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = None;
        Ok(self.status())
    }

    /// Stop announcing, and wait for the thread to notice.
    pub fn stop(&self) {
        if let Some(stop) = self.stop.lock().unwrap_or_else(|p| p.into_inner()).take() {
            stop.store(true, Ordering::Release);
        }
        if let Some(thread) = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = thread.join();
        }
        *self.address.lock().unwrap_or_else(|p| p.into_inner()) = None;
        *self.send_to.lock().unwrap_or_else(|p| p.into_inner()) = None;
        self.live.peers.store(0, Ordering::Relaxed);
        self.live.bpm.store(0, Ordering::Relaxed);
    }

    /// What the interface shows.
    #[must_use]
    pub fn status(&self) -> PeerStatus {
        let thousandths = self.live.bpm.load(Ordering::Relaxed);
        // Each mutex is taken **once**, into a local.
        //
        // Written inline in the struct literal, this read `self.address` twice
        // -- once for `running` and once for `address` -- and deadlocked
        // against itself every time: a guard built inside a struct expression
        // is a temporary that lives to the end of the whole statement, and
        // `std::sync::Mutex` is not reentrant. Every test that asked for a
        // status hung, which is what a DJ opening the panel would have got.
        let address = self
            .address
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let send_to = self
            .send_to
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let error = self.error.lock().unwrap_or_else(|p| p.into_inner()).clone();
        PeerStatus {
            running: address.is_some(),
            address,
            send_to,
            peers: self.live.peers.load(Ordering::Relaxed),
            #[allow(clippy::cast_precision_loss)]
            peer_bpm: (thousandths > 0).then(|| thousandths as f64 / 1000.0),
            error,
        }
    }
}

/// The peer thread: announce on a schedule, poll continuously.
fn pump(mut sync: PeerSync, registry: &ParameterRegistry, stop: &AtomicBool, live: &Live) {
    let mut last = Instant::now();
    let mut next_announcement = Instant::now();

    while !stop.load(Ordering::Acquire) {
        // Polled far more often than announcements go out, so a peer's
        // correction is applied when it arrives rather than at our own cadence.
        std::thread::sleep(Duration::from_millis(10));
        let now = Instant::now();
        let elapsed = now.duration_since(last).as_secs_f64();
        last = now;

        let local = local_tempo(registry);

        if now >= next_announcement {
            let _ = sync.announce(local);
            next_announcement = now + ANNOUNCE_INTERVAL;
        }

        let advice = sync.poll(elapsed, local);
        live.peers.store(advice.peers, Ordering::Relaxed);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        live.bpm.store(
            advice.tempo.map_or(0, |bpm| (bpm.get() * 1000.0) as u64),
            Ordering::Relaxed,
        );
        // The nudge is published rather than dispatched.
        //
        // Deliberately: a peer correction is a *pitch* change, and the
        // decision to let the network move a deck's pitch belongs to the DJ
        // who can see the pitch fader, not to a thread that cannot. Sync to
        // the peers is opt-in per deck through the ordinary sync verbs, the
        // same way following a MIDI clock is -- and until a deck opts in, this
        // is a tempo reference the interface shows and nothing else.
        #[allow(clippy::cast_possible_truncation)]
        registry.set(ParamId::Global(GlobalParam::PeerNudge), advice.nudge as f32);
    }
}

/// Where the local rig is, as the peers need to hear it.
fn local_tempo(registry: &ParameterRegistry) -> LocalTempo {
    let bpm = f64::from(registry.get(ParamId::Global(GlobalParam::MasterBpm)));
    let phase = f64::from(registry.get(ParamId::Global(GlobalParam::MasterPhase)));
    LocalTempo {
        // Zero means nothing is playing, and `Bpm::new` refuses it along with
        // anything outside 20..=400 -- the same "is there something to be in
        // time with" question the MIDI clock asks.
        bpm: Bpm::new(bpm),
        // Negative means no grid, and it is passed through as such --
        // `PeerSync::announce` treats anything outside 0.0..1.0 as "no phase".
        // It used to be turned into a NaN here, which JSON cannot carry: the
        // announcement failed to encode and the rig went invisible instead of
        // merely unsynced.
        phase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_control::ParameterRegistry;

    fn loopback(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    /// A free port, released before use. Racy in principle; the alternative is
    /// a hard-coded port, which is racy in practice against a parallel test.
    fn free_port() -> u16 {
        let probe = std::net::UdpSocket::bind(loopback(0)).unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        port
    }

    fn registry_at(bpm: f32, phase: f32) -> Arc<ParameterRegistry> {
        let registry = Arc::new(ParameterRegistry::new());
        registry.set(ParamId::Global(GlobalParam::MasterBpm), bpm);
        registry.set(ParamId::Global(GlobalParam::MasterPhase), phase);
        registry
    }

    /// **Two instances find each other**, which is the whole feature. Started
    /// exactly as the commands start them, so this covers the wiring and not
    /// just the crate underneath it.
    #[test]
    fn two_instances_see_each_other() {
        let (a_port, b_port) = (free_port(), free_port());
        let a = Peers::default();
        let b = Peers::default();

        a.start(loopback(a_port), loopback(b_port), registry_at(128.0, 0.25))
            .expect("a should bind");
        b.start(loopback(b_port), loopback(a_port), registry_at(128.0, 0.75))
            .expect("b should bind");

        // Announcements go out ten times a second; give them a few rounds.
        std::thread::sleep(Duration::from_millis(400));

        let seen = a.status();
        assert!(seen.running, "a is not running");
        assert_eq!(seen.peers, 1, "a did not see b: {seen:?}");
        assert!(
            seen.peer_bpm.is_some_and(|bpm| (bpm - 128.0).abs() < 5.0),
            "a did not settle near b's tempo: {:?}",
            seen.peer_bpm
        );

        a.stop();
        b.stop();
        assert_eq!(a.status().peers, 0, "stopping did not clear the peers");
        assert!(!a.status().running);
    }

    /// A rig with nothing playing announces itself but is not a tempo
    /// reference. Its peers should see it and not follow it.
    #[test]
    fn a_silent_instance_is_seen_but_not_followed() {
        let (a_port, b_port) = (free_port(), free_port());
        let a = Peers::default();
        let b = Peers::default();

        a.start(loopback(a_port), loopback(b_port), registry_at(128.0, 0.25))
            .unwrap();
        // Zero BPM is what the engine publishes when nothing is playing.
        b.start(loopback(b_port), loopback(a_port), registry_at(0.0, -1.0))
            .unwrap();
        std::thread::sleep(Duration::from_millis(400));

        let seen = a.status();
        assert_eq!(seen.peers, 1, "a silent peer vanished");
        assert_eq!(
            seen.peer_bpm, None,
            "a silent peer was taken for a tempo reference"
        );
        a.stop();
        b.stop();
    }

    /// Starting twice does not leave the first socket bound — restarting on
    /// the same port is what changing the address in Settings does.
    #[test]
    fn restarting_rebinds_rather_than_failing() {
        let port = free_port();
        let peers = Peers::default();
        peers
            .start(loopback(port), loopback(port), registry_at(128.0, 0.0))
            .expect("first start");
        peers
            .start(loopback(port), loopback(port), registry_at(128.0, 0.0))
            .expect("a restart on the same port should rebind, not fail");
        assert!(peers.status().running);
        peers.stop();
    }

    /// A port that cannot be bound is reported rather than swallowed, and the
    /// reason is kept for the panel to show.
    #[test]
    fn a_port_that_cannot_be_bound_is_reported() {
        let held = std::net::UdpSocket::bind(loopback(0)).unwrap();
        let taken = held.local_addr().unwrap();
        let peers = Peers::default();
        let error = peers
            .start(taken, taken, registry_at(128.0, 0.0))
            .expect_err("binding a port already held should fail");
        assert!(!error.is_empty());
        assert!(!peers.status().running);
        assert_eq!(peers.status().error.as_deref(), Some(error.as_str()));
    }

    /// The nudge is published, never dispatched. A network thread does not get
    /// to move a deck's pitch without the DJ opting in.
    #[test]
    fn the_nudge_is_published_not_applied() {
        let (a_port, b_port) = (free_port(), free_port());
        let registry = registry_at(128.0, 0.0);
        let a = Peers::default();
        let b = Peers::default();
        a.start(loopback(a_port), loopback(b_port), Arc::clone(&registry))
            .unwrap();
        // Half a beat away, so there is definitely a correction to ask for.
        b.start(loopback(b_port), loopback(a_port), registry_at(128.0, 0.5))
            .unwrap();
        std::thread::sleep(Duration::from_millis(400));

        // The parameter exists and is finite whether or not it is zero; what
        // matters is that nothing reached the action bus.
        let nudge = registry.get(ParamId::Global(GlobalParam::PeerNudge));
        assert!(nudge.is_finite(), "the nudge was not published: {nudge}");
        assert!(nudge.abs() <= 0.01, "the nudge escaped its clamp: {nudge}");
        a.stop();
        b.stop();
    }
}

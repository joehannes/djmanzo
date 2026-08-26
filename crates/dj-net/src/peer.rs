//! Tempo and phase sync between djmanzo instances on a network.
//!
//! # What this is not
//!
//! **It is not Ableton Link.** Link is GPLv2-or-proprietary, and ADR-0002 rules
//! out linking the former; its protocol is documented well enough to reimplement,
//! but a reimplementation that claimed Link compatibility without ever having
//! been tested against Live, Serato or a real Link peer would be a claim nobody
//! here can stand behind. So this syncs djmanzo to djmanzo and says so. Link
//! interop stays a separate item, needing either the commercial licence or a
//! machine with a Link peer on it.
//!
//! What it does cover is the case a second laptop actually creates: two DJs
//! back to back, or a main rig and its backup, wanting one tempo and one
//! downbeat between them.
//!
//! # The shape
//!
//! Every peer both announces and listens. An announcement is one JSON object
//! per datagram — the same readable-on-the-wire choice the line protocol makes,
//! for the same reason: a protocol you can watch with `nc` is a protocol you
//! can debug at a gig.
//!
//! There is no election and no master. Each peer follows the *others* through
//! [`crate::PhaseFollower`], which corrects by a fraction of the error each
//! time; two peers converge on each other rather than one dragging the other.
//! That also means a peer appearing or vanishing costs nothing — there is no
//! state to hand over.
//!
//! # Authentication, and its absence
//!
//! There is none, and unlike the control server that is defensible rather than
//! deferred. UDP has no handshake to carry a passphrase, and what a hostile
//! packet can do here is bounded by the follower itself: a tempo more than six
//! percent away is ignored outright, and the rate nudge that reaches a deck is
//! clamped to one percent. The worst a stranger on the LAN achieves is a
//! slight, slow pull — against a control port, which can load tracks and open
//! devices, the same reasoning does not hold at all, and that one requires a
//! passphrase off loopback.

use dj_core::Bpm;
use serde::{Deserialize, Serialize};

/// What one peer tells the others about where it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announcement {
    /// Who is speaking, so a peer can ignore its own broadcast.
    ///
    /// Ignoring yourself matters more than it sounds: a peer that followed its
    /// own announcement would apply a correction to a phase it had just
    /// reported, which is a feedback loop with the network's jitter inside it.
    pub peer: String,
    /// The tempo this peer is playing at.
    pub bpm: f64,
    /// Where in the beat it is, 0.0 at the downbeat.
    ///
    /// **Negative when there is no beat to be in**, matching
    /// `GlobalParam::MasterPhase`. Never NaN: JSON has no way to write one, so
    /// `serde_json` refuses the whole announcement rather than encoding it —
    /// which meant a rig whose deck had no grid silently never announced at
    /// all, and was invisible to every peer on the network.
    pub phase: f64,
    /// False when nothing is playing.
    ///
    /// A stopped peer still announces — so the others know it is there — but
    /// its phase means nothing and following it would pull them to the tempo
    /// of a deck sitting still.
    pub playing: bool,
}

impl Announcement {
    /// The tempo, if it is one djmanzo would accept.
    ///
    /// `None` for a figure outside `Bpm`'s range, which is what a garbled
    /// packet or another program on the port looks like.
    #[must_use]
    pub fn tempo(&self) -> Option<Bpm> {
        Bpm::new(self.bpm)
    }

    /// Whether this announcement is worth following.
    ///
    /// A stopped peer, a nonsense tempo or a non-finite phase are all "no" —
    /// and all three arrive in practice, the last one from a peer whose deck
    /// has no grid yet.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        // A range check rather than `is_finite`: it covers the "no grid"
        // negative, and it also covers a hostile sender's NaN, which no local
        // code can produce but a stranger on the LAN certainly can.
        self.playing && (0.0..1.0).contains(&self.phase) && self.tempo().is_some()
    }
}

/// Encode an announcement for the wire.
///
/// # Errors
/// Never in practice — the type is plain data — but the error is returned
/// rather than unwrapped so a serialisation change cannot panic a gig.
pub fn encode(announcement: &Announcement) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(announcement)
}

/// Decode one datagram.
///
/// # Errors
/// When the bytes are not an announcement. Anything else on the port — another
/// program, a stray broadcast — lands here and is dropped.
pub fn decode(packet: &[u8]) -> Result<Announcement, serde_json::Error> {
    serde_json::from_slice(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn announcement() -> Announcement {
        Announcement {
            peer: "abc".into(),
            bpm: 128.0,
            phase: 0.25,
            playing: true,
        }
    }

    #[test]
    fn an_announcement_survives_the_wire() {
        let out = announcement();
        let bytes = encode(&out).unwrap();
        assert_eq!(decode(&bytes).unwrap(), out);
    }

    /// Anything else on the port is dropped rather than acted on. A UDP port is
    /// shared with whatever else is broadcasting on the LAN.
    #[test]
    fn rubbish_on_the_port_is_refused() {
        assert!(decode(b"").is_err());
        assert!(decode(b"not json").is_err());
        assert!(decode(br#"{"peer":"a"}"#).is_err(), "missing fields");
        assert!(
            decode(br#"{"peer":"a","bpm":"fast","phase":0.0,"playing":true}"#).is_err(),
            "a tempo that is not a number"
        );
    }

    /// A tempo outside `Bpm`'s range is not a tempo. Following one would drag a
    /// deck somewhere it cannot go, and `Bpm::new` is where that is decided for
    /// the whole project.
    #[test]
    fn a_nonsense_tempo_is_not_followed() {
        for bpm in [0.0, -128.0, 1e9, f64::NAN, f64::INFINITY] {
            let peer = Announcement {
                bpm,
                ..announcement()
            };
            assert!(peer.tempo().is_none(), "{bpm} was accepted as a tempo");
            assert!(!peer.is_usable(), "{bpm} was followed");
        }
    }

    /// A peer that is not playing is not a tempo reference. Its phase is
    /// wherever it was left, and following it would pull the room to a
    /// standstill's idea of the beat.
    #[test]
    fn a_stopped_peer_is_not_followed() {
        let stopped = Announcement {
            playing: false,
            ..announcement()
        };
        assert!(!stopped.is_usable());
        // Still a valid announcement, though -- it says "I am here".
        assert_eq!(decode(&encode(&stopped).unwrap()).unwrap(), stopped);
    }

    /// A deck with no grid has no phase, and says so with a negative one.
    #[test]
    fn a_peer_with_no_phase_is_not_followed() {
        for phase in [-1.0, f64::NAN, f64::INFINITY, 1.5] {
            let ungrided = Announcement {
                phase,
                ..announcement()
            };
            assert!(!ungrided.is_usable(), "{phase} was taken for a phase");
        }
    }
}

/// What the local rig is doing, handed to [`PeerSync`] each time it announces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalTempo {
    pub bpm: Option<Bpm>,
    pub phase: f64,
}

/// What following the peers is asking of the local rig.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PeerAdvice {
    /// A fractional rate nudge, already clamped by [`crate::PhaseFollower`].
    pub nudge: f64,
    /// How many peers were heard from in the last window.
    pub peers: usize,
    /// The tempo the followers have converged on, if any peer is playing.
    pub tempo: Option<Bpm>,
}

/// Announce the local tempo and follow everybody else's.
///
/// Both directions on one socket, because a peer that only listened would be
/// invisible to the peers it was following — and "why can the other laptop see
/// me but not the other way round" is not a thing to debug in a booth.
pub struct PeerSync {
    socket: std::net::UdpSocket,
    /// Where announcements go. Broadcast, or a named peer.
    send_to: std::net::SocketAddr,
    /// This peer's name on the wire, so its own announcements are ignored.
    id: String,
    /// One entry per peer heard from, keyed by their id.
    ///
    /// Per peer rather than one shared follower: two peers at different tempos
    /// are two disagreements, and averaging them into one correction would
    /// chase a tempo neither of them is playing.
    peers: std::collections::HashMap<String, Peer>,
}

/// How long a peer may say nothing before it is presumed gone.
///
/// Comfortably more than the announcement interval, because "has not spoken
/// since the last poll" is not the same question: peers announce at their own
/// rate and the interface polls at its own, so a peer announcing ten times a
/// second and polled sixty times a second is silent on five polls out of six
/// while being perfectly present.
const PEER_TIMEOUT_SECONDS: f64 = 2.0;

/// One peer, as far as this rig is concerned.
#[derive(Debug)]
struct Peer {
    follower: crate::PhaseFollower,
    /// Seconds since this peer last said anything.
    silent_for: f64,
    /// Whether it was playing when it last spoke. A stopped peer is still a
    /// peer -- it is on the network and the interface should say so -- it is
    /// just not a tempo reference.
    playing: bool,
}

impl PeerSync {
    /// Bind a socket and start listening.
    ///
    /// `send_to` is where announcements go — a broadcast address for a LAN, or
    /// one peer's address for a direct link between two machines.
    ///
    /// # Errors
    /// When the address cannot be bound, or broadcast cannot be enabled on it.
    pub fn bind(
        listen: std::net::SocketAddr,
        send_to: std::net::SocketAddr,
        id: impl Into<String>,
    ) -> Result<Self, crate::ServerError> {
        let socket = std::net::UdpSocket::bind(listen)
            .map_err(|e| crate::ServerError::Listen(listen, e.to_string()))?;
        // Non-blocking rather than a read timeout: this is polled from a loop
        // that also has to announce on a schedule, so it must never sit in a
        // read while an announcement comes due.
        socket
            .set_nonblocking(true)
            .map_err(|e| crate::ServerError::Listen(listen, e.to_string()))?;
        // Best-effort: a direct peer-to-peer address does not need it, and a
        // platform that refuses it should not stop the direct case working.
        let _ = socket.set_broadcast(true);
        Ok(Self {
            socket,
            send_to,
            id: id.into(),
            peers: std::collections::HashMap::new(),
        })
    }

    /// Where this peer is listening, which is what port 0 is for.
    ///
    /// # Errors
    /// When the socket has no local address, which would mean it is closed.
    pub fn address(&self) -> std::io::Result<std::net::SocketAddr> {
        self.socket.local_addr()
    }

    /// Say where the local rig is.
    ///
    /// # Errors
    /// When the datagram cannot be sent. A failure here is not fatal — the next
    /// announcement is a few tens of milliseconds away — so callers generally
    /// log and carry on.
    pub fn announce(&self, local: LocalTempo) -> std::io::Result<()> {
        let announcement = Announcement {
            peer: self.id.clone(),
            bpm: local.bpm.map_or(0.0, Bpm::get),
            // Sanitised here rather than trusted from the caller. A phase that
            // is not a phase goes out as -1.0, because JSON cannot carry a NaN
            // and `serde_json` would refuse the whole announcement -- leaving
            // this rig invisible instead of merely unsynced.
            phase: if (0.0..1.0).contains(&local.phase) {
                local.phase
            } else {
                -1.0
            },
            playing: local.bpm.is_some(),
        };
        let bytes = encode(&announcement)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.socket.send_to(&bytes, self.send_to).map(|_| ())
    }

    /// Drain whatever has arrived and return what it asks of the local rig.
    ///
    /// `elapsed` is how long since the last call, used to advance each
    /// follower's own phase between observations.
    ///
    /// Every waiting datagram is read, not just one: a burst from several peers
    /// arrives together, and leaving some in the buffer would mean following a
    /// tempo one poll out of date and growing.
    pub fn poll(&mut self, elapsed: f64, local: LocalTempo) -> PeerAdvice {
        for peer in self.peers.values_mut() {
            peer.follower.advance(elapsed);
            peer.silent_for += elapsed.max(0.0);
        }

        let mut buffer = [0u8; 1024];
        let mut nudge = 0.0f64;
        loop {
            let Ok((read, _from)) = self.socket.recv_from(&mut buffer) else {
                break; // nothing waiting, or a packet that vanished
            };
            let Ok(announcement) = decode(&buffer[..read]) else {
                continue;
            };
            // Our own broadcast, come back to us. Following it would apply a
            // correction to a phase we had just reported.
            if announcement.peer == self.id {
                continue;
            }
            let usable = announcement.is_usable();
            let entry = self
                .peers
                .entry(announcement.peer.clone())
                .or_insert_with(|| Peer {
                    // A new peer starts from the local tempo rather than from
                    // theirs, so joining a network nudges rather than jumps.
                    follower: crate::PhaseFollower::new(
                        local
                            .bpm
                            .or_else(|| announcement.tempo())
                            .unwrap_or(Bpm::new(120.0).expect("120 is a tempo")),
                    ),
                    silent_for: 0.0,
                    playing: usable,
                });
            entry.silent_for = 0.0;
            entry.playing = usable;
            if !usable {
                continue;
            }
            let Some(tempo) = announcement.tempo() else {
                continue;
            };
            let advice = entry.follower.observe(announcement.phase, tempo);
            // The largest correction any one peer asks for, rather than the
            // sum: three peers all half a beat ahead want one nudge, not three.
            if advice.abs() > nudge.abs() {
                nudge = advice;
            }
        }

        // A peer that has stopped announcing altogether is presumed gone, so
        // its stale tempo stops being reported. By time rather than by poll --
        // see `PEER_TIMEOUT_SECONDS`.
        self.peers
            .retain(|_, peer| peer.silent_for < PEER_TIMEOUT_SECONDS);

        let tempo = self
            .peers
            .values()
            .find(|peer| peer.playing)
            .map(|peer| peer.follower.tempo());
        PeerAdvice {
            nudge,
            peers: self.peers.len(),
            tempo,
        }
    }
}

impl std::fmt::Debug for PeerSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerSync")
            .field("id", &self.id)
            .field("send_to", &self.send_to)
            .field("peers", &self.peers.len())
            .finish()
    }
}

/// Two real sockets on loopback, because the interesting failures are in the
/// socket handling rather than in the arithmetic — `PhaseFollower` already has
/// its own tests, and mine would only restate them.
#[cfg(test)]
mod sync_tests_support {
    use super::*;

    /// A pair wired directly at each other, as two laptops on a switch are.
    pub fn pair() -> (PeerSync, PeerSync) {
        let loopback = |port| std::net::SocketAddr::from(([127, 0, 0, 1], port));
        // Bound first so the ports are known, then re-bound at each other.
        let a_probe = std::net::UdpSocket::bind(loopback(0)).unwrap();
        let b_probe = std::net::UdpSocket::bind(loopback(0)).unwrap();
        let a_at = a_probe.local_addr().unwrap();
        let b_at = b_probe.local_addr().unwrap();
        drop(a_probe);
        drop(b_probe);

        let a = PeerSync::bind(a_at, b_at, "a").unwrap();
        let b = PeerSync::bind(b_at, a_at, "b").unwrap();
        (a, b)
    }

    /// Give the loopback stack a moment to deliver.
    pub fn settle() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod sync_tests {
    use super::sync_tests_support::{pair, settle};
    use super::*;

    fn local(bpm: f64, phase: f64) -> LocalTempo {
        LocalTempo {
            bpm: Bpm::new(bpm),
            phase,
        }
    }

    /// The headline: a peer out of phase is pulled towards the other, and the
    /// error shrinks rather than oscillating.
    #[test]
    fn two_peers_converge_on_one_phase() {
        let (mut a, mut b) = pair();
        let mut a_phase = 0.0f64;
        let b_phase = 0.5f64; // half a beat apart, the worst case

        a.announce(local(128.0, a_phase)).unwrap();
        b.announce(local(128.0, b_phase)).unwrap();
        settle();

        let first = a.poll(0.0, local(128.0, a_phase)).nudge;
        assert!(
            first.abs() > 0.0,
            "a peer half a beat away produced no nudge"
        );

        // Run a few rounds with `a` actually taking the advice.
        let mut last = first.abs();
        for _ in 0..12 {
            a_phase = (a_phase + first.signum() * 0.05).rem_euclid(1.0);
            a.announce(local(128.0, a_phase)).unwrap();
            b.announce(local(128.0, b_phase)).unwrap();
            settle();
            let _ = b.poll(0.05, local(128.0, b_phase));
            let advice = a.poll(0.05, local(128.0, a_phase));
            last = advice.nudge.abs();
        }
        assert!(
            last <= first.abs(),
            "the correction grew instead of shrinking: {first} -> {last}"
        );
    }

    /// A peer sees the other and not itself.
    ///
    /// On a broadcast address every announcement comes back, and a peer that
    /// followed its own would be correcting towards a phase it had just
    /// reported — a feedback loop with the network's jitter inside it.
    #[test]
    fn a_peer_does_not_follow_itself() {
        let loopback = |port| std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let probe = std::net::UdpSocket::bind(loopback(0)).unwrap();
        let at = probe.local_addr().unwrap();
        drop(probe);
        // Talking to itself, which is what a broadcast address does.
        let mut alone = PeerSync::bind(at, at, "solo").unwrap();

        for _ in 0..5 {
            alone.announce(local(128.0, 0.25)).unwrap();
        }
        settle();
        let advice = alone.poll(0.0, local(128.0, 0.25));
        assert_eq!(advice.peers, 0, "it counted itself as a peer");
        assert_eq!(advice.nudge, 0.0, "it followed its own announcement");
    }

    /// A stopped peer is heard but not followed. It is still *there* — the
    /// count says so — but its phase is wherever its deck was left.
    #[test]
    fn a_stopped_peer_is_seen_but_not_followed() {
        let (mut a, b) = pair();
        b.announce(LocalTempo {
            bpm: None,
            phase: 0.5,
        })
        .unwrap();
        settle();

        let advice = a.poll(0.0, local(128.0, 0.0));
        assert_eq!(advice.peers, 1, "a stopped peer vanished from the count");
        assert_eq!(advice.nudge, 0.0, "a stopped peer was followed");
    }

    /// A peer that goes quiet is forgotten, so the interface stops reporting a
    /// tempo nobody is playing.
    #[test]
    fn a_peer_that_goes_quiet_is_forgotten() {
        let (mut a, b) = pair();
        b.announce(local(128.0, 0.25)).unwrap();
        settle();
        assert_eq!(a.poll(0.0, local(128.0, 0.0)).peers, 1);

        // b says nothing, for longer than the timeout. A single silent poll is
        // deliberately *not* enough -- peers announce at their own rate.
        settle();
        assert_eq!(
            a.poll(0.05, local(128.0, 0.0)).peers,
            1,
            "one silent poll should not lose a peer"
        );
        let advice = a.poll(PEER_TIMEOUT_SECONDS + 0.1, local(128.0, 0.0));
        assert_eq!(advice.peers, 0, "a silent peer was still counted");
        assert_eq!(
            advice.tempo, None,
            "a silent peer's tempo was still reported"
        );
    }

    /// Every waiting datagram is read, not one per poll. A burst from several
    /// peers arrives together, and leaving some buffered would mean following a
    /// tempo one poll out of date and falling further behind each round.
    #[test]
    fn a_burst_is_drained_in_one_poll() {
        let (mut a, b) = pair();
        for _ in 0..8 {
            b.announce(local(130.0, 0.5)).unwrap();
        }
        settle();

        let advice = a.poll(0.0, local(128.0, 0.0));
        assert_eq!(advice.peers, 1);
        assert!(advice.nudge.abs() > 0.0, "the burst produced no correction");

        // Nothing was left in the buffer: a second poll with no new
        // announcements has nothing to observe, so it asks for nothing.
        //
        // Measured by the nudge rather than by the peer count, because a peer
        // is remembered across silent polls on purpose -- see
        // `PEER_TIMEOUT_SECONDS`.
        let after = a.poll(0.0, local(128.0, 0.0));
        assert_eq!(
            after.nudge, 0.0,
            "the queue still held packets after a poll that claimed to drain it"
        );
        assert_eq!(after.peers, 1, "the peer should still be known");
    }

    /// Rubbish on the port does not stop the peers that are behaving.
    #[test]
    fn a_bad_packet_does_not_stop_the_good_ones() {
        let (mut a, b) = pair();
        let noise = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        noise
            .send_to(b"not an announcement", a.address().unwrap())
            .unwrap();
        b.announce(local(128.0, 0.5)).unwrap();
        noise.send_to(b"{}", a.address().unwrap()).unwrap();
        settle();

        let advice = a.poll(0.0, local(128.0, 0.0));
        assert_eq!(
            advice.peers, 1,
            "the good peer was lost with the bad packets"
        );
    }
}

/// A rig whose deck has no grid is still on the network.
#[cfg(test)]
mod no_phase_tests {
    use super::sync_tests_support::*;
    use super::*;

    /// **A phase that is not a phase must still leave a visible peer.**
    ///
    /// `serde_json` does not refuse a NaN — it writes `null` — so the failure
    /// lands on the *receiving* side, where `null` will not deserialise into an
    /// `f64` and the whole announcement is dropped. A rig whose deck had no
    /// grid was therefore invisible to every peer rather than merely unsynced,
    /// and no peer could tell it apart from one that was switched off.
    ///
    /// The first version of this test only checked that encoding succeeded,
    /// which it always did. A mutation caught that: removing the sanitising
    /// left every test green. Both ends are needed to make the claim.
    #[test]
    fn a_rig_with_no_grid_is_still_seen_by_its_peers() {
        for phase in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 7.5] {
            let (mut a, b) = pair();
            b.announce(LocalTempo {
                bpm: Bpm::new(128.0),
                phase,
            })
            .expect("announcing should not fail");
            settle();

            let advice = a.poll(
                0.0,
                LocalTempo {
                    bpm: Bpm::new(128.0),
                    phase: 0.0,
                },
            );
            assert_eq!(
                advice.peers, 1,
                "a peer announcing a phase of {phase} was invisible"
            );
            assert_eq!(
                advice.nudge, 0.0,
                "a peer with no phase of {phase} was followed anyway"
            );
        }
    }
}

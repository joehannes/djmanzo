//! §119: OBS, driven over obs-websocket 5.
//!
//! OBS 28 and later carry obs-websocket built in, listening on
//! `ws://localhost:4455` behind a password the DJ copies from OBS's *Tools →
//! WebSocket Server Settings*. This is a client of it, and only of the few
//! requests djmanzo needs: whether the stream is on, which scenes there are,
//! start and stop the stream, and put a scene on air. Nothing here changes
//! OBS's own set-up — the scenes, the sources and the stream key are the
//! DJ's, made in OBS. OBS is a separate program spoken to over a socket, so
//! its GPL does not reach djmanzo (ADR-0002).
//!
//! # The protocol, as far as djmanzo goes
//!
//! JSON messages over a WebSocket, each `{"op": n, "d": {...}}`. OBS opens
//! with *Hello* (op 0), carrying a challenge and a salt when it has a
//! password; djmanzo answers *Identify* (op 1) with [`authentication`] and
//! the events it wants; OBS says *Identified* (op 2) or closes the socket
//! with code 4009 for a wrong password. After that, a *Request* (op 6) is
//! answered by a *RequestResponse* (op 7) with the same id, and *Events*
//! (op 5) arrive whenever OBS has something to say — kept, in order, for
//! whoever asks next.
//!
//! Blocking, with a read timeout, and owned by one thread: the link in
//! `dj_app::obs` holds it and nothing else touches it.

use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tungstenite::protocol::CloseFrame;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::{Message, WebSocket};

/// Where OBS listens unless the DJ changed it.
pub const DEFAULT_PORT: u16 = 4455;

/// How long one request may take before the link calls OBS gone. OBS answers
/// on the same computer in milliseconds; five seconds is a hung OBS.
pub const ANSWER_WITHIN: Duration = Duration::from_secs(5);

/// The rpc version djmanzo speaks — obs-websocket 5's first and only.
const RPC_VERSION: u64 = 1;

/// The events djmanzo listens for: scenes (bit 2) and outputs (bit 6) —
/// a scene put on air by hand, and the stream starting or stopping.
const EVENTS: u64 = (1 << 2) | (1 << 6);

/// obs-websocket's close code for a password it refused.
const AUTHENTICATION_FAILED: u16 = 4009;

/// What went wrong talking to OBS, in words a DJ can act on.
#[derive(Debug, thiserror::Error)]
pub enum ObsError {
    #[error("OBS is not answering at {0} — is it open, with its WebSocket server on?")]
    Unreachable(String),
    #[error("OBS refused the password")]
    Refused,
    #[error("OBS closed the connection")]
    Closed,
    #[error("OBS did not answer in time")]
    Timeout,
    #[error("OBS would not {request}: {comment}")]
    Request { request: String, comment: String },
    #[error("OBS said something djmanzo did not expect: {0}")]
    Protocol(String),
}

/// Something OBS said without being asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A scene went on air — by djmanzo, by the DJ's hand, by anything.
    Scene(String),
    /// The stream started (`true`) or stopped.
    Streaming(bool),
}

/// The scenes OBS has, and which is on air.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Scenes {
    pub current: String,
    /// In OBS's own order, top of its list first.
    pub all: Vec<String>,
}

/// The string obs-websocket 5 asks for in *Identify*:
/// `base64(sha256(base64(sha256(password + salt)) + challenge))`.
#[must_use]
pub fn authentication(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

/// A connection to OBS, identified and ready for requests.
pub struct Obs {
    socket: WebSocket<TcpStream>,
    next_id: u64,
    events: VecDeque<Event>,
}

impl std::fmt::Debug for Obs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Obs")
            .field("requests", &self.next_id)
            .field("events", &self.events)
            .finish_non_exhaustive()
    }
}

impl Obs {
    /// Connect to OBS at `host:port`, and identify with `password` if OBS
    /// asks for one.
    ///
    /// # Errors
    /// OBS not there, a password it refuses, or a conversation that is not
    /// obs-websocket 5's.
    pub fn connect(host: &str, port: u16, password: &str) -> Result<Self, ObsError> {
        let at = format!("{host}:{port}");
        let unreachable = || ObsError::Unreachable(at.clone());
        let address = at
            .to_socket_addrs()
            .map_err(|_| unreachable())?
            .next()
            .ok_or_else(unreachable)?;
        let stream =
            TcpStream::connect_timeout(&address, ANSWER_WITHIN).map_err(|_| unreachable())?;
        stream
            .set_read_timeout(Some(ANSWER_WITHIN))
            .map_err(|_| unreachable())?;
        let (socket, _) = tungstenite::client(format!("ws://{at}"), stream)
            .map_err(|e| ObsError::Protocol(format!("the WebSocket handshake failed: {e}")))?;
        let mut obs = Self {
            socket,
            next_id: 0,
            events: VecDeque::new(),
        };
        obs.identify(password)?;
        Ok(obs)
    }

    fn identify(&mut self, password: &str) -> Result<(), ObsError> {
        let hello = self.receive()?;
        if hello["op"] != 0 {
            return Err(ObsError::Protocol(format!("expected Hello, got {hello}")));
        }
        let mut identify = json!({ "rpcVersion": RPC_VERSION, "eventSubscriptions": EVENTS });
        if let Some(auth) = hello["d"].get("authentication") {
            let (Some(challenge), Some(salt)) = (auth["challenge"].as_str(), auth["salt"].as_str())
            else {
                return Err(ObsError::Protocol(format!(
                    "a Hello without its challenge: {hello}"
                )));
            };
            identify["authentication"] = json!(authentication(password, salt, challenge));
        }
        self.send(&json!({ "op": 1, "d": identify }))?;
        let identified = self.receive()?;
        if identified["op"] == 2 {
            Ok(())
        } else {
            Err(ObsError::Protocol(format!(
                "expected Identified, got {identified}"
            )))
        }
    }

    /// Ask OBS for `request`, with `data`, and wait for its answer's data.
    /// Events that arrive meanwhile are kept for [`Obs::events`].
    ///
    /// # Errors
    /// OBS gone, too slow, or refusing the request (its own words).
    pub fn request(&mut self, request: &str, data: Value) -> Result<Value, ObsError> {
        self.next_id += 1;
        let id = self.next_id.to_string();
        self.send(&json!({
            "op": 6,
            "d": { "requestType": request, "requestId": id, "requestData": data }
        }))?;
        loop {
            let message = self.receive()?;
            if message["op"] == 7 && message["d"]["requestId"] == id.as_str() {
                let status = &message["d"]["requestStatus"];
                if status["result"] == true {
                    return Ok(message["d"]["responseData"].clone());
                }
                return Err(ObsError::Request {
                    request: request.to_owned(),
                    comment: status["comment"]
                        .as_str()
                        .map_or_else(|| format!("code {}", status["code"]), str::to_owned),
                });
            }
            self.keep_event(&message);
        }
    }

    /// Whether the stream is on air.
    ///
    /// # Errors
    /// As [`Obs::request`].
    pub fn streaming(&mut self) -> Result<bool, ObsError> {
        Ok(self.request("GetStreamStatus", json!({}))?["outputActive"] == true)
    }

    /// The scenes there are, and which is on air.
    ///
    /// # Errors
    /// As [`Obs::request`].
    pub fn scenes(&mut self) -> Result<Scenes, ObsError> {
        let data = self.request("GetSceneList", json!({}))?;
        // OBS lists them bottom first; its own window shows them top first.
        let mut all: Vec<String> = data["scenes"]
            .as_array()
            .map(|scenes| {
                scenes
                    .iter()
                    .filter_map(|scene| scene["sceneName"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        all.reverse();
        Ok(Scenes {
            current: data["currentProgramSceneName"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            all,
        })
    }

    /// Put `scene` on air.
    ///
    /// # Errors
    /// As [`Obs::request`] — a scene OBS does not have, in OBS's words.
    pub fn put_on_air(&mut self, scene: &str) -> Result<(), ObsError> {
        self.request("SetCurrentProgramScene", json!({ "sceneName": scene }))
            .map(|_| ())
    }

    /// Start the stream.
    ///
    /// # Errors
    /// As [`Obs::request`] — already streaming, or no stream set up in OBS.
    pub fn start_stream(&mut self) -> Result<(), ObsError> {
        self.request("StartStream", json!({})).map(|_| ())
    }

    /// Stop the stream.
    ///
    /// # Errors
    /// As [`Obs::request`].
    pub fn stop_stream(&mut self) -> Result<(), ObsError> {
        self.request("StopStream", json!({})).map(|_| ())
    }

    /// What OBS has said since last asked, oldest first; waits up to `for_`
    /// for something new when nothing is kept.
    ///
    /// # Errors
    /// OBS gone.
    pub fn events(&mut self, for_: Duration) -> Result<Vec<Event>, ObsError> {
        if self.events.is_empty() {
            let _ = self
                .socket
                .get_mut()
                .set_read_timeout(Some(for_.max(Duration::from_millis(1))));
            let read = self.receive();
            let _ = self.socket.get_mut().set_read_timeout(Some(ANSWER_WITHIN));
            match read {
                Ok(message) => self.keep_event(&message),
                Err(ObsError::Timeout) => {}
                Err(other) => return Err(other),
            }
        }
        Ok(self.events.drain(..).collect())
    }

    fn keep_event(&mut self, message: &Value) {
        if message["op"] != 5 {
            return;
        }
        let data = &message["d"]["eventData"];
        let event = match message["d"]["eventType"].as_str() {
            Some("CurrentProgramSceneChanged") => data["sceneName"]
                .as_str()
                .map(|s| Event::Scene(s.to_owned())),
            Some("StreamStateChanged") => data["outputActive"].as_bool().map(Event::Streaming),
            _ => None,
        };
        if let Some(event) = event {
            self.events.push_back(event);
        }
    }

    fn send(&mut self, message: &Value) -> Result<(), ObsError> {
        self.socket
            .send(Message::text(message.to_string()))
            .map_err(|e| failure(&e))
    }

    /// The next JSON message, skipping pings and the like.
    fn receive(&mut self) -> Result<Value, ObsError> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str(&text)
                        .map_err(|e| ObsError::Protocol(format!("not JSON: {e}")));
                }
                Ok(Message::Close(frame)) => return Err(closed(frame.as_ref())),
                Ok(_) => {}
                Err(e) => return Err(failure(&e)),
            }
        }
    }
}

fn closed(frame: Option<&CloseFrame>) -> ObsError {
    match frame {
        Some(frame) if frame.code == CloseCode::from(AUTHENTICATION_FAILED) => ObsError::Refused,
        _ => ObsError::Closed,
    }
}

fn failure(error: &tungstenite::Error) -> ObsError {
    match error {
        tungstenite::Error::Io(io)
            if matches!(
                io.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) =>
        {
            ObsError::Timeout
        }
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            ObsError::Closed
        }
        tungstenite::Error::Protocol(_) | tungstenite::Error::Io(_) => ObsError::Closed,
        other => ObsError::Protocol(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    const SALT: &str = "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=";
    const CHALLENGE: &str = "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY=";

    /// The algorithm obs-websocket 5 documents, checked against a value
    /// computed outside djmanzo (Python's `hashlib` and `base64`) for its own
    /// example salt and challenge.
    #[test]
    fn the_password_is_answered_the_way_obs_websocket_documents() {
        assert_eq!(
            authentication("supersecretpassword", SALT, CHALLENGE),
            "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4="
        );
    }

    /// A stand-in for OBS: speaks obs-websocket 5 over a real socket, with a
    /// password, two scenes, and a stream that starts and stops — and hands
    /// every message djmanzo sent back to the test.
    fn fake_obs(password: &'static str) -> (u16, mpsc::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (heard, hear) = mpsc::channel();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let say = |ws: &mut WebSocket<TcpStream>, v: Value| {
                ws.send(Message::text(v.to_string())).unwrap()
            };
            say(
                &mut ws,
                json!({"op": 0, "d": {"obsWebSocketVersion": "5.5.0", "rpcVersion": 1,
                    "authentication": {"challenge": CHALLENGE, "salt": SALT}}}),
            );
            let mut on_air = "Decks".to_owned();
            let mut streaming = false;
            loop {
                let Ok(Message::Text(text)) = ws.read() else {
                    return;
                };
                let message: Value = serde_json::from_str(&text).unwrap();
                heard.send(message.clone()).unwrap();
                match message["op"].as_u64() {
                    Some(1) => {
                        if message["d"]["authentication"]
                            != authentication(password, SALT, CHALLENGE)
                        {
                            let _ = ws.close(Some(CloseFrame {
                                code: CloseCode::from(AUTHENTICATION_FAILED),
                                reason: "Authentication failed.".into(),
                            }));
                            let _ = ws.flush();
                            return;
                        }
                        say(&mut ws, json!({"op": 2, "d": {"negotiatedRpcVersion": 1}}));
                    }
                    Some(6) => {
                        let d = &message["d"];
                        let id = d["requestId"].clone();
                        let kind = d["requestType"].as_str().unwrap_or_default().to_owned();
                        let (ok, data, comment) = match kind.as_str() {
                            "GetStreamStatus" => (true, json!({"outputActive": streaming}), ""),
                            "GetSceneList" => (
                                true,
                                json!({"currentProgramSceneName": on_air,
                                       "scenes": [{"sceneIndex": 0, "sceneName": "Crowd"},
                                                  {"sceneIndex": 1, "sceneName": "Decks"}]}),
                                "",
                            ),
                            "SetCurrentProgramScene" => {
                                let name =
                                    d["requestData"]["sceneName"].as_str().unwrap_or_default();
                                if name == "Decks" || name == "Crowd" {
                                    on_air = name.to_owned();
                                    // OBS tells its listeners before it answers.
                                    say(
                                        &mut ws,
                                        json!({"op": 5, "d": {"eventType": "CurrentProgramSceneChanged",
                                        "eventIntent": 4, "eventData": {"sceneName": name}}}),
                                    );
                                    (true, json!(null), "")
                                } else {
                                    (
                                        false,
                                        json!(null),
                                        "No source was found by the name of `Nowhere`.",
                                    )
                                }
                            }
                            "StartStream" if !streaming => {
                                streaming = true;
                                (true, json!(null), "")
                            }
                            "StartStream" => {
                                (false, json!(null), "The stream output is already active.")
                            }
                            "StopStream" => {
                                streaming = false;
                                (true, json!(null), "")
                            }
                            _ => (false, json!(null), "Unknown request."),
                        };
                        let mut status = json!({"result": ok, "code": if ok { 100 } else { 600 }});
                        if !comment.is_empty() {
                            status["comment"] = json!(comment);
                        }
                        say(
                            &mut ws,
                            json!({"op": 7, "d": {"requestType": kind, "requestId": id,
                            "requestStatus": status, "responseData": data}}),
                        );
                    }
                    _ => {}
                }
            }
        });
        (port, hear)
    }

    /// **djmanzo identifies with the password and drives the stream and the
    /// scenes**, each a request obs-websocket names, through a real socket.
    #[test]
    fn djmanzo_identifies_and_drives_obs() {
        let (port, hear) = fake_obs("hunter2");
        let mut obs = Obs::connect("127.0.0.1", port, "hunter2").expect("identified");
        let identify = hear.recv().unwrap();
        assert_eq!(identify["op"], 1);
        assert_eq!(identify["d"]["rpcVersion"], 1);
        assert_eq!(
            identify["d"]["eventSubscriptions"], EVENTS,
            "scenes and outputs"
        );

        assert!(!obs.streaming().unwrap());
        let scenes = obs.scenes().unwrap();
        assert_eq!(scenes.current, "Decks");
        assert_eq!(scenes.all, ["Decks", "Crowd"], "top of OBS's list first");

        obs.put_on_air("Crowd").unwrap();
        assert_eq!(obs.scenes().unwrap().current, "Crowd");
        // The event OBS sent before answering is kept for whoever asks.
        assert_eq!(
            obs.events(Duration::from_millis(10)).unwrap(),
            [Event::Scene("Crowd".into())]
        );
        assert!(
            obs.events(Duration::from_millis(10)).unwrap().is_empty(),
            "handed over once"
        );

        obs.start_stream().unwrap();
        assert!(obs.streaming().unwrap());
        // OBS's own words when it says no.
        let again = obs.start_stream().unwrap_err().to_string();
        assert!(again.contains("already active"), "{again}");
        let nowhere = obs.put_on_air("Nowhere").unwrap_err().to_string();
        assert!(nowhere.contains("Nowhere"), "{nowhere}");
        obs.stop_stream().unwrap();
        assert!(!obs.streaming().unwrap());

        // Every request carried its own id.
        let ids: Vec<Value> = hear
            .try_iter()
            .map(|m| m["d"]["requestId"].clone())
            .collect();
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(ids, unique);
    }

    /// **A wrong password is said as a wrong password**, not as OBS being
    /// gone — the one a DJ can fix by retyping.
    #[test]
    fn a_wrong_password_is_named() {
        let (port, _hear) = fake_obs("hunter2");
        let refused = Obs::connect("127.0.0.1", port, "hunter3").expect_err("refused");
        assert!(matches!(refused, ObsError::Refused), "{refused:?}");
    }

    /// **No OBS is said as no OBS**, quickly.
    #[test]
    fn no_obs_is_named() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let gone = Obs::connect("127.0.0.1", port, "").expect_err("nobody there");
        assert!(matches!(gone, ObsError::Unreachable(_)), "{gone:?}");
        assert!(gone.to_string().contains("WebSocket server"), "{gone}");
    }
}

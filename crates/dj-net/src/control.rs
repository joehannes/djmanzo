use dj_control::{ActionBus, BusFull, ParameterRegistry};
use dj_core::{Action, ParamId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Versioned, JSON-serializable control message for WebSocket and OSC bridges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Parse and dispatch the public text action grammar (for example `deck 1 play`).
    Action { action: String },
    /// Return the named, stable parameter map. This avoids UI/DOM scraping.
    Parameters,
    /// Offer a token. Required as the first frame when the server has one.
    ///
    /// Part of the request vocabulary rather than a transport header because
    /// the transport is a line of JSON: there is nowhere else to put it, and a
    /// client that can send an action can send this.
    Hello { token: String },
}

/// Why a control request was refused.
///
/// An enum rather than a string so a client can branch on it, and so the set
/// is closed: a transport cannot invent a code, and adding one here is a
/// visible change to the protocol.
///
/// It was `&'static str`, which serialised fine and could not be
/// *deserialised* at all -- `#[derive(Deserialize)]` on a borrowed `'static`
/// field requires `'static` input, so any client trying to parse a response
/// would not have compiled. A response type that cannot be read back is not a
/// protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The frame was not the JSON this protocol expects.
    BadRequest,
    /// The frame was well formed but the action text is not in the grammar.
    BadAction,
    /// The engine is not keeping up; the sender should back off and retry.
    QueueFull,
    /// The token was missing or wrong. The connection is closed after this.
    Unauthorised,
    /// Too many requests too quickly. The connection stays open: this is a
    /// client to slow down, not one to throw out.
    TooFast,
    /// The action was in the grammar and the application would not carry it
    /// out.
    ///
    /// Distinct from [`ErrorCode::BadAction`] on purpose: that one means the
    /// client sent something djmanzo has never understood and should be fixed;
    /// this one means a verb that usually works did not this time — no device
    /// open, a deck that is not there, a recording that cannot be started — and
    /// the same frame sent a minute later may be accepted.
    Refused,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Accepted,
    Parameters { values: Vec<NamedParameter> },
    Error { code: ErrorCode, message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedParameter {
    pub name: String,
    pub value: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("invalid JSON control request: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid action: {0}")]
    Action(#[from] dj_core::action::ParseError),
    #[error("the action queue is full")]
    QueueFull,
    #[error("{0}")]
    Refused(String),
}

/// Where a parsed action goes.
///
/// **The bus is not always the right answer, and that was a defect.** Several
/// verbs in the vocabulary mean something *outside* the engine: `deck 1 eject`
/// has to clear the deck's name and its analysis, which live in the
/// application, and `record on` has to open a file, which the engine cannot do
/// at all. A socket that dispatched straight at the bus therefore ejected in
/// the engine and left the interface showing a record that was no longer on the
/// deck, and started no recording while answering `accepted`.
///
/// §87 is the section that names this: *if track loading originates from
/// browser, assistant, preset, controller, keyboard, network or drag & drop,
/// the resulting UI must look identical*. It was not identical, and it was the
/// network that differed, because that origin was the only one not going
/// through the application's own entry point.
///
/// So the service is handed somewhere to put an action rather than assuming it.
/// `dj-net` keeps the bus as its own default — it has no application to ask —
/// and djmanzo passes `commands::perform`.
pub trait Carry: std::fmt::Debug + Send + Sync {
    /// Carry out one action.
    ///
    /// # Errors
    /// [`ControlError::Refused`] with the application's own sentence, or
    /// [`ControlError::QueueFull`].
    fn carry(&self, action: Action) -> Result<(), ControlError>;

    /// Carry out one **line** of the public grammar.
    ///
    /// Separate from [`Carry::carry`] because the host's vocabulary can be
    /// wider than [`Action`]'s, and djmanzo's is: `load deck 1 <track-id>` is a
    /// line every session file contains and `Action::parse` has never accepted
    /// it, on purpose — a load carries an `Arc` and nothing external should be
    /// inventing one, so it is a *command* rather than an action.
    ///
    /// Parsing here and handing over an `Action` therefore refused, at dj-net's
    /// door, a line the application performs perfectly well — which is how §87's
    /// network origin came to be the one that could not load. It was found by
    /// opening the port and sending the line, and not by any test: every test
    /// on both sides of this seam was asking about actions.
    ///
    /// The default is what a host with no wider vocabulary wants.
    ///
    /// # Errors
    /// As [`Carry::carry`], plus a parse error when the line is not in the
    /// grammar at all.
    fn carry_line(&self, line: &str) -> Result<(), ControlError> {
        self.carry(Action::parse(line)?)
    }
}

/// Applies control requests through the public action bus and registry only.
///
/// **Shared handles, not owned ones.** These were taken by value, which meant
/// the service could only ever be built on a bus and a registry of its own --
/// and an application dispatching into a private ring buffer nobody reads is
/// not remote control, it is a very well tested no-op. `ActionBus` and
/// `ParameterRegistry` are deliberately not `Clone` (one bus, one log, one set
/// of atomics), so sharing means an `Arc`, which is how every other consumer
/// in djmanzo holds them.
#[derive(Debug)]
pub struct ControlService<C> {
    bus: Arc<ActionBus<C>>,
    registry: Arc<ParameterRegistry>,
    /// Where actions go, when the host has somewhere better than the bus.
    carrier: Option<Arc<dyn Carry>>,
}

impl<C> ControlService<C>
where
    C: From<Action>,
{
    #[must_use]
    pub fn new(bus: Arc<ActionBus<C>>, registry: Arc<ParameterRegistry>) -> Self {
        Self {
            bus,
            registry,
            carrier: None,
        }
    }

    /// Send actions here instead of straight at the bus.
    ///
    /// See [`Carry`]. The registry is still read directly — a parameter read is
    /// a read, and there is nothing for an application to intercept in it.
    #[must_use]
    pub fn carried_by(mut self, carrier: Arc<dyn Carry>) -> Self {
        self.carrier = Some(carrier);
        self
    }

    /// One place an already-parsed action goes. OSC's road.
    fn send(&self, action: Action) -> Result<(), ControlError> {
        match &self.carrier {
            Some(carrier) => carrier.carry(action),
            None => self
                .bus
                .dispatch(action)
                .map_err(|_: BusFull| ControlError::QueueFull),
        }
    }

    /// One place a line of the grammar goes.
    ///
    /// Unparsed when there is a carrier, so the host's vocabulary decides what
    /// the line means. Parsed here when there is not, because the bus takes
    /// actions and nothing else.
    fn send_line(&self, line: &str) -> Result<(), ControlError> {
        match &self.carrier {
            Some(carrier) => carrier.carry_line(line),
            None => self.send(Action::parse(line)?),
        }
    }

    /// Handles one frame. Transport implementations supply their own framing,
    /// authentication and rate limits; this method has no socket or engine access.
    pub fn handle_json(&self, json: &str) -> ControlResponse {
        match serde_json::from_str::<ControlRequest>(json) {
            Ok(request) => match self.handle(request) {
                Ok(response) => response,
                Err(error) => ControlResponse::Error {
                    code: error.code(),
                    message: error.to_string(),
                },
            },
            Err(error) => ControlResponse::Error {
                code: ErrorCode::BadRequest,
                message: error.to_string(),
            },
        }
    }

    /// Send one action, for a transport that has already parsed it.
    ///
    /// OSC arrives as an address rather than as action text, so it has an
    /// `Action` in hand before it reaches here. It still goes through the same
    /// bus, the same bound and the same log as everything else.
    ///
    /// # Errors
    /// When the engine's queue is full.
    pub fn dispatch(&self, action: Action) -> Result<(), ControlError> {
        self.send(action)
    }

    pub fn handle(&self, request: ControlRequest) -> Result<ControlResponse, ControlError> {
        match request {
            ControlRequest::Action { action } => {
                self.send_line(&action)?;
                Ok(ControlResponse::Accepted)
            }
            // Answered rather than refused so a client may greet a server that
            // wants no token: "I offered a key and the door was already open"
            // is not an error worth failing a connection over.
            ControlRequest::Hello { .. } => Ok(ControlResponse::Accepted),
            ControlRequest::Parameters => Ok(ControlResponse::Parameters {
                values: ParamId::all()
                    .map(|id| NamedParameter {
                        name: id.name(),
                        value: self.registry.get(id),
                    })
                    .collect(),
            }),
        }
    }
}

impl ControlError {
    /// The code a client branches on.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Json(_) => ErrorCode::BadRequest,
            Self::Action(_) => ErrorCode::BadAction,
            Self::QueueFull => ErrorCode::QueueFull,
            Self::Refused(_) => ErrorCode::Refused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Command {
        Action(Action),
    }
    impl From<Action> for Command {
        fn from(value: Action) -> Self {
            Self::Action(value)
        }
    }

    fn service(depth: usize) -> (ControlService<Command>, rtrb::Consumer<Command>) {
        let (bus, consumer) = ActionBus::<Command>::new(depth);
        (
            ControlService::new(Arc::new(bus), Arc::new(ParameterRegistry::new())),
            consumer,
        )
    }

    /// **The load-bearing one for §87: a host can say where actions go.**
    ///
    /// Before this the service dispatched straight at the bus, and so skipped
    /// everything djmanzo does around an action — `eject` left the interface
    /// showing a record that was no longer on the deck, and `record on` was
    /// answered "accepted" having started nothing, because the engine cannot
    /// open a file. The network was the one of §87's seven origins not going
    /// through the application's own entry point, which is exactly why it was
    /// the one whose resulting state differed.
    ///
    /// Both halves are asserted. The carrier gets the action, and **the bus does
    /// not** — a service that helpfully did both would double every action a
    /// socket sent.
    #[test]
    fn a_carrier_gets_the_action_instead_of_the_bus() {
        #[derive(Debug, Default)]
        struct Held(std::sync::Mutex<Vec<Action>>);
        impl Carry for Held {
            fn carry(&self, action: Action) -> Result<(), ControlError> {
                self.0.lock().unwrap().push(action);
                Ok(())
            }
        }

        let (bus, mut consumer) = ActionBus::<Command>::new(4);
        let held = Arc::new(Held::default());
        let service = ControlService::new(Arc::new(bus), Arc::new(ParameterRegistry::new()))
            .carried_by(held.clone());

        assert_eq!(
            service.handle_json(r#"{"type":"action","action":"deck 1 eject"}"#),
            ControlResponse::Accepted
        );
        assert_eq!(
            held.0.lock().unwrap().as_slice(),
            &[Action::parse("deck 1 eject").unwrap()],
            "the host was handed nowhere to put the action and it went to the \
             bus anyway, which is the state §87 found"
        );
        assert!(
            consumer.pop().is_err(),
            "the action reached the bus as well as the host, so a socket sends \
             everything twice"
        );

        // And OSC, which arrives already parsed, takes the same road.
        service
            .dispatch(Action::parse("deck 2 play").unwrap())
            .unwrap();
        assert_eq!(held.0.lock().unwrap().len(), 2);
        assert!(consumer.pop().is_err());
    }

    /// **A host whose vocabulary is wider than `Action`'s gets the line.**
    ///
    /// The defect the tests on both sides of this seam could not see, because
    /// every one of them was asking about *actions*. djmanzo's grammar has one
    /// verb that is not an action — `load deck 1 <track-id>`, which is in every
    /// session file — and parsing here refused it at dj-net's door while the
    /// application would have performed it. §87's network origin was therefore
    /// the one origin that could not load, after all the work to make sure it
    /// could. It was found by opening the port and sending the line.
    #[test]
    fn a_line_the_grammar_refuses_still_reaches_a_host_that_understands_it() {
        #[derive(Debug, Default)]
        struct Wider(std::sync::Mutex<Vec<String>>);
        impl Carry for Wider {
            fn carry(&self, _: Action) -> Result<(), ControlError> {
                unreachable!("the line should not have been parsed first")
            }
            fn carry_line(&self, line: &str) -> Result<(), ControlError> {
                self.0.lock().unwrap().push(line.to_owned());
                Ok(())
            }
        }

        let (bus, _consumer) = ActionBus::<Command>::new(4);
        let wider = Arc::new(Wider::default());
        let service = ControlService::new(Arc::new(bus), Arc::new(ParameterRegistry::new()))
            .carried_by(wider.clone());

        let line = "load deck 1 abababababababababababababababababababababababababababababababab";
        assert!(
            Action::parse(line).is_err(),
            "this line is now an action, so this test is measuring nothing"
        );
        assert_eq!(
            service.handle_json(
                &serde_json::to_string(&ControlRequest::Action {
                    action: line.to_owned()
                })
                .unwrap()
            ),
            ControlResponse::Accepted,
            "the service parsed the line itself and refused a verb the host has"
        );
        assert_eq!(wider.0.lock().unwrap().as_slice(), &[line.to_owned()]);
    }

    /// A refusal reaches the client as a refusal, with the reason.
    ///
    /// Its own code rather than `bad_action`: the client did not send something
    /// wrong, the application would not do it *now*. A socket told its verb is
    /// not in the grammar goes looking for a bug in itself.
    #[test]
    fn an_application_that_refuses_says_so_and_says_why() {
        #[derive(Debug)]
        struct No;
        impl Carry for No {
            fn carry(&self, _: Action) -> Result<(), ControlError> {
                Err(ControlError::Refused("no device is open".to_owned()))
            }
        }

        let (bus, _consumer) = ActionBus::<Command>::new(4);
        let service = ControlService::new(Arc::new(bus), Arc::new(ParameterRegistry::new()))
            .carried_by(Arc::new(No));

        assert_eq!(
            service.handle_json(r#"{"type":"action","action":"deck 1 play"}"#),
            ControlResponse::Error {
                code: ErrorCode::Refused,
                message: "no device is open".to_owned(),
            }
        );
    }

    #[test]
    fn action_requests_use_the_shared_bus() {
        let (service, mut consumer) = service(2);
        assert_eq!(
            service.handle_json(r#"{"type":"action","action":"deck 1 play"}"#),
            ControlResponse::Accepted
        );
        assert_eq!(
            consumer.pop().unwrap(),
            Command::Action(Action::parse("deck 1 play").unwrap())
        );
    }

    #[test]
    fn malformed_input_is_a_safe_structured_error() {
        let (service, _consumer) = service(2);
        assert!(matches!(
            service.handle_json("not json"),
            ControlResponse::Error {
                code: ErrorCode::BadRequest,
                ..
            }
        ));
        assert!(matches!(
            service.handle_json(r#"{"type":"action","action":"deck nope play"}"#),
            ControlResponse::Error {
                code: ErrorCode::BadAction,
                ..
            }
        ));
    }

    /// A request naming something this protocol does not have is refused as a
    /// bad request, not ignored. Silence would leave a client waiting.
    #[test]
    fn an_unknown_request_type_is_refused() {
        let (service, _consumer) = service(2);
        assert!(matches!(
            service.handle_json(r#"{"type":"shutdown"}"#),
            ControlResponse::Error {
                code: ErrorCode::BadRequest,
                ..
            }
        ));
    }

    /// **The back-pressure path**, and the one a flooding peer will find
    /// first. A full queue has to be a refusal the sender can act on, not a
    /// silent drop and not a panic.
    #[test]
    fn a_full_queue_is_reported_rather_than_dropped() {
        let (service, _consumer) = service(2);
        let frame = r#"{"type":"action","action":"deck 1 play"}"#;

        let mut refusals = 0;
        for _ in 0..32 {
            if let ControlResponse::Error { code, .. } = service.handle_json(frame) {
                assert_eq!(code, ErrorCode::QueueFull);
                refusals += 1;
            }
        }
        assert!(refusals > 0, "a bounded queue never filled");
    }

    /// The other half of the API. Reading parameters is what keeps a network
    /// client from scraping the interface for state.
    #[test]
    fn parameters_come_back_named() {
        let (service, _consumer) = service(2);
        let ControlResponse::Parameters { values } =
            service.handle_json(r#"{"type":"parameters"}"#)
        else {
            panic!("expected a parameter map");
        };

        assert!(!values.is_empty(), "no parameters at all");
        assert_eq!(
            values.len(),
            ParamId::all().count(),
            "the map has to be every parameter, or a client cannot tell what is missing"
        );
        assert!(
            values.iter().all(|p| !p.name.is_empty()),
            "a parameter with no name is not addressable"
        );
    }

    /// **ADR-0003, at the network boundary.** A peer gets the same text
    /// grammar as a controller mapping or the assistant -- no more. If some
    /// action were reachable over the network and not by parsing, this crate
    /// would have grown a private engine API.
    #[test]
    fn the_network_can_say_exactly_what_the_grammar_can_say() {
        let (service, mut consumer) = service(64);
        for text in [
            "deck 1 play",
            "deck 2 cue",
            "deck 1 volume 0.5",
            "crossfader 0.0",
            "deck 1 stem_mute vocal",
        ] {
            let frame = format!(r#"{{"type":"action","action":"{text}"}}"#);
            assert_eq!(
                service.handle_json(&frame),
                ControlResponse::Accepted,
                "the grammar accepts `{text}` but the network did not"
            );
            assert_eq!(
                consumer.pop().unwrap(),
                Command::Action(Action::parse(text).unwrap()),
                "`{text}` did not arrive as the action it parses to"
            );
        }
    }

    /// A response has to survive the round trip, or a client cannot read it.
    /// The error code used to be a `&'static str`, which no client could have
    /// deserialised.
    #[test]
    fn every_response_round_trips_through_json() {
        let responses = [
            ControlResponse::Accepted,
            ControlResponse::Parameters {
                values: vec![NamedParameter {
                    name: "deck.1.volume".to_owned(),
                    value: 0.5,
                }],
            },
            ControlResponse::Error {
                code: ErrorCode::QueueFull,
                message: "the action queue is full".to_owned(),
            },
        ];

        for response in responses {
            let text = serde_json::to_string(&response).expect("serialise");
            let back: ControlResponse = serde_json::from_str(&text).expect("deserialise");
            assert_eq!(back, response, "round trip changed {text}");
        }
    }
}

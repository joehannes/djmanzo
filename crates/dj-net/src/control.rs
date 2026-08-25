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
}

impl<C> ControlService<C>
where
    C: From<Action>,
{
    #[must_use]
    pub fn new(bus: Arc<ActionBus<C>>, registry: Arc<ParameterRegistry>) -> Self {
        Self { bus, registry }
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

    pub fn handle(&self, request: ControlRequest) -> Result<ControlResponse, ControlError> {
        match request {
            ControlRequest::Action { action } => {
                self.bus
                    .dispatch(Action::parse(&action)?)
                    .map_err(|_: BusFull| ControlError::QueueFull)?;
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

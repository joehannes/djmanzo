use dj_control::{ActionBus, BusFull, ParameterRegistry};
use dj_core::{Action, ParamId};
use serde::{Deserialize, Serialize};

/// Versioned, JSON-serializable control message for WebSocket and OSC bridges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Parse and dispatch the public text action grammar (for example `deck 1 play`).
    Action { action: String },
    /// Return the named, stable parameter map. This avoids UI/DOM scraping.
    Parameters,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Accepted,
    Parameters { values: Vec<NamedParameter> },
    Error { code: &'static str, message: String },
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
#[derive(Debug)]
pub struct ControlService<C> {
    bus: ActionBus<C>,
    registry: ParameterRegistry,
}

impl<C> ControlService<C>
where
    C: From<Action>,
{
    #[must_use]
    pub fn new(bus: ActionBus<C>, registry: ParameterRegistry) -> Self {
        Self { bus, registry }
    }

    /// Handles one frame. Transport implementations supply their own framing,
    /// authentication and rate limits; this method has no socket or engine access.
    pub fn handle_json(&self, json: &str) -> ControlResponse {
        match serde_json::from_str::<ControlRequest>(json) {
            Ok(request) => match self.handle(request) {
                Ok(response) => response,
                Err(error) => ControlResponse::Error {
                    code: error_code(&error),
                    message: error.to_string(),
                },
            },
            Err(error) => ControlResponse::Error {
                code: "bad_request",
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

fn error_code(error: &ControlError) -> &'static str {
    match error {
        ControlError::Json(_) => "bad_request",
        ControlError::Action(_) => "bad_action",
        ControlError::QueueFull => "queue_full",
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
    #[test]
    fn action_requests_use_the_shared_bus() {
        let (bus, mut consumer) = ActionBus::<Command>::new(2);
        let service = ControlService::new(bus, ParameterRegistry::new());
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
        let (bus, _) = ActionBus::<Command>::new(2);
        let service = ControlService::new(bus, ParameterRegistry::new());
        assert!(matches!(
            service.handle_json("not json"),
            ControlResponse::Error {
                code: "bad_request",
                ..
            }
        ));
        assert!(matches!(
            service.handle_json(r#"{"type":"action","action":"deck nope play"}"#),
            ControlResponse::Error {
                code: "bad_action",
                ..
            }
        ));
    }
}

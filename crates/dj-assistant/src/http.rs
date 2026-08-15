//! JSON over HTTP, behind a trait.
//!
//! Same reasoning as `dj-sources`' transport: the risky part of talking to a
//! provider is *parsing what comes back*, and every provider returns a
//! different shape that changes without notice. Behind a trait, each one's
//! parsing is tested against a captured response — no key, no network, no rate
//! limit — which is the only way those tests can run in CI.
//!
//! A separate trait from `dj-sources`' rather than a shared one: that is
//! GET-and-form shaped, this is POST-JSON shaped, and coupling two crates
//! together to save forty lines would be a worse trade than the duplication.

use crate::provider::AssistantError;
use serde_json::Value;
use std::time::Duration;

/// Generous compared to a search box, because a model genuinely takes this long
/// to think, and a planning request that gets cut off at ten seconds is worse
/// than useless.
const TIMEOUT: Duration = Duration::from_secs(90);

#[async_trait::async_trait]
pub trait HttpJson: Send + Sync + std::fmt::Debug {
    async fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Value, HttpError>;

    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
    ) -> Result<Value, HttpError>;
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("{0}")]
    Transport(String),
    #[error("server returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("response was not the JSON we expected: {0}")]
    Decode(String),
}

impl HttpError {
    #[must_use]
    pub fn into_assistant_error(self, provider: &'static str) -> AssistantError {
        match self {
            HttpError::Transport(message) => AssistantError::Network { provider, message },
            other => AssistantError::BadResponse {
                provider,
                message: other.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReqwestJson {
    inner: reqwest::Client,
}

impl ReqwestJson {
    /// # Errors
    /// If the TLS backend cannot be initialised.
    pub fn new() -> Result<Self, HttpError> {
        Ok(Self {
            inner: reqwest::Client::builder()
                .timeout(TIMEOUT)
                .user_agent(concat!("djmanzo/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| HttpError::Transport(e.to_string()))?,
        })
    }
}

async fn read(response: reqwest::Response) -> Result<Value, HttpError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| HttpError::Transport(e.to_string()))?;
    if !status.is_success() {
        // Truncated: provider error bodies can be an entire HTML page, and the
        // useful part is always at the front.
        let body: String = body.chars().take(400).collect();
        return Err(HttpError::Status {
            status: status.as_u16(),
            body: if body.is_empty() {
                "(empty)".into()
            } else {
                body
            },
        });
    }
    serde_json::from_str(&body).map_err(|e| HttpError::Decode(e.to_string()))
}

#[async_trait::async_trait]
impl HttpJson for ReqwestJson {
    async fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Value, HttpError> {
        let mut request = self.inner.get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        read(
            request
                .send()
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))?,
        )
        .await
    }

    async fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
    ) -> Result<Value, HttpError> {
        let mut request = self.inner.post(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        read(
            request
                .json(body)
                .send()
                .await
                .map_err(|e| HttpError::Transport(e.to_string()))?,
        )
        .await
    }
}

/// Answers from a script instead of the network.
#[derive(Debug, Default)]
pub struct StubHttp {
    responses: std::sync::Mutex<Vec<Result<Value, String>>>,
    pub requests: std::sync::Mutex<Vec<(String, Option<Value>)>>,
}

impl StubHttp {
    #[must_use]
    pub fn new(responses: Vec<Value>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses.into_iter().map(Ok).collect()),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn failing(message: &str) -> Self {
        Self {
            responses: std::sync::Mutex::new(vec![Err(message.to_owned())]),
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// The body of the last POST, so a test can assert what was actually sent.
    #[must_use]
    pub fn last_body(&self) -> Option<Value> {
        self.requests.lock().ok()?.last()?.1.clone()
    }

    #[must_use]
    pub fn call_count(&self) -> usize {
        self.requests.lock().map(|r| r.len()).unwrap_or(0)
    }

    fn next(&self, url: &str, body: Option<Value>) -> Result<Value, HttpError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push((url.to_owned(), body));
        }
        let mut responses = self
            .responses
            .lock()
            .map_err(|_| HttpError::Transport("poisoned".into()))?;
        if responses.is_empty() {
            return Err(HttpError::Transport("stub ran out of responses".into()));
        }
        responses.remove(0).map_err(HttpError::Transport)
    }
}

#[async_trait::async_trait]
impl HttpJson for StubHttp {
    async fn get(&self, url: &str, _: &[(String, String)]) -> Result<Value, HttpError> {
        self.next(url, None)
    }

    async fn post(
        &self,
        url: &str,
        _: &[(String, String)],
        body: &Value,
    ) -> Result<Value, HttpError> {
        self.next(url, Some(body.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn the_stub_records_what_was_posted() {
        let stub = StubHttp::new(vec![json!({"ok": true})]);
        stub.post("https://x/chat", &[], &json!({"model": "m"}))
            .await
            .unwrap();
        assert_eq!(stub.last_body().unwrap()["model"], "m");
        assert_eq!(stub.call_count(), 1);
    }

    /// A 401 is a key problem, not a network problem; telling the user
    /// "network error" would send them to check their wifi.
    #[test]
    fn a_bad_status_is_a_response_problem_not_a_network_one() {
        let error = HttpError::Status {
            status: 401,
            body: "bad key".into(),
        }
        .into_assistant_error("Test");
        assert!(matches!(error, AssistantError::BadResponse { .. }));

        let error = HttpError::Transport("dns".into()).into_assistant_error("Test");
        assert!(matches!(error, AssistantError::Network { .. }));
    }
}

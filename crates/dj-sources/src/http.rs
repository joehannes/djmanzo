//! The one place this crate talks to the network.
//!
//! Behind a trait, for a reason that pays for itself immediately: the risky part
//! of a source integration is not making the request, it is *parsing what comes
//! back*. Field names change, fields go missing, a duration arrives in
//! milliseconds where the docs said seconds. With HTTP behind a trait, every
//! provider's parsing is testable against a captured response with no network,
//! no key and no rate limit — which is the only way those tests can run in CI.

use crate::provider::SourceError;
use serde_json::Value;
use std::time::Duration;

/// Requests give up after this. A DJ waiting on a search box has a much shorter
/// patience than a default HTTP timeout assumes, and a venue's wifi is exactly
/// where a hung request happens.
const TIMEOUT: Duration = Duration::from_secs(12);

#[async_trait::async_trait]
pub trait HttpClient: Send + Sync + std::fmt::Debug {
    async fn get_json(&self, url: &str, headers: &[(String, String)])
    -> Result<Value, HttpError>;

    async fn post_form(
        &self,
        url: &str,
        headers: &[(String, String)],
        form: &[(&str, &str)],
    ) -> Result<Value, HttpError>;
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("request failed: {0}")]
    Transport(String),
    #[error("server returned {status}: {body}")]
    Status { status: u16, body: String },
    #[error("response was not the JSON we expected: {0}")]
    Decode(String),
}

impl HttpError {
    /// Turn a transport-level failure into something a provider can return.
    #[must_use]
    pub fn into_source_error(self, provider: &'static str) -> SourceError {
        match self {
            HttpError::Transport(message) => SourceError::Network { provider, message },
            other => SourceError::BadResponse {
                provider,
                message: other.to_string(),
            },
        }
    }
}

/// The real client.
#[derive(Debug, Clone)]
pub struct ReqwestClient {
    inner: reqwest::Client,
}

impl ReqwestClient {
    /// # Errors
    /// If the TLS backend cannot be initialised.
    pub fn new() -> Result<Self, HttpError> {
        let inner = reqwest::Client::builder()
            .timeout(TIMEOUT)
            // Identify ourselves. Several of these APIs rate-limit unknown
            // agents harder, and an anonymous client is rude besides.
            .user_agent(concat!("djmanzo/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        Ok(Self { inner })
    }
}

async fn read_json(response: reqwest::Response) -> Result<Value, HttpError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| HttpError::Transport(e.to_string()))?;
    if !status.is_success() {
        // Truncated: an error body can be an entire HTML page, and the useful
        // part is always at the front.
        let mut body: String = body.chars().take(400).collect();
        if body.is_empty() {
            body.push_str("(empty)");
        }
        return Err(HttpError::Status {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(|e| HttpError::Decode(e.to_string()))
}

#[async_trait::async_trait]
impl HttpClient for ReqwestClient {
    async fn get_json(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<Value, HttpError> {
        let mut request = self.inner.get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = request
            .send()
            .await
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        read_json(response).await
    }

    async fn post_form(
        &self,
        url: &str,
        headers: &[(String, String)],
        form: &[(&str, &str)],
    ) -> Result<Value, HttpError> {
        let mut request = self.inner.post(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = request
            .form(form)
            .send()
            .await
            .map_err(|e| HttpError::Transport(e.to_string()))?;
        read_json(response).await
    }
}

/// A client that answers from a script instead of the network.
///
/// Every provider's response parsing is tested through this.
#[derive(Debug, Default)]
pub struct StubClient {
    responses: std::sync::Mutex<Vec<Result<Value, String>>>,
    /// URLs actually requested, so a test can assert the query was built right.
    pub requested: std::sync::Mutex<Vec<String>>,
}

impl StubClient {
    #[must_use]
    pub fn new(responses: Vec<Value>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses.into_iter().map(Ok).collect()),
            requested: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// A client that always fails, for testing the unhappy path.
    #[must_use]
    pub fn failing(message: &str) -> Self {
        Self {
            responses: std::sync::Mutex::new(vec![Err(message.to_owned())]),
            requested: std::sync::Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn last_url(&self) -> String {
        self.requested
            .lock()
            .map(|urls| urls.last().cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    fn next(&self, url: &str) -> Result<Value, HttpError> {
        if let Ok(mut urls) = self.requested.lock() {
            urls.push(url.to_owned());
        }
        let mut responses = self
            .responses
            .lock()
            .map_err(|_| HttpError::Transport("poisoned".into()))?;
        if responses.is_empty() {
            return Err(HttpError::Transport("stub ran out of responses".into()));
        }
        match responses.remove(0) {
            Ok(value) => Ok(value),
            Err(message) => Err(HttpError::Transport(message)),
        }
    }
}

#[async_trait::async_trait]
impl HttpClient for StubClient {
    async fn get_json(&self, url: &str, _: &[(String, String)]) -> Result<Value, HttpError> {
        self.next(url)
    }

    async fn post_form(
        &self,
        url: &str,
        _: &[(String, String)],
        _: &[(&str, &str)],
    ) -> Result<Value, HttpError> {
        self.next(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn the_stub_answers_in_order_and_records_urls() {
        let stub = StubClient::new(vec![json!({"n": 1}), json!({"n": 2})]);
        assert_eq!(stub.get_json("a", &[]).await.unwrap()["n"], 1);
        assert_eq!(stub.get_json("b", &[]).await.unwrap()["n"], 2);
        assert_eq!(stub.last_url(), "b");
    }

    #[tokio::test]
    async fn running_out_of_stub_responses_is_an_error_not_a_panic() {
        let stub = StubClient::new(vec![]);
        assert!(stub.get_json("a", &[]).await.is_err());
    }

    #[test]
    fn transport_failures_are_reported_as_network_problems() {
        let error = HttpError::Transport("dns".into()).into_source_error("Test");
        assert!(matches!(error, SourceError::Network { .. }));
    }

    /// A 401 is a credentials problem, not a network problem, and telling the
    /// user "network error" would send them to check their wifi.
    #[test]
    fn a_bad_status_is_reported_as_a_bad_response() {
        let error = HttpError::Status {
            status: 401,
            body: "unauthorized".into(),
        }
        .into_source_error("Test");
        assert!(matches!(error, SourceError::BadResponse { .. }));
    }
}

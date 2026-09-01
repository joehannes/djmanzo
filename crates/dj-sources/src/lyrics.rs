//! Words, so a half-remembered line can find a record.
//!
//! # Why LRCLIB and nothing else
//!
//! Every other lyrics service djmanzo could reach wants either a key, a
//! contract, or both, and most of them forbid storing what they return — which
//! is exactly what a "search the words you remember" feature has to do, because
//! searching means having the text before the question is asked.
//!
//! [LRCLIB](https://lrclib.net) is the exception: no key, no account, no rate
//! limit worth the name, and a community database explicitly meant to be used
//! this way. Its own client software is MIT. It asks for a `User-Agent` that
//! says who is calling, which djmanzo sends.
//!
//! # What this is not
//!
//! **It is not a way to find a song nobody owns.** LRCLIB is looked up *by
//! artist and title*, not searched by lyric text — so this fetches the words
//! for records djmanzo already knows about, and the searching happens locally
//! afterwards, in `dj_library`. That is a real limit and it is the honest
//! shape of the thing: djmanzo can find the song in *your* collection from a
//! line you half remember, and cannot find one you have never had.
//!
//! # Why the duration is sent
//!
//! Because there are four recordings of most songs worth playing, and the
//! seven-minute one has different words in different places. LRCLIB matches on
//! duration for exactly that reason, and an answer for the wrong edit is worse
//! than no answer -- it puts a line in the DJ's search index that the record
//! does not contain.

use crate::http::{HttpClient, HttpError};
use std::sync::Arc;

const BASE: &str = "https://lrclib.net/api";

/// How far the duration may be off before a match is refused, in seconds.
///
/// Two. LRCLIB's own client uses the same tolerance, and it is about the
/// difference between two rips of one master rather than between two edits.
pub const CLOSE_ENOUGH: u32 = 2;

/// Lyrics for one recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    /// The words, one line per line. Empty for an instrumental.
    pub plain: String,
    /// The same words with timestamps, when the database has them.
    ///
    /// Kept even though nothing reads them yet: they arrive in the same
    /// response, they are the expensive half to fetch again, and a lyric
    /// display is the obvious next thing to want.
    pub synced: Option<String>,
    /// Whether the database says this recording has no words at all.
    pub instrumental: bool,
}

impl Lyrics {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plain.trim().is_empty() && self.synced.is_none()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("no lyrics for that recording")]
    NotFound,
    #[error("could not reach the lyrics database: {0}")]
    Unreachable(String),
    #[error("the lyrics database answered something unexpected: {0}")]
    Unexpected(String),
}

/// The lyrics database, as djmanzo uses it.
#[derive(Debug)]
pub struct LyricsSource {
    http: Arc<dyn HttpClient>,
}

impl LyricsSource {
    #[must_use]
    pub fn new(http: Arc<dyn HttpClient>) -> Self {
        Self { http }
    }

    /// The words for one recording.
    ///
    /// `seconds` is the track's own length; passing it is what stops the
    /// four-minute single's words being filed against the seven-minute mix.
    ///
    /// # Errors
    /// When the database has nothing for it, cannot be reached, or answers
    /// something that is not what it documents.
    pub async fn words_for(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
        seconds: u32,
    ) -> Result<Lyrics, LyricsError> {
        let url = get_url(artist, title, album, seconds);
        let headers = [("User-Agent".to_owned(), user_agent())];
        match self.http.get_json(&url, &headers).await {
            Ok(body) => parse(&body).ok_or(LyricsError::NotFound),
            // A 404 is the database saying "not here", which is an answer and
            // not a failure. Anything else is a failure.
            Err(HttpError::Status { status: 404, .. }) => Err(LyricsError::NotFound),
            Err(HttpError::Transport(why)) => Err(LyricsError::Unreachable(why)),
            Err(other) => Err(LyricsError::Unexpected(other.to_string())),
        }
    }
}

/// What djmanzo calls itself to LRCLIB.
///
/// Asked for by the service, and the right thing to send anyway: an operator
/// looking at a spike in traffic should be able to tell who is causing it and
/// find somebody to ask about it.
fn user_agent() -> String {
    format!(
        "djmanzo/{} (https://github.com/joehannes/djmanzo)",
        env!("CARGO_PKG_VERSION")
    )
}

fn get_url(artist: &str, title: &str, album: Option<&str>, seconds: u32) -> String {
    let mut url = format!(
        "{BASE}/get?artist_name={}&track_name={}&duration={seconds}",
        urlencoding::encode(artist),
        urlencoding::encode(title),
    );
    // Sent only when there is one. An empty album narrows nothing and makes
    // the request different from the one a retry would send.
    if let Some(album) = album.filter(|a| !a.trim().is_empty()) {
        url.push_str("&album_name=");
        url.push_str(&urlencoding::encode(album));
    }
    url
}

fn parse(body: &serde_json::Value) -> Option<Lyrics> {
    // The service answers a miss with a JSON body carrying a status code
    // rather than only with an HTTP one, so both shapes have to be read.
    if body.get("statusCode").and_then(serde_json::Value::as_i64) == Some(404) {
        return None;
    }
    let instrumental = body
        .get("instrumental")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let plain = body
        .get("plainLyrics")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let synced = body
        .get("syncedLyrics")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned);

    let lyrics = Lyrics {
        plain,
        synced,
        instrumental,
    };
    // An instrumental is an answer: it is why there are no words, and storing
    // it stops djmanzo asking again every time the library is swept.
    if lyrics.is_empty() && !lyrics.instrumental {
        return None;
    }
    Some(lyrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::StubClient;
    use serde_json::json;

    fn client(body: serde_json::Value) -> (Arc<StubClient>, LyricsSource) {
        let stub = Arc::new(StubClient::new(vec![body]));
        let source = LyricsSource::new(Arc::clone(&stub) as Arc<dyn HttpClient>);
        (stub, source)
    }

    /// **The request carries the duration, so the wrong edit cannot answer.**
    #[test]
    fn the_request_names_the_recording_not_just_the_song() {
        let url = get_url("Aventura", "Obsesión", Some("We Broke the Rules"), 245);
        assert!(url.contains("artist_name=Aventura"), "{url}");
        assert!(url.contains("duration=245"), "{url}");
        assert!(url.contains("album_name=We%20Broke%20the%20Rules"), "{url}");
        // Accents survive the trip.
        assert!(url.contains("Obsesi%C3%B3n"), "{url}");
    }

    /// **An album nobody filled in is not sent as an empty one.**
    ///
    /// An empty `album_name` narrows nothing and makes the URL different from
    /// the one a retry would build, which is how a cache stops working.
    #[test]
    fn a_blank_album_is_left_out() {
        let url = get_url("Aventura", "Obsesión", Some("   "), 245);
        assert!(!url.contains("album_name"), "{url}");
        assert!(!get_url("a", "b", None, 1).contains("album_name"));
    }

    #[tokio::test]
    async fn words_come_back_with_their_timings() {
        let (stub, source) = client(json!({
            "plainLyrics": "Son las doce de la noche\nY no puedo dormir",
            "syncedLyrics": "[00:12.00] Son las doce de la noche",
            "instrumental": false,
        }));
        let found = source
            .words_for("Aventura", "Obsesión", None, 245)
            .await
            .expect("lyrics");
        assert!(found.plain.contains("Son las doce"));
        assert!(found.synced.is_some());
        assert!(!found.instrumental);
        assert!(stub.last_url().contains("lrclib.net"));
    }

    /// **An instrumental is an answer, not a miss.**
    ///
    /// Otherwise every sweep of the library asks the database again about the
    /// same forty records that will never have words.
    #[tokio::test]
    async fn an_instrumental_is_a_result() {
        let (_stub, source) = client(json!({
            "plainLyrics": "",
            "syncedLyrics": serde_json::Value::Null,
            "instrumental": true,
        }));
        let found = source.words_for("a", "b", None, 100).await.expect("answer");
        assert!(found.instrumental);
        assert!(found.is_empty());
    }

    /// **A miss reported in the body is still a miss.**
    #[tokio::test]
    async fn a_status_code_in_the_body_is_read() {
        let (_stub, source) = client(json!({
            "statusCode": 404,
            "error": "Not Found",
            "message": "Failed to find specified track",
        }));
        assert!(matches!(
            source.words_for("a", "b", None, 100).await,
            Err(LyricsError::NotFound)
        ));
    }

    /// **A body with nothing in it is a miss rather than empty lyrics.**
    #[tokio::test]
    async fn an_empty_answer_is_not_stored_as_empty_words() {
        let (_stub, source) = client(json!({}));
        assert!(matches!(
            source.words_for("a", "b", None, 100).await,
            Err(LyricsError::NotFound)
        ));
    }

    /// **A network failure is told apart from a miss.**
    ///
    /// They need opposite responses: a miss is remembered so djmanzo stops
    /// asking, and a failure is retried later.
    #[tokio::test]
    async fn a_dead_network_is_not_a_missing_lyric() {
        let stub = Arc::new(StubClient::failing("no route to host"));
        let source = LyricsSource::new(stub as Arc<dyn HttpClient>);
        assert!(matches!(
            source.words_for("a", "b", None, 100).await,
            Err(LyricsError::Unreachable(_))
        ));
    }

    #[test]
    fn djmanzo_says_who_it_is() {
        let agent = user_agent();
        assert!(agent.starts_with("djmanzo/"), "{agent}");
        assert!(agent.contains("github.com"), "{agent}");
    }
}

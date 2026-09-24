//! §108: what is playing, said to a live stream.
//!
//! > improve integration of social networks
//!
//! Most of a DJ's social presence now is a live stream — Twitch, YouTube,
//! Instagram, TikTok, Kick — sent from OBS or software like it, and the one
//! question every chat asks is *what is this track?* This answers it on the
//! stream itself, in the two ways OBS can carry text:
//!
//! - a **Browser source** pointed at this server: a page with a transparent
//!   ground and the record in large type, which changes when the record does;
//! - a **Text source** set to *Read from file*, which `dj_app::live` writes.
//!
//! # Why loopback, when the request page faces the room
//!
//! [`crate::front`] faces the network because a phone in the room has to reach
//! it. Nothing in the room needs this: the streaming software runs on the DJ's
//! own machine. So it listens on `127.0.0.1` only, and its handler holds one
//! string and nothing else — no book, no bus, no deck. What a page on some
//! other site could learn by reaching it is the name of the record the DJ has
//! chosen to put on their stream.
//!
//! # Why the page has a script, when the request page has none
//!
//! The request page has no script because a phone on venue wifi may have no
//! route out and a slow one in. This page is read by the streaming software on
//! the same machine, and a page that reloaded itself every few seconds would
//! flash on the stream each time. So it asks for the words and changes only
//! when they change; without a script it falls back to reloading.

use crate::web::{Handler, Incoming, Method, Reply};
use std::sync::{Arc, Mutex};

/// Where the overlay listens unless told otherwise: the request page's port,
/// plus one.
pub const DEFAULT_PORT: u16 = 7332;

/// How often the page asks for the words, in milliseconds. OBS's own text
/// source reads its file once a second; this matches it, near enough.
pub const ASK_EVERY_MS: u32 = 1_500;

/// The words on the stream, shared between whoever decides them and the page.
#[derive(Debug, Default)]
pub struct NowPlaying {
    text: Mutex<String>,
}

impl NowPlaying {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Say this. Empty says nothing, and the page shows nothing.
    pub fn set(&self, text: &str) {
        if let Ok(mut held) = self.text.lock() {
            text.trim().clone_into(&mut held);
        }
    }

    #[must_use]
    pub fn get(&self) -> String {
        self.text.lock().map(|t| t.clone()).unwrap_or_default()
    }
}

/// The handler: the page, the words, and nothing else.
#[derive(Debug, Clone)]
pub struct Overlay {
    now: Arc<NowPlaying>,
}

impl Overlay {
    #[must_use]
    pub fn new(now: Arc<NowPlaying>) -> Self {
        Self { now }
    }
}

impl Handler for Overlay {
    fn handle(&self, request: &Incoming) -> Reply {
        let mut reply = match (request.method, request.path.as_str()) {
            (Method::Get, "/") => Reply::html(page(&self.now.get())),
            (Method::Get, "/now.txt") => Reply::text(200, self.now.get()),
            _ => return Reply::not_found(),
        };
        // Always asked afresh: a cached answer is yesterday's record on
        // tonight's stream.
        reply
            .headers
            .push(("Cache-Control".to_owned(), "no-store".to_owned()));
        reply
    }
}

/// The overlay page, saying `now`.
///
/// Transparent, so it sits on whatever the stream shows; white type with a
/// dark edge, so it reads on a light scene and a dark one alike.
#[must_use]
pub fn page(now: &str) -> String {
    let now = crate::page::escape(now);
    let quiet = if now.is_empty() {
        " class=\"quiet\""
    } else {
        ""
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>djmanzo - now playing</title>
<noscript><meta http-equiv="refresh" content="3"></noscript>
<style>
  html, body {{ margin: 0; background: transparent; overflow: hidden; }}
  body {{ font: 700 44px/1.2 system-ui, -apple-system, "Segoe UI", sans-serif; color: #fff; padding: 16px 24px; }}
  .said {{ font-size: 0.42em; letter-spacing: 0.18em; text-transform: uppercase; opacity: 0.8; }}
  .said, #now {{ text-shadow: 0 0 2px #000, 0 2px 8px rgb(0 0 0 / 0.7); }}
  #now {{ white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
  #now.in {{ animation: in 600ms ease-out; }}
  @keyframes in {{ from {{ opacity: 0; transform: translateY(8px); }} to {{ opacity: 1; transform: none; }} }}
  body.quiet {{ visibility: hidden; }}
</style>
</head>
<body{quiet}>
<div class="said">Now playing</div>
<div id="now">{now}</div>
<script>
  const line = document.getElementById("now");
  async function look() {{
    try {{
      const answer = await fetch("/now.txt", {{ cache: "no-store" }});
      const words = (await answer.text()).trim();
      if (words !== line.textContent) {{
        line.classList.remove("in");
        void line.offsetWidth;
        line.textContent = words;
        line.classList.add("in");
      }}
      document.body.classList.toggle("quiet", words === "");
    }} catch (_) {{}}
  }}
  setInterval(look, {ASK_EVERY_MS});
</script>
</body>
</html>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::WebServer;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};

    fn talk(address: SocketAddr, request: &str) -> String {
        let mut stream = TcpStream::connect(address).expect("connect");
        stream.write_all(request.as_bytes()).expect("write");
        stream.flush().expect("flush");
        let mut got = String::new();
        let _ = stream.read_to_string(&mut got);
        got
    }

    fn get(address: SocketAddr, path: &str) -> String {
        talk(
            address,
            &format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"),
        )
    }

    fn serving(now: &Arc<NowPlaying>) -> WebServer {
        WebServer::start(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(Overlay::new(Arc::clone(now))),
        )
        .expect("serve")
    }

    /// **The stream says the record, and changes when it does** — over a
    /// real socket, as the streaming software asks.
    #[test]
    fn the_stream_is_told_what_is_playing() {
        let now = Arc::new(NowPlaying::new());
        now.set("Aventura - Obsesión");
        let server = serving(&now);
        let page = get(server.address(), "/");
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("Aventura - Obsesión"));
        assert!(page.contains("background: transparent"));
        assert!(
            page.to_ascii_lowercase()
                .contains("cache-control: no-store")
        );

        now.set("Juan Luis Guerra - Burbujas de amor");
        let words = get(server.address(), "/now.txt");
        assert!(
            words.ends_with("Juan Luis Guerra - Burbujas de amor"),
            "{words}"
        );
        assert!(words.contains("text/plain"));
    }

    /// Nothing playing is a page that shows nothing, not the word "None" or
    /// the last record.
    #[test]
    fn nothing_playing_says_nothing() {
        let now = Arc::new(NowPlaying::new());
        now.set("A - B");
        now.set("   ");
        assert_eq!(now.get(), "");
        assert!(page("").contains("<body class=\"quiet\">"));
        assert!(!page("A - B").contains("<body class=\"quiet\">"));
    }

    /// A title is the one thing on this page that came from outside, and it
    /// is text on it, never markup.
    #[test]
    fn a_title_cannot_become_markup() {
        let drawn = page("<script>alert(1)</script> & \"Friends\"");
        assert!(drawn.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;Friends&quot;"));
        assert_eq!(drawn.matches("<script>").count(), 1, "only the page's own");
    }

    /// **The shape is the safety**: two paths, read-only. Anything else — a
    /// deck, the request book, a form — is not here.
    #[test]
    fn it_answers_two_paths_and_nothing_else() {
        let now = Arc::new(NowPlaying::new());
        let server = serving(&now);
        for path in [
            "/request",
            "/deck/1/play",
            "/api/state",
            "/../../etc/passwd",
        ] {
            assert!(
                get(server.address(), path).starts_with("HTTP/1.1 404"),
                "{path}"
            );
        }
        let posted = talk(
            server.address(),
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        assert!(posted.starts_with("HTTP/1.1 404"), "{posted}");
    }
}

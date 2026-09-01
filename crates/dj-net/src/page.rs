//! The page a stranger's phone sees.
//!
//! Everything here is decided by where it is read: a dark room, one hand, a
//! phone that may be five years old, on a venue's wifi that may have no route
//! to the internet at all.
//!
//! So: **one file, no requests after the first.** No fonts, no framework, no
//! analytics, no icons — anything fetched from elsewhere is a spinner in a
//! room where nothing loads. **It works with JavaScript off**, because the
//! request is a `<form>` and a `<button>` and nothing else; a phone with a
//! broken script must not silently eat somebody's song. And **every piece of
//! text that came from a phone is escaped on the way back out**, because this
//! page shows the room what the room typed, which is the oldest way there is
//! to put a script on somebody else's screen.
//!
//! The words are a struct rather than literals because this page is read by
//! the room and not by the DJ. djmanzo's own interface can be in English; a
//! request box in a bar in Santo Domingo cannot be.

use crate::room::{Ask, LONGEST_ASK, Outcome};

/// The visible text of the page, in one language.
#[derive(Debug, Clone, Copy)]
pub struct Words {
    /// For `<html lang>`; also what the DJ picks by.
    pub tag: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub playing_now: &'static str,
    pub field_label: &'static str,
    pub placeholder: &'static str,
    pub send: &'static str,
    pub asked_already: &'static str,
    pub me_too: &'static str,
    pub counted: &'static str,
    pub one_voice: &'static str,
    /// Formatted with the count, as `{n}`.
    pub many_voices: &'static str,
    pub closed: &'static str,
    pub said_added: &'static str,
    pub said_seconded: &'static str,
    pub said_yours: &'static str,
    pub said_empty: &'static str,
    pub said_long: &'static str,
    pub said_many: &'static str,
    pub said_full: &'static str,
    pub said_gone: &'static str,
}

impl Words {
    #[must_use]
    pub const fn english() -> Self {
        Self {
            tag: "en",
            name: "English",
            title: "Request a song",
            playing_now: "Playing now",
            field_label: "What do you want to hear?",
            placeholder: "Artist — song",
            send: "Send it",
            asked_already: "Already asked for",
            me_too: "Me too",
            counted: "Counted",
            one_voice: "1 person",
            many_voices: "{n} people",
            closed: "The DJ has closed requests for now.",
            said_added: "Got it. The DJ can see it.",
            said_seconded: "Counted — somebody had the same idea.",
            said_yours: "You already asked for that one.",
            said_empty: "Type a song first.",
            said_long: "A bit shorter, please.",
            said_many: "That's a few already. Try again in a while.",
            said_full: "The list is full tonight.",
            said_gone: "That one is no longer on the list.",
        }
    }

    /// Spanish, for the rooms djmanzo was written for.
    #[must_use]
    pub const fn spanish() -> Self {
        Self {
            tag: "es",
            name: "Español",
            title: "Pide una canción",
            playing_now: "Sonando ahora",
            field_label: "¿Qué quieres escuchar?",
            placeholder: "Artista — canción",
            send: "Enviar",
            asked_already: "Ya pedidas",
            me_too: "Yo también",
            counted: "Contado",
            one_voice: "1 persona",
            many_voices: "{n} personas",
            closed: "El DJ cerró las peticiones por ahora.",
            said_added: "Listo. El DJ ya la ve.",
            said_seconded: "Contada — alguien pidió lo mismo.",
            said_yours: "Ya pediste esa.",
            said_empty: "Escribe una canción primero.",
            said_long: "Un poco más corta, por favor.",
            said_many: "Ya son varias. Prueba de nuevo en un rato.",
            said_full: "La lista está llena esta noche.",
            said_gone: "Esa ya no está en la lista.",
        }
    }

    /// Every language the page is written in.
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![Self::english(), Self::spanish()]
    }

    /// By `tag`, falling back to English rather than failing — a page that
    /// will not render is worse than a page in the wrong language.
    #[must_use]
    pub fn by_tag(tag: &str) -> Self {
        Self::all()
            .into_iter()
            .find(|w| w.tag == tag)
            .unwrap_or_else(Self::english)
    }
}

/// What the page says happened, carried across the redirect after a POST.
///
/// A short tag in the URL rather than a session cookie: a cookie would need
/// storage and consent for a sentence that is true for four seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    Added,
    Seconded,
    Yours,
    Empty,
    TooLong,
    TooMany,
    Full,
    Gone,
}

impl Said {
    #[must_use]
    pub fn of(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Added(_) => Self::Added,
            Outcome::Seconded(_) => Self::Seconded,
            Outcome::AlreadyYours(_) => Self::Yours,
            Outcome::Empty => Self::Empty,
            Outcome::TooLong => Self::TooLong,
            Outcome::TooManyFromYou => Self::TooMany,
            Outcome::BookIsFull => Self::Full,
            Outcome::NoSuchAsk => Self::Gone,
        }
    }

    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Added => "ok",
            Self::Seconded => "too",
            Self::Yours => "yours",
            Self::Empty => "empty",
            Self::TooLong => "long",
            Self::TooMany => "many",
            Self::Full => "full",
            Self::Gone => "gone",
        }
    }

    #[must_use]
    pub fn by_tag(tag: &str) -> Option<Self> {
        [
            Self::Added,
            Self::Seconded,
            Self::Yours,
            Self::Empty,
            Self::TooLong,
            Self::TooMany,
            Self::Full,
            Self::Gone,
        ]
        .into_iter()
        .find(|s| s.tag() == tag)
    }

    /// Whether this is good news, which is the only thing the colour says.
    #[must_use]
    pub fn is_good(self) -> bool {
        matches!(self, Self::Added | Self::Seconded)
    }

    fn sentence(self, words: &Words) -> &'static str {
        match self {
            Self::Added => words.said_added,
            Self::Seconded => words.said_seconded,
            Self::Yours => words.said_yours,
            Self::Empty => words.said_empty,
            Self::TooLong => words.said_long,
            Self::TooMany => words.said_many,
            Self::Full => words.said_full,
            Self::Gone => words.said_gone,
        }
    }
}

/// One row of the already-asked list.
#[derive(Debug, Clone)]
pub struct Line {
    pub id: u64,
    pub text: String,
    pub voices: u32,
    /// Whether this phone has already been counted for it.
    pub mine: bool,
}

impl Line {
    /// The rows to show, from the book's own ordering.
    #[must_use]
    pub fn from_asks(asks: &[&Ask], phone: &str, most: usize) -> Vec<Self> {
        asks.iter()
            .take(most)
            .map(|ask| Self {
                id: ask.id,
                text: ask.text.clone(),
                voices: ask.voices,
                mine: ask.is_from(phone),
            })
            .collect()
    }
}

/// Everything the page needs to draw itself.
#[derive(Debug, Clone)]
pub struct View<'a> {
    pub words: Words,
    /// The heading — the night's name, or the venue's.
    pub heading: &'a str,
    pub playing: Option<&'a str>,
    pub lines: Vec<Line>,
    pub said: Option<Said>,
    /// When false, the form is gone and only [`Words::closed`] remains.
    pub open: bool,
}

/// The most rows the page shows.
///
/// Enough to stop the duplicates, short enough that the form stays above the
/// fold on a phone — which matters, because the form is what people came for
/// and a list they have to scroll past reads as a wall.
pub const ROWS: usize = 8;

/// The whole page, as one self-contained document.
#[must_use]
pub fn request_page(view: &View) -> String {
    let w = &view.words;
    let mut html = String::with_capacity(4096);
    html.push_str("<!doctype html>\n<html lang=\"");
    html.push_str(w.tag);
    html.push_str("\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n");
    html.push_str("<meta name=\"robots\" content=\"noindex\">\n<title>");
    html.push_str(&escape(view.heading));
    html.push_str(" — ");
    html.push_str(w.title);
    html.push_str("</title>\n<style>\n");
    html.push_str(STYLE);
    html.push_str("</style>\n</head>\n<body>\n<main>\n");

    html.push_str("<h1>");
    html.push_str(&escape(view.heading));
    html.push_str("</h1>\n");

    if let Some(playing) = view.playing {
        html.push_str("<p class=\"now\"><span>");
        html.push_str(w.playing_now);
        html.push_str("</span> ");
        html.push_str(&escape(playing));
        html.push_str("</p>\n");
    }

    if let Some(said) = view.said {
        html.push_str("<p class=\"said ");
        html.push_str(if said.is_good() { "good" } else { "no" });
        html.push_str("\" role=\"status\">");
        html.push_str(said.sentence(w));
        html.push_str("</p>\n");
    }

    if view.open {
        html.push_str("<form method=\"post\" action=\"/request\">\n<label for=\"song\">");
        html.push_str(w.field_label);
        html.push_str("</label>\n<input id=\"song\" name=\"song\" type=\"text\" autocomplete=\"off\" enterkeyhint=\"send\" maxlength=\"");
        html.push_str(&LONGEST_ASK.to_string());
        html.push_str("\" placeholder=\"");
        html.push_str(w.placeholder);
        html.push_str("\">\n<button type=\"submit\">");
        html.push_str(w.send);
        html.push_str("</button>\n</form>\n");
    } else {
        html.push_str("<p class=\"closed\">");
        html.push_str(w.closed);
        html.push_str("</p>\n");
    }

    if !view.lines.is_empty() {
        html.push_str("<h2>");
        html.push_str(w.asked_already);
        html.push_str("</h2>\n<ul class=\"asks\">\n");
        for line in &view.lines {
            html.push_str("<li><span class=\"what\">");
            html.push_str(&escape(&line.text));
            html.push_str("</span><span class=\"who\">");
            html.push_str(&voices(w, line.voices));
            html.push_str("</span>");
            if line.mine {
                html.push_str("<span class=\"mine\">");
                html.push_str(w.counted);
                html.push_str("</span>");
            } else if view.open {
                html.push_str("<form method=\"post\" action=\"/request\"><input type=\"hidden\" name=\"second\" value=\"");
                html.push_str(&line.id.to_string());
                html.push_str("\"><button type=\"submit\" class=\"plus\">");
                html.push_str(w.me_too);
                html.push_str("</button></form>");
            }
            html.push_str("</li>\n");
        }
        html.push_str("</ul>\n");
    }

    html.push_str("<p class=\"mark\">djmanzo</p>\n</main>\n</body>\n</html>\n");
    html
}

fn voices(words: &Words, count: u32) -> String {
    if count == 1 {
        words.one_voice.to_owned()
    } else {
        words.many_voices.replace("{n}", &count.to_string())
    }
}

/// Make text safe to put inside HTML.
///
/// The five characters that can leave a text node or an attribute value. This
/// page exists to show a room what the room typed, so this function is the
/// difference between a request box and a way to run a script on every phone
/// in the building.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Dark, because of the room. Large, because of the one hand.
const STYLE: &str = "\
:root{color-scheme:dark}
*{box-sizing:border-box}
body{margin:0;background:#0b0d10;color:#e9edf2;
 font:16px/1.45 system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;
 padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)}
main{max-width:30rem;margin:0 auto;padding:1.5rem 1.1rem 3rem}
h1{font-size:1.5rem;margin:0 0 .25rem;letter-spacing:.01em}
h2{font-size:.85rem;text-transform:uppercase;letter-spacing:.08em;color:#8b97a6;margin:2rem 0 .5rem}
.now{margin:0 0 1.25rem;color:#b9c4d0;font-size:.95rem}
.now span{color:#8b97a6;margin-right:.4rem}
label{display:block;font-size:1.05rem;margin-bottom:.5rem;color:#cfd8e3}
input{width:100%;padding:.85rem .9rem;border-radius:.6rem;border:1px solid #2b333d;
 background:#141920;color:#e9edf2;font-size:1.1rem}
input:focus{outline:2px solid #4f9dff;outline-offset:1px;border-color:#4f9dff}
button{width:100%;margin-top:.7rem;min-height:3rem;border:0;border-radius:.6rem;
 background:#4f9dff;color:#06121f;font-size:1.05rem;font-weight:600;cursor:pointer}
button:active{background:#3d86e0}
.said{margin:0 0 1rem;padding:.7rem .85rem;border-radius:.55rem;font-size:.98rem}
.said.good{background:#12331f;color:#9fe6b4}
.said.no{background:#331a16;color:#f0b3a8}
.closed{padding:1rem;border-radius:.6rem;background:#141920;color:#b9c4d0}
.asks{list-style:none;margin:0;padding:0}
.asks li{display:flex;align-items:center;gap:.6rem;padding:.6rem 0;border-top:1px solid #1c222a}
.what{flex:1;min-width:0;overflow-wrap:anywhere}
.who{color:#8b97a6;font-size:.85rem;white-space:nowrap}
.mine{color:#7fbf95;font-size:.85rem;white-space:nowrap}
.asks form{margin:0}
.plus{width:auto;margin:0;min-height:2.25rem;padding:0 .8rem;font-size:.9rem;
 background:#1e2833;color:#cfd8e3}
.mark{margin-top:2.5rem;text-align:center;color:#4a5563;font-size:.75rem;letter-spacing:.14em}
";

#[cfg(test)]
mod tests {
    use super::*;

    fn a_view() -> View<'static> {
        View {
            words: Words::english(),
            heading: "Sábado en La Guácara",
            playing: None,
            lines: Vec::new(),
            said: None,
            open: true,
        }
    }

    /// **A request typed as a script comes back as text.**
    ///
    /// The load-bearing test of the whole feature. This page shows the room
    /// what the room typed; without this it shows the room what one person in
    /// the room decided every other phone should run.
    #[test]
    fn what_a_phone_typed_cannot_become_markup() {
        let mut view = a_view();
        view.lines = vec![Line {
            id: 1,
            text: "<script>alert('hi')</script>".to_owned(),
            voices: 1,
            mine: false,
        }];
        view.playing = Some("<img src=x onerror=alert(1)>");
        view.heading = "\"><script>bad()</script>";
        let html = request_page(&view);

        // What matters is that no tag opens. The words `script` and `onerror`
        // still appear -- as text between `&lt;` and `&gt;`, which is the
        // whole point: the room sees what was typed and nothing runs it.
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(&#39;hi&#39;)&lt;/script&gt;"));
        assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
        // The heading escaped an attribute *and* a tag; both are shut.
        assert!(html.contains("&quot;&gt;&lt;script&gt;"));
    }

    /// **The request survives JavaScript being off.**
    ///
    /// Which is to say there is none: the form posts by itself.
    #[test]
    fn the_page_needs_no_script_at_all() {
        let html = request_page(&a_view());
        assert!(!html.contains("<script"), "the page grew a script");
        assert!(!html.contains("onclick"), "the page grew a handler");
        assert!(html.contains("<form method=\"post\" action=\"/request\">"));
        assert!(html.contains("<button type=\"submit\">"));
    }

    /// **Nothing is fetched from anywhere.**
    ///
    /// The venue wifi may have no route out. One document, or nothing works.
    #[test]
    fn the_page_fetches_nothing() {
        let mut view = a_view();
        view.lines = vec![Line {
            id: 1,
            text: "Suavemente".to_owned(),
            voices: 3,
            mine: false,
        }];
        let html = request_page(&view);
        for fetched in ["http://", "https://", "//cdn", "<link", "<img", "@import"] {
            assert!(!html.contains(fetched), "the page fetches {fetched}");
        }
    }

    /// **Closing requests takes away the form, not just the invitation.**
    #[test]
    fn a_closed_room_has_no_way_to_post() {
        let mut view = a_view();
        view.open = false;
        view.lines = vec![Line {
            id: 1,
            text: "Suavemente".to_owned(),
            voices: 1,
            mine: false,
        }];
        let html = request_page(&view);
        assert!(!html.contains("<form"), "{html}");
        assert!(html.contains(Words::english().closed));
        // The list still shows, so people can see they were heard.
        assert!(html.contains("Suavemente"));
    }

    /// **A row this phone already backed offers no second vote.**
    #[test]
    fn your_own_row_shows_counted_instead_of_a_button() {
        let mut view = a_view();
        view.lines = vec![
            Line {
                id: 1,
                text: "Mine".to_owned(),
                voices: 2,
                mine: true,
            },
            Line {
                id: 2,
                text: "Theirs".to_owned(),
                voices: 1,
                mine: false,
            },
        ];
        let html = request_page(&view);
        assert!(html.contains("value=\"2\""), "the other row has a +1");
        assert!(
            !html.contains("value=\"1\""),
            "your own row has a +1: {html}"
        );
        assert!(html.contains(Words::english().counted));
    }

    /// **A title with an ampersand is shown, not decoded.**
    ///
    /// Left raw, `&` starts a character reference: "Us &times Them" arrives on
    /// every phone in the room as "Us × Them", and the DJ is asked for a song
    /// nobody typed.
    #[test]
    fn an_ampersand_stays_an_ampersand() {
        let mut view = a_view();
        view.lines = vec![Line {
            id: 1,
            text: "Fito & Fitipaldis — Us &times Them".to_owned(),
            voices: 1,
            mine: false,
        }];
        let html = request_page(&view);
        assert!(html.contains("Fito &amp; Fitipaldis"), "{html}");
        assert!(html.contains("Us &amp;times Them"), "{html}");
    }

    #[test]
    fn one_person_is_not_one_people() {
        let english = Words::english();
        assert_eq!(voices(&english, 1), "1 person");
        assert_eq!(voices(&english, 4), "4 people");
        let spanish = Words::spanish();
        assert_eq!(voices(&spanish, 1), "1 persona");
        assert_eq!(voices(&spanish, 4), "4 personas");
    }

    /// **Every language says every sentence.**
    ///
    /// A page half-translated is worse than one not translated: the missing
    /// half is exactly the half that only appears when something goes wrong.
    #[test]
    fn no_language_is_half_finished() {
        for words in Words::all() {
            let empty = |s: &str| s.trim().is_empty();
            assert!(!empty(words.tag) && !empty(words.name), "{}", words.tag);
            for said in [
                Said::Added,
                Said::Seconded,
                Said::Yours,
                Said::Empty,
                Said::TooLong,
                Said::TooMany,
                Said::Full,
                Said::Gone,
            ] {
                assert!(!empty(said.sentence(&words)), "{} {said:?}", words.tag);
            }
            for text in [
                words.title,
                words.playing_now,
                words.field_label,
                words.placeholder,
                words.send,
                words.asked_already,
                words.me_too,
                words.counted,
                words.one_voice,
                words.many_voices,
                words.closed,
            ] {
                assert!(!empty(text), "{}", words.tag);
            }
            assert!(words.many_voices.contains("{n}"), "{}", words.tag);
        }
    }

    #[test]
    fn a_language_is_found_by_tag_and_falls_back() {
        assert_eq!(Words::by_tag("es").tag, "es");
        assert_eq!(Words::by_tag("en").tag, "en");
        assert_eq!(Words::by_tag("kl").tag, "en");
    }

    #[test]
    fn every_outcome_has_a_tag_that_round_trips() {
        for outcome in [
            Outcome::Added(1),
            Outcome::Seconded(1),
            Outcome::AlreadyYours(1),
            Outcome::Empty,
            Outcome::TooLong,
            Outcome::TooManyFromYou,
            Outcome::BookIsFull,
            Outcome::NoSuchAsk,
        ] {
            let said = Said::of(outcome);
            assert_eq!(Said::by_tag(said.tag()), Some(said), "{outcome:?}");
        }
        assert_eq!(Said::by_tag("nonsense"), None);
    }
}

//! K7: the quiz as a phone plays it, at `/quiz`.
//!
//! Its own page rather than a part of the room's: while a question is open
//! it asks the phone to load it again every few seconds, so the next
//! question arrives without a press — and a page that reloads itself must
//! not be the one holding a half-typed request. The room's page links here
//! while a round runs. Like the room's page it has no script: the reload is
//! a `<meta http-equiv="refresh">`, and every answer is a form.

use crate::page::{STYLE, escape};
use crate::quiz::{Answered, LETTERS, LONGEST_NICKNAME, Named};

/// How often a phone playing along loads the page again, in seconds.
pub const RELOAD_SECONDS: u32 = 3;

/// The words of the quiz page, in the room's language.
#[derive(Debug, Clone, Copy)]
pub struct QuizWords {
    pub heading: &'static str,
    /// On the room's page, while a round runs.
    pub invite: &'static str,
    pub play_along: &'static str,
    pub nickname: &'static str,
    pub play: &'static str,
    /// Formatted with `{name}`.
    pub playing_as: &'static str,
    /// Formatted with `{n}`.
    pub points: &'static str,
    /// Formatted with `{n}` and `{of}`.
    pub question: &'static str,
    /// Formatted with `{letter}`.
    pub answered: &'static str,
    pub closed: &'static str,
    pub waiting: &'static str,
    /// Formatted with `{letter}`.
    pub it_was: &'static str,
    pub right: &'static str,
    pub wrong: &'static str,
    pub leaders: &'static str,
    pub off: &'static str,
    pub back: &'static str,
    pub said_named: &'static str,
    pub said_empty: &'static str,
    pub said_long: &'static str,
    pub said_taken: &'static str,
    pub said_answered: &'static str,
    pub said_nameless: &'static str,
    pub said_closed: &'static str,
    pub said_again: &'static str,
}

impl QuizWords {
    #[must_use]
    pub const fn english() -> Self {
        Self {
            heading: "Quiz",
            invite: "The DJ is running a quiz.",
            play_along: "Play along",
            nickname: "Your nickname",
            play: "Play",
            playing_as: "You're playing as {name}.",
            points: "{n} points",
            question: "Question {n} of {of}",
            answered: "You answered {letter}.",
            closed: "Time's up.",
            waiting: "Waiting for the next question.",
            it_was: "It was {letter}:",
            right: "You were right!",
            wrong: "Not this time.",
            leaders: "Leaderboard",
            off: "No quiz right now.",
            back: "Back to requests",
            said_named: "You're in. Answers appear here.",
            said_empty: "Write a nickname first.",
            said_long: "That nickname is too long.",
            said_taken: "Someone plays by that name. Choose another.",
            said_answered: "Got it.",
            said_nameless: "Choose a nickname first.",
            said_closed: "Too late: that question has closed.",
            said_again: "You've answered this one.",
        }
    }

    #[must_use]
    pub const fn spanish() -> Self {
        Self {
            heading: "Quiz",
            invite: "El DJ está haciendo un quiz.",
            play_along: "Juega",
            nickname: "Tu apodo",
            play: "Jugar",
            playing_as: "Juegas como {name}.",
            points: "{n} puntos",
            question: "Pregunta {n} de {of}",
            answered: "Respondiste {letter}.",
            closed: "Se acabó el tiempo.",
            waiting: "Esperando la siguiente pregunta.",
            it_was: "Era la {letter}:",
            right: "¡Acertaste!",
            wrong: "Esta vez no.",
            leaders: "Clasificación",
            off: "Ahora no hay quiz.",
            back: "Volver a las peticiones",
            said_named: "Ya estás dentro. Las respuestas aparecen aquí.",
            said_empty: "Escribe primero un apodo.",
            said_long: "Ese apodo es demasiado largo.",
            said_taken: "Alguien juega con ese nombre. Elige otro.",
            said_answered: "Recibido.",
            said_nameless: "Elige primero un apodo.",
            said_closed: "Demasiado tarde: esa pregunta ya se cerró.",
            said_again: "Ya respondiste esta.",
        }
    }

    /// By the page's language tag; English for any other.
    #[must_use]
    pub fn by_tag(tag: &str) -> Self {
        match tag {
            "es" => Self::spanish(),
            _ => Self::english(),
        }
    }
}

/// What the quiz page says happened, across the redirect after a POST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuizSaid {
    Named(Named),
    Answered(Answered),
}

impl QuizSaid {
    const ALL: [Self; 9] = [
        Self::Named(Named::Named),
        Self::Named(Named::Empty),
        Self::Named(Named::Long),
        Self::Named(Named::Taken),
        Self::Named(Named::Already),
        Self::Answered(Answered::Taken),
        Self::Answered(Answered::Nameless),
        Self::Answered(Answered::Closed),
        Self::Answered(Answered::Again),
    ];

    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Named(Named::Named) => "in",
            Self::Named(Named::Empty) => "noname",
            Self::Named(Named::Long) => "longname",
            Self::Named(Named::Taken) => "taken",
            Self::Named(Named::Already) => "named",
            Self::Answered(Answered::Taken) => "got",
            Self::Answered(Answered::Nameless) => "nameless",
            Self::Answered(Answered::Closed) => "late",
            Self::Answered(Answered::Again) => "again",
        }
    }

    #[must_use]
    pub fn by_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|said| said.tag() == tag)
    }

    fn is_good(self) -> bool {
        matches!(
            self,
            Self::Named(Named::Named | Named::Already) | Self::Answered(Answered::Taken)
        )
    }

    fn sentence(self, w: &QuizWords) -> Option<&'static str> {
        Some(match self {
            Self::Named(Named::Named) => w.said_named,
            Self::Named(Named::Empty) => w.said_empty,
            Self::Named(Named::Long) => w.said_long,
            Self::Named(Named::Taken) => w.said_taken,
            // Nothing went wrong: the page already says who they are.
            Self::Named(Named::Already) => return None,
            Self::Answered(Answered::Taken) => w.said_answered,
            Self::Answered(Answered::Nameless) => w.said_nameless,
            Self::Answered(Answered::Closed) => w.said_closed,
            Self::Answered(Answered::Again) => w.said_again,
        })
    }
}

/// The question as one phone sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneQuestion {
    pub id: u64,
    pub answers: [String; 4],
    pub number: usize,
    pub of: usize,
    /// Answers are still taken.
    pub open: bool,
    /// The letter this phone chose.
    pub chosen: Option<usize>,
    /// The right one, once shown.
    pub right: Option<usize>,
}

/// What the quiz page shows one phone.
#[derive(Debug, Clone)]
pub struct QuizView<'a> {
    pub words: QuizWords,
    /// The page's language tag.
    pub tag: &'a str,
    pub heading: &'a str,
    /// A round is running.
    pub on: bool,
    pub nickname: Option<String>,
    pub points: u32,
    pub question: Option<PhoneQuestion>,
    pub leaders: Vec<(String, u32)>,
    pub said: Option<QuizSaid>,
}

/// The quiz page, as one self-contained document.
#[must_use]
pub fn quiz_page(view: &QuizView) -> String {
    let w = &view.words;
    let mut html = String::with_capacity(4096);
    html.push_str("<!doctype html>\n<html lang=\"");
    html.push_str(view.tag);
    html.push_str("\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">\n");
    html.push_str("<meta name=\"robots\" content=\"noindex\">\n");
    // Reloaded only for a phone that is playing: one still typing its
    // nickname would lose it.
    if view.on && view.nickname.is_some() {
        html.push_str("<meta http-equiv=\"refresh\" content=\"");
        html.push_str(&RELOAD_SECONDS.to_string());
        html.push_str(";url=/quiz\">\n");
    }
    html.push_str("<title>");
    html.push_str(&escape(view.heading));
    html.push_str(" — ");
    html.push_str(w.heading);
    html.push_str("</title>\n<style>\n");
    html.push_str(STYLE);
    html.push_str(QUIZ_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<main class=\"quiz\">\n<h1>");
    html.push_str(&escape(view.heading));
    html.push_str("</h1>\n<h2>");
    html.push_str(w.heading);
    html.push_str("</h2>\n");

    if let Some(sentence) = view.said.and_then(|said| Some((said, said.sentence(w)?))) {
        html.push_str("<p class=\"said ");
        html.push_str(if sentence.0.is_good() { "good" } else { "no" });
        html.push_str("\" role=\"status\">");
        html.push_str(sentence.1);
        html.push_str("</p>\n");
    }

    if !view.on {
        html.push_str("<p class=\"closed\">");
        html.push_str(w.off);
        html.push_str("</p>\n");
    } else if let Some(nickname) = &view.nickname {
        html.push_str("<p class=\"you\">");
        html.push_str(&escape(&w.playing_as.replace("{name}", nickname)));
        html.push_str(" <strong>");
        html.push_str(&w.points.replace("{n}", &view.points.to_string()));
        html.push_str("</strong></p>\n");
        match &view.question {
            Some(question) => question_part(&mut html, question, w),
            None => {
                html.push_str("<p class=\"wait\">");
                html.push_str(w.waiting);
                html.push_str("</p>\n");
            }
        }
    } else {
        html.push_str("<form method=\"post\" action=\"/quiz/name\">\n<label for=\"nickname\">");
        html.push_str(w.nickname);
        html.push_str("</label>\n<input id=\"nickname\" name=\"nickname\" type=\"text\" autocomplete=\"nickname\" maxlength=\"");
        html.push_str(&LONGEST_NICKNAME.to_string());
        html.push_str("\">\n<button type=\"submit\">");
        html.push_str(w.play);
        html.push_str("</button>\n</form>\n");
    }

    if !view.leaders.is_empty() {
        html.push_str("<h2>");
        html.push_str(w.leaders);
        html.push_str("</h2>\n<ol class=\"leaders\">\n");
        for (place, (nickname, points)) in view.leaders.iter().enumerate() {
            // Written out: a flex row has no list marker to number it.
            html.push_str("<li><span class=\"place\">");
            html.push_str(&(place + 1).to_string());
            html.push_str("</span><span class=\"what\">");
            html.push_str(&escape(nickname));
            html.push_str("</span><span class=\"who\">");
            html.push_str(&points.to_string());
            html.push_str("</span></li>\n");
        }
        html.push_str("</ol>\n");
    }

    html.push_str("<p class=\"back\"><a href=\"/\">");
    html.push_str(w.back);
    html.push_str("</a></p>\n<p class=\"mark\">djmanzo</p>\n</main>\n</body>\n</html>\n");
    html
}

/// The open question: four buttons while it is open and unanswered; the
/// letter chosen once answered; the right one once shown.
fn question_part(html: &mut String, question: &PhoneQuestion, w: &QuizWords) {
    html.push_str("<p class=\"number\">");
    html.push_str(
        &w.question
            .replace("{n}", &question.number.to_string())
            .replace("{of}", &question.of.to_string()),
    );
    html.push_str("</p>\n");
    if let Some(right) = question.right {
        html.push_str("<p class=\"it-was\">");
        html.push_str(&w.it_was.replace("{letter}", LETTERS[right]));
        html.push_str(" <strong>");
        html.push_str(&escape(&question.answers[right]));
        html.push_str("</strong></p>\n");
        if question.chosen.is_some() {
            html.push_str("<p class=\"said ");
            html.push_str(if question.chosen == Some(right) {
                "good\">"
            } else {
                "no\">"
            });
            html.push_str(if question.chosen == Some(right) {
                w.right
            } else {
                w.wrong
            });
            html.push_str("</p>\n");
        }
        return;
    }
    if let Some(chosen) = question.chosen {
        html.push_str("<p class=\"chosen\">");
        html.push_str(&w.answered.replace("{letter}", LETTERS[chosen]));
        html.push_str("</p>\n");
        return;
    }
    if !question.open {
        html.push_str("<p class=\"closed\">");
        html.push_str(w.closed);
        html.push_str("</p>\n");
        return;
    }
    html.push_str("<ul class=\"answers\">\n");
    for (letter, answer) in question.answers.iter().enumerate() {
        html.push_str("<li><form method=\"post\" action=\"/quiz/answer\"><input type=\"hidden\" name=\"question\" value=\"");
        html.push_str(&question.id.to_string());
        html.push_str("\"><input type=\"hidden\" name=\"letter\" value=\"");
        html.push_str(LETTERS[letter]);
        html.push_str("\"><button type=\"submit\"><span class=\"letter\">");
        html.push_str(LETTERS[letter]);
        html.push_str("</span>");
        html.push_str(&escape(answer));
        html.push_str("</button></form></li>\n");
    }
    html.push_str("</ul>\n");
}

/// On the room's page while a round runs: where to play.
#[must_use]
pub fn invitation(tag: &str) -> String {
    let w = QuizWords::by_tag(tag);
    let mut html = String::from("<section class=\"quiz-invite\" id=\"quiz\">\n<h2>");
    html.push_str(w.heading);
    html.push_str("</h2>\n<p>");
    html.push_str(w.invite);
    html.push_str("</p>\n<p><a class=\"play\" href=\"/quiz\">");
    html.push_str(w.play_along);
    html.push_str("</a></p>\n</section>\n");
    html
}

/// Four large answers, each its letter first: one thumb, a dark room.
pub(crate) const QUIZ_STYLE: &str = "\
.answers{list-style:none;margin:.75rem 0 0;padding:0}
.answers form{margin:0}
.answers button{display:flex;align-items:center;gap:.8rem;text-align:left;min-height:3.8rem;
 padding:.6rem .9rem;font-size:1.1rem;background:#1e2833;color:#e9edf2}
.answers .letter{font-size:1.4rem;font-weight:700;color:#4f9dff;min-width:1.4rem}
.number{color:#8b97a6;margin:.5rem 0 .25rem}
.chosen,.wait{font-size:1.15rem;color:#cfd8e3}
.it-was{font-size:1.15rem}
.it-was strong{display:block;color:#9fe6b4;margin-top:.2rem}
.leaders{list-style:none;margin:0;padding:0}
.leaders .place{min-width:1.4rem;color:#8b97a6}
.leaders li{display:flex;gap:.6rem;padding:.35rem 0;border-top:1px solid #1c222a}
.back{margin-top:2rem}
a{color:#4f9dff}
.quiz-invite{margin-top:2.5rem;padding-top:.5rem;border-top:1px solid #1c222a}
.play{display:block;text-align:center;padding:.9rem;border-radius:.6rem;background:#4f9dff;
 color:#06121f;font-weight:600;text-decoration:none}
";

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> QuizView<'static> {
        QuizView {
            words: QuizWords::english(),
            tag: "en",
            heading: "Sábado",
            on: true,
            nickname: Some("Ana".to_owned()),
            points: 812,
            question: Some(PhoneQuestion {
                id: 7,
                answers: ["Obsesión — Aventura", "B", "C", "<script>x()</script>"]
                    .map(str::to_owned),
                number: 3,
                of: 10,
                open: true,
                chosen: None,
                right: None,
            }),
            leaders: vec![("<b>Bo</b>".to_owned(), 900)],
            said: None,
        }
    }

    /// **Four buttons for the open question, each carrying the question it
    /// answers, everything the room typed escaped, and no script.**
    #[test]
    fn the_open_question_is_four_buttons() {
        let html = quiz_page(&view());
        assert_eq!(html.matches("action=\"/quiz/answer\"").count(), 4);
        assert_eq!(html.matches("name=\"question\" value=\"7\"").count(), 4);
        assert!(
            html.contains("<span class=\"letter\">D</span>&lt;script&gt;"),
            "{html}"
        );
        assert!(
            html.contains(
                "<span class=\"place\">1</span><span class=\"what\">&lt;b&gt;Bo&lt;/b&gt;"
            ),
            "{html}"
        );
        assert!(!html.contains("<script"));
        assert!(html.contains("Question 3 of 10") && html.contains("812 points"));
        assert!(
            html.contains("http-equiv=\"refresh\""),
            "a player's page reloads"
        );
    }

    /// **Answered, the buttons go; shown, the answer is said and whether
    /// this phone had it.**
    #[test]
    fn answered_then_shown() {
        let mut v = view();
        v.question.as_mut().unwrap().chosen = Some(1);
        let answered = quiz_page(&v);
        assert!(!answered.contains("/quiz/answer"));
        assert!(answered.contains("You answered B."));
        v.question.as_mut().unwrap().right = Some(0);
        let shown = quiz_page(&v);
        assert!(
            shown.contains("It was A: <strong>Obsesión — Aventura</strong>"),
            "{shown}"
        );
        assert!(shown.contains("Not this time."));
    }

    /// **A phone without a nickname is asked for one, and its page does not
    /// reload under its typing.**
    #[test]
    fn a_nickname_first_and_no_reload_while_typing() {
        let mut v = view();
        v.nickname = None;
        let html = quiz_page(&v);
        assert!(html.contains("action=\"/quiz/name\""));
        assert!(!html.contains("/quiz/answer"));
        assert!(!html.contains("http-equiv=\"refresh\""));
    }

    /// **Every said tag reads back as itself.**
    #[test]
    fn said_tags_round_trip() {
        for said in QuizSaid::ALL {
            assert_eq!(QuizSaid::by_tag(said.tag()), Some(said));
        }
    }
}

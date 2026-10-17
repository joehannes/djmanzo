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
use crate::stage::{KEY_RANGE, LONGEST_NAME, LONGEST_SONG, Match, Place, Signed, Voice};

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
    /// K4: the *Sing* part, when the host has sign-ups open or this phone
    /// has a singer down tonight.
    pub sing: Option<SingView>,
    /// K7: a quiz round is running, and the page says where to play.
    pub quiz: bool,
    /// Karaoke beta B2: the row of emoji, while the host has them on.
    pub react: Option<crate::reactions::Row>,
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
    if view.react.is_some() {
        html.push_str(crate::reactions::ROW_STYLE);
    }
    if view.quiz {
        // K7: the invitation's own look, only on a page that carries it.
        html.push_str(crate::quiz_page::QUIZ_STYLE);
    }
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

    if let Some(row) = view.react {
        html.push_str(&crate::reactions::row_html(w.tag, row));
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

    if let Some(sing) = &view.sing {
        sing_part(&mut html, sing, &SingWords::by_tag(w.tag));
    }

    if view.quiz {
        html.push_str(&crate::quiz_page::invitation(w.tag));
    }

    html.push_str("<p class=\"mark\">djmanzo</p>\n</main>\n</body>\n</html>\n");
    html
}

/// K4: the words of the *Sing* part, in the page's language.
#[derive(Debug, Clone, Copy)]
pub struct SingWords {
    pub heading: &'static str,
    pub name: &'static str,
    pub find_label: &'static str,
    pub find_placeholder: &'static str,
    pub find: &'static str,
    pub sing_this: &'static str,
    pub none: &'static str,
    pub written_label: &'static str,
    pub send: &'static str,
    /// Formatted with the singer's name, as `{name}`.
    pub you: &'static str,
    pub next: &'static str,
    pub one_before: &'static str,
    /// Formatted with the count, as `{n}`.
    pub before: &'static str,
    pub coming: &'static str,
    pub sung: &'static str,
    pub ready: &'static str,
    pub ready_said: &'static str,
    pub another: &'static str,
    pub added: &'static str,
    pub closed: &'static str,
    pub no_name: &'static str,
    pub long_name: &'static str,
    pub no_song: &'static str,
    pub long_song: &'static str,
    pub gone: &'static str,
    pub other_name: &'static str,
    pub name_taken: &'static str,
    pub too_many: &'static str,
    pub full: &'static str,
    pub key: &'static str,
    pub key_as_recorded: &'static str,
    pub key_lower_one: &'static str,
    /// Formatted with the count, as `{n}`.
    pub key_lower: &'static str,
    pub key_higher_one: &'static str,
    /// Formatted with the count, as `{n}`.
    pub key_higher: &'static str,
    pub voice: &'static str,
    pub voice_as_recorded: &'static str,
    pub voice_guide: &'static str,
    pub voice_out: &'static str,
    /// Under the choices: they are asked for, not set.
    pub proposals: &'static str,
    pub photo_heading: &'static str,
    pub photo_label: &'static str,
    /// Before the button: what happens to it, said before it is sent.
    pub photo_note: &'static str,
    pub photo_send: &'static str,
    pub photo_waiting: &'static str,
    pub photo_shown: &'static str,
    pub photo_sent: &'static str,
    pub photo_not: &'static str,
    pub photo_big: &'static str,
    pub photo_shut: &'static str,
}

impl SingWords {
    #[must_use]
    pub const fn english() -> Self {
        Self {
            heading: "Sing",
            name: "Your name",
            find_label: "Find your song",
            find_placeholder: "Artist or song",
            find: "Find it",
            sing_this: "Sing this",
            none: "Not in the songbook. Try the artist's name.",
            written_label: "Which song?",
            send: "Put me down",
            you: "You're down as {name}.",
            next: "You're up next.",
            one_before: "1 singer before you.",
            before: "{n} singers before you.",
            coming: "The DJ is adding you.",
            sung: "Thanks for singing!",
            ready: "I'm ready",
            ready_said: "The DJ knows you're ready.",
            another: "Another song",
            added: "You're on the list.",
            closed: "Sign-ups to sing are closed.",
            no_name: "Write your name first.",
            long_name: "That name is too long.",
            no_song: "Choose a song.",
            long_song: "That song name is too long.",
            gone: "The songbook changed. Search again.",
            other_name: "This phone already has a singer.",
            name_taken: "Someone is down by that name. Add your surname.",
            too_many: "You have as many songs waiting as the DJ allows.",
            full: "The list is full tonight.",
            key: "Key",
            key_as_recorded: "As recorded",
            key_lower_one: "1 semitone lower",
            key_lower: "{n} semitones lower",
            key_higher_one: "1 semitone higher",
            key_higher: "{n} semitones higher",
            voice: "The record's own singer",
            voice_as_recorded: "As recorded",
            voice_guide: "Quietly, as a guide",
            voice_out: "Taken out",
            proposals: "The DJ sees what you ask for and decides.",
            photo_heading: "A photo of you",
            photo_label: "Choose a photo",
            photo_note: "Shown on the screen in this room tonight, while you sing, if the DJ approves it. Not kept after tonight.",
            photo_send: "Send the photo",
            photo_waiting: "Your photo is waiting for the DJ.",
            photo_shown: "The DJ approved your photo.",
            photo_sent: "Photo received. The DJ will look at it.",
            photo_not: "That is not a photo we can show.",
            photo_big: "That photo is too large.",
            photo_shut: "Photos are not being taken now.",
        }
    }

    #[must_use]
    pub const fn spanish() -> Self {
        Self {
            heading: "Canta",
            name: "Tu nombre",
            find_label: "Busca tu canción",
            find_placeholder: "Artista o canción",
            find: "Buscar",
            sing_this: "Cantar esta",
            none: "No está en el cancionero. Prueba con el nombre del artista.",
            written_label: "¿Qué canción?",
            send: "Apúntame",
            you: "Te apuntaste como {name}.",
            next: "Te toca a continuación.",
            one_before: "1 persona antes que tú.",
            before: "{n} personas antes que tú.",
            coming: "El DJ te está añadiendo.",
            sung: "¡Gracias por cantar!",
            ready: "Ya voy",
            ready_said: "El DJ ya sabe que vas.",
            another: "Otra canción",
            added: "Ya estás en la lista.",
            closed: "La lista para cantar está cerrada.",
            no_name: "Escribe primero tu nombre.",
            long_name: "Ese nombre es demasiado largo.",
            no_song: "Elige una canción.",
            long_song: "El nombre de la canción es demasiado largo.",
            gone: "El cancionero cambió. Busca de nuevo.",
            other_name: "Este teléfono ya tiene cantante.",
            name_taken: "Alguien ya está con ese nombre. Añade tu apellido.",
            too_many: "Ya tienes todas las canciones que el DJ permite.",
            full: "La lista está llena esta noche.",
            key: "Tono",
            key_as_recorded: "Como está grabada",
            key_lower_one: "1 semitono más bajo",
            key_lower: "{n} semitonos más bajo",
            key_higher_one: "1 semitono más alto",
            key_higher: "{n} semitonos más alto",
            voice: "La voz de la grabación",
            voice_as_recorded: "Como está grabada",
            voice_guide: "Suave, como guía",
            voice_out: "Quitada",
            proposals: "El DJ ve lo que pides y decide.",
            photo_heading: "Una foto tuya",
            photo_label: "Elige una foto",
            photo_note: "Se muestra en la pantalla de este local esta noche, mientras cantas, si el DJ la aprueba. No se guarda después de esta noche.",
            photo_send: "Enviar la foto",
            photo_waiting: "Tu foto espera al DJ.",
            photo_shown: "El DJ aprobó tu foto.",
            photo_sent: "Foto recibida. El DJ la verá.",
            photo_not: "Eso no es una foto que podamos mostrar.",
            photo_big: "Esa foto es demasiado grande.",
            photo_shut: "Ahora no se aceptan fotos.",
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

/// What the *Sing* part says happened, across the redirect after a POST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sang {
    Signed(Signed),
    Ready,
    Photo(PhotoSaid),
}

/// K4: what became of a photo, as the page says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoSaid {
    Sent,
    NotAPicture,
    TooLarge,
    Closed,
}

impl Sang {
    const ALL: [Self; 16] = [
        Self::Signed(Signed::Added),
        Self::Signed(Signed::Closed),
        Self::Signed(Signed::NoName),
        Self::Signed(Signed::LongName),
        Self::Signed(Signed::NoSong),
        Self::Signed(Signed::LongSong),
        Self::Signed(Signed::Gone),
        Self::Signed(Signed::OtherName),
        Self::Signed(Signed::NameTaken),
        Self::Signed(Signed::TooMany),
        Self::Signed(Signed::Full),
        Self::Ready,
        Self::Photo(PhotoSaid::Sent),
        Self::Photo(PhotoSaid::NotAPicture),
        Self::Photo(PhotoSaid::TooLarge),
        Self::Photo(PhotoSaid::Closed),
    ];

    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Signed(Signed::Added) => "on",
            Self::Signed(Signed::Closed) => "shut",
            Self::Signed(Signed::NoName) => "noname",
            Self::Signed(Signed::LongName) => "longname",
            Self::Signed(Signed::NoSong) => "nosong",
            Self::Signed(Signed::LongSong) => "longsong",
            Self::Signed(Signed::Gone) => "changed",
            Self::Signed(Signed::OtherName) => "phone",
            Self::Signed(Signed::NameTaken) => "taken",
            Self::Signed(Signed::TooMany) => "enough",
            Self::Signed(Signed::Full) => "full",
            Self::Ready => "ready",
            Self::Photo(PhotoSaid::Sent) => "photo",
            Self::Photo(PhotoSaid::NotAPicture) => "notphoto",
            Self::Photo(PhotoSaid::TooLarge) => "bigphoto",
            Self::Photo(PhotoSaid::Closed) => "nophotos",
        }
    }

    #[must_use]
    pub fn by_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.tag() == tag)
    }

    fn is_good(self) -> bool {
        matches!(
            self,
            Self::Signed(Signed::Added) | Self::Ready | Self::Photo(PhotoSaid::Sent)
        )
    }

    fn sentence(self, w: &SingWords) -> &'static str {
        match self {
            Self::Signed(Signed::Added) => w.added,
            Self::Signed(Signed::Closed) => w.closed,
            Self::Signed(Signed::NoName) => w.no_name,
            Self::Signed(Signed::LongName) => w.long_name,
            Self::Signed(Signed::NoSong) => w.no_song,
            Self::Signed(Signed::LongSong) => w.long_song,
            Self::Signed(Signed::Gone) => w.gone,
            Self::Signed(Signed::OtherName) => w.other_name,
            Self::Signed(Signed::NameTaken) => w.name_taken,
            Self::Signed(Signed::TooMany) => w.too_many,
            Self::Signed(Signed::Full) => w.full,
            Self::Ready => w.ready_said,
            Self::Photo(PhotoSaid::Sent) => w.photo_sent,
            Self::Photo(PhotoSaid::NotAPicture) => w.photo_not,
            Self::Photo(PhotoSaid::TooLarge) => w.photo_big,
            Self::Photo(PhotoSaid::Closed) => w.photo_shut,
        }
    }
}

/// K4: what the *Sing* part shows one phone.
#[derive(Debug, Clone, Default)]
pub struct SingView {
    /// Whether the host is taking names now.
    pub open: bool,
    /// The singer this phone put down tonight.
    pub name: Option<String>,
    pub place: Option<Place>,
    /// Whether this singer may put down another song now.
    pub more: bool,
    /// Songs come from the songbook; otherwise they are written.
    pub songbook: bool,
    /// The name typed with a search, carried to the results.
    pub typed_name: String,
    /// What was searched for, when something was.
    pub query: Option<String>,
    pub matches: Vec<Match>,
    pub said: Option<Sang>,
    /// Whether a key may be proposed, and the one chosen with a search.
    pub key: bool,
    pub chosen_key: Option<i32>,
    /// Whether the record's singer may be, and what was chosen.
    pub guide: bool,
    pub chosen_voice: Option<Voice>,
    /// Whether this phone's singer may send a photo.
    pub photos: bool,
    /// This phone's singer's photo: waiting (false) or approved (true).
    pub photo: Option<bool>,
}

fn sing_part(html: &mut String, sing: &SingView, w: &SingWords) {
    html.push_str("<section class=\"sing\" id=\"sing\">\n<h2>");
    html.push_str(w.heading);
    html.push_str("</h2>\n");
    if let Some(said) = sing.said {
        html.push_str("<p class=\"said ");
        html.push_str(if said.is_good() { "good" } else { "no" });
        html.push_str("\" role=\"status\">");
        html.push_str(said.sentence(w));
        html.push_str("</p>\n");
    }
    if let Some(name) = &sing.name {
        html.push_str("<p class=\"you\">");
        html.push_str(&escape(&w.you.replace("{name}", name)));
        html.push_str(" <strong>");
        html.push_str(&match sing.place {
            Some(Place::Waiting(0)) => w.next.to_owned(),
            Some(Place::Waiting(1)) => w.one_before.to_owned(),
            Some(Place::Waiting(n)) => w.before.replace("{n}", &n.to_string()),
            Some(Place::Coming) => w.coming.to_owned(),
            Some(Place::Sung) | None => w.sung.to_owned(),
        });
        html.push_str("</strong></p>\n");
        if sing.place == Some(Place::Waiting(0)) && sing.said != Some(Sang::Ready) {
            html.push_str(
                "<form method=\"post\" action=\"/ready\"><button type=\"submit\" class=\"ready\">",
            );
            html.push_str(w.ready);
            html.push_str("</button></form>\n");
        }
        if sing.photos {
            photo_part(html, sing, w);
        }
    }
    let may_choose = sing.open && (sing.name.is_none() || sing.more);
    if !may_choose {
        if !sing.open && sing.name.is_none() {
            html.push_str("<p class=\"closed\">");
            html.push_str(w.closed);
            html.push_str("</p>\n");
        }
        html.push_str("</section>\n");
        return;
    }
    let name = sing.name.as_deref().unwrap_or(&sing.typed_name);
    if sing.name.is_some() {
        html.push_str("<h3>");
        html.push_str(w.another);
        html.push_str("</h3>\n");
    }
    if sing.songbook {
        html.push_str("<form method=\"get\" action=\"/#sing\">\n");
        name_field(html, sing, w, name);
        html.push_str("<label for=\"q\">");
        html.push_str(w.find_label);
        html.push_str("</label>\n<input id=\"q\" name=\"q\" type=\"search\" autocomplete=\"off\" enterkeyhint=\"search\" maxlength=\"80\" placeholder=\"");
        html.push_str(w.find_placeholder);
        html.push_str("\" value=\"");
        html.push_str(&escape(sing.query.as_deref().unwrap_or_default()));
        html.push_str("\">\n");
        proposal_fields(html, sing, w);
        html.push_str("<button type=\"submit\">");
        html.push_str(w.find);
        html.push_str("</button>\n</form>\n");
        if sing.query.is_some() {
            if sing.matches.is_empty() {
                html.push_str("<p class=\"none\">");
                html.push_str(w.none);
                html.push_str("</p>\n");
            } else {
                html.push_str("<ul class=\"songs\">\n");
                for found in &sing.matches {
                    html.push_str("<li><span class=\"what\">");
                    html.push_str(&escape(&found.title));
                    if !found.artist.is_empty() {
                        html.push_str("<span class=\"by\">");
                        html.push_str(&escape(&found.artist));
                        html.push_str("</span>");
                    }
                    html.push_str("</span><form method=\"post\" action=\"/sing\"><input type=\"hidden\" name=\"name\" value=\"");
                    html.push_str(&escape(name));
                    html.push_str("\"><input type=\"hidden\" name=\"song\" value=\"");
                    html.push_str(&escape(&found.token));
                    html.push_str("\">");
                    if let Some(key) = sing.chosen_key.filter(|_| sing.key) {
                        html.push_str("<input type=\"hidden\" name=\"key\" value=\"");
                        html.push_str(&key.to_string());
                        html.push_str("\">");
                    }
                    if let Some(voice) = sing.chosen_voice.filter(|_| sing.guide) {
                        html.push_str("<input type=\"hidden\" name=\"voice\" value=\"");
                        html.push_str(voice.tag());
                        html.push_str("\">");
                    }
                    html.push_str("<button type=\"submit\" class=\"plus\">");
                    html.push_str(w.sing_this);
                    html.push_str("</button></form></li>\n");
                }
                html.push_str("</ul>\n");
            }
        }
    } else {
        html.push_str("<form method=\"post\" action=\"/sing\">\n");
        name_field(html, sing, w, name);
        html.push_str("<label for=\"written\">");
        html.push_str(w.written_label);
        html.push_str("</label>\n<input id=\"written\" name=\"written\" type=\"text\" autocomplete=\"off\" maxlength=\"");
        html.push_str(&LONGEST_SONG.to_string());
        html.push_str("\">\n");
        proposal_fields(html, sing, w);
        html.push_str("<button type=\"submit\">");
        html.push_str(w.send);
        html.push_str("</button>\n</form>\n");
    }
    html.push_str("</section>\n");
}

/// K4: a photo of the singer — what happens to it said before the button.
fn photo_part(html: &mut String, sing: &SingView, w: &SingWords) {
    html.push_str("<h3>");
    html.push_str(w.photo_heading);
    html.push_str("</h3>\n");
    match sing.photo {
        Some(true) => {
            html.push_str("<p class=\"photo-state\">");
            html.push_str(w.photo_shown);
            html.push_str("</p>\n");
        }
        Some(false) => {
            html.push_str("<p class=\"photo-state\">");
            html.push_str(w.photo_waiting);
            html.push_str("</p>\n");
        }
        None => {}
    }
    html.push_str("<form method=\"post\" action=\"/photo\" enctype=\"multipart/form-data\">\n<label for=\"photo\">");
    html.push_str(w.photo_label);
    html.push_str("</label>\n<input id=\"photo\" name=\"photo\" type=\"file\" accept=\"image/jpeg,image/png\" required>\n<p class=\"asks\">");
    html.push_str(w.photo_note);
    html.push_str("</p>\n<button type=\"submit\">");
    html.push_str(w.photo_send);
    html.push_str("</button>\n</form>\n");
}

/// K4: the key and the record's singer, where the host lets a phone ask.
fn proposal_fields(html: &mut String, sing: &SingView, w: &SingWords) {
    if sing.key {
        html.push_str("<label for=\"key\">");
        html.push_str(w.key);
        html.push_str("</label>\n<select id=\"key\" name=\"key\">\n");
        for key in -KEY_RANGE..=KEY_RANGE {
            let words = match key {
                0 => w.key_as_recorded.to_owned(),
                -1 => w.key_lower_one.to_owned(),
                1 => w.key_higher_one.to_owned(),
                k if k < 0 => w.key_lower.replace("{n}", &(-k).to_string()),
                k => w.key_higher.replace("{n}", &k.to_string()),
            };
            html.push_str("<option value=\"");
            html.push_str(&key.to_string());
            html.push('"');
            if sing.chosen_key.unwrap_or(0) == key {
                html.push_str(" selected");
            }
            html.push('>');
            html.push_str(&words);
            html.push_str("</option>\n");
        }
        html.push_str("</select>\n");
    }
    if sing.guide {
        html.push_str("<label for=\"voice\">");
        html.push_str(w.voice);
        html.push_str("</label>\n<select id=\"voice\" name=\"voice\">\n");
        for voice in Voice::ALL {
            html.push_str("<option value=\"");
            html.push_str(voice.tag());
            html.push('"');
            if sing.chosen_voice.unwrap_or(Voice::AsRecorded) == voice {
                html.push_str(" selected");
            }
            html.push('>');
            html.push_str(match voice {
                Voice::AsRecorded => w.voice_as_recorded,
                Voice::Guide => w.voice_guide,
                Voice::Out => w.voice_out,
            });
            html.push_str("</option>\n");
        }
        html.push_str("</select>\n");
    }
    if sing.key || sing.guide {
        html.push_str("<p class=\"asks\">");
        html.push_str(w.proposals);
        html.push_str("</p>\n");
    }
}

/// The name field, or the name itself once this phone has put one down.
fn name_field(html: &mut String, sing: &SingView, w: &SingWords, name: &str) {
    if sing.name.is_some() {
        html.push_str("<input type=\"hidden\" name=\"name\" value=\"");
        html.push_str(&escape(name));
        html.push_str("\">\n");
        return;
    }
    html.push_str("<label for=\"name\">");
    html.push_str(w.name);
    html.push_str("</label>\n<input id=\"name\" name=\"name\" type=\"text\" autocomplete=\"nickname\" maxlength=\"");
    html.push_str(&LONGEST_NAME.to_string());
    html.push_str("\" value=\"");
    html.push_str(&escape(name));
    html.push_str("\">\n");
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
pub(crate) fn escape(text: &str) -> String {
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
pub(crate) const STYLE: &str = "\
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
.sing{margin-top:2.5rem;padding-top:.5rem;border-top:1px solid #1c222a}
.sing h3{font-size:1rem;margin:1.25rem 0 .5rem;color:#cfd8e3}
.sing label{margin-top:.8rem}
.you{margin:0 0 .75rem;font-size:1.05rem}
.you strong{display:block;font-size:1.25rem;margin-top:.2rem;color:#9fe6b4}
.ready{min-height:3.6rem;font-size:1.25rem;background:#3fbf6f;color:#06150c}
.none{color:#b9c4d0}
.songs{list-style:none;margin:.75rem 0 0;padding:0}
.songs li{display:flex;align-items:center;gap:.6rem;padding:.6rem 0;border-top:1px solid #1c222a}
.songs form{margin:0}
.by{display:block;color:#8b97a6;font-size:.85rem}
select{width:100%;padding:.75rem .8rem;border-radius:.6rem;border:1px solid #2b333d;
 background:#141920;color:#e9edf2;font-size:1.05rem}
.sing p.asks{margin:.5rem 0 0;color:#8b97a6;font-size:.9rem}
.photo-state{margin:0 0 .5rem;color:#9fe6b4}
input[type=file]{padding:.6rem;font-size:.95rem}
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
            sing: None,
            quiz: false,
            react: None,
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

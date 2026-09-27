//! §123: the words of a song written for a karaoke guest.
//!
//! > auto prefill the lyrics with individual lyrics mentioning the venue,
//! > event, evening, name of the artist, that she sang this karaoke song
//! > tonight ... let the DJ be able to add a few keywords/ideas of his own ...
//! > as of the styles, let it be a mix of her favorite song, favorite band,
//! > favorite genre and the song sung ... if the evening also is about some
//! > music genre, that has influences as well ... the lyrics shall be created
//! > in the language of the karaoke song (+ if differing, a second version in
//! > the native language of the karaoke artist)
//!
//! What goes to the model is a [`SongBrief`]: the guest's name as the host
//! wrote it, the song, their favourites and languages, and where and when —
//! **never** their email, phone, age, home or nationality, which a song does
//! not need. What comes back is read into a [`SongDraft`]: a style line for
//! the music service and one set of words per language, each checked here
//! rather than trusted.
//!
//! Two things are refused in what the model says, whatever it says:
//!
//! - **Names in the style.** The style is a description of a sound —
//!   genres, moods, instruments. The guest's favourite band and the artist of
//!   the song they sang are taken out of it: the guest's song is to be their
//!   own, not a copy of somebody else's.
//! - **Anything past a length** a music service's boxes take: [`STYLE_MOST`]
//!   and [`LYRICS_MOST`], cut at a comma or a line rather than mid-word.

use serde::{Deserialize, Serialize};

/// The longest style line kept, in characters.
pub const STYLE_MOST: usize = 200;

/// The longest set of words kept per language, in characters.
pub const LYRICS_MOST: usize = 3000;

/// What the model is told about the guest and the night.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SongBrief {
    /// As the host wrote it down.
    pub singer: String,
    /// The song they sang tonight, as the rotation had it.
    pub sang: String,
    /// When, in words: "Saturday 26 September 2026".
    pub date: String,
    pub event: String,
    pub place: String,
    /// What tonight is about, musically: the event's genres.
    pub evening: Vec<String>,
    pub favourite_band: String,
    pub favourite_genre: String,
    pub favourite_song: String,
    /// The DJ's own ideas, as typed.
    pub keywords: String,
    /// One set of words per language, in this order. See [`languages`].
    pub languages: Vec<String>,
}

/// One set of words, in one language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Version {
    pub language: String,
    pub lyrics: String,
}

/// What came back, checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SongDraft {
    pub style: String,
    pub versions: Vec<Version>,
}

/// The languages to write in: the song's, then the guest's own if it
/// differs. A song whose language nobody has said is written "in the language
/// it is sung in", which a model knows for any well-known song.
#[must_use]
pub fn languages(song: &str, sang: &str, native: &str) -> Vec<String> {
    let song = song.trim();
    let first = if song.is_empty() {
        format!("the language \"{}\" is sung in", sang.trim())
    } else {
        language_name(song)
    };
    let mut out = vec![first];
    let native = native.trim();
    if !native.is_empty() && !same_language(song, native) {
        out.push(language_name(native));
    }
    out
}

/// A two-letter code as a name a model and a DJ both read; anything else as
/// it was written.
#[must_use]
pub fn language_name(language: &str) -> String {
    let language = language.trim();
    NAMES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(language))
        .map_or_else(|| language.to_owned(), |(_, name)| (*name).to_owned())
}

/// Whether two ways of writing a language are the same one: "es",
/// "Spanish" and "spanish" are.
fn same_language(a: &str, b: &str) -> bool {
    let a = language_name(a);
    let b = language_name(b);
    !a.is_empty() && a.eq_ignore_ascii_case(&b)
}

/// The codes Whisper answers with, for the languages a DJ is most likely to
/// meet.
const NAMES: [(&str, &str); 24] = [
    ("en", "English"),
    ("es", "Spanish"),
    ("de", "German"),
    ("fr", "French"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("tr", "Turkish"),
    ("el", "Greek"),
    ("ar", "Arabic"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh", "Chinese"),
    ("vi", "Vietnamese"),
    ("id", "Indonesian"),
    ("tl", "Tagalog"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("no", "Norwegian"),
];

/// What the model is told to do.
#[must_use]
pub fn song_prompt() -> String {
    format!(
        "You write the words of a short, personal song for a guest who sang karaoke \
         tonight. The song will be made with an AI music service.\n\
         \n\
         The words celebrate the guest by name, that they sang the song named in the \
         brief tonight, and the event, place and date; work in the DJ's ideas where \
         there are any. Warm, specific, singable; nothing unkind, nothing \
         suggestive.\n\
         Write only original words: never quote or closely paraphrase a line of the \
         song they sang or of any other existing song.\n\
         Put section tags on lines of their own: [Verse], [Chorus], [Verse], \
         [Chorus], [Bridge], [Chorus]. Keep each version under 1800 characters.\n\
         Write one version for each language listed, in that order. Each is written \
         to be sung in its language, not translated word for word, and says the same \
         things.\n\
         Then describe the music in one STYLE line: comma-separated genres, moods, \
         instruments, tempo and voice, blending the guest's favourite genre, the feel \
         of their favourite song and band, the song they sang, and tonight's genres. \
         No names of artists, bands or songs in it. At most {STYLE_MOST} characters.\n\
         \n\
         Answer in exactly this shape and nothing else — no preamble, no notes, no \
         markdown:\n\
         STYLE: <the style line>\n\
         === LYRICS: <language> ===\n\
         <the words>\n\
         === LYRICS: <language> ===\n\
         <the words>"
    )
}

/// The brief, as the question: a line for everything known, nothing for what
/// is not.
#[must_use]
pub fn song_request(brief: &SongBrief) -> String {
    let mut out = String::new();
    let mut line = |label: &str, value: &str| {
        let value = value.trim();
        if !value.is_empty() {
            out.push_str(label);
            out.push_str(": ");
            out.push_str(value);
            out.push('\n');
        }
    };
    line("Guest", &brief.singer);
    line("Sang tonight", &brief.sang);
    line("Event", &brief.event);
    line("Place", &brief.place);
    line("Date", &brief.date);
    line("Tonight's music", &brief.evening.join(", "));
    line("Favourite genre", &brief.favourite_genre);
    line("Favourite band", &brief.favourite_band);
    line("Favourite song", &brief.favourite_song);
    line("The DJ's ideas", &brief.keywords);
    line("Languages, in order", &brief.languages.join("; "));
    out
}

/// Read the answer, forgiving about decoration and strict about shape: a
/// version with no words is dropped, and so is anything outside a section.
#[must_use]
pub fn parse_song(text: &str) -> SongDraft {
    let mut style = String::new();
    let mut versions: Vec<Version> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim().trim_matches('*').trim();
        if line.starts_with("```") {
            continue;
        }
        if let Some(language) = header(line) {
            versions.push(Version {
                language,
                lyrics: String::new(),
            });
            continue;
        }
        if style.is_empty() && versions.is_empty() {
            if let Some(rest) = strip_label(line, "STYLE") {
                style = rest.trim_matches('*').trim().to_owned();
            }
            continue;
        }
        if let Some(current) = versions.last_mut() {
            current.lyrics.push_str(raw.trim_end());
            current.lyrics.push('\n');
        }
    }
    let versions = versions
        .into_iter()
        .map(|version| Version {
            language: version.language,
            lyrics: cut_lines(version.lyrics.trim(), LYRICS_MOST),
        })
        .filter(|version| !version.lyrics.is_empty())
        .collect();
    SongDraft { style, versions }
}

/// `=== LYRICS: Spanish ===`, however many `=` and whatever the case.
fn header(line: &str) -> Option<String> {
    if !line.starts_with('=') {
        return None;
    }
    let inner = line.trim_matches('=').trim();
    let rest = strip_label(inner, "LYRICS")?;
    let language = rest.trim_matches('=').trim();
    Some(language.to_owned())
}

/// `LABEL:` at the start, in any case; what follows it.
fn strip_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let head = line.get(..label.len())?;
    if !head.eq_ignore_ascii_case(label) {
        return None;
    }
    let rest = line[label.len()..].trim_start();
    Some(rest.strip_prefix(':').unwrap_or(rest).trim())
}

/// `text` cut to at most `most` characters, at the end of a line.
fn cut_lines(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut out = String::new();
    for line in text.lines() {
        if out.chars().count() + line.chars().count() + 1 > most {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_owned()
}

/// The style with every part naming one of `names` taken out, repeats
/// dropped, and cut at a comma to [`STYLE_MOST`].
#[must_use]
pub fn clean_style(style: &str, names: &[&str]) -> String {
    let names: Vec<String> = names
        .iter()
        .map(|name| name.trim().to_lowercase())
        .filter(|name| name.chars().count() >= 2)
        .collect();
    let mut kept: Vec<&str> = Vec::new();
    for part in style.split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        let lower = part.to_lowercase();
        if names.iter().any(|name| lower.contains(name.as_str())) {
            continue;
        }
        // A comparison names somebody, whether or not the record knows who:
        // "in the style of …", "like …". The names above are only the ones
        // the guest's record holds; a model can reach for any other.
        if COMPARISONS.iter().any(|lead| lower.starts_with(lead)) {
            continue;
        }
        if kept.iter().any(|seen| seen.eq_ignore_ascii_case(part)) {
            continue;
        }
        let length: usize = kept.iter().map(|k| k.chars().count() + 2).sum();
        if length + part.chars().count() > STYLE_MOST {
            break;
        }
        kept.push(part);
    }
    kept.join(", ")
}

/// How a style part that compares the song to somebody else's begins.
const COMPARISONS: [&str; 9] = [
    "in the style of",
    "style of",
    "like ",
    "sounds like",
    "inspired by",
    "influenced by",
    "reminiscent of",
    "similar to",
    "à la ",
];

/// The names a style must not carry for this brief: the guest's favourite
/// band and song, and the song they sang and its artist.
#[must_use]
pub fn names_in(brief: &SongBrief) -> Vec<String> {
    let mut names = Vec::new();
    for field in [&brief.favourite_band, &brief.favourite_song, &brief.sang] {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        names.push(field.to_owned());
        // "Artist - Title" as the rotation often writes it: each half too.
        for separator in [" - ", " – ", " — ", " by "] {
            for part in field.split(separator) {
                let part = part.trim();
                if !part.is_empty() && part != field {
                    names.push(part.to_owned());
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief() -> SongBrief {
        SongBrief {
            singer: "Aiko".into(),
            sang: "Juan Luis Guerra - Bachata en Fukuoka".into(),
            date: "Saturday 26 September 2026".into(),
            event: "Noche Latina".into(),
            place: "Bar Sol, Fukuoka".into(),
            evening: vec!["latin".into()],
            favourite_band: "Aventura".into(),
            favourite_genre: "bachata".into(),
            favourite_song: "Obsesión".into(),
            keywords: "first time on stage, brave".into(),
            languages: languages("es", "Bachata en Fukuoka", "Japanese"),
        }
    }

    /// **What reaches the model is what a song needs, and nothing else.**
    #[test]
    fn the_request_carries_the_night_and_not_the_guests_details() {
        let request = song_request(&brief());
        for needed in [
            "Guest: Aiko",
            "Sang tonight: Juan Luis Guerra - Bachata en Fukuoka",
            "Event: Noche Latina",
            "Place: Bar Sol, Fukuoka",
            "Date: Saturday 26 September 2026",
            "Tonight's music: latin",
            "Favourite band: Aventura",
            "The DJ's ideas: first time on stage, brave",
            "Languages, in order: Spanish; Japanese",
        ] {
            assert!(
                request.contains(needed),
                "{needed} missing from:\n{request}"
            );
        }
        // The brief has no field for them, and nothing adds one.
        assert!(!request.to_lowercase().contains("email"));
        assert!(!request.to_lowercase().contains("phone"));
        // An empty field is no line, not "Label: ".
        let bare = song_request(&SongBrief {
            singer: "Aiko".into(),
            ..SongBrief::default()
        });
        assert_eq!(bare, "Guest: Aiko\n");
    }

    #[test]
    fn a_second_version_only_when_the_guests_language_differs() {
        assert_eq!(languages("es", "x", "Japanese"), ["Spanish", "Japanese"]);
        assert_eq!(languages("es", "x", "spanish"), ["Spanish"]);
        assert_eq!(languages("Spanish", "x", "es"), ["Spanish"]);
        assert_eq!(languages("es", "x", ""), ["Spanish"]);
        assert_eq!(
            languages("", "Obsesión", "German"),
            ["the language \"Obsesión\" is sung in", "German"]
        );
        assert_eq!(
            language_name("Kreyòl"),
            "Kreyòl",
            "an unknown one is kept as written"
        );
    }

    /// **The answer is read into versions, decoration and all.**
    #[test]
    fn an_answer_becomes_a_style_and_a_version_per_language() {
        let draft = parse_song(
            "Here you go!\n\
             **STYLE:** bachata, romantic guitar, warm female voice, 128 bpm\n\
             === LYRICS: Spanish ===\n\
             [Verse]\n\
             Aiko cantó esta noche\n\
             \n\
             [Chorus]\n\
             Noche Latina\n\
             ==== lyrics: Japanese ====\n\
             [Verse]\n\
             今夜アイコが歌った\n\
             === LYRICS: French ===\n\
             \n",
        );
        assert_eq!(
            draft.style,
            "bachata, romantic guitar, warm female voice, 128 bpm"
        );
        assert_eq!(
            draft.versions.len(),
            2,
            "an empty version is dropped: {draft:?}"
        );
        assert_eq!(draft.versions[0].language, "Spanish");
        assert_eq!(
            draft.versions[0].lyrics,
            "[Verse]\nAiko cantó esta noche\n\n[Chorus]\nNoche Latina"
        );
        assert_eq!(draft.versions[1].language, "Japanese");
        assert!(draft.versions[1].lyrics.contains("今夜アイコが歌った"));
    }

    #[test]
    fn prose_with_no_sections_is_no_song() {
        let draft = parse_song("I'm sorry, I can't help with that.");
        assert!(draft.versions.is_empty());
        assert!(draft.style.is_empty());
    }

    /// **Names are taken out of the style, and it is cut at a comma.**
    #[test]
    fn the_style_describes_a_sound_and_names_nobody() {
        let names = names_in(&brief());
        let style = clean_style(
            "bachata, in the style of Aventura, Juan Luis Guerra vibes, romantic guitar, Bachata, \
             like Obsesión, warm voice",
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        assert_eq!(style, "bachata, romantic guitar, warm voice");
        // A name the record does not hold, in a comparison, goes too; a
        // word that only starts the same way does not.
        let unknown = clean_style(
            "bachata, in the style of Romeo Santos, Like Prince Royce, inspired by the nineties, likeable groove",
            &[],
        );
        assert_eq!(unknown, "bachata, likeable groove");
        let long = (0..60)
            .map(|i| format!("tag{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let cut = clean_style(&long, &[]);
        assert!(cut.chars().count() <= STYLE_MOST, "{}", cut.len());
        assert!(!cut.ends_with(','), "{cut}");
        assert!(
            cut.split(", ").all(|tag| tag.starts_with("tag")),
            "cut mid-tag: {cut}"
        );
    }

    #[test]
    fn long_words_are_cut_at_a_line() {
        let long = (0..400)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let draft = parse_song(&format!("STYLE: pop\n=== LYRICS: English ===\n{long}"));
        let words = &draft.versions[0].lyrics;
        assert!(words.chars().count() <= LYRICS_MOST);
        assert!(
            words.lines().all(|line| line.starts_with("line ")),
            "cut mid-line"
        );
    }
}

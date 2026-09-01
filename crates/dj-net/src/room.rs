//! What the room asks for.
//!
//! A song request arrives from a stranger's phone, and everything about that
//! sentence is a design constraint. Strangers type the same song six different
//! ways, type it in Spanish with and without the accents, second each other's
//! ideas, and occasionally decide it would be funny to submit four hundred of
//! them. So this is not a list of strings: it is a book that folds the six
//! spellings into one entry, counts how many people wanted it, and is bounded
//! in every direction that a room can push on.
//!
//! It holds no locks, does no I/O and knows nothing about HTTP. The server in
//! [`crate::web`] hands it text; the application decides what to do with the
//! tally. That separation is what lets the whole of the awkward part — the
//! folding, the counting, the limits — be tested without a socket.

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

/// The most distinct songs the book will hold in one night.
///
/// A busy room asks for a few dozen. Two hundred is far past that and still
/// small enough that a phone with a script cannot grow it without bound.
pub const MOST_ASKS: usize = 200;

/// The longest request, in characters.
///
/// "Aventura - Obsesión (the one with the piano)" is 43. A hundred and twenty
/// leaves room for a long title, a featured artist and a parenthetical, and
/// refuses an essay.
pub const LONGEST_ASK: usize = 120;

/// How many new songs one phone may put in the book per [`WINDOW`].
///
/// Three, because a person at a bar has two or three songs in mind and a
/// person with a grudge has hundreds.
pub const ASKS_PER_PHONE: usize = 3;

/// How much of a name has to be there before a partial match counts.
///
/// Six folded characters. Below that the request is a word rather than a
/// title: "si", "amor" and "baila" are each inside a hundred songs, and a
/// request auto-marked as played because the deck happens to hold one of them
/// is a request the room asked for and never got.
pub const SPECIFIC_ENOUGH: usize = 6;

/// The window the per-phone limit is measured over.
pub const WINDOW: Duration = Duration::from_secs(15 * 60);

/// What happened to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A song nobody had asked for yet.
    Added(u64),
    /// Somebody had already asked; this counts as another voice for it.
    Seconded(u64),
    /// This phone had already seconded that one. Counted once, not twice.
    AlreadyYours(u64),
    /// Nothing but spaces and punctuation.
    Empty,
    /// Past [`LONGEST_ASK`].
    TooLong,
    /// This phone has used up [`ASKS_PER_PHONE`] for now.
    TooManyFromYou,
    /// The book is at [`MOST_ASKS`].
    BookIsFull,
    /// A `+1` for a row that is no longer there.
    NoSuchAsk,
}

/// Where an ask stands, as far as the DJ is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Standing {
    #[default]
    Waiting,
    Played,
    /// Seen and not going to happen — wrong room, wrong night, no such record.
    Passed,
}

/// One song the room asked for, however many people asked for it.
#[derive(Debug, Clone)]
pub struct Ask {
    pub id: u64,
    /// As the first person typed it.
    ///
    /// Not the last: the tally belongs to whoever thought of it, and a title
    /// that changes spelling under the DJ's eyes between glances is worse than
    /// one that is occasionally somebody's abbreviation.
    pub text: String,
    /// The folded form the six spellings agree on. Not shown to anyone.
    key: String,
    pub voices: u32,
    pub first_asked: SystemTime,
    pub last_asked: SystemTime,
    pub standing: Standing,
    /// Who has already been counted for this one.
    seconded_by: BTreeSet<String>,
}

impl Ask {
    /// Whether `phone` has already been counted for this ask.
    #[must_use]
    pub fn is_from(&self, phone: &str) -> bool {
        self.seconded_by.contains(phone)
    }
}

/// The room's requests for one session.
#[derive(Debug, Default)]
pub struct RequestBook {
    asks: Vec<Ask>,
    next_id: u64,
    /// When each phone last put something *new* in the book.
    recent: Vec<(String, SystemTime)>,
}

impl RequestBook {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a request from `phone`.
    ///
    /// `now` is passed rather than read so a night can be replayed and a limit
    /// can be tested without waiting fifteen minutes for it.
    pub fn ask(&mut self, phone: &str, text: &str, now: SystemTime) -> Outcome {
        let text = tidy(text);
        if text.chars().count() > LONGEST_ASK {
            return Outcome::TooLong;
        }
        let key = fold(&text);
        if key.is_empty() {
            return Outcome::Empty;
        }

        if let Some(ask) = self.asks.iter_mut().find(|a| a.key == key) {
            // Seconding is deliberately not rate-limited. One phone seconding
            // every ask in the book adds one voice to every ask in the book,
            // which changes no ordering at all -- so the cheap defence is the
            // one that already exists: each phone counts once per ask.
            if !ask.seconded_by.insert(phone.to_owned()) {
                return Outcome::AlreadyYours(ask.id);
            }
            ask.voices += 1;
            ask.last_asked = now;
            return Outcome::Seconded(ask.id);
        }

        if self.asks.len() >= MOST_ASKS {
            return Outcome::BookIsFull;
        }
        self.forget_older_than(now);
        if self.recent.iter().filter(|(who, _)| who == phone).count() >= ASKS_PER_PHONE {
            return Outcome::TooManyFromYou;
        }

        self.next_id += 1;
        let id = self.next_id;
        self.recent.push((phone.to_owned(), now));
        self.asks.push(Ask {
            id,
            text,
            key,
            voices: 1,
            first_asked: now,
            last_asked: now,
            standing: Standing::Waiting,
            seconded_by: [phone.to_owned()].into_iter().collect(),
        });
        Outcome::Added(id)
    }

    /// Add a voice to an ask the room can already see, by id.
    ///
    /// The `+1` next to a row on the page. Separate from [`Self::ask`] because
    /// the row is already in the book: re-folding its own text to find it
    /// again would be a spelling round trip that can only lose.
    pub fn second(&mut self, id: u64, phone: &str, now: SystemTime) -> Outcome {
        let Some(ask) = self.asks.iter_mut().find(|a| a.id == id) else {
            return Outcome::NoSuchAsk;
        };
        if !ask.seconded_by.insert(phone.to_owned()) {
            return Outcome::AlreadyYours(id);
        }
        ask.voices += 1;
        ask.last_asked = now;
        Outcome::Seconded(id)
    }

    /// Drop rate-limit records that have aged out of the window.
    fn forget_older_than(&mut self, now: SystemTime) {
        self.recent.retain(|(_, when)| {
            now.duration_since(*when)
                .map_or(true, |since| since < WINDOW)
        });
    }

    /// Say what became of an ask. `false` when there is no such ask.
    pub fn settle(&mut self, id: u64, standing: Standing) -> bool {
        match self.asks.iter_mut().find(|a| a.id == id) {
            Some(ask) => {
                ask.standing = standing;
                true
            }
            None => false,
        }
    }

    /// Everything, in the order it was first asked.
    #[must_use]
    pub fn all(&self) -> &[Ask] {
        &self.asks
    }

    /// What the DJ should look at: still waiting, most-wanted first.
    ///
    /// Ties break towards whoever asked first, so a song does not lose its
    /// place to one that caught up later.
    #[must_use]
    pub fn waiting(&self) -> Vec<&Ask> {
        let mut waiting: Vec<&Ask> = self
            .asks
            .iter()
            .filter(|a| a.standing == Standing::Waiting)
            .collect();
        waiting.sort_by(|a, b| {
            b.voices
                .cmp(&a.voices)
                .then(a.first_asked.cmp(&b.first_asked))
        });
        waiting
    }

    /// The ask matching `text`, if the room has already asked for it.
    ///
    /// How a track starting to play finds its own request without the DJ
    /// having to notice that it had one.
    ///
    /// Deliberately not an equality test. Nobody types what a file is called:
    /// the room asks for "Obsesión" and the deck loads "Aventura - Obsesión
    /// (Album Version) [128kbps]". So one folded key containing the other
    /// counts — which is why [`SPECIFIC_ENOUGH`] exists, because "si" is
    /// inside half a Latin catalogue. The longest match wins, so an ask for
    /// the artist does not beat an ask for the song.
    #[must_use]
    pub fn matching(&self, text: &str) -> Option<&Ask> {
        let key = fold(&tidy(text));
        if key.is_empty() {
            return None;
        }
        self.asks
            .iter()
            .filter(|a| {
                a.key.chars().count() >= SPECIFIC_ENOUGH
                    && (key.contains(&a.key) || a.key.contains(&key))
            })
            .max_by_key(|a| a.key.chars().count())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.asks.is_empty()
    }
}

/// Trim, and collapse runs of whitespace into single spaces.
fn tidy(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The form six spellings of one song agree on.
///
/// Lowercase, unaccented, letters and digits only, `&` read as "and". The
/// accents are the point: half a room types "Obsesión" and half types
/// "Obsesion", and a book that treats those as two songs splits the tally in
/// half — which is the one number the whole feature exists to produce.
fn fold(text: &str) -> String {
    text.replace('&', " and ")
        .chars()
        .flat_map(char::to_lowercase)
        .map(plain)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// A lowercase letter without its accent, where djmanzo's music has accents.
///
/// Spanish, Portuguese, French and German, which covers the Caribbean and
/// European catalogue this is for. Deliberately a list rather than Unicode
/// decomposition: the list is short, finite, and does not add a dependency to
/// fold four hundred characters nobody will type into a request box.
fn plain(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds)
    }

    /// **Two spellings of one song are one ask with two voices.**
    ///
    /// The whole feature is the tally. A book where "Obsesión" and "obsesion"
    /// are separate rows tells the DJ two people wanted two different things,
    /// when in fact four people wanted one.
    #[test]
    fn accents_and_case_do_not_split_the_tally() {
        let mut book = RequestBook::new();
        assert_eq!(book.ask("a", "Obsesión", at(0)), Outcome::Added(1));
        assert_eq!(book.ask("b", "obsesion", at(1)), Outcome::Seconded(1));
        assert_eq!(book.ask("c", "  OBSESION  ", at(2)), Outcome::Seconded(1));
        assert_eq!(book.all().len(), 1);
        assert_eq!(book.all()[0].voices, 3);

        // The one that costs a Spanish room the most: a phone keyboard that
        // will not produce ñ, next to one that will.
        assert_eq!(book.ask("a", "Niña Bonita", at(3)), Outcome::Added(2));
        assert_eq!(book.ask("b", "nina bonita", at(4)), Outcome::Seconded(2));
        assert_eq!(book.all().len(), 2);
        // And every other accent this catalogue actually carries.
        for (n, (accented, plainly)) in [
            ("Corazón", "corazon"),
            ("Bailá", "baila"),
            ("Você", "voce"),
            ("Küss", "kuss"),
            ("Água", "agua"),
            ("Ceú", "ceu"),
        ]
        .into_iter()
        .enumerate()
        {
            // A phone each: three is all one phone gets, which is the point of
            // the limit and would otherwise be mistaken for a folding failure.
            let (first_phone, second_phone) = (format!("{n} one"), format!("{n} two"));
            let first = book.ask(&first_phone, accented, at(5));
            let Outcome::Added(id) = first else {
                panic!("{accented} was not new: {first:?}");
            };
            assert_eq!(
                book.ask(&second_phone, plainly, at(6)),
                Outcome::Seconded(id),
                "{accented} and {plainly} were counted apart"
            );
        }
        // Shown as the first person wrote it, accent and all.
        assert_eq!(book.all()[0].text, "Obsesión");
    }

    /// **Punctuation and spacing do not split it either.**
    #[test]
    fn punctuation_and_spacing_fold_away() {
        let mut book = RequestBook::new();
        assert_eq!(book.ask("a", "Rock & Roll", at(0)), Outcome::Added(1));
        assert_eq!(book.ask("b", "rock and roll", at(1)), Outcome::Seconded(1));
        assert_eq!(
            book.ask("c", "Rock  and   Roll!", at(2)),
            Outcome::Seconded(1)
        );
        assert_eq!(book.all().len(), 1);
    }

    /// **One phone cannot vote for the same song twice.**
    ///
    /// Otherwise the tally measures persistence rather than popularity, and
    /// the loudest thing in the room is a thumb.
    #[test]
    fn a_phone_counts_once_per_ask() {
        let mut book = RequestBook::new();
        assert_eq!(book.ask("a", "Suavemente", at(0)), Outcome::Added(1));
        assert_eq!(book.ask("a", "suavemente", at(1)), Outcome::AlreadyYours(1));
        assert_eq!(book.ask("a", "SUAVEMENTE", at(2)), Outcome::AlreadyYours(1));
        assert_eq!(book.all()[0].voices, 1);
    }

    /// **One phone cannot fill the book, and can again once the window passes.**
    #[test]
    fn a_phone_may_add_only_a_few_new_songs_at_a_time() {
        let mut book = RequestBook::new();
        for n in 0..ASKS_PER_PHONE {
            let text = format!("song number {n}");
            assert!(matches!(book.ask("a", &text, at(0)), Outcome::Added(_)));
        }
        assert_eq!(book.ask("a", "one more", at(0)), Outcome::TooManyFromYou);
        // Somebody else is unaffected.
        assert!(matches!(
            book.ask("b", "one more", at(0)),
            Outcome::Added(_)
        ));
        // And the same phone is welcome again later.
        let later = at(WINDOW.as_secs() + 1);
        assert!(matches!(
            book.ask("a", "much later", later),
            Outcome::Added(_)
        ));
    }

    /// **Seconding is never rate-limited.**
    ///
    /// A phone that has used its three may still agree with the room, because
    /// agreeing costs the book nothing and refusing it would silence the
    /// people who came in a group.
    #[test]
    fn a_phone_out_of_asks_may_still_second() {
        let mut book = RequestBook::new();
        assert!(matches!(
            book.ask("b", "Vivir Mi Vida", at(0)),
            Outcome::Added(_)
        ));
        for n in 0..ASKS_PER_PHONE {
            let text = format!("song number {n}");
            assert!(matches!(book.ask("a", &text, at(0)), Outcome::Added(_)));
        }
        assert_eq!(book.ask("a", "spent", at(0)), Outcome::TooManyFromYou);
        assert!(matches!(
            book.ask("a", "Vivir Mi Vida", at(0)),
            Outcome::Seconded(_)
        ));
    }

    #[test]
    fn nothing_and_essays_are_refused() {
        let mut book = RequestBook::new();
        assert_eq!(book.ask("a", "   ", at(0)), Outcome::Empty);
        assert_eq!(book.ask("a", "!!! ??? ...", at(0)), Outcome::Empty);
        assert_eq!(
            book.ask("a", &"x".repeat(LONGEST_ASK + 1), at(0)),
            Outcome::TooLong
        );
        // Exactly the limit is fine.
        assert!(matches!(
            book.ask("a", &"x".repeat(LONGEST_ASK), at(0)),
            Outcome::Added(_)
        ));
        assert!(book.all().len() == 1);
    }

    /// **The limit measures the song, not the spacing.**
    ///
    /// Why the tidying collapses runs of whitespace rather than only trimming
    /// the ends. A thumb on a phone keyboard produces double spaces, and a
    /// request refused for being too long when it is two words is a refusal
    /// nobody can act on.
    #[test]
    fn padding_does_not_make_a_short_request_too_long() {
        let mut book = RequestBook::new();
        let padded = format!("Aventura{}Obsesión", " ".repeat(LONGEST_ASK));
        assert!(
            padded.chars().count() > LONGEST_ASK,
            "the test needs a request that is only long because of the spaces"
        );
        assert!(matches!(book.ask("a", &padded, at(0)), Outcome::Added(_)));
        assert_eq!(book.all()[0].text, "Aventura Obsesión");
    }

    /// **The length limit counts characters, not bytes.**
    ///
    /// A request in Spanish is not two thirds of a request in English.
    #[test]
    fn the_length_limit_is_in_characters() {
        let mut book = RequestBook::new();
        let accented = "ó".repeat(LONGEST_ASK);
        assert!(
            accented.len() > LONGEST_ASK,
            "the test needs multibyte text"
        );
        assert!(matches!(book.ask("a", &accented, at(0)), Outcome::Added(_)));
    }

    /// **The book stops growing, and stops before it is asked to allocate.**
    #[test]
    fn the_book_has_a_ceiling() {
        let mut book = RequestBook::new();
        for n in 0..MOST_ASKS {
            let phone = format!("phone {n}");
            let text = format!("song number {n}");
            assert!(matches!(book.ask(&phone, &text, at(0)), Outcome::Added(_)));
        }
        assert_eq!(
            book.ask("someone else", "one more", at(0)),
            Outcome::BookIsFull
        );
        // Seconding still works when the book is full: it adds no row.
        assert!(matches!(
            book.ask("someone else", "song number 0", at(0)),
            Outcome::Seconded(_)
        ));
        assert_eq!(book.all().len(), MOST_ASKS);
    }

    /// **Most-wanted first, and an early ask keeps its place in a tie.**
    #[test]
    fn waiting_is_ordered_by_voices_then_by_who_asked_first() {
        let mut book = RequestBook::new();
        book.ask("a", "early", at(0));
        book.ask("b", "late", at(100));
        book.ask("c", "late", at(101)); // two voices
        book.ask("d", "earlier tie", at(50));
        book.ask("e", "earlier tie", at(51)); // also two voices

        let order: Vec<&str> = book.waiting().iter().map(|a| a.text.as_str()).collect();
        assert_eq!(order, vec!["earlier tie", "late", "early"]);
    }

    /// **Settled asks leave the waiting list without leaving the book.**
    #[test]
    fn playing_one_takes_it_off_the_list() {
        let mut book = RequestBook::new();
        let Outcome::Added(id) = book.ask("a", "Suavemente", at(0)) else {
            panic!("not added");
        };
        book.ask("b", "Vivir Mi Vida", at(1));
        assert!(book.settle(id, Standing::Played));
        assert_eq!(book.waiting().len(), 1);
        assert_eq!(book.all().len(), 2);
        assert!(!book.settle(9_999, Standing::Played));
    }

    /// **`+1` counts once, and only for a row that exists.**
    #[test]
    fn seconding_by_id_counts_once() {
        let mut book = RequestBook::new();
        let Outcome::Added(id) = book.ask("a", "Suavemente", at(0)) else {
            panic!("not added");
        };
        assert_eq!(book.second(id, "b", at(1)), Outcome::Seconded(id));
        assert_eq!(book.second(id, "b", at(2)), Outcome::AlreadyYours(id));
        assert_eq!(book.second(id, "a", at(3)), Outcome::AlreadyYours(id));
        assert_eq!(book.second(9_999, "c", at(4)), Outcome::NoSuchAsk);
        assert_eq!(book.all()[0].voices, 2);
    }

    /// **A track starting to play finds its own request.**
    ///
    /// Including the case that actually happens: somebody typed the song, and
    /// the file on disk carries the artist, the edition and the bitrate.
    #[test]
    fn a_playing_track_matches_the_ask_that_wanted_it() {
        let mut book = RequestBook::new();
        book.ask("a", "Obsesión", at(0));
        assert!(book.matching("obsesion").is_some());
        assert!(
            book.matching("Aventura - Obsesión (Album Version) [128kbps]")
                .is_some(),
            "a request never carries the whole filename"
        );
        assert!(book.matching("something else").is_none());
        assert!(book.matching("   ").is_none());
    }

    /// **A word is not a title.**
    ///
    /// Left as plain containment, an ask for "amor" is marked played by the
    /// next song with "amor" in it — and the room's actual request quietly
    /// leaves the list without anybody hearing it.
    #[test]
    fn a_short_request_does_not_match_everything() {
        let mut book = RequestBook::new();
        book.ask("a", "si", at(0));
        book.ask("b", "amor", at(0));
        assert!(book.matching("Sí Señor - Amor de Verano").is_none());
    }

    /// **The most specific ask wins.**
    #[test]
    fn the_longest_match_is_the_one_marked() {
        let mut book = RequestBook::new();
        let Outcome::Added(artist) = book.ask("a", "Aventura", at(0)) else {
            panic!("not added");
        };
        let Outcome::Added(song) = book.ask("b", "Aventura - Obsesión", at(1)) else {
            panic!("not added");
        };
        let found = book
            .matching("Aventura - Obsesión (Album Version)")
            .expect("a match");
        assert_eq!(found.id, song, "the artist's ask beat the song's");
        assert_ne!(found.id, artist);
    }
}

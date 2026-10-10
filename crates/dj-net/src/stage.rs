//! K4: singers from the room — the second book on the request page.
//!
//! `docs/KARAOKE.md` §7. A guest puts their own name down to sing, from the
//! page the room already reaches for requests, and picks the song from the
//! songbook: the records in the DJ's collection that have words. This is the
//! book that takes it, beside [`crate::room::RequestBook`] and under the same
//! rule — a phone can add to a book and reach nothing else. The rotation is
//! the application's; it takes what is in here with [`Stage::take`] and tells
//! the book who is waiting with [`Stage::set_order`], and nothing a phone
//! sends reaches it any other way.
//!
//! Like the request book it holds no locks, does no I/O and knows nothing of
//! HTTP, so every limit is tested without a socket.

use crate::room::{fold, tidy};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// The longest name a phone may put down, in characters.
pub const LONGEST_NAME: usize = 40;

/// The longest song a phone may write, where the songbook is not offered.
pub const LONGEST_SONG: usize = 120;

/// The most matches one search shows: a phone's screen of them.
pub const MATCHES: usize = 12;

/// The most sign-ups one night takes from phones.
pub const MOST_SIGNUPS: usize = 300;

/// The most songs a singer may have waiting, whatever the host sets.
pub const MOST_EACH: u8 = 3;

/// What the host allows, from the Singers panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    /// Whether phones may put names down at all. Off until the host opens it.
    pub open: bool,
    /// Songs one singer may have waiting at once, 1 to [`MOST_EACH`].
    pub songs_each: u8,
    /// Whether the song is chosen from the songbook (true) or written (false).
    pub songbook: bool,
    /// Whether a phone may propose a key.
    pub key: bool,
    /// Whether a phone may propose what is done with the record's singer.
    pub guide: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            open: false,
            songs_each: 1,
            songbook: true,
            key: false,
            guide: false,
        }
    }
}

/// The most a proposed key may move, either way: the rotation's own bound
/// (`dj_app::karaoke::KEY_RANGE`).
pub const KEY_RANGE: i32 = 7;

/// What a singer may ask be done with the record's own singer — the three
/// the Singers panel offers per deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    AsRecorded,
    Guide,
    Out,
}

impl Voice {
    pub const ALL: [Self; 3] = [Self::AsRecorded, Self::Guide, Self::Out];

    /// As the page's form and the rotation name it.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::AsRecorded => "as-recorded",
            Self::Guide => "guide",
            Self::Out => "out",
        }
    }

    #[must_use]
    pub fn by_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.tag() == tag)
    }
}

/// What a singer asks for besides the song: proposals the host takes or
/// refuses, never settings (docs/KARAOKE.md §7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Proposal {
    /// Semitones from the record's own key.
    pub key: Option<i32>,
    pub voice: Option<Voice>,
}

impl Proposal {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.key.is_none() && self.voice.is_none()
    }
}

/// A record a singer may choose: one of the DJ's records with words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookSong {
    /// The record, as the application names it. Never shown to a phone.
    pub track: String,
    pub title: String,
    pub artist: String,
}

/// One search result, as a phone sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Names the record to [`Stage::sign`] for as long as this songbook
    /// stands. Not the record's own name, which a phone has no use for.
    pub token: String,
    pub title: String,
    pub artist: String,
}

/// What a singer chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// A record from the songbook.
    Book(BookSong),
    /// A song written in, where the songbook is not offered: the host finds
    /// the record before it can be called.
    Written(String),
}

impl Chosen {
    /// What the song is called, as the rotation shows it.
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Book(song) if song.artist.is_empty() => song.title.clone(),
            Self::Book(song) => format!("{} — {}", song.artist, song.title),
            Self::Written(text) => text.clone(),
        }
    }
}

/// One name put down from a phone, waiting for the application to take it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignUp {
    pub name: String,
    pub song: Chosen,
    /// Only what the host allows phones to propose; the rest is dropped.
    pub proposal: Proposal,
    pub at: SystemTime,
}

/// What became of a sign-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signed {
    Added,
    /// The host has not opened sign-ups, or has closed them.
    Closed,
    NoName,
    LongName,
    NoSong,
    LongSong,
    /// A token from a songbook that has since changed: search again.
    Gone,
    /// This phone already put another name down.
    OtherName,
    /// Somebody else is singing under that name tonight.
    NameTaken,
    /// The singer has as many songs waiting as the host allows.
    TooMany,
    /// [`MOST_SIGNUPS`] tonight.
    Full,
}

/// Where a phone's singer stands, as its page says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Put down, and not yet in the rotation the host sees.
    Coming,
    /// In the rotation with this many singers before them: 0 is up next.
    Waiting(usize),
    /// In the rotation tonight, with nothing waiting.
    Sung,
}

/// The songbook, the sign-ups and what the host allows.
#[derive(Debug)]
pub struct Stage {
    rules: Rules,
    songs: Vec<(BookSong, String)>,
    /// Which songbook a token belongs to.
    edition: u64,
    /// Put down and not yet taken by the application.
    waiting: Vec<SignUp>,
    /// Which name each phone put down, for the whole night.
    names: BTreeMap<String, String>,
    /// The rotation's calling order: each singer with songs waiting, and how
    /// many. As the application last said it.
    order: Vec<(String, usize)>,
    /// Everybody in tonight's rotation, waiting or not.
    singers: Vec<String>,
    /// Names whose phone pressed *I'm ready*, not yet taken.
    ready: Vec<String>,
    taken: usize,
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

/// Names as a host hears them: case and surrounding space do not make two
/// people. The rotation's own rule (`dj_app::karaoke`).
fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

impl Stage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Rules::default(),
            songs: Vec::new(),
            edition: edition_seed(),
            waiting: Vec::new(),
            names: BTreeMap::new(),
            order: Vec::new(),
            singers: Vec::new(),
            ready: Vec::new(),
            taken: 0,
        }
    }

    #[must_use]
    pub fn rules(&self) -> Rules {
        self.rules
    }

    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = Rules {
            songs_each: rules.songs_each.clamp(1, MOST_EACH),
            ..rules
        };
    }

    /// Replace the songbook. Tokens from the last one stop naming anything,
    /// so a phone that searched before the collection changed is asked to
    /// search again rather than handed the wrong record.
    ///
    /// The same songbook again changes nothing: a phone's search stays good
    /// while the host's panel asks for it to be kept current.
    pub fn set_songbook(&mut self, songs: Vec<BookSong>) {
        if songs.len() == self.songs.len()
            && songs.iter().zip(&self.songs).all(|(a, (b, _))| a == b)
        {
            return;
        }
        self.songs = songs
            .into_iter()
            .map(|song| {
                let folded = fold(&format!("{} {}", song.artist, song.title));
                (song, folded)
            })
            .collect();
        self.edition = self.edition.wrapping_add(1);
    }

    #[must_use]
    pub fn songbook_len(&self) -> usize {
        self.songs.len()
    }

    /// The songs whose artist and title hold every word of `query`, folded as
    /// requests are — so *obsesion* finds *Obsesión* — at most [`MATCHES`].
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<Match> {
        let words: Vec<String> = query
            .split_whitespace()
            .map(fold)
            .filter(|w| !w.is_empty())
            .collect();
        if words.iter().map(String::len).sum::<usize>() < 2 {
            return Vec::new();
        }
        self.songs
            .iter()
            .enumerate()
            .filter(|(_, (_, folded))| words.iter().all(|w| folded.contains(w.as_str())))
            .take(MATCHES)
            .map(|(index, (song, _))| Match {
                token: format!("{:x}-{index}", self.edition),
                title: song.title.clone(),
                artist: song.artist.clone(),
            })
            .collect()
    }

    fn by_token(&self, token: &str) -> Option<&BookSong> {
        let (edition, index) = token.split_once('-')?;
        if u64::from_str_radix(edition, 16).ok()? != self.edition {
            return None;
        }
        self.songs.get(index.parse::<usize>().ok()?).map(|(s, _)| s)
    }

    /// The name `phone` put down tonight, if any.
    #[must_use]
    pub fn name_of(&self, phone: &str) -> Option<&str> {
        self.names.get(phone).map(String::as_str)
    }

    /// Put a name down from `phone`: a song from the songbook by its `token`,
    /// or `written` where the songbook is not offered.
    pub fn sign(
        &mut self,
        phone: &str,
        name: &str,
        token: Option<&str>,
        written: Option<&str>,
        now: SystemTime,
    ) -> Signed {
        self.sign_with(phone, name, token, written, Proposal::default(), now)
    }

    /// [`Stage::sign`], with what the singer proposes besides the song. A
    /// part the host does not allow is dropped rather than refused — the
    /// page offers only what is allowed, so it can only come from a page
    /// opened before the host changed their mind — and a key is held to
    /// [`KEY_RANGE`].
    pub fn sign_with(
        &mut self,
        phone: &str,
        name: &str,
        token: Option<&str>,
        written: Option<&str>,
        proposal: Proposal,
        now: SystemTime,
    ) -> Signed {
        if !self.rules.open {
            return Signed::Closed;
        }
        let name = tidy(name);
        if name.is_empty() {
            return Signed::NoName;
        }
        if name.chars().count() > LONGEST_NAME {
            return Signed::LongName;
        }
        if let Some(mine) = self.names.get(phone)
            && !same(mine, &name)
        {
            return Signed::OtherName;
        }
        // A name another phone put down, or one the host put in the
        // rotation by hand, is somebody else's.
        let owned_elsewhere = self
            .names
            .iter()
            .any(|(other, theirs)| other != phone && same(theirs, &name));
        let by_hand = !self.names.contains_key(phone)
            && self.singers.iter().any(|singer| same(singer, &name));
        if owned_elsewhere || by_hand {
            return Signed::NameTaken;
        }
        let song = if self.rules.songbook {
            match token.map(str::trim).filter(|t| !t.is_empty()) {
                None => return Signed::NoSong,
                Some(token) => match self.by_token(token) {
                    Some(song) => Chosen::Book(song.clone()),
                    None => return Signed::Gone,
                },
            }
        } else {
            let text = tidy(written.unwrap_or_default());
            if text.is_empty() {
                return Signed::NoSong;
            }
            if text.chars().count() > LONGEST_SONG {
                return Signed::LongSong;
            }
            Chosen::Written(text)
        };
        let already = self.songs_waiting(&name);
        if already >= usize::from(self.rules.songs_each) {
            return Signed::TooMany;
        }
        if self.taken + self.waiting.len() >= MOST_SIGNUPS {
            return Signed::Full;
        }
        self.names.insert(phone.to_owned(), name.clone());
        let proposal = Proposal {
            key: proposal
                .key
                .filter(|_| self.rules.key)
                .map(|key| key.clamp(-KEY_RANGE, KEY_RANGE))
                .filter(|key| *key != 0),
            voice: proposal.voice.filter(|_| self.rules.guide),
        };
        self.waiting.push(SignUp {
            name,
            song,
            proposal,
            at: now,
        });
        Signed::Added
    }

    /// Songs `name` has waiting: in the rotation, and put down since.
    fn songs_waiting(&self, name: &str) -> usize {
        let in_rotation = self
            .order
            .iter()
            .find(|(singer, _)| same(singer, name))
            .map_or(0, |(_, songs)| *songs);
        in_rotation + self.waiting.iter().filter(|s| same(&s.name, name)).count()
    }

    /// Whether `phone` may put down a song now: no singer yet, or one with
    /// fewer songs waiting than the host allows.
    #[must_use]
    pub fn may_add(&self, phone: &str) -> bool {
        self.names
            .get(phone)
            .is_none_or(|name| self.songs_waiting(name) < usize::from(self.rules.songs_each))
    }

    /// Where `phone`'s singer stands.
    #[must_use]
    pub fn place(&self, phone: &str) -> Option<Place> {
        let name = self.names.get(phone)?;
        if let Some(index) = self.order.iter().position(|(n, _)| same(n, name)) {
            return Some(Place::Waiting(index));
        }
        if self.waiting.iter().any(|s| same(&s.name, name)) {
            return Some(Place::Coming);
        }
        Some(Place::Sung)
    }

    /// The phone of the singer up next says they are ready. False from any
    /// other phone: only the one called can be ready for it.
    pub fn ready(&mut self, phone: &str) -> bool {
        if self.place(phone) != Some(Place::Waiting(0)) {
            return false;
        }
        let Some(name) = self.names.get(phone) else {
            return false;
        };
        if !self.ready.iter().any(|n| same(n, name)) {
            self.ready.push(name.clone());
        }
        true
    }

    /// Whether phones have sent anything [`Stage::take`] would hand over.
    #[must_use]
    pub fn has_news(&self) -> bool {
        !self.waiting.is_empty() || !self.ready.is_empty()
    }

    /// Hand the application what phones sent since it last asked: the names
    /// put down, and the names whose phone said *I'm ready*.
    pub fn take(&mut self) -> (Vec<SignUp>, Vec<String>) {
        let signed = std::mem::take(&mut self.waiting);
        self.taken += signed.len();
        (signed, std::mem::take(&mut self.ready))
    }

    /// The rotation as it now stands: `order` is the calling order of the
    /// singers with songs waiting, and how many each; `singers` is everybody
    /// in tonight's rotation.
    pub fn set_order(&mut self, order: Vec<(String, usize)>, singers: Vec<String>) {
        self.order = order;
        self.singers = singers;
    }

    /// A new night: every name, sign-up and readiness forgotten. The
    /// songbook and the host's rules stay.
    pub fn new_night(&mut self) {
        self.waiting.clear();
        self.names.clear();
        self.order.clear();
        self.singers.clear();
        self.ready.clear();
        self.taken = 0;
    }
}

/// Where a night's songbook editions start, so a token from yesterday's
/// djmanzo does not name a record in today's.
fn edition_seed() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, artist: &str) -> BookSong {
        BookSong {
            track: format!("{title}-id"),
            title: title.to_owned(),
            artist: artist.to_owned(),
        }
    }

    fn open_stage() -> Stage {
        let mut stage = Stage::new();
        stage.set_rules(Rules {
            open: true,
            ..Rules::default()
        });
        stage.set_songbook(vec![
            song("Obsesión", "Aventura"),
            song("Suavemente", "Elvis Crespo"),
            song("Dancing Queen", "ABBA"),
        ]);
        stage
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    fn token(stage: &Stage, query: &str) -> String {
        stage.search(query)[0].token.clone()
    }

    /// **A song is found the way people type it, and only by its token.**
    #[test]
    fn the_songbook_is_searched_as_people_type() {
        let stage = open_stage();
        let found = stage.search("aventura obsesion");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Obsesión");
        assert_eq!(stage.search("QUEEN")[0].artist, "ABBA");
        // Every word must be there, and one letter finds nothing.
        assert!(stage.search("aventura queen").is_empty());
        assert!(stage.search("a").is_empty());
        // What a phone is given names no file and no record.
        assert!(!found[0].token.contains("id"), "{}", found[0].token);
    }

    #[test]
    fn at_most_a_screen_of_matches() {
        let mut stage = open_stage();
        stage.set_songbook(
            (0..40)
                .map(|n| song(&format!("Love song {n}"), "Band"))
                .collect(),
        );
        assert_eq!(stage.search("love").len(), MATCHES);
    }

    /// **A name put down reaches the rotation once, with its record.**
    #[test]
    fn a_name_put_down_is_taken_once() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        assert_eq!(
            stage.sign("phone-a", " Ana ", Some(&t), None, now()),
            Signed::Added
        );
        assert_eq!(stage.place("phone-a"), Some(Place::Coming));
        let (signed, ready) = stage.take();
        assert_eq!(signed.len(), 1);
        assert_eq!(signed[0].name, "Ana");
        assert_eq!(signed[0].song, Chosen::Book(song("Obsesión", "Aventura")));
        assert!(ready.is_empty());
        assert!(stage.take().0.is_empty(), "taken once");
    }

    /// **Nothing is taken until the host opens sign-ups.**
    #[test]
    fn closed_until_the_host_opens_it() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        stage.set_rules(Rules::default());
        assert_eq!(
            stage.sign("phone-a", "Ana", Some(&t), None, now()),
            Signed::Closed
        );
        assert!(stage.take().0.is_empty());
    }

    /// **One phone, one singer; one name, one phone.**
    #[test]
    fn a_phone_puts_down_one_name_and_a_name_has_one_phone() {
        let mut stage = open_stage();
        stage.set_rules(Rules {
            open: true,
            songs_each: 3,
            songbook: true,
            ..Rules::default()
        });
        let t = token(&stage, "obsesion");
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Added);
        assert_eq!(
            stage.sign("a", "Bob", Some(&t), None, now()),
            Signed::OtherName
        );
        assert_eq!(
            stage.sign("b", "ana", Some(&t), None, now()),
            Signed::NameTaken
        );
        // A singer the host put in by hand is theirs, not a phone's.
        stage.set_order(vec![("Carla".into(), 1)], vec!["Carla".into()]);
        assert_eq!(
            stage.sign("c", "Carla", Some(&t), None, now()),
            Signed::NameTaken
        );
        // The same phone, the same name: another song.
        assert_eq!(stage.sign("a", "ANA", Some(&t), None, now()), Signed::Added);
    }

    /// **As many songs waiting as the host allows, counting the rotation's.**
    #[test]
    fn songs_waiting_are_held_to_the_hosts_number() {
        let mut stage = open_stage();
        let t = token(&stage, "suavemente");
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Added);
        assert_eq!(
            stage.sign("a", "Ana", Some(&t), None, now()),
            Signed::TooMany
        );
        stage.take();
        // In the rotation now with one song waiting: still one.
        stage.set_order(vec![("Ana".into(), 1)], vec!["Ana".into()]);
        assert_eq!(
            stage.sign("a", "Ana", Some(&t), None, now()),
            Signed::TooMany
        );
        // Sung, nothing waiting: another.
        stage.set_order(vec![], vec!["Ana".into()]);
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Added);
        // Two each, with one waiting: one more.
        stage.set_rules(Rules {
            open: true,
            songs_each: 2,
            songbook: true,
            ..Rules::default()
        });
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Added);
        assert_eq!(
            stage.sign("a", "Ana", Some(&t), None, now()),
            Signed::TooMany
        );
    }

    /// **A proposal is kept only where the host allows it, and in range.**
    #[test]
    fn a_proposal_is_what_the_host_allows() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        let asks = Proposal {
            key: Some(-12),
            voice: Some(Voice::Guide),
        };
        // Neither allowed: the song is taken, the proposal dropped.
        assert_eq!(
            stage.sign_with("a", "Ana", Some(&t), None, asks, now()),
            Signed::Added
        );
        assert!(stage.take().0[0].proposal.is_empty());
        stage.set_order(vec![], vec!["Ana".into()]);
        stage.set_rules(Rules {
            open: true,
            key: true,
            guide: true,
            ..Rules::default()
        });
        assert_eq!(
            stage.sign_with("a", "Ana", Some(&t), None, asks, now()),
            Signed::Added
        );
        assert_eq!(
            stage.take().0[0].proposal,
            Proposal {
                key: Some(-KEY_RANGE),
                voice: Some(Voice::Guide),
            },
            "the key held to the range"
        );
        // As recorded is no proposal at all.
        let none = Proposal {
            key: Some(0),
            voice: None,
        };
        assert_eq!(
            stage.sign_with("a", "Ana", Some(&t), None, none, now()),
            Signed::Added
        );
        assert!(stage.take().0[0].proposal.is_empty());
    }

    /// **The same songbook again keeps every phone's search good.**
    #[test]
    fn the_same_songbook_again_changes_nothing() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        stage.set_songbook(vec![
            song("Obsesión", "Aventura"),
            song("Suavemente", "Elvis Crespo"),
            song("Dancing Queen", "ABBA"),
        ]);
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Added);
    }

    /// **A token from a songbook that has changed names nothing.**
    #[test]
    fn a_stale_token_is_gone_not_another_record() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        stage.set_songbook(vec![song("Something else", "Other")]);
        assert_eq!(stage.sign("a", "Ana", Some(&t), None, now()), Signed::Gone);
        assert_eq!(
            stage.sign("a", "Ana", Some("ff-0"), None, now()),
            Signed::Gone
        );
        assert_eq!(stage.sign("a", "Ana", None, None, now()), Signed::NoSong);
    }

    /// **Where the host offers no songbook, a song is written in.**
    #[test]
    fn without_the_songbook_a_song_is_written() {
        let mut stage = open_stage();
        stage.set_rules(Rules {
            open: true,
            songs_each: 1,
            songbook: false,
            ..Rules::default()
        });
        assert_eq!(
            stage.sign("a", "Ana", Some("anything"), Some("  "), now()),
            Signed::NoSong
        );
        assert_eq!(
            stage.sign("a", "Ana", None, Some(&"x".repeat(LONGEST_SONG + 1)), now()),
            Signed::LongSong
        );
        assert_eq!(
            stage.sign("a", "Ana", None, Some("Juan Luis Guerra — Ojalá"), now()),
            Signed::Added
        );
        assert_eq!(
            stage.take().0[0].song,
            Chosen::Written("Juan Luis Guerra — Ojalá".into())
        );
    }

    #[test]
    fn a_name_is_a_name() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        assert_eq!(
            stage.sign("a", "   ", Some(&t), None, now()),
            Signed::NoName
        );
        assert_eq!(
            stage.sign("a", &"n".repeat(LONGEST_NAME + 1), Some(&t), None, now()),
            Signed::LongName
        );
    }

    /// **Only the phone of the singer up next can say they are ready.**
    #[test]
    fn only_the_singer_up_next_is_ready() {
        let mut stage = open_stage();
        stage.set_rules(Rules {
            open: true,
            songs_each: 1,
            songbook: true,
            ..Rules::default()
        });
        let t = token(&stage, "obsesion");
        stage.sign("a", "Ana", Some(&t), None, now());
        stage.sign("b", "Bob", Some(&t), None, now());
        stage.take();
        stage.set_order(
            vec![("Bob".into(), 1), ("Ana".into(), 1)],
            vec!["Ana".into(), "Bob".into()],
        );
        assert_eq!(stage.place("a"), Some(Place::Waiting(1)));
        assert!(!stage.ready("a"), "Ana is not up next");
        assert!(!stage.ready("nobody"));
        assert!(stage.ready("b"));
        assert!(stage.ready("b"), "pressed twice");
        assert_eq!(stage.take().1, vec!["Bob".to_owned()], "once");
        stage.set_order(vec![("Ana".into(), 1)], vec!["Ana".into(), "Bob".into()]);
        assert_eq!(stage.place("b"), Some(Place::Sung));
    }

    #[test]
    fn a_new_night_forgets_every_name() {
        let mut stage = open_stage();
        let t = token(&stage, "obsesion");
        stage.sign("a", "Ana", Some(&t), None, now());
        stage.new_night();
        assert_eq!(stage.name_of("a"), None);
        assert!(stage.take().0.is_empty());
        assert_eq!(stage.sign("b", "Ana", Some(&t), None, now()), Signed::Added);
    }
}

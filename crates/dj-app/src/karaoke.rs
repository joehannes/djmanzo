//! §107: the singer rotation — who sings next, and what, in which key.
//!
//! > then also, improve all karaoke feature, also investigate and research in
//! > online resources what else can be done to improve karaoke features and
//! > usability.
//!
//! `docs/KARAOKE.md` planned a singer queue and djmanzo had none; what it had
//! was a band-limited centre cancel nothing called and lyrics fetched only to
//! be searched. This is the queue, and it is first because a karaoke night is
//! run by its rotation: the host's one job the room can see is calling the
//! next singer, and getting it wrong in front of a queue of people who have
//! been waiting is the failure a karaoke host is remembered for.
//!
//! # The rules, and where they come from
//!
//! Not invented here. Hosts write their rotation rules down, and the dedicated
//! hosting programs (Karaoki, the KJ tools, VirtualDJ's rotation manager)
//! implement the same handful — read from their feature descriptions and hosts'
//! own guides, never from anybody's code; see `docs/RESEARCH.md`:
//!
//! - **First come, first served.** A new singer joins the end of the rotation,
//!   after everybody already in it, so nobody who has been waiting is jumped.
//! - **One song a turn.** A singer with three songs queued sings one, then
//!   goes to the back; the other two wait for their next turn.
//! - **Not ready when called is the bottom of the list** — not out, and not
//!   the front again, which would reward not being there.
//! - **The key is the singer's.** Hosts keep a history of who sang what in
//!   which key, because the same person asking for the same song next month
//!   wants it where they sang it last time. Remembered per singer and song,
//!   and offered when they ask again.
//!
//! The host can always override: move anybody anywhere. A rule that could not
//! be broken would be one the host routed around with a paper list.

use serde::{Deserialize, Serialize};

/// One song a singer has asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// What they asked for, as they said it.
    pub title: String,
    /// The record in the collection, when it has been found — hex, the same
    /// form every other command takes a track in.
    #[serde(default)]
    pub track: Option<String>,
    /// Where the record's file is, so the host can load it in one press.
    #[serde(default)]
    pub path: Option<String>,
    /// Semitones from the record's own key. Zero is as recorded.
    #[serde(default)]
    pub key: i32,
}

/// One singer in the rotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Singer {
    pub name: String,
    /// What they have asked for, in the order they asked.
    #[serde(default)]
    pub songs: Vec<Request>,
    /// How many times they have sung tonight.
    #[serde(default)]
    pub turns: u32,
}

/// Somebody sang something, in some key. The history keys are read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sung {
    pub singer: String,
    pub title: String,
    #[serde(default)]
    pub track: Option<String>,
    pub key: i32,
}

/// The night's rotation: the singers in the order they will be called, and
/// everything sung so far.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotation {
    /// In calling order. The first singer with a song is up next.
    #[serde(default)]
    pub singers: Vec<Singer>,
    /// Oldest first. Kept across nights, because keys are.
    #[serde(default)]
    pub history: Vec<Sung>,
}

/// The most key change a request may ask for, either way. Beyond a fifth a
/// record stops sounding like itself, and a slip of the finger to +40 should
/// not be a thing the engine is asked to do.
pub const KEY_RANGE: i32 = 7;

/// Names are matched the way a host hears them: case and surrounding space do
/// not make two people.
fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl Rotation {
    /// Somebody asks for a song.
    ///
    /// A singer already in the rotation adds it to their own list and keeps
    /// their place; a new one joins the end. With no key given, the key they
    /// sang this song in last time is used — the host's memory, kept for them.
    ///
    /// Returns whether it was taken: an empty name or title is not a request.
    pub fn ask(
        &mut self,
        name: &str,
        title: &str,
        track: Option<String>,
        path: Option<String>,
        key: Option<i32>,
    ) -> bool {
        let (name, title) = (name.trim(), title.trim());
        if name.is_empty() || title.is_empty() {
            return false;
        }
        let key = key
            .or_else(|| self.last_key(name, title, track.as_deref()))
            .unwrap_or(0)
            .clamp(-KEY_RANGE, KEY_RANGE);
        let request = Request {
            title: title.to_owned(),
            track,
            path,
            key,
        };
        match self
            .singers
            .iter_mut()
            .find(|singer| same(&singer.name, name))
        {
            Some(singer) => singer.songs.push(request),
            None => self.singers.push(Singer {
                name: name.to_owned(),
                songs: vec![request],
                turns: 0,
            }),
        }
        true
    }

    /// Who is up next, and with what: the first singer in calling order who
    /// has a song waiting.
    #[must_use]
    pub fn up_next(&self) -> Option<(&Singer, &Request)> {
        self.singers
            .iter()
            .find_map(|singer| singer.songs.first().map(|song| (singer, song)))
    }

    /// Everybody with a song waiting, in the order they will be called — what
    /// the room's screen shows so nobody has to ask the host.
    #[must_use]
    pub fn order(&self) -> Vec<(&Singer, &Request)> {
        self.singers
            .iter()
            .filter_map(|singer| singer.songs.first().map(|song| (singer, song)))
            .collect()
    }

    /// The singer up next has sung. Their song is written into the history,
    /// in the key it was sung in, and they go to the back — whether or not
    /// they have more songs waiting, so a singer who asks again later does
    /// not jump the people who have been waiting.
    ///
    /// Returns what was sung, or nothing when nobody was up.
    pub fn sang(&mut self) -> Option<Sung> {
        let at = self
            .singers
            .iter()
            .position(|singer| !singer.songs.is_empty())?;
        let mut singer = self.singers.remove(at);
        let song = singer.songs.remove(0);
        singer.turns += 1;
        let sung = Sung {
            singer: singer.name.clone(),
            title: song.title,
            track: song.track,
            key: song.key,
        };
        self.history.push(sung.clone());
        self.singers.push(singer);
        Some(sung)
    }

    /// Called, and not there. To the bottom of the list with their songs
    /// kept: not out, and not rewarded with the front again.
    pub fn not_here(&mut self, name: &str) -> bool {
        let Some(at) = self
            .singers
            .iter()
            .position(|singer| same(&singer.name, name))
        else {
            return false;
        };
        let singer = self.singers.remove(at);
        self.singers.push(singer);
        true
    }

    /// The host moves somebody: `to` is the place in calling order, clamped
    /// to the list. The rules are for fairness, and the host is the one who
    /// knows the birthday singer should go next.
    pub fn move_to(&mut self, name: &str, to: usize) -> bool {
        let Some(at) = self
            .singers
            .iter()
            .position(|singer| same(&singer.name, name))
        else {
            return false;
        };
        let singer = self.singers.remove(at);
        let to = to.min(self.singers.len());
        self.singers.insert(to, singer);
        true
    }

    /// Somebody leaves. Their history stays: it is how their key is known next
    /// time.
    pub fn leave(&mut self, name: &str) -> bool {
        let before = self.singers.len();
        self.singers.retain(|singer| !same(&singer.name, name));
        self.singers.len() != before
    }

    /// Change the key of a singer's next song — the host at the booth,
    /// between verses of the conversation "a bit lower?".
    pub fn set_key(&mut self, name: &str, key: i32) -> bool {
        let Some(song) = self
            .singers
            .iter_mut()
            .find(|singer| same(&singer.name, name))
            .and_then(|singer| singer.songs.first_mut())
        else {
            return false;
        };
        song.key = key.clamp(-KEY_RANGE, KEY_RANGE);
        true
    }

    /// The key this singer last sang this song in, if they have.
    ///
    /// Matched on the record when both are known, and on the title otherwise
    /// — "Dancing Queen" asked for by name is the same song whether or not the
    /// host has found the file yet.
    #[must_use]
    pub fn last_key(&self, name: &str, title: &str, track: Option<&str>) -> Option<i32> {
        self.history
            .iter()
            .rev()
            .find(|sung| {
                same(&sung.singer, name)
                    && match (track, sung.track.as_deref()) {
                        (Some(a), Some(b)) => a == b,
                        _ => same(&sung.title, title),
                    }
            })
            .map(|sung| sung.key)
    }

    /// A new night: everybody off the list, the history kept.
    pub fn clear(&mut self) {
        self.singers.clear();
    }
}

/// §107: the words for the singers' screen, as it draws them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SingerLyrics {
    /// Timed lines, in the order they are sung. Empty where the record has no
    /// timed words, and then `plain` is what there is.
    pub lines: Vec<LyricLine>,
    /// The words without times, a line each — shown still, because words
    /// wiped at a guessed pace would lead a singer wrong.
    pub plain: Vec<String>,
    /// Whether the lyrics database says the record has no words at all.
    pub instrumental: bool,
}

/// One timed line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LyricLine {
    /// Seconds into the record.
    pub at: f64,
    pub text: String,
    /// Each word and when it is sung, where the file says. Empty otherwise.
    pub words: Vec<(f64, String)>,
}

/// The singers' screen's words, from what the library stored for a record.
///
/// `None` in, nothing out: a record nobody has fetched words for is an empty
/// screen with its title on it, not an error.
#[must_use]
pub fn lyrics_for(stored: Option<dj_library::lyrics::Stored>) -> SingerLyrics {
    let Some(stored) = stored else {
        return SingerLyrics {
            lines: Vec::new(),
            plain: Vec::new(),
            instrumental: false,
        };
    };
    let lines: Vec<LyricLine> = stored
        .synced
        .as_deref()
        .map(dj_library::lrc::parse)
        .unwrap_or_default()
        .into_iter()
        .map(|line| LyricLine {
            at: line.at,
            text: line.text,
            words: line
                .words
                .into_iter()
                .map(|word| (word.at, word.text))
                .collect(),
        })
        .collect();
    let plain = if lines.is_empty() {
        stored
            .plain
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    } else {
        Vec::new()
    };
    SingerLyrics {
        lines,
        plain,
        instrumental: stored.instrumental,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn night(asks: &[(&str, &str)]) -> Rotation {
        let mut rotation = Rotation::default();
        for (name, title) in asks {
            assert!(rotation.ask(name, title, None, None, None));
        }
        rotation
    }

    fn calling(rotation: &Rotation) -> Vec<String> {
        rotation
            .order()
            .into_iter()
            .map(|(singer, song)| format!("{}:{}", singer.name, song.title))
            .collect()
    }

    /// **The load-bearing one: one song a turn, and to the back.**
    ///
    /// Ana asks for three songs, Ben and Cleo for one each. Ana sings one and
    /// goes behind Ben and Cleo; her second song waits for her next turn. A
    /// rotation that let her sing all three in a row is the one hosts write
    /// rules to prevent.
    #[test]
    fn one_song_a_turn_then_the_back_of_the_line() {
        let mut rotation = night(&[
            ("Ana", "Valerie"),
            ("Ana", "Jolene"),
            ("Ben", "Mr Brightside"),
            ("Cleo", "Dancing Queen"),
            ("Ana", "Zombie"),
        ]);
        assert_eq!(
            calling(&rotation),
            ["Ana:Valerie", "Ben:Mr Brightside", "Cleo:Dancing Queen"]
        );

        let sung = rotation.sang().unwrap();
        assert_eq!(
            (sung.singer.as_str(), sung.title.as_str()),
            ("Ana", "Valerie")
        );
        assert_eq!(
            calling(&rotation),
            ["Ben:Mr Brightside", "Cleo:Dancing Queen", "Ana:Jolene"]
        );

        rotation.sang();
        rotation.sang();
        assert_eq!(
            calling(&rotation),
            ["Ana:Jolene"],
            "Ben and Cleo have nothing left"
        );
        assert_eq!(
            rotation.singers.iter().map(|s| s.turns).collect::<Vec<_>>(),
            [1, 1, 1]
        );
    }

    /// **A newcomer joins the end, and nobody waiting is jumped** — including
    /// a singer who has sung and asked again, who is already at the back.
    #[test]
    fn a_newcomer_joins_the_end() {
        let mut rotation = night(&[("Ana", "Valerie"), ("Ben", "Mr Brightside")]);
        rotation.sang(); // Ana sings, goes behind Ben.
        rotation.ask("Dee", "Respect", None, None, None);
        rotation.ask("Ana", "Jolene", None, None, None);
        assert_eq!(
            calling(&rotation),
            ["Ben:Mr Brightside", "Ana:Jolene", "Dee:Respect"]
        );
    }

    /// **Not here when called: the bottom, with the songs kept.**
    #[test]
    fn not_ready_goes_to_the_bottom_not_out() {
        let mut rotation = night(&[
            ("Ana", "Valerie"),
            ("Ben", "Mr Brightside"),
            ("Cleo", "Dancing Queen"),
        ]);
        assert!(rotation.not_here("ana"));
        assert_eq!(
            calling(&rotation),
            ["Ben:Mr Brightside", "Cleo:Dancing Queen", "Ana:Valerie"]
        );
        assert_eq!(
            rotation.singers[2].turns, 0,
            "not being there is not a turn"
        );
        assert!(rotation.history.is_empty());
        assert!(!rotation.not_here("nobody"));
    }

    /// **The key is remembered per singer and song**, and offered when they
    /// ask again — next month as well as later tonight.
    #[test]
    fn a_singers_key_comes_back_with_the_song() {
        let mut rotation = Rotation::default();
        rotation.ask("Ana", "Valerie", Some("ab12".to_owned()), None, Some(-2));
        rotation.sang();
        rotation.clear(); // a new night; the history stays

        rotation.ask("ana ", "Valerie", Some("ab12".to_owned()), None, None);
        assert_eq!(
            rotation.up_next().unwrap().1.key,
            -2,
            "her key was not remembered"
        );

        // Somebody else's key for the same song is not hers.
        rotation.ask("Ben", "Valerie", Some("ab12".to_owned()), None, None);
        assert_eq!(rotation.singers[1].songs[0].key, 0);

        // The title is enough when the file has not been found yet.
        assert_eq!(rotation.last_key("Ana", "valerie", None), Some(-2));
    }

    /// The host can always override, and the key stays within a fifth.
    #[test]
    fn the_host_moves_people_and_keys_stay_sane() {
        let mut rotation = night(&[("Ana", "A"), ("Ben", "B"), ("Cleo", "C")]);
        assert!(rotation.move_to("Cleo", 0));
        assert_eq!(calling(&rotation), ["Cleo:C", "Ana:A", "Ben:B"]);
        assert!(rotation.move_to("Cleo", 99), "past the end is the end");
        assert_eq!(calling(&rotation), ["Ana:A", "Ben:B", "Cleo:C"]);

        assert!(rotation.set_key("Ana", 40));
        assert_eq!(rotation.up_next().unwrap().1.key, KEY_RANGE);
        assert!(rotation.leave("ben"));
        assert_eq!(calling(&rotation), ["Ana:A", "Cleo:C"]);
    }

    /// Nothing to sing is an empty answer, not a panic; and an empty name or
    /// title is not a request.
    #[test]
    fn an_empty_night_is_quiet() {
        let mut rotation = Rotation::default();
        assert!(rotation.up_next().is_none());
        assert!(rotation.sang().is_none());
        assert!(!rotation.ask("  ", "Valerie", None, None, None));
        assert!(!rotation.ask("Ana", "", None, None, None));
        assert!(rotation.singers.is_empty());
    }
}

#[cfg(test)]
mod lyric_tests {
    use super::*;

    fn stored(plain: &str, synced: Option<&str>) -> dj_library::lyrics::Stored {
        dj_library::lyrics::Stored {
            plain: plain.to_owned(),
            synced: synced.map(str::to_owned),
            found: true,
            instrumental: false,
            source: "lrclib".to_owned(),
            fetched_at: 0,
        }
    }

    /// **Timed words are timed lines, and the untimed words are not also
    /// sent** — a screen with both would show the lyric twice.
    #[test]
    fn synced_words_become_timed_lines() {
        let words = lyrics_for(Some(stored(
            "First\nSecond",
            Some("[00:01.00]First\n[00:03.00]<00:03.00>Sec<00:03.50>ond"),
        )));
        assert_eq!(words.lines.len(), 2);
        assert_eq!(
            words.lines[1].words,
            vec![(3.0, "Sec".to_owned()), (3.5, "ond".to_owned())]
        );
        assert!(words.plain.is_empty());
    }

    /// Untimed words are shown still, a line each, blank lines dropped; no
    /// words at all is an empty screen rather than an error.
    #[test]
    fn untimed_words_are_kept_still_and_nothing_is_nothing() {
        let words = lyrics_for(Some(stored("First\n\n  Second  ", None)));
        assert!(words.lines.is_empty());
        assert_eq!(words.plain, vec!["First".to_owned(), "Second".to_owned()]);
        let none = lyrics_for(None);
        assert!(none.lines.is_empty() && none.plain.is_empty() && !none.instrumental);
    }
}

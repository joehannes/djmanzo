//! K7: a quiz made from the collection.
//!
//! Questions made by rules over the DJ's own records — no model writes them,
//! so each can be checked and a wrong one seen for why. A question is a clip
//! of a record, played on a deck through the ordinary actions, and four
//! answers: the record and three decoys. See `docs/KARAOKE.md` §10.
//!
//! # The decoys are the craft
//!
//! Three titles picked at random from a collection of bachata and techno make
//! a bachata question answer itself. So the decoys are the records nearest
//! the asked one — its own genre family first, then within a tenth of its
//! tempo, then the nearest tempo there is — and a few of the nearest are
//! drawn from, so the same record is not always asked beside the same three.
//! Never another copy of the same title: two answers that read the same are
//! not a question.

use dj_core::TrackId;
use dj_library::LibraryTrack;

/// How long a clip lasts, in seconds of the record.
pub const CLIP_SECONDS: f64 = 10.0;

/// How long a *Stems in* clip lasts: its four stems brought in one at a
/// time, [`STEM_EVERY`] apart.
pub const STEMS_SECONDS: f64 = 20.0;

/// How long each stem of a *Stems in* clip plays before the next joins it.
pub const STEM_EVERY: f64 = 5.0;

/// The order a *Stems in* clip brings the stems in: the rhythm first, the
/// voice — the giveaway — last.
pub const STEMS_ORDER: [dj_core::Stem; 4] = [
    dj_core::Stem::Drums,
    dj_core::Stem::Bass,
    dj_core::Stem::Other,
    dj_core::Stem::Vocal,
];

/// How long a *Stems in* clip waits for its stems to be separated before it
/// gives up, in seconds.
pub const STEMS_WAIT: f64 = 45.0;

/// How fast a chipmunk plays, with keylock off: half as fast again, the voice
/// up a fifth — the way the trick has always sounded.
pub const CHIPMUNK_RATE: f64 = 1.5;

/// The shortest record worth asking about, in seconds: a clip from a third
/// of the way in has to fit.
pub const SHORTEST: f64 = 30.0;

/// How many of the nearest records the three decoys are drawn from.
const NEAREST: usize = 6;

/// What kind of question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A clip, as recorded.
    NameIt,
    /// The clip played backwards.
    Backwards,
    /// The clip at [`CHIPMUNK_RATE`] with keylock off.
    Chipmunk,
    /// The separated stems brought in one at a time ([`STEMS_ORDER`]).
    StemsIn,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::NameIt, Kind::Backwards, Kind::Chipmunk, Kind::StemsIn];

    /// What the screen calls it.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Kind::NameIt => "Name it",
            Kind::Backwards => "Backwards",
            Kind::Chipmunk => "Chipmunk",
            Kind::StemsIn => "Stems in",
        }
    }

    /// How long its clip lasts, in seconds of the record.
    #[must_use]
    pub const fn seconds(self) -> f64 {
        match self {
            Kind::StemsIn => STEMS_SECONDS,
            _ => CLIP_SECONDS,
        }
    }
}

/// One question.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Question {
    pub kind: Kind,
    /// The record it is about.
    #[serde(serialize_with = "hex")]
    pub record: TrackId,
    #[serde(skip)]
    pub path: std::path::PathBuf,
    /// *Title — Artist*, four of them, A to D.
    pub answers: [String; 4],
    /// Which of them is right.
    pub right: usize,
    /// Where the clip starts, in seconds of the record.
    pub from: f64,
}

fn hex<S: serde::Serializer>(id: &TrackId, out: S) -> Result<S::Ok, S::Error> {
    out.serialize_str(&id.to_hex())
}

/// A small, seeded source of choices: SplitMix64. A quiz needs variety, not
/// security, and a seed makes every rule here testable.
#[derive(Debug, Clone)]
pub struct Dice(u64);

impl Dice {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// A die seeded from the clock.
    #[must_use]
    pub fn rolled() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5eed);
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// One of `0..n`; zero for an empty range.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        #[allow(clippy::cast_possible_truncation)]
        let pick = (self.next() % n as u64) as usize;
        pick
    }
}

/// The words a record is asked about by — `None` for a record without a
/// title and an artist, which is not asked about: *Unknown — Unknown* is not
/// a question.
#[must_use]
pub fn label(track: &LibraryTrack) -> Option<String> {
    let clean = |text: &Option<String>| {
        text.as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    Some(format!(
        "{} — {}",
        clean(&track.tags.title)?,
        clean(&track.tags.artist)?
    ))
}

fn askable(track: &LibraryTrack) -> bool {
    label(track).is_some() && track.duration_seconds() >= SHORTEST
}

fn family(track: &LibraryTrack) -> Option<&'static str> {
    track
        .tags
        .genre
        .as_deref()
        .and_then(dj_core::genre::family_for)
        .map(|family| family.name)
}

/// The three decoys for `record`: the nearest records in the collection by
/// family and tempo — three drawn from those of its family within a tenth
/// of its tempo when there are three, else from the six nearest — each a
/// different title from the record's and from each other's. `None` when the
/// collection has fewer than three.
#[must_use]
pub fn decoys<'a>(
    record: &LibraryTrack,
    library: &'a [LibraryTrack],
    dice: &mut Dice,
) -> Option<[&'a LibraryTrack; 3]> {
    let title = |track: &LibraryTrack| {
        track
            .tags
            .title
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase()
    };
    let own = title(record);
    let apart = |track: &LibraryTrack| match (record.analysis.bpm, track.analysis.bpm) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (b / a - 1.0).abs(),
        _ => f64::INFINITY,
    };
    let mut near: Vec<&LibraryTrack> = library
        .iter()
        .filter(|track| track.id != record.id && askable(track) && title(track) != own)
        .collect();
    near.sort_by(|a, b| {
        let key = |track: &LibraryTrack| {
            let other_family = family(track) != family(record) || family(record).is_none();
            let far = apart(track);
            (other_family, far > 0.1, far)
        };
        let (fa, ta, da) = key(a);
        let (fb, tb, db) = key(b);
        (fa, ta).cmp(&(fb, tb)).then(da.total_cmp(&db))
    });
    // One of each title among them, so no two answers read the same.
    let mut titles = Vec::new();
    near.retain(|track| {
        let t = title(track);
        let fresh = !titles.contains(&t);
        titles.push(t);
        fresh
    });
    // Draw from the closest tier — same family, within a tenth of the
    // tempo — when it holds three; only short of that does the draw reach
    // for the nearest the collection has beyond it.
    let best = near
        .iter()
        .take_while(|track| {
            family(track).is_some() && family(track) == family(record) && apart(track) <= 0.1
        })
        .count();
    near.truncate(if best >= 3 { best } else { NEAREST.max(3) });
    if near.len() < 3 {
        return None;
    }
    let mut chosen = Vec::with_capacity(3);
    while chosen.len() < 3 {
        chosen.push(near.remove(dice.below(near.len())));
    }
    Some([chosen[0], chosen[1], chosen[2]])
}

/// Where a clip starts: a third of the way in, back to the phrase boundary
/// there when the record has one — where it sounds like itself, past its
/// intro — and early enough that the whole clip, `seconds` long, fits.
#[must_use]
pub fn clip_from(record: &LibraryTrack, seconds: f64) -> f64 {
    let length = record.duration_seconds();
    let third = length / 3.0;
    let latest = (length - seconds).max(0.0);
    let on_phrase = (|| {
        let grid = record.analysis.beatgrid()?;
        let beats = f64::from(record.analysis.phrase_beats?);
        let anchor_beat = f64::from(record.analysis.phrase_anchor.unwrap_or(0));
        let beat = 60.0 / grid.bpm.get();
        let anchor = grid.anchor.get() / record.sample_rate.as_f64();
        let phrase = beats * beat;
        let first = anchor + anchor_beat * beat;
        let phrases = ((third - first) / phrase).floor().max(0.0);
        Some(first + phrases * phrase)
    })();
    on_phrase.unwrap_or(third).clamp(0.0, latest)
}

/// A question of `kind` about a record of `library` not in `asked`, or
/// `None` when no record can be asked about — too few records with titles
/// and artists, or every one asked already.
#[must_use]
pub fn ask(
    kind: Kind,
    library: &[LibraryTrack],
    asked: &[TrackId],
    dice: &mut Dice,
) -> Option<Question> {
    let fresh: Vec<&LibraryTrack> = library
        .iter()
        .filter(|track| askable(track) && !asked.contains(&track.id))
        .collect();
    // A record drawn until one has decoys: a collection of mostly untagged
    // records may hold a few that do not.
    let mut order: Vec<usize> = (0..fresh.len()).collect();
    while !order.is_empty() {
        let record = fresh[order.remove(dice.below(order.len()))];
        let Some(decoys) = decoys(record, library, dice) else {
            continue;
        };
        let right = dice.below(4);
        let mut others = decoys.iter().filter_map(|track| label(track));
        let answers: [String; 4] = std::array::from_fn(|at| {
            if at == right {
                label(record).unwrap_or_default()
            } else {
                others.next().unwrap_or_default()
            }
        });
        return Some(Question {
            kind,
            record: record.id,
            path: record.path.clone(),
            answers,
            right,
            from: clip_from(record, kind.seconds()),
        });
    }
    None
}

/// How many questions a round can be.
pub const COUNTS: [usize; 3] = [5, 10, 15];

/// How long past its expected end a clip is given before it is put back
/// regardless — a deck that never reports its end must not keep the quiz's
/// hands on it.
const GRACE_SECONDS: f64 = 5.0;

/// How the host has a round set.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Settings {
    /// The deck the clips play on, counted from one.
    pub deck: u8,
    /// Which kinds are asked, one drawn per question.
    pub kinds: Vec<Kind>,
    /// How many questions a round is: one of [`COUNTS`].
    pub count: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            deck: 2,
            kinds: Kind::ALL.to_vec(),
            count: 10,
        }
    }
}

/// How the quiz found the deck, so it can be put back so: the pitch fader,
/// keylock, reverse, sync, and which stems were muted. The record on it is not put back — the clip's
/// record replaced it, and a quiz deck is not one the host was playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Found {
    pub pitch: f64,
    pub keylock: bool,
    pub reversed: bool,
    pub synced: bool,
    /// Each stem muted, by [`dj_core::Stem::index`].
    pub muted: [bool; 4],
}

/// A clip on its deck.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub deck: u8,
    pub record: TrackId,
    pub kind: Kind,
    /// Where it starts and ends, in seconds of the record.
    pub from: f64,
    pub to: f64,
    /// The record's own sample rate: a seek is in its frames.
    pub rate: f64,
    pub found: Found,
    /// `None` while the record is still being put on the deck; then when
    /// the clip started, on the pump's clock.
    pub started: Option<f64>,
    /// *Stems in*: when the record was found on the deck and the clip began
    /// waiting for its stems.
    pub waiting: Option<f64>,
    /// *Stems in*: how many of [`STEMS_ORDER`] have been brought in.
    pub brought_in: usize,
}

/// What the pump sees of the quiz's deck.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckView {
    pub track: Option<TrackId>,
    pub playing: bool,
    /// Seconds of the record.
    pub position: f64,
    /// Every stem of the clip's span has been separated.
    pub stems_ready: bool,
}

/// What a step of the clip asks for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Step {
    /// Actions in the ordinary vocabulary, filed as the machine's.
    pub actions: Vec<String>,
    /// The clip is over and its deck put back.
    pub done: bool,
    /// *Stems in*: its stems were not separated in time, and it was not
    /// played.
    pub gave_up: bool,
}

impl Clip {
    /// A clip of `question`, on `deck`, found as `found`.
    #[must_use]
    pub fn of(question: &Question, deck: u8, rate: f64, found: Found) -> Self {
        Self {
            deck,
            record: question.record,
            kind: question.kind,
            from: question.from,
            to: question.from + question.kind.seconds(),
            rate,
            found,
            started: None,
            waiting: None,
            brought_in: 0,
        }
    }

    /// How the clip is played: the pitch it is played at, in the deck's
    /// own fraction, keylock off for a chipmunk, the seek, reverse for
    /// backwards — then play.
    #[must_use]
    pub fn start(&self) -> Vec<String> {
        let deck = self.deck;
        let mut actions = Vec::new();
        if self.found.synced {
            // Sync would hold the record to another deck's tempo, and a
            // chipmunk to nobody's.
            actions.push(format!("deck {deck} sync_off"));
        }
        let pitch = if self.kind == Kind::Chipmunk {
            actions.push(format!("deck {deck} keylock_off"));
            CHIPMUNK_RATE - 1.0
        } else {
            0.0
        };
        actions.push(format!("deck {deck} pitch {pitch}"));
        if self.kind == Kind::StemsIn {
            // The first stem alone; the rest muted until their turn.
            for (n, stem) in STEMS_ORDER.iter().enumerate() {
                let state = if n == 0 { "off" } else { "on" };
                actions.push(format!("deck {deck} stem_mute_{state} {stem}"));
            }
        }
        let backwards = self.kind == Kind::Backwards;
        let at = if backwards { self.to } else { self.from };
        actions.push(format!("deck {deck} seek {:.0}", at * self.rate));
        actions.push(format!(
            "deck {deck} {}",
            if backwards {
                "reverse_on"
            } else {
                "reverse_off"
            }
        ));
        actions.push(format!("deck {deck} play"));
        actions
    }

    /// The deck put back as it was found: paused — unless another record
    /// is on it now, which is the host's — and its pitch, keylock, reverse
    /// and sync as they were.
    #[must_use]
    pub fn put_back(&self, ours: bool) -> Vec<String> {
        let deck = self.deck;
        let found = self.found;
        let mut actions = Vec::new();
        if ours {
            actions.push(format!("deck {deck} pause"));
        }
        actions.push(format!(
            "deck {deck} {}",
            if found.reversed {
                "reverse_on"
            } else {
                "reverse_off"
            }
        ));
        actions.push(format!("deck {deck} pitch {}", found.pitch));
        actions.push(format!(
            "deck {deck} {}",
            if found.keylock {
                "keylock_on"
            } else {
                "keylock_off"
            }
        ));
        if found.synced {
            actions.push(format!("deck {deck} sync"));
        }
        if self.kind == Kind::StemsIn {
            for stem in dj_core::Stem::ALL {
                let state = if found.muted[stem.index()] {
                    "on"
                } else {
                    "off"
                };
                actions.push(format!("deck {deck} stem_mute_{state} {stem}"));
            }
        }
        actions
    }

    /// One step on the pump, at `now` on its clock: started once the record
    /// is on the deck; put back once the clip has played past its end —
    /// back past its start, played backwards — or stopped, or the record
    /// is gone, or it has run well past how long it should take.
    ///
    /// *Stems in* waits, once its record is on the deck, until the stems of
    /// its span are separated — seeking there first, because separation
    /// works outward from the playhead — for [`STEMS_WAIT`] at most; then
    /// brings in a stem every [`STEM_EVERY`].
    pub fn step(&mut self, deck: &DeckView, now: f64) -> Step {
        let ours = deck.track == Some(self.record);
        let Some(started) = self.started else {
            if !ours {
                return Step::default();
            }
            if self.kind == Kind::StemsIn && !deck.stems_ready {
                let Some(since) = self.waiting else {
                    self.waiting = Some(now);
                    let seek = format!("deck {} seek {:.0}", self.deck, self.from * self.rate);
                    return Step {
                        actions: vec![seek],
                        ..Step::default()
                    };
                };
                if now - since > STEMS_WAIT {
                    return Step {
                        actions: self.put_back(ours),
                        done: true,
                        gave_up: true,
                    };
                }
                return Step::default();
            }
            self.started = Some(now);
            self.brought_in = usize::from(self.kind == Kind::StemsIn);
            return Step {
                actions: self.start(),
                ..Step::default()
            };
        };
        let speed = if self.kind == Kind::Chipmunk {
            CHIPMUNK_RATE
        } else {
            1.0
        };
        let elapsed = now - started;
        let past = if self.kind == Kind::Backwards {
            deck.position <= self.from
        } else {
            deck.position >= self.to
        };
        // The deck reports the play a frame or two after it is asked for.
        let stopped = !deck.playing && elapsed > 1.0;
        let late = elapsed > self.kind.seconds() / speed + GRACE_SECONDS;
        if !ours || (past && elapsed > 0.2) || stopped || late {
            return Step {
                actions: self.put_back(ours),
                done: true,
                ..Step::default()
            };
        }
        let mut actions = Vec::new();
        if self.kind == Kind::StemsIn {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let due = ((elapsed / STEM_EVERY).floor() as usize + 1).min(STEMS_ORDER.len());
            while self.brought_in < due {
                let stem = STEMS_ORDER[self.brought_in];
                actions.push(format!("deck {} stem_mute_off {stem}", self.deck));
                self.brought_in += 1;
            }
        }
        Step {
            actions,
            ..Step::default()
        }
    }
}

/// A round: how it is set, what has been asked, the question open, and its
/// clip.
#[derive(Debug, Clone)]
pub struct Round {
    pub settings: Settings,
    pub asked: Vec<TrackId>,
    pub question: Option<Question>,
    pub revealed: bool,
    pub clip: Option<Clip>,
    pub problem: Option<String>,
    pub dice: Dice,
}

impl Default for Round {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            asked: Vec::new(),
            question: None,
            revealed: false,
            clip: None,
            problem: None,
            dice: Dice::rolled(),
        }
    }
}

impl Round {
    /// Whether the round has asked all its questions.
    #[must_use]
    pub fn over(&self) -> bool {
        self.asked.len() >= self.settings.count
    }

    /// The next question, from `library`, of a kind drawn from those set;
    /// kept as the open one. `None` when there is no record left to ask
    /// about.
    pub fn next(&mut self, library: &[LibraryTrack]) -> Option<&Question> {
        self.next_of(library, |_| true)
    }

    /// [`Self::next`], of a kind `can` play — *Stems in* needs a separator.
    /// `None`, too, when none of the kinds set can be played.
    pub fn next_of(
        &mut self,
        library: &[LibraryTrack],
        can: impl Fn(Kind) -> bool,
    ) -> Option<&Question> {
        let set = if self.settings.kinds.is_empty() {
            &Kind::ALL[..]
        } else {
            &self.settings.kinds[..]
        };
        let kinds: Vec<Kind> = set.iter().copied().filter(|kind| can(*kind)).collect();
        if kinds.is_empty() {
            return None;
        }
        let kind = kinds[self.dice.below(kinds.len())];
        let question = ask(kind, library, &self.asked, &mut self.dice)?;
        self.asked.push(question.record);
        self.revealed = false;
        self.question = Some(question);
        self.question.as_ref()
    }

    /// The round over and forgotten; how it was set is kept.
    pub fn end(&mut self) {
        self.asked.clear();
        self.question = None;
        self.revealed = false;
        self.clip = None;
        self.problem = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dj_library::{StoredAnalysis, Tags};

    fn record(byte: u8, title: &str, artist: &str, genre: &str, bpm: f64) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([byte; 32]),
            path: std::path::PathBuf::from(format!("/music/{byte}.flac")),
            tags: Tags {
                title: (!title.is_empty()).then(|| title.to_owned()),
                artist: (!artist.is_empty()).then(|| artist.to_owned()),
                genre: (!genre.is_empty()).then(|| genre.to_owned()),
                ..Tags::default()
            },
            duration_frames: 200 * 48_000,
            sample_rate: dj_core::SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: StoredAnalysis {
                bpm: Some(bpm),
                ..StoredAnalysis::default()
            },
            stats: dj_library::PlayStats::default(),
            colour: None,
        }
    }

    /// A small collection: bachata at 128 and 130, a slow bachata, techno,
    /// a second copy of one title, and records with no title or artist.
    fn library() -> Vec<LibraryTrack> {
        vec![
            record(1, "Obsesión", "Aventura", "Bachata", 128.0),
            record(2, "Propuesta Indecente", "Romeo Santos", "Bachata", 130.0),
            record(3, "Darte un Beso", "Prince Royce", "Bachata", 125.0),
            record(4, "Eres Mía", "Romeo Santos", "Bachata", 124.0),
            record(5, "Bachata Lenta", "Someone", "Bachata", 95.0),
            record(6, "Strobe", "deadmau5", "Techno", 128.0),
            record(7, "Obsesión", "Aventura", "Bachata", 128.0),
            record(8, "", "Nobody", "Bachata", 128.0),
            record(9, "Untitled", "", "Bachata", 128.0),
        ]
    }

    /// **The decoys are the asked record's neighbours.** For a bachata at
    /// 128, every draw is bachata within a tenth of its tempo — never the
    /// techno record at the same tempo, never the slow bachata while nearer
    /// ones are left, never its own title again, never an untitled record.
    #[test]
    fn the_decoys_are_the_records_neighbours() {
        let library = library();
        for seed in 0..50 {
            let mut dice = Dice::new(seed);
            let three = decoys(&library[0], &library, &mut dice).expect("three decoys");
            let mut ids: Vec<u8> = three.iter().map(|t| t.id.as_bytes()[0]).collect();
            ids.sort_unstable();
            assert_eq!(ids, [2, 3, 4], "seed {seed}");
        }
    }

    /// **Too few neighbours, and the next nearest are used** — a family the
    /// collection has one record of draws from the rest by tempo, still
    /// with three different titles.
    #[test]
    fn with_few_neighbours_the_next_nearest_are_used() {
        let library = library();
        let mut dice = Dice::new(7);
        let three = decoys(&library[5], &library, &mut dice).expect("three decoys");
        let titles: Vec<String> = three.iter().filter_map(|t| t.tags.title.clone()).collect();
        assert_eq!(titles.len(), 3);
        assert!(!titles.contains(&"Strobe".to_owned()));
        let unique: std::collections::BTreeSet<&String> = titles.iter().collect();
        assert_eq!(unique.len(), 3, "{titles:?}");
    }

    /// **A question is the record and three decoys, the right answer
    /// anywhere, and nothing is asked twice.** Asked until the collection
    /// runs out, every question's right answer is its record's label, and
    /// no record without a title and artist is ever asked.
    #[test]
    fn a_question_holds_its_record_once_and_runs_out() {
        let library = library();
        let mut dice = Dice::new(42);
        let mut asked = Vec::new();
        let mut places = std::collections::BTreeSet::new();
        while let Some(question) = ask(Kind::NameIt, &library, &asked, &mut dice) {
            let record = library.iter().find(|t| t.id == question.record).unwrap();
            assert_eq!(question.answers[question.right], label(record).unwrap());
            assert!(
                ![8u8, 9].contains(&record.id.as_bytes()[0]),
                "an untitled record was asked"
            );
            let unique: std::collections::BTreeSet<&String> = question.answers.iter().collect();
            assert_eq!(unique.len(), 4, "{:?}", question.answers);
            places.insert(question.right);
            asked.push(question.record);
        }
        assert_eq!(asked.len(), 7, "every titled record, once");
        assert!(places.len() > 1, "the right answer was always in one place");
    }

    /// **The clip starts on a phrase a third of the way in**, or a third of
    /// the way in without phrases, and always fits.
    #[test]
    fn a_clip_starts_on_a_phrase_a_third_in() {
        let mut plain = record(1, "A", "B", "", 120.0);
        assert!((clip_from(&plain, CLIP_SECONDS) - 200.0 / 3.0).abs() < 1e-9);
        plain.analysis.grid_anchor = Some(0.0);
        plain.analysis.grid_beats_per_bar = Some(4);
        plain.analysis.grid_confidence = Some(0.9);
        plain.analysis.phrase_beats = Some(16);
        plain.analysis.phrase_anchor = Some(0);
        // 16 beats at 120 is 8 s: a third of 200 s is 66.7, so 64.
        assert!(
            (clip_from(&plain, CLIP_SECONDS) - 64.0).abs() < 1e-9,
            "{}",
            clip_from(&plain, CLIP_SECONDS)
        );
        let short = LibraryTrack {
            duration_frames: 12 * 48_000,
            ..record(2, "A", "B", "", 120.0)
        };
        assert!(clip_from(&short, CLIP_SECONDS) <= 2.0 + 1e-9);
        let long = LibraryTrack {
            duration_frames: 30 * 48_000,
            ..record(2, "A", "B", "", 120.0)
        };
        assert!(
            clip_from(&long, STEMS_SECONDS) <= 10.0 + 1e-9,
            "a stems clip fits too"
        );
    }

    fn question(kind: Kind) -> Question {
        Question {
            kind,
            record: TrackId::from_bytes([1; 32]),
            path: std::path::PathBuf::new(),
            answers: Default::default(),
            right: 0,
            from: 60.0,
        }
    }

    const FOUND: Found = Found {
        pitch: 0.04,
        keylock: true,
        reversed: false,
        synced: true,
        muted: [false, false, true, false],
    };

    fn on_deck(playing: bool, position: f64) -> DeckView {
        DeckView {
            track: Some(TrackId::from_bytes([1; 32])),
            playing,
            position,
            stems_ready: true,
        }
    }

    /// **Each kind is played as it says**: as recorded from the clip's
    /// start; backwards from its end with reverse on; a chipmunk at 1.5×
    /// with keylock off. Sync is let go of first, so it cannot pull the
    /// tempo back.
    #[test]
    fn each_kind_is_played_as_it_says() {
        let plain = Clip::of(&question(Kind::NameIt), 3, 44_100.0, FOUND).start();
        assert_eq!(
            plain,
            [
                "deck 3 sync_off",
                "deck 3 pitch 0",
                "deck 3 seek 2646000",
                "deck 3 reverse_off",
                "deck 3 play"
            ]
        );
        let back = Clip::of(&question(Kind::Backwards), 3, 44_100.0, FOUND).start();
        assert!(back.contains(&"deck 3 seek 3087000".to_owned()), "{back:?}");
        assert!(back.contains(&"deck 3 reverse_on".to_owned()), "{back:?}");
        let chipmunk = Clip::of(&question(Kind::Chipmunk), 3, 44_100.0, FOUND).start();
        assert!(
            chipmunk.contains(&"deck 3 keylock_off".to_owned()),
            "{chipmunk:?}"
        );
        assert!(
            chipmunk.contains(&"deck 3 pitch 0.5".to_owned()),
            "{chipmunk:?}"
        );
        for actions in [&plain, &back, &chipmunk] {
            for action in actions.iter() {
                dj_core::Action::parse(action).unwrap_or_else(|e| panic!("{action}: {e:?}"));
            }
        }
    }

    /// **The clip waits for its record, plays to its end, and the deck is
    /// put back as it was found** — pitch, keylock, reverse and sync.
    #[test]
    fn a_clip_plays_to_its_end_and_puts_the_deck_back() {
        let mut clip = Clip::of(&question(Kind::NameIt), 2, 48_000.0, FOUND);
        let empty = DeckView {
            track: None,
            playing: false,
            position: 0.0,
            stems_ready: false,
        };
        assert_eq!(
            clip.step(&empty, 0.0),
            Step::default(),
            "started before its record"
        );
        let started = clip.step(&on_deck(false, 0.0), 1.0);
        assert!(!started.done && started.actions.contains(&"deck 2 play".to_owned()));
        assert_eq!(clip.step(&on_deck(true, 60.0), 1.1), Step::default());
        assert_eq!(
            clip.step(&on_deck(true, 65.0), 6.0),
            Step::default(),
            "over mid-clip"
        );
        let over = clip.step(&on_deck(true, 70.01), 11.0);
        assert!(over.done);
        assert_eq!(
            over.actions,
            [
                "deck 2 pause",
                "deck 2 reverse_off",
                "deck 2 pitch 0.04",
                "deck 2 keylock_on",
                "deck 2 sync"
            ]
        );
    }

    /// **Backwards ends at the clip's start; a chipmunk is over sooner;
    /// a record taken off is not paused.**
    #[test]
    fn a_clip_ends_by_its_kind() {
        let mut back = Clip::of(&question(Kind::Backwards), 2, 48_000.0, FOUND);
        back.step(&on_deck(false, 0.0), 0.0);
        assert!(!back.step(&on_deck(true, 65.0), 5.0).done);
        assert!(back.step(&on_deck(true, 59.99), 10.0).done);

        let mut chipmunk = Clip::of(&question(Kind::Chipmunk), 2, 48_000.0, FOUND);
        chipmunk.step(&on_deck(false, 0.0), 0.0);
        assert!(!chipmunk.step(&on_deck(true, 69.0), 6.0).done);
        // A deck that never reports the end is let go of past the grace.
        assert!(chipmunk.step(&on_deck(true, 69.0), 12.0).done);

        let mut gone = Clip::of(&question(Kind::NameIt), 2, 48_000.0, FOUND);
        gone.step(&on_deck(false, 0.0), 0.0);
        let other = DeckView {
            track: Some(TrackId::from_bytes([9; 32])),
            playing: true,
            position: 3.0,
            stems_ready: true,
        };
        let step = gone.step(&other, 2.0);
        assert!(step.done && !step.actions.contains(&"deck 2 pause".to_owned()));
    }

    /// **A round asks its count and no record twice**, of the kinds set.
    #[test]
    fn a_round_asks_the_kinds_set() {
        let library = library();
        let mut round = Round {
            dice: Dice::new(3),
            settings: Settings {
                deck: 2,
                kinds: vec![Kind::Backwards],
                count: 5,
            },
            ..Round::default()
        };
        while !round.over() {
            let kind = round.next(&library).expect("a question").kind;
            assert_eq!(kind, Kind::Backwards);
        }
        assert_eq!(round.asked.len(), 5);
        round.end();
        assert!(round.asked.is_empty() && round.question.is_none());
        assert_eq!(round.settings.count, 5, "how it was set is kept");
    }

    fn waiting_for_stems(position: f64) -> DeckView {
        DeckView {
            stems_ready: false,
            ..on_deck(false, position)
        }
    }

    /// **Stems in waits for its stems**: on finding its record it seeks to
    /// the clip — separation works outward from the playhead — and waits,
    /// asking nothing more, until they are separated; then starts. Too long,
    /// and it gives up and puts the deck back.
    #[test]
    fn stems_in_waits_for_its_stems() {
        let mut clip = Clip::of(&question(Kind::StemsIn), 2, 48_000.0, FOUND);
        assert_eq!(clip.to, 80.0, "twenty seconds");
        let first = clip.step(&waiting_for_stems(0.0), 0.0);
        assert_eq!(first.actions, ["deck 2 seek 2880000"]);
        assert_eq!(clip.step(&waiting_for_stems(60.0), 5.0), Step::default());
        let ready = clip.step(&on_deck(false, 60.0), 8.0);
        assert!(
            ready.actions.contains(&"deck 2 play".to_owned()),
            "{:?}",
            ready.actions
        );

        let mut slow = Clip::of(&question(Kind::StemsIn), 2, 48_000.0, FOUND);
        slow.step(&waiting_for_stems(0.0), 0.0);
        assert!(!slow.step(&waiting_for_stems(60.0), STEMS_WAIT - 1.0).done);
        let gave_up = slow.step(&waiting_for_stems(60.0), STEMS_WAIT + 1.0);
        assert!(gave_up.done && gave_up.gave_up);
        assert!(gave_up.actions.contains(&"deck 2 pause".to_owned()));
    }

    /// **The stems come in drums, bass, the rest, the voice — five seconds
    /// apart — and the deck's mutes are put back as found.**
    #[test]
    fn stems_come_in_one_at_a_time_and_are_put_back() {
        let mut clip = Clip::of(&question(Kind::StemsIn), 2, 48_000.0, FOUND);
        let start = clip.step(&on_deck(false, 0.0), 0.0).actions;
        for (stem, state) in [
            ("drums", "off"),
            ("bass", "on"),
            ("other", "on"),
            ("vocal", "on"),
        ] {
            let action = format!("deck 2 stem_mute_{state} {stem}");
            assert!(start.contains(&action), "{action} not in {start:?}");
        }
        for action in &start {
            dj_core::Action::parse(action).unwrap_or_else(|e| panic!("{action}: {e:?}"));
        }
        assert_eq!(clip.step(&on_deck(true, 64.0), 4.0), Step::default());
        assert_eq!(
            clip.step(&on_deck(true, 65.1), 5.1).actions,
            ["deck 2 stem_mute_off bass"]
        );
        // A pump that missed a beat brings in what is due, in order.
        assert_eq!(
            clip.step(&on_deck(true, 75.5), 15.5).actions,
            ["deck 2 stem_mute_off other", "deck 2 stem_mute_off vocal"]
        );
        assert_eq!(
            clip.step(&on_deck(true, 79.0), 19.0),
            Step::default(),
            "nothing left to bring in"
        );
        let over = clip.step(&on_deck(true, 80.01), 20.1);
        assert!(over.done);
        for (stem, state) in [
            ("vocal", "off"),
            ("drums", "off"),
            ("bass", "on"),
            ("other", "off"),
        ] {
            let action = format!("deck 2 stem_mute_{state} {stem}");
            assert!(
                over.actions.contains(&action),
                "{action} not in {:?}",
                over.actions
            );
        }
    }

    /// **A kind that cannot be played is not asked** — and with none left,
    /// nothing is.
    #[test]
    fn a_kind_that_cannot_be_played_is_not_asked() {
        let library = library();
        let mut round = Round {
            dice: Dice::new(5),
            settings: Settings {
                deck: 2,
                kinds: vec![Kind::NameIt, Kind::StemsIn],
                count: 15,
            },
            ..Round::default()
        };
        for _ in 0..5 {
            let kind = round
                .next_of(&library, |kind| kind != Kind::StemsIn)
                .expect("a question")
                .kind;
            assert_eq!(kind, Kind::NameIt);
        }
        round.settings.kinds = vec![Kind::StemsIn];
        assert!(
            round
                .next_of(&library, |kind| kind != Kind::StemsIn)
                .is_none()
        );
    }
}

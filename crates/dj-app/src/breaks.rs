//! §107: break music between singers.
//!
//! > also investigate and research in online resources what else can be done
//! > to improve karaoke features and usability.
//!
//! The hosting programs a karaoke host already knows (Karaoki's own feature
//! list, in `docs/RESEARCH.md`) all do one thing djmanzo did not: when a
//! singer's song ends, **break music** fades in, and when the next singer's
//! song starts it fades out. Without it the room sits in silence while the
//! host finds the next name, and a karaoke night's silences are where it
//! loses its room.
//!
//! # What it does, and nothing more
//!
//! A plain state machine on the snapshot pump, as the automix is, sending the
//! same actions a host would send by hand — `deck N volume`, `play`, `pause` —
//! so everything it does is visible on the mixer and could be done without
//! it. It plays on **one deck the host names**, from **one playlist the host
//! names**, and:
//!
//! - when nothing else is playing, for a moment, it fades a break record in;
//! - when anything else starts playing — the next singer, or a DJ's own
//!   record — it fades out, pauses, and puts the fader back where it was;
//! - when a break record ends, it asks for the next one.
//!
//! **It only ever plays a record it loaded itself.** A host who puts the next
//! singer's song on the break deck by mistake must not have it started as
//! break music, at a break level, before the singer has the microphone; so
//! the machine keeps which record it loaded, and a deck holding anything else
//! is left alone. For the same reason the fader is put back after every fade
//! out: a paused deck at zero is a singer's song that starts silent.
//!
//! # K5: chosen for the next song
//!
//! Which record comes next is [`Settings::choose`]'s: with *Lead into the
//! next song* on, the record in the playlist that leads best into the song of
//! the singer up next, judged as it will be sung and by the Next rail's own
//! scorer (`dj_library::suggest::score`), so the panel and the rail can never
//! disagree about one pair. See `docs/KARAOKE.md` §8.
//!
//! # Time comes from the music, except for silence
//!
//! Both fades are measured along the break record's own playhead, as the
//! automix measures a transition, so a busy machine stretches nothing. The
//! one clock is for the silence before a break starts, where no playhead is
//! moving: a song ending on a held note is not the room going quiet.

use dj_core::DeckId;
use dj_core::action::{Action, DeckAction};
use dj_library::LibraryTrack;
use dj_library::suggest::{self, Playing, Reason, Trajectory};
use serde::{Deserialize, Serialize};

/// How long nothing must play before break music starts, in seconds.
///
/// Long enough that a song's own last beat and the half second before a host
/// presses play on the next one are not a break; short enough that applause
/// is not followed by a silence nobody chose.
pub const QUIET_SECONDS: f64 = 2.0;

/// The fade in, in seconds of the break record.
pub const FADE_IN_SECONDS: f64 = 3.0;

/// The fade out, in seconds of the break record. Shorter than the fade in:
/// the next singer's intro is already playing under it.
pub const FADE_OUT_SECONDS: f64 = 2.0;

/// How long to wait for a record that was asked for before asking again.
pub const LOAD_PATIENCE_SECONDS: f64 = 10.0;

/// How long a deck asked to play may take to show it before the fade in is
/// given up. The snapshot a tick reads can be a frame behind the action.
pub const START_PATIENCE_SECONDS: f64 = 1.0;

/// The fader levels a host chooses between. Under the room's talk, to fill
/// the gap; a background a room can still hear itself over; or as loud as
/// the singers.
pub const LEVELS: [(f32, &str); 3] = [(0.4, "Quiet"), (0.7, "Background"), (1.0, "Full")];

/// What the host set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub on: bool,
    /// The deck break music plays on, counted from one.
    #[serde(default = "Settings::default_deck")]
    pub deck: u8,
    /// The playlist it plays from.
    #[serde(default)]
    pub playlist: Option<i64>,
    /// Where the fader goes once it has faded in.
    #[serde(default = "Settings::default_level")]
    pub level: f32,
    /// Which entry of the playlist is next, so a night's breaks walk through
    /// it rather than playing its first record every time.
    #[serde(default)]
    pub next: usize,
    /// K5: choose the record that leads into the next singer's song, rather
    /// than the next in the playlist's order. On unless the host turns it off.
    #[serde(default = "Settings::default_lead")]
    pub lead: bool,
    /// K5: the records played tonight, by the library's id, so none is
    /// chosen twice while the playlist has others left. Forgotten at *New
    /// night*, with a new playlist, and once every record has had its turn.
    #[serde(default)]
    pub played: Vec<String>,
}

/// How many played records are remembered. A playlist that lost records
/// while they were remembered would otherwise leave their ids behind for
/// ever; a night of break music is far fewer than this.
pub const MOST_PLAYED: usize = 500;

/// A song break music leads towards: its record, and the semitones it will
/// be sung in from the record's own key.
#[derive(Debug, Clone, Copy)]
pub struct Towards<'a> {
    pub track: &'a LibraryTrack,
    pub key: i32,
}

impl Towards<'_> {
    /// Whether there is anything to rank against: a key or a tempo.
    #[must_use]
    pub fn analysed(&self) -> bool {
        analysed(self.track)
    }

    /// The record as it will be sung: its key moved by the host's semitones,
    /// its tempo its own, since a key change keeps the tempo.
    fn as_sung(&self) -> LibraryTrack {
        let mut track = self.track.clone();
        if let Some(key) = track.analysis.key() {
            let moved = key.transposed(self.key);
            track.analysis.key_hour = Some(moved.hour());
            track.analysis.key_mode = Some(moved.mode());
        }
        track
    }
}

fn analysed(track: &LibraryTrack) -> bool {
    track.analysis.key().is_some() || track.analysis.bpm.is_some()
}

/// The record [`Settings::choose`] chose, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// Its place in the playlist.
    pub at: usize,
    /// The scorer's reasons for the move from it into the first song it was
    /// ranked against; empty when it was the playlist's turn, not a choice.
    pub reasons: Vec<Reason>,
}

impl Settings {
    const fn default_deck() -> u8 {
        2
    }

    const fn default_level() -> f32 {
        0.7
    }

    const fn default_lead() -> bool {
        true
    }

    /// K5: which of `entries`, the playlist in its order, plays next.
    ///
    /// With *lead* on and an analysed song in `towards`, the record that
    /// leads best into it: the scorer's move *from* the record *to* the song,
    /// summed over every song given, at [`Trajectory::Hold`] — a break leaves
    /// the room where the last singer left it. A record played tonight waits
    /// until the rest have had their turn; a record with neither key nor
    /// tempo is ranked last, as the rail ranks it; and ties go to the
    /// playlist's own order, walking on from where it got to, so a playlist
    /// the scorer cannot tell apart plays as it always did.
    ///
    /// Otherwise the walk, exactly as §107: the next in the playlist.
    ///
    /// Moves `next` and `played` on whichever way it chose. `None` for an
    /// empty playlist.
    pub fn choose(&mut self, entries: &[LibraryTrack], towards: &[Towards<'_>]) -> Option<Choice> {
        let len = entries.len();
        if len == 0 {
            return None;
        }
        let songs: Vec<LibraryTrack> = towards
            .iter()
            .filter(|t| t.analysed())
            .map(Towards::as_sung)
            .collect();
        let choice = if self.lead && !songs.is_empty() {
            let unplayed = |track: &LibraryTrack| !self.played.contains(&track.id.to_hex());
            if !entries.iter().any(unplayed) {
                self.played.clear();
            }
            let from = self.next % len;
            let best = (0..len)
                .map(|step| (from + step) % len)
                .filter(|&at| !self.played.contains(&entries[at].id.to_hex()))
                .map(|at| {
                    let record = &entries[at];
                    let (score, reasons) = if analysed(record) {
                        let now = Playing::of(record);
                        let mut total = 0.0;
                        let mut first = None;
                        for song in &songs {
                            let judged = suggest::score(&now, Trajectory::Hold, song);
                            total += judged.score;
                            first.get_or_insert(judged.reasons);
                        }
                        (total, first.unwrap_or_default())
                    } else {
                        (
                            Reason::Unanalysed.weight() * songs.len() as f64,
                            vec![Reason::Unanalysed],
                        )
                    };
                    (at, score, reasons)
                })
                // The first of the highest: `max_by` would keep the last of
                // equals, and the walk's order is what breaks a tie.
                .fold(None::<(usize, f64, Vec<Reason>)>, |best, this| match best {
                    Some(held) if held.1 >= this.1 => Some(held),
                    _ => Some(this),
                })?;
            Choice {
                at: best.0,
                reasons: best.2,
            }
        } else {
            Choice {
                at: self.next % len,
                reasons: Vec::new(),
            }
        };
        self.next = (choice.at + 1) % len;
        self.played.push(entries[choice.at].id.to_hex());
        if self.played.len() > MOST_PLAYED {
            self.played.drain(..self.played.len() - MOST_PLAYED);
        }
        Some(choice)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on: false,
            deck: Self::default_deck(),
            playlist: None,
            level: Self::default_level(),
            next: 0,
            lead: Self::default_lead(),
            played: Vec::new(),
        }
    }
}

/// One deck, as break music sees it: off the same snapshot the interface
/// draws, like the automix's view.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckView {
    pub id: DeckId,
    pub loaded: bool,
    pub playing: bool,
    /// Playhead, in frames.
    pub position: f64,
    /// Length, in frames.
    pub length: f64,
    pub sample_rate: f64,
    /// The channel fader.
    pub volume: f32,
    /// The record on it, by the library's id.
    pub track: Option<String>,
}

impl DeckView {
    /// Played to its end: the deck stops there by itself.
    fn finished(&self) -> bool {
        self.loaded && !self.playing && self.length > 0.0 && self.position >= self.length - 1.0
    }
}

/// Where break music has got to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// Not playing.
    Off,
    /// A record was asked for, at this time.
    Loading { asked: f64 },
    /// Fading in from `from`, a playhead in frames, asked to play at `at`
    /// seconds.
    FadingIn { from: f64, at: f64 },
    /// At its level.
    Playing,
    /// Fading out from `from`, starting at the fader level `at`.
    FadingOut { from: f64, at: f32 },
}

/// Break music as the application holds it: the host's settings, read from
/// disk once rather than on every tick, the machine, and what went wrong last.
#[derive(Debug, Default)]
pub struct Held {
    /// `None` until first read.
    pub settings: Option<Settings>,
    pub machine: Breaks,
    /// Why the last record could not be put on the deck, in words for the
    /// host; cleared by the next one that could.
    pub problem: Option<String>,
    /// K5: what the record on the deck was chosen for, when it was chosen
    /// rather than next in the playlist.
    pub chosen: Option<Chosen>,
}

/// K5: a break record chosen for a song, as the Singers panel says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chosen {
    /// The break record.
    pub title: String,
    /// Whose song it leads into, or out of.
    pub singer: String,
    pub song: String,
    /// True when it leads out of the song just sung, because nobody is
    /// waiting; false when it leads into the next.
    pub after: bool,
    /// The scorer's reasons, as the Next rail writes them.
    pub why: String,
}

/// What one tick asks for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    pub actions: Vec<Action>,
    /// Load the next break record on this deck.
    pub load: Option<DeckId>,
}

impl Plan {
    fn deck(&mut self, deck: DeckId, action: DeckAction) {
        self.actions.push(Action::Deck { deck, action });
    }
}

/// The machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Breaks {
    phase: Phase,
    /// The record break music loaded, and so the only one it will play.
    ours: Option<String>,
    /// Where the fader was before break music took it.
    restore: Option<f32>,
    /// Since when nothing has played, by the tick's clock.
    quiet_since: Option<f64>,
    /// What was on the deck when the next record was asked for: ours, played
    /// out, or nothing. Anything else arriving is the host's.
    was: Option<String>,
}

impl Default for Breaks {
    fn default() -> Self {
        Self::new()
    }
}

impl Breaks {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phase: Phase::Off,
            ours: None,
            restore: None,
            quiet_since: None,
            was: None,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// The record break music put on its deck, by the library's id.
    #[must_use]
    pub fn ours(&self) -> Option<&str> {
        self.ours.as_deref()
    }

    /// The record the host's loader put on the break deck.
    pub fn loaded(&mut self, track: String) {
        self.ours = Some(track);
    }

    /// Stop, giving the fader back to the host.
    fn let_go(&mut self, plan: &mut Plan, deck: DeckId) {
        self.phase = Phase::Off;
        if let Some(level) = self.restore.take() {
            plan.deck(deck, DeckAction::SetVolume(level));
        }
    }

    /// Whether the record on this deck is one break music loaded.
    fn is_ours(&self, deck: &DeckView) -> bool {
        deck.loaded && deck.track.is_some() && deck.track == self.ours
    }

    /// One step, at `now` seconds on any steady clock.
    pub fn tick(&mut self, settings: &Settings, decks: &[DeckView], now: f64) -> Plan {
        let mut plan = Plan::default();
        let Some(deck) =
            DeckId::from_human(settings.deck).and_then(|id| decks.iter().find(|d| d.id == id))
        else {
            self.phase = Phase::Off;
            return plan;
        };
        let others_playing = decks.iter().any(|d| d.id != deck.id && d.playing);
        let frames = |seconds: f64| (seconds * deck.sample_rate).max(1.0);

        // Anything else playing, or the host switching it off, ends a break.
        if !settings.on || others_playing {
            self.quiet_since = None;
            match self.phase {
                Phase::FadingIn { .. } | Phase::Playing if deck.playing && self.is_ours(deck) => {
                    self.phase = Phase::FadingOut {
                        from: deck.position,
                        at: deck.volume,
                    };
                }
                Phase::FadingOut { .. } => {}
                Phase::FadingIn { .. } | Phase::Playing => {
                    self.let_go(&mut plan, deck.id);
                    return plan;
                }
                Phase::Loading { .. } | Phase::Off => {
                    self.phase = Phase::Off;
                    return plan;
                }
            }
        }

        match self.phase {
            Phase::FadingOut { from, at } => {
                let done = ((deck.position - from) / frames(FADE_OUT_SECONDS)).clamp(0.0, 1.0);
                if done >= 1.0 || !deck.playing || !self.is_ours(deck) {
                    if deck.playing && self.is_ours(deck) {
                        plan.deck(deck.id, DeckAction::Pause);
                    }
                    self.let_go(&mut plan, deck.id);
                } else {
                    plan.deck(deck.id, DeckAction::SetVolume(at * (1.0 - done as f32)));
                }
                return plan;
            }
            Phase::FadingIn { from, at } => {
                let starting =
                    !deck.playing && deck.position <= from && now - at < START_PATIENCE_SECONDS;
                if starting && self.is_ours(deck) {
                    return plan;
                }
                if !deck.playing || !self.is_ours(deck) {
                    // Paused by hand, or replaced: the host has the deck.
                    self.let_go(&mut plan, deck.id);
                    return plan;
                }
                let done = ((deck.position - from) / frames(FADE_IN_SECONDS)).clamp(0.0, 1.0);
                plan.deck(deck.id, DeckAction::SetVolume(settings.level * done as f32));
                if done >= 1.0 {
                    self.phase = Phase::Playing;
                }
                return plan;
            }
            Phase::Playing => {
                if deck.playing && self.is_ours(deck) {
                    return plan;
                }
                if !(deck.finished() && self.is_ours(deck)) {
                    // Paused by hand, or replaced: the host has the deck.
                    self.let_go(&mut plan, deck.id);
                    return plan;
                }
                // Played out: straight on to the next, with no second silence.
                self.phase = Phase::Off;
                self.quiet_since = Some(now - QUIET_SECONDS);
            }
            Phase::Loading { asked } => {
                if self.is_ours(deck) && !deck.finished() {
                    // Arrived.
                    self.phase = Phase::Off;
                } else if deck.loaded && deck.track != self.was && deck.track != self.ours {
                    // What arrived is the host's, not ours.
                    self.let_go(&mut plan, deck.id);
                    return plan;
                } else {
                    if now - asked >= LOAD_PATIENCE_SECONDS {
                        self.phase = Phase::Off;
                    }
                    return plan;
                }
            }
            Phase::Off => {}
        }

        // Off: is it a break yet?
        if deck.playing {
            // The break deck playing something by hand is not a break.
            self.quiet_since = None;
            return plan;
        }
        let since = *self.quiet_since.get_or_insert(now);
        if now - since < QUIET_SECONDS {
            return plan;
        }
        if self.is_ours(deck) && !deck.finished() {
            if self.restore.is_none() {
                self.restore = Some(deck.volume);
            }
            plan.deck(deck.id, DeckAction::SetVolume(0.0));
            plan.deck(deck.id, DeckAction::Play);
            self.phase = Phase::FadingIn {
                from: deck.position,
                at: now,
            };
        } else if !deck.loaded || self.is_ours(deck) {
            // Nothing on the deck, or ours played out: the next one. A deck
            // holding a record break music did not load is the host's.
            plan.load = Some(deck.id);
            self.was = deck.track.clone();
            self.phase = Phase::Loading { asked: now };
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn settings() -> Settings {
        Settings {
            on: true,
            playlist: Some(1),
            ..Settings::default()
        }
    }

    fn deck(n: u8) -> DeckView {
        DeckView {
            id: DeckId::from_human(n).unwrap(),
            loaded: false,
            playing: false,
            position: 0.0,
            length: 0.0,
            sample_rate: RATE,
            volume: 0.9,
            track: None,
        }
    }

    fn with(n: u8, track: &str) -> DeckView {
        DeckView {
            loaded: true,
            length: 180.0 * RATE,
            track: Some(track.to_owned()),
            ..deck(n)
        }
    }

    fn volumes(plan: &Plan) -> Vec<f32> {
        plan.actions
            .iter()
            .filter_map(|a| match a {
                Action::Deck {
                    action: DeckAction::SetVolume(v),
                    ..
                } => Some(*v),
                _ => None,
            })
            .collect()
    }

    fn does(plan: &Plan, wanted: &DeckAction) -> bool {
        plan.actions
            .iter()
            .any(|a| matches!(a, Action::Deck { action, .. } if action == wanted))
    }

    /// **A break starts when the room has gone quiet, not the moment a song
    /// stops.** Nothing loaded and nothing playing asks for a record on the
    /// break deck -- after two seconds of quiet, once, and again only when
    /// the first ask has had time to arrive.
    #[test]
    fn a_break_starts_when_the_room_has_gone_quiet() {
        let mut breaks = Breaks::new();
        let decks = [deck(1), deck(2)];
        assert_eq!(breaks.tick(&settings(), &decks, 0.0), Plan::default());
        assert_eq!(breaks.tick(&settings(), &decks, 1.9).load, None);
        assert_eq!(
            breaks.tick(&settings(), &decks, 2.0).load,
            DeckId::from_human(2)
        );
        assert_eq!(
            breaks.tick(&settings(), &decks, 2.1).load,
            None,
            "asked once"
        );
        assert_eq!(breaks.tick(&settings(), &decks, 11.9).load, None);
        assert_eq!(breaks.tick(&settings(), &decks, 12.0), Plan::default());
        assert_eq!(
            breaks.tick(&settings(), &decks, 12.1).load,
            DeckId::from_human(2),
            "and again once it is clear the first is not coming"
        );
        // Switched off, nothing at all.
        let mut off = Breaks::new();
        let quiet = Settings {
            on: false,
            ..settings()
        };
        for now in [0.0, 5.0, 60.0] {
            assert_eq!(off.tick(&quiet, &decks, now), Plan::default());
        }
    }

    /// **It fades in along the record, and out when a singer starts.** The
    /// fade is measured on the break record's playhead; the next song
    /// starting on the other deck fades it out, pauses it, and puts the
    /// fader back where the host had it.
    #[test]
    fn it_fades_in_along_the_record_and_out_when_a_singer_starts() {
        let mut breaks = Breaks::new();
        breaks.loaded("bgm".to_owned());
        let mut decks = [deck(1), with(2, "bgm")];
        breaks.tick(&settings(), &decks, 0.0);
        let start = breaks.tick(&settings(), &decks, 2.0);
        assert_eq!(volumes(&start), [0.0]);
        assert!(does(&start, &DeckAction::Play));

        decks[1].playing = true;
        decks[1].volume = 0.0;
        decks[1].position = 1.5 * RATE;
        let half = breaks.tick(&settings(), &decks, 3.5);
        assert!((volumes(&half)[0] - 0.35).abs() < 1e-3, "{half:?}");
        decks[1].position = 3.0 * RATE;
        assert_eq!(volumes(&breaks.tick(&settings(), &decks, 5.0)), [0.7]);
        assert_eq!(breaks.phase(), Phase::Playing);
        decks[1].volume = 0.7;

        // The next singer.
        decks[0] = DeckView {
            playing: true,
            ..with(1, "song")
        };
        let starts = breaks.tick(&settings(), &decks, 60.0);
        assert_eq!(volumes(&starts), [0.7], "the fade out starts where it is");
        decks[1].position += RATE;
        let fading = breaks.tick(&settings(), &decks, 61.0);
        assert!((volumes(&fading)[0] - 0.35).abs() < 1e-3, "{fading:?}");
        decks[1].position += RATE;
        let out = breaks.tick(&settings(), &decks, 62.0);
        assert!(does(&out, &DeckAction::Pause), "{out:?}");
        assert_eq!(
            volumes(&out),
            [0.9],
            "the fader is put back where the host had it"
        );
        assert_eq!(breaks.phase(), Phase::Off);
    }

    /// **It never plays a record it did not load.** The host's next song put
    /// on the break deck by mistake is not started as break music.
    #[test]
    fn it_never_plays_a_record_it_did_not_load() {
        let mut breaks = Breaks::new();
        breaks.loaded("bgm".to_owned());
        let decks = [deck(1), with(2, "the next singer's song")];
        for now in [0.0, 2.0, 5.0, 30.0] {
            assert_eq!(breaks.tick(&settings(), &decks, now), Plan::default());
        }
    }

    /// When a break record plays to its end the next one is asked for, and
    /// when the host switches break music off mid-record it fades out.
    #[test]
    fn a_record_that_ends_asks_for_the_next_and_off_fades_out() {
        let mut breaks = Breaks::new();
        breaks.loaded("bgm".to_owned());
        let mut decks = [deck(1), with(2, "bgm")];
        breaks.tick(&settings(), &decks, 0.0);
        breaks.tick(&settings(), &decks, 2.0);
        decks[1].playing = true;
        decks[1].position = 3.0 * RATE;
        breaks.tick(&settings(), &decks, 5.0);
        assert_eq!(breaks.phase(), Phase::Playing);

        decks[1].playing = false;
        decks[1].position = decks[1].length;
        let next = breaks.tick(&settings(), &decks, 200.0);
        assert_eq!(next.load, DeckId::from_human(2), "{next:?}");

        let mut breaks = Breaks::new();
        breaks.loaded("bgm".to_owned());
        let mut decks = [deck(1), with(2, "bgm")];
        breaks.tick(&settings(), &decks, 0.0);
        breaks.tick(&settings(), &decks, 2.0);
        decks[1].playing = true;
        decks[1].volume = 0.5;
        let off = Settings {
            on: false,
            ..settings()
        };
        assert_eq!(volumes(&breaks.tick(&off, &decks, 3.0)), [0.5]);
        assert!(matches!(breaks.phase(), Phase::FadingOut { .. }));
    }

    /// K5's choosing, over records with only what the scorer reads.
    mod choosing {
        use super::super::*;
        use dj_core::{Mode, MusicalKey, SampleRate, TrackId};
        use dj_library::{StoredAnalysis, Tags};

        fn record(byte: u8, key: Option<(u8, Mode)>, bpm: Option<f64>) -> LibraryTrack {
            LibraryTrack {
                id: TrackId::from_bytes([byte; 32]),
                path: std::path::PathBuf::from(format!("/music/{byte}.flac")),
                tags: Tags::default(),
                duration_frames: 180 * 48_000,
                sample_rate: SampleRate::DEFAULT,
                channels: 2,
                file_size: None,
                file_modified: None,
                added_at: 0,
                analysis: StoredAnalysis {
                    bpm,
                    key_hour: key.map(|k| k.0),
                    key_mode: key.map(|k| k.1),
                    ..StoredAnalysis::default()
                },
                stats: dj_library::PlayStats::default(),
                colour: None,
            }
        }

        const A_MINOR: (u8, Mode) = (8, Mode::Minor);

        fn leading() -> Settings {
            Settings {
                on: true,
                playlist: Some(1),
                ..Settings::default()
            }
        }

        fn pick(
            settings: &mut Settings,
            entries: &[LibraryTrack],
            towards: &[Towards<'_>],
        ) -> usize {
            settings
                .choose(entries, towards)
                .expect("a playlist with records")
                .at
        }

        /// **The record that leads into the song, not the next in line.**
        /// Last in the playlist and the walk at its top: only the ranking can
        /// have chosen it.
        #[test]
        fn the_record_that_leads_into_the_next_song_is_chosen() {
            let song = record(9, Some(A_MINOR), Some(120.0));
            let entries = [
                record(1, Some((2, Mode::Minor)), Some(120.0)), // a tritone away
                record(2, Some(A_MINOR), Some(90.0)),           // a tempo too far
                record(3, Some(A_MINOR), Some(121.0)),
            ];
            let mut settings = leading();
            let choice = settings
                .choose(
                    &entries,
                    &[Towards {
                        track: &song,
                        key: 0,
                    }],
                )
                .unwrap();
            assert_eq!(choice.at, 2, "the walk's record, not the one that leads in");
            assert!(
                choice
                    .reasons
                    .contains(&Reason::SameKey(MusicalKey::new(8, Mode::Minor).unwrap()))
            );
            assert_eq!(settings.next, 0, "the walk goes on after the record chosen");
        }

        /// **The song as it will be sung.** A song in B minor taken two
        /// semitones down is sung in A minor, and the break record follows it.
        #[test]
        fn the_song_is_judged_in_the_key_the_host_set() {
            let song = record(9, Some((10, Mode::Minor)), Some(120.0)); // B minor
            let entries = [
                record(1, Some((10, Mode::Minor)), Some(120.0)),
                record(2, Some(A_MINOR), Some(120.0)),
            ];
            let as_recorded = pick(
                &mut leading(),
                &entries,
                &[Towards {
                    track: &song,
                    key: 0,
                }],
            );
            let down_two = pick(
                &mut leading(),
                &entries,
                &[Towards {
                    track: &song,
                    key: -2,
                }],
            );
            assert_eq!((as_recorded, down_two), (0, 1));
        }

        /// **Not twice in a night** while the playlist has others; once all
        /// have played, the best again.
        #[test]
        fn a_record_waits_until_the_rest_have_played() {
            let song = record(9, Some(A_MINOR), Some(120.0));
            let entries = [
                record(1, Some((3, Mode::Minor)), Some(120.0)),
                record(2, Some(A_MINOR), Some(120.0)),
                record(3, Some((9, Mode::Minor)), Some(120.0)),
            ];
            let towards = [Towards {
                track: &song,
                key: 0,
            }];
            let mut settings = leading();
            let order: Vec<usize> = (0..4)
                .map(|_| pick(&mut settings, &entries, &towards))
                .collect();
            assert_eq!(order, [1, 2, 0, 1]);
            assert_eq!(
                settings.played,
                [entries[1].id.to_hex()],
                "a new round, not a longer list"
            );
        }

        /// **Ties go to the playlist's order**, walking on from where it got
        /// to — a playlist the scorer cannot tell apart plays as before.
        #[test]
        fn equal_records_play_in_the_playlists_order() {
            let song = record(9, Some(A_MINOR), Some(120.0));
            let entries: Vec<LibraryTrack> = (1..=3)
                .map(|b| record(b, Some(A_MINOR), Some(120.0)))
                .collect();
            let towards = [Towards {
                track: &song,
                key: 0,
            }];
            let mut settings = Settings {
                next: 1,
                ..leading()
            };
            let order: Vec<usize> = (0..3)
                .map(|_| pick(&mut settings, &entries, &towards))
                .collect();
            assert_eq!(order, [1, 2, 0]);
        }

        /// **A record nobody has analysed comes last**, as on the rail — below
        /// even one that fits neither the key nor the tempo, which an empty
        /// analysis would otherwise outscore by having no reasons at all.
        #[test]
        fn an_unanalysed_record_is_ranked_last() {
            let song = record(9, Some(A_MINOR), Some(120.0));
            let entries = [
                record(1, None, None),
                record(2, Some((2, Mode::Minor)), Some(90.0)),
            ];
            let choice = leading()
                .choose(
                    &entries,
                    &[Towards {
                        track: &song,
                        key: 0,
                    }],
                )
                .unwrap();
            assert_eq!(choice.at, 1);
        }

        /// **Nothing to lead towards, or the switch off, and it is §107's
        /// walk**, with no reasons claimed for it.
        #[test]
        fn without_a_song_or_the_switch_it_walks() {
            let analysed = record(9, Some(A_MINOR), Some(120.0));
            let unanalysed = record(8, None, None);
            let entries = [
                record(1, Some((2, Mode::Minor)), Some(90.0)),
                record(2, Some(A_MINOR), Some(120.0)),
            ];
            for (lead, towards) in [
                (true, vec![]),
                (
                    true,
                    vec![Towards {
                        track: &unanalysed,
                        key: 0,
                    }],
                ),
                (
                    false,
                    vec![Towards {
                        track: &analysed,
                        key: 0,
                    }],
                ),
            ] {
                let mut settings = Settings { lead, ..leading() };
                let choice = settings.choose(&entries, &towards).unwrap();
                assert_eq!(
                    choice,
                    Choice {
                        at: 0,
                        reasons: vec![]
                    },
                    "lead {lead}"
                );
                assert_eq!(settings.next, 1);
            }
            assert_eq!(leading().choose(&[], &[]), None);
        }

        /// **Two songs, both heard.** A record mixed in under one song and
        /// leading into the next is ranked against both: the one that fits
        /// both tempos beats one that fits either exactly.
        #[test]
        fn two_songs_are_scored_together() {
            let ending = record(8, Some(A_MINOR), Some(120.0));
            let next = record(9, Some(A_MINOR), Some(112.0));
            let entries = [
                record(1, Some(A_MINOR), Some(120.0)), // the one ending only
                record(2, Some(A_MINOR), Some(112.0)), // the next only
                record(3, Some(A_MINOR), Some(116.0)), // both, within the deck's range
            ];
            let towards = [
                Towards {
                    track: &ending,
                    key: 0,
                },
                Towards {
                    track: &next,
                    key: 0,
                },
            ];
            assert_eq!(pick(&mut leading(), &entries, &towards[..1]), 0);
            assert_eq!(pick(&mut leading(), &entries, &towards[1..]), 1);
            assert_eq!(pick(&mut leading(), &entries, &towards), 2);
        }
    }
}

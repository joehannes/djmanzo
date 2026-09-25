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
//! # Time comes from the music, except for silence
//!
//! Both fades are measured along the break record's own playhead, as the
//! automix measures a transition, so a busy machine stretches nothing. The
//! one clock is for the silence before a break starts, where no playhead is
//! moving: a song ending on a held note is not the room going quiet.

use dj_core::DeckId;
use dj_core::action::{Action, DeckAction};
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
}

impl Settings {
    const fn default_deck() -> u8 {
        2
    }

    const fn default_level() -> f32 {
        0.7
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
}

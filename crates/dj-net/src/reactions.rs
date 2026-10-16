//! Karaoke beta B2: emoji from the room, for the singers' screen.
//!
//! A guest taps one of a few emoji on the room's page; the singers' screen
//! draws it small, in a corner away from the words, and lets it go within
//! seconds. The owner's word was to hold it back so a singer is not
//! distracted, and every number here is that: off until the host turns it
//! on, a handful of glyphs and no text (nothing to moderate), one tap per
//! phone every [`GAP`], a cap on the whole room per second ([`ROOM_PER_SECOND`]),
//! and nothing kept past [`SHOWN`] for the screen to draw.
//!
//! The book is in memory and small whatever the clock does: [`KEPT`]
//! reactions and [`PHONES`] phones at most.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// What a guest can send. Glyphs only: a word from a stranger on the
/// singer's screen would need reading first, and an emoji does not.
pub const EMOJI: [&str; 6] = ["👏", "❤️", "🔥", "😂", "🎤", "🙌"];

/// How long a reaction is worth drawing.
pub const SHOWN: Duration = Duration::from_secs(3);

/// The shortest time between two taps from one phone.
pub const GAP: Duration = Duration::from_secs(2);

/// The most the whole room may send in one second; past it the room is
/// told the screen is busy rather than the screen filling up.
pub const ROOM_PER_SECOND: usize = 6;

/// The most reactions kept.
pub const KEPT: usize = 64;

/// The most phones remembered for [`GAP`].
pub const PHONES: usize = 300;

/// One reaction, as the screen asks for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaction {
    /// Increasing, so the screen can ask for what is newer than it drew.
    pub id: u64,
    /// An index into [`EMOJI`].
    pub emoji: usize,
    pub at: SystemTime,
}

/// What became of a tap, for the page to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reacted {
    Sent,
    /// The host has them off.
    Off,
    /// Not one of [`EMOJI`].
    Unknown,
    /// This phone tapped within [`GAP`].
    TooSoon,
    /// The room has sent [`ROOM_PER_SECOND`] this second already.
    RoomBusy,
}

impl Reacted {
    /// The word in the page's address after a tap.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Off => "off",
            Self::Unknown => "unknown",
            Self::TooSoon => "soon",
            Self::RoomBusy => "busy",
        }
    }

    #[must_use]
    pub fn by_tag(tag: &str) -> Option<Self> {
        [
            Self::Sent,
            Self::Off,
            Self::Unknown,
            Self::TooSoon,
            Self::RoomBusy,
        ]
        .into_iter()
        .find(|reacted| reacted.tag() == tag)
    }
}

/// The room's reactions, and the host's switch.
#[derive(Debug, Default)]
pub struct Reactions {
    on: bool,
    next: u64,
    kept: VecDeque<Reaction>,
    last: HashMap<String, SystemTime>,
}

impl Reactions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn is_on(&self) -> bool {
        self.on
    }

    /// The host's switch. Off forgets what was waiting to be drawn.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            self.kept.clear();
        }
    }

    /// A tap from `phone` on the emoji at `emoji`.
    pub fn react(&mut self, phone: &str, emoji: usize, now: SystemTime) -> Reacted {
        if !self.on {
            return Reacted::Off;
        }
        if emoji >= EMOJI.len() {
            return Reacted::Unknown;
        }
        if self
            .last
            .get(phone)
            .is_some_and(|then| within(*then, now, GAP))
        {
            return Reacted::TooSoon;
        }
        let this_second = self
            .kept
            .iter()
            .filter(|kept| within(kept.at, now, Duration::from_secs(1)))
            .count();
        if this_second >= ROOM_PER_SECOND {
            return Reacted::RoomBusy;
        }
        self.next += 1;
        self.kept.push_back(Reaction {
            id: self.next,
            emoji,
            at: now,
        });
        while self.kept.len() > KEPT {
            self.kept.pop_front();
        }
        self.last.retain(|_, then| within(*then, now, GAP));
        if self.last.len() >= PHONES
            && let Some(oldest) = self
                .last
                .iter()
                .min_by_key(|(_, then)| **then)
                .map(|(phone, _)| phone.clone())
        {
            self.last.remove(&oldest);
        }
        self.last.insert(phone.to_owned(), now);
        Reacted::Sent
    }

    /// What is newer than `after` and still worth drawing at `now`.
    #[must_use]
    pub fn since(&self, after: u64, now: SystemTime) -> Vec<Reaction> {
        self.kept
            .iter()
            .filter(|kept| kept.id > after && within(kept.at, now, SHOWN))
            .copied()
            .collect()
    }
}

/// Whether `then` is less than `span` before `now` — and not after it, which
/// a clock set back can make it.
fn within(then: SystemTime, now: SystemTime, span: Duration) -> bool {
    now.duration_since(then).is_ok_and(|age| age < span)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
    }

    fn on() -> Reactions {
        let mut r = Reactions::new();
        r.set_on(true);
        r
    }

    /// **Off until the host says, and only what can be drawn.**
    #[test]
    fn nothing_is_taken_while_off_or_unknown() {
        let mut r = Reactions::new();
        assert_eq!(r.react("a", 0, at(0)), Reacted::Off);
        assert!(r.since(0, at(0)).is_empty());
        r.set_on(true);
        assert_eq!(r.react("a", EMOJI.len(), at(0)), Reacted::Unknown);
        assert_eq!(r.react("a", 0, at(0)), Reacted::Sent);
        r.set_on(false);
        assert!(r.since(0, at(0)).is_empty(), "off forgets what waited");
    }

    /// **One phone cannot flood it, nor the whole room.**
    #[test]
    fn a_phone_and_the_room_are_held_back() {
        let mut r = on();
        assert_eq!(r.react("a", 0, at(0)), Reacted::Sent);
        assert_eq!(r.react("a", 1, at(1_000)), Reacted::TooSoon);
        assert_eq!(r.react("a", 1, at(2_000)), Reacted::Sent);
        let mut busy = on();
        for phone in 0..ROOM_PER_SECOND {
            assert_eq!(
                busy.react(&format!("p{phone}"), 0, at(5_000)),
                Reacted::Sent
            );
        }
        assert_eq!(busy.react("late", 0, at(5_500)), Reacted::RoomBusy);
        assert_eq!(busy.react("late", 0, at(6_100)), Reacted::Sent);
    }

    /// **The screen is told what is new, and only while it is worth drawing.**
    #[test]
    fn the_screen_gets_what_is_new_and_still_fresh() {
        let mut r = on();
        r.react("a", 0, at(0));
        r.react("b", 1, at(2_100));
        let all = r.since(0, at(2_100));
        assert_eq!(all.len(), 2);
        let newer = r.since(all[0].id, at(2_100));
        assert_eq!(newer.len(), 1);
        assert_eq!(newer[0].emoji, 1);
        // Four seconds after the first, only the second is still worth drawing.
        let late = r.since(0, at(4_000));
        assert_eq!(late.iter().map(|x| x.emoji).collect::<Vec<_>>(), [1]);
        assert!(r.since(0, at(6_000)).is_empty());
    }

    #[test]
    fn the_book_stays_small_whatever_the_clock_does() {
        let mut r = on();
        for i in 0..200u64 {
            r.react(&format!("p{i}"), 0, at(i * 600));
        }
        assert!(r.kept.len() <= KEPT);
        assert!(r.last.len() <= PHONES);
        // A clock set back is not a reaction from the future.
        assert!(r.since(0, at(0)).is_empty());
    }

    #[test]
    fn tags_round_trip() {
        for reacted in [
            Reacted::Sent,
            Reacted::Off,
            Reacted::Unknown,
            Reacted::TooSoon,
            Reacted::RoomBusy,
        ] {
            assert_eq!(Reacted::by_tag(reacted.tag()), Some(reacted));
        }
        assert_eq!(Reacted::by_tag("nonsense"), None);
    }
}

/// The row on the room's page, as the page is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// What became of this phone's last tap, when the page came back from it.
    pub reacted: Option<Reacted>,
}

/// The row's own style, added to the page's head when the row is shown.
pub const ROW_STYLE: &str = "\
.react{display:flex;gap:.4rem;flex-wrap:wrap;margin:.6rem 0}\
.react button{font-size:1.7rem;line-height:1;padding:.45rem .6rem;border-radius:.8rem;border:1px solid #8884;background:#fff1;min-width:3rem}\
.reacted{font-size:.9rem;opacity:.8;margin:.2rem 0 .6rem}";

/// What each glyph is called, for a screen reader — in the page's language.
fn names(tag: &str) -> [&'static str; 6] {
    if tag == "es" {
        [
            "Aplausos",
            "Amor",
            "Fuego",
            "Risa",
            "Micrófono",
            "Manos arriba",
        ]
    } else {
        [
            "Applause",
            "Love",
            "Fire",
            "Laughing",
            "Microphone",
            "Hands up",
        ]
    }
}

/// The row: a heading line, one button per glyph, and what the last tap did.
#[must_use]
pub fn row_html(tag: &str, row: Row) -> String {
    let spanish = tag == "es";
    let mut html =
        String::from("<section aria-labelledby=\"react-title\">\n<p id=\"react-title\">");
    html.push_str(if spanish {
        "Anima a quien canta — aparece pequeño en su pantalla"
    } else {
        "Cheer the singer on — it shows small on their screen"
    });
    html.push_str("</p>\n<form method=\"post\" action=\"/react\" class=\"react\">\n");
    for (index, (glyph, name)) in EMOJI.iter().zip(names(tag)).enumerate() {
        html.push_str(&format!(
            "<button name=\"emoji\" value=\"{index}\" aria-label=\"{name}\">{glyph}</button>\n"
        ));
    }
    html.push_str("</form>\n");
    if let Some(reacted) = row.reacted {
        let said = match (reacted, spanish) {
            (Reacted::Sent, false) => "Sent.",
            (Reacted::Sent, true) => "Enviado.",
            (Reacted::TooSoon, false) => "One moment — a couple of seconds between each.",
            (Reacted::TooSoon, true) => "Un momento — un par de segundos entre cada uno.",
            (Reacted::RoomBusy, false) => "The screen is busy — try again in a second.",
            (Reacted::RoomBusy, true) => "La pantalla está llena — prueba en un segundo.",
            (Reacted::Off | Reacted::Unknown, false) => "Not taken.",
            (Reacted::Off | Reacted::Unknown, true) => "No se tomó.",
        };
        html.push_str("<p class=\"reacted\" role=\"status\">");
        html.push_str(said);
        html.push_str("</p>\n");
    }
    html.push_str("</section>\n");
    html
}

#[cfg(test)]
mod row_tests {
    use super::*;

    /// **The row is every glyph as a button posting its index, named for a
    /// screen reader, and says what the last tap did.**
    #[test]
    fn the_row_posts_each_glyph_and_says_what_happened() {
        let html = row_html(
            "en",
            Row {
                reacted: Some(Reacted::TooSoon),
            },
        );
        assert!(html.contains("action=\"/react\""));
        for (index, glyph) in EMOJI.iter().enumerate() {
            assert!(html.contains(&format!("value=\"{index}\"")), "{index}");
            assert!(html.contains(glyph));
        }
        assert!(html.contains("aria-label=\"Applause\""));
        assert!(html.contains("a couple of seconds"));
        let spanish = row_html("es", Row { reacted: None });
        assert!(spanish.contains("Aplausos") && !spanish.contains("role=\"status\""));
    }
}

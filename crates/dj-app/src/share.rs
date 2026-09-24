//! Handing a set to somebody else.
//!
//! The night ends and three people ask what that fourth record was. A DJ who
//! has to answer that from memory answers it wrong; a DJ who has to export a
//! file, find it, attach it and type a message does not answer it at all.
//!
//! So djmanzo writes the message and the DJ presses send. That division is the
//! whole design: **djmanzo prepares the share, the human sends it.** Nothing
//! here posts anything. It opens a compose window with the text already in it,
//! and what happens next is a decision a person makes with their thumb.
//!
//! # Why the URL is built here and not in the interface
//!
//! The obvious shape is a command that takes a URL and opens it. That hands
//! the webview the ability to ask the operating system to open anything at
//! all, which is a much larger permission than "share this set" and is granted
//! permanently the first time it is convenient.
//!
//! Instead the interface names a *session*, and this module decides what URL
//! that means. The webview cannot reach any other destination because it never
//! gets to say one.
//!
//! # What a tracklist is measured in
//!
//! Times are relative to the first record, not the wall clock. "0:00, 4:12,
//! 8:30" reads as a set; "01:47:23" reads as a server log. Someone scrolling
//! this in a chat wants to know where in the night a record landed, and the
//! answer is "forty minutes in", never "at 01:47".

use dj_library::PlayRecord;

/// The most bytes a `wa.me` link may run to.
///
/// A handoff goes through the operating system's URL handler, and those have
/// limits that are real, undocumented and unequal: Windows' `ShellExecute` has
/// historically refused beyond roughly 2 KB, and other platforms vary. Two
/// thousand is under every limit reported anywhere, which is the right way to
/// pick this number -- a share that silently does nothing on one machine is
/// worse than a share that says it dropped eleven records.
const MAX_URL_BYTES: usize = 2_000;

/// How a shared set is addressed.
///
/// §108 added the three networks after WhatsApp, under the same rule: each is
/// the network's own documented compose address with the text in it, opening
/// a composer the DJ reads, edits and posts from. Telegram is not here
/// because its share link requires a link to share, and a tracklist has none;
/// Instagram and TikTok are not here because neither has a compose address
/// for text. Sources in `docs/RESEARCH.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// WhatsApp's own handoff link. Opens the desktop app when it is
    /// installed and the web client when it is not; both land on a compose
    /// window with the message in it and no recipient chosen.
    WhatsApp,
    /// X's post intent.
    X,
    /// Bluesky's compose intent.
    Bluesky,
    /// Threads' post intent.
    Threads,
}

impl Channel {
    /// Every channel, in the order the panel offers them.
    pub const ALL: [Channel; 4] = [Self::WhatsApp, Self::X, Self::Bluesky, Self::Threads];

    /// The word a command names it by.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::WhatsApp => "whatsapp",
            Self::X => "x",
            Self::Bluesky => "bluesky",
            Self::Threads => "threads",
        }
    }

    /// What the DJ calls it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::WhatsApp => "WhatsApp",
            Self::X => "X",
            Self::Bluesky => "Bluesky",
            Self::Threads => "Threads",
        }
    }

    #[must_use]
    pub fn by_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|channel| channel.slug() == slug)
    }

    /// The link that opens a compose window carrying `message`.
    #[must_use]
    pub fn compose_url(self, message: &str) -> String {
        let text = urlencoding::encode(message);
        match self {
            // No phone number in the path: djmanzo does not know who this is
            // for, and guessing would put a set in front of whoever was
            // messaged last.
            Self::WhatsApp => format!("https://wa.me/?text={text}"),
            Self::X => format!("https://x.com/intent/post?text={text}"),
            Self::Bluesky => format!("https://bsky.app/intent/compose?text={text}"),
            Self::Threads => format!("https://www.threads.net/intent/post?text={text}"),
        }
    }

    /// The longest post the network takes, in its own measure; `None` where
    /// only the link's length limits it.
    #[must_use]
    pub const fn post_limit(self) -> Option<usize> {
        match self {
            Self::WhatsApp => None,
            Self::X => Some(280),
            Self::Bluesky => Some(300),
            Self::Threads => Some(500),
        }
    }

    /// How long `text` is in the network's own measure — never shorter than
    /// the network counts it, so a message that fits here fits there.
    ///
    /// X weighs a character from outside its light ranges (Latin, and most
    /// punctuation) as two, so an emoji or a CJK title costs double; each
    /// code point is weighed alone, which over-counts an emoji sequence and
    /// is the safe direction. Bluesky counts grapheme clusters, of which a
    /// text never has more than it has characters.
    #[must_use]
    pub fn length(self, text: &str) -> usize {
        match self {
            Self::X => text.chars().map(x_weight).sum(),
            _ => text.chars().count(),
        }
    }

    /// Whether `text` can be handed over whole: the link under what a URL
    /// handler takes, and the post under the network's limit.
    #[must_use]
    pub fn fits(self, text: &str) -> bool {
        self.compose_url(text).len() <= MAX_URL_BYTES
            && self
                .post_limit()
                .is_none_or(|limit| self.length(text) <= limit)
    }
}

/// One character's weight in X's count: the light ranges of its published
/// counting configuration weigh one, everything else two.
fn x_weight(c: char) -> usize {
    let point = u32::from(c);
    let light = [(0, 4_351), (8_192, 8_205), (8_208, 8_223), (8_242, 8_247)];
    if light.iter().any(|&(from, to)| (from..=to).contains(&point)) {
        1
    } else {
        2
    }
}

/// How a set is written out.
#[derive(Debug, Clone)]
pub struct Style {
    /// Shown above the list. Empty for none.
    pub heading: String,
    /// Prefix each record with where in the set it played.
    pub timestamps: bool,
    /// Stop before the link grows past what a URL handler will take.
    pub limit_for_url: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            heading: String::new(),
            timestamps: true,
            limit_for_url: false,
        }
    }
}

/// One line of a tracklist, already resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Seconds after the first record of the set.
    pub at: i64,
    pub artist: String,
    pub title: String,
}

/// Turn what the library recorded into lines measured from the set's start.
///
/// The library stores wall-clock unix seconds because that is what a play
/// *is*; a tracklist wants elapsed time. Converting here rather than at the
/// query keeps the history table answering the question it was built for.
#[must_use]
pub fn entries(plays: &[PlayRecord]) -> Vec<Entry> {
    let Some(start) = plays.iter().map(|p| p.played_at).min() else {
        return Vec::new();
    };
    plays
        .iter()
        .map(|p| Entry {
            // No clamp: `start` is the smallest `played_at` in the slice, so
            // this difference cannot be negative. A guard here would look like
            // a safeguard against a clock stepping backwards and would in fact
            // be unreachable -- and an unreachable guard is worse than none,
            // because it stops anyone looking for the real one.
            //
            // A clock that does step back is still visible, as a record whose
            // offset is *smaller* than the one before it. That is the library's
            // ordering showing through honestly, and clamping would hide it
            // rather than fix it.
            at: p.played_at - start,
            artist: p.artist.clone(),
            title: p.title.clone(),
        })
        .collect()
}

/// `m:ss`, or `h:mm:ss` once a set runs past the hour.
///
/// Not zero-padded at the front: `4:12` is how a person says it, and `04:12`
/// is how a machine does.
#[must_use]
pub(crate) fn clock(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let (h, m, s) = (seconds / 3_600, (seconds / 60) % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A set, written out for a human to read in a chat window.
///
/// Plain text on purpose. WhatsApp reads `*` and `_` as formatting and offers
/// no way to escape them, so djmanzo adds none of its own -- a heading in
/// asterisks would render as bold until the night a record called `*NSYNC`
/// turned half the list italic.
#[must_use]
pub fn message(entries: &[Entry], style: &Style) -> String {
    let (text, _) = message_and_dropped(entries, style);
    text
}

/// The message, and how many records did not fit.
///
/// Returned together because the caller has to tell the DJ. A share that
/// quietly loses the last third of the night is worse than no share: the DJ
/// finds out when somebody asks about a record that is not on the list.
#[must_use]
pub fn message_and_dropped(entries: &[Entry], style: &Style) -> (String, usize) {
    message_for(entries, style, Channel::WhatsApp)
}

/// The message for one channel, and how many records did not fit it.
///
/// A post on X holds a handful of records and a WhatsApp link a hundred or
/// so, so what fits is the channel's to say, and the count of what did not
/// is said in the message whichever it is.
#[must_use]
pub fn message_for(entries: &[Entry], style: &Style, channel: Channel) -> (String, usize) {
    let head = if style.heading.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", style.heading)
    };

    let line = |e: &Entry| {
        if style.timestamps {
            format!("{}  {} - {}", clock(e.at), e.artist, e.title)
        } else {
            format!("{} - {}", e.artist, e.title)
        }
    };

    if !style.limit_for_url {
        let body = entries.iter().map(line).collect::<Vec<_>>().join("\n");
        return (format!("{head}{body}"), 0);
    }

    // Grow the list one record at a time and stop at the last one whose
    // *encoded* length still fits. Measuring the encoded form is the point:
    // accented Spanish -- and this is a Caribbean repertoire, so it is most of
    // it -- costs nine bytes per character once percent-encoded, and counting
    // characters would let a list through that the URL handler then refuses.
    let mut kept: Vec<String> = Vec::new();
    let mut dropped = 0;
    for e in entries {
        let mut candidate = kept.clone();
        candidate.push(line(e));
        let text = format!("{head}{}", candidate.join("\n"));
        if !channel.fits(&text) {
            dropped += 1;
        } else {
            kept = candidate;
        }
    }

    let mut text = format!("{head}{}", kept.join("\n"));
    if dropped > 0 {
        // Said in the message itself, not only in the interface, because the
        // person reading it in the chat is the one who will otherwise assume
        // the night ended here.
        let tail = format!("\n\n(+{dropped} more)");
        // Trimming from the kept list rather than exceeding the budget: the
        // note is what makes the truncation honest, so it is the one part
        // that may not be dropped to make room for a record.
        while !kept.is_empty() && !channel.fits(&format!("{head}{}{tail}", kept.join("\n"))) {
            kept.pop();
            dropped += 1;
        }
        let tail = format!("\n\n(+{dropped} more)");
        text = format!("{head}{}{tail}", kept.join("\n"));
    }
    (text, dropped)
}

/// The full set as a file, for when it is too long to send as a message.
///
/// The escape hatch from `MAX_URL_BYTES`: a four-hour set does not fit in a
/// link and there is no version of this feature where that becomes acceptable
/// to silently discard.
#[must_use]
pub fn as_file(entries: &[Entry], heading: &str) -> String {
    message(
        entries,
        &Style {
            heading: heading.to_string(),
            timestamps: true,
            limit_for_url: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(at: i64, artist: &str, title: &str) -> PlayRecord {
        PlayRecord {
            track_id: format!("{at}"),
            title: title.into(),
            artist: artist.into(),
            played_at: at,
            session_id: Some("s".into()),
        }
    }

    /// **A tracklist is measured from the first record, not from midnight.**
    #[test]
    fn times_run_from_the_start_of_the_set() {
        let plays = vec![
            play(1_700_000_000, "Aventura", "Obsesion"),
            play(1_700_000_252, "Romeo Santos", "Propuesta Indecente"),
        ];
        let out = entries(&plays);
        assert_eq!(out[0].at, 0);
        assert_eq!(out[1].at, 252);
    }

    /// **Each network gets its own compose address with the words in it**,
    /// https on its own domain, the text encoded so a title cannot add a
    /// parameter of its own.
    #[test]
    fn every_channel_opens_its_own_composer() {
        let mut slugs = std::collections::BTreeSet::new();
        for channel in Channel::ALL {
            assert!(slugs.insert(channel.slug()));
            assert_eq!(Channel::by_slug(channel.slug()), Some(channel));
            let url = channel.compose_url("Noche & día #1");
            assert!(url.starts_with("https://"), "{url}");
            assert!(url.ends_with("text=Noche%20%26%20d%C3%ADa%20%231"), "{url}");
        }
        assert!(
            Channel::X
                .compose_url("")
                .starts_with("https://x.com/intent/post?")
        );
        assert!(
            Channel::Bluesky
                .compose_url("")
                .starts_with("https://bsky.app/intent/compose?")
        );
        assert!(
            Channel::Threads
                .compose_url("")
                .starts_with("https://www.threads.net/intent/post?")
        );
        assert_eq!(Channel::by_slug("telegram"), None);
    }

    /// X counts a Latin letter as one and an emoji or a CJK character as two.
    #[test]
    fn x_weighs_characters_as_x_does() {
        assert_eq!(Channel::X.length("Añoranza"), 8);
        assert_eq!(Channel::X.length("🔥"), 2);
        assert_eq!(Channel::X.length("東京"), 4);
        assert_eq!(Channel::Bluesky.length("東京🔥"), 3);
    }

    /// **A post never exceeds its network's limit, and says what it left
    /// out** — swept across set sizes, accented, on every channel, because a
    /// post that is one character over is a composer that will not post.
    #[test]
    fn every_channel_gets_a_post_that_fits_it() {
        let style = Style {
            heading: "Sábado en el Jumbo 🔥".into(),
            limit_for_url: true,
            ..Style::default()
        };
        for channel in Channel::ALL {
            for n in 1..60 {
                let plays: Vec<_> = (0..n)
                    .map(|i| play(i * 200, "Añoranza Corazón", "Canción Íntima"))
                    .collect();
                let (text, dropped) = message_for(&entries(&plays), &style, channel);
                // Measured here, not by `fits`, so a limit that went missing
                // from the channel's own table is caught rather than agreed with.
                let limit = match channel {
                    Channel::X => Some(280),
                    Channel::Bluesky => Some(300),
                    Channel::Threads => Some(500),
                    Channel::WhatsApp => None,
                };
                if let Some(limit) = limit {
                    // The heading's flame is two on X; every other character one.
                    let measured = text.chars().count()
                        + usize::from(channel == Channel::X) * text.matches('🔥').count();
                    assert!(
                        measured <= limit,
                        "{} at {n}: {measured} > {limit}",
                        channel.name()
                    );
                }
                assert!(
                    channel.compose_url(&text).len() <= MAX_URL_BYTES,
                    "{} at {n}",
                    channel.name()
                );
                let kept = text.lines().filter(|l| l.contains(" - ")).count();
                assert_eq!(kept + dropped, n as usize, "{} at {n}", channel.name());
                if dropped > 0 {
                    assert!(text.ends_with(&format!("(+{dropped} more)")), "{text}");
                }
            }
        }
        // And across heading lengths, so the room left after the last record
        // that fits takes every size — including too little for the note
        // saying what was left out, which must then cost a record.
        for pad in 0..48 {
            let style = Style {
                heading: format!("Sábado {}", "x".repeat(pad)),
                limit_for_url: true,
                ..Style::default()
            };
            let plays: Vec<_> = (0..20)
                .map(|i| play(i * 200, "Añoranza Corazón", "Canción Íntima"))
                .collect();
            for (channel, limit) in [(Channel::X, 280), (Channel::Bluesky, 300)] {
                let (text, dropped) = message_for(&entries(&plays), &style, channel);
                assert!(
                    text.chars().count() <= limit,
                    "{} with a heading of {pad}: {} > {limit}",
                    channel.name(),
                    text.chars().count()
                );
                let kept = text.lines().filter(|l| l.contains(" - ")).count();
                assert_eq!(kept + dropped, 20);
            }
        }

        // The limits are the networks': X holds a handful, WhatsApp dozens.
        let plays: Vec<_> = (0..40)
            .map(|i| play(i * 200, "Añoranza Corazón", "Canción Íntima"))
            .collect();
        let kept = |channel| 40 - message_for(&entries(&plays), &style, channel).1;
        // Twenty characters apart, which is less than a line.
        assert!(kept(Channel::X) <= kept(Channel::Bluesky));
        assert!(kept(Channel::Bluesky) < kept(Channel::Threads));
        assert!(kept(Channel::Threads) < kept(Channel::WhatsApp));
        assert!(
            kept(Channel::X) >= 3,
            "an X post should still hold a few records"
        );
    }

    /// **Zero is the earliest record, whatever order they arrive in.**
    ///
    /// The library orders by `played_at`, but a clock that steps backwards --
    /// NTP correcting, a laptop waking -- can put a later play at an earlier
    /// timestamp. The set still starts at the earliest of them, so no offset
    /// is negative and none is invented.
    #[test]
    fn the_set_starts_at_the_earliest_record_not_the_first_row() {
        let plays = vec![
            play(1_700_000_100, "El Gran Combo", "Un Verano en Nueva York"),
            play(1_700_000_000, "Juan Luis Guerra", "Bachata Rosa"),
        ];
        let out = entries(&plays);
        assert_eq!(out[0].at, 100, "the row that came first is 100s in");
        assert_eq!(out[1].at, 0, "and the earliest record is the zero point");
    }

    #[test]
    fn a_set_past_the_hour_prints_hours() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(252), "4:12");
        assert_eq!(clock(3_600), "1:00:00");
        assert_eq!(clock(3_661), "1:01:01");
    }

    /// **The message is what a person would have typed.**
    #[test]
    fn a_short_set_is_written_out_whole() {
        let plays = vec![
            play(0, "Aventura", "Obsesion"),
            play(252, "Romeo Santos", "Propuesta Indecente"),
        ];
        let style = Style {
            heading: "Saturday at Jumbo".into(),
            ..Style::default()
        };
        let (text, dropped) = message_and_dropped(&entries(&plays), &style);
        assert_eq!(dropped, 0);
        assert_eq!(
            text,
            "Saturday at Jumbo\n\n0:00  Aventura - Obsesion\n4:12  Romeo Santos - Propuesta Indecente"
        );
    }

    /// **djmanzo adds no formatting characters of its own.**
    ///
    /// WhatsApp reads `*` and `_` as bold and italic and has no escape. Any
    /// decoration djmanzo added would be one unbalanced marker in a track
    /// title away from reformatting the rest of the list.
    #[test]
    fn nothing_in_the_message_is_markup() {
        let plays = vec![play(0, "Aventura", "Obsesion")];
        let style = Style {
            heading: "Saturday".into(),
            ..Style::default()
        };
        let text = message(&entries(&plays), &style);
        assert!(!text.contains('*'), "{text}");
        assert!(!text.contains('_'), "{text}");
    }

    /// **A long set is cut to fit, and says that it was.**
    ///
    /// The failure this prevents is the quiet one: a DJ shares four hours,
    /// the link carries ninety minutes, and nobody finds out until somebody
    /// asks about a record that is not on the list.
    #[test]
    fn a_set_too_long_for_a_link_is_cut_and_says_so() {
        let plays: Vec<_> = (0..400)
            .map(|i| {
                play(
                    i * 200,
                    "Un Artista Con Nombre Bastante Largo",
                    "Una Cancion Con Un Titulo Igualmente Largo",
                )
            })
            .collect();
        let style = Style {
            heading: "Long night".into(),
            limit_for_url: true,
            ..Style::default()
        };
        let (text, dropped) = message_and_dropped(&entries(&plays), &style);
        assert!(dropped > 0, "a 400-record set must not fit in a link");
        assert!(text.contains(&format!("(+{dropped} more)")), "{text}");
        assert!(
            Channel::WhatsApp.compose_url(&text).len() <= MAX_URL_BYTES,
            "url was {} bytes",
            Channel::WhatsApp.compose_url(&text).len()
        );
        // The count has to be the truth, not a lower bound.
        let kept = text.lines().filter(|l| l.contains(" - ")).count();
        assert_eq!(
            kept + dropped,
            400,
            "every record is either kept or counted"
        );
    }

    /// **The budget is bytes after encoding, not characters before it.**
    ///
    /// Accented Spanish is most of this repertoire, and one accented character
    /// costs nine bytes once percent-encoded. Counting characters would pass a
    /// list the URL handler then refuses -- and it would fail on exactly the
    /// sets djmanzo is built for.
    #[test]
    fn accented_titles_are_budgeted_by_their_encoded_size() {
        let plays: Vec<_> = (0..400)
            .map(|i| play(i * 200, "Añoranza Corazón", "Canción Íntima Ñoña"))
            .collect();
        let style = Style {
            limit_for_url: true,
            ..Style::default()
        };
        let (text, dropped) = message_and_dropped(&entries(&plays), &style);
        assert!(dropped > 0);
        let url = Channel::WhatsApp.compose_url(&text);
        assert!(url.len() <= MAX_URL_BYTES, "url was {} bytes", url.len());
        // And the reason, stated as the mistake it prevents: a budget counted
        // in characters would have admitted a list the URL handler refuses.
        // `chars` here is deliberate -- `len` on a Rust `String` is already
        // bytes, so writing this against `len` would prove nothing.
        assert!(
            text.chars().count() < url.len(),
            "characters {} should be well under encoded bytes {}",
            text.chars().count(),
            url.len()
        );
        let by_characters = text.chars().count();
        assert!(
            by_characters * 3 / 2 < MAX_URL_BYTES,
            "a character budget would have left room for {} more, which the \
             handler would then refuse",
            MAX_URL_BYTES - by_characters
        );
    }

    /// **Every message that claims to fit, fits.**
    ///
    /// The single-size test above could not see a budget counted in the wrong
    /// unit, because when records *are* dropped a later pass re-measures in
    /// bytes and quietly corrects it. The gap is the sizes where nothing is
    /// dropped: there the first measurement is the only one, and a character
    /// count would wave through a link the handler refuses.
    ///
    /// So sweep the sizes rather than picking one. Accented, because that is
    /// where characters and bytes disagree, and because that is this
    /// repertoire.
    #[test]
    fn no_set_size_produces_a_link_that_is_too_long() {
        let style = Style {
            heading: "Noche".into(),
            limit_for_url: true,
            ..Style::default()
        };
        for n in 1..120 {
            let plays: Vec<_> = (0..n)
                .map(|i| play(i * 200, "Añoranza Corazón", "Canción Íntima Ñoña"))
                .collect();
            let (text, _) = message_and_dropped(&entries(&plays), &style);
            let url = Channel::WhatsApp.compose_url(&text);
            assert!(
                url.len() <= MAX_URL_BYTES,
                "{n} records produced a {}-byte link",
                url.len()
            );
        }
    }

    /// **A file has no length limit, so it loses nothing.**
    #[test]
    fn the_file_form_keeps_every_record() {
        let plays: Vec<_> = (0..400)
            .map(|i| play(i * 200, "Artista", "Cancion"))
            .collect();
        let text = as_file(&entries(&plays), "Long night");
        assert_eq!(text.lines().filter(|l| l.contains(" - ")).count(), 400);
    }

    /// **The link carries a message and no recipient.**
    ///
    /// djmanzo does not know who this is for. A number in the path would send
    /// a set to whoever was messaged last.
    #[test]
    fn the_link_names_no_recipient() {
        let url = Channel::WhatsApp.compose_url("hola");
        assert_eq!(url, "https://wa.me/?text=hola");
    }

    /// **The separators survive encoding.**
    ///
    /// A newline, an ampersand and a `#` are all structural in a URL. Left
    /// raw, the message would be cut off at the first one -- and `&` turns up
    /// in artist names constantly.
    #[test]
    fn characters_that_would_break_a_url_are_encoded() {
        let url = Channel::WhatsApp.compose_url("a\nb & c #d");
        assert!(!url.contains('\n'), "{url}");
        assert!(!url.contains(" & "), "{url}");
        assert!(!url.contains('#'), "{url}");
        assert!(url.contains("%0A"), "{url}");
    }

    #[test]
    fn an_empty_set_shares_nothing_rather_than_crashing() {
        assert!(entries(&[]).is_empty());
        assert_eq!(message(&[], &Style::default()), "");
    }
}

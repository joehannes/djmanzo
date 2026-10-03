//! §123: the karaoke journal — who sang, what they sang, and what they agreed
//! to.
//!
//! > the current karaoke singer/guest/guest artist shall be remembered in some
//! > kind of useful and versatile and rich karaoke journal ... minimal
//! > requirements: name of the guest artist, song sung, age, email, social
//! > profile(s), WA/phone nr, home country/town, nationality, favorite band,
//! > favorite musical genre, favorite song, native language, additional
//! > languages
//!
//! §107's rotation ([`crate::karaoke`]) is tonight's queue and a history of
//! who sang what in which key. This is the other half: the **guests**, kept
//! across nights, each with the songs they sang where and when — and nothing
//! about a guest kept that they did not agree to.
//!
//! # What a guest agrees to, and what follows from each
//!
//! Three separate questions, asked each time and recorded with when they were
//! answered ([`Consent`]), because each is a different use and a guest may
//! want one without the others:
//!
//! - **keep** — the record outlives tonight. Without it the guest is in the
//!   journal for this night only and goes when the host starts a new one
//!   ([`Journal::new_night`]): tonight's song for them can still be made.
//! - **contact** — the email, the phone or WhatsApp number and the social
//!   profiles. Without it they are not kept at all, not merely hidden:
//!   [`Journal::save`] clears them whatever the form sent.
//! - **voice** — about fifteen seconds of their singing recorded, and given
//!   to an AI music service to make a song with. Without it no recording is
//!   attached, and taking it back deletes the ones there are.
//!
//! **A child cannot agree alone.** Under [`CONSENT_AGE`] — sixteen, the age
//! the GDPR sets unless a country lowers it — contact and voice are refused:
//! that needs a parent, and a DJ booth is not where a parent's consent is
//! checked.
//!
//! # Local, and theirs
//!
//! The journal is one file in djmanzo's settings folder and the recordings a
//! folder beside it; nothing here sends anything anywhere. A guest's record
//! can be exported for them ([`Journal::export_guest`], the GDPR's right to a
//! copy) and deleted with everything recorded of them ([`Journal::forget`]).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How long a voice take is: the owner's "about 15 secs".
pub const VOICE_SECONDS: u8 = 15;

/// Under this age a guest cannot agree to contact or to their voice being
/// used on their own.
pub const CONSENT_AGE: u8 = 16;

/// The version of the sentences in [`ASKS`]. Stamped on every consent, so a
/// record says which wording its guest agreed to if the wording ever changes.
pub const WORDING: u32 = 1;

/// The three questions, exactly as the guest is asked them: the interface
/// shows these and nothing else, so what was agreed is what is written here.
pub const ASKS: [(&str, &str); 3] = [
    (
        "keep",
        "Keep my details after tonight, so the DJ remembers me next time.",
    ),
    (
        "contact",
        "Keep my email, phone or WhatsApp and social profiles, and send me what is made of tonight.",
    ),
    (
        "voice",
        "Record about fifteen seconds of my singing and use it with an AI music service to make a song for me.",
    ),
];

/// What a guest agreed to, and when.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Consent {
    pub keep: bool,
    pub contact: bool,
    pub voice: bool,
    /// When they answered, in seconds since 1970. Zero is never.
    pub given: i64,
    /// Which wording of [`ASKS`] they answered.
    pub wording: u32,
}

/// One song a guest sang.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Performance {
    pub title: String,
    /// The record in the collection, as hex, when it was found.
    pub track: Option<String>,
    /// Semitones from the record's own key.
    pub key: i32,
    /// When, in seconds since 1970.
    pub at: i64,
    /// §118's event it was sung at, and where that was, when one was being
    /// played.
    pub event: String,
    pub place: String,
    /// The recording of their voice, a file name in the journal's folder —
    /// only with their consent.
    pub voice: Option<String>,
    /// §123: the words written for a song of their own, and its style — only
    /// with their consent to a song being made for them.
    pub song: Option<dj_assistant::SongDraft>,
}

/// A guest who sang.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Guest {
    /// Stable: made when the guest is first written down.
    pub id: String,
    pub name: String,
    pub age: Option<u8>,
    pub email: String,
    /// A phone or WhatsApp number, as they gave it.
    pub phone: String,
    pub socials: Vec<String>,
    /// Home town and country, as they said it.
    pub home: String,
    pub nationality: String,
    pub favourite_band: String,
    pub favourite_genre: String,
    pub favourite_song: String,
    pub native_language: String,
    pub languages: Vec<String>,
    /// The host's own note.
    pub notes: String,
    pub consent: Consent,
    /// When they were first written down, in seconds since 1970.
    pub since: i64,
    /// Every song, oldest first.
    pub sang: Vec<Performance>,
    /// A recording of their voice made while they are singing, before the
    /// song is marked as sung; it goes onto that song when it is.
    pub take: Option<String>,
    /// K3: the microphone chain they sounded best through — the compressor
    /// and the room the host found for them — laid over a strip when they
    /// are put on one (docs/KARAOKE.md §6). Settings for a voice with a name
    /// on them are about a person, so they are kept here only with
    /// [`Consent::keep`]; without it they are kept on the guest's place in
    /// tonight's rotation ([`crate::karaoke::Singer::microphone`]) and go
    /// with it.
    pub microphone: Option<dj_vocal::Chain>,
}

impl Guest {
    /// Whether they are old enough to agree to contact and voice alone. An age
    /// not given is taken as old enough: the host is asked to write it down
    /// when it matters, not to card every singer.
    #[must_use]
    pub fn may_consent(&self) -> bool {
        self.age.is_none_or(|age| age >= CONSENT_AGE)
    }
}

/// §123: what the model is told for a guest's song — their name, their last
/// song and where they sang it, their favourites and languages, and the DJ's
/// ideas. Their email, phone, age, home and nationality are not in it: a
/// song does not need them, and they are not the model's to see.
///
/// `None` for a guest who has not agreed to a song being made for them —
/// the third question of [`ASKS`] — or has not sung yet.
#[must_use]
pub fn brief(
    guest: &Guest,
    song_language: &str,
    keywords: &str,
    date: &str,
    evening: Vec<String>,
) -> Option<dj_assistant::SongBrief> {
    if !guest.consent.voice || !guest.may_consent() {
        return None;
    }
    let last = guest.sang.last()?;
    Some(dj_assistant::SongBrief {
        singer: guest.name.clone(),
        sang: last.title.clone(),
        date: date.trim().to_owned(),
        event: last.event.clone(),
        place: last.place.clone(),
        evening,
        favourite_band: guest.favourite_band.clone(),
        favourite_genre: guest.favourite_genre.clone(),
        favourite_song: guest.favourite_song.clone(),
        keywords: keywords.trim().to_owned(),
        languages: dj_assistant::song::languages(
            song_language,
            &last.title,
            &guest.native_language,
        ),
    })
}

/// §123: how the song reaches a guest — a message the DJ sends themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    WhatsApp,
    Mail,
}

impl Reach {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "whatsapp" => Some(Self::WhatsApp),
            "mail" => Some(Self::Mail),
            _ => None,
        }
    }
}

/// The message that goes with a guest's song: their name, what they sang
/// and where, and the link the DJ pastes. Written in English; the DJ can
/// change it in WhatsApp or the mail before sending.
#[must_use]
pub fn message(guest: &Guest, link: &str) -> String {
    let mut text = format!("Hi {}! Thank you for singing", guest.name.trim());
    if let Some(last) = guest.sang.last() {
        text.push_str(&format!(" \"{}\"", last.title.trim()));
        if !last.event.trim().is_empty() {
            text.push_str(&format!(" at {}", last.event.trim()));
        } else if !last.place.trim().is_empty() {
            text.push_str(&format!(" at {}", last.place.trim()));
        }
    }
    text.push_str(". Here is a song made for you from tonight");
    let link = link.trim();
    if link.is_empty() {
        text.push('.');
    } else {
        text.push_str(": ");
        text.push_str(link);
    }
    text
}

/// The address that opens WhatsApp or the DJ's mail with the message
/// written and the guest as its recipient. Nothing is sent: the DJ reads it
/// and presses send.
///
/// # Errors
/// A guest who has not agreed to be contacted, or whose record has no number
/// or address to use — WhatsApp needs the country code, because djmanzo
/// cannot know which country a local number belongs to.
pub fn compose(guest: &Guest, reach: Reach, text: &str) -> Result<String, String> {
    if !guest.consent.contact || !guest.may_consent() {
        return Err(format!("{} has not agreed to be contacted", guest.name));
    }
    match reach {
        Reach::WhatsApp => {
            let phone = guest.phone.trim();
            let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
            let international = if phone.starts_with('+') {
                digits
            } else if let Some(rest) = digits.strip_prefix("00") {
                rest.to_owned()
            } else {
                return Err(format!(
                    "WhatsApp needs {}'s number with its country code, like +34 600 123 456",
                    guest.name
                ));
            };
            if international.len() < 8 {
                return Err(format!("{}'s number is too short to dial", guest.name));
            }
            Ok(format!(
                "https://wa.me/{international}?text={}",
                urlencoding::encode(text)
            ))
        }
        Reach::Mail => {
            let email = guest.email.trim();
            if email.is_empty() {
                return Err(format!("there is no email address for {}", guest.name));
            }
            let subject = format!("Your song from tonight, {}", guest.name.trim());
            Ok(format!(
                "mailto:{}?subject={}&body={}",
                urlencoding::encode(email).replace("%40", "@"),
                urlencoding::encode(&subject),
                urlencoding::encode(text)
            ))
        }
    }
}

/// The newest recording of a guest's voice: the take made during the song
/// they are singing, if there is one — it is newer than anything already on
/// a song — else the one on their latest song that has one.
#[must_use]
pub fn latest_voice(guest: &Guest) -> Option<&str> {
    guest.take.as_deref().or_else(|| {
        guest
            .sang
            .iter()
            .rev()
            .find_map(|song| song.voice.as_deref())
    })
}

/// The journal: every guest kept, and tonight's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Journal {
    pub guests: Vec<Guest>,
}

/// Why a guest was not saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    NoName,
    /// Contact or voice agreed to by a guest under [`CONSENT_AGE`].
    TooYoung,
    /// An email address with no `@` in it, or with spaces.
    Email,
    /// A phone number with letters in it, or too few digits to dial.
    Phone,
    /// An id that is not in the journal.
    Unknown,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Refusal::NoName => "a guest needs a name",
            Refusal::TooYoung => {
                "under 16 a guest cannot agree to contact or to their voice being used; that needs a parent"
            }
            Refusal::Email => "that email address has no @ in it, or has spaces",
            Refusal::Phone => "a phone number is digits, spaces, + ( ) - and at least six digits",
            Refusal::Unknown => "there is no such guest in the journal",
        })
    }
}

/// What saving did to a record beyond keeping it: the files it no longer
/// has any right to, for the caller to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub id: String,
    /// Voice recordings to delete, because voice was not agreed to.
    pub unlink: Vec<String>,
}

/// Whether two names are the same person's, as the host typed them.
#[must_use]
pub fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

fn valid_email(email: &str) -> bool {
    let email = email.trim();
    email.is_empty()
        || (!email.contains(char::is_whitespace)
            && email
                .split_once('@')
                .is_some_and(|(user, host)| !user.is_empty() && host.contains('.')))
}

fn valid_phone(phone: &str) -> bool {
    let phone = phone.trim();
    phone.is_empty()
        || (phone
            .chars()
            .all(|c| c.is_ascii_digit() || " +()-.".contains(c))
            && phone.chars().filter(char::is_ascii_digit).count() >= 6)
}

/// Whether `name` is a recording's file name as the journal makes them: a
/// plain name with no folder in it, ending in `.wav`.
#[must_use]
pub fn is_voice_file(name: &str) -> bool {
    !name.is_empty()
        && name.ends_with(".wav")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !name.starts_with('.')
}

/// A new id: random, so two journals merged by hand never share one. The
/// standard library's per-process random hash keys, over the clock and a
/// count, are random enough for that without a dependency.
#[must_use]
pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static MADE: AtomicU64 = AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos()),
    );
    hasher.write_u64(MADE.fetch_add(1, Ordering::Relaxed));
    format!("{:016x}", hasher.finish())
}

impl Journal {
    fn fresh_id(&self) -> String {
        loop {
            let id = new_id();
            if self.get(&id).is_none() {
                return id;
            }
        }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Guest> {
        self.guests.iter().find(|guest| guest.id == id)
    }

    /// Write a guest down, or change one — holding to what they agreed.
    ///
    /// An empty id is a new guest. The performances and the date first seen
    /// are the journal's, not the form's: a form sent back with an old copy
    /// cannot lose a song sung since. Whatever the form carries, the contact
    /// details are cleared without `contact`, and without `voice` every
    /// recording is let go of and named in [`Saved::unlink`].
    ///
    /// # Errors
    /// A [`Refusal`], and nothing changed.
    pub fn save(&mut self, mut guest: Guest, now: i64) -> Result<Saved, Refusal> {
        guest.name = guest.name.trim().to_owned();
        if guest.name.is_empty() {
            return Err(Refusal::NoName);
        }
        if !guest.may_consent() && (guest.consent.contact || guest.consent.voice) {
            return Err(Refusal::TooYoung);
        }
        if !valid_email(&guest.email) {
            return Err(Refusal::Email);
        }
        if !valid_phone(&guest.phone) {
            return Err(Refusal::Phone);
        }
        let existing = if guest.id.is_empty() {
            None
        } else {
            Some(
                self.guests
                    .iter()
                    .position(|g| g.id == guest.id)
                    .ok_or(Refusal::Unknown)?,
            )
        };
        if !guest.consent.contact {
            guest.email.clear();
            guest.phone.clear();
            guest.socials.clear();
        }
        guest.email = guest.email.trim().to_owned();
        guest.phone = guest.phone.trim().to_owned();
        guest.socials.retain(|s| !s.trim().is_empty());
        guest.languages.retain(|l| !l.trim().is_empty());
        let before = existing.map(|at| self.guests[at].clone());
        // A consent that changed is a consent given now, to this wording.
        if before.as_ref().map(|g| g.consent) != Some(guest.consent) {
            guest.consent.given = now;
            guest.consent.wording = WORDING;
        } else if let Some(before) = &before {
            guest.consent.given = before.consent.given;
            guest.consent.wording = before.consent.wording;
        }
        guest.sang = before.as_ref().map(|g| g.sang.clone()).unwrap_or_default();
        guest.since = before.as_ref().map_or(now, |g| g.since);
        guest.take = before.as_ref().and_then(|g| g.take.clone());
        // Kept by the press that keeps it, never by the form — and not at
        // all once the guest is not to be kept.
        guest.microphone = before
            .as_ref()
            .and_then(|g| g.microphone)
            .filter(|_| guest.consent.keep);
        let mut unlink = Vec::new();
        if !guest.consent.voice {
            for song in &mut guest.sang {
                if let Some(file) = song.voice.take() {
                    unlink.push(file);
                }
            }
            unlink.extend(guest.take.take());
        }
        if let Some(at) = existing {
            self.guests[at] = guest;
        } else {
            guest.id = self.fresh_id();
            self.guests.push(guest);
        }
        let id = existing.map_or_else(
            || self.guests.last().map(|g| g.id.clone()).unwrap_or_default(),
            |at| self.guests[at].id.clone(),
        );
        Ok(Saved { id, unlink })
    }

    /// Somebody from the rotation sang: the song goes on their record, found
    /// by name — tonight's guest of that name if there is one, else the
    /// latest kept one. A singer the journal has never seen is written down
    /// with their name and the song, agreeing to nothing, so they are
    /// tonight's only until they are asked.
    pub fn sang(&mut self, name: &str, song: Performance, now: i64) -> String {
        let name = name.trim();
        if let Some(guest) = self.named(name).map(|at| &mut self.guests[at]) {
            let mut song = song;
            if let Some(take) = guest.take.take() {
                song.voice = Some(take);
            }
            guest.sang.push(song);
            return guest.id.clone();
        }
        let guest = Guest {
            id: self.fresh_id(),
            name: name.to_owned(),
            since: now,
            sang: vec![song],
            ..Guest::default()
        };
        let id = guest.id.clone();
        self.guests.push(guest);
        id
    }

    /// Where the guest a rotation's singer is stands in the journal, found by
    /// name: tonight's guest of that name if there is one, else the latest
    /// kept one.
    fn named(&self, name: &str) -> Option<usize> {
        self.guests
            .iter()
            .rposition(|guest| same_name(&guest.name, name))
    }

    /// K3: the microphone chain kept for the rotation's singer `name` — only
    /// on a guest who has agreed to be kept, and held to the strip's limits,
    /// because the journal is a file.
    #[must_use]
    pub fn microphone(&self, name: &str) -> Option<dj_vocal::Chain> {
        let guest = &self.guests[self.named(name)?];
        guest
            .consent
            .keep
            .then_some(guest.microphone)
            .flatten()
            .map(|chain| chain.held())
    }

    /// K3: keep `chain` for the rotation's singer `name`, on their record —
    /// when they are in the journal and have agreed to be kept. Answers
    /// whether it was kept here; when it was not, it belongs on their place
    /// in tonight's rotation.
    pub fn keep_microphone(&mut self, name: &str, chain: dj_vocal::Chain) -> bool {
        let Some(guest) = self.named(name).map(|at| &mut self.guests[at]) else {
            return false;
        };
        if !guest.consent.keep {
            return false;
        }
        guest.microphone = Some(chain.held());
        true
    }

    /// Attach a recording of a guest's voice: to the song they are singing
    /// now, which is not on their record until it is marked as sung, or else
    /// to the song they sang last. Answers the recording it replaced, to
    /// delete.
    ///
    /// # Errors
    /// A guest who is not there, has not agreed to it, or has sung nothing
    /// and is not singing.
    pub fn attach_voice(
        &mut self,
        id: &str,
        file: String,
        singing: bool,
    ) -> Result<Option<String>, String> {
        let guest = self
            .guests
            .iter_mut()
            .find(|guest| guest.id == id)
            .ok_or_else(|| Refusal::Unknown.to_string())?;
        if !guest.consent.voice || !guest.may_consent() {
            return Err(format!(
                "{} has not agreed to their voice being recorded",
                guest.name
            ));
        }
        if singing {
            return Ok(guest.take.replace(file));
        }
        let song = guest
            .sang
            .last_mut()
            .ok_or_else(|| format!("{} has not sung yet", guest.name))?;
        Ok(song.voice.replace(file))
    }

    /// §123: keep the song written for a guest on their last performance.
    ///
    /// # Errors
    /// A guest who is not there, has not agreed to a song, or has sung nothing.
    pub fn set_song(&mut self, id: &str, draft: dj_assistant::SongDraft) -> Result<(), String> {
        let guest = self
            .guests
            .iter_mut()
            .find(|guest| guest.id == id)
            .ok_or_else(|| Refusal::Unknown.to_string())?;
        if !guest.consent.voice || !guest.may_consent() {
            return Err(format!(
                "{} has not agreed to a song being made for them",
                guest.name
            ));
        }
        let song = guest
            .sang
            .last_mut()
            .ok_or_else(|| format!("{} has not sung yet", guest.name))?;
        song.song = Some(draft);
        Ok(())
    }

    /// Delete a guest and everything recorded of them. Answers the recordings
    /// to delete, or `None` for a guest not in the journal.
    pub fn forget(&mut self, id: &str) -> Option<Vec<String>> {
        let at = self.guests.iter().position(|guest| guest.id == id)?;
        let guest = self.guests.remove(at);
        Some(
            guest
                .sang
                .into_iter()
                .filter_map(|song| song.voice)
                .chain(guest.take)
                .collect(),
        )
    }

    /// A new night: every guest who did not agree to be kept goes, with their
    /// recordings. Answers the recordings to delete.
    pub fn new_night(&mut self) -> Vec<String> {
        let mut unlink = Vec::new();
        self.guests.retain(|guest| {
            if guest.consent.keep {
                return true;
            }
            unlink.extend(guest.sang.iter().filter_map(|song| song.voice.clone()));
            unlink.extend(guest.take.clone());
            false
        });
        unlink
    }

    /// One guest's record, whole, as they could be given it.
    #[must_use]
    pub fn export_guest(&self, id: &str) -> Option<String> {
        serde_json::to_string_pretty(self.get(id)?).ok()
    }

    /// The journal as a table: one row a song, the guest's details on each.
    /// For the host's own records; the recordings stay files.
    #[must_use]
    pub fn export_table(&self) -> String {
        const HEAD: [&str; 18] = [
            "name",
            "age",
            "email",
            "phone",
            "socials",
            "home",
            "nationality",
            "favourite band",
            "favourite genre",
            "favourite song",
            "native language",
            "languages",
            "keep",
            "contact",
            "voice",
            "song",
            "when",
            "event",
        ];
        let cell = |text: &str| {
            if text.contains([',', '"', '\n']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_owned()
            }
        };
        let mut out = HEAD.join(",");
        out.push('\n');
        for guest in &self.guests {
            let songs: Vec<Option<&Performance>> = if guest.sang.is_empty() {
                vec![None]
            } else {
                guest.sang.iter().map(Some).collect()
            };
            for song in songs {
                let row = [
                    guest.name.clone(),
                    guest.age.map(|a| a.to_string()).unwrap_or_default(),
                    guest.email.clone(),
                    guest.phone.clone(),
                    guest.socials.join(" "),
                    guest.home.clone(),
                    guest.nationality.clone(),
                    guest.favourite_band.clone(),
                    guest.favourite_genre.clone(),
                    guest.favourite_song.clone(),
                    guest.native_language.clone(),
                    guest.languages.join(" "),
                    guest.consent.keep.to_string(),
                    guest.consent.contact.to_string(),
                    guest.consent.voice.to_string(),
                    song.map(|s| s.title.clone()).unwrap_or_default(),
                    song.map(|s| s.at.to_string()).unwrap_or_default(),
                    song.map(|s| s.event.clone()).unwrap_or_default(),
                ];
                let cells: Vec<String> = row.iter().map(|text| cell(text)).collect();
                out.push_str(&cells.join(","));
                out.push('\n');
            }
        }
        out
    }
}

/// The journal's file in a settings folder.
#[must_use]
pub fn journal_path(config: &Path) -> PathBuf {
    config.join("guests.json")
}

/// Where the recordings of guests' voices are kept: beside the journal, and
/// nowhere else.
#[must_use]
pub fn voices_path(config: &Path) -> PathBuf {
    config.join("guests")
}

/// The journal kept in `config`. Empty when there is none or it cannot be
/// read.
#[must_use]
pub fn read(config: &Path) -> Journal {
    std::fs::read_to_string(journal_path(config))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Keep the journal in `config`, and delete the recordings it has let go of.
///
/// # Errors
/// The file system's own sentence: a journal that could not be written is
/// said, not shrugged at, because what a guest agreed to is in it.
pub fn write(config: &Path, journal: &Journal, unlink: &[String]) -> Result<(), String> {
    let path = journal_path(config);
    let text = serde_json::to_string_pretty(journal).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    let voices = voices_path(config);
    for file in unlink {
        // Only a plain file name, in the voices folder: a journal edited by
        // hand cannot point this at anything else.
        if is_voice_file(file) {
            let _ = std::fs::remove_file(voices.join(file));
        }
    }
    Ok(())
}

/// A voice take asked for: whose it is, and where their journal is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Take {
    pub config: PathBuf,
    pub guest: String,
    /// Whether they are singing now, so the take waits for the song to be
    /// marked as sung rather than going on the last one.
    pub singing: bool,
    /// When it was asked for, in seconds since 1970.
    pub asked: i64,
}

/// How long a take asked for may go unanswered before another may be asked
/// for: a take the recorder refused — busy with a sample, say — never lands,
/// and must not hold the next one off for ever.
pub const TAKE_EXPIRES: i64 = 60;

/// §123: the voice take in flight, shared between the command that asks for
/// it and the host thread it arrives on.
///
/// **A take nobody asked for is thrown away.** The recorder can be started
/// by any action — a script, a controller, the assistant — and none of them
/// asked a guest anything; only [`Takes::expect`], called after a guest's
/// consent was checked, gives a take somewhere to go.
#[derive(Debug, Default)]
pub struct Takes {
    pending: std::sync::Mutex<Option<Take>>,
    /// What became of the last take: the file, or why there is none.
    last: std::sync::Mutex<Option<Result<String, String>>>,
}

impl Takes {
    /// Expect a take for this guest. Refused while another is in flight and
    /// not yet expired.
    pub fn expect(&self, take: Take) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        if pending
            .as_ref()
            .is_some_and(|waiting| take.asked - waiting.asked < TAKE_EXPIRES)
        {
            return false;
        }
        *pending = Some(take);
        if let Ok(mut last) = self.last.lock() {
            *last = None;
        }
        true
    }

    /// Stop expecting one, for a take that could not be started.
    pub fn forget(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
    }

    /// The guest a take is being recorded for, if one is.
    #[must_use]
    pub fn recording(&self) -> Option<String> {
        self.pending
            .lock()
            .ok()?
            .as_ref()
            .map(|take| take.guest.clone())
    }

    /// What became of the last take.
    #[must_use]
    pub fn last(&self) -> Option<Result<String, String>> {
        self.last.lock().ok()?.clone()
    }

    /// A take has finished: keep it as a file for the guest it was asked
    /// for, and put it on their record — or throw it away when nobody asked
    /// for it or they no longer agree. On the host thread, never the audio
    /// one.
    pub fn land(&self, samples: &[f32], sample_rate: dj_core::SampleRate) {
        let Some(take) = self.pending.lock().ok().and_then(|mut p| p.take()) else {
            tracing::warn!("a voice take arrived that nobody asked for; not kept");
            return;
        };
        let outcome = keep_take(&take, samples, sample_rate);
        if let Err(error) = &outcome {
            tracing::warn!(%error, "a guest's voice take was not kept");
        }
        if let Ok(mut last) = self.last.lock() {
            *last = Some(outcome);
        }
    }
}

/// Put the take on the guest's record, then write it: the record decides
/// whether they still agree (the one place that rule is, `attach_voice`), so
/// a voice they took consent back from is never written to disk at all.
fn keep_take(
    take: &Take,
    samples: &[f32],
    sample_rate: dj_core::SampleRate,
) -> Result<String, String> {
    let file = format!("{}-{}.wav", take.guest, take.asked);
    if !is_voice_file(&file) {
        return Err(format!("{file:?} is not a name a recording is kept under"));
    }
    let mut journal = read(&take.config);
    let replaced = journal.attach_voice(&take.guest, file.clone(), take.singing)?;
    let dir = voices_path(&take.config);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(&file);
    let pcm: Vec<i16> = samples
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
        .collect();
    let written = crate::wav::Wav::create(&path, sample_rate.get())
        .and_then(|mut wav| {
            wav.write(&pcm)?;
            wav.close()
        })
        .map_err(|e| e.to_string());
    if let Err(error) = written {
        let _ = std::fs::remove_file(&path);
        return Err(error);
    }
    write(
        &take.config,
        &journal,
        &replaced.into_iter().collect::<Vec<_>>(),
    )?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guest(name: &str) -> Guest {
        Guest {
            name: name.to_owned(),
            ..Guest::default()
        }
    }

    fn song(title: &str) -> Performance {
        Performance {
            title: title.to_owned(),
            ..Performance::default()
        }
    }

    /// **§123: a song brief only with consent, and with nothing a song does
    /// not need.** Their contact details, age, home and nationality never
    /// reach the model; the song they sang, where, and their languages do.
    #[test]
    fn a_song_is_briefed_only_with_consent_and_without_their_details() {
        let mut aiko = guest("Aiko");
        aiko.email = "aiko@example.org".into();
        aiko.phone = "+81 90 1234 5678".into();
        aiko.age = Some(29);
        aiko.home = "Fukuoka, Japan".into();
        aiko.nationality = "Japanese".into();
        aiko.native_language = "Japanese".into();
        aiko.favourite_band = "Aventura".into();
        aiko.sang = vec![Performance {
            event: "Noche Latina".into(),
            place: "Bar Sol".into(),
            ..song("Bachata en Fukuoka")
        }];
        assert_eq!(
            brief(&aiko, "es", "", "today", Vec::new()),
            None,
            "no consent yet"
        );

        aiko.consent.voice = true;
        let made =
            brief(&aiko, "es", " brave ", "Saturday", vec!["latin".into()]).expect("briefed");
        assert_eq!(made.singer, "Aiko");
        assert_eq!(made.sang, "Bachata en Fukuoka");
        assert_eq!(
            (made.event.as_str(), made.place.as_str()),
            ("Noche Latina", "Bar Sol")
        );
        assert_eq!(made.languages, ["Spanish", "Japanese"]);
        assert_eq!(made.keywords, "brave");
        let told = dj_assistant::song::song_request(&made);
        for private in ["aiko@example.org", "+81", "29", "Fukuoka, Japan"] {
            assert!(
                !told.contains(private),
                "{private} reached the model:\n{told}"
            );
        }

        aiko.age = Some(15);
        assert_eq!(
            brief(&aiko, "es", "", "", Vec::new()),
            None,
            "too young to agree"
        );
        aiko.age = Some(29);
        aiko.sang.clear();
        assert_eq!(
            brief(&aiko, "es", "", "", Vec::new()),
            None,
            "nothing sung yet"
        );
    }

    #[test]
    fn a_song_is_kept_on_the_last_performance_and_only_with_consent() {
        let mut journal = Journal::default();
        let mut aiko = guest("Aiko");
        aiko.id = "a1".into();
        aiko.sang = vec![song("First"), song("Second")];
        journal.guests.push(aiko);
        let draft = dj_assistant::SongDraft {
            style: "bachata".into(),
            versions: vec![dj_assistant::Version {
                language: "Spanish".into(),
                lyrics: "[Verse]".into(),
            }],
        };
        assert!(journal.set_song("a1", draft.clone()).is_err(), "no consent");
        journal.guests[0].consent.voice = true;
        journal.set_song("a1", draft.clone()).expect("kept");
        assert_eq!(journal.guests[0].sang[1].song.as_ref(), Some(&draft));
        assert_eq!(journal.guests[0].sang[0].song, None);
        assert!(journal.set_song("nobody", draft).is_err());
    }

    /// **The message reaches only a guest who agreed, at the number or
    /// address on their record, written and not sent.**
    #[test]
    fn a_message_is_composed_only_with_consent_and_a_way_to_reach_them() {
        let mut ana = guest("Ana");
        ana.phone = "+34 600-123 456".into();
        ana.email = "ana.m@example.org".into();
        ana.sang = vec![Performance {
            event: "Noche Latina".into(),
            ..song("Obsesión")
        }];
        let text = message(&ana, " https://suno.com/s/abc ");
        assert_eq!(
            text,
            "Hi Ana! Thank you for singing \"Obsesión\" at Noche Latina. \
             Here is a song made for you from tonight: https://suno.com/s/abc"
        );
        assert!(
            compose(&ana, Reach::WhatsApp, &text).is_err(),
            "no consent to contact"
        );

        ana.consent.contact = true;
        let whatsapp = compose(&ana, Reach::WhatsApp, &text).expect("composed");
        assert!(
            whatsapp.starts_with("https://wa.me/34600123456?text=Hi%20Ana"),
            "{whatsapp}"
        );
        assert!(
            whatsapp.contains("https%3A%2F%2Fsuno.com%2Fs%2Fabc"),
            "{whatsapp}"
        );
        let mail = compose(&ana, Reach::Mail, &text).expect("composed");
        assert!(
            mail.starts_with("mailto:ana.m@example.org?subject=Your%20song"),
            "{mail}"
        );

        ana.phone = "0034 600 123 456".into();
        assert!(
            compose(&ana, Reach::WhatsApp, &text)
                .expect("00 is a country code")
                .contains("wa.me/34600123456")
        );
        ana.phone = "600 123 456".into();
        let local = compose(&ana, Reach::WhatsApp, &text).expect_err("no country code");
        assert!(local.contains("country code"), "{local}");
        ana.email.clear();
        assert!(compose(&ana, Reach::Mail, &text).is_err());
        ana.age = Some(14);
        ana.phone = "+34 600 123 456".into();
        assert!(
            compose(&ana, Reach::WhatsApp, &text).is_err(),
            "too young to agree"
        );
        assert_eq!(
            message(&guest("Bo"), ""),
            "Hi Bo! Thank you for singing. Here is a song made for you from tonight."
        );
    }

    #[test]
    fn the_newest_voice_is_the_one_shown() {
        let mut ana = guest("Ana");
        assert_eq!(latest_voice(&ana), None);
        ana.sang = vec![
            Performance {
                voice: Some("a.wav".into()),
                ..song("A")
            },
            Performance {
                voice: Some("b.wav".into()),
                ..song("B")
            },
            song("C"),
        ];
        assert_eq!(latest_voice(&ana), Some("b.wav"));
        ana.take = Some("t.wav".into());
        assert_eq!(
            latest_voice(&ana),
            Some("t.wav"),
            "the take is newer than any song's"
        );
    }

    /// **The load-bearing one.** Nothing is kept that was not agreed to: the
    /// contact details go without `contact`, whatever the form sent; the
    /// recordings go when `voice` is taken back; and a guest not agreeing to
    /// be kept is gone at the next night, with what was recorded of them.
    #[test]
    fn nothing_is_kept_that_was_not_agreed_to() {
        let mut journal = Journal::default();
        let mut ana = guest("Ana");
        ana.email = "ana@example.org".into();
        ana.phone = "+34 600 123 456".into();
        ana.socials = vec!["@ana".into()];
        ana.favourite_band = "Aventura".into();
        let saved = journal.save(ana.clone(), 100).expect("saved");
        let kept = journal.get(&saved.id).expect("there");
        assert!(kept.email.is_empty() && kept.phone.is_empty() && kept.socials.is_empty());
        assert_eq!(kept.favourite_band, "Aventura", "what is not contact stays");

        ana.id = saved.id.clone();
        ana.consent = Consent {
            keep: true,
            contact: true,
            voice: true,
            ..Consent::default()
        };
        journal.save(ana.clone(), 200).expect("agreed");
        let kept = journal.get(&saved.id).expect("there");
        assert_eq!(kept.email, "ana@example.org");
        assert_eq!(kept.consent.given, 200, "stamped when it changed");
        assert_eq!(kept.consent.wording, WORDING);

        journal.sang("ana", song("Obsesión"), 300);
        assert_eq!(
            journal.attach_voice(&saved.id, "ana-1.wav".into(), false),
            Ok(None)
        );
        // Voice taken back: the recording is let go of, and named to delete.
        ana.consent.voice = false;
        let saved = journal.save(ana, 400).expect("saved");
        assert_eq!(saved.unlink, ["ana-1.wav"]);
        let kept = journal.get(&saved.id).expect("there");
        assert_eq!(kept.sang.len(), 1, "the song stays; only the voice goes");
        assert!(kept.sang[0].voice.is_none());
        assert!(
            journal
                .attach_voice(&saved.id, "again.wav".into(), false)
                .is_err()
        );

        // Tonight only: gone at the next night, with the recording.
        let mut tonight = guest("Bo");
        tonight.consent.voice = true;
        let bo = journal.save(tonight, 500).expect("saved").id;
        journal.sang("Bo", song("Vivir mi vida"), 510);
        journal
            .attach_voice(&bo, "bo-1.wav".into(), false)
            .expect("agreed");
        assert_eq!(journal.new_night(), ["bo-1.wav"]);
        assert!(journal.get(&bo).is_none());
        assert!(journal.get(&saved.id).is_some(), "Ana agreed to be kept");
    }

    #[test]
    fn a_child_cannot_agree_to_contact_or_voice_alone() {
        let mut journal = Journal::default();
        let mut kid = guest("Kid");
        kid.age = Some(14);
        kid.consent.voice = true;
        assert_eq!(journal.save(kid.clone(), 1), Err(Refusal::TooYoung));
        kid.consent = Consent {
            keep: true,
            contact: true,
            ..Consent::default()
        };
        assert_eq!(journal.save(kid.clone(), 1), Err(Refusal::TooYoung));
        kid.consent.contact = false;
        assert!(journal.save(kid.clone(), 1).is_ok(), "kept, not contacted");
        kid.age = Some(CONSENT_AGE);
        kid.consent.voice = true;
        kid.id = journal.guests[0].id.clone();
        assert!(journal.save(kid, 1).is_ok());
        assert_eq!(journal.guests.len(), 1);
    }

    #[test]
    fn a_song_goes_on_the_record_of_the_guest_who_sang_it() {
        let mut journal = Journal::default();
        let ana = journal.save(guest("Ana"), 1).expect("saved").id;
        assert_eq!(journal.sang(" ANA ", song("Obsesión"), 2), ana);
        let stranger = journal.sang("Cleo", song("Hips don't lie"), 3);
        assert_ne!(stranger, ana);
        let cleo = journal.get(&stranger).expect("written down");
        assert_eq!(cleo.consent, Consent::default(), "agreeing to nothing");
        assert_eq!(cleo.since, 3);
        // A form sent back with an old copy cannot lose a song sung since.
        let mut old = journal.get(&ana).expect("there").clone();
        old.sang.clear();
        old.since = 999;
        journal.save(old, 4).expect("saved");
        let kept = journal.get(&ana).expect("there");
        assert_eq!(kept.sang.len(), 1);
        assert_eq!(kept.since, 1);
    }

    #[test]
    fn a_guest_is_forgotten_whole_and_can_have_a_copy() {
        let mut journal = Journal::default();
        let mut ana = guest("Ana");
        ana.consent.voice = true;
        let id = journal.save(ana, 1).expect("saved").id;
        journal.sang("Ana", song("Obsesión"), 2);
        journal
            .attach_voice(&id, "ana-1.wav".into(), false)
            .expect("agreed");
        let copy = journal.export_guest(&id).expect("a copy");
        assert!(copy.contains("Obsesión") && copy.contains("ana-1.wav"));
        assert_eq!(journal.forget(&id), Some(vec!["ana-1.wav".to_owned()]));
        assert_eq!(journal.forget(&id), None);
        assert!(journal.export_guest(&id).is_none());
    }

    #[test]
    fn what_is_typed_is_checked_before_it_is_kept() {
        let mut journal = Journal::default();
        assert_eq!(journal.save(guest("  "), 1), Err(Refusal::NoName));
        let mut ana = guest("Ana");
        ana.consent.contact = true;
        ana.email = "ana at home".into();
        assert_eq!(journal.save(ana.clone(), 1), Err(Refusal::Email));
        ana.email = "ana@example.org".into();
        ana.phone = "call me".into();
        assert_eq!(journal.save(ana.clone(), 1), Err(Refusal::Phone));
        ana.phone = "+49 (30) 1234-567".into();
        assert!(journal.save(ana.clone(), 1).is_ok());
        ana.id = "nobody".into();
        assert_eq!(journal.save(ana, 1), Err(Refusal::Unknown));
    }

    #[test]
    fn the_table_has_a_row_a_song_and_quotes_what_needs_it() {
        let mut journal = Journal::default();
        let mut ana = guest("Ana, from Madrid");
        ana.favourite_song = "Say \"yes\"".into();
        journal.save(ana, 1).expect("saved");
        journal.sang("Ana, from Madrid", song("One"), 2);
        journal.sang("Ana, from Madrid", song("Two"), 3);
        journal.save(guest("Bo"), 4).expect("saved");
        let table = journal.export_table();
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 4, "a head, two songs, and Bo with none");
        assert!(lines[1].starts_with("\"Ana, from Madrid\""));
        assert!(lines[1].contains("\"Say \"\"yes\"\"\""));
        assert!(lines[3].starts_with("Bo,"));
    }

    /// K3: **a guest's microphone chain is the journal's, kept only while
    /// they agree to be kept.** A form cannot put one on a record or take one
    /// off, and a guest who is not to be kept has none there to read — the
    /// host keeps it on their place in tonight's rotation instead.
    #[test]
    fn a_microphone_chain_is_kept_only_with_keep() {
        let soft = dj_vocal::Preset::SoftSinger
            .applied_to(&dj_vocal::StripSettings::default())
            .chain();
        let mut journal = Journal::default();
        let mut forged = guest("Ana");
        forged.consent.keep = true;
        forged.microphone = Some(soft);
        let id = journal.save(forged, 1).expect("saved").id;
        assert_eq!(journal.microphone("Ana"), None, "not the form's to set");

        assert!(journal.keep_microphone("ana", soft));
        assert_eq!(journal.microphone("Ana"), Some(soft));
        let mut sent_back = journal.get(&id).expect("there").clone();
        sent_back.microphone = None;
        sent_back.notes = "sang in the window".to_owned();
        journal.save(sent_back, 2).expect("saved");
        assert_eq!(
            journal.microphone("Ana"),
            Some(soft),
            "nor the form's to clear"
        );

        let mut tonight_only = journal.get(&id).expect("there").clone();
        tonight_only.consent.keep = false;
        journal.save(tonight_only, 3).expect("saved");
        assert_eq!(journal.microphone("Ana"), None);
        assert!(!journal.keep_microphone("Ana", soft));
        assert!(journal.get(&id).expect("there").microphone.is_none());
        assert!(!journal.keep_microphone("Nobody", soft));
    }

    fn agreeing(name: &str) -> Guest {
        let mut guest = guest(name);
        guest.consent = Consent {
            keep: true,
            voice: true,
            ..Consent::default()
        };
        guest
    }

    fn take(config: &Path, guest: &str, singing: bool, asked: i64) -> Take {
        Take {
            config: config.to_path_buf(),
            guest: guest.to_owned(),
            singing,
            asked,
        }
    }

    const RATE: dj_core::SampleRate = dj_core::SampleRate::DEFAULT;

    /// **The load-bearing one for the voice.** A take is kept only for the
    /// guest it was asked for, after they agreed: as a WAV beside the
    /// journal, on the song they are singing once it is marked as sung. A
    /// take nobody asked for — the recorder can be started by any script or
    /// controller — is thrown away, and so is one whose guest took their
    /// consent back while it was recording.
    #[test]
    fn a_voice_is_kept_only_for_the_guest_who_agreed() {
        let dir = tempfile::tempdir().expect("a folder");
        let config = dir.path();
        let mut journal = Journal::default();
        let ana = journal.save(agreeing("Ana"), 1).expect("saved").id;
        write(config, &journal, &[]).expect("kept");
        let takes = Takes::default();
        let voice = vec![0.25f32; 48_000 * 2];

        // Nobody asked: nothing written, nothing attached.
        takes.land(&voice, RATE);
        assert!(!voices_path(config).exists());
        assert_eq!(read(config), journal);

        // Asked for Ana, while she sings.
        assert!(takes.expect(take(config, &ana, true, 100)));
        assert_eq!(takes.recording().as_deref(), Some(ana.as_str()));
        takes.land(&voice, RATE);
        assert_eq!(takes.recording(), None);
        let file = format!("{ana}-100.wav");
        assert_eq!(takes.last(), Some(Ok(file.clone())));
        let path = voices_path(config).join(&file);
        let bytes = std::fs::read(&path).expect("the recording is a file");
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + 48_000 * 2 * 2, "a second, stereo, 16-bit");
        let mut journal = read(config);
        assert_eq!(
            journal.get(&ana).expect("there").take.as_deref(),
            Some(file.as_str())
        );
        // Marked as sung: onto that song.
        journal.sang("Ana", song("Obsesión"), 120);
        let kept = journal.get(&ana).expect("there");
        assert_eq!(kept.sang[0].voice.as_deref(), Some(file.as_str()));
        assert!(kept.take.is_none());

        // Consent taken back while a take was recording: not kept.
        let mut ana_now = kept.clone();
        write(config, &journal, &[]).expect("kept");
        assert!(takes.expect(take(config, &ana, false, 200)));
        ana_now.consent.voice = false;
        let saved = journal.save(ana_now, 210).expect("saved");
        write(config, &journal, &saved.unlink).expect("kept");
        assert!(!path.exists(), "taking voice back deletes the recording");
        takes.land(&voice, RATE);
        assert!(matches!(takes.last(), Some(Err(_))));
        assert_eq!(
            std::fs::read_dir(voices_path(config))
                .expect("the folder")
                .count(),
            0,
            "nothing left behind"
        );
    }

    #[test]
    fn one_take_at_a_time_until_one_is_left_hanging() {
        let takes = Takes::default();
        let here = Path::new("/nowhere");
        assert!(takes.expect(take(here, "a", false, 1_000)));
        assert!(
            !takes.expect(take(here, "b", false, 1_010)),
            "one at a time"
        );
        assert!(
            takes.expect(take(here, "b", false, 1_000 + TAKE_EXPIRES)),
            "a take the recorder never answered does not hold the next off"
        );
        takes.forget();
        assert_eq!(takes.recording(), None);
    }
}

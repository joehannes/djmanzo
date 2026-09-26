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
}

/// A guest who sang.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// The journal: every guest kept, and tonight's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

fn same_name(a: &str, b: &str) -> bool {
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
        let mut unlink = Vec::new();
        if !guest.consent.voice {
            for song in &mut guest.sang {
                if let Some(file) = song.voice.take() {
                    unlink.push(file);
                }
            }
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
        if let Some(guest) = self
            .guests
            .iter_mut()
            .rev()
            .find(|guest| same_name(&guest.name, name))
        {
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

    /// Attach a recording of a guest's voice to the song they sang last.
    ///
    /// # Errors
    /// A guest who is not there, has not agreed to it, or has sung nothing.
    pub fn attach_voice(&mut self, id: &str, file: String) -> Result<Option<String>, String> {
        let guest = self
            .guests
            .iter_mut()
            .find(|guest| guest.id == id)
            .ok_or_else(|| Refusal::Unknown.to_string())?;
        if !guest.consent.voice {
            return Err(format!(
                "{} has not agreed to their voice being recorded",
                guest.name
            ));
        }
        let song = guest
            .sang
            .last_mut()
            .ok_or_else(|| format!("{} has not sung yet", guest.name))?;
        Ok(song.voice.replace(file))
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
            journal.attach_voice(&saved.id, "ana-1.wav".into()),
            Ok(None)
        );
        // Voice taken back: the recording is let go of, and named to delete.
        ana.consent.voice = false;
        let saved = journal.save(ana, 400).expect("saved");
        assert_eq!(saved.unlink, ["ana-1.wav"]);
        let kept = journal.get(&saved.id).expect("there");
        assert_eq!(kept.sang.len(), 1, "the song stays; only the voice goes");
        assert!(kept.sang[0].voice.is_none());
        assert!(journal.attach_voice(&saved.id, "again.wav".into()).is_err());

        // Tonight only: gone at the next night, with the recording.
        let mut tonight = guest("Bo");
        tonight.consent.voice = true;
        let bo = journal.save(tonight, 500).expect("saved").id;
        journal.sang("Bo", song("Vivir mi vida"), 510);
        journal
            .attach_voice(&bo, "bo-1.wav".into())
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
            .attach_voice(&id, "ana-1.wav".into())
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
}

//! §128: finding the song a singer cannot name, by what they do remember.
//!
//! > if the singer has logic connotations/partial memories of the song,
//! > guide him through a social/human useful and helpful way to come closer
//! > to the real song/singers name and try to locate the actual song and
//! > similar candidates by those partials
//!
//! [`crate::memory`] already takes three ways in — a line of the words, a
//! description for the assistant, a hum (its key, its tempo and its tune
//! against the stored melodies). What a singer at the booth has is rarely
//! any one of those whole; it is a handful of partials a host can ask for
//! in a few seconds: *when is it from, is it slow or fast, who sings it, in
//! what language, what kind of music, who does it sound like, where did you
//! hear it*. Each answer is a question with a short set of replies, so it
//! can be asked across a noisy booth.
//!
//! Two things are done with them, and they are kept apart because they are
//! honest about different things:
//!
//! - [`narrow`] keeps the records **in the collection** that fit what the
//!   collection knows — the year tag, the genre, the tempo, the artist.
//!   Who sings and the language are not in any tag djmanzo reads, so they
//!   narrow nothing here, and the interface says so.
//! - [`describe`] turns every answer into one sentence for the assistant,
//!   which is the only part of djmanzo that can name a song nobody owns —
//!   through the guess the memory panel already makes.

use crate::commands::LibraryTrackDto;
use crate::state::AppState;
use serde::{Deserialize, Serialize};

/// How fast it goes, as a singer would say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pace {
    Slow,
    Medium,
    Fast,
}

/// Who sings it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Voice {
    Man,
    Woman,
    Duet,
    Group,
}

/// What a singer remembers. Every field is optional: one answer is a start.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Remembered {
    /// The first year of the decade, e.g. 1980.
    pub decade: Option<i32>,
    pub pace: Option<Pace>,
    pub voice: Option<Voice>,
    pub language: Option<String>,
    /// The kind of music, in their words — "bachata", "rock".
    pub genre: Option<String>,
    /// An artist it sounds like, or who they think sings it.
    pub like: Option<String>,
    /// Where they heard it — a wedding, the radio, a film.
    pub heard: Option<String>,
    /// Whatever of the words they have.
    pub words: Option<String>,
}

/// Below this many beats a minute a record is slow; above [`FAST_BPM`],
/// fast — as a singer hears it. A record written at 140 that is felt at 70
/// (a ballad's half time) is not caught by this, which is why pace only
/// narrows and never excludes on its own: see [`narrow`].
pub const SLOW_BPM: f64 = 100.0;
pub const FAST_BPM: f64 = 125.0;

/// What the collection knows of one record, for [`fits`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Known {
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub bpm: Option<f64>,
}

fn blank(text: &Option<String>) -> Option<&str> {
    text.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// The reasons a record fits what was remembered, one per answer it
/// matches; `None` when it contradicts one (a year from another decade, a
/// genre of another family, another artist named). An answer the record's
/// tags say nothing about neither fits nor contradicts.
#[must_use]
pub fn fits(known: &Known, remembered: &Remembered) -> Option<Vec<String>> {
    let mut because = Vec::new();
    if let (Some(decade), Some(year)) = (remembered.decade, known.year) {
        if (decade..decade + 10).contains(&year) {
            because.push(format!("from the {decade}s ({year})"));
        } else {
            return None;
        }
    }
    if let (Some(asked), Some(tag)) = (blank(&remembered.genre), blank(&known.genre)) {
        let same = match (
            dj_core::genre::family_for(asked),
            dj_core::genre::family_for(tag),
        ) {
            (Some(a), Some(b)) => a.name == b.name,
            _ => tag.to_lowercase().contains(&asked.to_lowercase()),
        };
        if same {
            because.push(format!("{tag}, as remembered"));
        } else {
            return None;
        }
    }
    if let (Some(pace), Some(bpm)) = (remembered.pace, known.bpm) {
        let heard = if bpm < SLOW_BPM {
            Pace::Slow
        } else if bpm > FAST_BPM {
            Pace::Fast
        } else {
            Pace::Medium
        };
        if heard == pace {
            because.push(format!("{} at {bpm:.0} BPM", word(pace)));
        }
    }
    if let (Some(like), Some(artist)) = (blank(&remembered.like), blank(&known.artist)) {
        if artist.to_lowercase().contains(&like.to_lowercase()) {
            because.push(format!("by {artist}"));
        } else {
            return None;
        }
    }
    Some(because)
}

const fn word(pace: Pace) -> &'static str {
    match pace {
        Pace::Slow => "slow",
        Pace::Medium => "mid-tempo",
        Pace::Fast => "fast",
    }
}

/// The records that fit, the ones that fit most answers first, each with
/// its reasons. A record must match at least one answer, not merely not
/// contradict any — so with no answer the collection can check (only a
/// voice, a language or the words) nothing is offered: a list of
/// everything is not a narrowing.
#[must_use]
pub fn narrow<T: Clone>(
    records: &[(T, Known)],
    remembered: &Remembered,
    most: usize,
) -> Vec<(T, Vec<String>)> {
    let mut kept: Vec<(T, Vec<String>)> = records
        .iter()
        .filter_map(|(record, known)| {
            let because = fits(known, remembered)?;
            (!because.is_empty()).then(|| (record.clone(), because))
        })
        .collect();
    // Stable: equally good records keep the collection's own order.
    kept.sort_by_key(|(_, because)| std::cmp::Reverse(because.len()));
    kept.truncate(most);
    kept
}

/// Every answer as one sentence for the assistant. Empty when nothing was
/// remembered.
#[must_use]
pub fn describe(remembered: &Remembered) -> String {
    let mut parts = Vec::new();
    if let Some(decade) = remembered.decade {
        parts.push(format!("from the {decade}s"));
    }
    if let Some(pace) = remembered.pace {
        parts.push(word(pace).to_owned());
    }
    if let Some(voice) = remembered.voice {
        parts.push(
            match voice {
                Voice::Man => "a man singing",
                Voice::Woman => "a woman singing",
                Voice::Duet => "a duet",
                Voice::Group => "a group singing",
            }
            .to_owned(),
        );
    }
    if let Some(language) = blank(&remembered.language) {
        parts.push(format!("sung in {language}"));
    }
    if let Some(genre) = blank(&remembered.genre) {
        parts.push(format!("probably {genre}"));
    }
    if let Some(like) = blank(&remembered.like) {
        parts.push(format!("sounds like {like}"));
    }
    if let Some(heard) = blank(&remembered.heard) {
        parts.push(format!("heard at {heard}"));
    }
    let mut sentence = if parts.is_empty() {
        String::new()
    } else {
        format!(
            "A song a singer wants to sing but cannot name: {}.",
            parts.join(", ")
        )
    };
    if let Some(words) = blank(&remembered.words) {
        if sentence.is_empty() {
            sentence.push_str("A song a singer wants to sing but cannot name.");
        }
        sentence.push_str(&format!(" It goes something like \"{words}\"."));
    }
    sentence
}

/// What the guided questions found.
#[derive(Debug, Clone, Serialize)]
pub struct FoundDto {
    /// Records in the collection that fit, with why.
    pub near: Vec<NearDto>,
    /// Whether any answer was one the collection can check — when not, an
    /// empty `near` is "nothing to check yet", not "nothing fits".
    pub checked: bool,
    /// Every answer as one sentence, for the assistant's guess.
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NearDto {
    pub track: LibraryTrackDto,
    pub because: Vec<String>,
}

/// The records that fit what a singer remembers, and the sentence for the
/// assistant.
///
/// # Errors
/// When the collection cannot be read.
#[tauri::command]
pub fn find_song(
    state: tauri::State<'_, AppState>,
    remembered: Remembered,
) -> Result<FoundDto, String> {
    let tracks = crate::commands::library(&state)?
        .search("", 5_000)
        .map_err(|e| e.to_string())?;
    let records: Vec<(dj_library::LibraryTrack, Known)> = tracks
        .into_iter()
        .map(|track| {
            let known = Known {
                artist: track.tags.artist.clone(),
                genre: track.tags.genre.clone(),
                year: track.tags.year,
                bpm: track.analysis.bpm,
            };
            (track, known)
        })
        .collect();
    let checked = remembered.decade.is_some()
        || remembered.pace.is_some()
        || blank(&remembered.genre).is_some()
        || blank(&remembered.like).is_some();
    Ok(FoundDto {
        near: narrow(&records, &remembered, 30)
            .into_iter()
            .map(|(track, because)| NearDto {
                track: LibraryTrackDto::from(track),
                because,
            })
            .collect(),
        checked,
        description: describe(&remembered),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(artist: &str, genre: &str, year: i32, bpm: f64) -> Known {
        Known {
            artist: Some(artist.to_owned()),
            genre: Some(genre.to_owned()),
            year: Some(year),
            bpm: Some(bpm),
        }
    }

    /// **Each answer narrows by what the tags say, and contradicting one
    /// rules a record out.** The 1980s keeps 1984 and drops 1995; *bachata*
    /// keeps a record tagged with one of its other names and drops a
    /// merengue; a named artist keeps theirs and drops another's. A record
    /// whose tag is missing is neither kept nor dropped by that answer.
    #[test]
    fn each_answer_narrows_by_the_tags_and_a_contradiction_rules_out() {
        let eighties = Remembered {
            decade: Some(1980),
            ..Remembered::default()
        };
        assert!(fits(&known("A", "pop", 1984, 110.0), &eighties).is_some_and(|b| b.len() == 1));
        assert!(fits(&known("A", "pop", 1995, 110.0), &eighties).is_none());
        let untagged = Known::default();
        assert_eq!(fits(&untagged, &eighties), Some(Vec::new()));

        let bachata = Remembered {
            genre: Some("Bachata".to_owned()),
            ..Remembered::default()
        };
        let alias = dj_core::genre::family_for("bachata")
            .and_then(|family| family.aliases.first())
            .copied()
            .unwrap_or("bachata");
        assert!(fits(&known("A", alias, 2010, 128.0), &bachata).is_some_and(|b| !b.is_empty()));
        assert!(fits(&known("A", "merengue", 2010, 150.0), &bachata).is_none());

        let aventura = Remembered {
            like: Some("aventura".to_owned()),
            ..Remembered::default()
        };
        assert!(fits(&known("Aventura", "bachata", 2002, 128.0), &aventura).is_some());
        assert!(fits(&known("Romeo Santos", "bachata", 2011, 128.0), &aventura).is_none());
    }

    /// **The best fits come first, and nothing is a list of everything.**
    /// With no answer the collection can check — only a voice, a language
    /// or the words — nothing is offered; with two answers the record that
    /// fits both is ahead of the one that fits one.
    #[test]
    fn the_best_fits_come_first_and_nothing_is_not_everything() {
        let records = vec![
            ("one fit", known("A", "pop", 1984, 150.0)),
            ("two fits", known("B", "pop", 1986, 80.0)),
            ("ruled out", known("C", "pop", 1999, 80.0)),
        ];
        let only_voice = Remembered {
            voice: Some(Voice::Woman),
            language: Some("Spanish".to_owned()),
            ..Remembered::default()
        };
        assert!(narrow(&records, &only_voice, 10).is_empty());
        let slow_eighties = Remembered {
            decade: Some(1980),
            pace: Some(Pace::Slow),
            ..Remembered::default()
        };
        let found: Vec<&str> = narrow(&records, &slow_eighties, 10)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(found, vec!["two fits", "one fit"]);
        assert_eq!(narrow(&records, &slow_eighties, 1).len(), 1);
    }

    /// **Every answer reaches the assistant, in words**, including the
    /// ones the collection cannot check.
    #[test]
    fn every_answer_reaches_the_assistant_in_words() {
        assert_eq!(describe(&Remembered::default()), "");
        let all = Remembered {
            decade: Some(1990),
            pace: Some(Pace::Slow),
            voice: Some(Voice::Woman),
            language: Some("Spanish".to_owned()),
            genre: Some("bolero".to_owned()),
            like: Some("Luis Miguel".to_owned()),
            heard: Some("a wedding".to_owned()),
            words: Some("contigo aprendí".to_owned()),
        };
        let said = describe(&all);
        for part in [
            "1990s",
            "slow",
            "a woman singing",
            "Spanish",
            "bolero",
            "Luis Miguel",
            "a wedding",
            "\"contigo aprendí\"",
        ] {
            assert!(said.contains(part), "{part} missing from {said}");
        }
        let only_words = Remembered {
            words: Some("la la".to_owned()),
            ..Remembered::default()
        };
        assert!(describe(&only_words).contains("\"la la\""));
    }
}

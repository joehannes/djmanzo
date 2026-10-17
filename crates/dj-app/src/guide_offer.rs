//! Karaoke beta B7 (§128): the guide vocal, proposed for a song the singer
//! does not know well.
//!
//! When the host is about to load a singer's song, the up-next card offers
//! the guide — the record's own singer quietly underneath, about −12 dB —
//! where the song is new to them: never sung by them in the guest book. A
//! singer who already chose a voice for the record (from their phone, or the
//! host taking it) is not asked again; that choice is applied on load. The
//! offer is a proposal the host takes or leaves, never applied by itself.

use serde::Serialize;
use tauri::State;

use crate::guests::{Journal, same_name};
use crate::karaoke::Rotation;
use crate::state::AppState;

/// Why the guide is offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuideOffer {
    /// The host's line, in the up-next card.
    pub line: String,
}

/// A title as compared: lower case, the artist before a dash left out, and
/// only letters and digits.
fn plain(title: &str) -> String {
    let title = title
        .rsplit_once(" - ")
        .or_else(|| title.rsplit_once(" – "))
        .map_or(title, |(_, song)| song);
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether `song` is one of `sung` — the same song however it was written
/// down ("Juan Luis Guerra - Bachata Rosa" is "bachata rosa").
#[must_use]
pub fn sung_before(sung: &[&str], song: &str) -> bool {
    let wanted = plain(song);
    !wanted.is_empty() && sung.iter().any(|title| plain(title) == wanted)
}

/// The offer, or none: none when a voice is already chosen for the record,
/// or the guest book has them singing this song before — unless they said
/// they are not sure of it (`unsure`), when it is offered all the same.
#[must_use]
pub fn offer(
    name: &str,
    song: &str,
    chosen_voice: bool,
    unsure: bool,
    sung: &[&str],
) -> Option<GuideOffer> {
    if chosen_voice || song.trim().is_empty() {
        return None;
    }
    let line = if unsure {
        format!(
            "Not sure of it, {}: the guide voice keeps the original singer quietly underneath.",
            name.trim()
        )
    } else if sung_before(sung, song) {
        return None;
    } else {
        format!(
            "New to {}: the guide voice keeps the original singer quietly underneath.",
            name.trim()
        )
    };
    Some(GuideOffer { line })
}

/// The offer for the singer up next and their first song, from the
/// rotation and the guest book. A song counts as sung before by its title or
/// by the record it was sung to.
#[must_use]
pub fn for_up_next(rotation: &Rotation, journal: &Journal) -> Option<GuideOffer> {
    let (singer, song) = rotation.up_next()?;
    let theirs = journal
        .guests
        .iter()
        .filter(|guest| same_name(&guest.name, &singer.name))
        .flat_map(|guest| guest.sang.iter());
    let mut titles = Vec::new();
    for sang in theirs {
        if song.track.is_some() && sang.track == song.track && !song.unsure {
            return None;
        }
        titles.push(sang.title.as_str());
    }
    offer(
        &singer.name,
        &song.title,
        song.voice.is_some(),
        song.unsure,
        &titles,
    )
}

/// [`for_up_next`], for the Singers panel's up-next card.
#[tauri::command]
#[must_use]
pub fn karaoke_guide_offer(state: State<'_, AppState>) -> Option<GuideOffer> {
    for_up_next(&state.karaoke(), &state.guests())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Offered where the song is new to them, and only there** — not for
    /// a song the guest book has them singing however it was written, not
    /// where they already chose a voice, and never for no song.
    #[test]
    fn the_guide_is_offered_for_a_song_new_to_the_singer() {
        let offered =
            offer("Ana", "Bachata Rosa", false, false, &["Vivir mi vida"]).expect("new to her");
        assert!(offered.line.starts_with("New to Ana"), "{}", offered.line);
        assert!(offered.line.contains("quietly underneath"));
        assert!(offer("Ana", "Bachata Rosa", false, false, &[]).is_some());

        assert_eq!(
            offer(
                "Ana",
                "Bachata Rosa",
                false,
                false,
                &["Juan Luis Guerra - Bachata Rosa"]
            ),
            None,
            "sung before, written with the artist"
        );
        assert_eq!(
            offer("Ana", "bachata rosa!", false, false, &["Bachata Rosa"]),
            None
        );
        assert_eq!(
            offer("Ana", "Bachata Rosa", true, false, &[]),
            None,
            "a voice already chosen"
        );
        assert_eq!(offer("Ana", "  ", false, false, &[]), None);
        assert!(
            offer("Ana", "Bachata Rosa", false, true, &["Bachata Rosa"]).is_some(),
            "not sure of a song sung before"
        );
        assert_eq!(
            offer("Ana", "Bachata Rosa", true, true, &[]),
            None,
            "a voice chosen is chosen, sure or not"
        );
        assert!(!sung_before(&["Rosa"], "Bachata Rosa"));
    }

    fn sang(title: &str, track: Option<&str>) -> crate::guests::Performance {
        crate::guests::Performance {
            title: title.into(),
            track: track.map(str::to_owned),
            ..Default::default()
        }
    }

    /// **The singer up next, against their own record in the guest book** —
    /// another guest's songs are not theirs, a name written differently is
    /// still them, and the same record under another title is the same song.
    #[test]
    fn the_offer_reads_the_singer_up_next_and_their_guest_book() {
        let mut rotation = Rotation::default();
        assert_eq!(
            for_up_next(&rotation, &Journal::default()),
            None,
            "nobody up next"
        );
        rotation.ask("Ana", "Bachata Rosa", Some("ab".into()), None, None);
        rotation.ask("Ben", "Hey Jude", None, None, None);
        let mut journal = Journal::default();
        journal.guests.push(crate::guests::Guest {
            name: "Ben".into(),
            sang: vec![sang("Bachata Rosa", Some("ab"))],
            ..Default::default()
        });
        let offered = for_up_next(&rotation, &journal).expect("Ben's songs are not Ana's");
        assert!(offered.line.starts_with("New to Ana"), "{}", offered.line);

        journal.guests.push(crate::guests::Guest {
            name: " ana ".into(),
            sang: vec![sang("Rosa (live)", Some("ab"))],
            ..Default::default()
        });
        assert_eq!(for_up_next(&rotation, &journal), None, "the same record");
        journal.guests[1].sang = vec![sang("Juan Luis Guerra - Bachata Rosa", None)];
        assert_eq!(for_up_next(&rotation, &journal), None, "the same title");
        journal.guests[1].sang = vec![sang("Ojalá que llueva café", Some("cd"))];
        assert!(for_up_next(&rotation, &journal).is_some());

        // Not sure of it: offered though the guest book has her singing it.
        journal.guests[1].sang = vec![sang("Rosa (live)", Some("ab"))];
        assert_eq!(for_up_next(&rotation, &journal), None);
        assert!(rotation.unsure_of_last(" ana "));
        let unsure = for_up_next(&rotation, &journal).expect("she is not sure of it");
        assert!(
            unsure.line.starts_with("Not sure of it, Ana"),
            "{}",
            unsure.line
        );
        assert!(!rotation.unsure_of_last("Nobody"), "nobody asked");
    }
}

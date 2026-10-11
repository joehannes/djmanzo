//! K9: a singer's song recorded for them (`docs/KARAOKE.md` §12).
//!
//! The engine streams their strip's share of the PA over the records as the
//! room hears them ([`dj_engine::Command::PerformanceStream`]); a thread of
//! its own writes it, as the set recording's does ([`crate::setrec`]); and
//! the file is on their record in the guest book from the moment it starts,
//! so *Sang* moves it onto the song like the voice take.

/// The longest a song is recorded for, in seconds: a song, not a night.
pub const MOST_SECONDS: f64 = 600.0;

/// A song being recorded.
#[derive(Debug)]
pub struct Run {
    /// The guest book's id for the singer.
    pub guest: String,
    /// The rotation's name for them.
    pub singer: String,
    /// The file's name in the guest book's folder.
    pub file: String,
    pub recording: crate::setrec::Recording,
}

/// The file a recording of guest `id`'s song started at `now` is kept as —
/// a name `crate::guests::is_voice_file` admits.
#[must_use]
pub fn file_for(id: &str, now: i64) -> String {
    format!("{id}-song-{now}.wav")
}

/// Whether the journal still holds `file` — on the song being sung or on
/// one sung — so a recording whose singer took their agreement back while
/// it ran is deleted when it ends.
#[must_use]
pub fn still_kept(journal: &crate::guests::Journal, file: &str) -> bool {
    journal.guests.iter().any(|guest| {
        guest.performing.as_deref() == Some(file)
            || guest
                .sang
                .iter()
                .any(|song| song.recording.as_deref() == Some(file))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A recording's name is one the guest book keeps**, and it is kept
    /// only while the journal holds it.
    #[test]
    fn a_recording_is_kept_while_the_journal_holds_it() {
        let file = file_for("g1a2", 1_700_000_000);
        assert!(crate::guests::is_voice_file(&file), "{file}");
        let mut journal = crate::guests::Journal::default();
        assert!(!still_kept(&journal, &file));
        journal.guests.push(crate::guests::Guest {
            performing: Some(file.clone()),
            ..crate::guests::Guest::default()
        });
        assert!(still_kept(&journal, &file));
        journal.guests[0].performing = None;
        journal.guests[0].sang.push(crate::guests::Performance {
            recording: Some(file.clone()),
            ..crate::guests::Performance::default()
        });
        assert!(still_kept(&journal, &file));
        journal.guests[0].sang[0].recording = None;
        assert!(!still_kept(&journal, &file));
    }
}

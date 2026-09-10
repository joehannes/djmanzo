//! What kind of night this is.
//!
//! §81 asks djmanzo not to keep one universal profile of its DJ but a
//! *conditional* one, and gives the shape:
//!
//! ```text
//! Johannes
//!   ├─ Club / Peak
//!   ├─ Beach / Sunset
//!   ├─ Wedding
//!   ├─ Latin
//!   ├─ Practice
//!   └─ Open Format
//! ```
//!
//! Those are not phases of a night. [`dj_core::SessionPhase`] is already the
//! arc *within* an evening — warming up, building, peak, coming down — and
//! every night has one. This is the other axis: what the evening **is**. A
//! wedding has a peak and so does a club, and what a DJ does at each is not the
//! same thing.
//!
//! # Nothing infers this
//!
//! djmanzo works out the phase of a night from the music, and it is right to:
//! energy, tempo and how records are being joined are all in the signal. There
//! is nothing in the signal that says *wedding*. A room full of people dancing
//! to the same record at 128 BPM is a club or a wedding or a beach bar
//! according to facts no microphone has — who is in it, what they were
//! promised, and what happens at eleven.
//!
//! So the setting is **told**, never guessed. That is not a limitation to be
//! engineered away later; guessing it is the §13 failure at the level of a
//! whole night. Learning "you play like this at weddings" from a night that was
//! not a wedding is worse than learning nothing, because it is confident and
//! it is filed under the wrong name.
//!
//! # Six, and why they are fixed
//!
//! The list is the directive's own. Free text would make "Wedding", "wedding"
//! and "Weddings" three profiles a DJ meant as one, each with a third of the
//! evidence and none of them reaching the threshold that lets it say anything
//! — which is a learning system quietly disabled by a capital letter.

use serde::{Deserialize, Serialize};

/// What kind of night this is. §81's list, and only that list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Setting {
    /// A club, at the hour a club is a club.
    Club,
    /// A beach bar at sunset, and anything else that is warm and unhurried.
    Beach,
    /// A wedding: a room that is not there for the DJ, and mostly does not
    /// dance for the first two hours.
    Wedding,
    /// A Latin night — the one genre-named entry in §81's own list, kept
    /// because it is there and because it names a different set of techniques
    /// rather than merely a different crate.
    Latin,
    /// Practising. Not a gig, and what is learned here should not be mistaken
    /// for what is done in front of people.
    Practice,
    /// Open format: whatever the room turns out to want.
    OpenFormat,
}

impl Setting {
    /// Every setting, in the order §81 lists them.
    pub const ALL: [Setting; 6] = [
        Setting::Club,
        Setting::Beach,
        Setting::Wedding,
        Setting::Latin,
        Setting::Practice,
        Setting::OpenFormat,
    ];

    /// The slug it is stored and spoken as.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Setting::Club => "club",
            Setting::Beach => "beach",
            Setting::Wedding => "wedding",
            Setting::Latin => "latin",
            Setting::Practice => "practice",
            Setting::OpenFormat => "open-format",
        }
    }

    /// What a DJ would call it.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Setting::Club => "Club",
            Setting::Beach => "Beach / sunset",
            Setting::Wedding => "Wedding",
            Setting::Latin => "Latin",
            Setting::Practice => "Practice",
            Setting::OpenFormat => "Open format",
        }
    }

    /// One line, so a picker is a choice rather than a guess.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Setting::Club => "A crowd that came to dance, and knows what it came for.",
            Setting::Beach => "Warm, unhurried, and nobody is waiting for a drop.",
            Setting::Wedding => "A room that is not there for the DJ.",
            Setting::Latin => "Where the technique is the genre's, not the format's.",
            Setting::Practice => "Nobody is listening. What happens here is not a gig.",
            Setting::OpenFormat => "Whatever the room turns out to want.",
        }
    }

    /// Read back what [`Self::slug`] wrote.
    ///
    /// `None` rather than a fallback: a stored setting nobody recognises must
    /// not quietly become a club, because the whole point of §81 is that the
    /// profiles are kept apart.
    #[must_use]
    pub fn parse(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }
}

impl std::fmt::Display for Setting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every setting round-trips through its slug, so a profile stored tonight
    /// is the same profile tomorrow.
    #[test]
    fn a_setting_survives_being_written_down() {
        for setting in Setting::ALL {
            assert_eq!(Setting::parse(setting.slug()), Some(setting));
        }
    }

    /// **A slug djmanzo does not know is not a club.**
    ///
    /// A fallback here would file a night under the wrong profile and go on
    /// filing them there, which is §13's failure at the scale of a whole
    /// night: confident, wrong, and filed where nobody will look for it.
    #[test]
    fn an_unknown_setting_is_refused_rather_than_guessed() {
        assert_eq!(Setting::parse("Wedding"), None, "case is not forgiven");
        assert_eq!(Setting::parse("weddings"), None);
        assert_eq!(Setting::parse(""), None);
        assert_eq!(Setting::parse("bar-mitzvah"), None);
    }

    /// The slugs and the titles are both distinct: two settings that stored the
    /// same way would merge two profiles, and two that read the same way would
    /// make a picker a coin toss.
    #[test]
    fn no_two_settings_look_alike() {
        let slugs: std::collections::BTreeSet<_> = Setting::ALL.iter().map(|s| s.slug()).collect();
        let titles: std::collections::BTreeSet<_> =
            Setting::ALL.iter().map(|s| s.title()).collect();
        assert_eq!(slugs.len(), Setting::ALL.len());
        assert_eq!(titles.len(), Setting::ALL.len());
    }
}

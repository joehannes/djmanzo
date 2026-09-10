//! How this DJ plays, per kind of night.
//!
//! §81, in one sentence: *do not store one universal DJ profile; store
//! conditional profiles.* The directive draws it as a tree — Johannes, and
//! under him Club, Beach, Wedding, Latin, Practice, Open Format — and lists
//! what may differ between the branches: density, technique preferences, genre
//! weights, transition style, automation tolerance.
//!
//! # Why one profile is worse than none
//!
//! A DJ who plays bachata at weddings and techno at clubs, averaged, is a DJ
//! who plays neither. The average is not a compromise between two true things;
//! it is a third thing that describes no evening either of them had. Worse, it
//! is *confident* — it has twice the evidence of each real answer — so a system
//! that offers it will offer it strongly. That is §13's failure with more rows
//! behind it, and the reason §81 exists.
//!
//! # The same discipline as a tendency, one level up
//!
//! [`crate::signals::Tendency`] is constructible only through `tendencies()`,
//! never without a phase and never on fewer than four occurrences in that
//! phase. A [`Profile`] is the same shape of promise about a whole night: it
//! has private fields, it can only be made by [`profiles`], and it is not made
//! at all until there are [`ENOUGH_NIGHTS`] of that setting. "How you play at
//! weddings" after one wedding is a description of one evening wearing the
//! word *always*.
//!
//! Each field then answers separately, because they arrive at different rates.
//! A night has a density from its first minute and a commonest transition style
//! only once it has had transitions, so a profile with plenty of nights behind
//! it can still have nothing to say about one of the five. `None` is a real
//! answer and is drawn as one.
//!
//! # Derived where it can be, kept where it cannot
//!
//! Genre weights are **derived**: the plays are in `history` and the genres are
//! on the tracks, so a stored weight would be a second copy that drifts the
//! first time a record is re-tagged.
//!
//! The other four are read off the **action log**, and the action log does not
//! outlive the run of the application that made it. So they are written to the
//! night's row as the night goes — see the `nights` table. That is not a second
//! copy of the log; it is the only trace that survives it.

use std::collections::BTreeMap;

use crate::setting::Setting;
use crate::signals::Did;
use dj_assistant::posture::Posture;
use dj_core::action::TransitionStyle;
use dj_library::Night;

/// How many nights of a setting before djmanzo will generalise about it.
///
/// Three. One night is an evening; two is a coincidence; three is the smallest
/// number that can show a habit rather than a repeat. The same argument
/// `signals::ENOUGH` makes about gestures, at the scale of a whole night —
/// and deliberately smaller, because nights are rarer than gestures and a
/// threshold nobody reaches is a feature that never speaks.
pub const ENOUGH_NIGHTS: usize = 3;

/// How much of the evidence has to agree before a single answer is given.
///
/// Half. A transition style used on two of five wedding nights is not "how you
/// mix at weddings", it is what you did twice — and a profile that reported it
/// anyway would be putting the DJ's own minority behaviour to them as their
/// habit.
const AGREE: f64 = 0.5;

/// What djmanzo has worked out about how this DJ plays in one setting.
///
/// Private fields on purpose: every one of them is a claim that needed enough
/// evidence, and a struct anyone can build is a claim anyone can make.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    setting: Setting,
    nights: usize,
    density: Option<String>,
    style: Option<TransitionStyle>,
    automation: Option<Posture>,
    techniques: Vec<Did>,
    genres: Vec<(String, f64)>,
}

impl Profile {
    /// Which kind of night this is about. There is no profile without one.
    #[must_use]
    pub fn setting(&self) -> Setting {
        self.setting
    }

    /// How many nights it rests on, so an interface can say so rather than
    /// presenting three nights and thirty with the same face.
    #[must_use]
    pub fn nights(&self) -> usize {
        self.nights
    }

    /// The interface density this DJ actually runs at here, when enough nights
    /// agree.
    #[must_use]
    pub fn density(&self) -> Option<&str> {
        self.density.as_deref()
    }

    /// How they mostly join records here.
    #[must_use]
    pub fn style(&self) -> Option<TransitionStyle> {
        self.style
    }

    /// §81's "automation tolerance": how much they let the assistant do here.
    #[must_use]
    pub fn automation(&self) -> Option<Posture> {
        self.automation
    }

    /// The gestures that show up on most of these nights.
    #[must_use]
    pub fn techniques(&self) -> &[Did] {
        &self.techniques
    }

    /// What gets played here, as shares of the plays, commonest first. Sums to
    /// one over the genres present; records with no genre are left out rather
    /// than counted as a genre called nothing.
    #[must_use]
    pub fn genres(&self) -> &[(String, f64)] {
        &self.genres
    }

    /// One sentence, written here rather than in the interface.
    ///
    /// The same rule §13 follows: the words are Rust's, so a panel cannot
    /// assemble a claim out of the parts that djmanzo would not make. This one
    /// always names the setting and always names how many nights it is from,
    /// because a profile read without either is exactly the universal profile
    /// §81 forbids.
    #[must_use]
    pub fn words(&self) -> String {
        // The title as it is written, not lower-cased into a sentence: "at
        // wedding" and "at beach / sunset" both read badly, and the six titles
        // have no single preposition that fits them all. Leading with the name
        // works for every one of them.
        let mut said = format!(
            "{}, over {} night{}",
            self.setting.title(),
            self.nights,
            if self.nights == 1 { "" } else { "s" }
        );
        if let Some(style) = self.style {
            said.push_str(&format!(": mostly {style} transitions"));
        } else {
            said.push_str(": too varied so far to say how you mix");
        }
        if let Some((genre, share)) = self.genres.first() {
            said.push_str(&format!(
                ", {}% {genre}",
                (share * 100.0).round() as i64,
                genre = genre
            ));
        }
        if let Some(posture) = self.automation {
            said.push_str(&format!(", assistant on {}", posture.name()));
        }
        said.push('.');
        said
    }
}

/// Build every profile there is enough evidence for.
///
/// `genres` is looked up per setting by the caller, which is the layer with a
/// database; this module stays a pure function over what it is handed, the way
/// `signals::tendencies` does.
///
/// Settings with fewer than [`ENOUGH_NIGHTS`] produce **nothing at all** —
/// not an empty profile, which an interface would draw as a profile that knows
/// nothing rather than as one that does not exist yet.
#[must_use]
pub fn profiles(nights: &[Night], genres: &dyn Fn(Setting) -> Vec<(String, u32)>) -> Vec<Profile> {
    let mut per_setting: BTreeMap<Setting, Vec<&Night>> = BTreeMap::new();
    for night in nights {
        // A stored setting nobody recognises is skipped rather than folded
        // into open format — see `Setting::parse`.
        if let Some(setting) = Setting::parse(&night.setting) {
            per_setting.entry(setting).or_default().push(night);
        }
    }

    per_setting
        .into_iter()
        .filter(|(_, seen)| seen.len() >= ENOUGH_NIGHTS)
        .map(|(setting, seen)| Profile {
            setting,
            nights: seen.len(),
            density: agreed(seen.iter().map(|n| n.density.clone())),
            style: agreed(seen.iter().map(|n| n.style.clone()))
                .and_then(|word| TransitionStyle::parse(&word)),
            automation: agreed(seen.iter().map(|n| n.posture.clone()))
                .and_then(|word| Posture::ALL.into_iter().find(|p| p.name() == word)),
            techniques: usual(&seen),
            genres: shares(genres(setting)),
        })
        .collect()
}

/// The commonest answer, if enough of those who answered agree.
///
/// Counted over the nights that said *something*, not over all of them: a
/// night with no transitions has no opinion about transition style, and
/// counting it as a vote against would make a habit disappear as soon as the
/// DJ had a quiet evening.
fn agreed(values: impl Iterator<Item = Option<String>>) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut answered = 0;
    for value in values.flatten() {
        if value.is_empty() {
            continue;
        }
        answered += 1;
        *counts.entry(value).or_default() += 1;
    }
    if answered == 0 {
        return None;
    }
    // Ties break by the name, which is arbitrary and stable — an answer that
    // changed each time it was asked would be worse than either.
    let (best, count) = counts
        .into_iter()
        .max_by_key(|(name, n)| (*n, name.clone()))?;
    #[allow(clippy::cast_precision_loss)]
    let share = count as f64 / answered as f64;
    (share >= AGREE).then_some(best)
}

/// The gestures that turn up on most of the nights.
///
/// Per *night*, not per occurrence: a DJ who looped forty times in one evening
/// and never again has done a thing once, and counting the forty would make it
/// their signature.
fn usual(nights: &[&Night]) -> Vec<Did> {
    let mut counts: BTreeMap<Did, usize> = BTreeMap::new();
    for night in nights {
        let Some(list) = night.techniques.as_deref() else {
            continue;
        };
        // Each gesture counts once per night however many times it names it.
        let mut seen: Vec<Did> = list.split(',').filter_map(Did::parse).collect();
        seen.sort_by_key(|d| d.slug());
        seen.dedup();
        for did in seen {
            *counts.entry(did).or_default() += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let floor = (nights.len() as f64 * AGREE).ceil() as usize;
    let mut out: Vec<(Did, usize)> = counts
        .into_iter()
        .filter(|(_, n)| *n >= floor.max(1))
        .collect();
    out.sort_by_key(|(did, n)| (std::cmp::Reverse(*n), did.slug()));
    out.into_iter().map(|(did, _)| did).collect()
}

/// Play counts as shares of the total, commonest first.
fn shares(counts: Vec<(String, u32)>) -> Vec<(String, f64)> {
    let total: u32 = counts.iter().map(|(_, n)| *n).sum();
    if total == 0 {
        return Vec::new();
    }
    counts
        .into_iter()
        .map(|(genre, n)| (genre, f64::from(n) / f64::from(total)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn night(id: &str, setting: &str) -> Night {
        Night {
            session_id: id.to_owned(),
            setting: setting.to_owned(),
            began_at: 0,
            density: None,
            style: None,
            posture: None,
            techniques: None,
        }
    }

    fn nights(setting: &str, count: usize) -> Vec<Night> {
        (0..count)
            .map(|n| night(&format!("{setting}-{n}"), setting))
            .collect()
    }

    fn nothing(_: Setting) -> Vec<(String, u32)> {
        Vec::new()
    }

    /// **§81's whole point: two settings are two answers, never their
    /// average.**
    ///
    /// A DJ who plays bachata at weddings and techno at clubs, averaged, is a
    /// DJ who plays neither — and the average has twice the evidence of either
    /// real answer, so a system offering it would offer it strongly.
    #[test]
    fn a_wedding_and_a_club_are_two_profiles_and_not_one() {
        let mut rows = nights("wedding", 3);
        rows.extend(nights("club", 3));
        for row in &mut rows {
            row.style = Some(
                if row.setting == "wedding" {
                    "fade"
                } else {
                    "cut"
                }
                .to_owned(),
            );
        }

        let genres = |setting: Setting| match setting {
            Setting::Wedding => vec![("Bachata".to_owned(), 9), ("Techno".to_owned(), 1)],
            Setting::Club => vec![("Techno".to_owned(), 10)],
            _ => Vec::new(),
        };
        let built = profiles(&rows, &genres);

        assert_eq!(built.len(), 2, "two settings collapsed into {built:#?}");
        let wedding = built
            .iter()
            .find(|p| p.setting() == Setting::Wedding)
            .expect("no wedding profile");
        let club = built
            .iter()
            .find(|p| p.setting() == Setting::Club)
            .expect("no club profile");

        assert_eq!(wedding.style(), Some(TransitionStyle::Fade));
        assert_eq!(club.style(), Some(TransitionStyle::Cut));
        assert_eq!(wedding.genres()[0].0, "Bachata");
        assert_eq!(club.genres()[0].0, "Techno");
        // And neither is a blend of the two.
        assert!(
            wedding
                .genres()
                .iter()
                .all(|(g, share)| g != "Techno" || *share < 0.2)
        );
    }

    /// **One night is not a habit.**
    ///
    /// The §13 rule at the scale of a whole evening. A setting under the
    /// threshold produces *nothing*, not an empty profile — an interface draws
    /// an empty profile as one that knows nothing about you, which is a
    /// different and much less useful claim than one that does not exist yet.
    #[test]
    fn a_setting_below_the_threshold_produces_no_profile_at_all() {
        // The numbers, not `ENOUGH_NIGHTS`. Written against the constant this
        // test moves with it and defends nothing — lowering the threshold to
        // one left it green, which is the whole judgement gone. **One evening
        // is not a habit and neither is two.**
        for count in [0, 1, 2] {
            assert!(
                profiles(&nights("beach", count), &nothing).is_empty(),
                "{count} night(s) was enough to generalise about a whole setting"
            );
        }
        assert_eq!(profiles(&nights("beach", 3), &nothing).len(), 1);
        assert_eq!(
            ENOUGH_NIGHTS, 3,
            "the threshold moved; the three cases above are the claim"
        );
    }

    /// **A minority is not reported as a habit.**
    ///
    /// Two blends out of five is what the DJ did twice, not how they mix.
    /// Reporting it would put their own outlier to them as their signature.
    #[test]
    fn a_field_stays_quiet_until_enough_of_the_nights_agree() {
        let mut rows = nights("club", 5);
        rows[0].style = Some("blend".to_owned());
        rows[1].style = Some("blend".to_owned());
        rows[2].style = Some("cut".to_owned());
        rows[3].style = Some("fade".to_owned());
        rows[4].style = Some("echo".to_owned());
        assert_eq!(profiles(&rows, &nothing)[0].style(), None);

        // Three of five agreeing is enough.
        rows[3].style = Some("blend".to_owned());
        assert_eq!(
            profiles(&rows, &nothing)[0].style(),
            Some(TransitionStyle::Blend)
        );
    }

    /// **A quiet night is not a vote against.**
    ///
    /// A night with no transitions has no opinion about transition style.
    /// Counting it as disagreement would make a real habit vanish the moment
    /// the DJ had an easy evening.
    #[test]
    fn a_night_with_nothing_to_say_is_not_counted_against() {
        let mut rows = nights("latin", 4);
        rows[0].style = Some("blend".to_owned());
        rows[1].style = Some("blend".to_owned());
        // rows[2] and rows[3] said nothing at all.
        assert_eq!(
            profiles(&rows, &nothing)[0].style(),
            Some(TransitionStyle::Blend),
            "two silent nights outvoted two that spoke"
        );
    }

    /// **A gesture counts once per night, not once per press.**
    ///
    /// Forty loops in one evening and none since is a thing done once. Counted
    /// per occurrence it would be the DJ's signature technique.
    #[test]
    fn a_technique_is_counted_per_night_rather_than_per_press() {
        let mut rows = nights("club", 4);
        rows[0].techniques = Some("looped,looped,looped,looped".to_owned());
        rows[1].techniques = Some("eq-moved".to_owned());
        rows[2].techniques = Some("eq-moved".to_owned());
        rows[3].techniques = Some("eq-moved".to_owned());

        let built = profiles(&rows, &nothing);
        assert_eq!(
            built[0].techniques(),
            &[Did::EqMoved],
            "one busy night outranked three consistent ones"
        );
    }

    /// A stored setting djmanzo does not know belongs to no profile, rather
    /// than to the default one — the same refusal `Setting::parse` makes.
    #[test]
    fn a_night_of_an_unknown_setting_joins_nothing() {
        let mut rows = nights("bar-mitzvah", 5);
        rows.extend(nights("club", 3));
        let built = profiles(&rows, &nothing);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].setting(), Setting::Club);
        assert_eq!(built[0].nights(), 3, "unknown nights were counted as club");
    }

    /// **The sentence always names the setting and how many nights it is
    /// from.**
    ///
    /// A profile read without either is exactly the universal profile §81
    /// forbids — and the words are written here so that a panel cannot
    /// assemble the parts into a claim djmanzo would not make.
    #[test]
    fn the_words_never_drop_the_setting_or_the_evidence() {
        let mut rows = nights("wedding", 3);
        for row in &mut rows {
            row.style = Some("fade".to_owned());
            row.posture = Some("prepare".to_owned());
        }
        let said = profiles(&rows, &|_| vec![("Bachata".to_owned(), 4)])[0].words();
        assert!(said.contains("Wedding"), "{said}");
        assert!(said.contains("3 nights"), "{said}");
        assert!(said.contains("fade"), "{said}");
        assert!(said.contains("Bachata"), "{said}");
        assert!(said.contains("prepare"), "{said}");
    }

    /// And it says so plainly when it cannot say how you mix, rather than
    /// leaving a sentence that reads as though it had.
    #[test]
    fn it_admits_when_it_cannot_say_how_you_mix() {
        let said = profiles(&nights("practice", 3), &nothing)[0].words();
        assert!(said.contains("too varied"), "{said}");
        assert!(said.contains("Practice"), "{said}");
    }

    /// Genre shares are shares: they sum to one over what was played.
    #[test]
    fn genre_weights_are_shares_of_what_was_played() {
        let genres = |_| vec![("Bachata".to_owned(), 3), ("Salsa".to_owned(), 1)];
        let built = profiles(&nights("latin", 3), &genres);
        let shares = built[0].genres();
        assert_eq!(shares[0], ("Bachata".to_owned(), 0.75));
        assert_eq!(shares[1], ("Salsa".to_owned(), 0.25));
        let total: f64 = shares.iter().map(|(_, s)| s).sum();
        assert!((total - 1.0).abs() < 1e-9);
    }
}

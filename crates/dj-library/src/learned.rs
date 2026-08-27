//! What a DJ actually plays, as opposed to what they say they like.
//!
//! [`setlist::Taste`](crate::setlist::Taste) is typed in: a DJ names the
//! families they want favoured. That is right for "no country at my wedding",
//! and useless for the thing a DJ cannot easily say — which of the forty
//! genres in their collection they *reach for*.
//!
//! # The comparison that makes this mean anything
//!
//! Counting plays per family learns the shape of the collection, not the DJ.
//! Somebody whose library is nine-tenths bachata will play mostly bachata
//! whatever they feel about it, and a suggester built on that count would
//! confidently recommend the thing the DJ already cannot avoid.
//!
//! So a leaning here is a **ratio**: how often a family is played against how
//! often *owning* it would predict. One means "exactly as often as it turns up
//! in the library" — no information. Two means the DJ reaches for it twice as
//! often as chance. That is the number worth acting on, and it is the whole
//! design.
//!
//! # Why recency
//!
//! Taste drifts. A family played constantly two years ago and never since is
//! history, not preference. Plays are therefore weighted by a half-life, so
//! last month counts for more than last year without a cliff-edge where a
//! record stops mattering overnight.
//!
//! # What this deliberately will not do
//!
//! **It never learns an avoidance.** A family owned and never played is a
//! family the DJ has not got round to as easily as one they dislike, and the
//! two are indistinguishable from here. Avoiding stays an explicit choice,
//! because it is honoured strictly and a wrong guess silently removes music
//! from a night.

use crate::record::LibraryTrack;
use dj_core::genre;
use std::collections::BTreeMap;

/// How long it takes a play to count half as much.
///
/// A hundred and eighty days. Long enough that a DJ who works seasonally --
/// and most do -- still has last summer counted, and short enough that a phase
/// two years ago has faded to a sixteenth. Ninety days would erase a whole
/// season every time one turned over.
const HALF_LIFE_DAYS: f64 = 180.0;

/// The fewest plays worth drawing a conclusion from.
///
/// About one night's worth. Under this the ratios are noise: two plays of a
/// family the DJ owns one copy of is a leaning of enormous size and no
/// meaning.
const ENOUGH_PLAYS: usize = 20;

/// A play, as far as this needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Played {
    /// The genre tag on the track that was played, if it had one.
    pub genre: Option<String>,
    /// Unix seconds.
    pub at: i64,
}

/// What the history says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Learned {
    /// Per family: plays relative to what owning it would predict.
    ///
    /// One is no information. Above one, the DJ reaches for it more often than
    /// chance; below, less. Families neither owned nor played are absent
    /// rather than one, so a caller can tell "no leaning" from "not seen".
    pub leaning: BTreeMap<&'static str, f32>,
    /// How many plays this was drawn from.
    pub plays: usize,
}

impl Learned {
    /// Whether there is enough history to act on.
    #[must_use]
    pub fn is_confident(&self) -> bool {
        self.plays >= ENOUGH_PLAYS
    }

    /// The families most reached for, strongest first.
    ///
    /// Empty until there is enough history, so a caller that hands these
    /// straight to [`Taste::favours`](crate::setlist::Taste) cannot tilt a
    /// whole night on four plays.
    #[must_use]
    pub fn favourites(&self, most: usize) -> Vec<String> {
        if !self.is_confident() {
            return Vec::new();
        }
        let mut ranked: Vec<_> = self
            .leaning
            .iter()
            // Only a leaning that is actually a leaning. At 1.0 the family is
            // played exactly as often as it is owned, which says nothing.
            .filter(|(_, lean)| **lean > 1.0)
            .collect();
        // By strength, then by name so the answer is stable run to run.
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
        ranked
            .into_iter()
            .take(most)
            .map(|(name, _)| (*name).to_string())
            .collect()
    }

    /// The most a leaning may move a suggestion's score.
    ///
    /// Three quarters of a point, against a scale where a same-key match is
    /// worth three and a key clash minus two and a half. So taste can reorder
    /// records that would all work — it may prefer the DJ's own family among
    /// two valid keys — and can never lift one that would not: the gap it
    /// would have to cross is more than seven times this.
    ///
    /// Taste breaks ties. It does not overrule the mixing.
    const MOST_IT_MAY_MOVE: f64 = 0.75;

    /// How much this track's family should move its score, up or down.
    ///
    /// **Added, never multiplied.** A suggestion's score is signed — a key
    /// clash is negative — and multiplying a negative by a number above one
    /// makes it *better*, which would promote exactly the records taste should
    /// be pushing down.
    ///
    /// Logarithmic, because a leaning is a ratio: twice as often as chance and
    /// half as often as chance are equal and opposite, and only log space says
    /// so. One — no information — sits at zero by construction.
    #[must_use]
    pub fn tilt_for(&self, track: &LibraryTrack) -> f64 {
        // No confidence check here: `leaning_for` already returns a flat 1.0
        // until there is enough history, and log2(1) is 0. A second guard
        // would read as a safeguard and be unreachable.
        let lean = f64::from(self.leaning_for(track));
        if lean <= 0.0 {
            return 0.0;
        }
        lean.log2()
            .clamp(-Self::MOST_IT_MAY_MOVE, Self::MOST_IT_MAY_MOVE)
    }

    /// This track's family leaning, or `1.0` when there is nothing to say.
    ///
    /// Neutral rather than zero for the unknown cases -- an untagged track, a
    /// family with no history, a library too thin to have learned anything.
    /// Most collections are half-tagged and penalising that would hide half of
    /// one.
    #[must_use]
    pub fn leaning_for(&self, track: &LibraryTrack) -> f32 {
        if !self.is_confident() {
            return 1.0;
        }
        track
            .tags
            .genre
            .as_deref()
            .and_then(genre::family_for)
            .and_then(|family| self.leaning.get(family.name).copied())
            .unwrap_or(1.0)
    }
}

/// How much a play that happened `at` still counts, at `now`.
fn recency(at: i64, now: i64) -> f64 {
    // A play stamped in the future -- a clock that ran fast, an imported
    // history -- counts as fully current rather than more than current.
    let days = ((now - at).max(0) as f64) / 86_400.0;
    0.5_f64.powf(days / HALF_LIFE_DAYS)
}

/// Learn from a history, against the collection it was played from.
///
/// `owned` is every track's genre tag, which is what turns raw counts into the
/// ratio the module docs describe. Without it this would learn the library.
#[must_use]
pub fn learn(played: &[Played], owned: &[Option<String>], now: i64) -> Learned {
    let family_of = |tag: &Option<String>| tag.as_deref().and_then(genre::family_for);

    // What the collection is made of.
    let mut owned_count: BTreeMap<&'static str, f64> = BTreeMap::new();
    let mut owned_total = 0.0;
    for tag in owned {
        if let Some(family) = family_of(tag) {
            *owned_count.entry(family.name).or_default() += 1.0;
            owned_total += 1.0;
        }
    }

    // What was reached for, discounted by age.
    let mut played_weight: BTreeMap<&'static str, f64> = BTreeMap::new();
    let mut played_total = 0.0;
    let mut plays = 0;
    for play in played {
        if let Some(family) = family_of(&play.genre) {
            let weight = recency(play.at, now);
            *played_weight.entry(family.name).or_default() += weight;
            played_total += weight;
            plays += 1;
        }
    }

    if owned_total <= 0.0 || played_total <= 0.0 {
        return Learned {
            leaning: BTreeMap::new(),
            plays,
        };
    }

    let leaning = played_weight
        .into_iter()
        .filter_map(|(family, weight)| {
            // A family played but no longer owned cannot be compared against
            // anything, so it is left out rather than given an enormous
            // leaning by dividing by almost nothing.
            let owned_share = owned_count.get(family).copied()? / owned_total;
            if owned_share <= 0.0 {
                return None;
            }
            let played_share = weight / played_total;
            // Safe: a share ratio, bounded by the sizes of two finite sets.
            Some((family, (played_share / owned_share) as f32))
        })
        .collect();

    Learned { leaning, plays }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;
    const NOW: i64 = 1_800_000_000;

    fn played(genre: &str, days_ago: i64) -> Played {
        Played {
            genre: Some(genre.to_string()),
            at: NOW - days_ago * DAY,
        }
    }

    fn owned(genre: &str, how_many: usize) -> Vec<Option<String>> {
        vec![Some(genre.to_string()); how_many]
    }

    /// A track that exists only to carry a genre tag.
    ///
    /// Local rather than shared with `store`'s fixtures: everything below asks
    /// one question of one field, and borrowing a fixture built for database
    /// round-trips would tie these tests to changes that have nothing to do
    /// with them.
    fn tagged(byte: u8, genre: &str) -> LibraryTrack {
        LibraryTrack {
            id: dj_core::TrackId::from_bytes([byte; 32]),
            path: std::path::PathBuf::from("/music/x.flac"),
            tags: crate::record::Tags {
                genre: Some(genre.to_string()),
                ..crate::record::Tags::default()
            },
            duration_frames: 48_000 * 200,
            sample_rate: dj_core::SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: crate::record::StoredAnalysis::default(),
            stats: crate::record::PlayStats::default(),
            colour: None,
        }
    }

    /// **A leaning is a comparison, not a count.**
    ///
    /// The property the whole module exists for. A DJ whose library is almost
    /// entirely bachata will play mostly bachata whatever they think of it;
    /// what says something is playing techno far more often than owning three
    /// of them would predict.
    #[test]
    fn playing_what_you_own_most_of_is_not_a_preference() {
        let mut collection = owned("bachata", 90);
        collection.extend(owned("techno", 10));

        // Forty plays split evenly -- so techno is reached for four times
        // more often than the shelf would suggest, and bachata less.
        let history: Vec<_> = (0..20)
            .flat_map(|i| [played("bachata", i), played("techno", i)])
            .collect();

        let learned = learn(&history, &collection, NOW);
        let bachata = learned.leaning["bachata"];
        let techno = learned.leaning["techno"];
        assert!(techno > 2.0, "techno leaning was {techno}");
        assert!(bachata < 1.0, "bachata leaning was {bachata}");
        assert_eq!(learned.favourites(3), vec!["techno"]);
    }

    /// **Playing exactly what you own says nothing.**
    #[test]
    fn a_collection_played_evenly_produces_no_favourites() {
        let mut collection = owned("bachata", 50);
        collection.extend(owned("techno", 50));
        let history: Vec<_> = (0..15)
            .flat_map(|i| [played("bachata", i), played("techno", i)])
            .collect();

        let learned = learn(&history, &collection, NOW);
        for (family, lean) in &learned.leaning {
            assert!(
                (lean - 1.0).abs() < 0.05,
                "{family} leaned {lean} on an evenly played collection"
            );
        }
        assert!(learned.favourites(3).is_empty());
    }

    /// **Last month counts for more than two years ago.**
    ///
    /// Taste drifts, and a phase the DJ has left should stop steering the
    /// suggester without a cliff edge where it vanishes overnight.
    #[test]
    fn what_was_played_recently_weighs_more() {
        let mut collection = owned("bachata", 50);
        collection.extend(owned("techno", 50));

        let mut history: Vec<_> = (0..20).map(|i| played("techno", i)).collect();
        // The same number of bachata plays, but from a phase two years gone.
        history.extend((0..20).map(|i| played("bachata", 730 + i)));

        let learned = learn(&history, &collection, NOW);
        assert!(
            learned.leaning["techno"] > learned.leaning["bachata"] * 4.0,
            "recent {} vs old {}",
            learned.leaning["techno"],
            learned.leaning["bachata"]
        );
        assert_eq!(learned.favourites(1), vec!["techno"]);
    }

    /// **Four plays is not a taste.**
    ///
    /// Under about a night's worth the ratios are noise, and a caller that
    /// handed them to a setlist would tilt a whole evening on an accident.
    #[test]
    fn too_little_history_produces_no_opinion() {
        let mut collection = owned("bachata", 90);
        collection.extend(owned("techno", 10));
        let history = vec![played("techno", 1), played("techno", 2)];

        let learned = learn(&history, &collection, NOW);
        assert!(!learned.is_confident());
        assert!(learned.favourites(3).is_empty());
        // And the per-track weight is neutral rather than confidently wrong.
        assert_eq!(learned.plays, 2);
    }

    /// **A family played but no longer owned is left out, not made enormous.**
    ///
    /// Dividing by an owned share of zero would put a deleted genre at the top
    /// of the favourites for ever.
    #[test]
    fn a_genre_that_left_the_library_does_not_dominate() {
        let collection = owned("bachata", 100);
        let mut history: Vec<_> = (0..20).map(|i| played("bachata", i)).collect();
        history.extend((0..5).map(|i| played("techno", i)));

        let learned = learn(&history, &collection, NOW);
        assert!(
            !learned.leaning.contains_key("techno"),
            "an unowned genre was given a leaning"
        );
        assert!(learned.leaning.contains_key("bachata"));
    }

    /// **Untagged music is neutral, not suspicious.**
    ///
    /// Most collections are half-tagged. Treating an unknown genre as disliked
    /// would hide half of one.
    #[test]
    fn tracks_with_no_genre_are_ignored_rather_than_penalised() {
        let mut collection = owned("bachata", 50);
        collection.push(None);
        let mut history: Vec<_> = (0..20).map(|i| played("bachata", i)).collect();
        history.push(Played {
            genre: None,
            at: NOW,
        });

        let learned = learn(&history, &collection, NOW);
        assert!(learned.leaning.contains_key("bachata"));
        assert_eq!(learned.leaning.len(), 1);
    }

    /// **Taste never promotes a mix that does not work.**
    ///
    /// The bound this whole design rests on. A DJ who plays a great deal of
    /// techno should see techno float up among records that all fit — and
    /// should never be handed a key clash because of it.
    #[test]
    fn a_favoured_genre_cannot_out_rank_a_record_that_actually_fits() {
        let mut collection = owned("bachata", 90);
        collection.extend(owned("techno", 10));
        let history: Vec<_> = (0..30).map(|i| played("techno", i % 30)).collect();
        let learned = learn(&history, &collection, NOW);
        assert!(learned.is_confident());

        let loved = tagged(1, "techno");
        let neutral = tagged(2, "bachata");

        let tilt_loved = learned.tilt_for(&loved);
        let tilt_neutral = learned.tilt_for(&neutral);
        assert!(tilt_loved > 0.0, "a played family did not float up");

        // A key clash is -2.5 and a same-key match +3.0. Even at full swing
        // in opposite directions, taste cannot close that.
        let swing = tilt_loved - tilt_neutral;
        assert!(
            swing < 5.5,
            "taste could move a record {swing} points, across the {} that \
             separates a clash from a match",
            5.5
        );
    }

    /// **However lopsided the history, taste stays inside its bound.**
    ///
    /// A DJ who owns one salsa record and plays it every night produces an
    /// enormous leaning. Unbounded, that would add several points to its score
    /// — enough to lift a key clash above a same-key match, which is exactly
    /// what taste is not allowed to do.
    #[test]
    fn an_extreme_leaning_still_cannot_overrule_the_mixing() {
        let learned = Learned {
            leaning: [("salsa", 400.0), ("techno", 0.001)].into_iter().collect(),
            plays: 500,
        };
        let adored = learned.tilt_for(&tagged(1, "salsa"));
        let shunned = learned.tilt_for(&tagged(2, "techno"));

        assert!(adored <= 0.75, "a favoured family moved {adored} points");
        assert!(shunned >= -0.75, "a shunned family moved {shunned} points");
        // The gap between a key clash (-2.5) and a same-key match (+3.0).
        assert!(
            adored - shunned < 5.5,
            "taste could swing {} points across a 5.5-point gap",
            adored - shunned
        );
    }

    /// **Twice as often and half as often are equal and opposite.**
    ///
    /// What the logarithm is for. On a linear scale a leaning of 2.0 would be
    /// worth +1.0 and 0.5 only -0.5, so an over-played family would always
    /// outweigh an under-played one by twice as much.
    #[test]
    fn a_leaning_and_its_inverse_pull_equally_hard() {
        let learned = Learned {
            leaning: [("techno", 2.0), ("bachata", 0.5)].into_iter().collect(),
            plays: 100,
        };
        let up = tagged(1, "techno");
        let down = tagged(2, "bachata");
        assert!((learned.tilt_for(&up) + learned.tilt_for(&down)).abs() < 1e-9);
    }

    /// **With too little history, taste moves nothing at all.**
    #[test]
    fn an_unlearned_taste_does_not_tilt() {
        let learned = Learned {
            leaning: [("techno", 5.0)].into_iter().collect(),
            plays: 3,
        };
        assert_eq!(learned.tilt_for(&tagged(1, "techno")), 0.0);
    }

    /// **A clock that ran fast does not invent a play from the future.**
    #[test]
    fn a_play_stamped_ahead_of_now_counts_as_current() {
        assert!((recency(NOW + 10 * DAY, NOW) - 1.0).abs() < f64::EPSILON);
        assert!((recency(NOW, NOW) - 1.0).abs() < f64::EPSILON);
        assert!((recency(NOW - 180 * DAY, NOW) - 0.5).abs() < 0.001);
    }

    #[test]
    fn nothing_at_all_is_not_a_crash() {
        let learned = learn(&[], &[], NOW);
        assert!(learned.leaning.is_empty());
        assert!(learned.favourites(3).is_empty());
        assert!(!learned.is_confident());
    }
}

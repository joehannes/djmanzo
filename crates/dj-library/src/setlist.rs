//! Building a set before it is played.
//!
//! The suggester answers "what next, given what is playing". This answers the
//! larger question: *given the room, the night and what I like, what is the
//! whole set?* -- and it does so by asking the suggester repeatedly, each
//! answer becoming the next question.
//!
//! # A set has a shape
//!
//! An hour of tracks that all mix is not a set. What makes it one is an **arc**:
//! it starts somewhere, goes somewhere, and comes back. The arc here is a
//! sequence of [`Trajectory`] over the length of the set, and it is what stops
//! the assembler producing sixty minutes of the same energy because every
//! individual step was locally optimal.
//!
//! # Taste is a weight, not a filter
//!
//! A DJ's preferred genres tilt the ranking; they do not restrict it. Two
//! reasons. A set that never leaves the DJ's four favourite families is the
//! set they would have built by hand, so the assembler adds nothing. And the
//! records that make a night are usually the ones just outside -- the assembler
//! that cannot reach them is only useful to somebody who did not need it.
//!
//! # What it will not do
//!
//! **It will not cross a rhythmic grammar mid-blend.** Dembow into
//! four-on-the-floor is a cut whatever the tempos say (see
//! [`dj_core::genre`]), so the assembler places such a pair at a point where a
//! cut is acceptable, or not at all.
//!
//! **It will not repeat a track**, and it will not put two tracks by the same
//! artist next to each other -- the first is an error, the second is the
//! commonest complaint about every automatic playlist ever built.

use crate::LibraryTrack;
use crate::suggest::{self, Playing};
use dj_core::{TrackId, Trajectory, genre};

/// The shape of a set.
///
/// Named for what a DJ would call it rather than for the numbers, because the
/// numbers are an implementation detail and the name is what goes on a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Arc {
    /// Up, and stay up. A support slot before the headliner.
    Rising,
    /// Up, peak, and down. A whole night in one set.
    #[default]
    Journey,
    /// Level throughout. A bar, a restaurant, a room where the music is not
    /// the point.
    Flat,
    /// Down. The last hour.
    Descent,
}

impl Arc {
    /// Where the set should be going, `0.0` at the start and `1.0` at the end.
    #[must_use]
    pub fn trajectory_at(self, through: f32) -> Trajectory {
        let through = through.clamp(0.0, 1.0);
        match self {
            Self::Flat => Trajectory::Hold,
            Self::Descent => Trajectory::Ease,
            Self::Rising => {
                if through < 0.7 {
                    Trajectory::Lift
                } else {
                    Trajectory::Hold
                }
            }
            Self::Journey => {
                // Three quarters climbing, a plateau, then down. The plateau
                // matters: a set that peaks and immediately descends feels like
                // an accident rather than a decision.
                if through < 0.55 {
                    Trajectory::Lift
                } else if through < 0.8 {
                    Trajectory::Hold
                } else {
                    Trajectory::Ease
                }
            }
        }
    }
}

/// What the DJ likes, and what the night is.
#[derive(Debug, Clone, Default)]
pub struct Taste {
    /// Genre families to favour, by name or alias. Empty means no preference.
    ///
    /// A weight rather than a filter -- see the module docs.
    pub favours: Vec<String>,
    /// Families to keep out entirely. A short list, and honoured strictly:
    /// "no country at my wedding" is not a preference to be balanced against
    /// other factors.
    pub avoids: Vec<String>,
}

impl Taste {
    /// How much this track's genre is wanted: `1.0` neutral, above for
    /// favoured, `0.0` for avoided.
    #[must_use]
    pub fn weight_for(&self, track: &LibraryTrack) -> f32 {
        let Some(tag) = track.tags.genre.as_deref() else {
            // Unknown genre is neutral, not suspicious. Most libraries are
            // half-tagged and penalising that would hide half the collection.
            return 1.0;
        };
        let family = genre::family_for(tag);
        let matches = |list: &[String]| {
            list.iter().any(|wanted| {
                genre::family_for(wanted).is_some_and(|w| family.is_some_and(|f| f.name == w.name))
                    || wanted.eq_ignore_ascii_case(tag)
            })
        };
        if matches(&self.avoids) {
            return 0.0;
        }
        if matches(&self.favours) { 1.6 } else { 1.0 }
    }
}

/// One track in an assembled set, and why it is there.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub track: TrackId,
    /// Where in the set it falls, `0.0` to `1.0`.
    pub through: f32,
    /// What the arc wanted at this point.
    pub trajectory: Trajectory,
    /// The suggester's reasons for it, following the previous track.
    pub reasons: Vec<suggest::Reason>,
}

/// Build a set.
///
/// `minutes` is a target, not a promise: the assembler stops when it runs out
/// of tracks that fit, and a short set of records that work is better than a
/// long one padded with records that do not.
///
/// `opener` may name where to start. Without one the assembler picks the
/// candidate best suited to the arc's opening trajectory, which for a Journey
/// means the quietest thing that still fits the taste.
#[must_use]
pub fn assemble(
    pool: &[LibraryTrack],
    arc: Arc,
    taste: &Taste,
    minutes: f64,
    opener: Option<TrackId>,
) -> Vec<Slot> {
    if pool.is_empty() || minutes <= 0.0 {
        return Vec::new();
    }

    let allowed: Vec<&LibraryTrack> = pool.iter().filter(|t| taste.weight_for(t) > 0.0).collect();
    if allowed.is_empty() {
        return Vec::new();
    }

    let mut chosen: Vec<Slot> = Vec::new();
    let mut used: Vec<TrackId> = Vec::new();
    let mut seconds = 0.0f64;
    let target = minutes * 60.0;

    // The opener. Named, or the best fit for the arc's first trajectory.
    let mut current: &LibraryTrack = match opener.and_then(|id| allowed.iter().find(|t| t.id == id))
    {
        Some(found) => found,
        None => pick_opener(&allowed, arc, taste),
    };

    used.push(current.id);
    chosen.push(Slot {
        track: current.id,
        through: 0.0,
        trajectory: arc.trajectory_at(0.0),
        reasons: Vec::new(),
    });
    seconds += length_seconds(current);

    while seconds < target {
        #[allow(clippy::cast_possible_truncation)]
        let through = (seconds / target) as f32;
        let trajectory = arc.trajectory_at(through);

        let now = Playing {
            key: current.analysis.key(),
            bpm: current.analysis.bpm,
            lufs: current.analysis.loudness_lufs,
            phrase_beats: current.analysis.phrase_beats,
        };

        let candidates: Vec<LibraryTrack> = allowed
            .iter()
            .filter(|t| !used.contains(&t.id))
            .filter(|t| !same_artist(current, t))
            .filter(|t| grammar_allows(current, t))
            .map(|t| (*t).clone())
            .collect();
        if candidates.is_empty() {
            break;
        }

        let mut ranked = suggest::rank(&now, trajectory, &candidates);
        // Taste tilts the ranking here rather than inside the suggester,
        // which knows about mixing and has no business knowing what anybody
        // likes.
        for entry in &mut ranked {
            if let Some(track) = candidates.iter().find(|t| t.id == entry.track) {
                entry.score *= f64::from(taste.weight_for(track));
            }
        }
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.track.cmp(&b.track))
        });

        let Some(best) = ranked.first() else { break };
        let Some(next) = allowed.iter().find(|t| t.id == best.track) else {
            break;
        };

        used.push(next.id);
        chosen.push(Slot {
            track: next.id,
            through,
            trajectory,
            reasons: best.reasons.clone(),
        });
        seconds += length_seconds(next);
        current = next;
    }

    chosen
}

/// Where to start.
///
/// The quietest record that fits the taste when the arc climbs, the loudest
/// when it descends. A set that opens at full energy has nowhere to go, and one
/// that opens quietly when it is meant to descend has nothing to descend from.
fn pick_opener<'a>(allowed: &[&'a LibraryTrack], arc: Arc, taste: &Taste) -> &'a LibraryTrack {
    let climbing = matches!(arc.trajectory_at(0.0), Trajectory::Lift);
    allowed
        .iter()
        .copied()
        .max_by(|a, b| {
            let score = |t: &LibraryTrack| {
                let loudness = t.analysis.loudness_lufs.unwrap_or(-12.0);
                let tilted = if climbing { -loudness } else { loudness };
                tilted * f64::from(taste.weight_for(t))
            };
            score(a)
                .partial_cmp(&score(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                // Ties break on the id, so the same library gives the same set.
                .then_with(|| b.id.cmp(&a.id))
        })
        .unwrap_or(allowed[0])
}

/// Whether two tracks are by the same artist.
///
/// Back to back is the commonest complaint about every automatic playlist ever
/// built, and it is cheap to prevent.
fn same_artist(a: &LibraryTrack, b: &LibraryTrack) -> bool {
    match (a.tags.artist.as_deref(), b.tags.artist.as_deref()) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => false,
    }
}

/// Whether the rhythmic grammars permit putting these two next to each other.
///
/// Not a tempo question. See [`dj_core::genre`]: dembow and four-on-the-floor
/// have their kicks in different places, so holding them together is a mistake
/// whatever the numbers say. Unknown genres are permitted -- most libraries are
/// half-tagged, and refusing everything untagged would empty the pool.
fn grammar_allows(from: &LibraryTrack, to: &LibraryTrack) -> bool {
    let (Some(a), Some(b)) = (
        from.tags.genre.as_deref().and_then(genre::family_for),
        to.tags.genre.as_deref().and_then(genre::family_for),
    ) else {
        return true;
    };
    a.blends_with(b) != genre::Blendability::Cut
}

/// A track's length in seconds, or a sensible guess.
fn length_seconds(track: &LibraryTrack) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let seconds = track.duration_frames as f64 / track.sample_rate.as_f64();
    // An unanalysed or zero-length record still has to advance the clock, or
    // the assembler loops until it runs out of candidates.
    if seconds.is_finite() && seconds > 30.0 {
        seconds
    } else {
        210.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StoredAnalysis, Tags};
    use dj_core::{Mode, SampleRate};

    /// A candidate. Everything a set needs to be assembled from.
    fn track(byte: u8, bpm: f64, lufs: f64, genre: &str, artist: &str) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([byte; 32]),
            path: std::path::PathBuf::from(format!("/music/{byte}.wav")),
            tags: Tags {
                artist: Some(artist.to_owned()),
                genre: (!genre.is_empty()).then(|| genre.to_owned()),
                ..Tags::default()
            },
            // Three and a half minutes, so a 30-minute set is about nine tracks.
            duration_frames: 44_100 * 210,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: StoredAnalysis {
                bpm: Some(bpm),
                key_hour: Some(8),
                key_mode: Some(Mode::Minor),
                loudness_lufs: Some(lufs),
                phrase_beats: Some(16),
                phrase_anchor: Some(0),
                ..StoredAnalysis::default()
            },
            stats: crate::PlayStats::default(),
            colour: None,
        }
    }

    /// A pool wide enough to build from: house at a spread of loudnesses.
    fn pool() -> Vec<LibraryTrack> {
        (0..20u8)
            .map(|n| {
                track(
                    n + 1,
                    124.0,
                    -16.0 + f64::from(n) * 0.5,
                    "house",
                    &format!("artist {n}"),
                )
            })
            .collect()
    }

    /// **A set is assembled, and it is long enough.**
    #[test]
    fn a_set_is_built_to_roughly_the_length_asked_for() {
        let set = assemble(&pool(), Arc::Journey, &Taste::default(), 30.0, None);
        assert!(
            (8..=12).contains(&set.len()),
            "a 30 minute set came out as {} tracks of 3.5 minutes each",
            set.len()
        );
    }

    /// **No track is played twice.**
    #[test]
    fn a_set_never_repeats_a_track() {
        let set = assemble(&pool(), Arc::Journey, &Taste::default(), 60.0, None);
        let mut ids: Vec<TrackId> = set.iter().map(|s| s.track).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "the set repeats a track");
    }

    /// **No two tracks in a row by the same artist.**
    ///
    /// The commonest complaint about every automatic playlist ever built, and
    /// cheap to prevent.
    #[test]
    fn the_same_artist_never_plays_twice_in_a_row() {
        let mut pool = pool();
        // Half the pool by one artist, so an assembler that did not check
        // would certainly pair them.
        for t in pool.iter_mut().take(10) {
            t.tags.artist = Some("the same person".to_owned());
        }
        let set = assemble(&pool, Arc::Journey, &Taste::default(), 40.0, None);

        for pair in set.windows(2) {
            let artist_of = |id: TrackId| {
                pool.iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tags.artist.clone())
            };
            assert_ne!(
                artist_of(pair[0].track),
                artist_of(pair[1].track),
                "two tracks by the same artist ended up next to each other"
            );
        }
    }

    /// **A journey goes up and then comes down.**
    ///
    /// The thing that makes it a set rather than a list. Without an arc every
    /// step is locally optimal and the whole is sixty minutes at one energy.
    #[test]
    fn a_journey_climbs_and_then_descends() {
        assert_eq!(Arc::Journey.trajectory_at(0.0), Trajectory::Lift);
        assert_eq!(Arc::Journey.trajectory_at(0.65), Trajectory::Hold);
        assert_eq!(Arc::Journey.trajectory_at(0.95), Trajectory::Ease);
        // And the plateau exists: a set that peaks and immediately drops feels
        // like an accident.
        assert_eq!(Arc::Journey.trajectory_at(0.6), Trajectory::Hold);
    }

    /// A flat arc holds all the way through, which is what a bar wants.
    #[test]
    fn a_flat_arc_never_changes_direction() {
        for through in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(Arc::Flat.trajectory_at(through), Trajectory::Hold);
        }
    }

    /// **A climbing set opens quietly.**
    ///
    /// A set that opens at full energy has nowhere to go.
    #[test]
    fn a_rising_set_starts_with_the_quietest_record() {
        let set = assemble(&pool(), Arc::Rising, &Taste::default(), 30.0, None);
        let opener = set[0].track;
        let quietest = pool()
            .into_iter()
            .min_by(|a, b| {
                a.analysis
                    .loudness_lufs
                    .partial_cmp(&b.analysis.loudness_lufs)
                    .unwrap()
            })
            .unwrap();
        assert_eq!(
            opener, quietest.id,
            "a rising set did not start from the quietest record it had"
        );
    }

    /// **An avoided genre never appears.**
    ///
    /// Strictly, not as a weight to be balanced. "No country at my wedding" is
    /// not a preference.
    #[test]
    fn an_avoided_genre_is_kept_out_entirely() {
        let mut pool = pool();
        for t in pool.iter_mut().take(15) {
            t.tags.genre = Some("hardstyle".to_owned());
        }
        let taste = Taste {
            avoids: vec!["hardstyle".to_owned()],
            ..Taste::default()
        };
        let set = assemble(&pool, Arc::Flat, &taste, 60.0, None);

        for slot in &set {
            let track = pool.iter().find(|t| t.id == slot.track).unwrap();
            assert_ne!(
                track.tags.genre.as_deref(),
                Some("hardstyle"),
                "an avoided genre was played anyway"
            );
        }
    }

    /// **A favoured genre is a tilt, and the tilt is real.**
    ///
    /// Two claims, and an earlier version of this test made only the first.
    /// With the favour weight deleted entirely it still passed, because both
    /// genres appeared either way -- it proved the tilt was not a fence without
    /// proving there was a tilt at all. Mutation testing found that.
    #[test]
    fn a_favoured_genre_tilts_without_excluding() {
        let mut pool = pool();
        for t in pool.iter_mut().skip(10) {
            t.tags.genre = Some("disco".to_owned());
        }
        let count_house = |set: &[Slot]| {
            set.iter()
                .filter(|s| {
                    pool.iter()
                        .find(|t| t.id == s.track)
                        .and_then(|t| t.tags.genre.as_deref())
                        == Some("house")
                })
                .count()
        };

        let neutral = assemble(&pool, Arc::Flat, &Taste::default(), 60.0, None);
        let favoured = assemble(
            &pool,
            Arc::Flat,
            &Taste {
                favours: vec!["house".to_owned()],
                ..Taste::default()
            },
            60.0,
            None,
        );

        assert!(
            count_house(&favoured) > count_house(&neutral),
            "favouring house gave {} of them against {} without the preference, \
             so the tilt does nothing",
            count_house(&favoured),
            count_house(&neutral)
        );
        // And it is still not a fence: the records that make a night are
        // usually just outside what the DJ asked for.
        assert!(
            favoured.len() > count_house(&favoured),
            "the set never left the favoured genre, so the tilt is a fence"
        );
    }

    /// **The assembler will not blend across a rhythmic grammar.**
    ///
    /// Reggaetón into tech house is a cut whatever the tempos say. A set that
    /// placed them adjacent would be one a DJ could not actually play as
    /// written.
    #[test]
    fn dembow_and_four_on_the_floor_never_end_up_adjacent() {
        let mut pool: Vec<LibraryTrack> = (0..10u8)
            .map(|n| track(n + 1, 96.0, -10.0, "reggaeton", &format!("a{n}")))
            .collect();
        pool.extend((10..20u8).map(|n| track(n + 1, 126.0, -10.0, "tech house", &format!("b{n}"))));

        let set = assemble(&pool, Arc::Flat, &Taste::default(), 60.0, None);
        for pair in set.windows(2) {
            let genre_of = |id: TrackId| {
                pool.iter()
                    .find(|t| t.id == id)
                    .and_then(|t| t.tags.genre.clone())
            };
            let (a, b) = (genre_of(pair[0].track), genre_of(pair[1].track));
            assert_eq!(
                a, b,
                "the set puts {a:?} next to {b:?}, which is a change of rhythmic grammar"
            );
        }
    }

    /// **The same library builds the same set twice.**
    ///
    /// A DJ who asks again and gets a different answer cannot trust either.
    #[test]
    fn assembling_twice_gives_the_same_set() {
        let pool = pool();
        let first = assemble(&pool, Arc::Journey, &Taste::default(), 30.0, None);
        let second = assemble(&pool, Arc::Journey, &Taste::default(), 30.0, None);
        assert_eq!(
            first.iter().map(|s| s.track).collect::<Vec<_>>(),
            second.iter().map(|s| s.track).collect::<Vec<_>>()
        );
    }

    /// An empty library, or one entirely avoided, produces no set rather than
    /// a panic or a set of nothing.
    #[test]
    fn nothing_to_play_produces_no_set() {
        assert!(assemble(&[], Arc::Journey, &Taste::default(), 30.0, None).is_empty());
        let taste = Taste {
            avoids: vec!["house".to_owned()],
            ..Taste::default()
        };
        assert!(assemble(&pool(), Arc::Journey, &taste, 30.0, None).is_empty());
        assert!(assemble(&pool(), Arc::Journey, &Taste::default(), 0.0, None).is_empty());
    }

    /// A named opener is honoured.
    #[test]
    fn a_named_opener_starts_the_set() {
        let wanted = TrackId::from_bytes([7; 32]);
        let set = assemble(&pool(), Arc::Journey, &Taste::default(), 30.0, Some(wanted));
        assert_eq!(set[0].track, wanted);
    }
}

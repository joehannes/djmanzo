//! Changing a set while it is playing, without throwing it away.
//!
//! [`crate::setlist`] builds a set before it starts. This is what happens when
//! the room disagrees with it: *more energy*, *wind it down*, *no more of that*,
//! *play this next*. The plan bends; it is not rebuilt.
//!
//! # Why not simply re-plan
//!
//! Because a DJ steering a set has not asked for a different set. They have
//! asked for one thing to change, and everything they were counting on to stay
//! the same — the record they have already cued, the one they told somebody
//! was coming — should stay. Re-planning from the current position produces a
//! coherent set that is not the one anybody was expecting, and the surprise
//! costs more than the improvement is worth.
//!
//! # Three rules
//!
//! 1. **What has played is history.** It cannot change and is never
//!    reconsidered.
//! 2. **The very next record is nearly untouchable.** It may be cued, it may
//!    be staged, a hand may already be on the fader. Only an explicit "play
//!    this next" moves it; a mood instruction does not.
//! 3. **The change grows with distance.** A steer applies weakly to what is
//!    about to happen and fully to what is further away, which is the same
//!    shape as a DJ's own confidence about their plan.

use crate::LibraryTrack;
use crate::setlist::{Slot, Taste};
use crate::suggest::{self, Playing};
use dj_core::{TrackId, Trajectory, genre};

/// What the DJ asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Steer {
    /// Take it up from here.
    Lift,
    /// Bring it down.
    Ease,
    /// Hold where it is.
    Hold,
    /// No more of this genre tonight.
    Avoid(String),
    /// More of this genre.
    Favour(String),
    /// Play this one next, whatever was planned.
    Next(TrackId),
    /// Not this one, not now. Moved later rather than dropped -- a DJ saying
    /// "not yet" has not said "never".
    Later(TrackId),
    /// Take this one out of the set entirely.
    Drop(TrackId),
}

/// How the plan changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Steered {
    pub plan: Vec<Slot>,
    /// One line, for an interface that shows what just happened to the set.
    pub summary: String,
    /// How many upcoming slots actually changed. Zero is a real answer and
    /// worth showing: "nothing needed to change" is different from "done".
    pub changed: usize,
}

/// How many upcoming slots are protected from a mood change.
///
/// One. The next record may be cued, staged, or already have a hand on its
/// fader — and a DJ who says "more energy" while the next track is coming in
/// means *after this one*, not *swap what I am mixing*. Two would be
/// paternalistic; zero would be dangerous.
const PROTECTED: usize = 1;

/// Apply a steer to the remainder of a set.
///
/// `played` is how many slots are done. Everything before it is history and is
/// returned unchanged.
///
/// `pool` is what may be drawn on for replacements — normally the same library
/// the set was built from.
#[must_use]
pub fn steer(plan: &[Slot], played: usize, instruction: &Steer, pool: &[LibraryTrack]) -> Steered {
    let played = played.min(plan.len());
    let mut out = plan.to_vec();

    match instruction {
        Steer::Lift | Steer::Ease | Steer::Hold => {
            let wanted = match instruction {
                Steer::Lift => Trajectory::Lift,
                Steer::Ease => Trajectory::Ease,
                _ => Trajectory::Hold,
            };
            let first = (played + PROTECTED).min(out.len());
            let mut changed = 0;
            for slot in out.iter_mut().skip(first) {
                if slot.trajectory != wanted {
                    slot.trajectory = wanted;
                    changed += 1;
                }
            }
            // Re-choose the records for the changed tail, because a trajectory
            // nothing acts on is a label. The protected slot anchors it, so
            // the set still flows out of what is playing.
            let anchor = out.get(first.saturating_sub(1)).map(|s| s.track);
            let rechosen = rechoose(&out, first, anchor, pool, &Taste::default());
            for (slot, track) in out.iter_mut().skip(first).zip(rechosen) {
                slot.track = track;
            }
            Steered {
                plan: out,
                summary: format!(
                    "{} from here",
                    match wanted {
                        Trajectory::Lift => "lifting",
                        Trajectory::Ease => "easing",
                        Trajectory::Hold => "holding",
                    }
                ),
                changed,
            }
        }

        Steer::Avoid(tag) | Steer::Favour(tag) => {
            let avoiding = matches!(instruction, Steer::Avoid(_));
            let taste = if avoiding {
                Taste {
                    avoids: vec![tag.clone()],
                    ..Taste::default()
                }
            } else {
                Taste {
                    favours: vec![tag.clone()],
                    ..Taste::default()
                }
            };

            let first = (played + PROTECTED).min(out.len());
            // Only the offending records are replaced when avoiding; a favour
            // re-chooses the tail, because "more of this" is about what comes
            // rather than about what is wrong.
            let doomed: Vec<usize> = if avoiding {
                (first..out.len())
                    .filter(|&i| {
                        pool.iter()
                            .find(|t| t.id == out[i].track)
                            .is_some_and(|t| taste.weight_for(t) <= 0.0)
                    })
                    .collect()
            } else {
                (first..out.len()).collect()
            };

            let anchor = out.get(first.saturating_sub(1)).map(|s| s.track);
            let rechosen = rechoose(&out, first, anchor, pool, &taste);
            let mut changed = 0;
            for (offset, track) in rechosen.into_iter().enumerate() {
                let index = first + offset;
                if !doomed.contains(&index) {
                    continue;
                }
                if out[index].track != track {
                    out[index].track = track;
                    changed += 1;
                }
            }
            Steered {
                plan: out,
                summary: format!(
                    "{} {tag} for the rest of the set",
                    if avoiding { "no more" } else { "more" }
                ),
                changed,
            }
        }

        Steer::Next(track) => {
            // The one instruction that may move the protected slot, because it
            // is explicitly about it.
            let at = played.min(out.len());
            let existing = out.iter().position(|s| s.track == *track);
            let changed = usize::from(existing != Some(at));
            match existing {
                Some(found) if found >= at => {
                    let slot = out.remove(found);
                    out.insert(at, Slot { ..slot });
                }
                _ => {
                    let template = out.get(at).cloned();
                    if let Some(template) = template {
                        out.insert(
                            at,
                            Slot {
                                track: *track,
                                ..template
                            },
                        );
                    }
                }
            }
            Steered {
                plan: out,
                summary: "playing that next".to_owned(),
                changed,
            }
        }

        Steer::Later(track) => {
            let first = played.min(out.len());
            let Some(found) = out.iter().skip(first).position(|s| s.track == *track) else {
                return Steered {
                    plan: out,
                    summary: "that is not in the set".to_owned(),
                    changed: 0,
                };
            };
            let index = first + found;
            let slot = out.remove(index);
            // Three slots later, or the end. Far enough to be out of the way,
            // near enough that "not yet" does not become "never".
            let to = (index + 3).min(out.len());
            out.insert(to, slot);
            Steered {
                plan: out,
                summary: "moved that later".to_owned(),
                changed: 1,
            }
        }

        Steer::Drop(track) => {
            let first = played.min(out.len());
            let before = out.len();
            let mut index = first;
            while index < out.len() {
                if out[index].track == *track {
                    out.remove(index);
                } else {
                    index += 1;
                }
            }
            Steered {
                changed: before - out.len(),
                plan: out,
                summary: "taken out of the set".to_owned(),
            }
        }
    }
}

/// Re-pick the tail from `first`, following on from `anchor`.
///
/// Each choice becomes the next question, exactly as the assembler does it, so
/// a steered set flows the same way an assembled one does.
fn rechoose(
    plan: &[Slot],
    first: usize,
    anchor: Option<TrackId>,
    pool: &[LibraryTrack],
    taste: &Taste,
) -> Vec<TrackId> {
    let mut used: Vec<TrackId> = plan.iter().take(first).map(|s| s.track).collect();
    let mut current = anchor.and_then(|id| pool.iter().find(|t| t.id == id));
    let mut out = Vec::new();

    for slot in plan.iter().skip(first) {
        let now = current.map_or(
            Playing {
                key: None,
                bpm: None,
                lufs: None,
                phrase_beats: None,
            },
            |t| Playing {
                key: t.analysis.key(),
                bpm: t.analysis.bpm,
                lufs: t.analysis.loudness_lufs,
                phrase_beats: t.analysis.phrase_beats,
            },
        );

        let candidates: Vec<LibraryTrack> = pool
            .iter()
            .filter(|t| !used.contains(&t.id))
            .filter(|t| taste.weight_for(t) > 0.0)
            .filter(|t| grammar_allows(current, t))
            .cloned()
            .collect();
        if candidates.is_empty() {
            // Nothing left that fits: keep what was planned rather than
            // truncating the set. A shorter set is a worse answer than a
            // slightly wrong one, because the room does not stop.
            out.push(slot.track);
            used.push(slot.track);
            continue;
        }

        let mut ranked = suggest::rank(&now, slot.trajectory, &candidates);
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

        let picked = ranked.first().map_or(slot.track, |s| s.track);
        out.push(picked);
        used.push(picked);
        current = pool.iter().find(|t| t.id == picked);
    }
    out
}

/// Whether the rhythmic grammars permit this pairing. Same rule the assembler
/// uses; see `dj_core::genre`.
fn grammar_allows(from: Option<&LibraryTrack>, to: &LibraryTrack) -> bool {
    let Some(from) = from else { return true };
    let (Some(a), Some(b)) = (
        from.tags.genre.as_deref().and_then(genre::family_for),
        to.tags.genre.as_deref().and_then(genre::family_for),
    ) else {
        return true;
    };
    a.blends_with(b) != genre::Blendability::Cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setlist::{Arc, assemble};
    use crate::{PlayStats, StoredAnalysis, Tags};
    use dj_core::{Mode, SampleRate};

    fn track(byte: u8, lufs: f64, genre: &str) -> LibraryTrack {
        LibraryTrack {
            id: TrackId::from_bytes([byte; 32]),
            path: std::path::PathBuf::from(format!("/music/{byte}.wav")),
            tags: Tags {
                artist: Some(format!("artist {byte}")),
                genre: Some(genre.to_owned()),
                ..Tags::default()
            },
            duration_frames: 44_100 * 210,
            sample_rate: SampleRate::DEFAULT,
            channels: 2,
            file_size: None,
            file_modified: None,
            added_at: 0,
            analysis: StoredAnalysis {
                bpm: Some(124.0),
                key_hour: Some(8),
                key_mode: Some(Mode::Minor),
                loudness_lufs: Some(lufs),
                phrase_beats: Some(16),
                phrase_anchor: Some(0),
                ..StoredAnalysis::default()
            },
            stats: PlayStats::default(),
            colour: None,
        }
    }

    fn pool() -> Vec<LibraryTrack> {
        (0..24u8)
            .map(|n| {
                let genre = if n % 2 == 0 { "house" } else { "disco" };
                track(n + 1, -16.0 + f64::from(n) * 0.4, genre)
            })
            .collect()
    }

    fn a_plan() -> Vec<Slot> {
        assemble(&pool(), Arc::Flat, &Taste::default(), 40.0, None)
    }

    /// **What has already played never changes.**
    ///
    /// The first rule. A set's history is not a plan and cannot be steered.
    #[test]
    fn the_past_is_never_touched() {
        let plan = a_plan();
        let played = 3;
        let before: Vec<TrackId> = plan.iter().take(played).map(|s| s.track).collect();

        for instruction in [
            Steer::Lift,
            Steer::Ease,
            Steer::Avoid("house".to_owned()),
            Steer::Drop(plan[0].track),
        ] {
            let after = steer(&plan, played, &instruction, &pool());
            let kept: Vec<TrackId> = after.plan.iter().take(played).map(|s| s.track).collect();
            assert_eq!(kept, before, "{instruction:?} rewrote history");
        }
    }

    /// **A mood instruction does not swap the record coming in.**
    ///
    /// The next one may be cued, staged, or have a hand on its fader already. A
    /// DJ saying "more energy" means *after this one*, and an assistant that
    /// swapped what was mixing would be actively dangerous.
    ///
    /// Asserted as "lift and ease agree about this slot", not as "it equals
    /// what was planned". The weaker form passed with the protection removed
    /// entirely, because re-choosing happened to pick the same record; two
    /// opposite instructions agreeing can only happen if neither touched it.
    #[test]
    fn a_mood_change_leaves_the_next_record_alone() {
        let plan = a_plan();
        let played = 2;
        let planned_next = plan[played].track;

        let lifted = steer(&plan, played, &Steer::Lift, &pool());
        let eased = steer(&plan, played, &Steer::Ease, &pool());

        assert_eq!(
            lifted.plan[played].track, planned_next,
            "lifting swapped the record about to play"
        );
        assert_eq!(
            eased.plan[played].track, planned_next,
            "easing swapped the record about to play"
        );
    }

    /// **But "play this next" does move it**, because it is explicitly about
    /// that slot.
    #[test]
    fn play_this_next_moves_the_protected_slot() {
        let plan = a_plan();
        let played = 2;
        let wanted = plan[plan.len() - 1].track;

        let after = steer(&plan, played, &Steer::Next(wanted), &pool());
        assert_eq!(after.plan[played].track, wanted);
    }

    /// **A trajectory change actually changes the records, not only the
    /// labels.**
    ///
    /// A trajectory nothing acts on is decoration. Lift and ease are compared
    /// against *each other* rather than against the original: an earlier
    /// version asserted the tail differed from before steering, and it did not
    /// -- the assembled set had already climbed to the loudest records it had,
    /// so lifting from there had nowhere left to go and produced the same
    /// sequence. Comparing the two directions is the claim that matters and
    /// cannot pass by accident.
    #[test]
    fn lifting_and_easing_produce_different_tails() {
        let plan = a_plan();
        let lifted = steer(&plan, 1, &Steer::Lift, &pool());
        let eased = steer(&plan, 1, &Steer::Ease, &pool());

        let tail = |s: &Steered| -> Vec<TrackId> {
            s.plan.iter().skip(3).map(|slot| slot.track).collect()
        };
        assert_ne!(
            tail(&lifted),
            tail(&eased),
            "lifting and easing chose the same records, so the trajectory does \
             nothing"
        );
        assert!(lifted.changed > 0 || eased.changed > 0);
    }

    /// **An avoided genre leaves the set, and the rest of it stays.**
    ///
    /// Avoiding is surgical: the offending records are replaced, and the ones
    /// that were fine are not disturbed. A DJ saying "no more disco" has not
    /// asked for a different night.
    #[test]
    fn avoiding_a_genre_replaces_only_the_offenders() {
        let pool = pool();
        let plan = a_plan();
        let played = 1;

        let after = steer(&plan, played, &Steer::Avoid("disco".to_owned()), &pool);

        let genre_of = |id: TrackId| {
            pool.iter()
                .find(|t| t.id == id)
                .and_then(|t| t.tags.genre.clone())
        };

        for slot in after.plan.iter().skip(played + PROTECTED) {
            assert_ne!(
                genre_of(slot.track).as_deref(),
                Some("disco"),
                "an avoided genre survived the steer"
            );
        }

        // And the records that were already fine are still there, in place.
        // Without this the test passed with every slot replaced, which is
        // rebuilding the set rather than steering it -- the exact thing this
        // module exists not to do.
        for (index, before) in plan.iter().enumerate().skip(played + PROTECTED) {
            if genre_of(before.track).as_deref() == Some("disco") {
                continue;
            }
            assert_eq!(
                after.plan.get(index).map(|s| s.track),
                Some(before.track),
                "a record that was fine was replaced anyway, at slot {index}"
            );
        }
    }

    /// **"Not yet" is not "never".**
    ///
    /// A record moved later stays in the set. Dropping it would be answering a
    /// different question.
    #[test]
    fn later_moves_a_record_without_losing_it() {
        let plan = a_plan();
        let played = 1;
        let victim = plan[played + 1].track;

        let after = steer(&plan, played, &Steer::Later(victim), &pool());
        let position = after.plan.iter().position(|s| s.track == victim);
        assert!(
            position.is_some(),
            "\"not yet\" dropped the record entirely"
        );
        assert!(
            position.unwrap() > played + 1,
            "the record did not actually move later"
        );
    }

    /// Dropping does remove it, and removes every instance.
    #[test]
    fn dropping_removes_a_record_from_the_rest_of_the_set() {
        let plan = a_plan();
        let victim = plan[3].track;
        let after = steer(&plan, 1, &Steer::Drop(victim), &pool());

        assert!(
            !after.plan.iter().skip(1).any(|s| s.track == victim),
            "the dropped record is still in the set"
        );
        assert_eq!(after.changed, 1);
    }

    /// **A steer that changes nothing says so.**
    ///
    /// "Nothing needed to change" is a different answer from "done", and an
    /// interface that showed the same thing for both would be lying about one
    /// of them.
    #[test]
    fn a_steer_with_nothing_to_do_reports_zero() {
        let plan = a_plan();
        let after = steer(&plan, 1, &Steer::Avoid("hardstyle".to_owned()), &pool());
        assert_eq!(
            after.changed, 0,
            "avoiding a genre that was never in the set changed something"
        );
    }

    /// **Steering never crosses a rhythmic grammar either.**
    ///
    /// The assembler's rule has to hold for the steered tail too, or a DJ could
    /// steer their way into a set they cannot actually play.
    ///
    /// The fixture is built so that ignoring the grammar would *visibly* break
    /// it: every track has the same tempo, key and loudness, so the suggester
    /// scores them all alike and ties break on track id -- and the tech house
    /// records hold the **low** ids. A tail chosen without the grammar check
    /// therefore fills with tech house immediately. Two earlier versions of
    /// this test passed with the check deleted, because the ids happened to
    /// favour the right answer.
    #[test]
    fn a_steered_tail_still_respects_the_grammar() {
        let mut pool: Vec<LibraryTrack> = (0..12u8)
            .map(|n| track(n + 1, -8.0, "tech house"))
            .collect();
        pool.extend((12..24u8).map(|n| track(n + 1, -8.0, "reggaeton")));

        // Start on a reggaetón record, which the low-id tech house tracks
        // cannot be blended into.
        let opener = TrackId::from_bytes([13; 32]);
        let plan = assemble(&pool, Arc::Flat, &Taste::default(), 40.0, Some(opener));

        let genre_of = |id: TrackId| {
            pool.iter()
                .find(|t| t.id == id)
                .and_then(|t| t.tags.genre.clone())
        };
        assert_eq!(
            genre_of(plan[0].track).as_deref(),
            Some("reggaeton"),
            "the fixture did not start where it claims to"
        );

        let after = steer(&plan, 1, &Steer::Lift, &pool);
        for slot in &after.plan {
            assert_eq!(
                genre_of(slot.track).as_deref(),
                Some("reggaeton"),
                "steering crossed into a different rhythmic grammar"
            );
        }
    }

    /// Steering a set that has entirely played does nothing and does not panic.
    #[test]
    fn steering_a_finished_set_is_harmless() {
        let plan = a_plan();
        let after = steer(&plan, plan.len(), &Steer::Lift, &pool());
        assert_eq!(after.changed, 0);
        assert_eq!(after.plan.len(), plan.len());
        // And past the end, which a caller counting wrongly could ask for.
        let over = steer(&plan, plan.len() + 5, &Steer::Ease, &pool());
        assert_eq!(over.plan.len(), plan.len());
    }

    /// Every steer explains itself, for the same reason every suggestion does.
    #[test]
    fn every_steer_says_what_it_did() {
        let plan = a_plan();
        for instruction in [
            Steer::Lift,
            Steer::Ease,
            Steer::Hold,
            Steer::Avoid("disco".to_owned()),
            Steer::Favour("house".to_owned()),
            Steer::Next(plan[4].track),
            Steer::Later(plan[3].track),
            Steer::Drop(plan[3].track),
        ] {
            let after = steer(&plan, 1, &instruction, &pool());
            assert!(
                !after.summary.trim().is_empty(),
                "{instruction:?} gave no summary"
            );
        }
    }
}

//! §12's *cue conventions*: which pad a DJ puts which kind of cue on.
//!
//! A DJ who always keeps the record's start on pad 1, the first drop on pad 2
//! and the outro on pad 8 has a convention, and an assistant that knows it can
//! say "jump to your drop" without asking which pad that is. The evidence is
//! the cues they set, which the library keeps per record (`cues`): the slot,
//! the position, and the DJ's own label and colour.
//!
//! # Only the DJ's hand
//!
//! djmanzo never places a hot cue itself: the `cues` table is written from the
//! deck's cues as the DJ set them (`persist`), or imported from software the
//! DJ set them in. So every row is the DJ's own, and none needs filtering out
//! the way the autopilot's records do in [`crate::learned`].
//!
//! # Decayed like everything else
//!
//! A record's cues weigh by how recently it was played, on
//! [`crate::learned::recency`]'s half-life, so a convention the DJ gave up
//! fades rather than outvoting the present forever. A record never played
//! weighs by when it was added: the cues were set some time after that.
//!
//! # A convention is earned
//!
//! §13's rule, applied to pads: a slot says nothing until it holds a cue on
//! [`ENOUGH`] records, and a kind of cue is that slot's convention only when it
//! is [`MOST`] of the slot's weighted evidence. Fewer is not a habit, and a
//! slot used every which way has none.

use serde::Serialize;

/// Records a slot must hold a cue on before it says anything.
pub const ENOUGH: usize = 4;

/// The share of a slot's weighted evidence one kind must hold to be its
/// convention.
pub const MOST: f64 = 0.6;

/// A record's beat grid, as far as a cue's place needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// The frame of the first beat.
    pub anchor: f64,
    pub frames_per_beat: f64,
    /// The phrase length in beats and the beat one starts on, when known.
    pub phrase: Option<(u32, i64)>,
}

/// One stored cue, with what its record says about where it sits.
#[derive(Debug, Clone, PartialEq)]
pub struct CueSeen {
    /// 1-based, as pads are numbered.
    pub slot: u8,
    pub frame: f64,
    pub label: Option<String>,
    pub colour: Option<String>,
    /// `None` for a record with no beat grid: its cues have no place.
    pub grid: Option<Grid>,
    pub duration_frames: f64,
    /// [`crate::learned::recency`] of the record's last play, or its adding.
    pub weight: f64,
}

/// Where in a record a cue sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// On or before the first beat.
    Start,
    /// On the first beat of a phrase.
    PhraseStart,
    /// In the last fifth of the record.
    Ending,
    /// Anywhere else.
    Elsewhere,
}

impl Place {
    /// What the briefing calls it.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Start => "the record's start",
            Self::PhraseStart => "a phrase start",
            Self::Ending => "the last fifth",
            Self::Elsewhere => "off the phrase",
        }
    }
}

/// Where `cue` sits, or `None` when its record has no beat grid.
///
/// A quarter of a beat either side is *on* a beat: a cue a DJ set by ear on
/// the drop lands a few milliseconds off it, and that is still the drop.
#[must_use]
pub fn place(cue: &CueSeen) -> Option<Place> {
    let grid = cue.grid?;
    if grid.frames_per_beat <= 0.0 {
        return None;
    }
    let beat = (cue.frame - grid.anchor) / grid.frames_per_beat;
    if beat < 0.5 {
        return Some(Place::Start);
    }
    if cue.duration_frames > 0.0 && cue.frame >= cue.duration_frames * 0.8 {
        return Some(Place::Ending);
    }
    let nearest = beat.round();
    let on_beat = (beat - nearest).abs() < 0.25;
    let on_phrase = grid.phrase.is_some_and(|(beats, start)| {
        beats > 0 && on_beat && ((nearest as i64) - start).rem_euclid(i64::from(beats)) == 0
    });
    Some(if on_phrase {
        Place::PhraseStart
    } else {
        Place::Elsewhere
    })
}

/// A slot's convention, where it has one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlotHabit {
    pub slot: u8,
    /// Records with a cue in this slot.
    pub records: usize,
    /// Where the cue sits, when one place holds [`MOST`] of the evidence.
    pub place: Option<Place>,
    /// The DJ's own label for it, case folded, when one holds [`MOST`].
    pub label: Option<String>,
    /// Its colour, when one holds [`MOST`].
    pub colour: Option<String>,
}

/// The kind holding at least [`MOST`] of `weighted`'s total, if one does.
fn most<K: Ord + Clone>(weighted: &std::collections::BTreeMap<K, f64>, total: f64) -> Option<K> {
    if total <= 0.0 {
        return None;
    }
    weighted
        .iter()
        .filter(|(_, weight)| **weight / total >= MOST)
        .map(|(kind, _)| kind.clone())
        .next()
}

/// Every slot's convention, slot by slot, from the cues a DJ has set.
///
/// A slot with fewer than [`ENOUGH`] records is left out; a slot with enough
/// and no kind holding [`MOST`] is kept with every field `None` — "pad 5 is
/// used, for anything" is worth telling.
#[must_use]
pub fn conventions(seen: &[CueSeen]) -> Vec<SlotHabit> {
    use std::collections::BTreeMap;
    let mut slots: BTreeMap<u8, Vec<&CueSeen>> = BTreeMap::new();
    for cue in seen {
        slots.entry(cue.slot).or_default().push(cue);
    }
    slots
        .into_iter()
        .filter(|(_, cues)| cues.len() >= ENOUGH)
        .map(|(slot, cues)| {
            let total: f64 = cues.iter().map(|cue| cue.weight).sum();
            let mut places: BTreeMap<u8, f64> = BTreeMap::new();
            let mut labels: BTreeMap<String, f64> = BTreeMap::new();
            let mut colours: BTreeMap<String, f64> = BTreeMap::new();
            let mut placed = 0.0;
            for cue in &cues {
                if let Some(at) = place(cue) {
                    *places.entry(at as u8).or_default() += cue.weight;
                    placed += cue.weight;
                }
                let label = cue.label.as_deref().map(str::trim).unwrap_or_default();
                if !label.is_empty() {
                    *labels.entry(label.to_lowercase()).or_default() += cue.weight;
                }
                if let Some(colour) = cue.colour.as_deref().filter(|c| !c.trim().is_empty()) {
                    *colours.entry(colour.trim().to_lowercase()).or_default() += cue.weight;
                }
            }
            const PLACES: [Place; 4] = [
                Place::Start,
                Place::PhraseStart,
                Place::Ending,
                Place::Elsewhere,
            ];
            SlotHabit {
                slot,
                records: cues.len(),
                // Of the cues that have a place: a record with no grid is no
                // evidence about where this DJ puts the cue.
                place: most(&places, placed).map(|at| PLACES[usize::from(at)]),
                label: most(&labels, total),
                colour: most(&colours, total),
            }
        })
        .collect()
}

/// The conventions as the assistant's briefing says them, slot by slot:
/// `pad 1: the record's start, labelled intro (12 records)`. Empty when no
/// slot has earned a line.
#[must_use]
pub fn words(habits: &[SlotHabit]) -> String {
    habits
        .iter()
        .map(|habit| {
            let mut kinds = Vec::new();
            if let Some(place) = habit.place {
                kinds.push(place.words().to_owned());
            }
            if let Some(label) = &habit.label {
                kinds.push(format!("labelled {label}"));
            }
            if let Some(colour) = &habit.colour {
                kinds.push(format!("coloured {colour}"));
            }
            let kind = if kinds.is_empty() {
                "used for anything".to_owned()
            } else {
                kinds.join(", ")
            };
            format!("pad {}: {kind} ({} records)", habit.slot, habit.records)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record at 120 BPM and 48 kHz (24 000 frames a beat), first beat at
    /// frame 1000, sixteen-beat phrases from beat 0, five minutes long.
    fn grid() -> Grid {
        Grid {
            anchor: 1_000.0,
            frames_per_beat: 24_000.0,
            phrase: Some((16, 0)),
        }
    }

    fn cue(slot: u8, beat: f64, label: Option<&str>, weight: f64) -> CueSeen {
        CueSeen {
            slot,
            frame: 1_000.0 + beat * 24_000.0,
            label: label.map(str::to_owned),
            colour: None,
            grid: Some(grid()),
            duration_frames: 48_000.0 * 300.0,
            weight,
        }
    }

    /// **Where a cue sits**: on the first beat or before it is the start; on
    /// a phrase's first beat, within a quarter beat, a phrase start; the last
    /// fifth the ending; anything else off the phrase; no grid, no place.
    #[test]
    fn a_cue_has_a_place_only_on_a_grid() {
        assert_eq!(place(&cue(1, 0.0, None, 1.0)), Some(Place::Start));
        assert_eq!(place(&cue(1, -2.0, None, 1.0)), Some(Place::Start));
        assert_eq!(place(&cue(1, 32.1, None, 1.0)), Some(Place::PhraseStart));
        assert_eq!(place(&cue(1, 33.0, None, 1.0)), Some(Place::Elsewhere));
        assert_eq!(place(&cue(1, 31.6, None, 1.0)), Some(Place::Elsewhere));
        assert_eq!(place(&cue(1, 500.0, None, 1.0)), Some(Place::Ending));
        let mut loose = cue(1, 32.0, None, 1.0);
        loose.grid = None;
        assert_eq!(place(&loose), None);
    }

    /// **A slot's convention is earned, and recent records weigh more.**
    /// Three records say nothing; four on the start make pad 1's; a slot used
    /// every which way is kept with no convention; a label the DJ gave up a
    /// long time ago is outvoted by the one they use now.
    #[test]
    fn a_slot_has_a_convention_only_when_most_of_its_records_agree() {
        let mut seen = vec![
            cue(1, 0.0, Some("Intro"), 1.0),
            cue(1, 0.0, Some("intro "), 1.0),
            cue(1, 0.0, None, 1.0),
        ];
        assert!(conventions(&seen).is_empty(), "three records made a habit");
        seen.push(cue(1, 0.2, None, 1.0));
        let pad1 = &conventions(&seen)[0];
        assert_eq!(
            (pad1.slot, pad1.records, pad1.place, pad1.label.as_deref()),
            (1, 4, Some(Place::Start), None),
            "two of four labelled is not most of them: {pad1:?}"
        );
        // Case and spaces folded: three of five is the label.
        seen.push(cue(1, 0.0, Some("INTRO"), 1.0));
        assert_eq!(conventions(&seen)[0].label.as_deref(), Some("intro"));

        // Pad 2 every which way: kept, with nothing claimed.
        for beat in [0.0, 32.0, 33.0, 500.0] {
            seen.push(cue(2, beat, None, 1.0));
        }
        let pad2 = conventions(&seen)
            .into_iter()
            .find(|habit| habit.slot == 2)
            .expect("used");
        assert_eq!((pad2.place, pad2.label), (None, None));

        // Pad 3: "drop" on four old records, "build" on two this month.
        let mut changed: Vec<CueSeen> = (0..4)
            .map(|_| {
                cue(
                    3,
                    64.0,
                    Some("drop"),
                    crate::learned::recency(0, 400 * 86_400),
                )
            })
            .collect();
        changed.push(cue(3, 64.0, Some("build"), 1.0));
        changed.push(cue(3, 64.0, Some("build"), 1.0));
        let pad3 = &conventions(&changed)[0];
        assert_eq!(pad3.label.as_deref(), Some("build"), "{pad3:?}");
        assert_eq!(pad3.place, Some(Place::PhraseStart));
    }

    #[test]
    fn the_conventions_are_said_slot_by_slot() {
        let habits = [
            SlotHabit {
                slot: 1,
                records: 12,
                place: Some(Place::Start),
                label: Some("intro".into()),
                colour: None,
            },
            SlotHabit {
                slot: 5,
                records: 4,
                place: None,
                label: None,
                colour: None,
            },
        ];
        assert_eq!(
            words(&habits),
            "pad 1: the record's start, labelled intro (12 records); pad 5: used for anything (4 records)"
        );
        assert_eq!(words(&[]), "");
    }
}

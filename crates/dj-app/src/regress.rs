//! §90's performance regression, and the one measurement that can be a ratchet.
//!
//! > Measure: UI frame rate, audio xruns, memory, CPU, worker utilization. **Do
//! > not let visually sophisticated changes compromise realtime audio.**
//!
//! # The last line is the deliverable; the five are how you would notice
//!
//! djmanzo measures two of §90's five and shows both: the Mission Bar carries
//! the master bus's CPU and its dropout count, and `report_bench` logs the
//! interface's own frame timing. None of the five is *ratcheted* — nothing
//! fails when a change makes one worse — which is the difference between
//! measuring and regressing.
//!
//! # Why three of them cannot be ratcheted here, and saying so is the point
//!
//! A ratchet is a number a test refuses to let grow, so it is only worth having
//! where the number means the same thing on two machines. Three of §90's five
//! do not: frame rate, memory and CPU are properties of the machine at least as
//! much as of the code, CI installs its own Chromium and this container has no
//! GPU at all. `docs/HANDOFF.md` records the same argument for §89's screenshot
//! baselines, and the conclusion is the same — a suite that has to be
//! re-blessed every run has stopped being a test, and one re-blessed
//! automatically never was one.
//!
//! # What *is* machine-independent, and is §90's own sentence
//!
//! Allocation counts. `dj-engine/tests/rt_safety.rs` already installs a
//! counting allocator and proves the audio callback allocates **zero** times
//! over thousands of blocks, which is the strongest form §90's last line can
//! take on the engine side.
//!
//! What that test does not cover is the other half of the sentence — the
//! *visually sophisticated changes*. The 60 Hz snapshot is where they arrive:
//! every field the interface wants is built sixty times a second on the thread
//! that also serves the engine's controls, and a snapshot that quietly grew
//! from a handful of allocations to hundreds is exactly "visual sophistication
//! taxing the engine", arriving one harmless-looking field at a time. That
//! number is the same on every machine, so it can be a ratchet, and
//! [`SNAPSHOT_ALLOCATIONS`] is it.

/// One of §90's five measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure {
    /// §90's own words for it.
    pub what: &'static str,
    /// Where djmanzo takes the reading, or empty if it takes none.
    pub measured: &'static str,
    /// Whether a test refuses to let it get worse.
    pub ratcheted: bool,
    /// Why it is not ratcheted. Empty for the one that is.
    pub why_not: &'static str,
}

/// §90's five, in §90's order.
pub const ALL: [Measure; 5] = [
    Measure {
        what: "UI frame rate",
        measured: "The webview times its own compositing and reports it to `report_bench`.",
        ratcheted: false,
        why_not: "A frame rate is a property of the machine at least as much as \
                  of the code. CI installs its own browser build and this \
                  container has no GPU, so a baseline captured in either place \
                  fails in the other for reasons no change caused. What runs \
                  instead is §89's geometry rules, which fail for the right one.",
    },
    Measure {
        what: "audio xruns",
        measured: "The master bus counts dropouts; the Mission Bar shows them.",
        ratcheted: false,
        why_not: "There is no audio device in CI or in this container, so the \
                  callback never runs against a real clock and the count is \
                  always nought — a ratchet on it would pass for the wrong \
                  reason. What guards the same thing structurally is \
                  `dj-engine/tests/rt_safety.rs`, which proves the callback \
                  allocates zero times over thousands of blocks.",
    },
    Measure {
        what: "memory",
        measured: "",
        ratcheted: false,
        why_not: "Not measured. A resident-set figure on a container sharing a \
                  page cache with a build is noise, and the allocation count \
                  below is the part of it that means the same thing twice.",
    },
    Measure {
        what: "CPU",
        measured: "The master bus reports its own load; the Mission Bar shows it.",
        ratcheted: false,
        why_not: "Machine-dependent for the same reason as the frame rate, and \
                  measured against a null device here, which is not the load a \
                  real one imposes.",
    },
    Measure {
        what: "worker utilization",
        measured: "",
        ratcheted: false,
        why_not: "Not measured. The decoder and analyser threads report whether \
                  they are working, not how hard, and instrumenting them is its \
                  own piece of work rather than a line here.",
    },
];

/// How many allocations building one 60 Hz snapshot may cost.
///
/// **A ratchet, not a target.** The number is whatever the code does today; its
/// job is to fail when a change makes it worse, so that adding a field to the
/// snapshot is a decision somebody makes rather than something that happens.
/// Lower it whenever the real figure drops — a ratchet that has drifted above
/// the truth has stopped holding anything.
///
/// Why this number and not a duration: it is the same on every machine, which
/// is the whole reason it can be a test at all. See the module docs.
///
/// Why the snapshot and not the callback: the callback is already proven to
/// allocate zero times by `dj-engine/tests/rt_safety.rs`. This is the other
/// half of §90's sentence — where *visually sophisticated changes* actually
/// arrive, sixty times a second, on the thread that also serves the controls.
///
/// Fifty-six, against a real figure of fifty-two over six decks. The headroom
/// is four, and it is that tight for a measured reason: a plausible per-deck
/// field — one `format!` per deck and a `Vec` to hold them — costs thirteen,
/// and an earlier draft with thirteen of slack let exactly that mutation
/// through. A ratchet that cannot catch the most ordinary regression there is
/// holds nothing.
///
/// The slack has a test of its own, which fails once it passes an eighth of
/// the ratchet. The first draft of this constant was six hundred; that test is
/// what caught it.
///
/// **Four is tight enough to be platform-sensitive, and that is accepted.** CI
/// runs Linux, macOS and Windows, and `std`'s own allocation behaviour need not
/// match across them. If it disagrees the ratchet will say so, which is
/// information worth having — raise it deliberately and record what the
/// difference was, rather than widening it in advance for a difference nobody
/// has seen.
pub const SNAPSHOT_ALLOCATIONS: usize = 56;

/// How many decks the ratchet is measured over.
///
/// The most djmanzo supports, because that is the worst case a DJ can put it
/// in and a budget measured on two decks would not notice a field that is
/// per-deck.
pub const RATCHET_DECKS: usize = dj_core::MAX_DECKS;

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every one of §90's five is accounted for, and the ones that are not
    /// ratcheted say why.**
    ///
    /// The §8 posture, and §90 needs it: four of the five cannot be ratcheted
    /// here and a list that simply left them out would read as §90 being done.
    /// Two of the four are not measured at all, which is a different and worse
    /// thing than being measured and not ratcheted — both say which.
    #[test]
    fn every_measurement_is_accounted_for() {
        assert_eq!(ALL.len(), 5, "§90 names five measurements");
        for measure in ALL {
            if measure.ratcheted {
                assert!(
                    measure.why_not.is_empty(),
                    "`{}` is ratcheted and still carries a reason it is not",
                    measure.what
                );
            } else {
                assert!(
                    !measure.why_not.trim().is_empty(),
                    "`{}` is not ratcheted and does not say why, which reads as \
                     an oversight rather than a decision",
                    measure.what
                );
            }
            for mark in ['*', '#'] {
                assert!(
                    !measure.why_not.contains(mark),
                    "`{}` is written in markup",
                    measure.what
                );
            }
        }
    }

    /// **§90's own words are all here.**
    ///
    /// Written out so that dropping one is a decision somebody makes rather
    /// than something that happens when a row is edited.
    #[test]
    fn the_directive_names_five_and_all_five_are_on_the_list() {
        let named: Vec<&str> = ALL.iter().map(|m| m.what).collect();
        assert_eq!(
            named,
            [
                "UI frame rate",
                "audio xruns",
                "memory",
                "CPU",
                "worker utilization"
            ]
        );
    }

    /// **A measurement djmanzo does not take does not claim a source.**
    ///
    /// The pair that is easiest to get wrong in a table like this: a row saying
    /// where it is measured *and* that it is not measured would be two claims
    /// in opposite directions, and the one a reader believes is whichever they
    /// read first.
    #[test]
    fn a_row_that_measures_nothing_names_no_source() {
        for measure in ALL {
            let takes_a_reading = !measure.measured.is_empty();
            let says_not_measured = measure.why_not.starts_with("Not measured");
            assert_ne!(
                takes_a_reading, says_not_measured,
                "`{}` is inconsistent about whether djmanzo measures it",
                measure.what
            );
        }
    }
}

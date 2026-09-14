//! §90's *worker utilization*, measured the one way that means anything.
//!
//! > Measure: UI frame rate, audio xruns, memory, CPU, **worker utilization**.
//! > Do not let visually sophisticated changes compromise realtime audio.
//!
//! [`crate::regress`] is the table of those five and what djmanzo does about
//! each. Two of them read "not measured", and this closes one: the background
//! threads reported whether they were working, which is a light, and not how
//! much of their lives they spend working, which is the measurement.
//!
//! # Share of its own time, not CPU
//!
//! A worker's utilisation here is `busy / (busy + waiting)` as the worker
//! itself times it. Not a share of a core: a percentage of CPU depends on how
//! many cores there are, what else is running and how the scheduler feels, and
//! `dj_app::regress` gives at length the reason djmanzo does not ratchet
//! numbers like that. This one is a fact about the **thread's own loop** — how
//! much of the time it went round, it had something in front of it — and it
//! means the same thing on a laptop and in a container with no GPU.
//!
//! # The two it is worth measuring, and why they are opposite
//!
//! The **interface builder** is the 60 Hz snapshot thread, and §90's last
//! sentence is about it: every field the interface wants is assembled there,
//! sixty times a second, on the thread that also serves the engine's controls.
//! A share creeping up is *visual sophistication taxing the machine*, arriving
//! one harmless-looking field at a time.
//!
//! The **library worker** decodes and analyses imported files. High there is
//! **healthy**: it means a queue is being got through. The two numbers
//! therefore mean opposite things, and that is exactly why this reports them
//! separately rather than as one "busiest worker" figure — a single number
//! would alarm at an import and say nothing at all about the thing §90 is
//! actually worried about.
//!
//! Neither is ratcheted, and [`crate::regress`] says why in the same words.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// One background worker's account of its own time.
///
/// Cheap enough to touch on every pass of a loop that runs sixty times a
/// second: two relaxed atomic adds, no lock and no allocation. Relaxed is
/// right — these are counters read for a readout, and no other memory ordering
/// depends on them.
#[derive(Debug, Default)]
pub struct Worker {
    busy_nanos: AtomicU64,
    idle_nanos: AtomicU64,
}

impl Worker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            busy_nanos: AtomicU64::new(0),
            idle_nanos: AtomicU64::new(0),
        }
    }

    /// Record a stretch the worker spent with something in front of it.
    pub fn worked(&self, took: Duration) {
        add(&self.busy_nanos, took);
    }

    /// Record a stretch it spent waiting for something.
    pub fn waited(&self, took: Duration) {
        add(&self.idle_nanos, took);
    }

    /// The share of its own life this worker has spent working, 0..=1.
    ///
    /// `None` until it has accounted for any time at all. A worker that has
    /// not started is not an idle worker, and reporting `0.0` for it would say
    /// it was — which on the interface builder is the difference between
    /// "nothing to do" and "never ran", and only one of those is fine.
    #[must_use]
    pub fn share(&self) -> Option<f32> {
        let busy = self.busy_nanos.load(Ordering::Relaxed);
        let idle = self.idle_nanos.load(Ordering::Relaxed);
        let lived = busy.checked_add(idle)?;
        if lived == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        Some((busy as f64 / lived as f64) as f32)
    }
}

/// Saturating, because a counter that wrapped would report a worker that had
/// just started as one that had been busy for five hundred years. Nothing here
/// will reach it — `u64` nanoseconds is five centuries — and a counter whose
/// overflow behaviour is "whatever `+` does" is one nobody can reason about.
fn add(counter: &AtomicU64, took: Duration) {
    let nanos = u64::try_from(took.as_nanos()).unwrap_or(u64::MAX);
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |held| {
            Some(held.saturating_add(nanos))
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A worker that spent half its life working reports half.**
    #[test]
    fn the_share_is_working_time_over_lived_time() {
        let worker = Worker::new();
        worker.worked(Duration::from_millis(30));
        worker.waited(Duration::from_millis(30));
        let share = worker.share().expect("a worker that has lived reports");
        assert!((share - 0.5).abs() < 1e-6, "half a life read as {share}");

        worker.waited(Duration::from_millis(60));
        let share = worker.share().expect("still lived");
        assert!((share - 0.25).abs() < 1e-6, "a quarter read as {share}");
    }

    /// **A worker that has not run yet is absent, not idle.**
    ///
    /// The distinction is the whole reason this is an `Option`: on the
    /// interface builder, "nothing to do" and "never ran" look identical as
    /// zero and only one of them is fine.
    #[test]
    fn a_worker_that_has_not_run_reports_nothing_rather_than_nought() {
        assert_eq!(Worker::new().share(), None);

        // And one that has only ever waited is genuinely idle, which is a
        // reading rather than an absence.
        let idle = Worker::new();
        idle.waited(Duration::from_millis(5));
        assert_eq!(idle.share(), Some(0.0));
    }

    /// **Saturating rather than wrapping.**
    ///
    /// Unreachable in practice — `u64` nanoseconds is five centuries — and the
    /// point is that the unreachable case is defined: a counter that wrapped
    /// would report a worker that had just started as one busy since the
    /// Renaissance, which is the sort of readout that gets a real measurement
    /// disbelieved.
    #[test]
    fn the_counters_saturate_rather_than_wrapping_round_to_nothing() {
        let worker = Worker::new();
        worker.worked(Duration::from_nanos(u64::MAX));
        worker.worked(Duration::from_secs(1));
        assert_eq!(worker.share(), Some(1.0));
    }
}

//! §90's one ratchet: what building the 60 Hz snapshot costs.
//!
//! > Measure: UI frame rate, audio xruns, memory, CPU, worker utilization. **Do
//! > not let visually sophisticated changes compromise realtime audio.**
//!
//! `dj_app::regress` says why four of §90's five cannot be ratcheted here: a
//! frame rate, a memory figure and a CPU load are properties of the machine at
//! least as much as of the code, and an xrun count measured against a null
//! device is always nought. Allocation counts are the exception — the same
//! number on every machine — which is what makes this a test rather than a
//! measurement somebody reads.
//!
//! `dj-engine/tests/rt_safety.rs` already proves the audio callback allocates
//! **zero** times over thousands of blocks. That is the engine half of §90's
//! last line. This is the other half: the snapshot is where *visually
//! sophisticated changes* arrive, sixty times a second, built on the thread
//! that also serves the engine's controls. A snapshot that grew from a handful
//! of allocations to hundreds would be exactly what §90 forbids, arriving one
//! harmless-looking field at a time with nothing failing.
//!
//! # It is a ratchet, not a target
//!
//! [`dj_app::regress::SNAPSHOT_ALLOCATIONS`] is whatever the code does today.
//! Its job is to fail when a change makes it worse, so that adding a field to
//! the snapshot is a decision rather than something that happens. **Lower it
//! whenever the real figure drops**; a ratchet that has drifted above the truth
//! has stopped holding anything, which is why this test prints the real number
//! on failure in both directions.

// A `GlobalAlloc` implementation is unsafe by definition. `dj-engine`'s
// `rt_safety.rs` makes the same exception for the same reason.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use dj_app::regress::{RATCHET_DECKS, SNAPSHOT_ALLOCATIONS};
use dj_app::snapshot::Snapshot;
use dj_control::ParameterRegistry;

thread_local! {
    /// Whether this thread is currently under scrutiny.
    ///
    /// `const` initialisation matters: a lazily-initialised thread-local would
    /// itself allocate on first access, from inside the allocator, which
    /// recurses.
    static WATCHING: Cell<bool> = const { Cell::new(false) };

    /// How many allocations this thread has seen while watching.
    ///
    /// **Thread-local, not a process-wide atomic.** Cargo runs the tests in
    /// this file on separate threads at the same time, and a shared counter
    /// meant each measurement included whatever the others were allocating —
    /// the figures came out as 52 alone and 86 in parallel, and the
    /// determinism test is what caught it. A counter that is only correct when
    /// nothing else is running is not a ratchet.
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }
}

fn note_allocation() {
    // `try_with` because the thread-local may already be destroyed during
    // thread teardown, and panicking inside the allocator is unrecoverable.
    if WATCHING.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Run `body` with allocation counting on, returning how many happened.
///
/// Counts only what this thread allocates, so two of these running at once do
/// not measure each other.
fn count_allocations<T>(body: impl FnOnce() -> T) -> (T, usize) {
    COUNT.with(|c| c.set(0));
    WATCHING.with(|w| w.set(true));
    let result = body();
    WATCHING.with(|w| w.set(false));
    (result, COUNT.with(Cell::get))
}

/// How many captures to take before the figure is the steady state.
///
/// Something behind the registry initialises lazily on first use, and it does
/// not all happen in one capture: measured here, the first counted capture
/// costs 94 and every later one 52. That is start-up, not the steady state the
/// interface lives in sixty times a second, and counting it would ratchet the
/// wrong number.
///
/// **This was found by the determinism test below, failing.** The first draft
/// warmed once and passed when the file was run alone — the tests share a
/// process, so whichever ran first paid the initialisation — and failed in the
/// full workspace run when a different one went first. A flaky ratchet is worse
/// than none, so the shape here is: warm, then measure the tail, and assert the
/// tail is flat.
const WARM_UP: usize = 3;

/// Captures taken after the warm-up, and the run the tail is checked over.
const RUNS: usize = 8;

/// The steady cost of one capture, and every figure behind it.
fn steady_cost(decks: usize) -> (usize, Vec<usize>) {
    let registry = ParameterRegistry::new();
    for _ in 0..WARM_UP {
        let _ = Snapshot::capture(&registry, decks);
    }
    let counts: Vec<usize> = (0..RUNS)
        .map(|_| count_allocations(|| Snapshot::capture(&registry, decks)).1)
        .collect();
    // The maximum of the steady tail rather than the minimum: a ratchet has to
    // hold against the worst the steady state does, and the tail being flat is
    // asserted separately.
    let worst = *counts.iter().max().expect("RUNS is not zero");
    (worst, counts)
}

/// **The load-bearing one: building a snapshot costs no more than it did.**/// **The load-bearing one: building a snapshot costs no more than it did.**
///
/// Measured over the most decks djmanzo supports, because that is the worst
/// case a DJ can put it in and a budget taken on two would not notice a field
/// that is per-deck.
///
/// The warm-up captures are excluded on purpose: something behind the registry
/// initialises lazily, and counting it would measure start-up rather than the
/// steady state the interface lives in sixty times a second. See [`WARM_UP`]
/// for how that was found.
#[test]
fn one_snapshot_costs_no_more_than_the_ratchet() {
    let registry = ParameterRegistry::new();
    let snapshot = Snapshot::capture(&registry, RATCHET_DECKS);
    assert_eq!(
        snapshot.decks.len(),
        RATCHET_DECKS,
        "the ratchet measured a snapshot that is not the shape it claims"
    );

    let (allocations, _) = steady_cost(RATCHET_DECKS);
    assert!(
        allocations <= SNAPSHOT_ALLOCATIONS,
        "building one snapshot over {RATCHET_DECKS} decks now costs {allocations} \
         allocations, and the ratchet is {SNAPSHOT_ALLOCATIONS}.\n\n\
         This runs sixty times a second on the thread that also serves the \
         engine's controls, which is what §90 means by not letting visually \
         sophisticated changes compromise realtime audio. Either take the cost \
         back out, or raise `dj_app::regress::SNAPSHOT_ALLOCATIONS` \
         deliberately and say in the commit what the field is worth."
    );
}

/// **And the ratchet has not drifted above the truth.**
///
/// A ratchet set far above what the code does holds nothing: a change could
/// double the real figure and still pass. An eighth, because a *quarter* was
/// measured to be too loose — a plausible per-deck field costs about thirteen
/// allocations over six decks, and a quarter of this ratchet is thirteen, so
/// the most ordinary regression there is slid underneath it. The message says
/// to lower the constant, because that is the action; widening anything here is
/// the failure this test exists to prevent.
#[test]
fn the_ratchet_is_still_close_to_what_the_code_does() {
    let (allocations, _) = steady_cost(RATCHET_DECKS);

    let slack = SNAPSHOT_ALLOCATIONS.saturating_sub(allocations);
    assert!(
        slack * 8 <= SNAPSHOT_ALLOCATIONS,
        "the snapshot costs {allocations} allocations and the ratchet is \
         {SNAPSHOT_ALLOCATIONS} — {slack} of slack, which is enough for a real \
         regression to fit under. Lower `dj_app::regress::SNAPSHOT_ALLOCATIONS` \
         to about {allocations}."
    );
}

/// **The count is the same every time, which is what lets it be a ratchet.**
///
/// A figure that wobbled between runs would fail at random and be turned off
/// within a week. Ten captures rather than two: a lazily-warmed cache shows up
/// on the second, and anything that grows without bound shows up by the tenth.
///
/// **Within one machine.** Nothing here can prove the figure is the same on
/// macOS or Windows, where `std`'s own allocation behaviour may differ — CI
/// runs all three, and if it disagrees the ratchet is what will say so. That is
/// information worth having rather than a reason to loosen it: raise the
/// constant deliberately and record what the difference was.
#[test]
fn the_same_snapshot_costs_the_same_every_time() {
    let (_, counts) = steady_cost(RATCHET_DECKS);
    let first = counts[0];
    assert!(
        counts.iter().all(|c| *c == first),
        "after {WARM_UP} warm-up captures the snapshot's cost is still not \
         steady on this machine: {counts:?}. A ratchet on a wobbling number \
         fails at random and gets turned off — either the warm-up is too short \
         or something is growing per call, and the two look different: a short \
         warm-up shows one high figure at the front, something growing shows a \
         rising tail."
    );
}

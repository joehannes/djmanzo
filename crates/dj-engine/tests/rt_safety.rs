//! Proof that the audio callback does not allocate.
//!
//! `docs/ROADMAP.md` promises this from M0 onward. The rule is easy to state and
//! easy to break by accident -- a `Vec::push` past capacity, a `format!` in a log
//! line, a `Box<dyn Error>`, an `Arc` dropped at the wrong moment. Every one of
//! those calls into the allocator, which can take a lock, which can block the
//! realtime thread, which is a dropout in front of an audience.
//!
//! So we measure it instead of trusting it. This test installs a global
//! allocator that counts allocations on threads that opt in, renders thousands
//! of blocks through the engine, and asserts the count is exactly zero.

// A `GlobalAlloc` implementation is unsafe by definition. This is the one place
// in the project where that is expected; the workspace lint stays on everywhere
// else.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dj_audio::{OfflineRenderer, StreamConfig};
use dj_control::ParameterRegistry;
use dj_core::param::{DeckParam, GlobalParam};
use dj_core::{Action, DeckAction, DeckId, MixerAction, ParamId, SampleRate};
use dj_decode::{AudioBuffer, TrackSource};
use dj_engine::{Command, Engine, Retired};

// ---------------------------------------------------------------------------
// Allocation counting
// ---------------------------------------------------------------------------

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Whether this thread is currently under scrutiny.
    ///
    /// `const` initialisation matters: a lazily-initialised thread-local would
    /// itself allocate on first access, from inside the allocator, which
    /// recurses. This form compiles to a plain thread-local slot.
    static WATCHING: Cell<bool> = const { Cell::new(false) };
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
    let watching = WATCHING.try_with(Cell::get).unwrap_or(false);
    if watching {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Run `body` with allocation counting on, returning how many happened.
fn count_allocations<T>(body: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    WATCHING.with(|w| w.set(true));
    let result = body();
    WATCHING.with(|w| w.set(false));
    (result, ALLOCATIONS.load(Ordering::Relaxed))
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SR: SampleRate = SampleRate::DEFAULT;

fn deck(n: u8) -> DeckId {
    DeckId::from_human(n).unwrap()
}

fn tone(frames: usize) -> Arc<dyn TrackSource> {
    // A slow ramp rather than a constant, so interpolation actually does work.
    let samples: Vec<f32> = (0..frames)
        .flat_map(|n| {
            let v = ((n % 100) as f32 / 100.0) - 0.5;
            [v, -v]
        })
        .collect();
    Arc::new(AudioBuffer::from_interleaved(samples, SR))
}

struct Rig {
    renderer: OfflineRenderer,
    commands: rtrb::Producer<Command>,
    retired: rtrb::Consumer<Retired>,
    registry: Arc<ParameterRegistry>,
}

fn rig(decks: usize, buffer_frames: u32) -> Rig {
    rig_with_channels(decks, buffer_frames, 2)
}

fn rig_with_channels(decks: usize, buffer_frames: u32, channels: u16) -> Rig {
    let (command_tx, command_rx) = rtrb::RingBuffer::new(1024);
    let (retired_tx, retired_rx) = rtrb::RingBuffer::new(64);
    let registry = Arc::new(ParameterRegistry::new());
    let engine = Engine::new(decks, SR, command_rx, retired_tx, Arc::clone(&registry));

    let config = StreamConfig {
        buffer_frames,
        sample_rate: SR,
        channels,
        device: None,
    };

    Rig {
        renderer: OfflineRenderer::new(Box::new(engine), &config),
        commands: command_tx,
        retired: retired_rx,
        registry,
    }
}

impl Rig {
    fn send(&mut self, command: Command) {
        self.commands.push(command).expect("command queue full");
    }

    fn act(&mut self, action: Action) {
        self.send(Command::Action(action));
    }

    fn load_and_play(&mut self, n: u8, frames: usize) {
        self.send(Command::Load {
            deck: deck(n),
            source: tone(frames),
        });
        self.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::Play,
        });
    }

    /// Render blocks and drain the retirement queue, as the host thread does.
    fn warm_up(&mut self, blocks: usize) {
        for _ in 0..blocks {
            self.renderer.render_block();
            while self.retired.pop().is_ok() {}
        }
    }
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

/// The headline guarantee: steady-state playback allocates nothing.
#[test]
fn steady_state_playback_never_allocates() {
    let mut rig = rig(4, 256);
    for n in 1..=4u8 {
        rig.load_and_play(n, 2_000_000);
    }
    // Let the loads land and every lazily-initialised thing settle.
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(10_000);
    });

    assert_eq!(
        allocations, 0,
        "the audio callback allocated {allocations} times across 10,000 blocks"
    );
}

/// Actions arriving mid-set are the common case -- a controller sweep sends
/// hundreds a second. None of that may allocate either.
#[test]
fn processing_actions_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 1_000_000);
    rig.load_and_play(2, 1_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            let position = ((step % 200) as f32 / 100.0) - 1.0;
            rig.commands
                .push(Command::Action(Action::Mixer(MixerAction::Crossfader(
                    position,
                ))))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::SetVolume((step % 100) as f32 / 100.0),
                }))
                .ok();
            rig.renderer.render_block();
        }
    });

    assert_eq!(
        allocations, 0,
        "handling actions allocated {allocations} times"
    );
}

/// Loading a track displaces an `Arc`. If the engine dropped it inline, the
/// deallocation would happen here -- and `dealloc` is exactly as blocking as
/// `alloc`. This test pins the retirement mechanism in place.
#[test]
fn loading_a_track_never_allocates_on_the_audio_thread() {
    let mut rig = rig(2, 256);
    rig.warm_up(8);

    // Prepare the sources up front; building them is host-thread work.
    let sources: Vec<Arc<dyn TrackSource>> = (0..16).map(|_| tone(50_000)).collect();

    let (_, allocations) = count_allocations(|| {
        for source in &sources {
            rig.commands
                .push(Command::Load {
                    deck: deck(1),
                    source: Arc::clone(source),
                })
                .ok();
            rig.renderer.render_block();
        }
    });

    assert_eq!(
        allocations, 0,
        "loading allocated {allocations} times inside the callback"
    );

    // And the displaced sources really did come back out.
    let mut retired = 0;
    while rig.retired.pop().is_ok() {
        retired += 1;
    }
    assert!(retired > 0, "no sources were handed back for retirement");
}

/// A deck running off the end of its track takes a different branch. Make sure
/// that branch is allocation-free too.
#[test]
fn running_past_the_end_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 512); // Very short: exhausted within a few blocks.
    rig.warm_up(4);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(1_000);
    });

    assert_eq!(
        allocations, 0,
        "end-of-track path allocated {allocations} times"
    );
}

/// The cue path takes a different branch through the render loop -- an extra
/// bus, a blend, and a split-cue case. It must be allocation-free too, and a
/// stereo-only rig never exercises it.
#[test]
fn the_headphone_cue_path_never_allocates() {
    let mut rig = rig_with_channels(4, 256, 4);
    for n in 1..=4u8 {
        rig.load_and_play(n, 2_000_000);
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetCue(n % 2 == 0),
        });
    }
    rig.act(Action::Mixer(MixerAction::CueMix(0.4)));
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });
    assert_eq!(
        allocations, 0,
        "the cue path allocated {allocations} times across 5,000 blocks"
    );
}

/// Split cue is a separate branch again.
#[test]
fn split_cue_never_allocates() {
    let mut rig = rig_with_channels(2, 256, 4);
    rig.load_and_play(1, 1_000_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetCue(true),
    });
    rig.act(Action::Mixer(MixerAction::SplitCue(true)));
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });
    assert_eq!(allocations, 0, "split cue allocated {allocations} times");
}

/// Six channels adds the booth bus on top of master and cue.
#[test]
fn the_booth_bus_never_allocates() {
    let mut rig = rig_with_channels(2, 256, 6);
    rig.load_and_play(1, 1_000_000);
    rig.act(Action::Mixer(MixerAction::BoothGainDb(-6.0)));
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });
    assert_eq!(allocations, 0, "booth bus allocated {allocations} times");
}

/// Buffer size is a user setting, and an unusual one must not change the
/// guarantee.
#[test]
fn allocation_freedom_holds_across_buffer_sizes() {
    for frames in [64u32, 128, 256, 512, 1024] {
        let mut rig = rig(2, frames);
        rig.load_and_play(1, 1_000_000);
        rig.warm_up(16);

        let (_, allocations) = count_allocations(|| {
            rig.renderer.render_discarding(500);
        });

        assert_eq!(
            allocations, 0,
            "buffer size {frames} allocated {allocations} times"
        );
    }
}

// ---------------------------------------------------------------------------
// Behaviour, verified offline
// ---------------------------------------------------------------------------

/// The walking-skeleton assertion: a loaded, playing deck advances its playhead
/// by exactly the block size, and reports it.
#[test]
fn playback_advances_position_by_the_block_size() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 1_000_000);
    rig.renderer.render_block(); // Load lands.

    let before = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::Position));
    rig.renderer.render_block();
    let after = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::Position));

    assert!(
        (after - before - 256.0).abs() < 1.0,
        "expected the playhead to advance 256 frames, went {before} -> {after}"
    );
}

#[test]
fn a_paused_deck_renders_exact_silence() {
    let mut rig = rig(2, 256);
    rig.send(Command::Load {
        deck: deck(1),
        source: tone(100_000),
    });
    rig.renderer.render_block();

    let out = rig.renderer.render(8);
    assert!(
        out.iter().all(|&s| s == 0.0),
        "a loaded but paused deck must be silent"
    );
}

#[test]
fn the_engine_reports_its_cpu_load() {
    let mut rig = rig(4, 256);
    for n in 1..=4u8 {
        rig.load_and_play(n, 500_000);
    }
    rig.warm_up(64);

    let load = rig.registry.get(ParamId::Global(GlobalParam::CpuLoad));
    assert!(load > 0.0, "CPU load should be measured, got {load}");
    assert!(
        load < 1.0,
        "four decks should not exhaust the block budget, got {load}"
    );
}

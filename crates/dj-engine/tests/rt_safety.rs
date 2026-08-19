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

use dj_audio::{AudioCallback, OfflineRenderer, RenderContext, StreamConfig, cue_bridge};
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

/// The master limiter is in the path of every single frame that leaves the
/// application, on both the master and the headphone bus.
///
/// Its interesting case is the periodic rescan of the look-ahead window, which
/// is the one branch in `process_frame` that does more than constant work. It
/// only fires when the peak leaves the window, so it has to be *driven* to fire
/// — a steady tone never triggers it. Material that keeps rising and falling
/// past the ceiling does, hundreds of times a second.
#[test]
fn the_master_limiter_never_allocates() {
    let mut rig = rig_with_channels(4, 256, 4);
    for n in 1..=4u8 {
        rig.load_and_play(n, 2_000_000);
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetCue(n % 2 == 0),
        });
        // Well over full scale once summed, so the limiter is working rather
        // than sitting at unity for the whole test.
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetGainDb(12.0),
        });
    }
    rig.act(Action::Mixer(MixerAction::CueMix(0.4)));
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });
    assert_eq!(
        allocations, 0,
        "the limiter allocated {allocations} times across 5,000 blocks"
    );
}

/// Toggling the limiter is a control a DJ can reach for mid-set, so the switch
/// itself has to be free of allocation too — not just the steady state.
#[test]
fn toggling_the_limiter_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 1_000_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetGainDb(18.0),
    });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..200 {
            rig.act(Action::Mixer(MixerAction::SetLimiter(round % 2 == 0)));
            rig.renderer.render_discarding(4);
        }
    });
    assert_eq!(
        allocations, 0,
        "toggling the limiter allocated {allocations} times"
    );
}

// ---------------------------------------------------------------------------
// The dual-device bridge
//
// This one runs on *two* audio threads rather than one, and the interesting
// part is the control loop: it computes a resampling ratio every block, which
// is exactly the kind of arithmetic that grows a buffer if written carelessly.
// ---------------------------------------------------------------------------

/// Neither half of the bridge may allocate, and the correction has to be
/// actively working while it is measured -- a matched pair sits at a ratio of
/// 1.0 and never exercises the interpolator's shift path properly.
#[test]
fn the_dual_device_bridge_never_allocates() {
    let (mut producer, mut consumer, _stats) = cue_bridge(SampleRate::DEFAULT, 256);
    let block: Vec<f32> = (0..256 * 2).map(|n| (n % 97) as f32 / 97.0).collect();
    let mut out = vec![0.0f32; 256 * 2];

    // Prime outside the measured region: the first fill is the one place
    // start-up work could hide.
    for _ in 0..8 {
        producer.push(&block);
        consumer.pull(&mut out);
    }

    let (_, allocations) = count_allocations(|| {
        for round in 0..5_000 {
            // Deliberately mismatched: push an extra frame occasionally so the
            // queue drifts and the loop has something to correct.
            producer.push(&block);
            if round % 64 == 0 {
                producer.push(&block[..2]);
            }
            consumer.pull(&mut out);
        }
    });
    assert_eq!(
        allocations, 0,
        "the bridge allocated {allocations} times across 5,000 blocks"
    );
}

/// The split callbacks are what the device actually calls, so they are what
/// has to be allocation-free -- including the scratch buffer the primary uses
/// to render four channels into a two-channel device.
#[test]
fn the_split_callbacks_never_allocate() {
    // Built directly rather than through `rig`, which wraps the engine in an
    // `OfflineRenderer`; here the split callback is the wrapper.
    let (mut commands, command_rx) = rtrb::RingBuffer::new(1024);
    let (retired_tx, mut retired) = rtrb::RingBuffer::new(64);
    let registry = Arc::new(ParameterRegistry::new());
    let engine = Engine::new(2, SR, command_rx, retired_tx, registry);

    commands
        .push(Command::Load {
            deck: deck(1),
            source: tone(1_000_000),
        })
        .ok();
    for action in [DeckAction::Play, DeckAction::SetCue(true)] {
        commands
            .push(Command::Action(Action::Deck {
                deck: deck(1),
                action,
            }))
            .ok();
    }

    let (producer, consumer, _stats) = cue_bridge(SampleRate::DEFAULT, 256);
    let mut primary = dj_audio::SplitPrimary::new(Box::new(engine), producer);
    let mut secondary = dj_audio::SplitSecondary::new(consumer);

    let mut master = vec![0.0f32; 256 * 2];
    let mut phones = vec![0.0f32; 256 * 2];
    let ctx = RenderContext {
        frames: 256,
        channels: 2,
        sample_rate: SampleRate::DEFAULT,
    };

    // Prime first, as above, draining retirements the way the host thread does.
    for _ in 0..32 {
        primary.render(&mut master, &ctx);
        secondary.render(&mut phones, &ctx);
        while retired.pop().is_ok() {}
    }

    let (_, allocations) = count_allocations(|| {
        for _ in 0..2_000 {
            primary.render(&mut master, &ctx);
            secondary.render(&mut phones, &ctx);
        }
    });
    assert_eq!(
        allocations, 0,
        "the split callbacks allocated {allocations} times"
    );
}

/// Sync runs inside the callback and reads one deck while writing another,
/// which is exactly the shape that tempts a `Vec` of candidates or a collected
/// iterator. Pressed repeatedly, as a DJ riding sync between two decks does.
#[test]
fn sync_and_beat_jump_never_allocate() {
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

    let mut rig = rig(4, 256);
    for n in 1..=2u8 {
        rig.load_and_play(n, 2_000_000);
        rig.send(Command::SetGrid {
            deck: deck(n),
            grid: Some(Beatgrid::new(
                FramePos::new(0.0),
                Bpm::new(if n == 1 { 128.0 } else { 120.0 }).unwrap(),
                Confidence::new(0.9),
            )),
        });
    }
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            rig.act(Action::Deck {
                deck: deck(2),
                action: if round % 2 == 0 {
                    DeckAction::Sync
                } else {
                    DeckAction::SyncOff
                },
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::BeatJump(if round % 4 == 0 { 4 } else { -4 }),
            });
            rig.act(Action::Mixer(MixerAction::SetQuantize(round % 3 == 0)));
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "sync and beat jump allocated {allocations} times"
    );
}

/// Loops fold the playhead on *every frame* of both render paths, and hot cues
/// index a fixed array. Both are the kind of thing that grows a `Vec` if
/// written carelessly, and the loop fold in particular runs 48 000 times a
/// second per deck.
///
/// Driven with a loop shorter than one buffer, so the fold actually fires many
/// times per block rather than never.
#[test]
fn loops_and_hot_cues_never_allocate() {
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.send(Command::SetGrid {
        deck: deck(1),
        grid: Some(Beatgrid::new(
            FramePos::new(0.0),
            Bpm::new(128.0).unwrap(),
            Confidence::new(0.9),
        )),
    });
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::LoopBeats(1),
    });
    // Quarter of a beat: shorter than the 256-frame buffer at 128 BPM.
    for _ in 0..2 {
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::LoopHalve,
        });
    }
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::HotCue((round % 8 + 1) as u8),
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: if round % 2 == 0 {
                    DeckAction::LoopHalve
                } else {
                    DeckAction::LoopDouble
                },
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::LoopMove(if round % 4 == 0 { 1 } else { -1 }),
            });
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "loops and hot cues allocated {allocations} times"
    );
}

/// Slip runs a second playhead on the audio thread, and censor and reverse
/// change the sign of the step mid-callback. All three are arithmetic on
/// existing state, and this is what says so.
#[test]
fn slip_reverse_and_censor_never_allocate() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetSlip(true),
    });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            // A censor held and released every few blocks, which is the
            // realistic gesture and also the one that jumps the playhead.
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::SetCensor(round % 8 < 4),
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::ToggleReverse,
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::ToggleSlip,
            });
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "slip, reverse and censor allocated {allocations} times"
    );
}

/// The keylocked path runs the shadow per block rather than per frame, and a
/// reversed read cursor is its own arithmetic. Its own proof, for the same
/// reason the loop has one.
#[test]
fn a_keylocked_censor_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetKeylock(true),
    });
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetSlip(true),
    });
    rig.warm_up(64);

    let (_, allocations) = count_allocations(|| {
        for round in 0..1_000 {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::SetCensor(round % 6 < 3),
            });
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "a keylocked censor allocated {allocations} times"
    );
}

/// The keylocked path folds the loop separately, with a read cursor running
/// ahead of the playhead. It has its own arithmetic and so needs its own proof.
#[test]
fn a_keylocked_loop_never_allocates() {
    use dj_core::{Beatgrid, Bpm, Confidence, FramePos};

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.send(Command::SetGrid {
        deck: deck(1),
        grid: Some(Beatgrid::new(
            FramePos::new(0.0),
            Bpm::new(128.0).unwrap(),
            Confidence::new(0.9),
        )),
    });
    for action in [
        DeckAction::SetKeylock(true),
        DeckAction::SetPitch(0.06),
        DeckAction::LoopBeats(1),
    ] {
        rig.act(Action::Deck {
            deck: deck(1),
            action,
        });
    }
    rig.warm_up(64);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(3_000);
    });
    assert_eq!(
        allocations, 0,
        "a keylocked loop allocated {allocations} times"
    );
}

/// Keylock puts a C++ phase vocoder in the audio path, and this is the test
/// that says whether that was allowed.
///
/// The upstream library sizes its internals in `configure` and is written for
/// realtime use, but "written for realtime use" is a claim, not a proof, and it
/// is not our code. A `std::vector` that grows one element past its reserve
/// would be invisible in review and audible on stage. So it is measured.
#[test]
fn keylock_never_allocates() {
    let mut rig = rig(2, 256);
    for n in 1..=2u8 {
        rig.load_and_play(n, 2_000_000);
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetKeylock(true),
        });
        // Off centre, so the shifter is doing real work rather than passing
        // unity through.
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetPitch(0.08),
        });
    }
    rig.warm_up(64);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });
    assert_eq!(
        allocations, 0,
        "keylock allocated {allocations} times across 5,000 blocks"
    );
}

/// Engaging keylock mid-set refills the shifter's history, which is a much
/// larger burst of work than a steady block -- and still must not allocate.
#[test]
fn engaging_keylock_and_seeking_never_allocate() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..20 {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::SetKeylock(round % 2 == 0),
            });
            // A seek re-primes too; both paths are exercised here.
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::Seek(dj_core::FramePos::new(round as f64 * 10_000.0)),
            });
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::Play,
            });
            rig.renderer.render_discarding(50);
        }
    });
    assert_eq!(
        allocations, 0,
        "toggling keylock allocated {allocations} times"
    );
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

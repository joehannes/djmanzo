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

/// A track whose stems are already separated, as one looks to a deck after
/// the background worker has caught up.
///
/// The four parts are deliberately distinguishable -- each carries a different
/// constant -- so a test can tell which of them a mute actually removed.
fn tone_with_stems(frames: usize) -> Arc<dyn TrackSource> {
    let samples: Vec<f32> = (0..frames)
        .flat_map(|n| {
            let v = ((n % 100) as f32 / 100.0) - 0.5;
            [v, -v]
        })
        .collect();
    let buffer = AudioBuffer::from_interleaved(samples, SR);
    // vocal, drums, bass, other -- in dj_core::Stem::ALL order.
    let chunk: dj_decode::StemChunk = (0..frames)
        .map(|_| [0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4])
        .collect();
    buffer.stems_lock().store(Arc::new(
        dj_decode::StemTable::default()
            .with_chunk(0, chunk)
            .expect("the first chunk always fits"),
    ));
    Arc::new(buffer)
}

/// A separated track whose vocal stem is a straight ramp: frame `n` carries
/// `n / 1000`.
///
/// Constant stems cannot tell a working interpolation from one that reads the
/// same frame all block. A ramp can: the value on the wire *is* the position
/// it was read at.
fn ramp_with_stems(frames: usize) -> Arc<dyn TrackSource> {
    let samples: Vec<f32> = (0..frames)
        .flat_map(|n| {
            let v = ((n % 100) as f32 / 100.0) - 0.5;
            [v, -v]
        })
        .collect();
    let buffer = AudioBuffer::from_interleaved(samples, SR);
    let chunk: dj_decode::StemChunk = (0..frames)
        .map(|n| {
            let v = n as f32 / 1000.0;
            [v, v, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4]
        })
        .collect();
    buffer.stems_lock().store(Arc::new(
        dj_decode::StemTable::default()
            .with_chunk(0, chunk)
            .expect("the first chunk always fits"),
    ));
    Arc::new(buffer)
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

    fn load_and_play_separated(&mut self, n: u8, frames: usize) {
        self.send(Command::Load {
            deck: deck(n),
            source: tone_with_stems(frames),
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
        action: DeckAction::LoopBeats(1.0),
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

/// A brake changes the playback rate every frame, which is the most a deck's
/// inner loop is ever asked to do — and it also switches the deck off the
/// keylocked path onto the direct one mid-set. Neither may allocate.
#[test]
fn braking_and_backspinning_never_allocate() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.load_and_play(2, 2_000_000);
    // Deck 2 keylocked, so the path switch is exercised too.
    rig.act(Action::Deck {
        deck: deck(2),
        action: DeckAction::SetKeylock(true),
    });
    rig.send(Command::SetGrid {
        deck: deck(1),
        grid: Some(dj_core::Beatgrid::new(
            dj_core::FramePos::ZERO,
            dj_core::Bpm::new(128.0).unwrap(),
            dj_core::Confidence::new(0.9),
        )),
    });
    rig.send(Command::SetGrid {
        deck: deck(2),
        grid: Some(dj_core::Beatgrid::new(
            dj_core::FramePos::ZERO,
            dj_core::Bpm::new(128.0).unwrap(),
            dj_core::Confidence::new(0.9),
        )),
    });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            // Start one, let it run a while, release it, start the other kind.
            match round % 64 {
                0 => rig.act(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Brake(Some(1.0)),
                }),
                16 => rig.act(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Brake(None),
                }),
                32 => rig.act(Action::Deck {
                    deck: deck(2),
                    action: DeckAction::Backspin(Some(0.5)),
                }),
                48 => {
                    rig.act(Action::Deck {
                        deck: deck(2),
                        action: DeckAction::Backspin(None),
                    });
                    // Put them back on, since a coast that runs out pauses.
                    for n in 1..=2u8 {
                        rig.act(Action::Deck {
                            deck: deck(n),
                            action: DeckAction::Play,
                        });
                    }
                }
                _ => {}
            }
            rig.renderer.render_block();
        }
    });
    assert_eq!(allocations, 0, "braking allocated {allocations} times");
}

/// The sampler, played the way a DJ plays one.
///
/// Firing a pad has to be free: it happens on the audio thread like every other
/// action, and a sampler that allocates when a pad is hit would drop out on the
/// one gesture that is always in time with the music.
///
/// Loading is in here too, because a load hands the displaced buffer back
/// through the retirement queue rather than dropping it — dropping an `Arc` can
/// free memory, and freeing is an allocator call.
#[test]
fn the_sampler_never_allocates() {
    use dj_core::{SampleChange, SampleOutput, SamplerChange, TriggerMode};

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    // Something in every slot of every bank, so the mixing loop has work.
    for bank in 1..=dj_core::SAMPLE_BANKS as u8 {
        for slot in 1..=dj_core::SAMPLE_SLOTS as u8 {
            rig.send(Command::LoadSample {
                bank,
                slot,
                source: tone(20_000),
                bpm: Some(120.0),
            });
        }
    }
    rig.warm_up(64);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            let slot = (round % dj_core::SAMPLE_SLOTS) as u8 + 1;
            let mode = TriggerMode::ALL[round % TriggerMode::ALL.len()];
            for change in [
                SampleChange::SetMode(mode),
                SampleChange::Trigger,
                SampleChange::Volume(round as f32 % 100.0 / 100.0),
                SampleChange::SetSync(round % 2 == 0),
                SampleChange::Route(if round % 3 == 0 {
                    SampleOutput::Cue
                } else {
                    SampleOutput::Master
                }),
                SampleChange::Release,
            ] {
                rig.act(Action::Mixer(MixerAction::Sample { slot, change }));
            }
            if round % 16 == 0 {
                rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::Bank(
                    (round % dj_core::SAMPLE_BANKS) as u8 + 1,
                ))));
            }
            if round % 64 == 0 {
                rig.act(Action::Mixer(MixerAction::Sampler(SamplerChange::StopAll)));
            }
            rig.renderer.render_block();
        }
    });
    assert_eq!(allocations, 0, "the sampler allocated {allocations} times");
}

/// Loading a sample mid-set must not allocate either. The buffer is built on
/// the host thread; all the audio thread does is swap two pointers and hand the
/// old one back.
#[test]
fn loading_samples_never_allocates_on_the_audio_thread() {
    let mut rig = rig(2, 256);
    rig.warm_up(32);
    let spare: Vec<Arc<dyn TrackSource>> = (0..64).map(|_| tone(10_000)).collect();

    let (_, allocations) = count_allocations(|| {
        for (round, source) in spare.iter().enumerate() {
            rig.send(Command::LoadSample {
                bank: (round % dj_core::SAMPLE_BANKS) as u8 + 1,
                slot: (round % dj_core::SAMPLE_SLOTS) as u8 + 1,
                source: Arc::clone(source),
                bpm: None,
            });
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });
    assert_eq!(
        allocations, 0,
        "loading samples allocated {allocations} times"
    );
}

/// The effect rack, switched and swept the way a DJ actually uses it.
///
/// This is the test the whole rack design exists to pass. An effect is an enum
/// variant with a slot-owned buffer rather than a `Box<dyn Effect>` precisely so
/// that changing one is an assignment: a boxed effect built on the control
/// thread would have to cross a queue, and one built here would allocate inside
/// the callback. Switching effects mid-callback is a normal DJ move — that is
/// what an FX select knob does — so it has to be free.
#[test]
fn the_effect_rack_never_allocates() {
    use dj_core::fx::{EffectKind, FxChange};

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.load_and_play(2, 2_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..2_000 {
            // Every effect in turn, in every slot, on a deck and on the master.
            let kind = EffectKind::ALL[round % EffectKind::ALL.len()];
            let slot = (round % dj_core::fx::FX_SLOTS) as u8 + 1;

            for change in [
                FxChange::Select(kind),
                FxChange::SetEnabled(true),
                FxChange::Wet(round as f32 % 100.0 / 100.0),
                FxChange::Beats(1.0 / (1 + round % 8) as f32),
                FxChange::Amount(round as f32 % 50.0 / 50.0),
                FxChange::Place(if round % 2 == 0 {
                    dj_core::fx::Placement::PreFader
                } else {
                    dj_core::fx::Placement::PostFader
                }),
            ] {
                rig.act(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Fx { slot, change },
                });
                rig.act(Action::Mixer(MixerAction::Fx { slot, change }));
            }
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "the effect rack allocated {allocations} times"
    );
}

/// Keylock and effects together: the keylocked path runs the rack from a
/// different loop, over a scratch buffer, and that loop has to stay clean too.
#[test]
fn a_keylocked_deck_with_effects_never_allocates() {
    use dj_core::fx::{EffectKind, FxChange};

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetKeylock(true),
    });
    for change in [
        FxChange::Select(EffectKind::Echo),
        FxChange::SetEnabled(true),
        FxChange::Wet(0.5),
    ] {
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::Fx { slot: 1, change },
        });
    }
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for round in 0..1_000 {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::Fx {
                    slot: 1,
                    change: FxChange::Wet(round as f32 % 100.0 / 100.0),
                },
            });
            rig.renderer.render_block();
        }
    });
    assert_eq!(
        allocations, 0,
        "a keylocked deck with effects allocated {allocations} times"
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
        DeckAction::LoopBeats(1.0),
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

/// The spectrum runs an FFT inside the callback, which is exactly the kind of
/// thing that allocates if nobody checks.
///
/// It is covered incidentally by every test above -- the transform fires on a
/// 512-frame hop and those tests render far more than that. This one names the
/// property, so a change that makes `rustfft` allocate points here rather than
/// at whichever unrelated test happens to notice first. Ten thousand blocks at
/// 256 frames is roughly 5,000 transforms.
#[test]
fn the_spectrum_never_allocates() {
    let mut rig = rig(1, 256);
    rig.load_and_play(1, 2_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(10_000);
    });

    assert_eq!(
        allocations, 0,
        "the spectral analysis allocated {allocations} times"
    );
}

/// Recording writes into a buffer on the audio thread and hands it back full.
///
/// The handback is the part worth checking: a capture leaves through the
/// retirement queue as a `Vec`, and a `Vec` moved by value is three words —
/// but a `Vec` cloned, or a capture assembled by collecting, would be an
/// allocation in the callback. Eight full takes, alternating between the deck
/// tap and the master tap because they write from different points in the
/// block.
///
/// The buffers are made up front. Each capture takes its buffer with it, so a
/// version of this test that allocated a replacement inside the loop would be
/// counting the *host's* allocation and calling it the engine's — and a
/// version that sent no replacement at all would find takes two to eight
/// silently doing nothing, which is why the capture count is asserted.
#[test]
fn recording_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.load_and_play(2, 2_000_000);

    const TAKES: usize = 8;
    let mut spare: Vec<Vec<f32>> = (0..TAKES).map(|_| vec![0.0; 48_000 * 2]).collect();
    rig.commands
        .push(Command::RecordSpace {
            samples: spare.pop().unwrap(),
        })
        .ok();
    rig.warm_up(32);

    let mut captured = 0usize;
    let (_, allocations) = count_allocations(|| {
        for take in 0..TAKES {
            let source = if take % 2 == 0 {
                dj_core::RecordSource::Master
            } else {
                dj_core::RecordSource::Deck(deck(1))
            };
            rig.commands
                .push(Command::Action(Action::Mixer(MixerAction::Sampler(
                    dj_core::SamplerChange::Record { slot: 1, source },
                ))))
                .ok();
            rig.renderer.render_discarding(20);
            rig.commands
                .push(Command::Action(Action::Mixer(MixerAction::Sampler(
                    dj_core::SamplerChange::RecordStop,
                ))))
                .ok();
            rig.renderer.render_discarding(2);

            // The host thread's half. Forgotten rather than dropped only so
            // that a `free` cannot be mistaken for the `malloc` being counted;
            // the buffers are recycled from `spare` instead.
            while let Ok(item) = rig.retired.pop() {
                if matches!(item, Retired::Capture(_)) {
                    captured += 1;
                }
                std::mem::forget(item);
            }
            if let Some(samples) = spare.pop() {
                rig.commands.push(Command::RecordSpace { samples }).ok();
                rig.renderer.render_discarding(1);
            }
        }
    });

    assert_eq!(
        captured, TAKES,
        "only {captured} of {TAKES} takes produced a capture, so this proved less than it looks"
    );
    assert_eq!(
        allocations, 0,
        "recording allocated {allocations} times across {TAKES} takes"
    );
}

/// Slicing is a loop entered and left at pad speed.
///
/// A DJ working the slicer sends a press and a release several times a bar, and
/// each one moves the playhead and re-anchors the shadow. None of that may
/// allocate.
#[test]
fn slicing_never_allocates() {
    let mut rig = rig(1, 256);
    rig.load_and_play(1, 2_000_000);
    rig.commands
        .push(Command::SetGrid {
            deck: deck(1),
            grid: Some(dj_core::Beatgrid::new(
                dj_core::FramePos::ZERO,
                dj_core::Bpm::new(128.0).unwrap(),
                dj_core::Confidence::new(0.9),
            )),
        })
        .ok();
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            let slice = (step % 8) as u8 + 1;
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Slice(Some(slice)),
                }))
                .ok();
            rig.renderer.render_discarding(2);
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Slice(None),
                }))
                .ok();
            rig.renderer.render_discarding(2);
            // The span, too: a controller with an encoder on it sends these.
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::SliceDomain(if step % 2 == 0 { 8.0 } else { 16.0 }),
                }))
                .ok();
        }
    });

    assert_eq!(
        allocations, 0,
        "the slicer allocated {allocations} times across 2,000 presses"
    );
}

/// Recording the set streams the master out of the callback for hours.
///
/// The push is per sample and the ring is shared with a writer thread, so this
/// is the one tap that runs on *every* frame of a whole night. It also has to
/// stay allocation-free when the ring is full, which is the interesting half:
/// a `push` that fails hands the sample back, and dropping it must cost
/// nothing.
#[test]
fn recording_the_set_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.load_and_play(2, 2_000_000);

    // Deliberately small, so it fills within the first few blocks and the rest
    // of the run measures the overflow path rather than the happy one.
    let (sink, _samples) = rtrb::RingBuffer::<f32>::new(4_096);
    rig.commands
        .push(Command::RecordStream { sink: Some(sink) })
        .ok();
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });

    assert_eq!(
        allocations, 0,
        "streaming the master allocated {allocations} times"
    );

    // And stopping hands the ring back rather than dropping it here.
    rig.commands.push(Command::RecordStream { sink: None }).ok();
    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(4);
    });
    assert_eq!(allocations, 0, "stopping allocated {allocations} times");

    let mut handed_back = false;
    while let Ok(item) = rig.retired.pop() {
        if matches!(item, Retired::Stream(_)) {
            handed_back = true;
        }
        std::mem::forget(item);
    }
    assert!(
        handed_back,
        "the ring was dropped on the audio thread rather than handed back"
    );
}

/// The microphone runs on the audio thread like everything else: two ring pops
/// and arithmetic per frame, and no allocation anywhere in it.
///
/// Both paths are exercised deliberately. The happy one — a ring with audio in
/// it — is the easy case; the starved one, where the ring has run dry and the
/// engine has to decide what to do about it, is where a naive implementation
/// reaches for something.
#[test]
fn the_microphone_never_allocates() {
    use dj_core::action::MicChange;

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);

    let (mut voice, consumer) = rtrb::RingBuffer::<f32>::new(48_000 * 2);
    rig.commands
        .push(Command::MicInput {
            source: Some(consumer),
        })
        .ok();
    rig.commands
        .push(Command::Action(Action::Mixer(MixerAction::Mic(
            MicChange::SetOpen(true),
        ))))
        .ok();
    rig.commands
        .push(Command::Action(Action::Mixer(MixerAction::Mic(
            MicChange::SetCue(true),
        ))))
        .ok();
    // Loud enough to hold the ducker open through the whole run, so the
    // gain-reduction path is measured rather than the resting one.
    for _ in 0..24_000 {
        voice.push(0.6).ok();
        voice.push(0.6).ok();
    }
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(8_000);
    });
    assert_eq!(
        allocations, 0,
        "an open microphone allocated {allocations} times"
    );

    // The ring is dry by now; starving must not allocate either.
    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(8_000);
    });
    assert_eq!(
        allocations, 0,
        "a starved microphone allocated {allocations} times"
    );

    // And detaching hands the consumer back rather than dropping it here.
    rig.commands.push(Command::MicInput { source: None }).ok();
    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(4);
    });
    assert_eq!(allocations, 0, "detaching allocated {allocations} times");

    let mut handed_back = false;
    while let Ok(item) = rig.retired.pop() {
        if matches!(item, Retired::MicInput(_)) {
            handed_back = true;
        }
        std::mem::forget(item);
    }
    assert!(
        handed_back,
        "the input ring was dropped on the audio thread rather than handed back"
    );
}

/// Stems are the newest thing on the audio thread and the most suspicious:
/// the deck reads them through a lock, from a `Vec` a background worker is
/// still growing. Neither the read nor the per-stem EQ and filter may
/// allocate.
#[test]
fn a_deck_playing_stems_never_allocates() {
    let mut rig = rig(2, 256);
    rig.load_and_play_separated(1, 500_000);
    rig.load_and_play_separated(2, 500_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });

    assert_eq!(
        allocations, 0,
        "playing separated stems allocated {allocations} times"
    );
}

/// Muting and soloing is what a DJ actually does with stems, and it happens
/// mid-phrase. The solo path in particular saves and restores the previous
/// mute state, which is the shape that invites a `Vec`.
#[test]
fn muting_and_soloing_stems_never_allocates() {
    use dj_core::{Stem, StemChange};

    let mut rig = rig(2, 256);
    rig.load_and_play_separated(1, 500_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            let stem = Stem::ALL[step % Stem::COUNT];
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::ToggleMute,
                    },
                }))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::SetSolo(step % 2 == 0),
                    },
                }))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::Volume((step % 100) as f32 / 100.0),
                    },
                }))
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(
        allocations, 0,
        "stem mutes and solos allocated {allocations} times"
    );
}

/// The platter is on the audio thread: `take_jog` runs once a block and the
/// step it produces is read every frame. A wheel is also the control a DJ
/// touches most, so a dropout here would be the one they notice.
#[test]
fn scratching_and_bending_never_allocate() {
    use dj_core::JogMode;

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 1_000_000);
    rig.load_and_play(2, 1_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            // A hand on deck 1's platter, scratching.
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::JogTouch(step % 64 < 32),
                }))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Jog(((step % 20) as f32 - 10.0) * 0.002),
                }))
                .ok();
            // And a bend on deck 2, in the other mode.
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(2),
                    action: DeckAction::SetJogMode(if step % 128 < 64 {
                        JogMode::Cdj
                    } else {
                        JogMode::Vinyl
                    }),
                }))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(2),
                    action: DeckAction::Jog(0.001),
                }))
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(
        allocations, 0,
        "the platter allocated {allocations} times across 2,000 blocks"
    );
}

/// A paused deck being searched renders rather than taking the cheap early
/// return, so that path has to be allocation-free too -- it is the one a DJ
/// uses while hunting for a cue point with the crowd waiting.
#[test]
fn searching_a_paused_deck_never_allocates() {
    let mut rig = rig(2, 256);
    rig.send(Command::Load {
        deck: deck(1),
        source: tone(1_000_000),
    });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Jog(if step % 2 == 0 { 0.01 } else { -0.01 }),
                }))
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(allocations, 0, "searching allocated {allocations} times");
}

// ---------------------------------------------------------------------------
// Per-stem outputs
// ---------------------------------------------------------------------------

/// The whole feature in one assertion: four parts on four pairs, in order.
///
/// `tone_with_stems` gives each part its own constant, so this checks not
/// "there is audio on channel 4" but "the *bass* is on channel 4" — which is
/// the failure a transposed pair would actually produce, and the one a
/// level-only test would sail straight past.
#[test]
fn each_stem_lands_on_its_own_output_pair() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 500_000);
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    // vocal, drums, bass, other -- the constants the harness writes.
    for (pair, expected) in [0.1_f32, 0.2, 0.3, 0.4].iter().copied().enumerate() {
        let left = block[pair * 2];
        let right = block[pair * 2 + 1];
        assert!(
            (left - expected).abs() < 1e-6,
            "pair {pair} left carried {left}, expected {expected}"
        );
        assert!(
            (right - expected).abs() < 1e-6,
            "pair {pair} right carried {right}, expected {expected}"
        );
    }
}

/// A muted stem must silence *its* pair and leave the other three alone.
///
/// The interesting half is the second assertion. A mute implemented by
/// clearing the whole tap, or by muting the deck, would pass the first.
#[test]
fn muting_a_stem_silences_only_its_own_pair() {
    use dj_core::{Stem, StemChange};

    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 500_000);
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Drums,
            change: StemChange::ToggleMute,
        },
    });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    assert_eq!(block[2], 0.0, "muted drums still on its left channel");
    assert_eq!(block[3], 0.0, "muted drums still on its right channel");
    assert!(block[0].abs() > 0.05, "muting drums silenced the vocal");
    assert!(block[4].abs() > 0.05, "muting drums silenced the bass");
    assert!(block[6].abs() > 0.05, "muting drums silenced the other");
}

/// Eight channels is the minimum, and there is no honest way to send three
/// stems of four. A four-channel device keeps its normal mix.
#[test]
fn a_narrow_device_keeps_its_mix_instead() {
    let mut rig = rig_with_channels(2, 256, 4);
    rig.load_and_play_separated(1, 500_000);
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    // The mix is the sum of the four parts (1.0 in, before the deck's own
    // gain), and in particular is not one part sitting alone on a pair.
    let main = block[0];
    assert!(
        main.abs() > 0.05,
        "the master went silent on a device too narrow for stem out ({main})"
    );
    for pair in 1..2 {
        let value = block[pair * 2];
        assert!(
            (value - 0.2).abs() > 1e-6,
            "channel {} carried a stem constant on a 4-channel device",
            pair * 2
        );
    }
}

/// Turning it off puts the mix back. Without this the feature is one-way and
/// a DJ who tried it has to restart the application.
#[test]
fn clearing_stem_out_restores_the_mix() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 500_000);
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(8);
    assert!(
        (rig.renderer.render_block()[0] - 0.1).abs() < 1e-6,
        "stem out never engaged, so this proves nothing"
    );

    rig.send(Command::SetStemOut { deck: None });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    assert!(
        (block[0] - 0.1).abs() > 1e-6,
        "the vocal stem is still alone on the main output after clearing"
    );
    assert!(
        block[0].abs() > 0.05,
        "the mix did not come back after clearing stem out"
    );
}

/// A track whose stems have not been separated yet gets silence on the pairs,
/// not the mix.
///
/// Falling back to the mix would be worse than silence: it would send the same
/// full track down all four cables and the external mixer would sum four
/// copies of it.
#[test]
fn an_unseparated_track_sends_silence_not_the_mix() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play(1, 500_000); // No stems on this one.
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    for (index, sample) in block[..8].iter().enumerate() {
        assert_eq!(
            *sample, 0.0,
            "channel {index} carried {sample} for a track with no stems"
        );
    }
}

/// The tap runs inside the callback, reading through the same lock the deck
/// does, and writes across the whole block. It must not allocate.
#[test]
fn sending_a_deck_out_in_parts_never_allocates() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 2_000_000);
    rig.load_and_play_separated(2, 2_000_000);
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });

    assert_eq!(
        allocations, 0,
        "the stem tap allocated {allocations} times across 5,000 blocks"
    );
}

/// Switching it on and off mid-set changes the bus layout under the callback,
/// which is the shape that invites a fresh buffer.
#[test]
fn toggling_stem_out_never_allocates() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 2_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            rig.commands
                .push(Command::SetStemOut {
                    deck: (step % 2 == 0).then(|| deck(1)),
                })
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(
        allocations, 0,
        "toggling stem out allocated {allocations} times"
    );
}

/// The tap reads the deck's position once per block and walks the rest, so the
/// walk has to land on the same frames the deck actually played.
///
/// With a ramp for the vocal stem, the value on channel 0 *is* the position it
/// came from: one frame of the track per frame of output, in order. A tap that
/// held one frame for the whole block, or that stepped at the wrong rate,
/// would be a pitch error on the stem cables that the mix in the room would
/// never reveal.
#[test]
fn the_stem_tap_advances_one_frame_per_frame() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.send(Command::Load {
        deck: deck(1),
        source: ramp_with_stems(500_000),
    });
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Play,
    });
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(8);

    let block = rig.renderer.render_block();
    let frames: Vec<f32> = block.chunks_exact(8).map(|frame| frame[0]).collect();
    assert_eq!(
        frames.len(),
        256,
        "expected one value per frame of the block"
    );

    // Measured from the first frame rather than between neighbours. A
    // per-neighbour step is a difference of two f32s a few thousand frames
    // into a ramp, and its noise floor is wider than the error being looked
    // for — an off-by-one in the divisor is 0.4% of a step, which a
    // neighbour check cannot see and a cumulative one cannot miss.
    let first = frames[0];
    for (index, value) in frames.iter().enumerate() {
        let expected = index as f32 * 0.001;
        let walked = value - first;
        assert!(
            (walked - expected).abs() < 2e-4,
            "frame {index} of the block came from {walked} into the ramp, expected {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// Per-deck outputs
// ---------------------------------------------------------------------------

/// Each deck on its own pair, and nothing summed.
///
/// The decks are given different gains so the pairs are told apart by level:
/// a transposition, or a deck arriving on two pairs, changes what is on a
/// channel and this notices.
#[test]
fn each_deck_lands_on_its_own_output_pair() {
    let mut rig = rig_with_channels(4, 256, 8);
    for n in 1..=4u8 {
        rig.load_and_play(n, 500_000);
        rig.act(Action::Deck {
            deck: deck(n),
            action: DeckAction::SetGainDb(-6.0 * f32::from(n - 1)),
        });
    }
    rig.send(Command::SetDeckOut { decks: Some(4) });
    rig.warm_up(16);

    let block = rig.renderer.render_block();
    let peaks: Vec<f32> = (0..4)
        .map(|pair| {
            block
                .chunks_exact(8)
                .map(|frame| frame[pair * 2].abs())
                .fold(0.0_f32, f32::max)
        })
        .collect();

    for (pair, peak) in peaks.iter().enumerate() {
        assert!(
            *peak > 0.001,
            "pair {pair} was silent; deck {} never reached its socket",
            pair + 1
        );
    }
    // Each deck is 6 dB below the one before it, so the pairs must descend.
    for window in peaks.windows(2) {
        assert!(
            window[0] > window[1] * 1.5,
            "the pairs do not descend in level: {peaks:?} — decks are being summed"
        );
    }
}

/// The whole point: nothing is mixed. A deck sent out on its own pair must not
/// *also* arrive on the master, or the room hears it twice — once through our
/// crossfader and once through the external mixer.
#[test]
fn a_deck_sent_out_separately_is_not_also_mixed() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play(2, 500_000);
    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.warm_up(16);

    let block = rig.renderer.render_block();
    let on_own_pair = block
        .chunks_exact(8)
        .map(|frame| frame[2].abs())
        .fold(0.0_f32, f32::max);
    let on_the_master = block
        .chunks_exact(8)
        .map(|frame| frame[0].abs())
        .fold(0.0_f32, f32::max);

    assert!(on_own_pair > 0.001, "deck 2 never reached pair 2");
    assert_eq!(
        on_the_master, 0.0,
        "deck 2 is on its own pair and on channel 0 as well"
    );
}

/// Pre-fader, because the mixing is happening on the other end of the cables.
///
/// A closed fader here must not silence the socket: that would be two faders
/// in series, and the second one invisible to the person standing at the
/// external mixer.
#[test]
fn the_deck_socket_is_pre_fader() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play(1, 500_000);
    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.warm_up(16);

    let open = rig
        .renderer
        .render_block()
        .chunks_exact(8)
        .map(|frame| frame[0].abs())
        .fold(0.0_f32, f32::max);

    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetVolume(0.0),
    });
    rig.warm_up(16);

    let closed = rig
        .renderer
        .render_block()
        .chunks_exact(8)
        .map(|frame| frame[0].abs())
        .fold(0.0_f32, f32::max);

    assert!(open > 0.001, "the socket was silent with the fader open");
    assert!(
        (closed - open).abs() < open * 0.05,
        "closing the fader changed the socket: {open} -> {closed}"
    );
}

/// Two decks need four channels, six decks need twelve — and a device that
/// cannot carry a pair for every deck keeps its mix instead of sending half of
/// them out and mixing the rest.
#[test]
fn a_device_too_narrow_for_every_deck_keeps_its_mix() {
    let mut rig = rig_with_channels(4, 256, 4);
    rig.load_and_play(1, 500_000);
    rig.load_and_play(3, 500_000);
    rig.send(Command::SetDeckOut { decks: Some(4) });
    rig.warm_up(16);

    let block = rig.renderer.render_block();
    let main = block
        .chunks_exact(4)
        .map(|frame| frame[0].abs())
        .fold(0.0_f32, f32::max);
    assert!(
        main > 0.001,
        "the master went silent on a device too narrow for per-deck outputs"
    );
}

/// Stem out and deck out want the same sockets, so asking for one puts the
/// other away. Without this the arrangement would depend on which `if` came
/// first in `render`.
#[test]
fn deck_out_and_stem_out_do_not_both_apply() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 500_000);
    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.send(Command::SetStemOut {
        deck: Some(deck(1)),
    });
    rig.warm_up(16);

    // Stem out was asked for last, so the vocal constant is on pair 1.
    let block = rig.renderer.render_block();
    assert!(
        (block[0] - 0.1).abs() < 1e-6,
        "the last arrangement asked for did not win: channel 0 carried {}",
        block[0]
    );

    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.warm_up(16);
    let block = rig.renderer.render_block();
    assert!(
        (block[0] - 0.1).abs() > 1e-6,
        "asking for deck out did not put stem out away"
    );
}

/// Turning it off puts the mix back, master chain and all.
#[test]
fn clearing_deck_out_restores_the_master() {
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play(2, 500_000);
    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.warm_up(16);
    assert_eq!(
        rig.renderer
            .render_block()
            .chunks_exact(8)
            .map(|frame| frame[0].abs())
            .fold(0.0_f32, f32::max),
        0.0,
        "deck out never engaged, so this proves nothing"
    );

    rig.send(Command::SetDeckOut { decks: None });
    rig.warm_up(16);

    let main = rig
        .renderer
        .render_block()
        .chunks_exact(8)
        .map(|frame| frame[0].abs())
        .fold(0.0_f32, f32::max);
    assert!(main > 0.001, "the master did not come back");
}

/// The microphone ring fills whether or not anything is listening.
///
/// In deck-out mode the master loop that drains it does not run, so it has to
/// be drained on purpose. Without that the ring backs up, and a DJ who
/// switched to per-deck outputs and back would hear a queue of everything said
/// in between — the input equivalent of a tape delay nobody asked for.
///
/// Written by filling the ring and watching it empty, because "it did not
/// panic" is not a claim about draining.
#[test]
fn the_microphone_is_still_drained_with_the_decks_out() {
    let (mut producer, consumer) = rtrb::RingBuffer::<f32>::new(8192);
    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play(1, 500_000);
    rig.send(Command::MicInput {
        source: Some(consumer),
    });
    rig.send(Command::SetDeckOut { decks: Some(2) });
    rig.warm_up(16);
    while rig.retired.pop().is_ok() {}

    // A block's worth of microphone, four times over.
    for _ in 0..(256 * 4) {
        producer.push(0.25).expect("the ring was sized for this");
    }
    let queued = producer.slots();
    rig.renderer.render_discarding(8);

    assert!(
        producer.slots() > queued,
        "the microphone ring did not empty with the decks going out separately: \
         {queued} free slots before, {} after",
        producer.slots()
    );
}

/// Sending the decks out separately runs a different path through the block —
/// no master chain, a write per deck — and it must not allocate.
#[test]
fn sending_the_decks_out_separately_never_allocates() {
    let mut rig = rig_with_channels(4, 256, 8);
    for n in 1..=4u8 {
        rig.load_and_play(n, 2_000_000);
    }
    rig.send(Command::SetDeckOut { decks: Some(4) });
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        rig.renderer.render_discarding(5_000);
    });

    assert_eq!(
        allocations, 0,
        "per-deck outputs allocated {allocations} times across 5,000 blocks"
    );
}

/// Switching between the two arrangements mid-set changes the bus layout under
/// the callback, which is the shape that invites a fresh buffer.
#[test]
fn toggling_deck_out_never_allocates() {
    let mut rig = rig_with_channels(4, 256, 8);
    for n in 1..=4u8 {
        rig.load_and_play(n, 2_000_000);
    }
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            rig.commands
                .push(Command::SetDeckOut {
                    decks: (step % 2 == 0).then_some(4),
                })
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(
        allocations, 0,
        "toggling per-deck outputs allocated {allocations} times"
    );
}

// ---------------------------------------------------------------------------
// Per-stem tone
// ---------------------------------------------------------------------------

/// A per-stem EQ that nothing can set is decoration, and that is what this was
/// before the verbs existed: the filters ran on every frame with the
/// coefficients the constructor gave them, and no action could reach them.
///
/// Measured on the **mix**, not on the stem-out tap. The tap is deliberately
/// pre-EQ — that is the whole point of it — so an EQ change can never show
/// there, and the first version of this test proved nothing for exactly that
/// reason. One stem is isolated by muting the other three instead.
#[test]
fn a_per_stem_eq_kill_changes_that_stem_and_no_other() {
    use dj_core::{EqBand, Stem, StemChange};

    /// Peak of the main bus over a block.
    fn level(rig: &mut Rig) -> f32 {
        rig.renderer
            .render_block()
            .chunks_exact(2)
            .map(|frame| frame[0].abs())
            .fold(0.0_f32, f32::max)
    }

    fn only(rig: &mut Rig, audible: Stem) {
        for stem in Stem::ALL {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::Stem {
                    stem,
                    change: StemChange::SetMute(stem != audible),
                },
            });
        }
    }

    let mut rig = rig(2, 256);
    rig.load_and_play_separated(1, 500_000);
    only(&mut rig, Stem::Drums);
    rig.warm_up(64);
    let drums_before = level(&mut rig);
    assert!(
        drums_before > 0.001,
        "the drums were not audible to begin with"
    );

    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Drums,
            change: StemChange::Eq(EqBand::Low, 0.0),
        },
    });
    rig.warm_up(64);
    let drums_after = level(&mut rig);
    assert!(
        drums_after < drums_before * 0.5,
        "killing the drums' low band did nothing: {drums_before} -> {drums_after}"
    );

    // The same kill is still in force. Switch to the vocal, which was never
    // touched: it must be unaffected.
    only(&mut rig, Stem::Vocal);
    rig.warm_up(64);
    let vocal_with_drums_killed = level(&mut rig);

    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Drums,
            change: StemChange::Eq(EqBand::Low, 1.0),
        },
    });
    rig.warm_up(64);
    let vocal_with_drums_flat = level(&mut rig);

    assert!(
        (vocal_with_drums_killed - vocal_with_drums_flat).abs()
            < vocal_with_drums_flat.max(1e-6) * 0.05,
        "the drums' EQ moved the vocal: {vocal_with_drums_killed} vs {vocal_with_drums_flat}"
    );
}

/// The deck's EQ is the channel strip and a stem's is a trim on top, so the two
/// multiply.
///
/// The claim that matters is the **default**: on a separated track the deck's
/// own `shape` is skipped and the tone comes entirely from the stem channels,
/// so if the deck's EQ stopped reaching an untouched stem it would stop working
/// altogether the moment a track finished separating. That is a silent
/// regression in a control every DJ uses on every mix.
#[test]
fn an_untouched_stem_still_follows_the_deck_eq() {
    use dj_core::{EqBand, Stem, StemChange};

    fn level(rig: &mut Rig) -> f32 {
        rig.renderer
            .render_block()
            .chunks_exact(2)
            .map(|frame| frame[0].abs())
            .fold(0.0_f32, f32::max)
    }

    let mut rig = rig(2, 256);
    rig.load_and_play_separated(1, 500_000);
    // Only the vocal, so the figure below is one stem's and not a sum.
    for stem in Stem::ALL {
        rig.act(Action::Deck {
            deck: deck(1),
            action: DeckAction::Stem {
                stem,
                change: StemChange::SetMute(stem != Stem::Vocal),
            },
        });
    }
    rig.warm_up(64);
    let before = level(&mut rig);
    assert!(before > 0.001, "the vocal was not audible to begin with");

    // The deck's own low kill. Nothing per-stem has been told about it.
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetEqLow(0.0),
    });
    rig.warm_up(64);
    let after = level(&mut rig);
    assert!(
        after < before * 0.5,
        "the deck's EQ stopped reaching an untouched stem: {before} -> {after}"
    );

    // And a stem boosting its own low cannot get past a deck kill, because the
    // two multiply and one of them is zero.
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Vocal,
            change: StemChange::Eq(EqBand::Low, 4.0),
        },
    });
    rig.warm_up(64);
    let boosted = level(&mut rig);
    assert!(
        boosted < before * 0.5,
        "a stem boost got past the deck's kill, so the two are not composed: \
         {before} -> {boosted}"
    );
}

/// The stem filter, which the EQ tests do not cover and a mutation proved they
/// did not: dropping `filter_trim` on the floor left every test green.
///
/// Swept fully high-pass, a stem whose content sits at the bottom has to go
/// away — and the three stems nobody touched have to stay exactly where they
/// were.
#[test]
fn a_per_stem_filter_sweep_changes_that_stem_and_no_other() {
    use dj_core::{Stem, StemChange};

    fn level(rig: &mut Rig) -> f32 {
        rig.renderer
            .render_block()
            .chunks_exact(2)
            .map(|frame| frame[0].abs())
            .fold(0.0_f32, f32::max)
    }

    fn only(rig: &mut Rig, audible: Stem) {
        for stem in Stem::ALL {
            rig.act(Action::Deck {
                deck: deck(1),
                action: DeckAction::Stem {
                    stem,
                    change: StemChange::SetMute(stem != audible),
                },
            });
        }
    }

    let mut rig = rig(2, 256);
    rig.load_and_play_separated(1, 500_000);
    only(&mut rig, Stem::Bass);
    rig.warm_up(64);
    let before = level(&mut rig);
    assert!(before > 0.001, "the bass was not audible to begin with");

    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Bass,
            change: StemChange::Filter(1.0),
        },
    });
    rig.warm_up(64);
    let after = level(&mut rig);
    assert!(
        after < before * 0.5,
        "sweeping the bass stem fully high-pass did nothing: {before} -> {after}"
    );

    // The sweep is still in force. A stem nobody touched is unaffected.
    only(&mut rig, Stem::Other);
    rig.warm_up(64);
    let other_with_bass_swept = level(&mut rig);

    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Bass,
            change: StemChange::Filter(0.0),
        },
    });
    rig.warm_up(64);
    let other_with_bass_open = level(&mut rig);

    assert!(
        (other_with_bass_swept - other_with_bass_open).abs()
            < other_with_bass_open.max(1e-6) * 0.05,
        "the bass stem's filter moved another stem: \
         {other_with_bass_swept} vs {other_with_bass_open}"
    );
}

/// The trim is what the DJ set, not the product of theirs and the deck's — a
/// knob that showed someone else's number would jump the moment the channel
/// strip moved.
#[test]
fn the_published_stem_eq_is_the_djs_own_setting() {
    use dj_core::{EqBand, Stem, StemChange};

    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 500_000);
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::Stem {
            stem: Stem::Bass,
            change: StemChange::Eq(EqBand::Mid, 2.0),
        },
    });
    rig.act(Action::Deck {
        deck: deck(1),
        action: DeckAction::SetEqMid(0.5),
    });
    rig.warm_up(8);

    let published = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::StemBassEqMid));
    assert!(
        (published - 2.0).abs() < 1e-6,
        "the panel would show {published} for a knob the DJ set to 2.0"
    );
}

/// Per-stem tone runs inside the callback on four channels at once, and the
/// coefficient recalculation happens on the audio thread when a knob moves.
#[test]
fn per_stem_tone_never_allocates() {
    use dj_core::{EqBand, Stem, StemChange};

    let mut rig = rig_with_channels(2, 256, 8);
    rig.load_and_play_separated(1, 2_000_000);
    rig.load_and_play_separated(2, 2_000_000);
    rig.warm_up(32);

    let (_, allocations) = count_allocations(|| {
        for step in 0..2_000 {
            let stem = Stem::ALL[step % Stem::COUNT];
            let band = EqBand::ALL[step % EqBand::ALL.len()];
            #[allow(clippy::cast_precision_loss)]
            let value = (step % 40) as f32 / 10.0;
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::Eq(band, value),
                    },
                }))
                .ok();
            rig.commands
                .push(Command::Action(Action::Deck {
                    deck: deck(1),
                    action: DeckAction::Stem {
                        stem,
                        change: StemChange::Filter(value / 4.0 - 0.5),
                    },
                }))
                .ok();
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });

    assert_eq!(
        allocations, 0,
        "per-stem tone allocated {allocations} times"
    );
}

// ---------------------------------------------------------------------------
// Timecode vinyl
// ---------------------------------------------------------------------------

/// A deck driven by a control record, with the record's signal generated here.
fn timecode_rig(absolute: bool) -> (Rig, rtrb::Producer<f32>, dj_dvs::Synth) {
    let format = dj_dvs::TimecodeFormat::bundled()[0].clone();
    let synth = dj_dvs::Synth::new(format.clone(), SR.as_f64()).expect("a synth");
    let decoder = dj_dvs::Decoder::new(format, SR.as_f64()).expect("a decoder");
    let (producer, consumer) = rtrb::RingBuffer::<f32>::new(1 << 16);

    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.send(Command::SetTimecode {
        deck: deck(1),
        input: Some(Box::new(dj_engine::command::TimecodeInput {
            decoder,
            source: consumer,
            absolute,
        })),
    });
    (rig, producer, synth)
}

/// A continuous stretch of control signal, rendered once.
///
/// **Continuous matters.** A block of 256 frames at 48 kHz carries 5.3 cycles
/// of a 1 kHz carrier, and therefore 5.3 bits — not 256. Rendering each block
/// from a bit index advanced by the *frame* count gives a signal that jumps
/// forty-eight times too fast and decodes to nonsense, which is how the first
/// version of these tests seeked the playhead to the end of the track and then
/// blamed the engine.
fn stream(synth: &dj_dvs::Synth, from: u32, speed: f64, frames: usize) -> Vec<f32> {
    synth.render(from, speed, frames)
}

/// Feed one block from a rendered stream, then let the engine answer.
fn play_block(
    rig: &mut Rig,
    producer: &mut rtrb::Producer<f32>,
    signal: &[f32],
    block: usize,
    frames: usize,
) {
    let from = block * frames * 2;
    let to = (from + frames * 2).min(signal.len());
    for sample in &signal[from..to] {
        let _ = producer.push(*sample);
    }
    rig.renderer.render_block();
    while rig.retired.pop().is_ok() {}
}

/// The deck's rate after `blocks` of a record turning at `speed`.
fn rate_after(absolute: bool, from: u32, speed: f64, blocks: usize) -> f32 {
    let (mut rig, mut producer, synth) = timecode_rig(absolute);
    let signal = stream(&synth, from, speed, blocks * 256);
    for block in 0..blocks {
        play_block(&mut rig, &mut producer, &signal, block, 256);
    }
    rig.registry.get(ParamId::Deck(deck(1), DeckParam::Rate))
}

/// **The record drives the deck.** A control record turning at normal speed
/// leaves the deck at normal speed.
#[test]
fn a_control_record_at_normal_speed_plays_the_deck_at_normal_speed() {
    let rate = rate_after(false, 10_000, 1.0, 40);
    assert!(
        (rate - 1.0).abs() < 0.15,
        "a record at normal speed drove the deck at {rate}"
    );
}

/// Half speed on the platter is half speed on the deck — the pitch fader a DJ
/// is actually holding.
#[test]
fn a_slower_record_plays_the_deck_slower() {
    let rate = rate_after(false, 10_000, 0.5, 40);
    assert!(
        (rate - 0.5).abs() < 0.15,
        "a record at half speed drove the deck at {rate}"
    );
}

/// **A deck not on vinyl says so, and it does not say zero.**
///
/// The calibration panel has three states to draw and only one number to draw
/// them from: not on a record at all, on a record and hearing nothing, and
/// reading. The middle one is a dead cartridge or the wrong input picked, and a
/// DJ staring at a deck that will not move has to be able to tell it from the
/// first. Zero for both would collapse the two.
#[test]
fn a_deck_with_no_control_record_reports_a_negative_quality() {
    let mut rig = rig(2, 256);
    rig.load_and_play(1, 2_000_000);
    rig.renderer.render_block();
    let quality = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::TimecodeQuality));
    assert!(
        quality < 0.0,
        "a deck on no record reported quality {quality}, which the panel would draw as a \
         connected input hearing nothing"
    );
}

/// **The published speed is the speed the record is asking for**, not the
/// deck's rate after everything else has had its say.
///
/// Two numbers side by side is the whole point of a calibration screen: the
/// record says one thing, the deck does another, and the gap is what a DJ is
/// diagnosing. Publishing the deck's rate under a name that promises the
/// record's would make the screen agree with itself no matter what was wrong.
#[test]
fn the_published_speed_follows_the_record() {
    let (mut rig, mut producer, synth) = timecode_rig(false);
    let signal = stream(&synth, 10_000, 0.5, 40 * 256);
    for block in 0..40 {
        play_block(&mut rig, &mut producer, &signal, block, 256);
    }
    let speed = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::TimecodeSpeed));
    assert!(
        (speed - 0.5).abs() < 0.15,
        "a record at half speed published a speed of {speed}"
    );
    let quality = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::TimecodeQuality));
    assert!(
        quality > 0.5,
        "a clean synthetic record read at quality {quality}, so the panel would tell a DJ \
         with a perfect signal that their needle is dirty"
    );
}

/// **Silence on the input is not the same as no input.**
///
/// This is the state a DJ hits when they pick the wrong capture device, and it
/// is the one the interface most needs to name. Feeding real silence has to
/// leave the quality at zero or above -- never back at the "no record"
/// sentinel, which would send them looking for a setting they already set.
#[test]
fn a_connected_input_carrying_silence_reports_zero_not_absent() {
    let (mut rig, mut producer, _synth) = timecode_rig(false);
    for _ in 0..40 {
        for _ in 0..256 * 2 {
            let _ = producer.push(0.0);
        }
        rig.renderer.render_block();
        while rig.retired.pop().is_ok() {}
    }
    let quality = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::TimecodeQuality));
    assert!(
        quality >= 0.0,
        "a connected input carrying silence reported {quality}, which the panel would draw as \
         no control record at all"
    );
    assert!(
        quality < 0.5,
        "silence read as quality {quality}, so a disconnected turntable would look like a \
         working one"
    );
}

/// **Backwards is backwards.** Half of scratching, and the thing a decoder that
/// lost the quadrature sign would get exactly wrong.
#[test]
fn a_record_turned_backwards_reverses_the_deck() {
    let rate = rate_after(false, 40_000, -1.0, 40);
    assert!(rate < -0.5, "a backwards record drove the deck at {rate}");
}

/// **Relative mode leaves the playhead alone.**
///
/// Not a lesser mode: a DJ nudging the record to beatmatch moves the needle
/// without meaning to move the playhead, and a deck that jumped back to where
/// the vinyl said it should be would undo the nudge every time.
#[test]
fn relative_mode_does_not_move_the_playhead() {
    let (mut rig, mut producer, synth) = timecode_rig(false);
    let near = stream(&synth, 1_000, 1.0, 20 * 256);
    for block in 0..20 {
        play_block(&mut rig, &mut producer, &near, block, 256);
    }
    let before = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::Position));

    // A record claiming to be ten minutes further on. Relative mode must
    // ignore the claim and keep following the movement.
    let far = stream(&synth, 600_000, 1.0, 40 * 256);
    for block in 0..40 {
        play_block(&mut rig, &mut producer, &far, block, 256);
    }
    let after = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::Position));

    let advanced = after - before;
    assert!(
        advanced > 0.0,
        "the deck did not advance at all: {advanced} frames"
    );
    assert!(
        advanced < 48_000.0 * 30.0,
        "relative mode jumped the playhead by {advanced} frames"
    );
}

/// Absolute mode does move it — that is what dropping the needle is for.
#[test]
fn absolute_mode_follows_the_needle() {
    let (mut rig, mut producer, synth) = timecode_rig(true);
    // Twenty seconds in. Inside the track: the deck holds 2,000,000 frames,
    // which is forty-one seconds, and a seek past the end is clamped to it --
    // so a landing point beyond the track would test the clamp, not the needle.
    let landing = 20_000u32;
    let signal = stream(&synth, landing, 1.0, 60 * 256);
    for block in 0..60 {
        play_block(&mut rig, &mut producer, &signal, block, 256);
    }
    let position = rig
        .registry
        .get(ParamId::Deck(deck(1), DeckParam::Position));
    let expected = f64::from(landing) / 1000.0 * SR.as_f64();
    let drift = (f64::from(position) - expected).abs();
    assert!(
        drift < SR.as_f64() * 2.0,
        "the needle landed at {expected} and the playhead went to {position}"
    );
}

/// Blocks of control signal, rendered up front.
///
/// The renderer allocates, so an allocation test must not call it inside the
/// window it is measuring — and because the counter behind `count_allocations`
/// is global while its "am I watching" flag is per-thread, allocating there
/// does not merely inflate this test's own figure: it corrupts whichever other
/// allocation test happens to be measuring at the same moment.
fn prerendered(synth: &dj_dvs::Synth, from: u32, blocks: usize, frames: usize) -> Vec<Vec<f32>> {
    // One continuous stretch, cut into blocks — see `stream`.
    synth
        .render(from, 1.0, blocks * frames)
        .chunks(frames * 2)
        .map(<[f32]>::to_vec)
        .collect()
}

/// Detaching hands the decoder back through the retirement queue rather than
/// freeing four megabytes on the audio thread.
#[test]
fn detaching_a_timecode_input_retires_it() {
    let (mut rig, mut producer, synth) = timecode_rig(false);
    let signal = stream(&synth, 10_000, 1.0, 256);
    play_block(&mut rig, &mut producer, &signal, 0, 256);

    rig.send(Command::SetTimecode {
        deck: deck(1),
        input: None,
    });
    rig.renderer.render_block();

    let retired = std::iter::from_fn(|| rig.retired.pop().ok()).count();
    assert!(
        retired > 0,
        "the decoder was dropped on the audio thread instead of retired"
    );
}

/// The decoder runs inside the callback, so it must not allocate — the table
/// was built before it ever got there.
#[test]
fn decoding_timecode_never_allocates() {
    let (mut rig, mut producer, synth) = timecode_rig(true);
    let warm = stream(&synth, 10_000, 1.0, 8 * 256);
    for block in 0..8 {
        play_block(&mut rig, &mut producer, &warm, block, 256);
    }

    // Rendered before the window opens: see `prerendered`.
    let blocks = prerendered(&synth, 10_000, 500, 256);

    let (_, allocations) = count_allocations(|| {
        for block in &blocks {
            for sample in block {
                let _ = producer.push(*sample);
            }
            rig.renderer.render_block();
            while rig.retired.pop().is_ok() {}
        }
    });
    assert_eq!(
        allocations, 0,
        "decoding timecode allocated {allocations} times"
    );
}

/// A deck with no control record is untouched — the feature is off unless
/// asked for, and must not cost a deck that never opts in.
#[test]
fn a_deck_without_a_record_is_left_alone() {
    let (mut rig, mut producer, synth) = timecode_rig(false);
    rig.load_and_play(2, 2_000_000);
    let signal = stream(&synth, 10_000, 0.5, 30 * 256);
    for block in 0..30 {
        play_block(&mut rig, &mut producer, &signal, block, 256);
    }
    let untouched = rig.registry.get(ParamId::Deck(deck(2), DeckParam::Rate));
    assert!(
        (untouched - 1.0).abs() < 0.01,
        "deck 2 has no record and was driven to {untouched}"
    );
}

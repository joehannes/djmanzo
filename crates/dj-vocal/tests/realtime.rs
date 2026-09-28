//! Proof that the singers' microphones do not allocate once they are made.
//!
//! The same counting allocator `dj-engine`'s `rt_safety` test installs: every
//! allocation on a thread that has opted in is counted, and eight strips —
//! every stage on, settings changed mid-way, voices coming and going, the ring
//! running dry — must make none.

// A `GlobalAlloc` implementation is unsafe by definition.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use dj_vocal::{EchoSettings, EqSettings, StripSettings, Vocals};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
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
    if WATCHING.try_with(Cell::get).unwrap_or(false) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn counted(body: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    WATCHING.with(|w| w.set(true));
    body();
    WATCHING.with(|w| w.set(false));
    ALLOCATIONS.load(Ordering::Relaxed) - before
}

#[test]
fn eight_singers_never_allocate() {
    const RATE: f32 = 48_000.0;
    const STRIPS: usize = 8;
    let mut vocals = Vocals::new(RATE, STRIPS);
    let (mut producer, consumer) = rtrb::RingBuffer::new(RATE as usize * STRIPS);
    vocals.set_input(Some(consumer));
    let everything = StripSettings {
        open: true,
        talkover: true,
        eq: Some(EqSettings {
            low_db: -3.0,
            mid_db: 2.0,
            mid_hz: 2_000.0,
            high_db: 2.0,
        }),
        echo: Some(EchoSettings {
            delay_ms: 250.0,
            feedback: 0.4,
            level: 0.3,
        }),
        ..StripSettings::default()
    };
    let changed = StripSettings {
        pan: 0.5,
        echo: Some(EchoSettings {
            delay_ms: 1_400.0,
            feedback: 0.8,
            level: 0.5,
        }),
        ..everything.clone()
    };

    let allocations = counted(|| {
        for index in 0..STRIPS {
            vocals.strip_mut(index).unwrap().apply(&everything);
        }
        let mut n = 0u32;
        for block in 0..40 {
            // Half a second of voices, then half a second of the ring
            // running dry, round and round; a setting changed mid-way.
            if block == 20 {
                vocals.strip_mut(3).unwrap().apply(&changed);
            }
            if block % 2 == 0 {
                for _ in 0..24_000 {
                    for channel in 0..STRIPS {
                        #[allow(clippy::cast_precision_loss)]
                        let v = (n as f32 * 0.03 + channel as f32).sin() * 0.3;
                        let _ = producer.push(v);
                    }
                    n = n.wrapping_add(1);
                }
            }
            for _ in 0..24_000 {
                std::hint::black_box(vocals.next_frame());
            }
        }
    });
    assert_eq!(allocations, 0, "the singers' microphones allocated");
    assert!(vocals.starved_frames() > 0, "the dry half never ran");
}

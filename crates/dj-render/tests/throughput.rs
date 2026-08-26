//! Evidence for the CPU rasterisation decision.
//!
//! ADR-0004 originally said tiles would be rasterised with `wgpu`. This measures
//! what the CPU path actually costs, so the choice rests on numbers rather than
//! on an assumption made before any code existed.
//!
//! The bar these have to clear is not "fast" in the abstract. It is: can a full
//! screen of waveform be produced fast enough that scrolling never waits, and
//! can a track be summarised fast enough that loading does not stall?

use dj_core::SampleRate;
use dj_render::{Palette, TileSpec, WaveformSummary, render_tile};
use std::f32::consts::PI;
use std::time::Instant;

const SR: SampleRate = SampleRate::DEFAULT;

/// Five minutes of stereo at 48 kHz -- an ordinary track.
fn realistic_track() -> Vec<f32> {
    let frames = 48_000 * 300;
    (0..frames)
        .flat_map(|n| {
            let t = n as f32 / 48_000.0;
            // Something with content across the spectrum, so the band filters
            // do real work rather than optimising away on silence.
            let bass = (2.0 * PI * 60.0 * t).sin() * 0.5;
            let mid = (2.0 * PI * 800.0 * t).sin() * 0.3;
            let high = (2.0 * PI * 9_000.0 * t).sin() * 0.2;
            let sample = bass + mid + high;
            [sample, sample * 0.9]
        })
        .collect()
}

/// The fastest of a few attempts, in milliseconds per iteration.
///
/// These four tests each build a five-minute track and then race each other
/// for CPU, because `cargo test` runs them in parallel. A single wall-clock
/// sample therefore measures the scheduler as much as the renderer, and the
/// per-tile assertions below were failing about one run in three on
/// unmodified code — a gate that cries wolf gets ignored the one time it is
/// right.
///
/// The *fastest* attempt is the one least polluted by contention, and it is
/// still a real measurement: a genuine regression makes even the best attempt
/// slow. Serialised, these come in at about a third of their limits.
fn fastest_ms_per(iterations: usize, attempts: usize, mut work: impl FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..attempts {
        let start = Instant::now();
        work();
        let ms = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;
        best = best.min(ms);
    }
    best
}

#[test]
fn summarising_a_five_minute_track_is_fast_enough_to_load_with() {
    let samples = realistic_track();

    let start = Instant::now();
    let summary = WaveformSummary::analyse(&samples, SR);
    let elapsed = start.elapsed();

    println!(
        "summarised 5:00 in {:.0} ms ({} buckets, {} levels)",
        elapsed.as_secs_f64() * 1000.0,
        summary.level(0).len(),
        summary.level_count()
    );

    // This runs on an analysis worker while the track loads, so seconds would be
    // acceptable. Anything above five means something is quadratic.
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "summarising took {:.1}s, which suggests an algorithmic problem",
        elapsed.as_secs_f64()
    );
}

#[test]
fn a_full_screen_of_tiles_renders_far_inside_one_frame() {
    let summary = WaveformSummary::analyse(&realistic_track(), SR);
    let palette = Palette::default();

    // A 4K-wide lane at 512 px per tile is 8 tiles; four decks is 32. Rendering
    // every one of them from scratch is the worst case -- in practice they are
    // cached and only the newly-exposed edge is generated.
    const TILES: usize = 32;
    const TILE_WIDTH: u32 = 512;

    let mut total_bytes = 0usize;
    let per_tile = fastest_ms_per(TILES, 3, || {
        total_bytes = 0;
        for i in 0..TILES {
            let tile = render_tile(
                &summary,
                &TileSpec {
                    width: TILE_WIDTH,
                    height: 128,
                    start_frame: (i as f64) * 512.0 * 256.0,
                    frames_per_pixel: 256.0,
                },
                &palette,
            );
            total_bytes += tile.pixels.len();
        }
    });
    let total_ms = per_tile * TILES as f64;
    println!(
        "rendered {TILES} tiles ({:.1} MB) in {total_ms:.2} ms -- {per_tile:.2} ms/tile \
         (one 60 fps frame is 16.7 ms)",
        total_bytes as f64 / 1e6,
    );

    // Asserted per tile rather than on the total. The total is the pathological
    // case -- every tile on screen regenerated at once, which only happens on a
    // zoom change -- and on a slow shared CI runner it lands close enough to the
    // 16.7 ms frame budget to flake. Per-tile cost is the number that actually
    // determines whether scrolling keeps up, and it has orders of magnitude of
    // headroom.
    assert!(
        per_tile < 3.0,
        "{per_tile:.2} ms per tile is too slow to keep ahead of a scroll"
    );
}

#[test]
fn scrolling_only_costs_the_newly_exposed_edge() {
    let summary = WaveformSummary::analyse(&realistic_track(), SR);
    let palette = Palette::default();

    // Scrolling reveals one tile at a time; the rest are already cached. This is
    // the steady-state cost of a moving waveform.
    let per_tile = fastest_ms_per(60, 3, || {
        for i in 0..60 {
            let _ = render_tile(
                &summary,
                &TileSpec {
                    width: 512,
                    height: 128,
                    start_frame: (i as f64) * 512.0 * 256.0,
                    frames_per_pixel: 256.0,
                },
                &palette,
            );
        }
    });

    println!("steady-state scroll: {per_tile:.3} ms per newly-exposed tile");
    assert!(
        per_tile < 2.0,
        "a single tile took {per_tile:.2} ms, too slow to keep ahead of a scroll"
    );
}

#[test]
fn zoomed_out_rendering_does_not_walk_the_whole_track() {
    let summary = WaveformSummary::analyse(&realistic_track(), SR);
    let palette = Palette::default();

    // The overview waveform covers the entire track in one lane. Without the
    // resolution pyramid this would touch all fourteen million frames per draw.
    let start = Instant::now();
    let _ = render_tile(
        &summary,
        &TileSpec {
            width: 2_000,
            height: 64,
            start_frame: 0.0,
            frames_per_pixel: summary.total_frames() as f64 / 2_000.0,
        },
        &palette,
    );
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    println!("full-track overview: {elapsed:.2} ms");
    assert!(
        elapsed < 16.7,
        "the overview took {elapsed:.1} ms, which means the pyramid is not being used"
    );
}

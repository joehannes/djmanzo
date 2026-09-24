//! The ONNX separator against real ONNX Runtime, on stand-in models with the
//! interface of an HT-Demucs export and none of its weights.
//!
//! The stand-ins (`tests/fixtures/`, written by
//! `scripts/stems-standin-models.py`) give each stem as the mix times a gain,
//! so what djmanzo makes of the output can be checked sample by sample:
//! drums 0.1, bass 0.2, other 0.3, vocals 0.4, in HT-Demucs's order.
//!
//! This is the test v0.23.0 did not have. Its engine called the model's
//! input `input`; the export calls it `mix`, and every chunk failed.
//!
//! It needs ONNX Runtime: set `ORT_DYLIB_PATH` to the library
//! `scripts/fetch-onnxruntime.sh` stages (CI does). Without one it says so
//! and passes, as the crate's other runtime tests do — it cannot show
//! anything about a runtime that is not there.

use dj_core::Stem;
use dj_stems::stems::Separator;
use dj_stems::{StemsEngine, Unavailable};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn stereo(frames: usize, rate: u32) -> Vec<f32> {
    (0..frames)
        .flat_map(|n| {
            let t = n as f32 / rate as f32;
            [
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.4,
                (t * 660.0 * std::f32::consts::TAU).sin() * 0.3,
            ]
        })
        .collect()
}

/// Worst difference between `stem` and `mix × gain`, skipping `edge` frames
/// at each end.
fn off_by(stem: &[f32], mix: &[f32], gain: f32, edge: usize) -> f32 {
    assert_eq!(stem.len(), mix.len());
    stem.iter()
        .zip(mix)
        .skip(edge * 2)
        .take(mix.len() - edge * 4)
        .map(|(s, m)| (s - m * gain).abs())
        .fold(0.0, f32::max)
}

/// Whether this machine has an ONNX Runtime djmanzo can use.
///
/// A library that opens is not enough: Windows carries an older
/// `onnxruntime.dll` of its own in System32, which the probe finds and `ort`
/// then refuses as too old. That is the machine saying it cannot run this,
/// not the model failing, so it skips too.
fn runtime_here() -> bool {
    let library = dj_stems::availability::runtime_library();
    if let Err(reason) = dj_stems::availability::probe_named_runtime(&library) {
        eprintln!("skipped: {reason} (set ORT_DYLIB_PATH to run this)");
        return false;
    }
    match StemsEngine::new(&fixture("standin-4.onnx")) {
        Err(reason @ Unavailable::Runtime { .. }) => {
            eprintln!("skipped: {reason} (set ORT_DYLIB_PATH to run this)");
            false
        }
        _ => true,
    }
}

/// One test, so the runtime is set up once and the cases run in order.
#[test]
fn an_htdemucs_shaped_model_separates_into_the_right_stems() {
    if !runtime_here() {
        return;
    }

    // -- the export's own interface: `mix` in, `stems` out, fixed segment --
    let engine = StemsEngine::new(&fixture("standin-4.onnx")).expect("the stand-in loads");
    assert_eq!(engine.contract().input, "mix");
    assert_eq!(engine.contract().output, "stems");
    assert_eq!(engine.contract().segment, Some(1024));

    // Ten segments and a bit, at the model's own rate: exact.
    let mix = stereo(10_500, 44_100);
    let stems = Separator::separate(&engine, &mix, 44_100).expect("a chunk separates");
    for (stem, gain) in [
        (Stem::Vocal, 0.4),
        (Stem::Drums, 0.1),
        (Stem::Bass, 0.2),
        (Stem::Other, 0.3),
    ] {
        let off = off_by(stems.get(stem), &mix, gain, 0);
        assert!(off < 1e-5, "{stem} is off by {off}");
    }

    // At 48 kHz it is taken to the model's rate and back: the same stems,
    // as close as resampling twice allows, and exactly as long.
    let mix = stereo(24_000, 48_000);
    let stems = Separator::separate(&engine, &mix, 48_000).expect("a 48 kHz chunk separates");
    assert_eq!(stems.get(Stem::Vocal).len(), mix.len());
    let off = off_by(stems.get(Stem::Vocal), &mix, 0.4, 200);
    assert!(off < 5e-3, "vocals at 48 kHz are off by {off}");

    // -- six stems: guitar and piano are other --
    let six = StemsEngine::new(&fixture("standin-6.onnx")).expect("the six-stem stand-in loads");
    let mix = stereo(3_000, 44_100);
    let stems = Separator::separate(&six, &mix, 44_100).unwrap();
    assert!(off_by(stems.get(Stem::Other), &mix, 0.45, 0) < 1e-5);
    assert!(off_by(stems.get(Stem::Vocal), &mix, 0.25, 0) < 1e-5);

    // -- other names, open length: read by position, run once --
    let open = StemsEngine::new(&fixture("standin-dynamic.onnx")).expect("the open stand-in loads");
    assert_eq!(open.contract().input, "input");
    assert_eq!(open.contract().segment, None);
    let mix = stereo(5_000, 44_100);
    let stems = Separator::separate(&open, &mix, 44_100).unwrap();
    assert!(off_by(stems.get(Stem::Vocal), &mix, 0.4, 0) < 1e-5);

    // -- a model that is not a separator is refused at load, with why --
    match StemsEngine::new(&fixture("standin-wrong.onnx")) {
        Err(Unavailable::Unsuited { reason }) => {
            assert!(reason.contains("stems out"), "{reason}");
        }
        other => panic!("expected the model to be refused, got {other:?}"),
    }
}

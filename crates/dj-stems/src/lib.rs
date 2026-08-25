pub mod availability;
pub mod cache;
pub mod worker;

pub use availability::Unavailable;

use ort::session::{Session, builder::GraphOptimizationLevel};
use std::path::Path;
use std::sync::Mutex;

/// The number of threads ONNX Runtime may use for one separation.
///
/// Separation is a background job competing with the audio callback for cores.
/// Four is enough to be useful on a laptop and leaves room for the mixer; the
/// audio thread is never on this path either way.
const INTRA_THREADS: usize = 4;

pub struct StemsEngine {
    session: Mutex<Session>,
}

impl StemsEngine {
    /// Load a separation model, or say why it cannot be loaded.
    ///
    /// # Order of checks
    ///
    /// The runtime is probed **before anything in `ort` is touched**, and
    /// before the model is looked for. Both parts of that matter:
    ///
    /// - probing first is what keeps a machine without ONNX Runtime from
    ///   aborting at process exit -- see [`availability`] for the mechanism;
    /// - probing the *runtime* first means a machine missing both is told
    ///   about the runtime, which is the one that no download of a model will
    ///   fix.
    pub fn new(model_path: &Path) -> Result<Self, Unavailable> {
        availability::probe_runtime()?;
        availability::probe_model(model_path)?;

        // Only now is it safe to call into `ort`.
        let _ = ort::init().with_name("djmanzo-stems").commit();

        let build = || -> Result<Session, ort::Error> {
            Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .with_intra_threads(INTRA_THREADS)?
                // TODO: configure Execution Providers (CoreML, DirectML, CUDA,
                // CPU) requires feature flags enabled in Cargo.toml for
                // specific EPs.
                .commit_from_file(model_path)
        };
        let session = build().map_err(|error| Unavailable::Session {
            reason: error.to_string(),
        })?;

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Run inference on a batch of audio samples.
    /// `input` is expected to be interleaved stereo `f32` (L, R, L, R...).
    /// Returns 4 channels of stems: Vocal, Drums, Bass, Other as interleaved stereo `f32`.
    pub fn separate(&self, input: &[f32]) -> Result<Vec<Vec<f32>>, ort::Error> {
        let frames = input.len() / 2;

        // De-interleave into [channels, samples]
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for frame in input.chunks_exact(2) {
            left.push(frame[0]);
            right.push(frame[1]);
        }

        // Shape: [batch_size, channels, samples] -> [1, 2, frames]
        let tensor = ort::value::Tensor::from_array(([1, 2, frames], [left, right].concat()))?;
        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs!["input" => tensor])?;

        let (_shape, slice) = outputs["output"].try_extract_tensor::<f32>()?;

        // Expected output shape from models like HTDemucs is [batch, stems, channels, samples] -> [1, 4, 2, frames]
        // If the model shape differs, this logic will panic or fail.
        let mut stems = Vec::with_capacity(4);
        for stem_idx in 0..4 {
            let mut interleaved = Vec::with_capacity(frames * 2);
            let stem_left = &slice[stem_idx * 2 * frames..stem_idx * 2 * frames + frames];
            let stem_right =
                &slice[stem_idx * 2 * frames + frames..stem_idx * 2 * frames + 2 * frames];
            for i in 0..frames {
                interleaved.push(stem_left[i]);
                interleaved.push(stem_right[i]);
            }
            stems.push(interleaved);
        }

        Ok(stems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the probe, and the reason this test is worth more
    /// than its assertions: if [`StemsEngine::new`] reaches `ort` on a machine
    /// with no ONNX Runtime, `ort` panics inside a `Once` and poisons a mutex
    /// that its own `atexit` handler then locks. That handler cannot unwind,
    /// so the *test binary aborts on exit* -- every test in this crate is
    /// reported as passing and then the process dies with a non-zero status.
    ///
    /// So the assertion below is only half the test. The other half is that
    /// the process survives long enough to report it.
    ///
    /// Both mutations were run, and they fail differently:
    ///
    /// - deleting `probe_runtime()?` alone does **not** abort, because
    ///   `probe_model` still returns before `ort` is reached. It is caught by
    ///   [`the_runtime_is_reported_before_the_model`], which then reports a
    ///   missing model on a machine that has no runtime either;
    /// - deleting both probes aborts the test binary with SIGABRT and
    ///   "thread caused non-unwinding panic", after every test in the crate
    ///   has already printed `ok`.
    ///
    /// The second is the failure a DJ would have had on their laptop, and it
    /// is why the model check is not sufficient on its own: it only masks the
    /// abort for as long as no model file exists. Ship a model, keep no
    /// runtime, and the abort comes back.
    ///
    /// [`the_runtime_is_reported_before_the_model`]: #method.the_runtime_is_reported_before_the_model
    #[test]
    fn a_missing_runtime_is_an_error_not_an_abort() {
        // Point the probe at a name that cannot resolve, so the outcome does
        // not depend on whether the machine running the tests happens to have
        // ONNX Runtime installed.
        let missing = availability::probe_named_runtime("libonnxruntime-not-here-4b1c.so");
        assert!(
            matches!(missing, Err(Unavailable::Runtime { .. })),
            "{missing:?}"
        );

        // And the real entry point: whatever this machine has, asking for a
        // model that is not there must come back as a value.
        let engine = StemsEngine::new(Path::new("/nowhere/htdemucs.onnx"));
        match engine {
            Err(Unavailable::Runtime { .. } | Unavailable::Model { .. }) => {}
            Err(other) => panic!("unexpected reason: {other}"),
            Ok(_) => panic!("there is no model at that path"),
        }
    }

    /// A machine with neither part should be told about the runtime, because
    /// downloading a model would not help it. Ordering the two probes the
    /// other way round would send that DJ looking for the wrong thing.
    #[test]
    fn the_runtime_is_reported_before_the_model() {
        if availability::probe_runtime().is_ok() {
            // This machine has ONNX Runtime, so it cannot demonstrate the
            // ordering. Skip rather than assert something untrue of it.
            return;
        }
        let engine = StemsEngine::new(Path::new("/nowhere/htdemucs.onnx"));
        // `StemsEngine` holds an ONNX session and is not `Debug`, so report
        // the reason rather than the whole result.
        let reason = engine.err().map(|reason| reason.to_string());
        assert!(
            matches!(reason.as_deref(), Some(text) if text.contains("ONNX Runtime")),
            "with no runtime the model path is beside the point, got {reason:?}"
        );
    }
}

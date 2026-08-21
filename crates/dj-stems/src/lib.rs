pub mod cache;
pub mod worker;

use ort::session::{builder::GraphOptimizationLevel, Session};
use std::path::Path;
use std::sync::Mutex;

pub struct StemsEngine {
    session: Mutex<Session>,
}

impl StemsEngine {
    pub fn new(model_path: &Path) -> Result<Self, ort::Error> {
        let _ = ort::init().with_name("djmanzo-stems").commit();

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            // TODO: configure Execution Providers (CoreML, DirectML, CUDA, CPU)
            // requires feature flags enabled in Cargo.toml for specific EPs.
            .commit_from_file(model_path)?;

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
        let tensor = ort::value::Tensor::from_array(([1, 2, frames], vec![left, right].concat()))?;
        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs!["input" => tensor])?;
        
        let (_shape, slice) = outputs["output"].try_extract_tensor::<f32>()?;
        
        // Expected output shape from models like HTDemucs is [batch, stems, channels, samples] -> [1, 4, 2, frames]
        // If the model shape differs, this logic will panic or fail.
        let mut stems = Vec::with_capacity(4);
        for stem_idx in 0..4 {
            let mut interleaved = Vec::with_capacity(frames * 2);
            let stem_left = &slice[stem_idx * 2 * frames .. stem_idx * 2 * frames + frames];
            let stem_right = &slice[stem_idx * 2 * frames + frames .. stem_idx * 2 * frames + 2 * frames];
            for i in 0..frames {
                interleaved.push(stem_left[i]);
                interleaved.push(stem_right[i]);
            }
            stems.push(interleaved);
        }

        Ok(stems)
    }
}

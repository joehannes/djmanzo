pub mod cache;
pub mod worker;

use ort::session::{builder::GraphOptimizationLevel, Session};
use std::path::Path;
use std::sync::Arc;

pub struct StemsEngine {
    session: Arc<Session>,
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
            session: Arc::new(session),
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

        // In a real implementation, we would build an ndarray and run the session:
        // let array = ndarray::Array3::from_shape_vec((1, 2, frames), vec![left, right].concat())?;
        // let tensor = ort::Value::from_array(self.session.allocator(), &array)?;
        // let outputs = self.session.run(ort::inputs!["input" => tensor]?)?;

        // For now, return dummy silence buffers of the correct interleaved size.
        let out_buffer = vec![0.0; input.len()];
        Ok(vec![
            out_buffer.clone(), // Vocal
            out_buffer.clone(), // Drums
            out_buffer.clone(), // Bass
            out_buffer,         // Other
        ])
    }
}

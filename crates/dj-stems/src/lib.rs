pub mod cache;

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
    /// Returns 4 channels of stems: Vocal, Drums, Bass, Other.
    pub fn separate(&self, input: &[f32]) -> Result<Vec<Vec<f32>>, ort::Error> {
        // TODO: Implement actual tensor packing/unpacking for Demucs
        // Demucs takes shape [batch, channels, samples] and outputs [batch, sources, channels, samples]
        Ok(vec![
            vec![0.0; input.len()], // Vocal
            vec![0.0; input.len()], // Drums
            vec![0.0; input.len()], // Bass
            vec![0.0; input.len()], // Other
        ])
    }
}

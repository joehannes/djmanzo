use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// A realtime-safe Spectral Analyzer for extracting frequency bands.
/// Pre-allocates all buffers and lookup tables.
#[derive(Clone)]
pub struct SpectralAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    fft_buffer: Vec<Complex<f32>>,
    
    /// A sliding window of audio, sized to the FFT length.
    history: Vec<f32>,
    /// Where we write next in the sliding window.
    cursor: usize,
    
    /// Pre-calculated Hann window multiplier table.
    window_table: Vec<f32>,

    /// The frequency width of each FFT bin in Hz.
    bin_width: f32,
    
    /// The normalization factor applied to the output magnitudes.
    norm_factor: f32,
}

impl std::fmt::Debug for SpectralAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpectralAnalyzer")
            .finish()
    }
}

impl SpectralAnalyzer {
    pub fn new(size: usize, sample_rate: f32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(size);
        let scratch_len = fft.get_inplace_scratch_len();
        
        let mut window_table = Vec::with_capacity(size);
        for i in 0..size {
            let multiplier = 0.5 * (1.0 - (std::f32::consts::TAU * (i as f32) / ((size - 1) as f32)).cos());
            window_table.push(multiplier);
        }

        Self {
            fft,
            fft_scratch: vec![Complex { re: 0.0, im: 0.0 }; scratch_len],
            fft_buffer: vec![Complex { re: 0.0, im: 0.0 }; size],
            history: vec![0.0; size],
            cursor: 0,
            window_table,
            bin_width: sample_rate / (size as f32),
            norm_factor: 2.0 / (size as f32),
        }
    }

    /// Push a single mono sample into the sliding window.
    #[inline]
    pub fn push(&mut self, sample: f32) {
        self.history[self.cursor] = sample;
        self.cursor = (self.cursor + 1) % self.history.len();
    }

    /// Run the FFT and extract the 4 normalized bands: (Bass, LowMid, HighMid, Treble)
    pub fn process_bands(&mut self) -> (f32, f32, f32, f32) {
        let n = self.fft_buffer.len();
        
        // Copy history into complex buffer applying the window table
        for i in 0..n {
            let history_idx = (self.cursor + i) % n;
            self.fft_buffer[i] = Complex {
                re: self.history[history_idx] * self.window_table[i],
                im: 0.0,
            };
        }

        self.fft.process_with_scratch(&mut self.fft_buffer, &mut self.fft_scratch);

        let mut bass = 0.0f32;
        let mut low_mid = 0.0f32;
        let mut high_mid = 0.0f32;
        let mut treble = 0.0f32;

        // Discard DC offset (bin 0) and only read the first half of the symmetric output
        for (i, complex) in self.fft_buffer.iter().enumerate().take(n / 2).skip(1) {
            let freq = (i as f32) * self.bin_width;
            let mag = complex.norm();
            
            if freq < 250.0 {
                bass += mag;
            } else if freq < 2000.0 {
                low_mid += mag;
            } else if freq < 6000.0 {
                high_mid += mag;
            } else if freq < 20000.0 {
                treble += mag;
            }
        }

        (
            (bass * self.norm_factor).clamp(0.0, 1.0),
            (low_mid * self.norm_factor).clamp(0.0, 1.0),
            (high_mid * self.norm_factor).clamp(0.0, 1.0),
            (treble * self.norm_factor).clamp(0.0, 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectral_bands_no_allocation() {
        let mut analyzer = SpectralAnalyzer::new(1024, 44100.0);
        
        // Ensure no panics and sensible empty state
        let (b, lm, hm, t) = analyzer.process_bands();
        assert_eq!(b, 0.0);
        
        // Generate a synthetic 50Hz sine wave (pure bass)
        let sample_rate = 44100.0;
        let freq_bass = 50.0;
        for i in 0..1024 {
            let t = (i as f32) / sample_rate;
            let sample = (std::f32::consts::TAU * freq_bass * t).sin();
            analyzer.push(sample);
        }
        
        let (bass, low_mid, high_mid, treble) = analyzer.process_bands();
        
        // Bass should be significantly higher than other bands
        assert!(bass > 0.1, "Expected significant bass energy, got {}", bass);
        assert!(low_mid < 0.1, "Expected low mid to be quiet, got {}", low_mid);
        assert!(high_mid < 0.1, "Expected high mid to be quiet, got {}", high_mid);
        assert!(treble < 0.1, "Expected treble to be quiet, got {}", treble);

        // Generate a synthetic 10kHz sine wave (pure treble)
        let freq_treble = 10000.0;
        for i in 0..1024 {
            let t = (i as f32) / sample_rate;
            let sample = (std::f32::consts::TAU * freq_treble * t).sin();
            analyzer.push(sample);
        }

        let (bass, low_mid, high_mid, treble) = analyzer.process_bands();
        assert!(treble > 0.1, "Expected significant treble energy, got {}", treble);
        assert!(bass < 0.1, "Expected bass to be quiet, got {}", bass);
    }
}

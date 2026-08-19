//! Session Context for the Adaptive UI
//!
//! Exposes physical and session-level state (time of day, phase, mood, energy)
//! so the adaptive SVG interface (Adaptive Flora) can shift its geometry and color.

use serde::{Deserialize, Serialize};

/// The overall phase of a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    #[default]
    WarmUp,
    Heat,
    Peak,
    Cooldown,
    ChillOut,
}

/// Simulated external physical context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentContext {
    pub time_of_day: String, // e.g., "Dawn", "Noon", "Dusk", "Night"
    pub weather: String,     // e.g., "Clear", "Rain", "Snow"
    pub temperature_c: f32,
}

impl Default for EnvironmentContext {
    fn default() -> Self {
        Self {
            time_of_day: "Night".to_string(),
            weather: "Clear".to_string(),
            temperature_c: 20.0,
        }
    }
}

/// Live audio stream metrics exposed to the UI for audio-reactive themes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioMetrics {
    /// 0.0 to 1.0 (clamped human-sensible limits based on RMS/peak)
    pub momentary_loudness: f32,
    /// Spectral bands [Bass, LowMid, HighMid, Treble], 0.0 to 1.0. 
    /// (Currently a proxy/placeholder until the DSP FFT node is wired).
    pub spectral_bands: [f32; 4],
}

/// The entire context state that the UI uses to morph SVG controls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionContext {
    pub phase: SessionPhase,
    pub environment: EnvironmentContext,
    /// 0.0 to 1.0 - inferred from BPM, loudness, or manual override
    pub energy_level: f32,
    /// Real-time audio analysis for pulse/throb animations
    pub audio: AudioMetrics,
}

impl SessionContext {
    /// A fast, default mock context for development until the real engine fills it.
    pub fn mock_peak_time() -> Self {
        Self {
            phase: SessionPhase::Peak,
            environment: EnvironmentContext {
                time_of_day: "Night".to_string(),
                weather: "Clear".to_string(),
                temperature_c: 28.0,
            },
            energy_level: 0.95,
            audio: AudioMetrics {
                momentary_loudness: 0.8,
                spectral_bands: [0.9, 0.6, 0.5, 0.8],
            },
        }
    }
    
    pub fn mock_warm_up() -> Self {
        Self {
            phase: SessionPhase::WarmUp,
            environment: EnvironmentContext {
                time_of_day: "Dusk".to_string(),
                weather: "Rain".to_string(),
                temperature_c: 12.0,
            },
            energy_level: 0.3,
            audio: AudioMetrics {
                momentary_loudness: 0.2,
                spectral_bands: [0.3, 0.2, 0.1, 0.1],
            },
        }
    }
}

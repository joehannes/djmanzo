//! Devices and stream configuration.

use dj_core::SampleRate;

/// Opaque handle for a device, stable enough to persist in settings.
///
/// Backed by the device name, which is what every platform actually gives us.
/// It is not unique in principle -- two identical interfaces can report the same
/// name -- so device selection falls back to the default when a saved id no
/// longer resolves, rather than refusing to start.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(String);

impl DeviceId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What we can tell the user about an output device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    /// Channels the device can output. Four or more means headphone cueing can
    /// share one interface with the master, which is the setup most controllers
    /// with a built-in card provide.
    pub max_output_channels: u16,
    pub default_sample_rate: SampleRate,
    pub is_default: bool,
}

impl DeviceInfo {
    /// True when master and cue can live on this one device.
    #[must_use]
    pub fn supports_split_output(&self) -> bool {
        self.max_output_channels >= 4
    }
}

/// What we ask a backend to open.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamConfig {
    /// `None` means the system default.
    pub device: Option<DeviceId>,
    pub sample_rate: SampleRate,
    /// Frames per callback. Smaller is lower latency and higher risk of xruns.
    pub buffer_frames: u32,
    pub channels: u16,
}

impl StreamConfig {
    /// 256 frames at 48 kHz is ~5.3 ms -- the target in `ARCHITECTURE.md §4`.
    pub const DEFAULT_BUFFER_FRAMES: u32 = 256;

    #[must_use]
    pub fn latency_ms(&self) -> f64 {
        f64::from(self.buffer_frames) / self.sample_rate.as_f64() * 1000.0
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            device: None,
            sample_rate: SampleRate::DEFAULT,
            buffer_frames: Self::DEFAULT_BUFFER_FRAMES,
            channels: 2,
        }
    }
}

/// What a backend actually gave us.
///
/// Separate from [`StreamConfig`] because a request is not a promise: devices
/// routinely refuse a buffer size or rate and substitute their own. The UI shows
/// this, not the request, so the latency figure on screen is the real one.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveConfig {
    pub device_name: String,
    pub sample_rate: SampleRate,
    pub buffer_frames: u32,
    pub channels: u16,
}

impl ActiveConfig {
    #[must_use]
    pub fn latency_ms(&self) -> f64 {
        f64::from(self.buffer_frames) / self.sample_rate.as_f64() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_hits_the_latency_target() {
        let config = StreamConfig::default();
        assert!(
            (config.latency_ms() - 5.333).abs() < 0.01,
            "expected ~5.3ms, got {}",
            config.latency_ms()
        );
    }

    #[test]
    fn latency_scales_with_buffer_and_rate() {
        let config = StreamConfig {
            buffer_frames: 512,
            sample_rate: SampleRate::new(96_000).unwrap(),
            ..Default::default()
        };
        assert!((config.latency_ms() - 5.333).abs() < 0.01);
    }

    #[test]
    fn four_channels_means_cue_can_share_the_device() {
        let mut info = DeviceInfo {
            id: DeviceId::new("x"),
            name: "x".into(),
            max_output_channels: 2,
            default_sample_rate: SampleRate::DEFAULT,
            is_default: true,
        };
        assert!(!info.supports_split_output());
        info.max_output_channels = 4;
        assert!(info.supports_split_output());
    }
}

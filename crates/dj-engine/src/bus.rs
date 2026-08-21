//! Output bus layout.
//!
//! A DJ mixer has more than one output. The master goes to the PA, the booth
//! goes to the monitors at its own level, and the headphones carry whatever the
//! DJ is cueing. Which physical channels those land on depends entirely on the
//! interface: a two-channel laptop output has room for the master only, while a
//! controller with a built-in card typically gives four.

/// Which output channels each bus occupies.
///
/// Channel indices into the interleaved buffer the device hands over. Derived
/// from the channel count rather than configured, because the mapping is a
/// convention every interface follows: master first, then booth, then cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusLayout {
    pub channels: usize,
    /// Master output. Always present -- there is nowhere else for audio to go.
    pub main: (usize, usize),
    /// Headphone cue. Absent on a two-channel device.
    pub cue: Option<(usize, usize)>,
    /// Booth output with independent level. Needs six channels.
    pub booth: Option<(usize, usize)>,
    /// Direct outputs for external processing (e.g. per-deck or per-stem).
    pub direct_outs: Vec<(usize, usize)>,
}

impl BusLayout {
    /// Work out the layout for a device with `channels` outputs.
    ///
    /// - 1: mono master. Unusual, but a device that offers it should still work.
    /// - 2: master only. Cueing needs a second device, which arrives with
    ///   dual-device support.
    /// - 4: master + cue. The common controller layout.
    /// - 6+: master + booth + cue.
    #[must_use]
    pub fn for_channels(channels: usize) -> Self {
        match channels {
            0 | 1 => Self {
                channels: channels.max(1),
                main: (0, 0),
                cue: None,
                booth: None,
                direct_outs: Vec::new(),
            },
            2 | 3 => Self {
                channels,
                main: (0, 1),
                cue: None,
                booth: None,
                direct_outs: Vec::new(),
            },
            4 | 5 => Self {
                channels,
                main: (0, 1),
                cue: Some((2, 3)),
                booth: None,
                direct_outs: Vec::new(),
            },
            _ => Self {
                channels,
                main: (0, 1),
                booth: Some((2, 3)),
                cue: Some((4, 5)),
                direct_outs: Vec::new(),
            },
        }
    }

    /// True when the device can carry a headphone cue.
    #[must_use]
    pub fn has_cue(&self) -> bool {
        self.cue.is_some()
    }

    #[must_use]
    pub fn has_booth(&self) -> bool {
        self.booth.is_some()
    }

    /// True when the master is a single channel, so stereo must be summed.
    #[must_use]
    pub fn is_mono(&self) -> bool {
        self.main.0 == self.main.1
    }

    /// Frames in a buffer of `samples` interleaved samples.
    #[must_use]
    pub fn frames(&self, samples: usize) -> usize {
        samples / self.channels.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_has_no_cue() {
        let layout = BusLayout::for_channels(2);
        assert_eq!(layout.main, (0, 1));
        assert!(!layout.has_cue());
        assert!(!layout.has_booth());
    }

    #[test]
    fn four_channels_gives_master_and_cue() {
        let layout = BusLayout::for_channels(4);
        assert_eq!(layout.main, (0, 1));
        assert_eq!(layout.cue, Some((2, 3)));
        assert!(!layout.has_booth());
    }

    #[test]
    fn six_channels_adds_a_booth() {
        let layout = BusLayout::for_channels(6);
        assert_eq!(layout.main, (0, 1));
        assert_eq!(layout.booth, Some((2, 3)));
        assert_eq!(layout.cue, Some((4, 5)));
    }

    /// Buses must never share a channel, or the cue would bleed into the master
    /// and the audience would hear whatever the DJ is previewing.
    #[test]
    fn buses_never_overlap() {
        for channels in [2usize, 4, 6, 8] {
            let layout = BusLayout::for_channels(channels);
            let mut used = vec![layout.main.0, layout.main.1];
            if let Some((l, r)) = layout.cue {
                used.push(l);
                used.push(r);
            }
            if let Some((l, r)) = layout.booth {
                used.push(l);
                used.push(r);
            }
            let unique: std::collections::HashSet<_> = used.iter().copied().collect();
            assert_eq!(
                unique.len(),
                used.len(),
                "buses overlap at {channels} channels: {used:?}"
            );
        }
    }

    #[test]
    fn every_channel_index_is_in_range() {
        for channels in [1usize, 2, 4, 6, 8] {
            let layout = BusLayout::for_channels(channels);
            let mut indices = vec![layout.main.0, layout.main.1];
            if let Some((l, r)) = layout.cue {
                indices.extend([l, r]);
            }
            if let Some((l, r)) = layout.booth {
                indices.extend([l, r]);
            }
            for index in indices {
                assert!(
                    index < layout.channels,
                    "channel {index} out of range for {channels}"
                );
            }
        }
    }

    #[test]
    fn mono_is_detected_and_survives() {
        let layout = BusLayout::for_channels(1);
        assert!(layout.is_mono());
        assert!(!layout.has_cue());
        assert_eq!(layout.frames(64), 64);
    }

    #[test]
    fn zero_channels_does_not_divide_by_zero() {
        let layout = BusLayout::for_channels(0);
        assert_eq!(layout.frames(64), 64);
    }
}

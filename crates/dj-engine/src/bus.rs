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
    /// Where each separated stem goes, when a deck is being sent out in parts.
    ///
    /// `None` normally, which is every set that is not being fed through an
    /// external processor.
    pub stems: Option<[(usize, usize); 4]>,
}

/// How many outputs sending a deck out in parts needs.
///
/// Four stems, two channels each. There is no arrangement that fits in fewer,
/// and no honest way to send three of four.
pub const STEM_OUT_CHANNELS: usize = 8;

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
                stems: None,
            },
            2 | 3 => Self {
                channels,
                main: (0, 1),
                cue: None,
                booth: None,
                stems: None,
            },
            4 | 5 => Self {
                channels,
                main: (0, 1),
                cue: Some((2, 3)),
                booth: None,
                stems: None,
            },
            _ => Self {
                channels,
                main: (0, 1),
                booth: Some((2, 3)),
                cue: Some((4, 5)),
                stems: None,
            },
        }
    }

    /// Send a deck out in parts instead of mixing it.
    ///
    /// **This takes the whole output.** Four stems need eight channels, which
    /// leaves nowhere for the master — and that is the point: a DJ sending
    /// stems to an external mixer is monitoring from the external mixer.
    ///
    /// Returns the layout unchanged when the device has fewer than
    /// [`STEM_OUT_CHANNELS`] outputs, because there is no honest way to send
    /// three stems of four.
    #[must_use]
    pub fn with_stem_out(mut self) -> Self {
        if self.channels < STEM_OUT_CHANNELS {
            return self;
        }
        self.stems = Some([(0, 1), (2, 3), (4, 5), (6, 7)]);
        self
    }

    /// True when a deck is being sent out in parts.
    #[must_use]
    pub fn is_stem_out(&self) -> bool {
        self.stems.is_some()
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

/// A bus arrangement a device's own mapping asked for.
///
/// [`BusLayout::for_channels`] guesses from the channel count, which is right
/// for most devices and wrong for the ones that arrange their sockets
/// differently -- and wrong here means the room hears the cue. A controller
/// that states its arrangement gets it honoured instead.
///
/// Plain pairs rather than a `dj-hid` type, so the engine does not depend on
/// the controller crate: the engine's business is where the audio goes, not
/// where the instruction came from.
///
/// `Copy` and free of allocation because it crosses to the audio thread and is
/// read there once a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusRouting {
    pub main: (usize, usize),
    pub cue: Option<(usize, usize)>,
    pub booth: Option<(usize, usize)>,
}

impl BusRouting {
    #[must_use]
    pub fn new(
        main: (usize, usize),
        cue: Option<(usize, usize)>,
        booth: Option<(usize, usize)>,
    ) -> Self {
        Self { main, cue, booth }
    }

    /// How many outputs a device needs for this arrangement to fit.
    #[must_use]
    pub fn channels_needed(&self) -> usize {
        [Some(self.main), self.cue, self.booth]
            .into_iter()
            .flatten()
            .flat_map(|(left, right)| [left, right])
            .max()
            .map_or(0, |highest| highest + 1)
    }

    /// This arrangement on a device with `channels` outputs.
    ///
    /// `None` when it does not fit, which is the whole reason the check lives
    /// here rather than at each call site: a routing written for a controller
    /// with six sockets, applied unchecked to the laptop's built-in stereo
    /// output, would index past the end of the buffer the device handed over.
    /// The caller falls back to [`BusLayout::for_channels`], which is a worse
    /// answer than the mapping's and a much better one than a crash.
    #[must_use]
    pub fn layout(&self, channels: usize) -> Option<BusLayout> {
        if self.channels_needed() > channels {
            return None;
        }
        Some(BusLayout {
            channels,
            main: self.main,
            cue: self.cue,
            booth: self.booth,
            stems: None,
        })
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

    /// The case the type exists for: a controller whose master is not first.
    #[test]
    fn a_routing_overrides_the_guess() {
        let routing = BusRouting::new((2, 3), Some((0, 1)), None);
        let layout = routing.layout(4).expect("four channels is enough for four");
        assert_eq!(layout.main, (2, 3));
        assert_eq!(layout.cue, Some((0, 1)));
        assert_ne!(layout.main, BusLayout::for_channels(4).main);
    }

    /// A routing that does not fit must be refused rather than clamped: every
    /// index it names is written into the device's buffer, and there is no
    /// honest way to squeeze channel 5 into a stereo output.
    #[test]
    fn a_routing_wider_than_the_device_does_not_fit() {
        let routing = BusRouting::new((0, 1), Some((4, 5)), None);
        assert_eq!(routing.channels_needed(), 6);
        assert!(routing.layout(2).is_none());
        assert!(routing.layout(4).is_none());
        assert!(routing.layout(6).is_some());
    }

    /// Every index a routing hands back has to be inside the buffer, whatever
    /// the mapping said -- this is the assertion that stands between a typo in
    /// a controller file and a write past the end of the device's buffer.
    #[test]
    fn a_fitting_routing_names_only_channels_that_exist() {
        for (main, cue, booth, channels) in [
            ((0usize, 1usize), None, None, 2usize),
            ((2, 3), Some((0, 1)), None, 4),
            ((0, 1), Some((4, 5)), Some((2, 3)), 6),
            ((6, 7), Some((0, 1)), Some((2, 3)), 8),
        ] {
            let routing = BusRouting::new(main, cue, booth);
            let layout = routing
                .layout(channels)
                .expect("this routing fits by construction");
            let mut indices = vec![layout.main.0, layout.main.1];
            if let Some((l, r)) = layout.cue {
                indices.extend([l, r]);
            }
            if let Some((l, r)) = layout.booth {
                indices.extend([l, r]);
            }
            for index in indices {
                assert!(index < channels, "channel {index} is past {channels}");
            }
        }
    }

    /// A device with room to spare keeps its own channel count, because the
    /// buffer is still that wide however few sockets the mapping names.
    #[test]
    fn a_narrow_routing_on_a_wide_device_keeps_the_device_count() {
        let routing = BusRouting::new((0, 1), Some((2, 3)), None);
        let layout = routing.layout(8).expect("four channels of eight");
        assert_eq!(layout.channels, 8);
        assert_eq!(layout.frames(64), 8);
    }
}

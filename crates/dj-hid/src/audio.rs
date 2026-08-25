//! Which sockets on a controller carry which bus.
//!
//! A controller with a built-in soundcard has a fixed arrangement: the DDJ-400
//! puts the room on outputs 1-2 and the headphones on 3-4, and the manual says
//! so. [`dj_engine::BusLayout::for_channels`] guesses that arrangement from the
//! channel count, which is right for most devices and wrong for the ones that
//! do it differently -- and "wrong" here means the room hears the cue.
//!
//! So a mapping may state it. The same file that says which pad is play says
//! which socket is the master, because they are the same fact about the same
//! piece of hardware and a DJ should not have to find one of them in a
//! settings panel with a crowd waiting.
//!
//! # The one thing that must never happen
//!
//! **The master and the cue may not share a channel.** Cueing is listening to
//! what the room is not hearing; a layout where they overlap plays the next
//! track out loud while the DJ lines it up. It is refused when the file loads,
//! because there is no moment later at which finding out is any use.

use serde::{Deserialize, Serialize};

/// A stereo pair of output channels, zero-based as the device counts them.
pub type Pair = (usize, usize);

/// Where a controller's buses come out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPreset {
    /// Part of the audio device's name, matched the same loose way a MIDI port
    /// is: a device announces itself differently on every platform.
    #[serde(default)]
    pub device: String,
    /// The room. Two channel numbers.
    pub master: Vec<usize>,
    /// The headphones. Absent on a device that has none.
    #[serde(default)]
    pub cue: Option<Vec<usize>>,
    /// A booth feed with its own level.
    #[serde(default)]
    pub booth: Option<Vec<usize>>,
}

/// A validated preset: pairs rather than vectors, and known not to overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioRouting {
    pub master: Pair,
    pub cue: Option<Pair>,
    pub booth: Option<Pair>,
    /// The highest channel named, plus one -- what the device must provide.
    pub channels_needed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioPresetError {
    #[error("{0} needs exactly two channels, got {1}")]
    NotAPair(&'static str, usize),
    #[error("the master and the cue share channel {0}, so the room would hear the cue")]
    MasterMeetsCue(usize),
    #[error("the master and the booth share channel {0}")]
    MasterMeetsBooth(usize),
    #[error("the cue and the booth share channel {0}")]
    CueMeetsBooth(usize),
    #[error("this device has {available} outputs and the mapping asks for channel {wanted}")]
    NotEnoughChannels { wanted: usize, available: usize },
}

impl AudioPreset {
    /// Check the preset and turn it into routing.
    ///
    /// # Errors
    /// If a bus is not a pair, or if two buses share a channel.
    pub fn routing(&self) -> Result<AudioRouting, AudioPresetError> {
        let master = pair("the master", &self.master)?;
        let cue = self
            .cue
            .as_deref()
            .map(|c| pair("the cue", c))
            .transpose()?;
        let booth = self
            .booth
            .as_deref()
            .map(|b| pair("the booth", b))
            .transpose()?;

        // Overlap is checked between every pair of buses, not only against the
        // master, because a booth that doubled the cue would send the
        // headphones to a second room.
        if let Some(cue) = cue
            && let Some(shared) = overlap(master, cue)
        {
            return Err(AudioPresetError::MasterMeetsCue(shared));
        }
        if let Some(booth) = booth {
            if let Some(shared) = overlap(master, booth) {
                return Err(AudioPresetError::MasterMeetsBooth(shared));
            }
            if let Some(cue) = cue
                && let Some(shared) = overlap(cue, booth)
            {
                return Err(AudioPresetError::CueMeetsBooth(shared));
            }
        }

        let highest = [Some(master), cue, booth]
            .into_iter()
            .flatten()
            .flat_map(|(left, right)| [left, right])
            .max()
            .unwrap_or(0);

        Ok(AudioRouting {
            master,
            cue,
            booth,
            channels_needed: highest + 1,
        })
    }

    /// Whether this preset is for `device`, matched loosely.
    ///
    /// The same rule a MIDI port uses, for the same reason: a device is
    /// "DDJ-400" on one platform and "PIONEER DDJ-400 Analog Stereo" on
    /// another, and an exact match would work on the machine it was written on
    /// and nowhere else.
    #[must_use]
    pub fn fits(&self, device: &str) -> bool {
        if self.device.is_empty() {
            return false;
        }
        device.to_lowercase().contains(&self.device.to_lowercase())
    }
}

impl AudioRouting {
    /// Check the routing against a device that actually has `channels`.
    ///
    /// Separate from [`AudioPreset::routing`] because the file can be read
    /// long before the device is opened -- and a preset that is right for the
    /// hardware it was written for should still say something useful when
    /// opened against the laptop's built-in output.
    ///
    /// # Errors
    /// If the device has fewer outputs than the preset names.
    pub fn check_against(&self, channels: usize) -> Result<(), AudioPresetError> {
        if self.channels_needed > channels {
            return Err(AudioPresetError::NotEnoughChannels {
                wanted: self.channels_needed - 1,
                available: channels,
            });
        }
        Ok(())
    }
}

fn pair(what: &'static str, channels: &[usize]) -> Result<Pair, AudioPresetError> {
    match channels {
        [left, right] => Ok((*left, *right)),
        other => Err(AudioPresetError::NotAPair(what, other.len())),
    }
}

/// A channel both pairs use, if there is one.
fn overlap(a: Pair, b: Pair) -> Option<usize> {
    [a.0, a.1]
        .into_iter()
        .find(|channel| *channel == b.0 || *channel == b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(master: &[usize], cue: Option<&[usize]>) -> AudioPreset {
        AudioPreset {
            device: "DDJ".to_owned(),
            master: master.to_vec(),
            cue: cue.map(<[usize]>::to_vec),
            booth: None,
        }
    }

    #[test]
    fn the_common_controller_arrangement_is_accepted() {
        let routing = preset(&[0, 1], Some(&[2, 3]))
            .routing()
            .expect("a normal deck");
        assert_eq!(routing.master, (0, 1));
        assert_eq!(routing.cue, Some((2, 3)));
        assert_eq!(routing.channels_needed, 4);
    }

    /// The reason a preset exists at all: some controllers do not put the
    /// master first, and guessing from the channel count gets those backwards.
    #[test]
    fn a_controller_that_puts_the_cue_first_is_accepted() {
        let routing = preset(&[2, 3], Some(&[0, 1]))
            .routing()
            .expect("unusual but real");
        assert_eq!(routing.master, (2, 3));
        assert_eq!(routing.cue, Some((0, 1)));
    }

    /// **The one thing that must never happen.** Cueing is listening to what
    /// the room is not hearing. A layout where the two overlap plays the next
    /// track out loud while the DJ lines it up.
    #[test]
    fn a_master_that_overlaps_the_cue_is_refused() {
        assert_eq!(
            preset(&[0, 1], Some(&[1, 2])).routing().unwrap_err(),
            AudioPresetError::MasterMeetsCue(1)
        );
        assert_eq!(
            preset(&[0, 1], Some(&[0, 1])).routing().unwrap_err(),
            AudioPresetError::MasterMeetsCue(0)
        );
        // Overlap on the second channel only, which a check that looked at the
        // first alone would miss.
        assert_eq!(
            preset(&[0, 1], Some(&[2, 1])).routing().unwrap_err(),
            AudioPresetError::MasterMeetsCue(1)
        );
    }

    /// The booth is a second room, so it must not double the headphones
    /// either -- that would put the cue on a speaker.
    #[test]
    fn a_booth_that_overlaps_another_bus_is_refused() {
        let with_booth = |booth: &[usize]| AudioPreset {
            device: "DDJ".to_owned(),
            master: vec![0, 1],
            cue: Some(vec![2, 3]),
            booth: Some(booth.to_vec()),
        };
        assert_eq!(
            with_booth(&[1, 4]).routing().unwrap_err(),
            AudioPresetError::MasterMeetsBooth(1)
        );
        assert_eq!(
            with_booth(&[3, 4]).routing().unwrap_err(),
            AudioPresetError::CueMeetsBooth(3)
        );
        assert!(with_booth(&[4, 5]).routing().is_ok());
    }

    /// A bus is a stereo pair. One channel is not a mistake worth guessing
    /// about, and three is a typo.
    #[test]
    fn a_bus_that_is_not_a_pair_is_refused() {
        assert_eq!(
            preset(&[0], None).routing().unwrap_err(),
            AudioPresetError::NotAPair("the master", 1)
        );
        assert_eq!(
            preset(&[0, 1, 2], None).routing().unwrap_err(),
            AudioPresetError::NotAPair("the master", 3)
        );
        assert_eq!(
            preset(&[0, 1], Some(&[2])).routing().unwrap_err(),
            AudioPresetError::NotAPair("the cue", 1)
        );
    }

    /// A device with no headphones is a normal thing -- most laptops -- and
    /// must not be an error.
    #[test]
    fn a_preset_with_no_cue_is_fine() {
        let routing = preset(&[0, 1], None).routing().expect("a master is enough");
        assert_eq!(routing.cue, None);
        assert_eq!(routing.channels_needed, 2);
    }

    /// **The moment this matters in practice.** A DJ writes a preset for their
    /// controller, then opens the laptop's built-in output. The preset names
    /// channel 3 and the laptop has two, so it has to say so rather than
    /// route the cue into nothing.
    #[test]
    fn a_preset_that_wants_more_channels_than_the_device_has_says_so() {
        let routing = preset(&[0, 1], Some(&[2, 3])).routing().unwrap();
        assert_eq!(
            routing.check_against(2).unwrap_err(),
            AudioPresetError::NotEnoughChannels {
                wanted: 3,
                available: 2
            }
        );
        assert!(routing.check_against(4).is_ok());
        assert!(routing.check_against(8).is_ok(), "more than enough is fine");
    }

    #[test]
    fn a_device_is_matched_the_loose_way_a_midi_port_is() {
        let preset = preset(&[0, 1], None);
        assert!(preset.fits("DDJ-400"));
        assert!(preset.fits("PIONEER DDJ-400 Analog Stereo"));
        assert!(
            preset.fits("pioneer ddj-400"),
            "matching is case-insensitive"
        );
        assert!(!preset.fits("Built-in Output"));
    }

    /// An empty device name matches nothing rather than everything. The other
    /// way round, a preset with the field left out would claim every device on
    /// the machine.
    #[test]
    fn a_preset_with_no_device_claims_nothing() {
        let mut nameless = preset(&[0, 1], None);
        nameless.device = String::new();
        assert!(!nameless.fits("DDJ-400"));
        assert!(!nameless.fits(""));
    }
}

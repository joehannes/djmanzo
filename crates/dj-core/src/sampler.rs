//! The sampler's vocabulary: banks, slots and how a pad behaves.
//!
//! Names and shapes only, like [`crate::fx`] — the audio lives in `dj-engine`.
//! Here for the same reason: a pad, a script line, a controller mapping and the
//! assistant all have to say "trigger slot 3" long before anything makes a
//! sound, and [ADR-0003](../../docs/adr/0003-action-bus-and-parameter-registry.md)
//! has one vocabulary for all of them.

use serde::{Deserialize, Serialize};

/// Slots per bank.
///
/// Eight, to match the pad grid exactly. A sampler bank that did not fit the
/// pads would need a scroll or a second page, and a sample you have to scroll
/// to is a sample you will not reach in time.
pub const SAMPLE_SLOTS: usize = 8;

/// Banks.
///
/// Four, so a set can carry a bank of drops, a bank of vocals, a bank of
/// risers and one spare without the switch itself becoming a thing to manage.
pub const SAMPLE_BANKS: usize = 4;

/// How a pad behaves when it is pressed.
///
/// Four modes that are genuinely four behaviours. The temptation is to add a
/// fifth that is one of these under another name; the test below exists to
/// stop that, because a mode that duplicates another is a choice a DJ has to
/// make for no reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TriggerMode {
    /// Press plays from the start to the end and releases do nothing. Pressing
    /// again while it sounds starts it over.
    ///
    /// The default, because it is what a pad does on every piece of hardware
    /// and what a sample usually is: a thing you fire and let finish.
    #[default]
    OneShot,
    /// Sounds only while held, from the start. Release stops it.
    Hold,
    /// Press starts it looping; press again stops it.
    Loop,
    /// Sounds while held *and* restarts on every press, so machine-gunning the
    /// pad gives the stutter it is named for. The difference from `Hold` is
    /// only what a second press does while the first is still sounding — which
    /// is small on paper and the entire effect in practice.
    Stutter,
}

impl TriggerMode {
    pub const ALL: [TriggerMode; 4] = [
        TriggerMode::OneShot,
        TriggerMode::Hold,
        TriggerMode::Loop,
        TriggerMode::Stutter,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            TriggerMode::OneShot => "one_shot",
            TriggerMode::Hold => "hold",
            TriggerMode::Loop => "loop",
            TriggerMode::Stutter => "stutter",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }

    /// Whether releasing the pad stops the sound.
    #[must_use]
    pub const fn is_momentary(self) -> bool {
        matches!(self, TriggerMode::Hold | TriggerMode::Stutter)
    }

    /// Whether a press while it is already sounding starts it over.
    #[must_use]
    pub const fn retriggers(self) -> bool {
        matches!(self, TriggerMode::OneShot | TriggerMode::Stutter)
    }

    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0)
    }

    #[must_use]
    pub fn from_index(index: usize) -> Self {
        Self::ALL
            .get(index)
            .copied()
            .unwrap_or(TriggerMode::OneShot)
    }
}

/// Where a slot's audio goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SampleOutput {
    /// Into the mix, like a deck. What a sample is for.
    #[default]
    Master,
    /// Headphones only, so a sample can be lined up before anyone hears it.
    ///
    /// The same courtesy the decks get. A sampler that can only be auditioned
    /// in front of the room is a sampler you use twice and then stop trusting.
    Cue,
}

/// One change to one sampler slot.
///
/// The same shape as [`crate::fx::FxChange`], and for the same reason: eight
/// slots times four banks times six controls is not a vocabulary anyone —
/// human or model — should have to read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SampleChange {
    /// Fire it, however its mode says to.
    Trigger,
    /// Let go. Only means anything for a momentary mode.
    Release,
    /// Stop it now, whatever it was doing.
    Stop,
    SetMode(TriggerMode),
    /// 0..=1.
    Volume(f32),
    Route(SampleOutput),
    /// Follow the master tempo, stretching the sample to fit.
    SetSync(bool),
    /// Forget what is loaded.
    Clear,
}

impl std::fmt::Display for SampleChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SampleChange::Trigger => write!(f, "trigger"),
            SampleChange::Release => write!(f, "release"),
            SampleChange::Stop => write!(f, "stop"),
            SampleChange::SetMode(mode) => write!(f, "{}", mode.name()),
            SampleChange::Volume(v) => write!(f, "volume {v}"),
            SampleChange::Route(SampleOutput::Master) => write!(f, "master"),
            SampleChange::Route(SampleOutput::Cue) => write!(f, "cue"),
            SampleChange::SetSync(true) => write!(f, "sync"),
            SampleChange::SetSync(false) => write!(f, "sync_off"),
            SampleChange::Clear => write!(f, "clear"),
        }
    }
}

/// A change to the sampler as a whole rather than to one slot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SamplerChange {
    /// Switch banks, 1-based.
    Bank(u8),
    /// The sampler's own level, 0..=1.
    Volume(f32),
    /// Silence everything, in every bank.
    ///
    /// The panic button. A sampler with eight loops running and no way to stop
    /// them in one gesture is a sampler that will one day be the loudest thing
    /// in the room.
    StopAll,
}

impl std::fmt::Display for SamplerChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SamplerChange::Bank(n) => write!(f, "bank {n}"),
            SamplerChange::Volume(v) => write!(f, "volume {v}"),
            SamplerChange::StopAll => write!(f, "stop_all"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_name_and_index() {
        for mode in TriggerMode::ALL {
            assert_eq!(TriggerMode::parse(mode.name()), Some(mode));
            assert_eq!(TriggerMode::from_index(mode.index()), mode);
        }
        assert_eq!(TriggerMode::parse("machine_gun"), None);
        // A registry read before anything was written is zero, which has to
        // mean the default rather than an arbitrary mode.
        assert_eq!(TriggerMode::from_index(0), TriggerMode::default());
        assert_eq!(TriggerMode::from_index(99), TriggerMode::default());
    }

    /// The guard against a fifth mode that is a fourth under another name.
    ///
    /// Two modes are the same behaviour if they agree on both questions a mode
    /// answers: does releasing stop it, and does a second press restart it.
    /// Every pair must differ on at least one.
    #[test]
    fn no_two_modes_behave_identically() {
        for (index, a) in TriggerMode::ALL.iter().enumerate() {
            for b in &TriggerMode::ALL[index + 1..] {
                assert!(
                    a.is_momentary() != b.is_momentary() || a.retriggers() != b.retriggers(),
                    "{} and {} are the same mode under two names",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    /// The four modes, spelt out, so a change to the table is a change to a
    /// test rather than a silent change to what a pad does.
    #[test]
    fn each_mode_behaves_the_way_its_name_says() {
        assert!(!TriggerMode::OneShot.is_momentary());
        assert!(TriggerMode::OneShot.retriggers());

        assert!(TriggerMode::Hold.is_momentary());
        assert!(!TriggerMode::Hold.retriggers());

        assert!(!TriggerMode::Loop.is_momentary());
        assert!(!TriggerMode::Loop.retriggers());

        assert!(TriggerMode::Stutter.is_momentary());
        assert!(TriggerMode::Stutter.retriggers());
    }

    /// A bank has to fit the pads exactly, or a sample needs scrolling to
    /// reach — and a sample you scroll to is one you will not reach in time.
    #[test]
    fn a_bank_is_exactly_one_pad_page() {
        assert_eq!(SAMPLE_SLOTS, crate::pads::PADS);
    }
}

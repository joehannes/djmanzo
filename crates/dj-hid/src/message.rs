//! What arrives from a device.
//!
//! MIDI's own wire format, decoded just far enough to be matched against a
//! mapping. Deliberately not an abstraction over "controls": a mapping file
//! names notes and controller numbers because that is what is printed in a
//! controller's own documentation, and a layer that renamed them would mean
//! reading two documents instead of one.

/// One decoded message.
///
/// Channel is 0-based here and 1-based in a mapping file, because a mapping
/// file is read by a person and the wire is not. [`Message::from_bytes`] is the
/// only place that distinction lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// A pad or button going down. Velocity is kept: some controllers send
    /// note-on with velocity 0 to mean note-off, and some pads are pressure
    /// sensitive.
    NoteOn { channel: u8, note: u8, velocity: u8 },
    /// A pad or button coming up.
    NoteOff { channel: u8, note: u8 },
    /// A knob, fader or encoder.
    Control {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// A pitch bend wheel, or a high-resolution fader that uses one. 14 bits.
    PitchBend { channel: u8, value: u16 },
}

impl Message {
    /// Decode a MIDI message, or `None` if it is one this does not handle.
    ///
    /// Clock, sysex, aftertouch and program change all arrive on a busy
    /// controller and none of them are mapped, so they are dropped here rather
    /// than in every caller.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let status = *bytes.first()?;
        let kind = status & 0xF0;
        let channel = status & 0x0F;
        match kind {
            0x80 => Some(Message::NoteOff {
                channel,
                note: *bytes.get(1)? & 0x7F,
            }),
            0x90 => {
                let note = *bytes.get(1)? & 0x7F;
                let velocity = *bytes.get(2)? & 0x7F;
                // Note-on at velocity zero *is* a note-off. Devices differ, and
                // a mapping that had to handle both spellings would get it
                // wrong on half of them.
                if velocity == 0 {
                    Some(Message::NoteOff { channel, note })
                } else {
                    Some(Message::NoteOn {
                        channel,
                        note,
                        velocity,
                    })
                }
            }
            0xB0 => Some(Message::Control {
                channel,
                controller: *bytes.get(1)? & 0x7F,
                value: *bytes.get(2)? & 0x7F,
            }),
            0xE0 => {
                let low = u16::from(*bytes.get(1)? & 0x7F);
                let high = u16::from(*bytes.get(2)? & 0x7F);
                Some(Message::PitchBend {
                    channel,
                    value: (high << 7) | low,
                })
            }
            _ => None,
        }
    }

    /// The channel it arrived on, 0-based.
    #[must_use]
    pub const fn channel(self) -> u8 {
        match self {
            Message::NoteOn { channel, .. }
            | Message::NoteOff { channel, .. }
            | Message::Control { channel, .. }
            | Message::PitchBend { channel, .. } => channel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_decodes_with_its_channel() {
        assert_eq!(
            Message::from_bytes(&[0x91, 0x3C, 0x7F]),
            Some(Message::NoteOn {
                channel: 1,
                note: 60,
                velocity: 127,
            })
        );
        assert_eq!(
            Message::from_bytes(&[0x81, 0x3C, 0x40]),
            Some(Message::NoteOff {
                channel: 1,
                note: 60,
            })
        );
    }

    /// **Half the controllers in the world do this.** A note-on at velocity
    /// zero is a note-off, and a mapping that had to handle both spellings
    /// would work on one make and not the other.
    #[test]
    fn a_note_on_at_zero_velocity_is_a_note_off() {
        assert_eq!(
            Message::from_bytes(&[0x90, 0x24, 0x00]),
            Some(Message::NoteOff {
                channel: 0,
                note: 36,
            })
        );
    }

    #[test]
    fn a_controller_decodes() {
        assert_eq!(
            Message::from_bytes(&[0xB2, 0x07, 0x64]),
            Some(Message::Control {
                channel: 2,
                controller: 7,
                value: 100,
            })
        );
    }

    /// Pitch bend is little-endian across two seven-bit halves, which is the
    /// one place MIDI's byte order surprises people.
    #[test]
    fn pitch_bend_is_fourteen_bits_low_byte_first() {
        assert_eq!(
            Message::from_bytes(&[0xE0, 0x00, 0x40]),
            Some(Message::PitchBend {
                channel: 0,
                value: 8_192,
            }),
            "centre"
        );
        assert_eq!(
            Message::from_bytes(&[0xE0, 0x7F, 0x7F]),
            Some(Message::PitchBend {
                channel: 0,
                value: 16_383,
            }),
            "top"
        );
        assert_eq!(
            Message::from_bytes(&[0xE0, 0x00, 0x00]),
            Some(Message::PitchBend {
                channel: 0,
                value: 0,
            }),
            "bottom"
        );
    }

    /// A controller sends clock twenty-four times a beat whether anything is
    /// listening or not. Dropping the kinds nothing maps here means no caller
    /// has to.
    #[test]
    fn what_is_not_mapped_is_dropped_here() {
        assert_eq!(Message::from_bytes(&[0xF8]), None, "clock");
        assert_eq!(Message::from_bytes(&[0xFE]), None, "active sensing");
        assert_eq!(Message::from_bytes(&[0xC0, 0x01]), None, "program change");
        assert_eq!(Message::from_bytes(&[0xD0, 0x40]), None, "aftertouch");
        assert_eq!(Message::from_bytes(&[]), None, "nothing at all");
    }

    /// A truncated message must not panic. USB delivers what it delivers.
    #[test]
    fn a_short_message_is_dropped_rather_than_panicking() {
        assert_eq!(Message::from_bytes(&[0x90]), None);
        assert_eq!(Message::from_bytes(&[0x90, 0x3C]), None);
        assert_eq!(Message::from_bytes(&[0xE0, 0x00]), None);
    }

    /// The high bit is the status flag, so a data byte that has one is a device
    /// misbehaving. Masking rather than refusing keeps one bad byte from
    /// silencing a control for the rest of the set.
    #[test]
    fn data_bytes_are_masked_to_seven_bits() {
        assert_eq!(
            Message::from_bytes(&[0xB0, 0xFF, 0xFF]),
            Some(Message::Control {
                channel: 0,
                controller: 127,
                value: 127,
            })
        );
    }
}

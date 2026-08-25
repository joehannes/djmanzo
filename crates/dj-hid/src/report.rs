//! Reading a control out of a raw HID input report.
//!
//! # Why HID needs its own layer at all
//!
//! MIDI is **edge**-based: a controller sends a note-on when the pad goes down
//! and a note-off when it comes up, and says nothing in between. HID is
//! **level**-based: the device sends the state of every control it has, in one
//! packet, as often as it likes -- up to a thousand times a second on a device
//! that reports at 1 ms. Nothing in that packet says what just changed.
//!
//! So the whole job here is turning level into edge. A field is read out of
//! each report, compared with the last value seen, and only a *change* becomes
//! an action. Without that, holding the play button would send "play" a
//! thousand times a second, and a fader sitting still would flood the action
//! bus with the value it already had.
//!
//! # Why the mapping has to say where the field is
//!
//! A HID report is opaque bytes. Nothing in it is labelled: byte 3 is the
//! transport buttons on one controller and the crossfader on another, and only
//! the device's descriptor or its manual says which. djmanzo does not guess --
//! the mapping states the offset, exactly as it states a note number for MIDI.
//!
//! # What HID buys, and it is one thing
//!
//! Resolution. A 7-bit MIDI control gives 128 steps; a HID jog wheel reports
//! 16 bits, which is 65,536. That is the whole reason a DJ would put up with
//! writing byte offsets by hand, and it is why [`Width::Word`] and
//! [`Width::WordLe`] exist beside the byte.

/// How wide a field is, and how to read it.
///
/// Endianness is **declared, not guessed**, for the same reason the encoder
/// convention is: the two orderings produce completely different numbers from
/// the same two bytes, and a jog wheel read the wrong way round jumps between
/// its two halves instead of turning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Width {
    /// One bit of one byte: a button, a switch, a touch sensor.
    Bit(u8),
    /// One whole byte. 0-255.
    Byte,
    /// Two bytes, high byte first. 0-65535.
    Word,
    /// Two bytes, low byte first. The commoner of the two on USB devices,
    /// which is why it is worth being able to say so.
    WordLe,
}

impl Width {
    /// The largest value this width can hold.
    #[must_use]
    pub fn max(self) -> u32 {
        match self {
            Width::Bit(_) => 1,
            Width::Byte => 0xFF,
            Width::Word | Width::WordLe => 0xFFFF,
        }
    }

    /// How many bytes it occupies, for the bounds check.
    #[must_use]
    pub fn bytes(self) -> usize {
        match self {
            Width::Bit(_) | Width::Byte => 1,
            Width::Word | Width::WordLe => 2,
        }
    }

    /// Whether this is a switch rather than a range.
    ///
    /// The distinction the whole binding layer turns on: a bit becomes a press
    /// and a release, everything else becomes a movement.
    #[must_use]
    pub fn is_switch(self) -> bool {
        matches!(self, Width::Bit(_))
    }
}

/// Where a control lives inside a report, and how wide it is.
///
/// `report` is the report ID. Devices that use numbered reports put it in the
/// first byte; devices that do not use `0`, and then the whole packet is
/// payload. Both are handled, because both are common and a mapping should not
/// have to know which convention the operating system used to hand the packet
/// over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Field {
    pub report: u8,
    /// Byte offset **into the payload**, after any report ID.
    pub offset: usize,
    pub width: Width,
}

impl Field {
    #[must_use]
    pub fn new(report: u8, offset: usize, width: Width) -> Self {
        Self {
            report,
            offset,
            width,
        }
    }

    /// This field's value in `report`, or `None` when the packet is not for
    /// this field.
    ///
    /// `None` covers three different things, all of which are normal traffic
    /// on a live device rather than errors:
    ///
    /// - a report with a different ID, which is most of them on a device that
    ///   sends several kinds;
    /// - a report too short to contain the field, which happens when a device
    ///   sends a shorter status packet down the same pipe;
    /// - an empty packet.
    ///
    /// Refusing loudly would mean a warning per packet at up to a thousand a
    /// second. Saying nothing is the correct answer to a packet that is not
    /// about you.
    #[must_use]
    pub fn read(&self, report: &[u8]) -> Option<u32> {
        let payload = self.payload(report)?;
        let bytes = payload.get(self.offset..self.offset + self.width.bytes())?;
        Some(match self.width {
            // A bit index past the end of a byte cannot match anything, so it
            // reads as clear rather than shifting into nonsense.
            Width::Bit(bit) if bit < 8 => u32::from((bytes[0] >> bit) & 1),
            Width::Bit(_) => 0,
            Width::Byte => u32::from(bytes[0]),
            Width::Word => u32::from(u16::from_be_bytes([bytes[0], bytes[1]])),
            Width::WordLe => u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
        })
    }

    /// The payload of `report`, if this field's report ID matches it.
    ///
    /// A field on report 0 reads the packet whole. A field on any other report
    /// expects the ID in the first byte and reads what follows -- which is what
    /// makes an offset in a mapping file mean the same thing as an offset in
    /// the device's manual, where the ID is not part of the numbering.
    fn payload<'a>(&self, report: &'a [u8]) -> Option<&'a [u8]> {
        if self.report == 0 {
            return Some(report);
        }
        match report.split_first() {
            Some((id, rest)) if *id == self.report => Some(rest),
            _ => None,
        }
    }

    /// This field as `on = "..."` text the parser will read back.
    #[must_use]
    pub fn describe(&self) -> String {
        match self.width {
            Width::Bit(bit) => format!("hid {} bit {}.{}", self.report, self.offset, bit),
            Width::Byte => format!("hid {} byte {}", self.report, self.offset),
            Width::Word => format!("hid {} word {}", self.report, self.offset),
            Width::WordLe => format!("hid {} word-le {}", self.report, self.offset),
        }
    }
}

/// The field that changed between two reports of the same kind.
///
/// # Why this is the only way HID is mappable by hand
///
/// A MIDI controller announces itself: press a pad and a note number arrives,
/// which is the number you write in the file. A HID report is anonymous bytes,
/// and a DJ with an undocumented controller has nothing to type. So the editor
/// watches two consecutive reports and says what moved.
///
/// # Reading the change, not the value
///
/// A single bit flipping is a **button**; a whole byte moving is a **fader**.
/// The distinction is made from the size of the change, because that is the
/// only evidence there is -- and it is the right evidence: a button's byte
/// changes by exactly one bit and a fader sweeping past changes several.
///
/// `None` when nothing changed, when the reports are different lengths (a
/// device that sends more than one kind), or when several bytes moved at once
/// -- which happens when a DJ brushes two controls, and guessing between them
/// would bind the wrong one.
#[must_use]
pub fn changed_field(report: u8, before: &[u8], after: &[u8]) -> Option<Field> {
    if before.len() != after.len() {
        return None;
    }
    // Skip the report ID when there is one, so the offset counts from where
    // the device's own manual counts from.
    let skip = usize::from(report != 0);
    let before = before.get(skip..)?;
    let after = after.get(skip..)?;

    let mut moved = before
        .iter()
        .zip(after)
        .enumerate()
        .filter(|(_, (a, b))| a != b);
    let (offset, (was, now)) = moved.next()?;

    // Two bytes moving together is a sixteen-bit control -- the case HID
    // exists for, and worth recognising rather than reporting as noise.
    if let Some((next, _)) = moved.next() {
        if next == offset + 1 && moved.next().is_none() {
            // Which order the two bytes are in cannot be told from one step,
            // so the commoner one is offered and the DJ corrects it if the
            // wheel runs backwards. Saying "word-le" and being wrong is a
            // one-word edit; saying nothing leaves them with no mapping.
            return Some(Field::new(report, offset, Width::WordLe));
        }
        // Three or more bytes at once: a brushed control, or two at the same
        // time. There is no honest answer.
        return None;
    }

    // Exactly one bit different in one byte is a switch.
    let difference = was ^ now;
    if difference.count_ones() == 1 {
        #[allow(clippy::cast_possible_truncation)]
        return Some(Field::new(
            report,
            offset,
            Width::Bit(difference.trailing_zeros() as u8),
        ));
    }
    Some(Field::new(report, offset, Width::Byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bit_is_lifted_out_of_its_byte() {
        // Report 1, payload 0b0000_0100 at offset 0: bit 2 set, nothing else.
        let packet = [1u8, 0b0000_0100];
        assert_eq!(Field::new(1, 0, Width::Bit(2)).read(&packet), Some(1));
        assert_eq!(Field::new(1, 0, Width::Bit(3)).read(&packet), Some(0));
        assert_eq!(Field::new(1, 0, Width::Bit(0)).read(&packet), Some(0));
    }

    #[test]
    fn a_byte_is_read_whole() {
        let packet = [1u8, 0x00, 0x7F, 0xFF];
        assert_eq!(Field::new(1, 1, Width::Byte).read(&packet), Some(0x7F));
        assert_eq!(Field::new(1, 2, Width::Byte).read(&packet), Some(0xFF));
    }

    /// The reason both orderings exist. The same two bytes are 258 one way
    /// round and 513 the other, and a jog wheel read backwards jumps between
    /// its halves instead of turning.
    #[test]
    fn the_two_byte_orders_give_different_numbers() {
        let packet = [1u8, 0x01, 0x02];
        assert_eq!(Field::new(1, 0, Width::Word).read(&packet), Some(0x0102));
        assert_eq!(Field::new(1, 0, Width::WordLe).read(&packet), Some(0x0201));
        assert_ne!(
            Field::new(1, 0, Width::Word).read(&packet),
            Field::new(1, 0, Width::WordLe).read(&packet)
        );
    }

    /// What HID is for. Sixteen bits is 65,536 positions where MIDI's seven
    /// give 128 -- the difference between a jog wheel that scratches and one
    /// that steps.
    #[test]
    fn sixteen_bits_reach_every_value_seven_cannot() {
        let packet = [1u8, 0xFF, 0xFF];
        assert_eq!(Field::new(1, 0, Width::Word).read(&packet), Some(65_535));
        assert_eq!(Width::Word.max(), 65_535);
        assert_eq!(Width::Byte.max(), 255);
        assert_eq!(Width::Bit(0).max(), 1);
    }

    /// A device that sends several kinds of report sends them all down one
    /// pipe. A field on report 1 must ignore report 2 entirely -- reading
    /// offset 0 of the wrong report would be a button pressing itself.
    #[test]
    fn a_field_ignores_reports_that_are_not_its_own() {
        let other = [2u8, 0xFF, 0xFF];
        assert_eq!(Field::new(1, 0, Width::Byte).read(&other), None);
        assert_eq!(Field::new(2, 0, Width::Byte).read(&other), Some(0xFF));
    }

    /// Report 0 means the device does not number its reports, so the packet is
    /// payload from the first byte. Reading it as though byte 0 were an ID
    /// would shift every offset in the mapping by one.
    #[test]
    fn report_zero_reads_the_packet_whole() {
        let packet = [0xAAu8, 0xBB];
        assert_eq!(Field::new(0, 0, Width::Byte).read(&packet), Some(0xAA));
        assert_eq!(Field::new(0, 1, Width::Byte).read(&packet), Some(0xBB));
    }

    /// Short packets are ordinary traffic, not errors: a device sends a
    /// shorter status report down the same pipe. Reading past the end has to
    /// be nothing, never a panic and never a stale byte.
    #[test]
    fn a_packet_too_short_for_the_field_reads_as_nothing() {
        let short = [1u8, 0x40];
        assert_eq!(Field::new(1, 0, Width::Word).read(&short), None);
        assert_eq!(Field::new(1, 4, Width::Byte).read(&short), None);
        assert_eq!(Field::new(1, 0, Width::Byte).read(&short), Some(0x40));
        assert_eq!(Field::new(1, 0, Width::Byte).read(&[]), None);
        assert_eq!(Field::new(0, 0, Width::Byte).read(&[]), None);
    }

    /// Pressing a pad moves one bit. That is what makes it a pad, and it is
    /// the only evidence the editor has.
    #[test]
    fn one_bit_moving_is_read_as_a_button() {
        let before = [1u8, 0b0000_0000, 0x40];
        let after = [1u8, 0b0000_0100, 0x40];
        assert_eq!(
            changed_field(1, &before, &after),
            Some(Field::new(1, 0, Width::Bit(2)))
        );
        // And letting go names the same control, or the release would be
        // bound to something else.
        assert_eq!(
            changed_field(1, &after, &before),
            Some(Field::new(1, 0, Width::Bit(2)))
        );
    }

    /// A fader sweeping moves several bits of one byte. Calling that a bit
    /// would bind the pad to whichever bit happened to flip.
    #[test]
    fn several_bits_of_one_byte_is_read_as_a_fader() {
        let before = [1u8, 0, 0x40];
        let after = [1u8, 0, 0x7F];
        assert_eq!(
            changed_field(1, &before, &after),
            Some(Field::new(1, 1, Width::Byte))
        );
    }

    /// Two adjacent bytes moving together is the sixteen-bit control HID
    /// exists for -- a jog wheel. Reported as one field, not two.
    #[test]
    fn two_adjacent_bytes_are_read_as_one_wide_control() {
        let before = [1u8, 0x00, 0x00, 0xFF];
        let after = [1u8, 0x00, 0x01, 0x01];
        assert_eq!(
            changed_field(1, &before, &after),
            Some(Field::new(1, 1, Width::WordLe))
        );
    }

    /// Nothing moved, so nothing is learned. Otherwise the editor would bind
    /// the first control it saw to whatever the DJ pressed next.
    #[test]
    fn an_unchanged_report_teaches_nothing() {
        let same = [1u8, 0x0F, 0x40];
        assert_eq!(changed_field(1, &same, &same), None);
    }

    /// A DJ brushing two controls on the way to the right one must not bind
    /// either. Guessing between them is worse than asking again.
    #[test]
    fn two_controls_at_once_teach_nothing() {
        let before = [1u8, 0x00, 0x00, 0x00];
        let after = [1u8, 0x01, 0x00, 0x01];
        assert_eq!(changed_field(1, &before, &after), None);
    }

    /// Reports of different lengths are different kinds of report, not a
    /// change -- diffing them would name a field that does not exist.
    #[test]
    fn reports_of_different_lengths_teach_nothing() {
        assert_eq!(changed_field(1, &[1u8, 0], &[1u8, 0, 0]), None);
        assert_eq!(changed_field(1, &[], &[]), None);
    }

    /// The offset the editor names has to be the offset the parser reads, or
    /// a learned mapping points one byte away from the control it learned.
    #[test]
    fn a_learned_field_reads_the_control_it_was_learned_from() {
        for report in [0u8, 1] {
            let skip = usize::from(report != 0);
            let mut before = vec![0u8; 4];
            let mut after = vec![0u8; 4];
            if report != 0 {
                before[0] = report;
                after[0] = report;
            }
            // Move byte 2 of the payload.
            before[skip + 2] = 0x10;
            after[skip + 2] = 0x70;

            let field = changed_field(report, &before, &after)
                .unwrap_or_else(|| panic!("report {report}: nothing learned"));
            assert_eq!(
                field.read(&after),
                Some(0x70),
                "report {report}: the learned field reads the wrong byte"
            );
            // And it round-trips through the text a mapping file holds.
            let parsed = crate::mapping::Trigger::parse(&field.describe())
                .expect("the learned field parses")
                .field()
                .expect("it is a HID field");
            assert_eq!(parsed, field);
        }
    }

    /// Anything the editor writes, the parser reads -- the same invariant the
    /// MIDI side keeps, checked here across every width.
    #[test]
    fn every_width_describes_itself_the_way_it_is_written() {
        for (field, text) in [
            (Field::new(1, 3, Width::Bit(2)), "hid 1 bit 3.2"),
            (Field::new(1, 5, Width::Byte), "hid 1 byte 5"),
            (Field::new(2, 6, Width::Word), "hid 2 word 6"),
            (Field::new(2, 6, Width::WordLe), "hid 2 word-le 6"),
        ] {
            assert_eq!(field.describe(), text);
        }
    }
}

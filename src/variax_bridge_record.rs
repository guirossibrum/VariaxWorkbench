//! Offline inspection of two observed standalone Windows x86 bridge buffers.
//! This is not an IOCTL builder, driver adapter, USB frame or instrument codec.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    UnsupportedControl,
    ShortRecord,
    PayloadLength,
}

/// A borrowed, unchanged snapshot. No request is created or sent.
#[derive(Clone, Copy, Debug)]
pub struct ControlRecord<'a> {
    bytes: &'a [u8],
    control: u32,
}

impl<'a> ControlRecord<'a> {
    /// Strict input policy, not a reconstruction of every native error branch.
    /// Based only on standalone L6SM.sys b109c860… Windows x86 handlers.
    pub fn inspect(control: u32, bytes: &'a [u8]) -> Result<Self, RecordError> {
        if !matches!(control, 0x0022_200c | 0x0022_2010) {
            return Err(RecordError::UnsupportedControl);
        }
        if bytes.len() < 16 {
            return Err(RecordError::ShortRecord);
        }
        if bytes[2] > 8 {
            return Err(RecordError::PayloadLength);
        }
        Ok(Self { bytes, control })
    }

    pub fn control(&self) -> u32 {
        self.control
    }

    /// Neutral source word: not established as guitar/model/product identity.
    pub fn raw_word(&self) -> u16 {
        u16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }

    pub fn payload(&self) -> &'a [u8] {
        &self.bytes[3..3 + usize::from(self.bytes[2])]
    }

    /// Includes uninterpreted slack and any supplied extension bytes unchanged.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_source_controls_and_all_observed_length_boundaries() {
        for control in [0x22200c, 0x222010] {
            for len in 0..=8 {
                let mut b = [0xa5; 16];
                b[0] = 0x34;
                b[1] = 0x12;
                b[2] = len;
                let r = ControlRecord::inspect(control, &b).unwrap();
                assert_eq!(r.control(), control);
                assert_eq!(r.raw_word(), 0x1234);
                assert_eq!(r.payload(), &b[3..3 + usize::from(len)]);
                assert_eq!(r.as_bytes(), b);
            }
        }
    }

    #[test]
    fn every_short_record_is_rejected_before_field_access() {
        for n in 0..16 {
            assert_eq!(
                ControlRecord::inspect(0x22200c, &vec![0; n]).unwrap_err(),
                RecordError::ShortRecord
            );
        }
    }

    #[test]
    fn adjacent_and_extreme_invalid_payload_lengths_are_rejected() {
        for len in 9..=255 {
            let mut b = [0; 16];
            b[2] = len;
            assert_eq!(
                ControlRecord::inspect(0x222010, &b).unwrap_err(),
                RecordError::PayloadLength
            );
        }
    }

    #[test]
    fn unexamined_codes_and_wrong_method_bits_are_not_admitted() {
        for code in [0, 0x222008, 0x22200d, 0x222011, 0x222014, u32::MAX] {
            assert_eq!(
                ControlRecord::inspect(code, &[0; 16]).unwrap_err(),
                RecordError::UnsupportedControl
            );
        }
    }

    #[test]
    fn opaque_slack_extensions_and_input_are_preserved() {
        let mut b: Vec<u8> = (0..=255).collect();
        b[2] = 8;
        let original = b.clone();
        let r = ControlRecord::inspect(0x22200c, &b).unwrap();
        assert_eq!(r.as_bytes(), original);
        assert_eq!(r.as_bytes().as_ptr(), b.as_ptr());
        assert_eq!(&r.as_bytes()[11..], &original[11..]);
        assert_eq!(b, original);
    }

    #[test]
    fn raw_word_is_not_restricted_to_invented_instrument_ids() {
        for value in [0, 0x534d, 0x4159, 0xffff] {
            let mut b = [0; 16];
            b[..2].copy_from_slice(&u16::to_le_bytes(value));
            assert_eq!(
                ControlRecord::inspect(0x222010, &b).unwrap().raw_word(),
                value
            );
        }
    }
}

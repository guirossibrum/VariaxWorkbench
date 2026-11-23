//! Universal MIDI identity request and reply.

pub const REQUEST: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];
pub const LINE6: [u8; 3] = [0x00, 0x01, 0x0c];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub manufacturer: [u8; 3],
    pub family: u16,
    pub member: u16,
    pub version: [u8; 4],
}

impl Identity {
    /// A complete 17-byte identity reply, or `None`.
    pub fn parse(message: &[u8]) -> Option<Self> {
        if message.len() != 17
            || message[..5] != [0xf0, 0x7e, 0x7f, 0x06, 0x02]
            || message[16] != 0xf7
        {
            return None;
        }
        Some(Self {
            manufacturer: [message[5], message[6], message[7]],
            family: u16::from_le_bytes([message[8], message[9]]),
            member: u16::from_le_bytes([message[10], message[11]]),
            version: [message[12], message[13], message[14], message[15]],
        })
    }

    pub fn is_line6(&self) -> bool {
        self.manufacturer == LINE6
    }

    /// The Variax sends ASCII digits; the PODxt sends digit values (0..=9).
    pub fn version_text(&self) -> String {
        let digits: String = self
            .version
            .iter()
            .map(|b| {
                if b.is_ascii_digit() {
                    char::from(*b)
                } else {
                    char::from(b'0' + (b % 10))
                }
            })
            .collect();
        let major = digits[..2].trim_start_matches('0');
        format!(
            "{}.{}",
            if major.is_empty() { "0" } else { major },
            &digits[2..]
        )
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // Synthetic replies with the published family/member ids and made-up
    // versions. Real captured frames stay local and untracked.
    pub const VARIAX_600: [u8; 17] = [
        0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x04, 0x00, 0x31, 0x32, 0x30,
        0x33, 0xf7,
    ];
    pub const PODXT_LIVE: [u8; 17] = [
        0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x03, 0x00, 0x0a, 0x00, 0x00, 0x01, 0x02,
        0x07, 0xf7,
    ];

    #[test]
    fn both_device_replies_parse() {
        let variax = Identity::parse(&VARIAX_600).unwrap();
        assert_eq!(
            (variax.family, variax.member, variax.version_text()),
            (7, 4, "12.03".into())
        );
        let podxt = Identity::parse(&PODXT_LIVE).unwrap();
        assert_eq!(
            (podxt.family, podxt.member, podxt.version_text()),
            (3, 0x0a, "1.27".into())
        );
        assert!(variax.is_line6() && podxt.is_line6());
    }

    #[test]
    fn partial_replies_do_not_parse() {
        assert!(Identity::parse(&VARIAX_600[..16]).is_none());
        assert!(Identity::parse(&[0xf7]).is_none());
    }
}

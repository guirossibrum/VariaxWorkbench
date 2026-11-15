//! MIDI framing and identity parsing, shared by USB transport and offline tools.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub family: u16,
    pub model: u16,
    pub revision: [u8; 4],
    pub version: Option<u32>,
}

impl Identity {
    pub fn parse(packet: &[u8]) -> Result<Self, String> {
        if packet.len() != 17
            || packet[..2] != [0xf0, 0x7e]
            || packet[3..8] != [6, 2, 0, 1, 12]
            || packet[16] != 0xf7
        {
            return Err("expected a complete Line 6 identity reply".into());
        }
        let revision: [u8; 4] = packet[12..16].try_into().unwrap();
        let digits = if revision.iter().all(u8::is_ascii_digit) {
            Some(revision.map(|b| b - b'0'))
        } else if revision.iter().all(|&b| b <= 9) {
            Some(revision)
        } else {
            None
        };
        let version = digits.map(|d| {
            ((d[0] as u32 * 10 + d[1] as u32) << 24) | ((d[2] as u32) << 20) | ((d[3] as u32) << 16)
        });
        Ok(Self {
            family: u16::from_le_bytes([packet[8], packet[9]]),
            model: u16::from_le_bytes([packet[10], packet[11]]),
            revision,
            version,
        })
    }
    pub fn product_id(&self) -> u32 {
        (self.family as u32) << 16 | self.model as u32
    }
    pub fn version_string(&self) -> String {
        self.version
            .map(|v| {
                format!(
                    "{}.{:02}",
                    v >> 24,
                    ((v >> 20) & 15) * 10 + ((v >> 16) & 15)
                )
            })
            .unwrap_or_else(|| format!("{:02x?}", self.revision))
    }
}

#[derive(Default)]
pub struct SysexDecoder {
    pending: Vec<u8>,
}
impl SysexDecoder {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut messages = Vec::new();
        for &byte in bytes {
            if byte >= 0xf8 {
                continue;
            }
            match byte {
                0xf0 => {
                    self.pending.clear();
                    self.pending.push(byte);
                }
                0xf7 if !self.pending.is_empty() => {
                    self.pending.push(byte);
                    messages.push(std::mem::take(&mut self.pending));
                }
                0x80..=0xf7 => self.pending.clear(),
                _ if !self.pending.is_empty() => self.pending.push(byte),
                _ => {}
            }
            if self.pending.len() > 8192 {
                self.pending.clear();
                return Err("SysEx message exceeds the supported size".into());
            }
        }
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_observed_variax_ascii_and_pod_numeric_revisions() {
        let guitar = [
            0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
        ];
        let id = Identity::parse(&guitar).unwrap();
        assert_eq!(id.product_id(), 0x70004);
        assert_eq!(id.version, Some(0x03100000));
        assert_eq!(id.version_string(), "3.10");
        let pod = [
            0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 3, 0, 10, 0, 0, 3, 0, 1, 0xf7,
        ];
        assert_eq!(Identity::parse(&pod).unwrap().version_string(), "3.01");
        assert!(Identity::parse(&pod[..16]).is_err());
    }
    #[test]
    fn reassembles_fragments_realtime_and_multiple_messages() {
        let mut decoder = SysexDecoder::default();
        assert!(
            decoder
                .feed(&[0xb0, 7, 1, 0xf0, 0, 0xf8, 1])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            decoder.feed(&[12, 0xf7, 0xf0, 7, 0xf7]).unwrap(),
            [vec![0xf0, 0, 1, 12, 0xf7], vec![0xf0, 7, 0xf7]]
        );
        decoder.feed(&[0xf0, 1, 2, 0xb0]).unwrap();
        assert!(decoder.feed(&[3, 4, 0xf7]).unwrap().is_empty());
    }
}

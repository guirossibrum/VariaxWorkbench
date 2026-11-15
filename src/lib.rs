//! Recovered Workbench 1.75 protocol. Hardware behavior is not yet validated.
//!
//! Sources: L6VariaxDevice, L6VariaxPatchManager, L6VariaxMIDITranscoder.
//! This crate currently has no hardware transport or persistent-write commands.

use std::collections::BTreeMap;

pub mod body;
pub mod bundle;
pub mod document;
pub mod dsp;
pub mod firmware;
pub mod manual_check;
pub mod midi;
pub mod models;
pub mod pickup;
pub mod preset;
pub mod provenance;
pub mod read_session;
#[cfg(feature = "usb")]
pub mod usb;

pub const HEADER: [u8; 6] = [0xf0, 0, 1, 0x0c, 7, 0];
pub const PODXT_ROUTING_ENABLE: [u8; 8] = [0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];

/// Six low-to-high nibbles, as verified against the original Java encoder.
pub fn encode_word(value: u32) -> Result<[u8; 6], String> {
    if value > 0xffffff {
        return Err("word exceeds 24 bits".into());
    }
    Ok(std::array::from_fn(|i| ((value >> (4 * i)) & 15) as u8))
}

pub fn decode_word(data: &[u8]) -> Result<u32, String> {
    if data.len() != 6 || data.iter().any(|&n| n > 15) {
        return Err("word must contain exactly six nibbles".into());
    }
    Ok(data
        .iter()
        .enumerate()
        .fold(0, |v, (i, &n)| v | ((n as u32) << (4 * i))))
}

fn message(payload: &[u8]) -> Vec<u8> {
    let mut result = HEADER.to_vec();
    result.extend_from_slice(payload);
    result.push(0xf7);
    result
}

pub fn midi_enable() -> Vec<u8> {
    message(&[0x2a, 1])
}

/// Read current-preset metadata only, not a full DSP patch backup.
pub fn metadata_requests() -> [Vec<u8>; 2] {
    [851, 883].map(|address| read_words_request(address, 32).expect("constant range"))
}

/// Read within the 1,092-word current DSP edit buffer. This request never
/// addresses firmware or persistent preset memory.
pub fn read_words_request(address: u32, count: u32) -> Result<Vec<u8>, String> {
    if count == 0 || count > 32 || address.checked_add(count).is_none_or(|end| end > 1092) {
        return Err("read must fit within the current DSP buffer, in blocks of 1–32 words".into());
    }
    let mut payload = vec![0x3c];
    payload.extend_from_slice(&encode_word(count)?);
    payload.extend_from_slice(&encode_word(address)?);
    Ok(message(&payload))
}

pub fn decode_words_reply(packet: &[u8], count: usize) -> Result<Vec<u8>, String> {
    if !(1..=32).contains(&count) || packet.len() != count * 6 + 8 {
        return Err("unexpected DSP data reply length".into());
    }
    if packet[..5] != HEADER[..5]
        || !matches!(packet[5], 0 | 4)
        || packet[6] != 0x3c
        || packet[packet.len() - 1] != 0xf7
    {
        return Err("unexpected DSP data reply framing or opcode".into());
    }
    let mut output = Vec::with_capacity(count * 3);
    for nibbles in packet[7..packet.len() - 1].as_chunks::<6>().0 {
        output.extend_from_slice(&decode_word(nibbles)?.to_be_bytes()[1..]);
    }
    Ok(output)
}

/// Parse one complete channel-zero metadata reply. MIDI fragmentation must be
/// reassembled before calling this function. Other messages must be dispatched
/// separately; invalid packets are never accepted as metadata.
pub fn decode_metadata_reply(packet: &[u8]) -> Result<[u8; 96], String> {
    // Replies identify the guitar model: 0 = original 500/700, 4 = 300/600.
    // Outgoing requests use 0 even when a Variax 600 responds with 4.
    Ok(decode_words_reply(packet, 32)?
        .try_into()
        .expect("32 words = 96 bytes"))
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Integer(i32),
    Boolean(bool),
    Float(f32),
}

/// Names use Workbench's internal string numbering, which runs opposite to the
/// labels displayed in the original UI. Raw metadata is retained losslessly.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetMetadata {
    pub name: String,
    pub parameters: BTreeMap<&'static str, ParameterValue>,
    pub raw: [u8; 192],
}

impl PresetMetadata {
    /// Replies must be supplied in request order; the recovered reply format
    /// does not carry an address with which to detect swapped blocks.
    pub fn from_replies(first: &[u8], second: &[u8]) -> Result<Self, String> {
        let mut raw = [0; 192];
        raw[..96].copy_from_slice(&decode_metadata_reply(first)?);
        raw[96..].copy_from_slice(&decode_metadata_reply(second)?);
        Self::from_bytes(raw)
    }

    pub fn from_bytes(raw: [u8; 192]) -> Result<Self, String> {
        if !raw[..18].is_ascii() {
            return Err("preset name contains non-ASCII bytes".into());
        }
        let name = String::from_utf8(raw[..18].to_vec())
            .expect("validated ASCII")
            .trim_end_matches(['\0', ' '])
            .to_owned();
        let mut parameters = BTreeMap::new();
        let mut cursor = 18;
        // Field order and types come from DecodePatch, not native DSP layout.
        let mut read = |key, width: usize, kind| {
            let bytes = &raw[cursor..cursor + width];
            cursor += width;
            let value = match kind {
                'i' => ParameterValue::Integer(i32::from_be_bytes(bytes.try_into().unwrap())),
                'b' => ParameterValue::Boolean(bytes[0] != 0),
                'u' => ParameterValue::Integer(bytes[0] as i32),
                's' => ParameterValue::Integer(bytes[0] as i8 as i32),
                'f' => ParameterValue::Float(f32::from_bits(u32::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], 0,
                ]))),
                _ => unreachable!(),
            };
            parameters.insert(key, value);
        };
        read("body_model", 4, 'i');
        for (enabled, model, position, angle, level) in [
            (
                "pickup1_enabled",
                "pickup1_model",
                "pickup1_position",
                "pickup1_angle",
                "pickup1_level",
            ),
            (
                "pickup2_enabled",
                "pickup2_model",
                "pickup2_position",
                "pickup2_angle",
                "pickup2_level",
            ),
        ] {
            read(enabled, 1, 'b');
            read(model, 4, 'i');
            read(position, 3, 'f');
            read(angle, 3, 'f');
            read(level, 3, 'f');
        }
        read("pickup2_phase_reverse", 1, 'b');
        for key in ["tone_capacitance", "tone_resistance", "volume_resistance"] {
            read(key, 3, 'f');
        }
        read("volume_taper", 1, 'u');
        read("tone_position", 3, 'f');
        read("save_tone_position", 1, 'b');
        read("trim", 3, 'f');
        read("alternate_tuning_enabled", 1, 'b');
        for key in [
            "string1_tuning",
            "string2_tuning",
            "string3_tuning",
            "string4_tuning",
            "string5_tuning",
            "string6_tuning",
        ] {
            read(key, 1, 's');
        }
        for key in [
            "string1_detune",
            "string2_detune",
            "string3_detune",
            "string4_detune",
            "string5_detune",
            "string6_detune",
            "string1_mix",
            "string2_mix",
            "string3_mix",
            "string4_mix",
            "string5_mix",
            "string6_mix",
            "string_spread",
        ] {
            read(key, 3, 'f');
        }
        read("pickups_in_series", 1, 'b');
        Ok(Self {
            name,
            parameters,
            raw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_original_java_encoder() {
        for line in include_str!("../tests/fixtures/original-java-word-encoding.txt").lines() {
            let (value, hex) = line.split_once(' ').unwrap();
            let expected: Vec<u8> = (0..12)
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(
                encode_word(value.parse().unwrap()).unwrap().as_slice(),
                expected
            );
        }
        assert!(encode_word(0x1000000).is_err());
    }

    #[test]
    fn rejects_malformed_replies() {
        let mut reply = message(&[0; 193]);
        reply[6] = 0x3c;
        assert!(decode_metadata_reply(&reply).is_ok());
        reply[7] = 16;
        assert!(decode_metadata_reply(&reply).is_err());
        reply[7] = 0;
        reply[6] = 0x61; // A write-related opcode must never parse as metadata.
        assert!(decode_metadata_reply(&reply).is_err());
        reply[6] = 0x3c;
        reply[199] = 0;
        assert!(decode_metadata_reply(&reply).is_err());
        assert!(decode_metadata_reply(&reply[..199]).is_err());
        assert!(decode_word(&[0, 0]).is_err());
    }

    #[test]
    fn decodes_nibble_order_without_byte_reversal() {
        let mut reply = message(&[0; 193]);
        reply[6] = 0x3c;
        reply[7..13].copy_from_slice(&[6, 5, 4, 3, 2, 1]);
        assert_eq!(
            &decode_metadata_reply(&reply).unwrap()[..3],
            &[0x12, 0x34, 0x56]
        );
    }

    #[test]
    fn decodes_metadata_types_and_preserves_padding() {
        let mut raw = [0; 192];
        raw[..4].copy_from_slice(b"Test");
        raw[18..22].copy_from_slice(&0x03100003_i32.to_be_bytes());
        raw[22] = 1;
        raw[27..30].copy_from_slice(&[0x40, 0x20, 0]); // 2.5, truncated IEEE-754
        raw[69] = 0xf4; // First internal string tuning: -12 semitones.
        raw[191] = 0xa5;
        let preset = PresetMetadata::from_bytes(raw).unwrap();
        assert_eq!(preset.name, "Test");
        assert_eq!(
            preset.parameters["body_model"],
            ParameterValue::Integer(0x03100003)
        );
        assert_eq!(
            preset.parameters["pickup1_enabled"],
            ParameterValue::Boolean(true)
        );
        assert_eq!(
            preset.parameters["pickup1_position"],
            ParameterValue::Float(2.5)
        );
        assert_eq!(
            preset.parameters["string1_tuning"],
            ParameterValue::Integer(-12)
        );
        assert_eq!(preset.raw[191], 0xa5);
    }
}

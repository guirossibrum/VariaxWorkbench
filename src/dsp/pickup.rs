//! Native pickup block assembly into the logical offline patch.
use super::{NativePatch, TableKind};
use crate::pickup::{CalculatedResponse, GeometryProjection};

#[derive(Clone, Copy, Debug)]
pub struct PickupResponseControls {
    pub enabled: bool,
    pub tone_position: f32,
    pub save_tone: bool,
    pub tone_function: u16,
    pub primary_gain_scalar: f32,
}
impl NativePatch {
    pub fn write_pickup_response(
        &mut self,
        response: &CalculatedResponse,
        controls: PickupResponseControls,
    ) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(0x100, controls.enabled);
        next.set_table(TableKind::PickupLowpass, response.lowpass_coefficient)?;
        next.set_table(TableKind::Tone2Gain, response.secondary_gain)?;
        next.set_table(TableKind::Tone2Q, response.secondary_q)?;
        next.set_pickup_secondary_frequency_coefficient(response.secondary_frequency_coefficient);
        next.set_table(TableKind::Tone1Gain, response.tone_gain)?;
        next.set_table(
            TableKind::ToneOffGain,
            response
                .primary_gain
                .map(|v| controls.primary_gain_scalar * v),
        )?;
        next.set_table(TableKind::ToneOffQ, response.primary_q)?;
        next.set_table(
            TableKind::ToneOffFrequency,
            response.primary_frequency_coefficient,
        )?;
        next.set_tone_position(controls.tone_position, controls.save_tone)?;
        next.set_tone_function(controls.tone_function);
        *self = next;
        Ok(())
    }
    pub fn write_pickup_geometry(&mut self, geometry: &GeometryProjection) -> Result<(), String> {
        let mut next = self.clone();
        if !geometry.scale_length.is_finite() || geometry.scale_length <= 0.0 {
            return Err("invalid pickup scale length".into());
        }
        next.set_block_enabled(0x20, geometry.enabled);
        next.set_pickup_scale(geometry.scale_length);
        next.set_pickup_num_coils(geometry.num_coils)?;
        for (i, v) in geometry.magnetic_aperture.iter().enumerate() {
            next.set_pickup_aperture(i, *v)?;
        }
        for (i, v) in geometry.coil_gains.iter().enumerate() {
            next.set_pickup_coil_gain(i, *v)?;
        }
        for (i, (v, phase)) in geometry.poles.iter().enumerate() {
            next.set_pickup_pole(i, *v, *phase)?;
        }
        *self = next;
        Ok(())
    }
}

/// Analysis-only 360-byte snapshot of the native response object's fields.
/// No vtable/pointer is used or followed; this is not a vendor preset format.
pub fn response_from_analysis(
    bytes: &[u8],
) -> Result<(CalculatedResponse, PickupResponseControls), String> {
    if bytes.len() != 0x168 || bytes[4] > 1 || bytes[0x10] > 1 {
        return Err("expected canonical 360-byte response analysis snapshot".into());
    }
    let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let value = |o| f32::from_bits(word(o));
    let array = |o| std::array::from_fn(|i| value(o + i * 4));
    Ok((
        CalculatedResponse {
            lowpass_frequency: array(0x18),
            secondary_gain: array(0x3c),
            secondary_q: array(0x60),
            secondary_frequency: value(0x84),
            tone_gain: array(0x88),
            primary_gain: array(0xac),
            primary_q: array(0xd0),
            primary_frequency: array(0xf4),
            lowpass_coefficient: array(0x11c),
            secondary_frequency_coefficient: value(0x140),
            primary_frequency_coefficient: array(0x144),
        },
        PickupResponseControls {
            enabled: bytes[4] != 0,
            tone_position: value(0xc),
            save_tone: bytes[0x10] != 0,
            tone_function: word(0x14) as u16,
            primary_gain_scalar: value(0x118),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pickup::GeometryInput;
    fn decode_hex(value: &str) -> Vec<u8> {
        (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn complete_pickup_blocks_match_all_native_words() {
        for line in include_str!("../../tests/fixtures/native-dsp-pickup-blocks.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let fields: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(fields[1], 16).unwrap();
            let mut patch = NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            let mut expected = patch.words;
            for changed in output.split_whitespace() {
                let (i, v) = changed.split_once(':').unwrap();
                expected[usize::from_str_radix(i, 16).unwrap()] =
                    u32::from_str_radix(v, 16).unwrap();
            }
            let bytes = decode_hex(fields[2]);
            if fields[0] == "geometry" {
                patch
                    .write_pickup_geometry(
                        &GeometryInput::parse(&bytes).unwrap().project().unwrap(),
                    )
                    .unwrap();
            } else {
                let (response, controls) = response_from_analysis(&bytes).unwrap();
                patch.write_pickup_response(&response, controls).unwrap();
            }
            assert_eq!(patch.words, expected, "{} {}", fields[0], fields[1]);
        }
    }
    #[test]
    fn block_errors_preserve_the_previous_patch() {
        let first = include_str!("../../tests/fixtures/native-dsp-pickup-blocks.txt")
            .lines()
            .next()
            .unwrap();
        let fields: Vec<_> = first.split_whitespace().collect();
        let bytes = decode_hex(fields[2]);
        let mut geometry = GeometryInput::parse(&bytes).unwrap().project().unwrap();
        geometry.coil_gains[3] = f32::NAN;
        let mut patch = NativePatch::default();
        patch.words.fill(0x98765432);
        let before = patch.clone();
        assert!(patch.write_pickup_geometry(&geometry).is_err());
        assert_eq!(patch, before);
        for size in 0..0x168 {
            assert!(response_from_analysis(&vec![0; size]).is_err());
        }
        let (response, mut controls) = response_from_analysis(&[0; 0x168]).unwrap();
        controls.tone_position = 2.0;
        assert!(patch.write_pickup_response(&response, controls).is_err());
        assert_eq!(patch, before);
    }
}

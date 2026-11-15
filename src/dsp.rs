//! Native logical-word packing and selected conversions, not a complete engine.
//!
//! Matches L6VariaxHWPatch::Pack (i386 Mac library, address 0x1bd10).
//! Java overlays metadata and recomputes its checksum later. Therefore this
//! intermediate buffer is not, by itself, a finished uploadable guitar patch.

mod coefficients;
mod variant;
pub use variant::RenderOptions;
mod resonance;
pub use resonance::write_resonance_analysis;
mod equalizer;
pub use equalizer::{EqParameters, equalizer_coefficients, write_eq_analysis};
mod filters;
pub use filters::{biquad_coefficients, write_filter_analysis};
mod patch;
mod pickup;
mod simple;
mod tables;
pub use patch::{NativePatch, vx_float_words};
pub use pickup::{PickupResponseControls, response_from_analysis};
pub use simple::write_simple_analysis;
pub use tables::{TableKind, signed_q23};

/// Default stored by the original host coefficient-engine constructors. This
/// is not a claim about POD USB recording rates or measured hardware clocks.
pub const LEGACY_SAMPLE_RATE: f32 = 39062.5;

pub const DATA_WORDS: usize = 1091;
pub const PACKED_BYTES: usize = 3276;

/// Native body-resonator conversion, including its asymmetric rounding and
/// stored word order. Inputs are mathematical coefficients before the native
/// 0.5 scaling; output words may retain a carry bit until packing masks them.
pub fn body_resonator_words(coefficients: [f32; 3]) -> Result<[u32; 3], String> {
    if coefficients.iter().any(|value| !value.is_finite()) {
        return Err("resonator coefficients must be finite".into());
    }
    Ok([
        signed_q23(coefficients[0] * 0.5)?,
        signed_q23(coefficients[2] * 0.5)?,
        signed_q23(coefficients[1] * 0.5)?,
    ])
}

/// Pack the low 24 bits of each native word and append their modulo-2^24 sum.
pub fn pack_native_words(words: &[u32; DATA_WORDS]) -> [u8; PACKED_BYTES] {
    let mut output = [0; PACKED_BYTES];
    let checksum = words
        .iter()
        .fold(0u32, |sum, &word| sum.wrapping_add(word) & 0xffffff);
    for (i, word) in words.iter().copied().chain([checksum]).enumerate() {
        output[i * 3..i * 3 + 3].copy_from_slice(&word.to_be_bytes()[1..]);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_original_machine_code_on_synthetic_words() {
        // Produced by verify_native_pack.py using the original routine in an
        // isolated i386 CPU emulator; input includes overflow/high-bit cases.
        let input = include_bytes!("../tests/fixtures/native-pack-input.bin");
        let words = std::array::from_fn(|i| {
            u32::from_le_bytes(input[i * 4..i * 4 + 4].try_into().unwrap())
        });
        let expected = include_bytes!("../tests/fixtures/native-pack-output.bin");
        assert_eq!(&pack_native_words(&words), expected);
    }

    #[test]
    fn checksum_is_an_additive_24_bit_sum() {
        let mut words = [0; DATA_WORDS];
        assert_eq!(pack_native_words(&words), [0; PACKED_BYTES]);
        words[0] = 0xffffff;
        words[1] = 2;
        let packed = pack_native_words(&words);
        assert_eq!(&packed[..6], &[255, 255, 255, 0, 0, 2]);
        assert_eq!(&packed[PACKED_BYTES - 3..], &[0, 0, 1]);
    }

    #[test]
    fn resonator_conversion_matches_original_machine_code() {
        for line in include_str!("../tests/fixtures/native-resonator-vectors.txt").lines() {
            let values: Vec<u32> = line
                .split_whitespace()
                .map(|s| u32::from_str_radix(s, 16).unwrap())
                .collect();
            let input = [
                f32::from_bits(values[0]),
                f32::from_bits(values[1]),
                f32::from_bits(values[2]),
            ];
            assert_eq!(body_resonator_words(input).unwrap(), values[3..6]);
        }
        assert!(body_resonator_words([f32::NAN, 0.0, 0.0]).is_err());
        assert!(body_resonator_words([0.0, f32::INFINITY, 0.0]).is_err());
    }
}

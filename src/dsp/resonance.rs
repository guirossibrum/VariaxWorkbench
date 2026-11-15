//! Resonant block parameter-to-word conversions, without an audio renderer.
use super::NativePatch;
use crate::body::{BodyResonance, SitarBuzz, StringTone, StringToneString};
use crate::math::EngineFloat as _;
impl NativePatch {
    pub fn write_body_resonance(&mut self, block: &BodyResonance, rate: f32) -> Result<(), String> {
        if !rate.is_finite()
            || !(1.0..=384000.0).contains(&rate)
            || [
                block.master,
                block.body_mix,
                block.mid_mix,
                block.treble_mix,
                block.treble_frequency,
                block.rolloff_frequency,
            ]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err("unsupported body-resonance parameters".into());
        }
        let master = 10.0_f32.engine_powf(block.master / 20.0);
        let mut next = self.clone();
        next.set_block_enabled(0x200, block.enabled);
        let mut count = 0;
        for i in 0..12 {
            if !block.resonator_enabled[i] {
                continue;
            }
            let (frequency, q, gain) = (block.frequencies[i], block.q[i], block.gain[i]);
            if !frequency.is_finite() || !q.is_finite() || q <= 0.0 || !gain.is_finite() {
                return Err("unsupported body resonator".into());
            }
            let k = ((f64::from(frequency) * std::f64::consts::PI) / f64::from(rate)).engine_tan();
            let square = k * k;
            let damping = k / f64::from(q);
            let inverse = 1.0 / (square + damping + 1.0);
            let b2 = (-damping * inverse * 10.0_f64.engine_powf(f64::from(gain) / 20.0)) as f32;
            let a1 = (inverse * ((1.0 - square) + (1.0 - square))) as f32;
            let a2 = (((damping - 1.0) - square) * inverse) as f32;
            next.apply_block_coefficients(
                "body-resonator",
                &[
                    count,
                    (b2 * master).to_bits(),
                    (-a1).to_bits(),
                    (-a2).to_bits(),
                ],
            )?;
            count += 1;
        }
        next.set_body_resonator_count(count)?;
        for i in count..12 {
            next.apply_block_coefficients("body-resonator", &[i, 0, 0, 0])?;
        }
        let treble = -((block.treble_frequency / rate) * -std::f32::consts::TAU).engine_exp();
        let roll = ((block.rolloff_frequency / rate) * -std::f32::consts::TAU).engine_exp();
        next.apply_block_coefficients(
            "body-treble",
            &[
                block.treble_mix.to_bits(),
                treble.to_bits(),
                (1.0 - roll).to_bits(),
                (-roll).to_bits(),
            ],
        )?;
        next.apply_block_coefficients(
            "body-mix",
            &[block.body_mix.to_bits(), block.mid_mix.to_bits()],
        )?;
        *self = next;
        Ok(())
    }
    pub fn write_string_tone(&mut self, block: &StringTone, rate: f32) -> Result<(), String> {
        if !rate.is_finite() || !(1.0..=384000.0).contains(&rate) {
            return Err("unsupported string-tone sample rate".into());
        }
        let mut next = self.clone();
        next.set_block_enabled(8, block.enabled);
        for (string, input) in block.strings.iter().enumerate() {
            if [input.master, input.hold_off, input.decay]
                .iter()
                .any(|v| !v.is_finite())
            {
                return Err("nonfinite string-tone parameters".into());
            }
            let master = 10.0_f32.engine_powf(input.master / 20.0);
            let hold = (-1.0 / f64::from(input.hold_off * rate)).engine_exp() as f32;
            let decay = (-1.0 / f64::from(input.decay * rate)).engine_exp() as f32;
            next.apply_block_coefficients(
                "string-tone",
                &[
                    string as u32,
                    6,
                    hold.clamp(-1.0, 1.0).to_bits(),
                    decay.clamp(-1.0, 1.0).to_bits(),
                ],
            )?;
            for resonator in 0..6 {
                if !input.resonator_enabled[resonator] {
                    next.apply_block_coefficients(
                        "string-tone-resonator",
                        &[string as u32, resonator as u32, 0, 0, 0],
                    )?;
                    continue;
                }
                let (frequency, q, gain) = (
                    input.frequencies[resonator],
                    input.q[resonator],
                    input.gain[resonator],
                );
                if !frequency.is_finite() || !q.is_finite() || q <= 0.0 || !gain.is_finite() {
                    return Err("unsupported string resonator".into());
                }
                let k = ((frequency * std::f32::consts::PI) / rate).engine_tan();
                let square = k * k;
                let damping = k / q;
                let inverse = 1.0 / (square + damping + 1.0);
                let b2 = 10.0_f32.engine_powf(gain / 20.0) * (-damping * inverse);
                let a2 = ((damping - 1.0) - square) * inverse;
                let a1 = inverse * ((1.0 - square) + (1.0 - square));
                next.apply_block_coefficients(
                    "string-tone-resonator",
                    &[
                        string as u32,
                        resonator as u32,
                        (-a2).to_bits(),
                        (-a1).to_bits(),
                        (b2 * master).to_bits(),
                    ],
                )?;
            }
        }
        *self = next;
        Ok(())
    }
    pub fn write_sitar(&mut self, block: &SitarBuzz, rate: f32) -> Result<(), String> {
        self.write_sitar_inner(block, rate, false)
    }
    // Original logarithmic interpolation of zero frequencies at an endpoint can
    // yield NaN even with finite source definitions. Only the internal host
    // path accepts those derived frequencies; public numeric inputs stay strict.
    pub(crate) fn write_interpolated_sitar(
        &mut self,
        block: &SitarBuzz,
        rate: f32,
    ) -> Result<(), String> {
        self.write_sitar_inner(block, rate, true)
    }
    fn write_sitar_inner(
        &mut self,
        block: &SitarBuzz,
        rate: f32,
        derived: bool,
    ) -> Result<(), String> {
        if !rate.is_finite()
            || !(1.0..=384000.0).contains(&rate)
            || (!derived
                && block
                    .start_frequency
                    .iter()
                    .chain(block.stop_frequency.iter())
                    .any(|v| !v.is_finite()))
            || block
                .decay
                .iter()
                .chain(block.q.iter())
                .chain(block.mix.iter())
                .any(|v| !v.is_finite())
        {
            return Err("unsupported sitar parameters".into());
        }
        let mut next = self.clone();
        next.set_block_enabled(0x2000, block.enabled);
        for i in 0..6 {
            let mix = block.mix[i];
            let damping = 0.5 / block.q[i];
            let stop = ((f64::from(block.stop_frequency[i]) * std::f64::consts::TAU)
                / f64::from(rate)) as f32;
            let difference = ((f64::from(block.start_frequency[i] - block.stop_frequency[i])
                * std::f64::consts::TAU)
                / f64::from(rate)) as f32;
            // The original uses double exp after a float32 rate*time product.
            let decay = (-1.0 / f64::from(rate * block.decay[i])).engine_exp() as f32;
            // Direct original-instruction comparisons: NaN conversion retains
            // logical word 0x1000000, masked to zero by Pack. Do not infer the
            // unordered floating-point branch from decompiler comparisons.
            let frequency_word_input = |v: f32| if v.is_nan() { 0.0 } else { v.clamp(-1.0, 1.0) };
            next.apply_block_coefficients(
                "sitar-string",
                &[
                    i as u32,
                    decay.clamp(-1.0, 1.0).to_bits(),
                    frequency_word_input(difference).to_bits(),
                    frequency_word_input(stop).to_bits(),
                    damping.clamp(-1.0, 1.0).to_bits(),
                    mix.to_bits(),
                ],
            )?;
            if difference.is_nan() {
                next.words[0x8c8 / 4 + i] = 0x1000000;
            }
            if stop.is_nan() {
                next.words[0x8e0 / 4 + i] = 0x1000000;
            }
        }
        *self = next;
        Ok(())
    }
}
pub fn write_resonance_analysis(
    patch: &mut NativePatch,
    kind: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let length = match kind {
        "sitar" => 0x84,
        "body" => 0x1ec,
        "string-tone" => 0x6e8,
        _ => return Err("unknown resonant block".into()),
    };
    if bytes.len() != length || bytes[4] > 1 {
        return Err("invalid resonant-block snapshot".into());
    }
    let value = |o| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    if kind == "body" {
        let count = u32::from_le_bytes(bytes[0xc..0x10].try_into().unwrap());
        if count > 12 {
            return Err("body resonator count exceeds twelve".into());
        }
        return patch.write_body_resonance(
            &BodyResonance {
                enabled: bytes[4] != 0,
                resonator_enabled: std::array::from_fn(|i| (i as u32) < count),
                frequencies: std::array::from_fn(|i| value(0x2c + i * 16)),
                q: std::array::from_fn(|i| value(0x30 + i * 16)),
                gain: std::array::from_fn(|i| value(0x34 + i * 16)),
                treble_frequency: value(0x20),
                rolloff_frequency: value(0x1c),
                master: value(0x18),
                body_mix: value(0x10),
                mid_mix: value(0x14),
                treble_mix: value(0x24),
            },
            value(8),
        );
    }
    if kind == "string-tone" {
        if (0..36).any(|i| bytes[0x34 + i * 0x2c] > 1) {
            return Err("noncanonical resonator flag".into());
        }
        let strings = std::array::from_fn(|string| StringToneString {
            master: value(0x658 + string * 4),
            hold_off: value(0x688 + string * 4),
            decay: value(0x6a0 + string * 4),
            resonator_enabled: std::array::from_fn(|i| bytes[0x34 + (string * 6 + i) * 0x2c] != 0),
            frequencies: std::array::from_fn(|i| value(0x28 + (string * 6 + i) * 0x2c)),
            q: std::array::from_fn(|i| value(0x2c + (string * 6 + i) * 0x2c)),
            gain: std::array::from_fn(|i| value(0x30 + (string * 6 + i) * 0x2c)),
        });
        return patch.write_string_tone(
            &StringTone {
                enabled: bytes[4] != 0,
                strings,
            },
            value(8),
        );
    }
    patch.write_sitar(
        &SitarBuzz {
            enabled: bytes[4] != 0,
            decay: std::array::from_fn(|i| value(0xc + i * 4)),
            start_frequency: std::array::from_fn(|i| value(0x24 + i * 4)),
            stop_frequency: std::array::from_fn(|i| value(0x3c + i * 4)),
            q: std::array::from_fn(|i| value(0x54 + i * 4)),
            mix: std::array::from_fn(|i| value(0x6c + i * 4)),
        },
        value(8),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sitar_word_paths_match_native_with_signed_zero_and_zero_q() {
        for line in include_str!("../../tests/fixtures/native-dsp-sitar.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let f: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(f[0], 16).unwrap();
            let bytes: Vec<_> = (0..f[1].len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&f[1][i..i + 2], 16).unwrap())
                .collect();
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
            write_resonance_analysis(&mut patch, "sitar", &bytes).unwrap();
            assert_eq!(patch.words, expected, "{input}");
        }
    }
    #[test]
    fn body_and_string_tone_words_match_original_instructions() {
        let mut count = 0;
        for line in include_str!("../../tests/fixtures/native-dsp-resonance-blocks.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let f: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(f[1], 16).unwrap();
            let bytes: Vec<_> = (0..f[2].len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&f[2][i..i + 2], 16).unwrap())
                .collect();
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
            write_resonance_analysis(&mut patch, f[0], &bytes).unwrap();
            assert_eq!(patch.words, expected, "case {count}: {}", f[0]);
            count += 1;
        }
        assert_eq!(count, 432);
    }
    #[test]
    fn malformed_resonance_snapshots_preserve_the_previous_patch() {
        let mut patch = NativePatch::default();
        patch.words.fill(0x789abc);
        let before = patch.clone();
        for (kind, size) in [("body", 492), ("string-tone", 1768)] {
            for length in [0, size - 1, size + 1] {
                assert!(write_resonance_analysis(&mut patch, kind, &vec![0; length]).is_err());
                assert_eq!(patch, before);
            }
            let mut bytes = vec![0; size];
            bytes[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
            assert!(write_resonance_analysis(&mut patch, kind, &bytes).is_err());
            assert_eq!(patch, before);
        }
    }
    #[test]
    fn derived_sitar_nan_words_match_original_without_accepting_public_nan_inputs() {
        let mut count = 0;
        for line in include_str!("../../tests/fixtures/native-dsp-derived-sitar.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let (seed, encoded) = input.split_once(' ').unwrap();
            let bytes: Vec<_> = (0..encoded.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
                .collect();
            let value = |o| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            let block = SitarBuzz {
                enabled: bytes[4] != 0,
                decay: std::array::from_fn(|i| value(0xc + i * 4)),
                start_frequency: std::array::from_fn(|i| value(0x24 + i * 4)),
                stop_frequency: std::array::from_fn(|i| value(0x3c + i * 4)),
                q: std::array::from_fn(|i| value(0x54 + i * 4)),
                mix: std::array::from_fn(|i| value(0x6c + i * 4)),
            };
            let mut seed = u32::from_str_radix(seed, 16).unwrap();
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
            if block
                .start_frequency
                .iter()
                .chain(block.stop_frequency.iter())
                .any(|v| v.is_nan())
            {
                let before = patch.clone();
                assert!(patch.write_sitar(&block, value(8)).is_err());
                assert_eq!(patch, before);
            }
            patch.write_interpolated_sitar(&block, value(8)).unwrap();
            assert_eq!(patch.words, expected, "derived case {count}");
            count += 1;
        }
        assert_eq!(count, 48);
    }
    #[test]
    fn malformed_sitar_input_cannot_partially_change_words() {
        let mut patch = NativePatch::default();
        patch.words.fill(0x789abc);
        let before = patch.clone();
        for size in 0..0x84 {
            assert!(write_resonance_analysis(&mut patch, "sitar", &vec![0; size]).is_err());
            assert_eq!(patch, before);
        }
        let mut bytes = [0; 0x84];
        bytes[8..12].copy_from_slice(&31250_f32.to_le_bytes());
        bytes[0x6c..0x70].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(write_resonance_analysis(&mut patch, "sitar", &bytes).is_err());
        assert_eq!(patch, before);
    }
}

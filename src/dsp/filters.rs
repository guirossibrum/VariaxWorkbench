//! Native generic biquad helper and block parameter preparation.
use super::{NativePatch, TableKind};
use crate::body::{Compressor, LowCut, PickFilter, Reverb};

/// Original helper coefficient order: a1, a2, b0, b1, b2. Only modes 4..7
/// have formulas in the recovered helper; every other mode returns identity.
pub fn biquad_coefficients(
    kind: u32,
    frequency: f32,
    q: f32,
    rate: f32,
) -> Result<[f32; 5], String> {
    if !rate.is_finite()
        || !(1.0..=384000.0).contains(&rate)
        || !frequency.is_finite()
        || ((kind == 6 || kind == 7) && (!q.is_finite() || q <= 0.0))
    {
        return Err("unsupported biquad parameters".into());
    }
    let fraction = frequency / rate;
    let omega = (fraction + fraction) * std::f32::consts::PI;
    Ok(match kind {
        4 | 5 => {
            let pole = (-omega).exp();
            [if kind == 4 { pole } else { -pole }, 0., 1. - pole, 0., 0.]
        }
        6 | 7 => {
            let k = (omega * 0.5).tan();
            let square = k * k;
            let inverse = 1.0 / (k / q + square + 1.0);
            let a1 = (1.0 - square) * -2.0 * inverse;
            let a2 = -((k / q - square) - 1.0) * inverse;
            if kind == 6 {
                [
                    a1,
                    a2,
                    square * inverse,
                    inverse * (square + square),
                    square * inverse,
                ]
            } else {
                [a1, a2, inverse, inverse * -2.0, inverse]
            }
        }
        _ => [0., 0., 1., 0., 0.],
    })
}
fn validate_rate(rate: f32) -> Result<(), String> {
    if rate.is_finite() && (1.0..=384000.0).contains(&rate) {
        Ok(())
    } else {
        Err("unsupported filter sample rate".into())
    }
}
impl NativePatch {
    pub fn write_low_cut(&mut self, block: &LowCut, rate: f32) -> Result<(), String> {
        validate_rate(rate)?;
        if !block.frequency.is_finite() {
            return Err("nonfinite low-cut frequency".into());
        }
        let k = ((block.frequency * std::f32::consts::PI) / rate).tan();
        let denominator = k * 1.4144272 + 1.0 + k * k;
        let second = ((1.0 - k * 1.4144272) + k * k) / denominator;
        let first = ((k + k) * k - 2.0) / denominator;
        let mut next = self.clone();
        next.set_block_enabled(0x1000, block.enabled);
        next.apply_block_coefficients("low-cut", &[first.to_bits(), second.to_bits()])?;
        next.set_low_cut_fourth_order(block.fourth_order);
        *self = next;
        Ok(())
    }
    pub fn write_pick_filter(&mut self, block: &PickFilter, rate: f32) -> Result<(), String> {
        let coefficients = biquad_coefficients(6, block.frequency, block.q, rate)?;
        if !block.decay.is_finite() {
            return Err("nonfinite pick-filter decay".into());
        }
        let decay = (-1.0 / (block.decay * rate)).exp();
        let mut next = self.clone();
        next.set_block_enabled(0x200000, block.enabled);
        next.apply_block_coefficients(
            "pick-filter",
            &[
                coefficients[0].to_bits(),
                coefficients[1].to_bits(),
                decay.to_bits(),
            ],
        )?;
        next.set_table(TableKind::PickFilterGain, block.gain)?;
        *self = next;
        Ok(())
    }
    pub fn write_compressor(&mut self, block: &Compressor, rate: f32) -> Result<(), String> {
        validate_rate(rate)?;
        if !block.attack_time.is_finite() || !block.release_time.is_finite() {
            return Err("nonfinite compressor times".into());
        }
        let attack = (-1.0 / (block.attack_time * rate)).exp();
        let release = (-1.0 / (block.release_time * rate)).exp();
        let mut next = self.clone();
        next.set_block_enabled(0x20000, block.enabled);
        next.apply_block_coefficients("compressor", &[attack.to_bits(), release.to_bits()])?;
        next.set_table(TableKind::CompressorPostGain, block.post_gain)?;
        *self = next;
        Ok(())
    }
    pub fn write_reverb(&mut self, block: &Reverb, rate: f32) -> Result<(), String> {
        validate_rate(rate)?;
        if block
            .allpass_delays
            .iter()
            .chain(block.tank_decay.iter())
            .chain([block.pre_delay].iter())
            .any(|v| !v.is_finite())
        {
            return Err("nonfinite reverb times".into());
        }
        let mut next = self.clone();
        next.set_block_enabled(0x400, block.enabled);
        for (i, delay) in block.allpass_delays.iter().enumerate() {
            let samples = ((delay.max(0.) / 1000.) * rate) as u32;
            next.set_reverb_allpass_delay(i, samples)?;
        }
        next.set_table(TableKind::ReverbAllpassDensity, block.allpass_density)?;
        for (tank, multiplier) in [2237., 2399., 2621., 3169.].into_iter().enumerate() {
            let values = std::array::from_fn(|i| {
                let factor = -3.0 / ((block.tank_decay[i] / 1000.) * rate);
                10.0_f32.powf(factor * multiplier)
            });
            next.set_table(TableKind::ReverbFeedback(tank), values)?;
        }
        next.set_reverb_pre_delay(((block.pre_delay.max(0.) / 1000.) * rate) as u32);
        next.set_reverb_control_sources(
            block.allpass_density_control_source,
            block.tank_feedback_control_source,
        );
        *self = next;
        Ok(())
    }
}

/// Bounded numeric snapshots used by offline reference tools only.
pub fn write_filter_analysis(
    patch: &mut NativePatch,
    kind: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let length = match kind {
        "low-cut" => 0x3c,
        "pick-filter" => 0x54,
        "compressor" => 0x40,
        "reverb" => 0x114,
        _ => return Err("unknown filter block".into()),
    };
    if bytes.len() != length || bytes[4] > 1 {
        return Err("invalid filter snapshot length or enabled flag".into());
    }
    let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let value = |o| f32::from_bits(word(o));
    let enabled = bytes[4] != 0;
    let rate = value(8);
    match kind {
        "low-cut" => {
            if bytes[0x10] > 1 {
                return Err("noncanonical filter-order flag".into());
            }
            patch.write_low_cut(
                &LowCut {
                    enabled,
                    frequency: value(0xc),
                    fourth_order: bytes[0x10] != 0,
                },
                rate,
            )
        }
        "pick-filter" => patch.write_pick_filter(
            &PickFilter {
                enabled,
                frequency: value(0xc),
                q: value(0x10),
                decay: value(0x14),
                gain: std::array::from_fn(|i| value(0x18 + i * 4)),
            },
            rate,
        ),
        "compressor" => patch.write_compressor(
            &Compressor {
                enabled,
                attack_time: value(0xc),
                release_time: value(0x10),
                post_gain: std::array::from_fn(|i| value(0x14 + i * 4)),
            },
            rate,
        ),
        "reverb" => patch.write_reverb(
            &Reverb {
                enabled,
                allpass_delays: std::array::from_fn(|i| value(0xc + i * 4)),
                allpass_density: std::array::from_fn(|i| value(0x1c + i * 4)),
                tank_decay: std::array::from_fn(|i| value(0x4c + i * 4)),
                pre_delay: value(0x40),
                allpass_density_control_source: word(0x44),
                tank_feedback_control_source: word(0x48),
            },
            rate,
        ),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_biquad_modes_match_original_sse_arithmetic() {
        for line in include_str!("../../tests/fixtures/native-dsp-biquad.txt").lines() {
            let v: Vec<_> = line
                .split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(v.len(), 9);
            let actual = biquad_coefficients(
                v[0],
                f32::from_bits(v[1]),
                f32::from_bits(v[2]),
                f32::from_bits(v[3]),
            )
            .unwrap();
            assert_eq!(actual.map(f32::to_bits), v[4..], "{line}");
        }
    }
    #[test]
    fn complete_filter_parameter_to_word_paths_match_native() {
        for line in include_str!("../../tests/fixtures/native-dsp-filter-blocks.txt").lines() {
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
            write_filter_analysis(&mut patch, f[0], &bytes).unwrap();
            assert_eq!(patch.words, expected, "{} {}", f[0], f[1]);
        }
    }
    #[test]
    fn invalid_filter_inputs_leave_existing_words_unchanged() {
        for (kind, length) in [
            ("low-cut", 0x3c),
            ("pick-filter", 0x54),
            ("compressor", 0x40),
            ("reverb", 0x114),
        ] {
            let mut patch = NativePatch::default();
            patch.words.fill(0xff123456);
            let before = patch.clone();
            for size in 0..length {
                assert!(write_filter_analysis(&mut patch, kind, &vec![0; size]).is_err());
                assert_eq!(patch, before);
            }
            let mut bytes = vec![0; length];
            bytes[8..12].copy_from_slice(&31250_f32.to_le_bytes());
            bytes[0xc..0x10].copy_from_slice(&f32::NAN.to_le_bytes());
            assert!(write_filter_analysis(&mut patch, kind, &bytes).is_err());
            assert_eq!(patch, before);
        }
        assert!(biquad_coefficients(6, 440., 0., 31250.).is_err());
        assert!(biquad_coefficients(7, 440., 0.707, 0.).is_err());
    }
}

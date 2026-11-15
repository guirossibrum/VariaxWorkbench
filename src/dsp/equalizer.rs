//! Original mixed-precision string EQ calculations and packed-band ordering.
use super::NativePatch;
use crate::body::{EqBands, StringEq, TwelveStringEq};
use crate::math::EngineFloat as _;

#[derive(Clone, Copy, Debug)]
pub struct EqParameters {
    pub kind: u32,
    pub frequency: f32,
    pub q: f32,
    pub gain_db: f32,
    pub mix: f32,
}
/// Coefficient order a1,a2,b0,b1,b2, matching the native band object fields.
pub fn equalizer_coefficients(p: EqParameters, rate: f32) -> Result<[f32; 5], String> {
    if !rate.is_finite()
        || !(1.0..=384000.0).contains(&rate)
        || [p.frequency, p.q, p.gain_db, p.mix]
            .iter()
            .any(|v| !v.is_finite())
        || (p.kind == 1 && p.q <= 0.0)
    {
        return Err("unsupported equalizer parameters".into());
    }
    let damping = f64::from(if (4..=5).contains(&p.kind) {
        1.4144272
    } else {
        1.0 / p.q
    });
    let exponent = if p.gain_db < 0.0 {
        p.gain_db / -20.0
    } else {
        p.gain_db / 20.0
    };
    let gain = 10.0_f64.engine_powf(f64::from(exponent)) - 1.0;
    let mut out = match p.kind {
        1 => {
            let k = f64::from(((p.frequency * std::f32::consts::PI) / rate).engine_tan());
            let (a1, a2, term) = if p.gain_db < 0.0 {
                let product = (gain + 1.0) * damping * k;
                let inverse = 1.0 / (product + 1.0 + k * k);
                (
                    ((k + k) * k - 2.0) * inverse,
                    ((1.0 - product) + k * k) * inverse,
                    -(damping * k) * inverse,
                )
            } else {
                let product = damping * k;
                let inverse = 1.0 / (product + 1.0 + k * k);
                (
                    ((k + k) * k - 2.0) * inverse,
                    ((1.0 - product) + k * k) * inverse,
                    product * inverse,
                )
            };
            [
                a1 as f32,
                a2 as f32,
                (gain * term + 1.0) as f32,
                a1 as f32,
                (-term * gain + a2) as f32,
            ]
        }
        2 => {
            let r = f64::from(((p.frequency * -std::f32::consts::TAU) / rate).engine_exp());
            let (a1, positive, negative) = if p.gain_db < 0.0 {
                let sum = r + 1.0;
                let inverse = 1.0 / (2.0 / sum + gain);
                (
                    (sum * -0.5 * gain - r) / (sum * 0.5 * gain + 1.0),
                    -inverse,
                    inverse,
                )
            } else {
                (-r, (r + 1.0) * 0.5, -((r + 1.0) * 0.5))
            };
            [
                a1 as f32,
                0.,
                (gain * positive + 1.0) as f32,
                (negative * gain + a1) as f32,
                0.,
            ]
        }
        3 => {
            let r = f64::from(((p.frequency * -std::f32::consts::TAU) / rate).engine_exp());
            let (a1, term) = if p.gain_db < 0.0 {
                let inverse = 1.0 / ((gain + 1.0) - gain * r);
                (-r * inverse, (r - 1.0) * inverse)
            } else {
                (-r, 1.0 - r)
            };
            [
                a1 as f32,
                0.,
                (gain * term + 1.0) as f32,
                (gain * 0.0 + a1) as f32,
                0.,
            ]
        }
        4 => {
            let k = f64::from(((p.frequency * std::f32::consts::PI) / rate).engine_tan());
            let square = 1.0 / (k * k);
            let inverse = 1.0 / (square + damping / k + 1.0);
            let a2 = ((square - damping / k) + 1.0) * inverse;
            let term = -1.0 / (k * k) + 1.0;
            [((term + term) * inverse) as f32, a2 as f32, 1., -2., 1.]
        }
        5 => {
            let k = f64::from(((p.frequency * std::f32::consts::PI) / rate).engine_tan());
            let square = 1.0 / (k * k);
            let inverse = 1.0 / (square + damping / k + 1.0);
            let term = -1.0 / (k * k) + 1.0;
            let a2 = ((square - damping / k) + 1.0) * inverse;
            let a1 = (term + term) * inverse;
            [a1 as f32, a2 as f32, (a1 + 1.0 + a2) as f32, 0., 0.]
        }
        _ => [0., 0., 1., 0., 0.],
    };
    // Native instructions narrow the wet product to float, then combine with
    // a double dry complement/product before the final float narrowing.
    let complement = f64::from(p.mix) - 1.0;
    out[4] = (f64::from(p.mix * out[4]) - f64::from(out[1]) * complement) as f32;
    out[3] = (f64::from(p.mix * out[3]) - f64::from(out[0]) * complement) as f32;
    out[2] = (f64::from(p.mix * out[2]) + (1.0 - f64::from(p.mix))) as f32;
    Ok(out)
}
fn parameters<const N: usize>(bands: &EqBands<N>, index: usize) -> EqParameters {
    EqParameters {
        kind: bands.filter_types[index],
        frequency: bands.frequencies[index],
        q: bands.q[index],
        gain_db: bands.gain[index],
        mix: bands.mix[index],
    }
}
fn args(index: usize, c: [f32; 5]) -> [u32; 6] {
    [
        index as u32,
        c[1].to_bits(),
        c[0].to_bits(),
        c[4].to_bits(),
        c[3].to_bits(),
        c[2].to_bits(),
    ]
}
impl NativePatch {
    pub fn write_string_eq(&mut self, block: &StringEq, rate: f32) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(4, block.enabled);
        let mut packed = 0;
        for (string, input) in block.strings.iter().enumerate() {
            if !input.level.is_finite() {
                return Err("nonfinite string-EQ level".into());
            }
            next.apply_block_coefficients(
                "string-eq-gain",
                &[string as u32, input.level.clamp(0., 1.).to_bits()],
            )?;
            let mut count = 0;
            for band in 0..6 {
                if input.bands.filter_types[band] == 0 {
                    continue;
                }
                let c = equalizer_coefficients(parameters(&input.bands, band), rate)?;
                next.apply_block_coefficients("string-eq", &args(packed, c))?;
                packed += 1;
                count += 1;
            }
            next.set_string_eq_band_count(string, count)?;
        }
        for i in packed..36 {
            next.apply_block_coefficients("string-eq", &args(i, [0., 0., 1., 0., 0.]))?;
        }
        *self = next;
        Ok(())
    }
    pub fn write_twelve_string_eq(
        &mut self,
        block: &TwelveStringEq,
        rate: f32,
    ) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(0x80000, block.enabled);
        for (string, bands) in block.strings.iter().enumerate() {
            for band in 0..2 {
                let c = equalizer_coefficients(parameters(bands, band), rate)?;
                next.apply_block_coefficients("twelve-eq", &args(string * 2 + band, c))?;
            }
        }
        *self = next;
        Ok(())
    }
}

/// Canonical offline EQ interchange: enabled/rate/count/version (four LE words),
/// then six string levels followed by count band records of kind/freq/Q/gain/mix.
/// This contains no pointers and is not an on-device or vendor preset format.
pub fn write_eq_analysis(patch: &mut NativePatch, kind: &str, bytes: &[u8]) -> Result<(), String> {
    let bands = match kind {
        "string-eq" => 6,
        "twelve-eq" => 2,
        _ => return Err("unknown EQ block".into()),
    };
    if bytes.len() != 16 + 6 * (4 + bands * 20) {
        return Err("invalid EQ analysis length".into());
    }
    let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let value = |o| f32::from_bits(word(o));
    if word(0) > 1 || word(8) != bands as u32 || word(12) != 1 {
        return Err("invalid EQ analysis header".into());
    }
    let rate = value(4);
    if !rate.is_finite() || !(1.0..=384000.0).contains(&rate) {
        return Err("unsupported EQ sample rate".into());
    }
    let enabled = word(0) != 0;
    if bands == 6 {
        let strings = std::array::from_fn(|s| {
            let base = 16 + s * (4 + 6 * 20);
            crate::body::StringEqString {
                level: value(base),
                bands: EqBands {
                    filter_types: std::array::from_fn(|i| word(base + 4 + i * 20)),
                    frequencies: std::array::from_fn(|i| value(base + 8 + i * 20)),
                    q: std::array::from_fn(|i| value(base + 12 + i * 20)),
                    gain: std::array::from_fn(|i| value(base + 16 + i * 20)),
                    mix: std::array::from_fn(|i| value(base + 20 + i * 20)),
                },
            }
        });
        patch.write_string_eq(&StringEq { enabled, strings }, rate)
    } else {
        let strings = std::array::from_fn(|s| {
            let base = 16 + s * (4 + 2 * 20);
            EqBands {
                filter_types: std::array::from_fn(|i| word(base + 4 + i * 20)),
                frequencies: std::array::from_fn(|i| value(base + 8 + i * 20)),
                q: std::array::from_fn(|i| value(base + 12 + i * 20)),
                gain: std::array::from_fn(|i| value(base + 16 + i * 20)),
                mix: std::array::from_fn(|i| value(base + 20 + i * 20)),
            }
        });
        patch.write_twelve_string_eq(&TwelveStringEq { enabled, strings }, rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_precision_coefficients_match_original_library() {
        for line in include_str!("../../tests/fixtures/native-dsp-equalizer.txt")
            .lines()
            .chain(include_str!("../../tests/fixtures/native-dsp-eq-mixing.txt").lines())
        {
            let v: Vec<_> = line
                .split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(v.len(), 11);
            let p = EqParameters {
                kind: v[0],
                frequency: f32::from_bits(v[1]),
                q: f32::from_bits(v[2]),
                gain_db: f32::from_bits(v[3]),
                mix: f32::from_bits(v[4]),
            };
            assert_eq!(
                equalizer_coefficients(p, f32::from_bits(v[5]))
                    .unwrap()
                    .map(f32::to_bits),
                v[6..],
                "{line}"
            );
        }
    }
    #[test]
    fn full_string_eq_blocks_match_all_native_words() {
        for line in include_str!("../../tests/fixtures/native-dsp-eq-blocks.txt").lines() {
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
            write_eq_analysis(&mut patch, f[0], &bytes).unwrap();
            assert_eq!(patch.words, expected, "{} {}", f[0], f[1]);
        }
    }
    #[test]
    fn malformed_equalizer_snapshots_do_not_change_the_patch() {
        for (kind, bands) in [("string-eq", 6), ("twelve-eq", 2)] {
            let mut patch = NativePatch::default();
            patch.words.fill(0x654321);
            let before = patch.clone();
            let length = 16 + 6 * (4 + bands * 20);
            for size in 0..length {
                assert!(write_eq_analysis(&mut patch, kind, &vec![0; size]).is_err());
                assert_eq!(patch, before);
            }
            assert!(write_eq_analysis(&mut patch, kind, &vec![0; length]).is_err());
            assert_eq!(patch, before);
        }
    }
}

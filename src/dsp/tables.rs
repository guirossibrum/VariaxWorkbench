//! Native coefficient-table word conversions, without hardware transport.
use super::{NativePatch, vx_float_words};

/// Original finite Q23 saturation and asymmetric negative rounding. The possible
/// 25th bit is retained until Pack masks each word down to 24 bits.
pub fn signed_q23(value: f32) -> Result<u32, String> {
    if !value.is_finite() {
        return Err("fixed-point coefficient must be finite".into());
    }
    let value = value.clamp(-1., 1.);
    Ok(if value < 0. {
        0x1000000 - ((value * -8388607.0 - 0.5).max(0.) as u32)
    } else {
        (value * 8388607.0 + 0.5).max(0.) as u32
    })
}

#[derive(Clone, Copy, Debug)]
pub enum TableKind {
    MicRandomLevel,
    VolumeCurve,
    ReverbAllpassDensity,
    ReverbFeedback(usize),
    PickupLowpass,
    PickFilterGain,
    CompressorPostGain,
    MicNotchDepth,
    MicPosition,
    Tone2Gain,
    Tone2Q,
    Tone1Gain,
    ToneOffGain,
    ToneOffQ,
    ToneOffFrequency,
}
impl NativePatch {
    pub fn set_table(&mut self, kind: TableKind, values: [f32; 9]) -> Result<(), String> {
        let (offset, vx, multiplier) = match kind {
            TableKind::MicRandomLevel => (0x50, false, 1.),
            TableKind::VolumeCurve => (0x888, false, 1.),
            TableKind::ReverbAllpassDensity => (0x9e0, false, 1.),
            TableKind::ReverbFeedback(tank) => {
                if tank >= 4 {
                    return Err("reverb tank index exceeds three".into());
                }
                (0xa0c + tank * 0x24, false, 1.)
            }
            TableKind::PickupLowpass => (0x9c, false, 1.),
            TableKind::PickFilterGain => (0x9c, false, 0.25),
            TableKind::CompressorPostGain => (0xc8, false, 1.),
            TableKind::MicNotchDepth => (8, false, 1.),
            TableKind::MicPosition => (0x2c, false, 1.),
            TableKind::Tone2Gain => (0x228, true, 1.),
            TableKind::Tone2Q => (0x1e0, true, 1.),
            TableKind::Tone1Gain => (0x198, true, 1.),
            TableKind::ToneOffGain => (0x150, true, 1.),
            TableKind::ToneOffQ => (0xc0, true, 1.),
            TableKind::ToneOffFrequency => (0x108, true, 1.),
        };
        if vx {
            for (i, value) in values.into_iter().enumerate() {
                self.words[offset / 4 + i * 2..offset / 4 + i * 2 + 2]
                    .copy_from_slice(&vx_float_words(value));
            }
        } else {
            let converted: Vec<_> = values
                .into_iter()
                .map(|v| signed_q23(v * multiplier))
                .collect::<Result<_, _>>()?;
            self.words[offset / 4..offset / 4 + 9].copy_from_slice(&converted);
        }
        Ok(())
    }
    pub fn set_pickup_secondary_frequency_coefficient(&mut self, value: f32) {
        self.words[0x94 / 4..0x94 / 4 + 2].copy_from_slice(&vx_float_words(value));
    }
    pub fn set_pickup_coil_gain(&mut self, index: usize, value: f32) -> Result<(), String> {
        if index >= 4 {
            return Err("coil gain index exceeds three".into());
        }
        let word = signed_q23(value)?;
        self.words[0xc0c / 4 + index] = word;
        Ok(())
    }
    pub fn set_pickup_pole(&mut self, index: usize, value: f32, phase: bool) -> Result<(), String> {
        if index >= 24 {
            return Err("pole index exceeds 23".into());
        }
        let word = signed_q23(value)? | if phase { 0x800000 } else { 0 };
        self.words[0x14 / 4 + index] = word;
        Ok(())
    }
    pub fn set_sum_to_mono_level(&mut self, string: usize, value: f32) -> Result<(), String> {
        if string >= 6 {
            return Err("string index exceeds five".into());
        }
        let word = signed_q23(value)?;
        self.words[0x870 / 4 + string] = word;
        Ok(())
    }
    pub fn set_trim_level(&mut self, value: f32) -> Result<(), String> {
        let word = signed_q23(value)?;
        self.words[0x8ac / 4] = word | 0x800000;
        Ok(())
    }
    pub fn set_drone_level(&mut self, value: f32) -> Result<(), String> {
        let word = signed_q23(value)?;
        self.words[0x928 / 4] = word;
        Ok(())
    }

    pub fn apply_coefficient(&mut self, name: &str, args: &[u32]) -> Result<(), String> {
        let table = |a: &[u32]| -> Result<[f32; 9], String> {
            Ok(<[u32; 9]>::try_from(a)
                .map_err(|_| "expected nine float bit patterns")?
                .map(f32::from_bits))
        };
        match (name, args) {
            ("secondary-frequency", [v]) => {
                self.set_pickup_secondary_frequency_coefficient(f32::from_bits(*v))
            }
            ("coil-gain", [index, v]) => {
                self.set_pickup_coil_gain(*index as usize, f32::from_bits(*v))?
            }
            ("pole", [index, v, phase]) => {
                if *phase > 1 {
                    return Err("phase must be 0 or 1".into());
                }
                self.set_pickup_pole(*index as usize, f32::from_bits(*v), *phase != 0)?;
            }
            ("sum-level", [index, v]) => {
                self.set_sum_to_mono_level(*index as usize, f32::from_bits(*v))?
            }
            ("trim-level", [v]) => self.set_trim_level(f32::from_bits(*v))?,
            ("drone-level", [v]) => self.set_drone_level(f32::from_bits(*v))?,
            ("reverb-feedback", [tank, values @ ..]) => {
                self.set_table(TableKind::ReverbFeedback(*tank as usize), table(values)?)?
            }
            (name, values) => {
                let kind = match name {
                    "mic-random" => TableKind::MicRandomLevel,
                    "volume-curve" => TableKind::VolumeCurve,
                    "reverb-density" => TableKind::ReverbAllpassDensity,
                    "pickup-lowpass" => TableKind::PickupLowpass,
                    "pick-gain" => TableKind::PickFilterGain,
                    "compressor-gain" => TableKind::CompressorPostGain,
                    "mic-notch" => TableKind::MicNotchDepth,
                    "mic-position" => TableKind::MicPosition,
                    "tone2-gain" => TableKind::Tone2Gain,
                    "tone2-q" => TableKind::Tone2Q,
                    "tone1-gain" => TableKind::Tone1Gain,
                    "tone-off-gain" => TableKind::ToneOffGain,
                    "tone-off-q" => TableKind::ToneOffQ,
                    "tone-off-frequency" => TableKind::ToneOffFrequency,
                    _ => return self.apply_block_coefficients(name, values),
                };
                self.set_table(kind, table(values)?)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_coefficient_writes_match_native_and_preserve_other_words() {
        for line in include_str!("../../tests/fixtures/native-dsp-tables.txt")
            .lines()
            .chain(include_str!("../../tests/fixtures/native-dsp-coefficients.txt").lines())
        {
            let (input, output) = line.split_once(" => ").unwrap();
            let f: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(f[0], 16).unwrap();
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
            let args: Vec<_> = f[2..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            patch.apply_coefficient(f[1], &args).unwrap();
            assert_eq!(patch.words, expected, "{line}");
        }
    }
    #[test]
    fn invalid_coefficients_never_partially_mutate_the_patch() {
        for (name, args) in [
            ("coil-gain", vec![4, 0]),
            ("pole", vec![24, 0, 0]),
            ("pole", vec![0, 0, 2]),
            ("sum-level", vec![6, 0]),
            ("trim-level", vec![f32::NAN.to_bits()]),
            ("volume-curve", vec![0; 8]),
            ("reverb-feedback", vec![4, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (
                "mic-position",
                vec![0, 0, 0, 0, 0, 0, 0, 0, f32::INFINITY.to_bits()],
            ),
        ] {
            let mut patch = NativePatch::default();
            patch.words.fill(0x12345678);
            let before = patch.clone();
            assert!(patch.apply_coefficient(name, &args).is_err());
            assert_eq!(patch, before);
        }
    }
}

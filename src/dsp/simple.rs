//! DSP parameter conversions and complete word writes for simple blocks.
use super::{NativePatch, TableKind};
use crate::body::{Drone, GlobalDecay, MicPosition, PitchShift, Volume};
use crate::math::EngineFloat as _;

fn rate_valid(rate: f32) -> Result<(), String> {
    if rate.is_finite() && (1.0..=384000.0).contains(&rate) {
        Ok(())
    } else {
        Err("unsupported DSP sample rate".into())
    }
}
impl NativePatch {
    pub fn write_volume(&mut self, block: &Volume, trim_db: Option<f32>) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(0x800, block.enabled);
        if let Some(trim) = trim_db {
            if !trim.is_finite() {
                return Err("nonfinite volume trim".into());
            }
            next.set_trim_level(10.0_f32.engine_powf(trim / 20.0))?;
        } else {
            next.set_analog_level(block.analog_gain);
        }
        next.set_table(TableKind::VolumeCurve, block.curve)?;
        *self = next;
        Ok(())
    }
    pub fn write_sum_to_mono(&mut self, enabled: bool, levels: [f32; 6]) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(0x80, enabled);
        for (i, value) in levels.into_iter().enumerate() {
            next.set_sum_to_mono_level(i, value)?;
        }
        *self = next;
        Ok(())
    }
    pub fn write_global_decay(&mut self, block: &GlobalDecay, rate: f32) -> Result<(), String> {
        rate_valid(rate)?;
        let mut next = self.clone();
        next.set_block_enabled(0x40, block.enabled);
        for i in 0..6 {
            if !block.hold_off[i].is_finite() || !block.decay[i].is_finite() {
                return Err("nonfinite decay duration".into());
            }
            let hold = (-1.0 / (block.hold_off[i] * rate)).engine_exp();
            let decay = (-1.0 / (block.decay[i] * rate)).engine_exp();
            next.apply_block_coefficients(
                "global-decay",
                &[
                    i as u32,
                    hold.to_bits(),
                    decay.to_bits(),
                    (-block.minimum[i]).to_bits(),
                ],
            )?;
        }
        *self = next;
        Ok(())
    }
    pub fn write_drone(&mut self, block: &Drone, rate: f32) -> Result<(), String> {
        rate_valid(rate)?;
        if block.frequencies.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite drone frequency".into());
        }
        let count = block.num_strings.min(12);
        let mut next = self.clone();
        next.set_block_enabled(0x4000, block.enabled);
        next.set_drone_count(count)?;
        for i in 0..12 {
            let (period, feedback) = if (i as u32) < count {
                (
                    ((rate / block.frequencies[i]) * 8192.0) as u32,
                    block.feedback[i],
                )
            } else {
                (0, 0.)
            };
            next.apply_block_coefficients(
                "drone-string",
                &[i as u32, period.min(0x190000), feedback.to_bits()],
            )?;
        }
        next.set_drone_level(block.level)?;
        *self = next;
        Ok(())
    }
    pub fn write_pitch_shift(&mut self, block: &PitchShift, rate: f32) -> Result<(), String> {
        rate_valid(rate)?;
        if block
            .tune
            .iter()
            .chain(block.delay.iter())
            .chain(block.mix.iter())
            .chain(block.level.iter())
            .any(|v| !v.is_finite())
        {
            return Err("nonfinite pitch-shift parameters".into());
        }
        let mut next = self.clone();
        next.set_block_enabled(0x10, block.enabled);
        for i in 0..6 {
            let ratio = (block.tune[i] / 12.).clamp(-1., 1.);
            let delay = (((rate * block.delay[i]) / 1000.) as u32).max(1);
            next.apply_block_coefficients("pitch-string", &[i as u32, ratio.to_bits(), delay])?;
            next.apply_block_coefficients(
                "twelve-wet-dry",
                &[
                    i as u32,
                    (block.mix[i] * block.level[i]).to_bits(),
                    ((1. - block.mix[i]) * block.level[i]).to_bits(),
                ],
            )?;
        }
        *self = next;
        Ok(())
    }
    pub fn write_mic_position_scaled(
        &mut self,
        block: &MicPosition,
        scale_length: f32,
    ) -> Result<(), String> {
        if !scale_length.is_finite() || scale_length <= 0.0 {
            return Err("invalid microphone-position scale length".into());
        }
        let mut normalized = block.clone();
        normalized.position = block.position.map(|v| v / scale_length);
        self.write_mic_position(&normalized)
    }
    pub fn write_mic_position(&mut self, block: &MicPosition) -> Result<(), String> {
        let mut next = self.clone();
        next.set_block_enabled(0x800000, block.enabled);
        next.set_table(TableKind::MicNotchDepth, block.notch_depth)?;
        next.set_table(TableKind::MicPosition, block.position)?;
        next.set_table(TableKind::MicRandomLevel, block.random_level)?;
        *self = next;
        Ok(())
    }
}

/// Decode a bounded snapshot of numeric fields solely for offline oracle tests.
/// All pointer/vtable slots are ignored. These are not vendor file formats.
pub fn write_simple_analysis(
    patch: &mut NativePatch,
    kind: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let length = match kind {
        "volume" => 0x3c,
        "sum" => 0x24,
        "decay" => 0x84,
        "drone" => 0xa4,
        "pitch" => 0x6c,
        "mic" | "mic-parameters" => 0xa0,
        _ => return Err("unknown simple DSP block".into()),
    };
    if bytes.len() != length || bytes[4] > 1 {
        return Err("wrong DSP snapshot length or enabled flag".into());
    }
    let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let value = |o| f32::from_bits(word(o));
    let rate = value(8);
    let enabled = bytes[4] != 0;
    match kind {
        "volume" => {
            if bytes[0x34] > 1 {
                return Err("noncanonical trim flag".into());
            }
            patch.write_volume(
                &Volume {
                    enabled,
                    curve: std::array::from_fn(|i| value(0xc + i * 4)),
                    analog_gain: word(0x30),
                    continuous_gain: 0.,
                },
                if bytes[0x34] != 0 {
                    Some(value(0x38))
                } else {
                    None
                },
            )
        }
        "sum" => patch.write_sum_to_mono(enabled, std::array::from_fn(|i| value(0xc + i * 4))),
        "decay" => patch.write_global_decay(
            &GlobalDecay {
                enabled,
                hold_off: std::array::from_fn(|i| value(0xc + i * 20)),
                decay: std::array::from_fn(|i| value(0x10 + i * 20)),
                minimum: std::array::from_fn(|i| value(0x14 + i * 20)),
            },
            rate,
        ),
        "drone" => patch.write_drone(
            &Drone {
                enabled,
                num_strings: word(0xc),
                level: value(0x10),
                frequencies: std::array::from_fn(|i| value(0x14 + i * 4)),
                feedback: std::array::from_fn(|i| value(0x44 + i * 4)),
            },
            rate,
        ),
        "pitch" => patch.write_pitch_shift(
            &PitchShift {
                enabled,
                tune: std::array::from_fn(|i| value(0xc + i * 4)),
                delay: std::array::from_fn(|i| value(0x24 + i * 4)),
                mix: std::array::from_fn(|i| value(0x3c + i * 4)),
                level: std::array::from_fn(|i| value(0x54 + i * 4)),
            },
            rate,
        ),
        "mic-parameters" => patch.write_mic_position_scaled(
            &MicPosition {
                enabled,
                notch_depth: std::array::from_fn(|i| value(0xc + i * 4)),
                random_level: std::array::from_fn(|i| value(0x54 + i * 4)),
                position: std::array::from_fn(|i| value(0x30 + i * 4)),
            },
            value(0x78),
        ),
        "mic" => patch.write_mic_position(&MicPosition {
            enabled,
            notch_depth: std::array::from_fn(|i| value(0xc + i * 4)),
            random_level: std::array::from_fn(|i| value(0x54 + i * 4)),
            position: std::array::from_fn(|i| value(0x7c + i * 4)),
        }),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode_hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn parameter_to_word_blocks_match_complete_native_outputs() {
        for line in include_str!("../../tests/fixtures/native-dsp-simple-blocks-v2.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let f: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(f[1], 16).unwrap();
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
            write_simple_analysis(&mut patch, f[0], &decode_hex(f[2])).unwrap();
            assert_eq!(patch.words, expected, "{} {}", f[0], f[1]);
        }
    }
    #[test]
    fn malformed_snapshots_and_invalid_values_preserve_previous_words() {
        for (kind, length) in [
            ("volume", 0x3c),
            ("sum", 0x24),
            ("decay", 0x84),
            ("drone", 0xa4),
            ("pitch", 0x6c),
            ("mic", 0xa0),
            ("mic-parameters", 0xa0),
        ] {
            let mut patch = NativePatch::default();
            patch.words.fill(0xabcdef);
            let before = patch.clone();
            for size in 0..length {
                assert!(write_simple_analysis(&mut patch, kind, &vec![0; size]).is_err());
                assert_eq!(patch, before);
            }
            let mut source = vec![0; length];
            source[4] = 2;
            assert!(write_simple_analysis(&mut patch, kind, &source).is_err());
            assert_eq!(patch, before);
            source[4] = 0;
            source[8..12].copy_from_slice(&31250_f32.to_le_bytes());
            let offset = match kind {
                "volume" => 0xc,
                "sum" => 0xc,
                "decay" => 0xc,
                "drone" => 0x10,
                "pitch" => 0xc,
                "mic" | "mic-parameters" => 0xc,
                _ => unreachable!(),
            };
            source[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
            assert!(write_simple_analysis(&mut patch, kind, &source).is_err());
            assert_eq!(patch, before);
        }
    }
}

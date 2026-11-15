//! Offline logical native-word patch. It has no device transport or upload API.
use super::{DATA_WORDS, PACKED_BYTES, pack_native_words};

/// Original FloatToVXFloat is a bit transformation, including signed zeros,
/// subnormals and nonfinite bit patterns; no IEEE float operation is performed.
pub fn vx_float_words(value: f32) -> [u32; 2] {
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 255) + 1;
    let fraction = if bits & 0x80000000 != 0 {
        ((bits >> 8) & 0x800000) + ((!bits & 0x7fffff) >> 1)
    } else {
        ((bits & 0x7fffff) + 0x800000) >> 1
    };
    [exponent, fraction]
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativePatch {
    pub words: [u32; DATA_WORDS],
}
impl Default for NativePatch {
    fn default() -> Self {
        Self {
            words: [0; DATA_WORDS],
        }
    }
}
impl NativePatch {
    pub fn packed(&self) -> [u8; PACKED_BYTES] {
        pack_native_words(&self.words)
    }
    pub fn set_block_status(&mut self, value: u32) {
        self.words[1] = (value & 0xffffff) | 0x883;
    }
    pub fn block_status(&self) -> u32 {
        self.words[1]
    }
    pub fn set_block_enabled(&mut self, mask: u32, enabled: bool) {
        let mut value = (!mask & self.words[1]) & 0xffffff;
        if enabled {
            value |= mask;
        }
        self.words[1] = value | 0x883;
    }
    pub fn set_pickup_scale(&mut self, value: f32) {
        self.words[2..4].copy_from_slice(&vx_float_words(value));
    }
    pub fn set_pickup_num_coils(&mut self, value: u32) -> Result<(), String> {
        if value > 4 {
            return Err("pickup coil count exceeds four".into());
        }
        self.words[4] = value;
        Ok(())
    }
    pub fn set_pickup_aperture(&mut self, string: usize, value: u32) -> Result<(), String> {
        if string >= 6 {
            return Err("string index exceeds five".into());
        }
        self.words[0x74 / 4 + string] = value;
        Ok(())
    }
    pub fn set_body_resonator_count(&mut self, count: u32) -> Result<(), String> {
        if count > 12 {
            return Err("body resonator count exceeds twelve".into());
        }
        self.words[0x290 / 4] = count;
        Ok(())
    }
    pub fn set_low_cut_fourth_order(&mut self, enabled: bool) {
        self.words[0x324 / 4] = u32::from(enabled);
    }
    pub fn set_string_eq_band_count(&mut self, string: usize, count: u32) -> Result<(), String> {
        if string >= 6 || count > 5 {
            return Err("unsupported string or EQ band count".into());
        }
        self.words[0x588 / 4 + string] = count;
        Ok(())
    }
    pub fn set_analog_level(&mut self, level: u32) {
        self.words[0x8ac / 4] = level;
    }
    pub fn set_reverb_allpass_delay(&mut self, index: usize, value: u32) -> Result<(), String> {
        if index >= 4 {
            return Err("allpass delay index exceeds three".into());
        }
        self.words[0x9d0 / 4 + index] = value;
        Ok(())
    }
    pub fn set_reverb_pre_delay(&mut self, value: u32) {
        self.words[0xa08 / 4] = value;
    }
    pub fn set_reverb_control_sources(&mut self, allpass: u32, feedback: u32) {
        self.words[0xa04 / 4] = allpass;
        self.words[0xa9c / 4] = feedback;
    }
    pub fn set_drone_count(&mut self, count: u32) -> Result<(), String> {
        if count > 12 {
            return Err("drone count exceeds twelve".into());
        }
        self.words[0x92c / 4] = count;
        Ok(())
    }
    pub fn set_tone_function(&mut self, value: u16) {
        let word = &mut self.words[0x90 / 4];
        *word = (*word & 0xffff) | (u32::from(value) << 16);
    }
    /// Float range validation belongs to the application. This setter accepts
    /// only the meaningful normalized range, unlike native integer overflow.
    pub fn set_tone_position(&mut self, value: f32, saved: bool) -> Result<(), String> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("tone position must be finite and normalized".into());
        }
        self.words[0x90 / 4] &= 0xfff000;
        self.words[0x90 / 4] |= if saved { (value * 255.0) as u32 } else { 0x1ff };
        Ok(())
    }
}

impl NativePatch {
    /// Closed analysis-only opcode set for differential fixtures. No arbitrary
    /// memory addresses or transport operations can be supplied here.
    pub fn apply_scalar(&mut self, name: &str, args: &[u32]) -> Result<(), String> {
        let boolean = |v: u32| {
            if v <= 1 {
                Ok(v != 0)
            } else {
                Err("expected canonical boolean".to_string())
            }
        };
        match (name, args) {
            ("status", [v]) => self.set_block_status(*v),
            ("block", [mask, enabled]) => self.set_block_enabled(*mask, boolean(*enabled)?),
            ("scale", [bits]) => self.set_pickup_scale(f32::from_bits(*bits)),
            ("coils", [v]) => self.set_pickup_num_coils(*v)?,
            ("aperture", [index, v]) => self.set_pickup_aperture(*index as usize, *v)?,
            ("resonators", [v]) => self.set_body_resonator_count(*v)?,
            ("fourth-order", [v]) => self.set_low_cut_fourth_order(boolean(*v)?),
            ("eq-bands", [index, count]) => {
                self.set_string_eq_band_count(*index as usize, *count)?
            }
            ("analog", [v]) => self.set_analog_level(*v),
            ("allpass-delay", [index, v]) => self.set_reverb_allpass_delay(*index as usize, *v)?,
            ("pre-delay", [v]) => self.set_reverb_pre_delay(*v),
            ("reverb-sources", [a, b]) => self.set_reverb_control_sources(*a, *b),
            ("drone-count", [v]) => self.set_drone_count(*v)?,
            ("tone-function", [v]) => self
                .set_tone_function(u16::try_from(*v).map_err(|_| "tone function exceeds 16 bits")?),
            ("tone-position", [bits, saved]) => {
                self.set_tone_position(f32::from_bits(*bits), boolean(*saved)?)?
            }
            _ => return Err("unknown scalar opcode or wrong argument count".into()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vx_encoding_matches_original_for_arbitrary_ieee_patterns() {
        for line in include_str!("../../tests/fixtures/native-vx-float.txt").lines() {
            let values: Vec<_> = line
                .split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(
                vx_float_words(f32::from_bits(values[0])),
                [values[1], values[2]],
                "{line}"
            );
        }
    }
    #[test]
    fn scalar_setters_preserve_every_unrelated_native_word() {
        for line in include_str!("../../tests/fixtures/native-dsp-scalar.txt").lines() {
            let (input, output) = line.split_once(" => ").unwrap();
            let fields: Vec<_> = input.split_whitespace().collect();
            let mut seed = u32::from_str_radix(fields[0], 16).unwrap();
            let mut patch = NativePatch::default();
            for word in &mut patch.words {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *word = seed;
            }
            let mut expected = patch.words;
            for changed in output.split_whitespace() {
                let (index, value) = changed.split_once(':').unwrap();
                expected[usize::from_str_radix(index, 16).unwrap()] =
                    u32::from_str_radix(value, 16).unwrap();
            }
            let args: Vec<_> = fields[2..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            patch.apply_scalar(fields[1], &args).unwrap();
            assert_eq!(patch.words, expected, "{line}");
        }
    }
    #[test]
    fn malformed_scalar_inputs_leave_all_words_unchanged() {
        for (name, args) in [
            ("coils", vec![5]),
            ("aperture", vec![6, 1]),
            ("resonators", vec![13]),
            ("eq-bands", vec![6, 1]),
            ("eq-bands", vec![0, 6]),
            ("allpass-delay", vec![4, 1]),
            ("drone-count", vec![13]),
            ("tone-function", vec![65536]),
            ("tone-position", vec![f32::NAN.to_bits(), 1]),
            ("tone-position", vec![2.0_f32.to_bits(), 1]),
            ("block", vec![1, 2]),
            ("arbitrary-address", vec![0, 0]),
            ("scale", vec![]),
        ] {
            let mut patch = NativePatch::default();
            patch.words.fill(0xa5a5a5a5);
            let before = patch.clone();
            assert!(patch.apply_scalar(name, &args).is_err());
            assert_eq!(patch, before);
        }
    }
}

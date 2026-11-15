//! Offline decoding of the original Workbench body-definition resources.
//!
//! Verified against the original i386 reader; see docs/body-definitions.md.
//! Decoded values are a view of the input. The original bytes remain unchanged.

pub const BODY_DEFINITION_BYTES: usize = 17_483;
pub const BODY_VARIANT_BYTES: usize = 2_897;
pub const BODY_VARIANT_NAMES: [&str; 6] = ["bb", "mb", "nb", "mm", "nm", "nn"];

#[derive(Debug, Clone, PartialEq)]
pub struct PickupDefaults {
    pub position: f32,
    pub angle: f32,
    pub pole_offsets: [f32; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyResonance {
    pub enabled: bool,
    pub resonator_enabled: [bool; 12],
    pub frequencies: [f32; 12],
    pub q: [f32; 12],
    pub gain: [f32; 12],
    pub treble_frequency: f32,
    pub rolloff_frequency: f32,
    pub master: f32,
    pub body_mix: f32,
    pub mid_mix: f32,
    pub treble_mix: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowCut {
    pub enabled: bool,
    pub frequency: f32,
    pub fourth_order: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyVariant {
    pub body_resonance: BodyResonance,
    pub low_cut: LowCut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyDefinition {
    original: Vec<u8>,
    pub has_middle: bool,
    /// Bridge, middle, then neck, including defaults for absent pickups.
    pub pickup_defaults: [PickupDefaults; 3],
    /// Original numeric mode; interpretation is not yet verified.
    pub interpolation_mode: u32,
    /// Original order: bb, mb, nb, mm, nm, nn.
    pub variants: [BodyVariant; 6],
}

fn float(bytes: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn floats<const N: usize>(bytes: &[u8], offset: usize) -> [f32; N] {
    std::array::from_fn(|i| float(bytes, offset + i * 4))
}

impl BodyDefinition {
    /// Decode an exact legacy resource. Booleans follow the native != 0 rule;
    /// floating-point bit patterns are retained, including nonfinite values.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != BODY_DEFINITION_BYTES {
            return Err(format!(
                "body definition must contain {BODY_DEFINITION_BYTES} bytes, got {}",
                bytes.len()
            ));
        }
        let pickup_defaults = std::array::from_fn(|i| {
            let start = 1 + i * 32;
            PickupDefaults {
                position: float(bytes, start),
                angle: float(bytes, start + 4),
                pole_offsets: floats(bytes, start + 8),
            }
        });
        let variants = std::array::from_fn(|i| {
            let b = &bytes[101 + i * BODY_VARIANT_BYTES..101 + (i + 1) * BODY_VARIANT_BYTES];
            BodyVariant {
                body_resonance: BodyResonance {
                    enabled: b[0] != 0,
                    resonator_enabled: std::array::from_fn(|j| b[1 + j] != 0),
                    frequencies: floats(b, 13),
                    q: floats(b, 61),
                    gain: floats(b, 109),
                    treble_frequency: float(b, 157),
                    rolloff_frequency: float(b, 161),
                    master: float(b, 165),
                    body_mix: float(b, 169),
                    mid_mix: float(b, 173),
                    treble_mix: float(b, 177),
                },
                low_cut: LowCut {
                    enabled: b[181] != 0,
                    frequency: float(b, 182),
                    fourth_order: b[186] != 0,
                },
            }
        });
        Ok(Self {
            original: bytes.to_vec(),
            has_middle: bytes[0] != 0,
            pickup_defaults,
            interpolation_mode: u32::from_be_bytes(bytes[97..101].try_into().unwrap()),
            variants,
        })
    }

    /// Exact input, including all currently opaque variant fields. Editing the
    /// decoded public values does not modify these bytes or create a device patch.
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }

    pub fn variant_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= self.variants.len() {
            return None;
        }
        let start = 101 + index * BODY_VARIANT_BYTES;
        Some(&self.original[start..start + BODY_VARIANT_BYTES])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/native-body-definition.bin");

    #[test]
    fn native_verified_synthetic_header_and_all_variants() {
        let body = BodyDefinition::parse(FIXTURE).unwrap();
        assert!(body.has_middle);
        assert_eq!(body.interpolation_mode, 4);
        for (i, p) in body.pickup_defaults.iter().enumerate() {
            assert_eq!(p.position, (2 + i * 3) as f32 * 0.125);
            assert_eq!(p.angle, (3 + i * 3) as f32 * 0.125);
            for j in 0..6 {
                assert_eq!(
                    p.pole_offsets[j],
                    (4 + i * 3) as f32 * 0.125 + j as f32 * 0.03125
                );
            }
        }
        for (i, variant) in body.variants.iter().enumerate() {
            let base = i * 235;
            let r = &variant.body_resonance;
            assert_eq!(r.enabled, (11 + base) % 2 != 0);
            for j in 0..12 {
                assert_eq!(r.resonator_enabled[j], (12 + base + j) % 2 != 0);
                assert_eq!(
                    r.frequencies[j],
                    (14 + base) as f32 * 0.125 + j as f32 * 0.03125
                );
                assert_eq!(r.q[j], (15 + base) as f32 * 0.125 + j as f32 * 0.03125);
                assert_eq!(r.gain[j], (16 + base) as f32 * 0.125 + j as f32 * 0.03125);
            }
            assert_eq!(
                [
                    r.treble_frequency,
                    r.rolloff_frequency,
                    r.master,
                    r.body_mix,
                    r.mid_mix,
                    r.treble_mix
                ],
                std::array::from_fn::<_, 6, _>(|j| (17 + base + j) as f32 * 0.125)
            );
            assert_eq!(variant.low_cut.enabled, (22 + base) % 2 != 0);
            assert_eq!(variant.low_cut.frequency, (24 + base) as f32 * 0.125);
            assert_eq!(variant.low_cut.fourth_order, (24 + base) % 2 != 0);
            assert_eq!(body.variant_bytes(i).unwrap().len(), BODY_VARIANT_BYTES);
        }
        assert_eq!(body.original_bytes(), FIXTURE);
        assert!(body.variant_bytes(6).is_none());
        assert!(body.variant_bytes(usize::MAX).is_none());
    }

    #[test]
    fn rejects_every_truncated_input_and_trailing_data() {
        for len in 0..FIXTURE.len() {
            assert!(BodyDefinition::parse(&FIXTURE[..len]).is_err());
        }
        let mut extra = FIXTURE.to_vec();
        extra.push(0);
        assert!(BodyDefinition::parse(&extra).is_err());
    }

    #[test]
    fn native_boolean_semantics_and_float_bits_are_preserved() {
        let mut bytes = FIXTURE.to_vec();
        bytes[0] = 255;
        bytes[1..5].copy_from_slice(&0x7fc01234u32.to_be_bytes());
        let body = BodyDefinition::parse(&bytes).unwrap();
        assert!(body.has_middle);
        assert_eq!(body.pickup_defaults[0].position.to_bits(), 0x7fc01234);
        assert_eq!(body.original_bytes(), bytes);
    }
}

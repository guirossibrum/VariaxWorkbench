//! Physical pickup definition files recovered from the native serial reader.

#[derive(Clone, Debug, PartialEq)]
pub struct PickupDefinition {
    pub dc_resistance: f64,
    pub magnetic_resistance: f64,
    pub inductance: f64,
    pub capacitance: f64,
    pub is_humbucker: bool,
    pub volume_offset: f32,
    pub pole_offsets: [f32; 6],
    pub coil_spacing: [f32; 6],
}

impl PickupDefinition {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 85 {
            return Err(format!(
                "expected 85-byte pickup definition, received {}",
                bytes.len()
            ));
        }
        let double = |offset| f64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap());
        let float = |offset| f32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
        Ok(Self {
            dc_resistance: double(0),
            magnetic_resistance: double(8),
            inductance: double(16),
            capacitance: double(24),
            is_humbucker: bytes[32] != 0,
            volume_offset: float(33),
            pole_offsets: std::array::from_fn(|i| float(37 + i * 4)),
            coil_spacing: std::array::from_fn(|i| float(61 + i * 4)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_fixture_verified_by_original_native_reader() {
        let bytes = include_bytes!("../tests/fixtures/native-pickup-definition.bin");
        let model = PickupDefinition::parse(bytes).unwrap();
        assert_eq!(model.dc_resistance, 1234.5);
        assert_eq!(model.magnetic_resistance, 6789.25);
        assert_eq!(model.inductance, 2.125);
        assert_eq!(model.capacitance, 0.000000345);
        assert!(model.is_humbucker);
        assert_eq!(model.volume_offset, -3.5);
        assert_eq!(model.pole_offsets, [0.0, 0.125, 0.25, 0.375, 0.5, 0.625]);
        assert_eq!(model.coil_spacing, [0.0, -0.25, -0.5, -0.75, -1.0, -1.25]);
        for size in 0..85 {
            assert!(PickupDefinition::parse(&bytes[..size]).is_err());
        }
    }
}

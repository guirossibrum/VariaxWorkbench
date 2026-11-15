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

/// Raw passive-circuit values consumed by the original pickup-response engine.
/// This helper preserves native arithmetic; parameter-range validation is a
/// separate stage and this structure does not describe the entire audio model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupCircuit {
    pub dc_resistance: f64,
    pub magnetic_resistance: f64,
    pub inductance: f64,
    pub capacitance: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupWiring {
    Parallel,
    Series,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombinedPickup {
    pub circuit: PickupCircuit,
    pub active: bool,
}
impl From<&PickupDefinition> for PickupCircuit {
    fn from(model: &PickupDefinition) -> Self {
        Self {
            dc_resistance: model.dc_resistance,
            magnetic_resistance: model.magnetic_resistance,
            inductance: model.inductance,
            capacitance: model.capacitance,
        }
    }
}
impl PickupCircuit {
    /// Native CombinePickups: zero inductance denotes a disabled pickup. A sole
    /// active pickup is copied verbatim; combined magnetic resistances use abs.
    /// Zero denominators/nonfinite values follow IEEE arithmetic, not a safety
    /// certification. No circuit calculation contacts or programs any device.
    pub fn combine(self, other: Self, wiring: PickupWiring) -> CombinedPickup {
        let circuit = if self.inductance == 0.0 {
            other
        } else if other.inductance == 0.0 {
            self
        } else if wiring == PickupWiring::Series {
            Self {
                dc_resistance: self.dc_resistance + other.dc_resistance,
                magnetic_resistance: self.magnetic_resistance.abs()
                    + other.magnetic_resistance.abs(),
                inductance: self.inductance + other.inductance,
                capacitance: (self.capacitance * other.capacitance)
                    / (self.capacitance + other.capacitance),
            }
        } else {
            Self {
                dc_resistance: (self.dc_resistance * other.dc_resistance)
                    / (self.dc_resistance + other.dc_resistance),
                magnetic_resistance: (self.magnetic_resistance.abs()
                    * other.magnetic_resistance.abs())
                    / (self.magnetic_resistance.abs() + other.magnetic_resistance.abs()),
                inductance: (self.inductance * other.inductance)
                    / (self.inductance + other.inductance),
                capacitance: self.capacitance + other.capacitance,
            }
        };
        CombinedPickup {
            active: circuit.inductance != 0.0,
            circuit,
        }
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
    #[test]
    fn passive_combinations_match_original_machine_code() {
        for line in include_str!("../tests/fixtures/native-pickup-combinations.txt").lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            assert_eq!(columns.len(), 14);
            let bits = |i: usize| u64::from_str_radix(columns[i], 16).unwrap();
            let circuit = |start: usize| PickupCircuit {
                dc_resistance: f64::from_bits(bits(start)),
                magnetic_resistance: f64::from_bits(bits(start + 1)),
                inductance: f64::from_bits(bits(start + 2)),
                capacitance: f64::from_bits(bits(start + 3)),
            };
            let wiring = if columns[8] == "1" {
                PickupWiring::Series
            } else {
                PickupWiring::Parallel
            };
            let actual = circuit(0).combine(circuit(4), wiring);
            assert_eq!(actual.active, columns[9] == "1");
            let c = actual.circuit;
            assert_eq!(
                [
                    c.dc_resistance.to_bits(),
                    c.magnetic_resistance.to_bits(),
                    c.inductance.to_bits(),
                    c.capacitance.to_bits()
                ],
                std::array::from_fn::<_, 4, _>(|i| bits(i + 10)),
                "native mismatch: {line}"
            );
        }
    }
}

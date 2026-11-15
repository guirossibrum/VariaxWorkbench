//! Pickup geometry primitives and original normalized pole projection.
use crate::math::EngineFloat as _;
use crate::{body::BodyDefinition, models::PickupDefinition};

impl BodyDefinition {
    /// Native GetPoleOffsets interpolation at an already normalized location.
    pub fn pole_offsets_at(&self, position: f32) -> Result<[f32; 6], String> {
        if !position.is_finite() || !(0.0..=1.0).contains(&position) {
            return Err("normalized pole position must be finite and in 0..=1".into());
        }
        let (left, right, weight) = if !self.has_middle {
            (0, 2, position)
        } else if position < 0.5 {
            (0, 1, position + position)
        } else {
            (1, 2, (position - 0.5) + (position - 0.5))
        };
        Ok(std::array::from_fn(|i| {
            weight * self.pickup_defaults[right].pole_offsets[i]
                + (1.0 - weight) * self.pickup_defaults[left].pole_offsets[i]
        }))
    }
}

#[derive(Clone, Debug)]
pub struct PlacedCoils {
    pub positions: Vec<[f32; 6]>,
    pub reverse_phase: bool,
}
impl PickupDefinition {
    /// Native host geometry formula; angle is the host's distance-domain value,
    /// not degrees. Java's angle conversion belongs to the host pipeline.
    pub fn place_coils(
        &self,
        body_offsets: [f32; 6],
        position: f32,
        angle: f32,
        reverse_phase: bool,
        single_coil: bool,
    ) -> Result<PlacedCoils, String> {
        if !position.is_finite()
            || !angle.is_finite()
            || body_offsets
                .iter()
                .chain(self.pole_offsets.iter())
                .chain(self.coil_spacing.iter())
                .any(|v| !v.is_finite())
        {
            return Err("nonfinite coil geometry".into());
        }
        let first = std::array::from_fn(|i| {
            let fraction = i as f32 / 5.0;
            (((fraction + fraction) - 1.0) * angle + self.pole_offsets[i])
                + position
                + body_offsets[i]
        });
        let mut positions = vec![first];
        if self.is_humbucker && !single_coil {
            positions.push(std::array::from_fn(|i| first[i] + self.coil_spacing[i]));
        }
        Ok(PlacedCoils {
            positions,
            reverse_phase,
        })
    }
}

#[derive(Clone, Debug)]
pub struct GeometryInput {
    pub enabled: bool,
    pub scale_length: f32,
    pub num_coils: u32,
    pub magnetic_aperture: [u32; 6],
    pub coil_gain_db: [f32; 4],
    pub positions: [[f32; 6]; 4],
    pub phases: [[bool; 6]; 4],
}
#[derive(Clone, Debug)]
pub struct GeometryProjection {
    pub enabled: bool,
    pub scale_length: f32,
    pub num_coils: u32,
    pub magnetic_aperture: [u32; 6],
    pub coil_gains: [f32; 4],
    pub poles: [(f32, bool); 24],
}
impl GeometryInput {
    /// Offline analysis interchange, not a vendor file or hardware packet.
    /// LE scale/count, six apertures, four dB gains, 24 positions, 24 phases,
    /// enabled. Strictly 169 bytes with canonical boolean fields.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 169 || bytes[144..].iter().any(|&b| b > 1) {
            return Err("expected 169-byte geometry input with canonical booleans".into());
        }
        let word = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        Ok(Self {
            enabled: bytes[168] != 0,
            scale_length: f32::from_bits(word(0)),
            num_coils: word(4),
            magnetic_aperture: std::array::from_fn(|i| word(8 + i * 4)),
            coil_gain_db: std::array::from_fn(|i| f32::from_bits(word(32 + i * 4))),
            positions: std::array::from_fn(|c| {
                std::array::from_fn(|s| f32::from_bits(word(48 + (c * 6 + s) * 4)))
            }),
            phases: std::array::from_fn(|c| std::array::from_fn(|s| bytes[144 + c * 6 + s] != 0)),
        })
    }
    pub fn project(&self) -> Result<GeometryProjection, String> {
        if self.num_coils > 4
            || !self.scale_length.is_finite()
            || self.scale_length <= 0.0
            || self.coil_gain_db.iter().any(|v| !v.is_finite())
            || self.positions.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("unsupported coil count, scale or nonfinite geometry".into());
        }
        let count = self.num_coils as usize;
        let mut poles = [(0.0, false); 24];
        for string in 0..6 {
            let mut values: [f32; 4] = std::array::from_fn(|coil| {
                let sign = if self.phases[coil][string] { -1.0 } else { 1.0 };
                sign * (self.positions[coil][string] / self.scale_length)
            });
            // Preserve the original <= tie swap and float32 signed-zero rules.
            for limit in (1..count).rev() {
                for i in 0..limit {
                    if values[i].abs() <= values[i + 1].abs() {
                        values.swap(i, i + 1);
                    }
                }
            }
            for coil in 0..count {
                let value = values[coil];
                let phase = value < 0.0;
                poles[string * count + coil] = (if phase { -value } else { value }, phase);
            }
        }
        Ok(GeometryProjection {
            enabled: self.enabled,
            scale_length: self.scale_length,
            num_coils: self.num_coils,
            magnetic_aperture: self.magnetic_aperture,
            coil_gains: self
                .coil_gain_db
                .map(|db| 10.0_f32.engine_powf(db / 20.0) * 0.0625),
            poles,
        })
    }
}
impl GeometryProjection {
    pub fn words(&self) -> Vec<u32> {
        let mut words = vec![
            u32::from(self.enabled),
            self.scale_length.to_bits(),
            self.num_coils,
        ];
        words.extend(self.magnetic_aperture);
        words.extend(self.coil_gains.map(f32::to_bits));
        for (position, phase) in self.poles {
            words.extend([position.to_bits(), u32::from(phase)]);
        }
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pole_offsets_and_projection_match_original_synthetic_evidence() {
        let body = BodyDefinition::parse(include_bytes!(
            "../../tests/fixtures/native-body-definition.bin"
        ))
        .unwrap();
        for line in include_str!("../../tests/fixtures/native-body-pole-offsets.txt").lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            let bits = |i| u32::from_str_radix(fields[i], 16).unwrap();
            assert_eq!(
                body.pole_offsets_at(f32::from_bits(bits(0)))
                    .unwrap()
                    .map(f32::to_bits),
                std::array::from_fn::<_, 6, _>(|i| bits(i + 1))
            );
        }
        for line in include_str!("../../tests/fixtures/native-pickup-projection.txt").lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            let raw: Vec<_> = (0..fields[0].len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&fields[0][i..i + 2], 16).unwrap())
                .collect();
            let actual = GeometryInput::parse(&raw).unwrap().project().unwrap();
            let expected: Vec<_> = fields[1..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(actual.words(), expected, "{line}");
        }
    }
    #[test]
    fn placements_match_original_host_geometry_including_single_coil_mode() {
        let body = BodyDefinition::parse(include_bytes!(
            "../../tests/fixtures/native-body-definition.bin"
        ))
        .unwrap();
        let pickup = PickupDefinition::parse(include_bytes!(
            "../../tests/fixtures/native-pickup-definition.bin"
        ))
        .unwrap();
        for line in include_str!("../../tests/fixtures/native-pickup-placement.txt").lines() {
            let f: Vec<_> = line.split_whitespace().collect();
            let bits = |i| u32::from_str_radix(f[i], 16).unwrap();
            let position = f32::from_bits(bits(0));
            let offsets = body
                .pole_offsets_at(body.normalized_pickup_position(position).unwrap())
                .unwrap();
            let placed = pickup
                .place_coils(
                    offsets,
                    position,
                    f32::from_bits(bits(1)),
                    f[2] == "1",
                    f[3] == "1",
                )
                .unwrap();
            let mut actual = vec![placed.positions.len() as u32];
            for coil in placed.positions {
                actual.extend(coil.map(f32::to_bits));
                actual.extend([u32::from(placed.reverse_phase); 6]);
            }
            let expected: Vec<_> = f[4..]
                .iter()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect();
            assert_eq!(actual, expected, "{line}");
        }
    }
    #[test]
    fn malformed_geometry_is_rejected_before_projection() {
        let valid = [0; 169];
        for size in 0..169 {
            assert!(GeometryInput::parse(&valid[..size]).is_err());
        }
        let mut raw = valid;
        raw[168] = 2;
        assert!(GeometryInput::parse(&raw).is_err());
        for (scale, count) in [(0.0_f32, 0), (f32::NAN, 0), (25.0, 5)] {
            raw = valid;
            raw[..4].copy_from_slice(&scale.to_le_bytes());
            raw[4..8].copy_from_slice(&u32::to_le_bytes(count));
            assert!(GeometryInput::parse(&raw).unwrap().project().is_err());
        }
    }
}

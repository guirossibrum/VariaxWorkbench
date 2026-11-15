//! Original table smoothing and float32 control-response conversion (0x262c0).
use super::{ResponseTables, TableSettings, WiringProperties};
use crate::models::PickupCircuit;

/// Values written by CalculateResponse, ordered by ascending native offsets.
/// Untouched flags/control state and vtable bytes are deliberately excluded.
#[derive(Clone, Debug)]
pub struct CalculatedResponse {
    pub lowpass_frequency: [f32; 9],
    pub secondary_gain: [f32; 9],
    pub secondary_q: [f32; 9],
    pub secondary_frequency: f32,
    pub tone_gain: [f32; 9],
    pub primary_gain: [f32; 9],
    pub primary_q: [f32; 9],
    pub primary_frequency: [f32; 9],
    pub lowpass_coefficient: [f32; 9],
    pub secondary_frequency_coefficient: f32,
    pub primary_frequency_coefficient: [f32; 9],
}
impl CalculatedResponse {
    /// Return exactly the native-assigned fields as (object offset, float bits).
    pub fn fields(&self) -> Vec<(usize, u32)> {
        let mut fields = Vec::with_capacity(83);
        for (start, values) in [
            (0x18, &self.lowpass_frequency),
            (0x3c, &self.secondary_gain),
            (0x60, &self.secondary_q),
            (0x88, &self.tone_gain),
            (0xac, &self.primary_gain),
            (0xd0, &self.primary_q),
            (0xf4, &self.primary_frequency),
            (0x11c, &self.lowpass_coefficient),
            (0x144, &self.primary_frequency_coefficient),
        ] {
            fields.extend(
                values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (start + i * 4, v.to_bits())),
            );
        }
        fields.push((0x84, self.secondary_frequency.to_bits()));
        fields.push((0x140, self.secondary_frequency_coefficient.to_bits()));
        fields.sort_unstable_by_key(|(offset, _)| *offset);
        fields
    }
}
fn smooth(values: &mut [f64]) {
    for _ in 0..5 {
        let targets: [f64; 7] = std::array::from_fn(|i| {
            let center = i + 1;
            let mut total = 0.0;
            let mut weights = 0.0;
            for delta in -2_i32..=2 {
                let index = (center as i32 + delta).clamp(0, 8) as usize;
                let weight = f64::from(1.0_f32 / (delta.unsigned_abs() as f32 + 1.0));
                let db = if values[index] <= 0.0 {
                    f64::from(1e-9_f32)
                } else {
                    f64::from((values[index] as f32).log10() * 20.0)
                };
                total += db * weight;
                weights += weight;
            }
            total / weights
        });
        for (i, target) in targets.iter().enumerate() {
            let amplitude = f64::from(10.0_f32.powf(*target as f32 / 20.0));
            values[i + 1] += (amplitude - values[i + 1]) * f64::from(0.4_f32);
        }
    }
}
impl PickupCircuit {
    pub fn calculated_response(
        self,
        right: Self,
        mut wiring: WiringProperties,
        settings: TableSettings,
    ) -> Result<Option<CalculatedResponse>, String> {
        let frozen_tone = wiring.tone_capacitance < 0.0;
        if frozen_tone {
            wiring.tone_capacitance = -wiring.tone_capacitance;
        }
        let Some(ResponseTables(mut data)) = self.response_tables(right, wiring, settings)? else {
            return Ok(None);
        };
        smooth(&mut data[54..63]);
        smooth(&mut data[45..54]);
        let frequency = data[63] as f32;
        let frequency_coefficient = |value: f32| {
            (1.0 / f64::from((value * std::f32::consts::PI) / settings.sample_rate).tan()) as f32
        };
        let lowpass_frequency = std::array::from_fn(|i| data[i] as f32);
        let primary_frequency = std::array::from_fn(|i| data[9 + i] as f32);
        Ok(Some(CalculatedResponse {
            lowpass_frequency,
            secondary_gain: std::array::from_fn(|i| {
                data[54 + if frozen_tone { 0 } else { i }] as f32
            }),
            secondary_q: std::array::from_fn(|i| data[45 + if frozen_tone { 0 } else { i }] as f32),
            secondary_frequency: frequency,
            tone_gain: std::array::from_fn(|i| data[36 + if frozen_tone { 0 } else { i }] as f32),
            primary_gain: std::array::from_fn(|i| data[18 + i] as f32),
            primary_q: std::array::from_fn(|i| data[27 + i] as f32),
            primary_frequency,
            lowpass_coefficient: std::array::from_fn(|i| {
                ((lowpass_frequency[i] * -std::f32::consts::TAU) / settings.sample_rate).exp()
            }),
            secondary_frequency_coefficient: frequency_coefficient(frequency),
            primary_frequency_coefficient: primary_frequency.map(frequency_coefficient),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{models::PickupWiring, pickup::VolumeTaper};
    #[test]
    fn all_assigned_fields_match_original_after_smoothing_and_freeze() {
        for line in include_str!("../../tests/fixtures/native-pickup-calculated.txt")
            .lines()
            .chain(include_str!("../../tests/fixtures/native-pickup-calculated-wiring.txt").lines())
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 99);
            let value = |i| f64::from_bits(u64::from_str_radix(fields[i], 16).unwrap());
            let circuit = |i| PickupCircuit {
                dc_resistance: value(i),
                magnetic_resistance: value(i + 1),
                inductance: value(i + 2),
                capacitance: value(i + 3),
            };
            let wiring = WiringProperties {
                load_capacitance: value(11),
                load_resistance: value(12),
                tone_resistance: value(13),
                volume_resistance: value(14),
                tone_capacitance: value(15),
            };
            let settings = TableSettings {
                sample_rate: f32::from_bits(u32::from_str_radix(fields[8], 16).unwrap()),
                wiring: if fields[9] == "1" {
                    PickupWiring::Series
                } else {
                    PickupWiring::Parallel
                },
                taper: if fields[10] == "1" {
                    VolumeTaper::Logarithmic
                } else {
                    VolumeTaper::Linear
                },
            };
            let actual = circuit(0)
                .calculated_response(circuit(4), wiring, settings)
                .unwrap()
                .unwrap();
            let expected: Vec<_> = fields[16..]
                .iter()
                .map(|v| {
                    let (offset, bits) = v.split_once(':').unwrap();
                    (
                        usize::from_str_radix(offset, 16).unwrap(),
                        u32::from_str_radix(bits, 16).unwrap(),
                    )
                })
                .collect();
            assert_eq!(actual.fields(), expected, "{line}");
        }
    }
}

//! Native 64-double electrical response tables; no hardware patch assembly.
use super::{FilterParameters, SecondaryFit, WiringProperties};
use crate::models::{PickupCircuit, PickupWiring};

#[derive(Clone, Copy, Debug)]
pub enum VolumeTaper {
    Linear,
    Logarithmic,
}
#[derive(Clone, Copy, Debug)]
pub struct TableSettings {
    pub sample_rate: f32,
    pub wiring: PickupWiring,
    pub taper: VolumeTaper,
}
/// Seven nine-value tables followed by the circuit resonance frequency.
/// Order: volume cutoff, primary frequency/gain/Q, tone gain, secondary Q/gain.
#[derive(Clone, Debug)]
pub struct ResponseTables(pub [f64; 64]);
impl PickupCircuit {
    pub fn response_tables(
        self,
        right: Self,
        wiring: WiringProperties,
        settings: TableSettings,
    ) -> Result<Option<ResponseTables>, String> {
        if !settings.sample_rate.is_finite() || !(1.0..=384000.0).contains(&settings.sample_rate) {
            return Err("unsupported table sample rate".into());
        }
        let combined = self.combine(right, settings.wiring);
        if !combined.active {
            return Ok(None);
        }
        let circuit = combined.circuit;
        let mut data = [0.0; 64];
        data[63] = 1.0
            / (((circuit.capacitance + wiring.tone_capacitance) * circuit.inductance).sqrt()
                * std::f64::consts::TAU);
        for index in (0..9).rev() {
            let volume = match settings.taper {
                VolumeTaper::Linear => (index + 1) as f64 / 9.0,
                VolumeTaper::Logarithmic => 0.01 * (1.0_f64 / 0.01).powf(index as f64 * 0.125),
            };
            let primary = circuit.fit_primary_filter(wiring, volume, 1.0)?;
            data[9 + index] = primary.frequency;
            data[18 + index] = primary.gain;
            data[27 + index] = primary.q;
            data[index] = if volume != 1.0 {
                1.0 / (volume
                    * wiring.volume_resistance
                    * (1.0 - volume)
                    * std::f64::consts::TAU
                    * wiring.load_capacitance)
            } else {
                f64::from(settings.sample_rate * 0.5)
            };
            if index != 8 {
                continue;
            }
            for tone_index in (0..9).rev() {
                let tone = if tone_index == 0 {
                    0.0
                } else {
                    (1.0_f64 / 0.001).powf(tone_index as f64 * 0.125) * 0.001
                };
                let gain = data[26] / circuit.response(wiring, data[17], volume, tone).magnitude();
                data[36 + tone_index] = gain;
                if tone < 0.04 {
                    let adjusted = FilterParameters {
                        frequency: data[17],
                        q: data[35] / gain,
                        gain: data[26] / gain,
                    };
                    let secondary = circuit.fit_secondary_filter(
                        wiring,
                        adjusted,
                        SecondaryFit {
                            sample_rate: settings.sample_rate,
                            volume,
                            tone,
                            frequency_limit: data[63],
                        },
                    )?;
                    if secondary.q > 0.0 {
                        data[45 + tone_index] = secondary.q;
                        data[54 + tone_index] = secondary.gain;
                        continue;
                    }
                }
                data[45 + tone_index] = 1.0;
                data[54 + tone_index] = 0.0;
            }
        }
        for q in &mut data[45..54] {
            if *q > 5.0 {
                *q = 5.0;
            }
        }
        // Native scans backwards beyond the gain table when none exceeds 0.2,
        // then its unsigned guard skips filling. Bound the search in Rust and
        // preserve that outcome without reproducing out-of-table reads.
        if let Some(index) = (0..9).rev().find(|&i| data[54 + i] > 0.2) {
            let q = data[45 + index];
            data[45 + index..54].fill(q);
        }
        let gain = data[26];
        for value in &mut data[18..27] {
            *value *= 1.0 / gain;
        }
        Ok(Some(ResponseTables(data)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_tables_match_original_synthetic_outputs() {
        for line in include_str!("../../tests/fixtures/native-pickup-tables.txt")
            .lines()
            .chain(include_str!("../../tests/fixtures/native-pickup-tables-wiring.txt").lines())
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 80);
            let bits = |i| u64::from_str_radix(fields[i], 16).unwrap();
            let value = |i| f64::from_bits(bits(i));
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
                .response_tables(circuit(4), wiring, settings)
                .unwrap()
                .unwrap();
            assert_eq!(
                actual.0.map(f64::to_bits),
                std::array::from_fn::<_, 64, _>(|i| bits(i + 16)),
                "{line}"
            );
        }
    }
    #[test]
    fn inactive_circuits_and_unsupported_rates_never_emit_tables() {
        let inactive = PickupCircuit {
            dc_resistance: 0.0,
            magnetic_resistance: 0.0,
            inductance: 0.0,
            capacitance: 0.0,
        };
        let wiring = WiringProperties {
            load_capacitance: 1e-9,
            load_resistance: 1e6,
            tone_resistance: 5e5,
            volume_resistance: 5e5,
            tone_capacitance: 2.2e-8,
        };
        let settings = TableSettings {
            sample_rate: 31250.,
            wiring: PickupWiring::Parallel,
            taper: VolumeTaper::Linear,
        };
        assert!(
            inactive
                .response_tables(inactive, wiring, settings)
                .unwrap()
                .is_none()
        );
        for rate in [0.0, -1.0, f32::NAN, f32::INFINITY, 1e9] {
            assert!(
                inactive
                    .response_tables(
                        inactive,
                        wiring,
                        TableSettings {
                            sample_rate: rate,
                            ..settings
                        }
                    )
                    .is_err()
            );
        }
    }
}

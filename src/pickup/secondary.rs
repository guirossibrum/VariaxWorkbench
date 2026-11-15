//! Residual-response secondary peak fitting, recovered from native 0x252b0.
use super::{Biquad, FilterParameters, WiringProperties};
use crate::models::PickupCircuit;

#[derive(Clone, Copy, Debug)]
pub struct SecondaryFit {
    pub sample_rate: f32,
    pub volume: f64,
    pub tone: f64,
    pub frequency_limit: f64,
}
impl PickupCircuit {
    pub fn fit_secondary_filter(
        self,
        wiring: WiringProperties,
        primary: FilterParameters,
        settings: SecondaryFit,
    ) -> Result<FilterParameters, String> {
        if !settings.sample_rate.is_finite()
            || !(1.0..=384000.0).contains(&settings.sample_rate)
            || !settings.volume.is_finite()
            || !(0.0..=1.0).contains(&settings.volume)
            || settings.volume == 0.0
            || !settings.tone.is_finite()
            || !(0.0..=1.0).contains(&settings.tone)
            || !settings.frequency_limit.is_finite()
            || settings.frequency_limit < 0.0
        {
            return Err("unsupported secondary-fit rate, controls or frequency limit".into());
        }
        let filter = Biquad::bandpass(settings.sample_rate, primary);
        let residual = |frequency| {
            let inverse = filter
                .response(settings.sample_rate, frequency)
                .reciprocal();
            self.response(wiring, frequency, settings.volume, settings.tone)
                .multiply(inverse)
                .magnitude()
        };
        let mut high = settings.frequency_limit * 6.0;
        let mut low = 0.0;
        let mut result = FilterParameters {
            frequency: -1.0,
            q: 0.0,
            gain: 0.0,
        };
        for _ in 0..9999 {
            if high - low <= 10.0 {
                break;
            }
            let point = (high + low) * 0.5;
            let before = residual(point - 10.0);
            let center = residual(point);
            let after = residual(point + 10.0);
            if after < center && center < before {
                high = point;
            }
            if center < after && before < center {
                low = point;
            }
            result.frequency = point;
            result.gain = center;
            if after < center && before < center {
                break;
            }
        }
        if result.frequency == -1.0 {
            return Ok(FilterParameters {
                frequency: 0.0,
                q: 0.0,
                gain: 0.0,
            });
        }
        let mut point = result.frequency;
        let mut upper = f64::from(settings.sample_rate * 0.5);
        while point < upper {
            if residual(point) <= result.gain * 0.707 {
                upper = point;
                break;
            }
            point += 10.0;
        }
        point = result.frequency;
        result.q = point / (upper - (point / upper) * point);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn residual_filter_matches_original_synthetic_outputs() {
        for line in include_str!("../../tests/fixtures/native-pickup-secondary.txt").lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 19);
            let bits = |i| u64::from_str_radix(fields[i], 16).unwrap();
            let value = |i| f64::from_bits(bits(i));
            let circuit = PickupCircuit {
                dc_resistance: value(0),
                magnetic_resistance: value(1),
                inductance: value(2),
                capacitance: value(3),
            };
            let wiring = WiringProperties {
                load_capacitance: value(5),
                load_resistance: value(6),
                tone_resistance: value(7),
                volume_resistance: value(8),
                tone_capacitance: value(9),
            };
            let primary = FilterParameters {
                frequency: value(13),
                q: value(14),
                gain: value(15),
            };
            let settings = SecondaryFit {
                sample_rate: f32::from_bits(u32::from_str_radix(fields[4], 16).unwrap()),
                volume: value(10),
                tone: value(11),
                frequency_limit: value(12),
            };
            let actual = circuit
                .fit_secondary_filter(wiring, primary, settings)
                .unwrap();
            assert_eq!(
                [
                    actual.frequency.to_bits(),
                    actual.q.to_bits(),
                    actual.gain.to_bits()
                ],
                [bits(16), bits(17), bits(18)],
                "{line}"
            );
        }
    }
}

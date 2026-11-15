//! Offline pickup circuit transfer calculations recovered from Workbench.
use crate::models::PickupCircuit;
mod biquad;
mod levels;
pub use levels::{BalancedLevels, PickupLevel};
mod geometry;
pub use geometry::{GeometryInput, GeometryProjection, PlacedCoils};
mod calculated;
pub use calculated::CalculatedResponse;
mod secondary;
mod tables;
pub use biquad::Biquad;
pub use secondary::SecondaryFit;
pub use tables::{ResponseTables, TableSettings, VolumeTaper};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComplexResponse {
    pub real: f64,
    pub imaginary: f64,
}
impl ComplexResponse {
    const ZERO: Self = Self::new(0.0, 0.0);
    const fn new(real: f64, imaginary: f64) -> Self {
        Self { real, imaginary }
    }
    fn add(self, b: Self) -> Self {
        Self::new(self.real + b.real, self.imaginary + b.imaginary)
    }
    fn subtract(self, b: Self) -> Self {
        Self::new(self.real - b.real, self.imaginary - b.imaginary)
    }
    fn multiply(self, b: Self) -> Self {
        Self::new(
            self.real * b.real - self.imaginary * b.imaginary,
            self.real * b.imaginary + self.imaginary * b.real,
        )
    }
    fn reciprocal(self) -> Self {
        let denominator = self.real * self.real + self.imaginary * self.imaginary;
        Self::new(self.real / denominator, -self.imaginary / denominator)
    }
    pub fn magnitude(self) -> f64 {
        (self.real * self.real + self.imaginary * self.imaginary).sqrt()
    }
}
fn determinant3(m: [[ComplexResponse; 3]; 3]) -> ComplexResponse {
    let a = m[1][1]
        .multiply(m[2][2])
        .subtract(m[1][2].multiply(m[2][1]));
    let b = m[1][0]
        .multiply(m[2][2])
        .subtract(m[1][2].multiply(m[2][0]));
    let c = m[1][0]
        .multiply(m[2][1])
        .subtract(m[1][1].multiply(m[2][0]));
    m[0][0]
        .multiply(a)
        .subtract(m[0][1].multiply(b))
        .add(m[0][2].multiply(c))
}
fn determinant4(m: [[ComplexResponse; 4]; 4]) -> ComplexResponse {
    let terms: [ComplexResponse; 4] = std::array::from_fn(|column| {
        let mut minor = [[ComplexResponse::ZERO; 3]; 3];
        for row in 0..3 {
            let mut target = 0;
            for (source, value) in m[row + 1].iter().enumerate() {
                if source != column {
                    minor[row][target] = *value;
                    target += 1;
                }
            }
        }
        m[0][column].multiply(determinant3(minor))
    });
    // Native groups the two signed pairs rather than accumulating left-to-right.
    terms[0].subtract(terms[1]).add(terms[2].subtract(terms[3]))
}

/// Native wiring-object values, prior to response-table fitting. Units are the
/// values passed to the original engine (ohms/farads), not the UI's kΩ/µF labels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WiringProperties {
    pub load_capacitance: f64,
    pub load_resistance: f64,
    pub tone_resistance: f64,
    pub volume_resistance: f64,
    pub tone_capacitance: f64,
}

impl PickupCircuit {
    /// Evaluate original complex transfer at frequency Hz and scalar volume/tone
    /// positions. This is circuit math, not audio DSP or a device operation.
    /// Raw IEEE behavior is preserved; parameter validation is a separate stage.
    fn system(
        self,
        wiring: WiringProperties,
        frequency: f64,
        volume: f64,
        tone: f64,
    ) -> ([[ComplexResponse; 4]; 4], f64, f64) {
        let omega = frequency * std::f64::consts::TAU;
        let product = omega * wiring.load_resistance * wiring.load_capacitance;
        let denominator = product * product + 1.0;
        let load_real = wiring.load_resistance / denominator;
        let load_imaginary = -(wiring.load_resistance * product) / denominator;
        let pickup_capacitor = -1.0 / (omega * self.capacitance);
        let tone_resistor = tone * wiring.tone_resistance;
        let volume_resistor = volume * wiring.volume_resistance;
        let tone_capacitor = -1.0 / (omega * wiring.tone_capacitance);
        let sum =
            ComplexResponse::new(load_real + volume_resistor, load_imaginary + 0.0).reciprocal();
        let combined = ComplexResponse::new(
            load_real * volume_resistor - load_imaginary * 0.0,
            load_imaginary * volume_resistor + load_real * 0.0,
        )
        .multiply(sum);
        let matrix = [
            [
                ComplexResponse::new(
                    self.magnetic_resistance + 0.0,
                    omega * self.inductance + 0.0,
                ),
                ComplexResponse::new(-self.magnetic_resistance, -0.0),
                ComplexResponse::ZERO,
                ComplexResponse::ZERO,
            ],
            [
                ComplexResponse::new(-self.magnetic_resistance, -0.0),
                ComplexResponse::new(
                    self.magnetic_resistance + self.dc_resistance + 0.0,
                    pickup_capacitor + 0.0,
                ),
                ComplexResponse::new(-0.0, -pickup_capacitor),
                ComplexResponse::ZERO,
            ],
            [
                ComplexResponse::ZERO,
                ComplexResponse::new(-0.0, -pickup_capacitor),
                ComplexResponse::new(tone_resistor + 0.0, pickup_capacitor + tone_capacitor),
                ComplexResponse::new(-tone_resistor, -tone_capacitor),
            ],
            [
                ComplexResponse::ZERO,
                ComplexResponse::ZERO,
                ComplexResponse::new(-tone_resistor, -tone_capacitor),
                ComplexResponse::new(
                    combined.real + ((1.0 - volume) * wiring.volume_resistance + tone_resistor),
                    combined.imaginary + tone_capacitor + 0.0,
                ),
            ],
        ];
        (matrix, omega, pickup_capacitor)
    }
    pub fn response(
        self,
        wiring: WiringProperties,
        frequency: f64,
        volume: f64,
        tone: f64,
    ) -> ComplexResponse {
        let (mut matrix, omega, pickup_capacitor) = self.system(wiring, frequency, volume, tone);
        let inverse = determinant4(matrix).reciprocal();
        let row1 = matrix[1];
        matrix[1] = [
            ComplexResponse::new(1.0, 0.0),
            ComplexResponse::ZERO,
            ComplexResponse::ZERO,
            ComplexResponse::ZERO,
        ];
        let first = determinant4(matrix).multiply(inverse);
        matrix[1] = row1;
        matrix[2] = [
            ComplexResponse::new(1.0, 0.0),
            ComplexResponse::ZERO,
            ComplexResponse::ZERO,
            ComplexResponse::ZERO,
        ];
        let second = determinant4(matrix).multiply(inverse);
        let difference = first.subtract(second);
        // Retain the original explicit zero terms and complex operation order.
        let real = difference.real * 0.0 - pickup_capacitor * difference.imaginary;
        let imaginary = pickup_capacitor * difference.real + difference.imaginary * 0.0;
        let interim = imaginary * omega + real * 0.0;
        let other = omega * real - imaginary * 0.0;
        ComplexResponse::new(other * 0.0 - interim, interim * 0.0 + other)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterParameters {
    pub frequency: f64,
    pub q: f64,
    pub gain: f64,
}
impl PickupCircuit {
    /// Native primary response peak/half-power fit. Positive volume is required
    /// because the original flat-zero search can consume its full 9,999 limit.
    pub fn fit_primary_filter(
        self,
        wiring: WiringProperties,
        volume: f64,
        tone: f64,
    ) -> Result<FilterParameters, String> {
        if !volume.is_finite()
            || volume <= 0.0
            || volume > 1.0
            || !tone.is_finite()
            || !(0.0..=1.0).contains(&tone)
        {
            return Err("filter fit requires volume in (0,1] and tone in [0,1]".into());
        }
        let mut high = 10000.0;
        let mut low = 0.0;
        let mut point = 0.0;
        let mut result = FilterParameters {
            frequency: 0.0,
            q: 0.1,
            gain: 0.0,
        };
        for _ in 1..10000 {
            if high - low <= 10.0 {
                break;
            }
            point = (low + high) * 0.5;
            let before = self
                .response(wiring, point - 10.0, volume, tone)
                .magnitude();
            let center = self.response(wiring, point, volume, tone).magnitude();
            let after = self
                .response(wiring, point + 10.0, volume, tone)
                .magnitude();
            result.frequency = point;
            result.gain = center;
            if center < before && center > after {
                high = point;
            }
            if before < center && after > center {
                low = point;
            }
            if before < center && after < center {
                break;
            }
        }
        while point > 0.0 {
            point -= 10.0;
            if self.response(wiring, point, volume, tone).magnitude() <= result.gain * 0.707 {
                if point != 0.0 {
                    result.q =
                        result.frequency / ((result.frequency / point) * result.frequency - point);
                }
                break;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_and_primary_filter_match_original_native_vectors() {
        for line in include_str!("../tests/fixtures/native-pickup-response.txt").lines() {
            let c: Vec<_> = line.split_whitespace().collect();
            assert_eq!(c.len(), 15);
            let bits = |i: usize| u64::from_str_radix(c[i], 16).unwrap();
            let value = |i: usize| f64::from_bits(bits(i));
            let circuit = PickupCircuit {
                dc_resistance: value(1),
                magnetic_resistance: value(2),
                inductance: value(3),
                capacitance: value(4),
            };
            let wiring = WiringProperties {
                load_capacitance: value(5),
                load_resistance: value(6),
                tone_resistance: value(7),
                volume_resistance: value(8),
                tone_capacitance: value(9),
            };
            if c[0] == "response" {
                let r = circuit.response(wiring, value(10), value(11), value(12));
                assert_eq!(
                    [r.real.to_bits(), r.imaginary.to_bits()],
                    [bits(13), bits(14)],
                    "{line}"
                );
            } else {
                let r = circuit
                    .fit_primary_filter(wiring, value(10), value(11))
                    .unwrap();
                assert_eq!(
                    [r.frequency.to_bits(), r.q.to_bits(), r.gain.to_bits()],
                    [bits(12), bits(13), bits(14)],
                    "{line}"
                );
            }
        }
    }
    #[test]
    fn primary_filter_rejects_unsupported_control_inputs() {
        let circuit = PickupCircuit {
            dc_resistance: 5000.0,
            magnetic_resistance: 1e5,
            inductance: 2.5,
            capacitance: 1e-10,
        };
        let wiring = WiringProperties {
            load_capacitance: 3.5e-10,
            load_resistance: 1e6,
            tone_resistance: 5e5,
            volume_resistance: 5e5,
            tone_capacitance: 2.2e-8,
        };
        for value in [0.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(circuit.fit_primary_filter(wiring, value, 0.5).is_err());
        }
        for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(circuit.fit_primary_filter(wiring, 0.5, value).is_err());
        }
    }
}

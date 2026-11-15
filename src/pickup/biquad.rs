//! Original pickup-response filter arithmetic, including float32 complex math.
use super::{ComplexResponse, FilterParameters};
use crate::math::EngineFloat as _;

/// Native memory order is [a2, a1, b2, b1, b0]. Feedback signs follow the
/// original denominator 1 + a1*z + a2*z², not packed hardware conventions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad(pub [f64; 5]);

#[derive(Clone, Copy)]
struct Complex32(f32, f32);
impl Complex32 {
    fn multiply(self, other: Self) -> Self {
        let first = self.0 * other.0;
        let second = self.1 * other.1;
        Self(
            first - second,
            ((self.0 + self.1) * (other.0 + other.1) - first) - second,
        )
    }
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1)
    }
    fn divide(self, other: Self) -> Self {
        let scale = 1.0 / (other.0 * other.0 + other.1 * other.1);
        Self(
            (other.0 * self.0 + other.1 * self.1) * scale,
            scale * (other.0 * self.1 - other.1 * self.0),
        )
    }
}
impl Biquad {
    pub fn bandpass(sample_rate: f32, parameters: FilterParameters) -> Self {
        let inverse = 1.0
            / ((parameters.frequency * std::f64::consts::PI) / f64::from(sample_rate)).engine_tan();
        let square = inverse * inverse;
        let scaled = inverse / parameters.q;
        let denominator = 1.0 / (square + scaled + 1.0);
        let a1 = ((1.0 - square) + (1.0 - square)) * denominator;
        let a2 = ((square - scaled) + 1.0) * denominator;
        let b0 = scaled * denominator * parameters.gain;
        Self([a2, a1, -b0, 0.0, b0])
    }

    /// Preserve the original positive-imaginary Reciprocal operation. This raw
    /// host-library response must not be substituted for an audio DSP renderer.
    pub fn response(self, sample_rate: f32, frequency: f64) -> ComplexResponse {
        let theta = (frequency / f64::from(sample_rate)) as f32 * std::f32::consts::TAU;
        let mut z = Complex32(theta.engine_cos(), theta.engine_sin());
        let inverse = 1.0 / (z.0 * z.0 + z.1 * z.1);
        z = Complex32(z.0 * inverse, z.1 * inverse);
        let z2 = z.multiply(z);
        let numerator = Complex32(self.0[4] as f32, 0.0)
            .add(Complex32(self.0[3] as f32, 0.0).multiply(z))
            .add(Complex32(self.0[2] as f32, 0.0).multiply(z2));
        let denominator = Complex32(1.0, 0.0)
            .add(Complex32(self.0[1] as f32, 0.0).multiply(z))
            .add(Complex32(self.0[0] as f32, 0.0).multiply(z2));
        let response = numerator.divide(denominator);
        ComplexResponse {
            real: f64::from(response.0),
            imaginary: f64::from(response.1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bandpass_and_response_match_original_float_bits() {
        for line in include_str!("../../tests/fixtures/native-pickup-biquad.txt").lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 10);
            let rate = f32::from_bits(u32::from_str_radix(fields[1], 16).unwrap());
            let bits = |i| u64::from_str_radix(fields[i], 16).unwrap();
            let value = |i| f64::from_bits(bits(i));
            if fields[0] == "bandpass" {
                let actual = Biquad::bandpass(
                    rate,
                    FilterParameters {
                        frequency: value(2),
                        q: value(3),
                        gain: value(4),
                    },
                );
                assert_eq!(
                    actual.0.map(f64::to_bits),
                    std::array::from_fn::<_, 5, _>(|i| bits(i + 5)),
                    "{line}"
                );
            } else {
                let actual = Biquad(std::array::from_fn(|i| value(i + 3))).response(rate, value(2));
                assert_eq!(
                    [actual.real.to_bits(), actual.imaginary.to_bits()],
                    [bits(8), bits(9)],
                    "{line}"
                );
            }
        }
    }
}

//! Versioned coefficient mathematics. The experimental portable profile uses
//! pinned Rust algorithms; the default profile retains host standard math.
//! Explicit aliases preserve operation grouping and float32 argument width.

#[cfg(feature = "portable-math")]
pub const PROFILE: &str = "software-wide-v1/libm-0.2.16";
#[cfg(not(feature = "portable-math"))]
pub const PROFILE: &str = "host-std-v1";

pub(crate) trait EngineFloat: Sized {
    fn engine_canonical(self) -> Self;
    fn engine_powf(self, exponent: Self) -> Self;
    fn engine_exp(self) -> Self;
    fn engine_ln(self) -> Self;
    fn engine_log10(self) -> Self;
    fn engine_sin(self) -> Self;
    fn engine_cos(self) -> Self;
    fn engine_tan(self) -> Self;
    fn engine_sqrt(self) -> Self;
}

macro_rules! unary {
    ($name:ident, $host:ident, $soft:ident, $ty:ty) => {
        fn $name(self) -> Self {
            #[cfg(feature = "portable-math")]
            {
                (libm::$soft(self as f64) as Self).engine_canonical()
            }
            #[cfg(not(feature = "portable-math"))]
            {
                <$ty>::$host(self)
            }
        }
    };
}

macro_rules! implement {
    ($ty:ty, $nan:expr) => {
        impl EngineFloat for $ty {
            fn engine_canonical(self) -> Self {
                #[cfg(feature = "portable-math")]
                {
                    if self.is_nan() {
                        <$ty>::from_bits($nan)
                    } else {
                        self
                    }
                }
                #[cfg(not(feature = "portable-math"))]
                {
                    self
                }
            }
            fn engine_powf(self, exponent: Self) -> Self {
                #[cfg(feature = "portable-math")]
                {
                    (libm::pow(self as f64, exponent as f64) as Self).engine_canonical()
                }
                #[cfg(not(feature = "portable-math"))]
                {
                    <$ty>::powf(self, exponent)
                }
            }
            unary!(engine_exp, exp, exp, $ty);
            unary!(engine_ln, ln, log, $ty);
            unary!(engine_log10, log10, log10, $ty);
            unary!(engine_sin, sin, sin, $ty);
            unary!(engine_cos, cos, cos, $ty);
            unary!(engine_tan, tan, tan, $ty);
            unary!(engine_sqrt, sqrt, sqrt, $ty);
        }
    };
}

implement!(f32, 0xffc0_0000);
implement!(f64, 0xfff8_0000_0000_0000);

// Reference data comes from the original instructions with the selected math
// adapter. Preserve both profiles' exact expectations; never add tolerances.
#[cfg(test)]
macro_rules! fixture {
    ($include:ident, $name:literal) => {{
        #[cfg(feature = "portable-math")]
        {
            $include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/software-wide-v1/",
                $name
            ))
        }
        #[cfg(not(feature = "portable-math"))]
        {
            $include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/",
                $name
            ))
        }
    }};
}
#[cfg(test)]
pub(crate) use fixture;

#[cfg(test)]
mod tests {
    use super::EngineFloat;

    #[test]
    fn nan_representation_is_explicit_and_finite_bits_are_unchanged() {
        for bits in [0, 0x8000_0000, 0x3f80_0000, 0x7f80_0000, 0xff80_0000] {
            assert_eq!(f32::from_bits(bits).engine_canonical().to_bits(), bits);
        }
        for bits in [0, 1 << 63, 0x3ff0_0000_0000_0000, 0x7ff0_0000_0000_0000] {
            assert_eq!(f64::from_bits(bits).engine_canonical().to_bits(), bits);
        }
        let nan32 = f32::from_bits(0x7fc0_0042);
        let nan64 = f64::from_bits(0xfff8_0000_0000_0042);
        #[cfg(feature = "portable-math")]
        {
            assert_eq!(nan32.engine_canonical().to_bits(), 0xffc0_0000);
            assert_eq!(nan64.engine_canonical().to_bits(), 0xfff8_0000_0000_0000);
            assert_eq!(nan32.engine_exp().to_bits(), 0xffc0_0000);
            assert_eq!((-1.0_f64).engine_ln().to_bits(), 0xfff8_0000_0000_0000);
        }
        #[cfg(not(feature = "portable-math"))]
        {
            assert_eq!(nan32.engine_canonical().to_bits(), nan32.to_bits());
            assert_eq!(nan64.engine_canonical().to_bits(), nan64.to_bits());
        }
    }
}

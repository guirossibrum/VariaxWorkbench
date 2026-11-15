//! Versioned coefficient mathematics. The experimental portable profile uses
//! pinned Rust algorithms; the default profile retains host standard math.
//! Explicit aliases preserve operation grouping and float32 argument width.

#[cfg(feature = "portable-math")]
pub const PROFILE: &str = "software-wide-v1/libm-0.2.16";
#[cfg(not(feature = "portable-math"))]
pub const PROFILE: &str = "host-std-v1";

pub(crate) trait EngineFloat: Sized {
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
                libm::$soft(self as f64) as Self
            }
            #[cfg(not(feature = "portable-math"))]
            {
                <$ty>::$host(self)
            }
        }
    };
}

macro_rules! implement {
    ($ty:ty) => {
        impl EngineFloat for $ty {
            fn engine_powf(self, exponent: Self) -> Self {
                #[cfg(feature = "portable-math")]
                {
                    libm::pow(self as f64, exponent as f64) as Self
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

implement!(f32);
implement!(f64);

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

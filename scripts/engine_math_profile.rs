//! Injected only into an ignored, generated engine-analysis source copy.
//! Explicit aliases preserve operation grouping and float32 argument width.

pub(crate) trait AuditMath: Sized {
    fn audit_powf(self, exponent: Self) -> Self;
    fn audit_exp(self) -> Self;
    fn audit_ln(self) -> Self;
    fn audit_log10(self) -> Self;
    fn audit_sin(self) -> Self;
    fn audit_cos(self) -> Self;
    fn audit_tan(self) -> Self;
    fn audit_sqrt(self) -> Self;
}

macro_rules! unary {
    ($name:ident, $host:ident, $soft:ident, $ty:ty) => {
        fn $name(self) -> Self {
            #[cfg(feature = "research-wide-math")]
            {
                libm::$soft(self as f64) as Self
            }
            #[cfg(not(feature = "research-wide-math"))]
            {
                <$ty>::$host(self)
            }
        }
    };
}

macro_rules! implement {
    ($ty:ty) => {
        impl AuditMath for $ty {
            fn audit_powf(self, exponent: Self) -> Self {
                #[cfg(feature = "research-wide-math")]
                {
                    libm::pow(self as f64, exponent as f64) as Self
                }
                #[cfg(not(feature = "research-wide-math"))]
                {
                    <$ty>::powf(self, exponent)
                }
            }
            unary!(audit_exp, exp, exp, $ty);
            unary!(audit_ln, ln, log, $ty);
            unary!(audit_log10, log10, log10, $ty);
            unary!(audit_sin, sin, sin, $ty);
            unary!(audit_cos, cos, cos, $ty);
            unary!(audit_tan, tan, tan, $ty);
            unary!(audit_sqrt, sqrt, sqrt, $ty);
        }
    };
}

implement!(f32);
implement!(f64);

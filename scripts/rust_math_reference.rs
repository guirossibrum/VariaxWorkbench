// Offline FFI adapter; the product does not depend on this helper or libm crate.
#[unsafe(no_mangle)]
pub extern "C" fn audit_rust_math(operation: i32, x: f64, y: f64, single: i32) -> f64 {
    if single != 0 {
        let (x, y) = (x as f32, y as f32);
        f64::from(match operation {
            0 => libm::sinf(x),
            1 => libm::cosf(x),
            2 => libm::tanf(x),
            3 => libm::expf(x),
            4 => libm::logf(x),
            5 => libm::log10f(x),
            6 => libm::powf(x, y),
            7 => libm::atan2f(x, y),
            _ => f32::NAN,
        })
    } else {
        match operation {
            0 => libm::sin(x),
            1 => libm::cos(x),
            2 => libm::tan(x),
            3 => libm::exp(x),
            4 => libm::log(x),
            5 => libm::log10(x),
            6 => libm::pow(x, y),
            7 => libm::atan2(x, y),
            _ => f64::NAN,
        }
    }
}

//! Isolated #275 owner probe. Running the test harness never opens either input.
#![deny(unsafe_code)]

#[cfg(all(target_os = "linux", not(test)))]
#[allow(unsafe_code)]
#[path = "../notice_probe/interrupt.rs"]
mod interrupt;
#[cfg(target_os = "linux")]
#[path = "../passive_ingress.rs"]
mod probe;
#[cfg(target_os = "linux")]
#[path = "../notice_probe/pump.rs"]
mod pump;

#[cfg(all(target_os = "linux", not(test)))]
fn main() -> std::process::ExitCode {
    match probe::run(std::env::args().skip(1)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("INCOMPLETE: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(not(target_os = "linux"), not(test)))]
fn main() -> std::process::ExitCode {
    eprintln!("The paired ingress probe requires Linux.");
    std::process::ExitCode::FAILURE
}

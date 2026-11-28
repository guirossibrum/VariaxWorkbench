use std::time::Duration;

/// Read the ALSA input without waiting, then give the USB input its bounded poll.
pub(crate) fn poll_pair<T>(
    podxt: impl FnOnce(Duration) -> Result<(), String>,
    variax: impl FnOnce(Duration) -> Result<T, String>,
    wait: Duration,
) -> Result<T, String> {
    podxt(Duration::ZERO)?;
    variax(wait)
}

//! Inactive operating-system contract adapters.
//!
//! These modules translate already supplied native negotiation/period data
//! into the common capture session. They do not enumerate endpoints, open or
//! initialize streams, load drivers, or provide an application start route.

#[cfg(target_os = "linux")]
pub mod linux_alsa;

/// Windows WASAPI capture cannot be implemented from the source contracts in
/// this checkout. See `docs/audio-capture-session.md` for the exact blocker.
#[cfg(target_os = "windows")]
pub const WINDOWS_ADAPTER_BLOCKER: &str = "WASAPI capture ABI/format ownership contract and matching installed Windows SDK headers are not present in the admitted inputs.";

/// macOS Core Audio capture cannot be implemented from the source contracts in
/// this checkout. See `docs/audio-capture-session.md` for the exact blocker.
#[cfg(target_os = "macos")]
pub const MACOS_ADAPTER_BLOCKER: &str = "Core Audio callback/format/lifetime contract and matching installed macOS SDK headers are not present in the admitted inputs.";

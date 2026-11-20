//! Inactive operating-system contract adapters.
//!
//! These modules translate already supplied native negotiation/period data
//! into the common capture session. They do not enumerate endpoints, open or
//! initialize streams, load drivers, or provide an application start route.

#[cfg(target_os = "linux")]
pub mod linux_alsa;

/// Rust-owned software model of an ASIO-style callback boundary. This is not
/// a Windows adapter and does not load or call ASIO, Windows, or device APIs.
pub mod offline_asio;

/// Windows ASIO capture cannot be implemented from the admitted contracts in
/// this checkout. See `docs/audio-capture-session.md` for the exact blocker.
#[cfg(target_os = "windows")]
pub const WINDOWS_ADAPTER_BLOCKER: &str = "An offline software-only ASIO-style callback model exists, but the complete versioned native ABI, source-qualified physical input channel/rate contract, interface admission, and concrete Windows capture adapter remain unavailable.";

/// macOS Core Audio capture cannot be implemented from the source contracts in
/// this checkout. See `docs/audio-capture-session.md` for the exact blocker.
#[cfg(target_os = "macos")]
pub const MACOS_ADAPTER_BLOCKER: &str = "Core Audio callback/format/lifetime contract and matching installed macOS SDK headers are not present in the admitted inputs.";

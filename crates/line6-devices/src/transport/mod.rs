//! Thin, replaceable links. Each implements [`crate::session::Link`] for one
//! [`crate::session::Path`]. Tests never construct these.

#[cfg(target_os = "linux")]
pub mod rawmidi;
#[cfg(feature = "usb")]
pub mod usb;

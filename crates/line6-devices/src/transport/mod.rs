//! Thin, replaceable links. Each implements [`crate::session::Link`] for one
//! [`crate::session::Path`]. Tests use in-memory links or local fixture descriptors.

#[cfg(target_os = "linux")]
pub mod rawmidi;
#[cfg(feature = "usb")]
pub mod usb;

use crate::session::Path;
use std::time::Duration;

/// Where one control interface lives and how it frames writes.
#[derive(Clone, Copy, Debug)]
pub struct InterfaceSpec {
    pub vendor: u16,
    pub product: u16,
    pub interface: u8,
    pub altsetting: u8,
    pub endpoint_out: u8,
    pub endpoint_in: u8,
    pub packet: usize,
    pub chunk_gap: Duration,
    pub path: Path,
}

/// How a device's kernel driver exposes its Linux ALSA MIDI port.
#[derive(Clone, Copy, Debug)]
pub struct RawMidiSpec {
    pub card_id: &'static str,
    pub device: u32,
    pub path: Path,
}

impl RawMidiSpec {
    /// `card` is the index of the matching card in the OS metadata.
    pub fn device_node(self, card: u32) -> std::path::PathBuf {
        format!("/dev/snd/midiC{card}D{}", self.device).into()
    }
}

//! Checked capture on a fresh, borrowed macOS interface-0 handle.

use super::{fresh::capture_fresh, provenance::FinalizedCapture};
use crate::{
    family::podxt::StoredTone,
    gate::GatePolicy,
    session::{Link, Log},
};

#[cfg(all(feature = "usb", target_os = "macos"))]
use crate::{
    device_lock::DeviceLock,
    transport::usb::{UsbInterfaceLink, macos, macos_inventory},
};

/// Capture the selected PODxt Live using a fresh macOS USB interface-0 handle.
/// The caller supplies the selected attachment and its IOKit registry ID,
/// the held rig lock, and the existing backup policy. The adapter rechecks
/// the registry generation immediately before open. The sweep and checked
/// release use one handle.
#[cfg(all(feature = "usb", target_os = "macos"))]
pub fn capture_macos_usb_library(
    rig: &DeviceLock,
    registry_id: u64,
    attachment: &macos::Attachment,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    progress: impl FnMut(&StoredTone),
) -> Result<FinalizedCapture, String> {
    capture_fresh_usb(
        || macos_inventory::verify_candidate(rig, registry_id, attachment),
        || {
            UsbInterfaceLink::open_attachment_borrowed(
                macos::PODXT_MIDI,
                &attachment.path,
                attachment.address,
                rig,
            )
        },
        UsbInterfaceLink::finish,
        policy,
        log,
        progress,
    )
}

pub(super) fn capture_fresh_usb<L: Link>(
    verify_candidate: impl FnOnce() -> Result<(), String>,
    open: impl FnOnce() -> Result<L, String>,
    finish: fn(&mut L) -> Result<(), String>,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    progress: impl FnMut(&StoredTone),
) -> Result<FinalizedCapture, String> {
    capture_fresh(
        || {
            verify_candidate()?;
            open()
        },
        |mut link| finish(&mut link),
        &|| false,
        policy,
        log,
        progress,
    )
}

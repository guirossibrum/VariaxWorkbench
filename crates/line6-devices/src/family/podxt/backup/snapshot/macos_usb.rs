//! Checked capture on a fresh, borrowed macOS interface-0 handle.

use super::provenance::{CaptureLink, FinalizedCapture, capture_library, sealed};
use crate::{
    family::podxt::StoredTone,
    gate::{GatePolicy, Operation},
    session::{Link, Log, Path},
};
use std::time::Duration;

#[cfg(all(feature = "usb", target_os = "macos"))]
use crate::{
    device_lock::DeviceLock,
    transport::usb::{UsbInterfaceLink, macos},
};

/// Capture the selected PODxt Live using a fresh macOS USB interface-0 handle.
/// The caller supplies the verified attachment, its held rig lock, and the
/// existing backup policy. The sweep and checked release use one handle.
#[cfg(all(feature = "usb", target_os = "macos"))]
pub fn capture_macos_usb_library(
    rig: &DeviceLock,
    attachment: &macos::Attachment,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    progress: impl FnMut(&StoredTone),
) -> Result<FinalizedCapture, String> {
    capture_fresh_usb(
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
    open: impl FnOnce() -> Result<L, String>,
    finish: fn(&mut L) -> Result<(), String>,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    progress: impl FnMut(&StoredTone),
) -> Result<FinalizedCapture, String> {
    for operation in [
        Operation::PodxtIdentity,
        Operation::PodxtRead,
        Operation::PodxtLibraryRead,
    ] {
        if !policy.allows(operation) {
            return Err(format!(
                "PODxt capture needs {operation:?}, which this build does not allow."
            ));
        }
    }
    let link = open()?;
    capture_library(CheckedUsbCapture { link, finish }, policy, log, progress)
}

struct CheckedUsbCapture<L> {
    link: L,
    finish: fn(&mut L) -> Result<(), String>,
}

impl<L: Link> Link for CheckedUsbCapture<L> {
    fn path(&self) -> Path {
        self.link.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.link.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.link.read(wait)
    }

    fn elapsed(&self) -> Duration {
        self.link.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.link.pause(duration);
    }
}

impl<L> sealed::Sealed for CheckedUsbCapture<L> {}

impl<L: Link> CaptureLink for CheckedUsbCapture<L> {
    fn finish_capture(mut self) -> Result<(), String> {
        (self.finish)(&mut self.link)
    }
}

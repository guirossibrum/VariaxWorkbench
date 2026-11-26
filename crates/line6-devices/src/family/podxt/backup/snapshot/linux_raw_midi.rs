//! Checked capture on a fresh Linux ALSA raw MIDI descriptor.

use super::{fresh::capture_fresh, provenance::FinalizedCapture};
use crate::{
    device_lock::DeviceLock,
    family::podxt::StoredTone,
    gate::GatePolicy,
    session::Log,
    transport::rawmidi::{RawMidiAttachment, ScopedRawMidiLink},
};

/// The verified Linux selection and the rig lock held since that selection.
///
/// ```compile_fail,E0505
/// use line6_devices::{device_lock::DeviceLock, family::podxt::backup::LinuxSnapshotAttachment};
/// fn unlock_after_selection(rig: DeviceLock) {
///     let selected = LinuxSnapshotAttachment::select_podxt_live(&rig, "8-2.1", 11).unwrap();
///     drop(rig);
///     drop(selected);
/// }
/// ```
pub struct LinuxSnapshotAttachment<'rig> {
    selected: RawMidiAttachment,
    rig: &'rig DeviceLock,
}

impl<'rig> LinuxSnapshotAttachment<'rig> {
    /// Record the generation at verified selection, under the caller's
    /// existing discovery gate. Retain this value until capture.
    pub fn select_podxt_live(
        rig: &'rig DeviceLock,
        usb_path: &str,
        address: u8,
    ) -> Result<Self, String> {
        let selected = RawMidiAttachment::select_podxt_live(rig, usb_path, address)?;
        Ok(Self { selected, rig })
    }

    /// Recheck and open the retained attachment under the same rig lock.
    /// Capture proof requires a complete sweep and checked close of that link.
    pub fn capture_library(
        &self,
        policy: &dyn GatePolicy,
        log: &mut dyn Log,
        cancelled: impl Fn() -> bool,
        progress: impl FnMut(&StoredTone),
    ) -> Result<FinalizedCapture, String> {
        capture_fresh(
            || {
                ScopedRawMidiLink::open_attachment_borrowed(&self.selected, self.rig, &cancelled)
                    .map_err(|error| error.to_string())
            },
            |link| link.finish().map_err(|error| error.to_string()),
            &cancelled,
            policy,
            log,
            progress,
        )
    }
}

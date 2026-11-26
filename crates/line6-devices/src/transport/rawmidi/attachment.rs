//! Attachment-bound Linux raw MIDI for a fresh, locked capture.

mod inventory;
mod native;
#[cfg(test)]
mod tests;

use super::{RawMidiLink, open_port};
use crate::{
    device_lock::DeviceLock,
    family::podxt::transport::PODXT_LIVE_MIDI,
    session::{Link, Path},
};
use inventory::{AttachmentIdentity, NodeIdentity, probe};
use std::fs::File;
use std::{
    fmt, io,
    os::fd::OwnedFd,
    path::PathBuf,
    time::{Duration, Instant},
};

/// The OS identity of an already selected PODxt Live and its unique ALSA MIDI
/// node. Keep this selection when handing off capture. Refreshing it just
/// before opening would discard the generation check against that selection.
#[derive(Debug)]
pub struct RawMidiAttachment {
    usb_path: String,
    address: u8,
    identity: AttachmentIdentity,
}

impl RawMidiAttachment {
    /// Record the selected USB topology path and enumeration address under
    /// the rig lock. The caller supplies its existing verified selection and
    /// runs this under the OS discovery gate. This reads metadata only.
    pub fn select_podxt_live(
        _rig: &DeviceLock,
        usb_path: &str,
        address: u8,
    ) -> Result<Self, String> {
        let identity = probe(usb_path, address)?;
        Ok(Self {
            usb_path: usb_path.into(),
            address,
            identity,
        })
    }
}

/// The stage that refused the link before any capture traffic.
#[derive(Debug)]
pub enum RawMidiOpenFailure {
    Cancelled,
    Attachment(String),
    Open(io::Error),
    Descriptor(io::Error),
    WrongDescriptor,
}

/// A failed open or verification, including independent cleanup evidence when
/// a descriptor had already opened and its checked close also failed.
#[derive(Debug)]
pub struct RawMidiOpenError {
    pub device: PathBuf,
    pub failure: RawMidiOpenFailure,
    pub cleanup: Option<RawMidiCloseError>,
}

impl fmt::Display for RawMidiOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Raw MIDI {}: ", self.device.display())?;
        match &self.failure {
            RawMidiOpenFailure::Cancelled => f.write_str("opening cancelled")?,
            RawMidiOpenFailure::Attachment(error) => write!(f, "attachment check failed: {error}")?,
            RawMidiOpenFailure::Open(error) => write!(f, "open failed: {error}")?,
            RawMidiOpenFailure::Descriptor(error) => write!(f, "descriptor check failed: {error}")?,
            RawMidiOpenFailure::WrongDescriptor => {
                f.write_str("opened descriptor does not match the selected ALSA MIDI node")?
            }
        }
        if let Some(cleanup) = &self.cleanup {
            write!(f, ". {cleanup}")?;
        }
        Ok(())
    }
}

impl std::error::Error for RawMidiOpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.failure {
            RawMidiOpenFailure::Open(error) | RawMidiOpenFailure::Descriptor(error) => Some(error),
            _ => None,
        }
    }
}

/// Linux close reported failure. The underlying errno is retained, including
/// EINTR. The descriptor has been relinquished and must never be retried.
#[derive(Debug)]
pub struct RawMidiCloseError {
    pub device: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for RawMidiCloseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Raw MIDI {} checked close failed: {}",
            self.device.display(),
            self.source
        )
    }
}

impl std::error::Error for RawMidiCloseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// A fresh, verified read/write raw MIDI link borrowing the rig lock through
/// finalization. Call [Self::finish] after the sweep, including on read failure
/// or cancellation. Implicit drop releases the file without checked evidence.
///
/// ```compile_fail,E0505
/// use line6_devices::{device_lock::DeviceLock, transport::rawmidi::{RawMidiAttachment, ScopedRawMidiLink}};
/// fn unlock_early(rig: DeviceLock, selected: &RawMidiAttachment) {
///     let link = ScopedRawMidiLink::open_attachment_borrowed(selected, &rig, || false).unwrap();
///     drop(rig);
///     link.finish().unwrap();
/// }
/// ```
///
/// ```compile_fail,E0505
/// use line6_devices::{device_lock::DeviceLock, transport::rawmidi::{RawMidiAttachment, ScopedRawMidiLink}};
/// fn unlock_before_drop(rig: DeviceLock, selected: &RawMidiAttachment) {
///     let _link = ScopedRawMidiLink::open_attachment_borrowed(selected, &rig, || false).unwrap();
///     drop(rig);
/// }
/// ```
///
/// ```compile_fail,E0382
/// use line6_devices::transport::rawmidi::ScopedRawMidiLink;
/// fn retry(link: ScopedRawMidiLink<'_>) {
///     let _ = link.finish();
///     let _ = link.finish();
/// }
/// ```
#[must_use = "the link must be finished to establish checked cleanup"]
pub struct ScopedRawMidiLink<'rig> {
    link: Option<RawMidiLink>,
    _rig: &'rig DeviceLock,
}

impl<'rig> ScopedRawMidiLink<'rig> {
    /// Recheck the recorded selection under the rig lock, open its exact node
    /// once, and verify the descriptor and attachment again before returning a
    /// usable MIDI link. The caller supplies the existing discovery and backup
    /// gates, and checks cancellation during the capture itself.
    pub fn open_attachment_borrowed(
        selected: &RawMidiAttachment,
        rig: &'rig DeviceLock,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Self, RawMidiOpenError> {
        Self::open_with(
            selected,
            rig,
            cancelled,
            || probe(&selected.usb_path, selected.address),
            |node| open_port(node, libc::O_NOFOLLOW),
            |port| NodeIdentity::from_metadata(&port.metadata()?),
            native::close_once,
        )
    }

    fn open_with(
        selected: &RawMidiAttachment,
        rig: &'rig DeviceLock,
        mut cancelled: impl FnMut() -> bool,
        mut probe: impl FnMut() -> Result<AttachmentIdentity, String>,
        open: impl FnOnce(&std::path::Path) -> io::Result<File>,
        inspect: impl FnOnce(&File) -> io::Result<NodeIdentity>,
        close: impl FnOnce(OwnedFd) -> io::Result<()>,
    ) -> Result<Self, RawMidiOpenError> {
        use RawMidiOpenFailure::{Attachment, Cancelled, Descriptor, Open, WrongDescriptor};
        let device = &selected.identity.device;
        let error = |failure, cleanup| RawMidiOpenError {
            device: device.clone(),
            failure,
            cleanup,
        };
        let mut verify = || {
            let observed = probe().map_err(Attachment)?;
            if observed == selected.identity {
                Ok(())
            } else {
                Err(Attachment(format!(
                    "selected PODxt Live USB {} address {} or its ALSA MIDI node changed",
                    selected.usb_path, selected.address
                )))
            }
        };
        if cancelled() {
            return Err(error(Cancelled, None));
        }
        verify().map_err(|failure| error(failure, None))?;
        if cancelled() {
            return Err(error(Cancelled, None));
        }
        let port = open(device).map_err(|source| error(Open(source), None))?;
        let verified = (|| {
            if cancelled() {
                return Err(Cancelled);
            }
            if inspect(&port).map_err(Descriptor)? != selected.identity.node {
                return Err(WrongDescriptor);
            }
            verify()?;
            if cancelled() {
                return Err(Cancelled);
            }
            Ok(())
        })();
        if let Err(failure) = verified {
            let cleanup = checked_close(device.clone(), port, close).err();
            return Err(error(failure, cleanup));
        }

        Ok(Self {
            link: Some(RawMidiLink {
                port,
                opened: Instant::now(),
                path: PODXT_LIVE_MIDI.path,
                device: device.clone(),
            }),
            _rig: rig,
        })
    }

    /// Close the descriptor used by this link exactly once. A failure,
    /// including an interrupted close, is terminal for capture provenance.
    /// Success reports Linux's close result. The capture still needs a
    /// complete sweep and its own playing-tone checks.
    pub fn finish(self) -> Result<(), RawMidiCloseError> {
        self.finish_with(native::close_once)
    }

    fn finish_with(
        mut self,
        close: impl FnOnce(OwnedFd) -> io::Result<()>,
    ) -> Result<(), RawMidiCloseError> {
        let link = self.link.take().expect("scoped link is open");
        checked_close(link.device, link.port, close)
    }
}

impl Drop for ScopedRawMidiLink<'_> {
    fn drop(&mut self) {
        // Fallback while the rig is still borrowed. Checked close has already
        // taken the file out, even if it returned an error.
        drop(self.link.take());
    }
}

impl Link for ScopedRawMidiLink<'_> {
    fn path(&self) -> Path {
        PODXT_LIVE_MIDI.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.link
            .as_mut()
            .expect("scoped link is open")
            .write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.link.as_mut().expect("scoped link is open").read(wait)
    }

    fn elapsed(&self) -> Duration {
        self.link.as_ref().expect("scoped link is open").elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.link
            .as_mut()
            .expect("scoped link is open")
            .pause(duration);
    }
}

fn checked_close(
    device: PathBuf,
    port: File,
    close: impl FnOnce(OwnedFd) -> io::Result<()>,
) -> Result<(), RawMidiCloseError> {
    close(port.into()).map_err(|source| RawMidiCloseError { device, source })
}

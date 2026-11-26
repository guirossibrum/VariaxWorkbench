//! Same-transaction capture boundary.
//!
//! Platform integration contract
//!
//! - The caller enters through the existing gated workflow and selects the
//!   verified attachment under its rig lock. Both adapters borrow that
//!   lock through checked finalization and handle drop.
//! - On macOS use a fresh borrowed interface-0 USB handle and call its
//!   fallible `finish()` exactly once before dropping it. Do not wrap an
//!   already-finalized handle because repeated `finish()` can return success.
//! - On Linux retain the raw MIDI attachment token from selection, open a
//!   fresh scoped link against that token, and consume it with checked close.
//! - Capture with the existing read-only policy. Stage from the immutable
//!   `FinalizedCapture::library`, validate the staged directory, then call
//!   `bind`. A receipt proves that this finalized sweep agrees with the
//!   report's bytes. V1 files alone do not establish where they originated.
//!   Publication and offline startup policy are separate work.

use super::SnapshotReport;
use crate::{
    family::podxt::{BankKind, Library, Location, StoredTone, check_live_identity, read_library},
    gate::GatePolicy,
    identity::Identity,
    session::{Link, Log, Path, Session},
};

pub(super) mod sealed {
    pub trait Sealed {}
}

/// A fresh, attachment-scoped PODxt link that owns or borrows the transaction's
/// rig lock through finalization. Only trusted adapters can implement it. The
/// adapter must finalize this handle once, then drop it before losing the lock.
///
/// ```compile_fail,E0277
/// use line6_devices::{family::podxt::backup::CaptureLink, session::{Link, Path}};
/// use std::time::Duration;
///
/// struct CallerLink;
/// impl Link for CallerLink {
///     fn path(&self) -> Path { Path::PodxtMidi }
///     fn write(&mut self, _: &[u8]) -> Result<(), String> { Ok(()) }
///     fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, String> { Ok(None) }
///     fn elapsed(&self) -> Duration { Duration::ZERO }
///     fn pause(&mut self, _: Duration) {}
/// }
/// impl CaptureLink for CallerLink {
///     fn finish_capture(self) -> Result<(), String> { Ok(()) }
/// }
/// ```
pub trait CaptureLink: Link + sealed::Sealed {
    fn finish_capture(self) -> Result<(), String>;
}

/// The exact replies from a complete sweep whose own link finalized.
/// A snapshot report must match these replies before it can get a receipt.
pub struct FinalizedCapture {
    library: Library,
}

impl FinalizedCapture {
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Bind this capture to one validated manifest. The receipt carries no
    /// live-device permission and does not publish a cache generation.
    pub fn bind(self, report: &SnapshotReport) -> Result<CaptureReceipt, String> {
        report.check_capture_matches(&self.library)?;
        Ok(CaptureReceipt {
            manifest_sha256: report.manifest_sha256().to_owned(),
        })
    }
}

pub struct CaptureReceipt {
    manifest_sha256: String,
}

impl CaptureReceipt {
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
}

/// Run the existing read-only library sweep and finalize the same scoped
/// link, including on failure. No caller can pass a preexisting sweep or a
/// cleanup result into this boundary.
pub fn capture_library<L: CaptureLink>(
    mut link: L,
    policy: &dyn GatePolicy,
    log: &mut dyn Log,
    progress: impl FnMut(&StoredTone),
) -> Result<FinalizedCapture, String> {
    let read = if link.path() == Path::PodxtMidi {
        let mut session = Session::new(&mut link, policy, log);
        session.log_packets = false;
        read_library(&mut session, &BankKind::ALL, progress)
    } else {
        Err("PODxt capture requires the PODxt MIDI link.".into())
    };
    let checked = read.and_then(|library| {
        check_complete(&library)?;
        Ok(library)
    });
    match (checked, link.finish_capture()) {
        (Ok(library), Ok(())) => Ok(FinalizedCapture { library }),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("PODxt capture link cleanup failed: {error}")),
        (Err(read), Err(finish)) => Err(format!(
            "{read} PODxt capture link cleanup also failed: {finish}"
        )),
    }
}

fn check_complete(library: &Library) -> Result<(), String> {
    if let Some(reason) = &library.stopped {
        return Err(format!(
            "The PODxt capture stopped before completion: {reason}."
        ));
    }
    if library.entries.len() != Location::all(&BankKind::ALL).count() {
        return Err("The PODxt capture stopped before completion.".into());
    }
    let identity =
        Identity::parse(&library.identity_reply).ok_or("Invalid PODxt capture identity.")?;
    check_live_identity(&identity)?;
    let after = library
        .after
        .as_ref()
        .map_err(|error| format!("PODxt closing read failed: {error}"))?;
    if !library.before.same_tone(after) {
        return Err("The PODxt playing tone changed during the capture.".into());
    }
    Ok(())
}

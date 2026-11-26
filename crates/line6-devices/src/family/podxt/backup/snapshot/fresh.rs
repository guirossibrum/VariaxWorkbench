use super::provenance::{CaptureLink, FinalizedCapture, capture_library, sealed};
use crate::{
    family::podxt::StoredTone,
    gate::{GatePolicy, Operation},
    session::{Link, Log, Path},
};
use std::time::Duration;

pub(super) fn capture_fresh<L: Link>(
    open: impl FnOnce() -> Result<L, String>,
    finish: impl FnOnce(L) -> Result<(), String>,
    cancelled: &dyn Fn() -> bool,
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
    check_cancel(cancelled)?;
    let link = open()?;
    let capture = capture_library(
        FreshCapture {
            link,
            finish,
            cancelled,
        },
        policy,
        log,
        progress,
    )?;
    check_cancel(cancelled)?;
    Ok(capture)
}

fn check_cancel(cancelled: &dyn Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        return Err("PODxt capture cancelled.".into());
    }
    Ok(())
}

struct FreshCapture<'a, L, F> {
    link: L,
    finish: F,
    cancelled: &'a dyn Fn() -> bool,
}

impl<L: Link, F> Link for FreshCapture<'_, L, F> {
    fn path(&self) -> Path {
        self.link.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        check_cancel(self.cancelled)?;
        self.link.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        check_cancel(self.cancelled)?;
        let reply = self.link.read(wait)?;
        check_cancel(self.cancelled)?;
        Ok(reply)
    }

    fn elapsed(&self) -> Duration {
        self.link.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.link.pause(duration);
    }
}

impl<L, F> sealed::Sealed for FreshCapture<'_, L, F> {}

impl<L: Link, F: FnOnce(L) -> Result<(), String>> CaptureLink for FreshCapture<'_, L, F> {
    fn finish_capture(self) -> Result<(), String> {
        (self.finish)(self.link)
    }
}

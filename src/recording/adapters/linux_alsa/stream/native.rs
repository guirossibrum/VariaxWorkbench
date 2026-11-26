//! Inactive libasound adapter for an independently verified, prepared PCM.

use std::ffi::c_void;
use std::ptr::NonNull;

use crate::recording::capture::ValidatedPlan;

use super::{
    AlsaCaptureParams, AlsaCapturePcm, AlsaCaptureStream, AlsaStartError, VerifiedAlsaRoute,
};

#[allow(unsafe_code)]
mod ffi;
pub use ffi::Libasound;

/// This release has no issuer. A reviewed owner procedure must supply the
/// authority before any native PCM can enter the adapter.
pub enum CaptureAuthorization {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PcmAddress {
    card: u32,
    device: u32,
    subdevice: u32,
}

/// A trusted observer must verify the driver and physical attachment of the
/// exact open handle. The ALSA numeric address is an additional consistency
/// check. Addresses can be reused after unplugging.
pub struct PreparedPcmEvidence {
    observed_handle: Handle,
    address: PcmAddress,
    route: VerifiedAlsaRoute,
}

type Handle = NonNull<c_void>;

const EAGAIN: i32 = 11;
const ENODEV: i32 = 19;

trait NativeCalls {
    fn prepared_capture_address(&self, handle: Handle) -> Option<PcmAddress>;
    fn current_params(&self, handle: Handle) -> Result<AlsaCaptureParams, i32>;
    fn nonblock(&self, handle: Handle) -> Result<(), i32>;
    fn start(&self, handle: Handle) -> Result<(), i32>;
    fn readi(&self, handle: Handle, frames: usize, buffer: &mut [u8]) -> isize;
    fn drop_pcm(&self, handle: Handle) -> Result<(), i32>;
    fn close(&self, handle: Handle) -> Result<(), i32>;
}

fn start_prepared(
    calls: impl NativeCalls,
    handle: Handle,
    evidence: PreparedPcmEvidence,
    plan: ValidatedPlan,
    generation: u64,
) -> Result<AlsaCaptureStream<impl AlsaCapturePcm>, AlsaStartError> {
    AlsaCaptureStream::start(
        NativePcm {
            calls,
            handle: Some(handle),
            evidence,
            start_attempted: false,
        },
        plan,
        generation,
    )
}

struct NativePcm<C: NativeCalls> {
    calls: C,
    handle: Option<Handle>,
    evidence: PreparedPcmEvidence,
    start_attempted: bool,
}

impl<C: NativeCalls> NativePcm<C> {
    fn handle(&self) -> Handle {
        self.handle.expect("PCM handle owned until close")
    }

    fn close_once(&mut self) -> Result<(), i32> {
        self.handle
            .take()
            .map_or(Ok(()), |handle| self.calls.close(handle))
    }
}

impl<C: NativeCalls> AlsaCapturePcm for NativePcm<C> {
    fn verified_route(&mut self) -> Option<VerifiedAlsaRoute> {
        let handle = self.handle();
        let mut route = self.evidence.route;
        if route.driver == "snd_usb_pod" {
            route.driver = "snd-usb-pod";
        }
        (handle == self.evidence.observed_handle
            && (
                self.evidence.address.device,
                self.evidence.address.subdevice,
            ) == (0, 0)
            && self.calls.prepared_capture_address(handle) == Some(self.evidence.address))
        .then_some(route)
    }

    fn negotiated_params(&mut self) -> Result<AlsaCaptureParams, i32> {
        self.calls.current_params(self.handle())
    }

    fn start(&mut self) -> Result<(), i32> {
        if self.verified_route().is_none() {
            return Err(-ENODEV);
        }
        let handle = self.handle();
        self.calls.nonblock(handle)?;
        self.start_attempted = true;
        self.calls.start(handle)
    }

    fn read_interleaved(&mut self, frames: usize, buffer: &mut [u8]) -> isize {
        match self.calls.readi(self.handle(), frames, buffer) {
            result if result == -(EAGAIN as isize) => 0,
            result => result,
        }
    }

    fn drop_stream(&mut self) -> Result<(), i32> {
        if self.start_attempted {
            self.calls.drop_pcm(self.handle())
        } else {
            Ok(())
        }
    }

    fn close(mut self) -> Result<(), i32> {
        self.close_once()
    }
}

impl<C: NativeCalls> Drop for NativePcm<C> {
    fn drop(&mut self) {
        let _ = self.close_once();
    }
}

#[cfg(test)]
mod tests;

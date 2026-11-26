//! Inactive capture lifecycle for a platform-supplied ALSA PCM.
//!
//! The optional native platform module adopts an explicitly supplied PCM.
//! The application has no production initializer for it.

use std::marker::PhantomData;
use std::rc::Rc;

use crate::pod_audio::{self, RecordRoute};
use crate::recording::capture::{
    CaptureError, CaptureSession, ChannelMap, PendingCaptureEvent, ValidatedPlan,
};
use crate::rig::{ProductProfile, audio};

use super::{AlsaAdapterError, AlsaNegotiatedPcm, AlsaPodxtCaptureAdapter};

#[cfg(feature = "linux-alsa-native")]
pub mod native;

/// Evidence for the same attachment and hardware PCM that will supply audio.
/// The platform must obtain this independently of the recorder plan and PCM
/// parameters. A name, USB ID, or matching format by itself is insufficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedAlsaRoute {
    pub driver: &'static str,
    pub usb_vendor: u16,
    pub usb_product: u16,
    pub usb_interface: u8,
    pub capture_endpoint: u8,
    pub direct_hardware_pcm: bool,
    pub instrument: ProductProfile,
    pub record_send: RecordRoute,
    pub channel_map: ChannelMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlsaCaptureParams {
    pub pcm: AlsaNegotiatedPcm,
    pub period_frames: u32,
    pub periods: u32,
    pub buffer_frames: u32,
}

/// An owned, configured but unstarted capture PCM. All observations and
/// operations must concern that same handle and attachment. Implementations
/// must use bounded reads and preserve signed ALSA frame counts/errors.
/// Closing releases the handle even if the platform reports an error.
pub trait AlsaCapturePcm {
    fn verified_route(&mut self) -> Option<VerifiedAlsaRoute>;
    fn negotiated_params(&mut self) -> Result<AlsaCaptureParams, i32>;
    fn start(&mut self) -> Result<(), i32>;
    fn read_interleaved(&mut self, frames: usize, buffer: &mut [u8]) -> isize;
    fn drop_stream(&mut self) -> Result<(), i32>;
    fn close(self) -> Result<(), i32>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AlsaCleanup {
    pub drop_errno: Option<i32>,
    pub close_errno: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlsaStartFailure {
    UnverifiedRoute,
    Negotiation(i32),
    Contract(AlsaAdapterError),
    Period(pod_audio::PlanError),
    BufferGeometry,
    Start(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsaStartError {
    pub cause: AlsaStartFailure,
    pub cleanup: AlsaCleanup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsaRunError {
    pub capture: Option<CaptureError>,
    pub cleanup: AlsaCleanup,
}

/// Owns one PCM and one common capture session. The owner thread also
/// releases the PCM if the adapter falls out of scope.
pub struct AlsaCaptureStream<P: AlsaCapturePcm> {
    pcm: Option<P>,
    capture: AlsaPodxtCaptureAdapter,
    period_frames: usize,
    buffer: Vec<u8>,
    cleanup: AlsaCleanup,
    owner_thread: PhantomData<Rc<()>>,
}

impl<P: AlsaCapturePcm> AlsaCaptureStream<P> {
    pub fn start(mut pcm: P, plan: ValidatedPlan, generation: u64) -> Result<Self, AlsaStartError> {
        let (capture, period) = match prepare(&mut pcm, plan, generation) {
            Ok(prepared) => prepared,
            Err(cause) => {
                return Err(AlsaStartError {
                    cause,
                    cleanup: release(pcm, false),
                });
            }
        };
        let buffer = vec![0; period.period_bytes() as usize];
        if let Err(code) = pcm.start() {
            return Err(AlsaStartError {
                cause: AlsaStartFailure::Start(code),
                cleanup: release(pcm, true),
            });
        }
        Ok(Self {
            pcm: Some(pcm),
            capture,
            period_frames: period.frames_per_period() as usize,
            buffer,
            cleanup: AlsaCleanup::default(),
            owner_thread: PhantomData,
        })
    }

    pub fn session(&self) -> &CaptureSession {
        self.capture.session()
    }

    /// Reads at most one negotiated period and commits it to the common take.
    /// Any terminal failure releases the PCM and preserves common recovery data.
    pub fn read_period(&mut self, generation: u64) -> Result<usize, AlsaRunError> {
        let Some(pcm) = &mut self.pcm else {
            let error = self.capture.stop().err().unwrap_or(CaptureError::Stopped);
            return Err(self.run_error(Some(error)));
        };
        if let Err(error) = self.capture.session.check_callback_admission(generation) {
            return Err(self.run_error(Some(error)));
        }
        let frames = pcm.read_interleaved(self.period_frames, &mut self.buffer);
        let result = self
            .capture
            .translate_read_result(generation, frames, self.period_frames, &self.buffer)
            .and_then(|()| self.capture.drain());
        if let Err(error) = result {
            self.release();
            let _ = self.capture.stop();
            return Err(self.run_error(Some(error)));
        }
        Ok(frames as usize)
    }

    pub fn stop(&mut self) -> Result<(), AlsaRunError> {
        self.release();
        let result = self.capture.stop();
        self.finish(result)
    }

    pub fn disconnect(&mut self) -> Result<(), AlsaRunError> {
        self.release();
        let result = self.capture.disconnect();
        self.finish(result)
    }

    pub fn cancel(&mut self) -> Result<(), AlsaRunError> {
        self.release();
        let result = self.capture.cancel();
        self.finish(result)
    }

    pub fn take_pending_events(&mut self) -> Result<Vec<PendingCaptureEvent>, CaptureError> {
        self.capture.take_pending_events()
    }

    fn release(&mut self) {
        if let Some(pcm) = self.pcm.take() {
            self.cleanup = release(pcm, true);
        }
    }

    fn run_error(&self, capture: Option<CaptureError>) -> AlsaRunError {
        AlsaRunError {
            capture,
            cleanup: self.cleanup,
        }
    }

    fn finish(&self, result: Result<(), CaptureError>) -> Result<(), AlsaRunError> {
        match result {
            Ok(()) if self.cleanup == AlsaCleanup::default() => Ok(()),
            result => Err(self.run_error(result.err())),
        }
    }
}

impl<P: AlsaCapturePcm> Drop for AlsaCaptureStream<P> {
    fn drop(&mut self) {
        self.release();
    }
}

fn release<P: AlsaCapturePcm>(mut pcm: P, started_or_attempted: bool) -> AlsaCleanup {
    let drop_errno = if started_or_attempted {
        pcm.drop_stream().err()
    } else {
        None
    };
    AlsaCleanup {
        drop_errno,
        close_errno: pcm.close().err(),
    }
}

fn prepare(
    pcm: &mut impl AlsaCapturePcm,
    plan: ValidatedPlan,
    generation: u64,
) -> Result<(AlsaPodxtCaptureAdapter, pod_audio::BufferPlan), AlsaStartFailure> {
    plan.revalidate()
        .map_err(|error| AlsaStartFailure::Contract(error.into()))?;
    let contract = plan.contract();
    let declaration = audio::PODXT_LIVE_DECLARATION;
    let record_send = plan.signal_route();
    if contract.route_kind != Some(audio::RouteKind::SpecificBridge)
        || plan.connected_instrument() != Some(ProductProfile::Variax600)
        || !matches!(
            record_send,
            RecordRoute::SendProcessedGuitar | RecordRoute::SendCleanGuitar
        )
        || pcm.verified_route()
            != Some(VerifiedAlsaRoute {
                driver: "snd-usb-pod",
                usb_vendor: declaration.usb_vendor,
                usb_product: declaration.usb_product,
                usb_interface: declaration.usb_interface,
                capture_endpoint: declaration.capture_endpoint,
                direct_hardware_pcm: true,
                instrument: ProductProfile::Variax600,
                record_send,
                channel_map: contract.channel_map,
            })
    {
        return Err(AlsaStartFailure::UnverifiedRoute);
    }

    let negotiated = pcm
        .negotiated_params()
        .map_err(AlsaStartFailure::Negotiation)?;
    let period_bytes = negotiated
        .period_frames
        .checked_mul(pod_audio::FRAME_BYTES)
        .ok_or(AlsaStartFailure::BufferGeometry)?;
    let period = pod_audio::plan_buffer(period_bytes, negotiated.periods)
        .map_err(AlsaStartFailure::Period)?;
    if negotiated.buffer_frames != period.buffer_bytes() / pod_audio::FRAME_BYTES {
        return Err(AlsaStartFailure::BufferGeometry);
    }
    let capture = AlsaPodxtCaptureAdapter::from_negotiated(plan, negotiated.pcm, generation)
        .map_err(AlsaStartFailure::Contract)?;
    Ok((capture, period))
}

#[cfg(test)]
mod tests;

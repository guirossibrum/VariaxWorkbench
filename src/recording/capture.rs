//! Validated, bounded software capture sessions shared by recorder inputs.
//!
//! This module deliberately has no OS or device backend. Native route plans
//! are revalidated against the frozen registry, but remain declarations only;
//! external physical routes are rejected until an independently reviewed
//! interface contract is registered. The queue is single-owner and intended
//! for deterministic adapters and tests, not a real-time callback guarantee.

use std::collections::VecDeque;
use std::thread::ThreadId;

use crate::pod_audio::RecordRoute;
use crate::rig::ProductProfile;
use crate::rig::audio::{
    AdmissionError, CONTRACT, Declaration, EndpointUse, OfflineRoute, PODXT_LIVE_DECLARATION,
    Platform, Registry, Request, RouteKind, RuntimeState, SampleFormat,
};

use super::{FrameFormat, InputOrigin, SampleClock, Take, TakeError, TakeRequest};

pub const QUEUE_CAPACITY_BYTES: usize = 64 * 1024;
pub const MAX_CHANNELS: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceIdentity {
    PodxtLive,
    /// An identifier is descriptive only and cannot create a physical route.
    External {
        vendor: u16,
        product: u16,
        interface: u8,
    },
    SoftwareFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureBackend {
    LinuxDeclaredPod,
    WindowsUnverified,
    MacOsUnverified,
    SoftwareFile,
    ExternalUnverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelMap {
    /// Destination is a stereo recorder channel; source is a zero-based input.
    pub left: u8,
    pub right: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureContract {
    pub identity: InterfaceIdentity,
    pub backend: CaptureBackend,
    pub contract: &'static str,
    pub declaration: Option<Declaration>,
    pub channels: u8,
    pub clock: (u32, u32),
    pub format: SampleFormat,
    pub channel_map: ChannelMap,
    pub provenance: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Registry(AdmissionError),
    ForgedOrChangedRoute,
    ExternalContractMissing,
    UnsupportedPlatform,
    InvalidChannels,
    InvalidChannelMap,
    InvalidClock,
    InvalidFormat,
    WrongThread,
    StaleGeneration,
    QueueFull,
    EmptyChunk,
    MisalignedChunk,
    Take(TakeError),
    Stopped,
}

impl From<TakeError> for CaptureError {
    fn from(value: TakeError) -> Self {
        Self::Take(value)
    }
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CaptureError {}

/// A plan can only be made by one of the checked constructors below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedPlan {
    contract: CaptureContract,
    instrument: Option<ProductProfile>,
    signal_route: RecordRoute,
    max_take_bytes: u64,
}

impl ValidatedPlan {
    /// Validates every caller-supplied field by independently asking the frozen
    /// registry to plan it, then compares the result with the admitted record.
    /// This is still declaration-only and cannot open a live backend.
    pub fn native_podxt(
        registry: &Registry,
        request: Request<'_>,
        signal_route: RecordRoute,
        max_take_bytes: u64,
    ) -> Result<Self, CaptureError> {
        let route = registry.plan(&request).map_err(CaptureError::Registry)?;
        validate_native_route(&request, &route)?;
        if max_take_bytes == 0 || max_take_bytes > super::TAKE_BYTES_CEILING {
            return Err(CaptureError::Take(TakeError::TakeCapInvalid));
        }
        Ok(Self {
            contract: CaptureContract {
                identity: InterfaceIdentity::PodxtLive,
                backend: CaptureBackend::LinuxDeclaredPod,
                contract: CONTRACT,
                declaration: Some(PODXT_LIVE_DECLARATION),
                channels: 2,
                clock: PODXT_LIVE_DECLARATION.clock,
                format: SampleFormat::S24ThreeByteLe,
                channel_map: ChannelMap { left: 0, right: 1 },
                provenance: PODXT_LIVE_DECLARATION.source,
            },
            instrument: route.connected_instrument,
            signal_route,
            max_take_bytes,
        })
    }

    /// File input is an explicit software source, never an external hardware
    /// admission. This constructor accepts only exact stereo PCM S16 WAV rates.
    pub fn software_file(
        rate_hz: u32,
        channels: u8,
        map: ChannelMap,
        max_take_bytes: u64,
    ) -> Result<Self, CaptureError> {
        validate_software_shape(rate_hz, channels, map, max_take_bytes)?;
        Ok(Self {
            contract: CaptureContract {
                identity: InterfaceIdentity::SoftwareFile,
                backend: CaptureBackend::SoftwareFile,
                contract: "software-file-pcm-s16le-v1",
                declaration: None,
                channels,
                clock: (rate_hz, 1),
                format: SampleFormat::S16Le,
                channel_map: map,
                provenance: "user-selected regular PCM WAV; software file adapter",
            },
            instrument: None,
            signal_route: RecordRoute::SendProcessedGuitar,
            max_take_bytes,
        })
    }

    /// No external hardware contracts are independently admitted yet.
    pub fn external(_identity: InterfaceIdentity) -> Result<Self, CaptureError> {
        Err(CaptureError::ExternalContractMissing)
    }

    pub fn contract(&self) -> CaptureContract {
        self.contract
    }

    pub fn connected_instrument(&self) -> Option<ProductProfile> {
        self.instrument
    }

    /// Rechecks the plan before every session; private fields prevent callers
    /// from changing identity, timing, format, map, or provenance in between.
    pub fn revalidate(&self) -> Result<(), CaptureError> {
        match self.contract.identity {
            InterfaceIdentity::PodxtLive => {
                if self.contract.backend != CaptureBackend::LinuxDeclaredPod
                    || self.contract.contract != CONTRACT
                    || self.contract.declaration != Some(PODXT_LIVE_DECLARATION)
                    || self.contract.channels != 2
                    || self.contract.clock != (78_125, 2)
                    || self.contract.format != SampleFormat::S24ThreeByteLe
                    || self.contract.channel_map != (ChannelMap { left: 0, right: 1 })
                    || self.contract.provenance != PODXT_LIVE_DECLARATION.source
                {
                    return Err(CaptureError::ForgedOrChangedRoute);
                }
            }
            InterfaceIdentity::SoftwareFile => {
                if self.contract.backend != CaptureBackend::SoftwareFile
                    || self.contract.declaration.is_some()
                {
                    return Err(CaptureError::ForgedOrChangedRoute);
                }
                validate_software_shape(
                    self.contract.clock.0,
                    self.contract.channels,
                    self.contract.channel_map,
                    self.max_take_bytes,
                )?;
            }
            InterfaceIdentity::External { .. } => {
                return Err(CaptureError::ExternalContractMissing);
            }
        }
        Ok(())
    }

    pub fn prepare_take(&self) -> Result<Take, CaptureError> {
        self.revalidate()?;
        let (clock, format, origin) = match self.contract.identity {
            InterfaceIdentity::PodxtLive => (
                SampleClock::native_podxt(),
                FrameFormat::PodS24_3Le,
                InputOrigin::Synthetic,
            ),
            InterfaceIdentity::SoftwareFile => (
                SampleClock::Integer {
                    hz: self.contract.clock.0,
                },
                FrameFormat::PcmS16Le,
                InputOrigin::File {
                    label: "software file adapter".into(),
                },
            ),
            InterfaceIdentity::External { .. } => {
                return Err(CaptureError::ExternalContractMissing);
            }
        };
        let mut take = Take::prepare(TakeRequest {
            route: self.signal_route,
            clock,
            format,
            origin,
            max_bytes: self.max_take_bytes,
        })?;
        take.start()?;
        Ok(take)
    }
}

fn validate_native_route(request: &Request<'_>, route: &OfflineRoute) -> Result<(), CaptureError> {
    let d = route.declaration;
    if request.platform != Platform::Linux {
        return Err(CaptureError::UnsupportedPlatform);
    }
    if route.capture_profile != ProductProfile::PodxtLive
        || route.route_kind != RouteKind::NativeUsb
        || d != PODXT_LIVE_DECLARATION
        || d.clock != (78_125, 2)
        || request.contract != CONTRACT
        || request.installed_revision.is_some()
        || request.channels != 2
        || request.clock != (78_125, 2)
        || request.format != SampleFormat::S24ThreeByteLe
        || request.endpoint_use != EndpointUse::Capture
        || request.state != RuntimeState::Normal
    {
        return Err(CaptureError::ForgedOrChangedRoute);
    }
    Ok(())
}

fn validate_software_shape(
    rate_hz: u32,
    channels: u8,
    map: ChannelMap,
    max_take_bytes: u64,
) -> Result<(), CaptureError> {
    if !(2..=MAX_CHANNELS).contains(&channels) {
        return Err(CaptureError::InvalidChannels);
    }
    if map.left >= channels || map.right >= channels || map.left == map.right {
        return Err(CaptureError::InvalidChannelMap);
    }
    if ![22_050, 44_100, 48_000, 88_200, 96_000].contains(&rate_hz) {
        return Err(CaptureError::InvalidClock);
    }
    if max_take_bytes == 0 || max_take_bytes > super::TAKE_BYTES_CEILING {
        return Err(CaptureError::Take(TakeError::TakeCapInvalid));
    }
    Ok(())
}

/// Bounded single-owner queue. Full queues reject whole chunks and record an
/// explicit gap; no producer callback blocks or grows memory without a bound.
#[derive(Debug)]
pub struct CaptureSession {
    plan: ValidatedPlan,
    owner: ThreadId,
    generation: u64,
    queue: VecDeque<Vec<u8>>,
    queued_bytes: usize,
    take: Take,
    pending_dropouts: u32,
    error: Option<CaptureError>,
    stopped: bool,
}

impl CaptureSession {
    pub fn start(plan: ValidatedPlan, generation: u64) -> Result<Self, CaptureError> {
        plan.revalidate()?;
        let take = plan.prepare_take()?;
        Ok(Self {
            plan,
            owner: std::thread::current().id(),
            generation,
            queue: VecDeque::new(),
            queued_bytes: 0,
            take,
            pending_dropouts: 0,
            error: None,
            stopped: false,
        })
    }

    pub fn plan(&self) -> CaptureContract {
        self.plan.contract
    }
    pub fn take(&self) -> &Take {
        &self.take
    }
    pub fn retained_error(&self) -> Option<CaptureError> {
        self.error.clone()
    }
    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Callback-side operation: bounded copy only, no file I/O, no lock, and
    /// no adapter invocation. Queue ownership is deliberately single-thread.
    pub fn callback_chunk(&mut self, generation: u64, bytes: &[u8]) -> Result<(), CaptureError> {
        self.check_owner()?;
        if generation != self.generation {
            return self.fail(CaptureError::StaleGeneration);
        }
        if self.stopped {
            return self.fail(CaptureError::Stopped);
        }
        if bytes.is_empty() {
            return self.fail(CaptureError::EmptyChunk);
        }
        let sample_bytes = match self.plan.contract.format {
            SampleFormat::S16Le => 2,
            SampleFormat::S24ThreeByteLe => 3,
            SampleFormat::Float32 => return self.fail(CaptureError::InvalidFormat),
        };
        let input_frame_bytes = usize::from(self.plan.contract.channels) * sample_bytes;
        if bytes.len() > super::STREAM_CHUNK_BYTES_MAX
            || !bytes.len().is_multiple_of(input_frame_bytes)
        {
            return self.fail(CaptureError::MisalignedChunk);
        }
        let frames = bytes.len() / input_frame_bytes;
        let mut stereo = Vec::with_capacity(frames.saturating_mul(4));
        for frame in bytes.chunks_exact(input_frame_bytes) {
            for channel in [
                self.plan.contract.channel_map.left,
                self.plan.contract.channel_map.right,
            ] {
                let start = usize::from(channel) * sample_bytes;
                stereo.extend_from_slice(&frame[start..start + sample_bytes]);
            }
        }
        if self.queued_bytes.saturating_add(stereo.len()) > QUEUE_CAPACITY_BYTES {
            self.pending_dropouts = self
                .pending_dropouts
                .saturating_add(frames.min(u32::MAX as usize) as u32);
            return self.fail(CaptureError::QueueFull);
        }
        self.queued_bytes += stereo.len();
        self.queue.push_back(stereo);
        Ok(())
    }

    /// Consumer-side work drains queued chunks into existing bounded Take
    /// storage and meter/dropout accounting.
    pub fn drain(&mut self) -> Result<usize, CaptureError> {
        self.check_owner()?;
        while self.pending_dropouts > 0 {
            let n = self.pending_dropouts.min(super::MAX_DROPOUT_FRAMES);
            if let Err(error) = self.take.note_dropout(n) {
                self.error = Some(CaptureError::Take(error.clone()));
                return Err(CaptureError::Take(error));
            }
            self.pending_dropouts -= n;
        }
        let mut count = 0;
        while let Some(chunk) = self.queue.pop_front() {
            self.queued_bytes -= chunk.len();
            if let Err(error) = self.take.push(&chunk) {
                self.error = Some(CaptureError::Take(error.clone()));
                return Err(CaptureError::Take(error));
            }
            count += 1;
        }
        Ok(count)
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        self.check_owner()?;
        self.drain()?;
        self.take.stop()?;
        self.stopped = true;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), CaptureError> {
        self.check_owner()?;
        self.queue.clear();
        self.queued_bytes = 0;
        self.pending_dropouts = 0;
        self.take.cancel()?;
        self.stopped = true;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), CaptureError> {
        self.error = Some(CaptureError::Stopped);
        self.stop()
    }

    fn check_owner(&self) -> Result<(), CaptureError> {
        if std::thread::current().id() != self.owner {
            Err(CaptureError::WrongThread)
        } else {
            Ok(())
        }
    }

    fn fail<T>(&mut self, error: CaptureError) -> Result<T, CaptureError> {
        self.error = Some(error.clone());
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_plan() -> ValidatedPlan {
        ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 1 }, 64 * 1024)
            .unwrap()
    }

    #[test]
    fn software_file_session_bounds_buffers_meters_and_stops() {
        let mut session = CaptureSession::start(file_plan(), 7).unwrap();
        let frames = [0x10, 0x00, 0, 0xf0, 0xff, 0];
        let chunk: Vec<u8> = frames.repeat(32);
        session.callback_chunk(7, &chunk).unwrap();
        assert_eq!(session.queued_bytes(), chunk.len());
        assert_eq!(session.drain().unwrap(), 1);
        assert_eq!(session.take().frames(), 32);
        assert!(session.take().levels().left_peak > 0);
        session.stop().unwrap();
        assert!(session.callback_chunk(7, &chunk).is_err());
        assert_eq!(session.retained_error(), Some(CaptureError::Stopped));
    }

    #[test]
    fn stale_callback_and_unsupported_external_are_retained_as_errors() {
        assert_eq!(
            ValidatedPlan::external(InterfaceIdentity::External {
                vendor: 1,
                product: 2,
                interface: 0
            }),
            Err(CaptureError::ExternalContractMissing)
        );
        let mut session = CaptureSession::start(file_plan(), 9).unwrap();
        assert_eq!(
            session.callback_chunk(8, &[0; 4]),
            Err(CaptureError::StaleGeneration)
        );
        assert_eq!(
            session.retained_error(),
            Some(CaptureError::StaleGeneration)
        );
    }

    #[test]
    fn invalid_channel_map_and_rate_are_rejected() {
        assert_eq!(
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 1, right: 1 }, 1024),
            Err(CaptureError::InvalidChannelMap)
        );
        assert_eq!(
            ValidatedPlan::software_file(39_062, 2, ChannelMap { left: 0, right: 1 }, 1024),
            Err(CaptureError::InvalidClock)
        );
    }

    #[test]
    fn callback_applies_an_explicit_map_before_recording() {
        let plan = ValidatedPlan::software_file(48_000, 3, ChannelMap { left: 2, right: 0 }, 1024)
            .unwrap();
        let mut session = CaptureSession::start(plan, 3).unwrap();
        // Three S16 channels: [100, 200, 300]. Map channel 2 to left and 0 to right.
        session
            .callback_chunk(3, &[100, 0, 200, 0, 44, 1, 8, 0, 9, 0, 10, 0])
            .unwrap();
        session.drain().unwrap();
        assert_eq!(session.take().pcm(), &[44, 1, 100, 0, 10, 0, 8, 0]);
        assert_eq!(session.take().frames(), 2);
    }

    #[test]
    fn bounded_queue_records_overflow_as_a_gap_and_stops_cleanly() {
        let plan =
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 1 }, 128 * 1024)
                .unwrap();
        let mut session = CaptureSession::start(plan, 4).unwrap();
        let chunk = vec![1; super::super::STREAM_CHUNK_BYTES_MAX];
        for _ in 0..QUEUE_CAPACITY_BYTES / chunk.len() {
            session.callback_chunk(4, &chunk).unwrap();
        }
        assert_eq!(session.queued_bytes(), QUEUE_CAPACITY_BYTES);
        assert_eq!(
            session.callback_chunk(4, &chunk),
            Err(CaptureError::QueueFull)
        );
        assert_eq!(session.queued_bytes(), QUEUE_CAPACITY_BYTES);
        session.stop().unwrap();
        assert_eq!(session.queued_bytes(), 0);
        assert_eq!(session.take().frames(), 18_432);
        assert_eq!(session.take().dropouts().len(), 1);
    }
}

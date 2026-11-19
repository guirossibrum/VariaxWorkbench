//! Validated, bounded software capture sessions shared by recorder inputs.
//!
//! Native route plans are revalidated against the frozen registry, but remain
//! declarations only; external physical routes are rejected until an
//! independently reviewed interface contract is registered. OS-specific
//! contract translations live in `recording::adapters`; they are inactive and
//! do not initialize a device. The queue is single-owner and intended for
//! deterministic adapters and tests, not a real-time callback guarantee.

use std::collections::VecDeque;
use std::thread::ThreadId;

use crate::pod_audio::RecordRoute;
use crate::rig::ProductProfile;
use crate::rig::audio::{
    AdmissionError, CONTRACT, Declaration, EndpointUse, OfflineRoute, PODXT_LIVE_DECLARATION,
    Platform, Registry, Request, RouteKind, RuntimeState, SampleFormat,
};

use super::clock::{ClockError, ClockMeasurement, SoftwareClockTracker};
use super::{FrameFormat, InputOrigin, Phase, SampleClock, Take, TakeError, TakeRequest};

pub const QUEUE_CAPACITY_BYTES: usize = 64 * 1024;
pub const MAX_CHANNELS: u8 = 8;
pub const MAX_QUEUE_EVENTS: usize = QUEUE_CAPACITY_BYTES / 2 + 1;

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
    SoftwareSynthetic,
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
    pub capture_row: Option<usize>,
    pub route_kind: Option<RouteKind>,
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
    Clock(ClockError),
    WrongThread,
    StaleGeneration,
    QueueFull,
    EmptyChunk,
    MisalignedChunk,
    Take(TakeError),
    Disconnected,
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
    native_admission: Option<NativeAdmission>,
    input_origin: InputOrigin,
    signal_route: RecordRoute,
    max_take_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeAdmission {
    capture_row: usize,
    connected_instrument: Option<ProductProfile>,
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
                capture_row: Some(route.capture_row),
                route_kind: Some(route.route_kind),
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
            native_admission: Some(NativeAdmission {
                capture_row: route.capture_row,
                connected_instrument: route.connected_instrument,
            }),
            input_origin: InputOrigin::Synthetic,
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
        Self::software_file_labeled(
            rate_hz,
            channels,
            map,
            "software file adapter".into(),
            RecordRoute::SendProcessedGuitar,
            max_take_bytes,
        )
    }

    pub fn software_file_labeled(
        rate_hz: u32,
        channels: u8,
        map: ChannelMap,
        label: String,
        signal_route: RecordRoute,
        max_take_bytes: u64,
    ) -> Result<Self, CaptureError> {
        validate_software_shape(rate_hz, channels, map, max_take_bytes)?;
        if label.is_empty() {
            return Err(CaptureError::Take(TakeError::SelectionMissing(
                "file label",
            )));
        }
        Ok(Self {
            contract: CaptureContract {
                identity: InterfaceIdentity::SoftwareFile,
                capture_row: None,
                route_kind: None,
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
            native_admission: None,
            input_origin: InputOrigin::File { label },
            signal_route,
            max_take_bytes,
        })
    }

    pub fn software_synthetic(
        rate_hz: u32,
        channels: u8,
        map: ChannelMap,
        signal_route: RecordRoute,
        max_take_bytes: u64,
    ) -> Result<Self, CaptureError> {
        validate_software_shape(rate_hz, channels, map, max_take_bytes)?;
        Ok(Self {
            contract: CaptureContract {
                identity: InterfaceIdentity::SoftwareSynthetic,
                capture_row: None,
                route_kind: None,
                backend: CaptureBackend::SoftwareFile,
                contract: "software-synthetic-pcm-s16le-v1",
                declaration: None,
                channels,
                clock: (rate_hz, 1),
                format: SampleFormat::S16Le,
                channel_map: map,
                provenance: "owned deterministic software generator; no device source",
            },
            instrument: None,
            native_admission: None,
            input_origin: InputOrigin::Synthetic,
            signal_route,
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
                let admitted = self
                    .native_admission
                    .ok_or(CaptureError::ForgedOrChangedRoute)?;
                let registry =
                    Registry::frozen().map_err(|_| CaptureError::ForgedOrChangedRoute)?;
                let request = Request {
                    capture_row: admitted.capture_row,
                    capture_profile: ProductProfile::PodxtLive,
                    connected_instrument: admitted.connected_instrument,
                    platform: Platform::Linux,
                    state: RuntimeState::Normal,
                    endpoint_use: EndpointUse::Capture,
                    contract: CONTRACT,
                    installed_revision: None,
                    channels: 2,
                    clock: (78_125, 2),
                    format: SampleFormat::S24ThreeByteLe,
                };
                let route = registry.plan(&request).map_err(CaptureError::Registry)?;
                validate_native_route(&request, &route)?;
                if self.contract.backend != CaptureBackend::LinuxDeclaredPod
                    || self.contract.capture_row != Some(route.capture_row)
                    || self.contract.route_kind != Some(route.route_kind)
                    || self.contract.contract != CONTRACT
                    || self.contract.declaration != Some(PODXT_LIVE_DECLARATION)
                    || self.contract.channels != 2
                    || self.contract.clock != (78_125, 2)
                    || self.contract.format != SampleFormat::S24ThreeByteLe
                    || self.contract.channel_map != (ChannelMap { left: 0, right: 1 })
                    || self.contract.provenance != PODXT_LIVE_DECLARATION.source
                    || self.instrument != route.connected_instrument
                    || self.input_origin != InputOrigin::Synthetic
                {
                    return Err(CaptureError::ForgedOrChangedRoute);
                }
            }
            InterfaceIdentity::SoftwareFile => {
                if self.contract.backend != CaptureBackend::SoftwareFile
                    || self.contract.contract != "software-file-pcm-s16le-v1"
                    || self.contract.declaration.is_some()
                    || self.contract.capture_row.is_some()
                    || self.contract.route_kind.is_some()
                    || self.contract.format != SampleFormat::S16Le
                    || !matches!(self.input_origin, InputOrigin::File { .. })
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
            InterfaceIdentity::SoftwareSynthetic => {
                if self.contract.backend != CaptureBackend::SoftwareFile
                    || self.contract.contract != "software-synthetic-pcm-s16le-v1"
                    || self.contract.declaration.is_some()
                    || self.contract.capture_row.is_some()
                    || self.contract.route_kind.is_some()
                    || self.contract.format != SampleFormat::S16Le
                    || self.input_origin != InputOrigin::Synthetic
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
        let (clock, format) = match self.contract.identity {
            InterfaceIdentity::PodxtLive => (SampleClock::native_podxt(), FrameFormat::PodS24_3Le),
            InterfaceIdentity::SoftwareFile | InterfaceIdentity::SoftwareSynthetic => (
                SampleClock::Integer {
                    hz: self.contract.clock.0,
                },
                FrameFormat::PcmS16Le,
            ),
            InterfaceIdentity::External { .. } => {
                return Err(CaptureError::ExternalContractMissing);
            }
        };
        let mut take = Take::prepare(TakeRequest {
            route: self.signal_route,
            clock,
            format,
            origin: self.input_origin.clone(),
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
    let expected_route = if request.connected_instrument == Some(ProductProfile::Variax600) {
        RouteKind::SpecificBridge
    } else {
        RouteKind::NativeUsb
    };
    if route.capture_profile != ProductProfile::PodxtLive
        || route.connected_instrument != request.connected_instrument
        || route.route_kind != expected_route
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
#[derive(Clone, Debug, PartialEq, Eq)]
enum QueueEvent {
    Audio(Vec<u8>),
    Gap(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingCaptureEvent {
    Audio(Vec<u8>),
    Dropout { frames: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureSession {
    plan: ValidatedPlan,
    owner: ThreadId,
    generation: u64,
    queue: VecDeque<QueueEvent>,
    queued_bytes: usize,
    queued_gap_frames: u64,
    take: Take,
    clock: SoftwareClockTracker,
    last_levels: super::Levels,
    error: Option<CaptureError>,
    terminal_error: Option<CaptureError>,
    terminal: bool,
    drain_blocked: bool,
    stopped: bool,
}

impl CaptureSession {
    pub fn start(plan: ValidatedPlan, generation: u64) -> Result<Self, CaptureError> {
        plan.revalidate()?;
        let clock = SoftwareClockTracker::new(plan.contract.clock.0, plan.contract.clock.1)
            .map_err(CaptureError::Clock)?;
        let take = plan.prepare_take()?;
        Ok(Self {
            plan,
            owner: std::thread::current().id(),
            generation,
            queue: VecDeque::new(),
            queued_bytes: 0,
            queued_gap_frames: 0,
            take,
            clock,
            last_levels: super::Levels::default(),
            error: None,
            terminal_error: None,
            terminal: false,
            drain_blocked: false,
            stopped: false,
        })
    }

    pub fn plan(&self) -> CaptureContract {
        self.plan.contract
    }
    pub fn take(&self) -> &Take {
        &self.take
    }
    pub fn last_levels(&self) -> super::Levels {
        self.last_levels
    }
    pub fn retained_error(&self) -> Option<CaptureError> {
        self.terminal_error.clone().or_else(|| self.error.clone())
    }
    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn clock_measurement(&self) -> Option<ClockMeasurement> {
        self.clock.latest()
    }

    /// Records explicit adapter-supplied software timing observations. The
    /// timestamps are not obtained from hardware and do not certify device
    /// clock synchronization.
    pub fn observe_software_clock(
        &mut self,
        cumulative_frames: u64,
        monotonic_ns: u64,
        dropout_frames: u64,
    ) -> Result<ClockMeasurement, CaptureError> {
        self.check_owner()?;
        if self.take.phase() != Phase::Recording || self.terminal || self.stopped {
            return Err(self.terminal_error.clone().unwrap_or(CaptureError::Stopped));
        }
        match self
            .clock
            .observe(cumulative_frames, monotonic_ns, dropout_frames)
        {
            Ok(measurement) => Ok(measurement),
            Err(error) => self.fail(CaptureError::Clock(error)),
        }
    }
    pub fn pending_events(&self) -> usize {
        self.queue.len()
    }
    pub fn pending_audio_bytes(&self) -> usize {
        self.queued_bytes
    }

    /// Callback-side operation: bounded copy only, no file I/O, no lock, and
    /// no adapter invocation. Queue ownership is deliberately single-thread.
    pub fn callback_chunk(&mut self, generation: u64, bytes: &[u8]) -> Result<(), CaptureError> {
        self.check_owner()?;
        if generation != self.generation {
            return self.fail(CaptureError::StaleGeneration);
        }
        if self.terminal {
            return Err(self.error.clone().unwrap_or(CaptureError::Stopped));
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
        let mut stereo = Vec::with_capacity(frames.saturating_mul(self.output_frame_bytes()));
        for frame in bytes.chunks_exact(input_frame_bytes) {
            for channel in [
                self.plan.contract.channel_map.left,
                self.plan.contract.channel_map.right,
            ] {
                let start = usize::from(channel) * sample_bytes;
                stereo.extend_from_slice(&frame[start..start + sample_bytes]);
            }
        }
        let frame_bytes = self.output_frame_bytes();
        let used_bytes = self
            .take
            .pcm()
            .len()
            .saturating_add(self.queued_bytes)
            .saturating_add((self.queued_gap_frames as usize).saturating_mul(frame_bytes));
        if self.queued_bytes.saturating_add(stereo.len()) > QUEUE_CAPACITY_BYTES {
            let gap_bytes = frames.saturating_mul(frame_bytes);
            if used_bytes.saturating_add(gap_bytes) > self.plan.max_take_bytes as usize {
                return self.terminal_failure(CaptureError::Take(TakeError::CapExceeded));
            }
            if let Some(QueueEvent::Gap(previous)) = self.queue.back_mut() {
                *previous = previous.saturating_add(frames as u64);
            } else {
                if self.queue.len() >= MAX_QUEUE_EVENTS {
                    return self.terminal_failure(CaptureError::QueueFull);
                }
                self.queue.push_back(QueueEvent::Gap(frames as u64));
            }
            self.queued_gap_frames = self.queued_gap_frames.saturating_add(frames as u64);
            return self.fail(CaptureError::QueueFull);
        }
        if used_bytes.saturating_add(stereo.len()) > self.plan.max_take_bytes as usize {
            return self.terminal_failure(CaptureError::Take(TakeError::CapExceeded));
        }
        if self.queue.len() >= MAX_QUEUE_EVENTS {
            return self.terminal_failure(CaptureError::QueueFull);
        }
        self.queued_bytes += stereo.len();
        self.queue.push_back(QueueEvent::Audio(stereo));
        Ok(())
    }

    /// Consumer-side work drains queued chunks into existing bounded Take
    /// storage and meter/dropout accounting.
    pub fn drain(&mut self) -> Result<usize, CaptureError> {
        self.check_owner()?;
        if self.take.phase() != Phase::Recording {
            return self.terminal_error.clone().map_or(Ok(0), Err);
        }
        if self.drain_blocked {
            return Err(self.terminal_error.clone().unwrap_or(CaptureError::Stopped));
        }
        let mut count = 0;
        while let Some(event) = self.queue.front().cloned() {
            match event {
                QueueEvent::Audio(chunk) => {
                    let levels = match self.take.push(&chunk) {
                        Ok(levels) => levels,
                        Err(error) => {
                            return self.drain_failure(CaptureError::Take(error));
                        }
                    };
                    self.last_levels = levels;
                    self.queue.pop_front();
                    self.queued_bytes -= chunk.len();
                    count += 1;
                }
                QueueEvent::Gap(frames) => {
                    let n = frames.min(u64::from(super::MAX_DROPOUT_FRAMES)) as u32;
                    if let Err(error) = self.take.note_dropout(n) {
                        return self.drain_failure(CaptureError::Take(error));
                    }
                    self.queued_gap_frames -= u64::from(n);
                    if u64::from(n) == frames {
                        self.queue.pop_front();
                    } else if let Some(QueueEvent::Gap(remaining)) = self.queue.front_mut() {
                        *remaining -= u64::from(n);
                    }
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        self.check_owner()?;
        if self.take.phase() != Phase::Recording {
            self.stopped = true;
            return self.terminal_error.clone().map_or(Ok(()), Err);
        }

        let drained = if self.drain_blocked {
            Err(self.terminal_error.clone().unwrap_or(CaptureError::Stopped))
        } else {
            self.drain().map(|_| ())
        };
        if let Err(error) = self.take.stop() {
            let root = self.record_terminal_failure(CaptureError::Take(error));
            return Err(root);
        }
        self.stopped = true;
        if let Some(error) = self.terminal_error.clone() {
            return Err(error);
        }
        drained
    }

    pub fn cancel(&mut self) -> Result<(), CaptureError> {
        self.check_owner()?;
        if self.take.phase() == Phase::Cancelled {
            return Ok(());
        }
        self.queue.clear();
        self.queued_bytes = 0;
        self.queued_gap_frames = 0;
        self.take.cancel()?;
        self.stopped = true;
        self.terminal = true;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), CaptureError> {
        self.check_owner()?;
        if self.take.phase() == Phase::Recording && self.terminal_error.is_none() {
            let _ = self.record_terminal_failure(CaptureError::Disconnected);
        }
        self.stop()
    }

    /// Transfers accepted but not yet committed events to the caller after a
    /// terminal stop error. This is explicit recovery; it never silently drops
    /// queued audio or gap ordering.
    pub fn take_pending_events(&mut self) -> Result<Vec<PendingCaptureEvent>, CaptureError> {
        self.check_owner()?;
        if self.take.phase() != Phase::Stopped {
            return Err(CaptureError::Stopped);
        }
        self.queued_bytes = 0;
        self.queued_gap_frames = 0;
        Ok(self
            .queue
            .drain(..)
            .map(|event| match event {
                QueueEvent::Audio(bytes) => PendingCaptureEvent::Audio(bytes),
                QueueEvent::Gap(frames) => PendingCaptureEvent::Dropout { frames },
            })
            .collect())
    }

    fn check_owner(&self) -> Result<(), CaptureError> {
        if std::thread::current().id() != self.owner {
            Err(CaptureError::WrongThread)
        } else {
            Ok(())
        }
    }

    fn fail<T>(&mut self, error: CaptureError) -> Result<T, CaptureError> {
        self.error.get_or_insert(error.clone());
        Err(error)
    }

    fn terminal_failure<T>(&mut self, error: CaptureError) -> Result<T, CaptureError> {
        let root = self.record_terminal_failure(error);
        Err(root)
    }

    fn drain_failure<T>(&mut self, error: CaptureError) -> Result<T, CaptureError> {
        self.drain_blocked = true;
        self.terminal_failure(error)
    }

    fn record_terminal_failure(&mut self, error: CaptureError) -> CaptureError {
        let root = self.terminal_error.get_or_insert(error).clone();
        self.error = Some(root.clone());
        self.terminal = true;
        self.stopped = true;
        root
    }

    fn output_frame_bytes(&self) -> usize {
        let sample_bytes = match self.plan.contract.format {
            SampleFormat::S16Le => 2,
            SampleFormat::S24ThreeByteLe => 3,
            SampleFormat::Float32 => 0,
        };
        sample_bytes * 2
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
        let frames = [0x10, 0x00, 0xf0, 0xff];
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
    fn overflow_gaps_preserve_chronology_across_drains_and_later_callbacks() {
        let plan =
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 1 }, 128 * 1024)
                .unwrap();
        let mut session = CaptureSession::start(plan, 4).unwrap();
        let a = vec![0x11; super::super::STREAM_CHUNK_BYTES_MAX];
        let b = vec![0x22; super::super::STREAM_CHUNK_BYTES_MAX];
        for _ in 0..QUEUE_CAPACITY_BYTES / a.len() {
            session.callback_chunk(4, &a).unwrap();
        }
        assert_eq!(session.queued_bytes(), QUEUE_CAPACITY_BYTES);
        assert_eq!(session.callback_chunk(4, &b), Err(CaptureError::QueueFull));
        assert_eq!(session.callback_chunk(4, &b), Err(CaptureError::QueueFull));
        assert_eq!(session.queued_bytes(), QUEUE_CAPACITY_BYTES);
        session.drain().unwrap();
        assert_eq!(session.take().pcm().len(), QUEUE_CAPACITY_BYTES + 16_384);
        assert_eq!(
            &session.take().pcm()[..QUEUE_CAPACITY_BYTES],
            vec![0x11; QUEUE_CAPACITY_BYTES]
        );
        assert_eq!(
            &session.take().pcm()[QUEUE_CAPACITY_BYTES..],
            vec![0; 16_384]
        );
        assert_eq!(
            session
                .take()
                .dropouts()
                .iter()
                .map(|gap| (gap.at_frame, gap.missing_frames))
                .collect::<Vec<_>>(),
            vec![(16_384, 2_048), (18_432, 2_048)]
        );
        let c = [0x44, 0x33, 0x66, 0x55];
        session.callback_chunk(4, &c).unwrap();
        session.drain().unwrap();
        assert_eq!(&session.take().pcm()[QUEUE_CAPACITY_BYTES + 16_384..], &c);
        session.stop().unwrap();
        assert_eq!(session.queued_bytes(), 0);
        assert_eq!(session.take().frames(), 20_481);
    }

    fn native_request(instrument: Option<ProductProfile>) -> (Registry, Request<'static>) {
        let registry = Registry::frozen().unwrap();
        let row = registry
            .rows()
            .iter()
            .find(|row| {
                row.name == "PODxt Live" && row.role == crate::rig::audio::SourceRole::Bridge
            })
            .unwrap()
            .ordinal;
        (
            registry,
            Request {
                capture_row: row,
                capture_profile: ProductProfile::PodxtLive,
                connected_instrument: instrument,
                platform: Platform::Linux,
                state: RuntimeState::Normal,
                endpoint_use: EndpointUse::Capture,
                contract: CONTRACT,
                installed_revision: None,
                channels: 2,
                clock: (78_125, 2),
                format: SampleFormat::S24ThreeByteLe,
            },
        )
    }

    #[test]
    fn registry_native_and_specific_bridge_routes_are_revalidated_without_family_widening() {
        let (registry, direct) = native_request(None);
        let native =
            ValidatedPlan::native_podxt(&registry, direct, RecordRoute::SendCleanGuitar, 1024)
                .unwrap();
        assert_eq!(native.contract().route_kind, Some(RouteKind::NativeUsb));
        assert_eq!(native.connected_instrument(), None);
        native.revalidate().unwrap();

        let (registry, bridge) = native_request(Some(ProductProfile::Variax600));
        let plan =
            ValidatedPlan::native_podxt(&registry, bridge, RecordRoute::SendCleanGuitar, 1024)
                .unwrap();
        assert_eq!(plan.contract().route_kind, Some(RouteKind::SpecificBridge));
        assert_eq!(plan.connected_instrument(), Some(ProductProfile::Variax600));
        plan.revalidate().unwrap();

        let (registry, wrong_family) = native_request(Some(ProductProfile::Jtv69));
        assert_eq!(
            ValidatedPlan::native_podxt(
                &registry,
                wrong_family,
                RecordRoute::SendCleanGuitar,
                1024
            ),
            Err(CaptureError::Registry(AdmissionError::WrongBridgeFamily))
        );
    }

    #[test]
    fn native_session_clock_observation_keeps_the_exact_rational_source_rate() {
        let (registry, request) = native_request(None);
        let plan =
            ValidatedPlan::native_podxt(&registry, request, RecordRoute::SendCleanGuitar, 1024)
                .unwrap();
        let mut session = CaptureSession::start(plan, 11).unwrap();
        assert_eq!(session.plan().clock, (78_125, 2));
        session.observe_software_clock(0, 1_000, 0).unwrap();
        let observation = session
            .observe_software_clock(1_000, 25_601_000, 2)
            .unwrap();
        assert_eq!(observation.expected_elapsed_ns, 25_600_000);
        assert_eq!(observation.observed_elapsed_ns, 25_600_000);
        assert_eq!(observation.drift_ppm, Some(0));
        assert_eq!(observation.cumulative_dropout_frames, 2);
        assert_eq!(session.clock_measurement(), Some(observation));
        session.stop().unwrap();
        assert_eq!(
            session.observe_software_clock(2_000, 51_201_000, 0),
            Err(CaptureError::Stopped)
        );
        assert_eq!(session.clock_measurement(), Some(observation));
    }

    #[test]
    fn queued_chunk_failure_freezes_callbacks_and_stop_preserves_recorded_audio() {
        let (registry, request) = native_request(None);
        let plan =
            ValidatedPlan::native_podxt(&registry, request, RecordRoute::SendCleanGuitar, 1024)
                .unwrap();
        let mut session = CaptureSession::start(plan, 12).unwrap();
        let recorded = vec![0x12; 66];
        session.callback_chunk(12, &recorded).unwrap();
        session.drain().unwrap();
        let too_short = [1, 2, 3, 4, 5, 6];
        session.callback_chunk(12, &too_short).unwrap();
        assert!(matches!(
            session.drain(),
            Err(CaptureError::Take(TakeError::ChunkRejected(_)))
        ));
        assert_eq!(session.pending_audio_bytes(), 6);
        assert_eq!(session.take().pcm(), recorded);
        let retained = session.retained_error();
        assert_eq!(
            session.callback_chunk(12, &recorded),
            Err(retained.clone().unwrap())
        );
        assert!(matches!(
            session.stop(),
            Err(CaptureError::Take(TakeError::ChunkRejected(_)))
        ));
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(session.pending_audio_bytes(), 6);
        assert_eq!(session.take().pcm(), recorded);
        assert_eq!(session.retained_error(), retained);
        assert_eq!(session.drain(), Err(retained.clone().unwrap()));
        assert_eq!(session.stop(), Err(retained.clone().unwrap()));
        assert_eq!(session.disconnect(), Err(retained.clone().unwrap()));
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(session.take().pcm(), recorded);
        assert_eq!(session.pending_audio_bytes(), 6);
        assert_eq!(
            session.take_pending_events().unwrap(),
            vec![PendingCaptureEvent::Audio(too_short.to_vec())]
        );
        assert_eq!(session.pending_audio_bytes(), 0);

        let directory = tempfile::tempdir().unwrap();
        let saved_path = directory.path().join("recovered.aiff");
        let saved = session.take().save_aiff(&saved_path).unwrap();
        let saved_bytes = std::fs::read(saved.aiff_path).unwrap();
        assert!(!saved_bytes.is_empty());
        session.cancel().unwrap();
        session.cancel().unwrap();
        assert_eq!(session.take().phase(), Phase::Cancelled);
        assert!(session.take().pcm().is_empty());
        assert_eq!(std::fs::read(saved_path).unwrap(), saved_bytes);
        assert_eq!(session.retained_error(), retained);
    }

    #[test]
    fn successful_stop_drain_and_disconnect_retries_are_idempotent() {
        let mut session = CaptureSession::start(file_plan(), 13).unwrap();
        let bytes = [9, 0, 8, 0];
        session.callback_chunk(13, &bytes).unwrap();
        session.stop().unwrap();
        let recorded = session.take().pcm().to_vec();
        assert_eq!(recorded, bytes);
        assert_eq!(session.drain().unwrap(), 0);
        session.stop().unwrap();
        session.disconnect().unwrap();
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(session.take().pcm(), recorded);
        assert_eq!(session.pending_events(), 0);
        assert_eq!(session.retained_error(), None);
        assert_eq!(
            session.callback_chunk(13, &[1, 0, 2, 0]),
            Err(CaptureError::Stopped)
        );
        assert_eq!(session.take().pcm(), recorded);
    }

    #[test]
    fn disconnect_retry_keeps_disconnected_as_the_stable_terminal_cause() {
        let mut session = CaptureSession::start(file_plan(), 14).unwrap();
        let bytes = [3, 0, 4, 0];
        session.callback_chunk(14, &bytes).unwrap();
        assert_eq!(session.disconnect(), Err(CaptureError::Disconnected));
        let recorded = session.take().pcm().to_vec();
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(session.drain(), Err(CaptureError::Disconnected));
        assert_eq!(session.stop(), Err(CaptureError::Disconnected));
        assert_eq!(session.disconnect(), Err(CaptureError::Disconnected));
        assert_eq!(session.take().pcm(), recorded);
        assert_eq!(session.retained_error(), Some(CaptureError::Disconnected));
    }

    #[test]
    fn cap_failures_freeze_input_but_keep_recorded_and_queued_work_recoverable() {
        let plan =
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 1 }, 8).unwrap();
        let mut session = CaptureSession::start(plan, 20).unwrap();
        let recorded = [1, 0, 2, 0];
        session.callback_chunk(20, &recorded).unwrap();
        session.drain().unwrap();
        assert_eq!(
            session.callback_chunk(20, &[3, 0, 4, 0, 5, 0, 6, 0]),
            Err(CaptureError::Take(TakeError::CapExceeded))
        );
        assert!(session.stop().is_err());
        assert_eq!(session.take().pcm(), &recorded);
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(
            session.callback_chunk(20, &[7, 0, 8, 0]),
            Err(CaptureError::Take(TakeError::CapExceeded))
        );
        session.cancel().unwrap();
        assert_eq!(session.take().phase(), Phase::Cancelled);

        let plan = ValidatedPlan::software_file(
            44_100,
            2,
            ChannelMap { left: 0, right: 1 },
            QUEUE_CAPACITY_BYTES as u64,
        )
        .unwrap();
        let mut session = CaptureSession::start(plan, 21).unwrap();
        let chunk = vec![0x5a; super::super::STREAM_CHUNK_BYTES_MAX];
        for _ in 0..QUEUE_CAPACITY_BYTES / chunk.len() {
            session.callback_chunk(21, &chunk).unwrap();
        }
        assert_eq!(
            session.callback_chunk(21, &chunk),
            Err(CaptureError::Take(TakeError::CapExceeded))
        );
        assert!(session.stop().is_err());
        assert_eq!(session.take().pcm().len(), QUEUE_CAPACITY_BYTES);
        assert!(session.take().pcm().iter().all(|byte| *byte == 0x5a));
        assert_eq!(session.queued_bytes(), 0);
        assert_eq!(session.take().phase(), Phase::Stopped);
    }

    #[test]
    fn overflow_gap_that_exceeds_take_cap_is_terminal_without_losing_queued_audio() {
        let plan = ValidatedPlan::software_file(
            44_100,
            2,
            ChannelMap { left: 0, right: 1 },
            (QUEUE_CAPACITY_BYTES + 4) as u64,
        )
        .unwrap();
        let mut session = CaptureSession::start(plan, 22).unwrap();
        let chunk = vec![0x6b; super::super::STREAM_CHUNK_BYTES_MAX];
        for _ in 0..QUEUE_CAPACITY_BYTES / chunk.len() {
            session.callback_chunk(22, &chunk).unwrap();
        }
        assert_eq!(
            session.callback_chunk(22, &chunk),
            Err(CaptureError::Take(TakeError::CapExceeded))
        );
        assert_eq!(session.pending_audio_bytes(), QUEUE_CAPACITY_BYTES);
        assert!(session.stop().is_err());
        assert_eq!(session.take().phase(), Phase::Stopped);
        assert_eq!(session.take().pcm().len(), QUEUE_CAPACITY_BYTES);
        assert_eq!(session.pending_audio_bytes(), 0);
        assert_eq!(session.take().pcm()[0], 0x6b);
    }

    #[test]
    fn invalid_shapes_external_routes_late_callbacks_and_lifecycle_are_explicit() {
        let stereo = ChannelMap { left: 0, right: 1 };
        assert_eq!(
            ValidatedPlan::software_file(44_100, 1, stereo, 1024),
            Err(CaptureError::InvalidChannels)
        );
        assert_eq!(
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 0 }, 1024),
            Err(CaptureError::InvalidChannelMap)
        );
        assert_eq!(
            ValidatedPlan::software_file(44_101, 2, stereo, 1024),
            Err(CaptureError::InvalidClock)
        );
        assert_eq!(
            ValidatedPlan::external(InterfaceIdentity::External {
                vendor: 1,
                product: 2,
                interface: 0,
            }),
            Err(CaptureError::ExternalContractMissing)
        );

        for generation in 40..43 {
            let mut session = CaptureSession::start(file_plan(), generation).unwrap();
            assert_eq!(
                session.callback_chunk(generation - 1, &[1, 0, 2, 0]),
                Err(CaptureError::StaleGeneration)
            );
            session.callback_chunk(generation, &[1, 0, 2, 0]).unwrap();
            session.drain().unwrap();
            session.stop().unwrap();
            assert_eq!(session.take().frames(), 1);
            session.cancel().unwrap();
            assert_eq!(session.take().phase(), Phase::Cancelled);
        }
    }

    #[test]
    fn wrong_thread_disconnect_does_not_mutate_and_old_generations_are_rejected() {
        let mut session = CaptureSession::start(file_plan(), 30).unwrap();
        session.callback_chunk(30, &[1, 0, 2, 0]).unwrap();
        let (mut session, result) = std::thread::spawn(move || {
            let result = session.disconnect();
            (session, result)
        })
        .join()
        .unwrap();
        assert_eq!(result, Err(CaptureError::WrongThread));
        assert_eq!(session.pending_audio_bytes(), 4);
        assert_eq!(session.pending_events(), 1);
        assert_eq!(session.retained_error(), None);
        session.drain().unwrap();
        session.stop().unwrap();

        for generation in 31..34 {
            let mut restarted = CaptureSession::start(file_plan(), generation).unwrap();
            assert_eq!(
                restarted.callback_chunk(generation - 1, &[1, 0, 2, 0]),
                Err(CaptureError::StaleGeneration)
            );
            restarted.callback_chunk(generation, &[1, 0, 2, 0]).unwrap();
            restarted.drain().unwrap();
            restarted.stop().unwrap();
            assert_eq!(restarted.take().frames(), 1);
        }
    }
}

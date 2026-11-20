//! Software-only model of an ASIO-style double-buffer callback boundary.
//!
//! This module owns its sample buffers and translates explicit simulated
//! notifications into the common `CaptureSession` queue. It does not declare
//! or emulate a native ABI, load vendor code, or admit a physical interface.

use crate::pod_audio::RecordRoute;
use crate::recording::capture::{
    CaptureError, CaptureSession, ChannelMap, InterfaceIdentity, MAX_CHANNELS, PendingCaptureEvent,
    QUEUE_CAPACITY_BYTES, ValidatedPlan,
};
use crate::recording::clock::ClockMeasurement;
use crate::recording::{Phase, Take};

const MAX_BUFFER_BYTES: usize = QUEUE_CAPACITY_BYTES;
/// At most one source-order chunk plus one mapped stereo chunk coexist while
/// delivering a callback. This payload bound does not include pre-owned buffers.
pub const MAX_CALLBACK_STAGED_PCM_BYTES: usize = crate::recording::STREAM_CHUNK_BYTES_MAX * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelDescriptor {
    pub direction: ChannelDirection,
    /// Zero-based channel identity.
    pub channel: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationMode {
    Ordinary,
    TimeInfo,
}

/// Selector response `1` alone selects time-info notifications. Missing or
/// any other response selects ordinary notifications; it is not a device query.
pub const fn notification_mode(selector_response: Option<i32>) -> NotificationMode {
    if matches!(selector_response, Some(1)) {
        NotificationMode::TimeInfo
    } else {
        NotificationMode::Ordinary
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftwareTimeObservation {
    /// Caller-supplied software frame count and monotonic nanoseconds.
    pub cumulative_frames: u64,
    pub monotonic_ns: u64,
    /// Textual provenance for the software clock source.
    pub provenance: &'static str,
    /// Frames known missing since the previous observation.
    pub dropout_frames: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimulatedNotification {
    Ordinary {
        half: u8,
    },
    TimeInfo {
        half: u8,
        observation: Option<SoftwareTimeObservation>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Prepared,
    Running,
    Stopped,
    Disposed,
    Disconnected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfflineAsioError {
    Capture(CaptureError),
    NotSoftwareSynthetic,
    InvalidDescriptor,
    DuplicateChannel(u8),
    MissingChannel(u8),
    OutputChannel(u8),
    InvalidHalf(u8),
    InvalidBufferLength,
    BufferTooLarge,
    CallbackUnavailable(NotificationMode),
    NotificationModeMismatch,
    TimeUnavailable,
    InvalidTimeProvenance,
    UnsupportedRoute(RecordRoute),
    NotRunning,
    Disposed,
}

impl From<CaptureError> for OfflineAsioError {
    fn from(value: CaptureError) -> Self {
        Self::Capture(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OwnedChannel {
    descriptor: ChannelDescriptor,
    halves: [Vec<u8>; 2],
}

/// Fake owned buffers are supplied by tests/callers. Each channel has two
/// equal-length S16LE halves; callback delivery borrows them only while copying
/// into the common bounded session queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineAsioAdapter {
    session: CaptureSession,
    channels: Vec<OwnedChannel>,
    mode: NotificationMode,
    ordinary_callback_available: bool,
    time_info_callback_available: bool,
    lifecycle: Lifecycle,
    retained_error: Option<OfflineAsioError>,
}

impl OfflineAsioAdapter {
    pub fn prepare(
        plan: ValidatedPlan,
        descriptors: Vec<ChannelDescriptor>,
        halves: Vec<[Vec<u8>; 2]>,
        selector_response: Option<i32>,
        ordinary_callback_available: bool,
        time_info_callback_available: bool,
        generation: u64,
    ) -> Result<Self, OfflineAsioError> {
        plan.revalidate()?;
        let contract = plan.contract();
        if contract.identity != InterfaceIdentity::SoftwareSynthetic
            || contract.channels == 0
            || contract.channels > MAX_CHANNELS
            || contract.format != crate::rig::audio::SampleFormat::S16Le
            || contract.clock.1 != 1
        {
            return Err(OfflineAsioError::NotSoftwareSynthetic);
        }
        if matches!(
            plan.signal_route(),
            RecordRoute::SendCleanGuitarReampPlayback | RecordRoute::SendReampPlayback
        ) {
            return Err(OfflineAsioError::UnsupportedRoute(plan.signal_route()));
        }
        if descriptors.len() != usize::from(contract.channels) || halves.len() != descriptors.len()
        {
            return Err(OfflineAsioError::InvalidDescriptor);
        }
        let mut channels = Vec::with_capacity(descriptors.len());
        let mut seen = [false; MAX_CHANNELS as usize];
        let mut total_bytes = 0usize;
        let mut expected_len = None;
        for (descriptor, buffers) in descriptors.into_iter().zip(halves) {
            if descriptor.channel >= MAX_CHANNELS {
                return Err(OfflineAsioError::InvalidDescriptor);
            }
            if descriptor.direction == ChannelDirection::Output {
                return Err(OfflineAsioError::OutputChannel(descriptor.channel));
            }
            let index = usize::from(descriptor.channel);
            if seen[index] {
                return Err(OfflineAsioError::DuplicateChannel(descriptor.channel));
            }
            seen[index] = true;
            if buffers[0].is_empty()
                || buffers[0].len() != buffers[1].len()
                || !buffers[0].len().is_multiple_of(2)
            {
                return Err(OfflineAsioError::InvalidBufferLength);
            }
            if let Some(len) = expected_len {
                if buffers[0].len() != len {
                    return Err(OfflineAsioError::InvalidBufferLength);
                }
            } else {
                expected_len = Some(buffers[0].len());
            }
            total_bytes = total_bytes
                .checked_add(buffers[0].len())
                .and_then(|n| n.checked_add(buffers[1].len()))
                .ok_or(OfflineAsioError::BufferTooLarge)?;
            if total_bytes > MAX_BUFFER_BYTES {
                return Err(OfflineAsioError::BufferTooLarge);
            }
            channels.push(OwnedChannel {
                descriptor,
                halves: buffers,
            });
        }
        for channel in 0..contract.channels {
            if !seen[usize::from(channel)] {
                return Err(OfflineAsioError::MissingChannel(channel));
            }
        }
        let source_half_bytes = expected_len
            .and_then(|bytes| bytes.checked_mul(usize::from(contract.channels)))
            .ok_or(OfflineAsioError::BufferTooLarge)?;
        if source_half_bytes > crate::recording::STREAM_CHUNK_BYTES_MAX {
            return Err(OfflineAsioError::BufferTooLarge);
        }
        validate_map(contract.channel_map, contract.channels)?;
        let mode = notification_mode(selector_response);
        let available = match mode {
            NotificationMode::Ordinary => ordinary_callback_available,
            NotificationMode::TimeInfo => time_info_callback_available,
        };
        if !available {
            return Err(OfflineAsioError::CallbackUnavailable(mode));
        }
        Ok(Self {
            session: CaptureSession::start(plan, generation)?,
            channels,
            mode,
            ordinary_callback_available,
            time_info_callback_available,
            lifecycle: Lifecycle::Prepared,
            retained_error: None,
        })
    }

    pub fn start(&mut self) -> Result<(), OfflineAsioError> {
        self.session.check_owner()?;
        match self.lifecycle {
            Lifecycle::Prepared => {
                self.lifecycle = Lifecycle::Running;
                Ok(())
            }
            Lifecycle::Disposed => Err(OfflineAsioError::Disposed),
            _ => Err(OfflineAsioError::NotRunning),
        }
    }

    pub fn deliver(
        &mut self,
        generation: u64,
        notification: SimulatedNotification,
    ) -> Result<Option<ClockMeasurement>, OfflineAsioError> {
        let result = self.deliver_inner(generation, notification);
        if let Err(error) = &result
            && *error != OfflineAsioError::Capture(CaptureError::WrongThread)
        {
            self.retained_error.get_or_insert_with(|| error.clone());
        }
        result
    }

    fn deliver_inner(
        &mut self,
        generation: u64,
        notification: SimulatedNotification,
    ) -> Result<Option<ClockMeasurement>, OfflineAsioError> {
        if self.lifecycle == Lifecycle::Disposed {
            return Err(OfflineAsioError::Disposed);
        }
        if self.lifecycle != Lifecycle::Running {
            return Err(OfflineAsioError::NotRunning);
        }
        let (half, observation, observed_mode) = match notification {
            SimulatedNotification::Ordinary { half } => (half, None, NotificationMode::Ordinary),
            SimulatedNotification::TimeInfo { half, observation } => {
                (half, observation, NotificationMode::TimeInfo)
            }
        };
        if half > 1 {
            return Err(OfflineAsioError::InvalidHalf(half));
        }
        if observed_mode != self.mode {
            return Err(OfflineAsioError::NotificationModeMismatch);
        }
        if self.mode == NotificationMode::Ordinary && !self.ordinary_callback_available
            || self.mode == NotificationMode::TimeInfo && !self.time_info_callback_available
        {
            return Err(OfflineAsioError::CallbackUnavailable(self.mode));
        }
        if self.mode == NotificationMode::TimeInfo {
            let time = observation.ok_or(OfflineAsioError::TimeUnavailable)?;
            if time.provenance.trim().is_empty() {
                return Err(OfflineAsioError::InvalidTimeProvenance);
            }
        }
        self.session.check_callback_admission(generation)?;
        let bytes = self.channel_chunk(half)?;
        let software_clock = observation.map(|time| {
            (
                time.cumulative_frames,
                time.monotonic_ns,
                time.dropout_frames,
            )
        });
        Ok(self
            .session
            .callback_chunk_atomic(generation, &bytes, software_clock)?)
    }

    fn channel_chunk(&self, half: u8) -> Result<Vec<u8>, OfflineAsioError> {
        let channels = self.session.plan().channels;
        let frames = self.channel(0, half)?.len() / 2;
        let frame_bytes = usize::from(channels)
            .checked_mul(2)
            .ok_or(OfflineAsioError::BufferTooLarge)?;
        let out_len = frames
            .checked_mul(frame_bytes)
            .ok_or(OfflineAsioError::BufferTooLarge)?;
        if out_len > crate::recording::STREAM_CHUNK_BYTES_MAX {
            return Err(OfflineAsioError::BufferTooLarge);
        }
        let mut output = Vec::with_capacity(out_len);
        for frame in 0..frames {
            let sample = frame * 2;
            for channel in 0..channels {
                let source = self.channel(channel, half)?;
                output.extend_from_slice(&source[sample..sample + 2]);
            }
        }
        Ok(output)
    }

    fn channel(&self, index: u8, half: u8) -> Result<&[u8], OfflineAsioError> {
        self.channels
            .iter()
            .find(|channel| channel.descriptor.channel == index)
            .map(|channel| channel.halves[usize::from(half)].as_slice())
            .ok_or(OfflineAsioError::MissingChannel(index))
    }

    pub fn drain(&mut self) -> Result<usize, CaptureError> {
        self.session.drain()
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        self.session.check_owner()?;
        if self.lifecycle == Lifecycle::Disposed {
            return Err(CaptureError::Stopped);
        }
        let result = self.session.stop();
        if self.session.take().phase() != Phase::Recording {
            self.lifecycle = Lifecycle::Stopped;
        }
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn cancel(&mut self) -> Result<(), CaptureError> {
        self.session.check_owner()?;
        if self.lifecycle == Lifecycle::Disposed {
            return Err(CaptureError::Stopped);
        }
        let result = self.session.cancel();
        if result.is_ok() {
            self.lifecycle = Lifecycle::Stopped;
        }
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn disconnect(&mut self) -> Result<(), CaptureError> {
        self.session.check_owner()?;
        if self.lifecycle == Lifecycle::Disposed {
            return Err(CaptureError::Stopped);
        }
        let result = self.session.disconnect();
        if self.session.take().phase() != Phase::Recording {
            self.lifecycle = Lifecycle::Disconnected;
        }
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn dispose(&mut self) -> Result<(), OfflineAsioError> {
        self.session.check_owner()?;
        if self.lifecycle == Lifecycle::Disposed {
            return Ok(());
        }
        if self.session.take().phase() == Phase::Recording
            && let Err(error) = self.session.stop()
        {
            self.retained_error
                .get_or_insert(OfflineAsioError::Capture(error));
            if self.session.take().phase() == Phase::Recording {
                return Err(self
                    .retained_error
                    .clone()
                    .unwrap_or(OfflineAsioError::NotRunning));
            }
            self.lifecycle = Lifecycle::Disposed;
            return Err(self
                .retained_error
                .clone()
                .unwrap_or(OfflineAsioError::NotRunning));
        }
        self.lifecycle = Lifecycle::Disposed;
        Ok(())
    }

    pub fn session(&self) -> &CaptureSession {
        &self.session
    }

    pub fn take(&self) -> &Take {
        self.session.take()
    }

    pub fn take_pending_events(&mut self) -> Result<Vec<PendingCaptureEvent>, CaptureError> {
        self.session.take_pending_events()
    }

    pub const fn mode(&self) -> NotificationMode {
        self.mode
    }

    pub const fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    pub fn retained_error(&self) -> Option<&OfflineAsioError> {
        self.retained_error.as_ref()
    }
}

fn validate_map(map: ChannelMap, channels: u8) -> Result<(), OfflineAsioError> {
    if map.left >= channels || map.right >= channels || map.left == map.right {
        return Err(OfflineAsioError::InvalidDescriptor);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod_audio::RecordRoute;

    fn descriptors() -> Vec<ChannelDescriptor> {
        (0..2)
            .map(|channel| ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel,
            })
            .collect()
    }

    fn halves() -> Vec<[Vec<u8>; 2]> {
        vec![
            [vec![1, 0, 2, 0], vec![3, 0, 4, 0]],
            [vec![11, 0, 12, 0], vec![13, 0, 14, 0]],
        ]
    }

    fn plan() -> ValidatedPlan {
        plan_with(
            2,
            ChannelMap { left: 0, right: 1 },
            RecordRoute::SendProcessedGuitar,
        )
    }

    fn plan_with(channels: u8, map: ChannelMap, route: RecordRoute) -> ValidatedPlan {
        ValidatedPlan::software_synthetic(48_000, channels, map, route, 4096).unwrap()
    }

    fn prepare(response: Option<i32>) -> OfflineAsioAdapter {
        prepare_generation(response, 7)
    }

    fn prepare_generation(response: Option<i32>, generation: u64) -> OfflineAsioAdapter {
        OfflineAsioAdapter::prepare(
            plan(),
            descriptors(),
            halves(),
            response,
            true,
            true,
            generation,
        )
        .unwrap()
    }

    #[test]
    fn selector_response_and_callback_presence_choose_only_software_mode() {
        for (response, expected) in [
            (None, NotificationMode::Ordinary),
            (Some(0), NotificationMode::Ordinary),
            (Some(1), NotificationMode::TimeInfo),
            (Some(2), NotificationMode::Ordinary),
            (Some(-1), NotificationMode::Ordinary),
        ] {
            assert_eq!(notification_mode(response), expected);
            assert_eq!(prepare(response).mode(), expected);
        }
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), descriptors(), halves(), Some(1), true, false, 7),
            Err(OfflineAsioError::CallbackUnavailable(
                NotificationMode::TimeInfo
            ))
        );
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), descriptors(), halves(), None, false, true, 7),
            Err(OfflineAsioError::CallbackUnavailable(
                NotificationMode::Ordinary
            ))
        );
        let file_plan =
            ValidatedPlan::software_file(48_000, 2, ChannelMap { left: 0, right: 1 }, 1024)
                .unwrap();
        assert_eq!(
            OfflineAsioAdapter::prepare(file_plan, descriptors(), halves(), None, true, true, 7,),
            Err(OfflineAsioError::NotSoftwareSynthetic)
        );
    }

    #[test]
    fn literal_channels_and_both_halves_reach_common_take_in_stereo_order() {
        let mut adapter = prepare(None);
        adapter.start().unwrap();
        adapter
            .deliver(7, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        adapter
            .deliver(7, SimulatedNotification::Ordinary { half: 1 })
            .unwrap();
        assert_eq!(adapter.session().queued_bytes(), 16);
        assert_eq!(adapter.drain().unwrap(), 2);
        assert_eq!(
            adapter.take().pcm(),
            &[1, 0, 11, 0, 2, 0, 12, 0, 3, 0, 13, 0, 4, 0, 14, 0]
        );
        adapter.stop().unwrap();
        assert_eq!(adapter.lifecycle(), Lifecycle::Stopped);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("offline-callback.wav");
        adapter.take().save(&path).unwrap();
        let wav = std::fs::read(path).unwrap();
        assert_eq!(
            &wav[44..],
            &[1, 0, 11, 0, 2, 0, 12, 0, 3, 0, 13, 0, 4, 0, 14, 0]
        );
        assert_eq!(
            adapter.deliver(7, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::NotRunning)
        );
    }

    #[test]
    fn reversed_stereo_and_shuffled_descriptors_map_once_through_take_and_wav() {
        let descriptors = vec![
            ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel: 1,
            },
            ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel: 0,
            },
        ];
        let buffers = vec![
            [vec![0x22, 0x22, 0x44, 0x44], vec![0x66, 0x66, 0x88, 0x88]],
            [vec![0x11, 0x11, 0x33, 0x33], vec![0x55, 0x55, 0x77, 0x77]],
        ];
        let mut adapter = OfflineAsioAdapter::prepare(
            plan_with(
                2,
                ChannelMap { left: 1, right: 0 },
                RecordRoute::SendCleanGuitar,
            ),
            descriptors,
            buffers,
            None,
            true,
            true,
            18,
        )
        .unwrap();
        adapter.start().unwrap();
        adapter
            .deliver(18, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        adapter.drain().unwrap();
        assert_eq!(
            adapter.take().pcm(),
            &[0x22, 0x22, 0x11, 0x11, 0x44, 0x44, 0x33, 0x33]
        );
        adapter.stop().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let wav_path = directory.path().join("reversed.wav");
        adapter.take().save(&wav_path).unwrap();
        let wav = std::fs::read(wav_path).unwrap();
        assert_eq!(
            &wav[44..],
            &[0x22, 0x22, 0x11, 0x11, 0x44, 0x44, 0x33, 0x33]
        );
    }

    #[test]
    fn multichannel_source_order_selects_nontrivial_map_exactly_once() {
        let descriptors = vec![
            ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel: 2,
            },
            ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel: 0,
            },
            ChannelDescriptor {
                direction: ChannelDirection::Input,
                channel: 1,
            },
        ];
        let buffers = vec![
            [vec![21, 0, 22, 0], vec![31, 0, 32, 0]],
            [vec![1, 0, 2, 0], vec![11, 0, 12, 0]],
            [vec![101, 0, 102, 0], vec![111, 0, 112, 0]],
        ];
        let mut adapter = OfflineAsioAdapter::prepare(
            plan_with(
                3,
                ChannelMap { left: 2, right: 0 },
                RecordRoute::SendProcessedGuitar,
            ),
            descriptors,
            buffers,
            None,
            true,
            true,
            19,
        )
        .unwrap();
        adapter.start().unwrap();
        adapter
            .deliver(19, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        adapter.drain().unwrap();
        assert_eq!(adapter.take().pcm(), &[21, 0, 1, 0, 22, 0, 2, 0]);
        adapter.stop().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let wav_path = directory.path().join("multichannel.wav");
        adapter.take().save(&wav_path).unwrap();
        let wav = std::fs::read(wav_path).unwrap();
        assert_eq!(&wav[44..], &[21, 0, 1, 0, 22, 0, 2, 0]);
    }

    #[test]
    fn callback_staging_bound_does_not_grow_with_drained_take() {
        let mut adapter = prepare(None);
        adapter.start().unwrap();
        for _ in 0..256 {
            adapter
                .deliver(7, SimulatedNotification::Ordinary { half: 0 })
                .unwrap();
            adapter.drain().unwrap();
        }
        assert!(adapter.take().pcm().len() > 1024);
        assert_eq!(
            MAX_CALLBACK_STAGED_PCM_BYTES,
            crate::recording::STREAM_CHUNK_BYTES_MAX * 2
        );
        adapter.stop().unwrap();
    }

    #[test]
    fn bad_modes_halves_and_timing_are_rejected_before_queue_or_take_changes() {
        let mut adapter = prepare(Some(1));
        adapter.start().unwrap();
        assert_eq!(
            adapter.deliver(7, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::NotificationModeMismatch)
        );
        assert_eq!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 2,
                    observation: None,
                }
            ),
            Err(OfflineAsioError::InvalidHalf(2))
        );
        assert_eq!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 0,
                    observation: Some(SoftwareTimeObservation {
                        cumulative_frames: 1,
                        monotonic_ns: 10,
                        provenance: "test software clock",
                        dropout_frames: 0,
                    }),
                }
            ),
            Err(OfflineAsioError::Capture(CaptureError::Clock(
                crate::recording::clock::ClockError::FirstFrameMustBeZero
            )))
        );
        assert_eq!(adapter.session().pending_events(), 0);
        assert!(adapter.take().pcm().is_empty());
        assert!(adapter.session().clock_measurement().is_none());
    }

    #[test]
    fn time_info_uses_explicit_software_observations_and_existing_tracker() {
        let mut adapter = prepare(Some(1));
        adapter.start().unwrap();
        let origin = SoftwareTimeObservation {
            cumulative_frames: 0,
            monotonic_ns: 50,
            provenance: "caller synthetic monotonic clock",
            dropout_frames: 0,
        };
        assert!(
            adapter
                .deliver(
                    7,
                    SimulatedNotification::TimeInfo {
                        half: 0,
                        observation: Some(origin),
                    }
                )
                .unwrap()
                .is_some()
        );
        let next = SoftwareTimeObservation {
            cumulative_frames: 2,
            monotonic_ns: 50 + 41_666,
            provenance: "caller synthetic monotonic clock",
            dropout_frames: 0,
        };
        assert!(
            adapter
                .deliver(
                    7,
                    SimulatedNotification::TimeInfo {
                        half: 1,
                        observation: Some(next),
                    }
                )
                .unwrap()
                .is_some()
        );
        let accepted_clock = adapter.session().clock_measurement();
        assert_eq!(adapter.session().pending_events(), 2);
        assert_eq!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 0,
                    observation: Some(SoftwareTimeObservation {
                        cumulative_frames: 1,
                        monotonic_ns: 100_000,
                        provenance: "caller synthetic monotonic clock",
                        dropout_frames: 0,
                    }),
                }
            ),
            Err(OfflineAsioError::Capture(CaptureError::Clock(
                crate::recording::clock::ClockError::NonMonotonicFrames
            )))
        );
        assert_eq!(adapter.session().pending_events(), 2);
        assert_eq!(adapter.session().clock_measurement(), accepted_clock);
        adapter.drain().unwrap();
        assert_eq!(adapter.take().pcm().len(), 16);
        assert_eq!(adapter.session().plan().clock, (48_000, 1));
    }

    #[test]
    fn descriptors_and_buffers_are_checked_before_session_start() {
        let mut bad = descriptors();
        bad[1].channel = 0;
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), bad, halves(), None, true, true, 9),
            Err(OfflineAsioError::DuplicateChannel(0))
        );
        let mut output = descriptors();
        output[1].direction = ChannelDirection::Output;
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), output, halves(), None, true, true, 9),
            Err(OfflineAsioError::OutputChannel(1))
        );
        for route in [
            RecordRoute::SendCleanGuitarReampPlayback,
            RecordRoute::SendReampPlayback,
        ] {
            assert_eq!(
                OfflineAsioAdapter::prepare(
                    plan_with(2, ChannelMap { left: 0, right: 1 }, route),
                    descriptors(),
                    halves(),
                    None,
                    true,
                    true,
                    9,
                ),
                Err(OfflineAsioError::UnsupportedRoute(route))
            );
        }
        let mut missing = descriptors();
        missing[1].channel = 2;
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), missing, halves(), None, true, true, 9),
            Err(OfflineAsioError::MissingChannel(1))
        );
        let mut malformed = halves();
        malformed[1][1].pop();
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), descriptors(), malformed, None, true, true, 9),
            Err(OfflineAsioError::InvalidBufferLength)
        );
        let mut partial_sample = halves();
        partial_sample[0][0].push(0);
        assert_eq!(
            OfflineAsioAdapter::prepare(plan(), descriptors(), partial_sample, None, true, true, 9,),
            Err(OfflineAsioError::InvalidBufferLength)
        );
        let long_half = vec![0; crate::recording::STREAM_CHUNK_BYTES_MAX];
        assert_eq!(
            OfflineAsioAdapter::prepare(
                plan(),
                descriptors(),
                vec![
                    [long_half.clone(), long_half.clone()],
                    [long_half.clone(), long_half]
                ],
                None,
                true,
                true,
                9,
            ),
            Err(OfflineAsioError::BufferTooLarge)
        );
    }

    #[test]
    fn stale_generation_stop_and_dispose_never_publish_late_buffers() {
        let mut adapter = prepare(None);
        adapter.start().unwrap();
        assert_eq!(
            adapter.deliver(8, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::Capture(CaptureError::StaleGeneration))
        );
        assert_eq!(adapter.session().pending_events(), 0);
        assert_eq!(
            adapter.retained_error(),
            Some(&OfflineAsioError::Capture(CaptureError::StaleGeneration))
        );
        // The stale generation is retained as a session error, so validate the
        // lifecycle boundary on a fresh adapter instance.
        let mut stopped = prepare(None);
        stopped.start().unwrap();
        stopped.stop().unwrap();
        assert_eq!(
            stopped.deliver(7, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::NotRunning)
        );
        let mut disposed = prepare(None);
        disposed.dispose().unwrap();
        disposed.dispose().unwrap();
        assert_eq!(
            disposed.deliver(7, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::Disposed)
        );
    }

    #[test]
    fn backpressure_failure_keeps_prior_queue_and_take_unchanged() {
        let channel_half = vec![0x2a; 4096];
        let buffers = vec![
            [channel_half.clone(), channel_half.clone()],
            [channel_half.clone(), channel_half],
        ];
        let mut adapter = OfflineAsioAdapter::prepare(
            ValidatedPlan::software_synthetic(
                48_000,
                2,
                ChannelMap { left: 0, right: 1 },
                crate::pod_audio::RecordRoute::SendProcessedGuitar,
                1024 * 1024,
            )
            .unwrap(),
            descriptors(),
            buffers,
            None,
            true,
            true,
            12,
        )
        .unwrap();
        adapter.start().unwrap();
        for _ in 0..8 {
            adapter
                .deliver(12, SimulatedNotification::Ordinary { half: 0 })
                .unwrap();
        }
        assert_eq!(adapter.session().queued_bytes(), QUEUE_CAPACITY_BYTES);
        let before = adapter.take().pcm().to_vec();
        assert_eq!(
            adapter.deliver(12, SimulatedNotification::Ordinary { half: 1 }),
            Err(OfflineAsioError::Capture(CaptureError::QueueFull))
        );
        assert_eq!(adapter.session().queued_bytes(), QUEUE_CAPACITY_BYTES);
        assert_eq!(adapter.session().pending_events(), 8);
        assert_eq!(adapter.take().pcm(), before.as_slice());
        assert_eq!(
            adapter.retained_error(),
            Some(&OfflineAsioError::Capture(CaptureError::QueueFull))
        );
        adapter.drain().unwrap();
        assert_eq!(adapter.session().pending_events(), 0);
        assert_eq!(adapter.take().pcm().len(), QUEUE_CAPACITY_BYTES);
        adapter
            .deliver(12, SimulatedNotification::Ordinary { half: 1 })
            .unwrap();
        assert_eq!(adapter.session().queued_bytes(), 8192);
    }

    #[test]
    fn missing_or_discontinuous_time_stays_unavailable_without_publication() {
        let mut adapter = prepare(Some(1));
        adapter.start().unwrap();
        assert_eq!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 0,
                    observation: None,
                }
            ),
            Err(OfflineAsioError::TimeUnavailable)
        );
        let invalid = SoftwareTimeObservation {
            cumulative_frames: 0,
            monotonic_ns: 1,
            provenance: "  ",
            dropout_frames: 0,
        };
        assert_eq!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 0,
                    observation: Some(invalid),
                }
            ),
            Err(OfflineAsioError::InvalidTimeProvenance)
        );
        let origin = SoftwareTimeObservation {
            cumulative_frames: 0,
            monotonic_ns: 10,
            provenance: "synthetic clock",
            dropout_frames: 0,
        };
        adapter
            .deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 0,
                    observation: Some(origin),
                },
            )
            .unwrap();
        let nonmonotonic = SoftwareTimeObservation {
            cumulative_frames: 2,
            monotonic_ns: 10,
            provenance: "synthetic clock",
            dropout_frames: 0,
        };
        assert!(matches!(
            adapter.deliver(
                7,
                SimulatedNotification::TimeInfo {
                    half: 1,
                    observation: Some(nonmonotonic),
                }
            ),
            Err(OfflineAsioError::Capture(CaptureError::Clock(_)))
        ));
        assert_eq!(adapter.session().pending_events(), 1);
        assert!(adapter.take().pcm().is_empty());
        assert_eq!(
            adapter.session().clock_measurement().unwrap().frame_index,
            0
        );
    }

    #[test]
    fn wrong_owner_lifecycle_attempts_preserve_state_and_accepted_data() {
        let adapter = prepare(None);
        let (result, lifecycle, mut adapter) = std::thread::spawn(move || {
            let mut adapter = adapter;
            let result = adapter.start();
            (result, adapter.lifecycle(), adapter)
        })
        .join()
        .unwrap();
        assert_eq!(
            result,
            Err(OfflineAsioError::Capture(CaptureError::WrongThread))
        );
        assert_eq!(lifecycle, Lifecycle::Prepared);
        assert_eq!(adapter.lifecycle(), Lifecycle::Prepared);
        adapter.start().unwrap();
        adapter
            .deliver(7, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();

        let (attempts, lifecycle, retained_error, mut adapter) = std::thread::spawn(move || {
            let mut adapter = adapter;
            let attempts = (
                adapter.start(),
                adapter.stop(),
                adapter.cancel(),
                adapter.disconnect(),
                adapter.dispose(),
                adapter.deliver(7, SimulatedNotification::Ordinary { half: 1 }),
            );
            (
                attempts,
                adapter.lifecycle(),
                adapter.retained_error().cloned(),
                adapter,
            )
        })
        .join()
        .unwrap();
        assert_eq!(
            attempts.0,
            Err(OfflineAsioError::Capture(CaptureError::WrongThread))
        );
        assert_eq!(attempts.1, Err(CaptureError::WrongThread));
        assert_eq!(attempts.2, Err(CaptureError::WrongThread));
        assert_eq!(attempts.3, Err(CaptureError::WrongThread));
        assert_eq!(
            attempts.4,
            Err(OfflineAsioError::Capture(CaptureError::WrongThread))
        );
        assert_eq!(
            attempts.5,
            Err(OfflineAsioError::Capture(CaptureError::WrongThread))
        );
        assert_eq!(lifecycle, Lifecycle::Running);
        assert_eq!(adapter.lifecycle(), Lifecycle::Running);
        assert_eq!(retained_error, None);
        assert_eq!(adapter.session().pending_events(), 1);
        adapter
            .deliver(7, SimulatedNotification::Ordinary { half: 1 })
            .unwrap();
        adapter.stop().unwrap();
        assert_eq!(
            adapter.take().pcm(),
            &[1, 0, 11, 0, 2, 0, 12, 0, 3, 0, 13, 0, 4, 0, 14, 0]
        );
    }

    #[test]
    fn cancellation_disconnect_dispose_and_fresh_generation_are_deterministic() {
        let mut cancelled = prepare(None);
        cancelled.start().unwrap();
        cancelled
            .deliver(7, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        cancelled.cancel().unwrap();
        cancelled.cancel().unwrap();
        assert_eq!(cancelled.lifecycle(), Lifecycle::Stopped);
        assert!(cancelled.take().pcm().is_empty());

        let mut disconnected = prepare(None);
        disconnected.start().unwrap();
        disconnected
            .deliver(7, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        assert_eq!(disconnected.disconnect(), Err(CaptureError::Disconnected));
        assert_eq!(disconnected.lifecycle(), Lifecycle::Disconnected);
        assert_eq!(disconnected.take().pcm(), &[1, 0, 11, 0, 2, 0, 12, 0]);
        assert_eq!(disconnected.disconnect(), Err(CaptureError::Disconnected));

        let mut disposed = prepare(None);
        disposed.start().unwrap();
        disposed
            .deliver(7, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        disposed.dispose().unwrap();
        disposed.dispose().unwrap();
        assert_eq!(disposed.lifecycle(), Lifecycle::Disposed);
        assert_eq!(
            disposed.deliver(7, SimulatedNotification::Ordinary { half: 1 }),
            Err(OfflineAsioError::Disposed)
        );

        let mut old = prepare(None);
        old.start().unwrap();
        old.stop().unwrap();
        let mut restarted = prepare_generation(None, 8);
        restarted.start().unwrap();
        assert_eq!(
            restarted.deliver(7, SimulatedNotification::Ordinary { half: 0 }),
            Err(OfflineAsioError::Capture(CaptureError::StaleGeneration))
        );
        assert_eq!(restarted.session().pending_events(), 0);
        restarted
            .deliver(8, SimulatedNotification::Ordinary { half: 0 })
            .unwrap();
        restarted.stop().unwrap();
        assert_eq!(restarted.take().pcm(), &[1, 0, 11, 0, 2, 0, 12, 0]);
    }
}

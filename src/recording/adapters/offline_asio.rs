//! Software-only model of an ASIO-style double-buffer callback boundary.
//!
//! This module owns its sample buffers and translates explicit simulated
//! notifications into the common `CaptureSession` queue. It does not declare
//! or emulate a native ABI, load vendor code, or admit a physical interface.

use crate::recording::capture::{
    CaptureError, CaptureSession, ChannelMap, InterfaceIdentity, MAX_CHANNELS, PendingCaptureEvent,
    QUEUE_CAPACITY_BYTES, ValidatedPlan,
};
use crate::recording::clock::ClockMeasurement;
use crate::recording::{Phase, Take};

const MAX_BUFFER_BYTES: usize = QUEUE_CAPACITY_BYTES;

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
        if let Err(error) = &result {
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
        // Run both timing and queue mutation on a cloned session so malformed,
        // stale, discontinuous, or backpressured delivery cannot partially
        // change the accepted queue or clock state.
        let mut candidate = self.session.clone();
        let measurement = if let Some(time) = observation {
            Some(candidate.observe_software_clock(
                generation,
                time.cumulative_frames,
                time.monotonic_ns,
                time.dropout_frames,
            )?)
        } else {
            None
        };
        candidate.callback_chunk(generation, &bytes)?;
        self.session = candidate;
        Ok(measurement)
    }

    fn channel_chunk(&self, half: u8) -> Result<Vec<u8>, OfflineAsioError> {
        let map = self.session.plan().channel_map;
        let left = self.channel(map.left, half)?;
        let right = self.channel(map.right, half)?;
        let frames = left.len() / 2;
        let out_len = frames
            .checked_mul(4)
            .ok_or(OfflineAsioError::BufferTooLarge)?;
        if out_len > crate::recording::STREAM_CHUNK_BYTES_MAX {
            return Err(OfflineAsioError::BufferTooLarge);
        }
        let mut output = Vec::with_capacity(out_len);
        for frame in 0..frames {
            let sample = frame * 2;
            output.extend_from_slice(&left[sample..sample + 2]);
            output.extend_from_slice(&right[sample..sample + 2]);
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
        let result = self.session.stop();
        self.lifecycle = Lifecycle::Stopped;
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn cancel(&mut self) -> Result<(), CaptureError> {
        let result = self.session.cancel();
        self.lifecycle = Lifecycle::Stopped;
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn disconnect(&mut self) -> Result<(), CaptureError> {
        let result = self.session.disconnect();
        self.lifecycle = Lifecycle::Disconnected;
        if let Err(error) = &result {
            self.retained_error
                .get_or_insert_with(|| OfflineAsioError::Capture(error.clone()));
        }
        result
    }

    pub fn dispose(&mut self) -> Result<(), OfflineAsioError> {
        if self.lifecycle == Lifecycle::Disposed {
            return Ok(());
        }
        if self.session.take().phase() == Phase::Recording
            && let Err(error) = self.session.stop()
        {
            self.retained_error
                .get_or_insert(OfflineAsioError::Capture(error));
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
        ValidatedPlan::software_synthetic(
            48_000,
            2,
            ChannelMap { left: 0, right: 1 },
            RecordRoute::SendProcessedGuitar,
            4096,
        )
        .unwrap()
    }

    fn prepare(response: Option<i32>) -> OfflineAsioAdapter {
        OfflineAsioAdapter::prepare(plan(), descriptors(), halves(), response, true, true, 7)
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
    fn wrong_owner_cannot_publish_callback_data() {
        let mut adapter = prepare(None);
        adapter.start().unwrap();
        let result = std::thread::spawn(move || {
            adapter.deliver(7, SimulatedNotification::Ordinary { half: 0 })
        })
        .join()
        .unwrap();
        assert_eq!(
            result,
            Err(OfflineAsioError::Capture(CaptureError::WrongThread))
        );
    }
}

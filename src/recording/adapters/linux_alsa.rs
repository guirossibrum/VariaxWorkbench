//! Inactive ALSA-to-common-session contract translation for the one admitted
//! Linux PODxt Live declaration.
//!
//! This consumes values returned by ALSA negotiation and interleaved capture
//! periods supplied by a caller. It intentionally contains no ALSA FFI, PCM
//! handle, endpoint discovery, stream initialization, or production start
//! route. Native numeric values are pinned to the installed ALSA 1.2.16.1
//! header contract documented in `docs/audio-capture-session.md`.

use crate::recording::capture::{
    CaptureBackend, CaptureError, CaptureSession, ChannelMap, InterfaceIdentity,
    PendingCaptureEvent, ValidatedPlan,
};
use crate::recording::clock::ClockMeasurement;
use crate::rig::audio;

pub const ALSA_ACCESS_RW_INTERLEAVED: i32 = 3;
pub const ALSA_FORMAT_S24_3LE: i32 = 32;
pub const ALSA_ERRNO_EPIPE: i32 = 32;
pub const ALSA_ERRNO_ESTRPIPE: i32 = 86;
const PODXT_FRAME_BYTES: usize = 6;
const MAX_PERIOD_BYTES: usize = crate::recording::STREAM_CHUNK_BYTES_MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlsaNegotiatedPcm {
    pub access: i32,
    pub format: i32,
    pub channels: u32,
    pub rate_num: u32,
    pub rate_den: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlsaAdapterError {
    Capture(CaptureError),
    UnsupportedAccess(i32),
    UnsupportedFormat(i32),
    InvalidChannels(u32),
    InvalidClock { numerator: u32, denominator: u32 },
    UnadmittedSource,
}

impl From<CaptureError> for AlsaAdapterError {
    fn from(error: CaptureError) -> Self {
        Self::Capture(error)
    }
}

/// Converts only the exact Linux PODxt Live declaration. This is a
/// non-activating contract consumer; constructing it does not query ALSA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlsaPodxtCaptureAdapter {
    session: CaptureSession,
}

impl AlsaPodxtCaptureAdapter {
    pub fn from_negotiated(
        source: ValidatedPlan,
        negotiated: AlsaNegotiatedPcm,
        generation: u64,
    ) -> Result<Self, AlsaAdapterError> {
        source.revalidate()?;
        let admitted = source.contract();
        if admitted.identity != InterfaceIdentity::PodxtLive
            || admitted.backend != CaptureBackend::LinuxDeclaredPod
            || admitted.capture_row.is_none()
            || !matches!(
                admitted.route_kind,
                Some(audio::RouteKind::NativeUsb | audio::RouteKind::SpecificBridge)
            )
            || admitted.declaration != Some(audio::PODXT_LIVE_DECLARATION)
            || admitted.channels != 2
            || admitted.clock != (78_125, 2)
            || admitted.format != audio::SampleFormat::S24ThreeByteLe
            || admitted.channel_map != (ChannelMap { left: 0, right: 1 })
            || admitted.provenance != audio::PODXT_LIVE_DECLARATION.source
        {
            return Err(AlsaAdapterError::UnadmittedSource);
        }
        if negotiated.access != ALSA_ACCESS_RW_INTERLEAVED {
            return Err(AlsaAdapterError::UnsupportedAccess(negotiated.access));
        }
        if negotiated.format != ALSA_FORMAT_S24_3LE {
            return Err(AlsaAdapterError::UnsupportedFormat(negotiated.format));
        }
        if negotiated.channels != 2 {
            return Err(AlsaAdapterError::InvalidChannels(negotiated.channels));
        }
        if (negotiated.rate_num, negotiated.rate_den) != (78_125, 2) {
            return Err(AlsaAdapterError::InvalidClock {
                numerator: negotiated.rate_num,
                denominator: negotiated.rate_den,
            });
        }

        let session = CaptureSession::start(source, generation)?;
        Ok(Self { session })
    }

    /// Translates the signed result of `snd_pcm_readi`-style frame reads.
    /// `buffer` is borrowed only for this call; accepted PCM is copied into
    /// the bounded common queue before the borrow ends. The adapter never
    /// retains a native buffer or owns a PCM handle.
    pub fn translate_read_result(
        &mut self,
        generation: u64,
        native_result_frames: isize,
        requested_frames: usize,
        buffer: &[u8],
    ) -> Result<(), CaptureError> {
        if native_result_frames < 0 {
            let Ok(error_code) = i32::try_from(native_result_frames) else {
                return self.session.adapter_failure(generation, i32::MIN);
            };
            // ALSA reports overrun/suspend and other failures as negative
            // errno values. The lost-frame count is not returned by readi;
            // fail closed instead of inventing a dropout duration.
            let positive_errno = error_code.checked_neg().unwrap_or(i32::MAX);
            if positive_errno == ALSA_ERRNO_EPIPE {
                return self.session.adapter_discontinuity(generation, error_code);
            }
            if positive_errno == ALSA_ERRNO_ESTRPIPE {
                return self.session.adapter_suspended(generation, error_code);
            }
            return self.session.adapter_failure(generation, error_code);
        }
        let Some(expected_buffer_bytes) = requested_frames.checked_mul(PODXT_FRAME_BYTES) else {
            return self.session.adapter_failure(generation, i32::MIN);
        };
        if expected_buffer_bytes > MAX_PERIOD_BYTES || buffer.len() != expected_buffer_bytes {
            return self.session.adapter_failure(generation, i32::MIN);
        }
        let captured_frames = native_result_frames as usize;
        if captured_frames > requested_frames {
            return self.session.adapter_failure(generation, i32::MIN);
        }
        if captured_frames == 0 {
            return Ok(());
        }
        let Some(captured_bytes) = captured_frames.checked_mul(PODXT_FRAME_BYTES) else {
            return self.session.adapter_failure(generation, i32::MIN);
        };
        self.session
            .callback_chunk(generation, &buffer[..captured_bytes])
    }

    pub fn drain(&mut self) -> Result<usize, CaptureError> {
        self.session.drain()
    }

    pub fn stop(&mut self) -> Result<(), CaptureError> {
        self.session.stop()
    }

    pub fn disconnect(&mut self) -> Result<(), CaptureError> {
        self.session.disconnect()
    }

    pub fn observe_software_clock(
        &mut self,
        generation: u64,
        cumulative_frames: u64,
        monotonic_ns: u64,
        dropout_frames: u64,
    ) -> Result<ClockMeasurement, CaptureError> {
        self.session.observe_software_clock(
            generation,
            cumulative_frames,
            monotonic_ns,
            dropout_frames,
        )
    }

    pub fn cancel(&mut self) -> Result<(), CaptureError> {
        self.session.cancel()
    }

    pub fn session(&self) -> &CaptureSession {
        &self.session
    }

    pub fn take_pending_events(&mut self) -> Result<Vec<PendingCaptureEvent>, CaptureError> {
        self.session.take_pending_events()
    }

    pub fn channel_map(&self) -> ChannelMap {
        ChannelMap { left: 0, right: 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod_audio::RecordRoute;
    use crate::rig::ProductProfile;

    fn contract() -> AlsaNegotiatedPcm {
        AlsaNegotiatedPcm {
            access: ALSA_ACCESS_RW_INTERLEAVED,
            format: ALSA_FORMAT_S24_3LE,
            channels: 2,
            rate_num: 78_125,
            rate_den: 2,
        }
    }

    fn source(instrument: Option<ProductProfile>) -> ValidatedPlan {
        source_with_capacity(instrument, 1024)
    }

    fn source_with_capacity(
        instrument: Option<ProductProfile>,
        max_take_bytes: u64,
    ) -> ValidatedPlan {
        let (registry, request) = native_request(instrument);
        ValidatedPlan::native_podxt(
            &registry,
            request,
            RecordRoute::SendCleanGuitar,
            max_take_bytes,
        )
        .unwrap()
    }

    fn native_request(
        instrument: Option<ProductProfile>,
    ) -> (audio::Registry, audio::Request<'static>) {
        let registry = audio::Registry::frozen().unwrap();
        let row = registry
            .rows()
            .iter()
            .find(|row| row.name == "PODxt Live" && row.role == audio::SourceRole::Bridge)
            .unwrap()
            .ordinal;
        (
            registry,
            audio::Request {
                capture_row: row,
                capture_profile: ProductProfile::PodxtLive,
                connected_instrument: instrument,
                platform: audio::Platform::Linux,
                state: audio::RuntimeState::Normal,
                endpoint_use: audio::EndpointUse::Capture,
                contract: audio::CONTRACT,
                installed_revision: None,
                channels: 2,
                clock: (78_125, 2),
                format: audio::SampleFormat::S24ThreeByteLe,
            },
        )
    }

    #[test]
    fn alsa_contract_translation_preserves_native_clock_and_pcm() {
        assert_eq!(ALSA_ACCESS_RW_INTERLEAVED, 3);
        assert_eq!(ALSA_FORMAT_S24_3LE, 32);
        let mut adapter = AlsaPodxtCaptureAdapter::from_negotiated(
            source(Some(ProductProfile::Variax600)),
            contract(),
            51,
        )
        .unwrap();
        assert_eq!(adapter.session().plan().clock, (78_125, 2));
        assert_eq!(adapter.channel_map(), ChannelMap { left: 0, right: 1 });
        let period = vec![0x2a; 66];
        adapter.translate_read_result(51, 11, 11, &period).unwrap();
        adapter.drain().unwrap();
        assert_eq!(adapter.session().take().pcm(), period.as_slice());
        assert_eq!(adapter.stop(), Ok(()));
    }

    #[test]
    fn alsa_contract_translation_rejects_unadmitted_negotiation_shapes() {
        let mut negotiated = contract();
        negotiated.access = 4;
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), negotiated, 52,),
            Err(AlsaAdapterError::UnsupportedAccess(4))
        );

        let mut negotiated = contract();
        negotiated.format = 6;
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), negotiated, 52,),
            Err(AlsaAdapterError::UnsupportedFormat(6))
        );

        let mut negotiated = contract();
        negotiated.rate_num = 39_062;
        negotiated.rate_den = 1;
        assert!(matches!(
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), negotiated, 52,),
            Err(AlsaAdapterError::InvalidClock {
                numerator: 39_062,
                denominator: 1
            })
        ));
    }

    #[test]
    fn identical_pcm_shape_does_not_promote_an_unadmitted_source() {
        let software =
            ValidatedPlan::software_file(44_100, 2, ChannelMap { left: 0, right: 1 }, 1024)
                .unwrap();
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(software, contract(), 60),
            Err(AlsaAdapterError::UnadmittedSource)
        );

        let mut wrong_tuple = contract();
        wrong_tuple.channels = 1;
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), wrong_tuple, 61),
            Err(AlsaAdapterError::InvalidChannels(1))
        );

        for mutate in [
            0, // wrong source row
            1, // maintenance state
            2, // incompatible product profile
            3, // wrong platform
        ] {
            let (registry, mut request) = native_request(None);
            match mutate {
                0 => request.capture_row = usize::MAX,
                1 => request.state = audio::RuntimeState::Maintenance,
                2 => request.capture_profile = ProductProfile::PodHd500x,
                _ => request.platform = audio::Platform::Windows,
            }
            assert!(
                ValidatedPlan::native_podxt(&registry, request, RecordRoute::SendCleanGuitar, 1024)
                    .is_err()
            );
        }
    }

    #[test]
    fn period_translation_respects_frame_count_and_borrowed_buffer_bounds() {
        let mut adapter = AlsaPodxtCaptureAdapter::from_negotiated(
            source_with_capacity(None, 16_384),
            contract(),
            62,
        )
        .unwrap();
        // The admitted PODxt declaration rejects periods below 8,190 bytes.
        let bytes: Vec<u8> = (0..8_190).map(|byte| byte as u8).collect();
        adapter
            .translate_read_result(62, 1_365, 1_365, &bytes)
            .unwrap();
        assert_eq!(adapter.drain().unwrap(), 1);
        assert_eq!(adapter.session().take().pcm(), bytes.as_slice());
        assert_eq!(adapter.translate_read_result(62, 0, 0, &[]), Ok(()));

        let mut overread = AlsaPodxtCaptureAdapter::from_negotiated(
            source_with_capacity(None, 16_384),
            contract(),
            63,
        )
        .unwrap();
        assert_eq!(
            overread.translate_read_result(63, 1_366, 1_365, &bytes),
            Err(CaptureError::AdapterFailure(i32::MIN))
        );

        let mut short_buffer = AlsaPodxtCaptureAdapter::from_negotiated(
            source_with_capacity(None, 16_384),
            contract(),
            64,
        )
        .unwrap();
        assert_eq!(
            short_buffer.translate_read_result(64, 1, 1_365, &bytes[..6]),
            Err(CaptureError::AdapterFailure(i32::MIN))
        );
        assert!(short_buffer.session().take().pcm().is_empty());
    }

    #[test]
    fn xrun_retains_native_discontinuity_and_freezes_later_periods() {
        let mut adapter =
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), contract(), 63).unwrap();
        assert_eq!(
            adapter.translate_read_result(63, -(ALSA_ERRNO_EPIPE as isize), 4, &[]),
            Err(CaptureError::AdapterDiscontinuity(-32))
        );
        assert_eq!(
            adapter.session().retained_error(),
            Some(CaptureError::AdapterDiscontinuity(-32))
        );
        assert_eq!(
            adapter.translate_read_result(63, 4, 4, &[0; 24]),
            Err(CaptureError::AdapterDiscontinuity(-32))
        );
    }

    #[test]
    fn suspended_and_other_native_errors_are_retained_without_pcm() {
        let mut suspended =
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), contract(), 65).unwrap();
        assert_eq!(
            suspended.translate_read_result(65, -(ALSA_ERRNO_ESTRPIPE as isize), 4, &[]),
            Err(CaptureError::AdapterSuspended(-ALSA_ERRNO_ESTRPIPE))
        );
        let mut failed =
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), contract(), 66).unwrap();
        assert_eq!(
            failed.translate_read_result(66, -5, 4, &[]),
            Err(CaptureError::AdapterFailure(-5))
        );
    }

    #[test]
    fn timing_observations_follow_adapter_generation_and_owner_contract() {
        let mut adapter =
            AlsaPodxtCaptureAdapter::from_negotiated(source(None), contract(), 64).unwrap();
        let initial = adapter.observe_software_clock(64, 0, 1_000, 0).unwrap();
        assert_eq!(
            adapter.observe_software_clock(63, 1_000, 25_601_000, 0),
            Err(CaptureError::StaleGeneration)
        );
        assert_eq!(adapter.session().clock_measurement(), Some(initial));
        let (mut adapter, wrong_thread) = std::thread::spawn(move || {
            let result = adapter.observe_software_clock(64, 1_000, 25_601_000, 0);
            (adapter, result)
        })
        .join()
        .unwrap();
        assert_eq!(wrong_thread, Err(CaptureError::WrongThread));
        assert_eq!(adapter.session().clock_measurement(), Some(initial));
        let valid = adapter
            .observe_software_clock(64, 1_000, 25_601_000, 0)
            .unwrap();
        assert_eq!(valid.drift_ppm, Some(0));
        adapter.stop().unwrap();
        assert_eq!(
            adapter.observe_software_clock(64, 2_000, 51_201_000, 0),
            Err(CaptureError::Stopped)
        );
        assert_eq!(adapter.session().clock_measurement(), Some(valid));
    }
}

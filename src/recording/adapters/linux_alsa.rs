//! Inactive ALSA-to-common-session contract translation for the one admitted
//! Linux PODxt Live declaration.
//!
//! This consumes values returned by ALSA negotiation and interleaved capture
//! periods supplied by a caller. It intentionally contains no ALSA FFI, PCM
//! handle, endpoint discovery, stream initialization, or production start
//! route. Native numeric values are pinned to the installed ALSA 1.2.16.1
//! header contract documented in `docs/audio-capture-session.md`.

use crate::pod_audio::RecordRoute;
use crate::recording::capture::{
    CaptureError, CaptureSession, ChannelMap, PendingCaptureEvent, ValidatedPlan,
};
use crate::recording::clock::ClockMeasurement;
use crate::rig::{ProductProfile, audio};

pub const ALSA_ACCESS_RW_INTERLEAVED: i32 = 3;
pub const ALSA_FORMAT_S24_3LE: i32 = 32;

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
        negotiated: AlsaNegotiatedPcm,
        connected_instrument: Option<ProductProfile>,
        signal_route: RecordRoute,
        max_take_bytes: u64,
        generation: u64,
    ) -> Result<Self, AlsaAdapterError> {
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

        let registry = audio::Registry::frozen()
            .map_err(|_| AlsaAdapterError::Capture(CaptureError::ForgedOrChangedRoute))?;
        let capture_row = registry
            .rows()
            .iter()
            .find(|row| row.name == "PODxt Live" && row.role == audio::SourceRole::Bridge)
            .ok_or(AlsaAdapterError::Capture(
                CaptureError::ForgedOrChangedRoute,
            ))?
            .ordinal;
        let request = audio::Request {
            capture_row,
            capture_profile: ProductProfile::PodxtLive,
            connected_instrument,
            platform: audio::Platform::Linux,
            state: audio::RuntimeState::Normal,
            endpoint_use: audio::EndpointUse::Capture,
            contract: audio::CONTRACT,
            installed_revision: None,
            channels: 2,
            clock: (78_125, 2),
            format: audio::SampleFormat::S24ThreeByteLe,
        };
        let plan = ValidatedPlan::native_podxt(&registry, request, signal_route, max_take_bytes)?;
        let session = CaptureSession::start(plan, generation)?;
        Ok(Self { session })
    }

    /// Supplies one already-obtained interleaved ALSA period to the common
    /// bounded queue. The caller owns native stream lifetime; this adapter
    /// creates no such stream or handle.
    pub fn capture_period(
        &mut self,
        generation: u64,
        interleaved_s24_3le: &[u8],
    ) -> Result<(), CaptureError> {
        self.session.callback_chunk(generation, interleaved_s24_3le)
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
        cumulative_frames: u64,
        monotonic_ns: u64,
        dropout_frames: u64,
    ) -> Result<ClockMeasurement, CaptureError> {
        self.session
            .observe_software_clock(cumulative_frames, monotonic_ns, dropout_frames)
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

    fn contract() -> AlsaNegotiatedPcm {
        AlsaNegotiatedPcm {
            access: ALSA_ACCESS_RW_INTERLEAVED,
            format: ALSA_FORMAT_S24_3LE,
            channels: 2,
            rate_num: 78_125,
            rate_den: 2,
        }
    }

    #[test]
    fn alsa_contract_translation_preserves_native_clock_and_pcm() {
        assert_eq!(ALSA_ACCESS_RW_INTERLEAVED, 3);
        assert_eq!(ALSA_FORMAT_S24_3LE, 32);
        let mut adapter = AlsaPodxtCaptureAdapter::from_negotiated(
            contract(),
            Some(ProductProfile::Variax600),
            RecordRoute::SendCleanGuitar,
            1024,
            51,
        )
        .unwrap();
        assert_eq!(adapter.session().plan().clock, (78_125, 2));
        assert_eq!(adapter.channel_map(), ChannelMap { left: 0, right: 1 });
        let period = vec![0x2a; 66];
        adapter.capture_period(51, &period).unwrap();
        adapter.drain().unwrap();
        assert_eq!(adapter.session().take().pcm(), period.as_slice());
        assert_eq!(adapter.stop(), Ok(()));
    }

    #[test]
    fn alsa_contract_translation_rejects_unadmitted_negotiation_shapes() {
        let mut negotiated = contract();
        negotiated.access = 4;
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(
                negotiated,
                None,
                RecordRoute::SendProcessedGuitar,
                1024,
                52,
            ),
            Err(AlsaAdapterError::UnsupportedAccess(4))
        );

        let mut negotiated = contract();
        negotiated.format = 6;
        assert_eq!(
            AlsaPodxtCaptureAdapter::from_negotiated(
                negotiated,
                None,
                RecordRoute::SendProcessedGuitar,
                1024,
                52,
            ),
            Err(AlsaAdapterError::UnsupportedFormat(6))
        );

        let mut negotiated = contract();
        negotiated.rate_num = 39_062;
        negotiated.rate_den = 1;
        assert!(matches!(
            AlsaPodxtCaptureAdapter::from_negotiated(
                negotiated,
                None,
                RecordRoute::SendProcessedGuitar,
                1024,
                52,
            ),
            Err(AlsaAdapterError::InvalidClock {
                numerator: 39_062,
                denominator: 1
            })
        ));
    }
}

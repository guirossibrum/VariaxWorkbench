//! Offline PODxt Live record-route planner.
//!
//! Two evidence classes stay separate. The GearBox 2.0 recording guide,
//! revision 03, describes Record Send 1-2 and four routing modes. Linux
//! `sound/usb/line6` at tag `v7.2.15` declares a PCM device for the POD role.
//! Neither class is a measurement from a connected PODxt Live.
//!
//! Planning a route does not open ALSA, PipeWire, PulseAudio, USB, or MIDI,
//! and it does not load or bind a driver.

#![forbid(unsafe_code)]

use crate::linux_transport::{
    MAC_AUDIO_CONFIGURATION, MAC_AUDIO_INTERFACE, MAC_AUDIO_PRODUCT, MAC_AUDIO_VENDOR,
    POD_PODXT_LIVE, PODXT_LIVE_VARIAX_PROPERTIES, VARIAX_PODXT_LIVE, VARIAX_WORKBENCH,
    WINDOWS_DRIVER_VERSION, WINDOWS_PODXT_LIVE_HARDWARE_ID,
};

/// `LINE6_CAP_CONTROL` in Linux `v7.2.15` `driver.h`.
pub const CAP_CONTROL: u32 = 1 << 0;
/// `LINE6_CAP_PCM`.
pub const CAP_PCM: u32 = 1 << 1;
/// `LINE6_CAP_HWMON`.
pub const CAP_HWMON: u32 = 1 << 2;
/// `LINE6_CAP_IN_NEEDS_OUT`. The PODxt Live POD row does not set this bit.
pub const CAP_IN_NEEDS_OUT: u32 = 1 << 3;
/// `LINE6_CAP_CONTROL_MIDI`.
pub const CAP_CONTROL_MIDI: u32 = 1 << 4;

/// `LINE6_PODXTLIVE_POD` capabilities in `pod.c`: control, MIDI, PCM, and hwmon.
pub const POD_CAPABILITIES: u32 = CAP_CONTROL | CAP_CONTROL_MIDI | CAP_PCM | CAP_HWMON;
pub const POD_ALTSETTING: u8 = 1;
pub const POD_EP_CTRL_R: u8 = 0x84;
pub const POD_EP_CTRL_W: u8 = 0x03;
pub const POD_EP_AUDIO_R: u8 = 0x82;
pub const POD_EP_AUDIO_W: u8 = 0x01;

pub const LINUX_SOURCE_TAG: &str = "v7.2.15";
pub const POD_PCM_SOURCE: &str = "sound/usb/line6/pod.c";

/// `pod_ratden`: numerator 78125, denominator 2.
pub const RATE_NUMERATOR: u32 = 78_125;
pub const RATE_DENOMINATOR: u32 = 2;
/// Integer `rate_min` / `rate_max` fields beside that single knot.
pub const RATE_MIN_FIELD: u32 = 39_062;
pub const RATE_MAX_FIELD: u32 = 39_063;
pub const DECLARED_CHANNELS: u8 = 2;
pub const BYTES_PER_CHANNEL: u32 = 3;
pub const FRAME_BYTES: u32 = DECLARED_CHANNELS as u32 * BYTES_PER_CHANNEL;
pub const PERIOD_BYTES_MIN: u32 = 64;
pub const PERIOD_BYTES_MAX: u32 = 8_192;
pub const BUFFER_BYTES_MAX: u32 = 60_000;
pub const PERIODS_MIN: u32 = 1;
pub const PERIODS_MAX: u32 = 1_024;
pub const S24_MIN: i32 = -1 << 23;
pub const S24_MAX: i32 = (1 << 23) - 1;

/// `LINE6_ISO_PACKETS` and `LINE6_ISO_INTERVAL` in `pcm.h`.
pub const ISO_PACKETS_PER_URB: u8 = 1;
pub const ISO_INTERVAL: u8 = 1;
pub const USB_LOW_ISO_BUFFERS: u8 = 2;
pub const USB_HIGH_ISO_BUFFERS: u8 = 16;
pub const USB_LOW_INTERVALS_PER_SECOND: u32 = 1_000;
pub const USB_HIGH_INTERVALS_PER_SECOND: u32 = 8_000;
pub const FALLBACK_INTERVAL: u8 = 10;
pub const FALLBACK_MAX_PACKET: u16 = 16;

/// Playback `snd_pcm_hardware.info` includes `SNDRV_PCM_INFO_PAUSE`. Capture does not.
pub const PLAYBACK_PAUSE_DECLARED: bool = true;
pub const CAPTURE_PAUSE_DECLARED: bool = false;

const NO_OUTBOUND: &[u8] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordRoute {
    /// Guide mode 1. Processed guitar on Record Send 1-2.
    SendProcessedGuitar,
    /// Guide mode 2. Unprocessed guitar on Record Send 1-2.
    SendCleanGuitar,
    /// Guide mode 3. Clean record send while host audio feeds POD processing.
    SendCleanGuitarReampPlayback,
    /// Guide mode 4. Host audio enters the POD and processed audio returns on Record Send 1-2.
    SendReampPlayback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeignRoute {
    TonePortSend34,
    Spdif,
    AesEbu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordContent {
    ProcessedGuitar,
    UnprocessedGuitar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessingSource {
    Instrument,
    HostPlayback,
    CleanGuitarRecordedWhileHostFeedsProcessing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorMix {
    ProcessedGuitarAndHostPlayback,
    ProcessedHostPlayback,
    /// Mode 4 states the record-send return and does not restate the monitor mix.
    NotStatedForThisMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteBehavior {
    pub mode_number: u8,
    pub usb_record_send: RecordContent,
    pub processing_source: ProcessingSource,
    pub monitor: MonitorMix,
    pub reamp_pass: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    S24_3LE,
    S16LE,
    S24LE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferPlan {
    pub period_bytes: u32,
    pub periods: u32,
    pub buffer_bytes: u32,
    pub frames_per_period: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntheticBuffer {
    period_bytes: usize,
    capacity: usize,
    data: Vec<u8>,
    cancelled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentedConnections {
    pub record_pair: &'static str,
    pub instrument_reaches_record_send: bool,
    pub aux_reaches_record_send: bool,
    pub analog_includes_host_playback: bool,
    pub analog_includes_instrument: bool,
    pub analog_includes_aux: bool,
    pub phones_follow_analog: bool,
    /// The guide recommends a separate USB controller and a direct port.
    /// That sentence is advice in the guide.
    pub separate_usb_controller_is_guide_advice: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinuxAudioStack {
    pub source_tag: &'static str,
    pub pcm_source: &'static str,
    pub pcm_declared_for_pod_role: bool,
    pub gearbox_mode_names_in_driver_source: bool,
    pub custom_driver_proposed: bool,
    pub alsa_device_opened: bool,
    pub pipewire_opened: bool,
    pub pulseaudio_opened: bool,
    pub connected_device_measured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticOsReferences {
    pub mac_vendor: u32,
    pub mac_product: u32,
    pub mac_configuration: u32,
    pub mac_interface: u32,
    pub windows_hardware_id: &'static str,
    pub windows_driver_version: &'static str,
    pub packages_loaded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleWiring {
    pub interface_number: u8,
    pub driver_info: u64,
    pub capabilities: u32,
    pub altsetting: u8,
    pub pcm_capability: bool,
    pub hwmon_capability: bool,
    pub in_needs_out: bool,
    /// `line6_init_pcm` returns before creating a PCM device when `CAP_PCM` is clear.
    pub source_initializes_pcm: bool,
    pub ctrl_ep_read: u8,
    pub ctrl_ep_write: u8,
    pub listed_audio_ep_read: u8,
    pub listed_audio_ep_write: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coexistence {
    pub vendor_id: u16,
    pub product_id: u16,
    pub guitar: RoleWiring,
    pub pod: RoleWiring,
    pub workbench_product_id: u16,
    pub workbench_is_separate_match: bool,
    pub outbound: &'static [u8],
    pub stream_opened: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutePlan {
    pub route: RecordRoute,
    pub behavior: RouteBehavior,
    pub buffer: BufferPlan,
    pub coexistence: Coexistence,
    pub outbound: &'static [u8],
    pub stream_opened: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    ChannelsNotDeclared,
    FormatNotDeclared,
    RateNotDeclared,
    PeriodNotFrameAligned,
    PeriodBelowDeclaredMinimum,
    PeriodAboveDeclaredMaximum,
    PeriodCountOutOfRange,
    BufferAboveDeclaredMaximum,
    PeriodLengthMismatch,
    BufferFull,
    BufferCancelled,
    SampleOutOfRange,
    DeviceSpeedNotObserved,
    PacketSizeNotDeclared,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::ChannelsNotDeclared => "channel count is outside the declared stereo pair",
            Self::FormatNotDeclared => "sample format is outside the declared S24_3LE PCM",
            Self::RateNotDeclared => "rate is outside the declared 78125/2 knot",
            Self::PeriodNotFrameAligned => {
                "period byte count is not a multiple of the 6-byte frame"
            }
            Self::PeriodBelowDeclaredMinimum => "period is below the declared 64-byte minimum",
            Self::PeriodAboveDeclaredMaximum => "period is above the declared 8192-byte maximum",
            Self::PeriodCountOutOfRange => "period count is outside the declared 1..=1024 range",
            Self::BufferAboveDeclaredMaximum => "buffer exceeds the declared 60000-byte maximum",
            Self::PeriodLengthMismatch => "synthetic period length does not match the plan",
            Self::BufferFull => "synthetic buffer is full",
            Self::BufferCancelled => "synthetic buffer is cancelled",
            Self::SampleOutOfRange => "sample does not fit in signed 24-bit",
            Self::DeviceSpeedNotObserved => {
                "USB speed was not observed, so the ISO buffer count stays unselected"
            }
            Self::PacketSizeNotDeclared => {
                "max packet size is read from the endpoint at probe time and is not declared for this planner"
            }
        };
        formatter.write_str(message)
    }
}

pub const fn behavior(route: RecordRoute) -> RouteBehavior {
    match route {
        RecordRoute::SendProcessedGuitar => RouteBehavior {
            mode_number: 1,
            usb_record_send: RecordContent::ProcessedGuitar,
            processing_source: ProcessingSource::Instrument,
            monitor: MonitorMix::ProcessedGuitarAndHostPlayback,
            reamp_pass: false,
        },
        RecordRoute::SendCleanGuitar => RouteBehavior {
            mode_number: 2,
            usb_record_send: RecordContent::UnprocessedGuitar,
            processing_source: ProcessingSource::Instrument,
            monitor: MonitorMix::ProcessedGuitarAndHostPlayback,
            reamp_pass: false,
        },
        RecordRoute::SendCleanGuitarReampPlayback => RouteBehavior {
            mode_number: 3,
            usb_record_send: RecordContent::UnprocessedGuitar,
            processing_source: ProcessingSource::CleanGuitarRecordedWhileHostFeedsProcessing,
            monitor: MonitorMix::ProcessedHostPlayback,
            reamp_pass: true,
        },
        RecordRoute::SendReampPlayback => RouteBehavior {
            mode_number: 4,
            usb_record_send: RecordContent::ProcessedGuitar,
            processing_source: ProcessingSource::HostPlayback,
            monitor: MonitorMix::NotStatedForThisMode,
            reamp_pass: true,
        },
    }
}

pub const fn foreign_route(route: ForeignRoute) -> &'static str {
    match route {
        ForeignRoute::TonePortSend34 => {
            "TonePort Send 3-4 is a second stereo pair. PODxt Live documents Record Send 1-2."
        }
        ForeignRoute::Spdif => "S/PDIF in this guide belongs to TonePort UX2 and PODxt Pro.",
        ForeignRoute::AesEbu => "AES/EBU in this guide belongs to PODxt Pro.",
    }
}

pub const fn documented_connections() -> DocumentedConnections {
    DocumentedConnections {
        record_pair: "Record Send 1-2",
        instrument_reaches_record_send: true,
        aux_reaches_record_send: false,
        analog_includes_host_playback: true,
        analog_includes_instrument: true,
        analog_includes_aux: true,
        phones_follow_analog: true,
        separate_usb_controller_is_guide_advice: true,
    }
}

pub const fn linux_stack() -> LinuxAudioStack {
    LinuxAudioStack {
        source_tag: LINUX_SOURCE_TAG,
        pcm_source: POD_PCM_SOURCE,
        pcm_declared_for_pod_role: true,
        gearbox_mode_names_in_driver_source: false,
        custom_driver_proposed: false,
        alsa_device_opened: false,
        pipewire_opened: false,
        pulseaudio_opened: false,
        connected_device_measured: false,
    }
}

pub const fn static_os_references() -> StaticOsReferences {
    StaticOsReferences {
        mac_vendor: MAC_AUDIO_VENDOR,
        mac_product: MAC_AUDIO_PRODUCT,
        mac_configuration: MAC_AUDIO_CONFIGURATION,
        mac_interface: MAC_AUDIO_INTERFACE,
        windows_hardware_id: WINDOWS_PODXT_LIVE_HARDWARE_ID,
        windows_driver_version: WINDOWS_DRIVER_VERSION,
        packages_loaded: false,
    }
}

pub const fn coexistence() -> Coexistence {
    let guitar_caps = PODXT_LIVE_VARIAX_PROPERTIES.capabilities;
    let guitar_pcm = has_cap(guitar_caps, CAP_PCM);
    Coexistence {
        vendor_id: VARIAX_PODXT_LIVE.vendor_id,
        product_id: VARIAX_PODXT_LIVE.product_id,
        guitar: RoleWiring {
            interface_number: VARIAX_PODXT_LIVE.interface_number,
            driver_info: VARIAX_PODXT_LIVE.driver_info,
            capabilities: guitar_caps,
            altsetting: endpoint(PODXT_LIVE_VARIAX_PROPERTIES.altsetting),
            pcm_capability: guitar_pcm,
            hwmon_capability: has_cap(guitar_caps, CAP_HWMON),
            in_needs_out: has_cap(guitar_caps, CAP_IN_NEEDS_OUT),
            source_initializes_pcm: guitar_pcm,
            ctrl_ep_read: endpoint(PODXT_LIVE_VARIAX_PROPERTIES.ep_ctrl_r),
            ctrl_ep_write: endpoint(PODXT_LIVE_VARIAX_PROPERTIES.ep_ctrl_w),
            listed_audio_ep_read: endpoint(PODXT_LIVE_VARIAX_PROPERTIES.ep_audio_r),
            listed_audio_ep_write: endpoint(PODXT_LIVE_VARIAX_PROPERTIES.ep_audio_w),
        },
        pod: RoleWiring {
            interface_number: POD_PODXT_LIVE.interface_number,
            driver_info: POD_PODXT_LIVE.driver_info,
            capabilities: POD_CAPABILITIES,
            altsetting: POD_ALTSETTING,
            pcm_capability: has_cap(POD_CAPABILITIES, CAP_PCM),
            hwmon_capability: has_cap(POD_CAPABILITIES, CAP_HWMON),
            in_needs_out: has_cap(POD_CAPABILITIES, CAP_IN_NEEDS_OUT),
            source_initializes_pcm: has_cap(POD_CAPABILITIES, CAP_PCM),
            ctrl_ep_read: POD_EP_CTRL_R,
            ctrl_ep_write: POD_EP_CTRL_W,
            listed_audio_ep_read: POD_EP_AUDIO_R,
            listed_audio_ep_write: POD_EP_AUDIO_W,
        },
        workbench_product_id: VARIAX_WORKBENCH.product_id,
        workbench_is_separate_match: true,
        outbound: NO_OUTBOUND,
        stream_opened: false,
    }
}

pub fn accept_channels(channels: u8) -> Result<(), PlanError> {
    if channels == DECLARED_CHANNELS {
        Ok(())
    } else {
        Err(PlanError::ChannelsNotDeclared)
    }
}

pub fn accept_format(format: SampleFormat) -> Result<(), PlanError> {
    match format {
        SampleFormat::S24_3LE => Ok(()),
        SampleFormat::S16LE | SampleFormat::S24LE => Err(PlanError::FormatNotDeclared),
    }
}

/// Accepts the single `pod_ratden` fraction. The integer `rate_min` and
/// `rate_max` fields are not extra rates.
pub fn accept_rate(numerator: u32, denominator: u32) -> Result<(), PlanError> {
    if numerator == RATE_NUMERATOR && denominator == RATE_DENOMINATOR {
        Ok(())
    } else {
        Err(PlanError::RateNotDeclared)
    }
}

pub fn plan_buffer(period_bytes: u32, periods: u32) -> Result<BufferPlan, PlanError> {
    if !period_bytes.is_multiple_of(FRAME_BYTES) {
        return Err(PlanError::PeriodNotFrameAligned);
    }
    if period_bytes < PERIOD_BYTES_MIN {
        return Err(PlanError::PeriodBelowDeclaredMinimum);
    }
    if period_bytes > PERIOD_BYTES_MAX {
        return Err(PlanError::PeriodAboveDeclaredMaximum);
    }
    if !(PERIODS_MIN..=PERIODS_MAX).contains(&periods) {
        return Err(PlanError::PeriodCountOutOfRange);
    }
    let buffer_bytes = period_bytes
        .checked_mul(periods)
        .filter(|bytes| *bytes <= BUFFER_BYTES_MAX)
        .ok_or(PlanError::BufferAboveDeclaredMaximum)?;
    Ok(BufferPlan {
        period_bytes,
        periods,
        buffer_bytes,
        frames_per_period: period_bytes / FRAME_BYTES,
    })
}

/// `_open_device` is part of the signature so a caller can prove it is not invoked.
pub fn plan<F>(
    route: RecordRoute,
    period_bytes: u32,
    periods: u32,
    _open_device: F,
) -> Result<RoutePlan, PlanError>
where
    F: FnOnce(),
{
    Ok(RoutePlan {
        route,
        behavior: behavior(route),
        buffer: plan_buffer(period_bytes, periods)?,
        coexistence: coexistence(),
        outbound: NO_OUTBOUND,
        stream_opened: false,
    })
}

pub fn pack_stereo(left: i32, right: i32) -> Result<[u8; FRAME_BYTES as usize], PlanError> {
    let left = pack_s24(left)?;
    let right = pack_s24(right)?;
    Ok([left[0], left[1], left[2], right[0], right[1], right[2]])
}

pub fn unpack_stereo(frame: [u8; FRAME_BYTES as usize]) -> (i32, i32) {
    (
        unpack_s24([frame[0], frame[1], frame[2]]),
        unpack_s24([frame[3], frame[4], frame[5]]),
    )
}

pub fn iso_buffers_if_known() -> Result<u8, PlanError> {
    Err(PlanError::DeviceSpeedNotObserved)
}

pub fn max_packet_size() -> Result<u16, PlanError> {
    Err(PlanError::PacketSizeNotDeclared)
}

impl SyntheticBuffer {
    pub fn new(plan: &BufferPlan) -> Self {
        Self {
            period_bytes: plan.period_bytes as usize,
            capacity: plan.buffer_bytes as usize,
            data: Vec::new(),
            cancelled: false,
        }
    }

    pub fn write_period(&mut self, period: &[u8]) -> Result<(), PlanError> {
        if self.cancelled {
            return Err(PlanError::BufferCancelled);
        }
        if period.len() != self.period_bytes {
            return Err(PlanError::PeriodLengthMismatch);
        }
        let next = self
            .data
            .len()
            .checked_add(period.len())
            .ok_or(PlanError::BufferFull)?;
        if next > self.capacity {
            return Err(PlanError::BufferFull);
        }
        self.data.extend_from_slice(period);
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub const fn device_was_opened(&self) -> bool {
        false
    }
}

fn pack_s24(sample: i32) -> Result<[u8; 3], PlanError> {
    if !(S24_MIN..=S24_MAX).contains(&sample) {
        return Err(PlanError::SampleOutOfRange);
    }
    let bytes = sample.to_le_bytes();
    Ok([bytes[0], bytes[1], bytes[2]])
}

fn unpack_s24(bytes: [u8; 3]) -> i32 {
    let unsigned = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
    (unsigned << 8) >> 8
}

const fn has_cap(capabilities: u32, bit: u32) -> bool {
    (capabilities & bit) != 0
}

const fn endpoint(value: u32) -> u8 {
    assert!(value <= 0xff);
    value as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gearbox_modes_keep_record_send_monitor_and_reamp_distinct() {
        let processed = behavior(RecordRoute::SendProcessedGuitar);
        assert_eq!(processed.mode_number, 1);
        assert_eq!(processed.usb_record_send, RecordContent::ProcessedGuitar);
        assert_eq!(processed.processing_source, ProcessingSource::Instrument);
        assert_eq!(
            processed.monitor,
            MonitorMix::ProcessedGuitarAndHostPlayback
        );
        assert!(!processed.reamp_pass);

        let clean = behavior(RecordRoute::SendCleanGuitar);
        assert_eq!(clean.mode_number, 2);
        assert_eq!(clean.usb_record_send, RecordContent::UnprocessedGuitar);
        assert!(!clean.reamp_pass);

        let clean_reamp = behavior(RecordRoute::SendCleanGuitarReampPlayback);
        assert_eq!(clean_reamp.mode_number, 3);
        assert_eq!(
            clean_reamp.usb_record_send,
            RecordContent::UnprocessedGuitar
        );
        assert_eq!(
            clean_reamp.processing_source,
            ProcessingSource::CleanGuitarRecordedWhileHostFeedsProcessing
        );
        assert_eq!(clean_reamp.monitor, MonitorMix::ProcessedHostPlayback);
        assert!(clean_reamp.reamp_pass);

        let reamp = behavior(RecordRoute::SendReampPlayback);
        assert_eq!(reamp.mode_number, 4);
        assert_eq!(reamp.usb_record_send, RecordContent::ProcessedGuitar);
        assert_eq!(reamp.processing_source, ProcessingSource::HostPlayback);
        assert_eq!(reamp.monitor, MonitorMix::NotStatedForThisMode);
        assert!(reamp.reamp_pass);

        let connections = documented_connections();
        assert_eq!(connections.record_pair, "Record Send 1-2");
        assert!(connections.instrument_reaches_record_send);
        assert!(!connections.aux_reaches_record_send);
        assert!(connections.analog_includes_aux);
        assert!(connections.phones_follow_analog);
        assert!(connections.separate_usb_controller_is_guide_advice);
        assert_eq!(
            foreign_route(ForeignRoute::TonePortSend34),
            "TonePort Send 3-4 is a second stereo pair. PODxt Live documents Record Send 1-2."
        );
        assert!(foreign_route(ForeignRoute::Spdif).contains("PODxt Pro"));
        assert!(foreign_route(ForeignRoute::AesEbu).contains("PODxt Pro"));
    }

    #[test]
    fn declared_pcm_window_requires_a_six_byte_frame() {
        assert!(accept_rate(RATE_NUMERATOR, RATE_DENOMINATOR).is_ok());
        assert_eq!(accept_rate(44_100, 1), Err(PlanError::RateNotDeclared));
        assert_eq!(accept_rate(48_000, 1), Err(PlanError::RateNotDeclared));
        assert_eq!(
            accept_rate(RATE_MIN_FIELD, 1),
            Err(PlanError::RateNotDeclared)
        );
        assert_eq!(
            accept_rate(RATE_MAX_FIELD, 1),
            Err(PlanError::RateNotDeclared)
        );
        assert!(accept_channels(2).is_ok());
        assert_eq!(accept_channels(1), Err(PlanError::ChannelsNotDeclared));
        assert!(accept_format(SampleFormat::S24_3LE).is_ok());
        assert_eq!(
            accept_format(SampleFormat::S16LE),
            Err(PlanError::FormatNotDeclared)
        );
        assert_eq!(
            accept_format(SampleFormat::S24LE),
            Err(PlanError::FormatNotDeclared)
        );

        assert_eq!(plan_buffer(64, 1), Err(PlanError::PeriodNotFrameAligned));
        assert_eq!(plan_buffer(8_192, 1), Err(PlanError::PeriodNotFrameAligned));
        assert_eq!(
            plan_buffer(6, 1),
            Err(PlanError::PeriodBelowDeclaredMinimum)
        );
        assert_eq!(
            plan_buffer(8_196, 1),
            Err(PlanError::PeriodAboveDeclaredMaximum)
        );
        assert_eq!(plan_buffer(66, 0), Err(PlanError::PeriodCountOutOfRange));
        assert_eq!(
            plan_buffer(66, 1_025),
            Err(PlanError::PeriodCountOutOfRange)
        );

        let tight = plan_buffer(66, 909).expect("66 * 909 fits");
        assert_eq!(tight.buffer_bytes, 59_994);
        assert_eq!(tight.frames_per_period, 11);
        assert_eq!(
            plan_buffer(66, 910),
            Err(PlanError::BufferAboveDeclaredMaximum)
        );
        assert_eq!(
            plan_buffer(66, 1_024),
            Err(PlanError::BufferAboveDeclaredMaximum)
        );
        let wide = plan_buffer(8_190, 7).expect("8190 * 7 fits");
        assert_eq!(wide.frames_per_period, 1_365);
        assert_eq!(wide.buffer_bytes, 57_330);
        assert_eq!(
            plan_buffer(8_190, 8),
            Err(PlanError::BufferAboveDeclaredMaximum)
        );
        const {
            assert!(PLAYBACK_PAUSE_DECLARED);
            assert!(!CAPTURE_PAUSE_DECLARED);
        }
    }

    #[test]
    fn synthetic_frames_fill_a_bounded_buffer_and_cancel_in_memory() {
        assert_eq!(
            pack_stereo(S24_MAX + 1, 0),
            Err(PlanError::SampleOutOfRange)
        );
        assert_eq!(
            pack_stereo(0, S24_MIN - 1),
            Err(PlanError::SampleOutOfRange)
        );
        let peak = pack_stereo(S24_MAX, S24_MIN).expect("in range");
        assert_eq!(peak, [0xff, 0xff, 0x7f, 0x00, 0x00, 0x80]);
        assert_eq!(unpack_stereo(peak), (S24_MAX, S24_MIN));
        assert_eq!(unpack_stereo(pack_stereo(0, -1).unwrap()), (0, -1));

        let planned = plan_buffer(66, 2).unwrap();
        let mut buffer = SyntheticBuffer::new(&planned);
        let period = vec![0x11; 66];
        buffer.write_period(&period).unwrap();
        buffer.write_period(&period).unwrap();
        assert_eq!(buffer.as_bytes().len(), 132);
        assert_eq!(buffer.write_period(&period), Err(PlanError::BufferFull));
        assert_eq!(
            buffer.write_period(&[0; 60]),
            Err(PlanError::PeriodLengthMismatch)
        );
        buffer.cancel();
        assert_eq!(buffer.as_bytes().len(), 132);
        assert_eq!(
            buffer.write_period(&period),
            Err(PlanError::BufferCancelled)
        );
        assert!(!buffer.device_was_opened());
    }

    #[test]
    fn guitar_control_and_pod_audio_are_planned_together_without_a_stream() {
        let mut opened = 0;
        let planned = plan(RecordRoute::SendProcessedGuitar, 66, 2, || opened += 1).unwrap();
        assert_eq!(opened, 0);
        assert!(!planned.stream_opened);
        assert!(planned.outbound.is_empty());
        assert_ne!(planned.outbound, crate::PODXT_ROUTING_ENABLE);
        assert_eq!(planned.behavior, behavior(RecordRoute::SendProcessedGuitar));

        let both = planned.coexistence;
        assert_eq!(both.vendor_id, 0x0e41);
        assert_eq!(both.product_id, 0x4650);
        assert_eq!(both.guitar.interface_number, 1);
        assert_eq!(both.guitar.driver_info, 0);
        assert_eq!(both.guitar.capabilities, CAP_CONTROL | CAP_CONTROL_MIDI);
        assert!(!both.guitar.pcm_capability);
        assert!(!both.guitar.source_initializes_pcm);
        assert_eq!(both.guitar.ctrl_ep_read, 0x86);
        assert_eq!(both.guitar.ctrl_ep_write, 0x05);
        assert_eq!(both.pod.interface_number, 0);
        assert_eq!(both.pod.driver_info, 5);
        assert_eq!(both.pod.capabilities, 0x17);
        assert!(both.pod.pcm_capability);
        assert!(both.pod.hwmon_capability);
        assert!(!both.pod.in_needs_out);
        assert!(both.pod.source_initializes_pcm);
        assert_eq!(both.pod.ctrl_ep_read, 0x84);
        assert_eq!(both.pod.ctrl_ep_write, 0x03);
        assert_eq!(both.pod.listed_audio_ep_read, 0x82);
        assert_eq!(both.pod.listed_audio_ep_write, 0x01);
        assert_eq!(
            both.guitar.listed_audio_ep_read,
            both.pod.listed_audio_ep_read
        );
        assert_ne!(both.guitar.ctrl_ep_read, both.pod.ctrl_ep_read);
        assert_eq!(both.guitar.altsetting, 1);
        assert_eq!(both.pod.altsetting, 1);
        assert_ne!(
            u32::from(both.guitar.interface_number),
            PODXT_LIVE_VARIAX_PROPERTIES.ctrl_if
        );
        assert_eq!(both.workbench_product_id, 0x534d);
        assert!(both.workbench_is_separate_match);
        assert!(!both.stream_opened);
        assert_ne!(both.product_id, both.workbench_product_id);

        let stack = linux_stack();
        assert!(stack.pcm_declared_for_pod_role);
        assert!(!stack.gearbox_mode_names_in_driver_source);
        assert!(!stack.custom_driver_proposed);
        assert!(!stack.alsa_device_opened);
        assert!(!stack.pipewire_opened);
        assert!(!stack.pulseaudio_opened);
        assert!(!stack.connected_device_measured);

        let references = static_os_references();
        assert_eq!(references.mac_vendor, 0x0e41);
        assert_eq!(references.mac_product, 0x4650);
        assert_eq!(
            references.mac_interface,
            u32::from(both.pod.interface_number)
        );
        assert_eq!(references.windows_hardware_id, "USB\\VID_0E41&PID_4650");
        assert!(!references.packages_loaded);

        assert_eq!(
            iso_buffers_if_known(),
            Err(PlanError::DeviceSpeedNotObserved)
        );
        assert_eq!(max_packet_size(), Err(PlanError::PacketSizeNotDeclared));
        const {
            assert!(USB_LOW_ISO_BUFFERS != USB_HIGH_ISO_BUFFERS);
            assert!(FALLBACK_MAX_PACKET == 16);
            assert!(ISO_PACKETS_PER_URB == 1);
            assert!(ISO_INTERVAL == 1);
            assert!(USB_LOW_INTERVALS_PER_SECOND == 1_000);
            assert!(USB_HIGH_INTERVALS_PER_SECOND == 8_000);
            assert!(FALLBACK_INTERVAL == 10);
            assert!(FRAME_BYTES == 6);
            assert!(RATE_NUMERATOR == 78_125);
            assert!(RATE_DENOMINATOR == 2);
        }
    }
}

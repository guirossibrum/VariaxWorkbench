#![forbid(unsafe_code)]
//! Offline test recorder for synthetic and file-backed audio.
//!
//! This is not device capture. Nothing here opens ALSA, PipeWire, PulseAudio,
//! USB, or MIDI, and nothing changes a live record route, monitor mix, or
//! re-amp path. `StartAudioCapture` stays locked in the session gate.

use std::path::{Path, PathBuf};

use crate::document;
use crate::pod_audio::{
    BUFFER_BYTES_MAX, FRAME_BYTES, MonitorMix, PlanError, ProcessingSource, RATE_DENOMINATOR,
    RATE_MAX_FIELD, RATE_MIN_FIELD, RATE_NUMERATOR, RecordContent, RecordRoute, accept_rate,
    behavior, pack_stereo, plan_buffer, unpack_stereo,
};

/// Offline streaming chunk ceiling. It is the declared POD period-byte maximum,
/// used here only as a memory bound. It is not a measurement that a 44100 or
/// 48000 stream uses the device buffer.
pub const STREAM_CHUNK_BYTES_MAX: usize = 8_192;

/// Largest take a caller may request. The declared device period product
/// ([`BUFFER_BYTES_MAX`], 60000) is a different limit and is not this ceiling.
pub const TAKE_BYTES_CEILING: u64 = 32 * 1024 * 1024;

/// One injected gap cannot be larger than this many frames.
pub const MAX_DROPOUT_FRAMES: u32 = 2_048;

/// Frames the panel copies on one explicit Feed click.
pub const FEED_FRAMES: u32 = 256;

/// In-memory route plan checked when a take is armed. This is the smallest
/// accepted device-style period, not the recording length.
pub const OFFLINE_ROUTE_PERIOD_BYTES: u32 = 66;
pub const OFFLINE_ROUTE_PERIODS: u32 = 2;

/// Integer rates that may be written into a canonical PCM WAV header.
/// 44100 and 48000 are the GearBox digital-clock sentences. The others are
/// ordinary file clocks for offline tests. None of them is `pod_ratden`.
pub const INTEGER_WAV_RATES: [u32; 5] = [22_050, 44_100, 48_000, 88_200, 96_000];

pub const OFFLINE_BANNER: &str =
    "Offline test recording. This is not a recording from a connected device.";
pub const DEVICE_CAPTURE_LOCKED: &str = "Device capture and playback stay locked. No ALSA, PipeWire, PulseAudio, USB, or MIDI port is opened.";
pub const NO_AUTO_START: &str = "Opening this window does not start a take, change a record route, or turn monitoring or re-amping on.";
pub const PHASE_IDLE: &str = "Idle. Device capture is locked.";
pub const PHASE_RECORDING: &str = "Recording test audio in memory. No device is open.";
pub const PHASE_STOPPED: &str = "Stopped. A file is written only when you save.";
pub const PHASE_CANCELLED: &str = "Cancelled. No audio file was written.";

const PANEL_TAKE_BYTES: u64 = 1024 * 1024;
const LABEL_CHARS_MAX: usize = 240;
const WAV_FRAME_BYTES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleClock {
    /// Truthful integer Hertz for a synthetic or file-backed stream.
    Integer { hz: u32 },
    /// Linux `pod_ratden` evidence. PCM WAV cannot store this rational.
    NativePodxt { numerator: u32, denominator: u32 },
}

impl SampleClock {
    pub const fn native_podxt() -> Self {
        Self::NativePodxt {
            numerator: RATE_NUMERATOR,
            denominator: RATE_DENOMINATOR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameFormat {
    /// Interleaved little-endian 16-bit stereo. One frame is 4 bytes.
    PcmS16Le,
    /// Declared PODxt frame. One frame is 6 bytes. WAV export is rejected.
    PodS24_3Le,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    Synthetic,
    File { label: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticPattern {
    Constant {
        left: i16,
        right: i16,
    },
    /// Frame `n` writes `n` truncated to `i16` on the left and its negation on the right.
    Ramp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Recording,
    Stopped,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levels {
    pub left_peak: u32,
    pub right_peak: u32,
    pub chunk_left_peak: u32,
    pub chunk_right_peak: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dropout {
    pub at_frame: u64,
    pub missing_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakeRequest {
    pub route: RecordRoute,
    pub clock: SampleClock,
    pub format: FrameFormat,
    pub origin: InputOrigin,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WavPcm {
    pub sample_rate_hz: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTake {
    pub wav_path: PathBuf,
    pub sidecar_path: PathBuf,
    pub pcm_sha256: String,
    pub wav_sha256: String,
    pub frames: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TakeError {
    SelectionMissing(&'static str),
    FalseIntegerStandIn { hz: u32 },
    IntegerRateNotWav { hz: u32 },
    RationalClockNotWav { numerator: u32, denominator: u32 },
    NativeClockRejected { numerator: u32, denominator: u32 },
    FormatClockMismatch,
    RateMismatch { header: u32, selected: u32 },
    ChunkTooLarge { len: usize },
    ChunkEmpty,
    ChunkNotFrameAligned { len: usize, frame_bytes: usize },
    ChunkRejected(PlanError),
    CapExceeded,
    TakeCapInvalid,
    NotRecording,
    NotStopped,
    AlreadyStarted,
    Cancelled,
    Busy,
    NothingLeft,
    DropoutEmpty,
    DropoutTooLarge,
    Wav(&'static str),
    AlreadyExists,
    LabelTooLong,
    Save(String),
    Integrity,
}

impl std::fmt::Display for TakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelectionMissing(item) => {
                write!(formatter, "select {item} before starting")
            }
            Self::FalseIntegerStandIn { hz } => write!(
                formatter,
                "{hz} is not a WAV sample rate for the PODxt source clock. 39062 and 39063 are the integer fields beside 78125/2, and 78125 is that rational's numerator. None of them is written into a WAV header."
            ),
            Self::IntegerRateNotWav { hz } => write!(
                formatter,
                "{hz} Hz is not one of the offline integer WAV clocks {INTEGER_WAV_RATES:?}"
            ),
            Self::RationalClockNotWav {
                numerator,
                denominator,
            } => write!(
                formatter,
                "PCM WAV cannot store the source clock {numerator}/{denominator}. Writing 39062 or 39063 would round it. No verified rational container and no tested conversion are implemented, so this export is rejected."
            ),
            Self::NativeClockRejected {
                numerator,
                denominator,
            } => write!(
                formatter,
                "only the source rational {RATE_NUMERATOR}/{RATE_DENOMINATOR} is accepted as the native clock; {numerator}/{denominator} is not that rational"
            ),
            Self::FormatClockMismatch => formatter.write_str(
                "integer clocks use 16-bit PCM frames, and the native 78125/2 clock uses declared S24_3LE frames. This pairing is rejected.",
            ),
            Self::RateMismatch { header, selected } => write!(
                formatter,
                "the WAV header rate {header} does not match the selected integer clock {selected}"
            ),
            Self::ChunkTooLarge { len } => write!(
                formatter,
                "a streaming chunk is {len} bytes, above the offline ceiling of {STREAM_CHUNK_BYTES_MAX}"
            ),
            Self::ChunkEmpty => formatter.write_str("a streaming chunk is empty"),
            Self::ChunkNotFrameAligned { len, frame_bytes } => write!(
                formatter,
                "a streaming chunk of {len} bytes is not a multiple of the {frame_bytes}-byte frame"
            ),
            Self::ChunkRejected(error) => write!(
                formatter,
                "native streaming chunk was rejected by the declared period window: {error}"
            ),
            Self::CapExceeded => formatter.write_str(
                "the take byte cap would be exceeded. No partial chunk or gap was stored.",
            ),
            Self::TakeCapInvalid => write!(
                formatter,
                "the take cap must be from 1 byte through the documented ceiling of {TAKE_BYTES_CEILING}. The {BUFFER_BYTES_MAX}-byte device period product is not this cap."
            ),
            Self::NotRecording => formatter.write_str("the take is not recording"),
            Self::NotStopped => {
                formatter.write_str("stop the take before saving. Start does not write a file.")
            }
            Self::AlreadyStarted => formatter.write_str(
                "a take is already in progress or waiting to be saved. Cancel it before starting another.",
            ),
            Self::Cancelled => {
                formatter.write_str("the take was cancelled. No audio file was written.")
            }
            Self::Busy => formatter.write_str(
                "cancel the current take before changing the route, clock, or input",
            ),
            Self::NothingLeft => formatter.write_str("the selected file has no frames left to feed"),
            Self::DropoutEmpty => formatter.write_str("a dropout gap needs at least one frame"),
            Self::DropoutTooLarge => write!(
                formatter,
                "one dropout is limited to {MAX_DROPOUT_FRAMES} frames"
            ),
            Self::Wav(detail) => write!(formatter, "WAV: {detail}"),
            Self::AlreadyExists => formatter.write_str(
                "an audio file or its sidecar already exists. This save does not overwrite.",
            ),
            Self::LabelTooLong => write!(
                formatter,
                "a file label is longer than {LABEL_CHARS_MAX} characters"
            ),
            Self::Save(detail) => write!(formatter, "save failed: {detail}"),
            Self::Integrity => {
                formatter.write_str("stored audio length does not match the frame count")
            }
        }
    }
}

impl std::error::Error for TakeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Take {
    route: RecordRoute,
    clock: SampleClock,
    format: FrameFormat,
    origin: InputOrigin,
    max_bytes: u64,
    pcm: Vec<u8>,
    frames: u64,
    phase: Phase,
    dropouts: Vec<Dropout>,
    peak_left: u32,
    peak_right: u32,
}

impl Take {
    pub fn prepare(request: TakeRequest) -> Result<Self, TakeError> {
        if request.max_bytes == 0 || request.max_bytes > TAKE_BYTES_CEILING {
            return Err(TakeError::TakeCapInvalid);
        }
        accept_pair(request.clock, request.format)?;
        if let InputOrigin::File { label } = &request.origin {
            if label.is_empty() {
                return Err(TakeError::SelectionMissing("file label"));
            }
            if label.chars().count() > LABEL_CHARS_MAX {
                return Err(TakeError::LabelTooLong);
            }
        }
        Ok(Self {
            route: request.route,
            clock: request.clock,
            format: request.format,
            origin: request.origin,
            max_bytes: request.max_bytes,
            pcm: Vec::new(),
            frames: 0,
            phase: Phase::Idle,
            dropouts: Vec::new(),
            peak_left: 0,
            peak_right: 0,
        })
    }

    pub fn start(&mut self) -> Result<(), TakeError> {
        if self.phase != Phase::Idle {
            return Err(TakeError::AlreadyStarted);
        }
        self.phase = Phase::Recording;
        Ok(())
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Levels, TakeError> {
        if self.phase != Phase::Recording {
            return Err(TakeError::NotRecording);
        }
        let frame_bytes = self.frame_bytes();
        if chunk.is_empty() {
            return Err(TakeError::ChunkEmpty);
        }
        if chunk.len() > STREAM_CHUNK_BYTES_MAX {
            return Err(TakeError::ChunkTooLarge { len: chunk.len() });
        }
        if !chunk.len().is_multiple_of(frame_bytes) {
            return Err(TakeError::ChunkNotFrameAligned {
                len: chunk.len(),
                frame_bytes,
            });
        }
        // Integer PCM uses the offline ceiling above. Native frames also have
        // to pass the declared period window as one period. That does not mean
        // a 44100 stream was measured against the device buffer.
        if self.format == FrameFormat::PodS24_3Le {
            plan_buffer(chunk.len() as u32, 1).map_err(TakeError::ChunkRejected)?;
        }
        let added = self.pcm.len().saturating_add(chunk.len()) as u64;
        if added > self.max_bytes {
            return Err(TakeError::CapExceeded);
        }
        let (chunk_left, chunk_right) = peaks(self.format, chunk);
        let frames = (chunk.len() / frame_bytes) as u64;
        self.peak_left = self.peak_left.max(chunk_left);
        self.peak_right = self.peak_right.max(chunk_right);
        self.frames += frames;
        self.pcm.extend_from_slice(chunk);
        Ok(self.levels_with_chunk(chunk_left, chunk_right))
    }

    /// Inserts an explicit run of zero frames. This models a test gap. It is
    /// not a measured USB dropout, and it writes nothing to disk.
    pub fn note_dropout(&mut self, missing_frames: u32) -> Result<(), TakeError> {
        if self.phase != Phase::Recording {
            return Err(TakeError::NotRecording);
        }
        if missing_frames == 0 {
            return Err(TakeError::DropoutEmpty);
        }
        if missing_frames > MAX_DROPOUT_FRAMES {
            return Err(TakeError::DropoutTooLarge);
        }
        let added = u64::from(missing_frames) * self.frame_bytes() as u64;
        if self.pcm.len() as u64 + added > self.max_bytes {
            return Err(TakeError::CapExceeded);
        }
        let added_usize = usize::try_from(added).map_err(|_| TakeError::CapExceeded)?;
        self.dropouts.push(Dropout {
            at_frame: self.frames,
            missing_frames,
        });
        self.frames += u64::from(missing_frames);
        self.pcm.resize(self.pcm.len() + added_usize, 0);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), TakeError> {
        if self.phase != Phase::Recording {
            return Err(TakeError::NotRecording);
        }
        self.phase = Phase::Stopped;
        Ok(())
    }

    /// Discards the audio. A later save creates no file.
    pub fn cancel(&mut self) -> Result<(), TakeError> {
        if !matches!(self.phase, Phase::Recording | Phase::Stopped) {
            return Err(TakeError::NotRecording);
        }
        self.pcm.clear();
        self.frames = 0;
        self.dropouts.clear();
        self.peak_left = 0;
        self.peak_right = 0;
        self.phase = Phase::Cancelled;
        Ok(())
    }

    pub fn save(&self, wav_path: &Path) -> Result<SavedTake, TakeError> {
        if self.phase == Phase::Cancelled {
            return Err(TakeError::Cancelled);
        }
        if self.phase != Phase::Stopped {
            return Err(TakeError::NotStopped);
        }
        let hz = match self.clock {
            SampleClock::Integer { hz } => hz,
            SampleClock::NativePodxt {
                numerator,
                denominator,
            } => {
                return Err(TakeError::RationalClockNotWav {
                    numerator,
                    denominator,
                });
            }
        };
        if self.format != FrameFormat::PcmS16Le {
            return Err(TakeError::FormatClockMismatch);
        }
        accept_integer_rate(hz)?;
        if self.pcm.len() as u64 != self.frames * WAV_FRAME_BYTES as u64 {
            return Err(TakeError::Integrity);
        }
        let wav = write_wav(hz, &self.pcm)?;
        let sidecar_path = sidecar_path(wav_path)?;
        if path_exists(wav_path)? || path_exists(&sidecar_path)? {
            return Err(TakeError::AlreadyExists);
        }
        let pcm_sha256 = sha256_hex(&self.pcm);
        let wav_sha256 = sha256_hex(&wav);
        let sidecar = self.sidecar_json(hz, &pcm_sha256, &wav_sha256);
        document::save_new(wav_path, &wav).map_err(TakeError::Save)?;
        if let Err(error) = document::save_new(&sidecar_path, sidecar.as_bytes()) {
            return Err(TakeError::Save(format!(
                "{error}. The WAV was written and the sidecar was not."
            )));
        }
        Ok(SavedTake {
            wav_path: wav_path.to_path_buf(),
            sidecar_path,
            pcm_sha256,
            wav_sha256,
            frames: self.frames,
        })
    }

    pub const fn phase(&self) -> Phase {
        self.phase
    }

    pub const fn phase_label(&self) -> &'static str {
        match self.phase {
            Phase::Idle => PHASE_IDLE,
            Phase::Recording => PHASE_RECORDING,
            Phase::Stopped => PHASE_STOPPED,
            Phase::Cancelled => PHASE_CANCELLED,
        }
    }

    pub const fn route(&self) -> RecordRoute {
        self.route
    }

    pub const fn clock(&self) -> SampleClock {
        self.clock
    }

    pub const fn format(&self) -> FrameFormat {
        self.format
    }

    pub fn pcm(&self) -> &[u8] {
        &self.pcm
    }

    pub const fn frames(&self) -> u64 {
        self.frames
    }

    pub fn dropouts(&self) -> &[Dropout] {
        &self.dropouts
    }

    pub const fn levels(&self) -> Levels {
        Levels {
            left_peak: self.peak_left,
            right_peak: self.peak_right,
            chunk_left_peak: 0,
            chunk_right_peak: 0,
        }
    }

    pub const fn device_opened(&self) -> bool {
        false
    }

    pub const fn outbound_len(&self) -> usize {
        0
    }

    pub const fn live_route_changed(&self) -> bool {
        false
    }

    pub const fn live_monitor_changed(&self) -> bool {
        false
    }

    const fn frame_bytes(&self) -> usize {
        match self.format {
            FrameFormat::PcmS16Le => WAV_FRAME_BYTES,
            FrameFormat::PodS24_3Le => FRAME_BYTES as usize,
        }
    }

    const fn levels_with_chunk(&self, chunk_left: u32, chunk_right: u32) -> Levels {
        Levels {
            left_peak: self.peak_left,
            right_peak: self.peak_right,
            chunk_left_peak: chunk_left,
            chunk_right_peak: chunk_right,
        }
    }

    fn sidecar_json(&self, sample_rate_hz: u32, pcm_sha256: &str, wav_sha256: &str) -> String {
        let route = behavior(self.route);
        let (input, label) = match &self.origin {
            InputOrigin::Synthetic => ("synthetic", String::new()),
            InputOrigin::File { label } => ("file", json_string(label)),
        };
        let file_label = if label.is_empty() {
            "\"\"".to_string()
        } else {
            label
        };
        let mut gaps = String::from("[");
        for (index, gap) in self.dropouts.iter().enumerate() {
            if index > 0 {
                gaps.push(',');
            }
            gaps.push_str(&format!(
                "{{\"at_frame\":{},\"missing_frames\":{}}}",
                gap.at_frame, gap.missing_frames
            ));
        }
        gaps.push(']');
        format!(
            concat!(
                "{{\n",
                "  \"kind\": \"offline-test-recording\",\n",
                "  \"device_recording\": {device_recording},\n",
                "  \"device_opened\": {device_opened},\n",
                "  \"live_route_changed\": {live_route},\n",
                "  \"live_monitor_changed\": {live_monitor},\n",
                "  \"outbound_bytes\": {outbound},\n",
                "  \"route\": \"{route}\",\n",
                "  \"mode_number\": {mode},\n",
                "  \"record_send\": \"{content}\",\n",
                "  \"processing_source\": \"{source}\",\n",
                "  \"monitor_as_documented\": \"{monitor}\",\n",
                "  \"reamp_pass\": {reamp},\n",
                "  \"input\": \"{input}\",\n",
                "  \"file_label\": {file_label},\n",
                "  \"clock\": \"integer\",\n",
                "  \"sample_rate_hz\": {rate},\n",
                "  \"native_clock_exported\": false,\n",
                "  \"format\": \"pcm-s16le\",\n",
                "  \"channels\": 2,\n",
                "  \"frames\": {frames},\n",
                "  \"pcm_bytes\": {pcm_bytes},\n",
                "  \"dropout_count\": {dropouts},\n",
                "  \"dropouts\": {gaps},\n",
                "  \"peak_left\": {left},\n",
                "  \"peak_right\": {right},\n",
                "  \"pcm_sha256\": \"{pcm_sha}\",\n",
                "  \"wav_sha256\": \"{wav_sha}\",\n",
                "  \"stream_chunk_bytes_max\": {chunk_max},\n",
                "  \"take_byte_cap\": {take_cap},\n",
                "  \"take_bytes_ceiling\": {ceiling},\n",
                "  \"device_buffer_bytes_max\": {device_max},\n",
                "  \"note\": \"Offline synthetic or file-backed take. Not a measurement from a connected PODxt Live. No DAW was opened. Latency was not measured.\"\n",
                "}}\n",
            ),
            device_recording = false,
            device_opened = self.device_opened(),
            live_route = self.live_route_changed(),
            live_monitor = self.live_monitor_changed(),
            outbound = self.outbound_len(),
            route = route_id(self.route),
            mode = route.mode_number,
            content = content_id(route.usb_record_send),
            source = source_id(route.processing_source),
            monitor = monitor_id(route.monitor),
            reamp = route.reamp_pass,
            input = input,
            file_label = file_label,
            rate = sample_rate_hz,
            frames = self.frames,
            pcm_bytes = self.pcm.len(),
            dropouts = self.dropouts.len(),
            gaps = gaps,
            left = self.peak_left,
            right = self.peak_right,
            pcm_sha = pcm_sha256,
            wav_sha = wav_sha256,
            chunk_max = STREAM_CHUNK_BYTES_MAX,
            take_cap = self.max_bytes,
            ceiling = TAKE_BYTES_CEILING,
            device_max = BUFFER_BYTES_MAX,
        )
    }
}

pub fn render_pcm_s16(pattern: SyntheticPattern, start_frame: u64, frames: u32) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(frames as usize * WAV_FRAME_BYTES);
    for index in 0..frames {
        let (left, right) = sample_s16(pattern, start_frame + u64::from(index));
        pcm.extend_from_slice(&left.to_le_bytes());
        pcm.extend_from_slice(&right.to_le_bytes());
    }
    pcm
}

pub fn render_pod_s24(
    pattern: SyntheticPattern,
    start_frame: u64,
    frames: u32,
) -> Result<Vec<u8>, TakeError> {
    let mut pcm = Vec::with_capacity(frames as usize * FRAME_BYTES as usize);
    for index in 0..frames {
        let (left, right) = sample_s16(pattern, start_frame + u64::from(index));
        let packed =
            pack_stereo(i32::from(left), i32::from(right)).map_err(TakeError::ChunkRejected)?;
        pcm.extend_from_slice(&packed);
    }
    Ok(pcm)
}

pub fn write_wav(sample_rate_hz: u32, pcm: &[u8]) -> Result<Vec<u8>, TakeError> {
    accept_integer_rate(sample_rate_hz)?;
    if !pcm.len().is_multiple_of(WAV_FRAME_BYTES) {
        return Err(TakeError::ChunkNotFrameAligned {
            len: pcm.len(),
            frame_bytes: WAV_FRAME_BYTES,
        });
    }
    let data_size = u32::try_from(pcm.len()).map_err(|_| TakeError::CapExceeded)?;
    let riff_size = data_size.checked_add(36).ok_or(TakeError::CapExceeded)?;
    let byte_rate = sample_rate_hz
        .checked_mul(WAV_FRAME_BYTES as u32)
        .ok_or(TakeError::IntegerRateNotWav { hz: sample_rate_hz })?;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm);
    Ok(wav)
}

pub fn parse_wav(bytes: &[u8]) -> Result<WavPcm, TakeError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(TakeError::Wav("missing RIFF/WAVE"));
    }
    let riff_size = le_u32(&bytes[4..8])? as usize;
    if bytes.len() < 8 || riff_size != bytes.len() - 8 {
        return Err(TakeError::Wav("RIFF size does not match the file"));
    }
    let mut offset = 12usize;
    let mut fmt: Option<&[u8]> = None;
    let mut data: Option<&[u8]> = None;
    while offset < bytes.len() {
        if offset + 8 > bytes.len() {
            return Err(TakeError::Wav("truncated chunk header"));
        }
        let id = &bytes[offset..offset + 4];
        let size = le_u32(&bytes[offset + 4..offset + 8])? as usize;
        if size > TAKE_BYTES_CEILING as usize {
            return Err(TakeError::CapExceeded);
        }
        let body_start = offset + 8;
        let body_end = body_start
            .checked_add(size)
            .ok_or(TakeError::Wav("chunk size overflows"))?;
        if body_end > bytes.len() {
            return Err(TakeError::Wav("truncated chunk"));
        }
        if !size.is_multiple_of(2) {
            return Err(TakeError::Wav("odd WAV chunk sizes are not accepted"));
        }
        let body = &bytes[body_start..body_end];
        match id {
            b"fmt " => {
                if fmt.is_some() {
                    return Err(TakeError::Wav("duplicate fmt chunk"));
                }
                fmt = Some(body);
            }
            b"data" => {
                if fmt.is_none() {
                    return Err(TakeError::Wav("data chunk before fmt"));
                }
                if data.is_some() {
                    return Err(TakeError::Wav("duplicate data chunk"));
                }
                data = Some(body);
            }
            _ => {}
        }
        offset = body_end;
    }
    if offset != bytes.len() {
        return Err(TakeError::Wav("trailing bytes after WAV chunks"));
    }
    let fmt = fmt.ok_or(TakeError::Wav("missing fmt chunk"))?;
    let data = data.ok_or(TakeError::Wav("missing data chunk"))?;
    parse_fmt(fmt, data)
}

pub fn sidecar_path(wav_path: &Path) -> Result<PathBuf, TakeError> {
    let Some(file_name) = wav_path.file_name() else {
        return Err(TakeError::Save("save path needs a file name".into()));
    };
    if file_name.is_empty() {
        return Err(TakeError::Save("save path needs a file name".into()));
    }
    let mut sidecar_name = file_name.to_os_string();
    sidecar_name.push(".recording.json");
    Ok(wav_path.with_file_name(sidecar_name))
}

fn parse_fmt(fmt: &[u8], data: &[u8]) -> Result<WavPcm, TakeError> {
    if fmt.len() != 16 {
        return Err(TakeError::Wav(
            "only a 16-byte canonical PCM fmt chunk is accepted",
        ));
    }
    if le_u16(&fmt[0..2])? != 1 {
        return Err(TakeError::Wav(
            "only canonical integer PCM format 1 is accepted",
        ));
    }
    if le_u16(&fmt[2..4])? != 2 {
        return Err(TakeError::Wav("only stereo WAV files are accepted"));
    }
    let sample_rate_hz = le_u32(&fmt[4..8])?;
    let byte_rate = le_u32(&fmt[8..12])?;
    if le_u16(&fmt[12..14])? != 4 || le_u16(&fmt[14..16])? != 16 {
        return Err(TakeError::Wav("only 16-bit stereo PCM is accepted"));
    }
    let expected_byte_rate = sample_rate_hz
        .checked_mul(4)
        .ok_or(TakeError::IntegerRateNotWav { hz: sample_rate_hz })?;
    if byte_rate != expected_byte_rate {
        return Err(TakeError::Wav("byte rate does not match the sample rate"));
    }
    if !data.len().is_multiple_of(WAV_FRAME_BYTES) {
        return Err(TakeError::Wav("data length is not a multiple of the frame"));
    }
    accept_integer_rate(sample_rate_hz)?;
    Ok(WavPcm {
        sample_rate_hz,
        data: data.to_vec(),
    })
}

fn accept_pair(clock: SampleClock, format: FrameFormat) -> Result<(), TakeError> {
    match (clock, format) {
        (SampleClock::Integer { hz }, FrameFormat::PcmS16Le) => accept_integer_rate(hz),
        (SampleClock::Integer { .. }, FrameFormat::PodS24_3Le) => {
            Err(TakeError::FormatClockMismatch)
        }
        (
            SampleClock::NativePodxt {
                numerator,
                denominator,
            },
            FrameFormat::PodS24_3Le,
        ) => accept_rate(numerator, denominator).map_err(|_| TakeError::NativeClockRejected {
            numerator,
            denominator,
        }),
        (
            SampleClock::NativePodxt {
                numerator,
                denominator,
            },
            FrameFormat::PcmS16Le,
        ) => Err(TakeError::RationalClockNotWav {
            numerator,
            denominator,
        }),
    }
}

fn accept_integer_rate(hz: u32) -> Result<(), TakeError> {
    if hz == RATE_MIN_FIELD || hz == RATE_MAX_FIELD || hz == RATE_NUMERATOR {
        return Err(TakeError::FalseIntegerStandIn { hz });
    }
    if INTEGER_WAV_RATES.contains(&hz) {
        Ok(())
    } else {
        Err(TakeError::IntegerRateNotWav { hz })
    }
}

fn sample_s16(pattern: SyntheticPattern, frame: u64) -> (i16, i16) {
    match pattern {
        SyntheticPattern::Constant { left, right } => (left, right),
        SyntheticPattern::Ramp => {
            let left = frame as i16;
            (left, left.wrapping_neg())
        }
    }
}

fn peaks(format: FrameFormat, chunk: &[u8]) -> (u32, u32) {
    let mut left = 0u32;
    let mut right = 0u32;
    match format {
        FrameFormat::PcmS16Le => {
            for frame in chunk.as_chunks::<WAV_FRAME_BYTES>().0 {
                let sample_left = i16::from_le_bytes([frame[0], frame[1]]);
                let sample_right = i16::from_le_bytes([frame[2], frame[3]]);
                left = left.max(u32::from(sample_left.unsigned_abs()));
                right = right.max(u32::from(sample_right.unsigned_abs()));
            }
        }
        FrameFormat::PodS24_3Le => {
            for frame in chunk.as_chunks::<{ FRAME_BYTES as usize }>().0 {
                let (sample_left, sample_right) = unpack_stereo(*frame);
                left = left.max(sample_left.unsigned_abs());
                right = right.max(sample_right.unsigned_abs());
            }
        }
    }
    (left, right)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes))
}

fn json_string(value: &str) -> String {
    let mut encoded = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            other if other.is_control() => {
                encoded.push_str(&format!("\\u{:04x}", u32::from(other)));
            }
            other => encoded.push(other),
        }
    }
    encoded.push('"');
    encoded
}

fn path_exists(path: &Path) -> Result<bool, TakeError> {
    path.try_exists()
        .map_err(|error| TakeError::Save(error.to_string()))
}

fn le_u16(bytes: &[u8]) -> Result<u16, TakeError> {
    let array: [u8; 2] = bytes
        .try_into()
        .map_err(|_| TakeError::Wav("truncated integer"))?;
    Ok(u16::from_le_bytes(array))
}

fn le_u32(bytes: &[u8]) -> Result<u32, TakeError> {
    let array: [u8; 4] = bytes
        .try_into()
        .map_err(|_| TakeError::Wav("truncated integer"))?;
    Ok(u32::from_le_bytes(array))
}

const fn route_id(route: RecordRoute) -> &'static str {
    match route {
        RecordRoute::SendProcessedGuitar => "SendProcessedGuitar",
        RecordRoute::SendCleanGuitar => "SendCleanGuitar",
        RecordRoute::SendCleanGuitarReampPlayback => "SendCleanGuitarReampPlayback",
        RecordRoute::SendReampPlayback => "SendReampPlayback",
    }
}

const fn content_id(content: RecordContent) -> &'static str {
    match content {
        RecordContent::ProcessedGuitar => "ProcessedGuitar",
        RecordContent::UnprocessedGuitar => "UnprocessedGuitar",
    }
}

const fn source_id(source: ProcessingSource) -> &'static str {
    match source {
        ProcessingSource::Instrument => "Instrument",
        ProcessingSource::HostPlayback => "HostPlayback",
        ProcessingSource::CleanGuitarRecordedWhileHostFeedsProcessing => {
            "CleanGuitarRecordedWhileHostFeedsProcessing"
        }
    }
}

const fn monitor_id(monitor: MonitorMix) -> &'static str {
    match monitor {
        MonitorMix::ProcessedGuitarAndHostPlayback => "ProcessedGuitarAndHostPlayback",
        MonitorMix::ProcessedHostPlayback => "ProcessedHostPlayback",
        MonitorMix::NotStatedForThisMode => "NotStatedForThisMode",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockSelection {
    Unselected,
    Integer(u32),
    NativePodxt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSelection {
    Unselected,
    Synthetic,
    File,
}

pub const ROUTE_CHOICES: [(RecordRoute, &str); 4] = [
    (RecordRoute::SendProcessedGuitar, "Send processed guitar"),
    (RecordRoute::SendCleanGuitar, "Send clean guitar"),
    (
        RecordRoute::SendCleanGuitarReampPlayback,
        "Send clean guitar, re-amp playback",
    ),
    (RecordRoute::SendReampPlayback, "Send re-amp playback"),
];

pub const CLOCK_CHOICES: [(ClockSelection, &str); 3] = [
    (
        ClockSelection::Integer(44_100),
        "44100 Hz integer test clock",
    ),
    (
        ClockSelection::Integer(48_000),
        "48000 Hz integer test clock",
    ),
    (
        ClockSelection::NativePodxt,
        "PODxt source clock 78125/2 (no WAV export)",
    ),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panel {
    route: Option<RecordRoute>,
    clock: ClockSelection,
    input: InputSelection,
    loaded: Option<WavPcm>,
    file_label: String,
    pattern: SyntheticPattern,
    take: Option<Take>,
    cursor: u64,
    levels: Levels,
    message: String,
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel {
    pub fn new() -> Self {
        Self {
            route: None,
            clock: ClockSelection::Unselected,
            input: InputSelection::Unselected,
            loaded: None,
            file_label: String::new(),
            pattern: SyntheticPattern::Constant {
                left: 1_000,
                right: -2_000,
            },
            take: None,
            cursor: 0,
            levels: Levels::default(),
            message: "Select a route, an input, and a clock. Nothing starts until you press Start."
                .into(),
        }
    }

    pub fn route(&self) -> Option<RecordRoute> {
        self.route
    }

    pub fn clock(&self) -> ClockSelection {
        self.clock
    }

    pub fn input(&self) -> InputSelection {
        self.input
    }

    pub fn phase(&self) -> Phase {
        self.take.as_ref().map(Take::phase).unwrap_or(Phase::Idle)
    }

    pub fn phase_label(&self) -> &'static str {
        self.take
            .as_ref()
            .map(Take::phase_label)
            .unwrap_or(PHASE_IDLE)
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn levels(&self) -> Levels {
        self.levels
    }

    pub fn meter_line(&self) -> String {
        format!(
            "Left peak {}  Right peak {}",
            self.levels.left_peak, self.levels.right_peak
        )
    }

    pub const fn device_opened(&self) -> bool {
        false
    }

    pub const fn live_monitor_changed(&self) -> bool {
        false
    }

    pub const fn live_route_changed(&self) -> bool {
        false
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.message = text.into();
    }

    pub fn select_route(&mut self, route: RecordRoute) -> Result<(), TakeError> {
        self.ensure_idle()?;
        self.route = Some(route);
        Ok(())
    }

    pub fn select_clock(&mut self, clock: ClockSelection) -> Result<(), TakeError> {
        self.ensure_idle()?;
        let loaded_rate = self.loaded.as_ref().map(|wav| wav.sample_rate_hz);
        self.clock = clock;
        if let Some(rate) = loaded_rate {
            let same_integer = matches!(clock, ClockSelection::Integer(hz) if hz == rate);
            if !same_integer {
                self.loaded = None;
                self.file_label.clear();
            }
        }
        Ok(())
    }

    pub fn select_input(&mut self, input: InputSelection) -> Result<(), TakeError> {
        self.ensure_idle()?;
        self.input = input;
        Ok(())
    }

    pub fn load_wav(&mut self, label: &str, bytes: &[u8]) -> Result<(), TakeError> {
        self.ensure_idle()?;
        let selected = match self.clock {
            ClockSelection::Integer(hz) => hz,
            ClockSelection::NativePodxt => {
                return Err(TakeError::RationalClockNotWav {
                    numerator: RATE_NUMERATOR,
                    denominator: RATE_DENOMINATOR,
                });
            }
            ClockSelection::Unselected => {
                return Err(TakeError::SelectionMissing("an integer clock"));
            }
        };
        if label.chars().count() > LABEL_CHARS_MAX {
            return Err(TakeError::LabelTooLong);
        }
        let wav = parse_wav(bytes)?;
        if wav.sample_rate_hz != selected {
            return Err(TakeError::RateMismatch {
                header: wav.sample_rate_hz,
                selected,
            });
        }
        if wav.data.len() as u64 > PANEL_TAKE_BYTES {
            return Err(TakeError::CapExceeded);
        }
        self.file_label = label.to_string();
        self.loaded = Some(wav);
        self.input = InputSelection::File;
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), TakeError> {
        if self.take.is_some() {
            return Err(TakeError::AlreadyStarted);
        }
        let route = self
            .route
            .ok_or(TakeError::SelectionMissing("a record route"))?;
        let (clock, format, origin) = match (self.clock, self.input) {
            (ClockSelection::Integer(hz), InputSelection::Synthetic) => (
                SampleClock::Integer { hz },
                FrameFormat::PcmS16Le,
                InputOrigin::Synthetic,
            ),
            (ClockSelection::Integer(hz), InputSelection::File) => {
                let loaded = self
                    .loaded
                    .as_ref()
                    .ok_or(TakeError::SelectionMissing("a loaded wav"))?;
                if loaded.sample_rate_hz != hz {
                    return Err(TakeError::RateMismatch {
                        header: loaded.sample_rate_hz,
                        selected: hz,
                    });
                }
                (
                    SampleClock::Integer { hz },
                    FrameFormat::PcmS16Le,
                    InputOrigin::File {
                        label: self.file_label.clone(),
                    },
                )
            }
            (ClockSelection::NativePodxt, InputSelection::Synthetic) => (
                SampleClock::native_podxt(),
                FrameFormat::PodS24_3Le,
                InputOrigin::Synthetic,
            ),
            (ClockSelection::NativePodxt, InputSelection::File) => {
                return Err(TakeError::RationalClockNotWav {
                    numerator: RATE_NUMERATOR,
                    denominator: RATE_DENOMINATOR,
                });
            }
            (ClockSelection::Unselected, _) => {
                return Err(TakeError::SelectionMissing("a clock"));
            }
            (_, InputSelection::Unselected) => {
                return Err(TakeError::SelectionMissing("an input"));
            }
        };
        let mut take = Take::prepare(TakeRequest {
            route,
            clock,
            format,
            origin,
            max_bytes: PANEL_TAKE_BYTES,
        })?;
        take.start()?;
        self.cursor = 0;
        self.levels = Levels::default();
        self.take = Some(take);
        self.message = "Recording armed. Feed a chunk, then stop. Nothing is saved yet.".into();
        Ok(())
    }

    pub fn feed(&mut self) -> Result<(), TakeError> {
        let format = self.take.as_ref().ok_or(TakeError::NotRecording)?.format();
        let bytes = match (format, self.input) {
            (FrameFormat::PcmS16Le, InputSelection::File) => {
                let loaded = self
                    .loaded
                    .as_ref()
                    .ok_or(TakeError::SelectionMissing("a loaded wav"))?;
                let start = usize::try_from(self.cursor * WAV_FRAME_BYTES as u64)
                    .map_err(|_| TakeError::CapExceeded)?;
                if start >= loaded.data.len() {
                    return Err(TakeError::NothingLeft);
                }
                let end = start
                    .saturating_add(FEED_FRAMES as usize * WAV_FRAME_BYTES)
                    .min(loaded.data.len());
                loaded.data[start..end].to_vec()
            }
            (FrameFormat::PcmS16Le, _) => render_pcm_s16(self.pattern, self.cursor, FEED_FRAMES),
            (FrameFormat::PodS24_3Le, _) => render_pod_s24(self.pattern, self.cursor, FEED_FRAMES)?,
        };
        if bytes.is_empty() {
            return Err(TakeError::NothingLeft);
        }
        let frame_bytes = if format == FrameFormat::PcmS16Le {
            WAV_FRAME_BYTES
        } else {
            FRAME_BYTES as usize
        };
        let frames = (bytes.len() / frame_bytes) as u64;
        let levels = self
            .take
            .as_mut()
            .ok_or(TakeError::NotRecording)?
            .push(&bytes)?;
        self.cursor += frames;
        self.levels = levels;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), TakeError> {
        self.take.as_mut().ok_or(TakeError::NotRecording)?.stop()?;
        self.message = "Stopped. Save writes a new file, or cancel discards it.".into();
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), TakeError> {
        self.take
            .as_mut()
            .ok_or(TakeError::NotRecording)?
            .cancel()?;
        self.take = None;
        self.cursor = 0;
        self.levels = Levels::default();
        self.message = PHASE_CANCELLED.into();
        Ok(())
    }

    pub fn save(&mut self, path: &Path) -> Result<SavedTake, TakeError> {
        let saved = self
            .take
            .as_ref()
            .ok_or(TakeError::NotStopped)?
            .save(path)?;
        self.message = format!("Saved {}", path.display());
        Ok(saved)
    }

    pub fn guide_line(&self) -> String {
        match self.route {
            Some(route) => {
                let behavior = behavior(route);
                format!(
                    "Guide label, not a live route change: mode {} {} reamp {}",
                    behavior.mode_number,
                    content_id(behavior.usb_record_send),
                    behavior.reamp_pass
                )
            }
            None => "Guide label, not a live route change: no route selected".into(),
        }
    }

    fn ensure_idle(&self) -> Result<(), TakeError> {
        if self.take.is_some() {
            Err(TakeError::Busy)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_request(route: RecordRoute, hz: u32, max_bytes: u64) -> TakeRequest {
        TakeRequest {
            route,
            clock: SampleClock::Integer { hz },
            format: FrameFormat::PcmS16Le,
            origin: InputOrigin::Synthetic,
            max_bytes,
        }
    }

    fn header_rate(wav: &[u8]) -> u32 {
        u32::from_le_bytes(wav[24..28].try_into().unwrap())
    }

    #[test]
    fn truthful_wav_save_refuses_overwrite_and_keeps_the_route_label() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clean.wav");
        let mut take =
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 44_100, 4_096)).unwrap();
        assert!(take.start().is_ok());
        assert!(take.pcm().is_empty());
        let chunk = render_pcm_s16(
            SyntheticPattern::Constant {
                left: 100,
                right: -50,
            },
            0,
            4,
        );
        take.push(&chunk).unwrap();
        take.stop().unwrap();
        let saved = take.save(&path).unwrap();
        let wav = std::fs::read(&path).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(header_rate(&wav), 44_100);
        assert_ne!(header_rate(&wav), RATE_MIN_FIELD);
        assert_ne!(header_rate(&wav), RATE_MAX_FIELD);
        let parsed = parse_wav(&wav).unwrap();
        assert_eq!(parsed.data, chunk);
        let sidecar = std::fs::read_to_string(&saved.sidecar_path).unwrap();
        assert!(sidecar.contains("\"device_recording\": false"));
        assert!(sidecar.contains("\"device_opened\": false"));
        assert!(sidecar.contains("\"live_route_changed\": false"));
        assert!(sidecar.contains("\"live_monitor_changed\": false"));
        assert!(sidecar.contains("\"route\": \"SendCleanGuitar\""));
        assert!(sidecar.contains("\"mode_number\": 2"));
        assert!(sidecar.contains("\"record_send\": \"UnprocessedGuitar\""));
        assert!(sidecar.contains("\"reamp_pass\": false"));
        assert!(sidecar.contains("\"sample_rate_hz\": 44100"));
        assert!(sidecar.contains("\"native_clock_exported\": false"));
        assert!(!sidecar.contains("39062"));
        assert!(!sidecar.contains("39063"));
        assert!(!sidecar.contains("78125"));
        assert_eq!(sidecar_sha(&sidecar, "pcm_sha256"), sha256_hex(&chunk));
        assert_eq!(sidecar_sha(&sidecar, "wav_sha256"), sha256_hex(&wav));
        assert!(!take.device_opened());
        assert_eq!(take.outbound_len(), 0);
        let original = std::fs::read(&path).unwrap();
        assert!(matches!(take.save(&path), Err(TakeError::AlreadyExists)));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let occupied = directory.path().join("occupied.wav");
        std::fs::write(&occupied, b"keep").unwrap();
        assert!(matches!(
            take.save(&occupied),
            Err(TakeError::AlreadyExists)
        ));
        assert_eq!(std::fs::read(&occupied).unwrap(), b"keep");
        assert!(
            !directory
                .path()
                .join("occupied.wav.recording.json")
                .exists()
        );

        let reamp = directory.path().join("reamp.wav");
        let mut reamp_take = Take::prepare(integer_request(
            RecordRoute::SendReampPlayback,
            48_000,
            4_096,
        ))
        .unwrap();
        reamp_take.start().unwrap();
        reamp_take
            .push(&render_pcm_s16(SyntheticPattern::Ramp, 0, 1))
            .unwrap();
        reamp_take.stop().unwrap();
        let reamp_saved = reamp_take.save(&reamp).unwrap();
        let reamp_sidecar = std::fs::read_to_string(reamp_saved.sidecar_path).unwrap();
        assert!(reamp_sidecar.contains("\"route\": \"SendReampPlayback\""));
        assert!(reamp_sidecar.contains("\"reamp_pass\": true"));
        assert!(reamp_sidecar.contains("\"monitor_as_documented\": \"NotStatedForThisMode\""));
        assert!(reamp_sidecar.contains("\"live_monitor_changed\": false"));
        assert_eq!(header_rate(&std::fs::read(reamp).unwrap()), 48_000);
    }

    #[test]
    fn native_rational_export_rejects_without_rounding() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("native.wav");
        let mut take = Take::prepare(TakeRequest {
            route: RecordRoute::SendProcessedGuitar,
            clock: SampleClock::native_podxt(),
            format: FrameFormat::PodS24_3Le,
            origin: InputOrigin::Synthetic,
            max_bytes: 8_192,
        })
        .unwrap();
        assert_eq!(
            Take::prepare(TakeRequest {
                format: FrameFormat::PcmS16Le,
                ..take_request_from(&take)
            })
            .unwrap_err(),
            TakeError::RationalClockNotWav {
                numerator: RATE_NUMERATOR,
                denominator: RATE_DENOMINATOR,
            }
        );
        assert!(matches!(
            Take::prepare(TakeRequest {
                clock: SampleClock::NativePodxt {
                    numerator: RATE_MAX_FIELD,
                    denominator: 1,
                },
                format: FrameFormat::PodS24_3Le,
                ..take_request_from(&take)
            }),
            Err(TakeError::NativeClockRejected { .. })
        ));
        take.start().unwrap();
        let chunk =
            render_pod_s24(SyntheticPattern::Constant { left: 3, right: 4 }, 0, 11).unwrap();
        assert_eq!(chunk.len(), 66);
        take.push(&chunk).unwrap();
        take.stop().unwrap();
        let error = take.save(&path).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("78125/2"));
        assert!(text.contains("39062"));
        assert!(text.contains("39063"));
        assert!(matches!(
            error,
            TakeError::RationalClockNotWav {
                numerator: 78_125,
                denominator: 2,
            }
        ));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        assert!(!take.device_opened());
        assert!(crate::pod_audio::iso_buffers_if_known().is_err());
        assert!(crate::pod_audio::max_packet_size().is_err());
    }

    #[test]
    fn false_integer_stand_ins_are_rejected() {
        for hz in [RATE_MIN_FIELD, RATE_MAX_FIELD, RATE_NUMERATOR] {
            let error = Take::prepare(integer_request(RecordRoute::SendCleanGuitar, hz, 64));
            assert!(matches!(error, Err(TakeError::FalseIntegerStandIn { .. })));
            assert!(write_wav(hz, &[]).is_err());
        }
        let mut forged = write_wav(44_100, &[]).unwrap();
        forged[24..28].copy_from_slice(&RATE_MIN_FIELD.to_le_bytes());
        let byte_rate = RATE_MIN_FIELD * 4;
        forged[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        assert!(matches!(
            parse_wav(&forged),
            Err(TakeError::FalseIntegerStandIn { hz: RATE_MIN_FIELD })
        ));
    }

    #[test]
    fn file_backed_wav_round_trip_keeps_rate_and_bytes() {
        let pcm = render_pcm_s16(SyntheticPattern::Ramp, 0, 300);
        let encoded = write_wav(48_000, &pcm).unwrap();
        let mut with_fact = Vec::new();
        with_fact.extend_from_slice(&encoded[..36]);
        with_fact.extend_from_slice(b"fact");
        with_fact.extend_from_slice(&4u32.to_le_bytes());
        with_fact.extend_from_slice(&300u32.to_le_bytes());
        with_fact.extend_from_slice(&encoded[36..]);
        let riff_size = u32::try_from(with_fact.len() - 8).unwrap();
        with_fact[4..8].copy_from_slice(&riff_size.to_le_bytes());
        let parsed = parse_wav(&with_fact).unwrap();
        assert_eq!(parsed.sample_rate_hz, 48_000);
        assert_eq!(parsed.data, pcm);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.wav");
        let mut take = Take::prepare(TakeRequest {
            route: RecordRoute::SendProcessedGuitar,
            clock: SampleClock::Integer { hz: 48_000 },
            format: FrameFormat::PcmS16Le,
            origin: InputOrigin::File {
                label: "fixture.wav".into(),
            },
            max_bytes: 8_192,
        })
        .unwrap();
        take.start().unwrap();
        take.push(&pcm[..800]).unwrap();
        take.push(&pcm[800..]).unwrap();
        take.stop().unwrap();
        let saved = take.save(&path).unwrap();
        let stored = parse_wav(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored.sample_rate_hz, 48_000);
        assert_eq!(stored.data, pcm);
        let sidecar = std::fs::read_to_string(saved.sidecar_path).unwrap();
        assert!(sidecar.contains("\"input\": \"file\""));
        assert!(sidecar.contains("\"file_label\": \"fixture.wav\""));
        assert_eq!(sidecar_sha(&sidecar, "pcm_sha256"), sha256_hex(&pcm));
        assert!(
            Take::prepare(TakeRequest {
                clock: SampleClock::Integer { hz: 44_100 },
                origin: InputOrigin::File {
                    label: "fixture.wav".into(),
                },
                ..integer_request(RecordRoute::SendProcessedGuitar, 44_100, 8_192)
            })
            .is_ok()
        );
        let mismatch = parse_wav(&encoded).unwrap();
        assert_eq!(
            Panel::new()
                .load_after_clock(44_100, "fixture.wav", &encoded)
                .unwrap_err(),
            TakeError::RateMismatch {
                header: mismatch.sample_rate_hz,
                selected: 44_100,
            }
        );
    }

    #[test]
    fn dropout_inserts_exact_zeros_and_records_the_gap() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("gap.wav");
        let mut take =
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 44_100, 4_096)).unwrap();
        take.start().unwrap();
        let tone = render_pcm_s16(
            SyntheticPattern::Constant {
                left: 256,
                right: 256,
            },
            0,
            4,
        );
        take.push(&tone).unwrap();
        take.note_dropout(2).unwrap();
        take.push(&tone).unwrap();
        assert_eq!(
            take.dropouts(),
            &[Dropout {
                at_frame: 4,
                missing_frames: 2
            }]
        );
        let mut expected = tone.clone();
        expected.extend_from_slice(&[0u8; 8]);
        expected.extend_from_slice(&tone);
        assert_eq!(take.pcm(), expected);
        assert_eq!(take.levels().left_peak, 256);
        take.stop().unwrap();
        let saved = take.save(&path).unwrap();
        let sidecar = std::fs::read_to_string(saved.sidecar_path).unwrap();
        assert!(sidecar.contains("\"dropout_count\": 1"));
        assert!(sidecar.contains("{\"at_frame\":4,\"missing_frames\":2}"));
        assert_eq!(
            parse_wav(&std::fs::read(&path).unwrap()).unwrap().data,
            expected
        );
        assert_eq!(sidecar_sha(&sidecar, "pcm_sha256"), sha256_hex(&expected));
        let mut overflowing =
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 44_100, 16)).unwrap();
        overflowing.start().unwrap();
        overflowing.push(&tone).unwrap();
        let unchanged = overflowing.pcm().to_vec();
        assert_eq!(overflowing.note_dropout(2), Err(TakeError::CapExceeded));
        assert_eq!(overflowing.pcm(), unchanged);
    }

    #[test]
    fn cancel_discards_the_take_and_writes_no_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cancelled.wav");
        let mut take =
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 22_050, 4_096)).unwrap();
        assert!(matches!(take.cancel(), Err(TakeError::NotRecording)));
        assert!(matches!(take.save(&path), Err(TakeError::NotStopped)));
        take.start().unwrap();
        take.push(&render_pcm_s16(SyntheticPattern::Ramp, 0, 8))
            .unwrap();
        take.cancel().unwrap();
        assert_eq!(take.phase(), Phase::Cancelled);
        assert!(take.pcm().is_empty());
        assert!(matches!(take.save(&path), Err(TakeError::Cancelled)));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        assert!(!take.device_opened());
    }

    #[test]
    fn streaming_chunks_and_take_cap_are_separate_from_the_device_period() {
        assert!(matches!(
            Take::prepare(integer_request(
                RecordRoute::SendCleanGuitar,
                44_100,
                TAKE_BYTES_CEILING + 1,
            )),
            Err(TakeError::TakeCapInvalid)
        ));
        assert!(matches!(
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 44_100, 0)),
            Err(TakeError::TakeCapInvalid)
        ));
        let mut take = Take::prepare(integer_request(
            RecordRoute::SendCleanGuitar,
            96_000,
            70_000,
        ))
        .unwrap();
        take.start().unwrap();
        let small = render_pcm_s16(SyntheticPattern::Ramp, 0, 3);
        assert_eq!(small.len(), 12);
        take.push(&small).unwrap();
        let block = vec![1u8; STREAM_CHUNK_BYTES_MAX];
        for _ in 0..8 {
            take.push(&block).unwrap();
        }
        assert_eq!(take.pcm().len(), 12 + 8 * STREAM_CHUNK_BYTES_MAX);
        assert!(take.pcm().len() > BUFFER_BYTES_MAX as usize);
        let before = take.pcm().len();
        assert!(matches!(take.push(&block), Err(TakeError::CapExceeded)));
        assert_eq!(take.pcm().len(), before);
        assert!(matches!(
            take.push(&vec![0u8; STREAM_CHUNK_BYTES_MAX + 4]),
            Err(TakeError::ChunkTooLarge { .. })
        ));
        assert!(matches!(
            take.push(&[0, 1, 2, 3, 4, 5]),
            Err(TakeError::ChunkNotFrameAligned { .. })
        ));
        take.stop().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bounded.wav");
        take.save(&path).unwrap();
        let parsed = parse_wav(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(parsed.sample_rate_hz, 96_000);
        assert_eq!(parsed.data.len(), before);
        assert!(parsed.data.len() > BUFFER_BYTES_MAX as usize);

        let mut native = Take::prepare(TakeRequest {
            route: RecordRoute::SendCleanGuitar,
            clock: SampleClock::native_podxt(),
            format: FrameFormat::PodS24_3Le,
            origin: InputOrigin::Synthetic,
            max_bytes: 16_384,
        })
        .unwrap();
        native.start().unwrap();
        assert!(matches!(
            native.push(&[0u8; 6]),
            Err(TakeError::ChunkRejected(
                PlanError::PeriodBelowDeclaredMinimum
            ))
        ));
        assert!(native.pcm().is_empty());
        assert!(matches!(
            native.push(&[0u8; 64]),
            Err(TakeError::ChunkNotFrameAligned {
                len: 64,
                frame_bytes: 6,
            })
        ));
        native.push(&vec![0u8; 8_190]).unwrap();
        assert_eq!(native.pcm().len(), 8_190);
        assert!(matches!(
            Take::prepare(TakeRequest {
                clock: SampleClock::Integer { hz: 44_100 },
                format: FrameFormat::PodS24_3Le,
                ..take_request_from(&native)
            }),
            Err(TakeError::FormatClockMismatch)
        ));
    }

    #[test]
    fn meters_report_integer_peaks() {
        let mut take =
            Take::prepare(integer_request(RecordRoute::SendCleanGuitar, 44_100, 256)).unwrap();
        take.start().unwrap();
        let mut frame = Vec::new();
        frame.extend_from_slice(&i16::MIN.to_le_bytes());
        frame.extend_from_slice(&100i16.to_le_bytes());
        let levels = take.push(&frame).unwrap();
        assert_eq!(levels.left_peak, 32_768);
        assert_eq!(levels.chunk_left_peak, 32_768);
        assert_eq!(levels.right_peak, 100);
        let quiet = render_pcm_s16(SyntheticPattern::Constant { left: 1, right: 1 }, 0, 1);
        let later = take.push(&quiet).unwrap();
        assert_eq!(later.left_peak, 32_768);
        assert_eq!(later.chunk_left_peak, 1);
        assert_eq!(later.chunk_right_peak, 1);
        take.note_dropout(1).unwrap();
        assert_eq!(take.levels().left_peak, 32_768);
    }

    #[test]
    fn malformed_wav_files_are_rejected() {
        assert!(matches!(parse_wav(b"nope"), Err(TakeError::Wav(_))));
        let valid = write_wav(44_100, &render_pcm_s16(SyntheticPattern::Ramp, 0, 1)).unwrap();
        let mut truncated = valid.clone();
        truncated.pop();
        assert!(parse_wav(&truncated).is_err());
        let mut mono = valid.clone();
        mono[22..24].copy_from_slice(&1u16.to_le_bytes());
        assert!(matches!(parse_wav(&mono), Err(TakeError::Wav(_))));
        let mut float_tag = valid.clone();
        float_tag[20..22].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(parse_wav(&float_tag), Err(TakeError::Wav(_))));
        let mut byte_rate = valid.clone();
        byte_rate[28..32].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(parse_wav(&byte_rate), Err(TakeError::Wav(_))));
        let mut odd_chunk = Vec::new();
        odd_chunk.extend_from_slice(&valid[..36]);
        odd_chunk.extend_from_slice(b"padc");
        odd_chunk.extend_from_slice(&1u32.to_le_bytes());
        odd_chunk.push(0x7f);
        odd_chunk.extend_from_slice(&valid[36..]);
        let odd_size = u32::try_from(odd_chunk.len() - 8).unwrap();
        odd_chunk[4..8].copy_from_slice(&odd_size.to_le_bytes());
        assert!(matches!(
            parse_wav(&odd_chunk),
            Err(TakeError::Wav("odd WAV chunk sizes are not accepted"))
        ));
        let mut huge = valid[..44].to_vec();
        let riff_size = u32::try_from(huge.len() - 8).unwrap();
        huge[4..8].copy_from_slice(&riff_size.to_le_bytes());
        let claim = u32::try_from(TAKE_BYTES_CEILING + 1).unwrap();
        huge[40..44].copy_from_slice(&claim.to_le_bytes());
        assert!(matches!(parse_wav(&huge), Err(TakeError::CapExceeded)));
    }

    #[test]
    fn panel_stays_idle_until_start_and_names_the_offline_take() {
        let mut panel = Panel::new();
        assert_eq!(panel.phase(), Phase::Idle);
        assert_eq!(panel.phase_label(), PHASE_IDLE);
        assert!(!panel.device_opened());
        assert!(!panel.live_monitor_changed());
        assert!(!panel.live_route_changed());
        assert!(panel.start().is_err());
        assert_eq!(panel.phase(), Phase::Idle);
        panel.select_route(RecordRoute::SendReampPlayback).unwrap();
        panel.select_route(RecordRoute::SendCleanGuitar).unwrap();
        panel.select_clock(ClockSelection::Integer(44_100)).unwrap();
        panel.select_input(InputSelection::Synthetic).unwrap();
        assert!(!panel.live_monitor_changed());
        panel.start().unwrap();
        assert_eq!(panel.phase_label(), PHASE_RECORDING);
        assert!(panel.take.as_ref().unwrap().pcm().is_empty());
        panel.feed().unwrap();
        assert_eq!(panel.levels().left_peak, 1_000);
        assert_eq!(panel.levels().right_peak, 2_000);
        assert_eq!(panel.levels().chunk_right_peak, 2_000);
        assert!(panel.select_clock(ClockSelection::NativePodxt).is_err());
        let directory = tempfile::tempdir().unwrap();
        panel.cancel().unwrap();
        assert_eq!(panel.phase(), Phase::Idle);
        assert_eq!(panel.message(), PHASE_CANCELLED);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);

        panel.start().unwrap();
        panel.feed().unwrap();
        panel.stop().unwrap();
        let path = directory.path().join("panel.wav");
        panel.save(&path).unwrap();
        let wav = std::fs::read(&path).unwrap();
        assert_eq!(header_rate(&wav), 44_100);
        let sidecar = std::fs::read_to_string(sidecar_path(&path).unwrap()).unwrap();
        assert!(sidecar.contains(OFFLINE_BANNER) || sidecar.contains("offline-test-recording"));
        assert!(sidecar.contains("\"device_recording\": false"));
        assert!(matches!(panel.save(&path), Err(TakeError::AlreadyExists)));

        panel.cancel().unwrap();
        panel.select_clock(ClockSelection::NativePodxt).unwrap();
        panel.select_input(InputSelection::Synthetic).unwrap();
        panel.start().unwrap();
        panel.feed().unwrap();
        panel.stop().unwrap();
        let native = directory.path().join("native.wav");
        let error = panel.save(&native).unwrap_err();
        assert!(error.to_string().contains("78125/2"));
        assert!(!native.exists());
        assert!(
            panel
                .load_wav("nope.wav", &write_wav(44_100, &[]).unwrap())
                .is_err()
        );
    }

    fn sidecar_sha(sidecar: &str, key: &str) -> String {
        let needle = format!("\"{key}\": \"");
        let start = sidecar.find(&needle).unwrap() + needle.len();
        sidecar[start..].split('"').next().unwrap().to_string()
    }

    fn take_request_from(take: &Take) -> TakeRequest {
        TakeRequest {
            route: take.route,
            clock: take.clock,
            format: take.format,
            origin: InputOrigin::Synthetic,
            max_bytes: take.max_bytes,
        }
    }

    impl Panel {
        fn load_after_clock(
            &mut self,
            hz: u32,
            label: &str,
            bytes: &[u8],
        ) -> Result<(), TakeError> {
            self.select_clock(ClockSelection::Integer(hz))?;
            self.load_wav(label, bytes)
        }
    }
}

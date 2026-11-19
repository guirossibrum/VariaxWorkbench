#![forbid(unsafe_code)]
//! Offline PODxt Live tone files.
//!
//! The container is the shared Workbench IFF patch, not a Variax model program
//! and not a firmware resource-directory or region-17 record. `L6ToneFileIFFMap`
//! sends FORM/L6PA, FORM/L6FS, and FORM/L6AS to `L6Patch`. `L6ProductConstants.PODXT_LIVE`
//! is 196618 (`0x0003000A`). `L6Patch.UpdateMetaAmpName` reads model slot 196608
//! (`0x00030000`) for a Sharc tone. `UpdateMetaNotesForSetup` reads slot 196609
//! (`0x00030001`) only when the patch type is 2 (L6AS). Asset names are the
//! PODxt Live `GetAssetName` results in `pod_assets`. A 12-byte MINF carries
//! the signed ordinal, two raw reserved bytes, and `m_enabled`. `L6Buffer.Read8`
//! returns a Java signed byte on both mac and Windows. `L6Model` stores that
//! byte in `int m_ordinal` and sets enabled from `Read8() > 0`, so bytes
//! `0x80` through `0xFF` are negative and disabled. The file bytes stay as
//! stored. A 12-byte PARM carries an id, a value type, and raw bits. For the
//! PODxt product, Line6Edit 3.06 maps six amp selectors to shared semantic
//! PARM ids and finite 0..=1 host-scale definitions. Rust allows these edits
//! for source-listed amp models except the `No Amp` association, and only when
//! each existing 12-byte PARM has the expected id, prefix and float type. This
//! is not a device-accepted-domain or model-entitlement claim. Other bytes are
//! preserved.
//!
//! The MIDI hardware id `L6HW_ID_PODXTLIVE` (18000) is a different namespace.
//! The archived firmware version `0x03010000` is not a requirement of a tone file.

use crate::document;
use crate::pod_assets::{self, SlotRole};
use crate::preset;
use std::{ops::Range, path::Path};

/// PINF target for PODxt Live. Equal to `L6ProductConstants.PODXT_LIVE`.
pub const PODXT_LIVE_TONE_ID: u32 = 0x0003_000a;
/// Amp model slot from `L6Patch.UpdateMetaAmpName` and `kGPXTSlot_Amp`.
pub const AMP_SLOT: u32 = 0x0003_0000;
/// PODxt Live Plexi Lead 100 model from the device-specific model table.
pub const PLEXI_LEAD_100_MODEL: u32 = 0x0002_001b;
/// PODxt Live Plexi 45 model from the device-specific model table.
pub const PLEXI_45_MODEL: u32 = 0x0002_001a;
/// Semantic Amp Drive parameter id in the PODxt Sharc parameter map.
pub const AMP_DRIVE_PARAM: u32 = 0x0010_0003;
/// Semantic Amp Bass parameter id in the PODxt Sharc parameter map.
pub const AMP_BASS_PARAM: u32 = 0x0010_0000;
/// Semantic Amp Mid parameter id in the PODxt Sharc parameter map.
pub const AMP_MID_PARAM: u32 = 0x0010_0001;
/// Semantic Amp Treble parameter id in the PODxt Sharc parameter map.
pub const AMP_TREBLE_PARAM: u32 = 0x0010_0002;
/// Semantic Amp Presence parameter id in the PODxt Sharc parameter map.
pub const AMP_PRESENCE_PARAM: u32 = 0x0010_0004;
/// Semantic Amp Channel Volume parameter id in the PODxt Sharc parameter map.
pub const AMP_CHANNEL_VOLUME_PARAM: u32 = 0x0010_0005;
/// Models listed by PODxt Live `GetAmpModelList` that represent an amp.
/// The provider's `0x00020000` No Amp association is intentionally excluded.
pub const AMP_CONTROL_MODEL_IDS: [u32; 106] = [
    0x0002_002e,
    0x0002_0001,
    0x0002_0005,
    0x0002_0006,
    0x0002_0007,
    0x0002_0008,
    0x0002_000a,
    0x0002_000c,
    0x0002_000f,
    0x0002_0010,
    0x0002_0011,
    0x0002_0012,
    0x0002_0013,
    0x0002_0014,
    0x0002_0015,
    0x0002_0018,
    0x0002_0019,
    0x0002_001a,
    0x0002_001b,
    0x0002_001d,
    0x0002_001e,
    0x0002_0020,
    0x0002_0021,
    0x0002_0022,
    0x0002_0023,
    0x0002_0024,
    0x0002_0026,
    0x0002_0027,
    0x0002_0029,
    0x0002_002a,
    0x0002_002c,
    0x0002_002d,
    0x0002_003d,
    0x0002_003c,
    0x0002_003b,
    0x0002_0032,
    0x0002_002f,
    0x0002_003e,
    0x0002_0040,
    0x0002_0041,
    0x0002_0031,
    0x0002_0045,
    0x0002_0046,
    0x0002_0047,
    0x0002_0048,
    0x0002_0025,
    0x0002_0049,
    0x0002_003a,
    0x0002_0039,
    0x0002_0038,
    0x0002_000d,
    0x0002_0037,
    0x0002_0035,
    0x0002_0009,
    0x0002_0030,
    0x0002_003f,
    0x0002_0042,
    0x0002_0043,
    0x0002_0044,
    0x0002_0016,
    0x0002_0017,
    0x0002_001c,
    0x0002_001f,
    0x0002_0028,
    0x0002_002b,
    0x0002_0004,
    0x0002_0036,
    0x0002_000b,
    0x0002_0002,
    0x0002_0033,
    0x0002_0034,
    0x0002_0003,
    0x0003_0001,
    0x0003_0002,
    0x0003_0003,
    0x0003_0004,
    0x0003_0005,
    0x0003_0006,
    0x0003_0007,
    0x0003_0008,
    0x0003_0009,
    0x0003_000a,
    0x0003_000b,
    0x0003_000c,
    0x0003_000d,
    0x0003_000e,
    0x0003_000f,
    0x0003_0010,
    0x0003_0011,
    0x0003_0012,
    0x0003_0013,
    0x0003_0014,
    0x0003_0015,
    0x0003_0016,
    0x0003_0017,
    0x0003_0018,
    0x0003_0019,
    0x0003_001a,
    0x0003_001b,
    0x0003_001c,
    0x0002_004a,
    0x0002_004b,
    0x0002_004c,
    0x0002_004d,
    0x0002_004e,
    0x0002_004f,
];
/// PODxt Live stomp model slot from `GetStompModel`.
pub const STOMP_SLOT: u32 = 0x0002_0003;
/// PODxt Live modulation model slot from `GetModModel`.
pub const MOD_SLOT: u32 = 0x0002_0004;
/// PODxt Live Mod models listed by `GetModModelList`, in provider order.
pub const MOD_CONTROL_MODEL_IDS: [u32; 24] = [
    0x0203_0004,
    0x0203_0005,
    0x0203_0006,
    0x0203_0007,
    0x0203_0008,
    0x0203_0009,
    0x0203_000a,
    0x0203_000b,
    0x0203_000c,
    0x0203_000d,
    0x0203_000e,
    0x0203_0015,
    0x0203_0012,
    0x0203_001e,
    0x0203_0014,
    0x0203_001f,
    0x0203_0013,
    0x0203_0011,
    0x0203_001a,
    0x0203_001b,
    0x0203_0017,
    0x0203_0018,
    0x0203_0019,
    0x0203_001c,
];
/// PODxt Live Reverb model slot and source-listed provider profiles.
pub const REVERB_SLOT: u32 = 0x0002_0006;
pub const REVERB_CONTROL_MODEL_IDS: [u32; 15] = [
    0x0204_0002,
    0x0204_0003,
    0x0204_0004,
    0x0204_0005,
    0x0204_0006,
    0x0204_0007,
    0x0204_0008,
    0x0204_0009,
    0x0204_000a,
    0x0204_000b,
    0x0204_000c,
    0x0204_000d,
    0x0204_000e,
    0x0204_000f,
    0x0204_0010,
];
/// PODxt Live Delay model slot and source-listed provider profiles.
pub const DELAY_SLOT: u32 = 0x0002_0005;
pub const DELAY_CONTROL_MODEL_IDS: [u32; 14] = [
    0x0202_0002,
    0x0202_0003,
    0x0202_0004,
    0x0202_0005,
    0x0202_0006,
    0x0202_0007,
    0x0202_0008,
    0x0202_0009,
    0x0202_000a,
    0x0202_000c,
    0x0202_000b,
    0x0202_000d,
    0x0202_000e,
    0x0202_000f,
];
/// Facial Fuzz, the first model with the source-mapped Stomp control layout.
pub const STOMP_DRIVE_MODEL_ID: u32 = 0x0205_0000;
/// PODxt Live Stomp models with source-mapped percent control layouts.
pub const STOMP_CONTROL_MODEL_IDS: [u32; 31] = [
    0x0205_0000, // Facial Fuzz
    0x0205_0002, // Fuzz Pi
    0x0205_0003, // Screamer
    0x0205_0005, // Classic Dist
    0x0205_0004, // Octave Fuzz
    0x0200_0002,
    0x0200_0004,
    0x0200_0005,
    0x0200_0007,
    0x020a_0000,
    0x0205_0006, // FX-Killer Z
    0x0205_0001, // FX-Tube Drive
    0x0200_0006,
    0x0205_0007, // FX-Boost + EQ
    0x0200_0003,
    0x020a_000a,
    0x020a_000b,
    0x020a_000c,
    0x020a_0008,
    0x020a_0005,
    0x020a_0006,
    0x020a_0007,
    0x020a_0003,
    0x020a_0004,
    0x020a_0009,
    0x020a_0001,
    0x020a_0002,
    0x0205_0008, // Bass Overdrive
    0x0205_0009, // Bronze Master
    0x020a_000d,
    0x020a_000e,
];
/// Stomp Drive selector passed through `GetFXParamDef`.
pub const STOMP_DRIVE_SELECTOR: u32 = 0x0003_0001;
/// Raw semantic PARM identifier selected for Stomp Drive.
pub const STOMP_DRIVE_PARAM: u32 = 0x0010_0001;
pub const STOMP_GAIN_SELECTOR: u32 = 0x0003_0002;
pub const STOMP_GAIN_PARAM: u32 = 0x0010_0002;
pub const STOMP_TONE_SELECTOR: u32 = 0x0003_0003;
pub const STOMP_TONE_PARAM: u32 = 0x0010_0003;
/// Host provider minimum from `ParseVersionString("3.00")`; not a file-loader rule.
pub const PODXT_LIVE_HOST_MIN_VERSION: u32 = 0x0300_0000;
/// Cab model slot from `L6Patch.UpdateMetaNotesForSetup` and `kGPXTSlot_Cab`.
/// Only an L6AS tone exposes it as the cab model.
pub const CAB_SLOT: u32 = 0x0003_0001;
/// Application bound for the offline librarian. Not a recovered Line 6 limit.
pub const TONE_BYTES_CEILING: u64 = 1_048_576;
pub const BUNDLE_BYTES_CEILING: u64 = 16_777_216;

pub const OFFLINE_BANNER: &str =
    "Offline PODxt Live tone file. This does not edit a connected POD.";
pub const NOT_GUITAR: &str =
    "This is not the Variax guitar editor. Guitar model slots are not applied.";
pub const NOT_SENT: &str = "Nothing is sent to a device. No preset is switched.";
pub const PAYLOADS_NOT_DECODED: &str = "Known asset names and 12-byte parameter records are shown. Six shared amp controls on 106 models, 86 Stomp controls on 31 layouts, 52 Mod controls on 24 layouts, 37 Delay controls on 14 layouts, and 42 Reverb controls on 15 layouts have host-scale editors; other parameter formats remain raw. Unknown ids stay raw.";
pub const AMP_CONTROL_SCALE_LABEL: &str = "Line6Edit host scale 0–1 (not device-validated)";
pub const STOMP_CONTROL_SCALE_LABEL: &str = "Line6Edit Stomp host scale 0–1 (not device-validated)";
pub const MOD_CONTROL_SCALE_LABEL: &str = "Line6Edit Mod host scale 0–1 (not device-validated)";
pub const REVERB_CONTROL_SCALE_LABEL: &str =
    "Line6Edit Reverb host scale 0–1 (not device-validated)";
pub const DELAY_CONTROL_SCALE_LABEL: &str = "Line6Edit Delay host scale 0–1 (not device-validated)";

/// PODxt Live amp controls with an admitted Line6Edit host scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmpControl {
    Drive,
    Bass,
    Mid,
    Treble,
    Presence,
    ChannelVolume,
}

impl AmpControl {
    /// The source-backed controls in UI display order.
    pub const ALL: [Self; 6] = [
        Self::Drive,
        Self::Bass,
        Self::Mid,
        Self::Treble,
        Self::Presence,
        Self::ChannelVolume,
    ];

    pub const fn param_id(self) -> u32 {
        match self {
            Self::Drive => AMP_DRIVE_PARAM,
            Self::Bass => AMP_BASS_PARAM,
            Self::Mid => AMP_MID_PARAM,
            Self::Treble => AMP_TREBLE_PARAM,
            Self::Presence => AMP_PRESENCE_PARAM,
            Self::ChannelVolume => AMP_CHANNEL_VOLUME_PARAM,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "Drive",
            Self::Bass => "Bass",
            Self::Mid => "Mid",
            Self::Treble => "Treble",
            Self::Presence => "Presence",
            Self::ChannelVolume => "Channel Volume",
        }
    }
}

/// Source-backed percent controls on selected PODxt Live Stomp layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompControl {
    Drive,
    Gain,
    Tone,
    Bass,
    Treble,
    Contour,
    Mid,
    MidFrequency,
    Blend,
    Sustain,
    Level,
    Sensitivity,
    Amount,
    Ramp,
    Depth,
    Q,
    Filter,
    Decay,
    Mix,
    Attack,
    S1Ogt,
    S2Ogt,
    Position,
}

impl StompControl {
    pub const ALL: [Self; 23] = [
        Self::Drive,
        Self::Gain,
        Self::Tone,
        Self::Bass,
        Self::Treble,
        Self::Contour,
        Self::Mid,
        Self::MidFrequency,
        Self::Blend,
        Self::Sustain,
        Self::Level,
        Self::Sensitivity,
        Self::Amount,
        Self::Ramp,
        Self::Depth,
        Self::Q,
        Self::Filter,
        Self::Decay,
        Self::Mix,
        Self::Attack,
        Self::S1Ogt,
        Self::S2Ogt,
        Self::Position,
    ];

    /// Selector position established by `GetFXParamDef` for a particular model.
    pub const fn selector_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0003_0000 + position),
            None => None,
        }
    }

    /// Semantic raw PARM id established by `GetFXParam` for a model selector.
    pub const fn param_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0010_0000 + position),
            None => None,
        }
    }

    const fn position(self, model_id: u32) -> Option<u32> {
        match model_id {
            0x0205_0000 | 0x0205_0002 | 0x0205_0003 | 0x0205_0004 | 0x0205_0005 => match self {
                Self::Drive => Some(1),
                Self::Gain => Some(2),
                Self::Tone => Some(3),
                _ => None,
            },
            0x0205_0001 => match self {
                Self::Drive => Some(1),
                Self::Treble => Some(2),
                Self::Gain => Some(3),
                Self::Bass => Some(4),
                _ => None,
            },
            0x0205_0006 => match self {
                Self::Drive => Some(1),
                Self::Contour => Some(2),
                Self::Gain => Some(3),
                Self::Mid => Some(4),
                Self::MidFrequency => Some(5),
                _ => None,
            },
            0x0205_0007 => match self {
                Self::Gain => Some(1),
                Self::Bass => Some(2),
                Self::Treble => Some(3),
                Self::Mid => Some(4),
                Self::MidFrequency => Some(5),
                _ => None,
            },
            0x0205_0008 => match self {
                Self::Bass => Some(1),
                Self::Treble => Some(2),
                Self::Drive => Some(3),
                Self::Gain => Some(4),
                _ => None,
            },
            0x0205_0009 => match self {
                Self::Drive => Some(1),
                Self::Tone => Some(2),
                Self::Blend => Some(4),
                _ => None,
            },
            0x0200_0002 | 0x0200_0004 => match self {
                Self::Sustain => Some(1),
                Self::Level => Some(2),
                _ => None,
            },
            0x0200_0003 => match self {
                Self::Level => Some(1),
                Self::Sustain => Some(2),
                _ => None,
            },
            0x0200_0005 => match self {
                Self::Sensitivity => Some(1),
                Self::Level => Some(2),
                _ => None,
            },
            0x0200_0006 => match self {
                Self::Amount => Some(1),
                Self::Level => Some(2),
                _ => None,
            },
            0x0200_0007 => match self {
                Self::Ramp => Some(1),
                Self::Depth => Some(2),
                _ => None,
            },
            0x020a_0000 => match self {
                Self::Sensitivity => Some(1),
                Self::Q => Some(2),
                _ => None,
            },
            0x020a_0001 | 0x020a_0003 | 0x020a_0004 | 0x020a_0005 | 0x020a_0006 => match self {
                Self::Filter => Some(2),
                Self::Decay => Some(3),
                Self::Mix => Some(4),
                _ => None,
            },
            0x020a_0002 | 0x020a_0009 => match self {
                Self::Filter => Some(2),
                Self::Attack => Some(3),
                Self::Mix => Some(4),
                _ => None,
            },
            0x020a_0007 | 0x020a_000c => match self {
                Self::Mix => Some(4),
                _ => None,
            },
            0x020a_0008 | 0x020a_000d => match self {
                Self::S1Ogt => Some(1),
                Self::S2Ogt => Some(2),
                Self::Mix => Some(4),
                _ => None,
            },
            0x020a_000a => match self {
                Self::Sensitivity => Some(2),
                Self::Q => Some(3),
                _ => None,
            },
            0x020a_000b => match self {
                Self::Decay => Some(1),
                Self::Sensitivity => Some(2),
                Self::Q => Some(3),
                _ => None,
            },
            0x020a_000e => match self {
                Self::Position => Some(1),
                Self::Mix => Some(4),
                _ => None,
            },
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "Drive",
            Self::Gain => "Gain",
            Self::Tone => "Tone",
            Self::Bass => "Bass",
            Self::Treble => "Treble",
            Self::Contour => "Contour",
            Self::Mid => "Mid",
            Self::MidFrequency => "Mid Frequency",
            Self::Blend => "Blend",
            Self::Sustain => "Sustain",
            Self::Level => "Level",
            Self::Sensitivity => "Sensitivity",
            Self::Amount => "Amount",
            Self::Ramp => "Ramp",
            Self::Depth => "Depth",
            Self::Q => "Q",
            Self::Filter => "Filter",
            Self::Decay => "Decay",
            Self::Mix => "Mix",
            Self::Attack => "Attack",
            Self::S1Ogt => "-1OCTG",
            Self::S2Ogt => "-2OCTG",
            Self::Position => "Position",
        }
    }
}

/// Source-mapped finite percent controls from PODxt Mod layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModControl {
    Depth,
    Bass,
    Treble,
    Feedback,
    Manual,
    Predelay,
    Wave,
    Tone,
    Q,
    Frequency,
    Flutter,
    Distortion,
}

impl ModControl {
    pub const ALL: [Self; 12] = [
        Self::Depth,
        Self::Bass,
        Self::Treble,
        Self::Feedback,
        Self::Manual,
        Self::Predelay,
        Self::Wave,
        Self::Tone,
        Self::Q,
        Self::Frequency,
        Self::Flutter,
        Self::Distortion,
    ];

    pub const fn selector_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0004_0002 + position),
            None => None,
        }
    }

    pub const fn param_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0010_0000 + position),
            None => None,
        }
    }

    const fn position(self, model_id: u32) -> Option<u32> {
        match model_id {
            0x0203_0004 | 0x0203_0005 | 0x0203_0015 | 0x0203_0014 | 0x0203_0011 => match self {
                Self::Depth => Some(1),
                Self::Bass => Some(2),
                Self::Treble => Some(3),
                _ => None,
            },
            0x0203_0006 | 0x0203_0009 => match self {
                Self::Depth => Some(1),
                _ => None,
            },
            0x0203_0007 => match self {
                Self::Depth => Some(1),
                Self::Feedback => Some(2),
                Self::Manual => Some(3),
                _ => None,
            },
            0x0203_0008 => match self {
                Self::Feedback => Some(1),
                _ => None,
            },
            0x0203_000a | 0x0203_000b | 0x0203_000e => match self {
                Self::Wave => Some(1),
                _ => None,
            },
            0x0203_000c | 0x0203_000d => match self {
                Self::Tone => Some(2),
                _ => None,
            },
            0x0203_0012 | 0x0203_001e | 0x0203_001f | 0x0203_0013 => match self {
                Self::Depth => Some(1),
                Self::Predelay => Some(2),
                Self::Feedback => Some(3),
                _ => None,
            },
            0x0203_001a | 0x0203_0018 | 0x0203_001c => match self {
                Self::Depth => Some(1),
                Self::Q => Some(2),
                _ => None,
            },
            0x0203_001b => match self {
                Self::Depth => Some(1),
                Self::Q => Some(2),
                Self::Frequency => Some(3),
                _ => None,
            },
            0x0203_0017 => match self {
                Self::Feedback => Some(1),
                Self::Depth => Some(2),
                _ => None,
            },
            0x0203_0019 => match self {
                Self::Feedback => Some(1),
                Self::Flutter => Some(2),
                Self::Distortion => Some(3),
                _ => None,
            },
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Depth => "Depth",
            Self::Bass => "Bass",
            Self::Treble => "Treble",
            Self::Feedback => "Feedback",
            Self::Manual => "Manual",
            Self::Predelay => "Pre-delay",
            Self::Wave => "Wave",
            Self::Tone => "Tone",
            Self::Q => "Q",
            Self::Frequency => "Frequency",
            Self::Flutter => "Flutter",
            Self::Distortion => "Distortion",
        }
    }
}

/// Source-mapped finite percent controls from PODxt Reverb layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReverbControl {
    Dwell,
    Tone,
    Predelay,
    Decay,
}

impl ReverbControl {
    pub const ALL: [Self; 4] = [Self::Dwell, Self::Tone, Self::Predelay, Self::Decay];

    pub const fn selector_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(match position {
                0 => 0x0008_0002,
                1 => 0x0008_0001,
                2 => 0x0008_0003,
                _ => return None,
            }),
            None => None,
        }
    }

    pub const fn param_id(self, model_id: u32) -> Option<u32> {
        match self.selector_id(model_id) {
            Some(0x0008_0001) => Some(0x0010_0000),
            Some(0x0008_0002) => Some(0x0010_0001),
            Some(0x0008_0003) => Some(0x0010_0002),
            _ => None,
        }
    }

    const fn position(self, model_id: u32) -> Option<u32> {
        match (model_id, self) {
            (0x0204_0002..=0x0204_0004, Self::Dwell) => Some(1),
            (0x0204_0002..=0x0204_0004, Self::Tone) => Some(2),
            (0x0204_0005..=0x0204_0010, Self::Predelay) => Some(0),
            (0x0204_0005..=0x0204_0010, Self::Decay) => Some(1),
            (0x0204_0005..=0x0204_0010, Self::Tone) => Some(2),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dwell => "Dwell",
            Self::Tone => "Tone",
            Self::Predelay => "Pre-delay",
            Self::Decay => "Decay",
        }
    }
}

/// Source-mapped finite percent controls from PODxt Delay layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelayControl {
    Feedback,
    ModSpeed,
    Depth,
    Speed,
    Bass,
    Treble,
    Flutter,
    Drive,
    Offset,
    FeedbackLeft,
    FeedbackRight,
    Spread,
    Tone,
}

impl DelayControl {
    pub const ALL: [Self; 13] = [
        Self::Feedback,
        Self::ModSpeed,
        Self::Depth,
        Self::Speed,
        Self::Bass,
        Self::Treble,
        Self::Flutter,
        Self::Drive,
        Self::Offset,
        Self::FeedbackLeft,
        Self::FeedbackRight,
        Self::Spread,
        Self::Tone,
    ];

    pub const fn selector_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0005_0002 + position),
            None => None,
        }
    }

    pub const fn param_id(self, model_id: u32) -> Option<u32> {
        match self.position(model_id) {
            Some(position) => Some(0x0010_0000 + position),
            None => None,
        }
    }

    const fn position(self, model_id: u32) -> Option<u32> {
        match model_id {
            0x0202_0002 | 0x0202_0007 | 0x0202_000b => match self {
                Self::Feedback => Some(1),
                Self::Bass => Some(2),
                Self::Treble => Some(3),
                _ => None,
            },
            0x0202_0003 | 0x0202_000e => match self {
                Self::Feedback => Some(1),
                Self::ModSpeed => Some(2),
                Self::Depth => Some(3),
                _ => None,
            },
            0x0202_0004 => match self {
                Self::Feedback => Some(1),
                Self::Flutter => Some(2),
                Self::Drive => Some(3),
                _ => None,
            },
            0x0202_0005 | 0x0202_000c => match self {
                Self::Feedback => Some(1),
                Self::Flutter => Some(3),
                _ => None,
            },
            0x0202_0006 | 0x0202_000f => match self {
                Self::Feedback => Some(1),
                Self::Speed => Some(2),
                Self::Depth => Some(3),
                _ => None,
            },
            0x0202_0008 => match self {
                Self::Offset => Some(1),
                Self::FeedbackLeft => Some(2),
                Self::FeedbackRight => Some(3),
                _ => None,
            },
            0x0202_0009 => match self {
                Self::Feedback => Some(1),
                Self::Offset => Some(2),
                Self::Spread => Some(3),
                _ => None,
            },
            0x0202_000a => match self {
                Self::Feedback => Some(1),
                _ => None,
            },
            0x0202_000d => match self {
                Self::Feedback => Some(1),
                Self::Tone => Some(2),
                _ => None,
            },
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Feedback => "Feedback",
            Self::ModSpeed => "Mod Speed",
            Self::Depth => "Depth",
            Self::Speed => "Speed",
            Self::Bass => "Bass",
            Self::Treble => "Treble",
            Self::Flutter => "Flutter",
            Self::Drive => "Drive",
            Self::Offset => "Offset",
            Self::FeedbackLeft => "Feedback Left",
            Self::FeedbackRight => "Feedback Right",
            Self::Spread => "Spread",
            Self::Tone => "Tone",
        }
    }
}

/// Finite source-backed association value read from an existing Delay PARM.
/// Unknown float words stay visible and unchanged until the user selects a
/// recognized option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayAssociationValue<T> {
    Known(T),
    Unknown { raw_bits: u32 },
}

/// Source-listed four-head patterns for Delay model `0x02020005`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelayHeadsSelection {
    TwelveMinusMinus,
    OneThree,
    OneMinusFour,
    MinusTwoThreeMinus,
    OneTwoThreeMinus,
    OneTwoMinusFour,
    OneMinusThreeFour,
    MinusTwoThreeFour,
    OneTwoThreeFour,
}

impl DelayHeadsSelection {
    pub const ALL: [Self; 9] = [
        Self::TwelveMinusMinus,
        Self::OneThree,
        Self::OneMinusFour,
        Self::MinusTwoThreeMinus,
        Self::OneTwoThreeMinus,
        Self::OneTwoMinusFour,
        Self::OneMinusThreeFour,
        Self::MinusTwoThreeFour,
        Self::OneTwoThreeFour,
    ];

    const fn controller_value(self) -> u8 {
        match self {
            Self::TwelveMinusMinus => 0,
            Self::OneThree => 16,
            Self::OneMinusFour => 32,
            Self::MinusTwoThreeMinus => 48,
            Self::OneTwoThreeMinus => 64,
            Self::OneTwoMinusFour => 80,
            Self::OneMinusThreeFour => 96,
            Self::MinusTwoThreeFour => 112,
            Self::OneTwoThreeFour => 127,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::TwelveMinusMinus => "12--",
            Self::OneThree => "1-3-",
            Self::OneMinusFour => "1--4",
            Self::MinusTwoThreeMinus => "-23-",
            Self::OneTwoThreeMinus => "123-",
            Self::OneTwoMinusFour => "12-4",
            Self::OneMinusThreeFour => "1-34",
            Self::MinusTwoThreeFour => "-234",
            Self::OneTwoThreeFour => "1234",
        }
    }
}

/// Source-listed bit counts for Delay model `0x0202000D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelayBitsSelection {
    Twelve,
    Eleven,
    Ten,
    Nine,
    Eight,
    Seven,
    Six,
    Five,
    Four,
}

impl DelayBitsSelection {
    pub const ALL: [Self; 9] = [
        Self::Twelve,
        Self::Eleven,
        Self::Ten,
        Self::Nine,
        Self::Eight,
        Self::Seven,
        Self::Six,
        Self::Five,
        Self::Four,
    ];

    const fn controller_value(self) -> u8 {
        match self {
            Self::Twelve => 0,
            Self::Eleven => 16,
            Self::Ten => 32,
            Self::Nine => 48,
            Self::Eight => 64,
            Self::Seven => 80,
            Self::Six => 96,
            Self::Five => 112,
            Self::Four => 127,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Twelve => "12",
            Self::Eleven => "11",
            Self::Ten => "10",
            Self::Nine => "9",
            Self::Eight => "8",
            Self::Seven => "7",
            Self::Six => "6",
            Self::Five => "5",
            Self::Four => "4",
        }
    }
}

fn association_float_bits(controller_value: u8) -> u32 {
    let adjusted = f32::from(controller_value) - 0.5_f32;
    let normalized = adjusted / 127.0_f32;
    normalized.to_bits()
}

impl DelayHeadsSelection {
    fn float_bits(self) -> u32 {
        association_float_bits(self.controller_value())
    }

    fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

impl DelayBitsSelection {
    fn float_bits(self) -> u32 {
        association_float_bits(self.controller_value())
    }

    fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

const VARIAX_FAMILY_MASK: u32 = 0xffff_0000;
const VARIAX_FAMILY_HIGH: u32 = 0x0007_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    /// `L6Patch` type 0.
    L6pa,
    /// `L6Patch` type 1.
    L6fs,
    /// `L6Patch` type 2. The only container that exposes the cab slot.
    L6as,
}

impl ContainerKind {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::L6pa => "L6PA",
            Self::L6fs => "L6FS",
            Self::L6as => "L6AS",
        }
    }

    pub const fn exposes_cab(self) -> bool {
        matches!(self, Self::L6as)
    }

    fn from_bytes(tag: &[u8]) -> Option<Self> {
        match tag {
            b"L6PA" => Some(Self::L6pa),
            b"L6FS" => Some(Self::L6fs),
            b"L6AS" => Some(Self::L6as),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToneError {
    Format { detail: String },
    WrongTarget { device_id: u32 },
}

impl ToneError {
    pub fn format(detail: impl Into<String>) -> Self {
        Self::Format {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Format { detail } => detail.clone(),
            Self::WrongTarget { device_id } => {
                format!("PINF target {device_id:08x} is not PODxt Live {PODXT_LIVE_TONE_ID:08x}")
            }
        }
    }

    pub(crate) fn is_variax_family(device_id: u32) -> bool {
        device_id & VARIAX_FAMILY_MASK == VARIAX_FAMILY_HIGH
    }
}

/// Value stored in a 12-byte PARM record. The bits are not a knob position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    /// `L6ModelParam` value type 0. The bits are the Java `int`.
    Int { bits: u32 },
    /// `L6ModelParam` value type 1. The bits are an IEEE-754 `float`.
    Float { bits: u32 },
    /// Any other type word. Both words stay raw.
    Other { type_word: u32, bits: u32 },
}

/// One PARM chunk. Only the 12-byte `L6ModelParam` record is structured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelParameter {
    Recorded {
        /// High byte of the stored first word. A write uses `0x3F`. A read keeps any byte.
        prefix: u8,
        /// Low 24 bits of the first word.
        id: u32,
        value: ParameterValue,
    },
    /// A PARM chunk whose length is not 12. Its bytes stay in the tone file.
    Opaque { len: usize },
}

/// One model slot copied from MINF. Bytes outside the decoded fields stay in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSlot {
    pub slot: u32,
    pub model_id: u32,
    /// MINF byte 8 as the Java signed byte stored in `int m_ordinal`.
    /// `0x80` is -128 and `0xFF` is -1. The file byte is not rewritten.
    pub ordinal: Option<i8>,
    /// MINF bytes 9 and 10 when the chunk is at least 12 bytes. Not rewritten.
    pub reserved: Option<[u8; 2]>,
    /// MINF byte 11 as `Read8() > 0`. A byte from `0x80` through `0xFF` is not
    /// enabled. This is `m_enabled`, not a bypass flag, and it is not rewritten.
    pub enabled: Option<bool>,
    pub parameters: Vec<ModelParameter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodTone {
    bytes: Vec<u8>,
    name_offset: usize,
    name: String,
    device_id: u32,
    device_version: u32,
    container: ContainerKind,
    models: Vec<ModelSlot>,
    parameter_value_offsets: Vec<Vec<Option<usize>>>,
}

impl ParameterValue {
    fn label(self) -> String {
        match self {
            Self::Int { bits } => format!("int {bits:08x}"),
            Self::Float { bits } => format!("float {bits:08x}"),
            Self::Other { type_word, bits } => format!("type {type_word:08x} bits {bits:08x}"),
        }
    }
}

fn read_parameters(
    bytes: &[u8],
    fields: &[preset::Chunk],
) -> (Vec<ModelParameter>, Vec<Option<usize>>) {
    let parameters = fields
        .iter()
        .filter(|chunk| chunk.id == *b"PARM")
        .map(|chunk| {
            if chunk.data.len() != 12 {
                return ModelParameter::Opaque {
                    len: chunk.data.len(),
                };
            }
            let word0 = preset::word(bytes, chunk.data.start);
            let type_word = preset::word(bytes, chunk.data.start + 4);
            let bits = preset::word(bytes, chunk.data.start + 8);
            let value = match type_word {
                0 => ParameterValue::Int { bits },
                1 => ParameterValue::Float { bits },
                other => ParameterValue::Other {
                    type_word: other,
                    bits,
                },
            };
            ModelParameter::Recorded {
                prefix: (word0 >> 24) as u8,
                id: word0 & 0x00ff_ffff,
                value,
            }
        })
        .collect();
    let offsets = fields
        .iter()
        .filter(|chunk| chunk.id == *b"PARM")
        .map(|chunk| (chunk.data.len() == 12).then_some(chunk.data.start + 8))
        .collect();
    (parameters, offsets)
}

impl PodTone {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() as u64 > TONE_BYTES_CEILING {
            return Err(ToneError::format(
                "tone file exceeds the 1 MiB offline librarian ceiling",
            ));
        }
        if bytes.len() < 12 || &bytes[..4] != b"FORM" {
            return Err(ToneError::format(
                "expected an individual FORM tone; banks and collections are not POD presets",
            ));
        }
        let Some(container) = ContainerKind::from_bytes(&bytes[8..12]) else {
            return Err(ToneError::format(
                "expected FORM/L6PA, FORM/L6FS, or FORM/L6AS",
            ));
        };
        if preset::word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err(ToneError::format("FORM length does not match the file"));
        }
        let children = preset::chunks(&bytes, 12..bytes.len()).map_err(ToneError::format)?;
        let info = preset::unique(&children, b"PINF").map_err(ToneError::format)?;
        if info.data.len() < 76 {
            return Err(ToneError::format("truncated PINF chunk"));
        }
        let start = info.data.start;
        if preset::word(&bytes, start) != 1 {
            return Err(ToneError::format(
                "unsupported PINF version; the shared layout uses version word 1",
            ));
        }
        let device_id = preset::word(&bytes, start + 4);
        if device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::WrongTarget { device_id });
        }
        let name_offset = start + 8;
        let name = utf16_name(&bytes[name_offset..name_offset + 64])?;
        let device_version = preset::word(&bytes, start + 72);
        let patch_containers: Vec<_> = children
            .iter()
            .filter(|chunk| preset::container(&bytes, chunk, b"PATC"))
            .collect();
        if patch_containers.len() != 1 {
            return Err(ToneError::format("expected exactly one PATC container"));
        }
        let patch = patch_containers[0];
        let model_chunks = preset::chunks(&bytes, patch.data.start + 4..patch.data.end)
            .map_err(ToneError::format)?;
        let mut models = Vec::new();
        let mut parameter_value_offsets = Vec::new();
        for entry in model_chunks
            .iter()
            .filter(|chunk| preset::container(&bytes, chunk, b"MODL"))
        {
            let fields = preset::chunks(&bytes, entry.data.start + 4..entry.data.end)
                .map_err(ToneError::format)?;
            let minf = preset::unique(&fields, b"MINF").map_err(ToneError::format)?;
            if minf.data.len() < 8 {
                return Err(ToneError::format("truncated MINF chunk"));
            }
            let slot = preset::word(&bytes, minf.data.start + 4);
            if models.iter().any(|model: &ModelSlot| model.slot == slot) {
                return Err(ToneError::format(format!(
                    "ambiguous duplicate model slot {slot:08x}"
                )));
            }
            let (ordinal, reserved, enabled) = if minf.data.len() >= 12 {
                let ordinal_byte = bytes[minf.data.start + 8];
                let enabled_byte = bytes[minf.data.start + 11];
                (
                    Some(ordinal_byte as i8),
                    Some([bytes[minf.data.start + 9], bytes[minf.data.start + 10]]),
                    Some((enabled_byte as i8) > 0),
                )
            } else {
                (None, None, None)
            };
            let (parameters, offsets) = read_parameters(&bytes, &fields);
            models.push(ModelSlot {
                slot,
                model_id: preset::word(&bytes, minf.data.start),
                ordinal,
                reserved,
                enabled,
                parameters,
            });
            parameter_value_offsets.push(offsets);
        }
        Ok(Self {
            bytes,
            name_offset,
            name,
            device_id,
            device_version,
            container,
            models,
            parameter_value_offsets,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    pub fn device_version(&self) -> u32 {
        self.device_version
    }

    pub fn container(&self) -> ContainerKind {
        self.container
    }

    pub fn models(&self) -> &[ModelSlot] {
        &self.models
    }

    /// Amp model id when slot `0x00030000` is present.
    pub fn amp_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == AMP_SLOT)
            .map(|model| model.model_id)
    }

    /// Stomp model id when slot `0x00020003` is present.
    pub fn stomp_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
    }

    /// Mod model id when the primary Mod slot `0x00020004` is present.
    pub fn mod_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == MOD_SLOT)
            .map(|model| model.model_id)
    }

    pub fn reverb_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == REVERB_SLOT)
            .map(|model| model.model_id)
    }

    pub fn delay_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
    }

    /// Asset name for the amp model id, when the PODxt Live table has one.
    pub fn amp_asset_name(&self) -> Option<&'static str> {
        self.amp_model_id().and_then(pod_assets::asset_name)
    }

    /// Read one admitted Line6Edit host-scale amp control value (0..=1).
    /// This is not a claim about the value accepted by physical hardware.
    pub fn amp_control_fraction(&self, control: AmpControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.amp_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Amp {}", control.label()),
        )
    }

    /// Read the admitted PODxt Live Stomp Drive host-scale value (0..=1).
    /// This is not a claim about physical-device acceptance or effect sound.
    pub fn stomp_drive_fraction(&self) -> Result<f32, ToneError> {
        self.stomp_control_fraction(StompControl::Drive)
    }

    /// Read a source-backed Stomp host-scale value (0..=1) for the admitted model.
    pub fn stomp_control_fraction(&self, control: StompControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.stomp_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Stomp {}", control.label()),
        )
    }

    /// Read one source-backed Mod percent value (host scale 0..=1).
    pub fn mod_control_fraction(&self, control: ModControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.mod_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Mod {}", control.label()),
        )
    }

    pub fn reverb_control_fraction(&self, control: ReverbControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.reverb_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Reverb {}", control.label()),
        )
    }

    pub fn delay_control_fraction(&self, control: DelayControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.delay_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Delay {}", control.label()),
        )
    }

    /// Read the finite HEADS association selection, preserving unrecognized
    /// float bits as `Unknown` without snapping them to a displayed choice.
    pub fn delay_heads_selection(
        &self,
    ) -> Result<DelayAssociationValue<DelayHeadsSelection>, ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_0005, 0x0005_0004, 0x0010_0002, "Delay Heads")?;
        let bits = self.delay_association_bits_at(model_index, parameter_index, "Delay Heads")?;
        Ok(DelayHeadsSelection::from_float_bits(bits)
            .map(DelayAssociationValue::Known)
            .unwrap_or(DelayAssociationValue::Unknown { raw_bits: bits }))
    }

    /// Read the finite BITS association selection, preserving unrecognized
    /// float bits as `Unknown` without snapping them to a displayed choice.
    pub fn delay_bits_selection(
        &self,
    ) -> Result<DelayAssociationValue<DelayBitsSelection>, ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_000d, 0x0005_0005, 0x0010_0003, "Delay Bits")?;
        let bits = self.delay_association_bits_at(model_index, parameter_index, "Delay Bits")?;
        Ok(DelayBitsSelection::from_float_bits(bits)
            .map(DelayAssociationValue::Known)
            .unwrap_or(DelayAssociationValue::Unknown { raw_bits: bits }))
    }

    /// Compatibility convenience for callers that only use Drive.
    pub fn amp_drive_fraction(&self) -> Result<f32, ToneError> {
        self.amp_control_fraction(AmpControl::Drive)
    }

    /// Change only the existing raw value word for the selected amp control.
    pub fn set_amp_control_fraction(
        &mut self,
        control: AmpControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Amp {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.amp_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Change only the existing raw Stomp Drive value word.
    pub fn set_stomp_drive_fraction(&mut self, value: f32) -> Result<(), ToneError> {
        self.set_stomp_control_fraction(StompControl::Drive, value)
    }

    /// Change only the existing raw value word for an admitted Stomp control.
    pub fn set_stomp_control_fraction(
        &mut self,
        control: StompControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Stomp {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.stomp_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Change only the existing raw value word for a source-mapped Mod percent control.
    pub fn set_mod_control_fraction(
        &mut self,
        control: ModControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Mod {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.mod_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    pub fn set_reverb_control_fraction(
        &mut self,
        control: ReverbControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Reverb {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.reverb_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    pub fn set_delay_control_fraction(
        &mut self,
        control: DelayControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Delay {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.delay_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Write only the existing HEADS PARM value word using a finite registered
    /// choice. This discrete operation intentionally does not use the percent
    /// scale setter.
    pub fn set_delay_heads_selection(
        &mut self,
        selection: DelayHeadsSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_0005, 0x0005_0004, 0x0010_0002, "Delay Heads")?;
        self.set_delay_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Delay Heads",
        )
    }

    /// Write only the existing BITS PARM value word using a finite registered
    /// choice. This discrete operation intentionally does not use the percent
    /// scale setter.
    pub fn set_delay_bits_selection(
        &mut self,
        selection: DelayBitsSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_000d, 0x0005_0005, 0x0010_0003, "Delay Bits")?;
        self.set_delay_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Delay Bits",
        )
    }

    /// Compatibility convenience for callers that only use Drive.
    pub fn set_amp_drive_fraction(&mut self, value: f32) -> Result<(), ToneError> {
        self.set_amp_control_fraction(AmpControl::Drive, value)
    }

    fn amp_control_location(&self, control: AmpControl) -> Result<(usize, usize), ToneError> {
        self.control_location(
            AMP_SLOT,
            &AMP_CONTROL_MODEL_IDS,
            control.param_id(),
            &format!("Amp {}", control.label()),
        )
    }

    fn stomp_control_location(&self, control: StompControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Stomp {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(STOMP_SLOT, &[model_id], parameter_id, &label)
    }

    fn mod_control_location(&self, control: ModControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Mod {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == MOD_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(MOD_SLOT, &[model_id], parameter_id, &label)
    }

    fn reverb_control_location(&self, control: ReverbControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Reverb {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == REVERB_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(REVERB_SLOT, &[model_id], parameter_id, &label)
    }

    fn delay_control_location(&self, control: DelayControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Delay {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(DELAY_SLOT, &[model_id], parameter_id, &label)
    }

    fn delay_association_location(
        &self,
        expected_model_id: u32,
        selector_id: u32,
        parameter_id: u32,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        if self
            .models
            .iter()
            .filter(|model| model.slot == DELAY_SLOT)
            .count()
            != 1
        {
            return Err(ToneError::format(format!(
                "{label} requires one unambiguous existing model slot"
            )));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let expected_selector = match expected_model_id {
            0x0202_0005 => 0x0005_0004,
            0x0202_000d => 0x0005_0005,
            _ => 0,
        };
        let expected_parameter = match expected_model_id {
            0x0202_0005 => 0x0010_0002,
            0x0202_000d => 0x0010_0003,
            _ => 0,
        };
        if model_id != expected_model_id
            || selector_id != expected_selector
            || parameter_id != expected_parameter
        {
            return Err(ToneError::format(format!(
                "{label} source mapping does not apply to model 0x{model_id:08x}"
            )));
        }
        self.control_location(DELAY_SLOT, &[expected_model_id], parameter_id, label)
    }

    fn delay_association_bits_at(
        &self,
        model_index: usize,
        parameter_index: usize,
        label: &str,
    ) -> Result<u32, ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        Ok(*bits)
    }

    fn set_delay_association_bits_at(
        &mut self,
        model_index: usize,
        parameter_index: usize,
        bits: u32,
        label: &str,
    ) -> Result<(), ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float {
                bits: existing_bits,
            },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        if !f32::from_bits(*existing_bits).is_finite() {
            return Err(ToneError::format(format!(
                "{label} cannot replace a non-finite stored association value"
            )));
        }
        let offset = self.parameter_value_offsets[model_index][parameter_index]
            .ok_or_else(|| ToneError::format(format!("{label} has no unique raw value word")))?;
        self.bytes[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Float { bits: value_bits },
            ..
        } = &mut self.models[model_index].parameters[parameter_index]
        {
            *value_bits = bits;
        }
        Ok(())
    }

    fn control_location(
        &self,
        slot: u32,
        model_ids: &[u32],
        parameter_id: u32,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "control editing requires a PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "tone version is below the PODxt Live host profile minimum 3.00",
            ));
        }
        let model_index = self
            .models
            .iter()
            .position(|model| model.slot == slot)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        if !model_ids.contains(&self.models[model_index].model_id) {
            return Err(ToneError::format(format!(
                "{label} host contract does not apply to this model"
            )));
        }
        if self.models[model_index]
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
        {
            return Err(ToneError::format(format!(
                "opaque PARM in {label} model makes control target ambiguous"
            )));
        }
        let matches: Vec<_> = self.models[model_index]
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| match parameter {
                ModelParameter::Recorded { id, .. } if *id == parameter_id => Some(index),
                _ => None,
            })
            .collect();
        let parameter_index = matches
            .last()
            .copied()
            .ok_or_else(|| ToneError::format(format!("{label} parameter is absent")))?;
        match &self.models[model_index].parameters[parameter_index] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Float { .. },
                ..
            } => {}
            ModelParameter::Recorded { prefix, .. } if *prefix != 0x3f => {
                return Err(ToneError::format(format!(
                    "{label} raw prefix is not canonical 0x3f"
                )));
            }
            _ => {
                return Err(ToneError::format(format!(
                    "{label} target has an unsupported value type"
                )));
            }
        }
        if self.parameter_value_offsets[model_index][parameter_index].is_none() {
            return Err(ToneError::format(format!(
                "{label} target has no unique raw value word"
            )));
        }
        Ok((model_index, parameter_index))
    }

    fn validate_host_fraction(value: f32, label: &str) -> Result<(), ToneError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ToneError::format(format!(
                "{label} requires a finite host-scale value in 0..=1"
            )));
        }
        Ok(())
    }

    fn float_control_fraction_at(
        &self,
        model_index: usize,
        parameter_index: usize,
        label: &str,
    ) -> Result<f32, ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        let value = f32::from_bits(*bits);
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ToneError::format(format!(
                "{label} value is outside the finite host scale 0..=1"
            )));
        }
        Ok(value)
    }

    fn set_float_control_fraction_at(
        &mut self,
        model_index: usize,
        parameter_index: usize,
        value: f32,
        label: &str,
    ) -> Result<(), ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        let current = f32::from_bits(*bits);
        if !current.is_finite() || !(0.0..=1.0).contains(&current) {
            return Err(ToneError::format(format!(
                "existing {label} value is outside the finite host scale 0..=1"
            )));
        }
        let offset = self.parameter_value_offsets[model_index][parameter_index]
            .ok_or_else(|| ToneError::format(format!("{label} has no unique raw value word")))?;
        let new_bits = value.to_bits();
        self.bytes[offset..offset + 4].copy_from_slice(&new_bits.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &mut self.models[model_index].parameters[parameter_index]
        {
            *bits = new_bits;
        }
        Ok(())
    }

    /// Cab model id. Only L6AS exposes slot `0x00030001` as a cab.
    pub fn cab_model_id(&self) -> Option<u32> {
        if !self.container.exposes_cab() {
            return None;
        }
        self.models
            .iter()
            .find(|model| model.slot == CAB_SLOT)
            .map(|model| model.model_id)
    }

    /// Asset name for the exposed cab model id.
    pub fn cab_asset_name(&self) -> Option<&'static str> {
        self.cab_model_id().and_then(pod_assets::asset_name)
    }

    /// Slots whose id is not in `kGPXTSlot_*`.
    pub fn undecoded_slots(&self) -> usize {
        self.models
            .iter()
            .filter(|model| SlotRole::from_id(model.slot).is_none())
            .count()
    }

    /// PARM chunks whose length is the 12-byte record. Their ranges are still unknown.
    pub fn recorded_parameter_chunks(&self) -> usize {
        self.models
            .iter()
            .flat_map(|model| &model.parameters)
            .filter(|parameter| matches!(parameter, ModelParameter::Recorded { .. }))
            .count()
    }

    /// PARM chunks that are not the 12-byte record.
    pub fn undecoded_parameter_chunks(&self) -> usize {
        self.models
            .iter()
            .flat_map(|model| &model.parameters)
            .filter(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
            .count()
    }

    /// One line per model, then one line per parameter. This does not edit the file.
    pub fn control_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for model in &self.models {
            lines.push(pod_assets::describe_model(
                model.slot,
                model.model_id,
                model.ordinal,
                model.reserved,
                model.enabled,
            ));
            for parameter in &model.parameters {
                lines.push(match parameter {
                    ModelParameter::Opaque { len } => format!("opaque parameter {len} bytes"),
                    ModelParameter::Recorded { prefix, id, value } => {
                        pod_assets::describe_parameter(*prefix, *id, &value.label())
                    }
                });
            }
        }
        lines
    }

    /// Hex id, with the asset name in front when the table has one.
    pub fn model_label(id: Option<u32>) -> String {
        match id {
            Some(id) => match pod_assets::asset_name(id) {
                Some(name) => format!("{name} {id:08x}"),
                None => format!("{id:08x}"),
            },
            None => "absent".into(),
        }
    }

    /// Replace the fixed 32 UTF-16BE PINF name units. No other byte changes.
    pub fn rename(&mut self, name: &str) -> Result<(), ToneError> {
        let units: Vec<u16> = name.encode_utf16().collect();
        if units.len() > 32 || units.contains(&0) {
            return Err(ToneError::format(
                "name must contain at most 32 UTF-16 code units and no NUL",
            ));
        }
        self.bytes[self.name_offset..self.name_offset + 64].fill(0);
        for (index, unit) in units.iter().enumerate() {
            let at = self.name_offset + 2 * index;
            self.bytes[at..at + 2].copy_from_slice(&unit.to_be_bytes());
        }
        self.name = name.to_owned();
        Ok(())
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredTone {
    tone: PodTone,
    /// Bytes last imported or successfully saved. Compared with the applied tone.
    saved: Vec<u8>,
}

/// In-memory list of individual tone files. This is not a bank format.
///
/// Each tone keeps the bytes last imported or successfully saved. An applied
/// edit that differs from that baseline is unsaved. Saving or discarding one
/// tone does not change another tone's baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    tones: Vec<StoredTone>,
}

impl Library {
    pub fn clear(&mut self) {
        self.tones.clear();
    }

    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let tone = PodTone::parse(bytes)?;
        let saved = tone.as_bytes().to_vec();
        self.tones.push(StoredTone { tone, saved });
        Ok(self.tones.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.tones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tones.is_empty()
    }

    pub fn tone(&self, index: usize) -> Option<&PodTone> {
        self.tones.get(index).map(|entry| &entry.tone)
    }

    pub fn set_amp_drive_fraction(&mut self, index: usize, value: f32) -> Result<(), ToneError> {
        self.set_amp_control_fraction(index, AmpControl::Drive, value)
    }

    pub fn set_amp_control_fraction(
        &mut self,
        index: usize,
        control: AmpControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_amp_control_fraction(control, value)
    }

    pub fn set_stomp_drive_fraction(&mut self, index: usize, value: f32) -> Result<(), ToneError> {
        self.set_stomp_control_fraction(index, StompControl::Drive, value)
    }

    pub fn set_stomp_control_fraction(
        &mut self,
        index: usize,
        control: StompControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_control_fraction(control, value)
    }

    pub fn set_mod_control_fraction(
        &mut self,
        index: usize,
        control: ModControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_control_fraction(control, value)
    }

    pub fn set_reverb_control_fraction(
        &mut self,
        index: usize,
        control: ReverbControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_reverb_control_fraction(control, value)
    }

    pub fn set_delay_control_fraction(
        &mut self,
        index: usize,
        control: DelayControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_control_fraction(control, value)
    }

    pub fn set_delay_heads_selection(
        &mut self,
        index: usize,
        selection: DelayHeadsSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_heads_selection(selection)
    }

    pub fn set_delay_bits_selection(
        &mut self,
        index: usize,
        selection: DelayBitsSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_bits_selection(selection)
    }

    pub fn replace(&mut self, index: usize, bytes: Vec<u8>) -> Result<(), ToneError> {
        let tone = PodTone::parse(bytes)?;
        let slot = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        slot.tone = tone;
        Ok(())
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.tones
            .get(index)
            .is_some_and(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn any_dirty(&self) -> bool {
        self.tones
            .iter()
            .any(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn dirty_count(&self) -> usize {
        self.tones
            .iter()
            .filter(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.tones
            .iter()
            .position(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    /// Record a successful save of this tone only.
    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.saved = entry.tone.as_bytes().to_vec();
        Ok(())
    }

    /// Restore one tone to its imported or last-saved bytes.
    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        if entry.tone.as_bytes() != entry.saved.as_slice() {
            entry.tone = PodTone::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}

/// Target-specific view of a Workbench bank/collection containing only PODxt
/// Live tones. The generic guitar document continues to reject such files.
#[derive(Clone, Debug)]
pub struct PodBundleLibrary {
    bytes: Vec<u8>,
    saved: Vec<u8>,
    banks: Vec<PodBundleBank>,
}

#[derive(Clone, Debug)]
struct PodBundleBank {
    name: String,
    entry_ranges: Vec<Range<usize>>,
    chunk_start: usize,
    data_length: usize,
}

impl PodBundleLibrary {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        if bytes.len() < 12 || &bytes[..4] != b"FORM" || !matches!(&bytes[8..12], b"L6BA" | b"L6CO")
        {
            return Err(ToneError::format(
                "expected Workbench FORM/L6BA or FORM/L6CO",
            ));
        }
        if preset::word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err(ToneError::format(
                "bundle FORM length does not match the file",
            ));
        }
        let mut banks = Vec::new();
        let mut tone_count = 0;
        let outer = preset::chunks(&bytes, 12..bytes.len()).map_err(ToneError::format)?;
        for bank in outer.iter().filter(|entry| {
            preset::container(&bytes, entry, b"BANK") || preset::container(&bytes, entry, b"FLDR")
        }) {
            let children = preset::chunks(&bytes, bank.data.start + 4..bank.data.end)
                .map_err(ToneError::format)?;
            let info = preset::unique(&children, b"BINF").map_err(ToneError::format)?;
            if info.data.len() < 68 {
                return Err(ToneError::format("truncated bank information"));
            }
            let units: Vec<_> = bytes[info.data.start + 4..info.data.start + 68]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_be_bytes(*unit))
                .take_while(|&unit| unit != 0)
                .collect();
            let name = String::from_utf16(&units)
                .map_err(|_| ToneError::format("invalid UTF-16 bank name"))?
                .trim_end()
                .to_owned();
            let mut entry_ranges = Vec::new();
            for entry in children.iter().filter(|child| {
                preset::container(&bytes, child, b"L6PA")
                    || preset::container(&bytes, child, b"L6FS")
                    || preset::container(&bytes, child, b"L6AS")
            }) {
                let payload_end = entry.data.end;
                let full_end = payload_end + (entry.data.len() & 1);
                let full_range = entry.data.start - 8..full_end;
                let tone_range = entry.data.start - 8..payload_end;
                PodTone::parse(bytes[tone_range].to_vec())?;
                tone_count += 1;
                entry_ranges.push(full_range);
            }
            banks.push(PodBundleBank {
                name,
                entry_ranges,
                chunk_start: bank.data.start - 8,
                data_length: bank.data.len(),
            });
        }
        if tone_count == 0 {
            return Err(ToneError::format("POD bank/collection contains no tones"));
        }
        let saved = bytes.clone();
        Ok(Self {
            bytes,
            saved,
            banks,
        })
    }

    pub fn bank_count(&self) -> usize {
        self.banks.len()
    }

    pub fn entry_count(&self) -> usize {
        self.banks.iter().map(|bank| bank.entry_ranges.len()).sum()
    }

    pub fn entry_location(&self, index: usize) -> Option<(usize, usize)> {
        let mut offset = index;
        for (bank_index, bank) in self.banks.iter().enumerate() {
            if offset < bank.entry_ranges.len() {
                return Some((bank_index, offset));
            }
            offset -= bank.entry_ranges.len();
        }
        None
    }

    pub fn preset_count(&self, bank: usize) -> Option<usize> {
        self.banks.get(bank).map(|entry| entry.entry_ranges.len())
    }

    pub fn bank_name(&self, bank: usize) -> Option<&str> {
        self.banks.get(bank).map(|entry| entry.name.as_str())
    }

    pub fn tone(&self, bank: usize, preset: usize) -> Result<PodTone, ToneError> {
        let range = self
            .banks
            .get(bank)
            .and_then(|entry| entry.entry_ranges.get(preset))
            .ok_or_else(|| ToneError::format("bank or preset index is outside the library"))?
            .clone();
        // Parse by the embedded FORM's own declared length so its parent's IFF
        // alignment byte never enters PodTone.
        let form_length = preset::word(&self.bytes, range.start + 4) as usize;
        let end = range.start + 8 + form_length;
        if end > range.end {
            return Err(ToneError::format("embedded tone exceeds its IFF chunk"));
        }
        PodTone::parse(self.bytes[range.start..end].to_vec())
    }

    pub fn edit_tone(
        &mut self,
        bank: usize,
        preset: usize,
        edit: impl FnOnce(&mut PodTone) -> Result<(), ToneError>,
    ) -> Result<(), ToneError> {
        let mut tone = self.tone(bank, preset)?;
        edit(&mut tone)?;
        self.replace_tone(bank, preset, &tone)
    }

    pub fn edit_entry(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut PodTone) -> Result<(), ToneError>,
    ) -> Result<(), ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.edit_tone(bank, preset, edit)
    }

    pub fn replace_entry(&mut self, index: usize, bytes: Vec<u8>) -> Result<(), ToneError> {
        let replacement = PodTone::parse(bytes)?;
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.replace_tone(bank, preset, &replacement)
    }

    fn replace_tone(
        &mut self,
        bank_index: usize,
        preset_index: usize,
        replacement: &PodTone,
    ) -> Result<(), ToneError> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the library"))?;
        let range = bank
            .entry_ranges
            .get(preset_index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?
            .clone();
        let old_span = range.end - range.start;
        let old_form_length = preset::word(&self.bytes, range.start + 4) as usize;
        let old_payload_length = 8 + old_form_length;
        let tone_bytes = replacement.as_bytes();
        let replacement_span = tone_bytes.len() + (tone_bytes.len() & 1);
        let new_bank_length = bank
            .data_length
            .checked_sub(old_span)
            .and_then(|length| length.checked_add(replacement_span))
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        let new_file_length = self
            .bytes
            .len()
            .checked_sub(old_span)
            .and_then(|length| length.checked_add(replacement_span))
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        if new_file_length as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        let bank_length = u32::try_from(new_bank_length)
            .map_err(|_| ToneError::format("bank exceeds IFF size capacity"))?;
        let root_length = u32::try_from(new_file_length - 8)
            .map_err(|_| ToneError::format("bundle exceeds IFF size capacity"))?;
        let chunk_start = bank.chunk_start;
        let mut encoded = tone_bytes.to_vec();
        if tone_bytes.len() & 1 != 0 {
            let original_pad =
                (old_payload_length & 1 == 1).then(|| self.bytes[range.start + old_payload_length]);
            encoded.push(original_pad.unwrap_or(0));
        }
        let mut bytes = self.bytes.clone();
        bytes.splice(range, encoded);
        bytes[chunk_start + 4..chunk_start + 8].copy_from_slice(&bank_length.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_length.to_be_bytes());
        let saved = self.saved.clone();
        let mut next = Self::parse(bytes)?;
        next.saved = saved;
        *self = next;
        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_dirty(&self) -> bool {
        self.as_bytes() != self.saved
    }

    pub fn revert(&mut self) -> Result<(), ToneError> {
        if self.is_dirty() {
            let restored = Self::parse(self.saved.clone())?;
            *self = restored;
        }
        Ok(())
    }

    pub fn note_saved(&mut self) {
        self.saved = self.bytes.clone();
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, self.as_bytes()).map_err(ToneError::format)
    }
}

fn utf16_name(bytes: &[u8]) -> Result<String, ToneError> {
    if bytes.len() != 64 {
        return Err(ToneError::format("PINF name field is not 64 bytes"));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16(&units).map_err(|_| ToneError::format("invalid UTF-16 tone name"))
}

/// Facts from [`std::fs::metadata`]. That call does not open the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFacts {
    pub regular_file: bool,
    pub len: u64,
}

pub fn path_facts(path: &Path) -> std::io::Result<PathFacts> {
    let meta = std::fs::metadata(path)?;
    Ok(PathFacts {
        regular_file: meta.is_file(),
        len: meta.len(),
    })
}

/// Reads only after `facts` names a regular file at or under the ceiling.
///
/// A pipe, socket, or device node is refused first, including when its
/// reported length is zero. `read` is not called in that case.
pub fn read_regular_tone(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<Vec<u8>>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "tone input must be a regular file. A pipe, socket, or device node is not opened.",
        ));
    }
    if facts.len > TONE_BYTES_CEILING {
        return Err(ToneError::format(
            "tone file exceeds the 1 MiB offline librarian ceiling",
        ));
    }
    let bytes = read().map_err(|error| ToneError::format(error.to_string()))?;
    if bytes.len() as u64 > TONE_BYTES_CEILING {
        return Err(ToneError::format(
            "tone file exceeds the 1 MiB offline librarian ceiling",
        ));
    }
    Ok(bytes)
}

/// Reads a bounded Workbench POD bank after path and opened-handle checks.
///
/// The reader is limited to the application ceiling plus one sentinel byte,
/// including when the file grows after either metadata check.
pub fn read_regular_bundle<R: std::io::Read>(
    facts: std::io::Result<PathFacts>,
    open: impl FnOnce() -> std::io::Result<(PathFacts, R)>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "POD bank input must be a regular file. A pipe, socket, or device node is not opened.",
        ));
    }
    if facts.len > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    let (opened_facts, reader) = open().map_err(|error| ToneError::format(error.to_string()))?;
    if !opened_facts.regular_file {
        return Err(ToneError::format(
            "POD bank input must be a regular file. A pipe, socket, or device node is not read.",
        ));
    }
    if opened_facts.len > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    let mut bytes = Vec::with_capacity(opened_facts.len.min(BUNDLE_BYTES_CEILING) as usize);
    let mut bounded = std::io::Read::take(reader, BUNDLE_BYTES_CEILING + 1);
    std::io::Read::read_to_end(&mut bounded, &mut bytes)
        .map_err(|error| ToneError::format(error.to_string()))?;
    if bytes.len() as u64 > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
pub(crate) fn fixture(
    kind: ContainerKind,
    device_id: u32,
    device_version: u32,
    slots: &[(u32, u32)],
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&device_id.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    info[72..76].copy_from_slice(&device_version.to_be_bytes());
    info[76..80].copy_from_slice(&[0x5a, 0x5a, 0x5a, 0x5a]);
    let mut patch = b"PATC".to_vec();
    for &(slot, model_id) in slots {
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0x10, 0x20, 0x30, 0x40]);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &[0x11, 0x22, 0x33, 0x44]));
        model.extend(chunk(
            b"PARM",
            &[
                0xab, 0xcd, 0xef, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
            ],
        ));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"JUNK", &[9, 8, 7]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
struct BuiltModel<'a> {
    slot: u32,
    model_id: u32,
    tail: &'a [u8],
    parameters: &'a [&'a [u8]],
}

#[cfg(test)]
fn custom_tone(kind: ContainerKind, models: &[BuiltModel<'_>]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    info[72..76].copy_from_slice(&PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
    info[76..80].copy_from_slice(&[0x5a, 0xa5, 0x96, 0x69]);
    let mut patch = b"PATC".to_vec();
    for entry in models {
        let mut minf = Vec::new();
        minf.extend_from_slice(&entry.model_id.to_be_bytes());
        minf.extend_from_slice(&entry.slot.to_be_bytes());
        minf.extend_from_slice(entry.tail);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        for parameter in entry.parameters {
            model.extend(chunk(b"PARM", parameter));
        }
        model.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod_assets::{self, SlotRole};

    fn drive_record(prefix: u8, id: u32, type_word: u32, bits: u32) -> [u8; 12] {
        let mut bytes = [0; 12];
        bytes[..4].copy_from_slice(&(((prefix as u32) << 24) | id).to_be_bytes());
        bytes[4..8].copy_from_slice(&type_word.to_be_bytes());
        bytes[8..12].copy_from_slice(&bits.to_be_bytes());
        bytes
    }

    fn drive_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: model,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    fn bundle_with_tone(tone: &[u8]) -> Vec<u8> {
        bundle_with_banks(&[vec![tone.to_vec()]])
    }

    fn bundle_with_banks(banks: &[Vec<Vec<u8>>]) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        let mut root = b"L6CO".to_vec();
        root.extend(chunk(b"JUNK", &[0x71, 0x72, 0x73]));
        for (index, entries) in banks.iter().enumerate() {
            let mut info = vec![0; 68];
            info[4..6].copy_from_slice(&u16::from(b'A' + index as u8).to_be_bytes());
            let mut bank = if index % 2 == 0 {
                b"BANK".to_vec()
            } else {
                b"FLDR".to_vec()
            };
            bank.extend(chunk(b"BINF", &info));
            bank.extend(chunk(b"NOTE", &[0xde, 0xad, 0xbe]));
            for tone in entries {
                bank.extend_from_slice(tone);
            }
            root.extend(chunk(b"LIST", &bank));
        }
        chunk(b"FORM", &root)
    }

    fn bundle_with_total_length(tone: &[u8], total_length: usize) -> Vec<u8> {
        let mut bytes = bundle_with_tone(tone);
        assert!(total_length >= bytes.len() + 8);
        let payload_length = total_length - bytes.len() - 8;
        assert!(payload_length.is_multiple_of(2));
        bytes.extend_from_slice(b"JUNK");
        bytes.extend_from_slice(&(payload_length as u32).to_be_bytes());
        bytes.resize(total_length, 0x5a);
        bytes[4..8].copy_from_slice(&((total_length - 8) as u32).to_be_bytes());
        bytes
    }

    fn tone_with_empty_unknown_chunk(tone: &[u8]) -> Vec<u8> {
        let mut bytes = tone.to_vec();
        bytes.extend_from_slice(b"EXTR\0\0\0\0");
        let form_length = bytes.len() - 8;
        bytes[4..8].copy_from_slice(&(form_length as u32).to_be_bytes());
        bytes
    }

    fn stomp_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: model,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    fn mod_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: model,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    fn reverb_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: REVERB_SLOT,
                model_id: model,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    fn delay_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: DELAY_SLOT,
                model_id: model,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    #[derive(Clone, Copy)]
    enum MappedEffect {
        Mod,
        Reverb,
        Delay,
    }

    fn set_mapped_effect(
        tone: &mut PodTone,
        effect: MappedEffect,
        unsupported_control: bool,
        value: f32,
    ) -> Result<(), ToneError> {
        match (effect, unsupported_control) {
            (MappedEffect::Mod, false) => tone.set_mod_control_fraction(ModControl::Depth, value),
            (MappedEffect::Mod, true) => tone.set_mod_control_fraction(ModControl::Manual, value),
            (MappedEffect::Reverb, false) => {
                tone.set_reverb_control_fraction(ReverbControl::Dwell, value)
            }
            (MappedEffect::Reverb, true) => {
                tone.set_reverb_control_fraction(ReverbControl::Predelay, value)
            }
            (MappedEffect::Delay, false) => {
                tone.set_delay_control_fraction(DelayControl::Feedback, value)
            }
            (MappedEffect::Delay, true) => {
                tone.set_delay_control_fraction(DelayControl::Drive, value)
            }
        }
    }

    fn mapped_effect_value(tone: &PodTone, effect: MappedEffect) -> Result<f32, ToneError> {
        match effect {
            MappedEffect::Mod => tone.mod_control_fraction(ModControl::Depth),
            MappedEffect::Reverb => tone.reverb_control_fraction(ReverbControl::Dwell),
            MappedEffect::Delay => tone.delay_control_fraction(DelayControl::Feedback),
        }
    }

    fn mapped_effect_tone(slot: u32, model_id: u32, version: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot,
                model_id,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    #[test]
    fn amp_drive_accepts_host_scale_values_and_changes_only_the_last_raw_value_word() {
        let first = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3e80_0000);
        let last = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3f00_0000);
        let unknown = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
        let original = drive_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PLEXI_LEAD_100_MODEL,
            &[&first, &unknown, &last],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.amp_drive_fraction().unwrap(), 0.5);
        tone.set_amp_drive_fraction(0.317).unwrap();
        let changed: Vec<_> = original
            .iter()
            .zip(tone.as_bytes())
            .enumerate()
            .filter_map(|(index, (before, after))| (before != after).then_some(index))
            .collect();
        let last_at = original
            .windows(12)
            .position(|window| window == last)
            .unwrap();
        assert_eq!(changed, (last_at + 8..last_at + 12).collect::<Vec<_>>());
        assert_eq!(
            tone.amp_drive_fraction().unwrap().to_bits(),
            0.317f32.to_bits()
        );
        let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
        assert_eq!(
            reopened.amp_drive_fraction().unwrap().to_bits(),
            0.317f32.to_bits()
        );
        assert!(tone.as_bytes().windows(12).any(|window| window == first));
        assert!(tone.as_bytes().windows(12).any(|window| window == unknown));
    }

    #[test]
    fn amp_drive_finite_endpoints_nonintegers_and_signed_zero_keep_exact_float_bits() {
        let record = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0);
        let values = [0.0, 1.0, 0.314_159_27, f32::from_bits(1), -0.0];
        for value in values {
            let mut tone = PodTone::parse(drive_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PLEXI_LEAD_100_MODEL,
                &[&record],
            ))
            .unwrap();
            tone.set_amp_drive_fraction(value).unwrap();
            assert_eq!(
                tone.amp_drive_fraction().unwrap().to_bits(),
                value.to_bits()
            );
            assert!(
                tone.as_bytes()
                    .windows(4)
                    .any(|bytes| bytes == value.to_bits().to_be_bytes())
            );
        }
    }

    #[test]
    fn source_mapped_amp_controls_have_distinct_ids_and_preserve_every_other_byte() {
        assert_eq!(AMP_CONTROL_MODEL_IDS.len(), 106);
        assert!(!AMP_CONTROL_MODEL_IDS.contains(&0x0002_0000)); // No Amp
        let mut unique = AMP_CONTROL_MODEL_IDS.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), AMP_CONTROL_MODEL_IDS.len());

        for model_id in AMP_CONTROL_MODEL_IDS {
            let records: Vec<_> = AmpControl::ALL
                .into_iter()
                .enumerate()
                .map(|(index, control)| {
                    drive_record(
                        0x3f,
                        control.param_id(),
                        1,
                        (0.1f32 + index as f32 * 0.1).to_bits(),
                    )
                })
                .collect();
            let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
            let original = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &refs);
            for control in AmpControl::ALL {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let previous = tone.amp_control_fraction(control).unwrap();
                let new_value = (previous + 0.01).min(1.0);
                tone.set_amp_control_fraction(control, new_value).unwrap();
                assert_eq!(
                    tone.amp_control_fraction(control).unwrap().to_bits(),
                    new_value.to_bits(),
                    "{} on amp model {model_id:08x}",
                    control.label()
                );
                let (model_index, parameter_index) = tone.amp_control_location(control).unwrap();
                let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
                let expected = [
                    original[..offset].to_vec(),
                    new_value.to_bits().to_be_bytes().to_vec(),
                    original[offset + 4..].to_vec(),
                ]
                .concat();
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "{} on amp model {model_id:08x}",
                    control.label()
                );
            }
        }
    }

    #[test]
    fn source_mapped_stomp_controls_patch_only_their_existing_value_words() {
        assert_eq!(STOMP_SLOT, 0x0002_0003);
        assert_eq!(STOMP_DRIVE_MODEL_ID, 0x0205_0000);
        assert_eq!(STOMP_CONTROL_MODEL_IDS.len(), 31);
        let unknown = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
        let profiles = [
            (
                0x0200_0002,
                [
                    Some(StompControl::Sustain),
                    Some(StompControl::Level),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x0200_0003,
                [
                    Some(StompControl::Level),
                    Some(StompControl::Sustain),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x0200_0004,
                [
                    Some(StompControl::Sustain),
                    Some(StompControl::Level),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x0200_0005,
                [
                    Some(StompControl::Sensitivity),
                    Some(StompControl::Level),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x0200_0006,
                [
                    Some(StompControl::Amount),
                    Some(StompControl::Level),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x0200_0007,
                [
                    Some(StompControl::Ramp),
                    Some(StompControl::Depth),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x020a_0000,
                [
                    Some(StompControl::Sensitivity),
                    Some(StompControl::Q),
                    None,
                    None,
                    None,
                ],
            ),
            (
                0x020a_0001,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Decay),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0002,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Attack),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0003,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Decay),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0004,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Decay),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0005,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Decay),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0006,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Decay),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0007,
                [None, None, None, Some(StompControl::Mix), None],
            ),
            (
                0x020a_0008,
                [
                    Some(StompControl::S1Ogt),
                    Some(StompControl::S2Ogt),
                    None,
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_0009,
                [
                    None,
                    Some(StompControl::Filter),
                    Some(StompControl::Attack),
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_000a,
                [
                    None,
                    Some(StompControl::Sensitivity),
                    Some(StompControl::Q),
                    None,
                    None,
                ],
            ),
            (
                0x020a_000b,
                [
                    Some(StompControl::Decay),
                    Some(StompControl::Sensitivity),
                    Some(StompControl::Q),
                    None,
                    None,
                ],
            ),
            (
                0x020a_000c,
                [None, None, None, Some(StompControl::Mix), None],
            ),
            (
                0x0205_0000,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    Some(StompControl::Tone),
                    None,
                    None,
                ],
            ),
            (
                0x0205_0002,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    Some(StompControl::Tone),
                    None,
                    None,
                ],
            ),
            (
                0x0205_0003,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    Some(StompControl::Tone),
                    None,
                    None,
                ],
            ),
            (
                0x0205_0004,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    Some(StompControl::Tone),
                    None,
                    None,
                ],
            ),
            (
                0x0205_0005,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    Some(StompControl::Tone),
                    None,
                    None,
                ],
            ),
            (
                0x0205_0001,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Treble),
                    Some(StompControl::Gain),
                    Some(StompControl::Bass),
                    None,
                ],
            ),
            (
                0x0205_0006,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Contour),
                    Some(StompControl::Gain),
                    Some(StompControl::Mid),
                    Some(StompControl::MidFrequency),
                ],
            ),
            (
                0x0205_0007,
                [
                    Some(StompControl::Gain),
                    Some(StompControl::Bass),
                    Some(StompControl::Treble),
                    Some(StompControl::Mid),
                    Some(StompControl::MidFrequency),
                ],
            ),
            (
                0x0205_0008,
                [
                    Some(StompControl::Bass),
                    Some(StompControl::Treble),
                    Some(StompControl::Drive),
                    Some(StompControl::Gain),
                    None,
                ],
            ),
            (
                0x0205_0009,
                [
                    Some(StompControl::Drive),
                    Some(StompControl::Tone),
                    None,
                    Some(StompControl::Blend),
                    None,
                ],
            ),
            (
                0x020a_000d,
                [
                    Some(StompControl::S1Ogt),
                    Some(StompControl::S2Ogt),
                    None,
                    Some(StompControl::Mix),
                    None,
                ],
            ),
            (
                0x020a_000e,
                [
                    Some(StompControl::Position),
                    None,
                    None,
                    Some(StompControl::Mix),
                    None,
                ],
            ),
        ];
        assert_eq!(profiles.len(), STOMP_CONTROL_MODEL_IDS.len());
        assert_eq!(
            profiles
                .iter()
                .map(|(_, expected_layout)| expected_layout.iter().flatten().count())
                .sum::<usize>(),
            86
        );
        let mut profile_ids: Vec<_> = profiles.iter().map(|(model_id, _)| *model_id).collect();
        profile_ids.sort_unstable();
        profile_ids.dedup();
        assert_eq!(profile_ids.len(), profiles.len());
        for (model_id, expected_layout) in profiles {
            assert!(STOMP_CONTROL_MODEL_IDS.contains(&model_id));
            for control in StompControl::ALL {
                assert_eq!(
                    control.selector_id(model_id).is_some(),
                    expected_layout.contains(&Some(control)),
                    "unexpected {} mapping on model {model_id:08x}",
                    control.label()
                );
            }
            for (position, expected_control) in expected_layout.into_iter().enumerate() {
                let Some(control) = expected_control else {
                    continue;
                };
                let position = position as u32 + 1;
                assert_eq!(
                    control.selector_id(model_id),
                    Some(0x0003_0000 + position),
                    "{} on model {model_id:08x}",
                    control.label()
                );
                let parameter_id = 0x0010_0000 + position;
                assert_eq!(control.param_id(model_id), Some(parameter_id));
                let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
                let earlier_duplicate = drive_record(0x3f, parameter_id, 1, 0x3e00_0000);
                let bytes = stomp_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    model_id,
                    &[&unknown, &earlier_duplicate, &record],
                );
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert_eq!(
                    tone.stomp_control_fraction(control).unwrap().to_bits(),
                    0x3f00_0000
                );
                tone.set_stomp_control_fraction(control, 0.317).unwrap();
                let value_offset = bytes
                    .windows(record.len())
                    .position(|window| window == record)
                    .unwrap()
                    + 8;
                let earlier_value_offset = bytes
                    .windows(earlier_duplicate.len())
                    .position(|window| window == earlier_duplicate)
                    .unwrap()
                    + 8;
                let expected = [
                    bytes[..value_offset].to_vec(),
                    0.317f32.to_bits().to_be_bytes().to_vec(),
                    bytes[value_offset + 4..].to_vec(),
                ]
                .concat();
                assert_eq!(tone.as_bytes(), expected, "{}", control.label());
                assert_eq!(
                    &tone.as_bytes()[earlier_value_offset..earlier_value_offset + 4],
                    &bytes[earlier_value_offset..earlier_value_offset + 4],
                    "earlier duplicate {}",
                    control.label()
                );
                assert_eq!(
                    tone.stomp_control_fraction(control).unwrap().to_bits(),
                    0.317f32.to_bits(),
                    "{} on model {model_id:08x}",
                    control.label()
                );
            }
        }
    }

    #[test]
    fn source_mapped_mod_percent_controls_patch_only_their_existing_value_words() {
        assert_eq!(MOD_SLOT, 0x0002_0004);
        assert_eq!(MOD_CONTROL_MODEL_IDS.len(), 24);
        let profiles: [(u32, [Option<ModControl>; 3]); 24] = [
            (
                0x0203_0004,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Bass),
                    Some(ModControl::Treble),
                ],
            ),
            (
                0x0203_0005,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Bass),
                    Some(ModControl::Treble),
                ],
            ),
            (0x0203_0006, [Some(ModControl::Depth), None, None]),
            (
                0x0203_0007,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Feedback),
                    Some(ModControl::Manual),
                ],
            ),
            (0x0203_0008, [Some(ModControl::Feedback), None, None]),
            (0x0203_0009, [Some(ModControl::Depth), None, None]),
            (0x0203_000a, [Some(ModControl::Wave), None, None]),
            (0x0203_000b, [Some(ModControl::Wave), None, None]),
            (0x0203_000c, [None, Some(ModControl::Tone), None]),
            (0x0203_000d, [None, Some(ModControl::Tone), None]),
            (0x0203_000e, [Some(ModControl::Wave), None, None]),
            (
                0x0203_0015,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Bass),
                    Some(ModControl::Treble),
                ],
            ),
            (
                0x0203_0012,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Predelay),
                    Some(ModControl::Feedback),
                ],
            ),
            (
                0x0203_001e,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Predelay),
                    Some(ModControl::Feedback),
                ],
            ),
            (
                0x0203_0014,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Bass),
                    Some(ModControl::Treble),
                ],
            ),
            (
                0x0203_001f,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Predelay),
                    Some(ModControl::Feedback),
                ],
            ),
            (
                0x0203_0013,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Predelay),
                    Some(ModControl::Feedback),
                ],
            ),
            (
                0x0203_0011,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Bass),
                    Some(ModControl::Treble),
                ],
            ),
            (
                0x0203_001a,
                [Some(ModControl::Depth), Some(ModControl::Q), None],
            ),
            (
                0x0203_001b,
                [
                    Some(ModControl::Depth),
                    Some(ModControl::Q),
                    Some(ModControl::Frequency),
                ],
            ),
            (
                0x0203_0017,
                [Some(ModControl::Feedback), Some(ModControl::Depth), None],
            ),
            (
                0x0203_0018,
                [Some(ModControl::Depth), Some(ModControl::Q), None],
            ),
            (
                0x0203_0019,
                [
                    Some(ModControl::Feedback),
                    Some(ModControl::Flutter),
                    Some(ModControl::Distortion),
                ],
            ),
            (
                0x0203_001c,
                [Some(ModControl::Depth), Some(ModControl::Q), None],
            ),
        ];
        assert_eq!(
            profiles
                .iter()
                .map(|(_, layout)| layout.iter().flatten().count())
                .sum::<usize>(),
            52
        );
        assert_eq!(
            profiles
                .iter()
                .map(|(model_id, _)| *model_id)
                .collect::<Vec<_>>(),
            MOD_CONTROL_MODEL_IDS
        );
        for (model_id, layout) in profiles {
            assert!(MOD_CONTROL_MODEL_IDS.contains(&model_id));
            for control in ModControl::ALL {
                assert_eq!(
                    control.selector_id(model_id).is_some(),
                    layout.contains(&Some(control)),
                    "unexpected {} mapping on model {model_id:08x}",
                    control.label()
                );
            }
            let mut records = Vec::new();
            for (index, expected) in layout.into_iter().enumerate() {
                if expected.is_some() {
                    let parameter_id = 0x0010_0000 + index as u32 + 1;
                    records.push(drive_record(0x3f, parameter_id, 1, 0x3f00_0000));
                }
            }
            let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
            let bytes = mod_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            for control in ModControl::ALL {
                let Some(position) = control.selector_id(model_id).map(|id| id - 0x0004_0002)
                else {
                    continue;
                };
                assert_eq!(control.selector_id(model_id), Some(0x0004_0002 + position));
                let parameter_id = 0x0010_0000 + position;
                assert_eq!(control.param_id(model_id), Some(parameter_id));
                assert_eq!(tone.mod_control_fraction(control).unwrap(), 0.5);
                tone.set_mod_control_fraction(control, 0.317).unwrap();
                let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
                let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
                assert_eq!(
                    &tone.as_bytes()[offset..offset + 4],
                    &0.317f32.to_bits().to_be_bytes()
                );
            }
            assert_eq!(tone.models()[0].model_id, model_id);
        }
    }

    #[test]
    fn source_mapped_reverb_percent_controls_patch_only_their_existing_value_words() {
        assert_eq!(REVERB_SLOT, 0x0002_0006);
        let profiles: [(u32, [Option<ReverbControl>; 3]); 15] = [
            (
                0x0204_0002,
                [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
            ),
            (
                0x0204_0003,
                [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
            ),
            (
                0x0204_0004,
                [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
            ),
            (
                0x0204_0005,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_0006,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_0007,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_0008,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_0009,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000a,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000b,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000c,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000d,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000e,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_000f,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
            (
                0x0204_0010,
                [
                    Some(ReverbControl::Predelay),
                    Some(ReverbControl::Decay),
                    Some(ReverbControl::Tone),
                ],
            ),
        ];
        assert_eq!(
            profiles.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            REVERB_CONTROL_MODEL_IDS
        );
        for (model_id, layout) in profiles {
            for control in ReverbControl::ALL {
                assert_eq!(
                    control.selector_id(model_id).is_some(),
                    layout.contains(&Some(control)),
                    "unexpected {} mapping on model {model_id:08x}",
                    control.label()
                );
            }
            let mut records = Vec::new();
            for control in layout.into_iter().flatten() {
                let parameter_id = control.param_id(model_id).unwrap();
                records.push(drive_record(0x3f, parameter_id, 1, 0x3f00_0000));
            }
            let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
            let bytes = reverb_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            for control in ReverbControl::ALL {
                if control.selector_id(model_id).is_none() {
                    continue;
                }
                let parameter_id = control.param_id(model_id).unwrap();
                assert_eq!(tone.reverb_control_fraction(control).unwrap(), 0.5);
                tone.set_reverb_control_fraction(control, 0.317).unwrap();
                let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
                let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
                assert_eq!(
                    &tone.as_bytes()[offset..offset + 4],
                    &0.317f32.to_bits().to_be_bytes()
                );
            }
        }
    }

    #[test]
    fn source_mapped_delay_percent_controls_patch_only_their_existing_value_words() {
        assert_eq!(DELAY_SLOT, 0x0002_0005);
        let profiles: [(u32, [Option<DelayControl>; 4]); 14] = [
            (
                0x0202_0002,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Bass),
                    Some(DelayControl::Treble),
                ],
            ),
            (
                0x0202_0003,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::ModSpeed),
                    Some(DelayControl::Depth),
                ],
            ),
            (
                0x0202_0004,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Flutter),
                    Some(DelayControl::Drive),
                ],
            ),
            (
                0x0202_0005,
                [
                    None,
                    Some(DelayControl::Feedback),
                    None,
                    Some(DelayControl::Flutter),
                ],
            ),
            (
                0x0202_0006,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Speed),
                    Some(DelayControl::Depth),
                ],
            ),
            (
                0x0202_0007,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Bass),
                    Some(DelayControl::Treble),
                ],
            ),
            (
                0x0202_0008,
                [
                    None,
                    Some(DelayControl::Offset),
                    Some(DelayControl::FeedbackLeft),
                    Some(DelayControl::FeedbackRight),
                ],
            ),
            (
                0x0202_0009,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Offset),
                    Some(DelayControl::Spread),
                ],
            ),
            (
                0x0202_000a,
                [None, Some(DelayControl::Feedback), None, None],
            ),
            (
                0x0202_000c,
                [
                    None,
                    Some(DelayControl::Feedback),
                    None,
                    Some(DelayControl::Flutter),
                ],
            ),
            (
                0x0202_000b,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Bass),
                    Some(DelayControl::Treble),
                ],
            ),
            (
                0x0202_000d,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Tone),
                    None,
                ],
            ),
            (
                0x0202_000e,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::ModSpeed),
                    Some(DelayControl::Depth),
                ],
            ),
            (
                0x0202_000f,
                [
                    None,
                    Some(DelayControl::Feedback),
                    Some(DelayControl::Speed),
                    Some(DelayControl::Depth),
                ],
            ),
        ];
        assert_eq!(
            profiles.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            DELAY_CONTROL_MODEL_IDS
        );
        assert_eq!(
            profiles
                .iter()
                .map(|(_, row)| row.iter().flatten().count())
                .sum::<usize>(),
            37
        );
        for (model_id, layout) in profiles {
            for control in DelayControl::ALL {
                assert_eq!(
                    control.selector_id(model_id).is_some(),
                    layout.contains(&Some(control)),
                    "unexpected {} mapping on model {model_id:08x}",
                    control.label()
                );
            }
            let mut records = Vec::new();
            for control in layout.into_iter().flatten() {
                records.push(drive_record(
                    0x3f,
                    control.param_id(model_id).unwrap(),
                    1,
                    0x3f00_0000,
                ));
            }
            let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
            let bytes = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            for control in DelayControl::ALL {
                let Some(selector) = control.selector_id(model_id) else {
                    continue;
                };
                let position = selector - 0x0005_0002;
                assert_eq!(selector, 0x0005_0002 + position);
                assert_eq!(control.param_id(model_id), Some(0x0010_0000 + position));
                assert_eq!(tone.delay_control_fraction(control).unwrap(), 0.5);
                tone.set_delay_control_fraction(control, 0.317).unwrap();
                let record = drive_record(0x3f, 0x0010_0000 + position, 1, 0x3f00_0000);
                let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
                assert_eq!(
                    &tone.as_bytes()[offset..offset + 4],
                    &0.317f32.to_bits().to_be_bytes()
                );
            }
        }
    }

    #[test]
    fn delay_association_choices_encode_exact_float32_controller_keys() {
        let expected = [
            0xbb81_0204,
            0x3df9_f3e8,
            0x3e7d_fbf8,
            0x3ebf_7efe,
            0x3f00_0000,
            0x3f20_4081,
            0x3f40_8102,
            0x3f60_c183,
            0x3f7e_fdfc,
        ];
        let head_labels = [
            "12--", "1-3-", "1--4", "-23-", "123-", "12-4", "1-34", "-234", "1234",
        ];
        let bit_labels = ["12", "11", "10", "9", "8", "7", "6", "5", "4"];
        assert_eq!(DelayHeadsSelection::ALL.len(), expected.len());
        assert_eq!(DelayBitsSelection::ALL.len(), expected.len());
        for index in 0..expected.len() {
            let heads = DelayHeadsSelection::ALL[index];
            let bits = DelayBitsSelection::ALL[index];
            assert_eq!(heads.label(), head_labels[index]);
            assert_eq!(bits.label(), bit_labels[index]);
            assert_eq!(heads.float_bits(), expected[index]);
            assert_eq!(bits.float_bits(), expected[index]);
            assert!(f32::from_bits(expected[index]).is_finite());
            assert_eq!(
                DelayHeadsSelection::from_float_bits(expected[index]),
                Some(heads)
            );
            assert_eq!(
                DelayBitsSelection::from_float_bits(expected[index]),
                Some(bits)
            );
        }
        assert!(f32::from_bits(expected[0]).is_sign_negative());
    }

    #[test]
    fn delay_association_edits_are_typed_exact_last_match_value_word_patches() {
        let families = [
            (0x0202_0005, 0x0010_0002, 0x0005_0004),
            (0x0202_000d, 0x0010_0003, 0x0005_0005),
        ];
        for (model_id, association_id, selector_id) in families {
            assert_eq!(DELAY_SLOT, 0x0002_0005);
            let unrelated_id = if model_id == 0x0202_0005 {
                0x0010_0003
            } else {
                0x0010_0002
            };
            let first = drive_record(0x3f, association_id, 1, 0x3f00_0000);
            let unrelated = drive_record(0x3f, unrelated_id, 1, 0x3e80_0000);
            let last = drive_record(0x3f, association_id, 1, 0x3f20_4081);
            let original = delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                model_id,
                &[&first, &unrelated, &last],
            );
            let last_offset = original
                .windows(12)
                .rposition(|window| window == last)
                .unwrap();
            let first_offset = original
                .windows(12)
                .position(|window| window == first)
                .unwrap();
            let unrelated_offset = original
                .windows(12)
                .position(|window| window == unrelated)
                .unwrap();

            for index in 0..9 {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                if model_id == 0x0202_0005 {
                    assert_eq!(selector_id, 0x0005_0004);
                    assert_eq!(
                        tone.delay_heads_selection().unwrap(),
                        DelayAssociationValue::Known(DelayHeadsSelection::OneTwoMinusFour)
                    );
                    tone.set_delay_heads_selection(DelayHeadsSelection::ALL[index])
                        .unwrap();
                    assert_eq!(
                        tone.delay_heads_selection().unwrap(),
                        DelayAssociationValue::Known(DelayHeadsSelection::ALL[index])
                    );
                } else {
                    assert_eq!(selector_id, 0x0005_0005);
                    assert_eq!(
                        tone.delay_bits_selection().unwrap(),
                        DelayAssociationValue::Known(DelayBitsSelection::Seven)
                    );
                    tone.set_delay_bits_selection(DelayBitsSelection::ALL[index])
                        .unwrap();
                    assert_eq!(
                        tone.delay_bits_selection().unwrap(),
                        DelayAssociationValue::Known(DelayBitsSelection::ALL[index])
                    );
                }
                let changed: Vec<_> = original
                    .iter()
                    .zip(tone.as_bytes())
                    .enumerate()
                    .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                    .collect();
                let expected_bits = if model_id == 0x0202_0005 {
                    DelayHeadsSelection::ALL[index].float_bits()
                } else {
                    DelayBitsSelection::ALL[index].float_bits()
                };
                assert!(
                    changed
                        .iter()
                        .all(|offset| (last_offset + 8..last_offset + 12).contains(offset))
                );
                assert_eq!(
                    &tone.as_bytes()[last_offset + 8..last_offset + 12],
                    &expected_bits.to_be_bytes()
                );
                assert_eq!(
                    &tone.as_bytes()[first_offset..first_offset + 12],
                    &original[first_offset..first_offset + 12]
                );
                assert_eq!(
                    &tone.as_bytes()[unrelated_offset..unrelated_offset + 12],
                    &original[unrelated_offset..unrelated_offset + 12]
                );
                let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                if model_id == 0x0202_0005 {
                    assert_eq!(
                        reopened.delay_heads_selection().unwrap(),
                        DelayAssociationValue::Known(DelayHeadsSelection::ALL[index])
                    );
                } else {
                    assert_eq!(
                        reopened.delay_bits_selection().unwrap(),
                        DelayAssociationValue::Known(DelayBitsSelection::ALL[index])
                    );
                }
            }
        }
    }

    #[test]
    fn delay_associations_preserve_unknown_values_until_an_explicit_choice() {
        for (model_id, parameter_id, is_heads) in [
            (0x0202_0005, 0x0010_0002, true),
            (0x0202_000d, 0x0010_0003, false),
        ] {
            for raw_bits in [0x3f12_3456, 0x7fc0_0001, 0xbb81_0203] {
                let record = drive_record(0x3f, parameter_id, 1, raw_bits);
                let original = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
                let tone = PodTone::parse(original.clone()).unwrap();
                if is_heads {
                    assert_eq!(
                        tone.delay_heads_selection().unwrap(),
                        DelayAssociationValue::Unknown { raw_bits }
                    );
                } else {
                    assert_eq!(
                        tone.delay_bits_selection().unwrap(),
                        DelayAssociationValue::Unknown { raw_bits }
                    );
                }
                assert_eq!(tone.as_bytes(), original);
            }
        }
    }

    #[test]
    fn finite_unknown_delay_association_values_can_be_explicitly_replaced() {
        for (model_id, parameter_id, is_heads) in [
            (0x0202_0005, 0x0010_0002, true),
            (0x0202_000d, 0x0010_0003, false),
        ] {
            for raw_bits in [0x3f12_3456, 0xbb81_0203] {
                assert!(f32::from_bits(raw_bits).is_finite());
                let record = drive_record(0x3f, parameter_id, 1, raw_bits);
                let mut tone = PodTone::parse(delay_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    model_id,
                    &[&record],
                ))
                .unwrap();
                let result = if is_heads {
                    tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                } else {
                    tone.set_delay_bits_selection(DelayBitsSelection::Four)
                };
                assert!(result.is_ok());
                assert!(if is_heads {
                    matches!(
                        tone.delay_heads_selection().unwrap(),
                        DelayAssociationValue::Known(DelayHeadsSelection::OneTwoThreeFour)
                    )
                } else {
                    matches!(
                        tone.delay_bits_selection().unwrap(),
                        DelayAssociationValue::Known(DelayBitsSelection::Four)
                    )
                });
            }
        }
    }

    #[test]
    fn delay_association_setters_and_bundle_edits_reject_nonfinite_existing_values_transactionally()
    {
        for (model_id, parameter_id, is_heads) in [
            (0x0202_0005, 0x0010_0002, true),
            (0x0202_000d, 0x0010_0003, false),
        ] {
            for raw_bits in [0x7fc0_0001, 0x7f80_0000, 0xff80_0000] {
                let record = drive_record(0x3f, parameter_id, 1, raw_bits);
                let original_tone = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
                let mut tone = PodTone::parse(original_tone.clone()).unwrap();
                assert!(if is_heads {
                    matches!(tone.delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                } else {
                    matches!(tone.delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                });
                let result = if is_heads {
                    tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                } else {
                    tone.set_delay_bits_selection(DelayBitsSelection::Four)
                };
                assert!(result.is_err());
                assert_eq!(tone.as_bytes(), original_tone);
                assert!(if is_heads {
                    matches!(tone.delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                } else {
                    matches!(tone.delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                });

                let original_bundle = bundle_with_tone(&original_tone);
                let mut library = PodBundleLibrary::parse(original_bundle.clone()).unwrap();
                assert!(!library.is_dirty());
                let result = library.edit_entry(0, |tone| {
                    if is_heads {
                        tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                    } else {
                        tone.set_delay_bits_selection(DelayBitsSelection::Four)
                    }
                });
                assert!(result.is_err());
                assert_eq!(library.as_bytes(), original_bundle);
                assert!(!library.is_dirty());
                assert!(if is_heads {
                    matches!(library.tone(0, 0).unwrap().delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                } else {
                    matches!(library.tone(0, 0).unwrap().delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
                });
            }
        }
    }

    #[test]
    fn delay_association_admission_rejects_wrong_target_layout_and_record_before_mutation() {
        let heads_record = drive_record(0x3f, 0x0010_0002, 1, 0x3f00_0000);
        let bits_record = drive_record(0x3f, 0x0010_0003, 1, 0x3f00_0000);
        let mut wrong_product =
            delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[&heads_record]);
        let pinf = find_chunk_data(&wrong_product, b"PINF");
        wrong_product[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0009u32.to_be_bytes());

        let wrong_slot = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0x0202_0005,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters: &[&heads_record],
            }],
        );
        let opaque = [1, 2, 3, 4];
        let heads_bad_cases = [
            delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0006, &[&heads_record]),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                0x0202_0005,
                &[&heads_record],
            ),
            delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[]),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_0005,
                &[&opaque, &heads_record],
            ),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_0005,
                &[&drive_record(0x7e, 0x0010_0002, 1, 0x3f00_0000)],
            ),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_0005,
                &[&drive_record(0x3f, 0x0010_0002, 0, 0x3f00_0000)],
            ),
            wrong_slot,
        ];
        assert!(matches!(
            PodTone::parse(wrong_product),
            Err(ToneError::WrongTarget { .. })
        ));
        for bytes in heads_bad_cases {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.delay_heads_selection().is_err());
            assert!(
                tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                    .is_err()
            );
            assert!(
                tone.set_delay_bits_selection(DelayBitsSelection::Four)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), bytes);
        }
        let bits_bad_cases = [
            delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_000c, &[&bits_record]),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                0x0202_000d,
                &[&bits_record],
            ),
            delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_000d, &[]),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_000d,
                &[&opaque, &bits_record],
            ),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_000d,
                &[&drive_record(0x7e, 0x0010_0003, 1, 0x3f00_0000)],
            ),
            delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                0x0202_000d,
                &[&drive_record(0x3f, 0x0010_0003, 0, 0x3f00_0000)],
            ),
        ];
        for bytes in bits_bad_cases {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.delay_bits_selection().is_err());
            assert!(
                tone.set_delay_bits_selection(DelayBitsSelection::Four)
                    .is_err()
            );
            assert!(tone.as_bytes() == bytes);
        }
    }

    #[test]
    fn delay_association_library_edit_save_collision_revert_and_reopen_are_transactional() {
        let parameter = drive_record(0x3f, 0x0010_0002, 1, 0xbb81_0204);
        let original_tone = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[&parameter]);
        let original_bundle = bundle_with_tone(&original_tone);
        let mut library = PodBundleLibrary::parse(original_bundle.clone()).unwrap();
        assert!(!library.is_dirty());
        library
            .edit_entry(0, |tone| {
                tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
            })
            .unwrap();
        assert!(library.is_dirty());
        let collision_dir = tempfile::tempdir().unwrap();
        let collision_path = collision_dir.path().join("delay.l6b");
        std::fs::write(&collision_path, b"keep destination").unwrap();
        assert!(library.save_copy(&collision_path).is_err());
        assert_eq!(std::fs::read(&collision_path).unwrap(), b"keep destination");
        library.revert().unwrap();
        assert_eq!(library.as_bytes(), original_bundle);
        assert!(!library.is_dirty());

        library
            .edit_entry(0, |tone| {
                tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
            })
            .unwrap();
        let save_dir = tempfile::tempdir().unwrap();
        let save_path = save_dir.path().join("delay.l6b");
        library.save_copy(&save_path).unwrap();
        let reopened = PodBundleLibrary::parse(std::fs::read(save_path).unwrap()).unwrap();
        assert_eq!(
            reopened
                .tone(0, 0)
                .unwrap()
                .delay_heads_selection()
                .unwrap(),
            DelayAssociationValue::Known(DelayHeadsSelection::OneTwoThreeFour)
        );
        library.note_saved();
        assert!(!library.is_dirty());
    }

    #[test]
    fn mapped_effect_family_edits_have_independent_ids_full_file_preservation_and_saved_reparse() {
        let cases = [
            // Line6Edit positions are one-based here; these explicit expected
            // selector/PARM pairs are kept separate from the Rust lookup tables.
            (
                MappedEffect::Mod,
                MOD_SLOT,
                0x0203_0004,
                0x0004_0003,
                0x0010_0001,
            ),
            // Reverb's provider selector order is not the semantic PARM order.
            (
                MappedEffect::Reverb,
                REVERB_SLOT,
                0x0204_0002,
                0x0008_0001,
                0x0010_0000,
            ),
            (
                MappedEffect::Delay,
                DELAY_SLOT,
                0x0202_0002,
                0x0005_0003,
                0x0010_0001,
            ),
        ];

        for (effect, slot, model_id, expected_selector, expected_parameter) in cases {
            match effect {
                MappedEffect::Mod => {
                    assert_eq!(
                        ModControl::Depth.selector_id(model_id),
                        Some(expected_selector)
                    );
                    assert_eq!(
                        ModControl::Depth.param_id(model_id),
                        Some(expected_parameter)
                    );
                }
                MappedEffect::Reverb => {
                    assert_eq!(
                        ReverbControl::Dwell.selector_id(model_id),
                        Some(expected_selector)
                    );
                    assert_eq!(
                        ReverbControl::Dwell.param_id(model_id),
                        Some(expected_parameter)
                    );
                }
                MappedEffect::Delay => {
                    assert_eq!(
                        DelayControl::Feedback.selector_id(model_id),
                        Some(expected_selector)
                    );
                    assert_eq!(
                        DelayControl::Feedback.param_id(model_id),
                        Some(expected_parameter)
                    );
                }
            }

            let earlier = drive_record(0x3f, expected_parameter, 1, 0x3e80_0000);
            let selected = drive_record(0x3f, expected_parameter, 1, 0x3f00_0000);
            let unrelated = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
            let other_model_parameter = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3e00_0000);
            let target_parameters: [&[u8]; 3] = [&earlier, &unrelated, &selected];
            let other_parameters: [&[u8]; 1] = [&other_model_parameter];
            let original = custom_tone(
                ContainerKind::L6as,
                &[
                    BuiltModel {
                        slot,
                        model_id,
                        tail: &[0x91, 0x82, 0x73, 0x64],
                        parameters: &target_parameters,
                    },
                    BuiltModel {
                        slot: AMP_SLOT,
                        model_id: PLEXI_LEAD_100_MODEL,
                        tail: &[0x19, 0x28, 0x37, 0x46],
                        parameters: &other_parameters,
                    },
                ],
            );
            let selected_offset = original
                .windows(selected.len())
                .rposition(|window| window == selected)
                .unwrap()
                + 8;
            let mut expected = original.clone();
            expected[selected_offset..selected_offset + 4]
                .copy_from_slice(&0x3e00_0000u32.to_be_bytes());

            let mut library = Library::default();
            let index = library.import_bytes(original.clone()).unwrap();
            match effect {
                MappedEffect::Mod => library
                    .set_mod_control_fraction(index, ModControl::Depth, 0.125)
                    .unwrap(),
                MappedEffect::Reverb => library
                    .set_reverb_control_fraction(index, ReverbControl::Dwell, 0.125)
                    .unwrap(),
                MappedEffect::Delay => library
                    .set_delay_control_fraction(index, DelayControl::Feedback, 0.125)
                    .unwrap(),
            }
            let tone = library.tone(index).unwrap();
            assert_eq!(tone.as_bytes(), expected);
            assert_eq!(
                mapped_effect_value(tone, effect).unwrap().to_bits(),
                0x3e00_0000
            );
            assert!(library.is_dirty(index));

            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("mapped-effect.l6t");
            tone.save_copy(&path).unwrap();
            let saved = std::fs::read(&path).unwrap();
            assert_eq!(saved, expected);
            let reopened = PodTone::parse(saved).unwrap();
            assert_eq!(
                mapped_effect_value(&reopened, effect).unwrap().to_bits(),
                0x3e00_0000
            );
            assert_eq!(reopened.as_bytes(), expected);
            assert_eq!(
                &reopened.as_bytes()[selected_offset..selected_offset + 4],
                &0x3e00_0000u32.to_be_bytes()
            );
            assert!(
                reopened
                    .as_bytes()
                    .windows(earlier.len())
                    .any(|w| w == earlier)
            );
            assert!(
                reopened
                    .as_bytes()
                    .windows(unrelated.len())
                    .any(|w| w == unrelated)
            );
            assert!(
                reopened
                    .as_bytes()
                    .windows(other_model_parameter.len())
                    .any(|w| w == other_model_parameter)
            );
        }
    }

    #[test]
    fn each_mapped_effect_family_rejects_bad_targets_records_and_values_transactionally() {
        let cases = [
            (MappedEffect::Mod, MOD_SLOT, 0x0203_0004, 0x0010_0001),
            (MappedEffect::Reverb, REVERB_SLOT, 0x0204_0002, 0x0010_0000),
            (MappedEffect::Delay, DELAY_SLOT, 0x0202_0002, 0x0010_0001),
        ];
        let opaque = [1, 2, 3, 4];

        for (effect, slot, model_id, parameter_id) in cases {
            let valid = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
            let bad_prefix = drive_record(0x7e, parameter_id, 1, 0x3f00_0000);
            let bad_type = drive_record(0x3f, parameter_id, 0, 0x3f00_0000);
            let missing = mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[]);
            let opaque_tone =
                mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&opaque]);
            let wrong_prefix =
                mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&bad_prefix]);
            let wrong_type =
                mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&bad_type]);
            let wrong_version =
                mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION - 1, &[&valid]);
            let unsupported_slot =
                mapped_effect_tone(slot + 1, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
            let unsupported_model = mapped_effect_tone(
                slot,
                model_id + 0x1000,
                PODXT_LIVE_HOST_MIN_VERSION,
                &[&valid],
            );
            let nonfinite_current = drive_record(0x3f, parameter_id, 1, f32::NAN.to_bits());
            let out_of_range_current = drive_record(0x3f, parameter_id, 1, 0x3f80_0001);
            let invalid_floats = [
                f32::from_bits(0x8000_0001),
                f32::from_bits(0x3f80_0001),
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ];

            for bytes in [
                missing,
                opaque_tone,
                wrong_prefix,
                wrong_type,
                wrong_version,
                unsupported_slot,
                unsupported_model,
                mapped_effect_tone(
                    slot,
                    model_id,
                    PODXT_LIVE_HOST_MIN_VERSION,
                    &[&nonfinite_current],
                ),
                mapped_effect_tone(
                    slot,
                    model_id,
                    PODXT_LIVE_HOST_MIN_VERSION,
                    &[&out_of_range_current],
                ),
            ] {
                let original = bytes.clone();
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(set_mapped_effect(&mut tone, effect, false, 0.25).is_err());
                assert_eq!(tone.as_bytes(), original);
                assert!(set_mapped_effect(&mut tone, effect, true, 0.25).is_err());
                assert_eq!(tone.as_bytes(), original);
            }

            for invalid in invalid_floats {
                let bytes =
                    mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(set_mapped_effect(&mut tone, effect, false, invalid).is_err());
                assert_eq!(tone.as_bytes(), bytes);
            }

            let mut wrong_target =
                mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
            let pinf = find_chunk_data(&wrong_target, b"PINF");
            wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0002u32.to_be_bytes());
            let original = wrong_target.clone();
            assert!(PodTone::parse(wrong_target.clone()).is_err());
            assert_eq!(wrong_target, original);
        }
    }

    #[test]
    fn stomp_controls_reject_unsupported_targets_records_and_values_transactionally() {
        let drive = drive_record(0x3f, STOMP_DRIVE_PARAM, 1, 0x3f00_0000);
        let wrong_prefix = drive_record(0x7e, STOMP_DRIVE_PARAM, 1, 0x3f00_0000);
        let wrong_type = drive_record(0x3f, STOMP_DRIVE_PARAM, 0, 0x3f00_0000);
        let mut wrong_product =
            stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, STOMP_DRIVE_MODEL_ID, &[&drive]);
        let pinf = find_chunk_data(&wrong_product, b"PINF");
        wrong_product[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0002u32.to_be_bytes());
        let wrong_slot = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: STOMP_SLOT + 1,
                model_id: STOMP_DRIVE_MODEL_ID,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters: &[&drive],
            }],
        );
        let invalid_targets = [
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                STOMP_DRIVE_MODEL_ID,
                &[&drive],
            ),
            stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0205_000a, &[&drive]),
            stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, STOMP_DRIVE_MODEL_ID, &[]),
            wrong_slot,
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_DRIVE_MODEL_ID,
                &[&wrong_prefix],
            ),
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_DRIVE_MODEL_ID,
                &[&wrong_type],
            ),
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_DRIVE_MODEL_ID,
                &[&[1, 2, 3, 4], &drive],
            ),
        ];
        assert!(PodTone::parse(wrong_product).is_err());
        for model_id in STOMP_CONTROL_MODEL_IDS {
            for control in StompControl::ALL {
                let Some(parameter_id) = control.param_id(model_id) else {
                    continue;
                };
                let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
                let wrong_prefix = drive_record(0x7e, parameter_id, 1, 0x3f00_0000);
                let wrong_type = drive_record(0x3f, parameter_id, 0, 0x3f00_0000);
                let invalid_model =
                    stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0205_000a, &[&record]);
                let fixtures = invalid_targets.iter().cloned().chain([
                    invalid_model,
                    stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&wrong_prefix]),
                    stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&wrong_type]),
                    stomp_tone(
                        PODXT_LIVE_HOST_MIN_VERSION,
                        model_id,
                        &[&[1, 2, 3, 4], &record],
                    ),
                ]);
                for bytes in fixtures {
                    let mut tone = PodTone::parse(bytes.clone()).unwrap();
                    assert!(tone.stomp_control_fraction(control).is_err());
                    assert!(tone.set_stomp_control_fraction(control, 0.25).is_err());
                    assert_eq!(tone.as_bytes(), bytes);
                }
                for invalid in [
                    f32::from_bits(0x8000_0001),
                    f32::from_bits(0x3f80_0001),
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                ] {
                    let bytes = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
                    let mut tone = PodTone::parse(bytes.clone()).unwrap();
                    assert!(tone.set_stomp_control_fraction(control, invalid).is_err());
                    assert_eq!(tone.as_bytes(), bytes);
                }
                let nonfinite = drive_record(0x3f, parameter_id, 1, f32::NAN.to_bits());
                let bytes = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&nonfinite]);
                assert!(
                    PodTone::parse(bytes)
                        .unwrap()
                        .stomp_control_fraction(control)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn stomp_control_library_revert_copy_collision_and_reopen_keep_safe_copy_rules() {
        for model_id in STOMP_CONTROL_MODEL_IDS {
            let records: Vec<_> = StompControl::ALL
                .into_iter()
                .filter_map(|control| {
                    control
                        .param_id(model_id)
                        .map(|parameter_id| drive_record(0x3f, parameter_id, 1, 0x3f00_0000))
                })
                .collect();
            let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
            let original = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &refs);
            for control in StompControl::ALL {
                if control.param_id(model_id).is_none() {
                    continue;
                }
                let mut library = Library::default();
                let index = library.import_bytes(original.clone()).unwrap();
                library
                    .set_stomp_control_fraction(index, control, 0.625)
                    .unwrap();
                assert!(library.is_dirty(index), "{}", control.label());
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("stomp.l6t");
                std::fs::write(&path, b"keep existing destination").unwrap();
                assert!(library.tone(index).unwrap().save_copy(&path).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), b"keep existing destination");
                library.revert_unsaved(index).unwrap();
                assert_eq!(library.tone(index).unwrap().as_bytes(), original);
                library
                    .set_stomp_control_fraction(index, control, 0.625)
                    .unwrap();
                std::fs::remove_file(&path).unwrap();
                library.tone(index).unwrap().save_copy(&path).unwrap();
                let reopened = PodTone::parse(std::fs::read(path).unwrap()).unwrap();
                assert_eq!(reopened.stomp_control_fraction(control).unwrap(), 0.625);
                assert!(library.is_dirty(index));
                library.note_saved(index).unwrap();
                assert!(!library.is_dirty(index));
            }
        }
    }

    #[test]
    fn amp_control_rejections_are_transactional_for_domain_and_target_failures() {
        for control in AmpControl::ALL {
            let record = drive_record(0x3f, control.param_id(), 1, 0x3f00_0000);
            for invalid in [
                f32::from_bits(0x8000_0001),
                f32::from_bits(0x3f80_0001),
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ] {
                let bytes = drive_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    PLEXI_LEAD_100_MODEL,
                    &[&record],
                );
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(tone.set_amp_control_fraction(control, invalid).is_err());
                assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
            }
            let fixtures = [
                drive_tone(
                    PODXT_LIVE_HOST_MIN_VERSION - 1,
                    PLEXI_LEAD_100_MODEL,
                    &[&record],
                ),
                drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[]),
                drive_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0002_0000, &[&record]),
                drive_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    STOMP_DRIVE_MODEL_ID,
                    &[&record],
                ),
                drive_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0002_0050, &[&record]),
            ];
            for bytes in fixtures {
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
                assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
            }
            let wrong_prefix = drive_record(0x7e, control.param_id(), 1, 0x3f00_0000);
            let wrong_type = drive_record(0x3f, control.param_id(), 0, 0x3f00_0000);
            for parm in [&wrong_prefix[..], &wrong_type[..], &[1, 2, 3, 4][..]] {
                let bytes = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[parm]);
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
                assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
            }
            let nonfinite = drive_record(0x3f, control.param_id(), 1, f32::NAN.to_bits());
            let bytes = drive_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PLEXI_LEAD_100_MODEL,
                &[&nonfinite],
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
            assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
        }
    }

    #[test]
    fn amp_control_library_edit_revert_save_copy_and_collision_keep_safe_copy_rules() {
        let records: Vec<_> = AmpControl::ALL
            .into_iter()
            .map(|control| drive_record(0x3f, control.param_id(), 1, 0x3f00_0000))
            .collect();
        let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
        let original = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &refs);
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        for (number, control) in AmpControl::ALL.into_iter().enumerate() {
            library
                .set_amp_control_fraction(index, control, 0.125 + number as f32 * 0.125)
                .unwrap();
        }
        assert!(library.is_dirty(index));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drive.l6t");
        std::fs::write(&path, b"pre-existing bytes").unwrap();
        assert!(library.tone(index).unwrap().save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"pre-existing bytes");
        assert!(library.is_dirty(index));
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        for (number, control) in AmpControl::ALL.into_iter().enumerate() {
            library
                .set_amp_control_fraction(index, control, 0.125 + number as f32 * 0.125)
                .unwrap();
        }
        std::fs::remove_file(&path).unwrap();
        library.tone(index).unwrap().save_copy(&path).unwrap();
        assert!(library.tone(index).unwrap().save_copy(&path).is_err());
        let saved = std::fs::read(path).unwrap();
        let reopened = PodTone::parse(saved).unwrap();
        for (number, control) in AmpControl::ALL.into_iter().enumerate() {
            assert_eq!(
                reopened.amp_control_fraction(control).unwrap().to_bits(),
                (0.125 + number as f32 * 0.125).to_bits(),
                "{}",
                control.label()
            );
        }
        assert!(library.is_dirty(index));
        library.note_saved(index).unwrap();
        assert!(!library.is_dirty(index));
    }

    fn live_tone() -> Vec<u8> {
        fixture(
            ContainerKind::L6as,
            PODXT_LIVE_TONE_ID,
            0x0011_2233,
            &[
                (AMP_SLOT, 0x1111_0002),
                (CAB_SLOT, 0x2222_0003),
                (0x0003_00aa, 0x3333_0004),
            ],
        )
    }

    #[test]
    fn tone_id_matches_the_archived_product_id_and_not_the_midi_hardware_id() {
        assert_eq!(PODXT_LIVE_TONE_ID, crate::firmware::PODXT_LIVE_PRODUCT_ID);
        assert_eq!(PODXT_LIVE_TONE_ID, 196_618);
        assert_ne!(PODXT_LIVE_TONE_ID, 18_000);
        const {
            assert!(TONE_BYTES_CEILING == 1_048_576);
        }
    }

    #[test]
    fn l6as_exposes_raw_amp_and_cab_and_preserves_unknown_bytes() {
        let original = live_tone();
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.as_bytes(), original);
        assert_eq!(tone.container(), ContainerKind::L6as);
        assert_eq!(tone.device_version(), 0x0011_2233);
        assert_ne!(tone.device_version(), crate::firmware::PODXT_LIVE_VERSION);
        assert_eq!(tone.amp_model_id(), Some(0x1111_0002));
        assert_eq!(tone.cab_model_id(), Some(0x2222_0003));
        assert_eq!(tone.amp_asset_name(), None);
        assert_eq!(tone.cab_asset_name(), None);
        assert_eq!(tone.undecoded_slots(), 1);
        assert_eq!(tone.recorded_parameter_chunks(), 3);
        assert_eq!(tone.undecoded_parameter_chunks(), 3);
        assert_eq!(tone.models()[0].ordinal, Some(0x10));
        assert_eq!(tone.models()[0].reserved, Some([0x20, 0x30]));
        assert_eq!(tone.models()[0].enabled, Some(true));
        assert!(matches!(
            tone.models()[0].parameters[0],
            ModelParameter::Opaque { len: 4 }
        ));
        assert!(matches!(
            tone.models()[0].parameters[1],
            ModelParameter::Recorded {
                prefix: 0xab,
                id: 0x00cd_ef01,
                value: ParameterValue::Other {
                    type_word: 0x0203_0405,
                    bits: 0x0607_0809,
                },
            }
        ));
        assert_eq!(tone.name(), "A");
        tone.rename("Stage").unwrap();
        let reread = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
        assert_eq!(reread.name(), "Stage");
        assert_eq!(reread.amp_model_id(), Some(0x1111_0002));
        assert_eq!(reread.cab_model_id(), Some(0x2222_0003));
        let name_at = tone_name_offset(&original);
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x11, 0x22, 0x33, 0x44])
        );
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0xab, 0xcd, 0xef, 0x01])
        );
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x5a, 0x5a, 0x5a, 0x5a])
        );
        assert!(tone.as_bytes().windows(3).any(|bytes| bytes == [9, 8, 7]));
    }

    #[test]
    fn cab_slot_on_l6pa_and_l6fs_is_not_the_cab_model() {
        for kind in [ContainerKind::L6pa, ContainerKind::L6fs] {
            let tone = PodTone::parse(fixture(
                kind,
                PODXT_LIVE_TONE_ID,
                0,
                &[(AMP_SLOT, 7), (CAB_SLOT, 8)],
            ))
            .unwrap();
            assert_eq!(tone.container(), kind);
            assert_eq!(tone.amp_model_id(), Some(7));
            assert_eq!(tone.cab_model_id(), None);
            assert_eq!(tone.cab_asset_name(), None);
            assert_eq!(tone.undecoded_slots(), 0);
            assert_eq!(
                SlotRole::from_id(tone.models()[1].slot),
                Some(SlotRole::Cab)
            );
            assert_eq!(tone.models()[1].slot, CAB_SLOT);
            assert_eq!(tone.models()[1].model_id, 8);
        }
    }

    #[test]
    fn missing_amp_is_absent_and_other_products_are_rejected() {
        let empty =
            PodTone::parse(fixture(ContainerKind::L6pa, PODXT_LIVE_TONE_ID, 0, &[])).unwrap();
        assert_eq!(empty.amp_model_id(), None);
        assert_eq!(empty.cab_model_id(), None);
        assert_eq!(empty.undecoded_slots(), 0);
        for device_id in [
            0x0007_0004u32,
            0x0007_0000,
            0x0003_000b,
            0x0003_0002,
            196_608,
        ] {
            let error =
                PodTone::parse(fixture(ContainerKind::L6pa, device_id, 0, &[(AMP_SLOT, 1)]))
                    .unwrap_err();
            assert_eq!(error, ToneError::WrongTarget { device_id });
            assert!(!error.detail().contains("decoded"));
        }
    }

    #[test]
    fn rejects_banks_bad_versions_duplicates_and_truncation() {
        let source = live_tone();
        let mut bank = source.clone();
        bank[8..12].copy_from_slice(b"L6BA");
        assert!(PodTone::parse(bank).is_err());
        let mut version = source.clone();
        let info = find_chunk_data(&version, b"PINF");
        version[info..info + 4].copy_from_slice(&2u32.to_be_bytes());
        assert!(PodTone::parse(version).is_err());
        let mut duplicated = fixture(
            ContainerKind::L6as,
            PODXT_LIVE_TONE_ID,
            1,
            &[(AMP_SLOT, 1), (AMP_SLOT, 2)],
        );
        assert!(
            PodTone::parse(duplicated.clone())
                .unwrap_err()
                .detail()
                .contains("duplicate")
        );
        for end in 0..12 {
            assert!(PodTone::parse(source[..end].to_vec()).is_err());
        }
        duplicated.truncate(20);
        duplicated[4..8].copy_from_slice(&12u32.to_be_bytes());
        assert!(PodTone::parse(duplicated).is_err());
        assert!(PodTone::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
    }

    #[test]
    fn recovered_controls_keep_unknown_bytes_and_do_not_edit_parameters() {
        let original = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 131_073,
                    tail: &[0x04, 0x00, 0x00, 0x01, 0xaa, 0xbb, 0xcc, 0xdd],
                    parameters: &[
                        &[
                            0x3f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x3f, 0x80, 0x00, 0x00,
                        ],
                        &[0x3f, 0x00, 0x00, 0x01],
                        &[
                            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                            0xde, 0xef, 0x01, 0x02,
                        ],
                    ],
                },
                BuiltModel {
                    slot: CAB_SLOT,
                    model_id: 0x0100_000f,
                    tail: &[0x02, 0x7f, 0x7e, 0x00],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 131_074,
                    model_id: 0x0205_0000,
                    tail: &[0x01, 0x10, 0x20, 0x03],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 0x0003_00aa,
                    model_id: 0x3333_0004,
                    tail: &[],
                    parameters: &[],
                },
            ],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.amp_model_id(), Some(131_073));
        assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
        assert_eq!(tone.cab_model_id(), Some(0x0100_000f));
        assert_eq!(tone.cab_asset_name(), Some("No Cabinet"));
        assert_eq!(tone.undecoded_slots(), 1);
        assert_eq!(tone.recorded_parameter_chunks(), 1);
        assert_eq!(tone.undecoded_parameter_chunks(), 2);
        let amp = &tone.models()[0];
        assert_eq!(amp.ordinal, Some(0x04));
        assert_eq!(amp.reserved, Some([0x00, 0x00]));
        assert_eq!(amp.enabled, Some(true));
        assert_eq!(
            amp.parameters[0],
            ModelParameter::Recorded {
                prefix: 0x3f,
                id: 7,
                value: ParameterValue::Float { bits: 0x3f80_0000 },
            }
        );
        assert_eq!(amp.parameters[1], ModelParameter::Opaque { len: 4 });
        assert_eq!(amp.parameters[2], ModelParameter::Opaque { len: 16 });
        let cab = &tone.models()[1];
        assert_eq!(cab.enabled, Some(false));
        assert_eq!(cab.reserved, Some([0x7f, 0x7e]));
        let wah = &tone.models()[2];
        assert_eq!(SlotRole::from_id(wah.slot), Some(SlotRole::Wah));
        assert_eq!(pod_assets::asset_name(wah.model_id), Some("Facial Fuzz"));
        assert_eq!(wah.ordinal, Some(1));
        assert_eq!(wah.enabled, Some(true));
        let unknown = &tone.models()[3];
        assert_eq!(unknown.ordinal, None);
        assert_eq!(unknown.reserved, None);
        assert_eq!(unknown.enabled, None);
        assert_eq!(SlotRole::from_id(unknown.slot), None);
        let lines = tone.control_lines();
        assert!(lines[0].contains("Amp slot 00030000 model Line 6 Clean 00020001"));
        assert!(lines[0].contains(" enabled"));
        assert!(!lines[0].contains("disabled"));
        assert!(lines[0].contains("ordinal 4"));
        assert!(lines[0].contains("reserved 00 00"));
        assert!(lines[1].contains("parameter 000007 prefix 3f float 3f800000 no range"));
        assert!(lines[2].contains("opaque parameter 4 bytes"));
        assert!(lines[3].contains("opaque parameter 16 bytes"));
        assert!(lines[4].contains("Cab slot 00030001 model No Cabinet 0100000f"));
        assert!(lines[4].contains("disabled"));
        assert!(lines[5].contains("Wah slot 00020002 model Facial Fuzz 02050000"));
        assert!(lines[6].contains("no GPXT role slot 000300aa model 33330004"));
        assert!(!lines.iter().any(|line| line.contains("bypass")));
        tone.rename("Named").unwrap();
        assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
        assert_eq!(
            tone.models()[0].parameters,
            amp_parameters_after_rename(&original)
        );
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            let name_at = tone_name_offset(&original);
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0xaa, 0xbb, 0xcc, 0xdd])
        );
        let closed = PodTone::parse(custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0x0100_000f,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            }],
        ))
        .unwrap();
        assert_eq!(closed.cab_model_id(), None);
        assert_eq!(
            pod_assets::asset_name(closed.models()[0].model_id),
            Some("No Cabinet")
        );
        let partial = PodTone::parse(custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 1,
                tail: &[0x11, 0x22, 0x33],
                parameters: &[],
            }],
        ))
        .unwrap();
        assert_eq!(partial.models()[0].ordinal, None);
        assert_eq!(partial.models()[0].reserved, None);
        assert_eq!(partial.models()[0].enabled, None);
        assert!(
            partial
                .as_bytes()
                .windows(3)
                .any(|bytes| bytes == [0x11, 0x22, 0x33])
        );
    }

    #[test]
    fn signed_minf_fields_follow_java_read8_and_rename_keeps_the_bytes() {
        // Java `byte` values of the file bytes 0x00, 0x01, 0x7F, 0x80, and 0xFF.
        // Both the ordinal and the enabled byte use each value.
        const TAILS: [[u8; 4]; 5] = [
            [0x00, 0x5a, 0xa5, 0x00],
            [0x01, 0x5a, 0xa5, 0x01],
            [0x7f, 0x5a, 0xa5, 0x7f],
            [0x80, 0x5a, 0xa5, 0x80],
            [0xff, 0x5a, 0xa5, 0xff],
        ];
        let expected: [(u32, i8, bool); 5] = [
            (65_536, 0, false),
            (131_072, 1, true),
            (131_073, 127, true),
            (131_074, -128, false),
            (AMP_SLOT, -1, false),
        ];
        let models: Vec<BuiltModel<'_>> = expected
            .iter()
            .enumerate()
            .map(|(index, &(slot, _, _))| BuiltModel {
                slot,
                model_id: 0x1111_0002,
                tail: &TAILS[index],
                parameters: &[],
            })
            .collect();
        let original = custom_tone(ContainerKind::L6as, &models);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let lines = tone.control_lines();
        assert_eq!(lines.len(), expected.len());
        for (index, &(slot, ordinal, enabled)) in expected.iter().enumerate() {
            let model = &tone.models()[index];
            assert_eq!(model.slot, slot);
            assert_eq!(model.ordinal, Some(ordinal));
            assert_eq!(model.reserved, Some([0x5a, 0xa5]));
            assert_eq!(model.enabled, Some(enabled));
            assert!(lines[index].ends_with(&format!("ordinal {ordinal} reserved 5a a5")));
            if enabled {
                assert!(lines[index].contains(" enabled"));
                assert!(!lines[index].contains("disabled"));
            } else {
                assert!(lines[index].contains(" disabled"));
            }
        }
        tone.rename("Signed").unwrap();
        assert_eq!(tone.name(), "Signed");
        let name_at = tone_name_offset(&original);
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        for tail in TAILS {
            assert!(
                tone.as_bytes().windows(4).any(|bytes| bytes == tail),
                "MINF tail {tail:02x?} was not preserved"
            );
        }
    }

    fn amp_parameters_after_rename(original: &[u8]) -> Vec<ModelParameter> {
        PodTone::parse(original.to_vec()).unwrap().models()[0]
            .parameters
            .clone()
    }

    #[test]
    fn library_rename_and_save_do_not_overwrite_or_cross_tones() {
        let mut library = Library::default();
        let first = library.import_bytes(live_tone()).unwrap();
        let second = library
            .import_bytes(fixture(
                ContainerKind::L6pa,
                PODXT_LIVE_TONE_ID,
                4,
                &[(AMP_SLOT, 9)],
            ))
            .unwrap();
        let mut edited = library.tone(first).unwrap().clone();
        edited.rename("Guitare 🎸").unwrap();
        library.replace(first, edited.into_bytes()).unwrap();
        assert!(library.is_dirty(first));
        assert!(!library.is_dirty(second));
        assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
        assert_eq!(library.tone(second).unwrap().amp_model_id(), Some(9));
        assert_eq!(library.tone(second).unwrap().name(), "A");
        assert!(edited_name_too_long(library.tone(first).unwrap()));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("copy.tone");
        library.tone(first).unwrap().save_copy(&path).unwrap();
        assert!(library.is_dirty(first));
        library.note_saved(first).unwrap();
        assert!(!library.is_dirty(first));
        assert!(!library.is_dirty(second));
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(saved, library.tone(first).unwrap().as_bytes());
        assert!(library.tone(first).unwrap().save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        let mut other = library.tone(second).unwrap().clone();
        other.rename("Other").unwrap();
        library.replace(second, other.into_bytes()).unwrap();
        assert!(!library.is_dirty(first));
        assert!(library.is_dirty(second));
        library.revert_unsaved(second).unwrap();
        assert_eq!(library.tone(second).unwrap().name(), "A");
        assert!(!library.is_dirty(second));
        assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
        assert!(library.replace(7, live_tone()).is_err());
        assert!(library.note_saved(7).is_err());
        assert!(library.revert_unsaved(7).is_err());
    }

    #[test]
    fn pod_bundle_library_admits_each_standalone_patch_form() {
        for kind in [
            ContainerKind::L6pa,
            ContainerKind::L6fs,
            ContainerKind::L6as,
        ] {
            let mut tone = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[]);
            tone[8..12].copy_from_slice(kind.tag().as_bytes());
            let bundle = bundle_with_tone(&tone);
            let mut library = PodBundleLibrary::parse(bundle).unwrap();
            assert_eq!(library.tone(0, 0).unwrap().container(), kind);
            library
                .edit_tone(0, 0, |selected| selected.rename("Renamed"))
                .unwrap();
            let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
            assert_eq!(reopened.tone(0, 0).unwrap().container(), kind);
            assert_eq!(reopened.tone(0, 0).unwrap().name(), "Renamed");
        }
    }

    #[test]
    fn pod_bundle_library_edits_reverts_and_rejects_mixed_target_collections() {
        let first = drive_record(0x3f, 0x0010_0003, 1, 0x3e00_0000);
        let last = drive_record(0x3f, 0x0010_0003, 1, 0x3e80_0000);
        let opaque = [0xde, 0xad, 0xbe];
        let tone = custom_tone(
            ContainerKind::L6fs,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: PLEXI_LEAD_100_MODEL,
                    tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                    parameters: &[&first, &last],
                },
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x0200_0002,
                    tail: &[0x11, 0x22, 0x33, 0x44],
                    parameters: &[&opaque],
                },
            ],
        );
        let ambiguous = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: PLEXI_LEAD_100_MODEL,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters: &[&first, &opaque],
            }],
        );
        let third_parm = drive_record(0x3f, 0x0010_0003, 1, 0x3e00_0000);
        let unselected = drive_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PLEXI_LEAD_100_MODEL,
            &[&third_parm],
        );
        let original = bundle_with_banks(&[
            vec![tone.clone(), ambiguous.clone()],
            vec![unselected.clone()],
        ]);
        let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
        assert_eq!(library.bank_count(), 2);
        assert_eq!(library.preset_count(0), Some(2));
        assert_eq!(library.preset_count(1), Some(1));
        assert_eq!(library.bank_name(0), Some("A"));
        assert_eq!(library.bank_name(1), Some("B"));
        assert_eq!(library.tone(0, 0).unwrap().as_bytes(), tone);
        assert!(!library.is_dirty());

        let mut expected = original.clone();
        let tone_start = expected
            .windows(tone.len())
            .position(|window| window == tone)
            .unwrap();
        let parameter_positions: Vec<_> = tone
            .windows(last.len())
            .enumerate()
            .filter_map(|(index, window)| (window == last).then_some(index))
            .collect();
        let value_offset = parameter_positions.last().copied().unwrap() + 8;
        expected[tone_start + value_offset..tone_start + value_offset + 4]
            .copy_from_slice(&0x3f00_0000u32.to_be_bytes());
        library
            .edit_tone(0, 0, |selected| {
                selected.set_amp_control_fraction(AmpControl::Drive, 0.5)
            })
            .unwrap();
        assert_eq!(library.as_bytes(), expected);
        assert!(library.is_dirty());
        assert_eq!(library.tone(1, 0).unwrap().as_bytes(), unselected);
        assert!(
            library
                .tone(0, 0)
                .unwrap()
                .as_bytes()
                .windows(opaque.len())
                .any(|window| window == opaque)
        );
        assert_eq!(
            library.tone(0, 0).unwrap().as_bytes()[value_offset..value_offset + 4],
            0x3f00_0000u32.to_be_bytes()
        );
        let before_failed_edit = library.as_bytes().to_vec();
        assert!(
            library
                .edit_tone(0, 1, |selected| selected
                    .set_amp_control_fraction(AmpControl::Drive, 0.5))
                .is_err()
        );
        assert_eq!(library.as_bytes(), before_failed_edit);
        library.revert().unwrap();
        assert_eq!(library.as_bytes(), original);
        assert!(!library.is_dirty());
        assert!(library.edit_tone(2, 0, |_| Ok(())).is_err());

        library
            .edit_tone(0, 0, |selected| {
                selected.set_amp_control_fraction(AmpControl::Drive, 0.5)
            })
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("pod-bank.vwb");
        library.save_copy(&copy).unwrap();
        let saved_copy = std::fs::read(&copy).unwrap();
        assert_eq!(saved_copy, library.as_bytes());
        assert!(!PodBundleLibrary::parse(saved_copy).unwrap().is_dirty());
        assert!(
            library.is_dirty(),
            "copying does not change the source baseline"
        );
        assert!(
            library.save_copy(&copy).is_err(),
            "save copy refuses overwrite"
        );

        let guitar = include_bytes!("../tests/fixtures/editor-preset.vwm").to_vec();
        let mixed = bundle_with_banks(&[vec![tone.clone()], vec![guitar]]);
        assert!(matches!(
            PodBundleLibrary::parse(mixed),
            Err(ToneError::WrongTarget { device_id }) if device_id != PODXT_LIVE_TONE_ID
        ));
        assert!(PodBundleLibrary::parse(bundle_with_banks(&[vec![]])).is_err());
        let mut malformed = original;
        malformed[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(PodBundleLibrary::parse(malformed).is_err());
    }

    #[test]
    fn zero_length_nonregular_facts_are_not_read() {
        let mut called = false;
        let error = read_regular_tone(
            Ok(PathFacts {
                regular_file: false,
                len: 0,
            }),
            || {
                called = true;
                Ok(vec![1])
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(error.detail().contains("regular file"));
        assert!(error.detail().contains("not opened"));
        let mut called = false;
        let error = read_regular_tone(
            Ok(PathFacts {
                regular_file: true,
                len: TONE_BYTES_CEILING + 1,
            }),
            || {
                called = true;
                Ok(vec![1])
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(error.detail().contains("ceiling"));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-tone");
        std::fs::create_dir(&path).unwrap();
        let error = read_regular_tone(path_facts(&path), || panic!("directory must not be read"))
            .unwrap_err();
        assert!(error.detail().contains("regular file"));
    }

    #[test]
    fn pod_bundle_reader_checks_regular_path_and_16_mib_ceiling_before_read() {
        let mut called = false;
        assert!(
            read_regular_bundle(
                Ok(PathFacts {
                    regular_file: false,
                    len: 0,
                }),
                || {
                    called = true;
                    Ok((
                        PathFacts {
                            regular_file: true,
                            len: 0,
                        },
                        std::io::Cursor::new(vec![]),
                    ))
                },
            )
            .is_err()
        );
        assert!(!called);
        assert!(
            read_regular_bundle(
                Ok(PathFacts {
                    regular_file: true,
                    len: BUNDLE_BYTES_CEILING + 1,
                }),
                || {
                    called = true;
                    Ok((
                        PathFacts {
                            regular_file: true,
                            len: 0,
                        },
                        std::io::Cursor::new(vec![]),
                    ))
                },
            )
            .is_err()
        );
        assert!(!called);

        let error = read_regular_bundle(
            Ok(PathFacts {
                regular_file: true,
                len: 1,
            }),
            || {
                Ok((
                    PathFacts {
                        regular_file: false,
                        len: 0,
                    },
                    std::io::Cursor::new(vec![]),
                ))
            },
        )
        .unwrap_err();
        assert!(error.detail().contains("regular file"));
        assert!(error.detail().contains("not read"));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bounded-bank.vwb");
        std::fs::write(&path, b"offline contents").unwrap();
        let actual = read_regular_bundle(path_facts(&path), || {
            let file = std::fs::File::open(&path)?;
            let metadata = file.metadata()?;
            Ok((
                PathFacts {
                    regular_file: metadata.is_file(),
                    len: metadata.len(),
                },
                file,
            ))
        })
        .unwrap();
        assert_eq!(actual, b"offline contents");
    }

    #[test]
    fn pod_bundle_reader_stops_at_ceiling_plus_one_when_file_grows() {
        struct GrowingReader {
            remaining: u64,
            consumed: std::rc::Rc<std::cell::Cell<u64>>,
        }

        impl std::io::Read for GrowingReader {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                let count = output.len().min(self.remaining as usize);
                output[..count].fill(0x5a);
                self.remaining -= count as u64;
                self.consumed.set(self.consumed.get() + count as u64);
                Ok(count)
            }
        }

        let consumed = std::rc::Rc::new(std::cell::Cell::new(0));
        let reader_consumed = consumed.clone();
        let error = read_regular_bundle(
            Ok(PathFacts {
                regular_file: true,
                len: 12,
            }),
            move || {
                Ok((
                    PathFacts {
                        regular_file: true,
                        len: 12,
                    },
                    GrowingReader {
                        remaining: BUNDLE_BYTES_CEILING * 2,
                        consumed: reader_consumed,
                    },
                ))
            },
        )
        .unwrap_err();
        assert!(error.detail().contains("ceiling"));
        assert_eq!(consumed.get(), BUNDLE_BYTES_CEILING + 1);
    }

    #[test]
    fn pod_bundle_parse_rejects_structurally_valid_file_over_total_ceiling() {
        let oversized = bundle_with_total_length(&live_tone(), BUNDLE_BYTES_CEILING as usize + 2);
        assert_eq!(oversized.len(), BUNDLE_BYTES_CEILING as usize + 2);
        let error = PodBundleLibrary::parse(oversized).unwrap_err();
        assert!(error.detail().contains("16 MiB"));
    }

    #[test]
    fn pod_bundle_replacement_growth_over_ceiling_is_transactional() {
        let original = bundle_with_total_length(&live_tone(), BUNDLE_BYTES_CEILING as usize - 2);
        let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
        library
            .edit_entry(0, |tone| tone.rename("Unsaved"))
            .unwrap();
        let before_replace = library.as_bytes().to_vec();
        let saved_before = library.saved.clone();
        assert!(library.is_dirty());
        assert_eq!(library.entry_location(0), Some((0, 0)));

        let replacement = tone_with_empty_unknown_chunk(&live_tone());
        let error = library.replace_entry(0, replacement).unwrap_err();
        assert!(error.detail().contains("16 MiB"));
        assert_eq!(library.as_bytes(), before_replace);
        assert_eq!(library.saved, saved_before);
        assert!(library.is_dirty());
        assert_eq!(library.entry_location(0), Some((0, 0)));
        assert_eq!(library.tone(0, 0).unwrap().name(), "Unsaved");
    }

    fn edited_name_too_long(tone: &PodTone) -> bool {
        let mut tone = tone.clone();
        tone.rename(&"🎸".repeat(17)).is_err() && tone.rename("bad\0name").is_err()
    }

    fn tone_name_offset(bytes: &[u8]) -> usize {
        find_chunk_data(bytes, b"PINF") + 8
    }

    fn find_chunk_data(bytes: &[u8], id: &[u8; 4]) -> usize {
        let children = preset::chunks(bytes, 12..bytes.len()).unwrap();
        preset::unique(&children, id).unwrap().data.start
    }
}

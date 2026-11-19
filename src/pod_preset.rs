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
//! is not a device-accepted-domain or model-entitlement claim. Explicit source-
//! backed model-enable edits patch only the selected existing model's canonical
//! MINF enabled byte; the other file bytes are preserved.
//!
//! The MIDI hardware id `L6HW_ID_PODXTLIVE` (18000) is a different namespace.
//! The archived firmware version `0x03010000` is not a requirement of a tone file.

use crate::document;
use crate::pod_assets::{self, SlotRole};
use crate::preset;
use std::{ops::Range, path::Path};

/// PINF target for PODxt Live. Equal to `L6ProductConstants.PODXT_LIVE`.
pub const PODXT_LIVE_TONE_ID: u32 = 0x0003_000a;
/// Exact AMP model choices admitted for the bounded existing-slot operation.
pub const PODXT_AMP_MODEL_CHOICES: [PodAmpModelChoice; 2] =
    [PodAmpModelChoice::NoAmp, PodAmpModelChoice::Line6Clean];

/// One source-admitted AMP model-id choice. This is not a device-support claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PodAmpModelChoice {
    NoAmp,
    Line6Clean,
}

impl PodAmpModelChoice {
    pub const fn model_id(self) -> u32 {
        match self {
            Self::NoAmp => 0x0002_0000,
            Self::Line6Clean => 0x0002_0001,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NoAmp => "No Amp",
            Self::Line6Clean => "Line 6 Clean",
        }
    }

    pub const fn from_model_id(model_id: u32) -> Option<Self> {
        match model_id {
            0x0002_0000 => Some(Self::NoAmp),
            0x0002_0001 => Some(Self::Line6Clean),
            _ => None,
        }
    }
}

impl TryFrom<u32> for PodAmpModelChoice {
    type Error = u32;

    fn try_from(model_id: u32) -> Result<Self, Self::Error> {
        Self::from_model_id(model_id).ok_or(model_id)
    }
}

/// Exact unconditional CAB ids in the supplied PODxt source table.
pub const PODXT_CAB_MODEL_IDS: [u32; 25] = [
    0x0102_0000,
    0x0102_0001,
    0x0102_0002,
    0x0102_0003,
    0x0102_0004,
    0x0102_0005,
    0x0102_0006,
    0x0102_0007,
    0x0102_0008,
    0x0102_0009,
    0x0102_000a,
    0x0102_000b,
    0x0102_000c,
    0x0102_000d,
    0x0102_000e,
    0x0102_000f,
    0x0102_0010,
    0x0102_0011,
    0x0102_0012,
    0x0102_0013,
    0x0102_0014,
    0x0102_0015,
    0x0102_0016,
    0x0102_0018,
    0x0102_001a,
];
pub const PODXT_BASS_CAB_MODEL_IDS: [u32; 22] = [
    0x0103_0001,
    0x0103_0002,
    0x0103_0003,
    0x0103_0004,
    0x0103_0005,
    0x0103_0006,
    0x0103_0007,
    0x0103_0008,
    0x0103_0009,
    0x0103_000a,
    0x0103_000b,
    0x0103_000c,
    0x0103_000d,
    0x0103_000e,
    0x0103_000f,
    0x0103_0010,
    0x0103_0011,
    0x0103_0012,
    0x0103_0013,
    0x0103_0014,
    0x0103_0015,
    0x0103_0016,
];
pub const PODXT_MIC_PARAM: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PodCabModelChoice(u32);

impl PodCabModelChoice {
    pub const fn model_id(self) -> u32 {
        self.0
    }
    pub fn numeric_label(self) -> String {
        format!("CAB {:08x}", self.0)
    }
    pub fn from_model_id(id: u32) -> Option<Self> {
        PODXT_CAB_MODEL_IDS.contains(&id).then_some(Self(id))
    }
}

impl TryFrom<u32> for PodCabModelChoice {
    type Error = u32;
    fn try_from(id: u32) -> Result<Self, Self::Error> {
        Self::from_model_id(id).ok_or(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodCabClass {
    Bass,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PodMicChoice(u32);

impl PodMicChoice {
    pub const fn value(self) -> u32 {
        self.0
    }
    pub fn from_value(value: u32) -> Option<Self> {
        (value <= 3).then_some(Self(value))
    }
}

impl TryFrom<u32> for PodMicChoice {
    type Error = u32;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::from_value(value).ok_or(value)
    }
}
/// Amp model slot from `L6Patch.UpdateMetaAmpName` and `kGPXTSlot_Amp`.
pub const AMP_SLOT: u32 = 0x0003_0000;
/// Stomp selector resolved to the SYNWAVE definition at layout position 1.
pub const STOMP_SYNTH_WAVE_SELECTOR: u32 = 0x0003_0001;
/// Semantic PARM id resolved by the Stomp selector for SYNWAVE.
pub const STOMP_SYNTH_WAVE_PARAM: u32 = 0x0010_0001;
/// PODxt Live Stomp layouts whose position-1 control uses `SYNWAVE_8`.
pub const STOMP_SYNTH_WAVE_MODEL_IDS: [u32; 8] = [
    0x020a_0001,
    0x020a_0002,
    0x020a_0003,
    0x020a_0004,
    0x020a_0005,
    0x020a_0006,
    0x020a_0007,
    0x020a_000c,
];
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
/// Eleven Mod model IDs with no Pack 4 predicate in the PODxt source selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PodModModelChoice(u32);

impl PodModModelChoice {
    pub const MODEL_IDS: [u32; 11] = [
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
    ];

    pub const fn model_id(self) -> u32 {
        self.0
    }

    pub fn from_model_id(id: u32) -> Option<Self> {
        Self::MODEL_IDS.contains(&id).then_some(Self(id))
    }

    pub fn numeric_label(self) -> String {
        format!("Mod {:08x}", self.0)
    }
}

impl TryFrom<u32> for PodModModelChoice {
    type Error = u32;

    fn try_from(id: u32) -> Result<Self, Self::Error> {
        Self::from_model_id(id).ok_or(id)
    }
}
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
/// Nine unconditional PODxt Live Delay choices admitted for the offline ID-only edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PodDelayModelChoice(u32);

impl PodDelayModelChoice {
    pub const MODEL_IDS: [u32; 9] = [
        0x0202_0002,
        0x0202_0003,
        0x0202_0004,
        0x0202_0005,
        0x0202_0006,
        0x0202_0007,
        0x0202_0008,
        0x0202_0009,
        0x0202_000a,
    ];

    pub const fn model_id(self) -> u32 {
        self.0
    }
    pub fn from_model_id(id: u32) -> Option<Self> {
        Self::MODEL_IDS.contains(&id).then_some(Self(id))
    }
    pub fn numeric_label(self) -> String {
        format!("Delay {:08x}", self.0)
    }
}

impl TryFrom<u32> for PodDelayModelChoice {
    type Error = u32;
    fn try_from(id: u32) -> Result<Self, Self::Error> {
        Self::from_model_id(id).ok_or(id)
    }
}
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
/// The 14 Stomp ids with no Pack 4 case in the Line6Edit PODxt selector guard.
/// Gated ids remain valid imports but are not constructible requested choices.
pub const STOMP_MODEL_CHOICE_IDS: [u32; 14] = [
    0x0205_0000,
    0x0205_0002,
    0x0205_0003,
    0x0205_0005,
    0x0205_0004,
    0x0200_0002,
    0x0200_0004,
    0x0200_0005,
    0x0200_0007,
    0x020a_0000,
    0x0205_0008,
    0x0205_0009,
    0x020a_000d,
    0x020a_000e,
];

/// Closed requested Stomp model choice for the conservative offline selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PodStompModelChoice(u32);

impl PodStompModelChoice {
    pub const MODEL_IDS: [u32; 14] = STOMP_MODEL_CHOICE_IDS;

    pub const fn model_id(self) -> u32 {
        self.0
    }

    pub fn from_model_id(id: u32) -> Option<Self> {
        Self::MODEL_IDS.contains(&id).then_some(Self(id))
    }

    pub fn numeric_label(self) -> String {
        format!("Stomp {:08x}", self.0)
    }
}

impl TryFrom<u32> for PodStompModelChoice {
    type Error = u32;

    fn try_from(id: u32) -> Result<Self, Self::Error> {
        Self::from_model_id(id).ok_or(id)
    }
}
/// Stomp Drive selector passed through `GetFXParamDef`.
pub const STOMP_DRIVE_SELECTOR: u32 = 0x0003_0001;
/// Raw semantic PARM identifier selected for Stomp Drive.
pub const STOMP_DRIVE_PARAM: u32 = 0x0010_0001;
pub const STOMP_GAIN_SELECTOR: u32 = 0x0003_0002;
pub const STOMP_GAIN_PARAM: u32 = 0x0010_0002;
pub const STOMP_TONE_SELECTOR: u32 = 0x0003_0003;
pub const STOMP_TONE_PARAM: u32 = 0x0010_0003;
/// Stomp Whammy model whose two controls use the Line6Edit WHAMMY_INTERVAL format.
pub const STOMP_WHAMMY_INTERVAL_MODEL_ID: u32 = 0x020a_000e;
/// Non-Bass PODxt four-band EQ model and fixed slot from Line6Edit 3.06.
pub const PODXT_EQ_MODEL_ID: u32 = 0x020c_0002;
pub const PODXT_EQ_SLOT: u32 = 0x0003_0004;
/// Source editor selector for the existing amp model's MINF enabled flag.
pub const AMP_ENABLE_SELECTOR: u32 = 0x4001_0001;
/// Source editor selector for the fixed non-Bass PODxt EQ model's MINF enabled flag.
pub const EQ_ENABLE_SELECTOR: u32 = 0x4009_0000;
const PODXT_EQ_FREQUENCY_TABLES: [[f64; 9]; 4] = [
    [50.0, 130.0, 210.0, 290.0, 370.0, 450.0, 530.0, 610.0, 690.0],
    [
        50.0, 130.0, 290.0, 450.0, 1250.0, 2050.0, 2850.0, 4450.0, 6050.0,
    ],
    [
        100.0, 900.0, 1700.0, 3300.0, 4900.0, 6500.0, 8100.0, 9700.0, 11300.0,
    ],
    [
        500.0, 900.0, 1300.0, 2100.0, 2900.0, 4500.0, 6100.0, 7700.0, 9300.0,
    ],
];
const PODXT_EQ_GAIN_MIN: f32 = -12.8;
const PODXT_EQ_GAIN_MAX: f32 = 12.6;

/// One of the four non-Bass PODxt Live EQ graph bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PodxtEqBand {
    One,
    Two,
    Three,
    Four,
}

/// The two independently recovered Line6Edit model-enable controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PodModelEnable {
    Amp,
    Eq,
}

impl PodModelEnable {
    const fn label(self) -> &'static str {
        match self {
            Self::Amp => "Amp",
            Self::Eq => "EQ",
        }
    }

    const fn slot(self) -> u32 {
        match self {
            Self::Amp => AMP_SLOT,
            Self::Eq => PODXT_EQ_SLOT,
        }
    }

    const fn expected_model_id(self) -> Option<u32> {
        match self {
            Self::Amp => None,
            Self::Eq => Some(PODXT_EQ_MODEL_ID),
        }
    }
}

/// A MINF byte's source read interpretation and whether the offline editor may edit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelEnableState {
    Canonical(bool),
    NonCanonical {
        raw: u8,
        source_interpretation: bool,
    },
}

impl PodxtEqBand {
    pub const ALL: [Self; 4] = [Self::One, Self::Two, Self::Three, Self::Four];

    const fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
            Self::Three => 2,
            Self::Four => 3,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
        }
    }

    pub const fn selector_ids(self) -> (u32, u32) {
        let first = 0x0009_0001 + (self.index() as u32 * 2);
        (first, first + 1)
    }

    pub const fn parameter_ids(self) -> (u32, u32) {
        let first = 0x0010_0000 + (self.index() as u32 * 2);
        (first, first + 1)
    }
}
pub const STOMP_WHAMMY_HEEL_SELECTOR: u32 = 0x0003_0002;
pub const STOMP_WHAMMY_HEEL_PARAM: u32 = 0x0010_0002;
pub const STOMP_WHAMMY_TOE_SELECTOR: u32 = 0x0003_0003;
pub const STOMP_WHAMMY_TOE_PARAM: u32 = 0x0010_0003;
const WHAMMY_INTERVAL_RESOLUTION: f32 = 63.75;
const WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL: f32 = f32::from_bits(0x3c80_8081);
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
pub const PAYLOADS_NOT_DECODED: &str = "Known asset names and 12-byte parameter records are shown. Stored Amp/EQ model-enable flags and the listed host-scale controls have offline edits; other parameter formats remain raw. Unknown ids stay raw.";
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

/// PODxt Live Whammy Stomp knob whose input is the editor's normalized knob
/// position. This is distinct from a formatted display value or a stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompWhammyKnob {
    Heel,
    Toe,
}

impl StompWhammyKnob {
    pub const ALL: [Self; 2] = [Self::Heel, Self::Toe];

    pub const fn selector_id(self) -> u32 {
        match self {
            Self::Heel => STOMP_WHAMMY_HEEL_SELECTOR,
            Self::Toe => STOMP_WHAMMY_TOE_SELECTOR,
        }
    }

    pub const fn param_id(self) -> u32 {
        match self {
            Self::Heel => STOMP_WHAMMY_HEEL_PARAM,
            Self::Toe => STOMP_WHAMMY_TOE_PARAM,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Heel => "Heel",
            Self::Toe => "Toe",
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

/// Source-defined NOTE choices shared by PODxt Live Mod and Delay model
/// parameter IDs. Mod NOTE and Delay NOTE have separate edit operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum DelayNoteChoice {
    Off = 0,
    Whole = 1,
    HalfDotted = 2,
    Half = 3,
    HalfTriplet = 4,
    QuarterDotted = 5,
    Quarter = 6,
    QuarterTriplet = 7,
    EighthDotted = 8,
    Eighth = 9,
    EighthTriplet = 10,
    SixteenthDotted = 11,
    Sixteenth = 12,
    SixteenthTriplet = 13,
}

impl DelayNoteChoice {
    pub const ALL: [Self; 14] = [
        Self::Off,
        Self::Whole,
        Self::HalfDotted,
        Self::Half,
        Self::HalfTriplet,
        Self::QuarterDotted,
        Self::Quarter,
        Self::QuarterTriplet,
        Self::EighthDotted,
        Self::Eighth,
        Self::EighthTriplet,
        Self::SixteenthDotted,
        Self::Sixteenth,
        Self::SixteenthTriplet,
    ];

    pub const fn value(self) -> u32 {
        self as u32
    }

    pub const fn from_value(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::Whole),
            2 => Some(Self::HalfDotted),
            3 => Some(Self::Half),
            4 => Some(Self::HalfTriplet),
            5 => Some(Self::QuarterDotted),
            6 => Some(Self::Quarter),
            7 => Some(Self::QuarterTriplet),
            8 => Some(Self::EighthDotted),
            9 => Some(Self::Eighth),
            10 => Some(Self::EighthTriplet),
            11 => Some(Self::SixteenthDotted),
            12 => Some(Self::Sixteenth),
            13 => Some(Self::SixteenthTriplet),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Whole => "Whole",
            Self::HalfDotted => "Half Dotted",
            Self::Half => "Half",
            Self::HalfTriplet => "Half Triplet",
            Self::QuarterDotted => "Quarter Dotted",
            Self::Quarter => "Quarter",
            Self::QuarterTriplet => "Quarter Triplet",
            Self::EighthDotted => "8th Dotted",
            Self::Eighth => "8th",
            Self::EighthTriplet => "8th Triplet",
            Self::SixteenthDotted => "16th Dotted",
            Self::Sixteenth => "16th",
            Self::SixteenthTriplet => "16th Triplet",
        }
    }

    fn multiplier(self) -> Option<f32> {
        Some(match self {
            Self::Off => return None,
            Self::Whole => 1.0,
            Self::HalfDotted => 0.75,
            Self::Half => 0.5,
            Self::HalfTriplet => 0.33333334,
            Self::QuarterDotted => 0.375,
            Self::Quarter => 0.25,
            Self::QuarterTriplet => 0.16666667,
            Self::EighthDotted => 0.1875,
            Self::Eighth => 0.125,
            Self::EighthTriplet => 0.083333336,
            Self::SixteenthDotted => 0.09375,
            Self::Sixteenth => 0.0625,
            Self::SixteenthTriplet => 0.041666668,
        })
    }
}

/// Read-only NOTE presentation retains unsupported imported integer words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayNoteValue {
    Known(DelayNoteChoice),
    Unknown { raw_bits: u32 },
}

/// Mod NOTE uses the same source-defined ordinal table as Delay NOTE.
pub type ModNoteChoice = DelayNoteChoice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModNoteValue {
    Known(ModNoteChoice),
    Unknown { raw_bits: u32 },
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

/// One of the eight source-listed SYNWAVE choices for the admitted Stomp models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompWaveSelection {
    Wave1,
    Wave2,
    Wave3,
    Wave4,
    Wave5,
    Wave6,
    Wave7,
    Wave8,
}

impl StompWaveSelection {
    pub const ALL: [Self; 8] = [
        Self::Wave1,
        Self::Wave2,
        Self::Wave3,
        Self::Wave4,
        Self::Wave5,
        Self::Wave6,
        Self::Wave7,
        Self::Wave8,
    ];

    const fn controller_value(self) -> u8 {
        match self {
            Self::Wave1 => 0,
            Self::Wave2 => 16,
            Self::Wave3 => 32,
            Self::Wave4 => 48,
            Self::Wave5 => 64,
            Self::Wave6 => 80,
            Self::Wave7 => 96,
            Self::Wave8 => 112,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wave1 => "WAVE 1",
            Self::Wave2 => "WAVE 2",
            Self::Wave3 => "WAVE 3",
            Self::Wave4 => "WAVE 4",
            Self::Wave5 => "WAVE 5",
            Self::Wave6 => "WAVE 6",
            Self::Wave7 => "WAVE 7",
            Self::Wave8 => "WAVE 8",
        }
    }

    /// Reproduce Java's int-to-float, subtract 0.5f, then divide by 127.0f.
    pub const fn float_bits(self) -> u32 {
        let controller = self.controller_value();
        let as_float = controller as f32;
        let adjusted = as_float - 0.5_f32;
        let normalized = adjusted / 127.0_f32;
        normalized.to_bits()
    }

    fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

/// A known source choice or the exact raw float word from an unknown value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StompWaveValue {
    Known(StompWaveSelection),
    Unknown { raw_bits: u32 },
}

/// Source-backed harmony interval choices for the Stomp HARMONY_1M335 layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompHarmony1M335Selection {
    MinusOctave,
    MinusMajorSixth,
    MinusMinorSixth,
    MinusFourth,
    Unison,
    MinorThird,
    MajorThird,
    Fifth,
    Octave,
}

impl StompHarmony1M335Selection {
    pub const ALL: [Self; 9] = [
        Self::MinusOctave,
        Self::MinusMajorSixth,
        Self::MinusMinorSixth,
        Self::MinusFourth,
        Self::Unison,
        Self::MinorThird,
        Self::MajorThird,
        Self::Fifth,
        Self::Octave,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::MinusOctave => "-1 OCT",
            Self::MinusMajorSixth => "-MAJ6TH",
            Self::MinusMinorSixth => "-MIN6TH",
            Self::MinusFourth => "-4TH",
            Self::Unison => "UNISON",
            Self::MinorThird => "MIN3RD",
            Self::MajorThird => "MAJ3RD",
            Self::Fifth => "5TH",
            Self::Octave => "1 OCT",
        }
    }
    pub const fn controller_key(self) -> u8 {
        match self {
            Self::MinusOctave => 0,
            Self::MinusMajorSixth => 16,
            Self::MinusMinorSixth => 32,
            Self::MinusFourth => 48,
            Self::Unison => 64,
            Self::MinorThird => 80,
            Self::MajorThird => 96,
            Self::Fifth => 112,
            Self::Octave => 127,
        }
    }
    pub const fn float_bits(self) -> u32 {
        HARMONY_FLOAT_BITS[self as usize]
    }
    fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

/// Source-backed harmony interval choices for the Stomp HARMONY_1457 layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompHarmony1457Selection {
    MinusOctave,
    MinusFifth,
    MinusFourth,
    MinusSecond,
    Unison,
    Fourth,
    Fifth,
    Seventh,
    Octave,
}

impl StompHarmony1457Selection {
    pub const ALL: [Self; 9] = [
        Self::MinusOctave,
        Self::MinusFifth,
        Self::MinusFourth,
        Self::MinusSecond,
        Self::Unison,
        Self::Fourth,
        Self::Fifth,
        Self::Seventh,
        Self::Octave,
    ];
    pub const fn label(self) -> &'static str {
        match self {
            Self::MinusOctave => "-1 OCT",
            Self::MinusFifth => "-5TH",
            Self::MinusFourth => "-4TH",
            Self::MinusSecond => "-2ND",
            Self::Unison => "UNISON",
            Self::Fourth => "4TH",
            Self::Fifth => "5TH",
            Self::Seventh => "7TH",
            Self::Octave => "1 OCT",
        }
    }
    pub const fn controller_key(self) -> u8 {
        match self {
            Self::MinusOctave => 0,
            Self::MinusFifth => 16,
            Self::MinusFourth => 32,
            Self::MinusSecond => 48,
            Self::Unison => 64,
            Self::Fourth => 80,
            Self::Fifth => 96,
            Self::Seventh => 112,
            Self::Octave => 127,
        }
    }
    pub const fn float_bits(self) -> u32 {
        HARMONY_FLOAT_BITS[self as usize]
    }
    fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

const HARMONY_FLOAT_BITS: [u32; 9] = [
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StompHarmonyValue<T> {
    Known(T),
    Unknown { raw_bits: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompHarmonyControl {
    Harmony1M335,
    Harmony1457,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StompHarmonySelection {
    Harmony1M335(StompHarmony1M335Selection),
    Harmony1457(StompHarmony1457Selection),
}

impl StompHarmonyControl {
    pub const fn selector_id(self) -> u32 {
        match self {
            Self::Harmony1M335 => 0x0003_0001,
            Self::Harmony1457 => 0x0003_0002,
        }
    }
    pub const fn parameter_id(self) -> u32 {
        match self {
            Self::Harmony1M335 => 0x0010_0001,
            Self::Harmony1457 => 0x0010_0002,
        }
    }
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
    /// Parsed MINF body ranges parallel to `models`; never extend into padding.
    model_minfo_ranges: Vec<std::ops::Range<usize>>,
    parameter_value_offsets: Vec<Vec<Option<usize>>>,
    /// Semantic ids decoded from the first word when an opaque PARM is at least four bytes.
    opaque_parameter_id_hints: Vec<Vec<Option<u32>>>,
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
) -> (Vec<ModelParameter>, Vec<Option<usize>>, Vec<Option<u32>>) {
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
    let id_hints = fields
        .iter()
        .filter(|chunk| chunk.id == *b"PARM")
        .map(|chunk| {
            (chunk.data.len() >= 4).then(|| preset::word(bytes, chunk.data.start) & 0x00ff_ffff)
        })
        .collect();
    (parameters, offsets, id_hints)
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
        let mut model_minfo_ranges = Vec::new();
        let mut parameter_value_offsets = Vec::new();
        let mut opaque_parameter_id_hints = Vec::new();
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
            let (parameters, offsets, id_hints) = read_parameters(&bytes, &fields);
            model_minfo_ranges.push(minf.data.clone());
            models.push(ModelSlot {
                slot,
                model_id: preset::word(&bytes, minf.data.start),
                ordinal,
                reserved,
                enabled,
                parameters,
            });
            parameter_value_offsets.push(offsets);
            opaque_parameter_id_hints.push(id_hints);
        }
        Ok(Self {
            bytes,
            name_offset,
            name,
            device_id,
            device_version,
            container,
            models,
            model_minfo_ranges,
            parameter_value_offsets,
            opaque_parameter_id_hints,
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

    /// Apply one admitted AMP choice to the existing AMP MINF model-id word.
    /// This file-only Apply path never creates a slot or rewrites PARMs.
    pub fn set_amp_model_choice(&mut self, choice: PodAmpModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "AMP model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "AMP model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "AMP model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| model.slot == AMP_SLOT)
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "AMP model selection requires exactly one existing model in slot {AMP_SLOT:08x}"
            )));
        };
        let model_index = *model_index;
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("AMP model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|minimum_end| minimum_end > range.end)
        {
            return Err(ToneError::format(
                "AMP model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        let requested = choice.model_id();
        if self.models[model_index].model_id == requested {
            return Ok(());
        }
        self.bytes[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        self.models[model_index].model_id = requested;
        Ok(())
    }

    /// Change only the existing Delay model ID in an individual L6PA tone.
    /// This application policy requires exactly one primary-or-fallback slot.
    pub fn set_delay_model_choice(&mut self, choice: PodDelayModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Delay model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Delay model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Delay model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }
        let delay_slots = [DELAY_SLOT, 0x0005_0004];
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| delay_slots.contains(&model.slot))
            .map(|(i, _)| i)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(
                "Delay model selection requires exactly one existing primary or fallback model",
            ));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !DELAY_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Delay model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("Delay model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Delay model selection requires an existing MINF body of at least 12 bytes",
            ));
        }
        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested)
        {
            return Err(ToneError::format(
                "Delay model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    /// Change only the existing Mod MINF model ID in an individual L6PA tone.
    /// This conservative offline policy admits only source-listed choices
    /// without a Pack 4 selector predicate and requires one existing slot.
    pub fn set_mod_model_choice(&mut self, choice: PodModModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Mod model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Mod model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Mod model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let mod_slots = [MOD_SLOT, 0x0005_0003];
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| mod_slots.contains(&model.slot))
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(
                "Mod model selection requires exactly one existing primary or fallback model",
            ));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !MOD_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Mod model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .cloned()
            .ok_or_else(|| ToneError::format("Mod model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Mod model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested)
        {
            return Err(ToneError::format(
                "Mod model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    /// Return the parsed source interpretation of one applicable model flag.
    /// Noncanonical bytes remain readable and are reported with their raw byte.
    pub fn model_enable_state(
        &self,
        control: PodModelEnable,
    ) -> Result<ModelEnableState, ToneError> {
        let (model_index, offset) = self.model_enable_location(control)?;
        let raw = self.bytes[offset + 11];
        debug_assert_eq!(self.models[model_index].enabled, Some((raw as i8) > 0));
        Ok(match raw {
            0 => ModelEnableState::Canonical(false),
            1 => ModelEnableState::Canonical(true),
            raw => ModelEnableState::NonCanonical {
                raw,
                source_interpretation: (raw as i8) > 0,
            },
        })
    }

    /// Set one existing PODxt model's MINF enabled state without creating or
    /// replacing records. The only file-byte change is MINF body byte 11.
    pub fn set_model_enabled(
        &mut self,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let (model_index, offset) = self.model_enable_location(control)?;
        let raw = self.bytes[offset + 11];
        if raw != 0 && raw != 1 {
            return Err(ToneError::format(format!(
                "{} enable edit requires canonical MINF byte 0x00 or 0x01; found 0x{raw:02x}",
                control.label()
            )));
        }
        let next = u8::from(enabled);
        if raw == next {
            return Ok(());
        }
        self.bytes[offset + 11] = next;
        self.models[model_index].enabled = Some(enabled);
        Ok(())
    }

    fn model_enable_location(&self, control: PodModelEnable) -> Result<(usize, usize), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "model-enable edit requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "model-enable edit requires the PODxt Live host profile minimum 3.00",
            ));
        }
        let slot = control.slot();
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| model.slot == slot)
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{} enable edit requires exactly one existing model in slot {slot:08x}",
                control.label()
            )));
        };
        let model_index = *model_index;
        if let Some(expected) = control.expected_model_id()
            && self.models[model_index].model_id != expected
        {
            return Err(ToneError::format(format!(
                "EQ enable edit requires model {expected:08x} in slot {slot:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|minimum_end| minimum_end > range.end)
        {
            return Err(ToneError::format(format!(
                "{} enable edit requires an existing MINF body of at least 12 bytes",
                control.label()
            )));
        }
        Ok((model_index, range.start))
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

    /// Apply one admitted Stomp choice to the existing slot's MINF model ID.
    /// This conservative file-only operation changes no parameters or other bytes.
    pub fn set_stomp_model_choice(&mut self, choice: PodStompModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Stomp model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Stomp model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Stomp model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| model.slot == STOMP_SLOT)
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "Stomp model selection requires exactly one existing model in slot {STOMP_SLOT:08x}"
            )));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !STOMP_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Stomp model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .cloned()
            .ok_or_else(|| ToneError::format("Stomp model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Stomp model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested || model.slot != STOMP_SLOT)
        {
            return Err(ToneError::format(
                "Stomp model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
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

    /// Read a source-backed Whammy knob position without quantizing or
    /// normalizing the imported word. The definition range is `0..=1`.
    pub fn stomp_whammy_knob_position(&self, control: StompWhammyKnob) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.stomp_whammy_knob_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Stomp Whammy {} knob position", control.label()),
        )
    }

    /// Read the existing Stomp SYNWAVE value without snapping unknown raw words.
    pub fn stomp_wave_selection(&self) -> Result<StompWaveValue, ToneError> {
        let (model_index, parameter_index) = self.stomp_wave_location()?;
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(
                "Stomp SYNWAVE target is not a float PARM",
            ));
        };
        Ok(StompWaveSelection::from_float_bits(*bits)
            .map(StompWaveValue::Known)
            .unwrap_or(StompWaveValue::Unknown { raw_bits: *bits }))
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1M335)?;
        let bits =
            self.delay_association_bits_at(model_index, parameter_index, "Stomp HARMONY_1M335")?;
        Ok(StompHarmony1M335Selection::from_float_bits(bits)
            .map(StompHarmonyValue::Known)
            .unwrap_or(StompHarmonyValue::Unknown { raw_bits: bits }))
    }

    pub fn stomp_harmony_1457_selection(
        &self,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1457)?;
        let bits =
            self.delay_association_bits_at(model_index, parameter_index, "Stomp HARMONY_1457")?;
        Ok(StompHarmony1457Selection::from_float_bits(bits)
            .map(StompHarmonyValue::Known)
            .unwrap_or(StompHarmonyValue::Unknown { raw_bits: bits }))
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

    /// Return the stored normalized frequency/gain words without snapping or
    /// rejecting finite values outside the source definition range.
    pub fn podxt_eq_band_raw_values(&self, band: PodxtEqBand) -> Result<(f32, f32), ToneError> {
        let (frequency_id, gain_id) = band.parameter_ids();
        let (model_index, frequency_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            frequency_id,
            &format!("EQ band {} frequency", band.label()),
        )?;
        let (_, gain_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            gain_id,
            &format!("EQ band {} gain", band.label()),
        )?;
        let read = |index: usize, label: &str| match self.models[model_index].parameters[index] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => Ok(f32::from_bits(bits)),
            _ => Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            ))),
        };
        Ok((
            read(frequency_index, "EQ frequency")?,
            read(gain_index, "EQ gain")?,
        ))
    }

    /// Source-style EQ display values for normalized words in the definition
    /// range. The frequency display uses the 127-step read conversion.
    pub fn podxt_eq_band_display_values(&self, band: PodxtEqBand) -> Result<(f32, f32), ToneError> {
        let (frequency, gain) = self.podxt_eq_band_raw_values(band)?;
        if !frequency.is_finite()
            || !(0.0..=1.0).contains(&frequency)
            || !gain.is_finite()
            || !(0.0..=1.0).contains(&gain)
        {
            return Err(ToneError::format(
                "EQ stored value is outside the finite definition range 0..=1",
            ));
        }
        let table = &PODXT_EQ_FREQUENCY_TABLES[band.index()];
        // Line6Edit promotes the stored float to double before forming the
        // 127-step controller key, then handles both endpoints explicitly.
        let key = (f64::from(frequency) * 127.0 + 0.5) as i32;
        let frequency_hz = if key <= 0 {
            table[0]
        } else if key >= 127 {
            table[8]
        } else {
            let point = (key >> 4) as usize;
            let fraction = (key & 0x0f) as f64 / 16.0;
            table[point] + (table[point + 1] - table[point]) * fraction
        } as f32;
        let width = PODXT_EQ_GAIN_MAX - PODXT_EQ_GAIN_MIN;
        let gain_db = if gain <= 0.0 {
            PODXT_EQ_GAIN_MIN
        } else if gain >= 1.0 {
            PODXT_EQ_GAIN_MAX
        } else {
            (f64::from(gain) * f64::from(width) + f64::from(PODXT_EQ_GAIN_MIN)) as f32
        };
        Ok((frequency_hz, gain_db))
    }

    /// Apply the original four-band EQ graph callback to one existing band.
    /// Inputs are graph display values in Hz and dB (both received as float32).
    /// The frequency and gain value words are committed together after every
    /// target, current value, and input has passed validation.
    pub fn set_podxt_eq_band(
        &mut self,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        let label = format!("EQ band {}", band.label());
        let (frequency_id, gain_id) = band.parameter_ids();
        let (model_index, frequency_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            frequency_id,
            &format!("{label} frequency"),
        )?;
        let (_, gain_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            gain_id,
            &format!("{label} gain"),
        )?;
        let current = |index: usize, control: &str| -> Result<(), ToneError> {
            let bits = match self.models[model_index].parameters[index] {
                ModelParameter::Recorded {
                    value: ParameterValue::Float { bits },
                    ..
                } => bits,
                _ => {
                    return Err(ToneError::format(format!(
                        "{label} {control} target is not a float PARM"
                    )));
                }
            };
            let value = f32::from_bits(bits);
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ToneError::format(format!(
                    "existing {label} {control} value is outside the finite definition range 0..=1"
                )));
            }
            Ok(())
        };
        current(frequency_index, "frequency")?;
        current(gain_index, "gain")?;
        let table = &PODXT_EQ_FREQUENCY_TABLES[band.index()];
        if !frequency_hz.is_finite()
            || frequency_hz < table[0] as f32
            || frequency_hz > table[8] as f32
        {
            return Err(ToneError::format(format!(
                "{label} frequency must be finite and within {}..{} Hz",
                table[0], table[8]
            )));
        }
        if !gain_db.is_finite() || gain_db < PODXT_EQ_GAIN_MIN || gain_db > PODXT_EQ_GAIN_MAX {
            return Err(ToneError::format(format!(
                "{label} gain must be finite and within -12.8..12.6 dB"
            )));
        }
        let frequency = frequency_hz as f64;
        let segment = (0..8).find(|&i| frequency <= table[i + 1]).unwrap_or(7);
        let normalized_frequency = (((frequency - table[segment])
            / (table[segment + 1] - table[segment])
            + segment as f64)
            / 8.0) as f32;
        let gain_width = PODXT_EQ_GAIN_MAX - PODXT_EQ_GAIN_MIN;
        let normalized_gain = if gain_db <= PODXT_EQ_GAIN_MIN {
            0.0
        } else if gain_db >= PODXT_EQ_GAIN_MAX {
            1.0
        } else {
            ((gain_db as f64 - PODXT_EQ_GAIN_MIN as f64) / gain_width as f64) as f32
        };
        let frequency_bits = normalized_frequency.to_bits();
        let gain_bits = normalized_gain.to_bits();
        let frequency_offset = self.parameter_value_offsets[model_index][frequency_index]
            .ok_or_else(|| {
                ToneError::format(format!("{label} frequency has no unique raw value word"))
            })?;
        let gain_offset =
            self.parameter_value_offsets[model_index][gain_index].ok_or_else(|| {
                ToneError::format(format!("{label} gain has no unique raw value word"))
            })?;

        self.bytes[frequency_offset..frequency_offset + 4]
            .copy_from_slice(&frequency_bits.to_be_bytes());
        self.bytes[gain_offset..gain_offset + 4].copy_from_slice(&gain_bits.to_be_bytes());
        for (index, bits) in [(frequency_index, frequency_bits), (gain_index, gain_bits)] {
            if let ModelParameter::Recorded {
                value: ParameterValue::Float { bits: stored },
                ..
            } = &mut self.models[model_index].parameters[index]
            {
                *stored = bits;
            }
        }
        Ok(())
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

    /// Apply a finite normalized editor knob position through the original
    /// WHAMMY_INTERVAL float32 truncating quantizer. This is not a display-value
    /// parser and does not accept raw stored values.
    pub fn set_stomp_whammy_knob_position(
        &mut self,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Stomp Whammy {} knob position", control.label());
        Self::validate_host_fraction(position, &label)?;
        let (model_index, parameter_index) = self.stomp_whammy_knob_location(control)?;
        let current = match &self.models[model_index].parameters[parameter_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => f32::from_bits(*bits),
            _ => {
                return Err(ToneError::format(format!(
                    "{label} target is not a float PARM"
                )));
            }
        };
        if !current.is_finite() || !(0.0..=1.0).contains(&current) {
            return Err(ToneError::format(format!(
                "existing {label} value is outside the finite definition range 0..=1"
            )));
        }
        let scaled = position * WHAMMY_INTERVAL_RESOLUTION;
        let steps = scaled as i32;
        let stored = (steps as f32) * WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL;
        self.set_float_control_fraction_at(model_index, parameter_index, stored, &label)
    }

    /// Replace only the existing Stomp SYNWAVE value word with a typed choice.
    /// Unknown finite values may be replaced; non-finite stored values refuse mutation.
    pub fn set_stomp_wave_selection(
        &mut self,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) = self.stomp_wave_location()?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp SYNWAVE",
        )
    }

    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        selection: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1M335)?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp HARMONY_1M335",
        )
    }

    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        selection: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1457)?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp HARMONY_1457",
        )
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

    /// Read the stored Delay NOTE without normalizing unknown imported ordinals.
    pub fn delay_note_value(&self) -> Result<DelayNoteValue, ToneError> {
        let model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Delay NOTE")?;
        let bits = match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        Ok(DelayNoteChoice::from_value(bits)
            .map(DelayNoteValue::Known)
            .unwrap_or(DelayNoteValue::Unknown { raw_bits: bits }))
    }

    /// Display the normalized Delay TIME as milliseconds for UI staging.
    pub fn delay_time_ms(&self) -> Result<f32, ToneError> {
        let model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay TIME",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Delay TIME")?;
        let raw = self.float_parameter_bits(model, parameter, "Delay TIME")?;
        if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
            return Err(ToneError::format(
                "stored Delay TIME is outside finite raw range 0..=1",
            ));
        }
        let scale: f32 = 2000.0 - 20.0;
        Ok(20.0 + raw * scale)
    }

    pub fn mod_note_value(&self) -> Result<ModNoteValue, ToneError> {
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Mod NOTE")?;
        let bits = match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        Ok(DelayNoteChoice::from_value(bits)
            .map(ModNoteValue::Known)
            .unwrap_or(ModNoteValue::Unknown { raw_bits: bits }))
    }

    pub fn mod_speed_hz(&self) -> Result<f32, ToneError> {
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod SPEED",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Mod SPEED")?;
        let raw = self.float_parameter_bits(model, parameter, "Mod SPEED")?;
        if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
            return Err(ToneError::format(
                "stored Mod SPEED is outside finite raw range 0..=1",
            ));
        }
        Ok(0.1 + raw * (15.0_f32 - 0.1_f32))
    }

    /// Writes an existing Mod NOTE record. Positive choices recalculate SPEED
    /// from the source System Tempo; Off changes only NOTE. Delay NOTE must be
    /// Off because the original shared callback also recalculates Delay TIME.
    pub fn set_mod_note_choice(&mut self, choice: ModNoteChoice) -> Result<(), ToneError> {
        self.require_control_edit_profile("Mod NOTE")?;
        let mod_model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE",
        )?;
        let (note_index, note_offset) =
            self.unique_control_parameter(mod_model, 0x0020_0000, false, "Mod NOTE")?;
        let old_note = match self.models[mod_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if ModNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Mod NOTE is outside 0..=13"));
        }

        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE cross-effect guard",
        )?;
        let (delay_note_index, _) = self.unique_control_parameter(
            delay_model,
            0x0020_0000,
            false,
            "Delay NOTE cross-effect guard",
        )?;
        let delay_note = match self.models[delay_model].parameters[delay_note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if delay_note != 0 {
            return Err(ToneError::format(
                "Mod NOTE edit requires Delay NOTE Off to avoid a coupled Delay TIME write",
            ));
        }

        let speed_target = if choice != ModNoteChoice::Off {
            Some(self.unique_control_parameter(mod_model, 0x0010_0000, true, "Mod SPEED")?)
        } else {
            None
        };
        let speed_bits = if let Some((speed_index, _)) = speed_target {
            let old_speed = self.float_parameter_bits(mod_model, speed_index, "Mod SPEED")?;
            if !old_speed.is_finite() || !(0.0..=1.0).contains(&old_speed) {
                return Err(ToneError::format(
                    "stored Mod SPEED is outside finite raw range 0..=1",
                ));
            }
            let system_model =
                self.unique_control_model(&[0x0001_0000], &[0x3e70_0000], "System Tempo context")?;
            let (tempo_index, _) = self.unique_control_parameter(
                system_model,
                0x0020_0001,
                true,
                "System Tempo context",
            )?;
            let tempo =
                self.float_parameter_bits(system_model, tempo_index, "System Tempo context")?;
            if !tempo.is_finite() || !(30.0..=240.0).contains(&tempo) {
                return Err(ToneError::format(
                    "stored Tempo is outside finite BPM range 30..=240",
                ));
            }
            let multiplier = choice.multiplier().expect("positive Mod NOTE choice");
            let q = multiplier * 4.0_f32;
            let displayed_hz = (tempo / (q * 60.0_f32)).clamp(0.1, 15.0);
            Some(source_range_raw(displayed_hz, 0.1_f32, 15.0_f32).to_bits())
        } else {
            None
        };

        let current_speed_bits = speed_target
            .map(|(index, _)| self.float_parameter_bits(mod_model, index, "Mod SPEED"))
            .transpose()?
            .map(f32::to_bits);
        if old_note == choice.value() && speed_bits == current_speed_bits {
            return Ok(());
        }
        self.write_parameter_bits(mod_model, note_index, note_offset, choice.value());
        if let (Some(bits), Some((index, offset))) = (speed_bits, speed_target) {
            self.write_parameter_bits(mod_model, index, offset, bits);
        }
        Ok(())
    }

    /// Writes displayed Mod SPEED in Hz through the original range format and
    /// applies the source manual-speed event, which clears a positive NOTE.
    pub fn set_mod_speed_hz(&mut self, hz: f32) -> Result<(), ToneError> {
        self.require_control_edit_profile("Mod SPEED")?;
        if !hz.is_finite() || !(0.1..=15.0).contains(&hz) {
            return Err(ToneError::format(
                "Mod SPEED edit requires finite displayed Hz in 0.1..=15",
            ));
        }
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod SPEED",
        )?;
        let (speed_index, speed_offset) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Mod SPEED")?;
        let old_speed = self.float_parameter_bits(model, speed_index, "Mod SPEED")?;
        if !old_speed.is_finite() || !(0.0..=1.0).contains(&old_speed) {
            return Err(ToneError::format(
                "stored Mod SPEED is outside finite raw range 0..=1",
            ));
        }
        let (note_index, note_offset) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Mod NOTE")?;
        let old_note = match self.models[model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if ModNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Mod NOTE is outside 0..=13"));
        }
        let new_speed = source_range_raw(hz, 0.1_f32, 15.0_f32).to_bits();
        if old_speed.to_bits() == new_speed && old_note == 0 {
            return Ok(());
        }
        self.write_parameter_bits(model, speed_index, speed_offset, new_speed);
        if old_note != 0 {
            self.write_parameter_bits(model, note_index, note_offset, 0);
        }
        Ok(())
    }

    /// Apply a typed Delay NOTE choice to an existing target. Positive choices
    /// also update TIME using the original f32 NOTE formula and source range
    /// conversion. Off changes NOTE only. This operation never edits Mod.
    pub fn set_delay_note_choice(&mut self, choice: DelayNoteChoice) -> Result<(), ToneError> {
        self.require_control_edit_profile("Delay NOTE")?;
        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE",
        )?;
        let (note_index, note_offset) =
            self.unique_control_parameter(delay_model, 0x0020_0000, false, "Delay NOTE")?;
        let old_note = match self.models[delay_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        if DelayNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Delay NOTE is outside 0..=13"));
        }

        let mod_model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE context",
        )?;
        let (mod_note_index, _) =
            self.unique_control_parameter(mod_model, 0x0020_0000, false, "Mod NOTE context")?;
        let mod_note = match self.models[mod_model].parameters[mod_note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => return Err(ToneError::format("Mod NOTE context is not an integer PARM")),
        };
        if mod_note != 0 {
            return Err(ToneError::format(
                "Delay NOTE edit requires Mod NOTE Off to avoid the coupled Mod SPEED write",
            ));
        }

        let delay_time_target = if choice != DelayNoteChoice::Off {
            Some(self.unique_control_parameter(delay_model, 0x0010_0000, true, "Delay TIME")?)
        } else {
            None
        };
        let derived_bits = if let Some((time_index, _)) = delay_time_target {
            let old_time = self.float_parameter_bits(delay_model, time_index, "Delay TIME")?;
            if !old_time.is_finite() || !(0.0..=1.0).contains(&old_time) {
                return Err(ToneError::format(
                    "stored Delay TIME is outside finite raw range 0..=1",
                ));
            }
            let system_model =
                self.unique_control_model(&[0x0001_0000], &[0x3e70_0000], "System Tempo context")?;
            let (tempo_index, _) = self.unique_control_parameter(
                system_model,
                0x0020_0001,
                true,
                "System Tempo context",
            )?;
            let tempo =
                self.float_parameter_bits(system_model, tempo_index, "System Tempo context")?;
            if !tempo.is_finite() || !(30.0..=240.0).contains(&tempo) {
                return Err(ToneError::format(
                    "stored Tempo is outside finite BPM range 30..=240",
                ));
            }
            Some(delay_note_time_raw(choice, tempo))
        } else {
            None
        };

        if old_note == choice.value()
            && derived_bits.is_none_or(|bits| {
                self.float_parameter_bits(delay_model, delay_time_target.unwrap().0, "Delay TIME")
                    .is_ok_and(|old| old.to_bits() == bits)
            })
        {
            return Ok(());
        }
        self.write_parameter_bits(delay_model, note_index, note_offset, choice.value());
        if let (Some(bits), Some((time_index, offset))) = (derived_bits, delay_time_target) {
            self.write_parameter_bits(delay_model, time_index, offset, bits);
        }
        Ok(())
    }

    /// Apply a displayed Delay TIME in milliseconds. The explicit TIME event
    /// clears a positive NOTE; it deliberately has no Mod or Tempo dependency.
    pub fn set_delay_time_ms(&mut self, milliseconds: f32) -> Result<(), ToneError> {
        self.require_control_edit_profile("Delay TIME")?;
        if !milliseconds.is_finite() || !(20.0..=2000.0).contains(&milliseconds) {
            return Err(ToneError::format(
                "Delay TIME edit requires finite displayed milliseconds in 20..=2000",
            ));
        }
        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay TIME",
        )?;
        let (time_index, time_offset) =
            self.unique_control_parameter(delay_model, 0x0010_0000, true, "Delay TIME")?;
        let old_time = self.float_parameter_bits(delay_model, time_index, "Delay TIME")?;
        if !old_time.is_finite() || !(0.0..=1.0).contains(&old_time) {
            return Err(ToneError::format(
                "stored Delay TIME is outside finite raw range 0..=1",
            ));
        }
        let (note_index, note_offset) =
            self.unique_control_parameter(delay_model, 0x0020_0000, false, "Delay NOTE")?;
        let old_note = match self.models[delay_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        if DelayNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Delay NOTE is outside 0..=13"));
        }
        let new_time = delay_time_raw(milliseconds);
        if old_time.to_bits() == new_time && old_note == 0 {
            return Ok(());
        }
        self.write_parameter_bits(delay_model, time_index, time_offset, new_time);
        if old_note != 0 {
            self.write_parameter_bits(delay_model, note_index, note_offset, 0);
        }
        Ok(())
    }

    fn require_control_edit_profile(&self, label: &str) -> Result<(), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(format!(
                "{label} requires a PODxt Live PINF target"
            )));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(format!(
                "{label} requires the PODxt Live host profile minimum 3.00"
            )));
        }
        Ok(())
    }

    fn unique_control_model(
        &self,
        slots: &[u32],
        model_ids: &[u32],
        label: &str,
    ) -> Result<usize, ToneError> {
        self.require_control_edit_profile(label)?;
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| slots.contains(&model.slot))
            .map(|(index, _)| index)
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{label} requires exactly one model across slots {slots:02x?}"
            )));
        };
        let index = *index;
        if !model_ids.contains(&self.models[index].model_id) {
            return Err(ToneError::format(format!(
                "{label} model is outside the source-backed host profile"
            )));
        }
        if self.models[index]
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
        {
            return Err(ToneError::format(format!(
                "opaque PARM in {label} model makes control context ambiguous"
            )));
        }
        Ok(index)
    }

    fn unique_control_parameter(
        &self,
        model: usize,
        id: u32,
        float: bool,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        let matches: Vec<_> = self.models[model]
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| {
                matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)
                    .then_some(index)
            })
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{label} requires exactly one PARM 0x{id:06x}"
            )));
        };
        let index = *index;
        let ModelParameter::Recorded { prefix, value, .. } = &self.models[model].parameters[index]
        else {
            unreachable!()
        };
        if *prefix != 0x3f {
            return Err(ToneError::format(format!(
                "{label} target prefix is not canonical 0x3f"
            )));
        }
        let type_ok = if float {
            matches!(value, ParameterValue::Float { .. })
        } else {
            matches!(value, ParameterValue::Int { .. })
        };
        if !type_ok {
            return Err(ToneError::format(format!(
                "{label} target has an unsupported value type"
            )));
        }
        let offset = self
            .parameter_value_offsets
            .get(model)
            .and_then(|offsets| offsets.get(index))
            .copied()
            .flatten()
            .ok_or_else(|| {
                ToneError::format(format!("{label} target has no unique raw value word"))
            })?;
        Ok((index, offset))
    }

    fn float_parameter_bits(
        &self,
        model: usize,
        parameter: usize,
        label: &str,
    ) -> Result<f32, ToneError> {
        match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => Ok(f32::from_bits(bits)),
            _ => Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            ))),
        }
    }

    fn write_parameter_bits(&mut self, model: usize, parameter: usize, offset: usize, bits: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
        if let ModelParameter::Recorded { value, .. } =
            &mut self.models[model].parameters[parameter]
        {
            match value {
                ParameterValue::Int { bits: stored } | ParameterValue::Float { bits: stored } => {
                    *stored = bits
                }
                ParameterValue::Other { .. } => unreachable!("validated parameter type"),
            }
        }
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
        self.set_association_bits_at(
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
        self.set_association_bits_at(
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

    fn stomp_whammy_knob_location(
        &self,
        control: StompWhammyKnob,
    ) -> Result<(usize, usize), ToneError> {
        let label = format!("Stomp Whammy {} knob position", control.label());
        if self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count()
            != 1
        {
            return Err(ToneError::format(format!(
                "{label} requires one unambiguous existing Stomp slot"
            )));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing Stomp slot")))?;
        if model_id != STOMP_WHAMMY_INTERVAL_MODEL_ID {
            return Err(ToneError::format(format!(
                "{label} applies only to Stomp model 0x{STOMP_WHAMMY_INTERVAL_MODEL_ID:08x}"
            )));
        }
        let expected_param = match control {
            StompWhammyKnob::Heel => STOMP_WHAMMY_HEEL_PARAM,
            StompWhammyKnob::Toe => STOMP_WHAMMY_TOE_PARAM,
        };
        debug_assert_eq!(control.param_id(), expected_param);
        self.control_location(
            STOMP_SLOT,
            &[STOMP_WHAMMY_INTERVAL_MODEL_ID],
            expected_param,
            &label,
        )
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

    fn stomp_wave_location(&self) -> Result<(usize, usize), ToneError> {
        let slot_count = self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count();
        if slot_count != 1 {
            return Err(ToneError::format(
                "Stomp SYNWAVE requires one unambiguous existing model slot",
            ));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format("Stomp SYNWAVE requires an existing model slot"))?;
        if !STOMP_SYNTH_WAVE_MODEL_IDS.contains(&model_id) {
            return Err(ToneError::format(format!(
                "Stomp SYNWAVE source mapping does not apply to model 0x{model_id:08x}"
            )));
        }
        self.control_location(
            STOMP_SLOT,
            &[model_id],
            STOMP_SYNTH_WAVE_PARAM,
            "Stomp SYNWAVE",
        )
    }

    fn stomp_harmony_location(
        &self,
        control: StompHarmonyControl,
    ) -> Result<(usize, usize), ToneError> {
        let model_index = self
            .models
            .iter()
            .position(|model| model.slot == STOMP_SLOT)
            .ok_or_else(|| ToneError::format("Stomp harmony requires an existing model slot"))?;
        if self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count()
            != 1
            || self.models[model_index].model_id != 0x020a_0009
        {
            return Err(ToneError::format(
                "Stomp harmony requires one existing model 0x020a0009 slot",
            ));
        }
        let parameter_id = control.parameter_id();
        let target_count = self.models[model_index]
            .parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter,
                    ModelParameter::Recorded { id, .. } if *id == parameter_id
                )
            })
            .count();
        if target_count != 1 {
            return Err(ToneError::format(format!(
                "Stomp harmony requires exactly one existing PARM 0x{parameter_id:08x}"
            )));
        }
        self.control_location(STOMP_SLOT, &[0x020a_0009], parameter_id, "Stomp harmony")
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

    fn set_association_bits_at(
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

    fn cab_edit_location(&self) -> Result<(usize, Range<usize>), ToneError> {
        if self.container != ContainerKind::L6as {
            return Err(ToneError::format(
                "CAB/MIC editing requires a FORM/L6AS tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID || self.device_version < PODXT_LIVE_HOST_MIN_VERSION
        {
            return Err(ToneError::format(
                "CAB/MIC editing requires the PODxt Live target and host profile",
            ));
        }
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, m)| m.slot == CAB_SLOT)
            .map(|(i, _)| i)
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(
                "CAB/MIC edit requires exactly one existing CAB slot",
            ));
        };
        let index = *index;
        let range = self
            .model_minfo_ranges
            .get(index)
            .cloned()
            .ok_or_else(|| ToneError::format("CAB MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "CAB edit requires an existing MINF body of at least 12 bytes",
            ));
        }
        Ok((index, range))
    }

    /// Apply one source-admitted unconditional CAB id to the existing L6AS slot.
    pub fn set_cab_model_choice(&mut self, choice: PodCabModelChoice) -> Result<(), ToneError> {
        let (index, range) = self.cab_edit_location()?;
        let id = choice.model_id();
        if self.models[index].model_id != id {
            self.bytes[range.start..range.start + 4].copy_from_slice(&id.to_be_bytes());
            self.models[index].model_id = id;
        }
        Ok(())
    }

    /// Classify the stored CAB using the source's exact bass list.
    pub fn cab_class(&self) -> Option<PodCabClass> {
        let id = self.cab_model_id()?;
        (PODXT_CAB_MODEL_IDS.contains(&id) || PODXT_BASS_CAB_MODEL_IDS.contains(&id)).then_some(
            if PODXT_BASS_CAB_MODEL_IDS.contains(&id) {
                PodCabClass::Bass
            } else {
                PodCabClass::Other
            },
        )
    }

    /// Return a currently interpretable MIC integer; unknown raw integers remain preserved.
    pub fn mic_choice(&self) -> Option<PodMicChoice> {
        let (mi, _) = self.cab_edit_location().ok()?;
        let cab_id = self.models[mi].model_id;
        if !PODXT_CAB_MODEL_IDS.contains(&cab_id) && !PODXT_BASS_CAB_MODEL_IDS.contains(&cab_id) {
            return None;
        }
        let matches: Vec<_> = self.opaque_parameter_id_hints[mi]
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (*id == Some(PODXT_MIC_PARAM)).then_some(i))
            .collect();
        let [pi] = matches.as_slice() else {
            return None;
        };
        match &self.models[mi].parameters[*pi] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Int { bits },
                ..
            } => PodMicChoice::from_value(*bits),
            _ => None,
        }
    }

    /// Explicitly apply one MIC value. CAB selector changes never call this operation.
    pub fn set_mic_choice(&mut self, choice: PodMicChoice) -> Result<(), ToneError> {
        let (mi, _) = self.cab_edit_location()?;
        let id = self.models[mi].model_id;
        if !PODXT_CAB_MODEL_IDS.contains(&id) && !PODXT_BASS_CAB_MODEL_IDS.contains(&id) {
            return Err(ToneError::format(
                "MIC editing requires a source-listed current CAB id",
            ));
        }
        let matches: Vec<_> = self.opaque_parameter_id_hints[mi]
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (*id == Some(PODXT_MIC_PARAM)).then_some(i))
            .collect();
        let [pi] = matches.as_slice() else {
            return Err(ToneError::format(
                "MIC edit requires exactly one recorded PARM id 0",
            ));
        };
        let pi = *pi;
        match &self.models[mi].parameters[pi] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Int { .. },
                ..
            } => {}
            _ => {
                return Err(ToneError::format(
                    "MIC edit requires a canonical integer PARM",
                ));
            }
        }
        let offset = self.parameter_value_offsets[mi][pi]
            .ok_or_else(|| ToneError::format("MIC PARM has no unique value word"))?;
        let value = choice.value();
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Int { bits },
            ..
        } = &mut self.models[mi].parameters[pi]
        {
            *bits = value;
        }
        Ok(())
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

    pub fn set_model_enabled(
        &mut self,
        index: usize,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_model_enabled(control, enabled)
    }

    pub fn set_amp_model_choice(
        &mut self,
        index: usize,
        choice: PodAmpModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_amp_model_choice(choice)
    }

    pub fn set_delay_model_choice(
        &mut self,
        index: usize,
        choice: PodDelayModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_model_choice(choice)
    }

    pub fn set_mod_model_choice(
        &mut self,
        index: usize,
        choice: PodModModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_model_choice(choice)
    }

    pub fn set_stomp_model_choice(
        &mut self,
        index: usize,
        choice: PodStompModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_model_choice(choice)
    }

    pub fn set_cab_model_choice(
        &mut self,
        index: usize,
        choice: PodCabModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_cab_model_choice(choice)
    }
    pub fn set_mic_choice(&mut self, index: usize, choice: PodMicChoice) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mic_choice(choice)
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

    pub fn stomp_whammy_knob_position(
        &self,
        index: usize,
        control: StompWhammyKnob,
    ) -> Result<f32, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_whammy_knob_position(control)
    }

    pub fn set_stomp_whammy_knob_position(
        &mut self,
        index: usize,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_whammy_knob_position(control, position)
    }

    pub fn stomp_wave_selection(&self, index: usize) -> Result<StompWaveValue, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_wave_selection()
    }

    pub fn set_stomp_wave_selection(
        &mut self,
        index: usize,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_wave_selection(selection)
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_harmony_1m335_selection()
    }
    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_harmony_1m335_selection(choice)
    }
    pub fn stomp_harmony_1457_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_harmony_1457_selection()
    }
    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_harmony_1457_selection(choice)
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

    pub fn set_delay_note_choice(
        &mut self,
        index: usize,
        choice: DelayNoteChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_note_choice(choice)
    }

    pub fn set_mod_note_choice(
        &mut self,
        index: usize,
        choice: ModNoteChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_note_choice(choice)
    }

    pub fn set_mod_speed_hz(&mut self, index: usize, hz: f32) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_speed_hz(hz)
    }

    pub fn set_delay_time_ms(&mut self, index: usize, milliseconds: f32) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_time_ms(milliseconds)
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

    pub fn set_podxt_eq_band(
        &mut self,
        index: usize,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_podxt_eq_band(band, frequency_hz, gain_db)
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
    collection: bool,
    banks: Vec<PodBundleBank>,
}

#[derive(Clone, Debug)]
struct PodBundleBank {
    name: String,
    entry_ranges: Vec<Range<usize>>,
    children: Vec<PodBundleChild>,
    folder: bool,
    chunk_start: usize,
    data_length: usize,
}

#[derive(Clone, Debug)]
struct PodBundleChild {
    range: Range<usize>,
    tone: bool,
}

fn span_identities(spans: &[Range<usize>]) -> Vec<(usize, usize)> {
    let mut identities: Vec<_> = spans.iter().map(|span| (span.start, span.end)).collect();
    identities.sort_unstable();
    identities
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
        let collection = &bytes[8..12] == b"L6CO";
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
            let mut child_spans = Vec::with_capacity(children.len());
            for child in &children {
                let payload_end = child.data.end;
                let full_end = payload_end + (child.data.len() & 1);
                let full_range = child.data.start - 8..full_end;
                let is_tone = preset::container(&bytes, child, b"L6PA")
                    || preset::container(&bytes, child, b"L6FS")
                    || preset::container(&bytes, child, b"L6AS");
                if is_tone {
                    let tone_range = child.data.start - 8..payload_end;
                    PodTone::parse(bytes[tone_range].to_vec())?;
                    tone_count += 1;
                    entry_ranges.push(full_range.clone());
                }
                child_spans.push(PodBundleChild {
                    range: full_range,
                    tone: is_tone,
                });
            }
            banks.push(PodBundleBank {
                name,
                entry_ranges,
                children: child_spans,
                folder: preset::container(&bytes, bank, b"FLDR"),
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
            collection,
            banks,
        })
    }

    /// Membership edits are admitted only for existing folders in collections.
    pub fn membership_editable(&self, bank: usize) -> bool {
        self.collection && self.banks.get(bank).is_some_and(|bank| bank.folder)
    }

    /// Whether a bank is within the admitted fixed-bank tone reorder subset.
    /// This is an offline file operation, not a physical slot-layout claim.
    pub fn fixed_bank_reorder_editable(&self, bank: usize) -> bool {
        self.fixed_bank_tone_spans(bank).is_ok()
    }

    /// Move one existing PODxt Live tone to a final slot index in one fixed
    /// L6BA/BANK. Complete tone records are permuted only among their original
    /// token positions; opaque siblings and all enclosing bytes stay in place.
    pub fn reorder_fixed_bank_entry(
        &mut self,
        bank_index: usize,
        from: usize,
        to: usize,
    ) -> Result<usize, ToneError> {
        let mut tone_order = self.fixed_bank_tone_spans(bank_index)?;
        if from >= tone_order.len() || to >= tone_order.len() {
            return Err(ToneError::format("preset index is outside the fixed bank"));
        }
        if from == to {
            return Ok(self.flat_index(bank_index, to));
        }

        let original_ids = span_identities(&tone_order);
        let moved = tone_order.remove(from);
        tone_order.insert(to, moved);
        if span_identities(&tone_order) != original_ids {
            return Err(ToneError::format(
                "fixed-bank reorder must preserve every original tone exactly once",
            ));
        }

        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the file"))?;
        let mut next_tone = tone_order.iter();
        let mut replacement_children = Vec::with_capacity(bank.data_length - 4);
        for child in &bank.children {
            if child.tone {
                let range = next_tone.next().ok_or_else(|| {
                    ToneError::format("fixed-bank reorder has an invalid tone ordering")
                })?;
                replacement_children.extend_from_slice(&self.bytes[range.clone()]);
            } else {
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
            }
        }
        if next_tone.next().is_some() || replacement_children.len() != bank.data_length - 4 {
            return Err(ToneError::format(
                "fixed-bank reorder spans do not preserve the bank length",
            ));
        }

        let child_start = bank.chunk_start + 12;
        let child_end = bank.chunk_start + 8 + bank.data_length;
        if child_end.checked_sub(child_start) != Some(replacement_children.len()) {
            return Err(ToneError::format(
                "fixed-bank reorder has inconsistent child spans",
            ));
        }
        let mut candidate = self.bytes.clone();
        candidate[child_start..child_end].copy_from_slice(&replacement_children);

        // Parse the complete candidate before publishing any state. The saved
        // baseline is deliberately retained so moving back can become clean.
        let saved = self.saved.clone();
        let mut next = Self::parse(candidate)?;
        next.fixed_bank_tone_spans(bank_index)?;
        if next.preset_count(bank_index) != Some(tone_order.len()) {
            return Err(ToneError::format(
                "fixed-bank reorder changed the selected bank's tone count",
            ));
        }
        for (slot, expected) in tone_order.iter().enumerate() {
            let actual = &next.banks[bank_index].entry_ranges[slot];
            let expected_end =
                expected.start + 8 + preset::word(&self.bytes, expected.start + 4) as usize;
            if next.tone(bank_index, slot)?.as_bytes() != &self.bytes[expected.start..expected_end]
            {
                return Err(ToneError::format(
                    "fixed-bank reorder candidate did not preserve the requested tone order",
                ));
            }
            if actual.end - actual.start != expected.end - expected.start {
                return Err(ToneError::format(
                    "fixed-bank reorder changed a tone token span length",
                ));
            }
        }
        next.saved = saved;
        *self = next;
        Ok(self.flat_index(bank_index, to))
    }

    fn fixed_bank_tone_spans(&self, bank_index: usize) -> Result<Vec<Range<usize>>, ToneError> {
        if self.collection {
            return Err(ToneError::format(
                "fixed-bank reorder requires an L6BA root",
            ));
        }
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the file"))?;
        if bank.folder {
            return Err(ToneError::format(
                "fixed-bank reorder requires an existing BANK child",
            ));
        }
        if bank.entry_ranges.is_empty() {
            return Err(ToneError::format(
                "fixed-bank reorder requires a nonempty bank",
            ));
        }
        for index in 0..bank.entry_ranges.len() {
            let tone = self.tone(bank_index, index)?;
            if tone.container() != ContainerKind::L6pa || tone.device_id() != PODXT_LIVE_TONE_ID {
                return Err(ToneError::format(
                    "fixed-bank reorder accepts only existing PODxt Live L6PA tones",
                ));
            }
        }
        Ok(bank.entry_ranges.clone())
    }

    /// Duplicate an existing tone byte-for-byte after its current tone slot.
    /// Returns the new flattened entry index.
    pub fn duplicate_entry(&mut self, bank: usize, preset: usize) -> Result<usize, ToneError> {
        let order = self.membership_order(bank)?;
        let source = order
            .get(preset)
            .cloned()
            .ok_or_else(|| ToneError::format("preset index is outside the folder"))?;
        self.apply_membership_order(bank, order, None, Some(source))?;
        Ok(self.flat_index(bank, preset + 1))
    }

    /// Remove one existing tone. The current library invariant requires at
    /// least one recognized POD tone to remain in the collection.
    pub fn remove_entry(&mut self, bank: usize, preset: usize) -> Result<usize, ToneError> {
        let mut order = self.membership_order(bank)?;
        if preset >= order.len() {
            return Err(ToneError::format("preset index is outside the folder"));
        }
        let old_flat_index = self.flat_index(bank, preset);
        let removed = order.remove(preset);
        if self.entry_count() == 1 {
            return Err(ToneError::format(
                "cannot remove the last POD tone from the collection",
            ));
        }
        self.apply_membership_order(bank, order, Some(removed), None)?;
        Ok(old_flat_index.min(self.entry_count() - 1))
    }

    /// Move an existing tone to a final tone-slot index in the same folder.
    /// Returns the new flattened entry index.
    pub fn reorder_entry(
        &mut self,
        bank: usize,
        from: usize,
        to: usize,
    ) -> Result<usize, ToneError> {
        let mut order = self.membership_order(bank)?;
        if from >= order.len() || to >= order.len() {
            return Err(ToneError::format("preset index is outside the folder"));
        }
        if from != to {
            let tone = order.remove(from);
            order.insert(to, tone);
            self.apply_membership_order(bank, order, None, None)?;
        }
        Ok(self.flat_index(bank, to))
    }

    fn membership_order(&self, bank_index: usize) -> Result<Vec<Range<usize>>, ToneError> {
        if !self.membership_editable(bank_index) {
            return Err(ToneError::format(
                "POD membership edits require an existing FLDR in an L6CO collection",
            ));
        }
        Ok(self.banks[bank_index]
            .children
            .iter()
            .filter(|child| child.tone)
            .map(|child| child.range.clone())
            .collect())
    }

    fn apply_membership_order(
        &mut self,
        bank_index: usize,
        tone_order: Vec<Range<usize>>,
        remove_tone: Option<Range<usize>>,
        duplicate_after: Option<Range<usize>>,
    ) -> Result<(), ToneError> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the collection"))?;
        if !self.membership_editable(bank_index) {
            return Err(ToneError::format(
                "POD membership edits require an existing FLDR in an L6CO collection",
            ));
        }
        if tone_order.is_empty() && self.entry_count() == bank.entry_ranges.len() {
            return Err(ToneError::format(
                "cannot remove the last POD tone from the collection",
            ));
        }

        let child_length = bank.data_length - 4;
        let (new_child_length, new_file_length) = if let Some(removed) = &remove_tone {
            let delta = removed.len();
            (
                child_length
                    .checked_sub(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
                self.bytes
                    .len()
                    .checked_sub(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
            )
        } else if let Some(duplicate) = &duplicate_after {
            let delta = duplicate.len();
            (
                child_length
                    .checked_add(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
                self.bytes
                    .len()
                    .checked_add(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
            )
        } else {
            (child_length, self.bytes.len())
        };
        if new_file_length as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        let new_data_length = 4usize
            .checked_add(new_child_length)
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        let bank_length = u32::try_from(new_data_length)
            .map_err(|_| ToneError::format("folder exceeds IFF size capacity"))?;
        let root_length = u32::try_from(new_file_length - 8)
            .map_err(|_| ToneError::format("bundle exceeds IFF size capacity"))?;

        let mut replacement_children = Vec::with_capacity(new_child_length);
        let mut next_tone = tone_order.iter();
        if let Some(removed) = remove_tone {
            for child in &bank.children {
                if child.tone && child.range == removed {
                    continue;
                }
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
            }
        } else if let Some(duplicate) = duplicate_after {
            for child in &bank.children {
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                if child.tone && child.range == duplicate {
                    replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                }
            }
        } else {
            for child in &bank.children {
                if child.tone {
                    let range = next_tone.next().ok_or_else(|| {
                        ToneError::format("membership edit has an invalid tone ordering")
                    })?;
                    replacement_children.extend_from_slice(&self.bytes[range.clone()]);
                } else {
                    replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                }
            }
            if next_tone.next().is_some() {
                return Err(ToneError::format(
                    "membership edit has an invalid tone ordering",
                ));
            }
        }

        if replacement_children.len() != new_child_length {
            return Err(ToneError::format(
                "membership span lengths are inconsistent",
            ));
        }
        let child_start = bank.chunk_start + 12;
        let child_end = bank.chunk_start + 8 + bank.data_length;
        let mut bytes = self.bytes.clone();
        bytes.splice(child_start..child_end, replacement_children);
        bytes[bank.chunk_start + 4..bank.chunk_start + 8]
            .copy_from_slice(&bank_length.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_length.to_be_bytes());
        let saved = self.saved.clone();
        let mut next = Self::parse(bytes)?;
        next.saved = saved;
        *self = next;
        Ok(())
    }

    fn flat_index(&self, bank_index: usize, preset_index: usize) -> usize {
        self.banks[..bank_index]
            .iter()
            .map(|bank| bank.entry_ranges.len())
            .sum::<usize>()
            + preset_index
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
        let original = tone.as_bytes().to_vec();
        edit(&mut tone)?;
        if tone.as_bytes() == original {
            return Ok(());
        }
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

    pub fn set_delay_note_choice(
        &mut self,
        index: usize,
        choice: DelayNoteChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_note_choice(choice))
    }

    pub fn set_mod_note_choice(
        &mut self,
        index: usize,
        choice: ModNoteChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_note_choice(choice))
    }

    pub fn set_mod_speed_hz(&mut self, index: usize, hz: f32) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_speed_hz(hz))
    }

    pub fn set_delay_time_ms(&mut self, index: usize, milliseconds: f32) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_time_ms(milliseconds))
    }

    pub fn stomp_whammy_knob_position(
        &self,
        index: usize,
        control: StompWhammyKnob,
    ) -> Result<f32, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the bundle"))?;
        self.tone(bank, preset)?.stomp_whammy_knob_position(control)
    }

    pub fn set_stomp_whammy_knob_position(
        &mut self,
        index: usize,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| {
            tone.set_stomp_whammy_knob_position(control, position)
        })
    }

    pub fn set_podxt_eq_band(
        &mut self,
        index: usize,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| {
            tone.set_podxt_eq_band(band, frequency_hz, gain_db)
        })
    }

    pub fn set_model_enabled(
        &mut self,
        index: usize,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_model_enabled(control, enabled))
    }

    pub fn set_amp_model_choice(
        &mut self,
        index: usize,
        choice: PodAmpModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_amp_model_choice(choice))
    }

    pub fn set_delay_model_choice(
        &mut self,
        index: usize,
        choice: PodDelayModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_model_choice(choice))
    }

    pub fn set_mod_model_choice(
        &mut self,
        index: usize,
        choice: PodModModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_model_choice(choice))
    }

    pub fn set_stomp_model_choice(
        &mut self,
        index: usize,
        choice: PodStompModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_model_choice(choice))
    }

    pub fn set_cab_model_choice(
        &mut self,
        index: usize,
        choice: PodCabModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_cab_model_choice(choice))
    }
    pub fn set_mic_choice(&mut self, index: usize, choice: PodMicChoice) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mic_choice(choice))
    }

    pub fn stomp_wave_selection(&self, index: usize) -> Result<StompWaveValue, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_wave_selection()
    }

    pub fn set_stomp_wave_selection(
        &mut self,
        index: usize,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_wave_selection(selection))
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_harmony_1m335_selection()
    }
    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_harmony_1m335_selection(choice))
    }
    pub fn stomp_harmony_1457_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_harmony_1457_selection()
    }
    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_harmony_1457_selection(choice))
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

fn source_range_raw(displayed: f32, minimum: f32, maximum: f32) -> f32 {
    // L6RangeFormat: convert the stored float bounds to double, subtract the
    // minimum in double, but subtract max-min in float before promoting it.
    let numerator = f64::from(displayed) - f64::from(minimum);
    let span: f32 = maximum - minimum;
    (numerator / f64::from(span)) as f32
}

fn delay_time_raw(milliseconds: f32) -> u32 {
    source_range_raw(milliseconds.clamp(20.0, 2000.0), 20.0, 2000.0).to_bits()
}

fn delay_note_time_raw(choice: DelayNoteChoice, tempo: f32) -> u32 {
    let multiplier = choice.multiplier().expect("positive NOTE choice");
    let q = multiplier * 4.0_f32;
    let beat = q / tempo;
    let milliseconds = beat * 1000.0_f32;
    let displayed = milliseconds * 60.0_f32;
    source_range_raw(displayed.clamp(20.0, 2000.0), 20.0, 2000.0).to_bits()
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
fn alter_first_model_minf(bytes: &[u8], duplicate: bool) -> Vec<u8> {
    let root = preset::chunks(bytes, 12..bytes.len()).unwrap();
    let patch = root
        .iter()
        .find(|chunk| preset::container(bytes, chunk, b"PATC"))
        .unwrap();
    let models = preset::chunks(bytes, patch.data.start + 4..patch.data.end).unwrap();
    let model = models
        .iter()
        .find(|chunk| preset::container(bytes, chunk, b"MODL"))
        .unwrap();
    let fields = preset::chunks(bytes, model.data.start + 4..model.data.end).unwrap();
    let minf = fields.iter().find(|chunk| chunk.id == *b"MINF").unwrap();
    let chunk_start = minf.data.start - 8;
    let full_end = minf.data.end + (minf.data.len() & 1);
    let raw = bytes[chunk_start..full_end].to_vec();
    let mut result = bytes.to_vec();
    if duplicate {
        result.splice(model.data.end..model.data.end, raw.iter().copied());
    } else {
        result.drain(chunk_start..full_end);
    }
    let delta = raw.len() as i64 * if duplicate { 1 } else { -1 };
    for length_offset in [model.data.start - 4, patch.data.start - 4, 4] {
        let old = u32::from_be_bytes(result[length_offset..length_offset + 4].try_into().unwrap());
        result[length_offset..length_offset + 4]
            .copy_from_slice(&u32::try_from(i64::from(old) + delta).unwrap().to_be_bytes());
    }
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::pod_assets::{self, SlotRole};

    fn drive_record(prefix: u8, id: u32, type_word: u32, bits: u32) -> [u8; 12] {
        let mut bytes = [0; 12];
        bytes[..4].copy_from_slice(&(((prefix as u32) << 24) | id).to_be_bytes());
        bytes[4..8].copy_from_slice(&type_word.to_be_bytes());
        bytes[8..12].copy_from_slice(&bits.to_be_bytes());
        bytes
    }

    fn int_parameter(id: u32, bits: u32, prefix: u8) -> [u8; 12] {
        drive_record(prefix, id, 0, bits)
    }

    fn typed_int_parameter(id: u32, bits: u32, prefix: u8, type_word: u32) -> [u8; 12] {
        drive_record(prefix, id, type_word, bits)
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

    fn stomp_wave_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: model,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn delay_note_fixture(
        delay_slot: u32,
        delay_model: u32,
        delay_note: u32,
        delay_time: u32,
        mod_slot: Option<u32>,
        mod_model: u32,
        mod_note: u32,
        tempo: Option<f32>,
    ) -> Vec<u8> {
        let delay_note_row = drive_record(0x3f, 0x0020_0000, 0, delay_note);
        let delay_time_row = drive_record(0x3f, 0x0010_0000, 1, delay_time);
        let mod_note_row = drive_record(0x3f, 0x0020_0000, 0, mod_note);
        let mod_speed_row = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
        let tempo_row = tempo.map(|value| drive_record(0x3f, 0x0020_0001, 1, value.to_bits()));
        let sentinel = drive_record(0x3f, 0x007a_bcde, 0x7f, 0xdead_beef);
        let delay_rows = [&delay_time_row[..], &delay_note_row[..], &sentinel[..]];
        let mod_rows = [&mod_speed_row[..], &mod_note_row[..]];
        let tempo_rows = tempo_row.as_ref().map(|row| [row.as_slice()]);
        let mut models = vec![BuiltModel {
            slot: delay_slot,
            model_id: delay_model,
            tail: &[1, 2, 3, 4],
            parameters: &delay_rows,
        }];
        if let Some(slot) = mod_slot {
            models.push(BuiltModel {
                slot,
                model_id: mod_model,
                tail: &[5, 6, 7, 8],
                parameters: &mod_rows,
            });
        }
        if tempo_row.is_some() {
            models.push(BuiltModel {
                slot: 0x0001_0000,
                model_id: 0x3e70_0000,
                tail: &[9, 10, 11, 12],
                parameters: tempo_rows.as_ref().unwrap(),
            });
        }
        custom_tone(ContainerKind::L6as, &models)
    }

    fn delay_context_fixture(
        models: &[(u32, u32, Vec<Vec<u8>>)],
        target: u32,
        version: u32,
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
        info[4..8].copy_from_slice(&target.to_be_bytes());
        info[8..10].copy_from_slice(&u16::from(b'Z').to_be_bytes());
        info[72..76].copy_from_slice(&version.to_be_bytes());
        info[76..80].copy_from_slice(&[0x5a, 0xa5, 0x96, 0x69]);

        let mut patch = b"PATC".to_vec();
        for (index, (slot, model_id, parameters)) in models.iter().enumerate() {
            let mut minf = Vec::new();
            minf.extend_from_slice(&model_id.to_be_bytes());
            minf.extend_from_slice(&slot.to_be_bytes());
            minf.extend_from_slice(&[0x11 + index as u8, 0x22, 0x33, 0x44]);
            let mut model = b"MODL".to_vec();
            model.extend(chunk(b"MINF", &minf));
            for parameter in parameters {
                model.extend(chunk(b"PARM", parameter));
            }
            model.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
            patch.extend(chunk(b"LIST", &model));
        }
        let mut data = b"L6AS".to_vec();
        data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    fn mod_operation_fixture(models: &[(u32, u32, Vec<Vec<u8>>)]) -> Vec<u8> {
        delay_context_fixture(models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION)
    }

    fn mod_test_rows(note: u32, speed_bits: u32) -> Vec<Vec<u8>> {
        vec![
            drive_record(0x3f, 0x0010_0000, 1, speed_bits).to_vec(),
            drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
            drive_record(0x3f, 0x007a_bcde, 0x7f, 0xfeed_cafe).to_vec(),
        ]
    }

    fn mod_test_delay_rows(note: u32) -> Vec<Vec<u8>> {
        vec![
            drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
            drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
            drive_record(0x3f, 0x007a_bcde, 0x7f, 0xcafe_feed).to_vec(),
        ]
    }

    fn mod_test_system_rows(tempo: f32) -> Vec<Vec<u8>> {
        vec![drive_record(0x3f, 0x0020_0001, 1, tempo.to_bits()).to_vec()]
    }

    fn patch_expected_parameter(
        tone: &PodTone,
        expected: &mut [u8],
        slot: u32,
        id: u32,
        bits: u32,
    ) {
        let offset = parameter_word0_offset(tone, slot, id) + 8;
        expected[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
    }

    fn parm_bits(tone: &PodTone, slot: u32, id: u32) -> u32 {
        let model = tone
            .models()
            .iter()
            .find(|model| model.slot == slot)
            .unwrap();
        model
            .parameters
            .iter()
            .find_map(|parameter| match parameter {
                ModelParameter::Recorded {
                    id: found,
                    value: ParameterValue::Int { bits } | ParameterValue::Float { bits },
                    ..
                } if *found == id => Some(*bits),
                _ => None,
            })
            .unwrap()
    }

    fn parameter_word0_offset(tone: &PodTone, slot: u32, id: u32) -> usize {
        let model_index = tone
            .models
            .iter()
            .position(|model| model.slot == slot)
            .unwrap();
        let parameter_index = tone.models[model_index].parameters.iter().position(|parameter| {
            matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)
        }).unwrap();
        tone.parameter_value_offsets[model_index][parameter_index].unwrap() - 8
    }

    fn stomp_harmony_tone_with_models(
        models: &[BuiltModel<'_>],
        target: u32,
        version: u32,
    ) -> Vec<u8> {
        let mut bytes = custom_tone(ContainerKind::L6as, models);
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 4..pinf + 8].copy_from_slice(&target.to_be_bytes());
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        bytes
    }

    fn stomp_harmony_tone(bits1: u32, bits2: u32) -> Vec<u8> {
        let p1 = drive_record(0x3f, 0x0010_0001, 1, bits1);
        let p2 = drive_record(0x3f, 0x0010_0002, 1, bits2);
        stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[&p1, &p2],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        )
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

    fn membership_bundle(
        root_kind: &[u8; 4],
        fixed_tones: &[Vec<u8>],
        folder_children: &[Vec<u8>],
    ) -> Vec<u8> {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        fn bank(kind: &[u8; 4], name: u16, children: &[Vec<u8>]) -> Vec<u8> {
            let mut data = kind.to_vec();
            let mut info = vec![0; 68];
            info[4..6].copy_from_slice(&name.to_be_bytes());
            data.extend(chunk(b"BINF", &info));
            for child in children {
                data.extend_from_slice(child);
            }
            chunk(b"LIST", &data)
        }

        let mut root = root_kind.to_vec();
        root.extend(chunk(b"JUNK", &[0x91, 0x92, 0x93]));
        root.extend(bank(b"BANK", b'A' as u16, fixed_tones));
        root.extend(bank(b"FLDR", b'B' as u16, folder_children));
        root.extend(bank(
            b"FLDR",
            b'C' as u16,
            &[chunk(b"SIDE", &[0x31, 0x32, 0x33])],
        ));
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

    fn whammy_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
        stomp_tone(version, model, parameters)
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

    fn eq_tone(version: u32, model: u32, slot: u32, parameters: &[&[u8]]) -> Vec<u8> {
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot,
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

    fn enable_tone(amp_model: u32, eq_model: u32, amp_flag: u8, eq_flag: u8) -> Vec<u8> {
        custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: amp_model,
                    tail: &[0x80, 0xa1, 0xb2, amp_flag],
                    parameters: &[&[0xde, 0xad, 0xbe]],
                },
                BuiltModel {
                    slot: PODXT_EQ_SLOT,
                    model_id: eq_model,
                    tail: &[0xff, 0xc3, 0xd4, eq_flag],
                    parameters: &[],
                },
            ],
        )
    }

    fn short_enable_tone(minf_len: usize, following_byte: u8) -> Vec<u8> {
        assert!((8..12).contains(&minf_len));
        let tail = vec![0; minf_len - 8];
        let mut bytes = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xdead_beef,
                    tail: &tail,
                    parameters: &[],
                },
                BuiltModel {
                    slot: PODXT_EQ_SLOT,
                    model_id: PODXT_EQ_MODEL_ID,
                    tail: &tail,
                    parameters: &[],
                },
            ],
        );
        for (model_id, slot) in [(0xdead_beef, AMP_SLOT), (PODXT_EQ_MODEL_ID, PODXT_EQ_SLOT)] {
            let mut marker = model_id.to_be_bytes().to_vec();
            marker.extend_from_slice(&slot.to_be_bytes());
            let body = bytes
                .windows(marker.len())
                .position(|window| window == marker)
                .unwrap();
            // For short MINFs this absolute position is pad or following
            // chunk data. Make it look canonical to prove it is not admitted.
            bytes[body + 11] = following_byte;
        }
        bytes
    }

    #[test]
    fn amp_model_apply_changes_only_the_existing_amp_minf_id_and_noops_same_choice() {
        let opaque_parameter = [0xde, 0xad, 0xbe];
        let unusual_parameter = [
            0xff, 0x12, 0x34, 0x56, 0x7f, 0x00, 0x00, 0x01, 0x7f, 0xc0, 0x00, 0x01,
        ];
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                // Signed ordinal, reserved bytes and noncanonical enabled byte.
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[&opaque_parameter, &unusual_parameter],
            }],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.models()[0].ordinal, Some(-128));
        assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
        assert_eq!(tone.models()[0].enabled, Some(false));

        let id_offset = tone.model_minfo_ranges[0].start;
        tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
        let changed = tone.as_bytes().to_vec();
        assert_eq!(id_offset, tone.model_minfo_ranges[0].start);
        for index in 0..original.len() {
            if !(id_offset..id_offset + 4).contains(&index) {
                assert_eq!(changed[index], original[index], "changed byte {index:#x}");
            }
        }
        assert_eq!(
            &changed[id_offset..id_offset + 4],
            &PodAmpModelChoice::NoAmp.model_id().to_be_bytes()
        );
        assert_eq!(
            tone.amp_model_id(),
            Some(PodAmpModelChoice::NoAmp.model_id())
        );
        assert_eq!(tone.models()[0].ordinal, Some(-128));
        assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
        assert_eq!(tone.models()[0].enabled, Some(false));
        assert_eq!(
            tone.models()[0].parameters,
            vec![
                ModelParameter::Opaque { len: 3 },
                ModelParameter::Recorded {
                    prefix: 0xff,
                    id: 0x0012_3456,
                    value: ParameterValue::Other {
                        type_word: 0x7f00_0001,
                        bits: 0x7fc0_0001,
                    },
                },
            ]
        );

        tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
            .unwrap();
        let restored_id = tone.model_minfo_ranges[0].start;
        assert_eq!(
            &tone.as_bytes()[restored_id..restored_id + 4],
            &PodAmpModelChoice::Line6Clean.model_id().to_be_bytes()
        );
        let line6_clean = tone.as_bytes().to_vec();
        tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
        let no_amp_id = tone.model_minfo_ranges[0].start;
        assert_eq!(
            &tone.as_bytes()[no_amp_id..no_amp_id + 4],
            &PodAmpModelChoice::NoAmp.model_id().to_be_bytes()
        );
        tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
            .unwrap();
        assert_eq!(tone.as_bytes(), line6_clean);
        let before_noop = tone.as_bytes().to_vec();
        tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
            .unwrap();
        assert_eq!(tone.as_bytes(), before_noop);
        assert_eq!(PodAmpModelChoice::try_from(0xdead_beef), Err(0xdead_beef));
    }

    #[test]
    fn amp_model_apply_uses_the_documented_literal_whole_buffer() {
        let literal = "46 4f 52 4d 00 00 00 aa 4c 36 50 41 4a 55 4e 4b
            00 00 00 03 11 22 33 a5 50 49 4e 46 00 00 00 50
            00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
            00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
            4c 49 53 54 00 00 00 3a 50 41 54 43 4c 49 53 54
            00 00 00 2e 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
            00 02 00 01 00 03 00 00 fe 12 34 01 50 41 52 4d
            00 00 00 03 de ad be a5 58 54 52 41 00 00 00 02
            55 66";
        let original: Vec<u8> = literal
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect();
        assert_eq!(original.len(), 178);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
        let mut expected = original;
        expected[0x93] = 0x00;
        assert_eq!(tone.as_bytes(), expected);
        assert_eq!(tone.amp_model_id(), Some(0x0002_0000));
    }

    #[test]
    fn amp_model_apply_rejects_scope_and_structure_before_mutation() {
        let parameters: [&[u8]; 0] = [];
        for container in [ContainerKind::L6fs, ContainerKind::L6as] {
            let bytes = custom_tone(
                container,
                &[BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xdead_beef,
                    tail: &[0, 0, 0, 1],
                    parameters: &parameters,
                }],
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let before = tone.clone();
            assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
            assert_eq!(tone, before);
        }

        let short_minf = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[],
                parameters: &parameters,
            }],
        );
        let mut tone = PodTone::parse(short_minf).unwrap();
        let before = tone.clone();
        assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
        assert_eq!(tone, before);

        let missing_amp = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let mut tone = PodTone::parse(missing_amp).unwrap();
        let before = tone.clone();
        assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
        assert_eq!(tone, before);

        let mut old_version = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let pinf = find_chunk_data(&old_version, b"PINF");
        old_version[pinf + 72..pinf + 76]
            .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        let mut tone = PodTone::parse(old_version).unwrap();
        let before = tone.clone();
        assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
        assert_eq!(tone, before);

        let wrong_target = {
            let mut bytes = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xdead_beef,
                    tail: &[0, 0, 0, 1],
                    parameters: &parameters,
                }],
            );
            let pinf = find_chunk_data(&bytes, b"PINF");
            bytes[pinf + 4..pinf + 8].copy_from_slice(&0x0007_0001u32.to_be_bytes());
            bytes
        };
        assert!(PodTone::parse(wrong_target).is_err());

        let wrong_layout_version = {
            let mut bytes = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xdead_beef,
                    tail: &[0, 0, 0, 1],
                    parameters: &parameters,
                }],
            );
            let pinf = find_chunk_data(&bytes, b"PINF");
            bytes[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
            bytes
        };
        assert!(PodTone::parse(wrong_layout_version).is_err());

        let duplicate_slot = custom_tone(
            ContainerKind::L6pa,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xdead_beef,
                    tail: &[0, 0, 0, 1],
                    parameters: &parameters,
                },
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 0xfeed_beef,
                    tail: &[0, 0, 0, 1],
                    parameters: &parameters,
                },
            ],
        );
        let duplicate_slot_before = duplicate_slot.clone();
        assert!(PodTone::parse(duplicate_slot.clone()).is_err());
        assert_eq!(duplicate_slot, duplicate_slot_before);

        let mut missing_minf = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let minf = PodTone::parse(missing_minf.clone())
            .unwrap()
            .model_minfo_ranges[0]
            .start;
        missing_minf[minf - 8..minf - 4].copy_from_slice(b"XXXX");
        let missing_minf_before = missing_minf.clone();
        assert!(PodTone::parse(missing_minf.clone()).is_err());
        assert_eq!(missing_minf, missing_minf_before);

        let opaque_parameter = [0xde, 0xad, 0xbe];
        let unusual_parameter = [
            0xff, 0x12, 0x34, 0x56, 0x7f, 0x00, 0x00, 0x01, 0x7f, 0xc0, 0x00, 0x01,
        ];
        let mut duplicate_minf = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[&opaque_parameter, &unusual_parameter],
            }],
        );
        let top = preset::chunks(&duplicate_minf, 12..duplicate_minf.len()).unwrap();
        let patch = preset::unique(&top, b"LIST").unwrap();
        let model_chunks =
            preset::chunks(&duplicate_minf, patch.data.start + 4..patch.data.end).unwrap();
        let model = model_chunks
            .iter()
            .find(|chunk| preset::container(&duplicate_minf, chunk, b"MODL"))
            .unwrap();
        let fields = preset::chunks(&duplicate_minf, model.data.start + 4..model.data.end).unwrap();
        let minf = preset::unique(&fields, b"MINF").unwrap();
        let duplicate_chunk = duplicate_minf[minf.data.start - 8..minf.data.end].to_vec();
        let inserted_len = duplicate_chunk.len();
        let model_length_offset = model.data.start - 4;
        let patch_length_offset = patch.data.start - 4;
        let insert_at = model.data.end;
        duplicate_minf.splice(insert_at..insert_at, duplicate_chunk);
        for offset in [model_length_offset, patch_length_offset, 4] {
            let length = preset::word(&duplicate_minf, offset) as usize;
            duplicate_minf[offset..offset + 4]
                .copy_from_slice(&((length + inserted_len) as u32).to_be_bytes());
        }
        let duplicate_minf_before = duplicate_minf.clone();
        assert!(PodTone::parse(duplicate_minf.clone()).is_err());
        assert_eq!(duplicate_minf, duplicate_minf_before);
    }

    #[test]
    fn amp_model_apply_updates_library_and_bundle_dirty_state_and_revert() {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0x0002_0000,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[],
            }],
        );
        let mut library = Library::default();
        library.import_bytes(original.clone()).unwrap();
        library
            .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
            .unwrap();
        assert!(!library.is_dirty(0));
        library
            .set_amp_model_choice(0, PodAmpModelChoice::Line6Clean)
            .unwrap();
        assert!(library.is_dirty(0));
        assert_eq!(
            library.tone(0).unwrap().amp_model_id(),
            Some(PodAmpModelChoice::Line6Clean.model_id())
        );
        library.revert_unsaved(0).unwrap();
        assert!(!library.is_dirty(0));
        assert_eq!(library.tone(0).unwrap().as_bytes(), original);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        bundle
            .set_amp_model_choice(0, PodAmpModelChoice::Line6Clean)
            .unwrap();
        assert!(bundle.is_dirty());
        assert_eq!(
            bundle.tone(0, 0).unwrap().amp_model_id(),
            Some(PodAmpModelChoice::Line6Clean.model_id())
        );
        bundle.revert().unwrap();
        assert!(!bundle.is_dirty());
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);

        let wrong_container = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[],
            }],
        );
        let mut rejected_library = Library::default();
        rejected_library
            .import_bytes(wrong_container.clone())
            .unwrap();
        assert!(
            rejected_library
                .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
                .is_err()
        );
        assert!(!rejected_library.is_dirty(0));
        assert_eq!(
            rejected_library.tone(0).unwrap().as_bytes(),
            wrong_container
        );

        let mut rejected_bundle =
            PodBundleLibrary::parse(bundle_with_tone(&wrong_container)).unwrap();
        assert!(
            rejected_bundle
                .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
                .is_err()
        );
        assert!(!rejected_bundle.is_dirty());
        assert_eq!(
            rejected_bundle.tone(0, 0).unwrap().as_bytes(),
            wrong_container
        );
    }

    #[test]
    fn cab_mic_edits_use_exact_source_ids_and_patch_only_admitted_words() {
        // Kept independent from the production allowlist to catch omissions and accidental ranges.
        let literal_ids = [
            0x0102_0000,
            0x0102_0001,
            0x0102_0002,
            0x0102_0003,
            0x0102_0004,
            0x0102_0005,
            0x0102_0006,
            0x0102_0007,
            0x0102_0008,
            0x0102_0009,
            0x0102_000a,
            0x0102_000b,
            0x0102_000c,
            0x0102_000d,
            0x0102_000e,
            0x0102_000f,
            0x0102_0010,
            0x0102_0011,
            0x0102_0012,
            0x0102_0013,
            0x0102_0014,
            0x0102_0015,
            0x0102_0016,
            0x0102_0018,
            0x0102_001a,
        ];
        assert_eq!(PODXT_CAB_MODEL_IDS, literal_ids);
        let literal_gated_ids = [
            0x0103_0001,
            0x0103_0002,
            0x0103_0003,
            0x0103_0004,
            0x0103_0005,
            0x0103_0006,
            0x0103_0007,
            0x0103_0008,
            0x0103_0009,
            0x0103_000a,
            0x0103_000b,
            0x0103_000c,
            0x0103_000d,
            0x0103_000e,
            0x0103_000f,
            0x0103_0010,
            0x0103_0011,
            0x0103_0012,
            0x0103_0013,
            0x0103_0014,
            0x0103_0015,
            0x0103_0016,
        ];
        assert_eq!(PODXT_BASS_CAB_MODEL_IDS, literal_gated_ids);
        for id in literal_ids {
            let mic = int_parameter(0, 0xdead_beef, 0x3f);
            let original = custom_tone(
                ContainerKind::L6as,
                &[BuiltModel {
                    slot: CAB_SLOT,
                    model_id: 0xfedc_ba98,
                    tail: &[0x80, 0xa1, 0xb2, 0xfe],
                    parameters: &[&mic],
                }],
            );
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.mic_choice(),
                None,
                "unsupported stored MIC must remain raw"
            );
            let choice = PodCabModelChoice::try_from(id).unwrap();
            let offset = tone.model_minfo_ranges[0].start;
            let mut expected = original.clone();
            expected[offset..offset + 4].copy_from_slice(&id.to_be_bytes());
            tone.set_cab_model_choice(choice).unwrap();
            assert_eq!(
                tone.as_bytes(),
                expected,
                "unexpected write set for CAB {id:08x}"
            );
            assert_eq!(tone.models()[0].model_id, id);
            assert_eq!(tone.models()[0].ordinal, Some(-128));
            assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
            assert_eq!(tone.models()[0].enabled, Some(false));
            tone.set_cab_model_choice(choice).unwrap();
            assert_eq!(tone.as_bytes(), expected, "same CAB choice must be a no-op");
            assert_eq!(
                PodTone::parse(expected.clone()).unwrap().as_bytes(),
                expected
            );
        }
        for id in
            PODXT_BASS_CAB_MODEL_IDS
                .into_iter()
                .chain([0x0102_0017, 0x0102_0019, 0xdead_beef])
        {
            assert!(
                PodCabModelChoice::try_from(id).is_err(),
                "CAB {id:08x} must reject"
            );
        }
    }

    #[test]
    fn mic_edits_cover_both_classes_all_choices_and_exact_value_word() {
        for cab in [PODXT_CAB_MODEL_IDS[0], PODXT_BASS_CAB_MODEL_IDS[0]] {
            for value in 0..=3 {
                let mic = int_parameter(PODXT_MIC_PARAM, 0xffff_fffe, 0x3f);
                let original = custom_tone(
                    ContainerKind::L6as,
                    &[BuiltModel {
                        slot: CAB_SLOT,
                        model_id: cab,
                        tail: &[0, 0, 0, 1],
                        parameters: &[&mic],
                    }],
                );
                let mut tone = PodTone::parse(original.clone()).unwrap();
                assert_eq!(
                    tone.cab_class(),
                    Some(if PODXT_BASS_CAB_MODEL_IDS.contains(&cab) {
                        PodCabClass::Bass
                    } else {
                        PodCabClass::Other
                    })
                );
                assert_eq!(
                    tone.mic_choice(),
                    None,
                    "unknown integer must not display as a valid choice"
                );
                let choice = PodMicChoice::try_from(value).unwrap();
                let offset = tone.parameter_value_offsets[0][0].unwrap();
                let mut expected = original;
                expected[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                tone.set_mic_choice(choice).unwrap();
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "MIC value {value} for CAB {cab:08x}"
                );
                assert_eq!(tone.mic_choice(), Some(choice));
                tone.set_mic_choice(choice).unwrap();
                assert_eq!(tone.as_bytes(), expected, "same MIC choice must be a no-op");
            }
        }
    }

    #[test]
    fn cab_apply_preserves_missing_opaque_and_noncanonical_mic_records() {
        let opaque = [0x3f, 0, 0, 0, 0xaa];
        let noncanonical = int_parameter(0, 2, 0x3e);
        for parameters in [&[][..], &[&opaque[..]][..], &[&noncanonical[..]][..]] {
            let original = custom_tone(
                ContainerKind::L6as,
                &[BuiltModel {
                    slot: CAB_SLOT,
                    model_id: PODXT_BASS_CAB_MODEL_IDS[0],
                    tail: &[0x80, 0xa1, 0xb2, 0xfe],
                    parameters,
                }],
            );
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let mut expected = original;
            let id_offset = tone.model_minfo_ranges[0].start;
            let id = PODXT_CAB_MODEL_IDS[0];
            expected[id_offset..id_offset + 4].copy_from_slice(&id.to_be_bytes());
            tone.set_cab_model_choice(PodCabModelChoice::try_from(id).unwrap())
                .unwrap();
            assert_eq!(tone.as_bytes(), expected);
        }
    }

    #[test]
    fn mic_target_ambiguity_and_inapplicable_cab_reject_atomically() {
        let valid = int_parameter(0, 1, 0x3f);
        let bad_type = typed_int_parameter(0, 1, 0x3f, 7);
        let bad_prefix = int_parameter(0, 1, 0x3e);
        let opaque_target = [0x3f, 0, 0, 0, 0xaa];
        let unrelated_opaque = [0x3f, 0x12, 0x34, 0x56, 0xaa];
        let cases: Vec<Vec<Vec<u8>>> = vec![
            vec![],
            vec![bad_type.to_vec()],
            vec![bad_prefix.to_vec()],
            vec![opaque_target.to_vec()],
            vec![valid.to_vec(), valid.to_vec()],
            vec![unrelated_opaque.to_vec(), valid.to_vec()],
        ];
        for (case_index, records) in cases.into_iter().enumerate() {
            let refs: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
            let original = custom_tone(
                ContainerKind::L6as,
                &[BuiltModel {
                    slot: CAB_SLOT,
                    model_id: PODXT_CAB_MODEL_IDS[0],
                    tail: &[0, 0, 0, 1],
                    parameters: &refs,
                }],
            );
            let mut tone = PodTone::parse(original.clone()).unwrap();
            if case_index == 5 {
                tone.set_mic_choice(PodMicChoice::try_from(3).unwrap())
                    .unwrap();
                continue;
            }
            let before_models = tone.models().to_vec();
            assert!(
                tone.set_mic_choice(PodMicChoice::try_from(3).unwrap())
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), original);
            assert_eq!(tone.models(), before_models);
        }
        let raw = int_parameter(0, 1, 0x3f);
        let unknown_cab = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &[&raw],
            }],
        );
        let mut tone = PodTone::parse(unknown_cab.clone()).unwrap();
        assert_eq!(tone.mic_choice(), None);
        assert!(
            tone.set_mic_choice(PodMicChoice::try_from(2).unwrap())
                .is_err()
        );
        assert_eq!(tone.as_bytes(), unknown_cab);
    }

    #[test]
    fn cab_mic_validation_and_shared_library_bundle_lifecycle_are_atomic() {
        let mic = int_parameter(0, 1, 0x3f);
        let original = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[0],
                tail: &[0, 0, 0, 1],
                parameters: &[&mic],
            }],
        );
        let mut library = Library::default();
        library.import_bytes(original.clone()).unwrap();
        let cab = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[1]).unwrap();
        library.set_cab_model_choice(0, cab).unwrap();
        assert!(library.is_dirty(0));
        library
            .set_mic_choice(0, PodMicChoice::try_from(3).unwrap())
            .unwrap();
        assert_eq!(
            library.tone(0).unwrap().cab_model_id(),
            Some(cab.model_id())
        );
        assert_eq!(library.tone(0).unwrap().mic_choice().unwrap().value(), 3);
        library.note_saved(0).unwrap();
        assert!(!library.is_dirty(0));
        let saved = library.tone(0).unwrap().as_bytes().to_vec();
        assert!(
            library
                .set_mic_choice(9, PodMicChoice::try_from(0).unwrap())
                .is_err()
        );
        assert!(!library.is_dirty(0));
        assert_eq!(library.tone(0).unwrap().as_bytes(), saved);
        library
            .set_cab_model_choice(
                0,
                PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[2]).unwrap(),
            )
            .unwrap();
        assert!(library.is_dirty(0));
        library.revert_unsaved(0).unwrap();
        assert!(!library.is_dirty(0));
        assert_eq!(library.tone(0).unwrap().as_bytes(), saved);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        bundle.set_cab_model_choice(0, cab).unwrap();
        bundle
            .set_mic_choice(0, PodMicChoice::try_from(2).unwrap())
            .unwrap();
        assert!(bundle.is_dirty());
        let reopened = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
        assert_eq!(
            reopened.tone(0, 0).unwrap().cab_model_id(),
            Some(cab.model_id())
        );
        assert_eq!(
            reopened.tone(0, 0).unwrap().mic_choice().unwrap().value(),
            2
        );
        bundle.revert().unwrap();
        assert!(!bundle.is_dirty());
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);
    }

    #[test]
    fn cab_mic_guards_reject_wrong_target_version_container_and_short_or_missing_cab() {
        let id = PODXT_CAB_MODEL_IDS[0];
        let choice = PodCabModelChoice::try_from(id).unwrap();
        let original = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0xdead_beef,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[],
            }],
        );
        let mut old_version = original.clone();
        let pinf = old_version
            .windows(4)
            .position(|window| window == b"PINF")
            .unwrap()
            + 8;
        old_version[pinf + 72..pinf + 76]
            .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        let mut tone = PodTone::parse(old_version.clone()).unwrap();
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), old_version);
        let mut wrong_target = original.clone();
        let pinf = wrong_target
            .windows(4)
            .position(|window| window == b"PINF")
            .unwrap()
            + 8;
        wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000b_u32.to_be_bytes());
        assert!(matches!(
            PodTone::parse(wrong_target),
            Err(ToneError::WrongTarget { .. })
        ));
        let mut wrong_container = original.clone();
        wrong_container[8..12].copy_from_slice(b"L6PA");
        let mut tone = PodTone::parse(wrong_container.clone()).unwrap();
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), wrong_container);

        let short = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0xdead_beef,
                tail: &[0x80, 0xa1, 0xb2],
                parameters: &[],
            }],
        );
        let mut tone = PodTone::parse(short.clone()).unwrap();
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), short);

        let missing = custom_tone(ContainerKind::L6as, &[]);
        let mut tone = PodTone::parse(missing.clone()).unwrap();
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), missing);

        let duplicate = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: CAB_SLOT,
                    model_id: 1,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
                BuiltModel {
                    slot: CAB_SLOT,
                    model_id: 2,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
            ],
        );
        assert!(PodTone::parse(duplicate).is_err());
    }

    #[test]
    fn pod_model_enable_patches_only_the_selected_minf_byte_and_noops_same_value() {
        for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
            for initial in [0u8, 1] {
                let original = enable_tone(0xfedc_ba98, PODXT_EQ_MODEL_ID, initial, initial);
                let mut tone = PodTone::parse(original.clone()).unwrap();
                assert_eq!(
                    tone.model_enable_state(control).unwrap(),
                    ModelEnableState::Canonical(initial != 0)
                );
                let model_index = match control {
                    PodModelEnable::Amp => 0,
                    PodModelEnable::Eq => 1,
                };
                let enabled_offset = tone.model_minfo_ranges[model_index].start + 11;
                let desired = initial == 0;
                let mut expected = original.clone();
                expected[enabled_offset] = u8::from(desired);
                tone.set_model_enabled(control, desired).unwrap();
                assert_eq!(tone.as_bytes(), expected);
                assert_eq!(tone.models()[model_index].enabled, Some(desired));
                let after_change = tone.as_bytes().to_vec();
                tone.set_model_enabled(control, desired).unwrap();
                assert_eq!(tone.as_bytes(), after_change, "same value changed bytes");
                let (ordinal, reserved) = match control {
                    PodModelEnable::Amp => (-128, [0xa1, 0xb2]),
                    PodModelEnable::Eq => (-1, [0xc3, 0xd4]),
                };
                assert_eq!(tone.models()[model_index].ordinal, Some(ordinal));
                assert_eq!(tone.models()[model_index].reserved, Some(reserved));
                assert!(
                    tone.models()[0]
                        .parameters
                        .iter()
                        .any(|parameter| matches!(parameter, ModelParameter::Opaque { len: 3 }))
                );
                assert_eq!(
                    PodTone::parse(tone.as_bytes().to_vec()).unwrap().as_bytes(),
                    expected
                );
            }
        }
    }

    #[test]
    fn pod_model_enable_reads_signed_noncanonical_bytes_but_rejects_edits_atomically() {
        for raw in [0x02, 0x7f, 0x80, 0xff] {
            let original = enable_tone(0xdead_beef, PODXT_EQ_MODEL_ID, raw, raw);
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let source_interpretation = (raw as i8) > 0;
            assert_eq!(
                tone.model_enable_state(PodModelEnable::Amp).unwrap(),
                ModelEnableState::NonCanonical {
                    raw,
                    source_interpretation,
                }
            );
            assert_eq!(tone.models()[0].enabled, Some(source_interpretation));
            for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                let before = tone.clone();
                assert!(
                    tone.set_model_enabled(control, source_interpretation)
                        .is_err()
                );
                assert_eq!(tone, before, "rejected edit mutated tone for {raw:02x}");
                assert!(
                    tone.set_model_enabled(control, !source_interpretation)
                        .is_err()
                );
                assert_eq!(tone, before, "rejected edit mutated tone for {raw:02x}");
            }
        }
    }

    #[test]
    fn pod_model_enable_rejects_wrong_target_missing_short_and_duplicate_models() {
        let mut wrong_eq = PodTone::parse(enable_tone(1, 0x020c_0003, 0, 0)).unwrap();
        let before = wrong_eq.clone();
        assert!(
            wrong_eq
                .set_model_enabled(PodModelEnable::Eq, true)
                .is_err()
        );
        assert_eq!(wrong_eq, before);

        let mut no_amp = PodTone::parse(custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 1,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            }],
        ))
        .unwrap();
        assert!(
            no_amp
                .set_model_enabled(PodModelEnable::Amp, false)
                .is_err()
        );

        let mut short = PodTone::parse(custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0x80, 0xa1, 0xb2],
                parameters: &[],
            }],
        ))
        .unwrap();
        let before = short.clone();
        assert!(short.set_model_enabled(PodModelEnable::Amp, false).is_err());
        assert_eq!(short, before);

        let duplicate = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 1,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 2,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
            ],
        );
        assert!(PodTone::parse(duplicate).is_err());

        let mut old_version = PodTone::parse(enable_tone(1, PODXT_EQ_MODEL_ID, 0, 0)).unwrap();
        old_version.device_version = PODXT_LIVE_HOST_MIN_VERSION - 1;
        let before = old_version.clone();
        assert!(
            old_version
                .set_model_enabled(PodModelEnable::Amp, true)
                .is_err()
        );
        assert_eq!(old_version, before);
    }

    #[test]
    fn pod_model_enable_rejects_short_minf_before_get_or_mutation() {
        for minf_len in 8..12 {
            for following_byte in [0, 1] {
                let original = short_enable_tone(minf_len, following_byte);
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let before = tone.clone();
                for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                    assert_eq!(
                        tone.model_enable_state(control).unwrap_err().detail(),
                        format!(
                            "{} enable edit requires an existing MINF body of at least 12 bytes",
                            control.label()
                        )
                    );
                    for requested in [false, true] {
                        assert!(tone.set_model_enabled(control, requested).is_err());
                        assert_eq!(tone, before);
                        assert_eq!(tone.as_bytes(), original);
                    }
                }

                let mut library = Library::default();
                let index = library.import_bytes(original.clone()).unwrap();
                for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                    for requested in [false, true] {
                        assert!(
                            library
                                .set_model_enabled(index, control, requested)
                                .is_err()
                        );
                        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
                        assert!(!library.is_dirty(index));
                    }
                }

                let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
                let before = bundle.as_bytes().to_vec();
                for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                    assert!(
                        bundle
                            .tone(0, 0)
                            .unwrap()
                            .model_enable_state(control)
                            .is_err()
                    );
                    for requested in [false, true] {
                        assert!(bundle.set_model_enabled(0, control, requested).is_err());
                        assert_eq!(bundle.as_bytes(), before);
                        assert!(!bundle.is_dirty());
                    }
                }
            }
        }
    }

    #[test]
    fn pod_model_enable_library_and_bundle_edits_track_dirty_revert_and_save_copy() {
        let original = enable_tone(0xdead_beef, PODXT_EQ_MODEL_ID, 0, 1);
        let wrong_eq = enable_tone(0xdead_beef, 0x020c_0003, 0, 1);
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        let rejected_index = library.import_bytes(wrong_eq.clone()).unwrap();
        assert!(
            library
                .set_model_enabled(rejected_index, PodModelEnable::Eq, true)
                .is_err()
        );
        assert_eq!(library.tone(rejected_index).unwrap().as_bytes(), wrong_eq);
        assert!(!library.is_dirty(rejected_index));
        library
            .set_model_enabled(index, PodModelEnable::Amp, true)
            .unwrap();
        assert!(library.is_dirty(index));
        let edited = library.tone(index).unwrap().as_bytes().to_vec();
        let expected_offset = library.tone(index).unwrap().model_minfo_ranges[0].start + 11;
        assert_eq!(edited[expected_offset], 1);
        library
            .set_model_enabled(index, PodModelEnable::Amp, true)
            .unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), edited);
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        assert!(!library.is_dirty(index));

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let bundle_before = bundle.as_bytes().to_vec();
        let mut rejected_bundle = PodBundleLibrary::parse(bundle_with_tone(&wrong_eq)).unwrap();
        let rejected_bytes = rejected_bundle.as_bytes().to_vec();
        assert!(
            rejected_bundle
                .set_model_enabled(0, PodModelEnable::Eq, true)
                .is_err()
        );
        assert_eq!(rejected_bundle.as_bytes(), rejected_bytes);
        assert!(!rejected_bundle.is_dirty());
        assert!(!bundle.is_dirty());
        bundle
            .set_model_enabled(0, PodModelEnable::Eq, false)
            .unwrap();
        assert!(bundle.is_dirty());
        let edited_bundle = bundle.as_bytes().to_vec();
        bundle
            .set_model_enabled(0, PodModelEnable::Eq, false)
            .unwrap();
        assert_eq!(bundle.as_bytes(), edited_bundle);
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("enable-copy.vwb");
        bundle.save_copy(&copy).unwrap();
        assert!(bundle.save_copy(&copy).is_err());
        assert_eq!(std::fs::read(&copy).unwrap(), edited_bundle);
        let reopened = PodBundleLibrary::parse(std::fs::read(&copy).unwrap()).unwrap();
        assert_eq!(
            reopened
                .tone(0, 0)
                .unwrap()
                .model_enable_state(PodModelEnable::Eq),
            Ok(ModelEnableState::Canonical(false))
        );
        assert!(
            bundle.is_dirty(),
            "saving a copy must not move the baseline"
        );
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), bundle_before);
        assert!(!bundle.is_dirty());
    }

    #[test]
    fn podxt_eq_band_uses_source_f32_graph_conversion_and_patches_only_its_pair() {
        assert_eq!(
            PodxtEqBand::ALL.map(PodxtEqBand::selector_ids),
            [
                (0x0009_0001, 0x0009_0002),
                (0x0009_0003, 0x0009_0004),
                (0x0009_0005, 0x0009_0006),
                (0x0009_0007, 0x0009_0008),
            ]
        );
        assert_eq!(
            PodxtEqBand::ALL.map(PodxtEqBand::parameter_ids),
            [
                (0x0010_0000, 0x0010_0001),
                (0x0010_0002, 0x0010_0003),
                (0x0010_0004, 0x0010_0005),
                (0x0010_0006, 0x0010_0007),
            ]
        );
        let mut duplicated_records: Vec<_> = (0..8)
            .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
            .collect();
        duplicated_records.push(drive_record(0x3f, 0x0010_0000, 1, 0x3e00_0000));
        let duplicated_refs: Vec<_> = duplicated_records
            .iter()
            .map(|record| &record[..])
            .collect();
        let mut duplicated = PodTone::parse(eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &duplicated_refs,
        ))
        .unwrap();
        duplicated
            .set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0)
            .unwrap();
        let eq_model = duplicated
            .models
            .iter()
            .position(|model| model.slot == PODXT_EQ_SLOT)
            .unwrap();
        let matching: Vec<_> = duplicated.models[eq_model]
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| {
                matches!(
                    parameter,
                    ModelParameter::Recorded {
                        id: 0x0010_0000,
                        ..
                    }
                )
                .then_some(index)
            })
            .collect();
        assert_eq!(matching.len(), 2);
        let first_offset = duplicated.parameter_value_offsets[eq_model][matching[0]].unwrap();
        let last_offset = duplicated.parameter_value_offsets[eq_model][matching[1]].unwrap();
        assert_eq!(
            u32::from_be_bytes(
                duplicated.as_bytes()[first_offset..first_offset + 4]
                    .try_into()
                    .unwrap()
            ),
            0x3f00_0000
        );
        assert_eq!(
            u32::from_be_bytes(
                duplicated.as_bytes()[last_offset..last_offset + 4]
                    .try_into()
                    .unwrap()
            ),
            0x3d80_0000
        );

        let records: Vec<_> = (0..8)
            .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
            .collect();
        let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
        let original = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs,
        );
        let interiors = [90.0f32, 90.0, 500.0, 700.0];
        for (index, band) in PodxtEqBand::ALL.into_iter().enumerate() {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_podxt_eq_band(band, interiors[index], 6.3f32)
                .unwrap();
            let (frequency_id, gain_id) = band.parameter_ids();
            let eq_model = tone
                .models
                .iter()
                .position(|model| model.slot == PODXT_EQ_SLOT)
                .unwrap();
            let frequency_index = tone.models[eq_model].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == frequency_id)).unwrap();
            let gain_index = tone.models[eq_model].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == gain_id)).unwrap();
            assert_eq!(
                tone.parameter_value_offsets[eq_model][frequency_index].map(|offset| {
                    u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap())
                }),
                Some(0x3d80_0000)
            );
            assert_eq!(
                tone.parameter_value_offsets[eq_model][gain_index].map(
                    |offset| u32::from_be_bytes(
                        tone.as_bytes()[offset..offset + 4].try_into().unwrap()
                    )
                ),
                Some(0x3f40_8102)
            );
            let mut expected = original.clone();
            let frequency_offset = tone.parameter_value_offsets[eq_model][frequency_index].unwrap();
            let gain_offset = tone.parameter_value_offsets[eq_model][gain_index].unwrap();
            expected[frequency_offset..frequency_offset + 4]
                .copy_from_slice(&0x3d80_0000u32.to_be_bytes());
            expected[gain_offset..gain_offset + 4].copy_from_slice(&0x3f40_8102u32.to_be_bytes());
            assert_eq!(tone.as_bytes(), expected);
        }

        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_podxt_eq_band(PodxtEqBand::One, 209.999f32, -12.8f32)
            .unwrap();
        let (frequency_id, gain_id) = PodxtEqBand::One.parameter_ids();
        let model_index = tone
            .models
            .iter()
            .position(|model| model.slot == PODXT_EQ_SLOT)
            .unwrap();
        for (id, expected) in [(frequency_id, 0x3e7f_ff96), (gain_id, 0x0000_0000)] {
            let parameter = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)).unwrap();
            let offset = tone.parameter_value_offsets[model_index][parameter].unwrap();
            assert_eq!(
                u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
                expected
            );
        }

        let preceding = [
            [0x3dff_fffd, 0x3eff_fffe, 0x3f7f_fffe],
            [0x3dff_fffd, 0x3eff_ffff, 0x3f7f_ffff],
            [0x3dff_ffff, 0x3eff_ffff, 0x3f7f_ffff],
            [0x3dff_fffd, 0x3eff_ffff, 0x3f7f_ffff],
        ];
        for (band_index, band) in PodxtEqBand::ALL.into_iter().enumerate() {
            for (boundary_index, point) in [1usize, 4, 8].into_iter().enumerate() {
                let boundary = PODXT_EQ_FREQUENCY_TABLES[band_index][point] as f32;
                let adjacent = f32::from_bits(boundary.to_bits() - 1);
                let mut tone = PodTone::parse(original.clone()).unwrap();
                tone.set_podxt_eq_band(band, adjacent, 0.0).unwrap();
                let (frequency_id, _) = band.parameter_ids();
                let model_index = tone
                    .models
                    .iter()
                    .position(|model| model.slot == PODXT_EQ_SLOT)
                    .unwrap();
                let parameter_index = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == frequency_id)).unwrap();
                let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
                assert_eq!(
                    u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
                    preceding[band_index][boundary_index]
                );
                tone.set_podxt_eq_band(band, boundary, 12.6).unwrap();
                let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
                assert_eq!(
                    u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
                    [0x3e00_0000, 0x3f00_0000, 0x3f80_0000][boundary_index]
                );
                let (_, gain_id) = band.parameter_ids();
                let gain_index = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == gain_id)).unwrap();
                let gain_offset = tone.parameter_value_offsets[model_index][gain_index].unwrap();
                assert_eq!(
                    u32::from_be_bytes(
                        tone.as_bytes()[gain_offset..gain_offset + 4]
                            .try_into()
                            .unwrap()
                    ),
                    0x3f80_0000
                );
            }
        }
    }

    #[test]
    fn podxt_eq_display_matches_source_endpoints_and_double_key_boundaries() {
        let bands = PodxtEqBand::ALL;
        let first = [0x4248_0000, 0x4248_0000, 0x42c8_0000, 0x43fa_0000];
        let last = [0x442c_8000, 0x45bd_1000, 0x4630_9000, 0x4611_5000];
        let boundary_cases = [
            (
                [0x3b81_0203, 0x3b81_0204, 0x3b81_0205],
                [
                    [0x4248_0000, 0x4248_0000, 0x425c_0000],
                    [0x4248_0000, 0x4248_0000, 0x425c_0000],
                    [0x42c8_0000, 0x42c8_0000, 0x4316_0000],
                    [0x43fa_0000, 0x43fa_0000, 0x4403_4000],
                ],
            ),
            (
                [0x3df9_f3e7, 0x3df9_f3e8, 0x3df9_f3e9],
                [
                    [0x42fa_0000, 0x4302_0000, 0x4302_0000],
                    [0x42fa_0000, 0x4302_0000, 0x4302_0000],
                    [0x4454_8000, 0x4461_0000, 0x4461_0000],
                    [0x445a_c000, 0x4461_0000, 0x4461_0000],
                ],
            ),
            (
                [0x3f7e_fdfb, 0x3f7e_fdfc, 0x3f7e_fdfd],
                [
                    [0x442a_0000, 0x442c_8000, 0x442c_8000],
                    [0x45b6_d000, 0x45bd_1000, 0x45bd_1000],
                    [0x462d_7000, 0x4630_9000, 0x4630_9000],
                    [0x460e_3000, 0x4611_5000, 0x4611_5000],
                ],
            ),
        ];
        for (band_index, band) in bands.into_iter().enumerate() {
            for (raw, expected) in [
                (0u32, first[band_index]),
                (0x3f80_0000, last[band_index]),
                (0x3b81_0204, first[band_index]),
            ] {
                let refs: Vec<_> = (0..8)
                    .map(|i| {
                        drive_record(0x3f, 0x0010_0000 + i, 1, if i % 2 == 0 { raw } else { 0 })
                    })
                    .collect();
                let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
                let tone = PodTone::parse(eq_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    PODXT_EQ_MODEL_ID,
                    PODXT_EQ_SLOT,
                    &refs,
                ))
                .unwrap();
                let (frequency, _) = tone.podxt_eq_band_display_values(band).unwrap();
                assert_eq!(
                    frequency.to_bits(),
                    expected,
                    "band {band_index}, raw {raw:#010x}"
                );
            }
            for (raws, by_band) in boundary_cases {
                for (raw, expected) in raws.into_iter().zip(by_band[band_index]) {
                    let refs: Vec<_> = (0..8)
                        .map(|i| {
                            drive_record(0x3f, 0x0010_0000 + i, 1, if i % 2 == 0 { raw } else { 0 })
                        })
                        .collect();
                    let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
                    let tone = PodTone::parse(eq_tone(
                        PODXT_LIVE_HOST_MIN_VERSION,
                        PODXT_EQ_MODEL_ID,
                        PODXT_EQ_SLOT,
                        &refs,
                    ))
                    .unwrap();
                    let (frequency, _) = tone.podxt_eq_band_display_values(band).unwrap();
                    assert_eq!(
                        frequency.to_bits(),
                        expected,
                        "band {band_index}, boundary raw {raw:#010x}"
                    );
                }
            }
            let refs: Vec<_> = (0..8)
                .map(|i| drive_record(0x3f, 0x0010_0000 + i, 1, 0x3f80_0000))
                .collect();
            let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
            let mut tone = PodTone::parse(eq_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &refs,
            ))
            .unwrap();
            let (_, gain) = tone.podxt_eq_band_display_values(band).unwrap();
            assert_eq!(gain.to_bits(), 0x4149_999a);
            tone.set_podxt_eq_band(band, f32::from_bits(last[band_index]), gain)
                .unwrap();
            let (frequency_word, gain_word) = tone.podxt_eq_band_raw_values(band).unwrap();
            assert_eq!(frequency_word.to_bits(), 0x3f80_0000);
            assert_eq!(gain_word.to_bits(), 0x3f80_0000);

            for (raw_gain, expected) in [
                (0u32, 0xc14c_cccd),
                (1, 0xc14c_cccd),
                (0x3f7f_fffe, 0x4149_9998),
                (0x3f7f_ffff, 0x4149_9999),
                (0x3f80_0000, 0x4149_999a),
            ] {
                let refs: Vec<_> = (0..8)
                    .map(|i| {
                        drive_record(
                            0x3f,
                            0x0010_0000 + i,
                            1,
                            if i % 2 == 0 { 0x3f80_0000 } else { raw_gain },
                        )
                    })
                    .collect();
                let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
                let tone = PodTone::parse(eq_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    PODXT_EQ_MODEL_ID,
                    PODXT_EQ_SLOT,
                    &refs,
                ))
                .unwrap();
                let (_, displayed) = tone.podxt_eq_band_display_values(band).unwrap();
                assert_eq!(
                    displayed.to_bits(),
                    expected,
                    "band {band_index}, gain raw {raw_gain:#010x}"
                );
            }
        }
    }

    #[test]
    fn podxt_eq_pair_rejects_invalid_inputs_and_targets_without_partial_mutation() {
        let good = [
            drive_record(0x3f, 0x0010_0000, 1, 0x3f00_0000),
            drive_record(0x3f, 0x0010_0001, 1, 0x3f00_0000),
        ];
        let good_refs: Vec<_> = good.iter().map(|record| &record[..]).collect();
        let original = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &good_refs,
        );
        for (frequency, gain) in [
            (f32::NAN, 0.0),
            (f32::INFINITY, 0.0),
            (49.0, 0.0),
            (691.0, 0.0),
            (50.0, f32::NAN),
            (50.0, f32::NEG_INFINITY),
            (50.0, -12.81),
            (50.0, 12.61),
        ] {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert!(
                tone.set_podxt_eq_band(PodxtEqBand::One, frequency, gain)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), original);
        }
        let invalid_current = drive_record(0x3f, 0x0010_0001, 1, f32::NAN.to_bits());
        let invalid_refs: Vec<_> = [&good[0][..], &invalid_current[..]].into_iter().collect();
        let invalid = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &invalid_refs,
        );
        let mut tone = PodTone::parse(invalid.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), invalid);
        for invalid_bits in [
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
            0x3f80_0001,
        ] {
            let bad = drive_record(0x3f, 0x0010_0001, 1, invalid_bits);
            let refs: Vec<_> = [&good[0][..], &bad[..]].into_iter().collect();
            let bytes = eq_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &refs,
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
            assert_eq!(tone.as_bytes(), bytes);
        }

        for (version, model, slot, params) in [
            (
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &good_refs[..],
            ),
            (
                PODXT_LIVE_HOST_MIN_VERSION,
                0x020c_0003,
                PODXT_EQ_SLOT,
                &good_refs[..],
            ),
            (
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT + 1,
                &good_refs[..],
            ),
            (
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &good_refs[..1],
            ),
        ] {
            let bytes = eq_tone(version, model, slot, params);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
            assert_eq!(tone.as_bytes(), bytes);
        }
        let bad_gain = drive_record(0x3e, 0x0010_0001, 1, 0x3f00_0000);
        let bad_refs: Vec<_> = [&good[0][..], &bad_gain[..]].into_iter().collect();
        let bytes = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &bad_refs,
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), bytes);

        let wrong_type = drive_record(0x3f, 0x0010_0001, 0, 0x3f00_0000);
        let wrong_type_refs: Vec<_> = [&good[0][..], &wrong_type[..]].into_iter().collect();
        let bytes = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &wrong_type_refs,
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), bytes);

        let opaque_refs: Vec<_> = [&good[0][..], &good[1][..], &[0x12, 0x34, 0x56][..]]
            .into_iter()
            .collect();
        let bytes = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &opaque_refs,
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), bytes);

        let unknown = drive_record(0x3f, 0x0010_0001, 1, 0x3f80_0001);
        let unknown_refs: Vec<_> = [&good[0][..], &unknown[..]].into_iter().collect();
        let bytes = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &unknown_refs,
        );
        let tone = PodTone::parse(bytes.clone()).unwrap();
        assert_eq!(
            tone.podxt_eq_band_raw_values(PodxtEqBand::One)
                .unwrap()
                .1
                .to_bits(),
            0x3f80_0001
        );
        assert!(tone.podxt_eq_band_display_values(PodxtEqBand::One).is_err());
        assert_eq!(tone.as_bytes(), bytes);
    }

    #[test]
    fn podxt_eq_library_and_bundle_edits_revert_save_and_reopen_offline() {
        let records: Vec<_> = (0..8)
            .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
            .collect();
        let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
        let original = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs,
        );

        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        assert!(!library.is_dirty(index));
        library
            .set_podxt_eq_band(index, PodxtEqBand::Two, 90.0, 0.0)
            .unwrap();
        assert!(library.is_dirty(index));
        assert_eq!(
            library
                .tone(index)
                .unwrap()
                .podxt_eq_band_raw_values(PodxtEqBand::Two)
                .unwrap(),
            (0.0625, 0.503_937)
        );
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        assert!(!library.is_dirty(index));

        let bundle_bytes = bundle_with_tone(&original);
        let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
        bundle
            .set_podxt_eq_band(0, PodxtEqBand::Four, 700.0, 6.3)
            .unwrap();
        assert!(bundle.is_dirty());
        let location = bundle.entry_location(0).unwrap();
        assert_eq!(
            bundle
                .tone(location.0, location.1)
                .unwrap()
                .podxt_eq_band_raw_values(PodxtEqBand::Four)
                .unwrap(),
            (0.0625, f32::from_bits(0x3f40_8102))
        );
        bundle.revert().unwrap();
        assert!(!bundle.is_dirty());
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);

        bundle
            .set_podxt_eq_band(0, PodxtEqBand::One, 90.0, 0.0)
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("eq-copy.l6b");
        bundle.save_copy(&copy).unwrap();
        let reopened = PodBundleLibrary::parse(std::fs::read(copy).unwrap()).unwrap();
        assert_eq!(
            reopened
                .tone(0, 0)
                .unwrap()
                .podxt_eq_band_raw_values(PodxtEqBand::One)
                .unwrap(),
            (0.0625, 0.503_937)
        );
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
    fn stomp_harmony_choices_match_source_bits_and_patch_only_selected_existing_word() {
        let expected_bits = [
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
        let expected_keys = [0, 16, 32, 48, 64, 80, 96, 112, 127];
        assert_eq!(StompHarmonyControl::Harmony1M335.selector_id(), 0x0003_0001);
        assert_eq!(
            StompHarmonyControl::Harmony1M335.parameter_id(),
            0x0010_0001
        );
        assert_eq!(StompHarmonyControl::Harmony1457.selector_id(), 0x0003_0002);
        assert_eq!(StompHarmonyControl::Harmony1457.parameter_id(), 0x0010_0002);
        assert_eq!(StompHarmony1M335Selection::ALL.len(), 9);
        assert_eq!(StompHarmony1457Selection::ALL.len(), 9);
        let expected_labels1 = [
            "-1 OCT", "-MAJ6TH", "-MIN6TH", "-4TH", "UNISON", "MIN3RD", "MAJ3RD", "5TH", "1 OCT",
        ];
        let expected_labels2 = [
            "-1 OCT", "-5TH", "-4TH", "-2ND", "UNISON", "4TH", "5TH", "7TH", "1 OCT",
        ];
        for (((choice, bits), key), label) in StompHarmony1M335Selection::ALL
            .into_iter()
            .zip(expected_bits)
            .zip(expected_keys)
            .zip(expected_labels1)
        {
            assert_eq!(choice.float_bits(), bits);
            assert_eq!(choice.controller_key(), key);
            assert_eq!(choice.label(), label);
        }
        for (((choice, bits), key), label) in StompHarmony1457Selection::ALL
            .into_iter()
            .zip(expected_bits)
            .zip(expected_keys)
            .zip(expected_labels2)
        {
            assert_eq!(choice.float_bits(), bits);
            assert_eq!(choice.controller_key(), key);
            assert_eq!(choice.label(), label);
        }
        let original = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
        let value_start1 = original
            .windows(4)
            .position(|w| w == ((0x3f10_0001u32).to_be_bytes()))
            .unwrap()
            + 8;
        let value_start2 = original
            .windows(4)
            .position(|w| w == ((0x3f10_0002u32).to_be_bytes()))
            .unwrap()
            + 8;
        for (choice, bits) in StompHarmony1M335Selection::ALL
            .into_iter()
            .zip(expected_bits)
        {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_stomp_harmony_1m335_selection(choice).unwrap();
            let mut expected_bytes = original.clone();
            expected_bytes[value_start1..value_start1 + 4].copy_from_slice(&bits.to_be_bytes());
            assert_eq!(tone.as_bytes(), expected_bytes);
            assert_eq!(
                tone.stomp_harmony_1m335_selection().unwrap(),
                StompHarmonyValue::Known(choice)
            );
            assert_eq!(
                tone.stomp_harmony_1457_selection().unwrap(),
                StompHarmonyValue::Known(StompHarmony1457Selection::Fifth)
            );
        }
        for (choice, bits) in StompHarmony1457Selection::ALL
            .into_iter()
            .zip(expected_bits)
        {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_stomp_harmony_1457_selection(choice).unwrap();
            let mut expected_bytes = original.clone();
            expected_bytes[value_start2..value_start2 + 4].copy_from_slice(&bits.to_be_bytes());
            assert_eq!(tone.as_bytes(), expected_bytes);
            assert_eq!(
                tone.stomp_harmony_1457_selection().unwrap(),
                StompHarmonyValue::Known(choice)
            );
            assert_eq!(
                tone.stomp_harmony_1m335_selection().unwrap(),
                StompHarmonyValue::Known(StompHarmony1M335Selection::Unison)
            );
        }
    }

    #[test]
    fn stomp_harmony_unknown_is_preserved_and_invalid_current_values_reject_atomically() {
        let mut tone = PodTone::parse(stomp_harmony_tone(0x3f00_0001, 0x7fc0_1234)).unwrap();
        let before = tone.as_bytes().to_vec();
        assert_eq!(
            tone.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Unknown {
                raw_bits: 0x3f00_0001
            }
        );
        assert!(
            tone.set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), before);
        let mut unknown = PodTone::parse(stomp_harmony_tone(0x3f00_0001, 0x3f00_0000)).unwrap();
        unknown
            .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::MinusOctave)
            .unwrap();
        assert_eq!(
            unknown.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1M335Selection::MinusOctave)
        );
        let mut below_profile = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
        let pinf = find_chunk_data(&below_profile, b"PINF");
        below_profile[pinf + 72..pinf + 76]
            .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        let mut below_profile = PodTone::parse(below_profile).unwrap();
        let before = below_profile.as_bytes().to_vec();
        assert!(
            below_profile
                .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                .is_err(),
            "same-choice requests must still validate the host profile"
        );
        assert_eq!(below_profile.as_bytes(), before);
    }

    #[test]
    fn stomp_harmony_rejects_ambiguous_or_malformed_targets_without_mutation() {
        let controls = [
            StompHarmonyControl::Harmony1M335,
            StompHarmonyControl::Harmony1457,
        ];
        for control in controls {
            let target = control.parameter_id();
            let sibling = match control {
                StompHarmonyControl::Harmony1M335 => 0x0010_0002,
                StompHarmonyControl::Harmony1457 => 0x0010_0001,
            };
            let canonical = drive_record(0x3f, target, 1, 0x3f00_0000);
            let sibling_record = drive_record(0x3f, sibling, 1, 0x3f40_8102);
            let equal_duplicate = canonical;
            let different_duplicate = drive_record(0x3f, target, 1, 0x3f20_4081);
            let bad_prefix_duplicate = drive_record(0x3e, target, 1, 0x3f20_4081);
            let wrong_type = drive_record(0x3f, target, 0, 0x3f00_0000);
            let opaque_target = ((0x3fu32 << 24) | target).to_be_bytes();
            let mut invalid = Vec::new();
            for (first, second) in [
                (canonical.as_slice(), equal_duplicate.as_slice()),
                (canonical.as_slice(), different_duplicate.as_slice()),
                (bad_prefix_duplicate.as_slice(), canonical.as_slice()),
            ] {
                invalid.push(stomp_harmony_tone_with_models(
                    &[BuiltModel {
                        slot: STOMP_SLOT,
                        model_id: 0x020a_0009,
                        tail: &[0x91, 0x82, 0x73, 0x64],
                        parameters: &[first, second, sibling_record.as_slice()],
                    }],
                    PODXT_LIVE_TONE_ID,
                    PODXT_LIVE_HOST_MIN_VERSION,
                ));
            }
            let duplicate_slots = stomp_harmony_tone_with_models(
                &[
                    BuiltModel {
                        slot: STOMP_SLOT,
                        model_id: 0x020a_0009,
                        tail: &[0x91, 0x82, 0x73, 0x64],
                        parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                    },
                    BuiltModel {
                        slot: STOMP_SLOT,
                        model_id: 0x020a_0009,
                        tail: &[0x91, 0x82, 0x73, 0x64],
                        parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                    },
                ],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            );
            assert!(
                PodTone::parse(duplicate_slots.clone()).is_err(),
                "duplicate Stomp slots must be rejected while original bytes stay available"
            );
            assert!(!duplicate_slots.is_empty());
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0008,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT + 1,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[wrong_type.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[bad_prefix_duplicate.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[
                        opaque_target.as_slice(),
                        canonical.as_slice(),
                        sibling_record.as_slice(),
                    ],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
            let wrong_target = stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                }],
                0x1234_5678,
                PODXT_LIVE_HOST_MIN_VERSION,
            );
            assert!(PodTone::parse(wrong_target.clone()).is_err());
            assert!(!wrong_target.is_empty());
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION - 1,
            ));

            for (case, bytes) in invalid.into_iter().enumerate() {
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                let before = tone.as_bytes().to_vec();
                let getter_rejected = match control {
                    StompHarmonyControl::Harmony1M335 => {
                        tone.stomp_harmony_1m335_selection().is_err()
                    }
                    StompHarmonyControl::Harmony1457 => {
                        tone.stomp_harmony_1457_selection().is_err()
                    }
                };
                assert!(
                    getter_rejected,
                    "getter admitted invalid case {case} for {control:?}"
                );
                let setter_rejected = match control {
                    StompHarmonyControl::Harmony1M335 => tone
                        .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                        .is_err(),
                    StompHarmonyControl::Harmony1457 => tone
                        .set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                        .is_err(),
                };
                assert!(
                    setter_rejected,
                    "setter admitted invalid case {case} for {control:?}"
                );
                assert_eq!(tone.as_bytes(), before, "case {case} mutated tone");
            }

            let nonfinite_target = drive_record(0x3f, target, 1, 0x7fc0_1234);
            let nonfinite = stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[nonfinite_target.as_slice(), sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            );
            let mut tone = PodTone::parse(nonfinite).unwrap();
            let before = tone.as_bytes().to_vec();
            let setter_rejected = match control {
                StompHarmonyControl::Harmony1M335 => {
                    assert_eq!(
                        tone.stomp_harmony_1m335_selection().unwrap(),
                        StompHarmonyValue::Unknown {
                            raw_bits: 0x7fc0_1234
                        }
                    );
                    tone.set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                        .is_err()
                }
                StompHarmonyControl::Harmony1457 => {
                    assert_eq!(
                        tone.stomp_harmony_1457_selection().unwrap(),
                        StompHarmonyValue::Unknown {
                            raw_bits: 0x7fc0_1234
                        }
                    );
                    tone.set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                        .is_err()
                }
            };
            assert!(setter_rejected);
            assert_eq!(tone.as_bytes(), before);

            // A duplicate that happens to contain the requested choice still fails admission.
            let same_choice = stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[
                        canonical.as_slice(),
                        canonical.as_slice(),
                        sibling_record.as_slice(),
                    ],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            );
            let mut tone = PodTone::parse(same_choice).unwrap();
            let before = tone.as_bytes().to_vec();
            let rejected = match control {
                StompHarmonyControl::Harmony1M335 => tone
                    .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                    .is_err(),
                StompHarmonyControl::Harmony1457 => tone
                    .set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                    .is_err(),
            };
            assert!(rejected, "same-choice duplicate admitted for {control:?}");
            assert_eq!(tone.as_bytes(), before);
        }
    }

    #[test]
    fn stomp_harmony_library_and_bundle_paths_round_trip_offline_edits() {
        let original = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        library
            .set_stomp_harmony_1m335_selection(index, StompHarmony1M335Selection::MinusOctave)
            .unwrap();
        library
            .set_stomp_harmony_1457_selection(index, StompHarmony1457Selection::Fourth)
            .unwrap();
        assert!(library.is_dirty(index));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("harmony-copy.l6t");
        library.tone(index).unwrap().save_copy(&path).unwrap();
        let reopened = PodTone::parse(std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            reopened.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1M335Selection::MinusOctave)
        );
        assert_eq!(
            reopened.stomp_harmony_1457_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1457Selection::Fourth)
        );

        let bundle_bytes = bundle_with_tone(&original);
        let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
        bundle
            .set_stomp_harmony_1m335_selection(0, StompHarmony1M335Selection::Fifth)
            .unwrap();
        bundle
            .set_stomp_harmony_1457_selection(0, StompHarmony1457Selection::Seventh)
            .unwrap();
        assert!(bundle.is_dirty());
        let reopened = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
        let tone = reopened.tone(0, 0).unwrap();
        assert_eq!(
            tone.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1M335Selection::Fifth)
        );
        assert_eq!(
            tone.stomp_harmony_1457_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1457Selection::Seventh)
        );

        for (target, selector1) in [(0x0010_0001u32, true), (0x0010_0002, false)] {
            let duplicate = drive_record(0x3f, target, 1, 0x3f00_0000);
            let first = drive_record(0x3f, 0x0010_0001, 1, 0x3f00_0000);
            let second = drive_record(0x3f, 0x0010_0002, 1, 0x3f40_8102);
            let parameters = if selector1 {
                [first.as_slice(), duplicate.as_slice(), second.as_slice()]
            } else {
                [first.as_slice(), second.as_slice(), duplicate.as_slice()]
            };
            let duplicate_bytes = stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &parameters,
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            );
            let mut library = Library::default();
            let index = library.import_bytes(duplicate_bytes.clone()).unwrap();
            let setter_rejected = if selector1 {
                library
                    .set_stomp_harmony_1m335_selection(index, StompHarmony1M335Selection::Unison)
                    .is_err()
            } else {
                library
                    .set_stomp_harmony_1457_selection(index, StompHarmony1457Selection::Unison)
                    .is_err()
            };
            assert!(setter_rejected);
            assert!(!library.is_dirty(index));
            assert_eq!(library.tone(index).unwrap().as_bytes(), duplicate_bytes);

            let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&duplicate_bytes)).unwrap();
            let setter_rejected = if selector1 {
                bundle
                    .set_stomp_harmony_1m335_selection(0, StompHarmony1M335Selection::Unison)
                    .is_err()
            } else {
                bundle
                    .set_stomp_harmony_1457_selection(0, StompHarmony1457Selection::Unison)
                    .is_err()
            };
            assert!(setter_rejected);
            assert!(!bundle.is_dirty());
            assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), duplicate_bytes);
        }
    }

    #[test]
    fn stomp_wave_choices_match_exact_source_bits_and_all_eight_layouts() {
        let expected = [
            0xbb81_0204,
            0x3df9_f3e8,
            0x3e7d_fbf8,
            0x3ebf_7efe,
            0x3f00_0000,
            0x3f20_4081,
            0x3f40_8102,
            0x3f60_c183,
        ];
        assert_eq!(STOMP_SYNTH_WAVE_MODEL_IDS.len(), 8);
        assert_eq!(STOMP_SYNTH_WAVE_SELECTOR, 0x0003_0001);
        assert_eq!(STOMP_SYNTH_WAVE_PARAM, 0x0010_0001);
        assert_eq!(StompWaveSelection::ALL.len(), expected.len());
        for (choice, expected_bits) in StompWaveSelection::ALL.into_iter().zip(expected) {
            assert_eq!(choice.float_bits(), expected_bits);
            assert_eq!(
                StompWaveSelection::from_float_bits(expected_bits),
                Some(choice)
            );
        }
        assert_eq!(f32::from_bits(expected[0]), -0.003937008);
        assert!(!f32::from_bits(expected[0]).is_sign_positive());

        for model_id in STOMP_SYNTH_WAVE_MODEL_IDS {
            let earlier = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3e80_0000);
            let unrelated = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
            let selected = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
            let records: [&[u8]; 3] = [&earlier, &unrelated, &selected];
            let original = stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &records);
            let selected_offset = original
                .windows(selected.len())
                .rposition(|window| window == selected)
                .unwrap()
                + 8;
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.stomp_wave_selection().unwrap(),
                StompWaveValue::Known(StompWaveSelection::Wave5)
            );
            for (choice, expected_bits) in StompWaveSelection::ALL.into_iter().zip(expected) {
                let mut expected_bytes = original.clone();
                expected_bytes[selected_offset..selected_offset + 4]
                    .copy_from_slice(&expected_bits.to_be_bytes());
                tone.set_stomp_wave_selection(choice).unwrap();
                assert_eq!(
                    tone.as_bytes(),
                    expected_bytes,
                    "model 0x{model_id:08x} {choice:?}"
                );
                assert_eq!(
                    tone.stomp_wave_selection().unwrap(),
                    StompWaveValue::Known(choice)
                );
                assert!(tone.as_bytes().windows(earlier.len()).any(|w| w == earlier));
                assert!(
                    tone.as_bytes()
                        .windows(unrelated.len())
                        .any(|w| w == unrelated)
                );
            }
        }

        let ninth_candidate = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
        let nine_tone = stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x020a_0009,
            &[&ninth_candidate],
        );
        let mut nine_tone = PodTone::parse(nine_tone).unwrap();
        let before = nine_tone.as_bytes().to_vec();
        assert!(nine_tone.stomp_wave_selection().is_err());
        assert!(
            nine_tone
                .set_stomp_wave_selection(StompWaveSelection::Wave2)
                .is_err()
        );
        assert_eq!(nine_tone.as_bytes(), before);
    }

    #[test]
    fn stomp_wave_unknown_words_are_lossless_and_nonfinite_edits_are_atomic() {
        for bits in [
            0x3eaa_aaab,
            0x7fc0_1234,
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
        ] {
            let record = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, bits);
            let original = stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[&record],
            );
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.stomp_wave_selection().unwrap(),
                StompWaveValue::Unknown { raw_bits: bits }
            );
            assert_eq!(tone.as_bytes(), original);
            if f32::from_bits(bits).is_finite() {
                tone.set_stomp_wave_selection(StompWaveSelection::Wave8)
                    .unwrap();
                assert_eq!(
                    tone.stomp_wave_selection().unwrap(),
                    StompWaveValue::Known(StompWaveSelection::Wave8)
                );
            } else {
                assert!(
                    tone.set_stomp_wave_selection(StompWaveSelection::Wave8)
                        .is_err()
                );
                assert_eq!(tone.as_bytes(), original);
                assert_eq!(
                    tone.stomp_wave_selection().unwrap(),
                    StompWaveValue::Unknown { raw_bits: bits }
                );

                let mut library = Library::default();
                let index = library.import_bytes(original.clone()).unwrap();
                assert!(
                    library
                        .set_stomp_wave_selection(index, StompWaveSelection::Wave8)
                        .is_err()
                );
                assert!(!library.is_dirty(index));
                assert_eq!(library.tone(index).unwrap().as_bytes(), original);

                let bundle_bytes = bundle_with_tone(&original);
                let mut bundle = PodBundleLibrary::parse(bundle_bytes.clone()).unwrap();
                assert!(
                    bundle
                        .set_stomp_wave_selection(0, StompWaveSelection::Wave8)
                        .is_err()
                );
                assert!(!bundle.is_dirty());
                assert_eq!(bundle.as_bytes(), bundle_bytes);
            }
        }
    }

    #[test]
    fn stomp_wave_admission_rejects_wrong_target_model_layout_and_records() {
        let valid = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
        let bad_prefix = drive_record(0x7e, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
        let bad_type = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 0, 0x3f00_0000);
        let opaque = [1, 2, 3, 4];
        let cases = [
            stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[&valid],
            ),
            stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_0009, &[&valid]),
            stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_0000, &[&valid]),
            stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[],
            ),
            stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[&opaque],
            ),
            stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[&bad_prefix],
            ),
            stomp_wave_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_SYNTH_WAVE_MODEL_IDS[0],
                &[&bad_type],
            ),
            custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: AMP_SLOT,
                    model_id: PLEXI_LEAD_100_MODEL,
                    tail: &[],
                    parameters: &[&valid],
                }],
            ),
            custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: AMP_SLOT,
                    model_id: STOMP_SYNTH_WAVE_MODEL_IDS[0],
                    tail: &[],
                    parameters: &[&valid],
                }],
            ),
        ];
        for bytes in cases {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.stomp_wave_selection().is_err());
            assert!(
                tone.set_stomp_wave_selection(StompWaveSelection::Wave4)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), bytes);
        }

        let wrong_target = fixture(
            ContainerKind::L6pa,
            0x0003_000b,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[(STOMP_SLOT, STOMP_SYNTH_WAVE_MODEL_IDS[0])],
        );
        assert!(matches!(
            PodTone::parse(wrong_target),
            Err(ToneError::WrongTarget { .. })
        ));

        let duplicate_slot = custom_tone(
            ContainerKind::L6pa,
            &[
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: STOMP_SYNTH_WAVE_MODEL_IDS[0],
                    tail: &[],
                    parameters: &[&valid],
                },
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: STOMP_SYNTH_WAVE_MODEL_IDS[1],
                    tail: &[],
                    parameters: &[&valid],
                },
            ],
        );
        assert!(PodTone::parse(duplicate_slot).is_err());
    }

    #[test]
    fn stomp_wave_library_bundle_save_and_reopen_preserve_unrelated_bytes() {
        let record = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
        let original = stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[3],
            &[&record],
        );
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        assert_eq!(
            library.stomp_wave_selection(index).unwrap(),
            StompWaveValue::Known(StompWaveSelection::Wave5)
        );
        library
            .set_stomp_wave_selection(index, StompWaveSelection::Wave2)
            .unwrap();
        assert!(library.is_dirty(index));
        let edited = library.tone(index).unwrap().as_bytes().to_vec();
        let changed = edited.iter().zip(&original).filter(|(a, b)| a != b).count();
        assert_eq!(changed, 4);
        library.revert_unsaved(index).unwrap();
        assert!(!library.is_dirty(index));
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        library
            .set_stomp_wave_selection(index, StompWaveSelection::Wave2)
            .unwrap();
        assert!(library.is_dirty(index));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("stomp-wave.l6t");
        library.tone(index).unwrap().save_copy(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        assert!(library.tone(index).unwrap().save_copy(&path).is_err());
        let reopened = PodTone::parse(std::fs::read(path).unwrap()).unwrap();
        assert_eq!(reopened.as_bytes(), edited);
        assert_eq!(
            reopened.stomp_wave_selection().unwrap(),
            StompWaveValue::Known(StompWaveSelection::Wave2)
        );

        let bundle_bytes = bundle_with_tone(&original);
        let mut bundle = PodBundleLibrary::parse(bundle_bytes.clone()).unwrap();
        assert_eq!(
            bundle.stomp_wave_selection(0).unwrap(),
            StompWaveValue::Known(StompWaveSelection::Wave5)
        );
        bundle
            .set_stomp_wave_selection(0, StompWaveSelection::Wave3)
            .unwrap();
        assert!(bundle.is_dirty());
        let bundled_tone = bundle.tone(0, 0).unwrap();
        assert_eq!(
            bundled_tone.stomp_wave_selection().unwrap(),
            StompWaveValue::Known(StompWaveSelection::Wave3)
        );
        assert!(
            bundled_tone
                .as_bytes()
                .iter()
                .zip(&original)
                .filter(|(a, b)| a != b)
                .count()
                == 4
        );
        let reopened_bundle = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
        assert_eq!(
            reopened_bundle.tone(0, 0).unwrap().as_bytes(),
            bundled_tone.as_bytes()
        );
    }

    #[test]
    fn whammy_knob_position_uses_source_float_quantizer_and_patches_only_selected_word() {
        let heel = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
        let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0x3f00_0000);
        let unrelated = drive_record(0x3f, 0x0010_00aa, 1, 0x3e00_0000);
        let original = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[&heel, &toe, &unrelated],
                },
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: PLEXI_45_MODEL,
                    tail: &[0xde, 0xad, 0xbe, 0xef],
                    parameters: &[&unrelated],
                },
            ],
        );
        let cases: [(f32, u32); 6] = [
            (0.0f32, 0x0000_0000),
            (0.25, 0x3e70_f0f2),
            (0.5, 0x3ef8_f8fa),
            (0.75, 0x3f3c_bcbd),
            (1.0, 0x3f7c_fcfe),
            (0.1234567, 0x3de0_e0e2),
        ];
        for control in StompWhammyKnob::ALL {
            for (position, expected_bits) in cases {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                assert_eq!(
                    tone.stomp_whammy_knob_position(control).unwrap().to_bits(),
                    if control == StompWhammyKnob::Heel {
                        0x3e80_0000
                    } else {
                        0x3f00_0000
                    }
                );
                tone.set_stomp_whammy_knob_position(control, position)
                    .unwrap();
                let mut expected = original.clone();
                let (_, parameter_index) = tone.stomp_whammy_knob_location(control).unwrap();
                let model_index = tone
                    .models
                    .iter()
                    .position(|model| model.slot == STOMP_SLOT)
                    .unwrap();
                let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
                expected[offset..offset + 4].copy_from_slice(&expected_bits.to_be_bytes());
                assert_eq!(tone.as_bytes(), expected, "{control:?} at {position}");
            }
        }
    }

    #[test]
    fn whammy_quantizer_transition_neighbors_and_fixed_points_match_source_float32_steps() {
        let heel = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0);
        let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0);
        let original = whammy_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&heel, &toe],
        );
        let boundaries = [
            (0x3c80_8080, 0x0000_0000),
            (0x3c80_8081, 0x3c80_8081),
            (0x3c80_8082, 0x3c80_8081),
            (0x3f00_8080, 0x3ef8_f8fa),
            (0x3f00_8081, 0x3f00_8081),
            (0x3f00_8082, 0x3f00_8081),
            (0x3f7c_fcfc, 0x3f78_f8fa),
            (0x3f7c_fcfd, 0x3f7c_fcfe),
            (0x3f7c_fcfe, 0x3f7c_fcfe),
        ];
        for (input, expected) in boundaries {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, f32::from_bits(input))
                .unwrap();
            assert_eq!(
                tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                    .unwrap()
                    .to_bits(),
                expected,
                "input {input:08x}"
            );
        }
        for control in StompWhammyKnob::ALL {
            for step in 0..=63 {
                let quantized = ((step as f32) * WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL).to_bits();
                let mut tone = PodTone::parse(original.clone()).unwrap();
                tone.set_stomp_whammy_knob_position(control, f32::from_bits(quantized))
                    .unwrap();
                assert_eq!(
                    tone.stomp_whammy_knob_position(control).unwrap().to_bits(),
                    quantized,
                    "{control:?}, step {step}"
                );
            }
        }
    }

    #[test]
    fn whammy_knob_rejects_invalid_inputs_and_existing_values_without_mutation() {
        let valid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
        let original = whammy_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&valid],
        );
        for input in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert!(
                tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, input)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), original);
        }
        for bits in [
            f32::NAN.to_bits(),
            f32::INFINITY.to_bits(),
            f32::NEG_INFINITY.to_bits(),
            (-0.01f32).to_bits(),
            1.01f32.to_bits(),
        ] {
            let invalid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, bits);
            let bytes = whammy_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_WHAMMY_INTERVAL_MODEL_ID,
                &[&invalid],
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(
                tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                    .is_err()
            );
            assert!(
                tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.75)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), bytes);
        }
        let imported_raw = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e12_3456);
        let bytes = whammy_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&imported_raw],
        );
        let tone = PodTone::parse(bytes.clone()).unwrap();
        assert_eq!(
            tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            0x3e12_3456
        );
        assert_eq!(
            tone.as_bytes(),
            bytes,
            "reading an unquantized imported value is lossless"
        );
    }

    #[test]
    fn whammy_knob_admission_checks_target_version_model_slot_record_prefix_type_and_opaque_data() {
        let valid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
        for bytes in [
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                STOMP_WHAMMY_INTERVAL_MODEL_ID,
                &[&valid],
            ),
            stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_000d, &[&valid]),
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_WHAMMY_INTERVAL_MODEL_ID,
                &[],
            ),
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_WHAMMY_INTERVAL_MODEL_ID,
                &[&drive_record(0x3e, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000)],
            ),
            stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_WHAMMY_INTERVAL_MODEL_ID,
                &[&drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 2, 0x3e80_0000)],
            ),
        ] {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(
                tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                    .is_err()
            );
            assert!(
                tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), bytes);
        }
        let mut wrong_target = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
                tail: &[],
                parameters: &[&valid],
            }],
        );
        let pinf = find_chunk_data(&wrong_target, b"PINF");
        wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000bu32.to_be_bytes());
        assert!(matches!(
            PodTone::parse(wrong_target),
            Err(ToneError::WrongTarget {
                device_id: 0x0003_000b
            })
        ));

        let wrong_slot = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
                tail: &[],
                parameters: &[&valid],
            }],
        );
        let mut tone = PodTone::parse(wrong_slot.clone()).unwrap();
        assert!(
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), wrong_slot);

        let opaque = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
                tail: &[],
                parameters: &[&valid, b"\x00\x00"],
            }],
        );
        let mut tone = PodTone::parse(opaque.clone()).unwrap();
        assert!(
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), opaque);
    }

    #[test]
    fn whammy_knob_library_and_bundle_edits_revert_save_reopen_and_preserve_first_duplicate() {
        let early = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e00_0000);
        let later = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
        let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0x3f00_0000);
        let original = whammy_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&early, &later, &toe],
        );
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        assert_eq!(
            library
                .stomp_whammy_knob_position(index, StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            0x3e80_0000,
            "last matching source record is selected"
        );
        library
            .set_stomp_whammy_knob_position(index, StompWhammyKnob::Heel, 0.75)
            .unwrap();
        assert!(library.is_dirty(index));
        let edited = library.tone(index).unwrap().as_bytes().to_vec();
        let changed: Vec<_> = original
            .iter()
            .zip(&edited)
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert_eq!(changed.len(), 4);
        let (_, parameter_index) = library
            .tone(index)
            .unwrap()
            .stomp_whammy_knob_location(StompWhammyKnob::Heel)
            .unwrap();
        let model_index = library
            .tone(index)
            .unwrap()
            .models
            .iter()
            .position(|model| model.slot == STOMP_SLOT)
            .unwrap();
        let selected = library.tone(index).unwrap().parameter_value_offsets[model_index]
            [parameter_index]
            .unwrap();
        assert_eq!(changed, (selected..selected + 4).collect::<Vec<_>>());
        let first_parameter = library.tone(index).unwrap().models[model_index].parameters.iter().position(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == STOMP_WHAMMY_HEEL_PARAM)).unwrap();
        let earlier = library.tone(index).unwrap().parameter_value_offsets[model_index]
            [first_parameter]
            .unwrap();
        assert_eq!(
            &edited[earlier..earlier + 4],
            &original[earlier..earlier + 4]
        );
        library.revert_unsaved(index).unwrap();
        assert!(!library.is_dirty(index));
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        library
            .set_stomp_whammy_knob_position(index, StompWhammyKnob::Heel, 0.75)
            .unwrap();

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("whammy-copy.l6t");
        std::fs::write(&path, b"keep existing copy").unwrap();
        assert!(library.tone(index).unwrap().save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep existing copy");
        let successful = directory.path().join("whammy-saved.l6t");
        library.tone(index).unwrap().save_copy(&successful).unwrap();
        let reopened = PodTone::parse(std::fs::read(successful).unwrap()).unwrap();
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            0x3f3c_bcbd
        );
        assert_eq!(
            reopened
                .stomp_whammy_knob_position(StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            0x3f00_0000
        );

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        bundle
            .set_stomp_whammy_knob_position(0, StompWhammyKnob::Toe, 0.25)
            .unwrap();
        assert!(bundle.is_dirty());
        let reopened_bundle = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
        assert_eq!(
            reopened_bundle
                .stomp_whammy_knob_position(0, StompWhammyKnob::Toe)
                .unwrap()
                .to_bits(),
            0x3e70_f0f2
        );
        assert_eq!(
            reopened_bundle
                .stomp_whammy_knob_position(0, StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            0x3e80_0000
        );
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
    fn pod_membership_edits_splice_only_admitted_folder_tone_spans_transactionally() {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }

        let fixed = live_tone();
        let first = live_tone();
        let second = tone_with_empty_unknown_chunk(&live_tone());
        let before = chunk(b"BEFR", &[0x11, 0x22, 0x33]);
        let middle = chunk(b"MIDD", &[0x44, 0x55, 0x66]);
        let after = chunk(b"AFTR", &[0x77, 0x88, 0x99]);
        let original_children = vec![
            before.clone(),
            first.clone(),
            middle.clone(),
            second.clone(),
            after.clone(),
        ];
        let original = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &original_children);
        let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
        assert_eq!(library.bank_count(), 3);
        assert!(!library.membership_editable(0));
        assert!(library.membership_editable(1));
        assert!(library.membership_editable(2));

        let duplicate_index = library.duplicate_entry(1, 0).unwrap();
        let duplicated_children = vec![
            before.clone(),
            first.clone(),
            first.clone(),
            middle.clone(),
            second.clone(),
            after.clone(),
        ];
        let duplicated =
            membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &duplicated_children);
        assert_eq!(library.as_bytes(), duplicated);
        assert_eq!(duplicate_index, 2);
        assert_eq!(library.entry_location(duplicate_index), Some((1, 1)));
        assert_eq!(library.tone(1, 1).unwrap().as_bytes(), first);
        assert!(library.is_dirty());
        let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
        assert_eq!(reopened.as_bytes(), duplicated);

        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("membership-copy.l6c");
        library.save_copy(&copy).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), duplicated);
        assert!(library.is_dirty(), "save-copy keeps the existing baseline");
        assert!(
            library.save_copy(&copy).is_err(),
            "save-copy refuses overwrite"
        );
        library.revert().unwrap();
        assert_eq!(library.as_bytes(), original);
        assert!(!library.is_dirty());

        let selected = library.reorder_entry(1, 0, 1).unwrap();
        let reordered_children = vec![
            before.clone(),
            second.clone(),
            middle.clone(),
            first.clone(),
            after.clone(),
        ];
        let reordered =
            membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &reordered_children);
        assert_eq!(library.as_bytes(), reordered);
        assert_eq!(selected, 2);
        assert_eq!(library.tone(1, 1).unwrap().as_bytes(), first);
        let unchanged = library.as_bytes().to_vec();
        assert_eq!(library.reorder_entry(1, 1, 1).unwrap(), 2);
        assert_eq!(
            library.as_bytes(),
            unchanged,
            "same-slot reorder is a no-op"
        );

        let selected = library.remove_entry(1, 0).unwrap();
        let removed_children = vec![before.clone(), middle.clone(), first.clone(), after.clone()];
        let removed = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &removed_children);
        assert_eq!(library.as_bytes(), removed);
        assert_eq!(selected, 1);
        assert_eq!(library.tone(1, 0).unwrap().as_bytes(), first);
        let empty_folder_children = vec![before, middle, after];
        let empty_folder = membership_bundle(
            b"L6CO",
            std::slice::from_ref(&fixed),
            &empty_folder_children,
        );
        library.remove_entry(1, 0).unwrap();
        assert_eq!(library.as_bytes(), empty_folder);
        assert_eq!(library.preset_count(1), Some(0));
        assert_eq!(
            PodBundleLibrary::parse(empty_folder.clone())
                .unwrap()
                .as_bytes(),
            empty_folder
        );

        let unchanged = library.as_bytes().to_vec();
        assert!(library.duplicate_entry(1, 0).is_err());
        assert!(library.remove_entry(0, 0).is_err());
        assert!(library.reorder_entry(1, 0, 1).is_err());
        assert_eq!(library.as_bytes(), unchanged);

        let fixed_root = membership_bundle(
            b"L6BA",
            std::slice::from_ref(&fixed),
            std::slice::from_ref(&second),
        );
        let mut fixed_library = PodBundleLibrary::parse(fixed_root.clone()).unwrap();
        assert!(!fixed_library.membership_editable(1));
        assert!(fixed_library.duplicate_entry(1, 0).is_err());
        assert_eq!(fixed_library.as_bytes(), fixed_root);

        let only_tone = membership_bundle(b"L6CO", &[], std::slice::from_ref(&second));
        let mut last_tone = PodBundleLibrary::parse(only_tone.clone()).unwrap();
        assert!(last_tone.remove_entry(1, 0).is_err());
        assert_eq!(last_tone.as_bytes(), only_tone);
        assert!(!last_tone.is_dirty());

        let base = membership_bundle(
            b"L6CO",
            std::slice::from_ref(&fixed),
            std::slice::from_ref(&second),
        );
        let filler_len = BUNDLE_BYTES_CEILING as usize - base.len() - 8;
        assert_eq!(filler_len % 2, 0);
        let filler = chunk(b"FILL", &vec![0x6d; filler_len]);
        let capped = membership_bundle(b"L6CO", &[live_tone()], &[second, filler]);
        assert_eq!(capped.len() as u64, BUNDLE_BYTES_CEILING);
        let mut capped_library = PodBundleLibrary::parse(capped.clone()).unwrap();
        assert!(capped_library.duplicate_entry(1, 0).is_err());
        assert_eq!(capped_library.as_bytes(), capped);
        assert!(!capped_library.is_dirty());
    }

    #[test]
    fn fixed_bank_tone_reorder_preserves_raw_spans_tokens_lengths_and_baseline() {
        fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut bytes = id.to_vec();
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(0xa5);
            }
            bytes
        }
        fn tone(name: u16, extra_chunks: usize) -> Vec<u8> {
            let mut bytes = fixture(
                ContainerKind::L6pa,
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
                &[(AMP_SLOT, 0x1111_0002)],
            );
            let pinf = find_chunk_data(&bytes, b"PINF");
            bytes[pinf + 8..pinf + 10].copy_from_slice(&name.to_be_bytes());
            let junk = find_chunk_data(&bytes, b"JUNK");
            bytes[junk..junk + 3].copy_from_slice(&[
                name as u8,
                (name >> 8) as u8,
                (name as u8) ^ 0x5a,
            ]);
            for _ in 0..extra_chunks {
                bytes.extend_from_slice(&chunk(b"EXTR", &[name as u8; 3]));
            }
            let form_length = bytes.len() - 8;
            bytes[4..8].copy_from_slice(&(form_length as u32).to_be_bytes());
            bytes
        }
        fn bank_length(bytes: &[u8]) -> u32 {
            let kind = bytes
                .windows(4)
                .position(|window| window == b"BANK")
                .expect("fixed BANK type");
            u32::from_be_bytes(bytes[kind - 4..kind].try_into().unwrap())
        }

        let first = tone(u16::from(b'A'), 0);
        let second = tone(u16::from(b'B'), 1);
        let third = tone(u16::from(b'C'), 2);
        assert_ne!(first.len(), second.len());
        assert_ne!(second.len(), third.len());
        let before = chunk(b"BEFR", &[0x11, 0x22, 0x33]);
        let middle_one = chunk(b"MID1", &[0x44, 0x55, 0x66]);
        let middle_two = chunk(b"MID2", &[0x77, 0x88, 0x99]);
        let after = chunk(b"AFTR", &[0xaa, 0xbb, 0xcc]);
        let other_bank_tone = tone(u16::from(b'Z'), 0);
        let original_children = vec![
            before.clone(),
            first.clone(),
            middle_one.clone(),
            second.clone(),
            middle_two.clone(),
            third.clone(),
            after.clone(),
        ];
        let original = membership_bundle(
            b"L6BA",
            &original_children,
            std::slice::from_ref(&other_bank_tone),
        );
        let expected_children = vec![
            before.clone(),
            second.clone(),
            middle_one.clone(),
            third.clone(),
            middle_two.clone(),
            first.clone(),
            after.clone(),
        ];
        let expected = membership_bundle(
            b"L6BA",
            &expected_children,
            std::slice::from_ref(&other_bank_tone),
        );
        let root_length = preset::word(&original, 4);
        let fixed_bank_length = bank_length(&original);
        let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
        assert!(library.fixed_bank_reorder_editable(0));
        assert!(!library.fixed_bank_reorder_editable(1));
        assert!(!library.is_dirty());

        let unchanged = library.as_bytes().to_vec();
        assert_eq!(library.reorder_fixed_bank_entry(0, 1, 1).unwrap(), 1);
        assert_eq!(library.as_bytes(), unchanged);
        assert!(!library.is_dirty(), "same-slot move is a clean byte no-op");

        assert_eq!(library.reorder_fixed_bank_entry(0, 0, 2).unwrap(), 2);
        assert_eq!(library.as_bytes(), expected);
        assert_eq!(preset::word(library.as_bytes(), 4), root_length);
        assert_eq!(bank_length(library.as_bytes()), fixed_bank_length);
        assert!(library.is_dirty());
        assert_eq!(library.preset_count(0), Some(3));
        assert_eq!(library.tone(0, 0).unwrap().as_bytes(), second);
        assert_eq!(library.tone(0, 1).unwrap().as_bytes(), third);
        assert_eq!(library.tone(0, 2).unwrap().as_bytes(), first);
        assert_eq!(library.tone(1, 0).unwrap().as_bytes(), other_bank_tone);
        let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
        assert_eq!(reopened.as_bytes(), expected);
        assert_eq!(reopened.tone(0, 0).unwrap().as_bytes(), second);
        assert_eq!(reopened.tone(0, 1).unwrap().as_bytes(), third);
        assert_eq!(reopened.tone(0, 2).unwrap().as_bytes(), first);

        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("fixed-bank.vwb");
        library.save_copy(&copy).unwrap();
        let saved_copy = std::fs::read(&copy).unwrap();
        assert_eq!(saved_copy, expected);
        assert_eq!(
            PodBundleLibrary::parse(saved_copy)
                .unwrap()
                .tone(0, 0)
                .unwrap()
                .as_bytes(),
            second
        );
        assert!(library.save_copy(&copy).is_err());
        assert_eq!(std::fs::read(&copy).unwrap(), expected);
        assert!(library.is_dirty(), "saving a copy does not change baseline");

        assert_eq!(library.reorder_fixed_bank_entry(0, 1, 1).unwrap(), 1);
        assert_eq!(library.as_bytes(), expected);
        assert!(
            library.is_dirty(),
            "no-op preserves an existing dirty state"
        );
        assert_eq!(library.reorder_fixed_bank_entry(0, 2, 0).unwrap(), 0);
        assert_eq!(library.as_bytes(), original);
        assert!(!library.is_dirty(), "returning to saved bytes is clean");

        let before_rejections = library.as_bytes().to_vec();
        assert!(library.reorder_fixed_bank_entry(0, 0, 3).is_err());
        assert!(library.reorder_fixed_bank_entry(99, 0, 0).is_err());
        assert_eq!(library.as_bytes(), before_rejections);
        assert!(!library.is_dirty());
    }

    #[test]
    fn fixed_bank_tone_reorder_rejects_other_roots_banks_and_patch_types_atomically() {
        let tone = || {
            fixture(
                ContainerKind::L6pa,
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
                &[(AMP_SLOT, 0x1111_0002)],
            )
        };
        let alternate = fixture(
            ContainerKind::L6fs,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[(AMP_SLOT, 0x1111_0002)],
        );
        let amp_setup = live_tone();
        let live = tone();

        let mixed_root_child = membership_bundle(
            b"L6BA",
            std::slice::from_ref(&live),
            std::slice::from_ref(&live),
        );
        let mut mixed_root = PodBundleLibrary::parse(mixed_root_child).unwrap();
        let original = mixed_root.as_bytes().to_vec();
        assert!(!mixed_root.fixed_bank_reorder_editable(1));
        assert!(mixed_root.reorder_fixed_bank_entry(1, 0, 0).is_err());
        assert_eq!(mixed_root.as_bytes(), original);

        let collection_bank = membership_bundle(
            b"L6CO",
            std::slice::from_ref(&live),
            std::slice::from_ref(&live),
        );
        let mut collection = PodBundleLibrary::parse(collection_bank).unwrap();
        let original = collection.as_bytes().to_vec();
        assert!(!collection.fixed_bank_reorder_editable(0));
        assert!(collection.reorder_fixed_bank_entry(0, 0, 0).is_err());
        assert_eq!(collection.as_bytes(), original);

        let empty_fixed = membership_bundle(b"L6BA", &[], std::slice::from_ref(&live));
        let mut empty = PodBundleLibrary::parse(empty_fixed).unwrap();
        let original = empty.as_bytes().to_vec();
        assert!(!empty.fixed_bank_reorder_editable(0));
        assert!(empty.reorder_fixed_bank_entry(0, 0, 0).is_err());
        assert_eq!(empty.as_bytes(), original);

        for unsupported in [alternate, amp_setup] {
            let mixed = membership_bundle(b"L6BA", &[live.clone(), unsupported], &[]);
            let mut library = PodBundleLibrary::parse(mixed).unwrap();
            let original = library.as_bytes().to_vec();
            assert!(!library.fixed_bank_reorder_editable(0));
            assert!(library.reorder_fixed_bank_entry(0, 0, 1).is_err());
            assert_eq!(library.as_bytes(), original);
            assert!(!library.is_dirty());
        }

        let mut wrong_target = live.clone();
        let pinf = find_chunk_data(&wrong_target, b"PINF");
        wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000b_u32.to_be_bytes());
        let wrong_target_bundle =
            membership_bundle(b"L6BA", &[wrong_target], std::slice::from_ref(&live));
        assert!(PodBundleLibrary::parse(wrong_target_bundle).is_err());
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

    #[test]
    fn delay_note_choices_match_source_rounding_at_endpoints_and_interior_tempo() {
        let expected = [
            (0x3f80_0000, 0x3f80_0000, 0x3efd_6a06),
            (0x3f80_0000, 0x3f3f_5a81, 0x3ebc_c486),
            (0x3f80_0000, 0x3efd_6a06, 0x3e78_3e11),
            (0x3f80_0000, 0x3ea7_3807, 0x3e22_0c12),
            (0x3f80_0000, 0x3ebc_c486, 0x3e37_9891),
            (0x3f80_0000, 0x3e78_3e11, 0x3ded_e625),
            (0x3f29_ce02, 0x3e22_0c12, 0x3d97_b426),
            (0x3f3f_5a81, 0x3e37_9891, 0x3dad_40a5),
            (0x3efd_6a06, 0x3ded_e625, 0x3d59_364f),
            (0x3ea7_3807, 0x3d97_b426, 0x3d03_0450),
            (0x3ebc_c486, 0x3dad_40a5, 0x3d18_90cf),
            (0x3e78_3e11, 0x3d59_364f, 0x3caf_d6a1),
            (0x3e22_0c12, 0x3d03_0450, 0x3c33_4945),
        ];
        let tempos = [30.0_f32, 120.0_f32, 240.0_f32];
        for (index, choice) in DelayNoteChoice::ALL.into_iter().enumerate().skip(1) {
            for (tempo_index, tempo) in tempos.into_iter().enumerate() {
                let mut tone = PodTone::parse(delay_note_fixture(
                    DELAY_SLOT,
                    0x0202_0002,
                    0,
                    0x3e80_0000,
                    Some(MOD_SLOT),
                    MOD_CONTROL_MODEL_IDS[0],
                    0,
                    Some(tempo),
                ))
                .unwrap();
                tone.set_delay_note_choice(choice).unwrap();
                assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), index as u32);
                let expected_bits = match tempo_index {
                    0 => expected[index - 1].0,
                    1 => expected[index - 1].1,
                    _ => expected[index - 1].2,
                };
                assert_eq!(
                    parm_bits(&tone, DELAY_SLOT, 0x0010_0000),
                    expected_bits,
                    "NOTE {index}, BPM {tempo}"
                );
            }
        }
    }

    #[test]
    fn mod_note_speed_source_events_match_independent_literal_words() {
        let original = delay_note_fixture(
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_mod_note_choice(ModNoteChoice::Quarter).unwrap();
        let mut expected = original.clone();
        patch_expected_parameter(&tone, &mut expected, MOD_SLOT, 0x0020_0000, 6);
        patch_expected_parameter(&tone, &mut expected, MOD_SLOT, 0x0010_0000, 0x3e02_93c2);
        assert_eq!(tone.as_bytes(), expected);
        assert_eq!(
            tone.mod_note_value().unwrap(),
            ModNoteValue::Known(ModNoteChoice::Quarter)
        );

        let mut manual = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            6,
            None,
        ))
        .unwrap();
        let before_manual = manual.as_bytes().to_vec();
        manual.set_mod_speed_hz(3.0).unwrap();
        let mut expected_manual = before_manual;
        patch_expected_parameter(
            &manual,
            &mut expected_manual,
            MOD_SLOT,
            0x0010_0000,
            0x3e47_4d51,
        );
        patch_expected_parameter(&manual, &mut expected_manual, MOD_SLOT, 0x0020_0000, 0);
        assert_eq!(manual.as_bytes(), expected_manual);
    }

    #[test]
    fn mod_note_expected_speed_words_cover_every_choice_and_tempo_boundary() {
        let expected = [
            [
                0x3adb_eb61,
                0x3b92_9ced,
                0x3c24_f08a,
                0x3c97_31d4,
                0x3c80_494f,
                0x3cdb_eb62,
                0x3d32_af40,
                0x3d1b_c6bb,
                0x3d77_68cf,
                0x3dc0_6df6,
                0x3da9_8571,
                0x3e02_93c2,
                0x3e47_4d51,
            ],
            [
                0x3cdb_eb62,
                0x3d1b_c6bb,
                0x3d77_68cf,
                0x3dc0_6df6,
                0x3da9_8571,
                0x3e02_93c2,
                0x3e47_4d51,
                0x3e30_64cd,
                0x3e86_0370,
                0x3eca_bcff,
                0x3eb3_d47a,
                0x3f07_bb47,
                0x3f4c_74d5,
            ],
            [
                0x3d77_68cf,
                0x3da9_8571,
                0x3e02_93c2,
                0x3e47_4d51,
                0x3e30_64cd,
                0x3e86_0370,
                0x3eca_bcff,
                0x3eb3_d47a,
                0x3f07_bb47,
                0x3f4c_74d5,
                0x3f35_8c51,
                0x3f80_0000,
                0x3f80_0000,
            ],
        ];
        for (tempo_index, tempo) in [30.0_f32, 120.0, 240.0].into_iter().enumerate() {
            for (choice_index, choice) in ModNoteChoice::ALL.into_iter().enumerate().skip(1) {
                let bytes = delay_note_fixture(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    0,
                    0x3e80_0000,
                    Some(MOD_SLOT),
                    MOD_CONTROL_MODEL_IDS[0],
                    0,
                    Some(tempo),
                );
                let mut tone = PodTone::parse(bytes).unwrap();
                tone.set_mod_note_choice(choice).unwrap();
                assert_eq!(
                    parm_bits(&tone, MOD_SLOT, 0x0010_0000),
                    expected[tempo_index][choice_index - 1],
                    "Mod NOTE {choice_index} at {tempo} BPM"
                );
            }
        }
    }

    #[test]
    fn mod_note_model_slots_and_manual_speed_lower_boundary_are_bounded() {
        for slot in [MOD_SLOT, 0x0005_0003] {
            for model_id in MOD_CONTROL_MODEL_IDS {
                for delay_slot in [DELAY_SLOT, 0x0005_0004] {
                    let bytes = mod_operation_fixture(&[
                        (slot, model_id, mod_test_rows(0, 0x3e80_0000)),
                        (
                            delay_slot,
                            DELAY_CONTROL_MODEL_IDS[0],
                            mod_test_delay_rows(0),
                        ),
                        (0x0001_0000, 0x3e70_0000, mod_test_system_rows(120.0)),
                    ]);
                    let mut tone = PodTone::parse(bytes.clone()).unwrap();
                    let mut expected = bytes;
                    patch_expected_parameter(&tone, &mut expected, slot, 0x0020_0000, 6);
                    patch_expected_parameter(&tone, &mut expected, slot, 0x0010_0000, 0x3e02_93c2);
                    tone.set_mod_note_choice(ModNoteChoice::Quarter).unwrap();
                    assert_eq!(
                        tone.as_bytes(),
                        expected,
                        "Mod model {model_id:08x}, slot {slot:08x}, Delay slot {delay_slot:08x}"
                    );
                }
            }
        }
        let mut lower = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            None,
        ))
        .unwrap();
        lower.set_mod_speed_hz(0.1).unwrap();
        assert_eq!(parm_bits(&lower, MOD_SLOT, 0x0010_0000), 0x0000_0000);

        let mod_speed = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
        let mod_note = drive_record(0x3f, 0x0020_0000, 0, 6);
        let model = BuiltModel {
            slot: MOD_SLOT,
            model_id: MOD_CONTROL_MODEL_IDS[0],
            tail: &[1, 2, 3, 4],
            parameters: &[&mod_speed, &mod_note],
        };
        let bytes = custom_tone(ContainerKind::L6as, &[model]);
        let mut isolated_speed = PodTone::parse(bytes).unwrap();
        isolated_speed.set_mod_speed_hz(3.0).unwrap();
        assert_eq!(
            parm_bits(&isolated_speed, MOD_SLOT, 0x0010_0000),
            0x3e47_4d51
        );
        assert_eq!(parm_bits(&isolated_speed, MOD_SLOT, 0x0020_0000), 0);
    }

    #[test]
    fn mod_note_off_isolated_and_positive_cross_effect_guard_is_atomic() {
        let mut off = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            6,
            None,
        ))
        .unwrap();
        let original = off.as_bytes().to_vec();
        off.set_mod_note_choice(ModNoteChoice::Off).unwrap();
        let mut expected = original;
        patch_expected_parameter(&off, &mut expected, MOD_SLOT, 0x0020_0000, 0);
        assert_eq!(off.as_bytes(), expected);

        let delay_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
        let delay_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let invalid_speed_type = drive_record(0x3f, 0x0010_0000, 0, 0x3e80_0000).to_vec();
        let active_mod_note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
        let isolated_models = vec![
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![delay_time, delay_note],
            ),
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                vec![invalid_speed_type, active_mod_note],
            ),
        ];
        let mut no_speed_read = PodTone::parse(delay_context_fixture(
            &isolated_models,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ))
        .unwrap();
        let before = no_speed_read.as_bytes().to_vec();
        no_speed_read
            .set_mod_note_choice(ModNoteChoice::Off)
            .unwrap();
        let mut expected = before;
        patch_expected_parameter(&no_speed_read, &mut expected, MOD_SLOT, 0x0020_0000, 0);
        assert_eq!(no_speed_read.as_bytes(), expected);

        let mut guarded = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            6,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        ))
        .unwrap();
        let before = guarded.as_bytes().to_vec();
        assert!(guarded.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
        assert_eq!(guarded.as_bytes(), before);
    }

    #[test]
    fn delay_positive_note_uses_source_default_system_id_only() {
        for system_id in [0x3e70_0000, 0x3e80_0000, 0xdead_beef] {
            let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits());
            let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
            let note = drive_record(0x3f, 0x0020_0000, 0, 0);
            let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0);
            let models = vec![
                (
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![time.to_vec(), note.to_vec()],
                ),
                (MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mod_note.to_vec()]),
                (0x0001_0000, system_id, vec![tempo.to_vec()]),
            ];
            let mut tone = PodTone::parse(delay_context_fixture(
                &models,
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ))
            .unwrap();
            let original = tone.as_bytes().to_vec();
            let result = tone.set_delay_note_choice(DelayNoteChoice::Quarter);
            if system_id == 0x3e70_0000 {
                assert!(result.is_ok());
                assert_ne!(tone.as_bytes(), original);
            } else {
                assert!(result.is_err());
                assert_eq!(tone.as_bytes(), original);
            }
        }
    }

    #[test]
    fn mod_note_and_speed_rejections_are_transactional() {
        let tempo = || vec![drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()];
        let delay_rows = |note| {
            vec![
                drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
            ]
        };
        let build = |mod_rows: Vec<Vec<u8>>,
                     delays: Vec<(u32, u32, Vec<Vec<u8>>)>,
                     systems: Vec<(u32, u32, Vec<Vec<u8>>)>| {
            let mut models = vec![(MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], mod_rows)];
            models.extend(delays);
            models.extend(systems);
            mod_operation_fixture(&models)
        };
        let good_delay = || vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0))];
        let good_system = || vec![(0x0001_0000, 0x3e70_0000, tempo())];
        let speed = || drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
        let note = |prefix, kind, value| drive_record(prefix, 0x0020_0000, kind, value).to_vec();
        let reject_both = |bytes: Vec<u8>, label: &str| {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(
                tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err(),
                "{label} NOTE"
            );
            assert_eq!(tone.as_bytes(), before, "{label} NOTE");
            let mut tone = PodTone::parse(bytes).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_mod_speed_hz(3.0).is_err(), "{label} speed");
            assert_eq!(tone.as_bytes(), before, "{label} speed");
        };

        let mut malformed_mod = vec![
            ("missing NOTE", vec![speed()]),
            (
                "duplicate NOTE",
                vec![speed(), note(0x3f, 0, 0), note(0x3f, 0, 0)],
            ),
            ("wrong-type NOTE", vec![speed(), note(0x3f, 1, 0)]),
            ("noncanonical NOTE", vec![speed(), note(0x3e, 0, 0)]),
            ("unknown NOTE", vec![speed(), note(0x3f, 0, 14)]),
            ("missing SPEED", vec![note(0x3f, 0, 0)]),
            ("duplicate SPEED", vec![speed(), speed(), note(0x3f, 0, 0)]),
            (
                "wrong-type SPEED",
                vec![
                    drive_record(0x3f, 0x0010_0000, 0, 0).to_vec(),
                    note(0x3f, 0, 0),
                ],
            ),
            (
                "noncanonical SPEED",
                vec![
                    drive_record(0x3e, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                    note(0x3f, 0, 0),
                ],
            ),
            (
                "nonfinite SPEED",
                vec![
                    drive_record(0x3f, 0x0010_0000, 1, f32::INFINITY.to_bits()).to_vec(),
                    note(0x3f, 0, 0),
                ],
            ),
            (
                "out-of-range SPEED",
                vec![
                    drive_record(0x3f, 0x0010_0000, 1, 1.01_f32.to_bits()).to_vec(),
                    note(0x3f, 0, 0),
                ],
            ),
            (
                "opaque Mod PARM",
                vec![speed(), note(0x3f, 0, 0), vec![0x81, 0x82, 0x83]],
            ),
        ];
        for (label, rows) in malformed_mod.drain(..) {
            reject_both(build(rows, good_delay(), good_system()), label);
        }

        let missing_mod = build(vec![], good_delay(), good_system());
        let mut tone = PodTone::parse(missing_mod.clone()).unwrap();
        assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
        assert_eq!(tone.as_bytes(), missing_mod);
        let duplicate_mods = mod_operation_fixture(&[
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                vec![speed(), note(0x3f, 0, 0)],
            ),
            (
                0x0005_0003,
                MOD_CONTROL_MODEL_IDS[1],
                vec![speed(), note(0x3f, 0, 0)],
            ),
            (DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0)),
            (0x0001_0000, 0x3e70_0000, tempo()),
        ]);
        reject_both(duplicate_mods.clone(), "duplicate Mod slots");
        let mut tone = PodTone::parse(duplicate_mods.clone()).unwrap();
        assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
        assert_eq!(tone.as_bytes(), duplicate_mods);

        for (label, rows) in [
            ("missing Delay", vec![]),
            (
                "active Delay NOTE",
                vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(6))],
            ),
            (
                "duplicate Delay slots",
                vec![
                    (DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0)),
                    (0x0005_0004, DELAY_CONTROL_MODEL_IDS[1], delay_rows(0)),
                ],
            ),
            (
                "unknown Delay model",
                vec![(DELAY_SLOT, 0xdead_beef, delay_rows(0))],
            ),
            (
                "missing Delay NOTE",
                vec![(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec()],
                )],
            ),
            (
                "wrong-type Delay NOTE",
                vec![(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![
                        drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                        note(0x3f, 1, 0),
                    ],
                )],
            ),
            (
                "noncanonical Delay NOTE",
                vec![(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![
                        drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                        note(0x3e, 0, 0),
                    ],
                )],
            ),
            (
                "duplicate Delay NOTE",
                vec![(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![
                        drive_record(0x3f, 0x0020_0000, 0, 0).to_vec(),
                        note(0x3f, 0, 0),
                    ],
                )],
            ),
            (
                "unknown Delay NOTE",
                vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(14))],
            ),
            (
                "opaque Delay PARM",
                vec![(
                    DELAY_SLOT,
                    DELAY_CONTROL_MODEL_IDS[0],
                    vec![
                        drive_record(0x3f, 0x0020_0000, 0, 0).to_vec(),
                        vec![0x81, 0x82, 0x83],
                    ],
                )],
            ),
        ] {
            let bytes = build(vec![speed(), note(0x3f, 0, 0)], rows, good_system());
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            for choice in [ModNoteChoice::Off, ModNoteChoice::Quarter] {
                let before = tone.as_bytes().to_vec();
                assert!(tone.set_mod_note_choice(choice).is_err(), "{label}");
                assert_eq!(tone.as_bytes(), before, "{label}");
            }
        }

        let mut invalid_systems = vec![
            ("missing System", vec![]),
            ("wrong System ID", vec![(0x0001_0000, 0x3e80_0000, tempo())]),
            (
                "unknown System ID",
                vec![(0x0001_0000, 0xdead_beef, tempo())],
            ),
            ("missing Tempo", vec![(0x0001_0000, 0x3e70_0000, vec![])]),
            (
                "duplicate Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![tempo()[0].clone(), tempo()[0].clone()],
                )],
            ),
            (
                "wrong-type Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 0, 120).to_vec()],
                )],
            ),
            (
                "noncanonical Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![drive_record(0x3e, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()],
                )],
            ),
            (
                "low Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 1, 29.0_f32.to_bits()).to_vec()],
                )],
            ),
            (
                "high Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 1, 241.0_f32.to_bits()).to_vec()],
                )],
            ),
            (
                "nonfinite Tempo",
                vec![(
                    0x0001_0000,
                    0x3e70_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 1, f32::NAN.to_bits()).to_vec()],
                )],
            ),
        ];
        for (label, systems) in invalid_systems.drain(..) {
            let bytes = build(vec![speed(), note(0x3f, 0, 0)], good_delay(), systems);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(
                tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err(),
                "{label}"
            );
            assert_eq!(tone.as_bytes(), before, "{label}");
        }
        let duplicate_system = build(
            vec![speed(), note(0x3f, 0, 0)],
            good_delay(),
            vec![
                (0x0001_0000, 0x3e70_0000, tempo()),
                (0x0001_0000, 0x3e70_0000, tempo()),
            ],
        );
        assert!(PodTone::parse(duplicate_system).is_err());

        let already_matching = mod_operation_fixture(&[
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                mod_test_rows(6, 0x3e02_93c2),
            ),
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                mod_test_delay_rows(0),
            ),
            (0x0001_0000, 0x3e80_0000, tempo()),
        ]);
        let mut tone = PodTone::parse(already_matching.clone()).unwrap();
        assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
        assert_eq!(tone.as_bytes(), already_matching);

        let mut note_off_without_speed_or_tempo = PodTone::parse(mod_operation_fixture(&[
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                vec![drive_record(0x3f, 0x0020_0000, 0, 6).to_vec()],
            ),
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                mod_test_delay_rows(0),
            ),
        ]))
        .unwrap();
        let before = note_off_without_speed_or_tempo.as_bytes().to_vec();
        let mut expected = before;
        patch_expected_parameter(
            &note_off_without_speed_or_tempo,
            &mut expected,
            MOD_SLOT,
            0x0020_0000,
            0,
        );
        note_off_without_speed_or_tempo
            .set_mod_note_choice(ModNoteChoice::Off)
            .unwrap();
        assert_eq!(note_off_without_speed_or_tempo.as_bytes(), expected);

        let mut same_speed_clears_note = PodTone::parse(mod_operation_fixture(&[
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                mod_test_rows(6, 0x3e02_93c2),
            ),
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                mod_test_delay_rows(0),
            ),
        ]))
        .unwrap();
        let before = same_speed_clears_note.as_bytes().to_vec();
        let mut expected = before;
        patch_expected_parameter(
            &same_speed_clears_note,
            &mut expected,
            MOD_SLOT,
            0x0020_0000,
            0,
        );
        same_speed_clears_note.set_mod_speed_hz(2.0).unwrap();
        assert_eq!(same_speed_clears_note.as_bytes(), expected);

        for invalid_hz in [f32::NAN, f32::INFINITY, 0.099, 15.001] {
            let bytes = mod_operation_fixture(&[(
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                mod_test_rows(0, 0x3e80_0000),
            )]);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_mod_speed_hz(invalid_hz).is_err());
            assert_eq!(tone.as_bytes(), bytes);
        }

        let invalid_speed_library = mod_operation_fixture(&[
            (
                MOD_SLOT,
                MOD_CONTROL_MODEL_IDS[0],
                mod_test_rows(0, 0x3e80_0000),
            ),
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                mod_test_delay_rows(0),
            ),
            (0x0001_0000, 0x3e80_0000, tempo()),
        ]);
        let mut library = Library::default();
        library.import_bytes(invalid_speed_library.clone()).unwrap();
        let mut edited = PodTone::parse(invalid_speed_library.clone()).unwrap();
        edited.rename("Unsaved baseline").unwrap();
        library.replace(0, edited.into_bytes()).unwrap();
        let bytes_before = library.tone(0).unwrap().as_bytes().to_vec();
        let saved_before = library.tones[0].saved.clone();
        assert!(library.is_dirty(0));
        assert!(
            library
                .set_mod_note_choice(0, ModNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(library.tone(0).unwrap().as_bytes(), bytes_before);
        assert_eq!(library.tones[0].saved, saved_before);
        assert!(library.is_dirty(0));

        let bundle_bytes = membership_bundle(b"L6CO", &[invalid_speed_library], &[]);
        let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
        bundle
            .edit_entry(0, |tone| tone.rename("Unsaved bundle"))
            .unwrap();
        let bytes_before = bundle.as_bytes().to_vec();
        let saved_before = bundle.saved.clone();
        assert!(bundle.is_dirty());
        assert!(
            bundle
                .set_mod_note_choice(0, ModNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(bundle.as_bytes(), bytes_before);
        assert_eq!(bundle.saved, saved_before);
        assert!(bundle.is_dirty());
    }

    #[test]
    fn delay_note_models_and_primary_fallback_slots_match_complete_expected_buffers() {
        let delay_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
        let delay_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let delay_sentinel = drive_record(0x3f, 0x007a_bcde, 0x7f, 0xdead_beef).to_vec();
        let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let mod_sentinel = drive_record(0x3f, 0x007a_bcde, 0, 0xface_cafe).to_vec();
        let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
        let expected_time = [
            0x3f80_0000,
            0x3f3f_5a81,
            0x3efd_6a06,
            0x3ea7_3807,
            0x3ebc_c486,
            0x3e78_3e11,
            0x3e22_0c12,
            0x3e37_9891,
            0x3dede625,
            0x3d97_b426,
            0x3dad_40a5,
            0x3d59_364f,
            0x3d03_0450,
        ];

        for slot in [DELAY_SLOT, 0x0005_0004] {
            for model_id in DELAY_CONTROL_MODEL_IDS {
                let models = vec![
                    (
                        slot,
                        model_id,
                        vec![
                            delay_time.clone(),
                            delay_note.clone(),
                            delay_sentinel.clone(),
                        ],
                    ),
                    (
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![mod_note.clone(), mod_sentinel.clone()],
                    ),
                    (0x0001_0000, 0x3e70_0000, vec![tempo.clone()]),
                ];
                let original =
                    delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
                for (choice_index, choice) in DelayNoteChoice::ALL.into_iter().enumerate().skip(1) {
                    let mut tone = PodTone::parse(original.clone()).unwrap();
                    let mut expected = original.clone();
                    patch_expected_parameter(
                        &tone,
                        &mut expected,
                        slot,
                        0x0020_0000,
                        choice.value(),
                    );
                    patch_expected_parameter(
                        &tone,
                        &mut expected,
                        slot,
                        0x0010_0000,
                        expected_time[choice_index - 1],
                    );
                    tone.set_delay_note_choice(choice).unwrap();
                    assert_eq!(
                        tone.as_bytes(),
                        expected,
                        "Delay model {model_id:08x}, slot {slot:08x}, choice {choice:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn delay_note_context_rejections_preserve_full_bytes_baseline_and_dirty_state() {
        let dtime = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
        let dnote = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let mnote = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
        let delay_rows = vec![dtime.clone(), dnote.clone()];
        let normal_mod = vec![(MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mnote.clone()])];
        let normal_system = vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone()])];
        let make_tone = |delay_slot: u32,
                         delay_model: u32,
                         delay_rows: Vec<Vec<u8>>,
                         mods: Vec<(u32, u32, Vec<Vec<u8>>)>,
                         systems: Vec<(u32, u32, Vec<Vec<u8>>)>,
                         target: u32| {
            let mut models = vec![(delay_slot, delay_model, delay_rows)];
            models.extend(mods);
            models.extend(systems);
            delay_context_fixture(&models, target, PODXT_LIVE_HOST_MIN_VERSION)
        };

        let mut cases = vec![
            (
                "missing Mod model",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "unsupported Mod model",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(MOD_SLOT, 0xdead_beef, vec![mnote.clone()])],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "duplicate Mod models",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![
                        normal_mod[0].clone(),
                        (0x0005_0003, MOD_CONTROL_MODEL_IDS[1], vec![mnote.clone()]),
                    ],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "opaque Mod PARM context",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![mnote.clone(), vec![1, 2, 3]],
                    )],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "duplicate Mod NOTE rows",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![mnote.clone(), mnote.clone()],
                    )],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "wrong-type Mod NOTE",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![drive_record(0x3f, 0x0020_0000, 1, 0).to_vec()],
                    )],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "noncanonical Mod NOTE",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![drive_record(0x3e, 0x0020_0000, 0, 0).to_vec()],
                    )],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "positive Mod NOTE blocks even NOTE Off",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    vec![(
                        MOD_SLOT,
                        MOD_CONTROL_MODEL_IDS[0],
                        vec![drive_record(0x3f, 0x0020_0000, 0, 6).to_vec()],
                    )],
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "missing System model",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "unsupported System model",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(0x0001_0000, 0xdead_beef, vec![tempo.clone()])],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "opaque System PARM context",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone(), vec![4, 5, 6]])],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "duplicate Tempo rows",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone(), tempo.clone()])],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "wrong-type Tempo",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(
                        0x0001_0000,
                        0x3e80_0000,
                        vec![drive_record(0x3f, 0x0020_0001, 0, 120).to_vec()],
                    )],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "noncanonical Tempo",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(
                        0x0001_0000,
                        0x3e80_0000,
                        vec![drive_record(0x3e, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()],
                    )],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "invalid Tempo range",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(
                        0x0001_0000,
                        0x3e80_0000,
                        vec![drive_record(0x3f, 0x0020_0001, 1, 29.0_f32.to_bits()).to_vec()],
                    )],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "nonfinite Tempo",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    vec![(
                        0x0001_0000,
                        0x3e80_0000,
                        vec![drive_record(0x3f, 0x0020_0001, 1, f32::NAN.to_bits()).to_vec()],
                    )],
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "duplicate Delay TIME rows",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    vec![dtime.clone(), dtime.clone(), dnote.clone()],
                    normal_mod.clone(),
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "missing Delay TIME row",
                make_tone(
                    DELAY_SLOT,
                    0x0202_0002,
                    vec![dnote.clone()],
                    normal_mod.clone(),
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
            (
                "unsupported Delay model",
                make_tone(
                    DELAY_SLOT,
                    0xdead_beef,
                    delay_rows.clone(),
                    normal_mod.clone(),
                    normal_system.clone(),
                    PODXT_LIVE_TONE_ID,
                ),
            ),
        ];
        cases.push(("old profile", {
            let mut bytes = make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            );
            let pinf = find_chunk_data(&bytes, b"PINF");
            bytes[pinf + 72..pinf + 76]
                .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
            bytes
        }));

        let duplicate_system = make_tone(
            DELAY_SLOT,
            0x0202_0002,
            delay_rows.clone(),
            normal_mod.clone(),
            vec![normal_system[0].clone(), normal_system[0].clone()],
            PODXT_LIVE_TONE_ID,
        );
        assert!(PodTone::parse(duplicate_system.clone()).is_err());
        assert!(
            Library::default()
                .import_bytes(duplicate_system.clone())
                .is_err()
        );
        let wrong_target = make_tone(
            DELAY_SLOT,
            0x0202_0002,
            delay_rows.clone(),
            normal_mod.clone(),
            normal_system.clone(),
            0x0007_0005,
        );
        assert!(PodTone::parse(wrong_target.clone()).is_err());
        assert!(
            Library::default()
                .import_bytes(wrong_target.clone())
                .is_err()
        );
        assert_eq!(
            wrong_target,
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                normal_system.clone(),
                0x0007_0005,
            )
        );
        for (label, original) in cases {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(
                tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                    .is_err(),
                "{label}"
            );
            assert_eq!(tone.as_bytes(), before, "{label} changed tone bytes");

            let mut library = Library::default();
            let index = library.import_bytes(original.clone()).unwrap();
            library.tones[index].tone.rename("Already dirty").unwrap();
            let dirty_before = library.tone(index).unwrap().as_bytes().to_vec();
            let saved_before = library.tones[index].saved.clone();
            assert!(library.is_dirty(index), "{label} dirty setup");
            assert!(
                library
                    .set_delay_note_choice(index, DelayNoteChoice::Quarter)
                    .is_err(),
                "{label}"
            );
            assert_eq!(
                library.tone(index).unwrap().as_bytes(),
                dirty_before,
                "{label} changed dirty bytes"
            );
            assert_eq!(
                library.tones[index].saved, saved_before,
                "{label} changed saved baseline"
            );
            assert!(library.is_dirty(index), "{label} cleared dirty state");
        }
    }

    #[test]
    fn delay_note_noop_validates_and_time_same_value_still_clears_positive_note() {
        let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e78_3e11).to_vec();
        let note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
        let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
        let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
        let models = vec![
            (DELAY_SLOT, 0x0202_0002, vec![time.clone(), note.clone()]),
            (MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mod_note]),
            (0x0001_0000, 0x3e70_0000, vec![tempo]),
        ];
        let original =
            delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_delay_note_choice(DelayNoteChoice::Quarter)
            .unwrap();
        assert_eq!(
            tone.as_bytes(),
            original,
            "same-value NOTE must validate then no-op"
        );

        let same_display_time = drive_record(0x3f, 0x0010_0000, 1, 0x3f00_0000).to_vec();
        let models = vec![(DELAY_SLOT, 0x0202_0002, vec![same_display_time, note])];
        let original =
            delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let mut expected = original;
        patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
        tone.set_delay_time_ms(1010.0).unwrap();
        assert_eq!(
            tone.as_bytes(),
            expected,
            "same TIME must still clear positive NOTE"
        );
    }

    #[test]
    fn delay_time_rejections_preserve_full_bytes_and_dirty_library_baseline() {
        let normal_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
        let normal_note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
        let cases = [
            (
                "duplicate TIME",
                vec![
                    normal_time.clone(),
                    normal_time.clone(),
                    normal_note.clone(),
                ],
                PODXT_LIVE_TONE_ID,
            ),
            (
                "missing TIME",
                vec![normal_note.clone()],
                PODXT_LIVE_TONE_ID,
            ),
            (
                "wrong-type TIME",
                vec![
                    drive_record(0x3f, 0x0010_0000, 0, 0x3e80_0000).to_vec(),
                    normal_note.clone(),
                ],
                PODXT_LIVE_TONE_ID,
            ),
            (
                "noncanonical TIME",
                vec![
                    drive_record(0x3e, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                    normal_note.clone(),
                ],
                PODXT_LIVE_TONE_ID,
            ),
            (
                "unknown NOTE",
                vec![
                    normal_time.clone(),
                    drive_record(0x3f, 0x0020_0000, 0, 14).to_vec(),
                ],
                PODXT_LIVE_TONE_ID,
            ),
        ];

        for (label, delay_rows, target) in cases {
            let models = vec![(DELAY_SLOT, 0x0202_0002, delay_rows)];
            let original = delay_context_fixture(&models, target, PODXT_LIVE_HOST_MIN_VERSION);
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_delay_time_ms(1200.0).is_err(), "{label}");
            assert_eq!(tone.as_bytes(), before, "{label} changed tone bytes");

            let mut library = Library::default();
            let index = library.import_bytes(original).unwrap();
            library.tones[index].tone.rename("Already dirty").unwrap();
            let dirty_before = library.tone(index).unwrap().as_bytes().to_vec();
            let saved_before = library.tones[index].saved.clone();
            assert!(library.set_delay_time_ms(index, 1200.0).is_err(), "{label}");
            assert_eq!(
                library.tone(index).unwrap().as_bytes(),
                dirty_before,
                "{label}"
            );
            assert_eq!(library.tones[index].saved, saved_before, "{label}");
            assert!(library.is_dirty(index), "{label}");
        }
    }

    #[test]
    fn delay_note_off_needs_no_time_or_tempo_and_preserves_time() {
        let note = drive_record(0x3f, 0x0020_0000, 0, 6);
        let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0);
        let models = [
            BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail: &[1, 2, 3, 4],
                parameters: &[&note],
            },
            BuiltModel {
                slot: MOD_SLOT,
                model_id: MOD_CONTROL_MODEL_IDS[0],
                tail: &[5, 6, 7, 8],
                parameters: &[&mod_note],
            },
        ];
        let original = custom_tone(ContainerKind::L6as, &models);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let mut expected = original;
        patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
        tone.set_delay_note_choice(DelayNoteChoice::Off).unwrap();
        assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), 0);
        assert_eq!(parm_bits(&tone, MOD_SLOT, 0x0020_0000), 0);
        assert_eq!(tone.as_bytes(), expected);
        assert!(
            tone.models()
                .iter()
                .find(|model| model.slot == DELAY_SLOT)
                .unwrap()
                .parameters
                .iter()
                .all(|p| !matches!(
                    p,
                    ModelParameter::Recorded {
                        id: 0x0010_0000,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn delay_time_ms_isolated_edit_clears_note_and_uses_double_source_conversion() {
        let note = drive_record(0x3f, 0x0020_0000, 0, 6);
        let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
        let models = [BuiltModel {
            slot: DELAY_SLOT,
            model_id: 0x0202_0002,
            tail: &[1, 2, 3, 4],
            parameters: &[&time, &note],
        }];
        let original = custom_tone(ContainerKind::L6as, &models);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let mut expected = original.clone();
        patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0010_0000, 0x3f00_0000);
        patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
        tone.set_delay_time_ms(1010.0).unwrap();
        assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0010_0000), 0x3f00_0000);
        assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), 0);
        assert_eq!(tone.as_bytes(), expected);
        let untouched = PodTone::parse(original).unwrap();
        assert_eq!(tone.models()[0].model_id, untouched.models()[0].model_id);
        assert!(tone.set_delay_time_ms(f32::NAN).is_err());
        assert!(tone.set_delay_time_ms(2000.5).is_err());
    }

    #[test]
    fn delay_note_and_time_edits_reject_bad_context_atomically() {
        let mut positive_mod = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            6,
            Some(120.0),
        ))
        .unwrap();
        let before = positive_mod.as_bytes().to_vec();
        assert!(
            positive_mod
                .set_delay_note_choice(DelayNoteChoice::Off)
                .is_err()
        );
        assert_eq!(positive_mod.as_bytes(), before);

        let mut missing_tempo = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            None,
        ))
        .unwrap();
        let before = missing_tempo.as_bytes().to_vec();
        assert!(
            missing_tempo
                .set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(missing_tempo.as_bytes(), before);

        let mut bad_time = PodTone::parse(delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x7f80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        ))
        .unwrap();
        let before = bad_time.as_bytes().to_vec();
        assert!(
            bad_time
                .set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(bad_time.as_bytes(), before);
        assert!(bad_time.set_delay_time_ms(1010.0).is_err());
        assert_eq!(bad_time.as_bytes(), before);
    }

    #[test]
    fn delay_note_rejects_ambiguous_duplicate_slots_and_rows() {
        let base = delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        );
        let parsed = PodTone::parse(base.clone()).unwrap();
        let delay = parsed
            .models()
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .unwrap();
        let mod_model = parsed
            .models()
            .iter()
            .find(|model| model.slot == MOD_SLOT)
            .unwrap();
        let d_time = match delay.parameters[0] {
            ModelParameter::Recorded {
                prefix,
                id,
                value: ParameterValue::Float { bits },
            } => drive_record(prefix, id, 1, bits),
            _ => unreachable!(),
        };
        let d_note = match delay.parameters[1] {
            ModelParameter::Recorded {
                prefix,
                id,
                value: ParameterValue::Int { bits },
            } => drive_record(prefix, id, 0, bits),
            _ => unreachable!(),
        };
        let d_sentinel = match delay.parameters[2] {
            ModelParameter::Recorded {
                prefix,
                id,
                value: ParameterValue::Other { type_word, bits },
            } => drive_record(prefix, id, type_word, bits),
            _ => unreachable!(),
        };
        let m_speed = match mod_model.parameters[0] {
            ModelParameter::Recorded {
                prefix,
                id,
                value: ParameterValue::Float { bits },
            } => drive_record(prefix, id, 1, bits),
            _ => unreachable!(),
        };
        let m_note = match mod_model.parameters[1] {
            ModelParameter::Recorded {
                prefix,
                id,
                value: ParameterValue::Int { bits },
            } => drive_record(prefix, id, 0, bits),
            _ => unreachable!(),
        };
        let duplicate_delay_rows = [&d_time[..], &d_note[..], &d_note[..], &d_sentinel[..]];
        let normal_delay_rows = [&d_time[..], &d_note[..]];
        let mod_rows = [&m_speed[..], &m_note[..]];
        let tempo_row = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits());
        let tempo_rows = [&tempo_row[..]];
        let dup_delay = [
            BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail: &[1, 2, 3, 4],
                parameters: &duplicate_delay_rows,
            },
            BuiltModel {
                slot: MOD_SLOT,
                model_id: MOD_CONTROL_MODEL_IDS[0],
                tail: &[5, 6, 7, 8],
                parameters: &mod_rows,
            },
            BuiltModel {
                slot: 0x0001_0000,
                model_id: 0x3e70_0000,
                tail: &[9, 10, 11, 12],
                parameters: &tempo_rows,
            },
        ];
        let mut tone = PodTone::parse(custom_tone(ContainerKind::L6as, &dup_delay)).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), before);

        let duplicate_slot_models = vec![
            BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail: &[1, 2, 3, 4],
                parameters: &normal_delay_rows,
            },
            BuiltModel {
                slot: 0x0005_0004,
                model_id: 0x0202_0002,
                tail: &[1, 2, 3, 4],
                parameters: &normal_delay_rows,
            },
            BuiltModel {
                slot: MOD_SLOT,
                model_id: MOD_CONTROL_MODEL_IDS[0],
                tail: &[5, 6, 7, 8],
                parameters: &mod_rows,
            },
            BuiltModel {
                slot: 0x0001_0000,
                model_id: 0x3e70_0000,
                tail: &[9, 10, 11, 12],
                parameters: &tempo_rows,
            },
        ];
        let mut tone =
            PodTone::parse(custom_tone(ContainerKind::L6as, &duplicate_slot_models)).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), before);
    }

    #[test]
    fn delay_note_rejects_noncanonical_prefix_type_unknown_values_and_profiles() {
        let original = delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        );
        let mut bad_prefix = original.clone();
        let parsed = PodTone::parse(original.clone()).unwrap();
        let prefix_at = parameter_word0_offset(&parsed, DELAY_SLOT, 0x0020_0000);
        bad_prefix[prefix_at] = 0x3e;
        let mut tone = PodTone::parse(bad_prefix).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), before);

        let mut wrong_type = original.clone();
        wrong_type[prefix_at + 4..prefix_at + 8].copy_from_slice(&1u32.to_be_bytes());
        let mut tone = PodTone::parse(wrong_type).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), before);

        let mut unknown_note = original.clone();
        unknown_note[prefix_at + 8..prefix_at + 12].copy_from_slice(&14u32.to_be_bytes());
        let mut tone = PodTone::parse(unknown_note).unwrap();
        assert_eq!(
            tone.delay_note_value().unwrap(),
            DelayNoteValue::Unknown { raw_bits: 14 }
        );
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_note_choice(DelayNoteChoice::Off).is_err());
        assert_eq!(tone.as_bytes(), before);

        let mut old_profile = original.clone();
        let pinf = find_chunk_data(&old_profile, b"PINF");
        old_profile[pinf + 72..pinf + 76]
            .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        let mut tone = PodTone::parse(old_profile).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_time_ms(1010.0).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    #[test]
    fn delay_note_library_and_bundle_paths_revert_save_and_reopen() {
        let original = delay_note_fixture(
            DELAY_SLOT,
            0x0202_0002,
            0,
            0x3e80_0000,
            Some(MOD_SLOT),
            MOD_CONTROL_MODEL_IDS[0],
            0,
            Some(120.0),
        );
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        library
            .set_delay_note_choice(index, DelayNoteChoice::Quarter)
            .unwrap();
        assert_eq!(
            parm_bits(library.tone(index).unwrap(), DELAY_SLOT, 0x0020_0000),
            6
        );
        assert!(library.is_dirty(index));
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        library.set_delay_time_ms(index, 1010.0).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("delay-note.l6c");
        library.tone(index).unwrap().save_copy(&path).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
        assert!(library.tone(index).unwrap().save_copy(&path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), saved);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let before = bundle.as_bytes().to_vec();
        bundle
            .set_delay_note_choice(0, DelayNoteChoice::Quarter)
            .unwrap();
        assert!(bundle.is_dirty());
        let (bank, preset) = bundle.entry_location(0).unwrap();
        assert_eq!(
            parm_bits(&bundle.tone(bank, preset).unwrap(), DELAY_SLOT, 0x0020_0000),
            6
        );
        assert_eq!(bundle.as_bytes().len(), before.len());
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), before);
    }

    #[test]
    fn delay_model_id_selection_is_closed_exact_and_lossless() {
        for current in DELAY_CONTROL_MODEL_IDS {
            let original = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: DELAY_SLOT,
                    model_id: current,
                    tail: &[0x7f, 0xa5, 0x00, 0xfe, 0x55],
                    parameters: &[&[0x11, 0x22, 0x33]],
                }],
            );
            for requested in PodDelayModelChoice::MODEL_IDS {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let before = tone.as_bytes().to_vec();
                tone.set_delay_model_choice(PodDelayModelChoice::try_from(requested).unwrap())
                    .unwrap();
                let differences: Vec<_> = before
                    .iter()
                    .zip(tone.as_bytes())
                    .enumerate()
                    .filter_map(|(i, (a, b))| (a != b).then_some(i))
                    .collect();
                if current == requested {
                    assert!(differences.is_empty());
                } else {
                    let reparsed = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                    assert_eq!(reparsed.models()[0].model_id, requested);
                    let minf_word =
                        tone.model_minfo_ranges[0].start..tone.model_minfo_ranges[0].start + 4;
                    assert!(differences.iter().all(|offset| minf_word.contains(offset)));
                    assert_eq!(&tone.as_bytes()[minf_word], &requested.to_be_bytes());
                }
            }
        }
    }

    #[test]
    fn mod_model_id_selection_covers_current_and_requested_ids_losslessly() {
        for current in MOD_CONTROL_MODEL_IDS {
            let original = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: MOD_SLOT,
                    model_id: current,
                    // Preserve noncanonical enable/reserved bytes, opaque tail,
                    // parameters, and unrelated children through the edit.
                    tail: &[0x7f, 0xa5, 0x00, 0xfe, 0x55],
                    parameters: &[&[0x11, 0x22, 0x33]],
                }],
            );
            for requested in PodModModelChoice::MODEL_IDS {
                let choice = PodModModelChoice::try_from(requested).unwrap();
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let before = tone.as_bytes().to_vec();
                let minf_start = tone.model_minfo_ranges[0].start;
                let mut expected = before.clone();
                expected[minf_start..minf_start + 4].copy_from_slice(&requested.to_be_bytes());

                tone.set_mod_model_choice(choice).unwrap();

                assert_eq!(tone.as_bytes(), expected);
                let reparsed = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                assert_eq!(reparsed.models()[0].model_id, requested);
                assert_eq!(reparsed.models()[0].slot, MOD_SLOT);
            }
        }
    }

    #[test]
    fn stomp_model_choice_partition_and_full_buffer_matrix_are_exact() {
        assert_eq!(STOMP_CONTROL_MODEL_IDS.len(), 31);
        assert_eq!(STOMP_MODEL_CHOICE_IDS.len(), 14);
        for id in STOMP_MODEL_CHOICE_IDS {
            assert!(STOMP_CONTROL_MODEL_IDS.contains(&id));
        }
        let gated = [
            0x0205_0006,
            0x0205_0001,
            0x0200_0006,
            0x0205_0007,
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
        ];
        assert_eq!(gated.len(), 17);
        for id in gated {
            assert!(STOMP_CONTROL_MODEL_IDS.contains(&id));
            assert!(!STOMP_MODEL_CHOICE_IDS.contains(&id));
            assert!(PodStompModelChoice::try_from(id).is_err());
        }
        for (index, left) in STOMP_CONTROL_MODEL_IDS.iter().enumerate() {
            for right in STOMP_CONTROL_MODEL_IDS.iter().skip(index + 1) {
                assert_ne!(left, right);
            }
        }

        for current in STOMP_CONTROL_MODEL_IDS {
            let original = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: current,
                    tail: &[0x80, 0xa5, 0x00, 0xfe, 0x55],
                    parameters: &[&[0xde, 0xad, 0xbe]],
                }],
            );
            for requested in STOMP_MODEL_CHOICE_IDS {
                let choice = PodStompModelChoice::try_from(requested).unwrap();
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let before = tone.as_bytes().to_vec();
                let minf = tone.model_minfo_ranges[0].start;
                let mut expected = before.clone();
                if current != requested {
                    expected[minf..minf + 4].copy_from_slice(&requested.to_be_bytes());
                }
                tone.set_stomp_model_choice(choice).unwrap();
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "{current:08x} -> {requested:08x}"
                );
                let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                assert_eq!(reopened.stomp_model_id(), Some(requested));
                assert_eq!(reopened.models()[0].slot, STOMP_SLOT);
                assert_eq!(reopened.models()[0].ordinal, Some(-128));
                assert_eq!(reopened.models()[0].reserved, Some([0xa5, 0x00]));
                assert_eq!(reopened.models()[0].enabled, Some(false));
            }
        }
    }

    #[test]
    fn stomp_model_selection_rejects_bad_structure_and_context_atomically() {
        let choice = PodStompModelChoice::try_from(0x0205_0002).unwrap();
        for (kind, slot, current) in [
            (ContainerKind::L6as, STOMP_SLOT, 0x0205_0000),
            (ContainerKind::L6pa, MOD_SLOT, 0x0205_0000),
            (ContainerKind::L6pa, STOMP_SLOT, 0xdead_beef),
        ] {
            let mut tone = PodTone::parse(custom_tone(
                kind,
                &[BuiltModel {
                    slot,
                    model_id: current,
                    tail: &[1, 2, 3, 4],
                    parameters: &[],
                }],
            ))
            .unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_stomp_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }

        for tail in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 1][..]] {
            let mut tone = PodTone::parse(custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x0205_0000,
                    tail,
                    parameters: &[],
                }],
            ))
            .unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_stomp_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }

        for models in [
            vec![],
            vec![
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x0205_0000,
                    tail: &[0; 4],
                    parameters: &[],
                },
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x0205_0002,
                    tail: &[0; 4],
                    parameters: &[],
                },
            ],
        ] {
            let bytes = custom_tone(ContainerKind::L6pa, &models);
            if models.len() == 2 {
                assert!(PodTone::parse(bytes.clone()).is_err());
                continue;
            }
            let mut tone = PodTone::parse(bytes).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_stomp_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }

        let mut wrong_target = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0205_0000,
                tail: &[0; 4],
                parameters: &[],
            }],
        );
        // PINF target resides at the fixture's standard target word offset.
        let target = wrong_target
            .windows(8)
            .position(|window| window == b"\0\0\0\x01\0\x03\0\x0a")
            .unwrap()
            + 4;
        wrong_target[target + 3] ^= 1;
        assert!(PodTone::parse(wrong_target.clone()).is_err());
        assert_eq!(wrong_target[target + 3], 0x0b);

        let old_profile = fixture(
            ContainerKind::L6pa,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            &[(STOMP_SLOT, 0x0205_0000)],
        );
        let mut tone = PodTone::parse(old_profile.clone()).unwrap();
        assert!(tone.set_stomp_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), old_profile);

        fn alter_stomp_minfo(mut bytes: Vec<u8>, duplicate: bool) -> Vec<u8> {
            let root = preset::chunks(&bytes, 12..bytes.len()).unwrap();
            let patch = root
                .iter()
                .find(|chunk| {
                    chunk.id == *b"LIST"
                        && &bytes[chunk.data.start..chunk.data.start + 4] == b"PATC"
                })
                .unwrap()
                .clone();
            let models = preset::chunks(&bytes, patch.data.start + 4..patch.data.end).unwrap();
            let model = models
                .iter()
                .find(|chunk| {
                    chunk.id == *b"LIST"
                        && &bytes[chunk.data.start..chunk.data.start + 4] == b"MODL"
                })
                .unwrap()
                .clone();
            let fields = preset::chunks(&bytes, model.data.start + 4..model.data.end).unwrap();
            let minf = fields
                .iter()
                .find(|chunk| chunk.id == *b"MINF")
                .unwrap()
                .clone();
            let start = minf.data.start - 8;
            let end = minf.data.end + minf.data.len() % 2;
            let payload = bytes[start..end].to_vec();
            if duplicate {
                bytes.splice(end..end, payload.iter().copied());
            } else {
                bytes.drain(start..end);
            }
            let delta = if duplicate {
                payload.len() as i32
            } else {
                -(payload.len() as i32)
            };
            for chunk_start in [model.data.start - 8, patch.data.start - 8] {
                let length = preset::word(&bytes, chunk_start + 4) as i32;
                bytes[chunk_start + 4..chunk_start + 8]
                    .copy_from_slice(&(length + delta).to_be_bytes());
            }
            let root_length = preset::word(&bytes, 4) as i32;
            bytes[4..8].copy_from_slice(&(root_length + delta).to_be_bytes());
            bytes
        }
        for duplicate in [false, true] {
            let malformed = alter_stomp_minfo(
                custom_tone(
                    ContainerKind::L6pa,
                    &[BuiltModel {
                        slot: STOMP_SLOT,
                        model_id: 0x0205_0000,
                        tail: &[0; 4],
                        parameters: &[],
                    }],
                ),
                duplicate,
            );
            assert!(PodTone::parse(malformed).is_err());
        }
    }

    #[test]
    fn stomp_model_choice_matches_independent_whole_file_fixture_and_reopens() {
        fn vector(text: &str, marker: &str, stop: &str) -> Vec<u8> {
            let section = text.split_once(marker).unwrap().1;
            section
                .lines()
                .take_while(|line| !line.starts_with(stop))
                .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
                .flat_map(str::split_whitespace)
                .map(|byte| u8::from_str_radix(byte, 16).unwrap())
                .collect()
        }
        let contract = include_str!("../docs/pod-model-selection-contract.md")
            .split_once("## Stomp model selector: admitted offline ID-only operation")
            .unwrap()
            .1;
        let before = vector(
            contract,
            "before (178 bytes; FORM length 0x000000aa):",
            "after (178 bytes; FORM length 0x000000aa):",
        );
        let expected = vector(
            contract,
            "after (178 bytes; FORM length 0x000000aa):",
            "```",
        );
        assert_eq!(before.len(), 178);
        assert_eq!(expected.len(), 178);
        let mut tone = PodTone::parse(before).unwrap();
        tone.set_stomp_model_choice(PodStompModelChoice::try_from(0x0205_0002).unwrap())
            .unwrap();
        assert_eq!(tone.as_bytes(), expected);
        let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
        assert_eq!(reopened.stomp_model_id(), Some(0x0205_0002));
    }

    #[test]
    fn stomp_model_library_and_bundle_apply_track_dirty_revert_and_save_collision() {
        let original = fixture(
            ContainerKind::L6pa,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[(STOMP_SLOT, 0x020a_000a)],
        );
        let choice = PodStompModelChoice::try_from(0x0205_0000).unwrap();
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        library.set_stomp_model_choice(index, choice).unwrap();
        assert!(library.is_dirty(index));
        assert_eq!(
            library.tone(index).unwrap().stomp_model_id(),
            Some(choice.model_id())
        );
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join("stomp-choice.l6t");
        library.tone(index).unwrap().save_copy(&copy).unwrap();
        let saved = std::fs::read(&copy).unwrap();
        assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
        assert!(library.tone(index).unwrap().save_copy(&copy).is_err());
        assert_eq!(std::fs::read(&copy).unwrap(), saved);
        library.revert_unsaved(index).unwrap();
        assert!(!library.is_dirty(index));
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let before = bundle.as_bytes().to_vec();
        bundle.set_stomp_model_choice(0, choice).unwrap();
        assert!(bundle.is_dirty());
        let (bank, preset) = bundle.entry_location(0).unwrap();
        assert_eq!(
            bundle.tone(bank, preset).unwrap().stomp_model_id(),
            Some(choice.model_id())
        );
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), before);
        assert!(!bundle.is_dirty());
    }

    #[test]
    fn mod_model_edit_matches_independent_documented_full_buffer_vector() {
        fn vector(text: &str, marker: &str, stop: &str) -> Vec<u8> {
            let section = text.split_once(marker).unwrap().1;
            section
                .lines()
                .take_while(|line| !line.trim().starts_with("```") && !line.starts_with(stop))
                .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
                .flat_map(str::split_whitespace)
                .map(|byte| u8::from_str_radix(byte, 16).unwrap())
                .collect()
        }
        let contract = include_str!("../docs/pod-model-selection-contract.md")
            .split_once("## Mod model selector")
            .unwrap()
            .1;
        let before = vector(
            contract,
            "before (178 bytes; FORM length 0x000000aa):",
            "after (178 bytes; FORM length 0x000000aa):",
        );
        let expected = vector(
            contract,
            "after (178 bytes; FORM length 0x000000aa):",
            "```",
        );
        assert_eq!(before.len(), 178);
        assert_eq!(expected.len(), 178);
        let mut tone = PodTone::parse(before).unwrap();
        tone.set_mod_model_choice(PodModModelChoice::try_from(0x0203_0005).unwrap())
            .unwrap();
        assert_eq!(tone.as_bytes(), expected);
    }

    #[test]
    fn mod_model_id_selection_rejects_gated_requests_and_invalid_context_atomically() {
        let make = |slot, id| {
            custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot,
                    model_id: id,
                    tail: &[0x7f, 0xa5, 0x00, 0xfe],
                    parameters: &[],
                }],
            )
        };
        let choice = PodModModelChoice::try_from(0x0203_0005).unwrap();
        for gated in [
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
        ] {
            assert!(PodModModelChoice::try_from(gated).is_err());
        }

        let fallback = make(0x0005_0003, 0x0203_0015);
        let mut tone = PodTone::parse(fallback).unwrap();
        tone.set_mod_model_choice(choice).unwrap();
        assert_eq!(tone.models()[0].model_id, choice.model_id());
        assert_eq!(tone.models()[0].slot, 0x0005_0003);

        for (kind, slot, id) in [
            (ContainerKind::L6as, MOD_SLOT, 0x0203_0004),
            (ContainerKind::L6pa, MOD_SLOT, 0x3333_0001),
            (ContainerKind::L6pa, 0x0009_0001, 0x0203_0004),
        ] {
            let mut tone = PodTone::parse(custom_tone(
                kind,
                &[BuiltModel {
                    slot,
                    model_id: id,
                    tail: &[1, 2, 3, 4],
                    parameters: &[],
                }],
            ))
            .unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_mod_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }

        let both = custom_tone(
            ContainerKind::L6pa,
            &[
                BuiltModel {
                    slot: MOD_SLOT,
                    model_id: 0x0203_0004,
                    tail: &[1, 2, 3, 4],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 0x0005_0003,
                    model_id: 0x0203_0005,
                    tail: &[5, 6, 7, 8],
                    parameters: &[],
                },
            ],
        );
        let mut tone = PodTone::parse(both).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);

        let unknown = make(MOD_SLOT, 0x3333_0001);
        let mut tone = PodTone::parse(unknown.clone()).unwrap();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), unknown);

        for (tail, body_len) in [
            (&[][..], 8),
            (&[0][..], 9),
            (&[0, 0][..], 10),
            (&[0, 0, 1][..], 11),
        ] {
            let short = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: MOD_SLOT,
                    model_id: 0x0203_0004,
                    tail,
                    parameters: &[],
                }],
            );
            let mut tone = PodTone::parse(short).unwrap();
            assert_eq!(tone.model_minfo_ranges[0].len(), body_len);
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_mod_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }

        for duplicate in [false, true] {
            let valid = make(MOD_SLOT, 0x0203_0004);
            let malformed = alter_first_model_minf(&valid, duplicate);
            assert!(PodTone::parse(malformed).is_err());
        }

        let mut old_profile = make(MOD_SLOT, 0x0203_0004);
        let pinf = find_chunk_data(&old_profile, b"PINF");
        old_profile[pinf + 72..pinf + 76].copy_from_slice(&0x02ff_ffffu32.to_be_bytes());
        let mut tone = PodTone::parse(old_profile).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);

        let mut wrong_target = make(MOD_SLOT, 0x0203_0004);
        let pinf = find_chunk_data(&wrong_target, b"PINF");
        wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000bu32.to_be_bytes());
        let before = wrong_target.clone();
        assert!(PodTone::parse(wrong_target.clone()).is_err());
        assert_eq!(wrong_target, before);

        let mut wrong_layout = make(MOD_SLOT, 0x0203_0004);
        let pinf = find_chunk_data(&wrong_layout, b"PINF");
        wrong_layout[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
        assert!(PodTone::parse(wrong_layout.clone()).is_err());
        assert_eq!(wrong_layout[pinf..pinf + 4], 2u32.to_be_bytes());

        let duplicate_slot = custom_tone(
            ContainerKind::L6pa,
            &[
                BuiltModel {
                    slot: MOD_SLOT,
                    model_id: 0x0203_0004,
                    tail: &[1, 2, 3, 4],
                    parameters: &[],
                },
                BuiltModel {
                    slot: MOD_SLOT,
                    model_id: 0x0203_0005,
                    tail: &[5, 6, 7, 8],
                    parameters: &[],
                },
            ],
        );
        assert!(PodTone::parse(duplicate_slot.clone()).is_err());
        assert_eq!(duplicate_slot[8..12], *b"L6PA");

        let same_id_wrong_context = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: choice.model_id(),
                tail: &[1, 2, 3, 4],
                parameters: &[],
            }],
        );
        let mut tone = PodTone::parse(same_id_wrong_context).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    #[test]
    fn mod_model_id_selection_library_bundle_and_saved_baseline_are_isolated() {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0203_0004,
                tail: &[0x7f, 0xa5, 0x00, 0xfe],
                parameters: &[&[0x11, 0x22, 0x33]],
            }],
        );
        let choice = PodModModelChoice::try_from(0x0203_0005).unwrap();

        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        let baseline = library.tone(index).unwrap().as_bytes().to_vec();
        library.set_mod_model_choice(index, choice).unwrap();
        assert!(library.is_dirty(index));
        assert_ne!(library.tone(index).unwrap().as_bytes(), baseline);
        let directory = tempfile::tempdir().unwrap();
        let collision = directory.path().join("existing-mod.l6p");
        std::fs::write(&collision, b"keep existing bytes").unwrap();
        assert!(library.tone(index).unwrap().save_copy(&collision).is_err());
        assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");
        let saved = directory.path().join("edited-mod.l6p");
        library.tone(index).unwrap().save_copy(&saved).unwrap();
        let reopened = PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
        assert_eq!(reopened.models()[0].model_id, choice.model_id());
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);
        assert!(!library.is_dirty(index));

        let mut dirty_library = Library::default();
        let dirty_index = dirty_library.import_bytes(original.clone()).unwrap();
        let unknown_current = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x3333_0001,
                tail: &[0x7f, 0xa5, 0x00, 0xfe],
                parameters: &[],
            }],
        );
        dirty_library
            .replace(dirty_index, unknown_current.clone())
            .unwrap();
        let dirty_before_failure = dirty_library.tone(dirty_index).unwrap().as_bytes().to_vec();
        assert!(
            dirty_library
                .set_mod_model_choice(dirty_index, choice)
                .is_err()
        );
        assert_eq!(
            dirty_library.tone(dirty_index).unwrap().as_bytes(),
            dirty_before_failure
        );
        assert!(dirty_library.is_dirty(dirty_index));
        dirty_library.revert_unsaved(dirty_index).unwrap();
        assert_eq!(
            dirty_library.tone(dirty_index).unwrap().as_bytes(),
            original
        );

        let mut dirty_bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let bundle_baseline = dirty_bundle.as_bytes().to_vec();
        dirty_bundle
            .replace_entry(0, unknown_current.clone())
            .unwrap();
        let dirty_bundle_bytes = dirty_bundle.as_bytes().to_vec();
        assert!(dirty_bundle.set_mod_model_choice(0, choice).is_err());
        assert_eq!(dirty_bundle.as_bytes(), dirty_bundle_bytes);
        assert!(dirty_bundle.is_dirty());
        dirty_bundle.revert().unwrap();
        assert_eq!(dirty_bundle.as_bytes(), bundle_baseline);
        assert!(!dirty_bundle.is_dirty());

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let bundle_before = bundle.as_bytes().to_vec();
        bundle.set_mod_model_choice(0, choice).unwrap();
        assert!(bundle.is_dirty());
        let (bank, preset) = bundle.entry_location(0).unwrap();
        assert_eq!(
            bundle.tone(bank, preset).unwrap().models()[0].model_id,
            choice.model_id()
        );
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), bundle_before);
        assert!(!bundle.is_dirty());
    }

    #[test]
    fn delay_model_edit_matches_independent_documented_188_byte_vector() {
        fn vector(text: &str, marker: &str, end_marker: &str) -> Vec<u8> {
            let section = text.split_once(marker).unwrap().1;
            section
                .lines()
                .take_while(|line| !line.trim().starts_with("```") && !line.starts_with(end_marker))
                .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
                .flat_map(str::split_whitespace)
                .map(|byte| u8::from_str_radix(byte, 16).unwrap())
                .collect()
        }
        let contract = include_str!("../docs/pod-model-selection-contract.md");
        let before = vector(contract, "before (188 bytes):", "after (188 bytes):");
        let expected = vector(contract, "after (188 bytes):", "```");
        assert_eq!(before.len(), 188);
        assert_eq!(expected.len(), 188);
        let mut tone = PodTone::parse(before).unwrap();
        tone.set_delay_model_choice(PodDelayModelChoice::try_from(0x0202_0003).unwrap())
            .unwrap();
        assert_eq!(tone.as_bytes(), expected);
    }

    #[test]
    fn delay_model_id_selection_supports_fallback_and_rejects_invalid_context_atomically() {
        let make = |kind, slot, id| {
            custom_tone(
                kind,
                &[BuiltModel {
                    slot,
                    model_id: id,
                    tail: &[1, 0x99, 0x88, 0xfe],
                    parameters: &[],
                }],
            )
        };
        let choice = PodDelayModelChoice::try_from(0x0202_0003).unwrap();
        let mut fallback =
            PodTone::parse(make(ContainerKind::L6pa, 0x0005_0004, 0x0202_000f)).unwrap();
        fallback.set_delay_model_choice(choice).unwrap();
        assert_eq!(fallback.models()[0].model_id, choice.model_id());
        for (kind, slot, id, version) in [
            (
                ContainerKind::L6as,
                DELAY_SLOT,
                0x0202_0002,
                PODXT_LIVE_HOST_MIN_VERSION,
            ),
            (
                ContainerKind::L6pa,
                DELAY_SLOT,
                0x3333_0001,
                PODXT_LIVE_HOST_MIN_VERSION,
            ),
            (
                ContainerKind::L6pa,
                0x0009_0001,
                0x0202_0002,
                PODXT_LIVE_HOST_MIN_VERSION,
            ),
        ] {
            let mut tone = PodTone::parse(make(kind, slot, id)).unwrap();
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_delay_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
            let _ = version;
        }
        let both = custom_tone(
            ContainerKind::L6pa,
            &[
                BuiltModel {
                    slot: DELAY_SLOT,
                    model_id: 0x0202_0002,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 0x0005_0004,
                    model_id: 0x0202_0003,
                    tail: &[0, 0, 0, 1],
                    parameters: &[],
                },
            ],
        );
        let mut tone = PodTone::parse(both).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);

        let mut old_profile = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
        let pinf = find_chunk_data(&old_profile, b"PINF");
        old_profile[pinf + 72..pinf + 76].copy_from_slice(&0x02ff_ffffu32.to_be_bytes());
        let mut tone = PodTone::parse(old_profile).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
        for gated in 0x0202_000b..=0x0202_000f {
            assert!(PodDelayModelChoice::try_from(gated).is_err());
        }
        for (tail, body_len) in [
            (&[][..], 8),
            (&[0][..], 9),
            (&[0, 0][..], 10),
            (&[0, 0, 1][..], 11),
        ] {
            let short = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: DELAY_SLOT,
                    model_id: 0x0202_0002,
                    tail,
                    parameters: &[],
                }],
            );
            let mut tone = PodTone::parse(short).unwrap();
            assert_eq!(tone.model_minfo_ranges[0].len(), body_len);
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_delay_model_choice(choice).is_err());
            assert_eq!(tone.as_bytes(), before);
        }
        for duplicate in [false, true] {
            let valid = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
            let malformed = alter_first_model_minf(&valid, duplicate);
            let untouched = malformed.clone();
            assert!(PodTone::parse(malformed.clone()).is_err());
            assert_eq!(malformed, untouched);
        }
        for (offset, value) in [(4, 2u32), (8, 0x0003_000b)] {
            let mut invalid = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
            if offset == 4 {
                let pinf = find_chunk_data(&invalid, b"PINF");
                invalid[pinf..pinf + 4].copy_from_slice(&value.to_be_bytes());
            } else {
                let pinf = find_chunk_data(&invalid, b"PINF");
                invalid[pinf + 4..pinf + 8].copy_from_slice(&value.to_be_bytes());
            }
            let untouched = invalid.clone();
            assert!(PodTone::parse(invalid.clone()).is_err());
            assert_eq!(invalid, untouched);
        }
        let same_id_l6as = make(ContainerKind::L6as, DELAY_SLOT, 0x0202_0003);
        let mut tone = PodTone::parse(same_id_l6as).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);

        let unknown_current = make(ContainerKind::L6pa, DELAY_SLOT, 0x3333_0001);
        let mut library = Library::default();
        let index = library.import_bytes(unknown_current.clone()).unwrap();
        assert!(!library.is_dirty(index));
        assert!(library.set_delay_model_choice(index, choice).is_err());
        assert_eq!(library.tone(index).unwrap().as_bytes(), unknown_current);
        assert!(!library.is_dirty(index));
        let mut changed = PodTone::parse(unknown_current.clone()).unwrap();
        changed.rename("dirty baseline").unwrap();
        library.replace(index, changed.into_bytes()).unwrap();
        let before_failure = library.tone(index).unwrap().as_bytes().to_vec();
        assert!(library.set_delay_model_choice(index, choice).is_err());
        assert_eq!(library.tone(index).unwrap().as_bytes(), before_failure);
        assert!(library.is_dirty(index));
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), unknown_current);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&unknown_current)).unwrap();
        bundle
            .edit_entry(0, |tone| tone.rename("dirty bundle baseline"))
            .unwrap();
        let before_failure = bundle.as_bytes().to_vec();
        assert!(bundle.set_delay_model_choice(0, choice).is_err());
        assert_eq!(bundle.as_bytes(), before_failure);
        assert!(bundle.is_dirty());
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), bundle_with_tone(&unknown_current));
    }

    #[test]
    fn delay_model_choice_flows_through_library_and_transactional_bundle() {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail: &[0, 0xa3, 0x91, 0xff],
                parameters: &[&[0x1, 0x2, 0x3]],
            }],
        );
        let choice = PodDelayModelChoice::try_from(0x0202_000a).unwrap();
        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        library.set_delay_model_choice(index, choice).unwrap();
        assert!(library.is_dirty(index));
        assert_eq!(
            library.tone(index).unwrap().models()[0].model_id,
            choice.model_id()
        );
        library.revert_unsaved(index).unwrap();
        assert_eq!(library.tone(index).unwrap().as_bytes(), original);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
        let before = bundle.as_bytes().to_vec();
        bundle.set_delay_model_choice(0, choice).unwrap();
        assert!(bundle.is_dirty());
        let (bank, preset) = bundle.entry_location(0).unwrap();
        assert_eq!(
            bundle.tone(bank, preset).unwrap().models()[0].model_id,
            choice.model_id()
        );
        bundle.revert().unwrap();
        assert_eq!(bundle.as_bytes(), before);
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

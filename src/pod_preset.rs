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
//! stored. A 12-byte PARM carries an id, a value type, and raw bits. No
//! parameter range was recovered, and this module does not edit those values.
//! Other bytes are preserved.
//!
//! The MIDI hardware id `L6HW_ID_PODXTLIVE` (18000) is a different namespace.
//! The archived firmware version `0x03010000` is not a requirement of a tone file.

use crate::document;
use crate::pod_assets::{self, SlotRole};
use crate::preset;
use std::io::{self, Read};
use std::path::Path;

/// PINF target for PODxt Live. Equal to `L6ProductConstants.PODXT_LIVE`.
pub const PODXT_LIVE_TONE_ID: u32 = 0x0003_000a;
/// Amp model slot from `L6Patch.UpdateMetaAmpName` and `kGPXTSlot_Amp`.
pub const AMP_SLOT: u32 = 0x0003_0000;
/// Cab model slot from `L6Patch.UpdateMetaNotesForSetup` and `kGPXTSlot_Cab`.
/// Only an L6AS tone exposes it as the cab model.
pub const CAB_SLOT: u32 = 0x0003_0001;
/// Application bound for the offline librarian. Not a recovered Line 6 limit.
pub const TONE_BYTES_CEILING: u64 = 1_048_576;

/// Static Edit 2.27 file framing. The tag is an HD500x editor-group
/// observation; it does not prove a physical target or profile compatibility.
pub const H5EP_HEADER_BYTES: usize = 0x28;
pub const H5EP_BODY_BYTES: usize = 0x1000;
pub const H5EP_MIN_BYTES: usize = H5EP_HEADER_BYTES + H5EP_BODY_BYTES;
pub const H5ES_HEADER_BYTES: usize = 0x38;
pub const H5ES_PRESET_COUNT: usize = 64;
pub const H5ES_MIN_BYTES: usize = H5ES_HEADER_BYTES + H5ES_PRESET_COUNT * H5EP_MIN_BYTES;
pub const H5EB_HEADER_BYTES: usize = 0x28;
pub const H5EB_SETLIST_COUNT: usize = 8;
pub const H5EB_MIN_BYTES: usize = H5EB_HEADER_BYTES + H5EB_SETLIST_COUNT * H5ES_MIN_BYTES;
const HD_PRESET_BYTES_CEILING: usize = 1_048_576;
const HD_SETLIST_BYTES_CEILING: usize = 1_048_576;
pub const HD_BUNDLE_BYTES_CEILING: usize = 3 * 1024 * 1024;
const HD_EFFECT_BLOCK_OFFSET: usize = 0x428;
const HD_EFFECT_BLOCK_STRIDE: usize = 0x100;
const HD_EFFECT_COUNT: usize = 8;
const HD_EFFECT_ROWS_OFFSET: usize = 0x10;
const HD_PARAMETER_ROW_STRIDE: usize = 0x14;
const HD_PARAMETER_ROW_FLAG_OFFSET: usize = 0x10;
const HD_PARAMETER_ROW_COUNT: usize = 12;
const HD_EMPTY_EFFECT_MODEL_ID: u32 = 0x0200_0000;
const HD_SCREAMER_MODEL_ID: u32 = 0x0205_000e;
const HD_SUB_OCTAVE_FUZZ_MODEL_ID: u32 = 0x0205_0014;
const HD_TUBE_DRIVE_MODEL_ID: u32 = 0x0205_000d;
const HD_OCTAVE_FUZZ_MODEL_ID: u32 = 0x0205_000c;
pub const HD_TYPE9_CHOICE_MODEL_ID: u32 = 0x020a_0014;
pub const HD_TYPE8_CHOICE_MODEL_ID: u32 = 0x020a_0013;
pub const HD_TYPE8_CHOICE_KEY: u32 = 0x3f10_0004;
pub const HD_TYPE10_CHOICE_MODEL_ID: u32 = 0x020a_0010;
pub const HD_TYPE10_CHOICE_KEY: u32 = 0x3f10_0003;
pub const HD_TYPE10_CHOICE_MODEL_KEYS: [(u32, u32); 2] = [
    (HD_TYPE10_CHOICE_MODEL_ID, HD_TYPE10_CHOICE_KEY),
    (0x020a_000f, 0x3f10_0003),
];
/// Exact source-admitted type-11 controls. The explicit control identity
/// prevents callers from selecting a key by model alone when both keys are
/// present on the same model.
pub const HD_TYPE11_CHOICE_MODEL_KEYS: [(u32, u32); 4] = [
    (0x020a_001e, 0x3f10_0002),
    (0x020a_001e, 0x3f10_0003),
    (0x020a_001d, 0x3f10_0001),
    (0x020a_001d, 0x3f10_0002),
];
/// Exact source-admitted type-9 control associations. Descriptor ordinals and
/// vendor runtime selectors are not file FX slots.
pub const HD_TYPE9_CHOICE_MODEL_KEYS: [(u32, u32); 4] = [
    (0x020a_0014, 0x3f10_0004),
    (0x020a_0010, 0x3f10_0004),
    (0x020a_000f, 0x3f10_0004),
    (0x020a_0012, 0x3f10_0003),
];

pub fn hd_type9_choice_key(model_id: u32) -> Option<u32> {
    HD_TYPE9_CHOICE_MODEL_KEYS
        .iter()
        .find_map(|(admitted_model, key)| (*admitted_model == model_id).then_some(*key))
}

/// The single source-admitted type-8 two-choice association. This is kept
/// separate from the three-choice type-9 catalog so Choice2 cannot be used.
pub fn hd_type8_choice_key(model_id: u32) -> Option<u32> {
    (model_id == HD_TYPE8_CHOICE_MODEL_ID).then_some(HD_TYPE8_CHOICE_KEY)
}

/// Exact independently admitted type-10 control associations.
pub fn hd_type10_choice_key(model_id: u32) -> Option<u32> {
    HD_TYPE10_CHOICE_MODEL_KEYS
        .iter()
        .find_map(|(admitted_model, key)| (*admitted_model == model_id).then_some(*key))
}

/// One of the four explicitly admitted type-11 model/key controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType11Control {
    Model020a001eKey3f100002,
    Model020a001eKey3f100003,
    Model020a001dKey3f100001,
    Model020a001dKey3f100002,
}

impl HdType11Control {
    pub const ALL: [Self; 4] = [
        Self::Model020a001eKey3f100002,
        Self::Model020a001eKey3f100003,
        Self::Model020a001dKey3f100001,
        Self::Model020a001dKey3f100002,
    ];

    pub const fn model_id(self) -> u32 {
        match self {
            Self::Model020a001eKey3f100002 | Self::Model020a001eKey3f100003 => 0x020a_001e,
            Self::Model020a001dKey3f100001 | Self::Model020a001dKey3f100002 => 0x020a_001d,
        }
    }

    pub const fn key(self) -> u32 {
        match self {
            Self::Model020a001eKey3f100002 | Self::Model020a001dKey3f100002 => 0x3f10_0002,
            Self::Model020a001eKey3f100003 => 0x3f10_0003,
            Self::Model020a001dKey3f100001 => 0x3f10_0001,
        }
    }

    pub const fn for_model(model_id: u32) -> &'static [Self] {
        match model_id {
            0x020a_001e => &[
                Self::Model020a001eKey3f100002,
                Self::Model020a001eKey3f100003,
            ],
            0x020a_001d => &[
                Self::Model020a001dKey3f100001,
                Self::Model020a001dKey3f100002,
            ],
            _ => &[],
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Model020a001eKey3f100002 | Self::Model020a001dKey3f100002 => {
                "Control key 0x3F100002"
            }
            Self::Model020a001eKey3f100003 => "Control key 0x3F100003",
            Self::Model020a001dKey3f100001 => "Control key 0x3F100001",
        }
    }
}

/// One of the five exact type-11 menu values. Labels remain neutral.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType11Choice {
    Choice0,
    Choice1,
    Choice2,
    Choice3,
    Choice4,
}

impl HdType11Choice {
    pub const ALL: [Self; 5] = [
        Self::Choice0,
        Self::Choice1,
        Self::Choice2,
        Self::Choice3,
        Self::Choice4,
    ];

    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
            Self::Choice2 => 2,
            Self::Choice3 => 3,
            Self::Choice4 => 4,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
            Self::Choice2 => "Choice 2",
            Self::Choice3 => "Choice 3",
            Self::Choice4 => "Choice 4",
        }
    }

    pub const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3e80_0000,
            Self::Choice2 => 0x3f00_0000,
            Self::Choice3 => 0x3f40_0000,
            Self::Choice4 => 0x3f80_0000,
        }
    }

    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            2 => Ok(Self::Choice2),
            3 => Ok(Self::Choice3),
            4 => Ok(Self::Choice4),
            _ => Err(ToneError::format(
                "HD type-11 choice index must be in 0..=4",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType11ChoiceValue {
    Choice(HdType11Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}
const HD_BAND_SHIFT_EQ_MODEL_ID: u32 = 0x020c_000c;
const HD_GRAPHIC_EQ_MODEL_ID: u32 = 0x020c_0009;
const HD_STUDIO_EQ_MODEL_ID: u32 = 0x020c_000a;
const HD_HARD_GATE_MODEL_ID: u32 = 0x0200_0013;
const HD_STEREO_DELAY_MODEL_ID: u32 = 0x0202_0013;
const HD_FX_LOOP_MODEL_ID: u32 = 0x0208_0000;
const HD_TYPE1_DRIVE_KEY: u32 = 0x3f10_0001;
const HD_TYPE1_BASS_KEY: u32 = 0x3f10_0002;
const HD_TYPE1_THIRD_KEY: u32 = 0x3f10_0003;
const HD_TYPE1_TREBLE_KEY: u32 = 0x3f10_0004;
const HD_TYPE1_OUTPUT_KEY: u32 = 0x3f10_0005;
const HD_VINTAGE_PRE_MODEL_ID: u32 = 0x020c_000e;
const HD_VINTAGE_PRE_DEFAULT_KEYS: [u32; 5] = [
    0x3f10_0002,
    0x3f10_0003,
    0x3f10_0004,
    0x3f10_0005,
    0x3f10_0001,
];

/// One of the exact standalone distortion/fuzz model-default targets admitted
/// for the explicit Octave Fuzz replacement action. This is not a general HD
/// model catalog or arbitrary model-ID writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdDistortionDefault {
    JetFuzz,
    BuzzSaw,
    ClassicDistortion,
    ColorDrive,
    FacialFuzz,
    FuzzPi,
    HeavyDistortion,
    JumboFuzz,
    Line6Distortion,
    Line6Drive,
    Overdrive,
    TubeDrive,
}

impl HdDistortionDefault {
    pub const ALL: [Self; 12] = [
        Self::JetFuzz,
        Self::BuzzSaw,
        Self::ClassicDistortion,
        Self::ColorDrive,
        Self::FacialFuzz,
        Self::FuzzPi,
        Self::HeavyDistortion,
        Self::JumboFuzz,
        Self::Line6Distortion,
        Self::Line6Drive,
        Self::Overdrive,
        Self::TubeDrive,
    ];

    pub const fn model_id(self) -> u32 {
        match self {
            Self::JetFuzz => 0x0205_000a,
            Self::BuzzSaw => 0x0205_0015,
            Self::ClassicDistortion => 0x0205_000b,
            Self::ColorDrive => 0x0205_0018,
            Self::FacialFuzz => 0x0205_0011,
            Self::FuzzPi => 0x0205_000f,
            Self::HeavyDistortion => 0x0205_0016,
            Self::JumboFuzz => 0x0205_0017,
            Self::Line6Distortion => 0x0205_0012,
            Self::Line6Drive => 0x0205_0013,
            Self::Overdrive => 0x0205_0010,
            Self::TubeDrive => 0x0205_000d,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::JetFuzz => "Jet Fuzz",
            Self::BuzzSaw => "Buzz Saw",
            Self::ClassicDistortion => "Classic Distortion",
            Self::ColorDrive => "Color Drive",
            Self::FacialFuzz => "Facial Fuzz",
            Self::FuzzPi => "Fuzz Pi",
            Self::HeavyDistortion => "Heavy Distortion",
            Self::JumboFuzz => "Jumbo Fuzz",
            Self::Line6Distortion => "Line 6 Distortion",
            Self::Line6Drive => "Line 6 Drive",
            Self::Overdrive => "Overdrive",
            Self::TubeDrive => "Tube Drive",
        }
    }
}
// Retained Editor 2.27 source evidence identifies Screamer's five defaults
// in ascending key order, with selector 1 and zero value bits for each row.
const HD_SCREAMER_DEFAULT_KEYS: [u32; 5] = [
    HD_TYPE1_DRIVE_KEY,
    HD_TYPE1_BASS_KEY,
    HD_TYPE1_THIRD_KEY,
    HD_TYPE1_TREBLE_KEY,
    HD_TYPE1_OUTPUT_KEY,
];
// Editor 2.27's Sub Octave Fuzz descriptor initializes the same five ordered
// keys as Screamer, with selector 1 and zero selected value bits.
const HD_SUB_OCTAVE_FUZZ_DEFAULT_KEYS: [u32; 5] = [
    HD_TYPE1_DRIVE_KEY,
    HD_TYPE1_BASS_KEY,
    HD_TYPE1_THIRD_KEY,
    HD_TYPE1_TREBLE_KEY,
    HD_TYPE1_OUTPUT_KEY,
];
/// One of the eight editable HD FX positions. This file-only slot type is
/// independent of vendor runtime slot numbers (which are four through eleven).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HdFxSlot(usize);

impl HdFxSlot {
    pub fn new(index: usize) -> Result<Self, ToneError> {
        if index >= HD_EFFECT_COUNT {
            return Err(ToneError::format("HD FX slot must be in 0..=7"));
        }
        Ok(Self(index))
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

/// One of the three source-joined type-9 choice indices. Labels intentionally
/// remain neutral until their localized strings are independently decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType9Choice {
    Choice0,
    Choice1,
    Choice2,
}

impl HdType9Choice {
    pub const ALL: [Self; 3] = [Self::Choice0, Self::Choice1, Self::Choice2];

    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
            Self::Choice2 => 2,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
            Self::Choice2 => "Choice 2",
        }
    }

    const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3f00_0000,
            Self::Choice2 => 0x3f80_0000,
        }
    }

    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            2 => Ok(Self::Choice2),
            _ => Err(ToneError::format("HD type-9 choice index must be in 0..=2")),
        }
    }
}

/// Lossless read state for an admitted type-9 row. Imported values are
/// never normalized merely because they do not match the three choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType9ChoiceValue {
    Choice(HdType9Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}

/// One of the two source-joined choices for the exact type-8 association.
/// Labels remain neutral; localized resource strings are not decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType8Choice {
    Choice0,
    Choice1,
}

impl HdType8Choice {
    pub const ALL: [Self; 2] = [Self::Choice0, Self::Choice1];

    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
        }
    }

    const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3f80_0000,
        }
    }

    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            _ => Err(ToneError::format("HD type-8 choice index must be 0 or 1")),
        }
    }
}

/// Lossless read state for the exact admitted type-8 row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType8ChoiceValue {
    Choice(HdType8Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}

/// Neutral two-choice enum for the exact type-10 association.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType10Choice {
    Choice0,
    Choice1,
}

impl HdType10Choice {
    pub const ALL: [Self; 2] = [Self::Choice0, Self::Choice1];
    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
        }
    }
    const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3f80_0000,
        }
    }
    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            _ => Err(ToneError::format(
                "HD type-10 choice index must be in 0..=1",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType10ChoiceValue {
    Choice(HdType10Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}

pub const OFFLINE_BANNER: &str =
    "Offline PODxt Live tone file. This does not edit a connected POD.";
pub const HD_EDITOR_OFFLINE_NOTICE: &str = "HD Edit files are handled offline. Internal target and physical compatibility remain unverified.";
pub const NOT_GUITAR: &str =
    "This is not the Variax guitar editor. Guitar model slots are not applied.";
pub const NOT_SENT: &str = "Nothing is sent to a device. No preset is switched.";
pub const PAYLOADS_NOT_DECODED: &str = "Known asset names and 12-byte parameter records are shown. Parameter ranges are not decoded. Unknown ids stay raw.";

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

/// A POD-family file recognized by its byte structure, without implying that
/// its format is supported by a particular product profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PodPresetFile {
    PodxtLive(PodTone),
    H5ep(OpaqueH5epPreset),
    H5es(H5esSetlist),
    H5eb(H5ebBundle),
}

/// A statically framed H5EP record. Header words other than magic/version and
/// all body data remain raw unless a narrowly evidenced control is edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueH5epPreset {
    bytes: Vec<u8>,
}

/// A complete editor setlist with exactly 64 fixed-size H5EP children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5esSetlist {
    bytes: Vec<u8>,
}

/// A complete editor bundle with exactly eight fixed-size H5ES children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5ebBundle {
    bytes: Vec<u8>,
}

/// Screamer controls whose Editor 2.27 descriptor, numeric widget, and keyed
/// float write path are statically joined. This describes editor behavior,
/// not availability on a physical device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreamerControl {
    Drive,
    Bass,
    Tone,
    Treble,
    Output,
}

impl ScreamerControl {
    pub const ALL: [Self; 5] = [
        Self::Drive,
        Self::Bass,
        Self::Tone,
        Self::Treble,
        Self::Output,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "DRIVE",
            Self::Bass => "BASS",
            Self::Tone => "TONE",
            Self::Treble => "TREBLE",
            Self::Output => "OUTPUT",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Drive => HD_TYPE1_DRIVE_KEY,
            Self::Bass => HD_TYPE1_BASS_KEY,
            Self::Tone => HD_TYPE1_THIRD_KEY,
            Self::Treble => HD_TYPE1_TREBLE_KEY,
            Self::Output => HD_TYPE1_OUTPUT_KEY,
        }
    }
}

/// Tube Drive's type-1 controls share the recovered key family but are
/// admitted only for the Tube Drive model descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TubeDriveControl {
    Drive,
    Bass,
    Mid,
    Treble,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OctaveFuzzControl {
    Drive,
    Bass,
    Mid,
    Treble,
    Output,
}

impl OctaveFuzzControl {
    pub const ALL: [Self; 5] = [
        Self::Drive,
        Self::Bass,
        Self::Mid,
        Self::Treble,
        Self::Output,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "DRIVE",
            Self::Bass => "BASS",
            Self::Mid => "MID",
            Self::Treble => "TREBLE",
            Self::Output => "OUTPUT",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Drive => HD_TYPE1_DRIVE_KEY,
            Self::Bass => HD_TYPE1_BASS_KEY,
            Self::Mid => HD_TYPE1_THIRD_KEY,
            Self::Treble => HD_TYPE1_TREBLE_KEY,
            Self::Output => HD_TYPE1_OUTPUT_KEY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandShiftEqControl {
    Shift,
}

impl BandShiftEqControl {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Shift => "SHIFT",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Shift => HD_TYPE1_OUTPUT_KEY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StereoDelayControl {
    LeftFeedback,
    RightFeedback,
    Mix,
}

impl StereoDelayControl {
    pub const ALL: [Self; 3] = [Self::LeftFeedback, Self::RightFeedback, Self::Mix];

    pub const fn label(self) -> &'static str {
        match self {
            Self::LeftFeedback => "L FDBK",
            Self::RightFeedback => "R FDBK",
            Self::Mix => "MIX",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::LeftFeedback => HD_TYPE1_DRIVE_KEY,
            Self::RightFeedback => HD_TYPE1_THIRD_KEY,
            Self::Mix => HD_TYPE1_TREBLE_KEY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxLoopControl {
    Mix,
}

impl FxLoopControl {
    pub const fn label(self) -> &'static str {
        "MIX"
    }

    const fn key(self) -> u32 {
        HD_TYPE1_TREBLE_KEY
    }
}

impl TubeDriveControl {
    pub const ALL: [Self; 5] = [
        Self::Drive,
        Self::Bass,
        Self::Mid,
        Self::Treble,
        Self::Output,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "DRIVE",
            Self::Bass => "BASS",
            Self::Mid => "MID",
            Self::Treble => "TREBLE",
            Self::Output => "OUTPUT",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Drive => HD_TYPE1_DRIVE_KEY,
            Self::Bass => HD_TYPE1_BASS_KEY,
            Self::Mid => HD_TYPE1_THIRD_KEY,
            Self::Treble => HD_TYPE1_TREBLE_KEY,
            Self::Output => HD_TYPE1_OUTPUT_KEY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType1Control {
    Screamer(ScreamerControl),
    TubeDrive(TubeDriveControl),
    OctaveFuzz(OctaveFuzzControl),
    BandShiftEq(BandShiftEqControl),
    StereoDelay(StereoDelayControl),
    FxLoop(FxLoopControl),
}

impl HdType1Control {
    pub const ALL: [Self; 20] = [
        Self::Screamer(ScreamerControl::Drive),
        Self::Screamer(ScreamerControl::Bass),
        Self::Screamer(ScreamerControl::Tone),
        Self::Screamer(ScreamerControl::Treble),
        Self::Screamer(ScreamerControl::Output),
        Self::TubeDrive(TubeDriveControl::Drive),
        Self::TubeDrive(TubeDriveControl::Bass),
        Self::TubeDrive(TubeDriveControl::Mid),
        Self::TubeDrive(TubeDriveControl::Treble),
        Self::TubeDrive(TubeDriveControl::Output),
        Self::OctaveFuzz(OctaveFuzzControl::Drive),
        Self::OctaveFuzz(OctaveFuzzControl::Bass),
        Self::OctaveFuzz(OctaveFuzzControl::Mid),
        Self::OctaveFuzz(OctaveFuzzControl::Treble),
        Self::OctaveFuzz(OctaveFuzzControl::Output),
        Self::BandShiftEq(BandShiftEqControl::Shift),
        Self::StereoDelay(StereoDelayControl::LeftFeedback),
        Self::StereoDelay(StereoDelayControl::RightFeedback),
        Self::StereoDelay(StereoDelayControl::Mix),
        Self::FxLoop(FxLoopControl::Mix),
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Screamer(c) => c.label(),
            Self::TubeDrive(c) => c.label(),
            Self::OctaveFuzz(c) => c.label(),
            Self::BandShiftEq(c) => c.label(),
            Self::StereoDelay(c) => c.label(),
            Self::FxLoop(c) => c.label(),
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Screamer(c) => c.key(),
            Self::TubeDrive(c) => c.key(),
            Self::OctaveFuzz(c) => c.key(),
            Self::BandShiftEq(c) => c.key(),
            Self::StereoDelay(c) => c.key(),
            Self::FxLoop(c) => c.key(),
        }
    }

    pub const fn model_label(self) -> &'static str {
        match self {
            Self::Screamer(_) => "Screamer",
            Self::TubeDrive(_) => "Tube Drive",
            Self::OctaveFuzz(_) => "Octave Fuzz",
            Self::BandShiftEq(_) => "4 Band Shift EQ",
            Self::StereoDelay(_) => "Stereo Delay",
            Self::FxLoop(_) => "FX Loop",
        }
    }

    pub const fn model_id(self) -> u32 {
        match self {
            Self::Screamer(_) => HD_SCREAMER_MODEL_ID,
            Self::TubeDrive(_) => HD_TUBE_DRIVE_MODEL_ID,
            Self::OctaveFuzz(_) => HD_OCTAVE_FUZZ_MODEL_ID,
            Self::BandShiftEq(_) => HD_BAND_SHIFT_EQ_MODEL_ID,
            Self::StereoDelay(_) => HD_STEREO_DELAY_MODEL_ID,
            Self::FxLoop(_) => HD_FX_LOOP_MODEL_ID,
        }
    }
}

/// Source-joined primary type-1 controls for the selected delay model family.
/// Labels come from Editor 2.27 `PRMS` resources; their physical meaning and
/// availability are not asserted for any connected product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdType1MappedControl {
    model_id: u32,
    model_label: &'static str,
    label: &'static str,
    key: u32,
}

impl HdType1MappedControl {
    const fn new(model_id: u32, model_label: &'static str, label: &'static str, key: u32) -> Self {
        Self {
            model_id,
            model_label,
            label,
            key,
        }
    }

    pub const fn model_id(self) -> u32 {
        self.model_id
    }
    pub const fn model_label(self) -> &'static str {
        self.model_label
    }
    pub const fn label(self) -> &'static str {
        self.label
    }
    pub const fn key(self) -> u32 {
        self.key
    }
}

/// Source-joined primary type-1 controls from the corrected Editor 2.27
/// effect descriptor matrix. The source trace gives the common primary-widget
/// setup (integer 0..=100, step 1) and normalized float value path; this does
/// not claim physical availability or cross-family compatibility.
///
/// Rows come only from nonzero-key, raw type-1 primary descriptors. Auxiliary
/// widget candidates are not separate controls here; the edit path still
/// rejects nonzero assignment flags and preserves all other bytes.
pub const HD_TYPE1_MAPPED_CONTROLS: [HdType1MappedControl; 364] = [
    HdType1MappedControl::new(0x0205000C, "Octave Fuzz", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000C, "Octave Fuzz", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0205000C, "Octave Fuzz", "MID", 0x3F100003),
    HdType1MappedControl::new(0x0205000C, "Octave Fuzz", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x0205000C, "Octave Fuzz", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0205000E, "Screamer", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000E, "Screamer", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0205000E, "Screamer", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0205000E, "Screamer", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x0205000E, "Screamer", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0205000A, "Jet Fuzz", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000A, "Jet Fuzz", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x0205000A, "Jet Fuzz", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0205000A, "Jet Fuzz", "SPEED", 0x3F100004),
    HdType1MappedControl::new(0x0205000A, "Jet Fuzz", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050015, "Buzz Saw", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050015, "Buzz Saw", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050015, "Buzz Saw", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050015, "Buzz Saw", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050015, "Buzz Saw", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0205000B, "Classic Distortion", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000B, "Classic Distortion", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0205000B, "Classic Distortion", "FILTER", 0x3F100003),
    HdType1MappedControl::new(0x0205000B, "Classic Distortion", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x0205000B, "Classic Distortion", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050018, "Color Drive", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050018, "Color Drive", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050018, "Color Drive", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050018, "Color Drive", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050018, "Color Drive", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050011, "Facial Fuzz", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050011, "Facial Fuzz", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050011, "Facial Fuzz", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050011, "Facial Fuzz", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050011, "Facial Fuzz", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0205000F, "Fuzz Pi", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000F, "Fuzz Pi", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0205000F, "Fuzz Pi", "MID", 0x3F100003),
    HdType1MappedControl::new(0x0205000F, "Fuzz Pi", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x0205000F, "Fuzz Pi", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050016, "Heavy Distortion", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050016, "Heavy Distortion", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050016, "Heavy Distortion", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050016, "Heavy Distortion", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050016, "Heavy Distortion", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050017, "Jumbo Fuzz", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050017, "Jumbo Fuzz", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050017, "Jumbo Fuzz", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050017, "Jumbo Fuzz", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050017, "Jumbo Fuzz", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050012, "Line 6 Distortion", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050012, "Line 6 Distortion", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050012, "Line 6 Distortion", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050012, "Line 6 Distortion", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050012, "Line 6 Distortion", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050013, "Line 6 Drive", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050013, "Line 6 Drive", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050013, "Line 6 Drive", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050013, "Line 6 Drive", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050013, "Line 6 Drive", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050010, "Overdrive", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050010, "Overdrive", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050010, "Overdrive", "MID", 0x3F100003),
    HdType1MappedControl::new(0x02050010, "Overdrive", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050010, "Overdrive", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02050014, "Sub Octave Fuzz", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02050014, "Sub Octave Fuzz", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02050014, "Sub Octave Fuzz", "SUB", 0x3F100003),
    HdType1MappedControl::new(0x02050014, "Sub Octave Fuzz", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02050014, "Sub Octave Fuzz", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0205000D, "Tube Drive", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x0205000D, "Tube Drive", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0205000D, "Tube Drive", "MID", 0x3F100003),
    HdType1MappedControl::new(0x0205000D, "Tube Drive", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x0205000D, "Tube Drive", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x02000011, "Noise Gate", "THRESH", 0x3F100001),
    HdType1MappedControl::new(0x02000011, "Noise Gate", "DECAY", 0x3F100002),
    HdType1MappedControl::new(0x02000012, "Boost Comp", "DRIVE", 0x3F100001),
    HdType1MappedControl::new(0x02000012, "Boost Comp", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02000012, "Boost Comp", "COMP", 0x3F100003),
    HdType1MappedControl::new(0x02000012, "Boost Comp", "TREBLE", 0x3F100004),
    HdType1MappedControl::new(0x02000012, "Boost Comp", "OUTPUT", 0x3F100005),
    HdType1MappedControl::new(0x0200000B, "Red Comp", "SUSTAIN", 0x3F100001),
    HdType1MappedControl::new(0x0200000B, "Red Comp", "LEVEL", 0x3F100002),
    HdType1MappedControl::new(0x0200000C, "Blue Comp", "SUSTAIN", 0x3F100001),
    HdType1MappedControl::new(0x0200000C, "Blue Comp", "LEVEL", 0x3F100002),
    HdType1MappedControl::new(0x0200000D, "Blue Comp Treb", "SUSTAIN", 0x3F100002),
    HdType1MappedControl::new(0x0200000D, "Blue Comp Treb", "LEVEL", 0x3F100001),
    HdType1MappedControl::new(0x0200000F, "Vetta Comp", "SENS", 0x3F100001),
    HdType1MappedControl::new(0x0200000F, "Vetta Comp", "LEVEL", 0x3F100002),
    HdType1MappedControl::new(0x02000010, "Vetta Juice", "AMOUNT", 0x3F100001),
    HdType1MappedControl::new(0x02000010, "Vetta Juice", "LEVEL", 0x3F100002),
    HdType1MappedControl::new(0x0200000E, "Tube Comp", "THRESH", 0x3F100001),
    HdType1MappedControl::new(0x0200000E, "Tube Comp", "LEVEL", 0x3F100002),
    HdType1MappedControl::new(0x02040023, "Octo", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040023, "Octo", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040023, "Octo", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040022, "Ducking", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040022, "Ducking", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040022, "Ducking", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0204001E, "Plate", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x0204001E, "Plate", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0204001E, "Plate", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0204001F, "Room", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x0204001F, "Room", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0204001F, "Room", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040020, "Chamber", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040020, "Chamber", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040020, "Chamber", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040021, "Hall", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040021, "Hall", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040021, "Hall", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040024, "Cave", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040024, "Cave", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040024, "Cave", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040025, "Tile", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040025, "Tile", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040025, "Tile", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040026, "Echo", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x02040026, "Echo", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x02040026, "Echo", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0204001C, "'63 Spring", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x0204001C, "'63 Spring", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0204001C, "'63 Spring", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0204001D, "Spring", "DECAY", 0x3F100001),
    HdType1MappedControl::new(0x0204001D, "Spring", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0204001D, "Spring", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02040027, "Particle Verb", "DWELL", 0x3F100001),
    HdType1MappedControl::new(0x02040027, "Particle Verb", "GAIN", 0x3F100003),
    HdType1MappedControl::new(0x02040027, "Particle Verb", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020C000B, "Parametric EQ", "LOWS", 0x3F100001),
    HdType1MappedControl::new(0x020C000B, "Parametric EQ", "HIGHS", 0x3F100002),
    HdType1MappedControl::new(0x020C000B, "Parametric EQ", "FREQ", 0x3F100003),
    HdType1MappedControl::new(0x020C000B, "Parametric EQ", "Q", 0x3F100004),
    HdType1MappedControl::new(0x020C000B, "Parametric EQ", "GAIN", 0x3F100005),
    HdType1MappedControl::new(0x020C000C, "4 Band Shift EQ", "SHIFT", 0x3F100005),
    HdType1MappedControl::new(0x020C000D, "Mid Focus EQ", "HP FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020C000D, "Mid Focus EQ", "HP Q", 0x3F100002),
    HdType1MappedControl::new(0x020C000D, "Mid Focus EQ", "LP FREQ", 0x3F100003),
    HdType1MappedControl::new(0x020C000D, "Mid Focus EQ", "LP Q", 0x3F100004),
    HdType1MappedControl::new(0x020C000D, "Mid Focus EQ", "GAIN", 0x3F100005),
    HdType1MappedControl::new(0x020C000E, "Vintage Pre", "GAIN", 0x3F100002),
    HdType1MappedControl::new(0x020C000E, "Vintage Pre", "OUTPUT", 0x3F100003),
    HdType1MappedControl::new(0x02030049, "80A Flanger", "RANGE", 0x3F100001),
    HdType1MappedControl::new(0x02030049, "80A Flanger", "ENHANCE", 0x3F100002),
    HdType1MappedControl::new(0x02030049, "80A Flanger", "MANUAL", 0x3F100003),
    HdType1MappedControl::new(0x0203002A, "Analog Chorus", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x0203002A, "Analog Chorus", "TONE", 0x3F100003),
    HdType1MappedControl::new(0x0203002A, "Analog Chorus", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030028, "Analog Flanger", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030028, "Analog Flanger", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x02030028, "Analog Flanger", "MANUAL", 0x3F100003),
    HdType1MappedControl::new(0x02030028, "Analog Flanger", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030040, "Barberpole Phaser", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02030040, "Barberpole Phaser", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030021, "Bias Tremolo", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030021, "Bias Tremolo", "SHAPE", 0x3F100002),
    HdType1MappedControl::new(0x02030021, "Bias Tremolo", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x02030021, "Bias Tremolo", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002B, "Dimension", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030023, "Dual Phaser", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030023, "Dual Phaser", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x02030023, "Dual Phaser", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030042, "Frequency Shifter", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030029, "Jet FLanger", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030029, "Jet FLanger", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x02030029, "Jet FLanger", "MANUAL", 0x3F100003),
    HdType1MappedControl::new(0x02030029, "Jet FLanger", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030047, "AC Flanger", "WIDTH", 0x3F100001),
    HdType1MappedControl::new(0x02030047, "AC Flanger", "REGEN", 0x3F100002),
    HdType1MappedControl::new(0x02030047, "AC Flanger", "MANUAL", 0x3F100003),
    HdType1MappedControl::new(0x02030047, "AC Flanger", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030020, "Opto Tremolo", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030020, "Opto Tremolo", "SHAPE", 0x3F100002),
    HdType1MappedControl::new(0x02030020, "Opto Tremolo", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x02030020, "Opto Tremolo", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030024, "Panned Phaser", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030024, "Panned Phaser", "PANSPD", 0x3F100003),
    HdType1MappedControl::new(0x02030024, "Panned Phaser", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002F, "Panner", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x0203002F, "Panner", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x0203002F, "Panner", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030022, "Phaser", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030022, "Phaser", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x02030022, "Phaser", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002D, "Pitch Vibrato", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x0203002D, "Pitch Vibrato", "RISE", 0x3F100002),
    HdType1MappedControl::new(0x0203002D, "Pitch Vibrato", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x0203002D, "Pitch Vibrato", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030026, "Rotary Drum", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030026, "Rotary Drum", "TONE", 0x3F100002),
    HdType1MappedControl::new(0x02030026, "Rotary Drum", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x02030026, "Rotary Drum", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02030027, "Rotary Drum & Horn", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030027, "Rotary Drum & Horn", "HDEPTH", 0x3F100002),
    HdType1MappedControl::new(0x02030027, "Rotary Drum & Horn", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x02030027, "Rotary Drum & Horn", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002C, "Tri Chorus", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002C, "Tri Chorus", "DEPTH 1", 0x3F100001),
    HdType1MappedControl::new(0x0203002C, "Tri Chorus", "DEPTH 2", 0x3F100002),
    HdType1MappedControl::new(0x0203002C, "Tri Chorus", "DEPTH 3", 0x3F100003),
    HdType1MappedControl::new(0x02030025, "U-Vibe", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x02030025, "U-Vibe", "FDBACK", 0x3F100002),
    HdType1MappedControl::new(0x02030025, "U-Vibe", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x02030025, "U-Vibe", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0015, "Throbber", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0015, "Throbber", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0015, "Throbber", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0016, "Spin Cycle", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0016, "Spin Cycle", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0016, "Spin Cycle", "VOLSNS", 0x3F100003),
    HdType1MappedControl::new(0x020A0016, "Spin Cycle", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0202001A, "Multi-Head Delay", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202001A, "Multi-Head Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020023, "Auto-Volume", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020023, "Auto-Volume", "DEPTH", 0x3F100002),
    HdType1MappedControl::new(0x02020023, "Auto-Volume", "SWELL", 0x3F100003),
    HdType1MappedControl::new(0x02020023, "Auto-Volume", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020014, "Analog Echo", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020014, "Analog Echo", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02020014, "Analog Echo", "TREBLE", 0x3F100003),
    HdType1MappedControl::new(0x02020014, "Analog Echo", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020015, "Analog Dly w/Mod", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020015, "Analog Dly w/Mod", "MODSPD", 0x3F100002),
    HdType1MappedControl::new(0x02020015, "Analog Dly w/Mod", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x02020015, "Analog Dly w/Mod", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002E, "Sweep Echo", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002E, "Sweep Echo", "SWPSPD", 0x3F100002),
    HdType1MappedControl::new(0x0202002E, "Sweep Echo", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x0202002E, "Sweep Echo", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002F, "Sweep Echo Dry", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002F, "Sweep Echo Dry", "SWPSPD", 0x3F100002),
    HdType1MappedControl::new(0x0202002F, "Sweep Echo Dry", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x0202002F, "Sweep Echo Dry", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020030, "Echo Platter", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020030, "Echo Platter", "WOWFLT", 0x3F100002),
    HdType1MappedControl::new(0x02020030, "Echo Platter", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x02020030, "Echo Platter", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020031, "Echo Platter Dry", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020031, "Echo Platter Dry", "WOWFLT", 0x3F100002),
    HdType1MappedControl::new(0x02020031, "Echo Platter Dry", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x02020031, "Echo Platter Dry", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002A, "Tube Echo", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002A, "Tube Echo", "WOWFLT", 0x3F100002),
    HdType1MappedControl::new(0x0202002A, "Tube Echo", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x0202002A, "Tube Echo", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002B, "Tube Echo Dry", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002B, "Tube Echo Dry", "WOWFLT", 0x3F100002),
    HdType1MappedControl::new(0x0202002B, "Tube Echo Dry", "DRIVE", 0x3F100003),
    HdType1MappedControl::new(0x0202002B, "Tube Echo Dry", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002C, "Tape Echo", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002C, "Tape Echo", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0202002C, "Tape Echo", "TREBLE", 0x3F100003),
    HdType1MappedControl::new(0x0202002C, "Tape Echo", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202002D, "Tape Echo Dry", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202002D, "Tape Echo Dry", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x0202002D, "Tape Echo Dry", "TREBLE", 0x3F100003),
    HdType1MappedControl::new(0x0202002D, "Tape Echo Dry", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x0202001D, "Lo Res Delay", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x0202001D, "Lo Res Delay", "TONE", 0x3F100002),
    HdType1MappedControl::new(0x0202001D, "Lo Res Delay", "RESO", 0x3F100003),
    HdType1MappedControl::new(0x0202001D, "Lo Res Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020012, "Digital Dly w/Mod", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020012, "Digital Dly w/Mod", "MODSPD", 0x3F100002),
    HdType1MappedControl::new(0x02020012, "Digital Dly w/Mod", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x02020012, "Digital Dly w/Mod", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020021, "Reverse Delay", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020021, "Reverse Delay", "MODSPD", 0x3F100002),
    HdType1MappedControl::new(0x02020021, "Reverse Delay", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x02020021, "Reverse Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020013, "Stereo Delay", "L FDBK", 0x3F100001),
    HdType1MappedControl::new(0x02020013, "Stereo Delay", "R FDBK", 0x3F100003),
    HdType1MappedControl::new(0x02020013, "Stereo Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020011, "Digital Delay", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020011, "Digital Delay", "BASS", 0x3F100002),
    HdType1MappedControl::new(0x02020011, "Digital Delay", "TREBLE", 0x3F100003),
    HdType1MappedControl::new(0x02020011, "Digital Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020022, "Dynamic Delay", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020022, "Dynamic Delay", "THRESH", 0x3F100002),
    HdType1MappedControl::new(0x02020022, "Dynamic Delay", "DUCKING", 0x3F100003),
    HdType1MappedControl::new(0x02020022, "Dynamic Delay", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02020020, "Ping Pong", "FDBACK", 0x3F100001),
    HdType1MappedControl::new(0x02020020, "Ping Pong", "OFFSET", 0x3F100002),
    HdType1MappedControl::new(0x02020020, "Ping Pong", "SPREAD", 0x3F100003),
    HdType1MappedControl::new(0x02020020, "Ping Pong", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x020A001B, "Attack Synth", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A001B, "Attack Synth", "SPEED", 0x3F100003),
    HdType1MappedControl::new(0x020A001B, "Attack Synth", "PITCH", 0x3F100004),
    HdType1MappedControl::new(0x020A001B, "Attack Synth", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A001C, "Synth String", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A001C, "Synth String", "ATTACK", 0x3F100002),
    HdType1MappedControl::new(0x020A001C, "Synth String", "PITCH", 0x3F100003),
    HdType1MappedControl::new(0x020A001C, "Synth String", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0019, "Growler", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0019, "Growler", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0019, "Growler", "PITCH", 0x3F100003),
    HdType1MappedControl::new(0x020A0019, "Growler", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A001A, "Synth-O-Matic", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A001A, "Synth-O-Matic", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A001A, "Synth-O-Matic", "PITCH", 0x3F100004),
    HdType1MappedControl::new(0x020A001A, "Synth-O-Matic", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02090004, "Bass Octaver", "TONE", 0x3F100001),
    HdType1MappedControl::new(0x02090004, "Bass Octaver", "NORMAL", 0x3F100002),
    HdType1MappedControl::new(0x02090004, "Bass Octaver", "OCTAVE", 0x3F100003),
    HdType1MappedControl::new(0x020A0013, "Slow Filter", "SPEED", 0x3F100003),
    HdType1MappedControl::new(0x020A0013, "Slow Filter", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0013, "Slow Filter", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0013, "Slow Filter", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0010, "Tron Down", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0010, "Tron Down", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0010, "Tron Down", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A000F, "Tron Up", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A000F, "Tron Up", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A000F, "Tron Up", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0014, "Q Filter", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0014, "Q Filter", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0014, "Q Filter", "GAIN", 0x3F100003),
    HdType1MappedControl::new(0x020A0014, "Q Filter", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0026, "Vocoder", "INPUT", 0x3F100001),
    HdType1MappedControl::new(0x020A0026, "Vocoder", "MIC", 0x3F100002),
    HdType1MappedControl::new(0x020A0026, "Vocoder", "DECAY", 0x3F100003),
    HdType1MappedControl::new(0x020A0026, "Vocoder", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0018, "Octisynth", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0018, "Octisynth", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0018, "Octisynth", "DEPTH", 0x3F100003),
    HdType1MappedControl::new(0x020A0018, "Octisynth", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A001E, "V-Tron", "SPEED", 0x3F100001),
    HdType1MappedControl::new(0x020A001E, "V-Tron", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0017, "Comet Trails", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0017, "Comet Trails", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0017, "Comet Trails", "GAIN", 0x3F100003),
    HdType1MappedControl::new(0x020A0017, "Comet Trails", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0011, "Seeker", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0011, "Seeker", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0011, "Seeker", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A0012, "Obi Wah", "FREQ", 0x3F100001),
    HdType1MappedControl::new(0x020A0012, "Obi Wah", "Q", 0x3F100002),
    HdType1MappedControl::new(0x020A0012, "Obi Wah", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0203002E, "Ring Modulator", "AM/FM", 0x3F100003),
    HdType1MappedControl::new(0x0203002E, "Ring Modulator", "SPEED", 0x3F100004),
    HdType1MappedControl::new(0x0203002E, "Ring Modulator", "DEPTH", 0x3F100001),
    HdType1MappedControl::new(0x0203002E, "Ring Modulator", "SHAPE", 0x3F100002),
    HdType1MappedControl::new(0x0203002E, "Ring Modulator", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x020A001D, "Voice Box", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02090003, "Smart Harmony", "MIX", 0x3F010001),
    HdType1MappedControl::new(0x02090002, "Pitch Glide", "MIX", 0x3F010001),
    HdType1MappedControl::new(0x0206000B, "Vetta Wah", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x0206000B, "Vetta Wah", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0206000C, "Fassel", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x0206000C, "Fassel", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0206000E, "Chrome", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x0206000E, "Chrome", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0206000D, "Weeper", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x0206000D, "Weeper", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02060010, "Throaty", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x02060010, "Throaty", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x0206000F, "Chrome Custom", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x0206000F, "Chrome Custom", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02060011, "Conductor", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x02060011, "Conductor", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02060012, "Colorful", "POSITION", 0x3F100001),
    HdType1MappedControl::new(0x02060012, "Colorful", "MIX", 0x3F100005),
    HdType1MappedControl::new(0x02080000, "FX Loop", "MIX", 0x3F100004),
    HdType1MappedControl::new(0x02070004, "Volume Pedal", "VOLUME", 0x3F100001),
];

/// A source-joined non-type-1 numeric editor control. Bounds are the
/// constructor's integer widget arguments, not named physical units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdNumericControl {
    model_id: u32,
    model_label: &'static str,
    label: &'static str,
    key: u32,
    raw_type: u8,
    min: i32,
    max: i32,
}

impl HdNumericControl {
    const fn new(
        model_id: u32,
        model_label: &'static str,
        label: &'static str,
        key: u32,
        raw_type: u8,
        min: i32,
        max: i32,
    ) -> Self {
        Self {
            model_id,
            model_label,
            label,
            key,
            raw_type,
            min,
            max,
        }
    }

    pub const fn model_id(self) -> u32 {
        self.model_id
    }

    pub const fn model_label(self) -> &'static str {
        self.model_label
    }

    pub const fn label(self) -> &'static str {
        self.label
    }

    pub const fn raw_type(self) -> u8 {
        self.raw_type
    }

    pub const fn min(self) -> i32 {
        self.min
    }

    pub const fn max(self) -> i32 {
        self.max
    }

    const fn key(self) -> u32 {
        self.key
    }
}

/// Independently joined selected numeric controls across equalizer, gate,
/// delay and FX Loop descriptors. This is not a complete model catalog.
pub const HD_NUMERIC_CONTROLS: [HdNumericControl; 80] = [
    HdNumericControl::new(
        HD_GRAPHIC_EQ_MODEL_ID,
        "Graphic EQ",
        "80Hz",
        0x3f10_0001,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_GRAPHIC_EQ_MODEL_ID,
        "Graphic EQ",
        "220Hz",
        0x3f10_0002,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_GRAPHIC_EQ_MODEL_ID,
        "Graphic EQ",
        "440Hz",
        0x3f10_0003,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_GRAPHIC_EQ_MODEL_ID,
        "Graphic EQ",
        "1.1kHz",
        0x3f10_0004,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_GRAPHIC_EQ_MODEL_ID,
        "Graphic EQ",
        "2.2kHz",
        0x3f10_0005,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_BAND_SHIFT_EQ_MODEL_ID,
        "4 Band Shift EQ",
        "LO FREQ",
        0x3f10_0001,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_BAND_SHIFT_EQ_MODEL_ID,
        "4 Band Shift EQ",
        "LO MID",
        0x3f10_0002,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_BAND_SHIFT_EQ_MODEL_ID,
        "4 Band Shift EQ",
        "HI MID",
        0x3f10_0003,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_BAND_SHIFT_EQ_MODEL_ID,
        "4 Band Shift EQ",
        "HIGH",
        0x3f10_0004,
        0x05,
        0,
        240,
    ),
    HdNumericControl::new(
        HD_STUDIO_EQ_MODEL_ID,
        "Studio EQ",
        "LO GAIN",
        0x3f10_0002,
        0x06,
        0,
        220,
    ),
    HdNumericControl::new(
        HD_STUDIO_EQ_MODEL_ID,
        "Studio EQ",
        "HI GAIN",
        0x3f10_0004,
        0x06,
        0,
        220,
    ),
    HdNumericControl::new(
        HD_STUDIO_EQ_MODEL_ID,
        "Studio EQ",
        "GAIN",
        0x3f10_0005,
        0x07,
        0,
        360,
    ),
    HdNumericControl::new(
        HD_HARD_GATE_MODEL_ID,
        "Hard Gate",
        "OPEN",
        0x3f10_0000,
        0x08,
        -96,
        0,
    ),
    HdNumericControl::new(
        HD_HARD_GATE_MODEL_ID,
        "Hard Gate",
        "CLOSE",
        0x3f10_0001,
        0x08,
        -96,
        0,
    ),
    HdNumericControl::new(
        HD_HARD_GATE_MODEL_ID,
        "Hard Gate",
        "HOLD TIME",
        0x3f10_0002,
        0x12,
        0,
        800,
    ),
    HdNumericControl::new(
        HD_HARD_GATE_MODEL_ID,
        "Hard Gate",
        "DECAY",
        0x3f10_0003,
        0x13,
        0,
        4000,
    ),
    HdNumericControl::new(
        HD_STEREO_DELAY_MODEL_ID,
        "Stereo Delay",
        "L TIME",
        0x3f10_0000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        HD_STEREO_DELAY_MODEL_ID,
        "Stereo Delay",
        "R TIME",
        0x3f10_0002,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        HD_FX_LOOP_MODEL_ID,
        "FX Loop",
        "SEND",
        0x3f10_0001,
        0x09,
        -80,
        0,
    ),
    HdNumericControl::new(
        HD_FX_LOOP_MODEL_ID,
        "FX Loop",
        "RETURN",
        0x3f10_0002,
        0x0a,
        0,
        24,
    ),
    HdNumericControl::new(
        0x02040023,
        "Octo",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040022,
        "Ducking",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x0204001e,
        "Plate",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x0204001f,
        "Room",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040020,
        "Chamber",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040021,
        "Hall",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040024,
        "Cave",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040025,
        "Tile",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x02040026,
        "Echo",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x0204001c,
        "'63 Spring",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x0204001d,
        "Spring",
        "Key 0x3F100002",
        0x3f100002,
        0x11,
        0,
        200,
    ),
    HdNumericControl::new(
        0x020c000e,
        "Vintage Pre",
        "Key 0x3F100004",
        0x3f100004,
        0x1a,
        20,
        500,
    ),
    HdNumericControl::new(
        0x020c000e,
        "Vintage Pre",
        "Key 0x3F100005",
        0x3f100005,
        0x1b,
        0,
        15000,
    ),
    HdNumericControl::new(
        0x02030049,
        "80A Flanger",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x0203002a,
        "Analog Chorus",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030028,
        "Analog Flanger",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030040,
        "Barberpole Phaser",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030021,
        "Bias Tremolo",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030023,
        "Dual Phaser",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030042,
        "Frequency Shifter",
        "Key 0x3F100001",
        0x3f100001,
        0x0f,
        0,
        3520,
    ),
    HdNumericControl::new(
        0x02030029,
        "Jet FLanger",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030047,
        "AC Flanger",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030020,
        "Opto Tremolo",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030024,
        "Panned Phaser",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x0203002f,
        "Panner",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030043,
        "Pattern Tremolo",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030022,
        "Phaser",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x0203002d,
        "Pitch Vibrato",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030045,
        "Script Phase",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x0203002c,
        "Tri Chorus",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02030025,
        "U-Vibe",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0015,
        "Throbber",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0016,
        "Spin Cycle",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x0202001a,
        "Multi-Head Delay",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020023,
        "Auto-Volume",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020014,
        "Analog Echo",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020015,
        "Analog Dly w/Mod",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002e,
        "Sweep Echo",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002f,
        "Sweep Echo Dry",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020030,
        "Echo Platter",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020031,
        "Echo Platter Dry",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002a,
        "Tube Echo",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002b,
        "Tube Echo Dry",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002c,
        "Tape Echo",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202002d,
        "Tape Echo Dry",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x0202001d,
        "Lo Res Delay",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020012,
        "Digital Dly w/Mod",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020021,
        "Reverse Delay",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020011,
        "Digital Delay",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020022,
        "Dynamic Delay",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x02020020,
        "Ping Pong",
        "Key 0x3F100000",
        0x3f100000,
        0x10,
        20,
        2000,
    ),
    HdNumericControl::new(
        0x020a001c,
        "Synth String",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0019,
        "Growler",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0018,
        "Octisynth",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0017,
        "Comet Trails",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0011,
        "Seeker",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a0012,
        "Obi Wah",
        "Key 0x3F100000",
        0x3f100000,
        0x0d,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x020a001d,
        "Voice Box",
        "Key 0x3F100000",
        0x3f100000,
        0x0e,
        0,
        1400,
    ),
    HdNumericControl::new(
        0x02090002,
        "Pitch Glide",
        "Key 0x3F100001",
        0x3f100001,
        0x14,
        -240,
        240,
    ),
    HdNumericControl::new(
        0x02070005,
        "Pan",
        "Key 0x3F100001",
        0x3f100001,
        0x16,
        0,
        200,
    ),
];

impl PodPresetFile {
    /// Dispatches only on the file's structural magic. Suffixes are not used.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.starts_with(b"H5EP") {
            OpaqueH5epPreset::parse(bytes).map(Self::H5ep)
        } else if bytes.starts_with(b"H5ES") {
            H5esSetlist::parse(bytes).map(Self::H5es)
        } else if bytes.starts_with(b"H5EB") {
            H5ebBundle::parse(bytes).map(Self::H5eb)
        } else if bytes.starts_with(b"FORM") {
            PodTone::parse(bytes).map(Self::PodxtLive)
        } else {
            Err(ToneError::format(
                "unrecognized individual POD file structure",
            ))
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::PodxtLive(tone) => tone.as_bytes(),
            Self::H5ep(preset) => preset.as_bytes(),
            Self::H5es(setlist) => setlist.as_bytes(),
            Self::H5eb(bundle) => bundle.as_bytes(),
        }
    }

    /// Exact no-op serialization. This never edits or normalizes stored bytes.
    pub fn serialize_noop(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, self.as_bytes()).map_err(ToneError::format)
    }

    pub fn hd_container_kind(&self) -> Option<&'static str> {
        match self {
            Self::H5ep(_) => Some("H5EP preset"),
            Self::H5es(_) => Some("H5ES setlist"),
            Self::H5eb(_) => Some("H5EB bundle"),
            Self::PodxtLive(_) => None,
        }
    }
}

impl OpaqueH5epPreset {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_PRESET_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EP exceeds the offline file-size ceiling",
            ));
        }
        validate_h5ep_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes[..H5EP_HEADER_BYTES]
    }

    /// Raw H5EP header words at offsets 0x10 and 0x14. The Mac Editor 2.27
    /// writer sources them from `Memory + 0x18/+0x1c`, but a file alone does
    /// not establish which editor wrote them or whether they identify a
    /// physical target.
    pub fn header_words_10_14(&self) -> (u32, u32) {
        (
            u32::from_be_bytes(self.bytes[0x10..0x14].try_into().unwrap()),
            u32::from_be_bytes(self.bytes[0x14..0x18].try_into().unwrap()),
        )
    }

    pub fn body_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES]
    }

    /// Returns the editor's in-memory byte order using the statically traced
    /// selective word transform. Unmapped bytes are copied verbatim.
    pub fn body_in_editor_order(&self) -> Vec<u8> {
        let mut body = self.body_bytes().to_vec();
        transform_hd_body_words(&mut body);
        body
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_MIN_BYTES..]
    }

    /// The Edit 2.27 static source maps this control to a normalized float in
    /// the first matching effect row. The result is editor data, not a claim
    /// of physical-device availability.
    pub fn screamer_drive_percent(&self, effect_slot: usize) -> Option<u8> {
        self.screamer_control_percent(effect_slot, ScreamerControl::Drive)
    }

    /// Reads a statically joined Screamer type-1 control from its first keyed
    /// row when the row assignment state is admitted.
    pub fn screamer_control_percent(
        &self,
        effect_slot: usize,
        control: ScreamerControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::Screamer(control))
    }

    pub fn hd_type1_control_percent(
        &self,
        effect_slot: usize,
        control: HdType1Control,
    ) -> Option<u8> {
        let value_at = self.hd_type1_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_drive_percent(value)
    }

    /// Reads a selected source-joined delay-family type-1 control.
    pub fn hd_type1_mapped_control_percent(
        &self,
        effect_slot: usize,
        control: HdType1MappedControl,
    ) -> Option<u8> {
        let value_at = self.hd_type1_mapped_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_drive_percent(value)
    }

    /// Returns a selected non-type-1 control's canonical editor integer value.
    /// Values outside the exact normalized integer grid remain opaque.
    pub fn hd_numeric_control_value(
        &self,
        effect_slot: usize,
        control: HdNumericControl,
    ) -> Option<i32> {
        let value_at = self.hd_numeric_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_numeric_value(value, control.min, control.max)
    }

    /// Reads a statically joined type-9 choice control. Unknown finite
    /// and non-finite words remain observable without being normalized.
    pub fn hd_type9_choice_value(&self, effect_slot: HdFxSlot) -> HdType9ChoiceValue {
        let Ok(value_at) = self.hd_type9_choice_value_offset(effect_slot) else {
            return HdType9ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType9ChoiceValue::Unavailable;
        };
        match bits {
            0x0000_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice0),
            0x3f00_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice1),
            0x3f80_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice2),
            _ if f32::from_bits(bits).is_finite() => HdType9ChoiceValue::UnknownFinite(bits),
            _ => HdType9ChoiceValue::NonFinite(bits),
        }
    }

    /// Writes one explicit type-9 choice into exactly one existing H5EP row.
    /// All admission guards run before the byte-identical no-op check.
    pub fn set_hd_type9_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType9Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type9_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-9 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-9 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }

        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Writes one explicit type-8 choice into exactly one existing H5EP row.
    /// The two-choice API is distinct from type 9 and cannot accept Choice2.
    /// All admission guards run before the byte-identical no-op check.
    pub fn set_hd_type8_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType8Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type8_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-8 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-8 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }

        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Reads the exact type-8 row without normalizing unknown or non-finite
    /// imported words.
    pub fn hd_type8_choice_value(&self, effect_slot: HdFxSlot) -> HdType8ChoiceValue {
        let Ok(value_at) = self.hd_type8_choice_value_offset(effect_slot) else {
            return HdType8ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType8ChoiceValue::Unavailable;
        };
        match bits {
            0x0000_0000 => HdType8ChoiceValue::Choice(HdType8Choice::Choice0),
            0x3f80_0000 => HdType8ChoiceValue::Choice(HdType8Choice::Choice1),
            _ if f32::from_bits(bits).is_finite() => HdType8ChoiceValue::UnknownFinite(bits),
            _ => HdType8ChoiceValue::NonFinite(bits),
        }
    }

    pub fn hd_type10_choice_value(&self, effect_slot: HdFxSlot) -> HdType10ChoiceValue {
        let Ok(value_at) = self.hd_type10_choice_value_offset(effect_slot) else {
            return HdType10ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType10ChoiceValue::Unavailable;
        };
        match bits {
            0 => HdType10ChoiceValue::Choice(HdType10Choice::Choice0),
            0x3f80_0000 => HdType10ChoiceValue::Choice(HdType10Choice::Choice1),
            _ if f32::from_bits(bits).is_finite() => HdType10ChoiceValue::UnknownFinite(bits),
            _ => HdType10ChoiceValue::NonFinite(bits),
        }
    }

    pub fn set_hd_type10_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType10Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type10_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-10 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-10 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    fn hd_type10_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type10_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-10 choice requires an admitted model in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-10 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-10 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-10 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-10 choice row is missing"
            } else {
                "HD type-10 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-10 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-10 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn hd_type11_choice_value(
        &self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
    ) -> HdType11ChoiceValue {
        let Ok(value_at) = self.hd_type11_choice_value_offset(effect_slot, control) else {
            return HdType11ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType11ChoiceValue::Unavailable;
        };
        if let Some(choice) = HdType11Choice::ALL
            .into_iter()
            .find(|choice| choice.bits() == bits)
        {
            HdType11ChoiceValue::Choice(choice)
        } else if f32::from_bits(bits).is_finite() {
            HdType11ChoiceValue::UnknownFinite(bits)
        } else {
            HdType11ChoiceValue::NonFinite(bits)
        }
    }

    pub fn set_hd_type11_choice(
        &mut self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
        choice: HdType11Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type11_choice_value_offset(effect_slot, control)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-11 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-11 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    fn hd_type11_choice_value_offset(
        &self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
    ) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        if model_id != control.model_id()
            || !HD_TYPE11_CHOICE_MODEL_KEYS.contains(&(control.model_id(), control.key()))
        {
            return Err(ToneError::format(
                "HD type-11 choice requires the exact admitted model and control key in the selected FX slot",
            ));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-11 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-11 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-11 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(control.key()) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-11 choice row is missing"
            } else {
                "HD type-11 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-11 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-11 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    fn hd_type8_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type8_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-8 choice requires model 0x020a0013 in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-8 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-8 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-8 choice requires an effect row count in 1..=12",
            ));
        }

        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-8 choice row is missing"
            } else {
                "HD type-8 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-8 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-8 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    fn hd_type9_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type9_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-9 choice requires an admitted model in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-9 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-9 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-9 choice requires an effect row count in 1..=12",
            ));
        }

        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-9 choice row is missing"
            } else {
                "HD type-9 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-9 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-9 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn effect_model_id(&self, effect_slot: usize) -> Option<u32> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        read_be_u32(self.body_bytes(), block)
    }

    /// Reads the source-traced enable flag for an admitted existing HD effect.
    /// The on-disk byte is interpreted as a boolean; this does not claim that
    /// the file is applicable to a connected product.
    pub fn effect_enabled(&self, effect_slot: HdFxSlot) -> Option<bool> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block)?;
        (body[block + 0x0b] == 1 && is_admitted_hd_enable_model(model_id))
            .then_some(body[block + 8] != 0)
    }

    /// Changes only the existing effect's source-traced enable byte. Nonzero
    /// values read as enabled, but writes require a canonical stored byte so
    /// malformed values are never silently normalized.
    pub fn set_effect_enabled(
        &mut self,
        effect_slot: HdFxSlot,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block)
            .ok_or_else(|| ToneError::format("truncated HD effect model id"))?;
        if model_id == HD_EMPTY_EFFECT_MODEL_ID {
            return Err(ToneError::format(
                "cannot enable or disable an empty HD effect slot",
            ));
        }
        if !is_admitted_hd_enable_model(model_id) {
            return Err(ToneError::format(format!(
                "HD effect model {model_id:08x} has no admitted enable contract"
            )));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD effect enable requires a valid, non-empty effect slot",
            ));
        }
        if body[block + 8] > 1 {
            return Err(ToneError::format(
                "HD effect enable requires a canonical 0 or 1 byte",
            ));
        }
        if (body[block + 8] != 0) == enabled {
            return Ok(());
        }

        // Stage the single-byte write and validate the complete record before
        // publishing it, so a failure cannot leave a partial mutation.
        let mut candidate = self.bytes.clone();
        candidate[H5EP_HEADER_BYTES + block + 8] = u8::from(enabled);
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Replaces one existing Octave Fuzz with the source-derived Vintage Pre
    /// default prefix. This pure file operation has no vendor runtime context
    /// and makes no claim about physical target applicability.
    pub fn replace_octave_fuzz_with_vintage_pre_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_VINTAGE_PRE_MODEL_ID,
            HD_VINTAGE_PRE_DEFAULT_KEYS,
            "replacement",
        )
    }

    /// Replaces one existing Octave Fuzz with the source-derived Screamer
    /// default prefix. This remains a typed, standalone file operation.
    pub fn replace_octave_fuzz_with_screamer_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_SCREAMER_MODEL_ID,
            HD_SCREAMER_DEFAULT_KEYS,
            "Screamer",
        )
    }

    /// Replaces one existing Octave Fuzz with the source-derived Sub Octave
    /// Fuzz default prefix. This remains a typed, standalone file operation.
    pub fn replace_octave_fuzz_with_sub_octave_fuzz_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_SUB_OCTAVE_FUZZ_MODEL_ID,
            HD_SUB_OCTAVE_FUZZ_DEFAULT_KEYS,
            "Sub Octave Fuzz",
        )
    }

    /// Applies one member of the exact finite distortion/fuzz default set
    /// admitted for this explicit standalone replacement.
    pub fn replace_octave_fuzz_with_admitted_defaults(
        &mut self,
        effect_slot: HdFxSlot,
        target: HdDistortionDefault,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            target.model_id(),
            HD_SCREAMER_DEFAULT_KEYS,
            target.label(),
        )
    }

    fn replace_octave_fuzz_with_default_prefix(
        &mut self,
        effect_slot: HdFxSlot,
        target_model_id: u32,
        default_keys: [u32; 5],
        target_label: &str,
    ) -> Result<(), ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let prefix_error = format!(
            "{target_label} replacement requires each default key exactly once in the first five rows"
        );
        let format_error = |message: String| ToneError::format(message);

        if read_be_u32(body, block) != Some(HD_OCTAVE_FUZZ_MODEL_ID) {
            return Err(format_error(format!(
                "{target_label} replacement requires an existing Octave Fuzz model"
            )));
        }
        if body[block + 0x0b] != 1 {
            return Err(format_error(format!(
                "{target_label} replacement requires a valid, non-empty effect slot"
            )));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(format_error(format!(
                "{target_label} replacement requires a canonical effect enable byte"
            )));
        }
        if !(5..=HD_PARAMETER_ROW_COUNT as u8).contains(&body[block + 0x0f]) {
            return Err(format_error(format!(
                "{target_label} replacement requires an effect row count in 5..=12"
            )));
        }

        // The source Octave Fuzz must have these five keys exactly once, in any
        // order. Target labels may differ; this operation applies defaults.
        let mut seen = [false; 5];
        for row in 0..5 {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            let key = read_be_u32(body, row_at)
                .ok_or_else(|| ToneError::format("truncated HD effect parameter prefix"))?;
            let Some(key_index) = HD_SCREAMER_DEFAULT_KEYS
                .iter()
                .position(|expected| *expected == key)
            else {
                return Err(format_error(prefix_error.clone()));
            };
            if std::mem::replace(&mut seen[key_index], true) {
                return Err(format_error(prefix_error.clone()));
            }
        }
        if seen.iter().any(|present| !present) {
            return Err(format_error(prefix_error));
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let marker_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10;
            if !matches!(body[marker_at], 0..=2) {
                return Err(ToneError::format(format!(
                    "{target_label} replacement requires canonical effect assignment marker bytes"
                )));
            }
        }

        // Stage every write in a candidate and reparse the complete record
        // before changing the original object.
        let mut candidate = self.bytes.clone();
        let file_block = H5EP_HEADER_BYTES + block;
        candidate[file_block..file_block + 4].copy_from_slice(&target_model_id.to_be_bytes());
        for (row, key) in default_keys.into_iter().enumerate() {
            let row_at = file_block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            candidate[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            candidate[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        candidate[file_block + 0x0f] = default_keys.len() as u8;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let marker_at =
                file_block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10;
            candidate[marker_at] = 0;
        }
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    pub fn tube_drive_control_percent(
        &self,
        effect_slot: usize,
        control: TubeDriveControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::TubeDrive(control))
    }

    pub fn octave_fuzz_control_percent(
        &self,
        effect_slot: usize,
        control: OctaveFuzzControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::OctaveFuzz(control))
    }

    /// Edits only the first statically identified Screamer DRIVE value word.
    /// Unknown rows, duplicates and every other source byte remain unchanged.
    pub fn set_screamer_drive_percent(
        &mut self,
        effect_slot: usize,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_screamer_control_percent(effect_slot, ScreamerControl::Drive, percent)
    }

    /// Edits only the first matching keyed value for an admitted Screamer
    /// control. All unrelated rows and bytes are preserved.
    pub fn set_screamer_control_percent(
        &mut self,
        effect_slot: usize,
        control: ScreamerControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::Screamer(control), percent)
    }

    pub fn set_tube_drive_control_percent(
        &mut self,
        effect_slot: usize,
        control: TubeDriveControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::TubeDrive(control), percent)
    }

    pub fn set_octave_fuzz_control_percent(
        &mut self,
        effect_slot: usize,
        control: OctaveFuzzControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::OctaveFuzz(control), percent)
    }

    pub fn set_hd_type1_control_percent(
        &mut self,
        effect_slot: usize,
        control: HdType1Control,
        percent: u8,
    ) -> Result<(), ToneError> {
        if percent > 100 {
            return Err(ToneError::format(format!(
                "{} {} must be in 0..=100",
                control.model_label(),
                control.label()
            )));
        }
        let value_at = self
            .hd_type1_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD type-1 control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("Screamer DRIVE row is truncated"))?,
        );
        exact_drive_percent(current).ok_or_else(|| {
            ToneError::format(
                "existing HD control value is outside the exact recovered editor encoding",
            )
        })?;
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4]
            .copy_from_slice(&(percent as f32 / 100.0).to_bits().to_be_bytes());
        Ok(())
    }

    /// Edits only the first matching delay-family key's normalized value.
    pub fn set_hd_type1_mapped_control_percent(
        &mut self,
        effect_slot: usize,
        control: HdType1MappedControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        if percent > 100 {
            return Err(ToneError::format(format!(
                "{} {} must be in 0..=100",
                control.model_label, control.label
            )));
        }
        let value_at = self
            .hd_type1_mapped_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD mapped type-1 control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("HD mapped type-1 row is truncated"))?,
        );
        exact_drive_percent(current).ok_or_else(|| {
            ToneError::format("existing HD mapped type-1 value is outside the editor percent grid")
        })?;
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4]
            .copy_from_slice(&(percent as f32 / 100.0).to_bits().to_be_bytes());
        Ok(())
    }

    /// Edits one selected non-type-1 numeric widget value. The stored float
    /// must already be on the exact source-configured integer grid; only its
    /// keyed value word changes.
    pub fn set_hd_numeric_control_value(
        &mut self,
        effect_slot: usize,
        control: HdNumericControl,
        value: i32,
    ) -> Result<(), ToneError> {
        if !(control.min..=control.max).contains(&value) {
            return Err(ToneError::format(format!(
                "{} {} must be in {}..={}",
                control.model_label, control.label, control.min, control.max
            )));
        }
        let value_at = self
            .hd_numeric_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD numeric control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("HD numeric row is truncated"))?,
        );
        exact_numeric_value(current, control.min, control.max).ok_or_else(|| {
            ToneError::format(
                "existing HD numeric value is outside the exact recovered editor grid",
            )
        })?;
        let normalized = normalized_numeric_value(value, control.min, control.max);
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4].copy_from_slice(&normalized.to_bits().to_be_bytes());
        Ok(())
    }

    /// Exact no-op serialization. The envelope's unknown fields remain raw.
    pub fn serialize_noop(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }

    fn hd_type1_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdType1Control,
    ) -> Option<usize> {
        self.hd_type1_value_offset(effect_slot, control.model_id(), control.key())
    }

    fn hd_type1_mapped_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdType1MappedControl,
    ) -> Option<usize> {
        self.hd_type1_value_offset(effect_slot, control.model_id, control.key)
    }

    fn hd_type1_value_offset(&self, effect_slot: usize, model_id: u32, key: u32) -> Option<usize> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        if read_be_u32(body, block)? != model_id {
            return None;
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at)? == key {
                // The source sends the row flag into widget assignment state,
                // while the callback suppresses writes for expression controls.
                // Only the zero-flag state is admitted; auxiliary flag bytes
                // remain opaque and are never normalized.
                if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
                    return None;
                }
                return Some(row_at + 4);
            }
        }
        None
    }

    fn hd_numeric_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdNumericControl,
    ) -> Option<usize> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        if read_be_u32(body, block)? != control.model_id {
            return None;
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at)? == control.key() {
                if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
                    return None;
                }
                return Some(row_at + 4);
            }
        }
        None
    }
}

impl H5esSetlist {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_SETLIST_BYTES_CEILING {
            return Err(ToneError::format(
                "H5ES exceeds the offline file-size ceiling",
            ));
        }
        validate_h5es_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5ES_MIN_BYTES..]
    }

    pub fn preset_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5ES_PRESET_COUNT {
            return None;
        }
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        self.bytes.get(start..start + H5EP_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

impl H5ebBundle {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EB exceeds the offline file-size ceiling",
            ));
        }
        validate_h5eb_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EB_MIN_BYTES..]
    }

    pub fn setlist_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5EB_SETLIST_COUNT {
            return None;
        }
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        self.bytes.get(start..start + H5ES_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredHdFile {
    file: PodPresetFile,
    saved: Vec<u8>,
}

/// Shared in-memory library for statically framed HD Edit files. Imported
/// containers and their source bytes are retained independently; extracting a
/// child creates a separate preset entry and never rewrites its parent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HdEditorLibrary {
    files: Vec<StoredHdFile>,
}

impl HdEditorLibrary {
    /// Applies an admitted type-9 choice to an extracted standalone H5EP.
    /// H5ES/H5EB parent bytes remain independently preserved and unchanged.
    pub fn set_type9_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType9Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => preset.set_hd_type9_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-9 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the exact admitted type-8 choice to an extracted standalone
    /// H5EP. H5ES/H5EB parent bytes remain independently preserved.
    pub fn set_type8_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType8Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => preset.set_hd_type8_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-8 choice requires an extracted H5EP preset",
            )),
        }
    }

    pub fn set_type10_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType10Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => preset.set_hd_type10_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-10 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one explicitly keyed type-11 choice to an extracted H5EP.
    /// Same-model controls remain independent and parent containers unchanged.
    pub fn set_type11_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        control: HdType11Control,
        choice: HdType11Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => {
                preset.set_hd_type11_choice(effect_slot, control, choice)
            }
            _ => Err(ToneError::format(
                "HD type-11 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies an enable flag to one standalone H5EP item. Extract a child
    /// first for H5ES/H5EB; the parents remain untouched in the library.
    pub fn set_effect_enabled(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => preset.set_effect_enabled(effect_slot, enabled),
            _ => Err(ToneError::format(
                "HD effect enable requires an extracted H5EP preset",
            )),
        }
    }

    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let file = PodPresetFile::parse(bytes)?;
        if file.hd_container_kind().is_none() {
            return Err(ToneError::format(
                "HD editor library accepts H5EP, H5ES, and H5EB files only",
            ));
        }
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn file(&self, index: usize) -> Option<&PodPresetFile> {
        self.files.get(index).map(|entry| &entry.file)
    }

    pub fn file_mut(&mut self, index: usize) -> Option<&mut PodPresetFile> {
        self.files.get_mut(index).map(|entry| &mut entry.file)
    }

    pub fn extract_preset(
        &mut self,
        index: usize,
        setlist: usize,
        preset: usize,
    ) -> Result<usize, ToneError> {
        let child = match self.file(index) {
            Some(PodPresetFile::H5ep(_)) if setlist == 0 && preset == 0 => {
                return Ok(index);
            }
            Some(PodPresetFile::H5es(file)) if setlist == 0 => file.preset_bytes(preset),
            Some(PodPresetFile::H5eb(file)) => file.setlist_bytes(setlist).and_then(|bytes| {
                let start = H5ES_HEADER_BYTES + preset.checked_mul(H5EP_MIN_BYTES)?;
                bytes.get(start..start.checked_add(H5EP_MIN_BYTES)?)
            }),
            _ => None,
        }
        .ok_or_else(|| ToneError::format("setlist or preset index is outside the file"))?
        .to_vec();
        let file = PodPresetFile::H5ep(OpaqueH5epPreset::parse(child)?);
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    /// Applies the one admitted model-default replacement to a standalone
    /// H5EP item. For H5ES/H5EB parents, extract a child first; their bytes are
    /// retained unchanged as separate library items.
    pub fn replace_octave_fuzz_with_vintage_pre_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_vintage_pre_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Vintage Pre replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the bounded Screamer defaults replacement to a standalone H5EP
    /// item. Extract H5ES/H5EB children first; their parents remain untouched.
    pub fn replace_octave_fuzz_with_screamer_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_screamer_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Screamer replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the bounded Sub Octave Fuzz defaults replacement to a
    /// standalone H5EP item. Extract H5ES/H5EB children first; parents remain
    /// untouched.
    pub fn replace_octave_fuzz_with_sub_octave_fuzz_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_sub_octave_fuzz_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Sub Octave Fuzz replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one admitted closed-set distortion/fuzz default to an extracted
    /// H5EP item. H5ES/H5EB parents remain separate and unchanged.
    pub fn replace_octave_fuzz_with_admitted_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        target: HdDistortionDefault,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            PodPresetFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_admitted_defaults(effect_slot, target)
            }
            _ => Err(ToneError::format(
                "admitted defaults replacement requires an extracted H5EP preset",
            )),
        }
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.files
            .get(index)
            .is_some_and(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn any_dirty(&self) -> bool {
        self.files
            .iter()
            .any(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn dirty_count(&self) -> usize {
        self.files
            .iter()
            .filter(|entry| entry.file.as_bytes() != entry.saved)
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.files
            .iter()
            .position(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        entry.saved = entry.file.as_bytes().to_vec();
        Ok(())
    }

    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        if entry.file.as_bytes() != entry.saved {
            entry.file = PodPresetFile::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}

fn validate_h5ep_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EP_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EP record: expected at least {H5EP_MIN_BYTES} bytes"
        )));
    }
    if &bytes[..4] != b"H5EP" {
        return Err(ToneError::format("expected H5EP preset magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EP version"));
    }
    validate_hd_body(&bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES])
}

fn validate_h5es_header(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5ES_HEADER_BYTES {
        return Err(ToneError::format("truncated H5ES header"));
    }
    if &bytes[..4] != b"H5ES" {
        return Err(ToneError::format("expected H5ES setlist magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5ES version"));
    }
    Ok(())
}

fn validate_h5es_record(bytes: &[u8]) -> Result<(), ToneError> {
    validate_h5es_header(bytes)?;
    if bytes.len() < H5ES_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5ES children: expected at least {H5ES_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5ES_PRESET_COUNT {
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        validate_h5ep_record(&bytes[start..start + H5EP_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_h5eb_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EB_HEADER_BYTES {
        return Err(ToneError::format("truncated H5EB header"));
    }
    if &bytes[..4] != b"H5EB" {
        return Err(ToneError::format("expected H5EB bundle magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EB version"));
    }
    if bytes.len() < H5EB_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EB children: expected at least {H5EB_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5EB_SETLIST_COUNT {
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        validate_h5es_record(&bytes[start..start + H5ES_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_hd_body(body: &[u8]) -> Result<(), ToneError> {
    if body.len() != H5EP_BODY_BYTES {
        return Err(ToneError::format("invalid H5EP body length"));
    }
    for index in 0..2 {
        if body[0x028 + index * HD_EFFECT_BLOCK_STRIDE] != 0 {
            return Err(ToneError::format("invalid H5EP amp model class"));
        }
        if body[0x228 + index * HD_EFFECT_BLOCK_STRIDE] != 1 {
            return Err(ToneError::format("invalid H5EP cab model class"));
        }
    }
    let mut loop_count = 0;
    for index in 0..HD_EFFECT_COUNT {
        let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
        if body[offset] != 2 {
            return Err(ToneError::format("invalid H5EP effect model class"));
        }
        if read_be_u32(body, offset) == Some(0x0208_0000) {
            loop_count += 1;
        }
    }
    if loop_count >= 2 {
        return Err(ToneError::format("H5EP contains two FX Loop models"));
    }
    for (offset, maximum, field) in [
        (0xff3, 1, "TrailsEnabled"),
        (0xff6, 1, "AmpControl"),
        (0xff8, 7, "Mic[0]"),
        (0xff9, 7, "Mic[1]"),
        (0xffa, 1, "MainMute[0]"),
        (0xffb, 1, "MainMute[1]"),
        (0xffe, 10, "Input1Source"),
        (0xfff, 10, "Input2Source"),
    ] {
        if body[offset] > maximum {
            return Err(ToneError::format(format!("invalid H5EP {field} value")));
        }
    }
    Ok(())
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Enable editing is bounded to model ids already present in a source-joined
/// typed control table. This is intentionally not the editor's complete model
/// menu and cannot assign a model or infer slot applicability.
fn is_admitted_hd_enable_model(model_id: u32) -> bool {
    HdType1Control::ALL
        .into_iter()
        .any(|control| control.model_id() == model_id)
        || HD_TYPE1_MAPPED_CONTROLS
            .iter()
            .any(|control| control.model_id() == model_id)
        || HD_NUMERIC_CONTROLS
            .iter()
            .any(|control| control.model_id() == model_id)
}

fn exact_drive_percent(value: f32) -> Option<u8> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let percent = (value * 100.0).round() as u8;
    ((percent as f32 / 100.0).to_bits() == value.to_bits()).then_some(percent)
}

fn normalized_numeric_value(value: i32, min: i32, max: i32) -> f32 {
    (value - min) as f32 / (max - min) as f32
}

fn exact_numeric_value(value: f32, min: i32, max: i32) -> Option<i32> {
    if min >= max || !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let width = (max - min) as f32;
    let integer = min + (value * width).round() as i32;
    (normalized_numeric_value(integer, min, max).to_bits() == value.to_bits()).then_some(integer)
}

fn transform_hd_body_words(body: &mut [u8]) {
    const FIXED_WORDS: [usize; 9] = [0x20, 0x24, 0xd48, 0xd4c, 0xd50, 0xd54, 0xd58, 0xd7c, 0xd80];
    for offset in FIXED_WORDS {
        body[offset..offset + 4].reverse();
    }
    const BLOCKS: [usize; 4] = [0x28, 0x128, 0x228, 0x328];
    for offset in BLOCKS.into_iter().chain(
        (0..HD_EFFECT_COUNT).map(|index| HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE),
    ) {
        for word in 0..2 {
            let at = offset + word * 4;
            body[at..at + 4].reverse();
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = offset + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            for word in 0..4 {
                let at = row_at + word * 4;
                body[at..at + 4].reverse();
            }
        }
    }
}

impl ToneError {
    fn format(detail: impl Into<String>) -> Self {
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

fn read_parameters(bytes: &[u8], fields: &[preset::Chunk]) -> Vec<ModelParameter> {
    fields
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
        .collect()
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
            models.push(ModelSlot {
                slot,
                model_id: preset::word(&bytes, minf.data.start),
                ordinal,
                reserved,
                enabled,
                parameters: read_parameters(&bytes, &fields),
            });
        }
        Ok(Self {
            bytes,
            name_offset,
            name,
            device_id,
            device_version,
            container,
            models,
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

    /// Asset name for the amp model id, when the PODxt Live table has one.
    pub fn amp_asset_name(&self) -> Option<&'static str> {
        self.amp_model_id().and_then(pod_assets::asset_name)
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

/// Opens an input only when the opened handle itself names a regular file.
/// Callers also check the path facts first so non-file paths are not opened.
pub fn open_regular_file(path: &Path) -> std::io::Result<std::fs::File> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input must be a regular file",
        ));
    }
    Ok(file)
}

/// Reads only after `facts` names a regular file at or under the ceiling.
///
/// A pipe, socket, or device node is refused first, including when its
/// reported length is zero. `read` is not called in that case.
pub fn read_regular_tone<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
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
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        TONE_BYTES_CEILING,
        "tone file exceeds the 1 MiB offline librarian ceiling",
    )
}

/// Reads a regular offline `.5xe`/`.5xs`/`.5xb` candidate under the largest
/// bounded format size. Device nodes, pipes, and oversized files are refused
/// before the reader closure is called.
pub fn read_regular_hd_editor_file<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "HD editor input must be a regular file; non-file paths are not opened",
        ));
    }
    if facts.len > HD_BUNDLE_BYTES_CEILING as u64 {
        return Err(ToneError::format(
            "HD editor file exceeds the offline file-size ceiling",
        ));
    }
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        HD_BUNDLE_BYTES_CEILING as u64,
        "HD editor file exceeds the offline file-size ceiling",
    )
}

fn read_bounded_file<R: Read>(
    mut reader: R,
    ceiling: u64,
    too_large: &str,
) -> Result<Vec<u8>, ToneError> {
    let capacity = usize::try_from(
        ceiling
            .checked_add(1)
            .ok_or_else(|| ToneError::format("offline file-size ceiling overflow"))?,
    )
    .map_err(|_| ToneError::format("offline file-size ceiling is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0u8; 8192];
    loop {
        let remaining = capacity - bytes.len();
        if remaining == 0 {
            break;
        }
        let read_len = buffer.len().min(remaining);
        let count = reader
            .read(&mut buffer[..read_len])
            .map_err(|error| ToneError::format(error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 > ceiling {
        return Err(ToneError::format(too_large));
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
    let mut info = vec![0; 76];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
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
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Cursor;
    use std::rc::Rc;

    struct ContinuousReader(Rc<Cell<usize>>);

    impl Read for ContinuousReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(0x5a);
            self.0.set(self.0.get() + buffer.len());
            Ok(buffer.len())
        }
    }

    fn synthetic_h5ep(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; H5EP_MIN_BYTES];
        bytes[..4].copy_from_slice(b"H5EP");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        let body = &mut bytes[H5EP_HEADER_BYTES..];
        for offset in [0x228, 0x328] {
            body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
        }
        for index in 0..HD_EFFECT_COUNT {
            let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
            body[offset..offset + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    fn synthetic_octave_fuzz_h5ep(effect_slot: usize, with_tail: bool) -> Vec<u8> {
        let mut bytes = synthetic_h5ep(if with_tail { b"private-tail" } else { b"" });
        let block =
            H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        bytes[block..block + 4].copy_from_slice(&HD_OCTAVE_FUZZ_MODEL_ID.to_be_bytes());
        bytes[block + 4..block + 8].copy_from_slice(&0x1122_3344u32.to_be_bytes());
        bytes[block + 8] = (effect_slot % 2) as u8;
        bytes[block + 9] = 0x5a;
        bytes[block + 10] = 0xa5;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 8;

        let permutation = [
            HD_TYPE1_OUTPUT_KEY,
            HD_TYPE1_DRIVE_KEY,
            HD_TYPE1_THIRD_KEY,
            HD_TYPE1_BASS_KEY,
            HD_TYPE1_TREBLE_KEY,
        ];
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            let key = if row < permutation.len() {
                permutation[row]
            } else if row == 6 {
                HD_TYPE1_OUTPUT_KEY
            } else {
                0x6abc_0000 + row as u32
            };
            bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            bytes[row_at + 4..row_at + 8]
                .copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
            bytes[row_at + 8..row_at + 12]
                .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
            bytes[row_at + 12..row_at + 16]
                .copy_from_slice(&(0x7200_0000u32 + row as u32).to_be_bytes());
            bytes[row_at + 0x10] = (row % 3) as u8;
            bytes[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
                0xc1 + row as u8,
                0xd1 + row as u8,
                0xe1 + row as u8,
            ]);
        }
        bytes
    }

    fn synthetic_type9_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
        synthetic_type9_h5ep_for(
            slot,
            row,
            enabled,
            value,
            HD_TYPE9_CHOICE_MODEL_ID,
            0x3f10_0004,
        )
    }

    fn synthetic_type9_h5ep_for(
        slot: usize,
        row: usize,
        enabled: u8,
        value: u32,
        model_id: u32,
        key: u32,
    ) -> Vec<u8> {
        let mut bytes = synthetic_h5ep(b"type9-tail");
        bytes[8..H5EP_HEADER_BYTES].fill(0x6d);
        bytes[H5EP_HEADER_BYTES + 0x040..H5EP_HEADER_BYTES + 0x050].fill(0x5a);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 8] = enabled;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].copy_from_slice(&[0x31; 8]);
        bytes[row_at + 0x10..row_at + HD_PARAMETER_ROW_STRIDE]
            .copy_from_slice(&[0, 0x42, 0x43, 0x44]);
        for other_slot in 0..HD_EFFECT_COUNT {
            if other_slot != slot {
                let other = H5EP_HEADER_BYTES
                    + HD_EFFECT_BLOCK_OFFSET
                    + other_slot * HD_EFFECT_BLOCK_STRIDE;
                bytes[other + 4..other + 8].fill(0x73);
            }
        }
        for other_row in 0..HD_PARAMETER_ROW_COUNT {
            if other_row != row {
                let other = block + HD_EFFECT_ROWS_OFFSET + other_row * HD_PARAMETER_ROW_STRIDE;
                bytes[other + 8..other + 0x10].fill(0x24 + other_row as u8);
                bytes[other + 0x11..other + HD_PARAMETER_ROW_STRIDE].fill(0x44 + other_row as u8);
            }
        }
        bytes
    }

    fn synthetic_type8_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
        let mut bytes = synthetic_h5ep(b"type8-tail");
        // Keep this procedural matrix builder independent of the type-9
        // fixture builder; the fixed full-file vectors below are the oracle.
        let block = H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
        bytes[block..block + 4].copy_from_slice(&HD_TYPE8_CHOICE_MODEL_ID.to_be_bytes());
        bytes[block + 8] = enabled;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        let row_at = block + 0x10 + row * 0x14;
        bytes[row_at..row_at + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].copy_from_slice(&[0x31; 8]);
        bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
        bytes
    }

    fn synthetic_type10_h5ep_for(
        slot: usize,
        row: usize,
        enabled: u8,
        value: u32,
        model_id: u32,
        key: u32,
    ) -> Vec<u8> {
        let mut bytes = synthetic_h5ep(b"type10-tail");
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 8] = enabled;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].fill(0x35);
        bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
        bytes
    }

    fn synthetic_type10_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
        synthetic_type10_h5ep_for(
            slot,
            row,
            enabled,
            value,
            HD_TYPE10_CHOICE_MODEL_ID,
            HD_TYPE10_CHOICE_KEY,
        )
    }

    fn synthetic_type11_h5ep(
        slot: usize,
        target_row: usize,
        enabled: u8,
        model_id: u32,
        control: HdType11Control,
        value: u32,
    ) -> Vec<u8> {
        assert_eq!(control.model_id(), model_id);
        let mut bytes = synthetic_h5ep(b"type11-tail");
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 8] = enabled;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 12;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            bytes[row_at..row_at + 4].copy_from_slice(&(0x5500_0000 + row as u32).to_be_bytes());
            bytes[row_at + 4..row_at + 8]
                .copy_from_slice(&(0x3e00_0000 + row as u32).to_be_bytes());
            bytes[row_at + 8..row_at + 0x10].fill(0x61 + row as u8);
            bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x71, 0x72, 0x73]);
        }
        let controls = HdType11Control::for_model(model_id);
        assert_eq!(controls.len(), 2);
        let sibling = controls
            .iter()
            .copied()
            .find(|candidate| *candidate != control)
            .unwrap();
        let sibling_row = if target_row == 11 { 0 } else { 11 };
        for (row, selected_control, bits) in [
            (target_row, control, value),
            (sibling_row, sibling, 0x3eaa_1234),
        ] {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            bytes[row_at..row_at + 4].copy_from_slice(&selected_control.key().to_be_bytes());
            bytes[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn type11_choices_are_closed_typed_and_preserve_every_non_target_byte() {
        assert_eq!(HD_TYPE11_CHOICE_MODEL_KEYS.len(), 4);
        assert_eq!(HdType11Control::ALL.len(), 4);
        assert_eq!(HdType11Choice::ALL.len(), 5);
        for (index, choice) in HdType11Choice::ALL.into_iter().enumerate() {
            assert_eq!(HdType11Choice::from_index(index as u8), Ok(choice));
            assert_eq!(choice.index(), index as u8);
        }
        assert!(HdType11Choice::from_index(5).is_err());
        assert_eq!(
            HdType11Choice::ALL.map(HdType11Choice::bits),
            [
                0x0000_0000,
                0x3e80_0000,
                0x3f00_0000,
                0x3f40_0000,
                0x3f80_0000
            ]
        );
        for unknown in [0x020a_001c, 0x020a_001f, 0x020a_0020] {
            assert!(
                HdType11Control::ALL
                    .iter()
                    .all(|control| control.model_id() != unknown)
            );
        }

        for control in HdType11Control::ALL {
            for slot_index in 0..HD_EFFECT_COUNT {
                for target_row in [0, 5, 11] {
                    for enabled in [0, 1] {
                        for choice in HdType11Choice::ALL {
                            let original = synthetic_type11_h5ep(
                                slot_index,
                                target_row,
                                enabled,
                                control.model_id(),
                                control,
                                0x3eaa_4321,
                            );
                            let value_at = H5EP_HEADER_BYTES
                                + HD_EFFECT_BLOCK_OFFSET
                                + slot_index * HD_EFFECT_BLOCK_STRIDE
                                + HD_EFFECT_ROWS_OFFSET
                                + target_row * HD_PARAMETER_ROW_STRIDE
                                + 4;
                            let mut expected = original.clone();
                            expected[value_at..value_at + 4]
                                .copy_from_slice(&choice.bits().to_be_bytes());
                            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
                            let slot = HdFxSlot::new(slot_index).unwrap();
                            assert_eq!(
                                preset.hd_type11_choice_value(slot, control),
                                HdType11ChoiceValue::UnknownFinite(0x3eaa_4321)
                            );
                            preset.set_hd_type11_choice(slot, control, choice).unwrap();
                            assert_eq!(preset.as_bytes(), expected);
                            let same_word = preset.as_bytes().to_vec();
                            preset.set_hd_type11_choice(slot, control, choice).unwrap();
                            assert_eq!(preset.as_bytes(), same_word, "guarded same-word no-op");
                            assert_eq!(
                                preset.hd_type11_choice_value(slot, control),
                                HdType11ChoiceValue::Choice(choice)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn type11_strict_guards_precede_noop_and_reject_without_mutation() {
        for control in HdType11Control::ALL {
            let slot = HdFxSlot::new(5).unwrap();
            let source = synthetic_type11_h5ep(
                5,
                4,
                1,
                control.model_id(),
                control,
                HdType11Choice::Choice2.bits(),
            );
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
            let row_at = block + HD_EFFECT_ROWS_OFFSET + 4 * HD_PARAMETER_ROW_STRIDE;
            let sibling = HdType11Control::for_model(control.model_id())
                .iter()
                .copied()
                .find(|candidate| *candidate != control)
                .unwrap();
            let mut cases = Vec::new();
            let mut foreign_model = source.clone();
            foreign_model[block..block + 4].copy_from_slice(&0x020a_001c_u32.to_be_bytes());
            cases.push(("foreign model", foreign_model));
            let mut foreign_key = source.clone();
            foreign_key[row_at..row_at + 4].copy_from_slice(&sibling.key().to_be_bytes());
            cases.push(("wrong control key", foreign_key));
            let mut missing = source.clone();
            missing[row_at..row_at + 4].fill(0);
            cases.push(("missing target key", missing));
            let mut duplicate = source.clone();
            let late_row = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
            duplicate[late_row..late_row + 4].copy_from_slice(&control.key().to_be_bytes());
            duplicate[block + 0x0f] = 5;
            cases.push(("duplicate beyond declared count", duplicate));
            let mut beyond = source.clone();
            beyond[row_at..row_at + 4].fill(0);
            let row11 = block + HD_EFFECT_ROWS_OFFSET + 11 * HD_PARAMETER_ROW_STRIDE;
            beyond[row11..row11 + 4].copy_from_slice(&control.key().to_be_bytes());
            beyond[block + 0x0f] = 11;
            cases.push(("target beyond declared count", beyond));
            let mut assigned = source.clone();
            assigned[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            cases.push(("assigned target marker", assigned));
            for (offset, value, label) in [
                (0x0b, 0, "invalid effect marker"),
                (8, 2, "noncanonical enable"),
                (0x0f, 0, "zero row count"),
                (0x0f, 13, "excessive row count"),
            ] {
                let mut invalid = source.clone();
                invalid[block + offset] = value;
                cases.push((label, invalid));
            }
            for bits in [0x7fc0_0001_u32, 0x7f80_0000, 0xff80_0000] {
                let mut nonfinite = source.clone();
                nonfinite[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
                cases.push(("non-finite current value", nonfinite));
            }
            for (label, bytes) in cases {
                let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
                assert!(
                    preset
                        .set_hd_type11_choice(slot, control, HdType11Choice::Choice2)
                        .is_err(),
                    "{label}"
                );
                assert_eq!(preset.as_bytes(), bytes, "{label} must be atomic");
            }
            let mut malformed_noop = source.clone();
            malformed_noop[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            let mut preset = OpaqueH5epPreset::parse(malformed_noop.clone()).unwrap();
            assert!(
                preset
                    .set_hd_type11_choice(slot, control, HdType11Choice::Choice2)
                    .is_err()
            );
            assert_eq!(
                preset.as_bytes(),
                malformed_noop,
                "guards run before same-word no-op"
            );
        }
    }

    fn synthetic_enable_h5ep(effect_slot: usize, enabled_byte: u8) -> Vec<u8> {
        let mut bytes = synthetic_octave_fuzz_h5ep(effect_slot, true);
        let block =
            H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        bytes[block + 8] = enabled_byte;
        bytes
    }

    #[test]
    fn admitted_enable_edits_one_byte_for_every_effect_slot() {
        for effect_slot in 0..HD_EFFECT_COUNT {
            for requested in [false, true] {
                let old = u8::from(!requested);
                let original = synthetic_enable_h5ep(effect_slot, old);
                let mut expected = original.clone();
                let enable_at = H5EP_HEADER_BYTES
                    + HD_EFFECT_BLOCK_OFFSET
                    + effect_slot * HD_EFFECT_BLOCK_STRIDE
                    + 8;
                expected[enable_at] = u8::from(requested);

                let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                let slot = HdFxSlot::new(effect_slot).unwrap();
                assert_eq!(preset.effect_enabled(slot), Some(!requested));
                preset.set_effect_enabled(slot, requested).unwrap();
                assert_eq!(
                    preset.as_bytes(),
                    expected,
                    "slot {effect_slot}, {requested}"
                );
                assert_eq!(preset.effect_enabled(slot), Some(requested));
            }
        }
        assert!(HdFxSlot::new(HD_EFFECT_COUNT).is_err());
    }

    #[test]
    fn admitted_enable_noops_preserve_the_whole_file_and_noncanonical_writes_reject() {
        for raw in [0, 1] {
            let bytes = synthetic_enable_h5ep(3, raw);
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            preset
                .set_effect_enabled(HdFxSlot::new(3).unwrap(), raw != 0)
                .unwrap();
            assert_eq!(preset.as_bytes(), bytes, "canonical no-op for {raw:#04x}");
        }

        for raw in [0x02, 0x7f, 0x80, 0xff] {
            let bytes = synthetic_enable_h5ep(3, raw);
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert_eq!(preset.effect_enabled(HdFxSlot::new(3).unwrap()), Some(true));
            for requested in [false, true] {
                assert!(
                    preset
                        .set_effect_enabled(HdFxSlot::new(3).unwrap(), requested)
                        .is_err(),
                    "raw {raw:#04x}, requested {requested}"
                );
                assert_eq!(preset.as_bytes(), bytes, "raw {raw:#04x}");
            }
        }
    }

    #[test]
    fn admitted_enable_rejects_unknown_empty_invalid_and_unsupported_records_atomically() {
        let slot = HdFxSlot::new(0).unwrap();
        for (model, valid) in [
            (0x02ad_beef, 1),
            (HD_EMPTY_EFFECT_MODEL_ID, 1),
            (HD_SCREAMER_MODEL_ID, 0),
            (HD_SCREAMER_MODEL_ID, 2),
        ] {
            let mut bytes = synthetic_enable_h5ep(0, 0);
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            bytes[block..block + 4].copy_from_slice(&model.to_be_bytes());
            bytes[block + 0x0b] = valid;
            let original = bytes.clone();
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert!(preset.set_effect_enabled(slot, true).is_err());
            assert_eq!(
                preset.as_bytes(),
                original,
                "model {model:#010x}, valid {valid}"
            );
            assert_eq!(preset.effect_enabled(slot), None);
        }

        let mut bad_version = synthetic_enable_h5ep(0, 0);
        bad_version[7] = 2;
        assert!(OpaqueH5epPreset::parse(bad_version).is_err());
    }

    #[test]
    fn admitted_enable_dirty_state_returns_to_saved_baseline() {
        let mut library = HdEditorLibrary::default();
        let mut original = synthetic_enable_h5ep(0, 0);
        let other_block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_BLOCK_STRIDE;
        original[other_block..other_block + 4].copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        original[other_block + 8] = 0x7f;
        original[other_block + 0x0b] = 1;
        let index = library.import_bytes(original.clone()).unwrap();
        let slot = HdFxSlot::new(0).unwrap();
        library.set_effect_enabled(index, slot, true).unwrap();
        assert!(library.is_dirty(index));

        let dirty_bytes = library.file(index).unwrap().as_bytes().to_vec();
        let noncanonical_slot = HdFxSlot::new(1).unwrap();
        for requested in [false, true] {
            assert!(
                library
                    .set_effect_enabled(index, noncanonical_slot, requested)
                    .is_err()
            );
            assert_eq!(library.file(index).unwrap().as_bytes(), dirty_bytes);
            assert!(library.is_dirty(index));
        }

        library.set_effect_enabled(index, slot, false).unwrap();
        assert!(!library.is_dirty(index));
        assert!(!library.any_dirty());
        assert_eq!(library.file(index).unwrap().as_bytes(), original);
    }

    #[test]
    fn opaque_h5ep_dispatch_and_noop_round_trip_preserve_every_byte() {
        let mut original = synthetic_h5ep(b"uninterpreted trailing bytes");
        // Exercise noncanonical/unknown header and body values, including bytes
        // that resemble duplicated fields. No field is normalized or decoded.
        original[8..H5EP_HEADER_BYTES].fill(0xa5);
        original[H5EP_HEADER_BYTES + 0xd90..H5EP_HEADER_BYTES + 0xda0].fill(0x5a);
        original[0x0b] = 0x2e;
        original[0x13..0x16].copy_from_slice(&[0x06, 0x02, 0x62]);
        let file = PodPresetFile::parse(original.clone()).unwrap();
        let PodPresetFile::H5ep(envelope) = file else {
            panic!("H5EP must dispatch to the structured editor record");
        };
        assert_eq!(envelope.header_bytes().len(), H5EP_HEADER_BYTES);
        assert_eq!(envelope.body_bytes().len(), H5EP_BODY_BYTES);
        assert_eq!(envelope.trailing_bytes(), b"uninterpreted trailing bytes");
        assert_eq!(envelope.serialize_noop(), original);
    }

    #[test]
    fn opaque_h5ep_rejects_bad_magic_version_truncation_and_oversize() {
        let valid = synthetic_h5ep(&[]);
        for len in [0, 3, 4, H5EP_HEADER_BYTES - 1, H5EP_MIN_BYTES - 1] {
            assert!(
                OpaqueH5epPreset::parse(valid[..len].to_vec()).is_err(),
                "len {len}"
            );
        }
        let mut bad_magic = valid.clone();
        bad_magic[..4].copy_from_slice(b"H5ES");
        assert!(OpaqueH5epPreset::parse(bad_magic).is_err());
        let mut bad_version = valid;
        bad_version[7] = 2;
        assert!(OpaqueH5epPreset::parse(bad_version).is_err());
        assert!(OpaqueH5epPreset::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
    }

    #[test]
    fn octave_fuzz_replacement_rebuilds_the_ordered_prefix_for_every_fx_slot() {
        for effect_slot in 0..HD_EFFECT_COUNT {
            let original = synthetic_octave_fuzz_h5ep(effect_slot, true);
            let mut expected = original.clone();
            let block =
                H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
            expected[block..block + 4].copy_from_slice(&HD_VINTAGE_PRE_MODEL_ID.to_be_bytes());
            for (row, key) in HD_VINTAGE_PRE_DEFAULT_KEYS.into_iter().enumerate() {
                let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
            }
            expected[block + 0x0f] = 5;
            for row in 0..HD_PARAMETER_ROW_COUNT {
                expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
            }

            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            preset
                .replace_octave_fuzz_with_vintage_pre_defaults(HdFxSlot::new(effect_slot).unwrap())
                .unwrap();
            assert_eq!(preset.as_bytes(), expected, "FX slot {effect_slot}");
            assert_eq!(preset.trailing_bytes(), b"private-tail");
            assert_eq!(
                preset.effect_model_id(effect_slot),
                Some(HD_VINTAGE_PRE_MODEL_ID)
            );

            let row6 = block + HD_EFFECT_ROWS_OFFSET + 6 * HD_PARAMETER_ROW_STRIDE;
            assert_eq!(
                &preset.as_bytes()[row6..row6 + 8],
                &original[row6..row6 + 8]
            );
            assert_eq!(
                &preset.as_bytes()[block + 4..block + 0x0b],
                &original[block + 4..block + 0x0b]
            );
        }
        assert!(HdFxSlot::new(8).is_err());
    }

    #[test]
    fn octave_fuzz_to_screamer_uses_literal_defaults_and_preserves_the_bounded_write_set() {
        const SCREAMER_ID: u32 = 0x0205_000e;
        const EXPECTED_KEYS: [u32; 5] = [
            0x3f10_0001,
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
        ];
        for effect_slot in 0..HD_EFFECT_COUNT {
            let original = synthetic_octave_fuzz_h5ep(effect_slot, true);
            let mut expected = original.clone();
            let block =
                H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
            expected[block..block + 4].copy_from_slice(&SCREAMER_ID.to_be_bytes());
            for (row, key) in EXPECTED_KEYS.into_iter().enumerate() {
                let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
            }
            expected[block + 0x0f] = 5;
            for row in 0..HD_PARAMETER_ROW_COUNT {
                expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
            }

            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            preset
                .replace_octave_fuzz_with_screamer_defaults(HdFxSlot::new(effect_slot).unwrap())
                .unwrap();
            assert_eq!(preset.as_bytes(), expected, "FX slot {effect_slot}");
            assert_eq!(preset.effect_model_id(effect_slot), Some(SCREAMER_ID));
            assert_eq!(preset.trailing_bytes(), b"private-tail");
        }
    }

    #[test]
    fn octave_fuzz_to_sub_octave_fuzz_uses_literal_defaults_for_all_public_slots() {
        const TARGET_ID: u32 = 0x0205_0014;
        const EXPECTED_KEYS: [u32; 5] = [
            0x3f10_0001,
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
        ];
        for public_slot in 0..HD_EFFECT_COUNT {
            let original = synthetic_octave_fuzz_h5ep(public_slot, true);
            let mut expected = original.clone();
            let block =
                H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + public_slot * HD_EFFECT_BLOCK_STRIDE;
            expected[block..block + 4].copy_from_slice(&TARGET_ID.to_be_bytes());
            for (row, key) in EXPECTED_KEYS.into_iter().enumerate() {
                let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
            }
            expected[block + 0x0f] = 5;
            for row in 0..HD_PARAMETER_ROW_COUNT {
                expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
            }

            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            preset
                .replace_octave_fuzz_with_sub_octave_fuzz_defaults(
                    HdFxSlot::new(public_slot).unwrap(),
                )
                .unwrap();
            assert_eq!(preset.as_bytes(), expected, "public FX slot {public_slot}");
            assert_eq!(preset.effect_model_id(public_slot), Some(TARGET_ID));
            assert_eq!(preset.trailing_bytes(), b"private-tail");
        }
        assert!(HdFxSlot::new(8).is_err());
    }

    #[test]
    fn octave_fuzz_admitted_distortion_defaults_match_the_exact_closed_set_and_write_set() {
        const TARGETS: [(HdDistortionDefault, u32); 12] = [
            (HdDistortionDefault::JetFuzz, 0x0205_000a),
            (HdDistortionDefault::BuzzSaw, 0x0205_0015),
            (HdDistortionDefault::ClassicDistortion, 0x0205_000b),
            (HdDistortionDefault::ColorDrive, 0x0205_0018),
            (HdDistortionDefault::FacialFuzz, 0x0205_0011),
            (HdDistortionDefault::FuzzPi, 0x0205_000f),
            (HdDistortionDefault::HeavyDistortion, 0x0205_0016),
            (HdDistortionDefault::JumboFuzz, 0x0205_0017),
            (HdDistortionDefault::Line6Distortion, 0x0205_0012),
            (HdDistortionDefault::Line6Drive, 0x0205_0013),
            (HdDistortionDefault::Overdrive, 0x0205_0010),
            (HdDistortionDefault::TubeDrive, 0x0205_000d),
        ];
        assert_eq!(HdDistortionDefault::ALL.len(), 12);
        assert_eq!(HdDistortionDefault::ALL, TARGETS.map(|(target, _)| target));
        assert_eq!(
            TARGETS.map(|(target, _)| target.model_id()),
            TARGETS.map(|(_, model_id)| model_id)
        );
        const KEYS: [u32; 5] = [
            0x3f10_0001,
            0x3f10_0002,
            0x3f10_0003,
            0x3f10_0004,
            0x3f10_0005,
        ];

        for (target, target_id) in TARGETS {
            for public_slot in 0..HD_EFFECT_COUNT {
                let original = synthetic_octave_fuzz_h5ep(public_slot, true);
                let mut expected = original.clone();
                let block = H5EP_HEADER_BYTES
                    + HD_EFFECT_BLOCK_OFFSET
                    + public_slot * HD_EFFECT_BLOCK_STRIDE;
                expected[block..block + 4].copy_from_slice(&target_id.to_be_bytes());
                for (row, key) in KEYS.into_iter().enumerate() {
                    let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                    expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                    expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
                }
                expected[block + 0x0f] = 5;
                for row in 0..HD_PARAMETER_ROW_COUNT {
                    expected
                        [block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
                }

                let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
                preset
                    .replace_octave_fuzz_with_admitted_defaults(
                        HdFxSlot::new(public_slot).unwrap(),
                        target,
                    )
                    .unwrap();
                assert_eq!(
                    preset.as_bytes(),
                    expected,
                    "{target:?}, slot {public_slot}"
                );
                assert_eq!(preset.effect_model_id(public_slot), Some(target_id));
                assert_eq!(preset.trailing_bytes(), b"private-tail");
            }
        }
        assert!(HdFxSlot::new(8).is_err());
    }

    #[test]
    fn admitted_distortion_defaults_reject_guard_failures_without_changing_library_baselines() {
        let source = synthetic_octave_fuzz_h5ep(0, true);
        let mut cases = Vec::new();
        for model in [
            0x0205_000a,
            0x0205_0015,
            0x0205_000b,
            0x0205_0018,
            0x0205_0011,
            0x0205_000f,
            0x0205_0016,
            0x0205_0017,
            0x0205_0012,
            0x0205_0013,
            0x0205_0010,
            0x0205_000d,
            HD_SCREAMER_MODEL_ID,
            HD_SUB_OCTAVE_FUZZ_MODEL_ID,
            0x0200_0013,
            HD_FX_LOOP_MODEL_ID,
        ] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET
                ..H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4]
                .copy_from_slice(&model.to_be_bytes());
            cases.push(("wrong old model", bytes));
        }
        for validity in [0, 2] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
            cases.push(("validity", bytes));
        }
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
        cases.push(("enable", bytes));
        for count in [4, 13] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
            cases.push(("count", bytes));
        }
        let row0 = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
        let mut bytes = source.clone();
        bytes[row0..row0 + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
        cases.push(("unknown prefix key", bytes));
        let mut bytes = source.clone();
        let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
        bytes[row1..row1 + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
        cases.push(("duplicate prefix key", bytes));
        let marker = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + 11 * HD_PARAMETER_ROW_STRIDE
            + 0x10;
        let mut bytes = source.clone();
        bytes[marker] = 3;
        cases.push(("marker", bytes));

        for target in HdDistortionDefault::ALL {
            for (label, bytes) in &cases {
                let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
                let before = preset.as_bytes().to_vec();
                assert!(
                    preset
                        .replace_octave_fuzz_with_admitted_defaults(
                            HdFxSlot::new(0).unwrap(),
                            target,
                        )
                        .is_err(),
                    "{target:?}: {label}"
                );
                assert_eq!(preset.as_bytes(), before, "{target:?}: {label}");

                let mut library = HdEditorLibrary::default();
                let index = library.import_bytes(bytes.clone()).unwrap();
                let saved = library.file(index).unwrap().as_bytes().to_vec();
                assert!(
                    library
                        .replace_octave_fuzz_with_admitted_defaults(
                            index,
                            HdFxSlot::new(0).unwrap(),
                            target,
                        )
                        .is_err(),
                    "library {target:?}: {label}"
                );
                assert_eq!(library.file(index).unwrap().as_bytes(), saved);
                assert!(!library.is_dirty(index), "library {target:?}: {label}");
            }
        }
    }

    #[test]
    fn octave_fuzz_default_replacements_reject_invalid_inputs_atomically_for_both_targets() {
        type PresetReplace = fn(&mut OpaqueH5epPreset, HdFxSlot) -> Result<(), ToneError>;
        type LibraryReplace = fn(&mut HdEditorLibrary, usize, HdFxSlot) -> Result<(), ToneError>;
        let replacements: [(&str, PresetReplace, LibraryReplace); 3] = [
            (
                "Vintage Pre",
                OpaqueH5epPreset::replace_octave_fuzz_with_vintage_pre_defaults,
                HdEditorLibrary::replace_octave_fuzz_with_vintage_pre_defaults,
            ),
            (
                "Screamer",
                OpaqueH5epPreset::replace_octave_fuzz_with_screamer_defaults,
                HdEditorLibrary::replace_octave_fuzz_with_screamer_defaults,
            ),
            (
                "Sub Octave Fuzz",
                OpaqueH5epPreset::replace_octave_fuzz_with_sub_octave_fuzz_defaults,
                HdEditorLibrary::replace_octave_fuzz_with_sub_octave_fuzz_defaults,
            ),
        ];
        let source = synthetic_octave_fuzz_h5ep(0, true);
        let mut cases = Vec::new();
        for model in [
            HD_TUBE_DRIVE_MODEL_ID,
            HD_SCREAMER_MODEL_ID,
            HD_SUB_OCTAVE_FUZZ_MODEL_ID,
            0x0200_0013, // sentinel model
            HD_FX_LOOP_MODEL_ID,
        ] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET
                ..H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4]
                .copy_from_slice(&model.to_be_bytes());
            cases.push(("wrong old model", bytes));
        }
        for validity in [0, 2] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
            cases.push(("validity", bytes));
        }
        let mut invalid_enable = source.clone();
        invalid_enable[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
        cases.push(("enable", invalid_enable));
        for count in [4, 13] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
            cases.push(("count", bytes));
        }
        let row0 = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
        let mut unknown_prefix = source.clone();
        unknown_prefix[row0..row0 + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
        cases.push(("unknown first-five key", unknown_prefix));
        let mut duplicate_prefix = source.clone();
        let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
        duplicate_prefix[row1..row1 + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
        cases.push(("duplicate prefix", duplicate_prefix));
        let mut invalid_marker = source.clone();
        let marker = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + 11 * HD_PARAMETER_ROW_STRIDE
            + 0x10;
        invalid_marker[marker] = 3;
        cases.push(("assignment marker", invalid_marker));

        for (target, replace_preset, replace_library) in replacements {
            for (label, bytes) in &cases {
                let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
                let before = preset.as_bytes().to_vec();
                assert!(
                    replace_preset(&mut preset, HdFxSlot::new(0).unwrap()).is_err(),
                    "{target}: {label}"
                );
                assert_eq!(preset.as_bytes(), before, "{target}: {label} raw bytes");

                let mut library = HdEditorLibrary::default();
                let index = library.import_bytes(bytes.clone()).unwrap();
                assert!(!library.is_dirty(index));
                assert!(
                    replace_library(&mut library, index, HdFxSlot::new(0).unwrap()).is_err(),
                    "{target}: {label} shared library"
                );
                assert!(!library.is_dirty(index), "{target}: {label} dirty state");
                assert_eq!(library.file(index).unwrap().as_bytes(), bytes);
                library.revert_unsaved(index).unwrap();
                assert_eq!(
                    library.file(index).unwrap().as_bytes(),
                    bytes,
                    "{target}: {label} saved baseline"
                );
            }

            let parent_bytes = synthetic_h5es(&[]);
            let mut library = HdEditorLibrary::default();
            let parent = library.import_bytes(parent_bytes.clone()).unwrap();
            assert!(
                replace_library(&mut library, parent, HdFxSlot::new(0).unwrap()).is_err(),
                "{target}: reject container parent"
            );
            assert!(!library.is_dirty(parent), "{target}: parent remains clean");
            assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
        }
    }

    #[test]
    fn ceiling_is_checked_after_valid_magic_and_version_by_parser_and_dispatcher() {
        let mut at_ceiling = synthetic_h5ep(&[]);
        at_ceiling.resize(TONE_BYTES_CEILING as usize, 0x6d);
        let parsed = OpaqueH5epPreset::parse(at_ceiling.clone()).unwrap();
        assert_eq!(parsed.serialize_noop(), at_ceiling);
        let dispatched = PodPresetFile::parse(at_ceiling.clone()).unwrap();
        assert_eq!(dispatched.serialize_noop(), at_ceiling);

        let mut over_ceiling = at_ceiling.clone();
        over_ceiling.push(0x6d);
        for result in [
            OpaqueH5epPreset::parse(over_ceiling.clone()),
            match PodPresetFile::parse(over_ceiling.clone()) {
                Err(error) => Err(error),
                Ok(_) => panic!("oversized H5EP must fail dispatch"),
            },
        ] {
            assert!(
                matches!(result, Err(ToneError::Format { detail }) if detail.contains("ceiling"))
            );
        }
    }

    #[test]
    fn pod_dispatch_preserves_valid_tones_and_rejects_wrong_pinf_and_other_tags() {
        let valid = live_tone();
        let dispatched = PodPresetFile::parse(valid.clone()).unwrap();
        assert!(matches!(&dispatched, PodPresetFile::PodxtLive(_)));
        assert_eq!(dispatched.serialize_noop(), valid);

        let wrong_target = fixture(ContainerKind::L6pa, 0x0003_000b, 1, &[(AMP_SLOT, 7)]);
        assert!(matches!(
            PodPresetFile::parse(wrong_target),
            Err(ToneError::WrongTarget {
                device_id: 0x0003_000b
            })
        ));
        for tag in [b"H5ES".as_slice(), b"H5EB".as_slice()] {
            let mut unsupported = vec![0u8; H5EP_MIN_BYTES];
            unsupported[..4].copy_from_slice(tag);
            unsupported[4..8].copy_from_slice(&1u32.to_be_bytes());
            assert!(matches!(
                PodPresetFile::parse(unsupported),
                Err(ToneError::Format { .. })
            ));
        }
    }

    fn synthetic_h5es(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xa7; H5ES_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"H5ES");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        for _ in 0..H5ES_PRESET_COUNT {
            bytes.extend_from_slice(&synthetic_h5ep(&[]));
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    fn synthetic_h5eb(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x39; H5EB_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"H5EB");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        for _ in 0..H5EB_SETLIST_COUNT {
            bytes.extend_from_slice(&synthetic_h5es(&[]));
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    #[test]
    fn enable_action_edits_only_extracted_setlist_and_bundle_children() {
        let slot = HdFxSlot::new(0).unwrap();
        let child = synthetic_enable_h5ep(0, 0);

        let mut setlist_bytes = synthetic_h5es(&[]);
        setlist_bytes[H5ES_HEADER_BYTES..H5ES_HEADER_BYTES + H5EP_MIN_BYTES]
            .copy_from_slice(&child[..H5EP_MIN_BYTES]);
        let mut setlist_library = HdEditorLibrary::default();
        let setlist_index = setlist_library.import_bytes(setlist_bytes.clone()).unwrap();
        assert!(
            setlist_library
                .set_effect_enabled(setlist_index, slot, true)
                .is_err()
        );
        let extracted = setlist_library.extract_preset(setlist_index, 0, 0).unwrap();
        setlist_library
            .set_effect_enabled(extracted, slot, true)
            .unwrap();
        assert_eq!(
            setlist_library.file(setlist_index).unwrap().as_bytes(),
            setlist_bytes
        );
        assert!(setlist_library.is_dirty(extracted));
        assert_eq!(
            setlist_library.file(extracted).unwrap().as_bytes()
                [H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8],
            1
        );

        let mut bundle_bytes = synthetic_h5eb(&[]);
        let child_at = H5EB_HEADER_BYTES + H5ES_HEADER_BYTES;
        bundle_bytes[child_at..child_at + H5EP_MIN_BYTES].copy_from_slice(&child[..H5EP_MIN_BYTES]);
        let mut bundle_library = HdEditorLibrary::default();
        let bundle_index = bundle_library.import_bytes(bundle_bytes.clone()).unwrap();
        assert!(
            bundle_library
                .set_effect_enabled(bundle_index, slot, true)
                .is_err()
        );
        let extracted = bundle_library.extract_preset(bundle_index, 0, 0).unwrap();
        bundle_library
            .set_effect_enabled(extracted, slot, true)
            .unwrap();
        assert_eq!(
            bundle_library.file(bundle_index).unwrap().as_bytes(),
            bundle_bytes
        );
        assert!(bundle_library.is_dirty(extracted));
        assert_eq!(
            bundle_library.file(extracted).unwrap().as_bytes()
                [H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8],
            1
        );
    }

    #[test]
    fn h5es_and_h5eb_parse_fixed_nested_records_and_preserve_tails() {
        let setlist_bytes = synthetic_h5es(b"setlist tail");
        assert_eq!(setlist_bytes.len(), H5ES_MIN_BYTES + 12);
        let dispatched = PodPresetFile::parse(setlist_bytes.clone()).unwrap();
        let PodPresetFile::H5es(setlist) = dispatched else {
            panic!("H5ES should dispatch to a setlist");
        };
        assert_eq!(setlist.preset_bytes(0).unwrap().len(), H5EP_MIN_BYTES);
        assert_eq!(setlist.preset_bytes(63).unwrap().len(), H5EP_MIN_BYTES);
        assert!(setlist.preset_bytes(64).is_none());
        assert_eq!(setlist.trailing_bytes(), b"setlist tail");
        assert_eq!(setlist.as_bytes(), setlist_bytes);

        let bundle_bytes = synthetic_h5eb(b"bundle tail");
        assert_eq!(bundle_bytes.len(), H5EB_MIN_BYTES + 11);
        let dispatched = PodPresetFile::parse(bundle_bytes.clone()).unwrap();
        let PodPresetFile::H5eb(bundle) = dispatched else {
            panic!("H5EB should dispatch to a bundle");
        };
        assert_eq!(bundle.setlist_bytes(0).unwrap().len(), H5ES_MIN_BYTES);
        assert_eq!(bundle.setlist_bytes(7).unwrap().len(), H5ES_MIN_BYTES);
        assert!(bundle.setlist_bytes(8).is_none());
        assert_eq!(bundle.trailing_bytes(), b"bundle tail");
        assert_eq!(bundle.as_bytes(), bundle_bytes);
    }

    #[test]
    fn h5es_and_h5eb_reject_bad_nested_tags_versions_truncation_and_caps() {
        let setlist = synthetic_h5es(&[]);
        assert!(H5esSetlist::parse(setlist[..H5ES_MIN_BYTES - 1].to_vec()).is_err());
        let mut bad_child = setlist.clone();
        bad_child[H5ES_HEADER_BYTES..H5ES_HEADER_BYTES + 4].copy_from_slice(b"H5EB");
        assert!(H5esSetlist::parse(bad_child).is_err());
        let mut bad_version = setlist;
        bad_version[7] = 2;
        assert!(H5esSetlist::parse(bad_version).is_err());

        let bundle = synthetic_h5eb(&[]);
        assert!(H5ebBundle::parse(bundle[..H5EB_MIN_BYTES - 1].to_vec()).is_err());
        let mut bad_nested_setlist = bundle.clone();
        bad_nested_setlist[H5EB_HEADER_BYTES..H5EB_HEADER_BYTES + 4].copy_from_slice(b"H5EP");
        assert!(H5ebBundle::parse(bad_nested_setlist).is_err());
        let mut over_cap = bundle;
        over_cap.resize(HD_BUNDLE_BYTES_CEILING + 1, 0);
        assert!(
            matches!(H5ebBundle::parse(over_cap), Err(ToneError::Format { detail }) if detail.contains("ceiling"))
        );
    }

    #[test]
    fn hd_editor_library_tracks_edits_and_extracts_children_without_changing_parents() {
        let mut bytes = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        bytes[body + block..body + block + 4].copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row = body + block + HD_EFFECT_ROWS_OFFSET;
        bytes[row..row + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
        bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());

        let mut library = HdEditorLibrary::default();
        let preset = library.import_bytes(bytes.clone()).unwrap();
        assert_eq!(
            library.file(preset).unwrap().hd_container_kind(),
            Some("H5EP preset")
        );
        assert!(!library.any_dirty());
        let Some(PodPresetFile::H5ep(preset_file)) = library.file_mut(preset) else {
            panic!("expected H5EP");
        };
        assert_eq!(preset_file.screamer_drive_percent(0), Some(25));
        preset_file.set_screamer_drive_percent(0, 40).unwrap();
        assert!(library.is_dirty(preset));
        library.note_saved(preset).unwrap();
        assert!(!library.is_dirty(preset));

        let parent_bytes = synthetic_h5es(b"tail");
        let parent = library.import_bytes(parent_bytes.clone()).unwrap();
        let child = library.extract_preset(parent, 0, 17).unwrap();
        assert_ne!(parent, child);
        assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
        let start = H5ES_HEADER_BYTES + 17 * H5EP_MIN_BYTES;
        assert_eq!(
            library.file(child).unwrap().as_bytes(),
            &parent_bytes[start..start + H5EP_MIN_BYTES]
        );
        assert!(!library.any_dirty());
    }

    #[test]
    fn vintage_pre_replacement_operates_on_extracted_setlist_and_bundle_children_only() {
        let slot = HdFxSlot::new(3).unwrap();
        let source_child = synthetic_octave_fuzz_h5ep(slot.index(), false);
        let mut expected_child = OpaqueH5epPreset::parse(source_child.clone()).unwrap();
        expected_child
            .replace_octave_fuzz_with_vintage_pre_defaults(slot)
            .unwrap();

        let mut setlist = synthetic_h5es(&[]);
        let setlist_child_at = H5ES_HEADER_BYTES + 23 * H5EP_MIN_BYTES;
        setlist[setlist_child_at..setlist_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
        let mut library = HdEditorLibrary::default();
        let parent = library.import_bytes(setlist.clone()).unwrap();
        let child = library.extract_preset(parent, 0, 23).unwrap();
        assert!(!library.is_dirty(parent));
        library
            .replace_octave_fuzz_with_vintage_pre_defaults(child, slot)
            .unwrap();
        assert_eq!(library.file(parent).unwrap().as_bytes(), setlist);
        assert_eq!(
            library.file(child).unwrap().as_bytes(),
            expected_child.as_bytes()
        );
        assert!(!library.is_dirty(parent));
        assert!(library.is_dirty(child));
        library.revert_unsaved(child).unwrap();
        assert_eq!(library.file(child).unwrap().as_bytes(), source_child);
        assert!(!library.is_dirty(child));

        let mut bundle = synthetic_h5eb(&[]);
        let bundle_child_at =
            H5EB_HEADER_BYTES + 6 * H5ES_MIN_BYTES + H5ES_HEADER_BYTES + 57 * H5EP_MIN_BYTES;
        bundle[bundle_child_at..bundle_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
        let bundle_original = bundle.clone();
        let parent = library.import_bytes(bundle).unwrap();
        let child = library.extract_preset(parent, 6, 57).unwrap();
        library
            .replace_octave_fuzz_with_vintage_pre_defaults(child, slot)
            .unwrap();
        assert_eq!(library.file(parent).unwrap().as_bytes(), bundle_original);
        assert_eq!(
            library.file(child).unwrap().as_bytes(),
            expected_child.as_bytes()
        );
        assert!(!library.is_dirty(parent));
        assert!(library.is_dirty(child));
    }

    #[test]
    fn sub_octave_fuzz_replacement_edits_only_the_extracted_bundle_child() {
        let slot = HdFxSlot::new(7).unwrap();
        let source_child = synthetic_octave_fuzz_h5ep(slot.index(), false);
        let mut expected_child = OpaqueH5epPreset::parse(source_child.clone()).unwrap();
        expected_child
            .replace_octave_fuzz_with_sub_octave_fuzz_defaults(slot)
            .unwrap();

        let mut bundle = synthetic_h5eb(b"bundle tail");
        let parent_child_at =
            H5EB_HEADER_BYTES + 7 * H5ES_MIN_BYTES + H5ES_HEADER_BYTES + 63 * H5EP_MIN_BYTES;
        bundle[parent_child_at..parent_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
        let parent_bytes = bundle.clone();
        let mut library = HdEditorLibrary::default();
        let parent = library.import_bytes(bundle).unwrap();
        let child = library.extract_preset(parent, 7, 63).unwrap();
        library
            .replace_octave_fuzz_with_sub_octave_fuzz_defaults(child, slot)
            .unwrap();

        assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
        assert_eq!(
            library.file(child).unwrap().as_bytes(),
            expected_child.as_bytes()
        );
        assert!(!library.is_dirty(parent));
        assert!(library.is_dirty(child));
        library.revert_unsaved(child).unwrap();
        assert_eq!(library.file(child).unwrap().as_bytes(), source_child);
        assert!(!library.is_dirty(child));
        assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
    }

    #[test]
    fn hd_editor_input_reader_checks_regular_file_and_growth_bounds() {
        let called = std::cell::Cell::new(false);
        assert!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: false,
                    len: 0,
                }),
                || {
                    called.set(true);
                    Ok(Cursor::new(vec![]))
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: true,
                    len: (HD_BUNDLE_BYTES_CEILING + 1) as u64,
                }),
                || {
                    called.set(true);
                    Ok(Cursor::new(vec![]))
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert_eq!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: true,
                    len: 3,
                }),
                || Ok(Cursor::new(vec![1, 2, 3])),
            )
            .unwrap(),
            [1, 2, 3]
        );

        let consumed = Rc::new(Cell::new(0));
        let error = read_regular_hd_editor_file(
            Ok(PathFacts {
                regular_file: true,
                len: 1,
            }),
            || Ok(ContinuousReader(Rc::clone(&consumed))),
        )
        .unwrap_err();
        assert!(error.detail().contains("ceiling"));
        assert_eq!(consumed.get(), HD_BUNDLE_BYTES_CEILING + 1);
    }

    #[test]
    fn h5ep_body_conversion_is_involutive_and_known_drive_edit_is_narrow() {
        let mut original = synthetic_h5ep(b"opaque tail");
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body_start + block..body_start + block + 4]
            .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row0 = body_start + block + HD_EFFECT_ROWS_OFFSET;
        original[row0..row0 + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
        original[row0 + 4..row0 + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        original[row0 + 0x10..row0 + 0x14].copy_from_slice(&[0x00, 0xde, 0xad, 0xbe]);
        let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
        original[row1..row1 + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
        original[row1 + 4..row1 + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
        let file = OpaqueH5epPreset::parse(original.clone()).unwrap();
        assert_eq!(file.screamer_drive_percent(0), Some(50));

        let disk_body = file.body_bytes().to_vec();
        let mut transformed = disk_body.clone();
        transform_hd_body_words(&mut transformed);
        transform_hd_body_words(&mut transformed);
        assert_eq!(transformed, disk_body);
        let editor_body = file.body_in_editor_order();
        let mut round_trip_body = editor_body;
        transform_hd_body_words(&mut round_trip_body);
        assert_eq!(round_trip_body, disk_body);
        assert_eq!(file.serialize_noop(), original);

        let mut edited = file;
        for percent in [0, 50, 100] {
            edited.set_screamer_drive_percent(0, percent).unwrap();
            assert_eq!(edited.screamer_drive_percent(0), Some(percent));
        }
        let mut expected = original.clone();
        expected[row0 + 4..row0 + 8].copy_from_slice(&1.0f32.to_bits().to_be_bytes());
        assert_eq!(edited.as_bytes(), expected);
        assert_eq!(
            &edited.as_bytes()[row0 + 0x10..row0 + 0x14],
            &[0, 0xde, 0xad, 0xbe]
        );
        let before = edited.as_bytes().to_vec();
        assert!(edited.set_screamer_drive_percent(0, 101).is_err());
        assert!(edited.set_screamer_drive_percent(8, 50).is_err());
        assert_eq!(edited.as_bytes(), before);

        let mut non_grid = original.clone();
        non_grid[row0 + 4..row0 + 8].copy_from_slice(&0.123_456_7f32.to_bits().to_be_bytes());
        let mut non_grid = OpaqueH5epPreset::parse(non_grid).unwrap();
        assert_eq!(non_grid.screamer_drive_percent(0), None);
        let non_grid_before = non_grid.as_bytes().to_vec();
        assert!(non_grid.set_screamer_drive_percent(0, 12).is_err());
        assert_eq!(non_grid.as_bytes(), non_grid_before);
    }

    #[test]
    fn admitted_primary_type1_controls_edit_only_their_value_word() {
        for (model, controls) in [
            (HD_SCREAMER_MODEL_ID, &HdType1Control::ALL[..5]),
            (HD_TUBE_DRIVE_MODEL_ID, &HdType1Control::ALL[5..10]),
            (HD_OCTAVE_FUZZ_MODEL_ID, &HdType1Control::ALL[10..15]),
            (HD_BAND_SHIFT_EQ_MODEL_ID, &HdType1Control::ALL[15..16]),
            (HD_STEREO_DELAY_MODEL_ID, &HdType1Control::ALL[16..19]),
            (HD_FX_LOOP_MODEL_ID, &HdType1Control::ALL[19..20]),
        ] {
            let mut original = synthetic_h5ep(&[]);
            let body_start = H5EP_HEADER_BYTES;
            let block = HD_EFFECT_BLOCK_OFFSET;
            original[body_start + block..body_start + block + 4]
                .copy_from_slice(&model.to_be_bytes());
            let first_row = body_start + block + HD_EFFECT_ROWS_OFFSET;
            for (index, control) in controls.iter().copied().enumerate() {
                let row = first_row + index * HD_PARAMETER_ROW_STRIDE;
                original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
                original[row + 4..row + 8]
                    .copy_from_slice(&((index as u32 + 10) as f32 / 100.0).to_bits().to_be_bytes());
                if model == HD_OCTAVE_FUZZ_MODEL_ID && (1..=3).contains(&index) {
                    original[row + 8..row + 16]
                        .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
                    original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
                }
                original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
            }
            let mut preset = OpaqueH5epPreset::parse(original).unwrap();

            for (index, control) in controls.iter().copied().enumerate() {
                assert_eq!(
                    preset.hd_type1_control_percent(0, control),
                    Some(10 + index as u8)
                );
                let before = preset.as_bytes().to_vec();
                preset.set_hd_type1_control_percent(0, control, 73).unwrap();
                assert_eq!(preset.hd_type1_control_percent(0, control), Some(73));
                let value_at = first_row + index * HD_PARAMETER_ROW_STRIDE + 4;
                let mut expected = before;
                expected[value_at..value_at + 4].copy_from_slice(&0.73f32.to_bits().to_be_bytes());
                assert_eq!(preset.as_bytes(), expected);
            }
        }
    }

    #[test]
    fn selected_effect_type1_descriptors_are_model_gated_and_write_only_the_value_word() {
        assert_eq!(HD_TYPE1_MAPPED_CONTROLS.len(), 364);
        let mut source_rows = std::collections::BTreeSet::new();
        for control in HD_TYPE1_MAPPED_CONTROLS {
            assert!(
                source_rows.insert((control.model_id, control.key)),
                "duplicate source row for {} {}",
                control.model_label,
                control.label
            );
        }
        for (model_id, label) in [
            (HD_SCREAMER_MODEL_ID, "TONE"),
            (HD_TUBE_DRIVE_MODEL_ID, "MID"),
            (HD_OCTAVE_FUZZ_MODEL_ID, "MID"),
        ] {
            assert_eq!(
                HD_TYPE1_MAPPED_CONTROLS
                    .iter()
                    .filter(|control| (control.model_id, control.label) == (model_id, label))
                    .count(),
                1
            );
        }
        for control in HD_TYPE1_MAPPED_CONTROLS {
            let mut original = synthetic_h5ep(&[]);
            let body = H5EP_HEADER_BYTES;
            let block = HD_EFFECT_BLOCK_OFFSET;
            original[body + block..body + block + 4]
                .copy_from_slice(&control.model_id().to_be_bytes());
            let row = body + block + HD_EFFECT_ROWS_OFFSET;
            original[row..row + 4].copy_from_slice(&control.key.to_be_bytes());
            original[row + 4..row + 8].copy_from_slice(&0.37f32.to_bits().to_be_bytes());
            original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

            let mut preset = OpaqueH5epPreset::parse(original).unwrap();
            assert_eq!(preset.hd_type1_mapped_control_percent(0, control), Some(37));
            let before = preset.as_bytes().to_vec();
            preset
                .set_hd_type1_mapped_control_percent(0, control, 73)
                .unwrap();
            assert_eq!(preset.hd_type1_mapped_control_percent(0, control), Some(73));
            let mut expected = before;
            expected[row + 4..row + 8].copy_from_slice(&0.73f32.to_bits().to_be_bytes());
            assert_eq!(
                preset.as_bytes(),
                expected,
                "{} {}",
                control.model_label(),
                control.label()
            );

            let wrong_model_id = if control.model_id() == HD_SCREAMER_MODEL_ID {
                HD_TUBE_DRIVE_MODEL_ID
            } else {
                HD_SCREAMER_MODEL_ID
            };
            let mut wrong_model = preset.clone();
            wrong_model.bytes[body + block..body + block + 4]
                .copy_from_slice(&wrong_model_id.to_be_bytes());
            let before_wrong_model = wrong_model.as_bytes().to_vec();
            assert_eq!(
                wrong_model.hd_type1_mapped_control_percent(0, control),
                None
            );
            assert!(
                wrong_model
                    .set_hd_type1_mapped_control_percent(0, control, 50)
                    .is_err()
            );
            assert_eq!(wrong_model.as_bytes(), before_wrong_model);

            let mut wrong_key = preset.clone();
            wrong_key.bytes[row..row + 4]
                .copy_from_slice(&(control.key() ^ 0x8000_0000).to_be_bytes());
            let before_wrong_key = wrong_key.as_bytes().to_vec();
            assert_eq!(wrong_key.hd_type1_mapped_control_percent(0, control), None);
            assert!(
                wrong_key
                    .set_hd_type1_mapped_control_percent(0, control, 50)
                    .is_err()
            );
            assert_eq!(wrong_key.as_bytes(), before_wrong_key);

            let before_invalid = preset.as_bytes().to_vec();
            assert!(
                preset
                    .set_hd_type1_mapped_control_percent(0, control, 101)
                    .is_err()
            );
            assert_eq!(preset.as_bytes(), before_invalid);

            let mut assigned = preset.clone();
            assigned.bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            let before_assigned = assigned.as_bytes().to_vec();
            assert_eq!(assigned.hd_type1_mapped_control_percent(0, control), None);
            assert!(
                assigned
                    .set_hd_type1_mapped_control_percent(0, control, 50)
                    .is_err()
            );
            assert_eq!(assigned.as_bytes(), before_assigned);
        }
    }

    #[test]
    fn selected_non_type1_controls_use_their_source_integer_grid_and_preserve_other_bytes() {
        assert_eq!(HD_NUMERIC_CONTROLS.len(), 80);
        let unique_rows = HD_NUMERIC_CONTROLS
            .iter()
            .map(|control| (control.model_id(), control.key()))
            .collect::<std::collections::BTreeSet<_>>();
        let unique_models = HD_NUMERIC_CONTROLS
            .iter()
            .map(|control| control.model_id())
            .collect::<std::collections::BTreeSet<_>>();
        let unique_tags = HD_NUMERIC_CONTROLS
            .iter()
            .map(|control| control.raw_type())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique_rows.len(), 80);
        assert_eq!(unique_models.len(), 65);
        assert_eq!(unique_tags.len(), 17);
        for control in HD_NUMERIC_CONTROLS {
            assert_ne!(control.raw_type(), 0x01);
            if control.raw_type() == 0x1b {
                assert_eq!(
                    (
                        control.model_id(),
                        control.key(),
                        control.min(),
                        control.max()
                    ),
                    (0x020c_000e, 0x3f10_0005, 0, 15_000)
                );
            }
            let mut original = synthetic_h5ep(&[]);
            let body = H5EP_HEADER_BYTES;
            let block = HD_EFFECT_BLOCK_OFFSET;
            original[body + block..body + block + 4]
                .copy_from_slice(&control.model_id().to_be_bytes());
            let row = body + block + HD_EFFECT_ROWS_OFFSET;
            original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
            let initial = control.min() + (control.max() - control.min()) / 3;
            let encoded = normalized_numeric_value(initial, control.min(), control.max());
            original[row + 4..row + 8].copy_from_slice(&encoded.to_bits().to_be_bytes());
            original[row + 8..row + 16]
                .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
            original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
            original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);

            let mut preset = OpaqueH5epPreset::parse(original).unwrap();
            assert_eq!(
                preset.hd_numeric_control_value(0, control),
                Some(initial),
                "{} {}",
                control.model_label(),
                control.label()
            );
            let before = preset.as_bytes().to_vec();
            preset
                .set_hd_numeric_control_value(0, control, control.max())
                .unwrap();
            assert_eq!(
                preset.hd_numeric_control_value(0, control),
                Some(control.max())
            );
            let mut expected = before;
            let value_at = row + 4;
            expected[value_at..value_at + 4].copy_from_slice(
                &normalized_numeric_value(control.max(), control.min(), control.max())
                    .to_bits()
                    .to_be_bytes(),
            );
            assert_eq!(preset.as_bytes(), expected);

            let before_rejection = preset.as_bytes().to_vec();
            assert!(
                preset
                    .set_hd_numeric_control_value(0, control, control.max() + 1)
                    .is_err()
            );
            assert_eq!(preset.as_bytes(), before_rejection);

            let mut wrong_model = preset.clone();
            let wrong_model_id = if control.model_id() == HD_GRAPHIC_EQ_MODEL_ID {
                HD_STUDIO_EQ_MODEL_ID
            } else {
                HD_GRAPHIC_EQ_MODEL_ID
            };
            let wrong_model_at = H5EP_HEADER_BYTES + block;
            wrong_model.bytes[wrong_model_at..wrong_model_at + 4]
                .copy_from_slice(&wrong_model_id.to_be_bytes());
            let before_wrong_model = wrong_model.as_bytes().to_vec();
            assert_eq!(wrong_model.hd_numeric_control_value(0, control), None);
            assert!(
                wrong_model
                    .set_hd_numeric_control_value(0, control, initial)
                    .is_err()
            );
            assert_eq!(wrong_model.as_bytes(), before_wrong_model);

            let mut flagged = preset.clone();
            flagged.bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            let before_flagged = flagged.as_bytes().to_vec();
            assert_eq!(flagged.hd_numeric_control_value(0, control), None);
            assert!(
                flagged
                    .set_hd_numeric_control_value(0, control, initial)
                    .is_err()
            );
            assert_eq!(flagged.as_bytes(), before_flagged);

            let mut off_grid = preset.clone();
            off_grid.bytes[row + 4..row + 8].copy_from_slice(&0.12345f32.to_bits().to_be_bytes());
            let before_off_grid = off_grid.as_bytes().to_vec();
            assert_eq!(off_grid.hd_numeric_control_value(0, control), None);
            assert!(
                off_grid
                    .set_hd_numeric_control_value(0, control, initial)
                    .is_err()
            );
            assert_eq!(off_grid.as_bytes(), before_off_grid);
        }
    }

    #[test]
    fn vintage_pre_type1b_uses_fixed_source_words_and_patches_only_first_row() {
        const FIXTURES: &[(i32, u32)] = &[
            (0, 0x0000_0000),
            (1, 0x388b_cf65),
            (2, 0x390b_cf65),
            (3, 0x3951_b717),
            (7_499, 0x3eff_f743),
            (7_500, 0x3f00_0000),
            (7_501, 0x3f00_045e),
            (14_999, 0x3f7f_fba2),
            (15_000, 0x3f80_0000),
        ];
        let control = HD_NUMERIC_CONTROLS
            .iter()
            .copied()
            .find(|control| control.raw_type() == 0x1b)
            .expect("admitted primary Vintage Pre control");
        assert_eq!(control.model_id(), 0x020c_000e);
        assert_eq!(control.key(), 0x3f10_0005);
        assert_eq!((control.min(), control.max()), (0, 15_000));

        // These words were independently rounded from the source's integer
        // divide and DWORD float store for the exact 0..15000 / 0..1 path.
        // The expectations are literals, not values generated by the helper.
        for &(integer, word) in FIXTURES {
            assert_eq!(
                normalized_numeric_value(integer, 0, 15_000).to_bits(),
                word,
                "source word for editor integer {integer}"
            );
        }

        let mut original = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let first_row = body + block + HD_EFFECT_ROWS_OFFSET;
        original[first_row..first_row + 4].copy_from_slice(&control.key().to_be_bytes());
        original[first_row + 4..first_row + 8].copy_from_slice(&FIXTURES[5].1.to_be_bytes());
        original[first_row + 8..first_row + 16]
            .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
        original[first_row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
        original[first_row + 17..first_row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
        let duplicate = first_row + HD_PARAMETER_ROW_STRIDE;
        original[duplicate..duplicate + 4].copy_from_slice(&control.key().to_be_bytes());
        original[duplicate + 4..duplicate + 8].copy_from_slice(&FIXTURES[6].1.to_be_bytes());
        original[duplicate + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

        for &(integer, word) in FIXTURES {
            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            assert_eq!(preset.hd_numeric_control_value(0, control), Some(7_500));
            preset
                .set_hd_numeric_control_value(0, control, integer)
                .unwrap();

            let mut expected = original.clone();
            expected[first_row + 4..first_row + 8].copy_from_slice(&word.to_be_bytes());
            assert_eq!(preset.as_bytes(), expected, "editor integer {integer}");
            assert_eq!(preset.hd_numeric_control_value(0, control), Some(integer));
            assert_eq!(
                &preset.as_bytes()[duplicate + 4..duplicate + 8],
                &FIXTURES[6].1.to_be_bytes(),
                "a later duplicate must remain unchanged"
            );
        }
    }

    #[test]
    fn vintage_pre_type1b_rejects_unadmitted_rows_and_values_transactionally() {
        let control = HD_NUMERIC_CONTROLS
            .iter()
            .copied()
            .find(|control| control.raw_type() == 0x1b)
            .unwrap();
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        let row = body + block + HD_EFFECT_ROWS_OFFSET;
        let mut valid = synthetic_h5ep(&[]);
        valid[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        valid[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
        valid[row + 4..row + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
        valid[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

        let reject_unchanged = |bytes: Vec<u8>, slot: usize, value: i32| {
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            let before = preset.as_bytes().to_vec();
            assert!(
                preset
                    .set_hd_numeric_control_value(slot, control, value)
                    .is_err()
            );
            assert_eq!(preset.as_bytes(), before);
        };
        reject_unchanged(valid.clone(), HD_EFFECT_COUNT, 12_000);
        reject_unchanged(valid.clone(), 0, -1);
        reject_unchanged(valid.clone(), 0, 15_001);

        let wrong_rows: [(usize, u32); 5] = [
            (body + block, 0x020c_000d),
            (row, 0x3f10_0004),
            (row + 4, 0x7fc0_0000),
            (row + 4, 0x7f80_0000),
            (row + 4, 0x3f00_0001),
        ];
        for (offset, word) in wrong_rows {
            let mut bytes = valid.clone();
            bytes[offset..offset + 4].copy_from_slice(&word.to_be_bytes());
            reject_unchanged(bytes, 0, 12_000);
        }
        let mut assigned = valid;
        assigned[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        reject_unchanged(assigned, 0, 12_000);
    }

    #[test]
    fn non_type1_source_word_fixtures_pin_each_normalization_family() {
        // Independently transcribed from the admitted Edit 2.27 routines in
        // non-type1-converter-and-display-routines.asm (image SHA-256
        // 871c86da6b6810307dc201297d8411f2b2f7eec7e4c3e150f0aa881f8c89afe2).
        // The common routine subtracts integer min/max in 32-bit registers,
        // FILDs the numerator, FIDIVs by the integer width, maps the ratio
        // through the configured 0..1 endpoints, then FSTPs a DWORD. Tag 0x10
        // has its own converter with the same observed endpoint mapping.
        // Expected words below are fixed source-contract fixtures; neither
        // input nor output expectations are generated by the production encoder.
        const FIXTURES: &[(u8, i32, i32, i32, u32, i32, u32)] = &[
            (0x05, 0, 240, 73, 0x3e9b_bbbc, 167, 0x3f32_2222),
            (0x06, 0, 220, 73, 0x3ea9_e413, 147, 0x3f2b_0df7),
            (0x07, 0, 360, 127, 0x3eb4_9f4a, 247, 0x3f2f_a4fa),
            (0x08, -96, 0, -53, 0x3ee5_5555, -32, 0x3f2a_aaab),
            (0x09, -80, 0, -53, 0x3eac_cccd, -27, 0x3f29_999a),
            (0x0a, 0, 24, 7, 0x3e95_5555, 17, 0x3f35_5555),
            (0x0d, 0, 1400, 467, 0x3eaa_c9e0, 933, 0x3f2a_9b10),
            (0x0e, 0, 1400, 523, 0x3ebf_44c1, 1001, 0x3f37_0a3d),
            (0x0f, 0, 3520, 1107, 0x3ea1_04a8, 2347, 0x3f2a_b0df),
            (0x10, 20, 2000, 680, 0x3eaa_aaab, 1320, 0x3f28_14b0),
            (0x11, 0, 200, 61, 0x3e9c_28f6, 137, 0x3f2f_5c29),
            (0x12, 0, 800, 267, 0x3eaa_e148, 533, 0x3f2a_8f5c),
            (0x13, 0, 4000, 1337, 0x3eab_22d1, 2667, 0x3f2a_b021),
            (0x14, -240, 240, -81, 0x3ea9_999a, 80, 0x3f2a_aaab),
            (0x16, 0, 200, 73, 0x3eba_e148, 129, 0x3f25_1eb8),
            (0x1a, 20, 500, 181, 0x3eab_bbbc, 340, 0x3f2a_aaab),
            (0x1b, 0, 15_000, 7_500, 0x3f00_0000, 14_999, 0x3f7f_fba2),
        ];

        for &(raw_type, min, max, initial, initial_word, edited, edited_word) in FIXTURES {
            let control = HD_NUMERIC_CONTROLS
                .iter()
                .find(|control| {
                    control.raw_type() == raw_type && control.min() == min && control.max() == max
                })
                .unwrap_or_else(|| panic!("missing admitted fixture family {raw_type:#04x}"));
            let mut original = synthetic_h5ep(&[]);
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            original[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
            let row = block + HD_EFFECT_ROWS_OFFSET;
            original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
            original[row + 4..row + 8].copy_from_slice(&initial_word.to_be_bytes());
            original[row + 8..row + 16]
                .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
            original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
            original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);

            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            assert_eq!(preset.hd_numeric_control_value(0, *control), Some(initial));
            preset
                .set_hd_numeric_control_value(0, *control, edited)
                .unwrap();

            let mut expected = original;
            expected[row + 4..row + 8].copy_from_slice(&edited_word.to_be_bytes());
            assert_eq!(preset.as_bytes(), expected, "raw family {raw_type:#04x}");

            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("fixture.h5p");
            preset.save_copy(&path).unwrap();
            let saved = std::fs::read(&path).unwrap();
            assert_eq!(saved, expected, "saved raw family {raw_type:#04x}");
            let reparsed = OpaqueH5epPreset::parse(saved).unwrap();
            assert_eq!(reparsed.hd_numeric_control_value(0, *control), Some(edited));
            assert_eq!(
                &reparsed.as_bytes()[row + 4..row + 8],
                &edited_word.to_be_bytes()
            );
        }
    }

    #[test]
    fn screamer_and_tube_drive_contracts_keep_model_and_third_control_distinct() {
        for (model, controls, selected_label, selected_index) in [
            (
                HD_SCREAMER_MODEL_ID,
                HdType1Control::ALL[..5].to_vec(),
                "TONE",
                2,
            ),
            (
                HD_TUBE_DRIVE_MODEL_ID,
                HdType1Control::ALL[5..10].to_vec(),
                "MID",
                2,
            ),
            (
                HD_OCTAVE_FUZZ_MODEL_ID,
                HdType1Control::ALL[10..15].to_vec(),
                "MID",
                2,
            ),
            (
                HD_BAND_SHIFT_EQ_MODEL_ID,
                HdType1Control::ALL[15..16].to_vec(),
                "SHIFT",
                0,
            ),
            (
                HD_STEREO_DELAY_MODEL_ID,
                HdType1Control::ALL[16..19].to_vec(),
                "MIX",
                2,
            ),
            (
                HD_FX_LOOP_MODEL_ID,
                HdType1Control::ALL[19..20].to_vec(),
                "MIX",
                0,
            ),
        ] {
            let mut bytes = synthetic_h5ep(&[]);
            let body = H5EP_HEADER_BYTES;
            let block = HD_EFFECT_BLOCK_OFFSET;
            bytes[body + block..body + block + 4].copy_from_slice(&model.to_be_bytes());
            let first = body + block + HD_EFFECT_ROWS_OFFSET;
            for (index, control) in controls.iter().copied().enumerate() {
                let row = first + index * HD_PARAMETER_ROW_STRIDE;
                bytes[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
                bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
                if model == HD_OCTAVE_FUZZ_MODEL_ID && (1..=3).contains(&index) {
                    bytes[row + 8..row + 16]
                        .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
                    bytes[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
                }
            }
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            let selected = controls[selected_index];
            assert_eq!(selected.label(), selected_label);
            assert_eq!(preset.hd_type1_control_percent(0, selected), Some(25));
            let before = preset.as_bytes().to_vec();
            preset
                .set_hd_type1_control_percent(0, selected, 61)
                .unwrap();
            assert_eq!(preset.hd_type1_control_percent(0, selected), Some(61));
            let mut expected = before;
            let value = first + selected_index * HD_PARAMETER_ROW_STRIDE + 4;
            expected[value..value + 4].copy_from_slice(&0.61f32.to_bits().to_be_bytes());
            assert_eq!(preset.as_bytes(), expected);

            let wrong_model_control = match model {
                HD_SCREAMER_MODEL_ID => HdType1Control::TubeDrive(TubeDriveControl::Mid),
                HD_TUBE_DRIVE_MODEL_ID => HdType1Control::OctaveFuzz(OctaveFuzzControl::Mid),
                HD_OCTAVE_FUZZ_MODEL_ID => HdType1Control::BandShiftEq(BandShiftEqControl::Shift),
                HD_BAND_SHIFT_EQ_MODEL_ID => {
                    HdType1Control::StereoDelay(StereoDelayControl::LeftFeedback)
                }
                HD_STEREO_DELAY_MODEL_ID => HdType1Control::FxLoop(FxLoopControl::Mix),
                _ => HdType1Control::Screamer(ScreamerControl::Tone),
            };
            assert_eq!(
                preset.hd_type1_control_percent(0, wrong_model_control),
                None
            );
            let unchanged = preset.as_bytes().to_vec();
            assert!(
                preset
                    .set_hd_type1_control_percent(0, wrong_model_control, 50)
                    .is_err()
            );
            assert_eq!(preset.as_bytes(), unchanged);
        }
    }

    #[test]
    fn screamer_drive_admission_rejects_nonzero_row_flags_and_bad_values_atomically() {
        let mut eligible = synthetic_h5ep(&[]);
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        eligible[body_start + block..body_start + block + 4]
            .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row = body_start + block + HD_EFFECT_ROWS_OFFSET;
        eligible[row..row + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
        eligible[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        eligible[row + HD_PARAMETER_ROW_FLAG_OFFSET..row + HD_PARAMETER_ROW_STRIDE]
            .copy_from_slice(&[0, 0x11, 0x22, 0x33]);

        // Zero is the only admitted row flag; every nonzero value is treated
        // as unresolved assignment state, without changing any row bytes.
        for flag in 1..=u8::MAX {
            let mut bytes = eligible.clone();
            bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = flag;
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert_eq!(preset.screamer_drive_percent(0), None, "flag {flag:#04x}");
            let before = preset.as_bytes().to_vec();
            assert!(preset.set_screamer_drive_percent(0, 25).is_err());
            assert_eq!(preset.as_bytes(), before, "flag {flag:#04x}");
        }

        let mut missing_key = eligible.clone();
        missing_key[row..row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
        let mut missing_key = OpaqueH5epPreset::parse(missing_key).unwrap();
        assert_eq!(missing_key.screamer_drive_percent(0), None);
        let before = missing_key.as_bytes().to_vec();
        assert!(missing_key.set_screamer_drive_percent(0, 25).is_err());
        assert_eq!(missing_key.as_bytes(), before);

        for value in [f32::NAN, f32::INFINITY, -0.01, 1.01, 0.123_456_7] {
            let mut bytes = eligible.clone();
            bytes[row + 4..row + 8].copy_from_slice(&value.to_bits().to_be_bytes());
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert_eq!(preset.screamer_drive_percent(0), None);
            let before = preset.as_bytes().to_vec();
            assert!(preset.set_screamer_drive_percent(0, 25).is_err());
            assert_eq!(preset.as_bytes(), before);
        }
    }

    #[test]
    fn header_context_byte_is_preserved_but_not_admitted_as_product_identity() {
        for context_byte in [0x2d, 0x2e] {
            let mut bytes = synthetic_h5ep(&[]);
            bytes[0x0b] = context_byte;
            let parsed = PodPresetFile::parse(bytes.clone()).unwrap();
            assert!(matches!(parsed, PodPresetFile::H5ep(_)));
            assert_eq!(parsed.serialize_noop(), bytes);
        }
    }

    #[test]
    fn opaque_h5ep_save_copy_is_atomic_and_does_not_overwrite() {
        let original = synthetic_h5ep(b"tail");
        let file = PodPresetFile::parse(original.clone()).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("copy.5xe");
        file.save_copy(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(file.save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);

        let blocked_dir = directory.path().join("existing-directory");
        std::fs::create_dir(&blocked_dir).unwrap();
        let sentinel = blocked_dir.join("preserve.bin");
        std::fs::write(&sentinel, b"sentinel").unwrap();
        assert!(file.save_copy(&blocked_dir).is_err());
        assert_eq!(file.as_bytes(), original);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");

        let invalid_destination = directory.path().join("missing-parent").join("copy.5xe");
        assert!(file.save_copy(&invalid_destination).is_err());
        assert!(!invalid_destination.exists());
        assert_eq!(file.as_bytes(), original);
        assert_eq!(file.serialize_noop(), original);
    }
    use crate::pod_assets::{self, SlotRole};

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
    fn zero_length_nonregular_facts_are_not_read() {
        let mut called = false;
        let error = read_regular_tone(
            Ok(PathFacts {
                regular_file: false,
                len: 0,
            }),
            || {
                called = true;
                Ok(Cursor::new(vec![1]))
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
                Ok(Cursor::new(vec![1]))
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(error.detail().contains("ceiling"));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-tone");
        std::fs::create_dir(&path).unwrap();
        assert!(open_regular_file(&path).is_err());
        let error = read_regular_tone(path_facts(&path), || -> io::Result<Cursor<Vec<u8>>> {
            panic!("directory must not be read")
        })
        .unwrap_err();
        assert!(error.detail().contains("regular file"));
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

    #[test]
    fn type9_choice_writer_covers_all_file_slots_rows_choices_and_enable_states() {
        let expected_values: [(HdType9Choice, [u8; 4]); 3] = [
            (HdType9Choice::Choice0, [0x00, 0x00, 0x00, 0x00]),
            (HdType9Choice::Choice1, [0x3f, 0x00, 0x00, 0x00]),
            (HdType9Choice::Choice2, [0x3f, 0x80, 0x00, 0x00]),
        ];
        for slot_index in 0..8 {
            for row_index in [0, 5, 11] {
                for enabled in [0, 1] {
                    for (choice, expected_word) in expected_values {
                        let original =
                            synthetic_type9_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                        let mut expected = original.clone();
                        let value_at = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot_index * HD_EFFECT_BLOCK_STRIDE
                            + HD_EFFECT_ROWS_OFFSET
                            + row_index * HD_PARAMETER_ROW_STRIDE
                            + 4;
                        expected[value_at..value_at + 4].copy_from_slice(&expected_word);
                        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                        preset
                            .set_hd_type9_choice(HdFxSlot::new(slot_index).unwrap(), choice)
                            .unwrap();
                        assert_eq!(preset.as_bytes(), expected);
                        assert_eq!(
                            preset.as_bytes()[value_at - 4..value_at],
                            0x3f10_0004u32.to_be_bytes()
                        );
                        assert_eq!(preset.as_bytes()[value_at + 4], 0x31);
                        assert_eq!(preset.as_bytes()[value_at + 12], 0);
                        assert_eq!(&preset.as_bytes()[H5EP_MIN_BYTES..], b"type9-tail");
                    }
                }
            }
        }
        assert!(HdFxSlot::new(8).is_err());

        let mut count_one = synthetic_type9_h5ep(0, 0, 0, 0x3f40_0000);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
        count_one[block + 0x0f] = 1;
        let mut preset = OpaqueH5epPreset::parse(count_one).unwrap();
        preset
            .set_hd_type9_choice(HdFxSlot::new(0).unwrap(), HdType9Choice::Choice0)
            .unwrap();
        assert_eq!(preset.as_bytes()[block + 0x0f], 1);
    }

    #[test]
    fn type9_choice_catalog_requires_exact_model_key_pairs_and_preserves_other_bytes() {
        for (association_index, (model_id, key)) in
            HD_TYPE9_CHOICE_MODEL_KEYS.into_iter().enumerate()
        {
            assert_eq!(hd_type9_choice_key(model_id), Some(key));
            let slot_index = [0, 3, 7, 5][association_index];
            for row_index in [0, 5, 11] {
                for enabled in [0, 1] {
                    for choice in HdType9Choice::ALL {
                        let original = synthetic_type9_h5ep_for(
                            slot_index,
                            row_index,
                            enabled,
                            0x3eaa_aaab,
                            model_id,
                            key,
                        );
                        let mut expected = original.clone();
                        let value_at = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot_index * HD_EFFECT_BLOCK_STRIDE
                            + HD_EFFECT_ROWS_OFFSET
                            + row_index * HD_PARAMETER_ROW_STRIDE
                            + 4;
                        expected[value_at..value_at + 4]
                            .copy_from_slice(&choice.bits().to_be_bytes());
                        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                        let slot = HdFxSlot::new(slot_index).unwrap();
                        assert_eq!(
                            preset.hd_type9_choice_value(slot),
                            HdType9ChoiceValue::UnknownFinite(0x3eaa_aaab)
                        );
                        preset.set_hd_type9_choice(slot, choice).unwrap();
                        assert_eq!(
                            preset.as_bytes(),
                            expected,
                            "{model_id:#010x}/{key:#010x}, row {row_index}, enable {enabled}, choice {:?}",
                            choice
                        );
                        assert_eq!(
                            preset.hd_type9_choice_value(slot),
                            HdType9ChoiceValue::Choice(choice)
                        );
                    }
                }
            }
        }

        for (model_id, key) in HD_TYPE9_CHOICE_MODEL_KEYS {
            let make = || synthetic_type9_h5ep_for(0, 0, 0, 0x3f40_0000, model_id, key);
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            let row0 = block + HD_EFFECT_ROWS_OFFSET;
            let mut invalid_cases = Vec::new();

            let mut invalid_effect = make();
            invalid_effect[block + 0x0b] = 0;
            invalid_cases.push(("invalid effect", invalid_effect));
            let mut invalid_enable = make();
            invalid_enable[block + 8] = 2;
            invalid_cases.push(("noncanonical enable", invalid_enable));
            let mut invalid_count = make();
            invalid_count[block + 0x0f] = 0;
            invalid_cases.push(("invalid count", invalid_count));
            let mut beyond_count = make();
            beyond_count[row0..row0 + 4].fill(0);
            let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
            beyond_count[row5..row5 + 4].copy_from_slice(&key.to_be_bytes());
            beyond_count[block + 0x0f] = 5;
            invalid_cases.push(("target beyond count", beyond_count));
            let mut duplicate = make();
            let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
            duplicate[row9..row9 + 4].copy_from_slice(&key.to_be_bytes());
            invalid_cases.push(("duplicate key", duplicate));
            let mut assigned = make();
            assigned[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            invalid_cases.push(("assigned marker", assigned));
            let mut nonfinite = make();
            nonfinite[row0 + 4..row0 + 8].copy_from_slice(&0x7fc0_0001u32.to_be_bytes());
            invalid_cases.push(("non-finite existing value", nonfinite));

            for (reason, bytes) in invalid_cases {
                let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
                assert!(
                    preset
                        .set_hd_type9_choice(HdFxSlot::new(0).unwrap(), HdType9Choice::Choice1)
                        .is_err(),
                    "{model_id:#010x}/{key:#010x}: {reason} must reject"
                );
                assert_eq!(preset.as_bytes(), bytes, "{reason} must be atomic");
            }
        }

        for (model_id, expected_key, wrong_key) in [
            (0x020a_0010, Some(0x3f10_0004), 0x3f10_0003),
            (0x020a_000f, Some(0x3f10_0004), 0x3f10_0003),
            (0x020a_0012, Some(0x3f10_0003), 0x3f10_0004),
            (0x020a_0014, Some(0x3f10_0004), 0x3f10_0003),
            (0x020a_0013, None, 0x3f10_0004),
        ] {
            assert_eq!(hd_type9_choice_key(model_id), expected_key);
            let bytes = synthetic_type9_h5ep_for(2, 4, 0, 0x3f40_0000, model_id, wrong_key);
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert_eq!(
                preset.hd_type9_choice_value(HdFxSlot::new(2).unwrap()),
                HdType9ChoiceValue::Unavailable
            );
            assert!(
                preset
                    .set_hd_type9_choice(HdFxSlot::new(2).unwrap(), HdType9Choice::Choice1)
                    .is_err()
            );
            assert_eq!(
                preset.as_bytes(),
                bytes,
                "cross-pair {model_id:#010x}/{wrong_key:#010x}"
            );
        }
    }

    #[test]
    fn type9_choice_writer_rejects_invalid_inputs_atomically_and_scans_all_rows() {
        let slot = HdFxSlot::new(5).unwrap();
        let source = synthetic_type9_h5ep(5, 0, 0, 0x3f00_0000);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
        let row0 = block + HD_EFFECT_ROWS_OFFSET;
        let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();

        let mut wrong_model = source.clone();
        wrong_model[block..block + 4].copy_from_slice(&0x020a_0015u32.to_be_bytes());
        cases.push(("wrong model", wrong_model));
        let mut invalid_validity = source.clone();
        invalid_validity[block + 0x0b] = 0;
        cases.push(("invalid effect", invalid_validity));
        let mut invalid_enable = source.clone();
        invalid_enable[block + 8] = 2;
        cases.push(("invalid enable", invalid_enable));
        for count in [0, 13] {
            let mut invalid_count = source.clone();
            invalid_count[block + 0x0f] = count;
            cases.push(("invalid count", invalid_count));
        }
        let mut target_beyond_count = source.clone();
        let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
        target_beyond_count[row0..row0 + 4].fill(0);
        target_beyond_count[row5..row5 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        target_beyond_count[block + 0x0f] = 5;
        cases.push(("target beyond count", target_beyond_count));
        let mut missing = source.clone();
        missing[row0..row0 + 4].fill(0);
        cases.push(("missing key", missing));
        for duplicate_value in [0x3f00_0000u32, 0x3f80_0000] {
            let mut duplicate = source.clone();
            let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
            duplicate[row9..row9 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
            duplicate[row9 + 4..row9 + 8].copy_from_slice(&duplicate_value.to_be_bytes());
            duplicate[block + 0x0f] = 1; // duplicate lies beyond the admitted count
            cases.push(("duplicate key beyond count", duplicate));
        }
        let mut assigned = source.clone();
        assigned[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        cases.push(("assigned marker", assigned));
        for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
            let mut non_finite = source.clone();
            non_finite[row0 + 4..row0 + 8].copy_from_slice(&bits.to_be_bytes());
            cases.push(("non-finite current value", non_finite));
        }

        for (label, bytes) in cases {
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            let before = preset.as_bytes().to_vec();
            assert!(
                preset
                    .set_hd_type9_choice(slot, HdType9Choice::Choice1)
                    .is_err(),
                "{label} must reject"
            );
            assert_eq!(preset.as_bytes(), before, "{label} must be atomic");
        }

        let mut malformed_magic = source.clone();
        malformed_magic[..4].copy_from_slice(b"H5EX");
        assert!(OpaqueH5epPreset::parse(malformed_magic).is_err());
        let mut malformed_version = source.clone();
        malformed_version[4..8].copy_from_slice(&2u32.to_be_bytes());
        assert!(OpaqueH5epPreset::parse(malformed_version).is_err());
        assert!(OpaqueH5epPreset::parse(source[..H5EP_MIN_BYTES - 1].to_vec()).is_err());

        assert!(HdType9Choice::from_index(3).is_err());
        assert!(HdType9Choice::from_index(255).is_err());
    }

    #[test]
    fn type9_choice_read_is_lossless_and_equal_word_noop_still_checks_guards() {
        let slot = HdFxSlot::new(2).unwrap();
        for (bits, expected) in [
            (
                0x0000_0000,
                HdType9ChoiceValue::Choice(HdType9Choice::Choice0),
            ),
            (
                0x3f00_0000,
                HdType9ChoiceValue::Choice(HdType9Choice::Choice1),
            ),
            (
                0x3f80_0000,
                HdType9ChoiceValue::Choice(HdType9Choice::Choice2),
            ),
            (0x3f40_0000, HdType9ChoiceValue::UnknownFinite(0x3f40_0000)),
            (0x7fc0_0001, HdType9ChoiceValue::NonFinite(0x7fc0_0001)),
            (0x7f80_0000, HdType9ChoiceValue::NonFinite(0x7f80_0000)),
            (0xff80_0000, HdType9ChoiceValue::NonFinite(0xff80_0000)),
        ] {
            let bytes = synthetic_type9_h5ep(2, 11, 1, bits);
            let preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert_eq!(preset.hd_type9_choice_value(slot), expected);
            assert_eq!(preset.serialize_noop(), preset.as_bytes());
        }
        let mut same =
            OpaqueH5epPreset::parse(synthetic_type9_h5ep(2, 11, 1, 0x3f00_0000)).unwrap();
        let before = same.as_bytes().to_vec();
        same.set_hd_type9_choice(slot, HdType9Choice::Choice1)
            .unwrap();
        assert_eq!(same.as_bytes(), before);

        let mut bad_marker = before.clone();
        let marker = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + 2 * HD_EFFECT_BLOCK_STRIDE
            + HD_EFFECT_ROWS_OFFSET
            + 11 * HD_PARAMETER_ROW_STRIDE
            + HD_PARAMETER_ROW_FLAG_OFFSET;
        bad_marker[marker] = 1;
        let mut bad_marker = OpaqueH5epPreset::parse(bad_marker).unwrap();
        let original_bad_marker = bad_marker.as_bytes().to_vec();
        assert!(
            bad_marker
                .set_hd_type9_choice(slot, HdType9Choice::Choice1)
                .is_err()
        );
        assert_eq!(bad_marker.as_bytes(), original_bad_marker);
    }

    #[test]
    fn type8_choice_writer_patches_only_admitted_word_for_all_slots_rows_and_enable_states() {
        assert_eq!(
            hd_type8_choice_key(HD_TYPE8_CHOICE_MODEL_ID),
            Some(HD_TYPE8_CHOICE_KEY)
        );
        assert_eq!(HdType8Choice::from_index(0), Ok(HdType8Choice::Choice0));
        assert_eq!(HdType8Choice::from_index(1), Ok(HdType8Choice::Choice1));
        assert!(HdType8Choice::from_index(2).is_err());

        for slot_index in 0..8 {
            for row_index in [0, 5, 11] {
                for enabled in [0, 1] {
                    for (choice, word) in [
                        (HdType8Choice::Choice0, [0x00, 0x00, 0x00, 0x00]),
                        (HdType8Choice::Choice1, [0x3f, 0x80, 0x00, 0x00]),
                    ] {
                        let original =
                            synthetic_type8_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                        let value_at = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot_index * HD_EFFECT_BLOCK_STRIDE
                            + HD_EFFECT_ROWS_OFFSET
                            + row_index * HD_PARAMETER_ROW_STRIDE
                            + 4;
                        let mut expected = original.clone();
                        expected[value_at..value_at + 4].copy_from_slice(&word);
                        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                        let slot = HdFxSlot::new(slot_index).unwrap();
                        assert_eq!(
                            preset.hd_type8_choice_value(slot),
                            HdType8ChoiceValue::UnknownFinite(0x3eaa_aaab)
                        );
                        preset.set_hd_type8_choice(slot, choice).unwrap();
                        assert_eq!(preset.as_bytes(), expected);
                        assert_eq!(
                            preset.hd_type8_choice_value(slot),
                            HdType8ChoiceValue::Choice(choice)
                        );
                    }
                }
            }
        }

        let slot = HdFxSlot::new(2).unwrap();
        for (bits, expected) in [
            (
                0x0000_0000,
                HdType8ChoiceValue::Choice(HdType8Choice::Choice0),
            ),
            (
                0x3f80_0000,
                HdType8ChoiceValue::Choice(HdType8Choice::Choice1),
            ),
            (0x3f40_0000, HdType8ChoiceValue::UnknownFinite(0x3f40_0000)),
            (0x7fc0_0001, HdType8ChoiceValue::NonFinite(0x7fc0_0001)),
            (0x7f80_0000, HdType8ChoiceValue::NonFinite(0x7f80_0000)),
            (0xff80_0000, HdType8ChoiceValue::NonFinite(0xff80_0000)),
        ] {
            let preset = OpaqueH5epPreset::parse(synthetic_type8_h5ep(2, 11, 1, bits)).unwrap();
            assert_eq!(preset.hd_type8_choice_value(slot), expected);
        }
    }

    #[test]
    fn type8_choice_writer_rejects_foreign_or_malformed_contexts_atomically() {
        let slot = HdFxSlot::new(5).unwrap();
        let source = synthetic_type8_h5ep(5, 0, 0, 0x3f40_0000);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
        let row0 = block + HD_EFFECT_ROWS_OFFSET;
        let mut cases = Vec::new();
        let mut wrong_model = source.clone();
        wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
        cases.push(("wrong model", wrong_model));
        let mut wrong_key = source.clone();
        wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0003u32.to_be_bytes());
        cases.push(("wrong key", wrong_key));
        for (offset, value, name) in [
            (0x0b, 0, "invalid effect"),
            (8, 2, "noncanonical enable"),
            (0x0f, 0, "zero count"),
            (0x0f, 13, "excessive count"),
        ] {
            let mut invalid = source.clone();
            invalid[block + offset] = value;
            cases.push((name, invalid));
        }
        let mut missing = source.clone();
        missing[row0..row0 + 4].fill(0);
        cases.push(("missing key", missing));
        let mut beyond_count = source.clone();
        let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
        beyond_count[row0..row0 + 4].fill(0);
        beyond_count[row5..row5 + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
        beyond_count[block + 0x0f] = 5;
        cases.push(("target beyond count", beyond_count));
        let mut duplicate = source.clone();
        let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[row9..row9 + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
        duplicate[block + 0x0f] = 1;
        cases.push(("duplicate outside count", duplicate));
        let mut marker = source.clone();
        marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        cases.push(("assigned marker", marker));
        for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
            let mut nonfinite = source.clone();
            nonfinite[row0 + 4..row0 + 8].copy_from_slice(&bits.to_be_bytes());
            cases.push(("non-finite current value", nonfinite));
        }
        for (reason, bytes) in cases {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert!(
                preset
                    .set_hd_type8_choice(slot, HdType8Choice::Choice1)
                    .is_err(),
                "{reason}"
            );
            assert_eq!(
                preset.as_bytes(),
                bytes,
                "{reason} must preserve source bytes"
            );
        }

        let mut same = OpaqueH5epPreset::parse(synthetic_type8_h5ep(5, 0, 0, 0x3f80_0000)).unwrap();
        let before = same.as_bytes().to_vec();
        same.set_hd_type8_choice(slot, HdType8Choice::Choice1)
            .unwrap();
        assert_eq!(same.as_bytes(), before);
        let mut bad_marker = before.clone();
        bad_marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        let mut bad_marker = OpaqueH5epPreset::parse(bad_marker).unwrap();
        let before = bad_marker.as_bytes().to_vec();
        assert!(
            bad_marker
                .set_hd_type8_choice(slot, HdType8Choice::Choice1)
                .is_err()
        );
        assert_eq!(bad_marker.as_bytes(), before, "guards run before no-op");
    }

    fn decode_hex_fixture(hex: &str) -> Vec<u8> {
        hex.split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("fixture contains hex bytes"))
            .collect()
    }

    #[test]
    fn type11_choices_match_independent_complete_vectors_for_both_keys_per_model() {
        let cases = [
            (
                HdType11Control::Model020a001eKey3f100002,
                include_str!("../tests/fixtures/type11-model-020a001e-source.hex"),
                [
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100002-choice-0.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100002-choice-1.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100002-choice-2.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100002-choice-3.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100002-choice-4.hex"
                    ),
                ],
                3usize,
                0x3e12_3456u32,
            ),
            (
                HdType11Control::Model020a001eKey3f100003,
                include_str!("../tests/fixtures/type11-model-020a001e-source.hex"),
                [
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100003-choice-0.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100003-choice-1.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100003-choice-2.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100003-choice-3.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001e-key-3f100003-choice-4.hex"
                    ),
                ],
                8usize,
                0x3e65_4321u32,
            ),
            (
                HdType11Control::Model020a001dKey3f100001,
                include_str!("../tests/fixtures/type11-model-020a001d-source.hex"),
                [
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100001-choice-0.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100001-choice-1.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100001-choice-2.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100001-choice-3.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100001-choice-4.hex"
                    ),
                ],
                3usize,
                0x3e12_3456u32,
            ),
            (
                HdType11Control::Model020a001dKey3f100002,
                include_str!("../tests/fixtures/type11-model-020a001d-source.hex"),
                [
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100002-choice-0.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100002-choice-1.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100002-choice-2.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100002-choice-3.hex"
                    ),
                    include_str!(
                        "../tests/fixtures/type11-model-020a001d-key-3f100002-choice-4.hex"
                    ),
                ],
                8usize,
                0x3e65_4321u32,
            ),
        ];
        for (control, source_hex, expected_hex, target_row, current_bits) in cases {
            let source = decode_hex_fixture(source_hex);
            assert_eq!(&source[..8], b"H5EP\0\0\0\x01");
            assert_eq!(
                &source[H5EP_MIN_BYTES..],
                b"TYPE11-INDEPENDENT-SYNTHETIC-SUFFIX"
            );
            let block = H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
            let row = block + 0x10 + target_row * 0x14;
            assert_eq!(&source[row..row + 4], &control.key().to_be_bytes());
            assert_eq!(&source[row + 4..row + 8], &current_bits.to_be_bytes());
            for (choice, expected_hex) in HdType11Choice::ALL.into_iter().zip(expected_hex) {
                let expected = decode_hex_fixture(expected_hex);
                assert_eq!(source.len(), expected.len());
                assert_eq!(&expected[row + 4..row + 8], &choice.bits().to_be_bytes());
                let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
                preset
                    .set_hd_type11_choice(HdFxSlot::new(5).unwrap(), control, choice)
                    .unwrap();
                assert_eq!(preset.as_bytes(), expected, "{control:?} {choice:?}");
            }
        }
    }

    #[test]
    fn type8_choice_matches_independent_complete_file_vectors_and_noop() {
        // These vectors are checked-in complete H5EP + suffix fixtures. The
        // two expected outputs are literal files, not rebuilt with setter or
        // production offset helpers. The target value is at fixed file offset
        // 0x9dc in this independently specified source layout.
        let source = decode_hex_fixture(include_str!("../tests/fixtures/type8-choice-source.hex"));
        let choice0 = decode_hex_fixture(include_str!("../tests/fixtures/type8-choice-0.hex"));
        let choice1 = decode_hex_fixture(include_str!("../tests/fixtures/type8-choice-1.hex"));
        assert_eq!(source.len(), choice0.len());
        assert_eq!(source.len(), choice1.len());
        assert_eq!(&source[..8], b"H5EP\0\0\0\x01");
        assert_eq!(&source[0x9dc..0x9e0], &[0x3e, 0xaa, 0xaa, 0xab]);
        assert_eq!(&choice0[0x9dc..0x9e0], &[0x00, 0x00, 0x00, 0x00]);
        assert_eq!(&choice1[0x9dc..0x9e0], &[0x3f, 0x80, 0x00, 0x00]);
        for offset in 0..source.len() {
            if !(0x9dc..0x9e0).contains(&offset) {
                assert_eq!(source[offset], choice0[offset], "choice0 byte {offset:#x}");
                assert_eq!(source[offset], choice1[offset], "choice1 byte {offset:#x}");
            }
        }
        assert_eq!(&source[H5EP_MIN_BYTES..], b"TYPE8-SYNTHETIC-SUFFIX");

        let slot = HdFxSlot::new(5).unwrap();
        let mut to_zero = OpaqueH5epPreset::parse(source.clone()).unwrap();
        to_zero
            .set_hd_type8_choice(slot, HdType8Choice::Choice0)
            .unwrap();
        assert_eq!(to_zero.as_bytes(), choice0);
        let mut to_one = OpaqueH5epPreset::parse(source).unwrap();
        to_one
            .set_hd_type8_choice(slot, HdType8Choice::Choice1)
            .unwrap();
        assert_eq!(to_one.as_bytes(), choice1);

        let mut no_op = OpaqueH5epPreset::parse(choice0.clone()).unwrap();
        no_op
            .set_hd_type8_choice(slot, HdType8Choice::Choice0)
            .unwrap();
        assert_eq!(no_op.as_bytes(), choice0);
    }

    #[test]
    fn type10_choice_is_independent_and_preserves_every_non_target_byte() {
        assert_eq!(
            hd_type10_choice_key(HD_TYPE10_CHOICE_MODEL_ID),
            Some(HD_TYPE10_CHOICE_KEY)
        );
        assert_eq!(HdType10Choice::from_index(0), Ok(HdType10Choice::Choice0));
        assert_eq!(HdType10Choice::from_index(1), Ok(HdType10Choice::Choice1));
        assert!(HdType10Choice::from_index(2).is_err());
        assert_eq!(
            hd_type9_choice_key(HD_TYPE10_CHOICE_MODEL_ID),
            Some(0x3f10_0004)
        );
        for slot_index in 0..8 {
            for row_index in [0, 5, 11] {
                for enabled in [0, 1] {
                    let original =
                        synthetic_type10_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row_index * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    for (choice, word) in [
                        (HdType10Choice::Choice0, 0x0000_0000u32),
                        (HdType10Choice::Choice1, 0x3f80_0000),
                    ] {
                        let mut expected = original.clone();
                        expected[value_at..value_at + 4].copy_from_slice(&word.to_be_bytes());
                        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
                        let slot = HdFxSlot::new(slot_index).unwrap();
                        assert_eq!(
                            preset.hd_type10_choice_value(slot),
                            HdType10ChoiceValue::UnknownFinite(0x3eaa_aaab)
                        );
                        preset.set_hd_type10_choice(slot, choice).unwrap();
                        assert_eq!(preset.as_bytes(), expected);
                        assert_eq!(
                            preset.hd_type10_choice_value(slot),
                            HdType10ChoiceValue::Choice(choice)
                        );
                    }
                }
            }
        }
        let slot = HdFxSlot::new(2).unwrap();
        for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
            let mut p = OpaqueH5epPreset::parse(synthetic_type10_h5ep(2, 11, 1, bits)).unwrap();
            let before = p.as_bytes().to_vec();
            assert!(matches!(
                p.hd_type10_choice_value(slot),
                HdType10ChoiceValue::NonFinite(_)
            ));
            assert!(
                p.set_hd_type10_choice(slot, HdType10Choice::Choice1)
                    .is_err()
            );
            assert_eq!(p.as_bytes(), before);
        }
        let slot = HdFxSlot::new(5).unwrap();
        let source = synthetic_type10_h5ep(5, 0, 1, 0x3eaa_aaab);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
        let row0 = block + HD_EFFECT_ROWS_OFFSET;
        let mut cases = Vec::new();
        let mut wrong_model = source.clone();
        wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
        cases.push(("wrong model", wrong_model));
        let mut wrong_key = source.clone();
        wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        cases.push(("wrong key", wrong_key));
        for (offset, value) in [(0x0b, 0), (8, 2), (0x0f, 0), (0x0f, 13)] {
            let mut bad = source.clone();
            bad[block + offset] = value;
            cases.push(("invalid block guard", bad));
        }
        let mut missing = source.clone();
        missing[row0..row0 + 4].fill(0);
        cases.push(("missing row", missing));
        let mut duplicate = source.clone();
        let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[row9..row9 + 4].copy_from_slice(&HD_TYPE10_CHOICE_KEY.to_be_bytes());
        duplicate[block + 0x0f] = 1;
        cases.push(("duplicate outside count", duplicate));
        let mut beyond = source.clone();
        let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
        beyond[row0..row0 + 4].fill(0);
        beyond[row5..row5 + 4].copy_from_slice(&HD_TYPE10_CHOICE_KEY.to_be_bytes());
        beyond[block + 0x0f] = 5;
        cases.push(("target outside count", beyond));
        let mut marker = source.clone();
        marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        cases.push(("assigned marker", marker));
        for (reason, bytes) in cases {
            let mut equal_choice = bytes.clone();
            equal_choice[row0 + 4..row0 + 8].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
            let mut preset = OpaqueH5epPreset::parse(equal_choice.clone()).unwrap();
            assert!(
                preset
                    .set_hd_type10_choice(slot, HdType10Choice::Choice1)
                    .is_err(),
                "{reason} must reject before equal-word no-op"
            );
            assert_eq!(
                preset.as_bytes(),
                equal_choice,
                "{reason} must preserve all bytes"
            );
        }
        assert!(HdFxSlot::new(8).is_err());
        assert_eq!(HdType10Choice::ALL.len(), 2);
    }

    #[test]
    fn type10_choice_is_closed_to_two_associations_and_covers_each_slot_and_row_edge() {
        assert_eq!(HD_TYPE10_CHOICE_MODEL_KEYS.len(), 2);
        assert_eq!(
            hd_type10_choice_key(0x020a_000f),
            Some(HD_TYPE10_CHOICE_KEY)
        );
        for unknown in [0, 0x020a_000e, 0x020a_0011, 0x020a_0014, u32::MAX] {
            assert_eq!(hd_type10_choice_key(unknown), None);
        }

        for (model_id, key) in HD_TYPE10_CHOICE_MODEL_KEYS {
            for slot_index in 0..HD_EFFECT_COUNT {
                for row_index in [0, 5, 11] {
                    for enabled in [0, 1] {
                        let source = synthetic_type10_h5ep_for(
                            slot_index,
                            row_index,
                            enabled,
                            0x3eaa_aaab,
                            model_id,
                            key,
                        );
                        let value_at = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot_index * HD_EFFECT_BLOCK_STRIDE
                            + HD_EFFECT_ROWS_OFFSET
                            + row_index * HD_PARAMETER_ROW_STRIDE
                            + 4;
                        let slot = HdFxSlot::new(slot_index).unwrap();
                        for (choice, word) in [
                            (HdType10Choice::Choice0, 0x0000_0000u32),
                            (HdType10Choice::Choice1, 0x3f80_0000),
                        ] {
                            let mut expected = source.clone();
                            expected[value_at..value_at + 4].copy_from_slice(&word.to_be_bytes());
                            let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
                            assert_eq!(
                                preset.hd_type10_choice_value(slot),
                                HdType10ChoiceValue::UnknownFinite(0x3eaa_aaab)
                            );
                            preset.set_hd_type10_choice(slot, choice).unwrap();
                            assert_eq!(preset.as_bytes(), expected);
                            assert_eq!(
                                preset.hd_type10_choice_value(slot),
                                HdType10ChoiceValue::Choice(choice)
                            );

                            let before_noop = preset.as_bytes().to_vec();
                            preset.set_hd_type10_choice(slot, choice).unwrap();
                            assert_eq!(
                                preset.as_bytes(),
                                before_noop,
                                "same-choice no-op is byte-identical"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn type10_nonfinite_and_malformed_equal_word_guards_apply_to_both_associations() {
        let slot = HdFxSlot::new(5).unwrap();
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
        let row0 = block + HD_EFFECT_ROWS_OFFSET;
        for (model_id, key) in HD_TYPE10_CHOICE_MODEL_KEYS {
            for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
                let source = synthetic_type10_h5ep_for(5, 0, 1, bits, model_id, key);
                let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
                assert!(matches!(
                    preset.hd_type10_choice_value(slot),
                    HdType10ChoiceValue::NonFinite(_)
                ));
                for choice in HdType10Choice::ALL {
                    assert!(preset.set_hd_type10_choice(slot, choice).is_err());
                    assert_eq!(preset.as_bytes(), source);
                }
            }

            let source = synthetic_type10_h5ep_for(5, 0, 1, 0x3eaa_aaab, model_id, key);
            let mut cases = Vec::new();
            let mut wrong_model = source.clone();
            wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
            cases.push(("foreign model", wrong_model));
            let mut wrong_key = source.clone();
            wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
            cases.push(("type-9 key", wrong_key));
            for (offset, value) in [(0x0b, 0), (8, 2), (0x0f, 0), (0x0f, 13)] {
                let mut bad = source.clone();
                bad[block + offset] = value;
                cases.push(("invalid block guard", bad));
            }
            let mut missing = source.clone();
            missing[row0..row0 + 4].fill(0);
            cases.push(("missing key", missing));
            let mut duplicate = source.clone();
            let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
            duplicate[row9..row9 + 4].copy_from_slice(&key.to_be_bytes());
            duplicate[block + 0x0f] = 1;
            cases.push(("duplicate beyond count", duplicate));
            let mut beyond = source.clone();
            let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
            beyond[row0..row0 + 4].fill(0);
            beyond[row5..row5 + 4].copy_from_slice(&key.to_be_bytes());
            beyond[block + 0x0f] = 5;
            cases.push(("target beyond count", beyond));
            let mut marker = source.clone();
            marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
            cases.push(("assigned marker", marker));

            for (reason, mut malformed) in cases {
                for choice in HdType10Choice::ALL {
                    let word_at = row0 + 4;
                    malformed[word_at..word_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                    let before = malformed.clone();
                    let mut preset = OpaqueH5epPreset::parse(malformed.clone()).unwrap();
                    assert!(
                        preset.set_hd_type10_choice(slot, choice).is_err(),
                        "{reason}"
                    );
                    assert_eq!(preset.as_bytes(), before, "{reason} preserves bytes");
                }
            }
        }
    }

    #[test]
    fn type10_choice_matches_checked_in_complete_h5ep_vectors_and_leaves_type9_control_untouched() {
        let source = decode_hex_fixture(include_str!("../tests/fixtures/type10-choice-source.hex"));
        let expected0 = decode_hex_fixture(include_str!("../tests/fixtures/type10-choice-0.hex"));
        let expected1 = decode_hex_fixture(include_str!("../tests/fixtures/type10-choice-1.hex"));
        assert_eq!(source.len(), expected0.len());
        assert_eq!(source.len(), expected1.len());
        assert_eq!(
            &source[0x950..0x954],
            &HD_TYPE10_CHOICE_MODEL_ID.to_be_bytes()
        );
        assert_eq!(&source[0x9d8..0x9dc], &HD_TYPE10_CHOICE_KEY.to_be_bytes());
        assert_eq!(&source[0x9dc..0x9e0], &[0x3e, 0xaa, 0xaa, 0xab]);
        assert_eq!(&source[0x988..0x98c], &0x3f10_0004u32.to_be_bytes());
        assert_eq!(&source[0x98c..0x990], &0x3f00_0000u32.to_be_bytes());
        assert_eq!(&source[H5EP_MIN_BYTES..], b"TYPE10-SYNTHETIC-SUFFIX");
        for (choice, expected) in [
            (HdType10Choice::Choice0, expected0),
            (HdType10Choice::Choice1, expected1),
        ] {
            assert_eq!(source.len(), expected.len());
            for (i, (before, after)) in source.iter().zip(&expected).enumerate() {
                if !(0x9dc..0x9e0).contains(&i) {
                    assert_eq!(before, after, "unexpected vector byte change at {i:#x}");
                }
            }
            let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
            preset
                .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), choice)
                .unwrap();
            assert_eq!(preset.as_bytes(), expected);
        }
        let mut p = OpaqueH5epPreset::parse(source).unwrap();
        p.set_hd_type10_choice(HdFxSlot::new(5).unwrap(), HdType10Choice::Choice1)
            .unwrap();
        assert_eq!(
            &p.as_bytes()[0x988..0x990],
            &[0x3f, 0x10, 0x00, 0x04, 0x3f, 0x00, 0x00, 0x00]
        );
        let mut no_op = p.clone();
        let before = no_op.as_bytes().to_vec();
        no_op
            .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), HdType10Choice::Choice1)
            .unwrap();
        assert_eq!(
            no_op.as_bytes(),
            before,
            "same-word write is byte-identical"
        );
    }

    #[test]
    fn second_type10_association_matches_complete_synthetic_vectors() {
        let source = decode_hex_fixture(include_str!(
            "../tests/fixtures/type10-choice-000f-source.hex"
        ));
        let expected0 = decode_hex_fixture(include_str!(
            "../tests/fixtures/type10-choice-000f-choice-0.hex"
        ));
        let expected1 = decode_hex_fixture(include_str!(
            "../tests/fixtures/type10-choice-000f-choice-1.hex"
        ));
        assert_eq!(&source[0x950..0x954], &0x020a_000fu32.to_be_bytes());
        assert_eq!(&source[0x9d8..0x9dc], &HD_TYPE10_CHOICE_KEY.to_be_bytes());
        assert_eq!(&source[0x988..0x98c], &0x3f10_0004u32.to_be_bytes());
        for (choice, expected) in [
            (HdType10Choice::Choice0, expected0),
            (HdType10Choice::Choice1, expected1),
        ] {
            let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
            preset
                .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), choice)
                .unwrap();
            assert_eq!(preset.as_bytes(), expected);
        }
    }
}

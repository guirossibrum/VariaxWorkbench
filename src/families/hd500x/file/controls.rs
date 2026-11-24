use super::{HD_EFFECT_COUNT, ToneError};

pub(super) const HD_PARAMETER_ROW_COUNT: usize = 12;
pub(super) const HD_EMPTY_EFFECT_MODEL_ID: u32 = 0x0200_0000;
pub(super) const HD_SCREAMER_MODEL_ID: u32 = 0x0205_000e;
pub(super) const HD_SUB_OCTAVE_FUZZ_MODEL_ID: u32 = 0x0205_0014;
pub(super) const HD_TUBE_DRIVE_MODEL_ID: u32 = 0x0205_000d;
pub(super) const HD_OCTAVE_FUZZ_MODEL_ID: u32 = 0x0205_000c;
pub const HD_TYPE9_CHOICE_MODEL_ID: u32 = 0x020a_0014;
pub const HD_TYPE8_CHOICE_MODEL_ID: u32 = 0x020a_0013;
pub const HD_TYPE8_CHOICE_KEY: u32 = 0x3f10_0004;
pub const HD_TYPE10_CHOICE_MODEL_ID: u32 = 0x020a_0010;
pub const HD_TYPE10_CHOICE_KEY: u32 = 0x3f10_0003;
/// Single source-admitted type-12 choice association. This is not a general
/// model/key catalog and does not imply a file FX slot.
pub const HD_TYPE12_CHOICE_MODEL_ID: u32 = 0x020a_001e;
pub const HD_TYPE12_CHOICE_KEY: u32 = 0x3f10_0004;
/// Static-derived type-0x0D choice association. This is not a hardware or
/// localized catalog identity.
pub const HD_TYPE13_CHOICE_MODEL_ID: u32 = 0x020a_001d;
pub const HD_TYPE13_CHOICE_KEY: u32 = 0x3f10_0003;
/// Static-derived descriptor type 0x13 (decimal 19) candidate association.
/// This is not a hardware or localized catalog identity.
pub const HD_TYPE19_CHOICE_MODEL_ID: u32 = 0x020a_0011;
pub const HD_TYPE19_CHOICE_KEY: u32 = 0x3f10_0003;
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

/// Closed identity for the sole admitted type-12 association.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType12Control {
    Model020a001eKey3f100004,
}

impl HdType12Control {
    pub const ALL: [Self; 1] = [Self::Model020a001eKey3f100004];

    pub fn for_model(model_id: u32) -> &'static [Self] {
        if model_id == HD_TYPE12_CHOICE_MODEL_ID {
            &Self::ALL
        } else {
            &[]
        }
    }

    pub const fn model_id(self) -> u32 {
        match self {
            Self::Model020a001eKey3f100004 => HD_TYPE12_CHOICE_MODEL_ID,
        }
    }

    pub const fn key(self) -> u32 {
        match self {
            Self::Model020a001eKey3f100004 => HD_TYPE12_CHOICE_KEY,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Model020a001eKey3f100004 => "Control key 0x3F100004",
        }
    }
}

/// One of the exact two neutral type-12 choices. Labels/units remain unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType12Choice {
    Choice0,
    Choice1,
}

impl HdType12Choice {
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

    pub(super) const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3f80_0000,
        }
    }

    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            _ => Err(ToneError::format("HD type-12 choice index must be 0 or 1")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType12ChoiceValue {
    Choice(HdType12Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}

/// Closed identity for the single static-derived type-0x0D association.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType13Control {
    Model020a001dKey3f100003,
}

impl HdType13Control {
    pub const ALL: [Self; 1] = [Self::Model020a001dKey3f100003];

    pub fn for_model(model_id: u32) -> &'static [Self] {
        if model_id == HD_TYPE13_CHOICE_MODEL_ID {
            &Self::ALL
        } else {
            &[]
        }
    }

    pub const fn model_id(self) -> u32 {
        HD_TYPE13_CHOICE_MODEL_ID
    }
    pub const fn key(self) -> u32 {
        HD_TYPE13_CHOICE_KEY
    }
    pub const fn label(self) -> &'static str {
        "Control key 0x3F100003"
    }
}

/// Four static-derived neutral choices. Words include the intermediate f32
/// step rounding observed in the retained converter trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType13Choice {
    Choice0,
    Choice1,
    Choice2,
    Choice3,
}

impl HdType13Choice {
    pub const ALL: [Self; 4] = [Self::Choice0, Self::Choice1, Self::Choice2, Self::Choice3];
    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
            Self::Choice2 => 2,
            Self::Choice3 => 3,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
            Self::Choice2 => "Choice 2",
            Self::Choice3 => "Choice 3",
        }
    }
    pub const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3eaa_aaab,
            Self::Choice2 => 0x3f2a_aaab,
            Self::Choice3 => 0x3f80_0000,
        }
    }
    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        match index {
            0 => Ok(Self::Choice0),
            1 => Ok(Self::Choice1),
            2 => Ok(Self::Choice2),
            3 => Ok(Self::Choice3),
            _ => Err(ToneError::format(
                "HD type-13 choice index must be in 0..=3",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType13ChoiceValue {
    Choice(HdType13Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}

/// Closed identity for the single static-derived descriptor type-0x13
/// (decimal 19) association. Kept distinct from `HdType13Control`, which is
/// decimal type 13.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType19Control {
    Model020a0011Key3f100003,
}

impl HdType19Control {
    pub const ALL: [Self; 1] = [Self::Model020a0011Key3f100003];

    pub fn for_model(model_id: u32) -> &'static [Self] {
        if model_id == HD_TYPE19_CHOICE_MODEL_ID {
            &Self::ALL
        } else {
            &[]
        }
    }

    pub const fn model_id(self) -> u32 {
        HD_TYPE19_CHOICE_MODEL_ID
    }

    pub const fn key(self) -> u32 {
        HD_TYPE19_CHOICE_KEY
    }

    pub const fn label(self) -> &'static str {
        "Control key 0x3F100003"
    }
}

/// Eight static-derived neutral choices for descriptor type 0x13 (decimal 19).
/// The 0.125 step is exact in binary32; these are still static candidates, not
/// observed runtime or physical values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HdType19Choice {
    Choice0,
    Choice1,
    Choice2,
    Choice3,
    Choice4,
    Choice5,
    Choice6,
    Choice7,
}

impl HdType19Choice {
    pub const ALL: [Self; 8] = [
        Self::Choice0,
        Self::Choice1,
        Self::Choice2,
        Self::Choice3,
        Self::Choice4,
        Self::Choice5,
        Self::Choice6,
        Self::Choice7,
    ];

    pub const fn index(self) -> u8 {
        match self {
            Self::Choice0 => 0,
            Self::Choice1 => 1,
            Self::Choice2 => 2,
            Self::Choice3 => 3,
            Self::Choice4 => 4,
            Self::Choice5 => 5,
            Self::Choice6 => 6,
            Self::Choice7 => 7,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice0 => "Choice 0",
            Self::Choice1 => "Choice 1",
            Self::Choice2 => "Choice 2",
            Self::Choice3 => "Choice 3",
            Self::Choice4 => "Choice 4",
            Self::Choice5 => "Choice 5",
            Self::Choice6 => "Choice 6",
            Self::Choice7 => "Choice 7",
        }
    }

    pub const fn bits(self) -> u32 {
        match self {
            Self::Choice0 => 0x0000_0000,
            Self::Choice1 => 0x3e00_0000,
            Self::Choice2 => 0x3e80_0000,
            Self::Choice3 => 0x3ec0_0000,
            Self::Choice4 => 0x3f00_0000,
            Self::Choice5 => 0x3f20_0000,
            Self::Choice6 => 0x3f40_0000,
            Self::Choice7 => 0x3f60_0000,
        }
    }

    pub fn from_index(index: u8) -> Result<Self, ToneError> {
        Self::ALL.get(index as usize).copied().ok_or_else(|| {
            ToneError::format("HD descriptor type-0x13 choice index must be in 0..=7")
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdType19ChoiceValue {
    Choice(HdType19Choice),
    UnknownFinite(u32),
    NonFinite(u32),
    Unavailable,
}
pub(super) const HD_BAND_SHIFT_EQ_MODEL_ID: u32 = 0x020c_000c;
pub(super) const HD_GRAPHIC_EQ_MODEL_ID: u32 = 0x020c_0009;
pub(super) const HD_STUDIO_EQ_MODEL_ID: u32 = 0x020c_000a;
const HD_HARD_GATE_MODEL_ID: u32 = 0x0200_0013;
pub(super) const HD_STEREO_DELAY_MODEL_ID: u32 = 0x0202_0013;
pub(super) const HD_FX_LOOP_MODEL_ID: u32 = 0x0208_0000;
pub(super) const HD_TYPE1_DRIVE_KEY: u32 = 0x3f10_0001;
pub(super) const HD_TYPE1_BASS_KEY: u32 = 0x3f10_0002;
pub(super) const HD_TYPE1_THIRD_KEY: u32 = 0x3f10_0003;
pub(super) const HD_TYPE1_TREBLE_KEY: u32 = 0x3f10_0004;
pub(super) const HD_TYPE1_OUTPUT_KEY: u32 = 0x3f10_0005;
pub(super) const HD_VINTAGE_PRE_MODEL_ID: u32 = 0x020c_000e;
pub(super) const HD_NOISE_GATE_MODEL_ID: u32 = 0x0200_0011;
pub(super) const HD_NOISE_GATE_DEFAULT_KEYS: [u32; 2] = [0x3f10_0001, 0x3f10_0002];
const HD_HARD_GATE_DEFAULT_KEYS: [u32; 4] = [0x3f10_0000, 0x3f10_0001, 0x3f10_0002, 0x3f10_0003];
const HD_BOOST_COMP_DEFAULT_KEYS: [u32; 5] = [
    0x3f10_0001,
    0x3f10_0002,
    0x3f10_0003,
    0x3f10_0004,
    0x3f10_0005,
];
const HD_TWO_ROW_DEFAULT_KEYS: [u32; 2] = [0x3f10_0001, 0x3f10_0002];
const HD_BLUE_COMP_TREB_DEFAULT_KEYS: [u32; 2] = [0x3f10_0002, 0x3f10_0001];
pub(super) const HD_VINTAGE_PRE_DEFAULT_KEYS: [u32; 5] = [
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

/// One of the exact eight FX defaults admitted for the explicit Octave Fuzz
/// replacement action. This is not a general model catalog or arbitrary ID
/// writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdDynamicsDefault {
    HardGate,
    BoostComp,
    RedComp,
    BlueComp,
    BlueCompTreb,
    VettaComp,
    VettaJuice,
    TubeComp,
}

impl HdDynamicsDefault {
    pub const ALL: [Self; 8] = [
        Self::HardGate,
        Self::BoostComp,
        Self::RedComp,
        Self::BlueComp,
        Self::BlueCompTreb,
        Self::VettaComp,
        Self::VettaJuice,
        Self::TubeComp,
    ];

    pub const fn model_id(self) -> u32 {
        match self {
            Self::HardGate => 0x0200_0013,
            Self::BoostComp => 0x0200_0012,
            Self::RedComp => 0x0200_000b,
            Self::BlueComp => 0x0200_000c,
            Self::BlueCompTreb => 0x0200_000d,
            Self::VettaComp => 0x0200_000f,
            Self::VettaJuice => 0x0200_0010,
            Self::TubeComp => 0x0200_000e,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::HardGate => "Hard Gate",
            Self::BoostComp => "Boost Comp",
            Self::RedComp => "Red Comp",
            Self::BlueComp => "Blue Comp",
            Self::BlueCompTreb => "Blue Comp Treb",
            Self::VettaComp => "Vetta Comp",
            Self::VettaJuice => "Vetta Juice",
            Self::TubeComp => "Tube Comp",
        }
    }

    pub const fn default_keys(self) -> &'static [u32] {
        match self {
            Self::HardGate => &HD_HARD_GATE_DEFAULT_KEYS,
            Self::BoostComp => &HD_BOOST_COMP_DEFAULT_KEYS,
            Self::RedComp
            | Self::BlueComp
            | Self::VettaComp
            | Self::VettaJuice
            | Self::TubeComp => &HD_TWO_ROW_DEFAULT_KEYS,
            Self::BlueCompTreb => &HD_BLUE_COMP_TREB_DEFAULT_KEYS,
        }
    }
}

/// One of the exact eight reverb defaults admitted for the explicit
/// Octave Fuzz replacement action. This remains separate from the dynamics
/// set even where source parameter keys overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdReverbDefault {
    Octo,
    Ducking,
    Plate,
    Room,
    Chamber,
    Hall,
    Cave,
    Tile,
}

impl HdReverbDefault {
    pub const ALL: [Self; 8] = [
        Self::Octo,
        Self::Ducking,
        Self::Plate,
        Self::Room,
        Self::Chamber,
        Self::Hall,
        Self::Cave,
        Self::Tile,
    ];

    pub const fn model_id(self) -> u32 {
        match self {
            Self::Octo => 0x0204_0023,
            Self::Ducking => 0x0204_0022,
            Self::Plate => 0x0204_001e,
            Self::Room => 0x0204_001f,
            Self::Chamber => 0x0204_0020,
            Self::Hall => 0x0204_0021,
            Self::Cave => 0x0204_0024,
            Self::Tile => 0x0204_0025,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Octo => "Octo",
            Self::Ducking => "Ducking",
            Self::Plate => "Plate",
            Self::Room => "Room",
            Self::Chamber => "Chamber",
            Self::Hall => "Hall",
            Self::Cave => "Cave",
            Self::Tile => "Tile",
        }
    }

    pub const fn default_keys(self) -> &'static [u32] {
        match self {
            Self::Octo
            | Self::Ducking
            | Self::Plate
            | Self::Room
            | Self::Chamber
            | Self::Hall
            | Self::Cave
            | Self::Tile => &HD_REVERB_DEFAULT_KEYS,
        }
    }
}

const HD_REVERB_DEFAULT_KEYS: [u32; 4] = [0x3f10_0001, 0x3f10_0002, 0x3f10_0003, 0x3f10_0005];
// Retained Editor 2.27 source evidence identifies Screamer's five defaults
// in ascending key order, with selector 1 and zero value bits for each row.
pub(super) const HD_SCREAMER_DEFAULT_KEYS: [u32; 5] = [
    HD_TYPE1_DRIVE_KEY,
    HD_TYPE1_BASS_KEY,
    HD_TYPE1_THIRD_KEY,
    HD_TYPE1_TREBLE_KEY,
    HD_TYPE1_OUTPUT_KEY,
];
// Editor 2.27's Sub Octave Fuzz descriptor initializes the same five ordered
// keys as Screamer, with selector 1 and zero selected value bits.
pub(super) const HD_SUB_OCTAVE_FUZZ_DEFAULT_KEYS: [u32; 5] = [
    HD_TYPE1_DRIVE_KEY,
    HD_TYPE1_BASS_KEY,
    HD_TYPE1_THIRD_KEY,
    HD_TYPE1_TREBLE_KEY,
    HD_TYPE1_OUTPUT_KEY,
];
/// One of the eight editable HD FX positions. This file-only slot type is
/// independent of vendor runtime slot numbers (which are four through eleven).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HdFxSlot(pub(super) usize);

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

    pub(super) const fn bits(self) -> u32 {
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

    pub(super) const fn bits(self) -> u32 {
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
    pub(super) const fn bits(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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

    pub(super) const fn key(self) -> u32 {
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
    pub(super) model_id: u32,
    pub(super) model_label: &'static str,
    pub(super) label: &'static str,
    pub(super) key: u32,
}

impl HdType1MappedControl {
    pub(super) const fn new(
        model_id: u32,
        model_label: &'static str,
        label: &'static str,
        key: u32,
    ) -> Self {
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
    pub(super) model_id: u32,
    pub(super) model_label: &'static str,
    pub(super) label: &'static str,
    key: u32,
    raw_type: u8,
    pub(super) min: i32,
    pub(super) max: i32,
}

impl HdNumericControl {
    pub(super) const fn new(
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

    pub(super) const fn key(self) -> u32 {
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

pub(super) fn is_admitted_hd_enable_model(model_id: u32) -> bool {
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

pub(super) fn exact_drive_percent(value: f32) -> Option<u8> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let percent = (value * 100.0).round() as u8;
    ((percent as f32 / 100.0).to_bits() == value.to_bits()).then_some(percent)
}

pub(super) fn normalized_numeric_value(value: i32, min: i32, max: i32) -> f32 {
    (value - min) as f32 / (max - min) as f32
}

pub(super) fn exact_numeric_value(value: f32, min: i32, max: i32) -> Option<i32> {
    if min >= max || !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let width = (max - min) as f32;
    let integer = min + (value * width).round() as i32;
    (normalized_numeric_value(integer, min, max).to_bits() == value.to_bits()).then_some(integer)
}

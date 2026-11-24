/// PINF target for PODxt Live. Equal to `L6ProductConstants.PODXT_LIVE`.
pub const PODXT_LIVE_TONE_ID: u32 = 0x0003_000a;
/// Exact source-order AMP requests admitted by the bounded existing-slot operation.
pub const PODXT_AMP_MODEL_CHOICES: [PodAmpModelChoice; 43] = [
    PodAmpModelChoice::NoAmp,
    PodAmpModelChoice::Model2002e,
    PodAmpModelChoice::Line6Clean,
    PodAmpModelChoice::Model20005,
    PodAmpModelChoice::Model20006,
    PodAmpModelChoice::Model20007,
    PodAmpModelChoice::Model20008,
    PodAmpModelChoice::Model2000a,
    PodAmpModelChoice::Model2000c,
    PodAmpModelChoice::Model2000f,
    PodAmpModelChoice::Model20010,
    PodAmpModelChoice::Model20011,
    PodAmpModelChoice::Model20012,
    PodAmpModelChoice::Model20013,
    PodAmpModelChoice::Model20014,
    PodAmpModelChoice::Model20015,
    PodAmpModelChoice::Model20018,
    PodAmpModelChoice::Model20019,
    PodAmpModelChoice::Model2001a,
    PodAmpModelChoice::Model2001b,
    PodAmpModelChoice::Model2001d,
    PodAmpModelChoice::Model2001e,
    PodAmpModelChoice::Model20020,
    PodAmpModelChoice::Model20021,
    PodAmpModelChoice::Model20022,
    PodAmpModelChoice::Model20023,
    PodAmpModelChoice::Model20024,
    PodAmpModelChoice::Model20026,
    PodAmpModelChoice::Model20027,
    PodAmpModelChoice::Model20029,
    PodAmpModelChoice::Model2002a,
    PodAmpModelChoice::Model2002c,
    PodAmpModelChoice::Model2002d,
    PodAmpModelChoice::Model2003d,
    PodAmpModelChoice::Model2003c,
    PodAmpModelChoice::Model2003b,
    PodAmpModelChoice::Model20032,
    PodAmpModelChoice::Model2004a,
    PodAmpModelChoice::Model2004b,
    PodAmpModelChoice::Model2004c,
    PodAmpModelChoice::Model2004d,
    PodAmpModelChoice::Model2004e,
    PodAmpModelChoice::Model2004f,
];

/// Complete source-listed AMP current-ID facts, retained only for deciding
/// whether a stored current value may use the 41 newly admitted requests.
pub(super) const PODXT_AMP_SOURCE_MODEL_IDS: [u32; 107] = [
    0x0002_0000,
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

/// One source-admitted AMP model-id choice. This is not a device-support claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PodAmpModelChoice {
    NoAmp,
    Line6Clean,
    Model2002e,
    Model20005,
    Model20006,
    Model20007,
    Model20008,
    Model2000a,
    Model2000c,
    Model2000f,
    Model20010,
    Model20011,
    Model20012,
    Model20013,
    Model20014,
    Model20015,
    Model20018,
    Model20019,
    Model2001a,
    Model2001b,
    Model2001d,
    Model2001e,
    Model20020,
    Model20021,
    Model20022,
    Model20023,
    Model20024,
    Model20026,
    Model20027,
    Model20029,
    Model2002a,
    Model2002c,
    Model2002d,
    Model2003d,
    Model2003c,
    Model2003b,
    Model20032,
    Model2004a,
    Model2004b,
    Model2004c,
    Model2004d,
    Model2004e,
    Model2004f,
}

impl PodAmpModelChoice {
    pub const fn model_id(self) -> u32 {
        match self {
            Self::NoAmp => 0x0002_0000,
            Self::Line6Clean => 0x0002_0001,
            Self::Model2002e => 0x0002_002e,
            Self::Model20005 => 0x0002_0005,
            Self::Model20006 => 0x0002_0006,
            Self::Model20007 => 0x0002_0007,
            Self::Model20008 => 0x0002_0008,
            Self::Model2000a => 0x0002_000a,
            Self::Model2000c => 0x0002_000c,
            Self::Model2000f => 0x0002_000f,
            Self::Model20010 => 0x0002_0010,
            Self::Model20011 => 0x0002_0011,
            Self::Model20012 => 0x0002_0012,
            Self::Model20013 => 0x0002_0013,
            Self::Model20014 => 0x0002_0014,
            Self::Model20015 => 0x0002_0015,
            Self::Model20018 => 0x0002_0018,
            Self::Model20019 => 0x0002_0019,
            Self::Model2001a => 0x0002_001a,
            Self::Model2001b => 0x0002_001b,
            Self::Model2001d => 0x0002_001d,
            Self::Model2001e => 0x0002_001e,
            Self::Model20020 => 0x0002_0020,
            Self::Model20021 => 0x0002_0021,
            Self::Model20022 => 0x0002_0022,
            Self::Model20023 => 0x0002_0023,
            Self::Model20024 => 0x0002_0024,
            Self::Model20026 => 0x0002_0026,
            Self::Model20027 => 0x0002_0027,
            Self::Model20029 => 0x0002_0029,
            Self::Model2002a => 0x0002_002a,
            Self::Model2002c => 0x0002_002c,
            Self::Model2002d => 0x0002_002d,
            Self::Model2003d => 0x0002_003d,
            Self::Model2003c => 0x0002_003c,
            Self::Model2003b => 0x0002_003b,
            Self::Model20032 => 0x0002_0032,
            Self::Model2004a => 0x0002_004a,
            Self::Model2004b => 0x0002_004b,
            Self::Model2004c => 0x0002_004c,
            Self::Model2004d => 0x0002_004d,
            Self::Model2004e => 0x0002_004e,
            Self::Model2004f => 0x0002_004f,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NoAmp => "No Amp",
            Self::Line6Clean => "Line 6 Clean",
            _ => "AMP model",
        }
    }

    pub fn from_model_id(model_id: u32) -> Option<Self> {
        PODXT_AMP_MODEL_CHOICES
            .iter()
            .copied()
            .find(|choice| choice.model_id() == model_id)
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
/// The 106 non-No-Amp IDs from `GetAmpModelList`'s 107-entry `AMP_MODELS`
/// initializer. Scalar control definitions intentionally exclude No Amp.
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
/// Alternate persisted slot used by PODxt Reverb fallback tones.
pub const REVERB_FALLBACK_SLOT: u32 = 0x0005_0005;
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

/// Closed set of Reverb IDs admitted by the conservative offline selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PodReverbModelChoice(u32);

impl PodReverbModelChoice {
    pub const MODEL_IDS: [u32; 15] = REVERB_CONTROL_MODEL_IDS;

    pub const fn model_id(self) -> u32 {
        self.0
    }

    pub fn from_model_id(id: u32) -> Option<Self> {
        Self::MODEL_IDS.contains(&id).then_some(Self(id))
    }

    pub fn numeric_label(self) -> String {
        format!("Reverb {:08x}", self.0)
    }
}

impl TryFrom<u32> for PodReverbModelChoice {
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
pub(super) const PODXT_EQ_FREQUENCY_TABLES: [[f64; 9]; 4] = [
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
pub(super) const PODXT_EQ_GAIN_MIN: f32 = -12.8;
pub(super) const PODXT_EQ_GAIN_MAX: f32 = 12.6;
/// Four source-backed non-Bass PODxt reset frequencies/gains, as serialized f32 bits.
pub(super) const PODXT_EQ_RESET_WORDS: [u32; 8] = [
    0x3e00_0000,
    0x3f01_0204,
    0x3eec_0000,
    0x3f01_0204,
    0x3ef0_0000,
    0x3f01_0204,
    0x3f5c_0000,
    0x3f01_0204,
];

/// One of the four non-Bass PODxt Live EQ graph bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PodxtEqBand {
    One,
    Two,
    Three,
    Four,
}

/// Line 6 Edit model-enable controls stored as MINF body byte 11.
///
/// `L6SharcToneParamMap` handles `AMP_ENABLE` (`0x40010001`), `STOMP_ENABLE`
/// (`0x40030006`), `MOD_ENABLE` (`0x40040009`) and `DELAY_ENABLE`
/// (`0x40050009`) as the `0x40000000` enable flag. `SetParam` calls
/// `L6Model.SetEnabled` on the group model, and `WriteIFFContainer` writes
/// `(m_enabled ? 1 : 0)` at MINF byte 11. That write does not change the
/// model id, PARM records, or any other model. Amp and EQ already use this
/// byte. Stomp, Mod, and Delay use the same byte on their one existing model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PodModelEnable {
    Amp,
    Stomp,
    Mod,
    Delay,
    Eq,
}

impl PodModelEnable {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Amp => "Amp",
            Self::Stomp => "Stomp",
            Self::Mod => "Mod",
            Self::Delay => "Delay",
            Self::Eq => "EQ",
        }
    }

    pub(super) const fn slots(self) -> &'static [u32] {
        match self {
            Self::Amp => &[AMP_SLOT],
            Self::Stomp => &[STOMP_SLOT],
            // Same primary-or-fallback slots as Mod and Delay model selection.
            Self::Mod => &[MOD_SLOT, 0x0005_0003],
            Self::Delay => &[DELAY_SLOT, 0x0005_0004],
            Self::Eq => &[PODXT_EQ_SLOT],
        }
    }

    pub(super) const fn expected_model_id(self) -> Option<u32> {
        match self {
            Self::Amp | Self::Stomp | Self::Mod | Self::Delay => None,
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

    pub(super) const fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
            Self::Three => 2,
            Self::Four => 3,
        }
    }

    pub(super) const fn label(self) -> &'static str {
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
pub(super) const WHAMMY_INTERVAL_RESOLUTION: f32 = 63.75;
pub(super) const WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL: f32 = f32::from_bits(0x3c80_8081);
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

    pub(super) const fn position(self, model_id: u32) -> Option<u32> {
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

    pub(super) const fn position(self, model_id: u32) -> Option<u32> {
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

    pub(super) const fn position(self, model_id: u32) -> Option<u32> {
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

    pub(super) fn multiplier(self) -> Option<f32> {
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

    pub(super) const fn position(self, model_id: u32) -> Option<u32> {
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

    pub(super) fn from_float_bits(bits: u32) -> Option<Self> {
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
    pub(super) fn from_float_bits(bits: u32) -> Option<Self> {
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
    pub(super) fn from_float_bits(bits: u32) -> Option<Self> {
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
    pub(super) fn float_bits(self) -> u32 {
        association_float_bits(self.controller_value())
    }

    pub(super) fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

impl DelayBitsSelection {
    pub(super) fn float_bits(self) -> u32 {
        association_float_bits(self.controller_value())
    }

    pub(super) fn from_float_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.float_bits() == bits)
    }
}

pub(super) fn source_range_raw(displayed: f32, minimum: f32, maximum: f32) -> f32 {
    // L6RangeFormat: convert the stored float bounds to double, subtract the
    // minimum in double, but subtract max-min in float before promoting it.
    let numerator = f64::from(displayed) - f64::from(minimum);
    let span: f32 = maximum - minimum;
    (numerator / f64::from(span)) as f32
}

pub(super) fn delay_time_raw(milliseconds: f32) -> u32 {
    source_range_raw(milliseconds.clamp(20.0, 2000.0), 20.0, 2000.0).to_bits()
}

pub(super) fn delay_note_time_raw(choice: DelayNoteChoice, tempo: f32) -> u32 {
    let multiplier = choice.multiplier().expect("positive NOTE choice");
    let q = multiplier * 4.0_f32;
    let beat = q / tempo;
    let milliseconds = beat * 1000.0_f32;
    let displayed = milliseconds * 60.0_f32;
    source_range_raw(displayed.clamp(20.0, 2000.0), 20.0, 2000.0).to_bits()
}

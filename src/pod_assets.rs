#![forbid(unsafe_code)]
//! PODxt Live asset names and GPXT slot roles recovered from original code.
//!
//! `asset_name` is `L6Assets.GetAssetName(id, 196618)`. Both the mac and
//! Windows copies of that method return the same string for every id in
//! this table. The lookup uses the two-argument call (`bl` is false), so
//! the debug table is skipped. PODxt Live is `IsPODxt` and `IsSharcTone`,
//! and it is not Bass PODxt or Vetta, so the Vetta arm and the bass-only
//! arm are skipped. Ternaries in the shared switch use the false arm of
//! `bl2`, `bl4`, and `bl6`. Later switches still run when an earlier
//! switch does not return, including the POD 2.0 ordinals, the `0x01020000`
//! cabinet ids, the `BX-` model ids, and the acoustic-body ids. Those
//! strings are lookup results, not a claim that PODxt Live implements
//! every body. The runtime extra map is empty here and is not consulted.
//! An id this path does not return stays unnamed. The fallback phrase
//! "Unknown model" is not an asset name.
//!
//! Model id `0x7fffffff` is the asset name "Bypass". That is not the MINF
//! enabled byte. `L6ParamDefinition` ranges exist for Variax and Variax
//! Acoustic only, so this module does not define knob domains.

use std::fmt::Write as _;

/// Number of model ids whose mac and Windows names agree.
pub const ASSET_NAME_COUNT: usize = 348;

const ASSET_NAMES: &[(u32, &str)] = &[
    (0x00000000, "Tube Preamp"),
    (0x00000001, "POD Clean"),
    (0x00000002, "POD Crunch"),
    (0x00000003, "POD Drive"),
    (0x00000004, "POD Layer"),
    (0x00000005, "Small Tweed"),
    (0x00000006, "Tweed Blues"),
    (0x00000007, "Black Panel"),
    (0x00000008, "Modern Class A"),
    (0x00000009, "Brit Class A"),
    (0x0000000a, "Brit Blues"),
    (0x0000000b, "Brit Classic"),
    (0x0000000c, "Brit Hi Gain"),
    (0x0000000d, "Treadplate"),
    (0x0000000e, "Modern Hi Gain"),
    (0x0000000f, "Fuzz Box"),
    (0x00000010, "Jazz Clean"),
    (0x00000011, "Boutique 1"),
    (0x00000012, "Boutique 2"),
    (0x00000013, "Brit Class A 2"),
    (0x00000014, "Brit Class A 3"),
    (0x00000015, "Small Tweed 2"),
    (0x00000016, "Black Panel 2"),
    (0x00000017, "Boutique 3"),
    (0x00000018, "Cali Crunch 1"),
    (0x00000019, "Cali Crunch 2"),
    (0x0000001a, "Treadplate 2"),
    (0x0000001b, "Modern Hi Gain 2"),
    (0x0000001c, "Line 6 Twang"),
    (0x0000001d, "Line 6 Crunch"),
    (0x0000001e, "Line 6 Blues"),
    (0x0000001f, "Line 6 Insane"),
    (0x00020000, "No Amp"),
    (0x00020001, "Line 6 Clean"),
    (0x00020002, "CC-L6 Sparkle"),
    (0x00020003, "CC-L6 Twang"),
    (0x00020004, "CC-L6 Bayou"),
    (0x00020005, "Line 6 JTS-45"),
    (0x00020006, "Line 6 Class A"),
    (0x00020007, "Line 6 Mood"),
    (0x00020008, "Spinal Puppet"),
    (0x00020009, "MS-L6 Throttle"),
    (0x0002000a, "Line 6 Chem X"),
    (0x0002000b, "CC-L6 Purge"),
    (0x0002000c, "Line 6 Insane"),
    (0x0002000d, "MS-L6 Octone"),
    (0x0002000f, "Line 6 ACO 2"),
    (0x00020010, "Zen Master"),
    (0x00020011, "Small Tweed"),
    (0x00020012, "Tweed B-Man"),
    (0x00020013, "Tiny Tweed"),
    (0x00020014, "Blackface Lux"),
    (0x00020015, "Double Verb"),
    (0x00020016, "CC-Mini Double"),
    (0x00020017, "CC-Gibtone Expo"),
    (0x00020018, "Two-Tone"),
    (0x00020019, "Hiway 100"),
    (0x0002001a, "Plexi 45"),
    (0x0002001b, "Plexi Lead 100"),
    (0x0002001c, "CC-Brit Bass"),
    (0x0002001d, "Plexi Jump Lead"),
    (0x0002001e, "Plexi Variac"),
    (0x0002001f, "CC-Brit Major"),
    (0x00020020, "Brit J-800"),
    (0x00020021, "Brit JM Pre"),
    (0x00020022, "Match Chief"),
    (0x00020023, "Match D-30"),
    (0x00020024, "Treadplate Dual"),
    (0x00020025, "MS-Diamondplate"),
    (0x00020026, "Cali Crunch"),
    (0x00020027, "Jazz Clean"),
    (0x00020028, "CC-Silver Twelve"),
    (0x00020029, "Solo 100"),
    (0x0002002a, "Super O"),
    (0x0002002b, "CC-Super O Thunder"),
    (0x0002002c, "Class A-15"),
    (0x0002002d, "Class A-30 TB"),
    (0x0002002e, "Tube Preamp"),
    (0x0002002f, "MS-Bomber Uber"),
    (0x00020030, "CC-Bomber X-TC"),
    (0x00020031, "MS-Angel P-Ball"),
    (0x00020032, "Variax Acoustic"),
    (0x00020033, "CC-L6 Super Cln"),
    (0x00020034, "CC-L6 SuperSpark"),
    (0x00020035, "MS-L6 Sparkle Cln"),
    (0x00020036, "CC-L6 Crunch"),
    (0x00020037, "MS-L6 Smash"),
    (0x00020038, "MS-L6 Fuzz"),
    (0x00020039, "MS-L6 Chunk Chunk"),
    (0x0002003a, "MS-L6 Big Bottom"),
    (0x0002003b, "Line 6 Treadplate"),
    (0x0002003c, "Line 6 Lunatic"),
    (0x0002003d, "Line 6 Agro"),
    (0x0002003e, "MS-Connor 50"),
    (0x0002003f, "CC-Deity Crunch"),
    (0x00020040, "MS-Deity Lead"),
    (0x00020041, "MS-Deity's Son"),
    (0x00020042, "CC-Blackface Vibro"),
    (0x00020043, "CC-Double Show"),
    (0x00020044, "CC-Silverface Bass"),
    (0x00020045, "MS-Brit Silver"),
    (0x00020046, "MS-Brit J-900 Cln"),
    (0x00020047, "MS-Brit J-900 Dst"),
    (0x00020048, "MS-Brit J-2000"),
    (0x00020049, "MS-Criminal"),
    (0x0002004a, "Citrus D-30"),
    (0x0002004b, "L6 Modern Hi Gain"),
    (0x0002004c, "L6 Boutique #1"),
    (0x0002004d, "Class A-30 Fawn"),
    (0x0002004e, "Brit Gain 18"),
    (0x0002004f, "Brit J-2000 #2"),
    (0x00030000, "BX-Bypass"),
    (0x00030001, "BX-Tube Preamp"),
    (0x00030002, "BX-L6 Classic Jazz"),
    (0x00030003, "BX-L6 Brit Invader"),
    (0x00030004, "BX-L6 Super Thor"),
    (0x00030005, "BX-L6 Frankenstein"),
    (0x00030006, "BX-L6 Ebony Lux"),
    (0x00030007, "BX-L6 Doppleganger"),
    (0x00030008, "BX-L6 Sub Dub"),
    (0x00030009, "BX-Amp 360"),
    (0x0003000a, "BX-Jaguar"),
    (0x0003000b, "BX-Alchemist"),
    (0x0003000c, "BX-Rock Classic"),
    (0x0003000d, "BX-Flip Top"),
    (0x0003000e, "BX-Adam and Eve"),
    (0x0003000f, "BX-Tweed B-Man"),
    (0x00030010, "BX-Silverface Bass"),
    (0x00030011, "BX-Double Show"),
    (0x00030012, "BX-Eighties"),
    (0x00030013, "BX-Hiway 100"),
    (0x00030014, "BX-Hiway 200"),
    (0x00030015, "BX-British Major"),
    (0x00030016, "BX-British Bass"),
    (0x00030017, "BX-California"),
    (0x00030018, "BX-Jazz Tone"),
    (0x00030019, "BX-Stadium"),
    (0x0003001a, "BX-Studio Tone"),
    (0x0003001b, "BX-Motor City"),
    (0x0003001c, "BX-Brit Class A100"),
    (0x01000000, "1x8 Tweed Champ"),
    (0x01000001, "1x12 Tweed Deluxe"),
    (0x01000002, "1x12 AC-15"),
    (0x01000003, "1x12 Blackface Deluxe"),
    (0x01000004, "1x12 Line 6"),
    (0x01000005, "2x12 Blackface Twin"),
    (0x01000006, "2x12 AC-30"),
    (0x01000007, "2x12 Chieftain"),
    (0x01000008, "2x12 Line 6"),
    (0x01000009, "4x10 Bassman"),
    (0x0100000a, "4x10 Line 6"),
    (0x0100000b, "4x12 Vintage 30s"),
    (0x0100000c, "4x12 Stock 70s"),
    (0x0100000d, "4x12 Greenbacks"),
    (0x0100000e, "4x12 Line 6"),
    (0x0100000f, "No Cabinet"),
    (0x01020000, "No Cab"),
    (0x01020001, "1x6 Super O"),
    (0x01020002, "1x8 Tweed"),
    (0x01020003, "1x10 Gibtone"),
    (0x01020004, "1x10 G-Brand"),
    (0x01020005, "1x12 Line 6"),
    (0x01020006, "1x12 Tweed"),
    (0x01020007, "1x12 Blackface"),
    (0x01020008, "1x12 Class A"),
    (0x01020009, "2x2 Mini T"),
    (0x0102000a, "2x12 Line 6"),
    (0x0102000b, "2x12 Blackface"),
    (0x0102000c, "2x12 Match"),
    (0x0102000d, "2x12 Jazz"),
    (0x0102000e, "2x12 Class A"),
    (0x0102000f, "4x10 Line 6"),
    (0x01020010, "4x10 Tweed"),
    (0x01020011, "4x12 Line 6"),
    (0x01020012, "4x12 Green 20's"),
    (0x01020013, "4x12 Green 25's"),
    (0x01020014, "4x12 Brit T75"),
    (0x01020015, "4x12 Brit V30's"),
    (0x01020016, "4x12 Treadplate"),
    (0x01020017, "1x12 California"),
    (0x01020018, "1x15 Thunder"),
    (0x01020019, "2x12 Zen Master"),
    (0x0102001a, "2x12 Wishbook"),
    (0x0102001b, "4x12 Hiway"),
    (0x0102001c, "4x12 Higain Solo"),
    (0x0102001d, "2x10 Vibro"),
    (0x0102001e, "4x12 X-Load"),
    (0x0102001f, "4x12 Big Bottom"),
    (0x01030000, "BX-No Cabinet"),
    (0x01030001, "BX-1x12 Boutique"),
    (0x01030002, "BX-1x12 Motor City"),
    (0x01030003, "BX-1x15 Flip Top"),
    (0x01030004, "BX-1x15 Jazz Tone"),
    (0x01030005, "BX-1x18 Session"),
    (0x01030006, "BX-1x18 Amp 360"),
    (0x01030007, "BX-1x18 California"),
    (0x01030008, "BX-1x18+12 Stadium"),
    (0x01030009, "BX-2x10 Modern UK"),
    (0x0103000a, "BX-2x15 Doubleshow"),
    (0x0103000b, "BX-2x15 California"),
    (0x0103000c, "BX-2x15 Class A"),
    (0x0103000d, "BX-4x10 Line 6"),
    (0x0103000e, "BX-4x10 Tweed"),
    (0x0103000f, "BX-4x10 Adam Eve"),
    (0x01030010, "BX-4x10 Silvercone"),
    (0x01030011, "BX-4x10 Session"),
    (0x01030012, "BX-4x12 Hiway"),
    (0x01030013, "BX-4x12 Green 20's"),
    (0x01030014, "BX-4x12 Green 25's"),
    (0x01030015, "BX-4x15 Big Boy"),
    (0x01030016, "BX-8x10 Classic"),
    (0x02000000, "Compressor"),
    (0x02000001, "NoiseGate"),
    (0x02000002, "Blue Comp"),
    (0x02000003, "FX-Blue Comp Treb"),
    (0x02000004, "Red Comp"),
    (0x02000005, "Vetta Comp"),
    (0x02000006, "FX-Vetta Juice"),
    (0x02000007, "Auto Swell"),
    (0x02000008, "Auto Swell"),
    (0x02020000, "Delay"),
    (0x02020001, "PingPong"),
    (0x02020002, "Analog Delay"),
    (0x02020003, "Analog w/Mod"),
    (0x02020004, "Tube Echo"),
    (0x02020005, "Multi-Head"),
    (0x02020006, "Sweep Echo"),
    (0x02020007, "Digital Delay"),
    (0x02020008, "Stereo Delay"),
    (0x02020009, "Ping Pong"),
    (0x0202000a, "Reverse"),
    (0x0202000b, "FX-Tape Echo"),
    (0x0202000c, "FX-Echo Platter"),
    (0x0202000d, "FX-Low Rez"),
    (0x0202000e, "FX-Phaze Eko"),
    (0x0202000f, "FX-Bubble Echo"),
    (0x02020010, "Ducking Delay"),
    (0x02030000, "Chorus 1"),
    (0x02030001, "Flanger 1"),
    (0x02030002, "Rotary"),
    (0x02030003, "Tremolo"),
    (0x02030004, "Sine Chorus"),
    (0x02030005, "Analog Chorus"),
    (0x02030006, "Line 6 Flanger"),
    (0x02030007, "Jet Flanger"),
    (0x02030008, "Phaser"),
    (0x02030009, "U-Vibe"),
    (0x0203000a, "Opto Trem"),
    (0x0203000b, "Bias Trem"),
    (0x0203000c, "RotaryDrum+Horn"),
    (0x0203000d, "Rotary Drum"),
    (0x0203000e, "Auto Pan"),
    (0x0203000f, "Chorus 2"),
    (0x02030010, "Flanger 2"),
    (0x02030011, "FX-Lumpy Phase"),
    (0x02030012, "FX-Square Chorus"),
    (0x02030013, "FX-Expo Flange"),
    (0x02030014, "FX-Random Chorus"),
    (0x02030015, "FX-Analog Square"),
    (0x02030016, "Stereo Chorus"),
    (0x02030017, "FX-POD Purple X"),
    (0x02030018, "FX-Random S/H"),
    (0x02030019, "FX-Tape Eater"),
    (0x0203001a, "FX-Hi-Talk"),
    (0x0203001b, "FX-Sweeper"),
    (0x0203001c, "FX-Warble-Matic"),
    (0x0203001e, "FX-Expo Chorus"),
    (0x0203001f, "FX-Square Flange"),
    (0x02040000, "Room"),
    (0x02040001, "Spring"),
    (0x02040002, "Lux Spring"),
    (0x02040003, "Std Spring"),
    (0x02040004, "King Spring"),
    (0x02040005, "Small Room"),
    (0x02040006, "Tiled Room"),
    (0x02040007, "Brite Room"),
    (0x02040008, "Dark Hall"),
    (0x02040009, "Medium Hall"),
    (0x0204000a, "Large Hall"),
    (0x0204000b, "Rich Chamber"),
    (0x0204000c, "Chamber"),
    (0x0204000d, "Cavernous"),
    (0x0204000e, "Slap Plate"),
    (0x0204000f, "Vintage Plate"),
    (0x02040010, "Large Plate"),
    (0x02040011, "Large Room"),
    (0x02040012, "Studio 6"),
    (0x02040013, "Memphis Plate"),
    (0x02040014, "Foil Plate"),
    (0x02040015, "Blue Plate"),
    (0x02040016, "Bingo Hall"),
    (0x02040017, "Concert Hall"),
    (0x02040018, "War Memorial"),
    (0x02040019, "Hangar 18"),
    (0x0204001a, "Propellerverb"),
    (0x0204001b, "Radioverb"),
    (0x02050000, "Facial Fuzz"),
    (0x02050001, "FX-Tube Drive"),
    (0x02050002, "Fuzz Pi"),
    (0x02050003, "Screamer"),
    (0x02050004, "Octave Fuzz"),
    (0x02050005, "Classic Dist"),
    (0x02050006, "FX-Killer Z"),
    (0x02050007, "FX-Boost + EQ"),
    (0x02050008, "Bass Overdrive"),
    (0x02050009, "Bronze Master"),
    (0x02060000, "Vetta Wah"),
    (0x02060002, "Vetta Wah"),
    (0x02060003, "Fassel"),
    (0x02060004, "Weeper"),
    (0x02060005, "Chrome"),
    (0x02060006, "Chrome Custom"),
    (0x02060007, "Throaty"),
    (0x02060008, "Conductor"),
    (0x02060009, "Colorful"),
    (0x0206000a, "Custom"),
    (0x020a0000, "Auto Wah"),
    (0x020a0001, "FX-Synth Lead"),
    (0x020a0002, "FX-Synth String"),
    (0x020a0003, "FX-Synth Analog"),
    (0x020a0004, "FX-Synth FX"),
    (0x020a0005, "FX-Buzz Wave"),
    (0x020a0006, "FX-Rez Synth"),
    (0x020a0007, "FX-Saturn 5 Ring M"),
    (0x020a0008, "FX-Double Bass"),
    (0x020a0009, "FX-Synth Harmony"),
    (0x020a000a, "FX-Dingo-Tron"),
    (0x020a000b, "FX-Clean Sweep"),
    (0x020a000c, "FX-Seismik Synth"),
    (0x020a000d, "Sub Octaves"),
    (0x020a000e, "Bender"),
    (0x04100000, "Parlor"),
    (0x04100001, "Triple O"),
    (0x04100002, "Dread"),
    (0x04100003, "Jumbo"),
    (0x04100004, "C & W"),
    (0x04100005, "Gypsy"),
    (0x04100006, "Jazz"),
    (0x04100007, "Nylon"),
    (0x04100008, "Folk 12"),
    (0x04100009, "Blues 12"),
    (0x0410000a, "Rnd Neck"),
    (0x0410000b, "Sqr Neck"),
    (0x0410000c, "Banjo"),
    (0x0410000d, "Mandola"),
    (0x0410000e, "Shamisen"),
    (0x0410000f, "Sitar"),
    (0x7fffffff, "Bypass"),
];

/// Name returned for this model id on PODxt Live, when both sources agree.
pub fn asset_name(id: u32) -> Option<&'static str> {
    ASSET_NAMES
        .binary_search_by_key(&id, |entry| entry.0)
        .ok()
        .map(|index| ASSET_NAMES[index].1)
}

/// `L6Assets.GetType` label for a named type constant. Other high bytes stay raw.
pub fn model_type_label(id: u32) -> Option<&'static str> {
    match id & 0xff00_0000 {
        0x0000_0000 => Some("Amp"),
        0x0100_0000 => Some("Cab"),
        0x0200_0000 => Some("FX"),
        0x0300_0000 => Some("Electric guitar"),
        0x0400_0000 => Some("Acoustic guitar"),
        0x0500_0000 => Some("Bass guitar"),
        0x3e00_0000 => Some("System"),
        0x3f00_0000 => Some("DSP parameter"),
        0x7d00_0000 => Some("Extras"),
        0x7e00_0000 => Some("Application"),
        0x7f00_0000 => Some("Undefined"),
        _ => None,
    }
}

/// Slot id from `kGPXTSlot_*` in `L6AssetDefs_const`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRole {
    System,
    NoiseGate,
    VolumePedalPre,
    Wah,
    StompPre,
    ModPre,
    DelayPre,
    ReverbPre,
    BassEqPre,
    SpSend34Pre,
    SynthPre,
    Reserved01,
    Reserved02,
    FxLoopPre,
    VariaxElectric,
    VariaxAcoustic,
    VariaxBass,
    Amp,
    Cab,
    Tremolo,
    AirEr,
    PostEq,
    Compressor,
    StompPost,
    VolumePedalPost,
    ModPost,
    DelayPost,
    ReverbPost,
    BassEqPost,
    SpSend34Post,
    SynthPost,
    FxLoopPost,
}

impl SlotRole {
    pub const fn id(self) -> u32 {
        match self {
            Self::System => 65_536,
            Self::NoiseGate => 131_072,
            Self::VolumePedalPre => 131_073,
            Self::Wah => 131_074,
            Self::StompPre => 131_075,
            Self::ModPre => 131_076,
            Self::DelayPre => 131_077,
            Self::ReverbPre => 131_078,
            Self::BassEqPre => 131_079,
            Self::SpSend34Pre => 131_080,
            Self::SynthPre => 131_081,
            Self::Reserved01 => 131_082,
            Self::Reserved02 => 131_083,
            Self::FxLoopPre => 131_084,
            Self::VariaxElectric => 131_085,
            Self::VariaxAcoustic => 131_086,
            Self::VariaxBass => 131_087,
            Self::Amp => 196_608,
            Self::Cab => 196_609,
            Self::Tremolo => 196_610,
            Self::AirEr => 196_611,
            Self::PostEq => 196_612,
            Self::Compressor => 327_680,
            Self::StompPost => 327_681,
            Self::VolumePedalPost => 327_682,
            Self::ModPost => 327_683,
            Self::DelayPost => 327_684,
            Self::ReverbPost => 327_685,
            Self::BassEqPost => 327_686,
            Self::SpSend34Post => 327_687,
            Self::SynthPost => 327_688,
            Self::FxLoopPost => 327_689,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::NoiseGate => "Noise Gate",
            Self::VolumePedalPre => "Volume Pedal Pre",
            Self::Wah => "Wah",
            Self::StompPre => "Stomp Pre",
            Self::ModPre => "Mod Pre",
            Self::DelayPre => "Delay Pre",
            Self::ReverbPre => "Reverb Pre",
            Self::BassEqPre => "Bass EQ Pre",
            Self::SpSend34Pre => "SPSend34 Pre",
            Self::SynthPre => "Synth Pre",
            Self::Reserved01 => "Reserved 01",
            Self::Reserved02 => "Reserved 02",
            Self::FxLoopPre => "FX Loop Pre",
            Self::VariaxElectric => "Variax Electric",
            Self::VariaxAcoustic => "Variax Acoustic",
            Self::VariaxBass => "Variax Bass",
            Self::Amp => "Amp",
            Self::Cab => "Cab",
            Self::Tremolo => "Tremolo",
            Self::AirEr => "AIR ER",
            Self::PostEq => "Post EQ",
            Self::Compressor => "Compressor",
            Self::StompPost => "Stomp Post",
            Self::VolumePedalPost => "Volume Pedal Post",
            Self::ModPost => "Mod Post",
            Self::DelayPost => "Delay Post",
            Self::ReverbPost => "Reverb Post",
            Self::BassEqPost => "Bass EQ Post",
            Self::SpSend34Post => "SPSend34 Post",
            Self::SynthPost => "Synth Post",
            Self::FxLoopPost => "FX Loop Post",
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        SLOT_ROLES
            .binary_search_by_key(&id, |entry| entry.0)
            .ok()
            .map(|index| SLOT_ROLES[index].1)
    }
}

const SLOT_ROLES: &[(u32, SlotRole)] = &[
    (65_536, SlotRole::System),
    (131_072, SlotRole::NoiseGate),
    (131_073, SlotRole::VolumePedalPre),
    (131_074, SlotRole::Wah),
    (131_075, SlotRole::StompPre),
    (131_076, SlotRole::ModPre),
    (131_077, SlotRole::DelayPre),
    (131_078, SlotRole::ReverbPre),
    (131_079, SlotRole::BassEqPre),
    (131_080, SlotRole::SpSend34Pre),
    (131_081, SlotRole::SynthPre),
    (131_082, SlotRole::Reserved01),
    (131_083, SlotRole::Reserved02),
    (131_084, SlotRole::FxLoopPre),
    (131_085, SlotRole::VariaxElectric),
    (131_086, SlotRole::VariaxAcoustic),
    (131_087, SlotRole::VariaxBass),
    (196_608, SlotRole::Amp),
    (196_609, SlotRole::Cab),
    (196_610, SlotRole::Tremolo),
    (196_611, SlotRole::AirEr),
    (196_612, SlotRole::PostEq),
    (327_680, SlotRole::Compressor),
    (327_681, SlotRole::StompPost),
    (327_682, SlotRole::VolumePedalPost),
    (327_683, SlotRole::ModPost),
    (327_684, SlotRole::DelayPost),
    (327_685, SlotRole::ReverbPost),
    (327_686, SlotRole::BassEqPost),
    (327_687, SlotRole::SpSend34Post),
    (327_688, SlotRole::SynthPost),
    (327_689, SlotRole::FxLoopPost),
];

/// One displayed control line. Names and roles come from the tables above.
pub fn describe_model(
    slot: u32,
    model_id: u32,
    ordinal: Option<u8>,
    reserved: Option<[u8; 2]>,
    enabled: Option<bool>,
) -> String {
    let mut line = match SlotRole::from_id(slot) {
        Some(role) => format!("{} slot {slot:08x}", role.label()),
        None => format!("no GPXT role slot {slot:08x}"),
    };
    match asset_name(model_id) {
        Some(name) => write!(line, " model {name} {model_id:08x}").expect("string write"),
        None => write!(line, " model {model_id:08x}").expect("string write"),
    }
    if let Some(type_label) = model_type_label(model_id) {
        write!(line, " type {type_label}").expect("string write");
    }
    if let Some(enabled) = enabled {
        line.push_str(if enabled { " enabled" } else { " disabled" });
    }
    if let Some(ordinal) = ordinal {
        write!(line, " ordinal {ordinal}").expect("string write");
    }
    if let Some([first, second]) = reserved {
        write!(line, " reserved {first:02x} {second:02x}").expect("string write");
    }
    line
}

/// Structural PARM text. No range or unit is added.
pub fn describe_parameter(prefix: u8, id: u32, value: &str) -> String {
    format!("parameter {id:06x} prefix {prefix:02x} {value} no range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreed_names_are_sorted_and_cover_the_known_collisions() {
        assert_eq!(ASSET_NAMES.len(), ASSET_NAME_COUNT);
        for pair in ASSET_NAMES.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        assert_eq!(asset_name(131_072), Some("No Amp"));
        assert_eq!(asset_name(131_073), Some("Line 6 Clean"));
        assert_eq!(asset_name(0x0002_004f), Some("Brit J-2000 #2"));
        assert_eq!(asset_name(0x0202_0002), Some("Analog Delay"));
        assert_eq!(asset_name(0x0202_000a), Some("Reverse"));
        assert_eq!(asset_name(0x0203_0004), Some("Sine Chorus"));
        assert_eq!(asset_name(0x0203_0006), Some("Line 6 Flanger"));
        assert_eq!(asset_name(0x0100_000f), Some("No Cabinet"));
        assert_eq!(asset_name(0x0103_0000), Some("BX-No Cabinet"));
        assert_eq!(asset_name(0x0003_0000), Some("BX-Bypass"));
        assert_eq!(asset_name(0x0003_000a), Some("BX-Jaguar"));
        assert_ne!(asset_name(0x0003_000a), Some("Jaguar"));
        assert_eq!(asset_name(0x0410_0000), Some("Parlor"));
        assert_eq!(asset_name(0x7fff_ffff), Some("Bypass"));
        assert_eq!(asset_name(0), Some("Tube Preamp"));
        assert_eq!(asset_name(0x1111_0002), None);
        assert_eq!(model_type_label(131_073), Some("Amp"));
        assert_eq!(model_type_label(0x0100_000f), Some("Cab"));
        assert_eq!(model_type_label(0x0205_0000), Some("FX"));
        assert_eq!(model_type_label(0x0410_0000), Some("Acoustic guitar"));
        assert_eq!(model_type_label(0x7fff_ffff), Some("Undefined"));
        assert_eq!(model_type_label(0x1111_0002), None);
    }

    #[test]
    fn gpxt_slot_ids_round_trip_and_unknown_slots_stay_unlisted() {
        assert_eq!(SLOT_ROLES.len(), 32);
        for pair in SLOT_ROLES.windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
        for (id, role) in SLOT_ROLES {
            assert_eq!(SlotRole::from_id(*id), Some(*role));
            assert_eq!(role.id(), *id);
            assert!(!role.label().is_empty());
        }
        assert_eq!(SlotRole::from_id(196_608), Some(SlotRole::Amp));
        assert_eq!(SlotRole::from_id(196_609), Some(SlotRole::Cab));
        assert_eq!(SlotRole::from_id(131_074), Some(SlotRole::Wah));
        assert_eq!(SlotRole::from_id(131_082), Some(SlotRole::Reserved01));
        assert_eq!(SlotRole::from_id(0x0003_00aa), None);
        assert_eq!(SlotRole::from_id(0x0201_0000), None);
    }
}

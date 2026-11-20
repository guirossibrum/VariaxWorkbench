#![forbid(unsafe_code)]
//! Static capability matrix.
//!
//! A caller-supplied claim never changes the result. Physical operations are
//! `Locked` for every profile.

use super::model::ProductProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    OpenGuitarPreset,
    OpenGuitarBank,
    SaveGuitarCopy,
    ApplyFamilyMath,
    OpenPodPreset,
    EditPodPreset,
    SelectRecordRoute,
    InspectFirmware,
    StartAudioCapture,
    StopAudioCapture,
    OpenAudioStream,
    ReadInstalledUsbComponent,
    FlashFirmware,
    ResetDevice,
    SwitchBank,
    EnumerateUsb,
    OpenMidi,
    SendControl,
    InstallModule,
    BindDriver,
}

impl Operation {
    pub const ALL: [Self; 20] = [
        Self::OpenGuitarPreset,
        Self::OpenGuitarBank,
        Self::SaveGuitarCopy,
        Self::ApplyFamilyMath,
        Self::OpenPodPreset,
        Self::EditPodPreset,
        Self::SelectRecordRoute,
        Self::InspectFirmware,
        Self::StartAudioCapture,
        Self::StopAudioCapture,
        Self::OpenAudioStream,
        Self::ReadInstalledUsbComponent,
        Self::FlashFirmware,
        Self::ResetDevice,
        Self::SwitchBank,
        Self::EnumerateUsb,
        Self::OpenMidi,
        Self::SendControl,
        Self::InstallModule,
        Self::BindDriver,
    ];

    pub const fn is_physical(self) -> bool {
        matches!(
            self,
            Self::StartAudioCapture
                | Self::StopAudioCapture
                | Self::OpenAudioStream
                | Self::ReadInstalledUsbComponent
                | Self::FlashFirmware
                | Self::ResetDevice
                | Self::SwitchBank
                | Self::EnumerateUsb
                | Self::OpenMidi
                | Self::SendControl
                | Self::InstallModule
                | Self::BindDriver
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityState {
    /// A bounded offline path runs in this crate. It does not touch hardware.
    Offline,
    /// The interface exists. The behavior belongs to a later issue.
    Planned { issue: u16 },
    /// Physical operation. This value does not depend on session data.
    Locked,
    /// Extension profile. No codec, format, or version is registered.
    Unverified,
    /// The operation does not belong to this profile. There is no fallback.
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capability {
    pub profile: ProductProfile,
    pub operation: Operation,
    pub state: CapabilityState,
    pub note: &'static str,
}

/// `claim` is ignored. Registry rows and session fields cannot raise or clear a state.
pub fn resolve(
    profile: ProductProfile,
    operation: Operation,
    claim: Option<CapabilityState>,
) -> CapabilityState {
    let _ = claim;
    if operation.is_physical() {
        return CapabilityState::Locked;
    }
    declared(profile, operation)
}

pub fn capability(profile: ProductProfile, operation: Operation) -> Capability {
    Capability {
        profile,
        operation,
        state: resolve(profile, operation, None),
        note: note(profile, operation),
    }
}

fn declared(profile: ProductProfile, operation: Operation) -> CapabilityState {
    match profile {
        ProductProfile::PodHd500x if operation == Operation::InspectFirmware => {
            CapabilityState::Offline
        }
        ProductProfile::WorkbenchHd | ProductProfile::Jtv69 | ProductProfile::PodHd500x => {
            CapabilityState::Unverified
        }
        ProductProfile::Variax600 => variax_state(operation),
        ProductProfile::PodxtLive => pod_state(operation),
    }
}

fn variax_state(operation: Operation) -> CapabilityState {
    match operation {
        Operation::OpenGuitarPreset
        | Operation::OpenGuitarBank
        | Operation::SaveGuitarCopy
        | Operation::ApplyFamilyMath
        | Operation::InspectFirmware => CapabilityState::Offline,
        Operation::OpenPodPreset | Operation::EditPodPreset | Operation::SelectRecordRoute => {
            CapabilityState::Unsupported
        }
        Operation::StartAudioCapture
        | Operation::StopAudioCapture
        | Operation::OpenAudioStream
        | Operation::ReadInstalledUsbComponent
        | Operation::FlashFirmware
        | Operation::ResetDevice
        | Operation::SwitchBank
        | Operation::EnumerateUsb
        | Operation::OpenMidi
        | Operation::SendControl
        | Operation::InstallModule
        | Operation::BindDriver => CapabilityState::Locked,
    }
}

fn pod_state(operation: Operation) -> CapabilityState {
    match operation {
        Operation::InspectFirmware => CapabilityState::Offline,
        Operation::OpenPodPreset | Operation::EditPodPreset => CapabilityState::Offline,
        Operation::SelectRecordRoute => CapabilityState::Offline,
        Operation::OpenGuitarPreset
        | Operation::OpenGuitarBank
        | Operation::SaveGuitarCopy
        | Operation::ApplyFamilyMath => CapabilityState::Unsupported,
        Operation::StartAudioCapture
        | Operation::StopAudioCapture
        | Operation::OpenAudioStream
        | Operation::ReadInstalledUsbComponent
        | Operation::FlashFirmware
        | Operation::ResetDevice
        | Operation::SwitchBank
        | Operation::EnumerateUsb
        | Operation::OpenMidi
        | Operation::SendControl
        | Operation::InstallModule
        | Operation::BindDriver => CapabilityState::Locked,
    }
}

fn note(profile: ProductProfile, operation: Operation) -> &'static str {
    if profile == ProductProfile::PodHd500x && !operation.is_physical() {
        return hd500x_limit(operation);
    }
    match (profile, operation) {
        (ProductProfile::Variax600, Operation::OpenGuitarPreset) => {
            "Opens one original-family FORM/L6PA file in memory. Device id 0x00070004 only. Nothing is sent."
        }
        (ProductProfile::Variax600, Operation::OpenGuitarBank) => {
            "Browses one original-family FORM/L6BA or FORM/L6CO file. A preset with another device id is rejected."
        }
        (ProductProfile::Variax600, Operation::SaveGuitarCopy) => {
            "Writes a new guitar-file copy through the existing non-overwriting save. It does not upload."
        }
        (ProductProfile::Variax600, Operation::ApplyFamilyMath) => {
            "Identity gate for the original Variax math family. It does not execute the coefficient engine and does not accept another family's id."
        }
        (ProductProfile::Variax600, Operation::InspectFirmware) => {
            "Inspects a FORM/L6FF container and requires the archived Variax 300/600 product and version. MD5 agreement is not authentication."
        }
        (ProductProfile::Variax600, Operation::SelectRecordRoute) => {
            "USB audio belongs to the PODxt Live role on this rig. The guitar role has no recording route."
        }
        (ProductProfile::PodxtLive, Operation::InspectFirmware) => {
            "Inspects a FORM/L6FF container and requires the archived PODxt Live product and version. A Variax image is the wrong product."
        }
        (ProductProfile::PodxtLive, Operation::OpenPodPreset) => {
            "Opens one FORM/L6PA, FORM/L6FS, or FORM/L6AS tone whose PINF target is PODxt Live 0x0003000A. Amp slot 0x00030000 and, on L6AS only, cab slot 0x00030001 expose model ids. Known asset names and 12-byte parameter records are reported. No unit or range was recovered. Nothing is sent."
        }
        (ProductProfile::PodxtLive, Operation::EditPodPreset) => {
            "Renames the 32 UTF-16BE PINF name units and writes a new copy. Parameter values are not edited because no unit or range was recovered. The enabled byte, unknown chunks, and other bytes stay unchanged. Nothing is sent."
        }
        (ProductProfile::PodxtLive, Operation::SelectRecordRoute) => {
            "Selects one documented Record Send 1-2 mode in memory. It does not open a stream. Capture and playback stay locked. An offline test take can use this selection and still does not start capture."
        }
        (ProductProfile::PodxtLive, Operation::ApplyFamilyMath) => {
            "PODxt coefficients are not the Variax 600 engine. There is no cross-family fallback."
        }
        (_, operation) if operation.is_physical() => {
            "Physical operation. Locked for every profile, including a session claim that says otherwise. No live backend is constructed."
        }
        (profile, _) if profile.is_extension() => {
            "Extension profile. No codec, DSP, firmware format, transport, or version is registered."
        }
        _ => {
            "The static matrix rejects this operation for this profile. Another profile is not substituted."
        }
    }
}

fn hd500x_limit(operation: Operation) -> &'static str {
    match operation {
        Operation::OpenGuitarPreset | Operation::OpenGuitarBank | Operation::SaveGuitarCopy => {
            "No POD HD500X guitar-file identity is registered. Original Variax files are not converted."
        }
        Operation::ApplyFamilyMath => {
            "POD HD500X is a separate family from original Variax and from PODxt. No coefficient identity is registered."
        }
        Operation::OpenPodPreset => {
            "Offline library recognizes the statically traced Edit H5EP/H5ES/H5EB framing, validates the recovered body subset, and can extract child records. This file group does not prove physical target compatibility. A PODxt Live tone is not opened on this profile and is not converted."
        }
        Operation::EditPodPreset => {
            "Source-identified offline HD Edit operations cover 364 primary type-1 model/key rows across 108 descriptor models, 80 selected non-type-1 numeric controls across 65 descriptor models (444 controls across 143 descriptor model IDs), four exact type-9 three-choice model/key rows, four exact type-11 five-choice model/key rows, and one static-derived type-0x0D four-choice candidate (model 0x020A001D/key 0x3F100003). Type 0x13/decimal 19 remains deferred. Type-1 values use the editor integer-percent grid; numeric values use each descriptor's recovered primary-widget integer bounds and source-derived normalized float conversion, without assigning physical units. Type-9/type-11/type-0x0D choices use closed neutral enums and exact binary32 words; type-0x0D words include the statically traced intermediate f32 step rounding and are not a runtime parity claim. No localized names or physical units are inferred. The active tag 0x1B entry admits only the Vintage Pre primary key 0x3F100005 value on the 0..15000 editor grid; the auxiliary EDI widget and post-event root callback remain unresolved. H5EP/H5ES/H5EB association and edits do not establish a physical product target or complete editor parity. Other effect/control contracts, product-qualified maintenance and remaining source joins remain unavailable."
        }
        Operation::SelectRecordRoute => {
            "HD500X record routing remains Unverified. The Windows INF's DefaultSampleRate 48000, DefaultWordSize 24 and EngineFormatLock 0 are driver defaults; guide routing statements do not provide the product-qualified USB endpoint, format, timing and callback join required for an offline route. No stream is opened."
        }
        Operation::InspectFirmware => {
            "The HD500X session can run generic bounded FORM/L6FF structure inspection on the supplied P25 image. Central's separate metadata name mapping and native parser/loader trace do not resolve the PHF header product/version consumer join. The public Flash Memory v2.62 release 6260 listing does not establish that link or the separate USB Firmware 1.04 component/installed state."
        }
        Operation::StartAudioCapture
        | Operation::StopAudioCapture
        | Operation::OpenAudioStream
        | Operation::ReadInstalledUsbComponent
        | Operation::FlashFirmware
        | Operation::ResetDevice
        | Operation::SwitchBank
        | Operation::EnumerateUsb
        | Operation::OpenMidi
        | Operation::SendControl
        | Operation::InstallModule
        | Operation::BindDriver => {
            "Physical operation. Locked for every profile, including a session claim that says otherwise. No live backend is constructed."
        }
    }
}

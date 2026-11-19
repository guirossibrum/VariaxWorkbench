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
            "Offline HD Edit library supports five source-traced Screamer type-1 controls (DRIVE, BASS, MID, TREBLE, OUTPUT) on an exact editor integer-percent grid. This is normalized editor data, not physical units or device availability; other control contracts and full editor parity are unavailable. Preserved header context does not establish a product id; physical target compatibility and device behavior remain unverified."
        }
        Operation::SelectRecordRoute => {
            "The Windows INF DefaultSampleRate is 48000 and DefaultWordSize is 24, with EngineFormatLock 0. Those are driver defaults. They do not select a PODxt Live record route and do not open a stream."
        }
        Operation::InspectFirmware => {
            "The public listing names Flash Memory v2.62, release 6260, and the notes also name USB Firmware 1.04. Separately supplied private firmware-image material has been examined statically, but its release linkage/applicability is not admitted. FORM/L6FF inspection is not applied."
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

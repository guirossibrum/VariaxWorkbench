#![forbid(unsafe_code)]
//! Family-specific offline backends.
//!
//! Guitar files use the Workbench preset and bank parsers. Firmware uses the
//! FORM/L6FF inspector. PODxt Live tones use the separate offline librarian.
//! Recording and every physical transport stay out of these functions.

use std::path::Path;

use crate::bundle::BundleFile;
use crate::document;
use crate::firmware::{self, ApplicabilityError};
use crate::pod_preset::{self, ToneError};
use crate::preset::PresetFile;

use super::capability::Operation;
use super::model::{
    DeviceFamily, PODXT_LIVE_FILE_ID, ProductProfile, VARIAX_500_700_FILE_ID, VARIAX_600_FILE_ID,
};
use super::session::{
    FirmwareSummary, GuitarBankSummary, GuitarPresetSummary, PodToneSummary, Rejection,
};

pub(crate) fn open_guitar_preset(bytes: &[u8]) -> Result<GuitarPresetSummary, Rejection> {
    let preset = PresetFile::parse(bytes.to_vec()).map_err(Rejection::format)?;
    check_guitar_identity(preset.device_id(), Operation::OpenGuitarPreset)?;
    Ok(GuitarPresetSummary {
        name: preset.name().to_owned(),
        device_id: preset.device_id(),
        models: preset.models().len(),
    })
}

pub(crate) fn open_guitar_bank(bytes: &[u8]) -> Result<GuitarBankSummary, Rejection> {
    let bundle = BundleFile::parse(bytes.to_vec()).map_err(Rejection::format)?;
    let mut presets = 0;
    for bank in bundle.banks() {
        for preset in bank.presets() {
            check_guitar_identity(preset.device_id(), Operation::OpenGuitarBank)?;
            presets += 1;
        }
    }
    Ok(GuitarBankSummary {
        banks: bundle.banks().len(),
        presets,
        fixed: bundle.fixed,
    })
}

pub(crate) fn open_pod_preset(bytes: &[u8]) -> Result<PodToneSummary, Rejection> {
    let tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::OpenPodPreset))?;
    Ok(PodToneSummary {
        name: tone.name().to_owned(),
        device_id: tone.device_id(),
        device_version: tone.device_version(),
        container: tone.container().tag(),
        amp_model_id: tone.amp_model_id(),
        cab_model_id: tone.cab_model_id(),
        amp_name: tone.amp_asset_name(),
        cab_name: tone.cab_asset_name(),
        undecoded_slots: tone.undecoded_slots(),
        recorded_parameter_chunks: tone.recorded_parameter_chunks(),
        undecoded_parameter_chunks: tone.undecoded_parameter_chunks(),
    })
}

pub(crate) fn rename_pod_preset(bytes: &[u8], name: &str) -> Result<Vec<u8>, Rejection> {
    let mut tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.rename(name)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    Ok(tone.into_bytes())
}

pub(crate) fn set_pod_model_enabled(
    bytes: &[u8],
    control: pod_preset::PodModelEnable,
    enabled: bool,
) -> Result<Vec<u8>, Rejection> {
    let mut tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.set_model_enabled(control, enabled)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    Ok(tone.into_bytes())
}

pub(crate) fn set_pod_amp_model_choice(
    bytes: &[u8],
    choice: pod_preset::PodAmpModelChoice,
) -> Result<Vec<u8>, Rejection> {
    let mut tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.set_amp_model_choice(choice)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    Ok(tone.into_bytes())
}

pub(crate) fn set_pod_cab_model_choice(
    bytes: &[u8],
    choice: pod_preset::PodCabModelChoice,
) -> Result<Vec<u8>, Rejection> {
    let mut tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.set_cab_model_choice(choice)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    Ok(tone.into_bytes())
}

pub(crate) fn set_pod_mic_choice(
    bytes: &[u8],
    choice: pod_preset::PodMicChoice,
) -> Result<Vec<u8>, Rejection> {
    let mut tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.set_mic_choice(choice)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    Ok(tone.into_bytes())
}

pub(crate) fn save_pod_copy(path: &Path, bytes: &[u8]) -> Result<(), Rejection> {
    let tone = pod_preset::PodTone::parse(bytes.to_vec())
        .map_err(|error| map_tone(error, Operation::EditPodPreset))?;
    tone.save_copy(path)
        .map_err(|error| map_tone(error, Operation::EditPodPreset))
}

pub(crate) fn save_guitar_copy(path: &Path, bytes: &[u8]) -> Result<(), Rejection> {
    let preset = PresetFile::parse(bytes.to_vec()).map_err(Rejection::format)?;
    check_guitar_identity(preset.device_id(), Operation::SaveGuitarCopy)?;
    document::save_new(path, preset.as_bytes()).map_err(Rejection::format)
}

pub(crate) fn inspect_firmware(
    profile: ProductProfile,
    bytes: &[u8],
) -> Result<FirmwareSummary, Rejection> {
    let (product, version) = expected_container(profile)?;
    let inspection = firmware::inspect(bytes).map_err(Rejection::format)?;
    inspection
        .require_target(product, version)
        .map_err(|error| match error {
            ApplicabilityError::WrongProduct { declared, expected } => {
                Rejection::WrongProduct { declared, expected }
            }
            ApplicabilityError::WrongVersion { declared, expected } => {
                Rejection::WrongVersion { declared, expected }
            }
        })?;
    Ok(FirmwareSummary {
        product_id: inspection.report.product_id,
        version: inspection.report.version,
        applicability: inspection.report.applicability.tag(),
        integrity_is_not_authentication: inspection.report.integrity_is_not_authentication,
    })
}

pub(crate) fn accept_family_math(profile: ProductProfile, device_id: u32) -> Result<(), Rejection> {
    match profile {
        ProductProfile::Variax600 => check_guitar_identity(device_id, Operation::ApplyFamilyMath),
        ProductProfile::PodxtLive => Err(Rejection::Unsupported {
            profile,
            operation: Operation::ApplyFamilyMath,
        }),
        profile => Err(Rejection::Unverified {
            profile,
            operation: Operation::ApplyFamilyMath,
        }),
    }
}

fn expected_container(profile: ProductProfile) -> Result<(u32, u32), Rejection> {
    match profile {
        ProductProfile::Variax600 => Ok((
            crate::firmware::VARIAX_300_600_PRODUCT_ID,
            crate::firmware::VARIAX_300_600_VERSION,
        )),
        ProductProfile::PodxtLive => Ok((
            crate::firmware::PODXT_LIVE_PRODUCT_ID,
            crate::firmware::PODXT_LIVE_VERSION,
        )),
        profile => Err(Rejection::Unverified {
            profile,
            operation: Operation::InspectFirmware,
        }),
    }
}

fn map_tone(error: ToneError, operation: Operation) -> Rejection {
    match error {
        ToneError::WrongTarget { device_id } if ToneError::is_variax_family(device_id) => {
            Rejection::CrossFamily {
                from: DeviceFamily::VariaxWorkbench,
                to: DeviceFamily::PodXt,
                operation,
            }
        }
        ToneError::WrongTarget { device_id } => Rejection::Format {
            detail: format!(
                "PINF target {device_id:08x} is not PODxt Live {:08x}",
                pod_preset::PODXT_LIVE_TONE_ID
            ),
        },
        ToneError::Format { detail } => Rejection::Format { detail },
    }
}

fn check_guitar_identity(device_id: u32, operation: Operation) -> Result<(), Rejection> {
    if device_id == VARIAX_600_FILE_ID {
        Ok(())
    } else if device_id == VARIAX_500_700_FILE_ID {
        Err(Rejection::DifferentModel {
            profile: ProductProfile::Variax600,
            device_id,
        })
    } else if device_id == PODXT_LIVE_FILE_ID {
        Err(Rejection::CrossFamily {
            from: DeviceFamily::PodXt,
            to: DeviceFamily::VariaxWorkbench,
            operation,
        })
    } else {
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation,
        })
    }
}

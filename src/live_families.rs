//! Live reads through the per-family Line 6 library.
//!
//! Variax stays on the PODxt Live USB guitar interface. PODxt Live stays on
//! its own MIDI port. A test build returns before either link is opened.

#![cfg(feature = "usb")]

use crate::PresetMetadata;
use crate::hardware_gates::{self, Gate};

#[cfg(all(not(test), target_os = "linux"))]
use line6_devices::family::{self, podxt, variax};
#[cfg(all(not(test), target_os = "linux"))]
use line6_devices::gate::{GatePolicy, Operation};
#[cfg(all(not(test), target_os = "linux"))]
use line6_devices::session::{Quiet, Session};
#[cfg(all(not(test), target_os = "linux"))]
use line6_devices::transport::usb::{self, UsbInterfaceLink};

#[cfg(all(not(test), target_os = "linux"))]
struct DeviceCheckPolicy;

#[cfg(all(not(test), target_os = "linux"))]
impl GatePolicy for DeviceCheckPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !hardware_gates::is_locked(Gate::IdentityRequest),
            Operation::VariaxRead | Operation::PodxtIdentity | Operation::PodxtRead => {
                !hardware_gates::is_locked(Gate::DeviceCheckCurrentSettingsRead)
            }
            // Library backup and preset switching stay on the backup policy.
            Operation::PodxtLibraryRead
            | Operation::VariaxPresetSwitch
            | Operation::VariaxEditBufferWrite
            | Operation::VariaxStoreRestore
            | Operation::PodxtPackQuery
            | Operation::PodxtEditBufferWrite => false,
            // Opened in the app on the owner's direct permission (2026-11-23,
            // "open all 7"), each behind the gate for its kind of action.
            Operation::PodxtGlobalRead | Operation::PodxtTunerMode => {
                !hardware_gates::is_locked(Gate::DeviceCheckCurrentSettingsRead)
            }
            Operation::PodxtAdditionalEditBufferWrite => {
                !hardware_gates::is_locked(Gate::DeviceCheckLiveTest)
            }
            Operation::PodxtKeepEditBufferChanges => !hardware_gates::is_locked(Gate::KeepLiveEdit),
            Operation::PodxtChannelSwitch => !hardware_gates::is_locked(Gate::DeviceCheckChannel),
            Operation::PodxtVariaxControlRelease | Operation::PodxtLinkedVariaxModelEdit => {
                !hardware_gates::is_locked(Gate::EditBufferWrite)
            }
        }
    }
}

#[derive(Debug)]
pub struct BothReads {
    pub metadata: PresetMetadata,
    pub summary: String,
}

pub fn read_both() -> Result<BothReads, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, read_both_gated)
}

fn read_both_gated() -> Result<BothReads, String> {
    hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, read_both_supported)
    })
}

fn read_both_supported() -> Result<BothReads, String> {
    #[cfg(any(test, not(target_os = "linux")))]
    {
        Err("this build does not open a device for step 3; nothing was sent".into())
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        read_both_live()
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn read_variax() -> Result<(PresetMetadata, String), String> {
    let mut link = UsbInterfaceLink::open(usb::VARIAX_VIA_PODXT_LIVE)?;
    let preset = {
        let mut log = Quiet;
        let policy = DeviceCheckPolicy;
        let mut session = Session::new(&mut link, &policy, &mut log);
        variax::read_current(&mut session)
    };
    let cleanup = link.finish();
    let preset = match (preset, cleanup) {
        (Ok(preset), Ok(())) => preset,
        (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
        (Err(error), Err(cleanup)) => {
            return Err(format!("{error} Cleanup also failed: {cleanup}"));
        }
    };
    let metadata = PresetMetadata::from_replies(&preset.replies[0], &preset.replies[1])?;
    let family = family::Family::Variax;
    let text = format!(
        "Variax: {} firmware {}. Preset {}. Body model {}. First-string tuning {}.",
        family.member_name(preset.identified.identity.member),
        preset.identified.identity.version_text(),
        metadata.name,
        integer(&metadata, "body_model")?,
        integer(&metadata, "string1_tuning")?
    );
    Ok((metadata, text))
}

#[cfg(all(not(test), target_os = "linux"))]
fn integer(metadata: &PresetMetadata, key: &str) -> Result<i32, String> {
    match metadata.parameters.get(key) {
        Some(crate::ParameterValue::Integer(value)) => Ok(*value),
        _ => Err(format!("the Variax settings did not contain {key}")),
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn read_podxt() -> Result<String, String> {
    use line6_devices::session::Path;
    use line6_devices::transport::rawmidi::{self, RawMidiLink};
    let mut link = RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)?;
    let mut log = Quiet;
    let policy = DeviceCheckPolicy;
    let mut session = Session::new(&mut link, &policy, &mut log);
    let tone = podxt::read_current(&mut session)?;
    let family = family::Family::Podxt;
    let mut lines = vec![format!(
        "PODxt Live: {} firmware {}.",
        family.member_name(tone.identity.member),
        tone.identity.version_text()
    )];
    lines.extend(tone.tone.describe());
    Ok(lines.join(" "))
}

#[cfg(all(not(test), target_os = "linux"))]
fn read_both_live() -> Result<BothReads, String> {
    let (metadata, variax) =
        read_variax().map_err(|error| format!("Variax read stopped: {error}"))?;
    let podxt =
        read_podxt().map_err(|error| format!("{variax} PODxt Live read stopped: {error}"))?;
    Ok(BothReads {
        metadata,
        summary: format!(
            "{variax} {podxt} This is the current settings, not a backup. Nothing was written. Step 4 stays off."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_build_does_not_open_either_device() {
        let error = read_both().unwrap_err();
        assert!(error.contains("nothing was sent"), "{error}");
    }
}

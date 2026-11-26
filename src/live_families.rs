//! Live reads through the per-family Line 6 library.
//!
//! Variax stays on the PODxt Live USB guitar interface. PODxt Live stays on
//! its own MIDI port. A test build returns before either link is opened.

#![cfg(feature = "usb")]

use crate::PresetMetadata;
use crate::firmware_notice::FirmwareNotice;
use crate::hardware_gates::{self, Gate};

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::family::variax::transport::VARIAX_VIA_PODXT_LIVE;
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::family::{self, podxt, variax};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::gate::{GatePolicy, Operation};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::session::{Link, Quiet, Session};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::transport::usb::UsbInterfaceLink;

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
struct DeviceCheckPolicy;

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
impl GatePolicy for DeviceCheckPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !hardware_gates::is_locked(Gate::IdentityRequest),
            Operation::VariaxRead | Operation::PodxtIdentity | Operation::PodxtRead => {
                !hardware_gates::is_locked(Gate::DeviceCheckCurrentSettingsRead)
            }
            // Library backup and preset switching stay on the backup policy.
            Operation::VariaxBankNameRead
            | Operation::VariaxLibraryBridge
            | Operation::PodxtLibraryRead
            | Operation::VariaxPresetSwitch
            | Operation::VariaxEditBufferWrite
            | Operation::VariaxStoreRestore
            | Operation::PodxtStoreRestore
            | Operation::VariaxSlotRead
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
    pub firmware_notice: Option<FirmwareNotice>,
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
        let _rig =
            line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string())?;
        read_both_live(read_variax, read_podxt)
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn read_variax() -> Result<(PresetMetadata, String), String> {
    let mut link = UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE)?;
    let result = read_variax_on(&mut link);
    after_release(result, link.finish())
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn read_variax_on(link: &mut dyn Link) -> Result<(PresetMetadata, String), String> {
    let mut log = Quiet;
    let mut session = Session::new(link, &DeviceCheckPolicy, &mut log);
    let preset = variax::read_current(&mut session)?;
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

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn integer(metadata: &PresetMetadata, key: &str) -> Result<i32, String> {
    match metadata.parameters.get(key) {
        Some(crate::ParameterValue::Integer(value)) => Ok(*value),
        _ => Err(format!("the Variax settings did not contain {key}")),
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn read_podxt() -> Result<(String, Option<FirmwareNotice>), String> {
    use line6_devices::{
        family::podxt::transport::PODXT_LIVE_MIDI, transport::rawmidi::RawMidiLink,
    };
    read_podxt_on(&mut RawMidiLink::open_spec(PODXT_LIVE_MIDI)?)
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn read_podxt_on(link: &mut dyn Link) -> Result<(String, Option<FirmwareNotice>), String> {
    let mut log = Quiet;
    let mut session = Session::new(link, &DeviceCheckPolicy, &mut log);
    let tone = podxt::read_current(&mut session)?;
    if let Some(notice) = FirmwareNotice::podxt(&tone.identity) {
        return Ok((notice.message(), Some(notice)));
    }
    let family = family::Family::Podxt;
    let mut lines = vec![format!(
        "PODxt Live: {} firmware {}.",
        family.member_name(tone.identity.member),
        tone.identity.version_text()
    )];
    lines.extend(tone.tone.describe());
    Ok((lines.join(" "), None))
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn read_both_live(
    read_variax: impl FnOnce() -> Result<(PresetMetadata, String), String>,
    read_podxt: impl FnOnce() -> Result<(String, Option<FirmwareNotice>), String>,
) -> Result<BothReads, String> {
    let (metadata, variax) =
        read_variax().map_err(|error| format!("Variax read stopped: {error}"))?;
    let (podxt, firmware_notice) =
        read_podxt().map_err(|error| format!("{variax} PODxt Live read stopped: {error}"))?;
    Ok(BothReads {
        metadata,
        firmware_notice,
        summary: format!(
            "{variax} {podxt} This is the current settings, not a backup. Nothing was written. Step 4 stays off."
        ),
    })
}

#[cfg(all(target_os = "macos", not(test)))]
pub(crate) fn read_both_on(
    path: &str,
    address: u8,
    rig: &line6_devices::device_lock::DeviceLock,
) -> Result<BothReads, String> {
    use line6_devices::transport::usb::macos::PODXT_MIDI;

    if !crate::macos_usb_available() {
        return Err("This build does not open a device for step 3; nothing was sent.".into());
    }
    hardware_gates::run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
        hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
            hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
                read_both_live(
                    || {
                        let mut link = UsbInterfaceLink::open_attachment_borrowed(
                            VARIAX_VIA_PODXT_LIVE,
                            path,
                            address,
                            rig,
                        )?;
                        let result = read_variax_on(&mut link);
                        after_release(result, link.finish())
                    },
                    || {
                        let mut link = UsbInterfaceLink::open_attachment_borrowed(
                            PODXT_MIDI, path, address, rig,
                        )?;
                        let result = read_podxt_on(&mut link);
                        after_release(result, link.finish())
                    },
                )
            })
        })
    })
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) fn after_release<T>(
    result: Result<T, String>,
    release: Result<(), String>,
) -> Result<T, String> {
    match (result, release) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(format!("Device release failed: {error}")),
        (Err(error), Err(release)) => Err(format!("{error} Cleanup also failed: {release}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserving_operation_errors_does_not_hide_failed_release() {
        assert_eq!(
            after_release::<()>(Err("operation".into()), Err("release".into())),
            Err("operation Cleanup also failed: release".into())
        );
    }

    #[test]
    fn failed_variax_cleanup_stops_before_podxt_open() {
        let result = read_both_live(
            || after_release(Err("read failed".into()), Err("release failed".into())),
            || panic!("the PODxt interface must stay closed"),
        );
        assert_eq!(
            result.unwrap_err(),
            "Variax read stopped: read failed Cleanup also failed: release failed"
        );
    }

    #[test]
    fn a_test_build_does_not_open_either_device() {
        let error = read_both().unwrap_err();
        assert!(error.contains("nothing was sent"), "{error}");
    }
}

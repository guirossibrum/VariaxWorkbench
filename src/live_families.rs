//! Live reads through the per-family Line 6 library.
//!
//! Variax stays on the PODxt Live USB guitar interface. PODxt Live stays on
//! its own MIDI port. A test build returns before either link is opened.

#![cfg(feature = "usb")]

use crate::PresetMetadata;
use crate::firmware_notice::FirmwareNotice;
use crate::hardware_gates::{self, Gate};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use crate::session_log::Endpoint;
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use crate::session_log::Operation as LogOperation;

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::family::podxt;
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::family::variax::transport::VARIAX_VIA_PODXT_LIVE;
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use line6_devices::family::{self, variax};
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use line6_devices::gate::{GatePolicy, Operation};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::session::Quiet;
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use line6_devices::session::{Link, Session};
#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
use line6_devices::transport::usb::UsbInterfaceLink;

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
struct DeviceCheckPolicy;

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
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

#[cfg(any(test, not(target_os = "macos")))]
pub fn read_both() -> Result<BothReads, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, read_both_gated)
}

#[cfg(any(test, not(target_os = "macos")))]
fn read_both_gated() -> Result<BothReads, String> {
    hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, read_both_supported)
    })
}

#[cfg(any(test, not(target_os = "macos")))]
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
    read_variax_checked(
        &mut link,
        UsbInterfaceLink::finish,
        LogOperation::begin(Endpoint::Variax),
    )
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn read_variax_checked<L: Link>(
    link: &mut L,
    finish: impl FnOnce(&mut L) -> Result<(), String>,
    mut operation: LogOperation,
) -> Result<(PresetMetadata, String), String> {
    let result = read_variax_on(link, &mut operation);
    let start = std::time::Instant::now();
    let release = finish(link);
    let release_duration = start.elapsed();
    operation.finished(result.is_ok());
    operation.checked_release(release.is_ok(), release_duration);
    after_release(result, release)
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn read_variax_on(
    link: &mut dyn Link,
    log: &mut LogOperation,
) -> Result<(PresetMetadata, String), String> {
    let mut session = Session::new(link, &DeviceCheckPolicy, log);
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

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
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
                        read_variax_checked(
                            &mut link,
                            UsbInterfaceLink::finish,
                            LogOperation::begin(Endpoint::Variax),
                        )
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

#[cfg(test)]
mod logging_tests {
    use super::*;
    use crate::session_log::{Endpoint, SessionLog, Severity, tests::ScriptedLink};
    use line6_devices::family::variax::VariaxRequest::*;
    use line6_devices::session::Path;

    #[test]
    fn variax_step_three_logs_existing_orphan_retry_read_timeout_and_checked_release() {
        // Synthetic identity with the Variax 600 member and made-up firmware.
        let identity = vec![
            0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x04, 0x00, 0x31, 0x32,
            0x30, 0x33, 0xf7,
        ];
        let mut metadata = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x3c];
        for word in b"HiddenPreset".as_chunks::<3>().0 {
            let value = u32::from_be_bytes([0, word[0], word[1], word[2]]);
            metadata.extend(crate::encode_word(value).unwrap());
        }
        metadata.resize(199, 0);
        metadata.push(0xf7);

        for (identity_timeout, metadata_timeout) in [(false, false), (false, true), (true, false)] {
            let read_succeeded = !identity_timeout && !metadata_timeout;
            let temp = tempfile::tempdir().unwrap();
            let (log, guard) = SessionLog::open(Some(temp.path().into()), Severity::Trace);
            let script = if identity_timeout {
                vec![(Identity, None)]
            } else {
                vec![
                    (Identity, Some(vec![0xf7])),
                    (Identity, Some(identity.clone())),
                    (SessionOn, None),
                    (Metadata1, Some(metadata.clone())),
                    (Metadata2, (!metadata_timeout).then(|| metadata.clone())),
                    (SessionOff, None),
                ]
            };
            let mut link = ScriptedLink::new(Path::VariaxViaPodxt, script);
            let result = read_variax_checked(
                &mut link,
                |link| {
                    link.assert_finished();
                    if read_succeeded {
                        Err("private release detail".into())
                    } else {
                        Ok(())
                    }
                },
                LogOperation::with_log(&log, Endpoint::Variax),
            );
            assert!(result.is_err());
            drop(guard);
            let text = std::fs::read_to_string(log.status().path.unwrap()).unwrap();
            assert!(!text.contains("HiddenPreset") && !text.contains("private release detail"));
            let rows: Vec<serde_json::Value> = text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .filter(|row: &serde_json::Value| row["endpoint"] == "variax")
                .collect();
            assert!(
                rows.iter()
                    .all(|row| row["operation"] == rows[0]["operation"])
            );
            let transport = |request: &str, outcome: &str| {
                rows.iter()
                    .filter(|row| {
                        row["observation"]["request"] == request
                            && row["observation"]["outcome"] == outcome
                    })
                    .count()
            };
            assert_eq!(
                transport("VariaxIdentity", "Sent"),
                if identity_timeout { 1 } else { 2 }
            );
            assert_eq!(
                transport("VariaxIdentity", "OrphanQuiet"),
                usize::from(!identity_timeout)
            );
            assert_eq!(
                transport("VariaxIdentity", "Timeout"),
                usize::from(identity_timeout)
            );
            assert_eq!(
                transport("VariaxRead", "Sent"),
                if identity_timeout { 0 } else { 4 }
            );
            let packets = if identity_timeout {
                0
            } else if metadata_timeout {
                1
            } else {
                2
            };
            assert_eq!(transport("VariaxRead", "Packet"), packets);
            if metadata_timeout {
                assert!(transport("VariaxRead", "NoPacket") > 0);
            }
            assert_eq!(rows[rows.len() - 2]["category"], "operation_finished");
            assert_eq!(rows[rows.len() - 2]["success"], read_succeeded);
            assert_eq!(rows.last().unwrap()["category"], "checked_release");
            assert_eq!(rows.last().unwrap()["success"], !read_succeeded);
        }
    }
}

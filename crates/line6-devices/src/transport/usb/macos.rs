//! Driverless macOS attachment discovery and non-seizing interface claims.

use super::{
    InterfaceSpec, VARIAX_VIA_PODXT_LIVE, attachment_path,
    macos_inventory::{ObservedPodxt, PassiveReadinessBlock, assess_candidate},
    podxt::SPEC,
};
#[cfg(target_os = "macos")]
use super::{UsbInterfaceLink, macos_inventory::inventory, podxt};
#[cfg(target_os = "macos")]
use crate::device_lock::DeviceLock;

type NativeDevice = rusb::Device<rusb::GlobalContext>;

pub const PODXT_MIDI: InterfaceSpec = SPEC;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: String,
    pub address: u8,
}

/// Enumerate the identified USB product without claiming a device.
pub fn attachments() -> Result<Vec<Attachment>, String> {
    check_available()?;
    native_podxts()?.iter().map(native_attachment).collect()
}

fn native_podxts() -> Result<Vec<NativeDevice>, String> {
    let devices = rusb::devices().map_err(open_error)?;
    Ok(devices
        .iter()
        .filter(|device| {
            device.device_descriptor().is_ok_and(|descriptor| {
                descriptor.vendor_id() == SPEC.vendor && descriptor.product_id() == SPEC.product
            })
        })
        .collect())
}

fn native_attachment(device: &NativeDevice) -> Result<Attachment, String> {
    let ports = device.port_numbers().map_err(open_error)?;
    if ports.is_empty() {
        return Err("The PODxt Live physical USB port could not be identified.".into());
    }
    Ok(Attachment {
        path: attachment_path(device.bus_number(), &ports),
        address: device.address(),
    })
}

/// Select the single PODxt Live the Mac connection flow can verify.
pub fn single_podxt(devices: &[ObservedPodxt]) -> Result<&ObservedPodxt, String> {
    single_attachment(devices)
}

/// Evaluate passive readiness. A candidate still needs the exact USB opener
/// to verify its active descriptor and claim access.
pub fn ready_attachment(
    observed: &ObservedPodxt,
    devices: &[ObservedPodxt],
) -> Result<Attachment, String> {
    let current = single_podxt(devices)?;
    if !observed.matches_generation(current) {
        return Err("The PODxt Live USB attachment changed.".into());
    }
    assess_candidate(devices, 0).map_err(|reason| passive_readiness_message(Some(reason)).into())
}

pub fn passive_readiness_message(reason: Option<PassiveReadinessBlock>) -> &'static str {
    use PassiveReadinessBlock::*;

    match reason {
        None => "PODxt Live detected. USB interfaces 0 and 1 are published. Identity unverified.",
        Some(MissingCoordinates {
            address: true,
            route: true,
        }) => "PODxt Live detected. USB address and route unavailable in this scan.",
        Some(MissingCoordinates {
            address: true,
            route: false,
        }) => "PODxt Live detected. USB address unavailable in this scan.",
        Some(MissingCoordinates { address: false, .. }) => {
            "PODxt Live detected. USB route unavailable in this scan."
        }
        Some(Ambiguous { .. }) => {
            "PODxt Live detected. Multiple USB records match this location or route."
        }
        Some(UnreadableChildren { .. }) => {
            "PODxt Live detected. macOS USB interface list could not be read."
        }
        Some(UnreadableInterfaceNumber { .. }) => {
            "PODxt Live detected. macOS USB interface publication could not be determined."
        }
        Some(MissingInterfaces {
            seen: [false, true],
        }) => "PODxt Live detected. USB interface 0 was absent from this scan.",
        Some(MissingInterfaces {
            seen: [true, false],
        }) => "PODxt Live detected. USB interface 1 was absent from this scan.",
        Some(MissingInterfaces { .. }) => {
            "PODxt Live detected. USB interfaces 0 and 1 were absent from this scan."
        }
    }
}

/// Reconcile the ready registry candidate with the exact native USB route.
pub fn verify_usb_route(candidate: &Attachment) -> Result<(), String> {
    check_exact_route(candidate, &attachments()?)
}

/// Open the exact ready generation and borrow the rig lock through checked release.
#[cfg(target_os = "macos")]
pub fn open_ready_attachment<'rig>(
    rig: &'rig DeviceLock,
    spec: InterfaceSpec,
    registry_id: u64,
    expected: &Attachment,
    before_open: impl FnOnce() -> Result<(), String>,
) -> Result<UsbInterfaceLink<&'rig DeviceLock>, String> {
    check_open(spec, true)?;
    podxt::check_open(spec, true)?;
    let candidate = ready_attachment_for(registry_id, expected, &inventory()?)?;
    let devices = native_podxts()?;
    with_exact_generation(
        single_attachment(&devices)?,
        &candidate,
        registry_id,
        native_attachment,
        |device| device.darwin_registry_entry_id().ok(),
        |device| {
            before_open()?;
            UsbInterfaceLink::open_device(spec, device, rig)
        },
    )
}

fn with_exact_generation<D, T>(
    device: &D,
    expected: &Attachment,
    expected_id: u64,
    route: impl FnOnce(&D) -> Result<Attachment, String>,
    registry_id: impl FnOnce(&D) -> Option<u64>,
    open: impl FnOnce(&D) -> Result<T, String>,
) -> Result<T, String> {
    if route(device)? != *expected {
        return Err("The PODxt Live USB attachment changed.".into());
    }
    match registry_id(device) {
        Some(id) if id == expected_id => open(device),
        Some(_) => Err("The PODxt Live USB attachment changed.".into()),
        None => Err("The selected PODxt Live USB registry generation is unavailable.".into()),
    }
}

pub(super) fn ready_attachment_for(
    registry_id: u64,
    expected: &Attachment,
    devices: &[ObservedPodxt],
) -> Result<Attachment, String> {
    let current = single_podxt(devices)?;
    if current.registry_id != registry_id
        || current.route() != Some(expected.path.as_str())
        || current.address != Some(expected.address)
    {
        return Err("The PODxt Live USB attachment changed.".into());
    }
    ready_attachment(current, devices)
}

fn single_attachment<T>(attachments: &[T]) -> Result<&T, String> {
    let [attachment] = attachments else {
        return Err(format!(
            "Expected one PODxt Live on USB, found {}.",
            attachments.len()
        ));
    };
    Ok(attachment)
}

fn check_exact_route(expected: &Attachment, attachments: &[Attachment]) -> Result<(), String> {
    if single_attachment(attachments)? != expected {
        return Err("The PODxt Live USB attachment changed.".into());
    }
    Ok(())
}

fn check_available() -> Result<(), String> {
    if cfg!(test) {
        return Err("Test builds do not open or enumerate USB devices.".into());
    }
    Ok(())
}

pub(super) fn check_open(spec: InterfaceSpec, locked: bool) -> Result<(), String> {
    check_available()?;
    check_attachment_profile(spec, locked)
}

fn check_attachment_profile(spec: InterfaceSpec, locked: bool) -> Result<(), String> {
    if !locked {
        return Err("Mac USB requires the rig lock and the selected attachment.".into());
    }
    if spec != SPEC && spec != VARIAX_VIA_PODXT_LIVE {
        return Err("The Mac USB connection supports only the PODxt Live MIDI interfaces.".into());
    }
    Ok(())
}

pub(super) fn check_interfaces(
    driver_active: impl Fn(u8) -> rusb::Result<bool>,
) -> Result<(), String> {
    for interface in [0, 1] {
        if driver_active(interface).map_err(open_error)? {
            return Err(format!(
                "A kernel driver holds PODxt Live interface {interface}. Disconnect the legacy Line 6 driver before using direct USB."
            ));
        }
    }
    Ok(())
}

pub(super) fn open_error(error: rusb::Error) -> String {
    match error {
        rusb::Error::Busy => "The PODxt Live is held by a driver or another application. If the legacy Line 6 driver is installed, resolve that conflict before using direct USB.".into(),
        other => format!("Cannot use the PODxt Live USB connection: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn test_build_refuses_mac_inventory_and_open_before_usb() {
        assert!(attachments().unwrap_err().contains("Test builds"));
        assert!(
            check_open(PODXT_MIDI, true)
                .unwrap_err()
                .contains("Test builds")
        );
        assert!(
            check_open(VARIAX_VIA_PODXT_LIVE, true)
                .unwrap_err()
                .contains("Test builds")
        );
    }

    #[test]
    fn exact_native_route_refuses_absence_ambiguity_and_changed_coordinates() {
        let original = Attachment {
            path: "4-2".into(),
            address: 17,
        };
        let new_path = Attachment {
            path: "4-3".into(),
            ..original.clone()
        };
        let new_address = Attachment {
            address: 18,
            ..original.clone()
        };

        assert!(check_exact_route(&original, std::slice::from_ref(&original)).is_ok());
        assert!(
            check_exact_route(&original, &[])
                .unwrap_err()
                .contains("found 0")
        );
        assert!(
            check_exact_route(&original, &[original.clone(), new_path.clone()])
                .unwrap_err()
                .contains("found 2")
        );
        for changed in [new_path, new_address] {
            assert!(
                check_exact_route(&original, &[changed])
                    .unwrap_err()
                    .contains("attachment changed")
            );
        }
    }

    #[test]
    fn native_selection_only_opens_the_same_registry_generation() {
        struct Device {
            route: Attachment,
            id: Option<u64>,
        }

        let expected = Attachment {
            path: "4-2".into(),
            address: 17,
        };
        // Passive readiness already accepted generation 11. These are the
        // device references returned at the later native selection.
        for (id, error) in [
            (Some(12), Some("attachment changed")),
            (None, Some("registry generation is unavailable")),
            (Some(11), None),
        ] {
            let selected = Device {
                route: expected.clone(),
                id,
            };
            let mut events = Vec::new();
            let result = with_exact_generation(
                &selected,
                &expected,
                11,
                |device| Ok(device.route.clone()),
                |device| device.id,
                |device| {
                    assert!(std::ptr::eq(device, &selected));
                    events.extend(["open", "claim", "I/O"]);
                    Ok(())
                },
            );
            if let Some(error) = error {
                assert!(result.unwrap_err().contains(error));
                assert!(events.is_empty());
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(events, ["open", "claim", "I/O"]);
            }
        }
    }

    #[test]
    fn only_podxt_and_its_relay_can_open_with_a_locked_attachment() {
        for spec in [PODXT_MIDI, VARIAX_VIA_PODXT_LIVE] {
            assert!(check_attachment_profile(spec, true).is_ok());
            assert!(
                check_attachment_profile(spec, false)
                    .unwrap_err()
                    .contains("rig lock")
            );
        }
        let other = InterfaceSpec {
            product: 0xffff,
            ..PODXT_MIDI
        };
        assert!(
            check_attachment_profile(other, true)
                .unwrap_err()
                .contains("PODxt Live")
        );
    }

    #[test]
    fn either_interface_driver_prevents_a_claim() {
        for active in [None, Some(0), Some(1)] {
            let checked = RefCell::new(Vec::new());
            let result = check_interfaces(|interface| {
                checked.borrow_mut().push(interface);
                Ok(active == Some(interface))
            });
            if let Some(interface) = active {
                let error = result.unwrap_err();
                assert!(error.contains(&format!("interface {interface}")));
                assert!(error.contains("Line 6"));
            } else {
                assert!(result.is_ok());
            }
            assert_eq!(
                *checked.borrow(),
                if active == Some(0) {
                    vec![0]
                } else {
                    vec![0, 1]
                }
            );
        }
        assert!(check_interfaces(|_| Err(rusb::Error::NotSupported)).is_err());
        assert!(open_error(rusb::Error::Busy).contains("held by a driver"));
    }
}

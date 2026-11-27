//! Driverless macOS attachment discovery and non-seizing interface claims.

use super::{
    InterfaceSpec, VARIAX_VIA_PODXT_LIVE, attachment_path,
    macos_inventory::{ObservedPodxt, inventory},
    podxt::SPEC,
};
use crate::device_lock::DeviceLock;

pub const PODXT_MIDI: InterfaceSpec = SPEC;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: String,
    pub address: u8,
}

/// Enumerate the identified USB product without claiming a device.
pub fn attachments() -> Result<Vec<Attachment>, String> {
    check_available()?;
    let devices = rusb::devices().map_err(open_error)?;
    devices
        .iter()
        .filter(|device| {
            device.device_descriptor().is_ok_and(|descriptor| {
                descriptor.vendor_id() == SPEC.vendor && descriptor.product_id() == SPEC.product
            })
        })
        .map(|device| {
            let ports = device.port_numbers().map_err(open_error)?;
            if ports.is_empty() {
                return Err("The PODxt Live physical USB port could not be identified.".into());
            }
            Ok(Attachment {
                path: attachment_path(device.bus_number(), &ports),
                address: device.address(),
            })
        })
        .collect()
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
    if current.route().is_none() {
        return Err("PODxt Live detected, but its physical USB route is unavailable.".into());
    }
    if current.address.is_none() {
        return Err("PODxt Live detected, but its USB address is unavailable.".into());
    }
    current.attachment(devices).ok_or_else(|| {
        "PODxt Live detected, but the macOS USB inventory did not confirm both MIDI interfaces."
            .into()
    })
}

/// Reconcile the ready registry candidate with the exact native USB route.
pub fn verify_usb_route(candidate: &Attachment) -> Result<(), String> {
    check_exact_route(candidate, &attachments()?)
}

/// Recheck readiness and the generation under the rig lock at each open.
pub fn with_ready_attachment<T>(
    _rig: &DeviceLock,
    registry_id: u64,
    expected: &Attachment,
    open: impl FnOnce(&Attachment) -> Result<T, String>,
) -> Result<T, String> {
    let devices = inventory()?;
    let current = single_podxt(&devices)?;
    if current.registry_id != registry_id
        || current.route() != Some(expected.path.as_str())
        || current.address != Some(expected.address)
    {
        return Err("The PODxt Live USB attachment changed.".into());
    }
    let candidate = ready_attachment(current, &devices)?;
    verify_usb_route(&candidate)?;
    open(&candidate)
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

//! Driverless macOS attachment discovery and non-seizing interface claims.

use super::{InterfaceSpec, VARIAX_VIA_PODXT_LIVE, attachment_path, podxt::SPEC};

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
            Ok(Attachment {
                path: attachment_path(device.bus_number(), &ports),
                address: device.address(),
            })
        })
        .collect()
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

//! Direct PODxt Live MIDI on interface 0. See docs/podxt-usb-direct.md.

use super::InterfaceSpec;
use crate::session::Path;
use std::time::Duration;

/// The first direct USB identity run needs separate owner approval.
pub const PODXT_DIRECT_USB_APPROVED: bool = false;
pub(super) const SPEC: InterfaceSpec = InterfaceSpec {
    vendor: 0x0e41,
    product: 0x4650,
    interface: 0,
    altsetting: 1,
    endpoint_out: 0x03,
    endpoint_in: 0x84,
    packet: 16,
    chunk_gap: Duration::ZERO,
    path: Path::PodxtMidi,
};

pub(super) fn targets_interface(spec: InterfaceSpec) -> bool {
    (spec.vendor, spec.product, spec.interface) == (SPEC.vendor, SPEC.product, SPEC.interface)
}

pub(super) fn check_open(spec: InterfaceSpec, locked: bool) -> Result<(), String> {
    if !targets_interface(spec) {
        return Ok(());
    }
    if !PODXT_DIRECT_USB_APPROVED {
        return Err("Direct PODxt USB MIDI is closed pending the owner's identity test.".into());
    }
    if cfg!(test) {
        return Err("Test builds do not open a USB device.".into());
    }
    if !cfg!(target_os = "linux") {
        return Err("Direct PODxt USB MIDI is only available for the Linux identity test.".into());
    }
    if !locked {
        return Err(
            "Direct PODxt USB MIDI requires a verified attachment and the rig lock.".into(),
        );
    }
    if (
        spec.altsetting,
        spec.endpoint_out,
        spec.endpoint_in,
        spec.packet,
        spec.path,
    ) != (
        SPEC.altsetting,
        SPEC.endpoint_out,
        SPEC.endpoint_in,
        SPEC.packet,
        SPEC.path,
    ) || spec.chunk_gap != SPEC.chunk_gap
    {
        return Err("The requested interface does not match the PODxt Live MIDI profile.".into());
    }
    Ok(())
}

pub(super) fn verify_descriptor(device: &rusb::Device<rusb::GlobalContext>) -> Result<(), String> {
    let config = device
        .active_config_descriptor()
        .map_err(|error| format!("PODxt Live USB configuration descriptor: {error}"))?;
    if config.number() == 1
        && config
            .interfaces()
            .flat_map(|iface| iface.descriptors())
            .any(|alt| {
                alt.interface_number() == SPEC.interface
                    && alt.setting_number() == SPEC.altsetting
                    && alt.class_code() == 0xff
                    && [SPEC.endpoint_out, SPEC.endpoint_in]
                        .into_iter()
                        .all(|address| {
                            alt.endpoint_descriptors().any(|endpoint| {
                                endpoint.address() == address
                                    && endpoint.transfer_type() == rusb::TransferType::Interrupt
                                    && usize::from(endpoint.max_packet_size()) == SPEC.packet
                                    && endpoint.interval() == 5
                            })
                        })
            })
    {
        Ok(())
    } else {
        Err("The USB descriptors do not match PODxt Live interface 0 alt 1 MIDI.".into())
    }
}

#[cfg(target_os = "linux")]
const DRIVER: &str = "snd-usb-pod";

#[cfg(target_os = "linux")]
pub(super) fn require_driver(
    handle: &rusb::DeviceHandle<rusb::GlobalContext>,
) -> Result<(), String> {
    match driver_bound(handle)? {
        true => Ok(()),
        false => Err(format!(
            "PODxt Live interface 0 must be bound to {DRIVER} before the identity test."
        )),
    }
}

#[cfg(target_os = "linux")]
pub(super) fn return_driver(
    handle: &rusb::DeviceHandle<rusb::GlobalContext>,
) -> Result<(), String> {
    restore_binding(
        || driver_bound(handle),
        || {
            handle
                .attach_kernel_driver(SPEC.interface)
                .map_err(|error| error.to_string())
        },
        std::thread::sleep,
    )
    .inspect_err(|error| eprintln!("PODxt Live interface 0 cleanup: {error}"))
}

#[cfg(target_os = "linux")]
fn driver_bound(handle: &rusb::DeviceHandle<rusb::GlobalContext>) -> Result<bool, String> {
    let device = handle.device();
    let ports = device
        .port_numbers()
        .map_err(|error| format!("USB port path: {error}"))?;
    binding(
        std::path::Path::new("/sys/bus/usb/devices"),
        device.bus_number(),
        &ports,
        device.address(),
    )
}

#[cfg(target_os = "linux")]
fn binding(devices: &std::path::Path, bus: u8, ports: &[u8], address: u8) -> Result<bool, String> {
    let port = super::attachment_path(bus, ports);
    let same_attachment = || {
        if std::fs::read_to_string(devices.join(&port).join("devnum"))
            .ok()
            .and_then(|value| value.trim().parse::<u8>().ok())
            == Some(address)
        {
            Ok(())
        } else {
            Err(
                "The PODxt Live USB attachment changed; interface 0 binding cannot be verified."
                    .to_string(),
            )
        }
    };

    same_attachment()?;
    let interface = devices.join(format!("{port}:1.0"));
    let driver = std::fs::read_link(interface.join("driver"));
    same_attachment()?;
    match driver {
        Ok(target) => {
            let name = target
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.replace('_', "-") == DRIVER {
                Ok(true)
            } else {
                Err(format!(
                    "PODxt Live interface 0 has unexpected driver {name:?}."
                ))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && interface.is_dir() => {
            Ok(false)
        }
        Err(error) => Err(format!(
            "Cannot verify the PODxt Live interface 0 driver: {error}"
        )),
    }
}

#[cfg(target_os = "linux")]
fn restore_binding(
    mut bound: impl FnMut() -> Result<bool, String>,
    mut attach: impl FnMut() -> Result<(), String>,
    wait: impl FnOnce(Duration),
) -> Result<(), String> {
    if bound()? {
        return Ok(());
    }
    let _ = attach();
    if bound()? {
        return Ok(());
    }
    wait(Duration::from_millis(150));
    if bound()? {
        return Ok(());
    }
    let retry = attach();
    if bound()? {
        return Ok(());
    }
    let detail = retry
        .err()
        .map_or_else(String::new, |error| format!(" ({error})"));
    Err(format!(
        "{DRIVER} did not return to PODxt Live interface 0{detail}."
    ))
}

pub(super) fn normalize_received(bytes: &mut [u8]) {
    for byte in bytes {
        *byte = match *byte {
            0xf2 => 0xf0,
            0xb2 => 0xb0,
            0xc2 => 0xc0,
            value => value,
        };
    }
}

#[cfg(test)]
mod tests;

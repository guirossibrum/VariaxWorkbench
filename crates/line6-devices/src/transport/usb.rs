//! A USB interrupt-interface link (libusb via `rusb`).
//!
//! Opening detaches the kernel driver from the one named interface only,
//! claims it and selects the altsetting; [`UsbInterfaceLink::finish`] releases
//! it and gives it back to the kernel driver. Other interfaces are untouched.

use crate::device_lock::DeviceLock;
use crate::session::{Link, Path};
use std::time::{Duration, Instant};

// Compatibility exports for callers of the original USB adapter API (#214).
pub use super::InterfaceSpec;
pub use crate::family::variax::transport::VARIAX_VIA_PODXT_LIVE;

const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct UsbInterfaceLink {
    spec: InterfaceSpec,
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    opened: Instant,
    detached: bool,
    claimed: bool,
    finished: bool,
    _rig: Option<DeviceLock>,
}

impl UsbInterfaceLink {
    pub fn open(spec: InterfaceSpec) -> Result<Self, String> {
        let devices = rusb::devices().map_err(|e| format!("USB list: {e}"))?;
        let found: Vec<_> = devices
            .iter()
            .filter(|d| {
                d.device_descriptor()
                    .is_ok_and(|d| d.vendor_id() == spec.vendor && d.product_id() == spec.product)
            })
            .collect();
        if found.len() != 1 {
            return Err(format!(
                "Expected one USB device {:04x}:{:04x}, found {}.",
                spec.vendor,
                spec.product,
                found.len()
            ));
        }
        Self::open_device(spec, &found[0], None)
    }

    /// Select the verified attachment before opening or detaching anything.
    /// The link owns the rig lock until it is dropped, including during cleanup.
    pub fn open_attachment(
        spec: InterfaceSpec,
        path: &str,
        address: u8,
        rig: DeviceLock,
    ) -> Result<Self, String> {
        let device = with_attachment(&rig, spec, path, address, Ok)?;
        Self::open_device(spec, &device, Some(rig))
    }

    fn open_device(
        spec: InterfaceSpec,
        device: &rusb::Device<rusb::GlobalContext>,
        rig: Option<DeviceLock>,
    ) -> Result<Self, String> {
        let handle = device
            .open()
            .map_err(|e| format!("Cannot open the USB device: {e}"))?;
        let mut link = Self {
            spec,
            handle,
            opened: Instant::now(),
            detached: false,
            claimed: false,
            finished: false,
            _rig: rig,
        };
        if let Err(error) = link.prepare() {
            return Err(match link.finish() {
                Ok(()) => error,
                Err(cleanup) => format!("{error} Cleanup: {cleanup}"),
            });
        }
        Ok(link)
    }

    fn prepare(&mut self) -> Result<(), String> {
        let interface = self.spec.interface;
        if self
            .handle
            .kernel_driver_active(interface)
            .map_err(|e| format!("Kernel driver check: {e}"))?
        {
            self.handle
                .detach_kernel_driver(interface)
                .map_err(|e| format!("Detach interface {interface}: {e}"))?;
            self.detached = true;
        }
        self.handle
            .claim_interface(interface)
            .map_err(|e| format!("Claim interface {interface}: {e}"))?;
        self.claimed = true;
        self.handle
            .set_alternate_setting(interface, self.spec.altsetting)
            .map_err(|e| {
                format!(
                    "Altsetting {} on interface {interface}: {e}",
                    self.spec.altsetting
                )
            })
    }

    /// (kernel driver was detached, interface is claimed)
    pub fn state(&self) -> (bool, bool) {
        (self.detached, self.claimed)
    }

    /// Release the interface and give it back to the kernel driver.
    pub fn finish(&mut self) -> Result<(), String> {
        self.finished = true;
        let interface = self.spec.interface;
        let mut errors = Vec::new();
        if self.claimed {
            match self.handle.release_interface(interface) {
                Ok(()) => self.claimed = false,
                Err(e) => errors.push(format!("release interface {interface}: {e}")),
            }
        }
        if self.detached {
            match self.handle.attach_kernel_driver(interface) {
                Ok(()) => self.detached = false,
                Err(e) => errors.push(format!("reattach kernel driver: {e}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

/// Apply an operation to the single USB device at the verified physical
/// port and enumeration address. Other devices of the same model are ignored.
pub fn with_attachment<T>(
    _rig: &DeviceLock,
    spec: InterfaceSpec,
    path: &str,
    address: u8,
    open: impl FnOnce(rusb::Device<rusb::GlobalContext>) -> Result<T, String>,
) -> Result<T, String> {
    let devices = rusb::devices().map_err(|e| format!("USB list: {e}"))?;
    with_matching_device(
        devices.iter(),
        |device| {
            device.device_descriptor().is_ok_and(|descriptor| {
                descriptor.vendor_id() == spec.vendor && descriptor.product_id() == spec.product
            }) && device.port_numbers().is_ok_and(|ports| {
                matches_attachment(
                    (path, address),
                    device.bus_number(),
                    &ports,
                    device.address(),
                )
            })
        },
        open,
    )
}

fn matches_attachment(expected: (&str, u8), bus: u8, ports: &[u8], address: u8) -> bool {
    address == expected.1 && {
        let ports = ports
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(".");
        format!("{bus}-{ports}") == expected.0
    }
}

fn with_matching_device<D, T>(
    devices: impl IntoIterator<Item = D>,
    matches: impl Fn(&D) -> bool,
    open: impl FnOnce(D) -> Result<T, String>,
) -> Result<T, String> {
    let mut matches = devices.into_iter().filter(matches);
    match (matches.next(), matches.next()) {
        (Some(device), None) => open(device),
        _ => Err("The PODxt Live USB attachment changed. Wait for automatic identity.".into()),
    }
}

impl Drop for UsbInterfaceLink {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

impl Link for UsbInterfaceLink {
    fn path(&self) -> Path {
        self.spec.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        for chunk in bytes.chunks(self.spec.packet) {
            let sent = self
                .handle
                .write_interrupt(self.spec.endpoint_out, chunk, WRITE_TIMEOUT)
                .map_err(|e| format!("USB write: {e}"))?;
            if sent != chunk.len() {
                return Err("Short USB write.".into());
            }
            std::thread::sleep(self.spec.chunk_gap);
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let mut packet = vec![0; self.spec.packet];
        // A zero timeout would mean "wait forever" in libusb.
        match self.handle.read_interrupt(
            self.spec.endpoint_in,
            &mut packet,
            wait.max(Duration::from_millis(1)),
        ) {
            Ok(length) => {
                packet.truncate(length);
                Ok(Some(packet))
            }
            Err(rusb::Error::Timeout) => Ok(None),
            Err(e) => Err(format!("USB read: {e}")),
        }
    }

    fn elapsed(&self) -> Duration {
        self.opened.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_verified_port_and_address_even_with_another_podxt_present() {
        let opened = with_matching_device(
            [(8, &[3][..], 10), (8, &[2, 1][..], 11)],
            |&(bus, ports, address)| matches_attachment(("8-2.1", 11), bus, ports, address),
            |(_, _, address)| Ok(address),
        );
        assert_eq!(opened, Ok(11));
    }

    #[test]
    fn changed_port_or_same_port_replug_leaves_the_driver_and_transport_untouched() {
        let result = with_matching_device(
            [(8, &[3][..], 11), (8, &[2, 1][..], 12)],
            |&(bus, ports, address)| matches_attachment(("8-2.1", 11), bus, ports, address),
            |_| -> Result<(), String> { panic!("mismatch must not open, detach, or send") },
        );
        assert!(result.unwrap_err().contains("USB attachment changed"));
    }
}

//! A USB interrupt-interface link (libusb via `rusb`).
//!
//! Opening detaches the kernel driver from the one named interface only,
//! claims it and selects the altsetting; [`UsbInterfaceLink::finish`] releases
//! it and gives it back to the kernel driver. Other interfaces are untouched.

use crate::session::{Link, Path};
use std::time::{Duration, Instant};

/// Where one control interface lives and how it frames writes.
#[derive(Clone, Copy, Debug)]
pub struct InterfaceSpec {
    pub vendor: u16,
    pub product: u16,
    pub interface: u8,
    pub altsetting: u8,
    pub endpoint_out: u8,
    pub endpoint_in: u8,
    pub packet: usize,
    pub chunk_gap: Duration,
    pub path: Path,
}

/// Variax control through PODxt Live USB interface 1 (kernel `snd_usb_variax`).
pub const VARIAX_VIA_PODXT_LIVE: InterfaceSpec = InterfaceSpec {
    vendor: 0x0e41,
    product: 0x4650,
    interface: 1,
    altsetting: 1,
    endpoint_out: 0x05,
    endpoint_in: 0x86,
    packet: 16,
    chunk_gap: Duration::from_millis(5),
    path: Path::VariaxViaPodxt,
};

const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct UsbInterfaceLink {
    spec: InterfaceSpec,
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    opened: Instant,
    detached: bool,
    claimed: bool,
    finished: bool,
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
        let handle = found[0]
            .open()
            .map_err(|e| format!("Cannot open the USB device: {e}"))?;
        let mut link = Self {
            spec,
            handle,
            opened: Instant::now(),
            detached: false,
            claimed: false,
            finished: false,
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

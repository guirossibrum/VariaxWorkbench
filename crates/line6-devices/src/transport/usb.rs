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

#[cfg(any(target_os = "macos", test))]
pub mod macos;
mod podxt;
pub use podxt::PODXT_DIRECT_USB_APPROVED;

const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

pub struct UsbInterfaceLink<Rig = Option<DeviceLock>> {
    spec: InterfaceSpec,
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    opened: Instant,
    detached: bool,
    claimed: bool,
    finished: bool,
    _rig: Rig,
}

impl UsbInterfaceLink {
    pub fn open(spec: InterfaceSpec) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        macos::check_open(spec, false)?;
        podxt::check_open(spec, false)?;
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

    /// Open direct MIDI on PODxt Live interface 0 of the named attachment.
    /// The lock is retained through release and verification of driver return.
    pub fn open_podxt_midi(path: &str, address: u8, rig: DeviceLock) -> Result<Self, String> {
        Self::open_attachment(podxt::SPEC, path, address, rig)
    }

    /// Select the verified attachment before opening or detaching anything.
    /// The link owns the rig lock until it is dropped, including during cleanup.
    pub fn open_attachment(
        spec: InterfaceSpec,
        path: &str,
        address: u8,
        rig: DeviceLock,
    ) -> Result<Self, String> {
        let device = with_matching_attachment(&rig, spec, path, address, Ok)?;
        Self::open_device(spec, &device, Some(rig))
    }

    /// Open the verified attachment while the worker keeps its rig lock.
    /// The returned link borrows that lock through interface and driver cleanup.
    ///
    /// ```compile_fail
    /// use line6_devices::device_lock::DeviceLock;
    /// use line6_devices::transport::usb::{UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE};
    ///
    /// let rig = DeviceLock::acquire().unwrap();
    /// let mut link =
    ///     UsbInterfaceLink::open_attachment_borrowed(VARIAX_VIA_PODXT_LIVE, "8-2.1", 11, &rig)
    ///         .unwrap();
    /// drop(rig);
    /// link.finish().unwrap();
    /// ```
    pub fn open_attachment_borrowed<'rig>(
        spec: InterfaceSpec,
        path: &str,
        address: u8,
        rig: &'rig DeviceLock,
    ) -> Result<UsbInterfaceLink<&'rig DeviceLock>, String> {
        with_matching_attachment(rig, spec, path, address, |device| {
            UsbInterfaceLink::open_device(spec, &device, rig)
        })
    }
}

impl<Rig> UsbInterfaceLink<Rig> {
    fn open_device(
        spec: InterfaceSpec,
        device: &rusb::Device<rusb::GlobalContext>,
        rig: Rig,
    ) -> Result<Self, String> {
        if cfg!(target_os = "macos") || podxt::targets_interface(spec) {
            podxt::verify_descriptor(device, spec)?;
        }
        let handle = device.open().map_err(|error| {
            #[cfg(target_os = "macos")]
            return macos::open_error(error);
            #[cfg(not(target_os = "macos"))]
            format!("Cannot open the USB device: {error}")
        })?;
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
        #[cfg(target_os = "macos")]
        macos::check_interfaces(|number| self.handle.kernel_driver_active(number))?;
        #[cfg(not(target_os = "macos"))]
        {
            let driver_active = self
                .handle
                .kernel_driver_active(interface)
                .map_err(|e| format!("Kernel driver check: {e}"))?;
            #[cfg(target_os = "linux")]
            if podxt::targets_interface(self.spec) {
                if !driver_active {
                    return Err("PODxt Live interface 0 has no active kernel driver.".into());
                }
                podxt::require_driver(&self.handle)?;
            }
            if driver_active {
                self.handle
                    .detach_kernel_driver(interface)
                    .map_err(|e| format!("Detach interface {interface}: {e}"))?;
                self.detached = true;
            }
        }
        self.handle.claim_interface(interface).map_err(|error| {
            #[cfg(target_os = "macos")]
            return macos::open_error(error);
            #[cfg(not(target_os = "macos"))]
            format!("Claim interface {interface}: {error}")
        })?;
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

    /// Bus and port path of the USB handle used for this capture.
    pub fn usb_port_path(&self) -> Result<String, String> {
        let device = self.handle.device();
        let ports = device
            .port_numbers()
            .map_err(|error| format!("USB port path: {error}"))?;
        Ok(attachment_path(device.bus_number(), &ports))
    }

    /// (kernel driver was detached, interface is claimed)
    pub fn state(&self) -> (bool, bool) {
        (self.detached, self.claimed)
    }

    fn return_detached_driver(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        if podxt::targets_interface(self.spec) {
            return podxt::return_driver(&self.handle);
        }
        self.handle
            .attach_kernel_driver(self.spec.interface)
            .map_err(|error| error.to_string())
    }

    /// Release the interface and give it back to the kernel driver.
    pub fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let interface = self.spec.interface;
        let mut errors = Vec::new();
        let verify_driver = self.claimed || self.detached;
        if self.claimed {
            match self.handle.release_interface(interface) {
                Ok(()) => self.claimed = false,
                Err(e) => errors.push(format!("release interface {interface}: {e}")),
            }
        }
        if verify_driver {
            let result = if self.spec.path == Path::VariaxViaPodxt {
                return_variax_driver(&self.handle, self.detached)
            } else if self.detached {
                self.return_detached_driver()
            } else {
                Ok(())
            };
            match result {
                Ok(()) => self.detached = false,
                Err(error) => errors.push(format!("reattach kernel driver: {error}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

/// Verify the Variax driver after release, including a bounded retry if needed.
pub fn return_variax_driver(
    handle: &rusb::DeviceHandle<rusb::GlobalContext>,
    detached: bool,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let device = handle.device();
        let ports = device
            .port_numbers()
            .map_err(|error| format!("USB port path: {error}"))?;
        let path = attachment_path(device.bus_number(), &ports);
        crate::family::variax::driver::linux::restore(
            || variax_driver_binding(&path, device.address()),
            || {
                handle
                    .attach_kernel_driver(VARIAX_VIA_PODXT_LIVE.interface)
                    .map_err(|error| error.to_string())
            },
            detached,
            std::thread::sleep,
            |line| eprintln!("{line}"),
        )
        .inspect_err(|error| eprintln!("PODxt Live interface 1 cleanup: {error}"))
    }
    #[cfg(not(target_os = "linux"))]
    if detached {
        handle
            .attach_kernel_driver(VARIAX_VIA_PODXT_LIVE.interface)
            .map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use crate::family::variax::driver::linux::Binding as VariaxDriverBinding;

#[cfg(target_os = "linux")]
pub fn variax_driver_binding(path: &str, address: u8) -> Result<VariaxDriverBinding, String> {
    if cfg!(test) {
        return Err("Test builds must inject synthetic USB metadata.".into());
    }
    crate::family::variax::driver::linux::check(std::path::Path::new("/"), path, address)
}

/// Restore the binding on the verified attachment. Does not claim the
/// interface or send device commands. The caller retains the rig lock.
#[cfg(target_os = "linux")]
pub fn reattach_variax_driver(rig: &DeviceLock, path: &str, address: u8) -> Result<(), String> {
    if cfg!(test) {
        return Err("Test builds do not open a USB device.".into());
    }
    with_attachment(rig, VARIAX_VIA_PODXT_LIVE, path, address, |device| {
        let handle = device.open().map_err(|error| {
            format!("Cannot open PODxt Live USB to reattach the guitar interface driver: {error}.")
        })?;
        return_variax_driver(&handle, false)
    })
}

/// Apply an operation to the single USB device at the verified physical
/// port and enumeration address. Other devices of the same model are ignored.
#[cfg(not(target_os = "macos"))]
pub fn with_attachment<T>(
    rig: &DeviceLock,
    spec: InterfaceSpec,
    path: &str,
    address: u8,
    open: impl FnOnce(rusb::Device<rusb::GlobalContext>) -> Result<T, String>,
) -> Result<T, String> {
    with_matching_attachment(rig, spec, path, address, open)
}

fn with_matching_attachment<T>(
    _rig: &DeviceLock,
    spec: InterfaceSpec,
    path: &str,
    address: u8,
    open: impl FnOnce(rusb::Device<rusb::GlobalContext>) -> Result<T, String>,
) -> Result<T, String> {
    #[cfg(target_os = "macos")]
    macos::check_open(spec, true)?;
    podxt::check_open(spec, true)?;
    let devices = rusb::devices().map_err(|e| format!("USB list: {e}"))?;
    with_matching_device(
        devices.iter(),
        |device| {
            device.device_descriptor().is_ok_and(|descriptor| {
                descriptor.vendor_id() == spec.vendor && descriptor.product_id() == spec.product
            }) && device.port_numbers().is_ok_and(|ports| {
                (!cfg!(target_os = "macos") || !ports.is_empty())
                    && matches_attachment(
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
    address == expected.1 && attachment_path(bus, ports) == expected.0
}

fn attachment_path(bus: u8, ports: &[u8]) -> String {
    let ports = ports
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".");
    format!("{bus}-{ports}")
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

impl<Rig> Drop for UsbInterfaceLink<Rig> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

impl<Rig> Link for UsbInterfaceLink<Rig> {
    fn path(&self) -> Path {
        self.spec.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.write_usb_packets(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        read_packet(self.spec, wait, |endpoint, packet, timeout| {
            self.handle.read_interrupt(endpoint, packet, timeout)
        })
    }

    fn elapsed(&self) -> Duration {
        self.opened.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

impl<Rig> UsbInterfaceLink<Rig> {
    fn write_usb_packets(&mut self, bytes: &[u8]) -> Result<(), String> {
        write_packets(
            self.spec,
            bytes,
            |endpoint, chunk, timeout| self.handle.write_interrupt(endpoint, chunk, timeout),
            std::thread::sleep,
        )
    }
}

fn write_packets(
    spec: InterfaceSpec,
    bytes: &[u8],
    mut write: impl FnMut(u8, &[u8], Duration) -> rusb::Result<usize>,
    mut pause: impl FnMut(Duration),
) -> Result<(), String> {
    for chunk in bytes.chunks(spec.packet) {
        let sent = write(spec.endpoint_out, chunk, WRITE_TIMEOUT)
            .map_err(|error| format!("USB write: {error}"))?;
        if sent != chunk.len() {
            return Err("Short USB write.".into());
        }
        pause(spec.chunk_gap);
    }
    Ok(())
}

fn read_packet(
    spec: InterfaceSpec,
    wait: Duration,
    read: impl FnOnce(u8, &mut [u8], Duration) -> rusb::Result<usize>,
) -> Result<Option<Vec<u8>>, String> {
    let mut packet = vec![0; spec.packet];
    // A zero timeout would mean "wait forever" in libusb.
    match read(
        spec.endpoint_in,
        &mut packet,
        wait.max(Duration::from_millis(1)),
    ) {
        Ok(length) => {
            packet.truncate(length);
            if podxt::targets_interface(spec) {
                podxt::normalize_received(&mut packet);
            }
            Ok(Some(packet))
        }
        Err(rusb::Error::Timeout) => Ok(None),
        Err(error) => Err(format!("USB read: {error}")),
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

    #[cfg(target_os = "linux")]
    #[test]
    fn borrowed_open_and_driver_return_leave_the_rig_locked_by_the_worker() {
        use std::cell::Cell;
        use std::fs::{self, File, TryLockError};

        struct FakeBridge<'rig> {
            _rig: &'rig DeviceLock,
            address: u8,
            lock_file: &'rig std::path::Path,
            finished: bool,
            driver_returns: &'rig Cell<usize>,
        }

        impl FakeBridge<'_> {
            fn finish(&mut self) {
                assert!(matches!(
                    File::open(self.lock_file).unwrap().try_lock(),
                    Err(TryLockError::WouldBlock)
                ));
                self.driver_returns.set(self.driver_returns.get() + 1);
                self.finished = true;
            }
        }

        impl Drop for FakeBridge<'_> {
            fn drop(&mut self) {
                if !self.finished {
                    self.finish();
                }
            }
        }

        let folder = std::env::temp_dir().join(format!(
            "variax-usb-borrowed-lock-test-{}",
            std::process::id()
        ));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let rig = DeviceLock::acquire_test_lock(&path).unwrap();
        let candidates = [(8, &[2, 1][..], 11), (8, &[3][..], 12)];
        let driver_returns = Cell::new(0);
        let open = |(_, _, address)| {
            assert!(matches!(
                File::open(&path).unwrap().try_lock(),
                Err(TryLockError::WouldBlock)
            ));
            Ok(FakeBridge {
                _rig: &rig,
                address,
                lock_file: &path,
                finished: false,
                driver_returns: &driver_returns,
            })
        };

        for explicit_cleanup in [true, false] {
            let mut bridge = with_matching_device(
                candidates,
                |&(bus, ports, address)| matches_attachment(("8-2.1", 11), bus, ports, address),
                open,
            )
            .unwrap();
            assert_eq!(bridge.address, 11);
            if explicit_cleanup {
                bridge.finish();
            }
            drop(bridge);
            assert!(matches!(
                File::open(&path).unwrap().try_lock(),
                Err(TryLockError::WouldBlock)
            ));
        }
        let mismatch: Result<(), String> = with_matching_device(
            candidates,
            |&(bus, ports, address)| matches_attachment(("8-2.1", 12), bus, ports, address),
            |device| {
                let _bridge = open(device)?;
                panic!("mismatch must not detach or send")
            },
        );
        assert!(mismatch.is_err());
        assert_eq!(driver_returns.get(), 2);
        drop(rig);
        assert!(File::open(&path).unwrap().try_lock().is_ok());
        fs::remove_dir_all(&folder).unwrap();
    }
}

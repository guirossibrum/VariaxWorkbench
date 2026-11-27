//! Narrow IOKit adapter. It only reads registry services, properties and
//! topology. No plug-in, user client or USB device/interface handle is opened.

#![cfg(any(target_os = "macos", test))]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
/// Read-only metadata for a device service. Published interfaces are not a
/// claim or proof of availability to a client.
pub struct Record {
    pub registry_id: u64,
    pub location_id: Option<u32>,
    pub address: Option<u8>,
    pub ports: Vec<u8>,
    pub midi_interfaces_present: bool,
}
#[cfg(target_os = "macos")]
use std::ffi::{CStr, c_void};
#[cfg(target_os = "macos")]
use std::ptr;

#[cfg(target_os = "macos")]
type CfRef = *const c_void;

#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(class: *const std::ffi::c_char) -> CfRef;
    fn IOServiceGetMatchingServices(port: u32, matching: CfRef, out: *mut u32) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOObjectConformsTo(object: u32, class: *const std::ffi::c_char) -> u32;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, out: *mut u64) -> i32;
    fn IORegistryEntryGetParentEntry(
        entry: u32,
        plane: *const std::ffi::c_char,
        out: *mut u32,
    ) -> i32;
    fn IORegistryEntryGetChildIterator(
        entry: u32,
        plane: *const std::ffi::c_char,
        out: *mut u32,
    ) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CfRef,
        allocator: CfRef,
        options: u32,
    ) -> CfRef;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: CfRef,
        string: *const std::ffi::c_char,
        encoding: u32,
    ) -> CfRef;
    fn CFGetTypeID(value: CfRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(value: CfRef, number_type: isize, output: *mut i64) -> u8;
    fn CFDataGetTypeID() -> usize;
    fn CFDataGetLength(value: CfRef) -> isize;
    fn CFDataGetBytePtr(value: CfRef) -> *const u8;
    fn CFRelease(value: CfRef);
}

#[cfg(target_os = "macos")]
struct IoObject(u32);

#[cfg(target_os = "macos")]
impl IoObject {
    fn new(raw: u32) -> Option<Self> {
        (raw != 0).then(|| Self(raw))
    }

    fn next(&self) -> Option<Self> {
        // An iterator holds returned objects for the caller to release.
        Self::new(unsafe { IOIteratorNext(self.0) })
    }

    fn conforms(&self, class: &CStr) -> bool {
        // The held object and the C string remain valid for the call.
        unsafe { IOObjectConformsTo(self.0, class.as_ptr()) != 0 }
    }

    fn id(&self) -> Option<u64> {
        let mut id = 0;
        // The held service and writable output remain valid for the call.
        (unsafe { IORegistryEntryGetRegistryEntryID(self.0, &mut id) } == 0).then_some(id)
    }

    fn parent(&self, plane: &CStr) -> Option<Self> {
        let mut raw = 0;
        // The API returns a retained entry on success.
        let status = unsafe { IORegistryEntryGetParentEntry(self.0, plane.as_ptr(), &mut raw) };
        (status == 0).then(|| Self::new(raw)).flatten()
    }

    fn children(&self) -> Option<Self> {
        let mut raw = 0;
        // The API returns a retained iterator on success.
        let status =
            unsafe { IORegistryEntryGetChildIterator(self.0, c"IOService".as_ptr(), &mut raw) };
        (status == 0).then(|| Self::new(raw)).flatten()
    }

    fn property(&self, key: &CStr) -> Option<CfObject> {
        const UTF8: u32 = 0x0800_0100;
        // Both Create functions return owned CoreFoundation objects.
        let key =
            CfObject::new(unsafe { CFStringCreateWithCString(ptr::null(), key.as_ptr(), UTF8) })?;
        CfObject::new(unsafe { IORegistryEntryCreateCFProperty(self.0, key.0, ptr::null(), 0) })
    }

    fn number(&self, key: &CStr) -> Option<i64> {
        let property = self.property(key)?;
        let mut result = 0;
        const SINT64: isize = 4;
        // Type check before using the number API, with a writable i64 result.
        (unsafe {
            CFGetTypeID(property.0) == CFNumberGetTypeID()
                && CFNumberGetValue(property.0, SINT64, &mut result) != 0
        })
        .then_some(result)
    }

    fn port(&self) -> Option<u8> {
        self.number(c"PortNum")
            .and_then(|value| value.try_into().ok())
            .or_else(|| {
                // Match Darwin libusb's fallback to the IOService parent's port data.
                let property = self.parent(c"IOService")?.property(c"port")?;
                // The byte is valid while the checked CFData object is retained.
                unsafe {
                    if CFGetTypeID(property.0) != CFDataGetTypeID()
                        || CFDataGetLength(property.0) < 1
                    {
                        return None;
                    }
                    CFDataGetBytePtr(property.0).as_ref().copied()
                }
            })
            .filter(|port| *port != 0)
    }

    fn usb_parent(&self) -> Option<Self> {
        // The vendored get_device_parent_sessionID calls kIOUSBPlane.
        // IOService is used only for the separate parent port-data fallback.
        let mut next = self.parent(c"IOUSB");
        while let Some(parent) = next {
            if parent.number(c"sessionID").is_some() {
                return Some(parent);
            }
            next = parent.parent(c"IOUSB");
        }
        None
    }

    fn port_chain(&self) -> Option<Vec<u8>> {
        let mut ports = vec![self.port()?];
        let mut next = self.usb_parent();
        while let Some(parent) = next {
            next = parent.usb_parent();
            // A root hub without a port does not contribute to the chain.
            match parent.port() {
                Some(port) => ports.push(port),
                None if next.is_none() => break,
                None => return None,
            }
        }
        ports.reverse();
        Some(ports)
    }

    fn published_interfaces(&self) -> bool {
        let Some(children) = self.children() else {
            return false;
        };
        let mut midi = [false; 2];
        while let Some(child) = children.next() {
            if (child.conforms(c"IOUSBHostInterface") || child.conforms(c"IOUSBInterface"))
                && let Some(number @ 0..=1) = child.number(c"bInterfaceNumber")
            {
                midi[number as usize] = true;
            }
        }
        midi == [true; 2]
    }
}

#[cfg(target_os = "macos")]
impl Drop for IoObject {
    fn drop(&mut self) {
        // IoObject owns a single reference returned by IOKit.
        unsafe {
            IOObjectRelease(self.0);
        }
    }
}

#[cfg(target_os = "macos")]
struct CfObject(CfRef);

#[cfg(target_os = "macos")]
impl CfObject {
    fn new(raw: CfRef) -> Option<Self> {
        (!raw.is_null()).then(|| Self(raw))
    }
}

#[cfg(target_os = "macos")]
impl Drop for CfObject {
    fn drop(&mut self) {
        // CfObject owns a single reference from a Create function.
        unsafe {
            CFRelease(self.0);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn records(vendor: u16, product: u16) -> Result<Vec<Record>, String> {
    let mut raw = 0;
    // IOServiceGetMatchingServices consumes the matching dictionary.
    let status = unsafe {
        let matching = IOServiceMatching(c"IOUSBHostDevice".as_ptr());
        if matching.is_null() {
            return Err("Cannot create the macOS USB registry query.".into());
        }
        IOServiceGetMatchingServices(0, matching, &mut raw)
    };
    let iterator = IoObject::new(raw).map(|iterator| std::iter::from_fn(move || iterator.next()));
    scan_matching_services(status, iterator, |device| {
        if device.number(c"idVendor") != Some(vendor.into())
            || device.number(c"idProduct") != Some(product.into())
        {
            return None;
        }
        Some(Record {
            registry_id: device.id()?,
            location_id: device.number(c"locationID").and_then(|value| {
                u32::try_from(value)
                    .ok()
                    .or_else(|| i32::try_from(value).ok().map(|signed| signed as u32))
            }),
            address: device
                .number(c"USB Address")
                .and_then(|value| u8::try_from(value).ok())
                .filter(|address| *address != 0),
            ports: device.port_chain().unwrap_or_default(),
            midi_interfaces_present: device.published_interfaces(),
        })
    })
}

fn scan_matching_services<Service, Entry>(
    status: i32,
    iterator: Option<impl Iterator<Item = Service>>,
    read: impl FnMut(Service) -> Option<Entry>,
) -> Result<Vec<Entry>, String> {
    if status != 0 {
        return Err(format!(
            "Cannot list macOS USB registry devices ({status:#x})."
        ));
    }
    let iterator = iterator.ok_or("macOS USB registry returned no iterator.")?;
    Ok(iterator.filter_map(read).collect())
}

#[cfg(test)]
mod tests {
    use super::scan_matching_services;
    use std::cell::Cell;

    #[derive(Default)]
    struct Calls {
        next: Cell<u32>,
        release: Cell<u32>,
    }

    struct EmptyIterator<'a>(&'a Calls);

    impl Iterator for EmptyIterator<'_> {
        type Item = ();

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next.set(self.0.next.get() + 1);
            None
        }
    }

    impl Drop for EmptyIterator<'_> {
        fn drop(&mut self) {
            self.0.release.set(self.0.release.get() + 1);
        }
    }

    fn unexpected_service((): ()) -> Option<()> {
        panic!("an empty or failed scan must never read a service")
    }

    #[test]
    fn success_without_an_iterator_is_an_unavailable_scan() {
        assert_eq!(
            scan_matching_services(0, None::<EmptyIterator<'_>>, unexpected_service),
            Err("macOS USB registry returned no iterator.".into())
        );
    }

    #[test]
    fn nonzero_status_remains_the_error_with_or_without_an_iterator() {
        let calls = Calls::default();
        let status = 0xe000_02bdu32 as i32;
        for iterator in [None, Some(EmptyIterator(&calls))] {
            assert_eq!(
                scan_matching_services(status, iterator, unexpected_service),
                Err("Cannot list macOS USB registry devices (0xe00002bd).".into())
            );
        }
        assert_eq!(calls.next.get(), 0);
        assert_eq!(calls.release.get(), 1);
    }

    #[test]
    fn valid_empty_scan_iterates_and_releases_normally() {
        let calls = Calls::default();
        let records =
            scan_matching_services(0, Some(EmptyIterator(&calls)), unexpected_service).unwrap();
        assert!(records.is_empty());
        assert_eq!(calls.next.get(), 1);
        assert_eq!(calls.release.get(), 1);
    }
}

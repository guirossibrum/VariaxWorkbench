//! Device-level observations. Registry presence establishes neither a usable
//! configuration nor access to a USB interface.

use super::macos::Attachment;
#[cfg(any(target_os = "macos", test))]
use super::{attachment_path, macos::PODXT_MIDI};
use crate::device_lock::DeviceLock;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPodxt {
    pub registry_id: u64,
    pub location_id: Option<u32>,
    pub address: Option<u8>,
    /// Published services only. An open or claim can still be refused.
    pub midi_interfaces_present: bool,
    route: Option<String>,
}

impl ObservedPodxt {
    /// Display label when a portable bus/port route is unavailable.
    pub fn label(&self) -> String {
        match self.location_id {
            Some(location) => format!("USB location {location:08x}"),
            None => "USB location unavailable".into(),
        }
    }

    pub fn route(&self) -> Option<&str> {
        self.route.as_deref()
    }

    pub fn matches_generation(&self, other: &Self) -> bool {
        self.registry_id == other.registry_id
            && self.location_id == other.location_id
            && self.address == other.address
            && self.route == other.route
    }

    /// A candidate for the existing exact libusb opener. This is not
    /// confirmation that its active descriptor or ownership checks will pass.
    pub fn attachment(&self, inventory: &[Self]) -> Option<Attachment> {
        let path = self.route.as_ref()?;
        let address = self.address?;
        let mut matching = inventory.iter().filter(|record| {
            record.location_id == self.location_id || record.route.as_ref() == Some(path)
        });
        let current = matching.next()?;
        if !current.matches_generation(self)
            || !current.midi_interfaces_present
            || matching.next().is_some()
        {
            return None;
        }
        Some(Attachment {
            path: path.clone(),
            address,
        })
    }
}

#[cfg(any(target_os = "macos", test))]
struct Record {
    vendor: u16,
    product: u16,
    registry_id: u64,
    location_id: Option<u32>,
    address: Option<u8>,
    ports: Vec<u8>,
    midi_interfaces_present: bool,
}

#[cfg(any(target_os = "macos", test))]
impl Record {
    fn podxt(self) -> Option<ObservedPodxt> {
        if (self.vendor, self.product) != (PODXT_MIDI.vendor, PODXT_MIDI.product) {
            return None;
        }
        let route = self
            .location_id
            .filter(|_| !self.ports.is_empty())
            .map(|location| attachment_path((location >> 24) as u8, &self.ports));
        Some(ObservedPodxt {
            registry_id: self.registry_id,
            location_id: self.location_id,
            address: self.address,
            midi_interfaces_present: self.midi_interfaces_present,
            route,
        })
    }
}

pub fn inventory() -> Result<Vec<ObservedPodxt>, String> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        macos_usb_inventory::records(PODXT_MIDI.vendor, PODXT_MIDI.product).map(|records| {
            records
                .into_iter()
                .filter_map(|record| {
                    Record {
                        vendor: PODXT_MIDI.vendor,
                        product: PODXT_MIDI.product,
                        registry_id: record.registry_id,
                        location_id: record.location_id,
                        address: record.address,
                        ports: record.ports,
                        midi_interfaces_present: record.midi_interfaces_present,
                    }
                    .podxt()
                })
                .collect()
        })
    }
    #[cfg(not(all(target_os = "macos", not(test))))]
    {
        Err("Passive Mac USB inventory is unavailable in this build.".into())
    }
}

/// Recheck the passive generation immediately before the existing exact
/// libusb validation. The caller holds the rig lock through both checks.
pub fn verify_candidate(
    _rig: &DeviceLock,
    registry_id: u64,
    expected: &Attachment,
) -> Result<(), String> {
    let devices = inventory()?;
    if candidate_matches(&devices, registry_id, expected) {
        Ok(())
    } else {
        Err("The PODxt Live USB attachment changed or its interfaces are unavailable.".into())
    }
}

fn candidate_matches(devices: &[ObservedPodxt], registry_id: u64, expected: &Attachment) -> bool {
    devices.iter().any(|device| {
        device.registry_id == registry_id && device.attachment(devices).as_ref() == Some(expected)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: u64, location: u32, address: u8, ports: &[u8], published: bool) -> Record {
        Record {
            vendor: PODXT_MIDI.vendor,
            product: PODXT_MIDI.product,
            registry_id: id,
            location_id: Some(location),
            address: Some(address),
            ports: ports.to_vec(),
            midi_interfaces_present: published,
        }
    }

    #[test]
    fn direct_and_populated_hub_are_inventory_even_without_interfaces_or_access() {
        let direct = record(10, 0x01200000, 2, &[2], false);
        let published = record(11, 0x01430000, 3, &[4, 3], true);
        let mut neighbor = record(12, 0x01450000, 4, &[4, 5], true);
        neighbor.product = 0x1234;
        let inventory: Vec<_> = [direct, published, neighbor]
            .into_iter()
            .filter_map(Record::podxt)
            .collect();

        let [unconfigured, behind_hub] = inventory.as_slice() else {
            panic!("both PODxts should be listed");
        };
        assert!(unconfigured.attachment(&inventory).is_none());
        let candidate = behind_hub.attachment(&inventory).unwrap();
        assert_eq!(candidate.path, "1-4.3");
        assert_eq!(candidate.address, 3);
        // Even when the connection later reports Busy, inventory only records
        // published services and attachment identity.
        assert!(super::super::macos::open_error(rusb::Error::Busy).contains("held"));
        assert_eq!(behind_hub.attachment(&inventory), Some(candidate));
        assert!(super::inventory().is_err());
    }

    #[test]
    fn neighbor_hotplug_does_not_replace_the_selected_podxt() {
        let selected = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        let mut neighbor = record(13, 0x01450000, 5, &[4, 5], true);
        neighbor.vendor = 0xabcd;
        assert!(neighbor.podxt().is_none());
        let current = record(11, 0x01430000, 3, &[4, 3], false).podxt().unwrap();
        assert!(selected.matches_generation(&current));
    }

    #[test]
    fn reused_address_and_ambiguous_location_do_not_supply_the_old_candidate() {
        let first = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        let replacement = record(12, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        assert!(!first.matches_generation(&replacement));
        let expected = first.attachment(std::slice::from_ref(&first)).unwrap();
        assert!(candidate_matches(
            std::slice::from_ref(&first),
            11,
            &expected
        ));
        assert!(!candidate_matches(
            std::slice::from_ref(&replacement),
            11,
            &expected
        ));
        let ambiguous = [first, replacement];
        assert!(
            ambiguous
                .iter()
                .all(|device| device.attachment(&ambiguous).is_none())
        );
    }
}

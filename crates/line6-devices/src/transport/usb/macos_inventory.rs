//! Device-level observations. Registry presence establishes neither a usable
//! configuration nor access to a USB interface.

use super::macos::Attachment;
#[cfg(any(target_os = "macos", test))]
use super::{attachment_path, macos::PODXT_MIDI};
use crate::device_lock::DeviceLock;
use macos_usb_inventory::PublishedInterfaces;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPodxt {
    pub registry_id: u64,
    pub location_id: Option<u32>,
    pub address: Option<u8>,
    interfaces: PublishedInterfaces,
    route: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassiveReadinessBlock {
    MissingCoordinates { address: bool, route: bool },
    Ambiguous { matches: usize },
    UnreadableChildren { seen: [bool; 2] },
    UnreadableInterfaceNumber { seen: [bool; 2] },
    MissingInterfaces { seen: [bool; 2] },
}

/// Assess a row from the raw scan, before assigning display keys.
pub fn assess_candidate(
    inventory: &[ObservedPodxt],
    index: usize,
) -> Result<Attachment, PassiveReadinessBlock> {
    let current = &inventory[index];
    let (Some(path), Some(address)) = (current.route.as_ref(), current.address) else {
        return Err(PassiveReadinessBlock::MissingCoordinates {
            address: current.address.is_none(),
            route: current.route.is_none(),
        });
    };
    let matches = current.coordinate_matches(inventory).count();
    if matches != 1 {
        return Err(PassiveReadinessBlock::Ambiguous { matches });
    }
    match current.interfaces {
        // Preserve positive eligibility even if other children were unreadable.
        PublishedInterfaces::Read {
            seen: [true, true], ..
        }
        | PublishedInterfaces::IncompleteChildren { seen: [true, true] } => Ok(Attachment {
            path: path.clone(),
            address,
        }),
        PublishedInterfaces::IncompleteChildren { seen } => {
            Err(PassiveReadinessBlock::UnreadableChildren { seen })
        }
        PublishedInterfaces::Read {
            seen,
            unknown_number: true,
        } => Err(PassiveReadinessBlock::UnreadableInterfaceNumber { seen }),
        PublishedInterfaces::Read {
            seen,
            unknown_number: false,
        } => Err(PassiveReadinessBlock::MissingInterfaces { seen }),
    }
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
        let (index, current) = self.coordinate_matches(inventory).next()?;
        if !current.matches_generation(self) {
            return None;
        }
        assess_candidate(inventory, index).ok()
    }

    fn coordinate_matches<'a>(
        &self,
        inventory: &'a [Self],
    ) -> impl Iterator<Item = (usize, &'a Self)> {
        inventory.iter().enumerate().filter(|(_, record)| {
            record.location_id == self.location_id
                || self
                    .route()
                    .is_some_and(|route| record.route() == Some(route))
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
    interfaces: PublishedInterfaces,
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
            interfaces: self.interfaces,
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
                        interfaces: record.interfaces,
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

/// Recheck an observation from a passive scan before opening its USB interface.
/// The caller holds the rig lock through the scan and the exact USB open.
pub fn with_observed_attachment<T>(
    _rig: &DeviceLock,
    observed: &ObservedPodxt,
    open: impl FnOnce(Attachment) -> Result<T, String>,
) -> Result<T, String> {
    open_observed_attachment(observed, &inventory()?, open)
}

fn open_observed_attachment<T>(
    observed: &ObservedPodxt,
    devices: &[ObservedPodxt],
    open: impl FnOnce(Attachment) -> Result<T, String>,
) -> Result<T, String> {
    let attachment = observed.attachment(devices).ok_or(
        "The PODxt Live USB attachment changed or its interfaces are unavailable. Run Device Check step 1 again.",
    )?;
    open(attachment)
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
            interfaces: PublishedInterfaces::Read {
                seen: [published; 2],
                unknown_number: false,
            },
        }
    }

    #[test]
    fn coordinate_gaps_take_precedence_over_ambiguity_and_publication() {
        let ready = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        for (address, route) in [(true, false), (false, true), (true, true)] {
            let mut probe = ready.clone();
            if address {
                probe.address = None;
            }
            if route {
                probe.route = None;
            }
            probe.interfaces = PublishedInterfaces::IncompleteChildren { seen: [false; 2] };
            let scan = [probe, ready.clone()];
            assert_eq!(
                assess_candidate(&scan, 0),
                Err(PassiveReadinessBlock::MissingCoordinates { address, route }),
            );
            assert_eq!(
                assess_candidate(&scan, 1),
                Err(PassiveReadinessBlock::Ambiguous { matches: 2 }),
            );
        }
    }

    #[test]
    fn ambiguity_counts_a_location_or_route_match_once_and_ignores_unrelated_rows() {
        let ready = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        let same_location = record(12, 0x01430000, 4, &[4, 5], false).podxt().unwrap();
        let same_route = record(13, 0x01431000, 5, &[4, 3], false).podxt().unwrap();
        assert_eq!(
            assess_candidate(
                &[ready.clone(), same_location, same_route, ready.clone()],
                0
            ),
            Err(PassiveReadinessBlock::Ambiguous { matches: 4 }),
        );

        let unrelated = record(14, 0x01460000, 6, &[4, 6], true).podxt().unwrap();
        let distinct = [ready, unrelated];
        assert!(assess_candidate(&distinct, 0).is_ok());
        assert!(assess_candidate(&distinct, 1).is_ok());
        assert_eq!(
            super::super::macos::ready_attachment(&distinct[0], &distinct).unwrap_err(),
            "Expected one PODxt Live on USB, found 2.",
        );
    }

    #[test]
    fn interface_reasons_require_complete_readable_evidence_for_absence() {
        use super::super::macos::passive_readiness_message;

        let mut observed = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        for (seen, absent) in [
            ([false, false], "USB interfaces 0 and 1 were absent"),
            ([true, false], "USB interface 1 was absent"),
            ([false, true], "USB interface 0 was absent"),
        ] {
            for (interfaces, block, text) in [
                (
                    PublishedInterfaces::IncompleteChildren { seen },
                    PassiveReadinessBlock::UnreadableChildren { seen },
                    "macOS USB interface list could not be read",
                ),
                (
                    PublishedInterfaces::Read {
                        seen,
                        unknown_number: true,
                    },
                    PassiveReadinessBlock::UnreadableInterfaceNumber { seen },
                    "macOS USB interface publication could not be determined",
                ),
                (
                    PublishedInterfaces::Read {
                        seen,
                        unknown_number: false,
                    },
                    PassiveReadinessBlock::MissingInterfaces { seen },
                    absent,
                ),
            ] {
                observed.interfaces = interfaces;
                let scan = std::slice::from_ref(&observed);
                assert_eq!(assess_candidate(scan, 0), Err(block));
                assert!(observed.attachment(scan).is_none());
                assert!(passive_readiness_message(Some(block)).contains(text));
            }
        }

        for interfaces in [
            PublishedInterfaces::Read {
                seen: [true; 2],
                unknown_number: true,
            },
            PublishedInterfaces::IncompleteChildren { seen: [true; 2] },
        ] {
            observed.interfaces = interfaces;
            let scan = std::slice::from_ref(&observed);
            assert_eq!(assess_candidate(scan, 0).ok(), observed.attachment(scan));
            assert!(assess_candidate(scan, 0).is_ok());
        }
        assert!(passive_readiness_message(None).ends_with("Identity unverified."));
    }

    #[test]
    fn route_recheck_and_full_generation_recheck_keep_their_distinct_rules() {
        let saved = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        let current = record(11, 0x01431000, 3, &[4, 3], true).podxt().unwrap();
        let expected = saved.attachment(std::slice::from_ref(&saved)).unwrap();
        let scan = [current];
        assert!(candidate_matches(&scan, saved.registry_id, &expected));
        assert!(saved.attachment(&scan).is_none());
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
        assert_eq!(unconfigured.route(), Some("1-2"));
        let configured_direct = record(10, 0x01200000, 2, &[2], true).podxt().unwrap();
        let direct_candidate = configured_direct
            .attachment(std::slice::from_ref(&configured_direct))
            .unwrap();
        assert_eq!(direct_candidate.path, "1-2");
        assert_eq!(direct_candidate.address, 2);
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

    #[test]
    fn manual_identity_refuses_missing_replaced_ambiguous_or_unready_attachments_before_open() {
        let observed = record(11, 0x01430000, 3, &[4, 3], false).podxt().unwrap();
        let matching = record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap();
        let replacement = record(12, 0x01430000, 3, &[4, 3], true).podxt().unwrap();

        for current in [
            vec![],
            vec![replacement.clone()],
            vec![matching, replacement],
            vec![observed.clone()],
        ] {
            let error = open_observed_attachment::<()>(&observed, &current, |_| {
                panic!("a refused generation must never reach the USB opener")
            })
            .unwrap_err();
            assert!(error.contains("Run Device Check step 1 again."));
        }
    }

    #[test]
    fn manual_identity_opens_the_same_generation_once_after_interfaces_publish() {
        let observed = record(11, 0x01430000, 3, &[4, 3], false).podxt().unwrap();
        assert_eq!(observed.route(), Some("1-4.3"));
        assert!(
            observed
                .attachment(std::slice::from_ref(&observed))
                .is_none()
        );
        let current = [
            record(11, 0x01430000, 3, &[4, 3], true).podxt().unwrap(),
            record(12, 0x01450000, 4, &[4, 5], true).podxt().unwrap(),
        ];
        let mut opens = 0;
        let result = open_observed_attachment(&observed, &current, |attachment| {
            opens += 1;
            assert_eq!(attachment.path, "1-4.3");
            assert_eq!(attachment.address, 3);
            Ok("identity link")
        });
        assert_eq!(result.as_deref(), Ok("identity link"));
        assert_eq!(opens, 1);
    }
}

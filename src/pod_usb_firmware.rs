#![forbid(unsafe_code)]
//! PODxt Live USB-firmware component, kept separate from main flash.
//!
//! This is a partial offline record. Monkey's shipped catalog and one earlier
//! descriptor listing are included. The version-query request bytes are still
//! unrecovered. This module does not open a device, send a version query, or
//! install firmware.

use crate::firmware::PODXT_LIVE_PRODUCT_ID;
use crate::midi::Identity;
use crate::read_session::IDENTITY_REQUEST;

/// Static finding from the Monkey 1.78 i386 image. Addresses are virtual
/// addresses in that Mach-O. The request bytes behind `virtual_slot` were not
/// recovered, so this does not describe a message that can be sent.
pub const PRINT_SITE: PrintSite = PrintSite {
    printer_va: 0x3233a,
    label_instruction_va: 0x3295b,
    displayed_field_offset: 0x2c,
    getter_va: 0x678a,
    formatter_va: 0x65aa,
    virtual_slot: 0x28,
    request_bytes_recovered: false,
};

/// Earlier `lsusb` text in `analysis/reports/podxt-live-usb-descriptors.txt`.
/// It was not collected again for this record.
pub const SAVED_DESCRIPTOR: SavedDescriptor = SavedDescriptor {
    source: "analysis/reports/podxt-live-usb-descriptors.txt",
    vendor_id: 0x0e41,
    product_id: 0x4650,
    bcd_usb: "1.00",
    bcd_device: "0.01",
    configuration_count: 1,
    interface_count: 2,
    reenumerated_for_this_record: false,
    identified_as_usb_firmware_version: false,
};

/// Monkey 1.78 `default.cfg` and the Windows Monkey `RELEASES` resource use
/// the same placeholder. Version 0.00 is not an applicable image.
pub const USB_FIRMWARE_PLACEHOLDER: ShippedPlaceholder = ShippedPlaceholder {
    name: "USB Firmware",
    type_name: "usb",
    release_id: 0,
    version: "0.00",
    url_empty: true,
    filename_empty: true,
    release_date: "2010-09-30 0:00:00",
    note: "Updating this component requires a connection to the internet.",
    podxt_live_midi_id: PODXT_LIVE_PRODUCT_ID,
    is_an_image: false,
};

pub const FLASH_COMPONENT_NAME: &str = "PODxt Flash";
pub const USB_COMPONENT_NAME: &str = "PODxt USB Firmware";
pub const SERVER_UPDATE_STRING: &str = "Uses Server USB Firmware updates";
pub const VERSION_LABEL: &str = "USB Firmware Version:";

pub const USB_COMPONENT_QUERY: QueryRecovery = QueryRecovery::NotRecovered;

pub const FUTURE_PLAN: &[PlanStep] = &[
    PlanStep {
        id: "recover-request-offline",
        detail: "The USB Firmware Version label prints a cached string. The cache is filled from driver vtable slot 0x28, and that method body is still unrecovered. Record the request bytes from that method. Do not send any candidate.",
        gate: PlanGate::OfflineNext,
    },
    PlanStep {
        id: "review-before-any-read",
        detail: "Put the recovered request and expected reply in the offline ledger. A live read still needs a separate explicit authorization.",
        gate: PlanGate::RequiresSeparateAuthorization,
    },
    PlanStep {
        id: "one-reviewed-read",
        detail: "Only after that authorization, send the reviewed read once. Do not update, reset, switch banks, or install a driver.",
        gate: PlanGate::RequiresSeparateAuthorization,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintSite {
    pub printer_va: u32,
    pub label_instruction_va: u32,
    pub displayed_field_offset: u8,
    pub getter_va: u32,
    pub formatter_va: u32,
    pub virtual_slot: u8,
    pub request_bytes_recovered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedDescriptor {
    pub source: &'static str,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bcd_usb: &'static str,
    pub bcd_device: &'static str,
    pub configuration_count: u8,
    pub interface_count: u8,
    pub reenumerated_for_this_record: bool,
    pub identified_as_usb_firmware_version: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShippedPlaceholder {
    pub name: &'static str,
    pub type_name: &'static str,
    pub release_id: u32,
    pub version: &'static str,
    pub url_empty: bool,
    pub filename_empty: bool,
    pub release_date: &'static str,
    pub note: &'static str,
    pub podxt_live_midi_id: u32,
    pub is_an_image: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRecovery {
    NotRecovered,
}

impl QueryRecovery {
    pub const fn recovered(self) -> bool {
        match self {
            Self::NotRecovered => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanGate {
    OfflineNext,
    RequiresSeparateAuthorization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub id: &'static str,
    pub detail: &'static str,
    pub gate: PlanGate,
}

/// The identity reply has one product id and one revision. It has no field for
/// the separate USB-firmware component, so this returns no version.
pub fn usb_component_version_from_identity(_identity: &Identity) -> Option<u32> {
    None
}

pub fn validate_record() -> Result<(), String> {
    if USB_COMPONENT_QUERY.recovered() || PRINT_SITE.request_bytes_recovered {
        return Err("USB component query is marked recovered".into());
    }
    if PRINT_SITE.displayed_field_offset != 0x2c
        || PRINT_SITE.virtual_slot != 0x28
        || PRINT_SITE.printer_va == 0
        || PRINT_SITE.getter_va == 0
        || PRINT_SITE.formatter_va == 0
    {
        return Err("print site no longer points at the cached USB version field".into());
    }
    if USB_FIRMWARE_PLACEHOLDER.is_an_image
        || !USB_FIRMWARE_PLACEHOLDER.url_empty
        || !USB_FIRMWARE_PLACEHOLDER.filename_empty
        || USB_FIRMWARE_PLACEHOLDER.version != "0.00"
        || USB_FIRMWARE_PLACEHOLDER.podxt_live_midi_id != PODXT_LIVE_PRODUCT_ID
        || USB_FIRMWARE_PLACEHOLDER.type_name != "usb"
    {
        return Err("USB firmware placeholder is treated as an image".into());
    }
    if SAVED_DESCRIPTOR.reenumerated_for_this_record
        || SAVED_DESCRIPTOR.identified_as_usb_firmware_version
        || SAVED_DESCRIPTOR.bcd_device == "3.01"
        || SAVED_DESCRIPTOR.product_id == 0x000a
    {
        return Err("descriptor observation was promoted to a firmware version".into());
    }
    if IDENTITY_REQUEST != [0xf0, 0x7e, 0x7f, 6, 1, 0xf7] {
        return Err("identity request is no longer the universal inquiry".into());
    }
    let live_steps = FUTURE_PLAN
        .iter()
        .filter(|step| step.gate == PlanGate::RequiresSeparateAuthorization)
        .count();
    if live_steps != 2 || FUTURE_PLAN[0].gate != PlanGate::OfflineNext {
        return Err("future plan authorizes a live query".into());
    }
    if FLASH_COMPONENT_NAME == USB_COMPONENT_NAME {
        return Err("flash and USB component names were collapsed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::Identity;

    fn synthetic(family: u16, model: u16, revision: [u8; 4], version: Option<u32>) -> Identity {
        Identity {
            family,
            model,
            revision,
            version,
        }
    }

    #[test]
    fn synthetic_identity_has_no_usb_component_version() {
        validate_record().expect("usb firmware record");
        let artificial_revision = synthetic(0x0003, 0x000a, [9, 8, 7, 6], Some(0x0908_0000));
        let unrelated = synthetic(0x1111, 0x2222, [0xaa, 0xbb, 0xcc, 0xdd], None);
        assert_eq!(artificial_revision.product_id(), PODXT_LIVE_PRODUCT_ID);
        assert_eq!(artificial_revision.version_string(), "9.08");
        assert_ne!(
            artificial_revision.version_string(),
            USB_FIRMWARE_PLACEHOLDER.version
        );
        assert_ne!(
            artificial_revision.version_string(),
            SAVED_DESCRIPTOR.bcd_device
        );
        assert!(usb_component_version_from_identity(&artificial_revision).is_none());
        assert!(usb_component_version_from_identity(&unrelated).is_none());
        assert!(unrelated.version_string().contains("aa"));
    }

    #[test]
    fn version_query_remains_unrecovered() {
        validate_record().expect("usb firmware record");
        assert_eq!(IDENTITY_REQUEST.len(), 6);
        assert!(FUTURE_PLAN[0].detail.contains("unrecovered"));
        assert!(FUTURE_PLAN.iter().any(|step| {
            step.gate == PlanGate::RequiresSeparateAuthorization && step.id == "one-reviewed-read"
        }));
    }
}

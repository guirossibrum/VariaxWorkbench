#![forbid(unsafe_code)]
//! PODxt Live USB-firmware component, kept separate from main flash.
//!
//! Partial offline record. The version-query shape was recovered from the
//! macOS Monkey 1.78 image as a USB vendor control transfer. The fields are
//! plain data in this crate. Nothing here links IOKit, Monkey, or a driver,
//! and nothing opens a device, sends the transfer, or installs firmware.
//! The USB-firmware image and the installed version are still unknown.

use crate::firmware::PODXT_LIVE_PRODUCT_ID;
use crate::midi::Identity;
use crate::read_session::IDENTITY_REQUEST;

/// Static finding from the Monkey 1.78 i386 image. Addresses are virtual
/// addresses in that Mach-O. `request_shape_recovered` means the transfer
/// fields are in [`USB_VERSION_CONTROL`]. It does not make them sendable.
pub const PRINT_SITE: PrintSite = PrintSite {
    printer_va: 0x3233a,
    label_instruction_va: 0x3295b,
    displayed_field_offset: 0x2c,
    getter_va: 0x678a,
    formatter_va: 0x65aa,
    virtual_slot: 0x28,
    request_shape_recovered: true,
};

/// How virtual slot `0x28` reaches the control transfer in that image.
/// The base stub returns a failure code and does not build the request.
pub const SLOT_FORWARD: SlotForward = SlotForward {
    brandi_impl_slot_va: 0x145466,
    base_get_version_va: 0x9bcd0,
    mac_get_version_va: 0x8c150,
    request_builder_va: 0x8c040,
    interface_function_offset: 0x68,
    interface_type: 0x100,
};

/// Vendor IN control transfer recovered at [`SlotForward::request_builder_va`].
/// Device-to-host, vendor type, device recipient. These fields are not a URB,
/// and this crate has no function that submits them.
pub const USB_VERSION_CONTROL: StaticControlFields = StaticControlFields {
    bm_request_type: 0xc0,
    b_request: 0x67,
    w_value: 0x0011,
    w_index: 0,
    w_length: 3,
};

/// No transfer was sent, and no device reply was captured.
pub const DEVICE_REPLY_OBSERVED: bool = false;

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

pub const USB_COMPONENT_QUERY: QueryRecovery = QueryRecovery::StaticControlTransfer;

pub const FUTURE_PLAN: &[PlanStep] = &[
    PlanStep {
        id: "static-request-recorded",
        detail: "macOS Monkey 1.78 fills the USB Firmware Version cache from a vendor IN control transfer (bmRequestType 0xC0, bRequest 0x67, wValue 0x0011, wIndex 0, wLength 3). That shape is recorded and must not be sent. No USB-firmware image is in the local inventory, and the installed version is unknown.",
        gate: PlanGate::OfflineNext,
    },
    PlanStep {
        id: "review-before-any-read",
        detail: "The request shape is already in the offline record. No device reply has been captured. A live read still needs a separate explicit authorization.",
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
    pub request_shape_recovered: bool,
}

/// Addresses in the macOS Monkey 1.78 image. `interface_function_offset`
/// matches `DeviceRequest` in the public `IOUSBDeviceStruct182` layout when
/// the interface pointer begins with the usual reserved word. This crate does
/// not link that header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotForward {
    pub brandi_impl_slot_va: u32,
    pub base_get_version_va: u32,
    pub mac_get_version_va: u32,
    pub request_builder_va: u32,
    pub interface_function_offset: u8,
    pub interface_type: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticControlFields {
    pub bm_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}

/// Split of the three-byte buffer named by [`USB_VERSION_CONTROL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticReplyLayout {
    pub version_word: u16,
    pub trailing_byte: u8,
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
    /// Transfer fields recovered by static analysis. Not a transmit path.
    StaticControlTransfer,
}

impl QueryRecovery {
    pub const fn shape_recovered(self) -> bool {
        match self {
            Self::StaticControlTransfer => true,
        }
    }

    /// A recovered shape stays here. Adding a variant must keep this false
    /// until a separately authorized sender exists outside this record.
    pub const fn sendable(self) -> bool {
        match self {
            Self::StaticControlTransfer => false,
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

/// Splits the three-byte buffer named by the static request.
///
/// The first two bytes are the little-endian word the version formatter
/// receives. The third byte is stored separately. No device produced a
/// buffer, so the result is not an installed version.
pub const fn static_reply_layout(data: [u8; 3]) -> StaticReplyLayout {
    StaticReplyLayout {
        version_word: u16::from_le_bytes([data[0], data[1]]),
        trailing_byte: data[2],
    }
}

pub fn validate_record() -> Result<(), String> {
    if !USB_COMPONENT_QUERY.shape_recovered() || USB_COMPONENT_QUERY.sendable() {
        return Err("USB component query is sendable or its shape is missing".into());
    }
    if DEVICE_REPLY_OBSERVED {
        return Err("a device reply was marked observed".into());
    }
    if !PRINT_SITE.request_shape_recovered
        || PRINT_SITE.displayed_field_offset != 0x2c
        || PRINT_SITE.virtual_slot != 0x28
        || PRINT_SITE.printer_va == 0
        || PRINT_SITE.getter_va == 0
        || PRINT_SITE.formatter_va == 0
    {
        return Err("print site no longer points at the cached USB version field".into());
    }
    if USB_VERSION_CONTROL
        != (StaticControlFields {
            bm_request_type: 0xc0,
            b_request: 0x67,
            w_value: 0x0011,
            w_index: 0,
            w_length: 3,
        })
    {
        return Err("recovered control fields changed".into());
    }
    if SLOT_FORWARD.brandi_impl_slot_va != 0x145466
        || SLOT_FORWARD.base_get_version_va != 0x9bcd0
        || SLOT_FORWARD.mac_get_version_va != 0x8c150
        || SLOT_FORWARD.request_builder_va != 0x8c040
        || SLOT_FORWARD.interface_function_offset != 0x68
        || SLOT_FORWARD.interface_type != 0x100
    {
        return Err("slot forward no longer matches the static trace".into());
    }
    let sample = static_reply_layout([0x34, 0x12, 0xab]);
    if sample.version_word != 0x1234 || sample.trailing_byte != 0xab {
        return Err("static reply layout no longer matches the little-endian word".into());
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
        || SAVED_DESCRIPTOR.vendor_id != 0x0e41
        || SAVED_DESCRIPTOR.product_id != 0x4650
    {
        return Err("descriptor observation was promoted to a firmware version".into());
    }
    if IDENTITY_REQUEST != [0xf0, 0x7e, 0x7f, 6, 1, 0xf7] {
        return Err("identity request is no longer the universal inquiry".into());
    }
    if FUTURE_PLAN[0].id != "static-request-recorded"
        || FUTURE_PLAN[0].gate != PlanGate::OfflineNext
        || !FUTURE_PLAN[0].detail.contains("must not be sent")
    {
        return Err("offline plan step no longer withholds the transfer".into());
    }
    let live_steps = FUTURE_PLAN
        .iter()
        .filter(|step| step.gate == PlanGate::RequiresSeparateAuthorization)
        .count();
    if live_steps != 2 {
        return Err("future plan authorizes a live query".into());
    }
    if !FUTURE_PLAN.iter().any(|step| {
        step.gate == PlanGate::RequiresSeparateAuthorization && step.id == "one-reviewed-read"
    }) {
        return Err("reviewed read is no longer behind a separate authorization".into());
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
    fn version_query_shape_is_not_sendable() {
        validate_record().expect("usb firmware record");
        assert_eq!(IDENTITY_REQUEST.len(), 6);
        assert_eq!(USB_VERSION_CONTROL.w_length, 3);
        let layout = static_reply_layout([0x34, 0x12, 0xab]);
        assert_eq!(
            layout,
            StaticReplyLayout {
                version_word: 0x1234,
                trailing_byte: 0xab,
            }
        );
    }
}

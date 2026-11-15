#![forbid(unsafe_code)]
//! PODxt Live USB-firmware component, kept separate from main flash.
//!
//! Partial offline record. macOS Monkey 1.78 contains a userspace service
//! whose version slot builds a USB vendor control transfer. PODxt Live's
//! USB product id selects `L6USBDevice_DevMgr` construction. The version
//! path then reaches an `L6DeviceDescription`, whose slot returns a pointer
//! already cached on a device-control object. The description setter is a
//! virtual slot. No PC-relative call names it, and no Mach-O symbol stub
//! binds it. USB PID `0x4650` constructs an `L6XTAudioControlMac` and, when
//! control slot `+0x18` returns without the failure bit, virtually calls
//! that setter with the control. The index-0 kernel service class is still
//! open, and this record does not treat the transfer as a completed PODxt
//! Live query.
//! The fields are plain data. Nothing here links IOKit, Monkey, or a
//! driver, and nothing opens a device, sends the transfer, or installs
//! firmware. The USB-firmware image and the installed version are unknown.

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

/// Static match that constructs `L6USBDevice_DevMgr` for PODxt Live.
/// Addresses are virtual addresses in the macOS Monkey 1.78 image.
pub const PODXT_LIVE_DEV_MGR_SELECTION: DevMgrSelection = DevMgrSelection {
    vendor_compare_va: 0x1425d6,
    vendor_id: 0x0e41,
    product_compare_va: 0x142805,
    usb_product_id: 0x4650,
    expected_product_id: PODXT_LIVE_PRODUCT_ID,
    constructor_call_va: 0x1429a4,
    constructor_va: 0x148790,
};

/// Userspace and kernel service vtables. `returned_class_identified` stays
/// false until the object stored for the version call is one of these.
/// [`DESCRIPTION_SERVICE_HOP`] is the call that would have to return one of
/// them, and that call does not construct either service.
pub const SERVICE_CLASS_LINK: ServiceClassLink = ServiceClassLink {
    mac_constructor_va: 0x9bb70,
    mac_vptr: 0x1c1878,
    mac_version_slot_va: 0x8c150,
    kernel_vptr: 0x1c1838,
    kernel_version_slot_va: 0x8b810,
    returned_class_identified: false,
};

/// `L6DeviceMgrBase::GetDeviceDesc` returns an `L6DeviceDescription`.
/// The version slot on that object returns a cached device-control pointer.
/// `service_constructed_on_path` stays false: neither Brandi constructor runs
/// in this hop, and the cached class is not identified.
pub const DESCRIPTION_SERVICE_HOP: DescriptionServiceHop = DescriptionServiceHop {
    get_device_desc_va: 0x142b72,
    matcher_call_va: 0x142615,
    constructor_va: 0x85310,
    vptr: 0x1c1598,
    vendor_product_dword_offset: 0x104,
    usb_product_offset: 0x106,
    midi_product_offset: 0x208,
    control_pointer_offset: 0x21c,
    active_control_offset: 0x22c,
    version_slot_va: 0x85800,
    control_getter_va: 0x85150,
    control_service_index: 0,
    cached_service_offset: 0x8,
    service_constructed_on_path: false,
};

/// Virtual setter that writes the description's device-control smart pointer.
/// The function address appears only in this vtable slot. `direct_call_found`
/// stays false: no PC-relative `call` or `jmp` names `setter_va`.
/// `traced_virtual_call_found` stays false for the earlier `GetDeviceDesc`
/// and finder producers. Those producers still do not call the slot. The
/// Monkey image has no symbol-table entry for the setter, so no
/// indirect-symbol or lazy stub binds `setter_va`. The USB PID `0x4650`
/// virtual call is [`PODXT_LIVE_SETTER_ASSOCIATION`].
pub const DESCRIPTION_SETTER: DescriptionSetter = DescriptionSetter {
    vptr: 0x1c1598,
    slot_offset: 0x0c,
    setter_va: 0x85650,
    smart_pointer_offset: 0x21c,
    direct_call_found: false,
    traced_virtual_call_found: false,
};

/// `L6XTAudioControlMac` slot `+0x0c` stores a kernel Brandi service at
/// cache index 0 when that slot's refcount is zero. That producer is useful
/// evidence. `selected_for_podxt_live` stays false: the PID `0x4650` path
/// passes this control to the description setter, and it does not show this
/// kernel fill running. The kernel version slot does not embed the userspace
/// builder fields.
pub const XT_AUDIO_INDEX0_FILL: XtAudioIndex0Fill = XtAudioIndex0Fill {
    control_vptr: 0x1c18b8,
    cache_slot_va: 0x8dd20,
    service_index: 0,
    allocation_size: 0x10,
    kernel_thunk_va: 0x8ad80,
    kernel_constructor_va: 0x8adc0,
    stored_vptr: 0x1c1838,
    userspace_builder_embedded: false,
    selected_for_podxt_live: false,
};

/// Monkey-image association for USB PID `0x4650`. The equality allocates
/// [`PodxtLiveSetterAssociation::control_allocation_size`] bytes and calls
/// the XT-audio constructor, which stores
/// [`PodxtLiveSetterAssociation::control_vptr`]. When control slot `+0x18`
/// returns with bit `0x80000000` clear, the helper calls the description
/// constructor and the caller invokes setter slot `+0x0c` with that control.
/// `kernel_fill_shown` stays false. Nothing here builds or submits a transfer.
pub const PODXT_LIVE_SETTER_ASSOCIATION: PodxtLiveSetterAssociation = PodxtLiveSetterAssociation {
    product_compare_va: 0x953a6,
    usb_product_id: 0x4650,
    control_allocation_size: 0x418,
    constructor_call_va: 0x9544a,
    constructor_va: 0x8c700,
    constructor_body_va: 0x8c740,
    control_vptr: 0x1c18b8,
    control_init_slot: 0x18,
    init_call_va: 0x9555d,
    helper_call_va: 0x955f1,
    helper_va: 0x95e10,
    description_allocation_size: 0x26c,
    description_constructor_va: 0x852f0,
    description_constructor_body_va: 0x85310,
    virtual_call_va: 0x95616,
    setter_slot_offset: 0x0c,
    setter_va: 0x85650,
    stub_binding_found: false,
    direct_call_found: false,
    kernel_fill_shown: false,
    approved_live_read: false,
};

/// Symbolicated copies of the same PID association. Each image keeps its own
/// virtual addresses. Monkey addresses stay on [`PODXT_LIVE_SETTER_ASSOCIATION`].
/// In these images, control slot `+0x0c` is a different method from the Monkey
/// cache filler at `0x8dd20`.
pub const SIBLING_CONTROL_ASSOCIATIONS: &[SiblingControlAssociation] = &[
    SiblingControlAssociation {
        image: "Line 6 License Manager",
        product_compare_va: 0xa6ee6,
        usb_product_id: 0x4650,
        control_allocation_size: 0x418,
        constructor_va: 0x9df70,
        helper_va: 0xa7950,
        virtual_call_va: 0xa7156,
        setter_va: 0x96ee0,
        direct_call_found: false,
        stub_binding_found: false,
    },
    SiblingControlAssociation {
        image: "L6MIDIJava i386",
        product_compare_va: 0x2b0e6,
        usb_product_id: 0x4650,
        control_allocation_size: 0x418,
        constructor_va: 0x22ebc,
        helper_va: 0x2a58a,
        virtual_call_va: 0x2b28a,
        setter_va: 0x1d8da,
        direct_call_found: false,
        stub_binding_found: false,
    },
];

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
        detail: "PODxt Live USB PID 0x4650 matches product 0x0003000a. In the Monkey image that equality constructs an L6XTAudioControlMac and, when control slot +0x18 returns without the failure bit, virtually calls description slot +0x0c with that control. No Mach-O indirect-symbol or lazy stub binds the setter, and no PC-relative call names it. The index-0 kernel service class is not closed, and that fill is not shown on this path. The recorded transfer must not be sent, and its fields are not an approved live read. No USB-firmware image is in the local inventory, and the installed version is unknown.",
        gate: PlanGate::OfflineNext,
    },
    PlanStep {
        id: "review-before-any-read",
        detail: "The recorded fields are not an approved live read. No device reply has been captured. The description setter is recovered. USB PID 0x4650 passes an L6XTAudioControlMac to that setter when control slot +0x18 returns without the failure bit. The index-0 kernel service class remains unidentified. A live read still needs a separate explicit authorization.",
        gate: PlanGate::RequiresSeparateAuthorization,
    },
    PlanStep {
        id: "one-reviewed-read",
        detail: "The recorded control fields are not an approved live read. After the service class is resolved, a separate authorization is still required before any read. Do not update, reset, switch banks, or install a driver.",
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
pub struct DevMgrSelection {
    pub vendor_compare_va: u32,
    pub vendor_id: u16,
    pub product_compare_va: u32,
    pub usb_product_id: u16,
    pub expected_product_id: u32,
    pub constructor_call_va: u32,
    pub constructor_va: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceClassLink {
    pub mac_constructor_va: u32,
    pub mac_vptr: u32,
    pub mac_version_slot_va: u32,
    pub kernel_vptr: u32,
    pub kernel_version_slot_va: u32,
    pub returned_class_identified: bool,
}

/// Addresses in the macOS Monkey 1.78 image. Offsets are bytes from the
/// start of an `L6DeviceDescription` or, for `cached_service_offset`, from
/// the device-control object. Nothing here builds or submits a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptionServiceHop {
    pub get_device_desc_va: u32,
    pub matcher_call_va: u32,
    pub constructor_va: u32,
    pub vptr: u32,
    pub vendor_product_dword_offset: u16,
    pub usb_product_offset: u16,
    pub midi_product_offset: u16,
    pub control_pointer_offset: u16,
    pub active_control_offset: u16,
    pub version_slot_va: u32,
    pub control_getter_va: u32,
    pub control_service_index: u8,
    pub cached_service_offset: u8,
    pub service_constructed_on_path: bool,
}

/// Addresses in the macOS Monkey 1.78 image. `slot_offset` is the byte
/// offset of the setter in the description vtable. Nothing here builds or
/// submits a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptionSetter {
    pub vptr: u32,
    pub slot_offset: u8,
    pub setter_va: u32,
    pub smart_pointer_offset: u16,
    pub direct_call_found: bool,
    pub traced_virtual_call_found: bool,
}

/// Addresses in the macOS Monkey 1.78 image. `allocation_size` is the heap
/// size passed to the kernel Brandi constructor before the pointer is stored
/// at the control's cache slot. Nothing here builds or submits a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XtAudioIndex0Fill {
    pub control_vptr: u32,
    pub cache_slot_va: u32,
    pub service_index: u8,
    pub allocation_size: u8,
    pub kernel_thunk_va: u32,
    pub kernel_constructor_va: u32,
    pub stored_vptr: u32,
    pub userspace_builder_embedded: bool,
    pub selected_for_podxt_live: bool,
}

/// Addresses in the macOS Monkey 1.78 image. `control_init_slot` is the
/// control vtable slot called before the setter. The setter runs on this
/// path when that call returns with bit `0x80000000` clear.
/// `description_constructor_va` is the wrapper whose only direct caller is
/// inside `helper_va`; its body is the description constructor that stores
/// vptr `0x1c1598`. Nothing here builds or submits a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PodxtLiveSetterAssociation {
    pub product_compare_va: u32,
    pub usb_product_id: u16,
    pub control_allocation_size: u16,
    pub constructor_call_va: u32,
    pub constructor_va: u32,
    pub constructor_body_va: u32,
    pub control_vptr: u32,
    pub control_init_slot: u8,
    pub init_call_va: u32,
    pub helper_call_va: u32,
    pub helper_va: u32,
    pub description_allocation_size: u16,
    pub description_constructor_va: u32,
    pub description_constructor_body_va: u32,
    pub virtual_call_va: u32,
    pub setter_slot_offset: u8,
    pub setter_va: u32,
    pub stub_binding_found: bool,
    pub direct_call_found: bool,
    pub kernel_fill_shown: bool,
    pub approved_live_read: bool,
}

/// One symbolicated image that repeats the PID `0x4650` association.
/// `helper_va` is `L6DeviceManagerMac::GetDescription`. `constructor_va` is
/// `L6XTAudioControlMac::C1(unsigned int, unsigned long)`. `setter_va` is
/// `L6DeviceDescription::SetDeviceControl`. Nothing here builds or submits
/// a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiblingControlAssociation {
    pub image: &'static str,
    pub product_compare_va: u32,
    pub usb_product_id: u16,
    pub control_allocation_size: u16,
    pub constructor_va: u32,
    pub helper_va: u32,
    pub virtual_call_va: u32,
    pub setter_va: u32,
    pub direct_call_found: bool,
    pub stub_binding_found: bool,
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
    if PODXT_LIVE_DEV_MGR_SELECTION.vendor_compare_va != 0x1425d6
        || PODXT_LIVE_DEV_MGR_SELECTION.vendor_id != SAVED_DESCRIPTOR.vendor_id
        || PODXT_LIVE_DEV_MGR_SELECTION.product_compare_va != 0x142805
        || PODXT_LIVE_DEV_MGR_SELECTION.usb_product_id != SAVED_DESCRIPTOR.product_id
        || PODXT_LIVE_DEV_MGR_SELECTION.expected_product_id != PODXT_LIVE_PRODUCT_ID
        || PODXT_LIVE_DEV_MGR_SELECTION.constructor_call_va != 0x1429a4
        || PODXT_LIVE_DEV_MGR_SELECTION.constructor_va != 0x148790
    {
        return Err("PODxt Live device-manager selection changed".into());
    }
    if SERVICE_CLASS_LINK.returned_class_identified
        || SERVICE_CLASS_LINK.mac_constructor_va != 0x9bb70
        || SERVICE_CLASS_LINK.mac_vptr != 0x1c1878
        || SERVICE_CLASS_LINK.mac_version_slot_va != SLOT_FORWARD.mac_get_version_va
        || SERVICE_CLASS_LINK.kernel_vptr != 0x1c1838
        || SERVICE_CLASS_LINK.kernel_version_slot_va != 0x8b810
        || SERVICE_CLASS_LINK.kernel_version_slot_va == SERVICE_CLASS_LINK.mac_version_slot_va
    {
        return Err("service class link was closed or the vtables were collapsed".into());
    }
    if DESCRIPTION_SERVICE_HOP.service_constructed_on_path
        || DESCRIPTION_SERVICE_HOP.get_device_desc_va != 0x142b72
        || DESCRIPTION_SERVICE_HOP.matcher_call_va != 0x142615
        || DESCRIPTION_SERVICE_HOP.constructor_va != 0x85310
        || DESCRIPTION_SERVICE_HOP.vptr != 0x1c1598
        || DESCRIPTION_SERVICE_HOP.vendor_product_dword_offset != 0x104
        || DESCRIPTION_SERVICE_HOP.usb_product_offset != 0x106
        || DESCRIPTION_SERVICE_HOP.midi_product_offset != 0x208
        || DESCRIPTION_SERVICE_HOP.control_pointer_offset != 0x21c
        || DESCRIPTION_SERVICE_HOP.active_control_offset != 0x22c
        || DESCRIPTION_SERVICE_HOP.version_slot_va != 0x85800
        || DESCRIPTION_SERVICE_HOP.control_getter_va != 0x85150
        || DESCRIPTION_SERVICE_HOP.control_service_index != 0
        || DESCRIPTION_SERVICE_HOP.cached_service_offset != 0x8
        || DESCRIPTION_SERVICE_HOP.control_getter_va == SERVICE_CLASS_LINK.mac_constructor_va
    {
        return Err("description hop constructs a service or lost its addresses".into());
    }
    if DESCRIPTION_SETTER.direct_call_found
        || DESCRIPTION_SETTER.traced_virtual_call_found
        || DESCRIPTION_SETTER.vptr != DESCRIPTION_SERVICE_HOP.vptr
        || DESCRIPTION_SETTER.slot_offset != 0x0c
        || DESCRIPTION_SETTER.setter_va != 0x85650
        || DESCRIPTION_SETTER.smart_pointer_offset != DESCRIPTION_SERVICE_HOP.control_pointer_offset
    {
        return Err("description setter slot was called or lost its address".into());
    }
    if XT_AUDIO_INDEX0_FILL.selected_for_podxt_live
        || XT_AUDIO_INDEX0_FILL.userspace_builder_embedded
        || XT_AUDIO_INDEX0_FILL.control_vptr != 0x1c18b8
        || XT_AUDIO_INDEX0_FILL.cache_slot_va != 0x8dd20
        || XT_AUDIO_INDEX0_FILL.service_index != 0
        || XT_AUDIO_INDEX0_FILL.allocation_size != 0x10
        || XT_AUDIO_INDEX0_FILL.kernel_thunk_va != 0x8ad80
        || XT_AUDIO_INDEX0_FILL.kernel_constructor_va != 0x8adc0
        || XT_AUDIO_INDEX0_FILL.stored_vptr != SERVICE_CLASS_LINK.kernel_vptr
        || XT_AUDIO_INDEX0_FILL.cache_slot_va == SLOT_FORWARD.request_builder_va
    {
        return Err("index 0 cache fill was selected for PODxt Live or lost its addresses".into());
    }
    if PODXT_LIVE_SETTER_ASSOCIATION.stub_binding_found
        || PODXT_LIVE_SETTER_ASSOCIATION.direct_call_found
        || PODXT_LIVE_SETTER_ASSOCIATION.kernel_fill_shown
        || PODXT_LIVE_SETTER_ASSOCIATION.approved_live_read
        || PODXT_LIVE_SETTER_ASSOCIATION.product_compare_va != 0x953a6
        || PODXT_LIVE_SETTER_ASSOCIATION.usb_product_id != SAVED_DESCRIPTOR.product_id
        || PODXT_LIVE_SETTER_ASSOCIATION.control_allocation_size != 0x418
        || PODXT_LIVE_SETTER_ASSOCIATION.constructor_call_va != 0x9544a
        || PODXT_LIVE_SETTER_ASSOCIATION.constructor_va != 0x8c700
        || PODXT_LIVE_SETTER_ASSOCIATION.constructor_body_va != 0x8c740
        || PODXT_LIVE_SETTER_ASSOCIATION.control_vptr != XT_AUDIO_INDEX0_FILL.control_vptr
        || PODXT_LIVE_SETTER_ASSOCIATION.control_init_slot != 0x18
        || PODXT_LIVE_SETTER_ASSOCIATION.init_call_va != 0x9555d
        || PODXT_LIVE_SETTER_ASSOCIATION.helper_call_va != 0x955f1
        || PODXT_LIVE_SETTER_ASSOCIATION.helper_va != 0x95e10
        || PODXT_LIVE_SETTER_ASSOCIATION.description_allocation_size != 0x26c
        || PODXT_LIVE_SETTER_ASSOCIATION.description_constructor_va != 0x852f0
        || PODXT_LIVE_SETTER_ASSOCIATION.description_constructor_body_va
            != DESCRIPTION_SERVICE_HOP.constructor_va
        || PODXT_LIVE_SETTER_ASSOCIATION.virtual_call_va != 0x95616
        || PODXT_LIVE_SETTER_ASSOCIATION.setter_slot_offset != DESCRIPTION_SETTER.slot_offset
        || PODXT_LIVE_SETTER_ASSOCIATION.setter_va != DESCRIPTION_SETTER.setter_va
        || PODXT_LIVE_SETTER_ASSOCIATION.virtual_call_va == PODXT_LIVE_SETTER_ASSOCIATION.setter_va
    {
        return Err(
            "PODxt Live setter association closed the kernel fill or lost its addresses".into(),
        );
    }
    if SIBLING_CONTROL_ASSOCIATIONS.len() != 2 {
        return Err("sibling control associations changed".into());
    }
    let license = &SIBLING_CONTROL_ASSOCIATIONS[0];
    let midi_java = &SIBLING_CONTROL_ASSOCIATIONS[1];
    if license.image != "Line 6 License Manager"
        || license.product_compare_va != 0xa6ee6
        || license.constructor_va != 0x9df70
        || license.helper_va != 0xa7950
        || license.virtual_call_va != 0xa7156
        || license.setter_va != 0x96ee0
        || midi_java.image != "L6MIDIJava i386"
        || midi_java.product_compare_va != 0x2b0e6
        || midi_java.constructor_va != 0x22ebc
        || midi_java.helper_va != 0x2a58a
        || midi_java.virtual_call_va != 0x2b28a
        || midi_java.setter_va != 0x1d8da
    {
        return Err("sibling control association addresses changed".into());
    }
    for site in SIBLING_CONTROL_ASSOCIATIONS {
        if site.direct_call_found
            || site.stub_binding_found
            || site.usb_product_id != SAVED_DESCRIPTOR.product_id
            || site.control_allocation_size != PODXT_LIVE_SETTER_ASSOCIATION.control_allocation_size
            || site.setter_va == DESCRIPTION_SETTER.setter_va
            || site.virtual_call_va == site.setter_va
            || site.product_compare_va == PODXT_LIVE_SETTER_ASSOCIATION.product_compare_va
        {
            return Err("a sibling image was collapsed into the Monkey association".into());
        }
    }
    if !FUTURE_PLAN[0].detail.contains("not closed") {
        return Err("plan no longer records the open service class".into());
    }
    if !FUTURE_PLAN
        .iter()
        .any(|step| step.detail.contains("not an approved"))
    {
        return Err("plan treats the recorded fields as an approved live read".into());
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

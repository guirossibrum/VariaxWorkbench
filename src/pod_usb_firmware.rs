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

/// Userspace and kernel service vtables. `returned_class_identified` is true
/// for the successful index-0 cache miss on [`VERSION_PATH_SERVICE`]: that
/// return is the kernel object. The userspace vptr is a different class.
/// The flag is not an observed device state.
pub const SERVICE_CLASS_LINK: ServiceClassLink = ServiceClassLink {
    mac_constructor_va: 0x9bb70,
    mac_vptr: 0x1c1878,
    mac_version_slot_va: 0x8c150,
    kernel_vptr: 0x1c1838,
    kernel_version_slot_va: 0x8b810,
    returned_class_identified: true,
};

/// `L6DeviceMgrBase::GetDeviceDesc` returns an `L6DeviceDescription`.
/// Description slot `+0x14` calls the control's slot `+0x0c` with index 0.
/// `service_constructed_on_path` is true because that callee's cache miss
/// runs the kernel constructor. The description body itself does not.
/// `control_getter_va` is the base `AcquireService` body, which returns the
/// cached pointer and does not construct.
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
    service_constructed_on_path: true,
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
/// cache index 0 when that slot's refcount is zero. That producer is the
/// index-0 miss arm. `selected_for_podxt_live` stays false: the PID setter
/// path does not itself run the fill. [`VERSION_PATH_SERVICE`] records the
/// later return of that cache slot. The kernel version slot does not embed
/// the userspace builder fields.
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
/// `kernel_fill_shown` stays false: this setter path does not execute the
/// index-0 fill. [`VERSION_PATH_SERVICE`] is the later cache return.
/// Nothing here builds or submits a transfer.
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
/// License Manager names XT slot `+0x0c` `AcquireService`. That body matches
/// Monkey `0x8dd20`; the License address is [`VERSION_PATH_SERVICE`].
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

/// Index-0 hit and miss on the XT control, and the kernel version handler
/// the returned object exposes. Monkey addresses are the PODxt image.
/// `license_acquire_va` and `license_getversion_va` name the same bodies in
/// Line 6 License Manager and are not Monkey addresses.
///
/// The index-0 miss arm stores the kernel service. It does not return that
/// pointer. `base_acquire_va` reads the cache slot after the support bit is
/// set and returns it when the pointer is non-null. A hit returns the
/// resident pointer and does not construct again.
/// [`FIND_DRIVER_STORE`] is the registry entry that miss reads.
/// [`SHAREDXT_SELECTOR`] is the kernel handler for selector 6. Nothing here
/// builds or submits a transfer, and `approved_live_read` stays false.
pub const VERSION_PATH_SERVICE: VersionPathService = VersionPathService {
    acquire_va: 0x8dd20,
    license_acquire_va: 0x9f590,
    service_index: 0,
    refcount_offset: 0x38,
    cache_object_offset: 0x8,
    hit_branch_va: 0x8e67b,
    index0_arm_va: 0x8ddd0,
    service_name: "com_line6_driver_sharedxt",
    find_driver_va: 0x8e720,
    find_driver_result_consumed: false,
    find_service_va: 0x8e980,
    allocation_size: 0x10,
    operator_new_stub_va: 0x169162,
    kernel_thunk_va: 0x8ad80,
    release_stub_va: 0x167a3a,
    base_acquire_va: 0x85150,
    null_cache_status: 0x8000_7301,
    support_va: 0x8cf10,
    support_index0_arm_va: 0x8cf5e,
    index0_supported: true,
    index0_support_checks_product: false,
    miss_stores_kernel_service: true,
    return_reads_cache_slot: true,
    successful_miss_returns_kernel: true,
    hit_reconstructs: false,
    kernel_getversion_va: 0x8b810,
    license_getversion_va: 0x9d080,
    getversion_and_status_va: 0x8b750,
    semaphore_helper_va: 0x8b360,
    semaphore_timeout: 0x5dc,
    kio_getversion_status_va: 0x88350,
    iokit_stub_va: 0x1679b0,
    iokit_selector: 6,
    scalar_output_count: 2,
    sem_trywait_stub_va: 0x169366,
    error_stub_va: 0x1691bc,
    usleep_stub_va: 0x16941a,
    sem_post_stub_va: 0x169360,
    empty_connection_status: 0x8000_7000,
    userspace_fields_used: false,
    observed_hardware_state: false,
    approved_live_read: false,
};

/// Registry store inside `FindDriverService`. The AcquireService miss calls
/// it and then ignores the return register. A UniqueID match stores the
/// `com_line6_audio` entry at `driver_offset`. `FindServiceNamed` reads that
/// word and returns the child whose registry name is
/// [`VersionPathService::service_name`]. The constructor first stores its
/// integer argument at the same offset and zeroes the UniqueID byte.
/// License addresses name the bodies. They are not Monkey addresses.
/// Nothing here opens a registry entry.
pub const FIND_DRIVER_STORE: FindDriverServiceStore = FindDriverServiceStore {
    find_driver_va: 0x8e720,
    license_va: 0x9ff90,
    monkey_call_va: 0x8dd9d,
    license_call_va: 0x9f60d,
    driver_offset: 0x278,
    unique_id_offset: 0x16b,
    name_buffer_offset: 0x298,
    matching_name: "com_line6_audio",
    registry_plane: "IOService",
    property_key: "UniqueID",
    search_options: 3,
    child_iterator_options: 1,
    constructor_body_va: 0x8c740,
    constructor_store_va: 0x8c799,
    license_constructor_store_va: 0x9e009,
    license_constructor_call_va: 0xa6f8a,
    find_service_load_va: 0x8e9bd,
    result_consumed: false,
    stores_matching_entry: true,
    releases_previous_entry: false,
    observed_hardware_state: false,
};

/// Selector 6 on `com_line6_driver_sharedxt_UserClient` in Line6Audio.
/// The dispatch entry allows no scalar or structure input and two scalar
/// outputs. `externalMethod` loads the service from the user client at
/// `user_client_service_offset` and supplies that object as the target.
/// The action loads the target vtable and tail-calls `slot_offset`.
/// The qword at `slot_qword_va` is `VersionStatusCommand`. `direct_call`
/// stays false. `VersionStatusCommand` fills an `IOUSBDevRequest` with
/// [`USB_VERSION_CONTROL`], and `DeviceRequest` forwards that pointer.
/// Selector 7 is `GetIOPVersion` and is a different method. The unsigned
/// kext has the same bytes at these addresses, including the vtable slot.
/// This is not an observed device reply, and `approved_live_read` stays false.
pub const SHAREDXT_SELECTOR: SharedxtSelectorHandler = SharedxtSelectorHandler {
    external_method_va: 0x272e,
    service_load_va: 0x275f,
    user_client_service_offset: 0xd8,
    method_table_va: 0xa2100,
    selector: 6,
    entry_stride: 24,
    scalar_input_count: 0,
    structure_input_size: 0,
    scalar_output_count: 2,
    structure_output_size: 0,
    action_va: 0x2818,
    vtable_load_va: 0x281c,
    slot_load_va: 0x281f,
    tail_call_va: 0x282f,
    slot_offset: 0x890,
    vtable_symbol_va: 0xa8ec0,
    address_point_delta: 0x10,
    slot_qword_va: 0xa9760,
    direct_call: false,
    command_va: 0x13892,
    interface_offset: 0xa8,
    missing_interface_status: 0xe000_02cd,
    device_request_va: 0x1412e,
    request_pointer_move_va: 0x141a6,
    request_forward_call_va: 0x141b2,
    request_forward_slot: 0x978,
    request_timeout: 0x3e8,
    completion_is_null: true,
    request_rewritten_before_forward: false,
    iop_selector: 7,
    iop_command_va: 0x1370e,
    matches_recorded_fields: true,
    identifies_firmware_image: false,
    same_bytes_in_unsigned_image: true,
    observed_hardware_state: false,
    approved_live_read: false,
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

pub const USB_COMPONENT_QUERY: QueryRecovery = QueryRecovery::StaticControlTransfer;

pub const FUTURE_PLAN: &[PlanStep] = &[
    PlanStep {
        id: "static-request-recorded",
        detail: "PODxt Live USB PID 0x4650 matches product 0x0003000a. In the Monkey image that equality constructs an L6XTAudioControlMac and, when control slot +0x18 returns without the failure bit, virtually calls description slot +0x0c with that control. No Mach-O indirect-symbol or lazy stub binds the setter. AcquireService(0) on that control stores an L6KernelBrandiServiceMac on a cache miss, and the base acquire call returns that cache slot. Kernel GetVersion uses an IOConnectCallScalarMethod stub with selector 6. Sharedxt user-client selector 6 builds the same five fields in a device request and forwards that pointer. That handler is not GetIOPVersion and does not identify a firmware image. The recorded transfer must not be sent, and its fields are not an approved live read. No USB-firmware image is in the local inventory, and the installed version is not closed.",
        gate: PlanGate::OfflineNext,
    },
    PlanStep {
        id: "review-before-any-read",
        detail: "The recorded fields are not an approved live read. No device reply has been captured. A successful AcquireService(0) miss returns the kernel service on the static branch. A live read still needs a separate explicit authorization.",
        gate: PlanGate::RequiresSeparateAuthorization,
    },
    PlanStep {
        id: "one-reviewed-read",
        detail: "The recorded control fields are not an approved live read. The kernel version slot is identified on the static branch. A separate authorization is still required before any read. Do not update, reset, switch banks, or install a driver.",
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

/// Monkey-image record of `AcquireService(0)` on `L6XTAudioControlMac` and
/// the kernel `GetVersion` slot of the object that call returns. License
/// Manager addresses are named separately. Nothing here builds or submits
/// a transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionPathService {
    pub acquire_va: u32,
    pub license_acquire_va: u32,
    pub service_index: u8,
    pub refcount_offset: u8,
    pub cache_object_offset: u8,
    pub hit_branch_va: u32,
    pub index0_arm_va: u32,
    pub service_name: &'static str,
    pub find_driver_va: u32,
    pub find_driver_result_consumed: bool,
    pub find_service_va: u32,
    pub allocation_size: u8,
    pub operator_new_stub_va: u32,
    pub kernel_thunk_va: u32,
    pub release_stub_va: u32,
    pub base_acquire_va: u32,
    pub null_cache_status: u32,
    pub support_va: u32,
    pub support_index0_arm_va: u32,
    pub index0_supported: bool,
    pub index0_support_checks_product: bool,
    pub miss_stores_kernel_service: bool,
    pub return_reads_cache_slot: bool,
    pub successful_miss_returns_kernel: bool,
    pub hit_reconstructs: bool,
    pub kernel_getversion_va: u32,
    pub license_getversion_va: u32,
    pub getversion_and_status_va: u32,
    pub semaphore_helper_va: u32,
    pub semaphore_timeout: u16,
    pub kio_getversion_status_va: u32,
    pub iokit_stub_va: u32,
    pub iokit_selector: u8,
    pub scalar_output_count: u8,
    pub sem_trywait_stub_va: u32,
    pub error_stub_va: u32,
    pub usleep_stub_va: u32,
    pub sem_post_stub_va: u32,
    pub empty_connection_status: u32,
    pub userspace_fields_used: bool,
    pub observed_hardware_state: bool,
    pub approved_live_read: bool,
}

/// `FindDriverService` on the XT control. `monkey_call_va` is the only
/// direct caller in the Monkey image. `license_call_va` is the matching
/// caller in License Manager. `result_consumed` stays false because
/// AcquireService overwrites the return register. `releases_previous_entry`
/// stays false because a match overwrites `driver_offset` without an
/// `IOObjectRelease` of the old word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FindDriverServiceStore {
    pub find_driver_va: u32,
    pub license_va: u32,
    pub monkey_call_va: u32,
    pub license_call_va: u32,
    pub driver_offset: u16,
    pub unique_id_offset: u16,
    pub name_buffer_offset: u16,
    pub matching_name: &'static str,
    pub registry_plane: &'static str,
    pub property_key: &'static str,
    pub search_options: u32,
    pub child_iterator_options: u32,
    pub constructor_body_va: u32,
    pub constructor_store_va: u32,
    pub license_constructor_store_va: u32,
    pub license_constructor_call_va: u32,
    pub find_service_load_va: u32,
    pub result_consumed: bool,
    pub stores_matching_entry: bool,
    pub releases_previous_entry: bool,
    pub observed_hardware_state: bool,
}

/// Kernel dispatch for the userspace selector-6 call. The five request
/// fields stay the recorded [`USB_VERSION_CONTROL`] values.
/// `matches_recorded_fields` means the handler stores those immediates.
/// It does not approve them. `identifies_firmware_image` stays false: the
/// handler names a version word and a status byte, not a firmware file.
/// `service_load_va` belongs to `externalMethod`. `tail_call_va` belongs
/// to the action. `direct_call` stays false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedxtSelectorHandler {
    pub external_method_va: u32,
    pub service_load_va: u32,
    pub user_client_service_offset: u16,
    pub method_table_va: u32,
    pub selector: u8,
    pub entry_stride: u8,
    pub scalar_input_count: u8,
    pub structure_input_size: u32,
    pub scalar_output_count: u8,
    pub structure_output_size: u32,
    pub action_va: u32,
    pub vtable_load_va: u32,
    pub slot_load_va: u32,
    pub tail_call_va: u32,
    pub slot_offset: u16,
    pub vtable_symbol_va: u32,
    pub address_point_delta: u16,
    pub slot_qword_va: u32,
    pub direct_call: bool,
    pub command_va: u32,
    pub interface_offset: u16,
    pub missing_interface_status: u32,
    pub device_request_va: u32,
    pub request_pointer_move_va: u32,
    pub request_forward_call_va: u32,
    pub request_forward_slot: u16,
    pub request_timeout: u16,
    pub completion_is_null: bool,
    pub request_rewritten_before_forward: bool,
    pub iop_selector: u8,
    pub iop_command_va: u32,
    pub matches_recorded_fields: bool,
    pub identifies_firmware_image: bool,
    pub same_bytes_in_unsigned_image: bool,
    pub observed_hardware_state: bool,
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
    if !SERVICE_CLASS_LINK.returned_class_identified
        || SERVICE_CLASS_LINK.mac_constructor_va != 0x9bb70
        || SERVICE_CLASS_LINK.mac_vptr != 0x1c1878
        || SERVICE_CLASS_LINK.mac_version_slot_va != SLOT_FORWARD.mac_get_version_va
        || SERVICE_CLASS_LINK.kernel_vptr != 0x1c1838
        || SERVICE_CLASS_LINK.kernel_version_slot_va != 0x8b810
        || SERVICE_CLASS_LINK.kernel_version_slot_va == SERVICE_CLASS_LINK.mac_version_slot_va
        || SERVICE_CLASS_LINK.kernel_vptr == SERVICE_CLASS_LINK.mac_vptr
    {
        return Err("service class link lost the kernel return or collapsed the vtables".into());
    }
    if !DESCRIPTION_SERVICE_HOP.service_constructed_on_path
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
        return Err("description hop lost the kernel miss or its addresses".into());
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
    if !VERSION_PATH_SERVICE.miss_stores_kernel_service
        || !VERSION_PATH_SERVICE.return_reads_cache_slot
        || !VERSION_PATH_SERVICE.successful_miss_returns_kernel
        || !VERSION_PATH_SERVICE.index0_supported
        || VERSION_PATH_SERVICE.hit_reconstructs
        || VERSION_PATH_SERVICE.find_driver_result_consumed
        || VERSION_PATH_SERVICE.index0_support_checks_product
        || VERSION_PATH_SERVICE.userspace_fields_used
        || VERSION_PATH_SERVICE.observed_hardware_state
        || VERSION_PATH_SERVICE.approved_live_read
        || VERSION_PATH_SERVICE.acquire_va != XT_AUDIO_INDEX0_FILL.cache_slot_va
        || VERSION_PATH_SERVICE.license_acquire_va == VERSION_PATH_SERVICE.acquire_va
        || VERSION_PATH_SERVICE.service_index != XT_AUDIO_INDEX0_FILL.service_index
        || VERSION_PATH_SERVICE.refcount_offset != 0x38
        || VERSION_PATH_SERVICE.cache_object_offset != DESCRIPTION_SERVICE_HOP.cached_service_offset
        || VERSION_PATH_SERVICE.hit_branch_va != 0x8e67b
        || VERSION_PATH_SERVICE.index0_arm_va != 0x8ddd0
        || VERSION_PATH_SERVICE.service_name != "com_line6_driver_sharedxt"
        || VERSION_PATH_SERVICE.find_driver_va != 0x8e720
        || VERSION_PATH_SERVICE.find_service_va != 0x8e980
        || VERSION_PATH_SERVICE.allocation_size != XT_AUDIO_INDEX0_FILL.allocation_size
        || VERSION_PATH_SERVICE.operator_new_stub_va != 0x169162
        || VERSION_PATH_SERVICE.kernel_thunk_va != XT_AUDIO_INDEX0_FILL.kernel_thunk_va
        || VERSION_PATH_SERVICE.release_stub_va != 0x167a3a
        || VERSION_PATH_SERVICE.base_acquire_va != DESCRIPTION_SERVICE_HOP.control_getter_va
        || VERSION_PATH_SERVICE.null_cache_status != 0x8000_7301
        || VERSION_PATH_SERVICE.support_va != 0x8cf10
        || VERSION_PATH_SERVICE.support_index0_arm_va != 0x8cf5e
        || VERSION_PATH_SERVICE.kernel_getversion_va != SERVICE_CLASS_LINK.kernel_version_slot_va
        || VERSION_PATH_SERVICE.license_getversion_va == VERSION_PATH_SERVICE.kernel_getversion_va
        || VERSION_PATH_SERVICE.license_getversion_va != 0x9d080
        || VERSION_PATH_SERVICE.license_acquire_va != 0x9f590
        || VERSION_PATH_SERVICE.getversion_and_status_va != 0x8b750
        || VERSION_PATH_SERVICE.semaphore_helper_va != 0x8b360
        || VERSION_PATH_SERVICE.semaphore_timeout != 0x5dc
        || VERSION_PATH_SERVICE.kio_getversion_status_va != 0x88350
        || VERSION_PATH_SERVICE.iokit_stub_va != 0x1679b0
        || VERSION_PATH_SERVICE.iokit_selector != 6
        || VERSION_PATH_SERVICE.iokit_selector == USB_VERSION_CONTROL.b_request
        || VERSION_PATH_SERVICE.scalar_output_count != 2
        || VERSION_PATH_SERVICE.sem_trywait_stub_va != 0x169366
        || VERSION_PATH_SERVICE.error_stub_va != 0x1691bc
        || VERSION_PATH_SERVICE.usleep_stub_va != 0x16941a
        || VERSION_PATH_SERVICE.sem_post_stub_va != 0x169360
        || VERSION_PATH_SERVICE.empty_connection_status != 0x8000_7000
        || VERSION_PATH_SERVICE.kernel_getversion_va == SLOT_FORWARD.request_builder_va
    {
        return Err("version path collapsed the kernel return into the userspace builder".into());
    }
    if !FIND_DRIVER_STORE.stores_matching_entry
        || FIND_DRIVER_STORE.result_consumed
        || FIND_DRIVER_STORE.releases_previous_entry
        || FIND_DRIVER_STORE.observed_hardware_state
        || FIND_DRIVER_STORE.find_driver_va != VERSION_PATH_SERVICE.find_driver_va
        || FIND_DRIVER_STORE.license_va == FIND_DRIVER_STORE.find_driver_va
        || FIND_DRIVER_STORE.license_va != 0x9ff90
        || FIND_DRIVER_STORE.monkey_call_va != 0x8dd9d
        || FIND_DRIVER_STORE.license_call_va == FIND_DRIVER_STORE.monkey_call_va
        || FIND_DRIVER_STORE.license_call_va != 0x9f60d
        || FIND_DRIVER_STORE.driver_offset != 0x278
        || FIND_DRIVER_STORE.unique_id_offset != 0x16b
        || FIND_DRIVER_STORE.name_buffer_offset != 0x298
        || FIND_DRIVER_STORE.matching_name == VERSION_PATH_SERVICE.service_name
        || FIND_DRIVER_STORE.matching_name != "com_line6_audio"
        || FIND_DRIVER_STORE.registry_plane != "IOService"
        || FIND_DRIVER_STORE.property_key != "UniqueID"
        || FIND_DRIVER_STORE.search_options != 3
        || FIND_DRIVER_STORE.child_iterator_options != 1
        || FIND_DRIVER_STORE.constructor_body_va
            != PODXT_LIVE_SETTER_ASSOCIATION.constructor_body_va
        || FIND_DRIVER_STORE.constructor_store_va != 0x8c799
        || FIND_DRIVER_STORE.license_constructor_store_va == FIND_DRIVER_STORE.constructor_store_va
        || FIND_DRIVER_STORE.license_constructor_call_va != 0xa6f8a
        || FIND_DRIVER_STORE.find_service_load_va != 0x8e9bd
        || FIND_DRIVER_STORE.result_consumed != VERSION_PATH_SERVICE.find_driver_result_consumed
    {
        return Err("find-driver store lost the registry match or consumed the return".into());
    }
    if !SHAREDXT_SELECTOR.matches_recorded_fields
        || SHAREDXT_SELECTOR.identifies_firmware_image
        || SHAREDXT_SELECTOR.observed_hardware_state
        || SHAREDXT_SELECTOR.approved_live_read
        || SHAREDXT_SELECTOR.request_rewritten_before_forward
        || !SHAREDXT_SELECTOR.completion_is_null
        || !SHAREDXT_SELECTOR.same_bytes_in_unsigned_image
        || SHAREDXT_SELECTOR.selector != VERSION_PATH_SERVICE.iokit_selector
        || SHAREDXT_SELECTOR.selector == USB_VERSION_CONTROL.b_request
        || SHAREDXT_SELECTOR.scalar_input_count != 0
        || SHAREDXT_SELECTOR.scalar_output_count != VERSION_PATH_SERVICE.scalar_output_count
        || SHAREDXT_SELECTOR.structure_input_size != 0
        || SHAREDXT_SELECTOR.structure_output_size != 0
        || SHAREDXT_SELECTOR.entry_stride != 24
        || SHAREDXT_SELECTOR.external_method_va != 0x272e
        || SHAREDXT_SELECTOR.service_load_va != 0x275f
        || SHAREDXT_SELECTOR.service_load_va <= SHAREDXT_SELECTOR.external_method_va
        || SHAREDXT_SELECTOR.service_load_va >= SHAREDXT_SELECTOR.action_va
        || SHAREDXT_SELECTOR.user_client_service_offset != 0xd8
        || SHAREDXT_SELECTOR.user_client_service_offset == SHAREDXT_SELECTOR.interface_offset
        || SHAREDXT_SELECTOR.method_table_va != 0xa2100
        || SHAREDXT_SELECTOR.action_va != 0x2818
        || SHAREDXT_SELECTOR.vtable_load_va != 0x281c
        || SHAREDXT_SELECTOR.slot_load_va != 0x281f
        || SHAREDXT_SELECTOR.tail_call_va != 0x282f
        || SHAREDXT_SELECTOR.vtable_load_va <= SHAREDXT_SELECTOR.action_va
        || SHAREDXT_SELECTOR.slot_load_va <= SHAREDXT_SELECTOR.vtable_load_va
        || SHAREDXT_SELECTOR.tail_call_va <= SHAREDXT_SELECTOR.slot_load_va
        || SHAREDXT_SELECTOR.slot_offset != 0x890
        || SHAREDXT_SELECTOR.vtable_symbol_va != 0xa8ec0
        || SHAREDXT_SELECTOR.address_point_delta != 0x10
        || SHAREDXT_SELECTOR.slot_qword_va != 0xa9760
        || SHAREDXT_SELECTOR.slot_qword_va
            != SHAREDXT_SELECTOR.vtable_symbol_va
                + u32::from(SHAREDXT_SELECTOR.address_point_delta)
                + u32::from(SHAREDXT_SELECTOR.slot_offset)
        || SHAREDXT_SELECTOR.direct_call
        || SHAREDXT_SELECTOR.command_va != 0x13892
        || SHAREDXT_SELECTOR.action_va == SHAREDXT_SELECTOR.command_va
        || SHAREDXT_SELECTOR.tail_call_va == SHAREDXT_SELECTOR.command_va
        || SHAREDXT_SELECTOR.slot_qword_va == SHAREDXT_SELECTOR.command_va
        || SHAREDXT_SELECTOR.interface_offset != 0xa8
        || SHAREDXT_SELECTOR.missing_interface_status != 0xe000_02cd
        || SHAREDXT_SELECTOR.device_request_va != 0x1412e
        || SHAREDXT_SELECTOR.request_pointer_move_va != 0x141a6
        || SHAREDXT_SELECTOR.request_forward_call_va != 0x141b2
        || SHAREDXT_SELECTOR.request_forward_slot != 0x978
        || SHAREDXT_SELECTOR.request_pointer_move_va <= SHAREDXT_SELECTOR.device_request_va
        || SHAREDXT_SELECTOR.request_forward_call_va <= SHAREDXT_SELECTOR.request_pointer_move_va
        || SHAREDXT_SELECTOR.request_timeout != 0x3e8
        || SHAREDXT_SELECTOR.iop_selector == SHAREDXT_SELECTOR.selector
        || SHAREDXT_SELECTOR.iop_selector != 7
        || SHAREDXT_SELECTOR.iop_command_va != 0x1370e
        || SHAREDXT_SELECTOR.iop_command_va == SHAREDXT_SELECTOR.command_va
    {
        return Err(
            "sharedxt selector collapsed into the userspace builder or an IOP version".into(),
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

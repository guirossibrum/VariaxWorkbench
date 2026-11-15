#![forbid(unsafe_code)]
//! Offline record of the installed Linux Line 6 USB split.
//!
//! Module bytes come from kernel `7.2.15-1-omarchy`. The matching
//! `sound/usb/line6/variax.c` is Linux tag `v7.2.15`. Installed headers ship
//! `Kconfig` only inside that directory. A detached rebuild can use the rest
//! of those headers plus recovered `driver.h`, `midi.h`, and `midibuf.h`.
//! Nothing here loads a module, binds a driver, probes a device, or builds a
//! USB transfer.

use crate::midi::Identity;
use crate::midi_enable;
use crate::read_session::IDENTITY_REQUEST;

/// `USB_DEVICE_ID_MATCH_VENDOR` from the installed `include/linux/device-id/usb.h`.
pub const MATCH_VENDOR: u16 = 0x0001;
/// `USB_DEVICE_ID_MATCH_PRODUCT`.
pub const MATCH_PRODUCT: u16 = 0x0002;
/// `USB_DEVICE_ID_MATCH_INT_NUMBER`.
pub const MATCH_INTERFACE_NUMBER: u16 = 0x0400;

pub const VARIAX_KO_ZST_SHA256: &str =
    "198be2ba22408858304d2cbe4b267519516196893ab734848a9e50e5ea3a3c2b";
pub const POD_KO_ZST_SHA256: &str =
    "0f64d61922a6c557430f4f8f0d9290c80698bd7053fc5ee205331c797e124863";
pub const LINE6_KO_ZST_SHA256: &str =
    "3ad0267696dff9d609f949b2c4d744e4493e7493cff8d92cc29f1e38f7b8dc29";
pub const VARIAX_SRCVERSION: &str = "1885465675CE02E68674BB5";
pub const POD_SRCVERSION: &str = "A4A2201EC078E5484AE28F9";
pub const MODULE_VERMAGIC: &str = "7.2.15-1-omarchy SMP preempt mod_unload";

/// Linux stable tag whose `sound/usb/line6/variax.c` matches this module.
pub const UPSTREAM_VARIAX_C_TAG: &str = "v7.2.15";
/// Commit peeled from annotated tag `v7.2.15` (`This is the 7.2.15 stable release`).
pub const UPSTREAM_VARIAX_C_COMMIT: &str = "497791a0a96b866d90add64041c47acdff49cae6";
/// SHA-256 of that `variax.c`. The file is not vendored here.
pub const UPSTREAM_VARIAX_C_SHA256: &str =
    "f623cd628440ee3208f857a97a462799260c068633774e4b3ff2cad9107c62af";
/// `omacom/omarchy-pkgs` commit that builds `linux-omarchy` 7.2.15-1.
pub const PACKAGING_COMMIT: &str = "326470ed1eca0141d687c701a4faf11694c3358d";
/// Patch files in that recipe. None mention `variax` or `sound/usb/line6`.
pub const PACKAGING_PATCHES_SCANNED: u32 = 188;
pub const PACKAGING_PATCHES_TOUCH_VARIAX_C: bool = false;

/// SHA-256 of `driver.h` from [`UPSTREAM_VARIAX_C_COMMIT`]. Not vendored.
pub const UPSTREAM_DRIVER_H_SHA256: &str =
    "6bdbf42547971ee457d67a02b742ef342ffc7ca9a675d5f298ef21f6e1c3f8cf";
/// SHA-256 of `midi.h` from the same commit. `driver.h` includes it.
pub const UPSTREAM_MIDI_H_SHA256: &str =
    "8928524fc89e772aec5cfabf6eb568d33f43cd0216a195bdaf44cd9a9209e060";
/// SHA-256 of `midibuf.h` from the same commit. `midi.h` includes it.
pub const UPSTREAM_MIDIBUF_H_SHA256: &str =
    "c56dfe4fe244119b98dbcae55197d60267533be0d512e4c22d6ec9d613599f80";

/// `.text` size of the installed module and of both detached rebuilds.
pub const VARIAX_TEXT_SIZE: usize = 0x319;
/// SHA-256 of that installed `.text`. The detached unpatched rebuild matched it.
pub const INSTALLED_VARIAX_TEXT_SHA256: &str =
    "3eae27ccbea3a6ca067a86faf4ade9c713702234b369df20e1391fbe4ff9849d";
/// SHA-256 of `.text` from the detached rebuild of unmodified `variax.c`.
pub const DETACHED_UNPATCHED_TEXT_SHA256: &str =
    "3eae27ccbea3a6ca067a86faf4ade9c713702234b369df20e1391fbe4ff9849d";
/// SHA-256 of `.text` after `docs/linux-variax-model-bytes.patch`.
pub const DETACHED_PATCHED_TEXT_SHA256: &str =
    "3d3aa8f53ab2dc4a97bfffa37e73cffea55d4354ab31bbffdb4186b75c058454";
/// `srcversion` of the detached unpatched object. It is not the installed module.
pub const DETACHED_UNPATCHED_SRCVERSION: &str = "D89A1DD411DBFCF34E2F6C2";
/// `srcversion` of the detached patched object.
pub const DETACHED_PATCHED_SRCVERSION: &str = "8D7F8A31C939F5CEAB036CE";
/// The detached `make modules` finished. The objects were not installed.
pub const DETACHED_MODULE_BUILT: bool = true;
pub const DETACHED_MODULE_INSTALLED: bool = false;
/// Immediate in `testb $0xfb, buf[10]`. Only model bytes `0x00` and `0x04` pass.
pub const PATCHED_MODEL_TEST_MASK: u8 = 0xfb;

/// One `usb_device_id` row. `bInterfaceNumber` is byte offset 16.
/// `driver_info` is the aligned `kernel_ulong_t` at offset 24.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbIdMatch {
    pub match_flags: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface_number: u8,
    pub driver_info: u64,
}

impl UsbIdMatch {
    pub const fn matches_interface_number(self) -> bool {
        self.match_flags & MATCH_INTERFACE_NUMBER != 0
    }
}

/// `snd-usb-variax` row 0: USB `0x4650`, interface 1, `driver_info` 0.
pub const VARIAX_PODXT_LIVE: UsbIdMatch = UsbIdMatch {
    match_flags: MATCH_VENDOR | MATCH_PRODUCT | MATCH_INTERFACE_NUMBER,
    vendor_id: 0x0e41,
    product_id: 0x4650,
    interface_number: 1,
    driver_info: 0,
};

/// `snd-usb-variax` row 1: USB `0x534D`, no interface constraint, `driver_info` 1.
pub const VARIAX_WORKBENCH: UsbIdMatch = UsbIdMatch {
    match_flags: MATCH_VENDOR | MATCH_PRODUCT,
    vendor_id: 0x0e41,
    product_id: 0x534d,
    interface_number: 0,
    driver_info: 1,
};

/// `snd-usb-pod` row for the same USB product on interface 0, `driver_info` 5.
pub const POD_PODXT_LIVE: UsbIdMatch = UsbIdMatch {
    match_flags: MATCH_VENDOR | MATCH_PRODUCT | MATCH_INTERFACE_NUMBER,
    vendor_id: 0x0e41,
    product_id: 0x4650,
    interface_number: 0,
    driver_info: 5,
};

/// `line6_properties` for `driver_info` 0. Field order is the installed BTF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariaxProperties {
    pub id: &'static str,
    pub name: &'static str,
    pub capabilities: u32,
    pub altsetting: u32,
    pub ctrl_if: u32,
    pub ep_ctrl_r: u32,
    pub ep_ctrl_w: u32,
    pub ep_audio_r: u32,
    pub ep_audio_w: u32,
}

pub const PODXT_LIVE_VARIAX_PROPERTIES: VariaxProperties = VariaxProperties {
    id: "PODxtLive",
    name: "PODxt Live",
    capabilities: 0x11,
    altsetting: 1,
    ctrl_if: 0,
    ep_ctrl_r: 0x86,
    ep_ctrl_w: 0x05,
    ep_audio_r: 0x82,
    ep_audio_w: 0x01,
};

/// macOS `Line6Audio` personality `PODxtLive` in both audio kext plists.
pub const MAC_AUDIO_VENDOR: u32 = 3649;
pub const MAC_AUDIO_PRODUCT: u32 = 18000;
pub const MAC_AUDIO_CONFIGURATION: u32 = 1;
pub const MAC_AUDIO_INTERFACE: u32 = 0;
/// macOS `Line6VariaxUSB.kext` matches this device id and no interface number.
pub const MAC_VARIAX_USB_PRODUCT: u32 = 21325;

/// Windows `l6podxt.inf` hardware id for PODxt Live. It has no interface number.
pub const WINDOWS_PODXT_LIVE_HARDWARE_ID: &str = "USB\\VID_0E41&PID_4650";
pub const WINDOWS_DRIVER_VERSION: &str = "03/15/2019,4.2.7.7";

/// Eight bytes at identity offset 1. The immediate is `movabs $0x070c010002067f7e`.
pub const IDENTITY_PREFIX: [u8; 8] = [0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x07];
/// Four bytes at identity offset 8. The immediate is `cmp dword [msg+8], 7`.
pub const IDENTITY_FAMILY_MODEL: [u8; 4] = [0x07, 0x00, 0x00, 0x00];

/// `startup_progress` in installed BTF. Byte 8488 is `0x2128`.
pub const STARTUP_STATE_OFFSET: u32 = 0x2128;
pub const STARTUP_PROGRESS_BTF_OFFSET: u32 = 8488;
/// `VARIAX_STARTUP_VERSIONREQ` in the recovered `variax.c` enum.
pub const STARTUP_VERSION_REQUEST: i32 = 0;
/// `VARIAX_STARTUP_ACTIVATE`. The identity arm assigns this value.
pub const STARTUP_ACTIVATE: i32 = 1;
/// `VARIAX_STARTUP_SETUP`. The init-done arm schedules while progress is below this.
pub const STARTUP_SETUP: i32 = 2;

/// `variax_activate`, also the buffer copied with length 9 during init.
pub const VARIAX_ACTIVATE: [u8; 9] = [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, 0x01, 0xf7];
/// Bytes required by the separate init-done compare. This is not a send buffer.
pub const INIT_DONE_PREFIX: [u8; 7] = [0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x6b];

/// Derived product id of the saved Variax 300/600 reply. The frame stays local.
pub const SAVED_VARIAX_PRODUCT_ID: u32 = 0x0007_0004;
/// Derived revision of that reply, displayed 3.10. The frame stays local.
pub const SAVED_VARIAX_VERSION: u32 = 0x0310_0000;

/// ASCII `0001`. Comparator packets use this revision, not the saved one.
pub const SYNTHETIC_REVISION: [u8; 4] = *b"0001";
/// `Identity` value of [`SYNTHETIC_REVISION`].
pub const SYNTHETIC_COMPARATOR_VERSION: u32 = 0x0001_0000;

/// Model `0x0004` and [`SYNTHETIC_REVISION`]. This packet was not captured.
pub const SYNTHETIC_MODEL_FOUR_IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 7, 0, 4, 0, b'0', b'0', b'0', b'1', 0xf7,
];

/// Member `0x0000` and the same synthetic revision. This packet was not captured.
pub const SYNTHETIC_MEMBER_ZERO_IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 7, 0, 0, 0, b'0', b'0', b'0', b'1', 0xf7,
];

/// Model `0x0005` and the same synthetic revision. The offline patch rejects it.
pub const SYNTHETIC_OTHER_MODEL_IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 7, 0, 5, 0, b'0', b'0', b'0', b'1', 0xf7,
];

pub const SOURCE_FILE_ARCHIVED: bool = false;
pub const DRIVER_PATCH_INSTALLED: bool = false;
pub const LIVE_PROBE_PERFORMED: bool = false;
pub const IN_REPO_TRANSPORT_CHANGED: bool = false;

/// Byte compare for `variax_init_version`. This does not read `startup_progress`.
pub fn matches_startup_identity(packet: &[u8]) -> bool {
    packet.len() >= 12
        && packet[0] == 0xf0
        && packet[1..9] == IDENTITY_PREFIX
        && packet[8..12] == IDENTITY_FAMILY_MODEL
}

/// Progress after the identity arm.
///
/// The source assigns `VARIAX_STARTUP_ACTIVATE` only when the bytes match and
/// `startup_progress >= VARIAX_STARTUP_ACTIVATE` is false. The installed
/// object code is `testl` / `jg` on the dword at [`STARTUP_STATE_OFFSET`],
/// then a store of `1`. Those agree while the threshold is 1: a positive
/// value skips the store.
pub fn startup_state_after_identity(packet: &[u8], current_state: i32) -> i32 {
    if matches_startup_identity(packet) && current_state < STARTUP_ACTIVATE {
        STARTUP_ACTIVATE
    } else {
        current_state
    }
}

/// Object encoding of `buf[10] == 0x00 || buf[10] == 0x04`.
///
/// The patched rebuild keeps the family `cmpq`, then checks this byte with
/// `testb` and [`PATCHED_MODEL_TEST_MASK`]. A following `cmpb` still requires
/// the model high byte and the reserved byte to be zero. Those are separate
/// from this mask.
pub fn patched_object_accepts_model_byte(byte: u8) -> bool {
    byte & PATCHED_MODEL_TEST_MASK == 0
}

/// Offline compare from `docs/linux-variax-model-bytes.patch`.
///
/// Family `0x0007`, then model `0x0000` or `0x0004`. The progress gate is the
/// same one used by [`startup_state_after_identity`].
pub fn matches_patched_startup_identity(packet: &[u8]) -> bool {
    packet.len() >= 12
        && packet[0] == 0xf0
        && packet[1..9] == IDENTITY_PREFIX
        && packet[9] == 0x00
        && packet[11] == 0x00
        && (packet[10] == 0x00 || packet[10] == 0x04)
}

/// [`startup_state_after_identity`] using the offline model compare.
pub fn startup_state_after_patched_identity(packet: &[u8], current_state: i32) -> i32 {
    if matches_patched_startup_identity(packet) && current_state < STARTUP_ACTIVATE {
        STARTUP_ACTIVATE
    } else {
        current_state
    }
}

/// True when the message matches the init-done immediates.
pub fn matches_init_done(packet: &[u8]) -> bool {
    packet.len() >= INIT_DONE_PREFIX.len() && packet.starts_with(&INIT_DONE_PREFIX)
}

/// Init-done schedules work when the bytes match and progress is below `VARIAX_STARTUP_SETUP`.
///
/// The installed code compares the dword at [`STARTUP_STATE_OFFSET`] with `1`
/// and skips when that signed value is greater than 1. This arm does not store
/// startup state 1.
pub fn init_done_schedules_startup(packet: &[u8], current_state: i32) -> bool {
    matches_init_done(packet) && current_state < STARTUP_SETUP
}

pub fn validate() -> Result<(), &'static str> {
    if VARIAX_KO_ZST_SHA256.len() != 64
        || POD_KO_ZST_SHA256.len() != 64
        || LINE6_KO_ZST_SHA256.len() != 64
        || VARIAX_KO_ZST_SHA256 == POD_KO_ZST_SHA256
        || VARIAX_KO_ZST_SHA256 == LINE6_KO_ZST_SHA256
        || VARIAX_SRCVERSION == POD_SRCVERSION
        || VARIAX_SRCVERSION.is_empty()
        || MODULE_VERMAGIC.is_empty()
        || UPSTREAM_VARIAX_C_TAG != "v7.2.15"
        || UPSTREAM_VARIAX_C_COMMIT.len() != 40
        || UPSTREAM_VARIAX_C_SHA256.len() != 64
        || PACKAGING_COMMIT.len() != 40
        || PACKAGING_PATCHES_SCANNED != 188
        || PACKAGING_PATCHES_TOUCH_VARIAX_C
        || UPSTREAM_DRIVER_H_SHA256.len() != 64
        || UPSTREAM_MIDI_H_SHA256.len() != 64
        || UPSTREAM_MIDIBUF_H_SHA256.len() != 64
        || UPSTREAM_DRIVER_H_SHA256 == UPSTREAM_MIDI_H_SHA256
        || UPSTREAM_MIDI_H_SHA256 == UPSTREAM_MIDIBUF_H_SHA256
        || VARIAX_TEXT_SIZE != 0x319
        || INSTALLED_VARIAX_TEXT_SHA256.len() != 64
        || DETACHED_UNPATCHED_TEXT_SHA256 != INSTALLED_VARIAX_TEXT_SHA256
        || DETACHED_PATCHED_TEXT_SHA256.len() != 64
        || DETACHED_PATCHED_TEXT_SHA256 == INSTALLED_VARIAX_TEXT_SHA256
        || DETACHED_UNPATCHED_SRCVERSION == VARIAX_SRCVERSION
        || DETACHED_PATCHED_SRCVERSION == VARIAX_SRCVERSION
        || DETACHED_UNPATCHED_SRCVERSION == DETACHED_PATCHED_SRCVERSION
        || !DETACHED_MODULE_BUILT
        || DETACHED_MODULE_INSTALLED
        || PATCHED_MODEL_TEST_MASK != 0xfb
    {
        return Err("module identity record lost the installed build or the recovered variax.c");
    }
    if (0u8..=255).any(|byte| {
        let mut packet = SYNTHETIC_MEMBER_ZERO_IDENTITY;
        packet[10] = byte;
        patched_object_accepts_model_byte(byte) != (byte == 0x00 || byte == 0x04)
            || matches_patched_startup_identity(&packet) != patched_object_accepts_model_byte(byte)
    }) {
        return Err("patched object mask disagreed with the model-0 and model-4 compare");
    }
    if VARIAX_PODXT_LIVE.vendor_id != 0x0e41
        || VARIAX_PODXT_LIVE.product_id != 0x4650
        || VARIAX_PODXT_LIVE.interface_number != 1
        || !VARIAX_PODXT_LIVE.matches_interface_number()
        || VARIAX_PODXT_LIVE.driver_info != 0
        || POD_PODXT_LIVE.product_id != VARIAX_PODXT_LIVE.product_id
        || POD_PODXT_LIVE.interface_number != 0
        || !POD_PODXT_LIVE.matches_interface_number()
        || POD_PODXT_LIVE.driver_info != 5
        || VARIAX_PODXT_LIVE.interface_number == POD_PODXT_LIVE.interface_number
    {
        return Err("podxt live usb id collapsed the interface split");
    }
    if VARIAX_WORKBENCH.product_id != 0x534d
        || VARIAX_WORKBENCH.matches_interface_number()
        || VARIAX_WORKBENCH.driver_info != 1
        || VARIAX_WORKBENCH.product_id == VARIAX_PODXT_LIVE.product_id
    {
        return Err("workbench usb id was treated as the podxt live interface");
    }
    let row = PODXT_LIVE_VARIAX_PROPERTIES;
    if row.id != "PODxtLive"
        || row.name != "PODxt Live"
        || row.capabilities != 0x11
        || row.altsetting != 1
        || row.ctrl_if != 0
        || row.ctrl_if == u32::from(VARIAX_PODXT_LIVE.interface_number)
        || row.ep_ctrl_r != 0x86
        || row.ep_ctrl_w != 0x05
        || row.ep_audio_r != 0x82
        || row.ep_audio_w != 0x01
    {
        return Err("variax property row lost the interface-1 endpoints or altsetting");
    }
    if MAC_AUDIO_VENDOR != 0x0e41
        || MAC_AUDIO_PRODUCT != 0x4650
        || MAC_AUDIO_CONFIGURATION != 1
        || MAC_AUDIO_INTERFACE != u32::from(POD_PODXT_LIVE.interface_number)
        || MAC_VARIAX_USB_PRODUCT != u32::from(VARIAX_WORKBENCH.product_id)
    {
        return Err("macos personalities no longer match the usb ids");
    }
    if WINDOWS_DRIVER_VERSION != "03/15/2019,4.2.7.7"
        || WINDOWS_PODXT_LIVE_HARDWARE_ID != "USB\\VID_0E41&PID_4650"
        || WINDOWS_PODXT_LIVE_HARDWARE_ID.contains("MI_")
    {
        return Err("windows hardware id gained an interface number");
    }
    if IDENTITY_REQUEST != [0xf0, 0x7e, 0x7f, 6, 1, 0xf7] || midi_enable() != VARIAX_ACTIVATE {
        return Err("version request or activate frame drifted from the existing allowlist");
    }
    if VARIAX_ACTIVATE[7] != 1
        || STARTUP_STATE_OFFSET != 0x2128
        || STARTUP_PROGRESS_BTF_OFFSET != STARTUP_STATE_OFFSET
        || STARTUP_VERSION_REQUEST != 0
        || STARTUP_ACTIVATE != 1
        || STARTUP_SETUP != 2
        || SAVED_VARIAX_PRODUCT_ID != 0x0007_0004
        || SAVED_VARIAX_VERSION != 0x0310_0000
        || SYNTHETIC_COMPARATOR_VERSION == SAVED_VARIAX_VERSION
    {
        return Err("activate parameter, startup gate, or saved identity scalars drifted");
    }
    let model_four = Identity::parse(&SYNTHETIC_MODEL_FOUR_IDENTITY)
        .map_err(|_| "synthetic model-4 identity no longer parses")?;
    if model_four.product_id() != SAVED_VARIAX_PRODUCT_ID
        || model_four.version != Some(SYNTHETIC_COMPARATOR_VERSION)
        || SYNTHETIC_MODEL_FOUR_IDENTITY[12..16] != SYNTHETIC_REVISION
        || matches_startup_identity(&SYNTHETIC_MODEL_FOUR_IDENTITY)
        || matches_init_done(&SYNTHETIC_MODEL_FOUR_IDENTITY)
        || startup_state_after_identity(&SYNTHETIC_MODEL_FOUR_IDENTITY, STARTUP_VERSION_REQUEST)
            != STARTUP_VERSION_REQUEST
        || !matches_patched_startup_identity(&SYNTHETIC_MODEL_FOUR_IDENTITY)
        || startup_state_after_patched_identity(
            &SYNTHETIC_MODEL_FOUR_IDENTITY,
            STARTUP_VERSION_REQUEST,
        ) != STARTUP_ACTIVATE
        || startup_state_after_patched_identity(&SYNTHETIC_MODEL_FOUR_IDENTITY, STARTUP_ACTIVATE)
            != STARTUP_ACTIVATE
        || startup_state_after_patched_identity(&SYNTHETIC_MODEL_FOUR_IDENTITY, STARTUP_SETUP)
            != STARTUP_SETUP
    {
        return Err("synthetic model-4 reply was treated as the installed module identity");
    }
    let member_zero = Identity::parse(&SYNTHETIC_MEMBER_ZERO_IDENTITY)
        .map_err(|_| "synthetic member-zero identity no longer parses")?;
    if member_zero.product_id() != 0x0007_0000
        || member_zero.version != Some(SYNTHETIC_COMPARATOR_VERSION)
        || SYNTHETIC_MEMBER_ZERO_IDENTITY[12..16] != SYNTHETIC_REVISION
        || !matches_startup_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY)
        || !matches_patched_startup_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY)
        || startup_state_after_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY, STARTUP_VERSION_REQUEST)
            != STARTUP_ACTIVATE
        || startup_state_after_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY, -1) != STARTUP_ACTIVATE
        || startup_state_after_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY, STARTUP_ACTIVATE)
            != STARTUP_ACTIVATE
        || startup_state_after_identity(&SYNTHETIC_MEMBER_ZERO_IDENTITY, STARTUP_SETUP)
            != STARTUP_SETUP
    {
        return Err("member-zero compare lost the identity bytes or the progress gate");
    }
    let other = Identity::parse(&SYNTHETIC_OTHER_MODEL_IDENTITY)
        .map_err(|_| "synthetic other-model identity no longer parses")?;
    if other.product_id() == SAVED_VARIAX_PRODUCT_ID
        || other.product_id() == 0x0007_0000
        || SYNTHETIC_OTHER_MODEL_IDENTITY[12..16] != SYNTHETIC_REVISION
        || matches_startup_identity(&SYNTHETIC_OTHER_MODEL_IDENTITY)
        || matches_patched_startup_identity(&SYNTHETIC_OTHER_MODEL_IDENTITY)
    {
        return Err("offline model compare accepted a model outside 0x0000 and 0x0004");
    }
    let pod = [
        0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0x0c, 3, 0, 10, 0, b'0', b'0', b'0', b'1', 0xf7,
    ];
    let pod_id = Identity::parse(&pod).map_err(|_| "pod identity fixture no longer parses")?;
    if pod_id.product_id() != 0x0003_000a
        || matches_startup_identity(&pod)
        || matches_patched_startup_identity(&pod)
        || startup_state_after_identity(&pod, STARTUP_VERSION_REQUEST) != STARTUP_VERSION_REQUEST
    {
        return Err("pod identity was accepted by the variax startup compare");
    }
    if !init_done_schedules_startup(&INIT_DONE_PREFIX, STARTUP_VERSION_REQUEST)
        || !init_done_schedules_startup(&INIT_DONE_PREFIX, STARTUP_ACTIVATE)
        || init_done_schedules_startup(&INIT_DONE_PREFIX, STARTUP_SETUP)
        || init_done_schedules_startup(&SYNTHETIC_MEMBER_ZERO_IDENTITY, STARTUP_VERSION_REQUEST)
    {
        return Err("init-done scheduling lost its own progress gate");
    }
    if SOURCE_FILE_ARCHIVED
        || DRIVER_PATCH_INSTALLED
        || DETACHED_MODULE_INSTALLED
        || LIVE_PROBE_PERFORMED
        || IN_REPO_TRANSPORT_CHANGED
    {
        return Err(
            "linux transport record claimed a vendored source, an installed patch, a probe, or a repository transport change",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_the_interface_split_and_the_model_four_limit() {
        assert_eq!(
            validate(),
            Ok(()),
            "linux transport record collapsed the interface split or treated model 4 as the module identity"
        );
    }
}

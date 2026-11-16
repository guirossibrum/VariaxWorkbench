#![forbid(unsafe_code)]
//! Static description of the initial rig.
//!
//! The USB ids are the installed kernel match rows. Storing them does not
//! enumerate devices or bind a driver.

use crate::linux_transport::{POD_PODXT_LIVE, VARIAX_PODXT_LIVE, VARIAX_WORKBENCH};

use super::model::{ProductProfile, RoleKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalRole {
    pub kind: RoleKind,
    pub profile: ProductProfile,
    /// Matched `bInterfaceNumber` from the kernel id row. This is not `ctrl_if`.
    pub interface_number: u8,
    pub driver_info: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalHost {
    pub vendor_id: u16,
    pub product_id: u16,
    pub evidence: &'static str,
    pub roles: &'static [LogicalRole],
}

/// One PODxt Live USB device. The guitar and the POD are logical roles on it.
pub const INITIAL_HOST: PhysicalHost = PhysicalHost {
    vendor_id: VARIAX_PODXT_LIVE.vendor_id,
    product_id: VARIAX_PODXT_LIVE.product_id,
    evidence: "snd-usb-variax matches this device on interface 1. snd-usb-pod matches the same vendor and product on interface 0. The rows are two roles, not two physical devices.",
    roles: &[
        LogicalRole {
            kind: RoleKind::Guitar,
            profile: ProductProfile::Variax600,
            interface_number: VARIAX_PODXT_LIVE.interface_number,
            driver_info: VARIAX_PODXT_LIVE.driver_info,
        },
        LogicalRole {
            kind: RoleKind::Pod,
            profile: ProductProfile::PodxtLive,
            interface_number: POD_PODXT_LIVE.interface_number,
            driver_info: POD_PODXT_LIVE.driver_info,
        },
    ],
};

/// USB `0x534D` is a different kernel match. It is not a role of the initial rig.
pub const WORKBENCH_USB_IS_A_SEPARATE_MATCH: crate::linux_transport::UsbIdMatch = VARIAX_WORKBENCH;

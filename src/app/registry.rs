use super::detection::{DeviceDiscovery, DiscoveryProfile};
use super::{ProductFamily, hd500x_panel, podxt_panel, variax_panel};

pub(super) const FAMILIES: &[&dyn ProductFamily] = &[
    &variax_panel::FAMILY,
    &podxt_panel::FAMILY,
    &hd500x_panel::FAMILY,
];

pub(super) const LEGACY_STARTUP: &dyn ProductFamily = &variax_panel::FAMILY;

pub(super) const DEVICE_PROFILES: &[DiscoveryProfile] = &[
    DiscoveryProfile {
        family: &podxt_panel::FAMILY,
        discovery: DeviceDiscovery::Automatic(variax_workbench::device_probe::PODXT_LIVE_USB),
    },
    DiscoveryProfile {
        family: &variax_panel::FAMILY,
        discovery: DeviceDiscovery::ViaBridge(variax_workbench::device_probe::PODXT_LIVE_USB),
    },
    DiscoveryProfile {
        family: &hd500x_panel::FAMILY,
        discovery: DeviceDiscovery::Manual((
            variax_workbench::pod_hd500x::USB_VENDOR_ID,
            variax_workbench::pod_hd500x::USB_PRODUCT_ID,
        )),
    },
];

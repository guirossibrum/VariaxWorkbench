use super::{ProductFamily, hd500x_panel, podxt_panel, variax_panel};

pub(super) const FAMILIES: &[&dyn ProductFamily] = &[
    &variax_panel::FAMILY,
    &podxt_panel::FAMILY,
    &hd500x_panel::FAMILY,
];

pub(super) const LEGACY_STARTUP: &dyn ProductFamily = &variax_panel::FAMILY;

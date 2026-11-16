#![forbid(unsafe_code)]
//! Product profiles and version facts.
//!
//! Archived container words are catalog declarations. They are not installed
//! revisions of a connected unit. Missing evidence stays missing.

use crate::firmware::{
    PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION,
    VARIAX_500_700_PRODUCT_ID,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceFamily {
    /// Original Workbench Variax generation, including the 300/600 file identity.
    VariaxWorkbench,
    PodXt,
    /// Workbench HD only. Not the original Workbench family and not JTV-69.
    VariaxHd,
    /// James Tyler Variax JTV-69. Not Workbench HD and not the original family.
    Jtv,
    PodHd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductProfile {
    Variax600,
    PodxtLive,
    WorkbenchHd,
    Jtv69,
    PodHd500x,
}

impl ProductProfile {
    pub const ALL: [Self; 5] = [
        Self::Variax600,
        Self::PodxtLive,
        Self::WorkbenchHd,
        Self::Jtv69,
        Self::PodHd500x,
    ];

    pub const fn family(self) -> DeviceFamily {
        match self {
            Self::Variax600 => DeviceFamily::VariaxWorkbench,
            Self::PodxtLive => DeviceFamily::PodXt,
            Self::WorkbenchHd => DeviceFamily::VariaxHd,
            Self::Jtv69 => DeviceFamily::Jtv,
            Self::PodHd500x => DeviceFamily::PodHd,
        }
    }

    pub const fn is_extension(self) -> bool {
        matches!(self, Self::WorkbenchHd | Self::Jtv69 | Self::PodHd500x)
    }

    /// Firmware-container product id when one is archived.
    ///
    /// This is not a USB product id. Extension profiles have none registered.
    pub const fn archived_firmware_product(self) -> Option<u32> {
        match self {
            Self::Variax600 => Some(VARIAX_300_600_PRODUCT_ID),
            Self::PodxtLive => Some(PODXT_LIVE_PRODUCT_ID),
            Self::WorkbenchHd | Self::Jtv69 | Self::PodHd500x => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Variax600 => "Variax 600",
            Self::PodxtLive => "PODxt Live",
            Self::WorkbenchHd => "Workbench HD",
            Self::Jtv69 => "James Tyler Variax JTV-69",
            Self::PodHd500x => "POD HD500X",
        }
    }
}

/// Logical role on the initial rig. The two roles share one USB device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleKind {
    Guitar,
    Pod,
}

impl RoleKind {
    pub const fn profile(self) -> ProductProfile {
        match self {
            Self::Guitar => ProductProfile::Variax600,
            Self::Pod => ProductProfile::PodxtLive,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    /// FORM/L6FF HEAD version word from an archived container.
    ArchivedContainer,
    /// Separate PODxt USB-firmware component. Not main flash and not bcdDevice.
    InstalledUsbFirmware,
    /// Revision of a connected unit. No live read has established it.
    InstalledUnitRevision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionKnowledge {
    /// Catalog declaration only. Not proof of the connected unit.
    ArchivedDeclaration {
        word: u32,
        label: &'static str,
        evidence: &'static str,
    },
    Unknown {
        evidence: &'static str,
    },
    NotEstablished {
        evidence: &'static str,
    },
    NotApplicable {
        evidence: &'static str,
    },
}

impl VersionKnowledge {
    pub const fn is_known_value(self) -> bool {
        matches!(self, Self::ArchivedDeclaration { .. })
    }
}

pub(crate) fn version(profile: ProductProfile, component: Component) -> VersionKnowledge {
    match (profile, component) {
        (ProductProfile::Variax600, Component::ArchivedContainer) => {
            VersionKnowledge::ArchivedDeclaration {
                word: VARIAX_300_600_VERSION,
                label: "3.10",
                evidence: "HEAD version in the archived Variax 300/600 3.10 container. This is not a live read of the connected guitar.",
            }
        }
        (ProductProfile::Variax600, Component::InstalledUsbFirmware) => {
            VersionKnowledge::NotApplicable {
                evidence: "The USB-firmware component belongs to the PODxt Live role. The guitar role does not have a second copy of that version.",
            }
        }
        (ProductProfile::Variax600, Component::InstalledUnitRevision) => {
            VersionKnowledge::Unknown {
                evidence: "No live identity read is authorized. The archived 3.10 container word is not this unit's installed revision.",
            }
        }
        (ProductProfile::PodxtLive, Component::ArchivedContainer) => {
            VersionKnowledge::ArchivedDeclaration {
                word: PODXT_LIVE_VERSION,
                label: "3.01",
                evidence: "HEAD version in the archived PODxt Live 3.01 container. This is main-flash catalog identity, not USB PID 0x4650 and not the USB-firmware component.",
            }
        }
        (ProductProfile::PodxtLive, Component::InstalledUsbFirmware) => VersionKnowledge::Unknown {
            evidence: "The USB-firmware artifact and installed version remain unknown on #20. Main flash 3.01 and bcdDevice 0.01 are not this component.",
        },
        (ProductProfile::PodxtLive, Component::InstalledUnitRevision) => {
            VersionKnowledge::Unknown {
                evidence: "No live version read is authorized. The archived 3.01 container word is not a read of the connected unit.",
            }
        }
        (ProductProfile::WorkbenchHd | ProductProfile::Jtv69 | ProductProfile::PodHd500x, _) => {
            VersionKnowledge::NotEstablished {
                evidence: "No product id, firmware artifact, or version is registered for this profile.",
            }
        }
    }
}

pub(crate) const VARIAX_600_FILE_ID: u32 = VARIAX_300_600_PRODUCT_ID;
pub(crate) const VARIAX_500_700_FILE_ID: u32 = VARIAX_500_700_PRODUCT_ID;
pub(crate) const PODXT_LIVE_FILE_ID: u32 = PODXT_LIVE_PRODUCT_ID;

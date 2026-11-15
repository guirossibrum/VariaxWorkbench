//! Product and version applicability for archived FORM/L6FF identities.
//!
//! A catalog hit means the container's declared product id and version word match
//! an identity recorded from offline evidence. It does not mean these bytes are
//! the archived file, and it is not a signature check.

use std::fmt;

/// PODxt Live product id declared by the archived `PODxtLive_3_01.xtf` HEAD.
pub const PODXT_LIVE_PRODUCT_ID: u32 = 0x0003_000a;
/// Version word declared by that same PODxt Live 3.01 container.
pub const PODXT_LIVE_VERSION: u32 = 0x0301_0000;
/// Variax 300/600 product id declared by the archived `Variax300_3.10.vxf` HEAD.
/// Line 6 Central's Variax map uses the same value, decimal 458756.
pub const VARIAX_300_600_PRODUCT_ID: u32 = 0x0007_0004;
/// Version word declared by that same Variax 300/600 3.10 container.
pub const VARIAX_300_600_VERSION: u32 = 0x0310_0000;
/// Variax 500/700 MIDI identity from Central's map, decimal 458752.
/// No firmware image for this id is archived in this project.
pub const VARIAX_500_700_PRODUCT_ID: u32 = 0x0007_0000;

/// One offline product identity. `archived_version` is set only when a local
/// FORM/L6FF container was inspected and declared that exact version word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: &'static str,
    pub product_id: u32,
    pub archived_version: Option<u32>,
    pub evidence: &'static str,
}

pub const PRODUCT_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        name: "PODxt Live",
        product_id: PODXT_LIVE_PRODUCT_ID,
        archived_version: Some(PODXT_LIVE_VERSION),
        evidence: "HEAD product/version in the archived PODxtLive_3_01.xtf container. This is not USB PID 0x4650.",
    },
    CatalogEntry {
        name: "Variax 300/600",
        product_id: VARIAX_300_600_PRODUCT_ID,
        archived_version: Some(VARIAX_300_600_VERSION),
        evidence: "HEAD product/version in the archived Variax300_3.10.vxf container. Central maps decimal 458756 to Variax 300/600.",
    },
    CatalogEntry {
        name: "Variax 500/700",
        product_id: VARIAX_500_700_PRODUCT_ID,
        archived_version: None,
        evidence: "Central res/Variax.json maps decimal 458752 to 500/700. No matching firmware container is archived here.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// Declared product and version match an archived container's HEAD fields.
    RecordedDeclaration { name: &'static str },
    /// Product id matches an archived container, but the version word does not.
    WrongVersion {
        name: &'static str,
        recorded_version: u32,
    },
    /// The product id is known from offline evidence, and no image is archived.
    RecognizedWithoutArchivedImage { name: &'static str },
    /// The product id is absent from the offline catalog.
    UnknownProduct,
}

impl Applicability {
    pub fn catalog_name(self) -> Option<&'static str> {
        match self {
            Self::RecordedDeclaration { name }
            | Self::WrongVersion { name, .. }
            | Self::RecognizedWithoutArchivedImage { name } => Some(name),
            Self::UnknownProduct => None,
        }
    }

    pub fn recorded_version(self) -> Option<u32> {
        match self {
            Self::RecordedDeclaration { name } => PRODUCT_CATALOG
                .iter()
                .find(|entry| entry.name == name)
                .and_then(|entry| entry.archived_version),
            Self::WrongVersion {
                recorded_version, ..
            } => Some(recorded_version),
            Self::RecognizedWithoutArchivedImage { .. } | Self::UnknownProduct => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::RecordedDeclaration { .. } => "recorded-declaration",
            Self::WrongVersion { .. } => "wrong-version",
            Self::RecognizedWithoutArchivedImage { .. } => "recognized-without-archived-image",
            Self::UnknownProduct => "unknown-product",
        }
    }
}

impl fmt::Display for Applicability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordedDeclaration { name } => write!(
                f,
                "recorded declaration for {name}; this matches catalog identity fields only and is not authenticity or a byte-for-byte file check"
            ),
            Self::WrongVersion {
                name,
                recorded_version,
            } => write!(
                f,
                "product matches {name}, but the version is not the archived revision {recorded_version:08x}"
            ),
            Self::RecognizedWithoutArchivedImage { name } => write!(
                f,
                "{name} is a recognized product id and has no archived firmware image in this catalog"
            ),
            Self::UnknownProduct => write!(f, "product id is not in the offline catalog"),
        }
    }
}

pub(crate) fn classify(product_id: u32, version: u32) -> Applicability {
    let Some(entry) = PRODUCT_CATALOG
        .iter()
        .find(|entry| entry.product_id == product_id)
    else {
        return Applicability::UnknownProduct;
    };
    match entry.archived_version {
        Some(recorded) if recorded == version => {
            Applicability::RecordedDeclaration { name: entry.name }
        }
        Some(recorded) => Applicability::WrongVersion {
            name: entry.name,
            recorded_version: recorded,
        },
        None => Applicability::RecognizedWithoutArchivedImage { name: entry.name },
    }
}

/// Display the top two bytes as `major.minor` hex.
///
/// The archived containers declare `0x03010000` for 3.01 and `0x03100000` for
/// 3.10, so those two revisions render as `3.01` and `3.10`. Versions with the
/// low 16 bits set are not given this label.
pub fn observed_version_label(version: u32) -> Option<String> {
    if version & 0xffff != 0 {
        return None;
    }
    Some(format!(
        "{:x}.{:02x}",
        version >> 24,
        (version >> 16) & 0xff
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicabilityError {
    WrongProduct { declared: u32, expected: u32 },
    WrongVersion { declared: u32, expected: u32 },
}

impl fmt::Display for ApplicabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProduct { declared, expected } => write!(
                f,
                "wrong product: container declares {declared:08x}, expected {expected:08x}"
            ),
            Self::WrongVersion { declared, expected } => write!(
                f,
                "wrong version: container declares {declared:08x}, expected {expected:08x}"
            ),
        }
    }
}

impl std::error::Error for ApplicabilityError {}

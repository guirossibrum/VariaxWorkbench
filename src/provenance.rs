#![forbid(unsafe_code)]
//! Offline artifact provenance.
//!
//! These constants restate saved inventories and catalog responses. They do not
//! download files, open USB, MIDI, or audio devices, or check a signature.
//! Size and MD5 agreement with a catalog row is not authentication.

/// Directory that holds the ignored local archives. Tests do not open it.
pub const INSTALLER_DIR: &str = "vendor/installers";

pub const SIGNATURE_STATEMENT: &str = "MD5, SHA-256, and catalog size/MD5 agreement are integrity and listing checks. None of them is a digital signature, and no Authenticode or codesign check was performed.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Firmware,
    Editor,
    Updater,
    LicenseManager,
    DriverPackage,
    CentralApplication,
}

/// How the source URL is known. A documented URL that the download inventory
/// does not confirm stays distinct from a URL recorded with the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acquisition {
    RecordedUrl {
        url: &'static str,
        independent_record: &'static str,
    },
    DocumentedUrlNotInInventory {
        url: &'static str,
        document: &'static str,
    },
    UrlNotCaptured,
}

/// Relationship between a local file and a saved catalog row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogClaim {
    /// Local length and MD5 equal the saved catalog row for this release.
    SizeAndMd5 {
        release_id: u32,
    },
    /// A public listing names the release. Exact catalog size and MD5 were not saved.
    PublicListing {
        release_id: u32,
    },
    NotApplicable,
}

/// No variant means "signature checked."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureStatus {
    NotChecked,
}

impl SignatureStatus {
    pub const fn verified(self) -> bool {
        match self {
            Self::NotChecked => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    NotRun,
}

impl Execution {
    pub const fn ran(self) -> bool {
        match self {
            Self::NotRun => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialUse {
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareContact {
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalArtifact {
    pub id: &'static str,
    pub product: &'static str,
    pub version: &'static str,
    pub role: Role,
    pub archive_names: &'static [&'static str],
    pub bytes: u64,
    pub md5: &'static str,
    pub sha256: &'static str,
    pub acquisition: Acquisition,
    pub catalog: CatalogClaim,
    pub evidence: &'static [&'static str],
    pub signature: SignatureStatus,
    pub execution: Execution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogRelease {
    pub product: &'static str,
    pub midi_id: u32,
    pub release_id: u32,
    pub version: &'static str,
    pub release_date: &'static str,
    pub release_date_unix: u64,
    pub bytes: u64,
    pub md5: &'static str,
    pub pre_update_url: &'static str,
    pub post_update_url: &'static str,
    pub fail_update_url: &'static str,
    pub listing_page: Option<&'static str>,
    pub local_artifact_id: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicFlashListing {
    pub product: &'static str,
    pub version: &'static str,
    pub release_id: u32,
    pub release_date: &'static str,
    pub size_text: &'static str,
    pub page_url: &'static str,
    pub catalog_md5_saved: bool,
    pub exact_catalog_bytes_saved: bool,
    pub local_artifact_id: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateStep {
    pub name: &'static str,
    pub client_action: &'static str,
    pub next_step_signal: &'static str,
    pub engine_post_action: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductDeclaration {
    pub midi_id: u32,
    pub name: &'static str,
    /// Installer field. Zero here is not an observed guitar or POD version.
    pub min_installed_firmware_version: u32,
    pub min_app_version: &'static str,
    pub firmware_minimum_is_observed_device_version: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeniedRequest {
    pub description: &'static str,
    pub result: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CentralCatalogEvidence {
    pub installer_artifact_id: &'static str,
    pub variax_json_sha256: &'static str,
    pub release_list_endpoint: &'static str,
    pub queried_midi_id: u32,
    pub public_app_identifier_required: bool,
    pub public_app_identifier_included: bool,
    pub credentials: CredentialUse,
    pub hardware: HardwareContact,
    pub updater: Execution,
    pub getrelease_url: &'static str,
    pub getrelease_status: &'static str,
    pub getrelease_code: u32,
    pub getrelease_message: &'static str,
    pub browser_denied: &'static [DeniedRequest],
    pub ordered_update_steps: &'static [UpdateStep],
    pub failure_step: UpdateStep,
    pub products: &'static [ProductDeclaration],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonkeyEvidence {
    pub mac_artifact_id: &'static str,
    pub windows_artifact_id: &'static str,
    pub historical_download_url: &'static str,
    pub historical_download_source: &'static str,
    pub historical_url_recontacted: bool,
    pub update_symbols: &'static [&'static str],
    pub symbols_reconstruct_an_updater: bool,
    pub updater: Execution,
    /// Text inside the saved 3.10 catalog notes. It is not evidence that an update ran.
    pub release_note_minimum: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    pub id: &'static str,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DigestCheck {
    pub length_matches: bool,
    pub md5_matches: bool,
    pub sha256_matches: bool,
    pub signature_verified: bool,
}

const CENTRAL_URL: &str =
    "https://l6c-acdn2.line6.net/media/software/12401/Line6CentralInstaller1.00.exe";
const VARIAX_LISTING_PAGE: &str = "https://line6.com/software/index.html?hardware=Variax%20300%2F600&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show";
const POD_LISTING_PAGE: &str = "https://line6.com/software/index.html?hardware=PODxt%20Live&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show";
const MONKEY_HISTORICAL_URL: &str = "https://line6.com/getrelease?rid=8688";

pub const LOCAL_ARTIFACTS: &[LocalArtifact] = &[
    LocalArtifact {
        id: "variax-300-600-3.10",
        product: "Variax 300/600",
        version: "3.10",
        role: Role::Firmware,
        archive_names: &["Variax300_3.10.vxf"],
        bytes: 124_208,
        md5: "8b64eddd73f1d4a1eaa72c8dbd98ece5",
        sha256: "e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::SizeAndMd5 { release_id: 1062 },
        evidence: &[
            "analysis/reports/user-downloads-20261115.json",
            "analysis/reports/factory-central-release-list-458756.json",
            "docs/factory-reference.md",
        ],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "podxt-live-3.01",
        product: "PODxt Live",
        version: "3.01",
        role: Role::Firmware,
        archive_names: &["PODxtLive_3_01.xtf"],
        bytes: 421_218,
        md5: "faabb5e526bcba858d2defe96547e26c",
        sha256: "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::PublicListing { release_id: 1224 },
        evidence: &[
            "analysis/reports/pod-user-downloads.json",
            "docs/factory-reference.md",
        ],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "central-windows-1.00",
        product: "Line 6 Central",
        version: "1.00",
        role: Role::CentralApplication,
        archive_names: &["Line6CentralInstaller1.00.exe"],
        bytes: 98_587_952,
        md5: "f8b524cc176dd1b6e00504e0be339408",
        sha256: "6d7a4ba9efb2a6f9f45dd996cd2a7fe34eb7355ec77c8d4a6795b6c64bc9aee6",
        acquisition: Acquisition::RecordedUrl {
            url: CENTRAL_URL,
            independent_record: "docs/factory-reference.md records that the Microsoft winget manifest for Line 6 Central 1.00 has this SHA-256. That manifest was not downloaded again for this record.",
        },
        catalog: CatalogClaim::NotApplicable,
        evidence: &[
            "analysis/reports/factory-central-offline-summary.json",
            "docs/factory-reference.md",
        ],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "monkey-macos-1.78",
        product: "Line 6 Monkey",
        version: "1.78",
        role: Role::Updater,
        archive_names: &["Line 6 Monkey 1.78.dmg", "Line6-Monkey-1.78.dmg"],
        bytes: 3_792_653,
        md5: "72b37b904ad7838b8d849ff3cde04dee",
        sha256: "96dbb7f11635a736a84def227a76dd99aa8028cff4d01f047774e2d1f3b3b3a0",
        acquisition: Acquisition::DocumentedUrlNotInInventory {
            url: MONKEY_HISTORICAL_URL,
            document: "docs/native-analysis.md says Mac Monkey 1.78 was downloaded from this URL. The later user-download inventory for the same SHA-256 says the URL was not recorded. This record does not contact the endpoint again.",
        },
        catalog: CatalogClaim::NotApplicable,
        evidence: &[
            "analysis/reports/inventory.json",
            "analysis/reports/user-downloads-20261115.json",
            "docs/native-analysis.md",
        ],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "monkey-windows-1.77",
        product: "Line 6 Monkey",
        version: "1.77",
        role: Role::Updater,
        archive_names: &["Line6-Monkey-1.77.exe"],
        bytes: 5_328_432,
        md5: "0e4a0bf7bd0ee0ed5ca07b4f008eea7c",
        sha256: "9e4e487226f44ae8b58b67e67c55cf460447af5d290de1e1fd02ab3dce49b590",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/inventory.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "license-manager-windows-1.16",
        product: "Line 6 License Manager",
        version: "1.16",
        role: Role::LicenseManager,
        archive_names: &["Line 6 License Manager v1.16 Installer.exe"],
        bytes: 2_700_824,
        md5: "bbd1856f956b06916a9cb6af825e95d1",
        sha256: "5c2eb5ee09cd2f029507969bbf9c9279d80ea9489c31f830784048cc962123d2",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/user-downloads-20261115.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "workbench-macos-1.75",
        product: "Variax Workbench",
        version: "1.75",
        role: Role::Editor,
        archive_names: &["Variax Workbench 1.75.dmg"],
        bytes: 18_085_510,
        md5: "f7a2bec33a1efc33b98b9ed53b314b63",
        sha256: "d70b73a1b0e441e7a640c387f13e42f44284388063b155fa162fcc771a614fa1",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/inventory.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "workbench-windows-1.75",
        product: "Variax Workbench",
        version: "1.75",
        role: Role::Editor,
        archive_names: &["Variax Workbench v1.75 Installer.exe"],
        bytes: 19_871_616,
        md5: "3f452b92bd7a3234d3d8afb23c9b059d",
        sha256: "a6870ad2792fb0ee70683be0402cbc828ac316dcd1aa66abb2813cdc1df40f99",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/inventory.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "driver-windows-4.2.7.7",
        product: "Line 6 Drivers",
        version: "4.2.7.7",
        role: Role::DriverPackage,
        archive_names: &["Line 6 Drivers v4.2.7.7 Installer.exe"],
        bytes: 29_821_648,
        md5: "e11c8ad8bdd169692f724da4ac290def",
        sha256: "dd2f3a787006ecb0341b0ff2f4a8699c00df274b494eb6ed2c6beae4e13c7b92",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/pod-user-downloads.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
    LocalArtifact {
        id: "driver-macos-7.6.8",
        product: "Line 6 Audio-Midi Driver",
        version: "7.6.8",
        role: Role::DriverPackage,
        archive_names: &["Line 6 Audio-Midi Driver 7.6.8.dmg"],
        bytes: 2_869_592,
        md5: "a54c2f5da10ea8838c4e85ca992975ed",
        sha256: "48099370936bea11e09b9d8ee4828e2c9050dd1a1da1586828b7f5a72b559436",
        acquisition: Acquisition::UrlNotCaptured,
        catalog: CatalogClaim::NotApplicable,
        evidence: &["analysis/reports/pod-user-downloads.json"],
        signature: SignatureStatus::NotChecked,
        execution: Execution::NotRun,
    },
];

pub const CATALOG_RELEASES: &[CatalogRelease] = &[
    CatalogRelease {
        product: "Variax 300/600",
        midi_id: 458_756,
        release_id: 1062,
        version: "3.10",
        release_date: "2006-01-06 17:34:03",
        release_date_unix: 1_136_597_643,
        bytes: 124_208,
        md5: "8b64eddd73f1d4a1eaa72c8dbd98ece5",
        pre_update_url: "",
        post_update_url: "",
        fail_update_url: "",
        listing_page: Some(VARIAX_LISTING_PAGE),
        local_artifact_id: Some("variax-300-600-3.10"),
    },
    CatalogRelease {
        product: "Variax 500/700",
        midi_id: 458_752,
        release_id: 1061,
        version: "3.10",
        release_date: "2006-01-06 17:34:01",
        release_date_unix: 1_136_597_641,
        bytes: 121_870,
        md5: "d6c5566c19ee4183a105b0ef302e7c47",
        pre_update_url: "",
        post_update_url: "",
        fail_update_url: "",
        listing_page: None,
        local_artifact_id: None,
    },
    CatalogRelease {
        product: "Variax 500/700",
        midi_id: 458_752,
        release_id: 740,
        version: "3.05",
        release_date: "2005-08-03 09:07:20",
        release_date_unix: 1_123_085_240,
        bytes: 121_346,
        md5: "bb0256d9b10d6b64ef4c4923be4029bb",
        pre_update_url: "",
        post_update_url: "",
        fail_update_url: "",
        listing_page: None,
        local_artifact_id: None,
    },
];

pub const PUBLIC_LISTINGS: &[PublicFlashListing] = &[PublicFlashListing {
    product: "PODxt Live",
    version: "3.01",
    release_id: 1224,
    release_date: "2006-04-20",
    size_text: "approximately 411 KB",
    page_url: POD_LISTING_PAGE,
    catalog_md5_saved: false,
    exact_catalog_bytes_saved: false,
    local_artifact_id: Some("podxt-live-3.01"),
}];

const UPDATE_STEPS: &[UpdateStep] = &[
    UpdateStep {
        name: "sendEula",
        client_action: "displayEula",
        next_step_signal: "clientAcceptedEula",
        engine_post_action: "none",
    },
    UpdateStep {
        name: "sendKeepPresetsOption",
        client_action: "displayYesNoView",
        next_step_signal: "clientRespondedToKeepPresetOption",
        engine_post_action: "none",
    },
    UpdateStep {
        name: "sendReportEngineProgressOrder",
        client_action: "reportUpdateEngineProgress",
        next_step_signal: "updateEngineUpdateCompleted",
        engine_post_action: "startUpdateEngine",
    },
    UpdateStep {
        name: "sendUpdateCompletedHTML",
        client_action: "displayStaticInfoViewDismissibleOK",
        next_step_signal: "none",
        engine_post_action: "none",
    },
];

const FAILURE_STEP: UpdateStep = UpdateStep {
    name: "updateFailed",
    client_action: "displayStaticInfoViewDismissibleOK",
    next_step_signal: "none",
    engine_post_action: "none",
};

const PRODUCT_DECLARATIONS: &[ProductDeclaration] = &[
    ProductDeclaration {
        midi_id: 458_752,
        name: "Variax 500 700",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
    ProductDeclaration {
        midi_id: 458_753,
        name: "Variax Acoustic",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
    ProductDeclaration {
        midi_id: 458_754,
        name: "Variax Bass",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
    ProductDeclaration {
        midi_id: 458_756,
        name: "Variax 300 600",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
    ProductDeclaration {
        midi_id: 458_757,
        name: "Variax Tyler",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
    ProductDeclaration {
        midi_id: 458_758,
        name: "Variax Bass 705",
        min_installed_firmware_version: 0,
        min_app_version: "0.90.0a",
        firmware_minimum_is_observed_device_version: false,
    },
];

pub const CENTRAL_CATALOG: CentralCatalogEvidence = CentralCatalogEvidence {
    installer_artifact_id: "central-windows-1.00",
    variax_json_sha256: "0212bb5832870d6d8f74d61445d1c0ce1043d7657b50892f561766277fef1b1e",
    release_list_endpoint: "https://line6.com/api/rest/v1/release/list",
    queried_midi_id: 458_756,
    public_app_identifier_required: true,
    public_app_identifier_included: false,
    credentials: CredentialUse::None,
    hardware: HardwareContact::None,
    updater: Execution::NotRun,
    getrelease_url: "https://line6.com/api/rest/v1/getrelease?rid=1062",
    getrelease_status: "ERROR",
    getrelease_code: 15_877,
    getrelease_message: "No Token!",
    browser_denied: &[
        DeniedRequest {
            description: "https://line6.com/getrelease?rid=1062",
            result: "HTTP 403",
        },
        DeniedRequest {
            description: "POD release 1224; the saved note does not give a second full URL",
            result: "HTTP 403",
        },
    ],
    ordered_update_steps: UPDATE_STEPS,
    failure_step: FAILURE_STEP,
    products: PRODUCT_DECLARATIONS,
};

pub const MONKEY: MonkeyEvidence = MonkeyEvidence {
    mac_artifact_id: "monkey-macos-1.78",
    windows_artifact_id: "monkey-windows-1.77",
    historical_download_url: MONKEY_HISTORICAL_URL,
    historical_download_source: "docs/native-analysis.md",
    historical_url_recontacted: false,
    update_symbols: &["L6CamelSystem::UpdateFirmware", "MNKYUpdateFirmware"],
    symbols_reconstruct_an_updater: false,
    updater: Execution::NotRun,
    release_note_minimum: "Line 6 Monkey 1.10 or later",
};

pub const ACQUISITION_GAPS: &[Gap] = &[
    Gap {
        id: "not-a-signature",
        detail: "Catalog size/MD5 agreement and local SHA-256 do not authenticate a file. No signature scheme was checked.",
    },
    Gap {
        id: "no-token",
        detail: "The REST getrelease request for rid 1062 returned ERROR 15877, No Token. No account or credential was used.",
    },
    Gap {
        id: "http-403",
        detail: "The browser getrelease page for rid 1062 and the noted POD release 1224 request returned HTTP 403. No account-free firmware download route was established.",
    },
    Gap {
        id: "pod-catalog-md5-absent",
        detail: "PODxt Live 3.01 has local size, MD5, and SHA-256. The saved public listing has no catalog MD5 and no exact byte length. The local file is the main flash container, not a separate USB-component firmware.",
    },
    Gap {
        id: "variax-500-700-not-obtained",
        detail: "Variax 500/700 releases 1061 and 740 have different sizes and MD5 values and were not downloaded. They are not substitutes for release 1062.",
    },
    Gap {
        id: "source-urls-missing",
        detail: "Workbench, Monkey 1.77, License Manager, both driver installers, and both firmware files have no captured source URL. The Monkey 1.78 URL is a historical document claim that the user-download inventory does not confirm.",
    },
    Gap {
        id: "app-identifier-omitted",
        detail: "The catalog query required a public application identifier. That value was omitted from the saved evidence and is not stored here.",
    },
    Gap {
        id: "central-1-06-not-obtained",
        detail: "The Central listing 1.06 dated 2026-09-17 was not downloaded. The archived installer is 1.00.",
    },
    Gap {
        id: "firmware-minimum-not-observed",
        detail: "Variax.json minInstalledFirmwareVersion 0 is an installer declaration, not an observed device firmware version.",
    },
    Gap {
        id: "license-server-not-preserved",
        detail: "Keeping the License Manager and Monkey installers does not keep their transaction server available after the announced 2026-04-10 retirement.",
    },
];

pub fn artifact_by_id(id: &str) -> Option<&'static LocalArtifact> {
    LOCAL_ARTIFACTS.iter().find(|artifact| artifact.id == id)
}

pub fn artifact_by_archive_name(name: &str) -> Option<&'static LocalArtifact> {
    LOCAL_ARTIFACTS
        .iter()
        .find(|artifact| artifact.archive_names.contains(&name))
}

pub fn catalog_release(release_id: u32) -> Option<&'static CatalogRelease> {
    CATALOG_RELEASES
        .iter()
        .find(|release| release.release_id == release_id)
}

pub fn public_listing(release_id: u32) -> Option<&'static PublicFlashListing> {
    PUBLIC_LISTINGS
        .iter()
        .find(|listing| listing.release_id == release_id)
}

/// Compare already-loaded bytes with the recorded length and digests.
///
/// `signature_verified` stays false even when both digests match.
pub fn check_bytes(artifact: &LocalArtifact, bytes: &[u8]) -> DigestCheck {
    let md5 = format!("{:x}", <md5::Md5 as md5::Digest>::digest(bytes));
    let sha256 = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes));
    let length_matches = bytes.len() as u64 == artifact.bytes;
    DigestCheck {
        length_matches,
        md5_matches: length_matches && md5 == artifact.md5,
        sha256_matches: length_matches && sha256 == artifact.sha256,
        signature_verified: false,
    }
}

pub fn validate_catalog() -> Result<(), String> {
    let mut ids = Vec::new();
    let mut names = Vec::new();
    let mut sha256 = Vec::new();
    for artifact in LOCAL_ARTIFACTS {
        if artifact.id.is_empty() || ids.contains(&artifact.id) {
            return Err(format!("duplicate or empty artifact id {}", artifact.id));
        }
        ids.push(artifact.id);
        if artifact.archive_names.is_empty() || artifact.evidence.is_empty() {
            return Err(format!("{} is missing names or evidence", artifact.id));
        }
        if artifact.bytes == 0
            || !is_lower_hex(artifact.md5, 32)
            || !is_lower_hex(artifact.sha256, 64)
        {
            return Err(format!("{} has a bad length or digest", artifact.id));
        }
        if artifact.signature.verified() || artifact.execution.ran() {
            return Err(format!(
                "{} claims a signature or an execution",
                artifact.id
            ));
        }
        for name in artifact.archive_names {
            if name.is_empty() || names.contains(name) {
                return Err(format!("duplicate archive name {name}"));
            }
            names.push(*name);
        }
        if sha256.contains(&artifact.sha256) {
            return Err(format!("unexpected shared SHA-256 on {}", artifact.id));
        }
        sha256.push(artifact.sha256);
        let url = match artifact.acquisition {
            Acquisition::RecordedUrl { url, .. }
            | Acquisition::DocumentedUrlNotInInventory { url, .. } => Some(url),
            Acquisition::UrlNotCaptured => None,
        };
        if let Some(url) = url
            && !url.starts_with("https://")
        {
            return Err(format!("{} URL is not https", artifact.id));
        }
        match artifact.catalog {
            CatalogClaim::SizeAndMd5 { release_id } => {
                let release = catalog_release(release_id)
                    .ok_or_else(|| format!("missing catalog release {release_id}"))?;
                if release.local_artifact_id != Some(artifact.id)
                    || release.bytes != artifact.bytes
                    || release.md5 != artifact.md5
                    || release.version != artifact.version
                {
                    return Err(format!("{release_id} does not agree with {}", artifact.id));
                }
            }
            CatalogClaim::PublicListing { release_id } => {
                let listing = public_listing(release_id)
                    .ok_or_else(|| format!("missing public listing {release_id}"))?;
                if listing.local_artifact_id != Some(artifact.id)
                    || listing.catalog_md5_saved
                    || listing.exact_catalog_bytes_saved
                {
                    return Err(format!("listing {release_id} overclaims catalog data"));
                }
            }
            CatalogClaim::NotApplicable => {}
        }
    }

    let size_and_md5 = LOCAL_ARTIFACTS
        .iter()
        .filter(|artifact| matches!(artifact.catalog, CatalogClaim::SizeAndMd5 { .. }))
        .count();
    if size_and_md5 != 1 {
        return Err(format!(
            "expected one size/MD5 agreement, found {size_and_md5}"
        ));
    }
    let release_1062 = catalog_release(1062).ok_or("missing release 1062")?;
    let release_1061 = catalog_release(1061).ok_or("missing release 1061")?;
    let release_740 = catalog_release(740).ok_or("missing release 740")?;
    if release_1061.local_artifact_id.is_some() || release_740.local_artifact_id.is_some() {
        return Err("a 500/700 release is marked obtained".into());
    }
    if release_1061.md5 == release_1062.md5
        || release_740.md5 == release_1062.md5
        || release_1061.md5 == release_740.md5
        || release_1061.bytes == release_1062.bytes
        || release_740.bytes == release_1062.bytes
    {
        return Err("500/700 catalog rows are not distinct from 300/600".into());
    }
    for release in CATALOG_RELEASES {
        if !release.pre_update_url.is_empty()
            || !release.post_update_url.is_empty()
            || !release.fail_update_url.is_empty()
        {
            return Err(format!("release {} has an update URL", release.release_id));
        }
    }

    if CENTRAL_CATALOG.public_app_identifier_included
        || CENTRAL_CATALOG.credentials != CredentialUse::None
        || CENTRAL_CATALOG.hardware != HardwareContact::None
        || CENTRAL_CATALOG.updater.ran()
        || CENTRAL_CATALOG.getrelease_code != 15_877
        || CENTRAL_CATALOG.getrelease_message != "No Token!"
        || !is_lower_hex(CENTRAL_CATALOG.variax_json_sha256, 64)
    {
        return Err("Central evidence overclaims credentials, hardware, or a token".into());
    }
    if CENTRAL_CATALOG
        .ordered_update_steps
        .iter()
        .any(|step| step.name == "updateFailed")
        || CENTRAL_CATALOG.failure_step.name != "updateFailed"
    {
        return Err("updateFailed is in the wrong step list".into());
    }
    if CENTRAL_CATALOG.products.iter().any(|product| {
        product.firmware_minimum_is_observed_device_version
            || product.min_installed_firmware_version != 0
    }) {
        return Err("a product declaration is treated as an observed firmware version".into());
    }
    if MONKEY.historical_url_recontacted
        || MONKEY.symbols_reconstruct_an_updater
        || MONKEY.updater.ran()
    {
        return Err("Monkey evidence overclaims an update".into());
    }
    let monkey = artifact_by_id(MONKEY.mac_artifact_id).ok_or("missing Monkey mac artifact")?;
    let spaced =
        artifact_by_archive_name("Line 6 Monkey 1.78.dmg").ok_or("missing spaced Monkey name")?;
    let compact =
        artifact_by_archive_name("Line6-Monkey-1.78.dmg").ok_or("missing compact Monkey name")?;
    if monkey.archive_names.len() != 2 || spaced.id != monkey.id || compact.id != monkey.id {
        return Err("Monkey 1.78 names do not share one record".into());
    }
    if !matches!(
        monkey.acquisition,
        Acquisition::DocumentedUrlNotInInventory { .. }
    ) {
        return Err("Monkey 1.78 URL is labeled as confirmed".into());
    }
    let required = [
        "not-a-signature",
        "no-token",
        "http-403",
        "pod-catalog-md5-absent",
        "variax-500-700-not-obtained",
        "source-urls-missing",
        "app-identifier-omitted",
    ];
    for id in required {
        if !ACQUISITION_GAPS.iter().any(|gap| gap.id == id) {
            return Err(format!("missing gap {id}"));
        }
    }
    Ok(())
}

fn is_lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{
        Acquisition, CatalogClaim, Execution, INSTALLER_DIR, LOCAL_ARTIFACTS, SIGNATURE_STATEMENT,
        SignatureStatus, artifact_by_archive_name, artifact_by_id, catalog_release, check_bytes,
        validate_catalog,
    };

    #[test]
    fn saved_records_keep_catalog_agreement_distinct_from_a_signature() {
        validate_catalog().expect("provenance catalog");
        assert!(!SIGNATURE_STATEMENT.is_empty());
        assert_eq!(INSTALLER_DIR, "vendor/installers");
        assert!(LOCAL_ARTIFACTS.iter().all(|artifact| {
            artifact.signature == SignatureStatus::NotChecked
                && artifact.execution == Execution::NotRun
        }));
        let variax = artifact_by_id("variax-300-600-3.10").expect("variax");
        let release = catalog_release(1062).expect("1062");
        assert_eq!(variax.bytes, release.bytes);
        assert_eq!(variax.md5, release.md5);
        assert!(matches!(
            variax.catalog,
            CatalogClaim::SizeAndMd5 { release_id: 1062 }
        ));
        assert!(matches!(variax.acquisition, Acquisition::UrlNotCaptured));
        assert_ne!(catalog_release(1061).expect("1061").md5, variax.md5);
        assert!(
            catalog_release(740)
                .expect("740")
                .local_artifact_id
                .is_none()
        );
    }

    #[test]
    fn central_and_monkey_records_do_not_claim_a_completed_download_or_update() {
        validate_catalog().expect("provenance catalog");
        assert_eq!(super::CENTRAL_CATALOG.getrelease_message, "No Token!");
        let central = artifact_by_id("central-windows-1.00").expect("central");
        assert!(
            matches!(central.acquisition, Acquisition::RecordedUrl { url, .. } if url.contains("Line6CentralInstaller1.00.exe"))
        );
        let pod = artifact_by_id("podxt-live-3.01").expect("pod");
        assert!(matches!(
            pod.catalog,
            CatalogClaim::PublicListing { release_id: 1224 }
        ));
        assert!(
            super::public_listing(1224)
                .expect("1224")
                .size_text
                .contains("411 KB")
        );
    }

    #[test]
    fn byte_comparison_can_match_and_still_is_not_a_signature() {
        let empty = LocalArtifactStub::empty();
        let matched = check_bytes(&empty.artifact, b"");
        assert!(matched.length_matches && matched.md5_matches && matched.sha256_matches);
        assert!(!matched.signature_verified);
        let mismatched = check_bytes(&empty.artifact, b"not-the-archive");
        assert!(!mismatched.md5_matches && !mismatched.sha256_matches);
        assert!(!mismatched.signature_verified);
        assert!(artifact_by_archive_name("Line6-Monkey-1.78.dmg").is_some());
    }

    struct LocalArtifactStub {
        artifact: super::LocalArtifact,
    }

    impl LocalArtifactStub {
        fn empty() -> Self {
            Self {
                artifact: super::LocalArtifact {
                    id: "empty-synthetic",
                    product: "synthetic",
                    version: "0",
                    role: super::Role::Editor,
                    archive_names: &["empty.bin"],
                    bytes: 0,
                    md5: "d41d8cd98f00b204e9800998ecf8427e",
                    sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    acquisition: Acquisition::UrlNotCaptured,
                    catalog: CatalogClaim::NotApplicable,
                    evidence: &["synthetic"],
                    signature: SignatureStatus::NotChecked,
                    execution: Execution::NotRun,
                },
            }
        }
    }
}

#![forbid(unsafe_code)]
//! POD HD500X driver-text identity and the public catalog limit.
//!
//! USB ids are match values copied from static driver text. They are not a
//! live enumeration and not a firmware product id. The Edit 2.27 installers
//! were not acquired. Two public manuals supply printed file names only.
//! Nothing here opens a device, converts another family's file, or installs
//! a driver.

use crate::firmware::{
    PODXT_LIVE_PRODUCT_ID, VARIAX_300_600_PRODUCT_ID, VARIAX_500_700_PRODUCT_ID,
};
use crate::provenance::{self, CredentialUse, Execution, HardwareContact};

/// Windows INF and Mac kext text agree on this vendor id.
pub const USB_VENDOR_ID: u16 = 0x0e41;
/// `USB\VID_0E41&PID_4159` in `l6PODHD5SE.inf`. Decimal 16729.
pub const USB_PRODUCT_ID: u16 = 0x4159;
pub const HARDWARE_ID: &str = "USB\\VID_0E41&PID_4159";
pub const DEVICE_NAME: &str = "POD HD500X";
pub const DRIVER_VER: &str = "03/15/2019,4.2.7.7";
pub const MAC_VENDOR_ID: u32 = 3649;
pub const MAC_PRODUCT_ID: u32 = 16729;

/// POD HD500 without the X. Windows `PID_414D` and Mac personality `PODHD500`.
pub const POD_HD500_USB_PRODUCT_ID: u16 = 0x414d;
pub const POD_HD500_MAC_PRODUCT_ID: u32 = 16717;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbNeighbor {
    pub name: &'static str,
    pub product_id: u16,
}

/// Hardware ids from other INFs in the same Windows driver package.
/// Each one is a different product.
pub const USB_NEIGHBORS: &[UsbNeighbor] = &[
    UsbNeighbor {
        name: "POD HD 500",
        product_id: POD_HD500_USB_PRODUCT_ID,
    },
    UsbNeighbor {
        name: "POD HD",
        product_id: 0x4156,
    },
    UsbNeighbor {
        name: "POD HD Pro",
        product_id: 0x4157,
    },
    UsbNeighbor {
        name: "POD HD Pro X",
        product_id: 0x415a,
    },
    UsbNeighbor {
        name: "POD HD 300",
        product_id: 0x5057,
    },
    UsbNeighbor {
        name: "POD HD 400",
        product_id: 0x5058,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacPersonality {
    pub name: &'static str,
    /// Present only when the personality dict contains `bConfigurationValue`.
    pub configuration: Option<u32>,
    /// Present only when the personality dict contains `bInterfaceNumber`.
    pub interface_number: Option<u32>,
    pub io_class: &'static str,
    pub role: &'static str,
}

pub const MAC_PERSONALITIES: &[MacPersonality] = &[
    MacPersonality {
        name: "PODHD500X",
        configuration: Some(1),
        interface_number: Some(0),
        io_class: "com_line6_audio",
        role: "audio interface match",
    },
    MacPersonality {
        name: "PODHD500XBulk",
        configuration: Some(1),
        interface_number: Some(1),
        io_class: "com_line6_driver_bulkpipecomm",
        role: "bulk interface match",
    },
    MacPersonality {
        name: "PODHD500XDevice",
        configuration: None,
        interface_number: None,
        io_class: "AppleUSBComposite",
        role: "composite device match",
    },
    MacPersonality {
        name: "PODHD500XClassic",
        configuration: None,
        interface_number: None,
        io_class: "AppleUSBMergeNub",
        role: "classic merge match",
    },
];

/// Registry defaults in `l6PODHD5SE.inf`. `EngineFormatLock` 0 means the
/// sample rate and word size are defaults. They were not negotiated and no
/// stream was opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioDefaults {
    pub default_word_size: u32,
    pub default_sample_rate: u32,
    pub engine_format_lock: u32,
    pub asio_perf_buffer_size: u32,
    pub asio_word_size: u32,
    pub asio_name: &'static str,
    pub stream_opened: bool,
    pub format_negotiated: bool,
}

pub const AUDIO_DEFAULTS: AudioDefaults = AudioDefaults {
    default_word_size: 24,
    default_sample_rate: 48_000,
    engine_format_lock: 0,
    asio_perf_buffer_size: 512,
    asio_word_size: 24,
    asio_name: "ASIO POD HD500X",
    stream_opened: false,
    format_negotiated: false,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticText {
    pub archive_id: &'static str,
    pub path: &'static str,
    pub bytes: u64,
    pub md5: &'static str,
    pub sha256: &'static str,
    /// Native driver files were not loaded or executed.
    pub executed: bool,
}

pub const WINDOWS_INF: StaticText = StaticText {
    archive_id: "driver-windows-4.2.7.7",
    path: "Tools/Driver Archive/POD HD 500X/l6PODHD5SE.inf",
    bytes: 10_484,
    md5: "37f0d8c24dc286d2a629b8e6d7a2a261",
    sha256: "50080c9e9efb7b104e2520ea577db3eb2c13f9c16fb0cf90c1628ef31f5b1790",
    executed: false,
};

/// Both kext plists in the audio payload are this same text.
pub const MAC_PLIST_PATHS: &[&str] = &[
    "Line6AudioSigned.kext/Contents/Info.plist",
    "Line6AudioUnsigned.kext/Contents/Info.plist",
];

pub const MAC_PLIST: StaticText = StaticText {
    archive_id: "driver-macos-7.6.8",
    path: "Line6AudioSigned.kext/Contents/Info.plist",
    bytes: 47_343,
    md5: "889903adc3cb5db09c17a9b4be9316b1",
    sha256: "17f83de2fcfc818e7ea50699406fcbe7ab15ae52dd74db735a5f33e695c14b9a",
    executed: false,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogProbe {
    pub release_id: u32,
    pub version_label: &'static str,
    pub size_text: &'static str,
    pub page_url: &'static str,
    pub readeula_url: &'static str,
    pub readeula_result: &'static str,
    pub getrelease_url: &'static str,
    pub getrelease_result: &'static str,
    pub probed: &'static str,
    pub credentials: CredentialUse,
    pub hardware: HardwareContact,
    pub execution: Execution,
    /// Named in the v2.62 notes. No separate release id was on that listing.
    pub usb_firmware_label: &'static str,
    pub local_image: bool,
}

pub const FLASH: CatalogProbe = CatalogProbe {
    release_id: 6260,
    version_label: "2.62",
    size_text: "estimated 892 KB",
    page_url: provenance::HD500X_LISTING_PAGE,
    readeula_url: "https://line6.com/software/readeula.html?rid=6260",
    readeula_result: "HTTP 302 to /account/login.html?user_attempt=download, then HTTP 200 login HTML",
    getrelease_url: "https://line6.com/getrelease?rid=6260",
    getrelease_result: "HTTP 403",
    probed: "2026-11-16",
    credentials: CredentialUse::None,
    hardware: HardwareContact::None,
    execution: Execution::NotRun,
    usb_firmware_label: "1.04",
    local_image: false,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorListing {
    pub release_id: u32,
    pub platform: &'static str,
    pub size_text: &'static str,
    /// Sidebar text. Both cited rows print Version 2.27 and different dates.
    pub version_label: Option<&'static str>,
    /// Sidebar date, copied as printed. Not normalized.
    pub released_label: &'static str,
    pub downloaded: bool,
}

pub const EDITORS: &[EditorListing] = &[
    EditorListing {
        release_id: 9781,
        platform: "macOS listing",
        size_text: "estimated 35 MB",
        version_label: Some("2.27"),
        released_label: "8/4/20",
        downloaded: false,
    },
    EditorListing {
        release_id: 8559,
        platform: "Windows listing",
        size_text: "estimated 37 MB",
        version_label: Some("2.27"),
        released_label: "9/10/18",
        downloaded: false,
    },
];

/// Unauthenticated HEAD of one Edit 2.27 catalog link and of the CDN directory
/// that would hold a file if the release id were a public object name.
/// The catalog card does not publish a file name, so no object name was guessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorAcquisition {
    pub release_id: u32,
    pub request_method: &'static str,
    pub readeula_url: &'static str,
    pub readeula_status: u16,
    pub readeula_location: &'static str,
    pub readeula_date: &'static str,
    pub cdn_directory_url: &'static str,
    pub cdn_directory_status: u16,
    pub cdn_directory_date: &'static str,
    pub cdn_content_length: u64,
    pub followed_login: bool,
    pub credentials: CredentialUse,
    pub hardware: HardwareContact,
    pub execution: Execution,
    pub downloaded: bool,
}

pub const EDITOR_ACQUISITIONS: &[EditorAcquisition] = &[
    EditorAcquisition {
        release_id: 9781,
        request_method: "HEAD",
        readeula_url: "https://line6.com/software/readeula.html?rid=9781",
        readeula_status: 302,
        readeula_location: "/account/login.html?user_attempt=download",
        readeula_date: "Mon, 16 Nov 2026 04:19:45 GMT",
        cdn_directory_url: "https://l6c-acdn2.line6.net/media/software/9781/",
        cdn_directory_status: 404,
        cdn_directory_date: "Mon, 16 Nov 2026 04:19:47 GMT",
        cdn_content_length: 213,
        followed_login: false,
        credentials: CredentialUse::None,
        hardware: HardwareContact::None,
        execution: Execution::NotRun,
        downloaded: false,
    },
    EditorAcquisition {
        release_id: 8559,
        request_method: "HEAD",
        readeula_url: "https://line6.com/software/readeula.html?rid=8559",
        readeula_status: 302,
        readeula_location: "/account/login.html?user_attempt=download",
        readeula_date: "Mon, 16 Nov 2026 04:19:46 GMT",
        cdn_directory_url: "https://l6c-acdn2.line6.net/media/software/8559/",
        cdn_directory_status: 404,
        cdn_directory_date: "Mon, 16 Nov 2026 04:19:48 GMT",
        cdn_content_length: 213,
        followed_login: false,
        credentials: CredentialUse::None,
        hardware: HardwareContact::None,
        execution: Execution::NotRun,
        downloaded: false,
    },
];

/// Wayback CDX responses for the editor routes checked on 2026-11-16.
/// An empty JSON array is `[]` plus a newline, which curl reported as 3 bytes.
/// The second pattern does not match the manual URLs fetched directly below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaybackQuery {
    pub url_pattern: &'static str,
    pub http_status: u16,
    pub body_bytes: u64,
    pub rows: u32,
}

pub const EDITOR_WAYBACK: &[WaybackQuery] = &[
    WaybackQuery {
        url_pattern: "line6.com/software/readeula.html?rid=9781",
        http_status: 200,
        body_bytes: 3,
        rows: 0,
    },
    WaybackQuery {
        url_pattern: "line6.com/data/*HD500X*Edit*",
        http_status: 200,
        body_bytes: 3,
        rows: 0,
    },
];

/// A public Line 6 PDF. These are manuals, not the Edit 2.27 installer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicManual {
    pub id: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub pdf_version: &'static str,
    pub covers: &'static str,
    pub executed: bool,
}

pub const PUBLIC_MANUALS: &[PublicManual] = &[
    PublicManual {
        id: "edit-pilots-guide-rev-b",
        url: "https://line6.com/data/6/0a06434cb25651dddc9192349/application/pdf/POD%20HD500X%20Edit%20Pilot%27s%20Guide%20-%20English%20%28%20Rev%20B%20%29.pdf",
        bytes: 3_017_205,
        sha256: "853bdf6b5bf361b56a17e938e0b7d4f4e6140ecbb093b574977380b0c4e19cac",
        pdf_version: "1.7",
        covers: "POD HD500X Edit version 1.0 and Flash Memory version 1.0 or later",
        executed: false,
    },
    PublicManual {
        id: "edit-installer-guide-rev-a",
        url: "https://line6.com/data/6/0a06434c122752169db7bc869/application/pdf/POD%20HD500X%20Edit%20Installer%20Guide%20-%20English%20%28%20Rev%20A%20%29.pdf",
        bytes: 4_294_046,
        sha256: "7eef1aefbe5d16f30f04fabbc3c9cdc149854397b1f3a317bd2ea7ff855e5435",
        pdf_version: "1.6",
        covers: "Rev A installer guide, copyright 2013, with no Edit 2.27 label",
        executed: false,
    },
];

/// A file-name suffix printed in one manual. The two manuals do not agree.
/// `byte_layout_recovered` stays false: the suffix is not a container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintedFileName {
    pub manual_id: &'static str,
    pub role: &'static str,
    pub extension: &'static str,
    pub byte_layout_recovered: bool,
}

pub const PRINTED_FILE_NAMES: &[PrintedFileName] = &[
    PrintedFileName {
        manual_id: "edit-pilots-guide-rev-b",
        role: "preset",
        extension: ".5xe",
        byte_layout_recovered: false,
    },
    PrintedFileName {
        manual_id: "edit-pilots-guide-rev-b",
        role: "bundle",
        extension: ".5xb",
        byte_layout_recovered: false,
    },
    PrintedFileName {
        manual_id: "edit-pilots-guide-rev-b",
        role: "set list",
        extension: ".5xs",
        byte_layout_recovered: false,
    },
    PrintedFileName {
        manual_id: "edit-installer-guide-rev-a",
        role: "preset",
        extension: ".h5e",
        byte_layout_recovered: false,
    },
    PrintedFileName {
        manual_id: "edit-installer-guide-rev-a",
        role: "bundle",
        extension: ".h3b",
        byte_layout_recovered: false,
    },
    PrintedFileName {
        manual_id: "edit-installer-guide-rev-a",
        role: "set list",
        extension: ".h5s",
        byte_layout_recovered: false,
    },
];

/// Installer names printed by the 2013 installer guide. Neither file is local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintedInstallerName {
    pub manual_id: &'static str,
    pub name: &'static str,
    pub obtained: bool,
}

pub const PRINTED_INSTALLER_NAMES: &[PrintedInstallerName] = &[
    PrintedInstallerName {
        manual_id: "edit-installer-guide-rev-a",
        name: ".dmg",
        obtained: false,
    },
    PrintedInstallerName {
        manual_id: "edit-installer-guide-rev-a",
        name: "POD HD500X Edit.mpkg",
        obtained: false,
    },
];

/// Local installer archives searched as bytes. None contained these sequences.
/// The files were not executed, and tests do not reopen them.
pub const EDITOR_PLAINTEXT_NEEDLES: &[&str] = &[
    "HD500X",
    "9781",
    "8559",
    "POD HD500X Edit",
    "l6c-acdn",
    "media/software",
    ".5xe",
    ".h5e",
];

pub const LOCAL_INSTALLERS_SCANNED: &[&str] = &[
    "monkey-windows-1.77",
    "monkey-macos-1.78",
    "central-windows-1.00",
];

/// Local absence and the routes already requested. Wider sources stay unknown.
pub const EDITOR_ACQUISITION_LIMIT: &str = "No Edit 2.27 installer is available locally. The unauthenticated readeula HEAD requests redirected to login, and the release-id directory HEAD requests returned 404. A directory 404 can coexist with a public named object. An owner-supplied original installer would unblock this static-extraction approach. Wider availability and other independent sources remain unknown. This crate does not store credentials and does not retry the routes already requested or flash release 6260.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub usb_vendor_id: u16,
    pub usb_product_id: u16,
    pub hardware_id: &'static str,
    pub device_name: &'static str,
    pub driver_ver: &'static str,
    pub mac_vendor_id: u32,
    pub mac_product_id: u32,
    pub mac_personalities: &'static [MacPersonality],
    pub audio: AudioDefaults,
    pub windows_inf: StaticText,
    pub mac_plist: StaticText,
    pub flash: CatalogProbe,
    pub editors: &'static [EditorListing],
    pub editor_acquisitions: &'static [EditorAcquisition],
    pub manuals: &'static [PublicManual],
    pub printed_file_names: &'static [PrintedFileName],
    /// No HD500X file product id was recovered.
    pub file_product_id: Option<u32>,
    /// No firmware-container product id was recovered. The USB pid is not one.
    pub firmware_product_id: Option<u32>,
    pub preset_container_recovered: bool,
    pub model_ids_recovered: bool,
    pub effect_ids_recovered: bool,
    pub converts_other_family: bool,
    pub live_access: bool,
}

const RECORD: Record = Record {
    usb_vendor_id: USB_VENDOR_ID,
    usb_product_id: USB_PRODUCT_ID,
    hardware_id: HARDWARE_ID,
    device_name: DEVICE_NAME,
    driver_ver: DRIVER_VER,
    mac_vendor_id: MAC_VENDOR_ID,
    mac_product_id: MAC_PRODUCT_ID,
    mac_personalities: MAC_PERSONALITIES,
    audio: AUDIO_DEFAULTS,
    windows_inf: WINDOWS_INF,
    mac_plist: MAC_PLIST,
    flash: FLASH,
    editors: EDITORS,
    editor_acquisitions: EDITOR_ACQUISITIONS,
    manuals: PUBLIC_MANUALS,
    printed_file_names: PRINTED_FILE_NAMES,
    file_product_id: None,
    firmware_product_id: None,
    preset_container_recovered: false,
    model_ids_recovered: false,
    effect_ids_recovered: false,
    converts_other_family: false,
    live_access: false,
};

pub fn record() -> &'static Record {
    &RECORD
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OtherFamilyFile {
    Variax600,
    Variax500700,
    PodxtLive,
}

/// Classifies a file id that already belongs to another family.
///
/// `None` means this value is not one of those known file ids. It does not
/// mean the value is a POD HD500X file id.
pub fn other_family_file(id: u32) -> Option<OtherFamilyFile> {
    if id == VARIAX_300_600_PRODUCT_ID {
        Some(OtherFamilyFile::Variax600)
    } else if id == VARIAX_500_700_PRODUCT_ID {
        Some(OtherFamilyFile::Variax500700)
    } else if id == PODXT_LIVE_PRODUCT_ID {
        Some(OtherFamilyFile::PodxtLive)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMiss {
    DifferentExtension,
    InitialRig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_text_ids_stay_distinct_from_other_products_and_file_ids() {
        let record = record();
        assert_eq!(record.usb_vendor_id, 0x0e41);
        assert_eq!(u32::from(record.usb_vendor_id), record.mac_vendor_id);
        assert_eq!(u32::from(record.usb_product_id), record.mac_product_id);
        assert_eq!(record.mac_product_id, 16_729);
        assert_eq!(record.hardware_id, "USB\\VID_0E41&PID_4159");
        assert_eq!(
            u32::from(POD_HD500_USB_PRODUCT_ID),
            POD_HD500_MAC_PRODUCT_ID
        );
        assert_ne!(record.usb_product_id, POD_HD500_USB_PRODUCT_ID);
        assert!(
            USB_NEIGHBORS
                .iter()
                .all(|neighbor| neighbor.product_id != record.usb_product_id)
        );
        assert!(record.file_product_id.is_none());
        assert!(record.firmware_product_id.is_none());
        assert!(!record.preset_container_recovered);
        assert!(!record.model_ids_recovered);
        assert!(!record.effect_ids_recovered);
        assert!(!record.converts_other_family);
        assert!(!record.live_access);
        assert_eq!(
            other_family_file(PODXT_LIVE_PRODUCT_ID),
            Some(OtherFamilyFile::PodxtLive)
        );
        assert_eq!(
            other_family_file(VARIAX_300_600_PRODUCT_ID),
            Some(OtherFamilyFile::Variax600)
        );
        assert_eq!(
            other_family_file(VARIAX_500_700_PRODUCT_ID),
            Some(OtherFamilyFile::Variax500700)
        );
        assert!(other_family_file(u32::from(record.usb_product_id)).is_none());
        assert_ne!(u32::from(record.usb_product_id), PODXT_LIVE_PRODUCT_ID);
        let audio = record.mac_personalities[0];
        let bulk = record.mac_personalities[1];
        assert_eq!(audio.interface_number, Some(0));
        assert_eq!(bulk.interface_number, Some(1));
        assert_eq!(audio.configuration, Some(1));
        assert!(record.mac_personalities[2].configuration.is_none());
        assert!(record.mac_personalities[2].interface_number.is_none());
        assert!(record.mac_personalities[3].interface_number.is_none());
        assert!(!record.audio.stream_opened);
        assert!(!record.audio.format_negotiated);
        assert_eq!(record.audio.engine_format_lock, 0);
        assert_eq!(record.audio.default_sample_rate, 48_000);
        assert_eq!(record.audio.default_word_size, 24);
    }

    #[test]
    fn extracted_text_hashes_are_members_and_the_images_were_not_obtained() {
        provenance::validate_catalog().expect("catalog");
        let record = record();
        let windows = provenance::artifact_by_id(record.windows_inf.archive_id).expect("windows");
        let mac = provenance::artifact_by_id(record.mac_plist.archive_id).expect("mac");
        assert_ne!(record.windows_inf.sha256, windows.sha256);
        assert_ne!(record.windows_inf.bytes, windows.bytes);
        assert_ne!(record.mac_plist.sha256, mac.sha256);
        assert!(!record.windows_inf.executed);
        assert!(!record.mac_plist.executed);
        assert_eq!(MAC_PLIST_PATHS.len(), 2);
        assert_eq!(record.windows_inf.bytes, 10_484);
        assert_eq!(record.mac_plist.bytes, 47_343);
        let listing = provenance::public_listing(record.flash.release_id).expect("6260");
        assert_eq!(listing.version, record.flash.version_label);
        assert_eq!(listing.size_text, record.flash.size_text);
        assert_eq!(listing.page_url, record.flash.page_url);
        assert!(listing.local_artifact_id.is_none());
        assert!(!listing.catalog_md5_saved);
        assert!(!listing.exact_catalog_bytes_saved);
        assert!(!record.flash.local_image);
        assert_eq!(record.flash.credentials, CredentialUse::None);
        assert_eq!(record.flash.hardware, HardwareContact::None);
        assert!(!record.flash.execution.ran());
        assert_eq!(record.flash.usb_firmware_label, "1.04");
        assert_ne!(record.flash.usb_firmware_label, record.flash.version_label);
        assert!(record.editors.iter().all(|editor| !editor.downloaded));
        assert_eq!(
            record
                .editors
                .iter()
                .map(|editor| (
                    editor.release_id,
                    editor.version_label,
                    editor.released_label
                ))
                .collect::<Vec<_>>(),
            vec![
                (9781, Some("2.27"), "8/4/20"),
                (8559, Some("2.27"), "9/10/18"),
            ]
        );
        assert_eq!(listing.release_date, "4/9/15");
        assert!(listing.page_url.contains("submit_form=set"));
        assert!(listing.page_url.contains("submit_query=Show"));
        for id in [
            "hd500x-flash-2.62-not-obtained",
            "hd500x-editor-not-obtained",
            "hd500x-independent-bytes-are-not-a-product",
        ] {
            assert!(
                provenance::ACQUISITION_GAPS.iter().any(|gap| gap.id == id),
                "{id}"
            );
        }
        let evidence = include_str!("../docs/pod-hd500x.md");
        assert!(evidence.contains(record.windows_inf.sha256));
        assert!(evidence.contains(record.mac_plist.sha256));
        assert!(evidence.contains("HTTP 403"));
        assert!(evidence.contains("6260"));
        assert!(evidence.contains("submit_form=set"));
        assert!(evidence.contains("submit_query=Show"));
        assert!(
            evidence.contains("048b200988464d1e73942a36afe657e2af7e9aa933236b6000a452de48fe3657")
        );
        assert!(evidence.contains("4/9/15"));
        assert!(evidence.contains("2.62.00"));
        assert_eq!(record.editors.len(), record.editor_acquisitions.len());
        for editor in record.editors {
            let probe = record
                .editor_acquisitions
                .iter()
                .find(|probe| probe.release_id == editor.release_id)
                .expect("acquisition");
            assert_eq!(probe.request_method, "HEAD");
            assert_eq!(probe.readeula_status, 302);
            assert_eq!(
                probe.readeula_location,
                "/account/login.html?user_attempt=download"
            );
            assert!(probe.readeula_url.contains(&editor.release_id.to_string()));
            assert_eq!(probe.cdn_directory_status, 404);
            assert_eq!(probe.cdn_content_length, 213);
            assert!(!probe.followed_login);
            assert!(!probe.downloaded);
            assert_eq!(probe.credentials, CredentialUse::None);
            assert_eq!(probe.hardware, HardwareContact::None);
            assert!(!probe.execution.ran());
            assert!(evidence.contains(probe.readeula_date));
            assert!(evidence.contains(probe.cdn_directory_url));
        }
        assert!(EDITOR_WAYBACK.iter().all(|query| query.rows == 0));
        assert!(EDITOR_WAYBACK.iter().all(|query| query.body_bytes == 3));
        assert!(record.manuals.iter().all(|manual| !manual.executed));
        assert!(
            record
                .printed_file_names
                .iter()
                .all(|name| !name.byte_layout_recovered)
        );
        for role in ["preset", "bundle", "set list"] {
            let printed: Vec<_> = record
                .printed_file_names
                .iter()
                .filter(|name| name.role == role)
                .map(|name| name.extension)
                .collect();
            assert_eq!(printed.len(), 2);
            assert_ne!(printed[0], printed[1]);
        }
        assert!(PRINTED_INSTALLER_NAMES.iter().all(|name| !name.obtained));
        assert!(evidence.contains("POD HD500X Edit.mpkg"));
        for manual in record.manuals {
            assert!(is_lower_hex(manual.sha256, 64));
            assert!(evidence.contains(manual.sha256));
            assert!(evidence.contains(manual.url));
        }
        for id in LOCAL_INSTALLERS_SCANNED {
            let artifact = provenance::artifact_by_id(id).expect(id);
            assert!(!artifact.execution.ran());
            assert!(artifact.bytes > 0);
        }
        assert!(!EDITOR_PLAINTEXT_NEEDLES.is_empty());
        assert!(evidence.contains(EDITOR_ACQUISITION_LIMIT));
        let editor_gap = provenance::ACQUISITION_GAPS
            .iter()
            .find(|gap| gap.id == "hd500x-editor-not-obtained")
            .expect("editor gap");
        assert!(editor_gap.detail.contains("HTTP 302"));
        assert!(editor_gap.detail.contains("HTTP 404"));
    }

    fn is_lower_hex(value: &str, len: usize) -> bool {
        value.len() == len
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }
}

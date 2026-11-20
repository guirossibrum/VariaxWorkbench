#![forbid(unsafe_code)]
//! Sourced device inventory for the staged read-only probe.
//!
//! A row records recovered names and public ids. It does not open a device,
//! and membership is not permission to read or write settings.

use crate::rig::{CapabilityState, Operation, ProductProfile};
use std::fmt;

pub const SCHEMA: u32 = 2;
pub const COMMITTED_TABLE: &str = include_str!("../supported_devices.txt");

const SOURCE_TOKENS: &[&str] = &[
    "workbench-product-constants",
    "monkey-default-cfg",
    "committed-usb-record",
    "workbench-hd-static",
    "owner-name-list",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Bridge,
    Guitar,
    Product,
    Family,
    Alias,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    SourcedId,
    NameOnly,
    FamilyOnly,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    pub name: String,
    pub role: Role,
    pub usb_vid: Option<u16>,
    pub usb_pid: Option<u16>,
    pub l6_product: Option<u32>,
    pub engine_profile: Option<ProductProfile>,
    pub engine_operation: Option<Operation>,
    pub capability_state: Option<CapabilityState>,
    pub sources: Vec<String>,
    pub evidence: Evidence,
    pub owner_live_rig: bool,
    pub synthetic_fixture: String,
    pub gaps: Vec<String>,
}

impl DeviceRow {
    pub fn has_public_id(&self) -> bool {
        self.l6_product.is_some() || self.usb_pid.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameMatch<'a> {
    None,
    One(&'a DeviceRow),
    Ambiguous,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    rows: Vec<DeviceRow>,
}

impl Inventory {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        let schema = lines
            .next()
            .ok_or_else(|| "inventory is empty".to_string())?;
        let mut schema_fields = schema.split('\t');
        if schema_fields.next() != Some("schema")
            || schema_fields.next() != Some("2")
            || schema_fields.next().is_some()
        {
            return Err("inventory schema must be 2".into());
        }
        let header = lines
            .next()
            .ok_or_else(|| "inventory header is missing".to_string())?;
        if header.split('\t').collect::<Vec<_>>() != COLUMNS {
            return Err("inventory header does not match the reviewed columns".into());
        }
        let mut rows = Vec::new();
        for (index, line) in lines.enumerate() {
            rows.push(parse_row(index + 3, line)?);
        }
        if rows.is_empty() {
            return Err("inventory has no device rows".into());
        }
        Ok(Self { rows })
    }

    pub fn rows(&self) -> &[DeviceRow] {
        &self.rows
    }

    pub fn find_name(&self, name: &str) -> NameMatch<'_> {
        let mut found = None;
        for row in &self.rows {
            if row.name == name {
                if found.is_some() {
                    return NameMatch::Ambiguous;
                }
                found = Some(row);
            }
        }
        match found {
            Some(row) => NameMatch::One(row),
            None => NameMatch::None,
        }
    }

    pub fn find_usb(&self, vid: u16, pid: u16) -> NameMatch<'_> {
        let mut found = None;
        for row in &self.rows {
            if row.usb_vid == Some(vid) && row.usb_pid == Some(pid) {
                if found.is_some() {
                    return NameMatch::Ambiguous;
                }
                found = Some(row);
            }
        }
        match found {
            Some(row) => NameMatch::One(row),
            None => NameMatch::None,
        }
    }

    pub fn find_l6(&self, product: u32) -> NameMatch<'_> {
        let mut found = None;
        for row in &self.rows {
            if row.l6_product == Some(product) {
                if found.is_some() {
                    return NameMatch::Ambiguous;
                }
                found = Some(row);
            }
        }
        match found {
            Some(row) => NameMatch::One(row),
            None => NameMatch::None,
        }
    }
}

const COLUMNS: [&str; 13] = [
    "name",
    "role",
    "usb_vid",
    "usb_pid",
    "l6_product",
    "engine_profile",
    "engine_operation",
    "capability_state",
    "source",
    "evidence",
    "owner_live_rig",
    "synthetic_fixture",
    "gap",
];

fn parse_row(line_number: usize, line: &str) -> Result<DeviceRow, String> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != COLUMNS.len() {
        return Err(format!("line {line_number} does not have 13 fields"));
    }
    let name = required(line_number, "name", fields[0])?;
    let role = parse_role(line_number, fields[1])?;
    let usb_vid = optional_u16(line_number, "usb_vid", fields[2])?;
    let usb_pid = optional_u16(line_number, "usb_pid", fields[3])?;
    let l6_product = optional_hex(line_number, "l6_product", fields[4], 8)?;
    let engine_profile = optional_profile(line_number, fields[5])?;
    let engine_operation = optional_operation(line_number, fields[6])?;
    let capability_state = optional_capability(line_number, fields[7])?;
    let sources = parse_sources(line_number, fields[8])?;
    let evidence = parse_evidence(line_number, fields[9])?;
    let owner_live_rig = parse_yes_no(line_number, fields[10])?;
    let synthetic_fixture = required(line_number, "synthetic_fixture", fields[11])?;
    let gaps = split_tokens(line_number, "gap", fields[12])?;
    if usb_vid.is_some() && usb_pid.is_none() {
        return Err(format!(
            "line {line_number} has a USB vendor without a product"
        ));
    }
    let engine_parts = [
        engine_profile.is_some(),
        engine_operation.is_some(),
        capability_state.is_some(),
    ];
    if engine_parts.iter().any(|present| *present) && !engine_parts.iter().all(|present| *present) {
        return Err(format!(
            "line {line_number} must set the engine profile, operation, and capability together"
        ));
    }
    let row = DeviceRow {
        name,
        role,
        usb_vid,
        usb_pid,
        l6_product,
        engine_profile,
        engine_operation,
        capability_state,
        sources,
        evidence,
        owner_live_rig,
        synthetic_fixture,
        gaps,
    };
    check_role_shape(line_number, &row)?;
    Ok(row)
}

fn check_role_shape(line_number: usize, row: &DeviceRow) -> Result<(), String> {
    if row.role == Role::Alias {
        if row.has_public_id() || row.usb_vid.is_some() || row.engine_profile.is_some() {
            return Err(format!(
                "line {line_number} alias carries an id or engine profile"
            ));
        }
        if row.sources != ["owner-name-list"]
            || row.evidence != Evidence::NameOnly
            || row.owner_live_rig
        {
            return Err(format!(
                "line {line_number} alias is not a name-only owner entry"
            ));
        }
    }
    if row.evidence == Evidence::NameOnly && row.has_public_id() {
        return Err(format!("line {line_number} name-only row has an id"));
    }
    if row.evidence == Evidence::FamilyOnly && row.l6_product.is_none() {
        return Err(format!("line {line_number} family row has no product id"));
    }
    if matches!(row.evidence, Evidence::SourcedId | Evidence::Partial) && !row.has_public_id() {
        return Err(format!("line {line_number} sourced row has no public id"));
    }
    if row.has_public_id() && row.sources.iter().any(|source| source == "owner-name-list") {
        return Err(format!(
            "line {line_number} uses the owner name list as an id source"
        ));
    }
    Ok(())
}

fn required(line_number: usize, field: &str, value: &str) -> Result<String, String> {
    if value.is_empty() || value == "-" {
        return Err(format!("line {line_number} {field} is empty"));
    }
    if value.chars().any(|ch| ch.is_control()) {
        return Err(format!(
            "line {line_number} {field} contains a control character"
        ));
    }
    Ok(value.to_string())
}

fn optional_u16(line_number: usize, field: &str, value: &str) -> Result<Option<u16>, String> {
    optional_hex(line_number, field, value, 4)?
        .map(|value| {
            u16::try_from(value)
                .map_err(|_| format!("line {line_number} {field} does not fit in 16 bits"))
        })
        .transpose()
}

fn optional_hex(
    line_number: usize,
    field: &str,
    value: &str,
    digits: usize,
) -> Result<Option<u32>, String> {
    if value == "-" {
        return Ok(None);
    }
    let raw = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("line {line_number} {field} is not hexadecimal"))?;
    if raw.len() != digits || !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!(
            "line {line_number} {field} must be {digits} hex digits"
        ));
    }
    u32::from_str_radix(raw, 16)
        .map(Some)
        .map_err(|_| format!("line {line_number} {field} is not hexadecimal"))
}

fn parse_sources(line_number: usize, value: &str) -> Result<Vec<String>, String> {
    let sources = split_tokens(line_number, "source", value)?;
    for source in &sources {
        if !SOURCE_TOKENS.contains(&source.as_str()) {
            return Err(format!("line {line_number} has unknown source {source}"));
        }
    }
    let mut ordered = sources.clone();
    ordered.sort();
    if ordered != sources {
        return Err(format!("line {line_number} sources are not sorted"));
    }
    Ok(sources)
}

fn split_tokens(line_number: usize, field: &str, value: &str) -> Result<Vec<String>, String> {
    if value.is_empty() || value == "-" {
        return Err(format!("line {line_number} {field} is empty"));
    }
    let tokens: Vec<String> = value.split(['+', ',']).map(str::to_string).collect();
    if tokens.iter().any(|token| token.is_empty())
        || tokens
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != tokens.len()
    {
        return Err(format!(
            "line {line_number} {field} has an empty or repeated token"
        ));
    }
    Ok(tokens)
}

fn parse_yes_no(line_number: usize, value: &str) -> Result<bool, String> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!(
            "line {line_number} owner_live_rig must be yes or no"
        )),
    }
}

fn parse_role(line_number: usize, value: &str) -> Result<Role, String> {
    match value {
        "bridge" => Ok(Role::Bridge),
        "guitar" => Ok(Role::Guitar),
        "product" => Ok(Role::Product),
        "family" => Ok(Role::Family),
        "alias" => Ok(Role::Alias),
        _ => Err(format!("line {line_number} has unknown role {value}")),
    }
}

fn parse_evidence(line_number: usize, value: &str) -> Result<Evidence, String> {
    match value {
        "sourced-id" => Ok(Evidence::SourcedId),
        "name-only" => Ok(Evidence::NameOnly),
        "family-only" => Ok(Evidence::FamilyOnly),
        "partial" => Ok(Evidence::Partial),
        _ => Err(format!("line {line_number} has unknown evidence {value}")),
    }
}

fn optional_profile(line_number: usize, value: &str) -> Result<Option<ProductProfile>, String> {
    match value {
        "-" | "none" => Ok(None),
        "Variax600" => Ok(Some(ProductProfile::Variax600)),
        "PodxtLive" => Ok(Some(ProductProfile::PodxtLive)),
        "WorkbenchHd" => Ok(Some(ProductProfile::WorkbenchHd)),
        "Jtv69" => Ok(Some(ProductProfile::Jtv69)),
        "PodHd500x" => Ok(Some(ProductProfile::PodHd500x)),
        _ => Err(format!(
            "line {line_number} has unknown engine profile {value}"
        )),
    }
}

fn optional_operation(line_number: usize, value: &str) -> Result<Option<Operation>, String> {
    let operation = match value {
        "-" | "none" => return Ok(None),
        "OpenGuitarPreset" => Operation::OpenGuitarPreset,
        "OpenGuitarBank" => Operation::OpenGuitarBank,
        "SaveGuitarCopy" => Operation::SaveGuitarCopy,
        "ApplyFamilyMath" => Operation::ApplyFamilyMath,
        "OpenPodPreset" => Operation::OpenPodPreset,
        "EditPodPreset" => Operation::EditPodPreset,
        "SelectRecordRoute" => Operation::SelectRecordRoute,
        "InspectFirmware" => Operation::InspectFirmware,
        "StartAudioCapture" => Operation::StartAudioCapture,
        "StopAudioCapture" => Operation::StopAudioCapture,
        "OpenAudioStream" => Operation::OpenAudioStream,
        "ReadInstalledUsbComponent" => Operation::ReadInstalledUsbComponent,
        "FlashFirmware" => Operation::FlashFirmware,
        "ResetDevice" => Operation::ResetDevice,
        "SwitchBank" => Operation::SwitchBank,
        "EnumerateUsb" => Operation::EnumerateUsb,
        "OpenMidi" => Operation::OpenMidi,
        "SendControl" => Operation::SendControl,
        "InstallModule" => Operation::InstallModule,
        "BindDriver" => Operation::BindDriver,
        _ => {
            return Err(format!(
                "line {line_number} has unknown engine operation {value}"
            ));
        }
    };
    Ok(Some(operation))
}

fn optional_capability(line_number: usize, value: &str) -> Result<Option<CapabilityState>, String> {
    match value {
        "-" | "none" => Ok(None),
        "Offline" => Ok(Some(CapabilityState::Offline)),
        "Locked" => Ok(Some(CapabilityState::Locked)),
        "Unverified" => Ok(Some(CapabilityState::Unverified)),
        "Unsupported" => Ok(Some(CapabilityState::Unsupported)),
        _ => Err(format!(
            "line {line_number} has unknown capability state {value}"
        )),
    }
}

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bridge => "bridge",
            Self::Guitar => "guitar",
            Self::Product => "product",
            Self::Family => "family",
            Self::Alias => "alias",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firmware::{
        PODXT_LIVE_PRODUCT_ID, VARIAX_300_600_PRODUCT_ID, VARIAX_500_700_PRODUCT_ID,
    };
    use crate::linux_transport::{POD_PODXT_LIVE, VARIAX_PODXT_LIVE, VARIAX_WORKBENCH};
    use crate::pod_hd500x::{self, USB_NEIGHBORS, USB_PRODUCT_ID, USB_VENDOR_ID};
    use crate::rig::resolve;

    fn table() -> Inventory {
        Inventory::parse(COMMITTED_TABLE).expect("committed inventory")
    }

    #[test]
    fn committed_table_matches_the_existing_engine_records() {
        let inventory = table();
        let pod = expect_one(inventory.find_name("PODxt Live"));
        assert_eq!(pod.role, Role::Bridge);
        assert_eq!(pod.usb_vid, Some(VARIAX_PODXT_LIVE.vendor_id));
        assert_eq!(pod.usb_pid, Some(VARIAX_PODXT_LIVE.product_id));
        assert_eq!(pod.usb_vid, Some(POD_PODXT_LIVE.vendor_id));
        assert_eq!(pod.usb_pid, Some(POD_PODXT_LIVE.product_id));
        assert_eq!(pod.l6_product, Some(PODXT_LIVE_PRODUCT_ID));
        assert_eq!(pod.engine_profile, Some(ProductProfile::PodxtLive));
        assert_eq!(pod.capability_state, Some(CapabilityState::Locked));
        assert_eq!(
            resolve(
                ProductProfile::PodxtLive,
                Operation::EnumerateUsb,
                Some(CapabilityState::Offline)
            ),
            CapabilityState::Locked
        );
        assert!(pod.owner_live_rig);
        assert!(
            pod.gaps
                .iter()
                .any(|gap| gap == "read-requires-excluded-transfer")
        );
        assert!(pod.gaps.iter().any(|gap| gap == "not-a-backup"));

        let guitar = expect_one(inventory.find_name("Variax 300/600"));
        assert_eq!(guitar.role, Role::Guitar);
        assert_eq!(guitar.l6_product, Some(VARIAX_300_600_PRODUCT_ID));
        assert_eq!(
            guitar.l6_product,
            ProductProfile::Variax600.archived_firmware_product()
        );
        assert_eq!(guitar.engine_profile, Some(ProductProfile::Variax600));
        assert!(guitar.owner_live_rig);
        assert_eq!(
            resolve(ProductProfile::Variax600, Operation::EnumerateUsb, None),
            CapabilityState::Locked
        );

        let alias = expect_one(inventory.find_name("Variax 600"));
        assert_eq!(alias.role, Role::Alias);
        assert!(!alias.has_public_id());
        assert!(!alias.owner_live_rig);

        let other = expect_one(inventory.find_l6(VARIAX_500_700_PRODUCT_ID));
        assert_eq!(other.name, "Variax 500/700");
        assert_ne!(other.engine_profile, Some(ProductProfile::Variax600));

        let interface =
            expect_one(inventory.find_usb(VARIAX_WORKBENCH.vendor_id, VARIAX_WORKBENCH.product_id));
        assert_eq!(interface.name, "Variax USB Interface");
        assert_eq!(interface.role, Role::Bridge);
        assert_ne!(interface.usb_pid, Some(VARIAX_PODXT_LIVE.product_id));

        let hd = expect_one(inventory.find_usb(USB_VENDOR_ID, USB_PRODUCT_ID));
        assert_eq!(hd.name, pod_hd500x::DEVICE_NAME);
        assert_eq!(hd.engine_profile, Some(ProductProfile::PodHd500x));
        assert_eq!(hd.capability_state, Some(CapabilityState::Unverified));
        for neighbor in USB_NEIGHBORS {
            let row = expect_one(inventory.find_name(neighbor.name));
            assert_eq!(row.usb_pid, Some(neighbor.product_id));
            assert_eq!(row.usb_vid, Some(USB_VENDOR_ID));
            assert_ne!(row.engine_profile, Some(ProductProfile::PodHd500x));
        }
    }

    #[test]
    fn family_gate_does_not_select_jtv_69_and_aliases_do_not_authorize() {
        let inventory = table();
        let jtv = expect_one(inventory.find_name("JTV-69"));
        assert_eq!(jtv.l6_product, None);
        assert_eq!(jtv.engine_profile, Some(ProductProfile::Jtv69));
        assert!(jtv.gaps.iter().any(|gap| gap == "family-id-not-this-model"));
        let family = expect_one(inventory.find_l6(0x0007_0005));
        assert_eq!(family.name, "Tyler Variax");
        assert_eq!(family.role, Role::Family);
        assert_ne!(family.name, "JTV-69");

        let mut owners = Vec::new();
        let mut l6_ids = std::collections::BTreeSet::new();
        let mut usb_pairs = std::collections::BTreeSet::new();
        for row in inventory.rows() {
            if row.owner_live_rig {
                owners.push(row.name.as_str());
            }
            if let Some(product) = row.l6_product {
                assert!(l6_ids.insert(product), "repeated product id {product:#x}");
            }
            if let (Some(vid), Some(pid)) = (row.usb_vid, row.usb_pid) {
                assert!(
                    usb_pairs.insert((vid, pid)),
                    "repeated USB id {vid:#x}:{pid:#x}"
                );
            }
            if let (Some(profile), Some(operation), Some(state)) = (
                row.engine_profile,
                row.engine_operation,
                row.capability_state,
            ) {
                assert_eq!(
                    resolve(profile, operation, Some(CapabilityState::Offline)),
                    state
                );
            }
            if row.role == Role::Alias {
                assert!(!row.has_public_id());
                assert_eq!(row.gaps, ["alias-not-an-id"]);
            }
        }
        owners.sort_unstable();
        assert_eq!(owners, ["PODxt Live", "Variax 300/600"]);
        assert!(matches!(
            inventory.find_name("TonePort"),
            NameMatch::Ambiguous
        ));
        assert!(matches!(
            inventory.find_name("Not A Device"),
            NameMatch::None
        ));
        assert_eq!(inventory.rows().len(), 224);
    }

    fn expect_one(hit: NameMatch<'_>) -> &DeviceRow {
        match hit {
            NameMatch::One(row) => row,
            NameMatch::None => panic!("missing inventory row"),
            NameMatch::Ambiguous => panic!("ambiguous inventory row"),
        }
    }
}

#![forbid(unsafe_code)]
//! File-only reference extraction for one fingerprinted Variax 300/600 archive.
//!
//! Firmware defaults are not this guitar's shipping state or unique calibration.
//! No transport, firmware installation, restoration or device API exists here.

use crate::firmware::{self, Inspection, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION};
use crate::{PresetMetadata, preset::PresetFile};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const SOURCE_SHA256: &str = "e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a";
pub const PROGRAM_BYTES: usize = 3276;
pub const PREFIX_BYTES: usize = 2619;
pub const SLOT_BYTES: usize = 4096;
const NAME_OFFSET: usize = 2553;

#[derive(Debug, Clone)]
pub struct Template {
    pub index: usize,
    pub file_offset: usize,
    pub name: String,
    raw: Vec<u8>,
}
impl Template {
    pub fn raw_prefix(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub index: usize,
    pub file_offset: usize,
    pub metadata: PresetMetadata,
    /// A prefix match alone does not establish equality of the complete program.
    pub matching_template_indices: Vec<usize>,
    pub matching_expanded_template_indices: Vec<usize>,
    raw: Vec<u8>,
}
impl Slot {
    pub fn raw_slot(&self) -> &[u8] {
        &self.raw
    }
    pub fn raw_program(&self) -> &[u8] {
        &self.raw[..PROGRAM_BYTES]
    }
    /// Settings decoded from the firmware slot, not a complete hardware backup.
    pub fn settings_file(&self) -> Result<PresetFile, String> {
        PresetFile::from_metadata(
            &self.metadata,
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
        )
    }
}

#[derive(Debug, Clone)]
pub struct FactoryReference {
    source_sha256: [u8; 32],
    image_sha256: [u8; 32],
    templates: Vec<Template>,
    slots: Vec<Slot>,
}

impl FactoryReference {
    pub fn templates(&self) -> &[Template] {
        &self.templates
    }
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
    pub fn source_sha256(&self) -> &[u8; 32] {
        &self.source_sha256
    }
    pub fn image_sha256(&self) -> &[u8; 32] {
        &self.image_sha256
    }

    /// Requires byte-for-byte agreement with the locally archived source hash.
    /// The fingerprint establishes file identity, not publisher authentication.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let inspection = firmware::inspect(bytes).map_err(|e| e.to_string())?;
        inspection
            .require_target(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION)
            .map_err(|e| e.to_string())?;
        if hex(&inspection.report.source_sha256) != SOURCE_SHA256 {
            return Err("factory profile requires the exact archived source SHA-256; matching declarations are insufficient".into());
        }
        parse_layout(&inspection)
    }

    /// Creates a new directory. A partial failure leaves its evidence in place;
    /// manifest.tsv is written last and marks a complete extraction. No existing
    /// directory or file is replaced. Outputs are local forensic/reference files.
    pub fn export_new(&self, directory: &Path) -> Result<(), String> {
        // Validate every derived settings file before creating the destination.
        let settings: Vec<_> = self
            .slots
            .iter()
            .map(Slot::settings_file)
            .collect::<Result<_, _>>()?;
        std::fs::create_dir(directory)
            .map_err(|e| format!("cannot create new reference directory: {e}"))?;
        let mut manifest = format!(
            "# Source SHA-256: {}\n# Image SHA-256: {}\n# Firmware 3.10 defaults; not original shipping state, current-unit backup, calibration or restoration instructions.\n# Fingerprint and checksums are not publisher authentication. File offsets are not physical flash addresses.\nkind\tindex\tfile_offset\tbytes\tsha256\tname\n",
            hex(&self.source_sha256),
            hex(&self.image_sha256)
        );
        for t in &self.templates {
            crate::document::save_new(
                &directory.join(format!("template-{:02}.u24le", t.index)),
                &t.raw,
            )?;
            manifest.push_str(&format!(
                "template\t{}\t{}\t{}\t{}\t{}\n",
                t.index,
                t.file_offset,
                t.raw.len(),
                hex(&Sha256::digest(&t.raw)),
                t.name
            ));
        }
        for (s, preset) in self.slots.iter().zip(settings) {
            crate::document::save_new(
                &directory.join(format!("slot-{:02}.u24le", s.index)),
                &s.raw,
            )?;
            crate::document::save_new(
                &directory.join(format!("slot-{:02}.vwm", s.index)),
                preset.as_bytes(),
            )?;
            manifest.push_str(&format!(
                "slot\t{}\t{}\t{}\t{}\t{}\n",
                s.index,
                s.file_offset,
                s.raw.len(),
                hex(&Sha256::digest(&s.raw)),
                s.metadata.name
            ));
            manifest.push_str(&format!(
                "settings\t{}\tderived\t{}\t{}\t{}\n",
                s.index,
                preset.as_bytes().len(),
                hex(&Sha256::digest(preset.as_bytes())),
                s.metadata.name
            ));
        }
        crate::document::save_new(&directory.join("manifest.tsv"), manifest.as_bytes())
    }
}

fn parse_layout(inspection: &Inspection) -> Result<FactoryReference, String> {
    let regions = &inspection.report.regions;
    let expected = [
        (5, 0, 16384),
        (0, 16384, 49308),
        (2, 65692, 130950),
        (1, 196642, 245760),
    ];
    if regions.len() != 4
        || regions.iter().zip(expected).any(|(r, (tag, offset, len))| {
            r.type_raw != tag
                || r.offset != offset
                || r.len != len
                || r.version != VARIAX_300_600_VERSION
        })
    {
        return Err("unexpected archived Variax 3.10 region layout".into());
    }
    let image = inspection.image();
    let template_start = regions[2].offset as usize;
    let slot_start = regions[3].offset as usize;
    let mut templates = Vec::new();
    for index in 0..50 {
        let file_offset = template_start + index * PREFIX_BYTES;
        let raw = image[file_offset..file_offset + PREFIX_BYTES].to_vec();
        let canonical = canonical_words(&raw)?;
        let name = read_name(&canonical[NAME_OFFSET..NAME_OFFSET + 18])?;
        templates.push(Template {
            index,
            file_offset,
            name,
            raw,
        });
    }
    let mut slots = Vec::new();
    for index in 0..60 {
        let file_offset = slot_start + index * SLOT_BYTES;
        let raw = image[file_offset..file_offset + SLOT_BYTES].to_vec();
        if raw[PROGRAM_BYTES..].iter().any(|&v| v != 255) {
            return Err(format!("slot {index} has unexpected padding"));
        }
        let canonical = canonical_words(&raw[..PROGRAM_BYTES])?;
        let checksum = sum_words(&canonical[..1089 * 3]);
        if canonical[3273..] != checksum.to_be_bytes()[1..] {
            return Err(format!("slot {index} checksum mismatch"));
        }
        read_name(&canonical[NAME_OFFSET..NAME_OFFSET + 18])?;
        let metadata = PresetMetadata::from_bytes(
            canonical[NAME_OFFSET..NAME_OFFSET + 192]
                .try_into()
                .expect("bounded metadata"),
        )?;
        let matching_template_indices = templates
            .iter()
            .filter(|t| raw[..PREFIX_BYTES] == t.raw)
            .map(|t| t.index)
            .collect();
        let matching_expanded_template_indices = templates
            .iter()
            .filter(|t| {
                if raw[..PREFIX_BYTES] != t.raw || raw[PREFIX_BYTES..3273].iter().any(|&v| v != 0) {
                    return false;
                }
                // The raw slot checksum is already validated above.
                true
            })
            .map(|t| t.index)
            .collect();
        slots.push(Slot {
            index,
            file_offset,
            metadata,
            matching_template_indices,
            matching_expanded_template_indices,
            raw,
        });
    }
    Ok(FactoryReference {
        source_sha256: inspection.report.source_sha256,
        image_sha256: inspection.report.image_sha256,
        templates,
        slots,
    })
}

fn canonical_words(raw: &[u8]) -> Result<Vec<u8>, String> {
    if !raw.len().is_multiple_of(3) {
        return Err("partial 24-bit word".into());
    }
    Ok(raw
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|w| [w[2], w[1], w[0]])
        .collect())
}
fn sum_words(canonical: &[u8]) -> u32 {
    canonical.as_chunks::<3>().0.iter().fold(0u32, |sum, w| {
        sum.wrapping_add(u32::from_be_bytes([0, w[0], w[1], w[2]]))
    }) & 0xffffff
}
fn read_name(raw: &[u8]) -> Result<String, String> {
    if raw.iter().any(|b| !(32..=126).contains(b)) {
        return Err("unexpected nonprintable reference name".into());
    }
    Ok(String::from_utf8(raw.to_vec())
        .expect("ASCII")
        .trim_end_matches(' ')
        .to_owned())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firmware::synthetic::{RegionSpec, Spec, build};

    fn synthetic_image() -> Vec<u8> {
        let mut image = vec![0; 442402];
        for index in 0..50 {
            let mut canonical = vec![0; PREFIX_BYTES];
            canonical[..3].copy_from_slice(&(index as u32).to_be_bytes()[1..]);
            let name = format!("Synthetic {index:<8}");
            canonical[NAME_OFFSET..NAME_OFFSET + 18].copy_from_slice(name.as_bytes());
            canonical[2571..2575].copy_from_slice(&0x03100002u32.to_be_bytes());
            image[65692 + index * PREFIX_BYTES..65692 + (index + 1) * PREFIX_BYTES]
                .copy_from_slice(&canonical_words(&canonical).unwrap());
        }
        for index in 0..60 {
            let prefix = image
                [65692 + (index % 50) * PREFIX_BYTES..65692 + (index % 50 + 1) * PREFIX_BYTES]
                .to_vec();
            let start = 196642 + index * SLOT_BYTES;
            image[start..start + PREFIX_BYTES].copy_from_slice(&prefix);
            let checksum = sum_words(&canonical_words(&prefix).unwrap());
            image[start + 3273..start + 3276].copy_from_slice(&checksum.to_le_bytes()[..3]);
            image[start + 3276..start + SLOT_BYTES].fill(255);
        }
        image
    }
    fn inspection(image: &[u8]) -> Inspection {
        let mut spec = Spec::basic(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION, image);
        spec.regions = Some(
            [
                (5, 0, 16384),
                (0, 16384, 49308),
                (2, 65692, 130950),
                (1, 196642, 245760),
            ]
            .into_iter()
            .map(|(type_raw, offset, size)| RegionSpec {
                type_raw,
                version: VARIAX_300_600_VERSION,
                offset,
                size,
            })
            .collect(),
        );
        firmware::inspect(&build(&spec)).unwrap()
    }
    #[test]
    fn declarations_and_shape_cannot_substitute_for_archived_fingerprint() {
        let image = synthetic_image();
        let mut spec = Spec::basic(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION, &image);
        let bytes = build(&spec);
        assert!(
            FactoryReference::parse(&bytes)
                .unwrap_err()
                .contains("SHA-256")
        );
        spec.product = firmware::PODXT_LIVE_PRODUCT_ID;
        assert!(
            FactoryReference::parse(&build(&spec))
                .unwrap_err()
                .contains("product")
        );
    }
    #[test]
    fn slots_preserve_full_raw_bytes_offsets_metadata_and_checksums() {
        let image = synthetic_image();
        let reference = parse_layout(&inspection(&image)).unwrap();
        assert_eq!(reference.templates.len(), 50);
        assert_eq!(reference.slots.len(), 60);
        for slot in &reference.slots {
            assert_eq!(
                slot.raw_slot(),
                &image[slot.file_offset..slot.file_offset + SLOT_BYTES]
            );
            assert_eq!(slot.raw_program().len(), PROGRAM_BYTES);
            assert_eq!(slot.matching_template_indices, [slot.index % 50]);
            assert_eq!(slot.matching_expanded_template_indices, [slot.index % 50]);
            assert_eq!(slot.settings_file().unwrap().name(), slot.metadata.name);
            assert_eq!(
                slot.metadata.parameters["body_model"],
                crate::ParameterValue::Integer(0x03100002)
            );
        }
    }
    #[test]
    fn metadata_extension_is_distinct_from_compact_template_equality() {
        let mut image = synthetic_image();
        let start = 196642;
        image[start + PREFIX_BYTES + 3] = 1;
        // Independent LE24 sum of exactly 1089 words, then a non-covered word.
        let sum = (0..1089)
            .map(|i| {
                u32::from_le_bytes([
                    image[start + i * 3],
                    image[start + i * 3 + 1],
                    image[start + i * 3 + 2],
                    0,
                ])
            })
            .fold(0u32, u32::wrapping_add)
            & 0xffffff;
        image[start + 3270..start + 3273].copy_from_slice(&[7, 0, 0]);
        image[start + 3273..start + 3276].copy_from_slice(&sum.to_le_bytes()[..3]);
        let reference = parse_layout(&inspection(&image)).unwrap();
        assert_eq!(reference.slots[0].matching_template_indices, [0]);
        assert!(
            reference.slots[0]
                .matching_expanded_template_indices
                .is_empty()
        );
        assert_eq!(reference.slots[0].raw_program()[3270], 7);
    }
    #[test]
    fn rewrapped_corruption_and_unexpected_shapes_are_rejected() {
        let original = synthetic_image();
        for (at, value) in [(196642, 128), (196642 + 3276, 0), (65692 + 2553, 0)] {
            let mut image = original.clone();
            image[at] = value;
            assert!(parse_layout(&inspection(&image)).is_err());
        }
        let mut wrong = inspection(&original);
        wrong.report.regions[2].len -= 3;
        assert!(parse_layout(&wrong).is_err());
    }
    #[test]
    fn exports_create_one_new_directory_and_never_replace_existing_evidence() {
        let reference = parse_layout(&inspection(&synthetic_image())).unwrap();
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("reference");
        reference.export_new(&destination).unwrap();
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 171);
        let manifest = std::fs::read(destination.join("manifest.tsv")).unwrap();
        assert!(String::from_utf8_lossy(&manifest).contains("not original shipping state"));
        assert_eq!(
            std::fs::read(destination.join("slot-00.u24le")).unwrap(),
            reference.slots[0].raw_slot()
        );
        assert!(reference.export_new(&destination).is_err());
        assert_eq!(
            std::fs::read(destination.join("manifest.tsv")).unwrap(),
            manifest
        );
        let empty = parent.path().join("already-exists");
        std::fs::create_dir(&empty).unwrap();
        assert!(reference.export_new(&empty).is_err());
        assert_eq!(std::fs::read_dir(&empty).unwrap().count(), 0);
    }
}

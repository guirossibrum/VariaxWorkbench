//! Offline lossless Workbench bank and collection serialization and edits.
use crate::preset::{PresetFile, chunks, container, legacy_text_issue, unique, word};
use std::ops::Range;

#[cfg(any(feature = "usb", feature = "gui"))]
mod backup_import;
#[cfg(any(feature = "usb", feature = "gui"))]
pub use backup_import::BackupBundle;

#[derive(Clone, Debug)]
pub struct Bank {
    pub name: String,
    pub fixed: bool,
    presets: Vec<PresetFile>,
    ranges: Vec<Range<usize>>,
    chunk_start: usize,
    name_offset: usize,
    data_length: usize,
}
impl Bank {
    pub fn presets(&self) -> &[PresetFile] {
        &self.presets
    }
}

#[derive(Clone, Debug)]
pub struct BundleFile {
    bytes: Vec<u8>,
    pub fixed: bool,
    banks: Vec<Bank>,
}

const DEFAULT_BANKS: [&str; 12] = [
    "Custom 1", "T-Model", "Spank", "Lester", "Special", "R-Billy", "Chime", "Semi", "Jazzbox",
    "Acoustic", "Reso", "Custom 2",
];

fn bank_name_field(name: &str) -> [u8; 64] {
    let mut bytes = [0; 64];
    for (character, dest) in name.encode_utf16().zip(bytes.as_chunks_mut::<2>().0) {
        *dest = character.to_be_bytes();
    }
    bytes
}

fn write_chunk(bytes: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) -> Result<(), String> {
    let size = u32::try_from(payload.len()).map_err(|_| "bundle exceeds IFF size capacity")?;
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(&size.to_be_bytes());
    bytes.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        bytes.push(0);
    }
    Ok(())
}

impl BundleFile {
    /// Workbench's fixed Variax template has 12 banks with 5 default models each.
    pub fn new_default_bundle() -> Result<Self, String> {
        Self::new_default_bundle_with_author("")
    }

    pub fn new_default_bundle_with_author(author: &str) -> Result<Self, String> {
        let model = PresetFile::default_legacy_model_with_author(author)?;
        let models = [&model; 5];
        Self::create(true, DEFAULT_BANKS.map(|name| (name, models)))
    }

    #[cfg(feature = "gui")]
    pub fn from_variax_factory(
        reference: &crate::factory_reference::FactoryReference,
    ) -> Result<Self, String> {
        let models = reference
            .slots()
            .iter()
            .map(|slot| slot.settings_file())
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_factory_slots(&models)
    }

    #[cfg(feature = "gui")]
    fn from_factory_slots(models: &[PresetFile]) -> Result<Self, String> {
        let (banks, remainder) = models.as_chunks::<5>();
        if !remainder.is_empty() || banks.len() != DEFAULT_BANKS.len() {
            return Err("The Variax factory archive must contain all 60 presets.".into());
        }
        // The physical selector runs from flash slot 4 to 0 in each bank.
        // Workbench and the backup picker show the selector order.
        Self::create(
            true,
            DEFAULT_BANKS
                .into_iter()
                .zip(banks.iter().map(|bank| bank.iter().rev())),
        )
    }

    /// An editable collection begins with Workbench's empty "New Folder".
    pub fn new_collection() -> Result<Self, String> {
        Self::create(false, [("New Folder", [])])
    }

    fn create<'a, Models: IntoIterator<Item = &'a PresetFile>>(
        fixed: bool,
        banks: impl IntoIterator<Item = (&'a str, Models)>,
    ) -> Result<Self, String> {
        let mut payload = if fixed { b"L6BA" } else { b"L6CO" }.to_vec();
        write_chunk(&mut payload, b"HEAD", &1u32.to_be_bytes())?;
        for (name, models) in banks {
            let mut bank = if fixed { b"BANK" } else { b"FLDR" }.to_vec();
            let mut info = vec![0; 4];
            info.extend_from_slice(&bank_name_field(name));
            write_chunk(&mut bank, b"BINF", &info)?;
            for model in models {
                bank.extend_from_slice(model.as_bytes());
            }
            write_chunk(&mut payload, b"LIST", &bank)?;
        }
        let mut bytes = Vec::new();
        write_chunk(&mut bytes, b"FORM", &payload)?;
        Self::parse(bytes)
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 12 || &bytes[..4] != b"FORM" || !matches!(&bytes[8..12], b"L6BA" | b"L6CO")
        {
            return Err("expected a Workbench FORM/L6BA bundle or FORM/L6CO collection".into());
        }
        if word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err("bundle FORM length does not match the file".into());
        }
        let fixed = &bytes[8..12] == b"L6BA";
        let mut banks = Vec::new();
        for bank in chunks(&bytes, 12..bytes.len())? {
            let bank_fixed = container(&bytes, &bank, b"BANK");
            if !bank_fixed && !container(&bytes, &bank, b"FLDR") {
                continue;
            }
            let children = chunks(&bytes, bank.data.start + 4..bank.data.end)?;
            let info = unique(&children, b"BINF")?;
            if info.data.len() < 68 {
                return Err("truncated bank information".into());
            }
            let units: Vec<_> = bytes[info.data.start + 4..info.data.start + 68]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_be_bytes(*b))
                .take_while(|&c| c != 0)
                .collect();
            let name = String::from_utf16(&units)
                .map_err(|_| "invalid UTF-16 bank name")?
                .trim_end()
                .to_owned();
            let mut presets = Vec::new();
            let mut ranges = Vec::new();
            // The Workbench IFF map treats L6PA, L6FS and L6AS as L6Patch
            // entries. Preserve their concrete FORM type in PresetFile bytes.
            for patch in children.iter().filter(|c| {
                container(&bytes, c, b"L6PA")
                    || container(&bytes, c, b"L6FS")
                    || container(&bytes, c, b"L6AS")
            }) {
                let range = patch.data.start - 8..patch.data.end;
                presets.push(PresetFile::parse(bytes[range.clone()].to_vec())?);
                ranges.push(range);
            }
            banks.push(Bank {
                name,
                fixed: bank_fixed,
                presets,
                ranges,
                chunk_start: bank.data.start - 8,
                name_offset: info.data.start + 4,
                data_length: bank.data.len(),
            });
        }
        Ok(Self {
            bytes,
            fixed,
            banks,
        })
    }
    pub fn banks(&self) -> &[Bank] {
        &self.banks
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_empty_collection(&self) -> bool {
        !self.fixed
            && !self.banks.is_empty()
            && self
                .banks
                .iter()
                .all(|bank| !bank.fixed && bank.presets.is_empty())
    }
    pub fn summary(&self) -> String {
        use std::fmt::Write;
        let mut out = format!(
            "bundle\t{}\t{} banks\n",
            if self.fixed { "fixed" } else { "collection" },
            self.banks.len()
        );
        for (i, bank) in self.banks.iter().enumerate() {
            writeln!(
                out,
                "bank\t{i}\t{}\t{} presets",
                bank.name,
                bank.presets.len()
            )
            .unwrap();
            for (j, preset) in bank.presets.iter().enumerate() {
                writeln!(
                    out,
                    "preset\t{i}\t{j}\t{}\t{:08x}",
                    preset.name(),
                    preset.device_id()
                )
                .unwrap();
            }
        }
        out
    }
    pub(super) fn with_renamed_bank(&self, bank_index: usize, name: &str) -> Result<Self, String> {
        if legacy_text_issue(name, false).is_some() {
            return Err("Use at most 18 printable ASCII characters for the bank name.".into());
        }
        let bank = self
            .banks
            .get(bank_index)
            .ok_or("bank index is out of range")?;
        let mut bytes = self.bytes.clone();
        bytes[bank.name_offset..bank.name_offset + 64].copy_from_slice(&bank_name_field(name));
        Self::parse(bytes)
    }

    pub(super) fn with_appended_model(
        &self,
        bank_index: usize,
        model: &PresetFile,
    ) -> Result<Self, String> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or("bank index is out of range")?;
        if self.fixed || bank.fixed {
            return Err("Models can only be added to a collection folder.".into());
        }
        let model = model.as_bytes();
        let bank_length = bank
            .data_length
            .checked_add(model.len())
            .ok_or("bank size overflow")?;
        let file_length = self
            .bytes
            .len()
            .checked_add(model.len())
            .ok_or("collection size overflow")?;
        let bank_size = u32::try_from(bank_length).map_err(|_| "bank exceeds IFF size capacity")?;
        let root_size =
            u32::try_from(file_length - 8).map_err(|_| "bundle exceeds IFF size capacity")?;
        let end = bank.chunk_start + 8 + bank.data_length;
        let mut bytes = Vec::with_capacity(file_length);
        bytes.extend_from_slice(&self.bytes[..end]);
        bytes.extend_from_slice(model);
        bytes.extend_from_slice(&self.bytes[end..]);
        bytes[bank.chunk_start + 4..bank.chunk_start + 8].copy_from_slice(&bank_size.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_size.to_be_bytes());
        Self::parse(bytes)
    }

    pub fn replace(
        &mut self,
        bank_index: usize,
        preset_index: usize,
        replacement: &PresetFile,
    ) -> Result<(), String> {
        *self = self.with_replaced_models(bank_index, [(preset_index, replacement)])?;
        Ok(())
    }

    /// Replace models in ascending slot order while retaining every other chunk.
    pub(super) fn with_replaced_models<'a>(
        &self,
        bank_index: usize,
        replacements: impl IntoIterator<Item = (usize, &'a PresetFile)>,
    ) -> Result<Self, String> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or("bank index is out of range")?;
        let mut bank_length = bank.data_length;
        let mut bytes = Vec::with_capacity(self.bytes.len());
        let mut tail = 0;
        for (preset_index, replacement) in replacements {
            let range = bank
                .ranges
                .get(preset_index)
                .ok_or("preset index is out of range")?;
            if range.start < tail {
                return Err("replacement slots must be unique and in ascending order".into());
            }
            let replacement = replacement.as_bytes();
            bank_length = bank_length
                .checked_sub(range.len())
                .and_then(|length| length.checked_add(replacement.len()))
                .ok_or("bundle size overflow")?;
            bytes.extend_from_slice(&self.bytes[tail..range.start]);
            bytes.extend_from_slice(replacement);
            tail = range.end;
        }
        bytes.extend_from_slice(&self.bytes[tail..]);
        let bank_length =
            u32::try_from(bank_length).map_err(|_| "bank exceeds IFF size capacity")?;
        let root_length =
            u32::try_from(bytes.len() - 8).map_err(|_| "bundle exceeds IFF size capacity")?;
        let header = bank.chunk_start;
        bytes[header + 4..header + 8].copy_from_slice(&bank_length.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_length.to_be_bytes());
        Self::parse(bytes)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "gui")]
    #[test]
    fn factory_slots_use_workbench_selector_order() {
        let models = (0..60)
            .map(|index| {
                let mut preset = PresetFile::default_legacy_model().unwrap();
                preset.rename(&format!("Flash {index}")).unwrap();
                preset
            })
            .collect::<Vec<_>>();
        assert!(BundleFile::from_factory_slots(&models[..59]).is_err());
        let factory = BundleFile::from_factory_slots(&models).unwrap();
        assert_eq!(factory.banks()[0].name, "Custom 1");
        assert_eq!(factory.banks()[0].presets()[0].name(), "Flash 4");
        assert_eq!(factory.banks()[10].name, "Reso");
        assert_eq!(factory.banks()[10].presets()[4].name(), "Flash 50");
        assert_eq!(factory.banks()[11].presets()[0].name(), "Flash 59");
    }

    use super::*;
    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut b = id.to_vec();
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
        if !payload.len().is_multiple_of(2) {
            b.push(0);
        }
        b
    }
    fn example() -> Vec<u8> {
        example_with_patch_kind(*b"L6PA")
    }
    fn example_with_patch_kind(kind: [u8; 4]) -> Vec<u8> {
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&('B' as u16).to_be_bytes());
        let mut bank = b"BANK".to_vec();
        bank.extend(chunk(b"BINF", &info));
        let mut preset = include_bytes!("../../../../tests/fixtures/editor-preset.vwm").to_vec();
        preset[8..12].copy_from_slice(&kind);
        bank.extend_from_slice(&preset);
        let mut bundle = b"L6BA".to_vec();
        bundle.extend(chunk(b"HEAD", &1u32.to_be_bytes()));
        bundle.extend(chunk(b"LIST", &bank));
        chunk(b"FORM", &bundle)
    }
    #[test]
    fn edits_embedded_preset_and_preserves_the_rest() {
        let bytes = example();
        let mut bundle = BundleFile::parse(bytes.clone()).unwrap();
        assert_eq!(bundle.as_bytes(), bytes);
        let mut replacement = bundle.banks()[0].presets()[0].clone();
        replacement.rename("Changed").unwrap();
        bundle.replace(0, 0, &replacement).unwrap();
        let again = BundleFile::parse(bundle.as_bytes().to_vec()).unwrap();
        assert_eq!(again.banks()[0].presets()[0].name(), "Changed");
        assert_eq!(again.banks()[0].name, "B");
        assert!(bundle.replace(0, 4, &replacement).is_err());
    }
    #[test]
    fn all_workbench_patch_forms_are_retained_and_replaceable_in_bundles() {
        for kind in [*b"L6PA", *b"L6FS", *b"L6AS"] {
            let bytes = example_with_patch_kind(kind);
            let mut bundle = BundleFile::parse(bytes.clone()).unwrap();
            assert_eq!(bundle.as_bytes(), bytes);
            assert_eq!(bundle.banks()[0].presets().len(), 1);
            assert_eq!(&bundle.banks()[0].presets()[0].as_bytes()[8..12], &kind);
            let mut replacement = bundle.banks()[0].presets()[0].clone();
            replacement.rename("POD copy").unwrap();
            bundle.replace(0, 0, &replacement).unwrap();
            let reopened = BundleFile::parse(bundle.as_bytes().to_vec()).unwrap();
            assert_eq!(reopened.banks()[0].presets()[0].name(), "POD copy");
            assert_eq!(&reopened.banks()[0].presets()[0].as_bytes()[8..12], &kind);
            assert_eq!(reopened.banks()[0].name, "B");
        }
    }
    #[test]
    fn rename_updates_only_the_fixed_width_name_field() {
        let original = BundleFile::parse(
            include_bytes!("../../../../tests/fixtures/synthetic-bank.vwb").to_vec(),
        )
        .unwrap();
        for rejected in [
            "1234567890123456789",
            "démo",
            "line\nbreak",
            "tab\t",
            "\u{7f}",
        ] {
            assert!(original.with_renamed_bank(1, rejected).is_err());
        }
        let renamed = original.with_renamed_bank(1, "123456789012345678").unwrap();
        assert_eq!(renamed.banks()[1].name, "123456789012345678");
        assert_eq!(renamed.as_bytes().len(), original.as_bytes().len());
        let offset = original.banks()[1].name_offset;
        assert_eq!(
            &renamed.as_bytes()[..offset],
            &original.as_bytes()[..offset]
        );
        assert_eq!(
            &renamed.as_bytes()[offset + 64..],
            &original.as_bytes()[offset + 64..]
        );
    }

    #[test]
    fn replacing_two_resized_models_preserves_opaque_chunks_and_other_banks() {
        let original = BundleFile::parse(
            include_bytes!("../../../../tests/fixtures/synthetic-collection.l6c").to_vec(),
        )
        .unwrap();
        let source = &original.banks()[1];
        let mut first = source.presets()[1].clone();
        first
            .set_metadata(crate::preset::PresetText::Notes, "A larger first model")
            .unwrap();
        let mut second = source.presets()[0].clone();
        second
            .set_metadata(
                crate::preset::PresetText::Notes,
                "A larger second model, too",
            )
            .unwrap();
        let updated = original
            .with_replaced_models(1, [(0, &first), (1, &second)])
            .unwrap();
        let destination = &updated.banks()[1];
        assert_eq!(destination.presets()[0].as_bytes(), first.as_bytes());
        assert_eq!(destination.presets()[1].as_bytes(), second.as_bytes());
        assert_eq!(source.name, destination.name);

        let unaffected = |file: &BundleFile| {
            let bank = &file.banks()[1];
            let bytes = file.as_bytes();
            [
                // Root type, opaque root chunks, and the bank's tag.
                &bytes[8..bank.chunk_start + 4],
                // Bank type, BINF and opaque bank metadata.
                &bytes[bank.chunk_start + 8..bank.ranges[0].start],
                &bytes[bank.ranges[0].end..bank.ranges[1].start],
                &bytes[bank.ranges[1].end..],
            ]
            .concat()
        };
        assert_eq!(unaffected(&original), unaffected(&updated));
    }

    #[test]
    fn malformed_bundle_lengths_fail_without_panics() {
        let bytes = example();
        for n in 0..bytes.len() {
            assert!(BundleFile::parse(bytes[..n].to_vec()).is_err());
        }
        let mut invalid = bytes;
        invalid[28..32].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(BundleFile::parse(invalid).is_err());
    }
}

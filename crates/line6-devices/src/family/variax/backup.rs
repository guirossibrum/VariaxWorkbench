//! Variax backup v1: identity, optional bank names, original metadata and
//! two raw SysEx metadata replies per stored preset.

use super as variax;
use crate::backup::{BackupFolder, read_optional, verify_manifest};
use std::path::Path;

pub(crate) const STORED_FOLDERS: &[&str] = &["presets"];

pub const VARIAX_RESTORE: &str =
    "Variax 600 library backup (read-only copy, line6-devices backup v1).

presets/BB-Bank-S-1.syx and -2.syx are the exact 200-byte SysEx replies the guitar
sent for metadata blocks 1 (address 851) and 2 (address 883) of bank BB, slot S,
after a program change to that preset. index.txt lists bank, slot, Workbench
program number, preset name, body model and any bank names read from the guitar.
bank-names/BB.syx contains the exact 0x6D reply for bank BB when available.
Older backups omit these replies. original-1/2.syx is the preset that was
active before the read. Load with line6_devices::backup::load_variax_library,
or decode one preset with variax::PresetMetadata::from_replies.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   read the library again and compare the two folders
                   (line6_devices::backup::compare, `variax-probe compare`).

These blocks are current-preset metadata as Workbench reads it, not a DSP or
firmware image. Restoring them to the guitar needs a write gate (G6/G7), the
owner's direct permission and an approved restore procedure.
";

pub fn variax_preset_stem(preset: variax::Preset) -> String {
    format!(
        "presets/{:02}-{}-{}",
        preset.bank(),
        preset.bank_name().replace(' ', "_"),
        preset.slot() + 1
    )
}

pub(crate) fn variax_bank_name_file(bank: u8) -> String {
    format!("bank-names/{bank:02}.syx")
}

pub fn variax_index_line(preset: variax::Preset, metadata: &variax::PresetMetadata) -> String {
    let body = match metadata.parameters.get("body_model") {
        Some(variax::ParameterValue::Integer(id)) => format!("{id:#010x}"),
        _ => "?".into(),
    };
    format!(
        "{:>8} {} (program {:3}): {:<18} body {body}",
        preset.bank_name(),
        preset.slot() + 1,
        preset.program_number(),
        metadata.name
    )
}

/// Writes a Variax library read in the v1 layout, plus index and restore note.
pub fn save_variax_library(
    folder: &mut BackupFolder,
    library: &variax::Library,
) -> Result<(), String> {
    folder.file("identity.syx", &library.identified.reply)?;
    folder.file("original-1.syx", &library.original_replies[0])?;
    folder.file("original-2.syx", &library.original_replies[1])?;
    let mut index = String::new();
    for stored in &library.presets {
        let stem = variax_preset_stem(stored.preset);
        folder.file(&format!("{stem}-1.syx"), &stored.replies[0])?;
        folder.file(&format!("{stem}-2.syx"), &stored.replies[1])?;
        index.push_str(&variax_index_line(stored.preset, &stored.metadata));
        index.push('\n');
    }
    for (bank, name) in &library.bank_names {
        folder.file(&variax_bank_name_file(*bank), &name.reply)?;
        use std::fmt::Write as _;
        writeln!(index, "Bank {bank:02} name: {:?}", name.name).expect("write to String");
    }
    folder.file("index.txt", index.as_bytes())?;
    folder.set_restore_note(VARIAX_RESTORE);
    Ok(())
}

pub struct LoadedVariax {
    pub original: Option<variax::PresetMetadata>,
    pub presets: Vec<(variax::Preset, variax::PresetMetadata)>,
    pub bank_names: Vec<(u8, variax::BankName)>,
}

/// Verifies the manifest, then decodes the presets and optional bank names.
pub fn load_variax_library(dir: &Path) -> Result<LoadedVariax, String> {
    verify_manifest(dir)?;
    let pair = |first: &str, second: &str| -> Result<Option<variax::PresetMetadata>, String> {
        match (read_optional(dir, first)?, read_optional(dir, second)?) {
            (Some(a), Some(b)) => variax::PresetMetadata::from_replies(&a, &b)
                .map(Some)
                .map_err(|e| format!("{first}: {e}")),
            (None, None) => Ok(None),
            _ => Err(format!("{first} and {second} must both be present")),
        }
    };
    let original = pair("original-1.syx", "original-2.syx")?;
    let mut presets = Vec::new();
    for preset in variax::Preset::all() {
        let stem = variax_preset_stem(preset);
        if let Some(metadata) = pair(&format!("{stem}-1.syx"), &format!("{stem}-2.syx"))? {
            presets.push((preset, metadata));
        }
    }
    let mut bank_names = Vec::new();
    for bank in 0..variax::BANKS.len() as u8 {
        let path = variax_bank_name_file(bank);
        if let Some(reply) = read_optional(dir, &path)? {
            let name = variax::BankName::from_reply(reply).map_err(|e| format!("{path}: {e}"))?;
            bank_names.push((bank, name));
        }
    }
    Ok(LoadedVariax {
        original,
        presets,
        bank_names,
    })
}

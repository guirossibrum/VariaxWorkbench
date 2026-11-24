//! Convert a Device check Variax settings backup without changing the source.
use super::{BundleFile, PresetFile};
use crate::{ParameterValue, PresetMetadata, midi::Identity};
use line6_devices::family::variax::{
    self,
    backup::{LoadedVariax, load_variax_library},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub struct BackupBundle {
    bundle: BundleFile,
    directory: PathBuf,
    captured_bank_names: usize,
}

impl BackupBundle {
    /// Load every stored slot in Workbench order. The original active buffer in
    /// the backup is separate and is not a replacement for a stored slot.
    pub fn load(directory: &Path) -> Result<Self, String> {
        let directory = canonicalize(directory)?;
        let loaded = load_variax_library(&directory)?;
        let identity = backup_identity(&directory)?;
        let captured_bank_names = loaded.bank_names.len();
        let bundle = library_bundle(
            loaded,
            identity.product_id(),
            identity.version.unwrap_or_default(),
            false,
        )?;
        Ok(Self {
            bundle,
            directory,
            captured_bank_names,
        })
    }

    pub fn bundle(&self) -> &BundleFile {
        &self.bundle
    }

    pub fn into_bundle(self) -> BundleFile {
        self.bundle
    }

    pub fn captured_bank_names(&self) -> usize {
        self.captured_bank_names
    }

    /// Resolve the output directory before saving, including when the user chose
    /// a symlink to the backup. The backup stays read-only input for this action.
    pub fn destination(&self, path: &Path) -> Result<PathBuf, String> {
        let name = path.file_name().ok_or("Choose a new output file.")?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = canonicalize(parent)?;
        if parent.starts_with(&self.directory) {
            return Err("Choose a destination outside the Variax backup folder.".into());
        }
        Ok(parent.join(name))
    }
}

impl BundleFile {
    /// Build a complete editor bank from a live sweep. The backup importer
    /// alone may supply Workbench defaults for missing bank names.
    pub fn from_refreshed_variax(
        loaded: LoadedVariax,
        identity_reply: &[u8],
    ) -> Result<Self, String> {
        let identity = Identity::parse(identity_reply)?;
        if identity.product_id() != 0x0007_0004 || identity.version != Some(0x0310_0000) {
            return Err("Variax Refresh requires Variax 300/600 firmware 3.10.".into());
        }
        library_bundle(
            loaded,
            identity.product_id(),
            identity.version.unwrap_or_default(),
            true,
        )
    }
}

fn library_bundle(
    loaded: LoadedVariax,
    product_id: u32,
    version: u32,
    require_all_names: bool,
) -> Result<BundleFile, String> {
    if !loaded
        .presets
        .iter()
        .map(|(slot, _)| *slot)
        .eq(variax::Preset::all())
    {
        return Err(format!(
            "Incomplete Variax backup: found {} of {} stored model reply pairs.",
            loaded.presets.len(),
            variax::Preset::all().count()
        ));
    }

    let mut names = variax::BANKS.map(str::to_owned);
    let mut captured = [false; variax::BANKS.len()];
    for (bank, name) in loaded.bank_names {
        let bank = usize::from(bank);
        let seen = captured
            .get_mut(bank)
            .ok_or("Variax bank name index is out of range.")?;
        if std::mem::replace(seen, true) {
            return Err("Duplicate Variax bank name reply.".into());
        }
        names[bank] = name.name;
    }
    if require_all_names && captured.contains(&false) {
        return Err("Variax Refresh did not return all 12 bank names.".into());
    }

    // #229: both modes retain the captured position, which may reflect the
    // live knob. Keep the saved-tone flag and all distinct bank locations.
    let models = loaded
        .presets
        .into_iter()
        .map(|(_, metadata)| {
            PresetFile::from_metadata(&editor_metadata(metadata), product_id, version)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rows = names
        .iter()
        .map(String::as_str)
        .zip(models.chunks_exact(usize::from(variax::PRESETS_PER_BANK)));
    BundleFile::create(true, rows)
}

fn canonicalize(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn backup_identity(directory: &Path) -> Result<Identity, String> {
    let path = directory.join("identity.syx");
    let facts = crate::path_facts(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !facts.regular_file || facts.len != 17 {
        return Err("The backup identity.syx must contain one complete identity reply.".into());
    }
    let mut bytes = Vec::new();
    crate::open_regular_file(&path)
        .and_then(|file| file.take(18).read_to_end(&mut bytes))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let identity = Identity::parse(&bytes).map_err(|error| format!("identity.syx: {error}"))?;
    if !matches!(identity.product_id(), 0x0007_0000 | 0x0007_0004) {
        return Err("The backup must target a legacy electric Variax 300/600 or 500/700.".into());
    }
    Ok(identity)
}

fn editor_metadata(metadata: variax::PresetMetadata) -> PresetMetadata {
    use variax::ParameterValue as Captured;
    PresetMetadata {
        name: metadata.name,
        raw: metadata.raw,
        parameters: metadata
            .parameters
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    Captured::Integer(value) => ParameterValue::Integer(value),
                    Captured::Boolean(value) => ParameterValue::Boolean(value),
                    Captured::Float(value) => ParameterValue::Float(value),
                };
                (key, value)
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "synthetic_backup.rs"]
mod synthetic_backup;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{EditorDocument, save_new},
        edit::WIRING,
    };

    #[test]
    fn live_conversion_requires_a_complete_sweep_and_preserves_the_captured_tone_modes() {
        let root = tempfile::tempdir().unwrap();
        let directory = synthetic_backup::make_backup(root.path(), "sweep", 60, 4);
        let identity = fs::read(directory.join("identity.syx")).unwrap();
        let load = || {
            let mut loaded = load_variax_library(&directory).unwrap();
            loaded.bank_names = (0..12)
                .map(|bank| {
                    (
                        bank,
                        variax::BankName {
                            name: format!("Captured {bank}"),
                            reply: vec![],
                        },
                    )
                })
                .collect();
            loaded
        };
        let mut partial = load();
        partial.presets.pop();
        assert!(BundleFile::from_refreshed_variax(partial, &identity).is_err());
        let partial = load_variax_library(&directory).unwrap();
        assert!(BundleFile::from_refreshed_variax(partial, &identity).is_err());

        let file = BundleFile::from_refreshed_variax(load(), &identity).unwrap();
        assert_eq!(file.banks().len(), 12);
        for (bank, row) in file.banks().iter().enumerate() {
            assert_eq!(row.name, format!("Captured {bank}"));
            assert_eq!(row.presets().len(), 5);
        }
        let presets = file.banks()[0].presets();
        for (slot, saved, position) in [(0, 0, 0.25_f32), (1, 1, 0.25), (2, 1, 0.75)] {
            assert_eq!(
                presets[slot].parameter(WIRING, 0x200005).unwrap().bits,
                saved
            );
            assert_eq!(
                presets[slot].parameter(WIRING, 0x200004).unwrap().bits,
                position.to_bits()
            );
        }
    }

    #[test]
    fn full_backup_keeps_slot_order_names_identity_and_captured_tone_state() {
        let root = tempfile::tempdir().unwrap();
        let source = synthetic_backup::make_backup(root.path(), "backup", 60, 4);
        let before = synthetic_backup::snapshot(&source);
        let conversion = BackupBundle::load(&source).unwrap();
        let bundle = conversion.bundle();
        assert!(bundle.fixed);
        assert_eq!(bundle.banks().len(), 12);
        assert_eq!(conversion.captured_bank_names(), 2);
        for (index, bank) in bundle.banks().iter().enumerate() {
            let name = match index {
                0 => "Renamed Custom",
                11 => "",
                _ => variax::BANKS[index],
            };
            assert_eq!(bank.name, name);
            assert!(bank.fixed);
            assert_eq!(bank.presets().len(), 5);
            for (slot, model) in bank.presets().iter().enumerate() {
                let location = variax::Preset::new(index as u8, slot as u8).unwrap();
                let expected = if index == 0 && slot < 3 {
                    "Twin".to_owned()
                } else {
                    format!("Preset {}", location.program_number())
                };
                assert_eq!(model.name(), expected);
                assert_eq!(model.device_id(), 0x0007_0004);
                assert_eq!(model.device_version(), 0x0310_0000);
            }
        }
        let loaded = load_variax_library(&source).unwrap();
        assert!(!variax::same_preset(
            &loaded.presets[0].1,
            &loaded.presets[1].1
        ));
        assert!(variax::same_preset(
            &loaded.presets[1].1,
            &loaded.presets[2].1
        ));
        for (model, (flag, position)) in
            bundle.banks()[0]
                .presets()
                .iter()
                .zip([(0, 0.25_f32), (1, 0.25), (1, 0.75)])
        {
            assert_eq!(model.parameter(WIRING, 0x200005).unwrap().bits, flag);
            assert_eq!(
                model.parameter(WIRING, 0x200004).unwrap().bits,
                position.to_bits()
            );
        }

        let destination = conversion
            .destination(&root.path().join("output.vwb"))
            .unwrap();
        save_new(&destination, bundle.as_bytes()).unwrap();
        assert!(save_new(&destination, b"other contents").is_err());
        let document = EditorDocument::open(&destination).unwrap();
        assert_eq!(document.as_bytes(), bundle.as_bytes());
        assert_eq!(synthetic_backup::snapshot(&source), before);
    }

    #[test]
    fn loader_rejects_incomplete_backups_unsupported_targets_and_invalid_manifest() {
        fn expect_error(source: &Path, expected: &str) {
            match BackupBundle::load(source) {
                Err(error) => assert!(error.contains(expected), "{error}"),
                Ok(_) => panic!("accepted invalid backup"),
            }
        }

        let root = tempfile::tempdir().unwrap();
        let partial = synthetic_backup::make_backup(root.path(), "partial", 59, 4);
        expect_error(&partial, "59 of 60");
        let wrong_family = synthetic_backup::make_backup(root.path(), "other", 60, 5);
        expect_error(&wrong_family, "legacy electric Variax");
        let invalid = root.path().join("invalid");
        fs::create_dir(&invalid).unwrap();
        save_new(&invalid.join("MANIFEST.sha256"), b"bad  identity.syx\n").unwrap();
        save_new(&invalid.join("identity.syx"), b"identity").unwrap();
        expect_error(&invalid, "does not match MANIFEST.sha256");
    }

    #[test]
    fn destination_must_be_outside_the_source_root_including_directory_aliases() {
        let root = tempfile::tempdir().unwrap();
        let source = synthetic_backup::make_backup(root.path(), "backup", 60, 4);
        let conversion = BackupBundle::load(&source).unwrap();
        assert!(
            conversion
                .destination(&source.join("converted.vwb"))
                .is_err()
        );
        assert!(
            conversion
                .destination(&source.join("presets/converted.vwb"))
                .is_err()
        );
        assert!(
            conversion
                .destination(&source.join("../converted.vwb"))
                .is_ok()
        );
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&source, &alias).unwrap();
            assert!(
                conversion
                    .destination(&alias.join("converted.vwb"))
                    .is_err()
            );
        }
    }
}

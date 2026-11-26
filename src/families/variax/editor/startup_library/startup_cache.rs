//! Headless contract for a future adapter over the shared cache's publisher.
//! The adapter must resolve the selected profile and firmware tuple (including
//! the relay), validate the receipt schema and publication proof, and resolve
//! the snapshot within the cache root before exposing a candidate here.

use super::{BackupSource, BundleFile, LibraryOrigin, Path, SystemTime, VariaxEditor};
use line6_devices::{
    family::variax::backup::{SnapshotCoverage, validate_snapshot},
    identity::Identity,
};

#[derive(Clone, Copy)]
struct PublishedSnapshot<'a> {
    directory: &'a Path,
    identity: &'a Identity,
    manifest_sha256: &'a str,
    coverage: SnapshotCoverage,
    source: BackupSource,
    captured_at: SystemTime,
}

impl PublishedSnapshot<'_> {
    fn decode(&self) -> Result<BundleFile, String> {
        if self.source == BackupSource::ImportedBackup {
            return Err("An imported backup is a compatibility library.".into());
        }
        let snapshot = validate_snapshot(self.directory, self.identity, self.coverage)?;
        if snapshot.manifest_sha256() != self.manifest_sha256 {
            return Err("The Variax snapshot manifest differs from the published receipt.".into());
        }
        BundleFile::from_validated_variax_snapshot(&snapshot)
    }
}

fn load_startup(
    editor: &mut VariaxEditor,
    mut published: Vec<PublishedSnapshot<'_>>,
    preferences: &Path,
    load_factory: impl FnOnce(&Path) -> Result<BundleFile, String>,
) {
    if editor.has_document() {
        return;
    }
    published.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.captured_at));
    for candidate in published {
        let Ok(bundle) = candidate.decode() else {
            continue;
        };
        let origin =
            LibraryOrigin::Cached(candidate.source, candidate.captured_at, candidate.coverage);
        if editor.adopt_startup_library(bundle, origin).is_ok() {
            return;
        }
    }
    editor.load_startup_choices(preferences, load_factory);
}

mod tests {
    use super::super::{
        BANKS, Duration, UNIX_EPOCH, fs, synthetic_backup,
        tests::{defaults, record_unchecked_archive},
    };
    use super::*;
    use line6_devices::family::variax::{
        PRESETS_PER_BANK, Preset, procedures::set_firmware_checksum,
    };
    use sha2::{Digest, Sha256};
    use variax_workbench::variax_library::{CompletedVariaxBackup, LibraryPreferences};

    fn identity() -> Identity {
        Identity {
            manufacturer: line6_devices::identity::LINE6,
            family: 7,
            member: 4,
            version: *b"0310",
        }
    }

    fn time(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn manifest_digest(directory: &Path) -> String {
        format!(
            "{:x}",
            Sha256::digest(fs::read(directory.join("MANIFEST.sha256")).unwrap())
        )
    }

    fn backup(
        root: &Path,
        label: &str,
        count: usize,
        coverage: SnapshotCoverage,
    ) -> std::path::PathBuf {
        let mut names = std::array::from_fn::<_, 12, _>(|bank| (bank, BANKS[bank]));
        names[0].1 = label;
        if coverage == SnapshotCoverage::Metadata {
            return synthetic_backup::make_backup_with_names(root, label, count, 4, &names);
        }
        synthetic_backup::make_backup_with_files(root, label, count, 4, &names, |folder| {
            for preset in Preset::all() {
                let index = (preset.bank() + 1) * PRESETS_PER_BANK - 1 - preset.slot();
                let bank = preset.bank_name().replace(' ', "_");
                let name = format!("programs/{index:02}-{bank}-{}.bin", preset.slot() + 1);
                let mut bytes = [0; 3276];
                bytes[0] = preset.program_number();
                set_firmware_checksum(&mut bytes);
                folder.file(&name, &bytes).unwrap();
            }
        })
    }

    fn no_factory(_: &Path) -> Result<BundleFile, String> {
        panic!("startup should not load a factory archive")
    }

    #[test]
    fn newest_published_snapshot_decodes_after_newer_invalid_candidates() {
        let root = tempfile::tempdir().unwrap();
        let defaults = defaults(root.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let metadata = SnapshotCoverage::Metadata;
        let full = SnapshotCoverage::FullPrograms;
        let earlier = backup(root.path(), "Earlier", 60, full);
        let chosen = backup(root.path(), "Chosen", 60, metadata);
        let partial = synthetic_backup::make_backup(root.path(), "Partial", 59, 4);
        let tampered = backup(root.path(), "Tampered", 60, metadata);
        let tampered_digest = manifest_digest(&tampered);
        // A change in hex case preserves the parsed hashes and changes the
        // digest of the manifest bytes bound to the receipt.
        let manifest = tampered.join("MANIFEST.sha256");
        let mut changed_manifest = fs::read(&manifest).unwrap();
        let digit = changed_manifest
            .iter_mut()
            .find(|byte| matches!(**byte, b'a'..=b'f'))
            .unwrap();
        digit.make_ascii_uppercase();
        fs::remove_file(&manifest).unwrap();
        fs::write(&manifest, changed_manifest).unwrap();
        let unreceipted = backup(root.path(), "Unreceipted", 60, metadata);
        let before = synthetic_backup::snapshot(root.path());
        let identity = identity();
        let wrong_identity = Identity {
            version: *b"0320",
            ..identity.clone()
        };
        let missing = root.path().join("missing");
        let earlier_digest = manifest_digest(&earlier);
        let chosen_digest = manifest_digest(&chosen);
        let partial_digest = manifest_digest(&partial);
        let metadata_candidate = PublishedSnapshot {
            directory: &chosen,
            identity: &identity,
            manifest_sha256: &chosen_digest,
            coverage: metadata,
            source: BackupSource::DeviceCheck,
            captured_at: time(20),
        };
        let older_candidate = PublishedSnapshot {
            directory: &earlier,
            manifest_sha256: &earlier_digest,
            coverage: full,
            source: BackupSource::FullBackup,
            captured_at: time(10),
            ..metadata_candidate
        };
        let bad_candidates = [
            PublishedSnapshot {
                directory: &missing,
                captured_at: time(80),
                ..metadata_candidate
            },
            PublishedSnapshot {
                directory: &partial,
                manifest_sha256: &partial_digest,
                captured_at: time(70),
                ..metadata_candidate
            },
            PublishedSnapshot {
                directory: &tampered,
                manifest_sha256: &tampered_digest,
                captured_at: time(60),
                ..metadata_candidate
            },
            PublishedSnapshot {
                identity: &wrong_identity,
                captured_at: time(50),
                ..metadata_candidate
            },
            PublishedSnapshot {
                coverage: full,
                captured_at: time(40),
                ..metadata_candidate
            },
            PublishedSnapshot {
                source: BackupSource::ImportedBackup,
                captured_at: time(30),
                ..metadata_candidate
            },
        ];
        assert!(validate_snapshot(&unreceipted, &identity, metadata).is_ok());
        assert!(validate_snapshot(&tampered, &identity, metadata).is_ok());
        assert!(
            bad_candidates
                .iter()
                .all(|candidate| candidate.decode().is_err())
        );

        let mut editor = VariaxEditor::new(&defaults);
        let candidates = [older_candidate, metadata_candidate]
            .into_iter()
            .chain(bad_candidates)
            .collect();
        load_startup(&mut editor, candidates, preferences, no_factory);
        let document = editor.document.as_ref().unwrap();
        let banks = document.banks().unwrap();
        assert_eq!(banks.len(), 12);
        assert_eq!(banks[0].name, "Chosen");
        assert_eq!(banks[11].presets().len(), 5);
        assert_eq!(document.preset().name(), "Twin");
        assert_eq!(
            editor.library_label().unwrap(),
            "Cached Device check backup · 1970-01-01 00:00:20 UTC · Settings only"
        );
        assert!(editor.source_path.is_none());
        assert!(!editor.dirty());

        // The selected metadata generation stands alone. The earlier full
        // generation is usable when it is the only valid candidate.
        let mut editor = VariaxEditor::new(&defaults);
        load_startup(&mut editor, vec![older_candidate], preferences, no_factory);
        assert_eq!(
            editor.library_label().unwrap(),
            "Cached Full Variax backup · 1970-01-01 00:00:10 UTC · Full program capture"
        );
        assert_eq!(
            editor.document.as_ref().unwrap().banks().unwrap()[0].name,
            "Earlier"
        );
        editor.change(|preset| preset.rename("Offline edit"));
        assert!(editor.dirty());
        assert_eq!(synthetic_backup::snapshot(root.path()), before);
    }

    #[test]
    fn invalid_cache_falls_back_to_explicit_compatibility_then_factory_then_empty() {
        let root = tempfile::tempdir().unwrap();
        let defaults = defaults(root.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let prefs = LibraryPreferences::new(preferences);
        let identity = identity();
        let unreceipted = backup(root.path(), "Unreceipted", 60, SnapshotCoverage::Metadata);
        let bad_cache = || {
            vec![PublishedSnapshot {
                directory: &unreceipted,
                identity: &identity,
                manifest_sha256: "",
                coverage: SnapshotCoverage::Metadata,
                source: BackupSource::DeviceCheck,
                captured_at: time(100),
            }]
        };

        let mut editor = VariaxEditor::new(&defaults);
        editor.load_initial_library(&defaults);
        assert!(!editor.has_document());
        load_startup(&mut editor, bad_cache(), preferences, no_factory);
        assert!(!editor.has_document());
        assert!(editor.library_origin.is_none());

        let archive = root.path().join("firmware.vxf");
        fs::write(&archive, b"synthetic archive input").unwrap();
        record_unchecked_archive(preferences, &archive);
        let before = synthetic_backup::snapshot(root.path());
        load_startup(&mut editor, bad_cache(), preferences, |path| {
            assert_eq!(path, archive);
            BundleFile::new_default_bundle()
        });
        assert!(matches!(
            editor.library_origin,
            Some(LibraryOrigin::Factory)
        ));
        assert_eq!(synthetic_backup::snapshot(root.path()), before);

        let receipt = CompletedVariaxBackup::import(&unreceipted).unwrap();
        prefs.remember_backup(&receipt).unwrap();
        let before = synthetic_backup::snapshot(root.path());
        let mut editor = VariaxEditor::new(&defaults);
        load_startup(&mut editor, bad_cache(), preferences, no_factory);
        assert!(matches!(
            editor.library_origin,
            Some(LibraryOrigin::Backup(BackupSource::ImportedBackup, _))
        ));
        assert_eq!(
            editor.document.as_ref().unwrap().banks().unwrap()[0].name,
            "Unreceipted"
        );
        assert_eq!(synthetic_backup::snapshot(root.path()), before);
    }

    #[test]
    fn existing_user_document_and_dirty_edit_take_precedence() {
        let root = tempfile::tempdir().unwrap();
        let defaults = defaults(root.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let path = root.path().join("mine.vwb");
        let bundle = BundleFile::new_default_bundle().unwrap();
        fs::write(&path, bundle.as_bytes()).unwrap();
        let mut editor = VariaxEditor::new(&defaults);
        editor.adopt_document(
            super::super::EditorDocument::from_file_bytes(bundle.as_bytes().to_vec(), &path)
                .unwrap(),
            Some(&path),
        );
        editor.change(|preset| preset.rename("Unsaved edit"));
        let edited = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let directory = backup(root.path(), "Published", 60, SnapshotCoverage::Metadata);
        let before = synthetic_backup::snapshot(root.path());
        load_startup(
            &mut editor,
            vec![PublishedSnapshot {
                directory: &directory,
                identity: &identity(),
                manifest_sha256: &manifest_digest(&directory),
                coverage: SnapshotCoverage::Metadata,
                source: BackupSource::DeviceCheck,
                captured_at: time(20),
            }],
            preferences,
            no_factory,
        );
        assert!(editor.dirty());
        assert_eq!(editor.source_path.as_deref(), Some(path.as_path()));
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), edited);
        assert!(editor.library_origin.is_none());
        assert_eq!(synthetic_backup::snapshot(root.path()), before);
    }

    #[cfg(feature = "usb")]
    #[test]
    fn cache_remains_historical_across_two_same_model_attachments() {
        use super::super::super::{
            device_sync::Action,
            refresh::tests::{verified_rig, verified_tab_for},
        };
        use crate::editor::{FamilyTab, registry::DEVICE_PROFILES};

        let root = tempfile::tempdir().unwrap();
        let defaults = defaults(root.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let directory = backup(root.path(), "Published", 60, SnapshotCoverage::Metadata);
        let mut editor = VariaxEditor::new(&defaults);
        load_startup(
            &mut editor,
            vec![PublishedSnapshot {
                directory: &directory,
                identity: &identity(),
                manifest_sha256: &manifest_digest(&directory),
                coverage: SnapshotCoverage::Metadata,
                source: BackupSource::DeviceCheck,
                captured_at: time(20),
            }],
            preferences,
            no_factory,
        );
        editor.change(|preset| preset.rename("Keep my edit"));
        let edited = editor.document.as_ref().unwrap().as_bytes().to_vec();
        let mut rig = verified_rig();
        let first = verified_tab_for(&mut rig);
        editor.observe_device(Some(&first));
        let label = editor.library_label().unwrap();
        assert!(label.ends_with("Source guitar unverified for this connection"));
        assert!(editor.has_historical_guitar_library());
        assert!(!editor.has_current_guitar_library());
        assert!(!editor.sync_ready(Action::ReadUnmodified));

        rig.report_guitar_disconnect(&first.device.key, DEVICE_PROFILES)
            .unwrap();
        assert!(rig.queue_guitar_refresh(&first.device.key.attachment, false));
        let second = verified_tab_for(&mut rig);
        assert_eq!(first.device.key.attachment, second.device.key.attachment);
        assert_ne!(first.device.key, second.device.key);
        editor.observe_device(Some(&second));
        assert_eq!(editor.library_label().unwrap(), label);
        assert!(editor.has_historical_guitar_library());
        assert!(!editor.has_current_guitar_library());
        assert!(!editor.sync_ready(Action::ReadUnmodified));
        assert!(editor.dirty());
        assert_eq!(editor.document.as_ref().unwrap().as_bytes(), edited);
    }
}

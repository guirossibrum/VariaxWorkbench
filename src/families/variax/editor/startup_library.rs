use super::{
    EditorDefaults, EditorDocument, EditorServices, FAMILY, FileFilter, Path, PathBuf, PickRequest,
    ProductFamily, VariaxEditor, egui, tabs,
};
use line6_devices::{
    backup::utc,
    family::variax::{BANKS, PRESETS_PER_BANK},
};
#[cfg(feature = "usb")]
use std::io::Write;
use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use variax_workbench::{
    bundle::{BackupBundle, BundleFile},
    factory_reference::FactoryReference,
    firmware::MAX_CONTAINER_BYTES,
    variax_library::{BackupSource, LibraryPreferences},
};

#[derive(Clone, Copy)]
pub(super) enum LibraryOrigin {
    Download(SystemTime),
    Backup(BackupSource, SystemTime),
    Factory,
}

impl LibraryOrigin {
    pub(super) fn label(self) -> String {
        let (source, completed_at) = match self {
            Self::Download(time) => ("Downloaded from guitar", time),
            Self::Backup(BackupSource::DeviceCheck, time) => ("Device check backup", time),
            Self::Backup(BackupSource::FullBackup, time) => ("Full Variax backup", time),
            Self::Factory => {
                return "Factory presets from your Variax 3.10 firmware archive".into();
            }
        };
        let date = completed_at
            .duration_since(UNIX_EPOCH)
            .map_or_else(|_| "before 1970 UTC".into(), |time| utc(time.as_secs()).1);
        format!("{source} · {date}")
    }
}

fn snapshots(preferences: &Path) -> PathBuf {
    preferences
        .with_file_name("variax-library-snapshots")
        .join("backups")
}

fn snapshot_time(path: &Path) -> Option<SystemTime> {
    let stem = path
        .file_name()?
        .to_str()?
        .strip_suffix(".vwb")?
        .strip_prefix("download-")?;
    let (seconds, remainder) = stem.split_once('-')?;
    let (nanoseconds, _) = remainder.split_once('-')?;
    let nanos = nanoseconds.parse::<u32>().ok()?;
    if nanos >= 1_000_000_000 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::new(seconds.parse().ok()?, nanos))
}

#[cfg(feature = "usb")]
pub(super) fn save_download(
    preferences: &Path,
    document: &EditorDocument,
    time: SystemTime,
) -> Result<(), String> {
    let elapsed = time.duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?;
    let directory = snapshots(preferences);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".reading-")
        .tempfile_in(&directory)
        .map_err(|e| e.to_string())?;
    temporary
        .write_all(document.as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    let suffix = temporary
        .path()
        .file_name()
        .expect("temporary filename")
        .to_string_lossy();
    let destination = directory.join(format!(
        "download-{}-{}-{suffix}.vwb",
        elapsed.as_secs(),
        elapsed.subsec_nanos()
    ));
    temporary
        .persist_noclobber(destination)
        .map_err(|e| e.error.to_string())?;
    Ok(())
}

fn latest_download(preferences: &Path) -> Option<(BundleFile, SystemTime)> {
    let mut candidates = fs::read_dir(snapshots(preferences))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            Some((snapshot_time(&path)?, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates.into_iter().find_map(|(time, path)| {
        let bytes = tabs::open_candidate(&path, FAMILY.info().ceiling).ok()?;
        let bundle = BundleFile::parse(bytes).ok()?;
        (bundle.fixed
            && bundle.banks().len() == BANKS.len()
            && bundle.banks().iter().all(|bank| {
                bank.presets().len() == usize::from(PRESETS_PER_BANK)
                    && bank
                        .presets()
                        .iter()
                        .all(|preset| preset.is_legacy_electric())
            }))
        .then_some((bundle, time))
    })
}

fn latest_library(preferences: &Path) -> Result<Option<(BundleFile, LibraryOrigin)>, String> {
    let download = latest_download(preferences);
    let backup = LibraryPreferences::new(preferences).latest_variax_library();
    if let Some(backup) = backup
        && download
            .as_ref()
            .is_none_or(|(_, time)| backup.completed_at > *time)
    {
        match BackupBundle::load(backup.folder()) {
            Ok(bundle) => {
                return Ok(Some((
                    bundle.into_bundle(),
                    LibraryOrigin::Backup(backup.source, backup.completed_at),
                )));
            }
            Err(error) if download.is_none() => {
                return Err(format!("Could not load the saved Variax backup: {error}"));
            }
            Err(_) => {}
        }
    }
    Ok(download.map(|(bundle, time)| (bundle, LibraryOrigin::Download(time))))
}

fn factory_bundle(path: &Path) -> Result<BundleFile, String> {
    let bytes = tabs::open_candidate(path, MAX_CONTAINER_BYTES as u64)?;
    let reference = FactoryReference::parse(&bytes)
        .map_err(|error| format!("Could not open Variax factory presets: {error}"))?;
    BundleFile::from_variax_factory(&reference)
}

impl VariaxEditor {
    pub(super) fn load_initial_library(&mut self, defaults: &EditorDefaults) {
        #[cfg(test)]
        if defaults.preferences.is_none() {
            self.open_example();
            return;
        }
        let Some(preferences) = defaults.preferences.as_deref() else {
            return;
        };
        self.load_startup_choices(preferences, factory_bundle);
    }

    fn load_startup_choices(
        &mut self,
        preferences: &Path,
        load_factory: impl FnOnce(&Path) -> Result<BundleFile, String>,
    ) {
        match latest_library(preferences) {
            Ok(Some((bundle, origin))) => {
                if let Err(error) = self.adopt_startup_library(bundle, origin) {
                    self.error = Some(error);
                }
                return;
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
        if let Some(archive) = LibraryPreferences::new(preferences).factory_archive() {
            let result = load_factory(&archive)
                .and_then(|bundle| self.adopt_startup_library(bundle, LibraryOrigin::Factory));
            if let Err(error) = result {
                self.error = Some(error);
            }
        }
    }

    fn adopt_startup_library(
        &mut self,
        bundle: BundleFile,
        origin: LibraryOrigin,
    ) -> Result<(), String> {
        let source = PathBuf::from(match origin {
            LibraryOrigin::Factory => "Variax factory library",
            _ => "Downloaded Variax library",
        });
        let document = EditorDocument::from_bundle(bundle, source)?;
        self.adopt_document(document, None);
        self.library_origin = Some(origin);
        self.status.clear();
        Ok(())
    }

    pub(super) fn choose_factory_archive(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
    ) {
        let Some(preferences) = self.preferences_path.clone() else {
            return;
        };
        let request = PickRequest::OpenEditorFile(vec![FileFilter {
            name: "Variax firmware archive",
            extensions: &["vxf"],
        }]);
        services.request_picker::<Self>(ctx, request, move |editor, path, _| {
            let Some(path) = path else { return };
            match LibraryPreferences::new(&preferences).remember_factory_archive(&path) {
                Ok(()) if editor.document.is_none() => {
                    editor.load_startup_choices(&preferences, factory_bundle);
                }
                Ok(()) => {
                    editor.error = None;
                    editor.status = "Remembered the Variax factory archive.".into();
                }
                Err(error) => {
                    editor.error = Some(format!(
                        "Could not use the Variax firmware archive: {error}"
                    ))
                }
            }
        });
    }
}

#[cfg(all(test, feature = "usb"))]
use super::new_files::synthetic_backup;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{FakeDialogs, tests::app};

    fn defaults(directory: &Path) -> EditorDefaults {
        EditorDefaults {
            artwork: directory.join("artwork"),
            preferences: Some(directory.join("editor-preferences.txt")),
            startup_panel: None,
        }
    }

    fn record_unchecked_archive(preferences: &Path, archive: &Path) {
        let choices = preferences.with_file_name("variax-library-preferences");
        fs::create_dir_all(&choices).unwrap();
        let record = serde_json::json!({
            "chosen_at": UNIX_EPOCH + Duration::from_secs(42),
            "path": archive.as_os_str(),
        });
        fs::write(
            choices.join("archive-test.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn empty_startup_has_no_example_library() {
        let directory = tempfile::tempdir().unwrap();
        let defaults = defaults(directory.path());
        let editor = FAMILY
            .open(&defaults, crate::editor::TabSource::New)
            .unwrap();
        let editor = (&*editor as &dyn std::any::Any)
            .downcast_ref::<VariaxEditor>()
            .unwrap();
        assert!(!editor.has_document());
        assert!(editor.library_origin.is_none());
        assert!(editor.source_path.is_none());
    }

    #[test]
    fn firmware_only_startup_uses_the_remembered_choice_and_leaves_it_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let defaults = defaults(directory.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let archive = directory.path().join("firmware.vxf");
        fs::write(&archive, b"mock fingerprinted archive").unwrap();
        record_unchecked_archive(preferences, &archive);
        let mut editor = VariaxEditor::new(&defaults);
        editor.load_startup_choices(preferences, |chosen| {
            assert_eq!(chosen, archive);
            BundleFile::new_default_bundle()
        });
        assert!(matches!(
            editor.library_origin,
            Some(LibraryOrigin::Factory)
        ));
        assert_eq!(editor.document.as_ref().unwrap().banks().unwrap().len(), 12);
        assert!(!editor.dirty());
        assert!(editor.source_path.is_none());
        assert_eq!(fs::read(&archive).unwrap(), b"mock fingerprinted archive");
    }

    #[test]
    fn unknown_firmware_is_refused_at_pick_and_at_startup() {
        let (ctx, mut app) = app();
        let directory = tempfile::tempdir().unwrap();
        let defaults = defaults(directory.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let archive = directory.path().join("unknown.vxf");
        fs::write(&archive, b"unknown inputs").unwrap();
        *app.variax_mut() = VariaxEditor::new(&defaults);
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(archive.clone())].into(),
            ..Default::default()
        });
        app.test_action(|editor: &mut VariaxEditor, services| {
            editor.choose_factory_archive(&ctx, services)
        });
        app.poll_picker(&ctx);
        assert!(!app.variax().has_document());
        assert!(
            app.variax()
                .error
                .as_deref()
                .unwrap()
                .contains("Could not use")
        );
        assert!(
            LibraryPreferences::new(preferences)
                .factory_archive()
                .is_none()
        );

        record_unchecked_archive(preferences, &archive);
        app.variax_mut().load_initial_library(&defaults);
        assert!(!app.variax().has_document());
        assert!(
            app.variax()
                .error
                .as_deref()
                .unwrap()
                .contains("Could not open Variax factory presets")
        );
        assert_eq!(fs::read(&archive).unwrap(), b"unknown inputs");
    }

    #[cfg(feature = "usb")]
    #[test]
    fn latest_complete_download_wins_and_old_atomic_snapshots_are_retained() {
        use line6_devices::backup::verified_manifest_entries;
        use variax_workbench::variax_library::{BackupIdentity, CompletedVariaxBackup};

        let directory = tempfile::tempdir().unwrap();
        let defaults = defaults(directory.path());
        let preferences = defaults.preferences.as_deref().unwrap();
        let older = UNIX_EPOCH + Duration::from_secs(10);
        let backup_time = UNIX_EPOCH + Duration::from_secs(20);
        let newer = UNIX_EPOCH + Duration::from_secs(30);
        let mut snapshot = EditorDocument::from_bundle(
            BundleFile::new_default_bundle().unwrap(),
            Default::default(),
        )
        .unwrap();
        snapshot
            .apply(|preset| preset.rename("Older download"))
            .unwrap();
        save_download(preferences, &snapshot, older).unwrap();
        let names = std::array::from_fn::<_, 12, _>(|bank| (bank, BANKS[bank]));
        let folder = synthetic_backup::make_backup_with_names(
            directory.path(),
            "device-check",
            60,
            4,
            &names,
        );
        let before = synthetic_backup::snapshot(&folder);
        let receipt = serde_json::from_value::<CompletedVariaxBackup>(serde_json::json!({
            "folder": folder.as_os_str(),
            "source": BackupSource::DeviceCheck,
            "completed_at": backup_time,
            "identity": BackupIdentity {
                device: "Synthetic Variax".into(), firmware: "3.10".into(),
                usb_path: "test port".into(), product_id: 0x0007_0004,
            },
            "manifest_verified_files": verified_manifest_entries(&folder).unwrap().len(),
        }))
        .unwrap();
        LibraryPreferences::new(preferences)
            .remember_backup(&receipt)
            .unwrap();
        let mut editor = VariaxEditor::new(&defaults);
        editor.load_initial_library(&defaults);
        assert!(
            matches!(editor.library_origin, Some(LibraryOrigin::Backup(BackupSource::DeviceCheck, time)) if time == backup_time)
        );
        assert_eq!(editor.document.as_ref().unwrap().preset().name(), "Twin");

        snapshot
            .apply(|preset| preset.rename("Newer download"))
            .unwrap();
        save_download(preferences, &snapshot, newer).unwrap();
        fs::write(
            snapshots(preferences).join("download-40-0-broken.vwb"),
            b"incomplete",
        )
        .unwrap();
        let archive = directory.path().join("should-not-parse.vxf");
        fs::write(&archive, b"placeholder").unwrap();
        record_unchecked_archive(preferences, &archive);
        editor.load_startup_choices(preferences, |_| {
            panic!("a complete download takes precedence")
        });
        assert!(
            matches!(editor.library_origin, Some(LibraryOrigin::Download(time)) if time == newer)
        );
        assert_eq!(
            editor.document.as_ref().unwrap().preset().name(),
            "Newer download"
        );
        assert_eq!(
            editor.library_origin.unwrap().label(),
            "Downloaded from guitar · 1970-01-01 00:00:30 UTC"
        );
        assert_eq!(fs::read_dir(snapshots(preferences)).unwrap().count(), 3);
        assert_eq!(synthetic_backup::snapshot(&folder), before);
    }
}

use super::CompletedVariaxBackup;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Startup choices stored beside the app's editor preferences. Each update
/// creates a new record, so backup and archive selections cannot clobber each other.
pub struct LibraryPreferences {
    directory: PathBuf,
}

impl LibraryPreferences {
    pub fn new(editor_preferences: &Path) -> Self {
        Self {
            directory: editor_preferences.with_file_name("variax-library-preferences"),
        }
    }

    /// Most recent recorded complete backup whose folder still verifies.
    pub fn latest_variax_library(&self) -> Option<CompletedVariaxBackup> {
        let mut backups: Vec<CompletedVariaxBackup> = self.records("backup").collect();
        backups.sort_by_key(|backup| std::cmp::Reverse(backup.completed_at));
        backups
            .into_iter()
            .find(CompletedVariaxBackup::verify_again)
    }

    /// Most recently chosen owner archive, if it still exists. The caller
    /// verifies the fingerprint again when extracting factory presets.
    pub fn factory_archive(&self) -> Option<PathBuf> {
        let archive = self
            .records::<ArchiveChoice>("archive")
            .max_by_key(|choice| choice.chosen_at)?;
        let path = PathBuf::from(archive.path);
        path.is_file().then_some(path)
    }

    pub fn remember_backup(&self, backup: &CompletedVariaxBackup) -> Result<(), String> {
        if !backup.verify_again() {
            return Err("The completed Variax backup no longer matches its manifest.".into());
        }
        self.record("backup", backup)
    }

    /// Validate the owner's .vxf through the existing file-only factory
    /// extractor before remembering its path. The original stays in place.
    pub fn remember_factory_archive(&self, path: &Path) -> Result<(), String> {
        self.remember_archive_with(path, validate_factory_archive)
    }

    fn remember_archive_with(
        &self,
        path: &Path,
        validate: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("vxf"))
        {
            return Err("Choose a Variax firmware archive (.vxf).".into());
        }
        let path = std::path::absolute(path).map_err(|error| error.to_string())?;
        if !path.is_file() {
            return Err("The selected Variax firmware archive is not a regular file.".into());
        }
        validate(&path)?;
        self.record(
            "archive",
            &ArchiveChoice {
                chosen_at: SystemTime::now(),
                path: path.into_os_string(),
            },
        )
    }

    fn records<T: serde::de::DeserializeOwned>(
        &self,
        prefix: &'static str,
    ) -> impl Iterator<Item = T> {
        fs::read_dir(&self.directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(move |entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                if !name.starts_with(prefix) || !name.ends_with(".json") {
                    return None;
                }
                serde_json::from_reader(std::io::BufReader::new(fs::File::open(entry.path()).ok()?))
                    .ok()
            })
    }

    fn record(&self, prefix: &str, value: &impl Serialize) -> Result<(), String> {
        fs::create_dir_all(&self.directory).map_err(|error| error.to_string())?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".pending-")
            .tempfile_in(&self.directory)
            .map_err(|error| error.to_string())?;
        let name = temporary
            .path()
            .file_name()
            .expect("temporary filename")
            .to_string_lossy();
        let target = self.directory.join(format!("{prefix}-{name}.json"));
        serde_json::to_writer(&mut temporary, value).map_err(|error| error.to_string())?;
        temporary
            .write_all(b"\n")
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| error.to_string())?;
        temporary
            .persist_noclobber(target)
            .map_err(|error| error.error.to_string())?;
        Ok(())
    }
}

fn validate_factory_archive(path: &Path) -> Result<(), String> {
    let file = crate::open_regular_file(path).map_err(|error| error.to_string())?;
    let bytes = crate::shared::file::read_bounded_file(
        file,
        crate::firmware::MAX_CONTAINER_BYTES as u64,
        "The Variax firmware archive exceeds the container size limit.",
    )
    .map_err(|error| format!("Could not read the Variax firmware archive: {error:?}"))?;
    crate::factory_reference::FactoryReference::parse(&bytes)?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct ArchiveChoice {
    chosen_at: SystemTime,
    path: OsString,
}

pub(super) mod os_path {
    use super::*;

    pub fn serialize<S: serde::Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        path.as_os_str().serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<PathBuf, D::Error> {
        OsString::deserialize(deserializer).map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variax_library::{BackupIdentity, BackupSource};
    use line6_devices::backup::{BackupFolder, variax_preset_stem};
    use line6_devices::family::variax::{BANKS, Preset, identify};
    use line6_devices::gate::{GatePolicy, Operation};
    use line6_devices::session::{Link, Path as LinkPath, Quiet, Session};
    use std::time::{Duration, UNIX_EPOCH};

    struct IdentityOnly;

    impl GatePolicy for IdentityOnly {
        fn allows(&self, operation: Operation) -> bool {
            operation == Operation::VariaxIdentity
        }
    }

    struct FakeGuitar {
        sent: bool,
    }

    impl Link for FakeGuitar {
        fn path(&self) -> LinkPath {
            LinkPath::VariaxViaPodxt
        }

        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            assert_eq!(bytes, line6_devices::identity::REQUEST);
            assert!(!self.sent);
            self.sent = true;
            Ok(())
        }

        fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, String> {
            assert!(self.sent);
            Ok(Some(crate::identity_stage::guitar_reply()))
        }

        fn elapsed(&self) -> Duration {
            Duration::ZERO
        }

        fn pause(&mut self, _: Duration) {}
    }

    fn capture(
        root: &Path,
        name: &str,
        complete: bool,
        omit_a_preset: bool,
        programs: bool,
    ) -> PathBuf {
        let mut folder = BackupFolder::create(root, name).unwrap();
        let mut link = FakeGuitar { sent: false };
        let mut log = Quiet;
        let mut session = Session::new(&mut link, &IdentityOnly, &mut log);
        let identified = identify(&mut session).unwrap();
        folder.file("identity.syx", &identified.reply).unwrap();
        for file in ["original-1.syx", "original-2.syx"] {
            folder.file(file, b"synthetic").unwrap();
        }
        for bank in 0..BANKS.len() {
            folder
                .file(&format!("bank-names/{bank:02}.syx"), b"synthetic")
                .unwrap();
        }
        for (index, preset) in Preset::all().enumerate() {
            if omit_a_preset && index == 0 {
                continue;
            }
            let stem = variax_preset_stem(preset);
            for block in [1, 2] {
                folder
                    .file(&format!("{stem}-{block}.syx"), b"synthetic")
                    .unwrap();
            }
            if programs {
                folder
                    .file(&format!("programs/{index:02}.bin"), b"synthetic")
                    .unwrap();
            }
        }
        folder.finish(complete).unwrap()
    }

    fn receipt(
        path: PathBuf,
        seconds: u64,
        source: BackupSource,
    ) -> Result<CompletedVariaxBackup, String> {
        CompletedVariaxBackup::verify(
            path,
            source,
            UNIX_EPOCH + Duration::from_secs(seconds),
            BackupIdentity {
                device: "Variax 300/600".into(),
                firmware: "3.10".into(),
                usb_path: "8-2.1".into(),
                product_id: 0x0007_0004,
            },
        )
    }

    #[test]
    fn latest_backup_is_ordered_by_completion_and_round_trips_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("editor-preferences.txt");
        let preferences = LibraryPreferences::new(&settings);
        let first = receipt(
            capture(dir.path(), "first", true, false, false),
            1,
            BackupSource::DeviceCheck,
        )
        .unwrap();
        let newest = receipt(
            capture(dir.path(), "newest", true, false, true),
            2,
            BackupSource::FullBackup,
        )
        .unwrap();
        assert!(preferences.latest_variax_library().is_none());
        preferences.remember_backup(&newest).unwrap();
        preferences.remember_backup(&first).unwrap();
        assert_eq!(fs::read_dir(&preferences.directory).unwrap().count(), 2);
        assert_eq!(
            LibraryPreferences::new(&settings).latest_variax_library(),
            Some(newest)
        );
    }

    #[test]
    fn partial_and_unverified_backups_never_displace_a_verified_library() {
        let dir = tempfile::tempdir().unwrap();
        let preferences = LibraryPreferences::new(&dir.path().join("editor-preferences.txt"));
        let first = receipt(
            capture(dir.path(), "first", true, false, false),
            1,
            BackupSource::DeviceCheck,
        )
        .unwrap();
        preferences.remember_backup(&first).unwrap();
        for (name, complete, omit_a_preset) in [("partial", false, false), ("omitted", true, true)]
        {
            let path = capture(dir.path(), name, complete, omit_a_preset, false);
            assert!(receipt(path, 2, BackupSource::DeviceCheck).is_err());
        }
        assert!(
            receipt(
                capture(dir.path(), "missing-programs", true, false, false),
                2,
                BackupSource::FullBackup,
            )
            .is_err()
        );
        let newest = receipt(
            capture(dir.path(), "newest", true, false, false),
            3,
            BackupSource::DeviceCheck,
        )
        .unwrap();
        preferences.remember_backup(&newest).unwrap();
        fs::remove_file(newest.folder().join("identity.syx")).unwrap();
        assert!(preferences.remember_backup(&newest).is_err());
        assert_eq!(preferences.latest_variax_library(), Some(first));
    }

    #[test]
    fn archive_choice_is_persisted_separately_and_unknown_firmware_keeps_the_previous_choice() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("editor-preferences.txt");
        let preferences = LibraryPreferences::new(&settings);
        let archive = dir.path().join("owner.vxf");
        let unknown = dir.path().join("unknown.vxf");
        assert!(preferences.factory_archive().is_none());
        fs::write(&archive, b"synthetic fixture accepted by test validator").unwrap();
        use crate::firmware::{
            VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION,
            synthetic::{Spec, build},
        };
        let image = vec![0; 32];
        let unknown_bytes = build(&Spec::basic(
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
            &image,
        ));
        fs::write(&unknown, unknown_bytes).unwrap();
        preferences
            .remember_archive_with(&archive, |input| {
                assert_eq!(input, archive);
                Ok(())
            })
            .unwrap();
        assert!(
            preferences
                .remember_factory_archive(&unknown)
                .unwrap_err()
                .contains("SHA-256")
        );
        let backup = receipt(
            capture(dir.path(), "backup", true, false, false),
            1,
            BackupSource::DeviceCheck,
        )
        .unwrap();
        preferences.remember_backup(&backup).unwrap();
        let reopened = LibraryPreferences::new(&settings);
        assert_eq!(reopened.factory_archive(), Some(archive));
        assert_eq!(reopened.latest_variax_library(), Some(backup));
        assert_eq!(fs::read_dir(&preferences.directory).unwrap().count(), 2);
    }
}

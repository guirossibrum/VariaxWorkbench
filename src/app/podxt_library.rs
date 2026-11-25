//! Verified PODxt Live library receipts used for offline startup.

#[cfg(any(feature = "gui", all(not(test), feature = "usb", target_os = "linux")))]
use line6_devices::backup::{podxt_entry_name, verified_manifest_entries};
#[cfg(any(feature = "gui", all(not(test), feature = "usb", target_os = "linux")))]
use line6_devices::family::podxt::{BankKind, IDENTITY_FAMILY, Location, PODXT_LIVE_MEMBER};
#[cfg(any(feature = "gui", all(not(test), feature = "usb", target_os = "linux")))]
use line6_devices::identity::Identity;
#[cfg(any(feature = "gui", all(not(test), feature = "usb", target_os = "linux")))]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(feature = "gui")]
pub use crate::library_preferences::LibraryPreferences;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "gui", derive(serde::Serialize, serde::Deserialize))]
pub struct CompletedPodxtBackup {
    #[cfg_attr(feature = "gui", serde(with = "crate::library_preferences::os_path"))]
    pub(crate) folder: PathBuf,
    pub completed_at: SystemTime,
    pub firmware: String,
    pub product_id: u32,
    #[cfg_attr(feature = "gui", serde(with = "crate::library_preferences::os_path"))]
    pub midi_device: PathBuf,
    manifest_verified_files: usize,
}

impl CompletedPodxtBackup {
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn manifest_verified_files(&self) -> usize {
        self.manifest_verified_files
    }

    #[cfg(any(
        all(not(test), feature = "usb", target_os = "linux"),
        all(feature = "gui", test)
    ))]
    pub(crate) fn verify(
        folder: PathBuf,
        completed_at: SystemTime,
        midi_device: PathBuf,
    ) -> Result<Self, String> {
        let folder = std::path::absolute(folder).map_err(|error| error.to_string())?;
        let (entries, identity) = verify_complete_manifest(&folder)?;
        Ok(Self {
            folder,
            completed_at,
            firmware: identity.version_text(),
            product_id: (u32::from(identity.family) << 16) | u32::from(identity.member),
            midi_device,
            manifest_verified_files: entries.len(),
        })
    }

    #[cfg(feature = "gui")]
    pub(crate) fn verified_entries(&self) -> Result<Vec<String>, String> {
        let (entries, identity) = verify_complete_manifest(&self.folder)?;
        if entries.len() != self.manifest_verified_files
            || identity.version_text() != self.firmware
            || ((u32::from(identity.family) << 16) | u32::from(identity.member)) != self.product_id
        {
            return Err("The PODxt Live backup no longer matches its receipt.".into());
        }
        Ok(entries)
    }
}

#[cfg(any(feature = "gui", all(not(test), feature = "usb", target_os = "linux")))]
fn verify_complete_manifest(folder: &Path) -> Result<(Vec<String>, Identity), String> {
    if folder
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with("-INCOMPLETE"))
    {
        return Err("The PODxt Live backup is incomplete.".into());
    }
    let entries = verified_manifest_entries(folder)?;
    let names: BTreeSet<&str> = entries.iter().map(String::as_str).collect();
    if names.len() != entries.len() {
        return Err("The PODxt Live backup manifest lists duplicate files.".into());
    }
    // The closing current-tone read is diagnostic. A complete library has
    // the initial current tone and every stored tone and setup.
    for required in ["identity.syx".to_owned(), "current-before.syx".to_owned()]
        .into_iter()
        .chain(Location::all(&BankKind::ALL).map(podxt_entry_name))
    {
        if !names.contains(required.as_str()) {
            return Err(format!(
                "The PODxt Live backup manifest is missing {required}."
            ));
        }
    }
    let reply = std::fs::read(folder.join("identity.syx")).map_err(|error| error.to_string())?;
    let identity = Identity::parse(&reply)
        .filter(|identity| {
            identity.is_line6()
                && identity.family == IDENTITY_FAMILY
                && identity.member == PODXT_LIVE_MEMBER
        })
        .ok_or("The PODxt Live backup has an invalid device identity.")?;
    Ok((entries, identity))
}

#[cfg(all(test, feature = "gui"))]
mod tests {
    use super::*;
    use line6_devices::backup::{BackupFolder, load_podxt_library};
    use line6_devices::family::podxt::EDIT_BUFFER_LEN;
    use std::fs;
    use std::time::{Duration, UNIX_EPOCH};

    fn tone(name: &str) -> [u8; EDIT_BUFFER_LEN] {
        let mut dump = [0; EDIT_BUFFER_LEN];
        dump[..6].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74]);
        dump[7..23].fill(b' ');
        dump[7..7 + name.len()].copy_from_slice(name.as_bytes());
        dump[EDIT_BUFFER_LEN - 1] = 0xf7;
        dump
    }

    fn capture(
        root: &Path,
        name: &str,
        complete: bool,
        omit: Option<Location>,
        identity: &[u8],
    ) -> PathBuf {
        let mut folder = BackupFolder::create(root, name).unwrap();
        folder.file("identity.syx", identity).unwrap();
        folder.file("current-before.syx", &tone(name)).unwrap();
        for location in Location::all(&BankKind::ALL) {
            if Some(location) != omit {
                folder
                    .file(&podxt_entry_name(location), &tone(&location.label()))
                    .unwrap();
            }
        }
        folder.finish(complete).unwrap()
    }

    fn receipt(path: PathBuf, seconds: u64) -> Result<CompletedPodxtBackup, String> {
        CompletedPodxtBackup::verify(
            path,
            UNIX_EPOCH + Duration::from_secs(seconds),
            PathBuf::from("/dev/snd/midiC4D0"),
        )
    }

    #[test]
    fn exports_the_newest_independent_snapshot_with_current_tone_and_all_stored_entries() {
        let data = tempfile::tempdir().unwrap();
        let original = tempfile::tempdir().unwrap();
        let editor_path = data.path().join("editor-preferences.txt");
        let preferences = LibraryPreferences::new(&editor_path);
        assert!(preferences.latest_podxt_library().is_none());
        let identity = crate::identity_stage::bridge_reply();
        let newest = receipt(capture(original.path(), "newest", true, None, &identity), 2).unwrap();
        let older = receipt(capture(original.path(), "older", true, None, &identity), 1).unwrap();
        preferences.remember_podxt_backup(&newest).unwrap();
        preferences.remember_podxt_backup(&older).unwrap();
        let manifest = fs::read(newest.folder().join("MANIFEST.sha256")).unwrap();
        assert!(newest.verified_entries().is_ok());
        assert!(older.verified_entries().is_ok());
        fs::rename(newest.folder(), original.path().join("moved")).unwrap();

        let reopened = LibraryPreferences::new(&editor_path);
        let saved = reopened.latest_podxt_library().unwrap();
        assert_eq!(saved.completed_at, newest.completed_at);
        assert_eq!(saved.firmware, "3.01");
        assert_eq!(saved.product_id, 0x0003_000a);
        assert_eq!(saved.midi_device, newest.midi_device);
        assert_eq!(
            saved.manifest_verified_files(),
            newest.manifest_verified_files()
        );
        assert!(saved.folder().starts_with(data.path()));
        assert_eq!(
            fs::read(saved.folder().join("MANIFEST.sha256")).unwrap(),
            manifest
        );
        assert_eq!(
            fs::read(saved.folder().join("identity.syx")).unwrap(),
            identity
        );
        let loaded = load_podxt_library(saved.folder()).unwrap();
        assert_eq!(loaded.before.unwrap().name(), "newest");
        assert_eq!(loaded.entries.len(), Location::all(&BankKind::ALL).count());
        assert!(reopened.latest_variax_library().is_none());
    }

    #[test]
    fn skips_a_damaged_snapshot_and_rejects_incomplete_and_wrong_device_sources() {
        let data = tempfile::tempdir().unwrap();
        let original = tempfile::tempdir().unwrap();
        let preferences = LibraryPreferences::new(&data.path().join("editor-preferences.txt"));
        let identity = crate::identity_stage::bridge_reply();
        let initial = receipt(
            capture(original.path(), "initial", true, None, &identity),
            1,
        )
        .unwrap();
        preferences.remember_podxt_backup(&initial).unwrap();

        let location = Location::all(&BankKind::ALL).last().unwrap();
        let partial = capture(original.path(), "partial", false, None, &identity);
        assert!(receipt(partial, 2).unwrap_err().contains("incomplete"));
        let missing = capture(original.path(), "missing", true, Some(location), &identity);
        assert!(
            receipt(missing, 2)
                .unwrap_err()
                .contains(&podxt_entry_name(location))
        );
        let wrong_device = capture(
            original.path(),
            "wrong",
            true,
            None,
            &crate::identity_stage::guitar_reply(),
        );
        assert!(
            receipt(wrong_device, 2)
                .unwrap_err()
                .contains("invalid device identity")
        );

        let newer = receipt(capture(original.path(), "newer", true, None, &identity), 2).unwrap();
        preferences.remember_podxt_backup(&newer).unwrap();
        let snapshot = preferences.latest_podxt_library().unwrap();
        fs::remove_file(snapshot.folder().join(podxt_entry_name(location))).unwrap();
        let fallback = preferences.latest_podxt_library().unwrap();
        assert_eq!(fallback.completed_at, initial.completed_at);

        fs::remove_file(newer.folder().join(podxt_entry_name(location))).unwrap();
        assert!(preferences.remember_podxt_backup(&newer).is_err());
        assert_eq!(preferences.latest_podxt_library(), Some(fallback));
    }
}

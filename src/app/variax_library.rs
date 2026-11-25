//! Verified Variax backup receipts and their local startup preferences.

#[cfg(any(feature = "gui", all(feature = "usb", target_os = "linux")))]
use line6_devices::backup::{variax_preset_stem, verified_manifest_entries};
#[cfg(any(feature = "gui", all(feature = "usb", target_os = "linux")))]
use line6_devices::family::variax::{BANKS, Preset};
#[cfg(any(feature = "gui", all(feature = "usb", target_os = "linux")))]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "gui", derive(serde::Serialize, serde::Deserialize))]
pub enum BackupSource {
    DeviceCheck,
    FullBackup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "gui", derive(serde::Serialize, serde::Deserialize))]
pub struct BackupIdentity {
    pub device: String,
    pub firmware: String,
    pub usb_path: String,
    pub product_id: u32,
}

/// Only a completed capture with a verified manifest produces a receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "gui", derive(serde::Serialize, serde::Deserialize))]
pub struct CompletedVariaxBackup {
    #[cfg_attr(feature = "gui", serde(with = "preferences::os_path"))]
    folder: PathBuf,
    pub source: BackupSource,
    pub completed_at: SystemTime,
    pub identity: BackupIdentity,
    manifest_verified_files: usize,
}

impl CompletedVariaxBackup {
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn manifest_verified_files(&self) -> usize {
        self.manifest_verified_files
    }

    #[cfg(any(all(feature = "usb", target_os = "linux"), all(feature = "gui", test)))]
    pub(crate) fn verify(
        folder: PathBuf,
        source: BackupSource,
        completed_at: SystemTime,
        identity: BackupIdentity,
    ) -> Result<Self, String> {
        let folder = std::path::absolute(folder).map_err(|error| error.to_string())?;
        Ok(Self {
            manifest_verified_files: verify_complete_manifest(&folder, source)?,
            folder,
            source,
            completed_at,
            identity,
        })
    }

    #[cfg(feature = "gui")]
    fn verify_again(&self) -> bool {
        verify_complete_manifest(&self.folder, self.source)
            .is_ok_and(|count| count == self.manifest_verified_files)
    }
}

#[cfg(any(feature = "gui", all(feature = "usb", target_os = "linux")))]
fn verify_complete_manifest(folder: &Path, source: BackupSource) -> Result<usize, String> {
    if folder
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with("-INCOMPLETE"))
    {
        return Err("The Variax backup is incomplete.".into());
    }
    let entries = verified_manifest_entries(folder)?;
    let names: BTreeSet<&str> = entries.iter().map(String::as_str).collect();
    for required in ["identity.syx", "original-1.syx", "original-2.syx"]
        .into_iter()
        .map(str::to_owned)
        .chain((0..BANKS.len()).map(|bank| format!("bank-names/{bank:02}.syx")))
        .chain(Preset::all().flat_map(|preset| {
            let stem = variax_preset_stem(preset);
            [format!("{stem}-1.syx"), format!("{stem}-2.syx")]
        }))
    {
        if !names.contains(required.as_str()) {
            return Err(format!("The Variax backup manifest is missing {required}."));
        }
    }
    if names.len() != entries.len() {
        return Err("The Variax backup manifest lists duplicate files.".into());
    }
    if source == BackupSource::FullBackup
        && names
            .iter()
            .filter(|name| name.starts_with("programs/") && name.ends_with(".bin"))
            .count()
            != Preset::all().count()
    {
        return Err("The full Variax backup manifest is missing program files.".into());
    }
    Ok(names.len())
}

#[derive(Debug)]
pub enum BackupResult {
    Variax {
        backup: CompletedVariaxBackup,
        notice: String,
    },
    Podxt(String),
}

#[cfg(feature = "gui")]
#[path = "variax_library_preferences.rs"]
mod preferences;
#[cfg(feature = "gui")]
pub use preferences::LibraryPreferences;

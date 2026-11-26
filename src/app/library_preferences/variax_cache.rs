//! Offline publication and selection for the first-connection Variax cache.

use super::{LibraryPreferences, persist_record, stage_snapshot};
use crate::variax_library::BackupSource;
use line6_devices::{
    family::{
        podxt::check_live_identity,
        variax::{
            backup::{CaptureReceipt, SnapshotCoverage, ValidatedSnapshot, validate_snapshot},
            procedures::check_identity,
        },
    },
    identity::{Identity, LINE6},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cmp::Reverse,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

const SCHEMA: u32 = 1;
const RECEIPT_BYTES: u64 = 4096;
const MANIFEST_BYTES: u64 = 64 * 1024;
// The v1 validator's largest allowed member is an auxiliary text file.
const COPY_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariaxCacheKey {
    profile: String,
    guitar: DeviceVersion,
    relay: DeviceVersion,
}

impl VariaxCacheKey {
    /// The profile is an opaque local ID allocated by the caller. The MIDI
    /// identities identify supported models and firmware across attachments.
    pub fn new(profile: &str, guitar: &Identity, relay: &Identity) -> Result<Self, String> {
        if profile.is_empty()
            || profile.len() > 64
            || !profile.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
        {
            return Err("Invalid local Variax device profile ID.".into());
        }
        check_identity(guitar)?;
        check_live_identity(relay)?;
        Ok(Self {
            profile: profile.into(),
            guitar: DeviceVersion::new(guitar),
            relay: DeviceVersion::new(relay),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceVersion {
    family: u16,
    member: u16,
    firmware: [u8; 4],
}

impl DeviceVersion {
    fn new(identity: &Identity) -> Self {
        Self {
            family: identity.family,
            member: identity.member,
            firmware: identity.version,
        }
    }

    fn identity(&self) -> Identity {
        Identity {
            manufacturer: LINE6,
            family: self.family,
            member: self.member,
            version: self.firmware,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum Coverage {
    #[serde(rename = "variax-settings")]
    Settings,
    #[serde(rename = "variax-programs")]
    Programs,
}

impl From<SnapshotCoverage> for Coverage {
    fn from(coverage: SnapshotCoverage) -> Self {
        match coverage {
            SnapshotCoverage::Metadata => Self::Settings,
            SnapshotCoverage::FullPrograms => Self::Programs,
        }
    }
}

impl Coverage {
    fn snapshot(self) -> SnapshotCoverage {
        match self {
            Self::Settings => SnapshotCoverage::Metadata,
            Self::Programs => SnapshotCoverage::FullPrograms,
        }
    }

    fn source(self) -> BackupSource {
        match self {
            Self::Settings => BackupSource::DeviceCheck,
            Self::Programs => BackupSource::FullBackup,
        }
    }
}

/// The private on-disk receipt is the publication marker. Its path is
/// constrained to the corresponding generation in the selected profile.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u32,
    key: VariaxCacheKey,
    captured_at: SystemTime,
    source: BackupSource,
    coverage: Coverage,
    manifest_sha256: String,
    snapshot: String,
}

/// Published local receipt together with the independently decoded snapshot.
/// Treat this as historical library data across live attachments.
pub struct PublishedVariaxSnapshot {
    receipt: Receipt,
    directory: PathBuf,
    snapshot: ValidatedSnapshot,
}

impl PublishedVariaxSnapshot {
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn snapshot(&self) -> &ValidatedSnapshot {
        &self.snapshot
    }

    pub fn captured_at(&self) -> SystemTime {
        self.receipt.captured_at
    }

    pub fn source(&self) -> BackupSource {
        self.receipt.source
    }
}

impl LibraryPreferences {
    /// Consume the sealed capture receipt and publish an independently
    /// validated copy under its selected profile.
    ///
    /// ```compile_fail,E0308
    /// use line6_devices::family::variax::backup::ValidatedSnapshot;
    /// use variax_workbench::variax_library::LibraryPreferences;
    ///
    /// fn publish_compatibility_snapshot(prefs: &LibraryPreferences, backup: ValidatedSnapshot) {
    ///     let _ = prefs.publish_variax_cache(backup);
    /// }
    /// ```
    pub fn publish_variax_cache(&self, capture: CaptureReceipt) -> Result<(), String> {
        let key = VariaxCacheKey::new(capture.profile(), capture.guitar(), capture.relay())?;
        self.stage_variax_cache(
            capture.source(),
            key,
            capture.coverage(),
            capture.captured_at(),
            capture.manifest_sha256(),
        )?
        .publish()
    }

    /// Revalidate newest first within one profile and supported firmware tuple.
    /// Earlier complete generations remain available when a refresh fails
    /// or the consumer wants to keep a previous full capture separately.
    pub fn variax_cache_candidates(
        &self,
        key: &VariaxCacheKey,
    ) -> impl Iterator<Item = PublishedVariaxSnapshot> + use<> {
        let mut receipts = self.read_cache_receipts(key);
        receipts.sort_unstable_by(|a, b| {
            (Reverse(a.0.captured_at), &a.1).cmp(&(Reverse(b.0.captured_at), &b.1))
        });
        receipts.into_iter().filter_map(|(receipt, generation)| {
            check_dir(&generation).ok()?;
            let directory = generation.join("library");
            let snapshot = receipt.validate(&directory).ok()?;
            Some(PublishedVariaxSnapshot {
                receipt,
                directory,
                snapshot,
            })
        })
    }

    fn read_cache_receipts(&self, key: &VariaxCacheKey) -> Vec<(Receipt, PathBuf)> {
        let Ok((root, profile)) = self.cache_dirs(key) else {
            return Vec::new();
        };
        if [self.variax.directory.as_path(), &root, &profile]
            .into_iter()
            .any(|dir| check_dir(dir).is_err())
        {
            return Vec::new();
        }
        fs::read_dir(&profile)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| read_candidate(&entry, key, &profile))
            .collect()
    }

    fn cache_dirs(&self, key: &VariaxCacheKey) -> Result<(PathBuf, PathBuf), String> {
        // Variax 3.10 through PODxt Live 3.01 is currently the only tuple.
        // Check a key even when obtained by deserialization.
        VariaxCacheKey::new(&key.profile, &key.guitar.identity(), &key.relay.identity())?;
        let root = std::path::absolute(self.variax.directory.join("first-connection"))
            .map_err(|error| error.to_string())?;
        let profile = root.join(format!("profile-{}", key.profile));
        Ok((root, profile))
    }

    fn stage_variax_cache(
        &self,
        source: &Path,
        key: VariaxCacheKey,
        coverage: SnapshotCoverage,
        captured_at: SystemTime,
        manifest_sha256: &str,
    ) -> Result<Staged, String> {
        let (root, profile) = self.cache_dirs(&key)?;
        let snapshot = validate_snapshot(source, &key.guitar.identity(), coverage)?;
        if snapshot.manifest_sha256() != manifest_sha256 {
            return Err("The Variax backup manifest differs from the capture receipt.".into());
        }
        let entries = manifest_paths(source, manifest_sha256)?;

        for dir in [self.variax.directory.as_path(), &root, &profile] {
            create_synced_dir(dir)?;
        }

        let (reservation, directory) =
            stage_snapshot(&profile, source, &entries, Some(COPY_BYTES))?;
        let generation = reservation
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid Variax cache generation name.")?;
        let target = profile.join(format!("{generation}.json"));
        let coverage = Coverage::from(coverage);
        let receipt = Receipt {
            schema: SCHEMA,
            key,
            captured_at,
            source: coverage.source(),
            coverage,
            manifest_sha256: manifest_sha256.into(),
            snapshot: format!("{generation}/library"),
        };
        Ok(Staged {
            reservation,
            directory,
            profile,
            target,
            receipt,
        })
    }
}

struct Staged {
    reservation: tempfile::TempDir,
    directory: PathBuf,
    profile: PathBuf,
    target: PathBuf,
    receipt: Receipt,
}

impl Staged {
    fn publish(self) -> Result<(), String> {
        self.receipt.validate(&self.directory)?;
        // Sync the descendants and their parent entries before the receipt
        // can point readers at this copy.
        for name in ["presets", "bank-names"] {
            sync_dir(&self.directory.join(name))?;
        }
        if self.receipt.coverage == Coverage::Programs {
            sync_dir(&self.directory.join("programs"))?;
        }
        for dir in [&self.directory, self.reservation.path(), &self.profile] {
            sync_dir(dir)?;
        }

        let temporary = tempfile::Builder::new()
            .prefix(".pending-")
            .tempfile_in(&self.profile)
            .map_err(|error| error.to_string())?;
        persist_record(temporary, &self.target, &self.receipt)?;
        if let Err(error) = sync_dir(&self.profile) {
            // This attempt owns the receipt after successful no-clobber
            // publication. On error, its reservation also drops, so readers
            // will skip it even if removing the receipt fails.
            let _ = fs::remove_file(&self.target);
            return Err(error);
        }
        let _ = self.reservation.keep();
        Ok(())
    }
}

impl Receipt {
    fn validate(&self, directory: &Path) -> Result<ValidatedSnapshot, String> {
        let snapshot = validate_snapshot(
            directory,
            &self.key.guitar.identity(),
            self.coverage.snapshot(),
        )?;
        if snapshot.manifest_sha256() != self.manifest_sha256 {
            return Err("The Variax snapshot manifest differs from the published receipt.".into());
        }
        Ok(snapshot)
    }
}

fn read_candidate(
    entry: &fs::DirEntry,
    key: &VariaxCacheKey,
    profile: &Path,
) -> Option<(Receipt, PathBuf)> {
    let name = entry.file_name();
    let generation = name.to_str()?.strip_suffix(".json")?;
    if !generation.strip_prefix("backup-").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
    }) {
        return None;
    }
    let bytes = read_limited(&entry.path(), RECEIPT_BYTES).ok()?;
    let receipt: Receipt = serde_json::from_slice(&bytes).ok()?;
    if receipt.schema != SCHEMA
        || receipt.key != *key
        || receipt.source != receipt.coverage.source()
        || receipt.snapshot != format!("{generation}/library")
    {
        return None;
    }
    Some((receipt, profile.join(generation)))
}

fn manifest_paths(source: &Path, validated_digest: &str) -> Result<Vec<String>, String> {
    let bytes = read_limited(&source.join("MANIFEST.sha256"), MANIFEST_BYTES)?;
    // Use exactly the allowlisted manifest parsed by the strict validator.
    if format!("{:x}", Sha256::digest(&bytes)) != validated_digest {
        return Err("The Variax manifest changed before staging.".into());
    }
    std::str::from_utf8(&bytes)
        .map_err(|error| error.to_string())?
        .lines()
        .map(|line| {
            line.split_once("  ")
                .map(|(_, path)| path.to_owned())
                .ok_or_else(|| "Invalid Variax manifest entry.".into())
        })
        .collect()
}

fn check_dir(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()) {
        Ok(())
    } else {
        Err(format!(
            "Expected a plain cache directory at {}.",
            path.display()
        ))
    }
}

fn create_synced_dir(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_synced_dir(parent)?;
            return create_synced_dir(path);
        }
        Err(error) => return Err(format!("Could not create {}: {error}", path.display())),
    }
    check_dir(path)?;
    sync_dir(parent)
}

fn sync_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not sync {}: {error}", path.display()))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let read = || -> io::Result<Vec<u8>> {
        let meta = fs::symlink_metadata(path)?;
        if !meta.is_file() || meta.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid cache file",
            ));
        }
        let mut bytes = Vec::new();
        crate::open_regular_file(path)?
            .take(limit + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cache file exceeds the size limit",
            ));
        }
        Ok(bytes)
    };
    read().map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
#[path = "variax_cache/tests.rs"]
mod tests;

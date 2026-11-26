//! Offline validation of staged PODxt Live library snapshots.
//!
//! The v1 folder records hashes and filenames but has no typed transport
//! cleanup result, capture identifier or stable physical-unit identifier.
//! Automatic eligibility requires the original in-memory sweep and a
//! successful cleanup outcome from its owner. A log message cannot supply
//! that provenance. Existing explicit backup imports use the legacy loader.

use super::{STORED_FOLDERS, podxt_entry_name};
use crate::{
    family::podxt::{
        BankKind, EDIT_BUFFER_LEN, EditBuffer, Library, Location, StoredTone, TonePayload,
        check_live_identity,
    },
    identity::Identity,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

const MANIFEST: &str = "MANIFEST.sha256";
const IDENTITY_FILE: &str = "identity.syx";
const BEFORE: &str = "current-before.syx";
const AFTER: &str = "current-after.syx";
const AUXILIARY: [&str; 3] = ["index.txt", "log.txt", "RESTORE.txt"];
const MANIFEST_LIMIT: usize = 64 * 1024;
const AUXILIARY_LIMIT: usize = 16 * 1024 * 1024;

/// Decoded, complete v1 snapshot. Automatic caching also requires capture
/// evidence through [Self::check_cache_eligibility].
pub struct ValidatedSnapshot {
    identity: Identity,
    before: EditBuffer,
    after: EditBuffer,
    entries: Vec<StoredTone>,
    manifest_sha256: String,
}

impl ValidatedSnapshot {
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn before(&self) -> &EditBuffer {
        &self.before
    }

    pub fn after(&self) -> &EditBuffer {
        &self.after
    }

    pub fn entries(&self) -> &[StoredTone] {
        &self.entries
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// The owner of the capture transaction supplies its sweep result and
    /// the cleanup outcome from that same link. Disk files and log prose
    /// cannot serve as cleanup evidence. Compare every captured reply to the
    /// bytes that passed validation so a different capture cannot be paired
    /// with this snapshot.
    pub fn check_cache_eligibility(
        &self,
        capture: &Library,
        link_cleanup: Option<&Result<(), String>>,
    ) -> Result<(), String> {
        if !matches!(link_cleanup, Some(Ok(()))) {
            return Err("PODxt capture cleanup was not verified.".into());
        }
        if capture.stopped.is_some() {
            return Err("The PODxt capture stopped before completion.".into());
        }
        if Identity::parse(&capture.identity_reply).as_ref() != Some(&self.identity) {
            return Err("Snapshot identity differs from the PODxt capture.".into());
        }
        if capture.before != self.before || capture.after.as_ref().ok() != Some(&self.after) {
            return Err("Snapshot playing-tone replies differ from the PODxt capture.".into());
        }

        let mut by_address = BTreeMap::new();
        for entry in &capture.entries {
            let address = entry.location.address();
            if by_address.insert(address, entry).is_some() {
                return Err(format!("Duplicate PODxt capture address {address}."));
            }
        }
        for entry in &self.entries {
            let address = entry.location.address();
            let captured = by_address
                .remove(&address)
                .ok_or_else(|| format!("PODxt capture is missing address {address}."))?;
            if captured.location != entry.location || captured.tone != entry.tone {
                return Err(format!(
                    "Snapshot address {address} differs from the PODxt capture."
                ));
            }
        }
        if !by_address.is_empty() {
            return Err("Unexpected address in the PODxt capture.".into());
        }
        Ok(())
    }
}

/// Validate exactly the PODxt Live 3.01 v1 files in a caller-owned staged
/// directory. The expected identity comes from a verified capture or receipt.
/// Each known address is represented by its canonical filename in the v1
/// layout. The 03 74 reply itself carries no address. Playing-tone comparison
/// uses the existing name and controller comparison, including its documented
/// allowance for live-state controller drift.
pub fn validate_snapshot(
    dir: &Path,
    expected_identity: &Identity,
) -> Result<ValidatedSnapshot, String> {
    check_live_identity(expected_identity)?;
    if dir.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.ends_with(".partial") || name.ends_with("-INCOMPLETE")
    }) {
        return Err("The PODxt snapshot is incomplete.".into());
    }
    check_directory(dir)?;
    let mut required = BTreeSet::from([
        IDENTITY_FILE.to_owned(),
        BEFORE.to_owned(),
        AFTER.to_owned(),
    ]);
    let mut addresses = BTreeSet::new();
    for location in Location::all(&BankKind::ALL) {
        if !addresses.insert(location.address()) || !required.insert(podxt_entry_name(location)) {
            return Err("The PODxt bank layout has a duplicate address or filename.".into());
        }
    }

    let manifest = read_file(&dir.join(MANIFEST), MANIFEST_LIMIT)?;
    let contents = std::str::from_utf8(&manifest).map_err(|_| "Manifest is not UTF-8.")?;
    let mut hashes = BTreeMap::new();
    for line in contents.lines() {
        let (hash, name) = line
            .split_once("  ")
            .ok_or("Malformed PODxt snapshot manifest line.")?;
        if !required.contains(name) && !AUXILIARY.contains(&name) {
            return Err(format!("Unexpected PODxt snapshot manifest path {name:?}."));
        }
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Invalid SHA-256 for {name}."));
        }
        if hashes.insert(name, hash.to_ascii_lowercase()).is_some() {
            return Err(format!("Duplicate PODxt snapshot manifest path {name}."));
        }
    }
    if let Some(name) = required
        .iter()
        .find(|name| !hashes.contains_key(name.as_str()))
    {
        return Err(format!("PODxt snapshot manifest is missing {name}."));
    }
    check_members(dir, "", STORED_FOLDERS, &hashes)?;
    for name in STORED_FOLDERS {
        check_members(dir, name, &[], &hashes)?;
    }

    // Decode the same bounded bytes checked against the manifest.
    let read = |name: &str, limit| -> Result<Vec<u8>, String> {
        let bytes = read_file(&dir.join(name), limit)?;
        if hashes.get(name).is_none_or(|hash| *hash != digest(&bytes)) {
            return Err(format!("{name} does not match MANIFEST.sha256."));
        }
        Ok(bytes)
    };
    let identity =
        Identity::parse(&read(IDENTITY_FILE, 17)?).ok_or("Invalid PODxt snapshot identity.syx.")?;
    if identity != *expected_identity {
        return Err("PODxt snapshot identity does not match the expected PODxt Live 3.01.".into());
    }
    for name in AUXILIARY
        .into_iter()
        .filter(|name| hashes.contains_key(name))
    {
        read(name, AUXILIARY_LIMIT)?;
    }
    let dump = |name: &str| -> Result<EditBuffer, String> {
        let tone = EditBuffer::parse(read(name, EDIT_BUFFER_LEN)?)
            .map_err(|error| format!("{name}: {error}"))?;
        TonePayload::from_dump(&tone).map_err(|error| format!("{name}: {error}"))?;
        Ok(tone)
    };
    let before = dump(BEFORE)?;
    let after = dump(AFTER)?;
    if !before.same_tone(&after) {
        return Err("The PODxt playing tone changed during the snapshot.".into());
    }
    let entries = Location::all(&BankKind::ALL)
        .map(|location| {
            Ok(StoredTone {
                location,
                tone: dump(&podxt_entry_name(location))?,
            })
        })
        .collect::<Result<_, String>>()?;
    Ok(ValidatedSnapshot {
        identity,
        before,
        after,
        entries,
        manifest_sha256: digest(&manifest),
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn check_directory(dir: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(format!(
            "Expected a plain PODxt snapshot directory at {}.",
            dir.display()
        ));
    }
    Ok(())
}

fn check_members(
    root: &Path,
    prefix: &str,
    directories: &[&str],
    hashes: &BTreeMap<&str, String>,
) -> Result<(), String> {
    let dir = root.join(prefix);
    check_directory(&dir)?;
    for entry in fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let basename = entry.file_name();
        let name = basename
            .to_str()
            .ok_or("Invalid PODxt snapshot filename.")?;
        if directories.contains(&name) {
            check_directory(&entry.path())?;
            continue;
        }
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        if !(prefix.is_empty() && name == MANIFEST || hashes.contains_key(path.as_str())) {
            return Err(format!("Unlisted PODxt snapshot file {path:?}."));
        }
    }
    Ok(())
}

fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let fail = |error: std::io::Error| format!("{}: {error}", path.display());
    let metadata = fs::symlink_metadata(path).map_err(fail)?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(format!(
            "Expected a regular snapshot file of at most {limit} bytes at {}.",
            path.display()
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(fail)?;
    if !file.metadata().map_err(fail)?.is_file() {
        return Err(format!(
            "Expected a regular snapshot file at {}.",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    if bytes.len() > limit {
        return Err(format!(
            "Snapshot file exceeds {limit} bytes at {}.",
            path.display()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;

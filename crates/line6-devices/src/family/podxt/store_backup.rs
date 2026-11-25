//! Checked local backup and typed slot confirmation for a PODxt store.

use super::*;
use crate::backup::verify_manifest;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path as FilePath;

pub enum StoreSource {
    /// The first-run candidate. Restores the chosen slot's own backup bytes.
    OriginalSlot,
    /// A complete tone, sent only when an owner-approved caller selects it.
    Replacement(TonePayload),
}

/// The selected slot's exact payload from a verified 299-entry backup.
pub struct VerifiedToneBackup {
    pub(super) slot: Location,
    pub(super) original: TonePayload,
}

impl VerifiedToneBackup {
    pub fn slot(&self) -> Location {
        self.slot
    }

    pub fn tone_name(&self) -> String {
        self.original.name()
    }

    pub(super) fn verify_slot<L: Link>(&self, session: &mut Session<L>) -> Result<(), String> {
        let before = read_stored(session, self.slot)?;
        if TonePayload::from_dump(&before)? != self.original {
            return Err(format!(
                "Slot {} differs from its verified backup. No write was sent.",
                self.slot.slot_name()
            ));
        }
        Ok(())
    }
}

pub struct PreparedStore {
    pub(super) backup: VerifiedToneBackup,
    pub(super) candidate: TonePayload,
}

pub struct ConfirmedStore(pub(super) PreparedStore);

impl PreparedStore {
    pub fn confirmation(&self) -> String {
        format!("STORE PODXT {}", self.backup.slot.slot_name())
    }

    pub fn original_matches(&self, tone: &TonePayload) -> bool {
        &self.backup.original == tone
    }

    pub fn original_name(&self) -> String {
        self.backup.tone_name()
    }

    pub fn candidate_name(&self) -> String {
        self.candidate.name()
    }

    pub fn confirm(self, typed: &str) -> Result<ConfirmedStore, String> {
        if typed != self.confirmation() {
            return Err("The typed confirmation must match the PODxt slot exactly.".into());
        }
        Ok(ConfirmedStore(self))
    }
}

/// Prepare a store from a manifest-verified PODxt Live library backup.
pub fn prepare_tone_store(
    directory: &FilePath,
    slot: Location,
    source: StoreSource,
) -> Result<PreparedStore, String> {
    let backup = verify_tone_backup(directory, slot)?;
    let candidate = match source {
        StoreSource::OriginalSlot => backup.original.clone(),
        StoreSource::Replacement(tone) => tone,
    };
    Ok(PreparedStore { backup, candidate })
}

/// Validate the manifest, identity and all 299 entries. Retain the exact
/// destination payload for the addressed read inside the write transaction.
pub fn verify_tone_backup(
    directory: &FilePath,
    slot: Location,
) -> Result<VerifiedToneBackup, String> {
    if directory
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-INCOMPLETE") || name.ends_with(".partial"))
    {
        return Err("Select a completed PODxt Live library backup.".into());
    }
    verify_manifest(directory)?;
    let contents = fs::read_to_string(directory.join("MANIFEST.sha256"))
        .map_err(|error| format!("Cannot read backup manifest: {error}"))?;
    let mut manifest = BTreeMap::new();
    for line in contents.lines().filter(|line| !line.is_empty()) {
        let (hash, name) = line
            .split_once("  ")
            .ok_or("Backup manifest has a malformed line.")?;
        if manifest.insert(name, hash).is_some() {
            return Err(format!("Backup manifest lists {name} more than once."));
        }
    }
    let entry = |name: &str| -> Result<Vec<u8>, String> {
        let expected = manifest
            .get(name)
            .ok_or_else(|| format!("Backup manifest omits {name}."))?;
        let bytes = fs::read(directory.join(name)).map_err(|error| format!("{name}: {error}"))?;
        if format!("{:x}", Sha256::digest(&bytes)) != *expected {
            return Err(format!("{name} changed during verification."));
        }
        Ok(bytes)
    };
    let identity =
        Identity::parse(&entry("identity.syx")?).ok_or("Invalid PODxt backup identity.")?;
    check_live_identity(&identity)?;
    let log = entry("log.txt")?;
    if !std::str::from_utf8(&log)
        .ok()
        .and_then(|text| text.lines().next())
        .is_some_and(|line| line.starts_with("PODxt Live library backup at "))
    {
        return Err("Select a completed PODxt Live library backup.".into());
    }
    let mut original = None;
    for location in Location::all(&BankKind::ALL) {
        let name = backup::podxt_entry_name(location);
        let tone = EditBuffer::parse(entry(&name)?)
            .and_then(|dump| TonePayload::from_dump(&dump))
            .map_err(|error| format!("{name}: {error}"))?;
        if location == slot {
            original = Some(tone);
        }
    }
    Ok(VerifiedToneBackup {
        slot,
        original: original.expect("valid tone slot is in the full library"),
    })
}

pub(super) fn check_live_identity(identity: &Identity) -> Result<(), String> {
    if identity.is_line6()
        && identity.family == IDENTITY_FAMILY
        && identity.member == PODXT_LIVE_MEMBER
        && identity.version == [0, 3, 0, 1]
    {
        Ok(())
    } else {
        Err("PODxt Live firmware 3.01 is required for store.".into())
    }
}

#[cfg(test)]
mod tests;

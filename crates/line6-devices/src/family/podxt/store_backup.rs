//! Checked local backup and typed slot confirmation for a PODxt store.

use super::*;
use crate::backup::verify_manifest;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path as FilePath;
use std::time::{SystemTime, UNIX_EPOCH};

pub enum StoreSource {
    /// The first-run candidate. Restores the chosen slot's own backup bytes.
    OriginalSlot,
    /// A complete tone, sent only when an owner-approved caller selects it.
    Replacement(TonePayload),
}

pub struct PreparedStore {
    pub(super) slot: Location,
    pub(super) day: u64,
    pub(super) original: TonePayload,
    pub(super) candidate: TonePayload,
}

pub struct ConfirmedStore(pub(super) PreparedStore);

impl ConfirmedStore {
    /// Check that the verified backup is still current immediately before use.
    pub fn verify_current_day(&self) -> Result<(), String> {
        self.verify_day(utc_day()?)
    }

    pub(super) fn verify_day(&self, today: u64) -> Result<(), String> {
        if self.0.day != today {
            return Err("The UTC day changed. Verify a current backup before storing.".into());
        }
        Ok(())
    }
}

impl PreparedStore {
    pub fn confirmation(&self) -> String {
        format!("STORE PODXT {}", self.slot.label())
    }

    pub fn original_name(&self) -> String {
        self.original.name()
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

/// Verify all 299 library entries, manifest-covered identity and date, then
/// snapshot the destination bytes used by the preflight comparison.
pub fn prepare_tone_store(
    backup: &FilePath,
    slot: Location,
    source: StoreSource,
) -> Result<PreparedStore, String> {
    prepare_at(backup, slot, source, utc_day()?)
}

pub(super) fn utc_day() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() / 86_400)
        .map_err(|error| format!("Cannot determine the current UTC day: {error}"))
}

fn prepare_at(
    directory: &FilePath,
    slot: Location,
    source: StoreSource,
    day: u64,
) -> Result<PreparedStore, String> {
    if slot.kind() != BankKind::Tones {
        return Err("Select a saved PODxt tone slot (1A–32D).".into());
    }
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
    let stamp = crate::backup::utc(day * 86_400).1;
    let log = entry("log.txt")?;
    if !std::str::from_utf8(&log)
        .ok()
        .and_then(|text| text.lines().next())
        .and_then(|line| line.strip_prefix("PODxt Live library backup at "))
        .is_some_and(|line| {
            line.split_once(' ')
                .is_some_and(|(date, _)| date == &stamp[..10])
        })
    {
        return Err("Select a PODxt Live library backup made today (UTC).".into());
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
    let original = original.expect("valid tone slot is in the full library");
    let candidate = match source {
        StoreSource::OriginalSlot => original.clone(),
        StoreSource::Replacement(tone) => tone,
    };
    Ok(PreparedStore {
        slot,
        day,
        original,
        candidate,
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

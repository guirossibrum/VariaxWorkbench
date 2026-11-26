//! Strict cache validation for an immutable, staged Variax v1 backup.
//! The legacy explicit-import reader has its existing compatibility rules.

use super::{
    LoadedVariax, PROGRAM_BYTES, check_program_checksum, program_file, variax_bank_name_file,
    variax_preset_stem,
};
use crate::{
    family::variax::{
        BANKS, BankName, Library, Preset, PresetMetadata, Restore, procedures::check_identity,
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

// The existing bank-name decoder permits longer replies within the SysEx bound.
const BANK_REPLY_LIMIT: usize = 8192;
const MANIFEST_LIMIT: usize = 64 * 1024;
const AUXILIARY_LIMIT: usize = 16 * 1024 * 1024;
const AUXILIARY_FILES: [&str; 3] = ["index.txt", "log.txt", "RESTORE.txt"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotCoverage {
    Metadata,
    FullPrograms,
}

/// The requested coverage must come from the capture mode or cache receipt.
pub enum SnapshotPrograms {
    MetadataOnly,
    Full(Vec<(Preset, [u8; PROGRAM_BYTES])>),
}

pub struct ValidatedSnapshot {
    identity: Identity,
    library: LoadedVariax,
    programs: SnapshotPrograms,
    hashes: BTreeMap<String, String>,
    manifest_sha256: String,
}

impl ValidatedSnapshot {
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn library(&self) -> &LoadedVariax {
        &self.library
    }

    pub fn programs(&self) -> &SnapshotPrograms {
        &self.programs
    }

    pub fn coverage(&self) -> SnapshotCoverage {
        match self.programs {
            SnapshotPrograms::MetadataOnly => SnapshotCoverage::Metadata,
            SnapshotPrograms::Full(_) => SnapshotCoverage::FullPrograms,
        }
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Compare a compatibility sweep and its reported cleanup with this file
    /// snapshot. This comparison alone carries no capture provenance.
    pub fn check_cache_eligibility(
        &self,
        capture: SnapshotCapture<'_>,
        link_cleanup: Option<&Result<(), String>>,
    ) -> Result<(), String> {
        if !matches!(link_cleanup, Some(Ok(()))) {
            return Err("Variax session and link cleanup were not verified.".into());
        }
        self.check_capture_matches(capture)
    }

    pub(super) fn check_capture_matches(&self, capture: SnapshotCapture<'_>) -> Result<(), String> {
        let (settings, full) = match capture {
            SnapshotCapture::Metadata(library) => (library, None),
            SnapshotCapture::Full(backup) => (&backup.settings, Some(backup)),
        };
        if full.is_some() != (self.coverage() == SnapshotCoverage::FullPrograms) {
            return Err("Snapshot coverage does not match the capture.".into());
        }
        if settings.stopped.is_some()
            || settings.presets.len() != Preset::all().count()
            || settings.bank_names.len() != BANKS.len()
            || full.is_some_and(|backup| backup.programs_read() != Preset::all().count())
        {
            return Err("The Variax capture is incomplete.".into());
        }
        if !matches!(settings.restore, Restore::Switched { verified: true, .. }) {
            return Err("The Variax switch-back was not verified.".into());
        }
        if settings.release_error.is_some() {
            return Err("Variax session and link cleanup were not verified.".into());
        }
        if settings.identified.identity != self.identity {
            return Err("The captured Variax identity differs from the snapshot.".into());
        }

        let same = |name: &str, bytes: &[u8]| -> Result<(), String> {
            if self
                .hashes
                .get(name)
                .is_some_and(|hash| *hash == digest(bytes))
            {
                Ok(())
            } else {
                Err(format!("{name} differs from the Variax capture."))
            }
        };
        same("identity.syx", &settings.identified.reply)?;
        same("original-1.syx", &settings.original_replies[0])?;
        same("original-2.syx", &settings.original_replies[1])?;
        for preset in Preset::all() {
            let stem = variax_preset_stem(preset);
            let stored = settings
                .presets
                .iter()
                .find(|entry| entry.preset == preset)
                .ok_or_else(|| format!("Capture is missing {stem}."))?;
            same(&format!("{stem}-1.syx"), &stored.replies[0])?;
            same(&format!("{stem}-2.syx"), &stored.replies[1])?;
        }
        for bank in 0..BANKS.len() as u8 {
            let path = variax_bank_name_file(bank);
            let (_, name) = settings
                .bank_names
                .iter()
                .find(|(index, _)| *index == bank)
                .ok_or_else(|| format!("Capture is missing {path}."))?;
            same(&path, &name.reply)?;
        }
        if let Some(full) = full {
            for preset in Preset::all() {
                let path = program_file(preset);
                let (_, program) = full
                    .verified_programs()
                    .find(|(position, _)| *position == preset)
                    .ok_or_else(|| format!("Capture is missing {path}."))?;
                same(&path, program)?;
            }
        }
        Ok(())
    }
}

/// Original in-memory sweep result. Protocol cleanup is part of this
/// result; the transport cleanup is supplied separately by its owner.
pub enum SnapshotCapture<'a> {
    Metadata(&'a Library),
    Full(&'a super::FullLibrary),
}

/// Validate only the exact v1 files of the requested coverage. The caller
/// supplies an identity from the current verified capture or saved receipt,
/// and owns the staged directory for the duration of validation.
pub fn validate_snapshot(
    dir: &Path,
    expected_identity: &Identity,
    coverage: SnapshotCoverage,
) -> Result<ValidatedSnapshot, String> {
    check_identity(expected_identity)?;
    check_directory(dir)?;
    if dir.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.ends_with(".partial") || name.ends_with("-INCOMPLETE")
    }) {
        return Err("The Variax snapshot is incomplete.".into());
    }
    let mut required = BTreeSet::from([
        "identity.syx".to_owned(),
        "original-1.syx".to_owned(),
        "original-2.syx".to_owned(),
    ]);
    for preset in Preset::all() {
        let stem = variax_preset_stem(preset);
        required.extend([format!("{stem}-1.syx"), format!("{stem}-2.syx")]);
        if coverage == SnapshotCoverage::FullPrograms {
            required.insert(program_file(preset));
        }
    }
    required.extend((0..BANKS.len() as u8).map(variax_bank_name_file));

    // The permissive shared v1 verifier can open paths from the manifest.
    // Check an exact family allowlist before opening any listed path, and
    // hash the same bounded bytes passed to the Variax decoders.
    let manifest = read_file(&dir.join("MANIFEST.sha256"), MANIFEST_LIMIT)?;
    let contents = std::str::from_utf8(&manifest).map_err(|_| "Manifest is not UTF-8.")?;
    let mut hashes = BTreeMap::new();
    for line in contents.lines() {
        let (hash, name) = line
            .split_once("  ")
            .ok_or("Malformed snapshot manifest line.")?;
        if !required.contains(name) && !AUXILIARY_FILES.contains(&name) {
            return Err(format!("Unexpected snapshot manifest path {name:?}."));
        }
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Invalid SHA-256 for {name}."));
        }
        if hashes
            .insert(name.to_owned(), hash.to_ascii_lowercase())
            .is_some()
        {
            return Err(format!("Duplicate snapshot manifest path {name}."));
        }
    }
    if let Some(name) = required.iter().find(|name| !hashes.contains_key(*name)) {
        return Err(format!("Snapshot manifest is missing {name}."));
    }
    let directories = if coverage == SnapshotCoverage::FullPrograms {
        &["presets", "bank-names", "programs"][..]
    } else {
        &["presets", "bank-names"][..]
    };
    check_members(dir, "", directories, &hashes)?;
    for name in directories {
        check_members(dir, name, &[], &hashes)?;
    }

    let read = |name: &str, limit| -> Result<Vec<u8>, String> {
        let bytes = read_file(&dir.join(name), limit)?;
        if hashes.get(name).is_none_or(|hash| *hash != digest(&bytes)) {
            return Err(format!("{name} does not match MANIFEST.sha256."));
        }
        Ok(bytes)
    };
    let identity =
        Identity::parse(&read("identity.syx", 17)?).ok_or("Invalid snapshot identity.syx.")?;
    check_identity(&identity)?;
    if identity != *expected_identity {
        return Err("Snapshot identity does not match the expected device.".into());
    }
    for name in AUXILIARY_FILES
        .into_iter()
        .filter(|name| hashes.contains_key(*name))
    {
        read(name, AUXILIARY_LIMIT)?;
    }
    let pair = |stem: &str| -> Result<PresetMetadata, String> {
        PresetMetadata::from_replies(
            &read(&format!("{stem}-1.syx"), 200)?,
            &read(&format!("{stem}-2.syx"), 200)?,
        )
        .map_err(|error| format!("{stem}: {error}"))
    };
    let original = Some(pair("original")?);
    let presets = Preset::all()
        .map(|preset| Ok((preset, pair(&variax_preset_stem(preset))?)))
        .collect::<Result<_, String>>()?;
    let bank_names = (0..BANKS.len() as u8)
        .map(|bank| {
            let path = variax_bank_name_file(bank);
            let name = BankName::from_reply(read(&path, BANK_REPLY_LIMIT)?)
                .map_err(|error| format!("{path}: {error}"))?;
            Ok((bank, name))
        })
        .collect::<Result<_, String>>()?;
    let programs = match coverage {
        SnapshotCoverage::Metadata => SnapshotPrograms::MetadataOnly,
        SnapshotCoverage::FullPrograms => SnapshotPrograms::Full(
            Preset::all()
                .map(|preset| {
                    let path = program_file(preset);
                    let bytes = read(&path, PROGRAM_BYTES)?.try_into().map_err(|_| {
                        format!("{path} must contain {PROGRAM_BYTES} program bytes.")
                    })?;
                    check_program_checksum(&bytes).map_err(|error| format!("{path}: {error}"))?;
                    Ok((preset, bytes))
                })
                .collect::<Result<_, String>>()?,
        ),
    };

    Ok(ValidatedSnapshot {
        identity,
        library: LoadedVariax {
            original,
            presets,
            bank_names,
        },
        programs,
        hashes,
        manifest_sha256: digest(&manifest),
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn check_directory(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()) {
        Ok(())
    } else {
        Err(format!(
            "Expected a plain snapshot directory at {}.",
            path.display()
        ))
    }
}

fn check_members(
    root: &Path,
    prefix: &str,
    directories: &[&str],
    hashes: &BTreeMap<String, String>,
) -> Result<(), String> {
    let dir = root.join(prefix);
    check_directory(&dir)?;
    for entry in fs::read_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("Invalid snapshot file name.")?;
        if directories.contains(&name) {
            check_directory(&entry.path())?;
        } else {
            let path = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            if !(prefix.is_empty() && name == "MANIFEST.sha256" || hashes.contains_key(&path)) {
                return Err(format!("Unlisted snapshot file {path:?}."));
            }
        }
    }
    Ok(())
}

fn read_file(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let fail = |error: std::io::Error| format!("{}: {error}", path.display());
    let meta = fs::symlink_metadata(path).map_err(fail)?;
    if !meta.is_file() || meta.len() > max as u64 {
        return Err(format!(
            "Expected a regular snapshot file of at most {max} bytes at {}.",
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
    file.take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    if bytes.len() > max {
        return Err(format!(
            "Snapshot file exceeds {max} bytes at {}.",
            path.display()
        ));
    }
    Ok(bytes)
}

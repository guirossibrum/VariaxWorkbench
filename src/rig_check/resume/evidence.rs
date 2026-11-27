//! Validated local captures and backup manifests for a continuation.

use super::{GUITAR_BACKUP, POD_BACKUP};
use crate::rig_check::catalogue::StepId;
use crate::rig_check::practice::{self, Program, preset_label};
use crate::rig_check::procedures;
use crate::rig_check::record::{StepRecord, sha256_hex};
use crate::rig_check::recover::{self, RigState};
use crate::rig_check::steps::RunData;
use crate::rig_check::{differences, evidence::file_name};
use line6_devices::family::podxt::{self, BankKind, EditBuffer, Location};
use line6_devices::family::variax::Preset;
use line6_devices::identity::Identity;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub(super) fn identity_from_record(step: &StepRecord) -> Result<Identity, String> {
    let bytes = step
        .evidence
        .iter()
        .find_map(|line| line.strip_prefix("Identity reply: "))
        .and_then(|hex| {
            hex.split_whitespace()
                .map(|byte| u8::from_str_radix(byte, 16).ok())
                .collect::<Option<Vec<_>>>()
        })
        .and_then(|bytes| Identity::parse(&bytes));
    bytes.ok_or_else(|| format!("Step {} has no valid identity evidence.", step.id))
}

pub(super) fn check_fact(step: &StepRecord, expected: String) -> Result<(), String> {
    if step.evidence.contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "The checkpoint does not match the original result for {}.",
            step.id
        ))
    }
}

pub(super) fn check_hash(step: &StepRecord, label: &str, data: &[u8]) -> Result<(), String> {
    let actual = sha256_hex(data);
    if step
        .checksums
        .iter()
        .any(|(key, hash)| key == label && hash == &actual)
    {
        Ok(())
    } else {
        Err(format!("{label} differs from the report checksum."))
    }
}

pub(super) fn program(bytes: &[u8]) -> Result<Program, String> {
    let words = procedures::unpack_program(
        bytes
            .try_into()
            .map_err(|_| "Invalid Variax program size.")?,
    );
    if recover::match_hash(&words).is_none() {
        return Err("Variax program failed its firmware checksum.".into());
    }
    Ok(words)
}

pub(super) fn legacy_variax_hash(words: &Program) -> String {
    let mut copy = *words;
    for index in [871, 872, 1091] {
        copy[index] = 0;
    }
    sha256_hex(&procedures::program_bytes(&copy))
}

pub(super) struct Folder {
    pub(super) name: String,
    files: BTreeMap<String, Vec<u8>>,
}

impl Folder {
    pub(super) fn read(
        root: &Path,
        step: &StepRecord,
        base: &str,
        label: &str,
        legacy: bool,
    ) -> Result<Self, String> {
        let name = step.evidence_folder(base);
        let dir = root.join(&name);
        let root = root
            .canonicalize()
            .map_err(|error| format!("Evidence folder: {error}"))?;
        let dir = dir
            .canonicalize()
            .map_err(|error| format!("{name}: {error}"))?;
        if dir != root.join(&name) {
            return Err(format!(
                "Evidence folder {name} points outside its expected path."
            ));
        }
        let read = |name: &str| -> Result<Vec<u8>, String> {
            let path = dir.join(name);
            if path.canonicalize().is_ok_and(|resolved| resolved == path) {
                std::fs::read(&path).map_err(|error| format!("{name}: {error}"))
            } else {
                Err(format!("Missing or redirected evidence file {name}."))
            }
        };
        let manifest = read("MANIFEST.sha256")?;
        let manifest_hash = sha256_hex(&manifest);
        let hash = match (legacy, step.id) {
            (true, POD_BACKUP) => Some(&manifest_hash[..16]),
            (true, GUITAR_BACKUP) => Some(manifest_hash.as_str()),
            _ => None,
        };
        if let Some(hash) = hash {
            if !step
                .evidence
                .iter()
                .any(|line| line.contains(&format!("manifest SHA-256 {hash})")))
            {
                return Err(format!("{name} manifest differs from the legacy report."));
            }
        } else if !legacy {
            check_hash(step, label, &manifest)?;
        }
        let mut files = BTreeMap::new();
        for line in std::str::from_utf8(&manifest)
            .map_err(|_| "Invalid manifest text.")?
            .lines()
        {
            let (hash, file) = line
                .split_once("  ")
                .ok_or("Malformed evidence manifest.")?;
            let relative = Path::new(file);
            if file.is_empty()
                || file.contains(['\\', ':'])
                || !relative
                    .components()
                    .all(|p| matches!(p, Component::Normal(_)))
            {
                return Err("Invalid evidence manifest path.".into());
            }
            let bytes = read(file)?;
            if sha256_hex(&bytes) != hash || files.insert(file.to_owned(), bytes).is_some() {
                return Err(format!("{file} does not match its evidence manifest."));
            }
        }
        if files.is_empty() || files.insert("MANIFEST.sha256".into(), manifest).is_some() {
            return Err("Invalid evidence manifest entries.".into());
        }
        Ok(Self { name, files })
    }

    pub(super) fn file(&self, name: &str) -> Result<&[u8], String> {
        self.files
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("{name} is missing from {}.", self.name))
    }

    pub(super) fn copy_to(&self, root: &Path) -> Result<(), String> {
        let dir = root.join(&self.name);
        std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
        std::fs::create_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for (name, bytes) in &self.files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().expect("file has evidence folder"))
                .map_err(|error| error.to_string())?;
            crate::shared::document::save_new(&path, bytes)?;
        }
        Ok(())
    }
}

pub(super) fn load_podxt(
    root: &Path,
    step: &StepRecord,
    legacy: bool,
    _state: &RigState,
    data: &mut RunData,
) -> Result<Folder, String> {
    let folder = Folder::read(
        root,
        step,
        "podxt-library",
        "PODxt library manifest",
        legacy,
    )?;
    if Identity::parse(folder.file("identity.syx")?) != data.podxt_identity {
        return Err("PODxt backup identity differs from the starting identity.".into());
    }
    let before = EditBuffer::parse(folder.file("current-before.syx")?.to_vec())?;
    let after = EditBuffer::parse(folder.file("current-after.syx")?.to_vec())?;
    if !differences::same_podxt_tone(&before.raw, &after.raw) {
        return Err("The PODxt backup's closing tone differs from its opening tone.".into());
    }
    let library = Location::all(&BankKind::ALL)
        .map(|location| {
            let name = podxt::backup::podxt_entry_name(location);
            Ok((location, EditBuffer::parse(folder.file(&name)?.to_vec())?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let all: Vec<u8> = library
        .iter()
        .flat_map(|(_, tone)| tone.raw.iter().copied())
        .collect();
    check_hash(step, "PODxt library (299 dumps)", &all)?;
    let start = data
        .start_tone
        .as_ref()
        .ok_or("No verified PODxt starting tone.")?;
    let channel = data
        .start_channel
        .ok_or("No verified PODxt starting channel.")?;
    let saved = &library
        .iter()
        .find(|(entry, _)| *entry == channel)
        .ok_or("No starting channel in the PODxt backup.")?
        .1;
    data.podxt_unsaved = !differences::same_podxt_tone(&start.raw, &saved.raw);
    data.podxt_library = library;
    data.podxt_backup_dir = Some(PathBuf::from(&folder.name));
    Ok(folder)
}

pub(super) fn load_variax(
    root: &Path,
    step: &StepRecord,
    start_step: &StepRecord,
    legacy: bool,
    state: &RigState,
    data: &mut RunData,
) -> Result<Folder, String> {
    let folder = Folder::read(
        root,
        step,
        "variax-library",
        "Variax library manifest",
        legacy,
    )?;
    let library = (0..60)
        .map(|index| {
            let preset = Preset::new(index / 5, 4 - index % 5).expect("Variax library index");
            let label = preset_label(preset);
            let file = format!(
                "{:02}-{}.bin",
                practice::flash_index(preset),
                file_name(&label)
            );
            let bytes = folder.file(&file)?;
            check_hash(step, &format!("Variax {label} (1,092 words)"), bytes)?;
            Ok((preset, program(bytes)?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let start = data
        .start_program
        .as_ref()
        .ok_or("No verified Variax starting program.")?;
    let refreshed = start_step
        .evidence
        .iter()
        .any(|line| line == "Using the current Variax program for this continuation.");
    if let Some(preset) = state.preset() {
        let saved = &library
            .iter()
            .find(|(entry, _)| *entry == preset)
            .ok_or("No starting preset in the Variax backup.")?
            .1;
        if refreshed {
            check_fact(
                start_step,
                format!("Current Variax preset: {}", preset_label(preset)),
            )?;
        } else {
            check_fact(step, format!("Starting preset: {}", preset_label(preset)))?;
        }
        data.variax_unsaved = !procedures::same_program(start, saved);
        data.start_preset = Some(preset);
    } else if refreshed {
        check_fact(
            start_step,
            "Current Variax preset: not identified from the backup.".into(),
        )?;
    } else {
        return Err("No starting Variax preset in the checkpoint.".into());
    }
    data.variax_library = library;
    data.variax_backup_folder = Some(folder.name.clone());
    Ok(folder)
}

pub(super) fn recheck_channel(records: &[StepRecord], data: &mut RunData) -> Result<(), String> {
    let prior = records
        .iter()
        .find(|step| step.id == StepId(2, 3))
        .expect("checked step");
    let bytes = prior
        .evidence
        .iter()
        .filter_map(|line| line.strip_prefix("  "))
        .find_map(|hex| {
            hex.split_whitespace()
                .map(|byte| u8::from_str_radix(byte, 16).ok())
                .collect::<Option<Vec<_>>>()
        })
        .ok_or("Missing the independent channel 1A dump from step 2.3.")?;
    check_hash(prior, "PODxt channel 1A (168 bytes)", &bytes)?;
    data.podxt_channel_read = Some(Ok(EditBuffer::parse(bytes)?));
    Ok(())
}

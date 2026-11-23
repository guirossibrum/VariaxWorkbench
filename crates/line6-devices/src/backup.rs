//! Recoverable library backups on disk, shared by every front end (the
//! `variax-probe` CLI and the app), so both write and read the same layout.
//!
//! A backup folder is written as `<name>.partial` and renamed to `<name>`
//! (or `<name>-INCOMPLETE`) only when finished. It holds every device reply
//! as an exact `.syx` file, `index.txt`, `log.txt`, `RESTORE.txt` and
//! `MANIFEST.sha256` (hashes read back from disk, `sha256sum -c` format).
//! Every file is made read-only; nothing is ever overwritten. Loading checks
//! the manifest before decoding anything.
//!
//! Layout (format v1):
//! * Variax: `identity.syx`, `original-1.syx`/`original-2.syx`, and
//!   `presets/BB-Bank_Name-S-1.syx`/`-2.syx` (the two 200-byte metadata
//!   replies of bank BB, slot S).
//! * PODxt: `identity.syx`, `current-before.syx`, `current-after.syx`, and
//!   `tones/NNN-LABEL.syx`, `amp-setups/NNN-N.syx`, `fx-setups/NNN-N.syx`
//!   (168-byte dumps).

use crate::family::{podxt, variax};
use crate::session::Log;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "line6-devices backup v1";

pub const VARIAX_RESTORE: &str =
    "Variax 600 library backup (read-only copy, line6-devices backup v1).

presets/BB-Bank-S-1.syx and -2.syx are the exact 200-byte SysEx replies the guitar
sent for metadata blocks 1 (address 851) and 2 (address 883) of bank BB, slot S,
after a program change to that preset. index.txt lists bank, slot, Workbench
program number, preset name and body model. original-1/2.syx is the preset that
was active before the read. Load with line6_devices::backup::load_variax_library,
or decode one preset with variax::PresetMetadata::from_replies.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   read the library again and compare the two folders
                   (line6_devices::backup::compare, `variax-probe compare`).

These blocks are current-preset metadata as Workbench reads it, not a DSP or
firmware image. Restoring them to the guitar needs a write gate (G6/G7), the
owner's direct permission and an approved restore procedure.
";

pub const PODXT_RESTORE: &str =
    "PODxt Live library backup (read-only copy, line6-devices backup v1).

tones/NNN-LABEL.syx, amp-setups/NNN-N.syx and fx-setups/NNN-N.syx are the exact
168-byte SysEx dumps (F0 00 01 0C 03 74 .. F7) the PODxt sent when asked for each
stored location by address (Line 6 Edit 3.06 RequestPatch layout: name at bytes
7..22, controller n at byte 39+n). index.txt lists location, name and amp.
current-before.syx and current-after.syx are the active tone before and after.
Load with line6_devices::backup::load_podxt_library, or decode one dump with
podxt::EditBuffer::parse.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   read the library again and compare the two folders
                   (line6_devices::backup::compare, `variax-probe compare`).

Restoring to the PODxt needs a write gate (G6/G7), the owner's direct
permission and an approved restore procedure.
";

type Echo = Box<dyn FnMut(&str)>;

/// One run folder. Implements [`Log`], so it can be handed to a session.
pub struct BackupFolder {
    root: PathBuf,
    name: String,
    work: PathBuf,
    lines: Vec<String>,
    files: Vec<String>,
    restore_note: Option<String>,
    echo: Option<Echo>,
}

impl BackupFolder {
    /// A new `<name>.partial` under `root`. Fails if `<name>`,
    /// `<name>-INCOMPLETE` or `<name>.partial` already exists.
    pub fn create(root: &Path, name: &str) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
        for taken in [name.to_owned(), format!("{name}-INCOMPLETE")] {
            if root.join(&taken).exists() {
                return Err(format!("{} already exists", root.join(taken).display()));
            }
        }
        let work = root.join(format!("{name}.partial"));
        fs::create_dir(&work).map_err(|e| format!("{}: {e}", work.display()))?;
        Ok(Self {
            root: root.to_owned(),
            name: name.to_owned(),
            work,
            lines: Vec::new(),
            files: Vec::new(),
            restore_note: None,
            echo: None,
        })
    }

    /// Also passes every logged line to `echo` (for example to print it).
    pub fn with_echo(mut self, echo: impl FnMut(&str) + 'static) -> Self {
        self.echo = Some(Box::new(echo));
        self
    }

    /// Keeps a line for log.txt without echoing it.
    pub fn record(&mut self, text: String) {
        self.lines.push(text);
    }

    pub fn set_restore_note(&mut self, text: &str) {
        self.restore_note = Some(text.to_owned());
    }

    /// Writes one new file (create-new, synced). `name` may contain folders.
    pub fn file(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.work.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        self.files.push(name.to_owned());
        Ok(())
    }

    /// Writes log, restore note and manifest, makes every file read-only and
    /// renames the folder (`-INCOMPLETE` unless `complete`). Returns it.
    pub fn finish(mut self, complete: bool) -> Result<PathBuf, String> {
        let mut log = self.lines.join("\n");
        log.push('\n');
        self.file("log.txt", log.as_bytes())?;
        if let Some(note) = self.restore_note.take() {
            self.file("RESTORE.txt", note.as_bytes())?;
        }
        let mut files = self.files.clone();
        files.sort();
        let mut manifest = String::new();
        for name in &files {
            let bytes = fs::read(self.work.join(name)).map_err(|e| format!("{name}: {e}"))?;
            manifest.push_str(&format!("{}  {name}\n", sha256_hex(&bytes)));
        }
        self.file("MANIFEST.sha256", manifest.as_bytes())?;
        for name in &self.files {
            let path = self.work.join(name);
            let mut permissions = fs::metadata(&path)
                .map_err(|e| format!("{name}: {e}"))?
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&path, permissions).map_err(|e| format!("{name}: {e}"))?;
        }
        let target = self.root.join(if complete {
            self.name.clone()
        } else {
            format!("{}-INCOMPLETE", self.name)
        });
        if target.exists() {
            return Err(format!(
                "{} already exists; data left in {}",
                target.display(),
                self.work.display()
            ));
        }
        fs::rename(&self.work, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(target)
    }
}

impl Log for BackupFolder {
    fn line(&mut self, text: String) {
        if let Some(echo) = self.echo.as_mut() {
            echo(&text);
        }
        self.lines.push(text);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `YYYYMMDDTHHMMSSZ` and `YYYY-MM-DD HH:MM:SS UTC` for a Unix time.
pub fn utc(seconds: u64) -> (String, String) {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let (h, m, s) = (rest / 3600, rest % 3600 / 60, rest % 60);
    // Civil-from-days (H. Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        format!("{year:04}{month:02}{day:02}T{h:02}{m:02}{s:02}Z"),
        format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC"),
    )
}

// ---- layout and index lines ----

pub fn variax_preset_stem(preset: variax::Preset) -> String {
    format!(
        "presets/{:02}-{}-{}",
        preset.bank(),
        preset.bank_name().replace(' ', "_"),
        preset.slot() + 1
    )
}

pub fn podxt_entry_name(location: podxt::Location) -> String {
    format!(
        "{}/{:03}-{}.syx",
        location.kind().slug(),
        location.index() + 1,
        location.label().replace('#', "")
    )
}

pub fn variax_index_line(preset: variax::Preset, metadata: &variax::PresetMetadata) -> String {
    let body = match metadata.parameters.get("body_model") {
        Some(variax::ParameterValue::Integer(id)) => format!("{id:#010x}"),
        _ => "?".into(),
    };
    format!(
        "{:>8} {} (program {:3}): {:<18} body {body}",
        preset.bank_name(),
        preset.slot() + 1,
        preset.program_number(),
        metadata.name
    )
}

pub fn podxt_index_line(stored: &podxt::StoredTone) -> String {
    format!(
        "{:<10} {:>4}: {:<16} amp {}",
        stored.location.kind().name(),
        stored.location.label(),
        stored.tone.name(),
        stored.tone.amp().name
    )
}

// ---- save ----

/// Writes a Variax library read in the v1 layout, plus index and restore note.
pub fn save_variax_library(
    folder: &mut BackupFolder,
    library: &variax::Library,
) -> Result<(), String> {
    folder.file("identity.syx", &library.identified.reply)?;
    folder.file("original-1.syx", &library.original_replies[0])?;
    folder.file("original-2.syx", &library.original_replies[1])?;
    let mut index = String::new();
    for stored in &library.presets {
        let stem = variax_preset_stem(stored.preset);
        folder.file(&format!("{stem}-1.syx"), &stored.replies[0])?;
        folder.file(&format!("{stem}-2.syx"), &stored.replies[1])?;
        index.push_str(&variax_index_line(stored.preset, &stored.metadata));
        index.push('\n');
    }
    folder.file("index.txt", index.as_bytes())?;
    folder.set_restore_note(VARIAX_RESTORE);
    Ok(())
}

/// Writes a PODxt library read in the v1 layout, plus index and restore note.
pub fn save_podxt_library(
    folder: &mut BackupFolder,
    library: &podxt::Library,
) -> Result<(), String> {
    folder.file("identity.syx", &library.identity_reply)?;
    folder.file("current-before.syx", &library.before.raw)?;
    if let Ok(after) = &library.after {
        folder.file("current-after.syx", &after.raw)?;
    }
    let mut index = String::new();
    for stored in &library.entries {
        folder.file(&podxt_entry_name(stored.location), &stored.tone.raw)?;
        index.push_str(&podxt_index_line(stored));
        index.push('\n');
    }
    folder.file("index.txt", index.as_bytes())?;
    folder.set_restore_note(PODXT_RESTORE);
    Ok(())
}

// ---- load ----

/// Checks every line of `MANIFEST.sha256`; returns how many files matched.
pub fn verify_manifest(dir: &Path) -> Result<usize, String> {
    let path = dir.join("MANIFEST.sha256");
    let manifest = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut checked = 0;
    for line in manifest.lines().filter(|line| !line.is_empty()) {
        let (hash, name) = line
            .split_once("  ")
            .ok_or_else(|| format!("malformed manifest line: {line}"))?;
        if name.contains("..") || name.starts_with('/') {
            return Err(format!("manifest names a path outside the folder: {name}"));
        }
        let bytes = fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
        if sha256_hex(&bytes) != hash {
            return Err(format!("{name} does not match MANIFEST.sha256"));
        }
        checked += 1;
    }
    if checked == 0 {
        return Err("MANIFEST.sha256 lists no files".into());
    }
    Ok(checked)
}

fn read_optional(dir: &Path, name: &str) -> Result<Option<Vec<u8>>, String> {
    let path = dir.join(name);
    if path.exists() {
        fs::read(&path)
            .map(Some)
            .map_err(|e| format!("{name}: {e}"))
    } else {
        Ok(None)
    }
}

pub struct LoadedVariax {
    pub original: Option<variax::PresetMetadata>,
    pub presets: Vec<(variax::Preset, variax::PresetMetadata)>,
}

/// Verifies the manifest, then decodes every preset present in the folder.
pub fn load_variax_library(dir: &Path) -> Result<LoadedVariax, String> {
    verify_manifest(dir)?;
    let pair = |first: &str, second: &str| -> Result<Option<variax::PresetMetadata>, String> {
        match (read_optional(dir, first)?, read_optional(dir, second)?) {
            (Some(a), Some(b)) => variax::PresetMetadata::from_replies(&a, &b)
                .map(Some)
                .map_err(|e| format!("{first}: {e}")),
            (None, None) => Ok(None),
            _ => Err(format!("{first} and {second} must both be present")),
        }
    };
    let original = pair("original-1.syx", "original-2.syx")?;
    let mut presets = Vec::new();
    for preset in variax::Preset::all() {
        let stem = variax_preset_stem(preset);
        if let Some(metadata) = pair(&format!("{stem}-1.syx"), &format!("{stem}-2.syx"))? {
            presets.push((preset, metadata));
        }
    }
    Ok(LoadedVariax { original, presets })
}

pub struct LoadedPodxt {
    pub before: Option<podxt::EditBuffer>,
    pub after: Option<podxt::EditBuffer>,
    pub entries: Vec<podxt::StoredTone>,
}

/// Verifies the manifest, then decodes every stored entry present.
pub fn load_podxt_library(dir: &Path) -> Result<LoadedPodxt, String> {
    verify_manifest(dir)?;
    let dump = |name: &str| -> Result<Option<podxt::EditBuffer>, String> {
        read_optional(dir, name)?
            .map(|bytes| podxt::EditBuffer::parse(bytes).map_err(|e| format!("{name}: {e}")))
            .transpose()
    };
    let mut entries = Vec::new();
    for location in podxt::Location::all(&podxt::BankKind::ALL) {
        if let Some(tone) = dump(&podxt_entry_name(location))? {
            entries.push(podxt::StoredTone { location, tone });
        }
    }
    Ok(LoadedPodxt {
        before: dump("current-before.syx")?,
        after: dump("current-after.syx")?,
        entries,
    })
}

// ---- compare ----

const LIBRARY_FOLDERS: [&str; 4] = ["presets", "tones", "amp-setups", "fx-setups"];

fn library_files(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    for folder in LIBRARY_FOLDERS {
        let path = dir.join(folder);
        if !path.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".syx") {
                let bytes = fs::read(entry.path()).map_err(|e| format!("{name}: {e}"))?;
                files.insert(format!("{folder}/{name}"), bytes);
            }
        }
    }
    Ok(files)
}

pub struct Comparison {
    pub identical: usize,
    pub problems: Vec<String>,
}

impl Comparison {
    /// Both folders hold the same non-empty set of stored entries, byte for byte.
    pub fn verified(&self) -> bool {
        self.problems.is_empty() && self.identical > 0
    }
}

/// Compares the stored entries of two library backups byte for byte
/// (current/original tones are excluded: they are live state, not storage).
pub fn compare(a: &Path, b: &Path) -> Result<Comparison, String> {
    let (left, right) = (library_files(a)?, library_files(b)?);
    let mut identical = 0;
    let mut problems = Vec::new();
    for (name, bytes) in &left {
        match right.get(name) {
            Some(other) if other == bytes => identical += 1,
            Some(_) => problems.push(format!("differs: {name}")),
            None => problems.push(format!("only in first: {name}")),
        }
    }
    for name in right.keys().filter(|name| !left.contains_key(*name)) {
        problems.push(format!("only in second: {name}"));
    }
    Ok(Comparison {
        identical,
        problems,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::variax::tests::encode_reply;
    use crate::identity::{
        Identity,
        tests::{PODXT_LIVE, VARIAX_600},
    };

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("line6-devices-{tag}-{}", std::process::id()));
            let _ = make_writable_and_remove(&path);
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = make_writable_and_remove(&self.0);
        }
    }

    fn make_writable_and_remove(path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }
        fn walk(path: &Path) -> std::io::Result<()> {
            for entry in fs::read_dir(path)? {
                let entry = entry?.path();
                if entry.is_dir() {
                    walk(&entry)?;
                } else {
                    let mut p = fs::metadata(&entry)?.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    p.set_readonly(false);
                    fs::set_permissions(&entry, p)?;
                }
            }
            Ok(())
        }
        walk(path)?;
        fs::remove_dir_all(path)
    }

    fn variax_library() -> variax::Library {
        let reply = |name: &[u8]| {
            let mut raw = [0u8; 96];
            raw[..name.len()].copy_from_slice(name);
            encode_reply(&raw)
        };
        let zero = encode_reply(&[0; 96]);
        let presets = variax::Preset::all()
            .take(3)
            .map(|preset| {
                let first = reply(format!("P{}", preset.program_number()).as_bytes());
                let metadata = variax::PresetMetadata::from_replies(&first, &zero).unwrap();
                variax::StoredPreset {
                    preset,
                    replies: [first, zero.clone()],
                    metadata,
                }
            })
            .collect();
        let original_first = reply(b"P4");
        variax::Library {
            identified: variax::Identified {
                reply: VARIAX_600.to_vec(),
                identity: Identity::parse(&VARIAX_600).unwrap(),
                requests: 1,
            },
            original: variax::PresetMetadata::from_replies(&original_first, &zero).unwrap(),
            original_replies: [original_first, zero.clone()],
            presets,
            restore: variax::Restore::NoUniqueMatch { matches: 0 },
            stopped: None,
        }
    }

    fn podxt_dump(name: &[u8]) -> podxt::EditBuffer {
        let mut raw = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74, 0x0a];
        let mut padded = name.to_vec();
        padded.resize(16, b' ');
        raw.extend(padded);
        raw.resize(podxt::EDIT_BUFFER_LEN - 1, 0);
        raw.push(0xf7);
        podxt::EditBuffer::parse(raw).unwrap()
    }

    fn podxt_library() -> podxt::Library {
        let entries = podxt::Location::all(&[podxt::BankKind::FxSetups])
            .take(4)
            .map(|location| podxt::StoredTone {
                location,
                tone: podxt_dump(format!("FX {}", location.index()).as_bytes()),
            })
            .collect();
        podxt::Library {
            identity_reply: PODXT_LIVE.to_vec(),
            before: podxt_dump(b"Current"),
            after: Ok(podxt_dump(b"Current")),
            entries,
            stopped: None,
        }
    }

    #[test]
    fn utc_formats_known_instants() {
        assert_eq!(utc(0).1, "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400).0, "20000229T000000Z");
        assert_eq!(utc(1_795_408_934).1, "2026-11-23 04:42:14 UTC");
    }

    #[test]
    fn variax_backup_round_trips_through_the_manifest() {
        let root = TempRoot::new("variax");
        let library = variax_library();
        let mut folder = BackupFolder::create(&root.0, "lib").unwrap();
        save_variax_library(&mut folder, &library).unwrap();
        let dir = folder.finish(true).unwrap();
        assert_eq!(dir, root.0.join("lib"));
        assert!(
            fs::metadata(dir.join("index.txt"))
                .unwrap()
                .permissions()
                .readonly()
        );
        let loaded = load_variax_library(&dir).unwrap();
        assert_eq!(loaded.presets.len(), 3);
        assert_eq!(loaded.original.unwrap().name, "P4");
        for ((preset, metadata), stored) in loaded.presets.iter().zip(&library.presets) {
            assert_eq!(*preset, stored.preset);
            assert_eq!(metadata.raw, stored.metadata.raw);
        }
        assert!(compare(&dir, &dir).unwrap().verified());
        assert!(BackupFolder::create(&root.0, "lib").is_err());
    }

    #[test]
    fn podxt_backup_round_trips_and_detects_tampering() {
        let root = TempRoot::new("podxt");
        let mut folder = BackupFolder::create(&root.0, "lib").unwrap();
        save_podxt_library(&mut folder, &podxt_library()).unwrap();
        let dir = folder.finish(true).unwrap();
        let loaded = load_podxt_library(&dir).unwrap();
        assert_eq!(loaded.entries.len(), 4);
        assert_eq!(loaded.entries[2].tone.name(), "FX 2");
        assert_eq!(loaded.before.unwrap().name(), "Current");

        let victim = dir.join(podxt_entry_name(loaded.entries[0].location));
        let mut p = fs::metadata(&victim).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        fs::set_permissions(&victim, p).unwrap();
        let mut bytes = fs::read(&victim).unwrap();
        bytes[10] ^= 1;
        fs::write(&victim, bytes).unwrap();
        assert!(
            verify_manifest(&dir)
                .unwrap_err()
                .contains("does not match")
        );
        assert!(load_podxt_library(&dir).is_err());
    }

    #[test]
    fn incomplete_runs_are_marked_and_compare_reports_differences() {
        let root = TempRoot::new("compare");
        let mut a = BackupFolder::create(&root.0, "a").unwrap();
        save_podxt_library(&mut a, &podxt_library()).unwrap();
        let a = a.finish(true).unwrap();
        let mut library = podxt_library();
        library.entries.pop();
        let mut b = BackupFolder::create(&root.0, "b").unwrap();
        save_podxt_library(&mut b, &library).unwrap();
        let b = b.finish(false).unwrap();
        assert_eq!(b, root.0.join("b-INCOMPLETE"));
        let comparison = compare(&a, &b).unwrap();
        assert!(!comparison.verified());
        assert_eq!(comparison.identical, 3);
        assert_eq!(comparison.problems.len(), 1);
    }
}

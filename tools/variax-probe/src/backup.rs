//! Run folders that can be trusted later.
//!
//! Each run writes into `<name>.partial`. When everything is written the
//! folder gets `log.txt`, an optional `RESTORE.txt`, and `MANIFEST.sha256`
//! (hashes read back from disk, `sha256sum -c` format); every file is made
//! read-only and the folder is renamed to `<name>` — or `<name>-INCOMPLETE`
//! when the run stopped early. Nothing is ever overwritten.

use line6_devices::session::Log;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Capture {
    root: PathBuf,
    name: String,
    work: PathBuf,
    lines: Vec<String>,
    files: Vec<String>,
    restore_note: Option<String>,
}

impl Capture {
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
        })
    }

    /// Keeps a line for log.txt that was already printed.
    pub fn record(&mut self, text: String) {
        self.lines.push(text);
    }

    pub fn set_restore_note(&mut self, text: String) {
        self.restore_note = Some(text);
    }

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

    /// Writes log, restore note and manifest, makes everything read-only and
    /// renames the folder. Returns the final folder.
    pub fn finish(mut self, result: &Result<(), String>) -> Result<PathBuf, String> {
        if let Err(error) = result {
            self.line(format!("STOPPED: {error}"));
        }
        let mut log = self.lines.join("\n");
        log.push('\n');
        self.file("log.txt", log.as_bytes())?;
        if let Some(note) = self.restore_note.take() {
            self.file("RESTORE.txt", note.as_bytes())?;
        }
        let mut manifest = String::new();
        let mut files = self.files.clone();
        files.sort();
        for name in &files {
            let bytes = fs::read(self.work.join(name)).map_err(|e| format!("{name}: {e}"))?;
            manifest.push_str(&format!("{}  {name}\n", hex(&Sha256::digest(&bytes))));
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
        let target = self.root.join(if result.is_ok() {
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
        let _ = writeln!(std::io::stdout(), "Capture: {}", target.display());
        Ok(target)
    }
}

impl Log for Capture {
    fn line(&mut self, text: String) {
        // Never panic on a closed stdout (for example `| head`): the run must
        // still finish its cleanup (session off, interface give-back).
        let _ = writeln!(std::io::stdout(), "{text}");
        self.lines.push(text);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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

/// Stored-entry folders a library backup contains.
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

/// Offline: compares the stored entries of two library backups byte for
/// byte. `true` when both hold the same non-empty set and every file matches.
pub fn compare(a: &Path, b: &Path) -> Result<(Vec<String>, bool), String> {
    let (left, right) = (library_files(a)?, library_files(b)?);
    let mut identical = 0;
    let mut lines = Vec::new();
    for (name, bytes) in &left {
        match right.get(name) {
            Some(other) if other == bytes => identical += 1,
            Some(_) => lines.push(format!("  differs: {name}")),
            None => lines.push(format!("  only in first: {name}")),
        }
    }
    for name in right.keys().filter(|name| !left.contains_key(*name)) {
        lines.push(format!("  only in second: {name}"));
    }
    let verified = lines.is_empty() && identical > 0;
    lines.insert(
        0,
        format!(
            "{identical} stored entries identical, {} problems: {}",
            lines.len(),
            if verified {
                "VERIFIED (two independent reads match)"
            } else {
                "NOT verified"
            }
        ),
    );
    Ok((lines, verified))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formats_known_instants() {
        assert_eq!(utc(0).1, "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400).0, "20000229T000000Z");
        assert_eq!(utc(1_795_408_934).1, "2026-11-23 04:42:14 UTC");
    }

    #[test]
    fn run_folder_is_renamed_hashed_read_only_and_never_reused() {
        let root = std::env::temp_dir().join(format!("variax-probe-test-{}", std::process::id()));
        let mut capture = Capture::create(&root, "run").unwrap();
        capture.file("tones/001-1A.syx", &[0xf0, 0xf7]).unwrap();
        capture.set_restore_note("note".into());
        let dir = capture.finish(&Ok(())).unwrap();
        assert_eq!(dir, root.join("run"));
        let manifest = fs::read_to_string(dir.join("MANIFEST.sha256")).unwrap();
        assert!(manifest.contains("  tones/001-1A.syx\n") && manifest.contains("  RESTORE.txt\n"));
        assert!(
            fs::metadata(dir.join("tones/001-1A.syx"))
                .unwrap()
                .permissions()
                .readonly()
        );
        assert!(Capture::create(&root, "run").is_err());

        let stopped = Capture::create(&root, "other").unwrap();
        let dir = stopped.finish(&Err("stopped".into())).unwrap();
        assert_eq!(dir, root.join("other-INCOMPLETE"));

        let (lines, verified) = compare(&root.join("run"), &root.join("run")).unwrap();
        assert!(verified, "{lines:?}");
        let (_, verified) = compare(&root.join("run"), &root.join("other-INCOMPLETE")).unwrap();
        assert!(!verified);
        // Test folders are read-only; make them writable again before removal.
        for path in [root.join("run"), root.join("other-INCOMPLETE")] {
            for entry in walk(&path) {
                let mut p = fs::metadata(&entry).unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                p.set_readonly(false);
                fs::set_permissions(&entry, p).unwrap();
            }
        }
        fs::remove_dir_all(&root).unwrap();
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }
}

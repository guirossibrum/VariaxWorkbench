//! The optional evidence folder: backups and captures, kept apart from the
//! report (#249). Files are new, never overwritten, and listed in a
//! `MANIFEST.sha256` per folder.

use super::record::sha256_hex;
use std::path::Path;

/// Write `files` into `root/folder` with a manifest. Returns the manifest's
/// SHA-256. The folder must not exist yet.
pub fn save_folder(
    root: &Path,
    folder: &str,
    files: &[(String, Vec<u8>)],
) -> Result<String, String> {
    let dir = root.join(folder);
    std::fs::create_dir_all(root).map_err(|e| format!("{}: {e}", root.display()))?;
    std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut manifest = String::new();
    for (name, bytes) in files {
        if name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(format!("Invalid evidence file name {name:?}."));
        }
        crate::shared::document::save_new(&dir.join(name), bytes)?;
        manifest.push_str(&format!("{}  {name}\n", sha256_hex(bytes)));
    }
    crate::shared::document::save_new(&dir.join("MANIFEST.sha256"), manifest.as_bytes())?;
    Ok(sha256_hex(manifest.as_bytes()))
}

/// A file-name-safe version of a label.
pub fn file_name(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_new_and_listed() {
        let root = tempfile::tempdir().unwrap();
        let files = vec![("a.bin".to_string(), vec![1, 2, 3])];
        save_folder(root.path(), "backup", &files).unwrap();
        let manifest = std::fs::read_to_string(root.path().join("backup/MANIFEST.sha256")).unwrap();
        assert!(manifest.contains("  a.bin"));
        assert!(
            save_folder(root.path(), "backup", &files).is_err(),
            "never overwrites"
        );
        assert!(save_folder(root.path(), "other", &[("../x".into(), vec![])]).is_err());
        assert_eq!(file_name("Spank 1"), "Spank-1");
    }
}

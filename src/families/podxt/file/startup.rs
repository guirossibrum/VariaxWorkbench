//! Remember complete PODxt library backups for offline startup.

use line6_devices::{
    backup::{LoadedPodxt, load_podxt_library},
    family::podxt::{BankKind, Location, verify_tone_backup},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Deserialize, Serialize)]
struct BackupRecord {
    path: PathBuf,
    completed: SystemTime,
}

fn records_directory(preferences: &Path) -> PathBuf {
    preferences.with_file_name("podxt-library-backups")
}

pub fn load_complete_podxt_backup(path: &Path) -> Result<LoadedPodxt, String> {
    verify_tone_backup(
        path,
        Location::new(BankKind::Tones, 0).expect("first tone slot"),
    )?;
    let backup = load_podxt_library(path)?;
    if backup.entries.len()
        != BankKind::ALL
            .into_iter()
            .map(|bank| usize::from(bank.size()))
            .sum::<usize>()
    {
        return Err("The PODxt Live backup must contain all 299 entries.".into());
    }
    Ok(backup)
}

pub fn latest_podxt_backup(preferences: &Path) -> Option<LoadedPodxt> {
    let mut records: Vec<BackupRecord> = fs::read_dir(records_directory(preferences))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| serde_json::from_slice(&fs::read(entry.path()).ok()?).ok())
        .collect();
    records.sort_by_key(|record| std::cmp::Reverse(record.completed));
    records
        .iter()
        .find_map(|record| load_complete_podxt_backup(&record.path).ok())
}

pub fn remember_podxt_backup(preferences: &Path, path: &Path) -> Result<(), String> {
    load_complete_podxt_backup(path)?;
    let path = fs::canonicalize(path).map_err(|error| error.to_string())?;
    let completed = fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| error.to_string())?;
    let directory = records_directory(preferences);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut temporary = tempfile::Builder::new()
        .prefix("backup-")
        .tempfile_in(&directory)
        .map_err(|error| error.to_string())?;
    let target = temporary.path().with_extension("json");
    serde_json::to_writer(&mut temporary, &BackupRecord { path, completed })
        .map_err(|error| error.to_string())?;
    temporary
        .persist_noclobber(target)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

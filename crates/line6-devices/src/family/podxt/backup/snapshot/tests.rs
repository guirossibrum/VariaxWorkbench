use super::*;
use crate::backup::BackupFolder;
use crate::family::podxt::{LIVE_STATE_CCS, PODXT_LIVE_VERSION};
use crate::identity::tests::PODXT_LIVE;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "podxt-snapshot-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        #[cfg(windows)]
        fn clear_readonly(dir: &Path) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    clear_readonly(&entry.path());
                } else if kind.is_file() {
                    let mut permissions = entry.metadata().unwrap().permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(entry.path(), permissions).unwrap();
                }
            }
        }
        #[cfg(windows)]
        clear_readonly(&self.0);
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn identity_reply() -> Vec<u8> {
    let mut reply = PODXT_LIVE.to_vec();
    reply[12..16].copy_from_slice(&PODXT_LIVE_VERSION);
    reply
}

fn identity() -> Identity {
    Identity::parse(&identity_reply()).unwrap()
}

fn dump(address: u16) -> EditBuffer {
    let mut bytes = vec![0; EDIT_BUFFER_LEN];
    bytes[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 10]);
    bytes[7..16].copy_from_slice(b"Synthetic");
    bytes[39 + 13] = (address & 0x7f) as u8;
    bytes[39 + 14] = (address >> 7) as u8;
    bytes[EDIT_BUFFER_LEN - 1] = 0xf7;
    EditBuffer::parse(bytes).unwrap()
}

fn capture() -> Library {
    let before = dump(0);
    let mut after = before.clone();
    for cc in LIVE_STATE_CCS {
        after.raw[39 + usize::from(cc)] = 1;
    }
    Library {
        identity_reply: identity_reply(),
        before,
        after: Ok(after),
        entries: Location::all(&BankKind::ALL)
            .map(|location| StoredTone {
                location,
                tone: dump(location.address()),
            })
            .collect(),
        stopped: None,
    }
}

fn fixture(
    root: &FixtureRoot,
    name: &str,
    library: &Library,
    complete: bool,
    mut alter: impl FnMut(&str, &[u8]) -> Option<Vec<u8>>,
) -> PathBuf {
    let mut folder = BackupFolder::create(&root.0, name).unwrap();
    let mut add = |name: &str, bytes: &[u8]| {
        if let Some(bytes) = alter(name, bytes) {
            folder.file(name, &bytes).unwrap();
        }
    };
    add(IDENTITY_FILE, &library.identity_reply);
    add(BEFORE, &library.before.raw);
    if let Ok(after) = &library.after {
        add(AFTER, &after.raw);
    }
    for entry in &library.entries {
        add(&podxt_entry_name(entry.location), &entry.tone.raw);
    }
    // A log line cannot prove release, even when manifested.
    folder.record("Interface released successfully.".to_owned());
    folder.finish(complete).unwrap()
}

fn replace(path: &Path, bytes: &[u8]) {
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).unwrap();
    }
    fs::remove_file(path).unwrap();
    fs::write(path, bytes).unwrap();
}

fn validation_error(path: &Path) -> String {
    validate_snapshot(path, &identity())
        .err()
        .expect("snapshot must fail")
}

#[test]
fn complete_v1_snapshot_checks_299_unique_addresses_and_allows_live_drift() {
    let root = FixtureRoot::new();
    let library = capture();
    let mut folder = BackupFolder::create(&root.0, "complete").unwrap();
    super::super::save_podxt_library(&mut folder, &library).unwrap();
    let dir = folder.finish(true).unwrap();
    let snapshot = validate_snapshot(&dir, &identity()).unwrap();
    assert_eq!(snapshot.identity(), &identity());
    assert_eq!(snapshot.entries().len(), 299);
    for (kind, count) in BankKind::ALL.into_iter().zip([128, 107, 64]) {
        assert_eq!(
            snapshot
                .entries()
                .iter()
                .filter(|entry| entry.location.kind() == kind)
                .count(),
            count
        );
    }
    assert_eq!(
        snapshot
            .entries()
            .iter()
            .map(|entry| entry.location.address())
            .collect::<BTreeSet<_>>()
            .len(),
        299
    );
    assert_eq!(snapshot.before(), &library.before);
    assert_eq!(snapshot.after(), library.after.as_ref().unwrap());
    assert_eq!(
        snapshot.manifest_sha256(),
        digest(&fs::read(dir.join(MANIFEST)).unwrap())
    );
    snapshot
        .check_cache_eligibility(&library, Some(&Ok(())))
        .unwrap();
}

#[test]
fn disk_only_or_failed_cleanup_cannot_admit_a_complete_legacy_backup() {
    let root = FixtureRoot::new();
    let library = capture();
    let dir = fixture(&root, "legacy", &library, true, |_, bytes| {
        Some(bytes.to_vec())
    });
    assert_eq!(
        super::super::load_podxt_library(&dir)
            .unwrap()
            .entries
            .len(),
        299
    );
    let snapshot = validate_snapshot(&dir, &identity()).unwrap();
    for cleanup in [None, Some(&Err("release failed".into()))] {
        assert!(
            snapshot
                .check_cache_eligibility(&library, cleanup)
                .unwrap_err()
                .contains("cleanup")
        );
    }
}

#[test]
fn stopped_ambiguous_or_different_captures_cannot_admit_the_snapshot() {
    let root = FixtureRoot::new();
    let mut library = capture();
    let dir = fixture(&root, "saved", &library, true, |_, bytes| {
        Some(bytes.to_vec())
    });
    let snapshot = validate_snapshot(&dir, &identity()).unwrap();
    let cleanup = Some(&Ok(()));
    library.stopped = Some("interrupted".into());
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("stopped")
    );
    library.stopped = None;
    let last = library.entries.pop().unwrap();
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("missing address")
    );
    library.entries.push(StoredTone {
        location: library.entries[0].location,
        tone: last.tone.clone(),
    });
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("Duplicate")
    );
    *library.entries.last_mut().unwrap() = last;
    let another_tone = library.entries[1].tone.clone();
    let original = std::mem::replace(&mut library.entries[0].tone, another_tone);
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("address 0 differs")
    );
    library.entries[0].tone = original;
    let changed = &mut library.after.as_mut().unwrap().raw[39 + 7];
    *changed = 2; // Compatible but not the same raw reply.
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("playing-tone")
    );
    *library.after.as_mut().unwrap().raw.get_mut(39 + 7).unwrap() = 1;
    library.identity_reply[15] = 2;
    assert!(
        snapshot
            .check_cache_eligibility(&library, cleanup)
            .unwrap_err()
            .contains("identity")
    );
}

#[test]
fn rejects_wrong_identity_and_invalid_complete_dumps_even_with_matching_hashes() {
    let root = FixtureRoot::new();
    let library = capture();
    for (name, file, offset, value, error) in [
        ("firmware", IDENTITY_FILE, 15, b'1', "identity"),
        ("member", IDENTITY_FILE, 10, 9, "identity"),
        ("vendor", IDENTITY_FILE, 7, 13, "identity"),
        (
            "envelope",
            IDENTITY_FILE,
            2,
            0,
            "Invalid PODxt snapshot identity",
        ),
        ("name", AFTER, 7, b'W', "playing tone changed"),
        ("drive", AFTER, 39 + 13, 5, "playing tone changed"),
        ("data", "amp-setups/107-107.syx", 50, 0x80, "non-MIDI"),
        (
            "dump-member",
            "fx-setups/064-64.syx",
            6,
            9,
            "complete PODxt Live",
        ),
        (
            "dump-frame",
            "tones/128-32D.syx",
            167,
            0,
            "edit-buffer dump",
        ),
    ] {
        let dir = fixture(&root, name, &library, true, |key, bytes| {
            let mut bytes = bytes.to_vec();
            if key == file {
                bytes[offset] = value;
            }
            Some(bytes)
        });
        assert!(validation_error(&dir).contains(error), "{name}");
    }
    let dir = root.0.join("member");
    let mut expected = identity();
    expected.version = *b"0301";
    assert!(
        validate_snapshot(&dir, &expected)
            .err()
            .unwrap()
            .contains("verified target")
    );
}

#[test]
fn rejects_missing_bank_coverage_missing_after_and_incomplete_folders() {
    let root = FixtureRoot::new();
    let library = capture();
    for (name, file) in [
        ("tone", "tones/128-32D.syx"),
        ("amp", "amp-setups/107-107.syx"),
        ("fx", "fx-setups/064-64.syx"),
        ("after", AFTER),
    ] {
        let dir = fixture(&root, name, &library, true, |key, bytes| {
            (key != file).then(|| bytes.to_vec())
        });
        assert!(validation_error(&dir).contains(file));
        assert!(super::super::load_podxt_library(&dir).is_ok());
    }
    let dir = fixture(&root, "partial", &library, false, |_, bytes| {
        Some(bytes.to_vec())
    });
    assert!(validation_error(&dir).contains("incomplete"));
    let staged = root.0.join("interrupted.partial");
    fs::rename(&dir, &staged).unwrap();
    assert!(validation_error(&staged).contains("incomplete"));
}

#[test]
fn rejects_bad_hash_duplicate_traversal_and_unlisted_paths() {
    let root = FixtureRoot::new();
    let dir = fixture(&root, "manifest", &capture(), true, |_, bytes| {
        Some(bytes.to_vec())
    });
    let manifest = dir.join(MANIFEST);
    let original = fs::read_to_string(&manifest).unwrap();
    for (addition, error) in [
        (original.lines().next().unwrap().to_owned(), "Duplicate"),
        (format!("{}  ../other.syx", "0".repeat(64)), "Unexpected"),
        (
            format!("{}  tones/129-33A.syx", "0".repeat(64)),
            "Unexpected",
        ),
    ] {
        replace(&manifest, format!("{original}{addition}\n").as_bytes());
        assert!(validation_error(&dir).contains(error));
    }
    replace(&manifest, &vec![b'a'; MANIFEST_LIMIT + 1]);
    assert!(validation_error(&dir).contains("at most"));
    replace(&manifest, original.as_bytes());
    let unlisted = dir.join("tones/001-other.syx");
    fs::write(&unlisted, dump(0).raw).unwrap();
    assert!(validation_error(&dir).contains("Unlisted"));
    fs::remove_file(unlisted).unwrap();
    replace(&dir.join("tones/001-1A.syx"), &dump(1).raw);
    assert!(validation_error(&dir).contains("does not match"));
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_files_and_bank_directories() {
    use std::os::unix::fs::symlink;
    let root = FixtureRoot::new();
    let dir = fixture(&root, "backup", &capture(), true, |_, bytes| {
        Some(bytes.to_vec())
    });
    let target = dir.join("tones/001-1A.syx");
    let relocated = root.0.join("relocated.syx");
    fs::rename(&target, &relocated).unwrap();
    symlink(&relocated, &target).unwrap();
    assert!(validation_error(&dir).contains("regular snapshot file"));
    fs::remove_file(&target).unwrap();
    fs::rename(&relocated, &target).unwrap();
    let bank = dir.join("amp-setups");
    let relocated = root.0.join("relocated-bank");
    fs::rename(&bank, &relocated).unwrap();
    symlink(&relocated, &bank).unwrap();
    assert!(validation_error(&dir).contains("plain PODxt snapshot directory"));
}

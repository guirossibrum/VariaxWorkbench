use super::*;
use crate::identity::Identity;
use std::{fs, path::PathBuf};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("variax-snapshot-{label}-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        #[cfg(windows)]
        fn writable(dir: &Path) {
            for item in fs::read_dir(dir).unwrap().flatten() {
                let metadata = item.metadata().unwrap();
                if metadata.is_dir() {
                    writable(&item.path());
                } else if metadata.is_file() {
                    let mut permissions = metadata.permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(item.path(), permissions).unwrap();
                }
            }
        }
        #[cfg(windows)]
        writable(&self.0);
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn identity_reply() -> [u8; 17] {
    let mut data = VARIAX_600;
    data[12..16].copy_from_slice(b"0310");
    data
}

fn identity() -> Identity {
    Identity::parse(&identity_reply()).unwrap()
}

fn fixture(
    root: &TestRoot,
    label: &str,
    coverage: SnapshotCoverage,
    mut alter: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>,
    complete: bool,
) -> PathBuf {
    let mut folder = BackupFolder::create(&root.0, label).unwrap();
    let mut file = |name: &str, bytes: Vec<u8>| {
        if let Some(bytes) = alter(name, bytes) {
            folder.file(name, &bytes).unwrap();
        }
    };
    file("identity.syx", identity_reply().to_vec());
    for (preset, data) in std::iter::once((None, metadata(85)))
        .chain(Preset::all().map(|preset| (Some(preset), metadata(preset.program_number()))))
    {
        let stem = preset.map_or_else(|| "original".into(), variax_preset_stem);
        for (index, block) in data.as_chunks::<96>().0.iter().enumerate() {
            file(&format!("{stem}-{}.syx", index + 1), encode_reply(block));
        }
        if let Some(preset) = preset
            && coverage == SnapshotCoverage::FullPrograms
        {
            let mut image = [0; PROGRAM_BYTES];
            image[0] = preset.program_number();
            variax::procedures::set_firmware_checksum(&mut image);
            file(&program_file(preset), image.to_vec());
        }
    }
    for bank in 0..variax::BANKS.len() as u8 {
        file(&variax_bank_name_file(bank), bank_reply(b"Test bank"));
    }
    folder.finish(complete).unwrap()
}

fn capture(duplicate: bool, fail_release: bool) -> variax::Library {
    let mut script = whole_sweep(85, duplicate);
    script[0].1 = packets(&identity_reply());
    let release = script.len() - 1;
    let mut fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    if fail_release {
        fake.fail_at.push(release);
    }
    let mut log = Quiet;
    let mut session = Session::new(fake, &Policy(None), &mut log);
    let capture = variax::sweep_library(&mut session, || false, |_| {}).unwrap();
    session.link.assert_finished();
    capture
}

#[test]
fn metadata_cache_needs_verified_return_cleanup_and_the_same_capture() {
    let root = TestRoot::new("runtime");
    for (label, duplicate, fail_release) in [
        ("valid", false, false),
        ("ambiguous", true, false),
        ("release-failed", false, true),
    ] {
        let mut capture = capture(duplicate, fail_release);
        let mut folder = BackupFolder::create(&root.0, label).unwrap();
        save_variax_library(&mut folder, &capture).unwrap();
        let path = folder.finish(true).unwrap();
        let snapshot = validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata).unwrap();
        assert_eq!(snapshot.coverage(), SnapshotCoverage::Metadata);
        assert_eq!(snapshot.library().presets.len(), 60);
        assert_eq!(snapshot.library().bank_names.len(), 12);
        let eligible = |capture: &variax::Library, cleanup: Option<&Result<(), String>>| {
            snapshot.check_cache_eligibility(SnapshotCapture::Metadata(capture), cleanup)
        };
        assert_eq!(eligible(&capture, Some(&Ok(()))).is_ok(), label == "valid");
        assert!(eligible(&capture, None).is_err());
        assert!(eligible(&capture, Some(&Err("driver release failed".into()))).is_err());
        if label == "valid" {
            capture.presets[0].replies[0][7] ^= 1;
            assert!(
                eligible(&capture, Some(&Ok(())))
                    .unwrap_err()
                    .contains("differs")
            );
            capture.restore = Restore::Switched {
                to: Preset::new(6, 4).unwrap(),
                verified: false,
            };
            assert!(
                eligible(&capture, Some(&Ok(())))
                    .unwrap_err()
                    .contains("switch-back")
            );
        }
    }
}

#[test]
fn compatible_imports_remain_available_when_files_fall_short_of_cache_validation() {
    let root = TestRoot::new("imports");
    let coverage = SnapshotCoverage::Metadata;
    let path = fixture(
        &root,
        "without-name",
        coverage,
        |name, data| (name != "bank-names/11.syx").then_some(data),
        true,
    );
    assert_eq!(load_variax_library(&path).unwrap().bank_names.len(), 11);
    assert!(validate_snapshot(&path, &identity(), coverage).is_err());

    let path = fixture(&root, "partial", coverage, |_, data| Some(data), false);
    assert!(validate_snapshot(&path, &identity(), coverage).is_err());

    let path = fixture(
        &root,
        "missing-slot",
        coverage,
        |name, data| (name != "presets/00-Custom_1-1-1.syx").then_some(data),
        true,
    );
    assert!(validate_snapshot(&path, &identity(), coverage).is_err());
}

#[test]
fn framing_and_identity_are_checked_even_when_hashes_match() {
    let root = TestRoot::new("framing");
    for (label, target, index, value) in [
        ("wrong-firmware", "identity.syx", 15, b'1'),
        ("bad-identity", "identity.syx", 16, 0),
        ("bad-slot", "presets/00-Custom_1-1-1.syx", 7, 0x10),
        ("bad-bank", "bank-names/11.syx", 6, 0x6c),
    ] {
        let path = fixture(
            &root,
            label,
            SnapshotCoverage::Metadata,
            |name, mut data| {
                if name == target {
                    data[index] = value;
                }
                Some(data)
            },
            true,
        );
        assert!(
            validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata).is_err(),
            "{label}"
        );
    }
}

#[test]
fn full_coverage_requires_the_exact_slots_lengths_and_firmware_checksums() {
    let root = TestRoot::new("programs");
    for label in ["missing", "short", "long", "checksum"] {
        let path = fixture(
            &root,
            label,
            SnapshotCoverage::FullPrograms,
            |name, mut data| {
                if name == "programs/50-Reso-5.bin" {
                    match label {
                        "missing" => return None,
                        "short" => {
                            data.pop();
                        }
                        "long" => data.push(0),
                        _ => data[PROGRAM_BYTES - 1] ^= 1,
                    }
                }
                Some(data)
            },
            true,
        );
        assert!(
            validate_snapshot(&path, &identity(), SnapshotCoverage::FullPrograms).is_err(),
            "{label}"
        );
        assert!(
            validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata).is_err(),
            "{label}"
        );
    }
}

#[test]
fn manifest_integrity_and_membership_are_required() {
    let root = TestRoot::new("manifest");
    let path = fixture(
        &root,
        "unlisted",
        SnapshotCoverage::Metadata,
        |_, data| Some(data),
        true,
    );
    fs::write(path.join("bank-names/12.syx"), bank_reply(b"extra")).unwrap();
    assert!(validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata).is_err());
    for (label, lines) in [
        (
            "duplicated",
            format!("{0}  identity.syx\n{0}  identity.syx\n", "0".repeat(64)),
        ),
        (
            "traversal",
            format!("{}  ../identity.syx\n", "0".repeat(64)),
        ),
    ] {
        let path = root.0.join(label);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("MANIFEST.sha256"), lines).unwrap();
        let error = validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata)
            .err()
            .unwrap();
        assert!(error.contains(if label == "duplicated" {
            "Duplicate"
        } else {
            "Unexpected"
        }));
    }
    let path = fixture(
        &root,
        "bad-hash",
        SnapshotCoverage::Metadata,
        |_, data| Some(data),
        true,
    );
    let bank = path.join("bank-names/11.syx");
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(&bank).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&bank, permissions).unwrap();
    }
    fs::remove_file(&bank).unwrap();
    fs::write(bank, bank_reply(b"changed")).unwrap();
    let error = validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata)
        .err()
        .unwrap();
    assert!(error.contains("does not match MANIFEST"));
}

#[cfg(unix)]
#[test]
fn symlinked_payload_is_refused_even_when_its_bytes_match() {
    let root = TestRoot::new("symlinks");
    let path = fixture(
        &root,
        "snapshot",
        SnapshotCoverage::Metadata,
        |_, data| Some(data),
        true,
    );
    let identity_path = path.join("identity.syx");
    let outside = root.0.join("identity.syx");
    fs::rename(&identity_path, &outside).unwrap();
    std::os::unix::fs::symlink(outside, identity_path).unwrap();
    assert!(validate_snapshot(&path, &identity(), SnapshotCoverage::Metadata).is_err());
}

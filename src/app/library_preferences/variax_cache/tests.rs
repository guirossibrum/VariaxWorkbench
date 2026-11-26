use super::*;
use line6_devices::{
    backup::{BackupFolder, save_variax_library},
    family::{
        podxt::{IDENTITY_FAMILY, PODXT_LIVE_MEMBER, PODXT_LIVE_VERSION},
        variax::{
            BANKS, BankName, Identified, Library, PRESETS_PER_BANK, Preset, PresetMetadata,
            Restore, StoredPreset,
            procedures::{PROGRAM_BYTES, set_firmware_checksum},
        },
    },
};
use std::time::{Duration, UNIX_EPOCH};

fn key(profile: &str) -> VariaxCacheKey {
    VariaxCacheKey::new(
        profile,
        &Identity::parse(&crate::identity_stage::guitar_reply()).unwrap(),
        &Identity {
            manufacturer: LINE6,
            family: IDENTITY_FAMILY,
            member: PODXT_LIVE_MEMBER,
            version: PODXT_LIVE_VERSION,
        },
    )
    .unwrap()
}

fn replies(label: u8) -> [Vec<u8>; 2] {
    [label, 0].map(|label| {
        let mut bytes = vec![0xf0, 0, 1, 12, 7, 4, 0x3c];
        bytes.resize(199, 0);
        bytes[7] = label & 0xf;
        bytes[8] = label >> 4;
        bytes.push(0xf7);
        bytes
    })
}

fn capture(label: u8) -> Library {
    let reply = crate::identity_stage::guitar_reply().to_vec();
    let original_replies = replies(label);
    let original =
        PresetMetadata::from_replies(&original_replies[0], &original_replies[1]).unwrap();
    let start = Preset::new(0, 0).unwrap();
    let presets = Preset::all()
        .map(|preset| {
            let data = if preset == start {
                label
            } else {
                preset.program_number()
            };
            let replies = replies(data);
            let metadata = PresetMetadata::from_replies(&replies[0], &replies[1]).unwrap();
            StoredPreset {
                preset,
                replies,
                metadata,
            }
        })
        .collect();
    let bank_names = (0..BANKS.len() as u8)
        .map(|bank| {
            let mut reply = vec![0xf0, 0, 1, 12, 7, 4, 0x6d, b'A' + bank];
            reply.resize(25, 0);
            reply.push(0xf7);
            (
                bank,
                BankName {
                    name: char::from(b'A' + bank).to_string(),
                    reply,
                },
            )
        })
        .collect();
    Library {
        identified: Identified {
            identity: Identity::parse(&reply).unwrap(),
            reply,
            requests: 1,
        },
        original_replies,
        original,
        presets,
        bank_names,
        restore: Restore::Switched {
            to: start,
            verified: true,
        },
        release_error: None,
        stopped: None,
    }
}

struct Fixture {
    root: tempfile::TempDir,
    prefs: LibraryPreferences,
    key: VariaxCacheKey,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let prefs = LibraryPreferences::new(&root.path().join("settings/editor.json"));
        Self {
            root,
            prefs,
            key: key("one"),
        }
    }

    fn write(&self, label: u8, coverage: SnapshotCoverage) -> PathBuf {
        let capture = capture(label);
        let source = tempfile::tempdir_in(self.root.path()).unwrap().keep();
        let mut folder = BackupFolder::create(&source, "synthetic").unwrap();
        save_variax_library(&mut folder, &capture).unwrap();
        if coverage == SnapshotCoverage::FullPrograms {
            for entry in &capture.presets {
                let preset = entry.preset;
                let name = format!(
                    "programs/{:02}-{}-{}.bin",
                    (preset.bank() + 1) * PRESETS_PER_BANK - 1 - preset.slot(),
                    preset.bank_name().replace(' ', "_"),
                    preset.slot() + 1
                );
                let mut program = [0; PROGRAM_BYTES];
                program[851 * 3] = entry.replies[0][7] | (entry.replies[0][8] << 4);
                set_firmware_checksum(&mut program);
                folder.file(&name, &program).unwrap();
            }
        }
        folder.finish(true).unwrap()
    }

    // The sealed capture is exercised with fake links in the device crate.
    // App fixtures exercise the private staging path with synthetic v1 files.
    fn stage_file(
        &self,
        source: &Path,
        key: VariaxCacheKey,
        coverage: SnapshotCoverage,
        second: u8,
    ) -> Staged {
        self.prefs
            .stage_variax_cache(source, key, coverage, timestamp(second), &digest(source))
            .unwrap()
    }

    fn stage(&self, second: u8) -> Staged {
        let coverage = SnapshotCoverage::Metadata;
        let path = self.write(second, coverage);
        self.stage_file(&path, self.key.clone(), coverage, second)
    }

    fn latest(&self) -> PublishedVariaxSnapshot {
        self.prefs
            .variax_cache_candidates(&self.key)
            .next()
            .unwrap()
    }
}

fn timestamp(second: u8) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(u64::from(second))
}

fn digest(source: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(source.join("MANIFEST.sha256")).unwrap())
    )
}

fn replace_file(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::remove_file(path).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn published_receipt_reopens_independent_profile_snapshot() {
    let fixture = Fixture::new();
    let coverage = SnapshotCoverage::Metadata;
    let source = fixture.write(b'A', coverage);
    let manifest = fs::read(source.join("MANIFEST.sha256")).unwrap();
    fixture
        .stage_file(&source, fixture.key.clone(), coverage, b'A')
        .publish()
        .unwrap();
    assert_eq!(fs::read(source.join("MANIFEST.sha256")).unwrap(), manifest);
    fs::rename(&source, fixture.root.path().join("moved-source")).unwrap();

    let reopened = LibraryPreferences::new(&fixture.root.path().join("settings/editor.json"));
    let cached = reopened
        .variax_cache_candidates(&fixture.key)
        .next()
        .unwrap();
    assert_ne!(cached.directory(), source);
    assert_eq!(cached.captured_at(), timestamp(b'A'));
    assert_eq!(cached.source(), BackupSource::DeviceCheck);
    assert_eq!(cached.snapshot().coverage(), SnapshotCoverage::Metadata);
    assert_eq!(cached.snapshot().library().presets.len(), 60);
    assert_eq!(cached.snapshot().library().bank_names.len(), 12);
    assert_eq!(
        fs::read(cached.directory().join("MANIFEST.sha256")).unwrap(),
        manifest
    );
    assert!(
        reopened
            .variax_cache_candidates(&key("two"))
            .next()
            .is_none()
    );
    assert!(reopened.latest_variax_library().is_none());

    let guitar = fixture.key.guitar.identity();
    let relay = fixture.key.relay.identity();
    for bad in ["../escape", "name/child", "Profile", "", "C:\\elsewhere"] {
        assert!(VariaxCacheKey::new(bad, &guitar, &relay).is_err());
    }
    let wrong_relay = Identity {
        version: [0, 3, 0, 2],
        ..relay
    };
    assert!(VariaxCacheKey::new("one", &guitar, &wrong_relay).is_err());
}

#[test]
fn preserves_full_program_generation_after_a_metadata_refresh_in_the_same_profile() {
    use line6_devices::family::variax::backup::SnapshotPrograms;

    let fixture = Fixture::new();
    let coverage = SnapshotCoverage::FullPrograms;
    let full = fixture.write(b'A', coverage);
    assert!(
        fixture
            .prefs
            .stage_variax_cache(
                &full,
                fixture.key.clone(),
                SnapshotCoverage::Metadata,
                timestamp(b'A'),
                &digest(&full),
            )
            .is_err()
    );
    assert!(!fixture.prefs.variax.directory.exists());
    fixture
        .stage_file(&full, fixture.key.clone(), coverage, b'A')
        .publish()
        .unwrap();
    fixture.stage(b'B').publish().unwrap();

    let second_profile = key("two");
    let other = fixture.write(b'C', coverage);
    fixture
        .stage_file(&other, second_profile.clone(), coverage, b'C')
        .publish()
        .unwrap();
    let cached: Vec<_> = fixture
        .prefs
        .variax_cache_candidates(&fixture.key)
        .collect();
    let [settings, programs] = cached.as_slice() else {
        panic!("expected two cache generations for profile one");
    };
    assert_eq!(settings.captured_at(), timestamp(b'B'));
    assert_eq!(settings.source(), BackupSource::DeviceCheck);
    assert!(matches!(
        settings.snapshot().programs(),
        SnapshotPrograms::MetadataOnly
    ));
    assert_eq!(programs.captured_at(), timestamp(b'A'));
    assert_eq!(programs.source(), BackupSource::FullBackup);
    assert!(
        matches!(programs.snapshot().programs(), SnapshotPrograms::Full(entries) if entries.len() == 60)
    );
    assert_eq!(programs.snapshot().manifest_sha256(), digest(&full));

    let other_profile: Vec<_> = fixture
        .prefs
        .variax_cache_candidates(&second_profile)
        .collect();
    assert_eq!(other_profile.len(), 1);
    assert_eq!(other_profile[0].captured_at(), timestamp(b'C'));
}

#[test]
fn revalidates_bound_source_and_rejects_a_replacement_with_a_valid_manifest() {
    let fixture = Fixture::new();
    let coverage = SnapshotCoverage::Metadata;
    let source = fixture.write(b'A', coverage);
    let manifest_sha256 = digest(&source);
    let stage = || {
        fixture.prefs.stage_variax_cache(
            &source,
            fixture.key.clone(),
            coverage,
            timestamp(b'A'),
            &manifest_sha256,
        )
    };

    fs::rename(&source, fixture.root.path().join("preserved")).unwrap();
    assert!(stage().is_err());
    let different = fixture.write(b'B', coverage);
    fs::rename(&different, &source).unwrap();
    assert!(matches!(stage(), Err(error) if error.contains("manifest differs")));
    assert!(!fixture.prefs.variax.directory.exists());

    let legacy = crate::variax_library::CompletedVariaxBackup::import(&source).unwrap();
    fixture.prefs.remember_backup(&legacy).unwrap();
    assert!(fixture.prefs.latest_variax_library().is_some());
    assert!(
        fixture
            .prefs
            .variax_cache_candidates(&fixture.key)
            .next()
            .is_none()
    );
}

#[test]
fn interrupted_or_failed_publication_preserves_prior_receipt() {
    let fixture = Fixture::new();
    fixture.stage(b'A').publish().unwrap();
    let earlier = fixture.latest().directory().to_owned();

    let abandoned = fixture.stage(b'B');
    let rel = abandoned.directory.clone();
    let reservation = abandoned.reservation.keep();
    let pending = abandoned.profile.join(".pending-receipt");
    fs::write(&pending, serde_json::to_vec(&abandoned.receipt).unwrap()).unwrap();
    assert!(rel.is_dir() && reservation.is_dir());
    assert_eq!(fixture.latest().directory(), earlier);

    let damaged = fixture.stage(b'C');
    replace_file(&damaged.directory.join("original-1.syx"), b"corrupt");
    let target = damaged.target.clone();
    assert!(damaged.publish().is_err());
    assert!(!target.exists());

    let colliding = fixture.stage(b'D');
    let target = colliding.target.clone();
    fs::write(&target, b"occupied receipt name").unwrap();
    assert!(colliding.publish().is_err());
    assert_eq!(fs::read(target).unwrap(), b"occupied receipt name");
    assert_eq!(fixture.latest().directory(), earlier);

    let refresh = fixture.stage(b'E');
    let expected = refresh.directory.clone();
    refresh.publish().unwrap();
    assert_eq!(fixture.latest().directory(), expected);
    assert_eq!(
        fixture
            .prefs
            .variax_cache_candidates(&fixture.key)
            .map(|candidate| candidate.directory().to_owned())
            .collect::<Vec<_>>(),
        [expected, earlier]
    );
}

#[test]
fn reopening_rejects_moved_or_tampered_receipts_and_falls_back() {
    let fixture = Fixture::new();
    fixture.stage(b'A').publish().unwrap();
    let earlier = fixture.latest().directory().to_owned();
    let newer = fixture.stage(b'B');
    let target = newer.target.clone();
    let different = fixture.stage(b'C');
    let other_snapshot = different.receipt.snapshot.clone();
    let good = serde_json::to_value(&newer.receipt).unwrap();
    newer.publish().unwrap();
    assert_ne!(fixture.latest().directory(), earlier);

    for (fields, value) in [
        (&["schema"][..], serde_json::json!(2)),
        (&["key", "profile"][..], serde_json::json!("two")),
        (
            &["key", "relay", "firmware"][..],
            serde_json::json!([0, 3, 0, 2]),
        ),
        (&["key", "guitar", "member"][..], serde_json::json!(0)),
        (&["source"][..], serde_json::json!("ImportedBackup")),
        (&["coverage"][..], serde_json::json!("variax-programs")),
        (&["snapshot"][..], serde_json::json!("../foreign/library")),
        (&["snapshot"][..], serde_json::json!("/outside/library")),
        (&["snapshot"][..], serde_json::json!(other_snapshot)),
    ] {
        let mut tampered = good.clone();
        let mut field = &mut tampered;
        for name in fields {
            field = &mut field[*name];
        }
        *field = value;
        replace_file(&target, serde_json::to_vec(&tampered).unwrap());
        assert_eq!(fixture.latest().directory(), earlier, "{fields:?}");
    }
    replace_file(&target, vec![b' '; RECEIPT_BYTES as usize + 1]);
    assert_eq!(fixture.latest().directory(), earlier);

    replace_file(&target, serde_json::to_vec(&good).unwrap());
    let manifest_path = fixture.latest().directory().join("MANIFEST.sha256");
    let mut manifest = fs::read(&manifest_path).unwrap();
    manifest
        .iter_mut()
        .find(|b| matches!(**b, b'a'..=b'f'))
        .unwrap()
        .make_ascii_uppercase();
    replace_file(&manifest_path, manifest);
    assert_eq!(fixture.latest().directory(), earlier);
}

#[cfg(unix)]
#[test]
fn symlinked_receipts_and_generations_are_skipped() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.stage(b'A').publish().unwrap();
    let earlier = fixture.latest().directory().to_owned();
    let newer = fixture.stage(b'B');
    let target = newer.target.clone();
    let generation = newer.reservation.path().to_owned();
    newer.publish().unwrap();

    let outside_receipt = fixture.root.path().join("outside.json");
    fs::rename(&target, &outside_receipt).unwrap();
    symlink(&outside_receipt, &target).unwrap();
    assert_eq!(fixture.latest().directory(), earlier);

    replace_file(&target, fs::read(&outside_receipt).unwrap());
    let outside_generation = fixture.root.path().join("outside-generation");
    fs::rename(&generation, &outside_generation).unwrap();
    symlink(&outside_generation, &generation).unwrap();
    assert_eq!(fixture.latest().directory(), earlier);
}

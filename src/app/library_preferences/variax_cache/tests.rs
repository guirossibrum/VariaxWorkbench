use super::*;
use line6_devices::{
    backup::{BackupFolder, save_variax_library},
    family::{
        podxt::{IDENTITY_FAMILY, PODXT_LIVE_MEMBER, PODXT_LIVE_VERSION},
        variax::{
            BANKS, BankName, Identified, Library, Preset, PresetMetadata, Restore, StoredPreset,
            backup::SnapshotCapture,
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
    let presets = Preset::all()
        .map(|preset| {
            let replies = replies(preset.program_number());
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
            to: Preset::new(0, 0).unwrap(),
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

    fn write(&self, capture: &Library) -> PathBuf {
        let source = tempfile::tempdir_in(self.root.path()).unwrap().keep();
        let mut folder = BackupFolder::create(&source, "synthetic").unwrap();
        save_variax_library(&mut folder, capture).unwrap();
        folder.finish(true).unwrap()
    }

    // Only the synthetic fixture can supply a simulated transport outcome.
    // Production issuance needs a sealed same-link finalizer.
    fn proof(
        &self,
        path: &Path,
        capture: &Library,
        cleanup: Option<&Result<(), String>>,
        seconds: u64,
    ) -> Result<VariaxCaptureProof, String> {
        let snapshot = validate_snapshot(
            path,
            &self.key.guitar.identity(),
            SnapshotCoverage::Metadata,
        )?;
        snapshot.check_cache_eligibility(SnapshotCapture::Metadata(capture), cleanup)?;
        Ok(VariaxCaptureProof {
            key: self.key.clone(),
            source: path.into(),
            captured_at: UNIX_EPOCH + Duration::from_secs(seconds),
            coverage: Coverage::Settings,
            manifest_sha256: snapshot.manifest_sha256().into(),
        })
    }

    fn source_and_proof(&self, second: u8) -> (PathBuf, VariaxCaptureProof) {
        let capture = capture(second);
        let path = self.write(&capture);
        let proof = self
            .proof(&path, &capture, Some(&Ok(())), second.into())
            .unwrap();
        (path, proof)
    }

    fn stage(&self, second: u8) -> Staged {
        let (path, proof) = self.source_and_proof(second);
        self.prefs.stage_variax_cache(&path, proof).unwrap()
    }

    fn latest(&self) -> PublishedVariaxSnapshot {
        self.prefs
            .variax_cache_candidates(&self.key)
            .next()
            .unwrap()
    }
}

fn replace_file(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::remove_file(path).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn published_receipt_reopens_independent_profile_snapshot() {
    let fixture = Fixture::new();
    let (source, proof) = fixture.source_and_proof(b'A');
    let manifest = fs::read(source.join("MANIFEST.sha256")).unwrap();
    fixture.prefs.publish_variax_cache(&source, proof).unwrap();
    assert_eq!(fs::read(source.join("MANIFEST.sha256")).unwrap(), manifest);
    fs::rename(&source, fixture.root.path().join("moved-source")).unwrap();

    let reopened = LibraryPreferences::new(&fixture.root.path().join("settings/editor.json"));
    let cached = reopened
        .variax_cache_candidates(&fixture.key)
        .next()
        .unwrap();
    assert_ne!(cached.directory(), source);
    assert_eq!(
        cached.captured_at(),
        UNIX_EPOCH + Duration::from_secs(u64::from(b'A'))
    );
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
fn capture_proof_requires_matching_sweep_return_and_both_cleanup_results() {
    let fixture = Fixture::new();
    let mut original = capture(b'A');
    let source = fixture.write(&original);
    let valid_cleanup = Ok(());
    assert!(fixture.proof(&source, &original, None, 1).is_err());
    assert!(
        fixture
            .proof(&source, &original, Some(&Err("driver".into())), 1)
            .is_err()
    );
    let other = capture(b'B');
    assert!(
        fixture
            .proof(&source, &other, Some(&valid_cleanup), 1)
            .err()
            .unwrap()
            .contains("differs")
    );

    original.release_error = Some("bridge".into());
    assert!(
        fixture
            .proof(&source, &original, Some(&valid_cleanup), 1)
            .is_err()
    );
    original.release_error = None;
    original.restore = Restore::NoUniqueMatch { matches: 2 };
    assert!(
        fixture
            .proof(&source, &original, Some(&valid_cleanup), 1)
            .is_err()
    );
    assert!(!fixture.prefs.variax.directory.exists());
    assert!(
        fixture
            .prefs
            .variax_cache_candidates(&fixture.key)
            .next()
            .is_none()
    );
}

#[test]
fn wrong_source_path_or_new_valid_manifest_cannot_reuse_capture_proof() {
    let fixture = Fixture::new();
    let original = capture(b'A');
    let source = fixture.write(&original);
    let copy = fixture.write(&original);
    let proof = || fixture.proof(&source, &original, Some(&Ok(())), 1).unwrap();
    assert!(
        fixture
            .prefs
            .publish_variax_cache(&copy, proof())
            .unwrap_err()
            .contains("path")
    );

    let original_proof = proof();
    let different = fixture.write(&capture(b'B'));
    fs::rename(&source, fixture.root.path().join("preserved")).unwrap();
    fs::rename(&different, &source).unwrap();
    assert!(
        fixture
            .prefs
            .publish_variax_cache(&source, original_proof)
            .unwrap_err()
            .contains("manifest")
    );
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

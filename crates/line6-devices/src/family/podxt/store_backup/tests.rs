use super::super::transfer::tests::tone;
use super::*;
use crate::backup::BackupFolder;
use crate::identity::tests::PODXT_LIVE;

const DAY: u64 = 20_781;

fn identity() -> Vec<u8> {
    let mut identity = PODXT_LIVE.to_vec();
    identity[12..16].copy_from_slice(&[0, 3, 0, 1]);
    identity
}

fn make_backup(
    root: &FilePath,
    tag: &str,
    day: u64,
    missing: Option<Location>,
) -> std::path::PathBuf {
    let mut folder = BackupFolder::create(root, tag).unwrap();
    folder.file("identity.syx", &identity()).unwrap();
    folder.record(format!(
        "PODxt Live library backup at {}.",
        crate::backup::utc(day * 86_400).1
    ));
    let data = tone().edit_dump().raw;
    for location in Location::all(&BankKind::ALL).filter(|location| Some(*location) != missing) {
        folder
            .file(&backup::podxt_entry_name(location), &data)
            .unwrap();
    }
    folder.finish(true).unwrap()
}

#[test]
fn backup_must_be_complete_current_and_manifest_verified() {
    let root = std::env::temp_dir().join(format!("podxt-store-backup-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let slot = Location::new(BankKind::Tones, 64).unwrap();
    let good = make_backup(&root, "verified", DAY, None);
    let prepared = prepare_at(&good, slot, StoreSource::OriginalSlot, DAY)
        .ok()
        .unwrap();
    assert_eq!(prepared.confirmation(), "STORE PODXT 17A");
    assert_eq!(prepared.original_name(), "Offline Tone");
    assert_eq!(prepared.candidate_name(), prepared.original_name());

    assert!(
        prepare_at(&good, slot, StoreSource::OriginalSlot, DAY + 1)
            .err()
            .unwrap()
            .contains("today")
    );
    let incomplete = make_backup(&root, "missing-entry", DAY, Some(slot));
    assert!(
        prepare_at(&incomplete, slot, StoreSource::OriginalSlot, DAY)
            .err()
            .unwrap()
            .contains("omits")
    );
    let destination = good.join(backup::podxt_entry_name(slot));
    let mut permissions = fs::metadata(&destination).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&destination, permissions).unwrap();
    fs::write(destination, b"tampered destination").unwrap();
    assert!(
        prepare_at(&good, slot, StoreSource::OriginalSlot, DAY)
            .err()
            .unwrap()
            .contains("does not match MANIFEST")
    );
    #[cfg(unix)]
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_confirmation_names_the_slot_and_expires_with_the_utc_day() {
    let slot = Location::new(BankKind::Tones, 64).unwrap();
    let prepared = || PreparedStore {
        slot,
        day: DAY,
        original: tone(),
        candidate: tone(),
    };
    assert!(prepared().confirm("STORE PODXT 17B").is_err());
    let confirmed = prepared().confirm("STORE PODXT 17A").unwrap();
    assert!(confirmed.verify_day(DAY).is_ok());
    assert!(confirmed.verify_day(DAY + 1).is_err());
}

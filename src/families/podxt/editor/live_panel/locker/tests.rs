use super::*;

fn fixture(name: &str, link: u8) -> EditBuffer {
    let mut bytes = vec![0; podxt::EDIT_BUFFER_LEN];
    bytes[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    bytes[7..39].fill(b' ');
    bytes[7..7 + name.len()].copy_from_slice(name.as_bytes());
    bytes[39 + 120] = link;
    bytes[39 + 2] = 64; // Reserved byte absent from IFF.
    bytes[167] = 0xf7;
    EditBuffer::parse(bytes).unwrap()
}

#[test]
fn send_bundle_stages_only_changes_and_preserves_unassigned_device_bytes() {
    let snapshot = Snapshot {
        library: Location::all(&BankKind::ALL)
            .map(|slot| (slot.address(), fixture("Before", 10)))
            .collect(),
        ..Default::default()
    };
    let mut bundle = backup_bundle(&snapshot).unwrap();
    assert!(stage_bundle(&bundle, &snapshot).unwrap().is_empty());
    bundle
        .edit_tone(2, 0, |tone| tone.rename("New name"))
        .unwrap();

    let (address, stage) = stage_bundle(&bundle, &snapshot)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let target = Location::new(BankKind::FxSetups, 0).unwrap();
    assert_eq!(address, target.address());
    assert_eq!(stage.candidate.name(), "New name");
    assert_eq!(stage.candidate.as_bytes()[32 + 2], 64);
    assert_eq!(stage.original.name(), "Before");
    assert_eq!(stage_bundle(&bundle, &snapshot).unwrap().len(), 1);
    assert!(stage_bundle(&bundle, &Snapshot::default()).is_err());
}

#[test]
fn slot_type_is_checked_and_failed_sync_keeps_edits_staged() {
    let raw = fixture("Before", 0);
    let slot = Location::new(BankKind::AmpSetups, 8).unwrap();
    let mut snapshot = Snapshot {
        library: BTreeMap::from([(slot.address(), raw.clone())]),
        ..Default::default()
    };
    let source = TonePayload::from_dump(&raw).unwrap();
    let mut tone = decode(&source, slot).unwrap();
    tone.rename("Renamed").unwrap();

    let wrong = PodTone::from_podxt_payload(source.as_bytes(), ContainerKind::L6fs).unwrap();
    let mut locker = Locker::default();
    assert!(locker.revise(slot, &wrong, &snapshot).is_err());
    locker.revise(slot, &tone, &snapshot).unwrap();
    assert!(locker.dirty());

    let mut store = store::StorePanel::default();
    locker.queue.push_back(slot);
    locker.awaiting = Some(slot);
    locker.result(Err(()), &mut snapshot, &mut store);
    assert!(!locker.syncing());
    assert!(locker.dirty());

    locker.queue.push_back(slot);
    locker.awaiting = Some(slot);
    locker.result(
        Ok(StoredTone {
            location: slot,
            tone: raw,
        }),
        &mut snapshot,
        &mut store,
    );
    assert!(!locker.dirty());
    assert!(!locker.syncing());
}

#[test]
fn channel_review_warns_only_for_a_different_linked_model() {
    let slot = Location::new(BankKind::Tones, 0).unwrap();
    let mut snapshot = Snapshot {
        tone: Some(fixture("Current", 15)),
        library: BTreeMap::from([(slot.address(), fixture("Other", 10))]),
        ..Default::default()
    };
    assert!(
        link_warning(&snapshot, slot)
            .unwrap()
            .contains("F-T-Model - 1")
    );
    snapshot.tone = Some(fixture("Current", 10));
    assert!(link_warning(&snapshot, slot).is_none());
    snapshot.library.insert(slot.address(), fixture("Keep", 0));
    assert!(link_warning(&snapshot, slot).is_none());
}

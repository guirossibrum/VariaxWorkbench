use super::*;
use variax_workbench::firmware::synthetic::synthetic_container;

fn sample() -> PreparedStore {
    let prepared = prepared(Stage::Own);
    PreparedStore {
        settings: prepared.start,
        program: prepared.program,
    }
}

fn store_fake(prepared: &PreparedStore, replies: &[Vec<u8>], readback: bool) -> Fake {
    let mut inbox: VecDeque<_> = current_replies(&prepared.settings).into();
    inbox.extend(replies.iter().cloned());
    if readback {
        inbox.extend(current_replies(&prepared.settings));
    }
    Fake::new(DevicePath::VariaxViaPodxt, inbox)
}

fn run_store(
    fake: &mut Fake,
    prepared: &PreparedStore,
    cancelled: impl FnMut() -> bool,
) -> Result<(), String> {
    let mut quiet = Quiet;
    let mut session = Session::new(fake, &StorePolicy, &mut quiet);
    attempt_store(
        &mut session,
        prepared,
        || confirm_store(STORE_CONFIRMATION),
        cancelled,
    )
}

#[test]
fn archive_with_correct_product_and_version_but_wrong_fingerprint_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("other.vxf");
    fs::write(
        &archive,
        synthetic_container(
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
            b"synthetic image",
        ),
    )
    .unwrap();
    assert!(
        prepare_store(dir.path(), "2026-11-23", &archive)
            .err()
            .unwrap()
            .contains("exact archived source SHA-256")
    );
}

#[test]
fn slot_conversion_refuses_wrong_length_hash_checksum_and_model_name() {
    let program = sample().program;
    let raw = |bytes: &[u8]| {
        bytes
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[a, b, c]| [c, b, a])
            .collect::<Vec<_>>()
    };
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    assert_eq!(
        canonical_store_program(&raw(&program), &hash(&program)).unwrap(),
        program
    );
    assert!(
        canonical_store_program(&raw(&program[..3273]), &hash(&program))
            .unwrap_err()
            .contains("3,276")
    );
    assert!(
        canonical_store_program(&raw(&program), STORE_HASH)
            .unwrap_err()
            .contains("SHA-256")
    );

    let mut corrupt = program;
    corrupt[3275] ^= 1;
    assert!(
        canonical_store_program(&raw(&corrupt), &hash(&corrupt))
            .unwrap_err()
            .contains("checksum")
    );
    let mut wrong_name = program;
    wrong_name[METADATA.start] = b'X';
    checksum(&mut wrong_name);
    assert!(
        canonical_store_program(&raw(&wrong_name), &hash(&wrong_name))
            .unwrap_err()
            .contains("model name")
    );
}

#[test]
fn store_policy_restricts_each_operation_to_its_gate() {
    for allowed in [
        Operation::VariaxIdentity,
        Operation::VariaxRead,
        Operation::VariaxStoreRestore,
    ] {
        assert!(StorePolicy.allows(allowed));
    }
    for forbidden in [
        Operation::VariaxEditBufferWrite,
        Operation::VariaxPresetSwitch,
        Operation::PodxtIdentity,
        Operation::PodxtRead,
        Operation::PodxtLibraryRead,
        Operation::PodxtPackQuery,
        Operation::PodxtEditBufferWrite,
        Operation::PodxtAdditionalEditBufferWrite,
        Operation::PodxtKeepEditBufferChanges,
        Operation::PodxtChannelSwitch,
        Operation::PodxtVariaxControlRelease,
        Operation::PodxtLinkedVariaxModelEdit,
        Operation::PodxtGlobalRead,
        Operation::PodxtTunerMode,
    ] {
        assert!(!StorePolicy.allows(forbidden));
    }
    assert!(!VariaxPolicy.allows(Operation::VariaxStoreRestore));
}

#[test]
fn store_refuses_wrong_guitar_identity_wrong_name_settings_and_confirmation() {
    let prepared = sample();
    let mut fake = store_fake(&prepared, &[], false);
    fake.inbox[0][10] = 5;
    assert!(
        run_store(&mut fake, &prepared, || false)
            .unwrap_err()
            .contains("Variax 300/600")
    );
    assert_eq!(fake.writes, [line6_devices::identity::REQUEST]);

    for index in [0, 191] {
        let mut fake = store_fake(&prepared, &[], false);
        let mut wrong = prepared.settings;
        wrong[index] ^= 1;
        fake.inbox = current_replies(&wrong).into();
        assert!(
            run_store(&mut fake, &prepared, || false)
                .unwrap_err()
                .contains("active Variax settings differ")
        );
        assert_eq!(fake.writes, metadata_requests());
    }

    let mut fake = store_fake(&prepared, &[], false);
    let mut quiet = Quiet;
    let mut session = Session::new(&mut fake, &StorePolicy, &mut quiet);
    assert!(
        attempt_store(
            &mut session,
            &prepared,
            || confirm_store("STORE RESO-5"),
            || false
        )
        .unwrap_err()
        .contains("confirmation was not given")
    );
    assert_eq!(fake.writes, metadata_requests());
    assert!(confirm_store(&format!("{STORE_CONFIRMATION}\r\n")).is_ok());
    assert!(confirm_store(&format!(" {STORE_CONFIRMATION}")).is_err());
}

#[test]
fn store_success_matches_library_slot_50_transcript_and_reads_back() {
    let prepared = sample();
    let ack = guitar(&[0x71]);
    let mut direct = Fake::new(DevicePath::VariaxViaPodxt, [ack.clone()].into());
    let mut quiet = Quiet;
    let mut session = Session::new(&mut direct, &StorePolicy, &mut quiet);
    let program = write::Program::new(NAME, &prepared.program).unwrap();
    write::store(
        &mut session,
        variax::Preset::new(10, 4).unwrap(),
        &program,
        || false,
    )
    .unwrap();
    assert_eq!(
        session
            .sent
            .iter()
            .map(|bytes| bytes[6])
            .collect::<Vec<_>>(),
        [1, 0x2a, 0x4c, 0x43, 0x61, 0x4c, 0x2a, 0]
    );
    assert_eq!(session.sent[4], guitar(&[0x61, 50]));
    assert_eq!(session.sent[3].len(), 6566);
    assert_eq!(
        direct.writes[7..33]
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        [vec![256; 25], vec![166]].concat()
    );
    let expected = [metadata_requests(), direct.writes, metadata_requests()].concat();
    for notices in [vec![], vec![guitar(&[0x28])]] {
        let replies = [notices, vec![ack.clone()]].concat();
        let mut fake = store_fake(&prepared, &replies, true);
        run_store(&mut fake, &prepared, || false).unwrap();
        assert_eq!(fake.writes, expected);
        assert_eq!(fake.pauses, [Duration::from_millis(100); 4]);
        assert!(fake.inbox.is_empty());
    }
}

#[test]
fn store_timeout_and_wrong_reply_stop_after_cleanup_without_retry_or_readback() {
    let prepared = sample();
    for (replies, message) in [
        (vec![], "Timed out waiting"),
        (vec![guitar(&[0x70])], "Unexpected Variax store reply"),
    ] {
        let mut fake = store_fake(&prepared, &replies, false);
        let error = run_store(&mut fake, &prepared, || false).unwrap_err();
        assert!(error.contains(message) && error.contains("Slot state is unknown."));
        assert!(fake.writes.ends_with(&cleanup()));
        assert_eq!(
            fake.writes
                .iter()
                .filter(|chunk| chunk.as_slice() == [0x61, 50])
                .count(),
            1
        );
        assert_eq!(
            fake.writes
                .iter()
                .filter(|chunk| chunk.as_slice() == line6_devices::identity::REQUEST)
                .count(),
            1
        );
    }
}

#[test]
fn store_readback_checks_the_last_settings_byte() {
    let prepared = sample();
    let mut fake = store_fake(&prepared, &[guitar(&[0x71])], true);
    let mut wrong = prepared.settings;
    wrong[191] ^= 1;
    fake.inbox[6] = reply(&wrong[96..]);
    assert!(
        run_store(&mut fake, &prepared, || false)
            .unwrap_err()
            .contains("Settings read-back differs")
    );
    assert!(fake.writes.ends_with(&metadata_requests()));
}

#[test]
fn cancellation_before_opcode_prevents_store_and_cancellation_after_still_verifies() {
    let prepared = sample();
    for after in [false, true] {
        let cancelled = Rc::new(Cell::new(false));
        let mut fake = store_fake(&prepared, &[guitar(&[0x71])], true);
        // Start with the successful final upload fragment as the pre-0x61 trigger.
        let mut direct = Fake::new(DevicePath::VariaxViaPodxt, [guitar(&[0x71])].into());
        write::store(
            &mut Session::new(&mut direct, &StorePolicy, &mut Quiet),
            location(),
            &write::Program::new(NAME, &prepared.program).unwrap(),
            || false,
        )
        .unwrap();
        let trigger = if after {
            vec![HEADER.into(), vec![0x61, 50], vec![0xf7]]
        } else {
            vec![direct.writes[32].clone()]
        };
        fake.cancel_on_write = Some(CancelOnWrite {
            chunks: trigger,
            cancelled: Rc::clone(&cancelled),
        });
        let result = run_store(&mut fake, &prepared, || cancelled.get());
        assert!(cancelled.get());
        if after {
            result.unwrap();
            assert!(fake.writes.ends_with(&metadata_requests()));
            assert!(fake.inbox.is_empty());
        } else {
            assert!(
                result
                    .unwrap_err()
                    .contains("stopped before the slot store")
            );
            assert!(fake.writes.ends_with(&cleanup()));
            assert!(
                !fake
                    .writes
                    .iter()
                    .any(|chunk| chunk.as_slice() == [0x61, 50])
            );
        }
    }
}

#[test]
fn partial_program_write_error_terminates_and_cleans_up_without_storing() {
    let prepared = sample();
    let mut fake = store_fake(&prepared, &[], false);
    fake.fail_at = Some(13);
    assert!(
        run_store(&mut fake, &prepared, || false)
            .unwrap_err()
            .contains("transport write failure")
    );
    assert!(fake.writes.ends_with(&cleanup()));
    assert_eq!(fake.writes[fake.writes.len() - 8], [0xf7]);
    assert!(
        !fake
            .writes
            .iter()
            .any(|chunk| chunk.as_slice() == [0x61, 50])
    );
}

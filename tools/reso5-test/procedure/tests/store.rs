use super::*;
use crate::procedure::store::{self as ladder, Mode, Prompt};
use variax_workbench::firmware::synthetic::synthetic_container;

const MODES: [Mode; 3] = [Mode::Rename, Mode::Restore, Mode::Factory];
const CHUNKS: usize = 35;
const ACK: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x71, 0xf7];

fn program() -> [u8; PROGRAM_BYTES] {
    let mut capture = prepared(Stage::Own).program;
    capture[0] = 1;
    expected_stored_image(&capture)
}

fn sample(mode: Mode) -> ladder::Prepared {
    ladder::make_images(mode, program(), prepared(Stage::Own).program).unwrap()
}

fn settings_replies(image: &[u8; PROGRAM_BYTES]) -> [Vec<u8>; 3] {
    current_replies(image[METADATA].try_into().unwrap())
}

fn fake(
    before: &[u8; PROGRAM_BYTES],
    current: &[u8; PROGRAM_BYTES],
    replies: &[Vec<u8>],
    readback: Option<(&[u8; PROGRAM_BYTES], &[u8; PROGRAM_BYTES])>,
) -> Fake {
    let mut inbox: VecDeque<_> = settings_replies(before).into();
    inbox.extend(full_read::settled_replies(current, CHUNKS));
    inbox.extend(replies.iter().cloned());
    if let Some((image, settings)) = readback {
        inbox.extend(full_read::settled_replies(image, CHUNKS));
        inbox.extend(settings_replies(settings));
    }
    Fake::new(DevicePath::VariaxViaPodxt, inbox)
}

fn successful(prepared: &ladder::Prepared) -> Fake {
    fake(
        &prepared.before,
        &prepared.before,
        &[ACK.into()],
        Some((&prepared.after, &prepared.after)),
    )
}

fn confirm(prompt: Prompt) -> Result<(), String> {
    let phrase = match prompt {
        Prompt::Confirm(mode) => mode.confirmation(),
        Prompt::Reselect => ladder::RESELECT_CONFIRMATION,
    };
    ladder::require_confirmation(prompt, phrase)
}

fn run(
    fake: &mut Fake,
    prepared: &ladder::Prepared,
    dir: &Path,
    prompt: impl FnMut(Prompt) -> Result<(), String>,
    cancelled: impl FnMut() -> bool,
) -> Result<(), String> {
    ladder::attempt(
        &mut Session::new(fake, &StorePolicy, &mut Quiet),
        prepared,
        prepared.before[METADATA].try_into().unwrap(),
        dir,
        prompt,
        |_| {},
        cancelled,
    )
}

fn preflight_writes() -> Vec<Vec<u8>> {
    [
        metadata_requests(),
        full_read::guitar_sequence(CHUNKS)[1..].to_vec(),
    ]
    .concat()
}

fn store_writes(program: &[u8; PROGRAM_BYTES]) -> Vec<Vec<u8>> {
    let full = guitar(&[&[0x43, 4, 4, 4, 0, 0, 0][..], &sysex_words(program)].concat());
    assert_eq!(full.len(), 6566);
    let mut writes = vec![
        BRIDGE_ON.into(),
        HEADER.into(),
        vec![0x2a, 1],
        vec![0xf7],
        HEADER.into(),
        vec![0x4c, 1],
        vec![0xf7],
    ];
    writes.extend(full.chunks(256).map(<[u8]>::to_vec));
    writes.extend([HEADER.into(), vec![0x61, 50], vec![0xf7]]);
    writes.extend(cleanup());
    writes
}

#[test]
fn all_modes_use_the_exact_complete_sequence_and_compare_reselected_bytes() {
    let mut podxt = Fake::new(DevicePath::PodxtMidi, [PODXT_IDENTITY.into()].into());
    check_podxt(&mut podxt, &mut Quiet).unwrap();
    assert_eq!(podxt.writes, [line6_devices::identity::REQUEST]);
    for mode in MODES {
        let prepared = sample(mode);
        let dir = tempfile::tempdir().unwrap();
        let mut fake = fake(
            &prepared.before,
            &prepared.before,
            &[guitar(&[0x28]), ACK.into()],
            Some((&prepared.after, &prepared.after)),
        );
        let mut prompts = Vec::new();
        run(
            &mut fake,
            &prepared,
            dir.path(),
            |prompt| {
                prompts.push(prompt);
                confirm(prompt)
            },
            || false,
        )
        .unwrap();
        assert_eq!(prompts, [Prompt::Confirm(mode), Prompt::Reselect]);
        assert_eq!(
            fake.writes,
            [
                preflight_writes(),
                store_writes(&prepared.program),
                full_read::guitar_sequence(CHUNKS)[1..].to_vec(),
                metadata_requests(),
            ]
            .concat()
        );
        assert_eq!(
            fake.pauses
                .iter()
                .map(Duration::as_millis)
                .collect::<Vec<_>>(),
            [100, 1000, 100, 100, 100, 1000, 100, 100]
        );
        for (subdir, expected) in [("preflight", prepared.before), ("readback", prepared.after)] {
            let folder = dir.path().join(subdir);
            assert_eq!(fs::read_dir(&folder).unwrap().count(), CHUNKS);
            let last = fs::read(folder.join("start-1088-count-4.syx")).unwrap();
            assert_eq!(last, reply(&expected[1088 * 3..]));
        }
        assert!(fake.inbox.is_empty());
    }
}

#[test]
fn restore_preflight_retries_a_silent_identity_before_any_other_request() {
    let prepared = sample(Mode::Restore);
    let dir = tempfile::tempdir().unwrap();
    let mut rig = successful(&prepared);
    for _ in 0..30 {
        rig.inbox.push_front(vec![]);
    }
    run(&mut rig, &prepared, dir.path(), confirm, || false).unwrap();
    let identity = line6_devices::identity::REQUEST;
    let expected = [
        vec![identity.to_vec()],
        preflight_writes(),
        store_writes(&prepared.program),
        full_read::guitar_sequence(CHUNKS)[1..].to_vec(),
        metadata_requests(),
    ]
    .concat();
    assert_eq!(rig.writes, expected);
    assert_eq!(
        rig.write_times[1] - rig.write_times[0],
        Duration::from_secs(5)
    );
    assert_eq!(rig.pauses[0], Duration::from_secs(2));
}

#[test]
fn local_images_change_only_name_and_checksum_for_rename_and_reverse_it_for_restore() {
    for mode in MODES {
        assert_eq!(Mode::parse(mode.label()), Some(mode));
    }
    assert_eq!(Mode::parse("unknown"), None);
    let rename = sample(Mode::Rename);
    assert_eq!(rename.before, program());
    assert_eq!(&rename.program[2553..2571], b"Reso-5 T          ");
    assert_eq!(rename.program[..2553], rename.before[..2553]);
    assert_eq!(rename.program[2571..3273], rename.before[2571..3273]);
    assert_ne!(rename.program[3273..], rename.before[3273..]);
    assert_eq!(rename.program, rename.after);
    let restore = sample(Mode::Restore);
    assert_eq!(restore.before, rename.after);
    assert_eq!(restore.program, rename.before);
    assert_eq!(restore.after, rename.before);
    let factory = sample(Mode::Factory);
    assert_eq!(factory.before, program());
    assert_eq!(factory.program, prepared(Stage::Own).program);
    assert_eq!(
        &factory.after[767 * 3..771 * 3],
        b"\0\x000\0\x003\0\x001\0\x000"
    );
}

#[test]
fn stamped_checksum_sums_through_word_1090() {
    let mut program = [0; PROGRAM_BYTES];
    program[1089 * 3..1090 * 3].copy_from_slice(&[0xff, 0xff, 0xfe]);
    program[1090 * 3..1091 * 3].copy_from_slice(&[0, 0, 5]);
    let after = expected_stored_image(&program);
    assert_eq!(&after[1091 * 3..], &[0, 0, 0xc7]);
}

#[test]
fn rejects_wrong_archive_capture_checksum_hash_and_missing_chunks_before_io() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("other.vxf");
    document_fixture(
        &archive,
        &synthetic_container(
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
            b"synthetic image",
        ),
    );
    assert!(
        ladder::reference_images(Mode::Rename, &archive, dir.path())
            .err()
            .unwrap()
            .contains("exact archived source SHA-256")
    );
    assert!(
        ladder::load_capture(dir.path())
            .unwrap_err()
            .contains("start-0-count-32.syx")
    );
    fs::create_dir(dir.path().join("start-0-count-32.syx")).unwrap();
    assert!(
        ladder::load_capture(dir.path())
            .unwrap_err()
            .contains("regular file")
    );
    let capture = program();
    assert!(
        ladder::validate_capture(&capture, "wrong")
            .unwrap_err()
            .contains("pinned Reso-5 SHA-256")
    );
    let mut invalid = capture;
    invalid[3275] ^= 1;
    let hash = format!("{:x}", Sha256::digest(invalid));
    assert!(
        ladder::validate_capture(&invalid, &hash)
            .unwrap_err()
            .contains("checksum")
    );
    let mut invalid = capture;
    invalid[2553] = b'X';
    let hash = format!("{:x}", Sha256::digest(invalid));
    assert!(
        ladder::validate_capture(&invalid, &hash)
            .unwrap_err()
            .contains("model name")
    );
}

fn document_fixture(path: &Path, bytes: &[u8]) {
    variax_workbench::document::save_new(path, bytes).unwrap();
}

#[test]
fn rejects_an_illegal_reply_or_a_complete_capture_with_wrong_words() {
    for malformed in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        for (index, raw) in full_read::settled_replies(&program(), CHUNKS)
            .into_iter()
            .enumerate()
        {
            let count = if index == 34 { 4 } else { 32 };
            let mut raw = raw;
            if malformed && index == 2 {
                raw.pop();
            }
            document_fixture(
                &dir.path()
                    .join(format!("start-{}-count-{count}.syx", index * 32)),
                &raw,
            );
        }
        let error = ladder::load_capture(dir.path()).unwrap_err();
        assert!(error.contains(if malformed {
            "missing F7"
        } else {
            "pinned Reso-5 SHA-256"
        }));
    }
}

#[test]
fn each_mode_accepts_matching_old_backup_and_refuses_recent_backup_with_different_settings() {
    for mode in MODES {
        let prepared = sample(mode);
        let old = tempfile::tempdir().unwrap();
        let recent = tempfile::tempdir().unwrap();
        let expected: &[u8; 192] = prepared.before[METADATA].try_into().unwrap();
        let mut wrong = *expected;
        wrong[0] = b'X';
        backup_at(old.path(), expected, "2025-01-01", false);
        backup_at(recent.path(), &wrong, "2026-11-24", false);
        assert_eq!(check_backup(old.path(), expected).unwrap(), *expected);
        let error = check_backup(recent.path(), expected).unwrap_err();
        assert!(error.contains("backup's slot 50 settings"));
        assert!(error.contains("preset name at settings byte 0"));
    }
}

#[test]
fn refuses_wrong_identity_settings_and_complete_preflight_mismatch() {
    for mode in MODES {
        let prepared = sample(mode);
        let no_prompt = |_| panic!("refused run reached user confirmation");
        let dir = tempfile::tempdir().unwrap();
        let mut rig = successful(&prepared);
        rig.inbox[0][10] = 5;
        assert!(
            run(&mut rig, &prepared, dir.path(), no_prompt, || false)
                .unwrap_err()
                .contains("Variax 300/600")
        );
        assert_eq!(rig.writes, [line6_devices::identity::REQUEST]);

        let mut wrong = prepared.before;
        wrong[METADATA.end - 1] ^= 1;
        let mut rig = fake(&wrong, &prepared.before, &[], None);
        assert!(
            run(&mut rig, &prepared, dir.path(), no_prompt, || false)
                .unwrap_err()
                .contains(
                    "backup's slot 50 settings differs in unmapped settings at settings byte 191"
                )
        );
        assert_eq!(rig.writes, metadata_requests());

        let mut wrong = prepared.before;
        wrong[0] ^= 1;
        let mut rig = fake(&prepared.before, &wrong, &[], None);
        assert!(
            run(&mut rig, &prepared, dir.path(), no_prompt, || false)
                .unwrap_err()
                .contains("preflight differs at word 0")
        );
        assert_eq!(rig.writes, preflight_writes());
        let mut wrong = prepared.before;
        wrong[METADATA.start + 64] ^= 1;
        let dir = tempfile::tempdir().unwrap();
        let mut rig = fake(&prepared.before, &wrong, &[], None);
        let error = run(&mut rig, &prepared, dir.path(), no_prompt, || false).unwrap_err();
        assert!(error.contains("save_tone_position, settings byte 64"));
        assert_eq!(rig.writes, preflight_writes());
    }
}

#[test]
fn refuses_incomplete_preflight_after_saving_completed_chunks() {
    let prepared = sample(Mode::Factory);
    let mut rig = successful(&prepared);
    rig.inbox.truncate(3 + 6);
    let dir = tempfile::tempdir().unwrap();
    let error = run(&mut rig, &prepared, dir.path(), |_| panic!(), || false).unwrap_err();
    assert!(error.contains("Start 192, count 32"));
    assert_eq!(
        rig.writes,
        [
            metadata_requests(),
            full_read::guitar_sequence(7)[1..].to_vec()
        ]
        .concat()
    );
    assert_eq!(
        fs::read_dir(dir.path().join("preflight")).unwrap().count(),
        6
    );
}

#[test]
fn wrong_mode_phrase_refuses_and_missing_reselection_prevents_post_store_reads() {
    for mode in MODES {
        let prepared = sample(mode);
        let dir = tempfile::tempdir().unwrap();
        let wrong_mode = if mode == Mode::Factory {
            Mode::Rename
        } else {
            Mode::Factory
        };
        let mut rig = successful(&prepared);
        let error = run(
            &mut rig,
            &prepared,
            dir.path(),
            |prompt| ladder::require_confirmation(prompt, wrong_mode.confirmation()),
            || false,
        )
        .unwrap_err();
        assert!(error.contains("confirmation was not given"));
        assert_eq!(rig.writes, preflight_writes());

        let dir = tempfile::tempdir().unwrap();
        let mut rig = successful(&prepared);
        let error = run(
            &mut rig,
            &prepared,
            dir.path(),
            |prompt| {
                if prompt == Prompt::Reselect {
                    ladder::require_confirmation(prompt, "")
                } else {
                    confirm(prompt)
                }
            },
            || false,
        )
        .unwrap_err();
        assert!(error.contains("reselection and read-back were not verified"));
        assert_eq!(
            rig.writes,
            [preflight_writes(), store_writes(&prepared.program)].concat()
        );
    }
}

#[test]
fn post_store_refuses_a_single_image_word_or_settings_byte_difference() {
    for mode in MODES {
        let prepared = sample(mode);
        for image_differs in [false, true] {
            let mut wrong = prepared.after;
            wrong[871 * 3] ^= 1;
            let dir = tempfile::tempdir().unwrap();
            let (image, settings) = if image_differs {
                (&wrong, &prepared.after)
            } else {
                (&prepared.after, &wrong)
            };
            let mut rig = fake(
                &prepared.before,
                &prepared.before,
                &[ACK.into()],
                Some((image, settings)),
            );
            let error = run(&mut rig, &prepared, dir.path(), confirm, || false).unwrap_err();
            assert!(error.contains(if image_differs {
                "read-back differs at word 871"
            } else {
                "Settings read-back differs"
            }));
            let mut expected = [
                preflight_writes(),
                store_writes(&prepared.program),
                full_read::guitar_sequence(CHUNKS)[1..].to_vec(),
            ]
            .concat();
            if !image_differs {
                expected.extend(metadata_requests());
            }
            assert_eq!(rig.writes, expected);
        }
    }
}

#[test]
fn silent_timeout_waits_without_further_sends_and_wrong_reply_stops() {
    let prepared = sample(Mode::Rename);
    for silent in [true, false] {
        let replies = if silent {
            vec![]
        } else {
            vec![guitar(&[0x70])]
        };
        let mut rig = fake(&prepared.before, &prepared.before, &replies, None);
        let dir = tempfile::tempdir().unwrap();
        let noticed = Cell::new(false);
        let error = ladder::attempt(
            &mut Session::new(&mut rig, &StorePolicy, &mut Quiet),
            &prepared,
            prepared.before[METADATA].try_into().unwrap(),
            dir.path(),
            |prompt| {
                assert!(matches!(prompt, Prompt::Confirm(_)));
                confirm(prompt)
            },
            |notice| {
                assert!(notice.contains(ladder::SPARE_WARNING));
                noticed.set(true);
            },
            || false,
        )
        .unwrap_err();
        assert_eq!(noticed.get(), silent);
        if silent {
            assert!(
                error.contains("Device check Variax backup")
                    && error.contains(ladder::SPARE_WARNING)
            );
            assert_eq!(rig.pauses.last(), Some(&Duration::from_secs(120)));
        } else {
            assert!(error.contains("Unexpected Variax store reply"));
        }
        assert_eq!(
            rig.writes,
            [preflight_writes(), store_writes(&prepared.program)].concat()
        );
    }
}

#[test]
fn cancellation_before_61_stops_and_after_61_finishes_both_verifications() {
    let prepared = sample(Mode::Restore);
    for after in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let stopped = Rc::new(Cell::new(false));
        let writes = store_writes(&prepared.program);
        let mut rig = successful(&prepared);
        rig.cancel_on_write = Some(CancelOnWrite {
            chunks: if after {
                vec![HEADER.into(), vec![0x61, 50], vec![0xf7]]
            } else {
                vec![writes[32].clone()]
            },
            cancelled: Rc::clone(&stopped),
        });
        let result = run(&mut rig, &prepared, dir.path(), confirm, || stopped.get());
        assert!(stopped.get());
        if after {
            result.unwrap();
            assert!(rig.writes.ends_with(&metadata_requests()));
        } else {
            assert!(
                result
                    .unwrap_err()
                    .contains("stopped before the slot store")
            );
            assert!(!rig.writes.iter().any(|bytes| bytes == &[0x61, 50]));
            assert!(rig.writes.ends_with(&cleanup()));
        }
    }
}

#[test]
fn partial_upload_transport_error_closes_without_storing() {
    let prepared = sample(Mode::Factory);
    let dir = tempfile::tempdir().unwrap();
    let mut rig = successful(&prepared);
    rig.fail_at = Some(preflight_writes().len() + 8);
    let error = run(&mut rig, &prepared, dir.path(), confirm, || false).unwrap_err();
    assert!(error.contains("transport write failure"));
    assert!(rig.writes.ends_with(&cleanup()));
    assert_eq!(rig.writes[rig.writes.len() - 8], [0xf7]);
    assert!(!rig.writes.iter().any(|bytes| bytes == &[0x61, 50]));
}

#[test]
fn store_policy_admits_only_identity_g3_reads_and_g7_store() {
    for op in [
        Operation::VariaxIdentity,
        Operation::VariaxRead,
        Operation::VariaxStoreRestore,
    ] {
        assert!(StorePolicy.allows(op));
    }
    for op in [
        Operation::VariaxSlotRead,
        Operation::VariaxEditBufferWrite,
        Operation::VariaxPresetSwitch,
        Operation::VariaxBankNameRead,
        Operation::VariaxLibraryBridge,
        Operation::PodxtIdentity,
        Operation::PodxtEditBufferWrite,
        Operation::PodxtChannelSwitch,
    ] {
        assert!(!StorePolicy.allows(op));
    }
    assert!(!VariaxPolicy.allows(Operation::VariaxStoreRestore));
}

#[test]
#[ignore = "requires local fingerprinted archive and #234 capture"]
fn private_inputs_verify_all_three_expected_image_hashes() {
    let archive = std::env::var_os("VARIAX_310_ARCHIVE").expect("set VARIAX_310_ARCHIVE");
    let capture = std::env::var_os("VARIAX_RESO5_CAPTURE").expect("set VARIAX_RESO5_CAPTURE");
    for (mode, hash) in [
        (
            Mode::Rename,
            "75bcaaf5f775bcd2f5ba22289aa4b8e9cac0c6115b4ac32b75144b4ccb5c716b",
        ),
        (
            Mode::Restore,
            "3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249",
        ),
        (
            Mode::Factory,
            "90d79131f7909639aa2d0793cb3f617fbd95a95a1ada7a54f90ec84de4e28164",
        ),
    ] {
        let prepared =
            ladder::reference_images(mode, Path::new(&archive), Path::new(&capture)).unwrap();
        assert_eq!(prepared.expected_sha256(), hash);
        let backup = tempfile::tempdir().unwrap();
        backup_at(
            backup.path(),
            prepared.before[METADATA].try_into().unwrap(),
            "2025-01-01",
            false,
        );
        assert_eq!(
            ladder::prepare(
                mode,
                backup.path(),
                Path::new(&archive),
                Path::new(&capture)
            )
            .unwrap()
            .0
            .expected_sha256(),
            hash,
        );
    }
}

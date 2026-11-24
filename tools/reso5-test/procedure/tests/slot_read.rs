use super::*;
use crate::procedure::slot_read::{self, PreparedSlotRead};

const IDENTITY_REQUEST: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7];

fn guitar_with_replies(replies: impl IntoIterator<Item = Vec<u8>>) -> Fake {
    let inbox = [VARIAX_IDENTITY.to_vec()]
        .into_iter()
        .chain(replies)
        .collect();
    Fake::new(DevicePath::VariaxViaPodxt, inbox)
}

fn baseline() -> ([u8; PROGRAM_BYTES], PreparedSlotRead) {
    let settings = settings();
    let mut program = [0; PROGRAM_BYTES];
    program[METADATA].copy_from_slice(&settings);
    checksum(&mut program);
    (
        program,
        PreparedSlotRead(PreparedStore { settings, program }),
    )
}

fn slot_reply(index: u8, program: &[u8; PROGRAM_BYTES]) -> Vec<u8> {
    guitar(&[&[0x45, index], sysex_words(program).as_slice()].concat())
}

#[test]
fn slot_read_sends_only_the_approved_identity_and_slot_messages_and_saves_three_programs() {
    let (original, prepared) = baseline();
    let spare = [0xff; PROGRAM_BYTES];
    let dir = tempfile::tempdir().unwrap();
    let mut podxt = Fake::new(DevicePath::PodxtMidi, [PODXT_IDENTITY.to_vec()].into());
    let mut variax = guitar_with_replies([
        slot_reply(0x7f, &original),
        slot_reply(0x32, &original),
        slot_reply(0x3e, &spare),
    ]);
    let replies = slot_read::after_podxt_identity(&mut podxt, &mut Quiet, |log| {
        slot_read::capture(&mut variax, log, &prepared, dir.path(), || false)
    })
    .unwrap();
    assert_eq!(podxt.writes, [IDENTITY_REQUEST]);
    assert_eq!(
        variax.writes,
        [
            IDENTITY_REQUEST.to_vec(),
            BRIDGE_ON.to_vec(),
            guitar(&[0x2a, 1]),
            guitar(&[0x44, 0x7f]),
            guitar(&[0x44, 0x32]),
            guitar(&[0x44, 0x3e]),
            guitar(&[0x2a, 0]),
            BRIDGE_OFF.to_vec(),
        ]
    );
    for (file, expected) in [("7F", original), ("32", original), ("3E", spare)] {
        assert_eq!(
            fs::read(dir.path().join(format!("index-{file}.program"))).unwrap(),
            expected
        );
    }
    let summary = slot_read::summarize(&prepared, &replies);
    assert!(summary[0].ends_with("equal."));
    assert!(summary[1].ends_with("yes."));
    assert!(summary[2].ends_with("equal."));
    assert!(summary[3].contains("matches"));
    assert!(summary[4].contains("matches"));
    assert!(summary[5].contains("mismatch"));
}

#[test]
fn slot_read_reports_actual_first_differing_words_and_nonblank_spare() {
    let (original, prepared) = baseline();
    let mut changed = original;
    changed[5] = 0x39;
    let dir = tempfile::tempdir().unwrap();
    let fake = guitar_with_replies([
        slot_reply(0x7f, &original),
        slot_reply(0x32, &changed),
        slot_reply(0x3e, &original),
    ]);
    let replies = slot_read::capture(fake, &mut Quiet, &prepared, dir.path(), || false).unwrap();
    let summary = slot_read::summarize(&prepared, &replies);
    assert!(summary[0].contains("word 1 (zero-based), read 0x000039, reference 0x000000"));
    assert!(summary[1].ends_with("no."));
    assert!(summary[2].contains("word 1 (zero-based), read 0x000000, reference 0x000039"));
}

#[test]
fn slot_read_preserves_the_first_program_when_the_next_reply_is_missing() {
    let (original, prepared) = baseline();
    let dir = tempfile::tempdir().unwrap();
    let mut fake = guitar_with_replies([slot_reply(0x7f, &original)]);
    let error =
        slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).unwrap_err();
    assert!(error.contains("Index 0x32: Timed out"));
    assert_eq!(
        fs::read(dir.path().join("index-7F.program")).unwrap(),
        original
    );
    assert!(!dir.path().join("index-32.program").exists());
    assert_eq!(
        fake.writes[4..],
        [
            guitar(&[0x44, 0x32]),
            guitar(&[0x2a, 0]),
            BRIDGE_OFF.to_vec()
        ]
    );
}

#[test]
fn slot_read_rejects_wrong_active_metadata_before_either_stored_slot() {
    let (_, prepared) = baseline();
    let wrong = [0; PROGRAM_BYTES];
    let dir = tempfile::tempdir().unwrap();
    let mut fake = guitar_with_replies([slot_reply(0x7f, &wrong)]);
    let error =
        slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).unwrap_err();
    assert!(error.contains("metadata differs"));
    assert_eq!(
        fake.writes[3..],
        [
            guitar(&[0x44, 0x7f]),
            guitar(&[0x2a, 0]),
            BRIDGE_OFF.to_vec()
        ]
    );
    assert_eq!(
        fs::read(dir.path().join("index-7F.program")).unwrap(),
        wrong
    );
}

#[test]
fn slot_read_refuses_to_overwrite_a_program_file() {
    let (original, prepared) = baseline();
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("index-7F.program");
    fs::write(&existing, b"previous capture").unwrap();
    let mut fake = guitar_with_replies([slot_reply(0x7f, &original)]);
    assert!(slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).is_err());
    assert_eq!(fs::read(existing).unwrap(), b"previous capture");
    assert_eq!(fake.writes[4..], [guitar(&[0x2a, 0]), BRIDGE_OFF.to_vec()]);
}

#[test]
fn slot_read_refuses_wrong_live_identities_before_enabling_the_bridge() {
    let (_, prepared) = baseline();
    let dir = tempfile::tempdir().unwrap();
    let mut wrong_podxt = PODXT_IDENTITY;
    wrong_podxt[15] = 0;
    let mut podxt = Fake::new(DevicePath::PodxtMidi, [wrong_podxt.to_vec()].into());
    let result: Result<(), _> = slot_read::after_podxt_identity(&mut podxt, &mut Quiet, |_| {
        panic!("Variax interface must stay unopened after a PODxt identity mismatch")
    });
    assert!(
        result
            .unwrap_err()
            .contains("Expected PODxt Live firmware 3.01")
    );
    assert_eq!(podxt.writes, [IDENTITY_REQUEST]);

    let mut wrong_variax = VARIAX_IDENTITY;
    wrong_variax[15] = b'1';
    let mut variax = Fake::new(DevicePath::VariaxViaPodxt, [wrong_variax.to_vec()].into());
    let result = slot_read::capture(&mut variax, &mut Quiet, &prepared, dir.path(), || false);
    assert!(
        result
            .unwrap_err()
            .contains("Expected Variax 300/600 MIDI identity")
    );
    assert_eq!(variax.writes, [IDENTITY_REQUEST]);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn slot_read_retries_a_variax_identity_tail_once_and_does_not_retry_silence() {
    let (original, prepared) = baseline();
    for (identity_replies, requests, retry_succeeds) in [
        (vec![], 1, false),
        (vec![vec![0xf7]], 2, false),
        (
            vec![
                vec![0xf7],
                vec![],
                vec![],
                vec![],
                VARIAX_IDENTITY.to_vec(),
                slot_reply(0x7f, &original),
                slot_reply(0x32, &original),
                slot_reply(0x3e, &[0xff; PROGRAM_BYTES]),
            ],
            2,
            true,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut variax = Fake::new(DevicePath::VariaxViaPodxt, identity_replies.into());
        let result = slot_read::capture(&mut variax, &mut Quiet, &prepared, dir.path(), || false);
        assert_eq!(result.is_ok(), retry_succeeds);
        assert_eq!(variax.writes[..requests], vec![IDENTITY_REQUEST; requests]);
        if retry_succeeds {
            assert_eq!(variax.writes[requests], BRIDGE_ON);
            assert_eq!(variax.writes.len(), requests + 7);
        } else {
            assert_eq!(variax.writes.len(), requests);
        }
    }
}

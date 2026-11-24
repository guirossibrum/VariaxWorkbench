use super::*;
use crate::procedure::slot_read::{self, PreparedSlotRead};

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
fn slot_read_sends_only_the_seven_approved_messages_and_saves_three_programs() {
    let (original, prepared) = baseline();
    let spare = [0xff; PROGRAM_BYTES];
    let dir = tempfile::tempdir().unwrap();
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        [
            slot_reply(0x7f, &original),
            slot_reply(0x32, &original),
            slot_reply(0x3e, &spare),
        ]
        .into(),
    );
    let replies =
        slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).unwrap();
    assert_eq!(
        fake.writes,
        [
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
    let fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        [
            slot_reply(0x7f, &original),
            slot_reply(0x32, &changed),
            slot_reply(0x3e, &original),
        ]
        .into(),
    );
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
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        [slot_reply(0x7f, &original)].into(),
    );
    let error =
        slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).unwrap_err();
    assert!(error.contains("Index 0x32: Timed out"));
    assert_eq!(
        fs::read(dir.path().join("index-7F.program")).unwrap(),
        original
    );
    assert!(!dir.path().join("index-32.program").exists());
    assert_eq!(
        fake.writes[3..],
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
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        [slot_reply(0x7f, &wrong)].into(),
    );
    let error =
        slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).unwrap_err();
    assert!(error.contains("metadata differs"));
    assert_eq!(
        fake.writes[2..],
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
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        [slot_reply(0x7f, &original)].into(),
    );
    assert!(slot_read::capture(&mut fake, &mut Quiet, &prepared, dir.path(), || false).is_err());
    assert_eq!(fs::read(existing).unwrap(), b"previous capture");
    assert_eq!(fake.writes[3..], [guitar(&[0x2a, 0]), BRIDGE_OFF.to_vec()]);
}

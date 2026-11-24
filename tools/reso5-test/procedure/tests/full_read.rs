use super::*;
use crate::procedure::full_read as procedure;

const STEPS: [(usize, [u8; 3]); 5] = [
    (64, [0, 4, 0]),
    (128, [0, 8, 0]),
    (256, [0, 0, 1]),
    (512, [0, 0, 2]),
    (1092, [4, 4, 4]),
];

fn fixture() -> [u8; PROGRAM_BYTES] {
    let mut bytes = [0; PROGRAM_BYTES];
    for (i, word) in bytes.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        word.copy_from_slice(&(0x12_3400 + i as u32).to_be_bytes()[1..]);
    }
    bytes
}

fn reads(words: usize) -> usize {
    8 + 6 * words
}

fn guitar_sequence(steps: usize) -> Vec<Vec<u8>> {
    let mut expected = vec![
        line6_devices::identity::REQUEST.to_vec(),
        BRIDGE_ON.into(),
        guitar(&[0x2a, 1]),
    ];
    for (_, [a, b, c]) in &STEPS[..steps] {
        expected.push(guitar(&[0x3c, *a, *b, *c, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    }
    expected.extend([guitar(&[0x2a, 0]), BRIDGE_OFF.into()]);
    expected
}

fn settled_replies(data: &[u8; PROGRAM_BYTES], steps: usize) -> Vec<Vec<u8>> {
    STEPS[..steps]
        .iter()
        .map(|(count, _)| reply(&data[..count * 3]))
        .collect()
}

fn fragmented_inbox(replies: &[Vec<u8>]) -> VecDeque<Vec<u8>> {
    // The fixture advances a full polling interval per read. Fragment at 160
    // bytes to exercise reassembly without exhausting the virtual deadline.
    std::iter::once(VARIAX_IDENTITY.to_vec())
        .chain(
            replies
                .iter()
                .flat_map(|reply| reply.chunks(160).map(<[u8]>::to_vec)),
        )
        .collect()
}

fn reply_path(dir: &Path, count: usize) -> std::path::PathBuf {
    dir.join(format!("start-0-count-{count}.syx"))
}

#[test]
fn exact_identity_and_stepped_read_sequence_saves_each_reply_and_compares_every_word() {
    let dir = tempfile::tempdir().unwrap();
    let data = fixture();
    let replies = settled_replies(&data, STEPS.len());
    let mut podxt = Fake::new(DevicePath::PodxtMidi, [PODXT_IDENTITY.into()].into());
    let mut variax = Fake::new(DevicePath::VariaxViaPodxt, fragmented_inbox(&replies));
    let captured =
        crate::procedure::slot_read::after_podxt_identity(&mut podxt, &mut Quiet, |log| {
            procedure::capture(&mut variax, log, dir.path(), || false)
        })
        .unwrap();
    assert_eq!(podxt.writes, [line6_devices::identity::REQUEST]);
    assert_eq!(variax.writes, guitar_sequence(STEPS.len()));
    assert_eq!(variax.pauses, [Duration::from_millis(100)]);
    assert!(variax.inbox.is_empty());
    for ((count, _), raw) in STEPS.iter().zip(&replies) {
        assert_eq!(raw.len(), reads(*count));
        assert_eq!(fs::read(reply_path(dir.path(), *count)).unwrap(), *raw);
    }
    let mut factory = data;
    factory[0] = 0xff;
    factory[PROGRAM_BYTES - 1] = 0;
    let summary = procedure::summarize(&factory, &captured);
    assert_eq!(
        summary,
        [
            "Word 0: active 0x123400, factory 0xFF3400",
            "Word 1091: active 0x123843, factory 0x123800",
            "Compared 1092 active words against factory Reso-5. 2 differing words (no live-word exclusions)."
        ]
    );
}

#[test]
fn a_stall_at_any_step_keeps_prior_replies_and_only_sends_cleanup() {
    let data = fixture();
    for (failed, (count, _)) in STEPS.iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let prior = settled_replies(&data, failed);
        let mut inbox = fragmented_inbox(&prior);
        let mut incomplete = reply(&data[..count * 3]);
        incomplete.pop(); // All the words arrived. The final F7 never did.
        inbox.extend(incomplete.chunks(160).map(<[u8]>::to_vec));
        let mut fake = Fake::new(DevicePath::VariaxViaPodxt, inbox);
        let error = procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).unwrap_err();
        assert!(error.contains(&format!("Start 0, count {count}: Timed out")));
        assert_eq!(fake.writes, guitar_sequence(failed + 1));
        let earlier_polls: usize = prior.iter().map(|raw| raw.len().div_ceil(160)).sum();
        assert_eq!(
            fake.clock,
            Duration::from_millis(200 + 100 * earlier_polls as u64) + Duration::from_secs(10)
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), failed);
        for ((earlier_count, _), raw) in STEPS[..failed].iter().zip(prior) {
            assert_eq!(
                fs::read(reply_path(dir.path(), *earlier_count)).unwrap(),
                raw
            );
        }
    }
}

#[test]
fn malformed_reply_stops_before_the_next_step_even_with_more_input_available() {
    let dir = tempfile::tempdir().unwrap();
    let data = fixture();
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        fragmented_inbox(&[reply(&data[..63 * 3]), reply(&data[..64 * 3])]),
    );
    let error = procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).unwrap_err();
    assert!(error.contains("Start 0, count 64: Variax 0x3C reply has 386 bytes, expected 392."));
    assert_eq!(fake.writes, guitar_sequence(1));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    assert!(!fake.inbox.is_empty());
}

#[test]
fn wrong_variax_identity_stops_before_the_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let mut bad = VARIAX_IDENTITY;
    bad[14] = b'0';
    let mut fake = Fake::new(DevicePath::VariaxViaPodxt, [bad.into()].into());
    assert!(
        procedure::capture(&mut fake, &mut Quiet, dir.path(), || false)
            .unwrap_err()
            .contains("firmware 3.10")
    );
    assert_eq!(fake.writes, [line6_devices::identity::REQUEST]);
}

#[test]
fn offline_preparation_refuses_an_unreadable_archive() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        procedure::prepare(&dir.path().join("missing.vxf"))
            .unwrap_err()
            .contains("missing.vxf")
    );
}

#[test]
fn wrong_podxt_identity_prevents_the_variax_transport_from_opening() {
    let mut bad = PODXT_IDENTITY;
    bad[15] = 2;
    let mut fake = Fake::new(DevicePath::PodxtMidi, [bad.into()].into());
    let error = crate::procedure::slot_read::after_podxt_identity(
        &mut fake,
        &mut Quiet,
        |_| -> Result<(), String> { panic!("unchecked PODxt identity admitted") },
    )
    .unwrap_err();
    assert!(error.contains("PODxt Live firmware 3.01"));
    assert_eq!(fake.writes, [line6_devices::identity::REQUEST]);
}

#[test]
fn a_capture_collision_preserves_the_existing_file_and_stops() {
    let dir = tempfile::tempdir().unwrap();
    let filename = reply_path(dir.path(), 64);
    fs::File::create_new(&filename).unwrap();
    let replies = settled_replies(&fixture(), 1);
    let mut fake = Fake::new(DevicePath::VariaxViaPodxt, fragmented_inbox(&replies));
    assert!(procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).is_err());
    assert_eq!(fake.writes, guitar_sequence(1));
    assert_eq!(fs::metadata(filename).unwrap().len(), 0);
}

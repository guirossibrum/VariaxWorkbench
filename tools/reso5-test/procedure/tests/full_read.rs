use super::*;
use crate::procedure::full_read as procedure;

const REQUESTS: usize = 35;
const USB_PACKET_BYTES: usize = 16;

fn chunks() -> impl Iterator<Item = (usize, usize)> {
    (0..34).map(|index| (index * 32, 32)).chain([(1088, 4)])
}

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

pub(super) fn guitar_sequence(requests: usize) -> Vec<Vec<u8>> {
    let mut expected = vec![
        line6_devices::identity::REQUEST.to_vec(),
        BRIDGE_ON.into(),
        guitar(&[0x2a, 1]),
    ];
    for (start, count) in chunks().take(requests) {
        let count_nibbles = if count == 32 {
            [0, 2, 0, 0, 0, 0]
        } else {
            [4, 0, 0, 0, 0, 0]
        };
        let start_nibbles = [
            (start & 15) as u8,
            ((start >> 4) & 15) as u8,
            (start >> 8) as u8,
            0,
            0,
            0,
        ];
        expected.push(guitar(
            &[&[0x3c][..], &count_nibbles, &start_nibbles].concat(),
        ));
    }
    expected.extend([guitar(&[0x2a, 0]), BRIDGE_OFF.into()]);
    expected
}

pub(super) fn settled_replies(data: &[u8; PROGRAM_BYTES], requests: usize) -> Vec<Vec<u8>> {
    chunks()
        .take(requests)
        .map(|(start, count)| reply(&data[start * 3..(start + count) * 3]))
        .collect()
}

fn fragmented_inbox(replies: &[Vec<u8>]) -> VecDeque<Vec<u8>> {
    std::iter::once(VARIAX_IDENTITY.to_vec())
        .chain(
            replies
                .iter()
                .flat_map(|reply| reply.chunks(USB_PACKET_BYTES).map(<[u8]>::to_vec)),
        )
        .collect()
}

fn reply_path(dir: &Path, start: usize, count: usize) -> std::path::PathBuf {
    dir.join(format!("start-{start}-count-{count}.syx"))
}

#[test]
fn exact_35_chunk_sequence_saves_and_assembles_all_words() {
    let dir = tempfile::tempdir().unwrap();
    let data = fixture();
    let replies = settled_replies(&data, REQUESTS);
    let mut podxt = Fake::new(DevicePath::PodxtMidi, [PODXT_IDENTITY.into()].into());
    let mut variax = Fake::new(DevicePath::VariaxViaPodxt, fragmented_inbox(&replies));
    let captured =
        crate::procedure::slot_read::after_podxt_identity(&mut podxt, &mut Quiet, |log| {
            procedure::capture(&mut variax, log, dir.path(), || false)
        })
        .unwrap();
    assert_eq!(podxt.writes, [line6_devices::identity::REQUEST]);
    assert_eq!(variax.writes, guitar_sequence(REQUESTS));
    assert_eq!(variax.writes.len(), REQUESTS + 5);
    assert_eq!(replies.len(), REQUESTS);
    assert_eq!(
        variax.pauses,
        [Duration::from_secs(1), Duration::from_millis(100)]
    );
    assert_eq!(
        variax.write_times[2] - variax.write_times[1],
        Duration::from_secs(1)
    );
    assert_eq!(
        variax.write_times[3] - variax.write_times[2],
        Duration::from_millis(100)
    );
    assert!(variax.inbox.is_empty());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), REQUESTS);
    for ((start, count), raw) in chunks().zip(&replies) {
        assert_eq!(raw.len(), reads(count));
        assert_eq!(
            fs::read(reply_path(dir.path(), start, count)).unwrap(),
            *raw
        );
    }
    for (index, word) in captured.into_iter().enumerate() {
        assert_eq!(word, 0x12_3400 + index as u32, "word {index}");
    }
    let mut factory = data;
    factory[0] = 0xff;
    factory[PROGRAM_BYTES - 1] = 0;
    let summary = procedure::summarize(&factory, &captured);
    assert_eq!(
        summary,
        [
            "Word 0: active 0x123400, factory 0xFF3400 (finding)",
            "Word 1091: active 0x123843, factory 0x123800 (finding)",
            "Compared 1092 active words against factory Reso-5. Findings: 2. Unknown differences: 0."
        ]
    );
}

#[test]
fn silence_stops_and_keeps_completed_captures_even_after_retrying_an_incomplete_chunk() {
    let data = fixture();
    for (failed, silent) in [(0, true), (17, false), (34, false)] {
        let dir = tempfile::tempdir().unwrap();
        let (start, count) = chunks().nth(failed).unwrap();
        let prior = settled_replies(&data, failed);
        let mut inbox = fragmented_inbox(&prior);
        if !silent {
            let mut incomplete = reply(&data[start * 3..(start + count) * 3]);
            incomplete.pop(); // All the words arrived. The final F7 never did.
            inbox.extend(incomplete.chunks(USB_PACKET_BYTES).map(<[u8]>::to_vec));
        }
        let mut fake = Fake::new(DevicePath::VariaxViaPodxt, inbox);
        let error = procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).unwrap_err();
        assert!(error.contains(&format!("Start {start}, count {count}: Timed out")));
        let mut expected = guitar_sequence(failed + 1);
        if !silent {
            let cleanup = expected.len() - 2;
            expected.insert(cleanup, expected[cleanup - 1].clone());
        }
        assert_eq!(fake.writes, expected);
        let earlier_polls: usize = prior
            .iter()
            .map(|raw| raw.len().div_ceil(USB_PACKET_BYTES))
            .sum();
        let retry_time = if silent {
            Duration::ZERO
        } else {
            Duration::from_millis(10_100)
        };
        assert_eq!(
            fake.clock,
            Duration::from_millis(1200 + 100 * earlier_polls as u64)
                + Duration::from_secs(10)
                + retry_time
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), failed);
        for ((earlier_start, earlier_count), raw) in chunks().zip(prior) {
            assert_eq!(
                fs::read(reply_path(dir.path(), earlier_start, earlier_count)).unwrap(),
                raw
            );
        }
    }
}

#[test]
fn three_malformed_replies_stop_before_the_next_chunk_even_with_more_input_available() {
    let dir = tempfile::tempdir().unwrap();
    let data = fixture();
    let short = reply(&data[..31 * 3]);
    let mut fake = Fake::new(
        DevicePath::VariaxViaPodxt,
        fragmented_inbox(&[short.clone(), short.clone(), short, reply(&data[..32 * 3])]),
    );
    let error = procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).unwrap_err();
    assert!(error.contains("Start 0, count 32: Variax 0x3C reply has 194 bytes, expected 200."));
    assert!(error.contains("Failed after 3 attempts"));
    let mut expected = guitar_sequence(1);
    expected.splice(4..4, [expected[3].clone(), expected[3].clone()]);
    assert_eq!(fake.writes, expected);
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
    let filename = reply_path(dir.path(), 0, 32);
    fs::File::create_new(&filename).unwrap();
    let replies = settled_replies(&fixture(), 1);
    let mut fake = Fake::new(DevicePath::VariaxViaPodxt, fragmented_inbox(&replies));
    assert!(procedure::capture(&mut fake, &mut Quiet, dir.path(), || false).is_err());
    assert_eq!(fake.writes, guitar_sequence(1));
    assert_eq!(fs::metadata(filename).unwrap().len(), 0);
}

#[test]
fn reports_only_words_871_and_872_as_unknown() {
    let factory = [0; PROGRAM_BYTES];
    let mut captured = [0; 1092];
    captured[870..874].fill(1);
    assert_eq!(
        procedure::summarize(&factory, &captured),
        [
            "Word 870: active 0x000001, factory 0x000000 (finding)",
            "Word 871: active 0x000001, factory 0x000000 (unknown, not a failure)",
            "Word 872: active 0x000001, factory 0x000000 (unknown, not a failure)",
            "Word 873: active 0x000001, factory 0x000000 (finding)",
            "Compared 1092 active words against factory Reso-5. Findings: 2. Unknown differences: 2.",
        ]
    );
}

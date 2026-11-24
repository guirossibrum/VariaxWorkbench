use super::*;
use crate::{
    family::variax::VariaxRequest,
    gate::{AllClosed, GatePolicy},
    session::{
        Log, Quiet,
        testing::{Fake, packets},
    },
};
use std::cell::Cell;

struct ReadOnly;

impl GatePolicy for ReadOnly {
    fn allows(&self, operation: Operation) -> bool {
        operation == Operation::VariaxRead
    }
}

fn span(count: u16) -> PresetSpan {
    PresetSpan::new(0, count).unwrap()
}

fn payload(count: u16) -> Vec<u32> {
    (0..u32::from(count)).map(|i| 0xfed000 + i).collect()
}

fn frame(opcode: u8, body: &[u8]) -> Vec<u8> {
    [&[0xf0, 0, 1, 0x0c, 7, 4, opcode][..], body, &[0xf7]].concat()
}

fn encode(words: &[u32]) -> Vec<u8> {
    let nibbles = words
        .iter()
        .flat_map(|word| word.to_le_bytes()[..3].to_vec())
        .flat_map(|byte| [byte & 0xf, byte >> 4])
        .collect::<Vec<_>>();
    frame(0x3c, &nibbles)
}

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

fn conversation(span: PresetSpan, replies: Vec<Vec<u8>>) -> Script {
    attempts(span, [replies])
}

fn attempts(span: PresetSpan, replies: impl IntoIterator<Item = Vec<Vec<u8>>>) -> Script {
    let mut script = vec![
        (vec![0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7], vec![]),
        (VariaxRequest::SessionOn.bytes(), vec![]),
    ];
    script.extend(replies.into_iter().map(|packets| (span.bytes(), packets)));
    script.extend([
        (VariaxRequest::SessionOff.bytes(), vec![]),
        (vec![0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7], vec![]),
    ]);
    script
}

#[test]
fn refuses_empty_out_of_bounds_and_overflowing_spans() {
    for (start, count) in [
        (0, 0),
        (1091, 0),
        (1092, 1),
        (1091, 2),
        (0, 1093),
        (u16::MAX, 1),
        (1, u16::MAX),
        (u16::MAX, u16::MAX),
    ] {
        assert_eq!(PresetSpan::new(start, count), None);
    }
    assert_eq!((span(1092).start(), span(1092).count()), (0, 1092));
    assert!(PresetSpan::new(1091, 1).is_some());
    assert!(PresetSpan::new(1000, 92).is_some());
}

#[test]
fn encodes_count_before_start_and_reuses_the_metadata_wire_shape() {
    for (start, expected) in [
        (851, VariaxRequest::Metadata1),
        (883, VariaxRequest::Metadata2),
    ] {
        let request = PresetSpan::new(start, 32).unwrap();
        assert_eq!(request.bytes(), expected.bytes());
        assert_eq!(request.operation(), Operation::VariaxRead);
    }
    assert_eq!(
        PresetSpan::new(1091, 1).unwrap().bytes(),
        [
            0xf0, 0, 1, 0x0c, 7, 0, 0x3c, 1, 0, 0, 0, 0, 0, 3, 4, 4, 0, 0, 0, 0xf7
        ]
    );
}

#[test]
fn decodes_each_reply_size_into_canonical_words() {
    for (count, length) in [
        (64, 392),
        (128, 776),
        (256, 1544),
        (512, 3080),
        (1092, 6560),
    ] {
        let request = span(count);
        let words = payload(count);
        let mut raw = encode(&words);
        assert_eq!(raw.len(), length);
        assert_eq!(request.reply_bytes(), length);
        assert_eq!(request.decode_reply(&raw).unwrap(), words);
        raw[5] = 0;
        assert_eq!(request.decode_reply(&raw).unwrap(), words);
    }
}

#[test]
fn rejects_wrong_envelope_length_terminator_and_nibbles() {
    let request = span(64);
    let valid = encode(&payload(64));
    let mut wrong_family = valid.clone();
    wrong_family[4] = 3;
    let mut wrong_id = valid.clone();
    wrong_id[5] = 1;
    let mut wrong_opcode = valid.clone();
    wrong_opcode[6] = 0x44;
    let mut non_nibble = valid.clone();
    non_nibble[7] = 16;
    for bad in [
        vec![],
        wrong_family,
        wrong_id,
        wrong_opcode,
        non_nibble,
        valid[..valid.len() - 1].to_vec(),
        encode(&payload(63)),
        encode(&payload(65)),
    ] {
        assert!(request.decode_reply(&bad).is_err());
    }
}

#[test]
fn closed_policy_and_wrong_path_refuse_without_sending() {
    for (path, policy) in [
        (Path::VariaxViaPodxt, &AllClosed as &dyn GatePolicy),
        (Path::PodxtMidi, &ReadOnly as &dyn GatePolicy),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut log);
        assert!(read_spans(&mut session, &[span(64)], || false, |_| Ok(())).is_err());
        assert!(session.link.writes.is_empty());
    }
}

#[derive(Default)]
struct Notes(Vec<String>);

impl Log for Notes {
    fn line(&mut self, text: String) {
        self.0.push(text);
    }
}

#[test]
fn drains_through_f7_and_skips_only_known_notices_and_unsolicited_controls() {
    let words = payload(64);
    let raw = encode(&words);
    let mut replies = packets(&frame(0x28, &[]));
    replies.push([frame(0x4d, &[1]), vec![0xb0, 79, 55]].concat());
    replies.extend(packets(&raw[..raw.len() - 1]));
    replies.push([vec![0xf7], frame(0x74, &[])[..5].to_vec()].concat());
    replies.push(frame(0x74, &[])[5..].to_vec());
    let mut log = Notes::default();
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, conversation(span(64), replies)),
        &ReadOnly,
        &mut log,
    );
    let mut got = None;
    read_spans(
        &mut session,
        &[span(64)],
        || false,
        |reply| {
            got = Some(reply);
            Ok(())
        },
    )
    .unwrap();
    session.link.assert_finished();
    let captured = got.unwrap();
    assert_eq!(captured.span, span(64));
    assert_eq!(captured.raw, raw);
    assert_eq!(captured.words, words);
    for notice in ["0x28", "0x4D", "0x74"] {
        assert!(
            log.0
                .iter()
                .any(|line| line.contains(&format!("Skipped Variax notice {notice}")))
        );
    }
}

#[test]
fn a_dropped_byte_retries_the_same_span_after_100_ms_and_delivers_only_the_good_reply() {
    let span = PresetSpan::new(416, 32).unwrap();
    let words = payload(32);
    let valid = encode(&words);
    let mut short = valid.clone();
    short.remove(99);
    let mut log = Notes::default();
    let attempts = [packets(&short), packets(&valid)];
    let packet_time =
        Duration::from_millis(4 * attempts.iter().map(Vec::len).sum::<usize>() as u64);
    let script = self::attempts(span, attempts);
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &ReadOnly,
        &mut log,
    );
    let mut captures = Vec::new();
    read_spans(
        &mut session,
        &[span],
        || false,
        |reply| {
            captures.push(reply);
            Ok(())
        },
    )
    .unwrap();
    session.link.assert_finished();
    assert_eq!(session.link.writes[2..4], [span.bytes(), span.bytes()]);
    assert_eq!(
        session.link.elapsed(),
        BRIDGE_SETTLE + Duration::from_millis(100) + packet_time + Duration::from_millis(100),
    );
    assert_eq!(captures.len(), 1);
    assert_eq!(captures[0].raw, valid);
    assert_eq!(captures[0].words, words);
    assert!(log.0.iter().any(|line| line.contains(
        "Retrying Variax 0x3C start 416, count 32 (attempt 2/3): Variax 0x3C reply has 199 bytes, expected 200."
    )));
}

#[test]
fn three_bad_frames_stop_before_the_next_span_and_cleanup() {
    let span = span(32);
    let mut long = encode(&payload(32));
    long.insert(100, 0);
    let mut malformed = encode(&payload(32));
    malformed[9] = 16;
    let valid = encode(&payload(32));
    let mut log = Notes::default();
    let script = attempts(
        span,
        [
            packets(&long),
            packets(&malformed),
            packets(&[&valid[..80], &valid[..]].concat()),
        ],
    );
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &ReadOnly,
        &mut log,
    );
    let error = read_spans(
        &mut session,
        &[span, PresetSpan::new(32, 32).unwrap()],
        || false,
        |_| panic!("malformed frame was accepted"),
    )
    .unwrap_err();
    assert!(error.starts_with("Start 0, count 32: New SysEx before the previous F7."));
    assert!(error.contains("Failed after 3 attempts"));
    session.link.assert_finished();
    let retries: Vec<_> = log
        .0
        .iter()
        .filter(|line| line.contains("Retrying"))
        .collect();
    assert_eq!(retries.len(), 2);
    assert!(retries[0].contains("(attempt 2/3): Variax 0x3C reply has 201 bytes, expected 200."));
    assert!(retries[1].contains("(attempt 3/3):"));
}

#[test]
fn missing_f7_is_drained_to_the_deadline_and_the_retry_starts_a_new_frame() {
    let span = span(32);
    let valid = encode(&payload(32));
    let mut log = Quiet;
    let script = attempts(span, [packets(&valid[..valid.len() - 1]), packets(&valid)]);
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &ReadOnly,
        &mut log,
    );
    read_spans(
        &mut session,
        &[span],
        || false,
        |reply| {
            assert_eq!(reply.raw, valid);
            Ok(())
        },
    )
    .unwrap();
    session.link.assert_finished();
}

#[test]
fn cancellation_after_a_bad_reply_prevents_resending_it() {
    let span = span(32);
    let script = conversation(span, packets(&encode(&payload(31))));
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &ReadOnly,
        &mut log,
    );
    let checks = Cell::new(0);
    let error = read_spans(
        &mut session,
        &[span],
        || {
            checks.set(checks.get() + 1);
            checks.get() == 3
        },
        |_| panic!("malformed frame was accepted"),
    )
    .unwrap_err();
    assert!(error.contains("cancelled"));
    session.link.assert_finished();
}

#[test]
fn cancellation_between_spans_preserves_the_in_flight_reply_and_cleans_up() {
    let checks = Cell::new(0);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(
            Path::VariaxViaPodxt,
            conversation(span(64), packets(&encode(&payload(64)))),
        ),
        &ReadOnly,
        &mut log,
    );
    let mut captured = 0;
    let error = read_spans(
        &mut session,
        &[span(64), span(128)],
        || {
            checks.set(checks.get() + 1);
            checks.get() == 3
        },
        |_| {
            captured += 1;
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("cancelled"));
    assert_eq!(captured, 1);
    session.link.assert_finished();
}

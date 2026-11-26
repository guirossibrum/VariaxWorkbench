use super::*;
use crate::{
    family::variax::{VariaxRequest, span_read::PresetSpan},
    gate::{AllClosed, GatePolicy},
    identity::{REQUEST, tests::VARIAX_600},
    session::{
        Path, Quiet, Request,
        testing::{Fake, packets},
    },
};

struct ReadOnly;

impl GatePolicy for ReadOnly {
    fn allows(&self, operation: Operation) -> bool {
        matches!(operation, Operation::VariaxIdentity | Operation::VariaxRead)
    }
}

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 0, 0xf7];

fn identity(version: &[u8; 4]) -> [u8; 17] {
    let mut reply = VARIAX_600;
    reply[12..16].copy_from_slice(version);
    reply
}

fn encode(words: &[u32]) -> Vec<u8> {
    [0xf0, 0, 1, 0xc, 7, 4, 0x3c]
        .into_iter()
        .chain(
            words
                .iter()
                .flat_map(|word| (0..6).map(move |nibble| ((word >> (nibble * 4)) & 0xf) as u8)),
        )
        .chain([0xf7])
        .collect()
}

fn span(start: usize, count: usize) -> Vec<u8> {
    PresetSpan::new(start as u16, count as u16).unwrap().bytes()
}

fn settings_script(reply: &[u8], words: &FullBuffer) -> Script {
    vec![
        (REQUEST.to_vec(), packets(reply)),
        (VariaxRequest::SessionOn.bytes(), vec![]),
        (span(851, 32), packets(&encode(&words[851..883]))),
        (span(883, 32), packets(&encode(&words[883..915]))),
        (VariaxRequest::SessionOff.bytes(), vec![]),
        (BRIDGE_ON.to_vec(), vec![]),
        (VariaxRequest::SessionOn.bytes(), vec![]),
    ]
}

fn finish_buffer_script(script: &mut Script) {
    script.push((VariaxRequest::SessionOff.bytes(), vec![]));
    script.push((BRIDGE_OFF.to_vec(), vec![]));
}

#[test]
fn closed_policy_refuses_before_any_traffic() {
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, vec![]),
        &AllClosed,
        &mut log,
    );
    assert!(matches!(
        read_current_program(&mut session, || false, |_| Ok(())),
        Err(error) if error.contains("requires identity and read permission")
    ));
    assert!(session.link.writes.is_empty());
}

#[test]
fn reads_settings_and_all_35_chunks_for_verified_and_unverified_firmware() {
    let mut words: FullBuffer = std::array::from_fn(|index| index as u32);
    words[851..915].fill(0);
    words[851] = 0x416263; // "Abc", with the unused name bytes zero-padded.
    for (version, expected) in [
        (b"0310", FirmwareStatus::Verified310),
        (b"0309", FirmwareStatus::Unverified),
    ] {
        let reply = identity(version);
        let mut script = settings_script(&reply, &words);
        for (index, chunk) in words.chunks(32).enumerate() {
            script.push((span(index * 32, chunk.len()), packets(&encode(chunk))));
        }
        finish_buffer_script(&mut script);
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, script),
            &ReadOnly,
            &mut log,
        );
        let mut captured = Vec::new();
        let read = read_current_program(
            &mut session,
            || false,
            |reply| {
                captured.push((reply.span.start(), reply.span.count()));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(read.firmware, expected);
        assert_eq!(read.settings.identified.reply, reply);
        assert_eq!(read.settings.identified.identity.version, *version);
        assert_eq!(read.settings.metadata.name, "Abc");
        assert_eq!(&read.settings.metadata.raw[..3], b"Abc");
        assert_eq!(read.words, words);
        assert_eq!(captured.len(), 35);
        assert_eq!(captured.last(), Some(&(1088, 4)));
        assert!(captured[..34].iter().all(|&(_, count)| count == 32));
        session.link.assert_finished();
    }
}

#[test]
fn refuses_malformed_identity_and_other_variax_members_before_settings() {
    let mut wrong_member = identity(b"0310");
    wrong_member[10] = 0;
    let mut short = identity(b"0310").to_vec();
    short.remove(15);
    for (reply, reason) in [
        (identity(b"03x0").to_vec(), "malformed firmware version"),
        (wrong_member.to_vec(), "expects a Variax 300/600"),
        (short, "Unexpected SysEx"),
    ] {
        let script = vec![(REQUEST.to_vec(), packets(&reply))];
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, script),
            &ReadOnly,
            &mut log,
        );
        assert!(
            matches!(read_on(&mut session, || false, |_| Ok(())), Err(error) if error.contains(reason))
        );
        session.link.assert_finished();
    }
}

#[test]
fn a_malformed_program_chunk_retries_and_cleans_up_without_a_partial_result() {
    let words = [0; 1092];
    let mut script = settings_script(&identity(b"0309"), &words);
    script.push((span(0, 32), packets(&encode(&words[..32]))));
    let mut bad = encode(&words[32..64]);
    bad.remove(7);
    for _ in 0..3 {
        script.push((span(32, 32), packets(&bad)));
    }
    finish_buffer_script(&mut script);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &ReadOnly,
        &mut log,
    );
    let mut starts = Vec::new();
    let result = read_on(
        &mut session,
        || false,
        |reply| {
            starts.push(reply.span.start());
            Ok(())
        },
    );
    assert!(
        matches!(result, Err(error) if error.contains("Start 32, count 32") && error.contains("Failed after 3 attempts"))
    );
    assert_eq!(starts, [0]);
    session.link.assert_finished();
}

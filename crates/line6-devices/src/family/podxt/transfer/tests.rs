use super::*;
use crate::identity::tests::PODXT_LIVE;
use crate::session::Quiet;
use crate::session::testing::{AllowAll, Fake};

const TRANSCRIPT: &str = include_str!("../fixtures/line6-edit-3.06-transfer.txt");

pub(in crate::family::podxt) fn events(case: &str, tag: &str) -> Vec<Vec<u8>> {
    TRANSCRIPT
        .split_once(&format!("case {case}\n"))
        .expect("recorded Java scenario")
        .1
        .lines()
        .take_while(|line| !line.starts_with("case "))
        .filter_map(|line| line.strip_prefix(tag))
        .map(|hex| {
            let (pairs, suffix) = hex.as_bytes().as_chunks::<2>();
            assert!(suffix.is_empty(), "incomplete hex byte");
            pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        })
        .collect()
}

pub(in crate::family::podxt) fn tone() -> TonePayload {
    // Construct the synthetic patch independently from its recorded wire bytes.
    let mut data = [0; 160];
    data[..32].fill(b' ');
    data[..12].copy_from_slice(b"Offline Tone");
    data[32 + 7] = 127;
    data[32 + 13] = 64;
    data[32 + 96] = 10;
    TonePayload::from_bytes(data).unwrap()
}

#[test]
fn whole_tone_send_is_byte_identical_to_the_original_encoder() {
    let source = tone();
    let dump = source.edit_dump();
    let channel = Location::new(BankKind::Tones, 64).unwrap();
    let original = EditorBuffer {
        channel,
        tone: dump.clone(),
        packs: Packs(4),
    };
    let reply = |request: PodxtRequest, bytes: Vec<u8>| (request.bytes(), vec![bytes]);
    let channel_reply = || {
        reply(
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 4, 0, 0xf7],
        )
    };
    let mut identity = PODXT_LIVE.to_vec();
    identity[12..16].copy_from_slice(&[0, 3, 0, 1]);
    let script = vec![
        reply(PodxtRequest::Identity, identity),
        channel_reply(),
        reply(PodxtRequest::EditBuffer, dump.raw.clone()),
        reply(
            PodxtRequest::InstalledPacks,
            vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0x0a, 4, 0xf7],
        ),
        channel_reply(),
        (events("edit", "out ").remove(0), vec![dump.raw.clone()]),
        reply(PodxtRequest::EditBuffer, dump.raw.clone()),
        channel_reply(),
    ];
    let mut cached = original.clone();
    cached.tone.raw[39 + 7] = 50; // Get preceded a physical pedal movement.
    for (expected, candidate) in [
        (&original, WholeToneSource::Payload(&source)),
        (&cached, WholeToneSource::Current),
    ] {
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::PodxtMidi, script.clone()),
            &AllowAll,
            &mut quiet,
        );
        let result = send_whole_tone(&mut session, expected, candidate, || false).unwrap();
        assert_eq!(result.tone.raw, dump.raw);
        session.link.assert_finished();
    }
}

#[test]
fn whole_tone_validates_length_model_and_seven_bit_payload() {
    let original = tone();
    let mut malformed = original.edit_dump();
    malformed.raw.pop();
    assert!(TonePayload::from_dump(&malformed).is_err());

    let mut malformed = original.edit_dump();
    malformed.raw[6] = 2;
    assert!(TonePayload::from_dump(&malformed).is_err());

    let mut malformed = original.edit_dump();
    malformed.raw[39] = 0x80;
    assert!(TonePayload::from_dump(&malformed).is_err());
    assert_eq!(
        TonePayload::from_dump(&original.edit_dump()).unwrap(),
        original
    );
}

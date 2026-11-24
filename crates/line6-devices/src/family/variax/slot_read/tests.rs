use super::*;
use crate::{
    gate::AllClosed,
    session::{
        Quiet,
        testing::{AllowAll, Fake, packets},
    },
};

const FLASH: SlotIndex = SlotIndex::new(0x32).unwrap();
const CURRENT: SlotIndex = SlotIndex::CURRENT;
const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const SESSION_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7];
const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const READ_FLASH: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x44, 0x32, 0xf7];

fn reply(index: SlotIndex) -> Vec<u8> {
    let mut bytes = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x45, index.value()];
    bytes.resize(6560, 0);
    bytes.push(0xf7);
    bytes
}

#[test]
fn exact_requests_use_only_opcode_44_and_the_slot_policy() {
    for index in [0x7f, 0x32, 0x3e] {
        let request = SlotRead(SlotIndex::new(index).unwrap());
        assert_eq!(request.bytes(), [0xf0, 0, 1, 0xc, 7, 0, 0x44, index, 0xf7]);
        assert_eq!(request.path(), Path::VariaxViaPodxt);
        assert_eq!(request.operation(), Operation::VariaxSlotRead);
    }
    assert!(SlotIndex::new(0x80).is_none());
}

#[test]
fn ram_words_decode_low_nibble_first_and_check_the_firmware_sum() {
    let mut bytes = reply(CURRENT);
    for (offset, word) in [(8, 0xfe_dc_bau32), (6554, 0xfe_dc_ba)] {
        for nibble in 0..6 {
            bytes[offset + nibble] = ((word >> (4 * nibble)) & 0xf) as u8;
        }
    }
    let decoded = SlotReply::decode(CURRENT, &bytes).unwrap();
    assert_eq!(decoded.program[..3], [0xfe, 0xdc, 0xba]);
    assert_eq!(decoded.program[3273..], [0xfe, 0xdc, 0xba]);
    assert_eq!(decoded.checksum.stored, 0xfedcba);
    assert!(decoded.checksum.matches());
}

#[test]
fn flash_bytes_decode_as_big_endian_words_and_blank_is_retained() {
    let mut bytes = reply(FLASH);
    bytes[5] = 0;
    for (index, flash_byte) in [0x91, 0xab, 0x24].into_iter().enumerate() {
        bytes[8 + 2 * index] = flash_byte & 0xf;
        bytes[9 + 2 * index] = flash_byte >> 4;
    }
    assert_eq!(
        SlotReply::decode(FLASH, &bytes).unwrap().program[..3],
        [0x24, 0xab, 0x91]
    );
    bytes[8..6560].fill(0xf);
    let decoded = SlotReply::decode(FLASH, &bytes).unwrap();
    assert_eq!(decoded.program, [0xff; 3276]);
    assert_eq!(decoded.checksum.stored, 0xff_ffff);
    assert_eq!(decoded.checksum.computed, 0xff_fbbd);
    assert!(!decoded.checksum.matches());
}

#[test]
fn rejects_short_and_long_replies() {
    let mut short = reply(FLASH);
    short.remove(8);
    assert!(
        SlotReply::decode(FLASH, &short)
            .unwrap_err()
            .contains("6560 bytes")
    );
    let mut long = reply(FLASH);
    long.insert(8, 0);
    assert!(
        SlotReply::decode(FLASH, &long)
            .unwrap_err()
            .contains("6562 bytes")
    );
}

#[test]
fn rejects_wrong_echo_missing_eox_wrong_header_and_non_nibbles() {
    let mut bytes = reply(FLASH);
    assert!(
        SlotReply::decode(CURRENT, &bytes)
            .unwrap_err()
            .contains("echoed 32")
    );
    bytes[6560] = 0;
    assert!(
        SlotReply::decode(FLASH, &bytes)
            .unwrap_err()
            .contains("missing F7")
    );
    bytes[6560] = 0xf7;
    bytes[5] = 3;
    assert!(
        SlotReply::decode(FLASH, &bytes)
            .unwrap_err()
            .contains("envelope")
    );
    bytes[5] = 4;
    bytes[8] = 0x10;
    assert!(
        SlotReply::decode(FLASH, &bytes)
            .unwrap_err()
            .contains("non-nibble")
    );
}

fn script(replies: Vec<Vec<u8>>) -> Fake {
    Fake::from_bytes(
        Path::VariaxViaPodxt,
        vec![
            (BRIDGE_ON.into(), vec![]),
            (SESSION_ON.into(), vec![]),
            (READ_FLASH.into(), replies),
            (SESSION_OFF.into(), vec![]),
            (BRIDGE_OFF.into(), vec![]),
        ],
    )
}

#[test]
fn skips_clip_dsp_and_midi_queue_notices_and_reassembles_the_reply() {
    let mut inbox = [0x28, 0x4d, 0x74]
        .map(|op| vec![0xf0, 0, 1, 0xc, 7, 4, op, 0xf7])
        .to_vec();
    inbox.extend(packets(&reply(FLASH)));
    let mut log = Quiet;
    let mut session = Session::new(script(inbox), &AllowAll, &mut log);
    let mut count = 0;
    read_slots(
        &mut session,
        &[FLASH],
        || false,
        |decoded| {
            assert_eq!(decoded.index, FLASH);
            count += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(count, 1);
    session.link.assert_finished();
}

#[test]
fn silence_and_unterminated_frames_time_out_and_close_the_session() {
    for inbox in [vec![], packets(&reply(FLASH)[..6560])] {
        let mut log = Quiet;
        let mut session = Session::new(script(inbox), &AllowAll, &mut log);
        let error = read_slots(
            &mut session,
            &[FLASH],
            || false,
            |_| panic!("unexpected reply"),
        )
        .unwrap_err();
        assert!(error.contains("Timed out"));
        assert_eq!(session.link.elapsed(), SESSION_SETTLE + REPLY_WINDOW);
        session.link.assert_finished();
    }
}

#[test]
fn closed_policy_and_wrong_path_send_nothing() {
    for (path, policy) in [
        (
            Path::VariaxViaPodxt,
            &AllClosed as &dyn crate::gate::GatePolicy,
        ),
        (Path::PodxtMidi, &AllowAll),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut log);
        assert!(read_slots(&mut session, &[FLASH], || false, |_| Ok(())).is_err());
        assert!(session.sent.is_empty());
    }
}

#[test]
fn session_start_and_read_failures_still_attempt_both_cleanup_writes() {
    for failure in [1, 2] {
        let mut fake = if failure == 1 {
            Fake::from_bytes(
                Path::VariaxViaPodxt,
                vec![
                    (BRIDGE_ON.into(), vec![]),
                    (SESSION_ON.into(), vec![]),
                    (SESSION_OFF.into(), vec![]),
                    (BRIDGE_OFF.into(), vec![]),
                ],
            )
        } else {
            script(vec![])
        };
        fake.fail_at = vec![failure, failure + 1, failure + 2];
        let mut log = Quiet;
        let mut session = Session::new(fake, &AllowAll, &mut log);
        let error = read_slots(&mut session, &[FLASH], || false, |_| Ok(())).unwrap_err();
        assert!(
            error.contains("Session off also failed")
                && error.contains("PODxt release also failed")
        );
        session.link.assert_finished();
    }
}

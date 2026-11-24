use super::*;
use crate::gate::{AllClosed, GatePolicy};
use crate::session::Quiet;
use crate::session::testing::AllowAll;
use std::{collections::VecDeque, path::Path as FilePath};

const ACK: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x71, 0xf7];
const DSP_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x4c, 0, 0xf7];
const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];

#[derive(Default)]
struct Recording {
    writes: Vec<Vec<u8>>,
    transcript: Vec<String>,
    reply_read_at: Vec<usize>,
    elapsed: Duration,
    reply: Option<Vec<u8>>,
    difference_replies: VecDeque<Vec<Vec<u8>>>,
    difference_in_flight: bool,
    notices: VecDeque<(Duration, Vec<u8>)>,
    store_notices: VecDeque<(Duration, Vec<u8>)>,
    fail_at: Vec<usize>,
}

impl Link for Recording {
    fn path(&self) -> Path {
        Path::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.first() == Some(&0x3b) {
            self.difference_in_flight = true;
        }
        if bytes == [0xf7] && self.difference_in_flight {
            self.difference_in_flight = false;
            self.notices.extend(
                self.difference_replies
                    .pop_front()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|packet| (Duration::ZERO, packet)),
            );
        }
        if bytes.len() == 2 && bytes[0] == 0x61 {
            self.notices.append(&mut self.store_notices);
        }
        if matches!(bytes, [0x61, _] | [0x6d])
            && let Some(reply) = self.reply.take()
        {
            self.notices.push_back((Duration::ZERO, reply));
        }
        self.writes.push(bytes.to_vec());
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        self.transcript.push(format!("out {hex}"));
        if self.fail_at.contains(&(self.writes.len() - 1)) {
            return Err("simulated write failure".into());
        }
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        if let Some((delay, notice)) = self.notices.pop_front() {
            if delay > timeout {
                self.notices.push_front((delay - timeout, notice));
                self.elapsed += timeout;
                return Ok(None);
            }
            self.reply_read_at.push(self.writes.len());
            self.elapsed += delay;
            return Ok(Some(notice));
        }
        self.elapsed += timeout;
        Ok(None)
    }

    fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn pause(&mut self, duration: Duration) {
        self.transcript
            .push(format!("pause {}", duration.as_millis()));
        self.elapsed += duration;
    }
}

fn packed(name: &str) -> [u8; WORDS * 3] {
    let mut bytes = [0; WORDS * 3];
    bytes[NAME_OFFSET..NAME_OFFSET + NAME_SIZE].copy_from_slice(&name_field(name, b' ').unwrap());
    let mut sum = 0;
    for word in bytes[..1089 * 3].as_chunks::<3>().0 {
        sum += u32::from_be_bytes([0, word[0], word[1], word[2]]);
    }
    bytes[3273..].copy_from_slice(&(sum & 0xff_ffff).to_be_bytes()[1..]);
    bytes
}

fn assert_cleanup(writes: &[Vec<u8>]) {
    let tail = &writes[writes.len() - 7..];
    assert_eq!(tail[..3].concat(), DSP_ON);
    assert_eq!(tail[3..6].concat(), SESSION_OFF);
    assert_eq!(tail[6], BRIDGE_OFF);
}

#[test]
fn store_confirmation_success_timeout_and_unexpected_reply() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let location = Preset::new(8, 2).unwrap();
    for (reply, expected, elapsed_ms) in [
        (Some(ACK.to_vec()), None, 200),
        (None, Some("Timed out"), 15200),
        (Some(sysex(0x70, [])), Some("Unexpected"), 200),
        (
            Some(vec![0xf0, 0, 1, 0x0d, 7, 0, 0x71, 0xf7]),
            Some("Unexpected"),
            200,
        ),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                reply,
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let result = store(&mut session, location, &program, || false);
        match expected {
            Some(expected) => assert!(result.unwrap_err().contains(expected)),
            None => result.unwrap(),
        }
        let writes = &session.link.writes;
        assert_eq!(
            writes[7..33].iter().map(Vec::len).collect::<Vec<_>>(),
            [vec![256; 25], vec![166]].concat()
        );
        assert_eq!(
            writes[33..36].concat(),
            [0xf0, 0, 1, 0x0c, 7, 0, 0x61, 42, 0xf7]
        );
        assert_eq!(session.link.elapsed.as_millis(), elapsed_ms + 2);
        assert_cleanup(writes);
    }
}

#[test]
fn store_checked_identifies_silent_timeout_only_after_cleanup_sends_succeed() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let preset = Preset::new(10, 4).unwrap();
    for fail_at in [vec![], vec![36]] {
        let mut log = Quiet;
        let fail_cleanup = !fail_at.is_empty();
        let mut session = Session::new(
            Recording {
                fail_at,
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let error = store_checked(&mut session, preset, &program, || false).unwrap_err();
        if fail_cleanup {
            assert!(matches!(error, StoreError::Other(_)));
            assert!(error.to_string().contains("Timed out"));
            assert!(error.to_string().contains("cleanup failed"));
            assert_eq!(session.link.writes.last().unwrap(), BRIDGE_OFF);
        } else {
            assert_eq!(error, StoreError::NoConfirmation);
            assert_cleanup(&session.link.writes);
        }
    }
}

#[derive(Default)]
struct Notes(Vec<String>);

impl crate::session::Log for Notes {
    fn line(&mut self, text: String) {
        self.0.push(text);
    }
}

#[test]
fn each_store_notice_is_logged_and_skipped_even_with_strict_replies() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    for (command, label) in [
        (0x28, "clip"),
        (0x4d, "DSP overload"),
        (0x74, "MIDI queue overflow"),
    ] {
        let mut log = Notes::default();
        let mut notice = sysex(command, []);
        notice[5] = 4;
        let mut session = Session::new(
            Recording {
                store_notices: [(Duration::ZERO, notice)].into(),
                reply: Some(ACK.to_vec()),
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        session.require_expected_sysex();
        store(&mut session, Preset::new(10, 4).unwrap(), &program, || {
            false
        })
        .unwrap();
        assert_eq!(
            session
                .link
                .writes
                .iter()
                .filter(|chunk| chunk.as_slice() == [0x61, 50])
                .count(),
            1
        );
        assert_cleanup(&session.link.writes);
        assert!(
            log.0
                .iter()
                .any(|line| line.contains(&format!("Skipped Variax {label}")))
        );
    }
}

#[test]
fn store_notices_do_not_restart_the_fifteen_second_deadline() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let mut log = Quiet;
    let mut session = Session::new(
        Recording {
            store_notices: (0..149).map(|_| (POLL, sysex(0x28, []))).collect(),
            ..Default::default()
        },
        &AllowAll,
        &mut log,
    );
    assert!(
        store(&mut session, Preset::new(10, 4).unwrap(), &program, || {
            false
        })
        .unwrap_err()
        .starts_with("Timed out")
    );
    assert_eq!(session.link.elapsed, Duration::from_millis(15202));
    assert_cleanup(&session.link.writes);
}

#[test]
fn store_rejects_other_messages_even_when_confirmation_shares_the_packet() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let mut foreign_notice = sysex(0x28, []);
    foreign_notice[3] = 0x0d;
    for unexpected in [
        vec![0xb0, 79, 0],
        vec![0xc0, 1],
        vec![0xf8],
        vec![0xf6],
        foreign_notice,
        sysex(0x71, [0]),
        vec![0xf7],
    ] {
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                reply: Some([ACK, &unexpected].concat()),
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        assert_eq!(
            store(&mut session, Preset::new(10, 4).unwrap(), &program, || {
                false
            })
            .unwrap_err(),
            "Unexpected Variax store reply. Slot state is unknown."
        );
        assert_cleanup(&session.link.writes);
    }
}

#[test]
fn stop_mid_transfer_terminates_sysex_without_storing() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
    let mut checks = 0;
    let error = store(&mut session, Preset::new(0, 0).unwrap(), &program, || {
        checks += 1;
        checks == 3
    })
    .unwrap_err();
    assert!(error.contains("stopped"));
    let writes = &session.link.writes;
    assert_eq!(writes.len(), 16);
    assert_eq!(writes[7].len(), 256);
    assert_eq!(writes[8], [0xf7]);
    assert_cleanup(writes);
}

#[test]
fn partial_write_and_failed_cleanup_attempt_every_remaining_command() {
    let bytes = packed("A model");
    let program = Program::new("A model", &bytes).unwrap();
    let mut log = Quiet;
    let link = Recording {
        fail_at: vec![8, 10],
        ..Default::default()
    };
    let mut session = Session::new(link, &AllowAll, &mut log);
    let error = store(&mut session, Preset::new(0, 0).unwrap(), &program, || false).unwrap_err();
    assert!(error.contains("simulated write failure"));
    assert!(error.contains("DspEnabled(true)"));
    let writes = &session.link.writes;
    assert_eq!(writes.len(), 16);
    assert_eq!(writes[9], [0xf7]);
    assert_eq!(writes[10], DSP_ON[..6]);
    assert_eq!(writes[11], [0xf7]);
    assert_eq!(writes[12..15].concat(), SESSION_OFF);
    assert_eq!(writes[15], BRIDGE_OFF);
}

#[test]
fn invalid_names_and_closed_or_incorrect_gates_send_nothing() {
    struct G6Only;
    impl GatePolicy for G6Only {
        fn allows(&self, op: Operation) -> bool {
            op == G6
        }
    }
    struct G7Only;
    impl GatePolicy for G7Only {
        fn allows(&self, op: Operation) -> bool {
            op == G7
        }
    }
    let name = "A model";
    let bytes = packed(name);
    let program = Program::new(name, &bytes).unwrap();
    assert!(Program::new("1234567890123456789", &bytes).is_err());
    assert!(Program::new("Español", &bytes).is_err());
    assert!(Program::new("different", &bytes).is_err());
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllClosed, &mut log);
    assert!(edit(&mut session, &[Edit::Full(&program)], || false).is_err());
    assert!(session.link.writes.is_empty());
    let mut session = Session::new(Recording::default(), &G6Only, &mut log);
    assert!(store(&mut session, Preset::new(0, 0).unwrap(), &program, || false).is_err());
    assert!(rename_bank(&mut session, 0, "Custom").is_err());
    assert!(session.link.writes.is_empty());
    let mut session = Session::new(Recording::default(), &G7Only, &mut log);
    let error = rename_bank(&mut session, 0, "Custom").unwrap_err();
    assert!(error.contains("VariaxBankNameRead"));
    assert!(session.link.writes.is_empty());
}

#[test]
fn bank_name_reply_validation_and_timeout_keep_cleanup_order() {
    fn bank_reply(name: &str) -> Vec<u8> {
        sysex(0x6d, name_field(name, 0).unwrap())
    }

    for (reply, expected_error, elapsed) in [
        (Some(bank_reply("New bank")), None, 1000),
        (Some(bank_reply("Old bank")), Some("differs"), 1000),
        (Some(sysex(0x6d, [0; 17])), Some("Timed out"), 2000),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                reply,
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let result = rename_bank(&mut session, 1, "New bank");
        match expected_error {
            Some(error) => assert!(result.unwrap_err().contains(error)),
            None => assert_eq!(result.unwrap(), "New bank"),
        }
        assert_eq!(session.sent[3], [0xc0, 64]);
        assert_eq!(session.sent[5], sysex(0x6d, []));
        assert_eq!(session.link.elapsed, Duration::from_millis(elapsed));
        assert_cleanup(&session.link.writes);
    }
}

#[test]
fn ordinary_differences_keep_original_bytes() {
    let old = packed("");
    let mut new = old;
    set_word(&mut new, 0, 0x123456);
    set_word(&mut new, 10, 0xab);
    set_word(&mut new, 11, 0xcd);
    set_word(
        &mut new,
        CHECKSUM_WORD,
        Program(&old).word(CHECKSUM_WORD) + 0x1235ce,
    );
    let previous = Program::new("", &old).unwrap();
    let next = Program::new("", &new).unwrap();
    assert_eq!(
        difference(&previous, &next).unwrap(),
        [
            0xf0, 0, 1, 0x0c, 7, 0, 0x3b, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 5, 4, 3, 2, 1, 2,
            0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 11, 10, 0, 0, 0, 0, 13, 12, 0, 0, 0, 0, 0xf7,
        ]
    );
    assert!(difference(&previous, &previous).is_none());
}

fn set_word(bytes: &mut [u8; WORDS * 3], index: usize, value: u64) {
    bytes[index * 3..][..3].copy_from_slice(&value.to_be_bytes()[5..]);
}

#[test]
fn tone_cc_uses_original_truncation() {
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
    edit(&mut session, &[Edit::Tone(0.5)], || false).unwrap();
    assert_eq!(session.link.writes[4], [0xb0, 79, 63]);
    assert_cleanup(&session.link.writes);
}

#[test]
fn program_requires_both_checksums_before_a_write() {
    let mut bytes = packed("");
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
    for index in [1089, 1090] {
        set_word(&mut bytes, index, 1);
        let result = Program::new("", &bytes)
            .and_then(|program| edit(&mut session, &[Edit::Full(&program)], || false));
        assert!(result.unwrap_err().contains("firmware checksum"));
        set_word(&mut bytes, index, 0);
    }
    set_word(&mut bytes, 1089, 1);
    set_word(&mut bytes, 1090, WORD_MASK);
    assert!(
        Program::new("", &bytes).is_ok(),
        "the two words cancel modulo 24 bits"
    );
    set_word(&mut bytes, 1089, 2);
    set_word(
        &mut bytes,
        CHECKSUM_WORD,
        Program(&packed("")).word(CHECKSUM_WORD) + 1,
    );
    assert!(
        Program::new("", &bytes)
            .err()
            .unwrap()
            .contains("host checksum")
    );
    assert!(session.link.writes.is_empty());
}

#[test]
fn full_is_the_unknown_baseline_path_for_an_edit_or_a_tuning_toggle() {
    let bytes = packed("Final");
    let program = Program::new("Final", &bytes).unwrap();
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
    edit(&mut session, &[Edit::Full(&program)], || false).unwrap();
    assert_eq!(session.sent[2], full(&program));
    assert_eq!(session.sent.len(), 6);
    assert!(session.link.reply_read_at.is_empty());
    assert_cleanup(&session.link.writes);
}

#[test]
fn every_difference_reply_is_read_before_the_next_write_and_store_gets_its_own_reply() {
    let old = packed("Old");
    let new = packed("New");
    let previous = Program::new("Old", &old).unwrap();
    let next = Program::new("New", &new).unwrap();
    let mut log = Quiet;
    let mut session = Session::new(
        Recording {
            difference_replies: [
                vec![sysex(0x28, []), ACK[..4].to_vec(), ACK[4..].to_vec()],
                vec![ACK.to_vec()],
            ]
            .into(),
            reply: Some(ACK.to_vec()),
            ..Default::default()
        },
        &AllowAll,
        &mut log,
    );
    edit(
        &mut session,
        &[
            Edit::Difference {
                previous: &previous,
                next: &previous,
            },
            Edit::Difference {
                previous: &previous,
                next: &next,
            },
            Edit::AlternateTuning {
                previous: &next,
                transition: &previous,
                full: &next,
            },
        ],
        || false,
    )
    .unwrap();
    assert_eq!(session.link.reply_read_at, [7, 7, 7, 10]);
    assert_eq!(session.sent[2], difference(&previous, &next).unwrap());
    assert_eq!(session.sent[3], difference(&next, &previous).unwrap());
    assert_eq!(session.sent[4], full(&next));
    store(&mut session, Preset::new(0, 0).unwrap(), &next, || false).unwrap();
    assert_eq!(session.link.reply_read_at, [7, 7, 7, 10, 79]);
}

#[test]
fn missing_or_wrong_difference_reply_stops_before_the_tuning_upload() {
    let old = packed("Old");
    let new = packed("New");
    let previous = Program::new("Old", &old).unwrap();
    let next = Program::new("New", &new).unwrap();
    for (packets, expected) in [
        (vec![], "Timed out"),
        (vec![sysex(0x70, []), ACK.to_vec()], "Unexpected"),
    ] {
        let packet_count = packets.len();
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                difference_replies: [packets].into(),
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let error = edit(
            &mut session,
            &[Edit::AlternateTuning {
                previous: &previous,
                transition: &next,
                full: &next,
            }],
            || false,
        )
        .unwrap_err();
        assert!(error.contains(expected));
        assert_eq!(session.link.reply_read_at, vec![7; packet_count]);
        assert!(session.sent.iter().all(|message| message[6] != 0x43));
        assert_cleanup(&session.link.writes);
    }
}

#[test]
fn interrupted_difference_consumes_any_reply_to_recovery_eox() {
    let old = packed("Old");
    let new = packed("New");
    let previous = Program::new("Old", &old).unwrap();
    let next = Program::new("New", &new).unwrap();
    for (fail_at, expected) in [(vec![], "stopped"), (vec![5], "write failure")] {
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                difference_replies: [vec![ACK.to_vec()]].into(),
                fail_at,
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let mut checks = 0;
        let error = edit(
            &mut session,
            &[Edit::Difference {
                previous: &previous,
                next: &next,
            }],
            || {
                checks += 1;
                checks >= 4
            },
        )
        .unwrap_err();
        assert!(error.contains(expected));
        assert_eq!(session.link.reply_read_at, [7]);
        assert_cleanup(&session.link.writes);
    }
}

#[test]
fn store_refuses_a_queued_or_delayed_unread_reply_on_an_edit_link() {
    let bytes = packed("");
    let program = Program::new("", &bytes).unwrap();
    // A split reply still counts as unread, even before the assembler emits an event.
    for (delay, packet) in [(0, ACK), (0, &ACK[..4]), (2, ACK)] {
        let mut log = Quiet;
        let mut session = Session::new(
            Recording {
                notices: [(Duration::from_millis(delay), packet.to_vec())].into(),
                ..Default::default()
            },
            &AllowAll,
            &mut log,
        );
        let error =
            store(&mut session, Preset::new(0, 0).unwrap(), &program, || false).unwrap_err();
        assert!(error.contains("Unread MIDI data"));
        assert!(session.sent.iter().all(|message| message[6] != 0x61));
        if delay == 0 {
            assert!(session.link.writes.is_empty());
        } else {
            assert_cleanup(&session.link.writes);
        }
    }
}

// Bounded edit-buffer projection of P:0x14433 and P:0x14668 in #181.
// Read a whole six-nibble word even if F7 appears inside it. Only the
// command loop recognizes F7. Stop when the transcript runs out or a write
// would leave the modeled buffer.
fn replay_buffer(message: &[u8], buffer: &mut [u64; WORDS]) -> Option<u8> {
    fn read_word(bytes: &mut &[u8]) -> Option<usize> {
        let (word, rest) = bytes.split_first_chunk::<6>()?;
        *bytes = rest;
        Some(
            word.iter()
                .enumerate()
                .map(|(i, nibble)| usize::from(nibble & 15) << (i * 4))
                .sum(),
        )
    }
    let mut bytes = &message[7..];
    if message[6] == 0x43 {
        let count = read_word(&mut bytes)?;
        for index in 0..count {
            *buffer.get_mut(index)? = read_word(&mut bytes)? as u64;
        }
        assert_eq!(bytes, [0xf7]);
        assert_eq!(buffer[..1091].iter().sum::<u64>() & 0xff_ffff, buffer[1091]);
        return None; // A valid full upload reloads without 0x71.
    }
    assert_eq!(message[6], 0x3b);
    loop {
        if bytes.first() == Some(&0xf7) {
            return Some(0x71);
        }
        let count = read_word(&mut bytes)? & 0xffff;
        let start = read_word(&mut bytes)?;
        for index in start..start + count {
            *buffer.get_mut(index)? = read_word(&mut bytes)? as u64;
        }
    }
}

fn legacy_run(start: u64, count: u64, data: impl IntoIterator<Item = u64>) -> Vec<u8> {
    let mut payload = Vec::new();
    for word in [start, count].into_iter().chain(data) {
        push_word(&mut payload, word);
    }
    sysex(0x3b, payload)
}

#[test]
fn receiver_replay_terminal_runs_follow_firmware_count_start_order() {
    let old = packed("");
    let mut new = old;
    // The cancelling pair passes both sums and reaches the terminal word.
    set_word(&mut new, 1089, 3);
    set_word(&mut new, 1090, 0xfffffd);
    let previous = Program::new("", &old).unwrap();
    let next = Program::new("", &new).unwrap();

    let mut old_buffer = std::array::from_fn(|index| previous.word(index));
    let old_frame = legacy_run(1089, 2, [3, 0xfffffd]);
    assert_eq!(replay_buffer(&old_frame, &mut old_buffer), None);
    assert_eq!(old_buffer[2..4], [3, 0xfffffd]);
    assert_eq!(old_buffer[1089..1091], [0, 0]);

    let mut single_word = std::array::from_fn(|index| previous.word(index));
    assert_eq!(
        replay_buffer(&legacy_run(1090, 1, [0x789abc]), &mut single_word),
        None
    );
    assert_eq!(single_word[1], 0x789abc);
    assert_eq!(single_word[1090], 0);

    let mut log = Quiet;
    let mut session = Session::new(
        Recording {
            difference_replies: [vec![ACK.to_vec()]].into(),
            ..Default::default()
        },
        &AllowAll,
        &mut log,
    );
    edit(
        &mut session,
        &[Edit::Difference {
            previous: &previous,
            next: &next,
        }],
        || false,
    )
    .unwrap();
    let mut new_buffer = std::array::from_fn(|index| previous.word(index));
    assert_eq!(replay_buffer(&session.sent[2], &mut new_buffer), Some(0x71));
    assert_eq!(new_buffer, std::array::from_fn(|index| next.word(index)));
    assert_eq!(session.link.reply_read_at, [7]);
}

#[test]
fn receiver_replay_unknown_baseline_requires_a_full_upload() {
    let mut bytes = packed("");
    set_word(&mut bytes, 0, 1);
    set_word(&mut bytes, 1, 4);
    set_word(&mut bytes, 2, 17);
    set_word(
        &mut bytes,
        CHECKSUM_WORD,
        Program(&packed("")).word(CHECKSUM_WORD) + 22,
    );
    let next = Program::new("", &bytes).unwrap();
    let mut buffer = [0; WORDS];
    let legacy = legacy_run(0, 1091, (0..1091).map(|index| next.word(index)));
    let _ = replay_buffer(&legacy, &mut buffer);
    assert_eq!(buffer[0], 0);
    assert_eq!(buffer[4], 17, "legacy data is parsed as a second run");
    assert_eq!(replay_buffer(&full(&next), &mut buffer), None);
    assert_eq!(buffer, std::array::from_fn(|index| next.word(index)));
}

/// The private differential corpus is supplied by verify_variax_write.py.
#[test]
#[ignore = "requires the local original Workbench classes and factory corpus"]
fn original_transcripts() {
    let root = std::env::var_os("VARIAX_WRITE_REFERENCE").expect("run verify_variax_write.py");
    let root = FilePath::new(&root);
    let names = std::fs::read_to_string(root.join("names.txt")).unwrap();
    let names: Vec<_> = names.lines().collect();
    assert_eq!(names.len(), 60);
    let programs: Vec<[u8; WORDS * 3]> = (0..names.len())
        .map(|index| {
            std::fs::read(root.join(format!("rust-{index}-normal.bin")))
                .unwrap()
                .try_into()
                .unwrap()
        })
        .collect();
    let programs: Vec<_> = names
        .iter()
        .zip(&programs)
        .map(|(name, bytes)| Program::new(name, bytes).unwrap())
        .collect();
    let transitions: Vec<[u8; WORDS * 3]> = (0..names.len())
        .map(|index| {
            std::fs::read(root.join(format!("rust-{index}-muted.bin")))
                .unwrap()
                .try_into()
                .unwrap()
        })
        .collect();
    for (index, program) in programs.iter().enumerate() {
        let mut log = Quiet;
        let link = Recording {
            difference_replies: [vec![ACK.to_vec()], vec![ACK.to_vec()]].into(),
            ..Default::default()
        };
        let mut session = Session::new(link, &AllowAll, &mut log);
        let previous = &programs[(index + programs.len() - 1) % programs.len()];
        edit(
            &mut session,
            &[
                Edit::Full(program),
                Edit::Difference {
                    previous: program,
                    next: program,
                },
                Edit::Difference {
                    previous: program,
                    next: previous,
                },
                Edit::AlternateTuning {
                    previous,
                    transition: &Program::new(names[index], &transitions[index]).unwrap(),
                    full: program,
                },
                Edit::Tone(0.5),
            ],
            || false,
        )
        .unwrap();
        assert_reference(root, &session, index, "edit");

        let location = Preset::new((index / 5) as u8, (index % 5) as u8).unwrap();
        let link = Recording {
            reply: Some(ACK.to_vec()),
            ..Default::default()
        };
        let mut session = Session::new(link, &AllowAll, &mut log);
        store(&mut session, location, program, || false).unwrap();
        assert_reference(root, &session, index, "store");

        let bank_name = "Custom bank";
        let link = Recording {
            reply: Some(sysex(0x6d, name_field(bank_name, 0).unwrap())),
            ..Default::default()
        };
        let mut session = Session::new(link, &AllowAll, &mut log);
        rename_bank(&mut session, location.bank(), bank_name).unwrap();
        assert_reference(root, &session, index, "bank");
    }
}

fn assert_reference(root: &FilePath, session: &Session<'_, Recording>, index: usize, kind: &str) {
    let reference =
        std::fs::read_to_string(root.join(format!("reference-{index}-{kind}.txt"))).unwrap();
    let actual = &session.link.transcript;
    assert_eq!(
        actual.len(),
        reference.lines().count(),
        "slot {index}, {kind} event count"
    );
    for (event, (actual, expected)) in actual.iter().zip(reference.lines()).enumerate() {
        assert!(
            actual == expected,
            "slot {index}, {kind} event {event} differs"
        );
    }
}

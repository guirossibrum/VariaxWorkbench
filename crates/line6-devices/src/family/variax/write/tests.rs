use super::*;
use crate::gate::{AllClosed, GatePolicy};
use crate::session::Quiet;
use crate::session::testing::AllowAll;
use std::path::Path as FilePath;

const ACK: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x71, 0xf7];
const DSP_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x4c, 0, 0xf7];
const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];

#[derive(Default)]
struct Recording {
    writes: Vec<Vec<u8>>,
    transcript: Vec<String>,
    elapsed: Duration,
    reply: Option<Vec<u8>>,
    fail_at: Vec<usize>,
}

impl Link for Recording {
    fn path(&self) -> Path {
        Path::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writes.push(bytes.to_vec());
        let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        self.transcript.push(format!("out {hex}"));
        if self.fail_at.contains(&(self.writes.len() - 1)) {
            return Err("simulated write failure".into());
        }
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        if let Some(reply) = self.reply.take() {
            return Ok(Some(reply));
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
        assert_eq!(session.link.elapsed.as_millis(), elapsed_ms);
        assert_cleanup(writes);
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
fn explicit_original_diff_encoding_and_tone_cc() {
    let old = packed("");
    let mut new = old;
    new[..3].copy_from_slice(&[0x12, 0x34, 0x56]);
    new[3270..3273].copy_from_slice(&[0x78, 0x9a, 0xbc]);
    let checksum =
        (u32::from_be_bytes([0, old[3273], old[3274], old[3275]]) + 0x123456) & 0xff_ffff;
    new[3273..].copy_from_slice(&checksum.to_be_bytes()[1..]);
    let previous = Program::new("", &old).unwrap();
    let next = Program::new("", &new).unwrap();
    assert_eq!(
        difference(Some(&previous), &next).unwrap(),
        [
            0xf0, 0, 1, 0x0c, 7, 0, 0x3b, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 5, 4, 3, 2, 1, 2,
            4, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 12, 11, 10, 9, 8, 7, 0xf7
        ]
    );
    assert!(difference(Some(&previous), &previous).is_none());
    let mut log = Quiet;
    let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
    edit(&mut session, &[Edit::Tone(0.5)], || false).unwrap();
    assert_eq!(session.link.writes[4], [0xb0, 79, 63]);
    assert_cleanup(&session.link.writes);
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
        let mut session = Session::new(Recording::default(), &AllowAll, &mut log);
        let previous = &programs[(index + programs.len() - 1) % programs.len()];
        edit(
            &mut session,
            &[
                Edit::Full(program),
                Edit::Difference {
                    previous: Some(program),
                    next: program,
                },
                Edit::Difference {
                    previous: Some(program),
                    next: previous,
                },
                Edit::AlternateTuning {
                    previous: Some(previous),
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

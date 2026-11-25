use super::*;
use crate::procedure::{expected_stored_image, set_firmware_checksum};
use line6_devices::{
    family::variax,
    identity::REQUEST,
    session::{Path as DevicePath, Quiet},
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, time::Duration};

const IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0xc, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
];
const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 0, 0xf7];
const HEADER: &[u8] = &[0xf0, 0, 1, 0xc, 7, 0];
const ACK: &[u8] = &[0xf0, 0, 1, 0xc, 7, 4, 0x71, 0xf7];

fn guitar(payload: &[u8]) -> Vec<u8> {
    [HEADER, payload, &[0xf7]].concat()
}

fn nibbles(bytes: &[u8]) -> Vec<u8> {
    bytes
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[a, b, c]| [c & 15, c >> 4, b & 15, b >> 4, a & 15, a >> 4])
        .collect()
}

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

fn send(script: &mut Script, bytes: &[u8]) {
    script.push((bytes.into(), vec![]));
}

fn reply(script: &mut Script, bytes: &[u8], response: &[u8]) {
    script.push((
        bytes.into(),
        response.chunks(16).map(<[u8]>::to_vec).collect(),
    ));
}

fn read_chunks(script: &mut Script, program_number: Option<u8>, image: &Image) {
    send(script, BRIDGE_ON);
    send(script, &guitar(&[0x2a, 1]));
    if let Some(program) = program_number {
        send(script, &[0xc0, program]);
    }
    for start in (0..1092usize).step_by(32) {
        let count = (1092 - start).min(32);
        let request = guitar(&[
            0x3c,
            (count & 15) as u8,
            (count >> 4) as u8,
            0,
            0,
            0,
            0,
            (start & 15) as u8,
            ((start >> 4) & 15) as u8,
            (start >> 8) as u8,
            0,
            0,
            0,
        ]);
        let response = guitar(
            &[
                &[0x3c][..],
                &nibbles(&image[start * 3..(start + count) * 3]),
            ]
            .concat(),
        );
        reply(script, &request, &response);
    }
    send(script, &guitar(&[0x2a, 0]));
    send(script, BRIDGE_OFF);
}

fn read_settings(script: &mut Script, image: &Image) {
    reply(script, &REQUEST, &IDENTITY);
    send(script, &guitar(&[0x2a, 1]));
    for (index, [a, b, c]) in [[3, 5, 3], [3, 7, 3]].into_iter().enumerate() {
        let start = METADATA.start + index * 96;
        let data = guitar(&[&[0x3c][..], &nibbles(&image[start..start + 96])].concat());
        reply(
            script,
            &guitar(&[0x3c, 0, 2, 0, 0, 0, 0, a, b, c, 0, 0, 0]),
            &data,
        );
    }
    send(script, &guitar(&[0x2a, 0]));
}

fn wrapped(script: &mut Script, payload: &[u8]) {
    send(script, HEADER);
    send(script, payload);
    send(script, &[0xf7]);
}

fn upload(script: &mut Script, prepared: &Prepared, transmit: Option<&[u8]>) {
    send(script, BRIDGE_ON);
    if prepared.mode.listens() {
        send(script, &guitar(&[0x2a, 1]));
    } else {
        wrapped(script, &[0x2a, 1]);
        wrapped(script, &[0x4c, 1]);
    }
    let message = guitar(&[&[0x43, 4, 4, 4, 0, 0, 0][..], &nibbles(&prepared.program)].concat());
    for bytes in message.chunks(256) {
        send(script, bytes);
    }
    if prepared.mode.listens() {
        send(script, &guitar(&[0x2a, 0]));
    } else {
        if let Some(response) = transmit {
            send(script, HEADER);
            send(script, &[0x61, index(prepared.target)]);
            reply(script, &[0xf7], response);
        }
        wrapped(script, &[0x4c, 0]);
        wrapped(script, &[0x2a, 0]);
    }
    send(script, BRIDGE_OFF);
}

fn image(id: u8, name: &str) -> Image {
    let mut image = [0; 3276];
    image[0] = id;
    image[METADATA.start..METADATA.start + 18].fill(b' ');
    image[METADATA.start..METADATA.start + name.len()].copy_from_slice(name.as_bytes());
    set_firmware_checksum(&mut image);
    image
}

fn test_prepared(mode: Mode) -> Prepared {
    let current = (0..60)
        .map(|i| expected_stored_image(&image(i, "Original")))
        .collect::<Vec<_>>();
    let index = if matches!(mode, Mode::RenameReso | Mode::FactoryReso) {
        50
    } else {
        14
    };
    let backed_up = current[index as usize];
    let program = if mode == Mode::Restore {
        backed_up
    } else {
        image(101, "New")
    };
    sources::finish(mode, target(index).unwrap(), current, backed_up, program).unwrap()
}

fn preflight(script: &mut Script, prepared: &Prepared, target_image: &Image, restored: &Image) {
    reply(script, &REQUEST, &IDENTITY);
    read_chunks(script, None, &prepared.current[1]);
    read_chunks(script, Some(prepared.target.program_number()), target_image);
    read_chunks(script, Some(target(1).unwrap().program_number()), restored);
}

#[derive(Default, Clone, Copy)]
enum Cancel {
    #[default]
    Never,
    BeforeStore,
    AfterStore,
}

struct Fake {
    writes: VecDeque<(Vec<u8>, Vec<Vec<u8>>)>,
    inbox: VecDeque<Vec<u8>>,
    time: Duration,
    upload_at: Option<Duration>,
    store_at: Option<Duration>,
    cancel: Cancel,
    stopped: Rc<Cell<bool>>,
    events: Rc<RefCell<Vec<String>>>,
}

impl Fake {
    fn new(script: Script, cancel: Cancel) -> Self {
        Self {
            writes: script.into(),
            inbox: VecDeque::new(),
            time: Duration::ZERO,
            upload_at: None,
            store_at: None,
            cancel,
            stopped: Rc::new(Cell::new(false)),
            events: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn assert_finished(&self) {
        assert!(
            self.writes.is_empty(),
            "Expected more requests: {:?}",
            self.writes.front()
        );
        assert!(self.inbox.is_empty(), "Expected more reads.");
    }
}

impl Link for Fake {
    fn path(&self) -> DevicePath {
        DevicePath::VariaxViaPodxt
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (expected, incoming) = self.writes.pop_front().expect("Unexpected request");
        assert_eq!(bytes, expected);
        match bytes {
            [0xc0, program] => self
                .events
                .borrow_mut()
                .push(format!("Wire: MIDI program {program}")),
            [0xf0, 0, 1, 0xc, 7, 0, 0x43, ..] => {
                self.events.borrow_mut().push("Wire: upload".into())
            }
            [0x61, slot] => self
                .events
                .borrow_mut()
                .push(format!("Wire: save slot {slot}")),
            _ => {}
        }
        if bytes.starts_with(&[0xf0, 0, 1, 0xc, 7, 0, 0x43]) || bytes.len() > 100 {
            self.upload_at = Some(self.time);
        }
        if bytes.first() == Some(&0x61) && bytes.len() == 2 {
            self.store_at = Some(self.time);
            if matches!(self.cancel, Cancel::AfterStore) {
                self.stopped.set(true);
            }
        }
        self.inbox.extend(incoming);
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        let packet = self.inbox.pop_front();
        self.time += if packet.is_some() {
            Duration::from_millis(4)
        } else {
            timeout
        };
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.time
    }

    fn pause(&mut self, duration: Duration) {
        self.time += duration;
        if self.upload_at.is_some()
            && duration == Duration::from_millis(100)
            && matches!(self.cancel, Cancel::BeforeStore)
        {
            self.stopped.set(true);
        }
    }
}

fn run(
    fake: &mut Fake,
    prepared: &Prepared,
    prompt: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    run_with_messages(fake, prepared, prompt, |_| {})
}

fn run_with_messages(
    fake: &mut Fake,
    prepared: &Prepared,
    prompt: impl FnMut(&str) -> Result<(), String>,
    announce: impl FnMut(&str),
) -> Result<(), String> {
    let root = tempfile::tempdir().unwrap();
    let cancel = Rc::clone(&fake.stopped);
    let policy = Policy(prepared.mode);
    let mut quiet = Quiet;
    let mut session = Session::new(&mut *fake, &policy, &mut quiet);
    let result = attempt(
        &mut session,
        prepared,
        root.path(),
        prompt,
        announce,
        || cancel.get(),
    );
    fake.assert_finished();
    result
}

fn narrate_from_reso4(prepared: &Prepared) -> Vec<String> {
    let original = target(51).unwrap();
    let snapshot = &prepared.current[51];
    let mut script = vec![];
    reply(&mut script, &REQUEST, &IDENTITY);
    read_chunks(&mut script, None, snapshot);
    read_chunks(
        &mut script,
        Some(prepared.target.program_number()),
        &prepared.current[usize::from(index(prepared.target))],
    );
    read_chunks(&mut script, Some(original.program_number()), snapshot);
    upload(
        &mut script,
        prepared,
        (!prepared.mode.listens()).then_some(ACK),
    );
    if prepared.mode.listens() {
        read_chunks(&mut script, None, &prepared.program);
    } else {
        read_chunks(
            &mut script,
            Some(prepared.target.program_number()),
            &prepared.expected,
        );
        read_settings(&mut script, &prepared.expected);
    }
    read_chunks(&mut script, Some(original.program_number()), snapshot);
    let mut fake = Fake::new(script, Cancel::Never);
    let events = Rc::clone(&fake.events);
    run_with_messages(
        &mut fake,
        prepared,
        |phrase| {
            events.borrow_mut().push(format!("Prompt: {phrase}"));
            Ok(())
        },
        |text| events.borrow_mut().push(text.into()),
    )
    .unwrap();
    fake.events.take()
}

#[test]
fn owner_messages_precede_their_wire_actions_and_end_with_a_verified_result() {
    for mode in [Mode::Listen, Mode::Store, Mode::Restore] {
        let events = narrate_from_reso4(&test_prepared(mode));
        let mut anchors = vec![
            "HEAR: Your current preset stays selected",
            "SEE: Wait for the tool to name your starting preset.",
            "Starting preset is Reso 4, verified.",
            "HEAR: Switching to Spank 1",
            "SEE: Wait for the Spank 1 check",
            "Wire: MIDI program 69",
            "Spank 1 matches the expected current program.",
            "HEAR: Switching back to Reso 4.",
            "Wire: MIDI program 106",
            "Back on Reso 4, verified.",
            "HEAR: Reso 4 stays selected until you confirm.",
        ];
        if mode.listens() {
            anchors.extend([
                "SEE: Type LISTEN SLOT 14 Spank 1",
                "Prompt: LISTEN SLOT 14 Spank 1",
                "Wire: upload",
                "Edited sound uploaded",
                "HEAR: The edited sound stays loaded",
                "The working buffer matches",
                "HEAR: Expect the edited sound:",
                "SEE: Play now. Play strings only and do not touch the knobs.",
                "Prompt: RETURN TO ORIGINAL",
            ]);
        } else {
            let (instruction, prompt) = if mode == Mode::Restore {
                (
                    "SEE: Type RESTORE SLOT 14 Spank 1",
                    "Prompt: RESTORE SLOT 14 Spank 1",
                )
            } else {
                (
                    "SEE: Type STORE SLOT 14 Spank 1",
                    "Prompt: STORE SLOT 14 Spank 1",
                )
            };
            anchors.extend([
                instruction,
                prompt,
                "Wire: upload",
                "Wire: save slot 14",
                "The guitar confirmed the save to Spank 1.",
                "HEAR: Reloading Spank 1.",
                "Wire: MIDI program 69",
                "Spank 1 matches the expected saved sound and settings.",
            ]);
        }
        anchors.extend([
            "HEAR: Switching back to Reso 4.",
            "Wire: MIDI program 106",
            "Back on Reso 4, verified.",
            if mode.listens() {
                "Done. Sent the edited sound to the working buffer. Nothing was saved. The guitar is on Reso 4."
            } else if mode == Mode::Restore {
                "Done. Sent the backup program and one save command. Saved and verified Spank 1. The guitar is on Reso 4."
            } else {
                "Done. Sent the edited program and one save command. Saved and verified Spank 1. The guitar is on Reso 4."
            },
        ]);
        let mut actual = events.iter();
        for anchor in anchors {
            assert!(
                actual.any(|line| line.starts_with(anchor)),
                "missing {anchor:?} in {events:#?}"
            );
        }
    }
}

#[test]
fn all_five_modes_verify_and_switch_back_with_exact_transcripts() {
    for mode in [
        Mode::Listen,
        Mode::Store,
        Mode::Restore,
        Mode::RenameReso,
        Mode::FactoryReso,
    ] {
        let p = test_prepared(mode);
        let original = &p.current[1];
        let mut script = vec![];
        preflight(
            &mut script,
            &p,
            &p.current[usize::from(index(p.target))],
            original,
        );
        upload(&mut script, &p, (!mode.listens()).then_some(ACK));
        if mode.listens() {
            read_chunks(&mut script, None, &p.program);
        } else {
            read_chunks(&mut script, Some(p.target.program_number()), &p.expected);
            read_settings(&mut script, &p.expected);
        }
        read_chunks(
            &mut script,
            Some(target(1).unwrap().program_number()),
            original,
        );
        let mut fake = Fake::new(
            script,
            if mode.listens() {
                Cancel::Never
            } else {
                Cancel::AfterStore
            },
        );
        let mut prompts = vec![];
        run(&mut fake, &p, |phrase| {
            prompts.push(phrase.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            prompts[0],
            match mode {
                Mode::Listen => "LISTEN SLOT 14 Spank 1",
                Mode::Store => "STORE SLOT 14 Spank 1",
                Mode::Restore => "RESTORE SLOT 14 Spank 1",
                Mode::RenameReso => "3A SLOT 50 Reso 5",
                Mode::FactoryReso => "3B SLOT 50 Reso 5",
            }
        );
        if mode.listens() {
            assert_eq!(prompts[1], "RETURN TO ORIGINAL");
            assert!(fake.store_at.is_none());
        } else {
            assert_eq!(prompts.len(), 1);
            assert_eq!(
                fake.store_at.unwrap() - fake.upload_at.unwrap(),
                Duration::from_millis(100)
            );
        }
    }
}

#[test]
fn refuses_missing_duplicate_and_overwritten_original_without_switching() {
    for (case, message) in [
        ("missing", "matches no"),
        ("duplicate", "matches more than one"),
        ("same", "different starting preset"),
    ] {
        let mut p = test_prepared(Mode::Store);
        if case == "duplicate" {
            p.current[2] = p.current[1];
        }
        let active = match case {
            "missing" => image(99, "Unknown"),
            "same" => p.current[14],
            _ => p.current[1],
        };
        let mut script = vec![];
        reply(&mut script, &REQUEST, &IDENTITY);
        read_chunks(&mut script, None, &active);
        let error = run(&mut Fake::new(script, Cancel::Never), &p, |_| {
            panic!("unexpected prompt")
        })
        .unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn refuses_wrong_target_or_failed_return_before_prompt_and_attempts_switch_back() {
    for wrong_target in [true, false] {
        let p = test_prepared(Mode::Store);
        let wrong = image(99, "Wrong");
        let mut script = vec![];
        let (at_target, at_return) = if wrong_target {
            (&wrong, &p.current[1])
        } else {
            (&p.current[14], &wrong)
        };
        preflight(&mut script, &p, at_target, at_return);
        let error = run(&mut Fake::new(script, Cancel::Never), &p, |_| {
            panic!("unexpected prompt")
        })
        .unwrap_err();
        assert!(error.contains(if wrong_target {
            "Target preflight differs"
        } else {
            "switch-back failed"
        }));
    }
}

#[test]
fn refuses_unconfirmed_write_without_upload_and_switches_back_after_cancelled_upload() {
    let p = test_prepared(Mode::Store);
    let mut script = vec![];
    preflight(&mut script, &p, &p.current[14], &p.current[1]);
    let error = run(&mut Fake::new(script.clone(), Cancel::Never), &p, |_| {
        Err("No confirmation".into())
    })
    .unwrap_err();
    assert!(error.contains("No confirmation"));
    upload(&mut script, &p, None);
    read_chunks(
        &mut script,
        Some(target(1).unwrap().program_number()),
        &p.current[1],
    );
    let error = run(&mut Fake::new(script, Cancel::BeforeStore), &p, |_| Ok(())).unwrap_err();
    assert!(error.contains("before the slot store"));
    assert!(error.contains("reselected and verified"));
}

#[test]
fn silent_timeout_waits_two_more_minutes_with_no_selection_or_read_after_cleanup() {
    let p = test_prepared(Mode::Store);
    let mut script = vec![];
    preflight(&mut script, &p, &p.current[14], &p.current[1]);
    upload(&mut script, &p, Some(&[]));
    let mut fake = Fake::new(script, Cancel::Never);
    let error = run(&mut fake, &p, |_| Ok(())).unwrap_err();
    assert!(error.contains("Do not store any slot in 0–19 until spare 60 is known blank."));
    assert_eq!(fake.time - fake.store_at.unwrap(), Duration::from_secs(135));
}

#[test]
fn wrong_store_reply_is_unknown_and_failed_verification_after_ack_still_switches_back() {
    let p = test_prepared(Mode::Store);
    for ack in [ACK, &[0xf0, 0, 1, 0xc, 7, 4, 0x72, 0xf7][..]] {
        let mut script = vec![];
        preflight(&mut script, &p, &p.current[14], &p.current[1]);
        upload(&mut script, &p, Some(ack));
        if ack == ACK {
            read_chunks(&mut script, Some(p.target.program_number()), &p.current[14]);
            read_chunks(
                &mut script,
                Some(target(1).unwrap().program_number()),
                &p.current[1],
            );
        }
        let error = run(&mut Fake::new(script, Cancel::Never), &p, |_| Ok(())).unwrap_err();
        assert!(error.contains(if ack == ACK {
            "reselected and verified"
        } else {
            "Slot state is unknown"
        }));
    }
}

#[test]
fn diff_uses_workbench_units_and_reports_names() {
    let old = image(1, "Spank 1");
    let mut new = image(1, "Stratocaster");
    let field = (0..192)
        .find(|&offset| variax::PresetMetadata::field_at(offset) == Some("trim"))
        .unwrap();
    new[METADATA.start + field..METADATA.start + field + 3]
        .copy_from_slice(&(-1.25_f32).to_be_bytes()[..3]);
    assert_eq!(
        sources::diff(&old, &new).unwrap(),
        [
            "  Preset name: \"Spank 1\" → \"Stratocaster\"",
            "  trim: 0.00dB → -1.25dB",
        ]
    );
}

#[test]
fn flash_order_and_policy_are_bounded() {
    assert!(target(60).is_err());
    assert_eq!(target(14).unwrap().program_number(), 69);
    assert_eq!(target(50).unwrap().program_number(), 105);
    for i in 0..60 {
        assert_eq!(index(target(i).unwrap()), i);
    }
    assert!(Mode::parse("invalid").is_none());
    assert!(!Policy(Mode::Listen).allows(Operation::VariaxStoreRestore));
    assert!(!Policy(Mode::Store).allows(Operation::VariaxEditBufferWrite));
    assert!(!Policy(Mode::Store).allows(Operation::VariaxBankNameRead));
}

#[test]
#[ignore = "requires local #236 backup, archive, Stratocaster.vwm and model manifest"]
fn private_source_files_build_and_verify_each_ladder_image() {
    let backup = std::env::var_os("VARIAX_SELECTION_BACKUP").unwrap();
    let archive = std::env::var_os("VARIAX_310_ARCHIVE").unwrap();
    let vwm = std::env::var_os("VARIAX_SELECTION_VWM").unwrap();
    let manifest = std::env::var_os("VARIAX_SELECTION_MODELS").unwrap();
    let backup = Path::new(&backup);
    let archive = Path::new(&archive);
    let vwm = Path::new(&vwm);
    let manifest = Path::new(&manifest);
    let spank = backup.join("programs/14-Spank-1.bin");
    assert!(
        prepare(
            Mode::RenameReso,
            14,
            backup,
            archive,
            Path::new("archive"),
            manifest
        )
        .is_err()
    );
    let archive_source = prepare(
        Mode::Listen,
        14,
        backup,
        archive,
        Path::new("archive"),
        manifest,
    )
    .unwrap();
    let backup_source = prepare(Mode::Listen, 14, backup, archive, &spank, manifest).unwrap();
    let other_archive_slot = prepare(
        Mode::Listen,
        14,
        backup,
        archive,
        Path::new("archive:50"),
        manifest,
    )
    .unwrap();
    assert!(crate::procedure::matches_hash(
        &other_archive_slot.program,
        crate::procedure::STORE_HASH
    ));
    assert_ne!(archive_source.program, backup_source.program);
    assert_eq!(
        backup_source.program.as_slice(),
        std::fs::read(spank).unwrap()
    );
    let listen = prepare(Mode::Listen, 14, backup, archive, vwm, manifest).unwrap();
    let store = prepare(Mode::Store, 14, backup, archive, vwm, manifest).unwrap();
    let restore = prepare(Mode::Restore, 14, backup, archive, vwm, manifest).unwrap();
    for prepared in [&listen, &store] {
        let events = narrate_from_reso4(prepared);
        if prepared.mode.listens() {
            assert!(events.iter().any(|line| line.contains(
                "T-Model body, T-Model Bridge and Lester Custom Bridge, alternate tuning off"
            )));
        }
        println!("# {} dry run", prepared.mode.label());
        for line in events.iter().filter(|line| !line.starts_with("Wire: ")) {
            println!("{line}");
        }
    }
    assert_eq!(listen.program, store.program);
    assert_eq!(store.expected, restore.current[14]);
    assert_eq!(restore.expected, backup_source.program);
    assert!(
        store
            .description()
            .iter()
            .any(|line| line.contains("Stratocaster"))
    );
    let rename = prepare(
        Mode::RenameReso,
        50,
        backup,
        archive,
        Path::new("archive"),
        manifest,
    )
    .unwrap();
    let factory = prepare(
        Mode::FactoryReso,
        50,
        backup,
        archive,
        Path::new("archive"),
        manifest,
    )
    .unwrap();
    assert!(crate::procedure::matches_hash(
        &rename.expected,
        "8e640004ff7c8ffeb130055bdb950fed98fc0a62dfb84b0c4f2d45efe98ae3fc"
    ));
    assert_eq!(rename.expected, factory.current[50]);
    assert_eq!(factory.expected, rename.current[50]);
    assert!(crate::procedure::matches_hash(
        &factory.expected,
        crate::procedure::STORED_HASH
    ));
}

#[test]
fn rejects_bad_firmware_checksums_and_truncated_source_programs() {
    let good = image(1, "Spank 1");
    assert_eq!(sources::firmware_image(good.to_vec()).unwrap(), good);
    assert!(
        sources::firmware_image(good[..3275].to_vec())
            .unwrap_err()
            .contains("3,276")
    );
    let mut bad = good.to_vec();
    bad[100] ^= 1;
    assert!(
        sources::firmware_image(bad)
            .unwrap_err()
            .contains("checksum")
    );
}

fn upload_list_entry(script: &mut Script, program: &Image) {
    send(script, BRIDGE_ON);
    send(script, &guitar(&[0x2a, 1]));
    let message = guitar(&[&[0x43, 4, 4, 4, 0, 0, 0][..], &nibbles(program)].concat());
    for bytes in message.chunks(256) {
        send(script, bytes);
    }
    send(script, &guitar(&[0x2a, 0]));
    send(script, BRIDGE_OFF);
}

#[test]
fn listen_list_uploads_in_order_waits_after_each_and_restores_without_a_store() {
    let original = target(51).unwrap();
    let current = test_prepared(Mode::Listen).current;
    let original_image = current[51];
    let mut script = vec![];
    reply(&mut script, &REQUEST, &IDENTITY);
    read_chunks(&mut script, None, &original_image);
    let files = ["A.vwm", "B.vwm", "C.vwm", "D.vwm"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let program = image(100 + i as u8, "Audition");
            upload_list_entry(&mut script, &program);
            read_chunks(&mut script, None, &program);
            super::listen_list::Entry {
                label: name.into(),
                program,
            }
        })
        .collect();
    read_chunks(
        &mut script,
        Some(original.program_number()),
        &original_image,
    );
    let prepared = super::listen_list::Prepared::fixture(current, files);
    let description = prepared.description();
    assert_eq!(description.len(), 5);
    for (i, name) in ["A.vwm", "B.vwm", "C.vwm", "D.vwm"].into_iter().enumerate() {
        let hash = sources::digest(&image(100 + i as u8, "Audition"));
        assert_eq!(
            description[i + 1],
            format!(
                "{:02}. {name} | upload and expected working image SHA-256: {hash}",
                i + 1
            )
        );
    }

    let mut fake = Fake::new(script, Cancel::Never);
    let root = tempfile::tempdir().unwrap();
    let events = Rc::clone(&fake.events);
    let mut quiet = Quiet;
    let mut session = Session::new(&mut fake, &Policy(Mode::Listen), &mut quiet);
    super::listen_list::attempt(
        &mut session,
        &prepared,
        root.path(),
        || {
            events.borrow_mut().push("Enter".into());
            Ok(())
        },
        |line| events.borrow_mut().push(line.to_owned()),
        || false,
    )
    .unwrap();
    fake.assert_finished();
    assert_eq!(fake.store_at, None);
    let events = events.borrow();
    let mut order = events.iter();
    for name in ["A.vwm", "B.vwm", "C.vwm", "D.vwm"] {
        for anchor in [
            format!("HEAR: Loading {name}."),
            "Wire: upload".into(),
            format!("{name} working buffer verified."),
            "SEE: Play now.".into(),
            "Enter".into(),
        ] {
            assert!(order.any(|event| event.starts_with(&anchor)), "{events:#?}");
        }
    }
    assert!(order.any(|event| event == "Wire: MIDI program 106"));
    assert_eq!(events.iter().filter(|event| *event == "Enter").count(), 4);
    assert_eq!(
        events.last().unwrap(),
        "Done. Auditioned 4 files. Nothing was saved. The guitar is on Reso 4."
    );
}

#[test]
fn listen_list_mismatch_or_cancel_stops_the_list_and_restores_the_original() {
    for mismatch in [false, true] {
        let current = test_prepared(Mode::Listen).current;
        let original = current[51];
        let files = vec![
            super::listen_list::Entry {
                label: "First.vwm".into(),
                program: image(71, "First"),
            },
            super::listen_list::Entry {
                label: "Second.vwm".into(),
                program: image(72, "Second"),
            },
        ];
        let mut script = vec![];
        reply(&mut script, &REQUEST, &IDENTITY);
        read_chunks(&mut script, None, &original);
        upload_list_entry(&mut script, &files[0].program);
        let reply = if mismatch {
            &original
        } else {
            &files[0].program
        };
        read_chunks(&mut script, None, reply);
        read_chunks(
            &mut script,
            Some(target(51).unwrap().program_number()),
            &original,
        );
        let prepared = super::listen_list::Prepared::fixture(current, files);
        let mut fake = Fake::new(script, Cancel::Never);
        let root = tempfile::tempdir().unwrap();
        let mut quiet = Quiet;
        let mut session = Session::new(&mut fake, &Policy(Mode::Listen), &mut quiet);
        let mut enters = 0;
        let error = super::listen_list::attempt(
            &mut session,
            &prepared,
            root.path(),
            || {
                enters += 1;
                Err("Cancelled at the Enter prompt.".into())
            },
            |_| {},
            || false,
        )
        .unwrap_err();
        fake.assert_finished();
        assert!(error.contains("Original preset was reselected and verified."));
        assert_eq!(enters, usize::from(!mismatch));
        assert_eq!(fake.store_at, None);
    }
}

#[test]
fn listen_list_refuses_an_empty_list_before_accessing_inputs() {
    let unused = Path::new("unreadable");
    let status = super::listen_list::prepare(unused, unused, unused, &[]);
    assert!(status.err().unwrap().contains("at least one .vwm"));
}

fn difference(script: &mut Script, ack: bool) {
    send(script, BRIDGE_ON);
    wrapped(script, &[0x2a, 1]);
    // A single run containing one word at address 0. The checksum word is
    // deliberately absent, even though the candidate's checksum changed.
    send(script, HEADER);
    send(
        script,
        &[0x3b, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 6],
    );
    if ack {
        reply(script, &[0xf7], ACK);
    } else {
        send(script, &[0xf7]);
    }
    wrapped(script, &[0x4c, 0]);
    wrapped(script, &[0x2a, 0]);
    send(script, BRIDGE_OFF);
}

#[test]
fn diff_listen_checks_every_word_and_returns_only_after_the_acknowledged_edit() {
    let original = expected_stored_image(&image(1, "Original"));
    let mut edited = original;
    edited[0] = 102;
    set_firmware_checksum(&mut edited);
    let mut expected = edited;
    expected[3273..].copy_from_slice(&original[3273..]);

    for (ack, matches) in [(true, true), (true, false), (false, false)] {
        let mut script = Vec::new();
        reply(&mut script, &REQUEST, &IDENTITY);
        read_chunks(&mut script, None, &original);
        difference(&mut script, ack);
        if ack {
            // Reporting a new checksum here would hide a firmware difference.
            read_chunks(&mut script, None, if matches { &expected } else { &edited });
            read_chunks(
                &mut script,
                Some(target(0).unwrap().program_number()),
                &original,
            );
        }
        let mut fake = Fake::new(script, Cancel::Never);
        let mut log = Quiet;
        let policy = Policy(Mode::Listen);
        let root = tempfile::tempdir().unwrap();
        let prepared = diff_listen::Prepared::fixture(target(0).unwrap(), vec![original], edited);
        let mut prompted = false;
        let result = diff_listen::attempt(
            &mut Session::new(&mut fake, &policy, &mut log),
            &prepared,
            root.path(),
            || {
                prompted = true;
                Ok(())
            },
            |_| {},
            || false,
        );
        fake.assert_finished();
        assert_eq!(prompted, matches);
        match (ack, matches) {
            (true, true) => result.unwrap(),
            (true, false) => {
                let error = result.unwrap_err();
                assert!(error.contains("word 1091") && error.contains("reselected and verified"));
            }
            (false, _) => {
                let error = result.unwrap_err();
                assert!(
                    error.contains("Reopen the link") && error.contains("by hand"),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn diff_listen_refuses_another_preset_before_any_edit() {
    let original = expected_stored_image(&image(1, "Original"));
    let other = expected_stored_image(&image(2, "Other"));
    let prepared =
        diff_listen::Prepared::fixture(target(0).unwrap(), vec![original, other], original);
    let mut script = Vec::new();
    reply(&mut script, &REQUEST, &IDENTITY);
    read_chunks(&mut script, None, &other);
    let mut fake = Fake::new(script, Cancel::Never);
    let root = tempfile::tempdir().unwrap();
    let error = diff_listen::attempt(
        &mut Session::new(&mut fake, &Policy(Mode::Listen), &mut Quiet),
        &prepared,
        root.path(),
        || panic!("A different preset must not reach the audition."),
        |_| {},
        || false,
    )
    .unwrap_err();
    assert!(
        error.contains(&format!(
            "Select {} by hand",
            preset_name(target(0).unwrap())
        )),
        "{error}"
    );
    fake.assert_finished();
}

#[test]
fn shared_store_and_reload_checks_gate_first_and_uses_exact_owner_wire_sequence() {
    let prepared = test_prepared(Mode::Store);
    let name = sources::metadata(&prepared.program).unwrap().name;
    let program = write::Program::new(&name, &prepared.program).unwrap();
    let mut script = Vec::new();
    upload(&mut script, &prepared, Some(ACK));
    read_chunks(
        &mut script,
        Some(prepared.target.program_number()),
        &prepared.expected,
    );
    let mut fake = Fake::new(script, Cancel::Never);
    let mut log = Quiet;
    let mut session = Session::new(&mut fake, &crate::procedure::StorePolicy, &mut log);
    let error = procedures::store_and_reload(
        &mut session,
        prepared.target,
        &program,
        || false,
        |_| panic!("Unexpected timeout"),
        |_| Ok(()),
    )
    .unwrap_err();
    assert!(
        matches!(error, VerifiedStoreError::BeforeCommit(message) if message.contains("VariaxPresetSwitch"))
    );

    let mut session = Session::new(&mut fake, &Policy(Mode::Store), &mut log);
    let mut replies = 0;
    procedures::store_and_reload(
        &mut session,
        prepared.target,
        &program,
        || false,
        |_| panic!("Unexpected timeout"),
        |_| {
            replies += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(replies, 35);
    fake.assert_finished();
}

#[test]
#[ignore = "requires local #236 backup, archive and model manifest"]
fn private_inputs_verify_spank_one_trim_difference() {
    let backup = std::env::var_os("VARIAX_SELECTION_BACKUP").unwrap();
    let archive = std::env::var_os("VARIAX_310_ARCHIVE").unwrap();
    let manifest = std::env::var_os("VARIAX_SELECTION_MODELS").unwrap();
    let prepared = diff_listen::prepare(
        14,
        Path::new(&backup),
        Path::new(&archive),
        Path::new(&manifest),
    )
    .unwrap();
    let description = prepared.description().unwrap();
    assert_eq!(description.len(), 4);
    assert_eq!(description[1], "  trim: 0.00dB → -6.00dB");
    assert_eq!(
        description[3],
        "Expected working image SHA-256: b43a2b47105f31d0b49c9fadaae7465159b117524245703aa3b6ac6f13f788de"
    );
}

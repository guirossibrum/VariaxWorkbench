use super::*;
use line6_devices::session::{Path as DevicePath, Quiet};
use std::{cell::Cell, collections::VecDeque, rc::Rc};

const VARIAX_IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0xc, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
];
const PODXT_IDENTITY: [u8; 17] = [
    0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 0xc, 3, 0, 0xa, 0, 0, 3, 0, 1, 0xf7,
];
const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0xc, 3, 0x23, 0, 0xf7];
const HEADER: &[u8] = &[0xf0, 0, 1, 0xc, 7, 0];

struct CancelOnWrite {
    chunks: Vec<Vec<u8>>,
    cancelled: Rc<Cell<bool>>,
}

struct Fake {
    path: DevicePath,
    clock: Duration,
    pauses: Vec<Duration>,
    inbox: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    write_times: Vec<Duration>,
    chunks: Cell<usize>,
    fail_at: Option<usize>,
    cancel_on_write: Option<CancelOnWrite>,
}

impl Fake {
    fn variax(prepared: &Prepared) -> Self {
        Self::new(
            DevicePath::VariaxViaPodxt,
            current_replies(&prepared.start)
                .into_iter()
                .chain(current_replies(&prepared.end))
                .collect(),
        )
    }

    fn new(path: DevicePath, inbox: VecDeque<Vec<u8>>) -> Self {
        Self {
            path,
            clock: Duration::ZERO,
            pauses: vec![],
            inbox,
            writes: vec![],
            write_times: vec![],
            chunks: Cell::new(0),
            fail_at: None,
            cancel_on_write: None,
        }
    }
}

impl Link for Fake {
    fn path(&self) -> DevicePath {
        self.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let index = self.writes.len();
        self.writes.push(bytes.into());
        self.write_times.push(self.clock);
        if let Some(trigger) = &self.cancel_on_write
            && self.writes.ends_with(&trigger.chunks)
        {
            trigger.cancelled.set(true);
        }
        if bytes.len() == 256 {
            self.chunks.set(self.chunks.get() + 1);
        }
        if self.fail_at == Some(index) {
            Err("synthetic transport write failure".into())
        } else {
            Ok(())
        }
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.clock += wait;
        Ok(self.inbox.pop_front().filter(|packet| !packet.is_empty()))
    }

    fn elapsed(&self) -> Duration {
        self.clock
    }

    fn pause(&mut self, duration: Duration) {
        self.pauses.push(duration);
        self.clock += duration;
    }
}

fn guitar(command: &[u8]) -> Vec<u8> {
    [HEADER, command, &[0xf7]].concat()
}

fn sysex_words(bytes: &[u8]) -> Vec<u8> {
    bytes
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[a, b, c]| [c & 15, c >> 4, b & 15, b >> 4, a & 15, a >> 4])
        .collect()
}

fn reply(raw: &[u8]) -> Vec<u8> {
    guitar(&[&[0x3c], sysex_words(raw).as_slice()].concat())
}

fn current_replies(raw: &[u8; 192]) -> [Vec<u8>; 3] {
    [VARIAX_IDENTITY.into(), reply(&raw[..96]), reply(&raw[96..])]
}

fn checksum(program: &mut [u8; PROGRAM_BYTES]) {
    let sum = program[..3267]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[a, b, c]| u32::from_be_bytes([0, a, b, c]))
        .sum::<u32>();
    program[3273..].copy_from_slice(&sum.to_be_bytes()[1..]);
}

fn settings() -> [u8; 192] {
    let mut raw = [0; 192];
    raw[..18].fill(b' ');
    raw[..NAME.len()].copy_from_slice(NAME.as_bytes());
    raw
}

fn prepared(stage: Stage) -> Prepared {
    // Invented program and metadata. The independent oracle pins actual Reso-5.
    let start = settings();
    let mut end = start;
    if matches!(stage, Stage::Trim) {
        end[65..68].copy_from_slice(&[0xc0, 0xc0, 0]);
    }
    let mut program = [0; PROGRAM_BYTES];
    program[METADATA].copy_from_slice(&end);
    checksum(&mut program);
    Prepared {
        start,
        end,
        program,
    }
}

fn exercise(fake: &mut Fake, prepared: &Prepared, stop_after_chunk: bool) -> Result<(), String> {
    let mut quiet = Quiet;
    let mut session = Session::new(fake, &VariaxPolicy, &mut quiet);
    let polls = Cell::new(0);
    attempt(&mut session, prepared, || {
        polls.set(polls.get() + 1);
        stop_after_chunk && polls.get() == 5
    })
}

fn metadata_requests() -> Vec<Vec<u8>> {
    vec![
        line6_devices::identity::REQUEST.into(),
        guitar(&[0x2a, 1]),
        guitar(&[0x3c, 0, 2, 0, 0, 0, 0, 3, 5, 3, 0, 0, 0]),
        guitar(&[0x3c, 0, 2, 0, 0, 0, 0, 3, 7, 3, 0, 0, 0]),
        guitar(&[0x2a, 0]),
    ]
}

fn cleanup() -> [Vec<u8>; 7] {
    [
        HEADER.into(),
        vec![0x4c, 0],
        vec![0xf7],
        HEADER.into(),
        vec![0x2a, 0],
        vec![0xf7],
        BRIDGE_OFF.into(),
    ]
}

#[test]
fn both_stages_use_the_original_full_edit_sequence_and_read_back() {
    for stage in [Stage::Own, Stage::Trim] {
        let prepared = prepared(stage);
        let mut fake = Fake::variax(&prepared);
        exercise(&mut fake, &prepared, false).unwrap();

        let full = guitar(
            &[
                &[0x43, 4, 4, 4, 0, 0, 0],
                sysex_words(&prepared.program).as_slice(),
            ]
            .concat(),
        );
        assert_eq!(full.len(), 6_566);
        assert_eq!(
            full.chunks(256).map(<[u8]>::len).collect::<Vec<_>>(),
            [&[256; 25][..], &[166]].concat()
        );
        let mut expected = metadata_requests();
        expected.extend([BRIDGE_ON.into(), HEADER.into(), vec![0x2a, 1], vec![0xf7]]);
        expected.extend(full.chunks(256).map(<[u8]>::to_vec));
        expected.extend(cleanup());
        expected.extend(metadata_requests());
        assert_eq!(fake.writes.len(), expected.len());
        for (i, (actual, expected)) in fake.writes.iter().zip(&expected).enumerate() {
            assert!(actual == expected, "transport write {i} differs");
        }
        assert_eq!(fake.pauses, [Duration::from_millis(100); 3]);
    }
}

#[test]
fn wrong_podxt_or_variax_identity_stops_at_identity() {
    let mut wrong_podxt = PODXT_IDENTITY;
    wrong_podxt[15] = 2;
    for identity in [PODXT_IDENTITY, wrong_podxt] {
        let mut fake = Fake::new(DevicePath::PodxtMidi, [identity.into()].into());
        let result = check_podxt(&mut fake, &mut Quiet);
        assert_eq!(result.is_ok(), identity == PODXT_IDENTITY);
        assert_eq!(fake.writes, [line6_devices::identity::REQUEST]);
    }
    let prepared = prepared(Stage::Own);
    let mut fake = Fake::variax(&prepared);
    fake.inbox[0][14] = b'0';
    assert!(
        exercise(&mut fake, &prepared, false)
            .unwrap_err()
            .contains("firmware 3.10")
    );
    assert_eq!(fake.writes, [line6_devices::identity::REQUEST]);
}

#[test]
fn wrong_preset_and_unexpected_reply_stop_before_edit() {
    let prepared = prepared(Stage::Own);
    let mut fake = Fake::variax(&prepared);
    let mut changed = prepared.start;
    changed[0] = b'X';
    fake.inbox[1] = reply(&changed[..96]);
    assert!(
        exercise(&mut fake, &prepared, false)
            .unwrap_err()
            .contains("active Variax settings")
    );
    assert_eq!(fake.writes, metadata_requests());

    let mut fake = Fake::variax(&prepared);
    fake.inbox.insert(1, guitar(&[0x4d]));
    assert!(
        exercise(&mut fake, &prepared, false)
            .unwrap_err()
            .contains("Unexpected SysEx")
    );
    assert_eq!(fake.writes.last(), Some(&guitar(&[0x2a, 0])));
    assert!(!fake.writes.iter().any(|bytes| bytes == BRIDGE_ON));
}

#[test]
fn readback_mismatch_refuses_a_pass() {
    let prepared = prepared(Stage::Trim);
    let mut fake = Fake::variax(&prepared);
    fake.inbox[4] = reply(&prepared.start[..96]);
    assert!(
        exercise(&mut fake, &prepared, false)
            .unwrap_err()
            .contains("Settings read-back differs")
    );
}

#[test]
fn interrupted_transfer_terminates_sysex_and_attempts_all_cleanup() {
    for cancel in [false, true] {
        let prepared = prepared(Stage::Own);
        let mut fake = Fake::variax(&prepared);
        if !cancel {
            fake.fail_at = Some(10);
        }
        let error = exercise(&mut fake, &prepared, cancel).unwrap_err();
        assert!(error.contains(if cancel {
            "transfer stopped"
        } else {
            "transport write failure"
        }));
        assert_eq!(fake.chunks.get(), if cancel { 1 } else { 2 });
        assert_eq!(fake.writes[fake.writes.len() - 8], [0xf7]);
        assert!(fake.writes.ends_with(&cleanup()));
        assert_eq!(fake.inbox.len(), 3);
    }
}

#[test]
fn pinned_hash_and_both_reverb_tables_are_checked() {
    let mut candidate = prepared(Stage::Own).program;
    let firmware = candidate;
    let hash = |program: &[u8]| format!("{:x}", Sha256::digest(program));
    check_program(&candidate, &hash(&candidate), &firmware).unwrap();
    assert!(
        check_program(&candidate, OWN_HASH, &firmware)
            .unwrap_err()
            .contains("Mac Workbench")
    );

    candidate[632 * 3 + 2] = 3;
    checksum(&mut candidate);
    assert!(
        check_program(&candidate, &hash(&candidate), &firmware)
            .unwrap_err()
            .contains("632")
    );
    candidate[632 * 3 + 2] = 0;
    candidate[643 * 3] = 0x80;
    checksum(&mut candidate);
    assert!(
        check_program(&candidate, &hash(&candidate), &candidate)
            .unwrap_err()
            .contains("643")
    );
}

fn backup_at(dir: &Path, reso: &[u8; 192], date: &str, include_bank_names: bool) {
    fs::create_dir(dir.join("presets")).unwrap();
    if include_bank_names {
        fs::create_dir(dir.join("bank-names")).unwrap();
    }
    let mut manifest = String::new();
    let mut add = |name: &str, bytes: &[u8]| {
        fs::write(dir.join(name), bytes).unwrap();
        manifest.push_str(&format!("{:x}  {name}\n", Sha256::digest(bytes)));
    };
    for slot in variax::Preset::all() {
        let stem = backup::variax_preset_stem(slot);
        let raw = if slot == location() {
            *reso
        } else {
            settings()
        };
        add(&format!("{stem}-1.syx"), &reply(&raw[..96]));
        add(&format!("{stem}-2.syx"), &reply(&raw[96..]));
    }
    if include_bank_names {
        for bank in 0..12 {
            let mut reply = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x6d];
            reply.extend(format!("Bank {bank}").bytes());
            reply.resize(25, 0);
            reply.push(0xf7);
            add(&format!("bank-names/{bank:02}.syx"), &reply);
        }
    }
    add("identity.syx", &VARIAX_IDENTITY);
    add(
        "log.txt",
        format!("Variax library backup at {date} 00:00:00 UTC.\n").as_bytes(),
    );
    fs::write(dir.join("MANIFEST.sha256"), manifest).unwrap();
}

#[test]
fn matching_old_backup_is_accepted_but_mismatched_or_incomplete_backups_are_refused() {
    let old = tempfile::tempdir().unwrap();
    let recent = tempfile::tempdir().unwrap();
    let baseline = settings();
    backup_at(old.path(), &baseline, "2025-01-01", false);
    assert_eq!(check_backup(old.path(), &baseline).unwrap(), baseline);

    let mut different = baseline;
    different[64] ^= 1;
    backup_at(recent.path(), &different, "2026-11-24", false);
    let error = check_backup(recent.path(), &baseline).unwrap_err();
    assert!(error.contains("backup's slot 50 settings"));
    assert!(error.contains("save_tone_position at settings byte 64"));

    fs::remove_file(old.path().join("presets/10-Reso-5-2.syx")).unwrap();
    assert!(check_backup(old.path(), &baseline).is_err());
}

#[test]
fn backup_comparison_reports_a_saved_tone_toggle_and_preserves_both_captures() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let active = settings();
    let mut saved = active;
    saved[64] = 1;

    for (folder, raw) in [(first.path(), active), (second.path(), saved)] {
        backup_at(folder, &raw, "2026-11-24", false);
        let loaded = backup::load_variax_library(folder).unwrap();
        let (_, metadata) = loaded
            .presets
            .iter()
            .find(|(preset, _)| *preset == location())
            .unwrap();
        assert_eq!(metadata.raw, raw);
    }

    let name = format!("{}-1.syx", backup::variax_preset_stem(location()));
    let comparison = backup::compare(first.path(), second.path()).unwrap();
    assert_eq!(comparison.identical, 119);
    assert_eq!(comparison.problems, [format!("differs: {name}")]);
    assert!(!comparison.verified());
    assert_eq!(
        fs::read(first.path().join(&name)).unwrap(),
        reply(&active[..96])
    );
    assert_eq!(
        fs::read(second.path().join(&name)).unwrap(),
        reply(&saved[..96])
    );
}

#[test]
fn backup_check_accepts_bank_name_replies() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = settings();
    let date = "2026-11-23";
    backup_at(dir.path(), &baseline, date, true);
    check_backup(dir.path(), &baseline).unwrap();
}

#[test]
fn nonexistent_reference_is_refused_offline() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.vxf");
    let Err(error) = prepare(Stage::Own, dir.path(), &missing, &missing) else {
        panic!("invalid archive accepted");
    };
    assert!(
        error.contains(if variax_workbench::math::PROFILE == "host-std-v1" {
            "missing.vxf"
        } else {
            "host-std-v1"
        })
    );
}

#[test]
fn the_observed_identity_tail_allows_one_read_only_retry() {
    let prepared = prepared(Stage::Own);
    let mut fake = Fake::variax(&prepared);
    fake.inbox.insert(0, vec![0xf7]);
    fake.inbox.insert(1, vec![]);
    fake.inbox.insert(2, vec![]);
    fake.inbox.insert(3, vec![]);
    exercise(&mut fake, &prepared, false).unwrap();
    assert_eq!(fake.writes[0..2], [line6_devices::identity::REQUEST; 2]);
}

#[test]
fn the_policies_refuse_storage_preset_switches_and_podxt_writes() {
    for forbidden in [
        Operation::VariaxStoreRestore,
        Operation::VariaxPresetSwitch,
        Operation::PodxtIdentity,
        Operation::PodxtEditBufferWrite,
        Operation::PodxtAdditionalEditBufferWrite,
        Operation::PodxtChannelSwitch,
        Operation::PodxtLinkedVariaxModelEdit,
    ] {
        assert!(!VariaxPolicy.allows(forbidden));
        assert_eq!(
            PodxtIdentityOnly.allows(forbidden),
            forbidden == Operation::PodxtIdentity
        );
    }
}

#[test]
fn stage_numbers_and_hashes_are_distinct() {
    assert_eq!(Stage::Own.number(), 1);
    assert_eq!(Stage::Trim.number(), 2);
    assert_eq!(Stage::Own.hash(), OWN_HASH);
    assert_eq!(Stage::Trim.hash(), TRIM_HASH);
    assert_ne!(OWN_HASH, TRIM_HASH);
}

mod store;

mod slot_read;

mod full_read;

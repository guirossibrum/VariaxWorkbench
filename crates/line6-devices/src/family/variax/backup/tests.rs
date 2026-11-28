use super::*;
use crate::{
    gate::GatePolicy,
    identity::{REQUEST as IDENTITY, tests::VARIAX_600},
    session::{
        Path as DevicePath, Quiet,
        testing::{Fake, packets},
    },
};
use std::cell::Cell;
use variax::{
    Preset, Restore,
    tests::{bank_reply, encode_reply},
};

mod compact_buffer;
mod custom_restore;
mod snapshot;

const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const SESSION_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7];
const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const BLOCK_1: &[u8] = &[
    0xf0, 0, 1, 0x0c, 7, 0, 0x3c, 0, 2, 0, 0, 0, 0, 3, 5, 3, 0, 0, 0, 0xf7,
];
const BLOCK_2: &[u8] = &[
    0xf0, 0, 1, 0x0c, 7, 0, 0x3c, 0, 2, 0, 0, 0, 0, 3, 7, 3, 0, 0, 0, 0xf7,
];
const HEADER: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0];

const OPERATIONS: [Operation; 5] = [
    Operation::VariaxIdentity,
    Operation::VariaxLibraryBridge,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
    Operation::VariaxBankNameRead,
];

struct Policy(Option<Operation>);

impl GatePolicy for Policy {
    fn allows(&self, operation: Operation) -> bool {
        OPERATIONS.contains(&operation) && self.0 != Some(operation)
    }
}

pub(super) type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

pub(super) fn metadata(program: u8) -> [u8; 192] {
    let mut raw = [0; 192];
    let name = format!("Preset {program}");
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw[64] = program % 2;
    raw[191] = program;
    raw
}

pub(super) fn blocks(script: &mut Script, data: &[u8; 192]) {
    script.extend([
        (
            BLOCK_1.into(),
            packets(&encode_reply(data[..96].try_into().unwrap())),
        ),
        (
            BLOCK_2.into(),
            packets(&encode_reply(data[96..].try_into().unwrap())),
        ),
    ]);
}

const VARIAX_600_310: [u8; 17] = {
    let mut reply = VARIAX_600;
    let [major, minor, patch, build] = *b"0310";
    reply[12] = major;
    reply[13] = minor;
    reply[14] = patch;
    reply[15] = build;
    reply
};

pub(super) fn start(original: u8) -> Script {
    let mut script = vec![
        (IDENTITY.into(), packets(&VARIAX_600)),
        (BRIDGE_ON.into(), vec![]),
        (SESSION_ON.into(), vec![]),
    ];
    blocks(&mut script, &metadata(original));
    script
}

pub(super) fn select(script: &mut Script, program: u8, data: u8) {
    script.push((vec![0xc0, program], vec![]));
    blocks(script, &metadata(data));
}

pub(super) fn bank(script: &mut Script, reply: Vec<Vec<u8>>) {
    script.extend([
        (HEADER.into(), vec![]),
        (vec![0x6d], vec![]),
        (vec![0xf7], reply),
    ]);
}

pub(super) fn end(script: &mut Script) {
    script.extend([(SESSION_OFF.into(), vec![]), (BRIDGE_OFF.into(), vec![])]);
}

fn whole_sweep(original: u8, duplicate: bool) -> Script {
    let mut script = start(original);
    for (index, highest) in [4, 64, 69, 74, 79, 84, 89, 94, 99, 104, 109, 59]
        .into_iter()
        .enumerate()
    {
        for program in (highest - 4..=highest).rev() {
            let data = if duplicate && program == 3 {
                original
            } else {
                program
            };
            select(&mut script, program, data);
        }
        bank(
            &mut script,
            packets(&bank_reply(format!("Bank {index}").as_bytes())),
        );
    }
    if !duplicate {
        select(&mut script, original, original);
    }
    end(&mut script);
    script
}

fn run(
    script: Script,
    fail_at: Vec<usize>,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(Preset),
) -> Result<RefreshedLibrary, RefreshError> {
    let mut quiet = Quiet;
    let mut fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    fake.fail_at = fail_at;
    let mut session = Session::new(fake, &Policy(None), &mut quiet);
    let result = refresh_on(&mut session, cancelled, progress);
    session.link.assert_finished();
    result
}

fn refresh_identity(mut script: Script) -> Script {
    script[0].1 = packets(&VARIAX_600_310);
    script
}

#[test]
fn refresh_returns_all_sixty_settings_and_twelve_bank_names_in_backup_order() {
    let mut progress = Vec::new();
    let capture = run(
        refresh_identity(whole_sweep(85, false)),
        vec![],
        || false,
        |preset| progress.push(preset),
    )
    .unwrap();
    assert_eq!(capture.identified.reply, VARIAX_600_310);
    assert_eq!(capture.library.original.unwrap().raw, metadata(85));
    assert_eq!(progress, Preset::all().collect::<Vec<_>>());
    assert_eq!(capture.library.presets.len(), 60);
    for ((preset, data), visited) in capture.library.presets.iter().zip(progress) {
        assert_eq!(*preset, visited);
        assert_eq!(data.raw, metadata(preset.program_number()));
    }
    assert_eq!(capture.library.bank_names.len(), 12);
    for (index, (bank, name)) in capture.library.bank_names.iter().enumerate() {
        assert_eq!(usize::from(*bank), index);
        assert_eq!(name.name, format!("Bank {index}"));
        assert_eq!(name.reply, bank_reply(name.name.as_bytes()));
    }
    assert_eq!(
        capture.restore,
        Restore::Switched {
            to: Preset::new(6, 4).unwrap(),
            verified: true
        }
    );
    assert!(capture.release_error.is_none());
}

#[test]
fn refresh_retries_silent_identity_before_selecting_any_preset() {
    let mut script = refresh_identity(whole_sweep(85, false));
    script.insert(0, (IDENTITY.into(), vec![]));
    let capture = run(script, vec![], || false, |_| {}).unwrap();
    assert_eq!(capture.identified.requests, 2);
    assert_eq!(capture.library.presets.len(), 60);
}

#[test]
fn refresh_stops_after_two_silent_requests_without_selecting_a_preset() {
    let script = vec![(IDENTITY.into(), vec![]); 2];
    let result = run(script, vec![], || false, |_| panic!("unexpected progress"));
    assert!(matches!(result, Err(RefreshError::Read(error))
        if error.contains("Guitar sync identity: No reply after two requests.")));
}

#[test]
fn refresh_does_not_retry_a_reply_from_unsupported_firmware() {
    let script = vec![(IDENTITY.into(), packets(&VARIAX_600))];
    let result = run(script, vec![], || false, |_| panic!("unexpected progress"));
    assert!(matches!(result, Err(RefreshError::Read(error))
        if error.contains("Guitar sync identity: Expected Variax 300/600")));
}

#[test]
fn a_damaged_metadata_block_retries_without_reselecting_and_the_backup_completes() {
    let original = 85;
    let mut script = whole_sweep(original, false);
    let damaged = script
        .iter()
        .position(|(request, _)| request == &[0xc0, 74])
        .unwrap()
        + 1;
    let (request, _) = &script[damaged];
    assert_eq!(request, BLOCK_1);
    let mut short = encode_reply(metadata(74)[..96].try_into().unwrap());
    short.remove(99);
    script.insert(damaged, (BLOCK_1.into(), packets(&short)));

    let mut log = Quiet;
    let fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    let mut session = Session::new(fake, &Policy(None), &mut log);
    let backup = variax::read_library(&mut session, |_| {}).unwrap();
    session.link.assert_finished();
    assert!(backup.stopped.is_none() && backup.release_error.is_none());
    assert_eq!(backup.presets.len(), 60);
    assert_eq!(backup.bank_names.len(), 12);
    assert_eq!(
        backup.restore,
        Restore::Switched {
            to: Preset::new(6, 4).unwrap(),
            verified: true,
        }
    );
    let lester = backup
        .presets
        .iter()
        .find(|entry| entry.preset.program_number() == 74)
        .unwrap();
    assert_eq!(lester.metadata.raw, metadata(74));
    assert_eq!(lester.replies[0].len(), 200);
}

#[test]
fn cancel_between_presets_attempts_switch_back_and_withholds_partial_library() {
    let mut script = refresh_identity(start(4));
    select(&mut script, 4, 4);
    select(&mut script, 3, 3);
    select(&mut script, 4, 4);
    end(&mut script);
    let processed = Cell::new(0);
    let result = run(
        script,
        vec![],
        || processed.get() == 2,
        |_| processed.set(processed.get() + 1),
    );
    let Err(RefreshError::Incomplete {
        reason,
        restore,
        release_error,
    }) = result
    else {
        panic!("cancelled read must return incomplete status");
    };
    assert!(reason.contains("cancelled"));
    assert_eq!(
        restore,
        Restore::Switched {
            to: Preset::new(0, 0).unwrap(),
            verified: true
        }
    );
    assert!(release_error.is_none());
}

#[test]
fn a_failed_bank_name_read_stops_the_sweep_and_reselects_the_original() {
    let mut script = refresh_identity(start(4));
    for program in (0..=4).rev() {
        select(&mut script, program, program);
    }
    bank(&mut script, vec![]);
    select(&mut script, 4, 4);
    end(&mut script);
    let mut count = 0;
    let result = run(script, vec![], || false, |_| count += 1);
    let Err(RefreshError::Incomplete {
        reason, restore, ..
    }) = result
    else {
        panic!("missing bank name must prevent returning a library");
    };
    assert_eq!(count, 5);
    assert!(reason.contains("Custom 1 bank name: Timed out"));
    assert_eq!(
        restore,
        Restore::Switched {
            to: Preset::new(0, 0).unwrap(),
            verified: true
        }
    );
}

#[test]
fn ambiguous_original_yields_complete_data_without_an_automatic_switch() {
    let capture = run(
        refresh_identity(whole_sweep(4, true)),
        vec![],
        || false,
        |_| {},
    )
    .unwrap();
    assert_eq!(capture.library.presets.len(), 60);
    assert_eq!(capture.library.bank_names.len(), 12);
    assert_eq!(capture.restore, Restore::NoUniqueMatch { matches: 2 });
}

#[test]
fn complete_data_retains_the_bridge_release_failure() {
    let script = refresh_identity(whole_sweep(4, false));
    let release_at = script.len() - 1;
    let capture = run(script, vec![release_at], || false, |_| {}).unwrap();
    assert_eq!(capture.library.presets.len(), 60);
    assert_eq!(capture.library.bank_names.len(), 12);
    assert!(
        capture
            .release_error
            .unwrap()
            .contains("PODxt Variax-messages release failed")
    );
}

#[test]
fn missing_permission_refuses_before_the_identity_request() {
    for denied in OPERATIONS {
        let mut quiet = Quiet;
        let policy = Policy(Some(denied));
        let mut session = Session::new(
            Fake::from_bytes(DevicePath::VariaxViaPodxt, vec![]),
            &policy,
            &mut quiet,
        );
        let result = refresh_on(&mut session, || false, |_| panic!("unexpected progress"));
        let Err(RefreshError::Read(error)) = result else {
            panic!("missing gate must refuse");
        };
        assert!(error.contains(&format!("{denied:?}")));
        assert!(session.link.writes.is_empty());
    }
}

fn full_start(original: u8) -> Script {
    let mut script = start(original);
    let mut id = VARIAX_600;
    id[12..16].copy_from_slice(b"0310");
    script[0].1 = packets(&id);
    script
}

fn full_word(program: u8, index: u16) -> u32 {
    0xf0_0000 | (u32::from(program) << 12) | u32::from(index)
}

fn program_chunk(program: u8, start: u16, corrupt: bool) -> (Vec<u8>, Vec<Vec<u8>>) {
    let count = (1092 - start).min(32);
    let words = (start..start + count).map(|i| {
        if i == 1091 {
            let sum: u64 = (0..1091).map(|n| u64::from(full_word(program, n))).sum();
            (sum as u32 & 0xff_ffff) ^ u32::from(corrupt)
        } else {
            full_word(program, i)
        }
    });
    program_exchange(start, words)
}

fn program_exchange(
    start: u16,
    words: impl ExactSizeIterator<Item = u32>,
) -> (Vec<u8>, Vec<Vec<u8>>) {
    let append = |bytes: &mut Vec<u8>, word: u32| {
        bytes.extend((0..6).map(|i| ((word >> (4 * i)) & 15) as u8));
    };
    let mut request = [HEADER, &[0x3c]].concat();
    append(&mut request, words.len() as u32);
    append(&mut request, u32::from(start));
    request.push(0xf7);
    let mut reply = [HEADER, &[0x3c]].concat();
    for word in words {
        append(&mut reply, word);
    }
    reply.push(0xf7);
    (request, packets(&reply))
}

fn add_program(script: &mut Script, program: u8, corrupt: bool) {
    script.extend(
        (0..1092)
            .step_by(32)
            .map(|start| program_chunk(program, start, corrupt)),
    );
}

fn run_full(
    script: Script,
    cancelled: impl FnMut() -> bool,
    progress: impl FnMut(&variax::StoredPreset),
) -> FullLibrary {
    let mut quiet = Quiet;
    let fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    let mut session = Session::new(fake, &Policy(None), &mut quiet);
    let backup = read_full_on(&mut session, cancelled, progress).unwrap();
    session.link.assert_finished();
    backup
}

fn saved_backup(backup: &FullLibrary, label: &str) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("variax-full-backup-{label}-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut folder = BackupFolder::create(&root, "backup").unwrap();
    backup.save(&mut folder).unwrap();
    let saved = folder.finish(backup.complete()).unwrap();
    assert!(BackupFolder::create(&root, "backup").is_err());
    verify_manifest(&saved).unwrap();
    saved
}

#[test]
fn full_library_reads_exact_order_and_saves_big_endian_programs_with_settings() {
    let mut script = full_start(105);
    for (index, highest) in [4, 64, 69, 74, 79, 84, 89, 94, 99, 104, 109, 59]
        .into_iter()
        .enumerate()
    {
        for program in (highest - 4..=highest).rev() {
            select(&mut script, program, program);
            add_program(&mut script, program, false);
        }
        bank(
            &mut script,
            packets(&bank_reply(format!("Bank {index}").as_bytes())),
        );
    }
    select(&mut script, 105, 105);
    end(&mut script);

    let mut progress = 0;
    let backup = run_full(script, || false, |_| progress += 1);
    assert_eq!(progress, 60);
    assert!(backup.complete());
    assert_eq!(backup.settings.bank_names.len(), 12);
    assert_eq!(
        backup.settings.restore,
        Restore::Switched {
            to: Preset::new(10, 4).unwrap(),
            verified: true
        }
    );
    let path = saved_backup(&backup, "complete");
    let name = "programs/50-Reso-5.bin";
    let reso = std::fs::read(path.join(name)).unwrap();
    assert_eq!(reso.len(), 3276);
    assert_eq!(
        std::fs::read_dir(path.join("programs")).unwrap().count(),
        60
    );
    for (i, bytes) in reso.as_chunks::<3>().0[..1091].iter().enumerate() {
        assert_eq!(*bytes, full_word(105, i as u16).to_be_bytes()[1..]);
    }
    let index = std::fs::read_to_string(path.join("index.txt")).unwrap();
    assert!(index.contains("programs/50-Reso-5.bin: verified"));
    let manifest = std::fs::read_to_string(path.join("MANIFEST.sha256")).unwrap();
    assert!(manifest.contains(name));
    assert_eq!(load_variax_library(&path).unwrap().presets.len(), 60);
    let snapshot = validate_snapshot(
        &path,
        &backup.settings.identified.identity,
        SnapshotCoverage::FullPrograms,
    )
    .unwrap();
    let SnapshotPrograms::Full(programs) = snapshot.programs() else {
        panic!("expected complete programs");
    };
    assert_eq!(programs.len(), 60);
    assert!(
        programs
            .iter()
            .any(|(preset, program)| *preset == Preset::new(10, 4).unwrap()
                && program.as_slice() == reso)
    );
    snapshot
        .check_cache_eligibility(SnapshotCapture::Full(&backup), Some(&Ok(())))
        .unwrap();

    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn bad_checksum_or_truncated_reply_keeps_settings_and_marks_the_program_missing() {
    for (label, expected) in [
        ("checksum", "Firmware checksum mismatch"),
        ("truncated", "Start 32, count 32"),
    ] {
        let mut script = full_start(4);
        select(&mut script, 4, 4);
        if label == "checksum" {
            add_program(&mut script, 4, true);
        } else {
            script.push(program_chunk(4, 0, false));
            let (request, mut reply) = program_chunk(4, 32, false);
            reply.pop(); // Missing the end of this span.
            script.extend([
                (request.clone(), reply),
                (request, vec![]), // No reply to the retry.
            ]);
        }
        select(&mut script, 4, 4);
        end(&mut script);

        let backup = run_full(script, || false, |_| panic!("no verified program"));
        assert!(!backup.complete());
        assert_eq!(backup.programs_read(), 0);
        assert_eq!(backup.settings.presets.len(), 1);
        assert!(backup.settings.stopped.as_ref().unwrap().contains(expected));
        let path = saved_backup(&backup, label);
        assert_eq!(path.file_name().unwrap(), "backup-INCOMPLETE");
        assert!(path.join("presets/00-Custom_1-1-1.syx").is_file());
        assert!(!path.join("programs/04-Custom_1-1.bin").exists());
        let index = std::fs::read_to_string(path.join("index.txt")).unwrap();
        assert!(index.contains(&format!("programs/04-Custom_1-1.bin: MISSING ({expected}")));
        assert!(index.contains("programs/03-Custom_1-2.bin: MISSING (not read)"));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[test]
fn cancellation_between_chunks_reselects_the_original_and_keeps_only_completed_programs() {
    let mut script = full_start(3);
    select(&mut script, 4, 4);
    add_program(&mut script, 4, false);
    select(&mut script, 3, 3);
    script.push(program_chunk(3, 0, false));
    select(&mut script, 3, 3);
    end(&mut script);

    let first_complete = Cell::new(false);
    let checks = Cell::new(0);
    let backup = run_full(
        script,
        || {
            if first_complete.get() {
                checks.set(checks.get() + 1);
            }
            checks.get() >= 3
        },
        |_| first_complete.set(true),
    );
    assert!(backup.settings.stopped.unwrap().contains("cancelled"));
    assert_eq!(
        backup.settings.restore,
        Restore::Switched {
            to: Preset::new(0, 1).unwrap(),
            verified: true
        }
    );
    assert!(backup.programs.programs[0].1.is_ok());
    assert!(backup.programs.programs[1].1.is_err());
}

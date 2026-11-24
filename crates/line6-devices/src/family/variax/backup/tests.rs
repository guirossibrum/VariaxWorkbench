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

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

fn metadata(program: u8) -> [u8; 192] {
    let mut raw = [0; 192];
    let name = format!("Preset {program}");
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw[64] = program % 2;
    raw[191] = program;
    raw
}

fn blocks(script: &mut Script, data: &[u8; 192]) {
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

fn start(original: u8) -> Script {
    let mut script = vec![
        (IDENTITY.into(), packets(&VARIAX_600)),
        (BRIDGE_ON.into(), vec![]),
        (SESSION_ON.into(), vec![]),
    ];
    blocks(&mut script, &metadata(original));
    script
}

fn select(script: &mut Script, program: u8, data: u8) {
    script.push((vec![0xc0, program], vec![]));
    blocks(script, &metadata(data));
}

fn bank(script: &mut Script, reply: Vec<Vec<u8>>) {
    script.extend([
        (HEADER.into(), vec![]),
        (vec![0x6d], vec![]),
        (vec![0xf7], reply),
    ]);
}

fn end(script: &mut Script) {
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
    cancelled: impl FnMut() -> bool,
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

#[test]
fn refresh_returns_all_sixty_settings_and_twelve_bank_names_in_backup_order() {
    let mut progress = Vec::new();
    let capture = run(
        whole_sweep(85, false),
        vec![],
        || false,
        |preset| progress.push(preset),
    )
    .unwrap();
    assert_eq!(capture.identified.reply, VARIAX_600);
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
fn cancel_between_presets_attempts_switch_back_and_withholds_partial_library() {
    let mut script = start(4);
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
    let mut script = start(4);
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
    let capture = run(whole_sweep(4, true), vec![], || false, |_| {}).unwrap();
    assert_eq!(capture.library.presets.len(), 60);
    assert_eq!(capture.library.bank_names.len(), 12);
    assert_eq!(capture.restore, Restore::NoUniqueMatch { matches: 2 });
}

#[test]
fn complete_data_retains_the_bridge_release_failure() {
    let script = whole_sweep(4, false);
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

use super::*;
use crate::{
    backup::verify_manifest,
    gate::{AllClosed, GatePolicy},
    identity::{REQUEST, tests::VARIAX_600},
    session::{
        Quiet,
        testing::{AllowAll, Fake, packets},
    },
};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "variax-string-levels-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn path(&self) -> &FilePath {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Ok(entries) = fs::read_dir(self.0.join("levels")) {
            for entry in entries.flatten() {
                let mut permissions = entry.metadata().unwrap().permissions();
                permissions.set_readonly(false);
                let _ = fs::set_permissions(entry.path(), permissions);
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;
const ORIGINAL: Levels = Levels([0x800000, 0x123abc, 0xeeeeee, 0x345678, 0xffffff, 0x7fffff]);
const CHANGE: Levels = Levels([0x810000, 0x2abcde, 0xe00000, 0x1abcd0, 0xff0000, 0x7edcba]);
const READ_BYTES: &[u8] = &[0xf0, 0, 1, 0xc, 7, 0, 0x57, 0xf7];
const STORE_BYTES: &[u8] = &[0xf0, 0, 1, 0xc, 7, 0, 0x56, 0xf7];

fn reply_frame(opcode: u8, channel: u8, levels: Levels) -> Vec<u8> {
    let mut bytes = vec![0xf0, 0, 1, 0xc, 7, channel, opcode];
    for word in levels.wire_words() {
        bytes.extend(
            format!("{word:06x}")
                .chars()
                .rev()
                .map(|digit| digit.to_digit(16).unwrap() as u8),
        );
    }
    bytes.push(0xf7);
    bytes
}

fn write_levels(levels: Levels) -> (Vec<u8>, Vec<Vec<u8>>) {
    (reply_frame(0x54, 0, levels), vec![])
}

fn read_levels(levels: Levels) -> (Vec<u8>, Vec<Vec<u8>>) {
    (READ_BYTES.to_vec(), packets(&reply_frame(0x57, 4, levels)))
}

fn start() -> Script {
    let mut identity = VARIAX_600;
    identity[12..16].copy_from_slice(b"0310");
    vec![
        (REQUEST.to_vec(), packets(&identity)),
        (vec![0xf0, 0, 1, 0xc, 3, 0x23, 1, 0xf7], vec![]),
        (vec![0xf0, 0, 1, 0xc, 7, 0, 0x2a, 1, 0xf7], vec![]),
    ]
}

fn finish(script: &mut Script) {
    script.extend([
        (vec![0xf0, 0, 1, 0xc, 7, 0, 0x2a, 0, 0xf7], vec![]),
        (vec![0xf0, 0, 1, 0xc, 3, 0x23, 0, 0xf7], vec![]),
    ]);
}

fn run_edit<T>(
    mut script: Script,
    fail_at: Vec<usize>,
    policy: &dyn GatePolicy,
    root: &FilePath,
    run: impl FnOnce(&mut Edit<'_, '_, Fake>) -> Result<T, String>,
) -> Result<T, String> {
    finish(&mut script);
    let mut link = Fake::from_bytes(Path::VariaxViaPodxt, script);
    link.fail_at = fail_at;
    let mut log = Quiet;
    let mut session = Session::new(link, policy, &mut log);
    let result = edit_on(&mut session, root, "levels", run);
    session.link.assert_finished();
    result
}

#[test]
fn raw_backup_and_workbench_slider_conversion_preserve_their_distinct_representations() {
    assert_eq!(
        ORIGINAL.backup_bytes(),
        [
            0x80, 0, 0, 0x12, 0x3a, 0xbc, 0xee, 0xee, 0xee, 0x34, 0x56, 0x78, 0xff, 0xff, 0xff,
            0x7f, 0xff, 0xff
        ]
    );
    assert_eq!(Levels::from_backup_bytes(ORIGINAL.backup_bytes()), ORIGINAL);
    assert!(Levels::from_wire_words([0x1000000; 6]).is_err());
    assert_eq!(
        ORIGINAL
            .with_workbench_levels([1.0, 0.5, 0.0, 1.0, 0.5, 0.0])
            .unwrap()
            .wire_words(),
        [0, 0x3fffff, 0x800000, 0, 0xc00000, 0x7fffff]
    );
    let reversed = Levels([0, 0, 0, 0, 0, 0x800000]);
    assert_eq!(reversed.workbench_levels()[0], 1.0);
    assert_eq!(
        reversed
            .with_workbench_levels([1.0; 6])
            .unwrap()
            .wire_words(),
        [0x7fffff, 0x800000, 0x7fffff, 0x800000, 0x7fffff, 0x800000]
    );
    assert!(ORIGINAL.with_workbench_levels([f32::NAN; 6]).is_err());
}

#[test]
fn verified_read_skips_notices_and_closes_the_session() {
    let mut script = start();
    let (write, mut replies) = read_levels(ORIGINAL);
    replies.insert(0, vec![0xf0, 0, 1, 0xc, 7, 4, 0x28, 0xf7]);
    script.push((write, replies));
    finish(&mut script);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &AllowAll,
        &mut log,
    );
    assert_eq!(read_on(&mut session).unwrap(), ORIGINAL);
    session.link.assert_finished();
}

#[test]
fn damaged_and_missing_ram_replies_refuse_and_cleanup() {
    let good = reply_frame(0x57, 4, ORIGINAL);
    for reply in [
        vec![],
        good[..43].to_vec(),
        reply_frame(0x56, 4, ORIGINAL),
        reply_frame(0x57, 3, ORIGINAL),
        {
            let mut bad = good.clone();
            bad[12] = 16;
            bad
        },
        {
            let mut bad = good.clone();
            bad.remove(17);
            bad
        },
        {
            let mut bad = good.clone();
            bad.insert(17, 0);
            bad
        },
    ] {
        let mut script = start();
        script.push((READ_BYTES.to_vec(), packets(&reply)));
        finish(&mut script);
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, script),
            &AllowAll,
            &mut log,
        );
        assert!(read_on(&mut session).is_err());
        session.link.assert_finished();
    }
}

#[test]
fn approvals_policy_and_identity_refuse_before_level_messages() {
    let tmp = TempRoot::new();
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, vec![]),
        &AllowAll,
        &mut log,
    );
    assert!(read_ram(&mut session).unwrap_err().contains("approval"));
    assert!(
        with_temporary_edit(&mut session, tmp.path(), "levels", |_| Ok(()))
            .unwrap_err()
            .contains("approval")
    );
    for (path, policy) in [
        (Path::VariaxViaPodxt, &AllClosed as &dyn GatePolicy),
        (Path::PodxtMidi, &AllowAll as &dyn GatePolicy),
    ] {
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut log);
        assert!(read_on(&mut session).is_err());
        assert!(edit_on(&mut session, tmp.path(), "levels", |_| Ok(())).is_err());
    }
    let mut wrong = VARIAX_600;
    wrong[12..16].copy_from_slice(b"0307");
    let script = vec![(REQUEST.to_vec(), packets(&wrong))];
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &AllowAll,
        &mut log,
    );
    assert!(
        edit_on(&mut session, tmp.path(), "levels", |_| Ok(()))
            .unwrap_err()
            .contains("3.10")
    );
    assert!(!tmp.path().join("levels").exists());
    session.link.assert_finished();
}

#[test]
fn backup_precedes_edits_and_restoration_uses_original_raw_words() {
    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        write_levels(CHANGE),
        read_levels(CHANGE),
        write_levels(ORIGINAL),
        read_levels(ORIGINAL),
    ]);
    run_edit(script, vec![], &AllowAll, tmp.path(), |edit| {
        assert_eq!(edit.original(), ORIGINAL);
        let root = edit.backup_folder();
        assert_eq!(
            fs::read(root.join(BACKUP_FILE)).unwrap(),
            ORIGINAL.backup_bytes()
        );
        verify_manifest(root).unwrap();
        assert_eq!(edit.set_ram(CHANGE)?, CHANGE);
        assert!(edit.request_store().unwrap_err().contains("owner approval"));
        edit.restore_ram()
    })
    .unwrap();
}

#[test]
fn backup_collision_prevents_any_edit() {
    let tmp = TempRoot::new();
    fs::create_dir(tmp.path().join("levels")).unwrap();
    let mut script = start();
    script.push(read_levels(ORIGINAL));
    let error = run_edit::<()>(script, vec![], &AllowAll, tmp.path(), |_| {
        panic!("backup failed")
    })
    .unwrap_err();
    assert!(error.contains("already exists"));
    assert_eq!(fs::read_dir(tmp.path().join("levels")).unwrap().count(), 0);
}

#[test]
fn cancellation_and_failed_set_both_restore_the_original() {
    for reply in [CHANGE, ORIGINAL] {
        let tmp = TempRoot::new();
        let mut script = start();
        script.extend([
            read_levels(ORIGINAL),
            write_levels(CHANGE),
            read_levels(reply),
            write_levels(ORIGINAL),
            read_levels(ORIGINAL),
        ]);
        let error = run_edit(script, vec![], &AllowAll, tmp.path(), |edit| {
            edit.set_ram(CHANGE)?;
            Err::<(), _>("Cancelled".into())
        })
        .unwrap_err();
        assert!(error.contains(if reply == CHANGE {
            "Cancelled"
        } else {
            "differs"
        }));
    }
}

#[test]
fn partial_ram_send_terminates_before_restore_and_restore_failure_is_reported() {
    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        write_levels(CHANGE),
        (vec![0xf7], vec![]),
        write_levels(ORIGINAL),
        read_levels(CHANGE),
    ]);
    let error = run_edit(script, vec![4, 9], &AllowAll, tmp.path(), |edit| {
        edit.set_ram(CHANGE)
    })
    .unwrap_err();
    assert!(error.contains("simulated write failure"));
    assert!(error.contains("RAM restore could not be verified"));
    assert!(error.contains("release failed"));
}

#[test]
fn one_store_request_after_verified_ram_even_if_caller_tries_twice() {
    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        write_levels(CHANGE),
        read_levels(CHANGE),
        read_levels(CHANGE),
        (STORE_BYTES.to_vec(), vec![]),
    ]);
    let sent = run_edit(script, vec![], &AllowAll, tmp.path(), |edit| {
        edit.set_ram(CHANGE)?;
        let sent = edit.request_store_on()?;
        assert!(edit.request_store_on().is_err());
        assert!(edit.restore_ram().is_err());
        Ok(sent)
    })
    .unwrap();
    assert_eq!(sent.ram_before_request, CHANGE);
}

#[test]
fn wrong_ram_before_store_refuses_and_restores() {
    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        write_levels(CHANGE),
        read_levels(CHANGE),
        read_levels(ORIGINAL),
        write_levels(ORIGINAL),
        read_levels(ORIGINAL),
    ]);
    let error = run_edit(script, vec![], &AllowAll, tmp.path(), |edit| {
        edit.set_ram(CHANGE)?;
        edit.request_store_on()
    })
    .unwrap_err();
    assert!(error.contains("No store request was sent"));
}

#[test]
fn ambiguous_store_send_errors_even_if_callback_ignores_it_and_never_resends() {
    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        read_levels(ORIGINAL),
        (STORE_BYTES.to_vec(), vec![]),
    ]);
    let error = run_edit(script, vec![5], &AllowAll, tmp.path(), |edit| {
        assert!(edit.request_store_on().is_err());
        assert!(edit.request_store_on().is_err());
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("delivery is uncertain"));
}

#[test]
fn denying_the_store_policy_still_allows_ram_restoration() {
    struct NoStore;
    impl GatePolicy for NoStore {
        fn allows(&self, operation: Operation) -> bool {
            operation != STORE
        }
    }

    let tmp = TempRoot::new();
    let mut script = start();
    script.extend([
        read_levels(ORIGINAL),
        write_levels(CHANGE),
        read_levels(CHANGE),
        write_levels(ORIGINAL),
        read_levels(ORIGINAL),
    ]);
    run_edit(script, vec![], &NoStore, tmp.path(), |edit| {
        edit.set_ram(CHANGE)?;
        assert!(
            edit.request_store_on()
                .unwrap_err()
                .contains("VariaxStoreRestore")
        );
        Ok(())
    })
    .unwrap();
}

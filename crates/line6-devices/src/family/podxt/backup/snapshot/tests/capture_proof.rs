use super::super::{fresh::capture_fresh, macos_usb::capture_fresh_usb};
use super::*;
use crate::{
    family::podxt::PodxtRequest,
    gate::{GatePolicy, Operation},
    session::{
        Link, Path as DevicePath, Quiet,
        testing::{Fake, packets},
    },
};
use std::{cell::Cell, time::Duration};

#[derive(Default)]
struct Trace {
    held: Cell<bool>,
    cancelled: Cell<bool>,
    opens: Cell<usize>,
    writes: Cell<usize>,
    reads: Cell<usize>,
    finalized: Cell<usize>,
}

struct FakeRigLock<'a>(&'a Trace);

impl Drop for FakeRigLock<'_> {
    fn drop(&mut self) {
        assert!(self.0.held.replace(false));
        assert_eq!(self.0.finalized.get(), 1);
    }
}

struct ScopedFake<'a> {
    fake: Fake,
    lock: FakeRigLock<'a>,
    fail_finish: bool,
}

impl<'a> ScopedFake<'a> {
    fn open(
        trace: &'a Trace,
        script: Vec<(PodxtRequest, Vec<Vec<u8>>)>,
        fail_finish: bool,
    ) -> Self {
        assert!(!trace.held.replace(true));
        assert_eq!(trace.opens.replace(1), 0);
        Self {
            fake: Fake::new(DevicePath::PodxtMidi, script),
            lock: FakeRigLock(trace),
            fail_finish,
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        assert!(self.lock.0.held.get());
        assert_eq!(self.lock.0.finalized.replace(1), 0);
        self.fake.assert_finished();
        if self.fail_finish {
            Err("synthetic release failure".into())
        } else {
            Ok(())
        }
    }
}

impl Link for ScopedFake<'_> {
    fn path(&self) -> DevicePath {
        self.fake.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        assert!(self.lock.0.held.get());
        assert_eq!(self.lock.0.finalized.get(), 0);
        self.lock.0.writes.set(self.lock.0.writes.get() + 1);
        self.fake.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        assert!(self.lock.0.held.get());
        self.lock.0.reads.set(self.lock.0.reads.get() + 1);
        self.fake.read(wait)
    }

    fn elapsed(&self) -> Duration {
        self.fake.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.fake.pause(duration);
    }
}

struct ReadsOnly;

impl GatePolicy for ReadsOnly {
    fn allows(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::PodxtIdentity | Operation::PodxtRead | Operation::PodxtLibraryRead
        )
    }
}

fn script(library: &Library) -> Vec<(PodxtRequest, Vec<Vec<u8>>)> {
    let mut events = vec![
        (PodxtRequest::Identity, packets(&library.identity_reply)),
        (PodxtRequest::EditBuffer, packets(&library.before.raw)),
    ];
    events.extend(library.entries.iter().map(|entry| {
        (
            PodxtRequest::Stored(entry.location),
            packets(&entry.tone.raw),
        )
    }));
    events.push((
        PodxtRequest::EditBuffer,
        library
            .after
            .as_ref()
            .map_or_else(|_| Vec::new(), |after| packets(&after.raw)),
    ));
    events
}

#[test]
fn explicit_import_preserves_all_banks_while_snapshot_proof_requires_playing_tone_and_cleanup() {
    use crate::backup::load_podxt_library;
    use crate::family::podxt::verify_tone_backup;

    let root = FixtureRoot::new();
    let library = capture();
    let assert_import = |path: &Path, omitted: Option<&str>| {
        // These are the two checks used by the editor's explicit backup import.
        verify_tone_backup(path, Location::new(BankKind::Tones, 0).unwrap()).unwrap();
        let imported = load_podxt_library(path).unwrap();
        assert_eq!(
            imported.before.as_ref(),
            (omitted != Some(BEFORE)).then_some(&library.before)
        );
        assert_eq!(
            imported.after.as_ref(),
            (omitted != Some(AFTER)).then_some(library.after.as_ref().unwrap())
        );
        for (kind, count) in BankKind::ALL.into_iter().zip([128, 107, 64]) {
            assert_eq!(
                imported
                    .entries
                    .iter()
                    .filter(|entry| entry.location.kind() == kind)
                    .count(),
                count
            );
        }
        for (loaded, original) in imported.entries.iter().zip(&library.entries) {
            assert_eq!(loaded.location, original.location);
            assert_eq!(
                loaded.tone,
                original.tone,
                "address {}",
                loaded.location.address()
            );
        }
    };

    for omitted in [BEFORE, AFTER] {
        let legacy = fixture(&root, omitted, &library, true, |key, bytes| {
            (key != omitted).then(|| bytes.to_vec())
        });
        assert_import(&legacy, Some(omitted));
        assert_eq!(
            validation_error(&legacy),
            format!("PODxt snapshot manifest is missing {omitted}.")
        );
    }

    let complete = fixture(&root, "complete", &library, true, |_, bytes| {
        Some(bytes.to_vec())
    });
    assert_import(&complete, None);
    let report = validate_snapshot(&complete, &identity()).unwrap();
    report.check_capture_matches(&library).unwrap();

    let finalize = |trace, fail_finish| {
        capture_fresh_usb(
            || Ok(()),
            || Ok(ScopedFake::open(trace, script(&library), fail_finish)),
            ScopedFake::finish,
            &ReadsOnly,
            &mut Quiet,
            |_| {},
        )
    };
    let failed = Trace::default();
    assert!(matches!(
        finalize(&failed, true),
        Err(error) if error.contains("link cleanup failed: synthetic release failure")
    ));
    assert_eq!(failed.writes.get(), 302);
    assert_eq!(failed.finalized.get(), 1);

    let successful = Trace::default();
    let receipt = finalize(&successful, false).unwrap().bind(&report).unwrap();
    assert_eq!(successful.finalized.get(), 1);
    assert_eq!(
        receipt.manifest_sha256(),
        digest(&fs::read(complete.join(MANIFEST)).unwrap())
    );
    assert_import(&complete, None);
}

#[test]
fn complete_capture_finalizes_its_link_under_lock_and_binds_one_manifest() {
    let root = FixtureRoot::new();
    let library = capture();
    let trace = Trace::default();
    let verified = Cell::new(false);
    let mut slots = Vec::new();
    let finalized = capture_fresh_usb(
        || {
            assert_eq!(trace.opens.get(), 0);
            verified.set(true);
            Ok(())
        },
        || {
            assert!(verified.get());
            Ok(ScopedFake::open(&trace, script(&library), false))
        },
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |entry| {
            assert!(trace.held.get());
            slots.push(entry.location);
        },
    )
    .unwrap();
    assert_eq!(slots, Location::all(&BankKind::ALL).collect::<Vec<_>>());
    assert_eq!(trace.writes.get(), 302);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());

    let mut folder = BackupFolder::create(&root.0, "finalized").unwrap();
    super::super::super::save_podxt_library(&mut folder, finalized.library()).unwrap();
    let path = folder.finish(true).unwrap();
    let report = validate_snapshot(&path, &identity()).unwrap();
    let receipt = finalized.bind(&report).unwrap();
    assert_eq!(receipt.manifest_sha256(), report.manifest_sha256());
}

#[test]
fn successful_cleanup_from_a_different_capture_cannot_bind_a_mismatched_report() {
    let root = FixtureRoot::new();
    let first = capture();
    let mut second = capture();
    second.entries[0].tone.raw[39 + 13] = 127;
    let path = fixture(&root, "first-capture", &first, true, |_, bytes| {
        Some(bytes.to_vec())
    });
    let report = validate_snapshot(&path, &identity()).unwrap();

    let trace = Trace::default();
    let finalized = capture_fresh_usb(
        || Ok(()),
        || Ok(ScopedFake::open(&trace, script(&second), false)),
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |_| {},
    )
    .unwrap();
    assert_eq!(trace.finalized.get(), 1);
    assert!(
        finalized
            .bind(&report)
            .err()
            .unwrap()
            .contains("address 0 differs")
    );
}

#[test]
fn failed_finalization_issues_no_proof_for_a_complete_sweep() {
    let trace = Trace::default();
    let result = capture_fresh_usb(
        || Ok(()),
        || Ok(ScopedFake::open(&trace, script(&capture()), true)),
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |_| {},
    );
    assert!(result.err().unwrap().contains("link cleanup failed"));
    assert_eq!(trace.writes.get(), 302);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

#[test]
fn failed_read_still_finalizes_and_preserves_both_errors() {
    let trace = Trace::default();
    let result = capture_fresh_usb(
        || Ok(()),
        || {
            let mut link = ScopedFake::open(&trace, vec![(PodxtRequest::Identity, vec![])], true);
            link.fake.fail_at.push(0);
            Ok(link)
        },
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |_| panic!("identity read failed"),
    );
    let error = result.err().unwrap();
    assert!(error.contains("simulated write failure"), "{error}");
    assert!(error.contains("cleanup also failed"), "{error}");
    assert_eq!(trace.writes.get(), 1);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

#[test]
fn cancellation_still_finalizes_under_lock_and_issues_no_proof() {
    let library = capture();
    for (cancel_after, fail_finish) in [(1, false), (1, true), (library.entries.len(), false)] {
        let trace = Trace::default();
        let mut replies = script(&library);
        replies.truncate(cancel_after + 2); // Identity, opening tone, then addressed reads.
        let mut slots_read = 0;
        let result = capture_fresh(
            || Ok(ScopedFake::open(&trace, replies, fail_finish)),
            |mut link| link.finish(),
            &|| trace.cancelled.get(),
            &ReadsOnly,
            &mut Quiet,
            |_| {
                slots_read += 1;
                if slots_read == cancel_after {
                    trace.cancelled.set(true);
                }
            },
        );

        let error = result.err().unwrap();
        assert!(error.contains("PODxt capture cancelled"), "{error}");
        assert_eq!(error.contains("synthetic release failure"), fail_finish);
        assert_eq!(slots_read, cancel_after);
        assert_eq!(trace.writes.get(), cancel_after + 2);
        assert_eq!(trace.finalized.get(), 1);
        assert!(!trace.held.get());
    }
}

#[test]
fn cancelled_before_open_or_during_successful_close_issues_no_proof() {
    let result = capture_fresh(
        || panic!("a cancelled capture must not open a link"),
        |_: ScopedFake<'_>| panic!("there is no link to close"),
        &|| true,
        &ReadsOnly,
        &mut Quiet,
        |_| panic!("a cancelled capture cannot read a slot"),
    );
    assert_eq!(result.err().as_deref(), Some("PODxt capture cancelled."));

    let trace = Trace::default();
    let result = capture_fresh(
        || Ok(ScopedFake::open(&trace, script(&capture()), false)),
        |mut link| {
            let result = link.finish();
            trace.cancelled.set(true);
            result
        },
        &|| trace.cancelled.get(),
        &ReadsOnly,
        &mut Quiet,
        |_| {},
    );
    assert_eq!(result.err().as_deref(), Some("PODxt capture cancelled."));
    assert_eq!(trace.writes.get(), 302);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

#[test]
fn closing_read_failure_issues_no_proof_and_preserves_a_close_error() {
    let mut library = capture();
    library.after = Err("missing closing dump".into());
    for fail_finish in [false, true] {
        let trace = Trace::default();
        let result = capture_fresh(
            || Ok(ScopedFake::open(&trace, script(&library), fail_finish)),
            |mut link| link.finish(),
            &|| false,
            &ReadsOnly,
            &mut Quiet,
            |_| {},
        );
        let error = result.err().unwrap();
        assert!(error.contains("closing read failed"), "{error}");
        assert_eq!(error.contains("synthetic release failure"), fail_finish);
        assert_eq!(trace.writes.get(), 302);
        assert_eq!(trace.finalized.get(), 1);
        assert!(!trace.held.get());
    }
}

#[test]
fn missing_stored_dump_issues_no_proof_and_preserves_a_close_error() {
    let trace = Trace::default();
    let mut library = capture();
    library.entries.truncate(1);
    let mut replies = script(&library);
    replies[2].1.clear();
    let result = capture_fresh(
        || Ok(ScopedFake::open(&trace, replies, true)),
        |mut link| link.finish(),
        &|| false,
        &ReadsOnly,
        &mut Quiet,
        |_| panic!("the stored dump is missing"),
    );
    let error = result.err().unwrap();
    assert!(error.contains("stopped before completion"), "{error}");
    assert!(error.contains("no 168-byte dump"), "{error}");
    assert!(error.contains("synthetic release failure"), "{error}");
    assert_eq!(trace.writes.get(), 4);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

#[test]
fn any_denied_sweep_operation_prevents_open() {
    struct Denied(Operation);

    impl GatePolicy for Denied {
        fn allows(&self, operation: Operation) -> bool {
            ReadsOnly.allows(operation) && operation != self.0
        }
    }

    for operation in [
        Operation::PodxtIdentity,
        Operation::PodxtRead,
        Operation::PodxtLibraryRead,
    ] {
        let result = capture_fresh_usb(
            || panic!("a denied capture must not check the registry"),
            || panic!("a denied capture must not open a link"),
            ScopedFake::finish,
            &Denied(operation),
            &mut Quiet,
            |_| panic!("a denied capture cannot read a slot"),
        );
        let error = result.err().unwrap();
        assert!(error.contains(&format!("{operation:?}")), "{error}");
        assert!(error.contains("does not allow"), "{error}");
    }
}

#[test]
fn failed_open_issues_no_proof() {
    let result = capture_fresh_usb(
        || Ok(()),
        || Err("synthetic attachment changed".into()),
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |_| panic!("a failed open cannot read a slot"),
    );
    assert_eq!(
        result.err().as_deref(),
        Some("synthetic attachment changed")
    );
}

#[test]
fn replaced_registry_generation_refuses_open_at_reused_path_and_address() {
    let trace = Trace::default();
    let selected = ("1-4.3", 3, 11);
    let replacement = ("1-4.3", 3, 12);
    let changed = "synthetic registry generation changed";
    let result = capture_fresh_usb(
        || {
            if replacement == selected {
                Ok(())
            } else {
                Err(changed.into())
            }
        },
        || Ok(ScopedFake::open(&trace, script(&capture()), false)),
        ScopedFake::finish,
        &ReadsOnly,
        &mut Quiet,
        |_| panic!("a replaced attachment cannot read a slot"),
    );
    assert_eq!(result.err().as_deref(), Some(changed));
    assert_eq!(trace.opens.get(), 0);
    assert_eq!(trace.writes.get(), 0);
    assert_eq!(trace.reads.get(), 0);
    assert_eq!(trace.finalized.get(), 0);
}

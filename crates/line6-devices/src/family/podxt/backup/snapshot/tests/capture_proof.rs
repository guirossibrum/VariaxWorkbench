use super::*;
use crate::{
    family::podxt::PodxtRequest,
    gate::{AllClosed, GatePolicy, Operation},
    session::{
        Link, Path as DevicePath, Quiet,
        testing::{Fake, packets},
    },
};
use std::{cell::Cell, time::Duration};

#[derive(Default)]
struct Trace {
    held: Cell<bool>,
    writes: Cell<usize>,
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
        Self {
            fake: Fake::new(DevicePath::PodxtMidi, script),
            lock: FakeRigLock(trace),
            fail_finish,
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
        self.fake.read(wait)
    }

    fn elapsed(&self) -> Duration {
        self.fake.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.fake.pause(duration);
    }
}

impl super::super::provenance::sealed::Sealed for ScopedFake<'_> {}

impl CaptureLink for ScopedFake<'_> {
    fn finish_capture(self) -> Result<(), String> {
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
fn complete_capture_finalizes_its_link_under_lock_and_binds_one_manifest() {
    let root = FixtureRoot::new();
    let library = capture();
    let trace = Trace::default();
    let mut slots = Vec::new();
    let finalized = capture_library(
        ScopedFake::open(&trace, script(&library), false),
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
    let finalized = capture_library(
        ScopedFake::open(&trace, script(&second), false),
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
    let result = capture_library(
        ScopedFake::open(&trace, script(&capture()), true),
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
fn denied_read_still_finalizes_and_preserves_both_errors() {
    let trace = Trace::default();
    let result = capture_library(
        ScopedFake::open(&trace, vec![], true),
        &AllClosed,
        &mut Quiet,
        |_| panic!("the policy denies every request"),
    );
    let error = result.err().unwrap();
    assert!(error.contains("does not allow"), "{error}");
    assert!(error.contains("cleanup also failed"), "{error}");
    assert_eq!(trace.writes.get(), 0);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

#[test]
fn closing_read_failure_with_successful_cleanup_issues_no_proof() {
    let trace = Trace::default();
    let mut library = capture();
    library.after = Err("missing closing dump".into());
    let result = capture_library(
        ScopedFake::open(&trace, script(&library), false),
        &ReadsOnly,
        &mut Quiet,
        |_| {},
    );
    assert!(result.err().unwrap().contains("closing read failed"));
    assert_eq!(trace.writes.get(), 302);
    assert_eq!(trace.finalized.get(), 1);
    assert!(!trace.held.get());
}

use super::super::{
    PROGRAM_BYTES,
    tests::{Script, bank, blocks, end, metadata, select, start},
};
use super::*;
use crate::{
    family::variax::{procedures::set_firmware_checksum, span_read::PresetSpan, tests::bank_reply},
    gate::AllClosed,
    identity::tests::{PODXT_LIVE, VARIAX_600},
    session::{
        Quiet, Request,
        testing::{AllowAll, Fake, packets},
    },
};
use std::{cell::Cell, fs, rc::Rc};

const START_PROGRAM: u8 = 85;
const METADATA_OFFSET: usize = 851 * 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    AmbiguousStart,
    OriginalMismatch,
    DuplicateSlot,
    WrongGuitarReply,
    WrongRelay,
    ReplacedAtOpen,
    ReplacedDuringRead,
    ReplacedAtClose,
    Malformed,
    Cancelled,
    CancelledAtClose,
    UnverifiedReturn,
    WrongReturnedSlot,
    SessionOff,
    BridgeOff,
    Close,
    InvalidProgram,
    WrongProgram,
}

#[derive(Default)]
struct Events {
    opened: Cell<usize>,
    closed: Cell<usize>,
    positions: Cell<usize>,
    pcs: Cell<usize>,
    cancelled: Cell<bool>,
}

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("variax-capture-{label}-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn tuple(generation: u64) -> AttachmentIdentity<u64> {
    let mut guitar = VARIAX_600;
    guitar[12..16].copy_from_slice(b"0310");
    let mut relay = PODXT_LIVE;
    relay[12..16].copy_from_slice(&[0, 3, 0, 1]);
    AttachmentIdentity {
        generation,
        guitar: Identity::parse(&guitar).unwrap(),
        relay: Identity::parse(&relay).unwrap(),
    }
}

fn position() -> Preset {
    Preset::all()
        .find(|preset| preset.program_number() == START_PROGRAM)
        .unwrap()
}

struct Opener {
    lock_path: PathBuf,
    script: Script,
    events: Rc<Events>,
    fault: Fault,
}

struct ScopedFake {
    wire: Fake,
    lock_path: PathBuf,
    events: Rc<Events>,
    fault: Fault,
    handle: u64,
}

impl sealed::Sealed for Opener {}
impl sealed::Sealed for ScopedFake {}

impl CaptureAttachment for Opener {
    type Link = ScopedFake;

    fn open(
        self,
        _: &DeviceLock,
        selected: &AttachmentIdentity<u64>,
    ) -> Result<ScopedFake, String> {
        assert_locked(&self.lock_path);
        assert_eq!(selected.generation, 10);
        self.events.opened.set(self.events.opened.get() + 1);
        let total_writes = self.script.len();
        let mut wire = Fake::from_bytes(Path::VariaxViaPodxt, self.script);
        match self.fault {
            Fault::SessionOff => wire.fail_at.push(total_writes - 2),
            Fault::BridgeOff => wire.fail_at.push(total_writes - 1),
            _ => {}
        }
        Ok(ScopedFake {
            wire,
            lock_path: self.lock_path,
            events: self.events,
            fault: self.fault,
            handle: 27,
        })
    }
}

fn assert_locked(path: &FilePath) {
    assert!(matches!(
        fs::File::open(path).unwrap().try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
}

impl CaptureLink for ScopedFake {
    type Generation = u64;

    fn observe_attachment(&mut self) -> Result<AttachmentIdentity<u64>, String> {
        assert_locked(&self.lock_path);
        let mut observed = tuple(10);
        match self.fault {
            Fault::ReplacedAtOpen => observed.generation = 11,
            Fault::ReplacedDuringRead if self.wire.writes.len() >= 6 => observed.generation = 11,
            Fault::WrongRelay => observed.relay.version = [0, 3, 0, 2],
            _ => {}
        }
        Ok(observed)
    }

    fn observe_clean_position(&mut self) -> Result<CleanPosition, String> {
        assert_locked(&self.lock_path);
        let reads = self.events.positions.get();
        self.events.positions.set(reads + 1);
        if self.fault == Fault::AmbiguousStart {
            return Err("Ambiguous active Variax slot.".into());
        }
        let preset = if reads == 1 && self.fault == Fault::WrongReturnedSlot {
            Preset::all().next().unwrap()
        } else {
            position()
        };
        Ok(CleanPosition {
            preset,
            stored_metadata: PresetMetadata::from_bytes(metadata(START_PROGRAM)).unwrap(),
        })
    }

    fn finish_capture(self, selected: &AttachmentIdentity<u64>) -> Result<(), String> {
        assert_locked(&self.lock_path);
        assert_eq!(self.handle, 27);
        assert_eq!(selected.generation, 10);
        self.wire.assert_finished();
        self.events.closed.set(self.events.closed.get() + 1);
        match self.fault {
            Fault::ReplacedAtClose => Err("Attachment replaced during release.".into()),
            Fault::Close => Err("Driver did not return.".into()),
            Fault::CancelledAtClose => {
                self.events.cancelled.set(true);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl Link for ScopedFake {
    fn path(&self) -> Path {
        self.wire.path()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.starts_with(&[0xc0]) {
            self.events.pcs.set(self.events.pcs.get() + 1);
        }
        self.wire.write(bytes)
    }
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        self.wire.read(wait)
    }
    fn elapsed(&self) -> Duration {
        self.wire.elapsed()
    }
    fn pause(&mut self, duration: Duration) {
        self.wire.pause(duration)
    }
}

fn program_chunks(script: &mut Script, data: &[u8; 192], fault: Fault) {
    let mut image = [0; PROGRAM_BYTES];
    image[METADATA_OFFSET..][..data.len()].copy_from_slice(data);
    if fault == Fault::WrongProgram {
        image[METADATA_OFFSET] ^= 1;
    }
    set_firmware_checksum(&mut image);
    if fault == Fault::InvalidProgram {
        image[0] ^= 1;
    }
    for (chunk, words) in image.as_chunks::<3>().0.chunks(32).enumerate() {
        let start = (chunk * 32) as u16;
        let span = PresetSpan::new(start, words.len() as u16).unwrap();
        let mut reply = vec![0xf0, 0, 1, 0x0c, 7, 0, 0x3c];
        for word in words {
            for byte in word.iter().rev() {
                reply.extend([byte & 15, byte >> 4]);
            }
        }
        reply.push(0xf7);
        script.push((span.bytes(), packets(&reply)));
    }
}

fn script(coverage: SnapshotCoverage, fault: Fault) -> Script {
    if matches!(
        fault,
        Fault::AmbiguousStart | Fault::WrongRelay | Fault::ReplacedAtOpen
    ) {
        return vec![];
    }
    let original = if fault == Fault::OriginalMismatch {
        3
    } else {
        START_PROGRAM
    };
    let mut script = start(original);
    let mut identity = VARIAX_600;
    if fault == Fault::WrongGuitarReply {
        script.truncate(1);
        return script;
    }
    identity[12..16].copy_from_slice(b"0310");
    script[0].1 = packets(&identity);
    if fault == Fault::OriginalMismatch {
        end(&mut script);
        return script;
    }
    for preset in Preset::all() {
        let program = preset.program_number();
        if fault == Fault::ReplacedDuringRead {
            script.push((vec![0xc0, program], vec![]));
            return script;
        }
        let key = if program == 3 && fault == Fault::DuplicateSlot {
            START_PROGRAM
        } else {
            program
        };
        let mut data = metadata(key);
        if fault == Fault::Malformed {
            data[0] = 0xff;
        }
        select(&mut script, program, key);
        if fault == Fault::Malformed {
            script.truncate(script.len() - 2);
            blocks(&mut script, &data);
            break;
        }
        if coverage == SnapshotCoverage::FullPrograms {
            let program_fault = if program == 4 { fault } else { Fault::None };
            program_chunks(&mut script, &data, program_fault);
            if program_fault == Fault::InvalidProgram {
                break;
            }
        }
        if fault == Fault::Cancelled {
            break;
        }
        if preset.slot() == 4 {
            bank(
                &mut script,
                packets(&bank_reply(format!("Bank {}", preset.bank()).as_bytes())),
            );
        }
    }
    let returned = if fault == Fault::UnverifiedReturn {
        3
    } else {
        START_PROGRAM
    };
    select(&mut script, START_PROGRAM, returned);
    end(&mut script);
    script
}

fn run(
    root: &TestRoot,
    coverage: SnapshotCoverage,
    fault: Fault,
    policy: &dyn GatePolicy,
) -> (Result<FinalizedCapture, String>, Rc<Events>) {
    let lock_path = root.0.join("rig.lock");
    let rig = DeviceLock::acquire_test_lock(&lock_path).unwrap();
    let events = Rc::new(Events::default());
    let selected = SelectedCapture {
        profile: "music-room".into(),
        attachment: tuple(10),
        opener: Opener {
            lock_path: lock_path.clone(),
            script: script(coverage, fault),
            events: events.clone(),
            fault,
        },
        rig,
    };
    let result = capture_library(
        selected,
        coverage,
        policy,
        &mut Quiet,
        || events.cancelled.get(),
        |_| {
            if fault == Fault::Cancelled {
                events.cancelled.set(true);
            }
        },
    );
    assert!(DeviceLock::acquire_test_lock(&lock_path).is_ok());
    (result, events)
}

#[test]
fn seals_both_coverage_modes_after_the_same_link_releases_under_the_rig_lock() {
    let root = TestRoot::new("success");
    for (name, coverage) in [
        ("metadata", SnapshotCoverage::Metadata),
        ("programs", SnapshotCoverage::FullPrograms),
    ] {
        let (result, events) = run(&root, coverage, Fault::None, &AllowAll);
        let capture = result.unwrap();
        assert_eq!(
            (
                events.opened.get(),
                events.closed.get(),
                events.positions.get(),
                events.pcs.get()
            ),
            (1, 1, 2, 61)
        );
        let staged = capture.stage(&root.0, name).unwrap();
        let path = staged.source.clone();
        let expected = tuple(10);
        let report = validate_snapshot(&path, &expected.guitar, coverage).unwrap();
        let receipt = staged.bind().unwrap();
        assert_eq!(receipt.source(), path);
        assert_eq!(receipt.profile(), "music-room");
        assert_eq!(receipt.guitar(), &expected.guitar);
        assert_eq!(receipt.relay(), &expected.relay);
        assert_eq!(receipt.coverage(), coverage);
        assert_eq!(receipt.manifest_sha256(), report.manifest_sha256());
        assert!(receipt.captured_at() <= SystemTime::now());
    }
}

#[test]
fn refuses_an_unknown_start_or_attachment_before_any_program_change() {
    let root = TestRoot::new("before-sweep");
    for (fault, expected) in [
        (Fault::AmbiguousStart, "Ambiguous"),
        (Fault::OriginalMismatch, "starting Variax metadata"),
        (Fault::WrongGuitarReply, "identity reply differs"),
        (Fault::WrongRelay, "attachment changed"),
        (Fault::ReplacedAtOpen, "attachment changed"),
    ] {
        let (result, events) = run(&root, SnapshotCoverage::Metadata, fault, &AllowAll);
        assert!(result.err().unwrap().contains(expected), "{fault:?}");
        assert_eq!(
            (events.opened.get(), events.closed.get(), events.pcs.get()),
            (1, 1, 0)
        );
    }
    let (result, events) = run(&root, SnapshotCoverage::Metadata, Fault::None, &AllClosed);
    assert!(result.is_err());
    assert_eq!(events.opened.get(), 0);
}

#[test]
fn refuses_ambiguous_or_unverified_return_even_when_cleanup_completes() {
    let root = TestRoot::new("return");
    for (fault, expected) in [
        (Fault::DuplicateSlot, "uniquely match"),
        (Fault::UnverifiedReturn, "return could not be verified"),
        (Fault::WrongReturnedSlot, "position differs"),
    ] {
        let (result, events) = run(&root, SnapshotCoverage::Metadata, fault, &AllowAll);
        assert!(result.err().unwrap().contains(expected), "{fault:?}");
        assert_eq!((events.closed.get(), events.pcs.get()), (1, 61));
    }
}

#[test]
fn refuses_replacement_during_capture_and_during_checked_release() {
    let root = TestRoot::new("replacement");
    for (fault, switches, expected) in [
        (Fault::ReplacedDuringRead, 1, "attachment changed"),
        (Fault::ReplacedAtClose, 61, "replaced during release"),
    ] {
        let (result, events) = run(&root, SnapshotCoverage::Metadata, fault, &AllowAll);
        assert!(result.err().unwrap().contains(expected), "{fault:?}");
        assert_eq!((events.closed.get(), events.pcs.get()), (1, switches));
    }
}

#[test]
fn refuses_partial_malformed_or_cancelled_sweeps_and_attempts_return() {
    let root = TestRoot::new("partial");
    for (coverage, fault, expected) in [
        (SnapshotCoverage::Metadata, Fault::Malformed, "non-ASCII"),
        (SnapshotCoverage::Metadata, Fault::Cancelled, "cancelled"),
        (
            SnapshotCoverage::FullPrograms,
            Fault::InvalidProgram,
            "checksum mismatch",
        ),
    ] {
        let (result, events) = run(&root, coverage, fault, &AllowAll);
        assert!(result.err().unwrap().contains(expected), "{fault:?}");
        assert_eq!((events.closed.get(), events.pcs.get()), (1, 2));
    }
    let (result, events) = run(
        &root,
        SnapshotCoverage::Metadata,
        Fault::CancelledAtClose,
        &AllowAll,
    );
    assert!(result.err().unwrap().contains("cancelled"));
    assert_eq!((events.closed.get(), events.pcs.get()), (1, 61));
}

#[test]
fn refuses_checksum_valid_programs_with_different_slot_metadata() {
    let root = TestRoot::new("wrong-program");
    let (result, events) = run(
        &root,
        SnapshotCoverage::FullPrograms,
        Fault::WrongProgram,
        &AllowAll,
    );
    assert!(
        result
            .err()
            .unwrap()
            .contains("program differs from the metadata")
    );
    assert_eq!(events.closed.get(), 1);
}

#[test]
fn refuses_each_cleanup_failure() {
    let root = TestRoot::new("cleanup");
    for (fault, expected) in [
        (Fault::SessionOff, "Session off failed"),
        (Fault::BridgeOff, "Variax-messages release failed"),
        (Fault::Close, "Driver did not return"),
    ] {
        let (result, events) = run(&root, SnapshotCoverage::Metadata, fault, &AllowAll);
        assert!(result.err().unwrap().contains(expected), "{fault:?}");
        assert_eq!((events.closed.get(), events.pcs.get()), (1, 61));
    }
}

#[test]
fn binding_refuses_invalid_manifests_wrong_coverage_and_different_valid_payloads() {
    let root = TestRoot::new("binding");
    let fresh = |coverage| run(&root, coverage, Fault::None, &AllowAll).0.unwrap();
    let metadata = fresh(SnapshotCoverage::Metadata);
    assert!(metadata.stage(&root.0, "../outside").is_err());

    let staged = fresh(SnapshotCoverage::Metadata)
        .stage(&root.0, "wrong-coverage")
        .unwrap();
    assert!(
        fresh(SnapshotCoverage::Metadata)
            .stage(&root.0, "wrong-coverage")
            .is_err()
    );
    let full = fresh(SnapshotCoverage::FullPrograms)
        .stage(&root.0, "full")
        .unwrap();
    fs::remove_dir_all(&staged.source).unwrap();
    fs::rename(full.source, &staged.source).unwrap();
    assert!(staged.bind().is_err());

    let staged = fresh(SnapshotCoverage::Metadata)
        .stage(&root.0, "invalid")
        .unwrap();
    let manifest = staged.source.join("MANIFEST.sha256");
    fs::remove_file(&manifest).unwrap();
    fs::write(manifest, "invalid\n").unwrap();
    assert!(staged.bind().err().unwrap().contains("Malformed"));

    let staged = fresh(SnapshotCoverage::Metadata)
        .stage(&root.0, "auxiliary")
        .unwrap();
    let manifest = staged.source.join("MANIFEST.sha256");
    let original = fs::read_to_string(&manifest).unwrap();
    // A valid v1 manifest with the exact same wire replies can omit log.txt.
    fs::remove_file(staged.source.join("log.txt")).unwrap();
    fs::remove_file(&manifest).unwrap();
    let trimmed = original
        .lines()
        .filter(|line| !line.ends_with("  log.txt"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(manifest, trimmed).unwrap();
    assert!(
        staged
            .bind()
            .err()
            .unwrap()
            .contains("manifest changed after staging")
    );

    let staged = fresh(SnapshotCoverage::Metadata)
        .stage(&root.0, "original")
        .unwrap();
    let mut folder = BackupFolder::create(&root.0, "different").unwrap();
    let CapturedLibrary::Metadata(mut settings) = fresh(SnapshotCoverage::Metadata).library else {
        unreachable!()
    };
    settings.bank_names[0].1.reply = bank_reply(b"Other valid bank name");
    save_variax_library(&mut folder, &settings).unwrap();
    let other = folder.finish(true).unwrap();
    assert!(validate_snapshot(&other, &tuple(10).guitar, SnapshotCoverage::Metadata).is_ok());
    fs::remove_dir_all(&staged.source).unwrap();
    fs::rename(other, &staged.source).unwrap();
    assert!(
        staged
            .bind()
            .err()
            .unwrap()
            .contains("differs from the Variax capture")
    );
}

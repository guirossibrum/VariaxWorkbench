//! #178 rehearsal. All device traffic is scripted and all snapshot bytes are synthetic.

use super::{snapshot::TestRoot, *};
use std::time::Duration;
use variax::{
    procedures::{self, CHUNK_COUNT, Image, METADATA, StoreWait, VerifiedStoreError},
    write::Program,
};

mod fixture;
mod proof;

use fixture::{alter_program, fixture_edit, fixture_image, stopped_script};

const TARGET: Preset = Preset { bank: 0, slot: 0 };
const START: Preset = Preset { bank: 2, slot: 0 };
const EDIT: &str = "STORE EDIT CUSTOM 1 POSITION 1 FLASH 4";
const RESTORE: &str = "RESTORE CUSTOM 1 POSITION 1 FLASH 4";
const SLOT_STORE: &[u8] = &[0x61, 4];
const UPLOAD_HEADER: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x43];

struct RehearsalPolicy;

impl GatePolicy for RehearsalPolicy {
    fn allows(&self, operation: Operation) -> bool {
        OPERATIONS.contains(&operation) || operation == Operation::VariaxStoreRestore
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    MissingProgram,
    DuplicateManifest,
    BadChecksum,
    InvalidRestoreStamp,
    InvalidStartingImage,
    UnrelatedEditByte,
    TargetPreflight,
    PreflightReturn,
    EditConfirmation,
    EditSilent,
    EditUnexpectedReply,
    EditCleanup,
    EditIncompleteReadBack,
    IntermediateReturn,
    IntermediateCleanup,
    UnrelatedSlot,
    UnrelatedSettings,
    UnrelatedBank,
    RestorePreflight,
    RestoreConfirmation,
    RestoreMismatch,
    FinalComparison,
}

struct Capture {
    files: std::path::PathBuf,
    snapshot: ValidatedSnapshot,
}

fn capture(
    session: &mut Session<Fake>,
    root: &TestRoot,
    phase: &str,
    fault: Fault,
) -> Result<Capture, String> {
    let full = read_full_library_in_session(session, || false, |_| {})?;
    let identity = &full.settings.identified.identity;
    let mut folder = BackupFolder::create(&root.0, phase)?;
    full.save(&mut folder)?;
    let files = folder.finish(full.complete())?;
    let files = match fault {
        Fault::MissingProgram | Fault::BadChecksum => snapshot::fixture(
            root,
            "bad-files",
            SnapshotCoverage::FullPrograms,
            |name, mut data| {
                if name == program_file(TARGET) {
                    if fault == Fault::MissingProgram {
                        return None;
                    }
                    data[PROGRAM_BYTES - 1] ^= 1;
                }
                Some(data)
            },
            true,
        ),
        Fault::DuplicateManifest => {
            let invalid = root.0.join("duplicate");
            std::fs::create_dir(&invalid).unwrap();
            let entry = format!("{}  {}\n", "0".repeat(64), program_file(TARGET));
            std::fs::write(invalid.join("MANIFEST.sha256"), entry.repeat(2)).unwrap();
            invalid
        }
        _ => files,
    };
    let snapshot = validate_snapshot(&files, identity, SnapshotCoverage::FullPrograms)?;
    // The fake transport owner supplies the separate link-close result.
    let cleanup = match fault {
        Fault::IntermediateCleanup => Err("synthetic link close failed".into()),
        _ => Ok(()),
    };
    snapshot.check_cache_eligibility(SnapshotCapture::Full(&full), Some(&cleanup))?;
    Ok(Capture { files, snapshot })
}

impl Capture {
    fn programs(&self) -> &[(Preset, Image)] {
        match self.snapshot.programs() {
            SnapshotPrograms::Full(programs) => programs,
            SnapshotPrograms::MetadataOnly => panic!("rehearsal requires validated full coverage"),
        }
    }

    fn image(&self, preset: Preset) -> &Image {
        &self
            .programs()
            .iter()
            .find(|(key, _)| *key == preset)
            .unwrap()
            .1
    }
}

type Counts = (usize, usize, usize);

fn verify_baseline(baseline: &Capture) -> Result<Counts, String> {
    let library = baseline.snapshot.library();
    for (preset, settings) in &library.presets {
        if baseline.image(*preset)[METADATA] != settings.raw {
            return Err(format!(
                "Baseline program and settings disagree: {}",
                variax_preset_stem(*preset)
            ));
        }
    }
    Ok((
        baseline.programs().len(),
        library.presets.len(),
        library.bank_names.len(),
    ))
}

fn compare_library(
    baseline: &Capture,
    observed: &Capture,
    target: &Image,
) -> Result<Counts, String> {
    // Both snapshots passed the validator's complete and unique key-set check.
    let mut counts = (0, 0, 0);
    for (preset, original) in baseline.programs() {
        let expected = if *preset == TARGET { target } else { original };
        let stem = variax_preset_stem(*preset);
        if observed.image(*preset) != expected {
            return Err(format!("Library program differs: {stem}"));
        }
        counts.0 += 1;
        for (index, block) in expected[METADATA].as_chunks::<96>().0.iter().enumerate() {
            let name = format!("{stem}-{}.syx", index + 1);
            let observed = std::fs::read(observed.files.join(&name)).map_err(|e| e.to_string())?;
            let expected = if *preset == TARGET {
                encode_reply(block)
            } else {
                std::fs::read(baseline.files.join(&name)).map_err(|e| e.to_string())?
            };
            if observed != expected {
                return Err(format!("Library settings differ: {name}"));
            }
        }
        counts.1 += 1;
    }
    for (bank, name) in &baseline.snapshot.library().bank_names {
        let other = observed
            .snapshot
            .library()
            .bank_names
            .iter()
            .find(|(key, _)| key == bank)
            .unwrap();
        if other.1.reply != name.reply {
            return Err(format!("Library bank name differs: {bank}"));
        }
        counts.2 += 1;
    }
    Ok(counts)
}

#[derive(Default)]
struct Evidence {
    prompts: Vec<&'static str>,
    captures: Vec<Counts>,
    reads_after_ack: Vec<usize>,
    silent_wait: Option<Duration>,
}

fn verify_target(session: &mut Session<Fake>, image: &Image, phase: &str) -> Result<(), String> {
    let actual = procedures::read_unmodified(session, TARGET, || false, |_| Ok(()))?;
    procedures::compare_image(&actual, image, phase)
}

fn confirm(
    evidence: &mut Evidence,
    expected: &'static str,
    answer: Option<&str>,
) -> Result<(), String> {
    evidence.prompts.push(expected);
    if answer != Some(expected) {
        return Err(format!("Confirmation refused: {expected}"));
    }
    Ok(())
}

fn store(
    session: &mut Session<Fake>,
    evidence: &mut Evidence,
    image: &Program<'_>,
) -> Result<(), String> {
    let start = session.link.elapsed();
    let mut replies = 0;
    let result = procedures::store_and_reload_observing(
        session,
        TARGET,
        image,
        || false,
        |wait| {
            if let StoreWait::Silent { notice } = wait {
                assert!(notice.contains("flash") || notice.contains("slot in 0–19"));
                assert!(notice.contains("spare 60"));
            }
        },
        |_| {
            replies += 1;
            Ok(())
        },
    );
    if matches!(result, Err(VerifiedStoreError::SilentTimeout)) {
        evidence.silent_wait = Some(session.link.elapsed() - start);
    }
    if result.is_ok() {
        evidence.reads_after_ack.push(replies);
    }
    result.map_err(|e| e.to_string())
}

// A test-only orchestration with a concrete Fake link and injected inputs.
fn rehearse(
    session: &mut Session<Fake>,
    root: &TestRoot,
    edit: &Image,
    answers: [Option<&str>; 2],
    fault: Fault,
    evidence: &mut Evidence,
) -> Result<(), String> {
    let starting = procedures::pack(&procedures::read_buffer(session, || false, |_| Ok(()))?);
    let baseline_fault = match fault {
        Fault::MissingProgram | Fault::DuplicateManifest | Fault::BadChecksum => fault,
        _ => Fault::None,
    };
    let baseline = capture(session, root, "baseline", baseline_fault)?;
    let matches: Vec<_> = baseline
        .programs()
        .iter()
        .filter(|(_, image)| *image == starting)
        .collect();
    let [(start, _)] = matches.as_slice() else {
        return Err("Starting image did not uniquely match the baseline".into());
    };
    if *start == TARGET {
        return Err("Start on a preset other than the target".into());
    }
    let restore = baseline.image(TARGET);
    let name = variax::PresetMetadata::from_bytes(restore[METADATA].try_into().unwrap())?.name;
    let recovery = Program::new(&name, restore)?;
    let upload = Program::new(&name, edit)?;
    let edit = proof::verify(&recovery, &upload)?;
    verify_target(session, restore, "Edit preflight")?;
    procedures::return_to_preset(session, *start, &starting, |_| Ok(()))?;
    evidence.captures.push(verify_baseline(&baseline)?);
    confirm(evidence, EDIT, answers[0])?;
    store(session, evidence, &upload)?;

    procedures::return_to_preset(session, *start, &starting, |_| Ok(()))?;
    let intermediate = capture(session, root, "edited", fault)?;
    evidence
        .captures
        .push(compare_library(&baseline, &intermediate, &edit)?);
    verify_target(session, &edit, "Restore preflight")?;
    procedures::return_to_preset(session, *start, &starting, |_| Ok(()))?;
    confirm(evidence, RESTORE, answers[1])?;
    store(session, evidence, &recovery)?;

    procedures::return_to_preset(session, *start, &starting, |_| Ok(()))?;
    let final_backup = capture(session, root, "restored", fault)?;
    evidence
        .captures
        .push(compare_library(&baseline, &final_backup, restore)?);
    procedures::return_to_preset(session, *start, &starting, |_| Ok(()))?;
    Ok(())
}

fn run(fault: Fault, answers: [Option<&str>; 2]) -> (Result<(), String>, Evidence, usize) {
    let mut edit = fixture_edit(&fixture_image(TARGET));
    if fault == Fault::UnrelatedEditByte {
        edit = alter_program(edit);
    }
    let (script, fail_at) = stopped_script(fault, &edit);
    let root = TestRoot::new(&format!(
        "178-{fault:?}-{}",
        answers.iter().flatten().count()
    ));
    let mut fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    fake.fail_at = fail_at;
    let mut quiet = Quiet;
    let mut session = Session::new(fake, &RehearsalPolicy, &mut quiet);
    let mut evidence = Evidence::default();
    let result = rehearse(&mut session, &root, &edit, answers, fault, &mut evidence);
    session.link.assert_finished();
    let stores = session
        .link
        .writes
        .iter()
        .filter(|bytes| bytes.as_slice() == SLOT_STORE)
        .count();
    let uploads = session
        .link
        .writes
        .iter()
        .filter(|bytes| bytes.starts_with(UPLOAD_HEADER))
        .count();
    assert_eq!(uploads, stores, "{fault:?}");
    (result, evidence, stores)
}

const CONFIRMED: [Option<&str>; 2] = [Some(EDIT), Some(RESTORE)];

#[test]
fn custom_restore_success_confirms_each_store_and_compares_three_full_backups() {
    use variax::{ParameterValue, PresetMetadata};
    let original = fixture_image(TARGET);
    let edited = fixture_edit(&original);
    let original = PresetMetadata::from_bytes(original[METADATA].try_into().unwrap()).unwrap();
    let edited = PresetMetadata::from_bytes(edited[METADATA].try_into().unwrap()).unwrap();
    let changed: Vec<_> = original
        .parameters
        .iter()
        .filter_map(|(name, old)| (*old != edited.parameters[name]).then_some(*name))
        .collect();
    assert_eq!(changed, ["trim"]);
    assert_eq!(original.parameters["trim"], ParameterValue::Float(0.0));
    assert_eq!(edited.parameters["trim"], ParameterValue::Float(-6.0));

    let (result, evidence, stores) = run(Fault::None, CONFIRMED);
    result.unwrap();
    assert_eq!(stores, 2);
    assert_eq!(evidence.prompts, [EDIT, RESTORE]);
    assert_eq!(evidence.reads_after_ack, [CHUNK_COUNT; 2]);
    assert_eq!(evidence.captures, [(60, 60, 12); 3]);
    println!(
        "Synthetic Custom 1 rehearsal: confirmations {:?}, stores {stores}, full read-backs {:?}, program/settings/bank comparisons {:?}",
        evidence.prompts, evidence.reads_after_ack, evidence.captures
    );
}

#[test]
fn custom_restore_preflight_refusals_send_no_upload_or_store() {
    use Fault::*;
    for (fault, error) in [
        (MissingProgram, "missing programs/04"),
        (DuplicateManifest, "Duplicate snapshot manifest"),
        (BadChecksum, "Firmware checksum mismatch"),
        (InvalidRestoreStamp, "changes under firmware stamping"),
        (InvalidStartingImage, "did not uniquely match"),
        (UnrelatedEditByte, "unrelated changes"),
        (TargetPreflight, "Edit preflight differs"),
        (PreflightReturn, "Original preset switch-back differs"),
    ] {
        let (result, evidence, stores) = run(fault, CONFIRMED);
        assert!(result.unwrap_err().contains(error), "{fault:?}");
        assert_eq!(stores, 0, "{fault:?}");
        assert!(evidence.prompts.is_empty(), "{fault:?}");
    }
}

#[test]
fn custom_restore_confirmations_are_distinct_and_cannot_be_reused() {
    for (fault, answers, expected_prompts, stores) in [
        (
            Fault::EditConfirmation,
            [None, Some(RESTORE)],
            &([EDIT])[..],
            0,
        ),
        (
            Fault::EditConfirmation,
            [Some(RESTORE), None],
            &([EDIT])[..],
            0,
        ),
        (
            Fault::RestoreConfirmation,
            [Some(EDIT), None],
            &([EDIT, RESTORE])[..],
            1,
        ),
        (
            Fault::RestoreConfirmation,
            [Some(EDIT), Some(EDIT)],
            &([EDIT, RESTORE])[..],
            1,
        ),
    ] {
        let (result, evidence, actual_stores) = run(fault, answers);
        assert!(result.unwrap_err().contains("Confirmation refused"));
        assert_eq!(evidence.prompts, expected_prompts);
        assert_eq!(actual_stores, stores);
    }
}

#[test]
fn custom_restore_never_sends_second_store_after_edit_or_intermediate_fault() {
    use Fault::*;
    for (fault, error) in [
        (EditSilent, "No store reply"),
        (EditUnexpectedReply, "Unexpected Variax store reply"),
        (EditCleanup, "cleanup failed"),
        (EditIncompleteReadBack, "complete 200-byte"),
        (IntermediateReturn, "switch-back was not verified"),
        (IntermediateCleanup, "cleanup were not verified"),
        (UnrelatedSlot, "Library program differs"),
        (UnrelatedSettings, "Library settings differ"),
        (UnrelatedBank, "Library bank name differs"),
        (RestorePreflight, "Restore preflight differs"),
    ] {
        let (result, evidence, stores) = run(fault, CONFIRMED);
        let actual = result.unwrap_err();
        assert!(actual.contains(error), "{fault:?}: {actual}");
        assert_eq!(stores, 1, "{fault:?}");
        assert_eq!(evidence.prompts, [EDIT], "{fault:?}");
        if fault == EditSilent {
            let wait = evidence.silent_wait.unwrap();
            assert!(wait >= Duration::from_secs(135), "{wait:?}");
            assert!(wait < Duration::from_secs(136), "{wait:?}");
        }
    }
}

#[test]
fn custom_restore_mismatch_stops_after_the_second_store() {
    for (fault, error) in [
        (Fault::RestoreMismatch, "Stored target read-back differs"),
        (Fault::FinalComparison, "Library program differs"),
    ] {
        let (result, evidence, stores) = run(fault, CONFIRMED);
        assert!(result.unwrap_err().contains(error), "{fault:?}");
        assert_eq!(stores, 2);
        assert_eq!(evidence.prompts, [EDIT, RESTORE]);
        assert_eq!(evidence.captures, [(60, 60, 12); 2]);
    }
}

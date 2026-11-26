use super::*;
use crate::rig_check::port::{PracticePort, RigPort};
use crate::rig_check::practice::{Faults, SimRig};
use crate::rig_check::redact::Redactor;
use crate::rig_check::report::ReportFile;
use crate::rig_check::runner::{Decision, ReportSink, Runner};
use crate::rig_check::testing::{Script, direct_usb};
use std::cell::RefCell;
use std::rc::Rc;

const FIRST_SAVE: StepId = StepId(5, 1);
const NEXT_SAVE: StepId = StepId(5, 2);
const OLD_FAILURE: &str = LEGACY_DRIVE_FAILURE;

struct Fixture {
    sink: ReportFile,
    dir: tempfile::TempDir,
    data: RunData,
    run: RunRecord,
}

impl Fixture {
    fn new() -> Self {
        Self::from_rig(&SimRig::shared(Faults::default()))
    }

    fn from_rig(rig: &Rc<RefCell<SimRig>>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
        let data = report_data(&sink);
        let (run, data) = crate::rig_check::run_on(
            &mut PracticePort::new(rig.clone()),
            &mut Script::run_all(),
            &mut sink,
            data,
        );
        assert!(run.step(GUITAR_BACKUP).outcome.passed());
        assert!(run.step(POD_BACKUP).outcome.passed());
        assert!(run.step(NEXT_SAVE).outcome.passed());
        Self {
            sink,
            dir,
            data,
            run,
        }
    }

    fn stop_after(&mut self, step: StepId, legacy: bool) {
        for id in order(legacy)
            .into_iter()
            .skip_while(|id| *id != step)
            .skip(1)
        {
            if id != RIG_GUARD {
                *self.run.step_mut(id) = StepRecord::new(id);
            }
        }
        self.run.stopped_at = Some(step);
    }

    fn save(&mut self, legacy: bool) {
        let mut run = self.run.clone();
        if legacy {
            // Synthetic data in the layout of the owner's first Mac package.
            // Old reports used the coarse Variax hash and omitted manifest
            // checksums from the machine-readable list.
            run.catalogue_version = "1.5";
            run.git_commit = Some(LEGACY_BUILD);
            run.steps
                .sort_by_key(|record| order(true).iter().position(|id| *id == record.id));
            for step in &mut run.steps {
                step.checksums
                    .retain(|(label, _)| !label.ends_with(" manifest"));
            }
        }
        self.sink.update(&run).unwrap();
        let name = self.sink.path.file_name().unwrap().to_str().unwrap();
        let mut state = RigState::from_run(&run, &self.data, name);
        if legacy {
            state.variax_match_sha256 = self.data.start_program.as_ref().map(legacy_variax_hash);
        } else {
            state.report_sha256 = self.sink.report_digest();
        }
        recover::write_state(&self.sink.path.with_extension("state"), &state).unwrap();
    }

    fn legacy_failure() -> Self {
        let mut fixture = Self::new();
        fixture.mark_legacy_failure();
        fixture.save(true);
        fixture
    }

    fn mark_legacy_failure(&mut self) {
        self.stop_after(KNOBS, true);
        let step = self.run.step_mut(KNOBS);
        step.title = "Turn the PODxt Live Drive knob".into();
        step.outcome = Outcome::Failed(OLD_FAILURE.into());
        step.evidence = vec!["Synthetic legacy mismatch: observed 92, read 86.".into()];
        step.attempt = Some(2);
        let mut first = step.clone();
        first.attempt = Some(1);
        first.evidence = vec!["Synthetic legacy mismatch: observed 118, read 115.".into()];
        self.run.prior_attempts.push(first);
    }

    fn stopped_owner_pattern(rig: &Rc<RefCell<SimRig>>) -> Self {
        let mut fixture = Self::from_rig(rig);
        let computer_audio = fixture.run.step(StepId(8, 1)).clone();
        fixture.mark_legacy_failure();
        *fixture.run.step_mut(StepId(8, 1)) = computer_audio;
        for id in RUN_ORDER.iter().copied() {
            let definition = catalogue::step(id).unwrap();
            if definition.not_in_this_version.is_some() {
                fixture.run.step_mut(id).outcome =
                    Outcome::NotAvailable("Synthetic audio refusal.".into());
            } else if matches!(definition.risk, Risk::Live | Risk::Save) {
                fixture.run.step_mut(id).outcome =
                    Outcome::SkippedNeeds("read-only from here: step 7.5 failed".into());
            }
        }
        let mut guard = StepRecord::new(RIG_GUARD);
        guard.outcome = Outcome::Stopped(LEGACY_GUARD_STOP.into());
        *fixture.run.step_mut(RIG_GUARD) = guard;
        fixture.run.stopped_at = Some(RIG_GUARD);
        fixture.run.rig_state = LEGACY_CLEAN_RELEASE.into();
        fixture.save(true);
        fixture
    }

    fn continue_run(
        &self,
        plan: Plan,
        rig: &Rc<RefCell<SimRig>>,
        person: &mut Script,
    ) -> (RunRecord, ReportFile) {
        continue_in(self.dir.path(), None, plan, rig, person)
    }
}

fn continue_in(
    out: &Path,
    legacy: Option<PathBuf>,
    plan: Plan,
    rig: &Rc<RefCell<SimRig>>,
    person: &mut dyn Operator,
) -> (RunRecord, ReportFile) {
    continue_with_port(
        out,
        legacy,
        plan,
        &mut PracticePort::new(rig.clone()),
        person,
    )
}

fn continue_with_port(
    out: &Path,
    legacy: Option<PathBuf>,
    plan: Plan,
    port: &mut dyn RigPort,
    person: &mut dyn Operator,
) -> (RunRecord, ReportFile) {
    let mut sink = ReportFile::create(out, Redactor::default()).unwrap();
    let data = report_data(&sink);
    let path = sink.path.with_extension("state");
    let run = crate::rig_check::continue_on(port, person, &mut sink, data, path, plan, legacy).0;
    (run, sink)
}

fn report_data(sink: &ReportFile) -> RunData {
    let stem = sink.path.file_stem().unwrap().to_str().unwrap();
    RunData {
        evidence_dir: Some(sink.path.with_file_name(format!("{stem}-evidence"))),
        ..RunData::default()
    }
}

fn error(path: &Path) -> String {
    open(path).err().expect("expected refusal")
}

fn only_reads(rig: &Rc<RefCell<SimRig>>) {
    let sim = rig.borrow();
    // Allow identity, PODxt current/playing channel/addressed read, and
    // Variax span reads. The fake transcript must contain no edit or store.
    for (family, bytes) in &sim.written {
        if bytes == &line6_devices::identity::REQUEST {
            continue;
        }
        assert!(
            matches!(
                bytes.as_slice(),
                [0xf0, 0, 1, 0x0c, 3, 0x75 | 0x57 | 0x73 | 0x23, ..]
                    | [0xf0, 0, 1, 0x0c, 7, 0, 0x3c | 0x2a, ..]
            ),
            "{family}: {bytes:02x?}",
        );
    }
    assert_eq!(sim.podxt.stores, 0);
    assert_eq!(sim.variax.stores, 0);
}

#[test]
fn migrated_mac_75_retries_the_sweep_without_repeating_either_library_backup() {
    let fixture = Fixture::legacy_failure();
    let source = std::fs::read(&fixture.sink.path).unwrap();
    let checkpoint = fixture.sink.path.with_extension("state");
    let original_checkpoint = std::fs::read(&checkpoint).unwrap();
    let legacy_evidence = fixture.data.evidence_dir.as_ref().unwrap();
    let manifests = || {
        ["podxt-library", "variax-library"].map(|library| {
            std::fs::read(legacy_evidence.join(library).join("MANIFEST.sha256")).unwrap()
        })
    };
    let original_manifests = manifests();
    let visible = tempfile::tempdir().unwrap();
    assert_eq!(
        last_report(fixture.dir.path()).as_ref(),
        Some(&fixture.sink.path)
    );
    let selected = last_mac_report(visible.path(), fixture.dir.path(), Some(Mode::Practice));
    let plan = open(&selected.unwrap()).unwrap();
    assert_eq!(plan.target, KNOBS);
    for id in [POD_BACKUP, GUITAR_BACKUP, PEDAL] {
        assert!(plan.skips(id));
    }
    assert!(!plan.skips(StepId(7, 3)));
    let mut person = Script::run_all();
    // Stop on the first store card, after checking downstream eligibility.
    person.overrides.push((FIRST_SAVE, Decision::Stop));
    let rig = SimRig::shared(Faults::default());
    let (run, sink) = continue_in(
        visible.path(),
        Some(fixture.dir.path().to_owned()),
        plan,
        &rig,
        &mut person,
    );
    assert_eq!(sink.path.parent(), Some(visible.path()));
    assert!(sink.path.with_extension("state").is_file());
    assert!(report_data(&sink).evidence_dir.unwrap().is_dir());
    assert_eq!(std::fs::read(&checkpoint).unwrap(), original_checkpoint);
    assert_eq!(manifests(), original_manifests);
    assert!(run.step(KNOBS).outcome.passed(), "{:?}", run.continuation);
    assert_eq!(run.step(KNOBS).attempt, Some(3));
    assert_eq!(person.cards.first(), Some(&KNOBS));
    assert!(person.cards.contains(&FIRST_SAVE));
    assert!(run.step(RIG_GUARD).outcome.passed());
    for backup in [POD_BACKUP, GUITAR_BACKUP] {
        assert!(run.step(backup).reused && run.step(backup).outcome.passed());
        assert!(!person.cards.contains(&backup));
    }
    {
        let rig = rig.borrow();
        let addressed = rig
            .written
            .iter()
            .filter(|(_, bytes)| bytes.starts_with(&[0xf0, 0, 1, 0x0c, 3, 0x73]))
            .count();
        let guitar_selects = rig
            .written
            .iter()
            .filter(|(family, bytes)| {
                *family == "VariaxViaPodxt" && matches!(bytes.as_slice(), [0xc0, _])
            })
            .count();
        assert!(
            addressed < 20,
            "unexpected PODxt library reads: {addressed}"
        );
        assert!(
            guitar_selects < 25,
            "unexpected Variax library sweep: {guitar_selects}"
        );
    }
    assert_eq!(std::fs::read(&fixture.sink.path).unwrap(), source);
    assert_eq!(sink.last.matches(OLD_FAILURE).count(), 4); // text and JSON for both old attempts
    assert!(sink.last.contains("Source report"));
    assert!(error(&fixture.sink.path).contains("already has a continuation"));
    let next = open(&sink.path).unwrap();
    assert_eq!(next.target, FIRST_SAVE);
    assert!(next.skips(POD_BACKUP) && next.skips(GUITAR_BACKUP));
}

#[test]
fn mac_discovery_uses_both_roots_and_checks_each_local_companion() {
    let legacy = Fixture::legacy_failure();
    let mut visible = Fixture::new();
    visible.stop_after(KNOBS, false);
    visible.save(false);

    let set_modified = |path: &Path, second| {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(second)),
            )
            .unwrap();
    };
    set_modified(&legacy.sink.path, 10);
    set_modified(&visible.sink.path, 20);
    let find = || last_mac_report(visible.dir.path(), legacy.dir.path(), Some(Mode::Practice));
    assert_eq!(find(), Some(visible.sink.path.clone()));
    set_modified(&legacy.sink.path, 30);
    assert_eq!(find(), Some(legacy.sink.path.clone()));
    set_modified(&legacy.sink.path, 10);

    // A newer report paired with a checkpoint from another root must still
    // fail #297 validation, even if both files have the same basename.
    std::fs::copy(
        legacy.sink.path.with_extension("state"),
        visible.sink.path.with_extension("state"),
    )
    .unwrap();
    assert!(open(&visible.sink.path).is_err());
    assert_eq!(find(), Some(legacy.sink.path.clone()));
    assert_eq!(
        last_mac_report(visible.dir.path(), &legacy.dir.path().join("missing"), None),
        Some(visible.sink.path.clone()),
        "retain the diagnostic when no compatible report is found"
    );
}

#[test]
fn another_mac_root_with_an_uncertain_store_requires_manual_verification() {
    let mut legacy = Fixture::new();
    legacy.stop_after(FIRST_SAVE, false);
    legacy.run.finished = false;
    legacy.run.current = Some(FIRST_SAVE);
    *legacy.run.step_mut(FIRST_SAVE) = StepRecord::new(FIRST_SAVE);
    *legacy.run.step_mut(RIG_GUARD) = StepRecord::new(RIG_GUARD);
    legacy.save(false);
    assert!(error(&legacy.sink.path).contains("interrupted during a live edit or store"));
    let state = legacy.sink.path.with_extension("state");
    let original = std::fs::read(&state).unwrap();
    let visible = tempfile::tempdir().unwrap();
    let rig = SimRig::shared(Faults::default());
    let mut person = Script::run_all();
    person.accept = false;
    let result = recover::before_new_run(
        &mut PracticePort::new(rig.clone()),
        &mut person,
        visible.path(),
        Some(legacy.dir.path()),
    );
    assert!(result.is_err());
    assert!(person.notices[0].contains("verify the affected slot"));
    assert!(person.notices[0].contains(&*legacy.sink.path.to_string_lossy()));

    let selected = Fixture::legacy_failure();
    let plan = open(&selected.sink.path).unwrap();
    let result = recover::before_continuation(
        &mut person,
        Mode::Practice,
        &plan,
        selected.dir.path(),
        Some(legacy.dir.path()),
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(state).unwrap(), original);
    assert!(rig.borrow().written.is_empty());
}

#[test]
fn same_root_interrupted_store_blocks_resume_before_revalidation() {
    let mut source = Fixture::new();
    source.stop_after(KNOBS, false);
    source.run.finished = false;
    source.run.current = Some(KNOBS);
    *source.run.step_mut(KNOBS) = StepRecord::new(KNOBS);
    *source.run.step_mut(RIG_GUARD) = StepRecord::new(RIG_GUARD);
    source.save(false);
    let source_state = source.sink.path.with_extension("state");
    let source_bytes = std::fs::read(&source_state).unwrap();
    let plan = open(&source.sink.path).unwrap();
    assert_eq!(plan.target, KNOBS);

    let mut interrupted = ReportFile::create(source.dir.path(), Redactor::default()).unwrap();
    let mut run = source.run.clone();
    run.current = Some(FIRST_SAVE);
    interrupted.update(&run).unwrap();
    let path = interrupted.path.with_extension("state");
    let name = interrupted.path.file_name().unwrap().to_str().unwrap();
    let mut state = RigState::from_run(&run, &source.data, name);
    state.report_sha256 = interrupted.report_digest();
    recover::write_state(&path, &state).unwrap();
    let state_bytes = std::fs::read(&path).unwrap();
    assert!(error(&interrupted.path).contains("interrupted during a live edit or store"));
    assert_eq!(recover::pending(source.dir.path()).len(), 2);

    let output = tempfile::tempdir().unwrap();
    let rig = SimRig::shared(Faults::default());
    let mut person = Script::run_all();
    person.start_decisions = [true, false].into();
    let (run, _) = continue_in(
        output.path(),
        Some(source.dir.path().to_owned()),
        plan,
        &rig,
        &mut person,
    );
    assert!(run.rig_state.contains("pending rig recovery"));
    assert_eq!(person.notices.len(), 2);
    assert!(person.notices[1].contains(&*interrupted.path.to_string_lossy()));
    assert!(person.notices[1].contains("verify the affected slot"));
    assert!(person.cards.is_empty());
    assert!(rig.borrow().written.is_empty());
    assert!(!source.sink.path.with_extension("continuation").exists());
    assert_eq!(std::fs::read(&source_state).unwrap(), source_bytes);
    assert_eq!(std::fs::read(&path).unwrap(), state_bytes);
    assert_eq!(
        std::fs::read_to_string(&interrupted.path).unwrap(),
        interrupted.last
    );

    // With only the harmless interrupted source present, the held source
    // lock is sufficient and revalidation can go ahead.
    std::fs::remove_file(path).unwrap();
    let mut person = Script::run_all();
    person.overrides.push((FIRST_SAVE, Decision::Stop));
    let (run, _) = continue_in(
        output.path(),
        Some(source.dir.path().to_owned()),
        open(&source.sink.path).unwrap(),
        &rig,
        &mut person,
    );
    assert!(run.step(KNOBS).outcome.passed(), "{}", run.rig_state);
    assert_eq!(person.cards.first(), Some(&KNOBS));
    assert!(source.sink.path.with_extension("continuation").is_file());
    assert_eq!(std::fs::read(source_state).unwrap(), source_bytes);
    assert_eq!(
        std::fs::read_to_string(&source.sink.path).unwrap(),
        source.sink.last
    );
}

#[test]
fn linked_model_byte_blocks_resume_at_22_until_the_saved_state_matches() {
    const LINKED_MODEL_PAYLOAD_OFFSET: usize = 159 - 7;
    let initial_rig = SimRig::shared(Faults::default());
    {
        let mut sim = initial_rig.borrow_mut();
        let slot = usize::from(sim.podxt.channel.unwrap());
        sim.podxt.playing[LINKED_MODEL_PAYLOAD_OFFSET] = 20;
        sim.podxt.stored[slot][LINKED_MODEL_PAYLOAD_OFFSET] = 20;
    }
    let mut source = Fixture::from_rig(&initial_rig);
    source.stop_after(GUITAR_START, true);
    source.run.step_mut(GUITAR_START).outcome =
        Outcome::Failed("Synthetic comparison failure".into());
    source.save(true);
    let source_files = [
        source.sink.path.clone(),
        source.sink.path.with_extension("state"),
        source
            .data
            .evidence_dir
            .as_ref()
            .unwrap()
            .join("podxt-start/edit-buffer.bin"),
    ];
    let original = source_files
        .each_ref()
        .map(|path| std::fs::read(path).unwrap());
    let initial_tone = &original[2];
    assert_eq!(initial_tone.len(), 168);
    assert_eq!(initial_tone[159], 20);

    let plan = open(&source.sink.path).unwrap();
    assert_eq!(plan.target, GUITAR_START);
    assert!(plan.skips(POD_BACKUP));
    let rig = SimRig::shared(Faults::default());
    {
        let mut sim = rig.borrow_mut();
        let slot = usize::from(sim.podxt.channel.unwrap());
        sim.podxt.stored[slot][LINKED_MODEL_PAYLOAD_OFFSET] = 20;
        sim.podxt.playing[LINKED_MODEL_PAYLOAD_OFFSET] = 21;
    }
    let mut transcript = Vec::new();
    let mut terminal = super::super::cli::Terminal::new(&b"accept\naccept\n"[..], &mut transcript);
    let (run, sink) = continue_with_port(
        source.dir.path(),
        None,
        plan,
        &mut direct_usb::Port::with_rig(rig.clone()),
        &mut terminal,
    );
    let transcript = String::from_utf8(transcript).unwrap();
    let blocked = run.blocked_resume.as_ref().unwrap();
    assert_eq!((blocked.target, blocked.check), (GUITAR_START, POD_START));
    assert_eq!(blocked.details.len(), 2);
    for evidence in [
        BlockedResume::TITLE,
        "Selected target 3.3",
        "Blocking check 2.2",
        "expected 5B · observed 5B",
        "byte 159 (linked Variax model (CC 120)) · expected 0x14 · actual 0x15",
    ] {
        assert!(sink.last.contains(evidence), "{evidence}");
        assert!(transcript.contains(evidence), "{transcript}");
    }
    let json: serde_json::Value =
        serde_json::from_str(super::super::report::json_block(&sink.last).unwrap()).unwrap();
    assert_eq!(json["blocked_resume"]["target"], "3.3");
    assert_eq!(json["blocked_resume"]["check"], "2.2");
    assert_eq!(json["resume_cleanup"]["status"], "released");
    assert!(matches!(run.resume_cleanup, Some(ResumeCleanup::Released)));
    assert!(recover::pending(source.dir.path()).is_empty());
    assert!(run.continuation_pending);
    assert!(run.steps.iter().all(|step| step.started.is_none()));
    assert_eq!(run.step(GUITAR_START).outcome, Outcome::NotReached);
    assert!(matches!(
        run.step(RIG_GUARD).outcome,
        Outcome::NotAvailable(_)
    ));
    let rechecks = &run.prior_attempts[run.prior_attempts.len() - 2..];
    assert_eq!([rechecks[0].id, rechecks[1].id], [POD_ID, POD_START]);
    assert!(rechecks[0].outcome.passed());
    assert!(matches!(rechecks[1].outcome, Outcome::Failed(_)));
    assert_eq!(rechecks[1].comparisons.len(), 1);
    assert!(!source.sink.path.with_extension("continuation").exists());
    assert!(error(&sink.path).contains("Open its source report"));
    assert_eq!(
        source_files
            .each_ref()
            .map(|path| std::fs::read(path).unwrap()),
        original
    );
    only_reads(&rig);

    // The same source remains usable after the working tone is back in sync.
    let plan = open(&source.sink.path).unwrap();
    rig.borrow_mut().podxt.playing[LINKED_MODEL_PAYLOAD_OFFSET] = 20;
    let mut person = Script::run_all();
    person.overrides.push((GUITAR_BACKUP, Decision::Stop));
    let (run, _) = source.continue_run(plan, &rig, &mut person);
    assert!(run.blocked_resume.is_none(), "{}", run.rig_state);
    assert!(!run.continuation_pending);
    assert!(run.step(POD_BACKUP).reused);
    assert!(run.step(GUITAR_START).outcome.passed(), "{}", run.rig_state);
    assert_eq!(run.step(GUITAR_START).attempt, Some(2));
    assert_eq!(
        person.cards.first_chunk::<2>(),
        Some(&[SETTINGS, GUITAR_START])
    );
    assert_eq!(
        source_files
            .each_ref()
            .map(|path| std::fs::read(path).unwrap()),
        original
    );
}

#[test]
fn failed_usb_release_preserves_manual_recovery_before_any_follow_up_device_access() {
    let mut source = Fixture::new();
    source.stop_after(GUITAR_START, false);
    source.run.step_mut(GUITAR_START).outcome =
        Outcome::Failed("Synthetic comparison failure".into());
    source.save(false);
    let source_paths = [
        source.sink.path.clone(),
        source.sink.path.with_extension("state"),
    ];
    let snapshot = || {
        source_paths
            .each_ref()
            .map(|path| std::fs::read(path).unwrap())
    };
    let original = snapshot();

    let rig = SimRig::shared(Faults::default());
    rig.borrow_mut().podxt.playing[159 - 7] ^= 1;
    let mut failing_port = direct_usb::Port::with_rig(rig.clone());
    failing_port.fail_podxt_release();
    let mut transcript = Vec::new();
    let mut terminal = super::super::cli::Terminal::new(&b"accept\naccept\n"[..], &mut transcript);
    let (run, sink) = continue_with_port(
        source.dir.path(),
        None,
        open(&source.sink.path).unwrap(),
        &mut failing_port,
        &mut terminal,
    );
    assert!(run.finished);
    assert!(run.continuation_pending);
    assert_eq!(run.blocked_resume.as_ref().unwrap().check, POD_START);
    assert_eq!(run.step(GUITAR_START).outcome, Outcome::NotReached);
    let cleanup_error = "PODxt Live interface 0 release failed: simulated USB release error";
    assert_eq!(run.resume_cleanup_error(), Some(cleanup_error));
    assert!(failing_port.direct_usb_release_failed());
    let transcript = String::from_utf8(transcript).unwrap();
    for line in [cleanup_error, VERIFY_USB_CLEANUP] {
        assert!(sink.last.contains(line));
        assert!(transcript.contains(line));
    }
    let json: serde_json::Value =
        serde_json::from_str(super::super::report::json_block(&sink.last).unwrap()).unwrap();
    assert_eq!(json["resume_cleanup"]["status"], "failed");
    assert_eq!(json["resume_cleanup"]["error"], cleanup_error);
    assert!(open(&sink.path).is_err());
    only_reads(&rig);

    let checkpoint = sink.path.with_extension("state");
    let state_bytes = std::fs::read(&checkpoint).unwrap();
    let pending = recover::pending(source.dir.path());
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, checkpoint);
    assert!(pending[0].1.finished);
    assert!(pending[0].1.channel().is_none());
    assert!(pending[0].1.preset().is_none());

    // A fresh port can only proceed after the existing manual confirmation.
    let fresh = SimRig::shared(Faults::default());
    let mut fresh_port = direct_usb::Port::with_rig(fresh.clone());
    for accept in [false, true] {
        let mut person = Script::run_all();
        person.accept = accept;
        let result = recover::before_new_run(&mut fresh_port, &mut person, source.dir.path(), None);
        assert_eq!(result.is_ok(), accept);
        assert_eq!(person.notices.len(), 1);
        assert!(person.notices[0].contains(VERIFY_USB_CLEANUP));
        assert!(person.notices[0].contains(&*sink.path.to_string_lossy()));
        assert!(person.cards.is_empty());
    }
    // Retrying the original report also requires the confirmation before any read.
    let mut person = Script::run_all();
    person.start_decisions = [true, false].into();
    let (retry, _) = source.continue_run(open(&source.sink.path).unwrap(), &fresh, &mut person);
    assert!(retry.rig_state.contains("pending rig recovery"));
    assert!(person.notices[1].contains(VERIFY_USB_CLEANUP));
    assert!(person.cards.is_empty());
    assert!(fresh.borrow().written.is_empty());
    assert_eq!(snapshot(), original);
    assert_eq!(std::fs::read(checkpoint).unwrap(), state_bytes);
    assert_eq!(std::fs::read_to_string(&sink.path).unwrap(), sink.last);
    assert!(!source.sink.path.with_extension("continuation").exists());
}

#[test]
fn mismatched_tone_or_firmware_only_reads_and_preserves_the_source() {
    for (fault, reason) in [(false, "tone differs"), (true, "firmware")] {
        let fixture = Fixture::legacy_failure();
        let plan = open(&fixture.sink.path).unwrap();
        let rig = SimRig::shared(Faults::default());
        if fault {
            rig.borrow_mut().podxt.firmware = [0, 3, 0, 2];
        } else {
            rig.borrow_mut().podxt.playing[32 + 13] ^= 1;
        }
        let mut person = Script::run_all();
        let (run, _) = fixture.continue_run(plan, &rig, &mut person);
        assert!(run.rig_state.contains(reason), "{}", run.rig_state);
        assert!(person.cards.is_empty());
        assert!(open(&fixture.sink.path).is_ok());
        only_reads(&rig);
    }
}

#[test]
fn missing_or_corrupt_evidence_limits_rebuild_to_the_affected_backup() {
    for corrupt in [false, true] {
        let fixture = Fixture::legacy_failure();
        let root = fixture.data.evidence_dir.as_ref().unwrap();
        let backup = if corrupt {
            let path = root.join("variax-library/01-Spank-4.bin");
            let file = std::fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(Result::unwrap)
                .find(|entry| entry.file_name().to_string_lossy().ends_with(".bin"))
                .unwrap()
                .path();
            std::fs::write(file, [1, 2, 3]).unwrap();
            GUITAR_BACKUP
        } else {
            std::fs::rename(root.join("podxt-library"), root.join("moved-podxt-library")).unwrap();
            POD_BACKUP
        };
        let plan = open(&fixture.sink.path).unwrap();
        assert_eq!(plan.target, KNOBS);
        assert_eq!(plan.rebuild.contains(&POD_BACKUP), backup == POD_BACKUP);
        assert_eq!(
            plan.rebuild.contains(&GUITAR_BACKUP),
            backup == GUITAR_BACKUP
        );
        let rig = SimRig::shared(Faults::default());
        let mut person = Script::run_all();
        person.overrides.push((FIRST_SAVE, Decision::Stop));
        let (run, _) = fixture.continue_run(plan, &rig, &mut person);
        assert!(
            run.step(backup).outcome.passed(),
            "{:?}",
            run.step(backup).outcome
        );
        assert!(run.step(KNOBS).outcome.passed(), "{:?}", run.continuation);
        assert_eq!(person.cards.contains(&POD_BACKUP), backup == POD_BACKUP);
        assert_eq!(
            person.cards.contains(&GUITAR_BACKUP),
            backup == GUITAR_BACKUP
        );
    }
}

#[test]
fn incompatible_checkpoint_stopped_guard_and_uncertain_stores_are_refused_offline() {
    let mut fixture = Fixture::new();
    fixture.stop_after(FIRST_SAVE, false);
    fixture.run.current = Some(FIRST_SAVE);
    fixture.run.finished = false;
    fixture.run.step_mut(FIRST_SAVE).outcome = Outcome::NotReached;
    fixture.save(false);
    assert!(error(&fixture.sink.path).contains("interrupted during a live edit or store"));
    fixture.run.current = None;
    fixture.run.finished = true;
    fixture.run.step_mut(FIRST_SAVE).outcome = Outcome::Failed("Synthetic missing receipt.".into());
    fixture.save(false);
    assert!(error(&fixture.sink.path).contains("unresolved outcome"));
    fixture.run.step_mut(RIG_GUARD).outcome = Outcome::Stopped("Synthetic guard stop.".into());
    *fixture.run.step_mut(FIRST_SAVE) = StepRecord::new(FIRST_SAVE);
    fixture.save(false);
    assert!(error(&fixture.sink.path).contains("guard was stopped"));
    fixture.run.catalogue_version = "0.0";
    fixture.save(false);
    assert!(error(&fixture.sink.path).contains("incompatible"));
    fixture.run.catalogue_version = CATALOGUE_VERSION;
    fixture.save(false);
    std::fs::write(
        fixture.sink.path.with_extension("state"),
        "rig-check-state 1\nreport=other.txt\n",
    )
    .unwrap();
    assert!(error(&fixture.sink.path).contains("checkpoint do not match"));
}

#[test]
fn completed_stores_are_reused_without_a_store_card_or_duplicate_command() {
    let mut fixture = Fixture::new();
    fixture.stop_after(FIRST_SAVE, false);
    fixture.save(false);
    let plan = open(&fixture.sink.path).unwrap();
    assert_eq!(plan.target, NEXT_SAVE);
    assert!(plan.skips(FIRST_SAVE));
    let mut person = Script::run_all();
    person.overrides.push((NEXT_SAVE, Decision::Stop));
    let rig = SimRig::shared(Faults::default());
    let (run, _) = fixture.continue_run(plan, &rig, &mut person);
    assert!(run.step(FIRST_SAVE).reused);
    assert_eq!(person.cards.first(), Some(&NEXT_SAVE));
    assert!(!person.cards.contains(&FIRST_SAVE));
    only_reads(&rig);
}

#[test]
fn declining_resume_and_a_checkpoint_write_failure_prevent_device_actions() {
    let fixture = Fixture::legacy_failure();
    let mut person = Script::run_all();
    person.accept = false;
    let rig = SimRig::shared(Faults::default());
    let (_, _) = fixture.continue_run(open(&fixture.sink.path).unwrap(), &rig, &mut person);
    assert!(rig.borrow().written.is_empty());

    struct Broken;
    impl ReportSink for Broken {
        fn update(&mut self, run: &RunRecord) -> Result<(), String> {
            if run.current == Some(POD_ID) {
                Err("synthetic disk failure".into())
            } else {
                Ok(())
            }
        }
    }
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    let mut sink = Broken;
    let mut runner = Runner::new(&mut port, &mut person, &mut sink, RunData::default());
    runner.state_file = Some(fixture.dir.path().join("broken.state"));
    let (run, _) = runner.run();
    assert!(matches!(run.step(POD_ID).outcome, Outcome::NotAvailable(_)));
    assert!(rig.borrow().written.is_empty());
}

#[test]
fn a_running_checkpoint_and_a_stale_preview_are_refused_before_device_access() {
    let mut fixture = Fixture::new();
    fixture.stop_after(KNOBS, false);
    fixture.run.current = Some(KNOBS);
    fixture.run.finished = false;
    *fixture.run.step_mut(KNOBS) = StepRecord::new(KNOBS);
    *fixture.run.step_mut(RIG_GUARD) = StepRecord::new(RIG_GUARD);
    fixture.save(false);
    let plan = open(&fixture.sink.path).unwrap();
    assert_eq!(plan.target, KNOBS);
    assert!(plan.skips(POD_BACKUP) && plan.skips(GUITAR_BACKUP));

    let lock = recover::lock_checkpoint(&fixture.sink.path).unwrap();
    let rig = SimRig::shared(Faults::default());
    let (run, _) = fixture.continue_run(plan, &rig, &mut Script::run_all());
    assert!(run.rig_state.contains("Another Rig Check"));
    assert!(rig.borrow().written.is_empty());
    drop(lock);

    let plan = open(&fixture.sink.path).unwrap();
    fixture
        .run
        .rig_description
        .push_str(" (changed after preview)");
    fixture.save(false);
    let (run, _) = fixture.continue_run(plan, &rig, &mut Script::run_all());
    assert!(run.rig_state.contains("changed after preview"));
    assert!(rig.borrow().written.is_empty());
}

#[test]
fn collected_legacy_report_with_stopped_guard_runs_only_guided_checks_and_a_fresh_guard() {
    let rig = SimRig::shared(Faults::default());
    let presence = rig.borrow().podxt.playing[32 + 21] ^ 5;
    rig.borrow_mut().podxt.playing[32 + 21] = presence;
    let fixture = Fixture::stopped_owner_pattern(&rig);
    let alias = fixture.dir.path().join("latest.txt");
    std::fs::copy(&fixture.sink.path, &alias).unwrap();
    let original_state = fixture.sink.path.with_extension("state");
    let alias_state = alias.with_extension("state");
    std::fs::copy(&original_state, &alias_state).unwrap();
    let original_plan = open(&fixture.sink.path).unwrap();
    assert!(original_plan.is_selected_checkpoint(&alias_state).unwrap());
    let plan = open(&alias).unwrap();
    assert!(plan.is_selected_checkpoint(&original_state).unwrap());
    // A different original checkpoint still needs its own safety check.
    let bytes = std::fs::read(&original_state).unwrap();
    std::fs::write(
        &original_state,
        [bytes.as_slice(), b"finished=false\n"].concat(),
    )
    .unwrap();
    assert!(plan.is_selected_checkpoint(&original_state).is_err());
    std::fs::remove_file(&original_state).unwrap();
    std::fs::remove_file(&fixture.sink.path).unwrap();
    assert_eq!(plan.target, KNOBS);
    assert!(plan.pre_write_recovery && plan.skips(POD_BACKUP) && plan.skips(GUITAR_BACKUP));
    assert!(plan.restored.podxt_unsaved);
    assert!(!plan.skips(RIG_GUARD));
    let mut person = Script::run_all();
    let wrong_rig = SimRig::shared(Faults::default());
    let (refused, refused_report) = fixture.continue_run(plan, &wrong_rig, &mut person);
    assert!(
        refused.rig_state.contains("tone differs"),
        "{}",
        refused.rig_state
    );
    assert!(person.cards.is_empty());
    assert!(!refused.step(POD_BACKUP).reused);
    assert!(error(&refused_report.path).contains("never finished verifying"));
    only_reads(&wrong_rig);

    let rig = SimRig::shared(Faults::default());
    rig.borrow_mut().podxt.playing[32 + 21] = presence;
    let mut person = Script::run_all();
    let (run, sink) = fixture.continue_run(open(&alias).unwrap(), &rig, &mut person);
    assert_eq!(person.cards.first(), Some(&KNOBS));
    assert_eq!(person.cards.last(), Some(&RIG_GUARD));
    assert!(run.step(KNOBS).outcome.passed());
    assert!(run.step(POD_BACKUP).reused && run.step(GUITAR_BACKUP).reused);
    assert!(
        run.step(RIG_GUARD).outcome.passed(),
        "{:?}",
        run.step(RIG_GUARD)
    );
    assert!(
        run.prior_attempts
            .iter()
            .any(|attempt| attempt.id == RIG_GUARD
                && attempt.outcome == Outcome::Stopped(LEGACY_GUARD_STOP.into()))
    );
    assert!(
        run.prior_attempts
            .iter()
            .filter(|attempt| attempt.id == KNOBS)
            .count()
            >= 2
    );
    assert!(person.saves.is_empty());
    assert_eq!(rig.borrow().podxt.stores, 0);
    assert_eq!(rig.borrow().variax.stores, 0);
    assert_eq!(rig.borrow().podxt.playing[32 + 21], presence);
    assert!(matches!(
        run.step(FIRST_SAVE).outcome,
        Outcome::NotAuthorized(_)
    ));
    assert!(error(&alias).contains("already has a continuation"));
    assert!(sink.last.contains("pre_write_recovery\":true"));
}

#[test]
fn the_stopped_guard_exception_refuses_interruption_failed_cleanup_and_missing_evidence() {
    let rig = SimRig::shared(Faults::default());
    let mut fixture = Fixture::stopped_owner_pattern(&rig);
    fixture.run.rig_state = "Not checked yet. USB cleanup failed: synthetic failure.".into();
    fixture.save(true);
    assert!(error(&fixture.sink.path).contains("could not release the rig"));
    fixture.run.rig_state = LEGACY_CLEAN_RELEASE.into();
    fixture.run.finished = false;
    fixture.run.current = Some(RIG_GUARD);
    fixture.save(true);
    assert!(error(&fixture.sink.path).contains("guard was stopped"));
    fixture.run.finished = true;
    fixture.run.current = None;
    fixture.run.step_mut(FIRST_SAVE).outcome = Outcome::Failed("Synthetic uncertain store.".into());
    fixture.run.step_mut(FIRST_SAVE).started = Some(utc_now());
    fixture.save(true);
    assert!(error(&fixture.sink.path).contains("unresolved outcome"));
    *fixture.run.step_mut(FIRST_SAVE) = StepRecord::new(FIRST_SAVE);
    fixture.save(true);
    std::fs::rename(
        fixture
            .data
            .evidence_dir
            .as_ref()
            .unwrap()
            .join("podxt-library"),
        fixture.dir.path().join("missing"),
    )
    .unwrap();
    assert!(error(&fixture.sink.path).contains("Backup 2.5 cannot be verified"));
}

#[test]
fn start_new_offers_safe_pending_recovery_and_requires_manual_confirmation_for_an_uncertain_store()
{
    let mut fixture = Fixture::new();
    fixture.stop_after(KNOBS, false);
    fixture.run.finished = false;
    fixture.run.current = Some(KNOBS);
    *fixture.run.step_mut(KNOBS) = StepRecord::new(KNOBS);
    *fixture.run.step_mut(RIG_GUARD) = StepRecord::new(RIG_GUARD);
    fixture.save(false);
    let rig = SimRig::shared(Faults::default());
    rig.borrow_mut().user_acts(
        crate::rig_check::practice::UserAction::PodxtFootswitch(3),
        std::time::Duration::ZERO,
    );
    let mut person = Script::run_all();
    let lock = recover::lock_checkpoint(&fixture.sink.path).unwrap();
    assert!(
        recover::before_new_run(
            &mut PracticePort::new(rig.clone()),
            &mut person,
            fixture.dir.path(),
            None,
        )
        .is_err()
    );
    assert!(rig.borrow().written.is_empty());
    assert!(person.cards.is_empty());
    drop(lock);
    let result = recover::before_new_run(
        &mut PracticePort::new(rig.clone()),
        &mut person,
        fixture.dir.path(),
        None,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(person.cards, [RIG_GUARD]);
    assert_eq!(
        rig.borrow().podxt.channel,
        fixture.data.start_channel.map(|ch| ch.index())
    );
    assert!(recover::pending(fixture.dir.path()).is_empty());
    assert!(open(&fixture.sink.path).is_ok());

    fixture.run.current = Some(FIRST_SAVE);
    fixture.save(false);
    let rig = SimRig::shared(Faults::default());
    person = Script::run_all();
    person.accept = false;
    assert!(
        recover::before_new_run(
            &mut PracticePort::new(rig.clone()),
            &mut person,
            fixture.dir.path(),
            None,
        )
        .is_err()
    );
    assert!(
        person
            .notices
            .iter()
            .any(|notice| notice.contains("verify the affected slot"))
    );
    assert!(person.cards.is_empty());
    assert!(rig.borrow().written.is_empty());
}

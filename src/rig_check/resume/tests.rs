use super::*;
use crate::rig_check::port::PracticePort;
use crate::rig_check::practice::{Faults, SimRig};
use crate::rig_check::redact::Redactor;
use crate::rig_check::report::ReportFile;
use crate::rig_check::runner::{Decision, ReportSink, Runner};
use crate::rig_check::testing::Script;
use std::cell::RefCell;
use std::rc::Rc;

const FIRST_SAVE: StepId = StepId(5, 1);
const NEXT_SAVE: StepId = StepId(5, 2);
const OLD_FAILURE: &str = "The Drive value read back differs from the Follow update.";

struct Fixture {
    sink: ReportFile,
    dir: tempfile::TempDir,
    data: RunData,
    run: RunRecord,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
        let data = report_data(&sink);
        let (run, data, _) = crate::rig_check::run_practice(
            Faults::default(),
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
        fixture.stop_after(KNOBS, true);
        let step = fixture.run.step_mut(KNOBS);
        step.title = "Turn the PODxt Live Drive knob".into();
        step.outcome = Outcome::Failed(OLD_FAILURE.into());
        step.evidence = vec!["Synthetic legacy mismatch: observed 92, read 86.".into()];
        step.attempt = Some(2);
        let mut first = step.clone();
        first.attempt = Some(1);
        first.evidence = vec!["Synthetic legacy mismatch: observed 118, read 115.".into()];
        fixture.run.prior_attempts.push(first);
        fixture.save(true);
        fixture
    }

    fn continue_run(
        &self,
        plan: Plan,
        rig: &Rc<RefCell<SimRig>>,
        person: &mut Script,
    ) -> (RunRecord, ReportFile) {
        let mut sink = ReportFile::create(self.dir.path(), Redactor::default()).unwrap();
        let data = report_data(&sink);
        let path = sink.path.with_extension("state");
        let run = crate::rig_check::continue_on(
            &mut PracticePort::new(rig.clone()),
            person,
            &mut sink,
            data,
            path,
            plan,
        )
        .0;
        (run, sink)
    }
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
    assert_eq!(
        last_report(fixture.dir.path()).as_ref(),
        Some(&fixture.sink.path)
    );
    let plan = open(&fixture.sink.path).unwrap();
    assert_eq!(plan.target, KNOBS);
    for id in [POD_BACKUP, GUITAR_BACKUP, PEDAL] {
        assert!(plan.skips(id));
    }
    assert!(!plan.skips(StepId(7, 3)));
    let mut person = Script::run_all();
    // Stop on the first store card, after checking downstream eligibility.
    person.overrides.push((FIRST_SAVE, Decision::Stop));
    let rig = SimRig::shared(Faults::default());
    let (run, sink) = fixture.continue_run(plan, &rig, &mut person);
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

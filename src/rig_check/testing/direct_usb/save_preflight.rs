use super::*;
use crate::rig_check::{
    continue_on,
    record::{RunRecord, SavePhase},
    redact::Redactor,
    report::{ReportFile, json_block},
    resume, run_with_state,
    runner::ReportSink,
    variax_save::OWN_BYTES_STEP as SAVE,
};
use std::{cell::Cell, collections::VecDeque, path::PathBuf};

const UPLOAD: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x43];
const STORE: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x61];

#[derive(Default)]
pub(super) struct SaveTrace {
    scripted: VecDeque<Option<Vec<u8>>>,
    writes: Vec<Vec<u8>>,
    pub(super) fail_release: bool,
    pub(super) releases: usize,
    fail_upload: bool,
    report: Option<PathBuf>,
    before_identity: Option<(String, String)>,
    at_upload: Option<(String, String)>,
}

impl SaveTrace {
    pub(super) fn before_write(&mut self, bytes: &[u8]) -> Result<Option<Option<Vec<u8>>>, String> {
        let snapshot = || {
            self.report.as_ref().map(|path| {
                (
                    std::fs::read_to_string(path).unwrap(),
                    std::fs::read_to_string(path.with_extension("state")).unwrap(),
                )
            })
        };
        if self.writes.is_empty() {
            self.before_identity = snapshot();
        }
        if bytes.starts_with(UPLOAD) && self.at_upload.is_none() {
            self.at_upload = snapshot();
        }
        self.writes.push(bytes.to_vec());
        if self.fail_upload && bytes.starts_with(UPLOAD) {
            return Err("simulated error writing the first upload block".into());
        }
        Ok(if bytes == REQUEST {
            self.scripted.pop_front()
        } else {
            None
        })
    }
}

fn owner_transcript() -> VecDeque<Option<Vec<u8>>> {
    [None, Some(vec![0xf7])].into()
}

fn report_data(sink: &ReportFile) -> RunData {
    let stem = sink.path.file_stem().unwrap().to_str().unwrap();
    RunData {
        evidence_dir: Some(sink.path.with_file_name(format!("{stem}-evidence"))),
        ..RunData::default()
    }
}

#[test]
fn exact_silence_then_orphan_with_bad_cleanup_does_not_send_again() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save = SaveTrace {
        scripted: owner_transcript(),
        fail_release: true,
        ..SaveTrace::default()
    };
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    let step = run.step(SAVE);
    let bus = port.bus.borrow();
    assert_eq!(bus.save.writes, [REQUEST, REQUEST]);
    assert_eq!(bus.save.releases, 1);
    let incoming: Vec<_> = step
        .messages
        .iter()
        .filter_map(|m| m.split_once("IN  "))
        .map(|(_, bytes)| bytes)
        .collect();
    assert_eq!(incoming, ["f7"]);
    assert_eq!(step.retries, 1);
    assert_eq!(step.save_phase, Some(SavePhase::PreSend));
    assert!(step.outcome.reason().unwrap().contains("release error"));
    assert!(person.saves.is_empty());
    assert_eq!(port.rig.borrow().variax.stores, 0);
    assert!(run.rig_state.contains("USB cleanup failed"));
    assert!(!person.cards.contains(&RIG_GUARD));
}

#[test]
fn the_exact_transcript_gets_one_read_only_reclaim_and_then_normal_save_order() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save.scripted = owner_transcript();
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    let step = run.step(SAVE);
    assert!(step.outcome.passed(), "{step:?}");
    assert_eq!(step.retries, 2);
    assert_eq!(step.save_phase, Some(SavePhase::SendPossible));
    let bus = port.bus.borrow();
    assert!(bus.save.releases >= 1);
    let sent = &bus.save.writes;
    assert!(sent[..3].iter().all(|bytes| bytes == &REQUEST));
    let selected = sent
        .iter()
        .position(|bytes| bytes[0] & 0xf0 == 0xc0)
        .unwrap();
    assert!(selected >= 3);
    let upload = sent
        .iter()
        .position(|bytes| bytes.starts_with(UPLOAD))
        .unwrap();
    let last_identity = sent[..upload]
        .iter()
        .rposition(|bytes| bytes == &REQUEST)
        .unwrap();
    assert!(last_identity > selected);
    let wire = sent[upload..].concat();
    assert_eq!(
        wire.windows(STORE.len())
            .filter(|bytes| *bytes == STORE)
            .count(),
        1
    );
    assert!(person.saves.iter().any(|notice| notice.contains("Spank 1")));
    assert!(run.step(RIG_GUARD).outcome.passed());
}

#[test]
fn unrelated_wrong_and_exhausted_identity_paths_never_select_or_store() {
    let good = Port::new(Faults::default()).rig.borrow().variax_identity();
    let mut wrong = good.clone();
    wrong[10] = 0;
    for (replies, requests, retries) in [
        (vec![Some(wrong.clone())], 1, 0),
        (vec![None, Some(vec![0xf0, 0x7e, 0x7f, 0xf7])], 2, 1),
        (vec![None, Some(vec![0xb0, 12, 0])], 2, 1),
        (vec![None, Some(vec![0xf7, 0xf7])], 2, 1),
        (vec![Some(vec![0xf7]), Some(vec![0xf7])], 2, 1),
        (vec![None, None], 2, 1),
        (vec![None, Some(vec![0xf7]), None], 3, 2),
        (vec![None, Some(vec![0xf7]), Some(wrong)], 3, 2),
        (vec![None, Some(vec![0xf7]), Some(good[..8].to_vec())], 3, 2),
    ] {
        let mut port = Port::new(Faults::default());
        port.bus.borrow_mut().save.scripted = replies.into();
        let mut person = Script::run_all();
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        let step = run.step(SAVE);
        assert!(matches!(step.outcome, Outcome::Failed(_)), "{step:?}");
        assert_eq!(step.save_phase, Some(SavePhase::PreSend));
        assert_eq!(step.retries, retries);
        assert_eq!(port.bus.borrow().save.writes, vec![REQUEST; requests]);
        assert_eq!(port.rig.borrow().variax.stores, 0);
        assert!(person.saves.is_empty());
        assert!(run.step(RIG_GUARD).outcome.passed());
    }
}

#[test]
fn stopping_during_the_identity_backoff_does_not_count_a_retry() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save.scripted = owner_transcript();
    let mut person = Script::run_all();
    let stop_at = Rc::new(Cell::new(Duration::MAX));
    let clock = port.rig.clone();
    let deadline = stop_at.clone();
    person.on_card = Some(Box::new(move |step| {
        if step == SAVE {
            deadline.set(clock.borrow().clock + Duration::from_secs(4));
        } else {
            deadline.set(Duration::MAX);
        }
    }));
    let clock = port.rig.clone();
    person.stop_when = Some(Box::new(move || clock.borrow().clock >= stop_at.get()));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    let step = run.step(SAVE);
    assert!(matches!(step.outcome, Outcome::Stopped(_)));
    assert_eq!(step.retries, 0);
    assert_eq!(port.bus.borrow().save.writes, [REQUEST]);
    assert!(person.saves.is_empty());
    assert!(run.step(RIG_GUARD).outcome.passed());
}

#[test]
fn operator_can_retry_the_mac_identity_failure_before_any_upload() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save.scripted = [None, Some(vec![0xf7]), Some(vec![0xf7])].into();
    let mut person = Script::run_all();
    person.repeat_once = Some(SAVE);
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());

    let first = run
        .prior_attempts
        .iter()
        .find(|step| step.id == SAVE)
        .unwrap();
    assert!(first.can_retry_save_preflight());
    assert_eq!(first.save_phase, Some(SavePhase::PreSend));
    assert_eq!(person.cards.iter().filter(|&&id| id == SAVE).count(), 2);
    assert_eq!(run.step(SAVE).attempt, Some(2));
    assert!(run.step(SAVE).outcome.passed());
    assert!(!run.step(SAVE).can_repeat());
    assert!(run.step(RIG_GUARD).outcome.passed());
    let bus = port.bus.borrow();
    assert_eq!(bus.save.writes[..4], [REQUEST; 4]);
    assert!(bus.save.releases >= 2);
    assert!(first.answers.is_empty());
    assert_eq!(run.step(SAVE).answers.len(), 1);
    let wire = bus.save.writes.concat();
    assert_eq!(
        wire.windows(STORE.len())
            .filter(|bytes| *bytes == STORE)
            .count(),
        1
    );
}

#[test]
fn completed_preflight_failure_resumes_the_failed_step_without_replaying_passes() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save.scripted = [None, Some(vec![0xf7]), Some(vec![0xf7])].into();
    let dir = tempfile::tempdir().unwrap();
    let mut source = ReportFile::create(dir.path(), Redactor::default()).unwrap();
    let data = report_data(&source);
    let path = source.path.with_extension("state");
    let (run, _) = run_with_state(
        &mut port,
        &mut Script::run_all(),
        &mut source,
        data,
        path,
        None,
    );
    let original = run.step(SAVE);
    assert!(original.can_retry_save_preflight());
    assert!(run.step(RIG_GUARD).outcome.passed());
    assert_eq!(port.bus.borrow().save.writes, vec![REQUEST; 3]);
    assert_eq!(port.rig.borrow().variax.stores, 0);
    let plan = resume::open(&source.path).unwrap();
    assert_eq!(plan.target, SAVE);

    let mut person = Script::run_all();
    let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
    let data = report_data(&sink);
    let path = sink.path.with_extension("state");
    let (continued, _) = continue_on(&mut port, &mut person, &mut sink, data, path, plan, None);
    assert_eq!(person.cards.first(), Some(&SAVE));
    assert!(
        continued.step(SAVE).outcome.passed(),
        "{}",
        continued.rig_state
    );
    assert_eq!(continued.step(SAVE).attempt, Some(2));
    let first = continued
        .prior_attempts
        .iter()
        .find(|step| step.id == SAVE)
        .unwrap();
    assert_eq!(first.outcome, original.outcome);
    assert_eq!(first.message_count(), original.message_count());
    assert_eq!(continued.step(RIG_GUARD).attempt, Some(2));
    assert!(
        continued.step(RIG_GUARD).outcome.passed(),
        "{}",
        continued.rig_state
    );
}

#[test]
fn report_checkpoint_pair_changes_phase_before_the_first_upload() {
    let mut port = Port::new(Faults::default());
    let dir = tempfile::tempdir().unwrap();
    let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
    port.bus.borrow_mut().save.report = Some(sink.path.clone());
    let data = report_data(&sink);
    let path = sink.path.with_extension("state");
    let (run, _) = run_with_state(
        &mut port,
        &mut Script::run_all(),
        &mut sink,
        data,
        path.clone(),
        None,
    );
    assert!(run.step(SAVE).outcome.passed());
    let bus = port.bus.borrow();
    let snapshots = [
        bus.save.before_identity.as_ref().unwrap(),
        bus.save.at_upload.as_ref().unwrap(),
    ];
    for ((report, state), expected) in snapshots.into_iter().zip(["pre_send", "send_possible"]) {
        let json: serde_json::Value = serde_json::from_str(json_block(report).unwrap()).unwrap();
        let save = json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "5.1")
            .unwrap();
        assert_eq!(save["save_phase"], expected);
        assert_eq!(save["attempt"], 1);
        assert_eq!(save["outcome"], "not_reached");
        std::fs::write(&sink.path, report).unwrap();
        std::fs::write(&path, state).unwrap();
        let plan = resume::open(&sink.path);
        if expected == "pre_send" {
            assert_eq!(plan.unwrap().target, RIG_GUARD);
        } else {
            assert!(
                plan.err()
                    .unwrap()
                    .contains("interrupted during a live edit or store")
            );
        }
    }

    // If the app closes between the two atomic replacements, neither mixed
    // pair may turn the old phase into proof of a harmless interruption.
    for (report_at, checkpoint_at) in [(0, 1), (1, 0)] {
        std::fs::write(&sink.path, &snapshots[report_at].0).unwrap();
        std::fs::write(&path, &snapshots[checkpoint_at].1).unwrap();
        assert!(
            resume::open(&sink.path)
                .err()
                .unwrap()
                .contains("checkpoint do not match")
        );
    }
}

struct RejectBoundary(ReportFile);

impl ReportSink for RejectBoundary {
    fn update(&mut self, run: &RunRecord) -> Result<(), String> {
        if run.current == Some(SAVE) && run.step(SAVE).save_phase == Some(SavePhase::SendPossible) {
            return Err("simulated full filesystem".into());
        }
        self.0.update(run)
    }

    fn report_digest(&self) -> Option<String> {
        self.0.report_digest()
    }
}

#[test]
fn a_failed_phase_checkpoint_blocks_the_upload_and_keeps_the_guard_release_only() {
    let mut port = Port::new(Faults::default());
    let dir = tempfile::tempdir().unwrap();
    let mut sink = RejectBoundary(ReportFile::create(dir.path(), Redactor::default()).unwrap());
    let data = report_data(&sink.0);
    let path = sink.0.path.with_extension("state");
    let mut person = Script::run_all();
    let (run, _) = run_with_state(&mut port, &mut person, &mut sink, data, path, None);
    assert!(
        run.step(SAVE)
            .outcome
            .reason()
            .unwrap()
            .contains("persist the Variax save send boundary")
    );
    let wire = port.bus.borrow().save.writes.concat();
    assert!(!wire.windows(UPLOAD.len()).any(|bytes| bytes == UPLOAD));
    assert!(!wire.windows(STORE.len()).any(|bytes| bytes == STORE));
    assert!(!person.cards.contains(&RIG_GUARD));
    assert!(run.rig_state.contains("interface 1 released"));
    assert_eq!(port.rig.borrow().variax.stores, 0);
}

#[test]
fn an_upload_transport_error_stays_unresolved_even_when_no_store_was_sent() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().save.fail_upload = true;
    let dir = tempfile::tempdir().unwrap();
    let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
    let path = sink.path.with_extension("state");
    let data = report_data(&sink);
    let (run, _) = run_with_state(
        &mut port,
        &mut Script::run_all(),
        &mut sink,
        data,
        path,
        None,
    );
    let step = run.step(SAVE);
    assert!(
        step.outcome
            .reason()
            .unwrap()
            .contains("error writing the first upload")
    );
    assert_eq!(step.save_phase, Some(SavePhase::SendPossible));
    let bus = port.bus.borrow();
    assert_eq!(
        bus.save
            .writes
            .iter()
            .filter(|bytes| bytes.starts_with(UPLOAD))
            .count(),
        1
    );
    assert!(
        !bus.save
            .writes
            .concat()
            .windows(STORE.len())
            .any(|bytes| bytes == STORE)
    );
    assert!(run.step(RIG_GUARD).outcome.passed());
    assert!(
        resume::open(&sink.path)
            .err()
            .unwrap()
            .contains("unresolved outcome")
    );
}

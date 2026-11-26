//! A scripted person for tests, plus end-to-end runner tests.

use super::catalogue::{SaveNotice, Step, StepId};
use super::record::{Mode, Outcome, StepRecord};
use super::runner::{Answer, Decision, Operator, Progress, Question, ResultDecision, Retry};
use std::cell::Cell;

/// Answers every card with `default`, except listed steps.
pub struct Script {
    pub accept: bool,
    pub default: Decision,
    pub overrides: Vec<(StepId, Decision)>,
    /// Ask to stop while this step runs.
    pub stop_during: Option<StepId>,
    pub save_typing: SaveTyping,
    pub save_overrides: Vec<(StepId, SaveTyping)>,
    pub retry: Retry,
    pub answer: Answer,
    pub cards: Vec<StepId>,
    pub notices: Vec<String>,
    pub progress: Vec<(StepId, Progress)>,
    pub accept_evidence: bool,
    pub instructions: Vec<(StepId, String)>,
    pub questions: Vec<Question>,
    pub saves: Vec<String>,
    pub finished: Vec<(StepId, Outcome)>,
    pub different: Option<(StepId, String)>,
    current: Cell<Option<StepId>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SaveTyping {
    /// Type exactly what the notice asks for.
    Exact,
    Wrong,
    Skip,
}

impl Script {
    pub fn run_all() -> Self {
        Self {
            accept: true,
            default: Decision::Run,
            overrides: Vec::new(),
            stop_during: None,
            save_typing: SaveTyping::Exact,
            save_overrides: Vec::new(),
            retry: Retry::Skip,
            answer: Answer::Yes,
            cards: Vec::new(),
            notices: Vec::new(),
            progress: Vec::new(),
            accept_evidence: true,
            instructions: Vec::new(),
            questions: Vec::new(),
            saves: Vec::new(),
            finished: Vec::new(),
            different: None,
            current: Cell::new(None),
        }
    }

    #[allow(dead_code)]
    pub fn outcome(&self, id: StepId) -> &Outcome {
        &self
            .finished
            .iter()
            .find(|(step, _)| *step == id)
            .expect("step finished")
            .1
    }
}

impl Operator for Script {
    fn accept_start(&mut self, notice: &str) -> bool {
        self.notices.push(notice.to_owned());
        if [
            super::variax_evidence::NOTICE,
            super::variax_evidence::RELAY_NOTICE,
        ]
        .contains(&notice)
        {
            self.accept_evidence
        } else {
            self.accept
        }
    }

    fn confirm(&mut self, step: &Step, _: Mode) -> Decision {
        self.cards.push(step.id);
        self.current.set(Some(step.id));
        self.overrides
            .iter()
            .find(|(id, _)| *id == step.id)
            .map_or(self.default, |(_, decision)| *decision)
    }

    fn confirm_save(&mut self, notice: &SaveNotice) -> Option<String> {
        self.saves.push(notice.text());
        let typing = self
            .save_overrides
            .iter()
            .find(|(id, _)| Some(*id) == self.current.get())
            .map_or(self.save_typing, |(_, typing)| *typing);
        match typing {
            SaveTyping::Exact => Some(notice.confirmation.clone()),
            SaveTyping::Wrong => Some("store it".into()),
            SaveTyping::Skip => None,
        }
    }

    fn instruct(&mut self, step: StepId, see: &str, hear: &str) {
        self.instructions
            .push((step, format!("SEE {see} HEAR {hear}")));
    }

    fn not_seen(&mut self, _: StepId, _: &str) -> Retry {
        self.retry
    }

    fn ask(&mut self, question: &Question) -> (Answer, String) {
        self.questions.push(question.clone());
        (self.answer, "scripted".into())
    }

    fn progress(&mut self, progress: Progress) {
        if let Some(step) = self.current.get() {
            self.progress.push((step, progress));
        }
    }

    fn finished(&mut self, record: &StepRecord) -> ResultDecision {
        self.finished.push((record.id, record.outcome.clone()));
        if let Some((id, value)) = &self.different
            && *id == record.id
        {
            return ResultDecision::Different(value.clone());
        }
        ResultDecision::Continue
    }

    fn result_updated(&mut self, record: &StepRecord) {
        self.finished.last_mut().expect("result was shown").1 = record.outcome.clone();
    }

    fn stop_requested(&self) -> bool {
        self.stop_during.is_some() && self.current.get() == self.stop_during
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::{RIG_GUARD, RUN_ORDER, STEPS, VARIAX_EVIDENCE};
    use crate::rig_check::port::RealPort;
    use crate::rig_check::practice::{Faults, SimRig, flash_index};
    use crate::rig_check::run_on;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn at_start(rig: &Rc<RefCell<SimRig>>) -> bool {
        let fresh = SimRig::new(Faults::default());
        let rig = rig.borrow();
        rig.podxt.playing == fresh.podxt.playing
            && rig.podxt.channel == fresh.podxt.channel
            && rig.podxt.stored == fresh.podxt.stored
            && rig.variax.active == fresh.variax.active
            && rig.variax.selected == fresh.variax.selected
            && rig.variax.stored == fresh.variax.stored
            && !rig.variax_claimed
            && rig.driver_bound
    }

    #[test]
    fn practice_mode_runs_the_whole_catalogue() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(run.finished && run.start_notice_accepted);
        for step in STEPS {
            let outcome = &run.step(step.id).outcome;
            if step.id == VARIAX_EVIDENCE {
                assert_eq!(
                    outcome.reason(),
                    Some(
                        "Skipped: your Variax reports firmware 3.10, the tested version. This check is only for other versions."
                    )
                );
                assert_eq!(outcome.code(), "skipped_verified_firmware");
                assert_eq!(
                    outcome.technical_note(),
                    Some("evidence mode not entered: firmware verified")
                );
            } else if step.not_in_this_version.is_some() {
                assert!(matches!(outcome, Outcome::NotAvailable(_)), "{}", step.id);
            } else {
                assert_eq!(
                    outcome,
                    &Outcome::Passed,
                    "{}: {:?}",
                    step.id,
                    run.step(step.id)
                );
            }
        }
        // Every offered step showed its card first, in run order.
        let offered: Vec<StepId> = RUN_ORDER
            .iter()
            .copied()
            .filter(|id| {
                *id != VARIAX_EVIDENCE
                    && crate::rig_check::catalogue::step(*id)
                        .unwrap()
                        .not_in_this_version
                        .is_none()
            })
            .collect();
        assert_eq!(person.cards, offered);
        assert!(at_start(&rig), "the simulated rig is back at its start");
        assert!(run.rig_state.contains("starting"), "{}", run.rig_state);
    }

    #[test]
    fn the_engine_report_records_the_built_byte_hash_for_every_case() {
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut Script::run_all(),
            &mut NoReport,
            RunData::default(),
        );
        let report = crate::rig_check::report::render_plain(&run, "test time");
        let expected = crate::rig_check::engine_check::builtin();
        let step = run.step(StepId(0, 3));
        assert_eq!(step.checksums.len(), expected.len());
        for case in expected {
            let hash = case.actual_sha256.as_deref().expect("built reference case");
            assert!(report.contains(&format!("{hash}  {} (built here)", case.name)));
        }
    }

    #[test]
    fn look_checks_pass_on_continue_and_block_live_edits_and_saves_on_a_difference() {
        let look_step = StepId(2, 2);
        for different in [None, Some("  Other tone  ")] {
            let mut person = Script::run_all();
            person.different = different.map(|value| (look_step, value.into()));
            let (run, _, rig) = run_practice(
                Faults::default(),
                &mut person,
                &mut NoReport,
                RunData::default(),
            );
            let look = run.step(look_step).look.as_ref().unwrap();
            assert_eq!(look.expected, "Practice 5B");
            assert_eq!(look.different.as_deref(), different);
            let report = super::super::report::render_plain(&run, "practice");
            if let Some(value) = different {
                assert!(matches!(person.outcome(look_step), Outcome::Failed(_)));
                let expected =
                    format!("Step 2.2 — Expected \"Practice 5B\" · On the device: {value:?}");
                assert!(report.find(&expected).unwrap() < report.find("Passed ").unwrap());
                for id in [StepId(4, 1), StepId(4, 2), StepId(5, 1), StepId(5, 2)] {
                    assert!(
                        matches!(&run.step(id).outcome, Outcome::SkippedNeeds(reason) if reason == "read-only from here: step 2.2 failed")
                    );
                    assert!(!person.cards.contains(&id));
                }
                assert!(person.saves.is_empty());
                assert_eq!(rig.borrow().variax.stores, 0);
            } else {
                assert!(person.outcome(look_step).passed());
                for id in [StepId(2, 3), StepId(4, 1), StepId(7, 3)] {
                    let step = run.step(id);
                    assert!(step.outcome.passed() && step.look.is_some());
                }
                assert!(run.step(StepId(5, 1)).outcome.passed());
                assert!(report.contains("On the device: not reported different"));
            }
        }
    }

    fn assert_progress(script: &Script, id: StepId, units: &str, total: usize) {
        let seen: Vec<_> = script
            .progress
            .iter()
            .filter_map(|(step, progress)| {
                let (done, limit, measure) = progress.count?;
                if *step != id || measure != units {
                    return None;
                }
                assert_eq!(limit, total, "{id}");
                assert!(!progress.action.is_empty());
                Some(done)
            })
            .collect();
        assert!(!seen.is_empty(), "{id}: no {units} progress");
        assert_eq!(seen.len() % (total + 1), 0, "{id}: {seen:?}");
        for group in seen.chunks_exact(total + 1) {
            assert!(group.iter().copied().eq(0..=total), "{id}: {group:?}");
        }
    }

    #[test]
    fn repeated_work_reports_completed_counts_and_guided_waits_have_a_limit() {
        let mut person = Script::run_all();
        run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        for (id, units, total) in [
            (StepId(2, 4), "channels", 4),
            (StepId(2, 5), "entries", 299),
            (StepId(3, 4), "presets", 60),
            (StepId(3, 5), "presets", 1),
            (StepId(5, 4), "slots", 2),
        ] {
            assert_progress(&person, id, units, total);
        }
        for id in [
            StepId(3, 3),
            StepId(3, 4),
            StepId(3, 5),
            StepId(4, 2),
            StepId(4, 3),
            StepId(4, 4),
            StepId(4, 5),
            StepId(5, 1),
            StepId(5, 3),
            StepId(5, 4),
            StepId(7, 1),
        ] {
            assert_progress(&person, id, "reads", 35);
        }
        for id in [
            StepId(6, 1),
            StepId(7, 1),
            StepId(7, 2),
            StepId(7, 3),
            StepId(7, 4),
            StepId(7, 5),
        ] {
            assert!(
                person.progress.iter().any(|(step, progress)| {
                    *step == id
                        && progress.guided
                        && progress.wait == Some(super::super::guided::WINDOW)
                }),
                "{id}"
            );
        }
        for fault in [
            Faults {
                unverified_variax: true,
                ..Faults::default()
            },
            Faults {
                unverified_podxt: true,
                ..Faults::default()
            },
        ] {
            let mut person = Script::run_all();
            run_practice(fault, &mut person, &mut NoReport, RunData::default());
            assert_progress(&person, VARIAX_EVIDENCE, "reads", 35);
        }
    }

    #[test]
    fn silent_stores_report_the_confirmation_limit_then_the_two_minute_hold() {
        use std::time::Duration;
        for (id, receipt, faults) in [
            (
                StepId(5, 1),
                15,
                Faults {
                    silent_variax_store: true,
                    ..Faults::default()
                },
            ),
            (
                StepId(5, 2),
                7,
                Faults {
                    silent_podxt_store: true,
                    ..Faults::default()
                },
            ),
        ] {
            let mut person = Script::run_all();
            run_practice(faults, &mut person, &mut NoReport, RunData::default());
            let waits: Vec<_> = person
                .progress
                .iter()
                .filter_map(|(step, progress)| {
                    (*step == id)
                        .then_some(progress.wait?)
                        .map(|wait| wait.as_secs())
                })
                .collect();
            assert_eq!(waits, [receipt, Duration::from_mins(2).as_secs()]);
        }
    }

    #[test]
    fn nothing_runs_without_an_explicit_run() {
        let mut person = Script::run_all();
        person.default = Decision::Skip;
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(rig.borrow().written.is_empty(), "nothing was sent");
        for step in STEPS {
            assert!(!run.step(step.id).outcome.passed(), "{}", step.id);
        }
        let mut person = Script::run_all();
        person.accept = false;
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(person.cards.is_empty() && rig.borrow().written.is_empty());
        assert!(run.rig_state.contains("Nothing ran"));
    }

    #[test]
    fn a_failure_skips_its_dependents() {
        let mut person = Script::run_all();
        let faults = Faults {
            variax_silent: true,
            ..Faults::default()
        };
        let (run, _, _) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        assert!(matches!(run.step(StepId(3, 1)).outcome, Outcome::Failed(_)));
        for id in [
            StepId(3, 2),
            StepId(3, 3),
            StepId(3, 4),
            StepId(4, 2),
            StepId(5, 1),
        ] {
            assert!(
                matches!(run.step(id).outcome, Outcome::SkippedNeeds(_)),
                "{id}: {:?}",
                run.step(id).outcome
            );
            assert!(!person.cards.contains(&id), "{id} was not offered");
        }
        assert!(
            run.step(StepId(2, 2)).outcome.passed(),
            "PODxt steps still run"
        );
    }

    #[test]
    fn stop_at_any_step_leaves_the_rig_at_its_start() {
        for &stop in RUN_ORDER.iter().filter(|id| **id != RIG_GUARD) {
            for during in [false, true] {
                let mut person = Script::run_all();
                if during {
                    person.stop_during = Some(stop);
                } else {
                    person.overrides.push((stop, Decision::Stop));
                }
                let (run, _, rig) = run_practice(
                    Faults::default(),
                    &mut person,
                    &mut NoReport,
                    RunData::default(),
                );
                if !person.cards.contains(&stop) {
                    continue;
                }
                assert_eq!(*person.cards.last().unwrap(), RIG_GUARD, "{stop}");
                assert!(
                    at_start(&rig),
                    "stop at {stop} (during: {during}): {:?}",
                    run.step(RIG_GUARD)
                );
                assert!(
                    run.step(RIG_GUARD).outcome.passed(),
                    "{stop}: {:?}",
                    run.step(RIG_GUARD)
                );
                assert!(!run.rig_state.is_empty());
                assert!(run.stopped_at.is_some(), "{stop}");
            }
        }
    }

    #[test]
    fn real_mode_never_offers_audio_cards() {
        let mut person = Script::run_all();
        let mut port = RealPort::with_system(Box::new(
            crate::rig_check::computer::linux::synthetic_machine(true, true),
        ));
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        for id in [StepId(8, 2), StepId(8, 3), StepId(8, 4), StepId(8, 5)] {
            assert!(matches!(run.step(id).outcome, Outcome::NotAvailable(_)));
            assert!(!person.cards.contains(&id));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn real_mode_on_mac_only_offers_computer_checks() {
        let mut person = Script::run_all();
        let mut port = RealPort::with_system(Box::new(
            crate::rig_check::computer::linux::synthetic_machine(true, true),
        ));
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        for step in STEPS.iter().filter(|step| step.needs_device_approval()) {
            assert_eq!(
                run.step(step.id).outcome,
                Outcome::NotAvailable(crate::MAC_CONNECTION_UNAVAILABLE.into()),
                "{}",
                step.id
            );
        }
        assert_eq!(
            person.cards,
            [StepId(0, 1), StepId(0, 2), StepId(0, 3), StepId(8, 1)]
        );
        assert!(person.cards.iter().all(|id| run.step(*id).outcome.passed()));
    }

    #[test]
    fn a_failure_or_unpassed_step_prevents_later_saves() {
        for (faults, skip, reason) in [
            (
                Faults {
                    driver_not_returned: true,
                    ..Faults::default()
                },
                false,
                "read-only from here: step 1.2 failed",
            ),
            (
                Faults::default(),
                true,
                "read-only from here: step 4.2 did not pass",
            ),
        ] {
            let mut person = Script::run_all();
            if skip {
                person.overrides.push((StepId(4, 2), Decision::Skip));
            }
            let (run, _, rig) =
                run_practice(faults, &mut person, &mut NoReport, RunData::default());
            for id in [StepId(5, 1), StepId(5, 2), StepId(5, 3), StepId(5, 4)] {
                assert_eq!(run.step(id).outcome, Outcome::SkippedNeeds(reason.into()));
                assert!(!person.cards.contains(&id));
            }
            assert!(person.saves.is_empty());
            assert_eq!(rig.borrow().variax.stores, 0);
            assert_eq!(rig.borrow().podxt.stores, 0);
        }
    }

    #[test]
    fn new_steps_use_the_expected_device_messages_and_restore_the_rig() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        for (id, prefixes) in [
            (
                StepId(2, 6),
                &["OUT f0 00 01 0c 03 0e", "OUT f0 00 01 0c 03 57"][..],
            ),
            (StepId(4, 4), &["OUT f0 00 01 0c 07 00 3b", "OUT c0"][..]),
            (
                StepId(4, 5),
                &[
                    "OUT f0 00 01 0c 07 00 3b",
                    "OUT f0 00 01 0c 07 00 43",
                    "OUT c0",
                ][..],
            ),
            (StepId(4, 6), &["OUT f0 00 01 0c 03 74"][..]),
            (StepId(4, 7), &["OUT c0 12", "OUT c0 11"][..]),
            (
                StepId(4, 8),
                &["OUT b0 45 7f", "OUT f0 00 01 0c 03 57", "OUT b0 45 00"][..],
            ),
            (
                StepId(5, 3),
                &["OUT f0 00 01 0c 07 00 61 0e", "OUT f0 00 01 0c 07 00 61 0e"][..],
            ),
            (
                StepId(5, 4),
                &["OUT f0 00 01 0c 07 00 61 0e", "OUT f0 00 01 0c 07 00 61 0d"][..],
            ),
            (StepId(7, 5), &["IN  b0", "OUT f0 00 01 0c 03 75"][..]),
        ] {
            let step = run.step(id);
            assert!(step.outcome.passed(), "{id}: {:?}", step.outcome);
            let mut lines = step.messages.iter();
            for prefix in prefixes {
                assert!(
                    lines.any(|line| line.contains(prefix)),
                    "{id}: missing {prefix}"
                );
            }
        }
        assert!(at_start(&rig));
    }

    #[test]
    fn the_start_preset_is_found_and_restored_after_a_damaged_read() {
        let mut person = Script::run_all();
        let faults = Faults {
            damage_span_reply: Some(3),
            drop_first_variax_identity_packet: true,
            ..Faults::default()
        };
        let (run, data, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        let full = run.step(StepId(3, 3));
        assert!(full.outcome.passed() && full.retries >= 1, "{full:?}");
        assert!(run.step(StepId(3, 1)).retries >= 1);
        let start = data.start_preset.unwrap();
        assert_eq!(flash_index(start), 14, "Spank 1 is flash slot 14");
        assert!(at_start(&rig));
    }
}

//! A scripted person for tests, plus end-to-end runner tests.

use super::catalogue::{SaveNotice, Step, StepId};
use super::record::{Mode, Outcome, StepRecord};
use super::runner::{Answer, Decision, Operator, Question, Retry};
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
    pub accept_evidence: bool,
    pub instructions: Vec<(StepId, String)>,
    pub questions: Vec<Question>,
    pub saves: Vec<String>,
    pub finished: Vec<(StepId, Outcome)>,
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
            accept_evidence: true,
            instructions: Vec::new(),
            questions: Vec::new(),
            saves: Vec::new(),
            finished: Vec::new(),
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
        if notice == super::variax_evidence::NOTICE {
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

    fn progress(&mut self, _: &str) {}

    fn finished(&mut self, record: &StepRecord) -> bool {
        self.finished.push((record.id, record.outcome.clone()));
        true
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
            if step.id == VARIAX_EVIDENCE || step.not_in_this_version.is_some() {
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

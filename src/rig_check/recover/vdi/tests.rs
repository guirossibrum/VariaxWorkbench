use super::*;
use crate::rig_check::catalogue::StepId;
use crate::rig_check::guided::{ACTION_WINDOW, READY_DELAY};
use crate::rig_check::port::PracticePort;
use crate::rig_check::practice::{Faults, SimRig};
use crate::rig_check::record::{Outcome, StepRecord};
use crate::rig_check::runner::NoReport;
use crate::rig_check::steps::RunData;
use crate::rig_check::testing::Script;
use line6_devices::identity::REQUEST;
use std::rc::Rc;

const STEP: StepId = StepId(6, 1);

fn run(rig: &Rc<std::cell::RefCell<SimRig>>, person: &mut Script) -> StepRecord {
    let mut port = PracticePort::new(rig.clone());
    let (report, _) =
        crate::rig_check::run_on(&mut port, person, &mut NoReport, RunData::default());
    report.step(STEP).clone()
}

fn requests(step: &StepRecord) -> usize {
    let request = format!("OUT {}", line6_devices::midi::hex(&REQUEST));
    step.messages
        .iter()
        .filter(|line| line.ends_with(&request))
        .count()
}

#[test]
fn notices_complete_listening_and_the_return_is_checked_once() {
    let rig = SimRig::shared(Faults::default());
    let step = run(&rig, &mut Script::run_all());

    assert!(step.outcome.passed(), "{step:?}");
    assert_eq!(requests(&step), 2, "preflight and return verification");
    let results = step.result.join("\n");
    assert!(results.contains("reported that the guitar disconnected"));
    assert!(results.contains("reported that the guitar reconnected"));
    assert!(results.contains("VDI restored"));
}

#[test]
fn silent_notices_use_bounded_intervals_with_no_cable_cue_during_reads() {
    let rig = SimRig::shared(Faults {
        suppress_vdi_notices: true,
        guided_action_delay: Some(Duration::from_secs(26)),
        ..Faults::default()
    });
    let guidance = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut person = Script::run_all();
    let recorded = guidance.clone();
    let simulated = rig.clone();
    person.on_guidance = Some(Box::new(move |step, cue| {
        if step == STEP {
            let sim = simulated.borrow();
            recorded.borrow_mut().push((
                cue.to_owned(),
                sim.clock,
                sim.written
                    .iter()
                    .filter(|(_, bytes)| bytes == &REQUEST)
                    .count(),
            ));
        }
    }));
    let step = run(&rig, &mut person);
    assert!(step.outcome.passed(), "{step:?}");
    assert_eq!(requests(&step), 5, "preflight and two checks per phase");
    let results = step.result.join("\n");
    assert!(results.contains("Disconnect detected by identity check"));
    assert!(results.contains("Reconnect detected by identity check"));
    assert!(results.contains("VDI restored"));

    let cues = guidance.borrow();
    for phase in [Phase::Unplug, Phase::Reconnect] {
        let (see, _, _) = phase.cue();
        let ready_cue = match phase {
            Phase::Unplug => "Wait for the countdown",
            Phase::Reconnect => "Disconnect detected. Wait for the countdown",
        };
        let ready = cues
            .iter()
            .rposition(|(cue, _, _)| cue.starts_with(ready_cue))
            .unwrap();
        let active: Vec<_> = cues
            .iter()
            .enumerate()
            .filter(|(_, (cue, _, _))| cue == see)
            .collect();
        let (_, (_, first_at, first_requests)) = active[0];
        assert!(*first_at >= cues[ready].1 + READY_DELAY);
        assert_eq!(*first_requests, cues[ready].2, "no reads during countdown");

        assert_eq!(active.len(), 2);
        for (index, (_, act_at, act_requests)) in active {
            let (next_cue, next_at, next_requests) = &cues[index + 1];
            assert_eq!(next_cue, HOLD_CABLE);
            assert!(*next_at >= *act_at + LISTEN_INTERVAL);
            assert_eq!(
                next_requests, act_requests,
                "no reads while a cable cue is active"
            );
        }
    }
    assert!(cues.last().unwrap().1 - cues[0].1 < ACTION_WINDOW);
}

#[test]
fn transport_failure_in_a_fallback_check_is_not_a_disconnect() {
    let rig = SimRig::shared(Faults {
        suppress_vdi_notices: true,
        ..Faults::default()
    });
    let mut person = Script::run_all();
    let simulated = rig.clone();
    let mut checks = 0;
    person.on_guidance = Some(Box::new(move |step, cue| {
        if step == STEP && cue == HOLD_CABLE {
            checks += 1;
            if checks == 2 {
                simulated.borrow_mut().faults.fail_variax_read = true;
            }
        }
    }));
    let step = run(&rig, &mut person);
    assert!(matches!(
        step.outcome,
        Outcome::Failed(ref error) if error.contains("transport read failed")
    ));
    assert!(step.result.is_empty(), "{step:?}");
    assert!(
        !person
            .instructions
            .iter()
            .any(|(step, cue)| { *step == STEP && cue.contains("plugging the VDI cable back") })
    );
}

#[test]
fn each_phase_times_out_without_evidence_and_never_reports_restoration() {
    for (phase, expected) in [
        (Phase::Unplug, "the guitar still answering"),
        (Phase::Reconnect, "no guitar identity reply"),
    ] {
        let rig = SimRig::shared(Faults {
            suppress_vdi_notices: true,
            ..Faults::default()
        });
        let mut person = Script::run_all();
        let simulated = rig.clone();
        person.on_prepare = Some(Box::new(move |step| {
            if step == STEP {
                let mut model = simulated.borrow_mut();
                model.faults.user_does_not_act = match phase {
                    Phase::Unplug => true,
                    Phase::Reconnect => !model.variax.connected,
                };
            }
        }));
        let step = run(&rig, &mut person);
        assert_eq!(step.outcome, Outcome::SkippedByPerson);
        assert!(step.evidence.iter().any(|line| line.contains(expected)));
        assert!(!step.result.iter().any(|line| line.contains("VDI restored")));
        assert!((6..=10).contains(&requests(&step)), "{step:?}");
    }
}

#[test]
fn stop_during_countdown_listen_or_identity_read_never_starts_another_check() {
    for (during_reconnect, at, expected_requests) in [
        (false, READY_DELAY / 2, 1),
        (false, READY_DELAY + Duration::from_secs(2), 1),
        (
            true,
            READY_DELAY + LISTEN_INTERVAL + Duration::from_secs(1),
            2,
        ),
    ] {
        let rig = SimRig::shared(Faults::default());
        let mut person = Script::run_all();
        let deadline = Rc::new(Cell::new(None));
        let set_deadline = deadline.clone();
        let simulated = rig.clone();
        person.on_prepare = Some(Box::new(move |step| {
            if step == STEP {
                let mut model = simulated.borrow_mut();
                if model.variax.connected != during_reconnect {
                    set_deadline.set(Some(model.clock + at));
                    // Keep the guitar silent during the first reconnect read.
                    if during_reconnect {
                        model.faults.user_does_not_act = true;
                    }
                }
            }
        }));
        let simulated = rig.clone();
        person.stop_when = Some(Box::new(move || {
            deadline
                .get()
                .is_some_and(|at| simulated.borrow().clock >= at)
        }));
        let step = run(&rig, &mut person);
        assert!(matches!(step.outcome, Outcome::Stopped(_)), "{step:?}");
        assert_eq!(requests(&step), expected_requests);
        assert!(!step.result.iter().any(|line| line.contains("VDI restored")));
        assert_eq!(
            step.result
                .iter()
                .any(|line| line.contains("Disconnect detected")),
            during_reconnect
        );
    }
}

use super::*;
use crate::rig_check::catalogue::RIG_GUARD;
use crate::rig_check::practice::{SimRig, TestPodxtDump};
use crate::rig_check::{port::PracticePort, run_on};
use line6_devices::family::podxt::PodxtRequest;
use line6_devices::midi::hex;
use line6_devices::session::Request;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const FOOTSWITCH: StepId = StepId(7, 3);
const PEDAL: StepId = StepId(7, 4);
const START: u8 = 17;
const TARGET: u8 = 18;

#[test]
fn early_notices_trigger_one_settled_read_each_and_verify_the_return() {
    let mut person = Script::run_all();
    let (run, _, rig) = run_practice(
        Faults {
            guided_action_delay: Some(Duration::from_secs(2)),
            ..Faults::default()
        },
        &mut person,
        &mut NoReport,
        RunData::default(),
    );
    let step = run.step(FOOTSWITCH);
    assert!(step.outcome.passed(), "{step:?}");
    assert_eq!(
        step.evidence
            .iter()
            .filter(|line| line.contains("get-ready countdown"))
            .count(),
        2
    );
    let dump_request = format!("OUT {}", hex(&PodxtRequest::EditBuffer.bytes()));
    assert_eq!(
        step.messages
            .iter()
            .filter(|line| line.contains(&dump_request))
            .count(),
        4,
        "one preflight and one settled read per window"
    );
    let windows: Vec<_> = step
        .messages
        .split(|line| line.contains("Listening window open"))
        .skip(1)
        .collect();
    assert_eq!(windows.len(), 2);
    for (window, program) in windows.into_iter().zip([TARGET, START]) {
        let notice = window
            .iter()
            .position(|line| line.contains(&format!("program change {program} ")))
            .unwrap();
        let first_send = window
            .iter()
            .position(|line| line.contains(" OUT "))
            .unwrap();
        assert!(notice < first_send);
        assert!(window[first_send].contains(&dump_request));
    }
    assert!(
        step.result
            .iter()
            .any(|line| line.contains("Back on channel 5B"))
    );
    assert!(run.step(RIG_GUARD).outcome.passed());
    assert_eq!(rig.borrow().podxt.channel, Some(u16::from(START)));
}

#[test]
fn missing_dump_blocks_later_podxt_requests_and_guard_reports_unverified_return() {
    let rig = SimRig::shared(Faults::default());
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    person.on_prepare = Some(Box::new(move |step| {
        if step == FOOTSWITCH {
            rig.borrow_mut().next_podxt_dump = Some(TestPodxtDump::Missing);
        }
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(
        &run.step(FOOTSWITCH).outcome,
        Outcome::Failed(error) if error.contains("No 168-byte edit-buffer dump")
    ));
    let pedal = run.step(PEDAL);
    assert!(matches!(
        &pedal.outcome,
        Outcome::Failed(error) if error.contains("may still be outstanding")
    ));
    assert!(!pedal.messages.iter().any(|line| line.contains(" OUT ")));
    let guard = run.step(RIG_GUARD);
    assert!(matches!(&guard.outcome, Outcome::Failed(_)));
    assert!(
        guard
            .result
            .iter()
            .any(|line| line.contains("return could not be verified"))
    );
    assert!(
        !guard
            .messages
            .iter()
            .any(|line| line.contains(" OUT f0 00 01 0c 03 75 f7"))
    );
    assert!(guard.result.iter().any(|line| line.contains("the Variax")));
}

#[test]
fn late_split_return_dump_is_discarded_before_the_next_request() {
    let rig = SimRig::shared(Faults::default());
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    let mut windows = 0;
    person.on_prepare = Some(Box::new(move |step| {
        if step == FOOTSWITCH {
            windows += 1;
            if windows == 2 {
                // The fake tears the 168-byte dump into 16-byte packets, 1 ms
                // apart. The prefix arrives before the 2 s reply deadline.
                rig.borrow_mut().next_podxt_dump =
                    Some(TestPodxtDump::Delayed(Duration::from_millis(1995)));
            }
        }
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(
        &run.step(FOOTSWITCH).outcome,
        Outcome::Failed(error) if error.contains("No 168-byte edit-buffer dump")
    ));
    assert_discarded_before_send(run.step(PEDAL));
    assert!(run.step(RIG_GUARD).outcome.passed());
}

#[test]
fn intervening_channel_notice_rejects_the_read_and_quarantines_its_reply() {
    let rig = SimRig::shared(Faults::default());
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    person.on_prepare = Some(Box::new(move |step| {
        if step == FOOTSWITCH {
            rig.borrow_mut().next_podxt_dump = Some(TestPodxtDump::Interrupted {
                channel: START,
                delay: Duration::from_millis(600),
            });
        }
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(
        &run.step(FOOTSWITCH).outcome,
        Outcome::Failed(error) if error.contains("5B") && error.contains("interrupted")
    ));
    assert_discarded_before_send(run.step(PEDAL));
    assert!(run.step(RIG_GUARD).outcome.passed());
}

fn assert_discarded_before_send(step: &crate::rig_check::record::StepRecord) {
    let discard = step
        .messages
        .iter()
        .position(|line| line.contains("Discarded a late PODxt edit-buffer dump"))
        .unwrap();
    let send = step
        .messages
        .iter()
        .position(|line| line.contains(" OUT "))
        .unwrap();
    assert!(discard < send, "{step:?}");
}

#[test]
fn settled_recall_and_return_reject_changed_stable_bytes_with_the_same_name() {
    for channel in [TARGET, START] {
        let rig = SimRig::shared(Faults::default());
        let mut port = PracticePort::new(rig.clone());
        let mut person = Script::run_all();
        let mut windows = 0;
        person.on_prepare = Some(Box::new(move |step| {
            if step == FOOTSWITCH {
                windows += 1;
                if (windows == 1 && channel == TARGET) || (windows == 2 && channel == START) {
                    // Presence is a stable byte under Rig Check's comparison.
                    rig.borrow_mut().podxt.stored[usize::from(channel)][32 + 21] ^= 1;
                }
            }
        }));
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        let step = run.step(FOOTSWITCH);
        assert!(matches!(&step.outcome, Outcome::Failed(_)));
        assert!(
            step.comparisons
                .iter()
                .any(|diff| diff.summary.contains("presence"))
        );
    }
}

#[test]
fn stop_during_readback_leaves_quarantine_for_the_final_guard() {
    let rig = SimRig::shared(Faults::default());
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    let stop_after = Rc::new(Cell::new(None));
    let armed = stop_after.clone();
    let clear_stop = stop_after.clone();
    person.on_card = Some(Box::new(move |step| {
        if step == RIG_GUARD {
            clear_stop.set(None);
        }
    }));
    let wire = rig.clone();
    let mut windows = 0;
    person.on_prepare = Some(Box::new(move |step| {
        if step == FOOTSWITCH {
            windows += 1;
            if windows == 2 {
                let mut model = wire.borrow_mut();
                model.next_podxt_dump = Some(TestPodxtDump::Delayed(Duration::from_millis(500)));
                armed.set(Some(model.written.len() + 1));
            }
        }
    }));
    person.stop_when = Some(Box::new(move || {
        stop_after
            .get()
            .is_some_and(|count| rig.borrow().written.len() >= count)
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(&run.step(FOOTSWITCH).outcome, Outcome::Stopped(_)));
    let guard = run.step(RIG_GUARD);
    assert_discarded_before_send(guard);
    assert!(guard.outcome.passed(), "{guard:?}");
}

#[test]
fn guard_describes_an_accepted_live_position_change_without_claiming_byte_equality() {
    let rig = SimRig::shared(Faults::default());
    let mut port = PracticePort::new(rig.clone());
    let mut person = Script::run_all();
    person.on_card = Some(Box::new(move |step| {
        if step == RIG_GUARD {
            rig.borrow_mut().podxt.playing[32 + 7] ^= 1;
        }
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    let guard = run.step(RIG_GUARD);
    assert!(guard.outcome.passed(), "{guard:?}");
    assert!(
        guard
            .result
            .iter()
            .any(|line| line.contains("Live controller positions differ"))
    );
    assert!(
        !guard
            .result
            .iter()
            .any(|line| line.contains("all 168 bytes"))
    );
}

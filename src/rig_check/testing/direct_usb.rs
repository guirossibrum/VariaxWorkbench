use super::*;
use crate::rig_check::{
    catalogue::{RIG_GUARD, START_NOTICE, VARIAX_EVIDENCE},
    computer::{FileSource, UsbDevice},
    port::{DirectUsb, Interface, PracticePort, RigPort},
    practice::{Faults, SimLink, SimRig, UserAction},
    run_on, run_practice,
    runner::NoReport,
    steps::RunData,
};
use line6_devices::{identity::REQUEST, session::Link};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Claim(Interface),
    Finish(Interface),
    Drop(Interface),
}

#[derive(Default)]
struct Bus {
    events: Vec<Event>,
    claimed: Option<Interface>,
    fail_release: Option<Interface>,
    incompatible: Option<Interface>,
}

struct FakeHandle {
    link: SimLink,
    side: Interface,
    bus: Rc<RefCell<Bus>>,
}

impl FakeHandle {
    fn finish(&mut self) -> Result<(), String> {
        let mut bus = self.bus.borrow_mut();
        assert_eq!(bus.claimed, Some(self.side));
        bus.events.push(Event::Finish(self.side));
        if bus.fail_release == Some(self.side) {
            return Err("simulated USB release error".into());
        }
        Ok(())
    }
}

impl Drop for FakeHandle {
    fn drop(&mut self) {
        let mut bus = self.bus.borrow_mut();
        assert_eq!(bus.claimed.take(), Some(self.side));
        assert_eq!(bus.events.last(), Some(&Event::Finish(self.side)));
        bus.events.push(Event::Drop(self.side));
    }
}

struct Port {
    practice: PracticePort,
    rig: Rc<RefCell<SimRig>>,
    bus: Rc<RefCell<Bus>>,
    usb: DirectUsb<FakeHandle>,
}

impl Port {
    fn new(faults: Faults) -> Self {
        let rig = SimRig::shared(faults);
        Self {
            practice: PracticePort::new(rig.clone()),
            rig,
            bus: Rc::default(),
            usb: DirectUsb::default(),
        }
    }

    fn link(&mut self, side: Interface) -> Result<&mut dyn Link, String> {
        let handle = self.usb.link(
            side,
            || {
                let faults = &self.rig.borrow().faults;
                if faults.podxt_absent {
                    return Err("Expected one PODxt Live on USB, found 0.".into());
                }
                if faults.interface_busy {
                    return Err("Resource busy".into());
                }
                if faults.permission_denied {
                    return Err("Access denied".into());
                }
                let mut bus = self.bus.borrow_mut();
                if bus.incompatible == Some(side) {
                    return Err("PODxt Live interface has unexpected endpoints".into());
                }
                assert!(bus.claimed.replace(side).is_none());
                bus.events.push(Event::Claim(side));
                let link = match side {
                    Interface::Podxt => SimLink::podxt(&self.rig),
                    Interface::Variax => SimLink::variax(&self.rig),
                };
                Ok(FakeHandle {
                    link,
                    side,
                    bus: self.bus.clone(),
                })
            },
            FakeHandle::finish,
        )?;
        Ok(&mut handle.link)
    }
}

impl RigPort for Port {
    fn mode(&self) -> Mode {
        Mode::Practice
    }

    fn direct_usb(&self) -> bool {
        true
    }

    fn direct_usb_inventory(&mut self) -> Result<Vec<UsbDevice>, String> {
        if self.rig.borrow().faults.podxt_absent {
            return Ok(Vec::new());
        }
        Ok(vec![UsbDevice {
            port: "1-2".into(),
            address: Some(5),
            vendor: 0x0e41,
            product: 0x4650,
            name: Some("PODxt Live".into()),
            manufacturer: None,
            serial: None,
            speed: None,
            bus: None,
            interfaces: Vec::new(),
            node: None,
        }])
    }

    fn podxt(&mut self, _step: StepId) -> Result<&mut dyn Link, String> {
        self.link(Interface::Podxt)
    }

    fn variax(&mut self, _step: StepId) -> Result<&mut dyn Link, String> {
        self.link(Interface::Variax)
    }

    fn variax_claimed(&self) -> bool {
        self.usb.active() == Some(Interface::Variax)
    }

    fn release_variax(&mut self) -> Result<Vec<String>, String> {
        assert!(self.variax_claimed());
        self.finish_direct_usb()
    }

    fn direct_usb_release_failed(&self) -> bool {
        self.usb.release_failed()
    }

    fn finish_direct_usb(&mut self) -> Result<Vec<String>, String> {
        self.usb.release(FakeHandle::finish)
    }

    fn system(&self) -> Box<dyn FileSource + '_> {
        self.practice.system()
    }

    fn user_acts(&mut self, action: UserAction, after: Duration) {
        self.practice.user_acts(action, after);
    }

    fn simulation(&self) -> Option<Rc<RefCell<SimRig>>> {
        Some(self.rig.clone())
    }
}

#[test]
fn normal_catalogue_uses_the_same_cards_and_device_transcript() {
    let mut port = Port::new(Faults::default());
    let evidence = tempfile::tempdir().unwrap();
    let data = RunData {
        evidence_dir: Some(evidence.path().to_owned()),
        ..RunData::default()
    };
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, data);
    let mut linux_person = Script::run_all();
    let (linux, _, rig) = run_practice(
        Faults::default(),
        &mut linux_person,
        &mut NoReport,
        RunData::default(),
    );
    assert_eq!(person.notices, [START_NOTICE]);
    assert_eq!(person.cards, linux_person.cards);
    assert_eq!(person.saves, linux_person.saves);
    for id in crate::rig_check::catalogue::RUN_ORDER {
        assert_eq!(run.step(*id).outcome, linux.step(*id).outcome, "{id}");
    }
    assert_eq!(port.rig.borrow().written, rig.borrow().written);
    assert!(run.rig_state.contains("interface 1 released."));
    assert!(
        std::fs::read_dir(evidence.path()).unwrap().count() > 2,
        "the normal backups and device evidence were saved"
    );

    let bus = port.bus.borrow();
    assert_eq!(bus.claimed, None);
    let (claims, remainder) = bus.events.as_chunks::<3>();
    assert!(remainder.is_empty());
    let sides: Vec<_> = claims
        .iter()
        .map(|events| {
            let Event::Claim(side) = events[0] else {
                panic!("{events:?}")
            };
            assert_eq!(
                *events,
                [Event::Claim(side), Event::Finish(side), Event::Drop(side)]
            );
            side
        })
        .collect();
    assert_eq!(
        sides[..3],
        [Interface::Podxt, Interface::Variax, Interface::Podxt]
    );
    assert_eq!(sides.last(), Some(&Interface::Variax));
}

#[test]
fn missing_busy_and_denied_devices_send_nothing_and_never_offer_a_write() {
    for (faults, failed) in [
        (
            Faults {
                podxt_absent: true,
                ..Faults::default()
            },
            StepId(1, 1),
        ),
        (
            Faults {
                interface_busy: true,
                ..Faults::default()
            },
            StepId(2, 1),
        ),
        (
            Faults {
                permission_denied: true,
                ..Faults::default()
            },
            StepId(2, 1),
        ),
    ] {
        let mut port = Port::new(faults);
        let mut person = Script::run_all();
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        assert!(!run.step(failed).outcome.passed());
        assert!(port.bus.borrow().events.is_empty());
        assert!(port.rig.borrow().written.is_empty());
        assert!(person.saves.is_empty());
        assert!(!person.cards.contains(&StepId(3, 1)));
        assert!(!person.cards.contains(&StepId(4, 1)));
    }
}

#[test]
fn unverified_relay_offers_only_the_approved_read_only_evidence() {
    let mut port = Port::new(Faults {
        unverified_podxt: true,
        ..Faults::default()
    });
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(run.step(StepId(2, 1)).outcome.passed());
    assert!(run.step(VARIAX_EVIDENCE).outcome.passed());
    assert!(!person.cards.contains(&StepId(3, 1)));
    assert!(person.saves.is_empty());
    assert!(run.step(RIG_GUARD).messages.is_empty());
    assert_eq!(port.rig.borrow().podxt.stores, 0);
    assert_eq!(port.rig.borrow().variax.stores, 0);
    assert_eq!(port.rig.borrow().variax.working_uploads, 0);
    assert!(run.rig_state.contains("interface 0 released."));
}

#[test]
fn switch_cleanup_failure_stops_before_claiming_the_next_interface() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().fail_release = Some(Interface::Podxt);
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert_eq!(
        port.bus.borrow().events,
        [
            Event::Claim(Interface::Podxt),
            Event::Finish(Interface::Podxt),
            Event::Drop(Interface::Podxt),
        ]
    );
    assert_eq!(port.rig.borrow().written, [("PodxtMidi", REQUEST.to_vec())]);
    assert!(matches!(run.step(StepId(1, 2)).outcome, Outcome::Failed(_)));
    assert!(!person.cards.contains(&StepId(2, 2)));
    assert!(person.saves.is_empty());
    assert!(
        run.rig_state
            .contains("USB cleanup failed: PODxt Live interface 0")
    );
}

#[test]
fn stopping_after_podxt_identity_releases_interface_zero() {
    let mut port = Port::new(Faults::default());
    let mut person = Script::run_all();
    person.overrides = vec![(StepId(1, 2), Decision::Stop), (RIG_GUARD, Decision::Skip)];
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert_eq!(port.rig.borrow().written, [("PodxtMidi", REQUEST.to_vec())]);
    assert_eq!(port.bus.borrow().claimed, None);
    assert!(run.rig_state.contains("interface 0 released."));
}
#[test]
fn final_cleanup_failure_fails_the_guard_and_releases_the_handle() {
    let mut port = Port::new(Faults::default());
    let bus = port.bus.clone();
    let mut person = Script::run_all();
    person.on_card = Some(Box::new(move |id| {
        if id == RIG_GUARD {
            bus.borrow_mut().fail_release = Some(Interface::Variax);
        }
    }));
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(run.step(RIG_GUARD).outcome, Outcome::Failed(_)));
    assert!(
        run.rig_state
            .contains("USB cleanup failed: PODxt Live interface 1")
    );
    assert_eq!(port.bus.borrow().claimed, None);
}

#[test]
fn crash_restore_cleans_up_both_interfaces_and_keeps_state_pending_if_release_fails() {
    use crate::rig_check::record::sha256_hex;
    use crate::rig_check::recover::{RigState, offer_restore, write_state};

    for fail in [false, true] {
        let mut port = Port::new(Faults::default());
        if fail {
            port.bus.borrow_mut().fail_release = Some(Interface::Variax);
        }
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("pending.state");
        let state = {
            let initial = port.rig.borrow();
            let preset = initial.variax.selected.unwrap();
            RigState {
                podxt_channel: initial.podxt.channel,
                podxt_tone_sha256: Some(sha256_hex(&initial.podxt.playing)),
                variax_preset: Some((preset.bank(), preset.slot())),
                variax_match_sha256: crate::rig_check::recover::match_hash(&initial.variax.active),
                ..RigState::default()
            }
        };
        write_state(&state_path, &state).unwrap();
        let run = offer_restore(
            &mut port,
            &mut Script::run_all(),
            &mut NoReport,
            &state_path,
            &state,
        );
        assert_eq!(
            run.step(RIG_GUARD).outcome.passed(),
            !fail,
            "{:?}",
            run.rig_state
        );
        assert_eq!(
            port.bus.borrow().events,
            [
                Event::Claim(Interface::Podxt),
                Event::Finish(Interface::Podxt),
                Event::Drop(Interface::Podxt),
                Event::Claim(Interface::Variax),
                Event::Finish(Interface::Variax),
                Event::Drop(Interface::Variax),
            ]
        );
        let recorded = RigState::parse(&std::fs::read_to_string(state_path).unwrap()).unwrap();
        assert_eq!(recorded.finished, !fail);
    }
}

#[test]
fn incompatible_guitar_interface_cannot_reach_a_guitar_procedure_or_save() {
    let mut port = Port::new(Faults::default());
    port.bus.borrow_mut().incompatible = Some(Interface::Variax);
    let mut person = Script::run_all();
    let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
    assert!(matches!(
        &run.step(StepId(1, 2)).outcome,
        Outcome::Failed(error) if error.contains("unexpected endpoints")
    ));
    assert!(!person.cards.contains(&StepId(3, 1)));
    assert!(!person.cards.contains(&StepId(4, 2)));
    assert!(person.saves.is_empty());
    assert!(
        port.rig
            .borrow()
            .written
            .iter()
            .all(|(path, _)| *path == "PodxtMidi")
    );
    assert_eq!(port.bus.borrow().claimed, None);
}

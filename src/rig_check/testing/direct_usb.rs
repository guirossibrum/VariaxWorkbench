use super::*;
use crate::rig_check::{
    catalogue::{RIG_GUARD, START_NOTICE, VARIAX_EVIDENCE},
    computer::{
        FileSource, UsbView,
        other::{MacUsbAttachment, mac_usb_view},
    },
    port::{DirectUsb, Interface, PracticePort, RigPort},
    practice::{Faults, SimLink, SimRig, UserAction},
    run_on, run_practice,
    runner::NoReport,
    steps::RunData,
};
use line6_devices::{
    identity::REQUEST,
    session::{Link, Path},
};
use std::{cell::RefCell, rc::Rc, time::Duration};

mod save_preflight;
use save_preflight::SaveTrace;

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
    save: SaveTrace,
}

struct FakeHandle {
    link: SimLink,
    side: Interface,
    step: StepId,
    scripted_reply: Option<Option<Vec<u8>>>,
    bus: Rc<RefCell<Bus>>,
}

impl FakeHandle {
    fn finish(&mut self) -> Result<(), String> {
        let mut bus = self.bus.borrow_mut();
        assert_eq!(bus.claimed, Some(self.side));
        bus.events.push(Event::Finish(self.side));
        let save_preflight = self.side == Interface::Variax
            && self.step == crate::rig_check::variax_save::OWN_BYTES_STEP;
        if save_preflight {
            bus.save.releases += 1;
        }
        if bus.fail_release == Some(self.side) || (save_preflight && bus.save.fail_release) {
            return Err("simulated USB release error".into());
        }
        Ok(())
    }
}

impl Link for FakeHandle {
    fn path(&self) -> Path {
        self.link.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.step == crate::rig_check::variax_save::OWN_BYTES_STEP
            && self.side == Interface::Variax
        {
            let mut bus = self.bus.borrow_mut();
            self.scripted_reply = bus.save.before_write(bytes)?;
            if self.scripted_reply.is_some() {
                return Ok(());
            }
        }
        self.link.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        match &mut self.scripted_reply {
            Some(reply) => {
                if reply.is_none() {
                    self.link.pause(wait);
                }
                Ok(reply.take())
            }
            None => self.link.read(wait),
        }
    }

    fn elapsed(&self) -> Duration {
        self.link.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.link.pause(duration);
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

pub(in crate::rig_check) struct Port {
    practice: PracticePort,
    rig: Rc<RefCell<SimRig>>,
    bus: Rc<RefCell<Bus>>,
    usb: DirectUsb<FakeHandle>,
    inventory_errors: usize,
}

impl Port {
    fn new(faults: Faults) -> Self {
        Self::with_rig(SimRig::shared(faults))
    }

    pub(in crate::rig_check) fn with_rig(rig: Rc<RefCell<SimRig>>) -> Self {
        Self {
            practice: PracticePort::new(rig.clone()),
            rig,
            bus: Rc::default(),
            usb: DirectUsb::default(),
            inventory_errors: 0,
        }
    }

    pub(in crate::rig_check) fn fail_podxt_release(&mut self) {
        self.bus.borrow_mut().fail_release = Some(Interface::Podxt);
    }

    fn link(&mut self, side: Interface, step: StepId) -> Result<&mut dyn Link, String> {
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
                    step,
                    scripted_reply: None,
                    bus: self.bus.clone(),
                })
            },
            FakeHandle::finish,
        )?;
        if handle.step != step {
            handle.step = step;
            handle.scripted_reply = None;
        }
        Ok(handle)
    }
}

impl RigPort for Port {
    fn mode(&self) -> Mode {
        Mode::Practice
    }

    fn direct_usb(&self) -> bool {
        true
    }

    fn direct_usb_inventory(&mut self) -> Result<UsbView, String> {
        if self.inventory_errors > 0 {
            self.inventory_errors -= 1;
            return Err("simulated IOKit inventory unavailable".into());
        }
        let attached = (!self.rig.borrow().faults.podxt_absent).then_some(MacUsbAttachment {
            route: Some("1-2"),
            location_id: Some(0x01200000),
            address: Some(5),
            midi_interfaces_present: true,
        });
        Ok(mac_usb_view(attached))
    }

    fn podxt(&mut self, step: StepId) -> Result<&mut dyn Link, String> {
        self.link(Interface::Podxt, step)
    }

    fn variax(&mut self, step: StepId) -> Result<&mut dyn Link, String> {
        self.link(Interface::Variax, step)
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
fn computer_report_preserves_passive_usb_evidence_and_inventory_errors() {
    let location = "location ID 0x01200000, port 1-2, address 5";
    for (connected, inventory_errors, repeat, expected) in [
        (true, 0, false, location),
        (false, 0, false, "none detected"),
        (true, 1, false, "not available on this OS"),
        (true, 1, true, location),
    ] {
        let mut port = Port::new(Faults {
            podxt_absent: !connected,
            ..Faults::default()
        });
        port.inventory_errors = inventory_errors;
        let mut person = Script::run_all();
        person.default = Decision::Skip;
        person.overrides = vec![(StepId(0, 2), Decision::Run)];
        person.repeat_once = repeat.then_some(StepId(0, 2));
        let (run, _) = run_on(&mut port, &mut person, &mut NoReport, RunData::default());
        let inventory = run.step(StepId(0, 2));
        let inventory_failed = inventory_errors > 0 && !repeat;
        assert_eq!(inventory.outcome.passed(), !inventory_failed);
        if repeat {
            assert_eq!(run.prior_attempts.len(), 1);
            assert_eq!(
                run.prior_attempts[0].outcome.reason(),
                Some("simulated IOKit inventory unavailable")
            );
        }
        if inventory_failed {
            assert_eq!(
                inventory.outcome.reason(),
                Some("simulated IOKit inventory unavailable")
            );
        }
        let mut rows = run
            .computer
            .iter()
            .filter(|(name, _)| name == "PODxt Live USB");
        let usb = rows.next().unwrap();
        assert!(rows.next().is_none());
        assert!(usb.1.contains(expected));
        assert!(inventory.result.iter().any(|line| line.contains(expected)));
        let plain = crate::rig_check::report::render_plain(&run, "fixture");
        assert!(plain.contains(&format!("PODxt Live USB: {}", usb.1)));
        assert!(plain.contains("Device transport: Direct USB"));
        assert!(plain.contains("Firmware installation: unsupported"));
        assert!(inventory.messages.is_empty());
        assert!(port.bus.borrow().events.is_empty());
    }
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
        assert_eq!(recorded.recovery_completed, !fail);
        assert_eq!(recorded.finished, state.finished);
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

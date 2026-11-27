use super::*;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

const PODXT: Attachment = Attachment {
    registry_id: 21,
    location_id: 0x0120_0000,
    address: 5,
    vendor: 0x0e41,
    product: 0x4650,
    configurations: 1,
};
const EXCLUSIVE_ACCESS: u32 = 0xe000_02c5;
const NOT_PERMITTED: u32 = 0xe000_02e2;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Lock,
    Snapshot,
    AdvertisedDescriptor,
    Open,
    GetConfiguration,
    Activate,
    Close,
    Wait(Duration),
    ActiveDescriptor,
    Unlock,
}

struct Script {
    events: Vec<Event>,
    locked: bool,
    opened: bool,
    activated: bool,
    snapshot_count: usize,
    snapshot_overrides: BTreeMap<usize, Snapshot>,
    snapshot: Snapshot,
    interfaces_after_activation: u8,
    advertised: Vec<u8>,
    open_error: Option<Stop>,
    fresh: Result<u8, Stop>,
    configuration_length: usize,
    activation_error: Option<Stop>,
    close_error: Option<u32>,
    active: Result<Vec<u8>, Stop>,
}

impl Script {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            locked: false,
            opened: false,
            activated: false,
            snapshot_count: 0,
            snapshot_overrides: BTreeMap::new(),
            snapshot: Snapshot {
                attachment: PODXT,
                registered_client: false,
                conflicting_binding: false,
                interfaces: 0,
            },
            interfaces_after_activation: BOTH_INTERFACES,
            advertised: descriptor(),
            open_error: None,
            fresh: Ok(0),
            configuration_length: 1,
            activation_error: None,
            close_error: None,
            active: Ok(descriptor()),
        }
    }

    fn calls(&self, event: Event) -> usize {
        self.events.iter().filter(|entry| **entry == event).count()
    }
}

fn descriptor() -> Vec<u8> {
    vec![
        9, 2, 55, 0, 2, 1, 0, 0x80, 50, //
        9, 4, 0, 1, 2, 0xff, 0, 0, 0, //
        7, 5, 0x03, 3, 16, 0, 5, //
        7, 5, 0x84, 3, 16, 0, 5, //
        9, 4, 1, 1, 2, 0xff, 0, 0, 0, //
        7, 5, 0x05, 3, 16, 0, 5, //
        7, 5, 0x86, 3, 16, 0, 5,
    ]
}

struct Lock(Rc<RefCell<Script>>);

impl Drop for Lock {
    fn drop(&mut self) {
        let mut script = self.0.borrow_mut();
        assert!(script.locked);
        assert!(!script.opened);
        script.locked = false;
        script.events.push(Event::Unlock);
    }
}

struct FakeTransport {
    state: Rc<RefCell<Script>>,
    advertised: Vec<u8>,
    active: Result<Vec<u8>, Stop>,
}

impl Transport for FakeTransport {
    type Handle = FakeHandle;

    fn snapshot(&mut self) -> Result<Snapshot, Stop> {
        let mut script = self.state.borrow_mut();
        assert!(script.locked);
        script.events.push(Event::Snapshot);
        script.snapshot_count += 1;
        if let Some(snapshot) = script.snapshot_overrides.get(&script.snapshot_count) {
            return Ok(*snapshot);
        }
        let mut snapshot = script.snapshot;
        if script.activated && !script.opened {
            snapshot.interfaces = script.interfaces_after_activation;
        }
        Ok(snapshot)
    }

    fn advertised_descriptor(&mut self) -> &[u8] {
        let mut script = self.state.borrow_mut();
        assert!(script.locked && !script.opened);
        script.events.push(Event::AdvertisedDescriptor);
        &self.advertised
    }

    fn open_nonseizing(&mut self) -> Result<Self::Handle, Stop> {
        let mut script = self.state.borrow_mut();
        assert!(script.locked && !script.opened);
        script.events.push(Event::Open);
        if let Some(error) = script.open_error {
            return Err(error);
        }
        script.opened = true;
        Ok(FakeHandle(Rc::clone(&self.state)))
    }

    fn refreshed_active_descriptor(&mut self) -> Result<&[u8], Stop> {
        let mut script = self.state.borrow_mut();
        assert!(script.locked && !script.opened);
        script.events.push(Event::ActiveDescriptor);
        self.active.as_deref().map_err(|error| *error)
    }
}

struct FakeHandle(Rc<RefCell<Script>>);

impl DeviceHandle for FakeHandle {
    fn get_configuration(&mut self) -> Result<ConfigurationReply, Stop> {
        let mut script = self.0.borrow_mut();
        assert!(script.locked && script.opened);
        assert_eq!(script.calls(Event::GetConfiguration), 0);
        script.events.push(Event::GetConfiguration);
        script.fresh.map(|value| ConfigurationReply {
            value,
            bytes_read: script.configuration_length,
        })
    }

    fn activate_one_with_interface_matching(&mut self) -> Result<(), Stop> {
        let mut script = self.0.borrow_mut();
        assert!(script.locked && script.opened);
        assert_eq!(script.calls(Event::GetConfiguration), 1);
        assert_eq!(script.fresh, Ok(0));
        assert_eq!(script.configuration_length, 1);
        assert_eq!(script.calls(Event::Activate), 0);
        script.events.push(Event::Activate);
        if let Some(error) = script.activation_error {
            return Err(error);
        }
        script.activated = true;
        Ok(())
    }

    fn close(self) -> Result<(), u32> {
        let mut script = self.0.borrow_mut();
        assert!(script.locked && script.opened);
        script.opened = false;
        script.events.push(Event::Close);
        script.close_error.map_or(Ok(()), Err)
    }
}

fn run_script(script: Script, approval: Approval) -> (Report, Result<(), Failure>, Vec<Event>) {
    run_selection(script, PODXT, approval)
}

fn run_selection(
    script: Script,
    expected: Attachment,
    approval: Approval,
) -> (Report, Result<(), Failure>, Vec<Event>) {
    let advertised = script.advertised.clone();
    let active = script.active.clone();
    let state = Rc::new(RefCell::new(script));
    let (report, result) = run(
        &mut FakeTransport {
            state: Rc::clone(&state),
            advertised,
            active,
        },
        expected,
        approval,
        || {
            let mut script = state.borrow_mut();
            assert!(!script.locked);
            script.locked = true;
            script.events.push(Event::Lock);
            Ok(Lock(Rc::clone(&state)))
        },
        |duration| {
            let mut script = state.borrow_mut();
            assert!(script.locked && !script.opened);
            script.events.push(Event::Wait(duration));
        },
    );
    let events = std::mem::take(&mut state.borrow_mut().events);
    (report, result, events)
}

#[test]
fn visible_chrome_client_and_no_client_take_the_same_approved_path() {
    for registered in [false, true] {
        let mut script = Script::new();
        script.snapshot.registered_client = registered;
        let (report, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Ok(()));
        assert_eq!(report.registered_client_seen, registered);
        assert_eq!(report.fresh_configuration, Some(0));
        assert!(report.activation_accepted);
        assert_eq!(report.interfaces_after_close, Some(BOTH_INTERFACES));
        assert!(report.active_descriptor_verified);
        assert_eq!(
            events,
            [
                Event::Lock,
                Event::Snapshot,
                Event::AdvertisedDescriptor,
                Event::Snapshot,
                Event::Open,
                Event::Snapshot,
                Event::GetConfiguration,
                Event::Snapshot,
                Event::Activate,
                Event::Close,
                Event::Snapshot,
                Event::ActiveDescriptor,
                Event::Snapshot,
                Event::Unlock,
            ]
        );
    }
}

#[test]
fn os_open_refusals_preserve_the_phase_and_make_no_read_or_write() {
    for error in [
        Stop::OpenBusy(EXCLUSIVE_ACCESS),
        Stop::OpenRefused(NOT_PERMITTED),
    ] {
        let mut script = Script::new();
        script.snapshot.registered_client = true;
        script.open_error = Some(error);
        let (report, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(error.into()));
        assert!(report.registered_client_seen);
        assert_eq!(
            events,
            [
                Event::Lock,
                Event::Snapshot,
                Event::AdvertisedDescriptor,
                Event::Snapshot,
                Event::Open,
                Event::Unlock
            ]
        );
    }
}

#[test]
fn a_published_conflict_stops_before_open_or_activation() {
    for phase in [1, 3, 4] {
        let mut script = Script::new();
        script.snapshot_overrides.insert(
            phase,
            Snapshot {
                conflicting_binding: true,
                ..script.snapshot
            },
        );
        let (_, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(Stop::PublishedConflict.into()));
        assert_eq!(events.contains(&Event::Open), phase >= 3);
        assert_eq!(events.contains(&Event::Close), phase >= 3);
        assert!(!events.contains(&Event::Activate));
    }
}

#[test]
fn rejects_changed_attachment_including_same_location_and_address_with_new_generation() {
    for changed in [
        Attachment {
            registry_id: 22,
            ..PODXT
        },
        Attachment {
            address: 6,
            ..PODXT
        },
        Attachment {
            location_id: 0x0130_0000,
            ..PODXT
        },
        Attachment {
            configurations: 2,
            ..PODXT
        },
    ] {
        for phase in 1..=4 {
            let mut script = Script::new();
            script.snapshot_overrides.insert(
                phase,
                Snapshot {
                    attachment: changed,
                    ..script.snapshot
                },
            );
            let (_, result, events) = run_script(script, Approval::ActivateOnce);
            assert_eq!(result, Err(Stop::AttachmentChanged.into()));
            assert_eq!(events.contains(&Event::Open), phase >= 3);
            assert_eq!(events.contains(&Event::Close), phase >= 3);
            assert!(!events.contains(&Event::Activate));
        }
    }
}

#[test]
fn invalid_selection_and_wrong_advertisement_stop_before_open() {
    let (_, result, events) = run_selection(
        Script::new(),
        Attachment {
            product: 0x4651,
            ..PODXT
        },
        Approval::ActivateOnce,
    );
    assert_eq!(result, Err(Stop::InvalidSelection.into()));
    assert!(events.is_empty());

    for (offset, value) in [(5, 2), (12, 0), (20, 0x04), (22, 32), (54, 8)] {
        let mut script = Script::new();
        script.advertised[offset] = value;
        let (_, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(Stop::AdvertisedDescriptor.into()));
        assert!(!events.contains(&Event::Open));
        assert!(!events.contains(&Event::Activate));
    }
}

#[test]
fn configured_with_a_visible_client_revalidates_after_close_and_never_writes() {
    let mut script = Script::new();
    script.fresh = Ok(1);
    script.snapshot.registered_client = true;
    script.snapshot.interfaces = BOTH_INTERFACES;
    let (report, result, events) = run_script(script, Approval::ActivateOnce);
    assert_eq!(result, Ok(()));
    assert_eq!(report.fresh_configuration, Some(1));
    assert!(report.registered_client_seen);
    assert!(!report.activation_accepted);
    assert!(report.active_descriptor_verified);
    assert!(events.ends_with(&[
        Event::Close,
        Event::Snapshot,
        Event::ActiveDescriptor,
        Event::Snapshot,
        Event::Unlock,
    ]));
    assert!(!events.contains(&Event::Activate));
}

#[test]
fn read_only_fresh_zero_closes_without_activation() {
    let (report, result, events) = run_script(Script::new(), Approval::ReadOnly);
    assert_eq!(result, Ok(()));
    assert_eq!(report.fresh_configuration, Some(0));
    assert!(!report.activation_accepted);
    assert!(events.ends_with(&[
        Event::GetConfiguration,
        Event::Snapshot,
        Event::Close,
        Event::Unlock
    ]));
    assert!(!events.contains(&Event::Activate));
}

#[test]
fn contradictory_or_failed_reads_never_write() {
    for (pre, fresh, length, expected) in [
        (0, Ok(1), 1, Stop::InconsistentConfiguration),
        (BOTH_INTERFACES, Ok(0), 1, Stop::InconsistentConfiguration),
        (0, Ok(2), 1, Stop::InconsistentConfiguration),
        (0, Err(Stop::ConfigurationRead), 1, Stop::ConfigurationRead),
        (0, Ok(0), 0, Stop::ConfigurationRead),
        (0, Ok(0), 2, Stop::ConfigurationRead),
    ] {
        let mut script = Script::new();
        script.snapshot.interfaces = pre;
        script.fresh = fresh;
        script.configuration_length = length;
        let (_, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(expected.into()));
        assert!(events.ends_with(&[Event::Close, Event::Unlock]));
        assert!(!events.contains(&Event::Activate));
    }
}

#[test]
fn publication_changes_during_preflight_or_after_fresh_zero_prevent_activation() {
    for phase in [2, 3, 4] {
        let mut script = Script::new();
        script.snapshot_overrides.insert(
            phase,
            Snapshot {
                interfaces: BOTH_INTERFACES,
                ..script.snapshot
            },
        );
        let (_, result, events) = run_script(script, Approval::ActivateOnce);
        let expected = if phase == 4 {
            Stop::InconsistentConfiguration
        } else {
            Stop::InvalidInterfaceView
        };
        assert_eq!(result, Err(expected.into()));
        assert!(!events.contains(&Event::Activate));
    }
}

#[test]
fn host_refusal_is_once_and_close_failure_is_kept() {
    let mut script = Script::new();
    let refusal = Stop::ActivationRefused(EXCLUSIVE_ACCESS);
    script.activation_error = Some(refusal);
    script.close_error = Some(NOT_PERMITTED);
    let (report, result, events) = run_script(script, Approval::ActivateOnce);
    assert_eq!(
        result,
        Err(Failure {
            stopped: Some(refusal),
            close_error: Some(NOT_PERMITTED)
        })
    );
    assert!(!report.activation_accepted);
    assert!(events.ends_with(&[Event::Activate, Event::Close, Event::Unlock]));
}

#[test]
fn accepted_activation_with_close_error_stops_before_observation() {
    let mut script = Script::new();
    script.close_error = Some(NOT_PERMITTED);
    let (report, result, events) = run_script(script, Approval::ActivateOnce);
    assert_eq!(
        result,
        Err(Failure {
            stopped: None,
            close_error: Some(NOT_PERMITTED)
        })
    );
    assert!(report.activation_accepted);
    assert_eq!(report.interfaces_after_close, None);
    assert!(events.ends_with(&[Event::Activate, Event::Close, Event::Unlock]));
}

#[test]
fn partial_publication_has_a_bounded_observation_window() {
    let mut script = Script::new();
    script.interfaces_after_activation = 1;
    let (report, result, events) = run_script(script, Approval::ActivateOnce);
    assert_eq!(result, Err(Stop::PublicationIncomplete.into()));
    assert!(report.activation_accepted);
    assert_eq!(report.interfaces_after_close, Some(1));
    assert!(!events.contains(&Event::ActiveDescriptor));
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::Wait(OBSERVATION_DELAY))
            .count(),
        5
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::Activate)
            .count(),
        1
    );
}

#[test]
fn interface_publication_does_not_pass_a_stale_or_mismatched_active_descriptor() {
    for active in [Err(Stop::ActiveDescriptorUnavailable), Ok(vec![9, 2])] {
        let mut script = Script::new();
        script.active = active;
        let (report, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(Stop::ActiveDescriptorUnavailable.into()));
        assert!(report.activation_accepted);
        assert_eq!(report.interfaces_after_close, Some(BOTH_INTERFACES));
        assert!(!report.active_descriptor_verified);
        assert!(events.ends_with(&[
            Event::Close,
            Event::Snapshot,
            Event::ActiveDescriptor,
            Event::Unlock
        ]));
    }
}

#[test]
fn revalidation_refuses_replacement_during_publication_or_descriptor_refresh() {
    for phase in [5, 6] {
        let mut script = Script::new();
        script.snapshot_overrides.insert(
            phase,
            Snapshot {
                attachment: Attachment {
                    registry_id: 22,
                    ..PODXT
                },
                interfaces: BOTH_INTERFACES,
                ..script.snapshot
            },
        );
        let (report, result, events) = run_script(script, Approval::ActivateOnce);
        assert_eq!(result, Err(Stop::AttachmentChanged.into()));
        assert!(report.activation_accepted);
        assert!(!report.active_descriptor_verified);
        assert_eq!(events.contains(&Event::ActiveDescriptor), phase == 6);
    }
}

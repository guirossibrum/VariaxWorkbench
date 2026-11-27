use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{cell::Cell, rc::Rc, sync::mpsc};
use variax_workbench::device_probe::{OsMetadataSource, SyntheticSysfs};

#[derive(Clone, Copy, PartialEq, Eq)]
enum InventoryState {
    Ready,
    MissingIterator,
    InterfacesUnavailable,
}

struct UsbFixture(SyntheticSysfs, Rc<Cell<InventoryState>>, Option<u64>);

impl PresenceSource for UsbFixture {
    fn metadata(&self) -> &dyn OsMetadataSource {
        &self.0
    }

    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        if self.1.get() == InventoryState::MissingIterator {
            return Err(MetadataError::Unreadable(
                "missing registry iterator".into(),
            ));
        }
        self.0.usb_devices()
    }

    fn usb_open_candidate(&self, _: &str) -> bool {
        self.1.get() == InventoryState::Ready
    }

    fn usb_address(&self, _: &str) -> Option<u8> {
        Some(10)
    }

    fn usb_registry_id(&self, _: &str) -> Option<u64> {
        self.2
    }
}

#[test]
fn cadence_does_not_overlap_an_active_worker_and_quit_waits_for_its_release() {
    fn wait_for_quit(_: &Job, cancel: &AtomicBool) -> Outcome {
        while !cancel.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Unavailable("Identification cancelled.".into()),
        }
    }

    let mut usb = SyntheticSysfs::new();
    usb.add_device("1-1", 0x0e41, 0x4650, "PODxt Live", &[]);
    let mut service = DetectionService::new(Box::new(UsbFixture(
        usb,
        Rc::new(Cell::new(InventoryState::Ready)),
        Some(30),
    )));
    service.identify = Some(wait_for_quit);
    let start = Instant::now();
    let profiles = super::super::registry::DEVICE_PROFILES;
    service
        .refresh_at(Scan::Automatic, profiles, start)
        .unwrap();
    for poll in 1..=20 {
        service
            .refresh_at(
                Scan::Automatic,
                profiles,
                start + MAC_DISCOVERY_INTERVAL * poll,
            )
            .unwrap();
        assert_eq!(service.worker.as_ref().unwrap().job.kind, JobKind::Initial);
        assert_eq!(
            service.next_mac_identity,
            Some(start + MAC_DISCOVERY_INTERVAL)
        );
    }

    assert!(service.cancel_for_quit());
    let (job, outcome) = service.worker.take().unwrap().finish();
    service.apply_outcome(job, outcome, start + MAC_DISCOVERY_INTERVAL * 20);
    assert!(!service.cancel_for_quit());
    service
        .refresh_at(
            Scan::Automatic,
            profiles,
            start + MAC_DISCOVERY_INTERVAL * 21,
        )
        .unwrap();
    assert!(service.worker.is_none());
    assert!(
        !service.attachments["1-1"]
            .bridge
            .as_ref()
            .unwrap()
            .pending()
    );
}

#[test]
fn revoked_mac_discovery_resumes_at_the_interrupted_stage_after_release() {
    let mut recovered = Vec::new();
    for interrupted in [
        InventoryState::MissingIterator,
        InventoryState::InterfacesUnavailable,
    ] {
        for kind in [JobKind::Initial, JobKind::DiscoverGuitar] {
            recovered.push(check_revoked(
                interrupted,
                kind,
                Outcome::MacCancelled { pod_firmware: None },
                false,
            ));
        }
    }
    assert_eq!(
        recovered,
        [Some(JobKind::Initial), Some(JobKind::DiscoverGuitar)].repeat(2)
    );
}

fn check_revoked(
    interrupted: InventoryState,
    kind: JobKind,
    outcome: Outcome,
    quit: bool,
) -> Option<JobKind> {
    let (mut service, state, job, start) = in_flight(kind);
    let later = start + Duration::from_secs(10);
    let profiles = super::super::registry::DEVICE_PROFILES;

    let (cancelled, observe_cancelled) = mpsc::channel();
    let (release, allow_release) = mpsc::channel();
    service.worker = Some(
        Worker::start(job.clone(), move |_, cancel| {
            while !cancel.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            cancelled.send(()).unwrap();
            allow_release.recv_timeout(Duration::from_secs(5)).unwrap();
            outcome
        })
        .unwrap(),
    );

    state.set(interrupted);
    let result = service.refresh_at(Scan::Automatic, profiles, later);
    assert_eq!(
        result.is_err(),
        interrupted == InventoryState::MissingIterator
    );
    observe_cancelled
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    state.set(InventoryState::Ready);
    service
        .refresh_at(Scan::Automatic, profiles, later)
        .unwrap();
    assert_eq!(service.worker.as_ref().unwrap().job, job);
    assert!(
        service
            .bridge_mut(&job.attachment)
            .unwrap()
            .queued
            .is_none()
    );

    if quit {
        assert!(service.cancel_for_quit());
    }
    release.send(()).unwrap();
    await_worker(&service);
    service
        .refresh_at(Scan::Automatic, profiles, later)
        .unwrap();
    assert!(service.worker.is_none());
    service.bridge_mut(&job.attachment).unwrap().queued
}

fn in_flight(kind: JobKind) -> (DetectionService, Rc<Cell<InventoryState>>, Job, Instant) {
    in_flight_with_registry(kind, Some(30))
}

fn in_flight_with_registry(
    kind: JobKind,
    registry_id: Option<u64>,
) -> (DetectionService, Rc<Cell<InventoryState>>, Job, Instant) {
    let mut usb = SyntheticSysfs::new();
    usb.add_device("1-1", 0x0e41, 0x4650, "PODxt Live", &[]);
    let state = Rc::new(Cell::new(InventoryState::Ready));
    let mut service = DetectionService::new(Box::new(UsbFixture(usb, state.clone(), registry_id)));
    let start = Instant::now();
    let profiles = super::super::registry::DEVICE_PROFILES;
    service
        .refresh_at(Scan::Confirmed, profiles, start)
        .unwrap();
    let attachment = service.attachments.get_mut("1-1").unwrap();
    let job = Job {
        attachment: attachment.key.clone(),
        kind,
    };
    attachment.identity_started = true;
    let bridge = attachment.bridge.as_mut().unwrap();
    bridge.queued = None;
    bridge.running = Some(kind);
    if matches!(
        kind,
        JobKind::DiscoverGuitar | JobKind::RefreshGuitar | JobKind::VerifyGuitarOnReturn
    ) {
        bridge.pod = Some("3.01");
    }

    (service, state, job, start)
}

fn await_worker(service: &DetectionService) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !service.worker.as_ref().unwrap().finished() {
        assert!(Instant::now() < deadline, "fake worker did not finish");
        std::thread::yield_now();
    }
}

#[test]
fn completed_mac_discovery_revoked_before_collection_rearms_without_publishing_proof() {
    for interrupted in [
        InventoryState::MissingIterator,
        InventoryState::InterfacesUnavailable,
    ] {
        for (kind, outcome) in [
            (JobKind::Initial, Outcome::Deferred),
            (JobKind::Initial, Outcome::OtherProcessBusy),
            (JobKind::DiscoverGuitar, Outcome::Deferred),
            (JobKind::DiscoverGuitar, Outcome::OtherProcessBusy),
            (JobKind::Initial, Outcome::MacPodSilent),
            (
                JobKind::Initial,
                Outcome::MacCancelled {
                    pod_firmware: Some("3.01"),
                },
            ),
            (
                JobKind::Initial,
                Outcome::Finished {
                    pod_firmware: Some("3.01"),
                    guitar: Guitar::Silent,
                },
            ),
            (
                JobKind::Initial,
                Outcome::Finished {
                    pod_firmware: Some("3.01"),
                    guitar: Guitar::Verified("3.10"),
                },
            ),
            (
                JobKind::DiscoverGuitar,
                Outcome::Finished {
                    pod_firmware: None,
                    guitar: Guitar::Silent,
                },
            ),
            (
                JobKind::DiscoverGuitar,
                Outcome::Finished {
                    pod_firmware: None,
                    guitar: Guitar::Verified("3.10"),
                },
            ),
        ] {
            let (service, job, rearm_at) =
                completed_then_revoked(interrupted, kind, outcome, false);
            let bridge = service.attachments["1-1"].bridge.as_ref().unwrap();
            assert_eq!(bridge.queued, Some(kind));
            assert_eq!(bridge.pod.is_some(), kind == JobKind::DiscoverGuitar);
            assert!(bridge.guitar.is_none());
            assert_eq!(service.next_mac_identity, Some(rearm_at));
            assert_eq!(service.attachments["1-1"].key, job.attachment);
        }
    }
}

#[test]
fn revoked_mac_discovery_preserves_terminal_outcomes_and_quit() {
    use variax_workbench::{firmware_notice::FirmwareNotice, identity_stage::bridge_reply};
    let mut wrong_firmware = bridge_reply();
    wrong_firmware[15] = 2;
    let reply = line6_devices::identity::Identity::parse(&wrong_firmware).unwrap();
    for (outcome, suspended, warning) in [
        (
            Outcome::Finished {
                pod_firmware: Some("3.01"),
                guitar: Guitar::Unavailable("USB read failed.".into()),
            },
            false,
            false,
        ),
        (
            Outcome::UnverifiedPod(FirmwareNotice::podxt(&reply).unwrap()),
            false,
            true,
        ),
        (
            Outcome::Finished {
                pod_firmware: None,
                guitar: Guitar::CleanupFailed,
            },
            true,
            false,
        ),
    ] {
        let (service, _, _) = completed_then_revoked(
            InventoryState::InterfacesUnavailable,
            JobKind::Initial,
            outcome,
            false,
        );
        let bridge = service.attachments["1-1"].bridge.as_ref().unwrap();
        assert!(!bridge.pending());
        assert!(bridge.pod.is_none());
        assert_eq!(bridge.suspended, suspended);
        assert_eq!(bridge.pod_warning.is_some(), warning);
    }
    assert_eq!(
        check_revoked(
            InventoryState::MissingIterator,
            JobKind::Initial,
            Outcome::MacCancelled { pod_firmware: None },
            true,
        ),
        None
    );
    assert_eq!(
        check_revoked(
            InventoryState::InterfacesUnavailable,
            JobKind::RefreshGuitar,
            Outcome::MacCancelled { pod_firmware: None },
            false,
        ),
        None
    );
    for outcome in [
        Outcome::MacPodSilent,
        Outcome::Deferred,
        Outcome::OtherProcessBusy,
    ] {
        let (service, _, _) = completed_then_revoked(
            InventoryState::MissingIterator,
            JobKind::Initial,
            outcome,
            true,
        );
        assert!(
            !service.attachments["1-1"]
                .bridge
                .as_ref()
                .unwrap()
                .pending()
        );
    }
}

fn completed_then_revoked(
    interrupted: InventoryState,
    kind: JobKind,
    outcome: Outcome,
    quit: bool,
) -> (DetectionService, Job, Instant) {
    let (mut service, state, job, now) = in_flight(kind);
    service.worker = Some(Worker::start(job.clone(), move |_, _| outcome).unwrap());
    await_worker(&service);
    let profiles = super::super::registry::DEVICE_PROFILES;
    state.set(interrupted);
    let _ = service.refresh_at(Scan::Automatic, profiles, now);
    if interrupted == InventoryState::InterfacesUnavailable {
        assert!(service.worker.is_none());
        assert!(!service.attachments["1-1"].open_candidate);
    }
    if quit {
        assert!(!service.cancel_for_quit());
    }
    state.set(InventoryState::Ready);
    service.refresh_at(Scan::Automatic, profiles, now).unwrap();
    assert!(service.worker.is_none());
    (service, job, now + MAC_DISCOVERY_INTERVAL)
}

#[test]
fn quitting_discards_completed_silence_before_it_can_rearm() {
    let (mut service, _, job, now) = in_flight(JobKind::Initial);
    service.worker = Some(Worker::start(job, |_, _| Outcome::MacPodSilent).unwrap());
    await_worker(&service);
    assert!(!service.cancel_for_quit());
    let profiles = super::super::registry::DEVICE_PROFILES;
    service
        .refresh_at(Scan::Automatic, profiles, now + MAC_DISCOVERY_INTERVAL)
        .unwrap();
    assert!(
        !service.attachments["1-1"]
            .bridge
            .as_ref()
            .unwrap()
            .pending()
    );
}

#[test]
fn revoked_busy_discovery_waits_for_both_the_cadence_and_fresh_readiness() {
    let (mut service, state, job, now) = in_flight(JobKind::DiscoverGuitar);
    service.worker = Some(Worker::start(job.clone(), |_, _| Outcome::OtherProcessBusy).unwrap());
    await_worker(&service);
    let profiles = super::super::registry::DEVICE_PROFILES;
    state.set(InventoryState::InterfacesUnavailable);
    service.refresh_at(Scan::Automatic, profiles, now).unwrap();
    assert!(service.worker.is_none());

    service.identify = Some(|_, _| Outcome::Finished {
        pod_firmware: None,
        guitar: Guitar::Verified("3.10"),
    });
    let due = now + MAC_DISCOVERY_INTERVAL;
    state.set(InventoryState::Ready);
    service
        .refresh_at(Scan::Automatic, profiles, due - Duration::from_millis(1))
        .unwrap();
    assert!(service.worker.is_none());

    state.set(InventoryState::InterfacesUnavailable);
    service.refresh_at(Scan::Automatic, profiles, due).unwrap();
    assert!(service.worker.is_none());

    state.set(InventoryState::Ready);
    service.refresh_at(Scan::Automatic, profiles, due).unwrap();
    assert_eq!(service.worker.as_ref().unwrap().job, job);
}

#[test]
fn linux_loss_revokes_in_flight_identity_and_return_waits_for_release() {
    for result in [
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Verified("3.10"),
        },
        Outcome::OtherProcessBusy,
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::CleanupFailed,
        },
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::DriverUnbound("Driver was not restored.".into()),
        },
    ] {
        let cleanup_failed = matches!(
            result,
            Outcome::Finished {
                guitar: Guitar::CleanupFailed,
                ..
            }
        );
        let driver_unbound = matches!(
            result,
            Outcome::Finished {
                guitar: Guitar::DriverUnbound(_),
                ..
            }
        );
        let (mut service, _, job, now) =
            in_flight_with_registry(JobKind::VerifyGuitarOnReturn, None);
        let profiles = super::super::registry::DEVICE_PROFILES;
        let bridge = service.bridge_mut(&job.attachment).unwrap();
        bridge.guitar = Some((42, GuitarIdentity::Verified("3.10")));
        bridge.last_guitar = Some("3.10");
        service.rebuild_devices(Scan::Automatic, profiles);
        let old_guitar = service
            .known
            .keys()
            .find(|key| key.linked_generation == Some(42))
            .unwrap()
            .clone();
        let (finish, allow_finish) = mpsc::channel();
        service.worker = Some(
            Worker::start(job.clone(), move |_, _| {
                allow_finish.recv_timeout(Duration::from_secs(5)).unwrap();
                result
            })
            .unwrap(),
        );

        let devices = service
            .report_guitar_disconnect(&old_guitar, profiles)
            .unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].key.attachment, job.attachment);
        assert_eq!(devices[0].support, DeviceSupport::Verified("3.01"));
        assert_eq!(
            devices[0].guitar_status.as_deref(),
            Some("Guitar: disconnected · last verified Variax 600 3.10")
        );
        assert!(!service.queue_guitar_refresh(&job.attachment, false));

        service.apply_linux_notices(&job.attachment, vec![Notice::Connected, Notice::Connected]);
        service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert_eq!(service.worker.as_ref().unwrap().job, job);
        assert_eq!(service.known.len(), 1);

        finish.send(()).unwrap();
        await_worker(&service);
        let devices = service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert_eq!(
            devices.len(),
            1,
            "old identity must never restore the guitar"
        );
        assert_eq!(devices[0].support, DeviceSupport::Verified("3.01"));
        let bridge = service.bridge_mut(&job.attachment).unwrap();
        assert_eq!(bridge.suspended, cleanup_failed);
        assert_eq!(bridge.driver_unbound, driver_unbound);
        assert_eq!(
            bridge.queued,
            (!cleanup_failed && !driver_unbound).then_some(JobKind::VerifyGuitarOnReturn),
            "keep exactly the return work unless release failed"
        );
    }
}

#[test]
fn linux_listener_loss_preserves_manual_dispatch_and_busy_retry() {
    for (reattach, kind) in [
        (false, JobKind::RefreshGuitar),
        (true, JobKind::ReattachGuitarDriver),
    ] {
        let (mut service, _, mut listener, now) =
            in_flight_with_registry(JobKind::RefreshGuitar, None);
        listener.kind = JobKind::Listen;
        let key = &listener.attachment;
        let bridge = service.bridge_mut(key).unwrap();
        bridge.running = None;
        bridge.driver_unbound = reattach;
        bridge.guitar = (!reattach).then_some((42, GuitarIdentity::Verified("3.10")));
        bridge.last_guitar = Some("3.10");

        let (release, allow_release) = mpsc::channel();
        service.worker = Some(
            Worker::start(listener.clone(), move |_, cancel| {
                allow_release.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(cancel.load(Ordering::Relaxed));
                Outcome::Notices(vec![Notice::Disconnected])
            })
            .unwrap(),
        );
        service.identify = Some(|_, _| Outcome::OtherProcessBusy);
        let profiles = super::super::registry::DEVICE_PROFILES;
        assert!(service.queue_guitar_refresh(key, reattach));
        service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert_eq!(service.worker.as_ref().unwrap().job, listener);

        release.send(()).unwrap();
        await_worker(&service);
        let devices = service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert_eq!(devices.len(), 1, "listener loss withdraws old guitar proof");
        assert_eq!(devices[0].support, DeviceSupport::Verified("3.01"));
        assert_eq!(
            service.worker.as_ref().unwrap().job,
            Job {
                attachment: key.clone(),
                kind,
            }
        );

        // A further loss revokes in-flight proof, even if the worker has
        // already finished but its outcome has yet to be collected.
        service.apply_linux_notices(key, vec![Notice::Connected, Notice::Disconnected]);
        await_worker(&service);
        service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert!(service.worker.is_none(), "a busy result defers dispatch");
        assert!(service.bridge_mut(key).unwrap().waiting_for_other_process);

        // Accept further notices while the busy retry is queued.
        service.apply_linux_notices(key, vec![Notice::Connected, Notice::Disconnected]);
        assert!(service.bridge_mut(key).unwrap().waiting_for_other_process);
        service.identify = Some(|_, _| Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Verified("3.10"),
        });
        service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        assert_eq!(service.worker.as_ref().unwrap().job.kind, kind);
        await_worker(&service);
        let devices = service.refresh_at(Scan::Confirmed, profiles, now).unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|device| {
            device.key.linked_generation.is_some()
                && device.key.linked_generation != Some(42)
                && device.support == DeviceSupport::Verified("3.10")
        }));
        assert!(!service.bridge_mut(key).unwrap().pending());
    }
}

#[test]
fn linux_listener_drops_decoded_loss_after_presence_is_revoked() {
    let (mut service, state, mut job, now) = in_flight_with_registry(JobKind::RefreshGuitar, None);
    job.kind = JobKind::Listen;
    let bridge = service.bridge_mut(&job.attachment).unwrap();
    bridge.running = None;
    bridge.guitar = Some((42, GuitarIdentity::Verified("3.10")));
    service.worker = Some(
        Worker::start(job.clone(), |_, cancel| {
            while !cancel.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
            Outcome::Notices(vec![Notice::Disconnected])
        })
        .unwrap(),
    );

    let profiles = super::super::registry::DEVICE_PROFILES;
    state.set(InventoryState::MissingIterator);
    assert!(service.refresh_at(Scan::Automatic, profiles, now).is_err());
    state.set(InventoryState::Ready);
    await_worker(&service);
    let devices = service.refresh_at(Scan::Automatic, profiles, now).unwrap();
    assert_eq!(devices.len(), 2);
    let bridge = service.bridge_mut(&job.attachment).unwrap();
    assert!(
        bridge.can_refresh(),
        "keep the passive path available for return"
    );
}

#[test]
fn linux_listener_disregards_results_from_a_stale_generation() {
    let (mut service, _, mut job, now) = in_flight_with_registry(JobKind::RefreshGuitar, None);
    let bridge = service.bridge_mut(&job.attachment).unwrap();
    bridge.running = None;
    bridge.guitar = Some((42, GuitarIdentity::Verified("3.10")));

    // The USB path and address can both be reused after a replug.
    job.attachment.generation -= 1;
    job.kind = JobKind::Listen;
    service.worker =
        Some(Worker::start(job, |_, _| Outcome::Notices(vec![Notice::Disconnected])).unwrap());
    await_worker(&service);
    let profiles = super::super::registry::DEVICE_PROFILES;
    let devices = service.refresh_at(Scan::Automatic, profiles, now).unwrap();
    assert_eq!(devices.len(), 2);
    assert!(devices.iter().any(|device| {
        device.key.linked_generation == Some(42)
            && device.support == DeviceSupport::Verified("3.10")
    }));
}

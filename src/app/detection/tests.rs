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

#[cfg(feature = "usb")]
#[test]
fn save_evidence_does_not_cross_disconnect_with_reused_usb_coordinates() {
    let (mut service, _, job, _) = in_flight_with_registry(JobKind::RefreshGuitar, None);
    let old = Arc::clone(&service.attachments["1-1"].podxt_saves);
    let listener = old.begin();
    let slot = line6_devices::family::podxt::Location::tone_from_label("32D").unwrap();
    listener.observe(slot);
    let revision = old.revision();

    let profiles = super::super::registry::DEVICE_PROFILES;
    let state = Rc::new(Cell::new(InventoryState::Ready));
    let absent = UsbFixture(SyntheticSysfs::new(), Rc::clone(&state), None);
    service.refresh_attachments(inventory::capture(&absent, profiles).unwrap());
    assert!(!listener.is_current());
    assert!(old.revision() > revision);
    let after_removal = old.revision();

    let mut usb = SyntheticSysfs::new();
    usb.add_device("1-1", 0x0e41, 0x4650, "PODxt Live", &[]);
    let present = UsbFixture(usb, state, None);
    service.refresh_attachments(inventory::capture(&present, profiles).unwrap());
    let replacement = &service.attachments["1-1"];
    assert_eq!(
        replacement.key.usb_location(),
        job.attachment.usb_location()
    );
    assert_ne!(replacement.key, job.attachment);
    listener.observe(slot);
    assert_eq!(old.revision(), after_removal);
    assert_eq!(replacement.podxt_saves.revision(), 0);
    assert!(!replacement.podxt_saves.saved_since(0));
}

#[cfg(feature = "usb")]
#[test]
fn linux_program_notices_follow_the_verified_attachment_and_do_not_change_guitar_presence() {
    let (mut service, _, job, now) = in_flight_with_registry(JobKind::RefreshGuitar, None);
    let profiles = super::super::registry::DEVICE_PROFILES;
    let bridge = service.bridge_mut(&job.attachment).unwrap();
    bridge.guitar = Some((42, GuitarIdentity::Verified("3.10")));
    bridge.last_notice = Some(Notice::Connected);

    let program = PodxtProgram {
        channel: 127,
        observed_at: now,
    };
    let mut stale = job.attachment.clone();
    stale.generation += 1;
    service.apply_linux_notices(&stale, vec![Notice::PodxtProgram(program)]);
    service.rebuild_devices(Scan::Automatic, profiles);
    assert!(
        service
            .known
            .values()
            .all(|device| device.podxt_program.is_none())
    );

    service.apply_linux_notices(
        &job.attachment,
        vec![
            Notice::PodxtProgram(program),
            Notice::PodxtProgram(PodxtProgram {
                channel: 1,
                observed_at: now - Duration::from_millis(1),
            }),
        ],
    );
    service.rebuild_devices(Scan::Automatic, profiles);
    let pod = service
        .known
        .values()
        .find(|item| item.key.family_id == "podxt")
        .unwrap();
    assert_eq!(pod.podxt_program, Some(program));
    let guitar = service
        .known
        .values()
        .find(|item| item.key.linked_generation == Some(42))
        .unwrap();
    assert_eq!(guitar.podxt_program, None);
    assert_eq!(
        service.bridge_mut(&job.attachment).unwrap().last_notice,
        Some(Notice::Connected)
    );
}

#[cfg(all(feature = "usb", target_os = "linux"))]
#[test]
fn linux_linked_follow_inputs_share_one_exclusive_rig_lease() {
    use line6_devices::{
        device_lock::{FollowDeviceLock, LockError, PassiveDeviceLock},
        family::variax::{FollowUpdate, Preset, follow},
        gate::{GatePolicy, Operation},
        session::{Link, Path, Quiet, Session},
    };
    use variax_workbench::identity_stage::guitar_reply;

    // Use the production lock API against a private runtime directory. A
    // subprocess keeps the environment separate from parallel test workers
    // and from any owner's local rig lock.
    const CHILD: &str = "VARIAX_TEST_LINKED_FOLLOW_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let runtime = tempfile::tempdir().unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let exact = format!("{module}::linux_linked_follow_inputs_share_one_exclusive_rig_lease");
        let executable = std::env::args_os().next().unwrap();
        let output = std::process::Command::new(executable)
            .args(["--exact", &exact])
            .env(CHILD, runtime.path())
            .env("XDG_RUNTIME_DIR", runtime.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert_eq!(std::env::var_os(CHILD), std::env::var_os("XDG_RUNTIME_DIR"));

    struct GuitarInput(std::collections::VecDeque<Vec<u8>>);
    impl Link for GuitarInput {
        fn path(&self) -> Path {
            Path::VariaxViaPodxt
        }
        fn write(&mut self, _: &[u8]) -> Result<(), String> {
            Ok(())
        }
        fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, String> {
            self.0
                .pop_front()
                .map(Some)
                .ok_or("Fake guitar input exhausted".into())
        }
        fn elapsed(&self) -> Duration {
            Duration::ZERO
        }
        fn pause(&mut self, _: Duration) {}
    }

    struct ReadOnly;
    impl GatePolicy for ReadOnly {
        fn allows(&self, op: Operation) -> bool {
            matches!(op, Operation::VariaxIdentity | Operation::VariaxRead)
        }
    }

    fn podxt_input(job: &Job, cancel: &AtomicBool) -> Outcome {
        assert_eq!(job.kind, JobKind::Listen);
        let Ok(lease) = PassiveDeviceLock::acquire() else {
            return Outcome::Notices(Vec::new());
        };
        Outcome::Notices(notice::read_notices(
            |_| {
                assert!(matches!(FollowDeviceLock::acquire(), Err(LockError::Busy)));
                Ok(Some(vec![0xc0, 127]))
            },
            || cancel.load(Ordering::Relaxed) || lease.yield_requested(),
            |_| unreachable!("no save in this fixture"),
            |_| unreachable!("valid PODxt program change"),
        ))
    }

    let (mut service, _, mut job, now) = in_flight_with_registry(JobKind::RefreshGuitar, None);
    let bridge = service.bridge_mut(&job.attachment).unwrap();
    bridge.running = None;
    bridge.guitar = Some((42, GuitarIdentity::Verified("3.10")));
    job.kind = JobKind::Listen;
    let profiles = super::super::registry::DEVICE_PROFILES;
    let mut listen_for_podxt = || {
        service.worker = Some(Worker::start(job.clone(), podxt_input).unwrap());
        await_worker(&service);
        let devices = service.refresh_at(Scan::Automatic, profiles, now).unwrap();
        let pod = devices
            .iter()
            .find(|item| item.key.family_id == "podxt")
            .unwrap();
        let guitar = devices
            .iter()
            .find(|item| item.key.family_id == "variax")
            .unwrap();
        assert_eq!(pod.support, DeviceSupport::Verified("3.01"));
        assert_eq!(guitar.support, DeviceSupport::Verified("3.10"));
        assert_eq!(guitar.key.linked_generation, Some(42));
        assert_eq!(guitar.key.attachment, pod.key.attachment);
        assert!(guitar.podxt_program.is_none());
        pod.podxt_program.map(|program| program.channel)
    };

    let guitar_lease = FollowDeviceLock::acquire().unwrap();
    let mut quiet = Quiet;
    let input = GuitarInput(
        [
            guitar_reply(),
            vec![0xc0, 77],
            vec![0xb0, 7, 63],
            vec![0xb0, 79, 96],
        ]
        .into(),
    );
    let mut session = Session::new(input, &ReadOnly, &mut quiet);
    let updates = std::cell::RefCell::new(Vec::new());
    follow(
        guitar_lease.rig(),
        &mut session,
        || updates.borrow().len() == 3,
        |update| {
            if matches!(update, FollowUpdate::ProgramChange(_)) {
                // A PODxt movement on interface 0 cannot reach its decoder
                // while the active guitar session owns the sole rig lease.
                assert_eq!(listen_for_podxt(), None);
                assert!(!guitar_lease.yield_requested());
            }
            updates.borrow_mut().push(update);
        },
    )
    .unwrap();
    assert_eq!(
        updates.into_inner(),
        [
            FollowUpdate::ProgramChange(Preset::new(4, 2).unwrap()),
            FollowUpdate::Volume(63),
            FollowUpdate::TonePosition(96),
        ]
    );
    drop(guitar_lease);

    // A fresh physical movement can reach the same verified PODxt when
    // passive input owns the rig, even if Follow tries to acquire it.
    assert_eq!(listen_for_podxt(), Some(127));
}

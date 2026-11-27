use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use variax_workbench::device_probe::{OsMetadataSource, SyntheticSysfs};

struct MacFixture(SyntheticSysfs);

impl PresenceSource for MacFixture {
    fn metadata(&self) -> &dyn OsMetadataSource {
        &self.0
    }

    fn usb_address(&self, _: &str) -> Option<u8> {
        Some(10)
    }

    fn usb_registry_id(&self, _: &str) -> Option<u64> {
        Some(30)
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
    let mut service = DetectionService::new(Box::new(MacFixture(usb)));
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

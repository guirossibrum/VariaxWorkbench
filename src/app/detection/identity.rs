//! One identity-only operation at a time. Results belong to the USB
//! attachment generation that started the operation.
use super::AttachmentKey;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor) enum JobKind {
    Initial,
    RefreshGuitar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::editor) struct Job {
    pub attachment: AttachmentKey,
    pub kind: JobKind,
}

#[derive(Debug)]
pub(in crate::editor) enum Guitar {
    #[cfg_attr(
        not(any(test, all(feature = "usb", target_os = "linux"))),
        allow(dead_code)
    )]
    Verified(&'static str),
    #[cfg_attr(
        not(any(test, all(feature = "usb", target_os = "linux"))),
        allow(dead_code)
    )]
    Silent,
    Unavailable(String),
    CleanupFailed,
}

#[derive(Debug)]
pub(in crate::editor) enum Outcome {
    #[cfg_attr(
        not(any(test, all(feature = "usb", target_os = "linux"))),
        allow(dead_code)
    )]
    Deferred,
    Finished {
        pod_firmware: Option<&'static str>,
        guitar: Guitar,
    },
}

pub(super) type Identify = fn(&Job, &AtomicBool) -> Outcome;

pub(super) struct Worker {
    pub job: Job,
    cancel: Arc<AtomicBool>,
    handle: JoinHandle<Outcome>,
}

impl Worker {
    pub fn start(job: Job, identify: Identify) -> std::io::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_job = job.clone();
        let handle = std::thread::Builder::new()
            .name("rig-identity".into())
            .spawn(move || identify(&worker_job, &worker_cancel))?;
        Ok(Self {
            job,
            cancel,
            handle,
        })
    }

    pub fn finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn finish(self) -> (Job, Outcome) {
        let outcome = self.handle.join().unwrap_or(Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::CleanupFailed,
        });
        (self.job, outcome)
    }
}

pub(super) fn platform() -> Option<Identify> {
    #[cfg(all(feature = "usb", target_os = "linux", not(test)))]
    {
        Some(live::identify)
    }
    #[cfg(any(not(feature = "usb"), not(target_os = "linux"), test))]
    {
        None
    }
}

#[cfg(all(feature = "usb", target_os = "linux", not(test)))]
mod live {
    use super::*;
    use line6_devices::device_lock::{DeviceLock, LockError};
    use line6_devices::session::Path;
    use line6_devices::transport::rawmidi::{PODXT_LIVE_CARD, RawMidiLink};
    use variax_workbench::device_probe::{
        LinuxSysfs, Recognition, expected_driver_binding, recognise,
    };
    use variax_workbench::hardware_gates::{Gate, is_locked, run_if_unlocked};
    use variax_workbench::identity_stage::{
        BRIDGE_VERSION, GUITAR_VERSION, IdentifyError, IdentityFailure,
    };
    use variax_workbench::usb;

    pub(super) fn identify(job: &Job, cancel: &AtomicBool) -> Outcome {
        if is_locked(Gate::OsDiscovery) {
            return unavailable("USB discovery is closed.");
        }
        let rig = match DeviceLock::acquire() {
            Ok(rig) => rig,
            Err(LockError::Busy) => return Outcome::Deferred,
            Err(error) => return unavailable(error.to_string()),
        };
        run_if_unlocked(Gate::OsDiscovery, || Ok(identify_on(&rig, job, cancel)))
            .unwrap_or_else(unavailable)
    }

    fn identify_on(rig: &DeviceLock, job: &Job, cancel: &AtomicBool) -> Outcome {
        if cancel.load(Ordering::Relaxed) {
            return unavailable("Identification cancelled.");
        }
        let metadata = LinuxSysfs::new();
        let Recognition::Recognised {
            device,
            sound_cards,
        } = recognise(&metadata)
        else {
            return Outcome::Deferred;
        };
        if device.sysfs_name != job.attachment.path {
            return Outcome::Deferred;
        }
        let pod_firmware = if job.kind == JobKind::Initial {
            let mut cards = sound_cards.iter().filter(|card| card.id == PODXT_LIVE_CARD);
            let (Some(card), None) = (cards.next(), cards.next()) else {
                // Cached USB presence can arrive before ALSA publishes the card.
                return Outcome::Deferred;
            };
            let result = run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                let mut link = RawMidiLink::open_device(
                    format!("/dev/snd/midiC{}D0", card.index).into(),
                    Path::PodxtMidi,
                )?;
                super::pod::identify(&mut link, cancel)
            });
            if let Err(error) = result {
                return unavailable(format!("PODxt Live identity: {error}"));
            }
            Some(BRIDGE_VERSION)
        } else {
            None
        };
        let guitar_ready = matches!(recognise(&metadata), Recognition::Recognised { device, .. }
            if device.sysfs_name == job.attachment.path && expected_driver_binding(&device));
        let guitar = if !guitar_ready {
            Guitar::Unavailable("The expected guitar interface driver is unavailable.".into())
        } else {
            match usb::read_discovery_identity(rig, &job.attachment.path, cancel) {
                Ok(_) => Guitar::Verified(GUITAR_VERSION),
                Err(IdentifyError::Locked) => {
                    Guitar::Unavailable("The guitar identity gate is closed.".into())
                }
                Err(IdentifyError::Failed(stop)) if stop.cleanup.is_some() => Guitar::CleanupFailed,
                Err(IdentifyError::Failed(stop)) if stop.failure == IdentityFailure::TimedOut => {
                    Guitar::Silent
                }
                Err(IdentifyError::Failed(stop)) => {
                    Guitar::Unavailable(stop.failure.sentence().into())
                }
            }
        };
        Outcome::Finished {
            pod_firmware,
            guitar,
        }
    }

    fn unavailable(reason: impl Into<String>) -> Outcome {
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Unavailable(reason.into()),
        }
    }
}

#[cfg(all(feature = "usb", any(test, target_os = "linux")))]
mod pod {
    use super::*;
    use line6_devices::family::podxt::PodxtRequest;
    use line6_devices::gate::{GatePolicy, Operation};
    use line6_devices::identity::Identity;
    use line6_devices::midi::Event;
    use line6_devices::session::{Link, POLL, Quiet, Session};
    use std::time::Duration;
    use variax_workbench::hardware_gates::{Gate, is_locked};
    use variax_workbench::identity_stage::BRIDGE_PRODUCT;

    struct IdentityOnly;
    impl GatePolicy for IdentityOnly {
        fn allows(&self, operation: Operation) -> bool {
            operation == Operation::PodxtIdentity
                && !is_locked(Gate::DeviceCheckCurrentSettingsRead)
        }
    }

    pub(super) fn identify(link: impl Link, cancel: &AtomicBool) -> Result<(), String> {
        identify_with_policy(link, cancel, &IdentityOnly)
    }

    fn identify_with_policy(
        mut link: impl Link,
        cancel: &AtomicBool,
        policy: &dyn GatePolicy,
    ) -> Result<(), String> {
        let mut log = Quiet;
        let mut session = Session::new(&mut link, policy, &mut log);
        session.listen(Duration::from_millis(200))?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Identification cancelled.".into());
        }
        session.send(PodxtRequest::Identity)?;
        let until = session.link.elapsed() + Duration::from_secs(3);
        let mut identity = None;
        while let Some(remaining) = until
            .checked_sub(session.link.elapsed())
            .filter(|time| !time.is_zero())
        {
            if cancel.load(Ordering::Relaxed) {
                return Err("Identification cancelled.".into());
            }
            if let Some(events) = session.poll(remaining.min(POLL))? {
                for event in events {
                    if let Event::Sysex(frame) = event
                        && let Some(reply) = Identity::parse(&frame)
                        && identity.replace(reply).is_some()
                    {
                        return Err("More than one PODxt identity answered.".into());
                    }
                }
            }
        }
        let reply = identity.ok_or("No PODxt identity answered.")?;
        let product = (u32::from(reply.family) << 16) | u32::from(reply.member);
        if reply.is_line6() && product == BRIDGE_PRODUCT && reply.version == [0, 3, 0, 1] {
            Ok(())
        } else {
            Err("The reply did not match PODxt Live 3.01.".into())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use line6_devices::identity::REQUEST;
        use line6_devices::session::Path;
        use variax_workbench::identity_stage::{bridge_reply, guitar_reply};

        #[derive(Default)]
        struct FakeMidi {
            now: Duration,
            writes: Vec<Vec<u8>>,
            replies: Vec<Vec<u8>>,
        }

        impl Link for FakeMidi {
            fn path(&self) -> Path {
                Path::PodxtMidi
            }
            fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
                self.writes.push(bytes.to_vec());
                Ok(())
            }
            fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
                self.now += wait;
                Ok(if self.writes.is_empty() {
                    None
                } else {
                    self.replies.pop()
                })
            }
            fn elapsed(&self) -> Duration {
                self.now
            }
            fn pause(&mut self, time: Duration) {
                self.now += time;
            }
        }

        #[test]
        fn only_sends_identity_and_validates_the_unique_supported_reply() {
            for (replies, verified) in [
                (vec![bridge_reply()], true),
                (vec![guitar_reply()], false),
                (
                    {
                        let mut wrong_firmware = bridge_reply();
                        wrong_firmware[15] = 2;
                        vec![wrong_firmware]
                    },
                    false,
                ),
                (vec![bridge_reply(), bridge_reply()], false),
                (vec![], false),
            ] {
                let mut link = FakeMidi {
                    replies,
                    ..Default::default()
                };
                assert_eq!(
                    identify(&mut link, &AtomicBool::new(false)).is_ok(),
                    verified
                );
                assert_eq!(link.writes, [REQUEST]);
            }
        }

        #[test]
        fn a_denied_identity_operation_sends_nothing() {
            struct Denied;
            impl GatePolicy for Denied {
                fn allows(&self, _operation: Operation) -> bool {
                    false
                }
            }
            let mut link = FakeMidi::default();
            assert!(identify_with_policy(&mut link, &AtomicBool::new(false), &Denied).is_err());
            assert!(link.writes.is_empty());
        }

        #[test]
        fn cancellation_before_the_request_sends_nothing() {
            let mut link = FakeMidi::default();
            assert!(identify(&mut link, &AtomicBool::new(true)).is_err());
            assert!(link.writes.is_empty());
        }
    }
}

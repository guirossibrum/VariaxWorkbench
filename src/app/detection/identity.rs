//! One identity-only operation at a time. Results belong to the USB
//! attachment generation that started the operation.
use super::{AttachmentKey, notice::Notice};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use variax_workbench::{firmware_notice::FirmwareNotice, identity_stage::UnverifiedGuitarReply};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor) enum JobKind {
    Initial,
    RefreshGuitar,
    ReattachGuitarDriver,
    Listen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::editor) struct Job {
    pub attachment: AttachmentKey,
    pub kind: JobKind,
}

#[derive(Debug)]
pub(in crate::editor) enum Guitar {
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    Verified(&'static str),
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    Silent,
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    Unverified(UnverifiedGuitarReply),
    #[cfg_attr(
        not(any(test, all(feature = "usb", target_os = "linux"))),
        allow(dead_code)
    )]
    DriverUnbound(String),
    Unavailable(String),
    CleanupFailed,
}

#[derive(Debug)]
pub(in crate::editor) enum Outcome {
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    Deferred,
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    OtherProcessBusy,
    #[cfg_attr(
        not(any(test, all(feature = "usb", target_os = "linux"))),
        allow(dead_code)
    )]
    Notices(Vec<Notice>),
    #[cfg_attr(
        not(any(
            test,
            all(feature = "usb", any(target_os = "linux", target_os = "macos"))
        )),
        allow(dead_code)
    )]
    UnverifiedPod(FirmwareNotice),
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
        let outcome = self.handle.join().unwrap_or_else(|_| {
            if self.job.kind == JobKind::Listen {
                eprintln!("PODxt Variax presence: listener stopped unexpectedly");
                Outcome::Notices(Vec::new())
            } else {
                Outcome::Finished {
                    pod_firmware: None,
                    guitar: Guitar::CleanupFailed,
                }
            }
        });
        (self.job, outcome)
    }
}

#[cfg(any(target_os = "linux", test))]
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

#[cfg(all(feature = "usb", target_os = "macos", not(test)))]
pub(super) fn platform() -> Option<Identify> {
    variax_workbench::macos_usb_available().then_some(macos::identify)
}

#[cfg(all(feature = "usb", target_os = "macos", not(test)))]
mod macos {
    use super::*;
    use line6_devices::{
        device_lock::{DeviceLock, LockError},
        family::variax,
        gate::{GatePolicy, Operation},
        session::{Quiet, Session},
        transport::usb::{UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE},
    };
    use variax_workbench::{
        hardware_gates::{Gate, run_if_unlocked},
        identity_stage::{BRIDGE_VERSION, GUITAR_VERSION},
    };

    struct GuitarIdentityOnly;
    impl GatePolicy for GuitarIdentityOnly {
        fn allows(&self, operation: Operation) -> bool {
            operation == Operation::VariaxIdentity
        }
    }

    pub(super) fn identify(job: &Job, cancel: &AtomicBool) -> Outcome {
        let Some((path, address)) = job.attachment.usb_location() else {
            return Outcome::Deferred;
        };
        let result = run_if_unlocked(Gate::OsDiscovery, || {
            run_if_unlocked(Gate::IdentityRequest, || {
                run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                    identify_on(job.kind, path, address, cancel)
                })
            })
        });
        result.unwrap_or_else(|error| Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Unavailable(error),
        })
    }

    fn identify_on(
        kind: JobKind,
        path: &str,
        address: u8,
        cancel: &AtomicBool,
    ) -> Result<Outcome, String> {
        let rig = match DeviceLock::acquire() {
            Ok(rig) => rig,
            Err(LockError::Busy) => return Ok(Outcome::OtherProcessBusy),
            Err(LockError::BusyHere) => return Ok(Outcome::Deferred),
            Err(error) => return Err(error.to_string()),
        };
        if cancel.load(Ordering::Relaxed) {
            return Ok(Outcome::Deferred);
        }
        let pod_firmware = if kind == JobKind::Initial {
            let mut link = UsbInterfaceLink::open_attachment_borrowed(
                line6_devices::transport::usb::macos::PODXT_MIDI,
                path,
                address,
                &rig,
            )?;
            let pod = super::pod::identify(&mut link, cancel);
            link.finish()?;
            if let Some(notice) = pod? {
                return Ok(Outcome::UnverifiedPod(notice));
            }
            Some(BRIDGE_VERSION)
        } else {
            None
        };
        if cancel.load(Ordering::Relaxed) {
            return Ok(Outcome::Deferred);
        }
        let result = (|| {
            let mut link = UsbInterfaceLink::open_attachment_borrowed(
                VARIAX_VIA_PODXT_LIVE,
                path,
                address,
                &rig,
            )?;
            let mut quiet = Quiet;
            let mut session = Session::new(&mut link, &GuitarIdentityOnly, &mut quiet);
            session.require_expected_sysex();
            let reply = variax::identify_if_present(&mut session);
            if let Err(error) = link.finish() {
                eprintln!("PODxt Live interface 1 cleanup: {error}");
                return Ok(Guitar::CleanupFailed);
            }
            match reply? {
                Some(reply) if variax::procedures::check_identity(&reply.identity).is_ok() => {
                    Ok(Guitar::Verified(GUITAR_VERSION))
                }
                Some(reply) => UnverifiedGuitarReply::from_identity(&reply.identity)
                    .map(Guitar::Unverified)
                    .ok_or_else(|| "Could not classify the guitar identity reply.".into()),
                None => Ok(Guitar::Silent),
            }
        })();
        Ok(Outcome::Finished {
            pod_firmware,
            guitar: result.unwrap_or_else(Guitar::Unavailable),
        })
    }
}

#[cfg(all(feature = "usb", target_os = "linux", not(test)))]
mod live {
    use super::super::notice::read_notices;
    use super::*;
    use line6_devices::device_lock::{DeviceLock, LockError, PassiveDeviceLock};
    use line6_devices::transport::rawmidi::RawMidiInput;
    use line6_devices::transport::usb::{
        VariaxDriverBinding, reattach_variax_driver, variax_driver_binding,
    };
    use line6_devices::{
        family::podxt::transport::PODXT_LIVE_MIDI, transport::rawmidi::RawMidiLink,
    };
    use variax_workbench::device_probe::{
        LinuxSysfs, Recognition, expected_driver_binding, recognise,
    };
    use variax_workbench::hardware_gates::{Gate, is_locked, run_if_unlocked};
    use variax_workbench::identity_stage::{
        BRIDGE_VERSION, DiscoveryIdentity, GUITAR_VERSION, IdentifyError, IdentityFailure,
    };
    use variax_workbench::usb;

    pub(super) fn identify(job: &Job, cancel: &AtomicBool) -> Outcome {
        if job.kind == JobKind::Listen {
            return Outcome::Notices(listen(job, cancel).unwrap_or_default());
        }
        if is_locked(Gate::OsDiscovery) {
            return unavailable("USB discovery is closed.");
        }
        let rig = match DeviceLock::acquire() {
            Ok(rig) => rig,
            Err(LockError::Busy) => return Outcome::OtherProcessBusy,
            Err(LockError::BusyHere) => return Outcome::Deferred,
            Err(error) => return unavailable(error.to_string()),
        };
        run_if_unlocked(Gate::OsDiscovery, || Ok(identify_on(&rig, job, cancel)))
            .unwrap_or_else(unavailable)
    }

    fn listen(job: &Job, cancel: &AtomicBool) -> Option<Vec<Notice>> {
        run_if_unlocked(Gate::OsDiscovery, || {
            run_if_unlocked(Gate::IdentityRequest, || {
                run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                    Ok(listen_on(job, cancel))
                })
            })
        })
        .ok()
        .flatten()
    }

    fn listen_on(job: &Job, cancel: &AtomicBool) -> Option<Vec<Notice>> {
        let Recognition::Recognised {
            device,
            sound_cards,
        } = recognise(&LinuxSysfs::new())
        else {
            return None;
        };
        if device.sysfs_name != job.attachment.path || cancel.load(Ordering::Relaxed) {
            return None;
        }
        let mut cards = sound_cards
            .iter()
            .filter(|card| card.id == PODXT_LIVE_MIDI.card_id);
        let (Some(card), None) = (cards.next(), cards.next()) else {
            return None;
        };
        let rig = PassiveDeviceLock::acquire().ok()?;
        let (path, address) = job.attachment.usb_location()?;
        if cancel.load(Ordering::Relaxed)
            || rig.yield_requested()
            || LinuxSysfs::new().usb_device_address(path) != Some(address)
        {
            return None;
        }
        // Opening a read-only descriptor sends no request and never detaches a driver.
        let mut input = RawMidiInput::open(PODXT_LIVE_MIDI.device_node(card.index)).ok()?;
        let notices = read_notices(
            |wait| input.read(wait),
            || cancel.load(Ordering::Relaxed) || rig.yield_requested(),
            |message| eprintln!("PODxt Variax presence: ignored {message}"),
        );
        (!cancel.load(Ordering::Relaxed)).then_some(notices)
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
        let Some((path, address)) = job.attachment.usb_location() else {
            return Outcome::Deferred;
        };
        if device.sysfs_name != path || metadata.usb_device_address(path) != Some(address) {
            return Outcome::Deferred;
        }
        let pod_firmware = if job.kind == JobKind::Initial {
            let mut cards = sound_cards
                .iter()
                .filter(|card| card.id == PODXT_LIVE_MIDI.card_id);
            let (Some(card), None) = (cards.next(), cards.next()) else {
                // Cached USB presence can arrive before ALSA publishes the card.
                return Outcome::Deferred;
            };
            let result = run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                let mut link = RawMidiLink::open_device(
                    PODXT_LIVE_MIDI.device_node(card.index),
                    PODXT_LIVE_MIDI.path,
                )?;
                super::pod::identify(&mut link, cancel)
            });
            match result {
                Ok(None) => Some(BRIDGE_VERSION),
                Ok(Some(notice)) => return Outcome::UnverifiedPod(notice),
                Err(error) => return unavailable(format!("PODxt Live identity: {error}")),
            }
        } else {
            None
        };
        if metadata.usb_device_address(path) != Some(address) {
            return Outcome::Deferred;
        }
        Outcome::Finished {
            pod_firmware,
            guitar: identify_guitar(rig, job.kind, (path, address), cancel),
        }
    }

    fn identify_guitar(
        rig: &DeviceLock,
        kind: JobKind,
        (path, address): (&str, u8),
        cancel: &AtomicBool,
    ) -> Guitar {
        if is_locked(Gate::IdentityRequest) {
            return Guitar::Unavailable("The guitar identity gate is closed.".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Guitar::Unavailable("Identification cancelled.".into());
        }
        let binding = || variax_driver_binding(path, address);
        match binding() {
            Ok(VariaxDriverBinding::Expected) => {}
            Ok(VariaxDriverBinding::Unbound) if kind != JobKind::ReattachGuitarDriver => {
                return Guitar::DriverUnbound(
                    "The expected guitar interface driver is unavailable. Use Refresh guitar to reattach it.".into(),
                );
            }
            Ok(VariaxDriverBinding::Unbound) => {
                if let Err(error) = run_if_unlocked(Gate::IdentityRequest, || {
                    reattach_variax_driver(rig, path, address)
                }) {
                    let error = format!("Could not restore the guitar interface driver: {error}");
                    return if matches!(binding(), Ok(VariaxDriverBinding::Unbound)) {
                        Guitar::DriverUnbound(error)
                    } else {
                        Guitar::Unavailable(error)
                    };
                }
            }
            Ok(VariaxDriverBinding::Other(driver)) => {
                return Guitar::Unavailable(format!(
                    "Unexpected guitar interface driver {driver}."
                ));
            }
            Err(error) => return Guitar::Unavailable(error),
        }
        let metadata = LinuxSysfs::new();
        let guitar_ready = matches!(recognise(&metadata), Recognition::Recognised { device, .. }
            if device.sysfs_name == path && expected_driver_binding(&device)
                && metadata.usb_device_address(path) == Some(address));
        if !guitar_ready {
            return Guitar::Unavailable(
                "The expected guitar interface driver is unavailable.".into(),
            );
        }
        match usb::read_discovery_identity(rig, (path, address), cancel) {
            Ok(DiscoveryIdentity::Verified) => Guitar::Verified(GUITAR_VERSION),
            Ok(DiscoveryIdentity::Unverified(reply)) => Guitar::Unverified(reply),
            Err(IdentifyError::Locked) => {
                Guitar::Unavailable("The guitar identity gate is closed.".into())
            }
            Err(IdentifyError::Failed(stop)) if stop.cleanup.is_some() => {
                if matches!(binding(), Ok(VariaxDriverBinding::Unbound)) {
                    Guitar::DriverUnbound("USB cleanup left the guitar interface driver unbound. Use Refresh guitar to reattach it, or replug PODxt Live USB as a last resort.".into())
                } else {
                    Guitar::CleanupFailed
                }
            }
            Err(IdentifyError::Failed(stop)) if stop.failure == IdentityFailure::TimedOut => {
                Guitar::Silent
            }
            Err(IdentifyError::Failed(stop)) => Guitar::Unavailable(stop.failure.sentence().into()),
        }
    }

    fn unavailable(reason: impl Into<String>) -> Outcome {
        Outcome::Finished {
            pod_firmware: None,
            guitar: Guitar::Unavailable(reason.into()),
        }
    }
}

#[cfg(all(feature = "usb", any(test, target_os = "linux", target_os = "macos")))]
mod pod {
    use super::*;
    use line6_devices::family::podxt::PodxtRequest;
    use line6_devices::gate::{GatePolicy, Operation};
    use line6_devices::identity::Identity;
    use line6_devices::midi::Event;
    use line6_devices::session::{Link, POLL, Quiet, Session};
    use std::time::Duration;
    use variax_workbench::hardware_gates::{Gate, is_locked};

    struct IdentityOnly;
    impl GatePolicy for IdentityOnly {
        fn allows(&self, operation: Operation) -> bool {
            operation == Operation::PodxtIdentity
                && !is_locked(Gate::DeviceCheckCurrentSettingsRead)
        }
    }

    pub(super) fn identify(
        link: impl Link,
        cancel: &AtomicBool,
    ) -> Result<Option<FirmwareNotice>, String> {
        identify_with_policy(link, cancel, &IdentityOnly)
    }

    fn identify_with_policy(
        mut link: impl Link,
        cancel: &AtomicBool,
        policy: &dyn GatePolicy,
    ) -> Result<Option<FirmwareNotice>, String> {
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
        Ok(FirmwareNotice::podxt(&reply))
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
                    matches!(identify(&mut link, &AtomicBool::new(false)), Ok(None)),
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

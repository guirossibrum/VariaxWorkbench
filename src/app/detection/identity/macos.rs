use super::*;
use line6_devices::{
    family::variax,
    gate::{GatePolicy, Operation},
    session::{Link, Quiet, Session},
};
use std::time::Duration;
use variax_workbench::identity_stage::{BRIDGE_VERSION, GUITAR_VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interface {
    Podxt,
    Variax,
}

// Automatic connection follows the silent first attempt in Rig Check 3.1.
const STARTUP_SILENCE_DELAY: Duration = Duration::from_secs(2);

struct GuitarIdentityOnly;
impl GatePolicy for GuitarIdentityOnly {
    fn allows(&self, operation: Operation) -> bool {
        operation == Operation::VariaxIdentity
    }
}

#[cfg(all(target_os = "macos", not(test)))]
pub(super) fn identify(job: &Job, cancel: &AtomicBool) -> Outcome {
    use line6_devices::{
        device_lock::{DeviceLock, LockError},
        transport::usb::{UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE, macos},
    };
    use variax_workbench::hardware_gates::{Gate, run_if_unlocked};

    let Some((path, address)) = job.attachment.usb_location() else {
        return Outcome::Deferred;
    };
    let Some(registry_id) = job.attachment.registry_id else {
        return Outcome::Deferred;
    };
    let result = run_if_unlocked(Gate::OsDiscovery, || {
        run_if_unlocked(Gate::IdentityRequest, || {
            run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                let rig = match DeviceLock::acquire() {
                    Ok(rig) => rig,
                    Err(LockError::Busy) => return Ok(Outcome::OtherProcessBusy),
                    Err(LockError::BusyHere) => return Ok(Outcome::Deferred),
                    Err(error) => return Err(error.to_string()),
                };
                let attachment = macos::Attachment {
                    path: path.into(),
                    address,
                };
                Ok(identify_on(
                    job.kind,
                    cancel,
                    |side| {
                        let spec = match side {
                            Interface::Podxt => macos::PODXT_MIDI,
                            Interface::Variax => VARIAX_VIA_PODXT_LIVE,
                        };
                        macos::with_ready_attachment(&rig, registry_id, &attachment, |matched| {
                            if cancel.load(Ordering::Relaxed) {
                                return Err("Identification cancelled.".into());
                            }
                            UsbInterfaceLink::open_attachment_borrowed(
                                spec,
                                &matched.path,
                                matched.address,
                                &rig,
                            )
                        })
                    },
                    UsbInterfaceLink::finish,
                ))
            })
        })
    });
    result.unwrap_or_else(|error| finished(None, Guitar::Unavailable(error)))
}

fn identify_on<L: Link>(
    kind: JobKind,
    cancel: &AtomicBool,
    mut open: impl FnMut(Interface) -> Result<L, String>,
    finish: impl Fn(&mut L) -> Result<(), String>,
) -> Outcome {
    let stopping = || cancel.load(Ordering::Relaxed);
    if stopping() {
        return Outcome::Deferred;
    }
    let pod_firmware = match kind {
        JobKind::Initial => {
            let mut link = match open(Interface::Podxt) {
                Ok(link) => link,
                Err(error) => return finished(None, Guitar::Unavailable(error)),
            };
            let pod = super::pod::identify_macos_startup(&mut link, cancel);
            if let Err(error) = finish(&mut link) {
                eprintln!("PODxt Live interface 0 cleanup: {error}");
                return finished(None, Guitar::CleanupFailed);
            }
            match pod {
                Ok(None) => Some(BRIDGE_VERSION),
                Ok(Some(notice)) => return Outcome::UnverifiedPod(notice),
                Err(error) => return finished(None, Guitar::Unavailable(error)),
            }
        }
        JobKind::RefreshGuitar => None,
        JobKind::ReattachGuitarDriver | JobKind::Listen => {
            return finished(
                None,
                Guitar::Unavailable("Unsupported macOS identity action.".into()),
            );
        }
    };

    if stopping() {
        return finished(
            pod_firmware,
            Guitar::Unavailable("Identification cancelled.".into()),
        );
    }
    let guitar = (|| {
        let mut link = open(Interface::Variax)
            .map_err(|error| format!("Variax USB interface 1 open failed: {error}"))?;
        let mut quiet = Quiet;
        let mut session = Session::new(&mut link, &GuitarIdentityOnly, &mut quiet);
        let silence_delay = (kind == JobKind::Initial).then_some(STARTUP_SILENCE_DELAY);
        let reply = variax::identify_if_present_cancellable(&mut session, silence_delay, &stopping);
        if let Err(error) = finish(&mut link) {
            eprintln!("PODxt Live interface 1 cleanup: {error}");
            return Ok(Guitar::CleanupFailed);
        }
        if stopping() {
            return Err("Identification cancelled.".into());
        }
        match reply.map_err(|error| format!("Variax identity exchange failed: {error}"))? {
            Some(reply) if variax::procedures::check_identity(&reply.identity).is_ok() => {
                Ok(Guitar::Verified(GUITAR_VERSION))
            }
            Some(reply) => UnverifiedGuitarReply::from_identity(&reply.identity)
                .map(Guitar::Unverified)
                .ok_or_else(|| "Could not classify the guitar identity reply.".into()),
            None => Ok(Guitar::Silent),
        }
    })();
    finished(pod_firmware, guitar.unwrap_or_else(Guitar::Unavailable))
}

fn finished(pod_firmware: Option<&'static str>, guitar: Guitar) -> Outcome {
    Outcome::Finished {
        pod_firmware,
        guitar,
    }
}

#[cfg(test)]
mod tests;

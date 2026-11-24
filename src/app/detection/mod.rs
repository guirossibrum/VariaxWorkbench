//! Reconcile cached USB presence and attachment-scoped identity results.
mod identity;

use super::ProductFamily;
use identity::{Guitar, Identify, Job, JobKind, Outcome, Worker};
use std::collections::BTreeMap;
use variax_workbench::device_probe::{MetadataError, OsMetadataSource, UsbDevice};

#[cfg(test)]
pub(super) use identity::{
    Guitar as SimulatedGuitar, Job as SimulatedJob, Outcome as SimulatedOutcome,
};

type UsbId = (u16, u16);

#[derive(Clone, Copy)]
pub(super) enum DeviceDiscovery {
    Automatic(UsbId),
    Manual(UsbId),
    ViaBridge(UsbId),
}

pub(super) struct DiscoveryProfile {
    pub family: &'static dyn ProductFamily,
    pub discovery: DeviceDiscovery,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct AttachmentKey {
    pub path: String,
    generation: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct DeviceKey {
    pub attachment: AttachmentKey,
    pub family_id: &'static str,
    linked_generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DeviceSupport {
    IdentityPending,
    UsbOnly,
    Verified(&'static str),
    Refreshing(&'static str),
    FileEditingOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DetectedDevice {
    pub key: DeviceKey,
    pub model: &'static str,
    pub support: DeviceSupport,
    pub guitar_status: Option<String>,
    pub refresh_guitar: bool,
}

pub(super) trait PresenceSource {
    fn metadata(&self) -> &dyn OsMetadataSource;

    #[cfg(test)]
    fn simulated_identity(&self, _job: &Job) -> Option<Outcome> {
        None
    }
}

#[cfg(all(target_os = "linux", not(test)))]
struct CachedUsbPresence(variax_workbench::device_probe::LinuxSysfs);

#[cfg(all(target_os = "linux", not(test)))]
impl PresenceSource for CachedUsbPresence {
    fn metadata(&self) -> &dyn OsMetadataSource {
        &self.0
    }
}

#[derive(Clone, Copy)]
pub(super) enum Scan {
    Automatic,
    Confirmed,
}

struct Attachment {
    key: AttachmentKey,
    usb: UsbDevice,
    bridge: Option<Bridge>,
}

struct Bridge {
    queued: Option<JobKind>,
    running: Option<JobKind>,
    deferred: bool,
    pod: Option<&'static str>,
    guitar: Option<(u64, &'static str)>,
    last_guitar: Option<&'static str>,
    status: String,
    suspended: bool,
}

impl Bridge {
    fn new(enabled: bool) -> Self {
        Self {
            queued: enabled.then_some(JobKind::Initial),
            running: None,
            deferred: false,
            pod: None,
            guitar: None,
            last_guitar: None,
            status: if enabled {
                "checking…"
            } else {
                "identity unavailable in this build"
            }
            .into(),
            suspended: false,
        }
    }

    fn pending(&self) -> bool {
        self.queued.is_some() || self.running.is_some()
    }

    fn can_refresh(&self) -> bool {
        self.pod.is_some() && !self.pending() && !self.suspended
    }

    fn complete(&mut self, kind: JobKind, outcome: Outcome, generation: &mut u64) {
        self.running = None;
        match outcome {
            Outcome::Deferred => {
                self.queued = Some(kind);
                self.deferred = true;
            }
            Outcome::Finished {
                pod_firmware,
                guitar,
            } => {
                if kind == JobKind::Initial {
                    self.pod = pod_firmware;
                }
                match guitar {
                    Guitar::Verified(firmware) => {
                        let key = self.guitar.map_or_else(
                            || {
                                *generation += 1;
                                *generation
                            },
                            |(key, _)| key,
                        );
                        self.guitar = Some((key, firmware));
                        self.last_guitar = Some(firmware);
                        self.status = format!("Variax 600 {firmware}");
                    }
                    other => {
                        self.guitar = None;
                        self.status = match other {
                            Guitar::Silent => "no guitar answered".into(),
                            Guitar::Unavailable(reason) => reason,
                            Guitar::CleanupFailed => {
                                self.suspended = true;
                                "USB cleanup failed. Guitar identity is suspended until USB reconnects.".into()
                            }
                            Guitar::Verified(_) => unreachable!(),
                        };
                    }
                }
            }
        }
    }

    fn label(&self) -> String {
        let status = if self.pending() {
            if self.deferred {
                "checking… (waiting for the rig)"
            } else {
                "checking…"
            }
        } else {
            &self.status
        };
        let mut label = format!("Guitar: {status}");
        if let Some(firmware) = self.last_guitar
            && (self.pending() || self.guitar.is_none())
        {
            label.push_str(&format!(" · last verified Variax 600 {firmware}"));
        }
        label
    }
}

pub(super) struct DetectionService {
    source: Box<dyn PresenceSource>,
    attachments: BTreeMap<String, Attachment>,
    known: BTreeMap<DeviceKey, DetectedDevice>,
    generation: u64,
    identify: Option<Identify>,
    worker: Option<Worker>,
}

impl DetectionService {
    pub fn platform() -> Option<Self> {
        #[cfg(all(target_os = "linux", not(test)))]
        {
            if !variax_workbench::device_probe::live_stage_available(
                variax_workbench::device_probe::Stage::UsbRecognition,
            ) {
                return None;
            }
            Some(Self::new(Box::new(CachedUsbPresence(
                variax_workbench::device_probe::LinuxSysfs::new(),
            ))))
        }
        #[cfg(any(not(target_os = "linux"), test))]
        {
            None
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub fn new(source: Box<dyn PresenceSource>) -> Self {
        Self {
            source,
            attachments: BTreeMap::new(),
            known: BTreeMap::new(),
            generation: 0,
            identify: identity::platform(),
            worker: None,
        }
    }

    fn usb_id(usb: &UsbDevice) -> UsbId {
        (usb.vendor_id, usb.product_id)
    }

    pub fn refresh(
        &mut self,
        scan: Scan,
        profiles: &[DiscoveryProfile],
    ) -> Result<Vec<DetectedDevice>, MetadataError> {
        self.refresh_attachments(profiles)?;
        self.finish_worker();
        if matches!(scan, Scan::Automatic) {
            self.start_queued();
        }
        self.rebuild_devices(scan, profiles);
        Ok(self.known.values().cloned().collect())
    }

    fn refresh_attachments(&mut self, profiles: &[DiscoveryProfile]) -> Result<(), MetadataError> {
        let mut listed = BTreeMap::new();
        for device in self.source.metadata().usb_devices()? {
            if listed.insert(device.sysfs_name.clone(), device).is_some() {
                return Err(MetadataError::Unreadable(
                    "the USB device list (duplicate attachment path)".into(),
                ));
            }
        }

        // Only a successful absence or changed USB product starts a new
        // generation. Detaching and restoring the driver leaves it in place.
        let mut previous = std::mem::take(&mut self.attachments);
        for (path, usb) in listed {
            let id = Self::usb_id(&usb);
            let attachment = match previous.remove(&path) {
                Some(existing) if Self::usb_id(&existing.usb) == id => {
                    Attachment { usb, ..existing }
                }
                _ => {
                    self.generation += 1;
                    let key = AttachmentKey {
                        path: path.clone(),
                        generation: self.generation,
                    };
                    let bridge = profiles.iter()
                        .any(|p| matches!(p.discovery, DeviceDiscovery::Automatic(automatic) if automatic == id))
                        .then(|| Bridge::new(self.identify.is_some() || cfg!(test)));
                    Attachment { key, usb, bridge }
                }
            };
            self.attachments.insert(path, attachment);
        }
        if let Some(worker) = &self.worker
            && !self
                .attachments
                .get(&worker.job.attachment.path)
                .is_some_and(|attachment| attachment.key == worker.job.attachment)
        {
            worker.cancel();
        }
        Ok(())
    }

    fn bridge_mut(&mut self, key: &AttachmentKey) -> Option<&mut Bridge> {
        self.attachments
            .get_mut(&key.path)
            .filter(|attachment| &attachment.key == key)?
            .bridge
            .as_mut()
    }

    pub fn queue_guitar_refresh(&mut self, key: &AttachmentKey) -> bool {
        let Some(bridge) = self.bridge_mut(key).filter(|bridge| bridge.can_refresh()) else {
            return false;
        };
        bridge.queued = Some(JobKind::RefreshGuitar);
        true
    }

    fn apply_outcome(&mut self, job: Job, outcome: Outcome) {
        let Some(attachment) = self
            .attachments
            .get_mut(&job.attachment.path)
            .filter(|attachment| attachment.key == job.attachment)
        else {
            return;
        };
        if let Some(bridge) = &mut attachment.bridge {
            bridge.complete(job.kind, outcome, &mut self.generation);
        }
    }

    fn finish_worker(&mut self) {
        if self.worker.as_ref().is_some_and(Worker::finished) {
            let (job, outcome) = self.worker.take().expect("finished worker").finish();
            self.apply_outcome(job, outcome);
        }
    }

    /// Delay quit until the endpoint and the shared lock have been released.
    pub fn cancel_for_quit(&mut self) -> bool {
        self.finish_worker();
        if let Some(worker) = &self.worker {
            worker.cancel();
            return true;
        }
        false
    }

    fn start_queued(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let Some(job) = self.attachments.values().find_map(|attachment| {
            let kind = attachment.bridge.as_ref()?.queued?;
            Some(Job {
                attachment: attachment.key.clone(),
                kind,
            })
        }) else {
            return;
        };
        let Some(identify) = self.identify else {
            #[cfg(test)]
            if let Some(outcome) = self.source.simulated_identity(&job) {
                self.bridge_mut(&job.attachment)
                    .expect("queued bridge")
                    .queued = None;
                self.apply_outcome(job, outcome);
            }
            return;
        };
        let bridge = self.bridge_mut(&job.attachment).expect("queued bridge");
        bridge.queued = None;
        bridge.running = Some(job.kind);
        match Worker::start(job.clone(), identify) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) => self.apply_outcome(
                job,
                Outcome::Finished {
                    pod_firmware: None,
                    guitar: Guitar::Unavailable(format!("Could not start identity: {error}.")),
                },
            ),
        }
    }

    fn rebuild_devices(&mut self, scan: Scan, profiles: &[DiscoveryProfile]) {
        let mut current = BTreeMap::new();
        for attachment in self.attachments.values() {
            let attached_id = Self::usb_id(&attachment.usb);
            let bridge = attachment.bridge.as_ref();
            for profile in profiles {
                let info = profile.family.info();
                let mut key = DeviceKey {
                    attachment: attachment.key.clone(),
                    family_id: info.id,
                    linked_generation: None,
                };
                let (support, status, refresh) = match profile.discovery {
                    DeviceDiscovery::Automatic(id) if id == attached_id => {
                        let bridge = bridge.expect("automatic attachment");
                        let support = match bridge.pod {
                            Some(firmware) => DeviceSupport::Verified(firmware),
                            None if bridge.pending() => DeviceSupport::IdentityPending,
                            None => DeviceSupport::UsbOnly,
                        };
                        (support, Some(bridge.label()), bridge.can_refresh())
                    }
                    DeviceDiscovery::Manual(id)
                        if (matches!(scan, Scan::Confirmed) || self.known.contains_key(&key))
                            && id == attached_id =>
                    {
                        (DeviceSupport::FileEditingOnly, None, false)
                    }
                    DeviceDiscovery::ViaBridge(id) if id == attached_id => {
                        let Some(bridge) = bridge else { continue };
                        let Some((generation, firmware)) = bridge.guitar else {
                            continue;
                        };
                        key.linked_generation = Some(generation);
                        let support = if bridge.pending() {
                            DeviceSupport::Refreshing(firmware)
                        } else {
                            DeviceSupport::Verified(firmware)
                        };
                        (support, Some(bridge.label()), bridge.can_refresh())
                    }
                    _ => continue,
                };
                current.insert(
                    key.clone(),
                    DetectedDevice {
                        key,
                        model: info.title,
                        support,
                        guitar_status: status,
                        refresh_guitar: refresh,
                    },
                );
            }
        }
        self.known = current;
    }
}

impl Drop for DetectionService {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.cancel();
            worker.finish();
        }
    }
}

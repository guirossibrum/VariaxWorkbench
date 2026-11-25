//! Reconcile cached USB presence and attachment-scoped identity results.
mod identity;
mod notice;
use notice::Notice;

use super::ProductFamily;
use identity::{Guitar, Identify, Job, JobKind, Outcome, Worker};
use std::collections::BTreeMap;
use variax_workbench::device_probe::{MetadataError, OsMetadataSource, UsbDevice};

#[cfg(test)]
pub(super) use identity::{
    Guitar as SimulatedGuitar, Job as SimulatedJob, JobKind as SimulatedJobKind,
    Outcome as SimulatedOutcome,
};
#[cfg(test)]
pub(super) use notice::Notice as SimulatedNotice;

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
    address: Option<u8>,
    generation: u64,
}

impl AttachmentKey {
    pub fn usb_location(&self) -> Option<(&str, u8)> {
        Some((&self.path, self.address?))
    }
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
    pub guitar_warning: Option<String>,
    pub refresh_guitar: bool,
}

pub(super) trait PresenceSource {
    fn metadata(&self) -> &dyn OsMetadataSource;
    #[cfg(not(test))]
    fn usb_address(&self, path: &str) -> Option<u8>;

    #[cfg(test)]
    fn usb_address(&self, _path: &str) -> Option<u8> {
        None
    }

    #[cfg(test)]
    fn simulated_identity(&self, _job: &Job) -> Option<Outcome> {
        None
    }

    #[cfg(test)]
    fn simulated_notices(&self, _attachment: &AttachmentKey) -> Vec<Notice> {
        Vec::new()
    }
}

#[cfg(all(target_os = "linux", not(test)))]
struct CachedUsbPresence(variax_workbench::device_probe::LinuxSysfs);

#[cfg(all(target_os = "linux", not(test)))]
impl PresenceSource for CachedUsbPresence {
    fn metadata(&self) -> &dyn OsMetadataSource {
        &self.0
    }

    fn usb_address(&self, path: &str) -> Option<u8> {
        self.0.usb_device_address(path)
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
    waiting_for_other_process: bool,
    pod: Option<&'static str>,
    guitar: Option<(u64, &'static str)>,
    last_guitar: Option<&'static str>,
    status: String,
    firmware_warning: Option<String>,
    last_notice: Option<Notice>,
    suspended: bool,
    driver_unbound: bool,
}

impl Bridge {
    fn new(enabled: bool) -> Self {
        Self {
            queued: enabled.then_some(JobKind::Initial),
            running: None,
            waiting_for_other_process: false,
            pod: None,
            guitar: None,
            last_guitar: None,
            status: if enabled {
                "checking…"
            } else {
                "identity unavailable in this build"
            }
            .into(),
            firmware_warning: None,
            last_notice: None,
            suspended: false,
            driver_unbound: false,
        }
    }

    fn pending(&self) -> bool {
        self.queued.is_some() || self.running.is_some()
    }

    fn can_refresh(&self) -> bool {
        self.pod.is_some() && !self.pending() && !self.suspended
    }

    fn complete(&mut self, kind: JobKind, outcome: Outcome, generation: &mut u64) {
        match outcome {
            Outcome::Notices(notices) => {
                for notice in notices {
                    self.observe_notice(notice);
                }
            }
            Outcome::Deferred | Outcome::OtherProcessBusy => {
                self.running = None;
                self.queued = Some(kind);
                self.waiting_for_other_process = matches!(outcome, Outcome::OtherProcessBusy);
            }
            Outcome::Finished {
                pod_firmware,
                guitar,
            } => {
                self.running = None;
                self.waiting_for_other_process = false;
                self.firmware_warning = None;
                if kind == JobKind::Initial {
                    self.pod = pod_firmware;
                }
                self.driver_unbound = matches!(guitar, Guitar::DriverUnbound(_));
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
                            Guitar::OlderFirmware(version) => {
                                self.firmware_warning = Some(format!(
                                    "Update Firmware · Variax 300/600 firmware {version} is below Workbench’s minimum of 3.07."
                                ));
                                format!("Variax 300/600 {version}")
                            }
                            Guitar::Unavailable(reason) | Guitar::DriverUnbound(reason) => reason,
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

    fn observe_notice(&mut self, notice: Notice) {
        if self.last_notice.replace(notice) != Some(notice)
            && self.can_refresh()
            && !self.driver_unbound
        {
            self.queued = Some(JobKind::RefreshGuitar);
        }
    }

    fn label(&self) -> String {
        let status = if self.pending() {
            if self.waiting_for_other_process {
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
        if let Err(error) = self.refresh_attachments(profiles) {
            self.cancel_listener();
            return Err(error);
        }
        let waiting_for_other_process = self.finish_worker();
        if matches!(scan, Scan::Automatic) {
            #[cfg(test)]
            for attachment in self.attachments.values_mut() {
                if let Some(bridge) = attachment
                    .bridge
                    .as_mut()
                    .filter(|bridge| bridge.pod.is_some())
                {
                    for notice in self.source.simulated_notices(&attachment.key) {
                        bridge.observe_notice(notice);
                    }
                }
            }
            if !waiting_for_other_process {
                self.start_queued();
            }
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

        // An absence, changed USB product or changed address starts a new
        // generation. Detaching and restoring the driver leaves it in place.
        let mut previous = std::mem::take(&mut self.attachments);
        for (path, usb) in listed {
            let id = Self::usb_id(&usb);
            let automatic = profiles.iter().any(
                |p| matches!(p.discovery, DeviceDiscovery::Automatic(automatic) if automatic == id),
            );
            let address = if automatic {
                self.source.usb_address(&path)
            } else {
                None
            };
            let attachment = match previous.remove(&path) {
                Some(existing)
                    if Self::usb_id(&existing.usb) == id && existing.key.address == address =>
                {
                    Attachment { usb, ..existing }
                }
                _ => {
                    self.generation += 1;
                    let key = AttachmentKey {
                        path: path.clone(),
                        address,
                        generation: self.generation,
                    };
                    let bridge =
                        automatic.then(|| Bridge::new(self.identify.is_some() || cfg!(test)));
                    Attachment { key, usb, bridge }
                }
            };
            self.attachments.insert(path, attachment);
        }
        if let Some(worker) = &self.worker {
            let same_attachment = self
                .attachments
                .get(&worker.job.attachment.path)
                .is_some_and(|attachment| attachment.key == worker.job.attachment);
            let ambiguous_bridge = worker.job.kind == JobKind::Listen
                && self
                    .attachments
                    .values()
                    .filter(|attachment| attachment.bridge.is_some())
                    .count()
                    != 1;
            if !same_attachment || ambiguous_bridge {
                worker.cancel();
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn attachment_key(&self, path: &str) -> AttachmentKey {
        self.attachments[path].key.clone()
    }

    fn bridge_mut(&mut self, key: &AttachmentKey) -> Option<&mut Bridge> {
        self.attachments
            .get_mut(&key.path)
            .filter(|attachment| &attachment.key == key)?
            .bridge
            .as_mut()
    }

    /// Invalidate the previously verified guitar for a matching Follow notice.
    /// Update the cached devices so the shell can reconcile without opening a link.
    pub fn report_guitar_disconnect(
        &mut self,
        key: &DeviceKey,
        profiles: &[DiscoveryProfile],
    ) -> Option<Vec<DetectedDevice>> {
        if !self.known.contains_key(key) {
            return None;
        }
        let generation = key.linked_generation?;
        let bridge = self.bridge_mut(&key.attachment)?;
        if bridge.guitar?.0 != generation {
            return None;
        }
        bridge.guitar = None;
        bridge.status = "disconnected".into();
        bridge.last_notice = Some(Notice::Disconnected);
        self.rebuild_devices(Scan::Automatic, profiles);
        Some(self.known.values().cloned().collect())
    }

    pub fn needs_guitar_driver_consent(&mut self, key: &AttachmentKey) -> bool {
        self.bridge_mut(key)
            .is_some_and(|bridge| bridge.can_refresh() && bridge.driver_unbound)
    }

    pub fn queue_guitar_refresh(&mut self, key: &AttachmentKey, reattach: bool) -> bool {
        let Some(bridge) = self
            .bridge_mut(key)
            .filter(|bridge| bridge.can_refresh() && bridge.driver_unbound == reattach)
        else {
            return false;
        };
        bridge.queued = Some(if reattach {
            JobKind::ReattachGuitarDriver
        } else {
            JobKind::RefreshGuitar
        });
        self.cancel_listener();
        true
    }

    fn cancel_listener(&self) {
        if let Some(worker) = &self.worker
            && worker.job.kind == JobKind::Listen
        {
            worker.cancel();
        }
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

    fn finish_worker(&mut self) -> bool {
        if !self.worker.as_ref().is_some_and(Worker::finished) {
            return false;
        }
        let (job, outcome) = self.worker.take().expect("finished worker").finish();
        let waiting_for_other_process = matches!(outcome, Outcome::OtherProcessBusy);
        self.apply_outcome(job, outcome);
        waiting_for_other_process
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
        let queued = self.attachments.values().find_map(|attachment| {
            let kind = attachment.bridge.as_ref()?.queued?;
            Some(Job {
                attachment: attachment.key.clone(),
                kind,
            })
        });
        let Some(identify) = self.identify else {
            #[cfg(test)]
            if let Some(job) = queued
                && let Some(outcome) = self.source.simulated_identity(&job)
            {
                self.bridge_mut(&job.attachment)
                    .expect("queued bridge")
                    .queued = None;
                self.apply_outcome(job, outcome);
            }
            #[cfg(not(test))]
            let _ = queued;
            return;
        };
        let job = queued.or_else(|| {
            let mut bridges = self
                .attachments
                .values()
                .filter(|attachment| attachment.bridge.is_some());
            let (Some(attachment), None) = (bridges.next(), bridges.next()) else {
                return None;
            };
            let bridge = attachment.bridge.as_ref()?;
            bridge.can_refresh().then(|| Job {
                attachment: attachment.key.clone(),
                kind: JobKind::Listen,
            })
        });
        let Some(job) = job else { return };
        if job.kind != JobKind::Listen {
            let bridge = self.bridge_mut(&job.attachment).expect("queued bridge");
            bridge.queued = None;
            bridge.waiting_for_other_process = false;
            bridge.running = Some(job.kind);
        }
        match Worker::start(job.clone(), identify) {
            Ok(worker) => self.worker = Some(worker),
            Err(error) if job.kind == JobKind::Listen => {
                eprintln!("PODxt Variax presence: could not start listener: {error}.");
            }
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
                let (support, status, warning, refresh) = match profile.discovery {
                    DeviceDiscovery::Automatic(id) if id == attached_id => {
                        let bridge = bridge.expect("automatic attachment");
                        let support = match bridge.pod {
                            Some(firmware) => DeviceSupport::Verified(firmware),
                            None if bridge.pending() => DeviceSupport::IdentityPending,
                            None => DeviceSupport::UsbOnly,
                        };
                        (
                            support,
                            Some(bridge.label()),
                            bridge.firmware_warning.clone(),
                            bridge.can_refresh(),
                        )
                    }
                    DeviceDiscovery::Manual(id)
                        if (matches!(scan, Scan::Confirmed) || self.known.contains_key(&key))
                            && id == attached_id =>
                    {
                        (DeviceSupport::FileEditingOnly, None, None, false)
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
                        (support, Some(bridge.label()), None, bridge.can_refresh())
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
                        guitar_warning: warning,
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

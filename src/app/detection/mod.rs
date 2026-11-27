//! Reconcile cached USB presence and attachment-scoped identity results.
mod identity;
mod notice;
use notice::Notice;

use super::ProductFamily;
use identity::{Guitar, Identify, Job, JobKind, Outcome, Worker};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub(super) const MAC_DISCOVERY_INTERVAL: Duration = Duration::from_millis(500);
#[cfg(feature = "usb")]
use std::sync::{Arc, atomic::AtomicBool};
use variax_workbench::device_probe::{MetadataError, UsbDevice};
use variax_workbench::firmware_notice::FirmwareNotice;

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
    registry_id: Option<u64>,
}

impl AttachmentKey {
    pub fn usb_location(&self) -> Option<(&str, u8)> {
        Some((&self.path, self.address?))
    }

    /// Registry generation of this observed macOS USB attachment.
    pub fn usb_registry_id(&self) -> Option<u64> {
        self.registry_id
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
    UnverifiedGuitarFirmware(FirmwareNotice),
    FileEditingOnly,
}

#[derive(Clone, Debug)]
pub(super) struct DetectedDevice {
    pub key: DeviceKey,
    #[cfg(feature = "usb")]
    pub podxt_reply_pending: Arc<AtomicBool>,
    pub model: &'static str,
    pub support: DeviceSupport,
    pub guitar_status: Option<String>,
    pub mac_identity_details: Option<String>,
    pub guitar_warning: Option<FirmwareNotice>,
    pub pod_warning: Option<FirmwareNotice>,
    pub refresh_guitar: bool,
}

pub(super) trait PresenceSource {
    #[cfg(any(test, target_os = "linux"))]
    fn metadata(&self) -> &dyn variax_workbench::device_probe::OsMetadataSource;

    #[cfg(any(test, target_os = "linux"))]
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        self.metadata().usb_devices()
    }

    #[cfg(not(any(test, target_os = "linux")))]
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError>;
    #[cfg(not(test))]
    fn usb_address(&self, path: &str) -> Option<u8>;

    #[cfg(test)]
    fn usb_address(&self, _path: &str) -> Option<u8> {
        None
    }

    fn usb_registry_id(&self, _path: &str) -> Option<u64> {
        None
    }

    fn usb_open_candidate(&self, _path: &str) -> bool {
        true
    }

    fn observed_usb_coordinates(&self, path: &str) -> (Option<String>, Option<u8>) {
        (Some(path.into()), self.usb_address(path))
    }

    fn usb_readiness_reason(&self, _path: &str) -> Option<String> {
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
    fn metadata(&self) -> &dyn variax_workbench::device_probe::OsMetadataSource {
        &self.0
    }

    fn usb_address(&self, path: &str) -> Option<u8> {
        self.0.usb_device_address(path)
    }
}

#[cfg(all(target_os = "macos", feature = "usb", not(test)))]
use line6_devices::transport::usb::{
    macos::{passive_readiness_message, single_podxt},
    macos_inventory::{ObservedPodxt, assess_candidate, inventory},
};

#[cfg(all(target_os = "macos", feature = "usb", not(test)))]
#[derive(Default)]
struct MacUsbPresence(std::cell::RefCell<BTreeMap<String, MacUsbObservation>>);

#[cfg(all(target_os = "macos", feature = "usb", not(test)))]
struct MacUsbObservation {
    record: ObservedPodxt,
    // Keep the reconciliation key in an incomplete scan. Readiness and the
    // exact opener still use freshly observed coordinates.
    retained_address: Option<u8>,
    readiness: Result<(), String>,
}

#[cfg(all(target_os = "macos", feature = "usb", not(test)))]
impl PresenceSource for MacUsbPresence {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        use variax_workbench::hardware_gates::{Gate, run_if_unlocked};
        let records =
            run_if_unlocked(Gate::OsDiscovery, inventory).map_err(MetadataError::Unreadable)?;
        let mut snapshot = self.0.borrow_mut();
        let previous = std::mem::take(&mut *snapshot);
        let by_registry_id: BTreeMap<_, _> = previous
            .iter()
            .map(|(path, observation)| (observation.record.registry_id, (path, observation)))
            .collect();
        Ok(records
            .iter()
            .enumerate()
            .map(|(index, record)| {
                let prior = by_registry_id.get(&record.registry_id);
                let route = record.route().filter(|route| {
                    records
                        .iter()
                        .filter(|other| other.route() == Some(route))
                        .count()
                        == 1
                        || prior.is_some_and(|(path, _)| path.as_str() == *route)
                });
                // Preserve the previous key through temporary ambiguity or
                // missing coordinates so it cannot rearm a terminal attempt.
                let retained = prior.map(|(path, _)| path.as_str()).filter(|path| {
                    record.route().is_none()
                        && records.iter().all(|other| other.route() != Some(path))
                });
                let path = route.or(retained).map_or_else(
                    || format!("macOS USB entry {}", record.registry_id),
                    str::to_owned,
                );
                snapshot.insert(
                    path.clone(),
                    MacUsbObservation {
                        record: record.clone(),
                        retained_address: record
                            .address
                            .or_else(|| prior.and_then(|(_, old)| old.retained_address)),
                        readiness: assess_candidate(&records, index)
                            .map_err(|reason| passive_readiness_message(Some(reason)).into())
                            .and_then(|_| single_podxt(&records).map(|_| ())),
                    },
                );
                UsbDevice {
                    sysfs_name: path,
                    vendor_id: 0x0e41,
                    product_id: 0x4650,
                    product: Some("PODxt Live".into()),
                    bcd_device: None,
                    interfaces: vec![],
                }
            })
            .collect())
    }

    fn usb_address(&self, path: &str) -> Option<u8> {
        self.0.borrow().get(path)?.retained_address
    }

    fn usb_registry_id(&self, path: &str) -> Option<u64> {
        Some(self.0.borrow().get(path)?.record.registry_id)
    }

    fn usb_open_candidate(&self, path: &str) -> bool {
        self.0
            .borrow()
            .get(path)
            .is_some_and(|observation| observation.readiness.is_ok())
    }

    fn observed_usb_coordinates(&self, path: &str) -> (Option<String>, Option<u8>) {
        self.0
            .borrow()
            .get(path)
            .map_or((None, None), |observation| {
                (
                    observation.record.route().map(str::to_owned),
                    observation.record.address,
                )
            })
    }

    fn usb_readiness_reason(&self, path: &str) -> Option<String> {
        Some(
            self.0
                .borrow()
                .get(path)?
                .readiness
                .as_ref()
                .map_or_else(String::clone, |()| passive_readiness_message(None).into()),
        )
    }
}

#[derive(Clone, Copy)]
pub(super) enum Scan {
    Automatic,
    Confirmed,
}

struct Attachment {
    key: AttachmentKey,
    // Keep unresolved PODxt replies with the physical generation, even
    // when no device tab is open.
    #[cfg(feature = "usb")]
    podxt_reply_pending: Arc<AtomicBool>,
    usb: UsbDevice,
    bridge: Option<Bridge>,
    open_candidate: bool,
    readiness_reason: Option<String>,
    identity_started: bool,
}

#[derive(Clone, Copy)]
enum GuitarIdentity {
    Verified(&'static str),
    Unverified(FirmwareNotice),
}

struct Bridge {
    queued: Option<JobKind>,
    running: Option<JobKind>,
    waiting_for_other_process: bool,
    pod: Option<&'static str>,
    guitar: Option<(u64, GuitarIdentity)>,
    last_guitar: Option<&'static str>,
    status: String,
    firmware_warning: Option<FirmwareNotice>,
    pod_warning: Option<FirmwareNotice>,
    last_notice: Option<Notice>,
    running_guitar_invalidated: bool,
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
            pod_warning: None,
            last_notice: None,
            running_guitar_invalidated: false,
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

    fn complete(&mut self, job: &Job, outcome: Outcome, generation: &mut u64) {
        let kind = job.kind;
        let mac = job.attachment.registry_id.is_some();
        // A loss revokes guitar proof from work already in flight. Still
        // accept a release failure and keep the worker until it has returned.
        if std::mem::take(&mut self.running_guitar_invalidated) {
            self.running = None;
            match &outcome {
                // The manual request has yet to run an identity exchange.
                Outcome::Deferred | Outcome::OtherProcessBusy if kind.is_manual_guitar_check() => {}
                Outcome::Finished {
                    guitar: Guitar::DriverUnbound(_) | Guitar::CleanupFailed,
                    ..
                } => self.queued = None,
                _ => return,
            }
        }
        match outcome {
            Outcome::Notices(_) => unreachable!("listener results have their own dispatch"),
            Outcome::Deferred | Outcome::OtherProcessBusy => {
                self.running = None;
                self.queued = Some(kind);
                self.waiting_for_other_process = matches!(outcome, Outcome::OtherProcessBusy);
            }
            Outcome::MacCancelled { pod_firmware } => {
                self.running = None;
                self.waiting_for_other_process = false;
                if kind == JobKind::Initial {
                    self.pod = pod_firmware;
                }
                self.status = "Identification cancelled.".into();
            }
            Outcome::MacPodSilent => {
                self.running = None;
                self.waiting_for_other_process = false;
                self.pod = None;
                self.status = "PODxt Live interface 0 identity request sent. No reply in three seconds. USB release completed.".into();
            }
            Outcome::UnverifiedPod(notice) => {
                self.running = None;
                self.waiting_for_other_process = false;
                self.pod = None;
                self.pod_warning = Some(notice);
                self.firmware_warning = None;
                self.status = "identity check unavailable on this connection".into();
                self.guitar = None;
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
                    self.pod_warning = None;
                }
                self.driver_unbound = matches!(guitar, Guitar::DriverUnbound(_));
                match guitar {
                    Guitar::Verified(firmware) => {
                        self.remember_guitar(GuitarIdentity::Verified(firmware), generation);
                        self.last_guitar = Some(firmware);
                        self.status = if mac {
                            format!(
                                "Variax 600 {firmware} verified via PODxt Live interface 1. USB release completed."
                            )
                        } else {
                            format!("Variax 600 {firmware}")
                        };
                    }
                    Guitar::Unverified(reply) => {
                        if reply.is_variax {
                            self.remember_guitar(
                                GuitarIdentity::Unverified(reply.notice),
                                generation,
                            );
                        } else {
                            self.guitar = None;
                        }
                        self.firmware_warning = Some(reply.notice);
                        self.status = if mac {
                            "Variax identity reply unverified on PODxt Live interface 1. USB release completed.".into()
                        } else {
                            "identity unverified".into()
                        };
                    }
                    other => {
                        self.guitar = None;
                        self.status = match other {
                            Guitar::Silent if mac => {
                                let attempt = if kind == JobKind::RefreshGuitar {
                                    "One Variax identity request went unanswered"
                                } else {
                                    "Two Variax identity requests went unanswered"
                                };
                                let recovery = if kind == JobKind::RefreshGuitar {
                                    "Check the VDI cable, then use Refresh guitar."
                                } else {
                                    "Check the VDI cable while Workbench continues checking."
                                };
                                format!(
                                    "{attempt} on PODxt Live interface 1. USB release completed. {recovery}"
                                )
                            }
                            Guitar::Silent => "no guitar answered".into(),
                            Guitar::Unavailable(reason) | Guitar::DriverUnbound(reason) => reason,
                            Guitar::CleanupFailed => {
                                self.suspended = true;
                                if mac {
                                    let interface = u8::from(
                                        kind != JobKind::Initial || pod_firmware.is_some(),
                                    );
                                    format!(
                                        "PODxt Live interface {interface} cleanup failed. Reconnect PODxt Live USB to resume identity checks."
                                    )
                                } else {
                                    "USB cleanup failed. Guitar identity is suspended until USB reconnects.".into()
                                }
                            }
                            Guitar::Verified(_) | Guitar::Unverified(_) => unreachable!(),
                        };
                    }
                }
            }
        }
    }

    fn remember_guitar(&mut self, identity: GuitarIdentity, generation: &mut u64) {
        // Explicit Refresh can establish presence without a return notice.
        if self.last_notice == Some(Notice::Disconnected) {
            self.last_notice = Some(Notice::Connected);
        }
        // A new assessment of the same attachment updates its existing tab.
        let linked = self.guitar.map_or_else(
            || {
                *generation += 1;
                *generation
            },
            |(linked, _)| linked,
        );
        self.guitar = Some((linked, identity));
    }

    fn observe_notice(&mut self, notice: Notice) {
        if self.last_notice.replace(notice) == Some(notice) {
            return;
        }
        match notice {
            Notice::Disconnected => {
                self.guitar = None;
                self.firmware_warning = None;
                self.queued = self.queued.filter(|kind| kind.is_manual_guitar_check());
                if self.queued.is_none() {
                    self.waiting_for_other_process = false;
                }
                self.running_guitar_invalidated |= self.running.is_some();
                if !self.suspended && !self.driver_unbound {
                    self.status = "disconnected".into();
                }
            }
            Notice::Connected if !self.suspended && !self.driver_unbound => {
                if self.queued.is_none()
                    && (self.running.is_none() || self.running_guitar_invalidated)
                {
                    self.queued = Some(JobKind::VerifyGuitarOnReturn);
                }
            }
            Notice::Connected => {}
        }
    }

    fn label(&self) -> String {
        self.label_with_status(None, false)
    }

    fn label_with_status(&self, readiness: Option<&str>, mac: bool) -> String {
        let holding_loss =
            self.running_guitar_invalidated && self.last_notice == Some(Notice::Disconnected);
        let status = if let Some(reason) = readiness {
            reason.to_owned()
        } else if self.pending() && !holding_loss {
            if self.waiting_for_other_process && mac {
                "PODxt Live is busy with another operation. Wait for it to finish.".into()
            } else if self.waiting_for_other_process {
                "checking… (waiting for the rig)".into()
            } else if mac {
                match self.running.or(self.queued) {
                    Some(JobKind::Initial) if self.status == "checking…" => {
                        "Checking PODxt Live identity on USB interface 0…".into()
                    }
                    Some(JobKind::RefreshGuitar) => {
                        "Checking Variax identity through PODxt Live USB interface 1…".into()
                    }
                    _ => format!("Checking again. Previous status: {}", self.status),
                }
            } else {
                "checking…".into()
            }
        } else {
            self.status.clone()
        };
        let mut label = if mac {
            status
        } else {
            format!("Guitar: {status}")
        };
        if let Some(firmware) = self.last_guitar
            && (readiness.is_some()
                || self.pending()
                || !matches!(self.guitar, Some((_, GuitarIdentity::Verified(_)))))
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
    next_mac_identity: Option<Instant>,
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
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        {
            variax_workbench::macos_usb_available()
                .then(|| Self::new(Box::new(MacUsbPresence::default())))
        }
        #[cfg(not(any(
            all(target_os = "linux", not(test)),
            all(target_os = "macos", feature = "usb", not(test))
        )))]
        {
            None
        }
    }

    #[cfg(any(target_os = "linux", test, all(target_os = "macos", feature = "usb")))]
    pub fn new(source: Box<dyn PresenceSource>) -> Self {
        Self {
            source,
            attachments: BTreeMap::new(),
            known: BTreeMap::new(),
            generation: 0,
            identify: identity::platform(),
            worker: None,
            next_mac_identity: None,
        }
    }

    fn usb_id(usb: &UsbDevice) -> UsbId {
        (usb.vendor_id, usb.product_id)
    }

    pub fn passive_pods(&self) -> impl Iterator<Item = &DetectedDevice> {
        self.known.values().filter(|device| {
            device.key.attachment.usb_registry_id().is_some()
                && device.key.linked_generation.is_none()
        })
    }

    pub fn firmware_notices(&self) -> impl Iterator<Item = FirmwareNotice> + '_ {
        self.attachments
            .values()
            .filter_map(|attachment| attachment.bridge.as_ref())
            .flat_map(|bridge| {
                [bridge.pod_warning, bridge.firmware_warning]
                    .into_iter()
                    .flatten()
            })
    }

    pub fn refresh(
        &mut self,
        scan: Scan,
        profiles: &[DiscoveryProfile],
    ) -> Result<Vec<DetectedDevice>, MetadataError> {
        self.refresh_at(scan, profiles, Instant::now())
    }

    pub(super) fn refresh_at(
        &mut self,
        scan: Scan,
        profiles: &[DiscoveryProfile],
        now: Instant,
    ) -> Result<Vec<DetectedDevice>, MetadataError> {
        if let Err(error) = self.refresh_attachments(profiles) {
            if let Some(worker) = &mut self.worker
                && worker.job.attachment.registry_id.is_some()
            {
                worker.revoke_observation();
                let key = worker.job.attachment.clone();
                if let Some(bridge) = self.bridge_mut(&key) {
                    bridge.running = None;
                    bridge.status = "PODxt Live USB identity unverified".into();
                }
            } else if let Some(worker) = &mut self.worker
                && worker.job.kind == JobKind::Listen
            {
                worker.revoke_observation();
            }
            return Err(error);
        }
        #[cfg(test)]
        if matches!(scan, Scan::Automatic) {
            let batches: Vec<_> = self
                .attachments
                .values()
                .map(|attachment| {
                    (
                        attachment.key.clone(),
                        self.source.simulated_notices(&attachment.key),
                    )
                })
                .collect();
            for (key, notices) in batches {
                self.apply_linux_notices(&key, notices);
            }
        }
        let waiting_for_other_process = self.finish_worker(now);
        if matches!(scan, Scan::Automatic) && !waiting_for_other_process {
            self.start_queued(now);
        }
        self.rebuild_devices(scan, profiles);
        Ok(self.known.values().cloned().collect())
    }

    fn refresh_attachments(&mut self, profiles: &[DiscoveryProfile]) -> Result<(), MetadataError> {
        let mut listed = BTreeMap::new();
        for device in self.source.usb_devices()? {
            if listed.insert(device.sysfs_name.clone(), device).is_some() {
                return Err(MetadataError::Unreadable(
                    "the USB device list (duplicate attachment path)".into(),
                ));
            }
        }

        // An absence or changed attachment identity starts a new generation.
        // Published interface availability is independent of that identity.
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
            let registry_id = self.source.usb_registry_id(&path);
            let open_candidate = self.source.usb_open_candidate(&path);
            let readiness_reason = self.source.usb_readiness_reason(&path).or_else(|| {
                (!open_candidate).then(|| "Guitar: PODxt Live USB interfaces unavailable".into())
            });
            let can_identify = self.identify.is_some() || cfg!(test);
            let attachment = match previous.remove(&path) {
                Some(mut existing)
                    if Self::usb_id(&existing.usb) == id
                        && existing.key.address == address
                        && existing.key.registry_id == registry_id =>
                {
                    if let Some(bridge) = &mut existing.bridge {
                        if !open_candidate {
                            // Withdraw the connection while readiness is uncertain.
                            // Ignore an in-flight result even if readiness returns
                            // before its worker has given the interface back.
                            if bridge.running.take().is_some() {
                                bridge.status = "PODxt Live USB identity unverified".into();
                            }
                        } else if !existing.open_candidate
                            && !existing.identity_started
                            && can_identify
                        {
                            bridge.queued = Some(JobKind::Initial);
                        }
                    }
                    Attachment {
                        usb,
                        open_candidate,
                        readiness_reason,
                        ..existing
                    }
                }
                _ => {
                    self.generation += 1;
                    let key = AttachmentKey {
                        path: path.clone(),
                        address,
                        generation: self.generation,
                        registry_id,
                    };
                    Attachment {
                        key,
                        #[cfg(feature = "usb")]
                        podxt_reply_pending: Arc::default(),
                        usb,
                        bridge: automatic.then(|| Bridge::new(open_candidate && can_identify)),
                        open_candidate,
                        readiness_reason,
                        identity_started: false,
                    }
                }
            };
            self.attachments.insert(path, attachment);
        }
        if let Some(worker) = &mut self.worker {
            let same_attachment = self
                .attachments
                .get(&worker.job.attachment.path)
                .is_some_and(|attachment| {
                    attachment.key == worker.job.attachment && attachment.open_candidate
                });
            let ambiguous_bridge = worker.job.kind == JobKind::Listen
                && self
                    .attachments
                    .values()
                    .filter(|attachment| attachment.bridge.is_some())
                    .count()
                    != 1;
            if ambiguous_bridge
                || !same_attachment
                    && (worker.job.attachment.registry_id.is_some()
                        || worker.job.kind == JobKind::Listen)
            {
                worker.revoke_observation();
            } else if !same_attachment {
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

    fn apply_linux_notices(&mut self, key: &AttachmentKey, notices: Vec<Notice>) {
        if notices.is_empty()
            || key.registry_id.is_some()
            || self
                .attachments
                .values()
                .filter(|attachment| attachment.bridge.is_some())
                .count()
                != 1
        {
            return;
        }
        if let Some(bridge) = self
            .attachments
            .get_mut(&key.path)
            .filter(|attachment| attachment.key == *key && attachment.open_candidate)
            .and_then(|attachment| attachment.bridge.as_mut())
            .filter(|bridge| bridge.pod.is_some())
        {
            for notice in notices {
                bridge.observe_notice(notice);
            }
        }
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
        if !matches!(bridge.guitar, Some((linked, GuitarIdentity::Verified(_))) if linked == generation)
        {
            return None;
        }
        if key.attachment.registry_id.is_none() {
            bridge.observe_notice(Notice::Disconnected);
        } else {
            bridge.guitar = None;
            bridge.status = "disconnected".into();
            if bridge.can_refresh() {
                bridge.queued = Some(JobKind::DiscoverGuitar);
            }
        }
        self.rebuild_devices(Scan::Automatic, profiles);
        Some(self.known.values().cloned().collect())
    }

    pub fn needs_guitar_driver_consent(&mut self, key: &AttachmentKey) -> bool {
        self.bridge_mut(key)
            .is_some_and(|bridge| bridge.can_refresh() && bridge.driver_unbound)
    }

    pub fn queue_guitar_refresh(&mut self, key: &AttachmentKey, reattach: bool) -> bool {
        if !self
            .attachments
            .get(&key.path)
            .is_some_and(|attachment| attachment.open_candidate)
        {
            return false;
        }
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

    fn apply_outcome(&mut self, job: Job, outcome: Outcome, now: Instant) {
        let Some(attachment) = self
            .attachments
            .get_mut(&job.attachment.path)
            .filter(|attachment| attachment.key == job.attachment && attachment.open_candidate)
        else {
            return;
        };
        if let Some(bridge) = &mut attachment.bridge {
            let retry = if job.attachment.registry_id.is_some() {
                match (&outcome, job.kind) {
                    (Outcome::MacPodSilent, JobKind::Initial) => Some(JobKind::Initial),
                    (
                        Outcome::Finished {
                            guitar: Guitar::Silent,
                            ..
                        },
                        JobKind::Initial | JobKind::DiscoverGuitar,
                    ) => Some(JobKind::DiscoverGuitar),
                    _ => None,
                }
            } else {
                None
            };
            bridge.complete(&job, outcome, &mut self.generation);
            if let Some(kind) = retry
                && (kind == JobKind::Initial || bridge.pod.is_some())
            {
                bridge.queued = Some(kind);
                self.next_mac_identity = Some(now + MAC_DISCOVERY_INTERVAL);
            }
        }
    }

    fn finish_worker(&mut self, now: Instant) -> bool {
        if !self.worker.as_ref().is_some_and(Worker::finished) {
            return false;
        }
        let worker = self.worker.take().expect("finished worker");
        let revoked = worker.observation_revoked();
        let quitting = worker.quitting();
        let (job, outcome) = worker.finish();
        if let Outcome::Notices(notices) = outcome {
            if !revoked && !quitting {
                self.apply_linux_notices(&job.attachment, notices);
            }
            return false;
        }
        if job.attachment.registry_id.is_some()
            && (quitting
                || !self
                    .bridge_mut(&job.attachment)
                    .is_some_and(|bridge| bridge.running == Some(job.kind)))
        {
            self.finish_invalidated(job, outcome, revoked, now);
            return false;
        }
        let waiting_for_other_process = matches!(outcome, Outcome::OtherProcessBusy);
        self.apply_outcome(job, outcome, now);
        waiting_for_other_process
    }

    fn finish_invalidated(&mut self, job: Job, outcome: Outcome, revoked: bool, now: Instant) {
        let Some(bridge) = self
            .attachments
            .get_mut(&job.attachment.path)
            .filter(|attachment| attachment.key == job.attachment)
            .and_then(|attachment| attachment.bridge.as_mut())
        else {
            return;
        };
        bridge.running = None;
        match outcome {
            Outcome::Deferred
            | Outcome::OtherProcessBusy
            | Outcome::MacCancelled { .. }
            | Outcome::MacPodSilent
            | Outcome::Finished {
                guitar: Guitar::Silent | Guitar::Verified(_),
                ..
            } if revoked
                && !bridge.suspended
                && (job.kind == JobKind::Initial
                    || (job.kind == JobKind::DiscoverGuitar && bridge.pod.is_some())) =>
            {
                // The worker gave back its link or stopped before opening it.
                // Restart from the last accepted stage without adopting revoked proof.
                bridge.queued = Some(job.kind);
                self.next_mac_identity = Some(now + MAC_DISCOVERY_INTERVAL);
            }
            Outcome::UnverifiedPod(_) => {
                bridge.queued = None;
                bridge.complete(&job, outcome, &mut self.generation);
            }
            Outcome::Finished {
                guitar:
                    guitar @ (Guitar::Unverified(_)
                    | Guitar::Unavailable(_)
                    | Guitar::DriverUnbound(_)
                    | Guitar::CleanupFailed),
                ..
            } => {
                bridge.queued = None;
                bridge.complete(
                    &job,
                    Outcome::Finished {
                        pod_firmware: bridge.pod,
                        guitar,
                    },
                    &mut self.generation,
                );
            }
            _ => {}
        }
    }

    /// Delay quit until the endpoint and the shared lock have been released.
    pub fn cancel_for_quit(&mut self) -> bool {
        if let Some(worker) = &mut self.worker {
            worker.cancel_for_quit();
        }
        self.finish_worker(Instant::now());
        self.worker.is_some()
    }

    fn note_identity_started(&mut self, job: &Job) {
        if job.kind == JobKind::Initial
            && let Some(attachment) = self.attachments.get_mut(&job.attachment.path)
        {
            attachment.identity_started = true;
        }
    }

    fn start_queued(&mut self, now: Instant) {
        if self.worker.is_some() {
            return;
        }
        let queued = self.attachments.values().find_map(|attachment| {
            if !attachment.open_candidate
                || (attachment.key.registry_id.is_some()
                    && self
                        .next_mac_identity
                        .is_some_and(|earliest| now < earliest))
            {
                return None;
            }
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
                self.note_identity_started(&job);
                if job.attachment.registry_id.is_some() {
                    self.next_mac_identity = Some(now + MAC_DISCOVERY_INTERVAL);
                }
                self.bridge_mut(&job.attachment)
                    .expect("queued bridge")
                    .queued = None;
                self.apply_outcome(job, outcome, now);
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
            (attachment.key.registry_id.is_none() && bridge.can_refresh()).then(|| Job {
                attachment: attachment.key.clone(),
                kind: JobKind::Listen,
            })
        });
        let Some(job) = job else { return };
        if job.attachment.registry_id.is_some() {
            self.next_mac_identity = Some(now + MAC_DISCOVERY_INTERVAL);
        }
        if job.kind != JobKind::Listen {
            self.note_identity_started(&job);
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
                now,
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
                let mac = attachment.key.registry_id.is_some();
                let (support, status, warning, pod_warning, refresh) = match profile.discovery {
                    DeviceDiscovery::Automatic(id) if id == attached_id => {
                        let bridge = bridge.expect("automatic attachment");
                        let support = match bridge.pod {
                            Some(firmware) => DeviceSupport::Verified(firmware),
                            None if bridge.pending() => DeviceSupport::IdentityPending,
                            None => DeviceSupport::UsbOnly,
                        };
                        if !attachment.open_candidate {
                            (
                                DeviceSupport::UsbOnly,
                                if mac {
                                    Some(bridge.label_with_status(
                                        attachment.readiness_reason.as_deref(),
                                        true,
                                    ))
                                } else {
                                    attachment.readiness_reason.clone()
                                },
                                None,
                                None,
                                false,
                            )
                        } else {
                            (
                                support,
                                Some(
                                    attachment
                                        .readiness_reason
                                        .as_ref()
                                        .filter(|_| !attachment.identity_started)
                                        .cloned()
                                        .unwrap_or_else(|| bridge.label_with_status(None, mac)),
                                ),
                                bridge.firmware_warning,
                                bridge.pod_warning,
                                bridge.can_refresh(),
                            )
                        }
                    }
                    DeviceDiscovery::Manual(id)
                        if (matches!(scan, Scan::Confirmed) || self.known.contains_key(&key))
                            && id == attached_id =>
                    {
                        (DeviceSupport::FileEditingOnly, None, None, None, false)
                    }
                    DeviceDiscovery::ViaBridge(id) if id == attached_id => {
                        let Some(bridge) = bridge.filter(|_| attachment.open_candidate) else {
                            continue;
                        };
                        let Some((generation, identity)) = bridge.guitar else {
                            continue;
                        };
                        key.linked_generation = Some(generation);
                        let (support, refresh) = match identity {
                            GuitarIdentity::Verified(firmware) if bridge.pending() => {
                                (DeviceSupport::Refreshing(firmware), false)
                            }
                            GuitarIdentity::Verified(firmware) => {
                                (DeviceSupport::Verified(firmware), bridge.can_refresh())
                            }
                            GuitarIdentity::Unverified(notice) => {
                                (DeviceSupport::UnverifiedGuitarFirmware(notice), false)
                            }
                        };
                        (support, Some(bridge.label()), None, None, refresh)
                    }
                    _ => continue,
                };
                current.insert(
                    key.clone(),
                    DetectedDevice {
                        key,
                        #[cfg(feature = "usb")]
                        podxt_reply_pending: Arc::clone(&attachment.podxt_reply_pending),
                        model: info.title,
                        support,
                        guitar_status: status,
                        mac_identity_details: mac.then(|| {
                            let (path, address) = self.source.observed_usb_coordinates(&attachment.key.path);
                            let path = path.as_deref().unwrap_or("unknown");
                            let address = address.map_or_else(|| "unknown".into(), |value| value.to_string());
                            let readiness = if attachment.open_candidate {
                                "macOS published USB interfaces 0 and 1."
                            } else {
                                attachment.readiness_reason.as_deref().unwrap_or("USB readiness unknown.")
                            };
                            let pod = bridge.and_then(|bridge| bridge.pod).map_or_else(
                                || "PODxt Live firmware unverified.".into(),
                                |version| if attachment.open_candidate {
                                    format!("PODxt Live firmware {version} verified.")
                                } else {
                                    format!("Last verified PODxt Live firmware {version}.")
                                },
                            );
                            format!("PODxt Live direct USB, path {path}, address {address}. {readiness} {pod} Variax identity uses the PODxt Live relay on interface 1. USB configuration unknown.")
                        }),
                        guitar_warning: warning,
                        pod_warning,
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

#[cfg(test)]
mod tests;

//! Device presence from cached metadata and injected logical observations.
use super::ProductFamily;
use std::collections::{BTreeMap, BTreeSet};
use variax_workbench::device_probe::{MetadataError, OsMetadataSource, UsbDevice};

type UsbId = (u16, u16);

#[derive(Clone, Copy)]
pub(super) enum DeviceDiscovery {
    Automatic(UsbId),
    Manual(UsbId),
    ViaBridge {
        bridge: UsbId,
        candidate: fn(&UsbDevice) -> bool,
    },
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
    /// An independently observed connection behind the USB attachment.
    linked_generation: Option<u64>,
}

impl DeviceKey {
    pub fn is_linked(&self) -> bool {
        self.linked_generation.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DeviceSupport {
    IdentityPending,
    FileEditingOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DetectedDevice {
    pub key: DeviceKey,
    pub model: &'static str,
    pub support: DeviceSupport,
}

#[derive(Clone)]
pub(super) struct LinkedObservation {
    pub parent: AttachmentKey,
    pub family_id: &'static str,
}

pub(super) trait PresenceSource {
    fn metadata(&self) -> &dyn OsMetadataSource;

    /// A complete view of logical attachments known to this source.
    fn linked_devices(&self) -> Vec<LinkedObservation> {
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
}

#[derive(Clone, Copy)]
pub(super) enum Scan {
    Automatic,
    Confirmed,
}

struct Attachment {
    key: AttachmentKey,
    usb: UsbDevice,
}

pub(super) struct DetectionService {
    source: Box<dyn PresenceSource>,
    attachments: BTreeMap<String, Attachment>,
    known: BTreeMap<DeviceKey, DetectedDevice>,
    generation: u64,
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
        let mut listed = BTreeMap::new();
        for device in self.source.metadata().usb_devices()? {
            if listed.insert(device.sysfs_name.clone(), device).is_some() {
                return Err(MetadataError::Unreadable(
                    "the USB device list (duplicate attachment path)".into(),
                ));
            }
        }

        // A successful absence or a different USB product starts a new
        // attachment on this path. Driver binding changes leave it in place.
        let mut previous = std::mem::take(&mut self.attachments);
        for (path, usb) in listed {
            let key = previous
                .remove(&path)
                .filter(|existing| Self::usb_id(&existing.usb) == Self::usb_id(&usb))
                .map(|existing| existing.key)
                .unwrap_or_else(|| {
                    self.generation += 1;
                    AttachmentKey {
                        path: path.clone(),
                        generation: self.generation,
                    }
                });
            self.attachments.insert(path, Attachment { key, usb });
        }

        let mut current = BTreeMap::new();
        let mut linked: BTreeSet<_> = self
            .source
            .linked_devices()
            .into_iter()
            .map(|observation| (observation.parent, observation.family_id))
            .collect();
        for attachment in self.attachments.values() {
            let attached_id = Self::usb_id(&attachment.usb);
            for profile in profiles {
                let info = profile.family.info();
                let key = DeviceKey {
                    attachment: attachment.key.clone(),
                    family_id: info.id,
                    linked_generation: None,
                };
                let support = match profile.discovery {
                    DeviceDiscovery::Automatic(id) if id == attached_id => {
                        DeviceSupport::IdentityPending
                    }
                    DeviceDiscovery::Manual(id)
                        if (matches!(scan, Scan::Confirmed) || self.known.contains_key(&key))
                            && id == attached_id =>
                    {
                        DeviceSupport::FileEditingOnly
                    }
                    DeviceDiscovery::ViaBridge { bridge, candidate }
                        if bridge == attached_id && candidate(&attachment.usb) =>
                    {
                        linked.insert((attachment.key.clone(), info.id));
                        continue;
                    }
                    _ => continue,
                };
                current.insert(
                    key.clone(),
                    DetectedDevice {
                        key,
                        model: info.title,
                        support,
                    },
                );
            }
        }

        for (parent, family_id) in linked {
            let Some(attachment) = self.attachments.get(&parent.path) else {
                continue;
            };
            if attachment.key != parent {
                continue;
            }
            let Some(profile) = profiles.iter().find(|profile| {
                profile.family.info().id == family_id
                    && matches!(profile.discovery, DeviceDiscovery::ViaBridge { bridge, .. } if bridge == Self::usb_id(&attachment.usb))
            }) else {
                continue;
            };
            let generation = self
                .known
                .keys()
                .find(|key| key.attachment == parent && key.family_id == family_id)
                .and_then(|key| key.linked_generation)
                .unwrap_or_else(|| {
                    self.generation += 1;
                    self.generation
                });
            let key = DeviceKey {
                attachment: parent,
                family_id,
                linked_generation: Some(generation),
            };
            current.insert(
                key.clone(),
                DetectedDevice {
                    key,
                    model: profile.family.info().title,
                    support: DeviceSupport::IdentityPending,
                },
            );
        }
        self.known = current;
        Ok(self.known.values().cloned().collect())
    }
}

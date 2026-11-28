//! Passive USB observations. Only the UI thread reconciles attachment state.
use super::{
    DetectedDevice, DetectionService, DeviceDiscovery, DiscoveryProfile, MetadataError,
    PresenceSource, Scan, UsbDevice,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

pub(super) struct Observation {
    pub usb: UsbDevice,
    pub automatic: bool,
    pub address: Option<u8>,
    pub registry_id: Option<u64>,
    pub open_candidate: bool,
    pub readiness_reason: Option<String>,
    pub observed_coordinates: (Option<String>, Option<u8>),
}

pub(super) type Snapshot = Result<BTreeMap<String, Observation>, MetadataError>;

pub(super) fn capture(source: &dyn PresenceSource, profiles: &[DiscoveryProfile]) -> Snapshot {
    let mut listed = BTreeMap::new();
    for usb in source.usb_devices()? {
        let path = usb.sysfs_name.clone();
        if listed.contains_key(&path) {
            return Err(MetadataError::Unreadable(
                "the USB device list (duplicate attachment path)".into(),
            ));
        }
        let id = DetectionService::usb_id(&usb);
        let automatic = profiles.iter().any(|profile| {
            matches!(profile.discovery, DeviceDiscovery::Automatic(candidate) if candidate == id)
        });
        let address = if automatic {
            source.usb_address(&path)
        } else {
            None
        };
        let registry_id = source.usb_registry_id(&path);
        let open_candidate = source.usb_open_candidate(&path);
        let readiness_reason = source.usb_readiness_reason(&path).or_else(|| {
            (!open_candidate).then(|| "Guitar: PODxt Live USB interfaces unavailable".into())
        });
        let observed_coordinates = if registry_id.is_some() {
            source.observed_usb_coordinates(&path)
        } else {
            (None, None)
        };
        listed.insert(
            path,
            Observation {
                usb,
                automatic,
                address,
                registry_id,
                open_candidate,
                readiness_reason,
                observed_coordinates,
            },
        );
    }
    Ok(listed)
}

pub(super) enum Source {
    // The native adapter can retain a previous observation between samples.
    // Only the worker ever locks it.
    #[cfg_attr(
        not(any(target_os = "linux", test, all(target_os = "macos", feature = "usb"))),
        allow(dead_code)
    )]
    Background(Arc<Mutex<Box<dyn PresenceSource + Send>>>),
    #[cfg(test)]
    InlineFixture(Box<dyn PresenceSource>),
}

impl Source {
    #[cfg(any(target_os = "linux", test, all(target_os = "macos", feature = "usb")))]
    pub(super) fn background(source: Box<dyn PresenceSource + Send>) -> Self {
        Self::Background(Arc::new(Mutex::new(source)))
    }

    #[cfg(test)]
    pub(super) fn inline_fixture(&self) -> Option<&dyn PresenceSource> {
        match self {
            Self::InlineFixture(source) => Some(&**source),
            Self::Background(_) => None,
        }
    }

    fn start(
        &self,
        profiles: &'static [DiscoveryProfile],
    ) -> Result<JoinHandle<Snapshot>, MetadataError> {
        let source = match self {
            Self::Background(source) => Arc::clone(source),
            #[cfg(test)]
            Self::InlineFixture(_) => unreachable!("inline fixtures refresh synchronously"),
        };
        std::thread::Builder::new()
            .name("usb-inventory".into())
            .spawn(move || {
                let source = source.lock().unwrap_or_else(PoisonError::into_inner);
                capture(&**source, profiles)
            })
            .map_err(|error| {
                MetadataError::Unreadable(format!("the USB inventory worker ({error})"))
            })
    }
}

#[derive(Clone, Copy)]
struct Request {
    serial: u64,
    scan: Scan,
}

struct Running {
    request: Request,
    worker: JoinHandle<Snapshot>,
}

#[derive(Default)]
pub(in crate::editor) struct InventoryRefresh {
    serial: u64,
    pending: Option<Request>,
    running: Option<Running>,
}

impl InventoryRefresh {
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.running.is_some()
    }

    pub fn request(&mut self, scan: Scan) {
        // Coalesce explicit requests without losing consent for a manual scan.
        let scan = match self.current_request() {
            Some(Request {
                scan: Scan::Confirmed,
                ..
            }) => Scan::Confirmed,
            _ => scan,
        };
        self.serial += 1;
        self.pending = Some(Request {
            serial: self.serial,
            scan,
        });
    }

    pub fn invalidate(&mut self) {
        // A device event received during a scan may be newer than its snapshot.
        // Take a fresh observation without widening a manual scan's consent.
        if let Some(request) = self.current_request() {
            self.request(request.scan);
        }
    }

    fn current_request(&self) -> Option<Request> {
        self.pending
            .or_else(|| self.running.as_ref().map(|running| running.request))
            .filter(|request| request.serial == self.serial)
    }

    pub fn discard(&mut self) {
        self.serial += 1;
        self.pending = None;
    }

    pub fn poll(
        &mut self,
        service: &mut DetectionService,
        profiles: &'static [DiscoveryProfile],
    ) -> Option<(Scan, Result<Vec<DetectedDevice>, MetadataError>)> {
        if let Some(running) = &self.running {
            if !running.worker.is_finished() {
                return None;
            }
            let Running { request, worker } = self.running.take().expect("finished inventory");
            let snapshot = worker.join().unwrap_or_else(|_| {
                Err(MetadataError::Unreadable(
                    "the USB inventory worker failed".into(),
                ))
            });
            if request.serial == self.serial {
                let result =
                    service.apply_inventory(request.scan, profiles, Instant::now(), snapshot);
                return Some((request.scan, result));
            }
        }
        let request = self.pending.take()?;
        #[cfg(test)]
        if service.source.inline_fixture().is_some() {
            return Some((request.scan, service.refresh(request.scan, profiles)));
        }
        match service.source.start(profiles) {
            Ok(worker) => self.running = Some(Running { request, worker }),
            Err(error) => {
                let result =
                    service.apply_inventory(request.scan, profiles, Instant::now(), Err(error));
                return Some((request.scan, result));
            }
        }
        None
    }
}

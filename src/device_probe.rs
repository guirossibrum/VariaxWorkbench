#![forbid(unsafe_code)]
//! Staged, consented device check for Rig 1 (PODxt Live with Variax 600 attached).
//!
//! Stage 1 (gate G1) reads only what the operating system already holds about
//! connected USB devices and sound cards. Stages 2–4 (gates G2 and G3) are the
//! later identity and settings reads. Every stage asks first, the default answer
//! is No, and a stage runs only after an explicit Yes for that stage. Decline,
//! cancel, a failure or an ambiguous result stops the whole sequence. A passed
//! stage never starts the next one by itself.
//!
//! Two backends exist. [`SimulatedRig`] (the default) answers every stage from
//! a synthetic sysfs fixture and the in-memory [`VirtualControl`]. [`HostRig`]
//! ("This computer") runs **step 1 only**, with the real reader
//! [`LinuxSysfs`], whose reads go through `run_if_unlocked(Gate::OsDiscovery,
//! …)`. G1 is open by the owner's direct permission (g1.md section 5); G2–G7
//! stay locked, so steps 2–4 are never offered in that mode.
//! See `docs/device-check.md` and `docs/hardware-gates/g1.md`.

use crate::hardware_gates::{self, Gate};
use crate::midi::Identity;
use crate::read_session::{self, ControlTransport, IDENTITY_REQUEST};
use crate::simulation::{ControlFault, VirtualControl};
use crate::{ParameterValue, PresetMetadata};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// USB vendor/product of the PODxt Live (decimal 3649/18000).
pub const PODXT_LIVE_USB: (u16, u16) = (0x0e41, 0x4650);
/// The setting stage 4 asks the person to change (Workbench's internal string 1).
pub const WATCHED_SETTING: &str = "string1_tuning";
/// Stages that have a real ("This computer") backend: step 1 only.
pub const LIVE_STAGES: [Stage; 1] = [Stage::UsbRecognition];
/// The answer used whenever the person has not chosen.
pub const DEFAULT_CONSENT: Consent = Consent::No;
pub const SIMULATION_LABEL: &str = "Simulation — pretend devices only. Nothing is sent to, or read from, a real PODxt Live or guitar.";
pub const THIS_COMPUTER_LABEL: &str = "This computer (G1) — step 1 reads what Linux already knows about connected USB devices and sound cards. Nothing is sent to the PODxt Live or the guitar.";
pub const G1_ONLY_BANNER: &str = "Only gate G1 (USB recognition) is open in this build, by the owner's direct permission. Gates G2–G7 stay locked: steps 2–4 never run on a real device.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    UsbRecognition,
    Identification,
    SettingsRead,
    ChangeCheck,
}

impl Stage {
    pub const LADDER: [Self; 4] = [
        Self::UsbRecognition,
        Self::Identification,
        Self::SettingsRead,
        Self::ChangeCheck,
    ];

    pub const fn number(self) -> u8 {
        match self {
            Self::UsbRecognition => 1,
            Self::Identification => 2,
            Self::SettingsRead => 3,
            Self::ChangeCheck => 4,
        }
    }

    const fn index(self) -> usize {
        self.number() as usize - 1
    }

    /// The compile-time hardware gate the live form of this stage would need.
    pub const fn gate(self) -> Gate {
        match self {
            Self::UsbRecognition => Gate::OsDiscovery,
            Self::Identification => Gate::IdentityRequest,
            Self::SettingsRead | Self::ChangeCheck => Gate::CurrentSettingsRead,
        }
    }

    pub const fn gate_label(self) -> &'static str {
        match self {
            Self::UsbRecognition => "G1",
            Self::Identification => "G2",
            Self::SettingsRead | Self::ChangeCheck => "G3",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::UsbRecognition => "USB recognition",
            Self::Identification => "Device identification",
            Self::SettingsRead => "Settings read",
            Self::ChangeCheck => "Change check",
        }
    }

    /// Exact plain-language description of what the stage does.
    pub const fn action(self) -> &'static str {
        match self {
            Self::UsbRecognition => {
                "Look at what Linux already knows about connected USB devices: vendor and product numbers, the description Linux saved when the device was plugged in, which Linux driver owns each part of it, and the list of sound cards. The app sends nothing to the PODxt Live or the guitar."
            }
            Self::Identification => {
                "Ask the PODxt Live and the attached guitar to say what they are, with one identity request. In the real version Linux's driver is briefly detached from the guitar-control part of the PODxt Live only, and reattached afterwards. Nothing is stored on the devices."
            }
            Self::SettingsRead => {
                "Read the guitar's current model settings: turn the guitar-control session on, send two settings requests, then turn the session off. The result names the guitar model and the first-string tuning."
            }
            Self::ChangeCheck => {
                "You change the first-string tuning on the guitar yourself, then the app reads the settings again the same way and reports whether that value changed. Nothing is written to the guitar."
            }
        }
    }

    /// Exact plain-language risk text shown before consent.
    pub const fn risk(self) -> &'static str {
        match self {
            Self::UsbRecognition => {
                "Low risk. The app only reads information Linux keeps. When the PODxt Live was plugged in, Linux's own drivers already talked to it (for example the Variax driver asks for a version); that happens without this app. If no device, several devices or unreadable information is found, the check stops instead of guessing."
            }
            Self::Identification => {
                "Medium risk, higher than USB recognition. This sends a message to the devices and briefly takes the guitar-control connection away from Linux. It does not change presets or settings, but a failed reattach could leave that connection unused until the PODxt Live is unplugged."
            }
            Self::SettingsRead => {
                "High risk, higher than identification. Several messages go to the guitar and change its session state while reading. The result is one preset's current settings, not a backup."
            }
            Self::ChangeCheck => {
                "Highest of the read steps. A second read happens while you handle the guitar. An unchanged value is still a finished read. Nothing is written to the guitar."
            }
        }
    }

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::UsbRecognition => Some(Self::Identification),
            Self::Identification => Some(Self::SettingsRead),
            Self::SettingsRead => Some(Self::ChangeCheck),
            Self::ChangeCheck => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Consent {
    #[default]
    No,
    Yes,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageStatus {
    NotRun,
    Passed,
    Stopped,
}

impl StageStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotRun => "not run",
            Self::Passed => "passed",
            Self::Stopped => "stopped",
        }
    }
}

// ---------------------------------------------------------------------------
// Stage 1: operating-system metadata
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbInterface {
    pub number: u8,
    /// Name of the Linux driver bound to this interface, if any.
    pub driver: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbDevice {
    /// The sysfs device name, such as `1-2` (a bus path, not a serial number).
    pub sysfs_name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: Option<String>,
    pub bcd_device: Option<u16>,
    pub interfaces: Vec<UsbInterface>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundCard {
    pub index: u32,
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataError {
    Unreadable(String),
    Malformed {
        entry: String,
        field: &'static str,
        value: String,
    },
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(what) => write!(f, "could not read {what}"),
            Self::Malformed { entry, field, .. } => {
                write!(f, "{entry} has an unreadable {field}")
            }
        }
    }
}

/// Read-only view of what the OS already holds. Implementations must not send
/// a USB request, open a device node or change a driver binding.
pub trait OsMetadataSource {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError>;
    fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError>;
}

/// In-memory sysfs/procfs map. Keys mirror paths relative to `/sys` (USB)
/// and `/proc` (`asound/cards`); a `driver` key holds the link target text.
/// Fixtures built here are synthetic test data. [`LinuxSysfs`] fills the same
/// map from a directory tree so both share one parser.
#[derive(Clone, Debug, Default)]
pub struct SyntheticSysfs {
    files: BTreeMap<String, String>,
}

const USB_ROOT: &str = "bus/usb/devices/";
const CARDS: &str = "asound/cards";

impl SyntheticSysfs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, path: &str, contents: &str) -> &mut Self {
        self.files.insert(path.to_owned(), contents.to_owned());
        self
    }

    pub fn remove(&mut self, path: &str) -> &mut Self {
        self.files.remove(path);
        self
    }

    /// Add a device with interface drivers, as Linux would list it.
    pub fn add_device(
        &mut self,
        name: &str,
        vendor: u16,
        product: u16,
        product_name: &str,
        interfaces: &[(u8, Option<&str>)],
    ) -> &mut Self {
        let base = format!("{USB_ROOT}{name}/");
        self.insert(&format!("{base}idVendor"), &format!("{vendor:04x}\n"));
        self.insert(&format!("{base}idProduct"), &format!("{product:04x}\n"));
        self.insert(&format!("{base}product"), &format!("{product_name}\n"));
        self.insert(&format!("{base}bcdDevice"), "0001\n");
        for (number, driver) in interfaces {
            let interface = format!("{USB_ROOT}{name}:1.{number}/");
            self.insert(
                &format!("{interface}bInterfaceNumber"),
                &format!("{number:02x}\n"),
            );
            if let Some(driver) = driver {
                self.insert(
                    &format!("{interface}driver"),
                    &format!("../../../../../../bus/usb/drivers/{driver}"),
                );
            }
        }
        self
    }

    pub fn set_cards(&mut self, text: &str) -> &mut Self {
        self.insert(CARDS, text)
    }

    fn entries(&self) -> BTreeMap<&str, BTreeMap<&str, &str>> {
        let mut entries: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
        for (path, contents) in &self.files {
            if let Some(rest) = path.strip_prefix(USB_ROOT)
                && let Some((entry, file)) = rest.split_once('/')
            {
                entries
                    .entry(entry)
                    .or_default()
                    .insert(file, contents.as_str());
            }
        }
        entries
    }
}

fn hex_field(
    entry: &str,
    files: &BTreeMap<&str, &str>,
    field: &'static str,
    digits: usize,
) -> Result<Option<u32>, MetadataError> {
    let Some(raw) = files.get(field) else {
        return Ok(None);
    };
    let text = raw.trim();
    if text.len() != digits || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(MetadataError::Malformed {
            entry: entry.to_owned(),
            field,
            value: text.chars().take(16).collect(),
        });
    }
    Ok(Some(u32::from_str_radix(text, 16).expect("validated hex")))
}

fn printable(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect()
}

impl OsMetadataSource for SyntheticSysfs {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        let entries = self.entries();
        let mut devices = Vec::new();
        for (name, files) in &entries {
            if name.contains(':') || !files.contains_key("idVendor") {
                continue;
            }
            let vendor = hex_field(name, files, "idVendor", 4)?;
            let product = hex_field(name, files, "idProduct", 4)?.ok_or_else(|| {
                MetadataError::Malformed {
                    entry: (*name).to_owned(),
                    field: "idProduct",
                    value: String::new(),
                }
            })?;
            let bcd = hex_field(name, files, "bcdDevice", 4)?;
            let mut interfaces = Vec::new();
            let prefix = format!("{name}:");
            for (interface, interface_files) in entries.range(prefix.as_str()..) {
                if !interface.starts_with(&prefix) {
                    break;
                }
                let number = hex_field(interface, interface_files, "bInterfaceNumber", 2)?
                    .ok_or_else(|| MetadataError::Malformed {
                        entry: (*interface).to_owned(),
                        field: "bInterfaceNumber",
                        value: String::new(),
                    })?;
                let driver = interface_files
                    .get("driver")
                    .and_then(|link| link.trim().rsplit('/').next())
                    .filter(|driver| !driver.is_empty())
                    .map(printable);
                interfaces.push(UsbInterface {
                    number: number as u8,
                    driver,
                });
            }
            interfaces.sort_by_key(|interface| interface.number);
            devices.push(UsbDevice {
                sysfs_name: printable(name),
                vendor_id: vendor.expect("checked above") as u16,
                product_id: product as u16,
                product: files.get("product").map(|text| printable(text)),
                bcd_device: bcd.map(|value| value as u16),
                interfaces,
            });
        }
        Ok(devices)
    }

    fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError> {
        let Some(text) = self.files.get(CARDS) else {
            return Err(MetadataError::Unreadable("the sound card list".into()));
        };
        parse_sound_cards(text)
    }
}

/// Parse the `/proc/asound/cards` text layout: ` N [id   ]: driver - name`
/// followed by an indented long-name line.
pub fn parse_sound_cards(text: &str) -> Result<Vec<SoundCard>, MetadataError> {
    let mut cards = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || line.starts_with("    ") {
            continue;
        }
        if trimmed.starts_with("---") {
            continue; // "--- no soundcards ---"
        }
        let malformed = || MetadataError::Malformed {
            entry: "sound card list".into(),
            field: "card line",
            value: trimmed.chars().take(32).collect(),
        };
        let (index, rest) = trimmed.split_once(' ').ok_or_else(malformed)?;
        let index: u32 = index.parse().map_err(|_| malformed())?;
        let rest = rest.trim_start().strip_prefix('[').ok_or_else(malformed)?;
        let (id, rest) = rest.split_once(']').ok_or_else(malformed)?;
        let rest = rest.trim_start().strip_prefix(':').ok_or_else(malformed)?;
        let name = rest.split_once(" - ").map_or(rest, |(_, name)| name).trim();
        cards.push(SoundCard {
            index,
            id: printable(id),
            name: printable(name),
        });
    }
    Ok(cards)
}

// ---------------------------------------------------------------------------
// Real stage-1 reader (#158). G1 is open in this build; G2–G7 stay locked.
// ---------------------------------------------------------------------------

/// Device attributes read from `/sys/bus/usb/devices/<dev>/` (g1.md section 1).
pub const DEVICE_ATTRIBUTES: [&str; 4] = ["idVendor", "idProduct", "bcdDevice", "product"];
/// Interface attribute read from `/sys/bus/usb/devices/<dev>:<cfg>.<n>/`.
pub const INTERFACE_ATTRIBUTE: &str = "bInterfaceNumber";
/// Interface link whose target name is the bound driver (read with `read_link`).
pub const DRIVER_LINK: &str = "driver";
const SYS_USB_DEVICES: &str = "sys/bus/usb/devices";
const PROC_CARDS: &str = "proc/asound/cards";
const MAX_USB_ENTRIES: usize = 512;

/// Reads the G1 paths from a directory tree: `<root>/sys/bus/usb/devices` and
/// `<root>/proc/asound/cards`. The root is `/` by default; tests use a
/// temporary fake tree. It uses only `read_dir` on the USB device directory,
/// `read_to_string` on the attributes above and `read_link` on `driver`: no
/// rusb, libusb, usbfs (`/dev/bus/usb`), ALSA library or device node, and no
/// writes. Every read goes through `run_if_unlocked(Gate::OsDiscovery, …)`.
/// G1 is open in this build, so the closure runs. If that constant were locked
/// the closure would not run and nothing would be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxSysfs {
    root: PathBuf,
}

/// The production root. Tests never read it; they inject a temporary tree.
pub const DEFAULT_ROOT: &str = "/";

impl Default for LinuxSysfs {
    fn default() -> Self {
        Self {
            root: PathBuf::from(DEFAULT_ROOT),
        }
    }
}

impl LinuxSysfs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn locked_error(error: String) -> MetadataError {
    MetadataError::Unreadable(error)
}

impl OsMetadataSource for LinuxSysfs {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        let mut inner = None;
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
            inner = Some(read_usb_tree(&self.root));
            Ok(())
        })
        .map_err(locked_error)?;
        inner.expect("ran when unlocked")?.usb_devices()
    }

    fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError> {
        let mut inner = None;
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
            inner = Some(read_cards_file(&self.root));
            Ok(())
        })
        .map_err(locked_error)?;
        parse_sound_cards(&inner.expect("ran when unlocked")?)
    }
}

fn io_error(what: &str, error: &std::io::Error) -> MetadataError {
    if error.kind() == ErrorKind::InvalidData {
        MetadataError::Malformed {
            entry: what.to_owned(),
            field: "text",
            value: "not UTF-8".into(),
        }
    } else {
        MetadataError::Unreadable(format!("{what} ({})", error.kind()))
    }
}

/// Read one attribute; a missing file is `None`.
fn read_attribute(path: &Path, what: &str) -> Result<Option<String>, MetadataError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(what, &error)),
    }
}

/// Not gated: callers are the gated trait methods above and this module's tests.
fn read_usb_tree(root: &Path) -> Result<SyntheticSysfs, MetadataError> {
    let directory = root.join(SYS_USB_DEVICES);
    let listing =
        std::fs::read_dir(&directory).map_err(|error| io_error("the USB device list", &error))?;
    let mut snapshot = SyntheticSysfs::new();
    let mut count = 0;
    for entry in listing {
        let entry = entry.map_err(|error| io_error("the USB device list", &error))?;
        count += 1;
        if count > MAX_USB_ENTRIES {
            return Err(MetadataError::Unreadable(
                "the USB device list (too many entries)".into(),
            ));
        }
        let os_name = entry.file_name();
        let Some(name) = os_name.to_str() else {
            return Err(MetadataError::Malformed {
                entry: "USB device list".into(),
                field: "entry name",
                value: "not UTF-8".into(),
            });
        };
        let base = directory.join(name);
        let key = |file: &str| format!("{USB_ROOT}{name}/{file}");
        if name.contains(':') {
            if let Some(text) = read_attribute(&base.join(INTERFACE_ATTRIBUTE), name)? {
                snapshot.insert(&key(INTERFACE_ATTRIBUTE), &text);
                match std::fs::read_link(base.join(DRIVER_LINK)) {
                    Ok(target) => {
                        snapshot.insert(&key(DRIVER_LINK), &target.to_string_lossy());
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => return Err(io_error(name, &error)),
                }
            }
        } else {
            for attribute in DEVICE_ATTRIBUTES {
                if let Some(text) = read_attribute(&base.join(attribute), name)? {
                    snapshot.insert(&key(attribute), &text);
                }
            }
        }
    }
    Ok(snapshot)
}

/// Not gated: callers are the gated trait methods above and this module's tests.
fn read_cards_file(root: &Path) -> Result<String, MetadataError> {
    read_attribute(&root.join(PROC_CARDS), "the sound card list")?
        .ok_or_else(|| MetadataError::Unreadable("the sound card list".into()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recognition {
    Recognised {
        device: UsbDevice,
        sound_cards: Vec<SoundCard>,
    },
    Absent,
    Several(usize),
    Malformed(String),
    Unreadable(String),
}

impl Recognition {
    pub const fn passed(&self) -> bool {
        matches!(self, Self::Recognised { .. })
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Recognised {
                device,
                sound_cards,
            } => {
                let interfaces = device
                    .interfaces
                    .iter()
                    .map(|interface| {
                        format!(
                            "interface {} driver {}",
                            interface.number,
                            interface.driver.as_deref().unwrap_or("none")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let cards = sound_cards
                    .iter()
                    .map(|card| format!("{} \"{}\"", card.index, card.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "PODxt Live {:04x}:{:04x} found at USB {}; {interfaces}; sound cards: {}",
                    device.vendor_id,
                    device.product_id,
                    device.sysfs_name,
                    if cards.is_empty() {
                        "none".into()
                    } else {
                        cards
                    },
                )
            }
            Self::Absent => {
                "No PODxt Live is listed by Linux. Check the USB cable and power.".into()
            }
            Self::Several(count) => format!(
                "{count} PODxt Live devices are listed. Connect only one so the check cannot pick the wrong device."
            ),
            Self::Malformed(reason) => {
                format!("Linux's device information could not be read safely: {reason}.")
            }
            Self::Unreadable(reason) => {
                format!("Linux's device information is unavailable: {reason}.")
            }
        }
    }
}

/// Stage 1: decide from OS metadata only. Unreadable or malformed metadata,
/// zero matches and several matches all stop.
pub fn recognise(source: &dyn OsMetadataSource) -> Recognition {
    let devices = match source.usb_devices() {
        Ok(devices) => devices,
        Err(error @ MetadataError::Malformed { .. }) => {
            return Recognition::Malformed(error.to_string());
        }
        Err(error) => return Recognition::Unreadable(error.to_string()),
    };
    let mut matches: Vec<_> = devices
        .into_iter()
        .filter(|device| (device.vendor_id, device.product_id) == PODXT_LIVE_USB)
        .collect();
    match matches.len() {
        0 => return Recognition::Absent,
        1 => {}
        count => return Recognition::Several(count),
    }
    let sound_cards = match source.sound_cards() {
        Ok(cards) => cards,
        Err(error @ MetadataError::Malformed { .. }) => {
            return Recognition::Malformed(error.to_string());
        }
        Err(error) => return Recognition::Unreadable(error.to_string()),
    };
    Recognition::Recognised {
        device: matches.pop().expect("one match"),
        sound_cards,
    }
}

// ---------------------------------------------------------------------------
// Backends
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadPass {
    First,
    AfterChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeMode {
    Simulation,
    ThisComputer,
}

impl ProbeMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Simulation => "Simulation",
            Self::ThisComputer => "This computer (G1)",
        }
    }
}

/// Everything a stage may use: [`SimulatedRig`], [`HostRig`] or [`RigBackend`].
pub trait ProbeBackend {
    fn mode(&self) -> ProbeMode;
    /// Whether this backend may run the stage at all. A stage it cannot run
    /// is never offered for consent.
    fn can_run(&self, stage: Stage) -> bool;
    fn os_metadata(&mut self) -> &dyn OsMetadataSource;
    fn identify(&mut self) -> Result<Identity, String>;
    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario {
    #[default]
    Connected,
    NothingConnected,
    TwoPods,
    UnreadableInfo,
    GuitarSilent,
    NoChange,
}

impl Scenario {
    pub const ALL: [Self; 6] = [
        Self::Connected,
        Self::NothingConnected,
        Self::TwoPods,
        Self::UnreadableInfo,
        Self::GuitarSilent,
        Self::NoChange,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "PODxt Live with guitar, tuning changed",
            Self::NothingConnected => "Nothing connected",
            Self::TwoPods => "Two PODxt Live units connected",
            Self::UnreadableInfo => "Unreadable device information",
            Self::GuitarSilent => "Guitar does not answer the settings read",
            Self::NoChange => "Tuning left unchanged",
        }
    }
}

/// Counts of backend calls, so tests can prove nothing ran without consent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackendCalls {
    pub os_reads: usize,
    pub identity_requests: usize,
    pub settings_reads: usize,
}

/// Pretend PODxt Live + Variax 600: synthetic sysfs and in-memory control.
pub struct SimulatedRig {
    scenario: Scenario,
    sysfs: SyntheticSysfs,
    calls: BackendCalls,
}

/// Synthetic stage-1 fixture for a PODxt Live on bus path `1-2`.
pub fn synthetic_podxt_live() -> SyntheticSysfs {
    let mut sysfs = SyntheticSysfs::new();
    sysfs
        .add_device("usb1", 0x1d6b, 0x0002, "Synthetic root hub", &[(0, Some("hub"))])
        .add_device(
            "1-2",
            PODXT_LIVE_USB.0,
            PODXT_LIVE_USB.1,
            "PODxt Live",
            &[(0, Some("snd-usb-pod")), (1, Some("snd-usb-variax"))],
        )
        .set_cards(
            " 0 [Synthetic      ]: Synthetic - Synthetic onboard audio\n                      Synthetic onboard audio\n 1 [PODxtLive      ]: Line6-POD - PODxt Live\n                      Line 6 PODxt Live at USB 1-2:1.0\n",
        );
    sysfs
}

const SYNTHETIC_NAME: &[u8] = b"SIMULATED";
const BODY_MODEL_OFFSET: usize = 18;
const STRING1_TUNING_OFFSET: usize = 69;

/// Synthetic 192-byte current-preset metadata; never a copy of a unit's data.
pub fn synthetic_metadata(string1_tuning: i8) -> [u8; 192] {
    let mut raw = [0u8; 192];
    raw[..SYNTHETIC_NAME.len()].copy_from_slice(SYNTHETIC_NAME);
    for (index, byte) in raw[22..].iter_mut().enumerate() {
        *byte = (index * 13 + 7) as u8;
    }
    raw[BODY_MODEL_OFFSET..BODY_MODEL_OFFSET + 4].copy_from_slice(&3i32.to_be_bytes());
    raw[STRING1_TUNING_OFFSET] = string1_tuning as u8;
    raw
}

impl SimulatedRig {
    pub fn new(scenario: Scenario) -> Self {
        let mut sysfs = synthetic_podxt_live();
        match scenario {
            Scenario::NothingConnected => {
                let keys: Vec<_> = sysfs
                    .files
                    .keys()
                    .filter(|key| key.starts_with("bus/usb/devices/1-2"))
                    .cloned()
                    .collect();
                for key in keys {
                    sysfs.remove(&key);
                }
            }
            Scenario::TwoPods => {
                sysfs.add_device(
                    "1-3",
                    PODXT_LIVE_USB.0,
                    PODXT_LIVE_USB.1,
                    "PODxt Live",
                    &[(0, Some("snd-usb-pod")), (1, Some("snd-usb-variax"))],
                );
            }
            Scenario::UnreadableInfo => {
                sysfs.insert("bus/usb/devices/1-2/idProduct", "46?0\n");
            }
            _ => {}
        }
        Self {
            scenario,
            sysfs,
            calls: BackendCalls::default(),
        }
    }

    pub const fn scenario(&self) -> Scenario {
        self.scenario
    }

    pub const fn calls(&self) -> BackendCalls {
        self.calls
    }
}

impl ProbeBackend for SimulatedRig {
    fn mode(&self) -> ProbeMode {
        ProbeMode::Simulation
    }

    fn can_run(&self, _: Stage) -> bool {
        true
    }

    fn os_metadata(&mut self) -> &dyn OsMetadataSource {
        self.calls.os_reads += 1;
        &self.sysfs
    }

    fn identify(&mut self) -> Result<Identity, String> {
        self.calls.identity_requests += 1;
        let mut control = VirtualControl::new(synthetic_metadata(0))?;
        control.send(&IDENTITY_REQUEST)?;
        let cancel = AtomicBool::new(false);
        let reply = control
            .next_message(&cancel, Duration::from_secs(1))?
            .ok_or("the simulated bridge did not answer")?;
        let identity = Identity::parse(&reply)?;
        // Leave the simulated session as the live cleanup would.
        control.send(&read_session::MIDI_DISABLE)?;
        Ok(identity)
    }

    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String> {
        self.calls.settings_reads += 1;
        let tuning = match (pass, self.scenario) {
            (ReadPass::AfterChange, Scenario::Connected) => -2,
            _ => 0,
        };
        let mut control = VirtualControl::new(synthetic_metadata(tuning))?;
        if self.scenario == Scenario::GuitarSilent {
            control = control.with_fault(ControlFault::DropMetadata(0))?;
        }
        let cancel = AtomicBool::new(false);
        Ok(read_session::read_current_on(&mut control, &cancel)?.metadata)
    }
}

/// "This computer": step 1 with a real OS-metadata reader; nothing else. It
/// has no USB, MIDI or audio path: steps 2–4 are refused without any I/O.
pub struct HostRig<S: OsMetadataSource> {
    reader: S,
    calls: BackendCalls,
}

impl<S: OsMetadataSource> HostRig<S> {
    pub fn new(reader: S) -> Self {
        Self {
            reader,
            calls: BackendCalls::default(),
        }
    }

    pub fn reader(&self) -> &S {
        &self.reader
    }

    pub const fn calls(&self) -> BackendCalls {
        self.calls
    }
}

const HOST_REFUSAL: &str =
    "this step is locked in this build: only gate G1 is open, so nothing was sent";

impl<S: OsMetadataSource> ProbeBackend for HostRig<S> {
    fn mode(&self) -> ProbeMode {
        ProbeMode::ThisComputer
    }

    fn can_run(&self, stage: Stage) -> bool {
        live_stage_available(stage)
    }

    fn os_metadata(&mut self) -> &dyn OsMetadataSource {
        self.calls.os_reads += 1;
        &self.reader
    }

    fn identify(&mut self) -> Result<Identity, String> {
        self.calls.identity_requests += 1;
        Err(HOST_REFUSAL.into())
    }

    fn read_settings(&mut self, _: ReadPass) -> Result<PresetMetadata, String> {
        self.calls.settings_reads += 1;
        Err(HOST_REFUSAL.into())
    }
}

/// The window's backend: simulation (default) or this computer.
pub enum RigBackend {
    Simulated(SimulatedRig),
    ThisComputer(HostRig<LinuxSysfs>),
}

impl RigBackend {
    pub const fn calls(&self) -> BackendCalls {
        match self {
            Self::Simulated(rig) => rig.calls(),
            Self::ThisComputer(rig) => rig.calls(),
        }
    }
}

impl ProbeBackend for RigBackend {
    fn mode(&self) -> ProbeMode {
        match self {
            Self::Simulated(rig) => rig.mode(),
            Self::ThisComputer(rig) => rig.mode(),
        }
    }

    fn can_run(&self, stage: Stage) -> bool {
        match self {
            Self::Simulated(rig) => rig.can_run(stage),
            Self::ThisComputer(rig) => rig.can_run(stage),
        }
    }

    fn os_metadata(&mut self) -> &dyn OsMetadataSource {
        match self {
            Self::Simulated(rig) => rig.os_metadata(),
            Self::ThisComputer(rig) => rig.os_metadata(),
        }
    }

    fn identify(&mut self) -> Result<Identity, String> {
        match self {
            Self::Simulated(rig) => rig.identify(),
            Self::ThisComputer(rig) => rig.identify(),
        }
    }

    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String> {
        match self {
            Self::Simulated(rig) => rig.read_settings(pass),
            Self::ThisComputer(rig) => rig.read_settings(pass),
        }
    }
}

// ---------------------------------------------------------------------------
// The session (state machine)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub body_model: i32,
    pub string1_tuning: i32,
}

fn reading_of(metadata: &PresetMetadata) -> Result<Reading, String> {
    let integer = |key: &str| match metadata.parameters.get(key) {
        Some(ParameterValue::Integer(value)) => Ok(*value),
        _ => Err(format!("the settings read did not contain {key}")),
    };
    Ok(Reading {
        body_model: integer("body_model")?,
        string1_tuning: integer(WATCHED_SETTING)?,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Declined,
    Cancelled,
    Failed,
}

pub struct ProbeSession<B: ProbeBackend> {
    backend: B,
    statuses: [StageStatus; 4],
    details: [Option<String>; 4],
    decisions: [Option<Consent>; 4],
    awaiting: Option<Stage>,
    stop: Option<(Stage, StopReason)>,
    first_read: Option<Reading>,
}

impl<B: ProbeBackend> ProbeSession<B> {
    pub fn new(backend: B) -> Self {
        let first = Some(Stage::UsbRecognition).filter(|stage| backend.can_run(*stage));
        Self {
            backend,
            statuses: [StageStatus::NotRun; 4],
            details: Default::default(),
            decisions: [None; 4],
            awaiting: first,
            stop: None,
            first_read: None,
        }
    }

    pub fn mode(&self) -> ProbeMode {
        self.backend.mode()
    }

    /// True when this backend can never run the stage (steps 2–4 in
    /// "This computer" mode): shown as locked and never offered.
    pub fn locked(&self, stage: Stage) -> bool {
        !self.backend.can_run(stage)
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The stage waiting for the person's answer. `None` once stopped or done.
    pub const fn awaiting(&self) -> Option<Stage> {
        self.awaiting
    }

    pub const fn status(&self, stage: Stage) -> StageStatus {
        self.statuses[stage.index()]
    }

    pub fn detail(&self, stage: Stage) -> Option<&str> {
        self.details[stage.index()].as_deref()
    }

    pub const fn stopped(&self) -> Option<(Stage, StopReason)> {
        self.stop
    }

    pub fn finished(&self) -> bool {
        self.awaiting.is_none()
    }

    /// Answer the awaiting stage. Only `Yes` runs it; anything else stops the
    /// sequence. A passed stage makes the next one await its own answer.
    pub fn answer(&mut self, consent: Consent) {
        let Some(stage) = self.awaiting.take() else {
            return;
        };
        if !self.backend.can_run(stage) {
            return;
        }
        let index = stage.index();
        self.decisions[index] = Some(consent);
        let reason = match consent {
            Consent::No => Some((StopReason::Declined, "You chose No. Nothing was run.")),
            Consent::Cancel => Some((StopReason::Cancelled, "Cancelled. Nothing was run.")),
            Consent::Yes => None,
        };
        if let Some((reason, text)) = reason {
            self.statuses[index] = StageStatus::Stopped;
            self.details[index] = Some(text.into());
            self.stop = Some((stage, reason));
            return;
        }
        let outcome = self.run(stage);
        let passed = outcome.is_ok();
        self.details[index] = Some(match outcome {
            Ok(text) | Err(text) => text,
        });
        if passed {
            self.statuses[index] = StageStatus::Passed;
            self.awaiting = stage.next().filter(|next| self.backend.can_run(*next));
        } else {
            self.statuses[index] = StageStatus::Stopped;
            self.stop = Some((stage, StopReason::Failed));
        }
    }

    fn run(&mut self, stage: Stage) -> Result<String, String> {
        match stage {
            Stage::UsbRecognition => {
                let recognition = recognise(self.backend.os_metadata());
                let summary = recognition.summary();
                if recognition.passed() {
                    Ok(summary)
                } else {
                    Err(summary)
                }
            }
            Stage::Identification => {
                let identity = self
                    .backend
                    .identify()
                    .map_err(|error| format!("Identification failed: {error}."))?;
                match identity.product_id() {
                    0x0007_0004 => Ok(format!(
                        "Guitar answered as Variax 300/600 (product {:08x}), firmware {}.",
                        identity.product_id(),
                        identity.version_string()
                    )),
                    other => Err(format!(
                        "The answer named product {other:08x}, not a Variax 300/600. Stopped."
                    )),
                }
            }
            Stage::SettingsRead => {
                let metadata = self
                    .backend
                    .read_settings(ReadPass::First)
                    .map_err(|error| format!("Settings read failed: {error}."))?;
                let reading = reading_of(&metadata)?;
                let text = format!(
                    "Read body model {} and {WATCHED_SETTING} = {}. This is one preset's current settings, not a backup.",
                    reading.body_model, reading.string1_tuning
                );
                self.first_read = Some(reading);
                Ok(text)
            }
            Stage::ChangeCheck => {
                let before = self
                    .first_read
                    .clone()
                    .ok_or("the change check needs a finished settings read")?;
                let metadata = self
                    .backend
                    .read_settings(ReadPass::AfterChange)
                    .map_err(|error| format!("Second read failed: {error}."))?;
                let after = reading_of(&metadata)?;
                Ok(if after.string1_tuning == before.string1_tuning {
                    format!(
                        "{WATCHED_SETTING} stayed {}. The second read finished; the value did not change.",
                        after.string1_tuning
                    )
                } else {
                    format!(
                        "{WATCHED_SETTING} changed from {} to {}.",
                        before.string1_tuning, after.string1_tuning
                    )
                })
            }
        }
    }

    /// Sanitized plain-text report: allowlisted keys, no serial numbers, no
    /// preset names, no file paths.
    pub fn report(&self) -> String {
        let mut lines = vec![
            match self.backend.mode() {
                ProbeMode::Simulation => "device-check schema=1 mode=simulation writes=none",
                ProbeMode::ThisComputer => {
                    "device-check schema=1 mode=this-computer gates-open=G1 writes=none"
                }
            }
            .to_owned(),
        ];
        for stage in Stage::LADDER {
            let index = stage.index();
            if self.locked(stage) && self.statuses[index] == StageStatus::NotRun {
                lines.push(format!(
                    "stage={} gate={} decision=none status=locked detail=-",
                    stage.number(),
                    stage.gate_label(),
                ));
                continue;
            }
            let decision = match self.decisions[index] {
                Some(Consent::Yes) => "yes",
                Some(Consent::No) => "no",
                Some(Consent::Cancel) => "cancel",
                None => "none",
            };
            lines.push(format!(
                "stage={} gate={} decision={decision} status={} detail={}",
                stage.number(),
                stage.gate_label(),
                self.statuses[index].label().replace(' ', "-"),
                self.details[index].as_deref().unwrap_or("-"),
            ));
        }
        lines.join("\n")
    }
}

/// Whether the live form of a stage can run in this build.
///
/// True only on Linux, only for [`Stage::UsbRecognition`], and only while that
/// stage's compile-time gate is open (G1). Other platforms do not offer a real
/// reader, even though the G1 constant is the same source on every target.
pub fn live_stage_available(stage: Stage) -> bool {
    cfg!(target_os = "linux")
        && LIVE_STAGES.contains(&stage)
        && !hardware_gates::is_locked(stage.gate())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(scenario: Scenario) -> ProbeSession<SimulatedRig> {
        ProbeSession::new(SimulatedRig::new(scenario))
    }

    #[test]
    fn default_consent_is_no_and_every_stage_has_action_and_risk_text() {
        assert_eq!(Consent::default(), Consent::No);
        assert_eq!(DEFAULT_CONSENT, Consent::No);
        for stage in Stage::LADDER {
            assert!(stage.action().len() > 40, "{stage:?}");
            assert!(stage.risk().len() > 40, "{stage:?}");
            // Only Linux step 1 (G1) has a live form in this build.
            assert_eq!(
                live_stage_available(stage),
                cfg!(target_os = "linux") && stage == Stage::UsbRecognition,
                "{stage:?}"
            );
            assert_eq!(
                hardware_gates::is_locked(stage.gate()),
                stage != Stage::UsbRecognition,
                "{stage:?}"
            );
        }
        assert!(Stage::UsbRecognition.action().contains("sends nothing"));
        assert!(Stage::UsbRecognition.risk().starts_with("Low risk"));
    }

    #[test]
    fn recognised_pod_lists_interfaces_drivers_and_sound_cards() {
        let recognition = recognise(&synthetic_podxt_live());
        let Recognition::Recognised {
            device,
            sound_cards,
        } = &recognition
        else {
            panic!("expected recognition, got {recognition:?}");
        };
        assert_eq!(device.sysfs_name, "1-2");
        assert_eq!(
            device.interfaces,
            vec![
                UsbInterface {
                    number: 0,
                    driver: Some("snd-usb-pod".into())
                },
                UsbInterface {
                    number: 1,
                    driver: Some("snd-usb-variax".into())
                },
            ]
        );
        assert_eq!(sound_cards.len(), 2);
        assert_eq!(sound_cards[1].id, "PODxtLive");
        assert_eq!(sound_cards[1].name, "PODxt Live");
        let summary = recognition.summary();
        assert!(summary.contains("0e41:4650"));
        assert!(summary.contains("interface 1 driver snd-usb-variax"));
    }

    #[test]
    fn absent_two_devices_and_malformed_metadata_stop() {
        assert_eq!(
            recognise(&SimulatedRig::new(Scenario::NothingConnected).sysfs),
            Recognition::Absent
        );
        assert_eq!(
            recognise(&SimulatedRig::new(Scenario::TwoPods).sysfs),
            Recognition::Several(2)
        );
        assert!(matches!(
            recognise(&SimulatedRig::new(Scenario::UnreadableInfo).sysfs),
            Recognition::Malformed(_)
        ));
        let mut bad_interface = synthetic_podxt_live();
        bad_interface.insert("bus/usb/devices/1-2:1.1/bInterfaceNumber", "one\n");
        assert!(matches!(
            recognise(&bad_interface),
            Recognition::Malformed(_)
        ));
        let mut no_cards = synthetic_podxt_live();
        no_cards.remove(CARDS);
        assert!(matches!(recognise(&no_cards), Recognition::Unreadable(_)));
        let mut bad_cards = synthetic_podxt_live();
        bad_cards.set_cards("garbage line\n");
        assert!(matches!(recognise(&bad_cards), Recognition::Malformed(_)));
        assert_eq!(
            parse_sound_cards("--- no soundcards ---\n").unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn decline_and_cancel_stop_without_running_anything() {
        for (consent, reason) in [
            (Consent::No, StopReason::Declined),
            (Consent::Cancel, StopReason::Cancelled),
        ] {
            let mut probe = session(Scenario::Connected);
            probe.answer(consent);
            assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Stopped);
            assert_eq!(probe.stopped(), Some((Stage::UsbRecognition, reason)));
            assert!(probe.finished());
            assert_eq!(probe.backend().calls(), BackendCalls::default());
            probe.answer(Consent::Yes);
            assert_eq!(probe.backend().calls(), BackendCalls::default());
        }
    }

    #[test]
    fn passed_stage_waits_for_its_own_yes_and_never_advances_itself() {
        let mut probe = session(Scenario::Connected);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
        assert_eq!(probe.awaiting(), Some(Stage::Identification));
        assert_eq!(probe.status(Stage::Identification), StageStatus::NotRun);
        assert_eq!(
            probe.backend().calls(),
            BackendCalls {
                os_reads: 1,
                ..Default::default()
            }
        );
        // Declining stage 2 leaves stage 1's result and stops.
        probe.answer(Consent::No);
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
        assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        assert_eq!(probe.backend().calls().identity_requests, 0);
    }

    #[test]
    fn stage_one_failures_stop_the_sequence() {
        for scenario in [
            Scenario::NothingConnected,
            Scenario::TwoPods,
            Scenario::UnreadableInfo,
        ] {
            let mut probe = session(scenario);
            probe.answer(Consent::Yes);
            assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Stopped);
            assert_eq!(
                probe.stopped(),
                Some((Stage::UsbRecognition, StopReason::Failed))
            );
            assert!(probe.finished());
            assert_eq!(probe.backend().calls().identity_requests, 0);
        }
    }

    #[test]
    fn full_simulated_ladder_reports_change_and_no_change() {
        let mut probe = session(Scenario::Connected);
        for _ in Stage::LADDER {
            probe.answer(Consent::Yes);
        }
        for stage in Stage::LADDER {
            assert_eq!(probe.status(stage), StageStatus::Passed, "{stage:?}");
        }
        assert!(
            probe
                .detail(Stage::Identification)
                .unwrap()
                .contains("00070004")
        );
        assert!(
            probe
                .detail(Stage::SettingsRead)
                .unwrap()
                .contains("body model 3")
        );
        assert_eq!(
            probe.detail(Stage::ChangeCheck).unwrap(),
            "string1_tuning changed from 0 to -2."
        );
        let report = probe.report();
        assert!(report.starts_with("device-check schema=1 mode=simulation writes=none"));
        assert!(
            !report.contains("SIMULATED"),
            "preset name must not be reported"
        );

        let mut unchanged = session(Scenario::NoChange);
        for _ in Stage::LADDER {
            unchanged.answer(Consent::Yes);
        }
        assert!(
            unchanged
                .detail(Stage::ChangeCheck)
                .unwrap()
                .contains("stayed 0")
        );
    }

    #[test]
    fn silent_guitar_stops_at_settings_read() {
        let mut probe = session(Scenario::GuitarSilent);
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::SettingsRead), StageStatus::Stopped);
        assert_eq!(probe.status(Stage::ChangeCheck), StageStatus::NotRun);
        assert!(probe.finished());
    }

    /// Test-only source that reads a fake tree without the gate, to exercise
    /// the same reader the gated `LinuxSysfs` uses. Unix only: sysfs `driver`
    /// entries are symlinks, which the fake tree recreates.
    #[cfg(unix)]
    struct FakeTree(PathBuf);

    #[cfg(unix)]
    impl OsMetadataSource for FakeTree {
        fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
            read_usb_tree(&self.0)?.usb_devices()
        }
        fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError> {
            parse_sound_cards(&read_cards_file(&self.0)?)
        }
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Lay out one device the way sysfs does: attribute files, interface
    /// directories and a relative `driver` symlink.
    #[cfg(unix)]
    fn fake_device(root: &Path, name: &str, product: u16, drivers: &[(u8, Option<&str>)]) {
        let devices = root.join(SYS_USB_DEVICES);
        let base = devices.join(name);
        write(&base.join("idVendor"), "0e41\n");
        write(&base.join("idProduct"), &format!("{product:04x}\n"));
        write(&base.join("bcdDevice"), "0001\n");
        write(&base.join("product"), "PODxt Live\n");
        // Never read: proves the reader keeps to the section 1 allowlist.
        write(&base.join("serial"), "SECRET-SERIAL\n");
        write(&base.join("descriptors"), "binary");
        let bus_drivers = root.join("sys/bus/usb/drivers");
        for (number, driver) in drivers {
            let interface = devices.join(format!("{name}:1.{number}"));
            write(
                &interface.join("bInterfaceNumber"),
                &format!("{number:02x}\n"),
            );
            if let Some(driver) = driver {
                std::fs::create_dir_all(bus_drivers.join(driver)).unwrap();
                std::os::unix::fs::symlink(
                    format!("../../../bus/usb/drivers/{driver}"),
                    interface.join("driver"),
                )
                .unwrap();
            }
        }
    }

    fn fake_tree() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        write(
            &root.path().join("sys/bus/usb/devices/usb1/idVendor"),
            "1d6b\n",
        );
        write(
            &root.path().join("sys/bus/usb/devices/usb1/idProduct"),
            "0002\n",
        );
        write(
            &root.path().join(PROC_CARDS),
            " 0 [Fake           ]: Fake - Fake onboard audio\n                      Fake onboard audio\n 1 [PODxtLive      ]: Line6-POD - PODxt Live\n                      Line 6 PODxt Live at USB 1-2:1.0\n",
        );
        root
    }

    #[cfg(unix)]
    const BOTH_DRIVERS: [(u8, Option<&str>); 2] =
        [(0, Some("snd-usb-pod")), (1, Some("snd-usb-variax"))];

    #[test]
    #[cfg(unix)]
    fn real_reader_recognises_pod_with_both_drivers_in_a_fake_tree() {
        let root = fake_tree();
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        let recognition = recognise(&FakeTree(root.path().into()));
        let Recognition::Recognised {
            device,
            sound_cards,
        } = &recognition
        else {
            panic!("expected recognition, got {recognition:?}");
        };
        assert_eq!(device.sysfs_name, "1-2");
        assert_eq!(device.product.as_deref(), Some("PODxt Live"));
        assert_eq!(device.bcd_device, Some(1));
        assert_eq!(
            device
                .interfaces
                .iter()
                .map(|i| (i.number, i.driver.as_deref()))
                .collect::<Vec<_>>(),
            vec![(0, Some("snd-usb-pod")), (1, Some("snd-usb-variax"))]
        );
        assert_eq!(sound_cards.len(), 2);
        let summary = recognition.summary();
        assert!(!summary.contains("SECRET"));
        let snapshot = read_usb_tree(root.path()).unwrap();
        assert!(
            snapshot
                .files
                .keys()
                .all(|key| !key.ends_with("/serial") && !key.ends_with("/descriptors"))
        );
    }

    #[test]
    #[cfg(unix)]
    fn real_reader_reports_absent_two_units_and_unbound_interface() {
        let root = fake_tree();
        assert_eq!(
            recognise(&FakeTree(root.path().into())),
            Recognition::Absent
        );
        // Another vendor device and a different Line 6 product are not matches.
        fake_device(root.path(), "1-4", 0x534d, &[(0, None)]);
        assert_eq!(
            recognise(&FakeTree(root.path().into())),
            Recognition::Absent
        );
        fake_device(
            root.path(),
            "1-2",
            0x4650,
            &[(0, Some("snd-usb-pod")), (1, None)],
        );
        let Recognition::Recognised { device, .. } = recognise(&FakeTree(root.path().into()))
        else {
            panic!("expected one PODxt Live");
        };
        assert_eq!(device.interfaces[1].driver, None);
        fake_device(root.path(), "1-3", 0x4650, &BOTH_DRIVERS);
        assert_eq!(
            recognise(&FakeTree(root.path().into())),
            Recognition::Several(2)
        );
    }

    #[test]
    #[cfg(unix)]
    fn real_reader_stops_on_malformed_or_missing_files() {
        let cases: [(&str, &[u8]); 4] = [
            ("sys/bus/usb/devices/1-2/idProduct", b"46?0\n"),
            ("sys/bus/usb/devices/1-2/idVendor", b"\xff\xfe\n"),
            ("sys/bus/usb/devices/1-2:1.1/bInterfaceNumber", b"one\n"),
            ("proc/asound/cards", b"garbage line\n"),
        ];
        for (path, bytes) in cases {
            let root = fake_tree();
            fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
            std::fs::write(root.path().join(path), bytes).unwrap();
            assert!(
                matches!(
                    recognise(&FakeTree(root.path().into())),
                    Recognition::Malformed(_)
                ),
                "{path}"
            );
        }
        let root = fake_tree();
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        std::fs::remove_file(root.path().join(PROC_CARDS)).unwrap();
        assert!(matches!(
            recognise(&FakeTree(root.path().into())),
            Recognition::Unreadable(_)
        ));
        let empty = tempfile::tempdir().unwrap();
        assert!(matches!(
            recognise(&FakeTree(empty.path().into())),
            Recognition::Unreadable(_)
        ));
    }

    /// A G1 reader on a temporary fake tree. Tests never build a reader on
    /// `DEFAULT_ROOT`; this guard fails before any read if one ever does.
    fn injected_reader(root: &Path) -> LinuxSysfs {
        assert_ne!(root, Path::new(DEFAULT_ROOT), "tests must not read /");
        assert!(root.starts_with(std::env::temp_dir()), "{root:?}");
        LinuxSysfs::with_root(root)
    }

    #[test]
    fn real_reader_reads_only_its_injected_root_with_g1_open() {
        // The production default is "/", checked as a constant only: no test
        // constructs a reader on it, let alone reads from it.
        assert_eq!(DEFAULT_ROOT, "/");
        assert!(!hardware_gates::is_locked(Gate::OsDiscovery));
        let root = fake_tree();
        #[cfg(unix)]
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        let reader = injected_reader(root.path());
        assert_eq!(reader.root(), root.path());
        let recognition = recognise(&reader);
        #[cfg(unix)]
        assert!(recognition.passed(), "{recognition:?}");
        #[cfg(not(unix))]
        assert_eq!(recognition, Recognition::Absent);
        // An empty temporary tree is unreadable, never a fall back to "/".
        let empty = tempfile::tempdir().unwrap();
        assert!(matches!(
            injected_reader(empty.path()).usb_devices(),
            Err(MetadataError::Unreadable(_))
        ));
    }

    #[cfg(target_os = "linux")]
    fn host_session(root: &Path) -> ProbeSession<HostRig<LinuxSysfs>> {
        ProbeSession::new(HostRig::new(injected_reader(root)))
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn this_computer_mode_runs_step_one_only_and_reports_without_serial() {
        let root = fake_tree();
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        let mut probe = host_session(root.path());
        assert_eq!(probe.mode(), ProbeMode::ThisComputer);
        assert_eq!(probe.awaiting(), Some(Stage::UsbRecognition));
        for stage in [
            Stage::Identification,
            Stage::SettingsRead,
            Stage::ChangeCheck,
        ] {
            assert!(probe.locked(stage), "{stage:?}");
        }
        assert_eq!(probe.backend().calls(), BackendCalls::default());
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
        // Steps 2–4 are never offered, so a further Yes runs nothing.
        assert_eq!(probe.awaiting(), None);
        assert!(probe.finished());
        probe.answer(Consent::Yes);
        assert_eq!(
            probe.backend().calls(),
            BackendCalls {
                os_reads: 1,
                ..Default::default()
            }
        );
        for stage in [
            Stage::Identification,
            Stage::SettingsRead,
            Stage::ChangeCheck,
        ] {
            assert_eq!(probe.status(stage), StageStatus::NotRun);
        }
        let report = probe.report();
        let lines: Vec<_> = report.lines().collect();
        assert_eq!(
            lines[0],
            "device-check schema=1 mode=this-computer gates-open=G1 writes=none"
        );
        assert!(lines[1].starts_with("stage=1 gate=G1 decision=yes status=passed detail="));
        for needle in [
            "0e41:4650",
            "USB 1-2",
            "interface 0 driver snd-usb-pod",
            "interface 1 driver snd-usb-variax",
            "\"PODxt Live\"",
        ] {
            assert!(lines[1].contains(needle), "{needle}: {}", lines[1]);
        }
        assert_eq!(
            lines[2],
            "stage=2 gate=G2 decision=none status=locked detail=-"
        );
        assert_eq!(
            lines[3],
            "stage=3 gate=G3 decision=none status=locked detail=-"
        );
        assert_eq!(
            lines[4],
            "stage=4 gate=G3 decision=none status=locked detail=-"
        );
        assert!(!report.contains("SECRET"), "serial number leaked");
        assert!(
            !report.contains(&*root.path().to_string_lossy()),
            "path leaked"
        );
        assert!(!report.contains("SIMULATED"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn this_computer_mode_decline_and_cancel_read_nothing() {
        let root = fake_tree();
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        for (consent, decision) in [(Consent::No, "no"), (Consent::Cancel, "cancel")] {
            let mut probe = host_session(root.path());
            probe.answer(consent);
            assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Stopped);
            assert!(probe.finished());
            assert_eq!(probe.backend().calls(), BackendCalls::default());
            assert!(probe.report().contains(&format!(
                "stage=1 gate=G1 decision={decision} status=stopped"
            )));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn this_computer_mode_stops_when_no_or_two_pods_are_listed() {
        let root = fake_tree();
        let mut probe = host_session(root.path());
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Stopped);
        assert_eq!(
            probe.stopped(),
            Some((Stage::UsbRecognition, StopReason::Failed))
        );
        fake_device(root.path(), "1-2", 0x4650, &BOTH_DRIVERS);
        fake_device(root.path(), "1-3", 0x4650, &BOTH_DRIVERS);
        let mut probe = host_session(root.path());
        probe.answer(Consent::Yes);
        assert!(
            probe
                .detail(Stage::UsbRecognition)
                .unwrap()
                .contains("2 PODxt Live")
        );
        assert!(probe.finished());
    }

    #[test]
    fn host_rig_refuses_steps_two_to_four_without_io() {
        let empty = tempfile::tempdir().unwrap();
        let mut rig = HostRig::new(injected_reader(empty.path()));
        for stage in [
            Stage::Identification,
            Stage::SettingsRead,
            Stage::ChangeCheck,
        ] {
            assert!(!rig.can_run(stage), "{stage:?}");
        }
        assert!(rig.identify().is_err());
        assert!(rig.read_settings(ReadPass::First).is_err());
        assert_eq!(rig.calls().os_reads, 0);
    }

    #[test]
    fn simulation_mode_holds_no_real_reader() {
        // `RigBackend::Simulated` wraps a `SimulatedRig`, which has no
        // `LinuxSysfs` field; a full simulated ladder stays in that variant.
        let mut probe = ProbeSession::new(RigBackend::Simulated(SimulatedRig::new(
            Scenario::Connected,
        )));
        assert_eq!(probe.mode(), ProbeMode::Simulation);
        for _ in Stage::LADDER {
            probe.answer(Consent::Yes);
        }
        assert!(matches!(probe.backend(), RigBackend::Simulated(_)));
        for stage in Stage::LADDER {
            assert!(!probe.locked(stage));
            assert_eq!(probe.status(stage), StageStatus::Passed);
        }
        assert!(
            probe
                .report()
                .starts_with("device-check schema=1 mode=simulation")
        );
    }
}

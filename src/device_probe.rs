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
//! The host backend reads the OS metadata through the G1 gate. Step 2
//! needs this session's successful step 1 with the expected driver binding.
//! Step 3 reads the Variax settings and PODxt Live tone. The host backend
//! leaves step 4 unavailable. The simulated backend uses injected sources
//! for all four steps. See `docs/device-check.md`.

use crate::firmware_notice::FirmwareNotice;
use crate::hardware_gates::{self, Gate};
use crate::identity_stage::{
    self, CanonicalIdentity, IdentifyError, IdentifyStop, IdentityFailure, IdentityPair,
    IdentityRole,
};
use crate::read_session;
use crate::simulation::{ControlFault, VirtualControl};
use crate::{ParameterValue, PresetMetadata};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// USB vendor/product of the PODxt Live (decimal 3649/18000).
pub const PODXT_LIVE_USB: (u16, u16) = (0x0e41, 0x4650);
/// The setting stage 4 asks the person to change (Workbench's internal string 1).
pub const WATCHED_SETTING: &str = "string1_tuning";
/// Stages with a prepared live backend. Step 2 is listed so a build that opens
/// only G2 can run the identity request. It is still not offered while G2 is
/// locked, and it is absent unless this binary was built with the `usb` feature.
pub const LIVE_STAGES: [Stage; 2] = [Stage::UsbRecognition, Stage::Identification];
/// The answer used whenever the person has not chosen.
pub const DEFAULT_CONSENT: Consent = Consent::No;
pub const THIS_COMPUTER_LABEL: &str = "This computer (G1) — step 1 reads what Linux already knows about connected USB devices and sound cards. Nothing is sent to the PODxt Live or the guitar.";
/// Driver on PODxt Live interface 0 when the rig is ready for step 2.
pub const EXPECTED_POD_DRIVER: &str = "snd-usb-pod";
/// Driver on PODxt Live interface 1 when the rig is ready for step 2.
pub use line6_devices::family::variax::driver::EXPECTED_VARIAX_DRIVER;
/// What the window can actually cancel. Close is handled before Yes in the
/// same frame. After Yes, the exchange runs on the UI thread and the window
/// has no way to stop it.
pub const CANCEL_LIMIT: &str = "Close stops the step before it starts. Once Yes has started, this window runs the exchange through to the end and cannot cancel it.";
/// The app's one request, kept separate from the kernel driver's own startup.
pub const APP_REQUEST_VERSUS_LINUX: &str = "The app sends one identity request. After the driver is given back, Linux's own snd-usb-variax startup can send that same kind of question by itself. That later traffic is Linux, not a second request from this app.";
pub const STEP_TWO_PREFLIGHT: &str = "Step 2 is offered only after this session's step 1 shows one PODxt Live, snd-usb-pod on interface 0, and snd-usb-variax on interface 1.";
const MAC_STEP_TWO_PREFLIGHT: &str =
    "Step 2 uses the one PODxt Live attachment selected by this session's step 1.";

pub fn step_two_preflight() -> &'static str {
    if crate::macos_usb_available() {
        MAC_STEP_TWO_PREFLIGHT
    } else {
        STEP_TWO_PREFLIGHT
    }
}
pub const STEP_TWO_YES: &str = "Yes, send one Variax identity request";
pub const STEP_TWO_YES_SIMULATION: &str = "Yes, rehearse one Variax identity request (simulation)";
pub const SAVE_THEN_CHECK_AGAIN: &str = "Save the step 2 report, then run step 1 again";
pub const SAVE_BEFORE_RESTART: &str = "Save the step 2 report before running step 1 again.";

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
            Self::SettingsRead => Gate::DeviceCheckCurrentSettingsRead,
            Self::ChangeCheck => Gate::CurrentSettingsRead,
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
        if crate::macos_usb_available() {
            match self {
                Self::UsbRecognition => {
                    return "Look for the PODxt Live in the Mac USB attachment list. This step sends no MIDI and claims no interface.";
                }
                Self::Identification => {
                    return "Send one Variax identity request through the PODxt Live selected in step 1. The app releases the interface afterwards. It stops if another application or driver holds the connection.";
                }
                _ => {}
            }
        }
        match self {
            Self::UsbRecognition => {
                "Look at what Linux already knows about connected USB devices: vendor and product numbers, the description Linux saved when the device was plugged in, which Linux driver owns each part of it, and the list of sound cards. The app sends nothing to the PODxt Live or the guitar."
            }
            Self::Identification => concat!(
                "Send one identity request and accept only the Variax 600 answer. A real run sends that question, then reads the answer in short pieces and waits up to 3 seconds. Incomplete and missing replies stop the check. ",
                "The app sends one identity request. After the driver is given back, Linux's own snd-usb-variax startup can send that same kind of question by itself. That later traffic is Linux, not a second request from this app. ",
                "A PODxt Live identity is not requested in this step. Step 3 reads the Variax settings and the PODxt Live tone. Nothing is stored on the devices. Choosing Yes accepts the risk for this step. No stays the default. There is no password. If Linux refuses the open, the question is not sent and the guitar driver stays in place. After a reported success, save the steps 1–2 report, then run step 1 again and confirm snd-usb-pod on interface 0 and snd-usb-variax on interface 1."
            ),
            Self::SettingsRead => {
                "Read the Variax current settings and the PODxt Live current tone. The guitar read turns the guitar-control session on, reads two settings blocks, then turns the session off. The PODxt Live read asks its name and then reads the current tone. Nothing is written. The result is the current settings, not a backup. Step 4 stays off."
            }
            Self::ChangeCheck => {
                "You change the first-string tuning on the guitar yourself, then the app reads the settings again the same way and reports whether that value changed. Nothing is written to the guitar."
            }
        }
    }

    /// Exact plain-language risk text shown before consent.
    pub const fn risk(self) -> &'static str {
        if crate::macos_usb_available() {
            match self {
                Self::UsbRecognition => {
                    return "Low risk. If there is no PODxt Live or more than one, the check stops.";
                }
                Self::Identification => {
                    return "Medium risk. This asks only for the guitar's identity. It leaves presets and settings alone.";
                }
                _ => {}
            }
        }
        match self {
            Self::UsbRecognition => {
                "Low risk. The app only reads information Linux keeps. When the PODxt Live was plugged in, Linux's own drivers already talked to it (for example the Variax driver asks for a version); that happens without this app. If no device, several devices or unreadable information is found, the check stops instead of guessing."
            }
            Self::Identification => {
                "Medium risk on a real run, higher than USB recognition. Yes would send one identity question and briefly take the guitar-control connection away from Linux. It does not change presets or settings. If Linux does not get that connection back, unplug the PODxt Live and plug it in again, and do not continue."
            }
            Self::SettingsRead => {
                "High risk, higher than identification. Several messages go to the guitar and change its session state while reading, and the PODxt Live is asked for its name and its current tone. Nothing is stored. The result is the current settings, not a backup."
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

    /// Cached Linux USB enumeration address at a detected bus and port path.
    /// A new address on the same port means the previous attachment is stale.
    pub fn usb_device_address(&self, path: &str) -> Option<u8> {
        if !path.split_once('-').is_some_and(|(bus, ports)| {
            let number = |part: &str| part.parse::<u8>().is_ok_and(|n| n != 0);
            number(bus) && ports.split('.').all(number)
        }) {
            return None;
        }
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
            read_attribute(
                &self.root.join(SYS_USB_DEVICES).join(path).join("devnum"),
                path,
            )
            .map_err(|error| error.to_string())
        })
        .ok()
        .flatten()
        .and_then(|text| text.trim().parse::<u8>().ok())
        .filter(|address| (1..=127).contains(address))
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
                if crate::macos_usb_available() {
                    return format!("PODxt Live found at USB {}.", device.sysfs_name);
                }
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

/// Linux names these modules with a hyphen (`snd-usb-pod`) and sometimes
/// shows the sysfs driver link with an underscore (`snd_usb_pod`). Both
/// spellings are the same driver. The character must still be one or the
/// other; a different name does not match.
fn same_driver_spelling(observed: &str, expected: &str) -> bool {
    let spelling = |name: &str| name.replace('_', "-");
    spelling(observed) == spelling(expected)
}

/// The rig step 2 is allowed to follow: one PODxt Live, the POD driver on
/// interface 0, and the Variax driver on interface 1. Either spelling of
/// each name counts. The drivers may not be swapped, and any other binding
/// is not a pass for that offer.
pub fn expected_driver_binding(device: &UsbDevice) -> bool {
    if (device.vendor_id, device.product_id) != PODXT_LIVE_USB {
        return false;
    }
    let driver = |number: u8| {
        device
            .interfaces
            .iter()
            .find(|interface| interface.number == number)
            .and_then(|interface| interface.driver.as_deref())
    };
    match (driver(0), driver(1)) {
        (Some(pod), Some(variax)) => {
            same_driver_spelling(pod, EXPECTED_POD_DRIVER)
                && line6_devices::family::variax::driver::is_variax_driver(variax)
        }
        _ => false,
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
    pub fn label(self) -> &'static str {
        use crate::probe_report::OpenGates;
        match self {
            Self::Simulation => "Simulation",
            Self::ThisComputer => match open_gates() {
                OpenGates::G1ThroughG3 => "This computer (G1–G3)",
                OpenGates::G1AndG2 => "This computer (G1 and G2)",
                OpenGates::G2 => "This computer (G2)",
                OpenGates::G1 | OpenGates::None => "This computer (G1)",
            },
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
    fn recognise_usb(&mut self) -> Result<Recognition, String> {
        Ok(recognise(self.os_metadata()))
    }
    fn identification_ready(&self, device: &UsbDevice) -> bool {
        expected_driver_binding(device)
    }
    fn identify(&mut self) -> Result<IdentityPair, IdentifyError>;
    fn identity_open_error(&self) -> Option<&str> {
        None
    }
    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String>;
    /// Extra words for a live settings read. Simulation leaves this empty.
    fn firmware_notice(&self) -> Option<FirmwareNotice> {
        None
    }
    fn settings_note(&self) -> Option<&str> {
        None
    }
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
    identity: identity_stage::ScriptedIdentity,
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
        Self::with_identity(scenario, identity_stage::happy_script())
    }

    /// Stage 2 reads this in-memory transport and nothing else.
    pub fn with_identity(scenario: Scenario, identity: identity_stage::ScriptedIdentity) -> Self {
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
            identity,
        }
    }

    /// Calls recorded by the stage 2 transport. Zero means it was not used.
    pub fn identity_calls(&self) -> u32 {
        self.identity.calls()
    }

    pub const fn scenario(&self) -> Scenario {
        self.scenario
    }

    pub const fn calls(&self) -> BackendCalls {
        self.calls
    }

    /// Replace one interface driver on the synthetic PODxt Live at `1-2`.
    /// Tests use this to prove a mismatched binding does not offer step 2.
    pub fn set_interface_driver(&mut self, number: u8, driver: Option<&str>) {
        let key = format!("bus/usb/devices/1-2:1.{number}/driver");
        match driver {
            Some(driver) => {
                self.sysfs
                    .insert(&key, &format!("../../../../../../bus/usb/drivers/{driver}"));
            }
            None => {
                self.sysfs.remove(&key);
            }
        }
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

    fn identify(&mut self) -> Result<IdentityPair, IdentifyError> {
        self.calls.identity_requests += 1;
        let exchange =
            identity_stage::run_identity_exchange(&mut self.identity, &AtomicBool::new(false));
        identity_stage::exchange_result(exchange)
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

/// "This computer": step 1 reads OS metadata. Step 2 calls the identity-only
/// USB path when this binary includes `usb` and gate G2 is open. A test
/// build returns from that path before enumeration. Step 3 reads both
/// families outside tests. Step 4 stays refused.
pub struct HostRig<S: OsMetadataSource> {
    reader: S,
    lock_recognition: bool,
    calls: BackendCalls,
    /// Plain text from the last live settings read, when one finished.
    settings_note: Option<String>,
    firmware_notice: Option<FirmwareNotice>,
    #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
    mac_live: bool,
    #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
    mac_attachment: Option<line6_devices::transport::usb::macos::Attachment>,
    #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
    mac_open_error: Option<String>,
}

impl<S: OsMetadataSource> HostRig<S> {
    pub fn new(reader: S) -> Self {
        Self {
            reader,
            lock_recognition: false,
            calls: BackendCalls::default(),
            settings_note: None,
            firmware_notice: None,
            #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
            mac_live: false,
            #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
            mac_attachment: None,
            #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
            mac_open_error: None,
        }
    }

    /// Coordinate GUI recognition with the Follow worker, which may have
    /// detached the driver whose binding the first step inspects.
    pub fn with_recognition_lock(mut self) -> Self {
        self.lock_recognition = true;
        self
    }

    /// A native GUI opts in explicitly. Injected readers and GUI test builds
    /// never use the system USB source, even when linked against the live lib.
    #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
    pub fn with_macos_usb(mut self) -> Self {
        self.mac_live = true;
        self
    }

    pub fn reader(&self) -> &S {
        &self.reader
    }

    pub const fn calls(&self) -> BackendCalls {
        self.calls
    }
}

fn host_refusal() -> String {
    format!(
        "this step is locked in this build ({}); nothing was sent",
        open_gates().token()
    )
}

impl<S: OsMetadataSource> ProbeBackend for HostRig<S> {
    fn mode(&self) -> ProbeMode {
        ProbeMode::ThisComputer
    }

    fn can_run(&self, stage: Stage) -> bool {
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        if !self.mac_live {
            return false;
        }
        live_stage_available(stage)
    }

    fn os_metadata(&mut self) -> &dyn OsMetadataSource {
        self.calls.os_reads += 1;
        &self.reader
    }

    fn recognise_usb(&mut self) -> Result<Recognition, String> {
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        {
            use line6_devices::{device_lock::DeviceLock, transport::usb::macos};

            if !self.mac_live {
                return Err(host_refusal());
            }
            self.calls.os_reads += 1;
            self.mac_attachment = None;
            return hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
                let _rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
                let attachments = macos::attachments()?;
                match attachments.as_slice() {
                    [] => Ok(Recognition::Absent),
                    [attachment] => {
                        self.mac_attachment = Some(attachment.clone());
                        Ok(Recognition::Recognised {
                            device: UsbDevice {
                                sysfs_name: attachment.path.clone(),
                                vendor_id: PODXT_LIVE_USB.0,
                                product_id: PODXT_LIVE_USB.1,
                                product: Some("PODxt Live".into()),
                                bcd_device: None,
                                interfaces: Vec::new(),
                            },
                            sound_cards: Vec::new(),
                        })
                    }
                    _ => Ok(Recognition::Several(attachments.len())),
                }
            });
        }
        #[cfg(not(all(target_os = "macos", feature = "usb", not(test))))]
        {
            // Follow has to give interface 1 back before checking its binding.
            let _rig = (cfg!(target_os = "linux") && self.lock_recognition)
                .then(line6_devices::device_lock::DeviceLock::acquire)
                .transpose()
                .map_err(|error| error.to_string())?;
            Ok(recognise(self.os_metadata()))
        }
    }

    fn identification_ready(&self, device: &UsbDevice) -> bool {
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        return self
            .mac_attachment
            .as_ref()
            .is_some_and(|attachment| attachment.path == device.sysfs_name);
        #[cfg(not(all(target_os = "macos", feature = "usb", not(test))))]
        expected_driver_binding(device)
    }

    fn identify(&mut self) -> Result<IdentityPair, IdentifyError> {
        self.calls.identity_requests += 1;
        // This path enforces the single-request G2 exchange before any USB open.
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        {
            use line6_devices::device_lock::{DeviceLock, LockError};
            if !self.mac_live {
                return Err(IdentifyError::Locked);
            }
            self.mac_open_error = None;
            let attachment = self.mac_attachment.as_ref().ok_or(IdentifyError::Locked)?;
            let rig = DeviceLock::acquire().map_err(|error| {
                crate::usb::open_stopped(match error {
                    LockError::Busy | LockError::BusyHere => IdentityFailure::RigBusy,
                    LockError::Unavailable(_) => IdentityFailure::LockUnavailable,
                })
            })?;
            crate::usb::read_macos_identity(
                &rig,
                (&attachment.path, attachment.address),
                &mut self.mac_open_error,
            )
        }
        #[cfg(all(feature = "usb", not(all(target_os = "macos", not(test)))))]
        {
            crate::usb::read_identity(&AtomicBool::new(false))
        }
        #[cfg(not(feature = "usb"))]
        {
            Err(IdentifyError::Locked)
        }
    }

    fn identity_open_error(&self) -> Option<&str> {
        #[cfg(all(target_os = "macos", feature = "usb", not(test)))]
        return self.mac_open_error.as_deref();
        #[cfg(not(all(target_os = "macos", feature = "usb", not(test))))]
        None
    }

    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String> {
        self.calls.settings_reads += 1;
        self.settings_note = None;
        if pass == ReadPass::AfterChange || !live_stage_available(Stage::SettingsRead) {
            return Err(host_refusal());
        }
        #[cfg(feature = "usb")]
        {
            #[cfg(all(target_os = "macos", not(test)))]
            let both = {
                let attachment = self.mac_attachment.as_ref().ok_or_else(host_refusal)?;
                let rig = line6_devices::device_lock::DeviceLock::acquire()
                    .map_err(|error| error.to_string())?;
                crate::live_families::read_both_on(&attachment.path, attachment.address, &rig)?
            };
            #[cfg(not(all(target_os = "macos", not(test))))]
            let both = crate::live_families::read_both()?;
            self.firmware_notice = both.firmware_notice;
            if let Some(notice) = both.firmware_notice {
                return Err(notice.message());
            }
            self.settings_note = Some(both.summary);
            Ok(both.metadata)
        }
        #[cfg(not(feature = "usb"))]
        {
            Err("this build does not open a device for step 3; nothing was sent".into())
        }
    }

    fn firmware_notice(&self) -> Option<FirmwareNotice> {
        self.firmware_notice
    }

    fn settings_note(&self) -> Option<&str> {
        self.settings_note.as_deref()
    }
}

/// A staged check uses the host reader or an injected simulated rig.
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

    fn recognise_usb(&mut self) -> Result<Recognition, String> {
        match self {
            Self::Simulated(rig) => rig.recognise_usb(),
            Self::ThisComputer(rig) => rig.recognise_usb(),
        }
    }

    fn identify(&mut self) -> Result<IdentityPair, IdentifyError> {
        match self {
            Self::Simulated(rig) => rig.identify(),
            Self::ThisComputer(rig) => rig.identify(),
        }
    }

    fn identification_ready(&self, device: &UsbDevice) -> bool {
        match self {
            Self::Simulated(rig) => rig.identification_ready(device),
            Self::ThisComputer(rig) => rig.identification_ready(device),
        }
    }

    fn identity_open_error(&self) -> Option<&str> {
        match self {
            Self::Simulated(rig) => rig.identity_open_error(),
            Self::ThisComputer(rig) => rig.identity_open_error(),
        }
    }

    fn read_settings(&mut self, pass: ReadPass) -> Result<PresetMetadata, String> {
        match self {
            Self::Simulated(rig) => rig.read_settings(pass),
            Self::ThisComputer(rig) => rig.read_settings(pass),
        }
    }

    fn firmware_notice(&self) -> Option<FirmwareNotice> {
        match self {
            Self::Simulated(rig) => rig.firmware_notice(),
            Self::ThisComputer(rig) => rig.firmware_notice(),
        }
    }

    fn settings_note(&self) -> Option<&str> {
        match self {
            Self::Simulated(rig) => rig.settings_note(),
            Self::ThisComputer(rig) => rig.settings_note(),
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

fn observed_guitar(identity: &crate::midi::Identity) -> Option<CanonicalIdentity> {
    let version = identity.version_string();
    if identity.product_id() == identity_stage::GUITAR_PRODUCT
        && version == identity_stage::GUITAR_VERSION
    {
        Some(CanonicalIdentity {
            role: IdentityRole::Guitar,
            product_id: identity.product_id(),
            version: identity_stage::GUITAR_VERSION,
        })
    } else {
        None
    }
}

fn identity_detail(stop: &IdentifyStop) -> String {
    if let Some(notice) = stop.firmware_notice
        && stop.cleanup.is_none()
    {
        return notice.message();
    }
    let mut text = match stop.cleanup {
        Some(cleanup) if cleanup != stop.failure => {
            format!("{} {}", stop.failure.sentence(), cleanup.sentence())
        }
        _ => stop.failure.sentence().to_owned(),
    };
    if stop.release_failed && stop.reattach_failed {
        text.push_str(" Release failed. Reattach failed.");
    }
    if stop.reattach_failed {
        text.push_str(" If the guitar driver stays unbound, use Refresh guitar to reattach it. Replug PODxt Live USB as a last resort.");
    }
    if let Some(notice) = stop.firmware_notice {
        text.push(' ');
        text.push_str(&notice.message());
    }
    text
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
    identity_failure: Option<IdentityFailure>,
    firmware_notice: Option<FirmwareNotice>,
    cleanup_failure: Option<IdentityFailure>,
    /// True only after the fake identity exchange has run release/reattach.
    cleanup_ran: bool,
    decoded_bridge: Option<CanonicalIdentity>,
    decoded_guitar: Option<CanonicalIdentity>,
    /// Set only by the step 1 answer in this session, and only when that read
    /// shows the expected PODxt Live driver binding. A new session starts false.
    g1_expected_binding: bool,
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
            identity_failure: None,
            firmware_notice: None,
            cleanup_failure: None,
            cleanup_ran: false,
            decoded_bridge: None,
            decoded_guitar: None,
            g1_expected_binding: false,
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

    pub fn firmware_notice(&self) -> Option<FirmwareNotice> {
        self.firmware_notice
            .or_else(|| self.backend.firmware_notice())
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

    /// True after step 2 has been answered in this session, including No.
    pub fn identification_answered(&self) -> bool {
        self.decisions[Stage::Identification.index()].is_some()
    }

    pub const fn g1_expected_binding(&self) -> bool {
        self.g1_expected_binding
    }

    /// Step 2 is offered only when this backend can run it and this session's
    /// step 1 just passed the expected driver binding.
    fn may_offer(&self, stage: Stage) -> bool {
        if !self.backend.can_run(stage) {
            return false;
        }
        if stage == Stage::Identification && !self.g1_expected_binding {
            return false;
        }
        true
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
        if stage == Stage::Identification && !self.g1_expected_binding {
            self.statuses[index] = StageStatus::Stopped;
            self.details[index] = Some(if crate::macos_usb_available() {
                format!("{MAC_STEP_TWO_PREFLIGHT} Nothing was sent.")
            } else {
                "Step 2 needs a fresh step 1 pass for one PODxt Live with snd-usb-pod on interface 0 and snd-usb-variax on interface 1. Nothing was sent.".into()
            });
            self.stop = Some((stage, StopReason::Failed));
            return;
        }
        let outcome = self.run(stage);
        let passed = outcome.is_ok();
        self.details[index] = Some(match outcome {
            Ok(text) | Err(text) => text,
        });
        if passed {
            self.statuses[index] = StageStatus::Passed;
            self.awaiting = stage.next().filter(|next| self.may_offer(*next));
        } else {
            self.statuses[index] = StageStatus::Stopped;
            self.stop = Some((stage, StopReason::Failed));
        }
    }

    fn run(&mut self, stage: Stage) -> Result<String, String> {
        match stage {
            Stage::UsbRecognition => {
                let recognition = self.backend.recognise_usb()?;
                let summary = recognition.summary();
                let binding = match &recognition {
                    Recognition::Recognised { device, .. } => {
                        self.backend.identification_ready(device)
                    }
                    _ => false,
                };
                self.g1_expected_binding = binding;
                if recognition.passed() && binding {
                    Ok(summary)
                } else if recognition.passed() {
                    Err(format!(
                        "{summary} {} Step 2 is not offered.",
                        step_two_preflight()
                    ))
                } else {
                    Err(summary)
                }
            }
            Stage::Identification => match self.backend.identify() {
                Ok(pair) => {
                    self.remember_identity(None, observed_guitar(&pair.guitar), None);
                    Ok(format!(
                        "Variax 600 {:08x} firmware {}. PODxt Live identity was not requested.",
                        pair.guitar.product_id(),
                        pair.guitar.version_string()
                    ))
                }
                Err(IdentifyError::Locked) => Err(host_refusal()),
                Err(IdentifyError::Failed(stop)) => {
                    self.cleanup_ran = stop.cleanup_ran;
                    self.remember_identity(stop.bridge, stop.guitar, Some(stop));
                    let mut detail = identity_detail(&stop);
                    if let Some(error) = self.backend.identity_open_error() {
                        detail.push(' ');
                        detail.push_str(error);
                    }
                    Err(detail)
                }
            },
            Stage::SettingsRead => {
                let metadata = self
                    .backend
                    .read_settings(ReadPass::First)
                    .map_err(|error| format!("Settings read failed: {error}."))?;
                let reading = reading_of(&metadata)?;
                let text = self.backend.settings_note().map_or_else(
                    || {
                        format!(
                            "Read body model {} and {WATCHED_SETTING} = {}. This is one preset's current settings, not a backup.",
                            reading.body_model, reading.string1_tuning
                        )
                    },
                    str::to_owned,
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
        let header = match self.backend.mode() {
            ProbeMode::Simulation => "device-check schema=1 mode=simulation writes=none".to_owned(),
            ProbeMode::ThisComputer => format!(
                "device-check schema=1 mode=this-computer {} writes=none",
                open_gates().token()
            ),
        };
        let mut lines = vec![header];
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

    /// Stages 1–2 only, from fixed tokens. Saving it does not run a stage.
    pub fn sanitized_report(&self) -> String {
        crate::probe_report::render(&self.sanitized_facts())
    }

    pub fn save_sanitized_report(&self, path: &std::path::Path) -> Result<(), &'static str> {
        crate::probe_report::save_sanitized_report(path, &self.sanitized_report())
    }

    fn remember_identity(
        &mut self,
        bridge: Option<CanonicalIdentity>,
        guitar: Option<CanonicalIdentity>,
        stop: Option<IdentifyStop>,
    ) {
        self.decoded_bridge = bridge;
        self.decoded_guitar = guitar;
        self.cleanup_ran = true;
        if let Some(stop) = stop {
            self.identity_failure = Some(stop.failure);
            self.firmware_notice = stop.firmware_notice;
            self.cleanup_failure = stop.cleanup;
            self.cleanup_ran = stop.cleanup_ran;
        }
    }

    fn sanitized_facts(&self) -> crate::probe_report::SanitizedFacts {
        use crate::probe_report::{
            ReportCleanup, ReportDecision, ReportGate, ReportIdentity, ReportMode, ReportStatus,
            ReportStop, StageFact,
        };
        let mode = match self.mode() {
            ProbeMode::Simulation => ReportMode::Simulation,
            ProbeMode::ThisComputer => ReportMode::ThisComputer,
        };
        let fact = |stage: Stage| {
            let index = stage.index();
            let status = if self.locked(stage) && self.statuses[index] == StageStatus::NotRun {
                ReportStatus::Locked
            } else {
                match self.statuses[index] {
                    StageStatus::NotRun => ReportStatus::NotRun,
                    StageStatus::Passed => ReportStatus::Passed,
                    StageStatus::Stopped => ReportStatus::Stopped,
                }
            };
            let decision = match self.decisions[index] {
                Some(Consent::Yes) => ReportDecision::Yes,
                Some(Consent::No) => ReportDecision::No,
                Some(Consent::Cancel) => ReportDecision::Cancel,
                None => ReportDecision::None,
            };
            let stop = match (status, decision, stage) {
                (ReportStatus::Stopped, ReportDecision::No, _) => ReportStop::Declined,
                (ReportStatus::Stopped, ReportDecision::Cancel, _) => ReportStop::Cancelled,
                (ReportStatus::Stopped, ReportDecision::Yes, Stage::Identification) => self
                    .identity_failure
                    .map(ReportStop::Identity)
                    .unwrap_or(ReportStop::Failed),
                (ReportStatus::Stopped, _, _) => ReportStop::Failed,
                _ => ReportStop::None,
            };
            let (bridge, guitar, cleanup) = if stage == Stage::Identification {
                let cleanup = if !self.cleanup_ran {
                    ReportCleanup::NotRun
                } else if self.cleanup_failure.is_some() {
                    ReportCleanup::Failed
                } else {
                    ReportCleanup::Ok
                };
                (
                    ReportIdentity::from_canonical(self.decoded_bridge),
                    ReportIdentity::from_canonical(self.decoded_guitar),
                    cleanup,
                )
            } else {
                (
                    ReportIdentity::None,
                    ReportIdentity::None,
                    ReportCleanup::NotRun,
                )
            };
            StageFact {
                stage: stage.number(),
                gate: match stage {
                    Stage::Identification => ReportGate::IdentityRequest,
                    Stage::UsbRecognition | Stage::SettingsRead | Stage::ChangeCheck => {
                        ReportGate::OsDiscovery
                    }
                },
                decision,
                status,
                stop,
                cleanup,
                bridge,
                guitar,
            }
        };
        crate::probe_report::SanitizedFacts {
            mode,
            gates: open_gates(),
            stages: [fact(Stage::UsbRecognition), fact(Stage::Identification)],
        }
    }
}

/// Whether the live form of a stage can run in this build.
///
/// Step 1 is offered on Linux while G1 is open. Step 2 is offered on Linux
/// only when G2 is open and this binary includes `usb`. The `gui` feature
/// alone does not offer step 2. While G2 is locked, step 2 stays off even
/// though it is listed in [`LIVE_STAGES`]. Step 3 reads the Variax and the
/// PODxt Live when `usb` is on. Step 4 is not offered. Other platforms do
/// not offer a real reader.
pub fn live_stage_available(stage: Stage) -> bool {
    if !cfg!(target_os = "linux") && !crate::macos_usb_available() {
        return false;
    }
    // Step 3 has its own Device check gate. Step 4 has no host procedure.
    if stage == Stage::SettingsRead {
        return cfg!(feature = "usb")
            && !hardware_gates::is_locked(Gate::DeviceCheckCurrentSettingsRead)
            && live_stage_available(Stage::Identification)
            && live_stage_available(Stage::UsbRecognition);
    }
    if stage == Stage::ChangeCheck || hardware_gates::is_locked(stage.gate()) {
        return false;
    }
    match stage {
        Stage::UsbRecognition => LIVE_STAGES.contains(&stage),
        Stage::Identification => cfg!(feature = "usb") && LIVE_STAGES.contains(&stage),
        Stage::SettingsRead | Stage::ChangeCheck => false,
    }
}

/// The live gates this binary can run. The banner and both reports use this,
/// so an owner build that opens only G2 does not keep a G1-only sentence.
pub fn open_gates() -> crate::probe_report::OpenGates {
    use crate::probe_report::OpenGates;
    if live_stage_available(Stage::SettingsRead) {
        return OpenGates::G1ThroughG3;
    }
    match (
        live_stage_available(Stage::UsbRecognition),
        live_stage_available(Stage::Identification),
    ) {
        (true, true) => OpenGates::G1AndG2,
        (true, false) => OpenGates::G1,
        (false, true) => OpenGates::G2,
        (false, false) => OpenGates::None,
    }
}

const GATE_GROUPS: &[(&str, &[Gate])] = &[
    ("G1", &[Gate::OsDiscovery]),
    ("G2", &[Gate::IdentityRequest]),
    (
        "G3",
        &[
            Gate::CurrentSettingsRead,
            Gate::DeviceCheckCurrentSettingsRead,
        ],
    ),
    ("G4", &[Gate::AudioCapture]),
    ("G5", &[Gate::FullBackupRead, Gate::DeviceCheckBackup]),
    (
        "G6",
        &[
            Gate::EditBufferWrite,
            Gate::DeviceCheckLiveTest,
            Gate::KeepLiveEdit,
            Gate::DeviceCheckChannel,
        ],
    ),
    ("G7", &[Gate::StoreRestore]),
];

fn gate_summary(is_locked: impl Fn(Gate) -> bool) -> String {
    GATE_GROUPS
        .iter()
        .map(|(name, gates)| {
            let locked = gates.iter().filter(|&&gate| is_locked(gate)).count();
            let status = if locked == 0 {
                "open"
            } else if locked == gates.len() {
                "locked"
            } else {
                "partly open"
            };
            format!("{name} {status}")
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn availability_banner() -> String {
    format!(
        "Compiled hardware gates · {}. Firmware installation is outside the app.",
        gate_summary(hardware_gates::is_locked)
    )
}

/// Describes the numbered procedures this build offers.
pub fn this_computer_finished_message() -> &'static str {
    use crate::probe_report::OpenGates;
    match open_gates() {
        OpenGates::G1 => {
            "Step 1 finished. Steps 2–4 are unavailable in this build. Copy the detailed report into issue #125."
        }
        OpenGates::G1AndG2 => "This build offers steps 1 and 2. Steps 3 and 4 are unavailable.",
        OpenGates::G1ThroughG3 => {
            "Steps 1, 2, and 3 are the ones this build can run. Step 4 is not offered. Copy the detailed report into issue #125."
        }
        OpenGates::G2 | OpenGates::None => "This build cannot run the step 1 preflight.",
    }
}

/// Names the numbered steps in the report, independently of other gates.
pub fn reports_panel_note() -> &'static str {
    use crate::probe_report::OpenGates;
    match open_gates() {
        OpenGates::G1 => {
            "Steps 1–2 report (schema 2). Its identities are the ones this check decoded. This build offers step 1."
        }
        OpenGates::G1AndG2 => "Steps 1–2 report (schema 2). This build offers steps 1 and 2.",
        OpenGates::G1ThroughG3 => {
            "Steps 1–2 report (schema 2). Its identities are the ones this check decoded. This build offers steps 1–3. Step 3 reads the Variax and the PODxt Live. Step 4 is not offered."
        }
        OpenGates::G2 | OpenGates::None => {
            "Steps 1–2 report (schema 2). This build cannot run the step 1 preflight."
        }
    }
}

pub fn this_computer_label() -> &'static str {
    use crate::probe_report::OpenGates;
    if crate::macos_usb_available() {
        match open_gates() {
            OpenGates::G1ThroughG3 => {
                return "This computer (G1–G3) — check the Mac USB attachment, ask for Variax identity, then read the guitar settings and PODxt Live tone.";
            }
            OpenGates::G1AndG2 => {
                return "This computer (G1 and G2) — check the Mac USB attachment and ask for Variax identity.";
            }
            OpenGates::G1 => return "This computer (G1) — check the Mac USB attachment list.",
            OpenGates::G2 | OpenGates::None => {}
        }
    }
    match open_gates() {
        OpenGates::G1AndG2 => {
            "This computer (G1 and G2) — step 1 reads Linux metadata and step 2 asks for Variax identity."
        }
        OpenGates::G1ThroughG3 => {
            "This computer (G1–G3) — step 1 reads what Linux already knows. Step 2 is offered only after that read shows one PODxt Live with the expected drivers. Yes on step 2 asks the Variax its name. Yes on step 3 reads the Variax settings and the PODxt Live tone. Step 4 is not offered."
        }
        OpenGates::G2 => {
            "This computer (G2) — step 1 is not open in this build, so the identity request is not offered."
        }
        OpenGates::G1 => THIS_COMPUTER_LABEL,
        OpenGates::None => "This computer — Device check is unavailable in this build.",
    }
}

pub fn consent_yes_label(mode: ProbeMode, stage: Stage) -> &'static str {
    match (mode, stage) {
        (ProbeMode::ThisComputer, Stage::Identification) => STEP_TWO_YES,
        (_, Stage::Identification) => STEP_TWO_YES_SIMULATION,
        (ProbeMode::ThisComputer, Stage::SettingsRead) => "Yes, read the Variax and the PODxt Live",
        (ProbeMode::ThisComputer, Stage::UsbRecognition) => "Yes, read this computer's USB list",
        (ProbeMode::ThisComputer, Stage::ChangeCheck) | (ProbeMode::Simulation, _) => {
            "Yes, run this step (simulation)"
        }
    }
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
            let live = match stage {
                Stage::UsbRecognition => cfg!(target_os = "linux"),
                Stage::Identification | Stage::SettingsRead => {
                    cfg!(all(target_os = "linux", feature = "usb"))
                }
                Stage::ChangeCheck => false,
            };
            assert_eq!(live_stage_available(stage), live, "{stage:?}");
            // Every gate was released by the owner on 2026-11-23.
            assert!(!hardware_gates::is_locked(stage.gate()), "{stage:?}");
        }
        assert!(Stage::UsbRecognition.action().contains("sends nothing"));
        assert!(Stage::UsbRecognition.risk().starts_with("Low risk"));
        assert_eq!(
            consent_yes_label(ProbeMode::ThisComputer, Stage::Identification),
            STEP_TWO_YES
        );
        assert_eq!(
            consent_yes_label(ProbeMode::Simulation, Stage::Identification),
            STEP_TWO_YES_SIMULATION
        );
        assert!(!STEP_TWO_YES_SIMULATION.contains("send one"));
        assert!(
            Stage::Identification
                .action()
                .contains(APP_REQUEST_VERSUS_LINUX)
        );
        assert!(
            Stage::Identification
                .action()
                .contains("save the steps 1–2 report")
        );
        let note = reports_panel_note();
        let finished = this_computer_finished_message();
        let banner = availability_banner();
        for (name, _) in GATE_GROUPS {
            assert!(banner.contains(&format!("{name} open")), "{banner}");
        }
        assert!(banner.contains("Firmware installation is outside the app."));
        if cfg!(all(target_os = "linux", feature = "usb")) {
            assert!(this_computer_label().contains("G1–G3"));
            assert_eq!(ProbeMode::ThisComputer.label(), "This computer (G1–G3)");
            assert!(finished.starts_with("Steps 1, 2, and 3"));
            assert!(note.contains("steps 1–3"));
            assert!(note.contains("Step 4 is not offered."));
        } else if cfg!(target_os = "linux") {
            assert_eq!(this_computer_label(), THIS_COMPUTER_LABEL);
            assert_eq!(ProbeMode::ThisComputer.label(), "This computer (G1)");
            assert!(finished.starts_with("Step 1 finished."));
            assert!(finished.contains("Steps 2–4 are unavailable"));
            assert!(note.contains("This build offers step 1."));
        } else {
            assert!(note.contains("cannot run the step 1 preflight"));
        }
    }

    #[test]
    fn gate_summary_tracks_closed_and_partly_open_groups() {
        let summary =
            gate_summary(|gate| matches!(gate, Gate::AudioCapture | Gate::DeviceCheckBackup));
        assert_eq!(
            summary,
            "G1 open · G2 open · G3 open · G4 locked · G5 partly open · G6 open · G7 open"
        );
    }

    #[test]
    fn consent_preflight_banner_report_and_yes_label() {
        assert!(!hardware_gates::is_locked(
            hardware_gates::Gate::IdentityRequest
        ));
        assert!(!hardware_gates::is_locked(
            hardware_gates::Gate::OsDiscovery
        ));
        let gates = open_gates();
        assert_eq!(
            gates,
            if cfg!(all(target_os = "linux", feature = "usb")) {
                crate::probe_report::OpenGates::G1ThroughG3
            } else if cfg!(target_os = "linux") {
                crate::probe_report::OpenGates::G1
            } else {
                crate::probe_report::OpenGates::None
            }
        );
        assert_eq!(
            live_stage_available(Stage::Identification),
            matches!(
                gates,
                crate::probe_report::OpenGates::G1ThroughG3 | crate::probe_report::OpenGates::G2
            )
        );
        if matches!(
            gates,
            crate::probe_report::OpenGates::G1ThroughG3 | crate::probe_report::OpenGates::G2
        ) {
            assert!(gates.token().contains("G2"));
        } else {
            assert!(!gates.token().contains("G2"));
        }
        let mut probe = session(Scenario::Connected);
        assert!(!probe.g1_expected_binding());
        assert_eq!(probe.awaiting(), Some(Stage::UsbRecognition));
        probe.answer(Consent::No);
        let report = probe.sanitized_report();
        assert!(report.contains(gates.token()), "{report}");
        if matches!(gates, crate::probe_report::OpenGates::G1ThroughG3) {
            assert!(report.contains("gates-open=G1,G2,G3"), "{report}");
        } else {
            assert!(!report.contains("gates-open=G1,G2,G3"), "{report}");
        }
        assert!(crate::probe_report::allowlisted(&report), "{report}");
        assert_eq!(probe.backend().identity_calls(), 0);
    }

    #[test]
    fn consent_preflight_step_two_needs_fresh_binding() {
        let mut matched = session(Scenario::Connected);
        matched.answer(Consent::Yes);
        assert!(matched.g1_expected_binding());
        assert_eq!(matched.awaiting(), Some(Stage::Identification));
        assert_eq!(matched.backend().identity_calls(), 0);

        let mut wrong = SimulatedRig::new(Scenario::Connected);
        wrong.set_interface_driver(1, Some("snd-usb-audio"));
        let mut probe = ProbeSession::new(wrong);
        probe.answer(Consent::Yes);
        assert!(!probe.g1_expected_binding());
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Stopped);
        assert_eq!(probe.awaiting(), None);
        assert_eq!(probe.backend().calls().identity_requests, 0);
        assert_eq!(probe.backend().identity_calls(), 0);
        let detail = probe.detail(Stage::UsbRecognition).unwrap();
        assert!(detail.contains(STEP_TWO_PREFLIGHT), "{detail}");
        assert!(detail.contains("snd-usb-audio"), "{detail}");

        let mut unbound = SimulatedRig::new(Scenario::Connected);
        unbound.set_interface_driver(0, None);
        let mut probe = ProbeSession::new(unbound);
        probe.answer(Consent::Yes);
        assert!(!probe.g1_expected_binding());
        assert!(probe.finished());
        assert_eq!(probe.backend().identity_calls(), 0);

        let fresh = session(Scenario::Connected);
        assert!(!fresh.g1_expected_binding());
        assert_eq!(fresh.awaiting(), Some(Stage::UsbRecognition));
        assert!(!fresh.identification_answered());
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
            assert_eq!(probe.backend().identity_calls(), 0);
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
        assert_eq!(
            probe.locked(Stage::Identification),
            !live_stage_available(Stage::Identification)
        );
        assert_eq!(
            probe.locked(Stage::SettingsRead),
            !live_stage_available(Stage::SettingsRead)
        );
        assert!(probe.locked(Stage::ChangeCheck));
        assert_eq!(probe.backend().calls(), BackendCalls::default());
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
        // A usb build offers step 2. This test answers No and does not send.
        if live_stage_available(Stage::Identification) {
            assert_eq!(probe.awaiting(), Some(Stage::Identification));
            assert!(probe.g1_expected_binding());
            probe.answer(Consent::No);
            assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        } else {
            assert_eq!(probe.awaiting(), None);
            probe.answer(Consent::Yes);
            assert_eq!(probe.status(Stage::Identification), StageStatus::NotRun);
        }
        assert!(probe.finished());
        assert_eq!(
            probe.backend().calls(),
            BackendCalls {
                os_reads: 1,
                ..Default::default()
            }
        );
        assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
        assert_eq!(
            probe.locked(Stage::SettingsRead),
            !live_stage_available(Stage::SettingsRead)
        );
        assert_eq!(probe.status(Stage::ChangeCheck), StageStatus::NotRun);
        assert!(probe.locked(Stage::ChangeCheck));
        let report = probe.report();
        let lines: Vec<_> = report.lines().collect();
        assert_eq!(
            open_gates().token(),
            if live_stage_available(Stage::Identification) {
                "gates-open=G1,G2,G3"
            } else {
                "gates-open=G1"
            }
        );
        assert_eq!(
            lines[0],
            format!(
                "device-check schema=1 mode=this-computer {} writes=none",
                open_gates().token()
            )
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
        if live_stage_available(Stage::Identification) {
            assert!(
                lines[2].starts_with("stage=2 gate=G2 decision=no status=stopped detail="),
                "{}",
                lines[2]
            );
        } else {
            assert_eq!(
                lines[2],
                "stage=2 gate=G2 decision=none status=locked detail=-"
            );
        }
        assert_eq!(
            lines[3],
            if live_stage_available(Stage::SettingsRead) {
                "stage=3 gate=G3 decision=none status=not-run detail=-"
            } else {
                "stage=3 gate=G3 decision=none status=locked detail=-"
            }
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

    #[test]
    fn attachment_address_reads_only_the_selected_cached_sysfs_entry() {
        let root = fake_tree();
        let dir = root.path().join(SYS_USB_DEVICES).join("8-2.1");
        std::fs::create_dir_all(&dir).unwrap();
        let metadata = LinuxSysfs::with_root(root.path());
        assert_eq!(metadata.usb_device_address("8-2.1"), None);
        std::fs::write(dir.join("devnum"), "24\n").unwrap();
        assert_eq!(metadata.usb_device_address("8-2.1"), Some(24));
        assert_eq!(metadata.usb_device_address("8-3"), None);
        assert_eq!(metadata.usb_device_address("../8-2.1"), None);
        std::fs::write(dir.join("devnum"), "128\n").unwrap();
        assert_eq!(metadata.usb_device_address("8-2.1"), None);
    }

    /// Sysfs on this kernel can name the bound driver with underscores.
    /// `snd_usb_pod` still has to be interface 0, and `snd_usb_variax`
    /// interface 1. The temporary tree is never the real `/sys`.
    #[test]
    #[cfg(unix)]
    fn underscore_driver_names_in_a_fake_sysfs_tree_match_only_the_right_interface() {
        let matched = fake_tree();
        fake_device(
            matched.path(),
            "1-2",
            0x4650,
            &[(0, Some("snd_usb_pod")), (1, Some("snd_usb_variax"))],
        );
        let Recognition::Recognised { device, .. } = recognise(&FakeTree(matched.path().into()))
        else {
            panic!("expected one PODxt Live");
        };
        assert_eq!(device.interfaces[0].driver.as_deref(), Some("snd_usb_pod"));
        assert_eq!(
            device.interfaces[1].driver.as_deref(),
            Some("snd_usb_variax")
        );
        assert!(expected_driver_binding(&device));

        let mixed = fake_tree();
        fake_device(
            mixed.path(),
            "1-2",
            0x4650,
            &[(0, Some("snd-usb-pod")), (1, Some("snd_usb_variax"))],
        );
        let Recognition::Recognised { device, .. } = recognise(&FakeTree(mixed.path().into()))
        else {
            panic!("expected one PODxt Live");
        };
        assert!(expected_driver_binding(&device));

        let swapped = fake_tree();
        fake_device(
            swapped.path(),
            "1-2",
            0x4650,
            &[(0, Some("snd_usb_variax")), (1, Some("snd_usb_pod"))],
        );
        let Recognition::Recognised { device, .. } = recognise(&FakeTree(swapped.path().into()))
        else {
            panic!("expected one PODxt Live");
        };
        assert!(!expected_driver_binding(&device));

        #[cfg(target_os = "linux")]
        {
            let mut probe = host_session(matched.path());
            assert_eq!(probe.mode(), ProbeMode::ThisComputer);
            probe.answer(Consent::Yes);
            assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
            assert!(probe.g1_expected_binding());
            if live_stage_available(Stage::Identification) {
                assert_eq!(probe.awaiting(), Some(Stage::Identification));
                probe.answer(Consent::No);
                assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
            } else {
                assert_eq!(probe.awaiting(), None);
            }
            assert_eq!(probe.backend().calls().identity_requests, 0);
            let report = probe.report();
            assert!(
                report.contains("interface 0 driver snd_usb_pod"),
                "{report}"
            );
            assert!(
                report.contains("interface 1 driver snd_usb_variax"),
                "{report}"
            );
            assert!(report.contains(open_gates().token()), "{report}");
            assert!(!report.contains("SECRET"), "serial number leaked");
            assert!(
                !report.contains(&*matched.path().to_string_lossy()),
                "path leaked"
            );
        }
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
    fn host_rig_refuses_settings_and_does_not_open_usb_from_a_test() {
        let empty = tempfile::tempdir().unwrap();
        let mut rig = HostRig::new(injected_reader(empty.path()));
        assert_eq!(
            rig.can_run(Stage::Identification),
            live_stage_available(Stage::Identification)
        );
        assert_eq!(
            rig.can_run(Stage::SettingsRead),
            live_stage_available(Stage::SettingsRead)
        );
        assert!(!rig.can_run(Stage::ChangeCheck));
        // A test build returns before enumeration. This is not a live check.
        assert!(rig.identify().is_err());
        assert!(rig.read_settings(ReadPass::First).is_err());
        assert_eq!(rig.calls().os_reads, 0);
        assert_eq!(rig.calls().settings_reads, 1);
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
        let sanitized = probe.sanitized_report();
        assert!(crate::probe_report::allowlisted(&sanitized), "{sanitized}");
        assert!(sanitized.contains("bridge=none"));
        assert!(!sanitized.contains("bridge=podxt-live-3.01"));
        assert!(sanitized.contains("guitar=variax-600-3.10"));
        assert!(!sanitized.contains("SIMULATED"));
    }

    struct FailIdentify {
        sysfs: SyntheticSysfs,
        calls: BackendCalls,
        kind: IdentityFailure,
    }

    impl FailIdentify {
        fn new(kind: IdentityFailure) -> Self {
            Self {
                sysfs: synthetic_podxt_live(),
                calls: BackendCalls::default(),
                kind,
            }
        }
    }

    impl ProbeBackend for FailIdentify {
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
        fn identify(&mut self) -> Result<IdentityPair, IdentifyError> {
            self.calls.identity_requests += 1;
            Err(IdentifyError::Failed(IdentifyStop {
                failure: self.kind,
                cleanup: None,
                cleanup_ran: false,
                release_failed: false,
                reattach_failed: false,
                bridge: None,
                guitar: None,
                firmware_notice: None,
            }))
        }
        fn read_settings(&mut self, _: ReadPass) -> Result<PresetMetadata, String> {
            self.calls.settings_reads += 1;
            Err("settings must not run after a failed identity".into())
        }
    }

    #[test]
    fn stage_two_waits_for_its_own_yes_and_each_bad_identity_stops() {
        let mut waiting = ProbeSession::new(FailIdentify::new(IdentityFailure::Unknown));
        assert_eq!(Consent::default(), Consent::No);
        waiting.answer(Consent::Yes);
        assert_eq!(waiting.awaiting(), Some(Stage::Identification));
        assert_eq!(waiting.backend().calls.identity_requests, 0);
        waiting.answer(Consent::No);
        assert_eq!(waiting.backend().calls.identity_requests, 0);
        assert!(waiting.finished());

        for kind in [
            IdentityFailure::Unknown,
            IdentityFailure::Ambiguous,
            IdentityFailure::Inconsistent,
            IdentityFailure::TimedOut,
            IdentityFailure::OpenFailed,
            IdentityFailure::AccessDenied,
            IdentityFailure::SendFailed,
            IdentityFailure::ReadFailed,
        ] {
            let mut probe = ProbeSession::new(FailIdentify::new(kind));
            probe.answer(Consent::Yes);
            probe.answer(Consent::Yes);
            assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
            assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
            assert_eq!(
                probe.stopped(),
                Some((Stage::Identification, StopReason::Failed))
            );
            assert_eq!(probe.backend().calls.identity_requests, 1);
            assert_eq!(probe.backend().calls.settings_reads, 0);
            let report = probe.sanitized_report();
            assert!(report.contains(kind.stop_token()), "{report}");
            assert!(crate::probe_report::allowlisted(&report), "{report}");
            assert!(!report.contains('/'));
        }
    }

    #[test]
    fn saving_the_sanitized_report_does_not_run_another_stage() {
        let mut probe = session(Scenario::Connected);
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        let calls = probe.backend().calls();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SECRET-USER-report.txt");
        probe.save_sanitized_report(&path).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("guitar=variax-600-3.10"));
        assert!(!body.contains("SECRET"));
        assert!(
            probe
                .save_sanitized_report(&path)
                .is_err_and(|error| error.contains("report with that name already exists"))
        );
        assert_eq!(probe.backend().calls(), calls);
        // Detach, claim, alternate setting, one identity request, the reply,
        // the rest of the window, release, then reattach. No settings bytes.
        assert_eq!(probe.backend().identity_calls(), 8);
        assert_eq!(probe.backend().calls().settings_reads, 0);
    }

    fn scripted(script: identity_stage::ScriptedIdentity) -> ProbeSession<SimulatedRig> {
        ProbeSession::new(SimulatedRig::with_identity(Scenario::Connected, script))
    }

    #[test]
    fn device_check_reports_unverified_guitar_firmware_and_model_without_offering_settings() {
        for member in [4, 9] {
            let mut reply = identity_stage::guitar_reply();
            reply[10] = member;
            reply[12..16].copy_from_slice(b"0311");
            let mut probe = scripted(identity_stage::ScriptedIdentity::new([reply]));
            probe.answer(Consent::Yes);
            probe.answer(Consent::Yes);

            assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
            assert!(probe.awaiting().is_none());
            assert_eq!(probe.backend().calls().settings_reads, 0);
            let text = probe.detail(Stage::Identification).unwrap();
            assert!(text.contains("3.11") && text.contains("3.10"));
            assert!(text.contains("open, edit and save files"));
            assert_eq!(text.contains("model could not be verified"), member != 4);
            assert!(!text.to_ascii_lowercase().contains("update"));
        }
    }

    #[test]
    fn stage_two_decline_and_cancel_make_zero_transport_calls() {
        assert_eq!(Consent::default(), Consent::No);
        for consent in [Consent::No, Consent::Cancel] {
            let mut probe = scripted(identity_stage::happy_script());
            probe.answer(Consent::Yes);
            assert_eq!(probe.status(Stage::UsbRecognition), StageStatus::Passed);
            assert_eq!(probe.awaiting(), Some(Stage::Identification));
            assert_eq!(probe.status(Stage::Identification), StageStatus::NotRun);
            assert_eq!(probe.backend().identity_calls(), 0);
            probe.answer(consent);
            assert_eq!(probe.backend().identity_calls(), 0);
            assert_eq!(probe.backend().calls().identity_requests, 0);
            assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
            assert!(probe.sanitized_report().contains("cleanup=not-run"));
            assert!(probe.finished());
        }
    }

    #[test]
    fn stage_two_stops_on_each_bad_identity_from_the_fake_transport() {
        let mut unknown = identity_stage::bridge_reply();
        unknown[8] = 1;
        let cases = [
            (
                identity_stage::ScriptedIdentity::new([unknown]),
                IdentityFailure::Unknown,
            ),
            (
                identity_stage::ScriptedIdentity::several(vec![
                    identity_stage::bridge_reply(),
                    identity_stage::bridge_reply(),
                ]),
                IdentityFailure::Ambiguous,
            ),
            (
                identity_stage::ScriptedIdentity::new([identity_stage::bridge_reply()]),
                IdentityFailure::Inconsistent,
            ),
            (
                identity_stage::ScriptedIdentity::new([identity_stage::bridge_reply()]).silent(),
                IdentityFailure::TimedOut,
            ),
        ];
        for (script, kind) in cases {
            let mut probe = scripted(script);
            probe.answer(Consent::Yes);
            assert_eq!(probe.backend().identity_calls(), 0, "{kind:?}");
            probe.answer(Consent::Yes);
            assert!(probe.backend().identity_calls() > 0, "{kind:?}");
            assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
            assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
            assert_eq!(probe.backend().calls().settings_reads, 0);
            let report = probe.sanitized_report();
            assert!(report.contains(kind.stop_token()), "{kind:?} {report}");
            assert!(report.contains("cleanup=ok"), "{kind:?} {report}");
            assert!(crate::probe_report::allowlisted(&report), "{report}");
        }
    }

    #[test]
    fn an_unexpected_podxt_reply_is_recorded_and_does_not_pass() {
        let mut probe = scripted(identity_stage::ScriptedIdentity::new([
            identity_stage::bridge_reply(),
        ]));
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
        assert_eq!(probe.backend().calls().settings_reads, 0);
        assert_eq!(probe.backend().identity_calls(), 8);
        let report = probe.sanitized_report();
        assert!(report.contains("stop=inconsistent"), "{report}");
        assert!(report.contains("bridge=podxt-live-3.01"), "{report}");
        assert!(report.contains("guitar=none"), "{report}");
        assert!(report.contains("cleanup=ok"), "{report}");
        assert!(
            report
                .lines()
                .any(|line| { line.starts_with("stage=2 ") && line.contains("status=stopped") })
        );
        assert!(
            !report
                .lines()
                .any(|line| { line.starts_with("stage=2 ") && line.contains("status=passed") })
        );
        assert!(crate::probe_report::allowlisted(&report), "{report}");
    }

    #[test]
    fn timeout_and_reattach_failure_are_both_in_the_report() {
        let mut probe = scripted(
            identity_stage::ScriptedIdentity::new([identity_stage::guitar_reply()])
                .silent()
                .fail_reattach(),
        );
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
        let detail = probe.detail(Stage::Identification).unwrap();
        assert!(detail.contains("No identity answer arrived in time"));
        assert!(detail.contains("could not be put back"));
        let report = probe.sanitized_report();
        assert!(report.contains("stop=timed-out"), "{report}");
        assert!(report.contains("cleanup=failed"), "{report}");
        assert!(report.contains("bridge=none"), "{report}");
        assert!(report.contains("guitar=none"), "{report}");
        assert!(report.lines().any(|line| {
            line.starts_with("stage=2 ")
                && line.contains("status=stopped")
                && !line.contains("guitar=variax-600-3.10")
        }));
        assert!(crate::probe_report::allowlisted(&report), "{report}");
        assert!(probe.report().starts_with("device-check schema=1 "));
    }

    #[test]
    fn a_reply_that_arrives_after_the_first_valid_one_does_not_pass() {
        let mut probe = scripted(
            identity_stage::ScriptedIdentity::new([identity_stage::guitar_reply()])
                .arrive_after_first(identity_stage::guitar_reply()),
        );
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        assert_eq!(probe.status(Stage::SettingsRead), StageStatus::NotRun);
        assert_eq!(probe.backend().calls().settings_reads, 0);
        assert_eq!(probe.backend().identity.queued_at_start(), 1);
        let report = probe.sanitized_report();
        assert!(report.contains("stop=ambiguous"), "{report}");
        assert!(report.contains("guitar=variax-600-3.10"), "{report}");
        assert!(report.contains("cleanup=ok"), "{report}");
        assert!(report.lines().any(|line| {
            line.starts_with("stage=2 ")
                && line.contains("status=stopped")
                && line.contains("stop=ambiguous")
        }));
        assert!(crate::probe_report::allowlisted(&report), "{report}");
    }

    #[test]
    fn claim_failure_keeps_the_reattach_error_in_the_report() {
        let mut probe = scripted(identity_stage::happy_script().fail_claim().fail_reattach());
        probe.answer(Consent::Yes);
        probe.answer(Consent::Yes);
        assert_eq!(probe.status(Stage::Identification), StageStatus::Stopped);
        assert_eq!(probe.backend().calls().settings_reads, 0);
        let detail = probe.detail(Stage::Identification).unwrap();
        assert!(detail.contains("could not be claimed"), "{detail}");
        assert!(detail.contains("could not be put back"), "{detail}");
        let report = probe.sanitized_report();
        assert!(report.contains("stop=claim-failed"), "{report}");
        assert!(report.contains("cleanup=failed"), "{report}");
        assert!(report.contains("guitar=none"), "{report}");
        assert!(crate::probe_report::allowlisted(&report), "{report}");
    }
}

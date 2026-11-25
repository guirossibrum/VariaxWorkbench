//! Computer information for steps 0.1 and 0.2 (#250, #251). Read-only.
//!
//! Collectors read through a [`FileSource`] (Linux) or injected command
//! outputs (Windows, macOS), so tests and practice mode use synthetic inputs
//! and never read the real `/proc` or `/sys`.

pub mod linux;
pub mod other;

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Read-only access to a file tree, with paths relative to its root
/// (`etc/os-release`, `sys/bus/usb/devices`, ...).
pub trait FileSource {
    fn read(&self, path: &str) -> Option<String>;
    /// The final component of a symbolic link's target.
    fn link_name(&self, path: &str) -> Option<String>;
    /// Entry names in a directory, sorted.
    fn list(&self, dir: &str) -> Vec<String>;
    /// Permission bits, owner and group IDs.
    fn mode(&self, path: &str) -> Option<(u32, u32, u32)>;
}

/// The real file tree under `root` (`/` in the app). Tests never use `/`.
pub struct RootFs {
    pub root: PathBuf,
}

impl FileSource for RootFs {
    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(path)).ok()
    }

    fn link_name(&self, path: &str) -> Option<String> {
        std::fs::read_link(self.root.join(path))
            .ok()?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.join(dir))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .take(1024)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn mode(&self, path: &str) -> Option<(u32, u32, u32)> {
        let metadata = std::fs::metadata(self.root.join(path)).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some((metadata.mode() & 0o7777, metadata.uid(), metadata.gid()))
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            None
        }
    }
}

/// An in-memory tree for practice mode and tests.
#[derive(Clone, Default)]
pub struct MemFs {
    pub files: BTreeMap<String, String>,
    pub links: BTreeMap<String, String>,
    pub modes: BTreeMap<String, (u32, u32, u32)>,
}

impl MemFs {
    pub fn file(&mut self, path: &str, text: &str) -> &mut Self {
        self.files.insert(path.into(), text.into());
        self
    }

    pub fn link(&mut self, path: &str, target: &str) -> &mut Self {
        self.links.insert(path.into(), target.into());
        self
    }

    pub fn node(&mut self, path: &str, mode: u32, uid: u32, gid: u32) -> &mut Self {
        self.modes.insert(path.into(), (mode, uid, gid));
        self
    }
}

impl FileSource for MemFs {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn link_name(&self, path: &str) -> Option<String> {
        self.links
            .get(path)
            .and_then(|target| target.rsplit('/').next().map(str::to_owned))
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        let mut names: Vec<String> = self
            .files
            .keys()
            .chain(self.links.keys())
            .chain(self.modes.keys())
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('/').next())
            .map(str::to_owned)
            .collect();
        names.sort();
        names.dedup();
        names
    }

    fn mode(&self, path: &str) -> Option<(u32, u32, u32)> {
        self.modes.get(path).copied()
    }
}

/// One `name: value` line in the report's computer section.
pub type Line = (String, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbInterface {
    pub number: u8,
    pub driver: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbDevice {
    /// Bus and port path, such as `1-2.1` (not a serial number).
    pub port: String,
    pub vendor: u16,
    pub product: u16,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    /// Redacted (hashed) before it reaches the report.
    pub serial: Option<String>,
    pub speed: Option<String>,
    pub bus: Option<u16>,
    pub address: Option<u8>,
    pub interfaces: Vec<UsbInterface>,
    /// Device node permissions, such as `crw-rw-r-- root:root (0:0)`.
    pub node: Option<String>,
}

impl UsbDevice {
    pub fn is_podxt_live(&self) -> bool {
        (self.vendor, self.product) == crate::device_probe::PODXT_LIVE_USB
    }

    pub fn driver(&self, interface: u8) -> Option<&str> {
        self.interfaces
            .iter()
            .find(|i| i.number == interface)
            .and_then(|i| i.driver.as_deref())
    }

    pub fn describe(&self) -> String {
        let interfaces: Vec<String> = self
            .interfaces
            .iter()
            .map(|i| {
                format!(
                    "if{}={}",
                    i.number,
                    i.driver.as_deref().unwrap_or("(no driver)")
                )
            })
            .collect();
        format!(
            "{:04x}:{:04x} {} on port {} (bus {}, address {}, speed {} Mbit/s){}{}{}",
            self.vendor,
            self.product,
            self.name.as_deref().unwrap_or("(no name)"),
            self.port,
            self.bus.map_or("?".into(), |b| b.to_string()),
            self.address.map_or("?".into(), |a| a.to_string()),
            self.speed.as_deref().unwrap_or("?"),
            if interfaces.is_empty() {
                String::new()
            } else {
                format!(" [{}]", interfaces.join(", "))
            },
            self.serial.as_deref().map_or(String::new(), |s| format!(
                " serial {}",
                crate::rig_check::redact::Redactor::serial_tag(s)
            )),
            self.node
                .as_deref()
                .map_or(String::new(), |n| format!(" node {n}")),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct UsbView {
    pub devices: Vec<UsbDevice>,
    pub lines: Vec<Line>,
}

impl UsbView {
    pub fn podxt_lives(&self) -> Vec<&UsbDevice> {
        self.devices.iter().filter(|d| d.is_podxt_live()).collect()
    }
}

/// How this copy of Rig Check was built.
pub fn build_lines() -> Vec<Line> {
    let mut features = Vec::new();
    for (name, on) in [
        ("fma", cfg!(target_feature = "fma")),
        ("avx", cfg!(target_feature = "avx")),
        ("avx2", cfg!(target_feature = "avx2")),
        ("sse4.2", cfg!(target_feature = "sse4.2")),
        ("neon", cfg!(target_feature = "neon")),
    ] {
        if on {
            features.push(name);
        }
    }
    vec![
        (
            "Build target".into(),
            option_env!("RIG_CHECK_TARGET").map_or_else(
                || {
                    format!(
                        "{}-{} ({})",
                        std::env::consts::ARCH,
                        std::env::consts::OS,
                        std::env::consts::FAMILY
                    )
                },
                str::to_owned,
            ),
        ),
        (
            "Build CPU features".into(),
            if features.is_empty() {
                "baseline (no fma/avx enabled at build time)".into()
            } else {
                features.join(", ")
            },
        ),
        (
            "Compiler".into(),
            super::record::RUSTC_VERSION
                .unwrap_or("not recorded at build time")
                .into(),
        ),
        ("Maths profile".into(), crate::math::PROFILE.into()),
        (
            "Build profile".into(),
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
            .into(),
        ),
    ]
}

/// libusb's version, if this build links it. Reading the version does not
/// open any device.
pub fn libusb_version() -> String {
    #[cfg(feature = "usb")]
    {
        let v = rusb::version();
        format!("{}.{}.{}.{}", v.major(), v.minor(), v.micro(), v.nano())
    }
    #[cfg(not(feature = "usb"))]
    {
        "not linked (built without the usb feature)".into()
    }
}

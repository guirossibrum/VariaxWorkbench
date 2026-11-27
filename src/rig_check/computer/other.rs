//! Windows and macOS collectors (#251). Read-only.
//!
//! The report accepts synthetic command output and USB observations. The
//! live macOS USB build gets its PODxt Live observations from the same passive
//! IOKit inventory as the transport. Windows collector verification is pending.

use super::Line;

pub const NOT_VERIFIED: &str = "collector not verified on this OS yet (#251)";
pub const NOT_AVAILABLE: &str = "not available on this OS";

/// Runs a read-only OS query. The app uses [`SystemCommands`]; tests pass
/// canned output.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;

    fn env(&self, _name: &str) -> Option<String> {
        None
    }
}

/// The real commands, on Windows and macOS only. Test builds never run them.
pub struct SystemCommands;

impl CommandRunner for SystemCommands {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        if cfg!(test) || !(cfg!(target_os = "windows") || cfg!(target_os = "macos")) {
            return None;
        }
        #[cfg(target_os = "windows")]
        let mut command = {
            use std::os::windows::process::CommandExt;
            use std::path::PathBuf;

            let system = PathBuf::from(std::env::var_os("SystemRoot")?);
            if !system.is_absolute() {
                return None;
            }
            let binary = match program {
                "cmd" => "cmd.exe",
                "pnputil" => "pnputil.exe",
                "powershell" => "WindowsPowerShell/v1.0/powershell.exe",
                _ => return None,
            };
            let mut command = std::process::Command::new(system.join("System32").join(binary));
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            command
        };
        #[cfg(not(target_os = "windows"))]
        let mut command = std::process::Command::new(program);
        let output = command.args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        // An undecodable PnP field cannot establish device or driver absence.
        if program == "pnputil" {
            return String::from_utf8(output.stdout).ok();
        }
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn env(&self, name: &str) -> Option<String> {
        if cfg!(all(target_os = "windows", not(test))) {
            std::env::var(name).ok()
        } else {
            None
        }
    }
}

fn line(name: &str, value: impl Into<String>) -> Line {
    (name.to_string(), value.into())
}

fn or_missing(value: Option<String>) -> String {
    value.unwrap_or_else(|| NOT_AVAILABLE.into())
}

/// `key: value` or `key  : value` lines.
fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().to_owned())
    })
}

// ---------------------------------------------------------------- Windows

fn valid_dotted_version(version: &str, components: std::ops::RangeInclusive<usize>) -> bool {
    let parts = version.split('.');
    components.contains(&parts.clone().count())
        && parts.into_iter().all(|part| {
            part.parse::<u32>().is_ok() && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// `ver` keeps the dotted version in brackets even when the label varies.
fn windows_version(ver: &str) -> Option<&str> {
    let (name, detail) = ver.trim().trim_start_matches('\u{feff}').split_once('[')?;
    if !name.trim().eq_ignore_ascii_case("Microsoft Windows") {
        return None;
    }
    let version = detail.strip_suffix(']')?.split_whitespace().last()?;
    valid_dotted_version(version, 3..=4).then_some(version)
}

fn windows_architecture(commands: &dyn CommandRunner) -> Option<&'static str> {
    ["PROCESSOR_ARCHITEW6432", "PROCESSOR_ARCHITECTURE"]
        .into_iter()
        .filter_map(|name| commands.env(name))
        .find_map(|value| match value.trim().to_ascii_uppercase().as_str() {
            "AMD64" => Some("x86_64"),
            "ARM64" => Some("aarch64"),
            "X86" => Some("x86"),
            "ARM" => Some("arm"),
            _ => None,
        })
}

fn pnp_field<'a>(record: &[&'a str], key: &str) -> Option<&'a str> {
    let mut values = record
        .iter()
        .filter_map(|row| row.split_once(':'))
        .filter(|(name, _)| name.trim().eq_ignore_ascii_case(key))
        .map(|(_, value)| value.trim());
    let value = values.next()?;
    (!value.is_empty() && values.next().is_none()).then_some(value)
}

/// Each recognized English record must have its identifying fields. A failed
/// parse of any record means the inventory cannot establish absence.
fn pnp_inventory(
    output: &str,
    first_field: &str,
    parse_record: impl Fn(&[&str]) -> Option<Option<String>>,
) -> Option<Vec<String>> {
    let mut lines = output
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .skip_while(|row| row.is_empty())
        .peekable();
    lines.next_if(|row| row.eq_ignore_ascii_case("Microsoft PnP Utility"));
    let records = lines.collect::<Vec<_>>();
    let mut records = records
        .split(|row| row.is_empty())
        .filter(|block| !block.is_empty());
    let first_record = records.next()?;
    let mut matches = Vec::new();
    for block in std::iter::once(first_record).chain(records) {
        let (key, _) = block[0].split_once(':')?;
        if !key.trim().eq_ignore_ascii_case(first_field) {
            return None;
        }
        matches.extend(parse_record(block)?);
    }
    Some(matches)
}

fn inf_name(name: &str) -> Option<String> {
    let name = name.to_ascii_lowercase();
    let stem = name.strip_suffix(".inf")?;
    (!stem.is_empty()
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(&byte)))
    .then_some(name)
}

/// Third-party driver store packages attributed to Line 6.
fn windows_line6_drivers(output: &str) -> Option<Vec<String>> {
    pnp_inventory(output, "Published Name", |block| {
        let published = inf_name(pnp_field(block, "Published Name")?)?;
        let original = inf_name(pnp_field(block, "Original Name")?)?;
        let provider = pnp_field(block, "Provider Name")?;
        let normalized_provider = provider.to_ascii_lowercase().replace([' ', ',', '.'], "");
        if !matches!(normalized_provider.as_str(), "line6" | "line6inc") {
            return Some(None);
        }
        let version = pnp_field(block, "Driver Version")
            .and_then(|value| value.split_whitespace().last())
            .filter(|version| valid_dotted_version(version, 4..=4))
            .unwrap_or(NOT_AVAILABLE);
        Some(Some(format!(
            "{original} ({published}) version {version} by {provider}"
        )))
    })
}

/// Connected USB PnP records with a hardware vendor ID of 0E41.
fn windows_line6_devices(output: &str) -> Option<Vec<String>> {
    pnp_inventory(output, "Instance ID", |block| {
        let id = pnp_field(block, "Instance ID")?;
        let (bus, rest) = id.split_once('\\')?;
        let (hardware, instance) = rest.split_once('\\')?;
        if bus.is_empty() || hardware.is_empty() || instance.is_empty() {
            return None;
        }
        if !bus.eq_ignore_ascii_case("USB") {
            return Some(None);
        }
        let hardware = hardware.to_ascii_uppercase();
        let mut parts = hardware.split('&');
        let Some(vendor) = parts.next()?.strip_prefix("VID_") else {
            return Some(None);
        };
        let product = parts.next()?.strip_prefix("PID_")?;
        let is_usb_id = |id: &str| id.len() == 4 && id.bytes().all(|byte| byte.is_ascii_hexdigit());
        if !is_usb_id(vendor) || !is_usb_id(product) {
            return None;
        }
        if vendor != "0E41" {
            return Some(None);
        }
        let name = pnp_field(block, "Device Description").unwrap_or(NOT_AVAILABLE);
        let status = pnp_field(block, "Status").unwrap_or(NOT_AVAILABLE);
        let driver = pnp_field(block, "Driver Name")
            .and_then(inf_name)
            .unwrap_or_else(|| NOT_AVAILABLE.into());
        Some(Some(format!(
            "{name} — 0e41:{} — status {status} — driver {driver}",
            product.to_ascii_lowercase()
        )))
    })
}

pub fn windows_system(commands: &dyn CommandRunner) -> Vec<Line> {
    let ver = commands.run("cmd", &["/c", "ver"]);
    let version = ver.as_deref().and_then(windows_version);
    let build = version.and_then(|value| value.split('.').nth(2));
    let mut lines = vec![
        line("Operating system", "Windows"),
        line("Collector", NOT_VERIFIED),
        line("Windows version", version.unwrap_or(NOT_AVAILABLE)),
        line("Windows build", build.unwrap_or(NOT_AVAILABLE)),
        line(
            "Architecture",
            windows_architecture(commands).unwrap_or(NOT_AVAILABLE),
        ),
    ];
    lines.extend(super::build_lines());
    lines
}

pub fn windows_devices(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = vec![line("Collector", NOT_VERIFIED)];
    for (name, inventory, absent) in [
        (
            "Line 6 driver package",
            commands
                .run("pnputil", &["/enum-drivers"])
                .as_deref()
                .and_then(windows_line6_drivers),
            "none reported in the third-party driver store",
        ),
        (
            "Line 6 USB device",
            commands
                .run("pnputil", &["/enum-devices", "/connected"])
                .as_deref()
                .and_then(windows_line6_devices),
            "none connected",
        ),
    ] {
        match inventory {
            Some(list) if !list.is_empty() => {
                lines.extend(list.into_iter().map(|item| line(name, item)));
            }
            Some(_) => lines.push(line(name, absent)),
            None => lines.push(line(name, NOT_AVAILABLE)),
        }
    }
    lines.push(line("MIDI ports", NOT_AVAILABLE));
    lines.push(line("USB host controller", NOT_AVAILABLE));
    lines
}

pub fn windows(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = windows_system(commands);
    lines.extend(
        windows_devices(commands)
            .into_iter()
            .filter(|(name, _)| name != "Collector"),
    );
    lines
}

// ---------------------------------------------------------------- macOS

pub struct MacUsbDevice {
    pub name: String,
    pub product_id: Option<String>,
    pub vendor_id: Option<String>,
    pub location: Option<String>,
    pub speed: Option<String>,
    pub serial: Option<String>,
}

/// `system_profiler SPUSBDataType` entries whose vendor ID is 0x0e41.
pub fn mac_line6_usb(profiler: &str) -> Vec<MacUsbDevice> {
    let mut devices = Vec::new();
    let mut name = String::new();
    let mut current: Option<MacUsbDevice> = None;
    for raw in profiler.lines() {
        let text = raw.trim();
        if text.ends_with(':') && !text.contains(": ") {
            if let Some(done) = current.take() {
                devices.push(done);
            }
            name = text.trim_end_matches(':').to_owned();
            continue;
        }
        let Some((key, value)) = text.split_once(':') else {
            continue;
        };
        let value = value.trim().to_owned();
        let entry = current.get_or_insert_with(|| MacUsbDevice {
            name: name.clone(),
            product_id: None,
            vendor_id: None,
            location: None,
            speed: None,
            serial: None,
        });
        match key.trim() {
            "Product ID" => entry.product_id = Some(value),
            "Vendor ID" => entry.vendor_id = Some(value),
            "Location ID" => entry.location = Some(value),
            "Speed" => entry.speed = Some(value),
            "Serial Number" => entry.serial = Some(value),
            _ => {}
        }
    }
    devices.extend(current);
    devices
        .into_iter()
        .filter(|d| {
            d.vendor_id
                .as_deref()
                .is_some_and(|v| v.to_ascii_lowercase().contains("0x0e41"))
        })
        .collect()
}

pub fn mac_system(commands: &dyn CommandRunner) -> Vec<Line> {
    let version = commands.run("sw_vers", &[]);
    let get = |key: &str| version.as_deref().and_then(|v| field(v, key));
    let mut lines = vec![
        line("Operating system", "macOS"),
        line("macOS version", or_missing(get("ProductVersion"))),
        line("macOS build", or_missing(get("BuildVersion"))),
        line(
            "Architecture",
            or_missing(commands.run("uname", &["-m"]).map(|t| t.trim().to_owned())),
        ),
        line(
            "CPU model",
            or_missing(
                commands
                    .run("sysctl", &["-n", "machdep.cpu.brand_string"])
                    .map(|t| t.trim().to_owned()),
            ),
        ),
    ];
    lines.extend(super::build_lines());
    lines
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
pub(in crate::rig_check) struct MacUsbAttachment<'a> {
    pub route: Option<&'a str>,
    pub location_id: Option<u32>,
    pub address: Option<u8>,
    pub readiness: &'static str,
}

#[cfg(any(test, all(target_os = "macos", feature = "usb")))]
pub(in crate::rig_check) fn mac_usb_view<'a>(
    attachments: impl IntoIterator<Item = MacUsbAttachment<'a>>,
) -> super::UsbView {
    use super::{UsbDevice, UsbView};

    let mut view = UsbView::default();
    let (vendor, product) = crate::device_probe::PODXT_LIVE_USB;
    for attachment in attachments {
        let location = attachment.location_id.map(|id| format!("0x{id:08x}"));
        let port = attachment.route.map_or_else(
            || {
                location
                    .as_ref()
                    .map_or_else(|| "unavailable".into(), |id| format!("USB location {id}"))
            },
            str::to_owned,
        );
        view.lines.push(line(
            "PODxt Live USB",
            format!(
                "{vendor:04x}:{product:04x}, location ID {}, port {}, address {}. {}",
                location.as_deref().unwrap_or(NOT_AVAILABLE),
                attachment.route.unwrap_or(NOT_AVAILABLE),
                attachment
                    .address
                    .map_or_else(|| NOT_AVAILABLE.into(), |id| id.to_string()),
                attachment.readiness,
            ),
        ));
        view.devices.push(UsbDevice {
            port,
            vendor,
            product,
            name: Some("PODxt Live".into()),
            manufacturer: None,
            serial: None,
            speed: None,
            bus: None,
            address: attachment.address,
            interfaces: Vec::new(),
            node: None,
        });
    }
    if view.devices.is_empty() {
        view.lines.push(line("PODxt Live USB", "none detected"));
    }
    view
}

fn mac_driver_status(commands: &dyn CommandRunner) -> String {
    let kexts = commands
        .run("kextstat", &["-l"])
        .filter(|text| !text.trim().is_empty());
    let extensions = commands
        .run("systemextensionsctl", &["list"])
        .filter(|text| !text.trim().is_empty());
    let matches = kexts
        .iter()
        .chain(&extensions)
        .flat_map(|text| text.lines())
        .filter(|row| row.to_ascii_lowercase().contains("line6"))
        .map(|row| row.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>();
    match (kexts.is_some(), extensions.is_some(), matches.is_empty()) {
        (false, false, _) => NOT_AVAILABLE.into(),
        (true, true, true) => {
            "none reported among loaded kernel or listed system extensions".into()
        }
        (true, true, false) => matches.join("; "),
        (true, false, _) | (false, true, _) => {
            let (checked, unavailable) = if kexts.is_some() {
                ("loaded kernel extensions", "system extensions")
            } else {
                ("listed system extensions", "loaded kernel extensions")
            };
            let result = if matches.is_empty() {
                format!("none reported among {checked}")
            } else {
                matches.join("; ")
            };
            format!("{result}; {unavailable} query unavailable")
        }
    }
}

pub(in crate::rig_check) fn mac_host_devices(
    commands: &dyn CommandRunner,
    direct_usb: bool,
) -> Vec<Line> {
    let mut lines = vec![
        line(
            "Device transport",
            if direct_usb {
                "Direct USB for PODxt Live and its Variax connection. No Line 6 driver required."
            } else {
                "Direct USB unavailable in this build."
            },
        ),
        line("Line 6 driver or extension", mac_driver_status(commands)),
        line(
            "CoreMIDI ports",
            "not available on this OS (passive inventory does not query the MIDI server)",
        ),
        line("USB host controller", NOT_AVAILABLE),
        line("USB interface driver binding", NOT_AVAILABLE),
        line("USB device node permissions", NOT_AVAILABLE),
        line(
            "Device firmware",
            if direct_usb {
                "USB inventory does not read firmware. Step 2.1 reads PODxt Live firmware and step 3.1 reads Variax firmware."
            } else {
                "Not available in this build. Firmware reads require a build with direct USB support."
            },
        ),
        line(
            "PODxt Live USB-component firmware",
            "unavailable (no verified version query)",
        ),
        line("Firmware installation", "unsupported"),
    ];
    if direct_usb {
        lines.push(line("USB link speed", NOT_AVAILABLE));
    }
    lines
}

/// The command-only macOS profile. USB-enabled Rig Check uses passive IOKit
/// observations for the PODxt Live instead of parsing profiler output.
pub fn mac_devices(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = mac_host_devices(commands, false);
    match commands.run("system_profiler", &["SPUSBDataType"]) {
        Some(text) if !text.trim().is_empty() => {
            let devices = mac_line6_usb(&text);
            if devices.is_empty() {
                lines.push(line("Line 6 USB device", "none detected"));
            }
            for d in devices {
                lines.push(line(
                    "Line 6 USB device",
                    format!(
                        "{} product {} location {} speed {}{}",
                        d.name,
                        d.product_id.as_deref().unwrap_or(NOT_AVAILABLE),
                        d.location.as_deref().unwrap_or(NOT_AVAILABLE),
                        d.speed.as_deref().unwrap_or(NOT_AVAILABLE),
                        d.serial.as_deref().map_or(String::new(), |s| format!(
                            " serial {}",
                            crate::rig_check::redact::Redactor::serial_tag(s)
                        )),
                    ),
                ));
            }
        }
        _ => lines.push(line("Line 6 USB device", NOT_AVAILABLE)),
    }
    lines
}

pub fn mac(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = mac_system(commands);
    lines.extend(mac_devices(commands));
    lines
}

/// Command-only report for the OS this build runs on.
pub fn this_os(commands: &dyn CommandRunner) -> Option<Vec<Line>> {
    if cfg!(target_os = "windows") {
        Some(windows(commands))
    } else if cfg!(target_os = "macos") {
        Some(mac(commands))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Canned(BTreeMap<String, String>);

    impl CommandRunner for Canned {
        fn env(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }

        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.0
                .get(&format!("{program} {}", args.join(" ")))
                .cloned()
        }
    }

    fn value<'a>(lines: &'a [Line], name: &str) -> Vec<&'a str> {
        lines
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    const OTHER_DRIVER: &str =
        "Published Name: oem3.inf\nOriginal Name: mouse.inf\nProvider Name: Example";
    const OTHER_DEVICE: &str =
        "Instance ID: USB\\VID_046D&PID_C52B\\6&1\nDevice Description: Mouse\nStatus: Started";

    fn windows_pnp(drivers: &str, devices: &str) -> Vec<Line> {
        windows_devices(&Canned(BTreeMap::from([
            ("pnputil /enum-drivers".into(), drivers.into()),
            ("pnputil /enum-devices /connected".into(), devices.into()),
        ])))
    }

    #[test]
    fn windows_fields_from_synthetic_output() {
        let canned = Canned(BTreeMap::from([
            (
                "cmd /c ver".into(),
                "\r\nMicrosoft Windows [Versión 10.0.19045.3803]\r\n".into(),
            ),
            ("PROCESSOR_ARCHITECTURE".into(), "x86".into()),
            ("PROCESSOR_ARCHITEW6432".into(), " amd64 ".into()),
            (
                "pnputil /enum-drivers".into(),
                format!(
                    "\u{feff}Microsoft PnP Utility\r\n\r\nPublished Name: OEM12.INF\r\nOriginal Name: L6PODXT.INF\r\nProvider Name: Line 6, Inc.\r\nDriver Version: 15/03/2019 4.2.7.7\r\n \r\n{}",
                    OTHER_DRIVER.replace('\n', "\r\n")
                ),
            ),
            (
                "pnputil /enum-devices /connected".into(),
                format!(
                    "Microsoft PnP Utility\n\nInstance ID: usb\\vid_0e41&pid_4650\\private-instance\nDevice Description: PODxt Live\nClass Name: MEDIA\nStatus: Started\nDriver Name: OEM12.INF\n\n{OTHER_DEVICE}"
                ),
            ),
        ]));
        let lines = windows(&canned);
        assert_eq!(value(&lines, "Windows version"), ["10.0.19045.3803"]);
        assert_eq!(value(&lines, "Windows build"), ["19045"]);
        assert_eq!(value(&lines, "Architecture"), ["x86_64"]);
        assert_eq!(
            value(&lines, "Line 6 driver package"),
            ["l6podxt.inf (oem12.inf) version 4.2.7.7 by Line 6, Inc."]
        );
        assert_eq!(
            value(&lines, "Line 6 USB device"),
            ["PODxt Live — 0e41:4650 — status Started — driver oem12.inf"]
        );
        assert_eq!(value(&lines, "Collector"), [NOT_VERIFIED]);
        assert_eq!(value(&lines, "MIDI ports"), [NOT_AVAILABLE]);
        assert_eq!(value(&lines, "USB host controller"), [NOT_AVAILABLE]);
    }

    #[test]
    fn windows_reports_absence_only_after_parsing_inventory() {
        let lines = windows_pnp(
            &format!("{OTHER_DRIVER}\nSigner Name: Line 6"),
            &format!("{OTHER_DEVICE} (VID_0E41)\n\nInstance ID: HID\\VID_0E41&PID_4650\\test"),
        );
        assert_eq!(
            value(&lines, "Line 6 driver package"),
            ["none reported in the third-party driver store"]
        );
        assert_eq!(value(&lines, "Line 6 USB device"), ["none connected"]);

        let lines = windows(&Canned(BTreeMap::new()));
        for name in [
            "Windows version",
            "Windows build",
            "Architecture",
            "Line 6 driver package",
            "Line 6 USB device",
        ] {
            assert_eq!(value(&lines, name), [NOT_AVAILABLE]);
        }
    }

    #[test]
    fn windows_rejects_unsupported_or_incomplete_inventory() {
        for (drivers, devices) in [
            ("", ""),
            ("Microsoft PnP Utility\n", "Microsoft PnP Utility\n"),
            (
                "Usage: pnputil /enum-drivers",
                "Usage: pnputil /enum-devices",
            ),
            (
                "Microsoft PnP Utility\n\nVeröffentlichter Name: oem12.inf\nAnbietername: Line 6",
                "Microsoft PnP Utility\n\nInstanz-ID: USB\\VID_0E41&PID_4650\\test",
            ),
            (
                "Published Name: oem12.inf\nOriginal Name: l6podxt.inf",
                "Instance ID: USB\\VID_0E41&PID_????\\test",
            ),
        ] {
            let lines = windows_pnp(drivers, devices);
            assert_eq!(value(&lines, "Line 6 driver package"), [NOT_AVAILABLE]);
            assert_eq!(value(&lines, "Line 6 USB device"), [NOT_AVAILABLE]);
        }
        let lines = windows_pnp(
            &format!("{OTHER_DRIVER}\n\nPublished Name: oem12.inf"),
            &format!("{OTHER_DEVICE}\n\nInstance ID: unrecognized"),
        );
        assert_eq!(value(&lines, "Line 6 driver package"), [NOT_AVAILABLE]);
        assert_eq!(value(&lines, "Line 6 USB device"), [NOT_AVAILABLE]);
    }

    #[test]
    fn windows_reports_missing_values_within_identified_records() {
        let lines = windows_pnp(
            "Published Name: oem12.inf\nOriginal Name: l6podxt.inf\nProvider Name: Line6\nDriver Version: 03/15/2019",
            "Instance ID: USB\\VID_0E41&PID_4650\\test\nDevice Description: PODxt Live\nStatus:",
        );
        assert_eq!(
            value(&lines, "Line 6 driver package"),
            ["l6podxt.inf (oem12.inf) version not available on this OS by Line6"]
        );
        assert_eq!(
            value(&lines, "Line 6 USB device"),
            [
                "PODxt Live — 0e41:4650 — status not available on this OS — driver not available on this OS"
            ]
        );
    }

    #[test]
    fn windows_requires_identifiable_version_and_architecture() {
        let canned = Canned(BTreeMap::from([
            (
                "cmd /c ver".into(),
                "Microsoft Windows [Version 10.0.26100]".into(),
            ),
            ("PROCESSOR_ARCHITECTURE".into(), "arm64".into()),
        ]));
        let lines = windows_system(&canned);
        assert_eq!(value(&lines, "Windows version"), ["10.0.26100"]);
        assert_eq!(value(&lines, "Windows build"), ["26100"]);
        assert_eq!(value(&lines, "Architecture"), ["aarch64"]);

        let canned = Canned(BTreeMap::from([
            (
                "cmd /c ver".into(),
                "Microsoft Windows [Version 10.0.unknown]".into(),
            ),
            ("PROCESSOR_ARCHITECTURE".into(), "unknown".into()),
        ]));
        let lines = windows_system(&canned);
        for name in ["Windows version", "Windows build", "Architecture"] {
            assert_eq!(value(&lines, name), [NOT_AVAILABLE]);
        }
    }

    #[test]
    fn mac_fields_from_synthetic_output() {
        let canned = Canned(BTreeMap::from([
            ("sw_vers ".into(), "ProductName:\tmacOS\nProductVersion:\t14.6\nBuildVersion:\t23G80\n".into()),
            ("uname -m".into(), "arm64\n".into()),
            ("sysctl -n machdep.cpu.brand_string".into(), "Apple M1\n".into()),
            (
                "system_profiler SPUSBDataType".into(),
                "USB:\n\n    USB 3.1 Bus:\n\n        PODxt Live:\n\n          Product ID: 0x4650\n          Vendor ID: 0x0e41  (Line 6)\n          Serial Number: ABC123456\n          Speed: Up to 12 Mb/s\n          Location ID: 0x01100000 / 2\n\n        Keyboard:\n\n          Product ID: 0x0250\n          Vendor ID: 0x05ac  (Apple Inc.)\n".into(),
            ),
            ("kextstat -l".into(), "  101    0 0xffffff7f 0x1000 com.line6.driver.L6PODxtLive (1.0)\n".into()),
        ]));
        let lines = mac(&canned);
        assert_eq!(value(&lines, "macOS version"), ["14.6"]);
        assert_eq!(value(&lines, "macOS build"), ["23G80"]);
        assert_eq!(value(&lines, "Architecture"), ["arm64"]);
        let usb = value(&lines, "Line 6 USB device");
        assert_eq!(usb.len(), 1);
        assert!(usb[0].starts_with("PODxt Live product 0x4650 location 0x01100000 / 2"));
        assert!(!usb[0].contains("ABC123456"), "serial is hashed");
        assert!(value(&lines, "Line 6 driver or extension")[0].contains("com.line6.driver"));
        assert!(value(&lines, "Collector").is_empty());
        assert_eq!(
            value(&lines, "Device transport"),
            ["Direct USB unavailable in this build."]
        );
        assert!(value(&lines, "Device firmware")[0].contains("require a build with direct USB"));
    }

    #[test]
    fn mac_passive_usb_formats_the_location_separately_from_address_and_route() {
        let view = mac_usb_view([
            MacUsbAttachment {
                route: Some("1-4.3"),
                location_id: Some(0x01430000),
                address: Some(15),
                readiness: "PODxt Live detected. USB interfaces 0 and 1 are published. Identity unverified.",
            },
            MacUsbAttachment {
                route: None,
                location_id: Some(0x01200000),
                address: None,
                readiness: "PODxt Live detected. USB address and route unavailable in this scan.",
            },
            MacUsbAttachment {
                route: None,
                location_id: None,
                address: Some(12),
                readiness: "PODxt Live detected. USB route unavailable in this scan.",
            },
        ]);
        let rows = value(&view.lines, "PODxt Live USB");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            "0e41:4650, location ID 0x01430000, port 1-4.3, address 15. PODxt Live detected. USB interfaces 0 and 1 are published. Identity unverified."
        );
        assert!(rows[1].contains("location ID 0x01200000, port not available on this OS"));
        assert!(rows[1].ends_with(
            "address not available on this OS. PODxt Live detected. USB address and route unavailable in this scan."
        ));
        assert!(rows[2].contains("location ID not available on this OS"));
        assert_eq!(view.podxt_lives().len(), 3);
        assert_eq!(view.devices[0].port, "1-4.3");
        assert_eq!(view.devices[1].port, "USB location 0x01200000");
        assert_eq!(view.devices[2].port, "unavailable");
    }

    #[test]
    fn mac_report_distinguishes_absent_unavailable_and_unsupported() {
        let commands = Canned(BTreeMap::from([
            ("kextstat -l".into(), "com.apple.driver.usb\n".into()),
            ("systemextensionsctl list".into(), "0 extension(s)\n".into()),
            ("system_profiler SPUSBDataType".into(), "USB:\n".into()),
        ]));
        let offline = mac_devices(&commands);
        assert_eq!(value(&offline, "Line 6 USB device"), ["none detected"]);
        assert_eq!(
            value(&offline, "Line 6 driver or extension"),
            ["none reported among loaded kernel or listed system extensions"]
        );
        let view = mac_usb_view([]);
        assert_eq!(value(&view.lines, "PODxt Live USB"), ["none detected"]);
        assert!(view.podxt_lives().is_empty());

        let unavailable = Canned(BTreeMap::new());
        let live = mac_host_devices(&unavailable, true);
        assert!(value(&live, "Device transport")[0].starts_with("Direct USB for PODxt Live"));
        assert!(value(&live, "CoreMIDI ports")[0].contains("does not query the MIDI server"));
        assert_eq!(value(&live, "USB host controller"), [NOT_AVAILABLE]);
        assert_eq!(value(&live, "Line 6 driver or extension"), [NOT_AVAILABLE]);
        assert!(value(&live, "Device firmware")[0].contains("Step 2.1 reads PODxt Live firmware"));
        assert!(value(&live, "Device firmware")[0].contains("step 3.1 reads Variax firmware"));
        assert_eq!(
            value(&live, "PODxt Live USB-component firmware"),
            ["unavailable (no verified version query)"]
        );
        assert_eq!(value(&live, "Firmware installation"), ["unsupported"]);
        let offline = mac(&unavailable);
        assert_eq!(value(&offline, "macOS version"), [NOT_AVAILABLE]);
        assert_eq!(value(&offline, "Line 6 USB device"), [NOT_AVAILABLE]);
    }

    #[test]
    fn mac_driver_query_only_claims_what_was_checked() {
        let commands = Canned(BTreeMap::from([(
            "systemextensionsctl list".into(),
            "0 extension(s)\n".into(),
        )]));
        assert_eq!(
            mac_driver_status(&commands),
            "none reported among listed system extensions; loaded kernel extensions query unavailable"
        );
    }

    #[test]
    fn the_real_runner_never_runs_in_tests() {
        assert!(SystemCommands.run("uname", &[]).is_none());
        assert!(SystemCommands.env("PROCESSOR_ARCHITECTURE").is_none());
        assert_eq!(
            this_os(&SystemCommands).is_some(),
            cfg!(any(target_os = "windows", target_os = "macos"))
        );
    }
}

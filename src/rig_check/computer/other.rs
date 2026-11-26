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
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
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

/// `cmd /c ver`: `Microsoft Windows [Version 10.0.19045.3803]`.
pub fn windows_version(ver: &str) -> Option<String> {
    let start = ver.find("[Version ")? + "[Version ".len();
    let end = ver[start..].find(']')? + start;
    Some(ver[start..end].to_owned())
}

/// Blocks of `pnputil /enum-drivers` mentioning Line 6.
pub fn windows_line6_drivers(pnputil: &str) -> Vec<String> {
    pnputil
        .split("\n\n")
        .chain(pnputil.split("\r\n\r\n"))
        .filter(|block| block.to_ascii_lowercase().contains("line 6"))
        .filter_map(|block| {
            let name = field(block, "Original Name")?;
            Some(format!(
                "{name} version {} by {}",
                field(block, "Driver Version").unwrap_or_else(|| "?".into()),
                field(block, "Provider Name").unwrap_or_else(|| "?".into())
            ))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Blocks of `pnputil /enum-devices /connected` for USB VID 0E41.
pub fn windows_line6_devices(pnputil: &str) -> Vec<String> {
    pnputil
        .replace("\r\n", "\n")
        .split("\n\n")
        .filter(|block| block.to_ascii_uppercase().contains("VID_0E41"))
        .filter_map(|block| {
            let id = field(block, "Instance ID")?;
            Some(format!(
                "{} — {} — status {} — driver {}",
                id,
                field(block, "Device Description").unwrap_or_else(|| "?".into()),
                field(block, "Status").unwrap_or_else(|| "?".into()),
                field(block, "Driver Name").unwrap_or_else(|| "none".into()),
            ))
        })
        .collect()
}

pub fn windows_system(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = vec![
        line("Operating system", "Windows"),
        line("Collector", NOT_VERIFIED),
        line(
            "Windows version",
            or_missing(
                commands
                    .run("cmd", &["/c", "ver"])
                    .and_then(|v| windows_version(&v)),
            ),
        ),
        line(
            "Architecture",
            or_missing(
                std::env::var("PROCESSOR_ARCHITECTURE")
                    .ok()
                    .filter(|_| !cfg!(test)),
            ),
        ),
    ];
    lines.extend(super::build_lines());
    lines
}

pub fn windows_devices(commands: &dyn CommandRunner) -> Vec<Line> {
    let mut lines = vec![line("Collector", NOT_VERIFIED)];
    let drivers = commands
        .run("pnputil", &["/enum-drivers"])
        .map(|text| windows_line6_drivers(&text));
    match drivers {
        Some(list) if !list.is_empty() => {
            for driver in list {
                lines.push(line("Line 6 driver package", driver));
            }
        }
        Some(_) => lines.push(line("Line 6 driver package", "none installed")),
        None => lines.push(line("Line 6 driver package", NOT_AVAILABLE)),
    }
    match commands
        .run("pnputil", &["/enum-devices", "/connected"])
        .map(|text| windows_line6_devices(&text))
    {
        Some(list) if !list.is_empty() => {
            for device in list {
                lines.push(line("Line 6 USB device", device));
            }
        }
        Some(_) => lines.push(line("Line 6 USB device", "none connected")),
        None => lines.push(line("Line 6 USB device", NOT_AVAILABLE)),
    }
    lines.push(line("MIDI ports", crate::WINDOWS_CONNECTION_UNAVAILABLE));
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
    pub midi_interfaces_present: bool,
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
                "{vendor:04x}:{product:04x}, location ID {}, port {}, address {}, MIDI interface services {}",
                location.as_deref().unwrap_or(NOT_AVAILABLE),
                attachment.route.unwrap_or(NOT_AVAILABLE),
                attachment.address.map_or_else(|| NOT_AVAILABLE.into(), |id| id.to_string()),
                if attachment.midi_interfaces_present {
                    "published"
                } else {
                    "not both published"
                },
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

    #[test]
    fn windows_fields_from_synthetic_output() {
        let canned = Canned(BTreeMap::from([
            ("cmd /c ver".into(), "\r\nMicrosoft Windows [Version 10.0.19045.3803]\r\n".into()),
            (
                "pnputil /enum-drivers".into(),
                "Published Name:     oem12.inf\nOriginal Name:      line6usb.inf\nProvider Name:      Line 6\nClass Name:         Sound, video and game controllers\nDriver Version:     01/01/2014 7.6.0.0\n\nPublished Name:     oem3.inf\nOriginal Name:      other.inf\nProvider Name:      Someone\n".into(),
            ),
            (
                "pnputil /enum-devices /connected".into(),
                "Instance ID:                USB\\VID_0E41&PID_4650\\5&2A\nDevice Description:         PODxt Live\nClass Name:                 MEDIA\nStatus:                     Started\nDriver Name:                oem12.inf\n\nInstance ID:                USB\\VID_046D&PID_C52B\\6&1\nDevice Description:         Mouse\nStatus:                     Started\n".into(),
            ),
        ]));
        let lines = windows(&canned);
        assert_eq!(value(&lines, "Windows version"), ["10.0.19045.3803"]);
        assert_eq!(
            value(&lines, "Line 6 driver package"),
            ["line6usb.inf version 01/01/2014 7.6.0.0 by Line 6"]
        );
        let devices = value(&lines, "Line 6 USB device");
        assert_eq!(devices.len(), 1);
        assert!(devices[0].contains("PODxt Live") && devices[0].contains("Started"));
        assert_eq!(value(&lines, "Collector"), [NOT_VERIFIED]);
        assert_eq!(value(&lines, "USB host controller"), [NOT_AVAILABLE]);
        let empty = windows(&Canned(BTreeMap::new()));
        assert_eq!(value(&empty, "Windows version"), [NOT_AVAILABLE]);
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
                midi_interfaces_present: true,
            },
            MacUsbAttachment {
                route: None,
                location_id: Some(0x01200000),
                address: None,
                midi_interfaces_present: false,
            },
            MacUsbAttachment {
                route: None,
                location_id: None,
                address: Some(12),
                midi_interfaces_present: false,
            },
        ]);
        let rows = value(&view.lines, "PODxt Live USB");
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            "0e41:4650, location ID 0x01430000, port 1-4.3, address 15, MIDI interface services published"
        );
        assert!(rows[1].contains("location ID 0x01200000, port not available on this OS"));
        assert!(rows[1].ends_with(
            "address not available on this OS, MIDI interface services not both published"
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
        assert_eq!(
            this_os(&SystemCommands).is_some(),
            cfg!(any(target_os = "windows", target_os = "macos"))
        );
    }
}

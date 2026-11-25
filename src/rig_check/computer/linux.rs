//! Linux collectors (#250): `/etc/os-release`, `/proc` and `/sys` through a
//! [`FileSource`]. The app passes the real root; tests and practice mode
//! pass a synthetic tree.

use super::{FileSource, Line, MemFs, UsbDevice, UsbInterface, UsbView};
use crate::device_probe::{EXPECTED_POD_DRIVER, EXPECTED_VARIAX_DRIVER, parse_sound_cards};

const USB: &str = "sys/bus/usb/devices";
/// Runtime CPU features worth reporting for the maths profile.
const CPU_FEATURES: [&str; 8] = [
    "fma", "avx", "avx2", "sse4_2", "f16c", "asimd", "fphp", "neon",
];

fn os_release(source: &dyn FileSource) -> Vec<Line> {
    let Some(text) = source
        .read("etc/os-release")
        .or_else(|| source.read("usr/lib/os-release"))
    else {
        return vec![("Distribution".into(), "unknown (no os-release)".into())];
    };
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .map(|value| value.trim().trim_matches('"').to_owned())
    };
    vec![
        (
            "Distribution".into(),
            field("PRETTY_NAME")
                .or_else(|| field("NAME"))
                .unwrap_or_else(|| "unknown".into()),
        ),
        (
            "Distribution version".into(),
            field("VERSION_ID")
                .or_else(|| field("BUILD_ID"))
                .unwrap_or_else(|| "rolling or unknown".into()),
        ),
    ]
}

/// Step 0.1: operating system, kernel, processor and build.
pub fn system(source: &dyn FileSource) -> Vec<Line> {
    let mut lines = vec![("Operating system".to_string(), "Linux".to_string())];
    lines.extend(os_release(source));
    lines.push((
        "Kernel".into(),
        source
            .read("proc/sys/kernel/osrelease")
            .map_or("unknown".into(), |t| t.trim().to_owned()),
    ));
    lines.push((
        "Architecture".into(),
        source.read("proc/sys/kernel/arch").map_or_else(
            || std::env::consts::ARCH.to_string(),
            |t| t.trim().to_owned(),
        ),
    ));
    let cpuinfo = source.read("proc/cpuinfo").unwrap_or_default();
    let value = |keys: &[&str]| {
        cpuinfo.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            keys.contains(&key.trim()).then(|| value.trim().to_owned())
        })
    };
    lines.push((
        "CPU model".into(),
        value(&["model name", "Model", "Hardware", "cpu model"])
            .unwrap_or_else(|| "unknown".into()),
    ));
    let flags = value(&["flags", "Features"]).unwrap_or_default();
    let present: Vec<&str> = CPU_FEATURES
        .iter()
        .copied()
        .filter(|f| flags.split_whitespace().any(|x| x == *f))
        .collect();
    lines.push((
        "CPU features (runtime)".into(),
        if present.is_empty() {
            "none of fma/avx/avx2/sse4_2/f16c/asimd".into()
        } else {
            present.join(", ")
        },
    ));
    lines.push((
        "CPU count".into(),
        cpuinfo
            .lines()
            .filter(|line| line.starts_with("processor"))
            .count()
            .to_string(),
    ));
    lines.extend(super::build_lines());
    lines
}

fn hex16(text: Option<String>) -> Option<u16> {
    u16::from_str_radix(text?.trim(), 16).ok()
}

fn text(source: &dyn FileSource, path: &str) -> Option<String> {
    source
        .read(path)
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

fn permissions(mode: u32) -> String {
    let bits = ['r', 'w', 'x'];
    let mut out = String::from("c");
    for shift in [6, 3, 0] {
        for (i, c) in bits.iter().enumerate() {
            out.push(if (mode >> (shift + 2 - i as u32)) & 1 == 1 {
                *c
            } else {
                '-'
            });
        }
    }
    out
}

/// Step 0.2: USB devices, drivers per interface, nodes and permissions.
pub fn usb(source: &dyn FileSource, libusb: &str) -> UsbView {
    let mut devices = Vec::new();
    let entries = source.list(USB);
    for name in entries
        .iter()
        .filter(|n| !n.contains(':') && !n.starts_with("usb"))
    {
        let base = format!("{USB}/{name}");
        let (Some(vendor), Some(product)) = (
            hex16(source.read(&format!("{base}/idVendor"))),
            hex16(source.read(&format!("{base}/idProduct"))),
        ) else {
            continue;
        };
        let bus = text(source, &format!("{base}/busnum")).and_then(|t| t.parse().ok());
        let address = text(source, &format!("{base}/devnum")).and_then(|t| t.parse().ok());
        let interfaces = entries
            .iter()
            .filter(|n| n.starts_with(&format!("{name}:")))
            .filter_map(|interface| {
                let path = format!("{USB}/{interface}");
                let number = hex16(source.read(&format!("{path}/bInterfaceNumber")))? as u8;
                Some(UsbInterface {
                    number,
                    driver: source.link_name(&format!("{path}/driver")),
                })
            })
            .collect();
        let node = match (bus, address) {
            (Some(bus), Some(address)) => {
                let path = format!("dev/bus/usb/{bus:03}/{address:03}");
                source.mode(&path).map(|(mode, uid, gid)| {
                    format!("/{path} {} uid {uid} gid {gid}", permissions(mode))
                })
            }
            _ => None,
        };
        devices.push(UsbDevice {
            port: name.clone(),
            vendor,
            product,
            name: text(source, &format!("{base}/product")),
            manufacturer: text(source, &format!("{base}/manufacturer")),
            serial: text(source, &format!("{base}/serial")),
            speed: text(source, &format!("{base}/speed")),
            bus,
            address,
            interfaces,
            node,
        });
    }
    let mut lines: Vec<Line> = vec![("libusb".into(), libusb.into())];
    lines.push(("USB devices".into(), devices.len().to_string()));
    for device in &devices {
        lines.push((format!("USB {}", device.port), device.describe()));
    }
    for device in devices.iter().filter(|d| d.is_podxt_live()) {
        for (interface, expected) in [(0, EXPECTED_POD_DRIVER), (1, EXPECTED_VARIAX_DRIVER)] {
            let bound = device.driver(interface).unwrap_or("(no driver)");
            let ok = bound.replace('_', "-") == expected;
            lines.push((
                format!("PODxt Live interface {interface} driver"),
                format!(
                    "{bound}{}",
                    if ok {
                        " (expected)".to_string()
                    } else {
                        format!(" (expected {expected})")
                    }
                ),
            ));
        }
    }
    if let Some(cards) = source.read("proc/asound/cards") {
        match parse_sound_cards(&cards) {
            Ok(cards) => {
                for card in cards {
                    lines.push((
                        format!("Sound card {}", card.index),
                        format!("{} ({})", card.id, card.name),
                    ));
                }
            }
            Err(error) => lines.push(("Sound cards".into(), format!("unreadable: {error}"))),
        }
    }
    for node in source
        .list("dev/snd")
        .iter()
        .filter(|n| n.starts_with("midi"))
    {
        if let Some((mode, uid, gid)) = source.mode(&format!("dev/snd/{node}")) {
            lines.push((
                format!("MIDI node /dev/snd/{node}"),
                format!("{} uid {uid} gid {gid}", permissions(mode)),
            ));
        }
    }
    let rules: Vec<String> = [
        "etc/udev/rules.d",
        "usr/lib/udev/rules.d",
        "lib/udev/rules.d",
    ]
    .iter()
    .flat_map(|dir| {
        source
            .list(dir)
            .into_iter()
            .filter(|name| {
                let lower = name.to_ascii_lowercase();
                lower.contains("line6") || lower.contains("variax") || lower.contains("pod")
            })
            .map(move |name| format!("/{dir}/{name}"))
    })
    .collect();
    lines.push((
        "udev rules mentioning Line 6".into(),
        if rules.is_empty() {
            "none found".into()
        } else {
            rules.join(", ")
        },
    ));
    UsbView { devices, lines }
}

/// A synthetic Linux machine with a PODxt Live, for practice mode and tests.
pub fn synthetic_machine(podxt_present: bool, variax_driver_bound: bool) -> MemFs {
    let mut fs = MemFs::default();
    fs.file(
        "etc/os-release",
        "NAME=\"Practice Linux\"\nPRETTY_NAME=\"Practice Linux (simulated)\"\nVERSION_ID=\"1.0\"\n",
    )
    .file("proc/sys/kernel/osrelease", "6.0.0-practice\n")
    .file("proc/sys/kernel/arch", "x86_64\n")
    .file(
        "proc/cpuinfo",
        "processor\t: 0\nmodel name\t: Practice CPU (simulated)\nflags\t\t: fpu sse4_2 avx avx2 fma\n\nprocessor\t: 1\nmodel name\t: Practice CPU (simulated)\nflags\t\t: fpu sse4_2 avx avx2 fma\n",
    )
    .file(
        "proc/asound/cards",
        " 0 [PCH            ]: HDA-Intel - HDA Intel PCH\n                      HDA Intel PCH at 0xf7f10000 irq 32\n",
    );
    let hub = format!("{USB}/1-1");
    fs.file(&format!("{hub}/idVendor"), "1d6b\n")
        .file(&format!("{hub}/idProduct"), "0002\n")
        .file(&format!("{hub}/product"), "Practice hub\n")
        .file(&format!("{hub}/speed"), "480\n")
        .file(&format!("{hub}/busnum"), "1\n")
        .file(&format!("{hub}/devnum"), "2\n");
    if podxt_present {
        let base = format!("{USB}/1-2");
        fs.file(&format!("{base}/idVendor"), "0e41\n")
            .file(&format!("{base}/idProduct"), "4650\n")
            .file(&format!("{base}/product"), "PODxt Live\n")
            .file(&format!("{base}/manufacturer"), "Line 6\n")
            .file(&format!("{base}/serial"), "PRACTICE0001\n")
            .file(&format!("{base}/speed"), "12\n")
            .file(&format!("{base}/busnum"), "1\n")
            .file(&format!("{base}/devnum"), "5\n")
            .node("dev/bus/usb/001/005", 0o664, 0, 0)
            .node("dev/snd/midiC1D0", 0o660, 0, 92);
        for (n, driver) in [(0, Some("snd-usb-pod")), (1, Some("snd-usb-variax"))] {
            let path = format!("{USB}/1-2:1.{n}");
            fs.file(&format!("{path}/bInterfaceNumber"), &format!("{n:02x}\n"));
            let driver = if n == 1 && !variax_driver_bound {
                None
            } else {
                driver
            };
            if let Some(driver) = driver {
                fs.link(
                    &format!("{path}/driver"),
                    &format!("../../../../../../bus/usb/drivers/{driver}"),
                );
            }
        }
        let cards = fs.files["proc/asound/cards"].clone()
            + " 1 [PODxtLive      ]: PODxtLive - PODxt Live\n                      Line 6 PODxt Live at USB 1:5\n";
        fs.file("proc/asound/cards", &cards);
    }
    fs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value<'a>(lines: &'a [Line], name: &str) -> &'a str {
        &lines.iter().find(|(n, _)| n == name).unwrap().1
    }

    #[test]
    fn system_lines_come_from_the_injected_tree() {
        let mut fs = synthetic_machine(true, true);
        fs.file(
            "etc/os-release",
            "NAME=Arch\nPRETTY_NAME=\"Arch Linux\"\nBUILD_ID=rolling\n",
        );
        let lines = system(&fs);
        assert_eq!(value(&lines, "Distribution"), "Arch Linux");
        assert_eq!(value(&lines, "Distribution version"), "rolling");
        assert_eq!(value(&lines, "Kernel"), "6.0.0-practice");
        assert_eq!(value(&lines, "CPU model"), "Practice CPU (simulated)");
        assert_eq!(
            value(&lines, "CPU features (runtime)"),
            "fma, avx, avx2, sse4_2"
        );
        assert_eq!(value(&lines, "CPU count"), "2");
        assert_eq!(value(&lines, "Maths profile"), crate::math::PROFILE);
        let mut arm = MemFs::default();
        arm.file(
            "proc/cpuinfo",
            "processor : 0\nFeatures : fp asimd evtstrm\nModel : Raspberry Pi 4\n",
        );
        let lines = system(&arm);
        assert_eq!(value(&lines, "CPU model"), "Raspberry Pi 4");
        assert_eq!(value(&lines, "CPU features (runtime)"), "asimd");
        assert_eq!(value(&lines, "Distribution"), "unknown (no os-release)");
    }

    #[test]
    fn usb_devices_drivers_and_permissions() {
        let fs = synthetic_machine(true, true);
        let view = usb(&fs, "1.0.27.11882");
        let pods = view.podxt_lives();
        assert_eq!(pods.len(), 1);
        let pod = pods[0];
        assert_eq!(pod.port, "1-2");
        assert_eq!((pod.bus, pod.address), (Some(1), Some(5)));
        assert_eq!(pod.speed.as_deref(), Some("12"));
        assert_eq!(pod.driver(0), Some("snd-usb-pod"));
        assert_eq!(pod.driver(1), Some("snd-usb-variax"));
        assert_eq!(
            pod.node.as_deref(),
            Some("/dev/bus/usb/001/005 crw-rw-r-- uid 0 gid 0")
        );
        assert_eq!(
            value(&view.lines, "PODxt Live interface 1 driver"),
            "snd-usb-variax (expected)"
        );
        assert_eq!(value(&view.lines, "Sound card 1"), "PODxtLive (PODxt Live)");
        assert_eq!(
            value(&view.lines, "MIDI node /dev/snd/midiC1D0"),
            "crw-rw---- uid 0 gid 92"
        );
        assert_eq!(value(&view.lines, "libusb"), "1.0.27.11882");
        let unbound = usb(&synthetic_machine(true, false), "x");
        assert_eq!(
            value(&unbound.lines, "PODxt Live interface 1 driver"),
            "(no driver) (expected snd-usb-variax)"
        );
        assert!(
            usb(&synthetic_machine(false, true), "x")
                .podxt_lives()
                .is_empty()
        );
    }
}

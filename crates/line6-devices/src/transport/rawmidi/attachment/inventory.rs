//! Kernel metadata that relates a USB attachment to one ALSA raw MIDI node.

use crate::family::podxt::transport::PODXT_LIVE_MIDI;
use std::{
    fs::{self, Metadata},
    io,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

const PODXT_LIVE: (u16, u16) = (0x0e41, 0x4650);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct AttachmentIdentity {
    pub(super) device: PathBuf,
    pub(super) node: NodeIdentity,
    pub(super) usb: Inode,
    pub(super) interface: Inode,
    pub(super) card: Inode,
    pub(super) midi: Inode,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Inode(pub(super) u64, pub(super) u64);

impl Inode {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self(metadata.dev(), metadata.ino())
    }

    fn read(path: &Path) -> Result<Self, String> {
        metadata(path).map(|metadata| Self::from_metadata(&metadata))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NodeIdentity {
    pub(super) inode: Inode,
    pub(super) rdev: u64,
}

impl NodeIdentity {
    pub(super) fn from_metadata(metadata: &Metadata) -> io::Result<Self> {
        if !metadata.file_type().is_char_device() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected a character device",
            ));
        }
        Ok(Self {
            inode: Inode::from_metadata(metadata),
            rdev: metadata.rdev(),
        })
    }
}

pub(super) fn probe(usb_path: &str, address: u8) -> Result<AttachmentIdentity, String> {
    if cfg!(test) {
        return Err("Test builds do not inspect Linux MIDI attachments.".into());
    }
    probe_with(Path::new("/sys"), usb_path, address, |node| {
        NodeIdentity::from_metadata(&fs::symlink_metadata(node)?)
    })
}

pub(super) fn probe_with(
    sys: &Path,
    usb_path: &str,
    address: u8,
    node_identity: impl FnOnce(&Path) -> io::Result<NodeIdentity>,
) -> Result<AttachmentIdentity, String> {
    // Match discovery's Linux bus/port and enumeration-address policy.
    let port_number = |part: &str| part.parse::<u8>().is_ok_and(|n| n != 0);
    if !(1..=127).contains(&address)
        || !usb_path
            .split_once('-')
            .is_some_and(|(bus, ports)| port_number(bus) && ports.split('.').all(port_number))
    {
        return Err("invalid PODxt Live USB selection".into());
    }

    let usb_devices = sys.join("bus/usb/devices");
    let usb = unique_podxt(&usb_devices)?;
    let usb = canonical(&usb)?;
    let usb_inode = Inode::read(&usb)?;
    if usb.file_name().and_then(|name| name.to_str()) != Some(usb_path)
        || read_attr(&usb, "devnum")?.parse::<u8>().ok() != Some(address)
    {
        return Err(format!(
            "selected PODxt Live USB {usb_path} address {address} changed"
        ));
    }

    let interface = canonical(&usb_devices.join(format!("{usb_path}:1.0")))?;
    let driver = interface.join("driver");
    let driver_name =
        fs::read_link(&driver).map_err(|error| format!("{}: {error}", driver.display()))?;
    if interface.parent() != Some(&usb)
        || !matches!(
            driver_name.file_name().and_then(|name| name.to_str()),
            Some("snd-usb-pod" | "snd_usb_pod")
        )
    {
        return Err("selected PODxt Live has no expected interface 0 driver".into());
    }

    let sound = sys.join("class/sound");
    let card_index = unique_card(&sound)?;
    let card = canonical(&sound.join(format!("card{card_index}")))?;
    let device = PODXT_LIVE_MIDI.device_node(card_index);
    let midi = canonical(&sound.join(device.file_name().expect("MIDI node basename")))?;
    if !card.starts_with(&interface)
        || read_attr(&card, "id")? != PODXT_LIVE_MIDI.card_id
        || midi.parent() != Some(&card)
    {
        return Err("the ALSA MIDI node is not beneath the selected PODxt Live interface 0".into());
    }
    let dev = read_attr(&midi, "dev")?;
    let expected = dev
        .split_once(':')
        .and_then(|(major, minor)| Some(libc::makedev(major.parse().ok()?, minor.parse().ok()?)))
        .ok_or_else(|| format!("{} has an invalid device number: {dev}", midi.display()))?;
    let node = node_identity(&device).map_err(|error| format!("{}: {error}", device.display()))?;
    if node.rdev != expected {
        return Err(format!(
            "{} does not have the ALSA MIDI device number {dev}",
            device.display()
        ));
    }

    let identity = AttachmentIdentity {
        device,
        node,
        usb: usb_inode,
        interface: Inode::read(&interface)?,
        card: Inode::read(&card)?,
        midi: Inode::read(&midi)?,
    };
    if Inode::read(&usb)? != identity.usb {
        return Err("PODxt Live USB attachment changed while reading its MIDI node".into());
    }
    Ok(identity)
}

fn unique_podxt(devices: &Path) -> Result<PathBuf, String> {
    let mut found = None;
    for entry in entries(devices)? {
        let entry = entry.map_err(|error| format!("{}: {error}", devices.display()))?;
        if entry.file_name().as_encoded_bytes().contains(&b':') {
            continue;
        }
        let device = entry.path();
        let ids = [
            read_attr(&device, "idVendor")?,
            read_attr(&device, "idProduct")?,
        ];
        let [vendor, product] = ids.map(|id| u16::from_str_radix(&id, 16).ok());
        match (vendor, product) {
            (Some(vendor), Some(product)) if (vendor, product) == PODXT_LIVE => {
                if found.replace(device).is_some() {
                    return Err("multiple PODxt Live USB attachments".into());
                }
            }
            (Some(_), Some(_)) => {}
            _ => return Err(format!("{} has an invalid USB identity", device.display())),
        }
    }
    found.ok_or_else(|| "no PODxt Live USB attachment".into())
}

fn unique_card(sound: &Path) -> Result<u32, String> {
    let mut found = None;
    for entry in entries(sound)? {
        let entry = entry.map_err(|error| format!("{}: {error}", sound.display()))?;
        let name = entry.file_name();
        let Some(index) = name
            .to_str()
            .and_then(|name| name.strip_prefix("card"))
            .and_then(|index| index.parse().ok())
        else {
            continue;
        };
        if read_attr(&entry.path(), "id")? == PODXT_LIVE_MIDI.card_id
            && found.replace(index).is_some()
        {
            return Err("multiple PODxt Live ALSA MIDI cards".into());
        }
    }
    found.ok_or_else(|| "no PODxt Live ALSA MIDI card".into())
}

fn read_attr(path: &Path, name: &str) -> Result<String, String> {
    let path = path.join(name);
    fs::read_to_string(&path)
        .map(|text| text.trim().to_owned())
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn entries(path: &Path) -> Result<fs::ReadDir, String> {
    fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn metadata(path: &Path) -> Result<Metadata, String> {
    fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
}

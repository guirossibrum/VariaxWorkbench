//! Linux ALSA rawmidi link: a device's own MIDI port as its kernel driver
//! exposes it. No driver is detached.

use crate::session::{Link, Path};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// ALSA card id of the PODxt Live (`snd_usb_pod`).
pub const PODXT_LIVE_CARD: &str = "PODxtLive";

pub struct RawMidiLink {
    port: File,
    opened: Instant,
    path: Path,
    pub device: PathBuf,
}

/// `/dev/snd/midiC<n>D0` for the ALSA card whose id is `card_id`.
pub fn find_card(card_id: &str) -> Result<PathBuf, String> {
    let cards = std::fs::read_to_string("/proc/asound/cards")
        .map_err(|e| format!("/proc/asound/cards: {e}"))?;
    let mut cards = cards.lines().filter_map(|line| {
        let (number, rest) = line.split_once('[')?;
        let (id, _) = rest.split_once(']')?;
        (id.trim() == card_id)
            .then(|| number.trim().parse::<u32>().ok())
            .flatten()
    });
    let card = cards
        .next()
        .ok_or_else(|| format!("ALSA card {card_id} not found in /proc/asound/cards."))?;
    if cards.next().is_some() {
        return Err(format!("Several ALSA cards match {card_id}."));
    }
    let device = PathBuf::from(format!("/dev/snd/midiC{card}D0"));
    if device.exists() {
        Ok(device)
    } else {
        Err(format!("{} does not exist.", device.display()))
    }
}

impl RawMidiLink {
    pub fn open(card_id: &str, path: Path) -> Result<Self, String> {
        let device = find_card(card_id)?;
        let port = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&device)
            .map_err(|e| format!("{}: {e}", device.display()))?;
        Ok(Self {
            port,
            opened: Instant::now(),
            path,
            device,
        })
    }
}

impl Link for RawMidiLink {
    fn path(&self) -> Path {
        self.path
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.port
            .write_all(bytes)
            .map_err(|e| format!("MIDI write: {e}"))
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let deadline = Instant::now() + wait;
        let mut buffer = [0; 512];
        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => return Err("MIDI port closed.".into()),
                Ok(length) => return Ok(Some(buffer[..length].to_vec())),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(format!("MIDI read: {error}")),
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }

    fn elapsed(&self) -> Duration {
        self.opened.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

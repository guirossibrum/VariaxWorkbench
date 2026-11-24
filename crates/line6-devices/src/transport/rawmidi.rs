//! Linux ALSA rawmidi link: a device's own MIDI port as its kernel driver
//! exposes it. No driver is detached.

use crate::session::{Link, Path};
use rustix::fs::OFlags;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::io::{Read, Write};
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
    let marker = format!("[{card_id}");
    let card = cards
        .lines()
        .filter(|line| line.contains(&marker))
        .find_map(|line| line.split_whitespace().next()?.parse::<u32>().ok())
        .ok_or_else(|| format!("ALSA card {card_id} not found in /proc/asound/cards."))?;
    let device = PathBuf::from(format!("/dev/snd/midiC{card}D0"));
    if device.exists() {
        Ok(device)
    } else {
        Err(format!("{} does not exist.", device.display()))
    }
}

impl RawMidiLink {
    pub fn open(card_id: &str, path: Path) -> Result<Self, String> {
        Self::open_device(find_card(card_id)?, path)
    }

    pub fn open_device(device: PathBuf, path: Path) -> Result<Self, String> {
        let port = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(OFlags::NONBLOCK.bits() as i32)
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
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut remaining = bytes;
        while !remaining.is_empty() {
            match self.port.write(remaining) {
                Ok(0) => return Err("MIDI port closed during write.".into()),
                Ok(length) => remaining = &remaining[length..],
                Err(error) if retryable(&error) => {
                    if !pause_until(deadline) {
                        return Err("MIDI write timed out.".into());
                    }
                }
                Err(error) => return Err(format!("MIDI write: {error}")),
            }
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let deadline = Instant::now() + wait;
        let mut buffer = [0; 512];
        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => return Err("MIDI port closed.".into()),
                Ok(length) => return Ok(Some(buffer[..length].to_vec())),
                Err(error) if retryable(&error) => {
                    if !pause_until(deadline) {
                        return Ok(None);
                    }
                }
                Err(error) => return Err(format!("MIDI read: {error}")),
            }
        }
    }

    fn elapsed(&self) -> Duration {
        self.opened.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn retryable(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted)
}

fn pause_until(deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return false;
    }
    std::thread::sleep(remaining.min(Duration::from_millis(4)));
    true
}

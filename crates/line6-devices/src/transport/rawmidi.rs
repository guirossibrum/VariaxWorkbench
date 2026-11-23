//! Linux ALSA rawmidi link: a device's own MIDI port as its kernel driver
//! exposes it. No driver is detached.

use crate::session::{Link, Path};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// ALSA card id of the PODxt Live (`snd_usb_pod`).
pub const PODXT_LIVE_CARD: &str = "PODxtLive";

pub struct RawMidiLink {
    port: File,
    incoming: Receiver<Vec<u8>>,
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
        let device = find_card(card_id)?;
        let port = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device)
            .map_err(|e| format!("{}: {e}", device.display()))?;
        let mut reader = port
            .try_clone()
            .map_err(|e| format!("{}: {e}", device.display()))?;
        let (sender, incoming) = mpsc::channel();
        // Blocking reads on their own thread; the thread ends with the process.
        std::thread::spawn(move || {
            let mut buffer = [0; 512];
            while let Ok(length) = reader.read(&mut buffer) {
                if length == 0 || sender.send(buffer[..length].to_vec()).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            port,
            incoming,
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
        match self.incoming.recv_timeout(wait) {
            Ok(chunk) => Ok(Some(chunk)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err("MIDI port closed.".into()),
        }
    }

    fn elapsed(&self) -> Duration {
        self.opened.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

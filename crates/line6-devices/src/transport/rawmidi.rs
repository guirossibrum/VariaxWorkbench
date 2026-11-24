//! Linux ALSA rawmidi link: a device's own MIDI port as its kernel driver
//! exposes it. No driver is detached.

use super::RawMidiSpec;
use crate::session::{Link, Path};
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Compatibility export for callers of the original raw MIDI adapter API (#214).
pub use crate::family::podxt::transport::PODXT_LIVE_CARD;

pub struct RawMidiLink {
    port: File,
    opened: Instant,
    path: Path,
    pub device: PathBuf,
}

/// `/dev/snd/midiC<n>D0` for the ALSA card whose id is `card_id`.
pub fn find_card(card_id: &str) -> Result<PathBuf, String> {
    find_device(card_id, |card| format!("/dev/snd/midiC{card}D0").into())
}

fn find_device(card_id: &str, device_node: impl FnOnce(u32) -> PathBuf) -> Result<PathBuf, String> {
    let cards = std::fs::read_to_string("/proc/asound/cards")
        .map_err(|e| format!("/proc/asound/cards: {e}"))?;
    let marker = format!("[{card_id}");
    let card = cards
        .lines()
        .filter(|line| line.contains(&marker))
        .find_map(|line| line.split_whitespace().next()?.parse::<u32>().ok())
        .ok_or_else(|| format!("ALSA card {card_id} not found in /proc/asound/cards."))?;
    let device = device_node(card);
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

    pub fn open_spec(spec: RawMidiSpec) -> Result<Self, String> {
        Self::open_device(
            find_device(spec.card_id, |card| spec.device_node(card))?,
            spec.path,
        )
    }

    pub fn open_device(device: PathBuf, path: Path) -> Result<Self, String> {
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
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut remaining = bytes;
        while !remaining.is_empty() {
            if Instant::now() >= deadline {
                return Err("MIDI write timed out.".into());
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    fn fixture(port: impl Into<OwnedFd>) -> RawMidiLink {
        let port = File::from(port.into());
        RawMidiLink {
            port,
            opened: Instant::now(),
            path: Path::PodxtMidi,
            device: PathBuf::new(),
        }
    }

    #[test]
    fn nonblocking_link_reads_writes_and_closes_the_only_descriptor() {
        let (reader, writer) = UnixStream::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        writer.set_nonblocking(true).unwrap();
        let mut reader = fixture(reader);
        let mut writer = fixture(writer);
        assert_eq!(reader.read(Duration::from_millis(2)).unwrap(), None);
        writer.write(&[0xf0, 0xf7]).unwrap();
        assert_eq!(
            reader.read(Duration::from_secs(1)).unwrap(),
            Some(vec![0xf0, 0xf7])
        );
        drop(writer);
        assert_eq!(
            reader.read(Duration::from_millis(2)),
            Err("MIDI port closed.".into())
        );
    }
}

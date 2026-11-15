//! Direct, read-only Variax control through PODxt Live's second USB interface.
//! Interface 0 (POD MIDI/audio) is never claimed or reset. Interface 1 is released
//! and its prior kernel driver reattached after every bounded operation.
pub use crate::read_session::CurrentPreset;
use crate::{
    midi::SysexDecoder,
    midi_enable,
    read_session::{self, ControlTransport},
};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// User safety requirement: no live USB traffic until offline validation is
/// complete and the user explicitly authorizes a later hardware phase. There
/// is deliberately no environment variable, CLI flag or GUI override.
pub const LIVE_ACCESS_LOCKED: bool = true;

struct Session {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    detached: bool,
    claimed: bool,
    enabled: bool,
    decoder: SysexDecoder,
    queue: VecDeque<Vec<u8>>,
}

impl Session {
    fn open() -> Result<Self, String> {
        if LIVE_ACCESS_LOCKED {
            return Err("Live USB access is locked during offline validation. No device was opened and no message was sent.".into());
        }
        let devices = rusb::devices().map_err(|e| e.to_string())?;
        let matches: Vec<_> = devices
            .iter()
            .filter(|d| {
                d.device_descriptor()
                    .is_ok_and(|d| d.vendor_id() == 0x0e41 && d.product_id() == 0x4650)
            })
            .collect();
        if matches.len() != 1 {
            return Err(format!("expected one PODxt Live, found {}", matches.len()));
        }
        let handle = matches[0].open().map_err(|e| format!("Cannot open PODxt Live USB control: {e}. USB device access permission may be required."))?;
        let mut session = Self {
            handle,
            detached: false,
            claimed: false,
            enabled: false,
            decoder: SysexDecoder::default(),
            queue: VecDeque::new(),
        };
        if session
            .handle
            .kernel_driver_active(1)
            .map_err(|e| e.to_string())?
        {
            session
                .handle
                .detach_kernel_driver(1)
                .map_err(|e| e.to_string())?;
            session.detached = true;
        }
        session
            .handle
            .claim_interface(1)
            .map_err(|e| e.to_string())?;
        session.claimed = true;
        session
            .handle
            .set_alternate_setting(1, 1)
            .map_err(|e| e.to_string())?;
        Ok(session)
    }
    fn send_read_command(&mut self, bytes: &[u8]) -> Result<(), String> {
        if LIVE_ACCESS_LOCKED {
            return Err("Live USB access is locked".into());
        }
        if !read_session::is_read_session_message(bytes) {
            return Err("Command is not in the read-session allowlist".into());
        }
        if bytes == midi_enable() {
            self.enabled = true;
        }
        for chunk in bytes.chunks(16) {
            let sent = self
                .handle
                .write_interrupt(0x05, chunk, Duration::from_secs(1))
                .map_err(|e| e.to_string())?;
            if sent != chunk.len() {
                return Err("short USB control write".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if bytes == read_session::MIDI_DISABLE {
            self.enabled = false;
        }
        Ok(())
    }
    fn next_reply(
        &mut self,
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, String> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if cancel.load(Ordering::Relaxed) {
                return Err("guitar read cancelled".into());
            }
            if let Some(message) = self.queue.pop_front() {
                return Ok(Some(message));
            }
            let mut packet = [0; 16];
            match self
                .handle
                .read_interrupt(0x86, &mut packet, Duration::from_millis(100))
            {
                Ok(length) => self.queue.extend(self.decoder.feed(&packet[..length])?),
                Err(rusb::Error::Timeout) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(None)
    }
    fn cleanup(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        if self.enabled {
            if let Err(e) = self.send_read_command(&read_session::MIDI_DISABLE) {
                errors.push(e);
            }
            self.enabled = false;
        }
        if self.claimed {
            if let Err(e) = self.handle.release_interface(1) {
                errors.push(e.to_string());
            } else {
                self.claimed = false;
            }
        }
        if self.detached {
            if let Err(e) = self.handle.attach_kernel_driver(1) {
                errors.push(e.to_string());
            } else {
                self.detached = false;
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!("USB cleanup: {}", errors.join("; ")))
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl ControlTransport for Session {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_read_command(bytes)
    }
    fn next_message(
        &mut self,
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, String> {
        self.next_reply(cancel, timeout)
    }
}

pub fn read_current(cancel: &AtomicBool) -> Result<CurrentPreset, String> {
    let mut session = Session::open()?;
    let operation = read_session::read_current_on(&mut session, cancel);
    let cleanup = session.cleanup();
    match (operation, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hard_lock_blocks_before_usb_enumeration_or_open() {
        let error = read_current(&AtomicBool::new(false))
            .err()
            .expect("live access must be locked");
        assert!(error.contains("No device was opened"));
    }
}

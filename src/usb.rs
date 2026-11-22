//! Direct, read-only Variax control through PODxt Live's second USB interface.
//! Interface 0 (POD MIDI/audio) is never claimed or reset. Interface 1 is released
//! and its prior kernel driver reattached after every bounded operation.
pub use crate::read_session::CurrentPreset;
use crate::{
    hardware_gates::{self, Gate},
    identity_stage::{self, IdentityFailure, IdentityTransport},
    midi::SysexDecoder,
    midi_enable,
    read_session::{self, ControlTransport},
};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

struct Session {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
    detached: bool,
    claimed: bool,
    enabled: bool,
    response_gate: Gate,
    decoder: SysexDecoder,
    queue: VecDeque<Vec<u8>>,
    /// Set when the one identity request is sent. Later polls use this same
    /// deadline, so a second reply still counts inside the original 3 seconds.
    reply_deadline: Option<Instant>,
    release_was_attempted: bool,
    reattach_was_attempted: bool,
    release_did_fail: bool,
    reattach_did_fail: bool,
}

impl Session {
    fn open() -> Result<Self, String> {
        hardware_gates::run_if_unlocked(Gate::OsDiscovery, Self::open_unlocked)
    }

    fn open_handle() -> Result<Self, String> {
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
        Ok(Self {
            handle,
            detached: false,
            claimed: false,
            enabled: false,
            response_gate: Gate::IdentityRequest,
            decoder: SysexDecoder::default(),
            queue: VecDeque::new(),
            reply_deadline: None,
            release_was_attempted: false,
            reattach_was_attempted: false,
            release_did_fail: false,
            reattach_did_fail: false,
        })
    }

    /// Detach, then claim, then the alternate setting. A failure after detach
    /// still runs cleanup, and a cleanup error is kept beside the original error.
    fn open_unlocked() -> Result<Self, String> {
        let mut session = Self::open_handle()?;
        if let Err(error) = session.prepare_interface() {
            return Err(session.finish_open_error(error));
        }
        Ok(session)
    }

    fn prepare_interface(&mut self) -> Result<(), String> {
        if self
            .handle
            .kernel_driver_active(1)
            .map_err(|e| e.to_string())?
        {
            self.handle
                .detach_kernel_driver(1)
                .map_err(|e| e.to_string())?;
            self.detached = true;
        }
        if let Err(error) = self.handle.claim_interface(1) {
            return Err(error.to_string());
        }
        self.claimed = true;
        if let Err(error) = self.handle.set_alternate_setting(1, 1) {
            return Err(error.to_string());
        }
        Ok(())
    }

    fn finish_open_error(mut self, error: String) -> String {
        match self.cleanup() {
            Ok(()) => error,
            Err(cleanup) => format!("{error}; {cleanup}"),
        }
    }
    fn send_read_command(&mut self, bytes: &[u8]) -> Result<(), String> {
        if !read_session::is_read_session_message(bytes) {
            return Err("Command is not in the read-session allowlist".into());
        }
        let gate = if bytes == read_session::IDENTITY_REQUEST {
            Gate::IdentityRequest
        } else {
            Gate::CurrentSettingsRead
        };
        hardware_gates::run_if_unlocked(gate, || self.send_read_command_unlocked(bytes))
    }

    fn send_read_command_unlocked(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.response_gate = if bytes == read_session::IDENTITY_REQUEST {
            Gate::IdentityRequest
        } else {
            Gate::CurrentSettingsRead
        };
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
        hardware_gates::run_if_unlocked(self.response_gate, || {
            self.next_reply_unlocked(cancel, timeout)
        })
    }

    fn next_reply_unlocked(
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

impl IdentityTransport for Session {
    fn detach_if_active(&mut self) -> Result<(), IdentityFailure> {
        match self.handle.kernel_driver_active(1) {
            Ok(false) => Ok(()),
            Ok(true) => self
                .handle
                .detach_kernel_driver(1)
                .map_err(|_| IdentityFailure::DetachFailed)
                .inspect(|_| self.detached = true),
            Err(_) => Err(IdentityFailure::DetachFailed),
        }
    }

    fn claim_interface(&mut self) -> Result<(), IdentityFailure> {
        self.handle
            .claim_interface(1)
            .map_err(|_| IdentityFailure::ClaimFailed)
            .inspect(|_| self.claimed = true)
    }

    fn set_alternate(&mut self) -> Result<(), IdentityFailure> {
        self.handle
            .set_alternate_setting(1, 1)
            .map_err(|_| IdentityFailure::AlternateFailed)
    }

    fn detach_succeeded(&self) -> bool {
        self.detached
    }

    fn claim_succeeded(&self) -> bool {
        self.claimed
    }

    fn send_identity_request(&mut self) -> Result<(), IdentityFailure> {
        self.send_read_command(&read_session::IDENTITY_REQUEST)
            .map_err(|_| IdentityFailure::OpenFailed)?;
        self.reply_deadline = Some(Instant::now() + identity_stage::REPLY_TIMEOUT);
        Ok(())
    }

    fn poll_reply(&mut self, cancel: &AtomicBool) -> Result<Option<Vec<u8>>, IdentityFailure> {
        let Some(deadline) = self.reply_deadline else {
            return Err(IdentityFailure::OpenFailed);
        };
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match self.next_reply_unlocked(cancel, remaining) {
            Ok(message) => Ok(message),
            Err(error) if error.contains("cancelled") => Err(IdentityFailure::Cancelled),
            Err(_) => Err(IdentityFailure::OpenFailed),
        }
    }

    fn take_scripted_cancel(&mut self) -> bool {
        false
    }

    fn release_interface(&mut self) -> Result<(), IdentityFailure> {
        self.release_was_attempted = true;
        if !self.claimed {
            return Ok(());
        }
        match self.handle.release_interface(1) {
            Ok(()) => {
                self.claimed = false;
                Ok(())
            }
            Err(_) => {
                self.release_did_fail = true;
                Err(IdentityFailure::CleanupFailed)
            }
        }
    }

    fn reattach_driver(&mut self) -> Result<(), IdentityFailure> {
        self.reattach_was_attempted = true;
        if !self.detached {
            return Ok(());
        }
        match self.handle.attach_kernel_driver(1) {
            Ok(()) => {
                self.detached = false;
                Ok(())
            }
            Err(_) => {
                self.reattach_did_fail = true;
                Err(IdentityFailure::CleanupFailed)
            }
        }
    }

    fn release_attempted(&self) -> bool {
        self.release_was_attempted
    }

    fn reattach_attempted(&self) -> bool {
        self.reattach_was_attempted
    }

    fn release_failed(&self) -> bool {
        self.release_did_fail
    }

    fn reattach_failed(&self) -> bool {
        self.reattach_did_fail
    }
}

/// One Variax identity request. Gate G2 only: this does not require the
/// settings-read gate. The lock is checked before any USB enumeration.
pub fn read_identity(
    cancel: &AtomicBool,
) -> Result<identity_stage::IdentityPair, identity_stage::IdentifyError> {
    // Gate G2 is the first check. Nothing below runs while it is locked.
    if hardware_gates::is_locked(Gate::IdentityRequest) {
        return Err(identity_stage::IdentifyError::Locked);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(identity_stage::IdentifyError::Failed(
            identity_stage::IdentifyStop {
                failure: IdentityFailure::Cancelled,
                cleanup: None,
                cleanup_ran: false,
                release_failed: false,
                reattach_failed: false,
                bridge: None,
                guitar: None,
            },
        ));
    }
    let mut session = match Session::open_handle() {
        Ok(session) => session,
        Err(_) => {
            return Err(identity_stage::IdentifyError::Failed(
                identity_stage::IdentifyStop {
                    failure: IdentityFailure::OpenFailed,
                    cleanup: None,
                    cleanup_ran: false,
                    release_failed: false,
                    reattach_failed: false,
                    bridge: None,
                    guitar: None,
                },
            ));
        }
    };
    let exchange = identity_stage::run_identity_exchange(&mut session, cancel);
    identity_stage::exchange_result(exchange)
}

pub fn read_current(cancel: &AtomicBool) -> Result<CurrentPreset, String> {
    hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, || {
            let mut session = Session::open()?;
            let operation = read_session::read_current_on(&mut session, cancel);
            let cleanup = session.cleanup();
            match (operation, cleanup) {
                (Ok(value), Ok(())) => Ok(value),
                (Err(error), Ok(())) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hard_lock_blocks_before_usb_enumeration_or_open() {
        let error = read_current(&AtomicBool::new(false))
            .err()
            .expect("live access must be locked");
        assert!(error.contains("hardware gate") && error.contains("is locked"));
        assert!(
            error.contains("IdentityRequest"),
            "the settings entry is behind G2 as well as G3: {error}"
        );
    }

    #[test]
    fn identity_entry_stops_at_g2_and_does_not_require_g3() {
        let error = read_identity(&AtomicBool::new(false)).expect_err("G2 stays locked");
        assert!(matches!(error, identity_stage::IdentifyError::Locked));
    }
}

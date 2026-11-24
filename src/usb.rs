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
use line6_devices::family::variax::transport::VARIAX_VIA_PODXT_LIVE as USB;
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
    orphan_end: bool,
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

    fn open_handle(expected_path: Option<&str>) -> Result<Self, String> {
        let devices = rusb::devices().map_err(|e| e.to_string())?;
        let matches: Vec<_> = devices
            .iter()
            .filter(|d| {
                d.device_descriptor()
                    .is_ok_and(|d| d.vendor_id() == USB.vendor && d.product_id() == USB.product)
            })
            .collect();
        if matches.len() != 1 {
            return Err(format!("expected one PODxt Live, found {}", matches.len()));
        }
        let device = &matches[0];
        if let Some(expected_path) = expected_path {
            let port_path = device
                .port_numbers()
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|port| port.to_string())
                .collect::<Vec<_>>()
                .join(".");
            if format!("{}-{port_path}", device.bus_number()) != expected_path {
                return Err("the PODxt Live USB attachment changed".into());
            }
        }
        let handle = device.open().map_err(|e| format!("Cannot open PODxt Live USB control: {e}. USB device access permission may be required."))?;
        Ok(Self {
            handle,
            detached: false,
            claimed: false,
            enabled: false,
            response_gate: Gate::IdentityRequest,
            decoder: SysexDecoder::default(),
            queue: VecDeque::new(),
            orphan_end: false,
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
        let mut session = Self::open_handle(None)?;
        if let Err(error) = session.prepare_interface() {
            return Err(session.finish_open_error(error));
        }
        Ok(session)
    }

    fn prepare_interface(&mut self) -> Result<(), String> {
        if self
            .handle
            .kernel_driver_active(USB.interface)
            .map_err(|e| e.to_string())?
        {
            self.handle
                .detach_kernel_driver(USB.interface)
                .map_err(|e| e.to_string())?;
            self.detached = true;
        }
        if let Err(error) = self.handle.claim_interface(USB.interface) {
            return Err(error.to_string());
        }
        self.claimed = true;
        if let Err(error) = self
            .handle
            .set_alternate_setting(USB.interface, USB.altsetting)
        {
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
        for chunk in bytes.chunks(identity_stage::WRITE_CHUNK) {
            let sent = self
                .handle
                .write_interrupt(USB.endpoint_out, chunk, identity_stage::WRITE_TIMEOUT)
                .map_err(|e| e.to_string())?;
            if sent != chunk.len() {
                return Err("short USB control write".into());
            }
            std::thread::sleep(identity_stage::CHUNK_GAP);
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
            let mut packet = [0; identity_stage::LISTEN_BYTES];
            match self.handle.read_interrupt(
                USB.endpoint_in,
                &mut packet,
                identity_stage::READ_POLL,
            ) {
                Ok(length) => {
                    if self.response_gate == Gate::IdentityRequest
                        && packet[..length] == [0xf7]
                        && self.decoder.is_idle()
                    {
                        self.orphan_end = true;
                    }
                    self.queue.extend(decode_interrupt_packet(
                        &mut self.decoder,
                        &packet[..length],
                    )?);
                    if let Some(message) = self.queue.pop_front() {
                        return Ok(Some(message));
                    }
                }
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
            if let Err(e) = self.handle.release_interface(USB.interface) {
                errors.push(e.to_string());
            } else {
                self.claimed = false;
            }
        }
        if self.detached {
            if let Err(e) = self.handle.attach_kernel_driver(USB.interface) {
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
        match self.handle.kernel_driver_active(USB.interface) {
            Ok(false) => Ok(()),
            Ok(true) => self
                .handle
                .detach_kernel_driver(USB.interface)
                .map_err(|_| IdentityFailure::DetachFailed)
                .inspect(|_| self.detached = true),
            Err(_) => Err(IdentityFailure::DetachFailed),
        }
    }

    fn claim_interface(&mut self) -> Result<(), IdentityFailure> {
        self.handle
            .claim_interface(USB.interface)
            .map_err(|_| IdentityFailure::ClaimFailed)
            .inspect(|_| self.claimed = true)
    }

    fn set_alternate(&mut self) -> Result<(), IdentityFailure> {
        self.handle
            .set_alternate_setting(USB.interface, USB.altsetting)
            .map_err(|_| IdentityFailure::AlternateFailed)
    }

    fn detach_succeeded(&self) -> bool {
        self.detached
    }

    fn claim_succeeded(&self) -> bool {
        self.claimed
    }

    fn send_identity_request(&mut self) -> Result<(), IdentityFailure> {
        // One question, sent as soon as interface 1 is ready. The read that
        // already recognized this guitar did not wait first and did not ask
        // again. The three-second window starts after this write.
        self.send_read_command(&read_session::IDENTITY_REQUEST)
            .map_err(|_| IdentityFailure::SendFailed)?;
        self.reply_deadline = Some(Instant::now() + identity_stage::REPLY_TIMEOUT);
        Ok(())
    }

    fn poll_reply(&mut self, cancel: &AtomicBool) -> Result<Option<Vec<u8>>, IdentityFailure> {
        let Some(deadline) = self.reply_deadline else {
            return Err(IdentityFailure::SendFailed);
        };
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        match self.next_reply_unlocked(cancel, remaining) {
            Ok(message) => Ok(message),
            Err(error) if error.contains("cancelled") => Err(IdentityFailure::Cancelled),
            Err(_) => Err(IdentityFailure::ReadFailed),
        }
    }

    fn take_orphan_end(&mut self) -> bool {
        std::mem::take(&mut self.orphan_end)
    }

    fn take_scripted_cancel(&mut self) -> bool {
        false
    }

    fn release_interface(&mut self) -> Result<(), IdentityFailure> {
        self.release_was_attempted = true;
        if !self.claimed {
            return Ok(());
        }
        match self.handle.release_interface(USB.interface) {
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
        match self.handle.attach_kernel_driver(USB.interface) {
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

/// Classify an `open_handle` error without opening a device.
///
/// Access denied means Linux refused the open. The driver is still attached
/// and the identity question was not sent. Any other open error is the same
/// outcome with a different cause. A later write or read failure is not an
/// open failure.
pub(crate) fn open_failure_kind(error: &str) -> IdentityFailure {
    if error.contains("Access denied") {
        IdentityFailure::AccessDenied
    } else {
        IdentityFailure::OpenFailed
    }
}

/// The kernel receive path rewrites three status bytes whose low nibble is 2
/// before MIDI decoding. The saved identity capture starts with `0xF0`. This
/// stays on the USB receive path so other MIDI parsers are unchanged.
pub(crate) fn normalize_podxt_status(packet: &mut [u8]) {
    for byte in packet.iter_mut() {
        *byte = match *byte {
            0xf2 => 0xf0,
            0xb2 => 0xb0,
            0xc2 => 0xc0,
            other => other,
        };
    }
}

pub(crate) fn decode_interrupt_packet(
    decoder: &mut SysexDecoder,
    packet: &[u8],
) -> Result<Vec<Vec<u8>>, String> {
    let mut normalized = packet.to_vec();
    normalize_podxt_status(&mut normalized);
    decoder.feed(&normalized)
}

fn open_stopped(failure: IdentityFailure) -> identity_stage::IdentifyError {
    identity_stage::IdentifyError::Failed(identity_stage::IdentifyStop {
        failure,
        cleanup: None,
        cleanup_ran: false,
        release_failed: false,
        reattach_failed: false,
        bridge: None,
        guitar: None,
    })
}

/// An open refusal stops before detach or send, with no second open.
fn identity_on<T: IdentityTransport>(
    open: impl FnOnce() -> Result<T, String>,
    cancel: &AtomicBool,
    exchange: impl FnOnce(&mut T, &AtomicBool) -> identity_stage::IdentityExchange,
) -> Result<identity_stage::IdentityExchange, identity_stage::IdentifyError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(open_stopped(IdentityFailure::Cancelled));
    }
    let mut session = open().map_err(|error| open_stopped(open_failure_kind(&error)))?;
    Ok(exchange(&mut session, cancel))
}

/// One Variax identity request. Gate G2 only: this does not require the
/// settings-read gate. The lock is checked before any USB enumeration.
pub fn read_identity(
    cancel: &AtomicBool,
) -> Result<identity_stage::IdentityPair, identity_stage::IdentifyError> {
    // Gate G2 is the first check. Nothing below runs while it is locked.
    if hardware_gates::is_locked(Gate::IdentityRequest)
        || hardware_gates::is_locked(Gate::OsDiscovery)
    {
        return Err(identity_stage::IdentifyError::Locked);
    }
    // A test build stops here, before enumeration, even though G2 is open.
    // The normal app may open USB only when the user runs step 2.
    #[cfg(test)]
    {
        let _ = cancel;
        Err(identity_stage::IdentifyError::Locked)
    }
    #[cfg(not(test))]
    {
        if cancel.load(Ordering::Relaxed) {
            return Err(open_stopped(IdentityFailure::Cancelled));
        }
        let _rig = line6_devices::device_lock::DeviceLock::acquire().map_err(|error| {
            open_stopped(match error {
                line6_devices::device_lock::LockError::Busy => IdentityFailure::RigBusy,
                line6_devices::device_lock::LockError::Unavailable(_) => {
                    IdentityFailure::LockUnavailable
                }
            })
        })?;
        identity_on(
            || Session::open_handle(None),
            cancel,
            identity_stage::run_identity_exchange,
        )
        .and_then(identity_stage::exchange_result)
    }
}

/// Use the Device check exchange against the expected attachment while the
/// caller holds the rig lock. Only an observed orphan tail permits one resend.
#[cfg(all(not(test), target_os = "linux"))]
pub fn read_discovery_identity(
    _rig: &line6_devices::device_lock::DeviceLock,
    expected_path: &str,
    cancel: &AtomicBool,
) -> Result<identity_stage::DiscoveryIdentity, identity_stage::IdentifyError> {
    use identity_stage::IdentifyError;
    hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
        hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
            Ok(identity_on(
                || Session::open_handle(Some(expected_path)),
                cancel,
                identity_stage::run_discovery_exchange,
            )
            .and_then(identity_stage::discovery_result))
        })
    })
    .unwrap_or(Err(IdentifyError::Locked))
}

pub fn read_current(cancel: &AtomicBool) -> Result<CurrentPreset, String> {
    // A test build stops before enumeration, even with every gate open.
    if cfg!(test) {
        return Err("test builds do not open a USB device".into());
    }
    hardware_gates::run_if_unlocked(Gate::IdentityRequest, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, || {
            let _rig = line6_devices::device_lock::DeviceLock::acquire()
                .map_err(|error| error.to_string())?;
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
    fn a_test_build_stops_before_usb_enumeration_or_open() {
        // Every gate is released (2026-11-23); the test-build guard still
        // returns before any USB enumeration.
        let error = read_current(&AtomicBool::new(false))
            .err()
            .expect("a test build must not read a device");
        assert!(error.contains("test builds do not open"), "{error}");
    }

    #[test]
    fn automatic_identity_refused_open_never_runs_the_exchange() {
        let error = identity_on::<identity_stage::ScriptedIdentity>(
            || Err("Access denied".into()),
            &AtomicBool::new(false),
            |_, _| panic!("refused open must not detach or send"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            identity_stage::IdentifyError::Failed(identity_stage::IdentifyStop {
                failure: IdentityFailure::AccessDenied,
                cleanup_ran: false,
                ..
            })
        ));
    }

    #[test]
    fn access_denied_is_named_apart_from_a_failed_open_or_send() {
        let denied = open_failure_kind(
            "Cannot open PODxt Live USB control: Access denied (insufficient permissions). USB device access permission may be required.",
        );
        assert_eq!(denied, IdentityFailure::AccessDenied);
        let sentence = denied.sentence();
        assert!(sentence.contains("not sent"), "{sentence}");
        assert!(sentence.contains("not detached"), "{sentence}");
        assert_ne!(sentence, IdentityFailure::OpenFailed.sentence());
        assert_ne!(sentence, IdentityFailure::SendFailed.sentence());
        assert_ne!(sentence, IdentityFailure::ReadFailed.sentence());
        assert_eq!(
            open_failure_kind("expected one PODxt Live, found 0"),
            IdentityFailure::OpenFailed
        );
        assert!(IdentityFailure::OpenFailed.sentence().contains("not sent"));
        assert!(
            IdentityFailure::SendFailed
                .sentence()
                .contains("not accepted")
        );
        assert!(
            IdentityFailure::ReadFailed
                .sentence()
                .contains("could not be read")
        );
    }

    #[test]
    fn captured_split_reply_and_a_leading_status_nibble_both_reassemble() {
        let whole = identity_stage::guitar_reply();
        assert_eq!(whole.len(), 17);
        let (body, end) = whole.split_at(16);
        let mut decoder = SysexDecoder::default();
        assert!(
            decode_interrupt_packet(&mut decoder, body)
                .unwrap()
                .is_empty()
        );
        let messages = decode_interrupt_packet(&mut decoder, end).unwrap();
        assert_eq!(messages, vec![whole.clone()]);
        let parsed = crate::midi::Identity::parse(&messages[0]).unwrap();
        assert_eq!(parsed.product_id(), identity_stage::GUITAR_PRODUCT);
        assert_eq!(parsed.version_string(), identity_stage::GUITAR_VERSION);

        let mut wide = [0u8; identity_stage::LISTEN_BYTES];
        wide[..body.len()].copy_from_slice(body);
        let mut decoder = SysexDecoder::default();
        assert!(
            decode_interrupt_packet(&mut decoder, &wide[..body.len()])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            decode_interrupt_packet(&mut decoder, end).unwrap(),
            vec![whole.clone()]
        );

        let mut led = body.to_vec();
        led[0] = 0xf2;
        let mut decoder = SysexDecoder::default();
        assert!(
            decode_interrupt_packet(&mut decoder, &led)
                .unwrap()
                .is_empty()
        );
        let messages = decode_interrupt_packet(&mut decoder, end).unwrap();
        assert_eq!(messages[0][0], 0xf0);
        assert_eq!(messages, vec![whole]);
        assert_eq!(identity_stage::LISTEN_BYTES, 16);
        assert_eq!(identity_stage::LISTEN_BYTES, identity_stage::WRITE_CHUNK);
    }

    #[test]
    fn identity_entry_does_not_open_usb_from_a_test_build() {
        assert!(!hardware_gates::is_locked(Gate::IdentityRequest));
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        // G2 is open, but this is a test build: the call returns before
        // enumeration. It must not be used as a live check.
        let error = read_identity(&AtomicBool::new(false)).expect_err("tests do not open USB");
        assert!(matches!(error, identity_stage::IdentifyError::Locked));
    }
}

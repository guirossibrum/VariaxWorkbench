#![forbid(unsafe_code)]
//! Offline Stage 2 identity exchange for the Variax 600 through the PODxt Live.
//!
//! This is a proposed procedure rehearsed with a fake USB connection. The
//! success script returns one Variax 600 reply after one universal request,
//! then keeps listening until the three-second window ends. A second reply
//! that shows up after the first one does not pass. A pass shows that the
//! simulation accepted that input and then put the pretend connection back.
//! The 2026-11-14 captures show that one request and that guitar reply, then
//! a settings read. The settings read is not part of this exchange. A PODxt
//! Live identity reply has no recorded request here, so it does not pass.
//! This module does not open a device, detach a driver, or send USB or MIDI.
//! Its tests use a fake connection. The normal app can send the request when
//! a user runs the card in `docs/hardware-gates/g2.md`. Agents do not.

use crate::midi::Identity;
use crate::read_session::IDENTITY_REQUEST;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// PODxt Live bridge product, from the published 2026-11-14 identity shape.
pub const BRIDGE_PRODUCT: u32 = 0x0003_000a;
/// Variax 300/600 product, from the published 2026-11-14 identity shape.
/// Workbench treats decimal 458756, this same id, as the 300/600.
pub const GUITAR_PRODUCT: u32 = 0x0007_0004;
pub const BRIDGE_VERSION: &str = "3.01";
pub const GUITAR_VERSION: &str = "3.10";

/// Timings of the reviewed live procedure. Cited from `src/usb.rs` and
/// `src/read_session.rs`. This module never sleeps on them.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(1);
pub const WRITE_CHUNK: usize = 16;
pub const CHUNK_GAP: Duration = Duration::from_millis(5);
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(3);
pub const READ_POLL: Duration = Duration::from_millis(100);
/// Bytes asked for on each interrupt read. The read that already recognized
/// this guitar asked for 16, and the saved answer arrived as 16 bytes and
/// then the closing byte. A larger read keeps waiting after that full
/// packet and can drop those bytes when the wait ends.
pub const LISTEN_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityRole {
    Bridge,
    Guitar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalIdentity {
    pub role: IdentityRole,
    pub product_id: u32,
    pub version: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityPair {
    pub guitar: Identity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityFailure {
    RigBusy,
    LockUnavailable,
    Unknown,
    Ambiguous,
    Inconsistent,
    TimedOut,
    CleanupFailed,
    Cancelled,
    /// Detach of interface 1 failed. Nothing was claimed and nothing is reattached.
    DetachFailed,
    /// Claim failed after a successful detach. Release is skipped. Reattach still runs.
    ClaimFailed,
    /// Alternate setting failed after a successful claim. Release and reattach both run.
    AlternateFailed,
    /// The PODxt Live could not be opened. Nothing was detached and nothing was sent.
    OpenFailed,
    /// Linux refused the open. Nothing was detached and nothing was sent.
    AccessDenied,
    /// Detach, claim, and the alternate setting finished, and the write did not.
    SendFailed,
    /// The one request was handed to the connection, and the reply could not be read.
    ReadFailed,
}

impl IdentityFailure {
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::RigBusy => "PODxt Live is busy with another operation.",
            Self::LockUnavailable => "Cannot lock the PODxt Live. Nothing was sent.",
            Self::Unknown => {
                "The answer was not a known PODxt Live or Variax 600 identity. Stopped."
            }
            Self::Ambiguous => "More than one identity answered for the same device. Stopped.",
            Self::Inconsistent => {
                "The reply was not the Variax 600 3.10 identity this procedure expects. Stopped."
            }
            Self::TimedOut => "No identity answer arrived in time. Stopped.",
            Self::CleanupFailed => "The guitar-control connection could not be put back. Stopped.",
            Self::Cancelled => "Identification was cancelled. Stopped.",
            Self::DetachFailed => "The guitar-control driver could not be detached. Stopped.",
            Self::ClaimFailed => "The guitar-control connection could not be claimed. Stopped.",
            Self::AlternateFailed => {
                "The guitar-control connection could not be prepared. Stopped."
            }
            Self::OpenFailed => {
                "The PODxt Live could not be opened, so the identity question was not sent. Stopped."
            }
            Self::AccessDenied => {
                "Linux did not allow this app to open the PODxt Live. The identity question was not sent, and the guitar driver was not detached. Stopped."
            }
            Self::SendFailed => "The identity question was not accepted. Stopped.",
            Self::ReadFailed => "The identity answer could not be read. Stopped.",
        }
    }

    pub const fn stop_token(self) -> &'static str {
        match self {
            Self::RigBusy => "stop=rig-busy",
            Self::LockUnavailable => "stop=lock-unavailable",
            Self::Unknown => "stop=unknown",
            Self::Ambiguous => "stop=ambiguous",
            Self::Inconsistent => "stop=inconsistent",
            Self::TimedOut => "stop=timed-out",
            Self::CleanupFailed => "stop=cleanup-failed",
            Self::Cancelled => "stop=cancelled",
            Self::DetachFailed => "stop=detach-failed",
            Self::ClaimFailed => "stop=claim-failed",
            Self::AlternateFailed => "stop=alternate-failed",
            Self::OpenFailed => "stop=open-failed",
            Self::AccessDenied => "stop=access-denied",
            Self::SendFailed => "stop=send-failed",
            Self::ReadFailed => "stop=read-failed",
        }
    }
}

/// An identity stop and, separately, a later release or reattach stop.
///
/// Both are kept. A claim failure followed by a failed reattach is not
/// reported as only one of those two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentifyStop {
    pub failure: IdentityFailure,
    /// `Some` when release or reattach failed. `None` when cleanup was not run,
    /// or when it ran and succeeded. `cleanup_ran` tells those last two apart.
    pub cleanup: Option<IdentityFailure>,
    pub cleanup_ran: bool,
    /// True when release was attempted and returned an error.
    pub release_failed: bool,
    /// True when reattach was attempted and returned an error.
    pub reattach_failed: bool,
    /// Bridge identity actually decoded before the exchange stopped, if any.
    pub bridge: Option<CanonicalIdentity>,
    /// Guitar identity actually decoded before the exchange stopped, if any.
    pub guitar: Option<CanonicalIdentity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifyError {
    /// The live path is not available. Nothing was sent.
    Locked,
    Failed(IdentifyStop),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityExchange {
    pub pair: Option<IdentityPair>,
    /// Decoded before a later stop. Cleared from `pair` when the exchange does
    /// not pass, and kept here so a report can name what was decoded.
    pub bridge: Option<CanonicalIdentity>,
    pub guitar: Option<CanonicalIdentity>,
    pub requests_sent: usize,
    pub release_attempted: bool,
    pub reattach_attempted: bool,
    pub release_failed: bool,
    pub reattach_failed: bool,
    pub failure: Option<IdentityFailure>,
    /// Set when release or reattach fails, including when `failure` is already set.
    pub cleanup_failure: Option<IdentityFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CancelPoint {
    Detach,
    Claim,
    Alternate,
}

/// In-memory stand-in for interface 1. Detach, claim, and the alternate
/// setting are separate steps. Replies are delivered one poll at a time.
/// A reply scheduled with [`ScriptedIdentity::arrive_after_first`] is not in
/// the queue until the first reply has been taken, so the exchange cannot
/// know that a second reply is coming.
#[derive(Clone, Debug)]
pub struct ScriptedIdentity {
    queue: VecDeque<Vec<u8>>,
    /// Not visible until the first reply is taken.
    late: Option<Vec<u8>>,
    retry_reply: Option<Vec<u8>>,
    orphan_end: bool,
    queued_at_start: usize,
    delivered: usize,
    sent: Vec<Vec<u8>>,
    /// Each detach, claim, alternate, send, poll, release, and reattach.
    calls: u32,
    driver_active: bool,
    detach_succeeded: bool,
    claim_attempted: bool,
    /// Stays true after a successful claim, including after a later release.
    claim_succeeded: bool,
    alternate_set: bool,
    release_attempted: bool,
    reattach_attempted: bool,
    release_failed: bool,
    reattach_failed: bool,
    fail_detach: bool,
    fail_claim: bool,
    fail_alternate: bool,
    fail_release: bool,
    fail_reattach: bool,
    cancel_at: Option<CancelPoint>,
}

impl ScriptedIdentity {
    pub fn new(steps: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let queue: VecDeque<Vec<u8>> = steps.into_iter().collect();
        let queued_at_start = queue.len();
        Self {
            queue,
            late: None,
            retry_reply: None,
            orphan_end: false,
            queued_at_start,
            delivered: 0,
            sent: Vec::new(),
            calls: 0,
            driver_active: true,
            detach_succeeded: false,
            claim_attempted: false,
            claim_succeeded: false,
            alternate_set: false,
            release_attempted: false,
            reattach_attempted: false,
            release_failed: false,
            reattach_failed: false,
            fail_detach: false,
            fail_claim: false,
            fail_alternate: false,
            fail_release: false,
            fail_reattach: false,
            cancel_at: None,
        }
    }

    pub fn silent(self) -> Self {
        let mut script = self;
        script.queue.clear();
        script.late = None;
        script.queued_at_start = 0;
        script
    }

    /// Queue every reply before the exchange starts. The exchange still sees
    /// them one poll at a time and is not told how many there are.
    pub fn several(frames: Vec<Vec<u8>>) -> Self {
        Self::new(frames)
    }

    /// Hand the next reply to the exchange only after it has taken the first.
    pub fn arrive_after_first(mut self, frame: Vec<u8>) -> Self {
        self.late = Some(frame);
        self
    }

    pub fn fail_detach(mut self) -> Self {
        self.fail_detach = true;
        self
    }

    pub fn fail_claim(mut self) -> Self {
        self.fail_claim = true;
        self
    }

    pub fn fail_alternate(mut self) -> Self {
        self.fail_alternate = true;
        self
    }

    pub fn fail_release(mut self) -> Self {
        self.fail_release = true;
        self
    }

    pub fn fail_reattach(mut self) -> Self {
        self.fail_reattach = true;
        self
    }

    pub fn cancel_after_detach(mut self) -> Self {
        self.cancel_at = Some(CancelPoint::Detach);
        self
    }

    pub fn cancel_after_claim(mut self) -> Self {
        self.cancel_at = Some(CancelPoint::Claim);
        self
    }

    pub fn cancel_after_alternate(mut self) -> Self {
        self.cancel_at = Some(CancelPoint::Alternate);
        self
    }

    pub fn sent(&self) -> &[Vec<u8>] {
        &self.sent
    }

    pub const fn calls(&self) -> u32 {
        self.calls
    }

    pub const fn driver_claimed(&self) -> bool {
        self.claim_succeeded
    }

    pub const fn detach_succeeded(&self) -> bool {
        self.detach_succeeded
    }

    pub const fn queued_at_start(&self) -> usize {
        self.queued_at_start
    }

    pub const fn delivered(&self) -> usize {
        self.delivered
    }

    fn bump(&mut self) {
        self.calls = self.calls.saturating_add(1);
    }
}

pub(crate) trait IdentityTransport {
    fn detach_if_active(&mut self) -> Result<(), IdentityFailure>;
    fn claim_interface(&mut self) -> Result<(), IdentityFailure>;
    fn set_alternate(&mut self) -> Result<(), IdentityFailure>;
    fn detach_succeeded(&self) -> bool;
    fn claim_succeeded(&self) -> bool;
    fn send_identity_request(&mut self) -> Result<(), IdentityFailure>;
    /// One complete reply, or `None` when the three-second window has ended.
    /// The fake returns `None` as soon as its queue is empty. It does not
    /// report how many replies remain.
    fn poll_reply(&mut self, cancel: &AtomicBool) -> Result<Option<Vec<u8>>, IdentityFailure>;
    fn take_scripted_cancel(&mut self) -> bool;
    fn take_orphan_end(&mut self) -> bool;
    fn release_interface(&mut self) -> Result<(), IdentityFailure>;
    fn reattach_driver(&mut self) -> Result<(), IdentityFailure>;
    fn release_attempted(&self) -> bool;
    fn reattach_attempted(&self) -> bool;
    fn release_failed(&self) -> bool;
    fn reattach_failed(&self) -> bool;
}

impl IdentityTransport for ScriptedIdentity {
    fn detach_if_active(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        if !self.driver_active {
            return Ok(());
        }
        if self.fail_detach {
            return Err(IdentityFailure::DetachFailed);
        }
        self.detach_succeeded = true;
        Ok(())
    }

    fn claim_interface(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        self.claim_attempted = true;
        if self.fail_claim {
            return Err(IdentityFailure::ClaimFailed);
        }
        self.claim_succeeded = true;
        Ok(())
    }

    fn set_alternate(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        if self.fail_alternate {
            return Err(IdentityFailure::AlternateFailed);
        }
        self.alternate_set = true;
        Ok(())
    }

    fn detach_succeeded(&self) -> bool {
        self.detach_succeeded
    }

    fn claim_succeeded(&self) -> bool {
        self.claim_succeeded
    }

    fn send_identity_request(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        self.sent.push(IDENTITY_REQUEST.to_vec());
        if self.sent.len() == 2
            && let Some(frame) = self.retry_reply.take()
        {
            self.queue.push_back(frame);
        }
        Ok(())
    }

    fn poll_reply(&mut self, _cancel: &AtomicBool) -> Result<Option<Vec<u8>>, IdentityFailure> {
        self.bump();
        let Some(frame) = self.queue.pop_front() else {
            return Ok(None);
        };
        self.delivered = self.delivered.saturating_add(1);
        if self.queue.is_empty()
            && let Some(late) = self.late.take()
        {
            self.queue.push_back(late);
        }
        Ok(Some(frame))
    }

    fn take_orphan_end(&mut self) -> bool {
        std::mem::take(&mut self.orphan_end)
    }

    fn take_scripted_cancel(&mut self) -> bool {
        let ready = match self.cancel_at {
            Some(CancelPoint::Detach) => self.detach_succeeded && !self.claim_attempted,
            Some(CancelPoint::Claim) => self.claim_succeeded && !self.alternate_set,
            Some(CancelPoint::Alternate) => self.alternate_set && self.sent.is_empty(),
            None => false,
        };
        if ready {
            self.cancel_at = None;
        }
        ready
    }

    fn release_interface(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        self.release_attempted = true;
        if self.fail_release {
            self.release_failed = true;
            return Err(IdentityFailure::CleanupFailed);
        }
        Ok(())
    }

    fn reattach_driver(&mut self) -> Result<(), IdentityFailure> {
        self.bump();
        self.reattach_attempted = true;
        if self.fail_reattach {
            self.reattach_failed = true;
            return Err(IdentityFailure::CleanupFailed);
        }
        Ok(())
    }

    fn release_attempted(&self) -> bool {
        self.release_attempted
    }

    fn reattach_attempted(&self) -> bool {
        self.reattach_attempted
    }

    fn release_failed(&self) -> bool {
        self.release_failed
    }

    fn reattach_failed(&self) -> bool {
        self.reattach_failed
    }
}

/// Published reply shape for the PODxt Live bridge (product `0x0003000a`, 3.01).
pub fn bridge_reply() -> Vec<u8> {
    vec![
        0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 3, 0, 10, 0, 0, 3, 0, 1, 0xf7,
    ]
}

/// Published reply shape for the Variax 600 (product `0x00070004`, 3.10).
pub fn guitar_reply() -> Vec<u8> {
    vec![
        0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
    ]
}

pub fn happy_script() -> ScriptedIdentity {
    ScriptedIdentity::new([guitar_reply()])
}

fn classify(packet: &[u8]) -> Result<(CanonicalIdentity, Identity), IdentityFailure> {
    let identity = Identity::parse(packet).map_err(|_| IdentityFailure::Unknown)?;
    let version = identity.version_string();
    let canonical = match identity.product_id() {
        BRIDGE_PRODUCT if version == BRIDGE_VERSION => CanonicalIdentity {
            role: IdentityRole::Bridge,
            product_id: BRIDGE_PRODUCT,
            version: BRIDGE_VERSION,
        },
        GUITAR_PRODUCT if version == GUITAR_VERSION => CanonicalIdentity {
            role: IdentityRole::Guitar,
            product_id: GUITAR_PRODUCT,
            version: GUITAR_VERSION,
        },
        BRIDGE_PRODUCT | GUITAR_PRODUCT => return Err(IdentityFailure::Inconsistent),
        _ => return Err(IdentityFailure::Unknown),
    };
    Ok((canonical, identity))
}

fn remember(exchange: &mut IdentityExchange, identity: CanonicalIdentity) {
    match identity.role {
        IdentityRole::Bridge => exchange.bridge = Some(identity),
        IdentityRole::Guitar => exchange.guitar = Some(identity),
    }
}

fn cancelled(transport: &mut impl IdentityTransport, cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed) || transport.take_scripted_cancel()
}

/// Keep polling until the window ends. `None` from the transport means the
/// three seconds are over, not "no bytes on this 100 ms poll". A second
/// complete reply before that end is ambiguous, even when the first reply
/// was the Variax 600.
fn read_window(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
    exchange: &mut IdentityExchange,
) -> Result<(), IdentityFailure> {
    let mut first = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(IdentityFailure::Cancelled);
        }
        match transport.poll_reply(cancel)? {
            Some(frame) if first.is_none() => first = Some(frame),
            Some(_) => {
                if let Some(frame) = first.as_deref()
                    && let Ok((identity, _)) = classify(frame)
                {
                    remember(exchange, identity);
                }
                return Err(IdentityFailure::Ambiguous);
            }
            None => break,
        }
    }
    let Some(frame) = first else {
        return Err(IdentityFailure::TimedOut);
    };
    match classify(&frame)? {
        (identity, parsed) if identity.role == IdentityRole::Guitar => {
            remember(exchange, identity);
            exchange.pair = Some(IdentityPair { guitar: parsed });
            Ok(())
        }
        (identity, _) => {
            remember(exchange, identity);
            Err(IdentityFailure::Inconsistent)
        }
    }
}

fn open_and_exchange(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
    exchange: &mut IdentityExchange,
    retry_lost_packet: bool,
) -> Result<(), IdentityFailure> {
    if cancelled(transport, cancel) {
        return Err(IdentityFailure::Cancelled);
    }
    transport.detach_if_active()?;
    if cancelled(transport, cancel) {
        return Err(IdentityFailure::Cancelled);
    }
    transport.claim_interface()?;
    if cancelled(transport, cancel) {
        return Err(IdentityFailure::Cancelled);
    }
    transport.set_alternate()?;
    if cancelled(transport, cancel) {
        return Err(IdentityFailure::Cancelled);
    }
    transport.send_identity_request()?;
    exchange.requests_sent = 1;
    match read_window(transport, cancel, exchange) {
        Err(IdentityFailure::TimedOut) if retry_lost_packet && transport.take_orphan_end() => {
            if cancelled(transport, cancel) {
                return Err(IdentityFailure::Cancelled);
            }
            transport.send_identity_request()?;
            exchange.requests_sent += 1;
            read_window(transport, cancel, exchange)
        }
        result => result,
    }
}

fn finish_cleanup(transport: &mut impl IdentityTransport, exchange: &mut IdentityExchange) {
    if transport.claim_succeeded() {
        let _ = transport.release_interface();
    }
    if transport.detach_succeeded() {
        let _ = transport.reattach_driver();
    }
    exchange.release_attempted = transport.release_attempted();
    exchange.reattach_attempted = transport.reattach_attempted();
    exchange.release_failed = transport.release_failed();
    exchange.reattach_failed = transport.reattach_failed();
    if exchange.release_failed || exchange.reattach_failed {
        exchange.cleanup_failure = Some(IdentityFailure::CleanupFailed);
        exchange.pair = None;
        if exchange.failure.is_none() {
            exchange.failure = Some(IdentityFailure::CleanupFailed);
        }
    }
}

/// One identity request on interface 1.
///
/// Detach happens before claim. Release runs only after a successful claim.
/// Reattach runs whenever detach succeeded, including when claim fails, the
/// alternate setting fails, or the person cancels before the send. A cancel
/// before detach sends nothing and does not clean up. The original stop and
/// any cleanup stop are both kept.
pub(crate) fn run_identity_exchange(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
) -> IdentityExchange {
    run_exchange(transport, cancel, false)
}

#[cfg(any(test, all(target_os = "linux", feature = "usb")))]
pub(crate) fn run_discovery_exchange(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
) -> IdentityExchange {
    run_exchange(transport, cancel, true)
}

fn run_exchange(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
    retry_lost_packet: bool,
) -> IdentityExchange {
    let mut exchange = IdentityExchange {
        pair: None,
        bridge: None,
        guitar: None,
        requests_sent: 0,
        release_attempted: false,
        reattach_attempted: false,
        release_failed: false,
        reattach_failed: false,
        failure: None,
        cleanup_failure: None,
    };
    if let Err(failure) = open_and_exchange(transport, cancel, &mut exchange, retry_lost_packet) {
        exchange.failure = Some(failure);
        exchange.pair = None;
    }
    finish_cleanup(transport, &mut exchange);
    exchange
}

pub(crate) fn exchange_result(exchange: IdentityExchange) -> Result<IdentityPair, IdentifyError> {
    if exchange.failure.is_none()
        && exchange.cleanup_failure.is_none()
        && let Some(pair) = exchange.pair
    {
        return Ok(pair);
    }
    Err(IdentifyError::Failed(IdentifyStop {
        failure: exchange.failure.unwrap_or(IdentityFailure::CleanupFailed),
        cleanup: exchange.cleanup_failure,
        cleanup_ran: exchange.release_attempted || exchange.reattach_attempted,
        release_failed: exchange.release_failed,
        reattach_failed: exchange.reattach_failed,
        bridge: exchange.bridge,
        guitar: exchange.guitar,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_probe::{self, Stage};
    use crate::hardware_gates::{self, Gate};

    fn run(script: ScriptedIdentity) -> (IdentityExchange, ScriptedIdentity) {
        let mut script = script;
        let exchange = run_identity_exchange(&mut script, &AtomicBool::new(false));
        (exchange, script)
    }

    #[test]
    fn discovery_resends_only_for_a_lost_first_packet_and_never_a_third_time() {
        for (orphan, reply, expected_requests, verified) in [
            (false, None, 1, false),
            (true, None, 2, false),
            (true, Some(guitar_reply()), 2, true),
            (true, Some(bridge_reply()), 2, false),
            (
                true,
                Some({
                    let mut wrong_firmware = guitar_reply();
                    wrong_firmware[15] = b'1';
                    wrong_firmware
                }),
                2,
                false,
            ),
        ] {
            let mut script = ScriptedIdentity::new([]);
            script.orphan_end = orphan;
            script.retry_reply = reply;
            let exchange = run_discovery_exchange(&mut script, &AtomicBool::new(false));
            assert_eq!(exchange.requests_sent, expected_requests);
            assert_eq!(script.sent().len(), expected_requests);
            assert_full_cleanup(&exchange, &script);
            assert_eq!(exchange_result(exchange).is_ok(), verified);
        }
    }

    #[test]
    fn discovery_discards_identity_if_cleanup_fails() {
        let mut script = happy_script().fail_reattach();
        let exchange = run_discovery_exchange(&mut script, &AtomicBool::new(false));
        assert!(exchange.release_attempted && exchange.reattach_attempted);
        assert_eq!(exchange.requests_sent, 1);
        assert!(matches!(
            exchange_result(exchange),
            Err(IdentifyError::Failed(IdentifyStop {
                cleanup: Some(IdentityFailure::CleanupFailed),
                ..
            }))
        ));
    }

    fn assert_full_cleanup(exchange: &IdentityExchange, script: &ScriptedIdentity) {
        assert!(exchange.release_attempted);
        assert!(exchange.reattach_attempted);
        assert!(script.release_attempted);
        assert!(script.reattach_attempted);
        assert!(script.driver_claimed());
        assert!(script.detach_succeeded());
    }

    #[test]
    fn successful_exchange_sends_one_variax_request_and_cleans_up() {
        let (exchange, script) = run(happy_script());
        assert_eq!(exchange.failure, None);
        let pair = exchange.pair.as_ref().expect("guitar identity");
        assert!(exchange.bridge.is_none());
        assert_eq!(pair.guitar.product_id(), GUITAR_PRODUCT);
        assert_eq!(pair.guitar.version_string(), GUITAR_VERSION);
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent(), &[IDENTITY_REQUEST.to_vec()]);
        assert_ne!(script.sent()[0], crate::midi_enable());
        assert_ne!(script.sent()[0], crate::read_session::MIDI_DISABLE.to_vec());
        assert_full_cleanup(&exchange, &script);
        // Detach, claim, alternate, send, first reply, window end, release, reattach.
        assert_eq!(script.calls(), 8);
        assert_eq!(script.delivered(), 1);
        assert_eq!(WRITE_TIMEOUT, Duration::from_secs(1));
        assert_eq!(WRITE_CHUNK, 16);
        assert_eq!(CHUNK_GAP, Duration::from_millis(5));
        assert_eq!(REPLY_TIMEOUT, Duration::from_secs(3));
        assert_eq!(READ_POLL, Duration::from_millis(100));
        assert_eq!(LISTEN_BYTES, WRITE_CHUNK);
    }

    #[test]
    fn unknown_identity_stops_and_still_cleans_up() {
        let mut reply = bridge_reply();
        reply[8] = 1;
        let (exchange, script) = run(ScriptedIdentity::new([reply]));
        assert_eq!(exchange.failure, Some(IdentityFailure::Unknown));
        assert!(exchange.pair.is_none());
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent().len(), 1);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn ambiguous_identity_stops() {
        let (exchange, script) = run(ScriptedIdentity::several(vec![
            bridge_reply(),
            bridge_reply(),
        ]));
        assert_eq!(exchange.failure, Some(IdentityFailure::Ambiguous));
        assert!(exchange.pair.is_none());
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.delivered(), 2);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn a_second_reply_after_the_first_valid_one_is_ambiguous() {
        let script = ScriptedIdentity::new([guitar_reply()]).arrive_after_first(guitar_reply());
        assert_eq!(script.queued_at_start(), 1);
        let (exchange, script) = run(script);
        assert_eq!(script.queued_at_start(), 1);
        assert_eq!(script.delivered(), 2);
        assert_eq!(exchange.failure, Some(IdentityFailure::Ambiguous));
        assert!(exchange.pair.is_none());
        assert_eq!(
            exchange.guitar.map(|identity| identity.product_id),
            Some(GUITAR_PRODUCT)
        );
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent(), &[IDENTITY_REQUEST.to_vec()]);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn inconsistent_identity_stops() {
        let (bridge, script) = run(ScriptedIdentity::new([bridge_reply()]));
        assert_eq!(bridge.failure, Some(IdentityFailure::Inconsistent));
        assert!(bridge.pair.is_none());
        assert_eq!(
            bridge.bridge.map(|identity| identity.product_id),
            Some(BRIDGE_PRODUCT)
        );
        assert_eq!(bridge.requests_sent, 1);
        assert_eq!(script.sent(), &[IDENTITY_REQUEST.to_vec()]);
        assert_full_cleanup(&bridge, &script);
        let mut wrong_version = guitar_reply();
        wrong_version[15] = b'2';
        let (exchange, script) = run(ScriptedIdentity::new([wrong_version]));
        assert_eq!(exchange.failure, Some(IdentityFailure::Inconsistent));
        assert!(exchange.guitar.is_none());
        assert_eq!(exchange.requests_sent, 1);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn timed_out_identity_stops_after_one_request() {
        let (exchange, script) = run(ScriptedIdentity::new([bridge_reply()]).silent());
        assert_eq!(exchange.failure, Some(IdentityFailure::TimedOut));
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent().len(), 1);
        assert_eq!(script.delivered(), 0);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn timeout_keeps_a_later_reattach_failure() {
        let (exchange, script) = run(ScriptedIdentity::new([guitar_reply()])
            .silent()
            .fail_reattach());
        assert_eq!(exchange.failure, Some(IdentityFailure::TimedOut));
        assert_eq!(
            exchange.cleanup_failure,
            Some(IdentityFailure::CleanupFailed)
        );
        assert!(exchange.reattach_failed);
        assert!(!exchange.release_failed);
        assert!(exchange.bridge.is_none());
        assert!(exchange.guitar.is_none());
        assert!(exchange.pair.is_none());
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent(), &[IDENTITY_REQUEST.to_vec()]);
        assert_full_cleanup(&exchange, &script);
    }

    #[test]
    fn extra_reply_is_ambiguous_and_cleanup_failure_stops_a_match() {
        let (duplicate, script) = run(ScriptedIdentity::several(vec![
            guitar_reply(),
            guitar_reply(),
        ]));
        assert_eq!(duplicate.failure, Some(IdentityFailure::Ambiguous));
        assert_eq!(duplicate.requests_sent, 1);
        assert_eq!(script.sent().len(), 1);
        assert_eq!(script.delivered(), 2);
        let (reattach, script) = run(happy_script().fail_reattach());
        assert_eq!(reattach.failure, Some(IdentityFailure::CleanupFailed));
        assert!(reattach.pair.is_none());
        assert!(reattach.reattach_failed);
        assert_eq!(
            reattach.guitar.map(|identity| identity.product_id),
            Some(GUITAR_PRODUCT)
        );
        assert_full_cleanup(&reattach, &script);
        let (release, script) = run(happy_script().fail_release());
        assert_eq!(release.failure, Some(IdentityFailure::CleanupFailed));
        assert!(release.pair.is_none());
        assert!(release.release_failed);
        assert!(script.release_attempted && script.reattach_attempted);
        assert!(!script.reattach_failed);
    }

    #[test]
    fn claim_failure_reattaches_without_release_and_keeps_both_errors() {
        let (claim, script) = run(happy_script().fail_claim());
        assert_eq!(claim.failure, Some(IdentityFailure::ClaimFailed));
        assert!(claim.cleanup_failure.is_none());
        assert!(script.detach_succeeded());
        assert!(!script.driver_claimed());
        assert!(!claim.release_attempted);
        assert!(claim.reattach_attempted);
        assert!(script.sent().is_empty());
        assert_eq!(claim.requests_sent, 0);

        let (both, script) = run(happy_script().fail_claim().fail_reattach());
        assert_eq!(both.failure, Some(IdentityFailure::ClaimFailed));
        assert_eq!(both.cleanup_failure, Some(IdentityFailure::CleanupFailed));
        assert!(both.reattach_failed);
        assert!(!both.release_attempted);
        assert!(both.reattach_attempted);
        assert!(script.sent().is_empty());
        let stop = exchange_result(both).unwrap_err();
        let IdentifyError::Failed(stop) = stop else {
            panic!("claim and reattach failures are a stopped exchange");
        };
        let detail = format!(
            "{} {}",
            stop.failure.sentence(),
            stop.cleanup.unwrap().sentence()
        );
        assert!(detail.contains("could not be claimed"));
        assert!(detail.contains("could not be put back"));
    }

    #[test]
    fn alternate_failure_releases_and_reattaches() {
        let (exchange, script) = run(happy_script().fail_alternate());
        assert_eq!(exchange.failure, Some(IdentityFailure::AlternateFailed));
        assert!(script.driver_claimed());
        assert!(exchange.release_attempted);
        assert!(exchange.reattach_attempted);
        assert!(script.sent().is_empty());
        let (both, _) = run(happy_script().fail_alternate().fail_reattach());
        assert_eq!(both.failure, Some(IdentityFailure::AlternateFailed));
        assert_eq!(both.cleanup_failure, Some(IdentityFailure::CleanupFailed));
        assert!(both.release_attempted && both.reattach_attempted);
    }

    #[test]
    fn cancel_before_detach_claims_nothing_and_does_not_clean_up() {
        let mut script = happy_script();
        let exchange = run_identity_exchange(&mut script, &AtomicBool::new(true));
        assert_eq!(exchange.failure, Some(IdentityFailure::Cancelled));
        assert!(script.sent().is_empty());
        assert_eq!(exchange.requests_sent, 0);
        assert!(!script.detach_succeeded());
        assert!(!script.driver_claimed());
        assert!(!exchange.release_attempted && !exchange.reattach_attempted);
        assert_eq!(script.calls(), 0);
    }

    #[test]
    fn cancel_after_detach_and_before_the_send_reattaches() {
        let (after_detach, script) = run(happy_script().cancel_after_detach());
        assert_eq!(after_detach.failure, Some(IdentityFailure::Cancelled));
        assert!(script.detach_succeeded());
        assert!(!script.driver_claimed());
        assert!(!after_detach.release_attempted);
        assert!(after_detach.reattach_attempted);
        assert!(script.sent().is_empty());

        let (after_claim, script) = run(happy_script().cancel_after_claim());
        assert_eq!(after_claim.failure, Some(IdentityFailure::Cancelled));
        assert!(script.driver_claimed());
        assert!(after_claim.release_attempted);
        assert!(after_claim.reattach_attempted);
        assert!(script.sent().is_empty());

        let (after_alt, script) = run(happy_script().cancel_after_alternate());
        assert_eq!(after_alt.failure, Some(IdentityFailure::Cancelled));
        assert!(after_alt.release_attempted && after_alt.reattach_attempted);
        assert!(script.sent().is_empty());
        assert_eq!(after_alt.requests_sent, 0);
    }

    #[test]
    fn detach_failure_does_not_claim_or_reattach() {
        let (exchange, script) = run(happy_script().fail_detach());
        assert_eq!(exchange.failure, Some(IdentityFailure::DetachFailed));
        assert!(!script.detach_succeeded());
        assert!(!script.driver_claimed());
        assert!(!exchange.release_attempted);
        assert!(!exchange.reattach_attempted);
        assert!(exchange.cleanup_failure.is_none());
        assert!(script.sent().is_empty());
    }

    #[test]
    fn stage_two_uses_the_fake_connection_and_does_not_open_a_device() {
        assert!(!hardware_gates::is_locked(Gate::IdentityRequest));
        assert!(!hardware_gates::is_locked(Gate::OsDiscovery));
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
        assert!(device_probe::LIVE_STAGES.contains(&Stage::Identification));
        // Live step 2 needs Linux and the usb feature. These tests never call it.
        assert_eq!(
            device_probe::live_stage_available(Stage::Identification),
            cfg!(all(target_os = "linux", feature = "usb"))
        );
    }
}

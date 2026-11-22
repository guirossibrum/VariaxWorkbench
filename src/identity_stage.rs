#![forbid(unsafe_code)]
//! Offline Stage 2 identity exchange for the PODxt Live bridge and the Variax 600.
//!
//! This is a proposed procedure rehearsed with a fake transport. The transport
//! is given one bridge reply and then one guitar reply, after two identical
//! universal requests. A pass shows that the simulation accepted those inputs.
//! The 2026-11-14 capture summary describes one identity request followed by a
//! settings read, so it does not establish how this two-request exchange would
//! behave on a device. This module does not open a device, detach a driver, or
//! send USB or MIDI. Gate G2 stays locked.

use crate::midi::Identity;
use crate::read_session::IDENTITY_REQUEST;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// PODxt Live bridge product, from the published 2026-11-14 identity shape.
pub const BRIDGE_PRODUCT: u32 = 0x0003_000a;
/// Variax 300/600 product, from the published 2026-11-14 identity shape.
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
    pub bridge: CanonicalIdentity,
    pub guitar: Identity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityFailure {
    Unknown,
    Ambiguous,
    Inconsistent,
    TimedOut,
    CleanupFailed,
    Cancelled,
}

impl IdentityFailure {
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::Unknown => {
                "The answer was not a known PODxt Live or Variax 600 identity. Stopped."
            }
            Self::Ambiguous => "More than one identity answered for the same device. Stopped.",
            Self::Inconsistent => {
                "The bridge and guitar identities do not match the recorded pair. Stopped."
            }
            Self::TimedOut => "No identity answer arrived in time. Stopped.",
            Self::CleanupFailed => "The guitar-control connection could not be put back. Stopped.",
            Self::Cancelled => "Identification was cancelled. Stopped.",
        }
    }

    pub const fn stop_token(self) -> &'static str {
        match self {
            Self::Unknown => "stop=unknown",
            Self::Ambiguous => "stop=ambiguous",
            Self::Inconsistent => "stop=inconsistent",
            Self::TimedOut => "stop=timed-out",
            Self::CleanupFailed => "stop=cleanup-failed",
            Self::Cancelled => "stop=cancelled",
        }
    }
}

/// An identity stop and, separately, a later release or reattach stop.
///
/// Both are kept. A timeout followed by a failed reattach is not reported as
/// only the timeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentifyStop {
    pub failure: IdentityFailure,
    /// `Some` when release or reattach failed. `None` when cleanup was not run,
    /// or when it ran and succeeded. `cleanup_ran` tells those last two apart.
    pub cleanup: Option<IdentityFailure>,
    pub cleanup_ran: bool,
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
    pub failure: Option<IdentityFailure>,
    /// Set when release or reattach fails, including when `failure` is already set.
    pub cleanup_failure: Option<IdentityFailure>,
}

#[derive(Clone, Debug)]
enum Step {
    One(Vec<u8>),
    None,
    Several(Vec<Vec<u8>>),
}

/// In-memory stand-in for interface 1. It records the identity request and the
/// release/reattach steps. It has no USB handle.
#[derive(Clone, Debug)]
pub struct ScriptedIdentity {
    steps: VecDeque<Step>,
    sent: Vec<Vec<u8>>,
    /// Send, reply, and cleanup calls. Decline and cancel leave this at zero.
    calls: u32,
    release_attempted: bool,
    reattach_attempted: bool,
    release_fails: bool,
    reattach_fails: bool,
}

impl ScriptedIdentity {
    pub fn new(steps: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            steps: steps.into_iter().map(Step::One).collect(),
            sent: Vec::new(),
            calls: 0,
            release_attempted: false,
            reattach_attempted: false,
            release_fails: false,
            reattach_fails: false,
        }
    }

    pub fn silent(self) -> Self {
        let mut script = self;
        script.steps = VecDeque::from([Step::None]);
        script
    }

    pub fn several(frames: Vec<Vec<u8>>) -> Self {
        Self {
            steps: VecDeque::from([Step::Several(frames)]),
            sent: Vec::new(),
            calls: 0,
            release_attempted: false,
            reattach_attempted: false,
            release_fails: false,
            reattach_fails: false,
        }
    }

    pub fn fail_release(mut self) -> Self {
        self.release_fails = true;
        self
    }

    pub fn fail_reattach(mut self) -> Self {
        self.reattach_fails = true;
        self
    }

    pub fn sent(&self) -> &[Vec<u8>] {
        &self.sent
    }

    pub const fn calls(&self) -> u32 {
        self.calls
    }
}

pub(crate) trait IdentityTransport {
    fn send_identity_request(&mut self) -> Result<(), IdentityFailure>;
    fn pending_reply_count(&self) -> usize;
    fn next_reply(&mut self) -> Result<Option<Vec<u8>>, IdentityFailure>;
    fn release_and_reattach(&mut self) -> Result<(), IdentityFailure>;
    fn release_attempted(&self) -> bool;
    fn reattach_attempted(&self) -> bool;
}

impl IdentityTransport for ScriptedIdentity {
    fn send_identity_request(&mut self) -> Result<(), IdentityFailure> {
        self.calls = self.calls.saturating_add(1);
        self.sent.push(IDENTITY_REQUEST.to_vec());
        Ok(())
    }

    fn pending_reply_count(&self) -> usize {
        match self.steps.front() {
            Some(Step::Several(frames)) => frames.len(),
            Some(Step::One(_)) => 1,
            Some(Step::None) | None => 0,
        }
    }

    fn next_reply(&mut self) -> Result<Option<Vec<u8>>, IdentityFailure> {
        self.calls = self.calls.saturating_add(1);
        match self.steps.pop_front() {
            Some(Step::One(frame)) => Ok(Some(frame)),
            Some(Step::None) | None => Ok(None),
            Some(Step::Several(frames)) => Ok(frames.into_iter().next()),
        }
    }

    fn release_and_reattach(&mut self) -> Result<(), IdentityFailure> {
        self.calls = self.calls.saturating_add(1);
        self.release_attempted = true;
        if self.release_fails {
            self.reattach_attempted = true;
            return Err(IdentityFailure::CleanupFailed);
        }
        self.reattach_attempted = true;
        if self.reattach_fails {
            Err(IdentityFailure::CleanupFailed)
        } else {
            Ok(())
        }
    }

    fn release_attempted(&self) -> bool {
        self.release_attempted
    }

    fn reattach_attempted(&self) -> bool {
        self.reattach_attempted
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
    ScriptedIdentity::new([bridge_reply(), guitar_reply()])
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

fn one_reply(
    transport: &mut impl IdentityTransport,
) -> Result<(CanonicalIdentity, Identity), IdentityFailure> {
    if transport.pending_reply_count() > 1 {
        let _ = transport.next_reply();
        return Err(IdentityFailure::Ambiguous);
    }
    match transport.next_reply()? {
        Some(frame) => classify(&frame),
        None => Err(IdentityFailure::TimedOut),
    }
}

/// One identity request for the bridge, then one for the guitar, then release
/// and reattach. Cleanup runs even when the exchange stops early.
pub(crate) fn run_identity_exchange(
    transport: &mut impl IdentityTransport,
    cancel: &AtomicBool,
) -> IdentityExchange {
    let mut exchange = IdentityExchange {
        pair: None,
        bridge: None,
        guitar: None,
        requests_sent: 0,
        release_attempted: false,
        reattach_attempted: false,
        failure: None,
        cleanup_failure: None,
    };
    if cancel.load(Ordering::Relaxed) {
        exchange.failure = Some(IdentityFailure::Cancelled);
    } else if let Err(failure) = transport.send_identity_request() {
        exchange.failure = Some(failure);
    } else {
        exchange.requests_sent = 1;
        match one_reply(transport) {
            Ok((bridge, _)) if bridge.role == IdentityRole::Bridge => {
                exchange.bridge = Some(bridge);
                if cancel.load(Ordering::Relaxed) {
                    exchange.failure = Some(IdentityFailure::Cancelled);
                } else if let Err(failure) = transport.send_identity_request() {
                    exchange.failure = Some(failure);
                } else {
                    exchange.requests_sent = 2;
                    match one_reply(transport) {
                        Ok((guitar, parsed)) if guitar.role == IdentityRole::Guitar => {
                            exchange.guitar = Some(guitar);
                            exchange.pair = Some(IdentityPair {
                                bridge,
                                guitar: parsed,
                            });
                        }
                        Ok(_) => exchange.failure = Some(IdentityFailure::Ambiguous),
                        Err(failure) => exchange.failure = Some(failure),
                    }
                }
            }
            Ok(_) => exchange.failure = Some(IdentityFailure::Inconsistent),
            Err(failure) => exchange.failure = Some(failure),
        }
    }
    match transport.release_and_reattach() {
        Ok(()) => {}
        Err(failure) => {
            // Keep an earlier identity stop. The cleanup stop is recorded too.
            exchange.cleanup_failure = Some(failure);
            exchange.pair = None;
            if exchange.failure.is_none() {
                exchange.failure = Some(failure);
            }
        }
    }
    exchange.release_attempted = transport.release_attempted();
    exchange.reattach_attempted = transport.reattach_attempted();
    exchange
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

    fn assert_cleaned_up(exchange: &IdentityExchange, script: &ScriptedIdentity) {
        assert!(exchange.release_attempted);
        assert!(exchange.reattach_attempted);
        assert!(script.release_attempted);
        assert!(script.reattach_attempted);
    }

    #[test]
    fn successful_exchange_sends_one_request_per_device_and_cleans_up() {
        let (exchange, script) = run(happy_script());
        assert_eq!(exchange.failure, None);
        let pair = exchange.pair.as_ref().expect("matching pair");
        assert_eq!(pair.bridge.product_id, BRIDGE_PRODUCT);
        assert_eq!(pair.bridge.version, BRIDGE_VERSION);
        assert_eq!(pair.guitar.product_id(), GUITAR_PRODUCT);
        assert_eq!(pair.guitar.version_string(), GUITAR_VERSION);
        assert_eq!(exchange.requests_sent, 2);
        assert_eq!(
            script.sent(),
            &[IDENTITY_REQUEST.to_vec(), IDENTITY_REQUEST.to_vec()]
        );
        assert_ne!(script.sent()[0], crate::midi_enable());
        assert_cleaned_up(&exchange, &script);
        assert_eq!(WRITE_TIMEOUT, Duration::from_secs(1));
        assert_eq!(WRITE_CHUNK, 16);
        assert_eq!(CHUNK_GAP, Duration::from_millis(5));
        assert_eq!(REPLY_TIMEOUT, Duration::from_secs(3));
        assert_eq!(READ_POLL, Duration::from_millis(100));
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
        assert_cleaned_up(&exchange, &script);
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
        assert_cleaned_up(&exchange, &script);
    }

    #[test]
    fn inconsistent_identity_stops() {
        let (swapped, _) = run(ScriptedIdentity::new([guitar_reply(), bridge_reply()]));
        assert_eq!(swapped.failure, Some(IdentityFailure::Inconsistent));
        assert!(swapped.pair.is_none());
        let mut wrong_version = bridge_reply();
        wrong_version[15] = 2;
        let (exchange, script) = run(ScriptedIdentity::new([wrong_version, guitar_reply()]));
        assert_eq!(exchange.failure, Some(IdentityFailure::Inconsistent));
        assert_eq!(exchange.requests_sent, 1);
        assert_cleaned_up(&exchange, &script);
    }

    #[test]
    fn timed_out_identity_stops_before_the_second_request() {
        let (exchange, script) = run(ScriptedIdentity::new([bridge_reply()]).silent());
        assert_eq!(exchange.failure, Some(IdentityFailure::TimedOut));
        assert_eq!(exchange.requests_sent, 1);
        assert_eq!(script.sent().len(), 1);
        assert_cleaned_up(&exchange, &script);
    }

    #[test]
    fn timeout_keeps_a_later_reattach_failure() {
        let (exchange, script) = run(ScriptedIdentity::new([bridge_reply()]).fail_reattach());
        assert_eq!(exchange.failure, Some(IdentityFailure::TimedOut));
        assert_eq!(
            exchange.cleanup_failure,
            Some(IdentityFailure::CleanupFailed)
        );
        assert_eq!(
            exchange.bridge.map(|identity| identity.product_id),
            Some(BRIDGE_PRODUCT)
        );
        assert!(exchange.guitar.is_none());
        assert!(exchange.pair.is_none());
        assert_eq!(exchange.requests_sent, 2);
        assert_eq!(script.sent().len(), 2);
        assert_cleaned_up(&exchange, &script);
    }

    #[test]
    fn same_role_twice_is_ambiguous_and_cleanup_failure_stops_a_match() {
        let (duplicate, _) = run(ScriptedIdentity::new([bridge_reply(), bridge_reply()]));
        assert_eq!(duplicate.failure, Some(IdentityFailure::Ambiguous));
        assert_eq!(duplicate.requests_sent, 2);
        let (reattach, script) = run(happy_script().fail_reattach());
        assert_eq!(reattach.failure, Some(IdentityFailure::CleanupFailed));
        assert!(reattach.pair.is_none());
        assert_cleaned_up(&reattach, &script);
        let (release, script) = run(happy_script().fail_release());
        assert_eq!(release.failure, Some(IdentityFailure::CleanupFailed));
        assert!(script.release_attempted && script.reattach_attempted);
    }

    #[test]
    fn cancel_before_send_sends_nothing_and_still_cleans_up() {
        let mut script = happy_script();
        let exchange = run_identity_exchange(&mut script, &AtomicBool::new(true));
        assert_eq!(exchange.failure, Some(IdentityFailure::Cancelled));
        assert!(script.sent().is_empty());
        assert_eq!(exchange.requests_sent, 0);
        assert!(exchange.release_attempted && exchange.reattach_attempted);
    }

    #[test]
    fn stage_two_stays_offline_while_g2_is_locked() {
        assert!(hardware_gates::is_locked(Gate::IdentityRequest));
        assert!(!hardware_gates::is_locked(Gate::OsDiscovery));
        assert!(!device_probe::LIVE_STAGES.contains(&Stage::Identification));
        assert!(!device_probe::live_stage_available(Stage::Identification));
    }
}

//! Read protocol shared by the locked physical adapter and offline simulators.
use crate::{
    PresetMetadata, metadata_requests,
    midi::{Identity, SysexDecoder},
    midi_enable,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const IDENTITY_REQUEST: [u8; 6] = [0xf0, 0x7e, 0x7f, 6, 1, 0xf7];
pub const MIDI_DISABLE: [u8; 9] = [0xf0, 0, 1, 12, 7, 0, 0x2a, 0, 0xf7];

/// Exact allowlist: no bank changes, preset uploads, flash/firmware operations,
/// DSP bypass, or arbitrary memory addresses can pass through this interface.
pub fn is_read_session_message(bytes: &[u8]) -> bool {
    bytes == IDENTITY_REQUEST
        || bytes == MIDI_DISABLE
        || bytes == midi_enable()
        || metadata_requests().iter().any(|request| bytes == request)
}

pub struct CurrentPreset {
    pub identity: Identity,
    pub metadata: PresetMetadata,
    pub replies: [Vec<u8>; 2],
}

pub trait ControlTransport {
    fn send(&mut self, message: &[u8]) -> Result<(), String>;
    fn next_message(
        &mut self,
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> Result<Option<Vec<u8>>, String>;
    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("guitar read cancelled".into())
    } else {
        Ok(())
    }
}

fn receive(
    transport: &mut impl ControlTransport,
    cancel: &AtomicBool,
    predicate: impl Fn(&[u8]) -> bool,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        check_cancel(cancel)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("guitar response timed out".into());
        }
        match transport.next_message(cancel, remaining)? {
            Some(message) if predicate(&message) => return Ok(message),
            Some(_) => {}
            None => return Err("guitar response timed out".into()),
        }
    }
}

pub fn read_current_on(
    transport: &mut impl ControlTransport,
    cancel: &AtomicBool,
) -> Result<CurrentPreset, String> {
    check_cancel(cancel)?;
    transport.send(&IDENTITY_REQUEST)?;
    let reply = receive(transport, cancel, |m| Identity::parse(m).is_ok())?;
    let identity = Identity::parse(&reply)?;
    if !matches!(identity.product_id(), 0x70000 | 0x70004) {
        return Err("the control interface did not identify a legacy electric Variax".into());
    }
    check_cancel(cancel)?;
    // Attempt cleanup even if enabling failed partway through a USB transfer.
    let operation = (|| {
        transport.send(&midi_enable())?;
        transport.pause(Duration::from_millis(100));
        let mut replies = Vec::new();
        for request in metadata_requests() {
            check_cancel(cancel)?;
            transport.send(&request)?;
            replies.push(receive(transport, cancel, |m| {
                crate::decode_metadata_reply(m).is_ok()
            })?);
        }
        let replies: [Vec<u8>; 2] = replies.try_into().unwrap();
        // Replies contain no address or sequence number. Equal payloads cannot
        // distinguish legitimately equal blocks from a duplicated first reply.
        // Decline that ambiguity rather than returning a falsely verified read.
        // This does not establish freshness for other stale/reordered replies.
        if crate::decode_metadata_reply(&replies[0])? == crate::decode_metadata_reply(&replies[1])?
        {
            return Err("metadata blocks are identical; unaddressed replies cannot distinguish equal data from a duplicate, so the read is unverified".into());
        }
        let metadata = PresetMetadata::from_replies(&replies[0], &replies[1])?;
        Ok(CurrentPreset {
            identity,
            metadata,
            replies,
        })
    })();
    let cleanup = transport.send(&MIDI_DISABLE);
    match (operation, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("session cleanup failed: {error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error}; session cleanup failed: {cleanup}")),
    }
}

/// Decode an offline USB packet transcript with the same framing component as
/// the physical adapter. No device or OS interface is involved.
pub fn messages_from_packets(packets: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
    let mut decoder = SysexDecoder::default();
    let mut messages = Vec::new();
    for packet in packets {
        messages.extend(decoder.feed(packet)?);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Fake<'a> {
        sent: Vec<Vec<u8>>,
        replies: VecDeque<Result<Option<Vec<u8>>, String>>,
        fail: Option<Vec<u8>>,
        cancel_after_request: Option<&'a AtomicBool>,
    }
    impl ControlTransport for Fake<'_> {
        fn send(&mut self, message: &[u8]) -> Result<(), String> {
            assert!(is_read_session_message(message));
            self.sent.push(message.to_vec());
            if message == metadata_requests()[0]
                && let Some(cancel) = self.cancel_after_request
            {
                cancel.store(true, Ordering::Relaxed);
            }
            if self.fail.as_deref() == Some(message) {
                Err("simulated write failure".into())
            } else {
                Ok(())
            }
        }
        fn next_message(&mut self, _: &AtomicBool, _: Duration) -> Result<Option<Vec<u8>>, String> {
            self.replies.pop_front().unwrap_or(Ok(None))
        }
        fn pause(&mut self, _: Duration) {}
    }
    fn identity() -> Vec<u8> {
        vec![
            0xf0, 0x7e, 0x7f, 6, 2, 0, 1, 12, 7, 0, 4, 0, b'0', b'3', b'1', b'0', 0xf7,
        ]
    }
    fn metadata(block: u8) -> Vec<u8> {
        let mut m = vec![0; 200];
        m[..7].copy_from_slice(&[0xf0, 0, 1, 12, 7, 4, 0x3c]);
        m[7] = block;
        m[199] = 0xf7;
        m
    }
    fn fake() -> Fake<'static> {
        Fake {
            sent: vec![],
            replies: [identity(), metadata(0), metadata(1)]
                .into_iter()
                .map(|m| Ok(Some(m)))
                .collect(),
            fail: None,
            cancel_after_request: None,
        }
    }
    #[test]
    fn successful_session_sends_only_exact_read_allowlist() {
        let mut f = fake();
        let cancel = AtomicBool::new(false);
        let current = read_current_on(&mut f, &cancel).unwrap();
        assert_eq!(current.identity.product_id(), 0x70004);
        assert_eq!(
            f.sent,
            [
                IDENTITY_REQUEST.to_vec(),
                midi_enable(),
                metadata_requests()[0].clone(),
                metadata_requests()[1].clone(),
                MIDI_DISABLE.to_vec()
            ]
        );
        for opcode in [0x3b, 0x43, 0x61, 0x54, 0x56, 0x4c] {
            // DSP writes, edit-buffer writes, store/restore, and preset/bank
            // operations must never enter the identity/current-read backend.
            assert!(!is_read_session_message(&[
                0xf0, 0, 1, 12, 7, 0, opcode, 0xf7
            ]));
        }
        assert!(!is_read_session_message(&crate::PODXT_ROUTING_ENABLE));
    }
    #[test]
    fn rejects_wrong_device_before_activation() {
        let mut f = fake();
        let mut wrong = identity();
        wrong[8] = 3;
        wrong[10] = 10;
        f.replies[0] = Ok(Some(wrong));
        assert!(read_current_on(&mut f, &AtomicBool::new(false)).is_err());
        assert_eq!(f.sent, [IDENTITY_REQUEST]);
    }
    #[test]
    fn timeout_disconnect_and_partial_enable_failures_attempt_cleanup() {
        for failure in ["timeout", "disconnect", "enable"] {
            let mut f = fake();
            match failure {
                "timeout" => {
                    f.replies.pop_back();
                }
                "disconnect" => {
                    f.replies[2] = Err("disconnected".into());
                }
                _ => f.fail = Some(midi_enable()),
            }
            assert!(read_current_on(&mut f, &AtomicBool::new(false)).is_err());
            assert_eq!(f.sent.last().unwrap(), &MIDI_DISABLE);
        }
    }
    #[test]
    fn cancellation_never_skips_session_cleanup() {
        let cancel = AtomicBool::new(true);
        let mut f = fake();
        assert!(read_current_on(&mut f, &cancel).is_err());
        assert!(f.sent.is_empty());
        cancel.store(false, Ordering::Relaxed);
        f.cancel_after_request = Some(&cancel);
        assert!(read_current_on(&mut f, &cancel).is_err());
        assert_eq!(f.sent.last().unwrap(), &MIDI_DISABLE);
    }
    #[test]
    fn cleanup_error_is_reported_and_noise_does_not_become_metadata() {
        let mut f = fake();
        f.replies.insert(1, Ok(Some(vec![0xf0, 1, 2, 0xf7])));
        f.fail = Some(MIDI_DISABLE.to_vec());
        let error = read_current_on(&mut f, &AtomicBool::new(false))
            .err()
            .unwrap();
        assert!(error.contains("cleanup"));
    }
    #[test]
    fn fragmented_metadata_replays_through_actual_framer() {
        let mut bytes = identity();
        bytes.extend(metadata(0));
        bytes.extend(metadata(1));
        let packets = bytes.chunks(16).map(|p| p.to_vec()).collect::<Vec<_>>();
        let messages = messages_from_packets(&packets).unwrap();
        assert_eq!(messages.len(), 3);
        crate::PresetMetadata::from_replies(&messages[1], &messages[2]).unwrap();
    }
}

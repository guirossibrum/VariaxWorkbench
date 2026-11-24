//! Single-slot G7 store, matched to Line 6 Edit 3.06's host transcript.
//! The caller must hold the rig lock and pass its own owner-approval gate.

use super::*;
use crate::midi::Event;
use crate::session::POLL;
use std::sync::atomic::{AtomicU8, Ordering};
use store_backup::{check_live_identity, utc_day};

const STORE_WAIT: Duration = Duration::from_secs(7);
const PENDING: u8 = 0;
const CANCELLED: u8 = 1;
const STARTED: u8 = 2;

/// Cancellation and dispatch share one atomic state. Once dispatch starts,
/// dropping the caller or asking to cancel cannot skip block end or readback.
#[derive(Default)]
pub struct StoreControl(AtomicU8);

impl StoreControl {
    pub fn cancel(&self) -> bool {
        self.0
            .compare_exchange(PENDING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == CANCELLED
    }

    pub fn started(&self) -> bool {
        self.0.load(Ordering::Acquire) == STARTED
    }

    fn check(&self) -> Result<(), String> {
        if self.0.load(Ordering::Acquire) == PENDING {
            Ok(())
        } else {
            Err("The PODxt store was cancelled or already dispatched.".into())
        }
    }

    fn dispatch(&self) -> Result<(), String> {
        self.0
            .compare_exchange(PENDING, STARTED, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| "The PODxt store was cancelled or already dispatched.".into())
    }
}

#[derive(Clone, Copy, Debug)]
enum StoreFrame<'a> {
    Tone(Location, &'a TonePayload),
    Commit,
}

impl Request for StoreFrame<'_> {
    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Tone(slot, tone) => tone.frame(0x71, Some(slot.address())),
            Self::Commit => vec![0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7],
        }
    }

    fn path(self) -> Path {
        Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        Operation::PodxtStoreRestore
    }
}

#[derive(Debug)]
pub enum StoreResult {
    Verified { slot: Location, tone: EditBuffer },
    SlotStateUnknown { slot: Location, reason: String },
}

/// Consume one confirmation. Err means dispatch never began. Any link error
/// at or after the 03 71 write is indeterminate, even if no bytes got through.
pub fn store_tone<L: Link>(
    session: &mut Session<L>,
    confirmation: ConfirmedStore,
    control: &StoreControl,
) -> Result<StoreResult, String> {
    store_at(session, confirmation, control, utc_day)
}

fn store_at<L: Link>(
    session: &mut Session<L>,
    confirmation: ConfirmedStore,
    control: &StoreControl,
    current_day: impl FnOnce() -> Result<u64, String>,
) -> Result<StoreResult, String> {
    let slot = confirmation.0.slot;
    if !session.allows(Operation::PodxtStoreRestore) {
        return Err("PODxt Live store is closed. No store was sent.".into());
    }
    control.check()?;
    let (_, identity) = identify_podxt(session)?;
    check_live_identity(&identity)?;
    control.check()?;
    let before = read_stored(session, slot)?;
    if TonePayload::from_dump(&before)? != confirmation.0.original {
        return Err(format!(
            "Slot {} differs from its verified backup. No store was sent.",
            slot.label()
        ));
    }
    confirmation.verify_day(current_day()?)?;
    let candidate = confirmation.0.candidate;
    control.dispatch()?;

    let outcome = (|| {
        session.send(StoreFrame::Tone(slot, &candidate))?;
        session.send(StoreFrame::Commit)?;
        wait_for_store(session)?;
        let tone = read_stored(session, slot)?;
        if TonePayload::from_dump(&tone)? != candidate {
            return Err("The stored slot did not read back byte for byte.".into());
        }
        Ok(tone)
    })();
    Ok(match outcome {
        Ok(tone) => StoreResult::Verified { slot, tone },
        Err(reason) => StoreResult::SlotStateUnknown { slot, reason },
    })
}

fn wait_for_store<L: Link>(session: &mut Session<L>) -> Result<(), String> {
    const OK: [u8; 7] = [0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7];
    const FAILURE: [u8; 7] = [0xf0, 0, 1, 0x0c, 3, 0x51, 0xf7];
    let deadline = session.link.elapsed() + STORE_WAIT;
    while let Some(left) = deadline
        .checked_sub(session.link.elapsed())
        .filter(|left| !left.is_zero())
    {
        let packet = session.poll(left.min(POLL))?;
        if session.link.elapsed() > deadline {
            break;
        }
        let mut receipt = None;
        for event in packet.into_iter().flatten() {
            if let Event::Sysex(frame) = event {
                let next = match frame.as_slice() {
                    bytes if bytes == OK => Some(Ok(())),
                    bytes if bytes == FAILURE => {
                        Some(Err("PODxt reported flash write failure (03 51).".into()))
                    }
                    _ => None,
                };
                if next.is_some() && receipt.is_some() {
                    return Err("Ambiguous PODxt store replies.".into());
                }
                receipt = receipt.or(next);
            }
        }
        if let Some(result) = receipt {
            return result;
        }
    }
    Err("No PODxt store reply (03 50/03 51) within 7 seconds.".into())
}

#[cfg(test)]
mod tests;

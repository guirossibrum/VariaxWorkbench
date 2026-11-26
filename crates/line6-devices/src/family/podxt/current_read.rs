//! Retry policy for an isolated, unaddressed PODxt Live edit-buffer read.

use super::{
    EDIT_BUFFER_LEN, EditBuffer, Link, PODXT_LIVE_MEMBER, PodxtRequest, Session, WINDOW, remaining,
};
use crate::{
    midi::{Event, hex},
    session::POLL,
};
use std::time::Duration;

const ATTEMPTS: usize = 3;
const DRAIN: Duration = Duration::from_millis(300);
pub(super) const TONE_HEADER: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x74];

/// Treatment of an event outside the expected edit-buffer reply.
pub enum ToneReadEvent {
    Ignore,
    /// The observer handled a state change; it invalidates any retry.
    StateChanged,
    Unexpected,
}

/// Read the current tone on the caller's already verified, exclusively held
/// attachment with no earlier request outstanding. A timeout or an incomplete
/// reply permits reissuing only this read, provided observed tone state is stable.
/// The caller must quarantine the attachment if `outstanding` remains set.
/// `observe` sees non-tone events in wire order, including during the drain and
/// the reply guard. The timestamp is the packet arrival time. The final argument marks events
/// after a first-attempt candidate dump.
pub fn read_verified_current_tone<L: Link>(
    session: &mut Session<L>,
    check_stop: impl Fn() -> Result<(), String>,
    deadline: Option<Duration>,
    outstanding: &mut bool,
    mut observe: impl FnMut(&Event, Duration, bool) -> Result<ToneReadEvent, String>,
) -> Result<EditBuffer, String> {
    if *outstanding || session.assembler.partial_sysex().is_some() {
        *outstanding = true;
        return Err("A previous PODxt reply may still be pending. The tone is ambiguous.".into());
    }

    let mut changed = false;
    for attempt in 1..=ATTEMPTS {
        check_stop()?;
        let now = session.link.elapsed();
        let required = if attempt == 1 {
            Duration::ZERO
        } else {
            WINDOW * 2
        };
        if deadline.is_some_and(|end| now >= end || now + required > end) {
            return Err("The PODxt tone read reached its time limit.".into());
        }
        // A failed transport write may still have reached the device.
        *outstanding = true;
        session.send(PodxtRequest::EditBuffer)?;
        let end = (session.link.elapsed() + WINDOW).min(deadline.unwrap_or(Duration::MAX));
        let mut reason = "No 168-byte edit-buffer dump arrived.".to_owned();
        let mut reply = None;
        while let Some(left) = remaining(session, end) {
            check_stop()?;
            let events = session.poll(left.min(POLL))?;
            check_stop()?;
            let mut issue = Ok(());
            for event in events.into_iter().flatten() {
                match event {
                    Event::Sysex(frame) if frame.starts_with(TONE_HEADER) => {
                        if reply.is_some() {
                            issue = Err("More than one PODxt tone reply arrived.".into());
                        } else if frame.len() == EDIT_BUFFER_LEN
                            && frame[6] == PODXT_LIVE_MEMBER as u8
                        {
                            match EditBuffer::parse(frame) {
                                Ok(tone) => reply = Some(tone),
                                Err(error) => issue = Err(error),
                            }
                        } else if frame.len() < EDIT_BUFFER_LEN
                            && (frame.len() == TONE_HEADER.len() + 1
                                || frame[6] == PODXT_LIVE_MEMBER as u8)
                        {
                            reason = format!(
                                "Incomplete PODxt tone reply has {} bytes, expected {EDIT_BUFFER_LEN}.",
                                frame.len()
                            );
                        } else {
                            issue = Err(format!("Unexpected PODxt tone reply: {}", hex(&frame)));
                        }
                    }
                    other => {
                        if let Err(error) = observe_event(
                            &other,
                            session.link.elapsed(),
                            attempt == 1 && reply.is_some(),
                            &mut observe,
                            &mut changed,
                        ) {
                            issue = Err(error);
                        }
                    }
                }
            }
            issue?;
            if changed && (attempt > 1 || reply.is_some()) {
                return Err("The PODxt tone changed while waiting. The reply is ambiguous.".into());
            }
            if attempt == 1 && reply.is_some() && session.assembler.partial_sysex().is_some() {
                // Finish a trailing notification within this attempt's original window.
                // It could also be the start of a duplicate reply.
                continue;
            }
            if let Some(tone) = reply.take() {
                if attempt > 1 {
                    session.note("Checking for an extra PODxt dump before using the retried tone.");
                    let end = session.link.elapsed() + WINDOW;
                    observe_pending(session, &check_stop, end, true, &mut observe, &mut changed)?;
                }
                if session.assembler.partial_sysex().is_some() {
                    return Err("An unfinished SysEx followed the PODxt tone reply.".into());
                }
                *outstanding = false;
                return Ok(tone);
            }
        }
        if reply.is_some() {
            return Err("An unfinished SysEx followed the PODxt tone reply.".into());
        }
        if changed {
            return Err(format!(
                "{reason} The PODxt tone changed while waiting. The read was not retried."
            ));
        }
        if attempt == ATTEMPTS {
            return Err(format!(
                "{reason} The PODxt tone read used all {ATTEMPTS} attempts."
            ));
        }
        let end = session.link.elapsed() + DRAIN;
        if deadline.is_some_and(|deadline| end + WINDOW * 2 > deadline) {
            return Err(format!(
                "{reason} The PODxt tone read reached its time limit."
            ));
        }
        session.note("Discarding pending PODxt input after an incomplete read.");
        observe_pending(session, &check_stop, end, false, &mut observe, &mut changed)?;
        // Discard only a possible tone prefix. Keep running status and any
        // identifiable unsolicited message that is still arriving.
        if session
            .assembler
            .partial_sysex()
            .is_some_and(|frame| TONE_HEADER.starts_with(frame) || frame.starts_with(TONE_HEADER))
        {
            session.assembler.discard_partial_sysex();
        }
        session.note(format!(
            "Retrying PODxt EditBuffer (attempt {}/{ATTEMPTS}): {reason}",
            attempt + 1
        ));
    }
    unreachable!("the final attempt returns")
}

fn observe_event(
    event: &Event,
    at: Duration,
    after_reply: bool,
    observe: &mut impl FnMut(&Event, Duration, bool) -> Result<ToneReadEvent, String>,
    changed: &mut bool,
) -> Result<(), String> {
    match observe(event, at, after_reply)? {
        ToneReadEvent::Ignore => Ok(()),
        ToneReadEvent::StateChanged => {
            *changed = true;
            Ok(())
        }
        ToneReadEvent::Unexpected => Err(format!(
            "Unexpected PODxt tone reply: {}",
            match event {
                Event::Sysex(bytes) | Event::Short(bytes) => hex(bytes),
            }
        )),
    }
}

fn observe_pending<L: Link>(
    session: &mut Session<L>,
    check_stop: &impl Fn() -> Result<(), String>,
    end: Duration,
    guarding: bool,
    observe: &mut impl FnMut(&Event, Duration, bool) -> Result<ToneReadEvent, String>,
    changed: &mut bool,
) -> Result<(), String> {
    while let Some(left) = remaining(session, end) {
        check_stop()?;
        let events = session.poll(left.min(POLL))?;
        check_stop()?;
        let mut issue = Ok(());
        for event in events.into_iter().flatten() {
            if matches!(&event, Event::Sysex(frame) if frame.starts_with(TONE_HEADER)) {
                if guarding {
                    issue = Err(
                        "An extra PODxt tone reply arrived after the retry. The tone is ambiguous."
                            .into(),
                    );
                }
            } else if let Err(error) =
                observe_event(&event, session.link.elapsed(), false, observe, changed)
            {
                issue = Err(error);
            }
        }
        issue?;
        if *changed {
            return Err("The PODxt tone changed during recovery. The reply is ambiguous.".into());
        }
    }
    check_stop()
}

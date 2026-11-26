//! Shared protocol operations over an already-open PODxt MIDI link.
//! The caller owns the rig lock and supplies the session's gate policy.

use super::{
    BankKind, CC_GAP, CurrentTone, EditBuffer, IDENTITY_FAMILY, LiveControl, Location,
    PODXT_LIVE_MEMBER, PODXT_LIVE_VERSION, PodxtRequest, STORED_GAP, STORED_READ_END,
    STORED_WINDOW, WINDOW, is_edit_buffer, read_current,
};
use crate::{
    identity::Identity,
    midi::{Event, hex},
    session::{Link, POLL, Session, Wait},
};
use std::time::Duration;

pub(super) fn remaining<L: Link>(session: &Session<L>, end: Duration) -> Option<Duration> {
    end.checked_sub(session.link.elapsed())
        .filter(|duration| !duration.is_zero())
}

/// Drain, then identity; errors unless the reply is a PODxt.
pub(super) fn identify_podxt<L: Link>(
    session: &mut Session<L>,
) -> Result<(Vec<u8>, Identity), String> {
    let (reply, identity) = request_identity(session)?;
    if !identity.is_line6() || identity.family != IDENTITY_FAMILY {
        return Err("The reply is not a PODxt-family identity. Nothing else was sent.".into());
    }
    Ok((reply, identity))
}

/// Verify the attached PODxt Live and its firmware before any device write.
pub fn identify_podxt_live<L: Link>(session: &mut Session<L>) -> Result<Vec<u8>, String> {
    let (reply, identity) = request_identity(session)?;
    check_live_identity(&identity)?;
    Ok(reply)
}

pub fn check_live_identity(identity: &Identity) -> Result<(), String> {
    if identity.is_line6()
        && identity.family == IDENTITY_FAMILY
        && identity.member == PODXT_LIVE_MEMBER
        && identity.version == PODXT_LIVE_VERSION
    {
        Ok(())
    } else {
        Err(format!(
            "Found firmware {} (family 0x{:04x}, member 0x{:04x}). The verified target is PODxt Live firmware 3.01.",
            identity.version_text(),
            identity.family,
            identity.member,
        ))
    }
}

fn request_identity<L: Link>(session: &mut Session<L>) -> Result<(Vec<u8>, Identity), String> {
    // Drain anything already queued. Sends nothing.
    session.listen(Duration::from_millis(200))?;
    session.send(PodxtRequest::Identity)?;
    let is_identity = |message: &[u8]| Identity::parse(message).is_some();
    let mut answer = session.wait_for(WINDOW, is_identity, true)?;
    if matches!(answer, Wait::OrphanQuiet) {
        resend_identity(session)?;
        answer = session.wait_for(WINDOW, is_identity, false)?;
    }
    let Wait::Found(reply) = answer else {
        return Err("No identity reply from the PODxt. Nothing else was sent.".into());
    };
    let identity = Identity::parse(&reply).expect("checked above");
    Ok((reply, identity))
}

pub(super) fn resend_identity<L: Link>(session: &mut Session<L>) -> Result<(), String> {
    session.note(
        "PODxt identity reply arrived without its start. Sending the identity request once more.",
    );
    session.send(PodxtRequest::Identity)
}

/// Read one addressed tone on the caller's established session.
/// A missing dump returns None after the same settle time as a successful read.
pub fn read_stored_tone<L: Link>(
    session: &mut Session<L>,
    location: Location,
) -> Result<Option<EditBuffer>, String> {
    read_stored_tone_observing(session, location, || Ok(()), None, &mut false, |_, _| false)
}

/// The same addressed exchange with cancellation and an observer for recognized
/// unsolicited events. The observer cannot claim a dump or block-end marker.
pub(super) fn read_stored_tone_observing<L: Link>(
    session: &mut Session<L>,
    location: Location,
    check_stop: impl Fn() -> Result<(), String>,
    deadline: Option<Duration>,
    outstanding: &mut bool,
    mut observe: impl FnMut(&Event, Duration) -> bool,
) -> Result<Option<EditBuffer>, String> {
    check_stop()?;
    if session.assembler.partial_sysex().is_some() {
        *outstanding = true;
        return Err("An earlier SysEx is incomplete before the stored PODxt read.".into());
    }
    if deadline.is_some_and(|end| session.link.elapsed() + STORED_GAP >= end) {
        return Err("The stored PODxt read reached its time limit.".into());
    }
    *outstanding = true;
    session.send(PodxtRequest::Stored(location))?;
    let reply_end = (session.link.elapsed() + STORED_WINDOW)
        .min(deadline.unwrap_or(Duration::MAX).saturating_sub(STORED_GAP));
    let mut end = reply_end + STORED_GAP;
    // Keep the dump and its settle period in one exchange. The packet
    // containing the dump's F7 can also contain the block-end frame.
    let mut tone = None;
    let mut saw_marker = false;
    let mut in_sysex = false;
    let orphans = session.assembler.orphans;
    loop {
        check_stop()?;
        let now = session.link.elapsed();
        if now >= end {
            break;
        }
        let next_deadline = if tone.is_none() && now < reply_end {
            reply_end
        } else {
            end
        };
        let mut interrupted = false;
        let Some(events) = session.poll_with((next_deadline - now).min(POLL), |packet| {
            // The assembler keeps partial SysEx across packets. Detect a frame
            // that is abandoned or still unfinished when this read ends.
            for status in packet.iter().copied().filter(|b| (0x80..0xf8).contains(b)) {
                interrupted |= in_sysex && status != 0xf7;
                in_sysex = status == 0xf0;
            }
            Ok(())
        })?
        else {
            continue;
        };
        check_stop()?;
        let mut issue = Ok(());
        for event in events {
            if observe(&event, session.link.elapsed()) {
                continue;
            }
            match event {
                Event::Sysex(frame)
                    if tone.is_none()
                        && session.link.elapsed() <= reply_end
                        && is_edit_buffer(&frame) =>
                {
                    match EditBuffer::parse(frame) {
                        Ok(reply) => tone = Some(reply),
                        Err(error) => issue = Err(error),
                    }
                    end = session.link.elapsed() + STORED_GAP;
                }
                Event::Sysex(frame)
                    if tone.is_some() && !saw_marker && frame == STORED_READ_END =>
                {
                    saw_marker = true;
                }
                Event::Sysex(frame) => {
                    issue = Err(format!("Unexpected SysEx reply: {}", hex(&frame)));
                }
                Event::Short(message) if message[0] & 0xf0 == 0xc0 => {
                    issue = Err(format!("Unexpected program change: {}", hex(&message)));
                }
                Event::Short(_) => {}
            }
        }
        if interrupted {
            return Err("Interrupted SysEx during a stored PODxt read.".into());
        }
        if session.assembler.orphans != orphans {
            return Err("Unexpected orphan MIDI bytes during a stored PODxt read.".into());
        }
        issue?;
    }
    if in_sysex {
        return Err("Unfinished SysEx during a stored PODxt read.".into());
    }
    Ok(tone)
}

pub(super) fn read_edit_buffer<L: Link>(session: &mut Session<L>) -> Result<EditBuffer, String> {
    request_tone(None, |request, window| wait_tone(session, request, window))?
        .ok_or_else(|| "No 168-byte edit-buffer dump arrived.".into())
}

/// Read the same tone frames through a blocking session or an editor
/// listener that also needs cancellation and unsolicited notifications.
pub(super) fn request_tone(
    location: Option<Location>,
    exchange: impl FnOnce(PodxtRequest, Duration) -> Result<Option<Vec<u8>>, String>,
) -> Result<Option<EditBuffer>, String> {
    let (request, window) = location.map_or((PodxtRequest::EditBuffer, WINDOW), |location| {
        (PodxtRequest::Stored(location), STORED_WINDOW)
    });
    exchange(request, window)?
        .map(EditBuffer::parse)
        .transpose()
}

pub(super) fn wait_tone<L: Link>(
    session: &mut Session<L>,
    request: PodxtRequest,
    window: Duration,
) -> Result<Option<Vec<u8>>, String> {
    session.send(request)?;
    match session.wait_for(window, is_edit_buffer, false)? {
        Wait::Found(reply) => Ok(Some(reply)),
        _ => Ok(None),
    }
}

/// Send one logical control in encoder order, including the gap between
/// bytes of compound values. Callers choose when to cancel and read back.
pub(super) fn send_control<L: Link>(
    session: &mut Session<L>,
    control: LiveControl,
    target: u16,
    mut sent: impl FnMut(u8, u8),
) -> Result<(), String> {
    for (cc, value, request) in control.messages(target) {
        session.send(request)?;
        sent(cc, value);
        session.link.pause(CC_GAP);
    }
    Ok(())
}

/// Select the previously recorded channel after re-verifying the 3.01
/// identity, then return its playing tone for the caller to compare.
pub fn restore_channel<L: Link>(
    session: &mut Session<L>,
    channel: Location,
) -> Result<CurrentTone, String> {
    if channel.kind() != BankKind::Tones {
        return Err("Only tone channels can be selected.".into());
    }
    identify_podxt_live(session)?;
    session.send(PodxtRequest::Program(channel))?;
    session.link.pause(Duration::from_millis(400));
    read_current(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::podxt::{ReadCommand, read_live};
    use crate::identity::tests::PODXT_LIVE;
    use crate::session::{
        Path, Quiet, Request,
        testing::{AllowAll, Fake, packets},
    };

    fn live_identity() -> Vec<u8> {
        let mut reply = PODXT_LIVE.to_vec();
        reply[12..16].copy_from_slice(&[0, 3, 0, 1]);
        reply
    }

    #[test]
    fn identity_retries_one_orphan_tail_then_permits_current_tone_read() {
        let reply = live_identity();
        let tone = super::super::tests::synthetic();
        for editor in [false, true] {
            let script = vec![
                (PodxtRequest::Identity, vec![reply[16..].to_vec()]),
                (PodxtRequest::Identity, packets(&reply)),
                (PodxtRequest::EditBuffer, packets(&tone)),
            ];
            let mut log = Quiet;
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            let observed = if editor {
                let mut current = None;
                read_live(
                    &mut session,
                    ReadCommand::Current,
                    || false,
                    |update| {
                        if let crate::family::podxt::ReadUpdate::Current(tone) = update {
                            current = Some(tone);
                        }
                    },
                )
                .unwrap();
                current.unwrap()
            } else {
                read_current(&mut session).unwrap().tone
            };
            assert_eq!(observed.raw, tone);
            session.link.assert_finished();
        }
    }

    #[test]
    fn identity_never_retries_silence_or_resends_twice() {
        for (first, second) in [(vec![], None), (vec![vec![0xf7]], Some(vec![vec![0xf7]]))] {
            for editor in [false, true] {
                let mut script = vec![(PodxtRequest::Identity, first.clone())];
                if let Some(second) = &second {
                    script.push((PodxtRequest::Identity, second.clone()));
                }
                let mut log = Quiet;
                let mut session =
                    Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
                if editor {
                    assert!(
                        read_live(&mut session, ReadCommand::Current, || false, |_| {}).is_err()
                    );
                } else {
                    assert!(read_current(&mut session).is_err());
                }
                let identity = PodxtRequest::Identity.bytes();
                assert_eq!(
                    session.link.writes,
                    vec![identity; 1 + usize::from(second.is_some())]
                );
                session.link.assert_finished();
            }
        }
    }

    #[test]
    fn addressed_end_marker_is_optional_and_does_not_change_read_budgets() {
        let slot = Location::tone_from_label("1B").unwrap();
        let dump = super::super::tests::synthetic();
        for (reply, expected_elapsed) in [
            (vec![], STORED_WINDOW + STORED_GAP),
            (vec![dump.clone()], Duration::from_millis(4) + STORED_GAP),
            (
                vec![dump.clone(), STORED_READ_END.to_vec()],
                Duration::from_millis(4) + STORED_GAP,
            ),
        ] {
            let has_dump = !reply.is_empty();
            let mut log = Quiet;
            let mut session = Session::new(
                Fake::new(Path::PodxtMidi, vec![(PodxtRequest::Stored(slot), reply)]),
                &AllowAll,
                &mut log,
            );
            let actual = read_stored_tone(&mut session, slot).unwrap();
            assert_eq!(
                actual.as_ref().map(|tone| &tone.raw),
                has_dump.then_some(&dump)
            );
            assert_eq!(session.link.elapsed(), expected_elapsed);
            session.link.assert_finished();
        }
    }

    #[test]
    fn addressed_read_rejects_malformed_or_unrelated_frames() {
        let slot = Location::tone_from_label("1B").unwrap();
        let dump = super::super::tests::synthetic();
        let marker = STORED_READ_END.to_vec();
        let marker_prefix = marker[..marker.len() - 1].to_vec();
        let malformed_marker = [marker_prefix.clone(), vec![0x01, 0xf7]].concat();
        let unrelated = vec![0xf0, 0, 1, 0x0c, 3, 0x50, 0xf7];
        for (reply, error) in [
            (vec![marker.clone()], "Unexpected SysEx"),
            (
                vec![[marker.clone(), dump.clone()].concat()],
                "Unexpected SysEx",
            ),
            (
                vec![[dump.clone(), malformed_marker.clone()].concat()],
                "Unexpected SysEx",
            ),
            (vec![dump.clone(), malformed_marker], "Unexpected SysEx"),
            (vec![dump.clone(), unrelated], "Unexpected SysEx"),
            (
                vec![dump.clone(), marker.clone(), marker],
                "Unexpected SysEx",
            ),
            (
                vec![dump.clone(), marker_prefix.clone()],
                "Unfinished SysEx",
            ),
            (
                vec![[dump.clone(), marker_prefix].concat(), dump.clone()],
                "Interrupted SysEx",
            ),
            (
                vec![[dump.clone(), vec![0xf7]].concat()],
                "Unexpected orphan",
            ),
            (vec![dump, vec![0xc0, 2]], "Unexpected program change"),
        ] {
            let mut log = Quiet;
            let mut session = Session::new(
                Fake::new(Path::PodxtMidi, vec![(PodxtRequest::Stored(slot), reply)]),
                &AllowAll,
                &mut log,
            );
            let observed = read_stored_tone(&mut session, slot).unwrap_err();
            assert!(observed.contains(error), "{observed}");
            session.link.assert_finished();
        }
    }

    #[test]
    fn strict_current_tone_read_does_not_accept_the_addressed_end_marker() {
        let dump = [super::super::tests::synthetic(), STORED_READ_END.to_vec()].concat();
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::new(
                Path::PodxtMidi,
                vec![(PodxtRequest::EditBuffer, vec![dump])],
            ),
            &AllowAll,
            &mut log,
        );
        session.require_expected_sysex();
        assert!(
            read_edit_buffer(&mut session)
                .unwrap_err()
                .contains("Unexpected SysEx")
        );
        session.link.assert_finished();
    }
}

//! Read-only compatibility evidence from a PODxt Live reporting any valid version.

use super::{
    BankKind, EditBuffer, IDENTITY_FAMILY, Location, PODXT_LIVE_MEMBER, STORED_GAP,
    STORED_READ_END, TonePayload,
    procedures::{identify_podxt, request_tone},
};
use crate::{
    gate::Operation,
    identity::Identity,
    midi::Event,
    session::{Link, Session, Wait},
};

/// Separate owner approval for collecting PODxt firmware evidence.
pub const APPROVED_FIRMWARE_READ: bool = false;

/// The expected reply structure or library position for a raw frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reply {
    Identity,
    Current,
    Stored(Location),
    Closing,
}

fn is_live(identity: &Identity) -> bool {
    identity.is_line6()
        && identity.family == IDENTITY_FAMILY
        && identity.member == PODXT_LIVE_MEMBER
        && identity.version.iter().all(|&digit| digit <= 9)
}

/// Rig Check step 2.7 compares its captured identity offline. This separate
/// API is available to diagnostic callers. The caller holds the rig lock
/// and provides identity, current-tone and stored-read permissions. Each
/// reply is reported before the next request.
pub fn read<L: Link>(
    session: &mut Session<L>,
    stopping: impl Fn() -> bool,
    received: impl FnMut(Reply, &[u8]),
) -> Result<(), String> {
    if !APPROVED_FIRMWARE_READ {
        return Err("The PODxt firmware compatibility read has no owner approval.".into());
    }
    read_on(session, stopping, received)
}

fn read_on<L: Link>(
    session: &mut Session<L>,
    stopping: impl Fn() -> bool,
    mut received: impl FnMut(Reply, &[u8]),
) -> Result<(), String> {
    if [
        Operation::PodxtIdentity,
        Operation::PodxtRead,
        Operation::PodxtLibraryRead,
    ]
    .into_iter()
    .any(|operation| !session.allows(operation))
    {
        return Err(
            "PODxt firmware evidence requires identity and current and stored read permissions."
                .into(),
        );
    }

    check_stop(&stopping)?;
    session.require_expected_sysex();
    let (reply, identity) = identify_podxt(session)?;
    received(Reply::Identity, &reply);
    if !is_live(&identity) {
        return Err(
            "Firmware evidence requires a PODxt Live identity with four numeric version bytes."
                .into(),
        );
    }

    capture_tone(session, Reply::Current, &stopping, &mut received)?;
    for location in Location::all(&BankKind::ALL) {
        capture_tone(session, Reply::Stored(location), &stopping, &mut received)?;
        settle_stored_read(session, &stopping)?;
    }
    capture_tone(session, Reply::Closing, &stopping, &mut received)
}

fn capture_tone<L: Link>(
    session: &mut Session<L>,
    kind: Reply,
    stopping: &impl Fn() -> bool,
    received: &mut impl FnMut(Reply, &[u8]),
) -> Result<(), String> {
    check_stop(stopping)?;
    let location = match kind {
        Reply::Stored(location) => Some(location),
        _ => None,
    };
    let tone: EditBuffer = request_tone(location, |request, window| {
        session.send(request)?;
        let Wait::Found(raw) = session.wait_for(window, |_| true, false)? else {
            return Err(format!("No PODxt firmware evidence reply for {kind:?}."));
        };
        received(kind, &raw);
        Ok(Some(raw))
    })?
    .expect("evidence exchange reports a missing reply as an error");
    TonePayload::from_dump(&tone)?;
    check_stop(stopping)
}

fn settle_stored_read<L: Link>(
    session: &mut Session<L>,
    stopping: &impl Fn() -> bool,
) -> Result<(), String> {
    // The verified stored read allows 100 ms for the optional block-end.
    let end = session.link.elapsed() + STORED_GAP;
    let orphans = session.assembler.orphans;
    while let Some(left) = end
        .checked_sub(session.link.elapsed())
        .filter(|duration| !duration.is_zero())
    {
        check_stop(stopping)?;
        for event in session.poll(left)?.into_iter().flatten() {
            match event {
                Event::Sysex(frame) if frame != STORED_READ_END => {
                    return Err("Unexpected SysEx after a stored PODxt tone.".into());
                }
                Event::Short(message) if message[0] & 0xf0 == 0xc0 => {
                    return Err(
                        "The playing channel changed during PODxt firmware evidence.".into(),
                    );
                }
                _ => {}
            }
        }
        if session.assembler.orphans != orphans {
            return Err("Unexpected orphan bytes after a stored PODxt tone.".into());
        }
    }
    Ok(())
}

fn check_stop(stopping: &impl Fn() -> bool) -> Result<(), String> {
    if stopping() {
        Err("PODxt firmware evidence cancelled.".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

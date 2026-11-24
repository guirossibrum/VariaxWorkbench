//! Read-only live input from a Variax 300/600 through the PODxt Live bridge.

use std::time::Duration;

use super::{Preset, identify_variax, is_reply, with_bridged_read_session};
use crate::{
    device_lock::DeviceLock,
    gate::Operation,
    midi::Event,
    session::{Link, POLL, Path, Session},
};

/// A device event delivered in MIDI arrival order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FollowUpdate {
    /// Live CC 7 value (0–127).
    Volume(u8),
    /// Live CC 79 value (0–127). This carries no saved-tone mode change.
    TonePosition(u8),
    /// The bank and selector position decoded from the incoming program.
    ProgramChange(Preset),
    Clip,
    DspOverload,
    MidiQueueOverflow,
}

/// Follow the verified Variax 300/600 firmware 3.10 on the open guitar link.
///
/// Acquire the rig lock before opening the verified PODxt Live attachment
/// and retain it until the caller releases the USB interface. The session policy must grant Variax identity
/// and G3 read operations. Run this blocking call on a worker. The callback
/// receives unsolicited updates only, without an initial preset snapshot.
/// Program changes are labeled from the Workbench mapping and need no read.
///
/// Cancellation returns normally after successful cleanup. A transport or
/// cleanup failure is returned to the caller. The session-off and bridge-off
/// sends are both attempted after the listening session starts.
pub fn follow<L: Link>(
    _rig: &DeviceLock,
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    publish: impl FnMut(FollowUpdate),
) -> Result<(), String> {
    follow_on(session, cancelled, publish)
}

fn follow_on<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(FollowUpdate),
) -> Result<(), String> {
    if session.link.path() != Path::VariaxViaPodxt
        || !session.allows(Operation::VariaxIdentity)
        || !session.allows(Operation::VariaxRead)
    {
        return Err(
            "Variax Follow requires the PODxt Live bridge and Variax identity and read gates."
                .into(),
        );
    }
    if cancelled() {
        return Ok(());
    }
    let identity = identify_variax(session)?.identity;
    if identity.member != 4 || identity.version != *b"0310" {
        return Err("The open link did not verify as Variax 300/600 firmware 3.10.".into());
    }
    if cancelled() {
        return Ok(());
    }

    with_bridged_read_session(session, Duration::ZERO, |session| {
        while !cancelled() {
            for event in session.poll(POLL)?.into_iter().flatten() {
                if cancelled() {
                    return Ok(());
                }
                if let Some(update) = decode(&event) {
                    publish(update);
                }
            }
        }
        Ok(())
    })
}

fn decode(event: &Event) -> Option<FollowUpdate> {
    match event {
        Event::Short(message) => match message.as_slice() {
            [status, 7, value] if status & 0xf0 == 0xb0 => Some(FollowUpdate::Volume(*value)),
            [status, 79, value] if status & 0xf0 == 0xb0 => {
                Some(FollowUpdate::TonePosition(*value))
            }
            [status, program] if status & 0xf0 == 0xc0 => Preset::all()
                .find(|preset| preset.program_number() == *program)
                .map(FollowUpdate::ProgramChange),
            _ => None,
        },
        Event::Sysex(frame) if is_reply(frame, 0x28) => Some(FollowUpdate::Clip),
        Event::Sysex(frame) if is_reply(frame, 0x4d) => Some(FollowUpdate::DspOverload),
        Event::Sysex(frame) if is_reply(frame, 0x74) => Some(FollowUpdate::MidiQueueOverflow),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

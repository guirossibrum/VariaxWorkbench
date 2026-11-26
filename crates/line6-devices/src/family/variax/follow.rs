//! Read-only live input from a Variax 300/600 through the PODxt Live bridge.

use std::time::Duration;

use super::{Preset, identify_variax, is_reply, with_bridged_read_session};
use crate::{
    device_lock::DeviceLock,
    gate::Operation,
    midi::{Event, hex},
    session::{Link, POLL, Path, Session},
};

/// A device event delivered in MIDI arrival order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FollowUpdate {
    /// Live CC 7 value (0–127).
    Volume(u8),
    /// Live CC 79 value (0–127). This carries no saved-tone mode change.
    TonePosition(u8),
    /// Live CC 84 tuning override. This carries no preset change.
    TuningOverride(bool),
    /// The bank and selector position decoded from the incoming program.
    ProgramChange(Preset),
    Clip,
    DspOverload,
    MidiQueueOverflow,
    /// The first nonzero connection state reported during this run.
    Connected,
    /// The PODxt reported that the guitar disconnected. Follow ends after cleanup.
    Disconnected,
}

/// Follow the verified Variax 300/600 firmware 3.10 on the open guitar link.
///
/// Acquire the rig lock before opening the verified PODxt Live attachment
/// and retain it until the caller releases the USB interface. The session policy must grant Variax identity
/// and G3 read operations. Run this blocking call on a worker. The callback
/// receives unsolicited updates only, without an initial preset snapshot.
/// Program changes are labeled from the Workbench mapping and need no read.
///
/// Cancellation and a reported guitar disconnect return normally after
/// successful cleanup. A transport or cleanup failure is returned to the caller. The session-off and bridge-off
/// sends are both attempted after the listening session starts.
pub fn follow<L: Link>(
    _rig: &DeviceLock,
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    publish: impl FnMut(FollowUpdate),
) -> Result<(), String> {
    follow_on(session, None, cancelled, publish)
}

fn follow_on<L: Link>(
    session: &mut Session<L>,
    window: Option<Duration>,
    cancelled: impl Fn() -> bool,
    publish: impl FnMut(FollowUpdate),
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

    listen_updates(session, window, cancelled, publish)
}

/// Bounded Follow for an already locked diagnostic link. While waiting for
/// a replug the prior identity is required from the caller, since the guitar
/// cannot answer yet. Reverify that identity once it reconnects.
pub fn follow_window<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    waiting_for_replug: bool,
    cancelled: impl Fn() -> bool,
    publish: impl FnMut(FollowUpdate),
) -> Result<(), String> {
    if waiting_for_replug {
        listen_updates(session, Some(window), cancelled, publish)
    } else {
        follow_on(session, Some(window), cancelled, publish)
    }
}

fn listen_updates<L: Link>(
    session: &mut Session<L>,
    window: Option<Duration>,
    cancelled: impl Fn() -> bool,
    mut publish: impl FnMut(FollowUpdate),
) -> Result<(), String> {
    with_bridged_read_session(session, Duration::ZERO, |session| {
        let mut reported_connected = false;
        let start = session.link.elapsed();
        session.note("Listening window open for Variax Follow.");
        while !cancelled()
            && window.is_none_or(|window| session.link.elapsed().saturating_sub(start) < window)
        {
            for event in session.poll(POLL)?.into_iter().flatten() {
                if cancelled() {
                    return Ok(());
                }
                match decode(&event) {
                    Some(FollowUpdate::Connected) if reported_connected => {}
                    Some(update) => {
                        publish(update);
                        match update {
                            FollowUpdate::Connected => reported_connected = true,
                            FollowUpdate::Disconnected => return Ok(()),
                            _ => {}
                        }
                    }
                    None => {
                        let (kind, frame) = match &event {
                            Event::Sysex(frame) => ("SysEx", frame),
                            Event::Short(frame) => ("MIDI", frame),
                        };
                        session.note(format!(
                            "Ignored unrecognized Follow {kind} frame: {}",
                            hex(frame)
                        ));
                    }
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
            [status, 84, value] if status & 0xf0 == 0xb0 => {
                Some(FollowUpdate::TuningOverride(*value != 0))
            }
            [status, program] if status & 0xf0 == 0xc0 => Preset::all()
                .find(|preset| preset.program_number() == *program)
                .map(FollowUpdate::ProgramChange),
            _ => None,
        },
        Event::Sysex(frame) if is_reply(frame, 0x28) => Some(FollowUpdate::Clip),
        Event::Sysex(frame) if is_reply(frame, 0x4d) => Some(FollowUpdate::DspOverload),
        Event::Sysex(frame) if is_reply(frame, 0x74) => Some(FollowUpdate::MidiQueueOverflow),
        Event::Sysex(frame) => match frame.as_slice() {
            // Workbench 1.75 L6VariaxPatchManager.OnSysex. Keep the length
            // checks implicit in these complete Line 6 SysEx patterns.
            [0xf0, 0, 1, 0x0c, 0x03, 0x11, _, _, _, state, 0xf7] => Some(if *state == 0 {
                FollowUpdate::Disconnected
            } else {
                FollowUpdate::Connected
            }),
            [0xf0, 0, 1, 0x0c, 0x7f, 0x03, _, _, _, _, _, 0xf7] => Some(FollowUpdate::Disconnected),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests;

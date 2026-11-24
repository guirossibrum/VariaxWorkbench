//! Variax 600 firmware 3.10 slot-read protocol from the #181 static trace.
//! The caller supplies the gated session and an already-open bridge link.

use super::{HEADER, SESSION_SETTLE, VariaxRequest, bridge_message, is_reply};
use crate::{
    gate::Operation,
    midi::Event,
    session::{Link, POLL, Path, Request, Session},
};
use std::time::Duration;

const PROGRAM_BYTES: usize = 3_276;
const REPLY_BYTES: usize = PROGRAM_BYTES * 2 + 9;
const REPLY_WINDOW: Duration = Duration::from_secs(10);

/// MIDI indices are seven-bit values. 0x7F selects the active RAM buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotIndex(u8);

impl SlotIndex {
    pub const CURRENT: Self = Self(0x7f);

    pub const fn new(index: u8) -> Option<Self> {
        if index <= 0x7f {
            Some(Self(index))
        } else {
            None
        }
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Read one index with opcode 0x44. Opcode 0x45 is accepted only as input.
#[derive(Clone, Copy, Debug)]
pub struct SlotRead(pub SlotIndex);

impl Request for SlotRead {
    fn bytes(self) -> Vec<u8> {
        HEADER
            .into_iter()
            .chain([0, 0x44, self.0.0, 0xf7])
            .collect()
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        Operation::VariaxSlotRead
    }
}

#[derive(Debug)]
pub struct SlotReply {
    pub index: SlotIndex,
    pub program: [u8; PROGRAM_BYTES],
    pub checksum: Checksum,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Checksum {
    /// Low 24 bits of the sum of the first 1,091 words.
    pub computed: u32,
    pub stored: u32,
}

impl Checksum {
    pub fn matches(&self) -> bool {
        self.computed == self.stored
    }
}

impl SlotReply {
    pub fn decode(index: SlotIndex, reply: &[u8]) -> Result<Self, String> {
        if !is_reply(reply, 0x45) {
            return Err("Invalid Variax slot reply envelope or missing F7.".into());
        }
        if reply.len() != REPLY_BYTES {
            return Err(format!(
                "Variax slot reply has {} bytes, expected {REPLY_BYTES}.",
                reply.len()
            ));
        }
        if reply[7] != index.0 {
            return Err(format!(
                "Variax slot reply echoed {:02X}, expected {:02X}.",
                reply[7], index.0
            ));
        }
        let payload = &reply[8..REPLY_BYTES - 1];
        if payload.iter().any(|&nibble| nibble > 0x0f) {
            return Err("Variax slot reply contains a non-nibble payload byte.".into());
        }
        let mut program = [0; PROGRAM_BYTES];
        for (packed, &[lo0, hi0, lo1, hi1, lo2, hi2]) in program
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(payload.as_chunks::<6>().0)
        {
            // RAM emits low-first nibbles of one word. Flash emits low/high
            // pairs of consecutive little-endian bytes. Both yield the same
            // canonical big-endian word after reversing the three byte pairs.
            *packed = [hi2 << 4 | lo2, hi1 << 4 | lo1, hi0 << 4 | lo0];
        }
        let word = |[a, b, c]: [u8; 3]| u32::from_be_bytes([0, a, b, c]);
        let (stored, data) = program
            .as_chunks::<3>()
            .0
            .split_last()
            .expect("program words");
        let computed = data
            .iter()
            .fold(0u32, |sum, &bytes| sum.wrapping_add(word(bytes)))
            & 0xff_ffff;
        Ok(Self {
            index,
            program,
            checksum: Checksum {
                computed,
                stored: word(*stored),
            },
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Control {
    Bridge(bool),
    Session(bool),
}

impl Request for Control {
    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Bridge(enabled) => bridge_message(enabled),
            Self::Session(true) => VariaxRequest::SessionOn.bytes(),
            Self::Session(false) => VariaxRequest::SessionOff.bytes(),
        }
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        Operation::VariaxSlotRead
    }
}

/// Read indices in order in one session. Each decoded reply is handed to
/// `received` before the next request, so a partial capture can be preserved.
pub fn read_slots<L: Link>(
    session: &mut Session<L>,
    indices: &[SlotIndex],
    mut stopping: impl FnMut() -> bool,
    mut received: impl FnMut(SlotReply) -> Result<(), String>,
) -> Result<(), String> {
    if !session.allows(Operation::VariaxSlotRead) || session.link.path() != Path::VariaxViaPodxt {
        return Err("Slot read requires the PODxt Live bridge and VariaxSlotRead.".into());
    }
    if indices.is_empty() {
        return Ok(());
    }
    check_stopping(&mut stopping)?;
    let result = session.send(Control::Bridge(true)).and_then(|()| {
        check_stopping(&mut stopping)?;
        let result = session.send(Control::Session(true)).and_then(|()| {
            session.link.pause(SESSION_SETTLE);
            for &index in indices {
                check_stopping(&mut stopping)?;
                let reply = read_slot(session, index, &mut stopping)
                    .map_err(|error| format!("Index 0x{:02X}: {error}", index.0))?;
                received(reply)?;
            }
            check_stopping(&mut stopping)
        });
        with_cleanup(result, session.send(Control::Session(false)), "Session off")
    });
    with_cleanup(
        result,
        session.send(Control::Bridge(false)),
        "PODxt release",
    )
}

fn check_stopping(stopping: &mut impl FnMut() -> bool) -> Result<(), String> {
    if stopping() {
        return Err("Variax slot read cancelled or log unavailable.".into());
    }
    Ok(())
}

fn with_cleanup(
    result: Result<(), String>,
    cleanup: Result<(), String>,
    action: &str,
) -> Result<(), String> {
    match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(format!("{action} failed: {error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error} {action} also failed: {cleanup}")),
    }
}

fn read_slot<L: Link>(
    session: &mut Session<L>,
    index: SlotIndex,
    stopping: &mut impl FnMut() -> bool,
) -> Result<SlotReply, String> {
    session.send(SlotRead(index))?;
    let deadline = session.link.elapsed() + REPLY_WINDOW;
    let orphans = session.assembler.orphans;
    loop {
        check_stopping(stopping)?;
        let remaining = deadline.saturating_sub(session.link.elapsed());
        if remaining.is_zero() {
            return Err("Timed out waiting for a complete 0x45 reply ending in F7.".into());
        }
        let Some(events) = session.poll_with(remaining.min(POLL), |packet| {
            if packet.iter().any(|byte| matches!(byte, 0xf1..=0xf6)) {
                return Err("Unexpected system-common byte during slot read.".into());
            }
            Ok(())
        })?
        else {
            continue;
        };
        if session.assembler.orphans != orphans {
            return Err("Unexpected orphan MIDI bytes during slot read.".into());
        }
        let mut decoded = None;
        for event in events {
            match event {
                Event::Sysex(message)
                    if [0x28, 0x4d, 0x74].iter().any(|&op| is_reply(&message, op)) =>
                {
                    session.note(format!("Skipped Variax notice 0x{:02X}.", message[6]));
                }
                Event::Sysex(message) if decoded.is_none() => {
                    decoded = Some(SlotReply::decode(index, &message)?);
                }
                Event::Short(message) if message[0] & 0xf0 == 0xb0 => {
                    // The guitar may emit volume/tone CCs while the session is on.
                }
                _ => return Err("Unexpected MIDI message during Variax slot read.".into()),
            }
        }
        if let Some(reply) = decoded {
            return Ok(reply);
        }
    }
}

#[cfg(test)]
mod tests;

//! Workbench 1.75 write messages and transactions for the Variax 300/600.
//!
//! The caller supplies an already-open link and the G6/G7 policy. This module
//! has no device discovery, USB open, or application entry point.

use super::{HEADER, Preset, SESSION_SETTLE, VariaxRequest};
use crate::gate::Operation;
use crate::session::{Link, Path, Request, Session, Wait};
use std::time::Duration;

const WORDS: usize = 1092;
const NAME_SIZE: usize = 18;
const NAME_OFFSET: usize = 2553;
const BRIDGE_CHUNK: usize = 256;
const STORE_SETTLE: Duration = Duration::from_millis(100);
const STORE_ACK: Duration = Duration::from_secs(15);
const BANK_SELECT_SETTLE: Duration = Duration::from_millis(200);
const BANK_WRITE_SETTLE: Duration = Duration::from_millis(700);
const BANK_REPLY: Duration = Duration::from_secs(1);

/// A compiled program in original Workbench byte order, with its source name.
/// The source name is required because the compiler can truncate it.
pub struct Program<'a>(&'a [u8; WORDS * 3]);

impl<'a> Program<'a> {
    pub fn new(name: &str, packed: &'a [u8; WORDS * 3]) -> Result<Self, String> {
        let expected = name_field(name, b' ')?;
        if packed[NAME_OFFSET..NAME_OFFSET + NAME_SIZE] != expected {
            return Err("The compiled Variax program contains a different model name.".into());
        }
        let program = Self(packed);
        let checksum = (0..1089).map(|i| program.word(i)).sum::<u64>() & 0xffffff;
        if checksum != program.word(WORDS - 1) {
            return Err("The compiled Variax program has an invalid checksum.".into());
        }
        Ok(program)
    }

    fn word(&self, index: usize) -> u64 {
        let bytes = &self.0[index * 3..][..3];
        u64::from(u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]))
    }
}

fn name_field(name: &str, padding: u8) -> Result<[u8; NAME_SIZE], String> {
    if name.len() > NAME_SIZE || name.bytes().any(|byte| !matches!(byte, 0x20..=0x7e)) {
        return Err("Variax names must contain at most 18 printable ASCII characters.".into());
    }
    let mut bytes = [padding; NAME_SIZE];
    bytes[..name.len()].copy_from_slice(name.as_bytes());
    Ok(bytes)
}

fn push_word(bytes: &mut Vec<u8>, word: u64) {
    bytes.extend((0..6).map(|n| ((word >> (n * 4)) & 0xf) as u8));
}

fn sysex(command: u8, body: impl IntoIterator<Item = u8>) -> Vec<u8> {
    HEADER
        .into_iter()
        .chain([0, command])
        .chain(body)
        .chain([0xf7])
        .collect()
}

fn full(program: &Program<'_>) -> Vec<u8> {
    let mut payload = Vec::with_capacity((WORDS + 1) * 6);
    push_word(&mut payload, WORDS as u64);
    for index in 0..WORDS {
        push_word(&mut payload, program.word(index));
    }
    sysex(0x43, payload)
}

fn difference(previous: Option<&Program<'_>>, next: &Program<'_>) -> Option<Vec<u8>> {
    let mut payload = Vec::new();
    let limit = WORDS - 1;
    let changed = |index| previous.is_none_or(|previous| previous.word(index) != next.word(index));
    let mut index = 0;
    while index < limit {
        if !changed(index) {
            index += 1;
            continue;
        }
        let start = index;
        while index < limit && changed(index) {
            index += 1;
        }
        let count = index - start;
        // The original flushes a terminal run with its fields reversed.
        let (first, second) = if index == limit {
            (start, count)
        } else {
            (count, start)
        };
        push_word(&mut payload, first as u64);
        push_word(&mut payload, second as u64);
        for word in start..index {
            push_word(&mut payload, next.word(word));
        }
    }
    (!payload.is_empty()).then(|| sysex(0x3b, payload))
}

#[derive(Clone, Copy, Debug)]
enum Message<'a> {
    Bridge(bool),
    Session(bool),
    DspEnabled(bool),
    Full(&'a [u8]),
    Difference(&'a [u8]),
    Tone(u8),
    Store(Preset),
    SelectBank(Preset),
    BankName(&'a [u8; NAME_SIZE]),
    ReadBankName,
}

#[derive(Clone, Copy, Debug)]
struct WriteRequest<'a>(Operation, Message<'a>);

impl Request for WriteRequest<'_> {
    fn bytes(self) -> Vec<u8> {
        match self.1 {
            Message::Bridge(on) => vec![0xf0, 0, 1, 0x0c, 3, 0x23, u8::from(on), 0xf7],
            Message::Session(true) => VariaxRequest::SessionOn.bytes(),
            Message::Session(false) => VariaxRequest::SessionOff.bytes(),
            Message::DspEnabled(on) => sysex(0x4c, [u8::from(!on)]),
            Message::Full(bytes) | Message::Difference(bytes) => bytes.to_vec(),
            Message::Tone(value) => vec![0xb0, 79, value],
            Message::Store(preset) => sysex(0x61, [preset.bank() * 5 + (4 - preset.slot())]),
            Message::SelectBank(preset) => vec![0xc0, preset.program_number()],
            Message::BankName(name) => sysex(0x6c, *name),
            Message::ReadBankName => sysex(0x6d, []),
        }
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        self.0
    }
}

impl WriteRequest<'_> {
    fn send<L: Link>(self, session: &mut Session<L>) -> Result<(), String> {
        self.send_until(session, || false)
    }

    fn send_until<L: Link>(
        self,
        session: &mut Session<L>,
        stopping: impl FnMut() -> bool,
    ) -> Result<(), String> {
        match self.1 {
            Message::Bridge(_) | Message::Tone(_) | Message::SelectBank(_) => session.send(self),
            Message::Full(_) => session.send_chunked_sysex(self, BRIDGE_CHUNK, None, stopping),
            _ => session.send_chunked_sysex(self, BRIDGE_CHUNK, Some(HEADER.len() + 1), stopping),
        }
    }
}

const G6: Operation = Operation::VariaxEditBufferWrite;
const G7: Operation = Operation::VariaxStoreRestore;

fn transaction<L: Link, T>(
    session: &mut Session<L>,
    operation: Operation,
    bypass_dsp: bool,
    body: impl FnOnce(&mut Session<L>) -> Result<T, String>,
) -> Result<T, String> {
    if session.link.path() != Path::VariaxViaPodxt || !session.allows(operation) {
        return Err(format!(
            "Variax transaction requires the PODxt Live bridge and {operation:?}."
        ));
    }
    let send = |session: &mut Session<L>, message| WriteRequest(operation, message).send(session);
    let result = (|| {
        send(session, Message::Bridge(true))?;
        send(session, Message::Session(true))?;
        session.link.pause(SESSION_SETTLE);
        if bypass_dsp {
            send(session, Message::DspEnabled(false))?;
        }
        body(session)
    })();
    // Attempt every cleanup command even when a prior write failed partway.
    let cleanup: Vec<_> = [
        Message::DspEnabled(true),
        Message::Session(false),
        Message::Bridge(false),
    ]
    .into_iter()
    .filter_map(|message| {
        send(session, message)
            .err()
            .map(|error| format!("{message:?}: {error}"))
    })
    .collect();
    if cleanup.is_empty() {
        return result;
    }
    let cleanup = format!("Variax cleanup failed ({})", cleanup.join(", "));
    match result {
        Ok(_) => Err(cleanup),
        Err(error) => Err(format!("{error} {cleanup}")),
    }
}

/// A G6 edit within one open Workbench-style session.
pub enum Edit<'a> {
    Full(&'a Program<'a>),
    Difference {
        previous: Option<&'a Program<'a>>,
        next: &'a Program<'a>,
    },
    /// The original generates the diff target with its tuning-transition flag
    /// set, then regenerates the full program with the flag cleared.
    AlternateTuning {
        previous: Option<&'a Program<'a>>,
        transition: &'a Program<'a>,
        full: &'a Program<'a>,
    },
    Tone(f32),
}

/// Send G6 edits in order while ordinary live-edit DSP processing stays active.
pub fn edit<L: Link>(
    session: &mut Session<L>,
    edits: &[Edit<'_>],
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    if edits
        .iter()
        .any(|edit| matches!(edit, Edit::Tone(value) if !(0.0..=1.0).contains(value)))
    {
        return Err("The Variax tone knob position must be between 0 and 1.".into());
    }
    transaction(session, G6, false, |session| {
        for edit in edits {
            if stopping() {
                return Err("Variax transfer stopped.".into());
            }
            match edit {
                Edit::Full(program) => WriteRequest(G6, Message::Full(&full(program)))
                    .send_until(session, &mut stopping)?,
                Edit::Difference { previous, next } => {
                    if let Some(bytes) = difference(*previous, next) {
                        WriteRequest(G6, Message::Difference(&bytes))
                            .send_until(session, &mut stopping)?;
                    }
                }
                Edit::AlternateTuning {
                    previous,
                    transition,
                    full: program,
                } => {
                    if let Some(bytes) = difference(*previous, transition) {
                        WriteRequest(G6, Message::Difference(&bytes))
                            .send_until(session, &mut stopping)?;
                    }
                    WriteRequest(G6, Message::Full(&full(program)))
                        .send_until(session, &mut stopping)?;
                }
                Edit::Tone(value) => {
                    WriteRequest(G6, Message::Tone((127.0 * value) as u8)).send(session)?
                }
            }
        }
        Ok(())
    })
}

/// Store one slot. The preparatory edit buffer and cleanup belong to G7.
/// After sending the store request, always finish waiting for its confirmation.
pub fn store<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    if stopping() {
        return Err("Variax transfer stopped.".into());
    }
    transaction(session, G7, true, |session| {
        WriteRequest(G7, Message::Full(&full(program))).send_until(session, &mut stopping)?;
        session.link.pause(STORE_SETTLE);
        if stopping() {
            return Err("Variax transfer stopped before the slot store.".into());
        }
        let unknown = |error| format!("{error} Slot state is unknown.");
        WriteRequest(G7, Message::Store(preset))
            .send(session)
            .map_err(unknown)?;
        match session
            .wait_for(STORE_ACK, |_| true, false)
            .map_err(unknown)?
        {
            Wait::Found(reply) if reply.len() == 8 && is_reply(&reply, 0x71) => Ok(()),
            Wait::Found(_) => Err("Unexpected Variax store reply. Slot state is unknown.".into()),
            _ => Err("Timed out waiting for the Variax store reply. Slot state is unknown.".into()),
        }
    })
}

fn is_reply(message: &[u8], command: u8) -> bool {
    message.starts_with(&HEADER)
        && message
            .get(5)
            .is_some_and(|channel| matches!(channel, 0 | 4))
        && message.get(6) == Some(&command)
        && message.ends_with(&[0xf7])
}

/// Update the selected bank's name and read it back. Selecting a bank sends
/// Workbench's program change to switch position zero and changes the preset.
pub fn rename_bank<L: Link>(
    session: &mut Session<L>,
    bank: u8,
    name: &str,
) -> Result<String, String> {
    let location = Preset::new(bank, 0).ok_or("Invalid Variax bank.")?;
    let name_bytes = name_field(name, 0)?;
    transaction(session, G7, true, |session| {
        WriteRequest(G7, Message::SelectBank(location)).send(session)?;
        session.link.pause(BANK_SELECT_SETTLE);
        WriteRequest(G7, Message::BankName(&name_bytes)).send(session)?;
        session.link.pause(BANK_WRITE_SETTLE);
        let read_back = read_current_bank_name(session)?;
        if read_back != name.trim_end_matches(' ') {
            return Err("Variax bank-name readback differs from the requested name.".into());
        }
        Ok(read_back)
    })
}

/// Read the bank name without a program change.
pub fn read_bank_name<L: Link>(session: &mut Session<L>) -> Result<String, String> {
    transaction(session, G7, false, read_current_bank_name)
}

fn read_current_bank_name<L: Link>(session: &mut Session<L>) -> Result<String, String> {
    WriteRequest(G7, Message::ReadBankName).send(session)?;
    let Wait::Found(reply) = session.wait_for(
        BANK_REPLY,
        |reply| reply.len() >= 26 && is_reply(reply, 0x6d),
        false,
    )?
    else {
        return Err("Timed out waiting for the Variax bank-name reply.".into());
    };
    let name = &reply[7..7 + NAME_SIZE];
    if name.iter().any(|byte| !matches!(byte, 0 | 0x20..=0x7e)) {
        return Err("Invalid ASCII in the Variax bank-name reply.".into());
    }
    Ok(String::from_utf8_lossy(name)
        .trim_end_matches(['\0', ' '])
        .to_owned())
}

#[cfg(test)]
mod tests;

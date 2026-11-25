//! Workbench 1.75 write messages and transactions for the Variax 300/600.
//!
//! The caller supplies an already-open link and the G6/G7 policy. This module
//! has no device discovery, USB open, or application entry point.

pub use super::read_bank_name;
use super::{
    BANK_NAME_SIZE as NAME_SIZE, BRIDGE_CHUNK, HEADER, Preset, SESSION_SETTLE, VariaxRequest,
    bridge_message, is_reply, read_current_bank_name,
};
use crate::gate::Operation;
use crate::midi::Event;
use crate::session::{Link, POLL, Path, Request, Session};
use std::time::Duration;

const WORDS: usize = 1092;
const CHECKSUM_WORD: usize = WORDS - 1;
const HOST_SUM_WORDS: usize = 1089;
const WORD_MASK: u64 = 0xff_ffff;
const NAME_OFFSET: usize = 2553;
const STORE_SETTLE: Duration = Duration::from_millis(100);
const DIFFERENCE_ACK: Duration = Duration::from_secs(1);
const STORE_ACK: Duration = Duration::from_secs(15);
const BANK_SELECT_SETTLE: Duration = Duration::from_millis(200);
const BANK_WRITE_SETTLE: Duration = Duration::from_millis(700);

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
        let host_sum = (0..HOST_SUM_WORDS).map(|i| program.word(i)).sum::<u64>();
        let checksum = program.word(CHECKSUM_WORD);
        if host_sum & WORD_MASK != checksum {
            return Err("The compiled Variax program has an invalid host checksum.".into());
        }
        let firmware_sum = host_sum
            + (HOST_SUM_WORDS..CHECKSUM_WORD)
                .map(|i| program.word(i))
                .sum::<u64>();
        if firmware_sum & WORD_MASK != checksum {
            return Err("The compiled Variax program has an invalid firmware checksum.".into());
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

fn difference(previous: &Program<'_>, next: &Program<'_>) -> Option<Vec<u8>> {
    let mut payload = Vec::new();
    let changed = |index| previous.word(index) != next.word(index);
    let mut index = 0;
    while index < CHECKSUM_WORD {
        if !changed(index) {
            index += 1;
            continue;
        }
        let start = index;
        while index < CHECKSUM_WORD && changed(index) {
            index += 1;
        }
        let count = index - start;
        // Firmware 3.10 reads count/start even for a run reaching word 1,090.
        push_word(&mut payload, count as u64);
        push_word(&mut payload, start as u64);
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
}

#[derive(Clone, Copy, Debug)]
struct WriteRequest<'a>(Operation, Message<'a>);

impl Request for WriteRequest<'_> {
    fn bytes(self) -> Vec<u8> {
        match self.1 {
            Message::Bridge(on) => bridge_message(on),
            Message::Session(true) => VariaxRequest::SessionOn.bytes(),
            Message::Session(false) => VariaxRequest::SessionOff.bytes(),
            Message::DspEnabled(on) => sysex(0x4c, [u8::from(!on)]),
            Message::Full(bytes) | Message::Difference(bytes) => bytes.to_vec(),
            Message::Tone(value) => vec![0xb0, 79, value],
            Message::Store(preset) => sysex(0x61, [preset.bank() * 5 + (4 - preset.slot())]),
            Message::SelectBank(preset) => vec![0xc0, preset.program_number()],
            Message::BankName(name) => sysex(0x6c, *name),
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
        mut stopping: impl FnMut() -> bool,
    ) -> Result<(), String> {
        let check_stop = || {
            if stopping() {
                Err("Variax transfer stopped.")
            } else {
                Ok(())
            }
        };
        match self.1 {
            Message::Bridge(_) | Message::Tone(_) | Message::SelectBank(_) => session.send(self),
            Message::Full(_) => session.send_chunked_sysex(self, BRIDGE_CHUNK, None, check_stop),
            _ => session.send_chunked_sysex(self, BRIDGE_CHUNK, Some(HEADER.len() + 1), check_stop),
        }
    }
}

const G6: Operation = Operation::VariaxEditBufferWrite;
const G7: Operation = Operation::VariaxStoreRestore;

fn transaction<L: Link, T, E: From<String> + std::fmt::Display>(
    session: &mut Session<L>,
    operation: Operation,
    bypass_dsp: bool,
    body: impl FnOnce(&mut Session<L>) -> Result<T, E>,
) -> Result<T, E> {
    if session.link.path() != Path::VariaxViaPodxt || !session.allows(operation) {
        return Err(format!(
            "Variax transaction requires the PODxt Live bridge and {operation:?}."
        )
        .into());
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
        Ok(_) => Err(cleanup.into()),
        Err(error) => Err(format!("{error} {cleanup}").into()),
    }
}

/// Upload only the full working buffer in a read-style bridged session.
/// The owner audition does not bypass DSP or send a store command.
pub fn audition<L: Link>(
    session: &mut Session<L>,
    program: &Program<'_>,
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    if session.link.path() != Path::VariaxViaPodxt
        || !session.allows(G6)
        || !session.allows(Operation::VariaxRead)
    {
        return Err("Variax audition requires the bridged read and edit gates.".into());
    }
    if stopping() {
        return Err("Variax transfer stopped.".into());
    }
    super::with_bridged_read_session(session, super::span_read::BRIDGE_SETTLE, |session| {
        WriteRequest(G6, Message::Full(&full(program))).send_until(session, &mut stopping)
    })
}

/// A G6 edit within one open Workbench-style session.
pub enum Edit<'a> {
    /// Upload the final program when the current edit-buffer contents are unknown.
    Full(&'a Program<'a>),
    /// The caller supplies the program currently in the guitar's edit buffer.
    Difference {
        previous: &'a Program<'a>,
        next: &'a Program<'a>,
    },
    /// The original generates the diff target with its tuning-transition flag
    /// set, then regenerates the full program with the flag cleared.
    /// Supply the current program as the baseline, or upload the final program
    /// with `Edit::Full` when the baseline is unknown.
    AlternateTuning {
        previous: &'a Program<'a>,
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
                    if let Some(bytes) = difference(previous, next) {
                        send_difference(session, &bytes, &mut stopping)?;
                    }
                }
                Edit::AlternateTuning {
                    previous,
                    transition,
                    full: program,
                } => {
                    if let Some(bytes) = difference(previous, transition) {
                        send_difference(session, &bytes, &mut stopping)?;
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

fn send_difference<L: Link>(
    session: &mut Session<L>,
    bytes: &[u8],
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    require_quiet_reply_stream(session)?;
    let mut attempted = false;
    let sent = WriteRequest(G6, Message::Difference(bytes)).send_until(session, || {
        let stopped = stopping();
        attempted |= !stopped;
        stopped
    });
    // EOX recovery after cancellation or a transport error can still elicit
    // 0x71. Finish this wait before sending another command.
    if !attempted {
        return sent;
    }
    let reply =
        wait_for_confirmation(session, DIFFERENCE_ACK, "difference").and_then(
            |reply| match reply {
                Confirmation::Received => Ok(()),
                Confirmation::TimedOut => {
                    Err("Timed out waiting for the Variax difference reply.".into())
                }
            },
        );
    match (sent, reply) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("{error} Reopen the link before writing.")),
        (Err(send), Err(reply)) => Err(format!("{send} {reply} Reopen the link before writing.")),
    }
}

fn require_quiet_reply_stream<L: Link>(session: &mut Session<L>) -> Result<(), String> {
    if session.poll(Duration::from_millis(1))?.is_some() {
        return Err(
            "Unread MIDI data in the Variax session. Reopen the link before writing.".into(),
        );
    }
    Ok(())
}

/// Store one slot. The preparatory edit buffer and cleanup belong to G7.
/// After sending the store request, always finish waiting for its confirmation.
pub fn store<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    store_checked(session, preset, program, stopping).map_err(|error| error.to_string())
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The store was sent, the reply window expired, and cleanup sends succeeded.
    NoConfirmation,
    Other(String),
}

impl From<String> for StoreError {
    fn from(error: String) -> Self {
        Self::Other(error)
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConfirmation => {
                f.write_str("Timed out waiting for the Variax store reply. Slot state is unknown.")
            }
            Self::Other(error) => f.write_str(error),
        }
    }
}

/// Store with a distinct error for a silent reply timeout after successful
/// cleanup sends. Unexpected traffic and transport failures remain errors.
pub fn store_checked<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
) -> Result<(), StoreError> {
    store_observing_commit(session, preset, program, stopping, || {})
}

/// Call the observer immediately before attempting the irreversible slot command.
pub fn store_observing_commit<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    mut stopping: impl FnMut() -> bool,
    committing: impl FnOnce(),
) -> Result<(), StoreError> {
    if stopping() {
        return Err(StoreError::Other("Variax transfer stopped.".into()));
    }
    // A G6-enabled session can have edit acknowledgements on this stream.
    // Dedicated G7 procedures supply an exclusive, known reply stream.
    let shared_with_editing = session.allows(G6);
    if shared_with_editing && session.link.path() == Path::VariaxViaPodxt && session.allows(G7) {
        require_quiet_reply_stream(session)?;
    }
    transaction(session, G7, true, |session| {
        WriteRequest(G7, Message::Full(&full(program))).send_until(session, &mut stopping)?;
        session.link.pause(STORE_SETTLE);
        if stopping() {
            return Err("Variax transfer stopped before the slot store."
                .to_owned()
                .into());
        }
        if shared_with_editing {
            require_quiet_reply_stream(session)?;
        }
        let unknown = |error| format!("{error} Slot state is unknown.");
        committing();
        WriteRequest(G7, Message::Store(preset))
            .send(session)
            .map_err(unknown)?;
        match wait_for_confirmation(session, STORE_ACK, "store").map_err(unknown)? {
            Confirmation::Received => Ok(()),
            Confirmation::TimedOut => Err(StoreError::NoConfirmation),
        }
    })
}

enum Confirmation {
    Received,
    TimedOut,
}

fn wait_for_confirmation<L: Link>(
    session: &mut Session<L>,
    timeout: Duration,
    command: &str,
) -> Result<Confirmation, String> {
    let unexpected = || format!("Unexpected Variax {command} reply.");
    let start = session.link.elapsed();
    let orphans = session.assembler.orphans;
    let mut unexpected_data = false;
    loop {
        let remaining = timeout.saturating_sub(session.link.elapsed().saturating_sub(start));
        if remaining.is_zero() {
            return if unexpected_data {
                Err(unexpected())
            } else {
                Ok(Confirmation::TimedOut)
            };
        }
        let Some(events) = session.poll_with(remaining.min(POLL), |packet| {
            unexpected_data |= packet
                .iter()
                .any(|byte| matches!(byte, 0xf1..=0xf6 | 0xf8..=0xff));
            Ok(())
        })?
        else {
            continue;
        };
        unexpected_data |= session.assembler.orphans != orphans;
        let mut acknowledged = false;
        for event in events {
            match event {
                Event::Sysex(reply)
                    if !acknowledged && reply.len() == 8 && is_reply(&reply, 0x71) =>
                {
                    acknowledged = true;
                }
                Event::Sysex(reply) if is_reply(&reply, 0x28) => {
                    session.note(format!(
                        "Skipped Variax clip notice (0x28) during {command}."
                    ));
                }
                Event::Sysex(reply) if is_reply(&reply, 0x4d) => {
                    session.note(format!(
                        "Skipped Variax DSP overload notice (0x4D) during {command}."
                    ));
                }
                Event::Sysex(reply) if is_reply(&reply, 0x74) => {
                    session.note(format!(
                        "Skipped Variax MIDI queue overflow notice (0x74) during {command}."
                    ));
                }
                _ => unexpected_data = true,
            }
        }
        // Difference and store share 0x71. Keep reading a difference reply
        // after unexpected traffic so its acknowledgement does not leak into
        // a later wait. For a store, unexpected traffic ends the wait.
        if unexpected_data && (acknowledged || command == "store") {
            return Err(unexpected());
        }
        if acknowledged {
            return Ok(Confirmation::Received);
        }
    }
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
    if !session.allows(Operation::VariaxBankNameRead) {
        return Err("Variax rename requires VariaxBankNameRead for verification.".into());
    }
    transaction(session, G7, true, |session| {
        WriteRequest(G7, Message::SelectBank(location)).send(session)?;
        session.link.pause(BANK_SELECT_SETTLE);
        WriteRequest(G7, Message::BankName(&name_bytes)).send(session)?;
        session.link.pause(BANK_WRITE_SETTLE);
        let read_back = read_current_bank_name(session)?.name;
        if read_back != name.trim_end_matches(' ') {
            return Err("Variax bank-name readback differs from the requested name.".into());
        }
        Ok(read_back)
    })
}

#[cfg(test)]
mod tests;

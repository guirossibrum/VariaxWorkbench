//! Six global string levels on a Variax 300/600 with firmware 3.10 (#181, #206).
//! These words are independent of presets. For the protocol and lane 3
//! integration contract, see docs/variax-string-levels.md.

use super::{
    HEADER, decode_word, is_reply, procedures, span_read::BRIDGE_SETTLE, with_bridged_read_session,
};
use crate::{
    backup::BackupFolder,
    gate::Operation,
    midi::Event,
    session::{Link, POLL, Path, Request, Session},
};
use std::{
    path::{Component, Path as FilePath, PathBuf},
    time::Duration,
};

pub const APPROVED_STRING_LEVEL_READ: bool = false;
pub const APPROVED_STRING_LEVEL_EDIT: bool = false;
pub const APPROVED_STRING_LEVEL_STORE: bool = false;

const READ: Operation = Operation::VariaxRead;
const EDIT: Operation = Operation::VariaxEditBufferWrite;
const STORE: Operation = Operation::VariaxStoreRestore;
const READ_WINDOW: Duration = Duration::from_millis(500);
const MAX_WORD: u32 = 0xff_ffff;
pub const BACKUP_FILE: &str = "string-levels-ram.bin";

/// Six exact 24-bit words in device order (high E to low E).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Levels([u32; 6]);

impl Levels {
    pub fn from_wire_words(words: [u32; 6]) -> Result<Self, String> {
        if words.iter().any(|&word| word > MAX_WORD) {
            return Err("Variax string levels require six 24-bit words.".into());
        }
        Ok(Self(words))
    }

    pub fn wire_words(self) -> [u32; 6] {
        self.0
    }

    /// Save all bits, including sign, in the same order as the reply.
    pub fn backup_bytes(self) -> [u8; 18] {
        std::array::from_fn(|index| self.0[index / 3].to_be_bytes()[1 + index % 3])
    }

    pub fn from_backup_bytes(bytes: [u8; 18]) -> Self {
        let words = bytes.as_chunks::<3>().0;
        Self(std::array::from_fn(|i| {
            let [a, b, c] = words[i];
            u32::from_be_bytes([0, a, b, c])
        }))
    }

    fn oriented(self, string: usize) -> i32 {
        let word = ((self.0[5 - string] << 8) as i32) >> 8;
        if string.is_multiple_of(2) {
            word
        } else {
            -word
        }
    }

    /// Workbench slider fractions, low E to high E, bounded to 0..=1.
    pub fn workbench_levels(self) -> [f32; 6] {
        std::array::from_fn(|string| {
            (self.oriented(string).unsigned_abs() as f32 / 8_388_607.0).min(1.0)
        })
    }

    /// Convert slider positions using the original backup's inversion flag.
    /// Restoration must use the exact backup words because the slider mapping is lossy.
    pub fn with_workbench_levels(self, values: [f32; 6]) -> Result<Self, String> {
        if values.iter().any(|value| !(0.0..=1.0).contains(value)) {
            return Err("Variax string level fractions must be in 0..=1.".into());
        }
        let invert = (0..6).any(|string| self.oriented(string) < 0);
        Ok(Self(std::array::from_fn(|wire| {
            let string = 5 - wire;
            let level = values[string];
            let magnitude = (level * 8_388_607.0) as u32;
            if level > 0.0 && ((!string.is_multiple_of(2)) ^ invert) {
                MAX_WORD - magnitude
            } else {
                magnitude
            }
        })))
    }
}

#[derive(Clone, Copy, Debug)]
enum LevelRequest {
    Read,
    Set(Levels),
    Store,
}

impl Request for LevelRequest {
    fn bytes(self) -> Vec<u8> {
        let mut bytes = HEADER.to_vec();
        bytes.push(0);
        match self {
            Self::Read => bytes.push(0x57),
            Self::Set(levels) => {
                bytes.push(0x54);
                for word in levels.0 {
                    bytes.extend((0..6).map(|nibble| ((word >> (4 * nibble)) & 0xf) as u8));
                }
            }
            Self::Store => bytes.push(0x56),
        }
        bytes.push(0xf7);
        bytes
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        match self {
            Self::Read => READ,
            Self::Set(_) => EDIT,
            Self::Store => STORE,
        }
    }
}

fn decode_reply(reply: &[u8]) -> Result<Levels, String> {
    if reply.len() != 44 || !is_reply(reply, 0x57) {
        return Err("Expected a complete 44-byte Variax string-level reply (0x57).".into());
    }
    let mut words = [0; 6];
    for (word, nibbles) in words.iter_mut().zip(reply[7..43].as_chunks::<6>().0) {
        *word = decode_word(nibbles)?;
    }
    Ok(Levels(words))
}

fn read_active<L: Link>(session: &mut Session<L>) -> Result<Levels, String> {
    session.send(LevelRequest::Read)?;
    let deadline = session.link.elapsed() + READ_WINDOW;
    let orphans = session.assembler.orphans;
    while session.link.elapsed() < deadline {
        let left = deadline.saturating_sub(session.link.elapsed());
        if let Some(events) = session.poll(left.min(POLL))? {
            if session.assembler.orphans != orphans {
                return Err("Damaged Variax string-level reply.".into());
            }
            let mut result = None;
            for event in events {
                match event {
                    Event::Sysex(reply)
                        if [0x28, 0x4d, 0x74].iter().any(|&op| is_reply(&reply, op)) =>
                    {
                        session.note(format!(
                            "Skipped Variax notice 0x{:02X} during string-level read.",
                            reply[6]
                        ));
                    }
                    Event::Sysex(reply) if result.is_none() => result = Some(decode_reply(&reply)?),
                    Event::Sysex(_) => {
                        return Err(
                            "Unexpected extra SysEx during Variax string-level read.".into()
                        );
                    }
                    Event::Short(message) if message[0] & 0xf0 == 0xc0 => {
                        return Err(
                            "Unexpected program change during Variax string-level read.".into()
                        );
                    }
                    Event::Short(_) => {}
                }
            }
            if let Some(levels) = result {
                return Ok(levels);
            }
        }
    }
    Err("Variax string-level RAM read timed out.".into())
}

fn check_policy<L: Link>(session: &Session<L>, operations: &[Operation]) -> Result<(), String> {
    if session.link.path() != Path::VariaxViaPodxt
        || operations.iter().any(|&op| !session.allows(op))
    {
        return Err(format!(
            "Variax string levels require the PODxt Live bridge and {operations:?}."
        ));
    }
    Ok(())
}

fn read_on<L: Link>(session: &mut Session<L>) -> Result<Levels, String> {
    check_policy(session, &[Operation::VariaxIdentity, READ])?;
    procedures::identify(session)?;
    with_bridged_read_session(session, BRIDGE_SETTLE, read_active)
}

/// Read the six global RAM words, after verifying guitar identity on the caller's locked link.
pub fn read_ram<L: Link>(session: &mut Session<L>) -> Result<Levels, String> {
    if !APPROVED_STRING_LEVEL_READ {
        return Err("Variax string-level reads have no owner approval.".into());
    }
    read_on(session)
}

enum State {
    Ready(Levels),
    NeedsRestore,
    RestoreFailed(String),
    StoreRequested,
    StoreUncertain(String),
}

/// A string-level edit protected by a saved RAM backup and the caller's rig lock.
pub struct Edit<'s, 'p, L: Link> {
    session: &'s mut Session<'p, L>,
    original: Levels,
    backup: PathBuf,
    state: State,
}

/// A sent store request and the RAM evidence captured just before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreRequestSent {
    pub ram_before_request: Levels,
}

impl<L: Link> Edit<'_, '_, L> {
    pub fn original(&self) -> Levels {
        self.original
    }

    pub fn backup_folder(&self) -> &FilePath {
        &self.backup
    }

    fn set_and_check(&mut self, levels: Levels) -> Result<Levels, String> {
        self.session
            .send_chunked_sysex(LevelRequest::Set(levels), 256, None, || Ok(()))?;
        let actual = read_active(self.session)?;
        if actual != levels {
            return Err(format!(
                "String-level RAM read-back differs: expected {levels:?}, read {actual:?}."
            ));
        }
        Ok(actual)
    }

    /// Set all six levels in RAM and verify them with 0x57.
    /// After an error, only restoration is available through this edit.
    pub fn set_ram(&mut self, levels: Levels) -> Result<Levels, String> {
        if !matches!(self.state, State::Ready(_)) {
            return Err("String-level editing cannot continue in the current state.".into());
        }
        self.state = State::NeedsRestore;
        let actual = self.set_and_check(levels)?;
        self.state = State::Ready(actual);
        Ok(actual)
    }

    /// Restore and verify the exact six saved RAM words.
    pub fn restore_ram(&mut self) -> Result<(), String> {
        match &self.state {
            State::Ready(levels) if *levels == self.original => return Ok(()),
            State::Ready(_) | State::NeedsRestore => {}
            State::RestoreFailed(error) | State::StoreUncertain(error) => return Err(error.clone()),
            State::StoreRequested => {
                return Err("String-level restore is closed after a store request.".into());
            }
        }
        match self.set_and_check(self.original) {
            Ok(original) => {
                self.state = State::Ready(original);
                Ok(())
            }
            Err(error) => {
                let error = format!("String-level RAM restore could not be verified: {error}");
                self.state = State::RestoreFailed(error.clone());
                Err(error)
            }
        }
    }

    /// After separate approval, verify the current RAM and request flash storage once.
    /// The caller is responsible for a separately approved persistence check.
    pub fn request_store(&mut self) -> Result<StoreRequestSent, String> {
        if !APPROVED_STRING_LEVEL_STORE {
            return Err("Variax string-level storage has no owner approval.".into());
        }
        self.request_store_on()
    }

    fn request_store_on(&mut self) -> Result<StoreRequestSent, String> {
        check_policy(self.session, &[STORE])?;
        let State::Ready(expected) = self.state else {
            return Err(
                "String-level storage requires verified RAM and is limited to one request.".into(),
            );
        };
        self.state = State::NeedsRestore;
        let actual = read_active(self.session)?;
        if actual != expected {
            return Err(
                "String-level RAM changed before storage. No store request was sent.".into(),
            );
        }
        // A transport failure can happen after delivery. Do not send a second
        // request, an isolated F7, or a RAM restoration to this session.
        match self.session.send(LevelRequest::Store) {
            Ok(()) => {
                self.state = State::StoreRequested;
                Ok(StoreRequestSent {
                    ram_before_request: actual,
                })
            }
            Err(error) => {
                let error = format!("String-level store request delivery is uncertain: {error}");
                self.state = State::StoreUncertain(error.clone());
                Err(error)
            }
        }
    }
}

/// Back up the six raw words to a new local folder before exposing the edit.
/// On ordinary callback return, restore the original levels unless a store
/// request was attempted. A failed restore is reported and never retried.
/// The caller holds the rig lock for the entire callback and releases the link.
pub fn with_temporary_edit<L: Link, T>(
    session: &mut Session<L>,
    backup_root: &FilePath,
    backup_name: &str,
    run: impl FnOnce(&mut Edit<'_, '_, L>) -> Result<T, String>,
) -> Result<T, String> {
    if !APPROVED_STRING_LEVEL_READ || !APPROVED_STRING_LEVEL_EDIT {
        return Err("Variax string-level editing requires separate read and edit approval.".into());
    }
    edit_on(session, backup_root, backup_name, run)
}

fn edit_on<L: Link, T>(
    session: &mut Session<L>,
    backup_root: &FilePath,
    backup_name: &str,
    run: impl FnOnce(&mut Edit<'_, '_, L>) -> Result<T, String>,
) -> Result<T, String> {
    check_policy(session, &[Operation::VariaxIdentity, READ, EDIT])?;
    let mut components = FilePath::new(backup_name).components();
    if !matches!(components.next(), Some(Component::Normal(name)) if name == backup_name)
        || components.next().is_some()
    {
        return Err("String-level backup name must be one directory name.".into());
    }
    let identity = procedures::identify(session)?;
    with_bridged_read_session(session, BRIDGE_SETTLE, |session| {
        let original = read_active(session)?;
        let mut backup = BackupFolder::create(backup_root, backup_name)?;
        backup.file(BACKUP_FILE, &original.backup_bytes())?;
        backup.record(format!(
            "Variax 300/600 {}: six global string levels read from RAM (0x57).",
            identity.identity.version_text()
        ));
        backup.set_restore_note("This is a RAM snapshot. Restore its six exact words using the approved RAM path. Flash persistence requires a separate owner procedure.");
        let backup = backup.finish(true)?;
        session.note(format!(
            "Saved Variax string-level RAM backup to {}.",
            backup.display()
        ));
        let mut edit = Edit {
            session,
            original,
            backup,
            state: State::Ready(original),
        };
        let result = run(&mut edit);
        let recovery = match &edit.state {
            State::StoreRequested => Ok(()),
            _ => edit.restore_ram(),
        };
        match (result, recovery) {
            (result, Ok(())) => result,
            (Err(error), Err(recovery)) if error != recovery => Err(format!("{error} {recovery}")),
            (_, Err(error)) => Err(error),
        }
    })
}

#[cfg(test)]
mod tests;

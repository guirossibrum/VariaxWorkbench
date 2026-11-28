//! Variax backup v1: identity, optional bank names, original metadata and
//! two raw SysEx metadata replies per stored preset.

use super as variax;
use crate::{
    backup::{BackupFolder, read_optional, verify_manifest},
    device_lock::DeviceLock,
    gate::Operation,
    session::{Link, Session},
};
use std::{fmt, path::Path};

mod provenance;
mod snapshot;
pub use provenance::{
    AttachmentIdentity, CaptureAttachment, CaptureLink, CaptureReceipt, CleanPosition,
    FinalizedCapture, SelectedCapture, StagedCapture, capture_library,
};
pub use snapshot::{
    SnapshotCapture, SnapshotCoverage, SnapshotPrograms, ValidatedSnapshot, validate_snapshot,
};

pub(crate) const STORED_FOLDERS: &[&str] = &["presets"];

pub const VARIAX_RESTORE: &str =
    "Variax 600 library backup (read-only copy, line6-devices backup v1).

presets/BB-Bank-S-1.syx and -2.syx are the exact 200-byte SysEx replies the guitar
sent for metadata blocks 1 (address 851) and 2 (address 883) of bank BB, slot S,
after a program change to that preset. index.txt lists bank, slot, Workbench
program number, preset name, body model and any bank names read from the guitar.
bank-names/BB.syx contains the exact 0x6D reply for bank BB when available.
Older backups omit these replies. original-1/2.syx is the preset that was
active before the read. Load with line6_devices::backup::load_variax_library,
or decode one preset with variax::PresetMetadata::from_replies.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   read the library again and compare the two folders
                   (line6_devices::backup::compare, `variax-probe compare`).

These blocks are current-preset metadata as Workbench reads it, not a DSP or
firmware image. Restoring them to the guitar needs a write gate (G6/G7), the
owner's direct permission and an approved restore procedure.
";

pub fn variax_preset_stem(preset: variax::Preset) -> String {
    format!(
        "presets/{:02}-{}-{}",
        preset.bank(),
        preset.bank_name().replace(' ', "_"),
        preset.slot() + 1
    )
}

pub(crate) fn variax_bank_name_file(bank: u8) -> String {
    format!("bank-names/{bank:02}.syx")
}

pub fn variax_index_line(preset: variax::Preset, metadata: &variax::PresetMetadata) -> String {
    let body = match metadata.parameters.get("body_model") {
        Some(variax::ParameterValue::Integer(id)) => format!("{id:#010x}"),
        _ => "?".into(),
    };
    format!(
        "{:>8} {} (program {:3}): {:<18} body {body}",
        preset.bank_name(),
        preset.slot() + 1,
        preset.program_number(),
        metadata.name
    )
}

/// Writes a Variax library read in the v1 layout, plus index and restore note.
pub fn save_variax_library(
    folder: &mut BackupFolder,
    library: &variax::Library,
) -> Result<(), String> {
    save_library(folder, library, None)
}

fn save_library(
    folder: &mut BackupFolder,
    library: &variax::Library,
    programs: Option<&ProgramCaptures>,
) -> Result<(), String> {
    folder.file("identity.syx", &library.identified.reply)?;
    folder.file("original-1.syx", &library.original_replies[0])?;
    folder.file("original-2.syx", &library.original_replies[1])?;
    let mut index = String::new();
    for stored in &library.presets {
        let stem = variax_preset_stem(stored.preset);
        folder.file(&format!("{stem}-1.syx"), &stored.replies[0])?;
        folder.file(&format!("{stem}-2.syx"), &stored.replies[1])?;
        index.push_str(&variax_index_line(stored.preset, &stored.metadata));
        index.push('\n');
    }
    for (bank, name) in &library.bank_names {
        folder.file(&variax_bank_name_file(*bank), &name.reply)?;
        use std::fmt::Write as _;
        writeln!(index, "Bank {bank:02} name: {:?}", name.name).expect("write to String");
    }
    if let Some(programs) = programs {
        programs.save(folder, &mut index)?;
    }
    folder.file("index.txt", index.as_bytes())?;
    if programs.is_some() {
        folder.set_restore_note(&format!("{VARIAX_RESTORE}\n{FULL_PROGRAM_NOTE}"));
    } else {
        folder.set_restore_note(VARIAX_RESTORE);
    }
    Ok(())
}

const CHUNK_WORDS: u16 = 32;
const PROGRAM_WORDS: u16 = variax::span_read::BUFFER_WORDS;
const CHUNK_COUNT: usize = PROGRAM_WORDS.div_ceil(CHUNK_WORDS) as usize;
const PROGRAM_BYTES: usize = PROGRAM_WORDS as usize * 3;

const FULL_PROGRAM_NOTE: &str =
    "Full program mode adds programs/NN-Bank-S.bin. NN is the zero-based stored slot.
The compare command checks the settings. Compare the .bin files separately.
Each saved file contains the 1,092 program words packed as 24-bit big-endian
bytes, including the verified firmware checksum. index.txt lists missing
or failed programs. This capture does not authorize a restore.";

#[derive(Default)]
pub(super) struct ProgramCaptures {
    programs: Vec<(variax::Preset, Result<[u8; PROGRAM_BYTES], String>)>,
}

impl ProgramCaptures {
    pub(super) fn capture<L: Link>(
        &mut self,
        session: &mut Session<L>,
        preset: variax::Preset,
        cancelled: impl FnMut() -> bool,
    ) -> Result<(), String> {
        let result = read_program(session, cancelled);
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        self.programs.push((preset, result));
        status
    }

    fn save(&self, folder: &mut BackupFolder, index: &mut String) -> Result<(), String> {
        use fmt::Write as _;
        for preset in variax::Preset::all() {
            let name = program_file(preset);
            match self.programs.iter().find(|(slot, _)| *slot == preset) {
                Some((_, Ok(program))) => {
                    folder.file(&name, program)?;
                    writeln!(index, "{name}: verified (3276 bytes)").expect("write to String");
                }
                Some((_, Err(error))) => {
                    writeln!(index, "{name}: MISSING ({error})").expect("write to String");
                }
                None => writeln!(index, "{name}: MISSING (not read)").expect("write to String"),
            }
        }
        Ok(())
    }
}

fn program_file(preset: variax::Preset) -> String {
    format!(
        "programs/{:02}-{}-{}.bin",
        (preset.bank() + 1) * variax::PRESETS_PER_BANK - 1 - preset.slot(),
        preset.bank_name().replace(' ', "_"),
        preset.slot() + 1
    )
}

fn read_program<L: Link>(
    session: &mut Session<L>,
    cancelled: impl FnMut() -> bool,
) -> Result<[u8; PROGRAM_BYTES], String> {
    use variax::span_read::{PresetSpan, read_active_spans};
    let spans: [PresetSpan; CHUNK_COUNT] = std::array::from_fn(|index| {
        let start = index as u16 * CHUNK_WORDS;
        PresetSpan::new(start, (PROGRAM_WORDS - start).min(CHUNK_WORDS)).expect("in bounds")
    });
    let mut program = [0; PROGRAM_BYTES];
    read_active_spans(session, &spans, cancelled, |reply| {
        let start = usize::from(reply.span.start()) * 3;
        for (packed, word) in program[start..]
            .as_chunks_mut::<3>()
            .0
            .iter_mut()
            .zip(reply.words)
        {
            *packed = word.to_be_bytes()[1..].try_into().expect("24-bit word");
        }
        Ok(())
    })?;
    check_program_checksum(&program)?;
    Ok(program)
}

fn check_program_checksum(program: &[u8; PROGRAM_BYTES]) -> Result<(), String> {
    let word = |&[a, b, c]: &[u8; 3]| u32::from_be_bytes([0, a, b, c]);
    let (checksum, words) = program
        .as_chunks::<3>()
        .0
        .split_last()
        .expect("program words");
    let expected = words.iter().map(word).fold(0u32, u32::wrapping_add) & 0xff_ffff;
    let stored = word(checksum);
    if stored != expected {
        return Err(format!(
            "Firmware checksum mismatch (stored {stored:06X}, computed {expected:06X})."
        ));
    }
    Ok(())
}

pub struct FullLibrary {
    pub settings: variax::Library,
    programs: ProgramCaptures,
}

impl FullLibrary {
    /// Completed captured programs, in capture order.
    pub fn verified_programs(
        &self,
    ) -> impl Iterator<Item = (variax::Preset, &[u8; PROGRAM_BYTES])> {
        self.programs
            .programs
            .iter()
            .filter_map(|(preset, result)| result.as_ref().ok().map(|bytes| (*preset, bytes)))
    }

    pub fn programs_read(&self) -> usize {
        self.programs
            .programs
            .iter()
            .filter(|(_, result)| result.is_ok())
            .count()
    }

    pub fn complete(&self) -> bool {
        self.programs_read() == 60
            && self.settings.stopped.is_none()
            && self.settings.release_error.is_none()
    }

    pub fn save(&self, folder: &mut BackupFolder) -> Result<(), String> {
        save_library(folder, &self.settings, Some(&self.programs))
    }
}

/// Visit all stored presets in one locked library sweep, reading and checking the full
/// program after each metadata pair. Retain completed captures when a read fails.
pub fn read_full_library<L: Link>(
    _rig: &DeviceLock,
    session: &mut Session<L>,
    cancelled: impl FnMut() -> bool,
    progress: impl FnMut(&variax::StoredPreset),
) -> Result<FullLibrary, String> {
    read_full_library_in_session(session, cancelled, progress)
}

/// Use the full backup sweep on a session whose caller already owns the rig
/// lock and is responsible for releasing the attached link.
pub fn read_full_library_in_session<L: Link>(
    session: &mut Session<L>,
    cancelled: impl FnMut() -> bool,
    progress: impl FnMut(&variax::StoredPreset),
) -> Result<FullLibrary, String> {
    if session.link.path() != crate::session::Path::VariaxViaPodxt {
        return Err("Full program backup requires the PODxt Live bridge.".into());
    }
    for operation in [
        Operation::VariaxIdentity,
        Operation::VariaxLibraryBridge,
        Operation::VariaxRead,
        Operation::VariaxPresetSwitch,
        Operation::VariaxBankNameRead,
    ] {
        if !session.allows(operation) {
            return Err(format!("Full program backup requires {operation:?}."));
        }
    }
    read_full_on(session, cancelled, progress)
}

fn read_full_on<L: Link>(
    session: &mut Session<L>,
    cancelled: impl FnMut() -> bool,
    progress: impl FnMut(&variax::StoredPreset),
) -> Result<FullLibrary, String> {
    let mut programs = ProgramCaptures::default();
    let settings =
        variax::sweep_library_with_programs(session, cancelled, progress, Some(&mut programs))?;
    Ok(FullLibrary { settings, programs })
}

pub struct LoadedVariax {
    pub original: Option<variax::PresetMetadata>,
    pub presets: Vec<(variax::Preset, variax::PresetMetadata)>,
    pub bank_names: Vec<(u8, variax::BankName)>,
}

/// Verifies the manifest, then decodes the presets and optional bank names.
pub fn load_variax_library(dir: &Path) -> Result<LoadedVariax, String> {
    verify_manifest(dir)?;
    let pair = |first: &str, second: &str| -> Result<Option<variax::PresetMetadata>, String> {
        match (read_optional(dir, first)?, read_optional(dir, second)?) {
            (Some(a), Some(b)) => variax::PresetMetadata::from_replies(&a, &b)
                .map(Some)
                .map_err(|e| format!("{first}: {e}")),
            (None, None) => Ok(None),
            _ => Err(format!("{first} and {second} must both be present")),
        }
    };
    let original = pair("original-1.syx", "original-2.syx")?;
    let mut presets = Vec::new();
    for preset in variax::Preset::all() {
        let stem = variax_preset_stem(preset);
        if let Some(metadata) = pair(&format!("{stem}-1.syx"), &format!("{stem}-2.syx"))? {
            presets.push((preset, metadata));
        }
    }
    let mut bank_names = Vec::new();
    for bank in 0..variax::BANKS.len() as u8 {
        let path = variax_bank_name_file(bank);
        if let Some(reply) = read_optional(dir, &path)? {
            let name = variax::BankName::from_reply(reply).map_err(|e| format!("{path}: {e}"))?;
            bank_names.push((bank, name));
        }
    }
    Ok(LoadedVariax {
        original,
        presets,
        bank_names,
    })
}

/// Complete editor input from one sweep, in the same representation as a
/// loaded backup. Switch-back and bridge release are reported independently.
pub struct RefreshedLibrary {
    pub identified: variax::Identified,
    pub library: LoadedVariax,
    pub restore: variax::Restore,
    pub release_error: Option<String>,
}

#[derive(Debug)]
pub enum RefreshError {
    Read(String),
    Incomplete {
        reason: String,
        restore: variax::Restore,
        release_error: Option<String>,
    },
}

impl fmt::Display for RefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => formatter.write_str(error),
            Self::Incomplete {
                reason,
                restore,
                release_error,
            } => {
                write!(formatter, "Variax Refresh incomplete: {reason} ")?;
                match restore {
                    variax::Restore::Switched { to, verified } => {
                        write!(
                            formatter,
                            "Reselected {} {}. Read-back verified: {verified}.",
                            to.bank_name(),
                            to.slot() + 1,
                        )?;
                    }
                    variax::Restore::NoUniqueMatch { matches } => {
                        write!(
                            formatter,
                            "Found {matches} matches for the original preset. No preset was reselected."
                        )?;
                    }
                    variax::Restore::Failed(error) => {
                        write!(formatter, "Could not verify the original preset: {error}")?;
                    }
                }
                if let Some(error) = release_error {
                    write!(formatter, " {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RefreshError {}

/// Capture all 60 preset settings and 12 bank names without saving files.
/// Acquire the rig lock before opening the link and hold it through cleanup.
/// The identification stage retries complete silence once before selecting
/// a preset. Progress reports a preset after both metadata replies decode.
/// Cancellation is checked between requests for presets and bank names. On
/// cancellation or read failure, the backup sweep still attempts to reselect
/// a unique match for the original preset. Only complete data reaches the editor.
///
/// This uses the backup's gated program selections, settings reads, and read
/// bank name (`0x6D`). DSP processing stays active.
pub fn refresh_library<L: Link>(
    _rig: &DeviceLock,
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    progress: impl FnMut(variax::Preset),
) -> Result<RefreshedLibrary, RefreshError> {
    refresh_on(session, cancelled, progress)
}

fn refresh_on<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    mut progress: impl FnMut(variax::Preset),
) -> Result<RefreshedLibrary, RefreshError> {
    for operation in [
        Operation::VariaxIdentity,
        Operation::VariaxLibraryBridge,
        Operation::VariaxRead,
        Operation::VariaxPresetSwitch,
        Operation::VariaxBankNameRead,
    ] {
        if !session.allows(operation) {
            return Err(RefreshError::Read(format!(
                "Variax Refresh requires {operation:?}."
            )));
        }
    }
    let identified = variax::identify_exchange(
        session,
        Some(variax::procedures::IDENTITY_RETRY_DELAY),
        true,
        Some(&cancelled),
    )
    .and_then(|reply| reply.ok_or_else(|| "No reply after two requests.".into()))
    .and_then(|identified| {
        variax::procedures::check_identity(&identified.identity)?;
        Ok(identified)
    })
    .map_err(|error| RefreshError::Read(format!("Guitar sync identity: {error}")))?;
    let read = variax::sweep_identified_library(
        session,
        identified,
        cancelled,
        |stored| progress(stored.preset),
        None,
        None,
    )
    .map_err(|error| RefreshError::Read(format!("Guitar sync preparation: {error}")))?;
    if let Some(reason) = read.stopped {
        return Err(RefreshError::Incomplete {
            reason,
            restore: read.restore,
            release_error: read.release_error,
        });
    }
    Ok(RefreshedLibrary {
        identified: read.identified,
        library: LoadedVariax {
            original: Some(read.original),
            presets: read
                .presets
                .into_iter()
                .map(|stored| (stored.preset, stored.metadata))
                .collect(),
            bank_names: read.bank_names,
        },
        restore: read.restore,
        release_error: read.release_error,
    })
}

#[cfg(test)]
mod tests;

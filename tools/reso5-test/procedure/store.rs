use super::{
    METADATA, NAME, STORED_HASH, check_backup, compare_settings, expected_stored_image, full_read,
    location, matches_hash, set_firmware_checksum,
};
use line6_devices::{
    family::variax::{
        procedures::{self, VerifiedStoreError, compare_image, read_settings},
        write,
    },
    session::{Link, Session},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
use variax_workbench::{factory_reference::PROGRAM_BYTES, open_regular_file, path_facts};

const CAPTURE_HASH: &str = "3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249";
const RENAME_HASH: &str = "75bcaaf5f775bcd2f5ba22289aa4b8e9cac0c6115b4ac32b75144b4ccb5c716b";
const RENAMED: &str = "Reso-5 T";
pub const SPARE_WARNING: &str = "Do not store any slot in 40–59 until spare 62 is known blank.";

type Image = [u8; PROGRAM_BYTES];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Rename,
    Restore,
    Factory,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "rename" => Some(Self::Rename),
            "restore" => Some(Self::Restore),
            "factory" => Some(Self::Factory),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Restore => "restore",
            Self::Factory => "factory",
        }
    }

    pub fn confirmation(self) -> &'static str {
        match self {
            Self::Rename => "STORE RESO-5 RENAME PROGRAM TO SLOT 50",
            Self::Restore => "STORE RESO-5 RESTORE PROGRAM TO SLOT 50",
            Self::Factory => "STORE RESO-5 FACTORY PROGRAM TO SLOT 50",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Rename => RENAMED,
            Self::Restore | Self::Factory => NAME,
        }
    }

    fn expected_hash(self) -> &'static str {
        match self {
            Self::Rename => RENAME_HASH,
            Self::Restore => CAPTURE_HASH,
            Self::Factory => STORED_HASH,
        }
    }
}

pub struct Prepared {
    pub mode: Mode,
    pub(super) before: Image,
    pub(super) program: Image,
    pub(super) after: Image,
}

impl Prepared {
    pub fn expected_sha256(&self) -> String {
        format!("{:x}", Sha256::digest(self.after))
    }
}

// The same span plan, filenames and decoder are used by #234's live capture.
pub(super) fn load_capture(dir: &Path) -> Result<Image, String> {
    let mut program = [0; PROGRAM_BYTES];
    for span in full_read::chunks() {
        let file = full_read::chunk_path(dir, span);
        let facts = path_facts(&file).map_err(|error| format!("{}: {error}", file.display()))?;
        if !facts.regular_file {
            return Err(format!(
                "{}: capture chunk must be a regular file.",
                file.display()
            ));
        }
        let mut raw = Vec::new();
        open_regular_file(&file)
            .and_then(|handle| {
                handle
                    .take(span.reply_bytes() as u64 + 1)
                    .read_to_end(&mut raw)
            })
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let words = span
            .decode_reply(&raw)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let start = usize::from(span.start()) * 3;
        for (word, bytes) in words
            .into_iter()
            .zip(program[start..].as_chunks_mut::<3>().0)
        {
            bytes.copy_from_slice(&word.to_be_bytes()[1..]);
        }
    }
    validate_capture(&program, CAPTURE_HASH)?;
    Ok(program)
}

pub(super) fn validate_capture(program: &Image, hash: &str) -> Result<(), String> {
    if !matches_hash(program, hash) {
        return Err("The #234 capture does not match the pinned Reso-5 SHA-256.".into());
    }
    write::Program::new(NAME, program)?;
    Ok(())
}

pub(super) fn make_images(mode: Mode, capture: Image, factory: Image) -> Result<Prepared, String> {
    let mut renamed = capture;
    let name = &mut renamed[METADATA.start..METADATA.start + 18];
    name.fill(b' ');
    name[..RENAMED.len()].copy_from_slice(RENAMED.as_bytes());
    set_firmware_checksum(&mut renamed);

    let (before, program) = match mode {
        Mode::Rename => (capture, renamed),
        Mode::Restore => (expected_stored_image(&renamed), capture),
        Mode::Factory => (capture, factory),
    };
    write::Program::new(mode.name(), &program)?;
    let after = expected_stored_image(&program);
    Ok(Prepared {
        mode,
        before,
        program,
        after,
    })
}

pub fn prepare(
    mode: Mode,
    backup: &Path,
    archive: &Path,
    capture: &Path,
) -> Result<(Prepared, [u8; 192]), String> {
    let prepared = reference_images(mode, archive, capture)?;
    let backup_settings = check_backup(
        backup,
        prepared.before[METADATA].try_into().expect("metadata size"),
    )?;
    Ok((prepared, backup_settings))
}

pub(super) fn reference_images(
    mode: Mode,
    archive: &Path,
    capture: &Path,
) -> Result<Prepared, String> {
    let factory = full_read::prepare(archive)?;
    let prepared = make_images(mode, load_capture(capture)?, factory)?;
    if !matches_hash(&prepared.after, mode.expected_hash()) {
        return Err(format!(
            "Unexpected expected-image SHA-256 for {}.",
            mode.label()
        ));
    }
    Ok(prepared)
}

pub const RESELECT_CONFIRMATION: &str = "RESO 5 RESELECTED";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Confirm(Mode),
    Reselect,
}

pub fn require_confirmation(prompt: Prompt, answer: &str) -> Result<(), String> {
    let phrase = match prompt {
        Prompt::Confirm(mode) => mode.confirmation(),
        Prompt::Reselect => RESELECT_CONFIRMATION,
    };
    if answer.trim_end_matches(['\r', '\n']) != phrase {
        return Err(match prompt {
            Prompt::Confirm(_) => "Slot 50 store confirmation was not given.".into(),
            Prompt::Reselect => "Slot 50 store was confirmed, but manual reselection and read-back were not verified.".into(),
        });
    }
    Ok(())
}

pub fn attempt<L: Link>(
    session: &mut Session<L>,
    prepared: &Prepared,
    backup_settings: &[u8; 192],
    dir: &Path,
    mut prompt: impl FnMut(Prompt) -> Result<(), String>,
    announce_timeout: impl FnOnce(&str),
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), String> {
    session.require_expected_sysex();
    if cancelled() {
        return Err("Cancelled before reading the guitar.".into());
    }
    let current = read_settings(session)?;
    compare_settings(
        &current.metadata.raw,
        backup_settings,
        "The active Variax settings compared with the backup's slot 50 settings",
    )?;
    let preflight_dir = dir.join("preflight");
    let readback_dir = dir.join("readback");
    for path in [&preflight_dir, &readback_dir] {
        fs::create_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let before = full_read::read_chunks(session, &preflight_dir, &mut cancelled)?;
    compare_image(&before, &prepared.before, "Slot 50 preflight")?;
    let program = write::Program::new(prepared.mode.name(), &prepared.program)?;
    if cancelled() {
        return Err("Cancelled before confirming the store.".into());
    }
    prompt(Prompt::Confirm(prepared.mode))?;
    session.note(format!(
        "Slot 50 {} confirmed. Sending the pinned program.",
        prepared.mode.label()
    ));
    let verified = procedures::store_verified(
        session,
        location(),
        &program,
        "Reselected slot 50 read-back",
        &mut cancelled,
        announce_timeout,
        |session| {
            session.note("Received 0x71. Waiting for manual reselection of Reso 5.");
            prompt(Prompt::Reselect)?;
            full_read::read_chunks(session, &readback_dir, || false)
        },
    );
    match verified {
        Ok(()) => {}
        Err(VerifiedStoreError::SilentTimeout) => {
            return Err(format!(
                "Timed out waiting for the Variax store reply. Slot state is unknown. Check whether the guitar responds to a preset change. If it responds, take a Device check Variax backup and compare it with the first. Report the result and keep the store log. If it does not respond, report before powering off. {SPARE_WARNING}"
            ));
        }
        Err(
            VerifiedStoreError::BeforeCommit(error)
            | VerifiedStoreError::Unknown(error)
            | VerifiedStoreError::Verification(error),
        ) => return Err(error),
    }

    // Ctrl+C after 0x61 does not interrupt the verification reads.
    let settings = read_settings(session)?;
    compare_settings(
        &settings.metadata.raw,
        &prepared.after[METADATA],
        "Settings read-back",
    )?;
    session.note("Verified all 1,092 reselected words and all 192 settings bytes.");
    Ok(())
}

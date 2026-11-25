//! Owner-only #238 procedure. The live adapter keeps each rung separately closed.
mod sources;

use super::{IDENTITY_RETRY_DELAY, METADATA, compare_settings, full_read, store, variax_identity};
use line6_devices::{
    family::variax::{self, Preset, write},
    gate::{GatePolicy, Operation},
    session::{Link, Session},
};
use std::{cell::Cell, path::Path};
use variax_workbench::hardware_gates::{Gate, is_locked};

pub(super) type Image = [u8; variax_workbench::factory_reference::PROGRAM_BYTES];
pub use sources::prepare;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Listen,
    Store,
    Restore,
    RenameReso,
    FactoryReso,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "listen" => Some(Self::Listen),
            "store" => Some(Self::Store),
            "restore" => Some(Self::Restore),
            "3a" => Some(Self::RenameReso),
            "3b" => Some(Self::FactoryReso),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Listen => "listen",
            Self::Store => "store",
            Self::Restore => "restore",
            Self::RenameReso => "3a",
            Self::FactoryReso => "3b",
        }
    }

    pub fn listens(self) -> bool {
        self == Self::Listen
    }
}

pub fn target(index: u8) -> Result<Preset, String> {
    if index >= 60 {
        return Err("Flash index must be in 0–59.".into());
    }
    Ok(Preset::new(index / 5, 4 - index % 5).expect("validated flash index"))
}

fn index(preset: Preset) -> u8 {
    preset.bank() * 5 + 4 - preset.slot()
}

fn spare_warning(preset: Preset) -> String {
    let group = index(preset) / 20;
    let start = group * 20;
    format!(
        "Do not store any slot in {start}–{} until spare {} is known blank.",
        start + 19,
        60 + group
    )
}

fn slot_label(preset: Preset) -> String {
    format!(
        "slot {} ({} {}, MIDI program {})",
        index(preset),
        preset.bank_name(),
        preset.slot() + 1,
        preset.program_number()
    )
}

pub struct Prepared {
    pub mode: Mode,
    target: Preset,
    current: Vec<Image>,
    program: Image,
    expected: Image,
    description: Vec<String>,
}

impl Prepared {
    pub fn description(&self) -> &[String] {
        &self.description
    }

    pub fn confirmation(&self) -> String {
        format!(
            "{} SLOT {} {} {}",
            self.mode.label().to_ascii_uppercase(),
            index(self.target),
            self.target.bank_name(),
            self.target.slot() + 1
        )
    }
}

pub struct Policy(pub Mode);

impl GatePolicy for Policy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
            Operation::VariaxRead => !is_locked(Gate::CurrentSettingsRead),
            Operation::VariaxPresetSwitch => !is_locked(Gate::FullBackupRead),
            Operation::VariaxEditBufferWrite if self.0.listens() => {
                !is_locked(Gate::EditBufferWrite)
            }
            Operation::VariaxStoreRestore if !self.0.listens() => !is_locked(Gate::StoreRestore),
            _ => false,
        }
    }
}

fn identify(image: &Image, current: &[Image]) -> Result<Preset, String> {
    let mut matches = current
        .iter()
        .enumerate()
        .filter(|(_, program)| *program == image);
    let Some((index, _)) = matches.next() else {
        return Err("The active buffer matches no expected current slot byte for byte.".into());
    };
    if matches.next().is_some() {
        return Err("The active buffer matches more than one expected current slot.".into());
    }
    target(index as u8)
}

fn capture<L: Link>(
    session: &mut Session<L>,
    root: &Path,
    stage: &str,
    selected: Option<Preset>,
    stopping: impl FnMut() -> bool,
) -> Result<full_read::FullBuffer, String> {
    let dir = root.join(stage);
    std::fs::create_dir(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    if let Some(preset) = selected {
        session.note(format!("Selecting {} for {stage}.", slot_label(preset)));
    }
    full_read::read_chunks_selecting(session, &dir, selected, stopping)
}

fn pack(words: &full_read::FullBuffer) -> Image {
    let mut image = [0; std::mem::size_of::<Image>()];
    for (&word, bytes) in words.iter().zip(image.as_chunks_mut::<3>().0) {
        bytes.copy_from_slice(&word.to_be_bytes()[1..]);
    }
    image
}

fn returned<L: Link>(
    session: &mut Session<L>,
    root: &Path,
    stage: &str,
    original: Preset,
    snapshot: &Image,
    result: Result<(), String>,
) -> Result<(), String> {
    let restored = capture(session, root, stage, Some(original), || false)
        .and_then(|words| store::compare_image(&words, snapshot, "Original preset switch-back"));
    match (result, restored) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(format!(
            "{error} Original preset was reselected and verified."
        )),
        (Ok(()), Err(error)) => Err(format!("Original preset switch-back failed: {error}")),
        (Err(error), Err(restore)) => Err(format!(
            "{error} Original preset switch-back failed: {restore}"
        )),
    }
}

pub fn attempt<L: Link>(
    session: &mut Session<L>,
    prepared: &Prepared,
    root: &Path,
    mut prompt: impl FnMut(&str) -> Result<(), String>,
    mut announce: impl FnMut(&str),
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    session.require_expected_sysex();
    if stopping() {
        return Err("Cancelled before Variax identity.".into());
    }
    let device = variax::identify_retrying_silence(session, IDENTITY_RETRY_DELAY)?;
    variax_identity(&device.identity)?;
    let snapshot = pack(&capture(session, root, "snapshot", None, &mut stopping)?);
    let original = identify(&snapshot, &prepared.current)?;
    session.note(format!("Identified current {}.", slot_label(original)));
    if original == prepared.target && !prepared.mode.listens() {
        return Err("Select a different starting preset. Overwriting the original slot prevents a verified switch-back.".into());
    }
    if stopping() {
        return Err("Cancelled before target selection.".into());
    }
    let target = prepared.target;
    let baseline = &prepared.current[usize::from(index(target))];
    let preflight = capture(
        session,
        root,
        "target-preflight",
        Some(target),
        &mut stopping,
    )
    .and_then(|words| store::compare_image(&words, baseline, "Target preflight"));
    returned(
        session,
        root,
        "preflight-return",
        original,
        &snapshot,
        preflight,
    )?;
    if stopping() {
        return Err("Cancelled before uploading.".into());
    }
    prompt(&prepared.confirmation())?;
    if stopping() {
        return Err("Cancelled before uploading.".into());
    }
    let name = sources::metadata(&prepared.program)?.name;
    let program = write::Program::new(&name, &prepared.program)?;

    if prepared.mode.listens() {
        let result = write::audition(session, &program, &mut stopping)
            .and_then(|()| capture(session, root, "listen-readback", None, &mut stopping))
            .and_then(|words| store::compare_image(&words, &prepared.program, "Audition read-back"))
            .and_then(|()| {
                announce("Working buffer verified. Play now, then type RETURN TO ORIGINAL.");
                prompt("RETURN TO ORIGINAL")
            });
        return returned(session, root, "listen-return", original, &snapshot, result);
    }

    let committing = Cell::new(false);
    match write::store_observing_commit(session, target, &program, &mut stopping, || {
        committing.set(true)
    }) {
        Ok(()) => {}
        Err(write::StoreError::NoConfirmation) => {
            let warning = spare_warning(target);
            let notice =
                format!("No 0x71. Keep the rig powered and untouched for 2 minutes. {warning}");
            session.note(&notice);
            announce(&notice);
            session.link.pause(store::AFTER_TIMEOUT);
            return Err(format!(
                "Slot state is unknown. Check the guitar, take a Device check backup if it responds, and report with the log. {}",
                warning
            ));
        }
        Err(error) if committing.get() => return Err(format!("{error} {}", spare_warning(target))),
        Err(error) => {
            return returned(
                session,
                root,
                "cancel-return",
                original,
                &snapshot,
                Err(error.to_string()),
            );
        }
    }

    let verified = (|| {
        let words = capture(session, root, "stored-readback", Some(target), || false)?;
        store::compare_image(&words, &prepared.expected, "Stored target read-back")?;
        let settings = variax::read_current_checked_retrying_silence(
            session,
            IDENTITY_RETRY_DELAY,
            variax_identity,
        )?;
        compare_settings(
            &settings.metadata.raw,
            &prepared.expected[METADATA],
            "Stored target settings",
        )
    })();
    returned(
        session,
        root,
        "stored-return",
        original,
        &snapshot,
        verified,
    )
}

#[cfg(test)]
mod tests;

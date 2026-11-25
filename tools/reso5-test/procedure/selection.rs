//! Owner-only #238 procedure. The live adapter keeps each rung separately closed.
pub mod listen_list;
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

fn preset_name(preset: Preset) -> String {
    format!("{} {}", preset.bank_name(), preset.slot() + 1)
}

fn step(announce: &mut impl FnMut(&str), hear: &str, see: &str) {
    announce(&format!("HEAR: {hear}"));
    announce(&format!("SEE: {see}"));
}

const READ_TIME: &str = "about 7 seconds on a clean read";

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
    announce: &mut impl FnMut(&str),
) -> Result<(), String> {
    let name = preset_name(original);
    step(
        announce,
        &format!("Switching back to {name}. It will stay selected after the check."),
        &format!(
            "Wait {READ_TIME} for the tool to verify {name}. Retries can take longer. Nothing to type."
        ),
    );
    let restored = capture(session, root, stage, Some(original), || false)
        .and_then(|words| store::compare_image(&words, snapshot, "Original preset switch-back"));
    if restored.is_ok() {
        announce(&format!("Back on {name}, verified."));
    } else {
        announce(&format!("Could not verify the return to {name}."));
    }
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

fn snapshot<L: Link>(
    session: &mut Session<L>,
    current: &[Image],
    root: &Path,
    announce: &mut impl FnMut(&str),
    stopping: &mut impl FnMut() -> bool,
) -> Result<(Preset, Image), String> {
    session.require_expected_sysex();
    if stopping() {
        return Err("Cancelled before Variax identity.".into());
    }
    step(
        announce,
        &format!("Your current preset stays selected while the tool identifies it in {READ_TIME}."),
        "Wait for the tool to name your starting preset. Retries can take longer. Nothing to type.",
    );
    let device = variax::identify_retrying_silence(session, IDENTITY_RETRY_DELAY)?;
    variax_identity(&device.identity)?;
    let snapshot = pack(&capture(session, root, "snapshot", None, stopping)?);
    let original = identify(&snapshot, current)?;
    session.note(format!("Identified current {}.", slot_label(original)));
    let original_name = preset_name(original);

    announce(&format!("Starting preset is {original_name}, verified."));
    Ok((original, snapshot))
}

pub fn attempt<L: Link>(
    session: &mut Session<L>,
    prepared: &Prepared,
    root: &Path,
    mut prompt: impl FnMut(&str) -> Result<(), String>,
    mut announce: impl FnMut(&str),
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    let (original, snapshot) = snapshot(
        session,
        &prepared.current,
        root,
        &mut announce,
        &mut stopping,
    )?;
    let original_name = preset_name(original);
    let target_name = preset_name(prepared.target);
    if original == prepared.target && !prepared.mode.listens() {
        return Err("Select a different starting preset. Overwriting the original slot prevents a verified switch-back.".into());
    }
    if stopping() {
        return Err("Cancelled before target selection.".into());
    }
    let target = prepared.target;
    let baseline = &prepared.current[usize::from(index(target))];
    step(
        &mut announce,
        &format!(
            "Switching to {target_name} to check it. You will hear it for {READ_TIME}, then {original_name} again."
        ),
        &format!(
            "Wait for the {target_name} check and the return to {original_name}. Nothing to type."
        ),
    );
    let preflight = capture(
        session,
        root,
        "target-preflight",
        Some(target),
        &mut stopping,
    )
    .and_then(|words| store::compare_image(&words, baseline, "Target preflight"));
    if preflight.is_ok() {
        announce(&format!(
            "{target_name} matches the expected current program."
        ));
    } else {
        announce(&format!(
            "Could not verify {target_name}. No upload will be sent."
        ));
    }
    returned(
        session,
        root,
        "preflight-return",
        original,
        &snapshot,
        preflight,
        &mut announce,
    )?;
    if stopping() {
        return Err("Cancelled before uploading.".into());
    }
    let phrase = prepared.confirmation();
    if prepared.mode.listens() {
        step(
            &mut announce,
            &format!(
                "{original_name} stays selected until you confirm. Then the edited sound replaces it while the tool checks the upload and while you play."
            ),
            &format!(
                "Type {phrase} to load the edit temporarily. Wait for Play now before playing. Nothing will be saved."
            ),
        );
    } else {
        step(
            &mut announce,
            &format!(
                "{original_name} stays selected until you confirm. Then the sound may change during upload and the save wait of up to 15 seconds. The tool will check {target_name} and return to {original_name}."
            ),
            &format!(
                "Type {phrase} to save to {target_name}. Wait up to 15 seconds for the save reply, then wait for verification. Do not play or touch the knobs during the store."
            ),
        );
    }
    prompt(&phrase)?;
    if stopping() {
        return Err("Cancelled before uploading.".into());
    }
    let name = sources::metadata(&prepared.program)?.name;
    let program = write::Program::new(&name, &prepared.program)?;

    if prepared.mode.listens() {
        let result = write::audition(session, &program, &mut stopping)
            .and_then(|()| {
                announce("Edited sound uploaded to the working buffer.");
                step(
                    &mut announce,
                    &format!("The edited sound stays loaded during the check, which takes {READ_TIME}."),
                    "Wait for Play now. Retries can take longer. Nothing to type yet.",
                );
                capture(session, root, "listen-readback", None, &mut stopping)
            })
            .and_then(|words| store::compare_image(&words, &prepared.program, "Audition read-back"))
            .and_then(|()| {
                announce("The working buffer matches the edited program.");
                let sound = sources::audition_sound(&snapshot, &prepared.program)?;
                step(
                    &mut announce,
                    &format!("{sound} The edit stays loaded until you finish."),
                    &format!("Play now. Play strings only and do not touch the knobs. When finished, type RETURN TO ORIGINAL to reload and verify {original_name}."),
                );
                prompt("RETURN TO ORIGINAL")
            });
        returned(
            session,
            root,
            "listen-return",
            original,
            &snapshot,
            result,
            &mut announce,
        )?;
        announce(&format!(
            "Done. Sent the edited sound to the working buffer. Nothing was saved. The guitar is on {original_name}."
        ));
        return Ok(());
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
            step(
                &mut announce,
                "The edited sound may remain loaded. Leave the guitar powered and untouched for two minutes.",
                &format!(
                    "No save reply arrived. Wait for the stop message, then check the guitar and follow the backup instructions. {warning}"
                ),
            );
            session.link.pause(store::AFTER_TIMEOUT);
            announce(
                "Stopped. One save command was sent. Whether it saved and which sound is loaded are unknown.",
            );
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
                &mut announce,
            );
        }
    }

    announce(&format!("The guitar confirmed the save to {target_name}."));
    step(
        &mut announce,
        &format!(
            "Reloading {target_name}. You will hear that preset for {READ_TIME}, plus a settings check, then {original_name} again."
        ),
        &format!(
            "The full read takes {READ_TIME}, followed by a settings check. Retries can take longer. Nothing to type."
        ),
    );
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
    if verified.is_ok() {
        announce(&format!(
            "{target_name} matches the expected saved sound and settings."
        ));
    } else {
        announce(&format!("Could not verify the saved {target_name}."));
    }
    returned(
        session,
        root,
        "stored-return",
        original,
        &snapshot,
        verified,
        &mut announce,
    )?;
    let program = if prepared.mode == Mode::Restore {
        "the backup program"
    } else {
        "the edited program"
    };
    announce(&format!(
        "Done. Sent {program} and one save command. Saved and verified {target_name}. The guitar is on {original_name}."
    ));
    Ok(())
}

#[cfg(test)]
mod tests;

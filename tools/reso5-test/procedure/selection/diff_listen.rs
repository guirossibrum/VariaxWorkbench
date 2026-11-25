use super::{
    Image, READ_TIME, capture, index, preset_name, returned, snapshot, sources, step, target,
};
use line6_devices::{
    family::variax::{
        Preset,
        procedures::{self, BufferEdit, PROGRAM_BYTES, compare_image, set_firmware_checksum},
        write::Program,
    },
    session::{Link, Session},
};
use std::path::Path;
use variax_workbench::{ParameterValue, PresetMetadata, preset::PresetFile};

pub struct Prepared {
    preset: Preset,
    current: Vec<Image>,
    edited: Image,
}

impl Prepared {
    pub fn description(&self) -> Result<Vec<String>, String> {
        let original = &self.current[usize::from(index(self.preset))];
        let mut expected = self.edited;
        expected[PROGRAM_BYTES - 3..].copy_from_slice(&original[PROGRAM_BYTES - 3..]);
        let mut lines = vec![format!(
            "diff-listen starts on {}.",
            preset_name(self.preset)
        )];
        lines.extend(sources::diff(original, &self.edited)?);
        lines.push(format!(
            "Starting image SHA-256: {}",
            sources::digest(original)
        ));
        lines.push(format!(
            "Expected working image SHA-256: {}",
            sources::digest(&expected)
        ));
        Ok(lines)
    }
}

pub fn prepare(
    slot: u8,
    backup: &Path,
    archive: &Path,
    manifest: &Path,
) -> Result<Prepared, String> {
    let preset = target(slot)?;
    let (mut current, _, stamped_reso) = sources::load_current_library(backup, archive)?;
    current[50] = stamped_reso;
    let original = &current[usize::from(slot)];
    let metadata =
        PresetMetadata::from_bytes(original[super::METADATA].try_into().expect("metadata size"))?;
    let Some(ParameterValue::Float(trim)) = metadata.parameters.get("trim") else {
        return Err("The saved program has no output trim.".into());
    };
    let quieter = *trim - 6.0;
    if !(-20.0..=6.0).contains(&quieter) {
        return Err("This audition needs a preset whose trim can be lowered by 6 dB.".into());
    }
    let mut settings = PresetFile::from_metadata(&metadata, 0x70004, 0x310)?;
    let models = sources::models(manifest)?;
    let rendered_before = sources::render(&settings, &models)?;
    settings.set_parameter(
        crate::procedure::SYSTEM_SLOT,
        crate::procedure::TRIM_PARAM,
        1,
        quieter.to_bits(),
    )?;
    let rendered_after = sources::render(&settings, &models)?;
    let edited = apply_trim_words(original, &rendered_before, &rendered_after)?;
    Program::new(&metadata.name, &edited)?;
    Ok(Prepared {
        preset,
        current,
        edited,
    })
}

/// Apply the trim's rendered word changes to the exact saved preset. Refuse
/// unless every changed word agrees with the render of the original trim.
fn apply_trim_words(original: &Image, before: &Image, after: &Image) -> Result<Image, String> {
    let mut edited = *original;
    let mut changes = 0;
    for (word, ((dst, previous), next)) in edited
        .as_chunks_mut::<3>()
        .0
        .iter_mut()
        .zip(before.as_chunks::<3>().0)
        .zip(after.as_chunks::<3>().0)
        .take(PROGRAM_BYTES / 3 - 1)
        .enumerate()
    {
        if previous == next {
            continue;
        }
        if dst != previous {
            return Err(format!(
                "Cannot isolate the trim edit: saved word {word} differs from the original render."
            ));
        }
        *dst = *next;
        changes += 1;
    }
    if changes == 0 {
        return Err("The trim edit did not change the program.".into());
    }
    set_firmware_checksum(&mut edited);
    Ok(edited)
}

pub fn attempt<L: Link>(
    session: &mut Session<L>,
    prepared: &Prepared,
    root: &Path,
    mut wait_for_enter: impl FnMut() -> Result<(), String>,
    mut announce: impl FnMut(&str),
    mut stopping: impl FnMut() -> bool,
) -> Result<(), String> {
    let (original, original_image) = snapshot(
        session,
        &prepared.current,
        root,
        &mut announce,
        &mut stopping,
    )?;
    let name = preset_name(prepared.preset);
    if original != prepared.preset {
        return Err(format!(
            "Select {name} by hand before starting diff-listen."
        ));
    }
    if stopping() {
        return Err("Cancelled before the difference upload.".into());
    }
    let program = Program::new(&sources::metadata(&prepared.edited)?.name, &prepared.edited)?;
    step(
        &mut announce,
        &format!("{name} will get quieter by 6 dB; the body, pickups and tuning stay the same."),
        &format!("Wait {READ_TIME} for the full read-back and the Play now prompt."),
    );
    let expected = procedures::send_buffer(
        session,
        BufferEdit::Difference {
            previous: &original_image,
            next: &program,
        },
        &mut stopping,
    )
    .map_err(|error| format!("{error} Select {name} by hand to discard any partial edit."))?;
    let result = capture(session, root, "diff-readback", None, &mut stopping)
        .and_then(|actual| compare_image(&actual, &expected, "Difference read-back"))
        .and_then(|()| {
            announce("All 1,092 words of the difference read-back match.");
            step(
                &mut announce,
                &format!("The quieter {name} sound stays loaded until you finish."),
                &format!("Play now. Play strings only and leave the knobs alone. Press Enter to reload and verify {name}."),
            );
            wait_for_enter()
        });
    returned(
        session,
        root,
        "diff-return",
        original,
        &original_image,
        result,
        &mut announce,
    )?;
    announce(&format!(
        "Done. Sent one difference. Nothing was saved. The guitar is on {name}."
    ));
    Ok(())
}

#[cfg(test)]
impl Prepared {
    pub(super) fn fixture(preset: Preset, current: Vec<Image>, edited: Image) -> Self {
        Self {
            preset,
            current,
            edited,
        }
    }
}

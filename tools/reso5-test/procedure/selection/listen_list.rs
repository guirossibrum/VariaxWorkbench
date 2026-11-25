use super::{Image, READ_TIME, capture, preset_name, returned, snapshot, sources, step, store};
use line6_devices::{
    family::variax::write,
    session::{Link, Session},
};
use std::path::Path;

pub struct Prepared {
    current: Vec<Image>,
    entries: Vec<Entry>,
}

pub(super) struct Entry {
    pub(super) label: String,
    pub(super) program: Image,
}

impl Prepared {
    pub fn description(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "listen-list: {} files. Each is a temporary working-buffer upload.",
            self.entries.len()
        )];
        lines.extend(self.entries.iter().enumerate().map(|(i, entry)| {
            format!(
                "{:02}. {} | upload and expected working image SHA-256: {}",
                i + 1,
                entry.label,
                sources::digest(&entry.program)
            )
        }));
        lines
    }
}

pub fn prepare(
    backup: &Path,
    archive: &Path,
    manifest: &Path,
    files: &[&Path],
) -> Result<Prepared, String> {
    if files.is_empty() {
        return Err("Listen-list requires at least one .vwm file.".into());
    }
    let (mut current, _, stamped_reso) = sources::load_current_library(backup, archive)?;
    current[50] = stamped_reso;
    let models = sources::models(manifest)?;
    let entries = files
        .iter()
        .map(|path| {
            let label = path
                .file_name()
                .ok_or("Supply a .vwm filename.")?
                .to_string_lossy()
                .escape_debug()
                .to_string();
            let program = sources::render_file(path, &models)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            write::Program::new(&sources::metadata(&program)?.name, &program)?;
            Ok::<_, String>(Entry { label, program })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Prepared { current, entries })
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
    let original_name = preset_name(original);
    let mut attempted = false;
    let result = (|| {
        for (i, entry) in prepared.entries.iter().enumerate() {
            if stopping() {
                return Err("Listen-list cancelled before upload.".into());
            }
            let name = sources::metadata(&entry.program)?.name;
            let program = write::Program::new(&name, &entry.program)?;
            let sound = sources::audition_sound(&original_image, &entry.program)?;
            step(
                &mut announce,
                &format!("Loading {}. {sound}", entry.label),
                &format!(
                    "File {} of {}. Wait {READ_TIME} for verification and the Play now prompt.",
                    i + 1,
                    prepared.entries.len()
                ),
            );
            attempted = true;
            write::audition(session, &program, &mut stopping)?;
            let captured = capture(
                session,
                root,
                &format!("listen-list-{:02}", i + 1),
                None,
                &mut stopping,
            )?;
            store::compare_image(
                &captured,
                &entry.program,
                &format!("{} read-back", entry.label),
            )?;
            announce(&format!("{} working buffer verified.", entry.label));
            let next = if i + 1 == prepared.entries.len() {
                format!("reload and verify {original_name}")
            } else {
                format!("load and verify {}", prepared.entries[i + 1].label)
            };
            step(
                &mut announce,
                &format!("{sound} This sound stays loaded until you finish."),
                &format!(
                    "Play now. Play strings only and leave the knobs alone. When finished, press Enter to {next}."
                ),
            );
            wait_for_enter()?;
        }
        Ok(())
    })();
    if attempted {
        returned(
            session,
            root,
            "listen-list-return",
            original,
            &original_image,
            result,
            &mut announce,
        )?;
    } else {
        result?;
    }
    announce(&format!(
        "Done. Auditioned {} files. Nothing was saved. The guitar is on {original_name}.",
        prepared.entries.len()
    ));
    Ok(())
}

#[cfg(test)]
impl Prepared {
    pub(super) fn fixture(current: Vec<Image>, entries: Vec<Entry>) -> Self {
        Self { current, entries }
    }
}

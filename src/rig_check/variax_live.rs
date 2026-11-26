//! Steps 4.2 and 4.3 (#257), Variax live steps without saving.
//!
//! * 4.2: send the current program's own words to the working buffer and
//!   read them back.
//! * 4.3: play test models in the working buffer, ask an A/B question, then
//!   switch back to the starting preset and verify.
//!
//! No store command (`0x61`) is ever sent: the step policy does not include
//! the Variax store operation, and each step also checks its own log.

use super::practice::{Program, packed, preset_label, program_name, seal};
use super::procedures::{
    program_bytes, same_program, variax_full_read, variax_read_preset, with_variax_progress,
    word_differences,
};
use super::runner::{Answer, Ctx, Question};
use line6_devices::family::variax::{
    procedures::{self as shared_variax, BufferEdit},
    write,
};
use line6_devices::gate::Operation;

pub const VARIAX_LIVE: &[Operation] = &[
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxEditBufferWrite,
    Operation::VariaxPresetSwitch,
];

fn audition(ctx: &mut Ctx<'_>, words: &Program, label: &str) -> Result<Program, String> {
    let name = program_name(words);
    let bytes = packed(words);
    let read = with_variax_progress(ctx, VARIAX_LIVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        let program = write::Program::new(&name, &bytes)?;
        shared_variax::send_buffer(session, BufferEdit::Full(&program), stopping)?;
        variax_full_read(session, stopping, progress, label)
    })?;
    let sent = ctx.rec.checksum(
        format!("{label}: sent (1,092 words)"),
        &program_bytes(words),
    );
    let back = ctx
        .rec
        .checksum(format!("{label}: read back"), &program_bytes(&read.words));
    ctx.rec.note(format!(
        "{label}: sent SHA-256 {}, read back {}",
        &sent[..16],
        &back[..16]
    ));
    Ok(read.words)
}

fn no_store_sent(ctx: &Ctx<'_>) -> Result<(), String> {
    let store = ctx
        .rec
        .messages
        .iter()
        .any(|line| line.contains("OUT f0 00 01 0c 07 00 61"));
    if store {
        return Err("A store command was logged during a live step.".into());
    }
    Ok(())
}

pub fn variax_own_bytes(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    let back = audition(ctx, &start, "Own program")?;
    no_store_sent(ctx)?;
    let differences = word_differences(&start, &back);
    if differences.is_empty() {
        ctx.rec
            .show("Sent and read back: all 1,092 words identical. No store command was sent.");
        Ok(())
    } else {
        Err(format!(
            "The working buffer differs from what was sent at words {differences:?}."
        ))
    }
}

/// A bundled test model: the starting program with obvious, audible
/// changes. Built from the guitar's own program, so no vendor data is
/// bundled. Alternate tuning: string 6 down a whole tone (drop D), a change
/// any player hears at once.
pub fn test_model(start: &Program) -> Program {
    let mut words = *start;
    // Settings bytes: alternate tuning flag at offset 68, string tunings from 69.
    let mut settings = super::procedures::settings_of(&words);
    settings[68] = 1;
    settings[69..75].copy_from_slice(&[0xfe, 0, 0, 0, 0, 0]);
    for (i, chunk) in settings.chunks(3).enumerate() {
        words[851 + i] = u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]);
    }
    // Pitch shift parameters for the low string (words 680-685).
    words[680] ^= 0x10_0000;
    seal(&mut words);
    words
}

/// The test models offered in step 4.3. Practice mode uses a synthetic
/// drop-D change of the guitar's own program. Real mode uses only models
/// built on this computer that matched their reference in step 0.3, so no
/// unverified program is ever sent to a real guitar.
fn test_models(ctx: &Ctx<'_>, start: &Program) -> Vec<(String, Program, String)> {
    const GENERIC: &str = "Now play the test model (B) and compare it with the sound you just played (A). Is it clearly different (another body, 12-string, tuning or sitar)?";
    if ctx.mode == super::record::Mode::Practice {
        return vec![(
            "Practice test model (drop D)".into(),
            test_model(start),
            "Now play the test model (B) and compare it with the sound you just played (A). Is the lowest string tuned lower (drop D)?".into(),
        )];
    }
    ctx.data
        .test_models
        .iter()
        .map(|(name, words, question)| {
            (
                name.clone(),
                *words,
                question.clone().unwrap_or_else(|| GENERIC.into()),
            )
        })
        .collect()
}

pub fn variax_test_model(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    let preset = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let models = test_models(ctx, &start);
    if models.is_empty() {
        return Err("No verified test model: add your reference folder to step 0.3 (--engine-folder) so test models are built and checked on this computer first.".into());
    }
    super::guided::play_phrase(
        ctx,
        "Strum all six strings on your starting sound (A). Then press Continue.",
        "Your starting sound.",
    )?;
    let mut result = Ok(());
    for (name, model, question_text) in models {
        if ctx.stopping() {
            result = Err("Stopped by you.".to_string());
            break;
        }
        let back = match audition(ctx, &model, &name) {
            Ok(back) => back,
            Err(error) => {
                result = Err(error);
                break;
            }
        };
        if back != model {
            result = Err(format!(
                "{name}: the working buffer differs from the program that was sent."
            ));
            break;
        }
        if let Err(error) = super::guided::play_phrase(
            ctx,
            "Strum all six strings the same way with the test sound (B). Then press Continue.",
            &format!("{name}, the temporary test sound."),
        ) {
            result = Err(error);
            break;
        }
        let question = Question {
            step: ctx.rec.id,
            text: question_text,
            verified_by_bytes: vec![
                format!("{name} is in the working buffer: all 1,092 words read back"),
                "subtle settings (one pickup, tone capacitor, pot values) are proven by this byte check, not by ear".into(),
            ],
        };
        let (answer, note) = ctx.op.ask(&question);
        ctx.rec.answers.push((
            format!("{name}: {}", question.text),
            if note.is_empty() {
                answer.label().to_owned()
            } else {
                format!("{} — note: {note}", answer.label())
            },
        ));
        ctx.rec.show(format!(
            "{name}: you answered {} (the byte check already proves the upload).",
            answer.label()
        ));
        if answer == Answer::No {
            ctx.rec.note(format!(
                "{name}: you did not hear the change although the bytes arrived; the note describes what you heard."
            ));
        }
    }
    // Always switch back, also after a failure or Stop.
    let returned = with_variax_progress(ctx, VARIAX_LIVE, |session, _, progress| {
        variax_read_preset(
            session,
            preset,
            &|| false,
            progress,
            "Verifying the return to your starting preset",
        )
    });
    no_store_sent(ctx)?;
    match returned {
        Ok(returned) if same_program(&returned.words, &start) => ctx.rec.show(format!(
            "Back on {}: all 1,092 words match the start. Nothing was saved.",
            preset_label(preset)
        )),
        Ok(_) => {
            return Err(format!(
                "Switched back to {}, but the program differs from the start.",
                preset_label(preset)
            ));
        }
        Err(error) => {
            return Err(format!(
                "Could not switch back to {}: {error}",
                preset_label(preset)
            ));
        }
    }
    result
}

#[derive(Clone, Copy)]
pub(super) enum AuditionEdit {
    Trim,
    Tuning,
}

/// Build a selective edit of the captured program. Real candidates are rendered
/// with the same host options as the editor. Only words verified against the
/// unmodified render are carried over from the edited render.
pub(super) fn edit_images(
    ctx: &Ctx<'_>,
    original: &Program,
    edit: AuditionEdit,
) -> Result<(shared_variax::Image, Option<shared_variax::Image>), String> {
    if ctx.mode == super::record::Mode::Practice {
        let mut next = *original;
        return Ok(match edit {
            AuditionEdit::Trim => {
                next[10] ^= 0x1000;
                seal(&mut next);
                (packed(&next), None)
            }
            AuditionEdit::Tuning => {
                next = test_model(original);
                let mut muted = next;
                muted[10] = 0;
                seal(&mut muted);
                (packed(&next), Some(packed(&muted)))
            }
        });
    }

    let root = ctx.data.engine_folder.as_deref().ok_or(
        "Supply --engine-folder with a verified local models.tsv for the Variax edit checks.",
    )?;
    render_edits(root, original, edit)
}

fn render_edits(
    root: &std::path::Path,
    original: &Program,
    edit: AuditionEdit,
) -> Result<(shared_variax::Image, Option<shared_variax::Image>), String> {
    use crate::{
        ParameterValue, PresetMetadata, edit as field,
        host::{HostEngine, HostOptions, HostParameters, ModelLibrary},
        preset::PresetFile,
    };
    let original = packed(original);
    let metadata = PresetMetadata::from_bytes(
        original[shared_variax::METADATA]
            .try_into()
            .expect("settings size"),
    )?;
    let mut preset = PresetFile::from_metadata(&metadata, 0x0007_0004, 0x0310_0000)?;
    let models = ModelLibrary::load_manifest(&root.join("models.tsv"))?;
    let render = |preset: &PresetFile, muted| -> Result<shared_variax::Image, String> {
        let parameters = HostParameters::from_preset(preset)?;
        let settings = parameters.map(HostOptions {
            muted,
            ..HostOptions::default()
        })?;
        let result = HostEngine::new(&models).generate(&settings)?;
        Ok(parameters.finish(preset.name(), &settings, result.packed()))
    };
    let before = render(&preset, false)?;
    match edit {
        AuditionEdit::Trim => {
            let Some(ParameterValue::Float(trim)) = metadata.parameters.get("trim") else {
                return Err("The starting preset has no output trim.".into());
            };
            preset.set_parameter(field::SYSTEM, 0x200000, 1, (trim - 6.0).to_bits())?;
        }
        AuditionEdit::Tuning => {
            if metadata.parameters.get("alternate_tuning_enabled")
                != Some(&ParameterValue::Boolean(false))
            {
                return Err("Start on an electric preset with alternate tuning disabled.".into());
            }
            for string in 0..6 {
                let semitones = if string == 0 { -2i32 } else { 0 };
                preset.set_parameter(field::TUNING, 0x200000 + string, 0, semitones as u32)?;
            }
            preset.set_enabled(field::TUNING, true)?;
        }
    }
    let next = merge_rendered_words(&original, &before, &render(&preset, false)?)?;
    let muted = if matches!(edit, AuditionEdit::Tuning) {
        Some(merge_rendered_words(
            &original,
            &before,
            &render(&preset, true)?,
        )?)
    } else {
        None
    };
    Ok((next, muted))
}

fn merge_rendered_words(
    original: &shared_variax::Image,
    before: &shared_variax::Image,
    after: &shared_variax::Image,
) -> Result<shared_variax::Image, String> {
    let mut candidate = *original;
    for (index, (word, (old, new))) in candidate
        .as_chunks_mut::<3>()
        .0
        .iter_mut()
        .zip(
            before
                .as_chunks::<3>()
                .0
                .iter()
                .zip(after.as_chunks::<3>().0),
        )
        .take(1091)
        .enumerate()
    {
        if old != new {
            if word != old {
                return Err(format!(
                    "The starting program and its Workbench render differ at affected word {index}. Nothing was sent."
                ));
            }
            *word = *new;
        }
    }
    if candidate[..shared_variax::METADATA.start] == original[..shared_variax::METADATA.start] {
        return Err("The edit did not change any DSP words. Nothing was sent.".into());
    }
    shared_variax::set_firmware_checksum(&mut candidate);
    Ok(candidate)
}

pub fn trim(ctx: &mut Ctx<'_>) -> Result<(), String> {
    variant(
        ctx,
        AuditionEdit::Trim,
        "Play the same short passage. Does it sound quieter?",
    )
}

pub fn tuning(ctx: &mut Ctx<'_>) -> Result<(), String> {
    variant(
        ctx,
        AuditionEdit::Tuning,
        "Play the low E string. Does it sound lower than before?",
    )
}

fn variant(ctx: &mut Ctx<'_>, edit: AuditionEdit, question: &str) -> Result<(), String> {
    let start = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    let preset = ctx
        .data
        .start_preset
        .ok_or("No verified starting preset (3.4).")?;
    let (next, muted) = edit_images(ctx, &start, edit)?;
    let name = program_name(&start);
    let next = write::Program::new(&name, &next)?;
    let transition = muted
        .as_ref()
        .map(|bytes| write::Program::new(&name, bytes))
        .transpose()?;
    let phrase = match edit {
        AuditionEdit::Trim => "Play a short phrase, then press Continue.",
        AuditionEdit::Tuning => "Pluck the low E string, then press Continue.",
    };
    super::guided::play_phrase(ctx, phrase, "Your starting sound.")?;
    let mut attempted = false;
    let mut sent = with_variax_progress(ctx, VARIAX_LIVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        let baseline =
            variax_full_read(session, stopping, progress, "Checking the starting program")?.words;
        shared_variax::compare_image(&baseline, &packed(&start), "Starting program")?;
        let previous = packed(&baseline);
        let edit = match &transition {
            Some(transition) => BufferEdit::AlternateTuning {
                previous: &previous,
                transition,
                full: &next,
            },
            None => BufferEdit::Difference {
                previous: &previous,
                next: &next,
            },
        };
        attempted = true;
        shared_variax::send_buffer_verified(session, edit, "Audition read-back", stopping, || {
            let mut advance =
                super::procedures::program_progress(progress, "Verifying the temporary edit");
            Ok(move |_| {
                advance();
                Ok(())
            })
        })?;
        Ok(())
    });
    if sent.is_ok() {
        ctx.rec.show("The edited working buffer was verified.");
        sent = super::guided::play_phrase(ctx, phrase, "Your temporary edited sound.");
    }
    if sent.is_ok() {
        let (answer, note) = ctx.op.ask(&Question {
            step: ctx.rec.id,
            text: question.into(),
            verified_by_bytes: vec![
                "The complete working buffer matched the expected edit.".into(),
            ],
        });
        ctx.rec
            .answers
            .push((question.into(), format!("{} {note}", answer.label())));
    }
    if sent
        .as_ref()
        .is_err_and(|error| error.contains("Reopen the link"))
    {
        return Err(format!(
            "{} Reselect {} by hand to discard any partial edit.",
            sent.unwrap_err(),
            preset_label(preset)
        ));
    }
    if attempted {
        let returned = with_variax_progress(ctx, VARIAX_LIVE, |session, _, progress| {
            let mut advance = super::procedures::program_progress(
                progress,
                "Verifying the return to your starting preset",
            );
            shared_variax::return_to_preset(session, preset, &packed(&start), |_| {
                advance();
                Ok(())
            })
        });
        match (sent, returned) {
            (Ok(()), Ok(())) => ctx.rec.show(format!(
                "Back on {}, original program verified.",
                preset_label(preset)
            )),
            (Err(error), Ok(())) => {
                return Err(format!("{error} The original preset was restored."));
            }
            (Ok(()), Err(error)) => return Err(format!("Could not verify the return: {error}")),
            (Err(error), Err(restore)) => {
                return Err(format!("{error} Return also failed: {restore}"));
            }
        }
    } else {
        sent?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::{Faults, synthetic_program};
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    #[test]
    fn the_test_model_is_a_valid_program_that_changes_only_tuning() {
        let start = synthetic_program("Spank 1", 3);
        let model = test_model(&start);
        assert!(write::Program::new("Spank 1", &packed(&model)).is_ok());
        let changed: Vec<usize> = word_differences(&start, &model);
        assert!(
            changed.iter().all(|&w| matches!(w, 680 | 851..=914 | 1091)),
            "{changed:?}"
        );
    }

    #[test]
    fn live_steps_restore_and_never_store() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        for id in [
            StepId(4, 1),
            StepId(4, 2),
            StepId(4, 3),
            StepId(4, 4),
            StepId(4, 5),
        ] {
            assert!(run.step(id).outcome.passed(), "{id}: {:?}", run.step(id));
            assert!(
                !run.step(id)
                    .messages
                    .iter()
                    .any(|l| l.contains("OUT f0 00 01 0c 07 00 61"))
            );
        }
        assert!(
            run.step(StepId(4, 1))
                .evidence
                .iter()
                .any(|l| l.contains("byte for byte"))
        );
        let listen = run.step(StepId(4, 3));
        assert_eq!(listen.answers.len(), 1);
        assert!(
            listen.answers[0]
                .0
                .contains("compare it with the sound you just played (A)")
        );
        assert_eq!(person.questions[0].verified_by_bytes.len(), 2);
        let rig = rig.borrow();
        // Three full live uploads (4.2, 4.3, 4.5) and five saves. The
        // ordinary difference is a separate message.
        assert_eq!(rig.variax.working_uploads, 8);
        assert_eq!(rig.variax.stores, 5);
    }
    #[test]
    #[ignore = "requires the owner's local Workbench manifest and verified slot-14 backup"]
    fn private_spank_render_builds_trim_and_tuning_frames() {
        let root = std::path::PathBuf::from(std::env::var_os("RIG_CHECK_REFERENCE_DIR").unwrap());
        let backup = std::path::PathBuf::from(std::env::var_os("RIG_CHECK_SPANK_BACKUP").unwrap());
        let bytes = std::fs::read(backup).unwrap();
        let words = std::array::from_fn(|index| {
            let [a, b, c] = bytes.as_chunks::<3>().0[index];
            u32::from_be_bytes([0, a, b, c])
        });
        let name = program_name(&words);
        for edit in [super::AuditionEdit::Trim, super::AuditionEdit::Tuning] {
            let (next, muted) = super::render_edits(&root, &words, edit).unwrap();
            write::Program::new(&name, &next).unwrap();
            if let Some(muted) = muted {
                assert_ne!(next, muted);
                write::Program::new(&name, &muted).unwrap();
            }
        }
    }
}

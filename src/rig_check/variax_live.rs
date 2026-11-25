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
    program_bytes, same_program, variax_full_read, variax_read_preset, with_variax,
    word_differences,
};
use super::runner::{Answer, Ctx, Question};
use line6_devices::family::variax::write;
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
    let read = with_variax(ctx, VARIAX_LIVE, |session, stopping| {
        let program = write::Program::new(&name, &bytes)?;
        write::audition(session, &program, stopping)?;
        variax_full_read(session, stopping)
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
    let mut result = Ok(());
    for (name, model, question_text) in models {
        if ctx.stopping() {
            result = Err("Stopped by you.".to_string());
            break;
        }
        ctx.op.instruct(
            ctx.rec.id,
            "Play your own sound now (A): strum all six strings.",
            "Your own sound, unchanged.",
        );
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
    let returned = with_variax(ctx, VARIAX_LIVE, |session, _| {
        variax_read_preset(session, preset, &|| false)
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
        for id in [StepId(4, 1), StepId(4, 2), StepId(4, 3)] {
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
        // Two auditions (4.2, 4.3) and the program sent before the 5.1 store.
        assert_eq!(rig.variax.working_uploads, 3);
        assert_eq!(rig.variax.stores, 1, "only the save step 5.1 stores");
    }
}

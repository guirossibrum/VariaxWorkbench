//! Variax read steps 3.2 and 3.5 (#255), plus byte-loss accounting for
//! the 35-chunk reads. Steps 3.1, 3.3 and 3.4 are in `steps.rs`.

use super::differences::{self, Severity};
use super::practice::{Program, preset_label};
use super::procedures::{
    self, VARIAX_READ, VARIAX_SELECT, program_bytes, variax_read_preset, with_variax,
    with_variax_progress,
};
use super::runner::{Ctx, Progress};
use line6_devices::family::variax::{ParameterValue, Preset, procedures as shared_variax};
use line6_devices::midi::hex;

/// Bytes missing from damaged `0x3C` replies that were retried, from the
/// session log ("... reply has N bytes, expected M").
pub fn lost_bytes(messages: &[String]) -> usize {
    messages
        .iter()
        .filter(|line| line.contains("Retrying"))
        .filter_map(|line| {
            let has = line.split("reply has ").nth(1)?;
            let (n, rest) = has.split_once(" bytes, expected ")?;
            let m: String = rest.chars().take_while(char::is_ascii_digit).collect();
            Some(m.parse::<usize>().ok()?.saturating_sub(n.parse().ok()?))
        })
        .sum()
}

pub fn flag_unverified(ctx: &mut Ctx<'_>) {
    if ctx.data.variax_verified == Some(false) {
        ctx.rec.note(
            "Flagged: unverified device. Read-only steps run; live and save steps are skipped.",
        );
    }
}

fn describe(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Integer(v) => v.to_string(),
        ParameterValue::Boolean(v) => v.to_string(),
        ParameterValue::Float(v) => format!("{v:.4}"),
    }
}

pub fn current_settings(ctx: &mut Ctx<'_>) -> Result<(), String> {
    flag_unverified(ctx);
    let current = with_variax(ctx, VARIAX_READ, |session, _| {
        shared_variax::read_settings_checked(session, |_| Ok(()))
    })?;
    let metadata = current.metadata;
    ctx.rec.show(format!("Preset name: {:?}", metadata.name));
    for (index, reply) in current.replies.iter().enumerate() {
        ctx.rec.note(format!(
            "Reply {} ({} bytes): {}",
            index + 1,
            reply.len(),
            hex(reply)
        ));
    }
    for (key, value) in &metadata.parameters {
        ctx.rec.note(format!("  {key} = {}", describe(value)));
    }
    ctx.rec
        .checksum("Variax current settings (192 bytes)", &metadata.raw);
    ctx.data.start_settings = Some(metadata.raw);
    Ok(())
}

/// Extra evidence for step 3.3: lost bytes and the settings cross-check.
pub fn after_full_read(ctx: &mut Ctx<'_>, words: &Program) -> Result<(), String> {
    flag_unverified(ctx);
    let lost = lost_bytes(&ctx.rec.messages);
    ctx.rec.note(format!(
        "Damaged replies retried: {}; bytes lost in them: {lost}.",
        ctx.rec.retries
    ));
    if let Some(settings) = ctx.data.start_settings {
        let from_program = procedures::settings_of(words);
        let mut content = Vec::new();
        let mut live = Vec::new();
        for (index, (&expected, &actual)) in settings.iter().zip(&from_program).enumerate() {
            if expected == actual {
                continue;
            }
            let xor = expected ^ actual;
            let line = format!(
                "settings byte {index} (word {}) · expected 0x{expected:02X} · actual 0x{actual:02X} · XOR 0x{xor:02X} · {} bit(s)",
                851 + index / 3,
                xor.count_ones()
            );
            if (61..64).contains(&index) {
                live.push(line);
            } else {
                content.push(line);
            }
        }
        if !live.is_empty() {
            ctx.rec.difference(
                Severity::Info,
                "Variax tone position moved between reads",
                live,
            );
        }
        if !content.is_empty() {
            ctx.rec.difference(
                Severity::Stop,
                format!(
                    "{} settings bytes differ between 3.2 and 3.3",
                    content.len()
                ),
                content,
            );
            return Err("The Variax settings changed during the starting snapshot.".into());
        }
        ctx.rec.note(
            "Words 851-914 match the settings read in 3.2 (tone-position bytes 61-63 excepted).",
        );
    }
    Ok(())
}

fn backup_of(ctx: &Ctx<'_>, preset: Preset) -> Option<Program> {
    ctx.data
        .variax_library
        .iter()
        .find(|(p, _)| *p == preset)
        .map(|(_, w)| *w)
}

/// Read a saved preset with the editor's procedure and verify the return.
pub fn one_preset(ctx: &mut Ctx<'_>) -> Result<(), String> {
    flag_unverified(ctx);
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let start_program = ctx
        .data
        .exercise_program()
        .ok_or("No starting program (3.3).")?;
    let preset = other_preset(start);
    let label = preset_label(preset);
    let mut problems = Vec::new();
    let count = if ctx.stopping() {
        problems.push("Stopped by you.".to_string());
        0
    } else {
        ctx.op.progress(Progress::counted(
            format!("Reading {label}"),
            0,
            1,
            "presets",
        ));
        let read = with_variax_progress(ctx, VARIAX_SELECT, |session, stopping, progress| {
            variax_read_preset(
                session,
                preset,
                stopping,
                progress,
                &format!("Reading {label}"),
            )
        });
        match read {
            Ok(read) => {
                let hash = ctx.rec.checksum(
                    format!("Variax {label} (1,092 words)"),
                    &program_bytes(&read.words),
                );
                let matches = backup_of(ctx, preset).is_some_and(|stored| {
                    differences::program(
                        ctx.rec,
                        &label,
                        &stored,
                        &read.words,
                        Severity::Stop,
                        false,
                    )
                });
                let observation = if matches {
                    "matches the backup"
                } else {
                    "differs from the backup"
                };
                ctx.rec.result.push(format!("Read {label}: {observation}."));
                ctx.rec.note(format!(
                    "{label}: SHA-256 {}, {observation}{}",
                    &hash[..16],
                    if matches { " word for word" } else { "" }
                ));
                if !matches {
                    problems.push(format!("{label} differs from the backup."));
                }
                1
            }
            Err(error) => {
                problems.push(format!("{label}: {error}"));
                0
            }
        }
    };
    ctx.rec.result.push(format!("Read {count} of 1 presets."));
    ctx.op.progress(Progress::counted(
        format!("Returning to {}", preset_label(start)),
        count,
        1,
        "presets",
    ));
    let back = with_variax_progress(ctx, VARIAX_SELECT, |session, stopping, progress| {
        variax_read_preset(
            session,
            start,
            stopping,
            progress,
            &format!("Verifying return to {}", preset_label(start)),
        )
    });
    match back {
        Ok(back)
            if differences::program(
                ctx.rec,
                "Variax read-unmodified return",
                &start_program,
                &back.words,
                Severity::Stop,
                true,
            ) =>
        {
            ctx.rec.show(format!(
                "Back on {}: all 1,092 words match the start.",
                preset_label(start)
            ))
        }
        Ok(_) => problems.push(format!(
            "Switched back to {}, but the program differs from the start.",
            preset_label(start)
        )),
        Err(error) => problems.push(format!(
            "Could not switch back to {}: {error}",
            preset_label(start)
        )),
    }
    let lost = lost_bytes(&ctx.rec.messages);
    ctx.rec.note(format!(
        "Damaged replies retried: {}; bytes lost in them: {lost}.",
        ctx.rec.retries
    ));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join(" "))
    }
}

/// The preset read by 3.5: the next one in the starting preset's bank.
pub fn other_preset(start: Preset) -> Preset {
    Preset::new(start.bank(), (start.slot() + 1) % 5).expect("valid slot")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    #[test]
    fn lost_bytes_come_from_the_retry_lines() {
        let lines = vec![
            "  1.0s Retrying Variax 0x3C start 64, count 32 (attempt 2/3): Variax 0x3C reply has 199 bytes, expected 200.".to_string(),
            "  2.0s OUT f0".to_string(),
        ];
        assert_eq!(lost_bytes(&lines), 1);
    }

    #[test]
    fn variax_reads_compare_with_the_backup_and_return() {
        let mut person = Script::run_all();
        let faults = Faults {
            damage_span_reply: Some(40),
            ..Faults::default()
        };
        let (run, _, _) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        let settings = run.step(StepId(3, 2));
        assert!(
            settings
                .evidence
                .iter()
                .any(|l| l == "Preset name: \"Spank 1\"")
        );
        let full = run.step(StepId(3, 3));
        assert!(
            full.evidence
                .iter()
                .any(|l| l.starts_with("Words 851-914 match"))
        );
        let one = run.step(StepId(3, 5));
        assert!(
            one.evidence
                .iter()
                .any(|l| l.starts_with("Spank 2: SHA-256")
                    && l.ends_with("matches the backup word for word"))
        );
        assert!(
            one.evidence
                .iter()
                .any(|l| l == "Back on Spank 1: all 1,092 words match the start.")
        );
        let all = run.step(StepId(3, 4));
        assert!(
            all.retries >= 1,
            "the damaged reply was retried during the backup"
        );
        assert!(all.outcome.passed());
    }

    #[test]
    fn a_failed_podxt_identity_skips_every_variax_step() {
        use crate::rig_check::catalogue::{STEPS, is_variax_step};
        let mut person = Script::run_all();
        let faults = Faults {
            podxt_identity_silent: true,
            ..Faults::default()
        };
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        assert!(matches!(run.step(StepId(2, 1)).outcome, Outcome::Failed(_)));
        for step in STEPS.iter().filter(|step| is_variax_step(step.id)) {
            assert!(step.needs.contains(&StepId(2, 1)), "{}", step.id);
            assert!(
                matches!(&run.step(step.id).outcome,
                Outcome::SkippedNeeds(reason) if reason.contains("2.1")),
                "{}",
                step.id
            );
            assert!(!person.cards.contains(&step.id), "{}", step.id);
        }
        assert!(
            !rig.borrow()
                .written
                .iter()
                .any(|(path, _)| *path == "VariaxViaPodxt")
        );
    }

    #[test]
    fn an_unverified_podxt_allows_only_read_only_variax_evidence() {
        use crate::rig_check::catalogue::{STEPS, VARIAX_EVIDENCE, is_variax_step};
        let mut person = Script::run_all();
        let faults = Faults {
            unverified_podxt: true,
            ..Faults::default()
        };
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        assert!(run.step(StepId(2, 1)).outcome.passed());
        assert!(run.step(StepId(2, 7)).outcome.passed());
        assert!(run.step(StepId(2, 7)).messages.is_empty());
        assert!(
            run.step(StepId(2, 7))
                .technical_lines()
                .any(|line| line.contains("XOR 0x06"))
        );
        assert!(matches!(
            run.step(StepId(2, 5)).outcome,
            Outcome::SkippedNotApplicable(_)
        ));
        let evidence = run.step(VARIAX_EVIDENCE);
        assert!(evidence.outcome.passed(), "{:?}", evidence.outcome);
        assert!(
            person
                .notices
                .iter()
                .any(|notice| notice == super::super::variax_evidence::RELAY_NOTICE)
        );
        assert!(
            evidence
                .evidence
                .iter()
                .any(|line| line.contains("PODxt Live firmware 2.07") && line.contains("3.01"))
        );
        assert!(
            evidence
                .messages
                .iter()
                .any(|line| line.contains("OUT f0 00 01 0c 07 00 3c"))
        );
        for step in STEPS
            .iter()
            .filter(|step| is_variax_step(step.id) && step.id != VARIAX_EVIDENCE)
        {
            assert!(matches!(
                run.step(step.id).outcome,
                Outcome::SkippedNotApplicable(_) | Outcome::SkippedNeeds(_)
            ));
            assert!(!person.cards.contains(&step.id));
        }
        assert!(
            !rig.borrow()
                .written
                .iter()
                .any(|(path, _)| *path == "VariaxViaPodxt")
        );
        assert_eq!(rig.borrow().variax.stores, 0);
    }
}

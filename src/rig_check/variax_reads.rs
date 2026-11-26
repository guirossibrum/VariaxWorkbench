//! Variax read steps 3.2, 3.5 and 3.6 (#255), plus byte-loss accounting for
//! the 35-chunk reads. Steps 3.1, 3.3 and 3.4 are in `steps.rs`.

use super::practice::{Program, preset_label};
use super::procedures::{
    self, VARIAX_READ, VARIAX_SELECT, program_bytes, same_program, variax_read_preset, with_variax,
};
use super::runner::Ctx;
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
pub fn after_full_read(ctx: &mut Ctx<'_>, words: &Program) {
    flag_unverified(ctx);
    let lost = lost_bytes(&ctx.rec.messages);
    ctx.rec.note(format!(
        "Damaged replies retried: {}; bytes lost in them: {lost}.",
        ctx.rec.retries
    ));
    if let Some(settings) = ctx.data.start_settings {
        let from_program = procedures::settings_of(words);
        let same = settings[..61] == from_program[..61] && settings[64..] == from_program[64..];
        ctx.rec.note(if same {
            "Words 851-914 match the settings read in 3.2 (tone-position bytes 61-63 excepted)."
        } else {
            "Words 851-914 DIFFER from the settings read in 3.2: the preset changed between the two reads."
        });
    }
}

fn backup_of(ctx: &Ctx<'_>, preset: Preset) -> Option<Program> {
    ctx.data
        .variax_library
        .iter()
        .find(|(p, _)| *p == preset)
        .map(|(_, w)| *w)
}

/// Read presets, compare each with the backup, then return to the start.
fn read_and_return(ctx: &mut Ctx<'_>, presets: &[Preset]) -> Result<(), String> {
    flag_unverified(ctx);
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let start_program = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    let mut problems = Vec::new();
    let mut read_count = 0;
    for &preset in presets {
        if ctx.stopping() {
            problems.push("Stopped by you.".to_string());
            break;
        }
        let read = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
            variax_read_preset(session, preset, stopping)
        });
        match read {
            Ok(read) => {
                read_count += 1;
                let hash = ctx.rec.checksum(
                    format!("Variax {} (1,092 words)", preset_label(preset)),
                    &program_bytes(&read.words),
                );
                let matches = backup_of(ctx, preset).is_some_and(|b| b == read.words);
                ctx.rec.result.push(format!(
                    "Read {}: {}.",
                    preset_label(preset),
                    if matches {
                        "matches the backup"
                    } else {
                        "differs from the backup"
                    }
                ));
                ctx.rec.note(format!(
                    "{}: SHA-256 {}, {}",
                    preset_label(preset),
                    &hash[..16],
                    if matches {
                        "matches the backup word for word"
                    } else {
                        "DIFFERS from the backup"
                    }
                ));
                if !matches {
                    problems.push(format!("{} differs from the backup.", preset_label(preset)));
                }
            }
            Err(error) => {
                problems.push(format!("{}: {error}", preset_label(preset)));
                break;
            }
        }
    }
    ctx.rec
        .result
        .push(format!("Read {read_count} of {} presets.", presets.len()));
    let back = with_variax(ctx, VARIAX_SELECT, |session, stopping| {
        variax_read_preset(session, start, stopping)
    });
    match back {
        Ok(back) if same_program(&back.words, &start_program) => ctx.rec.show(format!(
            "Back on {}: all 1,092 words match the start.",
            preset_label(start)
        )),
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

pub fn one_preset(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    read_and_return(ctx, &[other_preset(start)])
}

pub fn one_bank(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let bank: Vec<Preset> = (0..5)
        .map(|slot| Preset::new(start.bank(), slot).expect("valid slot"))
        .collect();
    read_and_return(ctx, &bank)
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
        let bank = run.step(StepId(3, 6));
        assert_eq!(bank.checksums.len(), 5);
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
        assert!(run.step(StepId(2, 5)).outcome.passed());
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
                .any(|line| line.contains("PODxt Live firmware 2.00") && line.contains("3.01"))
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
                Outcome::UnverifiedDevice(_) | Outcome::SkippedNeeds(_)
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

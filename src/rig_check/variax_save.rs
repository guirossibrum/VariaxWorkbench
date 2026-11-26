//! Step 5.1 (#258): save the starting preset's own bytes back to its slot.
//!
//! The verified sequence from #177 and #238 4c, through the shared library:
//! check the slot against the 3.4 backup, show the save notice and require
//! the typed confirmation, send the full program (`0x43`), wait 100 ms, send
//! one store (`0x61`), wait at most 15 s for one `0x71`, then read the slot
//! back (35 chunks) and compare it with what was sent. A save is never
//! repeated. A missing confirmation follows the silent-timeout procedure:
//! no more MIDI, the rig kept untouched for 2 minutes, then instructions.

use super::catalogue::{SaveNotice, StepId};
use super::practice::{Program, flash_index, packed, preset_label, program_name};
use super::procedures::{
    program_bytes, program_progress, same_program, variax_read_preset, with_variax_progress,
};
use super::runner::{Ctx, Progress, SKIPPED};
use line6_devices::family::variax::{
    Preset,
    procedures::{self as shared_variax, SILENT_STORE_WAIT, StoreWait, VerifiedStoreError},
    write,
};
use line6_devices::gate::Operation;

pub const VARIAX_SAVE: &[Operation] = &[
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
    Operation::VariaxStoreRestore,
];
pub const SPARE_WARNING: &str =
    "Do not save any slot in flash 40-59 until spare sector 62 is known blank (#177).";

pub fn confirmation(preset: Preset) -> String {
    format!("STORE VARIAX {}", preset_label(preset))
}

/// The slot step 5.1 saves: the starting preset, outside flash 40-59.
pub fn slot(start: Preset) -> Result<Preset, String> {
    if (40..60).contains(&flash_index(start)) {
        return Err(format!(
            "The starting preset {} is flash slot {}. {SPARE_WARNING} Start on a preset in another bank to run this step.",
            preset_label(start),
            flash_index(start)
        ));
    }
    Ok(start)
}

fn store_progress(progress: &dyn Fn(Progress), wait: StoreWait<'_>) {
    let text = match wait {
        StoreWait::Confirmation => "Waiting for the guitar to confirm, up to 15 s",
        StoreWait::Silent { .. } => {
            "The guitar did not confirm. Leave the rig powered and connected. Rig Check is waiting two more minutes before you check the guitar."
        }
        StoreWait::ReadBack => "Reading back the saved program",
    };
    progress(wait.limit().map_or_else(
        || Progress::action(text),
        |limit| Progress::waiting(text, limit, false),
    ));
}

pub fn save_own_bytes(ctx: &mut Ctx<'_>) -> Result<(), String> {
    require_evidence(ctx)?;
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let preset = slot(start)?;
    let backup: Program = ctx
        .data
        .variax_library
        .iter()
        .find(|(p, _)| *p == preset)
        .map(|(_, w)| *w)
        .ok_or("The slot is not in the 3.4 backup.")?;
    let backup_hash = super::record::sha256_hex(&program_bytes(&backup));
    // Pre-check: the slot still holds what the backup says.
    let before = with_variax_progress(ctx, VARIAX_SAVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        variax_read_preset(
            session,
            preset,
            stopping,
            progress,
            "Checking the save target",
        )
    })?;
    if !same_program(&before.words, &backup) {
        return Err(format!(
            "{} no longer matches the 3.4 backup. Nothing was saved.",
            preset_label(preset)
        ));
    }
    ctx.rec.note(format!(
        "Pre-check: {} matches the 3.4 backup (SHA-256 {}).",
        preset_label(preset),
        &backup_hash[..16]
    ));
    let notice = SaveNotice {
        device: "Variax",
        slot: format!(
            "{} (flash slot {})",
            preset_label(preset),
            flash_index(preset)
        ),
        backup_step: StepId(3, 4),
        backup_file: format!(
            "{}/{:02}-{}.bin",
            backup_folder(ctx),
            flash_index(preset),
            super::evidence::file_name(&preset_label(preset))
        ),
        backup_sha256: backup_hash.clone(),
        confirmation: confirmation(preset),
    };
    match ctx.op.confirm_save(&notice) {
        Some(typed) if typed == notice.confirmation => {
            ctx.rec
                .answers
                .push((format!("Type {} to save", notice.confirmation), typed));
        }
        Some(typed) => {
            ctx.rec
                .answers
                .push((format!("Type {} to save", notice.confirmation), typed));
            return Err(format!(
                "{SKIPPED}The typed confirmation did not match. Nothing was saved."
            ));
        }
        None => return Err(format!("{SKIPPED}You skipped the save. Nothing was saved.")),
    }
    let bytes = packed(&backup);
    let name = program_name(&backup);
    let mut unknown = false;
    let acknowledged = with_variax_progress(ctx, VARIAX_SAVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        let program = write::Program::new(&name, &bytes)?;
        let timeout_notice = format!(
            "No 0x71. Keeping the rig untouched for {} s; no more MIDI is sent. {SPARE_WARNING}",
            SILENT_STORE_WAIT.as_secs()
        );
        progress(Progress::action(format!(
            "Sending {} to the guitar",
            preset_label(preset)
        )));
        let result = shared_variax::store_once_observing(
            session,
            preset,
            &program,
            stopping,
            &timeout_notice,
            |wait| store_progress(progress, wait),
        );
        match result {
            Ok(()) => Ok(true),
            Err(VerifiedStoreError::SilentTimeout) => {
                unknown = true;
                Ok(false)
            }
            Err(VerifiedStoreError::BeforeCommit(error)) => Err(error),
            Err(error) => {
                unknown = true;
                Err(error.to_string())
            }
        }
    });
    ctx.data.store_state_unknown |= unknown;
    let acknowledged = acknowledged?;
    ctx.rec
        .note("The store command (0x61) was sent once. It is never repeated.");
    if !acknowledged {
        ctx.op.instruct(
            ctx.rec.id,
            "Keep the rig powered and untouched. Rig Check waited 2 minutes.",
            "Nothing: no more messages are sent.",
        );
        return Err(format!(
            "Timed out waiting for the Variax store reply. Slot state is unknown. Check whether the guitar responds to a preset change. If it responds, run Rig Check again and compare a new 3.4 backup with this one (evidence folder). If it does not respond, report before powering off. {SPARE_WARNING}"
        ));
    }
    ctx.rec.note("Received the store confirmation (0x71).");
    // Verification is not interrupted by Stop once the store was sent.
    let after = with_variax_progress(ctx, VARIAX_SAVE, |session, _, progress| {
        variax_read_preset(
            session,
            preset,
            &|| false,
            progress,
            "Verifying the saved program",
        )
    })?;
    let after_hash = ctx.rec.checksum(
        "Variax slot after save (read back)",
        &program_bytes(&after.words),
    );
    if !same_program(&after.words, &backup) {
        return Err(format!(
            "The read-back of {} differs from the bytes that were saved. Keep the evidence folder.",
            preset_label(preset)
        ));
    }
    ctx.rec.note(format!(
        "Read-back: all 1,092 words match the saved program and the 3.4 backup (SHA-256 {}). The guitar plays {} again.",
        &after_hash[..16],
        preset_label(preset)
    ));
    ctx.rec.result.push(format!(
        "Saved and verified {}. The read-back matches the backup.",
        preset_label(preset)
    ));
    Ok(())
}

pub fn edit_and_restore(ctx: &mut Ctx<'_>) -> Result<(), String> {
    require_evidence(ctx)?;
    use super::variax_live::{AuditionEdit, edit_images};
    let start = slot(
        ctx.data
            .start_preset
            .ok_or("The starting preset is unknown.")?,
    )?;
    let backup = saved_program(ctx, start)?;
    let (edited, _) = edit_images(ctx, &backup, AuditionEdit::Trim)?;
    preflight(ctx, start, &backup)?;
    confirm_batch(ctx, &[(start, backup)], "EDIT AND RESTORE VARIAX")?;
    let name = program_name(&backup);
    save_image(ctx, start, &name, &edited, true)?;
    ctx.rec.show(format!(
        "Saved and verified the quieter {}.",
        preset_label(start)
    ));
    // Once the temporary store is verified, finish the approved restoration
    // even if Stop was pressed during its acknowledged transaction.
    save_image(ctx, start, &name, &packed(&backup), false)?;
    ctx.rec.show(format!(
        "Back on {} with the original backup verified.",
        preset_label(start)
    ));
    Ok(())
}

pub fn sync_own_bytes(ctx: &mut Ctx<'_>) -> Result<(), String> {
    require_evidence(ctx)?;
    let start = slot(
        ctx.data
            .start_preset
            .ok_or("The starting preset is unknown.")?,
    )?;
    let other = slot(super::variax_reads::other_preset(start))?;
    let original = saved_program(ctx, start)?;
    let targets = [(start, original), (other, saved_program(ctx, other)?)];
    confirm_batch(ctx, &targets, "SYNC VARIAX")?;
    let result = (|| {
        for (index, (preset, words)) in targets.into_iter().enumerate() {
            ctx.op.progress(Progress::counted(
                format!("Checking and saving {}", preset_label(preset)),
                index,
                targets.len(),
                "slots",
            ));
            preflight(ctx, preset, &words)?;
            save_image(ctx, preset, &program_name(&words), &packed(&words), true)?;
            ctx.rec
                .show(format!("Synced and verified {}.", preset_label(preset)));
        }
        Ok::<(), String>(())
    })();
    if ctx.data.store_state_unknown {
        return result;
    }
    if result.is_ok() {
        ctx.op.progress(Progress::counted(
            format!("Returning to {}", preset_label(start)),
            targets.len(),
            targets.len(),
            "slots",
        ));
    }
    let returned = with_variax_progress(ctx, VARIAX_SAVE, |session, _, progress| {
        let mut advance =
            program_progress(progress, "Verifying the return to your starting preset");
        shared_variax::return_to_preset(session, start, &packed(&original), |_| {
            advance();
            Ok(())
        })
    });
    match (result, returned) {
        (Ok(()), Ok(())) => {
            ctx.rec
                .show(format!("Back on {}, verified.", preset_label(start)));
            Ok(())
        }
        (Err(error), Ok(())) => Err(format!("{error} The starting preset was restored.")),
        (Ok(()), Err(error)) => Err(format!("Could not verify the return: {error}")),
        (Err(error), Err(restore)) => Err(format!("{error} Return also failed: {restore}")),
    }
}

fn require_evidence(ctx: &Ctx<'_>) -> Result<(), String> {
    if ctx.mode == super::record::Mode::Real {
        let root = ctx
            .data
            .evidence_dir
            .as_ref()
            .ok_or("Run with --evidence before offering a real store.")?;
        line6_devices::backup::verify_manifest(&root.join(backup_folder(ctx)))?;
    }
    Ok(())
}

fn backup_folder<'ctx>(ctx: &'ctx Ctx<'_>) -> &'ctx str {
    ctx.data
        .variax_backup_folder
        .as_deref()
        .unwrap_or("variax-library")
}

fn saved_program(ctx: &Ctx<'_>, preset: Preset) -> Result<Program, String> {
    let words = ctx
        .data
        .variax_library
        .iter()
        .find(|(p, _)| *p == preset)
        .map(|(_, words)| *words)
        .ok_or("The preset is missing from the 3.4 backup.")?;
    let bytes = packed(&words);
    if shared_variax::expected_stored_image(&bytes) != bytes {
        return Err(format!(
            "The backup for {} would change when stamped by the firmware. Nothing was saved.",
            preset_label(preset)
        ));
    }
    Ok(words)
}

fn preflight(ctx: &mut Ctx<'_>, preset: Preset, expected: &Program) -> Result<(), String> {
    with_variax_progress(ctx, VARIAX_SAVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        let action = format!("Checking {} against its backup", preset_label(preset));
        let mut advance = program_progress(progress, &action);
        let actual = shared_variax::read_unmodified(session, preset, stopping, |_| {
            advance();
            Ok(())
        })?;
        shared_variax::compare_image(
            &actual,
            &packed(expected),
            "Saved position compared with the backup",
        )
    })
}

fn confirm_batch(
    ctx: &mut Ctx<'_>,
    targets: &[(Preset, Program)],
    action: &str,
) -> Result<(), String> {
    let label = targets
        .iter()
        .map(|(preset, _)| preset_label(*preset))
        .collect::<Vec<_>>()
        .join(" and ");
    let backup = targets
        .iter()
        .flat_map(|(_, words)| packed(words))
        .collect::<Vec<_>>();
    let notice = SaveNotice {
        device: "Variax",
        slot: label.clone(),
        backup_step: StepId(3, 4),
        backup_file: format!("{} backup for these slots", backup_folder(ctx)),
        backup_sha256: super::record::sha256_hex(&backup),
        confirmation: format!("{action} {label}"),
    };
    let answer = ctx
        .op
        .confirm_save(&notice)
        .ok_or_else(|| format!("{SKIPPED}You skipped the save."))?;
    ctx.rec.answers.push((
        format!("Type {} to save", notice.confirmation),
        answer.clone(),
    ));
    if answer != notice.confirmation {
        return Err(format!(
            "{SKIPPED}The typed confirmation did not match. Nothing was saved."
        ));
    }
    Ok(())
}

fn save_image(
    ctx: &mut Ctx<'_>,
    target: Preset,
    name: &str,
    bytes: &shared_variax::Image,
    allow_stop: bool,
) -> Result<(), String> {
    let warning = shared_variax::spare_warning(target);
    let program = write::Program::new(name, bytes)?;
    let outcome = with_variax_progress(ctx, VARIAX_SAVE, |session, stopping, progress| {
        shared_variax::identify(session)?;
        progress(Progress::action(format!(
            "Sending {} to the guitar",
            preset_label(target)
        )));
        let action = format!("Verifying saved {}", preset_label(target));
        let mut advance = None;
        Ok(shared_variax::store_and_reload_observing(
            session,
            target,
            &program,
            || allow_stop && stopping(),
            |wait| store_progress(progress, wait),
            |_| {
                advance.get_or_insert_with(|| program_progress(progress, &action))();
                Ok(())
            },
        ))
    })?;
    match outcome {
        Ok(()) => {
            ctx.rec.checksum(
                format!("Stored {}", preset_label(target)),
                &shared_variax::expected_stored_image(bytes),
            );
            Ok(())
        }
        Err(VerifiedStoreError::BeforeCommit(error)) => Err(error),
        Err(error) => {
            ctx.data.store_state_unknown = true;
            Err(format!("{error} {warning}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::practice::{Faults, SimRig};
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::{SaveTyping, Script};

    const STEP: StepId = StepId(5, 1);

    #[test]
    fn the_own_bytes_are_saved_once_and_read_back() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(STEP);
        assert!(step.outcome.passed(), "{step:?}");
        assert!(
            person
                .saves
                .iter()
                .any(|n| n.contains("Spank 1 (flash slot 14)") && n.contains("3.4"))
        );
        let rig = rig.borrow();
        assert_eq!(rig.variax.stores, 5);
        assert_eq!(
            rig.variax.stored,
            SimRig::new(Faults::default()).variax.stored
        );
        let stores = step
            .messages
            .iter()
            .filter(|l| l.contains("OUT f0 00 01 0c 07 00 61"))
            .count();
        assert_eq!(stores, 1);
    }

    #[test]
    fn a_silent_store_is_never_repeated_and_waits_two_minutes() {
        let mut person = Script::run_all();
        let faults = Faults {
            silent_variax_store: true,
            ..Faults::default()
        };
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        let step = run.step(STEP);
        match &step.outcome {
            Outcome::Failed(reason) => assert!(reason.contains("Slot state is unknown")),
            other => panic!("{other:?}"),
        }
        assert!(
            step.messages
                .iter()
                .any(|l| l.contains("Keeping the rig untouched for 120 s"))
        );
        assert_eq!(rig.borrow().variax.stores, 1, "sent once, never repeated");
        assert!(rig.borrow().clock >= SILENT_STORE_WAIT);
    }

    #[test]
    fn a_wrong_or_missing_confirmation_saves_nothing() {
        for typing in [SaveTyping::Wrong, SaveTyping::Skip] {
            let mut person = Script::run_all();
            person.save_typing = typing;
            let (run, _, rig) = run_practice(
                Faults::default(),
                &mut person,
                &mut NoReport,
                RunData::default(),
            );
            assert_eq!(run.step(STEP).outcome, Outcome::SkippedByPerson);
            assert_eq!(rig.borrow().variax.stores, 0);
        }
    }

    #[test]
    fn slots_in_flash_40_to_59_are_refused() {
        let reso = Preset::new(10, 4).unwrap();
        assert_eq!(flash_index(reso), 50);
        assert!(slot(reso).unwrap_err().contains("spare sector 62"));
        assert!(slot(Preset::new(2, 0).unwrap()).is_ok());
    }
}

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
use super::procedures::{program_bytes, same_program, variax_read_preset, with_variax};
use super::runner::{Ctx, SKIPPED};
use line6_devices::family::variax::{Preset, write};
use line6_devices::gate::Operation;
use std::time::Duration;

pub const VARIAX_SAVE: &[Operation] = &[
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
    Operation::VariaxStoreRestore,
];
/// After a silent 15-second wait, keep the rig untouched this long (#177).
pub const AFTER_TIMEOUT: Duration = Duration::from_secs(120);
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

pub fn save_own_bytes(ctx: &mut Ctx<'_>) -> Result<(), String> {
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
    let before = with_variax(ctx, VARIAX_SAVE, |session, stopping| {
        variax_read_preset(session, preset, stopping)
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
            "variax-library/{:02}-{}.bin",
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
    let mut committed_at = None;
    let outcome = with_variax(ctx, VARIAX_SAVE, |session, stopping| {
        let program = write::Program::new(&name, &bytes)?;
        let result = write::store_observing_commit(session, preset, &program, stopping, || {
            committed_at = Some(());
        });
        match result {
            Ok(()) => Ok(Ok(session.link.elapsed())),
            Err(write::StoreError::NoConfirmation) => {
                session.note(format!(
                    "No 0x71. Keeping the rig untouched for {} s; no more MIDI is sent. {SPARE_WARNING}",
                    AFTER_TIMEOUT.as_secs()
                ));
                session.link.pause(AFTER_TIMEOUT);
                Ok(Err(()))
            }
            Err(write::StoreError::Other(error)) => Err(error),
        }
    })?;
    if committed_at.is_some() {
        ctx.rec
            .note("The store command (0x61) was sent once. It is never repeated.");
    }
    if outcome.is_err() {
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
    let after = with_variax(ctx, VARIAX_SAVE, |session, _| {
        variax_read_preset(session, preset, &|| false)
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
        assert_eq!(rig.variax.stores, 1);
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
        assert!(rig.borrow().clock >= AFTER_TIMEOUT);
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

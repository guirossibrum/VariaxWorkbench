//! Step 5.2 (#259): store the starting channel's own bytes back to it.
//!
//! Uses the shared library's PODxt store (#222 stage B): the 2.5 backup on
//! disk is verified (manifest, identity, every entry), the slot is read and
//! must match its backup entry in all 160 tone bytes, the typed confirmation
//! (`STORE PODXT 5B`) is required, then one store frame and one commit are
//! sent, the `03 50` receipt is awaited (7 s) and the slot is read back. A
//! save is never repeated; a missing receipt follows the 2-minute procedure.

use super::catalogue::{SaveNotice, StepId};
use super::procedures::with_podxt;
use super::record::sha256_hex;
use super::runner::{Ctx, SKIPPED};
use line6_devices::family::podxt::{
    self, StoreControl, StoreResult, StoreSource, backup::podxt_entry_name,
};
use line6_devices::gate::Operation;
use std::time::Duration;

pub const PODXT_SAVE: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtLibraryRead,
    Operation::PodxtStoreRestore,
];
pub const AFTER_TIMEOUT: Duration = Duration::from_secs(120);

pub fn save_own_bytes(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let dir = ctx.data.podxt_backup_dir.clone().ok_or(
        "This step needs the 2.5 backup on disk: run Rig Check with the evidence folder (--evidence).",
    )?;
    let slot = ctx
        .data
        .start_channel
        .ok_or("The starting channel is unknown (2.5).")?;
    let prepared = podxt::prepare_tone_store(&dir, slot, StoreSource::OriginalSlot)?;
    let entry = podxt_entry_name(slot);
    let entry_bytes = std::fs::read(dir.join(&entry)).map_err(|e| format!("{entry}: {e}"))?;
    let notice = SaveNotice {
        device: "PODxt Live",
        slot: format!("{} ({:?})", slot.slot_name(), prepared.original_name()),
        backup_step: StepId(2, 5),
        backup_file: format!("podxt-library/{entry}"),
        backup_sha256: sha256_hex(&entry_bytes),
        confirmation: prepared.confirmation(),
    };
    let typed = match ctx.op.confirm_save(&notice) {
        Some(typed) => typed,
        None => return Err(format!("{SKIPPED}You skipped the save. Nothing was saved.")),
    };
    ctx.rec.answers.push((
        format!("Type {} to save", notice.confirmation),
        typed.clone(),
    ));
    let confirmed = match prepared.confirm(&typed) {
        Ok(confirmed) => confirmed,
        Err(_) => {
            return Err(format!(
                "{SKIPPED}The typed confirmation did not match. Nothing was saved."
            ));
        }
    };
    let control = StoreControl::default();
    let result = with_podxt(ctx, PODXT_SAVE, |session| {
        let result = podxt::store_tone(session, confirmed, &control)?;
        if let StoreResult::SlotStateUnknown { reason, .. } = &result
            && reason.contains("No PODxt store reply")
        {
            session.note(format!(
                "No store receipt. Keeping the rig untouched for {} s; no more MIDI is sent.",
                AFTER_TIMEOUT.as_secs()
            ));
            session.link.pause(AFTER_TIMEOUT);
        }
        Ok(result)
    })?;
    if control.started() {
        ctx.rec
            .note("The store was sent once. It is never repeated.");
    }
    match result {
        StoreResult::Verified { slot, tone } => {
            let hash = ctx.rec.checksum(
                format!("PODxt channel {} after save", slot.label()),
                &tone.raw,
            );
            if tone.raw != entry_bytes {
                return Err(format!(
                    "Channel {} read back differently from its backup entry.",
                    slot.label()
                ));
            }
            ctx.rec.note(format!(
                "Receipt 03 50 received. Channel {} reads back byte for byte as its backup (SHA-256 {}).",
                slot.label(),
                &hash[..16]
            ));
            ctx.rec.result.push(format!(
                "Saved and verified channel {}. The read-back matches the backup.",
                slot.label()
            ));
            Ok(())
        }
        StoreResult::SlotStateUnknown { slot, reason } => {
            ctx.op.instruct(
                ctx.rec.id,
                "Keep the rig powered and untouched. Rig Check waited 2 minutes.",
                "Nothing: no more messages are sent.",
            );
            Err(format!(
                "Channel {}: slot state unknown ({reason}). The save was not repeated. Check the channel on the PODxt Live; if it plays, run Rig Check again and compare a new 2.5 backup with this one.",
                slot.label()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::{Faults, SimRig};
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::{SaveTyping, Script};

    const STEP: StepId = StepId(5, 2);

    fn run(
        faults: Faults,
        typing: SaveTyping,
    ) -> (
        crate::rig_check::record::RunRecord,
        std::rc::Rc<std::cell::RefCell<SimRig>>,
        Script,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let data = RunData {
            evidence_dir: Some(dir.path().join("evidence")),
            ..RunData::default()
        };
        let mut person = Script::run_all();
        person.save_typing = typing;
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, data);
        (run, rig, person)
    }

    #[test]
    fn the_channel_is_stored_once_and_read_back() {
        let (run, rig, person) = run(Faults::default(), SaveTyping::Exact);
        assert!(run.step(STEP).outcome.passed(), "{:?}", run.step(STEP));
        assert!(
            person
                .saves
                .iter()
                .any(|n| n.contains("STORE PODXT 5B")
                    && n.contains("podxt-library/tones/018-5B.syx"))
        );
        let rig = rig.borrow();
        assert_eq!(rig.podxt.stores, 1);
        assert_eq!(
            rig.podxt.stored,
            SimRig::new(Faults::default()).podxt.stored
        );
    }

    #[test]
    fn a_missing_receipt_is_never_repeated() {
        let (run, rig, _) = run(
            Faults {
                silent_podxt_store: true,
                ..Faults::default()
            },
            SaveTyping::Exact,
        );
        match &run.step(STEP).outcome {
            Outcome::Failed(reason) => assert!(reason.contains("slot state unknown")),
            other => panic!("{other:?}"),
        }
        assert_eq!(rig.borrow().podxt.stores, 1);
        assert!(rig.borrow().clock >= super::AFTER_TIMEOUT);
    }

    #[test]
    fn no_confirmation_saves_nothing() {
        for typing in [SaveTyping::Wrong, SaveTyping::Skip] {
            let (run, rig, _) = run(Faults::default(), typing);
            assert_eq!(run.step(STEP).outcome, Outcome::SkippedByPerson);
            assert_eq!(rig.borrow().podxt.stores, 0);
        }
        // Practice mode without an evidence folder keeps the backup in a
        // temporary folder, so the save still has its verified backup.
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(run.step(STEP).outcome.passed());
        assert_eq!(rig.borrow().podxt.stores, 1);
    }
}

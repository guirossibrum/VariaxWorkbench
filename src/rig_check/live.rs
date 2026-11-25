//! Step 4.1 (#256): switch the PODxt Live stomp of the playing tone, read
//! back, undo, and check the whole tone byte for byte. Nothing is stored:
//! the step's policy does not include the PODxt store operation.

use super::procedures::with_podxt;
use super::runner::Ctx;
use line6_devices::family::podxt;
use line6_devices::gate::Operation;
use line6_devices::midi::hex;

pub const PODXT_LIVE: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtEditBufferWrite,
];

pub fn podxt_stomp(ctx: &mut Ctx<'_>) -> Result<(), String> {
    // A closure, not the function path: the session's link lifetime is generic.
    #[allow(clippy::redundant_closure)]
    let test = with_podxt(ctx, PODXT_LIVE, |session| podxt::stomp_test(session))?;
    ctx.rec.note(format!("Before: {}", hex(&test.before.raw)));
    ctx.rec
        .checksum("PODxt tone before (168 bytes)", &test.before.raw);
    for (change, applied) in test.applied() {
        ctx.rec.note(format!(
            "Stomp {} -> {}: {}",
            change.original,
            change.target,
            if applied {
                "read back as sent"
            } else {
                "NOT seen in the read-back"
            }
        ));
    }
    if let Ok(after) = &test.after {
        ctx.rec
            .note(format!("After the change: {}", hex(&after.raw)));
    }
    if !test.unexpected().is_empty() {
        ctx.rec.note(format!(
            "Other controllers changed meanwhile (cc, before, after): {:?}",
            test.unexpected()
        ));
    }
    match &test.restored {
        Ok(restored) => {
            ctx.rec
                .note(format!("After the undo: {}", hex(&restored.raw)));
            ctx.rec
                .checksum("PODxt tone after undo (168 bytes)", &restored.raw);
        }
        Err(error) => ctx.rec.note(format!("Read after the undo failed: {error}")),
    }
    if !test.stuck() {
        return Err(
            "The stomp change was not confirmed by the read-back. The undo was still sent.".into(),
        );
    }
    if test.exact_undo() {
        ctx.rec
            .note("The whole tone matches the start byte for byte. Nothing was saved.");
        Ok(())
    } else if test.undone() {
        ctx.rec.note(format!(
            "The stomp is back, but other bytes differ from the start: {:?}",
            test.not_restored()
        ));
        Err("The tone is not byte-for-byte the starting tone after the undo.".into())
    } else {
        Err(
            "The undo did not restore the stomp. Switch the stomp back by hand and check the tone."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    #[test]
    fn the_stomp_is_switched_back_byte_for_byte() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(StepId(4, 1));
        assert!(step.outcome.passed(), "{step:?}");
        assert!(step.evidence.iter().any(|l| l.contains("byte for byte")));
        assert!(
            step.messages.iter().any(|l| l.contains("OUT b0 19 ")),
            "CC 25 was sent"
        );
        let rig = rig.borrow();
        assert_eq!(rig.podxt.stores, 1, "only the save step 5.2 stores");
        assert!(
            !step
                .messages
                .iter()
                .any(|l| l.contains("OUT f0 00 01 0c 03 71")),
            "4.1 sends no store frame"
        );
    }
}

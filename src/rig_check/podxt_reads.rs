//! Step 2.3 (#254): read stored PODxt Live channel 1A and compare it with
//! the library backup from step 2.5.

use super::differences::{self, Severity};
use super::procedures::{PODXT_LIBRARY, podxt_read_stored, with_podxt_progress};
use super::runner::{Ctx, Progress};
use line6_devices::family::podxt::{BankKind, EditBuffer, Location};
use line6_devices::midi::hex;

fn channel() -> Location {
    Location::new(BankKind::Tones, 0).expect("1A tone index")
}

fn record(ctx: &mut Ctx<'_>, location: Location, result: &Result<EditBuffer, String>) {
    match result {
        Ok(tone) => {
            ctx.rec
                .result
                .push(format!("Channel {}: {}", location.label(), tone.name()));
            let hash = ctx.rec.checksum(
                format!("PODxt channel {} (168 bytes)", location.label()),
                &tone.raw,
            );
            ctx.rec.note(format!(
                "Channel {}: {:?}, 168 bytes, expected shape, SHA-256 {}",
                location.label(),
                tone.name(),
                &hash[..16]
            ));
            ctx.rec.note(format!("  {}", hex(&tone.raw)));
        }
        Err(error) => ctx
            .rec
            .note(format!("Channel {}: {error}", location.label())),
    }
}

pub fn one_channel(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let location = channel();
    ctx.data.podxt_channel_read = None;
    let read = with_podxt_progress(ctx, PODXT_LIBRARY, |session, _, progress| {
        progress(Progress::counted("Reading channel 1A", 0, 1, "channels"));
        let read = podxt_read_stored(session, location);
        if read.is_ok() {
            progress(Progress::counted("Channel read complete", 1, 1, "channels"));
        }
        Ok(read)
    })?;
    record(ctx, location, &read);
    if let Ok(tone) = &read {
        ctx.rec.look_at(
            "If the PODxt Live is already on this channel, does its display show the same name?",
            tone.name(),
        );
    }
    let outcome = read.as_ref().map(|_| ()).map_err(Clone::clone);
    ctx.data.podxt_channel_read = Some(read);
    outcome
}

/// Compare the independent 1A read with the backup. Keep missing replies
/// separate from content differences.
pub fn compare_with_backup(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let Some(read) = &ctx.data.podxt_channel_read else {
        return Ok(());
    };
    let backup = ctx
        .data
        .podxt_library
        .iter()
        .find(|(location, _)| *location == channel())
        .map(|(_, tone)| tone);
    match (read, backup) {
        (Err(_), _) => ctx
            .rec
            .note("Channel 1A: the earlier read lost its reply (not a content difference)."),
        (Ok(_), None) => ctx.rec.note("Channel 1A: not in the backup."),
        (Ok(read), Some(backup)) if read.raw == backup.raw => {
            ctx.rec
                .note("Channel 1A: the earlier read matches the backup byte for byte.");
        }
        (Ok(read), Some(backup)) => {
            differences::tone(
                ctx.rec,
                "Channel 1A stored reads",
                &read.raw,
                &backup.raw,
                Severity::Stop,
                false,
            );
            ctx.rec.note(format!(
                "Channel 1A: CONTENT DIFFERS from the backup at controllers {:?} (the channel changed between reads, or a byte was corrupted).",
                read.differences(backup)
            ));
            return Err("The channel 1A read from step 2.3 differs from the backup.".into());
        }
    }
    Ok(())
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
    fn channel_and_backup_comparison_and_later_steps() {
        let mut person = Script::run_all();
        let (run, data, _) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let one = run.step(StepId(2, 3));
        assert!(one.outcome.passed());
        assert!(
            one.evidence[0].starts_with("Channel 1A: \"Practice 1A\", 168 bytes, expected shape")
        );
        let backup = run.step(StepId(2, 5));
        assert_eq!(
            backup
                .evidence
                .iter()
                .filter(|l| l.contains("matches the backup byte for byte"))
                .count(),
            1
        );
        assert!(backup.evidence.iter().any(|l| l == "Starting channel: 5B"));
        assert_eq!(data.podxt_library.len(), 299);
        for id in [
            StepId(2, 5),
            StepId(2, 6),
            StepId(4, 1),
            StepId(4, 6),
            StepId(4, 7),
            StepId(5, 2),
            StepId(6, 2),
        ] {
            assert!(run.step(id).outcome.passed(), "step {id}");
        }
        assert_eq!(
            person
                .cards
                .iter()
                .copied()
                .filter(|id| id.0 == 2)
                .collect::<Vec<_>>(),
            [
                StepId(2, 1),
                StepId(2, 2),
                StepId(2, 3),
                StepId(2, 5),
                StepId(2, 6)
            ]
        );
        assert!(
            person
                .saves
                .iter()
                .any(|notice| notice.contains("backup taken in step 2.5"))
        );
        assert!(
            run.step(StepId(2, 1))
                .evidence
                .iter()
                .any(|l| l == "PODxt Live, firmware 3.01 (verified device)")
        );
    }

    #[test]
    fn a_mismatch_between_1a_and_the_full_backup_blocks_later_edits_and_saves() {
        use crate::rig_check::{port::PracticePort, practice::SimRig, record::Outcome, run_on};

        let rig = SimRig::shared(Faults::default());
        let change = rig.clone();
        let mut person = Script::run_all();
        person.on_card = Some(Box::new(move |step| {
            if step == StepId(2, 5) {
                change.borrow_mut().podxt.stored[0][0] = b'X';
            }
        }));
        let (run, _) = run_on(
            &mut PracticePort::new(rig.clone()),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        assert!(run.step(StepId(2, 3)).outcome.passed());
        assert!(matches!(
            &run.step(StepId(2, 5)).outcome,
            Outcome::Failed(reason) if reason.contains("channel 1A")
        ));
        for id in [StepId(4, 1), StepId(4, 6), StepId(5, 2)] {
            assert_eq!(
                run.step(id).outcome,
                Outcome::SkippedNeeds("read-only from here: step 2.5 failed".into())
            );
        }
        assert!(person.saves.is_empty());
        assert_eq!(rig.borrow().podxt.stores, 0);
        assert_eq!(rig.borrow().variax.stores, 0);
    }
}

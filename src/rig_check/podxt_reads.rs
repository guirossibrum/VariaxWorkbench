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
    fn backup_reports_stop_details_and_benign_differences_without_changing_write_policy() {
        use crate::rig_check::{
            differences::Severity,
            port::PracticePort,
            practice::SimRig,
            record::Outcome,
            redact::Redactor,
            report::{ReportFile, json_block},
            run_on,
        };

        const MID: usize = 32 + 15;
        const PRESENCE: usize = 32 + 21;
        const PEDAL: usize = 32 + 4;
        let backup_step = StepId(2, 5);
        let failure = "The channel 1A read from step 2.3 differs from the backup.";
        let refusal = "read-only from here: step 2.5 failed";
        for mismatch in [true, false] {
            let rig = SimRig::shared(Faults::default());
            let mut person = Script::run_all();
            if mismatch {
                {
                    let mut model = rig.borrow_mut();
                    model.podxt.stored[0][MID] = 0x10;
                    model.podxt.stored[0][PRESENCE] = 0x28;
                }
                let change = rig.clone();
                person.on_card = Some(Box::new(move |step| {
                    if step == backup_step {
                        let mut model = change.borrow_mut();
                        model.podxt.stored[0][MID] = 0x13;
                        model.podxt.stored[0][PRESENCE] = 0x2c;
                    }
                }));
            } else {
                let mut model = rig.borrow_mut();
                let start = usize::from(model.podxt.channel.unwrap());
                model.podxt.stored[start][MID] = 0x10;
                model.podxt.playing[MID] = 0x13;
                model.podxt.stored[start][PEDAL] = 0x15;
                model.podxt.playing[PEDAL] = 0x16;
            }

            let folder = tempfile::tempdir().unwrap();
            let mut report = ReportFile::create(folder.path(), Redactor::default()).unwrap();
            let (run, _) = run_on(
                &mut PracticePort::new(rig.clone()),
                &mut person,
                &mut report,
                RunData::default(),
            );
            assert!(run.finished);
            assert!(run.step(StepId(2, 3)).outcome.passed());
            assert!(run.step(StepId(6, 2)).outcome.passed());

            let plain = std::fs::read_to_string(&report.path).unwrap();
            assert_eq!(plain, report.last);
            let document: serde_json::Value =
                serde_json::from_str(json_block(&plain).unwrap()).unwrap();
            assert_eq!(document["finished"], true);
            let saved_steps = document["steps"].as_array().unwrap();
            let saved = |id: StepId| {
                saved_steps
                    .iter()
                    .find(|step| step["id"] == id.to_string())
                    .unwrap()
            };
            let record = run.step(backup_step);
            let persisted = saved(backup_step);
            let technical = persisted["technical"].as_array().unwrap();
            if mismatch {
                assert_eq!(record.outcome, Outcome::Failed(failure.into()));
                assert_eq!(persisted["outcome"], "failed");
                assert_eq!(persisted["reason"], failure);
                assert_eq!(record.comparisons.len(), 1);
                let comparison = &record.comparisons[0];
                assert_eq!(comparison.severity, Severity::Stop);
                assert!(comparison.summary.contains("2 bytes differ"));
                assert_eq!(
                    comparison.details,
                    [
                        "byte 54 (mid (CC 15)) · expected 0x10 · actual 0x13 · XOR 0x03 · 2 bit(s)",
                        "byte 60 (presence (CC 21)) · expected 0x28 · actual 0x2C · XOR 0x04 · 1 bit(s)",
                    ]
                );
                for line in &comparison.details {
                    assert!(plain.contains(line));
                    assert!(technical.iter().any(|value| value == line));
                }
                assert!(plain.contains("STOP · Channel 1A stored reads"));
                assert!(
                    run.report_differences()
                        .all(|line| !line.contains("1A stored reads"))
                );
                for id in (1..=8)
                    .map(|n| StepId(4, n))
                    .chain((1..=4).map(|n| StepId(5, n)))
                {
                    assert_eq!(
                        run.step(id).outcome,
                        Outcome::SkippedNeeds(refusal.into()),
                        "{id}"
                    );
                    assert!(run.step(id).messages.is_empty(), "{id}");
                    assert!(!person.cards.contains(&id), "{id}");
                    assert_eq!(saved(id)["reason"], refusal, "{id}");
                }
                assert!(person.saves.is_empty());
                let model = rig.borrow();
                assert_eq!(model.podxt.stores, 0);
                assert_eq!(model.variax.stores, 0);
                assert_eq!(model.variax.working_uploads, 0);
            } else {
                assert!(record.outcome.passed());
                assert_eq!(persisted["outcome"], "passed");
                let classifications: Vec<_> = record
                    .comparisons
                    .iter()
                    .map(|entry| entry.severity)
                    .collect();
                assert_eq!(classifications, [Severity::Info, Severity::Report]);
                for comparison in &record.comparisons {
                    let header = format!("{} · {}", comparison.severity.code(), comparison.summary);
                    assert!(plain.contains(&header));
                    assert!(technical.iter().any(|line| line == &header));
                    assert_eq!(comparison.details.len(), 1);
                    assert!(technical.iter().any(|line| line == &comparison.details[0]));
                }
                assert!(record.comparisons[0].details[0].contains("XOR 0x03 · 2 bit(s)"));
                assert!(
                    run.report_differences()
                        .any(|line| line.contains("1 unsaved changes"))
                );
                assert!(
                    run.report_differences()
                        .all(|line| !line.contains("live controller position"))
                );
                for id in [StepId(4, 1), StepId(4, 2), StepId(5, 1), StepId(5, 2)] {
                    assert!(run.step(id).outcome.passed(), "{id}");
                }
                let model = rig.borrow();
                assert!(model.podxt.stores > 0);
                assert!(model.variax.stores > 0);
                assert_eq!(model.podxt.playing[MID], 0x13);
            }
        }
    }
}

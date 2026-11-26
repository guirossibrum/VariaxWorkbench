//! Steps 2.3 and 2.4 (#254): stored PODxt Live channels by memory location,
//! and their comparison with the library backup from step 2.5.

use super::catalogue::StepId;
use super::procedures::{PODXT_LIBRARY, podxt_read_stored, with_podxt_progress};
use super::runner::{Ctx, Progress};
use line6_devices::family::podxt::{BankKind, EditBuffer, Location};
use line6_devices::midi::hex;

/// The channel read by step 2.3 and the bank read by step 2.4.
pub struct ChannelRead {
    step: StepId,
    location: Location,
    result: Result<EditBuffer, String>,
}

pub const ONE_CHANNEL: u16 = 0; // 1A
pub const ONE_BANK: u16 = 0; // 1A-1D

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

fn read_channels(ctx: &mut Ctx<'_>, indices: &[u16]) -> Result<(), String> {
    let step = ctx.step();
    ctx.data.podxt_reads.retain(|read| read.step != step);
    let locations: Vec<Location> = indices
        .iter()
        .map(|&i| Location::new(BankKind::Tones, i).expect("tone index"))
        .collect();
    let results = with_podxt_progress(ctx, PODXT_LIBRARY, |session, _, progress| {
        let total = locations.len();
        let results = locations
            .iter()
            .enumerate()
            .map(|(index, &location)| {
                progress(Progress::counted(
                    format!("Reading channel {}", location.label()),
                    index,
                    total,
                    "channels",
                ));
                (location, podxt_read_stored(session, location))
            })
            .collect::<Vec<_>>();
        if results.iter().all(|(_, read)| read.is_ok()) {
            progress(Progress::counted(
                "Channel reads complete",
                total,
                total,
                "channels",
            ));
        }
        Ok(results)
    })?;
    let mut failures = Vec::new();
    for (location, result) in &results {
        record(ctx, *location, result);
        if let Err(error) = result {
            failures.push(error.clone());
        }
    }
    ctx.data
        .podxt_reads
        .extend(results.into_iter().map(|(location, result)| ChannelRead {
            step,
            location,
            result,
        }));
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join(" "))
    }
}

pub fn one_channel(ctx: &mut Ctx<'_>) -> Result<(), String> {
    read_channels(ctx, &[ONE_CHANNEL])?;
    if let Some(ChannelRead {
        result: Ok(tone), ..
    }) = ctx.data.podxt_reads.last()
    {
        ctx.rec.look_at(
            "If the PODxt Live is already on this channel, does its display show the same name?",
            tone.name(),
        );
    }
    Ok(())
}

pub fn one_bank(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let first = ONE_BANK * 4;
    read_channels(ctx, &[first, first + 1, first + 2, first + 3])
}

/// Compare the earlier reads with the backup: content differences are
/// reported apart from lost replies. Returns the number of content
/// differences.
pub fn compare_with_backup(ctx: &mut Ctx<'_>) -> usize {
    let reads = std::mem::take(&mut ctx.data.podxt_reads);
    let mut content = 0;
    for ChannelRead {
        location,
        result: read,
        ..
    } in &reads
    {
        let backup = ctx
            .data
            .podxt_library
            .iter()
            .find(|(l, _)| l == location)
            .map(|(_, tone)| tone.clone());
        let line = match (read, backup) {
            (Err(_), _) => format!(
                "Channel {}: the earlier read lost its reply (not a content difference).",
                location.label()
            ),
            (Ok(_), None) => format!("Channel {}: not in the backup.", location.label()),
            (Ok(read), Some(backup)) if read.raw == backup.raw => format!(
                "Channel {}: the earlier read matches the backup byte for byte.",
                location.label()
            ),
            (Ok(read), Some(backup)) => {
                content += 1;
                format!(
                    "Channel {}: CONTENT DIFFERS from the backup at controllers {:?} (the channel changed between reads, or a byte was corrupted).",
                    location.label(),
                    read.differences(&backup)
                )
            }
        };
        ctx.rec.note(line);
    }
    ctx.data.podxt_reads = reads;
    content
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
    fn channel_bank_and_backup_comparison() {
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
        assert_eq!(run.step(StepId(2, 4)).checksums.len(), 4);
        let backup = run.step(StepId(2, 5));
        assert_eq!(
            backup
                .evidence
                .iter()
                .filter(|l| l.contains("matches the backup byte for byte"))
                .count(),
            5
        );
        assert!(backup.evidence.iter().any(|l| l == "Starting channel: 5B"));
        assert_eq!(data.podxt_library.len(), 299);
        assert!(
            run.step(StepId(2, 1))
                .evidence
                .iter()
                .any(|l| l == "PODxt Live, firmware 3.01 (verified device)")
        );
    }
}

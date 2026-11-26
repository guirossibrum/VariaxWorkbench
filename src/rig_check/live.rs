//! Step 4.1 (#256): switch the PODxt Live stomp of the playing tone, read
//! back, undo, and check the whole tone byte for byte. Nothing is stored:
//! the step's policy does not include the PODxt store operation.

use super::differences::{self, Severity};
use super::procedures::{with_podxt, with_podxt_stopping};
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
        let status = if applied {
            "read back as sent"
        } else {
            "not seen in the read-back"
        };
        ctx.rec.note(format!(
            "Stomp {} -> {}: {status}",
            change.original, change.target
        ));
        let state = |value: u16| {
            if podxt::Control::StompEnable.is_on(value as u8) {
                "on"
            } else {
                "off"
            }
        };
        ctx.rec.result.push(format!(
            "Stomp {} to {}: {status}.",
            state(change.original),
            state(change.target)
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
    if let Ok(restored) = &test.restored {
        differences::tone(
            ctx.rec,
            "PODxt Stomp undo",
            &test.before.raw,
            &restored.raw,
            Severity::Stop,
            false,
        );
    }
    if !test.stuck() {
        return Err(
            "The stomp change was not confirmed by the read-back. The undo was still sent.".into(),
        );
    }
    if test.exact_undo() {
        podxt_editor_controls(ctx)?;
        let control = podxt::Control::StompEnable;
        let expected = if control.is_on(test.before.cc(control.cc())) {
            "on"
        } else {
            "off"
        };
        ctx.rec.look_at(
            "Check that the stomp LED is back to its starting state.",
            expected,
        );
        ctx.rec
            .show("The whole tone matches the start byte for byte. Nothing was saved.");
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

pub(super) const PODXT_EDITOR: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtLibraryRead,
    Operation::PodxtPackQuery,
    Operation::PodxtGlobalRead,
    Operation::PodxtTunerMode,
    Operation::PodxtEditBufferWrite,
    Operation::PodxtAdditionalEditBufferWrite,
    Operation::PodxtKeepEditBufferChanges,
    Operation::PodxtChannelSwitch,
];

pub fn podxt_editor_get(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let (buffer, globals) = with_podxt_stopping(ctx, PODXT_EDITOR, |session, stopping| {
        let buffer = podxt::get_editor_buffer(session, stopping)?;
        let mut globals = None;
        podxt::read_live(session, podxt::ReadCommand::Globals, stopping, |update| {
            if let podxt::ReadUpdate::Globals(values) = update {
                globals = Some(values);
            }
        })?;
        Ok((buffer, globals.ok_or("No PODxt global report.")?))
    })?;
    ctx.rec.show(format!(
        "Editor Get: {} plays {:?}.",
        buffer.channel.label(),
        buffer.tone.name()
    ));
    let count = globals.iter().filter(|(_, value)| value.is_some()).count();
    ctx.rec.show(format!(
        "{count} of {} global values answered. Installed packs: {:?}.",
        globals.len(),
        buffer.packs
    ));
    for (value, raw) in globals {
        ctx.rec.note(format!("{}: {raw:?}", value.name()));
    }
    if count == 0 {
        return Err("No PODxt global values answered.".into());
    }
    Ok(())
}

pub fn podxt_whole_tone(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let root = ctx
        .data
        .podxt_backup_dir
        .as_ref()
        .ok_or("The 2.5 backup was not saved.")?
        .clone();
    let start = ctx
        .data
        .start_tone
        .clone()
        .ok_or("No starting PODxt tone.")?;
    let channel = ctx.data.start_channel.ok_or("No starting PODxt channel.")?;
    let verified = podxt::verify_tone_backup(&root, channel)?;
    let mut before_raw = None;
    let sent = with_podxt_stopping(ctx, PODXT_EDITOR, |session, stopping| {
        let before = podxt::get_editor_buffer(session, stopping)?;
        before_raw = Some(before.tone.raw.clone());
        if before.channel != channel || !differences::same_podxt_tone(&start.raw, &before.tone.raw)
        {
            return Err("The playing channel changed since the backup. Nothing was sent.".into());
        }
        podxt::send_whole_tone(
            session,
            &before,
            podxt::WholeToneSource::Current,
            &verified,
            stopping,
        )
    });
    if let Err(reason) = sent {
        if let Some(before) = before_raw {
            if reason.contains("Whole-tone readback differs") {
                if let Some(actual) = differences::last_podxt_dump(&ctx.rec.messages) {
                    differences::tone(
                        ctx.rec,
                        "PODxt whole-tone send",
                        &before,
                        &actual,
                        Severity::Stop,
                        false,
                    );
                }
            } else if reason.contains("playing channel changed") {
                differences::tone(
                    ctx.rec,
                    "PODxt whole-tone preflight",
                    &start.raw,
                    &before,
                    Severity::Stop,
                    true,
                );
            }
        }
        return Err(reason);
    }
    ctx.rec
        .show("The whole tone read back as sent on your starting channel.");
    Ok(())
}

pub(super) fn restore_podxt_playing(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_tone
        .clone()
        .ok_or("No PODxt starting tone.")?;
    let channel = ctx.data.start_channel.ok_or("No PODxt starting channel.")?;
    let root = ctx
        .data
        .podxt_backup_dir
        .clone()
        .ok_or("No verified PODxt backup for putting back unsaved edits.")?;
    let backup = podxt::verify_tone_backup(&root, channel)?;
    let payload = podxt::TonePayload::from_dump(&start)?;
    let actual = with_podxt_stopping(ctx, PODXT_EDITOR, |session, _| {
        let mut current = podxt::get_editor_buffer(session, || false)?;
        if current.channel != channel {
            podxt::restore_channel(session, channel)?;
            current = podxt::get_editor_buffer(session, || false)?;
        }
        if current.channel != channel {
            return Err(
                "The starting channel was not selected before restoring the playing tone.".into(),
            );
        }
        podxt::send_whole_tone(
            session,
            &current,
            podxt::WholeToneSource::Payload(&payload),
            &backup,
            || false,
        )
    })?;
    let same = super::differences::tone(
        ctx.rec,
        "Restored PODxt edit buffer",
        &start.raw,
        &actual.tone.raw,
        super::differences::Severity::Stop,
        true,
    ) == 0;
    if same {
        ctx.rec.show(format!(
            "Rig: the PODxt Live is back on {} with your unsaved edits restored.",
            channel.label()
        ));
        Ok(())
    } else {
        Err("The restored PODxt tone differs from the starting edit buffer.".into())
    }
}

pub fn podxt_recall(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx.data.start_channel.ok_or("No starting PODxt channel.")?;
    let tone = ctx
        .data
        .start_tone
        .clone()
        .ok_or("No starting PODxt tone.")?;
    let target = podxt::Location::new(podxt::BankKind::Tones, (start.index() + 1) % 128)
        .expect("bounded next channel");
    let target_tone = ctx
        .data
        .podxt_library
        .iter()
        .find(|(loc, _)| *loc == target)
        .ok_or("The target channel is missing from the backup.")?
        .1
        .clone();
    let saved_start = ctx
        .data
        .podxt_library
        .iter()
        .find(|(location, _)| *location == start)
        .ok_or("The starting channel is missing from the backup.")?
        .1
        .clone();
    let unsaved = ctx.data.podxt_unsaved;
    let result = with_podxt_stopping(ctx, PODXT_EDITOR, |session, stopping| {
        let current = podxt::get_editor_buffer(session, stopping)?;
        if current.channel != start || !differences::same_podxt_tone(&tone.raw, &current.tone.raw) {
            return Err(
                "The starting PODxt channel or its edit buffer changed. Nothing was switched."
                    .into(),
            );
        }
        if unsaved {
            let loaded = podxt::restore_channel(session, start)?;
            if !differences::same_podxt_tone(&saved_start.raw, &loaded.tone.raw) {
                return Err("The saved starting channel differs from its backup.".into());
            }
        }
        let selected = podxt::recall_editor_channel(session, target, stopping);
        let returned = podxt::recall_editor_channel(session, start, || false)?;
        Ok((selected?.tone, returned.tone))
    });
    if unsaved {
        restore_podxt_playing(ctx)?;
    }
    let (selected, returned) = result?;
    let returned_ok = super::differences::tone(
        ctx.rec,
        "Starting stored channel after recall",
        &saved_start.raw,
        &returned.raw,
        super::differences::Severity::Stop,
        true,
    ) == 0;
    let selected_ok = super::differences::tone(
        ctx.rec,
        "Recalled PODxt tone",
        &target_tone.raw,
        &selected.raw,
        super::differences::Severity::Stop,
        true,
    ) == 0;
    if !returned_ok || !selected_ok {
        return Err("The recalled or returned tone differs from the backup.".into());
    }
    let result = selected.name();
    ctx.rec.show(format!(
        "Recalled {} ({result}), then verified the return to {}.",
        target.label(),
        start.label()
    ));
    Ok(())
}

pub fn podxt_tune(ctx: &mut Ctx<'_>) -> Result<(), String> {
    loop {
        let action = super::guided::begin_action(
            ctx,
            "Pluck one open string on the guitar.",
            "The PODxt tuner may mute the sound briefly.",
            "a note and its tuning deviation",
            &[],
        )?;
        super::guided::wait_until_ready(ctx, &action)?;
        let found = std::cell::Cell::new(false);
        let result = with_podxt_stopping(ctx, PODXT_EDITOR, |session, stopping| {
            podxt::tuner_window(
                session,
                super::guided::ACTION_WINDOW,
                || stopping() || found.get() || action.done(),
                |update| {
                    if let podxt::ReadUpdate::Tuner(reading) = update
                        && let (Some(note), Some(deviation)) = (reading.note, reading.deviation)
                    {
                        action
                            .clock
                            .observe(format!("Tuner note {note}, deviation {deviation}"));
                        found.set(true);
                    }
                },
            )
        });
        match result {
            Err(error) if error != "PODxt read cancelled." || ctx.stopping() => return Err(error),
            _ => {}
        }
        if found.get() {
            ctx.rec.show("Received a tuner reading. The tuner is off.");
            return Ok(());
        }
        super::guided::missed(
            ctx,
            "a tuner note and deviation",
            "no complete tuner reading",
        )?;
    }
}

fn podxt_editor_controls(ctx: &mut Ctx<'_>) -> Result<(), String> {
    use podxt::{Control, EditorEdit, LiveControl, LiveTarget, Setting, Target};
    if ctx.stopping() {
        return Err("Stopped before editor control edits.".into());
    }
    let mut before_raw = None;
    let mut tempo_available = false;
    let result = with_podxt_stopping(ctx, PODXT_EDITOR, |session, stopping| {
        let before = podxt::get_editor_buffer(session, stopping)?;
        before_raw = Some(before.tone.raw.clone());
        let tempo = LiveControl::Additional(Setting::Tempo).read(&before.tone);
        // The compound tempo path emits two CC messages. Use a nearby value
        // within the supported range and keep the original available for Undo.
        let drive = before.tone.cc(Control::Drive.cc());
        let mut targets = vec![LiveTarget::Basic(
            Control::Drive,
            Target::Offset(if drive == 127 { -1 } else { 1 }),
        )];
        tempo_available = (300..=2400).contains(&tempo);
        if tempo_available {
            targets.push(LiveTarget::Additional(
                Setting::Tempo,
                if tempo == 2400 { tempo - 1 } else { tempo + 1 },
            ));
        }
        let edited =
            podxt::edit_editor_buffer(session, &before, EditorEdit::Controls(&targets), stopping);
        let current = match edited.as_ref() {
            Ok(current) => current.clone(),
            Err(_) => podxt::get_editor_buffer(session, || false)?,
        };
        let back =
            podxt::edit_editor_buffer(session, &current, EditorEdit::Undo(&before), || false)?;
        if back.tone.raw != before.tone.raw {
            return Err("Editor Undo did not restore the complete PODxt tone.".into());
        }
        edited.map(|_| ())
    });
    if let Err(reason) = result {
        if reason.contains("Undo")
            && let Some(expected) = before_raw
            && let Some(actual) = differences::last_podxt_dump(&ctx.rec.messages)
        {
            differences::tone(
                ctx.rec,
                "PODxt editor Undo",
                &expected,
                &actual,
                Severity::Stop,
                false,
            );
        }
        return Err(reason);
    }
    if tempo_available {
        ctx.rec.note("Editor Drive and compound tempo edits read back. Editor Undo restored the complete tone.");
    } else {
        ctx.rec.show("Drive read back and restored. Tempo was outside the editable range, so its check was not applicable.");
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

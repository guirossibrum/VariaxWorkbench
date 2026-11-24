//! Bounded PODxt Live 3.01 edit-buffer transactions for the editor.
//! The app owns the attachment and holds the rig lock until the link closes.

use super::*;
use crate::midi::Event;
use crate::session::POLL;

#[derive(Clone, Debug)]
pub struct EditorBuffer {
    pub channel: Location,
    pub tone: EditBuffer,
    pub packs: Packs,
}

#[derive(Clone, Copy, Debug)]
pub enum EditorEdit<'a> {
    /// Apply verified encoder messages to the currently playing tone.
    Controls(&'a [LiveTarget]),
    /// Send the current dump with only these verified controls replaced.
    Send(&'a [LiveTarget]),
    /// Restore verified settings to the initial snapshot using controller messages.
    Undo(&'a EditorBuffer),
}

/// EncodePatch's edit destination prefixes the 160-byte payload with 74 0A.
/// The reverse direction has the same frame layout. No slot address is present.
#[derive(Clone, Copy, Debug)]
struct SendEditorBuffer<'a>(&'a EditBuffer);

impl Request for SendEditorBuffer<'_> {
    fn bytes(self) -> Vec<u8> {
        self.0.raw.clone()
    }

    fn path(self) -> Path {
        Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        Operation::PodxtEditBufferWrite
    }
}

pub fn get_editor_buffer<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
) -> Result<EditorBuffer, String> {
    open_editor(session, &cancelled)?;
    let channel = playing_channel(session)?;
    let tone = editor_tone(session)?;
    let packs = query_packs(session)?;
    check_channel(session, channel)?;
    check_stop(&cancelled)?;
    Ok(EditorBuffer {
        channel,
        tone,
        packs,
    })
}

pub fn edit_editor_buffer<L: Link>(
    session: &mut Session<L>,
    expected: &EditorBuffer,
    edit: EditorEdit<'_>,
    cancelled: impl Fn() -> bool,
) -> Result<EditorBuffer, String> {
    for operation in [
        Operation::PodxtEditBufferWrite,
        Operation::PodxtAdditionalEditBufferWrite,
        Operation::PodxtKeepEditBufferChanges,
    ] {
        if !session.allows(operation) {
            return Err("PODxt Live editor writes are closed. No edit was sent.".into());
        }
    }
    open_editor(session, &cancelled)?;
    check_channel(session, expected.channel)?;
    let before = editor_tone(session)?;
    if !still_current(&expected.tone, &before) {
        return Err(
            "The current tone changed outside the editor. Get edit buffer again before editing."
                .into(),
        );
    }

    let (tone, packs) = match edit {
        EditorEdit::Undo(snapshot) => {
            if snapshot.channel != expected.channel {
                return Err("The edit snapshot belongs to a different playing channel.".into());
            }
            let controls = changed_controls(&before, &snapshot.tone);
            let packs = checked_models(
                session,
                expected.packs,
                &before,
                controls
                    .iter()
                    .map(|&control| (control, control.read(&snapshot.tone))),
                false,
            )?;
            check_channel(session, expected.channel)?;
            check_stop(&cancelled)?;
            (
                restore_snapshot(session, expected.channel, &before, &snapshot.tone, controls)?,
                packs,
            )
        }
        EditorEdit::Controls(targets) | EditorEdit::Send(targets) => {
            let mut changes = editor_changes(&before, targets)?;
            let full = matches!(edit, EditorEdit::Send(_));
            let model_values = Setting::ALL
                .into_iter()
                .filter(|setting| setting.model_list().is_some())
                .map(LiveControl::Additional)
                .filter_map(|control| {
                    changes
                        .iter()
                        .find(|&&(entry, _)| entry == control)
                        .copied()
                        .or_else(|| full.then(|| (control, control.read(&before))))
                });
            let packs = checked_models(session, expected.packs, &before, model_values, full)?;
            check_channel(session, expected.channel)?;
            check_stop(&cancelled)?;
            let result = match edit {
                EditorEdit::Send(_) => send_dump(session, &before, &changes, &cancelled),
                _ => {
                    changes
                        .retain(|(control, value)| !control.matches(control.read(&before), *value));
                    send_controls(session, &changes, &cancelled)
                }
            };
            if let Err(error) = result {
                return Err(format!(
                    "{error} The edit could be partial. Get edit buffer before undoing."
                ));
            }
            let tone = read_after_write(session, expected.channel)?;
            if let Some(&(control, value)) = changes
                .iter()
                .find(|(control, value)| !control.matches(control.read(&tone), *value))
            {
                return Err(format!(
                    "{} read back as {} after sending {}. Get edit buffer before undoing.",
                    control.name(),
                    control.describe(control.read(&tone)),
                    control.describe(value)
                ));
            }
            if matches!(edit, EditorEdit::Send(_))
                && !still_current(&project(&before, &changes), &tone)
            {
                return Err("The edit-buffer transfer did not match the sent tone. Get edit buffer before undoing.".into());
            }
            (tone, packs)
        }
    };
    Ok(EditorBuffer {
        channel: expected.channel,
        tone,
        packs,
    })
}

fn check_stop(cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("PODxt Live operation cancelled.".into())
    } else {
        Ok(())
    }
}

fn open_editor<L: Link>(
    session: &mut Session<L>,
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    check_stop(cancelled)?;
    session.require_expected_sysex();
    let (_, identity) = identify_podxt(session)?;
    if identity.member != PODXT_LIVE_MEMBER || identity.version != [0, 3, 0, 1] {
        return Err("This port did not identify as PODxt Live 3.01.".into());
    }
    check_stop(cancelled)
}

fn playing_channel<L: Link>(session: &mut Session<L>) -> Result<Location, String> {
    read_global(session, Global::CurrentChannel)?
        .and_then(|index| Location::new(BankKind::Tones, index))
        .ok_or_else(|| "No valid playing-channel reply from the PODxt Live.".into())
}

fn check_channel<L: Link>(session: &mut Session<L>, expected: Location) -> Result<(), String> {
    if playing_channel(session)? != expected {
        return Err("The playing channel changed. Get edit buffer for the current channel.".into());
    }
    Ok(())
}

fn editor_tone<L: Link>(session: &mut Session<L>) -> Result<EditBuffer, String> {
    let tone = read_edit_buffer(session)?;
    if tone.raw[6] != PODXT_LIVE_MEMBER as u8 || tone.raw[7..167].iter().any(|byte| *byte > 127) {
        return Err("The reply is not a seven-bit PODxt Live edit buffer.".into());
    }
    Ok(tone)
}

fn still_current(expected: &EditBuffer, observed: &EditBuffer) -> bool {
    expected
        .raw
        .iter()
        .zip(&observed.raw)
        .enumerate()
        .all(|(index, (a, b))| {
            a == b
                || (39..167).contains(&index)
                    && (LIVE_STATE_CCS.contains(&((index - 39) as u8)) || index == 39 + 13)
        })
}

fn checked_models<L: Link>(
    session: &mut Session<L>,
    cached: Packs,
    before: &EditBuffer,
    changes: impl Iterator<Item = (LiveControl, u16)>,
    full: bool,
) -> Result<Packs, String> {
    let mut models = changes
        .filter_map(|(control, value)| match control {
            LiveControl::Additional(setting) if setting.model_list().is_some() => (full
                || control.read(before) != value)
                .then_some((setting, control.read(before), value)),
            _ => None,
        })
        .peekable();
    if models.peek().is_none() {
        return Ok(cached);
    }
    let packs = query_packs(session)?;
    for (setting, original, value) in models {
        setting.check_model(original, packs, "starting")?;
        setting.check_model(value, packs, "requested")?;
    }
    Ok(packs)
}

fn editor_changes(
    before: &EditBuffer,
    targets: &[LiveTarget],
) -> Result<Vec<(LiveControl, u16)>, String> {
    let mut changes = Vec::with_capacity(targets.len());
    for &target in targets {
        target.validate()?;
        let control = target.control();
        if changes.iter().any(|&(seen, _)| seen == control) {
            return Err(format!("{} is listed twice.", control.name()));
        }
        let original = control.read(before);
        if let LiveControl::Additional(setting) = control
            && !setting.valid_target(original)
            && setting != Setting::TweakParam
        {
            return Err(format!(
                "{} has an unrecognized starting value for undo.",
                control.name()
            ));
        }
        changes.push((control, target.resolve(original)));
    }
    for (manual, note) in [
        (Setting::ModSpeed, Setting::ModNote),
        (Setting::DelayTime, Setting::DelayNote),
    ] {
        let value = |setting| {
            let control = LiveControl::Additional(setting);
            changes
                .iter()
                .find(|&&(entry, _)| entry == control)
                .map_or_else(|| control.read(before), |&(_, value)| value)
        };
        if changes
            .iter()
            .any(|&(control, _)| control == LiveControl::Additional(manual))
            && value(note) != 0
        {
            return Err(format!(
                "Set {} to Off before editing {}.",
                note.name(),
                manual.name()
            ));
        }
    }
    changes.sort_by_key(|&(control, value)| control.order(value));
    Ok(changes)
}

fn send_controls<L: Link>(
    session: &mut Session<L>,
    changes: &[(LiveControl, u16)],
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    for &(control, value) in changes {
        check_stop(cancelled)?;
        // Do not cancel in between the two halves of a logical value.
        for (_, _, request) in control.messages(value) {
            session.send(request)?;
            session.link.pause(CC_GAP);
        }
    }
    Ok(())
}

fn project(before: &EditBuffer, changes: &[(LiveControl, u16)]) -> EditBuffer {
    let mut draft = before.clone();
    for &(control, value) in changes {
        for (cc, byte, _) in control.messages(value) {
            draft.raw[39 + usize::from(cc)] = byte;
        }
    }
    draft
}

fn send_dump<L: Link>(
    session: &mut Session<L>,
    before: &EditBuffer,
    changes: &[(LiveControl, u16)],
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    let mut draft = project(before, changes);
    // 03 74 already carries both timing bytes and NOTE. Only allow manual
    // rate changes with sync off. The remaining bytes come from the POD.
    draft.raw[6] = PODXT_LIVE_MEMBER as u8;
    session.send_chunked_sysex(SendEditorBuffer(&draft), 32, Some(7), || {
        if cancelled() {
            Err("PODxt edit-buffer transfer cancelled.")
        } else {
            Ok(())
        }
    })
}

fn read_after_write<L: Link>(
    session: &mut Session<L>,
    channel: Location,
) -> Result<EditBuffer, String> {
    // Drain command echoes before asking for a new 03 75 reply. In
    // particular, 03 74 can look exactly like an edit-buffer readback.
    let deadline = session.link.elapsed() + APPLY;
    while let Some(remaining) = deadline
        .checked_sub(session.link.elapsed())
        .filter(|left| !left.is_zero())
    {
        if session
            .poll(remaining.min(POLL))?
            .into_iter()
            .flatten()
            .any(|event| matches!(event, Event::Short(message) if message[0] & 0xf0 == 0xc0))
        {
            return Err(
                "The playing channel changed during the edit. Get edit buffer again.".into(),
            );
        }
    }
    let tone = editor_tone(session)?;
    check_channel(session, channel)?;
    Ok(tone)
}

fn changed_controls(before: &EditBuffer, snapshot: &EditBuffer) -> Vec<LiveControl> {
    Control::ALL
        .into_iter()
        .map(LiveControl::Basic)
        .chain(Setting::ALL.into_iter().map(LiveControl::Additional))
        .filter(|control| control.read(before) != control.read(snapshot))
        .collect()
}

fn restore_snapshot<L: Link>(
    session: &mut Session<L>,
    channel: Location,
    before: &EditBuffer,
    snapshot: &EditBuffer,
    mut controls: Vec<LiveControl>,
) -> Result<EditBuffer, String> {
    let mut observed = before.clone();
    // Model changes can reset companion controls. A second pass repairs the
    // read-back differences using the same encoder, including nonzero wah.
    for _ in 0..2 {
        if controls.is_empty() {
            break;
        }
        for (timing, note) in [
            (Setting::ModSpeed, Setting::ModNote),
            (Setting::DelayTime, Setting::DelayNote),
        ] {
            let rate = LiveControl::Additional(timing);
            let note = LiveControl::Additional(note);
            if !controls.contains(&rate) {
                continue;
            }
            if note.read(&snapshot) != 0 && !controls.contains(&note) {
                controls.push(note);
            }
            if note.read(&observed) != 0 {
                let (_, _, request) = note.messages(0).next().expect("one NOTE message");
                session.send(request)?;
                session.link.pause(CC_GAP);
            }
        }
        controls.sort_by_key(|control| control.order(control.read(snapshot)));
        let (status, _) = undo_settings(session, snapshot, &controls);
        status?;
        observed = read_after_write(session, channel)?;
        // A model that failed to restore must not be sent indefinitely.
        controls = changed_controls(&observed, snapshot);
        controls.retain(|control| {
            !matches!(control, LiveControl::Additional(setting)
            if setting.model_list().is_some())
        });
    }
    if observed.raw != snapshot.raw {
        let difference = observed.differences(snapshot).len();
        return Err(format!(
            "Undo did not restore the full snapshot ({difference} controller differences). Get edit buffer to inspect the current tone."
        ));
    }
    Ok(observed)
}

#[cfg(test)]
mod tests;

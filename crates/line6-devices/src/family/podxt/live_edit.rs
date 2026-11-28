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

impl EditorBuffer {
    /// Whether Undo can restore a supported control in this channel.
    pub fn has_restorable_changes(&self, snapshot: &Self) -> bool {
        self.channel == snapshot.channel
            && differing_controls(&self.tone, &snapshot.tone)
                .next()
                .is_some()
    }
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

pub fn get_editor_buffer<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
) -> Result<EditorBuffer, String> {
    get_buffer(session, cancelled, |session, _| editor_tone(session))
}

/// Workbench's standalone Get, on a fresh locked link. Revalidate the
/// playing channel after the dump and never let an unresolved reply reach
/// the following global or pack query.
pub fn get_editor_buffer_tracking_reply<L: Link>(
    session: &mut Session<L>,
    cancelled: impl Fn() -> bool,
    outstanding: &mut bool,
) -> Result<EditorBuffer, String> {
    if *outstanding {
        return Err(
            "An earlier PODxt reply may still be pending. Reconnect the PODxt Live.".into(),
        );
    }
    get_buffer(session, cancelled, |session, cancelled| {
        let tone = read_verified_current_tone(
            session,
            || check_stop(cancelled),
            None,
            outstanding,
            |event, _, _| match super::live_read::tone_event(event) {
                ToneReadEvent::StateChanged => {
                    Err("The PODxt tone changed while refreshing.".into())
                }
                disposition => Ok(disposition),
            },
        )?;
        validate_editor_tone(&tone)?;
        Ok(tone)
    })
}

fn get_buffer<L: Link, C: Fn() -> bool>(
    session: &mut Session<L>,
    cancelled: C,
    read_tone: impl FnOnce(&mut Session<L>, &C) -> Result<EditBuffer, String>,
) -> Result<EditorBuffer, String> {
    open_editor(session, &cancelled)?;
    let channel = playing_channel(session)?;
    let tone = read_tone(session, &cancelled)?;
    check_stop(&cancelled)?;
    let packs = query_packs(session)?;
    check_stop(&cancelled)?;
    check_channel(session, channel)?;
    check_stop(&cancelled)?;
    Ok(EditorBuffer {
        channel,
        tone,
        packs,
    })
}

/// Recall a stored channel after confirming the currently playing tone is
/// unedited. This transaction writes a program change only.
pub fn recall_editor_channel<L: Link>(
    session: &mut Session<L>,
    target: Location,
    cancelled: impl Fn() -> bool,
) -> Result<EditorBuffer, String> {
    if target.kind() != BankKind::Tones || !session.allows(Operation::PodxtChannelSwitch) {
        return Err("PODxt tone-channel recall is unavailable.".into());
    }
    open_editor(session, &cancelled)?;
    let channel = playing_channel(session)?;
    let before = editor_tone(session)?;
    if channel == target {
        return Ok(EditorBuffer {
            channel,
            tone: before,
            packs: query_packs(session)?,
        });
    }
    if !read_stored(session, channel)?.same_tone(&before) {
        return Err(format!(
            "The playing tone differs from saved channel {}. Nothing was switched.",
            channel.label()
        ));
    }
    let target_stored = read_stored(session, target)?;
    let packs = query_packs(session)?;
    check_channel(session, channel)?;
    check_stop(&cancelled)?;
    select_editor_channel(session, target, &target_stored, packs)
}

/// Byte differences between a playing dump and a saved slot. Callers supply
/// the provenance of the saved slot (cached or addressed on the current link).
#[derive(Clone, Debug)]
pub struct RecallComparison {
    pub name_changed: bool,
    pub controls: Vec<RecallControlDifference>,
    pub opaque_offsets: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct RecallControlDifference {
    pub cc: u8,
    pub playing: u8,
    pub saved: u8,
}

impl RecallControlDifference {
    pub fn volatile(self) -> bool {
        LIVE_STATE_CCS.contains(&self.cc)
    }

    pub fn name(self) -> Option<&'static str> {
        Control::ALL
            .into_iter()
            .map(LiveControl::Basic)
            .chain(Setting::ALL.into_iter().map(LiveControl::Additional))
            .find(|control| control.cc() == self.cc || control.low_cc() == Some(self.cc))
            .map(LiveControl::name)
            .or(match self.cc {
                4 => Some("Wah position"),
                7 => Some("Volume pedal"),
                105 => Some("Bypass volume"),
                _ => None,
            })
    }
}

impl RecallComparison {
    pub fn between(playing: &EditBuffer, saved: &EditBuffer) -> Self {
        Self {
            name_changed: playing.name() != saved.name(),
            controls: playing
                .differences(saved)
                .into_iter()
                .map(|(cc, playing, saved)| RecallControlDifference { cc, playing, saved })
                .collect(),
            opaque_offsets: (23..39)
                .filter(|&offset| playing.raw[offset] != saved.raw[offset])
                .collect(),
        }
    }

    pub fn has_name_or_control_difference(&self) -> bool {
        self.name_changed
            || self
                .controls
                .iter()
                .any(|difference| !difference.volatile())
    }

    /// A supported comparison with a saved tone that allows retaining the
    /// checked playing payload without guessing what unknown bytes mean.
    pub fn can_retain_as_draft(&self) -> bool {
        self.opaque_offsets.is_empty()
            && self.has_name_or_control_difference()
            && self
                .controls
                .iter()
                .all(|difference| difference.volatile() || difference.name().is_some())
    }
}

fn same_recall_source(left: &EditBuffer, right: &EditBuffer) -> bool {
    // Match the clean-recall rules, including the opaque bytes. Changes in
    // proven live state or name padding alone cannot expire a decision.
    left.same_tone(right) && left.raw[23..39] == right.raw[23..39]
}

/// The playing tone the user was asked to leave and its intended destination.
#[derive(Clone, Debug)]
pub struct RecallConfirmation {
    target: Location,
    playing: EditorBuffer,
    comparison: RecallComparison,
}

impl RecallConfirmation {
    pub fn target(&self) -> Location {
        self.target
    }

    pub fn playing(&self) -> &EditorBuffer {
        &self.playing
    }

    pub fn comparison(&self) -> &RecallComparison {
        &self.comparison
    }

    pub fn matches_playing(&self, buffer: &EditorBuffer) -> bool {
        self.playing.channel == buffer.channel
            && self.playing.packs == buffer.packs
            && same_recall_source(&self.playing.tone, &buffer.tone)
    }
}

#[derive(Debug)]
pub enum EditorRecall {
    Playing(EditorBuffer),
    ConfirmationRequired(RecallConfirmation),
    ConfirmationExpired(EditorBuffer),
}

/// Workbench Live recall. Rig Check uses the separate strict entry point.
pub fn recall_live_channel<L: Link>(
    session: &mut Session<L>,
    target: Location,
    confirmation: Option<&RecallConfirmation>,
    cancelled: impl Fn() -> bool,
) -> Result<EditorRecall, String> {
    recall_live_channel_with_draft(session, target, confirmation, None, cancelled)
}

/// A verified session draft that the caller owns for this attachment.
pub enum OutgoingDraft<'a> {
    Available(Location, &'a TonePayload),
    Required(Location, &'a TonePayload),
}

/// A caller may supply a checked session draft from this attachment. It
/// protects the outgoing tone only if both fresh reads match it byte for
/// byte. Recall selects the saved destination and never sends the draft.
pub fn recall_live_channel_with_draft<L: Link>(
    session: &mut Session<L>,
    target: Location,
    confirmation: Option<&RecallConfirmation>,
    retained: Option<OutgoingDraft<'_>>,
    cancelled: impl Fn() -> bool,
) -> Result<EditorRecall, String> {
    if target.kind() != BankKind::Tones || !session.allows(Operation::PodxtChannelSwitch) {
        return Err("PODxt tone-channel recall is unavailable.".into());
    }
    open_editor(session, &cancelled)?;
    let channel = playing_channel(session)?;
    check_stop(&cancelled)?;
    let tone = editor_tone(session)?;
    check_stop(&cancelled)?;
    let retains = |tone: &EditBuffer| {
        let Some(OutgoingDraft::Available(slot, payload) | OutgoingDraft::Required(slot, payload)) =
            retained
        else {
            return false;
        };
        slot == channel && TonePayload::from_dump(tone).as_ref() == Ok(payload)
    };
    let retention_required = matches!(retained, Some(OutgoingDraft::Required(..)));
    let retained_before = retains(&tone);
    let expired = (retention_required && !retained_before)
        || confirmation.is_some_and(|consent| {
            consent.target != target
                || consent.playing.channel != channel
                || !same_recall_source(&consent.playing.tone, &tone)
        });
    let saved_playing = if channel != target && confirmation.is_none() && !expired {
        let stored = read_stored(session, channel)?;
        validate_editor_tone(&stored)?;
        Some(stored)
    } else {
        None
    };
    let needs_confirmation = !retained_before
        && saved_playing
            .as_ref()
            .is_some_and(|saved| !same_recall_source(&tone, saved));
    check_stop(&cancelled)?;
    let target_stored = if channel != target && !expired && !needs_confirmation {
        let stored = read_stored(session, target)?;
        validate_editor_tone(&stored)?;
        check_stop(&cancelled)?;
        Some(stored)
    } else {
        None
    };
    let packs = query_packs(session)?;
    check_stop(&cancelled)?;
    // Recheck after the addressed target read, immediately before the final
    // channel check. Any intervening edit requires a new decision.
    let playing = EditorBuffer {
        channel,
        tone: if target_stored.is_some() {
            editor_tone(session)?
        } else {
            tone
        },
        packs,
    };
    check_channel(session, channel)?;
    check_stop(&cancelled)?;

    if expired
        || retention_required && !retains(&playing.tone)
        || confirmation.is_some_and(|consent| !consent.matches_playing(&playing))
    {
        return Ok(EditorRecall::ConfirmationExpired(playing));
    }
    if let Some(saved) = saved_playing
        && !(retained_before && retains(&playing.tone))
        && !same_recall_source(&playing.tone, &saved)
    {
        let comparison = RecallComparison::between(&playing.tone, &saved);
        return Ok(EditorRecall::ConfirmationRequired(RecallConfirmation {
            target,
            playing,
            comparison,
        }));
    }
    let Some(target_stored) = target_stored else {
        return Ok(EditorRecall::Playing(playing));
    };
    select_editor_channel(session, target, &target_stored, packs)
        .map(EditorRecall::Playing)
        .map_err(|error| format!("The channel switch could not be verified. {error}"))
}

fn select_editor_channel<L: Link>(
    session: &mut Session<L>,
    target: Location,
    target_stored: &EditBuffer,
    packs: Packs,
) -> Result<EditorBuffer, String> {
    session.send(PodxtRequest::Program(target))?;
    session.link.pause(CHANNEL_LOAD);
    session.listen(Duration::from_millis(50))?;
    let tone = read_after_write(session, target)?;
    if !tone.same_tone(target_stored) {
        return Err(
            "The selected channel differs from the saved tone. Refresh the current tone.".into(),
        );
    }
    Ok(EditorBuffer {
        channel: target,
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
    validate_editor_tone(&expected.tone)?;
    if let EditorEdit::Undo(snapshot) = edit {
        validate_editor_tone(&snapshot.tone)?;
        if snapshot.channel != expected.channel {
            return Err("The edit snapshot belongs to a different playing channel.".into());
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
            let controls: Vec<_> = differing_controls(&before, &snapshot.tone).collect();
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
                restore_snapshot(
                    session,
                    expected.channel,
                    &before,
                    &snapshot.tone,
                    controls,
                    &cancelled,
                )?,
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
            if full && !transfer_matches(&before, &changes, &tone) {
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

pub(super) fn check_stop(cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("PODxt Live operation cancelled.".into())
    } else {
        Ok(())
    }
}

pub(super) fn open_editor<L: Link>(
    session: &mut Session<L>,
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    check_stop(cancelled)?;
    session.require_expected_sysex();
    identify_podxt_live(session)?;
    check_stop(cancelled)
}

/// Read the playing channel on a verified PODxt Live session.
pub fn playing_channel<L: Link>(session: &mut Session<L>) -> Result<Location, String> {
    read_global(session, Global::CurrentChannel)?
        .and_then(|index| Location::new(BankKind::Tones, index))
        .ok_or_else(|| "No valid playing-channel reply from the PODxt Live.".into())
}

pub(super) fn check_channel<L: Link>(
    session: &mut Session<L>,
    expected: Location,
) -> Result<(), String> {
    if playing_channel(session)? != expected {
        return Err("The playing channel changed. Get edit buffer for the current channel.".into());
    }
    Ok(())
}

pub(super) fn editor_tone<L: Link>(session: &mut Session<L>) -> Result<EditBuffer, String> {
    let tone = read_edit_buffer(session)?;
    validate_editor_tone(&tone)?;
    Ok(tone)
}

fn validate_editor_tone(tone: &EditBuffer) -> Result<(), String> {
    if !is_edit_buffer(&tone.raw)
        || tone.raw[6] != PODXT_LIVE_MEMBER as u8
        || tone.raw[EDIT_BUFFER_LEN - 1] != 0xf7
        || tone.raw[7..EDIT_BUFFER_LEN - 1]
            .iter()
            .any(|byte| *byte > 127)
    {
        return Err("The tone is not a seven-bit PODxt Live edit buffer.".into());
    }
    Ok(())
}

pub(super) fn still_current(expected: &EditBuffer, observed: &EditBuffer) -> bool {
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
        send_control(session, control, value, |_, _| {})?;
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

fn transfer_matches(
    before: &EditBuffer,
    changes: &[(LiveControl, u16)],
    observed: &EditBuffer,
) -> bool {
    let mut sent = project(before, changes);
    let modified = |setting| {
        changes.iter().any(|&(control, value)| {
            control == LiveControl::Additional(setting) && control.read(before) != value
        })
    };
    for (timing, note) in [
        (Setting::ModSpeed, Setting::ModNote),
        (Setting::DelayTime, Setting::DelayNote),
    ] {
        if (modified(Setting::Tempo) || modified(note))
            && LiveControl::Additional(note).read(&sent) != 0
            && !modified(timing)
        {
            // Firmware can recalculate synchronized time from the sent NOTE/tempo.
            let control = LiveControl::Additional(timing);
            for (cc, byte, _) in control.messages(control.read(observed)) {
                sent.raw[39 + usize::from(cc)] = byte;
            }
        }
    }
    still_current(&sent, observed)
}

fn send_dump<L: Link>(
    session: &mut Session<L>,
    before: &EditBuffer,
    changes: &[(LiveControl, u16)],
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    let draft = TonePayload::from_dump(&project(before, changes))?;
    session.send_chunked_sysex(transfer::EditFrame(&draft), 32, Some(7), || {
        if cancelled() {
            Err("PODxt edit-buffer transfer cancelled.")
        } else {
            Ok(())
        }
    })
}

pub(super) fn read_after_write<L: Link>(
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

fn differing_controls<'a>(
    before: &'a EditBuffer,
    snapshot: &'a EditBuffer,
) -> impl Iterator<Item = LiveControl> + 'a {
    Control::ALL
        .into_iter()
        .map(LiveControl::Basic)
        .chain(Setting::ALL.into_iter().map(LiveControl::Additional))
        .filter(move |control| control.read(before) != control.read(snapshot))
}

fn restore_snapshot<L: Link>(
    session: &mut Session<L>,
    channel: Location,
    before: &EditBuffer,
    snapshot: &EditBuffer,
    mut controls: Vec<LiveControl>,
    cancelled: &impl Fn() -> bool,
) -> Result<EditBuffer, String> {
    let mut observed = before.clone();
    // Model changes can reset companion controls. A second pass repairs the
    // read-back differences using the same encoder, including nonzero wah.
    for _ in 0..2 {
        if controls.is_empty() {
            break;
        }
        let mut changes: Vec<_> = controls
            .iter()
            .map(|&control| (control, control.read(snapshot)))
            .collect();
        for (timing, note) in [
            (Setting::ModSpeed, Setting::ModNote),
            (Setting::DelayTime, Setting::DelayNote),
        ] {
            let rate = LiveControl::Additional(timing);
            let note = LiveControl::Additional(note);
            if controls.contains(&rate) {
                // Turn sync off after model and tempo changes, then restore
                // the original NOTE after sending both rate bytes.
                if !changes.contains(&(note, 0)) {
                    changes.push((note, 0));
                }
                let original_note = note.read(snapshot);
                if original_note != 0 && !changes.contains(&(note, original_note)) {
                    changes.push((note, original_note));
                }
            }
        }
        changes.sort_by_key(|&(control, value)| control.order(value));
        send_controls(session, &changes, cancelled)?;
        observed = read_after_write(session, channel)?;
        // A model that failed to restore must not be sent indefinitely.
        controls = differing_controls(&observed, snapshot).collect();
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

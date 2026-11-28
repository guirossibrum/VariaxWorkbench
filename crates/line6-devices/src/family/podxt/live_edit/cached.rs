use super::*;

/// A same-attachment recall candidate. The caller provides exact addressed
/// bytes, uninterrupted observation through this transaction, and a route
/// on which global 28 is known to report edit-buffer mutations reliably.
pub struct CachedRecall<'a> {
    pub current: &'a EditorBuffer,
    pub source: &'a EditBuffer,
    pub destination: &'a StoredTone,
    pub still_observed: &'a dyn Fn() -> bool,
}

/// Try a clean recall. A miss is returned only before a Program Change and
/// with no global reply outstanding. An error after selection requires a
/// checked refresh instead of retrying the write.
pub fn try_cached_recall<L: Link>(
    session: &mut Session<L>,
    target: Location,
    cache: CachedRecall<'_>,
    cancelled: impl Fn() -> bool,
    outstanding: &mut bool,
) -> Result<Option<EditorBuffer>, String> {
    if target.kind() != BankKind::Tones || !session.allows(Operation::PodxtChannelSwitch) {
        return Err("PODxt tone-channel recall is unavailable.".into());
    }
    let source = cache.current.channel;
    if source == target
        || cache.destination.location != target
        || !(cache.still_observed)()
        || !same_recall_source(&cache.current.tone, cache.source)
    {
        return Ok(None);
    }
    validate_editor_tone(cache.source)?;
    validate_editor_tone(&cache.destination.tone)?;
    open_editor(session, &cancelled)?;
    if !check_clean(session, source, &cancelled, outstanding)? || !(cache.still_observed)() {
        return Ok(None);
    }

    check_stop(&cancelled)?;
    session.send(PodxtRequest::Program(target))?;
    let verified = (|| {
        let until = session.link.elapsed() + CHANNEL_LOAD;
        let orphans = session.assembler.orphans;
        let mut quiet = true;
        while session.link.elapsed() < until {
            check_stop(&cancelled)?;
            let wait = (until - session.link.elapsed()).min(POLL);
            if let Some(events) = session.poll(wait)? {
                quiet &= events.iter().all(|event| benign(event, Some(target)));
            }
        }
        Ok::<_, String>(
            quiet
                && session.assembler.orphans == orphans
                && session.assembler.partial_sysex().is_none()
                && check_clean(session, target, &cancelled, outstanding)?
                && (cache.still_observed)(),
        )
    })()
    .map_err(|error| format!("The channel switch could not be verified. {error}"))?;
    if !verified {
        return Err(
            "The channel switch could not be verified. Read the playing tone again.".into(),
        );
    }
    Ok(Some(EditorBuffer {
        channel: target,
        tone: cache.destination.tone.clone(),
        packs: cache.current.packs,
    }))
}

fn check_clean<L: Link>(
    session: &mut Session<L>,
    channel: Location,
    cancelled: &impl Fn() -> bool,
    outstanding: &mut bool,
) -> Result<bool, String> {
    Ok(
        probe(session, Global::CurrentChannel, cancelled, outstanding)? == Some(channel.index())
            && probe(session, Global::Edited, cancelled, outstanding)? == Some(0),
    )
}

fn probe<L: Link>(
    session: &mut Session<L>,
    global: Global,
    cancelled: &impl Fn() -> bool,
    outstanding: &mut bool,
) -> Result<Option<u16>, String> {
    check_stop(cancelled)?;
    *outstanding = true;
    session.send(PodxtRequest::QueryGlobal(global))?;
    let id = global.id();
    let deadline = session.link.elapsed() + GLOBAL_WINDOW;
    let orphans = session.assembler.orphans;
    let mut quiet = true;
    while session.link.elapsed() < deadline {
        check_stop(cancelled)?;
        let wait = (deadline - session.link.elapsed()).min(POLL);
        let Some(events) = session.poll(wait)? else {
            continue;
        };
        let mut value = None;
        for event in events {
            match &event {
                Event::Sysex(frame)
                    if frame.starts_with(&[0xf0, 0, 1, 0x0c, 3, 0x56, id])
                        && frame.len() == 12
                        && frame[7..11].iter().all(|&digit| digit < 16) =>
                {
                    if value
                        .replace(global_value(frame, id).expect("validated global"))
                        .is_some()
                    {
                        return Err("Ambiguous PODxt global reply.".into());
                    }
                }
                _ => quiet &= benign(&event, None),
            }
        }
        quiet &= session.assembler.orphans == orphans;
        if let Some(value) = value {
            if session.assembler.partial_sysex().is_some() {
                return Err("Incomplete MIDI data after the PODxt global reply.".into());
            }
            *outstanding = false;
            return Ok(quiet.then_some(value));
        }
    }
    Err("No valid PODxt global reply. Reconnect before using device actions.".into())
}

fn benign(event: &Event, selection: Option<Location>) -> bool {
    match event {
        Event::Short(bytes) if bytes[0] & 0xf0 == 0xc0 => {
            selection.is_some_and(|slot| bytes.as_slice() == [0xc0, slot.index() as u8])
        }
        Event::Short(bytes) if bytes[0] & 0xf0 == 0xb0 => LIVE_STATE_CCS.contains(&bytes[1]),
        Event::Sysex(frame) if save_notification(frame).is_some() => false,
        Event::Sysex(_) => {
            controller_notification(event).is_some_and(|(cc, _)| LIVE_STATE_CCS.contains(&cc))
        }
        _ => matches!(tone_event(event), ToneReadEvent::Ignore),
    }
}

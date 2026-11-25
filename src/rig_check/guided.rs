//! Guided physical actions 7.1-7.4 (#260).
//!
//! Each action opens a listening window: nothing is sent while the person
//! acts (a selector message during a read stopped a #238 run). Detection
//! completes the current step on its own, so the person does not have to
//! walk back to the computer, but the next step still shows its own card
//! and waits. A window that sees nothing asks "Didn't see it: Try again /
//! Skip".

use super::practice::{UserAction, preset_for_program_number, preset_label};
use super::procedures::{same_program, variax_full_read, with_podxt, with_variax};
use super::runner::{Ctx, Retry, SKIPPED};
use super::variax_reads::other_preset;
use line6_devices::family::podxt::{self, BankKind, Location};
use line6_devices::family::variax::{self, Preset, VariaxRequest};
use line6_devices::gate::Operation;
use line6_devices::midi::Event;
use line6_devices::session::{Link, POLL, Session};
use std::time::Duration;

/// How long one listening window stays open.
pub const WINDOW: Duration = Duration::from_secs(30);
/// When the simulated person acts after the window opens (practice mode).
pub const PRACTICE_DELAY: Duration = Duration::from_secs(3);

const VARIAX_LISTEN: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];
const PODXT_LISTEN: &[Operation] = &[Operation::PodxtIdentity, Operation::PodxtRead];

/// Poll only, until `seen` accepts an event or the window ends.
pub fn watch<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    stopping: &dyn Fn() -> bool,
    mut seen: impl FnMut(&Event) -> bool,
) -> Result<bool, String> {
    let end = session.link.elapsed() + window;
    while session.link.elapsed() < end {
        if stopping() {
            return Err("Stopped by you.".into());
        }
        let left = end.saturating_sub(session.link.elapsed());
        if let Some(events) = session.poll(left.min(POLL))? {
            for event in &events {
                if seen(event) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Open the Variax listening session (bridge and MIDI session on), watch,
/// then close it again. Nothing is sent while the window is open.
pub fn watch_variax<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    stopping: &dyn Fn() -> bool,
    seen: impl FnMut(&Event) -> bool,
) -> Result<bool, String> {
    variax::set_bridge_for_read(session, true)?;
    session.link.pause(Duration::from_secs(1));
    let result = session.send(VariaxRequest::SessionOn).and_then(|()| {
        session.note("Listening window open: nothing is sent until it closes.");
        watch(session, window, stopping, seen)
    });
    let off = session.send(VariaxRequest::SessionOff);
    let release = variax::set_bridge_for_read(session, false);
    let seen = result?;
    off?;
    release?;
    Ok(seen)
}

/// One guided window with retries on the person's request.
fn guided<T>(
    ctx: &mut Ctx<'_>,
    see: &str,
    hear: &str,
    what: &str,
    practice: &[UserAction],
    mut window: impl FnMut(&mut Ctx<'_>) -> Result<Option<T>, String>,
) -> Result<T, String> {
    loop {
        if ctx.stopping() {
            return Err("Stopped by you before the listening window opened.".into());
        }
        ctx.op.instruct(ctx.rec.id, see, hear);
        // Practice mode: the simulated person acts a few seconds later.
        for action in practice {
            ctx.port.user_acts(action.clone(), PRACTICE_DELAY);
        }
        if let Some(found) = window(ctx)? {
            ctx.rec.note(format!("Seen: {what}."));
            return Ok(found);
        }
        ctx.rec
            .note(format!("Not seen within {} s: {what}.", WINDOW.as_secs()));
        match ctx.op.not_seen(ctx.rec.id, what) {
            Retry::TryAgain => ctx.rec.note("You chose Try again."),
            Retry::Skip => {
                return Err(format!("{SKIPPED}Didn't see it ({what}); you chose Skip."));
            }
        }
    }
}

fn program_seen(target: Preset) -> impl FnMut(&Event) -> bool {
    move |event| matches!(event, Event::Short(m) if m.len() == 2 && m[0] & 0xf0 == 0xc0 && preset_for_program_number(m[1]) == Some(target))
}

fn variax_program_window(ctx: &mut Ctx<'_>, target: Preset) -> Result<Option<()>, String> {
    let seen = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
        watch_variax(session, WINDOW, stopping, program_seen(target))
    })?;
    Ok(seen.then_some(()))
}

/// 7.1: the person turns the selector to a named preset and back.
pub fn selector(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_preset
        .ok_or("The starting preset is unknown (3.4).")?;
    let start_program = ctx.data.start_program.ok_or("No starting program (3.3).")?;
    let target = other_preset(start);
    let name = preset_label(target);
    guided(
        ctx,
        &format!("Turn the selector to {name}."),
        &format!("{name}."),
        &format!("program change to {name}"),
        &[UserAction::TurnSelector(target)],
        |ctx| variax_program_window(ctx, target),
    )?;
    let read = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
        super::procedures::drain(session)?;
        variax_full_read(session, stopping)
    })?;
    let backup = ctx
        .data
        .variax_library
        .iter()
        .find(|(p, _)| *p == target)
        .map(|(_, w)| *w);
    if backup.is_some_and(|b| same_program(&read.words, &b)) {
        ctx.rec.show(format!(
            "{name} plays the program from the 3.4 backup (1,092 words)."
        ));
    } else {
        return Err(format!("{name} does not match the 3.4 backup."));
    }
    let start_name = preset_label(start);
    guided(
        ctx,
        &format!("Turn the selector back to {start_name}."),
        &format!("{start_name}, your starting sound."),
        &format!("program change back to {start_name}"),
        &[UserAction::TurnSelector(start)],
        |ctx| variax_program_window(ctx, start),
    )?;
    let back = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
        super::procedures::drain(session)?;
        variax_full_read(session, stopping)
    })?;
    if !same_program(&back.words, &start_program) {
        return Err(format!(
            "Back on {start_name}, but the program differs from the start."
        ));
    }
    ctx.rec.show(format!(
        "Back on {start_name}: all 1,092 words match the start."
    ));
    Ok(())
}

#[derive(Default, Clone, Copy)]
struct Range {
    min: Option<u8>,
    max: Option<u8>,
}

impl Range {
    fn add(&mut self, value: u8) {
        self.min = Some(self.min.map_or(value, |m| m.min(value)));
        self.max = Some(self.max.map_or(value, |m| m.max(value)));
    }

    /// Swept far enough to count as a sweep.
    fn swept(self) -> bool {
        self.min.is_some_and(|m| m <= 20) && self.max.is_some_and(|m| m >= 107)
    }

    fn text(self) -> String {
        match (self.min, self.max) {
            (Some(min), Some(max)) => format!("{min}-{max}"),
            _ => "not seen".into(),
        }
    }
}

/// 7.2: the person sweeps the Variax volume (CC 7) and tone (CC 79) knobs.
pub fn knobs(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let (volume, tone) = guided(
        ctx,
        "Turn the volume knob down and up, then the tone knob down and up. Leave them where they were.",
        "The volume and the tone change while you turn.",
        "volume (CC 7) and tone (CC 79) sweeps",
        &[UserAction::SweepVariaxKnobs],
        |ctx| {
            let mut volume = Range::default();
            let mut tone = Range::default();
            let seen = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
                watch_variax(session, WINDOW, stopping, |event| {
                    if let Event::Short(m) = event
                        && m.len() == 3
                        && m[0] & 0xf0 == 0xb0
                    {
                        match m[1] {
                            7 => volume.add(m[2]),
                            79 => tone.add(m[2]),
                            _ => {}
                        }
                    }
                    volume.swept() && tone.swept()
                })
            })?;
            ctx.rec.note(format!(
                "Volume (CC 7) range {}, tone (CC 79) range {}.",
                volume.text(),
                tone.text()
            ));
            Ok(seen.then_some((volume, tone)))
        },
    )?;
    ctx.rec.show(format!(
        "Both knobs swept: volume {}, tone {}.",
        volume.text(),
        tone.text()
    ));
    Ok(())
}

fn podxt_window(ctx: &mut Ctx<'_>, seen: impl FnMut(&Event) -> bool) -> Result<bool, String> {
    let op = &*ctx.op;
    let stopping = || op.stop_requested();
    let step = ctx.rec.id;
    let mode = ctx.mode;
    let mut lines = super::procedures::Lines::default();
    let policy = super::procedures::StepPolicy {
        step,
        mode,
        ops: PODXT_LISTEN,
    };
    let result = match ctx.port.podxt(step) {
        Ok(link) => {
            let mut session = Session::new(link, &policy, &mut lines);
            session.note("Listening window open: nothing is sent until it closes.");
            watch(&mut session, WINDOW, &stopping, seen)
        }
        Err(error) => Err(error),
    };
    ctx.rec.messages.extend(lines.0);
    result
}

fn channel_window(ctx: &mut Ctx<'_>, target: Location) -> Result<Option<()>, String> {
    let index = target.index() as u8;
    let seen = podxt_window(
        ctx,
        |event| matches!(event, Event::Short(m) if m.len() == 2 && m[0] & 0xf0 == 0xc0 && m[1] == index),
    )?;
    Ok(seen.then_some(()))
}

/// 7.3: the person switches the PODxt Live to a named channel and back.
pub fn footswitch(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let start = ctx
        .data
        .start_channel
        .ok_or("The starting channel is unknown (2.5).")?;
    let start_tone = ctx
        .data
        .start_tone
        .clone()
        .ok_or("No starting tone (2.2).")?;
    let target = Location::new(
        BankKind::Tones,
        (start.index() & !3) + (start.index() + 1) % 4,
    )
    .expect("same bank");
    guided(
        ctx,
        &format!("Press the footswitch for channel {}.", target.label()),
        &format!("Channel {}.", target.label()),
        &format!("channel change to {}", target.label()),
        &[UserAction::PodxtFootswitch(target.index())],
        |ctx| channel_window(ctx, target),
    )?;
    let now = with_podxt(ctx, PODXT_LISTEN, |session| {
        podxt::read_current(session).map(|current| current.tone)
    })?;
    let stored = ctx
        .data
        .podxt_library
        .iter()
        .find(|(l, _)| *l == target)
        .map(|(_, t)| t.clone());
    if !stored.is_some_and(|s| s.same_tone(&now)) {
        return Err(format!(
            "Channel {} does not match the 2.5 backup.",
            target.label()
        ));
    }
    ctx.rec.show(format!(
        "Channel {} plays its stored tone {:?}.",
        target.label(),
        now.name()
    ));
    guided(
        ctx,
        &format!("Press the footswitch for channel {} again.", start.label()),
        &format!("Channel {}, your starting tone.", start.label()),
        &format!("channel change back to {}", start.label()),
        &[UserAction::PodxtFootswitch(start.index())],
        |ctx| channel_window(ctx, start),
    )?;
    let back = with_podxt(ctx, PODXT_LISTEN, |session| {
        podxt::read_current(session).map(|current| current.tone)
    })?;
    if !back.same_tone(&start_tone) {
        return Err(format!(
            "Back on {}, but the tone differs from the start.",
            start.label()
        ));
    }
    ctx.rec.show(format!(
        "Back on channel {}: the starting tone.",
        start.label()
    ));
    Ok(())
}

/// 7.4: the person rocks the PODxt Live pedal (CC 7, or CC 4 in wah mode).
pub fn pedal(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let range = guided(
        ctx,
        "Rock the pedal from heel to toe and back, then leave it where it was.",
        "The volume or the wah follows the pedal.",
        "pedal sweep (CC 7 or CC 4)",
        &[UserAction::MovePedal],
        |ctx| {
            let mut range = Range::default();
            let seen = podxt_window(ctx, |event| {
                if let Event::Short(m) = event
                    && m.len() == 3
                    && m[0] & 0xf0 == 0xb0
                    && matches!(m[1], 7 | 4)
                {
                    range.add(m[2]);
                }
                range.swept()
            })?;
            Ok(seen.then_some(range))
        },
    )?;
    ctx.rec.show(format!("Pedal range seen: {}.", range.text()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::{NoReport, Retry};
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    const GUIDED: [StepId; 4] = [StepId(7, 1), StepId(7, 2), StepId(7, 3), StepId(7, 4)];

    #[test]
    fn detection_completes_each_guided_step_without_sends_in_the_window() {
        let mut person = Script::run_all();
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        for id in GUIDED {
            let step = run.step(id);
            assert!(step.outcome.passed(), "{id}: {step:?}");
            // No message is sent between "window open" and the first received event.
            let mut open = false;
            for line in &step.messages {
                if line.contains("Listening window open") {
                    open = true;
                } else if open && line.contains(" OUT ") {
                    panic!("{id}: sent while listening: {line}");
                } else if line.contains(" IN ") {
                    open = false;
                }
            }
        }
        assert!(
            run.step(StepId(7, 2))
                .evidence
                .iter()
                .any(|l| l.starts_with("Both knobs swept: volume 0-127"))
        );
        assert!(
            run.step(StepId(7, 4))
                .evidence
                .iter()
                .any(|l| l == "Pedal range seen: 0-127.")
        );
        let instructions: Vec<_> = person
            .instructions
            .iter()
            .filter(|(id, _)| id.0 == 7)
            .collect();
        assert_eq!(
            instructions.len(),
            6,
            "two windows each for 7.1 and 7.3, one for 7.2 and 7.4"
        );
        // The next step still showed its own card after each detection.
        for id in GUIDED {
            assert!(person.cards.contains(&id));
        }
    }

    #[test]
    fn a_missed_action_offers_try_again_or_skip() {
        let mut person = Script::run_all();
        person.retry = Retry::Skip;
        let faults = Faults {
            user_does_not_act: true,
            ..Faults::default()
        };
        let (run, _, rig) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        for id in GUIDED {
            assert_eq!(run.step(id).outcome, Outcome::SkippedByPerson, "{id}");
            assert!(
                run.step(id)
                    .evidence
                    .iter()
                    .any(|l| l.starts_with("Not seen within 30 s"))
            );
        }
        assert!(
            run.step(StepId(6, 2)).outcome.passed(),
            "the rig is still at its start"
        );
        drop(rig);
    }
}

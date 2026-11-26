//! Guided physical actions 7.1-7.4 (#260).
//!
//! Each action opens a listening window: nothing is sent while the person
//! acts (a selector message during a read stopped a #238 run). Detection
//! completes the current step on its own, so the person does not have to
//! walk back to the computer, but the next step still shows its own card
//! and waits. A window that sees nothing asks "Didn't see it: Try again /
//! Skip".

use super::practice::{UserAction, preset_label};
use super::procedures::{same_program, variax_full_read, with_podxt, with_variax};
use super::runner::{Ctx, Retry, SKIPPED};
use super::variax_reads::other_preset;
use line6_devices::family::podxt::{self, BankKind, Location};
use line6_devices::family::variax::{self, FollowUpdate, Preset};
use line6_devices::gate::Operation;
use line6_devices::session::{Link, Session};
use std::cell::Cell;
use std::time::Duration;

/// How long one listening window stays open.
pub const WINDOW: Duration = Duration::from_secs(30);
/// When the simulated person acts after the window opens (practice mode).
pub const PRACTICE_DELAY: Duration = Duration::from_secs(5);

const VARIAX_LISTEN: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];
const PODXT_LISTEN: &[Operation] = &[Operation::PodxtIdentity, Operation::PodxtRead];

/// Use the editor's decoded Variax Follow stream until the action is observed.
pub fn watch_variax<L: Link>(
    session: &mut Session<L>,
    waiting_for_replug: bool,
    stopping: &dyn Fn() -> bool,
    mut accepts: impl FnMut(FollowUpdate) -> bool,
) -> Result<bool, String> {
    let seen = Cell::new(false);
    variax::follow_window(
        session,
        WINDOW,
        waiting_for_replug,
        || stopping() || seen.get(),
        |update| seen.set(accepts(update)),
    )?;
    if stopping() {
        return Err("Stopped by you.".into());
    }
    Ok(seen.get())
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

fn variax_program_window(ctx: &mut Ctx<'_>, target: Preset) -> Result<Option<()>, String> {
    let seen = with_variax(ctx, VARIAX_LISTEN, |session, stopping| {
        watch_variax(session, false, stopping, |update| {
            update == FollowUpdate::ProgramChange(target)
        })
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

    fn full_range(self) -> bool {
        self.min == Some(0) && self.max == Some(127)
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
                watch_variax(session, false, stopping, |update| {
                    match update {
                        FollowUpdate::Volume(value) => volume.add(value),
                        FollowUpdate::TonePosition(value) => tone.add(value),
                        _ => {}
                    }
                    volume.full_range() && tone.full_range()
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

fn podxt_window(
    ctx: &mut Ctx<'_>,
    mut accepts: impl FnMut(podxt::ReadUpdate) -> bool,
) -> Result<bool, String> {
    let op = &*ctx.op;
    let step = ctx.rec.id;
    let mut lines = super::procedures::Lines::default();
    let seen = Cell::new(false);
    let policy = super::procedures::StepPolicy {
        step,
        mode: ctx.mode,
        ops: PODXT_LISTEN,
    };
    let result = match ctx.port.podxt(step) {
        Ok(link) => {
            let mut session = Session::new(link, &policy, &mut lines);
            podxt::follow_window(
                &mut session,
                WINDOW,
                || op.stop_requested() || seen.get(),
                |update| {
                    seen.set(accepts(update));
                },
            )
        }
        Err(error) => Err(error),
    };
    super::procedures::absorb(ctx, lines);
    match result {
        Ok(()) => Ok(seen.get()),
        Err(error) if seen.get() && error == "PODxt read cancelled." => Ok(true),
        Err(error) => Err(error),
    }
}

fn channel_window(
    ctx: &mut Ctx<'_>,
    target: Location,
    expected_name: &str,
) -> Result<Option<podxt::EditBuffer>, String> {
    let mut primed = false;
    let mut matching_channel = false;
    let mut readback = None;
    podxt_window(ctx, |update| match update {
        podxt::ReadUpdate::Channel(location) if primed => {
            matching_channel = location == target;
            false
        }
        podxt::ReadUpdate::Current(tone) => {
            if primed && matching_channel {
                readback = Some(tone);
                true
            } else {
                primed = true;
                false
            }
        }
        _ => false,
    })?;
    match readback {
        Some(tone) if tone.name() != expected_name => Err(format!(
            "Follow re-read channel {} as {:?}, expected {expected_name:?}.",
            target.label(),
            tone.name()
        )),
        result => Ok(result),
    }
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
    let stored = ctx
        .data
        .podxt_library
        .iter()
        .find(|(loc, _)| *loc == target)
        .map(|(_, tone)| tone.clone())
        .ok_or("The target channel is missing from the backup.")?;
    let now = guided(
        ctx,
        &format!("Press the footswitch for channel {}.", target.label()),
        &format!("Channel {}.", target.label()),
        &format!("channel change to {}", target.label()),
        &[UserAction::PodxtFootswitch(target.index())],
        |ctx| channel_window(ctx, target, &stored.name()),
    )?;
    if !stored.same_tone(&now) {
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
    let back = guided(
        ctx,
        &format!("Press the footswitch for channel {} again.", start.label()),
        &format!("Channel {}, your starting tone.", start.label()),
        &format!("channel change back to {}", start.label()),
        &[UserAction::PodxtFootswitch(start.index())],
        |ctx| channel_window(ctx, start, &start_tone.name()),
    )?;
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
                if let podxt::ReadUpdate::Controller {
                    number: 7 | 4,
                    value,
                } = event
                {
                    range.add(value);
                }
                range.swept()
            })?;
            Ok(seen.then_some(range))
        },
    )?;
    ctx.rec.show(format!("Pedal range seen: {}.", range.text()));
    Ok(())
}

/// 7.5: confirm the editor's decoded controller update against the tone dump.
pub fn podxt_knob(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let cc = podxt::Control::Drive.cc();
    let start = with_podxt(ctx, PODXT_LISTEN, |session| {
        podxt::read_current(session).map(|current| current.tone)
    })?;
    let before = start.cc(cc);
    let trial = if before > 64 {
        before - 20
    } else {
        before + 20
    };
    let observe = |want_original: bool, ctx: &mut Ctx<'_>| {
        let mut value = None;
        podxt_window(ctx, |update| {
            if let podxt::ReadUpdate::Controller {
                number,
                value: observed,
            } = update
                && number == cc
                && (observed == before) == want_original
            {
                value = Some(observed);
            }
            value.is_some()
        })?;
        if let Some(value) = value {
            let current = with_podxt(ctx, PODXT_LISTEN, |session| {
                podxt::read_current(session).map(|current| current.tone)
            })?;
            if current.cc(cc) != value {
                return Err("The Drive value read back differs from the Follow update.".into());
            }
        }
        Ok(value)
    };
    let changed = guided(
        ctx,
        &format!("Turn Drive away from its starting value of {before}."),
        "The drive setting changes.",
        "a new Drive value",
        &[UserAction::PodxtKnob(trial)],
        |ctx| observe(false, ctx),
    )?;
    ctx.rec.show(format!(
        "Drive changed from {before} to {changed}, verified in the tone."
    ));
    guided(
        ctx,
        &format!("Turn Drive back to {before}."),
        "Your starting Drive setting.",
        "the starting Drive value",
        &[UserAction::PodxtKnob(before)],
        |ctx| observe(true, ctx),
    )?;
    ctx.rec
        .show(format!("Drive is back at {before}, verified in the tone."));
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

    const GUIDED: [StepId; 5] = [
        StepId(7, 1),
        StepId(7, 2),
        StepId(7, 3),
        StepId(7, 4),
        StepId(7, 5),
    ];

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
            8,
            "two windows each for selector, footswitch and Drive; one for knobs and pedal"
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

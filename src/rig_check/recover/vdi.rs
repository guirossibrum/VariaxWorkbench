//! Rig Check 6.1: observe the VDI cable, then verify the returning guitar.

use crate::rig_check::guided::{ActiveAction, begin_action, missed};
use crate::rig_check::practice::UserAction;
use crate::rig_check::procedures::{drain, with_variax};
use crate::rig_check::runner::{Ctx, Progress, STOPPED};
use line6_devices::family::variax::{self, FollowUpdate};
use line6_devices::gate::Operation;
use line6_devices::identity::Identity;
use std::cell::Cell;
use std::time::Duration;

const VDI_OPS: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];
const LISTEN_INTERVAL: Duration = Duration::from_secs(8);
const READY_POLL: Duration = Duration::from_millis(25);
const HOLD_CABLE: &str = "Checking guitar identity. Leave the VDI cable where it is.";

#[derive(Clone, Copy)]
enum Phase {
    Unplug,
    Reconnect,
}

impl Phase {
    fn expected(self) -> &'static str {
        match self {
            Self::Unplug => "the VDI cable to be unplugged",
            Self::Reconnect => "the guitar to reconnect and identify itself",
        }
    }

    fn cue(self) -> (&'static str, &'static str, UserAction) {
        match self {
            Self::Unplug => (
                "Unplug the VDI cable from the guitar. Leave the PODxt Live USB cable connected.",
                "After unplugging, the guitar goes silent.",
                UserAction::UnplugVdi,
            ),
            Self::Reconnect => (
                "Plug the VDI cable back into the guitar.",
                "After reconnecting, the guitar sound returns.",
                UserAction::ReplugVdi,
            ),
        }
    }

    fn matches_notice(self, update: FollowUpdate) -> bool {
        matches!(
            (self, update),
            (Self::Unplug, FollowUpdate::Disconnected) | (Self::Reconnect, FollowUpdate::Connected)
        )
    }
}

fn check_stop(stopping: bool) -> Result<(), String> {
    if stopping {
        Err(format!("{STOPPED}You stopped the VDI cable check."))
    } else {
        Ok(())
    }
}

fn check_identity(
    ctx: &mut Ctx<'_>,
    prior: &Identity,
    action: Option<&ActiveAction>,
) -> Result<bool, String> {
    check_stop(ctx.stopping())?;
    ctx.op.progress(Progress::action(HOLD_CABLE));
    with_variax(ctx, VDI_OPS, |session, stopping| {
        drain(session)?;
        if let Some(action) = action {
            // Poll the terminal's Stop key as well as the GUI stop flag.
            action.done();
        }
        check_stop(stopping())?;
        let reply = variax::identify_if_present(session);
        if let Some(action) = action {
            action.done();
        }
        check_stop(stopping())?;
        match reply? {
            None => Ok(false),
            Some(reported) if reported.identity == *prior => Ok(true),
            Some(_) => {
                Err("The guitar answered with a different identity than in step 3.1.".into())
            }
        }
    })
}

fn wait_until_ready(ctx: &mut Ctx<'_>, action: &ActiveAction) -> Result<(), String> {
    with_variax(ctx, VDI_OPS, |session, stopping| {
        while action.early() && !action.done() && !stopping() {
            session.link.pause(READY_POLL);
        }
        check_stop(stopping())
    })
}

fn listen(ctx: &mut Ctx<'_>, phase: Phase, action: &ActiveAction) -> Result<bool, String> {
    with_variax(ctx, VDI_OPS, |session, stopping| {
        let deadline = action.elapsed() + LISTEN_INTERVAL;
        let detected = Cell::new(false);
        // Follow closes its listener on any disconnect. If a duplicate
        // disconnect comes in during reconnect, finish the remainder of
        // this interval before checking identity.
        loop {
            variax::follow_action(
                session,
                || detected.get() || action.done() || stopping() || action.elapsed() >= deadline,
                |update| detected.set(phase.matches_notice(update)),
            )?;
            if detected.get() || action.done() || stopping() || action.elapsed() >= deadline {
                break;
            }
        }
        check_stop(stopping())?;
        Ok(detected.get())
    })
}

fn report_restored(ctx: &mut Ctx<'_>, prior: &Identity) {
    let restored = format!(
        "VDI restored. The guitar's identity matches step 3.1 (firmware {}).",
        prior.version_text()
    );
    ctx.op.progress(Progress::action(&restored));
    ctx.rec.show(restored);
}

fn run_phase(ctx: &mut Ctx<'_>, prior: &Identity, phase: Phase) -> Result<(), String> {
    let (see, hear, user_action) = phase.cue();
    let what = phase.expected();
    let get_ready = match phase {
        Phase::Unplug => "Wait for the countdown before unplugging the VDI cable from the guitar.",
        Phase::Reconnect => {
            "Disconnect detected. Wait for the countdown before plugging the VDI cable back into the guitar."
        }
    };
    let mut recheck = matches!(phase, Phase::Unplug);
    loop {
        if recheck {
            match (phase, check_identity(ctx, prior, None)?) {
                (Phase::Unplug, false) => {
                    ctx.rec.show("The guitar is already unplugged or not answering. The reconnect check comes next.");
                    return Ok(());
                }
                (Phase::Reconnect, true) => {
                    ctx.rec
                        .show("The guitar already answers with its starting identity.");
                    report_restored(ctx, prior);
                    return Ok(());
                }
                _ => {}
            }
        }

        let mut action = begin_action(
            ctx,
            get_ready,
            hear,
            what,
            std::slice::from_ref(&user_action),
        )?;
        wait_until_ready(ctx, &action)?;

        let mut connect_notice_seen = false;
        loop {
            let notice = if action.done() {
                false
            } else {
                ctx.op.instruct(ctx.rec.id, see, hear);
                listen(ctx, phase, &action)?
            };
            match (phase, notice) {
                (Phase::Unplug, true) => {
                    ctx.rec.show("Disconnect detected. The PODxt Live reported that the guitar disconnected.");
                    return Ok(());
                }
                (Phase::Reconnect, true) if !connect_notice_seen => {
                    ctx.rec
                        .show("The PODxt Live reported that the guitar reconnected.");
                    connect_notice_seen = true;
                }
                _ => {}
            }
            let answering = check_identity(ctx, prior, Some(&action))?;
            match (phase, answering) {
                (Phase::Unplug, false) => {
                    ctx.rec.show(
                        "Disconnect detected by identity check. The guitar stopped answering.",
                    );
                    return Ok(());
                }
                (Phase::Reconnect, true) => {
                    if !connect_notice_seen {
                        ctx.rec.show(
                            "Reconnect detected by identity check. The guitar answered again.",
                        );
                    }
                    report_restored(ctx, prior);
                    return Ok(());
                }
                _ => {}
            }
            if action.done() {
                check_stop(ctx.stopping())?;
                missed(
                    ctx,
                    what,
                    if answering {
                        "the guitar still answering"
                    } else {
                        "no guitar identity reply"
                    },
                )?;
                recheck = true;
                break;
            }
            ctx.op
                .progress(Progress::watching(format!("Watching for {what}")));
            action.resume(ctx, what);
        }
    }
}

pub(in crate::rig_check) fn vdi_replug(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let prior = ctx
        .data
        .variax_identity
        .clone()
        .ok_or("No verified guitar identity from 3.1.")?;
    run_phase(ctx, &prior, Phase::Unplug)?;
    run_phase(ctx, &prior, Phase::Reconnect)
}

#[cfg(test)]
mod tests;

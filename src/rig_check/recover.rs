//! Recovery (#261): step 6.1 (VDI cable unplug and replug noticed) and the
//! crash restore offered on the next launch.
//!
//! While a run is going, a small state file next to the report records the
//! starting PODxt channel and Variax preset with hashes of the starting tone
//! and program. A finished run marks it finished. When Rig Check starts and
//! finds an unfinished state file, it offers (default No) to switch the rig
//! back to that channel and preset and to check the hashes. That restore is a
//! device action: in real mode it needs the 6.2 owner approval like the rig
//! guard.

use super::catalogue::{self, RIG_GUARD};
use super::guided::{begin_action, missed, watch_variax};
use super::practice::{Program, UserAction, preset_label};
use super::procedures::{
    TONE_POSITION_WORDS, variax_read_preset, with_variax, with_variax_progress,
};
use super::record::{Mode, Outcome, RunRecord, StepRecord, sha256_hex, utc_now};
use super::runner::{Ctx, Decision, Operator, Progress, ReportSink};
use super::steps::RunData;
use line6_devices::family::podxt::{self, BankKind, Location};
use line6_devices::family::variax::{self, FollowUpdate, Preset, procedures as shared_variax};
use line6_devices::gate::Operation;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ------------------------------------------------------------------ 6.1

const VDI_OPS: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];

fn vdi_state(ctx: &mut Ctx<'_>) -> Result<Option<Duration>, String> {
    let prior = ctx
        .data
        .variax_identity
        .clone()
        .ok_or("No verified guitar identity from 3.1.")?;
    with_variax(ctx, VDI_OPS, |session, _| {
        super::procedures::drain(session)?;
        let Some(reported) = variax::identify_if_present(session)? else {
            return Ok(None);
        };
        if reported.identity != prior {
            return Err("The guitar answered with a different identity than in step 3.1.".into());
        }
        Ok(Some(session.link.elapsed()))
    })
}

fn vdi_window(ctx: &mut Ctx<'_>, connected: bool, action: UserAction) -> Result<(), String> {
    let what = if connected {
        "the guitar to connect"
    } else {
        "the guitar to disconnect"
    };
    loop {
        let current = vdi_state(ctx)?.is_some();
        if current == connected {
            if current {
                ctx.rec.show(
                    "The guitar is already connected and answers with its starting identity.",
                );
            } else {
                ctx.rec.show("The guitar is already unplugged or not answering. Plug it back in at the next prompt.");
            }
            return Ok(());
        }
        let (see, hear) = if connected {
            (
                "The guitar is not answering. Plug the VDI cable back in, then press Continue if the screen is still waiting.",
                "Your guitar again.",
            )
        } else {
            (
                "Unplug the VDI cable from the guitar after OK.",
                "Silence from the guitar.",
            )
        };
        let advance = begin_action(ctx, see, hear, what, std::slice::from_ref(&action))?;
        let (seen, at) = with_variax(ctx, VDI_OPS, |session, stopping| {
            let seen = watch_variax(session, stopping, &advance, |update| {
                matches!(
                    (connected, update),
                    (true, FollowUpdate::Connected) | (false, FollowUpdate::Disconnected)
                )
            })?;
            Ok((seen, session.link.elapsed()))
        })?;
        if seen {
            let state = if connected {
                "connected again"
            } else {
                "disconnected"
            };
            ctx.rec.show(format!(
                "The PODxt Live reported the guitar {state} (link time {:.3} s).",
                at.as_secs_f64()
            ));
            return Ok(());
        }
        let answering = vdi_state(ctx)?.is_some();
        if answering == connected {
            let state = if connected {
                "The guitar answered with its starting identity."
            } else {
                "The guitar stopped answering."
            };
            ctx.rec
                .show(format!("{state} No connection notice was decoded."));
            return Ok(());
        }
        missed(
            ctx,
            what,
            if answering {
                "the guitar still answering"
            } else {
                "no guitar identity reply"
            },
        )?;
    }
}

/// 6.1: the person unplugs the VDI cable and plugs it back; the identity is
/// read again afterwards.
pub fn vdi_replug(ctx: &mut Ctx<'_>) -> Result<(), String> {
    vdi_window(ctx, false, UserAction::UnplugVdi)?;
    vdi_window(ctx, true, UserAction::ReplugVdi)?;
    let identified = with_variax(ctx, VDI_OPS, |session, _| {
        super::procedures::drain(session)?;
        shared_variax::identify_reported(session)
    })?;
    let same = ctx
        .data
        .variax_identity
        .as_ref()
        .is_some_and(|before| *before == identified.identity);
    ctx.rec.show(format!(
        "Identity after the replug: firmware {}, {}.",
        identified.identity.version_text(),
        if same {
            "the same guitar as in 3.1"
        } else {
            "DIFFERENT from 3.1"
        }
    ));
    if !same {
        return Err("The guitar answered with a different identity after the replug.".into());
    }
    Ok(())
}

// ------------------------------------------------------------------ crash restore

/// The program with the tone-position words zeroed, so a knob move does not
/// change the hash.
pub fn match_hash(words: &Program) -> String {
    let mut copy = *words;
    for word in TONE_POSITION_WORDS {
        copy[word] = 0;
    }
    sha256_hex(&super::procedures::program_bytes(&copy))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RigState {
    pub report: String,
    pub started: String,
    pub podxt_channel: Option<u16>,
    pub podxt_tone_sha256: Option<String>,
    /// Bank and slot.
    pub variax_preset: Option<(u8, u8)>,
    pub variax_match_sha256: Option<String>,
    pub finished: bool,
}

impl RigState {
    pub fn from_run(run: &RunRecord, data: &RunData, report: &str) -> Self {
        Self {
            report: report.into(),
            started: run.started.clone(),
            podxt_channel: data.start_channel.map(|c| c.index()),
            podxt_tone_sha256: data.start_tone.as_ref().map(|t| sha256_hex(&t.raw)),
            variax_preset: data.start_preset.map(|p| (p.bank(), p.slot())),
            variax_match_sha256: data.start_program.as_ref().map(match_hash),
            finished: run.finished,
        }
    }

    pub fn text(&self) -> String {
        let opt = |v: &Option<String>| v.clone().unwrap_or_default();
        format!(
            "rig-check-state 1\nreport={}\nstarted={}\npodxt_channel={}\npodxt_tone_sha256={}\nvariax_preset={}\nvariax_match_sha256={}\nfinished={}\n",
            self.report,
            self.started,
            self.podxt_channel.map_or(String::new(), |c| c.to_string()),
            opt(&self.podxt_tone_sha256),
            self.variax_preset
                .map_or(String::new(), |(b, s)| format!("{b}.{s}")),
            opt(&self.variax_match_sha256),
            self.finished
        )
    }

    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()? != "rig-check-state 1" {
            return None;
        }
        let mut state = Self::default();
        for line in lines {
            let (key, value) = line.split_once('=')?;
            let some = |v: &str| (!v.is_empty()).then(|| v.to_owned());
            match key {
                "report" => state.report = value.into(),
                "started" => state.started = value.into(),
                "podxt_channel" => state.podxt_channel = value.parse().ok(),
                "podxt_tone_sha256" => state.podxt_tone_sha256 = some(value),
                "variax_preset" => {
                    state.variax_preset = value
                        .split_once('.')
                        .and_then(|(b, s)| Some((b.parse().ok()?, s.parse().ok()?)))
                }
                "variax_match_sha256" => state.variax_match_sha256 = some(value),
                "finished" => state.finished = value == "true",
                _ => {}
            }
        }
        Some(state)
    }

    pub fn channel(&self) -> Option<Location> {
        Location::new(BankKind::Tones, self.podxt_channel?)
    }

    pub fn preset(&self) -> Option<Preset> {
        let (bank, slot) = self.variax_preset?;
        Preset::new(bank, slot)
    }

    pub fn describe(&self) -> String {
        format!(
            "The run started {} did not finish (report {}). Starting PODxt Live channel: {}. Starting Variax preset: {}.",
            self.started,
            self.report,
            self.channel().map_or("unknown".into(), |c| c.label()),
            self.preset().map_or("unknown".into(), preset_label)
        )
    }
}

/// Write the state file next to the report (atomic replace of our own file).
pub fn write_state(path: &Path, state: &RigState) -> Result<(), String> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    file.write_all(state.text().as_bytes())
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path)
        .map_err(|e| format!("{}: {}", path.display(), e.error))?;
    Ok(())
}

/// Unfinished runs in a report folder, newest first.
pub fn pending(dir: &Path) -> Vec<(PathBuf, RigState)> {
    let mut found: Vec<(PathBuf, RigState)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "state"))
                .filter_map(|p| {
                    let state = RigState::parse(&std::fs::read_to_string(&p).ok()?)?;
                    (!state.finished && (state.channel().is_some() || state.preset().is_some()))
                        .then_some((p, state))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by(|a, b| b.1.started.cmp(&a.1.started));
    found
}

const RESTORE_OPS: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtChannelSwitch,
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
];

fn restore(ctx: &mut Ctx<'_>, state: &RigState) -> Result<(), String> {
    let mut problems = Vec::new();
    if let Some(channel) = state.channel() {
        let tone = super::procedures::with_podxt(ctx, RESTORE_OPS, |session| {
            podxt::restore_channel(session, channel).map(|current| current.tone)
        })?;
        let hash = sha256_hex(&tone.raw);
        if Some(&hash) == state.podxt_tone_sha256.as_ref() {
            ctx.rec.show(format!(
                "Rig: the PODxt Live is back on channel {} and its tone matches the start.",
                channel.label()
            ));
        } else {
            ctx.rec.note(format!(
                "Rig: the PODxt Live is on channel {}; its tone hash {} differs from the start (pedal or knob values can differ).",
                channel.label(),
                &hash[..16]
            ));
        }
    }
    if let Some(preset) = state.preset() {
        if state.channel().is_none() {
            super::procedures::with_podxt(ctx, RESTORE_OPS, |session| {
                podxt::identify_podxt_live(session).map(|_| ())
            })?;
        }
        let read = with_variax_progress(ctx, RESTORE_OPS, |session, _, progress| {
            super::procedures::drain(session)?;
            variax_read_preset(
                session,
                preset,
                &|| false,
                progress,
                "Verifying the return to your starting preset",
            )
        })?;
        if Some(match_hash(&read.words)) == state.variax_match_sha256 {
            ctx.rec.show(format!(
                "Rig: the Variax is back on {} and its program matches the start.",
                preset_label(preset)
            ));
        } else {
            problems.push(format!(
                "The Variax is on {}, but its program differs from the start.",
                preset_label(preset)
            ));
        }
    }
    if !ctx.port.direct_usb() && ctx.port.variax_claimed() {
        match ctx.port.release_variax() {
            Ok(lines) => lines
                .into_iter()
                .for_each(|l| ctx.rec.show(format!("Rig: {l}"))),
            Err(error) => problems.push(error),
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join(" "))
    }
}

/// Offer the crash restore for one unfinished run. Default: do not run.
pub fn offer_restore(
    port: &mut dyn super::port::RigPort,
    op: &mut dyn Operator,
    sink: &mut dyn ReportSink,
    state_path: &Path,
    state: &RigState,
) -> RunRecord {
    let mode = port.mode();
    let mut run = RunRecord::new(mode, utc_now());
    run.start_notice_accepted = true;
    op.progress(Progress::action(state.describe()));
    let step = catalogue::step(RIG_GUARD).expect("guard step");
    let mut rec = StepRecord::new(RIG_GUARD);
    rec.note(format!("Crash restore: {}", state.describe()));
    let outcome = if mode == Mode::Real
        && let Err(reason) = super::approvals::real_device_check(RIG_GUARD)
    {
        Outcome::NotAuthorized(reason)
    } else {
        match op.confirm(step, mode, None) {
            Decision::Run => {
                let mut data = RunData::default();
                rec.started = Some(utc_now());
                let result = {
                    let mut ctx = Ctx {
                        mode,
                        port,
                        op,
                        data: &mut data,
                        rec: &mut rec,
                    };
                    let result = restore(&mut ctx, state);
                    if ctx.port.direct_usb() {
                        super::port::finish_direct_usb(ctx.port, ctx.rec, result)
                    } else {
                        result
                    }
                };
                match result {
                    Ok(()) => {
                        let mut done = state.clone();
                        done.finished = true;
                        let _ = write_state(state_path, &done);
                        Outcome::Passed
                    }
                    Err(error) => Outcome::Failed(error),
                }
            }
            Decision::Skip | Decision::Stop => Outcome::SkippedByPerson,
        }
    };
    rec.outcome = outcome;
    run.rig_state = rec
        .evidence
        .iter()
        .filter_map(|l| l.strip_prefix("Rig: "))
        .collect::<Vec<_>>()
        .join(" ");
    *run.step_mut(RIG_GUARD) = rec.clone();
    run.finished = true;
    let _ = sink.update(&run);
    op.finished(&rec);
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::port::PracticePort;
    use crate::rig_check::practice::{Faults, SimRig};
    use crate::rig_check::runner::{NoReport, Runner};
    use crate::rig_check::testing::Script;

    #[test]
    fn a_late_unplug_and_an_already_unplugged_cable_both_allow_replug() {
        use crate::rig_check::run_on;
        for already_out in [false, true] {
            let rig = SimRig::shared(Faults {
                guided_action_delay: Some(Duration::from_secs(48)),
                ..Faults::default()
            });
            let mut port = PracticePort::new(rig.clone());
            let mut script = Script::run_all();
            if already_out {
                let rig = rig.clone();
                script.on_card = Some(Box::new(move |id| {
                    if id == StepId(6, 1) {
                        rig.borrow_mut().variax.connected = false;
                    }
                }));
            }
            let (run, _) = run_on(&mut port, &mut script, &mut NoReport, RunData::default());
            let step = run.step(StepId(6, 1));
            assert!(step.outcome.passed(), "{step:?}");
            assert!(
                step.evidence
                    .iter()
                    .any(|line| line.contains("already unplugged"))
                    == already_out
            );
            assert!(rig.borrow().variax.connected);
        }
    }

    #[test]
    fn trying_again_rechecks_the_cable_from_either_state() {
        use crate::rig_check::{run_on, runner::Retry};
        for already_out in [false, true] {
            let rig = SimRig::shared(Faults::default());
            let mut port = PracticePort::new(rig.clone());
            let mut script = Script::run_all();
            let on_card = rig.clone();
            script.on_card = Some(Box::new(move |id| {
                if id == StepId(6, 1) {
                    let mut model = on_card.borrow_mut();
                    model.variax.connected = !already_out;
                    model.faults.user_does_not_act = true;
                }
            }));
            let on_missed = rig.clone();
            script.on_missed = Some(Box::new(move |id| {
                assert_eq!(id, StepId(6, 1));
                on_missed.borrow_mut().faults.user_does_not_act = false;
            }));
            script.retry_choices.push_back(Retry::TryAgain);
            let (run, _) = run_on(&mut port, &mut script, &mut NoReport, RunData::default());
            let step = run.step(StepId(6, 1));
            assert!(step.outcome.passed(), "{step:?}");
            assert!(
                step.evidence
                    .iter()
                    .any(|line| line.contains("You chose Try again"))
            );
            assert!(rig.borrow().variax.connected);
        }
    }

    #[test]
    fn a_failed_vdi_attempt_can_be_repeated_and_the_pass_unblocks_saves() {
        use crate::rig_check::run_on;
        let id = StepId(6, 1);
        let rig = SimRig::shared(Faults::default());
        let mut port = PracticePort::new(rig.clone());
        let mut script = Script::run_all();
        script.repeat_once = Some(id);
        let attempt = std::rc::Rc::new(std::cell::Cell::new(0));
        let card_attempt = attempt.clone();
        let on_card = rig.clone();
        script.on_card = Some(Box::new(move |step| {
            if step == id {
                card_attempt.set(card_attempt.get() + 1);
                on_card.borrow_mut().variax.firmware = *b"0310";
            }
        }));
        let on_prepare = rig.clone();
        script.on_prepare = Some(Box::new(move |step| {
            if step == id && attempt.get() == 1 && !on_prepare.borrow().variax.connected {
                on_prepare.borrow_mut().variax.firmware = *b"0309";
            }
        }));
        let (run, _) = run_on(&mut port, &mut script, &mut NoReport, RunData::default());
        assert_eq!(run.prior_attempts.len(), 1);
        assert_eq!(run.prior_attempts[0].id, id);
        assert_eq!(run.prior_attempts[0].attempt, Some(1));
        assert!(matches!(run.prior_attempts[0].outcome, Outcome::Failed(_)));
        assert!(run.step(id).outcome.passed());
        assert_eq!(run.step(id).attempt, Some(2));
        assert!(run.step(StepId(5, 1)).outcome.passed());
        assert!(
            crate::rig_check::report::render_plain(&run, "practice").contains("EARLIER ATTEMPTS")
        );
    }

    #[test]
    fn the_vdi_replug_is_noticed_and_the_identity_read_again() {
        let mut person = Script::run_all();
        let (run, _, rig) = crate::rig_check::run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(StepId(6, 1));
        assert!(step.outcome.passed(), "{step:?}");
        assert!(
            step.evidence
                .iter()
                .any(|l| l.contains("the same guitar as in 3.1"))
        );
        assert!(rig.borrow().variax.connected);
    }

    #[test]
    fn state_files_round_trip() {
        let state = RigState {
            report: "rig-check-x.txt".into(),
            started: "2026-11-25 01:00:00 UTC".into(),
            podxt_channel: Some(17),
            podxt_tone_sha256: Some("aa".into()),
            variax_preset: Some((2, 0)),
            variax_match_sha256: Some("bb".into()),
            finished: false,
        };
        assert_eq!(RigState::parse(&state.text()), Some(state.clone()));
        assert!(state.describe().contains("5B") && state.describe().contains("Spank 1"));
    }

    #[test]
    fn crash_restore_refuses_unverified_podxt_firmware_before_program_change() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("rig-check-1.state");
        let state = RigState {
            podxt_channel: Some(1),
            ..RigState::default()
        };
        for (firmware, found) in [([0, 2, 0, 7], "2.07"), ([0, 3, 0, 2], "3.02")] {
            let rig = SimRig::shared(Faults::default());
            rig.borrow_mut().podxt.firmware = firmware;
            let mut port = PracticePort::new(rig.clone());
            let run = offer_restore(
                &mut port,
                &mut Script::run_all(),
                &mut NoReport,
                &state_path,
                &state,
            );
            assert!(matches!(
                &run.step(RIG_GUARD).outcome,
                Outcome::Failed(error)
                    if error.contains(found) && error.contains("PODxt Live firmware 3.01")
            ));
            let sim = rig.borrow();
            assert_eq!(sim.written.len(), 1);
            assert_eq!(sim.written[0].1, line6_devices::identity::REQUEST);
            assert!(!state_path.exists());
        }
    }

    #[test]
    fn an_interrupted_run_is_restored_on_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("rig-check-1.state");
        // First launch: stops (as a crash would) right after the reads and
        // the guided selector turn, before the rig guard.
        let rig = SimRig::shared(Faults::default());
        {
            let mut port = PracticePort::new(rig.clone());
            let mut person = Script::run_all();
            person.overrides.push((StepId(7, 2), Decision::Stop));
            person.overrides.push((StepId(6, 2), Decision::Skip));
            let mut sink = NoReport;
            let mut runner = Runner::new(&mut port, &mut person, &mut sink, RunData::default());
            runner.state_file = Some(state_path.clone());
            runner.run();
        }
        // Simulate the crash: the guitar was left on another preset and channel.
        {
            let mut sim = rig.borrow_mut();
            sim.user_acts(
                UserAction::TurnSelector(Preset::new(5, 3).unwrap()),
                Duration::ZERO,
            );
            sim.user_acts(UserAction::PodxtFootswitch(3), Duration::ZERO);
            sim.clock += Duration::from_secs(1);
        }
        let mut state = RigState::parse(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        state.finished = false; // the crash happened before the end
        write_state(&state_path, &state).unwrap();
        let found = pending(dir.path());
        assert_eq!(found.len(), 1);
        let mut port = PracticePort::new(rig.clone());
        let mut person = Script::run_all();
        let run = offer_restore(
            &mut port,
            &mut person,
            &mut NoReport,
            &found[0].0,
            &found[0].1,
        );
        assert!(
            run.step(RIG_GUARD).outcome.passed(),
            "{:?}",
            run.step(RIG_GUARD)
        );
        let fresh = SimRig::new(Faults::default());
        let sim = rig.borrow();
        assert_eq!(sim.variax.active, fresh.variax.active);
        assert_eq!(sim.podxt.channel, fresh.podxt.channel);
        assert!(pending(dir.path()).is_empty(), "marked finished");
        // Default No: nothing is sent.
        drop(sim);
        let mut person = Script::run_all();
        person.default = Decision::Skip;
        let before = rig.borrow().written.len();
        let mut port = PracticePort::new(rig.clone());
        offer_restore(
            &mut port,
            &mut person,
            &mut NoReport,
            &found[0].0,
            &found[0].1,
        );
        assert_eq!(rig.borrow().written.len(), before);
    }
}

//! Recovery (#261): step 6.1 (VDI cable unplug and replug noticed) and the
//! crash restore offered on the next launch.
//!
//! While a run is going, a small state file next to the report records the
//! starting PODxt channel and Variax preset with hashes of the starting tone
//! and program. A finished run marks it finished. When Rig Check starts and
//! finds an unfinished state file, it offers (default No) to switch the rig
//! back to that channel and preset and to check the hashes. That restore is a
//! device action: in real mode it needs the 6.2 owner approval like the rig
//! guard. If continuation revalidation left USB cleanup unverified, startup
//! requires manual verification before any automatic recovery.

use super::catalogue::{self, RIG_GUARD};
use super::practice::{Program, preset_label};
use super::procedures::{
    CHECKSUM_WORD, firmware_checksum, neutral_program, variax_read_preset, with_variax_progress,
};
use super::record::{Mode, Outcome, RunRecord, StepRecord, sha256_hex, utc_now};
use super::resume::{ResumeCleanup, VERIFY_USB_CLEANUP};
use super::runner::{Ctx, Decision, Operator, Progress, ReportSink};
use super::steps::RunData;
use line6_devices::family::podxt::{self, BankKind, Location};
use line6_devices::family::variax::Preset;
use line6_devices::gate::Operation;
use std::path::{Path, PathBuf};

mod vdi;

pub(super) use vdi::vdi_replug;

// ------------------------------------------------------------------ crash restore

const VARIAX_HASH_SCHEME: &str = "tone-position-v2:";

/// Hash a checksum-verified program, excluding only the live tone position
/// and the verified checksum. The prefix distinguishes older state files
/// whose hash also discarded neighboring settings.
pub fn match_hash(words: &Program) -> Option<String> {
    let neutral = neutral_program(words).ok()?;
    let hash = sha256_hex(&super::procedures::program_bytes(&neutral));
    Some(format!("{VARIAX_HASH_SCHEME}{hash}"))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RigState {
    pub report: String,
    pub report_sha256: Option<String>,
    pub started: String,
    pub podxt_channel: Option<u16>,
    pub podxt_tone_sha256: Option<String>,
    /// Bank and slot.
    pub variax_preset: Option<(u8, u8)>,
    pub variax_match_sha256: Option<String>,
    pub finished: bool,
    /// Revalidation started without a verified release or a committed continuation.
    pub resume_cleanup_unverified: bool,
    /// A separate restore completed. The source report still records its original interruption.
    pub recovery_completed: bool,
}

impl RigState {
    pub fn from_run(run: &RunRecord, data: &RunData, report: &str) -> Self {
        Self {
            report: report.into(),
            report_sha256: None,
            started: run.started.clone(),
            podxt_channel: data.start_channel.map(|c| c.index()),
            podxt_tone_sha256: data.start_tone.as_ref().map(|t| sha256_hex(&t.raw)),
            variax_preset: data.start_preset.map(|p| (p.bank(), p.slot())),
            variax_match_sha256: data.start_program.as_ref().and_then(match_hash),
            finished: run.finished,
            resume_cleanup_unverified: matches!(
                run.resume_cleanup,
                Some(ResumeCleanup::Pending | ResumeCleanup::Failed(_))
            ),
            recovery_completed: false,
        }
    }

    pub fn text(&self) -> String {
        let opt = |v: &Option<String>| v.clone().unwrap_or_default();
        format!(
            "rig-check-state 1\nreport={}\nstarted={}\npodxt_channel={}\npodxt_tone_sha256={}\nvariax_preset={}\nvariax_match_sha256={}\nfinished={}\nreport_sha256={}\nrecovery_completed={}\nresume_cleanup_unverified={}\n",
            self.report,
            self.started,
            self.podxt_channel.map_or(String::new(), |c| c.to_string()),
            opt(&self.podxt_tone_sha256),
            self.variax_preset
                .map_or(String::new(), |(b, s)| format!("{b}.{s}")),
            opt(&self.variax_match_sha256),
            self.finished,
            opt(&self.report_sha256),
            self.recovery_completed,
            self.resume_cleanup_unverified
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
                "report_sha256" => state.report_sha256 = some(value),
                "recovery_completed" => state.recovery_completed = value == "true",
                "resume_cleanup_unverified" => state.resume_cleanup_unverified = value == "true",
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

pub(super) struct CheckpointLock(std::fs::File);

impl Drop for CheckpointLock {
    fn drop(&mut self) {
        // Release the advisory lock even if a child briefly inherited the descriptor.
        let _ = self.0.unlock();
    }
}

/// Lock a checkpoint for its writer's lifetime. The stable sidecar stays in
/// place while the report and state files are replaced. The OS releases the
/// lock if the app exits or crashes.
pub(super) fn lock_checkpoint(path: &Path) -> Result<CheckpointLock, String> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("run-lock"))
        .map_err(|error| format!("Cannot open the checkpoint lock: {error}"))?;
    file.try_lock().map_err(|error| {
        format!("Cannot lock the checkpoint. Another Rig Check may be using it: {error}")
    })?;
    Ok(CheckpointLock(file))
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

/// Runs with pending recovery or unverified USB cleanup, newest first.
pub fn pending(dir: &Path) -> Vec<(PathBuf, RigState)> {
    let mut found: Vec<(PathBuf, RigState)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "state"))
                .filter_map(|p| {
                    let state = RigState::parse(&std::fs::read_to_string(&p).ok()?)?;
                    (state.resume_cleanup_unverified
                        || (!state.finished
                            && !state.recovery_completed
                            && (state.channel().is_some() || state.preset().is_some())))
                    .then_some((p, state))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by(|a, b| b.1.started.cmp(&a.1.started));
    found
}

fn confirm_manual_recovery(
    op: &mut dyn Operator,
    report: &Path,
    reason: &str,
) -> Result<(), String> {
    let notice = format!(
        "A previous Rig Check needs attention. {reason} Review {} and verify the channel, tone and guitar preset by hand. If any store was interrupted, verify the affected slot before proceeding. Continue only after that manual verification, or choose Quit.",
        report.display()
    );
    if op.accept_start(&notice) {
        Ok(())
    } else {
        Err(format!(
            "The pending rig recovery was not confirmed. {reason}"
        ))
    }
}

pub(super) fn before_new_run(
    port: &mut dyn super::port::RigPort,
    op: &mut dyn Operator,
    dir: &Path,
    legacy_dir: Option<&Path>,
) -> Result<(), String> {
    // The old Mac location stays in place. Require manual verification there
    // without running a restore that would rewrite its original checkpoint.
    if let Some(legacy_dir) = legacy_dir
        && legacy_dir.canonicalize().ok() != dir.canonicalize().ok()
    {
        confirm_pending(op, legacy_dir, port.mode(), None)?;
    }
    let (cleanup, interrupted): (Vec<_>, Vec<_>) = pending(dir)
        .into_iter()
        .filter(|(path, _)| {
            super::report::saved_mode(&path.with_extension("txt"))
                .is_none_or(|mode| mode == port.mode())
        })
        .partition(|(_, state)| state.resume_cleanup_unverified);
    for (path, state) in cleanup {
        confirm_pending_cleanup(op, &path, &state)?;
    }
    let Some((path, state)) = interrupted.into_iter().next() else {
        return Ok(());
    };
    let report = path.with_extension("txt");
    let plan = match super::resume::open(&report).and_then(|plan| {
        plan.restore_ready(&state)?;
        Ok(plan)
    }) {
        Ok(plan) => plan,
        Err(reason) => return confirm_manual_recovery(op, &report, &reason),
    };
    let _lock = plan.lock_source()?;
    verify_pending_state(&path, &state)?;
    let mut sink =
        super::report::ReportFile::create(dir, super::redact::Redactor::from_environment())?;
    let run = offer_restore_checked(port, op, &mut sink, &path, &state, Some(plan));
    let outcome = &run.step(RIG_GUARD).outcome;
    if outcome.passed() {
        return Ok(());
    }
    if matches!(outcome, Outcome::SkippedByPerson) {
        return confirm_manual_recovery(op, &report, "You skipped automatic recovery.");
    }
    Err(outcome
        .reason()
        .unwrap_or("Start new ended before the pending rig recovery was confirmed.")
        .into())
}

/// Check the configured report locations for interrupted runs. The selected checkpoint
/// is already locked and validated by the resume plan.
pub(super) fn before_continuation(
    op: &mut dyn Operator,
    mode: Mode,
    plan: &super::resume::Plan,
    output_dir: &Path,
    legacy_dir: Option<&Path>,
) -> Result<(), String> {
    let output = output_dir.canonicalize().ok();
    let legacy = legacy_dir.and_then(|dir| dir.canonicalize().ok());
    for dir in [
        output.as_deref(),
        legacy
            .as_deref()
            .filter(|dir| Some(*dir) != output.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        confirm_pending(op, dir, mode, Some(plan))?;
    }
    Ok(())
}

fn confirm_pending(
    op: &mut dyn Operator,
    dir: &Path,
    mode: Mode,
    selected: Option<&super::resume::Plan>,
) -> Result<(), String> {
    for (path, state) in pending(dir) {
        let report = path.with_extension("txt");
        if super::report::saved_mode(&report).is_some_and(|saved| saved != mode) {
            continue;
        }
        if state.resume_cleanup_unverified {
            confirm_pending_cleanup(op, &path, &state)?;
            continue;
        }
        if let Some(plan) = selected
            && plan.is_selected_checkpoint(&path)?
        {
            continue;
        }
        let _lock = lock_checkpoint(&path)?;
        verify_pending_state(&path, &state)?;
        confirm_manual_recovery(op, &report, "Another Rig Check was interrupted.")?;
    }
    Ok(())
}

fn confirm_pending_cleanup(
    op: &mut dyn Operator,
    path: &Path,
    state: &RigState,
) -> Result<(), String> {
    let _lock = lock_checkpoint(path)?;
    verify_pending_state(path, state)?;
    confirm_manual_recovery(op, &path.with_extension("txt"), VERIFY_USB_CLEANUP)
}

fn verify_pending_state(path: &Path, state: &RigState) -> Result<(), String> {
    let current = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| RigState::parse(&text));
    if current.as_ref() != Some(state) {
        return Err(
            "The pending recovery checkpoint changed. Open the latest report before trying again."
                .into(),
        );
    }
    Ok(())
}

const RESTORE_OPS: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtChannelSwitch,
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
];

pub(super) fn usable_variax_hash(state: &RigState) -> Option<&str> {
    state.variax_match_sha256.as_deref().filter(|hash| {
        hash.strip_prefix(VARIAX_HASH_SCHEME).is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
        })
    })
}

fn restore(ctx: &mut Ctx<'_>, state: &RigState, exact_tone: bool) -> Result<(), String> {
    let variax = state
        .preset()
        .map(|preset| {
            let hash = usable_variax_hash(state);
            hash.map(|hash| (preset, hash))
                .ok_or("This state file has no usable Variax recovery hash. Keep the earlier report and verify the rig manually before continuing.")
        })
        .transpose()?;
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
        } else if exact_tone {
            problems.push(format!("The PODxt Live returned to {}, but the tone differs from the starting capture. Check its original settings by hand.", channel.label()));
        } else {
            ctx.rec.note(format!(
                "Rig: the PODxt Live is on channel {}; its tone hash {} differs from the start (pedal or knob values can differ).",
                channel.label(),
                &hash[..16]
            ));
        }
    }
    if let Some((preset, expected_hash)) = variax {
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
        match match_hash(&read.words) {
            Some(actual) if actual == expected_hash => ctx.rec.show(format!(
                "Rig: the Variax is back on {} and its program matches the start.",
                preset_label(preset)
            )),
            Some(_) => problems.push(format!(
                "The Variax is on {}, but its program differs from the start.",
                preset_label(preset)
            )),
            None => {
                let reason = "The Variax recovery read failed its firmware checksum check.";
                ctx.rec.difference(
                    super::differences::Severity::Stop,
                    reason,
                    vec![super::differences::checksum_line(
                        firmware_checksum(&read.words),
                        read.words[CHECKSUM_WORD],
                    )],
                );
                problems.push(reason.into());
            }
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
    offer_restore_checked(port, op, sink, state_path, state, None)
}

fn offer_restore_checked(
    port: &mut dyn super::port::RigPort,
    op: &mut dyn Operator,
    sink: &mut dyn ReportSink,
    state_path: &Path,
    state: &RigState,
    plan: Option<super::resume::Plan>,
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
        match op.confirm(step, mode, Some(state.describe())) {
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
                    let result = match plan.as_ref() {
                        Some(plan) => plan
                            .verify_restore_identity(&mut ctx, state)
                            .and_then(|()| restore(&mut ctx, state, true)),
                        None => restore(&mut ctx, state, false),
                    };
                    if ctx.port.direct_usb() {
                        super::port::finish_direct_usb(ctx.port, ctx.rec, result)
                    } else if ctx.port.variax_claimed() {
                        match ctx.port.release_variax() {
                            Ok(lines) => {
                                for line in lines {
                                    ctx.rec.show(format!("Rig: {line}"));
                                }
                                result
                            }
                            Err(error) => Err(result
                                .err()
                                .map_or(error.clone(), |prior| format!("{prior} {error}"))),
                        }
                    } else {
                        result
                    }
                };
                match result {
                    Ok(()) => {
                        let mut done = state.clone();
                        done.recovery_completed = true;
                        match write_state(state_path, &done) {
                            Ok(()) => Outcome::Passed,
                            Err(error) => Outcome::Failed(format!(
                                "Could not record completed crash recovery: {error}"
                            )),
                        }
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
    use crate::rig_check::practice::{Faults, SimRig, UserAction};
    use crate::rig_check::runner::{NoReport, Runner};
    use crate::rig_check::testing::Script;
    use std::time::Duration;

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
        assert!(matches!(
            &run.prior_attempts[0].outcome,
            Outcome::Failed(error) if error.contains("different identity than in step 3.1")
        ));
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
        assert!(step.evidence.iter().any(|l| l.contains("matches step 3.1")));
        assert!(rig.borrow().variax.connected);
    }

    #[test]
    fn state_files_round_trip() {
        let state = RigState {
            report: "rig-check-x.txt".into(),
            report_sha256: None,
            started: "2026-11-25 01:00:00 UTC".into(),
            podxt_channel: Some(17),
            podxt_tone_sha256: Some("aa".into()),
            variax_preset: Some((2, 0)),
            variax_match_sha256: Some("bb".into()),
            finished: false,
            resume_cleanup_unverified: false,
            recovery_completed: false,
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

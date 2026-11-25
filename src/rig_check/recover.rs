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
use super::guided::{WINDOW, watch_variax};
use super::practice::{Program, UserAction, preset_label};
use super::procedures::{TONE_POSITION_WORDS, variax_read_preset, with_variax};
use super::record::{Mode, Outcome, RunRecord, StepRecord, sha256_hex, utc_now};
use super::runner::{Ctx, Decision, Operator, ReportSink, Retry, SKIPPED};
use super::steps::RunData;
use line6_devices::family::podxt::{self, BankKind, Location, PodxtRequest};
use line6_devices::family::variax::{Preset, procedures as shared_variax};
use line6_devices::gate::Operation;
use line6_devices::midi::Event;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ------------------------------------------------------------------ 6.1

const VDI_OPS: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];

fn connection_notice(event: &Event) -> Option<bool> {
    match event {
        Event::Sysex(frame) => match frame.as_slice() {
            [0xf0, 0, 1, 0x0c, 0x03, 0x11, _, _, _, state, 0xf7] => Some(*state != 0),
            [0xf0, 0, 1, 0x0c, 0x7f, 0x03, _, _, _, _, _, 0xf7] => Some(false),
            _ => None,
        },
        Event::Short(_) => None,
    }
}

fn vdi_window(
    ctx: &mut Ctx<'_>,
    connected: bool,
    see: &str,
    action: UserAction,
) -> Result<Duration, String> {
    let what = if connected {
        "the guitar coming back"
    } else {
        "the guitar going away"
    };
    loop {
        if ctx.stopping() {
            return Err("Stopped by you.".into());
        }
        let hear = if connected {
            "Your guitar again."
        } else {
            "Silence from the guitar."
        };
        ctx.op.instruct(ctx.rec.id, see, hear);
        ctx.port
            .user_acts(action.clone(), super::guided::PRACTICE_DELAY);
        let (seen, at) = with_variax(ctx, VDI_OPS, |session, stopping| {
            let seen = watch_variax(session, WINDOW, stopping, |event| {
                connection_notice(event) == Some(connected)
            })?;
            Ok((seen, session.link.elapsed()))
        })?;
        if seen {
            return Ok(at);
        }
        ctx.rec
            .note(format!("Not seen within {} s: {what}.", WINDOW.as_secs()));
        match ctx.op.not_seen(ctx.rec.id, what) {
            Retry::TryAgain => continue,
            Retry::Skip => return Err(format!("{SKIPPED}Didn't see {what}; you chose Skip.")),
        }
    }
}

/// 6.1: the person unplugs the VDI cable and plugs it back; the identity is
/// read again afterwards.
pub fn vdi_replug(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let gone = vdi_window(
        ctx,
        false,
        "Unplug the VDI cable from the guitar now.",
        UserAction::UnplugVdi,
    )?;
    ctx.rec.note(format!(
        "The PODxt Live reported the guitar disconnected (link time {:.3} s).",
        gone.as_secs_f64()
    ));
    let back = vdi_window(
        ctx,
        true,
        "Plug the VDI cable back into the guitar.",
        UserAction::ReplugVdi,
    )?;
    ctx.rec.note(format!(
        "The PODxt Live reported the guitar connected again (link time {:.3} s).",
        back.as_secs_f64()
    ));
    let identified = with_variax(ctx, VDI_OPS, |session, _| {
        super::procedures::drain(session)?;
        shared_variax::identify_reported(session)
    })?;
    let same = ctx
        .data
        .variax_identity
        .as_ref()
        .is_some_and(|before| *before == identified.identity);
    ctx.rec.note(format!(
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
            session.send(PodxtRequest::Program(channel))?;
            session.link.pause(Duration::from_millis(400));
            podxt::read_current(session).map(|current| current.tone)
        })?;
        let hash = sha256_hex(&tone.raw);
        if Some(&hash) == state.podxt_tone_sha256.as_ref() {
            ctx.rec.note(format!(
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
        let read = with_variax(ctx, RESTORE_OPS, |session, _| {
            super::procedures::drain(session)?;
            variax_read_preset(session, preset, &|| false)
        })?;
        if Some(match_hash(&read.words)) == state.variax_match_sha256 {
            ctx.rec.note(format!(
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
    if ctx.port.variax_claimed() {
        match ctx.port.release_variax() {
            Ok(lines) => lines
                .into_iter()
                .for_each(|l| ctx.rec.note(format!("Rig: {l}"))),
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
    op.progress(&state.describe());
    let step = catalogue::step(RIG_GUARD).expect("guard step");
    let mut rec = StepRecord::new(RIG_GUARD);
    rec.note(format!("Crash restore: {}", state.describe()));
    let outcome = if mode == Mode::Real
        && let Err(reason) = super::approvals::real_device_check(RIG_GUARD)
    {
        Outcome::NotAuthorized(reason)
    } else {
        match op.confirm(step, mode) {
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
                    restore(&mut ctx, state)
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
    op.finished(&rec);
    let _ = sink.update(&run);
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

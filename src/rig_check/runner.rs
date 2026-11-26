//! The step runner: one explained, confirmed step at a time.
//!
//! * Every step shows its card and waits. The default answer is not to run.
//! * Nothing moves into a new step by itself.
//! * A step runs only when every step it needs has passed.
//! * Real mode refuses each device step whose owner approval is off.
//! * Stop ends the run; the usual rig guard (6.2) still asks. Evidence mode
//!   only releases the USB interface.
//! * The report sink is called after every step so a crash leaves evidence.

use super::approvals;
use super::catalogue::{
    self, Kind, MAC_START_NOTICE, PODXT_COMPATIBILITY, RIG_GUARD, RUN_ORDER, Risk, START_NOTICE,
    SaveNotice, Step, StepId, VARIAX_EVIDENCE,
};
use super::port::RigPort;
use super::record::{Mode, Outcome, RunRecord, StepRecord, utc_now};
use super::steps::{self, RunData};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Run,
    Skip,
    Stop,
}

/// The default when the person has not chosen.
pub const DEFAULT_DECISION: Decision = Decision::Skip;

/// A step returns an error starting with this when the person declined
/// inside the step (for example the typed save confirmation). Nothing ran.
pub const SKIPPED: &str = "SKIPPED: ";
pub const STOPPED: &str = "STOPPED: ";
pub const NOT_APPLICABLE: &str = "NOT APPLICABLE: ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retry {
    TryAgain,
    Skip,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    NotSure,
}

impl Answer {
    pub fn label(self) -> &'static str {
        match self {
            Answer::Yes => "Yes",
            Answer::No => "No",
            Answer::NotSure => "Not sure",
        }
    }
}

/// A listening question: always an A/B against a sound just heard.
#[derive(Clone, Debug)]
pub struct Question {
    pub step: StepId,
    pub text: String,
    /// What the byte checks proved, shown next to the answer.
    pub verified_by_bytes: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResultDecision {
    Continue,
    Stop,
    Different(String),
    Repeat,
}

pub const CONTROLS_NOTICE: &str = "Leave the guitar selector, the volume and tone knobs, and the PODxt footswitches and knobs alone until this finishes. Playing is fine.";

#[derive(Clone, Debug)]
pub struct Progress {
    pub action: String,
    pub count: Option<(usize, usize, &'static str)>,
    pub wait: Option<std::time::Duration>,
    pub guided: bool,
}

impl Progress {
    pub fn action(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            count: None,
            wait: None,
            guided: false,
        }
    }

    pub fn counted(
        action: impl Into<String>,
        done: usize,
        total: usize,
        units: &'static str,
    ) -> Self {
        Self {
            count: Some((done, total, units)),
            ..Self::action(action)
        }
    }

    pub fn watching(action: impl Into<String>) -> Self {
        Self {
            guided: true,
            ..Self::action(action)
        }
    }

    pub fn waiting(action: impl Into<String>, limit: std::time::Duration, guided: bool) -> Self {
        Self {
            wait: Some(limit),
            guided,
            ..Self::action(action)
        }
    }
}

/// The person at the rig, through the GUI, the terminal or a test script.
pub trait Operator {
    /// The start notice. `false` (the default) quits before any step.
    fn accept_start(&mut self, notice: &str) -> bool;
    /// Show the step card and wait for Run, Skip or Stop.
    fn confirm(&mut self, step: &Step, mode: Mode, note: Option<String>) -> Decision;
    /// The second notice before a save. Returns the typed confirmation, or
    /// `None` to skip.
    fn confirm_save(&mut self, notice: &SaveNotice) -> Option<String>;
    /// Show large HEAR / SEE instructions for a guided action. Returns at once.
    fn instruct(&mut self, step: StepId, see: &str, hear: &str);
    /// A guided action was not seen in time.
    fn not_seen(&mut self, step: StepId, what: &str) -> Retry;
    fn variax_connected(&mut self) -> bool;
    /// Confirm the specific movement before opening the listener.
    fn prepare_action(&mut self, step: StepId, see: &str, hear: &str) -> Decision;
    /// Start watching. The returned check lets the person end this action.
    fn watch_action(
        &mut self,
        step: StepId,
        description: &str,
        clock: std::sync::Arc<super::guided::ActionClock>,
    ) -> Box<dyn Fn() -> bool>;
    /// A listening question with Yes / No / Not sure and an optional note.
    fn ask(&mut self, question: &Question) -> (Answer, String);
    /// The latest action, with optional completed work or time limit.
    fn progress(&mut self, progress: Progress);
    /// Publish any outcome. If the step ran, wait for the person to read it.
    fn finished(&mut self, record: &StepRecord) -> ResultDecision;
    /// The person asked to stop the running step.
    fn stop_requested(&self) -> bool;
}

/// Called after every step with the whole run so far.
pub trait ReportSink {
    fn update(&mut self, run: &RunRecord) -> Result<(), String>;
}

/// Discards updates (tests that do not check the file).
pub struct NoReport;

impl ReportSink for NoReport {
    fn update(&mut self, _: &RunRecord) -> Result<(), String> {
        Ok(())
    }
}

pub struct Ctx<'a> {
    pub mode: Mode,
    pub port: &'a mut dyn RigPort,
    pub op: &'a mut dyn Operator,
    pub data: &'a mut RunData,
    pub rec: &'a mut StepRecord,
}

impl Ctx<'_> {
    pub fn step(&self) -> StepId {
        self.rec.id
    }

    pub fn stopping(&self) -> bool {
        self.op.stop_requested()
    }
}

pub struct Runner<'a> {
    pub port: &'a mut dyn RigPort,
    pub op: &'a mut dyn Operator,
    pub sink: &'a mut dyn ReportSink,
    pub data: RunData,
    pub run: RunRecord,
    /// Where to keep the crash-restore state (#261), if anywhere.
    pub state_file: Option<std::path::PathBuf>,
}

impl<'a> Runner<'a> {
    pub fn new(
        port: &'a mut dyn RigPort,
        op: &'a mut dyn Operator,
        sink: &'a mut dyn ReportSink,
        data: RunData,
    ) -> Self {
        let mode = port.mode();
        Self {
            port,
            op,
            sink,
            data,
            run: RunRecord::new(mode, utc_now()),
            state_file: None,
        }
    }

    fn write_report(&mut self) {
        if let Some(path) = &self.state_file {
            let report = path.with_extension("txt");
            let report = report
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned());
            let state = super::recover::RigState::from_run(&self.run, &self.data, &report);
            if let Err(error) = super::recover::write_state(path, &state) {
                self.op.progress(Progress::action(format!(
                    "Could not write the restore state: {error}"
                )));
            }
        }
        if let Err(error) = self.sink.update(&self.run) {
            self.op.progress(Progress::action(format!(
                "Could not write the report: {error}"
            )));
        }
    }

    /// Run the catalogue. Returns the record of the whole run.
    pub fn run(mut self) -> (RunRecord, RunData) {
        self.write_report();
        let start_notice = if cfg!(target_os = "macos") && self.run.mode == Mode::Real {
            MAC_START_NOTICE
        } else {
            START_NOTICE
        };
        if !self.op.accept_start(start_notice) {
            self.run.rig_state =
                "The start notice was not accepted. Nothing ran and no device was touched.".into();
            self.run.finished = true;
            self.write_report();
            return (self.run, self.data);
        }
        self.run.start_notice_accepted = true;
        self.write_report();
        let mut stopped = false;
        for &id in RUN_ORDER.iter().filter(|id| **id != RIG_GUARD) {
            if stopped {
                break;
            }
            if id == VARIAX_EVIDENCE && self.variax_evidence_applicable() {
                stopped = self.run_variax_evidence();
                continue;
            }
            stopped = self.step(id);
            if stopped {
                self.write_report();
            }
        }
        if self.unverified_podxt_relay() || self.data.store_state_unknown {
            self.release_without_midi();
        } else {
            // The usual guard is offered last, also after Stop.
            self.step(RIG_GUARD);
        }
        self.run.finished = true;
        self.write_report();
        (self.run, self.data)
    }

    fn unverified_podxt_relay(&self) -> bool {
        use line6_devices::family::podxt::{IDENTITY_FAMILY, PODXT_LIVE_MEMBER};
        self.run.step(StepId(2, 1)).outcome.passed()
            && self.data.podxt_identity.as_ref().is_some_and(|identity| {
                identity.is_line6()
                    && identity.family == IDENTITY_FAMILY
                    && identity.member == PODXT_LIVE_MEMBER
                    && self.data.podxt_verified == Some(false)
            })
    }

    fn variax_evidence_applicable(&self) -> bool {
        self.unverified_podxt_relay()
            || (self.data.podxt_verified == Some(true)
                && self.run.step(StepId(3, 1)).outcome.passed()
                && super::variax_evidence::applicable(self.data.variax_identity.as_ref()))
    }

    fn run_variax_evidence(&mut self) -> bool {
        let notice = if self.unverified_podxt_relay() {
            super::variax_evidence::RELAY_NOTICE
        } else {
            super::variax_evidence::NOTICE
        };
        if self.op.accept_start(notice) {
            self.step(VARIAX_EVIDENCE)
        } else {
            self.finish(VARIAX_EVIDENCE, Outcome::SkippedByPerson, None, None);
            false
        }
    }

    fn release_without_midi(&mut self) {
        let release = if self.port.variax_claimed() {
            self.port.release_variax().map(|lines| lines.join(" "))
        } else {
            Ok("The guitar interface was already released.".into())
        };
        self.run.rig_state =
            release.unwrap_or_else(|error| format!("Guitar interface release failed: {error}"));
        self.finish(
            RIG_GUARD,
            Outcome::NotAvailable(
                if self.data.store_state_unknown {
                    "The store outcome is unknown. USB release only."
                } else {
                    "Evidence mode ends with USB release only."
                }
                .into(),
            ),
            None,
            None,
        );
    }

    /// Run or skip one step. Returns `true` when the person stopped the run.
    fn step(&mut self, id: StepId) -> bool {
        let step = catalogue::step(id).expect("run order uses catalogue IDs");
        if let Some(outcome) = self.precheck(step) {
            if cfg!(target_os = "macos") && id == RIG_GUARD && self.run.mode == Mode::Real {
                self.run.rig_state = crate::MAC_CONNECTION_UNAVAILABLE.into();
            }
            self.finish(id, outcome, None, None);
            return false;
        }
        let mut attempt_number = 1;
        loop {
            let note = if matches!(id, StepId(4, 4) | StepId(5, 3)) {
                self.data
                    .exercise_program()
                    .and_then(|program| super::variax_live::trim_note(&program).ok())
            } else {
                None
            };
            match self.op.confirm(step, self.run.mode, note) {
                Decision::Skip => {
                    self.finish(id, Outcome::SkippedByPerson, None, None);
                    return false;
                }
                Decision::Stop => {
                    self.run.stopped_at = Some(id);
                    self.finish(
                        id,
                        Outcome::Stopped("You chose Stop before this step.".into()),
                        None,
                        None,
                    );
                    return id != RIG_GUARD;
                }
                Decision::Run => {
                    self.run.current = Some(id);
                    self.write_report();
                    let started = utc_now();
                    let clock = Instant::now();
                    let mut rec = std::mem::replace(self.run.step_mut(id), StepRecord::new(id));
                    rec.attempt = Some(attempt_number);
                    let result = {
                        let mut ctx = Ctx {
                            mode: self.run.mode,
                            port: &mut *self.port,
                            op: &mut *self.op,
                            data: &mut self.data,
                            rec: &mut rec,
                        };
                        ctx.op.progress(Progress::action(step.title));
                        steps::execute(&mut ctx)
                    };
                    let stopped_now = self.op.stop_requested()
                        || result
                            .as_ref()
                            .is_err_and(|reason| reason.starts_with(STOPPED));
                    let outcome = match result {
                        Ok(()) => Outcome::Passed,
                        Err(reason) if reason.starts_with(NOT_APPLICABLE) => {
                            Outcome::SkippedNotApplicable(
                                reason.trim_start_matches(NOT_APPLICABLE).into(),
                            )
                        }
                        Err(reason) if reason.starts_with(SKIPPED) => {
                            rec.show(reason.trim_start_matches(SKIPPED));
                            Outcome::SkippedByPerson
                        }
                        Err(error) if stopped_now => {
                            Outcome::Stopped(error.trim_start_matches(STOPPED).into())
                        }
                        Err(error) => Outcome::Failed(error),
                    };
                    if id == RIG_GUARD {
                        self.run.rig_state = rec
                            .evidence
                            .iter()
                            .filter_map(|line| line.strip_prefix("Rig: "))
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                            .join(" ");
                    }
                    self.run.current = None;
                    super::hints::attach(&mut rec, &outcome);
                    *self.run.step_mut(id) = rec;
                    self.run.computer = self.data.computer.clone();
                    self.run.devices = self.data.devices.clone();
                    self.run.rig_description = self.data.rig_description();
                    let decision = self.finish(
                        id,
                        outcome,
                        Some(started),
                        Some(clock.elapsed().as_millis()),
                    );
                    if !stopped_now
                        && decision == ResultDecision::Repeat
                        && self.run.step(id).can_repeat()
                    {
                        let attempt = std::mem::replace(self.run.step_mut(id), StepRecord::new(id));
                        self.run.prior_attempts.push(attempt);
                        attempt_number += 1;
                        self.write_report();
                        continue;
                    }
                    self.data.read_only |= matches!(self.run.step(id).outcome, Outcome::Failed(_));
                    if (stopped_now || decision == ResultDecision::Stop) && id != RIG_GUARD {
                        self.run.stopped_at = Some(id);
                        return true;
                    }
                    return false;
                }
            }
        }
    }

    fn model_data_available(&self) -> bool {
        if let Some(rig) = self.port.simulation() {
            return !rig.borrow().faults.missing_model_data;
        }
        self.data
            .engine_folder
            .as_ref()
            .is_some_and(|dir| dir.join("models.tsv").is_file())
    }

    /// Why a step cannot be offered, without asking.
    fn precheck(&self, step: &Step) -> Option<Outcome> {
        if cfg!(target_os = "macos") && self.run.mode == Mode::Real && step.needs_device_approval()
        {
            return Some(Outcome::NotAvailable(
                crate::MAC_CONNECTION_UNAVAILABLE.into(),
            ));
        }
        if let Some(reason) = step.not_in_this_version {
            return Some(Outcome::NotAvailable(reason.into()));
        }
        if step.id == PODXT_COMPATIBILITY {
            if self.data.no_podxt {
                return Some(Outcome::SkippedNotApplicable("No PODxt Live found".into()));
            }
            if self.data.podxt_verified == Some(true) {
                return Some(Outcome::verified_firmware_skip("PODxt Live", "3.01"));
            }
        }
        if step.needs_device_approval() {
            if self.data.no_podxt {
                return Some(Outcome::SkippedNotApplicable("No PODxt Live found".into()));
            }
            if self.unverified_podxt_relay() && step.id != VARIAX_EVIDENCE {
                return Some(Outcome::SkippedNotApplicable(
                    "Not applicable: PODxt Live firmware is not the tested version".into(),
                ));
            }
            if catalogue::is_variax_step(step.id) {
                if self.data.no_variax {
                    return Some(Outcome::SkippedNotApplicable(
                        "Not applicable: no Variax connected".into(),
                    ));
                }
                if step.id != VARIAX_EVIDENCE
                    && self.data.variax_verified == Some(false)
                    && let Some(identity) = &self.data.variax_identity
                {
                    return Some(Outcome::SkippedNotApplicable(format!(
                        "Not applicable: Variax firmware {} isn't the tested version",
                        identity.version_text()
                    )));
                }
            }
        }
        if matches!(step.id, StepId(0, 3) | StepId(4, 3..=5) | StepId(5, 3))
            && !self.model_data_available()
        {
            return Some(Outcome::SkippedNotApplicable(
                super::variax_live::MODEL_DATA_NEEDED.into(),
            ));
        }
        if matches!(step.id, StepId(5, 1 | 3 | 4))
            && let Some(preset) = self.data.start_preset
            && let Err(reason) = super::variax_save::slot(preset)
        {
            return Some(Outcome::SkippedNotApplicable(
                reason.trim_start_matches(NOT_APPLICABLE).into(),
            ));
        }
        if step.id == StepId(5, 3)
            && matches!(
                self.run.step(StepId(4, 4)).outcome,
                Outcome::SkippedNotApplicable(_)
            )
        {
            return Some(Outcome::SkippedNotApplicable(
                "The temporary output trim change was unavailable in 4.4.".into(),
            ));
        }
        let earlier = || {
            RUN_ORDER
                .iter()
                .copied()
                .take_while(|&id| id != step.id)
                .filter(|&id| {
                    !matches!(
                        self.run.step(id).outcome,
                        Outcome::SkippedVerifiedFirmware(_) | Outcome::SkippedNotApplicable(_)
                    )
                })
        };
        if matches!(step.risk, Risk::Live | Risk::Save)
            && let Some(id) =
                earlier().find(|&id| matches!(self.run.step(id).outcome, Outcome::Failed(_)))
        {
            return Some(Outcome::SkippedNeeds(format!(
                "read-only from here: step {id} failed"
            )));
        }
        if step.risk == Risk::Save
            && let Some(id) = earlier().find(|&id| !self.run.step(id).outcome.passed())
        {
            return Some(Outcome::SkippedNeeds(format!(
                "read-only from here: step {id} did not pass"
            )));
        }
        let missing: Vec<String> = step
            .needs
            .iter()
            .filter(|need| !self.run.step(**need).outcome.passed())
            .map(|need| format!("{need} ({})", self.run.step(*need).outcome.label()))
            .collect();
        if !missing.is_empty() {
            return Some(Outcome::SkippedNeeds(format!(
                "Needs {}, which did not pass.",
                missing.join(", ")
            )));
        }
        if step.id == VARIAX_EVIDENCE {
            if self.data.podxt_verified == Some(true) && self.data.variax_verified == Some(true) {
                return Some(Outcome::verified_firmware_skip("Variax", "3.10"));
            }
            if !self.variax_evidence_applicable() {
                return Some(Outcome::NotAvailable(
                    "Available for unverified Variax 300/600 firmware or an unverified PODxt Live relay.".into(),
                ));
            }
        } else if catalogue::is_variax_step(step.id) && self.data.podxt_verified != Some(true) {
            return Some(Outcome::UnverifiedDevice(
                super::variax_evidence::RELAY_REASON.into(),
            ));
        }
        if self.run.mode == Mode::Real
            && let Err(reason) = approvals::real_device_check(step.id)
        {
            return Some(Outcome::NotAuthorized(reason));
        }
        if step.verified_devices_only
            && step.kind != Kind::Computer
            && let Some(reason) = self.data.unverified_reason(step.id)
        {
            return Some(Outcome::UnverifiedDevice(reason));
        }
        None
    }

    fn finish(
        &mut self,
        id: StepId,
        outcome: Outcome,
        started: Option<String>,
        duration: Option<u128>,
    ) -> ResultDecision {
        let rec = self.run.step_mut(id);
        rec.outcome = outcome;
        if started.is_some() {
            rec.started = started;
            rec.duration_ms = duration;
        }
        let rec = rec.clone();
        self.write_report();
        match self.op.finished(&rec) {
            ResultDecision::Different(value) => {
                let rec = self.run.step_mut(id);
                if rec.outcome.passed()
                    && let Some(look) = &mut rec.look
                {
                    look.different = Some(value);
                    rec.outcome = Outcome::Failed(
                        "The device display differed from the expected result.".into(),
                    );
                    let rec = rec.clone();
                    self.write_report();
                    return self.op.finished(&rec);
                }
                ResultDecision::Continue
            }
            decision => decision,
        }
    }
}

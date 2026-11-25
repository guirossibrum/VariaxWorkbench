//! The step runner: one explained, confirmed step at a time.
//!
//! * Every step shows its card and waits. The default answer is not to run.
//! * Nothing moves into a new step by itself.
//! * A step runs only when every step it needs has passed.
//! * Real mode refuses each device step whose owner approval is off.
//! * Stop ends the run; the rig guard (6.2) is still offered and still asks.
//! * The report sink is called after every step so a crash leaves evidence.

use super::approvals;
use super::catalogue::{self, Kind, RIG_GUARD, RUN_ORDER, START_NOTICE, SaveNotice, Step, StepId};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retry {
    TryAgain,
    Skip,
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

/// The person at the rig, through the GUI, the terminal or a test script.
pub trait Operator {
    /// The start notice. `false` (the default) quits before any step.
    fn accept_start(&mut self, notice: &str) -> bool;
    /// Show the step card and wait for Run, Skip or Stop.
    fn confirm(&mut self, step: &Step, mode: Mode) -> Decision;
    /// The second notice before a save. Returns the typed confirmation, or
    /// `None` to skip.
    fn confirm_save(&mut self, notice: &SaveNotice) -> Option<String>;
    /// Show large HEAR / SEE instructions for a guided action. Returns at once.
    fn instruct(&mut self, step: StepId, see: &str, hear: &str);
    /// A guided action was not seen in time.
    fn not_seen(&mut self, step: StepId, what: &str) -> Retry;
    /// A listening question with Yes / No / Not sure and an optional note.
    fn ask(&mut self, question: &Question) -> (Answer, String);
    /// A short progress line.
    fn progress(&mut self, text: &str);
    /// Publish any outcome. If the step ran, wait for the person to read it.
    /// Return false when they choose Stop or leave the session.
    fn finished(&mut self, record: &StepRecord) -> bool;
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
                self.op
                    .progress(&format!("Could not write the restore state: {error}"));
            }
        }
        if let Err(error) = self.sink.update(&self.run) {
            self.op
                .progress(&format!("Could not write the report: {error}"));
        }
    }

    /// Run the catalogue. Returns the record of the whole run.
    pub fn run(mut self) -> (RunRecord, RunData) {
        self.write_report();
        if !self.op.accept_start(START_NOTICE) {
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
            stopped = self.step(id);
            if stopped {
                self.write_report();
            }
        }
        // The rig guard is always offered last, also after Stop.
        self.step(RIG_GUARD);
        self.run.finished = true;
        self.write_report();
        (self.run, self.data)
    }

    /// Run or skip one step. Returns `true` when the person stopped the run.
    fn step(&mut self, id: StepId) -> bool {
        let step = catalogue::step(id).expect("run order uses catalogue IDs");
        if let Some(outcome) = self.precheck(step) {
            self.finish(id, outcome, None, None);
            return false;
        }
        match self.op.confirm(step, self.run.mode) {
            Decision::Skip => {
                self.finish(id, Outcome::SkippedByPerson, None, None);
                false
            }
            Decision::Stop => {
                self.run.stopped_at = Some(id);
                self.finish(
                    id,
                    Outcome::Stopped("You chose Stop before this step.".into()),
                    None,
                    None,
                );
                id != RIG_GUARD
            }
            Decision::Run => {
                self.run.current = Some(id);
                self.write_report();
                let started = utc_now();
                let clock = Instant::now();
                let mut rec = std::mem::replace(self.run.step_mut(id), StepRecord::new(id));
                let result = {
                    let mut ctx = Ctx {
                        mode: self.run.mode,
                        port: &mut *self.port,
                        op: &mut *self.op,
                        data: &mut self.data,
                        rec: &mut rec,
                    };
                    steps::execute(&mut ctx)
                };
                let stopped_now = self.op.stop_requested();
                let outcome = match result {
                    Ok(()) => Outcome::Passed,
                    Err(reason) if reason.starts_with(SKIPPED) => {
                        rec.show(reason.trim_start_matches(SKIPPED));
                        Outcome::SkippedByPerson
                    }
                    Err(error) if stopped_now => Outcome::Stopped(error),
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
                let continuing = self.finish(
                    id,
                    outcome,
                    Some(started),
                    Some(clock.elapsed().as_millis()),
                );
                if (stopped_now || !continuing) && id != RIG_GUARD {
                    self.run.stopped_at = Some(id);
                    return true;
                }
                false
            }
        }
    }

    /// Why a step cannot be offered, without asking.
    fn precheck(&self, step: &Step) -> Option<Outcome> {
        if let Some(reason) = step.not_in_this_version {
            return Some(Outcome::NotAvailable(reason.into()));
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
    ) -> bool {
        let rec = self.run.step_mut(id);
        rec.outcome = outcome;
        if started.is_some() {
            rec.started = started;
            rec.duration_ms = duration;
        }
        let rec = rec.clone();
        self.write_report();
        self.op.finished(&rec)
    }
}

//! Report preview and gated continuation. The report supplies history; the
//! companion checkpoint and its verified files supply the starting state.

use super::catalogue::{self, CATALOGUE_VERSION, RIG_GUARD, RUN_ORDER, Risk, StepId};
use super::procedures::{self, with_podxt, with_variax_progress};
use super::record::{Mode, Outcome, RunRecord, StepRecord, sha256_hex, utc_now};
use super::recover::{self, RigState};
use super::runner::{Ctx, Operator, Progress};
use super::steps::RunData;
use line6_devices::family::podxt::{self, EditBuffer};
use line6_devices::family::variax::procedures as variax;
use line6_devices::identity::Identity;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod evidence;
use evidence::{
    Folder, check_fact, check_hash, identity_from_record, legacy_variax_hash, load_podxt,
    load_variax, program, recheck_channel,
};

const POD_ID: StepId = StepId(2, 1);
const POD_START: StepId = StepId(2, 2);
const POD_BACKUP: StepId = StepId(2, 5);
const GUITAR_ID: StepId = StepId(3, 1);
const GUITAR_START: StepId = StepId(3, 3);
const GUITAR_BACKUP: StepId = StepId(3, 4);
const KNOBS: StepId = StepId(7, 5);
const SETTINGS: StepId = StepId(3, 2);
const PEDAL: StepId = StepId(7, 4);
const ENGINE: StepId = StepId(0, 3);
const LEGACY_DRIVE_FAILURE: &str = "The Drive value read back differs from the Follow update.";
const LEGACY_GUARD_STOP: &str = "You chose Stop before this step.";
const LEGACY_CLEAN_RELEASE: &str = "Not checked yet. PODxt Live interface 0 released.";
const LEGACY_BUILD: &str = "f13eaf72a5a17f4341607ded4f0751005ffe5cc0";

#[derive(Deserialize)]
struct SavedReport {
    report_format: u32,
    app_version: String,
    git_commit: Option<String>,
    catalogue_version: String,
    mode: String,
    started: String,
    finished: bool,
    rig_state: String,
    rig_description: String,
    stopped_at: Option<String>,
    #[serde(default)]
    pre_write_recovery: bool,
    #[serde(default)]
    continuation_pending: bool,
    computer: Vec<(String, String)>,
    devices: Vec<(String, String)>,
    steps: Vec<SavedStep>,
    prior_attempts: Vec<SavedStep>,
}

#[derive(Deserialize)]
struct SavedStep {
    id: String,
    title: String,
    outcome: String,
    reason: Option<String>,
    started: Option<String>,
    attempt: Option<usize>,
    duration_ms: Option<u128>,
    retries: u32,
    messages: usize,
    answers: Vec<(String, String)>,
    checksums: Vec<(String, String)>,
    results: Vec<String>,
    #[serde(default)]
    display_results: Vec<String>,
    technical: Vec<String>,
    hints: Vec<String>,
}

impl SavedStep {
    fn never_ran(&self) -> bool {
        self.started.is_none()
            && self.attempt.is_none()
            && self.duration_ms.is_none()
            && self.messages == 0
    }

    fn record(self) -> Result<StepRecord, String> {
        let id = StepId::parse(&self.id).ok_or("Unknown step ID in the report.")?;
        let reason = self.reason.unwrap_or_default();
        let outcome = match self.outcome.as_str() {
            "not_reached" => Outcome::NotReached,
            "passed" => Outcome::Passed,
            "failed" => Outcome::Failed(reason),
            "skipped" => Outcome::SkippedByPerson,
            "skipped_verified_firmware" => Outcome::SkippedVerifiedFirmware(reason),
            "skipped_not_applicable" => Outcome::SkippedNotApplicable(reason),
            "skipped_needs" => Outcome::SkippedNeeds(reason),
            "not_authorized" => Outcome::NotAuthorized(reason),
            "not_available" => Outcome::NotAvailable(reason),
            "unverified_device" => Outcome::UnverifiedDevice(reason),
            "stopped" => Outcome::Stopped(reason),
            _ => return Err(format!("Unknown outcome for step {id}.")),
        };
        if outcome.passed()
            && (self.started.is_none() || self.attempt.is_none_or(|attempt| attempt == 0))
        {
            return Err(format!("Step {id} is marked passed without an attempt."));
        }
        Ok(StepRecord {
            id,
            title: self.title,
            outcome,
            started: self.started,
            attempt: self.attempt,
            duration_ms: self.duration_ms,
            retries: self.retries,
            answers: self.answers,
            checksums: self.checksums,
            evidence: self.results,
            result: self.display_results,
            imported_technical: self.technical,
            hints: self.hints,
            ..StepRecord::new(id)
        })
    }
}

/// The most recently saved local report with its original companion filename.
pub fn last_report(dir: &Path) -> Option<PathBuf> {
    report_candidates(dir, false).max().map(|(_, path)| path)
}

fn report_candidates(
    dir: &Path,
    allow_collected: bool,
) -> impl Iterator<Item = (Option<std::time::SystemTime>, PathBuf)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(move |entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            ((name.starts_with("rig-check-") || (allow_collected && name == "latest.txt"))
                && path.extension().is_some_and(|ext| ext == "txt")
                && path.is_file()
                && path.with_extension("state").is_file())
            .then(|| (entry.metadata().ok().and_then(|m| m.modified().ok()), path))
        })
}

/// Find the newest compatible run across both Mac locations. Keep the newest
/// candidate available for its preview diagnostic if none can be continued.
pub fn last_mac_report(dir: &Path, legacy: &Path, mode: Option<Mode>) -> Option<PathBuf> {
    let mut candidates: Vec<_> = mac_report_candidates(dir, legacy).collect();
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    candidates
        .iter()
        .find(|(_, path)| open(path).is_ok_and(|plan| mode.is_none_or(|mode| mode == plan.mode)))
        .or_else(|| candidates.first())
        .map(|(_, path)| path.clone())
}

#[cfg(feature = "gui")]
pub(super) fn has_mac_reports(dir: &Path, legacy: &Path) -> bool {
    mac_report_candidates(dir, legacy).next().is_some()
}

fn mac_report_candidates(
    dir: &Path,
    legacy: &Path,
) -> impl Iterator<Item = (Option<std::time::SystemTime>, PathBuf)> {
    report_candidates(dir, true).chain(report_candidates(legacy, true))
}

/// A prerequisite failed before the selected continuation target could run.
#[derive(Clone, Debug)]
pub struct BlockedResume {
    pub target: StepId,
    pub check: StepId,
    pub reason: String,
    pub details: Vec<String>,
}

impl BlockedResume {
    pub const TITLE: &str = "Resume blocked";

    pub fn lines(&self) -> impl Iterator<Item = String> + '_ {
        let name = |id| catalogue::step(id).map_or("", |step| step.title);
        [
            format!("Selected target {} · {}", self.target, name(self.target)),
            format!("Blocking check {} · {}", self.check, name(self.check)),
            self.reason.clone(),
        ]
        .into_iter()
        .chain(self.details.iter().cloned())
    }
}

pub struct Plan {
    pub source: PathBuf,
    pub mode: Mode,
    report_sha256: String,
    checkpoint_sha256: String,
    original_path: PathBuf,
    pre_write_recovery: bool,
    pub target: StepId,
    pub rig: String,
    pub notes: Vec<String>,
    reused: Vec<StepRecord>,
    history: Vec<StepRecord>,
    restored: RunData,
    folders: Vec<Folder>,
    rebuild: BTreeSet<StepId>,
}

impl Plan {
    pub(super) fn reserve_successor(&self, checkpoint: &Path) -> Result<(), String> {
        let report = checkpoint
            .with_extension("txt")
            .canonicalize()
            .map_err(|error| format!("Cannot locate the continuation report: {error}"))?;
        crate::shared::document::save_new(
            &self.original_path.with_extension("continuation"),
            report.to_string_lossy().as_bytes(),
        )
        .map_err(|error| format!("Could not record this continuation: {error}"))
    }

    pub fn reused_steps(&self) -> String {
        self.reused
            .iter()
            .filter(|step| step.outcome.passed())
            .map(|step| step.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(super) fn skips(&self, id: StepId) -> bool {
        self.reused.iter().any(|record| record.id == id)
    }

    pub(super) fn seed(&mut self, run: &mut RunRecord) {
        run.prior_attempts = std::mem::take(&mut self.history);
        run.continuation_pending = true;
        run.pre_write_recovery = self.pre_write_recovery;
        run.continued_from = Some(self.source.display().to_string());
        run.continuation = self.notes.clone();
        run.computer = self.restored.computer.clone();
        run.devices = self.restored.devices.clone();
        run.rig_description = self.rig.clone();
    }

    pub(super) fn commit_reuse(&self, run: &mut RunRecord) {
        for record in &self.reused {
            *run.step_mut(record.id) = record.clone();
        }
        run.continuation_pending = false;
    }

    pub(super) fn lock_source(&self) -> Result<recover::CheckpointLock, String> {
        let lock = recover::lock_checkpoint(&self.original_path)?;
        let current = std::fs::read(&self.source)
            .map_err(|error| format!("Cannot reopen the source report: {error}"))?;
        let checkpoint = std::fs::read(self.source.with_extension("state"))
            .map_err(|error| format!("Cannot reopen the source checkpoint: {error}"))?;
        if sha256_hex(&current) != self.report_sha256
            || sha256_hex(&checkpoint) != self.checkpoint_sha256
            || self.original_path.with_extension("continuation").exists()
        {
            return Err(
                "The source report changed after preview. Open the latest report again.".into(),
            );
        }
        Ok(lock)
    }

    pub(super) fn is_selected_checkpoint(&self, checkpoint: &Path) -> Result<bool, String> {
        let Ok(checkpoint) = checkpoint.canonicalize() else {
            return Ok(false);
        };
        let is_checkpoint_for = |report: &Path| {
            report
                .with_extension("state")
                .canonicalize()
                .is_ok_and(|path| path == checkpoint)
        };
        if is_checkpoint_for(&self.source) {
            return Ok(true);
        }
        // A collected report can share the original basename's lock. Skip its
        // original companion only when it contains the same checkpoint.
        if self.original_path != self.source && is_checkpoint_for(&self.original_path) {
            if std::fs::read(checkpoint)
                .is_ok_and(|bytes| sha256_hex(&bytes) == self.checkpoint_sha256)
            {
                return Ok(true);
            }
            return Err("The collected report's checkpoint differs from the original. Verify the interrupted run using the original report.".into());
        }
        let alias = self.source.with_file_name("latest.txt");
        Ok(is_checkpoint_for(&alias)
            && open(&alias).is_ok_and(|plan| {
                plan.original_path == self.original_path
                    && plan.report_sha256 == self.report_sha256
                    && plan.checkpoint_sha256 == self.checkpoint_sha256
            }))
    }

    pub(super) fn restore_ready(&self, state: &RigState) -> Result<(), String> {
        if self.restored.podxt_identity.is_none()
            || (state.channel().is_some() && self.restored.start_tone.is_none())
            || state.preset().is_some() != self.restored.variax_identity.is_some()
            || (state.preset().is_some()
                && (self.restored.start_program.is_none()
                    || recover::usable_variax_hash(state).is_none()))
        {
            return Err("The pending report is missing a verified identity, starting snapshot or recoverable starting preset.".into());
        }
        Ok(())
    }

    pub(super) fn verify_restore_identity(
        &self,
        ctx: &mut Ctx<'_>,
        state: &RigState,
    ) -> Result<(), String> {
        self.restore_ready(state)?;
        ctx.data.podxt_identity = self.restored.podxt_identity.clone();
        ctx.data.variax_identity = self.restored.variax_identity.clone();
        check_pod_identity(ctx)?;
        if state.preset().is_some() {
            check_guitar_identity(ctx)?;
        }
        Ok(())
    }

    /// Run only read/identity operations before making any prior evidence
    /// available to the runner. A refusal must bypass the restoring guard.
    pub(super) fn revalidate(
        &mut self,
        port: &mut dyn super::port::RigPort,
        op: &mut dyn Operator,
        run: &mut RunRecord,
        data: &mut RunData,
    ) -> Result<(), String> {
        if self.mode != port.mode() {
            return Err("Report mode differs from the running port.".into());
        }
        if self.mode == Mode::Real
            && let Some(reason) = crate::offline_device_notice()
        {
            return Err(reason.into());
        }
        if self.mode == Mode::Real {
            for record in &self.reused {
                let step = catalogue::step(record.id).expect("validated resume step");
                if record.outcome.passed() && step.needs_device_approval() {
                    super::approvals::real_device_check(record.id).map_err(|reason| {
                        format!("Step {} can no longer be reused: {reason}", record.id)
                    })?;
                }
            }
        }
        let note = if self.pre_write_recovery {
            "Continue after the stopped final rig guard? Rig Check first checks the original settings and both backups using read-only operations. It then runs the guided checks and a fresh final guard. The confirmed return may restore the PODxt working buffer. Live edit checks and preset stores are unavailable for this continuation."
        } else {
            "Continue this saved run? Rig Check will check the connected rig and starting state before reusing results. Each new device action still requires your confirmation."
        };
        if !op.accept_start(note) {
            return Err("Continuation was declined.".into());
        }
        let mut recheck = |id, procedure: fn(&mut Ctx<'_>) -> Result<(), String>| {
            op.progress(Progress::action(format!("Revalidating step {id}")));
            let mut record = StepRecord::new(id);
            record.title = format!("{} (continuation revalidation)", record.title);
            record.started = Some(utc_now());
            record.attempt = Some(next_attempt(run, id));
            let mut ctx = Ctx {
                mode: self.mode,
                port,
                op,
                data: &mut self.restored,
                rec: &mut record,
            };
            let result = if ctx.stopping() {
                Err("Continuation stopped before revalidation.".into())
            } else {
                procedure(&mut ctx)
            };
            record.outcome = match &result {
                Ok(()) => Outcome::Passed,
                Err(error) => {
                    let details = record
                        .result
                        .iter()
                        .cloned()
                        .chain(
                            record
                                .comparisons
                                .iter()
                                .filter(|comparison| {
                                    comparison.severity == super::differences::Severity::Stop
                                })
                                .flat_map(|comparison| comparison.details.iter().cloned()),
                        )
                        .collect();
                    run.blocked_resume = Some(BlockedResume {
                        target: self.target,
                        check: id,
                        reason: error.clone(),
                        details,
                    });
                    Outcome::Failed(error.clone())
                }
            };
            run.prior_attempts.push(record);
            result
        };
        let pod = self.reused.iter().any(|step| step.id == POD_ID);
        let guitar = self.reused.iter().any(|step| step.id == GUITAR_ID);
        if pod {
            recheck(POD_ID, check_pod_identity)?;
            if self.reused.iter().any(|step| step.id == POD_START) {
                recheck(POD_START, check_pod_start)?;
            }
            if self.reused.iter().any(|step| step.id == POD_BACKUP) {
                recheck(POD_BACKUP, check_saved_pod_channel)?;
            }
        }
        if guitar {
            recheck(GUITAR_ID, check_guitar_identity)?;
            if self.reused.iter().any(|step| step.id == GUITAR_START) {
                recheck(GUITAR_START, check_guitar_start)?;
            }
        }
        if !self.skips(POD_BACKUP) && self.skips(StepId(2, 3)) {
            recheck_channel(&self.reused, &mut self.restored)?;
        }
        let root = data.evidence_dir.take();
        if !self.folders.is_empty() {
            let root = root
                .as_ref()
                .ok_or("No folder for the continuation evidence.")?;
            for folder in &self.folders {
                folder.copy_to(root)?;
            }
            if let Some(folder) = &self.restored.podxt_backup_dir {
                self.restored.podxt_backup_dir = Some(root.join(folder));
            }
        }
        self.restored.evidence_dir = root;
        self.restored.engine_folder = data.engine_folder.take();
        std::mem::swap(data, &mut self.restored);
        if !self.pre_write_recovery && self.skips(ENGINE) && !self.skips(StepId(4, 3)) {
            let mut rec = StepRecord::new(ENGINE);
            let mut ctx = Ctx {
                mode: self.mode,
                port,
                op,
                data,
                rec: &mut rec,
            };
            super::steps::execute(&mut ctx)?;
        }
        run.continuation
            .push("Starting state and identities required by the reused steps verified.".into());
        Ok(())
    }
}

pub(super) fn next_attempt(run: &RunRecord, id: StepId) -> usize {
    run.prior_attempts
        .iter()
        .filter(|step| step.id == id)
        .filter_map(|step| step.attempt)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn check_pod_identity(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let reply = with_podxt(ctx, procedures::PODXT_IDENTITY, |session| {
        podxt::identify_podxt_live(session)
    })?;
    let identity = Identity::parse(&reply).ok_or("Invalid PODxt Live identity reply.")?;
    podxt::check_live_identity(&identity)?;
    if Some(&identity) != ctx.data.podxt_identity.as_ref() {
        return Err("The current PODxt Live identity differs from the report.".into());
    }
    ctx.rec
        .show("PODxt Live identity and firmware match the source report.");
    Ok(())
}

fn check_pod_start(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let (current, channel) = with_podxt(ctx, procedures::PODXT_CHANNEL, |session| {
        Ok((
            podxt::read_current(session)?.tone,
            podxt::playing_channel(session)?,
        ))
    })?;
    if Some(channel) != ctx.data.start_channel
        || !ctx
            .data
            .start_tone
            .as_ref()
            .is_some_and(|start| ctx.data.same_starting_pod_tone(start, &current))
    {
        let expected_channel = ctx
            .data
            .start_channel
            .map_or_else(|| "unavailable".into(), |channel| channel.label());
        ctx.rec.show(format!(
            "PODxt Live channel · expected {expected_channel} · observed {}",
            channel.label()
        ));
        if let Some(start) = &ctx.data.start_tone {
            super::differences::tone(
                ctx.rec,
                "PODxt Live starting tone",
                &start.raw,
                &current.raw,
                super::differences::Severity::Stop,
                true,
            );
            if ctx.data.pre_write_recovery && start.cc(17) != current.cc(17) {
                ctx.rec.show(format!(
                    "Channel Volume (CC 17) · expected {} · observed {}",
                    start.cc(17),
                    current.cc(17)
                ));
            }
        }
        return Err("The PODxt Live channel or tone differs from the saved starting state. Use the original report to return the channel and current tone settings by hand, then reopen it. Leave the bank unchanged.".into());
    }
    ctx.rec
        .show("PODxt Live starting tone and channel verified.");
    Ok(())
}

fn check_saved_pod_channel(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let channel = ctx.data.start_channel.ok_or("No starting PODxt channel.")?;
    let saved = &ctx
        .data
        .podxt_library
        .iter()
        .find(|(location, _)| *location == channel)
        .ok_or("The starting PODxt channel is missing from the verified backup.")?
        .1;
    let expected = saved.raw.clone();
    let current = with_podxt(ctx, procedures::PODXT_LIBRARY, |session| {
        procedures::podxt_read_stored(session, channel)
    })?;
    if current.raw != expected {
        return Err(format!(
            "Stored PODxt channel {} differs from the saved backup. Check the rig and backup before continuing.",
            channel.label()
        ));
    }
    ctx.rec.show(format!(
        "Stored PODxt channel {} matches the backup.",
        channel.label()
    ));
    Ok(())
}

fn check_guitar_identity(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let current = procedures::with_variax(ctx, procedures::VARIAX_IDENTITY, |session, _| {
        variax::identify_reported(session)
    })?;
    variax::check_identity(&current.identity)?;
    if Some(&current.identity) != ctx.data.variax_identity.as_ref() {
        return Err("The current Variax identity differs from the report.".into());
    }
    ctx.rec
        .show("Variax identity and firmware match the source report.");
    Ok(())
}

fn check_guitar_start(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let read = with_variax_progress(ctx, procedures::VARIAX_READ, |session, stop, progress| {
        procedures::drain(session)?;
        procedures::variax_full_read(
            session,
            stop,
            progress,
            "Checking the Variax starting program",
        )
    })?;
    if !ctx
        .data
        .start_program
        .as_ref()
        .is_some_and(|start| procedures::same_program(&read.words, start))
    {
        return Err("The Variax starting program changed or failed its checksum. Check the original report and return to the starting program before trying again.".into());
    }
    ctx.rec
        .show("Variax starting program verified, including its firmware checksum.");
    Ok(())
}

fn order(legacy: bool) -> Vec<StepId> {
    if !legacy {
        return RUN_ORDER.to_vec();
    }
    let mut order: Vec<_> = RUN_ORDER
        .iter()
        .copied()
        .filter(|&id| id != KNOBS)
        .collect();
    let pedal = order
        .iter()
        .position(|&id| id == PEDAL)
        .expect("pedal step");
    order.insert(pedal + 1, KNOBS);
    order
}

fn applicable(record: &StepRecord) -> bool {
    let definition = catalogue::step(record.id).expect("saved step definition");
    definition.not_in_this_version.is_none()
        && !matches!(
            record.outcome,
            Outcome::SkippedVerifiedFirmware(_) | Outcome::SkippedNotApplicable(_)
        )
}

/// The released f13eaf7 Mac run stopped before offering any guard I/O.
fn stopped_before_legacy_guard(saved: &SavedReport) -> bool {
    let old_drive_failure = |step: &SavedStep, attempt| {
        step.id == "7.5"
            && step.outcome == "failed"
            && step.attempt == Some(attempt)
            && step.reason.as_deref() == Some(LEGACY_DRIVE_FAILURE)
            && step.started.is_some()
    };
    saved.finished
        && saved.stopped_at.as_deref() == Some("6.2")
        && saved.rig_state == LEGACY_CLEAN_RELEASE
        && saved.steps.iter().any(|step| {
            step.id == "6.2"
                && step.outcome == "stopped"
                && step.reason.as_deref() == Some(LEGACY_GUARD_STOP)
                && step.never_ran()
        })
        && saved.steps.iter().any(|step| old_drive_failure(step, 2))
        && saved
            .prior_attempts
            .iter()
            .filter(|step| step.id == "7.5")
            .count()
            == 1
        && saved
            .prior_attempts
            .iter()
            .any(|step| old_drive_failure(step, 1))
        && saved
            .steps
            .iter()
            .chain(&saved.prior_attempts)
            .filter(|step| {
                StepId::parse(&step.id)
                    .and_then(catalogue::step)
                    .is_some_and(|definition| {
                        definition.not_in_this_version.is_none()
                            && matches!(definition.risk, Risk::Live | Risk::Save)
                    })
            })
            .all(|step| {
                step.never_ran() && matches!(step.outcome.as_str(), "not_reached" | "skipped_needs")
            })
}

fn check_prior_actions(
    run: &RunRecord,
    current: Option<StepId>,
    stopped_before_guard: bool,
) -> Result<(), String> {
    if run.rig_state.contains("USB cleanup failed")
        || run.rig_state.contains("Driver not returned")
        || run.rig_state.contains("interface release failed")
    {
        return Err("The earlier run could not release the rig. Follow the original report's cleanup instructions and verify the rig before starting a new run.".into());
    }
    let risk = |id| catalogue::step(id).expect("saved step definition").risk;
    if current.is_some_and(|id| matches!(risk(id), Risk::Live | Risk::Save)) {
        return Err("The report was interrupted during a live edit or store. Verify the rig and any affected slots manually.".into());
    }
    for step in run.steps.iter().chain(&run.prior_attempts) {
        if risk(step.id) == Risk::Save
            && (step.started.is_some() || step.attempt.is_some())
            && !step.outcome.passed()
        {
            return Err(format!(
                "Store {} has an unresolved outcome. Verify the saved slot manually. The store will not be retried.",
                step.id
            ));
        }
    }
    if run
        .steps
        .iter()
        .any(|step| risk(step.id) == Risk::Live && step.started.is_some() && !step.outcome.passed())
    {
        return Err("A prior live edit could not be verified. Check the rig state before starting another run.".into());
    }
    if run.prior_attempts.iter().any(|step| {
        matches!(risk(step.id), Risk::Live | Risk::Save)
            && step.outcome.passed()
            && !run.step(step.id).outcome.passed()
    }) {
        return Err("The report records a completed write without a matching final result. Rig Check will not replay it.".into());
    }
    if current == Some(RIG_GUARD)
        || (!stopped_before_guard
            && (run.stopped_at == Some(RIG_GUARD)
                || matches!(run.step(RIG_GUARD).outcome, Outcome::Stopped(_))))
    {
        return Err(
            "The final rig guard was stopped. Check the rig manually before starting another run."
                .into(),
        );
    }
    let guard = &run.step(RIG_GUARD).outcome;
    let interrupted_sweep_restored_by_hand = !run.step(KNOBS).outcome.passed()
        && guard.reason() == Some("The PODxt Live tone or channel differs from the start.");
    if !interrupted_sweep_restored_by_hand
        && matches!(guard, Outcome::Failed(_) | Outcome::NotAvailable(_))
    {
        return Err("The final rig guard could not verify the starting state. Check the rig and original report manually.".into());
    }
    Ok(())
}

fn original_report_path(
    path: &Path,
    state: &RigState,
    allow_collected_copy: bool,
) -> Result<PathBuf, String> {
    if path.file_name().and_then(|name| name.to_str()) == Some(&state.report) {
        return Ok(path.to_owned());
    }
    if allow_collected_copy
        && path.file_name().is_some_and(|name| name == "latest.txt")
        && let Some(stamp) = state
            .report
            .strip_prefix("rig-check-")
            .and_then(|name| name.strip_suffix(".txt"))
        && stamp.len() == 16
        && stamp.as_bytes()[8] == b'T'
        && stamp.as_bytes()[15] == b'Z'
        && stamp
            .bytes()
            .enumerate()
            .all(|(i, ch)| matches!(i, 8 | 15) || ch.is_ascii_digit())
    {
        return Ok(path.with_file_name(&state.report));
    }
    Err("The report and companion checkpoint do not match. Keep the original report name and its matching .state file and evidence folder together.".into())
}

/// Load and validate the original local report and its companion files.
/// No port is opened while previewing. A legacy report is accepted only from
/// the known build, with the exact old order of stable IDs and legacy hashes.
pub fn open(path: &Path) -> Result<Plan, String> {
    let source = path
        .canonicalize()
        .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let path = source.as_path();
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let saved: SavedReport = serde_json::from_str(
        super::report::json_block(&text).ok_or("No complete Rig Check report found.")?,
    )
    .map_err(|error| format!("Invalid Rig Check report: {error}"))?;
    let legacy = saved.catalogue_version == "1.5"
        && matches!(saved.git_commit.as_deref(), Some("f13eaf7" | LEGACY_BUILD));
    if saved.report_format != super::report::REPORT_FORMAT
        || saved.app_version != env!("CARGO_PKG_VERSION")
        || (!legacy && saved.catalogue_version != CATALOGUE_VERSION)
    {
        return Err("This report's build or step catalogue is incompatible.".into());
    }
    if saved.continuation_pending {
        return Err("This continuation never finished verifying the starting rig. Open its source report. If the source already names this continuation, verify the starting rig by hand and choose Start new.".into());
    }
    let mode = [Mode::Practice, Mode::Real]
        .into_iter()
        .find(|mode| mode.label() == saved.mode)
        .ok_or("Unknown mode in the report.")?;
    let checkpoint = std::fs::read_to_string(path.with_extension("state"))
        .map_err(|_| "The report's companion checkpoint is missing or invalid.")?;
    let state = RigState::parse(&checkpoint)
        .ok_or("The report's companion checkpoint is missing or invalid.")?;
    let current = super::report::interrupted_step(&text);
    let stopped_before_guard = legacy && current.is_none() && stopped_before_legacy_guard(&saved);
    let original_path = original_report_path(path, &state, stopped_before_guard)?;
    if saved.started != state.started
        || saved.finished != state.finished
        || (!legacy && state.report_sha256.as_deref() != Some(&sha256_hex(text.as_bytes())))
    {
        return Err("The report and companion checkpoint do not match.".into());
    }
    if original_path != path
        && original_path.exists()
        && std::fs::read(&original_path).ok().as_deref() != Some(text.as_bytes())
    {
        return Err("The collected report differs from the original beside it.".into());
    }
    if original_path.with_extension("continuation").exists() {
        let next = std::fs::read_to_string(original_path.with_extension("continuation"))
            .unwrap_or_default();
        return Err(format!(
            "This report already has a continuation. Open the continuation report {next}."
        ));
    }
    let mut run = RunRecord::new(mode, saved.started);
    run.finished = saved.finished;
    run.stopped_at = saved
        .stopped_at
        .as_deref()
        .map(|id| StepId::parse(id).ok_or("Unknown stopped step in the report."))
        .transpose()?;
    run.rig_state = saved.rig_state;
    run.computer = saved.computer;
    run.devices = saved.devices;
    let records = saved
        .steps
        .into_iter()
        .map(SavedStep::record)
        .collect::<Result<Vec<_>, _>>()?;
    if records.iter().map(|step| step.id).ne(order(legacy)) {
        return Err("Report step IDs or their order do not match its catalogue.".into());
    }
    for record in records {
        let id = record.id;
        *run.step_mut(id) = record;
    }
    run.prior_attempts = saved
        .prior_attempts
        .into_iter()
        .map(SavedStep::record)
        .collect::<Result<_, _>>()?;
    check_prior_actions(&run, current, stopped_before_guard)?;
    if saved.pre_write_recovery && current == Some(StepId(7, 3)) {
        return Err("The continuation closed during the footswitch return. Verify the PODxt channel and working tone by hand before starting a new run.".into());
    }

    let target = RUN_ORDER
        .iter()
        .copied()
        .find(|&id| {
            id != RIG_GUARD
                && !(saved.pre_write_recovery
                    && catalogue::step(id)
                        .is_some_and(|step| matches!(step.risk, Risk::Live | Risk::Save)))
                && (legacy && id == KNOBS
                    || applicable(run.step(id)) && !run.step(id).outcome.passed())
        })
        .or_else(|| (run.step(RIG_GUARD).outcome != Outcome::Passed).then_some(RIG_GUARD))
        .ok_or("Every applicable step in this run already passed.")?;
    if stopped_before_guard && target != KNOBS {
        return Err("An earlier prerequisite did not pass before the final guard was stopped. Verify the starting rig manually, then choose Start new.".into());
    }
    let mut plan = Plan {
        source: path.to_owned(),
        mode,
        report_sha256: sha256_hex(text.as_bytes()),
        checkpoint_sha256: sha256_hex(checkpoint.as_bytes()),
        original_path,
        pre_write_recovery: stopped_before_guard || saved.pre_write_recovery,
        target,
        rig: saved.rig_description,
        notes: Vec::new(),
        reused: Vec::new(),
        history: std::mem::take(&mut run.prior_attempts),
        restored: RunData {
            computer: run.computer.clone(),
            devices: run.devices.clone(),
            ..RunData::default()
        },
        folders: Vec::new(),
        rebuild: BTreeSet::new(),
    };
    if legacy {
        plan.notes.push("Migrating catalogue 1.5 from f13eaf7. The old Drive result stays in Earlier Attempts. The six-knob sweep and footswitch return use the current procedure.".into());
    }
    plan.notes
        .push(format!("First failed or unfinished check: {target}."));
    if plan.pre_write_recovery {
        plan.notes.push("The earlier final guard was stopped before it ran. Fresh read-only checks must confirm the starting state before the retry. This continuation runs a fresh guard and blocks live edit checks and all stores.".into());
    }
    let stem = plan
        .original_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("Invalid report filename.")?;
    let root = path.with_file_name(format!("{stem}-evidence"));
    let before_target = |id| {
        RUN_ORDER
            .iter()
            .take_while(|&&step| step != target)
            .any(|&step| step == id)
    };

    // Recover the prerequisite captures for any step we would reuse.
    for (id, base, label) in [
        (POD_ID, "podxt-identity", "PODxt identity manifest"),
        (POD_START, "podxt-start", "PODxt start manifest"),
        (GUITAR_START, "variax-start", "Variax start manifest"),
    ] {
        let step = run.step(id);
        if step.outcome.passed() && before_target(id) {
            let folder = Folder::read(&root, step, base, label, legacy)?;
            match id {
                POD_ID => {
                    let bytes = folder.file("identity.bin")?;
                    let identity = identity_from_record(step)?;
                    if Identity::parse(bytes).as_ref() != Some(&identity) {
                        return Err("PODxt identity file differs from the report.".into());
                    }
                    podxt::check_live_identity(&identity)?;
                    plan.restored.podxt_identity = Some(identity);
                    plan.restored.podxt_verified = Some(true);
                }
                POD_START => {
                    let bytes = folder.file("edit-buffer.bin")?;
                    check_hash(step, "PODxt current tone (168 bytes)", bytes)?;
                    if state.podxt_tone_sha256.as_deref() != Some(&sha256_hex(bytes)) {
                        return Err("The PODxt starting tone differs from the checkpoint.".into());
                    }
                    plan.restored.start_tone = Some(EditBuffer::parse(bytes.to_vec())?);
                    let channel = state
                        .channel()
                        .ok_or("No starting PODxt channel in the checkpoint.")?;
                    check_fact(
                        step,
                        format!(
                            "Starting channel reported by the PODxt Live: {}",
                            channel.label()
                        ),
                    )?;
                    plan.restored.start_channel = Some(channel);
                }
                GUITAR_START => {
                    let bytes = folder.file("program.bin")?;
                    check_hash(step, "Variax current program (1,092 words)", bytes)?;
                    let start = program(bytes)?;
                    let hash = if legacy {
                        Some(legacy_variax_hash(&start))
                    } else {
                        recover::match_hash(&start)
                    };
                    if hash.is_none() || hash != state.variax_match_sha256 {
                        return Err(
                            "The Variax starting program differs from the checkpoint.".into()
                        );
                    }
                    plan.restored.start_program = Some(start);
                    plan.restored.start_settings = Some(procedures::settings_of(&start));
                }
                _ => unreachable!(),
            }
            plan.folders.push(folder);
        }
    }
    if run.step(GUITAR_ID).outcome.passed() && before_target(GUITAR_ID) {
        let identity = identity_from_record(run.step(GUITAR_ID))?;
        variax::check_identity(&identity)?;
        plan.restored.variax_identity = Some(identity);
        plan.restored.variax_verified = Some(true);
    }

    for (id, recover_backup) in [
        (
            POD_BACKUP,
            load_podxt
                as fn(&Path, &StepRecord, bool, &RigState, &mut RunData) -> Result<Folder, String>,
        ),
        (GUITAR_BACKUP, load_variax),
    ] {
        let step = run.step(id);
        if step.outcome.passed() && before_target(id) {
            match recover_backup(&root, step, legacy, &state, &mut plan.restored) {
                Ok(folder) => plan.folders.push(folder),
                Err(error) => {
                    if plan.pre_write_recovery {
                        return Err(format!(
                            "Backup {id} cannot be verified: {error} Check the original evidence, or verify the rig manually and choose Start new."
                        ));
                    }
                    plan.rebuild.insert(id);
                    plan.notes
                        .push(format!("Backup {id} needs to be read again: {error}"));
                }
            }
        }
    }
    if plan.pre_write_recovery
        && [
            POD_ID,
            POD_START,
            POD_BACKUP,
            GUITAR_ID,
            GUITAR_START,
            GUITAR_BACKUP,
        ]
        .into_iter()
        .any(|id| !run.step(id).outcome.passed() || !before_target(id))
    {
        return Err("The stopped-guard recovery needs both saved identities, starting snapshots and complete backups. Verify the rig manually, then choose Start new.".into());
    }
    if plan.pre_write_recovery {
        let data = &mut plan.restored;
        data.pre_write_recovery = true;
        let start = data
            .start_program
            .as_ref()
            .ok_or("Missing starting Variax program.")?;
        let mut matching = data
            .variax_library
            .iter()
            .filter(|(_, words)| procedures::same_program(start, words));
        if matching.next().map(|(preset, _)| *preset) != data.start_preset
            || matching.next().is_some()
        {
            return Err("The original Variax starting program does not identify a unique saved preset. Verify the guitar's starting state by hand, then choose Start new.".into());
        }
        if let (Some(start), Some(saved)) = (
            data.start_tone.as_ref(),
            data.podxt_library
                .iter()
                .find(|(location, _)| Some(*location) == data.start_channel)
                .map(|(_, tone)| tone),
        ) {
            data.podxt_unsaved = !data.same_starting_pod_tone(start, saved);
        }
    }
    if target == GUITAR_START && run.step(SETTINGS).outcome.passed() {
        plan.rebuild.insert(SETTINGS);
        plan.notes.push(
            "The Variax settings will be read again for the full-program cross-check.".into(),
        );
    }
    if plan.rebuild.contains(&GUITAR_BACKUP) && run.step(StepId(3, 5)).outcome.passed() {
        plan.rebuild.insert(StepId(3, 5));
    }
    let mut interrupted = current;
    for mut step in run.steps {
        if legacy {
            for (base, label) in [
                ("podxt-identity", "PODxt identity manifest"),
                ("podxt-start", "PODxt start manifest"),
                ("variax-start", "Variax start manifest"),
                ("podxt-library", "PODxt library manifest"),
                ("variax-library", "Variax library manifest"),
            ] {
                if let Some(folder) = plan
                    .folders
                    .iter()
                    .find(|folder| folder.name == step.evidence_folder(base))
                    && matches!(
                        (base, step.id),
                        ("podxt-identity", POD_ID)
                            | ("podxt-start", POD_START)
                            | ("variax-start", GUITAR_START)
                            | ("podxt-library", POD_BACKUP)
                            | ("variax-library", GUITAR_BACKUP)
                    )
                {
                    step.checksum(label, folder.file("MANIFEST.sha256")?);
                }
            }
        }
        let id = step.id;
        let done_write = step.outcome.passed()
            && matches!(
                catalogue::step(id).expect("saved step").risk,
                Risk::Live | Risk::Save
            );
        let retain = id != RIG_GUARD
            && !plan.rebuild.contains(&id)
            && (done_write
                || (before_target(id) && (step.outcome.passed() || !applicable(&step)))
                || (legacy && target == KNOBS && id == PEDAL && step.outcome.passed()));
        if retain {
            step.reused = true;
            plan.reused.push(step);
        } else if step.outcome != Outcome::NotReached {
            plan.history.push(step);
        } else if interrupted == Some(id) {
            interrupted = None;
            step.outcome =
                Outcome::Stopped("The app closed before this step recorded a result.".into());
            plan.history.push(step);
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod tests;

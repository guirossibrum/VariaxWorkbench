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
    answers: Vec<(String, String)>,
    checksums: Vec<(String, String)>,
    results: Vec<String>,
    #[serde(default)]
    display_results: Vec<String>,
    technical: Vec<String>,
    hints: Vec<String>,
}

impl SavedStep {
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
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.starts_with("rig-check-")
                && path.extension().is_some_and(|ext| ext == "txt")
                && path.with_extension("state").is_file())
            .then(|| (entry.metadata().ok().and_then(|m| m.modified().ok()), path))
        })
        .max()
        .map(|(_, path)| path)
}

pub struct Plan {
    pub source: PathBuf,
    pub mode: Mode,
    report_sha256: String,
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
            &self.source.with_extension("continuation"),
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
        for record in &self.reused {
            *run.step_mut(record.id) = record.clone();
        }
        run.continued_from = Some(self.source.display().to_string());
        run.continuation = self.notes.clone();
        run.computer = self.restored.computer.clone();
        run.devices = self.restored.devices.clone();
        run.rig_description = self.rig.clone();
    }

    pub(super) fn lock_source(&self) -> Result<std::fs::File, String> {
        let lock = recover::lock_checkpoint(&self.source)?;
        let current = std::fs::read(&self.source)
            .map_err(|error| format!("Cannot reopen the source report: {error}"))?;
        if sha256_hex(&current) != self.report_sha256
            || self.source.with_extension("continuation").exists()
        {
            return Err(
                "The source report changed after preview. Open the latest report again.".into(),
            );
        }
        Ok(lock)
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
        let note = "Continue this saved run? Rig Check will check the connected rig and starting state before reusing results. Each new device action still requires your confirmation.";
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
                Err(error) => Outcome::Failed(error.clone()),
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
        if self.skips(ENGINE) && !self.skips(StepId(4, 3)) {
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
            .is_some_and(|start| super::differences::same_podxt_tone(&current.raw, &start.raw))
    {
        return Err("The PODxt Live channel or tone differs from the saved starting state. Check the original report and return to the starting tone before trying again.".into());
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

fn check_prior_actions(run: &RunRecord, current: Option<StepId>) -> Result<(), String> {
    if current == Some(RIG_GUARD)
        || run.stopped_at == Some(RIG_GUARD)
        || matches!(run.step(RIG_GUARD).outcome, Outcome::Stopped(_))
    {
        return Err(
            "The final rig guard was stopped. Check the rig manually before starting another run."
                .into(),
        );
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

/// Load and validate the original local report and its companion files.
/// No port is opened while previewing. A legacy report is accepted only from
/// the known build, with the exact old order of stable IDs and legacy hashes.
pub fn open(path: &Path) -> Result<Plan, String> {
    let source = path
        .canonicalize()
        .map_err(|error| format!("Cannot open {}: {error}", path.display()))?;
    let path = source.as_path();
    if path.with_extension("continuation").exists() {
        let next = std::fs::read_to_string(path.with_extension("continuation")).unwrap_or_default();
        return Err(format!(
            "This report already has a continuation. Open the continuation report {next}."
        ));
    }
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
    let mode = [Mode::Practice, Mode::Real]
        .into_iter()
        .find(|mode| mode.label() == saved.mode)
        .ok_or("Unknown mode in the report.")?;
    let state = std::fs::read_to_string(path.with_extension("state"))
        .ok()
        .and_then(|text| RigState::parse(&text))
        .ok_or("The report's companion checkpoint is missing or invalid.")?;
    if path.file_name().and_then(|name| name.to_str()) != Some(&state.report)
        || saved.started != state.started
        || saved.finished != state.finished
        || (!legacy && state.report_sha256.as_deref() != Some(&sha256_hex(text.as_bytes())))
    {
        return Err("The report and companion checkpoint do not match.".into());
    }
    let current = super::report::interrupted_step(&text);
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
    check_prior_actions(&run, current)?;

    let target = RUN_ORDER
        .iter()
        .copied()
        .find(|&id| {
            id != RIG_GUARD
                && (legacy && id == KNOBS
                    || applicable(run.step(id)) && !run.step(id).outcome.passed())
        })
        .or_else(|| (run.step(RIG_GUARD).outcome != Outcome::Passed).then_some(RIG_GUARD))
        .ok_or("Every applicable step in this run already passed.")?;
    let mut plan = Plan {
        source: path.to_owned(),
        mode,
        report_sha256: sha256_hex(text.as_bytes()),
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
    let stem = path
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
                    plan.rebuild.insert(id);
                    plan.notes
                        .push(format!("Backup {id} needs to be read again: {error}"));
                }
            }
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

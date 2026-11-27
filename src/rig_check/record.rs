//! What a run records: one [`StepRecord`] per catalogue step, plus run facts.

use super::catalogue::{self, CATALOGUE_VERSION, StepId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Simulated devices. Nothing is sent to real hardware.
    Practice,
    /// Real devices, each step behind its owner approval and hardware gates.
    Real,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Practice => "practice (simulated devices)",
            Mode::Real => "real devices",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotReached,
    Passed,
    Failed(String),
    SkippedByPerson,
    SkippedVerifiedFirmware(String),
    SkippedNotApplicable(String),
    /// A step it needs did not pass.
    SkippedNeeds(String),
    NotAuthorized(String),
    NotAvailable(String),
    UnverifiedDevice(String),
    /// The person chose Stop at this step, or stopped it while it ran.
    Stopped(String),
}

impl Outcome {
    pub fn verified_firmware_skip(device: &str, version: &str) -> Self {
        Self::SkippedVerifiedFirmware(format!(
            "Skipped: your {device} reports firmware {version}, the tested version. This check is only for other versions."
        ))
    }

    pub fn technical_note(&self) -> Option<&'static str> {
        matches!(self, Self::SkippedVerifiedFirmware(_))
            .then_some("evidence mode not entered: firmware verified")
    }

    pub fn label(&self) -> &'static str {
        match self {
            Outcome::NotReached => "NOT REACHED",
            Outcome::Passed => "PASSED",
            Outcome::Failed(_) => "FAILED",
            Outcome::SkippedByPerson => "SKIPPED (by you)",
            Outcome::SkippedVerifiedFirmware(_) | Outcome::SkippedNotApplicable(_) => "SKIPPED",
            Outcome::SkippedNeeds(_) => "SKIPPED (needs an earlier step)",
            Outcome::NotAuthorized(_) => "NOT AUTHORIZED",
            Outcome::NotAvailable(_) => "NOT AVAILABLE",
            Outcome::UnverifiedDevice(_) => "SKIPPED (unverified device)",
            Outcome::Stopped(_) => "STOPPED",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Outcome::NotReached => "not_reached",
            Outcome::Passed => "passed",
            Outcome::Failed(_) => "failed",
            Outcome::SkippedByPerson => "skipped",
            Outcome::SkippedVerifiedFirmware(_) => "skipped_verified_firmware",
            Outcome::SkippedNotApplicable(_) => "skipped_not_applicable",
            Outcome::SkippedNeeds(_) => "skipped_needs",
            Outcome::NotAuthorized(_) => "not_authorized",
            Outcome::NotAvailable(_) => "not_available",
            Outcome::UnverifiedDevice(_) => "unverified_device",
            Outcome::Stopped(_) => "stopped",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Outcome::Failed(reason)
            | Outcome::SkippedNeeds(reason)
            | Outcome::SkippedVerifiedFirmware(reason)
            | Outcome::SkippedNotApplicable(reason)
            | Outcome::NotAuthorized(reason)
            | Outcome::NotAvailable(reason)
            | Outcome::UnverifiedDevice(reason)
            | Outcome::Stopped(reason) => Some(reason),
            _ => None,
        }
    }

    pub fn passed(&self) -> bool {
        *self == Outcome::Passed
    }
}

/// One line of the device conversation, as logged by the session.
pub type MessageLine = String;

#[derive(Clone, Debug)]
pub struct LookCheck {
    pub prompt: &'static str,
    pub expected: String,
    pub different: Option<String>,
}

impl LookCheck {
    pub fn report_line(&self) -> String {
        let reported = self.different.as_ref().map_or_else(
            || "not reported different".to_string(),
            |value| format!("{value:?}"),
        );
        format!("Expected {:?} · On the device: {reported}", self.expected)
    }
}

/// Only 5.1 currently records a send boundary. The possible-send phase must
/// be persisted before the first full program upload, even if it later fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavePhase {
    PreSend,
    SendPossible,
}

impl SavePhase {
    pub fn code(self) -> &'static str {
        match self {
            Self::PreSend => "pre_send",
            Self::SendPossible => "send_possible",
        }
    }
}

#[derive(Clone, Debug)]
pub struct StepRecord {
    pub id: StepId,
    pub title: String,
    pub reused: bool,
    pub outcome: Outcome,
    /// UTC start, `YYYY-MM-DD HH:MM:SS UTC`.
    pub started: Option<String>,
    pub attempt: Option<usize>,
    pub duration_ms: Option<u128>,
    pub save_phase: Option<SavePhase>,
    /// Questions and the person's answers, in order.
    pub answers: Vec<(String, String)>,
    pub look: Option<LookCheck>,
    /// Plain observations selected for the on-screen and terminal result.
    pub result: Vec<String>,
    /// Full observations, including byte details, for the report.
    pub evidence: Vec<String>,
    pub comparisons: Vec<super::differences::Comparison>,
    pub imported_technical: Vec<String>,
    pub imported_message_count: Option<usize>,
    /// Every message sent and received, with link time stamps.
    pub messages: Vec<MessageLine>,
    pub retries: u32,
    /// Label and SHA-256 (hex).
    pub checksums: Vec<(String, String)>,
    /// Known-cause hints for this step's failure.
    pub hints: Vec<String>,
}

impl StepRecord {
    pub fn new(id: StepId) -> Self {
        Self {
            id,
            title: catalogue::step(id).map_or("?", |s| s.title).into(),
            reused: false,
            outcome: Outcome::NotReached,
            started: None,
            attempt: None,
            duration_ms: None,
            save_phase: None,
            answers: Vec::new(),
            look: None,
            result: Vec::new(),
            evidence: Vec::new(),
            comparisons: Vec::new(),
            imported_technical: Vec::new(),
            imported_message_count: None,
            messages: Vec::new(),
            retries: 0,
            checksums: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn note(&mut self, line: impl Into<String>) {
        self.evidence.push(line.into());
    }

    /// Record an observation in both the result and the full report.
    pub fn show(&mut self, line: impl Into<String>) {
        let line = line.into();
        self.result.push(line.clone());
        self.note(line);
    }

    pub fn result_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.result.iter().map(|line| plain_result(line)).collect();
        if let Some(reason) = self.outcome.reason() {
            lines.push(plain_result(reason));
        }
        lines.extend(
            self.comparisons.iter().map(|comparison| {
                format!("{} · {}", comparison.severity.label(), comparison.summary)
            }),
        );
        if matches!(self.outcome, Outcome::Failed(_))
            && !self
                .comparisons
                .iter()
                .any(|comparison| comparison.severity == super::differences::Severity::Stop)
        {
            lines.push("Stops the test".into());
        }
        lines.extend(
            self.hints
                .iter()
                .map(|hint| format!("Hint: {}", plain_result(hint))),
        );
        if let Some(look) = &self.look {
            if look.different.is_some() {
                lines.push(look.report_line());
            } else if self.outcome.passed() {
                lines.push(format!("{} Expected: {:?}.", look.prompt, look.expected));
            }
        }
        lines
    }

    pub fn evidence_folder(&self, base: &str) -> String {
        match self.attempt {
            Some(number) if number > 1 => format!("{base}-attempt-{number}"),
            _ => base.into(),
        }
    }

    pub fn message_count(&self) -> usize {
        self.imported_message_count.unwrap_or(self.messages.len())
    }

    pub fn technical_lines(&self) -> impl Iterator<Item = String> + '_ {
        self.imported_technical
            .iter()
            .cloned()
            .chain(self.imported_technical.is_empty().then(|| {
                format!(
                    "Messages: {} · Retries: {} · Missing bytes in retried replies: {}",
                    self.message_count(),
                    self.retries,
                    super::variax_reads::lost_bytes(&self.messages)
                )
            }))
            .chain(self.messages.iter().filter_map(|line| {
                line.split_once("s Retrying PODxt ")
                    .map(|(_, reason)| format!("Retrying PODxt {reason}"))
            }))
            .chain(self.comparisons.iter().flat_map(|comparison| {
                std::iter::once(format!(
                    "{} · {}",
                    comparison.severity.code(),
                    comparison.summary
                ))
                .chain(comparison.details.iter().cloned())
            }))
    }

    pub fn can_retry_save_preflight(&self) -> bool {
        !self.reused
            && self.id == super::variax_save::OWN_BYTES_STEP
            && self.started.is_some()
            && self.attempt.is_some_and(|attempt| attempt > 0)
            && self.duration_ms.is_some()
            && self.save_phase == Some(SavePhase::PreSend)
            && matches!(&self.outcome, Outcome::Failed(reason)
                if reason == line6_devices::family::variax::procedures::SAVE_PREFLIGHT_REOPEN_EXHAUSTED)
    }

    pub fn can_repeat(&self) -> bool {
        if self.reused || self.attempt.is_none() || self.started.is_none() {
            return false;
        }
        self.can_retry_save_preflight()
            || catalogue::step(self.id).is_some_and(|step| step.risk != catalogue::Risk::Save)
                && matches!(
                    self.outcome,
                    Outcome::Passed | Outcome::Failed(_) | Outcome::SkippedByPerson
                )
    }

    pub fn look_at(&mut self, prompt: &'static str, expected: impl Into<String>) {
        self.look = Some(LookCheck {
            prompt,
            expected: expected.into(),
            different: None,
        });
    }

    pub fn checksum(&mut self, label: impl Into<String>, bytes: &[u8]) -> String {
        let hash = sha256_hex(bytes);
        self.checksums.push((label.into(), hash.clone()));
        hash
    }
}

/// Keep protocol detail in the report, including when a lower-level error
/// carries a byte sequence or a command number in its text.
pub fn plain_result(text: &str) -> String {
    fn hex(text: &str) -> bool {
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_hexdigit())
    }
    fn clean(word: &str) -> &str {
        word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ':')
            .trim_matches(':')
    }
    fn byte(word: &str) -> bool {
        let word = clean(word);
        word.len() == 2 && hex(word)
    }
    let mut output = Vec::new();
    let mut words = text.split_whitespace().peekable();
    while let Some(word) = words.next() {
        let key = clean(word);
        if byte(word) && words.clone().take(2).count() == 2 && words.clone().take(2).all(byte) {
            while words.next_if(|word| byte(word)).is_some() {}
            output.push("[bytes in report]");
        } else if key
            .strip_prefix("0x")
            .or_else(|| key.strip_prefix("0X"))
            .is_some_and(hex)
            || key
                .split_once(':')
                .is_some_and(|(a, b)| a.len() == 4 && b.len() == 4 && hex(a) && hex(b))
            || key.eq_ignore_ascii_case("f7")
        {
            output.push("[device code in report]");
        } else if key.len() > 16 && hex(key) {
            output.push("[hash in report]");
        } else {
            output.push(word);
        }
    }
    output.join(" ")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug)]
pub struct RunRecord {
    pub mode: Mode,
    pub app_version: &'static str,
    pub git_commit: Option<&'static str>,
    pub catalogue_version: &'static str,
    pub started: String,
    pub start_notice_accepted: bool,
    /// Section title and `name: value` lines (filled by steps 0.1 and 0.2).
    pub computer: Vec<(String, String)>,
    pub devices: Vec<(String, String)>,
    pub steps: Vec<StepRecord>,
    pub prior_attempts: Vec<StepRecord>,
    pub continued_from: Option<String>,
    pub continuation: Vec<String>,
    pub pre_write_recovery: bool,
    pub continuation_pending: bool,
    pub blocked_resume: Option<super::resume::BlockedResume>,
    pub resume_cleanup: Option<super::resume::ResumeCleanup>,
    /// Final statement about the rig's state (from step 6.2).
    pub rig_state: String,
    pub rig_description: String,
    pub finished: bool,
    pub stopped_at: Option<StepId>,
    /// The step running right now (written before it starts).
    pub current: Option<StepId>,
}

/// Recorded by `scripts/rig-check-package.sh` at build time, if used.
pub const GIT_COMMIT: Option<&str> = option_env!("RIG_CHECK_GIT_COMMIT");
pub const RUSTC_VERSION: Option<&str> = option_env!("RIG_CHECK_RUSTC_VERSION");

impl RunRecord {
    pub fn new(mode: Mode, started: String) -> Self {
        Self {
            mode,
            app_version: env!("CARGO_PKG_VERSION"),
            git_commit: GIT_COMMIT,
            catalogue_version: CATALOGUE_VERSION,
            started,
            start_notice_accepted: false,
            computer: Vec::new(),
            devices: Vec::new(),
            steps: catalogue::RUN_ORDER
                .iter()
                .map(|id| StepRecord::new(*id))
                .collect(),
            prior_attempts: Vec::new(),
            continued_from: None,
            continuation: Vec::new(),
            pre_write_recovery: false,
            continuation_pending: false,
            blocked_resume: None,
            resume_cleanup: None,
            rig_state: "Not checked yet.".into(),
            rig_description: "Rig not identified yet".into(),
            finished: false,
            stopped_at: None,
            current: None,
        }
    }

    pub(super) fn resume_cleanup_error(&self) -> Option<&str> {
        match &self.resume_cleanup {
            Some(super::resume::ResumeCleanup::Failed(error)) => Some(error),
            _ => None,
        }
    }

    pub fn step(&self, id: StepId) -> &StepRecord {
        self.steps
            .iter()
            .find(|s| s.id == id)
            .expect("every catalogue step has a record")
    }

    pub fn step_mut(&mut self, id: StepId) -> &mut StepRecord {
        self.steps
            .iter_mut()
            .find(|s| s.id == id)
            .expect("every catalogue step has a record")
    }

    pub fn report_differences(&self) -> impl Iterator<Item = String> + '_ {
        self.steps.iter().flat_map(|step| {
            step.comparisons
                .iter()
                .filter(|comparison| comparison.severity == super::differences::Severity::Report)
                .map(move |comparison| format!("Step {} — {}", step.id, comparison.summary))
        })
    }

    pub fn look_mismatches(&self) -> impl Iterator<Item = String> + '_ {
        self.steps.iter().filter_map(|step| {
            let look = step.look.as_ref()?;
            look.different.as_ref()?;
            Some(format!("Step {} — {}", step.id, look.report_line()))
        })
    }

    pub fn count(&self, code: &str) -> usize {
        self.steps
            .iter()
            .filter(|s| s.outcome.code() == code)
            .count()
    }
}

/// UTC wall time as `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc_now() -> String {
    let seconds = unix_now();
    utc_text(seconds)
}

/// `YYYY-MM-DD HH:MM:SS UTC` for a Unix time.
pub fn utc_text(seconds: u64) -> String {
    line6_devices::backup::utc(seconds).1
}

/// `YYYYMMDDTHHMMSSZ`, for file names.
pub fn utc_stamp(seconds: u64) -> String {
    line6_devices::backup::utc(seconds).0
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

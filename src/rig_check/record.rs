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
    /// A step it needs did not pass.
    SkippedNeeds(String),
    NotAuthorized(String),
    NotAvailable(String),
    UnverifiedDevice(String),
    /// The person chose Stop at this step, or stopped it while it ran.
    Stopped(String),
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::NotReached => "NOT REACHED",
            Outcome::Passed => "PASSED",
            Outcome::Failed(_) => "FAILED",
            Outcome::SkippedByPerson => "SKIPPED (by you)",
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
pub struct StepRecord {
    pub id: StepId,
    pub title: &'static str,
    pub outcome: Outcome,
    /// UTC start, `YYYY-MM-DD HH:MM:SS UTC`.
    pub started: Option<String>,
    pub duration_ms: Option<u128>,
    /// Questions and the person's answers, in order.
    pub answers: Vec<(String, String)>,
    /// Plain observations selected for the on-screen and terminal result.
    pub result: Vec<String>,
    /// Full observations, including byte details, for the report.
    pub evidence: Vec<String>,
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
            title: catalogue::step(id).map_or("?", |s| s.title),
            outcome: Outcome::NotReached,
            started: None,
            duration_ms: None,
            answers: Vec::new(),
            result: Vec::new(),
            evidence: Vec::new(),
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
            self.hints
                .iter()
                .map(|hint| format!("Hint: {}", plain_result(hint))),
        );
        if self.outcome.passed() {
            let look = match self.id {
                StepId(2, 2) => "Does the PODxt Live display show the same current tone name?",
                StepId(2, 3) => {
                    "If the PODxt Live is already on this channel, does its display show the same name?"
                }
                StepId(4, 1) => "Check that the stomp LED is back to its starting state.",
                StepId(7, 3) => {
                    "Check that the PODxt Live display shows the starting channel again."
                }
                _ => return lines,
            };
            lines.push(look.into());
        }
        lines
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
    /// Final statement about the rig's state (from step 6.2).
    pub rig_state: String,
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
            rig_state: "Not checked yet.".into(),
            finished: false,
            stopped_at: None,
            current: None,
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

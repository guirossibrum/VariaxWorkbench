//! The text report (#249): rewritten atomically after every step, redacted,
//! with a machine-readable JSON block at the end.

use super::catalogue::{self, StepId};
use super::record::{RunRecord, StepRecord, unix_now, utc_stamp, utc_text};
use super::redact::Redactor;
use super::runner::ReportSink;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub const REPORT_FORMAT: u32 = 1;
pub const JSON_BEGIN: &str = "BEGIN RIG CHECK JSON";
pub const JSON_END: &str = "END RIG CHECK JSON";

fn section(out: &mut String, title: &str) {
    let _ = writeln!(out, "\n{title}\n{}", "-".repeat(title.chars().count()));
}

/// The whole report, before redaction.
pub fn render_plain(run: &RunRecord, written: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "RIG CHECK REPORT\n================");
    let _ = writeln!(out, "Report format: {REPORT_FORMAT}");
    let _ = writeln!(
        out,
        "App: rig-check {} ({})",
        run.app_version,
        run.git_commit.map_or(
            "commit not recorded at build time".to_string(),
            |c| format!("commit {c}")
        )
    );
    let _ = writeln!(out, "Catalogue: {}", run.catalogue_version);
    let _ = writeln!(out, "Mode: {}", run.mode.label());
    let _ = writeln!(out, "Started: {}", run.started);
    let _ = writeln!(out, "Last written: {written}");
    let status = if run.finished {
        "finished".to_string()
    } else if let Some(current) = run.current {
        format!(
            "in progress: step {current} was running when this was written (a crash leaves the report up to the last completed step)"
        )
    } else {
        "in progress".to_string()
    };
    let _ = writeln!(out, "Status: {status}");
    let _ = writeln!(
        out,
        "Redaction: user name, host name and home folder replaced; serial numbers hashed."
    );

    section(&mut out, "SUMMARY");
    let _ = writeln!(
        out,
        "Passed {} · Failed {} · Skipped by you {} · Skipped (tested firmware) {} · Skipped (needs) {} · Not authorized {} · Not available {} · Unverified device {} · Stopped {} · Not reached {}",
        run.count("passed"),
        run.count("failed"),
        run.count("skipped"),
        run.count("skipped_verified_firmware"),
        run.count("skipped_needs"),
        run.count("not_authorized"),
        run.count("not_available"),
        run.count("unverified_device"),
        run.count("stopped"),
        run.count("not_reached"),
    );
    if let Some(stop) = run.stopped_at {
        let _ = writeln!(out, "You stopped the run at step {stop}.");
    }
    let _ = writeln!(out, "Rig state: {}", run.rig_state);
    for step in &run.steps {
        let reason = step
            .outcome
            .reason()
            .map(|r| format!(" — {r}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "  {:<4} {:<58} {}{reason}",
            step.id.to_string(),
            step.title,
            step.outcome.label()
        );
        if let Some(note) = step.outcome.technical_note() {
            let _ = writeln!(out, "       {note}");
        }
    }

    section(&mut out, "COMPUTER");
    if run.computer.is_empty() {
        let _ = writeln!(out, "(not collected: step 0.1/0.2 did not run)");
    }
    for (name, value) in &run.computer {
        let _ = writeln!(out, "{name}: {value}");
    }

    section(&mut out, "DEVICES");
    if run.devices.is_empty() {
        let _ = writeln!(out, "(no device identified)");
    }
    for (name, value) in &run.devices {
        let _ = writeln!(out, "{name}: {value}");
    }

    section(&mut out, "STEPS");
    for step in run.steps.iter().filter(|s| s.started.is_some()) {
        render_step(&mut out, step);
    }

    section(&mut out, "MACHINE-READABLE");
    let _ = writeln!(out, "{JSON_BEGIN}");
    let _ = writeln!(out, "{}", json(run, written));
    let _ = writeln!(out, "{JSON_END}");
    out
}

fn render_step(out: &mut String, step: &StepRecord) {
    let _ = writeln!(
        out,
        "\n[{}] {} — {}",
        step.id,
        step.title,
        step.outcome.label()
    );
    if let Some(reason) = step.outcome.reason() {
        let _ = writeln!(out, "  Reason: {reason}");
    }
    if let Some(def) = catalogue::step(step.id) {
        let _ = writeln!(
            out,
            "  Gate: {} · Risk: {}",
            def.gate_labels(),
            def.risk.label()
        );
    }
    let _ = writeln!(
        out,
        "  Started: {} · Duration: {} ms · Retries: {}",
        step.started.as_deref().unwrap_or("-"),
        step.duration_ms.unwrap_or(0),
        step.retries
    );
    if !step.answers.is_empty() {
        let _ = writeln!(out, "  Your answers:");
        for (question, answer) in &step.answers {
            let _ = writeln!(out, "    Q: {question}\n    A: {answer}");
        }
    }
    if !step.evidence.is_empty() {
        let _ = writeln!(out, "  Results:");
        for line in &step.evidence {
            let _ = writeln!(out, "    {line}");
        }
    }
    if !step.checksums.is_empty() {
        let _ = writeln!(out, "  Checksums (SHA-256):");
        for (label, hash) in &step.checksums {
            let _ = writeln!(out, "    {hash}  {label}");
        }
    }
    for hint in &step.hints {
        let _ = writeln!(out, "  Hint: {hint}");
    }
    if !step.messages.is_empty() {
        let _ = writeln!(
            out,
            "  Messages ({}; seconds since the link opened, OUT = sent, IN = received):",
            step.messages.len()
        );
        for line in &step.messages {
            let _ = writeln!(out, "    {line}");
        }
    }
}

pub fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_pairs(pairs: &[(String, String)]) -> String {
    let items: Vec<String> = pairs
        .iter()
        .map(|(k, v)| format!("[{},{}]", json_string(k), json_string(v)))
        .collect();
    format!("[{}]", items.join(","))
}

fn json_list(items: &[String]) -> String {
    let items: Vec<String> = items.iter().map(|s| json_string(s)).collect();
    format!("[{}]", items.join(","))
}

/// One compact JSON object describing the run (no message bytes; those are
/// in the text above).
pub fn json(run: &RunRecord, written: &str) -> String {
    let steps: Vec<String> = run
        .steps
        .iter()
        .map(|step| {
            format!(
                "{{\"id\":{},\"title\":{},\"outcome\":{},\"reason\":{},\"technical_note\":{},\"started\":{},\"duration_ms\":{},\"retries\":{},\"messages\":{},\"answers\":{},\"checksums\":{},\"results\":{},\"hints\":{}}}",
                json_string(&step.id.to_string()),
                json_string(step.title),
                json_string(step.outcome.code()),
                step.outcome.reason().map_or("null".into(), json_string),
                step.outcome.technical_note().map_or("null".into(), json_string),
                step.started.as_deref().map_or("null".into(), json_string),
                step.duration_ms.map_or("null".into(), |d| d.to_string()),
                step.retries,
                step.messages.len(),
                json_pairs(&step.answers),
                json_pairs(&step.checksums),
                json_list(&step.evidence),
                json_list(&step.hints),
            )
        })
        .collect();
    format!(
        "{{\"report_format\":{REPORT_FORMAT},\"app_version\":{},\"git_commit\":{},\"catalogue_version\":{},\"mode\":{},\"started\":{},\"written\":{},\"finished\":{},\"stopped_at\":{},\"rig_state\":{},\"computer\":{},\"devices\":{},\"steps\":[{}]}}",
        json_string(run.app_version),
        run.git_commit.map_or("null".into(), json_string),
        json_string(run.catalogue_version),
        json_string(run.mode.label()),
        json_string(&run.started),
        json_string(written),
        run.finished,
        run.stopped_at
            .map_or("null".into(), |id| json_string(&id.to_string())),
        json_string(&run.rig_state),
        json_pairs(&run.computer),
        json_pairs(&run.devices),
        steps.join(",")
    )
}

/// The report as it is saved and shown: redacted.
pub fn render(run: &RunRecord, redactor: &Redactor, written: &str) -> String {
    redactor.apply(&render_plain(run, written))
}

/// Writes the report into its own file after every update. The first write
/// creates a new file (never overwriting); later writes replace that file
/// atomically, so a crash leaves the last complete version.
pub struct ReportFile {
    pub path: PathBuf,
    pub redactor: Redactor,
    /// Fixed time for tests; `None` uses the clock.
    pub clock: Option<u64>,
    pub last: String,
}

impl ReportFile {
    /// Reserve `rig-check-<UTC>.txt` (or `-2`, `-3`, ...) in `dir`.
    pub fn create(dir: &Path, redactor: Redactor) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let stamp = utc_stamp(unix_now());
        for n in 1..1000 {
            let name = if n == 1 {
                format!("rig-check-{stamp}.txt")
            } else {
                format!("rig-check-{stamp}-{n}.txt")
            };
            let path = dir.join(name);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => {
                    return Ok(Self {
                        path,
                        redactor,
                        clock: None,
                        last: String::new(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Err("Too many reports with the same time stamp.".into())
    }

    fn replace(&self, text: &str) -> Result<(), String> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
        temporary
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist(&self.path)
            .map_err(|e| format!("{}: {}", self.path.display(), e.error))?;
        Ok(())
    }
}

impl ReportSink for ReportFile {
    fn update(&mut self, run: &RunRecord) -> Result<(), String> {
        let written = utc_text(self.clock.unwrap_or_else(unix_now));
        let text = render(run, &self.redactor, &written);
        self.replace(&text)?;
        self.last = text;
        Ok(())
    }
}

/// Save a copy of the previewed report to a new file (never overwrites).
pub fn save_copy(text: &str, path: &Path) -> Result<(), String> {
    crate::shared::document::save_new(path, text.as_bytes())
}

/// The JSON block of a saved report, if present and complete.
pub fn json_block(text: &str) -> Option<&str> {
    let start = text.find(JSON_BEGIN)? + JSON_BEGIN.len();
    let end = text[start..].find(JSON_END)? + start;
    Some(text[start..end].trim())
}

/// Which step was running when the report was last written, if any (used to
/// offer crash recovery, #261).
pub fn interrupted_step(text: &str) -> Option<StepId> {
    let line = text
        .lines()
        .find(|l| l.starts_with("Status: in progress: step "))?;
    let id = line
        .trim_start_matches("Status: in progress: step ")
        .split_whitespace()
        .next()?;
    StepId::parse(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::ReportSink;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    fn redactor() -> Redactor {
        let mut r = Redactor {
            user: Some("rigowner".into()),
            host: Some("studio-box".into()),
            home: Some("/home/rigowner".into()),
            serials: Vec::new(),
        };
        r.add_serial("SN0042XYZ");
        r
    }

    /// Records the text after every update, like a crash at each point.
    struct Snapshots(ReportFile, Vec<String>);

    impl ReportSink for Snapshots {
        fn update(&mut self, run: &RunRecord) -> Result<(), String> {
            self.0.update(run)?;
            self.1.push(std::fs::read_to_string(&self.0.path).unwrap());
            Ok(())
        }
    }

    #[test]
    fn the_report_is_rewritten_after_every_step_and_always_complete() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = ReportFile::create(dir.path(), redactor()).unwrap();
        file.clock = Some(1_764_000_000);
        let mut sink = Snapshots(file, Vec::new());
        let mut person = Script::run_all();
        let mut data = RunData::default();
        data.computer.push((
            "Host".into(),
            "studio-box (user rigowner, home /home/rigowner, USB serial SN0042XYZ)".into(),
        ));
        let (run, _, _) = run_practice(Faults::default(), &mut person, &mut sink, data);
        assert!(sink.1.len() > 30, "written after each step");
        for (n, snapshot) in sink.1.iter().enumerate() {
            assert!(snapshot.starts_with("RIG CHECK REPORT"), "{n}");
            let json = json_block(snapshot).unwrap_or_else(|| panic!("JSON in snapshot {n}"));
            assert!(json.starts_with('{') && json.ends_with('}'));
            assert!(snapshot.contains(&format!("Catalogue: {}", catalogue::CATALOGUE_VERSION)));
        }
        // An interrupted run says which step was running.
        assert!(
            sink.1
                .iter()
                .any(|s| interrupted_step(s) == Some(StepId(3, 3)))
        );
        let last = sink.1.last().unwrap();
        assert!(last.contains("Status: finished"));
        assert!(last.contains("[3.3] Variax full current program (35 reads) — PASSED"));
        assert!(
            last.contains("OUT f0 00 01 0c 07 00 3c"),
            "hex messages are logged"
        );
        assert!(run.step(StepId(2, 2)).outcome == Outcome::Passed);
        // Atomic rewrites leave no temporary files behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn no_user_host_home_or_serial_appears_in_the_report() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ReportFile::create(dir.path(), redactor()).unwrap();
        let mut person = Script::run_all();
        let mut data = RunData::default();
        data.computer
            .push(("Host name".into(), "studio-box".into()));
        data.computer.push(("User".into(), "rigowner".into()));
        data.computer.push((
            "Report folder".into(),
            "/home/rigowner/rig-check-output".into(),
        ));
        data.devices
            .push(("PODxt USB serial".into(), "SN0042XYZ".into()));
        run_practice(Faults::default(), &mut person, &mut sink, data);
        let text = std::fs::read_to_string(&sink.path).unwrap().to_lowercase();
        for secret in ["rigowner", "studio-box", "/home/rigowner", "sn0042xyz"] {
            assert!(!text.contains(secret), "{secret} leaked");
        }
        assert!(
            text.contains("<host>")
                && text.contains("<user>")
                && text.contains("~/rig-check-output")
        );
        assert!(text.contains(&Redactor::serial_tag("SN0042XYZ")));
    }

    #[test]
    fn a_copy_is_saved_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copy.txt");
        save_copy("one", &path).unwrap();
        assert!(save_copy("two", &path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
        let first = ReportFile::create(dir.path(), Redactor::default()).unwrap();
        let second = ReportFile::create(dir.path(), Redactor::default()).unwrap();
        assert_ne!(first.path, second.path);
    }

    #[test]
    fn backups_go_to_the_separate_evidence_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ReportFile::create(dir.path(), Redactor::default()).unwrap();
        let data = RunData {
            evidence_dir: Some(dir.path().join("evidence")),
            ..RunData::default()
        };
        let mut person = Script::run_all();
        run_practice(Faults::default(), &mut person, &mut sink, data);
        let count = |sub: &str| {
            std::fs::read_dir(dir.path().join("evidence").join(sub))
                .unwrap()
                .count()
        };
        assert_eq!(count("variax-library"), 61);
        assert_eq!(count("podxt-library/tones"), 128);
        assert!(
            dir.path()
                .join("evidence/podxt-library/MANIFEST.sha256")
                .exists()
        );
        let text = std::fs::read_to_string(&sink.path).unwrap();
        assert!(text.contains("Saved 60 programs to the evidence folder"));
        assert!(
            !text.contains("evidence/variax-library/"),
            "files stay out of the report"
        );
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_string("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}

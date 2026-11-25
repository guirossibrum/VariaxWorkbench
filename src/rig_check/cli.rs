//! Terminal front end. Every answer defaults to "do not run".

use super::catalogue::{SaveNotice, Step, StepId};
use super::record::{Mode, StepRecord};
use super::runner::{Answer, Decision, Operator, Question, Retry};
use std::io::{BufRead, Write};

pub struct Terminal<R, W> {
    input: R,
    output: W,
}

impl<R: BufRead, W: Write> Terminal<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self { input, output }
    }

    fn say(&mut self, text: &str) {
        let _ = writeln!(self.output, "{text}");
        let _ = self.output.flush();
    }

    /// One trimmed line; end of input counts as an empty answer (the default).
    fn line(&mut self) -> String {
        let mut text = String::new();
        if self.input.read_line(&mut text).unwrap_or(0) == 0 {
            return String::new();
        }
        text.trim().to_owned()
    }

    pub fn into_output(self) -> W {
        self.output
    }
}

pub fn card_text(step: &Step, mode: Mode) -> String {
    let needs = if step.needs.is_empty() {
        "nothing".to_string()
    } else {
        step.needs
            .iter()
            .map(StepId::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "\n=== Step {id}: {title} ===\nWhat it does: {does}\nYou do: {you}\nSEE:  {see}\nHEAR: {hear}\nRisk: {risk} · Gate: {gates} · Needs: {needs} · Mode: {mode}\nRecords: {evidence}",
        id = step.id,
        title = step.title,
        does = step.does,
        you = step.you_do,
        see = step.see.to_uppercase(),
        hear = step.hear.to_uppercase(),
        risk = step.risk.label(),
        gates = step.gate_labels(),
        evidence = step.evidence,
        mode = mode.label(),
    )
}

impl<R: BufRead, W: Write> Operator for Terminal<R, W> {
    fn accept_start(&mut self, notice: &str) -> bool {
        self.say(notice);
        self.say("\nType accept to start. Anything else quits. [Quit]");
        matches!(self.line().to_ascii_lowercase().as_str(), "accept" | "a")
    }

    fn confirm(&mut self, step: &Step, mode: Mode) -> Decision {
        let card = card_text(step, mode);
        self.say(&card);
        self.say("[r] Run   [s] Skip   [q] Stop the run.   Enter = Skip (do not run)");
        match self.line().to_ascii_lowercase().as_str() {
            "r" | "run" => Decision::Run,
            "q" | "stop" => Decision::Stop,
            _ => Decision::Skip,
        }
    }

    fn confirm_save(&mut self, notice: &SaveNotice) -> Option<String> {
        self.say(&format!("\n!!! {}", notice.text()));
        self.say("Type the confirmation exactly, or press Enter to skip.");
        let typed = self.line();
        (!typed.is_empty()).then_some(typed)
    }

    fn instruct(&mut self, step: StepId, see: &str, hear: &str) {
        self.say(&format!(
            "\n>>> Step {step}\n>>> SEE:  {}\n>>> HEAR: {}",
            see.to_uppercase(),
            hear.to_uppercase()
        ));
    }

    fn not_seen(&mut self, step: StepId, what: &str) -> Retry {
        self.say(&format!(
            "Step {step}: didn't see it ({what}). [t] Try again  [s] Skip.  Enter = Skip"
        ));
        match self.line().to_ascii_lowercase().as_str() {
            "t" | "try" | "try again" => Retry::TryAgain,
            _ => Retry::Skip,
        }
    }

    fn ask(&mut self, question: &Question) -> (Answer, String) {
        self.say(&format!("\nStep {}: {}", question.step, question.text));
        for line in &question.verified_by_bytes {
            self.say(&format!("  (verified by byte check: {line})"));
        }
        self.say("[y] Yes  [n] No  [?] Not sure.  Enter = Not sure");
        let answer = match self.line().to_ascii_lowercase().as_str() {
            "y" | "yes" => Answer::Yes,
            "n" | "no" => Answer::No,
            _ => Answer::NotSure,
        };
        self.say("Note (optional, Enter for none):");
        (answer, self.line())
    }

    fn progress(&mut self, text: &str) {
        self.say(text);
    }

    fn finished(&mut self, record: &StepRecord) {
        let reason = record
            .outcome
            .reason()
            .map(|r| format!(": {r}"))
            .unwrap_or_default();
        self.say(&format!(
            "--- {} {} — {}{reason}",
            record.id,
            record.title,
            record.outcome.label()
        ));
        for line in record.evidence.iter().take(12) {
            self.say(&format!("    {line}"));
        }
        for hint in &record.hints {
            self.say(&format!("    Hint: {hint}"));
        }
    }

    fn stop_requested(&self) -> bool {
        false
    }
}

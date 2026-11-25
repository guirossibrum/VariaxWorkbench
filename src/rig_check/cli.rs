//! Terminal front end. Every answer defaults to "do not run".

use super::catalogue::{SaveNotice, Step, StepId};
use super::record::{Mode, StepRecord, plain_result};
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
        self.say(&plain_result(text));
    }

    fn finished(&mut self, record: &StepRecord) -> bool {
        self.say(&format!(
            "--- {} {} — {}",
            record.id,
            record.title,
            record.outcome.label()
        ));
        for line in record.result_lines() {
            self.say(&format!("    {line}"));
        }
        if record.started.is_none() {
            return true;
        }
        loop {
            self.say("[c] Continue   [q] Stop.   Enter = Continue");
            let mut response = String::new();
            if self.input.read_line(&mut response).unwrap_or(0) == 0 {
                return false;
            }
            match response.trim().to_ascii_lowercase().as_str() {
                "" | "c" | "continue" => return true,
                "q" | "stop" => return false,
                _ => {}
            }
        }
    }

    fn stop_requested(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::record::Outcome;

    #[test]
    fn terminal_result_waits_for_continue_and_keeps_raw_failure_evidence_in_the_report() {
        let mut record = StepRecord::new(StepId(2, 3));
        record.started = Some("practice".into());
        record.outcome = Outcome::Failed("No reply to 0x3C ending in F7.".into());
        record.show("Channel 1A: Practice 1A");
        record.note("Identity reply: f0 00 01 0c 03 f7");
        record.hints.push("Check the cable.".into());

        let mut terminal = Terminal::new(&b"r\nc\n"[..], Vec::new());
        assert!(terminal.finished(&record));
        assert!(!terminal.finished(&record), "end of input stops");
        let text = String::from_utf8(terminal.into_output()).unwrap();
        for line in record.result_lines() {
            assert!(text.contains(&line));
        }
        assert!(text.contains("FAILED") && text.contains("Hint: Check the cable."));
        assert!(!text.contains("0x3C") && !text.contains("f0 00 01") && !text.contains("F7"));
        assert_eq!(text.matches("[c] Continue").count(), 3);
    }
}

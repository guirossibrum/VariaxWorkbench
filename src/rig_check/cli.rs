//! Terminal front end. Every answer defaults to "do not run".

use super::catalogue::{START_NOTICE, SaveNotice, Step, StepId};
use super::history::{self, Review};
use super::record::{Mode, StepRecord, plain_result};
use super::runner::{Answer, Decision, Operator, Progress, Question, ResultDecision, Retry};
use std::io::{BufRead, Write};

pub struct Terminal<R, W> {
    input: R,
    output: W,
    results: Vec<StepRecord>,
    start_accepted: bool,
}

impl<R: BufRead, W: Write> Terminal<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            input,
            output,
            results: Vec::new(),
            start_accepted: false,
        }
    }

    fn say(&mut self, text: &str) {
        let _ = writeln!(self.output, "{text}");
        let _ = self.output.flush();
    }

    fn response(&mut self) -> Option<String> {
        self.raw_response().map(|text| text.trim().to_owned())
    }

    fn raw_response(&mut self) -> Option<String> {
        let mut text = String::new();
        if self.input.read_line(&mut text).ok()? == 0 {
            return None;
        }
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        Some(text)
    }

    /// One trimmed line; end of input counts as an empty answer (the default).
    fn line(&mut self) -> String {
        self.response().unwrap_or_default()
    }

    fn show_result(&mut self, record: &StepRecord) {
        self.say(&format!(
            "--- {} {} — {}",
            record.id,
            record.title,
            record.outcome.label()
        ));
        for line in record.result_lines() {
            self.say(&format!("    {line}"));
        }
    }

    fn review(&mut self, mut view: Review) {
        loop {
            let previous = match view {
                Review::Start => {
                    self.say("Start notice (accepted)");
                    self.say(START_NOTICE);
                    None
                }
                Review::Result(id) => {
                    let Some(record) = self.results.iter().find(|record| record.id == id).cloned()
                    else {
                        break;
                    };
                    self.show_result(&record);
                    history::back(&self.results, Some(id), self.start_accepted)
                }
            };
            if previous.is_some() {
                self.say("[b] Back   [c] Continue / Return to current step.   Enter = Return");
            } else {
                self.say("[c] Continue / Return to current step.   Enter = Return");
            }
            match self.line().to_ascii_lowercase().as_str() {
                "b" | "back" => {
                    if let Some(previous) = previous {
                        view = previous;
                    }
                }
                "" | "c" | "continue" | "return" => break,
                _ => {}
            }
        }
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
        let accepted = matches!(self.line().to_ascii_lowercase().as_str(), "accept" | "a");
        self.start_accepted |= accepted && notice == START_NOTICE;
        accepted
    }

    fn confirm(&mut self, step: &Step, mode: Mode) -> Decision {
        let card = card_text(step, mode);
        let back = history::back(&self.results, None, self.start_accepted);
        loop {
            self.say(&card);
            let label = if back.is_some() { "[b] Back   " } else { "" };
            self.say(&format!(
                "{label}[r] Run   [s] Skip   [q] Stop the run.   Enter = Skip (do not run)"
            ));
            match self.line().to_ascii_lowercase().as_str() {
                "r" | "run" => return Decision::Run,
                "q" | "stop" => return Decision::Stop,
                "b" | "back" => {
                    if let Some(back) = back {
                        self.review(back);
                    }
                }
                _ => return Decision::Skip,
            }
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

    fn progress(&mut self, progress: Progress) {
        let action = plain_result(&progress.action);
        if let Some((completed, total, units)) = progress.count {
            self.say(&format!("{completed} of {total} {units} · {action}"));
        } else {
            self.say(&action);
        }
    }

    fn finished(&mut self, record: &StepRecord) -> ResultDecision {
        self.show_result(record);
        self.results.retain(|previous| previous.id != record.id);
        self.results.push(record.clone());
        if record.started.is_none() {
            return ResultDecision::Continue;
        }
        let look = record.outcome.passed() && record.look.is_some();
        let back = history::back(&self.results, Some(record.id), self.start_accepted);
        loop {
            let label = if back.is_some() { "[b] Back   " } else { "" };
            let different = if look { "[d] Different…   " } else { "" };
            self.say(&format!(
                "{label}[c] Continue   {different}[q] Stop.   Enter = Continue"
            ));
            let Some(response) = self.response() else {
                return ResultDecision::Stop;
            };
            match response.to_ascii_lowercase().as_str() {
                "" | "c" | "continue" => return ResultDecision::Continue,
                "q" | "stop" => return ResultDecision::Stop,
                "d" | "different" if look => {
                    self.say("Type exactly what the device shows. Enter alone goes back.");
                    match self.raw_response() {
                        Some(text) if !text.is_empty() => return ResultDecision::Different(text),
                        Some(_) => {}
                        None => return ResultDecision::Stop,
                    }
                }
                "b" | "back" => {
                    if let Some(back) = back {
                        self.review(back);
                        self.show_result(record);
                    }
                }
                _ => {}
            }
        }
    }

    fn result_updated(&mut self, record: &StepRecord) {
        if let Some(previous) = self
            .results
            .iter_mut()
            .find(|previous| previous.id == record.id)
        {
            *previous = record.clone();
        }
        self.show_result(record);
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
    fn different_accepts_exact_display_text_after_d() {
        let mut record = StepRecord::new(StepId(2, 2));
        record.started = Some("practice".into());
        record.outcome = Outcome::Passed;
        record.look_at("Compare the display.", "Practice 5B");
        let mut terminal = Terminal::new(&b"d\n  Other name  \r\n"[..], Vec::new());
        assert_eq!(
            terminal.finished(&record),
            ResultDecision::Different("  Other name  ".into())
        );
        let output = String::from_utf8(terminal.into_output()).unwrap();
        assert!(output.contains("[d] Different…"));
        assert!(output.contains("Type exactly what the device shows."));
    }

    #[test]
    fn terminal_result_waits_for_continue_and_keeps_raw_failure_evidence_in_the_report() {
        let mut record = StepRecord::new(StepId(2, 3));
        record.started = Some("practice".into());
        record.outcome = Outcome::Failed("No reply to 0x3C ending in F7.".into());
        record.show("Channel 1A: Practice 1A");
        record.note("Identity reply: f0 00 01 0c 03 f7");
        record.hints.push("Check the cable.".into());

        let mut terminal = Terminal::new(&b"r\nc\n"[..], Vec::new());
        assert_eq!(terminal.finished(&record), ResultDecision::Continue);
        assert_eq!(
            terminal.finished(&record),
            ResultDecision::Stop,
            "end of input stops"
        );
        let text = String::from_utf8(terminal.into_output()).unwrap();
        for line in record.result_lines() {
            assert!(text.contains(&line));
        }
        assert!(text.contains("FAILED") && text.contains("Hint: Check the cable."));
        assert!(!text.contains("0x3C") && !text.contains("f0 00 01") && !text.contains("F7"));
        assert_eq!(text.matches("[c] Continue").count(), 3);
    }
    #[test]
    fn terminal_back_replays_history_without_sending_messages_and_resumes_the_same_prompt() {
        use crate::rig_check::{
            port::PracticePort,
            practice::{Faults, SimRig},
            run_on,
            runner::NoReport,
            steps::RunData,
        };
        use std::{
            cell::RefCell,
            io::{Cursor, Read},
            rc::Rc,
        };

        struct CountedInput<'a> {
            input: Cursor<Vec<u8>>,
            rig: Rc<RefCell<SimRig>>,
            counts: &'a mut Vec<usize>,
        }
        impl Read for CountedInput<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.input.read(out)
            }
        }
        impl BufRead for CountedInput<'_> {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                self.input.fill_buf()
            }
            fn consume(&mut self, amount: usize) {
                self.input.consume(amount);
            }
            fn read_line(&mut self, line: &mut String) -> std::io::Result<usize> {
                self.counts.push(self.rig.borrow().written.len());
                self.input.read_line(line)
            }
        }

        let mut commands = vec!["accept", "b", "c"];
        for _ in 0..4 {
            commands.extend(["r", "c"]);
        }
        commands.push("r"); // 2.1 actually sends an identity request.
        let start_history = commands.len();
        commands.extend(["b", "b", "c", "c", "b", "c", "q", "s"]);
        let rig = SimRig::shared(Faults::default());
        let mut counts = Vec::new();
        let mut port = PracticePort::new(rig.clone());
        let mut terminal = Terminal::new(
            CountedInput {
                input: Cursor::new((commands.join("\n") + "\n").into_bytes()),
                rig,
                counts: &mut counts,
            },
            Vec::new(),
        );
        let (run, _) = run_on(&mut port, &mut terminal, &mut NoReport, RunData::default());
        let output = String::from_utf8(terminal.into_output()).unwrap();
        assert!(output.contains("Start notice (accepted)") && output.contains("[b] Back"));
        assert!(output.contains("Return to current step"));
        assert_eq!(output.matches("=== Step 1.2:").count(), 2);
        assert_eq!(run.stopped_at, Some(StepId(1, 2)));
        assert!(run.step(StepId(2, 1)).outcome.passed());
        assert_eq!(counts.len(), commands.len());
        assert!(counts[start_history] > 0);
        assert!(
            counts[start_history..]
                .iter()
                .all(|&count| count == counts[start_history])
        );
    }
}

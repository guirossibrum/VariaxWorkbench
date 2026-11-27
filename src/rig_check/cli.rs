//! Terminal front end. Every answer defaults to "do not run".

use super::catalogue::{SaveNotice, Step, StepId, accepted_start_notice};
use super::history::{self, Review};
use super::record::{Mode, StepRecord, plain_result};
use super::resume::BlockedResume;
use super::runner::{Answer, Decision, Operator, Progress, Question, ResultDecision, Retry};
use std::cell::Cell;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub struct Terminal<R, W> {
    input: R,
    output: W,
    results: Vec<StepRecord>,
    start_accepted: Option<&'static str>,
    keyboard: Option<Rc<Receiver<String>>>,
    stop: Rc<Cell<bool>>,
}

impl<R: BufRead, W: Write> Terminal<R, W> {
    pub fn new(input: R, output: W) -> Self {
        Self {
            input,
            output,
            results: Vec::new(),
            start_accepted: None,
            keyboard: None,
            stop: Rc::new(Cell::new(false)),
        }
    }

    fn say(&mut self, text: &str) {
        let _ = writeln!(self.output, "{text}");
        let _ = self.output.flush();
    }

    fn response(&mut self) -> Option<String> {
        self.raw_response().map(|text| text.trim().to_owned())
    }

    pub fn prompt_for_report_copy(&mut self) -> String {
        self.say("Save a copy of the report to (file path, Enter to skip):");
        self.line()
    }

    fn raw_response(&mut self) -> Option<String> {
        if let Some(keyboard) = &self.keyboard {
            return keyboard.recv().ok();
        }
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
                    if let Some(notice) = self.start_accepted {
                        self.say(notice);
                    }
                    None
                }
                Review::Result(id) => {
                    let Some(record) = self.results.iter().find(|record| record.id == id).cloned()
                    else {
                        break;
                    };
                    self.show_result(&record);
                    history::back(&self.results, Some(id), self.start_accepted.is_some())
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

impl<W: Write> Terminal<io::Empty, W> {
    /// Read standard input while the main thread waits for device activity.
    pub fn from_stdin(output: W) -> Self {
        let (send, keys) = mpsc::channel();
        std::thread::spawn(move || {
            for input in io::stdin().lock().lines() {
                let Ok(line) = input else { break };
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let mut term = Self::new(io::empty(), output);
        term.keyboard = Some(Rc::new(keys));
        term
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
        "\n=== Step {id}: {title} ===\nWhat it does: {does}\nYou do: {you}{starting_buffer}\nSEE:  {see}\nHEAR: {hear}\nRisk: {risk} · Gate: {gates} · Needs: {needs} · Mode: {mode}\nRecords: {evidence}",
        id = step.id,
        title = step.title,
        does = step.does,
        you = step.you_do,
        starting_buffer = step
            .starting_buffer_note()
            .map_or(String::new(), |note| format!("\n{note}")),
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
        if accepted && let Some(notice) = accepted_start_notice(notice) {
            self.start_accepted = Some(notice);
        }
        accepted
    }

    fn confirm(&mut self, step: &Step, mode: Mode, note: Option<String>) -> Decision {
        self.stop.set(false);
        let card = card_text(step, mode);
        let back = history::back(&self.results, None, self.start_accepted.is_some());
        loop {
            self.say(&card);
            if let Some(note) = &note {
                self.say(note);
            }
            let label = if back.is_some() { "[b] Back   " } else { "" };
            let start = if super::catalogue::asks_for_action(step.id) {
                "Start"
            } else {
                "Run"
            };
            self.say(&format!(
                "{label}[r] {start}   [s] Skip   [q] Stop the run.   Enter = Skip (do not run)"
            ));
            match self.line().to_ascii_lowercase().as_str() {
                "r" | "run" | "start" => return Decision::Run,
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

    fn prepare_action(&mut self, step: StepId, see: &str, hear: &str) -> Decision {
        self.say(&format!(
            "Step {step}: get ready.\nSEE: {see}\nHEAR: {hear}"
        ));
        self.say("Enter = OK when ready. [s] Skip   [q] Stop");
        match self.response().as_deref() {
            None | Some("q" | "stop") => Decision::Stop,
            Some("" | "ok") => Decision::Run,
            _ => Decision::Skip,
        }
    }

    fn watch_action(
        &mut self,
        _: StepId,
        what: &str,
        clock: std::sync::Arc<super::guided::ActionClock>,
    ) -> Box<dyn Fn() -> bool> {
        if clock.elapsed() < clock.ready_duration() {
            self.say(&format!(
                "Get ready. Start in {} s. Watching for {what}. Enter = Continue   [+10] More time   [q] Stop",
                super::guided::READY_DELAY.as_secs()
            ));
        }
        let keyboard = self.keyboard.clone();
        let stop = self.stop.clone();
        let advanced = Cell::new(false);
        Box::new(move || {
            if advanced.get() || stop.get() {
                return true;
            }
            let Some(keys) = &keyboard else { return false };
            match keys.try_recv() {
                Ok(line) => match line.trim().to_ascii_lowercase().as_str() {
                    "" | "c" | "continue" => advanced.set(true),
                    "q" | "stop" => stop.set(true),
                    "+10" => clock.add_ten_seconds(),
                    _ => {}
                },
                Err(TryRecvError::Disconnected) => stop.set(true),
                Err(TryRecvError::Empty) => {}
            }
            advanced.get() || stop.get()
        })
    }

    fn variax_connected(&mut self) -> bool {
        self.say("No guitar answered. Is a Variax connected by its VDI cable?");
        self.say("[n] No Variax   [t] Try again");
        matches!(self.line().to_ascii_lowercase().as_str(), "t" | "try again")
    }

    fn not_seen(&mut self, step: StepId, what: &str) -> Retry {
        self.say(&format!(
            "Step {step}: {what} [t] Try again  [s] Skip  [q] Stop. Enter = Skip"
        ));
        match self
            .response()
            .map(|answer| answer.to_ascii_lowercase())
            .as_deref()
        {
            Some("t" | "try" | "try again") => Retry::TryAgain,
            None | Some("q" | "stop") => Retry::Stop,
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

    fn blocked_resume(&mut self, blocked: &BlockedResume, cleanup_error: Option<&str>) {
        self.say(BlockedResume::TITLE);
        for line in blocked.lines() {
            self.say(&line);
        }
        if let Some(error) = cleanup_error {
            self.say(&format!("USB cleanup failed: {error}"));
            self.say(super::resume::VERIFY_USB_CLEANUP);
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
        let back = history::back(
            &self.results,
            Some(record.id),
            self.start_accepted.is_some(),
        );
        loop {
            let label = if back.is_some() { "[b] Back   " } else { "" };
            let different = if look { "[d] Different…   " } else { "" };
            let repeat = if record.can_retry_save_preflight() {
                "[r] Try again   "
            } else if record.can_repeat() {
                "[r] Repeat this step   "
            } else {
                ""
            };
            self.say(&format!(
                "{label}[c] Continue   {different}{repeat}[q] Stop.   Enter = Continue"
            ));
            let Some(response) = self.response() else {
                return ResultDecision::Stop;
            };
            match response.to_ascii_lowercase().as_str() {
                "" | "c" | "continue" => return ResultDecision::Continue,
                "q" | "stop" => return ResultDecision::Stop,
                "r" | "repeat" | "try again" if record.can_repeat() => {
                    return ResultDecision::Repeat;
                }
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

    fn stop_requested(&self) -> bool {
        self.stop.get()
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
    fn repeat_is_available_for_a_read_but_absent_for_a_save() {
        for (id, expected) in [
            (StepId(3, 3), ResultDecision::Repeat),
            (StepId(5, 1), ResultDecision::Continue),
        ] {
            let mut record = StepRecord::new(id);
            record.started = Some("practice".into());
            record.attempt = Some(1);
            record.outcome = Outcome::Passed;
            let repeatable = expected == ResultDecision::Repeat;
            let mut terminal = Terminal::new(&b"r\nc\n"[..], Vec::new());
            assert_eq!(terminal.finished(&record), expected);
            let transcript = String::from_utf8(terminal.into_output()).unwrap();
            assert_eq!(transcript.contains("Repeat this step"), repeatable);
        }
    }

    #[test]
    fn terminal_result_waits_for_continue_and_keeps_raw_failure_evidence_in_the_report() {
        let mut record = StepRecord::new(StepId(2, 3));
        record.started = Some("practice".into());
        record.outcome = Outcome::Failed("No reply to 0x3C ending in F7.".into());
        record.show("Channel 1A: Practice 1A");
        record.note("Identity reply: f0 00 01 0c 03 f7");
        record.hints.push("Check the cable.".into());

        let mut terminal = Terminal::new(&b"x\nc\n"[..], Vec::new());
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

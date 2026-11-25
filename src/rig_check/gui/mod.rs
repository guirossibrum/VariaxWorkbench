//! The Rig Check wizard (#248), drawn with the shared UI library.
//!
//! **Design approval pending:** the owner approves the mockups in
//! `docs/rig-check/mockups/` before this design is final (#248).
//!
//! The runner works on a background thread. [`ChannelOperator`] passes each
//! question to the window and blocks until the person answers, so the runner
//! code is the same for the terminal, the window and the tests. Every screen
//! is keyboard accessible: Tab moves between buttons, Enter or Space presses
//! the focused one, and focus starts on the safe answer (Quit, Skip).

pub mod raster;

use super::catalogue::{self, Risk, SaveNotice, Step, StepId};
use super::record::{Mode, Outcome, RunRecord, StepRecord, plain_result};
use super::runner::{Answer, Decision, Operator, Question, Retry};
use crate::ui::chrome::section_panel;
use crate::ui::theme::{
    self, AMBER, BG, BODY, DANGER, DIM, LINE, MUTED, OK, PANEL, RAISED, SECTION, SPACE_8, SPACE_12,
    SPACE_16, TARGET, TEXT, TITLE,
};
use eframe::egui::{self, Color32, RichText, vec2};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Text size for HEAR / SEE, readable from where the guitar is.
pub const BIG: f32 = 34.0;

/// What the runner asks the window.
#[derive(Clone, Debug)]
pub enum Prompt {
    Start(String),
    Card(StepId, Mode),
    Save {
        text: String,
        confirmation: String,
    },
    Instruct {
        step: StepId,
        see: String,
        hear: String,
    },
    NotSeen {
        step: StepId,
        what: String,
    },
    Ask(Question),
    Progress(String),
    Finished(StepRecord),
    Done {
        run: Box<RunRecord>,
        report: String,
        path: String,
    },
}

impl Prompt {
    fn needs_reply(&self) -> bool {
        matches!(
            self,
            Prompt::Start(_)
                | Prompt::Card(..)
                | Prompt::Save { .. }
                | Prompt::NotSeen { .. }
                | Prompt::Ask(_)
        ) || matches!(self, Prompt::Finished(record) if record.started.is_some())
    }
}

#[derive(Clone, Debug)]
pub enum Reply {
    Accept(bool),
    Decision(Decision),
    Typed(Option<String>),
    Retry(Retry),
    Answer(Answer, String),
    Continue(bool),
}

/// The runner's side: blocks on the window's answer. A closed window
/// answers with the safe default (do not run).
pub struct ChannelOperator {
    pub to_ui: Sender<Prompt>,
    pub from_ui: Receiver<Reply>,
    pub stop: Arc<AtomicBool>,
    pub repaint: Option<egui::Context>,
}

impl ChannelOperator {
    pub fn send(&self, prompt: Prompt) {
        let _ = self.to_ui.send(prompt);
        if let Some(ctx) = &self.repaint {
            ctx.request_repaint();
        }
    }

    fn request(&self, prompt: Prompt) -> Option<Reply> {
        self.send(prompt);
        self.from_ui.recv().ok()
    }
}

impl Operator for ChannelOperator {
    fn accept_start(&mut self, notice: &str) -> bool {
        matches!(
            self.request(Prompt::Start(notice.into())),
            Some(Reply::Accept(true))
        )
    }

    fn confirm(&mut self, step: &Step, mode: Mode) -> Decision {
        self.stop.store(false, Ordering::Relaxed);
        match self.request(Prompt::Card(step.id, mode)) {
            Some(Reply::Decision(decision)) => decision,
            _ => Decision::Skip,
        }
    }

    fn confirm_save(&mut self, notice: &SaveNotice) -> Option<String> {
        match self.request(Prompt::Save {
            text: notice.text(),
            confirmation: notice.confirmation.clone(),
        }) {
            Some(Reply::Typed(typed)) => typed,
            _ => None,
        }
    }

    fn instruct(&mut self, step: StepId, see: &str, hear: &str) {
        self.send(Prompt::Instruct {
            step,
            see: see.into(),
            hear: hear.into(),
        });
    }

    fn not_seen(&mut self, step: StepId, what: &str) -> Retry {
        match self.request(Prompt::NotSeen {
            step,
            what: what.into(),
        }) {
            Some(Reply::Retry(retry)) => retry,
            _ => Retry::Skip,
        }
    }

    fn ask(&mut self, question: &Question) -> (Answer, String) {
        match self.request(Prompt::Ask(question.clone())) {
            Some(Reply::Answer(answer, note)) => (answer, note),
            _ => (Answer::NotSure, String::new()),
        }
    }

    fn progress(&mut self, text: &str) {
        self.send(Prompt::Progress(text.into()));
    }

    fn finished(&mut self, record: &StepRecord) -> bool {
        let prompt = Prompt::Finished(record.clone());
        if prompt.needs_reply() {
            matches!(self.request(prompt), Some(Reply::Continue(true)))
        } else {
            self.send(prompt);
            true
        }
    }

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// The window's state. `prompt` is what is on screen now.
pub struct Wizard {
    pub mode: Option<Mode>,
    pub prompt: Option<Prompt>,
    pub instruction: Option<(StepId, String, String)>,
    pub running: Option<StepId>,
    pub progress: Vec<StepRecord>,
    reviewing: Option<StepId>,
    pub messages: Vec<String>,
    pub typed: String,
    pub note: String,
    pub preview: bool,
    pub saved: Option<String>,
    pub stop: Arc<AtomicBool>,
    to_runner: Option<Sender<Reply>>,
    from_runner: Option<Receiver<Prompt>>,
    focus_armed: bool,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            mode: None,
            prompt: None,
            instruction: None,
            running: None,
            progress: Vec::new(),
            reviewing: None,
            messages: Vec::new(),
            typed: String::new(),
            note: String::new(),
            preview: false,
            saved: None,
            stop: Arc::new(AtomicBool::new(false)),
            to_runner: None,
            from_runner: None,
            focus_armed: true,
        }
    }
}

/// Start the runner on a worker thread; `start` builds the port and sink
/// there (a practice rig is not `Send`).
pub fn spawn(
    wizard: &mut Wizard,
    ctx: &egui::Context,
    start: impl FnOnce(ChannelOperator) + Send + 'static,
) {
    let (to_ui, from_runner) = channel();
    let (to_runner, from_ui) = channel();
    let op = ChannelOperator {
        to_ui,
        from_ui,
        stop: wizard.stop.clone(),
        repaint: Some(ctx.clone()),
    };
    wizard.to_runner = Some(to_runner);
    wizard.from_runner = Some(from_runner);
    std::thread::spawn(move || start(op));
}

fn outcome_tag(outcome: &Outcome) -> (&'static str, Color32) {
    match outcome {
        Outcome::Passed => ("PASS", OK),
        Outcome::Failed(_) => ("FAIL", DANGER),
        Outcome::Stopped(_) => ("STOP", DANGER),
        Outcome::SkippedByPerson | Outcome::SkippedNeeds(_) | Outcome::UnverifiedDevice(_) => {
            ("SKIP", MUTED)
        }
        Outcome::NotAuthorized(_) | Outcome::NotAvailable(_) => ("N/A", DIM),
        Outcome::NotReached => ("-", DIM),
    }
}

fn risk_colour(risk: Risk) -> Color32 {
    match risk {
        Risk::None | Risk::Read => OK,
        Risk::Live => AMBER,
        Risk::Save => DANGER,
    }
}

fn big(ui: &mut egui::Ui, label: &str, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).size(BIG).strong().color(AMBER));
        ui.label(RichText::new(text).size(BIG).color(TEXT));
    });
}

fn button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(SECTION)).min_size(vec2(140.0, TARGET + 8.0)),
    )
}

impl Wizard {
    fn reply(&mut self, reply: Reply) {
        if let Some(tx) = &self.to_runner {
            let _ = tx.send(reply);
        }
        if let Some(Prompt::Card(step, _)) = &self.prompt {
            self.running = Some(*step);
        }
        self.prompt = None;
        self.typed.clear();
        self.note.clear();
        self.focus_armed = true;
    }

    /// Take new prompts from the runner. Non-blocking.
    pub fn poll(&mut self) {
        let Some(rx) = &self.from_runner else {
            return;
        };
        let mut incoming = Vec::new();
        while let Ok(prompt) = rx.try_recv() {
            incoming.push(prompt);
        }
        for prompt in incoming {
            match prompt {
                Prompt::Progress(text) => self.messages.push(text),
                Prompt::Finished(record) => {
                    self.running = None;
                    self.instruction = None;
                    self.progress.retain(|r| r.id != record.id);
                    if record.started.is_some() {
                        self.prompt = Some(Prompt::Finished(record.clone()));
                        self.focus_armed = true;
                    }
                    self.progress.push(record);
                }
                Prompt::Instruct { step, see, hear } => self.instruction = Some((step, see, hear)),
                other => {
                    if other.needs_reply() {
                        self.focus_armed = true;
                    }
                    self.prompt = Some(other);
                }
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.poll();
        egui::SidePanel::left("rig-check-progress")
            .resizable(false)
            .exact_width(300.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(SPACE_12))
            .show(ctx, |ui| self.progress_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(SPACE_16))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.central(ui));
            });
    }

    fn progress_panel(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Rig Check").size(TITLE).strong().color(TEXT));
        ui.label(
            RichText::new(match self.mode {
                Some(Mode::Practice) => "Practice: simulated devices",
                Some(Mode::Real) => "Real rig",
                None => "Choose a mode",
            })
            .size(BODY)
            .color(MUTED),
        );
        ui.add_space(SPACE_8);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for id in catalogue::RUN_ORDER {
                let step = catalogue::step(*id).expect("catalogue step");
                let finished = self.progress.iter().find(|r| r.id == *id);
                let (tag, colour) = match finished {
                    Some(record) => outcome_tag(&record.outcome),
                    None if self.running == Some(*id) => ("RUN", AMBER),
                    None => ("", DIM),
                };
                ui.horizontal(|ui| {
                    ui.add_sized(
                        vec2(40.0, 18.0),
                        egui::Label::new(RichText::new(tag).size(12.0).strong().color(colour)),
                    );
                    let title = RichText::new(format!("{} {}", id, step.title))
                        .size(12.0)
                        .color(if tag.is_empty() { DIM } else { TEXT });
                    if finished.is_some() {
                        let active = matches!(&self.prompt, Some(Prompt::Finished(record)) if record.id == *id);
                        if ui.add(egui::Button::new(title).truncate().selected(
                            self.reviewing == Some(*id) || (self.reviewing.is_none() && active)
                        )).clicked() {
                            self.reviewing = (!active).then_some(*id);
                            self.focus_armed = true;
                        }
                    } else {
                        ui.add(egui::Label::new(title).truncate());
                    }
                });
            }
        });
    }

    fn focus_once(&mut self, response: &egui::Response) {
        if self.focus_armed {
            response.request_focus();
            self.focus_armed = false;
        }
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        if self.mode.is_none() {
            ui.label(
                RichText::new("Welcome to Rig Check")
                    .size(TITLE)
                    .strong()
                    .color(TEXT),
            );
            ui.label(RichText::new("Choose how to run. Practice uses simulated devices and sends nothing to your rig. Real rig uses the connected PODxt Live and Variax; each device step needs the owner's approval in this version.").size(BODY).color(TEXT));
            ui.add_space(SPACE_16);
            ui.horizontal(|ui| {
                let practice = button(ui, "Practice", true);
                self.focus_once(&practice);
                if practice.clicked() {
                    self.mode = Some(Mode::Practice);
                }
                if button(ui, "Real rig", true).clicked() {
                    self.mode = Some(Mode::Real);
                }
            });
            return;
        }
        if let Some(id) = self.reviewing
            && let Some(record) = self.progress.iter().find(|record| record.id == id).cloned()
        {
            self.result(ui, &record, false);
            return;
        }
        if let Some((step, see, hear)) = self.instruction.clone()
            && self.prompt.is_none()
        {
            ui.label(
                RichText::new(format!("Step {step}: listening"))
                    .size(SECTION)
                    .color(MUTED),
            );
            ui.add_space(SPACE_12);
            big(ui, "SEE ", &see);
            ui.add_space(SPACE_12);
            big(ui, "HEAR ", &hear);
            ui.add_space(SPACE_16);
            ui.label(RichText::new("Nothing is sent to your rig while Rig Check listens. When it sees your action, the result appears and waits for you to continue.").size(BODY).color(MUTED));
            if button(ui, "Stop", true).clicked() {
                self.stop.store(true, Ordering::Relaxed);
            }
            return;
        }
        match self.prompt.clone() {
            None => self.waiting(ui),
            Some(Prompt::Start(notice)) => self.start_notice(ui, &notice),
            Some(Prompt::Card(id, mode)) => self.card(ui, id, mode),
            Some(Prompt::Save { text, confirmation }) => self.save(ui, &text, &confirmation),
            Some(Prompt::NotSeen { step, what }) => self.not_seen(ui, step, &what),
            Some(Prompt::Ask(question)) => self.question(ui, &question),
            Some(Prompt::Finished(record)) => self.result(ui, &record, true),
            Some(Prompt::Done { run, report, path }) => self.summary(ui, &run, &report, &path),
            Some(_) => {}
        }
    }

    fn result(&mut self, ui: &mut egui::Ui, record: &StepRecord, awaiting: bool) {
        let (_, colour) = outcome_tag(&record.outcome);
        ui.label(
            RichText::new(format!("Step {} result", record.id))
                .size(SECTION)
                .color(MUTED),
        );
        ui.label(
            RichText::new(record.title)
                .size(TITLE + 6.0)
                .strong()
                .color(TEXT),
        );
        ui.label(
            RichText::new(record.outcome.label())
                .size(SECTION)
                .strong()
                .color(colour),
        );
        let lines = record.result_lines();
        if !lines.is_empty() {
            ui.add_space(SPACE_8);
            section_panel(ui, "What Rig Check found", |ui| {
                for line in &lines {
                    ui.label(RichText::new(line).size(SECTION).color(TEXT));
                }
            });
        }
        ui.add_space(SPACE_16);
        ui.horizontal(|ui| {
            if awaiting {
                let next = button(ui, "Continue", true);
                self.focus_once(&next);
                let stop = button(ui, "Stop", true);
                if next.clicked() {
                    self.reply(Reply::Continue(true));
                } else if stop.clicked() {
                    self.reply(Reply::Continue(false));
                }
            } else {
                let back = button(ui, "Back", true);
                self.focus_once(&back);
                if back.clicked() {
                    self.reviewing = None;
                    self.focus_armed = true;
                }
            }
        });
    }

    fn waiting(&mut self, ui: &mut egui::Ui) {
        match self.running {
            Some(step) => {
                let title = catalogue::step(step).map_or("", |s| s.title);
                ui.label(
                    RichText::new(format!("Running step {step}: {title}"))
                        .size(TITLE)
                        .color(TEXT),
                );
                ui.spinner();
                ui.label(RichText::new("Stop asks the step to finish its current read and put the rig back. A save that has started is always completed and checked.").size(BODY).color(MUTED));
                if button(ui, "Stop", true).clicked() {
                    self.stop.store(true, Ordering::Relaxed);
                }
            }
            None => {
                ui.label(RichText::new("Preparing…").size(TITLE).color(MUTED));
            }
        }
        for line in self.messages.iter().rev().take(5) {
            ui.label(RichText::new(plain_result(line)).size(BODY).color(MUTED));
        }
    }

    fn start_notice(&mut self, ui: &mut egui::Ui, notice: &str) {
        section_panel(ui, "Before you start", |ui| {
            ui.label(RichText::new(notice).size(BODY + 1.0).color(TEXT));
        });
        ui.add_space(SPACE_16);
        ui.horizontal(|ui| {
            let quit = button(ui, "Quit", true);
            self.focus_once(&quit);
            if quit.clicked() {
                self.reply(Reply::Accept(false));
            }
            if button(ui, "Accept and continue", true).clicked() {
                self.reply(Reply::Accept(true));
            }
        });
    }

    fn card(&mut self, ui: &mut egui::Ui, id: StepId, mode: Mode) {
        let step = catalogue::step(id).expect("catalogue step");
        ui.label(
            RichText::new(format!("Step {id}"))
                .size(SECTION)
                .color(MUTED),
        );
        ui.label(
            RichText::new(step.title)
                .size(TITLE + 6.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(SPACE_8);
        section_panel(ui, "What it does", |ui| {
            ui.label(RichText::new(step.does).size(BODY + 1.0).color(TEXT));
            ui.label(
                RichText::new(format!("You do: {}", step.you_do))
                    .size(BODY)
                    .color(MUTED),
            );
        });
        ui.add_space(SPACE_8);
        section_panel(ui, "What you'll see and hear", |ui| {
            big(ui, "SEE ", step.see);
            big(ui, "HEAR ", step.hear);
        });
        ui.add_space(SPACE_8);
        section_panel(ui, "Risk", |ui| {
            ui.label(
                RichText::new(format!(
                    "{} · gate {} · needs {}",
                    step.risk.label(),
                    step.gate_labels(),
                    if step.needs.is_empty() {
                        "nothing".to_string()
                    } else {
                        step.needs
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                ))
                .size(SECTION)
                .strong()
                .color(risk_colour(step.risk)),
            );
            ui.label(
                RichText::new(format!("Records: {}", step.evidence))
                    .size(BODY)
                    .color(MUTED),
            );
            if mode == Mode::Practice {
                ui.label(
                    RichText::new("Practice: simulated devices, nothing is sent to your rig.")
                        .size(BODY)
                        .color(OK),
                );
            }
        });
        ui.add_space(SPACE_16);
        ui.horizontal(|ui| {
            let skip = button(ui, "Skip  (S)", true);
            self.focus_once(&skip);
            let run = button(ui, "Run  (R)", true);
            let stop = button(ui, "Stop the run  (Q)", true);
            let keys = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::R),
                    i.key_pressed(egui::Key::S),
                    i.key_pressed(egui::Key::Q),
                )
            });
            if skip.clicked() || keys.1 {
                self.reply(Reply::Decision(Decision::Skip));
            } else if run.clicked() || keys.0 {
                self.reply(Reply::Decision(Decision::Run));
            } else if stop.clicked() || keys.2 {
                self.reply(Reply::Decision(Decision::Stop));
            }
        });
        ui.label(
            RichText::new("Nothing runs until you choose Run. Skip is the default.")
                .size(BODY)
                .color(MUTED),
        );
    }

    fn save(&mut self, ui: &mut egui::Ui, text: &str, confirmation: &str) {
        ui.label(
            RichText::new("This step saves to your device")
                .size(TITLE + 4.0)
                .strong()
                .color(DANGER),
        );
        egui::Frame::new()
            .fill(RAISED)
            .stroke(egui::Stroke::new(2.0_f32, DANGER))
            .corner_radius(8)
            .inner_margin(SPACE_16)
            .show(ui, |ui| {
                ui.label(RichText::new(text).size(BODY + 1.0).color(TEXT));
            });
        ui.add_space(SPACE_12);
        ui.label(
            RichText::new(format!("Type {confirmation} to save:"))
                .size(SECTION)
                .color(TEXT),
        );
        let field = ui.add(
            egui::TextEdit::singleline(&mut self.typed)
                .desired_width(360.0)
                .font(egui::TextStyle::Monospace),
        );
        let _ = field;
        ui.add_space(SPACE_8);
        ui.horizontal(|ui| {
            let skip = button(ui, "Skip", true);
            self.focus_once(&skip);
            if skip.clicked() {
                self.reply(Reply::Typed(None));
            }
            let matches = self.typed == confirmation;
            if button(ui, "Save", matches)
                .on_disabled_hover_text("Type the confirmation exactly first.")
                .clicked()
            {
                let typed = self.typed.clone();
                self.reply(Reply::Typed(Some(typed)));
            }
        });
    }

    fn not_seen(&mut self, ui: &mut egui::Ui, step: StepId, what: &str) {
        ui.label(
            RichText::new(format!("Step {step}: didn't see it"))
                .size(TITLE + 4.0)
                .strong()
                .color(AMBER),
        );
        ui.label(
            RichText::new(format!(
                "Rig Check did not see {what} in 30 seconds. Nothing was sent meanwhile."
            ))
            .size(BODY + 1.0)
            .color(TEXT),
        );
        ui.add_space(SPACE_12);
        ui.horizontal(|ui| {
            let skip = button(ui, "Skip", true);
            self.focus_once(&skip);
            if skip.clicked() {
                self.reply(Reply::Retry(Retry::Skip));
            }
            if button(ui, "Try again", true).clicked() {
                self.reply(Reply::Retry(Retry::TryAgain));
            }
        });
    }

    fn question(&mut self, ui: &mut egui::Ui, question: &Question) {
        ui.label(
            RichText::new(format!("Step {}: listen", question.step))
                .size(SECTION)
                .color(MUTED),
        );
        ui.label(
            RichText::new(&question.text)
                .size(BIG - 8.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(SPACE_8);
        section_panel(ui, "Already proven by the byte checks", |ui| {
            for line in &question.verified_by_bytes {
                ui.label(RichText::new(format!("• {line}")).size(BODY).color(MUTED));
            }
        });
        ui.add_space(SPACE_8);
        ui.label(RichText::new("Note (optional)").size(BODY).color(MUTED));
        ui.add(egui::TextEdit::singleline(&mut self.note).desired_width(520.0));
        ui.add_space(SPACE_8);
        ui.horizontal(|ui| {
            let unsure = button(ui, "Not sure", true);
            self.focus_once(&unsure);
            let mut answer = None;
            if button(ui, "Yes", true).clicked() {
                answer = Some(Answer::Yes);
            }
            if button(ui, "No", true).clicked() {
                answer = Some(Answer::No);
            }
            if unsure.clicked() {
                answer = Some(Answer::NotSure);
            }
            if let Some(answer) = answer {
                let note = self.note.clone();
                self.reply(Reply::Answer(answer, note));
            }
        });
    }

    fn summary(&mut self, ui: &mut egui::Ui, run: &RunRecord, report: &str, path: &str) {
        ui.label(
            RichText::new("Rig Check finished")
                .size(TITLE + 6.0)
                .strong()
                .color(TEXT),
        );
        ui.label(
            RichText::new(format!(
                "{} passed · {} failed · {} skipped · {} not authorized or not available",
                run.count("passed"),
                run.count("failed"),
                run.count("skipped") + run.count("skipped_needs") + run.count("unverified_device"),
                run.count("not_authorized") + run.count("not_available")
            ))
            .size(SECTION)
            .color(TEXT),
        );
        section_panel(ui, "Your rig", |ui| {
            ui.label(
                RichText::new(plain_result(&run.rig_state))
                    .size(BODY + 1.0)
                    .color(TEXT),
            );
        });
        ui.add_space(SPACE_8);
        ui.label(
            RichText::new(format!(
                "The report is kept at {path} (rewritten after every step)."
            ))
            .size(BODY)
            .color(MUTED),
        );
        ui.horizontal(|ui| {
            let preview = button(
                ui,
                if self.preview {
                    "Hide results"
                } else {
                    "Preview results"
                },
                true,
            );
            self.focus_once(&preview);
            if preview.clicked() {
                self.preview = !self.preview;
            }
            if button(ui, "Save report…", true).clicked() {
                self.saved = Some(save_dialog(report));
            }
        });
        if let Some(saved) = &self.saved {
            ui.label(RichText::new(saved).size(BODY).color(OK));
        }
        if self.preview {
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0_f32, LINE))
                .inner_margin(SPACE_8)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for record in run.steps.iter().filter(|record| record.started.is_some())
                            {
                                ui.label(
                                    RichText::new(format!(
                                        "{} {} — {}",
                                        record.id,
                                        record.title,
                                        record.outcome.label()
                                    ))
                                    .strong(),
                                );
                                for line in record.result_lines() {
                                    ui.label(line);
                                }
                            }
                        });
                });
        }
    }
}

/// Save a copy of the report where the person chooses (never overwrites).
fn save_dialog(report: &str) -> String {
    if cfg!(test) {
        return "Saving is not available in tests.".into();
    }
    match rfd::FileDialog::new()
        .set_file_name("rig-check-report.txt")
        .save_file()
    {
        Some(path) => match super::report::save_copy(report, &path) {
            Ok(()) => format!("Saved {}.", path.display()),
            Err(error) => format!("Not saved: {error}"),
        },
        None => "Not saved.".into(),
    }
}

/// The eframe application.
pub struct App {
    pub wizard: Wizard,
    pub start: Option<Box<dyn FnOnce(Mode, ChannelOperator) + Send>>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let (Some(mode), true) = (self.wizard.mode, self.wizard.to_runner.is_none())
            && let Some(start) = self.start.take()
        {
            spawn(&mut self.wizard, ctx, move |op| start(mode, op));
        }
        self.wizard.show(ctx);
    }
}

pub fn apply_theme(ctx: &egui::Context) {
    theme::apply(ctx);
}

/// The screens drawn for the owner's design review (#248).
pub fn mockup_states() -> Vec<(&'static str, Wizard)> {
    use super::catalogue::START_NOTICE;
    let practice = |prompt: Option<Prompt>| {
        let mut wizard = Wizard {
            mode: Some(Mode::Practice),
            prompt,
            ..Wizard::default()
        };
        wizard.focus_armed = false;
        wizard
    };
    let mut done = Vec::new();
    for (id, outcome) in [
        (StepId(0, 1), Outcome::Passed),
        (StepId(0, 2), Outcome::Passed),
        (StepId(0, 3), Outcome::Passed),
        (StepId(1, 1), Outcome::Passed),
        (StepId(1, 2), Outcome::Passed),
        (StepId(2, 1), Outcome::Passed),
        (StepId(2, 2), Outcome::Passed),
        (StepId(3, 1), Outcome::Passed),
        (StepId(3, 2), Outcome::SkippedByPerson),
        (StepId(3, 3), Outcome::Passed),
    ] {
        let mut record = StepRecord::new(id);
        record.outcome = outcome;
        done.push(record);
    }
    let with_progress = |mut wizard: Wizard, upto: usize| {
        wizard.progress = done[..upto].to_vec();
        wizard
    };
    let mut states = vec![
        (
            "01-choose-mode",
            Wizard {
                focus_armed: false,
                ..Wizard::default()
            },
        ),
        ("02-start-notice", practice(Some(Prompt::Start(START_NOTICE.into())))),
        (
            "03-step-card-read",
            with_progress(practice(Some(Prompt::Card(StepId(2, 1), Mode::Practice))), 5),
        ),
        (
            "04-step-card-live",
            with_progress(practice(Some(Prompt::Card(StepId(4, 3), Mode::Real))), 10),
        ),
        ("05-running", {
            let mut w = with_progress(practice(None), 9);
            w.running = Some(StepId(3, 3));
            w
        }),
        ("06-listening", {
            let mut w = with_progress(practice(None), 10);
            w.running = Some(StepId(7, 1));
            w.instruction = Some((
                StepId(7, 1),
                "Turn the selector to Spank 2.".into(),
                "Spank 2.".into(),
            ));
            w
        }),
        (
            "07-not-seen",
            with_progress(
                practice(Some(Prompt::NotSeen {
                    step: StepId(7, 1),
                    what: "program change to Spank 2".into(),
                })),
                10,
            ),
        ),
        (
            "08-listening-question",
            with_progress(
                practice(Some(Prompt::Ask(Question {
                    step: StepId(4, 3),
                    text: "Now play the test model (B) and compare it with the sound you just played (A). Is the lowest string tuned lower (drop D)?".into(),
                    verified_by_bytes: vec![
                        "the test model is in the working buffer: all 1,092 words read back".into(),
                        "subtle settings (one pickup, tone capacitor, pot values) are proven by this byte check, not by ear".into(),
                    ],
                }))),
                10,
            ),
        ),
    ];
    let notice = SaveNotice {
        device: "Variax",
        slot: "Spank 1 (flash slot 14)".into(),
        backup_step: StepId(3, 4),
        backup_file: "variax-library/14-Spank-1.bin".into(),
        backup_sha256: "4e6b812d0c9a5e7f13b2d6a8c0f1e2d3b4a5968778695a4b3c2d1e0f9a8b33d5".into(),
        confirmation: "STORE VARIAX Spank 1".into(),
    };
    states.push((
        "09-save-notice",
        with_progress(
            practice(Some(Prompt::Save {
                text: notice.text(),
                confirmation: notice.confirmation.clone(),
            })),
            10,
        ),
    ));
    let mut record = StepRecord::new(StepId(2, 3));
    record.started = Some("2026-11-25 22:02:00 UTC".into());
    record.outcome = Outcome::Passed;
    record.show("Channel 1A: Practice 1A");
    let mut run = RunRecord::new(Mode::Practice, "2026-11-25 22:00:00 UTC".into());
    for record in &done {
        *run.step_mut(record.id) = record.clone();
    }
    *run.step_mut(record.id) = record.clone();
    run.rig_state = "The PODxt Live tone is the starting tone (all 168 bytes). The Variax plays the starting program (1,092 words). Guitar interface released. Interface 1 driver: snd-usb-variax (expected).".into();
    run.finished = true;
    let report = super::report::render_plain(&run, "2026-11-25 22:05:00 UTC");
    let mut summary = with_progress(
        practice(Some(Prompt::Done {
            run: Box::new(run),
            report,
            path: "rig-check-output/rig-check-20261125T220000Z.txt".into(),
        })),
        10,
    );
    summary.preview = true;
    states.push(("10-summary-preview", summary));
    let mut result = with_progress(practice(Some(Prompt::Finished(record.clone()))), 10);
    result.progress.push(record);
    result.focus_armed = true;
    states.push(("11-step-result", result));
    states
}

/// Render every mockup screen into `dir` as PNG (new files only).
pub fn write_mockups(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut written = Vec::new();
    for (name, mut wizard) in mockup_states() {
        let image = raster::render(1180, 800, |ctx| wizard.show(ctx));
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let path = dir.join(format!("{name}.png"));
        crate::shared::document::save_new(&path, &png)?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests;

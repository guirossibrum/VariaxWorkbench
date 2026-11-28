//! Help diagnostics for the current local Workbench session.

use super::{PickRequest, PickerAction, Workbench};
use eframe::egui;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;
use variax_workbench::session_log::{self, Export, LogStatus, SessionLog, SinkState};

const REFRESH: Duration = Duration::from_millis(50);

pub(crate) struct SessionLogPanel {
    open: bool,
    log: Option<SessionLog>,
    job: Option<Job>,
    message: Option<String>,
}

enum Job {
    Export { path: PathBuf, result: Export },
    OpenFolder(mpsc::Receiver<io::Result<()>>),
}

impl Workbench {
    pub(super) fn show_session_log_panel(&mut self, ctx: &egui::Context) {
        self.session_log_panel.poll(ctx);
        if !self.session_log_panel.open {
            return;
        }
        if self.session_log_panel.show(ctx, self.picker.is_open())
            && self.request_picker(
                ctx,
                PickRequest::SaveSessionLog,
                PickerAction::ExportSessionLog,
            )
        {
            self.session_log_panel.message = None;
        }
        if !self.session_log_panel.open {
            self.picker
                .discard_if(|action| matches!(action, PickerAction::ExportSessionLog));
        }
    }
}

impl SessionLogPanel {
    pub(super) fn new() -> Self {
        Self {
            open: false,
            log: session_log::current().cloned(),
            job: None,
            message: None,
        }
    }

    pub(super) fn open(&mut self) {
        self.open = true;
    }

    #[cfg(test)]
    pub(in super::super) fn use_log(&mut self, log: SessionLog) {
        self.log = Some(log);
    }

    #[cfg(test)]
    pub(in super::super) fn export_pending(&self) -> bool {
        matches!(self.job, Some(Job::Export { .. }))
    }

    fn show(&mut self, ctx: &egui::Context, picker_open: bool) -> bool {
        let status = self.log.as_ref().map(SessionLog::status);
        let folder = status.as_ref().and_then(|status| status.folder.as_deref());
        let can_export = status.as_ref().is_some_and(|status| {
            status.path.is_some()
                && matches!(status.state, SinkState::Recording | SinkState::SizeLimit)
        });
        if status
            .as_ref()
            .is_some_and(|s| s.state == SinkState::Starting)
        {
            ctx.request_repaint_after(REFRESH);
        }

        let idle = self.job.is_none() && !picker_open;
        let mut open_folder = false;
        let mut export = false;
        egui::Window::new("Diagnostics")
            .open(&mut self.open)
            .collapsible(false)
            .default_width(440.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading("Session log");
                ui.label(sink_status(status.as_ref()));
                ui.label(format!(
                    "Minimum level: {}",
                    status.as_ref().map_or("Unavailable", |s| s.level.label())
                ));
                show_path(
                    ui,
                    "Current file",
                    status.as_ref().and_then(|s| s.path.as_deref()),
                );
                show_path(ui, "Log folder", folder);
                ui.horizontal_wrapped(|ui| {
                    open_folder = ui
                        .add_enabled(
                            idle && folder.is_some(),
                            egui::Button::new("Open log folder"),
                        )
                        .clicked();
                    export = ui
                        .add_enabled(idle && can_export, egui::Button::new("Export session log…"))
                        .clicked();
                });
                if let Some(job) = &self.job {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(match job {
                            Job::Export { .. } => "Exporting session log…",
                            Job::OpenFolder(_) => "Opening log folder…",
                        });
                    });
                }
                if let Some(message) = &self.message {
                    ui.label(message);
                }
            });
        if open_folder && let Some(folder) = folder {
            self.message = None;
            match open_folder_async(folder.to_path_buf()) {
                Ok(result) => self.job = Some(Job::OpenFolder(result)),
                Err(error) => self.message = Some(format!("Could not open log folder: {error}")),
            }
            ctx.request_repaint();
        }
        self.open && export
    }

    pub(super) fn export_to(&mut self, ctx: &egui::Context, path: Option<PathBuf>) {
        self.message = match path {
            None => Some("Session log export canceled.".into()),
            Some(path) => match self
                .log
                .as_ref()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))
                .and_then(|log| log.export_current(path.clone()))
            {
                Ok(result) => {
                    self.job = Some(Job::Export { path, result });
                    None
                }
                Err(error) => Some(export_error(error)),
            },
        };
        ctx.request_repaint();
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let result = match job {
            Job::Export { result, .. } | Job::OpenFolder(result) => result.try_recv(),
        };
        let result = match result {
            Ok(result) => result,
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(REFRESH);
                return;
            }
            Err(TryRecvError::Disconnected) => Err(io::Error::other("The worker stopped.")),
        };
        self.message = match (job, result) {
            (Job::Export { path, .. }, Ok(())) => {
                Some(format!("Session log exported to {}", path.display()))
            }
            (Job::Export { .. }, Err(error)) => Some(export_error(error)),
            (Job::OpenFolder(_), Ok(())) => None,
            (Job::OpenFolder(_), Err(error)) => Some(format!("Could not open log folder: {error}")),
        };
        self.job = None;
    }
}

fn sink_status(status: Option<&LogStatus>) -> &'static str {
    match status.map(|status| status.state) {
        None => "Session logging is inactive.",
        Some(SinkState::Starting) => "Starting the session log…",
        Some(SinkState::Recording) => "Recording to the session log.",
        Some(SinkState::SizeLimit) => "The session log has reached its size limit.",
        Some(SinkState::Unavailable) => "Workbench cannot write the session log.",
        Some(SinkState::Closed) => "The session log is closed.",
    }
}

fn show_path(ui: &mut egui::Ui, label: &str, path: Option<&Path>) {
    ui.label(label);
    let value = path.map_or("Unavailable".into(), Path::to_string_lossy);
    ui.add(egui::Label::new(value.as_ref()).wrap().selectable(true));
}

fn export_error(error: io::Error) -> String {
    match error.kind() {
        io::ErrorKind::AlreadyExists => "That file already exists. Choose a new file name.".into(),
        io::ErrorKind::WouldBlock => "The session log is busy. Try exporting again.".into(),
        _ => format!("Could not export session log: {error}"),
    }
}

fn open_folder_async(folder: PathBuf) -> io::Result<mpsc::Receiver<io::Result<()>>> {
    let (sender, result) = mpsc::channel();
    std::thread::Builder::new()
        .name("workbench-open-log-folder".into())
        .spawn(move || {
            let _ = sender.send(open_folder(&folder));
        })?;
    Ok(result)
}

fn open_folder(folder: &Path) -> io::Result<()> {
    use std::process::{Command, Stdio};

    let folder = std::path::absolute(folder)?;
    if !folder.metadata()?.is_dir() {
        return Err(io::Error::other("The log folder is not a directory."));
    }
    #[cfg(target_os = "windows")]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "/usr/bin/open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";

    let status = Command::new(program)
        .arg(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    // Explorer can hand off a request to the desktop and exit with a nonzero status.
    if cfg!(target_os = "windows") || status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{program} returned {status}")))
    }
}

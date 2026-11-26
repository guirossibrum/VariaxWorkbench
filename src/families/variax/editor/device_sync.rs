use super::{
    FamilyTab, LibraryOrigin, PresetFile, VariaxEditor, egui, live_attachment::Attachment,
};
use line6_devices::family::variax::Preset;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
    time::Duration,
};

const WRITE_TO_DEVICE_LOCKED: bool = true;
const SYNC_TO_DEVICE_LOCKED: bool = true;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    WriteSelected,
    Sync,
    ReadUnmodified,
}

impl Action {
    pub(super) fn permitted(self) -> bool {
        match self {
            Self::WriteSelected => !WRITE_TO_DEVICE_LOCKED,
            Self::Sync => !SYNC_TO_DEVICE_LOCKED,
            Self::ReadUnmodified => true,
        }
    }

    pub(super) fn writes(self) -> bool {
        self != Self::ReadUnmodified
    }
}

type Position = (usize, usize);
type ModelUpdate = (usize, usize, PresetFile);

pub(super) struct Target {
    pub(super) position: Preset,
    pub(super) model: PresetFile,
}

#[derive(Default)]
pub(super) struct Report {
    pub(super) verified: Vec<ModelUpdate>,
    pub(super) error: Option<String>,
    pub(super) store_unknown: bool,
}

enum Prompt {
    Choose(Vec<Position>),
    Confirm(Action, Vec<Position>),
}

struct Worker {
    action: Action,
    cancel: Arc<AtomicBool>,
    status: Receiver<String>,
    thread: JoinHandle<Report>,
}

#[derive(Default)]
pub(super) struct DeviceSync {
    attachment: Option<Attachment>,
    prompt: Option<Prompt>,
    pub(super) imported_backup: Option<PathBuf>,
    worker: Option<Worker>,
    unsafe_store: bool,
}

impl DeviceSync {
    pub(super) fn observe(&mut self, attachment: Option<Attachment>) {
        if self.attachment != attachment {
            self.cancel();
            self.prompt = None;
            self.attachment = attachment;
        }
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn stopped_after_store(&self) -> bool {
        self.unsafe_store
    }

    pub(super) fn prompting(&self) -> bool {
        self.prompt.is_some()
    }

    pub(super) fn cancel(&self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for DeviceSync {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.thread.join();
        }
    }
}

#[cfg(any(not(target_os = "linux"), test))]
pub(super) fn unavailable(
    _: Action,
    _: &Attachment,
    _: (Option<&std::path::Path>, Option<&std::path::Path>),
    _: &[Target],
    _: &AtomicBool,
    _: &mpsc::Sender<String>,
) -> Report {
    Report {
        error: Some("Variax library device actions are unavailable in this build.".into()),
        ..Default::default()
    }
}

impl VariaxEditor {
    pub(super) fn modified_guitar_positions(&self) -> Vec<Position> {
        if !self.sync_available() {
            return Vec::new();
        }
        let Some(document) = &self.document else {
            return Vec::new();
        };
        Preset::all()
            .map(|p| (usize::from(p.bank()), usize::from(p.slot())))
            .filter(|&(bank, slot)| document.slot_is_unsent(bank, slot))
            .collect()
    }

    pub(super) fn sync_available(&self) -> bool {
        self.library_origin
            .as_ref()
            .is_some_and(LibraryOrigin::is_guitar_library)
    }

    pub(super) fn sync_ready(&self, action: Action) -> bool {
        cfg!(target_os = "linux")
            && self.sync_available()
            && action.permitted()
            && self.device_sync.attachment.is_some()
            && !self.device_sync.unsafe_store
            && !self.follow.disconnected()
            && !self.live_edit.busy()
            && !self.is_reading()
            && !self.opening_file
    }

    pub(super) fn sync_button(&mut self, ui: &mut egui::Ui) {
        if self.sync_available()
            && ui
                .add_enabled(
                    !self.is_reading() && !self.blocking_prompt(),
                    egui::Button::new("Sync"),
                )
                .clicked()
        {
            self.device_sync.prompt = Some(Prompt::Choose(self.modified_guitar_positions()));
        }
    }

    pub(super) fn confirm_position_action(&mut self, action: Action, position: Position) {
        if self.sync_ready(action) {
            self.device_sync.prompt = Some(Prompt::Confirm(action, vec![position]));
        }
    }

    pub(super) fn sync_dialog(&mut self, ctx: &egui::Context) {
        if self.device_sync.busy() {
            egui::Modal::new(egui::Id::new("variax-sync-progress")).show(ctx, |ui| {
                ui.heading("Guitar library");
                ui.label(&self.status);
                if ui.button("Cancel remaining work").clicked() {
                    self.device_sync.cancel();
                }
            });
            return;
        }
        let Some(prompt) = self.device_sync.prompt.take() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("variax-sync")).show(ctx, |ui| {
            match &prompt {
                Prompt::Choose(positions) if positions.is_empty() => {
                    ui.heading("Nothing to do");
                    ui.label("There are no modified tones!");
                    ui.button("OK").clicked().then_some(None)
                }
                Prompt::Choose(positions) => {
                    ui.heading("Sync");
                    ui.label(format!("There are {} modified Models.", positions.len()));
                    if ui.add_enabled(self.sync_ready(Action::Sync), egui::Button::new("Save modified Models to Variax"))
                        .on_disabled_hover_text("Sync stores await their first owner test and require a verified guitar.").clicked()
                    {
                        return Some(Some(Prompt::Confirm(Action::Sync, positions.clone())));
                    }
                    ui.add_enabled(self.sync_ready(Action::ReadUnmodified), egui::Button::new("Revert to unmodified Models"))
                        .clicked().then(|| Some(Prompt::Confirm(Action::ReadUnmodified, positions.clone())))
                }
                Prompt::Confirm(action, positions) => {
                    let (title, detail, button) = if action.writes() {
                        ("Overwrite?", "Replace the stored guitar models at these positions?", "Overwrite")
                    } else {
                        ("Read unmodified from device?", "Reload the guitar’s saved models into these positions?", "Read unmodified")
                    };
                    ui.heading(title);
                    ui.label(detail);
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        if let Some(banks) = self.document.as_ref().and_then(|doc| doc.banks()) {
                            for &(bank, slot) in positions {
                                ui.label(format!("{} {} · {}", banks[bank].name, slot + 1, banks[bank].presets()[slot].name()));
                            }
                        }
                    });
                    ui.label("The guitar will play the last position read or verified.");
                    if ui.add_enabled(self.sync_ready(*action), egui::Button::new(button)).clicked() {
                        if let Err(error) = self.start_device_sync(*action, positions) {
                            self.error = Some(error);
                        }
                        Some(None)
                    } else {
                        None
                    }
                }
            }.or_else(|| ui.button("Cancel").clicked().then_some(None))
        });
        if !modal.should_close() {
            self.device_sync.prompt = modal.inner.unwrap_or(Some(prompt));
        }
    }

    fn start_device_sync(&mut self, action: Action, positions: &[Position]) -> Result<(), String> {
        if !self.sync_ready(action) || positions.is_empty() {
            return Err("The guitar library action is unavailable.".into());
        }
        let attachment = self
            .device_sync
            .attachment
            .clone()
            .ok_or("No verified guitar.")?;
        let banks = self
            .document
            .as_ref()
            .and_then(|doc| doc.banks())
            .ok_or("Open a guitar library first.")?;
        let targets = positions
            .iter()
            .map(|&(bank, slot)| {
                let position = u8::try_from(bank)
                    .ok()
                    .zip(u8::try_from(slot).ok())
                    .and_then(|(bank, slot)| Preset::new(bank, slot))
                    .ok_or("Invalid guitar position.")?;
                let model = banks
                    .get(bank)
                    .and_then(|bank| bank.presets().get(slot))
                    .ok_or("Missing guitar library position.")?
                    .clone();
                Ok(Target { position, model })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let preferences = self.preferences_path.clone();
        let imported_backup = self.device_sync.imported_backup.clone();
        let (send, status) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&cancel);
        self.live_edit.stop();
        let thread = std::thread::Builder::new()
            .name("variax-library-sync".into())
            .spawn(move || {
                execute(
                    action,
                    &attachment,
                    (preferences.as_deref(), imported_backup.as_deref()),
                    &targets,
                    &stopping,
                    &send,
                )
            })
            .map_err(|error| format!("Could not start guitar library worker: {error}"))?;
        self.error = None;
        self.status = "Checking guitar library…".into();
        self.device_sync.worker = Some(Worker {
            action,
            cancel,
            status,
            thread,
        });
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn hold_fake_store(&mut self, report: Report) -> mpsc::Sender<()> {
        let (finish, wait) = mpsc::channel();
        let (_progress, status) = mpsc::channel();
        self.device_sync.worker = Some(Worker {
            action: Action::WriteSelected,
            cancel: Arc::new(AtomicBool::new(false)),
            status,
            thread: std::thread::spawn(move || {
                let _ = wait.recv();
                report
            }),
        });
        finish
    }

    pub(super) fn poll_device_sync(&mut self, ctx: &egui::Context) {
        if self.follow.disconnected() {
            self.device_sync.cancel();
        }
        let Some(worker) = &self.device_sync.worker else {
            return;
        };
        if let Some(status) = worker.status.try_iter().last() {
            self.status = status;
        }
        if worker.thread.is_finished() {
            let worker = self
                .device_sync
                .worker
                .take()
                .expect("finished sync worker");
            match worker.thread.join() {
                Ok(report) => self.finish_device_sync(worker.action, report),
                Err(_) => {
                    self.device_sync.unsafe_store |= worker.action.writes();
                    self.error = Some("The guitar library worker stopped unexpectedly.".into());
                }
            }
        } else {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }

    fn finish_device_sync(&mut self, action: Action, report: Report) {
        self.device_sync.unsafe_store |= report.store_unknown;
        let count = report.verified.len();
        if count != 0
            && let Some(document) = &mut self.document
        {
            if let Err(error) = document.accept_guitar_models(&report.verified, !action.writes()) {
                self.error = Some(error);
                return;
            }
            self.clear_text_drafts();
        }
        self.status = if action.writes() {
            format!("Verified {count} stored model(s).")
        } else {
            format!("Read {count} unmodified model(s).")
        };
        self.error = report.error;
    }
}

#[cfg(all(target_os = "linux", not(test)))]
use super::procedure_device::sync as execute;
#[cfg(any(not(target_os = "linux"), test))]
use unavailable as execute;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use variax_workbench::{bundle::BundleFile, document::SlotBuffer};

    #[test]
    fn paste_and_verified_operations_keep_file_and_guitar_baselines_separate() {
        let mut editor = VariaxEditor::new(&super::super::EditorDefaults {
            artwork: Default::default(),
            preferences: None,
            startup_panel: None,
        });
        let mut bundle = BundleFile::new_default_bundle().unwrap();
        let model = bundle.banks()[0].presets()[0].clone();
        let mut other = model.clone();
        other.rename("Edited").unwrap();
        bundle.replace(0, 1, &other).unwrap();
        editor
            .adopt_startup_library(bundle, LibraryOrigin::Download(SystemTime::UNIX_EPOCH))
            .unwrap();
        let doc = editor.document.as_mut().unwrap();
        doc.copy_to_slot_buffer(SlotBuffer::ModelClipboard).unwrap();
        doc.paste_into_guitar_position().unwrap();
        assert!(doc.slot_is_unsent(0, 0));
        assert!(!doc.is_dirty());
        doc.undo();
        assert!(!doc.slot_is_unsent(0, 0));
        doc.redo();
        doc.select(0, 1).unwrap();
        doc.paste_into_guitar_position().unwrap();
        assert_eq!(editor.modified_guitar_positions(), [(0, 0), (0, 1)]);

        editor.finish_device_sync(
            Action::WriteSelected,
            Report {
                verified: vec![(0, 0, model)],
                ..Default::default()
            },
        );
        assert_eq!(editor.modified_guitar_positions(), [(0, 1)]);
        assert!(editor.dirty());

        editor.finish_device_sync(
            Action::ReadUnmodified,
            Report {
                verified: vec![(0, 1, other)],
                ..Default::default()
            },
        );
        assert!(editor.modified_guitar_positions().is_empty());
        assert!(!editor.dirty());
        editor.undo();
        assert_eq!(editor.modified_guitar_positions(), [(0, 1)]);
    }

    #[test]
    fn write_and_sync_switches_stay_closed_separately() {
        assert!(!Action::WriteSelected.permitted());
        assert!(!Action::Sync.permitted());
        assert!(Action::ReadUnmodified.permitted());
    }
}

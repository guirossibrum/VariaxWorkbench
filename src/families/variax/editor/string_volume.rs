use super::{FamilyTab, VariaxEditor, egui, live_attachment::Attachment};
use line6_devices::family::variax::string_levels::{
    self, APPROVED_STRING_LEVEL_EDIT, APPROVED_STRING_LEVEL_READ, APPROVED_STRING_LEVEL_STORE,
    Levels, StoreRequestSent,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use variax_workbench::hardware_gates::{Gate, is_locked, run_if_unlocked};

const EDIT_GATES: [Gate; 5] = [
    Gate::OsDiscovery,
    Gate::IdentityRequest,
    Gate::CurrentSettingsRead,
    Gate::FullBackupRead,
    Gate::EditBufferWrite,
];

fn editing_approved() -> bool {
    APPROVED_STRING_LEVEL_READ
        && APPROVED_STRING_LEVEL_EDIT
        && EDIT_GATES.into_iter().all(|gate| !is_locked(gate))
}

fn store_approved() -> bool {
    APPROVED_STRING_LEVEL_STORE && !is_locked(Gate::StoreRestore)
}

enum Command {
    Preview([f32; 6]),
    Store,
}

enum Update {
    Ready(Levels, PathBuf),
    Verified,
    Store(StoreProgress),
}

enum StoreProgress {
    None,
    EvidenceSaved,
    Sent(Levels),
}

struct WorkerIo {
    commands: Receiver<Command>,
    updates: Sender<Update>,
    stop: Arc<AtomicBool>,
}

impl WorkerIo {
    fn cancelled(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

// The device crate's approvals also block its public API in tests. The dialog
// loop can use a fake edit session without exposing any approval override.
trait EditSession {
    fn original(&self) -> Levels;
    fn backup_folder(&self) -> &Path;
    fn set_ram(&mut self, levels: Levels) -> Result<Levels, String>;
    fn restore_ram(&mut self) -> Result<(), String>;
    fn request_store(&mut self) -> Result<StoreRequestSent, String>;
}

impl<L: line6_devices::session::Link> EditSession for string_levels::Edit<'_, '_, L> {
    fn original(&self) -> Levels {
        self.original()
    }
    fn backup_folder(&self) -> &Path {
        self.backup_folder()
    }
    fn set_ram(&mut self, levels: Levels) -> Result<Levels, String> {
        self.set_ram(levels)
    }
    fn restore_ram(&mut self) -> Result<(), String> {
        self.restore_ram()
    }
    fn request_store(&mut self) -> Result<StoreRequestSent, String> {
        run_if_unlocked(Gate::StoreRestore, || self.request_store())
    }
}

fn run_dialog(edit: &mut impl EditSession, io: &WorkerIo, can_store: bool) -> Result<(), String> {
    let original = edit.original();
    let backup = edit.backup_folder().to_owned();
    let path = backup.join(string_levels::BACKUP_FILE);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("Could not verify {}: {error}", path.display()))?;
    if bytes != original.backup_bytes()
        || !line6_devices::backup::verified_manifest_entries(&backup)?
            .iter()
            .any(|name| name == string_levels::BACKUP_FILE)
    {
        return Err(format!(
            "String-level RAM backup did not verify: {}",
            backup.display()
        ));
    }
    let _ = io.updates.send(Update::Ready(original, backup.clone()));
    let mut verified = original;
    while !io.cancelled()
        && let Ok(command) = io.commands.recv()
    {
        if io.cancelled() {
            break;
        }
        match command {
            Command::Preview(values) => {
                let levels = original.with_workbench_levels(values)?;
                if levels != verified {
                    verified = edit.set_ram(levels)?;
                }
                let _ = io.updates.send(Update::Verified);
            }
            Command::Store if can_store => {
                // This is the same verified value the device API will compare
                // against a fresh 0x57 read before sending the store request.
                let evidence = store_evidence_path(&backup);
                variax_workbench::document::save_new(&evidence, &verified.backup_bytes())
                    .map_err(|error| {
                        format!(
                            "Store request not sent. Could not save pre-store RAM evidence at {}: {error}",
                            evidence.display()
                        )
                    })?;
                let _ = io.updates.send(Update::Store(StoreProgress::EvidenceSaved));
                if io.cancelled() {
                    break;
                }
                let StoreRequestSent { ram_before_request } = edit.request_store()?;
                let _ = io
                    .updates
                    .send(Update::Store(StoreProgress::Sent(ram_before_request)));
                return Ok(());
            }
            Command::Store => return Err("String-level storage awaits separate approval.".into()),
        }
    }
    edit.restore_ram()
}

fn store_evidence_path(backup_folder: &Path) -> PathBuf {
    backup_folder.with_extension("before-store-ram.bin")
}

struct Draft {
    original: Levels,
    values: [f32; 6],
    requested: [f32; 6],
    awaiting_ram: bool,
    backup: PathBuf,
}

struct Worker {
    commands: Option<Sender<Command>>,
    updates: Receiver<Update>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    draft: Option<Draft>,
    can_store: bool,
    confirm_store: bool,
    finishing: bool,
    store: StoreProgress,
}

impl Worker {
    fn spawn(
        can_store: bool,
        run: impl FnOnce(WorkerIo) -> Result<(), String> + Send + 'static,
    ) -> Result<Self, String> {
        let (commands, receive) = mpsc::channel();
        let (send, updates) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let io = WorkerIo {
            commands: receive,
            updates: send,
            stop: stop.clone(),
        };
        let thread = thread::Builder::new()
            .name("variax-string-volume".into())
            .spawn(move || run(io))
            .map_err(|error| format!("Could not start String Volume Setup: {error}"))?;
        Ok(Self {
            commands: Some(commands),
            updates,
            stop,
            thread: Some(thread),
            draft: None,
            can_store,
            confirm_store: false,
            finishing: false,
            store: StoreProgress::None,
        })
    }

    fn disconnect(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.commands = None;
        self.finishing = true;
    }

    fn cancel(&mut self) {
        if !self.finishing {
            self.disconnect();
        }
    }

    fn send_preview(&mut self) {
        if let (Some(commands), Some(draft)) = (&self.commands, &mut self.draft)
            && !draft.awaiting_ram
            && draft.values != draft.requested
            && commands.send(Command::Preview(draft.values)).is_ok()
        {
            draft.requested = draft.values;
            draft.awaiting_ram = true;
        }
    }

    fn ready_to_store(&self) -> bool {
        self.can_store
            && !self.finishing
            && self
                .draft
                .as_ref()
                .is_some_and(|draft| !draft.awaiting_ram && draft.values == draft.requested)
    }

    fn store(&mut self) {
        if self.confirm_store
            && self.ready_to_store()
            && let Some(commands) = self.commands.take()
        {
            let _ = commands.send(Command::Store);
            self.finishing = true;
        }
    }

    fn poll(&mut self) -> Option<Result<(), String>> {
        let finished = self
            .thread
            .as_ref()
            .is_some_and(|thread| thread.is_finished());
        for update in self.updates.try_iter() {
            match update {
                Update::Ready(original, backup) => {
                    let values = original.workbench_levels();
                    self.draft = Some(Draft {
                        original,
                        values,
                        requested: values,
                        awaiting_ram: false,
                        backup,
                    });
                }
                Update::Verified => {
                    if let Some(draft) = &mut self.draft {
                        draft.awaiting_ram = false;
                    }
                }
                Update::Store(progress) => self.store = progress,
            }
        }
        if finished {
            self.thread.take().map(|thread| {
                thread.join().unwrap_or_else(|_| {
                    Err("String Volume Setup worker stopped unexpectedly.".into())
                })
            })
        } else {
            self.send_preview();
            None
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.disconnect();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Default)]
pub(super) struct Setup {
    attachment: Option<Attachment>,
    waiting_for_follow: bool,
    worker: Option<Worker>,
}

impl Setup {
    pub(super) fn active(&self) -> bool {
        self.waiting_for_follow || self.worker.is_some()
    }

    pub(super) fn observe(&mut self, attachment: Option<Attachment>) {
        if self.attachment != attachment {
            self.waiting_for_follow = false;
            if let Some(worker) = &mut self.worker {
                worker.disconnect();
            }
            self.attachment = attachment;
        }
    }

    pub(super) fn cancel(&mut self) {
        self.waiting_for_follow = false;
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
    }
}

impl VariaxEditor {
    pub(super) fn string_volume_button(&mut self, ui: &mut egui::Ui) {
        if self.connection_status.is_none() {
            return;
        }
        let ready = cfg!(target_os = "linux")
            && editing_approved()
            && self.string_volume.attachment.is_some()
            && self.preferences_path.is_some()
            && !self.follow.disconnected()
            && !self.device_sync.stopped_after_store()
            && !self.is_reading()
            && !self.live_edit.busy()
            && !self.blocking_prompt();
        if ui
            .add_enabled(ready, egui::Button::new("String Volume Setup…"))
            .on_disabled_hover_text("Requires an idle, verified guitar and approval for string-level reads and temporary edits.")
            .clicked()
        {
            self.error = None;
            self.follow.stop();
            self.string_volume.waiting_for_follow = true;
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn poll_string_volume(&mut self, ctx: &egui::Context) {
        if self.string_volume.waiting_for_follow && !self.follow.busy() {
            self.string_volume.waiting_for_follow = false;
            if let (Some(attachment), Some(root)) = (
                self.string_volume.attachment.clone(),
                self.preferences_path
                    .as_deref()
                    .and_then(Path::parent)
                    .map(|path| path.join("variax-string-level-backups")),
            ) && !self.follow.disconnected()
            {
                match Worker::spawn(store_approved(), move |io| {
                    edit_attached(&attachment, &root, &io)
                }) {
                    Ok(worker) => self.string_volume.worker = Some(worker),
                    Err(error) => self.error = Some(error),
                }
            }
        }
        if let Some(worker) = &mut self.string_volume.worker
            && let Some(result) = worker.poll()
        {
            self.status = match (&worker.store, &worker.draft) {
                (StoreProgress::Sent(levels), Some(draft)) => format!(
                    "Store request sent. Flash persistence is unverified. RAM before request: {:06X?}. Evidence: {}. Starting backup: {}",
                    levels.wire_words(),
                    store_evidence_path(&draft.backup).display(),
                    draft.backup.display()
                ),
                (StoreProgress::EvidenceSaved, Some(draft)) => format!(
                    "{}Pre-store RAM evidence: {}. Starting backup: {}",
                    if result.is_ok() {
                        "Store request not sent. "
                    } else {
                        ""
                    },
                    store_evidence_path(&draft.backup).display(),
                    draft.backup.display()
                ),
                (_, Some(draft)) if result.is_ok() => format!(
                    "Original string levels retained or restored in RAM: {:06X?}. Backup: {}",
                    draft.original.wire_words(),
                    draft.backup.display()
                ),
                (_, Some(draft)) => format!("String-level RAM backup: {}", draft.backup.display()),
                _ => "String Volume Setup closed.".into(),
            };
            self.error = result.err();
            self.string_volume.worker = None;
        }
        if self.string_volume.active() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    pub(super) fn string_volume_dialog(&mut self, ctx: &egui::Context) {
        if !self.string_volume.active() {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("variax-string-volume")).show(ctx, |ui| {
            ui.heading("String Volume Setup");
            ui.label("String levels affect every model on this guitar.");
            let Some(worker) = &mut self.string_volume.worker else {
                ui.label("Waiting for Guitar Follow to release the connection…");
                return ui.button("Cancel").clicked();
            };
            if worker.finishing {
                ui.spinner();
                ui.label("Finishing String Volume Setup…");
                return false;
            }
            let Some(draft) = &mut worker.draft else {
                ui.spinner();
                ui.label("Reading the six levels and saving the RAM backup…");
                return ui.button("Cancel").clicked();
            };
            if worker.confirm_store {
                ui.label("Save the verified RAM evidence and send one request to store these global levels? Flash persistence is checked separately.");
                if ui.button("Send store request").clicked() {
                    worker.store();
                }
                return ui.button("Cancel").clicked();
            }
            ui.horizontal(|ui| {
                for (label, value) in ["Low E", "A", "D", "G", "B", "High E"]
                    .into_iter()
                    .zip(&mut draft.values)
                {
                    ui.vertical_centered(|ui| {
                        ui.set_width(52.0);
                        ui.label(label);
                        ui.add(
                            egui::Slider::new(value, 0.0..=1.0)
                                .vertical()
                                .clamping(egui::SliderClamping::Edits)
                                .step_by(0.001)
                                .show_value(false),
                        );
                        ui.label(format!("{value:.2}"));
                    });
                }
            });
            ui.label(if draft.awaiting_ram || draft.values != draft.requested {
                "Verifying levels in RAM…"
            } else {
                "Levels verified in RAM."
            });
            ui.label(format!("RAM backup: {}", draft.backup.display()));
            ui.horizontal(|ui| {
                let ok = ui
                    .add_enabled(worker.ready_to_store(), egui::Button::new("OK"))
                    .on_disabled_hover_text("Storage needs separate approval and verified RAM levels.");
                if ok.clicked() {
                    worker.confirm_store = true;
                }
                ui.button("Cancel").clicked()
            })
            .inner
        });
        if modal.inner || modal.should_close() {
            self.string_volume.cancel();
        }
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn edit_attached(attachment: &Attachment, root: &Path, io: &WorkerIo) -> Result<(), String> {
    use line6_devices::{
        device_lock::DeviceLock,
        gate::{GatePolicy, Operation},
        session::{Quiet, Session},
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Policy;
    impl GatePolicy for Policy {
        fn allows(&self, operation: Operation) -> bool {
            match operation {
                Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
                Operation::VariaxRead => editing_approved(),
                Operation::VariaxEditBufferWrite => editing_approved(),
                Operation::VariaxStoreRestore => store_approved(),
                _ => false,
            }
        }
    }

    if !editing_approved() {
        return Err("String-level reads and temporary edits await owner approval.".into());
    }
    for gate in EDIT_GATES {
        run_if_unlocked(gate, || Ok(()))?;
    }
    if io.cancelled() {
        return Ok(());
    }
    let rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
    if io.cancelled() {
        return Ok(());
    }
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    let name = format!(
        "variax-string-levels-{}-{:09}",
        line6_devices::backup::utc(time.as_secs()).0,
        time.subsec_nanos()
    );
    let link = variax_workbench::usb::open_variax_bridge_borrowed(
        &rig,
        (&attachment.path, attachment.address),
    )?;
    let mut log = Quiet;
    let mut session = Session::new(link, &Policy, &mut log);
    session.log_packets = false;
    let result = string_levels::with_temporary_edit(&mut session, root, &name, |edit| {
        run_dialog(edit, io, store_approved())
    });
    match (result, session.link.finish()) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) | (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error} {cleanup}")),
    }
}

#[cfg(any(not(target_os = "linux"), test))]
fn edit_attached(_: &Attachment, _: &Path, _: &WorkerIo) -> Result<(), String> {
    Err("This build cannot open a USB link for String Volume Setup.".into())
}

#[cfg(test)]
mod tests;

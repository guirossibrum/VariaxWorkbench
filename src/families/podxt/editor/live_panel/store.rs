use super::*;
use podxt::{ConfirmedStore, PreparedStore, StoreControl, StoreResult, StoreSource};

// #222 implements this path offline. The owner's separate store decision
// is required before the native entry point can open the rig.
const STORE_OWNER_APPROVED: bool = false;

#[derive(Default)]
pub(super) struct StorePanel {
    index: u16,
    prepared: Option<PreparedStore>,
    typed: String,
    worker: Option<StoreWorker>,
    status: Option<String>,
}

struct StoreWorker {
    control: Arc<StoreControl>,
    thread: Option<std::thread::JoinHandle<Result<StoreResult, String>>>,
}

impl Drop for StoreWorker {
    fn drop(&mut self) {
        self.control.cancel();
    }
}

#[cfg(any(test, target_os = "linux"))]
struct StorePolicy;

#[cfg(any(test, target_os = "linux"))]
impl GatePolicy for StorePolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::PodxtIdentity => !hardware_gates::is_locked(Gate::CurrentSettingsRead),
            Operation::PodxtLibraryRead => !hardware_gates::is_locked(Gate::FullBackupRead),
            Operation::PodxtStoreRestore => !hardware_gates::is_locked(Gate::StoreRestore),
            _ => false,
        }
    }
}

#[cfg(any(test, target_os = "linux"))]
fn store_transaction<G, L: Link>(
    confirmation: ConfirmedStore,
    control: &StoreControl,
    lock: impl FnOnce() -> Result<G, String>,
    open: impl FnOnce(&G) -> Result<L, String>,
) -> Result<StoreResult, String> {
    hardware_gates::run_if_unlocked(Gate::StoreRestore, || {
        hardware_gates::run_if_unlocked(Gate::FullBackupRead, || {
            with_session(
                || control.cancelled(),
                lock,
                open,
                &StorePolicy,
                |session| podxt::store_tone(session, confirmation, control),
            )
        })
    })
}

#[cfg(all(target_os = "linux", not(test)))]
fn platform_store(
    device: &detection::DeviceKey,
    confirmation: ConfirmedStore,
    control: &StoreControl,
) -> Result<StoreResult, String> {
    if !STORE_OWNER_APPROVED {
        return Err("PODxt store awaits separate owner approval.".into());
    }
    store_transaction(
        confirmation,
        control,
        || line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string()),
        |_| open_platform(device),
    )
}

#[cfg(any(test, not(target_os = "linux")))]
fn platform_store(
    _device: &detection::DeviceKey,
    _confirmation: ConfirmedStore,
    _control: &StoreControl,
) -> Result<StoreResult, String> {
    Err("PODxt store is unavailable in this build.".into())
}

fn admitted() -> bool {
    STORE_OWNER_APPROVED
        && cfg!(all(target_os = "linux", not(test)))
        && !hardware_gates::is_locked(Gate::StoreRestore)
        && !hardware_gates::is_locked(Gate::FullBackupRead)
        && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
        && !hardware_gates::is_locked(Gate::OsDiscovery)
}

impl StorePanel {
    pub(super) fn busy(&self) -> bool {
        self.worker.is_some()
    }

    pub(super) fn cancel(&self) {
        if let Some(worker) = &self.worker {
            worker.control.cancel();
        }
    }

    fn clear_preparation(&mut self) {
        self.prepared = None;
        self.typed.clear();
    }

    pub(super) fn detach(&mut self) {
        self.cancel();
        self.clear_preparation();
    }

    fn start(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        if !admitted() || self.busy() {
            return;
        }
        let Some(prepared) = self.prepared.take() else {
            return;
        };
        let confirmation = prepared.confirm(&self.typed);
        self.typed.clear();
        let confirmation = match confirmation {
            Ok(confirmation) => confirmation,
            Err(error) => {
                self.status = Some(error);
                return;
            }
        };
        let device = device.clone();
        let control = Arc::new(StoreControl::default());
        let thread_control = Arc::clone(&control);
        let repaint = ctx.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-store".into())
            .spawn(move || {
                let result = platform_store(&device, confirmation, &thread_control);
                repaint.request_repaint();
                result
            });
        match thread {
            Ok(thread) => {
                self.status = None;
                self.worker = Some(StoreWorker {
                    control,
                    thread: Some(thread),
                });
            }
            Err(error) => {
                self.status = Some(format!("Could not start store. No store was sent. {error}"))
            }
        }
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(worker) = &self.worker else {
            return;
        };
        if !worker.thread.as_ref().expect("store worker").is_finished() {
            ctx.request_repaint_after(REPAINT_WHILE_READING);
            return;
        }
        let mut worker = self.worker.take().expect("finished store worker");
        let result = worker.thread.take().expect("store worker").join();
        self.status = Some(match result {
            Ok(Ok(StoreResult::Verified { slot, .. })) => format!(
                "Slot {} acknowledged and verified byte for byte.",
                slot.label()
            ),
            Ok(Ok(StoreResult::SlotStateUnknown { slot, reason })) => {
                format!("Slot {} state unknown. {reason}", slot.label())
            }
            Ok(Err(error)) => error,
            Err(_) if worker.control.started() => {
                "Slot state unknown. The store worker stopped after dispatch.".into()
            }
            Err(_) => "The store worker stopped before dispatch.".into(),
        });
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        device: Option<&detection::DeviceKey>,
        other_idle: bool,
    ) {
        ui.collapsing("Store original to slot", |ui| {
            if !STORE_OWNER_APPROVED {
                ui.label("PODxt store is not approved. Backup verification is available offline.");
            }
            ui.add_enabled_ui(other_idle && !self.busy(), |ui| {
                let selected = Location::new(BankKind::Tones, self.index).expect("valid selection");
                egui::ComboBox::from_label("Destination slot")
                    .selected_text(selected.label())
                    .show_ui(ui, |ui| {
                        for slot in Location::all(&[BankKind::Tones]) {
                            ui.selectable_value(&mut self.index, slot.index(), slot.label());
                        }
                    });
                if self.index != selected.index() {
                    self.clear_preparation();
                    self.status = None;
                }
                if ui.button("Verify same-day backup").clicked()
                    && let Some(folder) = services.dialogs.pick_folder()
                {
                    self.clear_preparation();
                    let destination =
                        Location::new(BankKind::Tones, self.index).expect("valid selection");
                    match podxt::prepare_tone_store(&folder, destination, StoreSource::OriginalSlot)
                    {
                        Ok(prepared) => {
                            self.status = None;
                            self.prepared = Some(prepared);
                        }
                        Err(error) => self.status = Some(error),
                    }
                }
                if let Some(prepared) = &self.prepared {
                    let phrase = prepared.confirmation();
                    ui.label(format!("Verified backup · {}", prepared.original_name()));
                    ui.label(format!("Type {phrase} to confirm the destination."));
                    ui.text_edit_singleline(&mut self.typed);
                    let confirmed = self.typed == phrase && admitted();
                    if ui
                        .add_enabled(
                            confirmed && device.is_some(),
                            egui::Button::new("Store original to slot"),
                        )
                        .clicked()
                        && let Some(device) = device
                    {
                        self.start(device, ui.ctx());
                    }
                }
            });
            if let Some(worker) = &self.worker {
                if worker.control.started() {
                    ui.label("Store dispatched. Waiting for acknowledgement and readback.");
                } else if ui
                    .add_enabled(
                        !worker.control.cancelled(),
                        egui::Button::new("Cancel before store"),
                    )
                    .clicked()
                {
                    worker.control.cancel();
                }
            }
            if let Some(status) = &self.status {
                ui.label(status);
            }
        });
    }
}

#[cfg(test)]
mod tests;

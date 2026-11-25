use super::*;
use podxt::{ConfirmedStore, PreparedStore, StoreControl, StoreResult, StoreSource};

// #222 implements this path offline. The owner's separate store decision
// is required before the native entry point can open the rig.
const STORE_OWNER_APPROVED: bool = false;

const SETUP_STORE_OWNER_APPROVED: bool = false;
const SYNC_OWNER_APPROVED: bool = false;

#[derive(Clone, Copy)]
enum StoreStage {
    Tone,
    Setup,
    Sync,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StoreView {
    Original,
    Current,
    Sync,
}

impl StoreStage {
    fn original(bank: BankKind) -> Self {
        match bank {
            BankKind::Tones => Self::Tone,
            BankKind::AmpSetups | BankKind::FxSetups => Self::Setup,
        }
    }

    fn approved(self) -> bool {
        STORE_OWNER_APPROVED
            && match self {
                Self::Tone => true,
                Self::Setup => SETUP_STORE_OWNER_APPROVED,
                Self::Sync => SYNC_OWNER_APPROVED,
            }
    }
}

#[derive(Default)]
pub(super) struct StorePanel {
    index: u16,
    bank: usize,
    prepared: Option<(StoreStage, PreparedStore)>,
    typed: String,
    worker: Option<StoreWorker>,
    status: Option<String>,
}

struct StoreWorker {
    device: detection::DeviceKey,
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
    stage: StoreStage,
    confirmation: ConfirmedStore,
    control: &StoreControl,
) -> Result<StoreResult, String> {
    if !stage.approved() {
        return Err("This PODxt store stage awaits separate owner approval.".into());
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
    _stage: StoreStage,
    _confirmation: ConfirmedStore,
    _control: &StoreControl,
) -> Result<StoreResult, String> {
    Err("PODxt store is unavailable in this build.".into())
}

fn admitted(stage: StoreStage) -> bool {
    stage.approved()
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

    pub(super) fn clear_preparation(&mut self) {
        self.prepared = None;
        self.typed.clear();
    }

    pub(super) fn prepare_replacement(
        &mut self,
        directory: &std::path::Path,
        slot: Location,
        tone: podxt::TonePayload,
        original: &podxt::TonePayload,
    ) -> Result<(), String> {
        self.clear_preparation();
        let prepared = podxt::prepare_tone_store(directory, slot, StoreSource::Replacement(tone))?;
        if !prepared.original_matches(original) {
            return Err("The verified backup and the slot used to stage this edit differ.".into());
        }
        self.prepared = Some((StoreStage::Sync, prepared));
        self.status = None;
        Ok(())
    }

    pub(super) fn prepare_current(
        &mut self,
        directory: &std::path::Path,
        slot: Location,
        tone: Option<&EditBuffer>,
    ) {
        self.clear_preparation();
        let result = tone
            .ok_or("Refresh the current tone before storing.".to_owned())
            .and_then(podxt::TonePayload::from_dump)
            .and_then(|tone| {
                podxt::prepare_tone_store(directory, slot, StoreSource::Replacement(tone))
            });
        self.status = match result {
            Ok(prepared) => {
                // The existing replacement gate is required for current edits too.
                self.prepared = Some((StoreStage::Sync, prepared));
                None
            }
            Err(error) => Some(error),
        };
    }

    pub(super) fn detach(&mut self) {
        self.cancel();
        self.clear_preparation();
    }

    fn start(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        if self.busy()
            || !self
                .prepared
                .as_ref()
                .is_some_and(|(stage, _)| admitted(*stage))
        {
            return;
        }
        let Some((stage, prepared)) = self.prepared.take() else {
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
        let endpoint = device.clone();
        let control = Arc::new(StoreControl::default());
        let thread_control = Arc::clone(&control);
        let repaint = ctx.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-store".into())
            .spawn(move || {
                let result = platform_store(&endpoint, stage, confirmation, &thread_control);
                repaint.request_repaint();
                result
            });
        match thread {
            Ok(thread) => {
                self.status = None;
                self.worker = Some(StoreWorker {
                    device,
                    control,
                    thread: Some(thread),
                });
            }
            Err(error) => {
                self.status = Some(format!("Could not start store. No store was sent. {error}"))
            }
        }
    }

    pub(super) fn poll(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<(detection::DeviceKey, Result<podxt::StoredTone, ()>)> {
        let Some(worker) = &self.worker else {
            return None;
        };
        if !worker.thread.as_ref().expect("store worker").is_finished() {
            ctx.request_repaint_after(REPAINT_WHILE_READING);
            return None;
        }
        let mut worker = self.worker.take().expect("finished store worker");
        let result = worker.thread.take().expect("store worker").join();
        self.status = Some(match &result {
            Ok(Ok(StoreResult::Verified { slot, .. })) => format!(
                "Slot {} acknowledged and verified byte for byte.",
                slot.slot_name()
            ),
            Ok(Ok(StoreResult::SlotStateUnknown { slot, reason })) => {
                format!("Slot {} state unknown. {reason}", slot.slot_name())
            }
            Ok(Err(error)) => error.clone(),
            Err(_) if worker.control.started() => {
                "Slot state unknown. The store worker stopped after dispatch.".into()
            }
            Err(_) => "The store worker stopped before dispatch.".into(),
        });
        let outcome = match result {
            Ok(Ok(StoreResult::Verified { slot, tone })) => Ok(podxt::StoredTone {
                location: slot,
                tone,
            }),
            _ => Err(()),
        };
        Some((worker.device.clone(), outcome))
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        device: Option<&detection::DeviceKey>,
        other_idle: bool,
        view: StoreView,
    ) {
        ui.collapsing(
            match view {
                StoreView::Original => "Store original to slot",
                StoreView::Current => "Confirm Store to POD",
                StoreView::Sync => "Sync slot confirmation",
            },
            |ui| {
                let stage = if view != StoreView::Original {
                    StoreStage::Sync
                } else {
                    StoreStage::original(BankKind::ALL[self.bank])
                };
                if !stage.approved() {
                    ui.label(
                        "PODxt store is not approved. Backup verification is available offline.",
                    );
                }
                ui.add_enabled_ui(other_idle && !self.busy(), |ui| {
                    if view == StoreView::Original {
                        let old_bank = self.bank;
                        egui::ComboBox::from_label("Destination bank")
                            .selected_text(BankKind::ALL[self.bank].name())
                            .show_ui(ui, |ui| {
                                for (index, bank) in BankKind::ALL.into_iter().enumerate() {
                                    ui.selectable_value(&mut self.bank, index, bank.name());
                                }
                            });
                        if self.bank != old_bank {
                            self.index = 0;
                        }
                        let kind = BankKind::ALL[self.bank];
                        let selected = Location::new(kind, self.index).expect("valid selection");
                        egui::ComboBox::from_label("Destination slot")
                            .selected_text(selected.slot_name())
                            .show_ui(ui, |ui| {
                                for slot in Location::all(&[kind]) {
                                    ui.selectable_value(
                                        &mut self.index,
                                        slot.index(),
                                        slot.slot_name(),
                                    );
                                }
                            });
                        if self.index != selected.index() || self.bank != old_bank {
                            self.clear_preparation();
                            self.status = None;
                        }
                        if ui.button("Verify library backup").clicked() {
                            let destination =
                                Location::new(kind, self.index).expect("valid selection");
                            services.request_picker::<PodxtEditor>(
                                ui.ctx(),
                                PickRequest::Folder,
                                move |editor, folder, _| {
                                    let Some(folder) = folder else {
                                        return;
                                    };
                                    let live = &mut editor.live;
                                    let store = &mut live.store;
                                    if live.locker.syncing()
                                        || BankKind::ALL[store.bank] != destination.kind()
                                        || store.index != destination.index()
                                    {
                                        return;
                                    }
                                    store.clear_preparation();
                                    match podxt::prepare_tone_store(
                                        &folder,
                                        destination,
                                        StoreSource::OriginalSlot,
                                    ) {
                                        Ok(prepared) => {
                                            store.status = None;
                                            store.prepared = Some((
                                                StoreStage::original(destination.kind()),
                                                prepared,
                                            ));
                                        }
                                        Err(error) => store.status = Some(error),
                                    }
                                },
                            );
                        }
                    }
                    if let Some((stage, prepared)) = &self.prepared {
                        let phrase = prepared.confirmation();
                        ui.label(format!("Verified backup · {}", prepared.original_name()));
                        ui.label(format!("Candidate · {}", prepared.candidate_name()));
                        ui.label(format!("Type {phrase} to confirm the destination."));
                        ui.text_edit_singleline(&mut self.typed);
                        let confirmed = self.typed == phrase && admitted(*stage);
                        if ui
                            .add_enabled(
                                confirmed && device.is_some(),
                                egui::Button::new(match view {
                                    StoreView::Original => "Store original to slot",
                                    StoreView::Current => "Store to POD",
                                    StoreView::Sync => "Store selected slot",
                                }),
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
            },
        );
    }
}

#[cfg(test)]
mod tests;

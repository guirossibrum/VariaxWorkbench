use super::*;

mod edit;
mod locker;
mod store;
use line6_devices::family::podxt::{
    self, BankKind, Control, EditBuffer, GlobalValues, LiveControl, Location, Packs, ReadCommand,
    ReadUpdate, Setting, TunerReading,
};
#[cfg(any(test, target_os = "linux"))]
use line6_devices::gate::{GatePolicy, Operation};
#[cfg(any(test, target_os = "linux"))]
use line6_devices::session::{Link, Quiet, Session as MidiSession};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

const PULSE: Duration = Duration::from_millis(850);
const REPAINT_WHILE_READING: Duration = Duration::from_millis(80);

#[derive(Default)]
pub(super) struct LivePanel {
    device: Option<detection::DeviceKey>,
    device_tab: bool,
    worker: Option<Worker>,
    editor: edit::LiveEditor,
    store: store::StorePanel,
    locker: locker::Locker,
    snapshot: Snapshot,
    show_values: bool,
    startup: Option<Startup>,
    pub(super) live_editing: bool,
    panel: Option<StagePanel>,
    scroll_to_panel: bool,
    message: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StagePanel {
    Locker,
    Store,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Startup {
    Current,
    Tones,
}

struct Worker {
    command: ReadCommand,
    device: detection::DeviceKey,
    stop: Arc<AtomicBool>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Clone, Default)]
struct Snapshot {
    tone: Option<EditBuffer>,
    library: std::collections::BTreeMap<u16, EditBuffer>,
    library_count: Option<usize>,
    packs: Option<Packs>,
    globals: Option<GlobalValues>,
    tuner: Option<TunerReading>,
    channel: Option<Location>,
    clip: Option<Instant>,
    tap: Option<Instant>,
    warning: Option<String>,
}

impl Snapshot {
    fn apply(&mut self, update: ReadUpdate, now: Instant) {
        match update {
            ReadUpdate::Current(tone) => self.tone = Some(tone),
            ReadUpdate::LibraryStarted => {
                self.library.clear();
                self.library_count = Some(0);
            }
            ReadUpdate::Stored(stored) => {
                self.library.insert(stored.location.address(), stored.tone);
                if let Some(count) = &mut self.library_count {
                    *count = self.library.len();
                }
            }
            ReadUpdate::Packs(packs) => self.packs = Some(packs),
            ReadUpdate::Globals(values) => self.globals = Some(values),
            ReadUpdate::Tuner(reading) => self.tuner = Some(reading),
            ReadUpdate::Controller { number, value } => {
                let number = if number == 11 { 12 } else { number };
                if number < 128
                    && value < 128
                    && let Some(byte) = self
                        .tone
                        .as_mut()
                        .and_then(|tone| tone.raw.get_mut(39 + usize::from(number)))
                {
                    *byte = value;
                }
            }
            ReadUpdate::Clip => self.clip = Some(now + PULSE),
            ReadUpdate::Tap => self.tap = Some(now + PULSE),
            ReadUpdate::Channel(location) => self.channel = Some(location),
            ReadUpdate::FollowWarning(message) => self.warning = Some(message),
        }
    }
}

#[cfg(any(test, target_os = "linux"))]
struct ReadPolicy(ReadCommand);

#[cfg(any(test, target_os = "linux"))]
impl GatePolicy for ReadPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::PodxtIdentity => !hardware_gates::is_locked(Gate::CurrentSettingsRead),
            Operation::PodxtRead => {
                matches!(self.0, ReadCommand::Current | ReadCommand::Follow)
                    && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            }
            Operation::PodxtPackQuery => {
                self.0 == ReadCommand::Packs
                    && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            }
            Operation::PodxtGlobalRead => {
                matches!(self.0, ReadCommand::Globals | ReadCommand::Tuner)
                    && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            }
            Operation::PodxtTunerMode => {
                self.0 == ReadCommand::Tuner
                    && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            }
            Operation::PodxtLibraryRead => {
                matches!(
                    self.0,
                    ReadCommand::Library | ReadCommand::ToneBank | ReadCommand::Follow
                ) && !hardware_gates::is_locked(Gate::FullBackupRead)
            }
            _ => false,
        }
    }
}

fn requires_library(command: ReadCommand) -> bool {
    matches!(command, ReadCommand::Library | ReadCommand::ToneBank)
}

/// The guard outlives the link, including cleanup on errors.
#[cfg(any(test, target_os = "linux"))]
fn with_session<G, L: Link, T>(
    cancelled: impl Fn() -> bool,
    lock: impl FnOnce() -> Result<G, String>,
    open: impl FnOnce(&G) -> Result<L, String>,
    policy: &dyn GatePolicy,
    run: impl FnOnce(&mut MidiSession<'_, L>) -> Result<T, String>,
) -> Result<T, String> {
    hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, || {
            let check_stop = || {
                if cancelled() {
                    Err("PODxt Live operation cancelled.".into())
                } else {
                    Ok::<(), String>(())
                }
            };
            check_stop()?;
            let rig = lock()?;
            check_stop()?;
            let link = open(&rig)?;
            let mut quiet = Quiet;
            let mut session = MidiSession::new(link, policy, &mut quiet);
            session.log_packets = false;
            run(&mut session)
        })
    })
}

#[cfg(any(test, target_os = "linux"))]
fn transaction<G, L: Link>(
    command: ReadCommand,
    stop: &AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    open: impl FnOnce(&G) -> Result<L, String>,
    publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    let operation = || {
        let cancelled = || stop.load(Ordering::Acquire);
        with_session(cancelled, lock, open, &ReadPolicy(command), |session| {
            podxt::read_live(session, command, cancelled, publish)
        })
    };
    if requires_library(command)
        || command == ReadCommand::Follow && !hardware_gates::is_locked(Gate::FullBackupRead)
    {
        hardware_gates::run_if_unlocked(Gate::FullBackupRead, operation)
    } else {
        operation()
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn open_platform(
    device: &detection::DeviceKey,
) -> Result<line6_devices::transport::rawmidi::RawMidiLink, String> {
    use line6_devices::{
        family::podxt::transport::PODXT_LIVE_MIDI, transport::rawmidi::RawMidiLink,
    };
    use variax_workbench::device_probe::{LinuxSysfs, Recognition, recognise};

    let Recognition::Recognised {
        device: detected,
        sound_cards,
    } = recognise(&LinuxSysfs::new())
    else {
        return Err("The verified PODxt Live attachment is no longer available.".into());
    };
    if detected.sysfs_name != device.attachment.path {
        return Err("The PODxt Live attachment changed. Wait for automatic identity.".into());
    }
    let mut cards = sound_cards
        .iter()
        .filter(|card| card.id == PODXT_LIVE_MIDI.card_id);
    let (Some(card), None) = (cards.next(), cards.next()) else {
        return Err("The verified PODxt Live has no unique ALSA MIDI card.".into());
    };
    RawMidiLink::open_device(
        PODXT_LIVE_MIDI.device_node(card.index),
        PODXT_LIVE_MIDI.path,
    )
}

#[cfg(all(target_os = "linux", not(test)))]
fn platform_read(
    device: &detection::DeviceKey,
    command: ReadCommand,
    stop: &AtomicBool,
    publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    transaction(
        command,
        stop,
        || line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string()),
        |_| open_platform(device),
        publish,
    )
}

#[cfg(any(test, not(all(target_os = "linux", feature = "usb"))))]
fn platform_read(
    _device: &detection::DeviceKey,
    _command: ReadCommand,
    _stop: &AtomicBool,
    _publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    Err("PODxt Live reads are unavailable in this build.".into())
}

const fn title(command: ReadCommand) -> &'static str {
    match command {
        ReadCommand::Current => "Read current tone",
        ReadCommand::Library => "Read library",
        ReadCommand::ToneBank => "Read tone channels",
        ReadCommand::Packs => "Read installed packs",
        ReadCommand::Globals => "Read globals",
        ReadCommand::Tuner => "Start tuner",
        ReadCommand::Follow => "Follow the POD",
    }
}

impl LivePanel {
    /// True when a new verified attachment should take over the stage.
    pub(super) fn set_device(&mut self, device: Option<&tabs::DeviceTab>) -> bool {
        self.device_tab = device.is_some();
        let verified = device
            .filter(|tab| {
                tab.connection == tabs::Connection::Present
                    && tab.device.key.family_id == "podxt"
                    && tab.device.support == detection::DeviceSupport::Verified("3.01")
            })
            .map(|tab| &tab.device.key);
        if self.device.as_ref() == verified {
            return false;
        }
        self.cancel();
        self.editor.detach();
        self.store.detach();
        self.locker.detach(&mut self.store);
        self.device = verified.cloned();
        self.startup = self.device.as_ref().map(|_| Startup::Current);
        self.live_editing = false;
        if self.panel == Some(StagePanel::Store) {
            self.panel = None;
        }
        self.editor.set_live(false);
        self.snapshot = Snapshot::default();
        self.message = None;
        self.device.is_some()
    }

    pub(super) fn playing_channel(&self) -> Option<Location> {
        self.snapshot.channel
    }

    pub(super) fn visible(&self) -> bool {
        self.device_tab
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || self.editor.busy() || self.store.busy()
    }

    pub(super) fn cancel(&mut self) {
        self.startup = None;
        self.editor.cancel();
        self.store.cancel();
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Release);
        }
    }

    fn start(&mut self, command: ReadCommand, ctx: &egui::Context) {
        let Some(device) = self.device.clone() else {
            return;
        };
        if self.busy() || !Self::admitted(command) {
            return;
        }
        if command == ReadCommand::Follow {
            self.store.clear_preparation();
            self.editor.set_live(false);
            self.live_editing = false;
        }
        let mut previous = self.snapshot.clone();
        previous.warning = None;
        previous.clip = None;
        previous.tap = None;
        previous.tuner = None;
        if requires_library(command) {
            previous.library_count = None;
        }
        let snapshot = Arc::new(Mutex::new(previous));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_snapshot = Arc::clone(&snapshot);
        let repaint = ctx.clone();
        let endpoint = device.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-read".into())
            .spawn(move || {
                platform_read(&endpoint, command, &worker_stop, |update| {
                    if !worker_stop.load(Ordering::Acquire) {
                        worker_snapshot
                            .lock()
                            .expect("snapshot mutex")
                            .apply(update, Instant::now());
                        repaint.request_repaint();
                    }
                })
            });
        match thread {
            Ok(thread) => {
                self.message = None;
                self.worker = Some(Worker {
                    command,
                    device,
                    thread: Some(thread),
                    snapshot,
                    stop,
                });
            }
            Err(error) => self.message = Some(format!("Could not start PODxt read: {error}")),
        }
    }

    fn admitted(command: ReadCommand) -> bool {
        cfg!(all(target_os = "linux", feature = "usb", not(test)))
            && !hardware_gates::is_locked(Gate::OsDiscovery)
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            && (!requires_library(command) || !hardware_gates::is_locked(Gate::FullBackupRead))
    }

    fn next_startup(&mut self) -> Option<Startup> {
        if self.busy() {
            return None;
        }
        let request = self.startup.take()?;
        if request == Startup::Current {
            self.startup = Some(Startup::Tones);
        }
        Some(request)
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        if let Some((device, result)) = self.store.poll(ctx) {
            let result = if self.device.as_ref() == Some(&device) {
                result
            } else {
                Err(())
            };
            self.locker
                .result(result, &mut self.snapshot, &mut self.store);
        }
        if let Some(buffer) = self.editor.poll(
            self.device.as_ref(),
            self.worker.is_none() && !self.store.busy(),
            ctx,
        ) {
            if self.panel == Some(StagePanel::Store)
                && (self.snapshot.tone.as_ref() != Some(&buffer.tone)
                    || self.snapshot.channel != Some(buffer.channel))
            {
                self.store.clear_preparation();
            }
            self.snapshot.tone = Some(buffer.tone.clone());
            self.snapshot.packs = Some(buffer.packs);
            self.snapshot.channel = Some(buffer.channel);
        }
        let Some(worker) = &self.worker else {
            match self.next_startup() {
                Some(Startup::Current) => {
                    self.refresh_current(ctx);
                    ctx.request_repaint();
                }
                Some(Startup::Tones) => self.start(ReadCommand::ToneBank, ctx),
                None => {}
            }
            return;
        };
        if self.device.as_ref() == Some(&worker.device) {
            self.snapshot = worker.snapshot.lock().expect("snapshot mutex").clone();
        }
        if !worker.thread.as_ref().expect("worker thread").is_finished() {
            ctx.request_repaint_after(REPAINT_WHILE_READING);
            return;
        }
        let mut worker = self.worker.take().expect("finished worker");
        // Leave the worker's stop guard in place until the thread has closed the link.
        // Taking it via Option makes Drop remain the cancellation path on early close.
        let command = worker.command;
        let same_device = self.device.as_ref() == Some(&worker.device);
        let stopped = worker.stop.load(Ordering::Acquire);
        if command == ReadCommand::Tuner {
            self.snapshot.tuner = None;
        }
        let result = worker
            .thread
            .take()
            .expect("worker thread")
            .join()
            .unwrap_or_else(|_| Err("PODxt reader stopped unexpectedly.".into()));
        if !same_device {
            return;
        }
        self.message = Some(match result {
            Ok(()) if requires_library(command) => format!(
                "Library read complete. {} entries.",
                self.snapshot.library.len()
            ),
            Ok(()) if command == ReadCommand::Tuner => "Tuner stopped.".into(),
            Ok(()) => format!("{} complete.", title(command)),
            Err(error) if stopped && requires_library(command) => {
                format!(
                    "Library read stopped after {} entries. {error}",
                    self.snapshot.library.len()
                )
            }
            Err(error) => error,
        });
        if command == ReadCommand::Follow {
            self.refresh_current(ctx);
        }
    }

    pub(super) fn connected(&self) -> bool {
        self.device.is_some()
    }

    pub(super) fn has_current(&self) -> bool {
        self.snapshot.tone.is_some()
    }

    pub(super) fn stage_tone(&self) -> Result<Option<(pod_preset::PodTone, u8)>, String> {
        let Some(dump) = self.snapshot.tone.as_ref() else {
            return Ok(None);
        };
        let payload = self
            .editor
            .stage_payload(dump)
            .ok_or_else(|| format!("{} · The edit buffer could not be decoded.", dump.name()))?;
        let tone =
            pod_preset::PodTone::from_podxt_payload(&payload, pod_preset::ContainerKind::L6pa)
                .map_err(|error| format!("{} · {}", dump.name(), error.detail()))?;
        Ok(Some((tone, self.snapshot.packs.map_or(0, |packs| packs.0))))
    }

    pub(super) fn use_document_directory(&mut self, path: Option<&std::path::Path>) {
        self.locker.use_document_directory(path);
    }

    pub(super) fn stage_footer(&self) -> &str {
        self.editor
            .status_text()
            .or(self.message.as_deref())
            .unwrap_or("Current POD tone")
    }

    pub(super) fn has_library_bank(&self, bank: BankKind) -> bool {
        Location::all(&[bank]).any(|slot| self.stored_tone(slot).is_some())
    }

    pub(super) fn channel_modified(&self, slot: Location) -> bool {
        self.snapshot.channel == Some(slot)
            && (self.editor.has_pending()
                || self
                    .snapshot
                    .tone
                    .as_ref()
                    .zip(self.stored_tone(slot))
                    .is_some_and(|(playing, saved)| !playing.same_tone(saved)))
    }

    pub(super) fn stored_tone(&self, slot: Location) -> Option<&EditBuffer> {
        self.snapshot.library.get(&slot.address())
    }

    pub(super) fn refresh_current(&mut self, ctx: &egui::Context) {
        if !self.busy()
            && let Some(device) = &self.device
        {
            self.editor.get(device, ctx);
        }
    }

    pub(super) fn queue_stage_target(&mut self, target: podxt::LiveTarget, ctx: &egui::Context) {
        if self.device.is_some() && self.worker.is_none() && !self.store.busy() {
            self.editor.queue_stage_target(target, ctx);
        }
    }

    pub(super) fn can_change_control(&self, control: LiveControl, block: &str) -> bool {
        self.device.is_some()
            && self.worker.is_none()
            && !self.store.busy()
            && self.editor.can_change(control, block)
    }

    pub(super) fn recall(&mut self, slot: Location, ctx: &egui::Context) {
        if self.live_editing
            && !self.busy()
            && let Some(device) = &self.device
        {
            self.editor.recall(slot, device, ctx);
        }
    }

    pub(super) fn locker_dirty(&self) -> bool {
        self.locker.dirty()
    }

    pub(super) fn discard_locker(&mut self) {
        self.locker.discard(&mut self.store);
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, current: &mut bool) {
        let command_running = self.worker.as_ref().map(|worker| worker.command);
        let idle = !self.busy();
        let connected = self.connected();
        if *current
            && ui
                .add_enabled(
                    connected && idle && !self.editor.has_pending() && self.editor.stage_editable(),
                    egui::Checkbox::new(&mut self.live_editing, "Live editing"),
                )
                .changed()
        {
            if let Some(device) = self.device.as_ref().filter(|_| self.live_editing) {
                self.editor.begin_live(device, ui.ctx());
            } else {
                self.editor.set_live(false);
            }
        }
        if *current {
            self.editor.toolbar(
                ui,
                self.device.as_ref(),
                self.worker.is_none() && !self.store.busy(),
            );
        }
        for (command, idle_label, active_label) in [
            (ReadCommand::Tuner, "Tuner", "Stop tuner"),
            (ReadCommand::Follow, "Follow", "Stop following"),
        ] {
            if !self.visible() {
                continue;
            }
            let running = command_running == Some(command);
            let ready = connected
                && idle
                && Self::admitted(command)
                && (command != ReadCommand::Follow
                    || self.snapshot.channel.is_some() && !self.editor.has_pending());
            if ui
                .add_enabled(
                    running || ready,
                    egui::Button::new(if running { active_label } else { idle_label }),
                )
                .clicked()
            {
                if running {
                    self.cancel();
                } else {
                    self.start(command, ui.ctx());
                }
            }
        }
        for (panel, label, enabled) in [
            (StagePanel::Locker, "Tone Locker · Librarian", true),
            (
                StagePanel::Store,
                "Store to POD…",
                *current
                    && idle
                    && !self.locker.syncing()
                    && !self.editor.has_pending()
                    && self.snapshot.channel.is_some(),
            ),
            (StagePanel::Advanced, "Advanced", true),
        ] {
            if panel != StagePanel::Locker && !self.visible()
                || panel == StagePanel::Store && !*current
            {
                continue;
            }
            let selected = self.panel == Some(panel);
            if ui
                .add_enabled(
                    enabled || selected,
                    egui::Button::selectable(selected, label),
                )
                .on_hover_text(if panel == StagePanel::Locker {
                    "Browse local files and PODxt banks. CustomTone was a separate online service."
                } else {
                    ""
                })
                .clicked()
            {
                if self.panel == Some(StagePanel::Store) || panel == StagePanel::Store {
                    self.store.clear_preparation();
                }
                self.panel = (!selected).then_some(panel);
                self.scroll_to_panel = self.panel.is_some();
            }
        }
    }

    pub(super) fn stage_status(&self, ui: &mut egui::Ui) {
        if !self.visible() {
            return;
        }
        if let Some(reading) = self.snapshot.tuner {
            let note = reading.note.map_or("—", podxt::tuner_note_name);
            let deviation = reading.deviation.map(podxt::tuner_cents);
            ui.label(deviation.map_or_else(
                || format!("Tuner {note}"),
                |cents| format!("Tuner {note} · {cents:+.0} cents"),
            ));
        }
        if let Some(worker) = &self.worker
            && requires_library(worker.command)
            && let Some(count) = self.snapshot.library_count
        {
            let total = if worker.command == ReadCommand::ToneBank {
                128
            } else {
                299
            };
            ui.weak(format!("Reading library · {count}/{total}"));
            if ui.button("Cancel library read").clicked() {
                worker.stop.store(true, Ordering::Release);
            }
        }
        self.editor.status(ui);
        if let Some(message) = &self.message {
            ui.weak(message);
        }
    }

    pub(super) fn panels(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        file_tone: Option<&pod_preset::PodTone>,
    ) -> Option<Vec<u8>> {
        let panel = self.panel?;
        let view = ui.scope(|ui| match panel {
            StagePanel::Locker => {
                let idle = !self.busy() && !self.editor.has_pending();
                let actions = self.locker.show(
                    ui,
                    services,
                    locker::View {
                        snapshot: &self.snapshot,
                        idle,
                        can_get: idle && self.connected() && Self::admitted(ReadCommand::Library),
                        editor_tone: file_tone,
                    },
                    &mut self.store,
                );
                if actions.get {
                    self.start(ReadCommand::Library, ui.ctx());
                }
                if self.locker.syncing() {
                    self.store.show(
                        ui,
                        services,
                        self.device.as_ref(),
                        idle,
                        store::StoreView::Sync,
                    );
                }
                actions.open
            }
            StagePanel::Store => {
                self.show_current_store(ui, services);
                None
            }
            StagePanel::Advanced => {
                self.show_advanced(ui, services);
                if !self.locker.syncing() {
                    self.store.show(
                        ui,
                        services,
                        self.device.as_ref(),
                        self.worker.is_none() && !self.editor.busy() && !self.editor.has_pending(),
                        store::StoreView::Original,
                    );
                }
                None
            }
        });
        if std::mem::take(&mut self.scroll_to_panel) {
            view.response.scroll_to_me(Some(egui::Align::Min));
        }
        view.inner
    }

    fn show_current_store(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        let Some(slot) = self.snapshot.channel else {
            return;
        };
        let idle = !self.busy() && !self.editor.has_pending();
        ui.group(|ui| {
            ui.strong(format!("Store to POD · {}", slot.slot_name()));
            if ui
                .add_enabled(
                    idle && self.snapshot.tone.is_some(),
                    egui::Button::new("Verify library backup…"),
                )
                .clicked()
            {
                services.request_picker::<PodxtEditor>(
                    ui.ctx(),
                    PickRequest::Folder,
                    move |editor, path, _| {
                        let Some(path) = path else {
                            return;
                        };
                        let live = &mut editor.live;
                        if live.panel == Some(StagePanel::Store)
                            && !live.locker.syncing()
                            && !live.busy()
                            && !live.editor.has_pending()
                            && live.snapshot.channel == Some(slot)
                        {
                            live.store
                                .prepare_current(&path, slot, live.snapshot.tone.as_ref());
                        }
                    },
                );
            }
            self.store.show(
                ui,
                services,
                self.device.as_ref(),
                idle,
                store::StoreView::Current,
            );
        });
    }

    fn show_advanced(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        ui.heading("PODxt Live reads");
        ui.horizontal_wrapped(|ui| {
            for command in [
                ReadCommand::Current,
                ReadCommand::Library,
                ReadCommand::Packs,
                ReadCommand::Globals,
            ] {
                let enabled = self.device.is_some() && !self.busy() && Self::admitted(command);
                if ui
                    .add_enabled(enabled, egui::Button::new(title(command)))
                    .clicked()
                {
                    self.start(command, ui.ctx());
                }
            }
            if let Some(worker) = &self.worker {
                let label = match worker.command {
                    ReadCommand::Tuner => "Stop tuner",
                    ReadCommand::Follow => "Stop following",
                    _ => "Cancel read",
                };
                if ui
                    .add_enabled(
                        !worker.stop.load(Ordering::Acquire),
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    self.cancel();
                }
            }
        });
        if self.device.is_none() {
            ui.weak("Live reads require automatic PODxt Live 3.01 identity.");
        } else if !cfg!(all(target_os = "linux", feature = "usb", not(test))) {
            ui.weak("Live reads are unavailable in this build.");
        }
        if let Some(worker) = &self.worker {
            if worker.stop.load(Ordering::Acquire) {
                ui.weak("Finishing the current device operation…");
            } else {
                ui.weak(match worker.command {
                    ReadCommand::Follow => "Following the POD · rig lock held",
                    ReadCommand::Tuner => "Tuning · rig lock held",
                    _ => "Reading from the PODxt Live…",
                });
            }
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }
        if let Some(warning) = &self.snapshot.warning {
            ui.label(warning);
        }
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.command == ReadCommand::Library)
            && let Some(count) = self.snapshot.library_count
        {
            let total: u16 = BankKind::ALL.into_iter().map(BankKind::size).sum();
            ui.add(
                egui::ProgressBar::new(count as f32 / f32::from(total))
                    .text(format!("{} of {total}", count)),
            );
        }
        let now = Instant::now();
        ui.horizontal_wrapped(|ui| {
            if let Some(until) = self.snapshot.clip.filter(|&until| until > now) {
                ui.colored_label(Color32::LIGHT_RED, "Clip");
                ui.ctx()
                    .request_repaint_after(until.saturating_duration_since(now));
            }
            if let Some(until) = self.snapshot.tap.filter(|&until| until > now) {
                ui.label("Tap");
                ui.ctx()
                    .request_repaint_after(until.saturating_duration_since(now));
            }
            if let Some(channel) = self.snapshot.channel {
                ui.label(format!("Playing {}", channel.label()));
            }
        });
        if let Some(reading) = self.snapshot.tuner {
            let note = reading.note.map_or("—", podxt::tuner_note_name);
            let deviation = reading.deviation.map(podxt::tuner_cents);
            ui.label(deviation.map_or_else(
                || format!("Tuner {note}"),
                |cents| format!("Tuner {note} · {cents:+.0} cents"),
            ));
        }
        if let Some(packs) = self.snapshot.packs {
            let names = packs.names();
            if names.is_empty() {
                ui.label("No recognized model packs installed");
            } else {
                ui.label(names.join(", "));
            }
        }
        if let Some(values) = &self.snapshot.globals {
            ui.collapsing("Global settings", |ui| {
                for (field, value) in values {
                    ui.label(format!(
                        "{} · {}",
                        field.name(),
                        value.map_or("No reply".into(), |raw| podxt::signed(raw).to_string())
                    ));
                }
            });
        }
        if let Some(tone) = &self.snapshot.tone {
            tone_panel(ui, "Current tone", tone, &mut self.show_values);
        }
        self.editor.show(
            ui,
            services,
            self.device.as_ref(),
            self.worker.is_none() && !self.store.busy(),
        );
    }
}

fn tone_panel(ui: &mut egui::Ui, heading: &str, tone: &EditBuffer, show_values: &mut bool) {
    ui.label(format!("{heading} · {}", tone.name()));
    ui.label(format!(
        "{} · {} · {} · Drive {:.0}% · {:.1} BPM",
        tone.amp().name,
        tone.cab().name,
        tone.reverb().name,
        tone.knob(13) * 100.0,
        tone.tempo_bpm()
    ));
    ui.checkbox(show_values, "Show control values");
    if *show_values {
        ui.push_id(heading, |ui| {
            for control in Control::ALL
                .into_iter()
                .map(LiveControl::Basic)
                .chain(Setting::ALL.into_iter().map(LiveControl::Additional))
            {
                ui.label(format!(
                    "{} · {}",
                    control.name(),
                    control.describe(control.read(tone))
                ));
            }
        });
    }
}

#[cfg(test)]
mod tests;

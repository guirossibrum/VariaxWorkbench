use super::*;

mod edit;
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
    snapshot: Snapshot,
    selected: Option<Location>,
    bank: usize,
    show_values: bool,
    message: Option<String>,
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
                matches!(self.0, ReadCommand::Library | ReadCommand::Follow)
                    && !hardware_gates::is_locked(Gate::FullBackupRead)
            }
            _ => false,
        }
    }
}

fn requires_library(command: ReadCommand) -> bool {
    command == ReadCommand::Library
}

/// The guard outlives the link, including cleanup on errors.
#[cfg(any(test, target_os = "linux"))]
fn with_session<G, L: Link, T>(
    stop: &AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    open: impl FnOnce(&G) -> Result<L, String>,
    policy: &dyn GatePolicy,
    run: impl FnOnce(&mut MidiSession<'_, L>) -> Result<T, String>,
) -> Result<T, String> {
    hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, || {
            let check_stop = || {
                if stop.load(Ordering::Acquire) {
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
        with_session(stop, lock, open, &ReadPolicy(command), |session| {
            podxt::read_live(session, command, || stop.load(Ordering::Acquire), publish)
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
        ReadCommand::Packs => "Read installed packs",
        ReadCommand::Globals => "Read globals",
        ReadCommand::Tuner => "Start tuner",
        ReadCommand::Follow => "Follow the POD",
    }
}

impl LivePanel {
    pub(super) fn set_device(&mut self, device: Option<&tabs::DeviceTab>) {
        self.device_tab = device.is_some();
        let verified = device
            .filter(|tab| {
                tab.connection == tabs::Connection::Present
                    && tab.device.key.family_id == "podxt"
                    && tab.device.support == detection::DeviceSupport::Verified("3.01")
            })
            .map(|tab| &tab.device.key);
        if self.device.as_ref() == verified {
            return;
        }
        self.cancel();
        self.editor.detach();
        self.device = verified.cloned();
        self.snapshot = Snapshot::default();
        self.selected = None;
        self.message = None;
    }

    pub(super) fn visible(&self) -> bool {
        self.device_tab
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || self.editor.busy()
    }

    pub(super) fn cancel(&mut self) {
        self.editor.cancel();
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
        let mut previous = self.snapshot.clone();
        previous.warning = None;
        previous.clip = None;
        previous.tap = None;
        previous.tuner = None;
        if command == ReadCommand::Library {
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

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        if let Some(buffer) = self
            .editor
            .poll(self.device.as_ref(), self.worker.is_none(), ctx)
        {
            self.snapshot.tone = Some(buffer.tone.clone());
            self.snapshot.packs = Some(buffer.packs);
            self.snapshot.channel = Some(buffer.channel);
        }
        let Some(worker) = &self.worker else {
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
            Ok(()) if command == ReadCommand::Library => format!(
                "Library read complete. {} entries.",
                self.snapshot.library.len()
            ),
            Ok(()) if command == ReadCommand::Tuner => "Tuner stopped.".into(),
            Ok(()) => format!("{} complete.", title(command)),
            Err(error) if stopped && command == ReadCommand::Library => {
                format!(
                    "Library read stopped after {} entries. {error}",
                    self.snapshot.library.len()
                )
            }
            Err(error) => error,
        });
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("PODxt Live reads");
        ui.horizontal_wrapped(|ui| {
            for command in [
                ReadCommand::Current,
                ReadCommand::Library,
                ReadCommand::Packs,
                ReadCommand::Globals,
                ReadCommand::Tuner,
                ReadCommand::Follow,
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
        self.editor
            .show(ui, self.device.as_ref(), self.worker.is_none());
        self.show_library(ui);
    }

    fn show_library(&mut self, ui: &mut egui::Ui) {
        if self.snapshot.library.is_empty() {
            return;
        }
        ui.collapsing("Device library", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (index, bank) in BankKind::ALL.into_iter().enumerate() {
                    ui.selectable_value(&mut self.bank, index, bank.name());
                }
            });
            let bank = BankKind::ALL[self.bank];
            let entries: Vec<_> = Location::all(&[bank])
                .filter_map(|location| {
                    self.snapshot
                        .library
                        .get(&location.address())
                        .map(|tone| (location, tone))
                })
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("podxt-device-library")
                .max_height(175.0)
                .show_rows(
                    ui,
                    ui.text_style_height(&egui::TextStyle::Body),
                    entries.len(),
                    |ui, range| {
                        for &(location, tone) in &entries[range] {
                            ui.selectable_value(
                                &mut self.selected,
                                Some(location),
                                format!("{} · {}", location.label(), tone.name()),
                            );
                        }
                    },
                );
            if let Some(location) = self.selected.filter(|location| location.kind() == bank)
                && let Some(tone) = self.snapshot.library.get(&location.address())
            {
                tone_panel(ui, "Stored tone preview", tone, &mut self.show_values);
            }
        });
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

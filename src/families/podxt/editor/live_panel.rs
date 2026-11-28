use super::*;
use tabs::{SyncAdmission, SyncUpdate};
#[cfg(all(not(target_os = "linux"), not(test), feature = "usb"))]
use variax_workbench::hardware_gates::Gate;

mod edit;
mod follow;
mod locker;
mod store;
mod tuner;
use line6_devices::family::podxt::{
    self, BankKind, Control, EditBuffer, GlobalValues, LiveControl, Location, Packs, ReadCommand,
    ReadUpdate, Setting, TunerReading,
};
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use line6_devices::gate::{GatePolicy, Operation};
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use line6_devices::session::{Link, Quiet, Session as MidiSession};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

const PENDING_REPLY: &str = "A PODxt reply may still be pending or the link did not close cleanly. Reconnect the PODxt Live before using device actions.";
const PULSE: Duration = Duration::from_millis(850);
const REPAINT_WHILE_READING: Duration = Duration::from_millis(80);

#[derive(Default)]
pub(super) struct LivePanel {
    device: Option<detection::DeviceKey>,
    device_tab: bool,
    worker: Option<Worker>,
    tuner: tuner::Tuner,
    editor: edit::LiveEditor,
    follow: follow::Follow,
    save_ingress: Arc<detection::PodxtSaveIngress>,
    resume_live: bool,
    reveal_playing: bool,
    live_since: Option<Instant>,
    linux_program_seen: Option<Instant>,
    linux_follow_due: Option<(Instant, Location)>,
    linux_follow_read: bool,
    store: store::StorePanel,
    locker: locker::Locker,
    snapshot: Snapshot,
    historical_library: std::collections::BTreeMap<u16, EditBuffer>,
    historical_bank: Option<(detection::DeviceKey, usize)>,
    show_values: bool,
    startup: Option<Startup>,
    sync_result: Option<Result<(), String>>,
    pub(super) live_editing: bool,
    panel: Option<StagePanel>,
    scroll_to_panel: bool,
    message: Option<String>,
    // Count intent before admission so the test transport cannot mask it.
    #[cfg(test)]
    device_requests: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StagePanel {
    Store,
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
    acquired: Option<Arc<AtomicBool>>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl Worker {
    fn request_stop(&self) {
        let _ = self
            .stop
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed);
    }

    fn library_progress(&self) -> Option<usize> {
        self.snapshot.lock().expect("snapshot mutex").library_count
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.request_stop();
        }
    }
}

#[derive(Clone, Default)]
struct Snapshot {
    tone: Option<EditBuffer>,
    library: std::collections::BTreeMap<u16, EditBuffer>,
    library_count: Option<usize>,
    library_save_baseline: u64,
    packs: Option<Packs>,
    globals: Option<GlobalValues>,
    tuner: Option<tuner::Sample>,
    tuner_mode: bool,
    channel: Option<Location>,
    clip: Option<Instant>,
    tap: Option<Instant>,
    warning: Option<String>,
}

impl Snapshot {
    fn complete_tone_bank(&self) -> Option<usize> {
        self.library_count.filter(|_| {
            Location::all(&[BankKind::Tones]).all(|slot| self.library.contains_key(&slot.address()))
        })
    }

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
            ReadUpdate::Tuner(pair) => {
                self.tuner = Some(tuner::Sample {
                    pair,
                    received_at: now,
                });
            }
            ReadUpdate::TunerMode(active) => self.tuner_mode = active,
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

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
struct ReadPolicy(ReadCommand);

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
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

fn platform_available() -> bool {
    !cfg!(test) && variax_workbench::podxt_live::workbench_link_available()
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
type RunOnLink<'a, T> = Box<dyn FnOnce(&mut dyn Link) -> Result<T, String> + 'a>;

/// The guard outlives the scoped link, including cleanup on errors.
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn with_session<'a, G, T>(
    cancelled: impl Fn() -> bool,
    reply_pending: &AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    with_link: impl FnOnce(&G, RunOnLink<'a, T>) -> Result<T, String>,
    policy: impl GatePolicy + 'a,
    run: impl FnOnce(&mut MidiSession<'_, &mut dyn Link>) -> Result<T, String> + 'a,
) -> Result<T, String> {
    hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, || {
            let check_admission = || {
                if reply_pending.load(Ordering::Acquire) {
                    Err(PENDING_REPLY.into())
                } else if cancelled() {
                    Err("PODxt Live operation cancelled.".into())
                } else {
                    Ok::<(), String>(())
                }
            };
            check_admission()?;
            let rig = lock()?;
            check_admission()?;
            with_link(
                &rig,
                Box::new(move |link| {
                    let mut quiet = Quiet;
                    let mut session = MidiSession::new(link, &policy, &mut quiet);
                    session.log_packets = false;
                    run(&mut session)
                }),
            )
        })
    })
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn transaction<'a, G>(
    command: ReadCommand,
    stop: &'a AtomicBool,
    reply_pending: &'a AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    with_link: impl FnOnce(&G, RunOnLink<'a, ()>) -> Result<(), String>,
    publish: impl FnMut(ReadUpdate) + 'a,
) -> Result<(), String> {
    let operation = || {
        let cancelled = || stop.load(Ordering::Acquire);
        with_session(
            cancelled,
            reply_pending,
            lock,
            with_link,
            ReadPolicy(command),
            move |session| {
                track_reply(reply_pending, |outstanding| {
                    podxt::read_live_tracking_reply(
                        session,
                        command,
                        cancelled,
                        outstanding,
                        publish,
                    )
                })
            },
        )
    };
    if requires_library(command)
        || command == ReadCommand::Follow && !hardware_gates::is_locked(Gate::FullBackupRead)
    {
        hardware_gates::run_if_unlocked(Gate::FullBackupRead, operation)
    } else {
        operation()
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn track_reply<T>(
    reply_pending: &AtomicBool,
    run: impl FnOnce(&mut bool) -> Result<T, String>,
) -> Result<T, String> {
    let mut outstanding = false;
    let result = run(&mut outstanding);
    if outstanding {
        reply_pending.store(true, Ordering::Release);
    }
    result
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

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn with_platform_link<T>(
    device: &detection::DeviceKey,
    rig: &line6_devices::device_lock::DeviceLock,
    run: RunOnLink<'_, T>,
) -> Result<T, String> {
    #[cfg(target_os = "linux")]
    {
        let _ = rig;
        run(&mut open_platform(device)?)
    }
    #[cfg(target_os = "macos")]
    {
        let attachment = device
            .attachment
            .usb_location()
            .ok_or("The verified PODxt Live attachment is no longer available.")?;
        variax_workbench::podxt_live::with_workbench_link(rig, Some(attachment), |link, _| {
            run(link)
        })
    }
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn with_verified_platform_link<T>(
    device: &detection::DeviceKey,
    rig: &line6_devices::device_lock::DeviceLock,
    run: RunOnLink<'_, T>,
) -> Result<T, String> {
    let (path, address) = device
        .attachment
        .usb_location()
        .ok_or("The verified PODxt Live USB attachment is no longer available.")?;
    #[cfg(target_os = "linux")]
    {
        let source = variax_workbench::device_probe::LinuxSysfs::new();
        if source.usb_device_address(path) != Some(address) {
            return Err("The PODxt Live USB attachment changed.".into());
        }
        with_platform_link(device, rig, run)
    }
    #[cfg(target_os = "macos")]
    {
        let registry_id = device
            .attachment
            .usb_registry_id()
            .ok_or("The verified PODxt Live registry generation is unavailable.")?;
        variax_workbench::podxt_live::with_verified_workbench_link(
            rig,
            path,
            address,
            registry_id,
            run,
        )
    }
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn platform_read(
    device: &detection::DeviceKey,
    command: ReadCommand,
    stop: &AtomicBool,
    reply_pending: &AtomicBool,
    acquired: Option<&AtomicBool>,
    publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    transaction(
        command,
        stop,
        reply_pending,
        || {
            use line6_devices::device_lock::DeviceLock;
            match acquired {
                Some(acquired) => {
                    DeviceLock::acquire_for_download(|| stop.load(Ordering::Acquire), acquired)
                }
                None => DeviceLock::acquire().map_err(|error| error.to_string()),
            }
        },
        |rig, run| {
            if acquired.is_some() {
                with_verified_platform_link(device, rig, run)
            } else {
                with_platform_link(device, rig, run)
            }
        },
        publish,
    )
}

#[cfg(any(test, not(any(target_os = "linux", target_os = "macos"))))]
fn platform_read(
    _device: &detection::DeviceKey,
    _command: ReadCommand,
    _stop: &AtomicBool,
    _reply_pending: &AtomicBool,
    _acquired: Option<&AtomicBool>,
    _publish: impl FnMut(ReadUpdate),
) -> Result<(), String> {
    Err("PODxt Live reads are unavailable in this build.".into())
}

fn library_total(command: ReadCommand) -> usize {
    match command {
        ReadCommand::ToneBank => usize::from(BankKind::Tones.size()),
        ReadCommand::Library => BankKind::ALL
            .into_iter()
            .map(|bank| usize::from(bank.size()))
            .sum(),
        _ => 0,
    }
}

const fn title(command: ReadCommand) -> &'static str {
    match command {
        ReadCommand::Current => "Read current tone (diagnostic snapshot)",
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
        let verified = device.filter(|tab| {
            tab.connection == tabs::Connection::Present
                && tab.device.key.family_id == "podxt"
                && tab.device.support == detection::DeviceSupport::Verified("3.01")
        });
        let key = verified.map(|tab| &tab.device.key);
        if self.device.as_ref() == key {
            if let Some(program) = verified.and_then(|tab| tab.device.podxt_program)
                && let Some(target) = Location::new(BankKind::Tones, u16::from(program.channel))
                && self
                    .live_since
                    .is_some_and(|since| program.observed_at >= since)
                && self
                    .linux_program_seen
                    .is_none_or(|seen| program.observed_at > seen)
            {
                self.linux_program_seen = Some(program.observed_at);
                self.linux_follow_due =
                    Some((program.observed_at + follow::CHANNEL_SETTLE, target));
                if self.editor.observe_program(target) {
                    self.message =
                        Some("The playing channel changed. Select the channel again.".into());
                }
            }
            return false;
        }
        let expired = self.editor.awaiting_recall_confirmation();
        self.tuner.detach();
        self.cancel();
        self.editor.detach_attachment();
        self.follow.reset();
        self.store.detach();
        self.locker.detach(&mut self.store);
        self.save_ingress =
            verified.map_or_else(Arc::default, |tab| Arc::clone(&tab.device.podxt_saves));
        if let Some(tab) = verified {
            self.editor.reply_pending = Arc::clone(&tab.device.podxt_reply_pending);
        }
        let previous_device = std::mem::replace(&mut self.device, key.cloned());
        self.startup = self.device.as_ref().map(|_| Startup::Current);
        self.sync_result = None;
        self.live_editing = false;
        self.live_since = None;
        self.linux_program_seen = None;
        self.resume_live = self.device_ready() && !self.editor.has_pending();
        if self.panel == Some(StagePanel::Store) {
            self.panel = None;
        }
        self.editor.set_live(false);
        let mut previous = std::mem::take(&mut self.snapshot);
        if let Some((device, count)) = previous_device.zip(previous.complete_tone_bank()) {
            self.historical_library = std::mem::take(&mut previous.library);
            self.historical_bank = Some((device, count));
        } else if self.historical_bank.is_none() {
            self.historical_library.append(&mut previous.library);
        }
        if let Some((source, count)) = self.historical_bank.as_ref()
            && key == Some(source)
        {
            self.snapshot.library = std::mem::take(&mut self.historical_library);
            self.snapshot.library_count = Some(*count);
            self.historical_bank = None;
        }
        if self.editor.has_pending() {
            self.snapshot.tone = previous.tone;
            self.snapshot.channel = previous.channel;
            self.snapshot.packs = previous.packs;
        }
        self.message =
            expired.then(|| "The PODxt Live attachment changed. Select the channel again.".into());
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

    pub(super) fn shows_operation_progress(&self) -> bool {
        self.worker.is_some() || self.editor.progress_label().is_some() || self.store.busy()
    }

    pub(super) fn cancel(&mut self) {
        self.startup = None;
        self.resume_live = false;
        self.linux_follow_due = None;
        self.linux_follow_read = false;
        self.follow.pause();
        self.editor.cancel();
        self.store.cancel();
        if let Some(worker) = &self.worker {
            worker.request_stop();
        }
    }

    fn start(&mut self, command: ReadCommand, ctx: &egui::Context) {
        if let Err(error) = self.start_read(command, ctx, false) {
            self.message = Some(error);
        }
    }

    fn start_read(
        &mut self,
        command: ReadCommand,
        ctx: &egui::Context,
        managed: bool,
    ) -> Result<(), String> {
        #[cfg(test)]
        {
            self.device_requests += 1;
        }
        let device = self
            .device
            .clone()
            .ok_or("The PODxt Live connection changed.")?;
        if self.busy() || !self.device_ready() || !Self::admitted(command) {
            return Err("PODxt Live is unavailable for this read.".into());
        }
        self.follow.pause();
        let mut previous = self.snapshot.clone();
        previous.warning = None;
        previous.clip = None;
        previous.tap = None;
        previous.tuner = None;
        previous.tuner_mode = false;
        if requires_library(command) {
            previous.library_count = None;
            previous.library_save_baseline = self.save_ingress.revision();
        }
        let snapshot = Arc::new(Mutex::new(previous));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_snapshot = Arc::clone(&snapshot);
        let repaint = ctx.clone();
        let endpoint = device.clone();
        let reply_pending = Arc::clone(&self.editor.reply_pending);
        let acquired = managed.then(|| Arc::new(AtomicBool::new(false)));
        let worker_acquired = acquired.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-read".into())
            .spawn(move || {
                platform_read(
                    &endpoint,
                    command,
                    &worker_stop,
                    &reply_pending,
                    worker_acquired.as_deref(),
                    |update| {
                        let received_at = Instant::now();
                        if !worker_stop.load(Ordering::Acquire) {
                            let mut snapshot = worker_snapshot.lock().expect("snapshot mutex");
                            if command == ReadCommand::Follow
                                && let ReadUpdate::Stored(stored) = &update
                                && !snapshot.library.contains_key(&stored.location.address())
                            {
                                return;
                            }
                            snapshot.apply(update, received_at);
                            repaint.request_repaint();
                        }
                    },
                )
            });
        let thread = thread.map_err(|error| format!("Could not start PODxt read: {error}"))?;
        self.message = None;
        self.worker = Some(Worker {
            command,
            device,
            thread: Some(thread),
            snapshot,
            stop,
            acquired,
        });
        Ok(())
    }

    fn admitted(command: ReadCommand) -> bool {
        platform_available()
            && !hardware_gates::is_locked(Gate::OsDiscovery)
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            && (!requires_library(command) || !hardware_gates::is_locked(Gate::FullBackupRead))
    }

    fn next_startup(&mut self) -> Option<Startup> {
        if self.busy() || !self.device_ready() {
            return None;
        }
        let request = self.startup.take()?;
        if request == Startup::Current && !self.has_session_library() {
            self.startup = Some(Startup::Tones);
        }
        Some(request)
    }

    pub(super) fn has_session_library(&self) -> bool {
        self.snapshot.complete_tone_bank().is_some() || self.historical_bank.is_some()
    }

    pub(super) fn connection_sync_admission(&self) -> SyncAdmission {
        if let Some(reason) = self.pending_reply_message() {
            return SyncAdmission::Blocked(reason);
        }
        if self.busy() || self.startup == Some(Startup::Current) {
            return SyncAdmission::Busy;
        }
        if self.snapshot.complete_tone_bank().is_some() {
            return SyncAdmission::Complete;
        }
        if !Self::admitted(ReadCommand::ToneBank) {
            return SyncAdmission::Unavailable(
                "PODxt library download is unavailable in this build.",
            );
        }
        SyncAdmission::Ready(
            library_total(ReadCommand::ToneBank)
                .try_into()
                .expect("PODxt tones"),
        )
    }

    pub(super) fn start_connection_sync(&mut self, ctx: &egui::Context) -> Result<(), String> {
        self.startup = None;
        self.sync_result = None;
        self.start_read(ReadCommand::ToneBank, ctx, true)
    }

    pub(super) fn connection_sync_update(&mut self) -> Option<SyncUpdate> {
        if let Some(result) = self.sync_result.take() {
            return Some(SyncUpdate::Finished(result));
        }
        let worker = self.worker.as_ref()?;
        let acquired = worker.acquired.as_deref()?;
        (self.device.as_ref() == Some(&worker.device)).then(|| {
            if acquired.load(Ordering::Acquire) {
                SyncUpdate::Reading(worker.library_progress().unwrap_or(0))
            } else {
                SyncUpdate::WaitingForRig
            }
        })
    }

    pub(super) fn cancel_connection_sync(&self) {
        if let Some(worker) = &self.worker
            && worker.acquired.is_some()
        {
            worker.stop.store(true, Ordering::Release);
        }
    }

    fn update_playing(&mut self, buffer: podxt::EditorBuffer) {
        if self.panel == Some(StagePanel::Store)
            && (self.snapshot.tone.as_ref() != Some(&buffer.tone)
                || self.snapshot.channel != Some(buffer.channel))
        {
            self.store.clear_preparation();
        }
        self.snapshot.tone = Some(buffer.tone);
        self.snapshot.packs = Some(buffer.packs);
        self.snapshot.channel = Some(buffer.channel);
    }

    fn accept_editor_read(&mut self, buffer: podxt::EditorBuffer, ctx: &egui::Context) {
        self.update_playing(buffer);
        self.reveal_playing |= std::mem::take(&mut self.linux_follow_read);
        if self.resume_live {
            self.resume_live = false;
            self.live_editing = true;
            self.live_since = Some(Instant::now());
            self.editor.set_live(true);
            ctx.request_repaint();
        }
    }

    pub(super) fn take_playing_reveal(&mut self) -> bool {
        std::mem::take(&mut self.reveal_playing)
    }

    fn poll_follow(&mut self, ctx: &egui::Context) {
        let eligible = self.live_editing
            && !self.editor.awaiting_recall_confirmation()
            && self.device_ready()
            && !self.busy()
            && (self.startup.is_none()
                || self.startup == Some(Startup::Tones)
                    && hardware_gates::is_locked(Gate::FullBackupRead))
            && self.has_current()
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead);
        if !eligible {
            self.follow.pause();
        } else if let Some((due, target)) = self.linux_follow_due {
            let remaining = due.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.linux_follow_due = None;
                self.linux_follow_read = true;
                self.refresh_current_after_program(Some(target), ctx);
            } else {
                ctx.request_repaint_after(remaining);
            }
        }
        match self.follow.poll(
            self.device.as_ref().filter(|_| eligible),
            &self.save_ingress,
            &self.editor.reply_pending,
            ctx,
        ) {
            Ok(Some(update)) => {
                self.reveal_playing |= update.program_target.is_some();
                if let Some(device) = &self.device {
                    self.editor.accept_program_follow(
                        update.buffer.clone(),
                        update.program_target,
                        device,
                        ctx,
                    );
                }
                self.update_playing(update.buffer);
            }
            Ok(None) => {}
            Err(error) => self.message = Some(format!("PODxt Follow stopped. {error}")),
        }
    }

    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        self.poll_follow(ctx);
        if let Some((device, result)) = self.store.poll(ctx) {
            let result = if self.device.as_ref() == Some(&device) {
                result
            } else {
                Err(())
            };
            if let Ok(stored) = &result {
                self.editor.reconcile_stored(stored);
            }
            self.locker
                .result(result, &mut self.snapshot, &mut self.store);
        }
        let ready = self.device_ready();
        if let Some(buffer) = self.editor.poll(
            self.device.as_ref(),
            self.worker.is_none() && !self.store.busy() && ready,
            ctx,
        ) {
            let buffer = buffer.clone();
            self.accept_editor_read(buffer, ctx);
        } else if !self.editor.busy() {
            self.linux_follow_read = false;
            if self.startup != Some(Startup::Current) {
                self.resume_live = false;
            }
        }
        let Some(worker) = &self.worker else {
            // The shell owns ToneBank admission and Retry. Keep the current
            // tone's existing editor read and its separate gate.
            if self.startup == Some(Startup::Current) && self.next_startup().is_some() {
                self.refresh_current(ctx);
                ctx.request_repaint();
            }
            return;
        };
        if self.device.as_ref() == Some(&worker.device)
            && worker.command != ReadCommand::Tuner
            && !requires_library(worker.command)
        {
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
        let result = worker
            .thread
            .take()
            .expect("worker thread")
            .join()
            .unwrap_or_else(|_| Err("PODxt reader stopped unexpectedly.".into()));
        if command == ReadCommand::Tuner {
            self.tuner.finished(stopped, result);
            ctx.request_repaint();
        } else if !same_device {
            return;
        } else if requires_library(command) {
            let result = if stopped {
                result.and(Err("PODxt Live read cancelled.".into()))
            } else {
                result
            };
            let count = worker.library_progress().unwrap_or(0);
            let result = result.and_then(|()| {
                if count != library_total(command) {
                    return Err(format!("PODxt library read ended after {count} entries."));
                }
                let mut captured = worker.snapshot.lock().expect("snapshot mutex");
                if self
                    .save_ingress
                    .saved_since(captured.library_save_baseline)
                {
                    return Err(
                        "A PODxt save superseded this library read. Read the library again.".into(),
                    );
                }
                if command == ReadCommand::Library {
                    self.snapshot.library.clear();
                }
                self.snapshot.library.append(&mut captured.library);
                self.snapshot.library_count = Some(count);
                Ok(())
            });
            if worker.acquired.is_some() {
                self.sync_result = Some(result.clone());
            }
            self.message = Some(match result {
                Ok(()) => format!("Library read complete. {count} entries."),
                Err(error) => error,
            });
        } else {
            self.message = Some(match result {
                Ok(()) => format!("{} complete.", title(command)),
                Err(error) => error,
            });
        }
        if !self.device_ready() {
            self.startup = None;
        }
    }

    pub(super) fn connected(&self) -> bool {
        self.device.is_some()
    }

    fn device_ready(&self) -> bool {
        self.connected() && self.pending_reply_message().is_none()
    }

    pub(super) fn pending_reply_message(&self) -> Option<&'static str> {
        (self.connected() && self.editor.reply_pending.load(Ordering::Acquire))
            .then_some(PENDING_REPLY)
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

    pub(super) fn stage_footer(&self) -> &str {
        self.editor
            .status_text()
            .or(self.message.as_deref())
            .unwrap_or("Current POD tone")
    }

    pub(super) fn has_library_bank(&self, bank: BankKind) -> bool {
        Location::all(&[bank]).any(|slot| self.stored_tone(slot).is_some())
    }

    pub(super) fn draft_counts(&self) -> (usize, usize) {
        self.editor.draft_counts()
    }

    pub(super) fn channel_draft(&self, slot: Location) -> Option<&podxt::TonePayload> {
        self.editor.draft(slot)
    }

    pub(super) fn channel_draft_is_imported(&self, slot: Location) -> bool {
        self.editor.draft_is_imported(slot)
    }

    pub(super) fn playing_draft_status(&self) -> Option<&'static str> {
        self.editor.playing_draft_status(self.playing_channel()?)
    }

    pub(super) fn channel_modified(&self, slot: Location) -> bool {
        self.snapshot.channel == Some(slot)
            && (self.editor.has_pending()
                || self
                    .cached_channel_comparison(slot)
                    .is_some_and(|comparison| comparison.has_name_or_control_difference()))
    }

    fn cached_channel_comparison(&self, slot: Location) -> Option<podxt::RecallComparison> {
        if self.playing_channel() != Some(slot) {
            return None;
        }
        Some(podxt::RecallComparison::between(
            self.snapshot.tone.as_ref()?,
            self.stored_tone(slot)?,
        ))
    }

    pub(super) fn channel_comparison_text(&self, slot: Location) -> Option<String> {
        let pending = self.playing_channel() == Some(slot) && self.editor.has_pending();
        let comparison = self.cached_channel_comparison(slot);
        let mut details = String::new();
        if pending {
            details.push_str("PODxt edits are queued.\n");
        }
        if let Some(comparison) = comparison
            && (comparison.name_changed
                || !comparison.controls.is_empty()
                || !comparison.opaque_offsets.is_empty())
        {
            details.push_str(&comparison_text(
                "Cached saved slot compared with the editor's last playing readback",
                &comparison,
            ));
        }
        (!details.is_empty()).then_some(details)
    }

    pub(super) fn stored_tone(&self, slot: Location) -> Option<&EditBuffer> {
        self.snapshot.library.get(&slot.address())
    }

    pub(super) fn historical_tone(&self, slot: Location) -> Option<&EditBuffer> {
        self.historical_library.get(&slot.address())
    }

    fn can_refresh_current(&self) -> bool {
        !self.busy() && self.device_ready() && self.editor.can_get()
    }

    pub(super) fn refresh_current(&mut self, ctx: &egui::Context) {
        self.refresh_current_after_program(None, ctx);
    }

    fn refresh_current_after_program(&mut self, target: Option<Location>, ctx: &egui::Context) {
        #[cfg(test)]
        {
            self.device_requests += 1;
        }
        if self.can_refresh_current()
            && let Some(device) = &self.device
        {
            self.follow.pause();
            if let Some(target) = target {
                self.editor.get_after_program(target, device, ctx);
            } else {
                self.editor.get(device, ctx);
            }
        }
    }

    pub(super) fn queue_stage_target(&mut self, target: podxt::LiveTarget, ctx: &egui::Context) {
        if self.device_ready() && self.worker.is_none() && !self.store.busy() {
            self.editor.queue_stage_target(target, ctx);
        }
    }

    pub(super) fn can_change_control(&self, control: LiveControl, block: &str) -> bool {
        self.device_ready()
            && self.worker.is_none()
            && !self.store.busy()
            && self.editor.can_change(control, block)
    }

    pub(super) fn recall(&mut self, slot: Location, ctx: &egui::Context) {
        #[cfg(test)]
        {
            self.device_requests += 1;
        }
        if !self.live_editing || !self.device_ready() {
            return;
        }
        if self.editor.awaiting_recall_confirmation() {
            self.message =
                Some("Cancel the pending switch before selecting another channel.".into());
        } else if self.busy() {
            self.message =
                Some("Wait for the current PODxt operation, then select the channel again.".into());
        } else if let Some(device) = &self.device {
            self.message = None;
            self.follow.pause();
            self.editor.recall(slot, device, ctx);
        }
    }

    pub(super) fn recall_controls(&mut self, ui: &mut egui::Ui) {
        if !self.live_editing || !self.device_ready() {
            return;
        }
        let idle = !self.busy();
        if let Some(device) = &self.device {
            self.editor.show_recall_confirmation(ui, device, idle);
        }
    }

    pub(super) fn locker_dirty(&self) -> bool {
        self.locker.dirty()
    }

    pub(super) fn discard_locker(&mut self) {
        self.locker.discard(&mut self.store);
    }

    pub(super) fn enter_edit(&mut self) {
        self.resume_live = false;
        self.live_editing = false;
        self.live_since = None;
        self.linux_follow_due = None;
        self.linux_follow_read = false;
        self.editor.set_live(false);
        self.follow.reset();
    }

    fn enter_live(&mut self, ctx: &egui::Context) {
        if !self.device_ready() || self.busy() || self.editor.has_pending() {
            return;
        }
        self.resume_live = true;
        self.follow.reset();
        self.refresh_current(ctx);
    }

    pub(super) fn has_staged_edits(&self) -> bool {
        !self.live_editing && self.editor.has_pending()
    }

    pub(super) fn discard_staged(&mut self) {
        self.editor.discard_staged();
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, current: &mut bool) {
        let idle = !self.busy();
        let connected = self.device_ready();
        if self.connected() {
            if ui
                .add_enabled(idle && self.live_editing, egui::Button::new("Edit"))
                .clicked()
            {
                self.enter_edit();
            }
            let staged = self.has_staged_edits();
            if ui
                .add_enabled(
                    connected
                        && idle
                        && !self.live_editing
                        && !self.resume_live
                        && !staged
                        && Self::admitted(ReadCommand::Current),
                    egui::Button::new("Live"),
                )
                .on_disabled_hover_text(if staged {
                    "Send or discard the staged edits before returning to Live."
                } else {
                    "Wait for the PODxt Live to be ready."
                })
                .clicked()
            {
                self.enter_live(ui.ctx());
                *current = true;
            }
        }
        let needs_refresh = self.editor.needs_refresh();
        if self.visible()
            && (!self.live_editing || needs_refresh)
            && ui
                .add_enabled(
                    self.can_refresh_current(),
                    egui::Button::new("Read playing tone"),
                )
                .on_hover_text(if self.editor.needs_refresh() {
                    "Read the PODxt edit buffer and refresh the editor before editing or recalling a channel."
                } else {
                    "Read the PODxt edit buffer into the playing tone editor."
                })
                .on_disabled_hover_text(if self.editor.has_pending() {
                    "Apply or discard staged edits before reading the playing tone."
                } else {
                    "A verified, idle PODxt Live read connection is required."
                })
                .clicked()
        {
            self.message = None;
            self.refresh_current(ui.ctx());
            *current = true;
        }
        if !*current && self.has_staged_edits() && ui.button("Staged PODxt tone").clicked() {
            *current = true;
        }
        if *current {
            self.editor.toolbar(
                ui,
                self.device.as_ref(),
                connected && self.worker.is_none() && !self.store.busy(),
            );
        }
        if self.visible() && *current {
            let help = "Review permanent storage of the playing tone in its PODxt channel.";
            let selected = self.panel == Some(StagePanel::Store);
            let enabled = connected
                && idle
                && !self.locker.syncing()
                && !self.editor.has_pending()
                && self.snapshot.channel.is_some();
            if ui
                .add_enabled(
                    enabled || selected,
                    egui::Button::selectable(selected, "Store to POD…"),
                )
                .on_hover_text(help)
                .on_disabled_hover_text(help)
                .clicked()
            {
                self.store.clear_preparation();
                self.panel = (!selected).then_some(StagePanel::Store);
                self.scroll_to_panel = self.panel.is_some();
            }
        }
    }

    pub(super) fn initialize_tuner(&mut self, preferences: Option<&Path>) {
        self.tuner = tuner::Tuner::with_preferences(preferences);
    }

    pub(super) fn tuner_command(&mut self, ui: &mut egui::Ui) -> Option<egui::Response> {
        if !self.visible() {
            return None;
        }
        let available = self.device_ready() && !self.busy() && Self::admitted(ReadCommand::Tuner);
        let response = ui.add_enabled(available, egui::Button::new("Tuner"));
        if response.clicked() && !self.tuner.is_open() {
            let result = self.start_read(ReadCommand::Tuner, ui.ctx(), false);
            if result.is_ok() {
                self.snapshot.tuner_mode = false;
            }
            self.tuner.started(result);
            ui.ctx().request_repaint();
        }
        Some(response)
    }

    pub(super) fn show_tuner(
        &mut self,
        ctx: &egui::Context,
        button: Option<&egui::Response>,
        fallback: &egui::Response,
    ) {
        self.tuner.show(
            ctx,
            self.worker
                .as_ref()
                .filter(|worker| worker.command == ReadCommand::Tuner),
            button
                .filter(|response| response.enabled())
                .unwrap_or(fallback),
        );
    }

    pub(super) fn stage_status(&self, ui: &mut egui::Ui) {
        if !self.visible() {
            return;
        }
        if self.snapshot.tuner_mode {
            ui.weak("Tuner active on POD");
        }
        if let Some(worker) = &self.worker
            && requires_library(worker.command)
            && let Some(count) = worker.library_progress()
        {
            let total = library_total(worker.command);
            ui.weak(format!("Reading library · {count}/{total}"));
            if ui.button("Cancel library read").clicked() {
                worker.stop.store(true, Ordering::Release);
            }
        }
        self.editor.status(ui);
        if let Some(message) = self.pending_reply_message() {
            ui.weak(message);
        }
        if let Some(message) = &self.message {
            ui.weak(message);
        }
    }

    pub(super) fn show_store_panel(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
    ) {
        if self.panel == Some(StagePanel::Store) {
            let view = ui.scope(|ui| self.show_current_store(ui, services));
            if std::mem::take(&mut self.scroll_to_panel) {
                view.response.scroll_to_me(Some(egui::Align::Min));
            }
        }
    }

    pub(super) fn show_locker(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        files: &mut super::locker::FileBrowser,
        editor_tone: Option<&pod_preset::PodTone>,
    ) -> Option<super::locker::Document> {
        let ready = self.device_ready();
        let idle = !self.busy() && !self.editor.has_pending();
        let actions = self.locker.show(
            ui,
            services,
            locker::View {
                snapshot: &self.snapshot,
                idle: idle && self.panel != Some(StagePanel::Store),
                can_get: idle && ready && Self::admitted(ReadCommand::Library),
                editor_tone,
            },
            &mut self.store,
            files,
        );
        if actions.get {
            self.start(ReadCommand::Library, ui.ctx());
        }
        if self.locker.syncing() {
            self.store.show(
                ui,
                services,
                self.device.as_ref().filter(|_| ready),
                &self.editor.reply_pending,
                idle,
                store::StoreView::Sync,
            );
        }
        if let Some(worker) = &self.worker
            && worker.command == ReadCommand::Library
        {
            let count = worker.library_progress().unwrap_or(0);
            let total = library_total(worker.command);
            ui.weak(format!("Reading full library · {count}/{total}"));
            if ui
                .add_enabled(
                    !worker.stop.load(Ordering::Acquire),
                    egui::Button::new("Cancel library read"),
                )
                .clicked()
            {
                worker.stop.store(true, Ordering::Release);
            }
        }
        if let Some(message) = &self.message {
            ui.weak(message);
        }
        if let Some(message) = self.pending_reply_message() {
            ui.weak(message);
        }
        actions.open
    }

    fn show_current_store(&mut self, ui: &mut egui::Ui, services: &mut EditorServices<'_>) {
        let Some(slot) = self.snapshot.channel else {
            return;
        };
        let idle = !self.busy() && !self.editor.has_pending();
        let ready = self.device_ready();
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
                self.device.as_ref().filter(|_| ready),
                &self.editor.reply_pending,
                idle,
                store::StoreView::Current,
            );
        });
    }

    pub(super) fn device_check_panel(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
    ) {
        ui.collapsing("PODxt Live diagnostics", |ui| {
            self.show_advanced(ui, services);
            if !self.locker.syncing() && self.panel != Some(StagePanel::Store) {
                let ready = self.device_ready();
                self.store.show(
                    ui,
                    services,
                    self.device.as_ref().filter(|_| ready),
                    &self.editor.reply_pending,
                    self.worker.is_none() && !self.editor.busy() && !self.editor.has_pending(),
                    store::StoreView::Original,
                );
            }
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
                let enabled = self.device_ready() && !self.busy() && Self::admitted(command);
                let response = ui.add_enabled(enabled, egui::Button::new(title(command)));
                let response = if command == ReadCommand::Current {
                    response.on_hover_text("Update the diagnostic snapshot of the current tone.")
                } else {
                    response
                };
                if response.clicked() {
                    self.start(command, ui.ctx());
                }
            }
            if let Some(worker) = &self.worker {
                let label = match worker.command {
                    ReadCommand::Tuner => "Stop tuner",
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
        if let Some(message) = self.pending_reply_message() {
            ui.weak(message);
        } else if self.device.is_none() {
            ui.weak("Live reads require automatic PODxt Live 3.01 identity.");
        } else if !platform_available() {
            ui.weak("Live reads are unavailable in this build.");
        }
        if let Some(worker) = &self.worker {
            if worker.stop.load(Ordering::Acquire) {
                ui.weak("Finishing the current device operation…");
            } else {
                ui.weak(match worker.command {
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
        if let Some(worker) = &self.worker
            && worker.command == ReadCommand::Library
            && let Some(count) = worker.library_progress()
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
        let ready = self.device_ready();
        self.editor.show(
            ui,
            services,
            self.device.as_ref(),
            ready && self.worker.is_none() && !self.store.busy(),
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

fn comparison_text(source: &str, comparison: &podxt::RecallComparison) -> String {
    use std::fmt::Write as _;

    let mut text = source.to_owned();
    if comparison.name_changed {
        text.push_str("\nTone name differs.");
    }
    for &difference in &comparison.controls {
        let label = difference.name().unwrap_or("Unidentified control");
        let category = if difference.volatile() {
            " · volatile live state"
        } else {
            ""
        };
        let _ = write!(
            text,
            "\n{label}, CC {}{category} · playing {}, saved {}",
            difference.cc, difference.playing, difference.saved,
        );
    }
    if !comparison.opaque_offsets.is_empty() {
        let offsets = comparison
            .opaque_offsets
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>();
        let _ = write!(
            text,
            "\nOpaque dump offsets (zero-based) {}",
            offsets.join(", ")
        );
    }
    text
}

use super::*;
use podxt::{EditorBuffer, LiveTarget, Target, VerifiedToneBackup};
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use podxt::{EditorEdit, WholeToneSource};

const EDIT_INTERVAL: Duration = Duration::from_millis(150);
// #222's full-payload transfer has a separate first-run decision.
const WHOLE_TONE_SEND_APPROVED: bool = false;

pub(super) struct LiveEditor {
    pub(super) reply_pending: Arc<AtomicBool>,
    session: Option<ActiveEdit>,
    worker: Option<EditWorker>,
    pending: Pending,
    whole_tone_backup: Option<VerifiedToneBackup>,
    stage_only: bool,
    needs_get: bool,
    last_dispatch: Option<Instant>,
    message: Option<Result<String, String>>,
}

impl Default for LiveEditor {
    fn default() -> Self {
        Self {
            reply_pending: Arc::default(),
            session: None,
            worker: None,
            pending: Pending::default(),
            whole_tone_backup: None,
            stage_only: true,
            needs_get: false,
            last_dispatch: None,
            message: None,
        }
    }
}

struct ActiveEdit {
    original: EditorBuffer,
    current: EditorBuffer,
}

#[derive(Default)]
struct Pending(Vec<LiveTarget>);

impl Pending {
    fn set(&mut self, target: LiveTarget) {
        match self
            .0
            .iter_mut()
            .find(|entry| entry.control() == target.control())
        {
            Some(entry) => *entry = target,
            None => self.0.push(target),
        }
    }

    fn overlay(&self, targets: &[LiveTarget], control: LiveControl, current: u16) -> u16 {
        self.0
            .iter()
            .chain(targets)
            .find(|target| target.control() == control)
            .map_or(current, |target| match *target {
                LiveTarget::Basic(_, Target::Value(value)) => u16::from(value.get()),
                LiveTarget::Additional(_, value) => value,
                _ => unreachable!("the editor stages absolute values"),
            })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Get,
    Controls,
    Send,
    ResendWholeTone,
    Recall(Location),
    Undo,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Get => "Refresh current tone",
            Self::Recall(_) => "Select channel",
            Self::Controls => "Apply live controls",
            Self::Send => "Send edit buffer",
            Self::ResendWholeTone => "Resend whole current tone",
            Self::Undo => "Undo to snapshot",
        }
    }
}

struct EditWorker {
    action: Action,
    device: Option<detection::DeviceKey>,
    stop: Arc<AtomicBool>,
    targets: Vec<LiveTarget>,
    thread: Option<std::thread::JoinHandle<Result<EditorBuffer, String>>>,
}

impl Drop for EditWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
struct EditPolicy(Action);

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
impl GatePolicy for EditPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::PodxtIdentity
            | Operation::PodxtRead
            | Operation::PodxtGlobalRead
            | Operation::PodxtPackQuery => !hardware_gates::is_locked(Gate::CurrentSettingsRead),
            Operation::PodxtLibraryRead => {
                matches!(self.0, Action::ResendWholeTone | Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::FullBackupRead)
            }
            Operation::PodxtChannelSwitch => {
                matches!(self.0, Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::DeviceCheckChannel)
            }
            Operation::PodxtEditBufferWrite | Operation::PodxtAdditionalEditBufferWrite => {
                !matches!(self.0, Action::Get | Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::EditBufferWrite)
            }
            Operation::PodxtKeepEditBufferChanges => {
                !matches!(self.0, Action::Get | Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::KeepLiveEdit)
            }
            _ => false,
        }
    }
}

struct Job {
    reply_pending: Arc<AtomicBool>,
    action: Action,
    whole_tone_backup: Option<VerifiedToneBackup>,
    current: Option<EditorBuffer>,
    original: Option<EditorBuffer>,
    targets: Vec<LiveTarget>,
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn edit_transaction<'a, G>(
    job: &'a Job,
    stop: &'a AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    with_link: impl FnOnce(&G, RunOnLink<'a, EditorBuffer>) -> Result<EditorBuffer, String>,
) -> Result<EditorBuffer, String> {
    if job.reply_pending.load(Ordering::Acquire) {
        return Err(PENDING_REPLY.into());
    }
    let run = || {
        let stopped = || stop.load(Ordering::Acquire);
        with_session(
            stopped,
            lock,
            with_link,
            EditPolicy(job.action),
            move |session| {
                let edit = match job.action {
                    Action::Get => {
                        return track_reply(&job.reply_pending, |outstanding| {
                            podxt::get_editor_buffer_tracking_reply(session, stopped, outstanding)
                        });
                    }
                    Action::Recall(slot) => {
                        return podxt::recall_editor_channel(session, slot, stopped);
                    }
                    Action::ResendWholeTone => {
                        return podxt::send_whole_tone(
                            session,
                            job.current.as_ref().expect("whole-tone send has snapshot"),
                            WholeToneSource::Current,
                            job.whole_tone_backup
                                .as_ref()
                                .ok_or("Verify a backup for the playing channel first.")?,
                            stopped,
                        );
                    }
                    Action::Controls => EditorEdit::Controls(&job.targets),
                    Action::Send => EditorEdit::Send(&job.targets),
                    Action::Undo => {
                        EditorEdit::Undo(job.original.as_ref().expect("undo has snapshot"))
                    }
                };
                podxt::edit_editor_buffer(
                    session,
                    job.current.as_ref().expect("editing has snapshot"),
                    edit,
                    stopped,
                )
            },
        )
    };
    match job.action {
        Action::Get => run(),
        Action::Recall(_) => hardware_gates::run_if_unlocked(Gate::DeviceCheckChannel, || {
            hardware_gates::run_if_unlocked(Gate::FullBackupRead, run)
        }),
        _ => hardware_gates::run_if_unlocked(Gate::EditBufferWrite, || {
            hardware_gates::run_if_unlocked(Gate::KeepLiveEdit, || {
                if job.action == Action::ResendWholeTone {
                    hardware_gates::run_if_unlocked(Gate::FullBackupRead, run)
                } else {
                    run()
                }
            })
        }),
    }
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn platform_edit(
    device: &detection::DeviceKey,
    job: &Job,
    stop: &AtomicBool,
) -> Result<EditorBuffer, String> {
    if job.action == Action::ResendWholeTone && !WHOLE_TONE_SEND_APPROVED {
        return Err("Whole-tone resend awaits owner approval.".into());
    }
    edit_transaction(
        job,
        stop,
        || line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string()),
        |rig, run| with_platform_link(device, rig, run),
    )
}

#[cfg(any(test, not(any(target_os = "linux", target_os = "macos"))))]
fn platform_edit(
    _device: &detection::DeviceKey,
    job: &Job,
    _stop: &AtomicBool,
) -> Result<EditorBuffer, String> {
    let _ = (
        job.action,
        &job.whole_tone_backup,
        &job.current,
        &job.original,
        &job.targets,
    );
    Err("PODxt Live editing is unavailable in this build.".into())
}

impl LiveEditor {
    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || !self.stage_only && !self.pending.0.is_empty()
    }

    pub(super) fn cancel(&mut self) {
        if !self.stage_only {
            self.pending.0.clear();
        }
        if let Some(worker) = &self.worker {
            worker.stop.store(true, Ordering::Release);
        }
    }

    pub(super) fn detach(&mut self) {
        self.cancel();
        if let Some(worker) = &mut self.worker {
            worker.device = None;
        }
        self.session = None;
        self.whole_tone_backup = None;
        self.pending.0.clear();
        self.needs_get = false;
        self.last_dispatch = None;
        self.message = None;
    }

    fn admitted(action: Action) -> bool {
        platform_available()
            && (action != Action::ResendWholeTone
                || WHOLE_TONE_SEND_APPROVED && !hardware_gates::is_locked(Gate::FullBackupRead))
            && !hardware_gates::is_locked(Gate::OsDiscovery)
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            && match action {
                Action::Get => true,
                Action::Recall(_) => {
                    !hardware_gates::is_locked(Gate::DeviceCheckChannel)
                        && !hardware_gates::is_locked(Gate::FullBackupRead)
                }
                _ => {
                    !hardware_gates::is_locked(Gate::EditBufferWrite)
                        && !hardware_gates::is_locked(Gate::KeepLiveEdit)
                }
            }
    }

    fn start(&mut self, action: Action, device: &detection::DeviceKey, ctx: &egui::Context) {
        if self.worker.is_some()
            || self.reply_pending.load(Ordering::Acquire)
            || !Self::admitted(action)
            || action == Action::ResendWholeTone && self.whole_tone_backup.is_none()
            || action != Action::Get && (self.session.is_none() || self.needs_get)
        {
            return;
        }
        let targets = match action {
            Action::Send | Action::Controls => std::mem::take(&mut self.pending.0),
            Action::Get | Action::Undo | Action::ResendWholeTone | Action::Recall(_) => {
                self.pending.0.clear();
                Vec::new()
            }
        };
        if matches!(action, Action::Get | Action::Recall(_)) {
            self.whole_tone_backup = None;
        }
        let job = Job {
            reply_pending: Arc::clone(&self.reply_pending),
            action,
            whole_tone_backup: if action == Action::ResendWholeTone {
                self.whole_tone_backup.take()
            } else {
                None
            },
            current: self.session.as_ref().map(|active| active.current.clone()),
            original: self.session.as_ref().map(|active| active.original.clone()),
            targets: targets.clone(),
        };
        let endpoint = device.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let repaint = ctx.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-edit-buffer".into())
            .spawn(move || {
                let result = platform_edit(&endpoint, &job, &worker_stop);
                repaint.request_repaint();
                result
            });
        match thread {
            Ok(thread) => {
                self.last_dispatch = Some(Instant::now());
                self.message = None;
                self.worker = Some(EditWorker {
                    action,
                    device: Some(device.clone()),
                    stop,
                    targets,
                    thread: Some(thread),
                });
            }
            Err(error) => {
                self.pending.0 = targets;
                self.message = Some(Err(format!(
                    "Could not start PODxt Live operation: {error}"
                )));
            }
        }
    }

    fn completed(
        &mut self,
        action: Action,
        result: Result<EditorBuffer, String>,
    ) -> Option<&EditorBuffer> {
        match result {
            Ok(buffer) => {
                let active = self.session.get_or_insert_with(|| ActiveEdit {
                    original: buffer.clone(),
                    current: buffer.clone(),
                });
                if active.original.channel != buffer.channel {
                    active.original = buffer.clone();
                }
                active.current = buffer;
                self.needs_get = false;
                self.message = Some(Ok(match action {
                    Action::Get => {
                        format!("Edit buffer read from {}.", active.current.channel.label())
                    }
                    Action::Undo => "Original snapshot verified byte for byte.".into(),
                    Action::Recall(slot) => format!("Playing {}.", slot.label()),
                    Action::Send => "Edit buffer sent and read back.".into(),
                    Action::ResendWholeTone => "Whole tone sent and read back.".into(),
                    Action::Controls => "Live edit read back.".into(),
                }));
                Some(&active.current)
            }
            Err(error) => {
                self.pending.0.clear();
                self.needs_get = true;
                self.message = Some(Err(error));
                None
            }
        }
    }

    fn dispatch_wait(&self, now: Instant) -> Duration {
        self.last_dispatch.map_or(Duration::ZERO, |last| {
            EDIT_INTERVAL.saturating_sub(now.saturating_duration_since(last))
        })
    }

    pub(super) fn poll(
        &mut self,
        device: Option<&detection::DeviceKey>,
        read_idle: bool,
        ctx: &egui::Context,
    ) -> Option<&EditorBuffer> {
        if let Some(worker) = &self.worker {
            if !worker.thread.as_ref().expect("edit worker").is_finished() {
                ctx.request_repaint_after(REPAINT_WHILE_READING);
                return None;
            }
            let mut worker = self.worker.take().expect("finished edit worker");
            let result = worker
                .thread
                .take()
                .expect("edit worker")
                .join()
                .unwrap_or_else(|_| Err("PODxt Live editor worker stopped unexpectedly.".into()));
            if device.is_some() && worker.device.as_ref() == device {
                if !self.stage_only && !self.pending.0.is_empty() {
                    ctx.request_repaint();
                }
                return self.completed(worker.action, result);
            }
        }
        if let Some(device) =
            device.filter(|_| read_idle && !self.stage_only && !self.pending.0.is_empty())
        {
            let wait = self.dispatch_wait(Instant::now());
            if wait.is_zero() {
                self.start(Action::Controls, device, ctx);
            } else {
                ctx.request_repaint_after(wait);
            }
        }
        None
    }

    pub(super) fn get(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        self.start(Action::Get, device, ctx);
    }

    pub(super) fn recall(
        &mut self,
        slot: Location,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
    ) {
        if self.pending.0.is_empty() {
            self.start(Action::Recall(slot), device, ctx);
        } else {
            self.message = Some(Err(
                "Send or clear staged edits before selecting another channel.".into(),
            ));
        }
    }

    pub(super) fn set_live(&mut self, on: bool) {
        self.stage_only = !on;
    }

    pub(super) fn begin_live(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        self.detach();
        self.set_live(true);
        self.get(device, ctx);
    }

    pub(super) fn can_change(&self, control: LiveControl, block: &str) -> bool {
        let Some(active) = &self.session else {
            return false;
        };
        if !self.stage_editable() {
            return false;
        }
        let targets = self
            .worker
            .as_ref()
            .map_or(&[][..], |worker| worker.targets.as_slice());
        let value = |control: LiveControl| {
            self.pending
                .overlay(targets, control, control.read(&active.current.tone))
        };
        let note = match control {
            LiveControl::Additional(Setting::ModSpeed) => Some(Setting::ModNote),
            LiveControl::Additional(Setting::DelayTime) => Some(Setting::DelayNote),
            _ => None,
        };
        if note.is_some_and(|note| value(LiveControl::Additional(note)) != 0) {
            return false;
        }
        if let LiveControl::Additional(setting) = control
            && setting.model_list().is_some()
        {
            return setting
                .model_list()
                .and_then(|models| models.get(usize::from(control.read(&active.current.tone))))
                .is_some_and(|&id| {
                    podxt::models::required_pack(id)
                        .is_none_or(|mask| active.current.packs.0 & mask != 0)
                });
        }
        let model = match block {
            "amp" => Some(Setting::AmpModel),
            "stomp" => Some(Setting::StompModel),
            "mod" => Some(Setting::ModModel),
            "delay" => Some(Setting::DelayModel),
            "reverb" => Some(Setting::ReverbModel),
            "wah" => Some(Setting::WahModel),
            _ => None,
        };
        model
            .map(LiveControl::Additional)
            .is_none_or(|model| value(model) == model.read(&active.current.tone))
    }

    pub(super) fn live_edit_admitted() -> bool {
        Self::admitted(Action::Controls)
    }

    pub(super) fn stage_editable(&self) -> bool {
        self.session.is_some()
            && !self.needs_get
            && Self::live_edit_admitted()
            && self
                .worker
                .as_ref()
                .is_none_or(|worker| worker.action == Action::Controls)
    }

    pub(super) fn queue_stage_target(&mut self, target: LiveTarget, ctx: &egui::Context) {
        if self.stage_editable() {
            self.pending.set(target);
            ctx.request_repaint();
        }
    }

    pub(super) fn stage_payload(&self, dump: &EditBuffer) -> Option<[u8; 160]> {
        let mut payload = *podxt::TonePayload::from_dump(dump).ok()?.as_bytes();
        let in_flight = self
            .worker
            .as_ref()
            .map_or(&[][..], |worker| worker.targets.as_slice());
        for &target in in_flight.iter().chain(&self.pending.0) {
            let control = target.control();
            let value = match target {
                LiveTarget::Basic(_, Target::Value(value)) => u16::from(value.get()),
                LiveTarget::Additional(_, value) => value,
                _ => continue,
            };
            if let Some(low) = control.low_cc() {
                payload[32 + usize::from(control.cc())] = (value >> 7) as u8;
                payload[32 + usize::from(low)] = (value & 127) as u8;
            } else {
                payload[32 + usize::from(control.cc())] = value as u8;
            }
        }
        Some(payload)
    }

    pub(super) fn has_pending(&self) -> bool {
        !self.pending.0.is_empty()
    }

    pub(super) fn toolbar(
        &mut self,
        ui: &mut egui::Ui,
        device: Option<&detection::DeviceKey>,
        idle: bool,
    ) {
        let ready = idle
            && device.is_some()
            && self.worker.is_none()
            && self.session.is_some()
            && !self.needs_get;
        for (action, title, enabled) in [
            (Action::Send, "Send to POD", self.has_pending()),
            (Action::Undo, "Undo live edits", !self.has_pending()),
        ] {
            if ui
                .add_enabled(
                    ready && enabled && Self::admitted(action),
                    egui::Button::new(title),
                )
                .clicked()
                && let Some(device) = device
            {
                self.start(action, device, ui.ctx());
            }
        }
        if self.stage_only && self.has_pending() && ui.button("Clear staged edits").clicked() {
            self.pending.0.clear();
        }
    }

    pub(super) fn status_text(&self) -> Option<&str> {
        self.worker
            .as_ref()
            .map(|worker| worker.action.label())
            .or_else(|| {
                self.message.as_ref().map(|result| match result {
                    Ok(message) | Err(message) => message.as_str(),
                })
            })
    }

    pub(super) fn status(&self, ui: &mut egui::Ui) {
        if let Some(worker) = &self.worker {
            ui.weak(format!("{}…", worker.action.label()));
        } else if let Some(message) = &self.message {
            match message {
                Ok(text) => {
                    ui.weak(text);
                }
                Err(error) => {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
            }
        }
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        services: &mut EditorServices<'_>,
        device: Option<&detection::DeviceKey>,
        read_idle: bool,
    ) {
        ui.separator();
        ui.heading("PODxt Live edit buffer");
        let snapshot = self.session.is_some();
        let idle = device.is_some() && read_idle && self.worker.is_none();
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    idle && Self::admitted(Action::Get),
                    egui::Button::new(Action::Get.label()),
                )
                .clicked()
            {
                action = Some(Action::Get);
            }
            if snapshot
                && ui
                    .add_enabled(idle, egui::Button::new("End edit session"))
                    .clicked()
            {
                self.detach();
            }
        });
        ui.horizontal_wrapped(|ui| {
            let channel = self.session.as_ref().map(|active| active.current.channel);
            if ui
                .add_enabled(
                    idle && !self.needs_get && channel.is_some(),
                    egui::Button::new("Verify backup for whole-tone resend…"),
                )
                .clicked()
                && let Some(channel) = channel
            {
                services.request_picker::<PodxtEditor>(
                    ui.ctx(),
                    PickRequest::Folder,
                    move |editor, folder, _| {
                        let Some(folder) = folder else { return };
                        let editor = &mut editor.live.editor;
                        if editor.busy()
                            || editor.needs_get
                            || !editor
                                .session
                                .as_ref()
                                .is_some_and(|active| active.current.channel == channel)
                        {
                            return;
                        }
                        editor.whole_tone_backup = None;
                        match podxt::verify_tone_backup(&folder, channel) {
                            Ok(backup) => {
                                editor.message = None;
                                editor.whole_tone_backup = Some(backup);
                            }
                            Err(error) => editor.message = Some(Err(error)),
                        }
                    },
                );
            }
            let backup = self
                .whole_tone_backup
                .as_ref()
                .filter(|backup| Some(backup.slot()) == channel);
            if let Some(backup) = backup {
                ui.label(format!(
                    "Verified backup · {} · {}",
                    backup.slot().slot_name(),
                    backup.tone_name()
                ));
            }
            let resend = ui
                .add_enabled(
                    idle && backup.is_some()
                        && !self.needs_get
                        && self.pending.0.is_empty()
                        && Self::admitted(Action::ResendWholeTone),
                    egui::Button::new(Action::ResendWholeTone.label()),
                )
                .on_disabled_hover_text(if WHOLE_TONE_SEND_APPROVED {
                    "Get the edit buffer and verify the playing channel's backup first."
                } else {
                    "The separate #222 owner run is awaiting approval."
                });
            if resend.clicked() {
                action = Some(Action::ResendWholeTone);
            }
        });
        if let (Some(action), Some(device)) = (action, device) {
            self.start(action, device, ui.ctx());
        }
        if !snapshot {
            ui.weak("Get edit buffer takes the snapshot for undo.");
        }
        if let Some(worker) = &self.worker {
            ui.label(format!("{} in progress…", worker.action.label()));
        }
        if let Some(message) = &self.message {
            match message {
                Ok(text) => {
                    ui.label(text);
                }
                Err(error) => {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
            }
        }
        let Some(active) = &self.session else {
            return;
        };
        ui.label(format!(
            "Undo snapshot · {} · {}",
            active.original.channel.label(),
            active.original.tone.name()
        ));
    }
}

#[cfg(test)]
mod tests;

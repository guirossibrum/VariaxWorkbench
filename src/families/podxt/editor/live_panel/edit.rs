use super::*;
use podxt::{
    EditorBuffer, EditorEdit, LiveTarget, Target, Value, VerifiedToneBackup, WholeToneSource,
};

const EDIT_INTERVAL: Duration = Duration::from_millis(150);
// #222's full-payload transfer has a separate first-run decision.
const WHOLE_TONE_SEND_APPROVED: bool = false;

#[derive(Default)]
pub(super) struct LiveEditor {
    session: Option<ActiveEdit>,
    worker: Option<EditWorker>,
    pending: Pending,
    whole_tone_backup: Option<VerifiedToneBackup>,
    stage_only: bool,
    needs_get: bool,
    last_dispatch: Option<Instant>,
    message: Option<Result<String, String>>,
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
    Undo,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Get => "Get edit buffer",
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

#[cfg(any(test, target_os = "linux"))]
struct EditPolicy(Action);

#[cfg(any(test, target_os = "linux"))]
impl GatePolicy for EditPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::PodxtIdentity
            | Operation::PodxtRead
            | Operation::PodxtGlobalRead
            | Operation::PodxtPackQuery => !hardware_gates::is_locked(Gate::CurrentSettingsRead),
            Operation::PodxtLibraryRead => {
                self.0 == Action::ResendWholeTone
                    && !hardware_gates::is_locked(Gate::FullBackupRead)
            }
            Operation::PodxtEditBufferWrite | Operation::PodxtAdditionalEditBufferWrite => {
                self.0 != Action::Get && !hardware_gates::is_locked(Gate::EditBufferWrite)
            }
            Operation::PodxtKeepEditBufferChanges => {
                self.0 != Action::Get && !hardware_gates::is_locked(Gate::KeepLiveEdit)
            }
            _ => false,
        }
    }
}

struct Job {
    action: Action,
    whole_tone_backup: Option<VerifiedToneBackup>,
    current: Option<EditorBuffer>,
    original: Option<EditorBuffer>,
    targets: Vec<LiveTarget>,
}

#[cfg(any(test, target_os = "linux"))]
fn edit_transaction<G, L: Link>(
    job: &Job,
    stop: &AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    open: impl FnOnce(&G) -> Result<L, String>,
) -> Result<EditorBuffer, String> {
    let run = || {
        let stopped = || stop.load(Ordering::Acquire);
        with_session(stopped, lock, open, &EditPolicy(job.action), |session| {
            let edit = match job.action {
                Action::Get => return podxt::get_editor_buffer(session, stopped),
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
                Action::Undo => EditorEdit::Undo(job.original.as_ref().expect("undo has snapshot")),
            };
            podxt::edit_editor_buffer(
                session,
                job.current.as_ref().expect("editing has snapshot"),
                edit,
                stopped,
            )
        })
    };
    if job.action == Action::Get {
        run()
    } else {
        hardware_gates::run_if_unlocked(Gate::EditBufferWrite, || {
            hardware_gates::run_if_unlocked(Gate::KeepLiveEdit, || {
                if job.action == Action::ResendWholeTone {
                    hardware_gates::run_if_unlocked(Gate::FullBackupRead, run)
                } else {
                    run()
                }
            })
        })
    }
}

#[cfg(all(target_os = "linux", not(test)))]
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
        |_| open_platform(device),
    )
}

#[cfg(any(test, not(target_os = "linux")))]
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
        cfg!(all(target_os = "linux", not(test)))
            && (action != Action::ResendWholeTone
                || WHOLE_TONE_SEND_APPROVED && !hardware_gates::is_locked(Gate::FullBackupRead))
            && !hardware_gates::is_locked(Gate::OsDiscovery)
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            && (action == Action::Get
                || !hardware_gates::is_locked(Gate::EditBufferWrite)
                    && !hardware_gates::is_locked(Gate::KeepLiveEdit))
    }

    fn start(&mut self, action: Action, device: &detection::DeviceKey, ctx: &egui::Context) {
        if self.worker.is_some()
            || !Self::admitted(action)
            || action == Action::ResendWholeTone && self.whole_tone_backup.is_none()
            || action != Action::Get && (self.session.is_none() || self.needs_get)
        {
            return;
        }
        let targets = match action {
            Action::Send | Action::Controls => std::mem::take(&mut self.pending.0),
            Action::Get | Action::Undo | Action::ResendWholeTone => {
                self.pending.0.clear();
                Vec::new()
            }
        };
        if action == Action::Get {
            self.whole_tone_backup = None;
        }
        let job = Job {
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
            for candidate in [Action::Get, Action::Send, Action::Undo] {
                let enabled = idle
                    && Self::admitted(candidate)
                    && (candidate == Action::Get || snapshot && !self.needs_get);
                if ui
                    .add_enabled(enabled, egui::Button::new(candidate.label()))
                    .clicked()
                {
                    action = Some(candidate);
                }
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
        let editing = device.is_some()
            && read_idle
            && !self.needs_get
            && Self::admitted(Action::Controls)
            && self
                .worker
                .as_ref()
                .is_none_or(|worker| worker.action == Action::Controls);
        if !Self::admitted(Action::Controls) && device.is_some() {
            ui.weak("Live writes are unavailable in this build.");
        }
        ui.add_enabled(
            idle && editing && self.pending.0.is_empty(),
            egui::Checkbox::new(&mut self.stage_only, "Stage changes for Send edit buffer"),
        );
        if self.stage_only && !self.pending.0.is_empty() {
            ui.label(format!("{} staged controls", self.pending.0.len()));
            if ui
                .add_enabled(idle, egui::Button::new("Clear staged changes"))
                .clicked()
            {
                self.pending.0.clear();
            }
        }
        let in_flight = self
            .worker
            .as_ref()
            .filter(|worker| worker.action == Action::Controls)
            .map_or(&[][..], |worker| worker.targets.as_slice());
        ui.add_enabled_ui(editing, |ui| {
            show_controls(ui, &active.current, &mut self.pending, in_flight);
        });
    }
}

fn show_controls(
    ui: &mut egui::Ui,
    buffer: &EditorBuffer,
    pending: &mut Pending,
    in_flight: &[LiveTarget],
) {
    use variax_workbench::pod_preset::pod_parameter_table;

    let model = |block: &str| -> Option<u32> {
        let setting = match block {
            "stomp" => Setting::StompModel,
            "mod" => Setting::ModModel,
            "delay" => Setting::DelayModel,
            "reverb" => Setting::ReverbModel,
            _ => return None,
        };
        // The instrument's readback defines the layout until the selector
        // is received. Staged model changes won't relabel the old knobs.
        let index = LiveControl::Additional(setting).read(&buffer.tone);
        setting.model_list()?.get(usize::from(index)).copied()
    };
    let table = pod_parameter_table();
    let controls: Vec<_> = Control::ALL
        .into_iter()
        .map(LiveControl::Basic)
        .chain(Setting::ALL.into_iter().map(LiveControl::Additional))
        .filter_map(|control| Some((control, table.wire_label(control.cc(), model)?)))
        .collect();
    for (block, title) in super::super::parameter_panel::GROUPS {
        if !controls.iter().any(|(_, label)| label.block == block) {
            continue;
        }
        ui.collapsing(title, |ui| {
            for (control, label) in controls.iter().filter(|(_, label)| label.block == block) {
                let value = |control: LiveControl| {
                    pending.overlay(in_flight, control, control.read(&buffer.tone))
                };
                let current = value(*control);
                let model_change_pending = controls.iter().any(|(other, other_label)| {
                    other_label.block == block
                        && matches!(other, LiveControl::Additional(setting) if setting.model_list().is_some())
                        && value(*other) != other.read(&buffer.tone)
                });
                let model_control = matches!(control, LiveControl::Additional(setting) if setting.model_list().is_some());
                let manual_disabled = match control {
                    LiveControl::Additional(Setting::ModSpeed) => value(LiveControl::Additional(Setting::ModNote)) != 0,
                    LiveControl::Additional(Setting::DelayTime) => value(LiveControl::Additional(Setting::DelayNote)) != 0,
                    _ => false,
                };
                let changed = ui.push_id(control.cc(), |ui| {
                    ui.add_enabled_ui(!manual_disabled && (!model_change_pending || model_control), |ui| {
                        input(ui, *control, label, current, control.read(&buffer.tone), buffer.packs)
                    }).inner
                }).inner;
                if let Some(target) = changed {
                    pending.set(target);
                    ui.ctx().request_repaint();
                }
            }
        });
    }
}

fn input(
    ui: &mut egui::Ui,
    control: LiveControl,
    label: &variax_workbench::pod_preset::PodWireLabel<'_>,
    mut value: u16,
    device_value: u16,
    packs: Packs,
) -> Option<LiveTarget> {
    let old = value;
    if let LiveControl::Additional(setting) = control
        && let Some(models) = setting.model_list()
    {
        let name = |index: u16| {
            models
                .get(usize::from(index))
                .and_then(|&id| podxt::models::name(id))
                .unwrap_or("Unknown model")
        };
        let available =
            |id| podxt::models::required_pack(id).is_none_or(|mask| packs.0 & mask != 0);
        let starting_model_available = models
            .get(usize::from(device_value))
            .is_some_and(|&id| available(id));
        ui.add_enabled_ui(starting_model_available, |ui| {
            egui::ComboBox::from_label(label.label)
                .selected_text(name(value))
                .show_ui(ui, |ui| {
                    for (index, _) in models.iter().enumerate().filter(|&(_, &id)| available(id)) {
                        let index = index as u16;
                        ui.selectable_value(&mut value, index, name(index));
                    }
                });
        })
        .response
        .on_disabled_hover_text(
            "The starting model is unavailable for undo with the installed packs.",
        );
    } else {
        let choices: Vec<_> = match control {
            LiveControl::Additional(Setting::PedalAssign) => label
                .choices()
                .filter_map(|(index, label)| Some((*[0, 64, 127].get(index as usize)?, label)))
                .collect(),
            LiveControl::Additional(Setting::TweakParam) => Vec::new(),
            _ => label
                .choices()
                .filter_map(|(index, label)| Some((u16::try_from(index).ok()?, label)))
                .collect(),
        };
        if !choices.is_empty() {
            let expected = |index: u16| match control {
                LiveControl::Basic(control) if control.is_switch() => {
                    u16::from(control.switch_value(index != 0))
                }
                _ => index,
            };
            let selected = choices
                .iter()
                .find(|&&(index, _)| match control {
                    LiveControl::Basic(control) if control.is_switch() => {
                        control.is_on(value as u8) == (index != 0)
                    }
                    _ => value == expected(index),
                })
                .map_or_else(|| control.describe(value), |&(_, label)| label.to_owned());
            egui::ComboBox::from_label(label.label)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (index, label) in choices {
                        ui.selectable_value(&mut value, expected(index), label);
                    }
                });
        } else {
            match control {
                LiveControl::Basic(control) if control.is_switch() => {
                    let mut on = control.is_on(value as u8);
                    if ui.checkbox(&mut on, label.label).changed() {
                        value = u16::from(control.switch_value(on));
                    }
                }
                LiveControl::Additional(Setting::Tempo) => {
                    let mut bpm = f64::from(value) / 10.0;
                    if ui
                        .add(
                            egui::Slider::new(&mut bpm, 30.0..=240.0)
                                .fixed_decimals(1)
                                .suffix(" BPM")
                                .text(label.label),
                        )
                        .changed()
                    {
                        value = (bpm * 10.0).round() as u16;
                    }
                }
                LiveControl::Additional(Setting::GateThreshold) => {
                    let mut db = -(i32::from(value));
                    if ui
                        .add(
                            egui::Slider::new(&mut db, -96..=0)
                                .suffix(" dB")
                                .text(label.label),
                        )
                        .changed()
                    {
                        value = (-db) as u16;
                    }
                }
                _ => {
                    let max = match control {
                        LiveControl::Additional(Setting::ModSpeed | Setting::DelayTime) => 16383,
                        LiveControl::Additional(Setting::TweakParam) => 13,
                        _ => 127,
                    };
                    let mut slider = egui::Slider::new(&mut value, 0..=max)
                        .clamping(egui::SliderClamping::Edits)
                        .text(label.label);
                    if max == 16383 {
                        slider = slider.suffix(" raw");
                    }
                    ui.add(slider);
                }
            }
        }
    }
    (value != old).then(|| match control {
        LiveControl::Basic(control) => LiveTarget::Basic(
            control,
            Target::Value(Value::new(value as u8).expect("7-bit input")),
        ),
        LiveControl::Additional(setting) => LiveTarget::Additional(setting, value),
    })
}

#[cfg(test)]
mod tests;

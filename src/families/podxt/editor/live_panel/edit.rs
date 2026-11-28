use super::*;
use podxt::{
    EditorBuffer, EditorRecall, LiveTarget, RecallConfirmation, Target, TonePayload,
    VerifiedToneBackup,
};
#[cfg(any(test, target_os = "linux", target_os = "macos"))]
use podxt::{EditorEdit, WholeToneSource};

const EDIT_INTERVAL: Duration = Duration::from_millis(150);
const UNDO_PENDING: &str = "Undo pending while the live control change finishes";
// #222's full-payload transfer has a separate first-run decision.
const WHOLE_TONE_SEND_APPROVED: bool = false;
// Approval of a manual resend does not admit automatic replay.
const AUTOMATIC_DRAFT_REPLAY_APPROVED: bool = false;

pub(super) struct LiveEditor {
    pub(super) reply_pending: Arc<AtomicBool>,
    session: Option<ActiveEdit>,
    drafts: Drafts,
    worker: Option<EditWorker>,
    pending: Pending,
    queued_undo: Option<QueuedUndo>,
    whole_tone_backup: Option<Arc<VerifiedToneBackup>>,
    replay_backups: HashMap<u16, Arc<VerifiedToneBackup>>,
    attachment_session: Arc<()>,
    stage_only: bool,
    needs_get: bool,
    recall_confirmation: Option<(detection::DeviceKey, RecallConfirmation)>,
    last_dispatch: Option<Instant>,
    message: Option<Result<String, String>>,
    #[cfg(test)]
    fake_get: bool,
    #[cfg(test)]
    fake_recall: Option<tests::FakeRecall>,
    #[cfg(test)]
    fake_edit: Option<tests::FakeEdit>,
    #[cfg(test)]
    fake_dispatch: Option<Arc<tests::FakeDispatch>>,
}

impl Default for LiveEditor {
    fn default() -> Self {
        Self {
            reply_pending: Arc::default(),
            session: None,
            drafts: Drafts::default(),
            worker: None,
            pending: Pending::default(),
            queued_undo: None,
            whole_tone_backup: None,
            replay_backups: HashMap::new(),
            attachment_session: Arc::new(()),
            stage_only: true,
            needs_get: false,
            recall_confirmation: None,
            last_dispatch: None,
            message: None,
            #[cfg(test)]
            fake_get: false,
            #[cfg(test)]
            fake_recall: None,
            #[cfg(test)]
            fake_edit: None,
            #[cfg(test)]
            fake_dispatch: None,
        }
    }
}

struct ActiveEdit {
    original: EditorBuffer,
    current: EditorBuffer,
}

#[derive(Default)]
struct Drafts {
    // LivePanel rotates this map whenever the verified attachment changes.
    attached: HashMap<u16, TonePayload>,
    previous_attachments: Vec<HashMap<u16, TonePayload>>,
}

impl Drafts {
    fn end_attachment(&mut self) {
        if !self.attached.is_empty() {
            self.previous_attachments
                .push(std::mem::take(&mut self.attached));
        }
    }

    fn record_edit(&mut self, active: &ActiveEdit, observed: &EditorBuffer) {
        let slot = observed.channel;
        if active.current.channel != slot || slot.kind() != BankKind::Tones {
            return;
        }
        let (Ok(before), Ok(after), Ok(original)) = (
            TonePayload::from_dump(&active.current.tone),
            TonePayload::from_dump(&observed.tone),
            TonePayload::from_dump(&active.original.tone),
        ) else {
            return;
        };
        // A readback of a no-op must leave a previously retained draft alone.
        if before != after {
            if after == original {
                self.attached.remove(&slot.address());
            } else {
                self.attached.insert(slot.address(), after);
            }
        }
    }
}

struct QueuedUndo {
    device: detection::DeviceKey,
    snapshot: EditorBuffer,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Get,
    Follow(Location),
    ReplayDraft(Location),
    Controls,
    Send,
    ResendWholeTone,
    Recall(Location),
    Undo,
}

impl Action {
    fn native_approval(self) -> bool {
        match self {
            Self::ReplayDraft(_) => WHOLE_TONE_SEND_APPROVED && AUTOMATIC_DRAFT_REPLAY_APPROVED,
            Self::ResendWholeTone => WHOLE_TONE_SEND_APPROVED,
            _ => true,
        }
    }

    fn is_whole_tone_send(self) -> bool {
        matches!(self, Self::ResendWholeTone | Self::ReplayDraft(_))
    }

    fn is_read(self) -> bool {
        matches!(self, Self::Get | Self::Follow(_))
    }

    fn progress_label(self) -> Option<&'static str> {
        (self != Self::Controls).then(|| self.label())
    }

    fn label(self) -> &'static str {
        match self {
            Self::Get | Self::Follow(_) => "Read playing tone",
            Self::ReplayDraft(_) => "Restore session draft",
            Self::Recall(_) => "Select channel",
            Self::Controls => "Apply live controls",
            Self::Send => "Send edit buffer",
            Self::ResendWholeTone => "Resend whole current tone",
            Self::Undo => "Undo to snapshot",
        }
    }
}

#[derive(Debug)]
enum EditOutcome {
    Buffer(EditorBuffer),
    Recall(EditorRecall),
}

struct EditWorker {
    action: Action,
    device: Option<detection::DeviceKey>,
    stop: Arc<AtomicBool>,
    targets: Vec<LiveTarget>,
    superseded: bool,
    replay: Option<Arc<DraftReplay>>,
    thread: Option<std::thread::JoinHandle<Result<EditOutcome, String>>>,
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
        if ReadBufferPolicy.allows(operation) {
            return true;
        }
        match operation {
            Operation::PodxtLibraryRead => {
                (self.0.is_whole_tone_send() || matches!(self.0, Action::Recall(_)))
                    && !hardware_gates::is_locked(Gate::FullBackupRead)
            }
            Operation::PodxtChannelSwitch => {
                matches!(self.0, Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::DeviceCheckChannel)
            }
            Operation::PodxtEditBufferWrite | Operation::PodxtAdditionalEditBufferWrite => {
                !self.0.is_read()
                    && !matches!(self.0, Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::EditBufferWrite)
            }
            Operation::PodxtKeepEditBufferChanges => {
                !self.0.is_read()
                    && !matches!(self.0, Action::Recall(_))
                    && !hardware_gates::is_locked(Gate::KeepLiveEdit)
            }
            _ => false,
        }
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
pub(super) struct ReadBufferPolicy;

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
impl GatePolicy for ReadBufferPolicy {
    fn allows(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::PodxtIdentity
                | Operation::PodxtRead
                | Operation::PodxtGlobalRead
                | Operation::PodxtPackQuery
        ) && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
    }
}

struct DraftReplay {
    attachment_session: Arc<()>,
    expected: EditorBuffer,
    payload: TonePayload,
    backup: Arc<VerifiedToneBackup>,
}

struct Job {
    recall_confirmation: Option<RecallConfirmation>,
    reply_pending: Arc<AtomicBool>,
    action: Action,
    whole_tone_backup: Option<Arc<VerifiedToneBackup>>,
    replay: Option<Arc<DraftReplay>>,
    current: Option<EditorBuffer>,
    original: Option<EditorBuffer>,
    targets: Vec<LiveTarget>,
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn edit_transaction<'a, G>(
    job: &'a Job,
    stop: &'a AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    with_link: impl FnOnce(&G, RunOnLink<'a, EditOutcome>) -> Result<EditOutcome, String>,
) -> Result<EditOutcome, String> {
    let run = || {
        let stopped = || stop.load(Ordering::Acquire);
        with_session(
            stopped,
            &job.reply_pending,
            lock,
            with_link,
            EditPolicy(job.action),
            move |session| {
                let edit = match job.action {
                    Action::Get | Action::Follow(_) => {
                        return track_reply(&job.reply_pending, |outstanding| {
                            podxt::get_editor_buffer_tracking_reply(session, stopped, outstanding)
                        })
                        .map(EditOutcome::Buffer);
                    }
                    Action::Recall(slot) => {
                        return podxt::recall_live_channel(
                            session,
                            slot,
                            job.recall_confirmation.as_ref(),
                            stopped,
                        )
                        .map(EditOutcome::Recall);
                    }
                    Action::ReplayDraft(slot) => {
                        let replay = job
                            .replay
                            .as_ref()
                            .expect("draft replay has verified selection");
                        if replay.expected.channel != slot || replay.backup.slot() != slot {
                            return Err("The draft replay target changed.".into());
                        }
                        return podxt::send_whole_tone(
                            session,
                            &replay.expected,
                            WholeToneSource::Payload(&replay.payload),
                            &replay.backup,
                            stopped,
                        )
                        .map(EditOutcome::Buffer);
                    }
                    Action::ResendWholeTone => {
                        return podxt::send_whole_tone(
                            session,
                            job.current.as_ref().expect("whole-tone send has snapshot"),
                            WholeToneSource::Current,
                            job.whole_tone_backup
                                .as_deref()
                                .ok_or("Verify a backup for the playing channel first.")?,
                            stopped,
                        )
                        .map(EditOutcome::Buffer);
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
                .map(EditOutcome::Buffer)
            },
        )
    };
    match job.action {
        Action::Get | Action::Follow(_) => run(),
        Action::Recall(_) => hardware_gates::run_if_unlocked(Gate::DeviceCheckChannel, || {
            hardware_gates::run_if_unlocked(Gate::FullBackupRead, run)
        }),
        _ => hardware_gates::run_if_unlocked(Gate::EditBufferWrite, || {
            hardware_gates::run_if_unlocked(Gate::KeepLiveEdit, || {
                if job.action.is_whole_tone_send() {
                    hardware_gates::run_if_unlocked(Gate::FullBackupRead, run)
                } else {
                    run()
                }
            })
        }),
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn wait_for_edit_lock<T>(
    stop: &AtomicBool,
    mut acquire: impl FnMut() -> Result<T, line6_devices::device_lock::LockError>,
    mut pause: impl FnMut(Duration),
) -> Result<T, String> {
    use line6_devices::device_lock::LockError;

    for attempt in 0..5 {
        if stop.load(Ordering::Acquire) {
            break;
        }
        match acquire() {
            Ok(guard) if !stop.load(Ordering::Acquire) => return Ok(guard),
            Ok(_) => break,
            Err(LockError::Busy | LockError::BusyHere) if attempt < 4 => {
                pause(Duration::from_millis(500));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("PODxt Live operation cancelled before opening the connection.".into())
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn platform_edit(
    device: &detection::DeviceKey,
    job: &Job,
    stop: &AtomicBool,
) -> Result<EditOutcome, String> {
    if !job.action.native_approval() {
        return Err(format!("{} awaits owner approval.", job.action.label()));
    }
    edit_transaction(
        job,
        stop,
        || {
            wait_for_edit_lock(
                stop,
                line6_devices::device_lock::DeviceLock::acquire,
                std::thread::sleep,
            )
        },
        |rig, run| {
            if matches!(
                job.action,
                Action::Recall(_)
                    | Action::Follow(_)
                    | Action::ReplayDraft(_)
                    | Action::Controls
                    | Action::Undo
            ) {
                with_checked_edit_link(device, rig, stop, run)
            } else {
                with_platform_link(device, rig, run)
            }
        },
    )
}

#[cfg(all(not(test), any(target_os = "linux", target_os = "macos")))]
fn with_checked_edit_link(
    device: &detection::DeviceKey,
    rig: &line6_devices::device_lock::DeviceLock,
    stop: &AtomicBool,
    run: RunOnLink<'_, EditOutcome>,
) -> Result<EditOutcome, String> {
    #[cfg(target_os = "linux")]
    {
        use line6_devices::transport::rawmidi::{RawMidiAttachment, ScopedRawMidiLink};

        let (path, address) = device
            .attachment
            .usb_location()
            .ok_or("The verified PODxt Live USB attachment is no longer available.")?;
        let selected = RawMidiAttachment::select_podxt_live(rig, path, address)?;
        let mut link = ScopedRawMidiLink::open_attachment_borrowed(&selected, rig, || {
            stop.load(Ordering::Acquire)
        })
        .map_err(|error| error.to_string())?;
        let result = run(&mut link);
        match link.finish() {
            Ok(()) => result,
            Err(close) => Err(match result {
                Ok(_) => close.to_string(),
                Err(error) => format!("{error} Cleanup also failed. {close}"),
            }),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = stop;
        with_verified_platform_link(device, rig, run)
    }
}

#[cfg(any(test, not(any(target_os = "linux", target_os = "macos"))))]
fn platform_edit(
    _device: &detection::DeviceKey,
    job: &Job,
    _stop: &AtomicBool,
) -> Result<EditOutcome, String> {
    let _ = (
        job.action,
        &job.whole_tone_backup,
        &job.replay,
        &job.current,
        &job.original,
        &job.targets,
        &job.recall_confirmation,
    );
    let _ = (EditOutcome::Buffer, EditOutcome::Recall);
    Err("PODxt Live editing is unavailable in this build.".into())
}

impl LiveEditor {
    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || !self.stage_only && !self.pending.0.is_empty()
    }

    pub(super) fn cancel(&mut self) {
        self.clear_recall_confirmation();
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
        self.drafts.end_attachment();
        self.replay_backups.clear();
        self.attachment_session = Arc::new(());
        self.whole_tone_backup = None;
        self.pending.0.clear();
        self.needs_get = false;
        self.last_dispatch = None;
        self.message = None;
        self.cancel_queued_undo("The PODxt Live attachment changed.");
    }

    #[cfg(test)]
    pub(super) fn stage_test_target(&mut self, target: LiveTarget) {
        self.pending.set(target);
    }

    #[cfg(test)]
    pub(super) fn use_fake_recall(&mut self) {
        self.fake_recall = Some(tests::FakeRecall::Clean);
    }

    #[cfg(test)]
    pub(super) fn use_fake_recall_with_changed_source(&mut self) {
        self.fake_recall = Some(tests::FakeRecall::ChangedSource);
    }

    #[cfg(test)]
    pub(super) fn publish_test_edit(
        &mut self,
        device: &detection::DeviceKey,
        before: EditorBuffer,
        after: EditorBuffer,
    ) {
        self.accept_follow(before);
        self.complete_job(
            Action::Controls,
            device,
            false,
            Ok(EditOutcome::Buffer(after)),
        );
    }

    #[cfg(test)]
    pub(super) fn use_fake_get(&mut self) {
        self.fake_get = true;
    }

    pub(super) fn detach_attachment(&mut self) {
        let staged = if self.stage_only {
            let mut staged = std::mem::take(&mut self.pending);
            if let Some(worker) = &self.worker
                && worker.action == Action::Send
            {
                for &target in &worker.targets {
                    staged.set(target);
                }
            }
            staged
        } else {
            Pending::default()
        };
        self.detach();
        self.pending = staged;
    }

    pub(super) fn draft(&self, slot: Location) -> Option<&TonePayload> {
        self.drafts.attached.get(&slot.address())
    }

    pub(super) fn draft_counts(&self) -> (usize, usize) {
        (
            self.drafts.attached.len(),
            self.drafts
                .previous_attachments
                .iter()
                .map(HashMap::len)
                .sum(),
        )
    }

    pub(super) fn reconcile_stored(&mut self, stored: &podxt::StoredTone) {
        if let Ok(saved) = TonePayload::from_dump(&stored.tone)
            && self.draft(stored.location) == Some(&saved)
        {
            self.drafts.attached.remove(&stored.location.address());
        }
    }

    pub(super) fn playing_draft_status(&self, slot: Location) -> Option<&'static str> {
        let draft = self.draft(slot)?;
        let confirmed = self.session.as_ref().filter(|active| {
            active.current.channel == slot
                && !self.needs_get
                && !self.reply_pending.load(Ordering::Acquire)
        });
        Some(match confirmed {
            Some(active) if TonePayload::from_dump(&active.current.tone).as_ref() == Ok(draft) => {
                "Session draft · playing"
            }
            Some(_) => "Draft retained · playing tone differs",
            None => "Draft retained · playing tone unverified",
        })
    }

    pub(super) fn discard_staged(&mut self) {
        self.pending.0.clear();
    }

    pub(super) fn observe_program(&mut self, slot: Location) {
        if let Some(worker) = &mut self.worker {
            match worker.action {
                // The app's own program change can reach passive Follow.
                Action::Recall(target) if slot != target => worker.superseded = true,
                Action::Follow(_) => worker.superseded = true,
                Action::ReplayDraft(_) => worker.stop.store(true, Ordering::Release),
                _ => {}
            }
        }
    }

    pub(super) fn accept_program_follow(
        &mut self,
        buffer: EditorBuffer,
        target: Option<Location>,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
    ) {
        self.accept_follow(buffer);
        if let Some(target) = target {
            self.replay_selected(target, device, ctx);
        }
    }

    pub(super) fn accept_follow(&mut self, buffer: EditorBuffer) {
        if let Some(worker) = &self.worker
            && matches!(worker.action, Action::ReplayDraft(_))
        {
            worker.stop.store(true, Ordering::Release);
        }
        self.whole_tone_backup = None;
        let expired = self
            .recall_confirmation
            .as_ref()
            .is_some_and(|(_, confirmation)| !confirmation.matches_playing(&buffer));
        let changed_during_undo = self.queued_undo.is_some()
            && self.session.as_ref().is_none_or(|active| {
                active.current.channel != buffer.channel || active.current.tone != buffer.tone
            });
        self.completed(Action::Get, Ok(buffer));
        if expired {
            self.recall_confirmation = None;
            self.message = Some(Err(
                "The playing tone changed. Select the channel again.".into()
            ));
        } else {
            self.message = None;
        }
        if changed_during_undo {
            self.cancel_queued_undo("The playing tone changed.");
            // The in-flight result predates the newly observed tone.
            if let Some(worker) = &mut self.worker {
                worker.device = None;
                worker.stop.store(true, Ordering::Release);
            }
        }
    }

    pub(super) fn recall_ready(&self) -> bool {
        self.session.is_some() && !self.needs_get
    }

    pub(super) fn needs_refresh(&self) -> bool {
        !self.recall_ready()
    }

    pub(super) fn awaiting_recall_confirmation(&self) -> bool {
        self.recall_confirmation.is_some()
    }

    fn clear_recall_confirmation(&mut self) {
        self.recall_confirmation = None;
    }

    fn admitted(action: Action) -> bool {
        Self::admitted_on(platform_available(), action.native_approval(), action)
    }

    fn admitted_on(available: bool, approved: bool, action: Action) -> bool {
        available
            && approved
            && (!action.is_whole_tone_send() || !hardware_gates::is_locked(Gate::FullBackupRead))
            && !hardware_gates::is_locked(Gate::OsDiscovery)
            && !hardware_gates::is_locked(Gate::CurrentSettingsRead)
            && match action {
                Action::Get | Action::Follow(_) => true,
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

    fn admits(&self, action: Action) -> bool {
        let available = platform_available();
        #[cfg(test)]
        let available = available
            || self.fake_edit.is_some() && matches!(action, Action::Controls | Action::Undo);
        #[cfg(test)]
        if self.fake_dispatch.is_some() {
            return Self::admitted_on(true, true, action);
        }
        Self::admitted_on(available, action.native_approval(), action)
    }

    fn start(&mut self, action: Action, device: &detection::DeviceKey, ctx: &egui::Context) {
        self.start_job(action, device, ctx, None, None);
    }

    fn start_job(
        &mut self,
        action: Action,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
        recall_confirmation: Option<RecallConfirmation>,
        replay: Option<Arc<DraftReplay>>,
    ) {
        let admitted = if action == Action::Get {
            self.can_get()
        } else {
            self.admits(action)
        };
        #[cfg(test)]
        let admitted = admitted
            || self.fake_recall.is_some()
                && matches!(action, Action::Recall(_))
                && Self::admitted_on(true, action.native_approval(), action);
        // An initial recall reads the playing channel and buffer on the same
        // verified link before it can select anything.
        let fresh_recall = matches!(action, Action::Recall(_)) && recall_confirmation.is_none();
        if self.worker.is_some()
            || self.reply_pending.load(Ordering::Acquire)
            || !admitted
            || action == Action::ResendWholeTone && self.whole_tone_backup.is_none()
            || !action.is_read() && !fresh_recall && !self.recall_ready()
            || action == Action::Undo && !self.can_undo()
            || matches!(action, Action::Recall(_)) && (self.stage_only || self.has_pending())
        {
            return;
        }
        self.clear_recall_confirmation();
        let targets = match action {
            Action::Send | Action::Controls => std::mem::take(&mut self.pending.0),
            Action::Recall(_) => Vec::new(),
            Action::Get
            | Action::Follow(_)
            | Action::Undo
            | Action::ResendWholeTone
            | Action::ReplayDraft(_) => {
                self.pending.0.clear();
                Vec::new()
            }
        };
        if matches!(action, Action::Get | Action::Follow(_) | Action::Recall(_)) {
            self.whole_tone_backup = None;
        }
        let job = Job {
            recall_confirmation,
            reply_pending: Arc::clone(&self.reply_pending),
            action,
            whole_tone_backup: if action == Action::ResendWholeTone {
                self.whole_tone_backup.take()
            } else {
                None
            },
            replay: replay.clone(),
            current: self.session.as_ref().map(|active| active.current.clone()),
            original: self.session.as_ref().map(|active| active.original.clone()),
            targets: targets.clone(),
        };
        let endpoint = device.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let repaint = ctx.clone();
        #[cfg(test)]
        let fake_get = action == Action::Get && self.fake_get;
        #[cfg(test)]
        let fake_recall = self
            .fake_recall
            .filter(|_| matches!(action, Action::Recall(_)));
        #[cfg(test)]
        let fake_edit = self
            .fake_edit
            .clone()
            .filter(|_| matches!(action, Action::Controls | Action::Undo));
        #[cfg(test)]
        let fake_dispatch = self.fake_dispatch.clone();
        let thread = std::thread::Builder::new()
            .name("podxt-edit-buffer".into())
            .spawn(move || {
                let run = || platform_edit(&endpoint, &job, &worker_stop);
                #[cfg(test)]
                let run = || {
                    if let Some(dispatch) = fake_dispatch {
                        dispatch(&job, &worker_stop)
                    } else if fake_get {
                        tests::fake_get(&job, &worker_stop)
                    } else if let Some(scenario) = fake_recall {
                        tests::fake_recall(scenario, &job, &worker_stop)
                    } else if let Some(fake_edit) = fake_edit {
                        fake_edit.run(&job, &worker_stop)
                    } else {
                        run()
                    }
                };
                let result = run();
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
                    superseded: false,
                    replay,
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
                let message = match action {
                    Action::Controls => None,
                    Action::Get | Action::Follow(_) => Some(format!(
                        "Edit buffer read from {}.",
                        active.current.channel.label()
                    )),
                    Action::Undo => Some("Original snapshot verified byte for byte.".into()),
                    Action::Recall(slot) => Some(format!("Playing {}.", slot.label())),
                    Action::Send => Some("Edit buffer sent and read back.".into()),
                    Action::ResendWholeTone => Some("Whole tone sent and read back.".into()),
                    Action::ReplayDraft(_) => Some("Session draft restored and read back.".into()),
                }
                .map(Ok);
                // Keep any queued Undo cancellation visible after Controls completes.
                if action != Action::Controls {
                    self.message = message;
                }
                Some(&active.current)
            }
            Err(error) => {
                if !matches!(action, Action::Recall(_)) {
                    self.pending.0.clear();
                }
                self.needs_get = true;
                self.message = Some(Err(error));
                None
            }
        }
    }

    fn complete_job(
        &mut self,
        action: Action,
        device: &detection::DeviceKey,
        stopped: bool,
        result: Result<EditOutcome, String>,
    ) -> Option<&EditorBuffer> {
        match result {
            Ok(
                EditOutcome::Buffer(buffer) | EditOutcome::Recall(EditorRecall::Playing(buffer)),
            ) => {
                if matches!(action, Action::Controls | Action::Send | Action::Undo)
                    && !stopped
                    && !self.reply_pending.load(Ordering::Acquire)
                    && let Some(active) = &self.session
                {
                    self.drafts.record_edit(active, &buffer);
                }
                self.completed(action, Ok(buffer))
            }
            Ok(EditOutcome::Recall(EditorRecall::ConfirmationRequired(confirmation))) => {
                let playing = confirmation.playing().clone();
                self.completed(Action::Get, Ok(playing));
                if !stopped && !self.stage_only && !self.has_pending() {
                    self.recall_confirmation = Some((device.clone(), confirmation));
                }
                self.message = Some(Ok("No channel switch was sent.".into()));
                self.session.as_ref().map(|active| &active.current)
            }
            Ok(EditOutcome::Recall(EditorRecall::ConfirmationExpired(playing))) => {
                self.completed(Action::Get, Ok(playing));
                self.message = Some(Err(
                    "The playing tone changed. Select the channel again.".into()
                ));
                self.session.as_ref().map(|active| &active.current)
            }
            Err(error) => self.completed(action, Err(error)),
        }
    }

    fn replay_selected(
        &mut self,
        target: Location,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
    ) {
        let action = Action::ReplayDraft(target);
        if self.stage_only
            || self.has_pending()
            || !self.admits(action)
            || self.reply_pending.load(Ordering::Acquire)
            || !self.recall_ready()
        {
            return;
        }
        let Some(active) = self
            .session
            .as_ref()
            .filter(|active| active.current.channel == target)
        else {
            return;
        };
        let Some(payload) = self.draft(target) else {
            return;
        };
        if TonePayload::from_dump(&active.current.tone).as_ref() == Ok(payload) {
            return;
        }
        let Some(backup) = self.replay_backups.get(&target.address()) else {
            self.message = Some(Err(format!(
                "Draft retained. Verify a backup for {} before replay.",
                target.label()
            )));
            return;
        };
        let replay = Arc::new(DraftReplay {
            attachment_session: Arc::clone(&self.attachment_session),
            expected: active.current.clone(),
            payload: payload.clone(),
            backup: Arc::clone(backup),
        });
        self.start_job(action, device, ctx, None, Some(replay));
    }

    fn verify_replay_result(
        &self,
        replay: &DraftReplay,
        stopped: bool,
        result: Result<EditOutcome, String>,
    ) -> Result<EditorBuffer, String> {
        let buffer = match result {
            Ok(EditOutcome::Buffer(buffer)) => buffer,
            Ok(EditOutcome::Recall(_)) => {
                return Err("Draft retained. Unexpected replay result.".into());
            }
            Err(error) => return Err(format!("Draft retained. {error}")),
        };
        let target = replay.backup.slot();
        if stopped
            || self.stage_only
            || self.needs_get
            || !Arc::ptr_eq(&self.attachment_session, &replay.attachment_session)
            || self.reply_pending.load(Ordering::Acquire)
            || !self.session.as_ref().is_some_and(|active| {
                active.current.channel == target
                    && active.current.tone == replay.expected.tone
                    && active.current.packs == replay.expected.packs
            })
            || self.draft(target) != Some(&replay.payload)
            || buffer.channel != target
            || TonePayload::from_dump(&buffer.tone).as_ref() != Ok(&replay.payload)
        {
            return Err("Draft retained. The replay result could not be verified for the current selection.".into());
        }
        Ok(buffer)
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
            if let Some(device) = device.filter(|device| worker.device.as_ref() == Some(*device)) {
                if !self.stage_only && !self.pending.0.is_empty() {
                    ctx.request_repaint();
                }
                if worker.action == Action::Send && self.stage_only && result.is_err() {
                    self.needs_get = true;
                    self.pending.0 = worker.targets.clone();
                    self.message = Some(result.map(|_| String::new()));
                    return None;
                }
                let stopped = worker.stop.load(Ordering::Acquire);
                let selection = match (&worker.action, &result) {
                    (Action::Recall(target), Ok(EditOutcome::Recall(EditorRecall::Playing(_))))
                    | (Action::Follow(target), Ok(EditOutcome::Buffer(_)))
                        if !stopped && !worker.superseded =>
                    {
                        Some(*target)
                    }
                    _ => None,
                };
                if let Some(replay) = &worker.replay {
                    let result = self.verify_replay_result(replay, stopped, result);
                    self.completed(worker.action, result)?;
                    return self.session.as_ref().map(|active| &active.current);
                }
                let verified = self
                    .complete_job(worker.action, device, stopped, result)
                    .is_some();
                self.finish_queued_undo(device, read_idle, ctx, stopped, verified);
                if verified
                    && read_idle
                    && let Some(target) = selection
                {
                    self.replay_selected(target, device, ctx);
                }
                return if verified {
                    self.session.as_ref().map(|active| &active.current)
                } else {
                    None
                };
            }
            self.cancel_queued_undo("The PODxt Live attachment changed.");
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

    fn finish_queued_undo(
        &mut self,
        device: &detection::DeviceKey,
        read_idle: bool,
        ctx: &egui::Context,
        stopped: bool,
        verified: bool,
    ) {
        let Some(undo) = self.queued_undo.take() else {
            return;
        };
        let failure = if stopped {
            Some("The control operation was cancelled.")
        } else if self.reply_pending.load(Ordering::Acquire) {
            Some(PENDING_REPLY)
        } else if !verified {
            self.status_text()
                .or(Some("The control operation could not be verified."))
        } else if undo.device != *device {
            Some("The PODxt Live attachment changed.")
        } else if !self.session.as_ref().is_some_and(|active| {
            active.current.channel == undo.snapshot.channel
                && active.original.channel == undo.snapshot.channel
                && active.original.tone == undo.snapshot.tone
        }) {
            Some("The playing channel or Undo snapshot changed.")
        } else if !read_idle || self.stage_only || !self.admits(Action::Undo) {
            Some("The Live editor is no longer ready.")
        } else {
            None
        };
        if let Some(reason) = failure {
            self.message = Some(Err(format!("Undo cancelled. {reason}")));
        } else if self.can_undo() {
            self.start(Action::Undo, device, ctx);
        } else if self
            .session
            .as_ref()
            .is_some_and(|active| active.current.tone == undo.snapshot.tone)
        {
            self.message = Some(Ok("No live edits to restore.".into()));
        } else {
            self.message = Some(Err(
                "Undo cancelled. The playing tone has differences Undo cannot restore.".into(),
            ));
        }
    }

    fn cancel_queued_undo(&mut self, reason: &str) {
        if self.queued_undo.take().is_some() {
            self.message = Some(Err(format!("Undo cancelled. {reason}")));
        }
    }

    fn can_request_undo(&self, device: &detection::DeviceKey) -> bool {
        self.recall_ready()
            && !self.reply_pending.load(Ordering::Acquire)
            && self.admits(Action::Undo)
            && self.queued_undo.is_none()
            && match self.worker.as_ref() {
                Some(worker) => {
                    !self.stage_only
                        && worker.action == Action::Controls
                        && worker.device.as_ref() == Some(device)
                        && !worker.stop.load(Ordering::Acquire)
                }
                None => self.can_undo() || !self.stage_only && self.has_pending(),
            }
    }

    fn request_undo(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        if !self.can_request_undo(device) {
            return;
        }
        self.clear_recall_confirmation();
        self.pending.0.clear();
        if self.worker.is_some() {
            let active = self.session.as_ref().expect("Undo request has a snapshot");
            self.queued_undo = Some(QueuedUndo {
                device: device.clone(),
                snapshot: active.original.clone(),
            });
            ctx.request_repaint();
        } else if self.can_undo() {
            self.start(Action::Undo, device, ctx);
        } else {
            self.message = Some(Ok("Pending live edits discarded.".into()));
        }
    }

    fn can_undo(&self) -> bool {
        !self.has_pending()
            && self
                .session
                .as_ref()
                .is_some_and(|active| active.current.has_restorable_changes(&active.original))
    }

    pub(super) fn can_get(&self) -> bool {
        let available = platform_available();
        #[cfg(test)]
        let available = available || self.fake_get || self.fake_dispatch.is_some();
        self.worker.is_none()
            && !self.reply_pending.load(Ordering::Acquire)
            && !self.has_pending()
            && Self::admitted_on(available, true, Action::Get)
    }

    pub(super) fn get(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        if self.can_get() {
            self.start(Action::Get, device, ctx);
        }
    }

    pub(super) fn get_after_program(
        &mut self,
        target: Location,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
    ) {
        if self.can_get() {
            // A replay trigger needs the program target and checked release.
            let action = if self.admits(Action::ReplayDraft(target)) {
                Action::Follow(target)
            } else {
                Action::Get
            };
            self.start(action, device, ctx);
        }
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

    fn cancel_switch(&mut self) {
        self.clear_recall_confirmation();
        self.message = Some(Ok("Kept the playing tone.".into()));
    }

    fn take_recall_confirmation(
        &mut self,
        device: &detection::DeviceKey,
    ) -> Option<RecallConfirmation> {
        let (attachment, confirmation) = self.recall_confirmation.take()?;
        if attachment == *device
            && !self.stage_only
            && !self.has_pending()
            && self.recall_ready()
            && self
                .session
                .as_ref()
                .is_some_and(|active| confirmation.matches_playing(&active.current))
        {
            Some(confirmation)
        } else {
            self.message = Some(Err(
                "The recall choice expired. Select the channel again.".into()
            ));
            None
        }
    }

    pub(super) fn show_recall_confirmation(
        &mut self,
        ui: &mut egui::Ui,
        device: &detection::DeviceKey,
        idle: bool,
    ) {
        let Some((attachment, confirmation)) = &self.recall_confirmation else {
            return;
        };
        if attachment != device || self.stage_only || self.has_pending() {
            self.clear_recall_confirmation();
            return;
        }
        let target = confirmation.target();
        ui.label(format!(
            "The playing tone differs from saved channel {}. Switch to {} and discard unsaved changes on the PODxt?",
            confirmation.playing().channel.label(),
            target.label(),
        ));
        ui.horizontal_wrapped(|ui| {
            if ui.button("Cancel switch").clicked() {
                self.cancel_switch();
            } else if ui
                .add_enabled(
                    idle && Self::admitted(Action::Recall(target)),
                    egui::Button::new(format!("Switch to {} and discard changes", target.label())),
                )
                .clicked()
                && let Some(confirmation) = self.take_recall_confirmation(device)
            {
                self.start_job(
                    Action::Recall(target),
                    device,
                    ui.ctx(),
                    Some(confirmation),
                    None,
                );
            }
        });
    }

    pub(super) fn set_live(&mut self, on: bool) {
        if !on {
            self.clear_recall_confirmation();
            if let Some(worker) = &self.worker
                && matches!(worker.action, Action::ReplayDraft(_))
            {
                worker.stop.store(true, Ordering::Release);
            }
        }
        self.stage_only = !on;
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

    pub(super) fn stage_editable(&self) -> bool {
        self.session.is_some()
            && !self.needs_get
            && self.queued_undo.is_none()
            && !self.reply_pending.load(Ordering::Acquire)
            && self.admits(Action::Controls)
            && self
                .worker
                .as_ref()
                .is_none_or(|worker| worker.action == Action::Controls)
    }

    pub(super) fn queue_stage_target(&mut self, target: LiveTarget, ctx: &egui::Context) {
        if self.stage_editable() {
            self.clear_recall_confirmation();
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
        let help = "Apply staged edits to the playing edit buffer. Store to POD… saves to a hardware channel.";
        if ui
            .add_enabled(
                ready && self.has_pending() && Self::admitted(Action::Send),
                egui::Button::new("Apply to playing tone"),
            )
            .on_hover_text(help)
            .on_disabled_hover_text(help)
            .clicked()
            && let Some(device) = device
        {
            self.start(Action::Send, device, ui.ctx());
        }
        if ui
            .add_enabled(
                idle && device.is_some_and(|device| self.can_request_undo(device)),
                egui::Button::new("Undo live edits"),
            )
            .clicked()
            && let Some(device) = device
        {
            self.request_undo(device, ui.ctx());
        }
        if self.queued_undo.is_some() && ui.button("Cancel pending Undo").clicked() {
            self.cancel_queued_undo("The pending restore was cancelled.");
        }
        if self.stage_only && self.has_pending() && ui.button("Discard staged edits").clicked() {
            self.discard_staged();
        }
    }

    pub(super) fn progress_label(&self) -> Option<&'static str> {
        self.worker
            .as_ref()
            .and_then(|worker| worker.action.progress_label())
    }

    pub(super) fn status_text(&self) -> Option<&str> {
        if self.queued_undo.is_some() {
            return Some(UNDO_PENDING);
        }
        self.progress_label().or_else(|| {
            self.message.as_ref().map(|result| match result {
                Ok(message) | Err(message) => message.as_str(),
            })
        })
    }

    pub(super) fn status(&self, ui: &mut egui::Ui) {
        if self.queued_undo.is_some() {
            ui.weak(format!("{UNDO_PENDING}…"));
        } else if let Some(label) = self.progress_label() {
            ui.weak(format!("{label}…"));
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
                    idle && self.can_get(),
                    egui::Button::new("Recover edit session"),
                )
                .on_hover_text("Read the playing edit buffer to restore the editor session.")
                .on_disabled_hover_text("Recovery requires an idle connection and no staged edits.")
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
                let attachment_session = Arc::clone(&self.attachment_session);
                services.request_picker::<PodxtEditor>(
                    ui.ctx(),
                    PickRequest::Folder,
                    move |editor, folder, _| {
                        let Some(folder) = folder else { return };
                        let editor = &mut editor.live.editor;
                        if !Arc::ptr_eq(&editor.attachment_session, &attachment_session)
                            || editor.busy()
                            || editor.needs_get
                            || !editor
                                .session
                                .as_ref()
                                .is_some_and(|active| active.current.channel == channel)
                        {
                            return;
                        }
                        editor.whole_tone_backup = None;
                        editor.replay_backups.remove(&channel.address());
                        match podxt::verify_tone_backup(&folder, channel) {
                            Ok(backup) => {
                                editor.message = None;
                                let backup = Arc::new(backup);
                                editor
                                    .replay_backups
                                    .insert(channel.address(), Arc::clone(&backup));
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
            ui.weak("Reading the edit buffer establishes the undo snapshot.");
        }
        if let Some(label) = self.progress_label() {
            ui.label(format!("{label} in progress…"));
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
pub(super) mod tests;

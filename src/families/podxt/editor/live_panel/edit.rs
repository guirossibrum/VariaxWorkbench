mod diagnostics;
use diagnostics::{ControlBatch, ControlChange, RecallLog};
use variax_workbench::session_log::{
    Disposition, Endpoint, Operation as SessionOperation, PodxtSyncEvidence as Evidence,
    PodxtSyncReason as Reason,
};

use super::*;
use podxt::{
    EditorBuffer, EditorRecall, LiveTarget, OutgoingDraft, RecallConfirmation, Target, TonePayload,
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
    recall_confirmation: Option<PendingRecall>,
    last_dispatch: Option<Instant>,
    message: Option<Result<String, String>>,
    #[cfg(test)]
    session_log: Option<variax_workbench::session_log::SessionLog>,
    #[cfg(test)]
    fake_get: bool,
    #[cfg(test)]
    fake_recall: Option<tests::FakeRecall>,
    #[cfg(test)]
    fake_edit: Option<tests::FakeEdit>,
    #[cfg(test)]
    fake_dispatch: Option<Arc<tests::FakeDispatch>>,
    #[cfg(test)]
    fake_native_admission: bool,
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
            session_log: None,
            #[cfg(test)]
            fake_get: false,
            #[cfg(test)]
            fake_recall: None,
            #[cfg(test)]
            fake_edit: None,
            #[cfg(test)]
            fake_dispatch: None,
            #[cfg(test)]
            fake_native_admission: false,
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
    attached: HashMap<u16, SessionDraft>,
    previous_attachments: Vec<HashMap<u16, SessionDraft>>,
}

struct SessionDraft {
    payload: TonePayload,
    imported: bool,
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
                self.attached.insert(
                    slot.address(),
                    SessionDraft {
                        payload: after,
                        imported: false,
                    },
                );
            }
        }
    }
}

struct QueuedUndo {
    device: detection::DeviceKey,
    snapshot: EditorBuffer,
}

#[derive(Default)]
struct Pending {
    targets: Vec<LiveTarget>,
    log: Option<ControlBatch>,
}

impl Pending {
    fn clear(&mut self, disposition: Disposition) {
        self.targets.clear();
        self.stop_logging(disposition);
    }

    fn stop_logging(&mut self, disposition: Disposition) {
        if let Some(batch) = self.log.take() {
            batch.record(disposition, None);
        }
    }

    fn set(&mut self, target: LiveTarget) {
        match self
            .targets
            .iter_mut()
            .find(|entry| entry.control() == target.control())
        {
            Some(entry) => *entry = target,
            None => self.targets.push(target),
        }
    }

    fn overlay(&self, targets: &[LiveTarget], control: LiveControl, current: u16) -> u16 {
        self.targets
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
    recall_log: Option<RecallLog>,
    control_log: Option<Arc<ControlBatch>>,
    superseded: bool,
    program_read: bool,
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

struct PendingRecall {
    device: detection::DeviceKey,
    confirmation: RecallConfirmation,
    log: Option<RecallLog>,
}

enum RecallAuthorization {
    Cached(Arc<recall_cache::Bank>),
    Confirmed(RecallConfirmation),
    RetainedDraft,
}

struct Job {
    recall_log: Option<RecallLog>,
    control_log: Option<SessionOperation>,
    recall_authorization: Option<RecallAuthorization>,
    outgoing_draft: Option<(Location, TonePayload)>,
    reply_pending: Arc<AtomicBool>,
    action: Action,
    whole_tone_backup: Option<Arc<VerifiedToneBackup>>,
    replay: Option<Arc<DraftReplay>>,
    current: Option<EditorBuffer>,
    original: Option<EditorBuffer>,
    targets: Vec<LiveTarget>,
}

impl Job {
    fn operation(&self) -> Option<&SessionOperation> {
        self.recall_log
            .as_ref()
            .map(RecallLog::operation)
            .or(self.control_log.as_ref())
    }
}

#[cfg(any(test, target_os = "linux", target_os = "macos"))]
fn edit_transaction<'a, G>(
    job: &'a Job,
    stop: &'a AtomicBool,
    lock: impl FnOnce() -> Result<G, String>,
    with_link: impl FnOnce(&G, RunOnLink<'a, EditOutcome>) -> Result<EditOutcome, String>,
) -> Result<EditOutcome, String> {
    let selected_from_cache = std::rc::Rc::new(std::cell::Cell::new(false));
    let cached = match (&job.recall_authorization, &job.current, job.action) {
        (Some(RecallAuthorization::Cached(bank)), Some(current), Action::Recall(target)) => {
            Some((bank, current, target))
        }
        _ => None,
    };
    let run = || {
        let stopped = || stop.load(Ordering::Acquire);
        let selected_in_transaction = selected_from_cache.clone();
        let result = with_session(
            stopped,
            &job.reply_pending,
            lock,
            with_link,
            EditPolicy(job.action),
            job.operation().cloned(),
            move |session| {
                let edit = match job.action {
                    Action::Get | Action::Follow(_) => {
                        return track_reply(&job.reply_pending, |outstanding| {
                            podxt::get_editor_buffer_tracking_reply(session, stopped, outstanding)
                        })
                        .map(EditOutcome::Buffer);
                    }
                    Action::Recall(slot) => {
                        if let Some((bank, current, target)) = cached
                            && let (Some(source), Some(destination)) =
                                (bank.slot(current.channel), bank.slot(target))
                        {
                            let still_observed = || bank.observes_pair(current.channel, target);
                            let attempt = track_reply(&job.reply_pending, |outstanding| {
                                podxt::try_cached_recall(
                                    session,
                                    target,
                                    podxt::CachedRecall {
                                        current,
                                        source: &source.tone,
                                        destination,
                                        still_observed: &still_observed,
                                    },
                                    stopped,
                                    outstanding,
                                )
                            })?;
                            if let Some(buffer) = attempt {
                                selected_in_transaction.set(true);
                                return Ok(EditOutcome::Recall(EditorRecall::Playing(buffer)));
                            }
                        }
                        let authorization = job.recall_authorization.as_ref();
                        let confirmation = match authorization {
                            Some(RecallAuthorization::Confirmed(confirmation)) => {
                                Some(confirmation)
                            }
                            _ => None,
                        };
                        let draft = job.outgoing_draft.as_ref().map(|(slot, payload)| {
                            if matches!(authorization, Some(RecallAuthorization::RetainedDraft)) {
                                OutgoingDraft::Required(*slot, payload)
                            } else {
                                OutgoingDraft::Available(*slot, payload)
                            }
                        });
                        let mut observe = |observation: podxt::RecallObservation<'_>| {
                            if let Some(log) = &job.recall_log {
                                log.observe(observation);
                            }
                        };
                        return podxt::recall_live_channel_observed(
                            session,
                            slot,
                            confirmation,
                            draft,
                            stopped,
                            job.recall_log
                                .as_ref()
                                .filter(|log| {
                                    line6_devices::session::Log::observing(log.operation())
                                })
                                .map(|_| {
                                    &mut observe as &mut dyn FnMut(podxt::RecallObservation<'_>)
                                }),
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
        );
        if let Some((bank, current, target)) = cached {
            if result.is_err() {
                bank.invalidate();
                if selected_from_cache.get() {
                    // The protocol succeeded but the checked link did not.
                    job.reply_pending.store(true, Ordering::Release);
                }
            } else if selected_from_cache.get()
                && (stopped() || !bank.observes_pair(current.channel, target))
            {
                bank.invalidate();
                return Err(
                    "PODxt recall evidence changed before release. Read the playing tone again."
                        .into(),
                );
            }
        }
        result
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
                with_checked_edit_link(device, rig, stop, job.operation(), run)
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
    log: Option<&SessionOperation>,
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
        match checked_edit_release(log, || link.finish()) {
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
        with_verified_platform_link(device, rig, log, run)
    }
}

#[cfg(any(test, target_os = "linux"))]
fn checked_edit_release<T>(
    log: Option<&SessionOperation>,
    finish: impl FnOnce() -> Result<(), T>,
) -> Result<(), T> {
    let started = Instant::now();
    let result = finish();
    if let Some(log) = log {
        log.checked_release(result.is_ok(), started.elapsed());
    }
    result
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
        &job.recall_authorization,
        &job.outgoing_draft,
        job.operation(),
    );
    let _ = (EditOutcome::Buffer, EditOutcome::Recall);
    Err("PODxt Live editing is unavailable in this build.".into())
}

impl LiveEditor {
    fn log_operation(&self) -> SessionOperation {
        #[cfg(test)]
        if let Some(log) = &self.session_log {
            return SessionOperation::with_log(log, Endpoint::Podxt);
        }
        SessionOperation::begin(Endpoint::Podxt)
    }

    #[cfg(test)]
    pub(super) fn use_session_log(&mut self, log: variax_workbench::session_log::SessionLog) {
        self.session_log = Some(log);
    }

    #[cfg(test)]
    pub(super) fn set_test_last_dispatch(&mut self, when: Option<Instant>) {
        self.last_dispatch = when;
    }

    pub(super) fn busy(&self) -> bool {
        self.worker.is_some() || !self.stage_only && !self.pending.targets.is_empty()
    }

    pub(super) fn cancel(&mut self) {
        self.clear_recall_confirmation();
        if !self.stage_only {
            self.pending.clear(Disposition::Cancelled);
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
        self.pending.clear(Disposition::Cancelled);
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
    pub(super) fn use_fake_recall_after_current_read(&mut self) {
        self.fake_recall = Some(tests::FakeRecall::AfterCurrentRead);
    }

    #[cfg(test)]
    pub(super) fn use_fake_recall_with_changed_source(&mut self) {
        self.fake_recall = Some(tests::FakeRecall::ChangedSource);
    }

    #[cfg(test)]
    pub(super) fn use_scripted_actions(
        &mut self,
        steps: Arc<Mutex<std::collections::VecDeque<tests::ScriptedAction>>>,
    ) {
        self.fake_native_admission = true;
        self.fake_dispatch = Some(Arc::new(move |job, stop| {
            let step = steps
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected editor action");
            step.run(job, stop)
        }));
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
            None,
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
        self.drafts
            .attached
            .get(&slot.address())
            .map(|draft| &draft.payload)
    }

    pub(super) fn draft_is_imported(&self, slot: Location) -> bool {
        self.drafts
            .attached
            .get(&slot.address())
            .is_some_and(|draft| draft.imported)
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
        let draft = self.drafts.attached.get(&slot.address())?;
        let confirmed = self.session.as_ref().filter(|active| {
            active.current.channel == slot
                && !self.needs_get
                && !self.reply_pending.load(Ordering::Acquire)
        });
        Some(match confirmed {
            Some(active)
                if TonePayload::from_dump(&active.current.tone).as_ref() == Ok(&draft.payload) =>
            {
                if draft.imported {
                    "Device edit buffer · retained as draft"
                } else {
                    "Session draft · playing"
                }
            }
            Some(_) => "Draft retained · playing tone differs",
            None => "Draft retained · playing tone unverified",
        })
    }

    pub(super) fn discard_staged(&mut self) {
        self.pending.clear(Disposition::Cancelled);
    }

    pub(super) fn observe_program(&mut self, slot: Location) -> bool {
        let expired = self
            .recall_confirmation
            .as_ref()
            .is_some_and(|pending| slot != pending.confirmation.playing().channel);
        if expired {
            self.clear_recall_confirmation();
        }
        if let Some(worker) = &mut self.worker {
            match worker.action {
                // The app's own program change can reach passive Follow.
                Action::Recall(target) if slot != target => {
                    worker.superseded = true;
                    worker.stop.store(true, Ordering::Release);
                }
                _ if worker.program_read => worker.superseded = true,
                Action::ReplayDraft(_) => worker.stop.store(true, Ordering::Release),
                _ => {}
            }
        }
        expired
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
            .is_some_and(|pending| !pending.confirmation.matches_playing(&buffer));
        let changed_during_undo = self.queued_undo.is_some()
            && self.session.as_ref().is_none_or(|active| {
                active.current.channel != buffer.channel || active.current.tone != buffer.tone
            });
        self.completed(Action::Get, Ok(buffer));
        if expired {
            self.expire_recall_confirmation();
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
        self.dismiss_recall_confirmation(Reason::Expired);
    }

    fn dismiss_recall_confirmation(&mut self, reason: Reason) {
        if let Some(PendingRecall {
            confirmation,
            log: Some(log),
            ..
        }) = self.recall_confirmation.take()
        {
            log.sync(
                Some(confirmation.playing().channel),
                reason,
                Evidence::VerifiedPlaying,
            );
        }
    }

    fn expire_recall_confirmation(&mut self) {
        self.clear_recall_confirmation();
        self.message = Some(Err(
            "The playing tone changed. Select the channel again.".into()
        ));
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
            return Self::admitted_on(
                true,
                !self.fake_native_admission || action.native_approval(),
                action,
            );
        }
        Self::admitted_on(available, action.native_approval(), action)
    }

    fn start(&mut self, action: Action, device: &detection::DeviceKey, ctx: &egui::Context) {
        self.start_job(action, device, ctx, None, None, None);
    }

    fn start_job(
        &mut self,
        action: Action,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
        recall_authorization: Option<RecallAuthorization>,
        replay: Option<Arc<DraftReplay>>,
        recall_log: Option<RecallLog>,
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
        // An initial recall verifies the playing state on the same link
        // before it can select anything.
        let fresh_recall = matches!(action, Action::Recall(_))
            && matches!(
                recall_authorization,
                None | Some(RecallAuthorization::Cached(_))
            );
        if self.worker.is_some()
            || self.reply_pending.load(Ordering::Acquire)
            || !admitted
            || action == Action::ResendWholeTone && self.whole_tone_backup.is_none()
            || !action.is_read() && !fresh_recall && !self.recall_ready()
            || action == Action::Undo && !self.can_undo()
            || matches!(action, Action::Recall(_)) && (self.stage_only || self.has_pending())
        {
            self.recall_blocked(recall_log.as_ref());
            return;
        }
        let retained_recall = matches!(
            recall_authorization,
            Some(RecallAuthorization::RetainedDraft)
        );
        if retained_recall {
            self.recall_confirmation = None;
        } else {
            self.clear_recall_confirmation();
        }
        let control_log = if action == Action::Controls {
            self.pending.log.take()
        } else {
            None
        };
        let targets = match action {
            Action::Send | Action::Controls => std::mem::take(&mut self.pending.targets),
            Action::Recall(_) => Vec::new(),
            Action::Get
            | Action::Follow(_)
            | Action::Undo
            | Action::ResendWholeTone
            | Action::ReplayDraft(_) => {
                self.pending.clear(Disposition::Cancelled);
                Vec::new()
            }
        };
        if matches!(action, Action::Get | Action::Follow(_) | Action::Recall(_)) {
            self.whole_tone_backup = None;
        }
        let outgoing_draft = self
            .session
            .as_ref()
            .filter(|_| fresh_recall || retained_recall)
            .and_then(|active| {
                let slot = active.current.channel;
                self.draft(slot).cloned().map(|payload| (slot, payload))
            });
        if retained_recall && outgoing_draft.is_none() {
            if let Some(log) = &recall_log {
                log.sync(None, Reason::Expired, Evidence::None);
            }
            self.expire_recall_confirmation();
            return;
        }
        let job = Job {
            recall_log: recall_log.clone(),
            control_log: control_log.as_ref().map(|batch| batch.operation.clone()),
            recall_authorization,
            outgoing_draft,
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
        let control_log = control_log.map(Arc::new);
        let dispatch_log = control_log.clone();
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
                if let Some(batch) = dispatch_log {
                    batch.record(Disposition::Accepted, None);
                }
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
                    recall_log,
                    control_log,
                    superseded: false,
                    program_read: matches!(action, Action::Follow(_)),
                    replay,
                    thread: Some(thread),
                });
            }
            Err(error) => {
                self.pending.targets = targets;
                self.pending.log = control_log.and_then(Arc::into_inner);
                if let Some(log) = recall_log {
                    log.sync(None, Reason::Failed, Evidence::None);
                    log.operation().finished(false);
                }
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
                    Action::Controls | Action::Recall(_) => None,
                    Action::Get | Action::Follow(_) => Some(format!(
                        "Edit buffer read from {}.",
                        active.current.channel.label()
                    )),
                    Action::Undo => Some("Original snapshot verified byte for byte.".into()),
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
                    self.pending.clear(Disposition::Cancelled);
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
        recall_log: Option<RecallLog>,
    ) -> Option<&EditorBuffer> {
        match result {
            Ok(
                EditOutcome::Buffer(buffer) | EditOutcome::Recall(EditorRecall::Playing(buffer)),
            ) => {
                if let Some(log) = recall_log {
                    if stopped {
                        log.sync(None, Reason::Cancelled, Evidence::None);
                    } else {
                        log.button(Disposition::Applied);
                        log.operation().finished(true);
                    }
                }
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
                    if let Some(log) = &recall_log {
                        log.sync(
                            Some(confirmation.playing().channel),
                            Reason::ConfirmationRequired,
                            Evidence::CheckedRelease,
                        );
                    }
                    self.recall_confirmation = Some(PendingRecall {
                        device: device.clone(),
                        confirmation,
                        log: recall_log,
                    });
                } else if let Some(log) = recall_log {
                    log.sync(
                        Some(confirmation.playing().channel),
                        Reason::Expired,
                        Evidence::CheckedRelease,
                    );
                }
                self.message = None;
                self.session.as_ref().map(|active| &active.current)
            }
            Ok(EditOutcome::Recall(EditorRecall::ConfirmationExpired(playing))) => {
                if let Some(log) = recall_log {
                    log.sync(
                        Some(playing.channel),
                        Reason::Expired,
                        Evidence::CheckedRelease,
                    );
                }
                self.completed(Action::Get, Ok(playing));
                self.message = Some(Err(
                    "The playing tone changed. Select the channel again.".into()
                ));
                self.session.as_ref().map(|active| &active.current)
            }
            Err(error) => {
                if let Some(log) = recall_log {
                    log.sync(
                        None,
                        if stopped {
                            Reason::Cancelled
                        } else {
                            Reason::Failed
                        },
                        Evidence::None,
                    );
                    log.operation().finished(false);
                }
                self.completed(action, Err(error))
            }
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
        self.start_job(action, device, ctx, None, Some(replay), None);
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
                if !self.stage_only && !self.pending.targets.is_empty() {
                    ctx.request_repaint();
                }
                if worker.action == Action::Send && self.stage_only && result.is_err() {
                    self.needs_get = true;
                    self.pending.targets = worker.targets.clone();
                    self.message = Some(result.map(|_| String::new()));
                    return None;
                }
                let stopped = worker.stop.load(Ordering::Acquire);
                if worker.superseded {
                    if let Some(log) = &worker.recall_log {
                        log.sync(None, Reason::Superseded, Evidence::None);
                    }
                    // A newer program notice will read back the latest selection.
                    // Keep the previous verified highlight until that read completes.
                    self.needs_get = true;
                    return None;
                }
                let selection = match (&worker.action, &result) {
                    (Action::Recall(target), Ok(EditOutcome::Recall(EditorRecall::Playing(_))))
                    | (Action::Follow(target), Ok(EditOutcome::Buffer(_)))
                        if !stopped =>
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
                if let Some(batch) = &worker.control_log {
                    match &result {
                        Ok(EditOutcome::Buffer(buffer)) if !stopped => {
                            batch.record(Disposition::Applied, Some(buffer));
                            batch.operation.finished(true);
                        }
                        _ => {
                            batch.record(
                                if stopped {
                                    Disposition::Cancelled
                                } else {
                                    Disposition::Failed
                                },
                                None,
                            );
                            batch.operation.finished(false);
                        }
                    }
                }
                let verified = self
                    .complete_job(
                        worker.action,
                        device,
                        stopped,
                        result,
                        worker.recall_log.take(),
                    )
                    .is_some();
                self.finish_queued_undo(device, read_idle, ctx, stopped, verified);
                if verified && read_idle {
                    if let Some(target) = selection {
                        self.replay_selected(target, device, ctx);
                    } else if matches!(worker.action, Action::Recall(_)) {
                        self.retain_outgoing_device_draft(device, ctx);
                    }
                }
                return if verified {
                    self.session.as_ref().map(|active| &active.current)
                } else {
                    None
                };
            }
            if let Some(log) = &worker.recall_log {
                log.sync(None, Reason::Superseded, Evidence::None);
            }
            if let Some(batch) = &worker.control_log {
                batch.record(Disposition::Cancelled, None);
            }
            self.cancel_queued_undo("The PODxt Live attachment changed.");
        }
        if let Some(device) =
            device.filter(|_| read_idle && !self.stage_only && !self.pending.targets.is_empty())
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

    fn retain_outgoing_device_draft(&mut self, device: &detection::DeviceKey, ctx: &egui::Context) {
        let Some(pending) = &self.recall_confirmation else {
            return;
        };
        let confirmation = &pending.confirmation;
        let playing = confirmation.playing();
        if pending.device != *device
            || !confirmation.comparison().can_retain_as_draft()
            || self.draft(playing.channel).is_some()
        {
            return;
        }
        let Ok(payload) = TonePayload::from_dump(&playing.tone) else {
            return;
        };
        let target = confirmation.target();
        let source = playing.channel;
        let log = pending.log.clone();
        self.drafts.attached.insert(
            source.address(),
            SessionDraft {
                payload,
                imported: true,
            },
        );
        // The candidate came from a checked transaction with a clean release.
        // A continuation may select only if the freshly read outgoing bytes
        // still exactly match the retained copy.
        if let Some(log) = &log {
            log.retained(source);
        }
        self.start_job(
            Action::Recall(target),
            device,
            ctx,
            Some(RecallAuthorization::RetainedDraft),
            None,
            log,
        );
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
        self.pending.clear(Disposition::Cancelled);
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
            if let Some(worker) = &mut self.worker {
                worker.program_read = true;
            }
        }
    }

    pub(super) fn recall_requested(&self, slot: Location) -> Option<RecallLog> {
        RecallLog::requested(self.log_operation(), slot)
    }

    pub(super) fn recall_blocked(&self, log: Option<&RecallLog>) {
        if let Some(log) = log {
            log.button(Disposition::Blocked);
            if self.has_pending() {
                log.sync(None, Reason::PendingEdits, Evidence::EditorQueue);
            }
        }
    }

    pub(super) fn recall_with_cache(
        &mut self,
        slot: Location,
        device: &detection::DeviceKey,
        ctx: &egui::Context,
        log: Option<RecallLog>,
        cache: Option<Arc<recall_cache::Bank>>,
    ) {
        if let Some(pending) = &self.recall_confirmation {
            self.recall_blocked(log.as_ref());
            if pending.confirmation.target() != slot {
                self.message = Some(Ok(
                    "Cancel the pending switch before selecting another channel.".into(),
                ));
            }
            return;
        }
        if self.pending.targets.is_empty() {
            self.start_job(
                Action::Recall(slot),
                device,
                ctx,
                cache.map(RecallAuthorization::Cached),
                None,
                log,
            );
        } else {
            self.recall_blocked(log.as_ref());
            self.message = Some(Err(
                "Send or clear staged edits before selecting another channel.".into(),
            ));
        }
    }

    fn cancel_switch(&mut self) {
        if let Some(log) = self
            .recall_confirmation
            .as_ref()
            .and_then(|pending| pending.log.as_ref())
        {
            log.button(Disposition::Cancelled);
        }
        self.dismiss_recall_confirmation(Reason::Cancelled);
        self.message = Some(Ok("Kept the playing tone.".into()));
    }

    fn take_recall_confirmation(
        &mut self,
        device: &detection::DeviceKey,
    ) -> Option<(RecallConfirmation, Option<RecallLog>)> {
        let PendingRecall {
            device: attachment,
            confirmation,
            log,
        } = self.recall_confirmation.take()?;
        if let Some(log) = &log {
            log.button(Disposition::Accepted);
        }
        if attachment == *device
            && !self.stage_only
            && !self.has_pending()
            && self.recall_ready()
            && self
                .session
                .as_ref()
                .is_some_and(|active| confirmation.matches_playing(&active.current))
        {
            if let Some(log) = &log {
                log.sync(
                    Some(confirmation.playing().channel),
                    Reason::Confirmed,
                    Evidence::VerifiedPlaying,
                );
            }
            Some((confirmation, log))
        } else {
            if let Some(log) = log {
                log.sync(
                    Some(confirmation.playing().channel),
                    Reason::Expired,
                    Evidence::VerifiedPlaying,
                );
            }
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
        let Some(pending) = &self.recall_confirmation else {
            return;
        };
        if pending.device != *device || self.stage_only || self.has_pending() {
            self.clear_recall_confirmation();
            return;
        }
        let confirmation = &pending.confirmation;
        let target = confirmation.target();
        ui.label(format!(
            "The playing tone differs from the saved tone read from {}. Switch to {} and leave the playing tone behind?",
            confirmation.playing().channel.label(),
            target.label(),
        ));
        ui.collapsing("Playing tone comparison", |ui| {
            egui::ScrollArea::vertical()
                .max_height(110.0)
                .show(ui, |ui| {
                    ui.label(super::comparison_text(
                        "Addressed saved slot read during this switch",
                        confirmation.comparison(),
                    ));
                });
        });
        ui.horizontal_wrapped(|ui| {
            let cancel = ui.button("Cancel switch");
            if cancel.clicked() || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.cancel_switch();
            } else if ui
                .add_enabled(
                    idle && self.admits(Action::Recall(target)),
                    egui::Button::new(format!("Switch to {} and discard changes", target.label())),
                )
                .clicked()
                && let Some((confirmation, log)) = self.take_recall_confirmation(device)
            {
                self.start_job(
                    Action::Recall(target),
                    device,
                    ui.ctx(),
                    Some(RecallAuthorization::Confirmed(confirmation)),
                    None,
                    log,
                );
            }
        });
    }

    pub(super) fn set_live(&mut self, on: bool) {
        if !on {
            self.clear_recall_confirmation();
            self.pending.stop_logging(Disposition::Cancelled);
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

    pub(super) fn queue_stage_target(
        &mut self,
        target: LiveTarget,
        shown: Option<&EditBuffer>,
        ctx: &egui::Context,
    ) {
        if self.stage_editable() {
            self.clear_recall_confirmation();
            if !self.stage_only && !self.pending.targets.contains(&target) {
                self.log_stage_target(target, shown);
            }
            self.pending.set(target);
            ctx.request_repaint();
        }
    }

    fn log_stage_target(&mut self, target: LiveTarget, shown: Option<&EditBuffer>) {
        let active = self.session.as_ref().expect("stage has a playing buffer");
        let control = target.control();
        let in_flight = self
            .worker
            .as_ref()
            .map_or(&[][..], |worker| worker.targets.as_slice());
        let old = self.pending.overlay(
            in_flight,
            control,
            control.read(shown.unwrap_or(&active.current.tone)),
        );
        let new = match target {
            LiveTarget::Basic(_, Target::Value(value)) => u16::from(value.get()),
            LiveTarget::Additional(_, value) => value,
            _ => return,
        };
        if self.pending.log.is_none() {
            self.pending.log = Some(ControlBatch::new(self.log_operation()));
        }
        self.pending
            .log
            .as_mut()
            .expect("control log batch")
            .queue(ControlChange { control, old, new });
    }

    pub(super) fn stage_payload(&self, dump: &EditBuffer) -> Option<[u8; 160]> {
        let mut payload = *podxt::TonePayload::from_dump(dump).ok()?.as_bytes();
        let in_flight = self
            .worker
            .as_ref()
            .map_or(&[][..], |worker| worker.targets.as_slice());
        for &target in in_flight.iter().chain(&self.pending.targets) {
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
        !self.pending.targets.is_empty()
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
                        && self.pending.targets.is_empty()
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
